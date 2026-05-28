//! Reusable tool library — catalog files of many tools.
//!
//! Unlike the machine library (one file per machine, referenced by a
//! project), tools live in **catalog** TOML files that each hold a
//! collection of tools (e.g. `endmills.toml`, `vbits.toml`). The GUI
//! imports a tool *out of* a catalog: the chosen tool is copied into the
//! project (snapshot), so a generated toolpath never changes behind the
//! operator's back when a catalog file is later edited. Re-import to pick
//! up a catalog change.
//!
//! Each tool is stored as a serialized [`ToolConfig`]; the project-local
//! `id` is ignored on import (`add_tool` reassigns it).
//!
//! Directory resolution (first that is set wins):
//! 1. `$RS_CAM_TOOL_DIR` — explicit override (tests / CI).
//! 2. `$XDG_CONFIG_HOME/rs_cam/tools`.
//! 3. `$HOME/.config/rs_cam/tools`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compute::tool_config::ToolConfig;

/// Errors from tool-library file operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolLibraryError {
    #[error("tool library directory could not be determined (no RS_CAM_TOOL_DIR, XDG_CONFIG_HOME, or HOME)")]
    NoLibraryDir,
    #[error("invalid catalog name {0:?}: must be non-empty and contain no path separators or '..'")]
    InvalidName(String),
    #[error("i/o error for catalog {name:?}: {source}")]
    Io {
        name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse catalog {0:?}: {1}")]
    Parse(String, toml::de::Error),
    #[error("failed to serialize catalog {0:?}: {1}")]
    Serialize(String, toml::ser::Error),
}

/// A catalog file: a named collection of tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCatalog {
    #[serde(default, rename = "tools")]
    pub tools: Vec<ToolConfig>,
}

/// Resolve the per-user tool-library directory. Does not create it.
pub fn library_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("RS_CAM_TOOL_DIR")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("rs_cam").join("tools"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("rs_cam")
                .join("tools"),
        );
    }
    None
}

fn name_is_valid(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != ".."
        && !name.contains("..")
}

fn path_in(dir: &Path, name: &str) -> Result<PathBuf, ToolLibraryError> {
    if !name_is_valid(name) {
        return Err(ToolLibraryError::InvalidName(name.to_owned()));
    }
    Ok(dir.join(format!("{name}.toml")))
}

/// List catalog names (file stems) in `dir`. Missing dir → empty.
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
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// List catalog names in the resolved library dir.
pub fn list_libraries() -> Vec<String> {
    library_dir().map(|d| list_in(&d)).unwrap_or_default()
}

/// Load a catalog by name from `dir`.
pub fn load_from(dir: &Path, name: &str) -> Result<ToolCatalog, ToolLibraryError> {
    let path = path_in(dir, name)?;
    let text = std::fs::read_to_string(&path).map_err(|source| ToolLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    toml::from_str(&text).map_err(|e| ToolLibraryError::Parse(name.to_owned(), e))
}

/// Load a catalog by name from the resolved library dir.
pub fn load_library(name: &str) -> Result<ToolCatalog, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    load_from(&dir, name)
}

/// Save a catalog under `name` into `dir`, creating the dir.
pub fn save_to(dir: &Path, name: &str, catalog: &ToolCatalog) -> Result<PathBuf, ToolLibraryError> {
    let path = path_in(dir, name)?;
    std::fs::create_dir_all(dir).map_err(|source| ToolLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    let text = toml::to_string_pretty(catalog)
        .map_err(|e| ToolLibraryError::Serialize(name.to_owned(), e))?;
    std::fs::write(&path, text).map_err(|source| ToolLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    Ok(path)
}

/// Save a catalog under `name` into the resolved library dir.
pub fn save_library(name: &str, catalog: &ToolCatalog) -> Result<PathBuf, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    save_to(&dir, name, catalog)
}

/// Append a tool to catalog `name` in `dir` (creating the catalog if
/// absent), then save. The tool keeps its fields verbatim; its
/// project-local `id` is irrelevant on later import.
pub fn append_to(dir: &Path, name: &str, tool: ToolConfig) -> Result<PathBuf, ToolLibraryError> {
    let mut catalog = match load_from(dir, name) {
        Ok(c) => c,
        Err(ToolLibraryError::Io { .. }) => ToolCatalog::default(),
        Err(e) => return Err(e),
    };
    catalog.tools.push(tool);
    save_to(dir, name, &catalog)
}

/// Append a tool to catalog `name` in the resolved library dir.
pub fn append_tool(name: &str, tool: ToolConfig) -> Result<PathBuf, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    append_to(&dir, name, tool)
}

/// Every tool across every catalog in the resolved library dir, paired
/// with its catalog name. Order: catalogs sorted, tools in file order.
pub fn all_tools() -> Vec<(String, ToolConfig)> {
    let Some(dir) = library_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for cat_name in list_in(&dir) {
        if let Ok(catalog) = load_from(&dir, &cat_name) {
            for tool in catalog.tools {
                out.push((cat_name.clone(), tool));
            }
        }
    }
    out
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
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs_cam_tool_lib_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn tool(name: &str, dia: f64) -> ToolConfig {
        let mut t = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        t.name = name.to_owned();
        t.diameter = dia;
        t
    }

    #[test]
    fn append_then_load_round_trips() {
        let dir = temp_dir("round_trip");
        append_to(&dir, "endmills", tool("1/4 downcut", 6.35)).unwrap();
        append_to(&dir, "endmills", tool("1/8 upcut", 3.175)).unwrap();
        let cat = load_from(&dir, "endmills").unwrap();
        assert_eq!(cat.tools.len(), 2);
        assert_eq!(cat.tools[0].name, "1/4 downcut");
        assert_eq!(cat.tools[1].diameter, 3.175);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_in_returns_sorted_catalog_stems() {
        let dir = temp_dir("list");
        append_to(&dir, "vbits", tool("60deg", 12.7)).unwrap();
        append_to(&dir, "endmills", tool("quarter", 6.35)).unwrap();
        assert_eq!(list_in(&dir), vec!["endmills", "vbits"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_catalog_names_rejected() {
        let dir = temp_dir("invalid");
        assert!(matches!(
            save_to(&dir, "../escape", &ToolCatalog::default()),
            Err(ToolLibraryError::InvalidName(_))
        ));
        assert!(matches!(path_in(&dir, ""), Err(ToolLibraryError::InvalidName(_))));
    }

    #[test]
    fn append_to_missing_catalog_creates_it() {
        let dir = temp_dir("create");
        // No prior file → append starts a fresh catalog.
        append_to(&dir, "fresh", tool("only", 8.0)).unwrap();
        let cat = load_from(&dir, "fresh").unwrap();
        assert_eq!(cat.tools.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
