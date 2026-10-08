//! Unit tests for the simulation state.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::*;
use crate::state::runtime::ToolpathRuntime;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3, V3};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::debug_trace::{ToolpathDebugBounds2, ToolpathDebugRecorder};
use rs_cam_core::trace::semantic_trace::{
    ToolpathSemanticKind, ToolpathSemanticRecorder, enrich_traces,
};
use std::sync::Arc;

const TEST_MAX_FEED: f64 = 3000.0;

fn gui_with_traces() -> GuiState {
    let mut gui = GuiState::new();
    let toolpath_id = rs_cam_core::ToolpathId(1);

    let semantic = ToolpathSemanticRecorder::new("Adaptive", "Adaptive");
    let root = semantic.root_context();
    let pass = root.start_item(ToolpathSemanticKind::Pass, "Pass 1");
    pass.set_move_range(0, 8);
    pass.set_xy_bbox(ToolpathDebugBounds2 {
        min_x: 0.0,
        max_x: 10.0,
        min_y: 0.0,
        max_y: 10.0,
    });
    pass.set_z_range(-1.0, -1.0);
    let entry_item = pass
        .context()
        .start_item(ToolpathSemanticKind::Entry, "Helix entry");
    entry_item.set_move_range(0, 2);
    entry_item.set_xy_bbox(ToolpathDebugBounds2 {
        min_x: 0.0,
        max_x: 4.0,
        min_y: 0.0,
        max_y: 4.0,
    });
    entry_item.set_z_range(-1.0, -1.0);
    entry_item.finish();
    let cleanup = pass
        .context()
        .start_item(ToolpathSemanticKind::Cleanup, "Cleanup");
    cleanup.set_move_range(6, 8);
    cleanup.set_xy_bbox(ToolpathDebugBounds2 {
        min_x: 6.0,
        max_x: 10.0,
        min_y: 6.0,
        max_y: 10.0,
    });
    cleanup.set_z_range(-1.0, -1.0);
    cleanup.finish();
    pass.finish();
    let mut semantic_trace = semantic.finish();

    let debug = ToolpathDebugRecorder::new("Adaptive", "Adaptive");
    let debug_ctx = debug.root_context();
    let pass_span = debug_ctx.start_span("adaptive_pass", "Pass 1");
    pass_span.set_move_range(0, 8);
    pass_span.set_xy_bbox(ToolpathDebugBounds2 {
        min_x: 0.0,
        max_x: 10.0,
        min_y: 0.0,
        max_y: 10.0,
    });
    pass_span.set_z_level(-1.0);
    let pass_span_id = pass_span.id();
    pass_span.finish();
    debug_ctx.add_annotation(1, "Entry");
    debug_ctx.add_annotation(7, "Cleanup");
    debug_ctx.record_hotspot(&rs_cam_core::trace::debug_trace::HotspotRecord {
        kind: "adaptive_pass".into(),
        center_x: 5.0,
        center_y: 5.0,
        z_level: Some(-1.0),
        bucket_size_xy: 10.0,
        bucket_size_z: Some(1.0),
        elapsed_us: 1_000,
        pass_count: 1,
        step_count: 8,
        low_yield_exit_count: 0,
    });
    if let Some(pass_item) = semantic_trace
        .items
        .iter_mut()
        .find(|item| item.label == "Pass 1")
    {
        pass_item.debug_span_id = Some(pass_span_id);
    }
    let mut debug_trace = debug.finish();
    enrich_traces(&mut debug_trace, &mut semantic_trace);
    let semantic_trace = Arc::new(semantic_trace);
    let debug_trace = Arc::new(debug_trace);
    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
    toolpath.feed_to(P3::new(0.0, 0.0, -1.0), 300.0);
    toolpath.feed_to(P3::new(2.0, 0.0, -1.0), 1000.0);
    toolpath.feed_to(P3::new(4.0, 0.0, -1.0), 1000.0);
    toolpath.rapid_to(P3::new(4.0, 0.0, 5.0));
    toolpath.rapid_to(P3::new(6.0, 6.0, 5.0));
    toolpath.feed_to(P3::new(6.0, 6.0, -1.0), 300.0);
    toolpath.feed_to(P3::new(8.0, 8.0, -1.0), 1000.0);
    toolpath.rapid_to(P3::new(8.0, 8.0, 5.0));
    let mut rt = ToolpathRuntime::new(true);
    rt.semantic_trace = Some(Arc::clone(&semantic_trace));
    rt.debug_trace = Some(Arc::clone(&debug_trace));
    rt.result = Some(crate::state::toolpath::ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            toolpath,
        )),
        stats: Default::default(),
        debug_trace: Some(debug_trace),
        semantic_trace: Some(semantic_trace),
        debug_trace_path: None,
        drill_op: None,
    });
    gui.toolpath_rt.insert(toolpath_id, rt);
    gui
}

fn simulation_for_toolpath() -> SimulationState {
    let mut sim = SimulationState::new();
    sim.results = Some(SimulationResults {
        mesh: std::sync::Arc::new(StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        }),
        total_moves: 9,
        boundaries: vec![ToolpathBoundary {
            id: ToolpathId(1),
            name: "Adaptive".to_owned(),
            tool_name: "6mm End Mill".to_owned(),
            start_move: 0,
            end_move: 8,
            direction: StockCutDirection::FromTop,
        }],
        setup_boundaries: vec![SetupBoundary {
            setup_id: SetupId(1),
            setup_name: "Setup 1".to_owned(),
            start_move: 0,
        }],
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        },
        cut_trace: None,
        cut_trace_path: None,
        column_grid_cell_mm: 0.5,
        prior_stocks: HashMap::new(),
    });
    sim
}

/// Attach a fresh cut trace, returning the `Arc` that was stored so cache
/// sentries can compare identities.
fn attach_cut_trace(sim: &mut SimulationState) -> Arc<SimulationCutTrace> {
    let trace = rs_cam_core::stock::simulation_cut::SimulationCutTrace::from_samples(
        0.5,
        vec![
            // The core constructor owns the neutral values, so a new
            // field on `SimulationCutSample` reaches these fixtures.
            // Only what this test measures is spelled out.
            rs_cam_core::stock::simulation_cut::SimulationCutSample {
                toolpath_id: rs_cam_core::ToolpathId(1),
                move_index: 1,
                position: [0.0, 0.0, -1.0],
                cumulative_time_s: 0.2,
                segment_time_s: 0.2,
                is_cutting: true,
                cut_kinematics: rs_cam_core::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 300.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.0,
                axial_engagement_mm: 1.0,
                arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
                chipload_mm_per_tooth: 0.0083,
                effective_chip_thickness_mm: Some(0.0083),
                engagement: rs_cam_core::stock::simulation_cut::Engagement::with_radial_woc(0.01),
                removed_volume_est_mm3: 0.1,
                mrr_mm3_s: 0.5,
                semantic_item_id: Some(2),
                ..rs_cam_core::stock::simulation_cut::SimulationCutSample::test_fixture()
            },
            rs_cam_core::stock::simulation_cut::SimulationCutSample {
                toolpath_id: rs_cam_core::ToolpathId(1),
                move_index: 7,
                sample_index: 1,
                position: [8.0, 8.0, -1.0],
                cumulative_time_s: 0.6,
                segment_time_s: 0.4,
                is_cutting: true,
                cut_kinematics: rs_cam_core::stock::simulation_cut::CutKinematics::Linear,
                feed_rate_mm_min: 1000.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 0.4,
                axial_engagement_mm: 0.4,
                arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
                chipload_mm_per_tooth: 0.0277,
                effective_chip_thickness_mm: Some(0.0277),
                engagement: rs_cam_core::stock::simulation_cut::Engagement::with_radial_woc(0.08),
                removed_volume_est_mm3: 2.0,
                mrr_mm3_s: 5.0,
                semantic_item_id: Some(3),
                ..rs_cam_core::stock::simulation_cut::SimulationCutSample::test_fixture()
            },
        ],
    );
    let trace = Arc::new(trace);
    if let Some(results) = sim.results.as_mut() {
        results.cut_trace = Some(Arc::clone(&trace));
    }
    trace
}

#[test]
fn the_issue_list_is_ordered_by_severity_not_by_move_index() {
    // D1 (census §3.5), ruled D-6. RED-FIRST SHAPE: the collision sits at
    // a LATER move than the air-cut run, so under the old ordering
    // (`move_index` primary, kind only as a tiebreak) it sorted BELOW the
    // per-run air-cut noise — and an operator stepping the list with
    // `focus_issue_delta` reached it after the noise, if at all.
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);
    sim.checks.rapid_collision_move_indices = vec![8];

    let issues = sim.issues(&gui, TEST_MAX_FEED);
    assert!(
        issues.len() >= 2,
        "fixture must produce a collision AND at least one air-cut run"
    );
    assert_eq!(
        issues[0].kind,
        SimulationIssueKind::RapidCollision,
        "the collision must sort first even though it is at the LAST move; \
         got {:?}",
        issues
            .iter()
            .map(|i| (i.kind, i.move_index))
            .collect::<Vec<_>>()
    );
    // And the air-cut tallies sort last, behind everything curated.
    assert_eq!(
        issues
            .last()
            .map(|i| i.kind)
            .expect("non-empty after the length assertion above"),
        SimulationIssueKind::AirCut
    );
}

#[test]
fn active_semantic_item_prefers_deepest_matching_item() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    sim.playback.current_move = 1;

    let active = sim
        .active_semantic_item(&gui)
        .expect("active semantic item");
    assert_eq!(active.item.label, "Helix entry");

    sim.playback.current_move = 7;
    let active = sim
        .active_semantic_item(&gui)
        .expect("active semantic item");
    assert_eq!(active.item.label, "Cleanup");
}

#[test]
fn current_debug_annotation_uses_local_toolpath_move() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    sim.playback.current_move = 7;

    let annotation = sim
        .current_debug_annotation(&gui)
        .expect("annotation for current move");
    assert_eq!(annotation.0, ToolpathId(1));
    assert_eq!(annotation.1.label, "Cleanup");
}

#[test]
fn pinned_semantic_item_overrides_playback_resolution() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    sim.playback.current_move = 7;
    sim.pin_semantic_item(ToolpathId(1), 2);

    let active = sim
        .active_semantic_item(&gui)
        .expect("pinned semantic item");
    assert_eq!(active.item.label, "Helix entry");

    sim.clear_pinned_semantic_item();
    let active = sim
        .active_semantic_item(&gui)
        .expect("playback semantic item");
    assert_eq!(active.item.label, "Cleanup");
}

#[test]
fn hotspot_target_resolves_move_and_semantic_item() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();

    let target = sim
        .trace_target_for_hotspot(&gui, ToolpathId(1), 0)
        .expect("hotspot target");
    assert_eq!(target.toolpath_id, ToolpathId(1));
    assert_eq!(target.move_index, 0);
    assert!(target.semantic_item_id.is_some());
    assert!(target.debug_span_id.is_some());
}

#[test]
fn issue_navigation_prioritizes_hotspots_then_annotations() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();

    let first = sim
        .focus_issue_delta(&gui, TEST_MAX_FEED, 1)
        .expect("first issue target");
    assert_eq!(first.move_index, 0);
    assert_eq!(
        sim.current_issue(&gui, TEST_MAX_FEED)
            .expect("focused issue")
            .kind,
        SimulationIssueKind::Hotspot
    );

    let second = sim
        .focus_issue_delta(&gui, TEST_MAX_FEED, 1)
        .expect("second issue target");
    assert_eq!(second.move_index, 1);
    assert_eq!(
        sim.current_issue(&gui, TEST_MAX_FEED)
            .expect("focused issue")
            .kind,
        SimulationIssueKind::Annotation
    );
}

#[test]
fn semantic_pick_prefers_deeper_item_then_smaller_move_span() {
    let gui = gui_with_traces();
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();

    let target = sim
        .pick_semantic_item_with_ray(
            &gui,
            &session,
            &P3::new(2.0, 2.0, 10.0),
            &V3::new(0.0, 0.0, -1.0),
        )
        .expect("semantic pick target");
    assert_eq!(target.toolpath_id, ToolpathId(1));
    assert_eq!(target.semantic_item_id, Some(2));
    assert_eq!(target.move_index, 0);
}

#[test]
fn cut_trace_surfaces_cutting_issues() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);
    sim.playback.current_move = 7;

    let issues = sim.issues(&gui, TEST_MAX_FEED);
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == SimulationIssueKind::AirCut)
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == SimulationIssueKind::LowEngagement)
    );
}

// ── Cache-key soundness (RESEARCH_f2_and_aba.md, Topic B) ─────────────

/// Sentinel poked into the cached triage. A cache **hit** returns the
/// same object and carries it out; a **rebuild** replaces `triage`
/// wholesale and wipes it. This is the hit/miss witness these sentries
/// use — the triage's own content cannot serve, because two different
/// traces may legitimately triage identically.
const POISON: usize = usize::MAX;

fn poison(sim: &mut SimulationState) {
    sim.debug.triage_cache.triage.counts.samples_total = POISON;
}

//
// The four viz caches below used to key on `Arc::as_ptr(trace) as usize`
// — three of them paired with the GUI edit counter, `SpanAggregateCache`
// with nothing at all. Neither component closes the ABA window: the
// reachable gesture is `invalidate_simulation` (`controller::events::
// simulation`), which sets `results = None` — freeing the trace with
// nothing replacing it — and bumps no counter and clears no cache.
// Every `ArcInner<SimulationCutTrace>` is the same fixed size, so a
// re-simulate after that free is a same-size-class allocation and
// same-address reuse is likely rather than unlikely.

/// The property that makes the address question moot: while a cache holds
/// a `Weak`, the freed allocation stays reserved, so no replacement can
/// be handed that address and a stale key cannot false-hit.
///
/// The `assert_ne!` is the load-bearing half — it is exactly the
/// comparison the old `usize` key performed, and it is guaranteed here
/// only *because* the `Weak` is still alive.
#[test]
fn a_weak_key_pins_the_freed_address_so_it_cannot_false_hit() {
    let mut sim = simulation_for_toolpath();
    let trace = attach_cut_trace(&mut sim);
    let freed_addr = Arc::as_ptr(&trace) as usize;
    let stored: Weak<SimulationCutTrace> = Arc::downgrade(&trace);
    drop(trace);
    sim.results = None; // the `invalidate_simulation` gesture

    assert!(
        stored.upgrade().is_none(),
        "fixture must actually drop the trace"
    );
    let mut replacements: Vec<Arc<SimulationCutTrace>> = Vec::new();
    for _ in 0..64 {
        let mut next = simulation_for_toolpath();
        let replacement = attach_cut_trace(&mut next);
        assert_ne!(
            Arc::as_ptr(&replacement) as usize,
            freed_addr,
            "a live Weak must reserve the freed allocation; the old \
             `Arc::as_ptr as usize` key had no such guarantee"
        );
        assert!(
            !weak_matches(Some(&stored), Some(&replacement)),
            "a dead Weak must never match a live Arc"
        );
        replacements.push(replacement);
    }
    // …and the arm that must survive: "no trace" is a cached state.
    assert!(weak_matches(None::<&Weak<SimulationCutTrace>>, None));
    assert!(!weak_matches(None, replacements.first()));
}

/// The ABA scenario end to end: cache the triage, invalidate the
/// simulation (no edit-counter bump, no cache clear), re-simulate, and
/// ask again. The cache must miss.
///
/// Witness of the miss is the build count and the poisoned value: a hit
/// keeps both.
#[test]
fn invalidate_then_resimulate_misses_the_triage_cache() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    let first_addr = Arc::as_ptr(&first) as usize;
    drop(first); // only `sim.results` holds the trace, as in the GUI

    let _ = sim.cached_simulation_triage(&session);
    assert_eq!(sim.debug.triage_cache.builds, 1);

    // A second ask at the same version is a hit — the cache still earns
    // its keep after the key change. Witnessed by a sentinel poked into
    // the cached value: a rebuild replaces the whole `triage`.
    poison(&mut sim);
    assert_eq!(
        sim.cached_simulation_triage(&session).counts.samples_total,
        POISON,
        "an unchanged project must still hit the cache"
    );

    // `invalidate_simulation`: results dropped, counter untouched.
    sim.results = None;
    let mut refreshed = simulation_for_toolpath();
    let second = attach_cut_trace(&mut refreshed);
    assert_ne!(
        Arc::as_ptr(&second) as usize,
        first_addr,
        "the cache's Weak reserves the freed address"
    );
    sim.results = refreshed.results.take();

    assert_ne!(
        sim.cached_simulation_triage(&session).counts.samples_total,
        POISON,
        "the invalidate → re-simulate gesture must MISS the cache"
    );
    assert_eq!(
        sim.debug.triage_cache.builds, 2,
        "the triage must have been rebuilt against the new trace"
    );
    assert!(
        Arc::strong_count(&second) > 1,
        "the run holds the new trace"
    );
}

/// §B.3 — the larger hole, and it is not ABA: the triage is built from
/// evidence that is not the trace, and the holder-collision report
/// arrives from a *separate async job* with the same trace and no cache
/// invalidation. Each arm below moves one
/// `project_evidence` input and must invalidate.
#[test]
fn evidence_movement_invalidates_the_cached_triage() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);

    let _ = sim.cached_simulation_triage(&session);
    let baseline_fp = sim.debug.triage_cache.evidence_fp;
    poison(&mut sim);
    assert_eq!(
        sim.cached_simulation_triage(&session).counts.samples_total,
        POISON,
        "an unchanged project must not rebuild"
    );

    // (a) the async holder-collision report lands.
    sim.checks.collision_report = Some(CollisionReport {
        collisions: vec![rs_cam_core::stock::collision::CollisionEvent {
            move_index: 4,
            position: P3::new(1.0, 1.0, -1.0),
            penetration_depth: 0.8,
            segment: rs_cam_core::stock::collision::AssemblySegment::Holder,
            kind: rs_cam_core::stock::collision::CollisionKind::Workpiece,
        }],
        min_safe_stickout: 42.0,
    });
    sim.checks.checked_scope.toolpath_id = Some(ToolpathId(1));
    sim.checks.holder_collision_count = 1;
    assert_ne!(
        sim.cached_simulation_triage(&session).counts.samples_total,
        POISON,
        "the holder report must force a rebuild"
    );
    let after_holder = sim.debug.triage_cache.evidence_fp;
    assert_ne!(
        after_holder, baseline_fp,
        "a holder report arriving must invalidate the triage — it feeds \
         `ProjectEvidence::holder_collisions` and moves no other key part"
    );

    // (b) a rapid-through-stock collision list change.
    sim.checks.rapid_collisions = vec![RapidCollision {
        move_index: 5,
        start: P3::new(0.0, 0.0, 5.0),
        end: P3::new(9.0, 9.0, 5.0),
    }];
    sim.checks.rapid_collision_move_indices = vec![5];
    let _ = sim.cached_simulation_triage(&session);
    let after_rapids = sim.debug.triage_cache.evidence_fp;
    assert_ne!(after_rapids, after_holder, "rapid collisions must be keyed");

    // (c) the simulated cell size, which enriches a measurability
    // abstention's reason — the field today's sole consumer renders.
    if let Some(results) = sim.results.as_mut() {
        results.column_grid_cell_mm *= 2.0;
    }
    let _ = sim.cached_simulation_triage(&session);
    assert_ne!(
        sim.debug.triage_cache.evidence_fp, after_rapids,
        "resolution_mm must be keyed"
    );
}

/// The sibling caches carry the same key shape, and `SpanAggregateCache`
/// carried the strictest form of the bug (bare pointer, no counter).
/// Rebuilding it against a *different* trace after the first was freed
/// must produce the new trace's aggregates, not the old ones.
#[test]
fn span_aggregate_cache_rebuilds_after_its_trace_is_freed() {
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    sim.debug.span_aggregates.ensure_built(&first);
    assert_eq!(
        sim.debug
            .span_aggregates
            .cutting_indices_for(ToolpathId(1))
            .len(),
        2,
        "fixture must have cutting samples to make the sentry non-vacuous"
    );
    drop(first);
    sim.results = None;

    // A shorter replacement: a false hit would keep the two-sample
    // grouping of the freed trace.
    let mut refreshed = simulation_for_toolpath();
    let full = attach_cut_trace(&mut refreshed);
    let short = Arc::new(
        rs_cam_core::stock::simulation_cut::SimulationCutTrace::from_samples(
            0.5,
            full.samples.iter().take(1).cloned().collect(),
        ),
    );
    sim.debug.span_aggregates.ensure_built(&short);
    assert_eq!(
        sim.debug
            .span_aggregates
            .cutting_indices_for(ToolpathId(1))
            .len(),
        1,
        "the cache must reflect the trace it was last given"
    );
}

/// The load-report cache: a hit must still be a hit (it is a per-frame
/// path), and it must miss once its trace is replaced. The load report
/// reads the session memo, so its builds are counted on the session.
#[test]
fn load_report_cache_hits_then_misses_on_a_new_trace() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    drop(first);

    let _ = sim.cached_load_report(&session);
    let _ = sim.cached_load_report(&session);
    assert_eq!(
        session.tool_load_report_builds(),
        1,
        "an unchanged frame must not rebuild the load report"
    );

    sim.results = None;
    let mut refreshed = simulation_for_toolpath();
    attach_cut_trace(&mut refreshed);
    sim.results = refreshed.results.take();

    let _ = sim.cached_load_report(&session);
    assert_eq!(
        session.tool_load_report_builds(),
        2,
        "a new trace must rebuild the load report"
    );
}

// ── G-CACHEKEYS: the caches key on their inputs, not on a counter ─────
//
// The defect: the triage and cut-metric caches keyed on the trace and the
// GUI edit counter. A regenerate adopts a new result with no edit and no
// new simulation, so neither part moved and the caches kept the old
// answer. Each sentry below adopts a result that way and asserts one
// rebuild, then no rebuild on a repeat call, then the cached value equals
// a fresh computation.

/// A toolpath with vertical fed descents above the pocket's 500 mm/min
/// plunge rate, so the triage has a plunge-class finding from the
/// kinematic rows (the rows the triage reads from the load report).
fn plunging_result(feed: f64) -> rs_cam_core::session::ToolpathComputeResult {
    let mut tp = Toolpath::new();
    for hole in 0..4 {
        let x = f64::from(hole) * 2.0;
        tp.rapid_to(P3::new(x, 0.0, 5.0));
        tp.feed_to(P3::new(x, 0.0, -1.0), feed);
        tp.feed_to(P3::new(x + 1.0, 1.0, -1.0), feed);
        tp.rapid_to(P3::new(x + 1.0, 1.0, 5.0));
    }
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Adopt `result` for `index` through the command door, as a worker
/// completion does. It moves no GUI edit counter and no trace.
fn adopt(
    session: &mut rs_cam_core::session::ProjectSession,
    index: usize,
    result: rs_cam_core::session::ToolpathComputeResult,
) {
    use rs_cam_core::session::{AdoptResultArgs, Command};
    let revision = session.toolpath_revision(index);
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision,
            result: Box::new(result),
        }))
        .expect("the revision is current");
}

/// Two pocket rows, so the second has `ToolpathId(1)`: the id the trace
/// fixture and the simulation boundary name. The second row carries a
/// result.
fn session_for_toolpath_one() -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::session::{ProjectSessionBuilder, ToolpathConfig};
    let mut builder =
        ProjectSessionBuilder::new().tool(ToolConfig::new_default(ToolId(1), ToolType::EndMill));
    for name in ["Unused", "Adaptive"] {
        let config = ToolpathConfig {
            id: rs_cam_core::ToolpathId(0),
            name: name.to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(Default::default()),
            dressups: Default::default(),
            heights: Default::default(),
            tool_id: 1,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: Default::default(),
            boundary_inherit: true,
            stock_source: Default::default(),
            coolant: Default::default(),
            face_selection: None,
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            rest_analysis: Default::default(),
            planner_origin: None,
        };
        let _ = builder.add_toolpath(0, config).expect("setup 0 exists");
    }
    let mut session = builder.build();
    assert_eq!(
        session.toolpath_configs()[1].id,
        rs_cam_core::ToolpathId(1),
        "the fixture trace names toolpath 1"
    );
    adopt(&mut session, 1, plunging_result(1200.0));
    session
}

#[test]
fn a_result_adoption_rebuilds_the_triage_once_g_cachekeys() {
    let mut session = session_for_toolpath_one();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);

    let _ = sim.cached_simulation_triage(&session);
    assert_eq!(sim.debug.triage_cache.builds, 1);
    let _ = sim.cached_simulation_triage(&session);
    assert_eq!(
        sim.debug.triage_cache.builds, 1,
        "an unchanged call rebuilt"
    );

    // The old defect: a regenerate with no edit and no new simulation.
    adopt(&mut session, 1, plunging_result(1500.0));
    let _ = sim.cached_simulation_triage(&session);
    assert_eq!(
        sim.debug.triage_cache.builds, 2,
        "a result adoption was missed"
    );
    let cached = format!("{:?}", sim.cached_simulation_triage(&session));
    assert_eq!(sim.debug.triage_cache.builds, 2, "a repeat call rebuilt");

    let fresh = session.simulation_triage(&sim.project_evidence());
    assert!(
        fresh
            .actions
            .iter()
            .any(|finding| finding.diagnostic.id.as_str()
                == rs_cam_core::diagnostics::ids::PROJECT_PLUNGE_CLASS_LOAD),
        "the fixture must give a plunge-class finding, or the equality is vacuous"
    );
    assert_eq!(
        cached,
        format!("{fresh:?}"),
        "the cache differs from a fresh triage"
    );
}

#[test]
fn a_result_adoption_rebuilds_the_cut_metrics_once_g_cachekeys() {
    let mut session = session_for_toolpath_one();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);
    let id = rs_cam_core::ToolpathId(1);

    let first = sim.cached_cut_metrics(&session, id);
    assert!(
        !first.cards.is_empty(),
        "no cards: the check would be vacuous"
    );
    assert_eq!(sim.debug.cut_metric_cache.builds, 1);
    let again = sim.cached_cut_metrics(&session, id);
    assert!(Arc::ptr_eq(&first, &again), "an unchanged call rebuilt");
    assert_eq!(sim.debug.cut_metric_cache.builds, 1);

    adopt(&mut session, 1, plunging_result(1500.0));
    let after = sim.cached_cut_metrics(&session, id);
    assert_eq!(
        sim.debug.cut_metric_cache.builds, 2,
        "a result adoption was missed"
    );
    let repeat = sim.cached_cut_metrics(&session, id);
    assert!(Arc::ptr_eq(&after, &repeat));
    assert_eq!(
        sim.debug.cut_metric_cache.builds, 2,
        "a repeat call rebuilt"
    );

    // A fresh computation: the same builder with no memo entry.
    sim.debug.cut_metric_cache = super::CutMetricCache::default();
    let fresh = sim.cached_cut_metrics(&session, id);
    assert_eq!(format!("{:?}", *after), format!("{:?}", *fresh));
}

// The issue list keyed on `Arc::as_ptr` of the cut trace and of each debug
// and semantic trace, plus `GuiState::edit_counter`. The list reads no
// session state, so the counter only rebuilt it for no reason, and the bare
// pointers had the ABA hazard. The key now holds a `Weak` to each trace.

/// An `edit_counter` bump alone moves no input of the issue list, so the
/// list is a hit: the same shared `Arc`.
#[test]
fn an_edit_counter_bump_alone_hits_the_issue_list_g_cachekeys() {
    let mut gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    let _trace = attach_cut_trace(&mut sim);
    let before = sim.issues(&gui, TEST_MAX_FEED);
    assert!(!before.is_empty(), "fixture must produce issues");
    gui.mark_edited();
    gui.mark_edited();
    let after = sim.issues(&gui, TEST_MAX_FEED);
    assert!(
        Arc::ptr_eq(&before, &after),
        "the issue list reads no session state; a counter bump must hit"
    );
}

/// The ABA gesture on the cut trace: the run drops its trace, and a new
/// trace is allocated. The held `Weak` reserves the old address, and the
/// key misses.
#[test]
fn a_new_cut_trace_after_a_drop_misses_the_issue_list_g_cachekeys() {
    let gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    let first_addr = Arc::as_ptr(&first) as usize;
    drop(first); // only `sim.results` holds the trace, as in the GUI
    let before = sim.issues(&gui, TEST_MAX_FEED);

    // Drop the trace, then allocate replacements of the same size class.
    if let Some(results) = sim.results.as_mut() {
        results.cut_trace = None;
    }
    let mut held = Vec::new();
    for _ in 0..16 {
        let mut next = simulation_for_toolpath();
        let replacement = attach_cut_trace(&mut next);
        assert_ne!(
            Arc::as_ptr(&replacement) as usize,
            first_addr,
            "the cache's Weak reserves the freed address"
        );
        held.push(replacement);
    }
    let mut refreshed = simulation_for_toolpath();
    let _second = attach_cut_trace(&mut refreshed);
    if let (Some(results), Some(fresh)) = (sim.results.as_mut(), refreshed.results.take()) {
        results.cut_trace = fresh.cut_trace;
    }

    let after = sim.issues(&gui, TEST_MAX_FEED);
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a new cut trace must miss the issue list, whatever its address"
    );
}

/// The ABA gesture on a debug trace: an equal trace in a new allocation is
/// a new input, and the key misses. The old key paired the pointer with
/// the annotation and hotspot counts, which an equal trace repeats.
#[test]
fn a_new_debug_trace_after_a_drop_misses_the_issue_list_g_cachekeys() {
    let mut gui = gui_with_traces();
    let mut sim = simulation_for_toolpath();
    let _trace = attach_cut_trace(&mut sim);
    let id = rs_cam_core::ToolpathId(1);
    let before = sim.issues(&gui, TEST_MAX_FEED);

    let rt = gui.toolpath_rt.get_mut(&id).expect("fixture toolpath");
    let old = rt.debug_trace.take().expect("fixture debug trace");
    if let Some(result) = rt.result.as_mut() {
        result.debug_trace = None;
    }
    let copy = (*old).clone();
    let old_addr = Arc::as_ptr(&old) as usize;
    drop(old);
    let replacement = Arc::new(copy);
    assert_ne!(
        Arc::as_ptr(&replacement) as usize,
        old_addr,
        "the cache's Weak reserves the freed address"
    );
    rt.debug_trace = Some(replacement);

    let after = sim.issues(&gui, TEST_MAX_FEED);
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a new debug trace must miss the issue list"
    );
    assert_eq!(
        before.len(),
        after.len(),
        "an equal trace gives an equal list"
    );
    let again = sim.issues(&gui, TEST_MAX_FEED);
    assert!(Arc::ptr_eq(&after, &again), "the new key then holds");
}

// ── Render review 2026-10-09, P1 (F4 and F6) ─────────────────────────────

/// Two toolpaths back to back: A owns global moves 0..100, B 100..150.
fn simulation_with_two_toolpaths() -> SimulationState {
    let mut sim = simulation_for_toolpath();
    let results = sim.results.as_mut().unwrap();
    results.total_moves = 150;
    let a = results.boundaries[0].clone();
    results.boundaries = vec![
        ToolpathBoundary {
            id: ToolpathId(1),
            start_move: 0,
            end_move: 100,
            ..a.clone()
        },
        ToolpathBoundary {
            id: ToolpathId(2),
            start_move: 100,
            end_move: 150,
            ..a
        },
    ];
    sim
}

/// F6: each toolpath counts its own moves from the global cursor. The old
/// draw used the global cursor (50) as B's local limit, so B showed its
/// first 50 moves while the playhead was still inside A.
#[test]
fn each_toolpath_is_trimmed_by_its_own_move_range_f6() {
    let sim = simulation_with_two_toolpaths();
    let at = |cursor: usize| {
        let limits = sim.toolpath_move_limits(cursor);
        (limits[&ToolpathId(1)], limits[&ToolpathId(2)])
    };
    assert_eq!(at(0), (0, 0));
    assert_eq!(
        at(50),
        (50, 0),
        "B must draw nothing while the playhead is in A"
    );
    assert_eq!(at(100), (100, 0));
    assert_eq!(at(120), (100, 20), "B counts from its own first move");
    assert_eq!(at(150), (100, 50));
    assert_eq!(
        at(10_000),
        (100, 50),
        "the limit never passes the toolpath's end"
    );
}

fn mesh_with_vertices(n: usize) -> StockMesh {
    StockMesh {
        vertices: vec![0.0; n * 3],
        indices: Vec::new(),
        colors: vec![0.5; n * 3],
    }
}

/// F4: the deviations fit the shown mesh only when it is the final
/// composite mesh and has one vertex per value. A mesh with the same vertex
/// count that is not the final one (the uncut block at move 0, a preview, a
/// checkpoint) gets no deviation colours.
#[test]
fn deviation_colours_go_only_on_the_final_mesh_f4() {
    use crate::render::sim_render::deviation_colors_for_mesh;

    let devs = [0.0_f32, 0.5, -0.7];
    assert!(
        deviation_colors_for_mesh(Some(&devs), true, 3).is_some(),
        "the control: the final mesh with one vertex per value takes the colours"
    );
    assert!(
        deviation_colors_for_mesh(Some(&devs), false, 3).is_none(),
        "a mesh that is not the final one abstains, also with the same count"
    );
    assert!(
        deviation_colors_for_mesh(Some(&devs), true, 4).is_none(),
        "a vertex count that differs abstains"
    );
    assert!(deviation_colors_for_mesh(None, true, 3).is_none());

    let mut sim = simulation_for_toolpath();
    sim.playback.display_deviations = Some(devs.to_vec());
    sim.playback.display_mesh = Some(mesh_with_vertices(3));
    sim.playback.display_mesh_is_final = false;
    assert!(
        !sim.playback.deviations_fit_display(),
        "a live mesh with the same vertex count is not the deviations' mesh"
    );
    sim.playback.display_mesh_is_final = true;
    assert!(sim.playback.deviations_fit_display());
    sim.playback.display_mesh = Some(mesh_with_vertices(4));
    assert!(!sim.playback.deviations_fit_display());
}

/// F4: right after a run the playhead is at move 0 and the view shows the
/// uncut block, not the final mesh. Only the Deviation mode, paused at the
/// end, asks for the final mesh.
#[test]
fn only_a_paused_deviation_view_at_the_end_asks_for_the_final_mesh_f4() {
    let mut sim = simulation_for_toolpath();
    sim.playback.display_deviations = Some(vec![0.0; 3]);
    sim.stock_viz_mode = StockVizMode::Deviation;
    sim.playback.current_move = 0;
    assert!(!sim.wants_final_mesh(), "move 0 shows the uncut block");
    sim.playback.current_move = sim.total_moves();
    assert!(
        sim.wants_final_mesh(),
        "paused at the end in Deviation mode"
    );
    sim.playback.playing = true;
    assert!(!sim.wants_final_mesh(), "playing shows the live stock");
    sim.playback.playing = false;
    sim.stock_viz_mode = StockVizMode::Solid;
    assert!(
        !sim.wants_final_mesh(),
        "the Solid mode shows the live stock"
    );
    sim.stock_viz_mode = StockVizMode::Deviation;
    sim.playback.display_deviations = None;
    assert!(!sim.wants_final_mesh(), "no deviations, no reason to swap");
}
