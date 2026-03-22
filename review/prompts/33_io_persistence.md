# Review: I/O & Persistence

## Scope
File I/O: import, export, project save/load, setup sheets, presets.

## Files to examine
- `crates/rs_cam_viz/src/io/mod.rs`
- `crates/rs_cam_viz/src/io/import.rs`
- `crates/rs_cam_viz/src/io/export.rs`
- `crates/rs_cam_viz/src/io/project.rs` (TOML project persistence)
- `crates/rs_cam_viz/src/io/setup_sheet.rs` (HTML generation)
- `crates/rs_cam_viz/src/io/presets.rs`
- FEATURE_CATALOG notes on what IS and ISN'T persisted

## What to review

### Project save/load
- TOML format: is it human-readable/editable?
- What round-trips correctly? What doesn't?
- Model paths: relative or absolute? What if files move?
- Toolpath results: persisted or regenerated on load?
- Simulation state: NOT persisted — is this documented?

### Import robustness
- Error handling for each format (STL, SVG, DXF)
- File dialog filters
- Large file handling

### Export
- G-code export: all paths (single, combined, per-setup)
- SVG preview
- Setup sheet HTML quality

### Presets
- Tool presets? Machine presets?
- Where are they stored? User directory?
- Can they be shared?

### Testing & code quality

## Output
Write findings to `review/results/33_io_persistence.md`.
