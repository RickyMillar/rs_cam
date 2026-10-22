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
//! Directory resolution (first that is set wins), in
//! `crate::settings::paths`, the ONE resolver for the GUI, MCP and the CLI:
//! 1. `$RS_CAM_TOOL_DIR` — explicit override (tests / CI).
//! 2. `[paths] tool_library` in the settings file (File ▸ Preferences).
//! 3. `$XDG_CONFIG_HOME/rs_cam/tools`, else `$HOME/.config/rs_cam/tools`,
//!    else `%APPDATA%\rs_cam\tools`, else `%USERPROFILE%\.config\rs_cam\tools`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::compute::tool_config::ToolConfig;

/// Errors from tool-library file operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolLibraryError {
    #[error(
        "tool library directory could not be determined (no RS_CAM_TOOL_DIR, [paths] tool_library, XDG_CONFIG_HOME, HOME, APPDATA or USERPROFILE)"
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
    /// F029: a tool in the catalog is outside its domain
    /// ([`ToolConfig::validate`]). The loader refuses the catalog.
    #[error("catalog {catalog:?}: tool {tool:?} refused: {source}")]
    InvalidTool {
        catalog: String,
        tool: String,
        #[source]
        source: crate::tool::ToolGeometryError,
    },
}

impl crate::io::named_toml_library::LibraryError for ToolLibraryError {
    fn invalid_name(name: &str) -> Self {
        Self::InvalidName(name.to_owned())
    }

    fn already_exists(name: &str) -> Self {
        Self::AlreadyExists(name.to_owned())
    }

    fn io(name: &str, source: std::io::Error) -> Self {
        Self::Io {
            name: name.to_owned(),
            source,
        }
    }
}

/// A catalog file: a named collection of tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCatalog {
    #[serde(default, rename = "tools")]
    pub tools: Vec<ToolConfig>,
}

/// Resolve the per-user tool-library directory. Does not create it.
pub fn library_dir() -> Option<PathBuf> {
    // One resolver for every surface: `RS_CAM_TOOL_DIR`, else `[paths]
    // tool_library` of the settings file, else the config folder
    // (`crate::settings::paths`).
    crate::settings::paths::tool_library_dir()
}

fn path_in(dir: &Path, name: &str) -> Result<PathBuf, ToolLibraryError> {
    crate::io::named_toml_library::path_in(dir, name)
}

/// List catalog names (file stems) in `dir`. Missing dir → empty.
pub fn list_in(dir: &Path) -> Vec<String> {
    crate::io::named_toml_library::list_in(dir)
}

/// List catalog names in the resolved library dir.
pub fn list_libraries() -> Vec<String> {
    library_dir().map(|d| list_in(&d)).unwrap_or_default()
}

/// Load a catalog by name from `dir`.
///
/// F029: refuses the catalog when one tool is outside its domain
/// ([`ToolLibraryError::InvalidTool`]).
pub fn load_from(dir: &Path, name: &str) -> Result<ToolCatalog, ToolLibraryError> {
    let path = path_in(dir, name)?;
    let text = std::fs::read_to_string(&path).map_err(|source| ToolLibraryError::Io {
        name: name.to_owned(),
        source,
    })?;
    let catalog: ToolCatalog =
        toml::from_str(&text).map_err(|e| ToolLibraryError::Parse(name.to_owned(), e))?;
    for tool in &catalog.tools {
        check_tool(name, tool)?;
    }
    Ok(catalog)
}

/// F029: the check every catalog writer runs on the tool it writes, so a
/// Save never makes a catalog that [`load_from`] refuses.
fn check_tool(catalog: &str, tool: &ToolConfig) -> Result<(), ToolLibraryError> {
    tool.validate()
        .map_err(|source| ToolLibraryError::InvalidTool {
            catalog: catalog.to_owned(),
            tool: tool.name.clone(),
            source,
        })
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

/// Append a tool to catalog `name` in `dir` (creating the catalog if
/// absent), then save. The tool keeps its fields verbatim; its
/// project-local `id` is irrelevant on later import.
pub fn append_to(dir: &Path, name: &str, tool: ToolConfig) -> Result<PathBuf, ToolLibraryError> {
    check_tool(name, &tool)?;
    let mut catalog = match load_from(dir, name) {
        Ok(c) => c,
        Err(ToolLibraryError::Io { .. }) => ToolCatalog::default(),
        Err(e) => return Err(e),
    };
    catalog.tools.push(tool);
    save_to(dir, name, &catalog)
}

/// Add `tool` to catalog `name` in `dir`, **replacing** an existing tool
/// with the same [`dedupe_key`] (geometry signature) instead of pushing a
/// duplicate. Returns `(path, replaced)`: `replaced` is the catalog entry
/// that the save overwrote, so the caller can name it (F176), or `None`
/// for a plain insert. TOO-004: this retires the silent-append that piled
/// up duplicates only the Dedupe button could later clean.
pub fn add_or_replace_to(
    dir: &Path,
    name: &str,
    tool: ToolConfig,
) -> Result<(PathBuf, Option<ToolConfig>), ToolLibraryError> {
    check_tool(name, &tool)?;
    let mut catalog = match load_from(dir, name) {
        Ok(c) => c,
        Err(ToolLibraryError::Io { .. }) => ToolCatalog::default(),
        Err(e) => return Err(e),
    };
    let key = dedupe_key(&tool);
    let replaced = if let Some(slot) = catalog.tools.iter_mut().find(|t| dedupe_key(t) == key) {
        Some(std::mem::replace(slot, tool))
    } else {
        catalog.tools.push(tool);
        None
    };
    let path = save_to(dir, name, &catalog)?;
    Ok((path, replaced))
}

/// Add-or-replace a tool in catalog `name` in the resolved library dir.
/// The second value is the entry that the save overwrote, if any.
pub fn add_or_replace_tool(
    name: &str,
    tool: ToolConfig,
) -> Result<(PathBuf, Option<ToolConfig>), ToolLibraryError> {
    let dir = library_dir().ok_or(ToolLibraryError::NoLibraryDir)?;
    add_or_replace_to(&dir, name, tool)
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
pub(crate) fn update_tool_at_in(
    dir: &Path,
    name: &str,
    index: usize,
    tool: ToolConfig,
) -> Result<ToolCatalog, ToolLibraryError> {
    check_tool(name, &tool)?;
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
    crate::io::named_toml_library::rename_in(dir, old, new)
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
///
/// Policy (operator ruling, F176, 2026-10-19): the geometry and the
/// material make a library tool distinct. Each field is IN or OUT:
///
/// - IN: every cutter dimension (`tool_type`, `diameter`,
///   `cutting_length`, `helix_deg`, `corner_radius_mm`, `corner_radius`,
///   `included_angle`, `taper_half_angle`, `shaft_diameter`), the
///   `shank_diameter` (a fixed part of the physical tool: deflection,
///   collision and the simulation cache read it), `flute_count`,
///   `tool_material` and `cut_direction`.
/// - OUT: `id` (project-local), `name`, `tool_number`, `stickout` (how
///   the tool is mounted, not what it is), `vendor`, `product_id` and
///   `size_units` (display only).
///
/// The destructure names every field with no `..`, so a new
/// `ToolConfig` field does not compile until someone puts it IN or OUT.
/// Lengths format at 0.001 mm and angles at 0.01 degree, so a float
/// round trip through TOML does not split one tool into two.
fn dedupe_key(t: &ToolConfig) -> String {
    let ToolConfig {
        id: _,
        name: _,
        tool_number: _,
        tool_type,
        diameter,
        cutting_length,
        helix_deg,
        corner_radius_mm,
        corner_radius,
        included_angle,
        taper_half_angle,
        shaft_diameter,
        shank_diameter,
        stickout: _,
        flute_count,
        tool_material,
        cut_direction,
        vendor: _,
        product_id: _,
        size_units: _,
    } = t;
    format!(
        "{tool_type:?}|{diameter:.3}|{flute_count}|{cutting_length:.3}|{taper_half_angle:.2}|\
         {included_angle:.2}|{corner_radius:.3}|{corner_radius_mm:.3}|{shaft_diameter:.3}|\
         {shank_diameter:.3}|{helix_deg:.2}|{tool_material:?}|{cut_direction:?}",
    )
}

/// Remove duplicate tools (by [`dedupe_key`]) from catalog `name` in
/// `dir`, keeping the first occurrence, then save. Returns the result.
pub(crate) fn dedupe_in(dir: &Path, name: &str) -> Result<ToolCatalog, ToolLibraryError> {
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
        let mut t = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
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

    #[test]
    fn add_or_replace_overwrites_same_key_and_appends_distinct() {
        let dir = temp_dir("add_or_replace");
        // First save of a 6 mm geometry: a plain insert.
        let (_, replaced) = add_or_replace_to(&dir, "cat", tool("first", 6.0)).unwrap();
        assert!(
            replaced.is_none(),
            "first save of a new geometry is not a replace"
        );
        // Re-saving the same geometry (different name) overwrites in place —
        // no duplicate piles up (TOO-004) — and names the entry it replaced.
        let (_, replaced) = add_or_replace_to(&dir, "cat", tool("renamed", 6.0)).unwrap();
        assert_eq!(
            replaced.map(|t| t.name).as_deref(),
            Some("first"),
            "same dedupe key should replace, not append, and name the old entry"
        );
        let cat = load_from(&dir, "cat").unwrap();
        assert_eq!(cat.tools.len(), 1, "no duplicate appended");
        assert_eq!(cat.tools[0].name, "renamed", "the entry was overwritten");
        // A distinct geometry still appends.
        let (_, replaced) = add_or_replace_to(&dir, "cat", tool("bigger", 8.0)).unwrap();
        assert!(replaced.is_none());
        assert_eq!(load_from(&dir, "cat").unwrap().tools.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F176: the fields that are OUT of the key (name, stickout, tool
    /// number, vendor, product id, size units, id) do not make a tool
    /// distinct.
    #[test]
    fn dedupe_key_ignores_name_stickout_and_catalog_metadata() {
        let a = tool("1/4 upcut", 6.35);
        let mut b = tool("quarter inch flat", 6.35);
        b.stickout = a.stickout + 12.0;
        b.tool_number = a.tool_number + 4;
        b.vendor = "Amana".to_owned();
        b.product_id = "46200".to_owned();
        b.size_units = Some(crate::compute::tool_config::SizeUnits::Imperial);
        b.id = ToolId::from_raw(7);
        assert_eq!(dedupe_key(&a), dedupe_key(&b));
    }

    /// F176: every geometry field and the material are IN the key. A
    /// second tool that differs in one of them is a different tool, and
    /// a Save must not replace the first one.
    #[test]
    fn dedupe_key_separates_geometry_and_material() {
        use crate::compute::tool_config::{BitCutDirection, ToolMaterial};
        type Edit = fn(&mut ToolConfig);
        let base = tool("t", 6.0);
        let variants: Vec<(&str, Edit)> = vec![
            ("tool_material", |t| t.tool_material = ToolMaterial::Hss),
            ("corner_radius_mm", |t| t.corner_radius_mm = 0.5),
            ("corner_radius", |t| t.corner_radius = 0.5),
            ("shaft_diameter", |t| t.shaft_diameter = 8.0),
            ("shank_diameter", |t| t.shank_diameter = 8.0),
            ("helix_deg", |t| t.helix_deg = 45.0),
            ("cut_direction", |t| {
                t.cut_direction = BitCutDirection::DownCut;
            }),
            ("included_angle", |t| t.included_angle = 60.0),
            ("taper_half_angle", |t| t.taper_half_angle = 5.0),
            ("flute_count", |t| t.flute_count = 3),
            ("cutting_length", |t| t.cutting_length = 12.0),
            ("diameter", |t| t.diameter = 8.0),
            ("tool_type", |t| t.tool_type = ToolType::BallNose),
        ];
        for (field, edit) in &variants {
            let mut other = base.clone();
            edit(&mut other);
            assert_ne!(
                dedupe_key(&base),
                dedupe_key(&other),
                "{field} must make a library tool distinct"
            );
        }
    }

    /// F176 regression: a second tool that differs only in material is
    /// appended; the Save does not replace the first one.
    #[test]
    fn add_or_replace_keeps_a_tool_of_another_material() {
        let dir = temp_dir("add_or_replace_material");
        add_or_replace_to(&dir, "cat", tool("carbide", 6.0)).unwrap();
        let mut hss = tool("hss", 6.0);
        hss.tool_material = crate::compute::tool_config::ToolMaterial::Hss;
        let (_, replaced) = add_or_replace_to(&dir, "cat", hss).unwrap();
        assert!(replaced.is_none());
        assert_eq!(load_from(&dir, "cat").unwrap().tools.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
