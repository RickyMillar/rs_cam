//! P9 — the GUI load path says the pin-keying fact once. C1: an old file
//! with the deleted `flip_axis` key (P3) refuses to open and names the key.
//!
//! `open_job_from_path` ran the core pin check a second time and hid the
//! copy with a dedupe. P9 deleted the second run: the GUI shows the core
//! loader's lines, and the core loader says each fact once.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest, LaneSnapshot,
    SimulationRequest, ToolpathSubmitOutcome,
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

/// The mini test piece as stored on 2026-10-12: two pins on the flip axis
/// (they seat, and do not key), three setups so the old GUI copy of the
/// check ran three times, and the deleted `flip_axis` key.
const MINI_PIECE: &str = r#"format_version = 3

[job]
name = "mini test piece"

[job.stock]
x = 65.0
y = 65.0
z = 24.5
origin_x = 0.0
origin_y = 0.0
origin_z = 0.0
padding = 0.0
auto_from_model = false
flip_axis = "horizontal"

[[job.stock.alignment_pins]]
x = 59.0
y = 32.5
diameter = 6.0

[[job.stock.alignment_pins]]
x = 6.0
y = 32.5
diameter = 6.0

[[setups]]
id = 0
name = "Top"
face_up = "top"

[[setups]]
id = 1
name = "Bottom"
face_up = "bottom"

[[setups]]
id = 2
name = "Bottom again"
face_up = "bottom"
"#;

#[test]
fn an_old_mini_piece_refuses_then_opens_with_one_keying_line() {
    let dir = std::env::temp_dir().join(format!("rs_cam_p9_load_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mini.toml");
    std::fs::write(&path, MINI_PIECE).unwrap();

    let mut controller = AppController::with_backend(IdleBackend);
    let refusal = controller
        .open_job_from_path(&path)
        .expect_err("C1: the deleted flip_axis key refuses the file");
    let message = format!("{refusal:?}");
    assert!(message.contains("job.stock.flip_axis"), "{message}");

    let without_key = MINI_PIECE.replace("flip_axis = \"horizontal\"\n", "");
    assert_ne!(without_key, MINI_PIECE, "non-vacuity: the key was there");
    std::fs::write(&path, without_key).unwrap();
    controller
        .open_job_from_path(&path)
        .expect("the file without the key opens");

    let keying: Vec<&String> = controller
        .load_warnings()
        .iter()
        .filter(|line| line.contains("do not key"))
        .collect();
    assert_eq!(
        keying.len(),
        1,
        "the load warnings must say the keying fact once: {:?}",
        controller.load_warnings()
    );
    assert_eq!(
        controller
            .state()
            .session()
            .stock_config()
            .alignment_pins
            .len(),
        2
    );
    std::fs::remove_dir_all(&dir).ok();
}
