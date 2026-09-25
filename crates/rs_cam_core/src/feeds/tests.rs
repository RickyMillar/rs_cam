//! Unit tests for the feeds calculator. Moved out of `feeds/mod.rs` by P4;
//! the module body is unchanged.

#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;
use crate::machine::MachineProfile;
use crate::material::{Material, WoodSpecies};

fn softwood_flat_6mm_pocket() -> FeedsInput<'static> {
    // We need 'static material/machine so use leaked boxes for test convenience
    let material: &'static Material = Box::leak(Box::new(Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }));
    let machine: &'static MachineProfile = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
    FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    }
}

#[test]
fn test_chip_load_soft_wood_6mm() {
    let machine = MachineProfile::shapeoko_vfd();
    let d: f64 = 6.0;
    let h = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
    .feed_scale_factor();
    let cl =
        machine.chip_load.k0 * d.powf(machine.chip_load.p) * (1.0 / h).powf(machine.chip_load.q);
    assert!((cl - 0.0716).abs() < 0.002, "expected ~0.0716, got {cl}");
}

#[test]
fn test_chip_load_hard_wood_3175mm() {
    let machine = MachineProfile::shapeoko_vfd();
    let d: f64 = 3.175;
    let h = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
    .feed_scale_factor();
    let cl =
        machine.chip_load.k0 * d.powf(machine.chip_load.p) * (1.0 / h).powf(machine.chip_load.q);
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

    assert!(
        result.rpm >= 6000.0 && result.rpm <= 24000.0,
        "RPM {}",
        result.rpm
    );
    assert!(
        result.feed_rate_mm_min > 500.0 && result.feed_rate_mm_min < 5000.0,
        "feed {}",
        result.feed_rate_mm_min
    );
    assert!(
        result.plunge_rate_mm_min > 100.0 && result.plunge_rate_mm_min < 2000.0,
        "plunge {}",
        result.plunge_rate_mm_min
    );
    assert!(
        result.axial_depth_mm > 0.0 && result.axial_depth_mm <= 18.0,
        "DOC {}",
        result.axial_depth_mm
    );
    assert!(
        result.radial_width_mm > 0.0 && result.radial_width_mm <= 6.0,
        "WOC {}",
        result.radial_width_mm
    );
    assert!(result.power_kw >= 0.0, "power {}", result.power_kw);
}

#[test]
fn test_adaptive_deeper_narrower_than_pocket() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let adaptive = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });
    let pocket = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        adaptive.axial_depth_mm > pocket.axial_depth_mm,
        "adaptive DOC {} should > pocket DOC {}",
        adaptive.axial_depth_mm,
        pocket.axial_depth_mm
    );
    assert!(
        adaptive.radial_width_mm < pocket.radial_width_mm,
        "adaptive WOC {} should < pocket WOC {}",
        adaptive.radial_width_mm,
        pocket.radial_width_mm
    );
}

#[test]
fn test_roughing_deeper_than_finishing() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let families = [
        OperationFamily::Adaptive,
        OperationFamily::Pocket,
        OperationFamily::Contour,
        OperationFamily::Parallel,
        OperationFamily::Scallop,
        OperationFamily::Trace,
        OperationFamily::Face,
    ];

    for family in families {
        let rough = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: family,
            operation_kind: None,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });
        let finish = calculate(&FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            tool_geometry: ToolGeometryHint::Flat,
            shank_diameter: None,
            material: &material,
            machine: &machine,
            operation: family,
            operation_kind: None,
            pass_role: PassRole::Finish,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        });

        assert!(
            rough.axial_depth_mm >= finish.axial_depth_mm,
            "{family:?}: roughing DOC {} should >= finishing DOC {}",
            rough.axial_depth_mm,
            finish.axial_depth_mm
        );
        assert!(
            rough.radial_width_mm >= finish.radial_width_mm,
            "{family:?}: roughing WOC {} should >= finishing WOC {}",
            rough.radial_width_mm,
            finish.radial_width_mm
        );
    }
}

/// Fix 2 (Wanaka audit): plunge for small ball/tapered-ball tools
/// must derate by tool-tip diameter. A 1 mm tapered ball in
/// hardwood was previously emitting 750 mm/min plunge (the
/// material-level base × machine safety factor) — 2.5× above
/// FSWizard's 100–300 mm/min safe band. Cap is 150 mm/min per
/// mm of tip diameter.
#[test]
fn test_small_tapered_ball_plunge_derated() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 1.0,
        flute_count: 1,
        flute_length: 6.0,
        tool_geometry: ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 7.0,
        },
        shank_diameter: Some(6.0),
        material: &material,
        machine: &machine,
        operation: OperationFamily::Parallel,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        result.plunge_rate_mm_min <= 200.0,
        "1mm TB plunge {} should be ≤ 200 mm/min after Fix 2 derate",
        result.plunge_rate_mm_min
    );
    assert!(
        result.plunge_rate_mm_min >= 50.0,
        "1mm TB plunge {} should not collapse to near-zero",
        result.plunge_rate_mm_min
    );
}

/// G10 (2026-09-25): a 6 mm 2F flat in hardwood plunges at feed / 2, the
/// flat end mill claim (ruling Q4). The Fix 2 tip cap is not in the value:
/// a flat has no tip cap.
#[test]
fn test_flat_endmill_plunge_is_the_g10_flat_claim() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // The G10 flat claim: plunge = floor(F / Z) at Z 2; no tip cap.
    let claim = result
        .plunge
        .claim()
        .unwrap_or_else(|| panic!("6mm 2F flat: basis {:?}", result.plunge));
    assert_eq!(claim.rule.id, "g10_plunge_flat");
    assert_eq!(result.plunge.tip_cap(), None);
    assert_eq!(
        result.plunge_rate_mm_min,
        (result.feed_rate_mm_min / 2.0).floor(),
        "6mm 2F flat plunge {} should be floor(feed {} / 2)",
        result.plunge_rate_mm_min,
        result.feed_rate_mm_min
    );
}

/// Fix 1 (Wanaka audit): adaptive stepover for wood + flat tools
/// should track the machine rigidity factor (`adaptive_woc_factor`),
/// not the metal-grade 0.14 base. Empirical: 6 mm flat in
/// Generic Hardwood on a wood-router with `adaptive_woc_factor =
/// 0.20` should yield ae ≈ 1.2 mm, not 0.7 mm. Engagement profile
/// then sits in the "normal" bin instead of "light".
#[test]
fn test_wood_adaptive_stepover_tracks_machine_factor() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let target = machine.rigidity.adaptive_woc_factor * 6.0;

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        (result.radial_width_mm - target).abs() < 1e-6,
        "wood adaptive WOC {} should match machine.adaptive_woc_factor × D = {}",
        result.radial_width_mm,
        target
    );
}

/// Counter-test: metals should NOT get the wood adaptive bonus —
/// the metal-grade 0.14 base (with hardness derates) stays in
/// force so adaptive stepover remains conservative.
#[test]
fn test_metal_adaptive_stepover_keeps_metal_base() {
    let material = Material::Plastic {
        family: crate::material::PlasticFamily::Acrylic,
    };
    // Acrylic is not wood-class — should bypass the Fix 1 floor.
    let machine = MachineProfile::shapeoko_vfd();
    let machine_factor_ae = machine.rigidity.adaptive_woc_factor * 6.0;

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Plastic is not wood-class; ae should sit below the
    // machine ceiling rather than being raised to it.
    assert!(
        result.radial_width_mm < machine_factor_ae,
        "non-wood adaptive WOC {} should stay below machine factor {}",
        result.radial_width_mm,
        machine_factor_ae
    );
}

/// Fix #7 (2026-06-02 audit): `Material::plunge_rate_base()`
/// scales linearly with tool diameter. A 6 mm bit gets the
/// preserved baseline; a 3 mm bit gets half, a 12 mm bit gets
/// double (within the [0.25×, 3×] clamp). Pre-fix a 3 mm bit
/// inherited the 6 mm plunge envelope, plunging 2× too fast.
#[test]
fn test_plunge_rate_base_scales_with_diameter() {
    use crate::material::Material;
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };

    let plunge_3mm = material.plunge_rate_base(3.0);
    let plunge_6mm = material.plunge_rate_base(6.0);
    let plunge_12mm = material.plunge_rate_base(12.0);

    // 6 mm baseline preserved (matches pre-fix material-only value).
    let h = material.feed_scale_factor();
    let expected_6mm = 1000.0 / h;
    assert!(
        (plunge_6mm - expected_6mm).abs() < 1e-6,
        "6 mm plunge {plunge_6mm} should preserve pre-fix value {expected_6mm}"
    );

    // 3 mm = half of 6 mm.
    assert!(
        (plunge_3mm - plunge_6mm * 0.5).abs() < 1e-6,
        "3 mm plunge {plunge_3mm} should be 0.5 × 6 mm plunge {plunge_6mm}"
    );

    // 12 mm = double of 6 mm.
    assert!(
        (plunge_12mm - plunge_6mm * 2.0).abs() < 1e-6,
        "12 mm plunge {plunge_12mm} should be 2.0 × 6 mm plunge {plunge_6mm}"
    );

    // Clamps: very small / very large tools don't run away.
    let plunge_tiny = material.plunge_rate_base(1.0);
    assert!(
        plunge_tiny >= plunge_6mm * 0.25 - 1e-6,
        "tiny-tool plunge {plunge_tiny} should clamp to 0.25 × baseline floor"
    );
    let plunge_huge = material.plunge_rate_base(25.0);
    assert!(
        plunge_huge <= plunge_6mm * 3.0 + 1e-6,
        "huge-tool plunge {plunge_huge} should clamp to 3.0 × baseline ceiling"
    );
}

/// Fix #3 (2026-06-02 audit): Drill ops are routed through their
/// own `OperationFamily::Drill` (was `OperationFamily::Pocket`),
/// and the calculate() path clamps drill RPM into the diameter tier
/// (8-14k at Ø6) — milling SFM/RPM derivation push small-D drills
/// past 16k where chipload starves.
///
/// Ruling B5 (2026-09-24) deleted the drill multiplier (2.5). The old
/// "0.05 mm/rev softwood drill floor" was a band fitted to formula x
/// 2.5 (W6 audit), so this test no longer asserts it. It asserts that
/// the preview chip IS the milling formula with no multiplier. With no
/// LUT the cell refuses (no G6 claim), and the number is a preview.
#[test]
fn test_drill_family_rpm_in_drill_band() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // RPM must sit inside the drill band, not the milling SFM result.
    assert!(
        (8_000.0..=14_000.0).contains(&result.rpm),
        "drill RPM {} must clamp to 8k-14k drill band",
        result.rpm
    );

    // Implied chipload = feed / (flutes * rpm). GenericSoftwood has a
    // feed scale of 1.0, so the milling formula is k0 * D^p. No drill
    // multiplier applies (ruling B5).
    let cl = &machine.chip_load;
    let formula = cl.k0 * 6.0_f64.powf(cl.p);
    let implied_chipload = result.feed_rate_mm_min / (2.0 * result.rpm);
    assert!(
        (implied_chipload / formula - 1.0).abs() < 1e-9,
        "drill implied chipload {implied_chipload} must be the milling formula {formula} \
         with no multiplier"
    );
    assert!(
        matches!(result.support, FeedsSupport::Refuse { .. }),
        "a drill with no G6 claim refuses; the number is a preview: {:?}",
        result.support
    );
}

/// F1 (2026-06-10 defect-class cleanup): drill results are
/// self-consistent — plunge IS feed, so the drill configs' aliased
/// setters ("feed IS plunge") are write-order-safe — and the feed
/// sits inside the material plunge-feed envelope. Pre-fix,
/// `apply_feeds_subset` wrote feed then plunge and the milling
/// plunge baseline (≈595 for Ø6 hardwood) clobbered the drill-tuned
/// feed; separately the unclamped drill feed (4000) exceeded the
/// Ø6 wood envelope max (2400).
#[test]
fn test_drill_feed_within_envelope_and_plunge_aliased() {
    for species in [WoodSpecies::GenericSoftwood, WoodSpecies::GenericHardwood] {
        let material = Material::SolidWood { species };
        let machine = MachineProfile::shapeoko_vfd();
        for d in [3.0, 6.0, 12.0] {
            let result = calculate(&FeedsInput {
                tool_diameter: d,
                flute_count: 2,
                flute_length: 18.0,
                tool_geometry: ToolGeometryHint::Flat,
                shank_diameter: None,
                material: &material,
                machine: &machine,
                operation: OperationFamily::Drill,
                operation_kind: None,
                pass_role: PassRole::Roughing,
                axial_depth_mm: None,
                radial_width_mm: None,
                target_scallop_mm: None,
                vendor_lut: None,
                setup: SetupContext::default(),
                spindle_strategy: crate::feeds::SpindleStrategy::default(),
            });
            assert_eq!(
                result.plunge_rate_mm_min, result.feed_rate_mm_min,
                "Ø{d} {species:?}: drill plunge must alias the final feed"
            );
            let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
            let ratio = result.feed_rate_mm_min / d;
            assert!(
                ratio >= lo - 1e-9 && ratio <= hi + 1e-9,
                "Ø{d} {species:?}: feed/Ø {ratio:.1} outside envelope {lo}-{hi}"
            );
        }
    }
}

/// F1: when the envelope ceiling binds, RPM follows the feed down
/// (bounded by the drill band floor) so the result keeps its chipload
/// instead of thinning toward rubbing.
///
/// Rebaselined for ruling B5 (2026-09-24). The old fixture (Ø3 oak,
/// "~0.107 mm/tooth") rested on formula x 2.5; without the multiplier
/// Ø3 oak sits at about 290 mm/min per mm, under the 580 ceiling. The
/// new fixture is a Ø2 3-flute formula preview in GenericSoftwood (no
/// LUT, so the cell refuses and the number is a preview):
///
/// - RPM: 200 000 / (π × 2) = 31 831 → machine 24 000 → drill cap 14 000;
/// - chip: k0 × 2^p = 0.024 × 2^0.61 = 0.03663 mm/tooth;
/// - feed: 14 000 × 0.03663 × 3 = 1 538.5 mm/min, over the ceiling
///   580 × 2 = 1 160;
/// - follow-down: 1 160 / (0.03663 × 3) = 10 556 RPM, above the 8 000
///   floor, so the chip holds at 0.03663.
#[test]
fn test_drill_envelope_ceiling_drops_rpm_to_hold_chipload() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let result = calculate(&FeedsInput {
        tool_diameter: 2.0,
        flute_count: 3,
        flute_length: 12.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });
    let clamp_fired = result
        .warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::DrillFeedClampedToEnvelope { .. }));
    assert!(
        clamp_fired,
        "Ø2 3-flute softwood drill must hit the envelope ceiling"
    );
    let (_, hi) = material.drill_plunge_feed_envelope_per_mm();
    assert!(
        (result.feed_rate_mm_min - hi * 2.0).abs() < 1e-9,
        "the feed must land on the ceiling {}, got {}",
        hi * 2.0,
        result.feed_rate_mm_min
    );
    assert!(
        result.rpm < 14_000.0,
        "RPM must follow the capped feed down, got {}",
        result.rpm
    );
    assert!(
        result.rpm >= 8_000.0,
        "follow-down bounded by the drill band floor, got {}",
        result.rpm
    );
    let cl = &machine.chip_load;
    let formula = cl.k0 * 2.0_f64.powf(cl.p);
    let implied = result.feed_rate_mm_min / (3.0 * result.rpm);
    assert!(
        (implied / formula - 1.0).abs() < 1e-9,
        "the follow-down must hold the chip {formula:.5}, got {implied:.5}"
    );
}

/// F1: the envelope clamp warns when it binds — never a silent
/// rewrite. The Ø2 3-flute softwood preview of
/// `test_drill_envelope_ceiling_drops_rpm_to_hold_chipload` drives the
/// feed (1 538.5) above the envelope max (580 × 2 = 1 160; ruling B5), so
/// the clamp must fire with the honest before/after. Before B5 the
/// fixture was Ø3 2-flute, which formula x 2.5 put over 400 × 3; without
/// the multiplier it sits in band and the warning check was vacuous.
#[test]
fn test_drill_feed_clamp_emits_warning_when_binding() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let result = calculate(&FeedsInput {
        tool_diameter: 2.0,
        flute_count: 3,
        flute_length: 12.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });
    let ratio = result.feed_rate_mm_min / 2.0;
    let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
    assert!(ratio >= lo - 1e-9 && ratio <= hi + 1e-9);
    let clamped = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested, actual, ..
        } => Some((*requested, *actual)),
        _ => None,
    });
    let Some((requested, actual)) = clamped else {
        panic!("non-vacuity: the fixture must bind the envelope ceiling");
    };
    assert!(
        (actual - result.feed_rate_mm_min).abs() < 1e-9,
        "warning's actual {actual} must match the shipped feed {}",
        result.feed_rate_mm_min
    );
    assert!(
        requested > hi * 2.0,
        "warning fired but requested {requested} was not above the ceiling {}",
        hi * 2.0
    );
}

/// Fix #3 regression guard: a Pocket op on the same tool/material
/// must NOT see the drill RPM clamp or chipload multiplier —
/// the fix is selective by family.
#[test]
fn test_pocket_family_unaffected_by_drill_fix() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Pocket RPM is allowed to be >14k (milling SFM band).
    // Don't assert a specific value — just check the drill clamp
    // didn't fire.
    assert!(
        result.rpm > 14_000.0 || result.rpm == machine.clamp_rpm(result.rpm),
        "pocket RPM {} should not be drill-clamped",
        result.rpm
    );
}

/// Fix #4 (2026-06-02 audit): RPM for a V-bit must be derived from
/// the engaged tip diameter at DOC, not the nominal shank diameter.
/// A 20° V-bit at shallow DOC has near-zero engaged D → SFM-derived
/// RPM should hit the machine ceiling (the rule of thumb says V-bit
/// wants high RPM because effective SFM at the tip is essentially
/// zero). The pre-fix path used nominal D=5.5 mm and produced ~11.5k
/// RPM regardless of DOC.
#[test]
fn test_vbit_rpm_uses_engaged_diameter() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let machine_max_rpm = match &machine.spindle {
        crate::machine::SpindleConfig::Variable { max_rpm, .. } => *max_rpm,
        _ => panic!("expected variable spindle on shapeoko_vfd"),
    };

    // 20° V-bit, 5.5 mm shank, at 0.5 mm DOC. Engaged tip diameter
    // = 2 * 0.5 * tan(10°) ≈ 0.176 mm — small enough that SFM-derived
    // ideal RPM exceeds the machine ceiling, so the result clamps
    // to machine max.
    let result = calculate(&FeedsInput {
        tool_diameter: 5.5,
        flute_count: 2,
        flute_length: 12.0,
        tool_geometry: ToolGeometryHint::VBit {
            included_angle: 20.0,
            tip_diameter: 0.0,
        },
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Trace,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(0.5),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        result.rpm >= machine_max_rpm * 0.99,
        "20° V-bit at DOC=0.5 mm should clamp to spindle max ({}), \
         got {} — engaged-D pipeline likely not firing",
        machine_max_rpm,
        result.rpm
    );
}

/// Fix #4 regression guard: Flat tools must produce the same RPM
/// and chipload before/after the engaged-D switch — `engaged_diameter_at_doc`
/// returns nominal D for Flat geometry, so all Flat-tool feeds
/// stay byte-identical.
#[test]
fn test_flat_tool_rpm_chipload_unchanged_by_engaged_d() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let d = 6.0;
    // Baseline: what the formula path produced via nominal D.
    let baseline_rpm =
        (material.base_cutting_speed_m_min() * 1000.0 / (std::f64::consts::PI * d)).round();

    let result = calculate(&FeedsInput {
        tool_diameter: d,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(3.0), // DOC doesn't matter for Flat
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Allow the machine-clamp the formula was about to be passed
    // through (so we compare clamp(baseline) ≈ result.rpm).
    let clamped_baseline = machine.clamp_rpm(baseline_rpm);
    assert!(
        (result.rpm - clamped_baseline).abs() < 1.0,
        "Flat-tool RPM should be unchanged by engaged-D switch \
         (baseline {clamped_baseline}, got {})",
        result.rpm
    );
}

/// Bug 2 (2026-06-02 audit): the species-aware `SolidWoodByJanka`
/// variant must qualify for the wood adaptive WOC floor — same as
/// `SolidWood`. Eastern White Pine (Janka 382.2, used in Wanaka)
/// should give ae = `adaptive_woc_factor × D` = 1.2 mm on a 6 mm
/// flat, not the 0.88 mm (= 0.147 × D) that the bug produced.
#[test]
fn test_wood_adaptive_stepover_solid_wood_by_janka_eastern_white_pine() {
    let material = Material::SolidWoodByJanka {
        janka_lbf: 382.2,
        label: "Pine, eastern white".to_owned(),
        source_id: "fpl_ch5_2010".to_owned(),
    };
    let machine = MachineProfile::shapeoko_vfd();
    let target = machine.rigidity.adaptive_woc_factor * 6.0;

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        (result.radial_width_mm - target).abs() < 1e-6,
        "SolidWoodByJanka adaptive WOC {} should match \
         machine.adaptive_woc_factor × D = {} (Bug 2 fix)",
        result.radial_width_mm,
        target
    );
}

#[test]
fn test_flute_guard_caps_doc() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 5.0, // very short flutes
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        result.axial_depth_mm <= 5.0 * 0.8 + 0.01,
        "DOC {} should be capped by flute guard 4.0",
        result.axial_depth_mm
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::DocExceedsFlute { .. }))
    );
}

#[test]
fn test_power_limiting_on_low_power_machine() {
    // Use softwood (high chip load) with a tiny spindle to trigger power limiting
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let mut machine = MachineProfile::shapeoko_vfd();
    machine.power = crate::machine::PowerModel::ConstantPower { power_kw: 0.01 }; // extremely tiny

    let result = calculate(&FeedsInput {
        tool_diameter: 12.0,
        flute_count: 4,
        flute_length: 25.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(5.0),
        radial_width_mm: Some(8.0),
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        result.power_limited,
        "should be power limited with 0.01kW spindle: power={:.4}kW, available={:.4}kW",
        result.power_kw, result.available_power_kw
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::PowerLimited { .. }))
    );
}

#[test]
fn test_scallop_stepover_used_for_ball_nose() {
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Ball,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Parallel,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(0.4),
        radial_width_mm: None,
        target_scallop_mm: Some(0.03),
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // With 3mm ball radius, 0.03mm scallop → stepover should be small
    assert!(result.radial_width_mm > 0.0 && result.radial_width_mm < 6.0);
}

#[test]
fn test_machine_feed_clamp() {
    let material = Material::Foam {
        density: crate::material::FoamDensity::Low,
    };
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_feed_mm_min = 500.0; // very low max feed

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Ruling R4 (2026-09-24): no factor after the ceiling.
    assert!(
        result.feed_rate_mm_min <= 500.0 + 0.01,
        "feed {} should be clamped to max {}",
        result.feed_rate_mm_min,
        500.0
    );
}

/// Ruling R4 (2026-09-24): the machine safety factor is gone. The feed is
/// the chipload times the combined factor times RPM times flutes, and the
/// combined factor holds no machine factor: on this unclamped pocket it is
/// the depth tier (the workholding factor is gone, ruling R4 Q8).
#[test]
fn no_machine_factor_multiplies_the_feed() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(result.feed_rate_mm_min > 0.0);
    let d = &result.derates;
    assert!((d.combined_factor() - d.depth_tier * d.power_limit * d.feed_clamp).abs() < 1e-12);
    let fpt = result.feed_rate_mm_min / (result.rpm * 2.0);
    assert!((fpt - d.effective_chip_load_mm()).abs() <= fpt * 1e-9);
}

#[test]
fn test_slotting_detection() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(10.0),
        radial_width_mm: Some(5.5), // >85% of D = slotting
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(
        result.axial_depth_mm <= 6.0 * 0.25 + 0.01,
        "slotting should reduce DOC, got {}",
        result.axial_depth_mm
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::SlottingDetected { .. }))
    );
}

// --- Vendor LUT integration tests ---

#[test]
fn test_lut_chipload_overrides_formula() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let with_lut = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });
    let without_lut = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Feeds matrix R5 (2026-09-23): the matched row is the printed Amana
    // Spektra v24 line "2 Flute 6mm (48118-K/48218-K) Wood/Plywood
    // 0.0050 in" = 0.127 mm/tooth, published as one value. The old
    // 0.0875 midpoint came from a band that is not on the chart.
    // Formula chipload should be ~0.0716
    assert!(
        (with_lut.chip_load_mm - 0.127).abs() < 0.002,
        "LUT chipload should be ~0.127, got {}",
        with_lut.chip_load_mm
    );
    assert!(
        (without_lut.chip_load_mm - 0.0716).abs() < 0.002,
        "formula chipload should be ~0.0716, got {}",
        without_lut.chip_load_mm
    );
    assert!(
        with_lut.chip_load_mm > without_lut.chip_load_mm,
        "LUT chipload {} should differ from formula {}",
        with_lut.chip_load_mm,
        without_lut.chip_load_mm
    );
    assert!(with_lut.vendor_source.is_some());
    assert!(matches!(
        with_lut.chipload_source,
        ChiploadSource::VendorLut { .. }
    ));
    assert!(without_lut.vendor_source.is_none());
    assert_eq!(without_lut.chipload_source, ChiploadSource::FormulaFallback);
}

/// Finding 1 (2026-06-04): Suggest's `ChiploadBounds` must mirror
/// the post-sim chipload gate's DOC-derated envelope so
/// `SuggestAggressiveness::target_chipload(bounds)` aims at a value
/// the gate will accept. Without derating, v3.0c median targeting
/// on high-DOC milling ops (e.g. wanaka Back Rough at ~3×D) lands
/// above the gate's derated max and trips `Exceeds(High)`.
///
/// This test pins the derating contract on a 3×D hardwood pocket-
/// roughing case: bounds at axial_depth=D get the 1.0 scale (no
/// change), bounds at axial_depth=3D get the 0.5 scale.
///
/// Fixture since feeds matrix R5 (2026-09-23): a Ø6.35 ball nose. The
/// flat 6 mm hardwood pocket cell now resolves to the printed Amana
/// Spektra row, which publishes one value and therefore no band; the
/// printed Amana ball-nose v7 row `amana-ball-hardwood-pocket-6350-2f-v7`
/// (0.127-0.1778 mm/tooth) publishes both bounds.
#[test]
fn chipload_bounds_derate_with_doc_ratio_for_milling() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let mut input = FeedsInput {
        tool_diameter: 6.35,
        flute_count: 2,
        flute_length: 24.0,
        tool_geometry: ToolGeometryHint::Ball,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(6.35), // 1×D ratio → scale 1.0
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    };
    let at_1x = calculate(&input);
    input.axial_depth_mm = Some(19.05); // 3×D ratio → scale 0.5
    let at_3x = calculate(&input);

    // The printed Amana ball-nose v7 row publishes both bounds,
    // so the orchestrator must produce `Some(ChiploadBounds)` here.
    let b1 = at_1x.chipload_bounds.unwrap();
    let b3 = at_3x.chipload_bounds.unwrap();
    let scale_min = b3.min_mm_per_tooth / b1.min_mm_per_tooth;
    let scale_max = b3.max_mm_per_tooth / b1.max_mm_per_tooth;
    assert!(
        (scale_min - 0.5).abs() < 1e-6,
        "min should derate to 0.5× at 3×D, got scale {scale_min} ({} → {})",
        b1.min_mm_per_tooth,
        b3.min_mm_per_tooth,
    );
    assert!(
        (scale_max - 0.5).abs() < 1e-6,
        "max should derate to 0.5× at 3×D, got scale {scale_max} ({} → {})",
        b1.max_mm_per_tooth,
        b3.max_mm_per_tooth,
    );
}

/// Drill ops are explicitly excluded from chipload-bounds DOC
/// derating: the post-sim chipload gate short-circuits drill ops
/// (`NotApplicableForOp`), and the chip-evacuation rule that
/// motivates derating in milling doesn't apply to drill bands
/// (peck depth, not engagement). Bounds for a deep-peck drill cycle
/// must equal the bounds at shallow peck.
#[test]
fn chipload_bounds_skip_doc_derating_for_drill_ops() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let mut input = FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 30.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Drill,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(6.0),
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    };
    let shallow = calculate(&input);
    input.axial_depth_mm = Some(18.0);
    let deep = calculate(&input);
    if let (Some(s), Some(d)) = (shallow.chipload_bounds, deep.chipload_bounds) {
        assert!(
            (s.min_mm_per_tooth - d.min_mm_per_tooth).abs() < 1e-9,
            "drill bounds.min must not derate with peck depth",
        );
        assert!(
            (s.max_mm_per_tooth - d.max_mm_per_tooth).abs() < 1e-9,
            "drill bounds.max must not derate with peck depth",
        );
    }
    // If the LUT doesn't publish drill chipload bounds (None on either
    // depth), the test is moot — derating exclusion is what we care
    // about and both being None already satisfies the contract.
    //
    // Ruling B5 (G6): this cell (a 6 mm 2-flute flat end mill in hard
    // maple) reads the Spektra 6 mm side row through the drill claim. The
    // row prints one value, so it is a point (0.127 / 2 = 0.0635), not a
    // band, and the point must not derate with the peck depth either.
    let (Some(sp), Some(dp)) = (shallow.chipload_point_mm, deep.chipload_point_mm) else {
        panic!(
            "the G6 drill claim serves this cell, so both depths carry a point: {:?} {:?}",
            shallow.chipload_point_mm, deep.chipload_point_mm
        );
    };
    assert!((sp - 0.0635).abs() < 1e-12, "point {sp}");
    assert!(
        (sp - dp).abs() < 1e-12,
        "drill point must not derate with peck depth"
    );
}

#[test]
fn test_lut_rpm_override_within_machine_range() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Vendor RPM for amana 6mm softwood adaptive is 18000
    assert!(
        (result.rpm - 18000.0).abs() < 100.0,
        "RPM should be ~18000 from vendor data, got {}",
        result.rpm
    );
}

#[test]
fn test_no_lut_backward_compatible() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    assert!(result.vendor_source.is_none());
    assert_eq!(result.chipload_source, ChiploadSource::FormulaFallback);
    assert!(result.rpm > 6000.0 && result.rpm < 24000.0);
    assert!(result.feed_rate_mm_min > 500.0);
    assert!(result.chip_load_mm > 0.05 && result.chip_load_mm < 0.12);
}

#[test]
fn test_setup_derate_long_overhang() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let normal = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext {
            tool_overhang_mm: Some(20.0), // L/D = 20/6 = 3.3, no derate
        },
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    let long = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext {
            tool_overhang_mm: Some(40.0), // L/D = 40/6 = 6.67, 25% derate
        },
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Ruling R4 Q7 (2026-09-24): the L/D share (0.75 above 6 x D) lowers the
    // aggressiveness dial's load target in Suggest pass 6b. It does NOT cut
    // the feed any more, so the ratio is 1.0 and the share is on the record.
    let ratio = long.feed_rate_mm_min / normal.feed_rate_mm_min;
    assert!(
        (ratio - 1.0).abs() < 1e-12,
        "the L/D share must not move the feed, got ratio {ratio}"
    );
    assert_eq!(long.derates.ld_overhang, 0.75);
    assert_eq!(normal.derates.ld_overhang, 1.0);
}

#[test]
fn test_setup_derate_medium_overhang() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let normal = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext {
            tool_overhang_mm: Some(20.0), // L/D = 3.3, no derate
        },
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    let medium = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: None,
        setup: SetupContext {
            tool_overhang_mm: Some(30.0), // L/D = 30/6 = 5.0, 12% derate
        },
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Ruling R4 Q7: the 0.88 share is a load target, not a feed factor.
    let ratio = medium.feed_rate_mm_min / normal.feed_rate_mm_min;
    assert!(
        (ratio - 1.0).abs() < 1e-12,
        "the L/D share must not move the feed, got ratio {ratio}"
    );
    assert_eq!(medium.derates.ld_overhang, 0.88);
}

#[test]
fn test_lut_ball_nose_different_chipload_than_flat() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let flat = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });
    let ball = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Ball,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Parallel,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Ball nose finishing should have a different (lower) chipload than flat adaptive
    assert_ne!(
        flat.chip_load_mm, ball.chip_load_mm,
        "LUT should give different chiploads for flat vs ball"
    );
    assert!(
        flat.chip_load_mm > ball.chip_load_mm,
        "flat adaptive chipload {} should be > ball finish chipload {}",
        flat.chip_load_mm,
        ball.chip_load_mm
    );
}

#[test]
fn test_lut_fallback_when_no_match() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::Foam {
        density: crate::material::FoamDensity::Low,
    };
    let machine = MachineProfile::shapeoko_vfd();

    // Foam has no LUT data — should fall back to formula
    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        tool_geometry: ToolGeometryHint::Flat,
        shank_diameter: None,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
    });

    // Foam maps to softwood in normalize, but with hardness 200 which is far from
    // any observation. If it does match, that's fine. If not, formula is used.
    assert!(result.feed_rate_mm_min > 0.0);
}

/// Phase 5 follow-up (2026-06-01): `SpindleStrategy::MaxSpeed`
/// pushes RPM toward the spindle ceiling and scales feed
/// proportionally to keep chipload constant. Sanity-check against
/// a softwood adaptive query whose vendor row publishes
/// rpm_nominal at 18000 with our 24000 RPM ceiling.
#[test]
fn spindle_strategy_max_speed_lifts_rpm_and_scales_feed() {
    let lut = vendor_lut::VendorLut::embedded();
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::GenericSoftwood,
    };
    let base = FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    };
    let match_chart = calculate(&base);
    let max_speed = calculate(&FeedsInput {
        spindle_strategy: SpindleStrategy::MaxSpeed,
        ..base
    });

    // Chipload is preserved (constant-chipload line).
    let chipload_chart =
        match_chart.feed_rate_mm_min / (match_chart.rpm * f64::from(base.flute_count));
    let chipload_max = max_speed.feed_rate_mm_min / (max_speed.rpm * f64::from(base.flute_count));
    assert!(
        (chipload_chart - chipload_max).abs() / chipload_chart < 0.02,
        "chipload should be preserved: chart {chipload_chart:.5} vs max {chipload_max:.5}"
    );

    // RPM is at or above the chart RPM under MaxSpeed (>= because
    // when the chart is already at ceiling there's no headroom).
    assert!(
        max_speed.rpm >= match_chart.rpm - 1.0,
        "MaxSpeed RPM {} should be >= MatchChart RPM {}",
        max_speed.rpm,
        match_chart.rpm,
    );

    // For the GenericSoftwood/6mm/adaptive query the chart is at
    // 18000 RPM and the machine ceiling is 24000 — we should see a
    // meaningful lift.
    let (_, machine_max) = machine.rpm_range();
    let speedup = max_speed.rpm / match_chart.rpm;
    let ceiling = machine_max * SPINDLE_CEILING_HEADROOM;
    assert!(
        speedup > 1.05 || max_speed.rpm >= ceiling * 0.99,
        "expected meaningful speedup or to hit the ceiling: speedup={speedup:.2}, rpm={}",
        max_speed.rpm
    );

    // FeedsDerates.spindle_scale tracks the multiplier for UI.
    assert!(
        (max_speed.derates.spindle_scale - speedup).abs() < 0.05,
        "derates.spindle_scale {} should match observed RPM speedup {}",
        max_speed.derates.spindle_scale,
        speedup,
    );

    // combined_factor (chipload multiplier) does NOT include
    // spindle_scale — it's purely a speed-axis change.
    let factor_max = max_speed.derates.combined_factor();
    let factor_chart = match_chart.derates.combined_factor();
    assert!(
        (factor_max - factor_chart).abs() / factor_chart.max(1e-6) < 0.02,
        "combined_factor should be unchanged by spindle strategy: \
         chart={factor_chart:.4} vs max={factor_max:.4}"
    );
}

/// `MaxSpeed` caps at `MAX_SPINDLE_SPEEDUP` even if the spindle
/// ceiling/chart ratio is bigger. This guards against unbounded
/// extrapolation past the chart's tested envelope.
#[test]
fn spindle_strategy_max_speed_respects_hard_cap() {
    // Construct a fake low-RPM scenario: pick a machine with a
    // very high ceiling (synthesise a MachineProfile if needed).
    // Easier: just verify the cap constant is sensible and the
    // observed speedup never exceeds it in `derates`.
    let lut = vendor_lut::VendorLut::embedded();
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::GenericSoftwood,
    };
    let result = calculate(&FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 18.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MaxSpeed,
    });
    assert!(
        result.derates.spindle_scale <= MAX_SPINDLE_SPEEDUP + 1e-6,
        "spindle_scale {} must not exceed MAX_SPINDLE_SPEEDUP {}",
        result.derates.spindle_scale,
        MAX_SPINDLE_SPEEDUP,
    );
}

// ── Phase 3 CutterKind pins (architectural refactor 2026-06-06) ──

/// Every `ToolGeometryHint` shape maps to exactly the expected
/// `CutterKind`, and the centralized LUT-family map matches the
/// (formerly triplicated) `ToolGeometryHint → ToolFamily` table
/// verbatim. A new hint variant fails to compile in
/// `cutter_kind()`; a changed family mapping fails here.
#[test]
fn cutter_kind_lut_family_map_is_pinned() {
    use vendor_lut::ToolFamily;
    let cases = [
        (
            ToolGeometryHint::Flat,
            CutterKind::Flat,
            ToolFamily::FlatEnd,
        ),
        (
            ToolGeometryHint::Ball,
            CutterKind::Ball,
            ToolFamily::BallNose,
        ),
        (
            ToolGeometryHint::Bull { corner_radius: 1.0 },
            CutterKind::Bull,
            ToolFamily::BullNose,
        ),
        (
            ToolGeometryHint::VBit {
                included_angle: 60.0,
                tip_diameter: 0.1,
            },
            CutterKind::VBit,
            ToolFamily::ChamferVbit,
        ),
        (
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 10.0,
            },
            CutterKind::TaperedBall,
            ToolFamily::TaperedBallNose,
        ),
    ];
    assert_eq!(cases.len(), CutterKind::ALL.len());
    for (hint, kind, family) in cases {
        assert_eq!(hint.cutter_kind(), kind, "{hint:?}");
        assert_eq!(kind.lut_family(), family, "{kind:?}");
    }
}

/// Named decision: `ToolFamily::FacingBit` is LUT row vocabulary
/// only — no cutter shape classifies to it. If a facing cutter
/// ever becomes a real `CutterKind`, this pin forces the
/// compatibility story (LUT routing, constraints) to be revisited
/// deliberately rather than inherited.
#[test]
fn facing_bit_is_a_lut_only_family() {
    for &kind in CutterKind::ALL {
        assert_ne!(
            kind.lut_family(),
            vendor_lut::ToolFamily::FacingBit,
            "{kind:?} must not classify to the data-only FacingBit family"
        );
    }
}

/// `ToolType ↔ CutterKind` is a bijection (5 ↔ 5): the convenience
/// `ToolType::cutter_kind()` and the registry-facing
/// `CutterKind::tool_type()` must be mutual inverses. A 6th cutter
/// breaking the bijection fails here and forces a decision.
#[test]
fn tool_type_cutter_kind_round_trips() {
    use crate::compute::ToolType;
    for &tool_type in ToolType::ALL {
        assert_eq!(tool_type.cutter_kind().tool_type(), tool_type);
    }
    for &kind in CutterKind::ALL {
        assert_eq!(kind.tool_type().cutter_kind(), kind);
    }
    assert_eq!(ToolType::ALL.len(), CutterKind::ALL.len());
}

/// S.3 parity sentry (`planning/finishing_stack_review_2026-07.md`).
///
/// `ToolGeometryHint::engaged_diameter_at_doc` (Suggest's path —
/// this module, ~line 108) and `MillingCutter::lookup_diameter_at`
/// (the post-sim gate's path — `tool::vbit::VBitEndmill` /
/// `tool::tapered_ball::TaperedBallEndmill`) are two hand-written
/// implementations of the same engaged-diameter geometry. They
/// agree today, but nothing enforces it — one edit to either side
/// could silently split Suggest's chipload-band targeting from the
/// gate's derating and produce false chipload trips.
///
/// Sweeps DOC across 0.1×..2× nominal diameter (crossing the
/// ball/cone and flute/taper transitions) for every shape with
/// nontrivial engagement geometry (v-bit, tapered ball), plus flat
/// and ball as trivial always-nominal-diameter cases, and asserts
/// exact agreement (1e-9) at every sample. If this ever fails: DO
/// NOT silently pick one implementation over the other — report
/// the numeric disagreement and mark this `#[ignore]` with a
/// pointer to the report so the tree stays green while the
/// implementations are reconciled deliberately.
#[test]
fn engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes() {
    use crate::tool::{BallEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill};

    struct Case {
        name: &'static str,
        diameter_mm: f64,
        shank_mm: f64,
        hint: ToolGeometryHint,
        lookup: Box<dyn Fn(f64) -> f64>,
    }

    let flat = FlatEndmill::new(6.0, 20.0);
    let ball = BallEndmill::new(6.0, 20.0);
    let vbit = VBitEndmill::new(6.0, 90.0, 20.0);
    // ball_diameter=6mm tip, taper_half_angle=20deg, shaft=10mm.
    let tapered = TaperedBallEndmill::new(6.0, 20.0, 10.0, 30.0);

    let cases: Vec<Case> = vec![
        Case {
            name: "flat (trivial: nominal diameter regardless of DOC)",
            diameter_mm: 6.0,
            shank_mm: 6.0,
            hint: ToolGeometryHint::Flat,
            lookup: Box::new(move |doc| flat.lookup_diameter_at(doc)),
        },
        Case {
            name: "ball (trivial: nominal diameter regardless of DOC)",
            diameter_mm: 6.0,
            shank_mm: 6.0,
            hint: ToolGeometryHint::Ball,
            lookup: Box::new(move |doc| ball.lookup_diameter_at(doc)),
        },
        Case {
            name: "vbit 90deg",
            diameter_mm: 6.0,
            shank_mm: 6.0,
            hint: ToolGeometryHint::VBit {
                included_angle: 90.0,
                tip_diameter: 0.0,
            },
            lookup: Box::new(move |doc| vbit.lookup_diameter_at(doc)),
        },
        Case {
            name: "tapered ball, tip r=3mm, taper=20deg, shaft=10mm",
            diameter_mm: 10.0,
            shank_mm: 10.0,
            hint: ToolGeometryHint::TaperedBall {
                tip_radius: 3.0,
                taper_angle_deg: 20.0,
            },
            lookup: Box::new(move |doc| tapered.lookup_diameter_at(doc)),
        },
    ];

    const STEPS: u32 = 40;
    for case in &cases {
        for i in 0..=STEPS {
            let frac = 0.1 + (2.0 - 0.1) * f64::from(i) / f64::from(STEPS);
            let doc_mm = frac * case.diameter_mm;
            let hint_d = case
                .hint
                .engaged_diameter_at_doc(doc_mm, case.diameter_mm, case.shank_mm);
            let trait_d = (case.lookup)(doc_mm);
            let diff = (hint_d - trait_d).abs();
            assert!(
                diff < 1e-9,
                "{}: at doc={doc_mm:.4}mm engaged_diameter_at_doc={hint_d:.9} \
                 lookup_diameter_at={trait_d:.9} (diff={diff:.3e}) — S.3 divergence, \
                 see planning/finishing_stack_review_2026-07.md",
                case.name,
            );
        }
    }
}
