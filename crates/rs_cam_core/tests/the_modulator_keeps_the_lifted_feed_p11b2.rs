//! Feeds contract P11b2 (`planning/feeds_contract_2026-11-03/STATUS.md` item
//! 3a; a consequence of rulings L1 and L6): the feed modulator caps the PEAK
//! chip at the band maximum, so it keeps the radial chip-thinning lift that
//! Suggest applies.
//!
//! P11b made Suggest size the feed so the peak chip (`fz sin psi`, Garr; the
//! one law `feeds::geometry::peak_chip_thinning_lift`) is the band chip.
//! Before P11b2 the constrained-max modulator capped the ADVANCE per tooth at
//! `band.max x rpm x Z`, so on a move where that cap bound the G-code lost
//! the lift.
//!
//! The fixture is the P11b lift fixture (D 6, Z 2, white oak roughing pocket,
//! stepover 0.1 D, depth 1 mm, a machine with no power or feed limit; test
//! values, not a machine claim). One long straight cut is commanded at the
//! Suggest feed at the Suggest engagement (`radial_woc_fraction = ae / D`).
//! No deflection or power cap is set and the cutting ceiling is far above the
//! feed, so only the chipload cap can bind.
//!
//! Arms (red before P11b2: the emitted feed is the un-lifted band max):
//!
//! 1. The emitted feed is at or above the Suggest feed (the lift is kept).
//! 2. The chipload cap binds at `band.max x lift x rpm x Z`.
//!
//! Both arms run twice: with the measured lift (`PerMoveEngagement::
//! thinning_lift`, the production path) and with none (the modelled width).
//! The red run at HEAD had the modelled arm only (the field did not exist);
//! log `p11b2/red_at_head.log` in the agent scratchpad.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    RadialThinning, adaptive_feed_modulate,
};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const D_MM: f64 = 6.0;
const AP_MM: f64 = 1.0;
const AE_MM: f64 = 0.1 * D_MM;
const FLUTES: u32 = 2;
/// One long straight cut, so the kinematic reach is not the binding cap.
const CUT_LENGTH_MM: f64 = 5000.0;

fn end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = 30.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(FLUTES));
    tool
}

fn pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(12.0),
        depth_per_pass: Some(AP_MM),
        stepover: Some(AE_MM),
        feed_rate: Some(1800.0),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

/// No spindle-power and no feed-ceiling limit binds (test values).
fn open_machine() -> MachineProfile {
    MachineProfile {
        power: PowerModel::ConstantPower { power_kw: 50.0 },
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    }
}

fn suggest() -> FeedsResult {
    let tool = end_mill();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let machine = open_machine();
    let input = FeedsInput::of_operation(
        &pocket(),
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

/// The modulator band of the Suggest result: the band, else the point.
fn band_of(r: &FeedsResult) -> ChiploadBand {
    if let Some(b) = r.chipload_bounds {
        ChiploadBand::new(b.min_mm_per_tooth, b.max_mm_per_tooth).expect("a valid band")
    } else {
        ChiploadBand::point(
            r.chipload_point_mm
                .expect("fixture: a band or a printed point"),
        )
        .expect("a valid point")
    }
}

#[test]
fn the_modulator_keeps_the_lifted_feed_at_the_suggest_engagement_p11b2() {
    let r = suggest();
    assert!(
        r.derates.power_limit >= 1.0 && r.derates.feed_clamp >= 1.0,
        "fixture: no power or feed-ceiling limit binds ({:?})",
        r.warnings
    );
    let band = band_of(&r);
    let lift = rs_cam_core::feeds::geometry::peak_chip_thinning_lift(AE_MM, D_MM);
    assert!(lift > 1.0, "fixture: a light cut lifts the feed ({lift})");
    let un_lifted_cap = band.max_mm_per_tooth * r.rpm * f64::from(FLUTES);
    assert!(
        r.feed_rate_mm_min > un_lifted_cap,
        "fixture: the Suggest feed {} is above the un-lifted chipload cap {un_lifted_cap}",
        r.feed_rate_mm_min
    );

    // The measured lift (the production path: the minimum of the gate's
    // per-sample lifts) and the modelled one (`radial_woc_fraction`, the
    // synthetic path). Both arms keep the lift.
    for measured in [Some(lift), None] {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to_with_intent(
            P3::new(CUT_LENGTH_MM, 0.0, 0.0),
            r.feed_rate_mm_min,
            MoveIntent::ClearingCut,
        );
        let engagements: Vec<PerMoveEngagement> = tp
            .moves
            .iter()
            .map(|m| {
                if matches!(m.move_type, MoveType::Rapid) {
                    PerMoveEngagement::default()
                } else {
                    PerMoveEngagement {
                        radial_woc_fraction: AE_MM / D_MM,
                        axial_doc_fraction: AP_MM / 25.0,
                        axial_doc_mm: AP_MM,
                        thinning_lift: measured,
                    }
                }
            })
            .collect();
        let kinematics = MachineKinematics::default();
        let ctx = ModulationContext {
            spindle_rpm: r.rpm,
            flute_count: FLUTES,
            cutting_ceiling: open_machine().cutting_ceiling(),
            rapid_feed_mm_min: 1.0e7,
            chipload_band: Some(band),
            radial_thinning: RadialThinning::Applied,
            kinematics: &kinematics,
            strategy: ModulationStrategy::ConstrainedMax,
            mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
            feed_scale: 1.0,
            deflection_inputs: None,
            power_inputs: None,
            force_budget_inputs: None,
            sample_engagements: &[],
            nominal_axial_doc_mm: AP_MM,
            plunge_rate_mm_min: 500.0,
        };
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).expect("a valid context");
        let (feed, binding) = *outcome
            .per_move
            .values()
            .next()
            .expect("the cut is modulated");
        assert!(
            feed >= r.feed_rate_mm_min * (1.0 - 1e-9),
            "P11b2 ({measured:?}): the modulator keeps the lifted Suggest feed {}; it emitted {feed} ({binding:?})",
            r.feed_rate_mm_min
        );
        let want = un_lifted_cap * lift;
        assert_eq!(
            binding,
            BindingConstraint::ChiploadMax,
            "P11b2: only the chipload cap binds here (emitted {feed}, want {want})"
        );
        assert!(
            (feed - want).abs() <= 1e-6 * want,
            "P11b2: the chipload cap is band.max x lift x rpm x Z = {want}; emitted {feed}"
        );
    }
}
