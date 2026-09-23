//! G-PILLCLAMP (UX-R03-014, 2026-09-10) — the per-field ⚡ pill writes the
//! value the apply funnel writes, not the raw calculator value.
//!
//! Pre-fix the GUI pill (`ui/components/value_row.rs`) wrote
//! `round_suggestion_value(result.axial_depth_mm, 0.001)` straight into the
//! field. On the demo pocket (Pocket op, Ø6 two-flute flat end mill, default
//! stock, material and machine) that is **4.2 mm**, while `⚡ Apply cut
//! geometry` goes through `apply_feeds_subset` → `enforce_invariants` and
//! writes the rigidity-capped **1.2 mm** (0.20 × 6.0, Generic Wood Router).
//!
//! **Pre-fix run (2026-09-10, first version of this file, which reproduced the
//! pill's arithmetic locally):**
//!
//! ```text
//! assertion `left == right` failed: the Depth/Pass ⚡ pill writes 4.2 mm where
//! Apply cut geometry writes 1.2000000000000002 mm (3.50×) — the pill bypasses
//! enforce_invariants
//! ```
//!
//! Note the funnel's value is the clamp output, NOT on the 0.001 grid: the
//! pill must write the preview verbatim, or it is off by 2e-16 from Apply.
//!
//! The pill now reads its value from
//! `feeds::suggest::preview_field_applies` — a dry run of the funnel on a
//! scratch clone, read back per field — so the clamp chain is never
//! duplicated in the GUI. This sentry pins that helper against the funnel.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyScope, FieldApplyPreviews, SuggestContext, apply_cut_geometry_to_op,
    apply_feeds_result_to_op, apply_speeds_to_op, feeds_result_for_operation,
    preview_field_applies, preview_field_apply, round_suggestion_value,
};
use rs_cam_core::feeds::{FeedsField, FeedsProvenance, FeedsResult};
use rs_cam_core::session::ProjectSession;

const FIELDS: [FeedsField; 5] = [
    FeedsField::FeedRate,
    FeedsField::PlungeRate,
    FeedsField::SpindleRpm,
    FeedsField::Stepover,
    FeedsField::DepthPerPass,
];

fn read(op: &OperationConfig, field: FeedsField) -> Option<f64> {
    match field {
        FeedsField::FeedRate => Some(op.feed_rate()),
        FeedsField::PlungeRate => Some(op.plunge_rate()),
        FeedsField::SpindleRpm => op.spindle_rpm().map(f64::from),
        FeedsField::Stepover => op.stepover(),
        FeedsField::DepthPerPass => op.depth_per_pass(),
        FeedsField::ScallopHeight => op.scallop_height(),
    }
}

/// The demo pocket: Ø6 two-flute flat end mill on a fresh Pocket op.
fn demo_pocket() -> (ProjectSession, ToolConfig, OperationConfig) {
    let session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    let op = OperationConfig::new_default(OperationType::Pocket);
    (session, tool, op)
}

fn try_recipe(
    session: &ProjectSession,
    tool: &ToolConfig,
    op: &OperationConfig,
) -> Option<FeedsResult> {
    let stock = session.stock_config();
    feeds_result_for_operation(
        op,
        tool,
        &stock.material,
        session.machine(),
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
    )
    .ok()
}

fn recipe(session: &ProjectSession, tool: &ToolConfig, op: &OperationConfig) -> FeedsResult {
    try_recipe(session, tool, op).expect("valid tool × operation pairing")
}

fn previews(
    session: &ProjectSession,
    tool: &ToolConfig,
    op: &OperationConfig,
    result: &FeedsResult,
) -> FieldApplyPreviews {
    preview_field_applies(
        op,
        result,
        tool,
        session.machine(),
        &session.stock_config().material,
        op.feeds_style().1,
        SuggestContext::default(),
    )
}

/// What `ValueRow::show` wrote on a pill click at HEAD before the fix.
fn pill_write_pre_fix(result: &FeedsResult) -> f64 {
    round_suggestion_value(result.axial_depth_mm, 0.001)
}

// ── the finding ─────────────────────────────────────────────────────────────

/// UX-R03-014 acceptance: on the demo pocket the Depth/Pass pill offers 1.2,
/// not 4.2, and what it offers is bit-identical to what Apply cut geometry
/// writes.
#[test]
fn depth_per_pass_pill_offers_the_funnel_value_not_the_calculator_value() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);

    let mut funnel_op = op.clone();
    let mut prov = FeedsProvenance::default();
    apply_cut_geometry_to_op(
        &mut funnel_op,
        &mut prov,
        &result,
        &tool,
        session.machine(),
        &session.stock_config().material,
        op.feeds_style().1,
        SuggestContext::default(),
    );
    let funnel_dpp = funnel_op.depth_per_pass().expect("pocket carries DOC");
    assert!(
        (funnel_dpp - 1.2).abs() < 1e-9,
        "fixture drift: the funnel wrote {funnel_dpp}, the demo pocket is 0.20 × 6.0 = 1.2"
    );
    assert!(
        (result.axial_depth_mm - 4.2).abs() < 1e-9,
        "fixture drift: raw calculator DOC {} (Pocket default 0.70 × 6.0 = 4.2)",
        result.axial_depth_mm
    );

    let pill = preview_field_apply(
        &op,
        &result,
        &tool,
        session.machine(),
        &session.stock_config().material,
        op.feeds_style().1,
        SuggestContext::default(),
        FeedsField::DepthPerPass,
    )
    .expect("the funnel writes DOC on a Pocket");

    assert_eq!(
        pill.value.to_bits(),
        funnel_dpp.to_bits(),
        "the Depth/Pass ⚡ pill offers {} mm where Apply cut geometry writes {funnel_dpp} mm",
        pill.value
    );
    assert_ne!(
        pill.value.to_bits(),
        pill_write_pre_fix(&result).to_bits(),
        "the pill still offers the raw calculator value {}",
        pill_write_pre_fix(&result)
    );
    // And the stamp it records is the recommendation's, the same one the
    // cut-geometry apply recorded — never Manual.
    assert_eq!(
        Some(&pill.provenance),
        prov.depth_per_pass.as_ref(),
        "pill stamp {:?} vs apply stamp {:?}",
        pill.provenance,
        prov.depth_per_pass
    );
}

/// The pre-fix reproduction, kept live: the raw write and the funnel write
/// must keep disagreeing on this fixture, or the fixture no longer proves
/// anything and needs rebasing.
#[test]
fn the_fixture_still_shows_the_clamp() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);
    let p = previews(&session, &tool, &op, &result);
    let dpp = p.depth_per_pass.expect("pocket DOC");
    let ratio = pill_write_pre_fix(&result) / dpp.value;
    assert!(
        (ratio - 3.5).abs() < 1e-6,
        "pre-fix pill ÷ funnel = {ratio:.3}×; the finding measured 3.50×"
    );
}

// ── the contract: preview == funnel write, per field, per operation ─────────

/// Every operation family that carries a ⚡ pill, on the tool that makes the
/// pairing valid. Scallop / UnifiedFinish carry no pill and are refused on a
/// flat tool, so they are not in the list.
fn pill_fixtures() -> Vec<(OperationType, ToolType)> {
    vec![
        (OperationType::Face, ToolType::EndMill),
        (OperationType::Pocket, ToolType::EndMill),
        (OperationType::Profile, ToolType::EndMill),
        (OperationType::Adaptive, ToolType::EndMill),
        (OperationType::VCarve, ToolType::VBit),
        (OperationType::Rest, ToolType::EndMill),
        (OperationType::Inlay, ToolType::EndMill),
        (OperationType::Zigzag, ToolType::EndMill),
        (OperationType::DropCutter, ToolType::EndMill),
        (OperationType::Adaptive3d, ToolType::EndMill),
        (OperationType::SteepShallow, ToolType::BallNose),
        (OperationType::SpiralFinish, ToolType::BallNose),
        (OperationType::HorizontalFinish, ToolType::EndMill),
        (OperationType::Drill, ToolType::EndMill),
        (OperationType::AlignmentPinDrill, ToolType::EndMill),
    ]
}

/// For every field the funnel writes, the preview equals the funnel's write
/// bit-for-bit and carries the funnel's stamp; for every field it does not
/// write, the preview is `None` AND the funnel left the field alone.
#[test]
fn every_field_the_funnel_writes_previews_exactly_what_it_writes() {
    let session = ProjectSession::new_empty();
    let mut covered = 0;
    for (op_type, tool_type) in pill_fixtures() {
        let tool = ToolConfig::new_default(ToolId(1), tool_type);
        let op = OperationConfig::new_default(op_type);
        let Some(result) = try_recipe(&session, &tool, &op) else {
            panic!("{op_type:?} on {tool_type:?} is refused — pick a valid tool for the fixture");
        };
        let p = previews(&session, &tool, &op, &result);

        let mut applied = op.clone();
        let mut prov = FeedsProvenance::default();
        apply_feeds_result_to_op(
            &mut applied,
            &mut prov,
            &result,
            &tool,
            session.machine(),
            &session.stock_config().material,
            op.feeds_style().1,
            SuggestContext::default(),
        );

        for field in FIELDS {
            match p.get(field) {
                Some(preview) => {
                    let written = read(&applied, field)
                        .unwrap_or_else(|| panic!("{op_type:?} {field:?}: previewed but absent"));
                    assert_eq!(
                        preview.value.to_bits(),
                        written.to_bits(),
                        "{op_type:?} {field:?}: preview {} vs funnel write {written}",
                        preview.value
                    );
                    assert_eq!(
                        Some(&preview.provenance),
                        prov.get(field),
                        "{op_type:?} {field:?}: preview stamp vs funnel stamp"
                    );
                    covered += 1;
                }
                None => {
                    assert_eq!(
                        read(&applied, field).map(f64::to_bits),
                        read(&op, field).map(f64::to_bits),
                        "{op_type:?} {field:?}: no preview, yet the funnel moved the field"
                    );
                    assert!(
                        prov.get(field).is_none(),
                        "{op_type:?} {field:?}: no preview, yet the funnel stamped it"
                    );
                }
            }
        }
    }
    assert!(covered >= 40, "only {covered} (op, field) pairs covered");
}

/// The 24 pill sites, classified. A `Some` row is a pill that now writes the
/// funnel's value; a `None` row is a dial the funnel does not write, whose
/// pill offers the raw calculator value labelled "not clamped". Pinned so a
/// later change to an `OperationParams` impl (say VCarve gaining
/// `depth_per_pass`) shows up here as a classification change, not as a
/// silently re-routed pill.
#[test]
fn pill_site_classification_is_pinned() {
    let session = ProjectSession::new_empty();
    let classify = |op_type: OperationType, tool_type: ToolType, field: FeedsField| -> bool {
        let tool = ToolConfig::new_default(ToolId(1), tool_type);
        let op = OperationConfig::new_default(op_type);
        let result = recipe(&session, &tool, &op);
        previews(&session, &tool, &op, &result).get(field).is_some()
    };
    use FeedsField::{DepthPerPass, FeedRate, Stepover};
    use OperationType as O;
    use ToolType as T;
    // (site, op, tool, field, funnel writes it?)
    let table: [(&str, O, T, FeedsField, bool); 24] = [
        // Feeds tab (draw_feeds_card)
        ("Feed", O::Pocket, T::EndMill, FeedRate, true),
        (
            "Plunge",
            O::Pocket,
            T::EndMill,
            FeedsField::PlungeRate,
            true,
        ),
        // boundary_2d.rs
        ("Face Stepover", O::Face, T::EndMill, Stepover, true),
        ("Face Depth/Pass", O::Face, T::EndMill, DepthPerPass, true),
        ("Pocket Stepover", O::Pocket, T::EndMill, Stepover, true),
        (
            "Pocket Depth/Pass",
            O::Pocket,
            T::EndMill,
            DepthPerPass,
            true,
        ),
        (
            "Profile Depth/Pass",
            O::Profile,
            T::EndMill,
            DepthPerPass,
            true,
        ),
        ("Adaptive Stepover", O::Adaptive, T::EndMill, Stepover, true),
        (
            "Adaptive Depth/Pass",
            O::Adaptive,
            T::EndMill,
            DepthPerPass,
            true,
        ),
        ("VCarve Max Depth", O::VCarve, T::VBit, DepthPerPass, false),
        ("VCarve Stepover", O::VCarve, T::VBit, Stepover, true),
        ("Rest Stepover", O::Rest, T::EndMill, Stepover, true),
        ("Rest Depth/Pass", O::Rest, T::EndMill, DepthPerPass, true),
        ("Inlay Stepover", O::Inlay, T::EndMill, Stepover, true),
        ("Zigzag Stepover", O::Zigzag, T::EndMill, Stepover, true),
        (
            "Zigzag Depth/Pass",
            O::Zigzag,
            T::EndMill,
            DepthPerPass,
            true,
        ),
        // surface_3d.rs
        (
            "DropCutter Stepover",
            O::DropCutter,
            T::EndMill,
            Stepover,
            true,
        ),
        (
            "Adaptive3d Stepover",
            O::Adaptive3d,
            T::EndMill,
            Stepover,
            true,
        ),
        (
            "Adaptive3d Depth/Pass",
            O::Adaptive3d,
            T::EndMill,
            DepthPerPass,
            true,
        ),
        (
            "SteepShallow Stepover",
            O::SteepShallow,
            T::BallNose,
            Stepover,
            true,
        ),
        // finishing.rs
        (
            "SpiralFinish Stepover",
            O::SpiralFinish,
            T::BallNose,
            Stepover,
            true,
        ),
        (
            "HorizontalFinish Stepover",
            O::HorizontalFinish,
            T::EndMill,
            Stepover,
            true,
        ),
        // drill.rs
        ("Drill Feed Rate", O::Drill, T::EndMill, FeedRate, true),
        (
            "AlignmentPinDrill Feed Rate",
            O::AlignmentPinDrill,
            T::EndMill,
            FeedRate,
            true,
        ),
    ];
    for (site, op_type, tool_type, field, expected) in table {
        assert_eq!(
            classify(op_type, tool_type, field),
            expected,
            "{site}: funnel-writes-it classification moved"
        );
    }
}

// ── a single-field write moves one field ────────────────────────────────────

/// `FieldApplyPreview::write_to` is the pill's write: the one field lands
/// bit-identical to the preview with the preview's stamp, and every other
/// field and stamp stays where it was.
#[test]
fn a_field_preview_write_moves_only_its_own_field() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);
    let p = previews(&session, &tool, &op, &result);
    for field in FIELDS {
        let Some(preview) = p.get(field) else {
            continue;
        };
        let mut target = op.clone();
        let mut prov = FeedsProvenance::default();
        preview.write_to(&mut target, &mut prov);
        for other in FIELDS {
            if other == field {
                assert_eq!(
                    read(&target, field).map(f64::to_bits),
                    Some(preview.value.to_bits()),
                    "{field:?}: written value"
                );
                assert_eq!(
                    prov.get(field),
                    Some(&preview.provenance),
                    "{field:?}: stamp"
                );
            } else {
                assert_eq!(
                    read(&target, other).map(f64::to_bits),
                    read(&op, other).map(f64::to_bits),
                    "writing {field:?} moved {other:?}"
                );
                assert!(
                    prov.get(other).is_none(),
                    "writing {field:?} stamped {other:?}"
                );
            }
        }
    }
}

/// A speeds-only pill (Feed) previews what `apply_speeds_to_op` writes, and a
/// cut-geometry pill previews what `apply_cut_geometry_to_op` writes — the
/// single dry run serves both scopes because both copy out of one resolved
/// operating point.
#[test]
fn previews_agree_with_both_scoped_applies() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);
    let p = previews(&session, &tool, &op, &result);
    let material = &session.stock_config().material;
    let pass_role = op.feeds_style().1;

    let mut speeds = op.clone();
    let mut sp = FeedsProvenance::default();
    apply_speeds_to_op(
        &mut speeds,
        &mut sp,
        &result,
        &tool,
        session.machine(),
        material,
        pass_role,
        SuggestContext::default(),
    );
    let mut cut = op;
    let mut cp = FeedsProvenance::default();
    apply_cut_geometry_to_op(
        &mut cut,
        &mut cp,
        &result,
        &tool,
        session.machine(),
        material,
        pass_role,
        SuggestContext::default(),
    );

    for field in [
        FeedsField::FeedRate,
        FeedsField::PlungeRate,
        FeedsField::SpindleRpm,
    ] {
        let preview = p
            .get(field)
            .unwrap_or_else(|| panic!("{field:?} previewed"));
        assert_eq!(
            Some(preview.value.to_bits()),
            read(&speeds, field).map(f64::to_bits),
            "{field:?} vs ApplyScope::Speeds"
        );
    }
    for field in [FeedsField::Stepover, FeedsField::DepthPerPass] {
        let preview = p
            .get(field)
            .unwrap_or_else(|| panic!("{field:?} previewed"));
        assert_eq!(
            Some(preview.value.to_bits()),
            read(&cut, field).map(f64::to_bits),
            "{field:?} vs ApplyScope::CutGeometry"
        );
    }
    // Name the scope in the failure text so a reader knows which button the
    // pill is now equivalent to.
    let _ = ApplyScope::Both;
}
