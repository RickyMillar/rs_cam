//! Reusable machine-profile library — a catalog you import *from*.
//!
//! Machines live as standalone TOML files in a per-user library
//! directory. Like the tool library ([`crate::tool_library`]), this uses
//! **snapshot** semantics: importing a machine COPIES it into the
//! project's inline `[job.machine]`, which is then authoritative. Later
//! edits to a library file never reach existing projects — re-import to
//! pick up a change. There is no live `machine_ref` link (a legacy one is
//! dropped on load and migrated into the inline copy).
//!
//! Directory resolution (first that is set wins):
//! 1. `$RS_CAM_MACHINE_DIR` — explicit override (used by tests / CI).
//! 2. `$XDG_CONFIG_HOME/rs_cam/machines`.
//! 3. `$HOME/.config/rs_cam/machines`.

use std::path::{Path, PathBuf};

use crate::machine::MachineProfile;

/// Errors from machine-library file operations.
#[derive(Debug, thiserror::Error)]
pub enum MachineLibraryError {
    #[error(
        "machine library directory could not be determined (no RS_CAM_MACHINE_DIR, XDG_CONFIG_HOME, or HOME)"
    )]
    NoLibraryDir,
    #[error("invalid machine name {0:?}: must be non-empty and contain no path separators or '..'")]
    InvalidName(String),
    #[error("machine {0:?} not found in the library")]
    NotFound(String),
    #[error("i/o error for machine {name:?}: {source}")]
    Io {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse machine {0:?}: {1}")]
    Parse(String, toml::de::Error),
    #[error("failed to serialize machine {0:?}: {1}")]
    Serialize(String, toml::ser::Error),
}

/// Resolve the per-user machine-library directory. Does not create it.
pub fn library_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("RS_CAM_MACHINE_DIR")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("rs_cam").join("machines"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("rs_cam")
                .join("machines"),
        );
    }
    None
}

/// True when `name` is a safe bare file stem (no separators, no `..`).
fn name_is_valid(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != ".."
        && !name.contains("..")
}

/// Path to the `.toml` for `name` inside `dir`.
fn path_in(dir: &Path, name: &str) -> Result<PathBuf, MachineLibraryError> {
    if !name_is_valid(name) {
        return Err(MachineLibraryError::InvalidName(name.to_owned()));
    }
    Ok(dir.join(format!("{name}.toml")))
}

/// Path to the `.toml` for `name` in the resolved library dir.
pub fn path_for(name: &str) -> Result<PathBuf, MachineLibraryError> {
    let dir = library_dir().ok_or(MachineLibraryError::NoLibraryDir)?;
    path_in(&dir, name)
}

/// List the machine names (file stems) available in `dir`. Missing dir
/// returns an empty list (not an error).
pub fn list_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                return None;
            }
            path.file_stem().and_then(|s| s.to_str()).map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// List the machine names available in the resolved library dir.
pub fn list() -> Vec<String> {
    library_dir().map(|d| list_in(&d)).unwrap_or_default()
}

/// Load a machine profile by name from `dir`.
pub fn load_from(dir: &Path, name: &str) -> Result<MachineProfile, MachineLibraryError> {
    let path = path_in(dir, name)?;
    if !path.exists() {
        return Err(MachineLibraryError::NotFound(name.to_owned()));
    }
    let text = std::fs::read_to_string(&path).map_err(|source| MachineLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    toml::from_str(&text).map_err(|e| MachineLibraryError::Parse(name.to_owned(), e))
}

/// Load a machine profile by name from the resolved library dir.
pub fn load(name: &str) -> Result<MachineProfile, MachineLibraryError> {
    let dir = library_dir().ok_or(MachineLibraryError::NoLibraryDir)?;
    load_from(&dir, name)
}

/// Save a machine profile under `name` into `dir`, creating the dir.
pub fn save_to(
    dir: &Path,
    name: &str,
    profile: &MachineProfile,
) -> Result<PathBuf, MachineLibraryError> {
    let path = path_in(dir, name)?;
    std::fs::create_dir_all(dir).map_err(|source| MachineLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    let text = toml::to_string_pretty(profile)
        .map_err(|e| MachineLibraryError::Serialize(name.to_owned(), e))?;
    std::fs::write(&path, text).map_err(|source| MachineLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    Ok(path)
}

/// Save a machine profile under `name` into the resolved library dir.
pub fn save(name: &str, profile: &MachineProfile) -> Result<PathBuf, MachineLibraryError> {
    let dir = library_dir().ok_or(MachineLibraryError::NoLibraryDir)?;
    save_to(&dir, name, profile)
}

// NOTE: machines use SNAPSHOT semantics (like `[[tools]]`) — the project's
// inline `[job.machine]` is authoritative. There is intentionally no
// `resolve()` override: importing a machine from this library COPIES it into
// the project (see the GUI Machine panel / MCP `load_machine_from_library`),
// and later edits to a library file never reach existing projects. The old
// live-reference `resolve`/`resolve_in` were removed when the model switched
// to snapshot (legacy `machine_ref` is now dropped on load).

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs_cam_machine_lib_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = temp_dir("round_trip");
        let mut m = MachineProfile::generic_wood_router();
        m.name = "Test Router".to_owned();
        m.max_feed_mm_min = 9999.0;
        save_to(&dir, "test_router", &m).unwrap();
        let loaded = load_from(&dir, "test_router").unwrap();
        assert_eq!(loaded.name, "Test Router");
        assert_eq!(loaded.max_feed_mm_min, 9999.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_returns_sorted_stems() {
        let dir = temp_dir("list");
        save_to(&dir, "b_machine", &MachineProfile::generic_wood_router()).unwrap();
        save_to(&dir, "a_machine", &MachineProfile::generic_wood_router()).unwrap();
        assert_eq!(list_in(&dir), vec!["a_machine", "b_machine"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_names_rejected() {
        let dir = temp_dir("invalid");
        assert!(matches!(
            save_to(&dir, "../escape", &MachineProfile::generic_wood_router()),
            Err(MachineLibraryError::InvalidName(_))
        ));
        assert!(matches!(
            path_in(&dir, "a/b"),
            Err(MachineLibraryError::InvalidName(_))
        ));
        assert!(matches!(
            path_in(&dir, ""),
            Err(MachineLibraryError::InvalidName(_))
        ));
    }
}
