//! The app settings file: `~/.config/rs_cam/settings.toml` (B5).
//!
//! Before this file, the tool and machine libraries were the only files the
//! app read from `~/.config/rs_cam`. The settings file holds the memory
//! limit of `planning/memory_budget_2026-10-01/PLAN.md` ("Architecture" 6):
//!
//! ```toml
//! [memory]
//! limit = "12GiB"      # a binary size: B, KiB, MiB, GiB or TiB
//! # limit = 12884901888  (a bare integer is a byte count)
//! # limit = "unlimited"  (no limit, whatever the default is)
//! ```
//!
//! No `limit` key means the default: `MemoryLimit::Default`, which reads
//! `rs_cam_core::budget::DEFAULT_SYSTEM_FRACTION` (RULING PENDING).
//!
//! The path resolves like the libraries (`rs_cam_core::io::tool_library::
//! library_dir`, `rs_cam_core::io::machine_library::library_dir`): the
//! environment override first, then `XDG_CONFIG_HOME`, then `HOME`. When
//! `HOME` is not set (Windows), `APPDATA`, then `USERPROFILE`.
//!
//! A missing file gives the defaults and is never an error. A file that
//! does not read or parse also gives the defaults, plus a warning for the
//! caller to show.

use std::path::{Path, PathBuf};

use rs_cam_core::budget::{MemoryBudget, MemoryLimit, parse_byte_size};
use serde::Deserialize;

/// The environment variable that names the settings FILE, not a directory.
pub const SETTINGS_ENV: &str = "RS_CAM_SETTINGS";

/// The file name inside the `rs_cam` config directory.
const SETTINGS_FILE: &str = "settings.toml";

/// The text of `[memory] limit` that asks for no limit.
const UNLIMITED: &str = "unlimited";

/// The settings the app reads at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppSettings {
    /// `[memory] limit`.
    pub memory_limit: MemoryLimit,
}

impl AppSettings {
    /// The memory budget these settings give.
    #[must_use]
    pub fn memory_budget(&self) -> MemoryBudget {
        MemoryBudget::from_setting(self.memory_limit)
    }
}

/// The result of [`load`]: the settings, where they came from, and a
/// warning when the file exists but was not used.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSettings {
    /// The settings; the defaults when no file was used.
    pub settings: AppSettings,
    /// The path that was looked up, or `None` when no path resolved.
    pub path: Option<PathBuf>,
    /// Why an existing file was not used, for a toast or a log line.
    pub warning: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawSettings {
    #[serde(default)]
    memory: RawMemory,
}

#[derive(Debug, Default, Deserialize)]
struct RawMemory {
    #[serde(default)]
    limit: Option<RawLimit>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawLimit {
    Bytes(u64),
    Text(String),
}

/// Parse the text of a settings file.
///
/// # Errors
/// A sentence that names the problem: bad TOML, or a limit that is not a
/// size.
pub fn parse(text: &str) -> Result<AppSettings, String> {
    let raw: RawSettings =
        toml::from_str(text).map_err(|error| format!("{SETTINGS_FILE} is not valid: {error}"))?;
    let memory_limit = match raw.memory.limit {
        None => MemoryLimit::Default,
        Some(RawLimit::Bytes(bytes)) => MemoryLimit::Bytes(bytes),
        Some(RawLimit::Text(text)) if text.trim().eq_ignore_ascii_case(UNLIMITED) => {
            MemoryLimit::Unlimited
        }
        Some(RawLimit::Text(text)) => parse_byte_size(&text)
            .map(MemoryLimit::Bytes)
            .map_err(|error| format!("[memory] limit: {error}"))?,
    };
    Ok(AppSettings { memory_limit })
}

/// The settings file path from an environment lookup. Pure, so a test can
/// give it any environment.
#[must_use]
pub fn settings_path_from(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty());
    if let Some(file) = var(SETTINGS_ENV) {
        return Some(PathBuf::from(file));
    }
    let config_dir = if let Some(xdg) = var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("rs_cam")
    } else if let Some(home) = var("HOME") {
        PathBuf::from(home).join(".config").join("rs_cam")
    } else if let Some(appdata) = var("APPDATA") {
        PathBuf::from(appdata).join("rs_cam")
    } else {
        let profile = var("USERPROFILE")?;
        PathBuf::from(profile).join(".config").join("rs_cam")
    };
    Some(config_dir.join(SETTINGS_FILE))
}

/// The settings file path of this process. Does not create it.
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    settings_path_from(|name| std::env::var(name).ok())
}

/// Read the settings of this process. Never fails; see [`LoadedSettings`].
#[must_use]
pub fn load() -> LoadedSettings {
    match settings_path() {
        Some(path) => load_from(&path),
        None => LoadedSettings::default(),
    }
}

/// Read the settings from `path`. A missing file gives the defaults with no
/// warning.
#[must_use]
pub fn load_from(path: &Path) -> LoadedSettings {
    let used_path = Some(path.to_path_buf());
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadedSettings {
                path: used_path,
                ..LoadedSettings::default()
            };
        }
        Err(error) => {
            return LoadedSettings {
                path: used_path,
                warning: Some(format!(
                    "{} was not read ({error}); the defaults apply",
                    path.display()
                )),
                ..LoadedSettings::default()
            };
        }
    };
    match parse(&text) {
        Ok(settings) => LoadedSettings {
            settings,
            path: used_path,
            warning: None,
        },
        Err(problem) => LoadedSettings {
            path: used_path,
            warning: Some(format!("{}: {problem}; the defaults apply", path.display())),
            ..LoadedSettings::default()
        },
    }
}

#[cfg(test)]
mod tests {
    // SAFETY: test module; a failed unwrap is a failed test.
    #![allow(clippy::unwrap_used)]

    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn a_limit_parses_as_a_size_a_byte_count_or_unlimited() {
        let text = "[memory]\nlimit = \"12GiB\"\n";
        assert_eq!(
            parse(text).unwrap().memory_limit,
            MemoryLimit::Bytes(12 * GIB)
        );
        let text = "[memory]\nlimit = 4096\n";
        assert_eq!(parse(text).unwrap().memory_limit, MemoryLimit::Bytes(4096));
        let text = "[memory]\nlimit = \"Unlimited\"\n";
        assert_eq!(parse(text).unwrap().memory_limit, MemoryLimit::Unlimited);
    }

    #[test]
    fn no_limit_key_is_the_default() {
        assert_eq!(parse("").unwrap(), AppSettings::default());
        assert_eq!(parse("[memory]\n").unwrap(), AppSettings::default());
        assert_eq!(
            parse("[other]\nkey = 1\n").unwrap().memory_limit,
            MemoryLimit::Default
        );
    }

    #[test]
    fn a_bad_limit_is_an_error_that_names_the_key() {
        let error = parse("[memory]\nlimit = \"12GB\"\n").unwrap_err();
        assert!(error.contains("[memory] limit"), "{error}");
        assert!(parse("[memory]\nlimit = -5\n").is_err());
        assert!(parse("not toml at all [").is_err());
    }

    #[test]
    fn a_missing_file_gives_the_defaults_with_no_warning() {
        let path = std::env::temp_dir().join(format!(
            "rs_cam_settings_missing_{}.toml",
            std::process::id()
        ));
        let loaded = load_from(&path);
        assert_eq!(loaded.settings, AppSettings::default());
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.path, Some(path));
    }

    #[test]
    fn a_bad_file_gives_the_defaults_with_a_warning() {
        let path =
            std::env::temp_dir().join(format!("rs_cam_settings_bad_{}.toml", std::process::id()));
        std::fs::write(&path, "[memory]\nlimit = \"lots\"\n").unwrap();
        let loaded = load_from(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
    }

    #[test]
    fn the_path_resolves_like_the_libraries_with_a_windows_fallback() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
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
}
