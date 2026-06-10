//! Modal state tracked by `program_builder` while constructing a `Program`.
//!
//! Captures the controller-state book-keeping the legacy emitter did
//! inline: last commanded feed (for F-elision), current spindle RPM
//! (for emitting `M3 S<rpm>` only on change), current tool identity (for
//! tool-change sequencing), and current coolant mode (for M9 / restart
//! timing).
//!
//! `current_tool` is keyed on the tool *config id* (`PhaseTool::id`),
//! not the user-curated display T-number — real projects carry
//! colliding T-numbers across distinct tools (WANAKA: "End Mill" and
//! "Tapered Ball 2mm" both T1), which silently suppressed the change.
//!
//! `prev_pos` tracks the previous move's target so rapid moves can be
//! split into safe Z-first / XY-then-Z sequences instead of a single
//! diagonal `G0 X Y Z` through unknown space. `None` means "machine
//! position unknown" (program start, or just after a tool change / M0
//! pause where the operator may have jogged).

use super::CoolantMode;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ModalState {
    pub last_feed: Option<f64>,
    pub current_rpm: u32,
    /// Identity of the loaded tool: the tool *config id*
    /// (`PhaseTool::id`), not the display T-number.
    pub current_tool: Option<usize>,
    pub current_coolant: CoolantMode,
    /// Target of the previous move (any type), used to sequence rapids
    /// safely. `None` = machine position unknown.
    pub prev_pos: Option<(f64, f64, f64)>,
}

impl ModalState {
    pub fn new(rpm: u32, tool: Option<usize>, coolant: CoolantMode) -> Self {
        Self {
            last_feed: None,
            current_rpm: rpm,
            current_tool: tool,
            current_coolant: coolant,
            prev_pos: None,
        }
    }

    /// Reset the F-elision tracker. Called after any non-feed write
    /// that breaks modal continuity (rapids, tool changes, setup boundaries).
    pub fn reset_feed(&mut self) {
        self.last_feed = None;
    }

    /// Forget the tracked machine position. Called after tool changes
    /// and M0 pauses, where the operator may have jogged the machine —
    /// the next rapid is sequenced Z-first like a program start.
    pub fn reset_position(&mut self) {
        self.prev_pos = None;
    }
}
