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
//!
//! # Layout and why it is a directory, not a test target
//!
//! Cargo builds one test binary per `.rs` file *directly* under `tests/`.
//! `tests/common/` is a directory whose `mod.rs` is pulled in with a plain
//! `mod common;`, so it compiles into each consuming binary and is never a
//! target of its own. That is the stock Rust integration-test convention and
//! it is already what 13 files in this suite do, so C6 grew it rather than
//! introducing a second mechanism. (The `#[path]` include style is used
//! elsewhere — `literature_matrix/` — but only to split ONE harness's private
//! internals across files, which is a different job.)
//!
//! # Submodules (C6, the shared fixture library)
//!
//! | module | what it holds |
//! |---|---|
//! | [`adversarial2d`] | R2's hostile 2D fixtures, their non-vacuity measures, the wall-clock/RSS watchdog, the cancellation seam, and the SVG renderers |
//! | [`bandmap`] | the finish-planner band-territory `BandMap` + deviation-histogram instrument (stock-mesh-vertex-deviation lineage — see its module doc for why this is NOT the `column_deviations`/COLUMNS lineage h4 uses) |
//! | [`chain`] | the F.4 generate/simulate fixpoint ladder + measurement-resolution re-sim, shared by wanaka-chain harnesses |
//! | [`meshes`] | synthetic mesh generators: plateau, grooved block, sawtooth plate, height fields, profile extrusion |
//! | [`offset_lab`] | M5's 2D-offset bench: fixtures, cascade runner, per-ring attribution, erosion oracle |
//! | [`reference_plate`] | **ARP-1**, the analytic reference plate: 16 non-blending closed-form zones with exact normals, curvatures, band areas and tool-reach floors, tessellated per zone in its own natural parameter |
//! | [`tools`] | the shipped taper / ball control as both `MillingCutter` shapes and `ToolConfig` records |
//! | [`session`] | `LoadedModel` / `StockConfig` / the 17-field `ToolpathConfig` / one-op `ProjectSession` builders |
//! | [`fingerprint`] | the single canonical FNV-1a-over-`Debug` toolpath fingerprint |
//! | [`scallop_oracle`] | M4's analytic tool-envelope surface scorer + its ground-truth validation helpers |
//!
//! # Migration policy
//!
//! **New tests import from here. Existing tests migrate opportunistically,
//! never in bulk.** A sentry's value is that it has not changed; rewriting
//! working sentries to use a new helper spends that value for tidiness. Move
//! a test over when you are editing it anyway — and when you do, its
//! assertions and any pinned constants must come through untouched.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod adversarial2d;
pub mod bandmap;
pub mod chain;
pub mod fingerprint;
pub mod meshes;
pub mod offset_lab;
pub mod reference_plate;
pub mod scallop_oracle;
pub mod session;
pub mod tools;

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
