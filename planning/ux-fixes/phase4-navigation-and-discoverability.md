# Phase 4: Navigation and Discoverability Fixes

## N5-05 + N1-01: Help menu + shortcut annotations [major]

### Problem

No Help menu exists. The app has ~20 keyboard shortcuts scattered across two handler methods but only 4 are shown in menus (Ctrl+Z, Ctrl+Shift+Z, Ctrl+S, Ctrl+Shift+E). Users have no way to discover the rest.

### Files to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/state/mod.rs` | 26-35 | Add `show_shortcuts: bool` field to `AppState` |
| `crates/rs_cam_viz/src/ui/menu_bar.rs` | 160-192 | Add Help menu after View menu; add accelerator text to File > Open Job |
| `crates/rs_cam_viz/src/app.rs` | 2220-2229 | Draw shortcuts window (same pattern as preflight modal) |
| `crates/rs_cam_viz/src/ui/mod.rs` | (new) | Add `shortcuts_window.rs` module |
| `crates/rs_cam_viz/src/ui/shortcuts_window.rs` | (new file) | Shortcuts reference window implementation |

### Current shortcut inventory

From `handle_keyboard_shortcuts` (app.rs:1835-1908):
- `Delete`/`Backspace` -- Delete selected toolpath
- `G` -- Generate selected toolpath
- `Shift+G` -- Generate all toolpaths
- `Space` -- Switch to Simulation workspace
- `I` -- Toggle isolation mode
- `H` -- Toggle selected toolpath visibility
- `1` -- Top view
- `2` -- Front view
- `3` -- Right view
- `4` -- Isometric view

From `handle_simulation_shortcuts` (app.rs:1912-1963):
- `Left` -- Step backward
- `Right` -- Step forward
- `Home` -- Jump to start
- `End` -- Jump to end
- `Space` -- Play/pause
- `Escape` -- Back to Toolpaths workspace
- `[` -- Speed down (0.5x)
- `]` -- Speed up (2x)

From `menu_bar.rs` (lines 6-21):
- `Ctrl+Z` -- Undo
- `Ctrl+Shift+Z` -- Redo
- `Ctrl+S` -- Save Job
- `Ctrl+Shift+E` -- Export G-code

Global (app.rs:2174):
- `F12` -- Screenshot

### Proposed changes

**1. Add `show_shortcuts` flag to AppState** (`state/mod.rs:26-35`)

Current:
```rust
pub struct AppState {
    ...
    pub show_preflight: bool,
}
```

Add after `show_preflight`:
```rust
    /// Show keyboard shortcuts reference window.
    pub show_shortcuts: bool,
```

Initialize to `false` in `AppState::new()`.

**2. Add Help menu to menu_bar.rs** (after line 190, before the closing `});`)

```rust
ui.menu_button("Help", |ui| {
    if ui.button("Keyboard Shortcuts...").clicked() {
        ui.close_menu();
        events.push(AppEvent::ShowShortcuts);
    }
});
```

**3. Add accelerator text to File > Open Job** (menu_bar.rs:63)

Change:
```rust
if ui.button("Open Job...").clicked() {
```
To:
```rust
if ui.add(egui::Button::new("Open Job...  Ctrl+O")).clicked() {
```

**4. Add `ShowShortcuts` variant to AppEvent** (ui/mod.rs)

Add to the enum:
```rust
    // Help
    ShowShortcuts,
```

**5. Create shortcuts_window.rs** (new file)

A function `pub fn draw(ctx: &egui::Context, show: &mut bool)` that draws an `egui::Window` with all shortcuts organized by workspace:

- **General**: Ctrl+O (Open Job), Ctrl+S (Save Job), Ctrl+Shift+E (Export G-code), Ctrl+Z (Undo), Ctrl+Shift+Z (Redo), F12 (Screenshot)
- **Toolpaths**: Delete (Remove toolpath), G (Generate selected), Shift+G (Generate all), Space (Go to Simulation), I (Toggle isolation), H (Toggle visibility), 1-4 (View presets)
- **Simulation**: Space (Play/pause), Left/Right (Step), Home/End (Jump), Escape (Back to Toolpaths), `[`/`]` (Speed)

Use `egui::Grid` with two columns: shortcut key in monospace, description in regular text. Group with `ui.heading()` per section.

**6. Handle ShowShortcuts in app.rs** (after preflight modal ~line 2229)

Follow same pattern as `show_preflight`:
```rust
AppEvent::ShowShortcuts => {
    self.controller.state_mut().show_shortcuts = true;
}
```

And in `update()` after the preflight block:
```rust
if self.controller.state().show_shortcuts {
    let mut show = true;
    crate::ui::shortcuts_window::draw(ctx, &mut show);
    if !show {
        self.controller.state_mut().show_shortcuts = false;
    }
}
```

**7. Register module** in `ui/mod.rs`:
```rust
pub mod shortcuts_window;
```

### Edge cases / risks

- The `ShowShortcuts` event needs routing. Since it only toggles a bool on AppState, handle it in `handle_events` in app.rs alongside the other direct-state mutations, or in the controller's `handle_internal_event`. The app.rs `handle_events` match is simpler since no controller logic is needed.
- The shortcuts window should use `egui::Window::open(&mut show)` so the X button closes it naturally.
- No conflict with existing key bindings -- the shortcut window is purely informational, no new keybindings needed for it (other than Ctrl+O for Open Job, which is new).

### How to verify

- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Launch GUI, confirm "Help" menu appears in menu bar.
- Click "Keyboard Shortcuts..." and confirm the window appears listing all shortcuts.
- Close the window via the X button.
- Confirm "Open Job... Ctrl+O" shows in File menu and Ctrl+O opens the file dialog.

---

## N2-06: Fill missing tooltip entries [minor]

### Problem

The `tooltip_for` lookup table (properties/mod.rs:2075-2128) has entries for most labels passed through `dv()`, but 14+ labels are missing.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | 2075-2128 | Add missing tooltip entries |

### Missing labels and proposed text

Identified by cross-referencing all `dv()` calls in `operations.rs` and `mod.rs` against the `tooltip_for` match arms:

| Label | Where used | Proposed tooltip |
|-------|-----------|-----------------|
| `"Slope From"` | DropCutter (ops:335), Scallop (ops:577), Morphed (ops:666) | `"Minimum surface slope (degrees) to machine. Faces shallower than this are skipped."` |
| `"Slope To"` | Already has entry, but double-check. It is present at line 2098 -- **no change needed**. |
| `"Pocket Depth"` | Inlay (ops:259) | `"Depth of the inlay pocket measured from stock surface."` |
| `"Flat Depth"` | Inlay (ops:268) | `"Depth for flat-bottom clearing in the inlay pocket. 0 = V-only."` |
| `"Boundary Offset"` | Inlay (ops:276) | `"Offset from the design boundary for the inlay cut. Adjusts fit."` |
| `"Flat Tool Radius"` | Inlay (ops:285) | `"Radius of the flat endmill used to clear the pocket floor."` |
| `"Spoilboard"` | AlignmentPinDrill (ops:2311) | `"How far the drill penetrates into the spoilboard below the stock."` |
| `"Width"` | Contour tab (ops:118) | `"Width of holding tabs that keep the part attached to stock."` |
| `"Height"` | Contour tab (ops:119) | `"Height of holding tabs from the floor of the cut."` |
| `"Offset Stepover"` | Pencil (ops:513) | `"Lateral step between offset cleanup passes around pencil traces."` |
| `"Pitch"` | Helix dressup (mod.rs:2182) | `"Vertical drop per revolution of the helical entry move."` |
| `"Radius"` | Helix dressup (mod.rs:2176), Lead-in (mod.rs:2221) | `"Radius of the helical or arc entry/exit move."` |
| `"Depth/Pass"` | Already has entry at line 2081 as "Depth per Pass" variant -- the match arm handles both. **No change needed.** |
| `"Max Rate"` | Feed optimization dressup (mod.rs:2297) | `"Maximum allowable feed rate during optimized sections."` |
| `"Ramp Rate"` | Feed optimization dressup (mod.rs:2305) | `"How quickly feed rate ramps up toward max (mm/min per mm of engagement)."` |

### Proposed change

In `tooltip_for` (mod.rs:2075-2128), add new match arms before the `_ => return None` fallback. The labels passed to `dv()` include leading whitespace (`"  Pitch:"`) but `tooltip_for` strips via `label.trim().trim_end_matches(':')`, so the match values should be the bare names:

```rust
"Slope From" => "Minimum surface slope (degrees) to machine. Faces shallower than this are skipped.",
"Pocket Depth" => "Depth of the inlay pocket measured from stock surface.",
"Flat Depth" => "Depth for flat-bottom clearing in the inlay pocket. 0 = V-only.",
"Boundary Offset" => "Offset from the design boundary for the inlay cut. Adjusts fit.",
"Flat Tool Radius" => "Radius of the flat endmill used to clear the pocket floor.",
"Spoilboard" => "How far the drill penetrates into the spoilboard below the stock.",
"Width" => "Width of holding tabs that keep the part attached to stock.",
"Height" => "Height of holding tabs from the floor of the cut.",
"Offset Stepover" => "Lateral step between offset cleanup passes around pencil traces.",
"Pitch" => "Vertical drop per revolution of the helical entry move.",
"Radius" => "Radius of the helical or arc entry/exit move.",
"Max Rate" => "Maximum allowable feed rate during optimized sections.",
"Ramp Rate" => "How quickly feed rate ramps up toward max (mm/min per mm of engagement).",
```

Insert these lines at mod.rs:2127, before the `_ => return None` line.

### Edge cases / risks

- `"Width"` and `"Height"` are generic labels. Currently they only appear for contour tabs. If a future operation uses `dv(ui, "Width:", ...)` for something unrelated, the tooltip might be misleading. Acceptable for now since the tooltip system is label-global.
- `"Radius"` is used for both helix entry and lead-in/out. The tooltip text is written to cover both cases.
- The leading `"  "` whitespace in labels like `"  Pitch:"` is stripped by `trim()` before matching, so the bare match arm works correctly.

### How to verify

- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Launch GUI, select a toolpath with an inlay operation. Hover over "Pocket Depth", "Flat Depth", "Boundary Offset", "Flat Tool Radius" labels and confirm tooltips appear.
- Select a DropCutter operation. Hover "Slope From:" and confirm tooltip.
- Enable Helix entry dressup. Hover "Pitch:" and "Radius:" and confirm tooltips.
- Enable lead-in/out dressup. Hover "Radius:" and confirm tooltip.
- Enable feed optimization. Hover "Max Rate:" and "Ramp Rate:" and confirm tooltips.

---

## N2-07 + N2-15: Fix undo coverage gaps [major]

### Problem

Two gaps in the undo system:

**(a) Stock snapshot comparison ignores material, alignment_pins, flip_axis, and workholding_rigidity.**

The comparison at mod.rs:139-145 only checks `x, y, z, origin_x, origin_y, origin_z, padding`. When the user changes stock material, alignment pins, flip axis, or workholding rigidity, the old snapshot is silently discarded (it returns `Some(old)` from `.take()` but the condition fails, so no `UndoAction` is pushed). The snapshot is lost because it was `.take()`-n.

**(b) Toolpath snapshot not pushed on Generate.**

When the user edits toolpath parameters and then clicks Generate (or presses G), the toolpath snapshot is still held. The `flush_toolpath_snapshot` check at mod.rs:82 sees `Selection::Toolpath(id)` still matches `tp_id`, so it puts the snapshot back. The generate event fires, and the result arrives asynchronously. If the user then clicks a different toolpath before the result arrives, the snapshot is flushed comparing the new result state (post-generate) against the pre-edit snapshot -- potentially losing intermediate edits if they undo.

Actually, re-reading the code more carefully: the snapshot is captured once (`if state.history.toolpath_snapshot.is_none()`) when the toolpath is first selected, and flushed when the user navigates away. The Generate event does not interfere with this flow -- the snapshot compares pre-edit vs post-edit state when selection changes. The real issue is different: if the user edits params, generates, then undoes -- the undo restores the old params but the generated result stays (the result is not part of the undo action). This is a known limitation but not a snapshot-loss bug.

However, there is a genuine gap: the `flush_toolpath_snapshot` at mod.rs:80-102 runs only in `draw()` when selection changes. If the user closes the app or loads a new job without changing selection, the pending snapshot is lost.

Let me re-focus on the concrete bugs:

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | 137-145 | Fix stock undo comparison to cover all fields |
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | 137-145 | Alternatively, replace field-by-field check with `!=` by deriving PartialEq on StockConfig |
| `crates/rs_cam_viz/src/state/job.rs` | 366 | Derive `PartialEq` on `StockConfig` |

### Current code (mod.rs:137-145)

```rust
if events.iter().any(|e| matches!(e, AppEvent::StockChanged))
    && let Some(old) = state.history.stock_snapshot.take()
    && (old.x != state.job.stock.x
        || old.y != state.job.stock.y
        || old.z != state.job.stock.z
        || old.origin_x != state.job.stock.origin_x
        || old.origin_y != state.job.stock.origin_y
        || old.origin_z != state.job.stock.origin_z
        || old.padding != state.job.stock.padding)
```

### Proposed fix

**Step 1:** Derive `PartialEq` on `StockConfig` (job.rs:366).

Current:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockConfig {
```

Change to:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockConfig {
```

This requires that `Material`, `AlignmentPin`, `FlipAxis`, and `WorkholdingRigidity` all implement `PartialEq`. Check:
- `Material` -- from rs_cam_core, check if it derives PartialEq. It's used in a `==` comparison already at stock.rs:27 (`stock.material == *mat`), so it does.
- `AlignmentPin` -- check if it derives PartialEq. If not, derive it.
- `FlipAxis` -- already uses `==` at stock.rs:220, so it does.
- `WorkholdingRigidity` -- from rs_cam_core, likely an enum that derives PartialEq. Verify.

**Step 2:** Replace the field-by-field comparison (mod.rs:139-145) with:
```rust
if events.iter().any(|e| matches!(e, AppEvent::StockChanged))
    && let Some(old) = state.history.stock_snapshot.take()
    && old != state.job.stock
```

This covers all current and future fields automatically.

**Step 3:** Additionally, the `StockMaterialChanged` event (emitted at stock.rs:32) should also trigger the undo comparison. Currently only `StockChanged` is checked. The material combo fires `StockMaterialChanged` but NOT `StockChanged`, so material changes never push an undo action.

Extend the event check:
```rust
if events.iter().any(|e| matches!(e, AppEvent::StockChanged | AppEvent::StockMaterialChanged))
    && let Some(old) = state.history.stock_snapshot.take()
    && old != state.job.stock
```

### Edge cases / risks

- Deriving `PartialEq` on `StockConfig` requires all inner types to implement it. If `Material` or `AlignmentPin` use `f64` fields, the derived `PartialEq` uses bitwise float comparison, which is correct for undo comparison (we want exact bit-equality, not approximate).
- If `AlignmentPin` does not derive `PartialEq`, add it. It's a simple struct with `x: f64, y: f64, diameter: f64`.
- Adding PartialEq to StockConfig is non-breaking.

### How to verify

- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Launch GUI. Change stock material. Press Ctrl+Z. Confirm the material reverts.
- Add an alignment pin. Press Ctrl+Z. Confirm the pin is removed.
- Change flip axis. Press Ctrl+Z. Confirm it reverts.
- Change workholding rigidity (if exposed in UI). Press Ctrl+Z. Confirm it reverts.
- Existing stock dimension undo still works (change X, Ctrl+Z, reverts).

---

## N1-06: Click-to-deselect in viewport [minor]

### Problem

Clicking empty viewport space does not deselect the current selection. The `handle_viewport_click` method (app.rs:1363-1396) calls `pick()` and only acts on `Some(hit)`. When `pick()` returns `None` (empty space), nothing happens.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/app.rs` | 1393-1395 | Add `else` branch to set `Selection::None` |

### Current code (app.rs:1393-1395)

```rust
if let Some(hit) = hit {
    self.handle_pick_hit(hit);
}
```

### Proposed change

```rust
if let Some(hit) = hit {
    self.handle_pick_hit(hit);
} else {
    self.controller.state_mut().selection = Selection::None;
}
```

### Edge cases / risks

- **Drag vs click**: The viewport `response` is allocated with `egui::Sense::click_and_drag` (app.rs:1532). The `.clicked()` guard (app.rs:1537) already ensures this only fires on actual clicks (press + release within threshold), not on drag-to-orbit. So no additional drag threshold guard is needed.
- **Simulation workspace**: The `handle_simulation_semantic_pick` call at line 1369 returns early if it handles the click. The deselect-on-empty only fires when the pick function finds nothing, which is correct -- in simulation mode, clicking empty space should also deselect (or at least not interfere).
- **Face selection mode**: When a toolpath is selected and the user clicks empty space in Toolpaths workspace, `pick()` returns None if they miss the model. Deselecting in this case is reasonable -- the user explicitly clicked away from the model/toolpath.
- **Undo flush**: Deselecting triggers `flush_toolpath_snapshot` / `flush_tool_snapshot` on the next `draw()` call, which is the correct behavior (commits pending edits to undo stack).

### How to verify

- Launch GUI. Select a toolpath in the tree. Click empty viewport space. Confirm the properties panel shows "Select an item in the project tree".
- Select stock. Click empty viewport. Confirm deselected.
- In simulation workspace, click empty space. Confirm no crash or unexpected behavior.
- Orbit the viewport (click-drag). Confirm no accidental deselection on drag.

---

## N5-06: Open Job missing Ctrl+O [minor]

### Problem

There is no Ctrl+O keyboard shortcut for "Open Job". The File menu shows "Open Job..." without any accelerator hint.

### Files to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/menu_bar.rs` | 6-21 | Add Ctrl+O binding in the keyboard shortcuts block |
| `crates/rs_cam_viz/src/ui/menu_bar.rs` | 63 | Add accelerator text to the button label |

### Current code

menu_bar.rs:6-21 (shortcut block):
```rust
ctx.input(|i| {
    if modifiers.ctrl && i.key_pressed(egui::Key::Z) {
        ...
    }
    if modifiers.ctrl && i.key_pressed(egui::Key::S) {
        events.push(AppEvent::SaveJob);
    }
    if modifiers.ctrl && modifiers.shift && i.key_pressed(egui::Key::E) {
        events.push(AppEvent::ExportGcode);
    }
});
```

menu_bar.rs:63:
```rust
if ui.button("Open Job...").clicked() {
```

### Proposed change

**1. Add Ctrl+O binding** (menu_bar.rs, inside the `ctx.input` closure, after the Ctrl+S block):

```rust
if modifiers.ctrl && i.key_pressed(egui::Key::O) {
    events.push(AppEvent::OpenJob);
}
```

**2. Update button label** (menu_bar.rs:63):

```rust
if ui.add(egui::Button::new("Open Job...  Ctrl+O")).clicked() {
```

### Edge cases / risks

- Ctrl+O does not conflict with any existing binding.
- The `OpenJob` event handler (app.rs:332-344) shows a file dialog, which is fine to trigger from a keyboard shortcut.
- On macOS, Cmd+O would be the expected binding. egui maps `modifiers.ctrl` to Cmd on macOS by default (via `command_or_ctrl`), but the code uses `modifiers.ctrl` directly. This matches the existing pattern (Ctrl+S, Ctrl+Z all use `modifiers.ctrl`). Follow the same convention.

### How to verify

- Launch GUI. Press Ctrl+O. Confirm the file dialog opens for loading a TOML job.
- Confirm "Open Job... Ctrl+O" appears in the File menu.

---

## N2-10: Kc and Hardness Index unexplained [minor]

### Problem

The stock material properties panel shows "Hardness Index" and "Kc" as bare numbers with no explanation of what they mean or how they affect machining.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/properties/stock.rs` | 42-66 | Add `.on_hover_text()` to the label responses |

### Current code (stock.rs:42-66)

```rust
egui::Grid::new("material_info")
    .num_columns(2)
    .spacing([8.0, 2.0])
    .show(ui, |ui| {
        ui.label(
            egui::RichText::new("Hardness Index:")
                .small()
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
        ui.label(
            egui::RichText::new(format!("{:.2}", stock.material.hardness_index()))
                .small()
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
        ui.end_row();

        ui.label(
            egui::RichText::new("Kc:")
                .small()
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
        ui.label(
            egui::RichText::new(format!("{:.1} N/mm\u{00B2}", stock.material.kc_n_per_mm2()))
                .small()
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
        ui.end_row();
    });
```

### Proposed change

Add `.on_hover_text()` to the label `Response` for each row. The `ui.label()` call returns a `Response`:

```rust
ui.label(
    egui::RichText::new("Hardness Index:")
        .small()
        .color(egui::Color32::from_rgb(140, 140, 150)),
).on_hover_text(
    "Relative material hardness (0-1). Higher values reduce recommended feed rates and depths of cut."
);
```

```rust
ui.label(
    egui::RichText::new("Kc:")
        .small()
        .color(egui::Color32::from_rgb(140, 140, 150)),
).on_hover_text(
    "Specific cutting force (N/mm\u{00B2}). Used to calculate spindle load and recommended feed rates. Higher Kc = harder to cut."
);
```

### Edge cases / risks

- None. Adding hover text to existing labels is purely additive.

### How to verify

- Launch GUI. Select Stock. Hover over "Hardness Index:" label. Confirm tooltip appears.
- Hover over "Kc:" label. Confirm tooltip appears with correct units.

---

## N2-11: Safe Z vs Retract Z confusion [minor]

### Problem

The post processor panel shows "Safe Z" but the operations also have "Retract Z" (drill operations). Users may confuse the two, as "Safe Z" in the post config is the global clearance plane while "Retract Z" is the per-operation R-plane for canned cycles.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/properties/post.rs` | 30-37 | Add `.on_hover_text()` to the Safe Z row |

### Current code (post.rs:30-37)

```rust
ui.label("Safe Z:");
ui.add(
    egui::DragValue::new(&mut post.safe_z)
        .suffix(" mm")
        .speed(0.5)
        .range(0.0..=500.0),
);
ui.end_row();
```

### Proposed change

Add hover text to both the label and the drag value. The label can use `.on_hover_text()`:

```rust
ui.label("Safe Z:").on_hover_text(
    "Global clearance plane for rapid moves between operations. \
     The tool rapids to this height before traversing to the next cut. \
     Different from per-operation Retract Z, which is the R-plane \
     for drill canned cycles."
);
ui.add(
    egui::DragValue::new(&mut post.safe_z)
        .suffix(" mm")
        .speed(0.5)
        .range(0.0..=500.0),
).on_hover_text(
    "Global clearance plane for rapid moves between operations. \
     Different from per-operation Retract Z (R-plane for drill cycles)."
);
ui.end_row();
```

Also update the existing `tooltip_for` entry for "Retract Z" (mod.rs:2122) to clarify:

Current:
```rust
"Retract Z" => "R-plane height: rapid down to here, then feed into material.",
```

Change to:
```rust
"Retract Z" => "R-plane height for this drill cycle: rapid down to here, then feed into material. Different from global Safe Z in Post Processor.",
```

### Edge cases / risks

- None. Purely informational additions.

### How to verify

- Launch GUI. Select Post Processor. Hover "Safe Z:" label. Confirm tooltip explains the difference from Retract Z.
- Select a drill operation. Hover "Retract Z:" value. Confirm tooltip mentions distinction from Safe Z.

---

## N5-07: VizError messages lack corrective hints [minor]

### Problem

Error toast messages describe what failed but never suggest what the user should do about it.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/error.rs` | 37-48 | Add corrective hints to `user_message()` output |

### Current code (error.rs:37-48)

```rust
pub fn user_message(&self) -> String {
    match self {
        Self::StlImport(e) => format!("Failed to import STL file: {e}"),
        Self::SvgImport(e) => format!("Failed to import SVG file: {e}"),
        Self::DxfImport(e) => format!("Failed to import DXF file: {e}"),
        Self::StepImport(e) => format!("Failed to import STEP file: {e}"),
        Self::ProjectSave(msg) => format!("Save failed: {msg}"),
        Self::ProjectLoad(msg) => format!("Load failed: {msg}"),
        Self::Export(msg) => format!("Export failed: {msg}"),
        Self::Other(msg) => msg.clone(),
    }
}
```

### Proposed change

```rust
pub fn user_message(&self) -> String {
    match self {
        Self::StlImport(e) => format!(
            "Failed to import STL file: {e}\n\
             Hint: Ensure the file is a valid binary or ASCII STL. Try re-exporting from your CAD program."
        ),
        Self::SvgImport(e) => format!(
            "Failed to import SVG file: {e}\n\
             Hint: Ensure the SVG contains path elements. Text and embedded images are not supported."
        ),
        Self::DxfImport(e) => format!(
            "Failed to import DXF file: {e}\n\
             Hint: Use DXF R12-R2018 format. Ensure geometry is in the XY plane with polylines or lines."
        ),
        Self::StepImport(e) => format!(
            "Failed to import STEP file: {e}\n\
             Hint: Use AP203 or AP214 STEP format. Ensure the file contains solid body geometry."
        ),
        Self::ProjectSave(msg) => format!(
            "Save failed: {msg}\n\
             Hint: Check that the target directory exists and you have write permissions."
        ),
        Self::ProjectLoad(msg) => format!(
            "Load failed: {msg}\n\
             Hint: Ensure the .toml file is a valid rs_cam job. Check that referenced model files still exist at their original paths."
        ),
        Self::Export(msg) => format!(
            "Export failed: {msg}\n\
             Hint: Ensure all toolpaths are generated and have valid tool assignments."
        ),
        Self::Other(msg) => msg.clone(),
    }
}
```

### Edge cases / risks

- The hint text is appended with `\n`. Verify that the notification/toast display system handles newlines. If toasts are single-line, the hint will appear on the same line separated by a newline character, which egui `Label` renders correctly (it wraps).
- `Other` variant is left as-is since it's a catch-all with no predictable content.
- The hints are generic. For specific sub-errors (e.g., "file not found" vs "parse error"), the inner error `{e}` already provides detail; the hint covers the most common resolution.

### How to verify

- Attempt to import a corrupt/invalid STL file. Confirm the error toast includes the hint text.
- Attempt to load a nonexistent .toml job path. Confirm hint appears.
- Test each import format with an invalid file if feasible, or verify by code review that the format strings compile.

---

## N5-15: Empty job has no onboarding hint [polish]

### Problem

When no item is selected and the job is empty (no models, no toolpaths), the properties panel shows only "Select an item in the project tree" -- which is unhelpful since the tree is also empty.

### File to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/rs_cam_viz/src/ui/properties/mod.rs` | 117-124 | Expand the `Selection::None` arm to show onboarding when job is empty |

### Current code (mod.rs:117-124)

```rust
Selection::None => {
    ui.label(
        egui::RichText::new("Select an item in the project tree")
            .italics()
            .color(egui::Color32::from_rgb(120, 120, 130)),
    );
}
```

### Proposed change

```rust
Selection::None => {
    if state.job.models.is_empty() && state.job.all_toolpaths().next().is_none() {
        // Empty job — show getting-started guidance
        ui.add_space(16.0);
        ui.heading("Getting Started");
        ui.add_space(8.0);
        ui.label("1. Import a model (File > Import STL/SVG/DXF/STEP)");
        ui.label("2. Configure stock dimensions and material");
        ui.label("3. Add a tool in the project tree");
        ui.label("4. Add a toolpath operation");
        ui.label("5. Generate toolpaths and simulate");
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("Tip: Press Ctrl+O to open a saved job")
                .small()
                .italics()
                .color(egui::Color32::from_rgb(140, 140, 150)),
        );
    } else {
        ui.label(
            egui::RichText::new("Select an item in the project tree")
                .italics()
                .color(egui::Color32::from_rgb(120, 120, 130)),
        );
    }
}
```

### Edge cases / risks

- `state.job.all_toolpaths()` is an iterator method. If the job has setups but no toolpaths, `next()` returns `None`. The check covers the "truly empty" case.
- After the user imports a model, this panel auto-switches because the selection changes (the import handler typically selects the new model). If it somehow stays at `Selection::None`, the else branch kicks in with the standard message.
- The numbered list is plain text. No interactive elements (buttons would add complexity and potentially conflict with the existing import-via-menu flow).

### How to verify

- Launch GUI with no job file. Confirm the properties panel shows the "Getting Started" heading and numbered steps.
- Import a model file. Confirm the getting-started panel disappears (replaced by model properties or the standard hint).
- Open an existing job. Confirm the getting-started panel is not shown.

---

## Summary of all changes

| Item | Severity | Files touched | New files |
|------|----------|---------------|-----------|
| N5-05 + N1-01: Help menu + shortcuts | major | `state/mod.rs`, `ui/menu_bar.rs`, `ui/mod.rs`, `app.rs` | `ui/shortcuts_window.rs` |
| N2-06: Missing tooltips | minor | `ui/properties/mod.rs` | -- |
| N2-07 + N2-15: Undo coverage | major | `ui/properties/mod.rs`, `state/job.rs` | -- |
| N1-06: Click-to-deselect | minor | `app.rs` | -- |
| N5-06: Ctrl+O shortcut | minor | `ui/menu_bar.rs` | -- |
| N2-10: Kc/Hardness tooltips | minor | `ui/properties/stock.rs` | -- |
| N2-11: Safe Z tooltip | minor | `ui/properties/post.rs`, `ui/properties/mod.rs` | -- |
| N5-07: Error corrective hints | minor | `error.rs` | -- |
| N5-15: Empty job onboarding | polish | `ui/properties/mod.rs` | -- |

Estimated implementation order (by dependency and risk):
1. N1-06 (1 line change, zero risk)
2. N5-06 (3 lines, zero risk)
3. N2-10 (2 hover texts, zero risk)
4. N2-11 (2 hover texts, zero risk)
5. N2-06 (13 tooltip entries, zero risk)
6. N5-15 (small UI addition, zero risk)
7. N5-07 (string changes only, low risk)
8. N2-07 + N2-15 (PartialEq derive + comparison fix, needs verification that inner types support PartialEq)
9. N5-05 + N1-01 (new file + new event variant + state field, most code)
