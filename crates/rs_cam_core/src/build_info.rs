//! Build identification: git commit, dirty flag, build timestamp, and
//! crate version, captured at compile time by `build.rs`.
//!
//! Lets a running binary report exactly which commit it was built from,
//! so an operator or agent can confirm whether a feature they expect is
//! present rather than guessing from output. Surfaced in the GUI title
//! bar, `rs_cam_cli --version`, and the MCP `build_info` block.

/// Git short-sha of the build, with a `-dirty` suffix when the working
/// tree had uncommitted changes at build time. `"unknown"` if git was
/// unavailable.
pub const GIT_DESC: &str = env!("RS_CAM_GIT_DESC");

/// UTC build timestamp (RFC-3339-ish, `YYYY-MM-DDТHH:MM:SSZ`).
pub const BUILD_TIMESTAMP: &str = env!("RS_CAM_BUILD_TS");

/// `rs_cam_core` semver from its Cargo manifest.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One-line human summary, e.g. `rs_cam_core 0.1.0 (3f9a1c2-dirty, built 2026-05-28T20:10:00Z)`.
pub fn summary() -> String {
    format!(
        "rs_cam_core {CORE_VERSION} ({GIT_DESC}, built {BUILD_TIMESTAMP})"
    )
}
