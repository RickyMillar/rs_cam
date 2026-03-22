# Review: UI Panels (Layout, Usability, Consistency)

## Summary

The GUI uses a clean workspace-based layout (Setup / Toolpaths / Simulation) with consistent 3-panel arrangement (left tree, center viewport, right properties). The egui panel patterns are well-applied with resizable side panels, badges, and drag-drop. Main concerns are two oversized files (`properties/mod.rs` at 2674 lines, `sim_timeline.rs` at 1246 lines), sparse automation coverage (~2%), missing tooltips on abbreviations, and inconsistent spacing/event patterns across editors.

## Findings

### Layout Architecture

- **Three workspaces** with distinct layouts: Setup, Toolpaths, Simulation (app.rs:1766-1925)
- **Consistent panel structure**: Left SidePanel (200-240px), Right SidePanel (240-280px), CentralPanel viewport, TopBottomPanel status bar
- All SidePanels are resizable — panel widths are NOT persisted across sessions
- **Menu bar** (TopBottomPanel::top) with keyboard shortcuts: Ctrl+Z, Ctrl+S, Ctrl+Shift+E
- **Workspace bar** (TopBottomPanel::top) with tabs + badge computation (pending count, collision count)
- **Viewport overlay** (viewport_overlay.rs): view presets, render mode, visibility toggles inside CentralPanel
- **Status bar** (status_bar.rs): model/tri count, toolpath done/total, lane status chips with queue depth
- **Simulation workspace** adds a bottom timeline panel (min height 60px) and a top controls bar

### Workspace Switching

- Entering Simulation saves viewport state (show_cutting, show_rapids, show_stock) and sets sim defaults (app.rs:145-155)
- Leaving Simulation restores saved state
- No visual hint that visibility changed — users may be confused why paths disappear

### Property Editors — Consistency

- **Controls**: All editors use `egui::ComboBox::from_id_salt()` for enums, DragValue with `.speed()/.range()/.suffix()` for numbers — consistent
- **Section headers**: Uniform `RichText::new().strong().color(rgb(180, 180, 195))` across all editors
- **Grid spacing**: Editable grids use `[8.0, 4.0]`, read-only/info grids use `[8.0, 3.0]` or `[8.0, 2.0]` — intentional but undocumented
- **DragValue helper** `dv()` (properties/mod.rs:973-1001) exists but is only used in mod.rs, not in pocket.rs or other sub-editors
- **Event emission inconsistency**: stock.rs accumulates changes into one `StockChanged` event; setup.rs emits `FixtureChanged` immediately per-field — multiple events per edit

### Simulation Panels

- **sim_diagnostics.rs** (592 lines): Excellent use of CollapsingHeader (6 sections), consistent color coding for severity
- **sim_op_list.rs** (456 lines): Compact operation list with progress bars, color swatches, trace badges ("SEM", "PERF", "TRACE")
- **sim_timeline.rs** (1246 lines): Timeline scrubber with operation boundary markers — well-designed UX but file is too large
- **sim_debug.rs** (177 lines): Utility for trace badge rendering, well-factored

### Automation IDs

- `automation.rs` (51 lines): Records widget state via `automation::record(ui, "id", &response, "Label")` into egui temp data
- **Only ~8 automation IDs across the entire UI** (~2% coverage)
- Present: viewport overlay buttons (cancel_all, simulate, collision_check), stock_to_leave controls
- Missing: tool/setup/fixture properties, sim playback controls, operation checkboxes, toolpath panel actions

### Usability Concerns

- **Abbreviations without tooltips**: "Col" (Collisions), "Fix" (Fixtures) in viewport_overlay.rs; "TP", "AN" lane chips in status_bar.rs — no `.on_hover_text()`
- **Keyboard access limited**: Only Ctrl+Z/S/Shift+E documented; no shortcuts for add toolpath, generate, select workspace (workspace tabs are mouse-only)
- **Setup panel density**: chips for fixture/keepout/pin counts packed horizontally, may wrap on narrow displays
- **Toolpath card density**: Two rows (status+name, tool+actions) horizontally tight

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | `properties/mod.rs` is 2674 lines — 20+ operation param functions + dispatcher in one file | properties/mod.rs |
| 2 | High | `sim_timeline.rs` is 1246 lines — timeline rendering + playback + annotations all in one | sim_timeline.rs |
| 3 | High | `draw_toolpath_panel()` is 227 lines — selectors, feeds, boundary, extras all inline | properties/mod.rs:743-969 |
| 4 | Med | `draw_toolpath_card()` is 176 lines mixing render + context menu + drag indicator | toolpath_panel.rs:135-316 |
| 5 | Med | `pocket.rs` duplicates param drawing from `mod.rs` without using `dv()` helper | properties/pocket.rs:102-186 |
| 6 | Med | Event emission inconsistent: stock batches, setup emits per-field | stock.rs vs setup.rs |
| 7 | Med | Automation coverage ~2% — effectively non-functional for deterministic testing | automation.rs |
| 8 | Med | UI abbreviations lack tooltips (Col, Fix, TP, AN) | viewport_overlay.rs:48-50, status_bar.rs |
| 9 | Med | Magic spacing numbers (2.0, 4.0, 6.0, 8.0, 12.0) scattered with no constants | all UI files |
| 10 | Low | Panel widths not persisted across sessions | app.rs |
| 11 | Low | Workspace visibility changes on sim enter/exit with no user hint | app.rs:145-155 |
| 12 | Low | `op_feed_rate()` match has 21 arms — should be a method on OperationConfig | preflight.rs:253-278 |
| 13 | Low | `draw_rest_badge()` (60 lines) is domain logic in UI — should be state validation | toolpath_panel.rs:366-427 |
| 14 | Low | Import dialogs (STL/SVG/DXF) nearly identical — could be factored | menu_bar.rs:25-51 |

## Test Gaps

- No UI unit tests for any panel
- No snapshot testing for panel layout
- Automation framework exists but coverage too sparse for meaningful use
- No regression tests for workspace switching visibility state

## Suggestions

1. **Split `properties/mod.rs`**: Move operation-specific param functions to `properties/operations/` with one file per type (or group by 2D/3D); keep dispatcher in mod.rs
2. **Split `sim_timeline.rs`**: Extract timeline_scrubber, playback_controls, and boundary_annotations into separate files
3. **Extract `draw_toolpath_panel()`**: Split into `draw_tp_selectors()`, `draw_tp_feeds()`, `draw_tp_boundary()`, `draw_tp_extras()`
4. **Standardize event emission**: Use stock.rs accumulate-then-emit pattern everywhere
5. **Define spacing constants**: `SPACING_XS=2.0`, `SPACING_SM=4.0`, `SPACING_MD=8.0`, `SPACING_LG=12.0` in a theme module
6. **Add missing tooltips**: `.on_hover_text()` on all abbreviated labels
7. **Remove pocket.rs duplication**: Route through mod.rs dispatch and delete standalone version
8. **Decide on automation**: Either systematically expand to all interactive controls or document it as minimal/critical-path-only
