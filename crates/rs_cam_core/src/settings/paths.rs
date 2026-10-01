//! Where the app keeps its files: the config folder, the settings file, the
//! two libraries, the cache folder for diagnostic artifacts.
//!
//! Every resolver has a pure form that takes an environment lookup (and the
//! settings-file value when there is one), so a test can give it any
//! environment. The tests never write the process environment:
//! `std::env::set_var` is `unsafe` in edition 2024.
//!
//! The order for a library folder is:
//!
//! 1. the environment override (`RS_CAM_TOOL_DIR`, `RS_CAM_MACHINE_DIR`);
//! 2. the `[paths]` value of the settings file;
//! 3. the default: `<config folder>/tools` or `<config folder>/machines`.
//!
//! The config folder is `XDG_CONFIG_HOME/rs_cam`, else `HOME/.config/rs_cam`,
//! else `APPDATA/rs_cam`, else `USERPROFILE/.config/rs_cam`. The GUI, MCP and
//! the CLI all read a library through `crate::io::tool_library::library_dir`
//! and `crate::io::machine_library::library_dir`, which call
//! [`tool_library_dir`] and [`machine_library_dir`] here. So every surface
//! resolves the SAME folder.
//!
//! The `[paths]` values of this process live in one cell. The first read
//! loads the settings file; [`install_paths`] replaces the cell when
//! File ▸ Preferences applies new values, so the next library read uses
//! them.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::PathSettings;

/// The environment variable that names the settings FILE, not a directory.
pub const SETTINGS_ENV: &str = "RS_CAM_SETTINGS";

/// The environment variable that overrides the tool-library folder.
pub const TOOL_DIR_ENV: &str = "RS_CAM_TOOL_DIR";

/// The environment variable that overrides the machine-library folder.
pub const MACHINE_DIR_ENV: &str = "RS_CAM_MACHINE_DIR";

/// The file name inside the `rs_cam` config folder.
pub(super) const SETTINGS_FILE: &str = "settings.toml";

/// The folder name of the app inside a config or cache root.
const APP_DIR: &str = "rs_cam";

/// The tool-library folder name inside the config folder.
const TOOLS_DIR: &str = "tools";

/// The machine-library folder name inside the config folder.
const MACHINES_DIR: &str = "machines";

/// The artifact folder name inside the cache folder.
const ARTIFACTS_DIR: &str = "artifacts";

/// The value of `name`, or `None` when it is not set or is empty.
fn non_empty(env: &impl Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    env(name).filter(|value| !value.is_empty())
}

/// The `rs_cam` config folder from an environment lookup. Pure.
#[must_use]
pub fn config_dir_from(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(xdg) = non_empty(&env, "XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join(APP_DIR));
    }
    if let Some(home) = non_empty(&env, "HOME") {
        return Some(PathBuf::from(home).join(".config").join(APP_DIR));
    }
    if let Some(appdata) = non_empty(&env, "APPDATA") {
        return Some(PathBuf::from(appdata).join(APP_DIR));
    }
    let profile = non_empty(&env, "USERPROFILE")?;
    Some(PathBuf::from(profile).join(".config").join(APP_DIR))
}

/// The `rs_cam` cache folder from an environment lookup. Pure.
///
/// `XDG_CACHE_HOME/rs_cam`, else `HOME/.cache/rs_cam`, else
/// `LOCALAPPDATA/rs_cam` (Windows keeps a cache on the local machine, not
/// in the roaming `APPDATA`), else `USERPROFILE/.cache/rs_cam`.
#[must_use]
pub fn cache_dir_from(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(xdg) = non_empty(&env, "XDG_CACHE_HOME") {
        return Some(PathBuf::from(xdg).join(APP_DIR));
    }
    if let Some(home) = non_empty(&env, "HOME") {
        return Some(PathBuf::from(home).join(".cache").join(APP_DIR));
    }
    if let Some(local) = non_empty(&env, "LOCALAPPDATA") {
        return Some(PathBuf::from(local).join(APP_DIR));
    }
    let profile = non_empty(&env, "USERPROFILE")?;
    Some(PathBuf::from(profile).join(".cache").join(APP_DIR))
}

/// The settings file path from an environment lookup. Pure.
///
/// `RS_CAM_SETTINGS` names the file; else `<config folder>/settings.toml`.
#[must_use]
pub fn settings_path_from(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(file) = non_empty(&env, SETTINGS_ENV) {
        return Some(PathBuf::from(file));
    }
    config_dir_from(env).map(|dir| dir.join(SETTINGS_FILE))
}

/// One library folder: the environment override `env_var`, else the
/// settings-file value `file`, else `<config folder>/<default_name>`. Pure.
fn library_dir_from(
    env: impl Fn(&str) -> Option<String>,
    env_var: &str,
    file: Option<&Path>,
    default_name: &str,
) -> Option<PathBuf> {
    if let Some(dir) = non_empty(&env, env_var) {
        return Some(PathBuf::from(dir));
    }
    if let Some(dir) = file {
        return Some(dir.to_path_buf());
    }
    config_dir_from(env).map(|dir| dir.join(default_name))
}

/// The tool-library folder: `RS_CAM_TOOL_DIR`, else `file`, else
/// `<config folder>/tools`. Pure.
#[must_use]
pub fn tool_library_dir_from(
    env: impl Fn(&str) -> Option<String>,
    file: Option<&Path>,
) -> Option<PathBuf> {
    library_dir_from(env, TOOL_DIR_ENV, file, TOOLS_DIR)
}

/// The machine-library folder: `RS_CAM_MACHINE_DIR`, else `file`, else
/// `<config folder>/machines`. Pure.
#[must_use]
pub fn machine_library_dir_from(
    env: impl Fn(&str) -> Option<String>,
    file: Option<&Path>,
) -> Option<PathBuf> {
    library_dir_from(env, MACHINE_DIR_ENV, file, MACHINES_DIR)
}

/// The diagnostic artifact folder: the settings-file value `file`, else
/// `<cache folder>/artifacts`. Pure.
///
/// The GUI writes `simulation_metrics/` and `toolpath_debug/` inside it.
/// Before 2026-10-02 the folder was the build tree's `target/`, a path that
/// the compiler fixed and that a packaged app does not have.
#[must_use]
pub fn artifact_dir_from(
    env: impl Fn(&str) -> Option<String>,
    file: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(dir) = file {
        return Some(dir.to_path_buf());
    }
    cache_dir_from(env).map(|dir| dir.join(ARTIFACTS_DIR))
}

/// The process environment, as a lookup.
fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The settings file path of this process. Does not create it.
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    settings_path_from(process_env)
}

/// The `[paths]` values of this process. `None` until the first read.
static PATHS: RwLock<Option<PathSettings>> = RwLock::new(None);

/// Replace the `[paths]` values of this process. File ▸ Preferences calls
/// it after it writes the file, so the next library read uses the new
/// folders without a restart.
pub fn install_paths(paths: &PathSettings) {
    match PATHS.write() {
        Ok(mut slot) => *slot = Some(paths.clone()),
        // A poisoned lock still holds a valid value: a writer stores a
        // whole `Option` in one assignment.
        Err(poisoned) => *poisoned.into_inner() = Some(paths.clone()),
    }
}

/// The `[paths]` values of this process. The first call reads the settings
/// file. A file that does not read gives the defaults (no values).
#[must_use]
pub fn installed_paths() -> PathSettings {
    let held = match PATHS.read() {
        Ok(slot) => slot.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    if let Some(paths) = held {
        return paths;
    }
    let paths = super::load().settings.paths;
    install_paths(&paths);
    paths
}

/// The tool-library folder of this process. Does not create it.
#[must_use]
pub fn tool_library_dir() -> Option<PathBuf> {
    let paths = installed_paths();
    tool_library_dir_from(process_env, paths.tool_library.as_deref())
}

/// The machine-library folder of this process. Does not create it.
#[must_use]
pub fn machine_library_dir() -> Option<PathBuf> {
    let paths = installed_paths();
    machine_library_dir_from(process_env, paths.machine_library.as_deref())
}

/// The artifact folder for the diagnostic settings `file`, in this process.
#[must_use]
pub fn artifact_dir(file: Option<&Path>) -> Option<PathBuf> {
    artifact_dir_from(process_env, file)
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap is a failed test.
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// An environment lookup over fixed pairs.
    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn the_settings_path_resolves_with_a_windows_fallback() {
        assert_eq!(
            settings_path_from(env(&[(SETTINGS_ENV, "/x/s.toml"), ("HOME", "/h")])),
            Some(PathBuf::from("/x/s.toml"))
        );
        assert_eq!(
            settings_path_from(env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/h")])),
            Some(PathBuf::from("/xdg").join("rs_cam").join("settings.toml"))
        );
        assert_eq!(
            settings_path_from(env(&[("HOME", "/h")])),
            Some(
                PathBuf::from("/h")
                    .join(".config")
                    .join("rs_cam")
                    .join("settings.toml")
            )
        );
        assert_eq!(
            settings_path_from(env(&[("HOME", ""), ("APPDATA", "C:/Users/u/AppData")])),
            Some(
                PathBuf::from("C:/Users/u/AppData")
                    .join("rs_cam")
                    .join("settings.toml")
            )
        );
        assert_eq!(
            settings_path_from(env(&[("USERPROFILE", "C:/Users/u")])),
            Some(
                PathBuf::from("C:/Users/u")
                    .join(".config")
                    .join("rs_cam")
                    .join("settings.toml")
            )
        );
        assert_eq!(settings_path_from(env(&[])), None);
    }

    /// The library order: the environment variable, then the settings
    /// file, then the default under the config folder. The Windows fallback
    /// of the settings file applies to the libraries too.
    #[test]
    fn a_library_folder_is_env_then_file_then_default() {
        let file = Path::new("/from/file/tools");
        let pairs = [(TOOL_DIR_ENV, "/from/env"), ("HOME", "/h")];
        assert_eq!(
            tool_library_dir_from(env(&pairs), Some(file)),
            Some(PathBuf::from("/from/env")),
            "the environment variable wins"
        );
        let pairs = [(TOOL_DIR_ENV, ""), ("HOME", "/h")];
        assert_eq!(
            tool_library_dir_from(env(&pairs), Some(file)),
            Some(file.to_path_buf()),
            "an empty variable is not set; the file is next"
        );
        assert_eq!(
            tool_library_dir_from(env(&[("HOME", "/h")]), None),
            Some(PathBuf::from("/h").join(".config").join("rs_cam").join("tools"))
        );
        assert_eq!(
            tool_library_dir_from(env(&[("APPDATA", "C:/A")]), None),
            Some(PathBuf::from("C:/A").join("rs_cam").join("tools"))
        );
        assert_eq!(tool_library_dir_from(env(&[]), None), None);

        let machines = Path::new("/from/file/machines");
        assert_eq!(
            machine_library_dir_from(
                env(&[(MACHINE_DIR_ENV, "/m/env"), ("HOME", "/h")]),
                Some(machines)
            ),
            Some(PathBuf::from("/m/env"))
        );
        assert_eq!(
            machine_library_dir_from(env(&[("HOME", "/h")]), Some(machines)),
            Some(machines.to_path_buf())
        );
        assert_eq!(
            machine_library_dir_from(env(&[("XDG_CONFIG_HOME", "/xdg")]), None),
            Some(PathBuf::from("/xdg").join("rs_cam").join("machines"))
        );
        // The tool variable does not move the machine folder.
        assert_eq!(
            machine_library_dir_from(env(&[(TOOL_DIR_ENV, "/t"), ("HOME", "/h")]), None),
            Some(
                PathBuf::from("/h")
                    .join(".config")
                    .join("rs_cam")
                    .join("machines")
            )
        );
    }

    /// The artifact folder is per user, never the build tree.
    #[test]
    fn the_artifact_folder_is_the_file_value_else_the_user_cache() {
        assert_eq!(
            artifact_dir_from(env(&[("HOME", "/h")]), Some(Path::new("/a"))),
            Some(PathBuf::from("/a"))
        );
        assert_eq!(
            artifact_dir_from(env(&[("XDG_CACHE_HOME", "/c"), ("HOME", "/h")]), None),
            Some(PathBuf::from("/c").join("rs_cam").join("artifacts"))
        );
        assert_eq!(
            artifact_dir_from(env(&[("HOME", "/h")]), None),
            Some(
                PathBuf::from("/h")
                    .join(".cache")
                    .join("rs_cam")
                    .join("artifacts")
            )
        );
        assert_eq!(
            artifact_dir_from(env(&[("LOCALAPPDATA", "C:/L"), ("APPDATA", "C:/R")]), None),
            Some(PathBuf::from("C:/L").join("rs_cam").join("artifacts"))
        );
        assert_eq!(
            artifact_dir_from(env(&[("USERPROFILE", "C:/U")]), None),
            Some(
                PathBuf::from("C:/U")
                    .join(".cache")
                    .join("rs_cam")
                    .join("artifacts")
            )
        );
        assert_eq!(artifact_dir_from(env(&[]), None), None);
    }
}
