//! G-RESTRES parity, the GUI half: the GUI Generate All on a small rest
//! cascade publishes the numbers `expected.json` holds.
//!
//! Plan: `planning/rest_stock_identity_2026-09-24/PLAN.md` §5 test 9.
//! Operator ruling 2026-09-24: the GUI, MCP and the CLI give IDENTICAL
//! numbers for the same project state.
//!
//! # One record, two paths
//!
//! `crates/rs_cam_core/tests/fixtures/rest_cascade_g_restres/project.toml`
//! stores its simulation resolution and holds a pocket and a rest pocket.
//! This test opens it through the GUI's own door, runs the GUI plan with the
//! real worker functions called inline, and reads the core diagnostics the
//! MCP `get_diagnostics` tool publishes. The CLI half
//! (`crates/rs_cam_cli/tests/rest_cascade_parity_g_restres.rs`) runs the
//! `rs_cam_cli project` binary on the same file with NO `--resolution` and
//! reads its `tp_*.json`. Both compare with the one `expected.json`, so the
//! two paths are equal through it, and neither half replicates the other.
//!
//! The record is the move count and the whole `source_stock` record: the
//! simulation cell, the digest of the snapshot the rest pocket read, and the
//! digest of each toolpath carved before it.

use std::sync::atomic::AtomicBool;

use super::*;

/// The real worker's two functions, called inline: the toolpath lane's
/// `run_compute_with_phase_tracker` (through `run_compute_inline`) and the analysis lane's
/// `run_simulation_with_phase`. No thread, no queue; the same code.
struct InlineBackend {
    drained: Vec<ComputeMessage>,
}

impl ComputeBackend for InlineBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let outcome = crate::compute::worker::execute::run_compute_inline(&request);
        self.drained.push(ComputeMessage::Toolpath(Box::new(
            crate::compute::ComputeResult {
                toolpath_id: request.viz.toolpath_id,
                revision: Some(request.handle.revision),
                result: outcome.result,
                debug_trace: outcome.debug_trace,
                semantic_trace: outcome.semantic_trace,
                debug_trace_path: outcome.debug_trace_path,
            },
        )));
        ToolpathSubmitOutcome::Queued
    }

    fn submit_simulation(&mut self, request: SimulationRequest) {
        let cancel = AtomicBool::new(false);
        let result = crate::compute::worker::execute::run_simulation_with_phase(
            &request,
            &cancel,
            |_| {},
            None,
        );
        self.drained
            .push(ComputeMessage::Simulation(result.map(Box::new)));
    }

    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/rest_cascade_g_restres")
}

#[test]
fn the_gui_plan_publishes_the_expected_rest_numbers() {
    let dir = fixture_dir();
    let mut controller = AppController::with_backend(InlineBackend {
        drained: Vec::new(),
    });
    controller
        .open_job_from_path(&dir.join("project.toml"))
        .expect("the fixture opens");
    assert_eq!(
        controller.state.session.simulation_resolution(),
        rs_cam_core::session::SimulationResolution::Fixed(0.5),
        "the project file carries the stored resolution"
    );

    controller.handle_internal_event(crate::ui::AppEvent::GenerateAll);
    for _ in 0..200 {
        if !controller.awaiting_generate_all() {
            break;
        }
        controller.drain_compute_results();
    }
    assert!(!controller.awaiting_generate_all(), "the plan finished");
    assert!(
        controller.pending_plan_confirm().is_none(),
        "a stored cell fine enough for the rest asks nothing"
    );

    let diagnostics = controller.state.session.diagnostics();
    let mut record = serde_json::Map::new();
    for tp in &diagnostics.per_toolpath {
        let _ = record.insert(
            tp.name.clone(),
            serde_json::json!({
                "move_count": tp.move_count,
                "source_stock": tp.source_stock,
            }),
        );
    }
    let record = serde_json::json!({
        "simulation_resolution_mm": controller.state.session.simulation_resolution_mm(),
        "toolpaths": record,
    });

    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("expected.json")).expect("expected.json exists"),
    )
    .expect("expected.json is JSON");
    assert!(
        record
            .pointer("/toolpaths/Rest pocket/source_stock/after/0/output")
            .is_some(),
        "non-vacuity: the rest pocket must record the pocket it read: {record}"
    );
    assert_eq!(
        record, expected,
        "the GUI path must publish the numbers the CLI path publishes"
    );
}
