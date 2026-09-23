//! The viewport overlay registry, the All viewport options catalogue and the
//! legend rail.
//!
//! `registry` is one declarative list of every viewport overlay. `panel` is
//! the catalogue built from that list, and the row renderer that the viewport
//! dock (`crate::ui::viewport_overlay`) shares with it. `legend_rail` draws
//! one block per active colour encoding above the dock. The MCP
//! `set_ui_view` `overlays` map and the completeness sentries read the same
//! list, so the surfaces cannot disagree about what exists, what it is
//! called, or why it cannot draw.

pub mod legend_rail;
pub mod panel;
pub mod registry;
