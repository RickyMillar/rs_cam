//! Unit tests for the canonical Suggest path. Moved out of `feeds/suggest.rs`
//! by P4; the module body is unchanged.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use super::axial_envelope::recompute_chipload_bounds_for_dpp;
use super::invariants::{DEFLECTION_BACKOFF_TARGET_UM, enforce_invariants_at_fixed_geometry};
use crate::compute::tool_config::ToolFact;

/// The invariant chain of a speeds write on `operation` at the geometry it
/// keeps (feeds contract P5: the geometry passes are deleted, so this is
/// the whole chain). The calculator geometry is the operation's own, so
/// pass 9 sees no move.
fn fixed_chain(
    operation: &mut OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    material: &Material,
    pass_role: PassRole,
    context: SuggestContext<'_>,
) -> Vec<SuggestWarning> {
    let (stepover, dpp) = (operation.stepover(), operation.depth_per_pass());
    enforce_invariants_at_fixed_geometry(
        operation, tool, machine, material, pass_role, context, stepover, dpp,
    )
}
use super::*;
use crate::compute::operation_configs::{DropCutterConfig, PocketConfig};
use crate::compute::tool_config::{ToolId, ToolType};
use crate::feeds::{ChiploadSource, EMBEDDED_LUT, PassRole};

/// Feeds contract P11b (L6, chip first): the re-derived band is the printed
/// band whether the effective diameter is populated or not, at any DPP.
#[test]
fn the_rederived_band_holds_at_every_depth_p11b() {
    let row = crate::feeds::vendor_lookup::LookupResult {
        chip_load_mm: 0.1,
        chip_load_min_mm: Some(0.05),
        chip_load_max_mm: Some(0.20),
        rpm_nominal: None,
        rpm_min: None,
        rpm_max: None,
        ap_min_mm: None,
        ap_max_mm: None,
        ap_min_factor: None,
        ap_max_factor: None,
        ae_min_mm: None,
        ae_max_mm: None,
        observation_id: "c2-sentinel".to_owned(),
        source_vendor: crate::feeds::vendor_lut::Vendor::Amana,
        score: 100,
        diameter_match_score: 200,
        row_diameter_mm: 6.0,
        chipload_diameter_scale: 1.0,
        chipload_hardness_scale: 1.0,
        chipload_diameter_ratio_raw: 1.0,
        chipload_hardness_ratio_raw: 1.0,
        is_extrapolated: false,
        row_pass_role: crate::feeds::vendor_lut::LutPassRole::Roughing,
        size_basis: crate::feeds::extrapolation::SizeBasis::Exact,
        hardness_basis: crate::feeds::extrapolation::HardnessBasis::Unscaled,
        family_basis: crate::feeds::extrapolation::FamilyBasis::Printed,
        drill_basis: crate::feeds::extrapolation::DrillBasis::Printed,
        material_label: String::new(),
        evidence_grade: crate::feeds::vendor_lut::EvidenceGrade::A,
        row_kind: crate::feeds::vendor_lut::ObservationKind::Exact,
        tied_with: None,
    };
    let op = OperationConfig::new_default(OperationType::Pocket);

    // Not populated → raw LUT band, no derating.
    let unpopulated = recompute_chipload_bounds_for_dpp(Some(&row), 0.0, &op, 12.0)
        .expect("a row with a full band must yield bounds");
    assert!(
        (unpopulated.max_mm_per_tooth - 0.20).abs() < 1e-12
            && (unpopulated.min_mm_per_tooth - 0.05).abs() < 1e-12,
        "unpopulated effective diameter must pass the raw band through, got {unpopulated:?}"
    );

    // Feeds contract P11b (L6, chip first): a populated diameter and a DPP
    // twice the diameter leave the band as printed too.
    let deep = recompute_chipload_bounds_for_dpp(Some(&row), 6.0, &op, 12.0)
        .expect("a row with a full band must yield bounds");
    assert_eq!(deep, unpopulated, "L6: the depth does not move the band");
}

/// Ball-nose tool of the given diameter (tip radius = diameter / 2).
fn ball_tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::BallNose);
    tool.diameter = diameter;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool
}

/// DropCutter (3D Finish) operation with an optional scallop target.
fn dropcutter_op(scallop_height: Option<f64>) -> OperationConfig {
    let mut op = OperationConfig::DropCutter(DropCutterConfig {
        scallop_height,
        ..DropCutterConfig::default()
    });
    // P10: the fixture states the stepover the registry held before P10.
    crate::compute::fixture_values::fill_unset_geometry(&mut op);
    op
}

/// Run the canonical feeds path for a DropCutter op + tool and return
/// the suggested radial stepover (mm).
fn dropcutter_stepover(op: &OperationConfig, tool: &ToolConfig) -> f64 {
    feeds_result_for_operation(
        op,
        tool,
        &Material::default(),
        &MachineProfile::default(),
        &EMBEDDED_LUT,
        crate::feeds::SpindleStrategy::default(),
    )
    .expect("dropcutter stepover should not be refused for ball tool")
    .radial_width_mm
}

/// S1: a scallop target on DropCutter overrides the `ae_factor`
/// formula stepover with the chord-height geometry stepover. A 10 μm
/// scallop on a 1 mm ball (tip r = 0.5 mm) gives
/// `2·√(2·0.5·0.010 − 0.010²) ≈ 0.199 mm` — practical wood finish —
/// instead of the sub-micron formula value.
#[test]
fn scallop_height_some_overrides_default_ae_factor() {
    let stepover = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
    let expected = 2.0 * (2.0 * 0.5 * 0.010 - 0.010_f64.powi(2)).sqrt();
    assert!(
        (stepover - expected).abs() < 1e-6,
        "scallop stepover {stepover} should match chord-height {expected}"
    );
    assert!(
        (0.198..0.200).contains(&stepover),
        "10 μm scallop on a 1 mm ball should give ~0.199 mm, got {stepover}"
    );
}

/// S1: with no scallop target the scallop override must not engage.
/// Feeds contract P5 (K2): the calculator then reads the op's own stated
/// stepover (`CutSpec::calc_inputs`), capped at the tool diameter, not
/// the family `ae_factor` value. Re-pinned by P5; the F-037 smoke baseline
/// reads the scallop path above.
#[test]
fn scallop_height_none_preserves_legacy_ae() {
    let op = dropcutter_op(None);
    let tool = ball_tool(1.0);
    let stepover = dropcutter_stepover(&op, &tool);
    let stated = op.stepover().expect("a DropCutter states its stepover");
    assert!(
        (stepover - stated.min(tool.diameter)).abs() < 1e-12,
        "with no scallop the calculator reads the stated stepover {stated} capped at D, got {stepover}"
    );
}

/// S1: a tapered ball cuts the same cusp curve as a true ball of the
/// same tip radius — the scallop stepover is determined by the
/// spherical tip only. A tapered ball with tip radius 0.5 mm
/// (diameter 1.0 mm) must produce the same stepover as a 1 mm true
/// ball for the same scallop target.
#[test]
fn scallop_height_uses_tip_radius_for_tapered_ball() {
    let mut tapered = ToolConfig::test_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
    tapered.diameter = 1.0; // tip diameter → tip radius 0.5 mm
    tapered.set_cutting_length(ToolFact::Stated(25.0));
    let tapered_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &tapered);
    let ball_step = dropcutter_stepover(&dropcutter_op(Some(0.010)), &ball_tool(1.0));
    assert!(
        (tapered_step - ball_step).abs() < 1e-9,
        "tapered ball (tip r=0.5) stepover {tapered_step} should equal \
             1 mm true ball stepover {ball_step}"
    );
}

fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 10.0,
        stock_bottom_z: 0.0,
        stock_z: Some(10.0),
        stock_padding: 1.0,
    }
}

fn tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = diameter;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool
}

#[test]
fn vendor_lut_row_present_path_is_used() {
    let result = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool(6.35),
        machine: &MachineProfile::default(),
        material: &Material::default(),
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock_ctx(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("pocket + flat is not a refused combination");
    assert!(matches!(
        result.feeds_result.chipload_source,
        ChiploadSource::VendorLut { .. }
    ));
}

/// F1 (2026-06-10 defect-class cleanup): the drill-family suggested
/// feed survives the apply funnel. Pre-fix, `apply_feeds_subset`
/// wrote feed then plunge, and the drill configs' aliased setters
/// ("feed IS plunge") let the milling plunge baseline win —
/// suggested ~2400 → stored ~595 on every funnel (GUI/MCP add,
/// Apply-all, CLI --apply-suggest). Post-fix `calculate()` aliases
/// plunge to the envelope-clamped drill feed, so write order is
/// irrelevant and the stored value equals the suggestion.
///
/// Ruling B5 (G6, 2026-09-24): a drill in plastic now refuses (every drill
/// cell with no G6 claim refuses, in every material), and a 6 mm 2-flute
/// flat end mill in GenericHardwood ships through the drill claim (the
/// Amana Spektra 6 mm side row / 2). So the round trip runs there. The
/// funnel is material-free.
#[test]
fn drill_apply_round_trips_suggested_feed() {
    let tool = tool(6.0);
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::GenericHardwood,
    };
    for op_type in [OperationType::Drill, OperationType::AlignmentPinDrill] {
        let s = suggest_params(SuggestParamsInput {
            op_type,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: &EMBEDDED_LUT,
            stock_ctx: &stock_ctx(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("a 6 mm 2-flute flat plunge in hardwood ships through the G6 claim");
        assert!(
            matches!(
                s.feeds_result.support,
                crate::feeds::FeedsSupport::DrillTransferred { size: None, .. }
            ),
            "{op_type:?}: the drill claim serves the cell, got {:?}",
            s.feeds_result.support
        );
        let stored = s.operation.feed_rate().expect("S11: the fixture sets it");
        let suggested = s.feeds_result.feed_rate_mm_min;
        assert_eq!(
            s.feeds_result.plunge_rate_mm_min, suggested,
            "{op_type:?}: drill FeedsResult must be self-consistent (plunge IS feed)"
        );
        assert!(
            (stored - suggested).abs() <= 1.0,
            "{op_type:?}: stored feed {stored} must match suggested {suggested} \
                 (pre-F1 the plunge baseline clobbered it)"
        );
        // And the stored feed must sit inside the drill plunge-feed
        // gate band — the calculator and the gate share one envelope.
        let (lo, hi) = material.drill_plunge_feed_envelope_per_mm();
        let ratio = stored / 6.0;
        assert!(
            ratio >= lo - 1e-9 && ratio <= hi + 1e-9,
            "{op_type:?}: stored feed/Ø {ratio:.1} outside envelope {lo}-{hi}"
        );
    }
}

// ── W3.1 SPEED / CUT split ──────────────────────────────────────────
//
// A speed-only apply must change feed/plunge/RPM exactly as the combined
// apply does while leaving stepover/DOC (and their provenance) untouched;
// a cut-only apply is the mirror image.

/// A Pocket op with distinctive sentinel speeds + geometry so "unchanged"
/// assertions are meaningful, plus the calculator's recommendation for it.
fn split_fixture() -> (
    OperationConfig,
    FeedsResult,
    ToolConfig,
    MachineProfile,
    Material,
    PassRole,
) {
    let tool = tool(6.35);
    let machine = MachineProfile::default();
    let material = Material::default();
    let suggested = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock_ctx(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("pocket + flat is not a refused combination");

    let mut base = OperationConfig::Pocket(PocketConfig::default());
    base.set_feed_rate(111.0);
    base.set_plunge_rate(22.0);
    base.set_spindle_rpm(Some(9000));
    base.set_stepover(1.234);
    base.set_depth_per_pass(2.345);
    let role = base.feeds_style().1;
    (base, suggested.feeds_result, tool, machine, material, role)
}

#[test]
fn apply_speeds_changes_speeds_leaves_cut_geometry() {
    let (base, result, tool, machine, material, role) = split_fixture();

    // Feeds contract P5: "Apply all" (the envelope proposal) is the
    // production write of geometry and speeds; it replaced the combined
    // chain apply this test compared with.
    let mut both = base.clone();
    let mut both_prov = crate::feeds::FeedsProvenance::default();
    apply_to_operation(
        OperationApplyInput {
            tool: &tool,
            machine: &machine,
            material: &material,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            lut: &EMBEDDED_LUT,
            suggest: SuggestContext::default(),
        },
        Some(crate::feeds::envelope::PickRequest::Proposal),
        None,
        &mut both,
        &mut both_prov,
    )
    .expect("a flat on a pocket is a valid pairing");

    let mut speeds = base.clone();
    let mut speeds_prov = crate::feeds::FeedsProvenance::default();
    apply_speeds_to_op(
        &mut speeds,
        &mut speeds_prov,
        &result,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );

    // the speeds are written
    assert!(speeds_prov.feed_rate.is_some());
    // cut geometry + its provenance untouched
    assert_eq!(speeds.as_params().stepover(), base.as_params().stepover());
    assert_eq!(
        speeds.as_params().depth_per_pass(),
        base.as_params().depth_per_pass()
    );
    assert!(speeds_prov.stepover.is_none());
    assert!(speeds_prov.depth_per_pass.is_none());

    // Adapted by G-SPEEDSAPPLY (2026-10-12). This test asserted that the
    // speeds of a speeds-only apply equal the speeds of the combined apply
    // on the base op (stepover 1.234, DPP 2.345). Those speeds were sized for
    // the geometry the combined apply writes, not for the geometry the base
    // op keeps. A speeds-only apply now resolves the speeds for the kept
    // geometry, so the equality holds when the op already carries the
    // geometry that the combined apply writes.
    let mut at_both_geometry = base;
    at_both_geometry.set_stepover(both.stepover().expect("pocket carries a stepover"));
    at_both_geometry.set_depth_per_pass(both.depth_per_pass().expect("pocket carries a DPP"));
    let result_at_both = feeds_result_for_operation(
        &at_both_geometry,
        &tool,
        &material,
        &machine,
        &EMBEDDED_LUT,
        crate::feeds::SpindleStrategy::default(),
    )
    .expect("a flat on a pocket is a valid pairing");
    let mut kept = at_both_geometry.clone();
    let mut kept_prov = crate::feeds::FeedsProvenance::default();
    apply_speeds_to_op(
        &mut kept,
        &mut kept_prov,
        &result_at_both,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );
    assert_eq!(
        kept.feed_rate().map(f64::to_bits),
        both.feed_rate().map(f64::to_bits)
    );
    assert_eq!(
        kept.plunge_rate().map(f64::to_bits),
        both.plunge_rate().map(f64::to_bits)
    );
    assert_eq!(kept.spindle_rpm(), both.spindle_rpm());
    assert_eq!(kept_prov.feed_rate, both_prov.feed_rate);
    assert_eq!(kept.stepover(), at_both_geometry.stepover());
    assert_eq!(kept.depth_per_pass(), at_both_geometry.depth_per_pass());
}

// ── Checkpoint I funnel (A-4, 2026-08-12) ──────────────────────────

/// Fixture pair for the funnel tests: the shipped default Ø6.35 2-flute
/// flat end mill on a Pocket (valid pairing) and on a Scallop (refused —
/// a zero tip radius makes `2·√(2·R·h − h²)` undefined).
fn preview_of(op: &OperationConfig, tool: &ToolConfig) -> FeedsPreview {
    feeds_preview_for_operation(
        op,
        tool,
        &Material::default(),
        &MachineProfile::default(),
        &EMBEDDED_LUT,
        crate::feeds::SpindleStrategy::default(),
    )
}

fn preview_for(op_type: OperationType) -> FeedsPreview {
    preview_of(
        &OperationConfig::new_default(op_type),
        &ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill),
    )
}

/// The funnel's central claim: an `ApplicableRecommendation` exists iff
/// the pairing validated, and it is the only way to reach [`apply`].
#[test]
fn preview_refuses_to_yield_an_applicable_recommendation_when_validation_refused() {
    let refused = preview_for(OperationType::Scallop);
    assert!(
        matches!(
            refused.refusal(),
            Some(FeedsError::WrongToolForOperation { .. })
        ),
        "fixture no longer refuses: {:?}",
        refused.refusal()
    );
    assert!(
        refused.applicable().is_none(),
        "a refused preview handed out a writable recommendation — the funnel's \
             only structural guarantee has been lost"
    );
    // The explanation survives the refusal — that is the point of I-3.
    assert!(refused.recommended().feed_rate_mm_min > 0.0);

    let ok = preview_for(OperationType::Pocket);
    assert!(ok.refusal().is_none());
    assert!(ok.applicable().is_some());
}

/// [`FeedsPreview::applicable`] hands back `explain().recommended`, and
/// the panel's [`feeds_result_for_operation`] hands back `calculate()` of
/// the same input. Both surfaces must therefore be applying the identical
/// recipe — this pins that, so "the modal and the panel now agree" rests
/// on a measured equality rather than on reading two call sites.
#[test]
fn preview_recommendation_is_bit_identical_to_validated_recipe() {
    let op = OperationConfig::new_default(OperationType::Pocket);
    let tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    let recipe = feeds_result_for_operation(
        &op,
        &tool,
        &Material::default(),
        &MachineProfile::default(),
        &EMBEDDED_LUT,
        crate::feeds::SpindleStrategy::default(),
    )
    .expect("pocket + flat end mill is a valid pairing");
    let preview = preview_for(OperationType::Pocket);
    let previewed = preview.recommended();
    for (name, a, b) in [
        ("feed", recipe.feed_rate_mm_min, previewed.feed_rate_mm_min),
        (
            "plunge",
            recipe.plunge_rate_mm_min,
            previewed.plunge_rate_mm_min,
        ),
        ("rpm", recipe.rpm, previewed.rpm),
        ("doc", recipe.axial_depth_mm, previewed.axial_depth_mm),
        ("woc", recipe.radial_width_mm, previewed.radial_width_mm),
    ] {
        assert_eq!(a.to_bits(), b.to_bits(), "{name}: {a} vs {b}");
    }
}

/// [`apply`] with no pick must be indistinguishable from the speeds-only
/// test door it backs (K2b, P5: the public apply writes the speeds for the
/// geometry the op keeps; the chain doors are test doors). No recipe number
/// may move (A-4 bar 2).
#[test]
fn apply_matches_the_speeds_entry_point_exactly() {
    let (base, _unused, tool, machine, material, role) = split_fixture();
    let preview = preview_of(&base, &tool);
    let result = preview.recommended().clone();
    let rec = preview.applicable().expect("valid pairing");
    let ctx = ApplyContext {
        tool: &tool,
        machine: &machine,
        material: &material,
        pass_role: role,
        suggest: SuggestContext::default(),
    };

    let mut legacy_op = base.clone();
    let mut legacy_prov = crate::feeds::FeedsProvenance::default();
    apply_speeds_to_op(
        &mut legacy_op,
        &mut legacy_prov,
        &result,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );

    let mut funnel_op = base.clone();
    let mut funnel_prov = crate::feeds::FeedsProvenance::default();
    let (complete, _) = rec.accept_unsourced();
    apply(&complete, &mut funnel_op, &mut funnel_prov, ctx);

    assert_eq!(funnel_op.feed_rate(), legacy_op.feed_rate(), "feed");
    assert_eq!(funnel_op.plunge_rate(), legacy_op.plunge_rate(), "plunge");
    assert_eq!(funnel_op.spindle_rpm(), legacy_op.spindle_rpm(), "rpm");
    assert_eq!(
        funnel_op.as_params().stepover(),
        base.as_params().stepover(),
        "woc: no pick, no geometry write"
    );
    assert_eq!(
        funnel_op.as_params().depth_per_pass(),
        base.as_params().depth_per_pass(),
        "doc: no pick, no geometry write"
    );
}

/// The explored-point apply keeps the operator's dragged feed/RPM (the
/// chipload recalibration must not re-solve them) while still taking the
/// clamps — here the plunge-to-feed clamp, exercised by dragging the feed
/// below the operation's plunge rate.
#[test]
fn explored_speeds_survive_the_funnel_but_still_get_clamped() {
    let (base, _result, tool, machine, material, role) = split_fixture();
    let preview = preview_of(&base, &tool);
    let rec = preview
        .applicable()
        .expect("valid pairing")
        .with_explored_speeds(120.0, 14_000.0);
    let mut op = base.clone();
    op.set_plunge_rate(900.0);
    let mut prov = crate::feeds::FeedsProvenance::default();
    let warnings = apply(
        &rec.clone().accept_unsourced().0,
        &mut op,
        &mut prov,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: role,
            suggest: SuggestContext::default(),
        },
    );
    assert_eq!(
        op.feed_rate(),
        Some(120.0),
        "the explored feed was re-solved"
    );
    assert_eq!(op.spindle_rpm(), Some(14_000), "the explored RPM moved");
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. })),
        "the clamp ran silently: {warnings:?}"
    );
    // Re-pinned by G10 (2026-09-25), cause: ruling Q4. The 6.35 mm 2-flute
    // flat is inside the G10 flat plunge claim (3.0-12.7 mm, feed / Z), so
    // the funnel runs the rule again at the explored feed: 120 / 2 = 60.
    // Before G10 the plunge stopped at the clamp, 120.
    assert_eq!(
        op.plunge_rate(),
        Some(60.0),
        "the plunge is not feed / Z at the explored feed"
    );
    assert!(
        warnings.iter().any(|w| matches!(
            w,
            SuggestWarning::PlungeReDerived {
                binding: crate::feeds::PlungeBinding::Rule,
                ..
            }
        )),
        "the funnel did not file the plunge move: {warnings:?}"
    );
    // Cut geometry untouched by a Speeds-scoped apply.
    assert_eq!(op.as_params().stepover(), base.as_params().stepover());
    assert_eq!(
        op.as_params().depth_per_pass(),
        base.as_params().depth_per_pass()
    );
}

#[test]
fn fallback_formula_path_is_used_without_matching_lut_row() {
    // 200 mm exceeds 10x the largest embedded flat-end pocket row
    // (12.7 mm compression spiral), so no row passes the diameter
    // sanity floor and the empirical fallback must take over.
    //
    // FM5 (2026-09-23): in a judged wood this no-row cell is UNJUDGED (the
    // embedded LUT always has a flat-end pocket row there, so the judgement
    // never saw it formula-only) and refuses. A plastic keeps the formula.
    let result = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool(200.0),
        machine: &MachineProfile::default(),
        material: &Material::Plastic {
            family: crate::material::PlasticFamily::Generic,
        },
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock_ctx(),
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("pocket + flat is not a refused combination");
    assert_eq!(
        result.feeds_result.chipload_source,
        ChiploadSource::FormulaFallback
    );
}

#[test]
fn invariants_clamp_and_warn() {
    // Feeds contract P5: the stepover, rigidity and cutting-length clamps
    // (passes 2, 4, 5) are deleted; the envelope holds those limits. The
    // speeds chain keeps pass 1 and never moves the geometry.
    let mut op = OperationConfig::Pocket(PocketConfig {
        feed_rate: Some(100.0),
        plunge_rate: Some(250.0),
        stepover: Some(12.0),
        depth_per_pass: Some(20.0),
        ..PocketConfig::default()
    });
    let tool = tool(6.0);
    let machine = MachineProfile::default();

    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &Material::default(),
        PassRole::Roughing,
        SuggestContext::default(),
    );

    assert_eq!(op.plunge_rate(), Some(100.0));
    assert_eq!(
        op.stepover(),
        Some(12.0),
        "the speeds chain moved the stepover"
    );
    assert_eq!(
        op.depth_per_pass(),
        Some(20.0),
        "the speeds chain moved the depth"
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. }))
    );
}

/// Fix #6 (2026-06-02 audit): `peck_depth` is overwritten by
/// `material.drill_default_peck_depth_mm(D)` when the Suggest
/// path runs.
///
/// **Updated 2026-06-03 (P5 lit-matrix fix):** softwood per-peck
/// max is now Janka-banded at 6.0×D (was a flat 2.0 for every
/// wood). Softwood Suggest default is therefore 0.5 × 6.0 × D =
/// 3.0×D — inside the matrix band 3–8×D — instead of the old
/// 1.0×D that matched dense hardwood. `apply_drill_defaults`
/// additionally clamps the result to `0.75 × drill_depth` so
/// even shallow holes still peck at least twice. For the test's
/// default `DrillConfig` (`depth = 10 mm`) the clamp ceiling is
/// 7.5 mm.
#[test]
fn drill_peck_depth_scales_with_diameter_and_material() {
    use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
    use crate::material::Material;

    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::GenericSoftwood,
    };

    // 3 mm bit — pre-2026-06-03 result was 3.0 mm (1.0×D, matched
    // hardwood). Janka-banded softwood now defaults to 0.5×6.0×D
    // = 9.0 mm, then clamps to 0.75 × default depth (10 mm) = 7.5 mm.
    let mut op_3mm = OperationConfig::Drill(DrillConfig {
        cycle: DrillCycleType::Peck,
        peck_depth: Some(3.0), // pre-fix hardcode value
        // S11: the fixture states the 10 mm hole the claim reads.
        floor: crate::compute::floor::Floor::top(10.0),
        ..DrillConfig::default()
    });
    let mut tool_3mm = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool_3mm.diameter = 3.0;

    apply_drill_defaults(&mut op_3mm, &tool_3mm, &material, None);

    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    let peck_3mm = match &op_3mm {
        OperationConfig::Drill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
        _ => panic!("expected Drill"),
    };
    assert!(
        (peck_3mm - 7.5).abs() < 1e-6,
        "3 mm bit softwood peck depth should be min(0.5×6.0×3.0, 0.75×10.0) = 7.5 mm \
             (Janka-banded softwood clamped to 0.75×depth), got {peck_3mm}"
    );

    // 12 mm bit — softwood Janka band gives 0.5×6.0×12 = 36.0 mm,
    // clamped to 0.75 × 10 mm = 7.5 mm by the default depth.
    let mut op_12mm = OperationConfig::Drill(DrillConfig {
        cycle: DrillCycleType::Peck,
        peck_depth: Some(3.0), // pre-fix hardcode value
        // S11: the fixture states the 10 mm hole the claim reads.
        floor: crate::compute::floor::Floor::top(10.0),
        ..DrillConfig::default()
    });
    let mut tool_12mm = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool_12mm.diameter = 12.0;

    apply_drill_defaults(&mut op_12mm, &tool_12mm, &material, None);

    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    let peck_12mm = match &op_12mm {
        OperationConfig::Drill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
        _ => panic!("expected Drill"),
    };
    assert!(
        peck_12mm > 6.0,
        "12 mm bit softwood peck depth should scale up beyond the old 3.0 mm hardcode, got {peck_12mm}"
    );

    // Non-drill op: untouched.
    let mut op_pocket = OperationConfig::Pocket(PocketConfig::default());
    let pocket_before = format!("{op_pocket:?}");
    apply_drill_defaults(&mut op_pocket, &tool_3mm, &material, None);
    assert_eq!(
        format!("{op_pocket:?}"),
        pocket_before,
        "apply_drill_defaults must be a no-op for non-drill ops"
    );
}

/// **DR-PIN sentry.** An out-of-range `AlignmentPinDrill` peck must
/// be clamped by the same rule, with the same ceiling factor, as
/// `Drill`'s — against the depth the pin family actually drills
/// (`stock_z + spoilboard_penetration`, the expression
/// `compute::execute::generate_alignment_pin_drill` uses).
///
/// The pre-fix reproduction is the first assertion and it stays:
/// with **no stock context** the pin arm is still written
/// unclamped, because the clamp has nothing to clamp against. That
/// was the ONLY behaviour before 2026-08-14 — the arm read
/// `AlignmentPinDrill(cfg) => cfg.peck_depth = peck` on every path
/// — and it is what `TECH_DEBT_2_CLOSEOUT.md` §4.4 DR-PIN measured:
/// a softwood Ø6 pin drill takes an 18 mm peck at a ~13 mm hole, a
/// single-shot cycle wearing a `Peck` label that the per-peck gate
/// then passes at 2.17 vs 6.0 because one descent is a legal
/// descent.
///
/// "Identically to Drill's" is asserted as an EQUALITY between the
/// two families at the same hole depth, not as two numbers that
/// happen to match a literal — so a future change to
/// `peck_for_hole`'s 0.75 factor moves both or fails here.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn alignment_pin_drill_peck_is_clamped_like_drill() {
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};
    use crate::material::{Material, WoodSpecies};

    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;

    // Softwood Ø6 Suggest default: 0.5 × 6.0 × 6.0 = 18.0 mm.
    let suggest_default = material.drill_default_peck_depth_mm(6.0);
    assert!(
        (suggest_default - 18.0).abs() < 1e-9,
        "fixture assumes the softwood Ø6 Suggest peck is 18 mm, got {suggest_default}"
    );

    let pin_cfg = || AlignmentPinDrillConfig {
        diameter: 6.0,
        spoilboard_penetration: 2.0,
        cycle: DrillCycleType::Peck,
        peck_depth: Some(3.0),
        ..AlignmentPinDrillConfig::default()
    };
    let peck_of = |op: &OperationConfig| -> f64 {
        match op {
            OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
            OperationConfig::Drill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
            other => panic!("expected a drill family op, got {other:?}"),
        }
    };

    // Pre-fix reproduction, preserved: no stock context, no clamp.
    let mut unclamped = OperationConfig::AlignmentPinDrill(pin_cfg());
    apply_drill_defaults(&mut unclamped, &tool, &material, None);
    assert!(
        (peck_of(&unclamped) - 18.0).abs() < 1e-9,
        "with no stock context the pin peck stays at the raw Suggest default \
             (the clamp has no depth to clamp against); got {}",
        peck_of(&unclamped)
    );

    // 11 mm stock + 2 mm spoilboard penetration = a 13 mm hole.
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -11.0,
        stock_z: Some(11.0),
        stock_padding: 0.0,
    };
    let hole_depth = stock.stock_z.expect("S11: the fixture sets it") + 2.0;

    let mut pin = OperationConfig::AlignmentPinDrill(pin_cfg());
    apply_drill_defaults(&mut pin, &tool, &material, Some(&stock));

    let mut drill = OperationConfig::Drill(DrillConfig {
        cycle: DrillCycleType::Peck,
        peck_depth: Some(3.0),
        floor: crate::compute::floor::Floor::Top { depth: hole_depth },
        ..DrillConfig::default()
    });
    apply_drill_defaults(&mut drill, &tool, &material, Some(&stock));

    assert!(
        (peck_of(&pin) - peck_of(&drill)).abs() < 1e-9,
        "the pin family must clamp identically to Drill at the same hole \
             depth ({hole_depth} mm): pin {} vs drill {}",
        peck_of(&pin),
        peck_of(&drill)
    );
    assert!(
        (peck_of(&pin) - 9.75).abs() < 1e-9,
        "0.75 × 13 mm = 9.75 mm is the shared ceiling; got {}",
        peck_of(&pin)
    );
    assert!(
        peck_of(&pin) < hole_depth,
        "a clamped peck must be strictly below the hole depth or the cycle \
             is single-shot: {} vs {hole_depth}",
        peck_of(&pin)
    );
}

/// P5 (2026-06-03 literature-matrix triage): softwood Suggest
/// per-peck must exceed dense-hardwood Suggest per-peck. Before
/// the Janka-banded `drill_per_peck_max_dtd` patch, every wood
/// species returned a flat 2.0 max → 1.0×D default, so softwood
/// pecks collapsed to the same value as ipe (Janka 3510). That
/// triggered the matrix anti-pattern
/// `softwood_drill_matches_hardwood_peck` (`peck_over_d < 3.0`)
/// on `flat_3mm_drill_softwood`.
///
/// Sources for the 3–8×D softwood band: Onsrud Drill Chart, FPL
/// Wood Handbook §3.7, Vectric drill defaults, Amana Spektra.
/// Hardwood ceiling of 5×D is the matrix invariant for white oak
/// in `flat_3mm_drill_oak`.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn drill_peck_depth_softwood_exceeds_hardwood() {
    use crate::compute::operation_configs::{DrillConfig, DrillCycleType};
    use crate::material::{Material, WoodSpecies};

    let pine = Material::SolidWood {
        species: WoodSpecies::RadiataPine, // Janka 710 → just above 700 cutoff;
                                           // use GenericSoftwood for true softwood
    };
    let generic_softwood = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood, // Janka 600
    };
    let white_oak = Material::SolidWood {
        species: WoodSpecies::WhiteOak, // Janka 1360 (medium hardwood)
    };
    let ipe = Material::SolidWood {
        species: WoodSpecies::Ipe, // Janka 3510 (dense hardwood / unknown bucket)
    };

    let diameter = 6.0_f64;
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = diameter;

    // Use a deep hole so the per-material Suggest defaults stay
    // below the 0.75 × drill_depth clamp; otherwise all materials
    // saturate at the same ceiling and the ordering can't be
    // tested. 50 mm hole → 37.5 mm clamp ceiling, well above any
    // 6 mm-diameter Suggest result.
    let drill_depth = 50.0_f64;

    let peck_for = |material: &Material| -> f64 {
        let mut op = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            peck_depth: Some(3.0),
            floor: crate::compute::floor::Floor::Top { depth: drill_depth },
            ..DrillConfig::default()
        });
        apply_drill_defaults(&mut op, &tool, material, None);
        match &op {
            OperationConfig::Drill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
            _ => panic!("expected Drill"),
        }
    };

    let pine_peck = peck_for(&pine);
    let generic_softwood_peck = peck_for(&generic_softwood);
    let oak_peck = peck_for(&white_oak);
    let ipe_peck = peck_for(&ipe);

    let pine_over_d = pine_peck / diameter;
    let softwood_over_d = generic_softwood_peck / diameter;
    let oak_over_d = oak_peck / diameter;
    let ipe_over_d = ipe_peck / diameter;

    // Softwood must clear the matrix floor (3×D) for
    // `softwood_drill_matches_hardwood_peck` (peck_over_d < 3.0).
    assert!(
        softwood_over_d >= 3.0,
        "generic softwood peck must be ≥ 3×D (matrix band floor for `flat_3mm_drill_softwood`), \
             got {softwood_over_d:.3}×D"
    );
    // RadiataPine is right at the 700 Janka softwood cutoff (710);
    // it falls into the medium-hardwood band by design — assert it
    // at least beats the dense bucket.
    assert!(
        pine_over_d >= 2.5,
        "radiata pine peck should be ≥ 2.5×D (sits at softwood/medium boundary), \
             got {pine_over_d:.3}×D"
    );
    // Softwood < 8×D ceiling.
    assert!(
        softwood_over_d <= 8.0,
        "softwood peck must stay ≤ 8×D (Onsrud/FPL upper band), got {softwood_over_d:.3}×D"
    );
    // Hardwood ceiling (matrix `peck_over_d_hardwood` invariant: 5×D).
    assert!(
        oak_over_d <= 5.0,
        "white oak peck must stay ≤ 5×D (matrix hardwood ceiling), got {oak_over_d:.3}×D"
    );
    assert!(
        ipe_over_d <= 5.0,
        "ipe peck must stay ≤ 5×D (matrix hardwood ceiling), got {ipe_over_d:.3}×D"
    );

    // Ordering: softwood > oak > ipe (matches density / Janka).
    assert!(
        softwood_over_d > oak_over_d,
        "softwood ({softwood_over_d:.3}×D) must exceed white oak ({oak_over_d:.3}×D)"
    );
    assert!(
        oak_over_d > ipe_over_d,
        "white oak ({oak_over_d:.3}×D) must exceed ipe ({ipe_over_d:.3}×D)"
    );
}

/// v1.3 combined-Suggest (2026-06-03 design doc § v1.3): when the
/// LUT + rigidity-factor path writes a DPP > 0.5×D on an Adaptive3d
/// op with `entry_style = Plunge`, the plunge-entry transient
/// breaches the deflection gate and the toolpath generates no
/// cutting moves (Wanaka 3D Rough 6 failure mode). Suggest must
/// emit `PlungeEntryUnstableAtDpp` *without* rewriting strategy
/// params — auto-rewrite is v2.
///
/// Two cases covered:
/// - DPP=9 mm, D=6 mm (pre-v1.1 motivating failure, ratio 1.5)
/// - DPP=3.69 mm, D=6 mm (post-v1.1 transient-spike calibration
///   point, ratio 0.615 — steady-state 162 µm but entry transient
///   peaks at 362 µm)
#[test]
fn plunge_entry_unstable_warning_fires_when_dpp_exceeds_half_diameter() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

    let cases: &[(f64, &str)] = &[
        (9.0, "pre-v1.1 case: DPP=9 mm on 6 mm tool (ratio 1.5)"),
        (
            3.69,
            "post-v1.1 transient-spike case: DPP=3.69 mm on 6 mm tool (ratio 0.615)",
        ),
    ];

    for (dpp, label) in cases {
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: Some(1500.0),
            plunge_rate: Some(500.0),
            stepover: Some(0.88),
            depth_per_pass: Some(*dpp),
            entry_style: Adaptive3dEntryStyle::Plunge,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.set_cutting_length(ToolFact::Stated(25.0));
        // Milling-Kc calibration (2026-06-17, milling factor 2.7):
        // at the new_default 45 mm stickout the deflection back-off
        // now rewrites DPP, but this test asserts the v1.3 warning-only
        // contract (DPP must NOT be auto-rewritten — the warning fires
        // without a rewrite). Stiffen the tool (stickout 45 → 12 mm; δ
        // ∝ stickout³ → ~0.019×) so deflection doesn't force a DPP
        // rewrite and the warning-only behaviour is what's under test.
        tool.set_stickout(ToolFact::Stated(12.0));
        let mut machine = MachineProfile::default();
        machine.rigidity.adaptive_doc_factor = 1.50;

        let warnings = fixed_chain(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            // v3.3c (StrategyAndFeeds default) auto-rewrites
            // entry_style and suppresses this warning. This test
            // asserts the v1.3 warning-only contract, so pin to
            // FeedsWithGates explicitly.
            SuggestContext {
                policy: SuggestPolicy {
                    scope: SuggestScope::FeedsWithGates,
                },
                ..SuggestContext::default()
            },
        );

        // The DPP must not have been rewritten — v1.3 is warning-only.
        assert_eq!(
            op.depth_per_pass(),
            Some(*dpp),
            "v1.3 must not auto-rewrite DPP (warning-only) — {label}"
        );
        let hit = warnings.iter().any(|w| {
            matches!(
                w,
                SuggestWarning::PlungeEntryUnstableAtDpp { dpp_mm, diameter_mm, entry_style }
                    if (*dpp_mm - *dpp).abs() < 1e-9
                        && (*diameter_mm - 6.0).abs() < 1e-9
                        && entry_style == "plunge"
            )
        });
        assert!(
            hit,
            "Adaptive3d + plunge entry + DPP > 0.5×D must emit \
                 PlungeEntryUnstableAtDpp — {label}, got {warnings:?}"
        );
    }
}

/// v1.3 counter-test: DPP well below the 0.5×D threshold must NOT
/// fire the warning. Guards the lower bound of the calibrated
/// 2026-06-03 threshold (was 1.0×D, widened to 0.5×D after the
/// Wanaka transient-spike data showed entry deflection breaching
/// at ratio ≈ 0.615).
#[test]
fn plunge_entry_unstable_does_not_fire_below_half_diameter() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

    // DPP=2.0 on a 6 mm tool → ratio 0.333, comfortably below the
    // 0.5×D trigger. Plunge entry, so the *only* reason the warning
    // wouldn't fire is the threshold — not the entry-style guard.
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: Some(1500.0),
        plunge_rate: Some(500.0),
        stepover: Some(0.88),
        depth_per_pass: Some(2.0),
        entry_style: Adaptive3dEntryStyle::Plunge,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    let mut machine = MachineProfile::default();
    machine.rigidity.adaptive_doc_factor = 1.50;

    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &Material::default(),
        PassRole::Roughing,
        SuggestContext::default(),
    );

    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
        "DPP=2.0 on 6 mm tool (ratio 0.333) is below the 0.5×D \
             trigger and must not fire PlungeEntryUnstableAtDpp, got {warnings:?}"
    );
}

/// v1.3 counter-test: a helix or ramp entry on the same DPP/tool
/// combo must NOT fire the warning — only plunge entries are
/// unstable at DPP > diameter. Guards against a too-broad firing
/// rule that would scare users off the safe entry styles.
#[test]
fn plunge_entry_unstable_does_not_fire_for_helix_or_ramp() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    let mut machine = MachineProfile::default();
    machine.rigidity.adaptive_doc_factor = 1.50;

    for style in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: Some(1500.0),
            plunge_rate: Some(500.0),
            stepover: Some(0.88),
            depth_per_pass: Some(9.0),
            entry_style: style,
            ..Adaptive3dConfig::default()
        });
        let warnings = fixed_chain(
            &mut op,
            &tool,
            &machine,
            &Material::default(),
            PassRole::Roughing,
            SuggestContext::default(),
        );
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
            "helix/ramp entry must not trigger PlungeEntryUnstableAtDpp, got {warnings:?} \
                 for style {style:?}"
        );
    }
}

/// SuggestContext plumbing invariant (2026-06-04 refactor): the
/// gate-aware context plumbed onto `SuggestParamsInput` /
/// `SuggestForOperationInput` is *carried* by v1.1 but not yet
/// *consumed*. A populated context must therefore produce
/// bit-identical Suggest output to `SuggestContext::default()` —
/// guards against an accidental v1.1 read of the v1.2 slots.
#[test]
fn suggest_context_is_no_op_in_v1_1() {
    let stock = stock_ctx();
    // Synthetic bbox: 200 × 150 × 12 mm, origin at zero. Realistic
    // model envelope but no path-dependence so the test stays
    // hermetic.
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -12.0),
        max: crate::geo::P3::new(200.0, 150.0, 0.0),
    };
    let populated = SuggestContext {
        model_bbox: Some(&bbox),
        stock: Some(&stock),
        upstream_leftover_stock_mm: Some(1.2),
        neighboring_strategy_hint: Some("adaptive3d"),
        chipload_bounds: None,
        matched_lut_row: None,
        effective_diameter_mm: 0.0,
        calculator_operating_point: None,
        policy: SuggestPolicy::default(),
        dressups: None,
    };

    let baseline = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool(6.35),
        machine: &MachineProfile::default(),
        material: &Material::default(),
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
    .expect("pocket + flat is not a refused combination");
    let with_ctx = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool(6.35),
        machine: &MachineProfile::default(),
        material: &Material::default(),
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: crate::feeds::SpindleStrategy::default(),
        context: populated,
    })
    .expect("pocket + flat is not a refused combination");

    assert_eq!(
        baseline.feeds_result.feed_rate_mm_min, with_ctx.feeds_result.feed_rate_mm_min,
        "populated SuggestContext must not perturb feed_rate in v1.1"
    );
    assert_eq!(
        baseline.feeds_result.plunge_rate_mm_min, with_ctx.feeds_result.plunge_rate_mm_min,
        "populated SuggestContext must not perturb plunge_rate in v1.1"
    );
    assert_eq!(
        baseline.feeds_result.radial_width_mm, with_ctx.feeds_result.radial_width_mm,
        "populated SuggestContext must not perturb stepover in v1.1"
    );
    assert_eq!(
        baseline.feeds_result.axial_depth_mm, with_ctx.feeds_result.axial_depth_mm,
        "populated SuggestContext must not perturb DPP in v1.1"
    );
    assert_eq!(
        baseline.feeds_result.rpm, with_ctx.feeds_result.rpm,
        "populated SuggestContext must not perturb RPM in v1.1"
    );
    assert_eq!(
        baseline.operation.feed_rate(),
        with_ctx.operation.feed_rate(),
        "operation feed_rate must match between context variants in v1.1"
    );
    assert_eq!(
        baseline.operation.depth_per_pass(),
        with_ctx.operation.depth_per_pass(),
        "operation DPP must match between context variants in v1.1"
    );
}

/// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
/// `feed_recalibration_raises_feed_for_wanaka_back_rough_case`).**
///
/// This was the pass-8 sentry: it required the arc-fit feed-up to fire
/// on the Wanaka Back Rough motivating case and pinned the closed-form
/// solve `target / arc_fit_ratio × rpm × flutes = 0.027 / 0.25 × 16000
/// × 2` at **3456 mm/min**, up from the calculator's 911 mm/min — a
/// 3.79× lift, with `FeedRaisedForChipload { cap_hit: None }`.
///
/// Pass 8 is retired. The assertion inverts: the same input must now
/// leave the feed where the calculator put it and emit no lift warning.
/// Both pre-fix numbers are kept above so the inversion is readable as
/// a movement, not a rewrite.
#[test]
fn retired_lift_leaves_wanaka_back_rough_feed_untouched() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::feeds::ChiploadBounds;
    use crate::material::WoodSpecies;

    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: Some(911.0),
        plunge_rate: Some(300.0),
        stepover: Some(1.2),
        // Post-v1.1-back-off DPP. Helix entry so we don't also
        // fire PlungeEntryUnstableAtDpp.
        depth_per_pass: Some(3.69),
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    // Stiffened (45 → 18 mm) by the pre-fix version so deflection left
    // the feed-up loop headroom. Kept so the fixture is bit-identical
    // to the one that produced 3456 mm/min.
    tool.set_stickout(ToolFact::Stated(18.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let mut machine = MachineProfile::default();
    // Disable the rigidity clamp at DPP=3.69 (3.69 < 1.6 × 6 = 9.6 is OK).
    machine.rigidity.adaptive_doc_factor = 1.60;
    machine.max_feed_mm_min = 10_000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.027,
        max_mm_per_tooth: 0.060,
    });

    let initial_feed = op.feed_rate().expect("S11: the fixture sets it");
    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            chipload_bounds: bounds,
            ..SuggestContext::default()
        },
    );

    assert!(
        (op.feed_rate().expect("S11: the fixture sets it") - initial_feed).abs() < 1e-6,
        "retired pass 8 must leave feed at the calculator value {initial_feed} \
             (pre-fix it was lifted to 3456), got {}",
        op.feed_rate().expect("S11: the fixture sets it")
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
        "no Suggest pass constructs FeedRaisedForChipload since 2026-08-13, got {warnings:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::ChiploadStillLowAfterRecalibration { .. })),
        "no Suggest pass constructs ChiploadStillLowAfterRecalibration since 2026-08-13, \
             got {warnings:?}"
    );
}
/// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
/// `feed_recalibration_caps_on_deflection` and
/// `speed_gated_by_deflection_fires`, merged).**
///
/// Both tests exercised a *cap* of the retired pass 8. Pre-fix, a
/// 100 mm-stickout Ø6 2F endmill put the closed-form predictor's
/// deflection in the (190, 200) µm refusal window, so the pass wrote
/// the solved feed, the verify tripped, and it reverted 911 → 911 while
/// still emitting `FeedRaisedForChipload { cap_hit:
/// Some(DeflectionThreshold) }` **plus**
/// `ChiploadStillLowAfterRecalibration { blocking_cap:
/// DeflectionThreshold }`. Under `SuggestAggressiveness::Speed` the
/// same fixture pinned `lut_target_mm_per_tooth` at the band max
/// (0.10), proving Speed's target was what got gated; its uncapped
/// solve would have been `0.10 / 0.25 × 16000 × 2 = 12 800 mm/min`.
///
/// With the pass retired there is no solve, no verify and no cap. Both
/// aggressiveness levels must now leave the feed untouched and emit
/// neither warning. The two fixtures are kept bit-identical and run as
/// one test because the only remaining difference between them is the
/// policy field.
///
/// **Note what this test does NOT claim.** A 100 mm-stickout Ø6
/// endmill at 190+ µm predicted deflection is still a bad operating
/// point; what changed is that Suggest no longer *raises feed into* it
/// and then congratulates itself for refusing. The v1.1 DPP back-off
/// against `DEFLECTION_BACKOFF_TARGET_UM` is untouched by this commit.
#[test]
fn retired_lift_fires_no_cap_on_a_deflection_bound_fixture() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::feeds::ChiploadBounds;
    use crate::material::WoodSpecies;

    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    // Band max 0.10 is what the pre-fix Speed arm pinned.
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.05,
        max_mm_per_tooth: 0.10,
    });
    let initial_feed = 911.0_f64;

    // Ruling R4 (2026-09-24) deleted `SuggestAggressiveness`; the two policy
    // arms this loop ran are now one run.
    {
        let aggressiveness = "the one Suggest policy";
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: Some(initial_feed),
            plunge_rate: Some(300.0),
            stepover: Some(1.2),
            depth_per_pass: Some(3.69),
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.set_cutting_length(ToolFact::Stated(25.0));
        // δ ∝ stickout³ — this is the value that lands the prediction in the
        // (190, 200) µm window. Retune if the force physics shifts; the setup
        // guard below asserts the window.
        //
        // Retuned 100.0 → 99.0 for T-17. The equivalent bending section made
        // this 2-flute tool 1.60× more compliant, so 100.0 mm read 200.7 µm
        // and broke the upper edge of the window. The window is the fixture's
        // premise, not the claim under test.
        //
        // Retuned 99.0 → 111.0 for B6. The Curti line for hard maple is
        // 0.69× the old anchor line, so 99.0 mm read 137.1 µm.
        // 99 × (195 / 137.1)^(1/3) = 111.3.
        //
        // Retuned 111.0 → 115.0 for P11a (L5). The peak engaged-edge force
        // (helix lag) reads 173.8 µm at 111.0 mm.
        // 111 × (195 / 173.8)^(1/3) = 115.3.
        tool.set_stickout(ToolFact::Stated(115.0));
        tool.set_flute_count(ToolFact::Stated(2));
        let mut machine = MachineProfile::default();
        machine.rigidity.adaptive_doc_factor = 1.60;
        machine.max_feed_mm_min = 20_000.0;

        // The fixture's premise, asserted rather than assumed: the
        // operating point really is deflection-bound. Without this the
        // "no cap fires" assertion below would be vacuous — a fixture
        // that was never near the cap proves nothing about its removal.
        let pre_predicted =
            crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
                .expect("the fixture's premise is a modelled operating point")
                .predicted_um;
        assert!(
            (190.0..DEFLECTION_BACKOFF_TARGET_UM).contains(&pre_predicted),
            "test setup: predicted deflection ({pre_predicted:.1} µm) must sit in \
                 [190, {DEFLECTION_BACKOFF_TARGET_UM:.0}) µm — retune tool.stickout"
        );

        let warnings = fixed_chain(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                chipload_bounds: bounds,
                ..SuggestContext::default()
            },
        );

        assert!(
            (op.feed_rate().expect("S11: the fixture sets it") - initial_feed).abs() < 1e-6,
            "{aggressiveness:?}: feed must stay at the calculator value {initial_feed}, got {}",
            op.feed_rate().expect("S11: the fixture sets it")
        );
        assert!(
            !warnings.iter().any(|w| matches!(
                w,
                SuggestWarning::FeedRaisedForChipload { .. }
                    | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
            )),
            "{aggressiveness:?}: no Suggest pass constructs the retired chipload-lift \
                 warnings since 2026-08-13, got {warnings:?}"
        );
    }
}

/// **RE-BASELINED 2026-08-13 — Checkpoint J-1 (was
/// `feed_recalibration_caps_on_max_feed`).**
///
/// Pre-fix: feed already at the 4000 mm/min machine cap, band
/// 0.10–0.20, so pass 8's closed-form target `0.10 / 0.25 × 16000 × 2
/// = 12 800 mm/min` clamped straight back down to 4000. Feed did not
/// move, so `FeedRaisedForChipload` stayed silent, but
/// `ChiploadStillLowAfterRecalibration { blocking_cap: MaxFeed }`
/// fired — the pass reporting that it could not reach a target it had
/// no business aiming at.
///
/// Post-retirement the still-low warning must go silent too. The
/// `MaxFeed` clamp itself is not what was retired: the calculator and
/// `machine.cutting_feed_ceiling_mm_min()` still bound feed, and this
/// test asserts the feed stays at 4000 for that reason.
#[test]
fn retired_lift_reports_no_max_feed_shortfall() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::feeds::ChiploadBounds;
    use crate::material::WoodSpecies;

    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: Some(4000.0),
        plunge_rate: Some(300.0),
        stepover: Some(1.2),
        depth_per_pass: Some(2.0),
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(25.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let mut machine = MachineProfile::default();
    machine.rigidity.adaptive_doc_factor = 1.60;
    machine.max_feed_mm_min = 4000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    // Band deliberately far above the operating point — pre-fix this is
    // what guaranteed the retired pass entered and then reported a
    // shortfall. It is kept so the inversion is measured on the fixture
    // that produced the shortfall, not on a comfortable one.
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.10,
        max_mm_per_tooth: 0.20,
    });

    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            chipload_bounds: bounds,
            ..SuggestContext::default()
        },
    );

    assert!(
        (op.feed_rate().expect("S11: the fixture sets it") - 4000.0).abs() < 1e-6,
        "feed must stay at the machine cap, got {}",
        op.feed_rate().expect("S11: the fixture sets it")
    );
    assert!(
        !warnings.iter().any(|w| matches!(
            w,
            SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
                | SuggestWarning::FeedRaisedForChipload { .. }
        )),
        "the retired pass's shortfall report must be silent, got {warnings:?}"
    );
}

/// v2 step 2 counter-test: when the predicted observed chipload
/// already sits at or above the LUT minimum, the recalibration
/// loop must skip entirely — no feed change, no warning. Guards
/// against accidentally firing the loop on every Suggest path.
///
/// **2026-08-13, Checkpoint J-1: this is the only one of the five
/// pass-8 tests that SURVIVES UNCHANGED — it is now the general
/// case.** Its assertions ("feed untouched, neither warning fires")
/// used to describe the narrow in-band corner; with the pass retired
/// they describe every Suggest path. Not one character of the fixture
/// or the assertions moved, which makes it the cleanest single piece of
/// evidence that the retirement generalised rather than inverted the
/// contract. The comment below about arc-fit ratio 0.25 is left
/// standing as a record of why these particular constants were chosen.
#[test]
fn feed_recalibration_skipped_when_already_in_band() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::feeds::ChiploadBounds;
    use crate::material::WoodSpecies;

    // Adaptive3d arc-fit ratio is 0.25 (Calibrated). To put the
    // observed median at or above 0.027 mm/tooth we need nominal
    // ≥ 0.108 mm/tooth → at 16 kRPM × 2 flutes that's feed ≥ 3456
    // mm/min. Use 4000 to be safely above.
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: Some(4000.0),
        plunge_rate: Some(500.0),
        stepover: Some(1.2),
        depth_per_pass: Some(2.0),
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(25.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let mut machine = MachineProfile::default();
    machine.rigidity.adaptive_doc_factor = 1.60;
    machine.max_feed_mm_min = 10_000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.025,
        max_mm_per_tooth: 0.050,
    });

    let initial_feed = op.feed_rate().expect("S11: the fixture sets it");
    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            chipload_bounds: bounds,
            ..SuggestContext::default()
        },
    );

    assert!(
        (op.feed_rate().expect("S11: the fixture sets it") - initial_feed).abs() < 1e-6,
        "feed must be unchanged when observed chipload already in band, got {} vs {initial_feed}",
        op.feed_rate().expect("S11: the fixture sets it")
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
        "FeedRaisedForChipload must not fire when initial observed ≥ LUT min, got {warnings:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::ChiploadStillLowAfterRecalibration { .. })),
        "ChiploadStillLowAfterRecalibration must not fire when initial observed ≥ LUT min, got {warnings:?}"
    );
}

/// Ruling Q11 (G10, 2026-09-25): Suggest never writes the entry style.
///
/// The Wanaka Back Rough scaffold: a 6 mm 2-flute end mill at DPP 3.69 mm
/// (DPP / D = 0.615, above the 0.5 threshold). Every entry style stays as
/// the operator set it, in every scope, with and without a model bbox. A
/// Plunge entry at this depth files `PlungeEntryUnstableAtDpp`.
#[test]
fn suggest_never_writes_entry_style() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -50.0),
        max: crate::geo::P3::new(140.0, 150.0, 0.0),
    };
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    // A short stickout keeps the deflection back-off off at DPP 3.69 mm,
    // so DPP / D stays above 0.5.
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };
    for style in [
        Adaptive3dEntryStyle::Plunge,
        Adaptive3dEntryStyle::Ramp,
        Adaptive3dEntryStyle::Helix,
    ] {
        for scope in [SuggestScope::StrategyAndFeeds, SuggestScope::FeedsWithGates] {
            for model_bbox in [Some(&bbox), None] {
                let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
                    depth_per_pass: Some(3.69),
                    stepover: Some(1.2),
                    feed_rate: Some(911.0),
                    spindle_rpm: Some(16_000),
                    entry_style: style,
                    ..Adaptive3dConfig::default()
                });
                let warnings = fixed_chain(
                    &mut op,
                    &tool,
                    &machine,
                    &material,
                    PassRole::Roughing,
                    SuggestContext {
                        model_bbox,
                        policy: SuggestPolicy { scope },
                        ..SuggestContext::default()
                    },
                );
                let OperationConfig::Adaptive3d(cfg) = &op else {
                    panic!("the op must stay Adaptive3d")
                };
                let case = format!("{style:?} {scope:?} bbox {}", model_bbox.is_some());
                assert_eq!(cfg.entry_style, style, "{case}: the entry style moved");
                let unstable = warnings
                    .iter()
                    .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. }));
                assert_eq!(
                    unstable,
                    style == Adaptive3dEntryStyle::Plunge,
                    "{case}: PlungeEntryUnstableAtDpp must fire on a deep plunge only, got \
                     {warnings:?}"
                );
            }
        }
    }
}

/// v3.3c: the clearing-strategy pass is WARN-ONLY. On an unpinned
/// (ContourParallel-default) Adaptive3d roughing op over a
/// well-formed model bbox (classifier → MixedTerrain), the
/// recommendation warning fires AND the field is left untouched.
#[test]
fn clearing_strategy_recommendation_is_warn_only() {
    use crate::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -12.0),
        max: crate::geo::P3::new(200.0, 150.0, 0.0),
    };
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        depth_per_pass: Some(2.0),
        stepover: Some(1.2),
        feed_rate: Some(911.0),
        spindle_rpm: Some(16_000),
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(30.0));
    tool.set_flute_count(ToolFact::Stated(2));
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };

    let warnings = fixed_chain(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            model_bbox: Some(&bbox),
            policy: SuggestPolicy {
                scope: SuggestScope::StrategyAndFeeds,
            },
            ..SuggestContext::default()
        },
    );
    let recommended = warnings.iter().find_map(|w| match w {
        SuggestWarning::StrategyRecommendedNotApplied {
            param: "clearing_strategy",
            current,
            recommended,
            reason,
        } => Some((current.clone(), recommended.clone(), *reason)),
        SuggestWarning::PlungeClampedToFeed { .. }
        | SuggestWarning::StepoverClampedToToolDiameter { .. }
        | SuggestWarning::RoughingDepthClampedToRigidity { .. }
        | SuggestWarning::DepthClampedToCuttingLength { .. }
        | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
        | SuggestWarning::DppCappedByDeflection { .. }
        | SuggestWarning::DeflectionBackoffUnmodeled { .. }
        | SuggestWarning::DeflectionBackoffFigureIsAFloor { .. }
        | SuggestWarning::StepoverRaisedForRuntime { .. }
        | SuggestWarning::FeedRaisedForChipload { .. }
        | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
        | SuggestWarning::StrategyRecommendedNotApplied { .. }
        | SuggestWarning::AxialEnvelopeSafeBandEmpty { .. }
        | SuggestWarning::AxialDocClampedByEnvelope { .. }
        | SuggestWarning::AxialDocBelowBurnFloor { .. }
        | SuggestWarning::ProjectCurveDepthInfeasible { .. }
        | SuggestWarning::FinishEnvelopeAdvisory { .. }
        | SuggestWarning::FeedRescaledToFinalGeometry { .. }
        | SuggestWarning::FeedClampedToChiploadFloor { .. }
        | SuggestWarning::PowerRecheckedAfterRescale { .. }
        | SuggestWarning::CutGeometryFieldNotHeld { .. }
        | SuggestWarning::RpmLoweredForFeedCeiling { .. }
        | SuggestWarning::RampFeed { .. }
        | SuggestWarning::UnsourcedRowsAccepted { .. }
        | SuggestWarning::ComputedWidthAssumptions
        | SuggestWarning::OutsideEnvelope { .. }
        | SuggestWarning::PlungeReDerived { .. } => None,
    });
    let (current, recommended, reason) = recommended
        .expect("clearing-strategy recommendation must fire on unpinned MixedTerrain op");
    assert_eq!(current, "contour_parallel");
    assert_eq!(recommended, "geodesic_adaptive");
    assert_eq!(reason, "mixed_terrain_classifier");
    // Warn-only: the field must be untouched.
    let OperationConfig::Adaptive3d(cfg) = &op else {
        panic!("op must remain Adaptive3d")
    };
    assert_eq!(
        cfg.clearing_strategy,
        ClearingStrategy::ContourParallel,
        "warn-only pass must NOT rewrite clearing_strategy"
    );
}

/// v3.3c skip conditions: pinned non-default value (heuristic B),
/// FeedsWithGates scope, and missing model bbox (classifier →
/// Unknown) must each suppress the recommendation.
#[test]
fn clearing_strategy_recommendation_skips() {
    use crate::compute::operation_configs::{Adaptive3dConfig, ClearingStrategy};
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -12.0),
        max: crate::geo::P3::new(200.0, 150.0, 0.0),
    };
    let run = |strategy: ClearingStrategy,
               scope: SuggestScope,
               model_bbox: Option<&crate::geo::BoundingBox3>|
     -> Vec<SuggestWarning> {
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: Some(2.0),
            stepover: Some(1.2),
            feed_rate: Some(911.0),
            spindle_rpm: Some(16_000),
            clearing_strategy: strategy,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.set_cutting_length(ToolFact::Stated(25.0));
        tool.set_stickout(ToolFact::Stated(30.0));
        tool.set_flute_count(ToolFact::Stated(2));
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };
        fixed_chain(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                model_bbox,
                policy: SuggestPolicy { scope },
                ..SuggestContext::default()
            },
        )
    };

    let no_recommendation = |warnings: &[SuggestWarning], ctx: &str| {
        assert!(
            !warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::StrategyRecommendedNotApplied { .. })),
            "{ctx}: recommendation must not fire, got {warnings:?}"
        );
    };

    // Pinned non-default values (heuristic B).
    // Removal step R2: Geodesic Adaptive is the one non-default value.
    no_recommendation(
        &run(
            ClearingStrategy::GeodesicAdaptive,
            SuggestScope::StrategyAndFeeds,
            Some(&bbox),
        ),
        "pinned GeodesicAdaptive",
    );
    // Strategy passes off.
    no_recommendation(
        &run(
            ClearingStrategy::ContourParallel,
            SuggestScope::FeedsWithGates,
            Some(&bbox),
        ),
        "FeedsWithGates scope",
    );
    // No bbox → classifier Unknown → no signal.
    no_recommendation(
        &run(
            ClearingStrategy::ContourParallel,
            SuggestScope::StrategyAndFeeds,
            None,
        ),
        "missing model bbox",
    );
}

/// PRE-Phase-1 coverage net (architectural refactor T1), updated by the
/// feeds contract P5 (K2): every operation's calculator inputs are an
/// *explicit decision*. Since P5 the calculator reads the operator's cut
/// (`CutSpec::calc_inputs`): the stated depth, the stated width (the tool
/// diameter for a slot) and the scallop target. The family default is the
/// fallback only where the spec states no value.
///
/// The expected table below is an exhaustive match over
/// `OperationConfig` with **no wildcard arm** — adding a new
/// operation fails to compile here, forcing the author to decide
/// (and record) what the new op feeds the calculator. Changing an
/// existing op's inputs fails the assert instead of silently rerouting
/// the feeds suggestion.
#[test]
fn operation_feeds_hints_is_an_explicit_per_op_decision() {
    use crate::compute::catalog::OperationType;
    // The tool diameter of the probe. A slot reads it as its width.
    let d = 6.0;
    let positive = |v: Option<f64>| v.filter(|v| *v > 0.0);

    for &op_type in OperationType::ALL {
        let config = OperationConfig::new_default(op_type);
        let actual = operation_feeds_hints(&config, d);

        #[expect(clippy::match_same_arms, reason = "one arm per op = the point")]
        let expected: (Option<f64>, Option<f64>, Option<f64>) = match &config {
            OperationConfig::Face(c) => (c.depth_per_pass, c.stepover, None),
            OperationConfig::Pocket(c) => (c.depth_per_pass, c.stepover, None),
            OperationConfig::Profile(c) => (c.depth_per_pass, Some(d), None),
            OperationConfig::Adaptive(c) => (c.depth_per_pass, c.stepover, None),
            OperationConfig::VCarve(c) => (positive(c.max_depth), None, None),
            OperationConfig::Inlay(c) => (positive(c.pocket_depth), None, None),
            OperationConfig::InlayPlug(c) => (c.depth_per_pass, None, None),
            OperationConfig::Zigzag(c) => (c.depth_per_pass, c.stepover, None),
            OperationConfig::Trace(c) => (c.depth_per_pass, Some(d), None),
            OperationConfig::Drill(_) => (None, None, None),
            OperationConfig::Bore(_) => (None, Some(d), None),
            OperationConfig::Chamfer(_) => (None, None, None),
            OperationConfig::DropCutter(c) => match c.scallop_height {
                Some(h) => (None, None, Some(h)),
                None => (None, c.stepover, None),
            },
            OperationConfig::Adaptive3d(c) => (c.depth_per_pass, c.stepover, None),
            OperationConfig::Waterline(c) => (c.z_step, None, None),
            OperationConfig::Pencil(c) => {
                if c.num_offset_passes > 1 {
                    (None, c.offset_stepover, None)
                } else {
                    (None, None, None)
                }
            }
            OperationConfig::Scallop(c) => (None, None, Some(c.scallop_height)),
            OperationConfig::UnifiedFinish(c) => (c.z_step, None, Some(c.scallop_height)),
            OperationConfig::SteepShallow(c) => (c.z_step, c.stepover, None),
            OperationConfig::RampFinish(c) => (c.max_stepdown, None, None),
            OperationConfig::SpiralFinish(c) => (None, c.stepover, None),
            OperationConfig::RadialFinish(_) => (None, None, None),
            OperationConfig::HorizontalFinish(c) => (None, c.stepover, None),
            OperationConfig::ProjectCurve(c) => (positive(c.depth), Some(d), None),
            OperationConfig::AlignmentPinDrill(_) => (None, None, None),
        };

        assert_eq!(
            actual, expected,
            "{op_type:?}: operation_feeds_hints changed — update \
                 CutSpec::calc_inputs AND this expected table deliberately"
        );
    }
}

/// B14 (W5): Suggest writes a peck that the schema accepts. The floor is
/// read from the `peck_depth` `ParamDef` range of each drill family, not
/// copied. Before the fix `clamp_peck_to_depth` wrote `0.75 × depth`
/// under the floor on a shallow hole (0.45 mm on a 0.6 mm hole). A hole
/// shallower than the floor takes the peck equal to its depth: one peck.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn a_suggested_peck_respects_the_schema_floor_b14() {
    use crate::compute::catalog::OperationType;
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};
    use crate::material::{Material, WoodSpecies};

    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    let floor_of = |op_type: OperationType| {
        let range = OperationConfig::param_range_for_type(op_type, "peck_depth")
            .expect("non-vacuity: the schema declares a peck_depth range");
        let floor = *range.inclusive_bounds().start();
        assert!(
            floor.is_finite() && floor > 0.0,
            "the floor is a real bound: {floor}"
        );
        floor
    };
    let drill_floor = floor_of(OperationType::Drill);
    let pin_floor = floor_of(OperationType::AlignmentPinDrill);
    let peck_of = |op: &OperationConfig| match op {
        OperationConfig::Drill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
        OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth.unwrap_or(f64::NAN),
        other => panic!("expected a drill family op, got {other:?}"),
    };
    let drill_at = |depth: f64| {
        let mut op = OperationConfig::Drill(DrillConfig {
            cycle: DrillCycleType::Peck,
            floor: crate::compute::floor::Floor::Top { depth },
            ..DrillConfig::default()
        });
        apply_drill_defaults(&mut op, &tool, &material, None);
        peck_of(&op)
    };

    // A shallow hole: 0.75 × depth lies under the floor, the depth does not.
    let shallow = drill_floor / 0.75 * 0.9;
    assert!(0.75 * shallow < drill_floor && shallow > drill_floor);
    let peck = drill_at(shallow);
    assert!(
        (peck - drill_floor).abs() < 1e-12,
        "a {shallow} mm hole takes the floor {drill_floor} mm, got {peck}"
    );

    // A hole shallower than the floor: one peck, equal to the depth.
    let thin = 0.8 * drill_floor;
    let peck = drill_at(thin);
    assert!(
        (peck - thin).abs() < 1e-12,
        "a {thin} mm hole takes one peck of its depth, got {peck}"
    );

    // The pin family reads its own range, with and without a stock.
    let pin = |stock_z: Option<f64>, diameter: f64| {
        let mut op = OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            diameter: 6.0,
            spoilboard_penetration: 0.0,
            cycle: DrillCycleType::Peck,
            ..AlignmentPinDrillConfig::default()
        });
        let mut pin_tool = tool.clone();
        pin_tool.diameter = diameter;
        let stock = stock_z.map(|z| StockContext {
            stock_top_z: 0.0,
            stock_bottom_z: -z,
            stock_z: Some(z),
            stock_padding: 0.0,
        });
        apply_drill_defaults(&mut op, &pin_tool, &material, stock.as_ref());
        peck_of(&op)
    };
    let pin_shallow = pin_floor / 0.75 * 0.9;
    let peck = pin(Some(pin_shallow), 6.0);
    assert!(
        (peck - pin_floor).abs() < 1e-12,
        "a {pin_shallow} mm pin hole takes the floor {pin_floor} mm, got {peck}"
    );
    // No stock: no depth to cap at, but a tiny bit's raw peck still
    // rises to the floor.
    let raw = material.drill_default_peck_depth_mm(0.1);
    assert!(
        raw < pin_floor,
        "fixture: the raw peck {raw} lies under the floor"
    );
    let peck = pin(None, 0.1);
    assert!(
        (peck - pin_floor).abs() < 1e-12,
        "with no stock the peck rises to the floor {pin_floor} mm, got {peck}"
    );
}

/// F161/F183: `pick_axial_envelope` picks its arm by the registry's
/// `axial_envelope` class. The three rough arms write a field of one op's
/// config (`if let`), so each rough class must hold exactly that op. A new
/// op that takes a rough class fails here until its arm writes its field.
/// The finish class is generic (it reads `kind_str`), so any op may take it.
#[test]
fn each_axial_envelope_class_names_the_op_its_arm_writes() {
    use crate::compute::catalog::AxialEnvelope;
    let mut finish = 0usize;
    for &op in OperationType::ALL {
        let config = OperationConfig::new_default(op);
        let writes_its_field = match config.capabilities().axial_envelope {
            AxialEnvelope::NotApplied => true,
            AxialEnvelope::Finish3d => {
                finish += 1;
                true
            }
            AxialEnvelope::Roughing3d => matches!(config, OperationConfig::Adaptive3d(_)),
            AxialEnvelope::VBitGroove => matches!(config, OperationConfig::VCarve(_)),
            AxialEnvelope::CurveRough => matches!(config, OperationConfig::ProjectCurve(_)),
        };
        assert!(
            writes_its_field,
            "{op:?}: its axial-envelope class writes another op's field in \
             `pick_axial_envelope`; give the class an arm for this op"
        );
    }
    // Non-vacuity: the finish class is populated (eight rows on 2026-10-20).
    assert!(finish > 0, "no op has the Finish3d class");
}
