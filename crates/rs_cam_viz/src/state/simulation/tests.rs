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
        mesh: StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
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
/// ask again at the *same* edit counter. The cache must miss.
///
/// Witness of the miss is the stored key itself: `trace` is written only
/// on a rebuild, so a hit would have left the dead `Weak` in place.
#[test]
fn invalidate_then_resimulate_misses_the_triage_cache() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    let first_addr = Arc::as_ptr(&first) as usize;
    drop(first); // only `sim.results` holds the trace, as in the GUI

    let _ = sim.cached_simulation_triage(&session, 7);
    assert!(sim.debug.triage_cache.built);

    // A second ask at the same version is a hit — the cache still earns
    // its keep after the key change. Witnessed by a sentinel poked into
    // the cached value: a rebuild replaces the whole `triage`.
    poison(&mut sim);
    assert_eq!(
        sim.cached_simulation_triage(&session, 7)
            .counts
            .samples_total,
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
        sim.cached_simulation_triage(&session, 7)
            .counts
            .samples_total,
        POISON,
        "the invalidate → re-simulate gesture must MISS the cache"
    );
    let stored_is_second = sim
        .debug
        .triage_cache
        .trace
        .as_ref()
        .and_then(|w| w.upgrade())
        .is_some_and(|up| Arc::ptr_eq(&up, &second));
    assert!(
        stored_is_second,
        "the triage must have been rebuilt against the new trace"
    );
}

/// §B.3 — the larger hole, and it is not ABA: the triage is built from
/// evidence that is not the trace, and the holder-collision report
/// arrives from a *separate async job* with the same trace, the same edit
/// counter and no cache invalidation. Each arm below moves one
/// `project_evidence` input and must invalidate.
#[test]
fn evidence_movement_invalidates_the_cached_triage() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    attach_cut_trace(&mut sim);

    let _ = sim.cached_simulation_triage(&session, 3);
    let baseline_fp = sim.debug.triage_cache.evidence_fp;
    poison(&mut sim);
    assert_eq!(
        sim.cached_simulation_triage(&session, 3)
            .counts
            .samples_total,
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
    sim.checks.holder_collision_count = 1;
    assert_ne!(
        sim.cached_simulation_triage(&session, 3)
            .counts
            .samples_total,
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
    let _ = sim.cached_simulation_triage(&session, 3);
    let after_rapids = sim.debug.triage_cache.evidence_fp;
    assert_ne!(after_rapids, after_holder, "rapid collisions must be keyed");

    // (c) the simulated cell size, which enriches a measurability
    // abstention's reason — the field today's sole consumer renders.
    if let Some(results) = sim.results.as_mut() {
        results.column_grid_cell_mm *= 2.0;
    }
    let _ = sim.cached_simulation_triage(&session, 3);
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

/// Load-report and chipload-envelope caches: a hit must still be a hit
/// (the key change is strictly narrowing, and both are per-frame paths),
/// and both must miss once their trace is replaced.
#[test]
fn load_report_and_envelope_caches_hit_then_miss_on_a_new_trace() {
    let session = rs_cam_core::session::ProjectSession::new_empty();
    let mut sim = simulation_for_toolpath();
    let first = attach_cut_trace(&mut sim);
    drop(first);

    let _ = sim.cached_load_report(&session, 1);
    let _ = sim.cached_chipload_envelopes(&session, 1);
    assert!(sim.debug.load_report_cache.report.is_some());
    assert!(sim.debug.chipload_envelope_cache.envelopes.is_some());
    let live = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .cloned();
    assert!(
        weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
        "the stored key must match the trace it was built from"
    );
    drop(live);

    sim.results = None;
    let mut refreshed = simulation_for_toolpath();
    attach_cut_trace(&mut refreshed);
    sim.results = refreshed.results.take();
    let live = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .cloned();
    assert!(
        !weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
        "a freed trace's key must not answer for its replacement"
    );
    assert!(
        !weak_matches(
            sim.debug.chipload_envelope_cache.trace.as_ref(),
            live.as_ref()
        ),
        "a freed trace's key must not answer for its replacement"
    );

    let _ = sim.cached_load_report(&session, 1);
    assert!(
        weak_matches(sim.debug.load_report_cache.trace.as_ref(), live.as_ref()),
        "the rebuild must re-key against the live trace"
    );
}
