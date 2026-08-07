//! M2.1 sentry — the END-TO-END tapered `UnifiedFinish` path.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds). Not a
//!   characterisation — every assertion below is a claim that must hold on
//!   any future implementation, not a snapshot of today's numbers.
//! * **Guards**: `32c5e48` (classification cell size follows the CUSP
//!   radius) and `5732f57` (finish-planner dials derive from the CUSP
//!   radius), against their parent `1748be2`. Both fixes are behaviourally
//!   INERT on ball cutters — `cusp_radius() == radius()` there — which is
//!   exactly why all 56 parameter sweeps and every pre-existing unit test
//!   missed the defect (`tapered_cusp_radius_sentry.rs` header, design doc
//!   §14t). `tapered_cusp_radius_sentry.rs` covers the pure helpers; this
//!   file covers the wiring they reach production through:
//!   `ProjectSession::generate_toolpath` → `session::compute` →
//!   `execute::generate_unified_finish` → annotation → spans → stats.
//! * **Variables held fixed**: one mesh, one set of planner thresholds
//!   (`steep 45 / waterline 75 / overlap 0`), one tolerance (0.05, below
//!   both tools' `cusp/4` so the cell size is cusp-DRIVEN and not
//!   tolerance-clamped), pinned `top_z` / `bottom_z`. The ONLY variable
//!   between the two runs is the tool's geometry.
//! * **Metric domains**: `area_mm2` on a region node is the planned band
//!   polygon's XY-PROJECTED area (`PlannedRegion::polygon.area()`), the
//!   same domain `FinishPlannerParams::min_region_area_mm2` is expressed
//!   in. It is NOT comparable to 3D mesh face area — the §14t audit finding
//!   — so no assertion here relates the two.
//! * **Fixture mutability**: the mesh is generated in-process from a
//!   profile literal. No Wanaka, no file, no `#[ignore]`.
//!
//! ## Why this can never pass vacuously
//!
//! Move counts are asserted non-zero BEFORE any structural property, and
//! the non-shallow assertions demand CUTTING moves (`MoveIntent::
//! FinishingCut`) inside a non-shallow node's own range — an empty
//! toolpath, an all-shallow decomposition, or a node that only retracts
//! all fail.
//!
//! ## Formerly-known gap, now GATED (task #14)
//!
//! Until `fix(diagnostics): remap semantic-trace move links through
//! post-generation transforms`, the semantic trace's move links were not
//! remapped by the post-span transforms in `session::compute`
//! (`apply_dressups`, the boundary clip and
//! `optimize_entry_descents_with_provenance` all remapped
//! `AnnotatedToolpath::spans` through a provenance map; none of them
//! touched `ToolpathSemanticTrace`). On this fixture the semantic ranges
//! tiled `0..1357` while the shipped toolpath had 1437 moves — A/M8's
//! range-equality invariant, pinned pre-transform by
//! `unified_finish_semantic_regions.rs`, did not survive to the shipped
//! result, and a semantic range could point PAST the end of the move list
//! (a consumer-panic class for anything that slices moves by region).
//!
//! Gate 4 now pins the post-transform form of that invariant, and Gate 2
//! states the semantic/structural agreement as exact range EQUALITY rather
//! than count-and-order.
//!
//! ## Why it would have been red before the fixes
//!
//! `would_have_been_red_the_prefix_dials_erase_every_steep_region`
//! reconstructs the pre-fix decomposition EXACTLY rather than arguing about
//! it: for our Ø1-tip/Ø6-shank taper the pre-fix code computed
//! `cell_size = radius()/4 = 0.75` and `pad = radius() = 3.0`, which is
//! bit-for-bit what `build_classification_surface_with_cancel` produces
//! TODAY for a Ø6 BALL (`cusp_radius() == radius() == 3.0`), and it
//! planned with `FinishPlannerParams::for_tool(3.0)`. Run that pair on this
//! mesh and every steep region is gone.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose_surface};
use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::semantic_trace::{ToolpathSemanticItem, ToolpathSemanticKind};
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::MoveIntent;
use rs_cam_core::toolpath_spans::{RegionSpanRole, SpanKind};

// ── Tool geometry ───────────────────────────────────────────────────────

/// The wanaka-class finisher: Ø1 TIP on a Ø6 SHANK, 7° half angle.
/// `radius()` reports 3.0 (the shank — the clearance-relevant width);
/// `cusp_radius()` reports 0.5 (the tip sphere — the feature scale). The
/// whole point of this file is that the second one drives band structure.
const TIP_DIAMETER_MM: f64 = 1.0;
const SHANK_DIAMETER_MM: f64 = 6.0;
const TAPER_HALF_ANGLE_DEG: f64 = 7.0;

/// Ball control: Ø3, where `cusp_radius() == radius() == 1.5` so both
/// fixes are identity transformations.
const BALL_DIAMETER_MM: f64 = 3.0;

fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: TIP_DIAMETER_MM,
        taper_half_angle: TAPER_HALF_ANGLE_DEG,
        shaft_diameter: SHANK_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose)
    }
}

fn ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: BALL_DIAMETER_MM,
        ..ToolConfig::new_default(ToolId(0), ToolType::BallNose)
    }
}

// ── Mesh fixture ────────────────────────────────────────────────────────

/// Extrude a Y-invariant cross-section profile into a mesh. Winding copied
/// from `tapered_cusp_radius_sentry::ribbon_mesh` (both faces point +Z).
fn extrude_profile(profile: &[(f64, f64)], y0: f64, y1: f64) -> TriangleMesh {
    let mut vertices = Vec::with_capacity(profile.len() * 2);
    for &(x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::with_capacity((profile.len() - 1) * 2);
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A 30 × 30 mm plateau cut by two grooves, sized to sit in the window the
/// two radii disagree about:
///
/// | feature            | width  | wall slope | XY area  |
/// |--------------------|--------|-----------|----------|
/// | plateau            | rest   | 0°        | Shallow  |
/// | mid-steep groove   | 2.0 mm | 60°       | ~60 mm²  |
/// | very-steep groove  | 1.5 mm | 85°       | ~45 mm²  |
///
/// Both grooves clear the TIP-derived `min_region_area_mm2` of
/// `(2·0.5)²·4 = 4 mm²` by an order of magnitude and sit far below the
/// SHANK-derived `(2·3.0)²·4 = 144 mm²`. They are also narrower than the
/// shank-derived `close_radius_mm` of 1.5 mm and only 2–3 shank-scale grid
/// cells (0.75 mm) wide, so the pre-fix pipeline could neither represent
/// nor keep them.
fn two_groove_plateau() -> TriangleMesh {
    // tan(60°) = 1.7320508, tan(85°) = 11.430052.
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
    extrude_profile(&profile, -15.0, 15.0)
}

// ── Session wiring ──────────────────────────────────────────────────────

fn mesh_model(mesh: TriangleMesh) -> LoadedModel {
    LoadedModel {
        id: 0,
        name: "two_groove_plateau".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://two_groove_plateau.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

/// Held fixed across both runs. `tolerance` matters: `cell_size =
/// max(cusp/4, tolerance)`, so a coarse tolerance would clamp BOTH tools to
/// the same cell and silently erase the very differential this file exists
/// to measure. 0.05 is below the taper's 0.125 and the ball's 0.375.
fn unified_finish_op() -> OperationConfig {
    OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        steep_threshold_deg: 45.0,
        waterline_threshold_deg: 75.0,
        overlap_mm: 0.0,
        tolerance: 0.05,
        sampling: 0.5,
        scallop_height: 0.15,
        raster_stepover: 1.5,
        z_step: 1.5,
        ..UnifiedFinishConfig::default()
    })
}

fn toolpath(op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: "Unified Finish".to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        // Pinned, not Auto: `bottom_z` Auto resolves to `top_z -
        // op_depth`, and a surface op carries no depth dial, so the
        // waterline band would get a zero-height Z range. Pinning is also
        // the "variables held fixed" contract.
        heights: HeightsConfig {
            top_z: HeightMode::Manual(0.0),
            bottom_z: HeightMode::Manual(-9.0),
            ..HeightsConfig::default()
        },
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn stock() -> StockConfig {
    StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The REAL production entry point — the one the GUI worker and the CLI
/// share. Not `execute_operation` directly, and emphatically not
/// `decompose_surface`.
fn generate_through_session(tool: ToolConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock());
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(two_groove_plateau()));
    session
        .add_toolpath(0, toolpath(unified_finish_op(), tool_id, model_id))
        .expect("add unified-finish toolpath");
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("UnifiedFinish must generate on the two-groove plateau");
    session
}

// ── Observations ────────────────────────────────────────────────────────

/// One routed region node as the SEMANTIC trace reports it: band label,
/// strategy label, XY-projected polygon area, and the move range —
/// converted here to the half-open convention the structural spans use
/// (semantic `move_end` is INCLUSIVE).
///
/// **`range` indexes the SHIPPED toolpath.** Since task #14 the post-span
/// transforms in `session::compute` carry the semantic trace's move links
/// through the same provenance maps they remap `AnnotatedToolpath::spans`
/// with, so a semantic range and its structural twin are the same range on
/// the same move list — Gate 2 asserts exactly that.
#[derive(Debug, Clone, PartialEq)]
struct SemanticNode {
    band: String,
    strategy: String,
    area_mm2: Option<f64>,
    range: (usize, usize),
}

fn semantic_nodes(session: &ProjectSession) -> Vec<SemanticNode> {
    let result = session.get_result(0).expect("generated result");
    let trace = result
        .semantic_trace
        .as_ref()
        .expect("session must attach a semantic trace");
    trace
        .items
        .iter()
        .filter(|item| item.kind == ToolpathSemanticKind::Region)
        .map(node_from_item)
        .collect()
}

fn node_from_item(item: &ToolpathSemanticItem) -> SemanticNode {
    // C4: typed keys. A typo in one of these literals used to yield an empty
    // string, i.e. a silently vacuous band/strategy assertion.
    let string_param = |key: rs_cam_core::semantic_trace::SemanticKey| {
        item.params
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    use rs_cam_core::semantic_trace::SemanticKey;
    SemanticNode {
        band: string_param(SemanticKey::Band),
        strategy: string_param(SemanticKey::Strategy),
        area_mm2: item
            .params
            .get(SemanticKey::AreaMm2)
            .and_then(|v| v.as_f64()),
        range: (
            item.move_start
                .expect("semantic region must be move-linked"),
            // INCLUSIVE → half-open, so it can be compared to a `Span`.
            item.move_end.expect("semantic region must be move-linked") + 1,
        ),
    }
}

/// The STRUCTURAL region-node spans, read off the FINAL annotated toolpath
/// — i.e. after every session-level transform (boundary clip, TSP reorder,
/// arc fit, feed optimisation) has had its chance to remap or invalidate
/// them. The ring spans `spans_from_labeled_events` also emits share
/// `SpanKind::Region`, so nodes are picked by their `RegionSpanRole::Node`
/// payload discriminator (Wave D3), exactly as
/// `unified_finish_semantic_regions.rs` does. Both used to filter on the
/// LABEL STRING; a label is free text and no producer was obliged to keep
/// it, so a rename would have emptied this list silently.
fn structural_nodes(session: &ProjectSession) -> Vec<(usize, usize, String)> {
    let result = session.get_result(0).expect("generated result");
    let mut nodes: Vec<(usize, usize, String)> = result
        .annotated()
        .spans
        .iter()
        .filter(|span| span.kind == SpanKind::Region && !span.is_boundary())
        .filter(|span| span.region_role() == Some(RegionSpanRole::Node))
        .map(|span| {
            (
                span.start_move,
                span.end_move,
                span.label.clone().into_owned(),
            )
        })
        .collect();
    nodes.sort_by_key(|(start, _, _)| *start);
    nodes
}

fn finishing_cuts_in(session: &ProjectSession, range: (usize, usize)) -> usize {
    let result = session.get_result(0).expect("generated result");
    result.toolpath().moves[range.0..range.1]
        .iter()
        .filter(|mv| mv.move_type.is_cutting() && mv.intent == MoveIntent::FinishingCut)
        .count()
}

/// The two region systems, paired by position. Both are built from
/// `UnifiedFinishReport::region_table` in stitch order, so position IS the
/// correspondence — see `unified_finish_semantic_regions.rs`, which pins
/// the pairing by move range on the pre-transform result.
fn paired_nodes(session: &ProjectSession) -> Vec<(SemanticNode, (usize, usize, String))> {
    let semantic = semantic_nodes(session);
    let structural = structural_nodes(session);
    assert_eq!(
        semantic.len(),
        structural.len(),
        "the semantic and structural region systems must emit the same \
         number of nodes — they read one table:\nsemantic {semantic:?}\n\
         structural {structural:?}"
    );
    for (sem, (_, _, label)) in semantic.iter().zip(structural.iter()) {
        let expected = match sem.band.as_str() {
            "Crease" => "Pencil claims".to_owned(),
            band => format!("{band} band"),
        };
        assert_eq!(
            *label, expected,
            "node order must agree between the two systems: semantic \
             {sem:?} vs structural label {label:?}"
        );
    }
    semantic.into_iter().zip(structural).collect()
}

fn describe(label: &str, session: &ProjectSession) -> Vec<SemanticNode> {
    let result = session.get_result(0).expect("generated result");
    let nodes = semantic_nodes(session);
    println!(
        "[{label}] {} moves, spans_valid {}, {} region nodes",
        result.toolpath().moves.len(),
        result.annotated().spans_valid,
        nodes.len()
    );
    for node in &nodes {
        println!(
            "[{label}]   {} ({}) area {:?} moves {:?} cuts {}",
            node.band,
            node.strategy,
            node.area_mm2.map(|a| (a * 10.0).round() / 10.0),
            node.range,
            finishing_cuts_in(session, node.range)
        );
    }
    println!("[{label}] structural {:?}", structural_nodes(session));
    let covered = covered_moves(session);
    let uncovered: Vec<(usize, String)> = result
        .toolpath()
        .moves
        .iter()
        .enumerate()
        .filter(|(i, _)| !covered[*i])
        .map(|(i, mv)| (i, format!("{:?}/{:?}", mv.move_type, mv.intent)))
        .collect();
    println!("[{label}] uncovered {}: {:?}", uncovered.len(), uncovered);
    nodes
}

/// Per-move flag: is this move inside some structural region node?
fn covered_moves(session: &ProjectSession) -> Vec<bool> {
    let result = session.get_result(0).expect("generated result");
    let mut covered = vec![false; result.toolpath().moves.len()];
    for (start, end, _) in structural_nodes(session) {
        for flag in covered.iter_mut().take(end).skip(start) {
            *flag = true;
        }
    }
    covered
}

// ── Gate 1: a non-shallow band actually cuts, end to end ────────────────

/// The M2.1 acceptance gate. Non-vacuity first: moves, then nodes, then
/// the property.
fn assert_a_non_shallow_band_cuts(session: &ProjectSession, label: &str) -> Vec<SemanticNode> {
    let result = session.get_result(0).expect("generated result");
    assert!(
        !result.toolpath().moves.is_empty(),
        "[{label}] the fixture must generate a toolpath — every assertion \
         below is vacuous on an empty one"
    );
    assert!(
        result.stats.move_count > 0,
        "[{label}] stats must agree that moves were emitted"
    );

    let nodes = describe(label, session);
    assert!(
        !nodes.is_empty(),
        "[{label}] UnifiedFinish must emit semantic region nodes"
    );

    let paired = paired_nodes(session);
    let non_shallow: Vec<&(SemanticNode, (usize, usize, String))> = paired
        .iter()
        .filter(|(n, _)| n.band == "MidSteep" || n.band == "VerySteep")
        .collect();
    assert!(
        !non_shallow.is_empty(),
        "[{label}] the two grooves must survive decomposition as \
         non-shallow bands; got only {:?}",
        nodes.iter().map(|n| &n.band).collect::<Vec<_>>()
    );

    for (sem, (start, end, _)) in &non_shallow {
        // Structural range, deliberately: it is the one remapped through
        // every post-generation transform.
        let cuts = finishing_cuts_in(session, (*start, *end));
        assert!(
            cuts > 0,
            "[{label}] the {} node ({}) spans moves {start}..{end} of the \
             SHIPPED toolpath but contains no `FinishingCut` — a routed \
             region that only retracts is not coverage",
            sem.band,
            sem.strategy
        );
    }

    // The strategy half of the label must match the band, or the mix table
    // H4 builds on this channel attributes time to the wrong generator.
    for node in &nodes {
        let expected = match node.band.as_str() {
            "Shallow" => "raster",
            "MidSteep" => "scallop",
            "VerySteep" => "waterline",
            "Crease" => "pencil",
            other => panic!("[{label}] unknown band label {other:?}"),
        };
        assert_eq!(
            node.strategy, expected,
            "[{label}] band {} must be generated by {expected}",
            node.band
        );
    }
    nodes
}

#[test]
fn tapered_unified_finish_cuts_non_shallow_bands_end_to_end() {
    let session = generate_through_session(tapered_ball_tool());
    let nodes = assert_a_non_shallow_band_cuts(&session, "tapered-ball");
    assert!(
        nodes.iter().any(|n| n.band == "VerySteep"),
        "the 85° groove must reach the VerySteep band on a Ø1 tip — that is \
         the band the shank-scale dials erased outright"
    );
}

#[test]
fn ball_unified_finish_cuts_non_shallow_bands_end_to_end() {
    let session = generate_through_session(ball_tool());
    assert_a_non_shallow_band_cuts(&session, "ball");
}

// ── Gate 2: spans and annotations survive the full pipeline ─────────────

/// `region_node_ranges_tile_the_stitched_toolpath` proves the tiling at the
/// generator's own output. This asserts the shipping form of the same
/// claim on the FINAL annotated toolpath — the object the dressup
/// stripper, the boundary clip, the TSP remap, the entry-descent splitter
/// and the exporter have all had their turn with.
///
/// The shipping form is deliberately stated as **cut attribution** rather
/// than exact tiling: `optimize_entry_descents_with_provenance` inserts
/// rapids AFTER the spans are built and remaps the spans around them, so
/// the final node ranges are separated by a handful of orphaned RAPIDS
/// (4 on this fixture). That is benign — the reorder that the tiling
/// invariant protects against has already run — but a per-region time or
/// length figure built on these spans silently drops those moves, so the
/// count is asserted small and their kind is asserted non-cutting.
///
/// What must never happen is a CUTTING move outside every node: that is
/// unattributed machining, and the mix table would under-report the
/// strategy that paid for it.
fn assert_spans_and_annotations_stay_valid(session: &ProjectSession, label: &str) {
    let result = session.get_result(0).expect("generated result");
    let move_count = result.toolpath().moves.len();
    assert!(move_count > 0, "[{label}] non-vacuity");
    assert!(
        result.annotated().spans_valid,
        "[{label}] a transform invalidated the spans — the region channel \
         is unusable when this is false"
    );

    let structural = structural_nodes(session);
    assert!(
        !structural.is_empty(),
        "[{label}] the final toolpath must still carry region-node spans"
    );
    assert_eq!(
        structural[0].0, 0,
        "[{label}] the first region node must open at move 0"
    );
    assert_eq!(
        structural[structural.len() - 1].1,
        move_count,
        "[{label}] the last region node must reach the end of the toolpath"
    );
    for pair in structural.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        assert!(
            a.1 <= b.0,
            "[{label}] region nodes must not OVERLAP: {:?} ends at {} but \
             {:?} starts at {} — a move attributed to two strategies makes \
             any per-region total double-count",
            a.2,
            a.1,
            b.2,
            b.0
        );
    }

    let covered = covered_moves(session);
    let orphans: Vec<usize> = (0..move_count).filter(|i| !covered[*i]).collect();
    assert!(
        orphans.len() * 20 < move_count,
        "[{label}] {} of {move_count} moves belong to no region node — the \
         span channel has stopped describing the toolpath",
        orphans.len()
    );
    for i in &orphans {
        let mv = &result.toolpath().moves[*i];
        assert!(
            !mv.move_type.is_cutting(),
            "[{label}] move {i} ({:?}/{:?}) cuts but belongs to no region \
             node — unattributed machining",
            mv.move_type,
            mv.intent
        );
    }

    // A/M8: the two region systems must not drift in COUNT, ORDER — or
    // RANGE. `unified_finish_semantic_regions.rs` pins the equality on the
    // generator's own output; task #14 made it survive the post-span
    // transforms, so it is restated here on the SHIPPED toolpath. (Before
    // that fix the semantic ranges tiled 0..1357 against 1437 shipped
    // moves and this loop was a lenient "tiles from 0, fits inside".)
    let paired = paired_nodes(session);
    let mut expected_start = 0usize;
    for (sem, (start, end, node_label)) in &paired {
        assert_eq!(
            sem.range,
            (*start, *end),
            "[{label}] the semantic range for {sem:?} must equal its \
             structural twin {node_label:?} {start}..{end} on the shipped \
             toolpath — the two systems read one region table, so any \
             difference is a transform that remapped one and not the other"
        );
        // Ordered and non-overlapping, NOT contiguous: the ranges inherit
        // the structural spans' gaps, which are the orphaned rapids the
        // entry-descent splitter inserts between nodes (budgeted and
        // asserted non-cutting above). Before task #14 this loop could
        // assert exact contiguity precisely BECAUSE the semantic links
        // still described the pre-splitter toolpath.
        assert!(
            sem.range.0 >= expected_start,
            "[{label}] semantic region ranges must not overlap: {sem:?} \
             starts before {expected_start}"
        );
        assert!(
            sem.range.1 > sem.range.0,
            "[{label}] a semantic region must span at least one move: {sem:?}"
        );
        expected_start = sem.range.1;
    }
    assert_eq!(
        expected_start, move_count,
        "[{label}] the last semantic region must reach the end of the \
         SHIPPED toolpath ({expected_start} vs {move_count} moves) — a \
         short tiling is the signature of links left in pre-transform \
         coordinates"
    );

    // Narration is the agent-facing end of the same channel.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains(&format!("regions {}", paired.len())),
        "[{label}] narration must report the region count:\n{narration}"
    );
}

#[test]
fn tapered_unified_finish_spans_and_annotations_stay_valid() {
    let session = generate_through_session(tapered_ball_tool());
    assert_spans_and_annotations_stay_valid(&session, "tapered-ball");
}

#[test]
fn ball_unified_finish_spans_and_annotations_stay_valid() {
    let session = generate_through_session(ball_tool());
    assert_spans_and_annotations_stay_valid(&session, "ball");
}

// ── Gate 4: semantic move links survive the transforms (task #14) ───────

/// EVERY item in the shipped semantic trace — not just the region nodes —
/// must name moves that exist.
///
/// `apply_dressups` (entry ramps, dogbones, leads, link moves, arc fit,
/// segment merge, the barriered and unbarriered TSP reorders, the air-cut
/// filter), the boundary clip and `optimize_entry_descents_with_provenance`
/// all insert, delete or permute moves AFTER the semantic items were bound
/// to their ranges. Every one of them remaps `AnnotatedToolpath::spans`
/// through a provenance map; until task #14 none of them touched the
/// semantic trace, so its links silently described a toolpath that no
/// longer existed — including ranges that ran past the end of the move
/// list, which panics any consumer that slices moves by semantic range
/// (GUI highlight, narration-by-region, per-region attribution).
///
/// Non-vacuity: the trace must contain linked items at all, and the
/// operation-level link must reach the LAST move of the shipped toolpath.
/// That last clause is the direct pin — pre-fix it read 1357 against 1437
/// shipped moves on this fixture.
fn assert_semantic_links_survive_transforms(session: &ProjectSession, label: &str) {
    let result = session.get_result(0).expect("generated result");
    let move_count = result.toolpath().moves.len();
    assert!(move_count > 0, "[{label}] non-vacuity");
    let trace = result
        .semantic_trace
        .as_ref()
        .expect("session must attach a semantic trace");

    let mut linked = 0usize;
    for item in &trace.items {
        match (item.move_start, item.move_end) {
            (Some(start), Some(end)) => {
                linked += 1;
                assert!(
                    end >= start,
                    "[{label}] semantic item {} ({:?} {:?}) has an inverted \
                     link {start}..={end}",
                    item.id,
                    item.kind,
                    item.label
                );
                assert!(
                    end < move_count,
                    "[{label}] semantic item {} ({:?} {:?}) is linked to \
                     moves {start}..={end} but the shipped toolpath has only \
                     {move_count} moves — slicing by this range reads out of \
                     bounds",
                    item.id,
                    item.kind,
                    item.label
                );
            }
            // The deleted-move policy: an item whose moves a transform
            // removed is UNLINKED, never half-linked and never clamped.
            (None, None) => {}
            half => panic!(
                "[{label}] semantic item {} ({:?} {:?}) is half-linked \
                 {half:?} — a link is both ends or neither",
                item.id, item.kind, item.label
            ),
        }
    }

    assert!(
        linked > 0,
        "[{label}] the shipped trace must still carry move-linked items — \
         unlinking everything would satisfy the bounds check vacuously"
    );
    assert_eq!(
        linked, trace.summary.move_linked_item_count,
        "[{label}] the summary must count the links the items actually carry"
    );

    let max_end = trace
        .items
        .iter()
        .filter_map(|item| item.move_end)
        .max()
        .expect("linked > 0 was just asserted");
    assert_eq!(
        max_end + 1,
        move_count,
        "[{label}] the outermost semantic item must still reach the last \
         move of the SHIPPED toolpath: links stop at {} of {move_count} \
         moves, so the transforms that inserted the remaining moves did not \
         carry the trace with them",
        max_end + 1
    );
}

#[test]
fn tapered_unified_finish_semantic_links_survive_transforms() {
    let session = generate_through_session(tapered_ball_tool());
    assert_semantic_links_survive_transforms(&session, "tapered-ball");
}

#[test]
fn ball_unified_finish_semantic_links_survive_transforms() {
    let session = generate_through_session(ball_tool());
    assert_semantic_links_survive_transforms(&session, "ball");
}

// ── Gate 3: the differential (M2.4 spirit) ──────────────────────────────

/// `min_region_area_mm2` as the SHANK radius would have produced it:
/// `(2·3.0)²·4`. Any node smaller than this is proof the planner dials came
/// from the tip.
const SHANK_MIN_REGION_AREA_MM2: f64 = 144.0;

/// The end-to-end pin: a routed non-shallow node whose XY-projected area is
/// below the floor the shank-derived dials impose. Such a node is not
/// merely different under the pre-fix code — it is UNREACHABLE, because
/// `absorb_small_regions` would have merged it away before extraction.
///
/// Domain note (§14t): `area_mm2` and `min_region_area_mm2` are both
/// XY-projected mm². Nothing here is compared against 3D face area.
#[test]
fn tapered_region_areas_prove_tip_scale_dials_end_to_end() {
    let session = generate_through_session(tapered_ball_tool());
    let nodes = describe("tapered-ball", &session);

    let tip_floor = (2.0 * 0.5_f64).powi(2) * 4.0;
    assert!(
        (tip_floor - 4.0).abs() < 1e-12,
        "the tip-derived floor is 4 mm²; this test's arithmetic assumes it"
    );

    let small: Vec<&SemanticNode> = nodes
        .iter()
        .filter(|n| n.band == "MidSteep" || n.band == "VerySteep")
        .filter(|n| n.area_mm2.is_some_and(|a| a < SHANK_MIN_REGION_AREA_MM2))
        .collect();
    assert!(
        !small.is_empty(),
        "at least one non-shallow node must be smaller than the shank-derived \
         min-region floor of {SHANK_MIN_REGION_AREA_MM2} mm² — otherwise this \
         fixture cannot tell tip-scale dials from shank-scale ones. Got {:?}",
        nodes
            .iter()
            .map(|n| (&n.band, n.area_mm2))
            .collect::<Vec<_>>()
    );
    for node in &small {
        assert!(
            node.area_mm2.is_some_and(|a| a > tip_floor),
            "[sanity] a routed node must clear its OWN floor: {node:?}"
        );
    }
}

/// The historical red. This reconstructs the pre-`32c5e48`/`5732f57`
/// pipeline exactly — see the module header — and asserts it loses every
/// steep region on the same mesh the end-to-end run above cuts.
///
/// The two halves are asserted separately so a future reader can see WHICH
/// of the two fixes each one guards.
#[test]
fn would_have_been_red_the_prefix_dials_erase_every_steep_region() {
    let mesh = two_groove_plateau();
    let index = SpatialIndex::build(&mesh, 10.0);
    let taper = TaperedBallEndmill::new(
        TIP_DIAMETER_MM,
        TAPER_HALF_ANGLE_DEG,
        SHANK_DIAMETER_MM,
        25.0,
    );
    let cancel = || false;
    let tolerance = 0.05;

    assert!((taper.radius() - 3.0).abs() < 1e-9);
    assert!((taper.cusp_radius() - 0.5).abs() < 1e-9);

    // Today: cell = cusp/4 = 0.125, dials from the tip.
    let live_surface =
        build_classification_surface_with_cancel(&mesh, &index, &taper, tolerance, &cancel)
            .expect("classification surface");
    assert!(
        (live_surface.cell_size() - 0.125).abs() < 1e-9,
        "cell size must follow the TIP; got {}",
        live_surface.cell_size()
    );
    let live = decompose_surface(
        &live_surface,
        &[],
        &FinishPlannerParams::for_tool(taper.cusp_radius()),
    );

    // Pre-fix: `cell = radius()/4 = 0.75`, `pad = radius() = 3.0`. A Ø6
    // BALL reproduces that grid bit-for-bit today, because its cusp radius
    // IS its radius — which is also why the fix is inert for balls.
    let prefix_probe = BallEndmill::new(SHANK_DIAMETER_MM, 25.0);
    assert!((prefix_probe.radius() - taper.radius()).abs() < 1e-12);
    let prefix_surface =
        build_classification_surface_with_cancel(&mesh, &index, &prefix_probe, tolerance, &cancel)
            .expect("pre-fix classification surface");
    assert!(
        (prefix_surface.cell_size() - 0.75).abs() < 1e-9,
        "the pre-fix reconstruction must use the SHANK cell; got {}",
        prefix_surface.cell_size()
    );
    let prefix = decompose_surface(
        &prefix_surface,
        &[],
        &FinishPlannerParams::for_tool(taper.radius()),
    );

    let count_non_shallow = |planned: &rs_cam_core::finish_planner::PlannedRegions| {
        planned
            .regions
            .iter()
            .filter(|r| r.band != FinishBand::Shallow)
            .count()
    };
    let (live_n, prefix_n) = (count_non_shallow(&live), count_non_shallow(&prefix));
    println!("tip-scale non-shallow regions {live_n}, shank-scale {prefix_n}");

    assert!(
        live_n > 0,
        "non-vacuity: the tip-scale pipeline must find the grooves"
    );
    assert_eq!(
        prefix_n, 0,
        "the pre-fix (shank-scale cell + shank-scale dials) pipeline must \
         lose every steep region on this fixture — if it does not, the \
         end-to-end gates above could pass against a revert"
    );
}

/// M2.4's "ball behavior unchanged" control, stated as an EQUALITY rather
/// than an inequality: on a ball both fixes are identity transformations,
/// so the pre-fix reconstruction and the live pipeline must produce
/// literally the same cell size, the same dials, and the same regions.
#[test]
fn ball_control_is_unaffected_by_both_radius_fixes() {
    let mesh = two_groove_plateau();
    let index = SpatialIndex::build(&mesh, 10.0);
    let ball = BallEndmill::new(BALL_DIAMETER_MM, 25.0);
    let cancel = || false;

    assert!(
        (ball.cusp_radius() - ball.radius()).abs() < 1e-12,
        "a ball's cusp radius IS its radius — the premise of this control"
    );

    let surface = build_classification_surface_with_cancel(&mesh, &index, &ball, 0.05, &cancel)
        .expect("classification surface");
    assert!(
        (surface.cell_size() - ball.radius() / 4.0).abs() < 1e-12,
        "post-fix `cusp/4` must equal pre-fix `radius/4` for a ball; got {}",
        surface.cell_size()
    );

    let by_cusp = decompose_surface(
        &surface,
        &[],
        &FinishPlannerParams::for_tool(ball.cusp_radius()),
    );
    let by_radius = decompose_surface(&surface, &[], &FinishPlannerParams::for_tool(ball.radius()));
    assert!(
        by_cusp
            .regions
            .iter()
            .any(|r| r.band != FinishBand::Shallow),
        "non-vacuity: the ball must find non-shallow regions too, or \
         'unchanged' is a statement about nothing"
    );
    assert_eq!(
        by_cusp.regions.len(),
        by_radius.regions.len(),
        "region count must be identical for a ball"
    );
    for (a, b) in by_cusp.regions.iter().zip(by_radius.regions.iter()) {
        assert_eq!(a.band, b.band, "band assignment must be identical");
        assert!(
            (a.polygon.area() - b.polygon.area()).abs() < 1e-9,
            "region area must be identical: {} vs {}",
            a.polygon.area(),
            b.polygon.area()
        );
    }
}
