//! Build script: capture git short-sha, dirty flag, and the commit
//! timestamp so the running binary can report exactly which commit it
//! was built from. Surfaced via `rs_cam_core::build_info`.
//!
//! IMPORTANT — must produce DETERMINISTIC output for a given
//! (HEAD, dirty) state. cargo recompiles the crate whenever this
//! script's emitted `rustc-env` values change. An earlier version
//! embedded the *build* timestamp (`date -u`), which differed on every
//! run; combined with a `rerun-if-changed` pointing at a path that
//! resolves relative to the crate dir (where there is no `.git`, so
//! cargo treated it as perpetually changed), the script reran every
//! build and forced a full rs_cam_core → mcp → viz recompile each time.
//! Using the COMMIT timestamp (stable per HEAD) makes reruns idempotent.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn main() {
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());

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

    // Commit timestamp (stable per HEAD), NOT the build wall-clock — see
    // the module note above on why a volatile value breaks incremental
    // builds.
    let commit_ts =
        git(&["log", "-1", "--format=%cI"]).unwrap_or_else(|| "unknown".to_owned());

    println!("cargo:rustc-env=RS_CAM_GIT_DESC={git_desc}");
    println!("cargo:rustc-env=RS_CAM_BUILD_TS={commit_ts}");

    // Trigger reruns on real git state changes using ABSOLUTE paths
    // (the crate-relative default would miss the workspace-root .git).
    // A commit on the current branch updates the branch ref file but not
    // HEAD itself, so watch both.
    if let Some(git_dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        if let Some(ref_name) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={git_dir}/{ref_name}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
