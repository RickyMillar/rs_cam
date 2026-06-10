//! Strongly-typed identifiers shared across the CAM data model.
//!
//! R3 (tech-debt review 2026-06-10): toolpath *ids* and *indexes into
//! `session.toolpath_configs`* used to unify as bare `usize`, which made
//! `toolpath_id == index`-shaped comparisons compile silently (the WANAKA
//! "Back Rough is index 1 but id 4" wrong-lookup class). [`ToolpathId`]
//! exists so the compiler surfaces every such confusion. Indexes stay
//! `usize` on purpose — do not newtype them.

use serde::{Deserialize, Serialize};

/// Stable identity of a toolpath (`ToolpathConfig.id`).
///
/// This is *not* an index into `session.toolpath_configs`: configs can be
/// reordered or deleted, so id and position diverge. Map between them via
/// `toolpath_configs[index].id` or
/// `toolpath_configs.iter().position(|tc| tc.id == id)`.
///
/// `#[serde(transparent)]` keeps the wire shape identical to a bare
/// integer — SimulationCutTrace JSON dumps, project TOML files, and MCP
/// payloads are byte-compatible with the pre-newtype format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolpathId(pub usize);

impl std::fmt::Display for ToolpathId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
