//! The one place the GUI stamps `stale_since` from a core answer.
//!
//! A core mutation reports `Effects::stale` — every toolpath index whose
//! generation-input revision moved. The GUI's auto-regeneration sweep
//! reads `ToolpathRuntime::stale_since`, not that answer. So a caller
//! that stamps ONE index leaves every other index core dropped green on
//! screen, and the sweep never queues it.
//!
//! WP8 found four such call sites at once. Each stamped the edited
//! toolpath alone, and the undo door had just widened to drop the whole
//! downstream chain.
//!
//! It is a free function over [`AppState`], not a method on the
//! controller, so a draw site can call it too (plan §19 ruling 4).
//!
//! WP19 (H3) merged the MCP surface's own helper into this one. That
//! helper skipped an index whose runtime row was absent, while this one
//! CREATES the row, so a never-drawn toolpath was stamped on one route
//! and not on the other. The stated reason for keeping the two apart —
//! that the MCP helper read the window rather than the state — was
//! false: it read `self.controller.state()` on both halves.

use std::collections::BTreeSet;

use super::AppState;
use super::runtime::ToolpathRuntime;

/// Stamp `stale_since` on the runtime row of every index the set names.
///
/// Each index resolves to a `ToolpathId` against the session as it
/// stands AFTER the mutation. An index the session no longer carries is
/// skipped: a removal reports the index as stale, and no runtime row is
/// left to stamp.
///
/// A row the operator has never drawn is CREATED, and it is created
/// with the operation's own `default_auto_regen`. The created row joins
/// `AppController::process_auto_regen`, which reads `rt.auto_regen`.
/// Twelve of the twenty-four operations declare `false`, so a row
/// hardcoded to `true` would queue a 3D finishing pass nobody asked
/// for. Every other row creator reads the catalog; WP19 made this one
/// read it too.
pub fn stamp_stale(state: &mut AppState, stale: &BTreeSet<usize>) {
    let now = std::time::Instant::now();
    let rows: Vec<(rs_cam_core::ToolpathId, bool)> = stale
        .iter()
        .filter_map(|&index| {
            state
                .session
                .get_toolpath_config(index)
                .map(|tc| (tc.id, tc.operation.default_auto_regen()))
        })
        .collect();
    for (id, auto_regen) in rows {
        state
            .gui
            .toolpath_rt
            .entry(id)
            .or_insert_with(|| ToolpathRuntime::new(auto_regen))
            .stale_since = Some(now);
    }
}
