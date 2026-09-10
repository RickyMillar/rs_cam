//! G-LOADREGEN sentry (F2.6) — opening a project asks the 2.5D
//! operations to regenerate and leaves the 3D ones alone.
//!
//! `open_job_from_path` built every toolpath runtime as
//! `ToolpathRuntime::new(true)` with `stale_since = Some(loaded_at)`,
//! regardless of the operation's own `default_auto_regen()`. Five hundred
//! milliseconds after a load, `process_auto_regen` therefore submitted
//! EVERY operation — including the 3D families whose card says MAN and
//! whose generation is minutes, not milliseconds. Opening a saved job
//! started that work without being asked, and that load-time sweep is the
//! precondition the G-REGEN-RACE reproduction is built on: an agent's
//! `generate_all` arriving while one of those submits is still the lane's
//! ACTIVE job resubmits it.
//!
//! The operator's ruling on R0.1 §7 Q2: request regeneration for 2.5D
//! operations only, respecting each operation's own dial; a 3D
//! manual-regen operation loads with no result and waits for an explicit
//! Generate.
//!
//! The load-bearing second half is that waiting must not LOOK like
//! failure. A freshly loaded 3D operation reads `NoResult` — the same
//! state a never-generated operation has always had, which every surface
//! F2.2 wired renders as outstanding work — and never `Error`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest, LaneSnapshot,
    OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::toolpath::{OperationConfig, ToolpathId};

/// A backend that records what the controller submitted and runs nothing.
/// The record lives behind an `Arc` so the test can read it after the
/// backend has moved into the controller.
struct RecordingBackend {
    submitted: Arc<Mutex<Vec<ToolpathId>>>,
}

impl ComputeBackend for RecordingBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        if let Ok(mut log) = self.submitted.lock() {
            log.push(request.toolpath_id);
        }
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

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// A project path in a directory of its own.
///
/// Not just a unique FILE name: `ProjectSession::save` writes its atomic
/// temp file as `.rs_cam_save_<pid>.tmp` in the project's parent
/// directory, so two saves from the same process into the same directory
/// race and one of them loses the rename. These tests run in parallel in
/// one process, so each gets its own directory.
fn temp_project_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rs_cam_{name}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("create the project directory");
    dir.join("job.toml")
}

/// Remove the project and the directory made for it.
fn clean_up(path: &std::path::Path) {
    std::fs::remove_file(path).ok();
    if let Some(dir) = path.parent() {
        std::fs::remove_dir(dir).ok();
    }
}

fn toolpath(id: usize, name: &str, model_id: usize, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id,
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
    }
}

/// Write a project with one Pocket (2.5D, `default_auto_regen` true) and
/// one Scallop (3D, `default_auto_regen` false) through the real save
/// path, so the file shape is whatever the loader actually accepts.
fn write_two_op_project() -> PathBuf {
    let mut session = ProjectSession::new_empty();
    session
        .tools_mut()
        .push(ToolConfig::new_default(ToolId(1), ToolType::BallNose));
    // Two models, because the two operations need different geometry: the
    // pocket is 2.5D and wants polygons, the scallop is 3D and wants a
    // mesh. The loader re-imports both from these paths, so an operation
    // that reaches the compute lane really is one the validator accepts.
    for (id, file, kind) in [
        (0usize, "square.svg", ModelKind::Svg),
        (1usize, "flat_plate.stl", ModelKind::Stl),
    ] {
        session.add_model(LoadedModel {
            id,
            path: fixtures_dir().join(file),
            name: file.to_owned(),
            kind: Some(kind),
            mesh: None,
            polygons: None,
            drill_targets: Arc::new(Vec::new()),
            layers: Arc::new(Vec::new()),
            enriched_mesh: None,
            units: Some(ModelUnits::Millimeters),
            winding_report: None,
            load_error: None,
        });
    }
    session
        .add_toolpath(
            0,
            toolpath(
                0,
                "Pocket",
                0,
                OperationConfig::Pocket(rs_cam_core::compute::PocketConfig::default()),
            ),
        )
        .expect("add pocket");
    session
        .add_toolpath(
            0,
            toolpath(
                1,
                "Scallop",
                1,
                OperationConfig::Scallop(rs_cam_core::compute::ScallopConfig::default()),
            ),
        )
        .expect("add scallop");

    let path = temp_project_path("loadregen");
    session.save(&path).expect("save the project");
    path
}

struct Loaded {
    controller: AppController<RecordingBackend>,
    submitted: Arc<Mutex<Vec<ToolpathId>>>,
    pocket: ToolpathId,
    scallop: ToolpathId,
    path: PathBuf,
}

fn load_the_project() -> Loaded {
    let path = write_two_op_project();
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut controller = AppController::with_backend(RecordingBackend {
        submitted: Arc::clone(&submitted),
    });
    controller
        .open_job_from_path(&path)
        .expect("the project loads");

    let ids: Vec<(String, ToolpathId)> = controller
        .state()
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| (tc.name.clone(), tc.id))
        .collect();
    let pocket = ids
        .iter()
        .find(|(name, _)| name == "Pocket")
        .expect("the pocket survived the round trip")
        .1;
    let scallop = ids
        .iter()
        .find(|(name, _)| name == "Scallop")
        .expect("the scallop survived the round trip")
        .1;

    Loaded {
        controller,
        submitted,
        pocket,
        scallop,
        path,
    }
}

// ── 1. What the load asks for ───────────────────────────────────────────

#[test]
fn a_load_requests_regeneration_for_the_25d_op_only() {
    let mut loaded = load_the_project();

    let rt = |c: &AppController<RecordingBackend>, id: ToolpathId| {
        (
            c.state().gui.toolpath_rt[&id].auto_regen,
            c.state().gui.toolpath_rt[&id].stale_since.is_some(),
        )
    };
    assert_eq!(
        rt(&loaded.controller, loaded.pocket),
        (true, true),
        "a 2.5D operation keeps its own dial (on) and gets a request"
    );
    assert_eq!(
        rt(&loaded.controller, loaded.scallop),
        (false, false),
        "a 3D manual-regen operation gets neither: opening a job must not \
         start minutes of compute nobody asked for"
    );

    // Age the request past the 500 ms debounce and run the sweep — the one
    // that used to submit everything.
    if let Some(entry) = loaded
        .controller
        .state_mut()
        .gui
        .toolpath_rt
        .get_mut(&loaded.pocket)
    {
        entry.stale_since = Some(Instant::now() - Duration::from_millis(600));
    }
    loaded.controller.process_auto_regen();

    let submitted = loaded.submitted.lock().expect("lock").clone();
    assert_eq!(
        submitted,
        vec![loaded.pocket],
        "the sweep submits the pocket and nothing else; got {submitted:?}"
    );

    clean_up(&loaded.path);
}

/// The operator-visible half: after the debounce has elapsed and the sweep
/// has run, the 3D operation is still waiting. Pre-fix it had been
/// submitted and was generating — the minutes of compute the ruling is
/// about — and the card had moved off PEND without anyone asking.
#[test]
fn the_3d_op_is_still_waiting_after_the_sweep_runs() {
    let mut loaded = load_the_project();

    // Age every request the load left, so the debounce cannot be what
    // holds the scallop back. Post-fix it has none to age.
    let ids: Vec<ToolpathId> = loaded
        .controller
        .state()
        .gui
        .toolpath_rt
        .keys()
        .copied()
        .collect();
    for id in ids {
        if let Some(entry) = loaded.controller.state_mut().gui.toolpath_rt.get_mut(&id)
            && entry.stale_since.is_some()
        {
            entry.stale_since = Some(Instant::now() - Duration::from_millis(600));
        }
    }
    loaded.controller.process_auto_regen();

    let submitted = loaded.submitted.lock().expect("lock").clone();
    assert!(
        !submitted.contains(&loaded.scallop),
        "the sweep must not submit a manual-regen operation; got {submitted:?}"
    );

    let state = loaded.controller.state();
    let index = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == loaded.scallop)
        .expect("the scallop is in the session");
    assert_eq!(
        freshness_at(&state.session, &state.gui, index),
        Some(FreshnessState::NoResult),
        "the 3D operation waits for an explicit Generate; it must not have \
         drifted into Regenerating on its own"
    );

    clean_up(&loaded.path);
}

// ── 2. Waiting must not look like failure ───────────────────────────────

#[test]
fn the_unregenerated_3d_op_reads_no_result_not_error() {
    let loaded = load_the_project();
    let state = loaded.controller.state();

    for (index, tc) in state.session.toolpath_configs().iter().enumerate() {
        let freshness = freshness_at(&state.session, &state.gui, index)
            .expect("every loaded toolpath has a state");
        assert_eq!(
            freshness,
            FreshnessState::NoResult,
            "'{}' loads as not-yet-generated; nothing about a fresh load is \
             a failure",
            tc.name
        );
        assert!(
            !matches!(freshness, FreshnessState::Error(_)),
            "'{}' must never read as an error on load",
            tc.name
        );
    }

    clean_up(&loaded.path);
}

/// The surfaces F2.2 wired, checked through the shared helpers rather than
/// by drawing: a freshly loaded project counts as PENDING work, never as
/// stale work and never as a failure. `stale` is the count that carries
/// the "you are looking at a wrong answer" meaning; it must be zero here,
/// because nothing has been generated to be wrong.
#[test]
fn a_fresh_load_counts_as_pending_never_as_stale() {
    let loaded = load_the_project();
    let state = loaded.controller.state();

    let (stale, pending) = rs_cam_viz::ui::readiness::freshness_counts(state);
    assert_eq!(
        (stale, pending),
        (0, 2),
        "both operations are outstanding work, neither is a wrong answer"
    );

    let (status, current, enabled) = rs_cam_viz::ui::readiness::operations_check(state);
    assert_eq!(
        (current, enabled),
        (0, 2),
        "nothing is current yet, and both operations are enabled"
    );
    assert_eq!(
        status,
        rs_cam_viz::ui::readiness::CheckStatus::Warning,
        "outstanding work is a Warning, not a Fail — a Fail is what a \
         collision and a failed generation spend, and a freshly opened job \
         is neither"
    );

    clean_up(&loaded.path);
}

/// The card chip for a loaded 3D operation is `PEND`, with no hover
/// warning. `ui::toolpath_panel::status_chip` is `pub(crate)`, so the
/// vocabulary itself is pinned where it can be reached —
/// `controller/tests.rs::freshness_chip_says_stale_and_never_says_ok`
/// asserts `(FreshnessState::NoResult, "PEND")` among the six. What is
/// left to pin from out here is the input to that lookup, which is the
/// half F2.6 changed: a freshly loaded 3D operation must arrive at the
/// chip as `NoResult` and not as anything louder.
#[test]
fn the_loaded_3d_op_reaches_the_chip_as_pending() {
    let loaded = load_the_project();
    let state = loaded.controller.state();
    let index = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == loaded.scallop)
        .expect("the scallop is in the session");

    assert_eq!(
        freshness_at(&state.session, &state.gui, index),
        Some(FreshnessState::NoResult),
        "the state the card chip reads for a loaded 3D operation"
    );

    clean_up(&loaded.path);
}
