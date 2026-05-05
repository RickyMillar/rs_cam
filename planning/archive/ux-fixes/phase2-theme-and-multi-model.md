# Phase 2: Theme Consolidation, Multi-Model Rendering, and Unified Import Scaling

## N4-01/02/04/05/07/08: Create `ui/theme.rs` with semantic color constants + widget helpers

### 1. Color Audit

Every hardcoded `Color32::from_rgb(...)` across the UI, catalogued by semantic meaning.

#### Warning yellow (stale, caution, flip instructions)
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| workspace_bar.rs | 122, 137 | (220, 180, 60) | Pending badge, stale badge |
| setup_panel.rs | 119, 199, 209 | (220, 180, 60) / (200, 170, 60) / (200, 160, 60) | Non-top face label, flip instruction, fresh-stock warning |
| toolpath_panel.rs | 187, 410 | (200, 180, 80) / (200, 180, 60) | Computing status, dep stale badge |
| project_tree.rs | 189, 242-244 | (220, 180, 60) / (200, 180, 80) | Fixture color, computing status |
| sim_op_list.rs | 77, 117 | (220, 180, 60) | Stale warning |
| sim_diagnostics.rs | 47, 116, 461, 466, 491 | (220, 190, 120) / (180, 160, 80) / (180, 140, 80) / (220, 180, 60) | Issue label, re-run hint, min stickout, stale results, holder not checked |
| status_bar.rs | 59, 93 | (210, 190, 90) / (140, 140, 100) | Running lane, "Modified" label |
| viewport_overlay.rs | 69 | (200, 180, 80) | Active lane label |
| preflight.rs | 195 | (220, 180, 60) | Warning icon color |
| properties/mod.rs | 429, 449 | (220, 170, 60) / (220, 190, 60) | Size hint, winding report |
| app.rs | 1997, 2041, 2281-2282 | (255, 200, 80) / (255, 220, 100) | Status message, toast warning |

**Proposed canonical values:**
- `WARNING` = `(220, 180, 60)` -- primary warning yellow
- `WARNING_MILD` = `(200, 170, 60)` -- secondary/softer warning
- `WARNING_TEXT` = `(255, 220, 100)` -- bright warning text (on dark bg)

#### Error red (collision, failure, missing dependency)
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| workspace_bar.rs | 143 | (220, 80, 80) | Collision badge |
| toolpath_panel.rs | 191, 415, 418 | (220, 80, 80) / (200, 80, 80) | Error status, "no dep" badge |
| project_tree.rs | 210, 244 | (220, 80, 80) | Keep-out color, error status |
| sim_diagnostics.rs | 454, 482 | (220, 80, 80) | Holder collision, rapid collision |
| status_bar.rs | 60, 84 | (220, 120, 90) / (220, 80, 80) | Cancelling lane, collision count |
| preflight.rs | 194 | (220, 80, 80) | Fail icon color |
| sim_timeline.rs | 271, 494 | (255, 50, 50) / (255, 120, 80) | Holder collision marker, air cut |
| app.rs | 2285-2286 | (255, 120, 120) | Toast error text |

**Proposed canonical values:**
- `ERROR` = `(220, 80, 80)` -- primary error red
- `ERROR_MILD` = `(200, 80, 80)` -- softer error (dep missing)
- `ERROR_TEXT` = `(255, 120, 120)` -- bright error text (on dark bg)
- `ERROR_MARKER` = `(255, 50, 50)` -- collision marker on timeline (stays unique -- render-specific)

#### Success green
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| workspace_bar.rs | 150 | (100, 180, 100) | Sim OK checkmark badge |
| toolpath_panel.rs | 189, 412 | (80, 180, 80) / (80, 160, 80) | Done status, dep resolved |
| project_tree.rs | 243 | (80, 180, 80) | Done status icon |
| sim_diagnostics.rs | 325, 344, 447, 473, 537 | (120, 210, 150) / (100, 180, 100) / (100, 180, 220) | Cut sample, clear check, est. cycle time |
| status_bar.rs | 76 | (100, 180, 100) | SIM results badge |
| preflight.rs | 193 | (100, 200, 100) | Pass icon color |
| sim_diagnostics.rs (current state) | 414 | (100, 180, 100) | Linear move type |

**Proposed canonical values:**
- `SUCCESS` = `(100, 180, 100)` -- primary success green
- `SUCCESS_BRIGHT` = `(80, 180, 80)` -- status chip green (Done/OK)
- `SUCCESS_METRICS` = `(120, 210, 150)` -- cutting metrics detail

#### Dim / muted text
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| workspace_bar.rs | 42 | (100, 100, 115) | Context hint |
| workspace_bar.rs | 67 | (140, 140, 155) | Inactive tab text |
| setup_panel.rs | 32, 70, 187 | (140, 140, 150) / (120, 120, 130) | Stock dims, empty state, empty workholding |
| toolpath_panel.rs | 50, 67, 184, 229 | (120, 120, 135) / (120, 120, 130) / (120, 120, 130) / (130, 130, 145) | Ready count, no toolpaths, pending status, tool summary |
| project_tree.rs | 66, 100, 187, 267 | (120, 120, 130) / (120, 120, 130) / (100, 100, 110) / (100, 100, 110) | Empty state, no tools, disabled fixture/tp |
| sim_op_list.rs | 175, 236, 344 | (140, 140, 150) | Tool name, move info, semantic dim text |
| sim_diagnostics.rs | 52, 164, 202, 214, 264, 279, 353, 359 | (150, 150, 165) / (140, 140, 155) / (120, 120, 130) | Various dim labels |
| sim_timeline.rs | 184, 347, 442, 642, 680 | (140, 140, 150-155) / (90, 90, 100) | Setups label, keybindings, semantic labels |

**Proposed canonical values:**
- `TEXT_DIM` = `(120, 120, 130)` -- disabled/empty state text
- `TEXT_MUTED` = `(140, 140, 155)` -- secondary info text
- `TEXT_FAINT` = `(100, 100, 115)` -- hints, keybindings

#### Heading / strong text
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| setup_panel.rs | 27, 112 | (160, 170, 190) / (200, 205, 220) | "Stock" heading, setup name |
| toolpath_panel.rs | 39 | (160, 170, 200) | Setup header name |
| project_tree.rs | 17, 153, 181 | (200, 200, 210) / (160, 170, 200) / (160, 160, 175) | Job name, setup name, "Workholding" |
| sim_op_list.rs | 33, 41, 117 | (180, 180, 195) / (140, 140, 155) / (180, 180, 200) | "Ready to simulate", hint text, setup name |
| properties/mod.rs | 396, 459, 506, 576 | (180, 180, 195) | Section headings ("Dimensions", "BREP", "Units", "Included Toolpaths") |

**Proposed canonical values:**
- `TEXT_HEADING` = `(180, 180, 195)` -- section headings
- `TEXT_STRONG` = `(200, 205, 220)` -- emphasized labels (setup name, job name)
- `TEXT_NORMAL` = `(190, 190, 200)` -- default body text

#### Card frame fills
| File | Line(s) | Fill | Inner margin | Rounding |
|------|---------|------|-------------|----------|
| setup_panel.rs | 19 | (36, 36, 44) | 6.0 | 4.0 |
| setup_panel.rs | 102 | (38, 40, 50) | 8.0 | 4.0 |
| toolpath_panel.rs | 164 | (38, 42, 55) | 4.0 | 3.0 |
| sim_op_list.rs | 25, 132 | (36, 36, 44) / (38, 42, 55) | 12.0 / 4.0 | 4.0 / 3.0 |
| app.rs | 2004, 2048, 2126, 2208 | (26, 26, 38) / (34, 34, 42) | varies | varies |

**Proposed canonical values:**
- `CARD_FILL` = `(36, 36, 44)` -- standard card background
- `CARD_FILL_SELECTED` = `(38, 42, 55)` -- selected/active card
- `VIEWPORT_FILL` = `(26, 26, 38)` -- viewport background
- `CARD_INNER_MARGIN` = `6.0` -- standard card inner margin
- `CARD_ROUNDING` = `4.0` -- standard card rounding

#### Accent blue (selection, active tab)
| File | Line(s) | RGB | Usage |
|------|---------|-----|-------|
| workspace_bar.rs | 94 | (100, 160, 220) | Active tab indicator |
| setup_panel.rs | 96 | (100, 160, 220) | Selected card border |
| toolpath_panel.rs | 313 | (100, 160, 220) | Drop indicator |
| sim_diagnostics.rs | 243, 257 | (140, 190, 230) | Performance trace, linked span |

**Proposed canonical values:**
- `ACCENT` = `(100, 160, 220)` -- primary accent blue
- `ACCENT_BRIGHT` = `(140, 190, 230)` -- performance/diagnostic accent

#### Badge / chip patterns (inconsistent widget)
| File | Function | Pattern |
|------|----------|---------|
| workspace_bar.rs | `workspace_tab()` badge | Inline RichText `.small().color(...)` after button |
| setup_panel.rs | `chip()` | Helper fn: `"{key}: {value}"` with `.small().color(color)` + tooltip |
| toolpath_panel.rs | status chip | Inline `RichText::new(text).small().strong().color(...)` |
| sim_debug.rs | `draw_trace_badge()` | Frame with `fill(color.linear_multiply(0.12))` + stroke + inner text |

### 2. `ui/theme.rs` Module Design

```rust
// crates/rs_cam_viz/src/ui/theme.rs

/// Semantic color constants for the rs_cam dark theme.
/// Every UI file should use these instead of inline Color32::from_rgb.

// --- Semantic status colors ---
pub const WARNING: egui::Color32 = egui::Color32::from_rgb(220, 180, 60);
pub const WARNING_MILD: egui::Color32 = egui::Color32::from_rgb(200, 170, 60);
pub const WARNING_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 220, 100);

pub const ERROR: egui::Color32 = egui::Color32::from_rgb(220, 80, 80);
pub const ERROR_MILD: egui::Color32 = egui::Color32::from_rgb(200, 80, 80);
pub const ERROR_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 120, 120);

pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(100, 180, 100);
pub const SUCCESS_BRIGHT: egui::Color32 = egui::Color32::from_rgb(80, 180, 80);
pub const SUCCESS_METRICS: egui::Color32 = egui::Color32::from_rgb(120, 210, 150);

// --- Text hierarchy ---
pub const TEXT_HEADING: egui::Color32 = egui::Color32::from_rgb(180, 180, 195);
pub const TEXT_STRONG: egui::Color32 = egui::Color32::from_rgb(200, 205, 220);
pub const TEXT_NORMAL: egui::Color32 = egui::Color32::from_rgb(190, 190, 200);
pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(140, 140, 155);
pub const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(120, 120, 130);
pub const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(100, 100, 115);
pub const TEXT_DISABLED: egui::Color32 = egui::Color32::from_rgb(100, 100, 110);

// --- Accent ---
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(100, 160, 220);
pub const ACCENT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(140, 190, 230);
pub const ACCENT_PINNED: egui::Color32 = egui::Color32::from_rgb(255, 210, 120);

// --- Card / frame ---
pub const CARD_FILL: egui::Color32 = egui::Color32::from_rgb(36, 36, 44);
pub const CARD_FILL_SELECTED: egui::Color32 = egui::Color32::from_rgb(38, 42, 55);
pub const VIEWPORT_FILL: egui::Color32 = egui::Color32::from_rgb(26, 26, 38);
pub const CARD_BORDER: egui::Color32 = egui::Color32::from_rgb(55, 55, 65);
pub const CARD_BORDER_HOVER: egui::Color32 = egui::Color32::from_rgb(80, 120, 170);

pub const CARD_INNER_MARGIN: f32 = 6.0;
pub const CARD_ROUNDING: f32 = 4.0;

// --- Workspace tab ---
pub const TAB_ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(55, 60, 80);
pub const TAB_ACTIVE_TEXT: egui::Color32 = egui::Color32::from_rgb(220, 225, 240);
pub const TAB_INACTIVE_TEXT: egui::Color32 = egui::Color32::from_rgb(140, 140, 155);

// --- Toast ---
pub const TOAST_INFO_BG: egui::Color32 = egui::Color32::from_rgb(40, 40, 50);
pub const TOAST_WARNING_BG: egui::Color32 = egui::Color32::from_rgb(80, 60, 10);
pub const TOAST_ERROR_BG: egui::Color32 = egui::Color32::from_rgb(80, 20, 20);

// --- Lane / status ---
pub const LANE_IDLE: egui::Color32 = egui::Color32::from_rgb(140, 140, 150);
pub const LANE_QUEUED: egui::Color32 = egui::Color32::from_rgb(150, 170, 210);
pub const LANE_RUNNING: egui::Color32 = egui::Color32::from_rgb(210, 190, 90);
pub const LANE_CANCELLING: egui::Color32 = egui::Color32::from_rgb(220, 120, 90);

// --- Setup chip accent colors (domain-specific, keep as-is) ---
pub const CHIP_ORIENT: egui::Color32 = egui::Color32::from_rgb(100, 140, 180);
pub const CHIP_DATUM: egui::Color32 = egui::Color32::from_rgb(140, 160, 100);
pub const CHIP_FIXTURE: egui::Color32 = egui::Color32::from_rgb(160, 130, 100);
pub const CHIP_KEEPOUT: egui::Color32 = egui::Color32::from_rgb(180, 100, 100);
pub const CHIP_PINS: egui::Color32 = egui::Color32::from_rgb(100, 160, 140);

// --- Axis gizmo colors (fixed, do not unify) ---
pub const AXIS_X: egui::Color32 = egui::Color32::from_rgb(220, 60, 60);
pub const AXIS_Y: egui::Color32 = egui::Color32::from_rgb(60, 200, 60);
pub const AXIS_Z: egui::Color32 = egui::Color32::from_rgb(70, 100, 230);
```

#### Helper functions

```rust
/// Standard card frame for list items and info panels.
pub fn card_frame(selected: bool) -> egui::Frame {
    egui::Frame::default()
        .fill(if selected { CARD_FILL_SELECTED } else { CARD_FILL })
        .inner_margin(CARD_INNER_MARGIN)
        .rounding(CARD_ROUNDING)
}

/// Card frame with explicit border for interactive cards (e.g. setup cards).
pub fn card_frame_bordered(selected: bool, border_color: egui::Color32) -> egui::Frame {
    card_frame(selected)
        .stroke(egui::Stroke::new(1.0, border_color))
}

/// A compact status badge: colored text on a tinted background with border.
/// Reusable for trace badges, status chips, etc.
pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::default()
        .fill(color.linear_multiply(0.12))
        .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.75)))
        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
        .rounding(3.0)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().strong().color(color));
        });
}

/// A key-value chip label (e.g. "Orient: Top") with tooltip.
pub fn chip(ui: &mut egui::Ui, key: &str, value: &str, color: egui::Color32, tooltip: &str) {
    ui.label(
        egui::RichText::new(format!("{key}: {value}"))
            .small()
            .color(color),
    )
    .on_hover_text(tooltip);
}

/// Status color for a ComputeStatus value.
pub fn compute_status_color(status: &crate::state::toolpath::ComputeStatus) -> egui::Color32 {
    match status {
        crate::state::toolpath::ComputeStatus::Pending => TEXT_DIM,
        crate::state::toolpath::ComputeStatus::Computing => WARNING,
        crate::state::toolpath::ComputeStatus::Done => SUCCESS_BRIGHT,
        crate::state::toolpath::ComputeStatus::Error(_) => ERROR,
    }
}
```

### 3. Files to Update

Every file below needs `use crate::ui::theme;` added and hardcoded colors replaced with `theme::CONSTANT_NAME`. The table shows the approximate number of replacements per file.

| File | Replacements | Notes |
|------|-------------|-------|
| `ui/mod.rs` | 1 | Add `pub mod theme;` |
| `ui/workspace_bar.rs` | ~10 | All tab colors, badge colors |
| `ui/setup_panel.rs` | ~14 | Card fills, chip colors, heading colors, dim text |
| `ui/toolpath_panel.rs` | ~16 | Status colors, card fills, dim text, badge colors, drop indicator |
| `ui/project_tree.rs` | ~18 | Status icons, dim text, fixture/keepout colors |
| `ui/status_bar.rs` | ~8 | Lane colors, SIM badge, collision count, modified |
| `ui/sim_op_list.rs` | ~14 | Card fills, heading text, dim text, warning |
| `ui/sim_diagnostics.rs` | ~22 | Status colors, metrics text, warnings, summary |
| `ui/sim_timeline.rs` | ~8 | Timeline labels, speed hint, semantic labels |
| `ui/viewport_overlay.rs` | ~1 | Active lane label |
| `ui/preflight.rs` | ~3 | Check card status colors |
| `ui/sim_debug.rs` | ~5 | Trace badge colors |
| `ui/properties/mod.rs` | ~6 | Section headings, size hints, winding report |
| `app.rs` | ~15 | Toast colors, theme config, viewport fill, axis gizmo, status message |

Total: ~141 replacements across 14 files.

### 4. Execution strategy

1. Create `crates/rs_cam_viz/src/ui/theme.rs` with all constants and helpers.
2. Add `pub mod theme;` to `ui/mod.rs`.
3. Update files one at a time, starting with the simplest (status_bar, viewport_overlay, preflight) to establish the pattern.
4. For the `chip()` function: move from `setup_panel.rs` to `theme.rs`, update `setup_panel.rs` to call `theme::chip()`.
5. Replace `draw_trace_badge` in `sim_debug.rs` to use `theme::badge()` internally.
6. For `compute_status_color`: call from both `toolpath_panel.rs` and `project_tree.rs` to eliminate the duplicate match arms.
7. Run `cargo clippy --workspace --all-targets -- -D warnings` after each file to catch regressions.

### 5. Edge cases and risks

- **Near-identical but intentionally different colors**: A few colors are close but contextually distinct (e.g. `(120, 120, 130)` vs `(120, 120, 135)` in dim text). These 5-unit differences are not visually meaningful -- unify to a single constant.
- **`linear_multiply` calls**: Some colors are derived at runtime via `color.linear_multiply(0.12)`. These patterns stay in the calling code -- the theme provides the base color.
- **Domain-specific chip colors** (Orient, Datum, etc.): Keep as constants in `theme.rs` but do not attempt to merge with semantic status colors.
- **Axis gizmo colors**: These are rendering constants, not UI theme. Include in theme for completeness but mark as fixed.
- **Toast frame margins**: Currently `Margin::symmetric(12.0, 8.0)` with rounding 6.0 -- slightly different from card_frame. Keep as a separate toast_frame() helper or just use the constants inline.

### 6. Verification

- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- `cargo test -q` passes (no UI tests exercise colors, but ensures no compile errors).
- Visual inspection: run `cargo run -p rs_cam_viz --bin rs_cam_gui`, open a project, and verify colors match before/after screenshots in each workspace.

---

## U-02: Multi-Model Rendering [critical]

### Current state

**`render/mod.rs` lines 136-137**: `RenderResources` holds single model data:
```rust
pub mesh_data: Option<MeshGpuData>,           // STL
pub enriched_mesh_data: Option<mesh_render::EnrichedMeshGpuData>,  // STEP
```

**`app.rs` lines 798-847** (`upload_gpu_data`): Only the first model with geometry is uploaded:
```rust
resources.enriched_mesh_data = None;
if let Some(model) = self.controller.state().job.models.iter()
    .find(|model| model.mesh.is_some())  // <-- finds FIRST only
{
    // uploads that one model
}
```

**`render/mod.rs` lines 811-835** (paint callback): Renders a single mesh -- `enriched_mesh_data` OR `mesh_data`, not both, not multiple.

**Picking** (`interaction/picking.rs` lines 142-159): Already iterates all models for face picking on enriched meshes. No single-mesh limitation here.

### Plan

#### Step 1: Change `RenderResources` to hold vectors

File: `crates/rs_cam_viz/src/render/mod.rs`, lines 136-137.

**Current:**
```rust
pub mesh_data: Option<MeshGpuData>,
pub enriched_mesh_data: Option<mesh_render::EnrichedMeshGpuData>,
```

**Change to:**
```rust
pub mesh_data_list: Vec<MeshGpuData>,
pub enriched_mesh_data_list: Vec<mesh_render::EnrichedMeshGpuData>,
```

Also update the constructor (`RenderResources::new`, line ~557):
```rust
mesh_data_list: Vec::new(),
enriched_mesh_data_list: Vec::new(),
```

Update the `needs_colored_uniforms` check (line ~700):
```rust
|| !resources.enriched_mesh_data_list.is_empty();
```

#### Step 2: Change `upload_gpu_data` to iterate all models

File: `crates/rs_cam_viz/src/app.rs`, lines 798-847.

**Current:** Uses `.find()` to get the first model.

**Change to:**
```rust
resources.mesh_data_list.clear();
resources.enriched_mesh_data_list.clear();

for model in &self.controller.state().job.models {
    if let Some(enriched) = &model.enriched_mesh {
        let selected_faces = self.selected_face_ids();
        let hovered_face = self.hovered_face_id();
        let transform = if use_local_frame {
            // SAFETY: use_local_frame is active_setup_ref.is_some()
            #[allow(clippy::unwrap_used)]
            let setup = active_setup_ref.unwrap();
            let stock = &self.controller.state().job.stock;
            Some(Box::new(move |p| setup.transform_point(p, stock))
                as Box<dyn Fn(...) -> ...>)
        } else {
            None
        };
        if let Some(gpu_data) = crate::render::mesh_render::enriched_mesh_gpu_data(
            &render_state.device,
            &resources.gpu_limits,
            enriched,
            &selected_faces,
            hovered_face,
            &transform,
        ) {
            resources.enriched_mesh_data_list.push(gpu_data);
        }
    } else if let Some(mesh) = &model.mesh {
        if use_local_frame {
            #[allow(clippy::unwrap_used)]
            let setup = active_setup_ref.unwrap();
            let transformed =
                transform_mesh(mesh, setup, &self.controller.state().job.stock);
            if let Some(gpu_data) = MeshGpuData::from_mesh(
                &render_state.device,
                &resources.gpu_limits,
                &Arc::new(transformed),
            ) {
                resources.mesh_data_list.push(gpu_data);
            }
        } else {
            if let Some(gpu_data) = MeshGpuData::from_mesh(
                &render_state.device,
                &resources.gpu_limits,
                mesh,
            ) {
                resources.mesh_data_list.push(gpu_data);
            }
        }
    }
}
```

**Note on transform closure**: The current code creates a closure borrowing `active_setup_ref`. When iterating multiple models, the transform logic stays the same -- the setup transform applies uniformly to all models (they all live in the same setup coordinate frame). The closure can be created once before the loop.

#### Step 3: Change the paint callback to render all meshes

File: `crates/rs_cam_viz/src/render/mod.rs`, lines 811-835.

**Current:** Single if/else-if chain rendering one mesh.

**Change to:**
```rust
// Draw mesh (sim mesh replaces raw models when simulation is active)
if self.show_sim_mesh {
    if let Some(sim) = &resources.sim_mesh_data {
        pass.set_pipeline(&resources.sim_mesh_pipeline);
        pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
        pass.set_vertex_buffer(0, sim.vertex_buffer.slice(..));
        pass.set_index_buffer(sim.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..sim.index_count, 0, 0..1);
    }
} else {
    // Draw all enriched (STEP) meshes
    for enriched in &resources.enriched_mesh_data_list {
        pass.set_pipeline(&resources.colored_opaque_pipeline);
        pass.set_bind_group(0, &resources.sim_mesh_bind_group, &[]);
        pass.set_vertex_buffer(0, enriched.vertex_buffer.slice(..));
        pass.set_index_buffer(enriched.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..enriched.index_count, 0, 0..1);
    }

    // Draw all plain (STL) meshes
    if self.has_mesh {
        for mesh in &resources.mesh_data_list {
            pass.set_pipeline(&resources.mesh_pipeline);
            pass.set_bind_group(0, &resources.mesh_bind_group, &[]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
```

#### Step 4: Update `has_mesh` flag

The `ViewportPaintCallback` struct has a `has_mesh: bool` field (used at line 827). Search for where it's set:

File: `app.rs`, wherever the paint callback is constructed. Currently likely:
```rust
has_mesh: resources.mesh_data.is_some(),
```
Change to:
```rust
has_mesh: !resources.mesh_data_list.is_empty(),
```

Also check the `needs_colored_uniforms` boolean at line 700 -- already addressed in step 1.

#### Step 5: Picking (no changes needed)

The picking code (`interaction/picking.rs` lines 142-159) already iterates `job.models` and ray-picks against each enriched mesh. It does not use `RenderResources` at all -- it works from the model data in `JobState`. No changes needed.

For plain STL models, picking currently only works on the first model (through the single `mesh_data`). However, looking at the picking code more carefully, it appears STL face picking is not implemented -- only enriched mesh face picking exists. The toolpath picking and collision picking work independently. So no picking changes are needed for this phase.

#### Step 6: Memory and performance implications

- **GPU memory**: Each model gets its own vertex + index buffer. For N models, GPU memory usage scales linearly. A typical STL is 1-10 MB on GPU. Even with 5 models, this is well under any GPU budget.
- **Draw calls**: One draw call per model per frame. Modern GPUs handle hundreds of draw calls at 60fps. Even 10 models is negligible.
- **CPU upload time**: `upload_gpu_data` runs once per frame when `pending_upload` is true (not every frame). Creating N GPU buffers is fine for N < 20.
- **Transform computation**: The setup transform closure is created once and applied to all models. No performance concern.

#### Verification

1. `cargo clippy --workspace --all-targets -- -D warnings` passes.
2. Import 2+ STL files. Both should appear in the 3D viewport.
3. Import 1 STL + 1 STEP file. Both should render (STL with Phong shading, STEP with per-face colors).
4. Verify simulation still works (sim mesh replaces all models when sim is active).
5. Verify face picking on STEP models still works.
6. Verify camera auto-fit encompasses all model bounding boxes.

#### Risks

- **Camera auto-fit**: The camera fit-to-bbox logic may only consider the first model. Check `fit_camera_to_bbox` -- if it takes a single bbox, it needs to be updated to take the union bbox of all models. This is a follow-up concern, not a rendering bug.
- **Simulation stock**: The simulation stock heightmap is independent of model rendering. No interaction.
- **Z-fighting**: Two overlapping models (e.g. same STL imported twice) will z-fight. This is expected behavior -- not a bug to fix.

---

## U-03: Unified Import Scaling UI [major]

### Current state

**`io/import.rs`**:
- `import_stl(path, id, scale)` -- takes explicit scale parameter
- `import_svg(path, id)` -- no scale parameter, uses raw SVG user-space units (CSS px at 96 DPI)
- `import_dxf(path, id)` -- passes hardcoded `5.0` as arc tolerance to `load_dxf()`, not a scale factor. The DXF loader handles `$INSUNITS` internally. The `5.0` is `arc_tolerance_deg`, NOT a scale.
- `import_step(path, id)` -- no scale parameter
- `import_model(path, id, kind, units)` -- dispatches; only passes scale for STL

**`state/job.rs`**: `LoadedModel` has a `units: ModelUnits` field. `ModelUnits` has variants: Millimeters, Inches, Meters, Centimeters, Custom(f64).

**`ui/properties/mod.rs` lines 500-548**: Scale UI gated on `model.kind == ModelKind::Stl`:
```rust
if model.kind == ModelKind::Stl {
    // Units / Scale selector UI
}
```

**`controller/io.rs` line 82**: `rescale_model()` early-returns for non-STL:
```rust
if model.kind != crate::state::job::ModelKind::Stl {
    return Ok(None);
}
```

### Important clarification: DXF "hardcoded 5.0" is NOT a scale

Reading the code carefully: `import_dxf` calls `load_dxf(path, 5.0)` where `5.0` is the **arc tessellation tolerance in degrees**, not a scale factor. The DXF loader (`dxf_input.rs`) reads `$INSUNITS` from the file header and auto-scales coordinates to mm. There is no hardcoded scale bug in DXF import.

The actual issue is: **SVG and DXF models cannot be rescaled after import** because:
1. The scale UI is gated on `ModelKind::Stl`
2. `rescale_model()` rejects non-STL models
3. SVG coordinates are in CSS pixels (96 DPI user-space units), which are not mm

### Plan

#### Step 1: Add scale to SVG import

File: `crates/rs_cam_viz/src/io/import.rs`, function `import_svg`.

**Current:**
```rust
pub fn import_svg(path: &Path, id: ModelId) -> Result<LoadedModel, VizError> {
    let polygons = load_svg(path, 0.1)?;
    // ...
    units: ModelUnits::Millimeters,
```

**Change to:**
```rust
pub fn import_svg(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let polygons = load_svg(path, 0.1)?;
    // Apply scale to polygon coordinates
    let polygons: Vec<_> = polygons.into_iter().map(|mut poly| {
        for pt in &mut poly.exterior {
            pt.x *= scale;
            pt.y *= scale;
        }
        for hole in &mut poly.holes {
            for pt in hole {
                pt.x *= scale;
                pt.y *= scale;
            }
        }
        poly
    }).collect();
    // ...
    let units = if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    };
    Ok(LoadedModel {
        // ...
        units,
        // ...
    })
}
```

**Default scale for SVG**: `1.0` (raw CSS px). SVG files from design tools like Inkscape use 96 DPI, so 1 px = 0.2646 mm. But many CAM SVGs are already in mm. Default to 1.0 and let the user adjust. Show a size hint (already exists for STL) to help users recognize wrong units.

#### Step 2: Add scale to DXF import

File: `crates/rs_cam_viz/src/io/import.rs`, function `import_dxf`.

**Current:**
```rust
pub fn import_dxf(path: &Path, id: ModelId) -> Result<LoadedModel, VizError> {
    let polygons = load_dxf(path, 5.0)?;  // 5.0 = arc tolerance degrees
    // ...
    units: ModelUnits::Millimeters,
```

The DXF loader already converts to mm via `$INSUNITS`. An additional user-specified scale is still useful if the DXF has no units set (falls back to mm 1:1) but was actually drawn in inches.

**Change to:**
```rust
pub fn import_dxf(path: &Path, id: ModelId, scale: f64) -> Result<LoadedModel, VizError> {
    let polygons = load_dxf(path, 5.0)?;
    // Apply post-INSUNITS scale (user override for unitless DXFs)
    let polygons: Vec<_> = if (scale - 1.0).abs() > 1e-9 {
        polygons.into_iter().map(|mut poly| {
            for pt in &mut poly.exterior {
                pt.x *= scale;
                pt.y *= scale;
            }
            for hole in &mut poly.holes {
                for pt in hole {
                    pt.x *= scale;
                    pt.y *= scale;
                }
            }
            poly
        }).collect()
    } else {
        polygons
    };
    let units = if (scale - 1.0).abs() < 1e-9 {
        ModelUnits::Millimeters
    } else {
        ModelUnits::Custom(scale)
    };
    Ok(LoadedModel {
        // ...
        units,
        // ...
    })
}
```

#### Step 3: Update `import_model` dispatcher

File: `crates/rs_cam_viz/src/io/import.rs`, function `import_model` (line 115-129).

**Current:**
```rust
pub fn import_model(path, id, kind, units) -> Result<LoadedModel, VizError> {
    let mut model = match kind {
        ModelKind::Stl => import_stl(path, id, units.scale_factor())?,
        ModelKind::Svg => import_svg(path, id)?,
        ModelKind::Dxf => import_dxf(path, id)?,
        ModelKind::Step => import_step(path, id)?,
    };
    model.units = units;
    Ok(model)
}
```

**Change to:**
```rust
pub fn import_model(path, id, kind, units) -> Result<LoadedModel, VizError> {
    let scale = units.scale_factor();
    let mut model = match kind {
        ModelKind::Stl => import_stl(path, id, scale)?,
        ModelKind::Svg => import_svg(path, id, scale)?,
        ModelKind::Dxf => import_dxf(path, id, scale)?,
        ModelKind::Step => import_step(path, id)?,  // STEP always mm
    };
    model.units = units;
    Ok(model)
}
```

#### Step 4: Update callers to pass scale

The import event handlers in `controller/io.rs` or `app.rs` that call `import_svg(path, id)` and `import_dxf(path, id)` need to pass a default scale of `1.0`.

Search for all call sites:

```
import_svg(  -> add , 1.0
import_dxf(  -> add , 1.0
```

Specific files and lines:
- `controller/io.rs` (or wherever `AppEvent::ImportSvg` / `AppEvent::ImportDxf` handlers live)
- `io/import.rs` line 124 (`import_model` already addressed above)

#### Step 5: Remove the `ModelKind::Stl` gate on scale UI

File: `crates/rs_cam_viz/src/ui/properties/mod.rs`, lines 500-548.

**Current:**
```rust
if model.kind == ModelKind::Stl {
    // Units / Scale selector UI
}
```

**Change to:**
```rust
// Scale is not meaningful for STEP models (always mm from BREP)
if model.kind != ModelKind::Step {
    // Units / Scale selector UI
}
```

STEP models define their own coordinate system via BREP geometry -- user-specified scaling would break the geometric meaning. STL, SVG, and DXF all benefit from scale control.

#### Step 6: Generalize `rescale_model()`

File: `crates/rs_cam_viz/src/controller/io.rs`, lines 68-108.

**Current:**
```rust
pub fn rescale_model(&mut self, model_id, new_units) -> Result<Option<BoundingBox3>, VizError> {
    // ...
    if model.kind != ModelKind::Stl {
        return Ok(None);  // <-- blocks SVG/DXF rescaling
    }
    let path = model.path.clone();
    let mut new_model = import::import_stl(&path, model_id, new_units.scale_factor())?;
    // ...
}
```

**Change to:**
```rust
pub fn rescale_model(&mut self, model_id, new_units) -> Result<Option<BoundingBox3>, VizError> {
    let Some(model) = self.state.job.models.iter().find(|m| m.id == model_id) else {
        return Ok(None);
    };
    // STEP models have fixed geometry from BREP -- no user rescaling
    if model.kind == ModelKind::Step {
        return Ok(None);
    }
    let path = model.path.clone();
    let kind = model.kind;
    let new_model = import::import_model(&path, model_id, kind, new_units)?;
    let bbox = new_model.bbox();
    if let Some(model) = self.state.job.models.iter_mut().find(|m| m.id == model_id) {
        model.mesh = new_model.mesh;
        model.polygons = new_model.polygons;
        model.enriched_mesh = new_model.enriched_mesh;
        model.units = new_units;
        model.winding_report = new_model.winding_report;
        if self.state.job.stock.auto_from_model {
            if let Some(mesh) = &model.mesh {
                self.state.job.stock.update_from_bbox(&mesh.bbox);
            }
        }
    }
    self.pending_upload = true;
    self.state.job.dirty = true;
    Ok(bbox)
}
```

This uses `import_model` which dispatches to the right importer with the scale factor, making it work for STL, SVG, and DXF.

#### Step 7: Default scales per format

| Format | Default scale | Rationale |
|--------|--------------|-----------|
| STL | 1.0 (mm) | STL has no unit metadata. Most CAD exports default to mm. |
| SVG | 1.0 (px) | SVG coordinates are in CSS user-space units. `load_svg` returns raw coordinates. Design intent varies widely. |
| DXF | 1.0 (post-INSUNITS) | DXF loader already converts from $INSUNITS to mm. Additional scale is 1.0 by default. |
| STEP | N/A | BREP geometry is in mm. No user scaling. |

#### Step 8: Size hint for SVG/DXF

The existing size hint in `properties/mod.rs` (lines 423-437) checks bbox dimensions and shows "Very small!" or "Very large!" warnings. This already works for any model with a bbox. For polygon-only models (SVG, DXF), need to ensure `LoadedModel::bbox()` returns a valid bbox from polygon extents -- which it already does (lines 119-140 of `state/job.rs`).

No additional changes needed for size hints.

### Verification

1. `cargo clippy --workspace --all-targets -- -D warnings` passes.
2. Import an SVG file. The scale UI appears in the properties panel. Changing scale re-imports the SVG with scaled coordinates.
3. Import a DXF file with no `$INSUNITS` (unitless). The scale UI appears. Setting to "inches (x25.4)" rescales correctly.
4. Import a STEP file. The scale UI does NOT appear (STEP is always mm).
5. Import an STL file. The scale UI still works as before.
6. Reload a model (right-click > "Reload from disk"). The current scale is preserved.
7. Save and reopen a project with scaled SVG/DXF models. The scales are restored correctly (via `import_model` which reads `units` from the project file).

### Risks

- **SVG coordinate conventions**: Different tools produce SVGs with different coordinate assumptions. Some use mm directly, some use CSS px (96 DPI), some use pt (72 DPI). The scale UI gives the user control, but the default of 1.0 may confuse users whose SVGs are in px. Consider adding a "Pixels (96 DPI)" preset to `ModelUnits::PRESETS` with scale 0.2646 (25.4/96).
- **DXF with $INSUNITS already handled**: If a DXF specifies inches and the user also sets scale to inches (x25.4), the coordinates will be double-scaled. The scale UI should note that DXF unit conversion is automatic. Add a UI hint: "DXF units are auto-detected from file header. Use custom scale only for override."
- **Polygon scaling mutability**: `import_svg` and `import_dxf` create new polygon data with the scale baked in. This is consistent with how STL works (mesh vertices are scaled on import). Re-import is required to change scale, which is the existing pattern.
- **Project serialization**: `LoadedModel` already serializes `units`. The `import_model` function already reads `units` for reimport. No serialization changes needed.
