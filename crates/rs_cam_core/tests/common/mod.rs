//! Shared helpers for the `rs_cam_core` integration tests.
//!
//! Each integration test file is its own crate, so common fixtures were
//! historically copy-pasted between them. This module holds the helpers
//! that were genuinely byte-identical across files; include it with
//! `mod common;` and import what you need. (Helpers that have drifted
//! into per-test variants — e.g. the F-### terrain/pocket session
//! builders — deliberately stay local to their test file so each
//! regression sentry keeps exercising exactly its own fixture.)
//!
//! `dead_code` is allowed because every consuming test binary only uses
//! a subset of these helpers.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};

/// Absolute path to the workspace root (two levels up from the crate
/// manifest dir), canonicalized.
pub fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves")
}

/// Canonical 6 mm 2-flute flat end mill used across the F-024/F-035 and
/// lift-bridge pocket sentries (45 mm stickout, 6.35 mm shank).
pub fn make_endmill_6mm() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    tool
}
