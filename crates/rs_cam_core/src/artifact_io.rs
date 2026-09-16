//! One home for the JSON artifact dumps that core writes to disk.
//!
//! Three writers had grown the same body: create the directory, stamp a file
//! name, write pretty JSON. Only the cut-trace writer carried the collision
//! fix, so the trace writer could still hand two artifacts one path. The
//! shared writer gives every artifact the unique name and keeps the prune
//! contract of [`crate::simulation_cut::prune_simulation_cut_artifacts`]:
//! the first `_`-field of the name stays the millisecond stamp.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-process sequence number for artifact file names. One counter serves
/// every writer, so two artifacts of different kinds cannot collide either.
static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Make `input` safe as one file-name component.
///
/// ASCII alphanumerics become lower case, `-` and `_` survive, every other
/// character becomes `_`, and leading or trailing `_` are trimmed. An empty
/// result falls back to `fallback`, which names the artifact kind.
pub(crate) fn sanitize_filename_component(input: &str, fallback: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        fallback.to_owned()
    } else {
        output.to_owned()
    }
}

/// Write `artifact` into `dir` as pretty JSON and return the path.
///
/// The name is `{timestamp_ms}_{pid}-{seq}_{stem}.json`. A millisecond stamp
/// alone is NOT unique: two writers in the same process (parallel tests; a
/// busy worker) can finish in the same millisecond, silently share one path,
/// and then one owner's cleanup deletes the other's artifact. Pid + sequence
/// make the name unique across concurrent sessions too;
/// `prune_simulation_cut_artifacts` reads only the first `_`-field, so its
/// age parse is unaffected.
pub(crate) fn write_json_artifact(
    dir: &Path,
    file_stem: &str,
    fallback: &str,
    artifact: &impl Serialize,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = format!(
        "{}_{}-{}_{}.json",
        timestamp_ms,
        std::process::id(),
        seq,
        sanitize_filename_component(file_stem, fallback)
    );
    let path = dir.join(file_name);
    let payload = serde_json::to_vec_pretty(artifact)?;
    std::fs::write(&path, payload)?;
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Each artifact kind keeps its own empty-input fallback. The three
    /// writers used to carry one literal each in three copies of this
    /// function; the literal is now the caller's argument.
    #[test]
    fn an_empty_component_falls_back_to_the_artifact_kind() {
        assert_eq!(
            sanitize_filename_component("", "toolpath_debug"),
            "toolpath_debug"
        );
        assert_eq!(
            sanitize_filename_component("___", "toolpath_trace"),
            "toolpath_trace"
        );
        assert_eq!(
            sanitize_filename_component("", "simulation_cut_trace"),
            "simulation_cut_trace"
        );
    }

    #[test]
    fn two_writes_in_the_same_millisecond_get_different_paths() {
        let dir = std::env::temp_dir().join(format!(
            "rs_cam_artifact_io_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos()
        ));
        let first = write_json_artifact(&dir, "Adaptive 3D", "toolpath_trace", &42_u32)
            .expect("write first artifact");
        let second = write_json_artifact(&dir, "Adaptive 3D", "toolpath_trace", &42_u32)
            .expect("write second artifact");
        assert_ne!(first, second, "artifact paths must be unique per write");
        std::fs::remove_file(first).ok();
        std::fs::remove_file(second).ok();
        std::fs::remove_dir(dir).ok();
    }
}
