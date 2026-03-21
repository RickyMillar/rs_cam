pub mod worker;

use std::time::{Duration, Instant};

pub use worker::{
    CollisionRequest, CollisionResult, ComputeRequest, ComputeResult, SimulationRequest,
    SimulationResult, ThreadedComputeBackend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeLane {
    Toolpath,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneState {
    Idle,
    Queued,
    Running,
    Cancelling,
}

#[derive(Debug, Clone)]
pub struct LaneSnapshot {
    pub lane: ComputeLane,
    pub state: LaneState,
    pub queue_depth: usize,
    pub current_job: Option<String>,
    pub started_at: Option<Instant>,
}

impl LaneSnapshot {
    pub fn idle(lane: ComputeLane) -> Self {
        Self {
            lane,
            state: LaneState::Idle,
            queue_depth: 0,
            current_job: None,
            started_at: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, LaneState::Running | LaneState::Cancelling)
    }

    pub fn elapsed(&self) -> Option<Duration> {
        self.started_at.map(|started_at| started_at.elapsed())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeError {
    Cancelled,
    Message(String),
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ComputeError {}

impl From<String> for ComputeError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

pub enum ComputeMessage {
    Toolpath(ComputeResult),
    Simulation(Result<SimulationResult, ComputeError>),
    Collision(Result<CollisionResult, ComputeError>),
}

pub trait ComputeBackend: Send {
    fn submit_toolpath(&mut self, request: ComputeRequest);
    fn submit_simulation(&mut self, request: SimulationRequest);
    fn submit_collision(&mut self, request: CollisionRequest);
    fn cancel_lane(&mut self, lane: ComputeLane);
    fn drain_results(&mut self) -> Vec<ComputeMessage>;
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot;

    fn cancel_all(&mut self) {
        self.cancel_lane(ComputeLane::Toolpath);
        self.cancel_lane(ComputeLane::Analysis);
    }

    fn lane_snapshots(&self) -> [LaneSnapshot; 2] {
        [
            self.lane_snapshot(ComputeLane::Toolpath),
            self.lane_snapshot(ComputeLane::Analysis),
        ]
    }
}
