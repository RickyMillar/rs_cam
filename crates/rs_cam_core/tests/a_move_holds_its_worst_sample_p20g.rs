//! Feeds contract P20g (`planning/feeds_contract_2026-11-03/phases/P20.md`
//! section 8; defect `P20_power_gap.md`): a move has one F word, so each load
//! cap of the modulator (power, deflection, force budget) is the MINIMUM over
//! the move's samples of that cap at each sample's own engagement (lead
//! rulings 1 and 2, 2026-11-09).
//!
//! Before P20g the modulator built ONE engagement per move from the
//! time-weighted mean width and depth, and the gates graded each sample at
//! its own engagement. A long move with a short full-slot section passed the
//! modulator and failed the gate (wanaka idx 1, move 1697: emitted 4516
//! mm/min, its slot sample needs about 730).
//!
//! The fixture (test values, not a machine claim): D 6 flat end mill, Z 2,
//! white oak, 18 000 rpm, depth 3 mm. One long straight move carries 20 thin
//! samples (ae 0.3 mm) and one full-slot sample. The band minimum is far
//! below every cap, so the chip floor does not bind; the band maximum is far
//! above, so only the load arm under test binds.
//!
//! Arms:
//!
//! (a) power: the modulated feed is at or below the feed at which the power
//!     gate's grade of the slot sample (`tool_load::power::sample_power_kw`)
//!     fits the available power.
//! (b) deflection: the same with the deflection gate's grade
//!     (`tool_load::deflection::sample_tip_deflection_mm`); (b2) the force
//!     budget with the force card's reading (`cut_force::sample_peak_force_n`).
//! (c) a uniform move (every sample at the mean) gives the same feed with and
//!     without the side table.
//! (e) a band whose chip floor is above the power cap of the slot sample:
//!     the load cap wins (lead ruling 2026-11-09): the feed is at or below
//!     the power cap, the binding is power, and the outcome carries
//!     `LOAD_CAP_BELOW_CHIP_FLOOR_NOTE` (the rub risk). Red before the change:
//!     the floor feed was emitted.
//! (d) the side table entry (`SampleEngagement::of_sample`, the function the
//!     session calls) equals the gates' own builders.
//!
//! Each of (a), (b), (b2) also runs the move with no side table (the
//! pre-P20g mean path) and asserts that it emits above the gate's fit, so
//! the fixture discriminates. Red check: with the per-sample fold disabled,
//! (a) fails (P20.md section 8.5).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, DeflectionLimitInputs, ForceBudgetLimitInputs,
    LOAD_CAP_BELOW_CHIP_FLOOR_NOTE, ModulationContext, ModulationCutter, ModulationOutcome,
    ModulationStrategy, MoveSampleEngagements, PerMoveEngagement, PowerLimitInputs, RadialThinning,
    SampleEngagement, adaptive_feed_modulate,
};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{Engagement as SimEngagement, SimulationCutSample};
use rs_cam_core::tool::ToolDefinition;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const D_MM: f64 = 6.0;
const AP_MM: f64 = 3.0;
const FLUTES: u32 = 2;
const RPM: u32 = 18_000;
const FLUTE_LENGTH_MM: f64 = 25.0;
const THIN_AE_MM: f64 = 0.3;
const THIN_COUNT: usize = 20;
/// The feed (mm/min) at which the slot sample meets each budget of the
/// fixture; the budgets are set from the gates' own grade at this feed.
const SLOT_FIT_FEED: f64 = 600.0;
/// One long straight cut, so the kinematic reach does not bind.
const CUT_LENGTH_MM: f64 = 5000.0;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(FLUTE_LENGTH_MM));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    tool
}

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn cutter(tool: &ToolConfig) -> ModulationCutter {
    ModulationCutter {
        geometry: tool.geometry_hint(),
        diameter_mm: tool.diameter,
        max_cut_diameter_mm: rs_cam_core::feeds::CutBody::of_tool(tool).max_cut_diameter_mm,
        helix_deg: tool.helix_deg,
    }
}

/// The arc of a width `ae` on the diameter `D`: `ae = r (1 - cos psi)`.
fn arc_of(ae_mm: f64) -> f64 {
    (1.0 - 2.0 * ae_mm / D_MM).clamp(-1.0, 1.0).acos()
}

/// One cutting sample of move 1 at the width `ae_mm` (an uncoalesced
/// sample: the stamped fraction `ae / D` and the arc agree).
fn sample(ae_mm: f64) -> SimulationCutSample {
    SimulationCutSample {
        move_index: 1,
        segment_time_s: 0.01,
        is_cutting: true,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: AP_MM,
        axial_engagement_mm: AP_MM,
        arc_engagement_radians: Some(arc_of(ae_mm)),
        engagement: SimEngagement::with_radial_woc(ae_mm / D_MM),
        ..SimulationCutSample::test_fixture()
    }
}

fn slot() -> SimulationCutSample {
    sample(D_MM)
}

/// 20 thin samples and one slot sample.
fn mixed_samples() -> Vec<SimulationCutSample> {
    let mut s: Vec<SimulationCutSample> = (0..THIN_COUNT).map(|_| sample(THIN_AE_MM)).collect();
    s.push(slot());
    s
}

/// The per-move mean the session builds (time-weighted; equal weights here)
/// and the minimum lift of P11b2.
fn mean_engagement(samples: &[SimulationCutSample], tool: &ToolDefinition) -> PerMoveEngagement {
    let n = samples.len() as f64;
    let woc = samples
        .iter()
        .map(|s| s.engagement.radial_woc_fraction)
        .sum::<f64>()
        / n;
    let lift = samples
        .iter()
        .filter_map(|s| {
            rs_cam_core::tool_load::display::sample_thinning_lift(s, tool, RadialThinning::Applied)
        })
        .fold(f64::INFINITY, f64::min);
    PerMoveEngagement {
        radial_woc_fraction: woc,
        axial_doc_fraction: AP_MM / FLUTE_LENGTH_MM,
        mean_axial_mm: AP_MM,
        thinning_lift: Some(lift),
    }
}

/// The side table the session builds: one entry per move, move 1 holds the
/// samples through `SampleEngagement::of_sample`.
fn table(samples: &[SimulationCutSample], tool: &ToolDefinition) -> Vec<MoveSampleEngagements> {
    vec![
        MoveSampleEngagements::default(),
        MoveSampleEngagements {
            samples: samples
                .iter()
                .map(|s| SampleEngagement::of_sample(s, tool))
                .collect(),
            mean_binds: false,
        },
    ]
}

#[derive(Clone, Copy)]
struct Arms {
    power: Option<PowerLimitInputs>,
    deflection: Option<DeflectionLimitInputs>,
    force_budget: Option<ForceBudgetLimitInputs>,
}

/// Modulate the one move; return the emitted feed and its binding.
fn modulate(
    samples: &[SimulationCutSample],
    with_table: bool,
    arms: Arms,
) -> (f64, BindingConstraint) {
    // The floor (0.001 x 18000 x 2 = 36 mm/min) is below every cap; the
    // maximum (1 mm/tooth, 36 000 mm/min) is above every cap.
    let band = ChiploadBand::new(0.001, 1.0).expect("a valid band");
    let outcome = modulate_in_band(samples, with_table, arms, band);
    *outcome.per_move.get(&1).expect("the cut is modulated")
}

/// Modulate the one move in `band`; return the whole outcome.
fn modulate_in_band(
    samples: &[SimulationCutSample],
    with_table: bool,
    arms: Arms,
    band: ChiploadBand,
) -> ModulationOutcome {
    let tool_cfg = end_mill();
    let tool = build_cutter(&tool_cfg).expect("a valid tool");
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    tp.feed_to_with_intent(
        P3::new(CUT_LENGTH_MM, 0.0, 0.0),
        5000.0,
        MoveIntent::ClearingCut,
    );
    let engagements = vec![
        PerMoveEngagement::default(),
        mean_engagement(samples, &tool),
    ];
    let side = table(samples, &tool);
    let kinematics = MachineKinematics::default();
    let machine = MachineProfile {
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    };
    let ctx = ModulationContext {
        spindle_rpm: f64::from(RPM),
        flute_count: FLUTES,
        cutting_ceiling: machine.cutting_ceiling(),
        rapid_feed_mm_min: 1.0e7,
        chipload_band: Some(band),
        radial_thinning: RadialThinning::Applied,
        kinematics: &kinematics,
        strategy: ModulationStrategy::ConstrainedMax,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: arms.deflection,
        power_inputs: arms.power,
        force_budget_inputs: arms.force_budget,
        sample_engagements: if with_table { &side } else { &[] },
        nominal_axial_doc_mm: AP_MM,
        plunge_rate_mm_min: 500.0,
    };
    adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("a valid context")
}

fn fz_of(feed: f64) -> f64 {
    feed / (f64::from(RPM) * f64::from(FLUTES))
}

fn power_arms() -> (Arms, f64) {
    let tool_cfg = end_mill();
    let tool = build_cutter(&tool_cfg).expect("a valid tool");
    let line = white_oak()
        .force_line()
        .expect("white oak has a force line");
    let available_kw =
        rs_cam_core::tool_load::power::sample_power_kw(&tool, &line, &slot(), SLOT_FIT_FEED)
            .expect("the slot sample has a power grade");
    let arms = Arms {
        power: Some(PowerLimitInputs {
            line,
            cutter: cutter(&tool_cfg),
            available_kw,
        }),
        deflection: None,
        force_budget: None,
    };
    (arms, available_kw)
}

#[test]
fn a_move_holds_the_power_of_its_slot_sample_p20g() {
    let tool = build_cutter(&end_mill()).expect("a valid tool");
    let line = white_oak()
        .force_line()
        .expect("white oak has a force line");
    let (arms, available_kw) = power_arms();
    let samples = mixed_samples();

    let (feed, binding) = modulate(&samples, true, arms);
    let graded = rs_cam_core::tool_load::power::sample_power_kw(&tool, &line, &slot(), feed)
        .expect("the slot sample has a power grade");
    assert_eq!(binding, BindingConstraint::PowerMax, "P20g: power binds");
    assert!(
        graded <= available_kw * (1.0 + 1e-9),
        "P20g (a): the gate grades the slot sample at {graded} kW at the modulated feed \
         {feed}; the available power is {available_kw} kW"
    );

    // The fixture discriminates: the mean path (no side table) emits above
    // the feed at which the slot sample fits.
    let (mean_feed, _) = modulate(&samples, false, arms);
    assert!(
        mean_feed > SLOT_FIT_FEED * 1.5,
        "fixture: the mean path emits {mean_feed}, above the slot fit {SLOT_FIT_FEED}"
    );
}

#[test]
fn a_move_holds_the_deflection_and_force_of_its_slot_sample_p20g() {
    let tool_cfg = end_mill();
    let tool = build_cutter(&tool_cfg).expect("a valid tool");
    let material = white_oak();
    let line = material.force_line().expect("white oak has a force line");
    let samples = mixed_samples();

    // (b) deflection: the compliance at the sample depth (every sample cuts
    // at AP_MM) and a bound that the slot sample meets at SLOT_FIT_FEED.
    let bound_mm = rs_cam_core::tool_load::deflection::sample_tip_deflection_mm(
        &tool,
        &material,
        &slot(),
        fz_of(SLOT_FIT_FEED),
    )
    .expect("the slot sample has a deflection grade");
    let arms = Arms {
        power: None,
        deflection: Some(DeflectionLimitInputs {
            line,
            cutter: cutter(&tool_cfg),
            compliance_mm_per_n: rs_cam_core::tool_load::deflection::tip_compliance_mm_per_n(
                &tool, AP_MM,
            )
            .expect("the tool has a compliance"),
            max_tip_deflection_mm: bound_mm,
        }),
        force_budget: None,
    };
    let (feed, binding) = modulate(&samples, true, arms);
    let graded = rs_cam_core::tool_load::deflection::sample_tip_deflection_mm(
        &tool,
        &material,
        &slot(),
        fz_of(feed),
    )
    .expect("the slot sample has a deflection grade");
    assert_eq!(binding, BindingConstraint::DeflectionMax);
    assert!(
        graded <= bound_mm * (1.0 + 1e-9),
        "P20g (b): the gate grades the slot sample at {graded} mm at the modulated feed \
         {feed}; the bound is {bound_mm} mm"
    );
    let (mean_feed, _) = modulate(&samples, false, arms);
    assert!(
        mean_feed > SLOT_FIT_FEED * 1.5,
        "fixture: the mean path emits {mean_feed}, above the slot fit {SLOT_FIT_FEED}"
    );

    // (b2) force budget: the force card's reading of the slot sample.
    let budget_n = rs_cam_core::tool_load::cut_force::sample_peak_force_n(
        &tool,
        &material,
        &slot(),
        fz_of(SLOT_FIT_FEED),
    )
    .expect("the slot sample has a force reading");
    let arms = Arms {
        power: None,
        deflection: None,
        force_budget: Some(ForceBudgetLimitInputs {
            line,
            cutter: cutter(&tool_cfg),
            budget_n,
        }),
    };
    let (feed, binding) = modulate(&samples, true, arms);
    let force = rs_cam_core::tool_load::cut_force::sample_peak_force_n(
        &tool,
        &material,
        &slot(),
        fz_of(feed),
    )
    .expect("the slot sample has a force reading");
    assert_eq!(binding, BindingConstraint::ForceBudget);
    assert!(
        force <= budget_n * (1.0 + 1e-9),
        "P20g (b2): the slot sample reads {force} N at the modulated feed {feed}; the budget \
         is {budget_n} N"
    );
    let (mean_feed, _) = modulate(&samples, false, arms);
    assert!(
        mean_feed > SLOT_FIT_FEED * 1.5,
        "fixture: the mean path emits {mean_feed}, above the slot fit {SLOT_FIT_FEED}"
    );
}

#[test]
fn a_uniform_move_keeps_its_feed_p20g() {
    // (c) every sample at one width: the mean equals the peak, so the side
    // table changes nothing (power, deflection and force budget together).
    let tool_cfg = end_mill();
    let tool = build_cutter(&tool_cfg).expect("a valid tool");
    let material = white_oak();
    let line = material.force_line().expect("white oak has a force line");
    let samples: Vec<SimulationCutSample> = (0..THIN_COUNT).map(|_| sample(2.0)).collect();
    let (power, _) = power_arms();
    for arms in [
        power,
        Arms {
            power: None,
            deflection: Some(DeflectionLimitInputs {
                line,
                cutter: cutter(&tool_cfg),
                compliance_mm_per_n: rs_cam_core::tool_load::deflection::tip_compliance_mm_per_n(
                    &tool, AP_MM,
                )
                .expect("the tool has a compliance"),
                max_tip_deflection_mm: 0.002,
            }),
            force_budget: None,
        },
        Arms {
            power: None,
            deflection: None,
            force_budget: Some(ForceBudgetLimitInputs {
                line,
                cutter: cutter(&tool_cfg),
                budget_n: 20.0,
            }),
        },
    ] {
        let (with, with_binding) = modulate(&samples, true, arms);
        let (without, without_binding) = modulate(&samples, false, arms);
        assert_eq!(with_binding, without_binding);
        assert!(
            (with - without).abs() <= 1e-6 * without,
            "P20g (c): a uniform move keeps its feed: {with} with the table, {without} without"
        );
    }
}

#[test]
fn the_modulator_and_the_gate_build_one_sample_engagement_p20g() {
    // (d) the side table entry is the gates' own engagement.
    let tool = build_cutter(&end_mill()).expect("a valid tool");
    for s in mixed_samples() {
        let entry = SampleEngagement::of_sample(&s, &tool);
        assert_eq!(
            entry.power,
            rs_cam_core::feeds::engagement::Engagement::from_sample(&s, &tool),
            "P20g (d): the power entry is the power gate's builder"
        );
        assert_eq!(
            entry.force,
            rs_cam_core::tool_load::deflection::sample_engagement(&tool, &s),
            "P20g (d): the force entry is the deflection gate's builder"
        );
        assert!(entry.power.is_some() && entry.force.is_some());
    }
}

/// P20g (e), then P32 (lead rule 2026-11-15): a load cap wins over the chip
/// MINIMUM floor, and it stops at the RUBBING floor of the move. This
/// fixture has its power cap (`SLOT_FIT_FEED`) below the rubbing floor, so
/// the move is held at the rubbing floor: it did not meet its cap, the
/// outcome counts it, and the gate still grades it over the power.
/// The case of a cap between the two floors (the cap wins, P20g) is the lib
/// test `a_load_cap_stops_at_the_rubbing_floor_p32`.
#[test]
fn a_load_cap_wins_over_the_chip_floor_p20g() {
    // The chip floor 0.03 x lift 1 (the slot sample) x 18000 x 2 = 1080
    // mm/min and the rubbing floor 0.025 x 18000 x 2 = 900 mm/min are both
    // above the power cap of the slot sample (SLOT_FIT_FEED).
    let tool = build_cutter(&end_mill()).expect("a valid tool");
    let line = white_oak()
        .force_line()
        .expect("white oak has a force line");
    let (arms, available_kw) = power_arms();
    let band = ChiploadBand::new(0.03, 1.0).expect("a valid band");
    let floor = band.min_mm_per_tooth * f64::from(RPM) * f64::from(FLUTES);
    let rubbing_floor =
        rs_cam_core::feeds::RUBBING_FLOOR_MM_TOOTH * f64::from(RPM) * f64::from(FLUTES);
    assert_eq!(
        band.rubbing_floor_mm_per_tooth(),
        rs_cam_core::feeds::RUBBING_FLOOR_MM_TOOTH,
        "fixture: the band minimum is above the repo rubbing floor"
    );
    assert!(
        rubbing_floor > SLOT_FIT_FEED * 1.2 && floor > rubbing_floor,
        "fixture: the cap {SLOT_FIT_FEED} is below the rubbing floor {rubbing_floor}, which is \
         below the chip floor {floor}"
    );

    let outcome = modulate_in_band(&mixed_samples(), true, arms, band);
    let (feed, binding) = *outcome.per_move.get(&1).expect("the cut is modulated");
    assert!(
        (feed - rubbing_floor).abs() <= 1e-9 * rubbing_floor,
        "P32: the load cap stops at the rubbing floor {rubbing_floor}, under the chip floor \
         {floor}; feed {feed}"
    );
    let graded = rs_cam_core::tool_load::power::sample_power_kw(&tool, &line, &slot(), feed)
        .expect("the slot sample has a power grade");
    assert!(
        graded > available_kw,
        "P32: the held move did not meet its cap: the gate grades the slot sample at {graded} \
         kW at the feed {feed}; the available power is {available_kw} kW"
    );
    assert_eq!(outcome.held.moves, 1, "P32: the held move is counted");
    assert_eq!(outcome.held.power, 1, "P32: it is held above the power cap");
    assert_eq!(outcome.held.deflection + outcome.held.force_budget, 0);
    assert_eq!(
        binding,
        BindingConstraint::PowerMax,
        "P20g (e): power binds"
    );
    assert!(
        outcome.notes.contains(&LOAD_CAP_BELOW_CHIP_FLOOR_NOTE),
        "P20g (e): the move below the chip floor carries the rub note; notes {:?}",
        outcome.notes
    );
}
