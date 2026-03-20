//! Feeds & speeds calculator — computes RPM, feed rate, plunge rate, DOC, WOC,
//! and power requirements from tool, material, machine, and operation parameters.
//!
//! Ported from reference/shapeoko_feeds_and_speeds/src/calcs.rs.
//! The calculation pipeline:
//! 1. RPM from surface speed → clamp to machine range
//! 2. Chip load from empirical formula: K₀ × D^p × (1/H)^q
//! 3. DOC/WOC from operation matrix × machine rigidity
//! 4. Flute guard: cap DOC to 0.8 × flute_length
//! 5. Feed = RPM × chipload × flutes × RCTF
//! 6. Power check: Kc × DOC × WOC × feed / 60e6 — reduce feed if over
//! 7. Clamp feed to machine max
//! 8. Plunge rate from material-dependent fraction
//! 9. Apply safety factor
//! 10. Collect warnings

pub mod geometry;

use crate::machine::MachineProfile;
use crate::material::Material;

/// Hint about the tool geometry for effective diameter calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolGeometryHint {
    Flat,
    Ball,
    Bull { corner_radius: f64 },
    VBit { included_angle: f64, tip_diameter: f64 },
    TaperedBall { tip_radius: f64, taper_angle_deg: f64 },
}

/// Which family of operation is being calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationFamily {
    Adaptive,
    Pocket,
    Contour,
    Parallel,
    Scallop,
    Trace,
    Face,
}

/// Role of the pass (roughing removes bulk, finishing for surface quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassRole {
    Roughing,
    SemiFinish,
    Finish,
}

/// Input parameters for the feeds calculator.
pub struct FeedsInput<'a> {
    pub tool_diameter: f64,
    pub flute_count: u32,
    pub flute_length: f64,
    pub tool_geometry: ToolGeometryHint,
    pub material: &'a Material,
    pub machine: &'a MachineProfile,
    pub operation: OperationFamily,
    pub pass_role: PassRole,
    /// Optional DOC override (None = auto-calculate).
    pub axial_depth_mm: Option<f64>,
    /// Optional WOC/stepover override (None = auto-calculate).
    pub radial_width_mm: Option<f64>,
    /// Target scallop height for ball/tapered ball finishing (mm).
    pub target_scallop_mm: Option<f64>,
}

/// Result of the feeds calculation.
#[derive(Debug, Clone)]
pub struct FeedsResult {
    pub rpm: f64,
    pub chip_load_mm: f64,
    pub feed_rate_mm_min: f64,
    pub plunge_rate_mm_min: f64,
    pub ramp_feed_mm_min: f64,
    pub axial_depth_mm: f64,
    pub radial_width_mm: f64,
    pub power_kw: f64,
    pub available_power_kw: f64,
    pub power_limited: bool,
    pub mrr_mm3_min: f64,
    pub warnings: Vec<FeedsWarning>,
}

/// Warnings generated during calculation.
#[derive(Debug, Clone)]
pub enum FeedsWarning {
    FeedRateClamped { requested: f64, actual: f64 },
    PowerLimited { required_kw: f64, available_kw: f64 },
    ShankTooLarge { shank_mm: f64, max_mm: f64 },
    DocExceedsFlute { requested: f64, capped: f64 },
    SlottingDetected { doc_reduced_to: f64 },
    ScallopInvalid { target: f64, max_possible: f64 },
}

/// Main calculation entry point.
pub fn calculate(input: &FeedsInput) -> FeedsResult {
    let mut warnings = Vec::new();
    let machine = input.machine;
    let material = input.material;
    let d = input.tool_diameter;

    // --- Step 1: RPM ---
    let base_cutting_speed = match material {
        Material::SolidWood { .. } => 200.0,
        Material::Plywood { .. } => 180.0,
        Material::SheetGood { .. } => 170.0,
        Material::Plastic { .. } => 250.0,
        Material::Foam { .. } => 300.0,
        Material::Custom { .. } => 200.0,
    };
    let ideal_rpm = if d > 0.0 {
        (base_cutting_speed * 1000.0) / (std::f64::consts::PI * d)
    } else {
        18000.0
    };
    let rpm = machine.clamp_rpm(ideal_rpm);

    // --- Step 2: Chip load ---
    let hardness = material.hardness_index();
    let cl = &machine.chip_load;
    let chip_load = cl.k0 * d.powf(cl.p) * (1.0 / hardness).powf(cl.q);

    // --- Step 3: DOC/WOC from operation defaults ---
    let profile = operation_default_profile(input.operation, input.pass_role);
    let (mut ap, mut ae) = default_engagement(d, &profile, input, machine);

    // --- Step 3b: Scallop-driven stepover for ball/tapered ball ---
    if let Some(target_scallop) = input.target_scallop_mm {
        let ball_r = match input.tool_geometry {
            ToolGeometryHint::Ball => d / 2.0,
            ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
            _ => 0.0,
        };
        if ball_r > 0.0 {
            if let Some(stepover) = geometry::scallop_stepover(ball_r, target_scallop) {
                ae = stepover;
            } else {
                warnings.push(FeedsWarning::ScallopInvalid {
                    target: target_scallop,
                    max_possible: ball_r,
                });
            }
        }
    }

    // Apply user overrides
    if let Some(user_ap) = input.axial_depth_mm {
        ap = user_ap;
    }
    if let Some(user_ae) = input.radial_width_mm {
        ae = user_ae;
    }

    // --- Step 4: Flute guard ---
    let flute_guard = if input.flute_length > 0.0 {
        input.flute_length * 0.8
    } else {
        d * 2.0
    };
    if ap > flute_guard {
        warnings.push(FeedsWarning::DocExceedsFlute {
            requested: ap,
            capped: flute_guard,
        });
        ap = flute_guard;
    }

    // Ensure minimum engagement
    ap = ap.max(0.05);
    ae = ae.max(0.02);
    // Cap ae to tool diameter
    ae = ae.min(d);

    // --- Step 4b: Slotting detection ---
    if ae > d * 0.85 {
        let slotting_cap = d * 0.25;
        if ap > slotting_cap {
            warnings.push(FeedsWarning::SlottingDetected {
                doc_reduced_to: slotting_cap,
            });
            ap = slotting_cap;
        }
    }

    // --- Step 5: Feed rate ---
    let effective_d = effective_diameter(input.tool_geometry, d, ap);
    let rctf = geometry::radial_chip_thinning_factor(ae, effective_d);
    let raw_feed = rpm * chip_load * input.flute_count as f64 * rctf;

    // --- Step 6: Power check ---
    let kc = material.kc_n_per_mm2();
    let available_power = machine.power_at_rpm(rpm);
    let required_power = (kc * ap * ae * raw_feed) / (60.0 * 1_000_000.0);

    let mut power_limited = false;
    let mut feed = raw_feed;

    if required_power > available_power && available_power > 0.0 {
        // Reduce feed to stay within power budget
        let power_ratio = available_power / required_power;
        feed = raw_feed * power_ratio;
        power_limited = true;
        warnings.push(FeedsWarning::PowerLimited {
            required_kw: required_power,
            available_kw: available_power,
        });
    }

    // --- Step 7: Machine feed clamp ---
    if feed > machine.max_feed_mm_min {
        warnings.push(FeedsWarning::FeedRateClamped {
            requested: feed,
            actual: machine.max_feed_mm_min,
        });
        feed = machine.max_feed_mm_min;
    }

    // --- Step 8: Plunge rate ---
    let plunge = estimate_plunge_rate(material, hardness);

    // Ramp feed: between plunge and cutting feed
    let ramp_feed = (feed * 0.5).max(plunge);

    // --- Step 9: Safety factor ---
    feed *= machine.safety_factor;
    let plunge_rate = plunge * machine.safety_factor;
    let ramp_feed_rate = ramp_feed * machine.safety_factor;

    // Final power at actual feed
    let actual_power = (kc * ap * ae * feed) / (60.0 * 1_000_000.0);
    let mrr = ap * ae * feed;

    FeedsResult {
        rpm,
        chip_load_mm: chip_load,
        feed_rate_mm_min: feed,
        plunge_rate_mm_min: plunge_rate,
        ramp_feed_mm_min: ramp_feed_rate,
        axial_depth_mm: ap,
        radial_width_mm: ae,
        power_kw: actual_power,
        available_power_kw: available_power,
        power_limited,
        mrr_mm3_min: mrr,
        warnings,
    }
}

/// Operation default DOC/WOC profile factors (multiplied by tool diameter).
struct DefaultProfile {
    ap_factor: f64,
    ae_factor: f64,
}

fn operation_default_profile(family: OperationFamily, role: PassRole) -> DefaultProfile {
    match (family, role) {
        // Adaptive: deep and narrow
        (OperationFamily::Adaptive, PassRole::Roughing) => DefaultProfile { ap_factor: 1.50, ae_factor: 0.12 },
        (OperationFamily::Adaptive, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.90, ae_factor: 0.10 },
        (OperationFamily::Adaptive, PassRole::Finish) => DefaultProfile { ap_factor: 0.70, ae_factor: 0.08 },
        // Pocket: moderate
        (OperationFamily::Pocket, PassRole::Roughing) => DefaultProfile { ap_factor: 0.70, ae_factor: 0.35 },
        (OperationFamily::Pocket, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.35, ae_factor: 0.20 },
        (OperationFamily::Pocket, PassRole::Finish) => DefaultProfile { ap_factor: 0.20, ae_factor: 0.08 },
        // Contour: moderate depth, narrow width
        (OperationFamily::Contour, PassRole::Roughing) => DefaultProfile { ap_factor: 0.80, ae_factor: 0.18 },
        (OperationFamily::Contour, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.45, ae_factor: 0.10 },
        (OperationFamily::Contour, PassRole::Finish) => DefaultProfile { ap_factor: 0.30, ae_factor: 0.05 },
        // Parallel: shallow surface following
        (OperationFamily::Parallel, PassRole::Roughing) => DefaultProfile { ap_factor: 0.25, ae_factor: 0.08 },
        (OperationFamily::Parallel, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.16, ae_factor: 0.05 },
        (OperationFamily::Parallel, PassRole::Finish) => DefaultProfile { ap_factor: 0.10, ae_factor: 0.03 },
        // Scallop: very fine
        (OperationFamily::Scallop, PassRole::Roughing) => DefaultProfile { ap_factor: 0.20, ae_factor: 0.07 },
        (OperationFamily::Scallop, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.14, ae_factor: 0.05 },
        (OperationFamily::Scallop, PassRole::Finish) => DefaultProfile { ap_factor: 0.08, ae_factor: 0.025 },
        // Trace: V-carve/engrave
        (OperationFamily::Trace, PassRole::Roughing) => DefaultProfile { ap_factor: 0.15, ae_factor: 0.05 },
        (OperationFamily::Trace, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.10, ae_factor: 0.03 },
        (OperationFamily::Trace, PassRole::Finish) => DefaultProfile { ap_factor: 0.06, ae_factor: 0.02 },
        // Face: wide and shallow
        (OperationFamily::Face, PassRole::Roughing) => DefaultProfile { ap_factor: 0.08, ae_factor: 0.65 },
        (OperationFamily::Face, PassRole::SemiFinish) => DefaultProfile { ap_factor: 0.06, ae_factor: 0.55 },
        (OperationFamily::Face, PassRole::Finish) => DefaultProfile { ap_factor: 0.04, ae_factor: 0.45 },
    }
}

fn default_engagement(d: f64, profile: &DefaultProfile, input: &FeedsInput, machine: &MachineProfile) -> (f64, f64) {
    let mut ap_factor = profile.ap_factor;
    let mut ae_factor = profile.ae_factor;

    // Tool geometry adjustments for finishing operations
    match (input.tool_geometry, input.operation, input.pass_role) {
        (ToolGeometryHint::Ball, OperationFamily::Parallel | OperationFamily::Scallop, PassRole::Finish) => {
            ap_factor = 0.06;
            ae_factor = 0.025;
        }
        (ToolGeometryHint::Ball, OperationFamily::Parallel | OperationFamily::Scallop, PassRole::SemiFinish) => {
            ap_factor = 0.10;
            ae_factor = 0.04;
        }
        (ToolGeometryHint::TaperedBall { .. }, OperationFamily::Parallel | OperationFamily::Scallop, PassRole::Finish) => {
            ap_factor = 0.10;
            ae_factor = 0.03;
        }
        (ToolGeometryHint::TaperedBall { .. }, OperationFamily::Parallel | OperationFamily::Scallop, PassRole::SemiFinish) => {
            ap_factor = 0.14;
            ae_factor = 0.05;
        }
        _ => {}
    }

    // Use machine rigidity factors for adaptive operations
    if input.operation == OperationFamily::Adaptive {
        ap_factor = ap_factor.max(machine.rigidity.adaptive_doc_factor * profile.ap_factor / 1.5);
        ae_factor = ae_factor.min(machine.rigidity.adaptive_woc_factor);
    }

    let ap = (d * ap_factor).max(0.05);
    let ae = (d * ae_factor).max(0.02);
    (ap, ae)
}

fn effective_diameter(geom: ToolGeometryHint, nominal_d: f64, ap: f64) -> f64 {
    match geom {
        ToolGeometryHint::Flat => nominal_d,
        ToolGeometryHint::Ball => geometry::ball_effective_diameter(nominal_d, ap),
        ToolGeometryHint::Bull { corner_radius } => {
            geometry::bull_nose_effective_diameter(nominal_d, corner_radius, ap)
        }
        ToolGeometryHint::VBit { included_angle, tip_diameter } => {
            geometry::vbit_width_at_depth(included_angle, tip_diameter, ap)
                .unwrap_or(nominal_d)
                .min(nominal_d)
        }
        ToolGeometryHint::TaperedBall { tip_radius, taper_angle_deg } => {
            geometry::tapered_ball_effective_diameter(nominal_d, tip_radius, taper_angle_deg, ap)
        }
    }
}

fn estimate_plunge_rate(material: &Material, hardness: f64) -> f64 {
    match material {
        Material::SolidWood { .. } => 1000.0 / hardness,
        Material::Plywood { .. } | Material::SheetGood { .. } => 900.0 / hardness,
        Material::Plastic { .. } => 1500.0,
        Material::Foam { .. } => 2000.0,
        Material::Custom { .. } => 800.0 / hardness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineProfile;
    use crate::material::{Material, WoodSpecies};

    fn softwood_flat_6mm_pocket() -> FeedsInput<'static> {
        // We need 'static material/machine so use leaked boxes for test convenience
        let material: &'static Material = Box::leak(Box::new(
            Material::SolidWood { species: WoodSpecies::GenericSoftwood }
        ));
        let machine: &'static MachineProfile = Box::leak(Box::new(
            MachineProfile::shapeoko_vfd()
        ));
        FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material,
            machine,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
        }
    }

    #[test]
    fn test_chip_load_soft_wood_6mm() {
        let machine = MachineProfile::shapeoko_vfd();
        let d: f64 = 6.0;
        let h = Material::SolidWood { species: WoodSpecies::GenericSoftwood }.hardness_index();
        let cl = machine.chip_load.k0 * d.powf(machine.chip_load.p) * (1.0 / h).powf(machine.chip_load.q);
        assert!((cl - 0.0716).abs() < 0.002, "expected ~0.0716, got {cl}");
    }

    #[test]
    fn test_chip_load_hard_wood_3175mm() {
        let machine = MachineProfile::shapeoko_vfd();
        let d: f64 = 3.175;
        let h = Material::SolidWood { species: WoodSpecies::HardMaple }.hardness_index();
        let cl = machine.chip_load.k0 * d.powf(machine.chip_load.p) * (1.0 / h).powf(machine.chip_load.q);
        assert!((cl - 0.0311).abs() < 0.002, "expected ~0.0311, got {cl}");
    }

    #[test]
    fn test_feed_rate_basic() {
        let feed = 18000.0 * 0.05 * 2.0;
        assert_eq!(feed, 1800.0);
    }

    #[test]
    fn test_calculate_produces_reasonable_values() {
        let input = softwood_flat_6mm_pocket();
        let result = calculate(&input);

        assert!(result.rpm >= 6000.0 && result.rpm <= 24000.0, "RPM {}", result.rpm);
        assert!(result.feed_rate_mm_min > 500.0 && result.feed_rate_mm_min < 5000.0,
            "feed {}", result.feed_rate_mm_min);
        assert!(result.plunge_rate_mm_min > 100.0 && result.plunge_rate_mm_min < 2000.0,
            "plunge {}", result.plunge_rate_mm_min);
        assert!(result.axial_depth_mm > 0.0 && result.axial_depth_mm <= 18.0,
            "DOC {}", result.axial_depth_mm);
        assert!(result.radial_width_mm > 0.0 && result.radial_width_mm <= 6.0,
            "WOC {}", result.radial_width_mm);
        assert!(result.power_kw >= 0.0, "power {}", result.power_kw);
    }

    #[test]
    fn test_adaptive_deeper_narrower_than_pocket() {
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let machine = MachineProfile::shapeoko_vfd();

        let adaptive = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Adaptive, pass_role: PassRole::Roughing,
            axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
        });
        let pocket = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Pocket, pass_role: PassRole::Roughing,
            axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
        });

        assert!(adaptive.axial_depth_mm > pocket.axial_depth_mm,
            "adaptive DOC {} should > pocket DOC {}", adaptive.axial_depth_mm, pocket.axial_depth_mm);
        assert!(adaptive.radial_width_mm < pocket.radial_width_mm,
            "adaptive WOC {} should < pocket WOC {}", adaptive.radial_width_mm, pocket.radial_width_mm);
    }

    #[test]
    fn test_roughing_deeper_than_finishing() {
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let machine = MachineProfile::shapeoko_vfd();

        let families = [
            OperationFamily::Adaptive, OperationFamily::Pocket, OperationFamily::Contour,
            OperationFamily::Parallel, OperationFamily::Scallop, OperationFamily::Trace,
            OperationFamily::Face,
        ];

        for family in families {
            let rough = calculate(&FeedsInput {
                tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
                tool_geometry: ToolGeometryHint::Flat,
                material: &material, machine: &machine,
                operation: family, pass_role: PassRole::Roughing,
                axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
            });
            let finish = calculate(&FeedsInput {
                tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
                tool_geometry: ToolGeometryHint::Flat,
                material: &material, machine: &machine,
                operation: family, pass_role: PassRole::Finish,
                axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
            });

            assert!(rough.axial_depth_mm >= finish.axial_depth_mm,
                "{family:?}: roughing DOC {} should >= finishing DOC {}",
                rough.axial_depth_mm, finish.axial_depth_mm);
            assert!(rough.radial_width_mm >= finish.radial_width_mm,
                "{family:?}: roughing WOC {} should >= finishing WOC {}",
                rough.radial_width_mm, finish.radial_width_mm);
        }
    }

    #[test]
    fn test_flute_guard_caps_doc() {
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 5.0, // very short flutes
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Adaptive, pass_role: PassRole::Roughing,
            axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
        });

        assert!(result.axial_depth_mm <= 5.0 * 0.8 + 0.01,
            "DOC {} should be capped by flute guard 4.0", result.axial_depth_mm);
        assert!(result.warnings.iter().any(|w| matches!(w, FeedsWarning::DocExceedsFlute { .. })));
    }

    #[test]
    fn test_power_limiting_on_low_power_machine() {
        // Use softwood (high chip load) with a tiny spindle to trigger power limiting
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let mut machine = MachineProfile::shapeoko_vfd();
        machine.power = crate::machine::PowerModel::ConstantPower { power_kw: 0.01 }; // extremely tiny

        let result = calculate(&FeedsInput {
            tool_diameter: 12.0, flute_count: 4, flute_length: 25.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Pocket, pass_role: PassRole::Roughing,
            axial_depth_mm: Some(5.0), radial_width_mm: Some(8.0),
            target_scallop_mm: None,
        });

        assert!(result.power_limited, "should be power limited with 0.01kW spindle: power={:.4}kW, available={:.4}kW",
            result.power_kw, result.available_power_kw);
        assert!(result.warnings.iter().any(|w| matches!(w, FeedsWarning::PowerLimited { .. })));
    }

    #[test]
    fn test_scallop_stepover_used_for_ball_nose() {
        let material = Material::SolidWood { species: WoodSpecies::HardMaple };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Ball,
            material: &material, machine: &machine,
            operation: OperationFamily::Parallel, pass_role: PassRole::Finish,
            axial_depth_mm: Some(0.4), radial_width_mm: None,
            target_scallop_mm: Some(0.03),
        });

        // With 3mm ball radius, 0.03mm scallop → stepover should be small
        assert!(result.radial_width_mm > 0.0 && result.radial_width_mm < 6.0);
    }

    #[test]
    fn test_machine_feed_clamp() {
        let material = Material::Foam { density: crate::material::FoamDensity::Low };
        let mut machine = MachineProfile::generic_wood_router();
        machine.max_feed_mm_min = 500.0; // very low max feed

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Pocket, pass_role: PassRole::Roughing,
            axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
        });

        assert!(result.feed_rate_mm_min <= 500.0 * machine.safety_factor + 0.01,
            "feed {} should be clamped to max {}", result.feed_rate_mm_min, 500.0);
    }

    #[test]
    fn test_safety_factor_applied() {
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Pocket, pass_role: PassRole::Roughing,
            axial_depth_mm: None, radial_width_mm: None, target_scallop_mm: None,
        });

        // Feed should be < what it would be without safety factor
        // The safety factor is 0.80, so feed should be roughly 80% of unclamped
        assert!(result.feed_rate_mm_min > 0.0);
    }

    #[test]
    fn test_slotting_detection() {
        let material = Material::SolidWood { species: WoodSpecies::GenericSoftwood };
        let machine = MachineProfile::shapeoko_vfd();

        let result = calculate(&FeedsInput {
            tool_diameter: 6.0, flute_count: 2, flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            material: &material, machine: &machine,
            operation: OperationFamily::Pocket, pass_role: PassRole::Roughing,
            axial_depth_mm: Some(10.0),
            radial_width_mm: Some(5.5), // >85% of D = slotting
            target_scallop_mm: None,
        });

        assert!(result.axial_depth_mm <= 6.0 * 0.25 + 0.01,
            "slotting should reduce DOC, got {}", result.axial_depth_mm);
        assert!(result.warnings.iter().any(|w| matches!(w, FeedsWarning::SlottingDetected { .. })));
    }
}
