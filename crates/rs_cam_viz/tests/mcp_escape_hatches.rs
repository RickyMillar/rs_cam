//! A/M12 sentries — the MCP escape hatches must answer while a generation is
//! in flight.
//!
//! Measured live on wanaka, 2026-07-30: a `generate_all` ran >40 minutes and
//! every other MCP call queued behind it. `list_toolpaths` was aborted after
//! 1800 s of silence; `cancel_generation` — documented "Instant response" —
//! was serviced only *after* the job had already finished and reported
//! `was_busy: false`. Both were the remedies `generate_all`'s own timeout
//! message recommended.
//!
//! The serializer was never a lock over `ProjectSession`. It is
//! `RsCamApp::drain_mcp_requests`: the single place every MCP request is
//! dispatched, running on the egui main thread, once per repaint, strictly
//! sequentially. These tests reproduce that condition exactly — an
//! `McpRequest` receiver that is held open and never drained — and assert the
//! escape hatches answer anyway.

#![cfg(feature = "mcp")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rs_cam_viz::compute::{
    CancelOutcome, ComputeLane, GenerationControl, LaneControl, LaneSnapshot, LaneState,
};
use rs_cam_viz::mcp_bridge::{McpReadCache, McpReadSnapshot, McpRequest};
use rs_cam_viz::mcp_server::EmbeddedCamServer;

/// The gate. Every escape hatch must beat this even with the GUI thread gone.
const GATE: Duration = Duration::from_secs(1);

/// A toolpath lane stuck on a long job — the live 2026-07-30 condition minus
/// the 40 minutes. Cancellation is observable; the stage string is advanced by
/// the test to stand in for the worker's phase tracker.
struct StuckLane {
    cancel_requested: AtomicBool,
    stage: Mutex<Option<String>>,
    started_at: Instant,
}

impl StuckLane {
    fn new(stage: &str) -> Arc<Self> {
        Arc::new(Self {
            cancel_requested: AtomicBool::new(false),
            stage: Mutex::new(Some(stage.to_owned())),
            started_at: Instant::now(),
        })
    }

    fn advance_stage(&self, stage: &str) {
        *self.stage.lock().unwrap() = Some(stage.to_owned());
    }

    fn snapshot_now(&self, state: LaneState) -> LaneSnapshot {
        LaneSnapshot {
            lane: ComputeLane::Toolpath,
            state,
            queue_depth: 2,
            current_job: Some("Unified Finish 6 (unified_finish)".to_owned()),
            current_phase: self.stage.lock().unwrap().clone(),
            started_at: Some(self.started_at),
            active_toolpath_id: Some(15),
            active_toolpath_index: Some(8),
        }
    }
}

impl LaneControl for StuckLane {
    fn snapshot(&self) -> LaneSnapshot {
        let state = if self.cancel_requested.load(Ordering::SeqCst) {
            LaneState::Cancelling
        } else {
            LaneState::Running
        };
        self.snapshot_now(state)
    }

    fn request_cancel(&self) -> CancelOutcome {
        self.cancel_requested.store(true, Ordering::SeqCst);
        CancelOutcome {
            was_busy: true,
            snapshot: self.snapshot_now(LaneState::Cancelling),
        }
    }
}

/// Build a server whose GUI thread is *gone*: the request receiver is kept
/// alive (so sends succeed) but never drained (so no oneshot ever resolves).
/// Returns the receiver so the caller keeps the channel open.
fn stalled_gui(
    lane: &Arc<StuckLane>,
    reads: McpReadCache,
) -> (EmbeddedCamServer, std::sync::mpsc::Receiver<McpRequest>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let control = GenerationControl::new(Arc::clone(lane) as Arc<dyn LaneControl>);
    let server = EmbeddedCamServer::new(tx, egui::Context::default(), control, reads);
    (server, rx)
}

fn published_cache() -> McpReadCache {
    let cache = McpReadCache::new();
    cache.publish(McpReadSnapshot {
        list_toolpaths: serde_json::to_string(&serde_json::json!([
            {"index": 0, "name": "Back Rough", "status": "Done"},
            {"index": 8, "name": "Unified Finish 6", "status": "Computing"},
        ]))
        .unwrap(),
        project_summary: serde_json::to_string(&serde_json::json!({"name": "wanaka"})).unwrap(),
        ..Default::default()
    });
    cache
}

/// The headline defect: `cancel_generation` answered only after the job it was
/// meant to abort had already ended. It must now answer immediately AND set
/// the flag the compute loop polls, with no GUI thread involved at all.
#[tokio::test]
async fn cancel_generation_answers_and_cancels_with_the_gui_thread_gone() {
    let lane = StuckLane::new("scallop_ring_cascade");
    let (server, _rx) = stalled_gui(&lane, published_cache());

    let started = Instant::now();
    let resp = server.cancel_generation().await;
    let elapsed = started.elapsed();

    assert!(
        elapsed < GATE,
        "cancel_generation took {elapsed:?} with a stalled GUI — the whole point is that \
         it never queues behind the frame loop"
    );
    assert!(
        lane.cancel_requested.load(Ordering::SeqCst),
        "cancel_generation must actually set the lane's cancel flag, not just reply"
    );

    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["was_busy"], true);
    assert_eq!(v["toolpath_index"], 8);
    assert_eq!(v["stage"], "scallop_ring_cascade");
}

/// `list_toolpaths` was aborted after 1800 s of silence. It must now answer
/// inside the gate, and it must say plainly that the answer is a snapshot —
/// a lagging view labelled as live would be a worse defect than the block.
#[tokio::test]
async fn list_toolpaths_answers_from_the_snapshot_when_the_frame_loop_stalls() {
    let lane = StuckLane::new("scallop_ring_cascade");
    let (server, _rx) = stalled_gui(&lane, published_cache());

    let started = Instant::now();
    let resp = server.list_toolpaths().await;
    let elapsed = started.elapsed();

    assert!(
        elapsed < GATE,
        "list_toolpaths took {elapsed:?} with a stalled GUI (gate: {GATE:?})"
    );

    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["served_from"], "snapshot");
    assert!(v["snapshot_age_s"].as_f64().unwrap() >= 0.0);
    assert_eq!(v["data"][1]["name"], "Unified Finish 6");
    assert_eq!(
        v["generation_in_flight"]["toolpath_index"], 8,
        "a snapshot answer must name what is holding the frame loop"
    );
}

/// `project_summary` takes the same path. Included so the fallback is not a
/// one-off special case for the tool the gate happens to name.
#[tokio::test]
async fn project_summary_also_answers_from_the_snapshot() {
    let lane = StuckLane::new("preflight");
    let (server, _rx) = stalled_gui(&lane, published_cache());

    let started = Instant::now();
    let resp = server.project_summary().await;
    assert!(started.elapsed() < GATE);

    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["served_from"], "snapshot");
    assert_eq!(v["data"]["name"], "wanaka");
}

/// Never-published is not the same as stale. If the GUI has not completed a
/// single frame there is nothing to serve, and the reply must say so rather
/// than hand back an empty list that reads like "this project has no
/// toolpaths".
#[tokio::test]
async fn a_read_with_no_published_snapshot_refuses_instead_of_inventing_one() {
    let lane = StuckLane::new("preflight");
    let (server, _rx) = stalled_gui(&lane, McpReadCache::new());

    let started = Instant::now();
    let resp = server.list_toolpaths().await;
    assert!(started.elapsed() < GATE);

    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["served_from"], "nothing");
    assert!(
        v["error"].as_str().unwrap().contains("never published"),
        "got: {resp}"
    );
}

/// An idle lane is NOT an exemption: a cheap read answers inside the gate
/// whatever the GUI thread is doing.
///
/// **This test deliberately replaces `an_idle_lane_keeps_the_old_unbounded_
/// read_behaviour`, which pinned the opposite behaviour** (C6, ruled
/// 2026-08-06). That test asserted `outcome.is_err()` — i.e. that a read
/// against an idle-lane stalled GUI *never returns* — on the assumption that
/// only a generation could hold the frame loop. It cannot hold: every drained
/// MCP request is handled in one frame on the egui main thread, so a long
/// `narrate_toolpath` or `get_cut_trace` stalls the loop with the lane idle,
/// and in that state all five cheap reads blocked for the whole read. The old
/// sentry would have stayed green through exactly the stall H2.6 is about.
///
/// The guarantee pinned here is the new one: **< 1 s, independent of lane
/// activity**, with the answer explicitly marked as a snapshot.
#[tokio::test]
async fn cheap_reads_answer_within_the_gate_even_with_the_lane_idle() {
    struct IdleLane;
    impl LaneControl for IdleLane {
        fn snapshot(&self) -> LaneSnapshot {
            LaneSnapshot::idle(ComputeLane::Toolpath)
        }
        fn request_cancel(&self) -> CancelOutcome {
            CancelOutcome {
                was_busy: false,
                snapshot: LaneSnapshot::idle(ComputeLane::Toolpath),
            }
        }
    }

    // The receiver is held but never drained: a GUI frame loop stalled by
    // something that is NOT a generation — the C6 case.
    let (tx, _rx) = std::sync::mpsc::channel();
    let server = EmbeddedCamServer::new(
        tx,
        egui::Context::default(),
        GenerationControl::new(Arc::new(IdleLane)),
        published_cache(),
    );

    let started = Instant::now();
    let resp = tokio::time::timeout(GATE, server.list_toolpaths())
        .await
        .expect(
            "list_toolpaths must answer within the 1 s gate with the lane IDLE — this is \
                 the guarantee C6 added, replacing a sentry that pinned the block",
        );
    let elapsed = started.elapsed();

    let v: serde_json::Value = serde_json::from_str(&resp).expect("valid json");
    assert_eq!(
        v["served_from"], "snapshot",
        "a read the stalled frame loop could not answer must say it came from the snapshot, \
         not pass a lagging view off as live: {resp}"
    );
    assert_eq!(
        v["ok"], true,
        "the snapshot was published, so this must succeed: {resp}"
    );
    assert_eq!(
        v["data"][1]["name"], "Unified Finish 6",
        "the snapshot payload must still be the real list_toolpaths body: {resp}"
    );
    // The summary must not claim a generation that is not running.
    let summary = v["summary"].as_str().unwrap_or_default();
    assert!(
        !summary.contains("generation is in flight"),
        "with the lane idle the fallback must not blame a generation: {summary}"
    );
    assert!(
        summary.contains("in-band read"),
        "the fallback should name what is actually holding the frame loop: {summary}"
    );
    assert!(elapsed < GATE, "answered in {elapsed:?}, gate is {GATE:?}");
}

/// `generation_status` must name the in-flight index and stage, and it must
/// ADVANCE — a status call that returns the same frozen string forever is
/// indistinguishable from the silence it was added to fix.
#[tokio::test]
async fn generation_status_names_the_in_flight_op_and_advances() {
    let lane = StuckLane::new("preflight");
    let (server, _rx) = stalled_gui(&lane, published_cache());

    let started = Instant::now();
    let first: serde_json::Value = serde_json::from_str(&server.generation_status().await).unwrap();
    assert!(started.elapsed() < GATE);

    assert_eq!(first["busy"], true);
    assert_eq!(first["lane_state"], "running");
    assert_eq!(first["toolpath_index"], 8);
    assert_eq!(first["toolpath_id"], 15);
    assert_eq!(first["stage"], "preflight");
    assert_eq!(first["queue_depth"], 2);
    let t0 = first["elapsed_s"].as_f64().unwrap();

    lane.advance_stage("scallop_ring_cascade");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let second: serde_json::Value =
        serde_json::from_str(&server.generation_status().await).unwrap();
    assert_eq!(
        second["stage"], "scallop_ring_cascade",
        "generation_status must track the planner's stage, not a value frozen at submit"
    );
    let t1 = second["elapsed_s"].as_f64().unwrap();
    assert!(
        t1 > t0,
        "elapsed_s must advance between calls ({t0} -> {t1}) or the caller cannot tell \
         a grinding job from a deadlocked one"
    );
    assert!(
        second["summary"]
            .as_str()
            .unwrap()
            .contains("toolpath index 8"),
        "got: {second}"
    );
}

/// An idle lane must say so plainly rather than reporting a stale last job.
#[tokio::test]
async fn generation_status_on_an_idle_lane_reports_idle() {
    struct IdleLane;
    impl LaneControl for IdleLane {
        fn snapshot(&self) -> LaneSnapshot {
            LaneSnapshot::idle(ComputeLane::Toolpath)
        }
        fn request_cancel(&self) -> CancelOutcome {
            CancelOutcome {
                was_busy: false,
                snapshot: LaneSnapshot::idle(ComputeLane::Toolpath),
            }
        }
    }
    let (tx, _rx) = std::sync::mpsc::channel();
    let server = EmbeddedCamServer::new(
        tx,
        egui::Context::default(),
        GenerationControl::new(Arc::new(IdleLane)),
        McpReadCache::new(),
    );

    let v: serde_json::Value = serde_json::from_str(&server.generation_status().await).unwrap();
    assert_eq!(v["busy"], false);
    assert_eq!(v["lane_state"], "idle");
    assert!(v["toolpath_index"].is_null());
    assert!(v["elapsed_s"].is_null());
}

/// The A/M12 acceptance gate, end to end over the MCP surface rather than the
/// library API underneath it: kick a generate, observe it, stop it — with the
/// GUI frame loop stalled throughout, which is the state the live run was in.
#[tokio::test]
async fn generate_then_status_then_cancel_over_the_mcp_surface() {
    let lane = StuckLane::new("preflight");
    let (server, _rx) = stalled_gui(&lane, published_cache());

    // 1. generate_all with a 1 s wait budget. The stalled GUI never resolves
    //    the oneshot, so this is the live "timed out, moved to background"
    //    condition.
    let generate: serde_json::Value = serde_json::from_str(
        &server
            .generate_all_without_peer(Some(1), Some(false), None)
            .await,
    )
    .unwrap();
    assert_eq!(generate["status"], "running");

    // 2. Observe it. This is the call that did not exist on 2026-07-30, when
    //    the only working diagnosis was reading /proc thread accounting.
    lane.advance_stage("clear_z_level");
    let started = Instant::now();
    let status: serde_json::Value =
        serde_json::from_str(&server.generation_status().await).unwrap();
    assert!(started.elapsed() < GATE);
    assert_eq!(status["toolpath_index"], 8);
    assert_eq!(status["stage"], "clear_z_level");

    // 3. Stop it — and have the flag actually set, in time to matter.
    let started = Instant::now();
    let cancel: serde_json::Value =
        serde_json::from_str(&server.cancel_generation().await).unwrap();
    assert!(started.elapsed() < GATE);
    assert_eq!(cancel["was_busy"], true);
    assert!(lane.cancel_requested.load(Ordering::SeqCst));

    // 4. And the lane reports it, so the caller can confirm rather than
    //    inferring from a CPU graph (the error the live run recorded).
    let after: serde_json::Value = serde_json::from_str(&server.generation_status().await).unwrap();
    assert_eq!(after["lane_state"], "cancelling");
}

/// Throughput guard for the snapshot machinery itself. The publish runs on the
/// GUI thread at ≤2 Hz; the read runs on the MCP thread. Neither touches the
/// compute lane, but the shared `RwLock` must not be a contention point if
/// something later raises the publish rate.
#[test]
fn read_cache_publish_and_get_are_cheap() {
    let cache = published_cache();
    let started = Instant::now();
    for _ in 0..10_000 {
        cache.publish(McpReadSnapshot {
            list_toolpaths: "[]".to_owned(),
            ..Default::default()
        });
        let _ = cache.get(rs_cam_viz::mcp_bridge::McpReadKind::ListToolpaths);
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "10k publish+get round trips took {elapsed:?}; the per-frame publish is \
         supposed to be free relative to a frame"
    );
}
