//! G-MODEXPORT sentry — the emitted BYTES must carry the modulated
//! per-move feed schedule, not the commanded one.
//!
//! The F-036b/F-039 adaptive feed-modulation post-pass
//! (`ProjectSession::apply_adaptive_feed_modulation`, reached from the
//! GUI/MCP sim-complete handler via `modulate_simulation_trace`) rewrites
//! per-move `feed_rate` values and swaps the modulated
//! `Arc<AnnotatedToolpath>` into `session.results` — and into nothing
//! else. Until 2026-08-22 every viz export surface (export wizard,
//! `app/export.rs`, `controller/io.rs`, MCP `export_gcode`) funnelled
//! through `io::export::emitted_toolpaths`, which read the *viz* store
//! `gui.toolpath_rt[id].result` — the compute worker's pre-modulation IR.
//! The export GATE meanwhile reads the viz cut trace, which the post-pass
//! DOES stamp, so the verdict and the emitted program described two
//! different feed schedules: on the live wanaka fixture the gate reported
//! chipload `Within` at a modulated ~443 mm/min median while the file
//! carried a flat commanded F3000 (6.8× band max on a Ø1-tip tapered
//! ball).
//!
//! The core-side F-036b net already asserted bytes — but through
//! `rs_cam_core::gcode::export_gcode_checked`, i.e. the CLI path, which
//! reads `session.results` and was always correct. Nothing crossed the
//! viz export boundary. This file does.
//!
//! Two tests, one per side of the store contract:
//!
//! 1. `viz_export_emits_the_modulated_feed_schedule` — result present in
//!    BOTH stores (the F1_RCA sync), then a modulated variant swapped
//!    into `session.results` ONLY (what the post-pass does). The emitted
//!    F-words must be the modulated ones.
//! 2. `viz_export_falls_back_to_the_worker_result_when_the_session_slot_is_invalidated`
//!    — pins the one reachable divergence in the other direction:
//!    `ProjectSession::invalidate_tool` (a GUI tool-param edit, see
//!    `ui/properties::commit_tool_draft`) drops `session.results` while
//!    the viz store keeps its result for the stale display. Export must
//!    still emit, from the worker IR, exactly as it did before the fix.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathComputeResult, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::io::export::export_gcode_from_session_with_policy;
use rs_cam_viz::state::runtime::{GuiState, ToolpathRuntime};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::state::toolpath::{OperationConfig, ToolpathResult};

/// Feed the generator commanded (mm/min).
const COMMANDED_FEED: f64 = 600.0;
/// Feed the modulation post-pass settled on (mm/min) — deliberately half
/// the commanded one, so a byte read can't confuse the two.
const MODULATED_FEED: f64 = 300.0;

/// Three cutting moves at `feed`, bracketed by rapids.
fn sample_toolpath(feed: f64) -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), feed);
    path.feed_to(P3::new(10.0, 10.0, -1.0), feed);
    path.feed_to(P3::new(0.0, 10.0, -1.0), feed);
    path.rapid_to(P3::new(0.0, 10.0, 5.0));
    path
}

fn core_result(path: Toolpath) -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(path))),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn viz_result(path: Toolpath) -> ToolpathResult {
    ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    }
}

/// Session + GUI + sim state with one enabled toolpath whose commanded
/// result lives in BOTH stores — the post-`drain_compute_results` state
/// (the F1_RCA sync in `controller/events/compute.rs` writes
/// `session.results` and `gui.toolpath_rt` from the same worker output).
fn build_state() -> (ProjectSession, GuiState, SimulationState) {
    let mut session = ProjectSession::new_empty();
    session.set_name("g-modexport sentry".to_owned());
    session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::EndMill));

    let mesh = Arc::new(make_test_flat(40.0));
    session.add_model(LoadedModel {
        id: 0,
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(mesh),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    let tp = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Sample Path".to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default()),
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
    };
    session.add_toolpath(0, tp).expect("add toolpath");
    let tp_id = session.toolpath_configs()[0].id;

    session
        .insert_result(0, core_result(sample_toolpath(COMMANDED_FEED)))
        .expect("insert core result");

    let mut gui = GuiState::new();
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(viz_result(sample_toolpath(COMMANDED_FEED)));
    gui.toolpath_rt.insert(tp_id, rt);

    // No simulation is run here, so every tool-load criterion reads
    // Unmodeled(SimulationRequired) — accept it, as the export gate's
    // override toggle does. The gate is covered elsewhere; this file is
    // about which IR the emitter is handed.
    gui.tool_load_overrides.accept_unmodeled = true;

    (session, gui, SimulationState::new())
}

/// Distinct standalone F-word values in the emitted program, in first-seen
/// order. Comment lines are skipped (the post writes a `(...)` header).
fn f_words(gcode: &str) -> Vec<f64> {
    let mut seen: Vec<f64> = Vec::new();
    for line in gcode.lines() {
        let line = line.trim();
        if line.starts_with('(') || line.starts_with(';') {
            continue;
        }
        let mut rest = line;
        while let Some(pos) = rest.find('F') {
            let after = &rest[pos + 1..];
            let end = after
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(after.len());
            let digits = &after[..end];
            if let Ok(v) = digits.parse::<f64>()
                && !seen.iter().any(|s| (s - v).abs() < 1e-6)
            {
                seen.push(v);
            }
            rest = &after[end..];
        }
    }
    seen
}

fn export(session: &ProjectSession, gui: &GuiState, sim: &SimulationState) -> String {
    export_gcode_from_session_with_policy(
        session,
        gui,
        sim,
        rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        },
    )
    .expect("export succeeds")
}

#[test]
fn viz_export_emits_the_modulated_feed_schedule() {
    let (mut session, gui, sim) = build_state();

    // Sanity: before modulation the program carries the commanded feed,
    // and the two stores agree.
    let before = f_words(&export(&session, &gui, &sim));
    assert_eq!(
        before,
        vec![COMMANDED_FEED],
        "pre-modulation export must carry the commanded feed only"
    );

    // What `apply_adaptive_feed_modulation` does: swap the modulated
    // annotated toolpath into `session.results` — and nowhere else.
    session
        .insert_result(0, core_result(sample_toolpath(MODULATED_FEED)))
        .expect("swap modulated result");

    let after = f_words(&export(&session, &gui, &sim));
    assert_eq!(
        after,
        vec![MODULATED_FEED],
        "G-MODEXPORT: the exported BYTES must carry the modulated \
         schedule that `session.results` holds ({MODULATED_FEED} mm/min), \
         not the pre-modulation worker feed the viz store still holds \
         ({COMMANDED_FEED} mm/min). Emitted F-words: {after:?}"
    );
}

#[test]
fn viz_export_falls_back_to_the_worker_result_when_the_session_slot_is_invalidated() {
    let (mut session, gui, sim) = build_state();

    // The reachable divergence: a GUI tool-param edit calls
    // `invalidate_tool`, which drops the affected `session.results`
    // entries. The viz store keeps its result (that's what the stale
    // badge is drawn over), so export must still emit rather than
    // refusing with "No computed toolpaths to export".
    session.invalidate_tool(1);
    assert!(
        session.get_result(0).is_none(),
        "invalidate_tool must clear the session result — otherwise this \
         test is not exercising the fallback"
    );

    let feeds = f_words(&export(&session, &gui, &sim));
    assert_eq!(
        feeds,
        vec![COMMANDED_FEED],
        "with the session slot invalidated the worker IR is the only \
         result left; export must fall back to it"
    );
}
