//! Feeds contract P20d: the power of a cut reads its true engaged section.
//!
//! The acceptance table (`planning/feeds_contract_2026-11-03/acceptance/
//! wanaka_21.md`, a25ff317d) found two defects. A drill (idx 3, 4) printed
//! 0.00005 kW from a fake side cut (ae 0.02 x ap 0.05), and a 60 deg V-bit
//! Trace (idx 10, 13) printed the power of a rectangle 1.9x and 4.0x over
//! the triangle. The one model is `P = [Ks A vf + F_edge Z n J] / 6e7`
//! (`feeds::section`):
//!
//! - a straight plunge of a flat end mill: `A = pi D^2 / 4`, `J = pi D^2 / 4`;
//! - a V slot of depth `h`, half angle `a`: `A = h^2 tan a`,
//!   `J = pi R^2 / (2 sin a)`, `R = h tan a`;
//! - a plunge of a tip with no axial section: no power, never 0.0.
//!
//! Each expected value is computed here from the inputs and the material's
//! force line, not read from the engine.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::f64::consts::PI;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{DrillConfig, TraceConfig};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::section::{AxialSectionRefusal, EngagedSection, SectionKind};
use rs_cam_core::feeds::{PowerUnmodeled, ToolGeometryHint, power_at_operating_point};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The wanaka drill and V-bit cut: 2286 mm/min at 18000 rpm, 2 flutes.
const FEED_MM_MIN: f64 = 2286.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn tool(kind: ToolType, diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = diameter;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
        t.set_v_tip_diameter(ToolFact::Stated(0.0));
    }
    t
}

fn drill() -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        floor: rs_cam_core::compute::floor::Floor::Top { depth: 8.0 },
        feed_rate: Some(FEED_MM_MIN),
        spindle_rpm: Some(RPM),
        ..DrillConfig::default()
    })
}

fn trace(depth: f64) -> OperationConfig {
    let mut op = OperationConfig::Trace(TraceConfig {
        depth: Some(depth),
        depth_per_pass: Some(depth),
        ..TraceConfig::default()
    });
    op.set_feed_rate(FEED_MM_MIN);
    op.set_spindle_rpm(Some(RPM));
    op
}

/// `(grain x Ks, grain x F_edge)` of white oak.
fn oak_line() -> (f64, f64) {
    let line = oak().force_line().expect("white oak has a force line");
    let g = line.grain_factor();
    (g * line.ks_n_per_mm2(), g * line.f_edge_n_per_mm())
}

/// The plunge of a 6 mm 2F flat end mill (wanaka idx 3, 4) is
/// `Ks x pi D^2 / 4 x vf / 6e7 + F_edge x Z x n x pi D^2 / 4 / 6e7`. The
/// figure is advisory, and its chip is the axial chip `vf / (n Z)`.
#[test]
fn the_plunge_power_is_ks_times_the_axial_circle_p20d() {
    let d = 6.0;
    let (ks, f_edge) = oak_line();
    let area = PI * d * d / 4.0;
    let n = f64::from(RPM);
    let z = f64::from(FLUTES);
    let want = ks * area * FEED_MM_MIN / 60_000_000.0 + f_edge * z * n * area / 60_000_000.0;
    let figure = power_at_operating_point(
        &drill(),
        &tool(ToolType::EndMill, d),
        &oak(),
        &MachineProfile::generic_wood_router(),
        None,
    )
    .expect("a flat end mill plunge has the axial section");
    assert!(
        (figure.required_kw - want).abs() <= 1e-12 * want,
        "plunge power {} kW, want {want} kW",
        figure.required_kw
    );
    assert_eq!(figure.section, SectionKind::AxialCircle);
    assert!(!figure.section.gates(), "the axial figure is advisory");
    let axial_chip = FEED_MM_MIN / (n * z);
    assert!(
        (figure.force.mean_chip_mm - axial_chip).abs() <= 1e-15,
        "the evaluated chip is the axial chip: {} vs {axial_chip}",
        figure.force.mean_chip_mm
    );
    // Non-vacuity against the defect: the fake side cut gave 0.00005 kW.
    assert!(figure.required_kw > 0.1, "{}", figure.required_kw);
}

/// The V slot (wanaka idx 10, 13: 60 deg, h 2.60 and 0.60) is the triangle
/// `Ks h^2 tan a vf / 6e7 + F_edge Z n pi R^2 / (2 sin a) / 6e7`.
#[test]
fn the_v_groove_power_is_the_triangle_p20d() {
    let d = 6.35;
    let (ks, f_edge) = oak_line();
    let a = 30.0_f64.to_radians();
    let n = f64::from(RPM);
    let z = f64::from(FLUTES);
    for h in [2.6, 0.6] {
        let r = h * a.tan();
        let want = ks * h * h * a.tan() * FEED_MM_MIN / 60_000_000.0
            + f_edge * z * n * PI * r * r / (2.0 * a.sin()) / 60_000_000.0;
        let figure = power_at_operating_point(
            &trace(h),
            &tool(ToolType::VBit, d),
            &oak(),
            &MachineProfile::generic_wood_router(),
            None,
        )
        .expect("a V-bit trace has a power figure");
        assert!(
            (figure.required_kw - want).abs() <= 1e-12 * want,
            "h {h}: V-groove power {} kW, want {want} kW",
            figure.required_kw
        );
        assert_eq!(figure.section, SectionKind::VGroove);
        assert!(figure.section.gates());
    }
}

/// A plunge of a tip with no axial section gives no section and no power
/// (a typed refusal), never a 0.0 figure.
#[test]
fn a_section_with_no_model_gives_none_never_zero_p20d() {
    for hint in [
        ToolGeometryHint::Ball,
        ToolGeometryHint::Bull { corner_radius: 1.0 },
        ToolGeometryHint::VBit {
            included_angle: 60.0,
            tip_diameter: 0.0,
        },
        ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 7.0,
        },
    ] {
        assert_eq!(
            EngagedSection::axial_plunge(hint, 6.0, 0.0),
            Err(AxialSectionRefusal::TipShapeNotModelled),
            "{hint:?}"
        );
    }
    let refusal = power_at_operating_point(
        &drill(),
        &tool(ToolType::BallNose, 6.0),
        &oak(),
        &MachineProfile::generic_wood_router(),
        None,
    )
    .expect_err("a ball plunge has no power model");
    assert_eq!(refusal, PowerUnmodeled::AxialTipNotModelled);
}
