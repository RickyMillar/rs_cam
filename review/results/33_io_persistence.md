# Review: I/O & Persistence

## Summary

The rs_cam I/O layer provides TOML-based project persistence, multi-format import (STL/SVG/DXF), G-code export with multiple post-processors, HTML setup sheet generation, and operation presets. The code is well-structured with comprehensive error handling and round-trip testing. Computed toolpaths, simulation checkpoints, and collision outputs are intentionally not persisted — only editable state is saved. Model paths are intelligently handled with relative path support when files are in the project directory.

## Findings

### Project Save/Load

**Format:** TOML v2 with fallback support for legacy format v1.

**What round-trips correctly (fully tested):**
- Job name, stock dimensions, stock origin, post-processor settings, machine profile
- All tool geometry (diameter, cutting length, corner radius, flute count, stickout, shaft/holder/shank)
- Tool material, cut direction, vendor/product ID
- All operation parameters for all operation types
- Dressups (entry style, lead-in/out, dogbone, link moves, feed optimization flag)
- Heights system (clearance, retract, feed, top, bottom)
- Boundary settings (enabled flag, containment mode)
- Toolpath state flags (enabled, visible, locked)
- Pre/post G-code strings
- Multi-setup structure with nested setup names, face orientation, Z rotation, datum methods
- Alignment pins, fixtures (all geometry and clearance), keep-out zones
- Debug options

**What does NOT round-trip (intentional, documented in FEATURE_CATALOG):**
- Computed toolpath results (`.result` cleared on load via `clear_runtime_state()`)
- Simulation checkpoints
- Collision detection outputs
- Feed rate calculation results
- Debug trace data
- Status flag (reset to `Pending`)

**Model Path Handling:**
- `persist_model_path()`: if model is in project directory (or subdirectory), stores relative path; otherwise absolute
- `resolve_model_path()`: resolves relative paths against the project directory on load
- Makes projects portable — moving the project directory keeps local models intact
- Test: `relative_paths_are_saved_and_resolved_against_project_dir()`

**Legacy Format Support:**
- Old projects (format_version 1) load via `load_legacy_project()`
- Auto-migrates tool indices to tool IDs and infers model kinds from file extension
- If no format version specified, defaults to 1

### Import Robustness

| Format | Errors Handled | Notes |
|--------|---------------|-------|
| STL | File read, parsing | Checks mesh winding; warns on reversed normals via `winding_report` |
| SVG | File read, parsing | Hardcoded tolerance 0.1mm for polygon conversion |
| DXF | File read, parsing | Hardcoded tolerance 5.0mm for polygon conversion |

- All return `Result<LoadedModel, String>` with descriptive errors
- No unwrap/panic in main code paths
- On successful import, toolpath entries marked `stale_since = Some(loaded_at)` to trigger recomputation
- If model file is missing on project load, `LoadedModel::placeholder()` created with `load_error` set — project still loads with a warning

**Graceful degradation warnings:**
1. `MissingModelFile` — path doesn't exist on disk
2. `ModelImportFailed` — file exists but parse failed
3. `MissingModelPath` — model section has empty path string
4. `MissingToolReference` — toolpath references nonexistent tool ID
5. `MissingModelReference` — toolpath references nonexistent model ID

### Export

**G-Code paths:**

| Function | Scope | Notes |
|----------|-------|-------|
| `export_gcode()` | All enabled toolpaths, flat | Calls `emit_gcode_phased()` |
| `export_combined_gcode()` | All setups with M0 pauses | `emit_gcode_multi_setup()` with safe_z between setups |
| `export_setup_gcode()` | Single setup only | Filters by `SetupId` |

- Post-processors: GRBL, LinuxCNC, Mach3
- High feedrate mode: converts G0 rapids to G1 at specified feedrate via `replace_rapids_with_feed()`
- Pre/post G-code fields are NOT included in output (intentional per FEATURE_CATALOG)

**Setup Sheet (HTML):**
- Comprehensive dark-theme HTML document (self-contained CSS)
- Sections: Stock, Setups, Workholding, Datum/Alignment, Tools, Operations, Post-Processor, Per-Op Details
- Estimated machining time from cutting distance / feed rate
- All user-supplied strings HTML-escaped
- 9 tests covering generation, format, escaping, validity

### Presets

- **Location:** `~/.rs_cam/presets/` (per user home directory)
- **Format:** TOML with name, operation, and content fields
- **Operations:** list, save, load, delete
- Filename sanitized (alphanumeric + `-/_`, spaces -> `-`)
- User-local only (no multi-machine sync); shareable by copying files
- 9 tests covering sanitization, TOML escape/unescape, save/load roundtrips

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | No atomic file writes for project save — crash during write corrupts project | project.rs:391 |
| 2 | Med | Preset TOML parsing is manual string slicing, not robust to whitespace/comment changes | presets.rs:107-140 |
| 3 | Low | Relative path handling on Windows: forward slashes work but Windows backslash paths won't round-trip cross-platform | project.rs:1074-1092 |
| 4 | Low | No path traversal checks — `resolve_model_path()` will try any path including `../../sensitive`; import functions fail gracefully | import.rs:12-85 |
| 5 | Low | Model scaling option only for STL; SVG/DXF always assume mm | import.rs:20-24 |
| 6 | Info | Computed toolpath results not persisted | FEATURE_CATALOG:110, project.rs:1030 |
| 7 | Info | Pre/post G-code not emitted on export | FEATURE_CATALOG:109 |

## Test Gaps

1. **Large file handling:** No tests for STL/DXF files >100MB. Memory usage and streaming untested.
2. **File dialog integration:** No tests for actual file picker dialogs (egui-based).
3. **Corrupt/malformed files:** Only missing file and parse failures are tested. No tests for partially written TOML, invalid UTF-8, or circular model references.
4. **Platform-specific paths:** No tests for Windows UNC paths, symlinks, or read-only filesystems.
5. **Preset edge cases:** No tests for presets with no `name` field, multiline content containing `"""` markers, or same-name collision.
6. **Export validation:** G-code syntax is not validated; no tests for invalid setup IDs in `export_setup_gcode()`.
7. **Model kind inference:** `infer_model_kind()` only checks extension; no magic-number validation.

## Suggestions

### High Priority
1. **Atomic project writes** — replace `fs::write()` with temp file + `rename()` pattern to ensure crash safety.
2. **Preset TOML parser** — replace manual string parsing with the `toml` crate for robustness.

### Medium Priority
3. **Path traversal audit** — add validation for model paths (canonicalize + starts_with check).
4. **Windows path handling** — test and document cross-platform behavior; consider `pathdiff` crate.
5. **Model scaling for 2D** — expose scale factor for SVG/DXF imports in UI and project format.

### Low Priority
6. **Corrupt file recovery** — add `.bak` backup of last successful save; offer recovery on load failure.
7. **Export validation** — add dry-run G-code validator (check for missing M3, unclosed loops, etc.).
8. **Version migration** — document format version upgrade path explicitly in code comments.

### Strengths Worth Noting
- Graceful degradation: missing files don't crash the app; warnings are surfaced
- Portable projects: relative paths + intelligent fallback
- Comprehensive round-trip: all editable state preserved; intentional non-persistence documented
- Setup sheets: thorough and operator-friendly HTML generation
- 25+ unit tests covering happy paths and edge cases
- Human-readable TOML format with sensible defaults
