//! Unit tests for the canonical Suggest path. Moved out of `feeds/suggest.rs`
//! by P4; the module body is unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::axial_envelope::recompute_chipload_bounds_for_dpp;
use super::invariants::{
    DEFLECTION_BACKOFF_DPP_FLOOR_MM, DEFLECTION_BACKOFF_TARGET_UM,
    STEPOVER_BACKOFF_DIAMETER_FRACTION, STEPOVER_BACKOFF_MAX_ITERATIONS,
    STEPOVER_BACKOFF_TARGET_MOVES, enforce_invariants,
};
use super::*;
use crate::compute::operation_configs::{DropCutterConfig, PocketConfig};
use crate::compute::tool_config::{ToolId, ToolType};
use crate::feeds::{ChiploadSource, EMBEDDED_LUT, PassRole};

/// C2 sentinel contract: `SuggestContext::effective_diameter_mm == 0.0`
/// is "not populated", and the post-mutation chipload re-derivation must
/// then leave the vendor band UNDERATED — the same outcome the drill path
/// gets by forcing `doc_ratio = 0.0`. A populated diameter with a DPP
/// above it must, by contrast, actually derate.
#[test]
fn unpopulated_effective_diameter_skips_doc_derating() {
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

    // Populated, DPP twice the diameter → derated below the raw band.
    let derated = recompute_chipload_bounds_for_dpp(Some(&row), 6.0, &op, 12.0)
        .expect("a row with a full band must yield bounds");
    assert!(
        derated.max_mm_per_tooth < 0.20,
        "a 2.0 DOC ratio must derate the band; got {derated:?}"
    );
}

/// Ball-nose tool of the given diameter (tip radius = diameter / 2).
fn ball_tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
    tool.diameter = diameter;
    tool.cutting_length = 25.0;
    tool
}

/// DropCutter (3D Finish) operation with an optional scallop target.
fn dropcutter_op(scallop_height: Option<f64>) -> OperationConfig {
    OperationConfig::DropCutter(DropCutterConfig {
        scallop_height,
        ..DropCutterConfig::default()
    })
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

/// S1: with no scallop target the legacy formula-based stepover is
/// preserved — the scallop override must not engage, so the result
/// stays the tiny `ae_factor`-derived value (well under the scallop
/// path's 0.199 mm). Guards the F-037 smoke baseline.
#[test]
fn scallop_height_none_preserves_legacy_ae() {
    let stepover = dropcutter_stepover(&dropcutter_op(None), &ball_tool(1.0));
    assert!(
        stepover > 0.0 && stepover < 0.05,
        "legacy DropCutter stepover on a 1 mm ball should stay small \
             (formula-based), got {stepover}"
    );
}

/// S1: a tapered ball cuts the same cusp curve as a true ball of the
/// same tip radius — the scallop stepover is determined by the
/// spherical tip only. A tapered ball with tip radius 0.5 mm
/// (diameter 1.0 mm) must produce the same stepover as a 1 mm true
/// ball for the same scallop target.
#[test]
fn scallop_height_uses_tip_radius_for_tapered_ball() {
    let mut tapered = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
    tapered.diameter = 1.0; // tip diameter → tip radius 0.5 mm
    tapered.cutting_length = 25.0;
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
        stock_z: 10.0,
        stock_padding: 1.0,
    }
}

fn tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = diameter;
    tool.cutting_length = 25.0;
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
        let stored = s.operation.feed_rate();
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

    let mut both = base.clone();
    let mut both_prov = crate::feeds::FeedsProvenance::default();
    apply_feeds_result_to_op(
        &mut both,
        &mut both_prov,
        &result,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );

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

    // speeds identical to the combined apply
    assert_eq!(speeds.feed_rate(), both.feed_rate());
    assert_eq!(speeds.plunge_rate(), both.plunge_rate());
    assert_eq!(speeds.spindle_rpm(), both.spindle_rpm());
    assert_eq!(speeds_prov.feed_rate, both_prov.feed_rate);
    assert!(speeds_prov.feed_rate.is_some());
    // cut geometry + its provenance untouched
    assert_eq!(speeds.as_params().stepover(), base.as_params().stepover());
    assert_eq!(
        speeds.as_params().depth_per_pass(),
        base.as_params().depth_per_pass()
    );
    assert!(speeds_prov.stepover.is_none());
    assert!(speeds_prov.depth_per_pass.is_none());
}

#[test]
fn apply_cut_geometry_changes_geometry_leaves_speeds() {
    let (base, result, tool, machine, material, role) = split_fixture();

    let mut both = base.clone();
    let mut both_prov = crate::feeds::FeedsProvenance::default();
    apply_feeds_result_to_op(
        &mut both,
        &mut both_prov,
        &result,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );

    let mut geom = base.clone();
    let mut geom_prov = crate::feeds::FeedsProvenance::default();
    apply_cut_geometry_to_op(
        &mut geom,
        &mut geom_prov,
        &result,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );

    // geometry identical to the combined apply
    assert_eq!(geom.as_params().stepover(), both.as_params().stepover());
    assert_eq!(
        geom.as_params().depth_per_pass(),
        both.as_params().depth_per_pass()
    );
    assert_eq!(geom_prov.stepover, both_prov.stepover);
    assert!(geom_prov.stepover.is_some());
    // speeds + their provenance untouched
    assert_eq!(geom.feed_rate(), base.feed_rate());
    assert_eq!(geom.plunge_rate(), base.plunge_rate());
    assert_eq!(geom.spindle_rpm(), base.spindle_rpm());
    assert!(geom_prov.feed_rate.is_none());
    assert!(geom_prov.spindle_rpm.is_none());
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
        &ToolConfig::new_default(ToolId(1), ToolType::EndMill),
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
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
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

/// [`apply`] with each scope must be indistinguishable from the three
/// legacy entry points it now backs — the funnel was extended, not
/// duplicated, and no recipe number may move (A-4 bar 2).
#[test]
fn apply_scope_matches_the_legacy_entry_points_exactly() {
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

    for scope in [
        ApplyScope::Both,
        ApplyScope::Speeds,
        ApplyScope::CutGeometry,
    ] {
        let mut legacy_op = base.clone();
        let mut legacy_prov = crate::feeds::FeedsProvenance::default();
        let legacy = match scope {
            ApplyScope::Both => apply_feeds_result_to_op,
            ApplyScope::Speeds => apply_speeds_to_op,
            ApplyScope::CutGeometry => apply_cut_geometry_to_op,
        };
        legacy(
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
        apply(&rec, scope, &mut funnel_op, &mut funnel_prov, ctx);

        assert_eq!(
            funnel_op.feed_rate(),
            legacy_op.feed_rate(),
            "{scope:?} feed"
        );
        assert_eq!(
            funnel_op.plunge_rate(),
            legacy_op.plunge_rate(),
            "{scope:?} plunge"
        );
        assert_eq!(
            funnel_op.spindle_rpm(),
            legacy_op.spindle_rpm(),
            "{scope:?} rpm"
        );
        assert_eq!(
            funnel_op.as_params().stepover(),
            legacy_op.as_params().stepover(),
            "{scope:?} woc"
        );
        assert_eq!(
            funnel_op.as_params().depth_per_pass(),
            legacy_op.as_params().depth_per_pass(),
            "{scope:?} doc"
        );
    }
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
        &rec,
        ApplyScope::Speeds,
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
    assert_eq!(op.feed_rate(), 120.0, "the explored feed was re-solved");
    assert_eq!(op.spindle_rpm(), Some(14_000), "the explored RPM moved");
    assert_eq!(
        op.plunge_rate(),
        120.0,
        "plunge was not clamped to the explored feed — the funnel's clamps did not run"
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. })),
        "the clamp ran silently: {warnings:?}"
    );
    // Cut geometry untouched by a Speeds-scoped apply.
    assert_eq!(op.as_params().stepover(), base.as_params().stepover());
    assert_eq!(
        op.as_params().depth_per_pass(),
        base.as_params().depth_per_pass()
    );
}

/// [`resolve_operation_invariants`] is the funnel entry for a single-axis
/// operator/optimizer edit (Checkpoint I-4, OPT-005). It must clamp, and
/// it must NOT rewrite the accepted value.
#[test]
fn resolve_operation_invariants_clamps_an_axis_edit_without_re_solving_it() {
    let (mut op, _result, tool, machine, material, role) = split_fixture();
    // Optimizer suggests a stepover wider than the cutter.
    op.set_stepover(tool.diameter * 3.0);
    op.set_feed_rate(1234.0);
    let warnings = resolve_operation_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        role,
        SuggestContext::default(),
    );
    assert_eq!(
        op.as_params().stepover(),
        Some(tool.diameter),
        "stepover was not clamped to the cutter diameter"
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::StepoverClampedToToolDiameter { .. })),
        "clamp ran without a warning: {warnings:?}"
    );
    assert_eq!(
        op.feed_rate(),
        1234.0,
        "the accepted feed was re-solved; the default context must leave it alone"
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
    let mut op = OperationConfig::Pocket(PocketConfig {
        feed_rate: 100.0,
        plunge_rate: 250.0,
        stepover: 12.0,
        depth_per_pass: 20.0,
        ..PocketConfig::default()
    });
    let mut tool = tool(6.0);
    tool.cutting_length = 1.0;
    // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
    // at the new_default 45 mm stickout the deflection back-off now
    // clamps DOC to 0.512, below the cutting-length clamp (1.0) this
    // invariant test asserts. Stiffen the tool (stickout 45 → 10 mm;
    // δ ∝ stickout³ → ~0.011×) so the cutting-length clamp is the
    // binding one and all four invariant warnings remain the thing
    // under test.
    tool.stickout = 10.0;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.25;

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &Material::default(),
        PassRole::Roughing,
        SuggestContext::default(),
    );

    assert_eq!(op.plunge_rate(), 100.0);
    assert_eq!(op.stepover(), Some(6.0));
    assert_eq!(op.depth_per_pass(), Some(1.0));
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. }))
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::StepoverClampedToToolDiameter { .. }))
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. }))
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::DepthClampedToCuttingLength { .. }))
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
        peck_depth: 3.0, // pre-fix hardcode value
        ..DrillConfig::default()
    });
    let mut tool_3mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool_3mm.diameter = 3.0;

    apply_drill_defaults(&mut op_3mm, &tool_3mm, &material, None);

    let peck_3mm = match &op_3mm {
        OperationConfig::Drill(cfg) => cfg.peck_depth,
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
        peck_depth: 3.0, // pre-fix hardcode value
        ..DrillConfig::default()
    });
    let mut tool_12mm = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool_12mm.diameter = 12.0;

    apply_drill_defaults(&mut op_12mm, &tool_12mm, &material, None);

    let peck_12mm = match &op_12mm {
        OperationConfig::Drill(cfg) => cfg.peck_depth,
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
/// `clamp_peck_to_depth`'s 0.75 factor moves both or fails here.
#[test]
fn alignment_pin_drill_peck_is_clamped_like_drill() {
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig, DrillCycleType};
    use crate::material::{Material, WoodSpecies};

    let material = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;

    // Softwood Ø6 Suggest default: 0.5 × 6.0 × 6.0 = 18.0 mm.
    let suggest_default = material.drill_default_peck_depth_mm(6.0);
    assert!(
        (suggest_default - 18.0).abs() < 1e-9,
        "fixture assumes the softwood Ø6 Suggest peck is 18 mm, got {suggest_default}"
    );

    let pin_cfg = || AlignmentPinDrillConfig {
        spoilboard_penetration: 2.0,
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        ..AlignmentPinDrillConfig::default()
    };
    let peck_of = |op: &OperationConfig| -> f64 {
        match op {
            OperationConfig::AlignmentPinDrill(cfg) => cfg.peck_depth,
            OperationConfig::Drill(cfg) => cfg.peck_depth,
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
        stock_z: 11.0,
        stock_padding: 0.0,
    };
    let hole_depth = stock.stock_z + 2.0;

    let mut pin = OperationConfig::AlignmentPinDrill(pin_cfg());
    apply_drill_defaults(&mut pin, &tool, &material, Some(&stock));

    let mut drill = OperationConfig::Drill(DrillConfig {
        cycle: DrillCycleType::Peck,
        peck_depth: 3.0,
        depth: hole_depth,
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
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
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
            peck_depth: 3.0,
            depth: drill_depth,
            ..DrillConfig::default()
        });
        apply_drill_defaults(&mut op, &tool, material, None);
        match &op {
            OperationConfig::Drill(cfg) => cfg.peck_depth,
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

/// Bug 1 (2026-06-02 audit, workflow `w39ma2j1y`): adaptive ops
/// use `adaptive_doc_factor` (deep+narrow), not the
/// conventional `doc_roughing_factor`. For a 6 mm bit on a wood
/// router with `doc_roughing_factor=0.20` and
/// `adaptive_doc_factor=1.50`, an Adaptive3d op with DOC=6 mm
/// must not be clamped. Pre-fix the clamp stripped it to 1.2 mm,
/// erasing the upstream `adaptive_doc_factor` floor and turning
/// adaptive into "shallow conventional".
#[test]
fn adaptive_op_uses_adaptive_doc_factor_not_doc_roughing_factor() {
    use crate::compute::operation_configs::Adaptive3dConfig;
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        stepover: 0.88,
        depth_per_pass: 6.0,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
    // at the new_default 45 mm stickout the deflection back-off now
    // binds and clamps DOC=6 → 3.84, masking the DOC-factor logic this
    // test isolates. Stiffen the tool (stickout 45 → 12 mm; δ ∝
    // stickout³ → ~0.019×) so deflection doesn't interfere and the
    // adaptive_doc_factor selection is the only thing under test.
    tool.stickout = 12.0;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.20; // conventional
    machine.rigidity.adaptive_doc_factor = 1.50; // adaptive can go deep

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &Material::default(),
        PassRole::Roughing,
        SuggestContext::default(),
    );

    // DOC=6 mm is under adaptive_doc_factor*D=9 mm and under
    // cutting_length=25 mm — should NOT clamp.
    assert_eq!(
        op.depth_per_pass(),
        Some(6.0),
        "Adaptive3d DOC=6 mm on 6 mm bit must not clamp (adaptive_doc_factor=1.5)"
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
        "Adaptive3d DOC=6 mm must not trigger RoughingDepthClampedToRigidity \
             (was Bug 1 — adaptive ops clamped by conventional factor)"
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
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            stepover: 0.88,
            depth_per_pass: *dpp,
            entry_style: Adaptive3dEntryStyle::Plunge,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
        // at the new_default 45 mm stickout the deflection back-off
        // now rewrites DPP, but this test asserts the v1.3 warning-only
        // contract (DPP must NOT be auto-rewritten — the warning fires
        // without a rewrite). Stiffen the tool (stickout 45 → 12 mm; δ
        // ∝ stickout³ → ~0.019×) so deflection doesn't force a DPP
        // rewrite and the warning-only behaviour is what's under test.
        tool.stickout = 12.0;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
        machine.rigidity.adaptive_doc_factor = 1.50;

        let warnings = enforce_invariants(
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
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        stepover: 0.88,
        depth_per_pass: 2.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.20;
    machine.rigidity.adaptive_doc_factor = 1.50;

    let warnings = enforce_invariants(
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
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    let mut machine = MachineProfile::default();
    machine.rigidity.adaptive_doc_factor = 1.50;

    for style in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            feed_rate: 1500.0,
            plunge_rate: 500.0,
            stepover: 0.88,
            depth_per_pass: 9.0,
            entry_style: style,
            ..Adaptive3dConfig::default()
        });
        let warnings = enforce_invariants(
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

/// Bug 1 counter-test: conventional Pocket op DOES still clamp to
/// `doc_roughing_factor` — the fix is selective on feeds family,
/// not a blanket relaxation.
#[test]
fn conventional_op_still_clamps_by_doc_roughing_factor() {
    let mut op = OperationConfig::Pocket(PocketConfig {
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        stepover: 3.0,
        depth_per_pass: 6.0,
        ..PocketConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
    // at the new_default 45 mm stickout the deflection back-off now
    // clamps DOC to 0.96, masking the doc_roughing_factor clamp (1.2)
    // this counter-test isolates. Stiffen the tool (stickout 45 →
    // 12 mm; δ ∝ stickout³ → ~0.019×) so the rigidity factor is the
    // binding clamp again.
    tool.stickout = 12.0;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.20;
    machine.rigidity.adaptive_doc_factor = 1.50;

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &Material::default(),
        PassRole::Roughing,
        SuggestContext::default(),
    );

    // Pocket on 6 mm bit: doc_roughing_factor*D = 1.2 mm cap.
    let dpp = op.depth_per_pass().expect("dpp set after clamp");
    assert!(
        (dpp - 1.2).abs() < 1e-6,
        "Pocket DOC must still clamp to doc_roughing_factor*D (1.2), got {dpp}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. })),
        "Pocket DOC over conventional ceiling must still warn"
    );
}

/// v1.1 combined-Suggest: deflection-aware DPP selection for the
/// Wanaka Back Rough motivating case — 6 mm carbide endmill at 45 mm
/// stickout in HardMaple, 9 mm commanded DPP. The Suggest pass must
/// produce a *deflection-safe* DPP (predicted peak ≤ the 200 µm bound).
///
/// Deflection-model reconciliation (2026-06-17): the closed-form
/// `predict_peak_deflection_um` now delegates its cantilever to the
/// same integrated two-section model the axial envelope
/// (`pick_axial_envelope` → `invert_deflection`) uses. The two no
/// longer disagree, which changes *which mechanism* does the clamping
/// and dissolves the old back-off convergence problem:
///
/// - The axial envelope finds the DPP where integrated δ = 200 µm
///   (~6.57 mm at WOC 1.2 mm) and clamps the 9 mm command to it in one
///   shot, emitting `AxialDocClampedByEnvelope { binding: "deflection" }`.
/// - The back-off loop then evaluates the *same* integrated physics at
///   that DPP, sees it is already at/under the 200 µm bound, and does
///   nothing (0 iterations, no `DppCappedByDeflection`).
///
/// Pre-reconciliation the closed-form read ~3× hotter than the
/// envelope's bound, so the back-off chased a phantom target down to
/// ~2.15 mm and still bottomed out at ~350 µm against its 5-iteration
/// cap. The fix is the envelope and predictor agreeing — the DPP is
/// chosen correctly once, not thrashed. This is the sentry for "the
/// Suggest pass lands the wanaka rough deflection-safe in one pass."
#[test]
fn deflection_machinery_caps_dpp_for_long_reach_tool() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::material::WoodSpecies;
    // Deflection binds only on long/thin tools under the feed-aware
    // literature-absolute force model. Long-reach 6 mm carbide endmill,
    // 75 mm stickout, DPP 9 mm command, WOC 1.2 mm, feed 911 mm/min @
    // 16 kRPM, HardMaple — the 9 mm command predicts past the 200 µm
    // bound, so the axial-DOC envelope clamps it to the deflection-safe
    // DPP in one shot (no phantom back-off thrash).
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: 911.0,
        plunge_rate: 300.0,
        stepover: 1.2,
        depth_per_pass: 9.0,
        spindle_rpm: Some(16_000),
        // Helix entry so we don't also fire PlungeEntryUnstableAtDpp.
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.stickout = 85.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::default();
    // Pin the rigidity factors so the rigidity clamp does NOT
    // pre-clamp DPP — we want to see the deflection machinery
    // applied to the 9 mm starting point directly.
    machine.rigidity.doc_roughing_factor = 0.20;
    machine.rigidity.adaptive_doc_factor = 1.60; // 1.6 × 6 = 9.6 > 9, no rigidity clamp
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext::default(),
    );

    let dpp_after = op.depth_per_pass().expect("dpp set");
    // DPP is clamped below the 9 mm command, to the deflection-safe
    // bound — in one shot, NOT thrashed down by a phantom-hot back-off.
    assert!(
        (4.0..9.0).contains(&dpp_after),
        "DPP must be clamped to the deflection-safe bound (below the 9 mm command, not over-cut), got {dpp_after} mm"
    );
    // The whole point: the resulting DPP is deflection-safe. Predicted
    // peak at the chosen DPP sits at/under the 200 µm bound (allow a
    // hair of binary-search tolerance).
    let predicted_after =
        crate::feeds::predict::predict_peak_deflection_um(&op, &tool, &material, &machine)
            .expect("a modelled end mill in hard maple must produce a figure")
            .predicted_um;
    assert!(
        predicted_after <= 205.0,
        "Suggest must land the wanaka rough deflection-safe (≤ 200 µm bound), \
             got {predicted_after:.1} µm at DPP={dpp_after:.2} mm"
    );
    // And it genuinely backed off from the command: the 9 mm command
    // predicts well over the bound.
    let predicted_at_command = {
        let mut probe = op.clone();
        probe.set_depth_per_pass(9.0);
        crate::feeds::predict::predict_peak_deflection_um(&probe, &tool, &material, &machine)
            .expect("the 9 mm probe is modelled")
            .predicted_um
    };
    assert!(
        predicted_at_command > DEFLECTION_BACKOFF_TARGET_UM,
        "the 9 mm command must exceed the 200 µm bound (otherwise nothing to clamp), got {predicted_at_command:.1} µm"
    );
    // The axial envelope is the mechanism that clamps it, bound by
    // deflection. (The back-off loop is now a confirming no-op since
    // it shares the envelope's physics — so we assert the envelope
    // warning, not `DppCappedByDeflection`.)
    let clamp = warnings.iter().find_map(|w| match w {
        SuggestWarning::AxialDocClampedByEnvelope {
            clamped_mm,
            binding,
            ..
        } => Some((*clamped_mm, *binding)),
        _ => None,
    });
    let (clamped_mm, binding) =
        clamp.expect("AxialDocClampedByEnvelope must fire on the wanaka case");
    assert_eq!(
        binding, "deflection",
        "the binding constraint must be deflection, got {binding}"
    );
    assert!(
        (clamped_mm - dpp_after).abs() < 1e-9,
        "envelope clamp value must match the post-Suggest DPP, got {clamped_mm} vs {dpp_after}"
    );
}

/// v1.1 step 2 counter-test: a Finishing pass on the same
/// tool/material does NOT trigger the back-off — deflection mostly
/// threatens roughing engagement, and finishing passes already
/// run shallow DPPs by design. Locking back-off to roughing keeps
/// the suggest path symmetric with the existing
/// `RoughingDepthClampedToRigidity` gate.
#[test]
fn deflection_back_off_skipped_for_finish_pass() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::material::WoodSpecies;
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: 911.0,
        plunge_rate: 300.0,
        stepover: 1.2,
        depth_per_pass: 9.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    // Milling-Kc calibration (2026-06-17, MILLING_KC_FACTOR = 2.7):
    // the deflection force is now ~2.7× higher. At the original 45 mm
    // stickout a 9 mm DPP exceeds the *role-agnostic* axial-envelope
    // deflection ceiling (pick_axial_envelope, runs for all roles)
    // and trims DPP to ~6.57 — NOT via the roughing-only back-off
    // (backoff_dpp_for_deflection / DppCappedByDeflection), which
    // this test verifies is skipped for finish. To keep that the only
    // thing under test, stiffen the tool (stickout 45 → 18 mm; δ ∝
    // stickout³ → ~0.064×) so 9 mm sits inside the deflection
    // envelope and the DPP stays 9.0 untouched. Confirmed: the
    // back-off path remains correctly roughing-only — this is the
    // separate cutter-geometry envelope, not a finish-path
    // regression.
    tool.stickout = 18.0;
    tool.flute_count = 2;
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Finish,
        SuggestContext::default(),
    );

    assert_eq!(
        op.depth_per_pass(),
        Some(9.0),
        "Finishing pass must not trigger deflection back-off (back-off is roughing-only)"
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. })),
        "Finishing pass must not emit DppCappedByDeflection, got {warnings:?}"
    );
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

/// v1.1 step 2 floor: the loop must terminate at
/// `DEFLECTION_BACKOFF_DPP_FLOOR_MM` even if the predicted
/// deflection at that depth still exceeds the 200 µm target.
/// Synthetic case: unrealistically long stickout (150 mm) on a
/// 2 mm endmill in HardMaple — δ ∝ L³/d⁴ keeps prediction huge
/// even at 0.5 mm DPP (the loop's floor). Pre-back-off DPP set to
/// 1.0 mm so the 0.8-factor reduction reaches the floor in 3-4
/// iterations rather than exhausting the 5-iteration max first.
#[test]
fn deflection_back_off_respects_05mm_floor() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    use crate::material::WoodSpecies;
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        stepover: 1.0,
        // Start at 1.0 mm so 0.8^N drops to 0.5 in <5 iterations
        // (log(0.5/1.0)/log(0.8) ≈ 3.1 steps). Otherwise the
        // 5-iteration cap would dominate over the floor and we'd
        // exit on iterations, not on the floor we're trying to
        // exercise.
        depth_per_pass: 1.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    // Very small core diameter (2 mm → core 1.4 mm, I ≈ 0.19 mm⁴)
    // combined with extreme stickout drives δ ∝ L³/d⁴ above the
    // 200 µm threshold even at the 0.5 mm DPP floor.
    tool.diameter = 2.0;
    tool.cutting_length = 25.0;
    tool.stickout = 220.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::default();
    // Disable the rigidity clamp so 1.0 mm starting DPP survives
    // to the back-off loop (1 < 2 × 0.20 = 0.4 would otherwise
    // already clamp). The loop is what we're testing.
    machine.rigidity.doc_roughing_factor = 5.0;
    machine.rigidity.adaptive_doc_factor = 5.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext::default(),
    );

    // Phase 3 (Pass 0, axial-DOC envelope): the cutter-axial-constraints
    // calculator runs the canonical deflection model via binary search,
    // so for the same pathological inputs it now clamps DPP **below**
    // the v1.1 back-off floor — the envelope is the more accurate
    // constraint and it binds first. The v1.1 back-off then short-
    // circuits because `current > FLOOR` is already false. Test asserts
    // the new behaviour: envelope-clamped DPP < FLOOR + AxialDocClampedByEnvelope
    // warning carries the deflection-binding signal.
    let dpp_after = op.depth_per_pass().expect("dpp set");
    assert!(
        dpp_after < DEFLECTION_BACKOFF_DPP_FLOOR_MM,
        "Phase 3 envelope must clamp DPP below the v1.1 back-off floor on \
             pathological tools (2 mm × 150 mm stickout); got {dpp_after}"
    );
    let envelope_warning_present = warnings.iter().any(|w| {
        matches!(
            w,
            SuggestWarning::AxialDocClampedByEnvelope {
                binding: "deflection",
                ..
            } | SuggestWarning::AxialEnvelopeSafeBandEmpty { .. }
        )
    });
    assert!(
        envelope_warning_present,
        "Phase 3 envelope must surface an Axial* warning when it clamps DPP, got {warnings:?}"
    );
}

/// v1.2 combined-Suggest (2026-06-04 design doc § v1.2): Wanaka 3D
/// Finish motivating case. A 2 mm-tip tapered-ball on a 140 × 150 mm
/// stock with a 0.03 mm stepover (scallop-height math taken
/// literally for a small tip radius) predicts millions of moves on
/// the model envelope. The back-off must raise stepover until the
/// predicted move count clears 500 k, and emit
/// `StepoverRaisedForRuntime`.
///
/// The test calls `enforce_invariants` directly with a pre-written
/// 0.03 mm stepover (the LUT scallop path that produced it isn't
/// re-exercised here — `predict::tests::predict_move_count_zero_for_v_carve`
/// guards the predictor itself and the LUT/scallop path is already
/// covered by `scallop_height_some_overrides_default_ae_factor`).
#[test]
fn stepover_raised_for_wanaka_3d_finish_case() {
    use crate::compute::operation_configs::DropCutterConfig;
    let mut op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 0.03,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        scallop_height: Some(0.010),
        ..DropCutterConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
    tool.diameter = 4.0; // tip diameter 4 → tip radius 2 mm
    tool.cutting_length = 25.0;
    // Wanaka envelope: 140 × 150 × 12 mm. Even at the diameter ×
    // 0.5 = 2 mm ceiling the predicted moves = (150/2) × (140/0.4)
    // ≈ 26 250 — well below 500 k, so the loop should converge
    // before bailing on the ceiling.
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -12.0),
        max: crate::geo::P3::new(140.0, 150.0, 0.0),
    };
    let machine = MachineProfile::default();
    let material = Material::default();
    let context = SuggestContext {
        model_bbox: Some(&bbox),
        ..SuggestContext::default()
    };

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Finish,
        context,
    );

    let stepover_after = op.stepover().expect("stepover must remain set");
    assert!(
        stepover_after > 0.03,
        "back-off must raise stepover above the 0.03 mm starting point, got {stepover_after}"
    );
    assert!(
        stepover_after <= tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION + 1e-9,
        "raised stepover must stay at or below tool.diameter × 0.5 = {} mm, got {stepover_after}",
        tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION
    );
    // The final predicted move count must come down below the
    // 500 k target (the back-off converged before the ceiling).
    let moves_after = crate::feeds::predict::predict_move_count(&op, Some(&bbox), &tool);
    assert!(
        moves_after <= STEPOVER_BACKOFF_TARGET_MOVES,
        "post-back-off move count must clear {STEPOVER_BACKOFF_TARGET_MOVES}, got {moves_after}"
    );

    let warning = warnings.iter().find_map(|w| match w {
        SuggestWarning::StepoverRaisedForRuntime {
            requested_mm,
            raised_mm,
            predicted_moves_at_requested,
            predicted_moves_at_raised,
            iterations,
        } => Some((
            *requested_mm,
            *raised_mm,
            *predicted_moves_at_requested,
            *predicted_moves_at_raised,
            *iterations,
        )),
        _ => None,
    });
    let (
        requested_mm,
        raised_mm,
        predicted_moves_at_requested,
        predicted_moves_at_raised,
        iterations,
    ) = warning.expect("StepoverRaisedForRuntime must fire on Wanaka 3D Finish case");
    assert!(
        (requested_mm - 0.03).abs() < 1e-6,
        "warning.requested_mm must capture the pre-back-off stepover, got {requested_mm}"
    );
    assert!(
        (raised_mm - stepover_after).abs() < 1e-9,
        "warning.raised_mm must match the post-back-off stepover, got {raised_mm} vs {stepover_after}"
    );
    assert!(
        predicted_moves_at_requested > STEPOVER_BACKOFF_TARGET_MOVES,
        "warning.predicted_moves_at_requested must exceed target (loop wouldn't have started otherwise), got {predicted_moves_at_requested}"
    );
    assert!(
        predicted_moves_at_raised <= STEPOVER_BACKOFF_TARGET_MOVES,
        "warning.predicted_moves_at_raised must clear target (no ceiling bail expected), got {predicted_moves_at_raised}"
    );
    assert!(
        (1..=STEPOVER_BACKOFF_MAX_ITERATIONS).contains(&iterations),
        "iterations must fall in [1, {}], got {iterations}",
        STEPOVER_BACKOFF_MAX_ITERATIONS
    );
}

/// v1.2 counter-test: when `context.model_bbox` is `None` the
/// predictor returns 0 ("no constraint signal") and the back-off
/// must short-circuit. Guards against the predictor being read
/// pessimistically (treating None as "infinite envelope") and
/// against `enforce_invariants` raising stepover when it has no
/// envelope information to gate on.
#[test]
fn stepover_unchanged_when_model_bbox_unknown() {
    use crate::compute::operation_configs::DropCutterConfig;
    let mut op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 0.03,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        scallop_height: Some(0.010),
        ..DropCutterConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
    tool.diameter = 4.0;
    tool.cutting_length = 25.0;
    let machine = MachineProfile::default();
    let material = Material::default();
    // No model_bbox — the back-off must short-circuit on the
    // predictor's 0 return.
    let context = SuggestContext::default();

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Finish,
        context,
    );

    assert_eq!(
        op.stepover(),
        Some(0.03),
        "stepover must be left untouched when model_bbox is None (no constraint signal)"
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::StepoverRaisedForRuntime { .. })),
        "StepoverRaisedForRuntime must not fire without a model_bbox, got {warnings:?}"
    );
}

/// v1.2 floor counter-test: when even raising stepover to
/// `tool.diameter × 0.5` would not bring the predicted move count
/// below the target, the loop must land on the diameter-fraction
/// ceiling and emit the warning (the `predicted_moves_at_raised`
/// field still exceeds the target — that's the "unmet gate" signal
/// until the v2 surface adds a structured unmet-condition channel).
///
/// Synthetic case: enormous stock envelope (5 × 5 m DropCutter scan)
/// on a small-tip tapered-ball. At the 0.5 × D ceiling the predicted
/// move count is still in the millions.
#[test]
fn stepover_floor_caps_back_off() {
    use crate::compute::operation_configs::DropCutterConfig;
    // Start at a stepover where 1.5⁵ growth reaches the
    // diameter-fraction ceiling before the iteration cap fires:
    // tool.diameter=2.0 → ceiling=1.0. Starting at 0.3 mm,
    // 0.3 × 1.5² = 0.675 (iter 2), × 1.5³ = 1.0125 (clamped to
    // ceiling) on iter 3 — bails on ceiling, not on max iterations.
    let mut op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 0.3,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        scallop_height: Some(0.010),
        ..DropCutterConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
    tool.diameter = 2.0; // tip diameter 2 → ceiling = 1.0 mm
    tool.cutting_length = 25.0;
    // 5 000 × 5 000 mm envelope. At ceiling 1.0 mm:
    //   passes = 5000 / 1.0 = 5000
    //   moves_per_pass = 5000 / (2.0 × 0.1) = 25 000
    //   total ≈ 1.25 × 10⁸ — well above 500 k.
    let bbox = crate::geo::BoundingBox3 {
        min: crate::geo::P3::new(0.0, 0.0, -10.0),
        max: crate::geo::P3::new(5000.0, 5000.0, 0.0),
    };
    let machine = MachineProfile::default();
    let material = Material::default();
    let context = SuggestContext {
        model_bbox: Some(&bbox),
        ..SuggestContext::default()
    };

    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Finish,
        context,
    );

    let stepover_after = op.stepover().expect("stepover must be set");
    let ceiling = tool.diameter * STEPOVER_BACKOFF_DIAMETER_FRACTION;
    assert!(
        (stepover_after - ceiling).abs() < 1e-6,
        "back-off must land on the ceiling ({ceiling} mm) when target is unreachable, got {stepover_after}"
    );
    let warning = warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::StepoverRaisedForRuntime {
                raised_mm,
                predicted_moves_at_raised,
                iterations,
                ..
            } => Some((*raised_mm, *predicted_moves_at_raised, *iterations)),
            _ => None,
        })
        .expect("warning must still fire when bailing on the diameter-fraction ceiling");
    let (raised_mm, predicted_moves_at_raised, iterations) = warning;
    assert!(
        (raised_mm - ceiling).abs() < 1e-6,
        "warning.raised_mm must equal the ceiling when loop bails, got {raised_mm}"
    );
    assert!(
        predicted_moves_at_raised > STEPOVER_BACKOFF_TARGET_MOVES,
        "warning.predicted_moves_at_raised must still exceed target to signal an unmet gate, got {predicted_moves_at_raised}"
    );
    assert!(
        iterations < STEPOVER_BACKOFF_MAX_ITERATIONS,
        "loop must bail on ceiling, not on max-iterations cap (iterations={iterations}, max={STEPOVER_BACKOFF_MAX_ITERATIONS})"
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
        feed_rate: 911.0,
        plunge_rate: 300.0,
        stepover: 1.2,
        // Post-v1.1-back-off DPP. Helix entry so we don't also
        // fire PlungeEntryUnstableAtDpp.
        depth_per_pass: 3.69,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    // Stiffened (45 → 18 mm) by the pre-fix version so deflection left
    // the feed-up loop headroom. Kept so the fixture is bit-identical
    // to the one that produced 3456 mm/min.
    tool.stickout = 18.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::default();
    // Disable the rigidity clamp at DPP=3.69 (3.69 < 1.6 × 6 = 9.6 is OK).
    machine.rigidity.doc_roughing_factor = 0.20;
    machine.rigidity.adaptive_doc_factor = 1.60;
    machine.max_feed_mm_min = 10_000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.027,
        max_mm_per_tooth: 0.060,
    });

    let initial_feed = op.feed_rate();
    let warnings = enforce_invariants(
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
        (op.feed_rate() - initial_feed).abs() < 1e-6,
        "retired pass 8 must leave feed at the calculator value {initial_feed} \
             (pre-fix it was lifted to 3456), got {}",
        op.feed_rate()
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
            feed_rate: initial_feed,
            plunge_rate: 300.0,
            stepover: 1.2,
            depth_per_pass: 3.69,
            spindle_rpm: Some(16_000),
            entry_style: Adaptive3dEntryStyle::Helix,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // δ ∝ stickout³ — this is the value that lands the prediction in the
        // (190, 200) µm window. Retune if the force physics shifts; the setup
        // guard below asserts the window.
        //
        // Retuned 100.0 → 99.0 for T-17. The equivalent bending section made
        // this 2-flute tool 1.60× more compliant, so 100.0 mm read 200.7 µm
        // and broke the upper edge of the window. The window is the fixture's
        // premise, not the claim under test.
        tool.stickout = 99.0;
        tool.flute_count = 2;
        let mut machine = MachineProfile::default();
        machine.rigidity.doc_roughing_factor = 0.20;
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

        let warnings = enforce_invariants(
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
            (op.feed_rate() - initial_feed).abs() < 1e-6,
            "{aggressiveness:?}: feed must stay at the calculator value {initial_feed}, got {}",
            op.feed_rate()
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
        feed_rate: 4000.0,
        plunge_rate: 300.0,
        stepover: 1.2,
        depth_per_pass: 2.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.stickout = 25.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.20;
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

    let warnings = enforce_invariants(
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
        (op.feed_rate() - 4000.0).abs() < 1e-6,
        "feed must stay at the machine cap, got {}",
        op.feed_rate()
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
        feed_rate: 4000.0,
        plunge_rate: 500.0,
        stepover: 1.2,
        depth_per_pass: 2.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Helix,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.stickout = 25.0;
    tool.flute_count = 2;
    let mut machine = MachineProfile::default();
    machine.rigidity.doc_roughing_factor = 0.20;
    machine.rigidity.adaptive_doc_factor = 1.60;
    machine.max_feed_mm_min = 10_000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let bounds = Some(ChiploadBounds {
        min_mm_per_tooth: 0.025,
        max_mm_per_tooth: 0.050,
    });

    let initial_feed = op.feed_rate();
    let warnings = enforce_invariants(
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
        (op.feed_rate() - initial_feed).abs() < 1e-6,
        "feed must be unchanged when observed chipload already in band, got {} vs {initial_feed}",
        op.feed_rate()
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

/// v3.3b: strategy-aware orchestrator rewrites Adaptive3d
/// `entry_style` from Plunge to Ramp on Roughing ops when:
///   * `policy.scope == StrategyAndFeeds`
///   * `DPP / D > 0.5` (i.e. the same threshold v1.3's plunge-entry
///     warning fires on)
///   * `entry_style == Default::default() == Plunge` (the v3 design
///     doc's "B" pinning heuristic — only rewrite unpinned values)
///
/// and DOES NOT fire under the v3.3a default `FeedsWithGates`
/// scope. Wanaka Back Rough scaffold: 6 mm carbide endmill, DPP
/// 3.69 mm (ratio 0.62), entry_style=Plunge.
#[test]
fn strategy_aware_rewrites_plunge_to_ramp_under_default_scope() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
    let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        depth_per_pass: 3.69,
        stepover: 1.2,
        feed_rate: 911.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Plunge,
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    // Short stickout keeps the closed-form deflection predictor
    // below the 200 µm back-off threshold at DPP 3.69 mm, so
    // pass 6 doesn't lower DPP and pass 7's DPP/D = 0.615 stays
    // above the 0.5 rewrite threshold.
    tool.stickout = 30.0;
    tool.flute_count = 2;
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };

    // Scope = StrategyAndFeeds → rewrite fires.
    let warnings = enforce_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            policy: SuggestPolicy {
                scope: SuggestScope::StrategyAndFeeds,
            },
            ..SuggestContext::default()
        },
    );
    let OperationConfig::Adaptive3d(cfg) = &op else {
        panic!("op must remain Adaptive3d after enforce_invariants")
    };
    assert_eq!(
        cfg.entry_style,
        Adaptive3dEntryStyle::Ramp,
        "StrategyAndFeeds must rewrite Plunge → Ramp at DPP/D > 0.5"
    );
    assert!(
        warnings.iter().any(|w| matches!(
            w,
            SuggestWarning::StrategyRewrote {
                param: "entry_style",
                ..
            }
        )),
        "StrategyRewrote warning must fire, got {warnings:?}"
    );
    // v1.3 PlungeEntryUnstableAtDpp must NOT fire — the rewrite
    // already handled it.
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
        "PlungeEntryUnstableAtDpp must not fire after auto-rewrite, got {warnings:?}"
    );

    // Scope = FeedsWithGates → no rewrite, v1.3 warning fires.
    let mut op2 = OperationConfig::Adaptive3d(Adaptive3dConfig {
        depth_per_pass: 3.69,
        stepover: 1.2,
        feed_rate: 911.0,
        spindle_rpm: Some(16_000),
        entry_style: Adaptive3dEntryStyle::Plunge,
        ..Adaptive3dConfig::default()
    });
    let warnings2 = enforce_invariants(
        &mut op2,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext {
            policy: SuggestPolicy {
                scope: SuggestScope::FeedsWithGates,
            },
            ..SuggestContext::default()
        },
    );
    let OperationConfig::Adaptive3d(cfg2) = &op2 else {
        panic!("op2 must remain Adaptive3d")
    };
    assert_eq!(
        cfg2.entry_style,
        Adaptive3dEntryStyle::Plunge,
        "FeedsWithGates must leave Plunge untouched"
    );
    assert!(
        warnings2
            .iter()
            .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
        "PlungeEntryUnstableAtDpp must fire under FeedsWithGates, got {warnings2:?}"
    );
}

/// v3.3b pinning sentry (design-doc `strategy_pinning_respected`):
/// when the user has explicitly set `entry_style` to a non-default
/// value (Helix or Ramp), the strategy auto-pick must NOT stomp it
/// even under StrategyAndFeeds — including the plan's exact
/// scenario of Ramp pinned on an op where the chooser would
/// otherwise pick Helix/Ramp itself (heuristic B: any value ≠
/// `Default::default()` is treated as pinned).
#[test]
fn strategy_aware_leaves_non_default_entry_style_alone() {
    use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};

    for pinned in [Adaptive3dEntryStyle::Helix, Adaptive3dEntryStyle::Ramp] {
        let mut op = OperationConfig::Adaptive3d(Adaptive3dConfig {
            depth_per_pass: 3.69,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            entry_style: pinned,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        // Short stickout keeps the closed-form deflection predictor
        // below the 200 µm back-off threshold at DPP 3.69 mm, so
        // pass 6 doesn't lower DPP and pass 7's DPP/D = 0.615 stays
        // above the 0.5 rewrite threshold.
        tool.stickout = 30.0;
        tool.flute_count = 2;
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };

        let warnings = enforce_invariants(
            &mut op,
            &tool,
            &machine,
            &material,
            PassRole::Roughing,
            SuggestContext {
                policy: SuggestPolicy {
                    scope: SuggestScope::StrategyAndFeeds,
                },
                ..SuggestContext::default()
            },
        );
        let OperationConfig::Adaptive3d(cfg) = &op else {
            panic!("op must remain Adaptive3d")
        };
        assert_eq!(
            cfg.entry_style, pinned,
            "user-set {pinned:?} must not be stomped"
        );
        assert!(
            !warnings.iter().any(|w| matches!(
                w,
                SuggestWarning::StrategyRewrote {
                    param: "entry_style",
                    ..
                }
            )),
            "no StrategyRewrote for entry_style when user-pinned {pinned:?}, got {warnings:?}"
        );
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
        depth_per_pass: 2.0,
        stepover: 1.2,
        feed_rate: 911.0,
        spindle_rpm: Some(16_000),
        ..Adaptive3dConfig::default()
    });
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.stickout = 30.0;
    tool.flute_count = 2;
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: crate::material::WoodSpecies::HardMaple,
    };

    let warnings = enforce_invariants(
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
        _ => None,
    });
    let (current, recommended, reason) = recommended
        .expect("clearing-strategy recommendation must fire on unpinned MixedTerrain op");
    assert_eq!(current, "contour_parallel");
    assert_eq!(recommended, "adaptive");
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
            depth_per_pass: 2.0,
            stepover: 1.2,
            feed_rate: 911.0,
            spindle_rpm: Some(16_000),
            clearing_strategy: strategy,
            ..Adaptive3dConfig::default()
        });
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 6.0;
        tool.cutting_length = 25.0;
        tool.stickout = 30.0;
        tool.flute_count = 2;
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::HardMaple,
        };
        enforce_invariants(
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
    for pinned in [ClearingStrategy::Adaptive, ClearingStrategy::AgentSearch] {
        no_recommendation(
            &run(pinned, SuggestScope::StrategyAndFeeds, Some(&bbox)),
            &format!("pinned {pinned:?}"),
        );
    }
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

/// PRE-Phase-1 coverage net (architectural refactor T1): every
/// operation's feeds hints must be an *explicit decision*. The
/// implementation now lives in the registry-layer
/// [`OperationConfig::feeds_hints`] exhaustive match (T3 replaced
/// the old `_ => (None, None, None)` wildcard here); this net stays
/// as the independent baseline guarding both the table and the tuple
/// adapter.
///
/// The expected table below is an exhaustive match over
/// `OperationConfig` with **no wildcard arm** — adding a new
/// operation fails to compile here, forcing the author to decide
/// (and record) what hints the new op feeds the calculator. Changing
/// an existing op's hint plumbing fails the assert instead of
/// silently rerouting the feeds suggestion.
#[test]
fn operation_feeds_hints_is_an_explicit_per_op_decision() {
    use crate::compute::catalog::OperationType;

    for &op_type in OperationType::ALL {
        let config = OperationConfig::new_default(op_type);
        let actual = operation_feeds_hints(&config);

        #[allow(clippy::match_same_arms)] // one arm per op = the point
        let expected: (Option<f64>, Option<f64>, Option<f64>) = match &config {
            // Ops that feed operation-specific hints into the
            // calculator (axial, radial, scallop):
            OperationConfig::Scallop(cfg) => (None, None, Some(cfg.scallop_height)),
            OperationConfig::UnifiedFinish(cfg) => (None, None, Some(cfg.scallop_height)),
            OperationConfig::DropCutter(cfg) => (None, None, cfg.scallop_height),
            OperationConfig::Waterline(cfg) => (Some(cfg.z_step), None, None),
            OperationConfig::SteepShallow(cfg) => (Some(cfg.z_step), None, None),
            OperationConfig::VCarve(cfg) => (Some(cfg.max_depth), None, None),
            OperationConfig::RampFinish(cfg) => (Some(cfg.max_stepdown), None, None),
            // Ops that have explicitly DECIDED to provide no hints —
            // the calculator falls back to LUT/diameter-factor
            // defaults. Listed individually (not `_`) so each is a
            // recorded decision:
            OperationConfig::Face(_) => (None, None, None),
            OperationConfig::Pocket(_) => (None, None, None),
            OperationConfig::Profile(_) => (None, None, None),
            OperationConfig::Adaptive(_) => (None, None, None),
            OperationConfig::Rest(_) => (None, None, None),
            OperationConfig::Inlay(_) => (None, None, None),
            OperationConfig::Zigzag(_) => (None, None, None),
            OperationConfig::Trace(_) => (None, None, None),
            OperationConfig::Drill(_) => (None, None, None),
            OperationConfig::Chamfer(_) => (None, None, None),
            OperationConfig::Adaptive3d(_) => (None, None, None),
            OperationConfig::Pencil(_) => (None, None, None),
            OperationConfig::SpiralFinish(_) => (None, None, None),
            OperationConfig::RadialFinish(_) => (None, None, None),
            OperationConfig::HorizontalFinish(_) => (None, None, None),
            OperationConfig::ProjectCurve(_) => (None, None, None),
            OperationConfig::AlignmentPinDrill(_) => (None, None, None),
        };

        assert_eq!(
            actual, expected,
            "{op_type:?}: operation_feeds_hints changed — update \
                 operation_feeds_hints AND this expected table deliberately \
                 (this net guards the suggest.rs (None,None,None) wildcard)"
        );
    }
}
