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
//! **Fixture moves (2026-09-26).** The file was red from 2026-09-23. Each
//! cause, and what the fixture does now:
//!
//! - `0007528` (feeds ruling R1): Suggest refuses a tool that the
//!   operation's registry row refuses. Inlay allows only a V-bit, so the
//!   Inlay fixture takes the fixture V-bit, not an end mill.
//! - `8702706` (ruling R4): the machine aggressiveness dial acts after the
//!   rigidity cap. The demo pocket funnel now writes **0.75 mm**, not 1.2:
//!   the rigidity cap gives 0.20 x 6.0 = 1.2; the dial target is
//!   k x ld = 0.85 x 0.75 = 0.6375 of the base load (`DEFAULT_AGGRESSIVENESS`
//!   0.85; the stick-out 45 / 6.0 = 7.5 is over the L/D 6 threshold, so the
//!   long-tool share is 0.75); the solve scale 0.688 gives 0.825 mm, which
//!   snaps to 3.0 / 4 passes = 0.75 mm (`realised_step_down`). The pre-fix
//!   pill ratio is 4.2 / 0.75 = 5.6x, not 3.5x. The test reads both stages
//!   from the funnel's own warnings, so the next move names its stage.
//! - `d5b7e34` (ruling B4): a V-bit row is read at its printed key, the
//!   nominal diameter. The default V-bit (12.7 mm, 90 degrees) is 2.1x the
//!   nearest printed row (6 mm), outside the 0.5x-2x window of the size
//!   law, so it refuses. The fixture V-bit is 6.0 mm, 90 degrees: the
//!   printed key of `amana-vbit-softwood-trace-6000-2f`.
//! - 2026-10-01: that row is retired (finding G5 D1: not on its chart). The
//!   6.0 mm fixture now reads the nearest printed 90 degree row, the Onsrud
//!   37-50 1/4 in Soft Wood row `onsrud-softwood-37-50-1_4-trace`. The
//!   fixture size stays; no assert of this file reads the V-bit's numbers.
//!
//! The pill now reads its value from
//! `feeds::suggest::preview_field_applies` — a dry run of the funnel on a
//! scratch clone, read back per field — so the clamp chain is never
//! duplicated in the GUI. This sentry pins that helper against the funnel.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    FieldApplyPreviews, SuggestContext, feeds_result_for_operation, preview_field_applies,
    preview_field_apply, round_suggestion_value,
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
        FeedsField::FeedRate => op.feed_rate(),
        FeedsField::PlungeRate => op.plunge_rate(),
        FeedsField::SpindleRpm => op.spindle_rpm().map(f64::from),
        FeedsField::Stepover => op.stepover(),
        FeedsField::DepthPerPass => op.depth_per_pass(),
        FeedsField::ScallopHeight => op.scallop_height(),
        FeedsField::RampFeedRate => op.ramp_feed_rate(),
    }
}

/// The demo pocket: Ø6 two-flute flat end mill on a fresh Pocket op.
/// Feeds contract P5 (K2b): the apply that "Apply all" makes, the
/// envelope proposal pick through the one funnel, with the default context.
fn apply_proposal(
    session: &ProjectSession,
    tool: &ToolConfig,
    op: &OperationConfig,
) -> (OperationConfig, FeedsProvenance) {
    let mut applied = op.clone();
    let mut prov = FeedsProvenance::default();
    rs_cam_core::feeds::suggest::apply_to_operation(
        rs_cam_core::feeds::suggest::OperationApplyInput {
            tool,
            machine: session.machine(),
            material: &session.stock_config().material,
            spindle_strategy: session.post_config().spindle_strategy,
            lut: rs_cam_core::feeds::embedded_vendor_lut(),
            suggest: SuggestContext::default(),
        },
        Some(rs_cam_core::feeds::envelope::PickRequest::Proposal),
        None,
        &mut applied,
        &mut prov,
    )
    .expect("the fixture pairing is valid");
    (applied, prov)
}

fn demo_pocket() -> (ProjectSession, ToolConfig, OperationConfig) {
    let session = ProjectSession::new_empty();
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
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

/// The fixture V-bit: 6.0 mm, 90 degrees. It was the printed key of the
/// Amana softwood V-bit row `amana-vbit-softwood-trace-6000-2f`, retired
/// 2026-10-01 (finding G5 D1); it now reads the Onsrud 37-50 1/4 in Soft
/// Wood row `onsrud-softwood-37-50-1_4-trace`. Ruling B4
/// (`d5b7e34`, 2026-09-25) reads a V-bit row at its nominal diameter; the
/// default V-bit (12.7 mm) was 2.1x the 6 mm row and refused. Since
/// 2026-10-01 it has its own printed row (Onsrud 37-60 1/2 in Soft Wood).
const FIXTURE_VBIT_DIAMETER_MM: f64 = 6.0;
const FIXTURE_VBIT_ANGLE_DEG: f64 = 90.0;

/// The tool of a fixture: the type's default, except on a drill and on a
/// V-bit. Ruling B5 (G6, 2026-09-24; range widened 2026-09-25): a drill
/// ships only through the drill claim (a 2- or 3-flute flat end mill at
/// 3.0-12.7 mm); a drill fixture takes a 6.0 mm end mill. Before B5 every
/// drill cell in softwood refused (ruling R1). A V-bit fixture takes the
/// fixture V-bit (ruling B4).
fn fixture_tool(op_type: OperationType, tool_type: ToolType) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(1), tool_type);
    if matches!(
        op_type,
        OperationType::Drill | OperationType::AlignmentPinDrill
    ) && tool_type == ToolType::EndMill
    {
        tool.diameter = 6.0;
    }
    if tool_type == ToolType::VBit {
        tool.diameter = FIXTURE_VBIT_DIAMETER_MM;
        tool.included_angle = FIXTURE_VBIT_ANGLE_DEG;
    }
    tool
}

fn recipe(session: &ProjectSession, tool: &ToolConfig, op: &OperationConfig) -> FeedsResult {
    try_recipe(session, tool, op).expect("valid tool × operation pairing")
}

fn previews(
    session: &ProjectSession,
    tool: &ToolConfig,
    op: &OperationConfig,
    // The recommendation the pill reads beside the preview. P5: the dry
    // run builds its own envelope from the inputs, so it does not read it.
    _result: &FeedsResult,
) -> FieldApplyPreviews {
    preview_field_applies(
        op,
        tool,
        session.machine(),
        &session.stock_config().material,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
        SuggestContext::default(),
    )
}

/// What `ValueRow::show` wrote on a pill click at HEAD before the fix.
fn pill_write_pre_fix(result: &FeedsResult) -> f64 {
    round_suggestion_value(result.axial_depth_mm, 0.001)
}

// ── the finding ─────────────────────────────────────────────────────────────

/// UX-R03-014 acceptance: on the demo pocket the Depth/Pass pill offers the
/// funnel's 0.75, not 4.2, and what it offers is bit-identical to what Apply
/// cut geometry writes. The funnel's chain is read from its own warnings:
/// the rigidity cap (1.2), then the R4 dial (0.75).
#[test]
fn depth_per_pass_pill_offers_the_funnel_value_not_the_calculator_value() {
    // Feeds contract P5 (K2b, ruling Q4): `⚡ Apply all` picks the envelope
    // proposal, min(stated, binding cap); the dial no longer acts on a
    // product door. The pill offers the proposal pick's write, bit for bit,
    // with its stamp (the pick, never a typed value).
    let (session, tool, op) = demo_pocket();
    let (funnel_op, prov) = apply_proposal(&session, &tool, &op);
    let funnel_dpp = funnel_op.depth_per_pass().expect("pocket carries DOC");
    let stated = op.depth_per_pass().expect("pocket carries DOC");
    assert!(
        funnel_dpp <= stated + 1e-12,
        "the proposal raised the stated DOC"
    );

    let pill = preview_field_apply(
        &op,
        &tool,
        session.machine(),
        &session.stock_config().material,
        rs_cam_core::feeds::embedded_vendor_lut(),
        session.post_config().spindle_strategy,
        SuggestContext::default(),
        FeedsField::DepthPerPass,
    )
    .expect("the funnel writes DOC on a Pocket");

    assert_eq!(
        pill.value.to_bits(),
        funnel_dpp.to_bits(),
        "the Depth/Pass ⚡ pill offers {} mm where Apply all writes {funnel_dpp} mm",
        pill.value
    );
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
/// anything and needs rebasing. Feeds contract P5 (K1/K2): the calculator
/// reads the stated DOC, and the proposal lowers it to the binding cap (the
/// rigidity cap on the demo pocket), so the raw write sits above the pill.
#[test]
fn the_fixture_still_shows_the_clamp() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);
    let p = previews(&session, &tool, &op, &result);
    let dpp = p.depth_per_pass.expect("pocket DOC");
    assert!(
        pill_write_pre_fix(&result) > dpp.value,
        "pre-fix pill {} is not above the funnel's {}; the fixture proves nothing",
        pill_write_pre_fix(&result),
        dpp.value
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
        // Inlay allows only a V-bit (registry tool rule, R1 `0007528`).
        (OperationType::Inlay, ToolType::VBit),
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
        let tool = fixture_tool(op_type, tool_type);
        let op = OperationConfig::new_default(op_type);
        let Some(result) = try_recipe(&session, &tool, &op) else {
            panic!("{op_type:?} on {tool_type:?} is refused — pick a valid tool for the fixture");
        };
        let p = previews(&session, &tool, &op, &result);

        let (applied, prov) = apply_proposal(&session, &tool, &op);

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

/// The 22 pill sites, classified. A `Some` row is a pill that now writes the
/// funnel's value; a `None` row is a dial the funnel does not write, whose
/// pill offers the raw calculator value labelled "not clamped". Pinned so a
/// later change to an `OperationParams` impl (say VCarve gaining
/// `depth_per_pass`) shows up here as a classification change, not as a
/// silently re-routed pill.
#[test]
fn pill_site_classification_is_pinned() {
    let session = ProjectSession::new_empty();
    let classify = |op_type: OperationType, tool_type: ToolType, field: FeedsField| -> bool {
        let tool = fixture_tool(op_type, tool_type);
        let op = OperationConfig::new_default(op_type);
        let result = recipe(&session, &tool, &op);
        previews(&session, &tool, &op, &result).get(field).is_some()
    };
    use FeedsField::{DepthPerPass, FeedRate, Stepover};
    use OperationType as O;
    use ToolType as T;
    // (site, op, tool, field, funnel writes it?)
    let table: [(&str, O, T, FeedsField, bool); 22] = [
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
        // Feeds contract P5 (K2b): a V groove has no free width (the width
        // follows the depth), so no pick writes the stepover.
        ("VCarve Stepover", O::VCarve, T::VBit, Stepover, false),
        ("Inlay Stepover", O::Inlay, T::VBit, Stepover, false),
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

/// Every pill previews what `⚡ Apply all` writes (feeds contract P5, K2b):
/// the proposal pick, then the speeds at it, from one dry run. Before P5
/// the speeds and the cut were two scoped applies; the scope is gone.
#[test]
fn previews_agree_with_both_scoped_applies() {
    let (session, tool, op) = demo_pocket();
    let result = recipe(&session, &tool, &op);
    let p = previews(&session, &tool, &op, &result);
    let (applied, _) = apply_proposal(&session, &tool, &op);
    for field in FIELDS {
        let preview = p
            .get(field)
            .unwrap_or_else(|| panic!("{field:?} previewed"));
        assert_eq!(
            Some(preview.value.to_bits()),
            read(&applied, field).map(f64::to_bits),
            "{field:?} vs Apply all (the proposal pick)"
        );
    }
}
