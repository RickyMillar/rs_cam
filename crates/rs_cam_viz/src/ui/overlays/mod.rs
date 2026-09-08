//! The viewport Overlays panel and its registry (P6).
//!
//! `registry` is one declarative list of every viewport overlay. `panel` is
//! the egui surface built from that list. The MCP `set_ui_view` `overlays`
//! map and the completeness sentries read the same list, so the three
//! surfaces cannot disagree about what exists, what it is called, or why it
//! cannot draw.

pub mod panel;
pub mod registry;
