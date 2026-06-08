# Spike: egui 0.30 → 0.34 + wgpu 23 → 29 cost estimate (rs_cam_viz)

> THROWAWAY SPIKE. Run in an isolated worktree. Goal is a cost estimate, not a
> working build. Numbers below come from an actual `cargo check -p rs_cam_viz`
> against the bumped deps, plus a real partial migration of the mesh render path.

## 1. Manifest changes

`wgpu` and `glow` are **NOT direct dependencies** — they are pulled transitively
through `egui-wgpu` / `eframe`. The `render/*.rs` files reach `wgpu` via
`use egui_wgpu::wgpu;` (mod.rs) and `use wgpu::util::DeviceExt;` (the re-exported
crate is in the extern prelude). So the version of wgpu is dictated entirely by
the egui family version, not by anything we pin.

Edited `crates/rs_cam_viz/Cargo.toml`:

```toml
# before
eframe = { version = "0.30", features = ["wgpu"] }
egui = "0.30"
egui-wgpu = "0.30"
egui_plot = "0.30"
# after
eframe = { version = "0.34", features = ["wgpu"] }
egui = "0.34"
egui-wgpu = "0.34"
egui_plot = "0.34"
wgpu = "29"     # added explicitly (was transitive) to confirm resolution
glow = "0.17"   # added explicitly (was transitive)
```

Workspace `Cargo.toml` and the other three crates (`rs_cam_core`, `rs_cam_cli`,
`rs_cam_mcp`) contain **no** egui/eframe/wgpu/glow references — nothing else to
change.

Resolved lockfile versions after `cargo update -p rs_cam_viz`:

| crate | before | after |
|-------|--------|-------|
| eframe | 0.30.0 | 0.34.3 |
| egui | 0.30.0 | **0.34.3 AND 0.33.3 (two copies)** |
| egui-wgpu | 0.30.0 | 0.34.3 |
| egui_plot | 0.30.0 | **0.34.1** (targets egui 0.33.3) |
| wgpu | 23.0.1 | 29.0.3 |
| glow | 0.14.2 + 0.16.0 | 0.17.0 |
| naga | 23.1.0 | 29.0.3 |
| winit | 0.30.13 | 0.30.13 (unchanged, as expected) |

Resolution itself **succeeds** (no hard version conflict / "cannot select"),
the deps download and compile. But see §2/§5: pinning `egui_plot = "0.34"` is a
trap — egui_plot 0.34.1 was built against egui **0.33**, so two incompatible
egui/ecolor/emath/epaint trees end up in the graph. The correct pin for an
egui-0.34 world is **`egui_plot = "0.35"`** (0.35.0 targets egui 0.34). There is
no egui_plot 0.34.x that matches egui 0.34.

## 2. Total error count + category breakdown

`cargo check -p rs_cam_viz`: **89 errors, 135 warnings** (deps compiled fine).

| Category | Count | Root cause |
|----------|------:|------------|
| **egui_plot dual-egui version clash** (sim_timeline.rs) | ~31 | egui_plot 0.34.1 pulls egui 0.33; `Color32`/`Stroke`/`Id`/`PlotPoint(s)` passed to plot are a *different type* than 0.34's. Plus egui_plot's own `Line::new`/`Polygon::new` gained a required name arg. |
| **egui painter / widget API churn** | ~22 | `Painter::rect_stroke` now takes 4 args (new `StrokeKind`); `Margin`/`Rounding`→`CornerRadius` switched from `f32` to `i8`/`u8` fields; misc widget signature changes. |
| **eframe shell** | 1 | E0046 "missing `ui`" on `impl eframe::App` — a *cascade* artifact from the broken render/plot modules, not a real eframe API change (App still uses `update`). |
| **wgpu 23→29 churn (render/)** | 11 distinct (all in `render/mod.rs`) | Descriptor field churn — see §3/§4. |
| egui color/id/stroke `From` mismatches outside plot | folded into row 1 | same dual-version root cause |

Error-code histogram: E0277 ×26, E0061 ×26, E0308 ×25, E0560 ×10, E0063 ×1, E0046 ×1.

By file (top offenders): `ui/sim_timeline.rs` (~30, all plot), `app.rs` (~9),
`ui/menu_bar.rs`, `ui/project_tree.rs`, `render/mod.rs` (11), `ui/toolpath_panel.rs`,
`ui/viewport_overlay.rs`, `ui/properties/operations/mod.rs` (lots of `rect_stroke`).

Representative verbatim messages:

egui_plot dual-version:
```
error[E0277]: the trait bound `ecolor::color32::Color32: From<Color32>` is not satisfied
  --> crates/rs_cam_viz/src/ui/sim_timeline.rs:734:32
error[E0277]: the trait bound `std::string::String: From<PlotPoints<'_>>` is not satisfied
  --> crates/rs_cam_viz/src/ui/sim_timeline.rs:725:31
error[E0061]: this function takes 2 arguments but 1 argument was supplied
  --> crates/rs_cam_viz/src/ui/sim_timeline.rs:611:25  (Polygon::new now needs a name)
```

egui painter / margin / rounding:
```
error[E0061]: this method takes 4 arguments but 3 arguments were supplied
  --> crates/rs_cam_viz/src/app/viewport.rs:633:17
     painter.rect_stroke(...)  argument #4 of type `StrokeKind` is missing
error[E0308]: arguments to this function are incorrect
  --> crates/rs_cam_viz/src/app/viewport.rs:426:43
     .inner_margin(egui::Margin::symmetric(10.0, 4.0))  expected `i8`, found floating-point number
error[E0308]: mismatched types
  --> crates/rs_cam_viz/src/ui/workspace_bar.rs:76:17
     sw: 0.0,  expected `u8`, found floating-point number   (Rounding→CornerRadius)
```

wgpu 23→29 (all render/mod.rs):
```
error[E0560]: struct `wgpu::RenderPipelineDescriptor<'_>` has no field named `multiview`
  --> crates/rs_cam_viz/src/render/mod.rs:251:13
error[E0560]: struct `wgpu::PipelineLayoutDescriptor<'_>` has no field named `push_constant_ranges`
  --> crates/rs_cam_viz/src/render/mod.rs:206:13
error[E0063]: missing field `depth_slice` in initializer of `wgpu::RenderPassColorAttachment<'_>`
  --> crates/rs_cam_viz/src/render/mod.rs:752:43
```

## 3. The wgpu 23 → 29 changes that bit

All five are **pure mechanical descriptor-field churn** — no lifetime rewrites,
no RenderPass borrow-checker changes, no pipeline/bindgroup restructuring:

1. `PipelineLayoutDescriptor.push_constant_ranges: &[]` → field removed; replaced
   by `immediate_size: u32` (push constants renamed "immediates"). Set `0`.
2. `RenderPipelineDescriptor.multiview: None` → field renamed to
   `multiview_mask: Option<NonZeroU32>`. Set `None`.
3. `RenderPassColorAttachment` gained required field `depth_slice: Option<u32>`
   (for 3D-texture views). Set `None`.
4. `PipelineLayoutDescriptor.bind_group_layouts: &[&BindGroupLayout]` →
   `&[Option<&BindGroupLayout>]`. Wrap each in `Some(...)`.
5. `DepthStencilState`: `depth_write_enabled: bool` → `Option<bool>`, and
   `depth_compare: CompareFunction` → `Option<CompareFunction>`. Wrap in `Some`.

None of the scary wgpu-29 surface (Surface config, `RenderPass<'a>` lifetime
collapse to `RenderPass<'static>` happened back in 22, Queue/Adapter request
signatures) shows up in the *mesh* path; those may surface in surface/offscreen
setup elsewhere in mod.rs but did not appear in this slice.

## 4. mesh_render.rs migration notes

Surprise finding: **`mesh_render.rs` itself produced ZERO wgpu errors** under
wgpu 29. Its 21 `wgpu::` sites are all `VertexBufferLayout` / `VertexAttribute` /
`VertexFormat` / `Buffer` / `BufferUsages` / `VertexStepMode` / `util::DeviceExt`
— none of which changed between 23 and 29. mesh_render builds GPU *buffers*; the
*pipelines* it feeds live in `render/mod.rs`.

So the "migrate mesh_render" task was really "migrate the two mesh pipelines +
shared depth-stencil in mod.rs that mesh_render depends on." Edits made:

- mod.rs `mesh_pipeline_layout`: `push_constant_ranges` → `immediate_size: 0`;
  `bind_group_layouts` wrapped in `Some(...)`.
- mod.rs `sim_mesh_pipeline_layout`: same two changes.
- mod.rs `depth_stencil` base: `depth_write_enabled: Some(true)`,
  `depth_compare: Some(...)`.
- mod.rs `depth_read_only`: `depth_write_enabled: Some(false)`.
- mod.rs both mesh pipelines: `multiview: None` → `multiview_mask: None`.
- mod.rs `3d_scene` render pass color attachment: added `depth_slice: None`.

**7 edits, ~10 field changes.** This cleared all mesh-path wgpu errors (89→80
errors total). Difficulty: **trivial / find-and-replace.** ~10 minutes of real
work for the mesh slice once the wgpu-29 type defs were read. Each change is the
same 5 patterns repeated; once you know them, the rest of render/ is rote.

The remaining render/mod.rs errors (lines 346, 381, 424, 470, 505, 534, …) are
the *other* pipelines (grid, stock, height-planes, sim) hitting the exact same
5 patterns — pure repetition, no new API surprises in this layer.

## 5. Effort estimate

### Track A — wgpu render-layer migration (render/*.rs)

The migration is mechanical and dominated by repetition, not by any single hard
problem. Per the spike, mesh's ~21 sites needed ~7 edits in ~10 min. The bulk of
the 315 sites are `wgpu::Foo` *type references* that don't change; only the
~5 descriptor patterns above need touching, and they cluster in `mod.rs` (176
sites, where all pipelines/passes/surface live).

- render/mod.rs (176 sites): the real work — ~8 pipelines + passes + offscreen
  + surface config. Surface/Adapter/Queue request signatures may add a few extra
  changes not seen in the mesh slice. ~0.5–1 day.
- All other render files (toolpath 33, sim 31, stock 14, grid 12, height 9,
  gpu_safety 7, fixture 7, gpu_upload 4, viewport 1 = ~118 sites): almost all
  buffer/layout/DeviceExt code (proven unchanged) plus a handful of descriptor
  repeats. ~0.5 day combined.

**Track A estimate: 1.0–2.0 person-days.** Low risk, high tedium. The only
real unknown is whether wgpu 29's surface-acquire / device-request path in mod.rs
needs more than field tweaks — budget the upper end if so.

### Track B — egui / eframe / egui_plot shell migration

Larger and spread across ~18 UI files (~78 errors).

- egui_plot dual-version clash (~31 errors, all sim_timeline.rs): the headline
  blocker. **Bump egui_plot to 0.35** (not 0.34) so it shares egui 0.34. That
  alone collapses most of these `From<…>` errors. Then fix egui_plot 0.35's own
  API churn (`Line::new`/`Polygon::new` name args, `PlotPoints` ownership).
  ~0.5–1 day.
- Painter `rect_stroke` + `StrokeKind` (many call sites in operations/mod.rs,
  viewport.rs, etc.): add the 4th arg everywhere. Mechanical. ~0.5 day.
- `Margin`/`Rounding`→`CornerRadius` float→int (`i8`/`u8`) across theme,
  workspace_bar, panels: change literals `0.0`→`0`, `Margin::symmetric(10.0,4.0)`
  →`(10,4)`. Mechanical but wide. ~0.5 day.
- Residual widget/ctx signature churn + the cascade `impl App` error (resolves
  once modules compile) + re-verify visual output: ~0.5–1 day.

**Track B estimate: 2.0–3.5 person-days.** Wider blast radius, more semantic
(plot rebuild), and needs visual re-verification of the timeline/plot UI.

### Single nastiest blocker

**The egui_plot version mismatch.** Pinning `egui_plot = "0.34"` silently pulls
egui 0.33 alongside egui 0.34, producing ~31 confusing "`Color32: From<Color32>`
not satisfied" / "`String: From<PlotPoints>`" errors that look like nonsense
until you realize there are *two* `egui` crates in the tree. The fix (egui_plot
0.35) is one line but is non-obvious from the error text and is the thing most
likely to burn an hour of head-scratching. Everything else is rote.

## Verdict

**One-line:** Not a clean 1-PR job and not a true trap — it's a ~3–5 person-day
mechanical slog: wgpu render layer is easy (1–2 days, pure descriptor field
churn), the egui/eframe shell is the bulk (2–3.5 days, painter+margin+plot
rewrites), and the one real gotcha is using egui_plot **0.35** (not 0.34) to
avoid a dual-egui version clash.
