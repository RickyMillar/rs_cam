//! G-NOLEGACY — `open_job_from_path` has one loader, and its refusal reaches
//! the operator.
//!
//! # The defect
//!
//! `open_job_from_path` matched on `ProjectSession::load` and caught EVERY
//! `SessionError` (I01 drift 3). The catch arm re-read the same bytes with a
//! second parser in `rs_cam_viz::io::project`, whose every section carried a
//! serde default. A file core refused — a CSV, a `Cargo.toml`, a project in a
//! format core does not read — therefore came back as a fully-defaulted empty
//! project, and the function returned `Ok(())`. The operator saw an empty
//! workspace and no error.
//!
//! # What is pinned
//!
//! A file that is not an rs_cam project returns `Err`, and the project the
//! controller already held is not replaced by an empty one.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest, LaneSnapshot,
    OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;

/// A backend that runs nothing. The loader never reaches it.
struct IdleBackend;

impl ComputeBackend for IdleBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> rs_cam_viz::compute::GenerationControl {
        rs_cam_viz::compute::GenerationControl::detached()
    }
}

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Write `content` to a private file and return its path.
fn write_temp(name: &str, content: &str) -> PathBuf {
    let pid = std::process::id();
    let seq = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rs_cam_nolegacy_{pid}"));
    std::fs::create_dir_all(&dir).expect("create the directory");
    let path = dir.join(format!("{name}_{seq}.toml"));
    std::fs::write(&path, content).expect("write the file");
    path
}

/// Well-formed TOML that is not an rs_cam project.
const NOT_A_PROJECT: &str = r#"
[package]
name = "something_else"
version = "0.1.0"

[dependencies]
serde = "1"
"#;

#[test]
fn a_file_that_is_not_a_project_is_refused() {
    let path = write_temp("not_a_project", NOT_A_PROJECT);
    let mut controller = AppController::with_backend(IdleBackend);
    let before = controller.state().session.name().to_owned();

    let result = controller.open_job_from_path(&path);

    assert!(
        result.is_err(),
        "a file that is not an rs_cam project must refuse to open, got {result:?}"
    );
    assert_eq!(
        controller.state().session.name(),
        before,
        "a refused open must leave the project that was already there"
    );
    std::fs::remove_file(&path).ok();
}
