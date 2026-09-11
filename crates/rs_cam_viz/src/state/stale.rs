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
//! downstream chain. Two precedents already did the walk by hand:
//! `apply_tool_snapshot` (`controller/events/undo.rs`) iterated
//! `invalidate_tool(..).stale`, and `mcp_stamp_stale` (`app/mcp.rs`)
//! does the same for the MCP surface. This is the viz half of the pair.
//!
//! It is a free function over [`AppState`], not a method on the
//! controller, so a draw site can call it too (plan §19 ruling 4). The
//! MCP helper is NOT merged into this one yet; it reads the window, not
//! the state.

use std::collections::BTreeSet;

use super::AppState;

/// Stamp `stale_since` on the runtime row of every index the set names.
///
/// Each index resolves to a `ToolpathId` against the session as it
/// stands AFTER the mutation. An index the session no longer carries is
/// skipped: a removal reports the index as stale, and no runtime row is
/// left to stamp.
pub fn stamp_stale(state: &mut AppState, stale: &BTreeSet<usize>) {
    let now = std::time::Instant::now();
    let ids: Vec<rs_cam_core::ToolpathId> = stale
        .iter()
        .filter_map(|&index| state.session.get_toolpath_config(index).map(|tc| tc.id))
        .collect();
    for id in ids {
        state.gui.toolpath_rt_or_default(id).stale_since = Some(now);
    }
}
