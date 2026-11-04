//! Feeds contract P11a, decision L5: the lateral force sums the cutting
//! force over the engaged edge with the helix lag, and deflection reads
//! the PEAK of that sum.
//!
//! Before P11a the force was one tooth at the peak chip over the full
//! depth, `ap (Ks fz sin(min(psi, pi/2)) + F_edge)`, with no helix. That
//! model says a slot costs 1.08x the force of a 0.2 D cut, and it
//! overstates a deep narrow cut about 4x against the mean engaged-edge
//! force (`planning/feeds_contract_2026-11-03/research_ae/ae_claims.py`,
//! line 73: 4.01 at 0.2 D, 1.165 in a slot; white oak, D 6, Z 2,
//! fz 0.05, an example chip).
//!
//! The arms read the pre-sim deflection predictor
//! (`predict_peak_deflection_um`), the door that reads an op. The tip
//! deflection is linear in the force at one depth, so a ratio of two
//! deflections at one depth is the ratio of the two forces. The fixture is
//! the rivmap100 end mill (D 6, Z 2, helix 30, stickout 45) in white oak.
//!
//! The depth of every arm is the depth where the helix wrap equals the
//! flute pitch, `ap* = (2 pi / Z) r / tan(helix)`
//! (`research_ae/ae_claims.py` line 63, 16.32 mm). At `ap*` each element
//! of the arc is cut by exactly one edge element at each rotation, so the
//! scalar engaged-edge sum is constant and equals its mean:
//! `F_mean = ap z / (2 pi) (Ks fz (1 - cos psi) + F_edge psi)`. The
//! resultant is at most the scalar sum, so the peak resultant at `ap*` is
//! at most `F_mean`.
//!
//! 1. A deep narrow cut (0.2 D at `ap*`): the predicted force is at most
//!    `F_mean`, about a quarter of the one-tooth force (red before: equal
//!    to the one-tooth force).
//! 2. A slot costs more force than a light stepover (0.2 D) at one depth,
//!    at an equal advance per tooth and at an equal peak chip. The bound is
//!    derived: the slot's peak resultant is at least the magnitude of its
//!    rotation mean, `ap z / (2 pi) (Ks fz pi/2 + 2 F_edge)`, and the light
//!    cut's peak is at most its `F_mean` (red before: 1.08).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::f64::consts::PI;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::predict::predict_peak_deflection_um;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;
const Z: u32 = 2;
const HELIX_DEG: f64 = 30.0;
const RPM: u32 = 18_000;
/// An example chip inside the Curti fit range (`ae_claims.py` header).
const FZ_MM: f64 = 0.05;

fn rivmap_end_mill() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.helix_deg = HELIX_DEG;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.shaft_diameter = 6.35;
    tool.set_shank_diameter(ToolFact::Stated(6.35));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(Z));
    tool
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

/// The depth where the helix wrap equals the flute pitch.
fn ap_star() -> f64 {
    (2.0 * PI / f64::from(Z)) * (D_MM / 2.0) / HELIX_DEG.to_radians().tan()
}

fn psi(ae: f64) -> f64 {
    (1.0 - ae / (D_MM / 2.0)).clamp(-1.0, 1.0).acos()
}

/// The predicted tip deflection (µm) of a pocket that cuts `(ap, ae)` at
/// the advance per tooth `fz`.
fn predicted_um(ap: f64, ae: f64, fz: f64) -> f64 {
    let op = OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(30.0),
        depth_per_pass: Some(ap),
        stepover: Some(ae),
        feed_rate: Some(fz * f64::from(RPM) * f64::from(Z)),
        plunge_rate: Some(500.0),
        spindle_rpm: Some(RPM),
        ..PocketConfig::default()
    });
    predict_peak_deflection_um(&op, &rivmap_end_mill(), &oak(), &MachineProfile::default())
        .expect("white oak and a stated stickout give a figure")
        .predicted_um
}

/// The force line `(Ks, F_edge)` of white oak.
fn line() -> (f64, f64) {
    let l = oak().force_line().expect("white oak has a force line");
    (l.ks_n_per_mm2(), l.f_edge_n_per_mm())
}

/// The retired one-tooth peak model.
fn one_tooth_n(ap: f64, ae: f64, fz: f64) -> f64 {
    let (ks, fe) = line();
    ap * (ks * fz * psi(ae).min(PI / 2.0).sin() + fe)
}

/// The mean scalar engaged-edge force (N).
fn mean_n(ap: f64, ae: f64, fz: f64) -> f64 {
    let (ks, fe) = line();
    let p = psi(ae);
    ap * f64::from(Z) / (2.0 * PI) * (ks * fz * (1.0 - p.cos()) + fe * p)
}

/// The magnitude of the rotation mean of the slot's force vector (N).
fn slot_mean_resultant_n(ap: f64, fz: f64) -> f64 {
    let (ks, fe) = line();
    ap * f64::from(Z) / (2.0 * PI) * (ks * fz * PI / 2.0 + 2.0 * fe)
}

#[test]
fn the_fixture_reads_the_reference_line_p11a() {
    // `ae_claims.py` line 73 asserts 4.01 +/- 0.01 for this line and chip.
    let ap = ap_star();
    let ae = 0.2 * D_MM;
    let ratio = one_tooth_n(ap, ae, FZ_MM) / mean_n(ap, ae, FZ_MM);
    assert!(
        (ratio - 4.01).abs() < 0.01,
        "the white oak line gives the research ratio: {ratio}"
    );
    assert!((ap - 16.32).abs() < 0.01, "ap* is 16.32 mm: {ap}");
}

#[test]
fn a_deep_narrow_cut_reads_the_engaged_edge_not_one_tooth_p11a() {
    let ap = ap_star();
    let ae = 0.2 * D_MM;
    // The op door gives a deflection, not a force. At one depth the
    // deflection is linear in the force, so the ratio of the 0.2 D
    // deflection to the slot deflection is the ratio of the two forces.
    let predicted = predicted_um(ap, ae, FZ_MM);
    let one_tooth = one_tooth_n(ap, ae, FZ_MM);
    let mean = mean_n(ap, ae, FZ_MM);
    let slot_pred = predicted_um(ap, D_MM, FZ_MM);
    let ratio = predicted / slot_pred;
    let one_tooth_ratio = one_tooth / one_tooth_n(ap, D_MM, FZ_MM);
    // The engaged-edge bound: light peak <= F_mean(light); slot peak >=
    // the slot's mean resultant.
    let bound = mean / slot_mean_resultant_n(ap, FZ_MM);
    assert!(
        ratio <= bound * (1.0 + 1e-6),
        "L5: the 0.2 D / slot force ratio at ap* is {ratio}; the engaged-edge model bounds \
         it at {bound}; the one-tooth model gives {one_tooth_ratio}"
    );
    assert!(
        one_tooth_ratio > 2.0 * bound,
        "non-vacuity: the one-tooth ratio {one_tooth_ratio} is far above the bound {bound}"
    );
}

#[test]
fn a_slot_costs_more_force_than_a_light_stepover_p11a() {
    let ap = ap_star();
    let light = 0.2 * D_MM;
    // Equal advance per tooth.
    let at_equal_fz = predicted_um(ap, D_MM, FZ_MM) / predicted_um(ap, light, FZ_MM);
    let bound_fz = slot_mean_resultant_n(ap, FZ_MM) / mean_n(ap, light, FZ_MM);
    assert!(
        at_equal_fz >= bound_fz * (1.0 - 1e-6),
        "L5: slot / 0.2 D at one fz is {at_equal_fz}; the engaged-edge model gives at least \
         {bound_fz}; the one-tooth model gives {}",
        one_tooth_n(ap, D_MM, FZ_MM) / one_tooth_n(ap, light, FZ_MM)
    );
    // Equal peak chip: the light cut runs fz / sin(psi) (Garr thinning).
    let fz_light = FZ_MM / psi(light).sin();
    let at_equal_chip = predicted_um(ap, D_MM, FZ_MM) / predicted_um(ap, light, fz_light);
    let bound_chip = slot_mean_resultant_n(ap, FZ_MM) / mean_n(ap, light, fz_light);
    assert!(
        at_equal_chip >= bound_chip * (1.0 - 1e-6),
        "L5: slot / 0.2 D at one peak chip is {at_equal_chip}; the engaged-edge model gives \
         at least {bound_chip}"
    );
    assert!(
        bound_chip > 2.0,
        "the derived bound is far above 1.08: {bound_chip}"
    );
}
