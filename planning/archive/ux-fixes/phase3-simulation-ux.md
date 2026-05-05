# Phase 3: Simulation UX Fixes

Implementation plan for 8 items from the UI/UX review of the simulation workspace.

---

## U-04: Combine scrubber + boundary timeline [major]

### Files
- `crates/rs_cam_viz/src/ui/sim_timeline.rs` lines 104-306

### Current code

The timeline bottom panel draws three rows via separate functions:

1. **`draw_transport_and_scrubber`** (lines 105-211): A `ui.horizontal` row containing transport buttons (|<, <, play, >, >|), a `ui.separator()`, then an `egui::Slider` sized to `(available_width - 160, 18px)` with `show_value(false)`, then a time display. Below that, an optional multi-setup row with clickable setup buttons.

2. **`draw_boundary_timeline`** (lines 215-306+): Allocates a custom-painted rect `(total_width, 12px)` with `Sense::click`. Paints per-op color segments (dim background + bright fill for progress), collision markers (red/orange lines), and a white playhead line. Click-to-seek sets `current_move` from fractional position.

3. **`draw_speed_controls`** (lines 316-350): Speed presets + DragValue + keyboard hint text.

The scrubber slider (row 1) and boundary bar (row 2) both control the same value (`sim.playback.current_move`) but render separately, with ~30px of vertical gap between them.

### Proposed change

Replace the separate slider and boundary bar with a single combined widget:

1. **Keep transport buttons** in their own `ui.horizontal` row, ending with the time display. Remove the `egui::Slider` from this row entirely.

2. **New combined timeline widget** replaces both the old slider and the old boundary bar. Implementation:

   ```
   fn draw_combined_timeline(ui, sim, job, current_boundary, active_semantic, events) {
       let total_width = ui.available_width();
       let height = 26.0; // was 12px bar + 18px slider; now one 26px widget
       let (rect, response) = ui.allocate_exact_size(
           vec2(total_width, height),
           Sense::click_and_drag(),  // was Sense::click for bar, slider handled drag separately
       );
       let painter = ui.painter_at(rect);

       // 1. Paint per-op boundary colors as background (same as current draw_boundary_timeline)
       //    Paint dim color for full segment, bright color for progress-filled portion

       // 2. Paint collision markers (holder + rapid) as vertical lines

       // 3. Paint white playhead line at current_move position

       // 4. Handle click AND drag: on pointer_pos, compute frac, set current_move
       //    Use response.dragged() || response.clicked() instead of just clicked()
       //    On drag, also pause playback

       // 5. Hover tooltip: show operation name + move number at hovered position
   }
   ```

3. **Interaction model**: Use `Sense::click_and_drag()`. On `response.clicked() || response.dragged()`, compute fractional position and update `sim.playback.current_move`. Pause playback on any interaction. This gives slider-like drag behavior without an actual `egui::Slider`.

4. **Visual details**:
   - Total height 26px (slight increase from 12px bar, but eliminates the 18px slider row)
   - Rounded corners (rounding 3.0 instead of 1.0) for the overall rect
   - A subtle 1px border on the rect (`egui::Color32::from_rgb(60, 60, 70)`)
   - Playhead: 2px white line with a small 6px-wide triangle/diamond handle at top

5. **Setup buttons row** stays unchanged below the combined widget (only shown for multi-setup jobs).

6. **Delete** the old `draw_transport_and_scrubber` slider section (lines 148-158) and fold `draw_boundary_timeline` into the new function. The `draw_transport_and_scrubber` function becomes transport buttons + time display only.

7. The semantic band (debug mode, drawn by `draw_semantic_band`) stays as a separate row below the combined timeline -- it operates on a per-op local move range, which is conceptually different from the global timeline.

### Edge cases
- Zero total moves: show empty rect with disabled appearance (same as current slider guard `if sim.total_moves() > 0`)
- Single-op jobs: entire bar is one color, still works
- Very small ops: segments may be < 1px; clamp segment width to min 1px (current code already does this implicitly)
- Drag beyond rect bounds: clamp frac to 0.0..=1.0 (already done in current click handler)
- The semantic band click handler in `draw_boundary_timeline` lines 308-612 must stay as its own row and is unaffected

### Verification
- `cargo clippy --workspace --all-targets -- -D warnings`
- Run GUI, load a multi-op job, enter simulation workspace
- Confirm: one unified bar replaces the old slider + boundary bar
- Drag across the bar: playhead follows mouse, playback pauses, move counter updates
- Click specific position: jumps to that move
- Collision markers still visible
- Per-op colors still paint correctly
- Transport buttons still work (play/pause, step, jump)
- Time display still updates
- Debug semantic band still appears below when debug enabled

---

## N3-01: Staleness warning too subtle [major]

### Files
- `crates/rs_cam_viz/src/ui/sim_op_list.rs` lines 71-83

### Current code

```rust
// Staleness warning
if sim.is_stale(job.edit_counter) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("\u{26A0} Results may be stale")
                .color(egui::Color32::from_rgb(220, 180, 60)),
        );
    });
    if ui.small_button("Re-run").clicked() {
        events.push(AppEvent::RunSimulation);
    }
    ui.separator();
}
```

Plain amber text on no background, a tiny `small_button("Re-run")` on its own line below, then a separator.

### Proposed change

Replace with an amber-bordered frame containing the warning and a full-width button:

```rust
if sim.is_stale(job.edit_counter) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(50, 42, 20))       // dark amber background
        .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 180, 60)))  // amber border
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{26A0} Results may be stale")
                        .strong()
                        .color(egui::Color32::from_rgb(220, 180, 60)),
                );
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Parameters changed since the last simulation run.")
                    .small()
                    .color(egui::Color32::from_rgb(180, 160, 100)),
            );
            ui.add_space(6.0);
            let btn = egui::Button::new(
                egui::RichText::new("Re-run Simulation").strong()
            )
            .min_size(egui::vec2(ui.available_width(), 28.0));
            if ui.add(btn).clicked() {
                events.push(AppEvent::RunSimulation);
            }
        });
    ui.add_space(4.0);
    ui.separator();
}
```

Key changes:
- Amber-bordered `Frame` with dark amber fill -- visually unmissable
- Warning text is `.strong()` for emphasis
- Added explanatory sub-text
- Full-width "Re-run Simulation" button with `.min_size` spanning available width and 28px height
- Removes the old `small_button` in favor of the prominent button

### Edge cases
- When not stale, this block is skipped entirely (no change)
- The frame adds ~50px of height; the panel is in a `ScrollArea` via the left side panel so this is fine
- The `edit_counter` check is already robust (existing logic)

### Verification
- Load a job, run simulation, then change a parameter (e.g., feed rate) to trigger staleness
- Confirm: amber-bordered box appears at top of Verification panel
- "Re-run Simulation" button spans full width and triggers `RunSimulation` event
- After re-running, the warning disappears

---

## N3-10: "Export Anyway" needs visual alarm [major]

### Files
- `crates/rs_cam_viz/src/ui/preflight.rs` lines 150-167

### Current code

```rust
let has_failures =
    sim.checks.holder_collision_count > 0 || !sim.checks.rapid_collisions.is_empty();

ui.horizontal(|ui| {
    let export_label = if has_failures {
        "Export Anyway"
    } else {
        "Export G-code"
    };
    if ui.button(export_label).clicked() {
        events.push(AppEvent::ExportGcodeConfirmed);
        still_open = false;
    }

    if ui.button("Cancel").clicked() {
        still_open = false;
    }
});
```

When `has_failures` is true, the button says "Export Anyway" but uses the default button style -- same size, same color as the safe "Export G-code" path. No confirmation gate.

### Proposed change

Add state tracking for a confirmation checkbox and style the button dangerously:

1. **Add a `confirm_export_with_failures` field** to track checkbox state. Since `preflight::draw` is a stateless function called each frame, use a local `static` or pass state through. The cleanest approach: add a `pub preflight_confirmed: bool` field to `SimulationState` (or `AppState`), defaulting to `false`, reset when the preflight dialog opens.

   In `crates/rs_cam_viz/src/state/simulation.rs`, add to `SimulationState`:
   ```rust
   /// User has acknowledged failures in preflight and confirmed export intent.
   pub preflight_export_confirmed: bool,
   ```
   Default to `false`. Reset it to `false` in the code path that opens the preflight dialog.

2. **Modify preflight.rs lines 150-167**:

   ```rust
   let has_failures =
       sim.checks.holder_collision_count > 0 || !sim.checks.rapid_collisions.is_empty();

   if has_failures {
       ui.add_space(4.0);
       // Warning banner
       egui::Frame::default()
           .fill(egui::Color32::from_rgb(60, 25, 25))
           .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 60, 60)))
           .inner_margin(8.0)
           .rounding(4.0)
           .show(ui, |ui| {
               ui.label(
                   egui::RichText::new("\u{26A0} Exporting with unresolved failures")
                       .strong()
                       .color(egui::Color32::from_rgb(220, 100, 80)),
               );
               ui.add_space(4.0);
               // NOTE: sim must be made mutable for this -- see note below
               ui.checkbox(
                   &mut state.simulation_preflight_confirmed,
                   "I understand the risks and want to export anyway",
               );
           });
       ui.add_space(4.0);
   }

   ui.horizontal(|ui| {
       if has_failures {
           // Red-styled button, disabled until checkbox is checked
           let confirmed = state.simulation_preflight_confirmed;
           let btn = egui::Button::new(
               egui::RichText::new("Export Anyway")
                   .strong()
                   .color(if confirmed {
                       egui::Color32::WHITE
                   } else {
                       egui::Color32::from_rgb(140, 100, 100)
                   }),
           )
           .fill(if confirmed {
               egui::Color32::from_rgb(180, 50, 40)
           } else {
               egui::Color32::from_rgb(80, 40, 40)
           });

           let response = ui.add_enabled(confirmed, btn);
           if response.clicked() {
               events.push(AppEvent::ExportGcodeConfirmed);
               still_open = false;
           }
       } else {
           if ui.button("Export G-code").clicked() {
               events.push(AppEvent::ExportGcodeConfirmed);
               still_open = false;
           }
       }

       if ui.button("Cancel").clicked() {
           still_open = false;
       }
   });
   ```

3. **Mutability**: The `draw` function signature currently takes `state: &AppState`. The confirmation checkbox needs `&mut` access. Options:
   - **Option A (preferred)**: Change the `preflight_export_confirmed` to live in a `Cell<bool>` or `RefCell<bool>` on `AppState` so it can be mutated through `&AppState`.
   - **Option B**: Change the `draw` signature to take `&mut AppState`. This requires updating the call site in `app.rs`.
   - **Option C**: Use a local `egui::Memory` key via `ui.data_mut(|d| ...)` to store the bool in egui's per-frame state, keyed by `Id::new("preflight_confirm")`. This avoids struct changes entirely.

   Option C is the simplest and most localized. The checkbox state persists across frames via egui's memory and automatically cleans up when the window closes.

   ```rust
   let confirm_id = egui::Id::new("preflight_export_confirm");
   let mut confirmed = ui.data(|d| d.get_temp::<bool>(confirm_id).unwrap_or(false));
   ui.checkbox(&mut confirmed, "I understand the risks and want to export anyway");
   ui.data_mut(|d| d.insert_temp(confirm_id, confirmed));
   ```

### Edge cases
- No failures: normal "Export G-code" button, no red styling, no checkbox
- Dialog reopened after closing: egui temp data resets when ID is gone -- user must re-check. This is desirable.
- Multiple failure types (both rapid + holder): same single checkbox covers all

### Verification
- Run simulation with a known collision, open Export Readiness dialog
- Confirm: red warning frame appears, checkbox unchecked, "Export Anyway" button disabled/dimmed
- Check the checkbox: button becomes bright red and clickable
- Click "Export Anyway": export proceeds
- Open dialog with no failures: normal green "Export G-code" button, no checkbox

---

## N3-02: Per-op checkbox re-runs sim immediately [minor]

### Files
- `crates/rs_cam_viz/src/ui/sim_op_list.rs` lines 286-314

### Current code

```rust
// If a checkbox was toggled, re-run sim with new selection
if let Some(id) = toggled_id {
    let mut new_selection: Vec<ToolpathId> = if all_selected {
        sim.boundaries().iter().map(|b| b.id).filter(|bid| *bid != id).collect()
    } else {
        let mut s = selected_set;
        if s.contains(&id) { s.retain(|x| *x != id); } else { s.push(id); }
        s
    };
    if new_selection.len() == boundaries.len() { new_selection.clear(); }
    if new_selection.is_empty() {
        events.push(AppEvent::RunSimulation);
    } else {
        events.push(AppEvent::RunSimulationWith(new_selection));
    }
}
```

Every checkbox toggle immediately fires `RunSimulation` or `RunSimulationWith`. If the user toggles 5 ops, that is 5 simulation runs, each one taking potentially seconds.

### Proposed change

Decouple the toggle from execution. Store the pending selection in `SimulationState` and show an explicit "Re-run with selection" button.

1. **Add field to `SimulationState`**:
   ```rust
   /// Pending toolpath selection that differs from the last-run selection.
   /// None = no pending change; Some(vec) = user has toggled checkboxes but not re-run.
   pub pending_selection: Option<Vec<ToolpathId>>,
   ```

2. **Modify sim_op_list.rs checkbox toggle block** (lines 286-314):
   Replace the immediate `events.push(AppEvent::RunSimulation*)` with storing the pending selection:

   ```rust
   if let Some(id) = toggled_id {
       // Build the new desired selection
       let mut new_selection: Vec<ToolpathId> = if all_selected {
           sim.boundaries().iter().map(|b| b.id).filter(|bid| *bid != id).collect()
       } else {
           // Start from pending if it exists, otherwise from current
           let mut s = sim.pending_selection.clone().unwrap_or(selected_set);
           if s.contains(&id) { s.retain(|x| *x != id); } else { s.push(id); }
           s
       };
       if new_selection.len() == boundaries.len() { new_selection.clear(); }
       sim.pending_selection = Some(new_selection);
   }
   ```

3. **Add a "Re-run with selection" button** after the op list loop, shown when `pending_selection.is_some()`:

   ```rust
   if let Some(ref pending) = sim.pending_selection {
       ui.add_space(6.0);
       egui::Frame::default()
           .fill(egui::Color32::from_rgb(36, 42, 55))
           .inner_margin(8.0)
           .rounding(4.0)
           .show(ui, |ui| {
               let count = pending.len();
               let label = if count == 0 {
                   "Re-run (all operations)".to_string()
               } else {
                   format!("Re-run with {} operations", count)
               };
               let btn = egui::Button::new(egui::RichText::new(label).strong())
                   .min_size(egui::vec2(ui.available_width(), 28.0));
               if ui.add(btn).clicked() {
                   if pending.is_empty() {
                       events.push(AppEvent::RunSimulation);
                   } else {
                       events.push(AppEvent::RunSimulationWith(pending.clone()));
                   }
                   sim.pending_selection = None;
               }
           });
   }
   ```

4. **Clear pending on actual run**: In the event handler for `RunSimulation` / `RunSimulationWith` (wherever those events are consumed), set `sim.pending_selection = None`.

5. **Checkbox checked state**: Update the `checked` calculation (line 143) to reflect pending selection when present:
   ```rust
   let effective_selected = sim.pending_selection.as_ref();
   let mut checked = if let Some(pending) = effective_selected {
       pending.is_empty() || pending.contains(&boundary.id)
   } else {
       all_selected || selected_set.contains(&boundary.id)
   };
   ```

### Edge cases
- User toggles checkboxes then navigates away: `pending_selection` persists until cleared. Consider clearing it on workspace switch, or just leave it (low risk).
- User toggles then clicks "Re-run" in the top bar (not the new button): that fires `RunSimulation` (all ops). Should we apply the pending selection? Probably not -- the top-bar "Re-run" means "re-run with current settings". Clear `pending_selection` on any simulation run.
- Empty pending list (all toggled back to full): treat as "all", fire `RunSimulation`.

### Verification
- Enter simulation, toggle an op checkbox
- Confirm: no immediate simulation run, "Re-run with N operations" button appears
- Toggle more checkboxes: button count updates, no sim runs
- Click the re-run button: simulation runs with the selected ops
- Button disappears after the run

---

## N3-13: Sim top bar 7+ ungrouped checkboxes [minor]

### Files
- `crates/rs_cam_viz/src/app.rs` lines 2058-2090

### Current code

```rust
egui::TopBottomPanel::top("sim_top_bar").show(ctx, |ui| {
    ui.horizontal(|ui| {
        {
            let (simulation, viewport, _) = self.controller.simulation_viewport_and_events_mut();
            ui.checkbox(&mut viewport.show_cutting, "Paths");
            ui.checkbox(&mut viewport.show_stock, "Stock");
            ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
            ui.checkbox(&mut viewport.show_collisions, "Collisions");
            ui.separator();
            let debug_changed = ui.checkbox(&mut simulation.debug.enabled, "Debug").changed();
            if debug_changed && simulation.debug.enabled { simulation.debug.drawer_open = true; }
            ui.checkbox(&mut simulation.metric_options.enabled, "Capture Metrics")
                .on_hover_text("Capture simulation-time cutting metrics on the next run.");
            if simulation.debug.enabled {
                ui.checkbox(&mut simulation.debug.highlight_active_item, "Highlight");
            }
        }
        // ... Re-run/Reset buttons right-aligned
    });
});
```

Seven (potentially eight with Highlight) checkboxes in a flat row with a single separator. At narrow widths they may wrap or clip. Conceptually they belong to two groups: viewport display toggles and analysis/debug options.

### Proposed change

Group the checkboxes with sub-labels:

```rust
egui::TopBottomPanel::top("sim_top_bar").show(ctx, |ui| {
    ui.horizontal(|ui| {
        {
            let (simulation, viewport, _) =
                self.controller.simulation_viewport_and_events_mut();

            // --- Viewport group ---
            ui.label(
                egui::RichText::new("View:")
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 145)),
            );
            ui.checkbox(&mut viewport.show_cutting, "Paths");
            ui.checkbox(&mut viewport.show_stock, "Stock");
            ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
            ui.checkbox(&mut viewport.show_collisions, "Collisions");

            ui.separator();

            // --- Analysis group ---
            ui.label(
                egui::RichText::new("Analysis:")
                    .small()
                    .color(egui::Color32::from_rgb(130, 130, 145)),
            );
            let debug_changed = ui
                .checkbox(&mut simulation.debug.enabled, "Debug")
                .changed();
            if debug_changed && simulation.debug.enabled {
                simulation.debug.drawer_open = true;
            }
            ui.checkbox(&mut simulation.metric_options.enabled, "Metrics")
                .on_hover_text("Capture simulation-time cutting metrics on the next run.");
            if simulation.debug.enabled {
                ui.checkbox(&mut simulation.debug.highlight_active_item, "Highlight");
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Re-run").clicked() {
                self.controller.events_mut().push(AppEvent::RunSimulation);
            }
            if ui.button("Reset").clicked() {
                self.controller.events_mut().push(AppEvent::ResetSimulation);
            }
        });
    });
});
```

Key changes:
- Add "View:" and "Analysis:" sub-labels in dim small text before each group
- Shorten "Capture Metrics" to "Metrics" (tooltip explains the full meaning)
- The separator between groups is kept

### Edge cases
- Narrow windows: the horizontal row may still overflow. If this is a real concern, could switch to `ui.horizontal_wrapped`, but the top bar panel is a fixed height, so wrapping may look odd. Leave as `horizontal` for now.
- The "Highlight" checkbox only appears when Debug is on; it stays in the Analysis group.

### Verification
- Open simulation workspace
- Confirm: "View:" label before Paths/Stock/Fixtures/Collisions, separator, "Analysis:" label before Debug/Metrics
- Enable Debug: "Highlight" appears in the Analysis cluster
- All checkboxes still function correctly

---

## N3-03: Speed control hint unparseable [minor]

### Files
- `crates/rs_cam_viz/src/ui/sim_timeline.rs` lines 316-350

### Current code

```rust
ui.label(
    egui::RichText::new("[ ] speed  <- -> step  Home/End jump  Space play")
        .small()
        .color(egui::Color32::from_rgb(90, 90, 100)),
);
```

A single packed string with raw text. "[ ]" is unclear, "speed" is unclear, and the mappings are hard to parse.

### Proposed change

Remove the inline text entirely. Instead, add tooltips to the relevant transport and speed buttons. The keyboard shortcuts are already partially documented on the transport buttons (lines 112-144 have `on_hover_text` with "Jump to start (Home)", "Step back (Left)", etc.). What's missing:

1. **Delete the label** at lines 344-348 entirely.

2. **Add tooltip to the speed DragValue** (lines 336-341):
   ```rust
   ui.add(
       egui::DragValue::new(&mut sim.playback.speed)
           .range(10.0..=50000.0)
           .speed(50.0)
           .suffix(" mv/s"),
   )
   .on_hover_text("Playback speed in moves per second.\n[ and ] to decrease/increase.");
   ```

3. **Add tooltip to speed preset buttons** (lines 320-333):
   The preset buttons already communicate their purpose by their labels (100, 500, 1k, etc.). Add a group-level tooltip by wrapping them:
   ```rust
   ui.label(
       egui::RichText::new("Speed:")
           .small()
           .color(egui::Color32::from_rgb(130, 130, 145)),
   )
   .on_hover_text("Keyboard: [ and ] to change speed, Space to play/pause, Left/Right to step, Home/End to jump.");
   ```
   This replaces the existing bare `ui.label("Speed:");` at line 318.

4. Optionally, add a `(?)` help button at the end of the speed row:
   ```rust
   ui.small_button("?").on_hover_text(
       "Keyboard shortcuts:\n\
        Space - Play / Pause\n\
        Left / Right - Step backward / forward\n\
        Home / End - Jump to start / end\n\
        [ / ] - Decrease / increase speed"
   );
   ```

### Edge cases
- Users who never hover won't see the shortcuts. But the current text is essentially invisible too (dim gray, cryptic format). The transport buttons already have hover text for their individual shortcuts.

### Verification
- Open simulation, hover over the "Speed:" label or the "?" button
- Confirm: clear multi-line keyboard shortcut reference appears
- Confirm: no more raw hint text string in the speed row

---

## N3-04: Timeline scrubber unusable for large toolpaths [minor]

### Files
- `crates/rs_cam_viz/src/ui/sim_timeline.rs` lines 148-158 (current slider, to be replaced by U-04)

### Current code

The slider is `egui::Slider::new(&mut pos, 0.0..=total_moves as f32)` with `step_by(1.0)` -- a linear mapping from 0 to potentially 500k+ moves. At typical panel widths (~800px), each pixel represents ~625 moves, making precise navigation impossible.

### Analysis

This is largely addressed by U-04's combined timeline. The custom-painted bar with click-and-drag gives the same linear mapping but with per-op color context, making it easier to navigate visually. However, the fundamental problem of 1px = N hundred moves remains.

### Proposed change (additive to U-04)

1. **Per-op zoom in the semantic band**: The debug semantic band (`draw_semantic_band`, lines 417-612) already provides a zoomed-in view of the current operation's local move range. This is only visible in debug mode. Consider making the semantic band always visible (not just in debug mode) as a second row below the combined timeline. The semantic band maps `local_total` moves (just the current op's range) across the full width, giving much finer resolution.

   In `draw_boundary_timeline` (which becomes part of the combined widget after U-04), change the guard on line 308:
   ```rust
   // Before:
   if sim.debug.enabled && let Some(boundary) = current_boundary.as_ref() {
   // After:
   if let Some(boundary) = current_boundary.as_ref() {
   ```

   The semantic band will then always appear below the global timeline when there's an active operation, giving a zoomed view of the current op. In debug mode, the full semantic item coloring appears; in non-debug mode, paint a simpler single-color version with just the playhead.

2. **Conditional simplification for non-debug**: When debug is off, `draw_semantic_band` can skip the semantic item painting and annotation/issue overlays, drawing just a solid dim bar with the playhead -- essentially a zoomed scrubber for the current op. This requires branching inside `draw_semantic_band` based on `sim.debug.enabled`.

3. **Click-to-jump on the zoom bar** already works (lines 518-611).

### Edge cases
- Single-op jobs: the zoom bar is the same range as the global bar (not useful). Could hide it for single-op jobs: `if sim.boundaries().len() > 1 && let Some(boundary) = ...`.
- No current boundary (scrubbed to a gap between ops): no zoom bar shown, which is fine.

### Verification
- Load a large multi-op job (>100k total moves)
- Confirm: per-op zoom bar appears below the global timeline
- Drag on the zoom bar: fine-grained control within the current operation
- Single-op job: zoom bar hidden (same range as global, not useful)

---

## N3-05: Deviation color mode hidden, no legend [minor]

### Files
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` lines 60-120

### Current code

```rust
egui::CollapsingHeader::new("Stock Display")
    .default_open(false)
    .show(ui, |ui| {
        // ... combo box for Solid / Deviation / By Height
        // ... opacity slider
        // ... resolution slider + auto checkbox
    });
```

The "Stock Display" section is collapsed by default. The Deviation color mode can be selected in the combo box but there is no legend explaining the color mapping. Users must know to expand this section and select "Deviation" to see surface error visualization.

### Proposed change

1. **Auto-open when deviation results are present**: Change `default_open` to be conditional:
   ```rust
   let has_deviation_data = sim.results.is_some(); // sim results contain the heightfield needed for deviation
   let show_deviation_default = has_deviation_data && sim.stock_viz_mode == StockVizMode::Deviation;

   egui::CollapsingHeader::new("Stock Display")
       .default_open(show_deviation_default || sim.stock_viz_mode != StockVizMode::Solid)
       .show(ui, |ui| {
   ```

   This opens the section automatically when the user has selected a non-default viz mode, so the controls stay visible.

   However, `default_open` only affects the *first* render (egui persists collapse state). A better approach: use `egui::CollapsingHeader::open()` to force it open when mode is non-Solid:
   ```rust
   let force_open = sim.stock_viz_mode != StockVizMode::Solid;
   let mut header = egui::CollapsingHeader::new("Stock Display")
       .default_open(false);
   if force_open {
       header = header.open(Some(true));
   }
   header.show(ui, |ui| { ... });
   ```

   Actually, `open(Some(true))` overrides user collapse. Better to just use `default_open` based on whether results exist:
   ```rust
   egui::CollapsingHeader::new("Stock Display")
       .default_open(sim.results.is_some())
       .show(ui, |ui| {
   ```

2. **Add color legend when Deviation mode is active** (after the combo box, inside the collapsing header):

   ```rust
   if sim.stock_viz_mode == StockVizMode::Deviation {
       ui.add_space(4.0);
       // Paint a horizontal gradient legend bar
       let legend_width = ui.available_width().min(200.0);
       let legend_height = 12.0;
       let (legend_rect, _) = ui.allocate_exact_size(
           egui::vec2(legend_width, legend_height),
           egui::Sense::hover(),
       );
       let painter = ui.painter_at(legend_rect);
       // Paint gradient: blue (under-cut) -> green (on-target) -> red (over-cut)
       let steps = 20;
       for i in 0..steps {
           let t = i as f32 / steps as f32;
           let color = deviation_legend_color(t);
           let x0 = legend_rect.min.x + t * legend_width;
           let x1 = legend_rect.min.x + (t + 1.0 / steps as f32) * legend_width;
           painter.rect_filled(
               egui::Rect::from_min_max(
                   egui::pos2(x0, legend_rect.min.y),
                   egui::pos2(x1, legend_rect.max.y),
               ),
               0.0,
               color,
           );
       }
       // Labels below the gradient
       ui.horizontal(|ui| {
           ui.label(
               egui::RichText::new("Under-cut")
                   .small()
                   .color(egui::Color32::from_rgb(80, 120, 220)),
           );
           ui.add_space(legend_width * 0.2);
           ui.label(
               egui::RichText::new("On target")
                   .small()
                   .color(egui::Color32::from_rgb(80, 200, 80)),
           );
           ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
               ui.label(
                   egui::RichText::new("Over-cut")
                       .small()
                       .color(egui::Color32::from_rgb(220, 80, 80)),
               );
           });
       });
   }
   ```

   The `deviation_legend_color(t)` helper maps t in [0, 1] to the same color ramp used in the shader/renderer for deviation visualization. This function should match whatever color mapping is in the stock renderer.

3. **Add tooltip to the Deviation combo item**: In the combo box (lines 74-86), add a hover explanation:
   ```rust
   ui.selectable_value(&mut sim.stock_viz_mode, StockVizMode::Deviation, "Deviation")
       .on_hover_text("Color stock by surface deviation from the target model. Blue = under-cut, green = on target, red = over-cut.");
   ```

### Edge cases
- No target model loaded (pure 2D work, no 3D reference surface): Deviation mode may show meaningless colors. The combo box entry could be disabled or grayed out when no reference surface is available, but this requires checking state -- punt to a separate issue.
- The `deviation_legend_color` function must exactly match the renderer's color ramp; if they diverge, the legend is misleading. Reference the shader/renderer code to extract the exact ramp.

### Verification
- Run simulation, expand "Stock Display", select "Deviation"
- Confirm: color legend gradient appears below the combo box with blue/green/red labels
- Hover the "Deviation" combo option: tooltip explains the color meaning
- Section auto-opens when simulation results are present
- Collapse the section manually: it stays collapsed (not forced open)
