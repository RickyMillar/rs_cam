//! Feeds contract P16 item 7 (D6): the modulator does not raise a feed past
//! the machine force budget, and it reads the one rule the depth gate reads.
//!
//! # The claim
//!
//! The depth gate applies the force budget only where
//! `RigidityProfile::depth_cap_rule(family, role)` is
//! `Some(DepthCapRule::ForceBudget(_))`. The session builds the modulator's
//! force-budget inputs through `ForceBudgetScope::of` from that same rule.
//!
//! 1. A cell where the rule is `ForceBudget` (Pocket, Roughing, the default
//!    hobby machine): the limit binds a full-slot move, the move carries
//!    `BindingConstraint::ForceBudget`, and the emitted feed is the feed of
//!    `Engagement::fz_cap_for_peak_force` at the budget of the rule.
//! 2. A cell where the rule is not `ForceBudget` (Adaptive, Roughing: the
//!    factor rule): the scope gives no inputs, no move carries
//!    `ForceBudget`, and the same move runs faster than the budget feed.
//! 3. The force-budget rule with no force line: no limit, and the scope
//!    gives the summary note "force budget: no force line".
//!
//! The budget is the rule's own value (`hobby_budget`, derived from the
//! printed hobby rows). The depth of the fixture is found here: the
//! deepest depth, halving from the tool's flute length, where the edge term
//! alone stays under the budget, so the cap is a solved feed, not a pin.
//! The band and the machine ceiling are set as multiples of that feed, so
//! only the force budget can bind in cell 1.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, FORCE_BUDGET_NO_FORCE_LINE_NOTE, ForceBudgetScope,
    ModulationContext, ModulationCutter, ModulationStrategy, PerMoveEngagement, RadialThinning,
    adaptive_feed_modulate,
};
use rs_cam_core::feeds::engagement::{EngagedDiameter, Engagement};
use rs_cam_core::feeds::envelope::CutPoint;
use rs_cam_core::feeds::{OperationFamily, PassRole, ToolGeometryHint};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::machine::{DepthCapRule, MachineProfile};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// The fixture cutter: a 6 mm two-flute flat end mill, 30 degree helix.
const D_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const HELIX_DEG: f64 = 30.0;
const RPM: f64 = 18_000.0;
/// The flute length the depth search starts from (the fixture tool).
const FLUTE_LENGTH_MM: f64 = 22.0;

fn cutter() -> ModulationCutter {
    ModulationCutter {
        geometry: ToolGeometryHint::Flat,
        diameter_mm: D_MM,
        max_cut_diameter_mm: D_MM,
        helix_deg: HELIX_DEG,
    }
}

fn line() -> rs_cam_core::material::force_line::ForceLine {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
    .force_line()
    .expect("generic hardwood has a printed force line")
}

/// The full-slot engagement at `ap_mm`, built the way the modulator builds
/// it (`ModulationCutter::engagement`: width `woc x d_eng`, `woc = 1`).
fn slot_engagement(ap_mm: f64) -> Engagement {
    let hint = ToolGeometryHint::Flat;
    let d_eng = EngagedDiameter::of_hint(hint, D_MM, D_MM, ap_mm);
    Engagement::from_cut(
        CutPoint {
            ap_mm,
            ae_mm: d_eng.mm(),
        },
        hint,
        d_eng,
        Some(HELIX_DEG),
    )
}

/// Long straight cuts, so the kinematic reach does not bind.
fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 0..n_cuts {
        let x = (i + 1) as f64 * 1000.0;
        tp.feed_to_with_intent(P3::new(x, 0.0, 0.0), feed_mm_min, MoveIntent::ClearingCut);
    }
    tp
}

fn engagements(tp: &Toolpath, ap_mm: f64) -> Vec<PerMoveEngagement> {
    tp.moves
        .iter()
        .map(|m| {
            if matches!(m.move_type, MoveType::Rapid) {
                PerMoveEngagement::default()
            } else {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: ap_mm / FLUTE_LENGTH_MM,
                    axial_doc_mm: ap_mm,
                    thinning_lift: None,
                }
            }
        })
        .collect()
}

fn cutting_ceiling(mm_min: f64) -> rs_cam_core::machine::CuttingCeilingMmMin {
    MachineProfile {
        max_feed_mm_min: mm_min,
        max_cutting_feed_mm_min: Some(mm_min),
        ..MachineProfile::default()
    }
    .cutting_ceiling()
}

#[test]
fn the_modulator_reads_the_depth_gate_force_budget_rule_p16() {
    let machine = MachineProfile::default();
    let pocket_rule = machine
        .rigidity
        .depth_cap_rule(OperationFamily::Pocket, PassRole::Roughing);
    let adaptive_rule = machine
        .rigidity
        .depth_cap_rule(OperationFamily::Adaptive, PassRole::Roughing);
    let Some(DepthCapRule::ForceBudget(budget_n)) = pocket_rule else {
        panic!("Pocket roughing must carry the force-budget rule: {pocket_rule:?}");
    };
    assert!(
        matches!(adaptive_rule, Some(DepthCapRule::Factor(_))),
        "Adaptive roughing must carry the factor rule: {adaptive_rule:?}"
    );

    // The scope of the two cells, through the one constructor.
    let pocket_scope = ForceBudgetScope::of(pocket_rule, Some(line()), cutter());
    let adaptive_scope = ForceBudgetScope::of(adaptive_rule, Some(line()), cutter());
    let inputs = pocket_scope
        .inputs()
        .expect("the force-budget rule with a line gives the limit");
    assert_eq!(inputs.budget_n.to_bits(), budget_n.to_bits());
    assert_eq!(adaptive_scope.inputs(), None);
    assert_eq!(adaptive_scope.note(), None);
    assert_eq!(pocket_scope.note(), None);

    // Cell 3: the force-budget rule with no force line.
    let no_line = ForceBudgetScope::of(pocket_rule, None, cutter());
    assert_eq!(no_line.inputs(), None);
    assert_eq!(no_line.note(), Some(FORCE_BUDGET_NO_FORCE_LINE_NOTE));
    assert_eq!(
        FORCE_BUDGET_NO_FORCE_LINE_NOTE,
        "force budget: no force line"
    );

    // The fixture depth: the deepest depth (halving from the flute length)
    // where the edge term alone stays under the budget.
    let mut ap_mm = FLUTE_LENGTH_MM;
    let fz_cap = loop {
        match slot_engagement(ap_mm).fz_cap_for_peak_force(line(), FLUTES, budget_n) {
            Ok(fz) => break fz,
            Err(_) if ap_mm > 0.05 => ap_mm /= 2.0,
            Err(e) => panic!("no depth gives a solved force-budget cap: {e:?}"),
        }
    };
    let cap_feed = fz_cap * RPM * f64::from(FLUTES);
    assert!(cap_feed.is_finite() && cap_feed > 0.0);

    let k = MachineKinematics::shapeoko_xxl_stock();
    let ctx_for = |scope: ForceBudgetScope| ModulationContext {
        spindle_rpm: RPM,
        flute_count: FLUTES,
        // Well over the budget feed: the ceiling does not bind.
        cutting_ceiling: cutting_ceiling(cap_feed * 4.0),
        rapid_feed_mm_min: cap_feed * 8.0,
        // The band max well over and the band min well under the budget
        // chip: the chipload cap and the floor do not bind.
        chipload_band: Some(ChiploadBand::new(fz_cap / 4.0, fz_cap * 3.0).expect("a valid band")),
        radial_thinning: RadialThinning::Applied,
        kinematics: &k,
        strategy: ModulationStrategy::ConstrainedMax,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: scope.inputs(),
        sample_engagements: &[],
        nominal_axial_doc_mm: ap_mm,
        plunge_rate_mm_min: f64::INFINITY,
    };
    let commanded = cap_feed * 2.0;

    // Cell 1: the rule is the force budget, and the limit binds.
    let mut tp = straight_toolpath(3, commanded);
    let eng = engagements(&tp, ap_mm);
    let outcome =
        adaptive_feed_modulate(&mut tp, &eng, &ctx_for(pocket_scope)).expect("a valid modulation");
    assert!(!outcome.per_move.is_empty(), "non-vacuity: moves visited");
    for (i, (feed, binding)) in &outcome.per_move {
        assert_eq!(
            *binding,
            BindingConstraint::ForceBudget,
            "move {i}: the force budget must bind (feed {feed}, budget feed {cap_feed})"
        );
        assert!(
            (feed - cap_feed).abs() <= 1e-9 * cap_feed,
            "move {i}: feed {feed} must be the budget feed {cap_feed}"
        );
    }
    assert_eq!(BindingConstraint::ForceBudget.label(), "force-budget");

    // Cell 2: the rule is not the force budget, and the limit is absent.
    let mut tp = straight_toolpath(3, commanded);
    let outcome = adaptive_feed_modulate(&mut tp, &eng, &ctx_for(adaptive_scope))
        .expect("a valid modulation");
    assert!(!outcome.per_move.is_empty(), "non-vacuity: moves visited");
    for (i, (feed, binding)) in &outcome.per_move {
        assert_ne!(
            *binding,
            BindingConstraint::ForceBudget,
            "move {i}: no force-budget limit outside its rule"
        );
        assert!(
            *feed > cap_feed,
            "move {i}: without the limit the feed {feed} runs past the budget feed {cap_feed}"
        );
    }
}
