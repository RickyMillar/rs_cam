//! The app settings file: `~/.config/rs_cam/settings.toml` (B5).
//!
//! The GUI and the CLI read this file through this ONE loader. The file holds
//! the memory limit of `planning/memory_budget_2026-10-01/PLAN.md`
//! ("Architecture" 6):
//!
//! ```toml
//! [memory]
//! limit = "12GiB"      # a binary size: B, KiB, MiB, GiB or TiB
//! # limit = 12884901888  (a bare integer is a byte count)
//! # limit = "unlimited"  (no limit, whatever the default is)
//! ```
//!
//! No `limit` key means the default: [`MemoryLimit::Default`], which reads
//! [`super::DEFAULT_SYSTEM_FRACTION`] (RULING PENDING).
//!
//! The path resolves like the libraries (`crate::io::tool_library::
//! library_dir`, `crate::io::machine_library::library_dir`): the environment
//! override first, then `XDG_CONFIG_HOME`, then `HOME`. When `HOME` is not set
//! (Windows), `APPDATA`, then `USERPROFILE`.
//!
//! A missing file gives the defaults and is never an error. A file that does
//! not read or parse also gives the defaults, plus a warning for the caller
//! to show.
//!
//! A surface flag overrides the file: [`resolve_limit`]. The CLI flag
//! `--memory-limit` is the one such flag today.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{ByteSizeError, MemoryBudget, MemoryLimit, parse_byte_size};

/// The environment variable that names the settings FILE, not a directory.
pub const SETTINGS_ENV: &str = "RS_CAM_SETTINGS";

/// The file name inside the `rs_cam` config directory.
const SETTINGS_FILE: &str = "settings.toml";

/// The text that asks for no limit, in the file and in the CLI flag.
pub const UNLIMITED: &str = "unlimited";

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

/// Parse one limit as text: a binary size (`"12GiB"`), a byte count
/// (`"4096"`), or [`UNLIMITED`] in any case. The file's text value and the
/// CLI flag both read through this function.
///
/// # Errors
/// The [`ByteSizeError`] of a text that is not a size.
pub fn parse_limit_text(text: &str) -> Result<MemoryLimit, ByteSizeError> {
    if text.trim().eq_ignore_ascii_case(UNLIMITED) {
        return Ok(MemoryLimit::Unlimited);
    }
    parse_byte_size(text).map(MemoryLimit::Bytes)
}

/// The limit that applies: the surface flag when it is given, else the
/// file's value. A flag of [`MemoryLimit::Default`] is still a flag.
#[must_use]
pub fn resolve_limit(flag: Option<MemoryLimit>, file: MemoryLimit) -> MemoryLimit {
    flag.unwrap_or(file)
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
        Some(RawLimit::Text(text)) => {
            parse_limit_text(&text).map_err(|error| format!("[memory] limit: {error}"))?
        }
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

    /// An environment lookup over fixed pairs. The tests never write the
    /// process environment: `std::env::set_var` is `unsafe` in edition 2024.
    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    /// A scratch file path that is unique to this process and this test.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rs_cam_settings_{name}_{}.toml",
            std::process::id()
        ))
    }

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
    fn the_flag_text_and_the_file_text_parse_the_same_way() {
        for text in ["12GiB", "4096", "unlimited", " UNLIMITED "] {
            let file = parse(&format!("[memory]\nlimit = \"{text}\"\n"))
                .unwrap()
                .memory_limit;
            assert_eq!(parse_limit_text(text).unwrap(), file, "{text}");
        }
        assert!(parse_limit_text("12GB").is_err());
        assert!(parse_limit_text("lots").is_err());
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
        let path = scratch("missing");
        let loaded = load_from(&path);
        assert_eq!(loaded.settings, AppSettings::default());
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.path, Some(path));
    }

    #[test]
    fn a_bad_file_gives_the_defaults_with_a_warning() {
        let path = scratch("bad");
        std::fs::write(&path, "[memory]\nlimit = \"lots\"\n").unwrap();
        let loaded = load_from(&path);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
    }

    /// The precedence, end to end: `RS_CAM_SETTINGS` names the file, the
    /// file gives the limit, and a surface flag overrides the file.
    #[test]
    fn the_env_file_gives_the_limit_and_a_flag_overrides_it() {
        let path = scratch("env_file");
        std::fs::write(&path, "[memory]\nlimit = \"3GiB\"\n").unwrap();
        let file_text = path.display().to_string();
        let pairs = [
            (SETTINGS_ENV, file_text.as_str()),
            ("XDG_CONFIG_HOME", "/elsewhere"),
            ("HOME", "/h"),
        ];
        let resolved = settings_path_from(env(&pairs)).unwrap();
        assert_eq!(resolved, path, "RS_CAM_SETTINGS wins over XDG and HOME");

        let loaded = load_from(&resolved);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(loaded.warning, None);
        let file = loaded.settings.memory_limit;
        assert_eq!(file, MemoryLimit::Bytes(3 * GIB));

        // No flag: the file decides.
        assert_eq!(resolve_limit(None, file), MemoryLimit::Bytes(3 * GIB));
        // A flag: the flag decides, larger, smaller or unlimited.
        assert_eq!(
            resolve_limit(Some(MemoryLimit::Bytes(GIB)), file),
            MemoryLimit::Bytes(GIB)
        );
        assert_eq!(
            resolve_limit(Some(MemoryLimit::Unlimited), file),
            MemoryLimit::Unlimited
        );
        assert_eq!(
            MemoryBudget::from_setting(resolve_limit(None, file)),
            MemoryBudget::with_limit(3 * GIB)
        );
    }

    #[test]
    fn the_path_resolves_like_the_libraries_with_a_windows_fallback() {
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
