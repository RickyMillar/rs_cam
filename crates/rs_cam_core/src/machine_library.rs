//! Reusable machine-profile library.
//!
//! Machines live as standalone TOML files in a per-user library
//! directory; projects reference one by name (`machine_ref`) instead of
//! embedding the full definition. The library is the single source of
//! truth — editing `shapeoko_pro_xxl.toml` updates every project that
//! references it on next load.
//!
//! A project keeps an inline `[job.machine]` copy as an offline fallback:
//! [`resolve`] prefers the library file when present and falls back to
//! the inline copy (with a warning) when the referenced file is missing,
//! so a project never fails to open just because the library moved.
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

/// Outcome of resolving a project's machine: the profile to use plus an
/// optional human-readable warning (e.g. the referenced file was
/// missing and the inline fallback was used).
pub struct Resolved {
    pub profile: MachineProfile,
    pub warning: Option<String>,
}

/// Resolve the active machine for a project given its `machine_ref` and
/// the inline `[job.machine]` copy, against `dir`.
///
/// - `machine_ref == None` → use the inline profile (no warning).
/// - `machine_ref == Some(name)` and the library file loads → use it.
/// - `machine_ref == Some(name)` but the file is missing/unparseable →
///   fall back to the inline profile and return a warning.
pub fn resolve_in(dir: &Path, machine_ref: Option<&str>, inline: MachineProfile) -> Resolved {
    let Some(name) = machine_ref else {
        return Resolved {
            profile: inline,
            warning: None,
        };
    };
    match load_from(dir, name) {
        // F4.4 — the library file is the source of truth, but when it
        // DIFFERS from the project's inline copy the override used to
        // be silent: the user saw inline numbers in the file while the
        // session ran different caps. Surface it.
        Ok(profile) => {
            let differs =
                serde_json::to_string(&profile).ok() != serde_json::to_string(&inline).ok();
            Resolved {
                warning: differs.then(|| {
                    format!(
                        "machine reference {name:?}: library profile overrides the \
                         project's inline machine copy (they differ; the library file \
                         is the source of truth)"
                    )
                }),
                profile,
            }
        }
        Err(e) => Resolved {
            profile: inline,
            warning: Some(format!(
                "machine reference {name:?} could not be loaded ({e}); using the project's inline machine copy"
            )),
        },
    }
}

/// Resolve the active machine using the resolved library dir. If no
/// library dir can be determined, the inline profile is used and a
/// warning is returned when a `machine_ref` was set.
pub fn resolve(machine_ref: Option<&str>, inline: MachineProfile) -> Resolved {
    match (machine_ref, library_dir()) {
        (Some(_), Some(dir)) => resolve_in(&dir, machine_ref, inline),
        (None, _) => Resolved {
            profile: inline,
            warning: None,
        },
        (Some(name), None) => Resolved {
            profile: inline,
            warning: Some(format!(
                "machine reference {name:?} set but no library directory is available; using the project's inline machine copy"
            )),
        },
    }
}

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

    #[test]
    fn resolve_prefers_library_over_inline() {
        let dir = temp_dir("resolve_prefer");
        let mut lib = MachineProfile::generic_wood_router();
        lib.name = "From Library".to_owned();
        save_to(&dir, "shapeoko", &lib).unwrap();

        let mut inline = MachineProfile::generic_wood_router();
        inline.name = "Inline Stale".to_owned();

        let r = resolve_in(&dir, Some("shapeoko"), inline);
        assert_eq!(r.profile.name, "From Library");
        // F4.4 — the override is no longer silent: library != inline
        // must surface a warning naming the source of truth.
        let warning = r.warning.expect("differing override must warn");
        assert!(warning.contains("overrides"), "got: {warning}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F4.4 — when the library file and the inline copy agree, the
    /// resolve stays quiet (no warning noise on every load).
    #[test]
    fn resolve_identical_library_and_inline_is_silent() {
        let dir = temp_dir("resolve_identical");
        let lib = MachineProfile::generic_wood_router();
        save_to(&dir, "shapeoko", &lib).unwrap();

        let r = resolve_in(
            &dir,
            Some("shapeoko"),
            MachineProfile::generic_wood_router(),
        );
        assert!(r.warning.is_none(), "got: {:?}", r.warning);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_falls_back_to_inline_when_missing() {
        let dir = temp_dir("resolve_missing");
        let mut inline = MachineProfile::generic_wood_router();
        inline.name = "Inline Fallback".to_owned();

        let r = resolve_in(&dir, Some("does_not_exist"), inline);
        assert_eq!(r.profile.name, "Inline Fallback");
        assert!(r.warning.is_some());
    }

    #[test]
    fn resolve_none_uses_inline_without_warning() {
        let dir = temp_dir("resolve_none");
        let mut inline = MachineProfile::generic_wood_router();
        inline.name = "Inline Only".to_owned();
        let r = resolve_in(&dir, None, inline);
        assert_eq!(r.profile.name, "Inline Only");
        assert!(r.warning.is_none());
    }
}
