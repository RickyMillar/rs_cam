//! Unit tests for the feeds calculator. Moved out of `feeds/mod.rs` by P4.
//! P29-7 7a: each input comes from an op (`FeedsInput::of_operation`).

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use super::*;
use crate::compute::catalog::{OperationConfig, OperationType};
use crate::compute::tool_config::{ToolConfig, ToolFact};
use crate::ids::ToolId;
use crate::machine::MachineProfile;
use crate::material::{Material, WoodSpecies};

/// The test tool of `hint`: the diameter `d`, `flutes`, the flute length
/// `flute_mm` and the shank `shank_mm`, each fact stated. The stick-out is
/// 4 x D: at that ratio the long-tool share is 1
/// (`long_tool_load_share`), as for an input with no overhang.
fn tool(hint: ToolGeometryHint, d: f64, flutes: u32, flute_mm: f64, shank_mm: f64) -> ToolConfig {
    let mut builder = ToolConfig::builder(ToolId::from_raw(1), hint.cutter_kind().tool_type())
        .diameter(d)
        .flutes(flutes)
        .flute_mm(flute_mm)
        .shank_mm(shank_mm)
        .stickout_mm(4.0 * d);
    let mut v_tip = None;
    match hint {
        ToolGeometryHint::Flat | ToolGeometryHint::Ball => {}
        ToolGeometryHint::Bull { corner_radius } => builder = builder.corner_radius(corner_radius),
        ToolGeometryHint::VBit {
            included_angle,
            tip_diameter,
        } => {
            builder = builder.included_angle_deg(included_angle);
            v_tip = Some(tip_diameter);
        }
        ToolGeometryHint::TaperedBall {
            tip_radius,
            taper_angle_deg,
        } => {
            assert!(
                (2.0 * tip_radius - d).abs() < 1e-12,
                "the tip is the diameter"
            );
            builder = builder
                .taper_half_angle_deg(taper_angle_deg)
                .shaft_mm(shank_mm);
        }
    }
    let mut tool = builder.build();
    if let Some(tip) = v_tip {
        tool.set_v_tip_diameter(ToolFact::Stated(tip));
    }
    assert_eq!(tool.geometry_hint(), hint, "the tool states the hint");
    tool
}

/// The default op of `kind` with the stated depth per pass `ap` and the
/// stated stepover `ae`. An op that holds no such field panics: a test
/// states only what an op gives.
fn op(kind: OperationType, ap: Option<f64>, ae: Option<f64>) -> OperationConfig {
    let mut op = OperationConfig::new_default(kind);
    if let Some(ap) = ap {
        assert!(
            op.set_depth_per_pass(ap),
            "{kind:?} holds no depth per pass"
        );
    }
    if let Some(ae) = ae {
        assert!(op.set_stepover(ae), "{kind:?} holds no stepover");
    }
    op
}

/// A VCarve to the stated depth `max_depth`.
fn vcarve_to_depth(max_depth: f64) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::VCarve);
    let OperationConfig::VCarve(c) = &mut op else {
        panic!("a VCarve config");
    };
    c.max_depth = Some(max_depth);
    op
}

/// A drop cutter at the scallop target `scallop_mm`. A drop cutter holds no
/// depth.
fn drop_cutter_to_scallop(scallop_mm: f64) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::DropCutter);
    op.set_scallop_height(scallop_mm);
    op
}

/// The calculator input of `op` on `tool` with no vendor table: the
/// formula path ([`FeedsInput::with_no_vendor_table`]).
fn formula_input<'a>(
    op: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
) -> FeedsInput<'a> {
    FeedsInput::of_operation(
        op,
        tool,
        material,
        machine,
        embedded_vendor_lut(),
        SpindleStrategy::default(),
    )
    .with_no_vendor_table()
}

/// The calculator input of `op` on `tool` with the vendor table `lut`.
fn table_input<'a>(
    op: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    lut: &'a VendorLut,
) -> FeedsInput<'a> {
    table_input_with(op, tool, material, machine, lut, SpindleStrategy::default())
}

/// [`table_input`] with the spindle policy `strategy`.
fn table_input_with<'a>(
    op: &OperationConfig,
    tool: &'a ToolConfig,
    material: &'a Material,
    machine: &'a MachineProfile,
    lut: &'a VendorLut,
    strategy: SpindleStrategy,
) -> FeedsInput<'a> {
    FeedsInput::of_operation(op, tool, material, machine, lut, strategy)
}

fn softwood_flat_6mm_pocket() -> FeedsInput<'static> {
    // We need 'static material/machine so use leaked boxes for test convenience
    let material: &'static Material = Box::leak(Box::new(Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }));
    let machine: &'static MachineProfile = Box::leak(Box::new(MachineProfile::shapeoko_vfd()));
    let tool: &'static ToolConfig =
        Box::leak(Box::new(tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0)));
    formula_input(
        &op(OperationType::Face, None, None),
        tool,
        material,
        machine,
    )
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
    assert!(
        result.power_kw.is_some_and(|kw| kw >= 0.0),
        "power {:?}",
        result.power_kw
    );
}

#[test]
fn test_adaptive_deeper_narrower_than_pocket() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let adaptive = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));
    let pocket = calculate(&formula_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    // P29-7 7a: the input comes from an op. Of the families of the old
    // loop, only the Contour family has an op of each role: Profile
    // (roughing) and Steep/Shallow (finish). No op gives a roughing pass of
    // the Parallel, Scallop or Trace family, a finish pass of the Pocket or
    // Adaptive family, or the Face family.
    let flat = tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0);
    let rough = calculate(&formula_input(
        &op(OperationType::Profile, None, None),
        &flat,
        &material,
        &machine,
    ));
    let finish = calculate(&formula_input(
        &op(OperationType::SteepShallow, None, None),
        &flat,
        &material,
        &machine,
    ));

    assert!(
        rough.axial_depth_mm >= finish.axial_depth_mm,
        "Contour: roughing DOC {} should >= finishing DOC {}",
        rough.axial_depth_mm,
        finish.axial_depth_mm
    );
    assert!(
        rough.radial_width_mm >= finish.radial_width_mm,
        "Contour: roughing WOC {} should >= finishing WOC {}",
        rough.radial_width_mm,
        finish.radial_width_mm
    );
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

    let result = calculate(&formula_input(
        &op(OperationType::DropCutter, None, None),
        &tool(
            ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 7.0,
            },
            1.0,
            1,
            6.0,
            6.0,
        ),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

    // The G10 flat claim: plunge = floor(F / Z) at Z 2; no tip cap.
    // Re-pinned by feeds contract P18 (review D14): F is the feed BEFORE the
    // radial chip-thinning lift, `feed / derates.radial_chip_thinning`. The
    // Adaptive default width is narrow, so the lift is above 1 here
    // (non-vacuity below); before P18 the pin read the lifted feed.
    let claim = result
        .plunge
        .claim()
        .unwrap_or_else(|| panic!("6mm 2F flat: basis {:?}", result.plunge));
    assert_eq!(claim.rule.id, "g10_plunge_flat");
    assert_eq!(result.plunge.tip_cap(), None);
    let lift = result.derates.radial_chip_thinning;
    assert!(
        lift > 1.0,
        "the Adaptive default width lifts the feed: {lift}"
    );
    let unlifted = result.feed_rate_mm_min / lift;
    assert_eq!(
        result.plunge_rate_mm_min,
        (unlifted / 2.0).floor(),
        "6mm 2F flat plunge {} should be floor(feed {} / lift {lift} / 2)",
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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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
/// own `OperationFamily::Drill` (was `OperationFamily::Pocket`).
///
/// P20a (2026-11-08) removed the drill RPM tier (8-14k at Ø6), which had
/// no source. A drill with no vendor row takes the Step 1 RPM: the
/// surface speed at the diameter, clamped to the machine range. At Ø6 the
/// formula feed stays below the drill feed ceiling, so no follow-down
/// moves it.
///
/// Ruling B5 (2026-09-24) deleted the drill multiplier (2.5). The old
/// "0.05 mm/rev softwood drill floor" was a band fitted to formula x
/// 2.5 (W6 audit), so this test no longer asserts it. It asserts that
/// the preview chip IS the milling formula with no multiplier. With no
/// LUT the cell refuses (no G6 claim), and the number is a preview.
#[test]
fn test_drill_family_rpm_is_the_surface_speed_rpm() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&formula_input(
        &op(OperationType::Drill, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

    // The RPM is the Step 1 surface-speed RPM in the machine range.
    let speed = material.base_cutting_speed_constant().value;
    let step1 = machine.clamp_rpm(speed * 1000.0 / (std::f64::consts::PI * 6.0));
    assert!(
        (result.rpm - step1).abs() < 1e-9,
        "drill RPM {} must be the surface-speed RPM {step1}",
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
            let result = calculate(&formula_input(
                &op(OperationType::Drill, None, None),
                &tool(ToolGeometryHint::Flat, d, 2, 18.0, d),
                &material,
                &machine,
            ));
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
/// (bounded only by the machine minimum since P20a) so the result keeps
/// its chipload instead of thinning toward rubbing.
///
/// Rebaselined for ruling B5 (2026-09-24). The old fixture (Ø3 oak,
/// "~0.107 mm/tooth") rested on formula x 2.5; without the multiplier
/// Ø3 oak sits at about 290 mm/min per mm, under the 580 ceiling. The
/// new fixture is a Ø2 3-flute formula preview in GenericSoftwood (no
/// LUT, so the cell refuses and the number is a preview):
///
/// P20a (2026-11-08) removed the drill RPM cap and re-derived the
/// ceiling (72.5 in/min x 25.4 / 3.0 = 613.83 per mm):
///
/// - RPM: 200 000 / (π × 2) = 31 831 → machine 24 000;
/// - chip: k0 × 2^p = 0.024 × 2^0.61 = 0.03663 mm/tooth;
/// - feed: 24 000 × 0.03663 × 3 = 2 637 mm/min, over the ceiling
///   613.83 × 2 = 1 227.7;
/// - follow-down: 1 227.7 / (0.03663 × 3) = 11 172 RPM, above the machine
///   minimum, so the chip holds at 0.03663.
#[test]
fn test_drill_envelope_ceiling_drops_rpm_to_hold_chipload() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let result = calculate(&formula_input(
        &op(OperationType::Drill, None, None),
        &tool(ToolGeometryHint::Flat, 2.0, 3, 12.0, 2.0),
        &material,
        &machine,
    ));
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
    let speed = material.base_cutting_speed_constant().value;
    let step1 = machine.clamp_rpm(speed * 1000.0 / (std::f64::consts::PI * 2.0));
    assert!(
        result.rpm < step1,
        "RPM must follow the capped feed down from {step1}, got {}",
        result.rpm
    );
    let (machine_min, _) = machine.rpm_range();
    assert!(
        result.rpm >= machine_min,
        "follow-down bounded by the machine minimum {machine_min}, got {}",
        result.rpm
    );
    let cl = &machine.chip_load;
    let formula = cl.k0 * 2.0_f64.powf(cl.p);
    assert!(
        (result.rpm - hi * 2.0 / (formula * 3.0)).abs() < 1e-6,
        "the follow-down RPM is the ceiling over (chip x Z), got {}",
        result.rpm
    );
    let implied = result.feed_rate_mm_min / (3.0 * result.rpm);
    assert!(
        (implied / formula - 1.0).abs() < 1e-9,
        "the follow-down must hold the chip {formula:.5}, got {implied:.5}"
    );
}

/// F1: the envelope clamp warns when it binds — never a silent
/// rewrite. The Ø2 3-flute softwood preview of
/// `test_drill_envelope_ceiling_drops_rpm_to_hold_chipload` drives the
/// feed (2 637) above the envelope max (613.83 × 2 = 1 227.7; P20a), so
/// the clamp must fire with the honest before/after. Before B5 the
/// fixture was Ø3 2-flute, which formula x 2.5 put over 400 × 3; without
/// the multiplier it sits in band and the warning check was vacuous.
#[test]
fn test_drill_feed_clamp_emits_warning_when_binding() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let result = calculate(&formula_input(
        &op(OperationType::Drill, None, None),
        &tool(ToolGeometryHint::Flat, 2.0, 3, 12.0, 2.0),
        &material,
        &machine,
    ));
    let ratio = result.feed_rate_mm_min / 2.0;
    let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
    assert!(ratio >= lo - 1e-9 && ratio <= hi + 1e-9);
    let clamped = result.warnings.iter().find_map(|w| match w {
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested, actual, ..
        } => Some((*requested, *actual)),
        FeedsWarning::FeedRateClamped { .. }
        | FeedsWarning::RpmLoweredForFeedCeiling { .. }
        | FeedsWarning::PowerLimited { .. }
        | FeedsWarning::PowerLadderReducedCut { .. }
        | FeedsWarning::ShankTooLarge { .. }
        | FeedsWarning::DepthJudgedAfterSimulation
        | FeedsWarning::DocExceedsFlute { .. }
        | FeedsWarning::CuttingCapUnstated { .. }
        | FeedsWarning::ScallopInvalid { .. }
        | FeedsWarning::ChiploadBelowRubbingFloor { .. }
        | FeedsWarning::LongToolDerate { .. }
        | FeedsWarning::VendorRowPublishesNoChipload { .. }
        | FeedsWarning::NoVendorRowsForRoutedOperation { .. }
        | FeedsWarning::FormulaChipNoPrintedRow { .. }
        | FeedsWarning::DrillFeedAtZAxisRate { .. } => None,
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
/// takes the surface-speed RPM in the machine range. P20a removed the
/// drill RPM clamp, so no family-selective clamp is left to guard.
#[test]
fn test_pocket_family_unaffected_by_drill_fix() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&formula_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

    // The pocket RPM is in the machine range.
    assert!(
        result.rpm == machine.clamp_rpm(result.rpm),
        "pocket RPM {} must be in the machine range",
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
        crate::machine::SpindleConfig::Discrete { .. } => {
            panic!("expected variable spindle on shapeoko_vfd")
        }
    };

    // 20° V-bit, 5.5 mm shank, at 0.5 mm DOC. Engaged tip diameter
    // = 2 * 0.5 * tan(10°) ≈ 0.176 mm — small enough that SFM-derived
    // ideal RPM exceeds the machine ceiling, so the result clamps
    // to machine max.
    let result = calculate(&formula_input(
        &vcarve_to_depth(0.5),
        &tool(
            ToolGeometryHint::VBit {
                included_angle: 20.0,
                tip_diameter: 0.0,
            },
            5.5,
            2,
            12.0,
            5.5,
        ),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, Some(3.0), None),
        &tool(ToolGeometryHint::Flat, d, 2, 18.0, d),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 5.0, 6.0),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Face, Some(5.0), Some(8.0)),
        &tool(ToolGeometryHint::Flat, 12.0, 4, 25.0, 12.0),
        &material,
        &machine,
    ));

    assert!(
        result.power_limited,
        "should be power limited with 0.01kW spindle: power={:?}kW, available={:.4}kW",
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

    let result = calculate(&formula_input(
        &drop_cutter_to_scallop(0.03),
        &tool(ToolGeometryHint::Ball, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

    assert!(result.feed_rate_mm_min > 0.0);
    let d = &result.derates;
    assert!(
        (d.combined_factor() - d.radial_chip_thinning * d.power_limit * d.feed_clamp).abs() < 1e-12
    );
    let fpt = result.feed_rate_mm_min / (result.rpm * 2.0);
    assert!((fpt - d.effective_chip_load_mm()).abs() <= fpt * 1e-9);
}

/// P16 (operator ruling D12): no slot depth cap. A stated full-width slot
/// depth ships, and the force budget is the only depth rule of a slot.
#[test]
fn a_stated_slot_depth_ships_with_no_slot_cap_p16() {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&formula_input(
        &op(OperationType::Pocket, Some(10.0), Some(5.5)),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

    // Feeds contract P11b: the stated depth ships.
    assert!(
        (result.axial_depth_mm - 10.0).abs() < 1e-12,
        "a stated slot depth ships, got {}",
        result.axial_depth_mm
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

    let with_lut = calculate(&table_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
        &lut,
    ));
    let without_lut = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

/// Feeds contract P11b (L6, chip first): the band does not move with the
/// depth. Until P11b it took the vendor depth table (0.5 at 3 x D); the
/// table is now a limit of the envelope.
///
/// Fixture since feeds matrix R5 (2026-09-23): a Ø6.35 ball nose. The
/// flat 6 mm hardwood pocket cell now resolves to the printed Amana
/// Spektra row, which publishes one value and therefore no band; the
/// printed Amana ball-nose v7 row `amana-ball-hardwood-pocket-6350-2f-v7`
/// (0.127-0.1778 mm/tooth) publishes both bounds.
#[test]
fn chipload_bounds_hold_at_every_depth_p11b() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let ball = tool(ToolGeometryHint::Ball, 6.35, 2, 24.0, 6.35);
    let input = table_input(
        &op(OperationType::Face, Some(6.35), None),
        &ball,
        &material,
        &machine,
        &lut,
    );
    let at_1x = calculate(&input);
    let input = table_input(
        &op(OperationType::Face, Some(19.05), None),
        &ball,
        &material,
        &machine,
        &lut,
    );
    let at_3x = calculate(&input);

    // The printed Amana ball-nose v7 row publishes both bounds,
    // so the orchestrator must produce `Some(ChiploadBounds)` here.
    let b1 = at_1x.chipload_bounds.unwrap();
    let b3 = at_3x.chipload_bounds.unwrap();
    assert_eq!(b1, b3, "L6: the band at 3 x D is the band at 1 x D");
}

/// Ruling B5 (G6): a drill cycle reads the point of its side row. This
/// cell (a 6 mm 2-flute flat end mill in hard maple) reads the Spektra
/// 6 mm side row through the drill claim. The row prints one value, so it
/// is a point (0.127 / 2 = 0.0635), not a band.
///
/// P29-7 7a: the old test compared a shallow and a deep peck. No op gives
/// the calculator a drill depth (the Drill op states no side-cut depth,
/// `CutSpec::calc_inputs`), so the depth half of the claim has no input
/// that an op can give, and the test keeps the point only.
#[test]
fn a_drill_cycle_reads_the_point_of_its_side_row() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let result = calculate(&table_input(
        &op(OperationType::Drill, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 30.0, 6.0),
        &material,
        &machine,
        &lut,
    ));
    let Some(point) = result.chipload_point_mm else {
        panic!(
            "the G6 drill claim serves this cell, so it carries a point: {:?}",
            result.chipload_point_mm
        );
    };
    assert!((point - 0.0635).abs() < 1e-12, "point {point}");
}

#[test]
fn test_lut_rpm_override_within_machine_range() {
    let lut = vendor_lut::VendorLut::embedded();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let machine = MachineProfile::shapeoko_vfd();

    let result = calculate(&table_input(
        &op(OperationType::Adaptive, None, Some(3.0)),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
        &lut,
    ));

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

    let result = calculate(&formula_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
    ));

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

    let normal = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0)
            .to_builder()
            .stickout_mm(20.0)
            .build(),
        &material,
        &machine,
    ));

    let long = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0)
            .to_builder()
            .stickout_mm(40.0)
            .build(),
        &material,
        &machine,
    ));

    // Ruling R4 Q7 (2026-09-24): the L/D share (0.75 above 6 x D) does NOT
    // cut the feed any more, so the ratio is 1.0 and the share is on the
    // record.
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

    let normal = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0)
            .to_builder()
            .stickout_mm(20.0)
            .build(),
        &material,
        &machine,
    ));

    let medium = calculate(&formula_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0)
            .to_builder()
            .stickout_mm(30.0)
            .build(),
        &material,
        &machine,
    ));

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

    let flat = calculate(&table_input(
        &op(OperationType::Adaptive, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
        &lut,
    ));
    let ball = calculate(&table_input(
        &op(OperationType::DropCutter, None, None),
        &tool(ToolGeometryHint::Ball, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
        &lut,
    ));

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
    let result = calculate(&table_input(
        &op(OperationType::Face, None, None),
        &tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0),
        &material,
        &machine,
        &lut,
    ));

    // Foam maps to softwood in normalize, but with hardness 200 which is far from
    // any observation. If it does match, that's fine. If not, formula is used.
    assert!(result.feed_rate_mm_min > 0.0);
}

/// A machine that states its axis rates and accelerations: the Shapeoko
/// VFD preset with the values that
/// `planning/rivmap_block_job/nz_wanaka_21_operator.toml` prints
/// (`max_feed_mm_min`, `[job.machine.kinematics]`).
fn machine_with_stated_rates() -> MachineProfile {
    MachineProfile {
        max_feed_mm_min: 15_000.0,
        ..MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0])
}

/// Feeds contract P21c: `SpindleStrategy::ConstantChip` holds the chip and
/// raises the RPM to the first named limit. A softwood adaptive row prints
/// 18000 RPM; with a ceiling of 24000 on a 24000 RPM spindle the spindle
/// maximum and the operator ceiling tie, and the RPM is 24000.
#[test]
fn constant_chip_lifts_the_rpm_to_the_first_limit_and_holds_the_chip() {
    let lut = vendor_lut::VendorLut::embedded();
    let machine = machine_with_stated_rates();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::GenericSoftwood,
    };
    let flat = tool(ToolGeometryHint::Flat, 6.0, 2, 18.0, 6.0);
    let base = table_input_with(
        &op(OperationType::Adaptive, None, Some(3.0)),
        &flat,
        &material,
        &machine,
        &lut,
        SpindleStrategy::MatchChart,
    );
    let match_chart = calculate(&base);
    assert!(match_chart.rpm_choice.is_none(), "MatchChart has no choice");
    let constant_chip = calculate(&table_input_with(
        &op(OperationType::Adaptive, None, Some(3.0)),
        &flat,
        &material,
        &machine,
        &lut,
        SpindleStrategy::ConstantChip {
            rpm_ceiling: Some(24_000),
        },
    ));
    let (_, spindle_max) = machine.rpm_range();
    assert_eq!(constant_chip.rpm, spindle_max, "the RPM is the spindle max");
    let choice = constant_chip.rpm_choice.as_ref();
    assert_eq!(choice.map(|c| c.rpm), Some(spindle_max));
    assert_eq!(choice.and_then(|c| c.chart_rpm), Some(match_chart.rpm));
    // The spindle maximum and the ceiling tie; the first of the list binds.
    assert_eq!(
        choice.map(|c| c.binding),
        Some(crate::feeds::RpmLimit::SpindleMax)
    );
    assert_eq!(
        constant_chip.derates.spindle_scale_reason,
        SpindleScaleReason::ConstantChip
    );

    // The chip holds: the feed over (RPM x Z) is the same.
    let chip =
        |r: &FeedsResult| r.feed_rate_mm_min / (r.rpm * f64::from(flat.flute_count_planning()));
    assert!(
        (chip(&match_chart) - chip(&constant_chip)).abs() < 1e-9,
        "chip {} vs {}",
        chip(&match_chart),
        chip(&constant_chip)
    );
    // `spindle_scale` is the RPM over the chart RPM; no multiplier.
    let scale = constant_chip.rpm / match_chart.rpm;
    assert!(
        (constant_chip.derates.spindle_scale - scale).abs() < 1e-12,
        "spindle_scale {} vs {scale}",
        constant_chip.derates.spindle_scale
    );
    // The chip-load factor does not read the speed axis.
    assert!(
        (constant_chip.derates.combined_factor() - match_chart.derates.combined_factor()).abs()
            < 1e-9
    );

    // An empty ceiling: the RPM stays at the chart RPM.
    let no_ceiling = calculate(&table_input_with(
        &op(OperationType::Adaptive, None, Some(3.0)),
        &flat,
        &material,
        &machine,
        &lut,
        SpindleStrategy::ConstantChip { rpm_ceiling: None },
    ));
    assert_eq!(no_ceiling.rpm, match_chart.rpm);
    assert_eq!(no_ceiling.feed_rate_mm_min, match_chart.feed_rate_mm_min);
    assert_eq!(
        no_ceiling.rpm_choice.as_ref().map(|c| c.binding),
        Some(crate::feeds::RpmLimit::ChartRpm)
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
    let vbit = VBitEndmill::new(6.0, 90.0, 20.0).unwrap();
    // ball_diameter=6mm tip, taper_half_angle=20deg, shaft=10mm.
    let tapered = TaperedBallEndmill::new(6.0, 20.0, 10.0, 30.0).unwrap();

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
