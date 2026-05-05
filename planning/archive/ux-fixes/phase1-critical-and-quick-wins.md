# Phase 1: Critical Fixes and Quick Wins

Implementation plan for 5 UX items (2 critical, 2 major, 1 minor batch).

---

## U-01: Scale box loses focus on every keystroke [critical]

### Problem

The custom scale `DragValue` at `crates/rs_cam_viz/src/ui/properties/mod.rs:533-547`
rebuilds with a new widget ID each frame because `RescaleModel` fires on every
`.changed()`, which mutates the model's `ModelUnits`, which changes the value
bound to the `DragValue`, which causes egui to treat it as a new widget.

The `ComboBox` on line 514 avoids this by using `.from_id_salt("model_units")`,
giving it a stable identity.

### Files to modify

- `crates/rs_cam_viz/src/ui/properties/mod.rs` lines 537-542

### Current code (lines 537-542)

```rust
if ui
    .add(
        egui::DragValue::new(&mut custom_scale)
            .speed(0.1)
            .range(0.001..=100000.0),
    )
    .changed()
```

### Fix

Add `.id_source("custom_model_scale")` to give the `DragValue` a stable widget ID:

```rust
if ui
    .add(
        egui::DragValue::new(&mut custom_scale)
            .speed(0.1)
            .range(0.001..=100000.0)
            .id_source("custom_model_scale"),
    )
    .changed()
```

Note: in egui 0.30, `DragValue` does not have `.id_source()` directly. The
correct approach is to wrap with `ui.push_id("custom_model_scale", ...)` or use
`egui::DragValue::new(...).id(egui::Id::new("custom_model_scale"))`. Check the
egui 0.30 DragValue API. If neither is available, wrap the `.add()` call in
`ui.push_id`:

```rust
ui.push_id("custom_model_scale", |ui| {
    if ui
        .add(
            egui::DragValue::new(&mut custom_scale)
                .speed(0.1)
                .range(0.001..=100000.0),
        )
        .changed()
    {
        events.push(AppEvent::RescaleModel(id, ModelUnits::Custom(custom_scale)));
    }
});
```

However, note that `.push_id` changes the inner response scope. Since the event
push is inside the closure already, this works. The outer `ui.horizontal` already
provides layout context.

### Edge cases

- If multiple models are open, each needs a unique salt. Since the whole block
  is rendered per-model and `id` (a `ModelId`) is in scope, use
  `ui.push_id(("custom_scale", id), ...)` to salt by model ID.
- Verify that typing a decimal value like `25.4` works without the cursor
  jumping or the field losing focus mid-edit.

### Verification

1. Run the GUI: `cargo run -p rs_cam_viz --bin rs_cam_gui`
2. Import any STL, select the model in the tree.
3. Click the custom scale field, type `25.4` character by character.
4. Confirm the cursor stays in the field and the value updates only on
   commit (Enter or click-away), not on each keystroke.

---

## N5-01: No unsaved-changes protection on Quit [critical]

### Problem

`AppEvent::Quit` at `crates/rs_cam_viz/src/app.rs:346` immediately sends
`ViewportCommand::Close` without checking whether the job has unsaved changes
(`job.dirty`). The OS window-close button (X) also needs interception.

### Files to modify

1. `crates/rs_cam_viz/src/app.rs` -- struct field, handle_events, update, new dialog render

### Implementation plan

#### Step 1: Add state flag to `RsCamApp` (line 28)

```rust
pub struct RsCamApp {
    controller: AppController,
    camera: OrbitCamera,
    viewport_rect: egui::Rect,
    pending_checkpoint_load: bool,
    auto_screenshot_frame: Option<u32>,
    last_hover_face: Option<rs_cam_core::enriched_mesh::FaceGroupId>,
    show_quit_dialog: bool,    // <-- NEW
}
```

Initialize to `false` in `RsCamApp::new`.

#### Step 2: Gate the Quit event (line 346)

Replace:

```rust
AppEvent::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
```

With:

```rust
AppEvent::Quit => {
    if self.controller.state().job.dirty {
        self.show_quit_dialog = true;
    } else {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
```

#### Step 3: Intercept OS close button in `update()`

At the top of `fn update()` (inside the `impl eframe::App` block, around
line 2163), add close-request interception. In eframe 0.30, the OS close
event is detected via `ctx.input(|i| i.viewport().close_requested())` and
cancelled via `ViewportCommand::CancelClose`:

```rust
// Intercept OS window close when there are unsaved changes
let os_close_requested = ctx.input(|i| i.viewport().close_requested());
if os_close_requested && self.controller.state().job.dirty {
    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    self.show_quit_dialog = true;
}
```

Place this **before** the `handle_events` call so the dialog flag is set
before UI rendering.

#### Step 4: Render the quit confirmation dialog

Add a method `fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context)`
and call it at the end of `update()`, before the repaint logic:

```rust
fn show_unsaved_changes_dialog(&mut self, ctx: &egui::Context) {
    if !self.show_quit_dialog {
        return;
    }
    egui::Window::new("Unsaved Changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("You have unsaved changes. What would you like to do?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save & Quit").clicked() {
                    // Trigger save, then close
                    let path = self.controller.state().job.file_path.clone().or_else(|| {
                        rfd::FileDialog::new()
                            .add_filter("TOML Job", &["toml"])
                            .set_file_name("job.toml")
                            .save_file()
                    });
                    if let Some(path) = path {
                        let _ = self.controller.save_job_to_path(&path);
                    }
                    self.show_quit_dialog = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Discard & Quit").clicked() {
                    self.show_quit_dialog = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    self.show_quit_dialog = false;
                }
            });
        });
}
```

### Edge cases

- If `file_path` is `None` and the user cancels the Save dialog, do NOT
  close -- leave `show_quit_dialog = true` so they can try again or pick
  Discard/Cancel.
- The auto-screenshot close path (line 2336) should bypass the dialog since
  it is not user-initiated. That path uses `ViewportCommand::Close` directly
  and `dirty` is typically false in screenshot mode, so no change needed.
- Keyboard shortcut (Ctrl+Q) also emits `AppEvent::Quit` via the menu bar,
  so it is covered by Step 2 automatically.

### Verification

1. Run GUI, import a model (marks job dirty).
2. Press Ctrl+Q or click window X button.
3. Confirm the dialog appears with three buttons.
4. Click Cancel -- dialog closes, app stays open.
5. Click Discard & Quit -- app closes without saving.
6. Repeat, click Save & Quit -- confirm file dialog appears, job is saved, app closes.
7. Open a fresh session (no changes), press Ctrl+Q -- app closes immediately (no dialog).

---

## N5-02/03/04/09/14: Silent failures -> push_notification [major, batch fix]

### Problem

Multiple code paths log errors or warnings via `tracing::error!` / `tracing::warn!` /
`tracing::info!` but never show a toast notification to the user. The user sees
nothing unless they have a terminal open.

### Files to modify

1. `crates/rs_cam_viz/src/app.rs` -- export functions
2. `crates/rs_cam_viz/src/controller/events.rs` -- tool/model/toolpath validation and compute results

### Inventory of silent locations and planned fixes

Each entry below shows: location, current tracing call, and the replacement.
Use `self.controller.push_notification(message, Severity)` in `app.rs` methods
(where `self` is `RsCamApp`) or `self.push_notification(message, Severity)` in
`controller/events.rs` methods (where `self` is `AppController`).

#### A. `app.rs` -- export_gcode_with_summary (lines 712-719)

| Line | Current | Replacement |
|------|---------|-------------|
| 713 | `tracing::error!("Failed to write G-code: {}", e)` | `self.controller.push_notification(format!("Failed to write G-code: {e}"), Severity::Error)` |
| 715 | `tracing::info!("Exported G-code to {}", path.display())` | `self.controller.push_notification(format!("Exported G-code to {}", path.display()), Severity::Info)` |
| 719 | `tracing::error!("Export failed: {error}")` | `self.controller.push_notification(format!("Export failed: {error}"), Severity::Error)` |

Note: `export_gcode_with_summary` currently takes `&self`. To call
`push_notification` it needs `&mut self`. Change signature to `fn export_gcode_with_summary(&mut self)`.

#### B. `app.rs` -- export_svg_preview (lines 731-738)

| Line | Current | Replacement |
|------|---------|-------------|
| 732 | `tracing::error!("Failed to write SVG: {error}")` | `self.controller.push_notification(format!("Failed to write SVG: {error}"), Severity::Error)` |
| 734 | `tracing::info!("Exported SVG preview to {}", path.display())` | `self.controller.push_notification(format!("Exported SVG to {}", path.display()), Severity::Info)` |
| 738 | `tracing::warn!("{error}")` | `self.controller.push_notification(format!("SVG export failed: {error}"), Severity::Warning)` |

Same `&self` -> `&mut self` change needed.

#### C. `app.rs` -- export setup sheet (lines 310-312)

| Line | Current | Replacement |
|------|---------|-------------|
| 310 | `tracing::error!("Failed to write setup sheet: {error}")` | `self.controller.push_notification(format!("Failed to write setup sheet: {error}"), Severity::Error)` |
| 312 | `tracing::info!("Exported setup sheet to {}", path.display())` | `self.controller.push_notification(format!("Exported setup sheet to {}", path.display()), Severity::Info)` |

This is inside `handle_events` where `self` is `&mut RsCamApp`, so no signature change needed.

#### D. `controller/events.rs` -- tool removal blocked (line 234)

| Line | Current | Replacement |
|------|---------|-------------|
| 234 | `tracing::warn!("Cannot remove tool {:?}: still referenced by one or more toolpaths", tool_id)` | `self.push_notification("Cannot remove tool: still referenced by one or more toolpaths".into(), Severity::Warning)` |

#### E. `controller/events.rs` -- model removal blocked (line 420)

| Line | Current | Replacement |
|------|---------|-------------|
| 420 | `tracing::warn!("Cannot remove model {:?}: still referenced by one or more toolpaths", model_id)` | `self.push_notification("Cannot remove model: still referenced by one or more toolpaths".into(), Severity::Warning)` |

#### F. `controller/events.rs` -- add toolpath guards (lines 452, 457)

| Line | Current | Replacement |
|------|---------|-------------|
| 452 | `tracing::warn!("Cannot add toolpath: no tools defined")` | `self.push_notification("Cannot add toolpath: define a tool first".into(), Severity::Warning)` |
| 457 | `tracing::warn!("Cannot add {} toolpath: import geometry first", operation.label())` | `self.push_notification(format!("Cannot add {} toolpath: import geometry first", operation.label()), Severity::Warning)` |

#### G. `controller/events.rs` -- simulation guards (lines 797, 811)

| Line | Current | Replacement |
|------|---------|-------------|
| 797 | `tracing::warn!("No computed toolpaths to simulate")` | `self.push_notification("No computed toolpaths to simulate".into(), Severity::Warning)` |
| 811 | `tracing::warn!("No computed toolpaths to simulate")` | `self.push_notification("No computed toolpaths to simulate".into(), Severity::Warning)` |

#### H. `controller/events.rs` -- collision check guard (line 864)

| Line | Current | Replacement |
|------|---------|-------------|
| 864 | `tracing::warn!("No toolpath with STL mesh available for collision check")` | `self.push_notification("No toolpath with mesh available for collision check".into(), Severity::Warning)` |

#### I. `controller/events.rs` -- compute result errors (lines 1431-1463)

| Line | Current | Replacement |
|------|---------|-------------|
| 1433 | `tracing::error!("Simulation failed: {error}")` | `self.push_notification(format!("Simulation failed: {error}"), Severity::Error)` |
| 1440 | `tracing::info!("No holder clearance issues detected")` | `self.push_notification("No holder clearance issues detected".into(), Severity::Info)` |
| 1442-1446 | `tracing::warn!("{} holder clearance issues, min safe stickout: {:.1} mm", ...)` | `self.push_notification(format!("{count} holder clearance issues, min safe stickout: {:.1} mm", collision.report.min_safe_stickout), Severity::Warning)` |
| 1461 | `tracing::error!("Collision check failed: {error}")` | `self.push_notification(format!("Collision check failed: {error}"), Severity::Error)` |

### Import needed

Add `use crate::controller::Severity;` at the top of `app.rs` if not already
present (check existing `crate::controller::Severity::Error` references at
lines 246, 286 -- these use the full path, so either add a `use` or continue
with the full path for consistency).

### Edge cases

- Keep the `tracing::` calls alongside the toast calls (log AND toast) so
  that terminal users and log files still get the messages. Pattern:
  ```rust
  tracing::error!("Failed to write G-code: {e}");
  self.controller.push_notification(format!("Failed to write G-code: {e}"), Severity::Error);
  ```
- For `ComputeError::Cancelled` (lines 1431, 1459), do NOT add a toast --
  cancellation is intentional and silent is correct.
- The `export_gcode_with_summary` info log on line 698-705 (the summary
  stats) should remain tracing-only (it's diagnostic, not user-facing).

### Verification

1. Run GUI with no tools defined. Click "Add Toolpath" in the toolbar.
   Confirm a warning toast appears: "Cannot add toolpath: define a tool first".
2. Add a tool, add a toolpath. Try to remove the tool.
   Confirm a warning toast: "Cannot remove tool: still referenced by one or more toolpaths".
3. Export G-code to a read-only directory. Confirm an error toast appears.
4. Successfully export G-code. Confirm an info toast with the file path.
5. Run simulation with no computed toolpaths. Confirm warning toast.

---

## N2-01/02/03: Missing unit suffixes on fixture/keep-out DragValues [minor]

### Problem

DragValues for fixture position/size and keep-out position/size in
`crates/rs_cam_viz/src/ui/properties/setup.rs` lack `.suffix(" mm")`.
The section headers say "(mm)" but the individual fields do not show units
in the drag-value widget itself. The `Clearance` field at line 402 already
has `.suffix(" mm")` -- this is the pattern to follow.

### File to modify

- `crates/rs_cam_viz/src/ui/properties/setup.rs`

### DragValues needing `.suffix(" mm")`

| Lines | Widget | Section |
|-------|--------|---------|
| 338 | `fixture.origin_x` | Fixture Position |
| 343 | `fixture.origin_y` | Fixture Position |
| 348 | `fixture.origin_z` | Fixture Position |
| 367-369 | `fixture.size_x` | Fixture Size |
| 375-378 | `fixture.size_y` | Fixture Size |
| 384-387 | `fixture.size_z` | Fixture Size |
| 444 | `zone.origin_x` | Keep-Out Position |
| 449 | `zone.origin_y` | Keep-Out Position |
| 468-470 | `zone.size_x` | Keep-Out Size |
| 476-479 | `zone.size_y` | Keep-Out Size |

Total: 10 DragValues.

### Fix pattern

For each bare `DragValue::new(&mut field).speed(0.5)`, add `.suffix(" mm")`:

**Before** (e.g., line 338):
```rust
changed |= ui
    .add(egui::DragValue::new(&mut fixture.origin_x).speed(0.5))
    .changed();
```

**After**:
```rust
changed |= ui
    .add(egui::DragValue::new(&mut fixture.origin_x).speed(0.5).suffix(" mm"))
    .changed();
```

For the ranged ones (e.g., line 367-369):
```rust
egui::DragValue::new(&mut fixture.size_x)
    .speed(0.5)
    .range(0.1..=10000.0)
    .suffix(" mm")
```

### Edge cases

- None. This is a display-only change. The suffix does not affect the stored
  value or parsing.
- The section headers already say "(mm)" and could optionally be simplified
  to just "Position" / "Size" after suffixes are added, but that is cosmetic
  and out of scope for this fix.

### Verification

1. Run GUI, select a fixture in the setup tree.
2. Confirm all position and size fields show " mm" suffix.
3. Confirm the clearance field (already had suffix) still looks correct.
4. Select a keep-out zone, confirm position and size fields show " mm".

---

## N2-05: Waterline `continuous` parameter inaccessible [major]

### Problem

`WaterlineConfig` has a `continuous: bool` field (defined at
`crates/rs_cam_viz/src/state/toolpath/configs.rs:473`) but the waterline
UI at `crates/rs_cam_viz/src/ui/properties/operations.rs:464-474` does not
expose it. The scallop UI at line 572-573 shows the pattern:

```rust
ui.label("Continuous:");
ui.checkbox(&mut cfg.continuous, "");
ui.end_row();
```

### File to modify

- `crates/rs_cam_viz/src/ui/properties/operations.rs` lines 464-474

### Current code

```rust
pub(super) fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
    egui::Grid::new("wl_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            // Z range now comes from the Heights tab (top_z / bottom_z)
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
}
```

### Fix

Add the `Continuous` checkbox after the `Sampling` row, before `draw_feed_params`:

```rust
pub(super) fn draw_waterline_params(ui: &mut egui::Ui, cfg: &mut WaterlineConfig) {
    egui::Grid::new("wl_p")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            dv(ui, "Z Step:", &mut cfg.z_step, " mm", 0.1, 0.05..=20.0);
            dv(ui, "Sampling:", &mut cfg.sampling, " mm", 0.1, 0.1..=5.0);
            ui.label("Continuous:");
            ui.checkbox(&mut cfg.continuous, "");
            ui.end_row();
            // Z range now comes from the Heights tab (top_z / bottom_z)
            draw_feed_params(ui, &mut cfg.feed_rate, &mut cfg.plunge_rate);
        });
}
```

### Edge cases

- The `continuous` field already serializes/deserializes via serde (it is in
  the `WaterlineConfig` struct with `Serialize, Deserialize`). Existing saved
  jobs that lack the field will get `false` from the `Default` impl. No
  migration needed.
- The core waterline implementation must actually respect this flag. Check
  whether the waterline generator in `rs_cam_core` reads `continuous`. If it
  does not, this UI addition is still correct (it sets the config value) and
  the core wiring is a separate task. The field exists in the config struct
  so it was intended to be used.

### Verification

1. Run GUI, add a Waterline toolpath.
2. In the operation parameters panel, confirm "Continuous:" checkbox appears
   between "Sampling" and the feed rate fields.
3. Toggle it on/off, recompute the toolpath, confirm the generated toolpath
   changes behavior (fewer retracts when continuous is on).
4. Save and reload the job, confirm the `continuous` setting persists.
