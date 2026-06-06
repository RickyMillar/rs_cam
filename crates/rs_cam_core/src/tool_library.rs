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
    #[error(
        "tool library directory could not be determined (no RS_CAM_TOOL_DIR, XDG_CONFIG_HOME, or HOME)"
    )]
    NoLibraryDir,
    #[error("invalid catalog name {0:?}: must be non-empty and contain no path separators or '..'")]
    InvalidName(String),
    #[error("catalog {0:?} already exists")]
    AlreadyExists(String),
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
            path.file_stem().and_then(|s| s.to_str()).map(str::to_owned)
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

/// Remove the tool at `index` from catalog `name` in `dir`, then save.
/// Out-of-range index is a no-op (the catalog is still re-saved). Returns
/// the resulting catalog.
pub fn remove_tool_at_in(
    dir: &Path,
    name: &str,
    index: usize,
) -> Result<ToolCatalog, ToolLibraryError> {
    let mut catalog = load_from(dir, name)?;
    if index < catalog.tools.len() {
        catalog.tools.remove(index);
    }
    save_to(dir, name, &catalog)?;
    Ok(catalog)
}

/// Remove the tool at `index` from catalog `name` in the resolved dir.
pub fn remove_tool_at(name: &str, index: usize) -> Result<ToolCatalog, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    remove_tool_at_in(&dir, name, index)
}

/// Replace the tool at `index` in catalog `name` in `dir`, then save.
/// Out-of-range index is a no-op. Returns the resulting catalog.
pub fn update_tool_at_in(
    dir: &Path,
    name: &str,
    index: usize,
    tool: ToolConfig,
) -> Result<ToolCatalog, ToolLibraryError> {
    let mut catalog = load_from(dir, name)?;
    if let Some(slot) = catalog.tools.get_mut(index) {
        *slot = tool;
    }
    save_to(dir, name, &catalog)?;
    Ok(catalog)
}

/// Replace the tool at `index` in catalog `name` in the resolved dir.
pub fn update_tool_at(
    name: &str,
    index: usize,
    tool: ToolConfig,
) -> Result<ToolCatalog, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    update_tool_at_in(&dir, name, index, tool)
}

/// Move the tool at `index` from catalog `from` to catalog `to` in `dir`.
/// Same-name move and out-of-range index are no-ops. The destination
/// catalog is created if absent.
pub fn move_tool_in(
    dir: &Path,
    from: &str,
    index: usize,
    to: &str,
) -> Result<(), ToolLibraryError> {
    if from == to {
        return Ok(());
    }
    let mut src = load_from(dir, from)?;
    if index >= src.tools.len() {
        return Ok(());
    }
    let tool = src.tools.remove(index);
    append_to(dir, to, tool)?;
    save_to(dir, from, &src)?;
    Ok(())
}

/// Move a tool between catalogs in the resolved dir.
pub fn move_tool(from: &str, index: usize, to: &str) -> Result<(), ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    move_tool_in(&dir, from, index, to)
}

/// Delete the whole catalog file `name` in `dir`.
pub fn delete_from(dir: &Path, name: &str) -> Result<(), ToolLibraryError> {
    let path = path_in(dir, name)?;
    std::fs::remove_file(&path).map_err(|source| ToolLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    Ok(())
}

/// Delete the whole catalog file `name` in the resolved dir.
pub fn delete_library(name: &str) -> Result<(), ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    delete_from(&dir, name)
}

/// Rename catalog `old` to `new` in `dir`. Errors if `new` already exists.
pub fn rename_in(dir: &Path, old: &str, new: &str) -> Result<(), ToolLibraryError> {
    let old_path = path_in(dir, old)?;
    let new_path = path_in(dir, new)?;
    if new_path.exists() {
        return Err(ToolLibraryError::AlreadyExists(new.to_owned()));
    }
    std::fs::rename(&old_path, &new_path).map_err(|source| ToolLibraryError::Io {
        name: old.to_owned(),
        source,
    })?;
    Ok(())
}

/// Rename a catalog in the resolved dir.
pub fn rename_library(old: &str, new: &str) -> Result<(), ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    rename_in(&dir, old, new)
}

/// Create an empty catalog `name` in `dir`. Errors if it already exists.
pub fn create_in(dir: &Path, name: &str) -> Result<PathBuf, ToolLibraryError> {
    let path = path_in(dir, name)?;
    if path.exists() {
        return Err(ToolLibraryError::AlreadyExists(name.to_owned()));
    }
    save_to(dir, name, &ToolCatalog::default())
}

/// Create an empty catalog in the resolved dir.
pub fn create_library(name: &str) -> Result<PathBuf, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    create_in(&dir, name)
}

/// Stable equality key for de-duplicating tools within a catalog.
/// Two tools that share type, diameter, flute count, cutting length,
/// and the relevant angle fields are treated as the same tool.
fn dedupe_key(t: &ToolConfig) -> String {
    format!(
        "{:?}|{:.3}|{}|{:.3}|{:.2}|{:.2}|{:.3}",
        t.tool_type,
        t.diameter,
        t.flute_count,
        t.cutting_length,
        t.taper_half_angle,
        t.included_angle,
        t.corner_radius,
    )
}

/// Remove duplicate tools (by [`dedupe_key`]) from catalog `name` in
/// `dir`, keeping the first occurrence, then save. Returns the result.
pub fn dedupe_in(dir: &Path, name: &str) -> Result<ToolCatalog, ToolLibraryError> {
    let mut catalog = load_from(dir, name)?;
    let mut seen = std::collections::HashSet::new();
    catalog.tools.retain(|t| seen.insert(dedupe_key(t)));
    save_to(dir, name, &catalog)?;
    Ok(catalog)
}

/// De-duplicate a catalog in the resolved dir.
pub fn dedupe_library(name: &str) -> Result<ToolCatalog, ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    dedupe_in(&dir, name)
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
        assert!(matches!(
            path_in(&dir, ""),
            Err(ToolLibraryError::InvalidName(_))
        ));
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

    #[test]
    fn remove_tool_at_drops_the_right_index_and_persists() {
        let dir = temp_dir("remove_at");
        append_to(&dir, "endmills", tool("a", 1.0)).unwrap();
        append_to(&dir, "endmills", tool("b", 2.0)).unwrap();
        append_to(&dir, "endmills", tool("c", 3.0)).unwrap();
        let cat = remove_tool_at_in(&dir, "endmills", 1).unwrap();
        assert_eq!(cat.tools.len(), 2);
        assert_eq!(cat.tools[0].name, "a");
        assert_eq!(cat.tools[1].name, "c");
        // Persisted to disk, not just returned.
        let reloaded = load_from(&dir, "endmills").unwrap();
        assert_eq!(reloaded.tools.len(), 2);
        assert_eq!(reloaded.tools[1].name, "c");
        // Out-of-range index is a no-op.
        let same = remove_tool_at_in(&dir, "endmills", 99).unwrap();
        assert_eq!(same.tools.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_tool_at_replaces_in_place() {
        let dir = temp_dir("update_at");
        append_to(&dir, "endmills", tool("a", 1.0)).unwrap();
        append_to(&dir, "endmills", tool("b", 2.0)).unwrap();
        let cat = update_tool_at_in(&dir, "endmills", 0, tool("a2", 1.5)).unwrap();
        assert_eq!(cat.tools[0].name, "a2");
        assert_eq!(cat.tools[0].diameter, 1.5);
        assert_eq!(cat.tools[1].name, "b");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_tool_transfers_between_catalogs() {
        let dir = temp_dir("move_tool");
        append_to(&dir, "src", tool("x", 1.0)).unwrap();
        append_to(&dir, "src", tool("y", 2.0)).unwrap();
        move_tool_in(&dir, "src", 0, "dst").unwrap();
        let src = load_from(&dir, "src").unwrap();
        let dst = load_from(&dir, "dst").unwrap();
        assert_eq!(src.tools.len(), 1);
        assert_eq!(src.tools[0].name, "y");
        assert_eq!(dst.tools.len(), 1);
        assert_eq!(dst.tools[0].name, "x");
        // Same-name move is a no-op.
        move_tool_in(&dir, "src", 0, "src").unwrap();
        assert_eq!(load_from(&dir, "src").unwrap().tools.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_and_rename_catalogs() {
        let dir = temp_dir("del_rename");
        append_to(&dir, "old", tool("x", 1.0)).unwrap();
        rename_in(&dir, "old", "new").unwrap();
        assert_eq!(list_in(&dir), vec!["new"]);
        // Rename onto an existing name is rejected.
        append_to(&dir, "other", tool("z", 2.0)).unwrap();
        assert!(matches!(
            rename_in(&dir, "new", "other"),
            Err(ToolLibraryError::AlreadyExists(_))
        ));
        delete_from(&dir, "new").unwrap();
        assert_eq!(list_in(&dir), vec!["other"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_empty_catalog_then_reject_duplicate() {
        let dir = temp_dir("create_empty");
        create_in(&dir, "blank").unwrap();
        assert_eq!(load_from(&dir, "blank").unwrap().tools.len(), 0);
        assert!(matches!(
            create_in(&dir, "blank"),
            Err(ToolLibraryError::AlreadyExists(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedupe_keeps_first_of_each_identical_tool() {
        let dir = temp_dir("dedupe");
        // Two identical geometries (same name irrelevant to the key) plus
        // one distinct diameter.
        append_to(&dir, "cat", tool("dup1", 6.0)).unwrap();
        append_to(&dir, "cat", tool("dup2", 6.0)).unwrap();
        append_to(&dir, "cat", tool("unique", 8.0)).unwrap();
        let cat = dedupe_in(&dir, "cat").unwrap();
        assert_eq!(cat.tools.len(), 2);
        assert_eq!(cat.tools[0].name, "dup1");
        assert_eq!(cat.tools[1].name, "unique");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
