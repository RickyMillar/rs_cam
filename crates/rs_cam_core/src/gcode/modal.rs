//! Modal state tracked by `program_builder` while constructing a `Program`.
//!
//! Captures the controller-state book-keeping the legacy emitter did
//! inline: last commanded feed (for F-elision), current spindle RPM
//! (for emitting `M3 S<rpm>` only on change), current tool number (for
//! M6 sequencing), and current coolant mode (for M9 / restart timing).
//!
//! Phase 3 will likely grow this struct with motion-mode tracking
//! (G0 vs G1) and units; for Phase 2 it stays scoped to the existing
//! emitter's elision rules so the IR refactor is byte-identical.

use super::CoolantMode;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ModalState {
    pub last_feed: Option<f64>,
    pub current_rpm: u32,
    pub current_tool: Option<u32>,
    pub current_coolant: CoolantMode,
}

impl ModalState {
    pub fn new(rpm: u32, tool: Option<u32>, coolant: CoolantMode) -> Self {
        Self {
            last_feed: None,
            current_rpm: rpm,
            current_tool: tool,
            current_coolant: coolant,
        }
    }

    /// Reset the F-elision tracker. Called after any non-feed write
    /// that breaks modal continuity (rapids, tool changes, setup boundaries).
    pub fn reset_feed(&mut self) {
        self.last_feed = None;
    }
}
