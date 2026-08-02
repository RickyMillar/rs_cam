//! C8 sentry — `narrate_toolpath` stops reporting `regions 0` for Trace,
//! Scallop and SpiralFinish.
//!
//! `ANTIPATTERNS_BACKLOG.md` P8: "`narrate` still reports `regions 0` for
//! Trace, Scallop, and SpiralFinish — the same structural gap A/M8 closed
//! for UnifiedFinish, unclosed for three ops."
//!
//! A/M8's point was that `0` is indistinguishable from "nothing looked". Its
//! fix was to project the operation's OWN region structure into the semantic
//! trace `narrate_toolpath` reads. Applying that here found three different
//! causes wearing one symptom, and this file pins each to its own answer
//! rather than to a shared number:
//!
//! | op | what it actually has | narration |
//! |---|---|---|
//! | Scallop | an independent ring cascade PER machining-boundary region (P2.3), concatenated in region order — a real partition the annotation stream was dropping | `regions N` |
//! | SpiralFinish | ONE continuous archimedean traversal that SKIPS out-of-region samples — no partition exists | `regions 1` |
//! | Trace | a naming divergence, not a missing projection: 15 families route the same structural spans to `Region` items, Trace routes them to `Chain` — so its structure was counted by nothing | `regions 1, chains N` |
//!
//! Inventing a per-boundary partition for the latter two would have reported
//! a structure the generator does not have, which is the failure mode the
//! programme keeps re-learning. `1` is a measurement; `0` was an absence.
//!
//! ## Red-first evidence
//!
//! Before C8, every arm below produced narration containing `regions 0,`
//! while the operation had emitted real cutting, and the Trace arm's chains
//! appeared in no counter at all.
//!
//! ## Why this cannot pass vacuously
//!
//! Every arm asserts the operation cut before reading narration, and asserts
//! the region items reconcile 1:1 with what the trace carries — A/M8's own
//! second gate, so the count cannot be a constant someone typed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{ScallopConfig, SpiralFinishConfig, TraceConfig};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::semantic_trace::ToolpathSemanticKind;
use rs_cam_core::session::ProjectSession;

use common::meshes::sawtooth_plate;
use common::session::{
    mesh_model, pinned_heights, polygon_model, single_op_session_with, square_polygon, stock_over,
};
use common::tools::{ball_tool_config, endmill_tool_config};

const HALF: f64 = 12.0;

/// Count semantic items of one kind.
fn count_kind(session: &ProjectSession, kind: &ToolpathSemanticKind) -> usize {
    session
        .get_result(0)
        .expect("generated result")
        .semantic_trace
        .as_ref()
        .expect("the production path attaches a semantic trace")
        .items
        .iter()
        .filter(|item| item.kind == *kind)
        .count()
}

/// The narration structure line, isolated so a failure prints the sentence
/// under test rather than the whole report.
fn structure_line(session: &ProjectSession) -> String {
    let narration = session.narrate_toolpath(0).expect("narrate");
    narration
        .lines()
        .find(|l| l.starts_with("Semantic trace:"))
        .unwrap_or_else(|| panic!("no semantic-trace line in:\n{narration}"))
        .to_owned()
}

/// Shared assertions: the op cut, narration reports a NON-ZERO region count,
/// and that count is exactly the number of semantic `Region` items — so the
/// narration cannot drift from the trace it claims to summarise (A/M8 gate 2).
fn assert_regions_reported(session: &ProjectSession, label: &str) -> usize {
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.cutting_distance > 0.0,
        "[{label}] the fixture must actually cut, or the region count proves nothing"
    );

    let regions = count_kind(session, &ToolpathSemanticKind::Region);
    let line = structure_line(session);
    println!("[{label}] {line}");

    assert!(
        regions > 0,
        "[{label}] no semantic Region items — this is the regions-0 defect: {line}"
    );
    assert!(
        !line.contains("regions 0,"),
        "[{label}] narration still reports regions 0: {line}"
    );
    assert!(
        line.contains(&format!("regions {regions},")),
        "[{label}] narration must report the item count it actually carries \
         ({regions}): {line}"
    );
    regions
}

// ── Scallop ──────────────────────────────────────────────────────────────

/// Scallop's regions are REAL: `scallop.rs` generates an independent ring
/// set per machining-boundary region and concatenates them in region order.
/// With no boundary set the whole mesh footprint is one region — `1`, which
/// is the truth, and distinguishable from the `0` that used to mean
/// "nothing looked".
#[test]
fn scallop_narration_reports_its_regions() {
    let mut session = single_op_session_with(
        stock_over(HALF, 6.0),
        ball_tool_config(3.0),
        mesh_model(sawtooth_plate(HALF, 4.0, 1.5), "sawtooth"),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -6.0),
    );
    common::session::generate(&mut session, 0);

    let regions = assert_regions_reported(&session, "scallop");
    assert_eq!(
        regions, 1,
        "one mesh footprint, no machining boundary — exactly one region"
    );

    // The rings must still be there, and must now be CHILDREN of the region
    // rather than siblings: a region that parents nothing is a label, not a
    // structure.
    let rings = count_kind(&session, &ToolpathSemanticKind::Ring);
    assert!(rings > 0, "scallop must emit rings");
    let trace = session
        .get_result(0)
        .unwrap()
        .semantic_trace
        .as_ref()
        .unwrap()
        .clone();
    let region_ids: Vec<_> = trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
        .map(|i| i.id)
        .collect();
    let parented = trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Ring)
        .filter(|i| i.parent_id.is_some_and(|p| region_ids.contains(&p)))
        .count();
    assert_eq!(
        parented, rings,
        "every scallop ring must hang off its own region node"
    );
}

// ── SpiralFinish ─────────────────────────────────────────────────────────

/// SpiralFinish has NO partition: one archimedean traversal, out-of-region
/// samples skipped. `1` is the honest count, and this arm exists partly to
/// pin that it is not silently copying scallop's per-boundary logic.
#[test]
fn spiral_finish_narration_reports_its_single_region() {
    let mut session = single_op_session_with(
        stock_over(HALF, 6.0),
        ball_tool_config(3.0),
        mesh_model(sawtooth_plate(HALF, 4.0, 1.5), "sawtooth"),
        "Spiral finish",
        OperationConfig::SpiralFinish(SpiralFinishConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -6.0),
    );
    common::session::generate(&mut session, 0);

    let regions = assert_regions_reported(&session, "spiral_finish");
    assert_eq!(
        regions, 1,
        "a spiral is ONE traversal — a per-boundary count here would report a \
         structure the generator does not have"
    );
    assert!(
        count_kind(&session, &ToolpathSemanticKind::Ring) > 0,
        "spiral finish must emit rings under its region"
    );
}

// ── Trace ────────────────────────────────────────────────────────────────

/// Trace's `regions 0` was a NAMING divergence: it routes the same
/// structural spans 15 other families send to `Region` items into `Chain`
/// items instead. The chains stay chains — and C8 adds the counter that
/// makes them visible, plus the one region the operation genuinely covers.
#[test]
fn trace_narration_reports_its_region_and_its_chains() {
    let square: Polygon2 = square_polygon(8.0);
    let mut session = single_op_session_with(
        stock_over(HALF, 6.0),
        endmill_tool_config(3.0),
        polygon_model(vec![square], "square"),
        "Trace",
        OperationConfig::Trace(TraceConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -2.0),
    );
    common::session::generate(&mut session, 0);

    let regions = assert_regions_reported(&session, "trace");
    assert_eq!(
        regions, 1,
        "trace engraves a contour set, it partitions nothing"
    );

    let chains = count_kind(&session, &ToolpathSemanticKind::Chain);
    assert!(chains > 0, "trace must emit chains");
    let line = structure_line(&session);
    assert!(
        line.contains(&format!("chains {chains}")),
        "the chain counter is the other half of this fix — without it a \
         40-contour engraving reads as no structure at all: {line}"
    );
}

/// The two `Region` populations must not merge. `narrate_toolpath` counts
/// semantic `Region` items with ONE counter, but the crate has two kinds of
/// region: the planner's territory nodes (A/M8, `UnifiedFinish`) and a
/// generator's own pass groupings (the 15 generic families, and now
/// scallop). Scallop is both, depending on who called it — and the first
/// cut of this change emitted its grouping unconditionally, which
/// double-counted inside `UnifiedFinish` and broke A/M8's 1:1
/// reconciliation gate outright (4 semantic regions vs 3 structural nodes,
/// with a stray `"Region 1/1 (scallop)"` among the band labels).
///
/// So a sub-generator emits rings FLAT. This pins that: a `UnifiedFinish`
/// whose mid-steep band runs scallop must publish only band/crease region
/// labels, never a scallop grouping.
#[test]
fn scallop_inside_unified_finish_does_not_add_a_second_region_population() {
    use rs_cam_core::compute::StockConfig;
    use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;

    // The M2.1 / Wave-D1 fixture, because it is the one that reliably
    // decomposes into several bands — a mid-steep groove is what puts
    // scallop under the planner in the first place.
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-6.0, 0.0),
        (-5.0, -1.732_050_8),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    let stock = StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut session = single_op_session_with(
        stock,
        common::tools::tapered_ball_tool_config(1.0, 7.0, 6.0),
        mesh_model(
            common::meshes::extrude_profile(&profile, -15.0, 15.0),
            "two_groove_plateau",
        ),
        "Unified finish",
        OperationConfig::UnifiedFinish(UnifiedFinishConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -9.0),
    );
    common::session::generate(&mut session, 0);

    let trace = session
        .get_result(0)
        .expect("generated result")
        .semantic_trace
        .as_ref()
        .expect("semantic trace")
        .clone();
    let labels: Vec<&str> = trace
        .items
        .iter()
        .filter(|i| i.kind == ToolpathSemanticKind::Region)
        .map(|i| i.label.as_str())
        .collect();
    println!("[unified_finish] region labels: {labels:?}");
    assert!(
        !labels.is_empty(),
        "A/M8 already closed regions-0 here; this arm must not be vacuous"
    );
    // A/M8's own labels legitimately END with a strategy — "MidSteep band
    // (scallop)" IS the planner naming the generator it routed to. What must
    // not appear is a SECOND population: the sub-generator's own
    // "Region N/M (…)" grouping.
    for label in &labels {
        assert!(
            !label.starts_with("Region "),
            "a sub-generator must not publish its own region population \
             alongside the planner's nodes: {labels:?}"
        );
    }
    // Non-vacuity: the planner really did route a mid-steep band to scallop
    // on this fixture, so the grouping had somewhere to appear.
    assert!(
        labels.iter().any(|l| l.contains("(scallop)")),
        "the fixture must exercise the scallop sub-generator: {labels:?}"
    );
}

/// The chain counter must not silently claim chains where there are none.
/// A scallop run emits rings, not chains, and its line must say `chains 0`
/// rather than omit the counter.
#[test]
fn the_chain_counter_reports_zero_where_there_are_no_chains() {
    let mut session = single_op_session_with(
        stock_over(HALF, 6.0),
        ball_tool_config(3.0),
        mesh_model(sawtooth_plate(HALF, 4.0, 1.5), "sawtooth"),
        "Scallop",
        OperationConfig::Scallop(ScallopConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -6.0),
    );
    common::session::generate(&mut session, 0);
    let line = structure_line(&session);
    assert!(
        line.contains("chains 0"),
        "the counter is always present, so a reader never has to guess \
         whether an absent number is zero or unmeasured: {line}"
    );
}
