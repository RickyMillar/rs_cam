//! Build script: capture git short-sha, dirty flag, and build timestamp
//! so the running binary can report exactly which commit it was built
//! from. Surfaced via `rs_cam_core::build_info`.

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let git_desc = if dirty {
        format!("{sha}-dirty")
    } else {
        sha
    };

    let build_ts = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    println!("cargo:rustc-env=RS_CAM_GIT_DESC={git_desc}");
    println!("cargo:rustc-env=RS_CAM_BUILD_TS={build_ts}");

    // Rerun when HEAD moves (commit/checkout). Uncommitted edits won't
    // retrigger automatically, but the `-dirty` flag from the last
    // build still flags that the tree was modified.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}
