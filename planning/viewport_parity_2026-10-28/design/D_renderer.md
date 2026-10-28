# D — Renderer design: MSAA, face state, face edges, ID pass (2026-10-28)

Design only. No product file changes. Code blocks are design, not compiled.
Scope: REPORT.md D.1 steps 5, 7, 8 and 10 (REN-1, REN-2, REN-3, REN-4, SEL-1).

## Files read

| File | Lines | What it decides |
|---|---|---|
| `crates/rs_cam_viz/src/render/mod.rs` | 24-46, 69-102, 111-269, 273-747, 795-850, 945-1110, 1112-1399, 1401-1452, 1454-1648 | uniforms, `OffscreenTargets`, `RenderResources`, five offscreen pipelines (:358 mesh, :471 sim, :506 height plane, :541 colored opaque, :650 line), blit (:714), `ensure_offscreen`, `prepare`, `paint`, four WGSL sources |
| `crates/rs_cam_viz/src/render/mesh_render.rs` | 1-338 | `MeshVertex`, smooth indexed STL path, `enriched_mesh_gpu_data` (:258-319), `EnrichedMeshGpuData` |
| `crates/rs_cam_viz/src/render/upload_cache.rs` | 1-84, 150-200, 290-400, 570-597 | `SlotInputs`, `refresh`, `EnrichedUploadKey` (:172-178), `UploadStats` (:368-397), the hover key test |
| `crates/rs_cam_viz/src/app/gpu_upload.rs` | 20-57, 140-320 | `selected_face_ids`, `hovered_face_id`, `upload_hover_lines`, `upload_tab_lines`, the stats delta log (:305-316) |
| `crates/rs_cam_viz/src/app/gpu_upload/meshes.rs` | 1-260 | `MeshInputs`, `EnrichedInputs` (:53-83), the one model walk `inputs` (:100-150), `upload`, `build_enriched` |
| `crates/rs_cam_viz/src/app/viewport.rs` | 700-772 | the face hover pick and `set_pending_upload` on a change |
| `crates/rs_cam_viz/src/interaction/picking.rs` | 225-290 | `pick_model_face`, `PickHit::ModelFace { model_id, face_id }`, `PickHit::MeshFace` |
| `crates/rs_cam_viz/src/render/colors.rs` | 100-170, 340-373 | `token_rgb`, selection tokens, `FACE_SELECTED`, `FACE_HOVER`, `face_group_color` |
| `crates/rs_cam_viz/src/render/gpu_safety.rs` | 1-48 | `GpuLimits`, `try_create_buffer` |
| `crates/rs_cam_viz/src/render/camera.rs` | 26-81 | `near` 0.1, `far` 10000, perspective and orthographic matrices |
| `crates/rs_cam_viz/src/ui/overlays/registry.rs` | 1-40, 232-320, 515-545, 780-815, 1548-1560, 1640-1760 | row shape, defaults, `model` row, `orientation_gizmo` row, `dock_section`, `apply_overlays`, `PREFERENCE_OVERLAYS`, `effective_default` |
| `crates/rs_cam_viz/src/state/overlays.rs` | 15-45 | `ModelVisibility` |
| `crates/rs_cam_viz/src/app/mcp/view.rs` | 611 | `set_ui_view` calls `apply_overlays` |
| `crates/rs_cam_viz/src/app.rs`, `src/lib.rs` | 218; 58-90 | `RenderResources::new` call site; `NativeOptions` (no multisampling, default backends) |
| `crates/rs_cam_viz/tests/render_pipelines_headless_g_pipesmoke.rs` | 1-202 | device creation, the validation scope, what it does not cover |
| `crates/rs_cam_viz/tests/the_selection_colours_live_in_colors_p4_4.rs` | 1-155 | the scan covers `app/gpu_upload.rs` and `app/gpu_upload/*` only |
| `crates/rs_cam_viz/tests/overlays_registry.rs` | 100-125 | every `ViewportState` bool needs exactly one row |
| `crates/rs_cam_core/src/geometry/enriched_mesh.rs` | 22, 60-145, 310-408 | `FaceGroupId(u16)`, `FaceGroup.boundary_loops`, `MAX_FACE_GROUPS` = 65 536, `build_enriched_mesh` |
| `crates/rs_cam_core/src/session/mod.rs`, `maps/geom_cache.rs`, `selection/flat_regions.rs` | 844-885; 370-417; 171-190 | STL flat regions are merged polygons with no triangle map |
| wgpu-types 30.0.1 `texture/format.rs` | 914-1000, 1968-1990 | guaranteed format features (`R32Uint` has no MSAA flag) |
| wgpu-core 30.0.1 `device/resource.rs` | 4585-4597 | depth bias on a non-triangle topology is a validation error |
| egui-wgpu 0.36.2 `lib.rs`, `setup.rs` | 107-127; 259-264 | `RenderState.adapter`; a GL adapter gets `downlevel_webgl2_defaults` |

## Three facts from the wgpu 30 sources that decide the design

1. **`R32Uint` cannot be multisampled.** `guaranteed_format_features` gives
   `R32Uint => (s_all, atomic)`, with no `MULTISAMPLE_X4` flag (format.rs
   :969). All colour attachments of one pass share one sample count. So the
   ID attachment cannot be an MRT target of the 4-sample scene pass, and
   "resolve by sample 0" (REPORT B.2 REN-2, D.1 step 10) is not available.
   The ID pass is its own single-sample pass (section 4).
2. **Depth bias is refused for `LineList`.** wgpu-core resource.rs :4589
   returns `DepthBiasWithIncompatibleTopology` when `bias.is_enabled()` and
   the topology is not triangles. A polygon-offset bias for the edges is not
   possible. The vertex-shader offset is the only path (section 3).
3. **Only 4 samples are guaranteed.** The guaranteed flags hold
   `MULTISAMPLE_X4` for `Rgba8Unorm`, `Bgra8Unorm`, the sRGB variants and
   `Depth32Float`. 8 and 16 need `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`,
   which the egui device descriptor does not request.

## 1. MSAA (step 5, REN-4)

### 1.1 Sample count: one field, queried once

`RenderResources::new(device, format)` has no adapter. Add a parameter and
store the value once. The five pipelines and `ensure_offscreen` read the
same field, so a pipeline and a texture cannot disagree.

```rust
// render/mod.rs
pub const MSAA_SAMPLES: u32 = 4; // the guaranteed count (fact 3)

/// The scene sample count this adapter gives for `format` and Depth32Float.
pub fn scene_sample_count(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
    let ok = |f| adapter.get_texture_format_features(f).flags.sample_count_supported(MSAA_SAMPLES);
    if ok(format) && ok(wgpu::TextureFormat::Depth32Float) { MSAA_SAMPLES } else { 1 }
}

pub struct RenderResources { /* … */ sample_count: u32, /* … */ }

impl RenderResources {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, sample_count: u32) -> Self {
        let scene_ms = wgpu::MultisampleState { count: sample_count, mask: !0, alpha_to_coverage_enabled: false };
        // :358 mesh, :471 sim_mesh, :506 height_plane, :541 colored_opaque,
        // :650 line, and the new face_mesh pipeline (section 2):
        //     multisample: scene_ms,
        // :714 blit: stays MultisampleState::default() (egui's pass is 1-sample).
    }
    pub fn sample_count(&self) -> u32 { self.sample_count }
}

// app.rs :218
let samples = crate::render::scene_sample_count(&render_state.adapter, render_state.target_format);
let resources = RenderResources::new(&render_state.device, render_state.target_format, samples);
```

### 1.2 `ensure_offscreen`

```rust
struct OffscreenTargets {
    /// The 4-sample colour; None when sample_count == 1.
    msaa_color_view: Option<wgpu::TextureView>,
    /// Single-sample colour: the resolve target, and the blit source.
    color_view: wgpu::TextureView,
    /// Depth at sample_count. Every offscreen pipeline shares it.
    depth_view: wgpu::TextureView,
    blit_bind_group: wgpu::BindGroup,
    width: u32, height: u32,
}

// in ensure_offscreen, after the size clamp:
let msaa_color_view = (self.sample_count > 1).then(|| {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen_color_msaa"), size, mip_level_count: 1,
        sample_count: self.sample_count, dimension: wgpu::TextureDimension::D2,
        format: self.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // never sampled
        view_formats: &[],
    }).create_view(&Default::default())
});
// color_texture: unchanged (sample_count 1, RENDER_ATTACHMENT | TEXTURE_BINDING)
// depth_texture: sample_count: self.sample_count (was 1)
crate::render::gpu_safety::log_offscreen_budget(width, height, self.sample_count, self.target_format);
```

### 1.3 The render pass descriptor (:1062-1110)

```rust
let (view, resolve_target, store) = match &offscreen.msaa_color_view {
    Some(ms) => (ms, Some(&offscreen.color_view), wgpu::StoreOp::Discard),
    None => (&offscreen.color_view, None, wgpu::StoreOp::Store),
};
color_attachments: &[Some(wgpu::RenderPassColorAttachment {
    view, depth_slice: None, resolve_target,
    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(/* SURFACE_SUNKEN, unchanged */), store },
})],
depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
    view: &offscreen.depth_view,
    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0),
        // Discard is correct today. Section 4 reads its OWN depth, not this one.
        store: wgpu::StoreOp::Discard }),
    stencil_ops: None,
}),
```

The resolve writes the single-sample `color_view` at the end of the pass.
The blit layout keeps `multisampled: false`. `paint` does not change.

### 1.4 The depth-read-only overlays

The height planes, the rest heatmap, the tier preview and the By Area
regions draw with `height_plane_pipeline` in the same pass. That pipeline
gets `count: sample_count` like the others. It tests against the same
4-sample depth, so the drape rule does not change. A pipeline at count 1 in
that pass is a pass-time validation error, not a pipeline-creation error
(see 1.6).

### 1.5 The `gpu_safety` budget line

wgpu gives no VRAM query, so the line is an instrument, not a gate. The
hard bound stays the existing `max_texture_dimension_2d` clamp.

```rust
// render/gpu_safety.rs
/// Bytes of the scene attachments: MSAA colour + MSAA depth + resolve colour.
pub fn offscreen_bytes(width: u32, height: u32, samples: u32, format: wgpu::TextureFormat) -> u64 {
    let px = u64::from(width) * u64::from(height);
    let colour = u64::from(format.block_copy_size(None).unwrap_or(4));
    let depth = 4; // Depth32Float
    let ms = if samples > 1 { px * colour * u64::from(samples) } else { 0 };
    ms + px * depth * u64::from(samples) + px * colour
}
pub fn log_offscreen_budget(w: u32, h: u32, s: u32, f: wgpu::TextureFormat) {
    tracing::debug!(w, h, samples = s, mb = offscreen_bytes(w, h, s, f) / (1 << 20), "offscreen targets");
}
```

| Viewport (px) | Samples | MSAA colour | MSAA depth | Resolve | Total |
|---|---|---|---|---|---|
| 1920 × 1080 | 1 | 0 | 7.9 MiB | 7.9 MiB | 15.8 MiB |
| 1920 × 1080 | 4 | 31.6 MiB | 31.6 MiB | 7.9 MiB | 71.2 MiB |
| 3840 × 2160 | 4 | 126.6 MiB | 126.6 MiB | 31.6 MiB | 284.8 MiB |
| 8192 × 8192 (clamp, `Limits::default`) | 4 | 1024 MiB | 1024 MiB | 256 MiB | 2304 MiB |

Formula: `w × h × 4 B × samples` for each multisampled attachment, plus
`w × h × 4 B` for the resolve. The last row is the worst case that the
clamp allows. The viewport is the 3D rect, not the window, so real values
are lower. Section 4 adds `w × h × 8 B` (id + id depth) when the outline
is on.

### 1.6 The headless sentry

G-PIPESMOKE builds `RenderResources::new` only. Its doc states that
`ensure_offscreen` is not covered, and a sample-count mismatch between a
pipeline and an attachment fails at `begin_render_pass`, not at pipeline
creation. So the test must render one frame:

```rust
// new arm in tests/render_pipelines_headless_g_pipesmoke.rs, per app format:
let x4 = adapter.get_texture_format_features(format).flags.sample_count_supported(4); // (and Depth32Float)
assert_eq!(scene_sample_count(&adapter, format), if x4 { 4 } else { 1 }); // a software GL adapter may lack X4
let mut res = RenderResources::new(&device, format, samples);
res.render_headless_frame(&device, &queue, 64, 48); // test door: ensure_offscreen + one scene pass
assert!(block_on(scope.pop()).is_none()); // both the 4-sample and the fallback path get cover
```

`render_headless_frame` is a `#[doc(hidden)] pub` test door in
`render/mod.rs` that calls `ensure_offscreen` and the scene pass with a
default `ViewportCallback`. The adapter helper moves to
`tests/fixtures/gpu.rs` so sections 2-5 reuse it.

## 2. Per-face state buffer (step 8, SEL-1) and smooth faces (REN-3)

### 2.1 Vertex type and upload (enriched only)

`ColoredMeshVertex` and the sim, heatmap, tier, regions and reach paths do
not change. `build_enriched_mesh` already appends each face's own vertex
list with an offset (enriched_mesh.rs :360-372), so no vertex is shared by
two faces. The native `mesh.triangles` index buffer therefore gives smooth
normals inside a face and a hard crease between faces with no extra work.

```rust
// render/mesh_render.rs
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FaceMeshVertex {
    pub position: [f32; 3], // 12
    pub normal: [f32; 3],   // 12, area-weighted inside the face
    pub face_id: u32,       // 4  (FaceGroupId widened)
} // 28 B
impl FaceMeshVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32];
        wgpu::VertexBufferLayout { array_stride: 28, step_mode: wgpu::VertexStepMode::Vertex, attributes: &A }
    }
}

pub struct FaceMeshGpuData {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub face_count: u32,
    pub face_state: wgpu::Buffer,       // STORAGE | COPY_DST, face_count × 4 B
    pub state_bind_group: wgpu::BindGroup, // group(1)
    pub model_id: rs_cam_core::ids::ModelId,
}

/// No selection and no hover input: the geometry only.
pub fn face_mesh_gpu_data(device: &wgpu::Device, limits: &GpuLimits, layout: &wgpu::BindGroupLayout,
    palette: &wgpu::Buffer, model_id: ModelId, enriched: &EnrichedMesh,
    transform: &VertexTransform<'_>) -> Option<FaceMeshGpuData> {
    // normals: the from_mesh area-weighted loop (stays in-face); face_id[v] = triangle_to_face[t];
    // indices = mesh.triangles; face_state = face_groups.len() zero words.
}
```

VRAM per triangle (`r` = vertices per triangle; a closed grid has `r ≈ 0.5`;
per-face tessellation duplicates the boundary, so `r` is a little higher):

| Path | Vertex bytes | Index bytes | Per triangle | 132 k triangles |
|---|---|---|---|---|
| Today (3 × 36 B unindexed, plus a trivial index buffer) | 108 | 12 | 120 B | 15.8 MB |
| `FaceMeshVertex`, indexed, `r = 0.5` | 14 | 12 | 26 B | 3.4 MB |
| `FaceMeshVertex`, indexed, `r = 0.6` | 16.8 | 12 | 28.8 B | 3.8 MB |
| Hover change today | — | — | 120 B (rebuild) | 15.8 MB |
| Hover change after | — | — | 0 | `faces × 4 B` |

Note: the B.1 figure (108 B) omits the index buffer that
`enriched_mesh_gpu_data` also uploads (:286-288, 12 B per triangle).

### 2.2 Bind groups and the palette

- Group 0: the existing `mesh_bgl` with `MeshUniforms` (view_proj,
  light_dir, camera_pos). The face pipeline needs no opacity.
- Group 1 (new `face_state_bgl`): binding 0 = `face_state` storage
  (read-only, FRAGMENT); binding 1 = `palette` uniform (FRAGMENT).
- One group-1 bind group per drawn STEP model, built with its buffers in
  the upload. Models are few (each visible STEP model in
  `ModelVisibility`), so no dynamic offset and no immediates.

```rust
// render/colors.rs: the one source; the WGSL indices are its slots.
pub const FACE_PALETTE_LEN: usize = 24;
pub const FACE_SLOT_SELECTED: u32 = 10;
pub const FACE_SLOT_HOVER: u32 = 11;
pub const FACE_SLOT_STATE0: u32 = 12; // + ItemState as Cut, Island, Skipped, Excluded, NotSelected
pub const FACE_SLOT_INK: u32 = 17;    // edges and outline (sections 3 and 4)
pub const FACE_SLOT_ACCENT: u32 = 18; // selection outline
/// Slots 0..10 = FACE_GROUP_PALETTE; then FACE_SELECTED, FACE_HOVER, the
/// five selection tokens, INK, ACCENT; the rest zero. vec4 stride for WGSL.
pub fn face_palette() -> [[f32; 4]; FACE_PALETTE_LEN] { /* built from the Rgb and token_rgb values */ }
pub const MODEL_EDGE: [f32; 3] = token_rgb(tokens::INK_65); // ink token; the lead picks the step
```

The p4_4 scan reads `app/gpu_upload*` only. The palette therefore lives in
`colors.rs`, and the upload reads it as `colors::face_palette()`. The scan
then counts one more `colors::` read and finds no literal.

### 2.3 The state word

```text
bit 0      hovered
bit 1      selected
bits 2-7   item-state palette slot + 1 (0 = no state; base pastel)
bits 8-31  reserved (0)
```

### 2.4 WGSL (new `FACE_MESH_SHADER_SRC`)

```wgsl
struct Uniforms { view_proj: mat4x4<f32>, light_dir: vec3<f32>, camera_pos: vec3<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var<storage, read> face_state: array<u32>;
@group(1) @binding(1) var<uniform> palette: array<vec4<f32>, 24>;

struct VsIn  { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) face_id: u32 };
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) nrm: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) face_id: u32, // naga refuses an integer varying without flat
};

@vertex fn vs_main(i: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = u.view_proj * vec4<f32>(i.pos, 1.0);
    o.nrm = i.nrm; o.world = i.pos; o.face_id = i.face_id;
    return o;
}

fn face_colour(id: u32) -> vec3<f32> {
    let s = face_state[id];
    if ((s & 2u) != 0u) { return palette[10].rgb; }          // selected
    if ((s & 1u) != 0u) { return palette[11].rgb; }          // hovered
    let st = (s >> 2u) & 63u;
    if (st != 0u) { return palette[st - 1u].rgb; }           // item state slot
    return palette[id % 10u].rgb;                            // face_group_color
}

@fragment fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    let c = face_colour(i.face_id);
    let l = normalize(u.light_dir);
    let n0 = normalize(i.nrm);
    let n = select(n0, -n0, dot(n0, l) < 0.0);
    let v = normalize(u.camera_pos - i.world);
    let spec = pow(max(dot(n, normalize(l + v)), 0.0), 32.0);
    // shading terms as fs_opaque today (:1548-1560), so the look does not move
    return vec4<f32>(c * 0.25 + c * max(dot(n, l), 0.0) + vec3<f32>(0.2) * spec, 1.0);
}
```

The `0.25`, `0.2` and `32.0` are the existing `fs_opaque` terms; they are
shading factors, not colours, and they move unchanged.

### 2.5 Write path and `upload_cache`

Recommendation: **one writer.** A keyed `face_state` slot whose build is a
full `queue.write_buffer` of each model's words on any key change, hover
included. The worst case is `MAX_FACE_GROUPS × 4 B` = 256 KiB per model;
the cost is O(faces), not O(triangles). A two-word partial write saves
almost nothing and needs a CPU shadow that a second writer shares.

```rust
// upload_cache.rs
#[derive(Debug, Clone, PartialEq)]
pub struct EnrichedUploadKey {          // loses selected_faces and hovered_face
    pub frame: FrameKey,
    pub meshes: Vec<(rs_cam_core::ids::ModelId, ArcId<EnrichedMesh>)>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FaceStateKey {
    /// Every input of the face-mesh build. A rebuilt mesh has zeroed state
    /// buffers, so this key must move whenever that key moves (a frame flip too).
    pub mesh: EnrichedUploadKey,
    pub hovered: Option<(ModelId, FaceGroupId)>,
    pub selected: Vec<(ModelId, FaceGroupId)>,
    pub states: Vec<(ModelId, FaceGroupId, u8)>, // item-state slot, when the face path shows states
}
pub struct UploadStats { /* … */ pub face_state_writes: u64 }
```

```rust
// app/gpu_upload/face_state.rs (new, F132: one file per slot)
pub(super) struct FaceStateInputs<'a> { mesh: &'a EnrichedInputs<'a>, hovered: Option<(ModelId, FaceGroupId)>,
    selected: Vec<(ModelId, FaceGroupId)>, states: Vec<(ModelId, FaceGroupId, u8)> }
impl SlotInputs for FaceStateInputs<'_> {
    type Key = upload_cache::FaceStateKey;
    fn key(&self) -> Self::Key {
        let Self { mesh, hovered, selected, states } = self;
        FaceStateKey { mesh: mesh.key(), hovered: *hovered, selected: selected.clone(), states: states.clone() }
    }
}
pub(super) fn upload(inputs: FaceStateInputs, gpu: &Gpu<'_>, resources: &mut RenderResources) {
    if let Refresh::Build(Some(i)) = upload_cache::refresh(&mut resources.face_state_key, Some(inputs)) {
        for m in &resources.face_mesh_data_list {
            let words = face_state_words(m.model_id, m.face_count, &i); // pure, pub(crate), unit-tested
            gpu.queue.write_buffer(&m.face_state, 0, bytemuck::cast_slice(&words));
        }
        resources.upload_stats.face_state_writes += 1;
    }
}
```

Order in `upload_gpu_data`: `meshes::upload` first, then
`face_state::upload`. The face-state key embeds the mesh key (one source,
held by the compiler). So every mesh rebuild (a re-import or a setup flip)
also moves the face-state key, and the new zeroed buffers get their words
in the same pass. (`meshes::upload` takes the mesh inputs by reference, so
the face-state inputs can borrow them.)

Two side fixes fall out of the key change:

- `hovered_face_id` and `selected_face_ids` today give a bare
  `FaceGroupId`. With two STEP models, one id lights the same face index on
  both. The new inputs carry `(ModelId, FaceGroupId)`. `last_hover_face`
  takes the `model_id` that `PickHit::ModelFace` already returns.
- The selection from the `selection_status` report maps `k.brep` with the
  key's model source; `Selection::Face(model, face)` already holds it.

Fallback, only if the frame-time instrument shows the full write on hover:
two `queue.write_buffer(&buf, 4 * face, word)` calls (old and new face) in
the same slot, with the previous key as the shadow.

### 2.6 Draw

In the `show_model` arm (:1186), each `FaceMeshGpuData` draws with
`face_mesh_pipeline`, group 0 = `mesh_bind_group`, group 1 = its own
`state_bind_group`, then `draw_indexed`. `needs_colored_uniforms` (:975)
drops the `enriched_mesh_data_list` term. `colored_opaque_pipeline` stays
for the reach overlay.

### 2.7 STL flat-face hover

The face-state path does not fit an STL as it is. The STL upload shares a
vertex between triangles of different flat regions (smooth indexed path),
and core keeps the flat regions as merged polygons with no triangle map
(`flat_regions.rs` :171-190). A per-vertex region id is therefore wrong on
every region border. Two options:

| Option | Change | Cost |
|---|---|---|
| A (recommended first) | `hover_vertices` looks up the key in `model.face_items()` when `faces_are_flat_regions()`, not in `m.items` only (gpu_upload.rs :601-605). The flat face then gets the existing hover ring at its own Z. | S, no renderer change |
| B (later) | Core exposes the per-triangle region map from `group_by_plane`. The STL upload splits a vertex only where two regions meet (key `(vertex, region)`), gives `face_id = region` or `u32::MAX`, and joins the face pipeline. The mesh key gains the flat-region `Arc` identity. | M: a core API, a lazy cache in the key, a second index of the same mesh |

## 3. Face edges (step 7, REN-1)

### 3.1 Upload slot `model_edges`

One new slot file `app/gpu_upload/model_edges.rs`. Its inputs come from the
same walk as `meshes::inputs` (that function returns a third struct), so
the edges draw for exactly the models that draw a face mesh: hidden models
(`ModelVisibility`) and obstacle models are out by construction.

```rust
pub(super) struct ModelEdgeInputs<'a> { frame: &'a PassFrame, meshes: Vec<&'a Arc<EnrichedMesh>> }
// Key: upload_cache::ModelEdgesKey { frame: FrameKey, meshes: Vec<ArcId<EnrichedMesh>> }
// Build: for each face group, for each loop in boundary_loops, push the
// closed polyline as LineList pairs (p[i], p[i+1]) and (p[last], p[0]),
// through the same setup transform as build_enriched, in colors::MODEL_EDGE.
// One buffer for all models: resources.model_edge_lines: Option<PolygonGpuData>.
```

A shared edge appears in both adjacent faces' loops, so it draws twice at
the same depth. That is harmless (REPLACE blend, same colour). A dedupe is
not worth its hash cost.

### 3.2 Depth: a view-space offset toward the eye

Polygon offset is refused for lines (fact 2). The vertex shader moves each
edge vertex toward the eye. A move along the eye ray does not change the
screen position in perspective, so only the depth test sees it.

Derivation (camera.rs: `near` = 0.1, `far` = 10000):

- Perspective depth is `d ≈ 1 − n / z`, so `∂d/∂z = n / z²`.
- One `Depth32Float` step near `d = 1` is `2^-24`; it spans `Δz = 2^-24 z² / n`.
- Four steps of margin: `δ_depth(z) = 2^-22 z² / n`.
- The edge polyline can also sit off the facet by the import chord
  tolerance `tol` (`load_step(path, tolerance)`, step_input.rs :44-50).
- Offset: `δ(z) = max(tol, 2^-22 z² / n)`. At `z` = 300 mm: 0.21 mm.
- Orthographic: depth is linear over `2 far`, so `δ = max(tol, 4 × 2^-24 × 2 far)`
  = `max(tol, 0.0048 mm)`.

```rust
// LineUniforms grows from 80 to 96 B; Rust and WGSL stay in step by field.
pub struct LineUniforms {
    pub view_proj: [[f32; 4]; 4],
    /// xyz = eye position (w = 1, perspective) or the direction to the eye (w = 0, orthographic).
    pub eye: [f32; 4],
    pub dim: f32,
    /// 0 for every line except the model edges.
    pub edge_tol: f32,
    /// 2^-22 / near in perspective; 0 in orthographic (edge_tol then holds the max).
    pub edge_k: f32,
    pub _pad: f32,
}
```

```wgsl
struct Uniforms { view_proj: mat4x4<f32>, eye: vec4<f32>, dim: f32, edge_tol: f32, edge_k: f32, _pad: f32 };
@vertex fn vs_main(in: VertexInput) -> VertexOutput {
    var p = in.position;
    if (u.edge_tol > 0.0) {
        let to_eye = u.eye.xyz - p * u.eye.w;          // w=0: eye.xyz is the direction
        let dist = length(to_eye);
        let off = max(u.edge_tol, u.edge_k * dist * dist);
        p = p + to_eye / max(dist, 1e-6) * off;
    }
    var out: VertexOutput;
    out.clip_position = u.view_proj * vec4<f32>(p, 1.0);
    out.color = in.color;
    return out;
}
```

With the offset, `CompareFunction::Less` already passes; `LessEqual` is not
needed. Note: `near` = 0.1 against `far` = 10000 makes the far offset
large. A larger `near` reduces it; that is a camera change for the lead.

### 3.3 Pipeline: reuse `line_pipeline`

No new pipeline. Add a third uniform buffer and bind group,
`line_edge_bind_group`, on the `line_dim_bind_group` precedent (:143-146):
the same `view_proj`, `edge_tol` and `edge_k` set, written each frame
beside the other two. The plain and dimmed groups write `edge_tol = 0`, so
the grid, toolpaths, hover ring and tab marks do not move.

Draw order: after the face meshes, before `show_polygons`:

```rust
if self.show_model && self.show_model_edges && let Some(e) = &resources.model_edge_lines {
    pass.set_pipeline(&resources.line_pipeline);
    pass.set_bind_group(0, &resources.line_edge_bind_group, &[]);
    pass.set_vertex_buffer(0, e.vertex_buffer.slice(..));
    pass.draw(0..e.vertex_count, 0..1);
}
```

The reach overlay replaces the model draws; the edges still draw over it
(they are geometry, not a colour field). The edges do not draw in
Simulation (the model does not draw there).

### 3.4 Registry row

```rust
OverlayRow {
    id: "model_edges",
    group: OverlayGroup::Geometry,
    label: "Face edges",
    surface: OverlaySurface::None,
    mechanism: OverlayMechanism::DrawTime,   // the slot is keyed on geometry; the flag gates the draw
    flag: Some("viewport.show_model_edges"),
    radio: false,
    get: |s| s.viewport.show_model_edges,
    set: |s, on| s.viewport.show_model_edges = on,
    precondition: |s| {
        if in_simulation(s) { Precondition::no("replaced by the simulated stock here") }
        else if !s.viewport.show_model { Precondition::no("the model is off") }
        else if !s.session().models().iter().any(|m| m.enriched_mesh.is_some()) {
            Precondition::no("no STEP model: an STL has no face edges")
        } else { Precondition::Ready }
    },
    default_for: geometry_default,           // Setup and Toolpaths on, Simulation off (Onshape default)
    hover: "The boundary of each STEP face, drawn in ink over the part.",
},
```

- `ViewportState` gains `pub show_model_edges: bool` (default `true`); the
  `overlays_registry` sentry then demands this row, and the row satisfies it.
- `dock_section` puts it in `Scene` through the `Geometry` fallback.
- Add `"model_edges"` to `PREFERENCE_OVERLAYS`, so File ▸ Preferences can set it.
- MCP: `set_ui_view { overlays: { "model_edges": false } }` works with no
  MCP change, because `apply_overlays` looks up every row by id (:1652-1703).
- `ViewportCallback` gains `show_model_edges`, set in `app/viewport.rs`
  beside `show_model`.

### 3.5 Line width

| Approach | What changes | Cost | Gain |
|---|---|---|---|
| Keep 1 px `LineList` | nothing | 0 | With MSAA (section 1) a 1 px line gets coverage smoothing. Thin on a 2× HiDPI screen (1 px = half a logical pixel). |
| Quad-expanded lines | A `wide_line_pipeline`: the existing `LineList` buffers bind as an instance buffer (`step_mode: Instance`, stride 48: `a.pos, a.col, b.pos, b.col`), `draw(0..6, 0..n)`; the vertex shader projects both ends and pushes a quad `width_px × pixels_per_point` wide in screen space. No data change. | M: one pipeline, one WGSL, a `viewport_px` uniform, every line draw site picks a pipeline (grid, toolpaths, hover ring, tab marks, edges); round joins are absent; 6 vertices per segment instead of 2 | Width control everywhere; the hover ring and tab marks read better; the toolpath "ribbon" lever (comment at :91) becomes possible |

Recommendation: land MSAA first and take an MCP screenshot pair of
`stepped_block.step` with edges at 1 px. Move to quads only if the pair
shows the edges too thin, and then move the hover ring, tab marks and edges
together (one pipeline, one rule). The toolpath moves stay on `LineList`
until the WP27 draw set is measured with quads.

## 4. ID pass and outline (step 10, lower depth)

### 4.1 Pass shape

The ID attachment cannot join the 4-sample pass (fact 1). It is a second,
single-sample pass in `prepare`, after the scene pass, and only while the
outline row or a visible-only pick needs it.

- Attachments: `id_view` (`R32Uint`, `RENDER_ATTACHMENT | TEXTURE_BINDING |
  COPY_SRC`, clear 0) and `id_depth_view` (`Depth32Float`, 1 sample,
  `RENDER_ATTACHMENT | TEXTURE_BINDING`, clear 1.0, store).
- Draws: every face mesh (face id) and every plain STL mesh (model id, no
  face), so an STL in front of a STEP face hides that face's id.
- `id_pipeline`: vertex layout `FaceMeshVertex` (STL: `MeshVertex` with a
  second `id_mesh_pipeline` that reads the id from an immediate or a tiny
  per-model uniform), `ColorTargetState { format: R32Uint, blend: None }`
  (blending an integer format is a validation error), depth `Less`, write on.
- Cost: one more vertex pass over the model, and `w × h × 8 B` of textures.

```wgsl
// id_fs: group(1) is the same face_state bind group as section 2.
struct IdUniforms { model_slot: u32 };       // per model, 1-based, < 2^15
@group(2) @binding(0) var<uniform> idu: IdUniforms;
@fragment fn fs_id(i: VsOut) -> @location(0) u32 {
    let sel = (face_state[i.face_id] >> 1u) & 1u;
    return (sel << 31u) | (idu.model_slot << 16u) | (i.face_id & 0xFFFFu);
}
// STL: (model_slot << 16) | 0xFFFF. Background: 0.
```

### 4.2 Blit edge test

The blit layout gains `t_id: texture_2d<u32>` and `t_iddepth: texture_depth_2d`
(both `textureLoad`, no sampler) and a small uniform (`near`, `far`, the
depth-step ratio, the outline flag). A second blit bind group without the id
textures keeps the outline-off path as it is today.

```wgsl
@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(t_color, s_color, in.uv);
    if (bu.outline == 0u) { return base; }
    let dim = vec2<i32>(textureDimensions(t_id));
    let p = clamp(vec2<i32>(in.uv * vec2<f32>(dim)), vec2<i32>(0), dim - 1);
    let c = textureLoad(t_id, p, 0).r;
    let zc = linear_depth(textureLoad(t_iddepth, p, 0));
    var ink = false; var accent = false;
    for (var k = 0; k < 4; k++) {
        let q = clamp(p + OFFS[k], vec2<i32>(0), dim - 1);
        let n = textureLoad(t_id, q, 0).r;
        let zn = linear_depth(textureLoad(t_iddepth, q, 0));
        if ((n & 0x7FFFFFFFu) != (c & 0x7FFFFFFFu) && (n >> 16u) != (c >> 16u)) { ink = true; } // silhouette: other model or background
        if (abs(zn - zc) > bu.depth_step * zc) { ink = true; }                               // fold of a curved face
        if ((n >> 31u) == 1u && (c >> 31u) == 0u) { accent = true; }                          // outside a selected face
    }
    if (accent) { return vec4<f32>(bu.accent.rgb, 1.0); }
    if (ink)    { return vec4<f32>(bu.ink.rgb, 1.0); }
    return base;
}
```

The face-to-face boundary inside one model is left to the section 3 edges,
so the outline does not double them. `bu.ink` and `bu.accent` come from
`colors::face_palette()` slots 17 and 18. `depth_step` is derived like the
section 3 offset (a ratio of the depth step to the depth, not a typed
number).

### 4.3 One-pixel readback ring (rerun pattern)

Source of the idea: rerun `re_renderer` readback belt and picking layer
(MIT or Apache-2.0); WebGPU Fundamentals "GPU picking". Credit both in
`CREDITS.md` when step 10 lands. No code is copied.

```rust
struct IdReadback { bufs: [wgpu::Buffer; 3], pending: [Option<(u64 /*request*/, Arc<AtomicBool>)>; 3], next: usize }
// in prepare, after the id pass, when a request (cursor px, request id) is set:
encoder.copy_texture_to_buffer(
    wgpu::TexelCopyTextureInfo { texture: &id_tex, mip_level: 0,
        origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All },
    wgpu::TexelCopyBufferInfo { buffer: &ring.bufs[i],
        layout: wgpu::TexelCopyBufferLayout { offset: 0,
            bytes_per_row: None,   // one row: no 256 B padding needed; a wider crop pads to 256 B
            rows_per_image: None } },
    wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 });
// next frame: map_async(Read) sets a flag; the next prepare calls device.poll(PollType::Poll)
// (check the variant name), reads the u32, unmaps, and publishes (request id, id).
// The click still uses the CPU BVH at once; the id answers "visible only".
```

Buffers: `MAP_READ | COPY_DST`, 256 B each (the copy alignment rule for a
wider crop). Latency: one or two frames, so the GUI never waits on it.

## 5. Tests

Names follow the `<claim>_<tag>.rs` convention of `crates/rs_cam_viz/tests/`.

| Test | File | Device | What it asserts |
|---|---|---|---|
| `the_scene_pass_renders_at_the_adapter_sample_count_g_pipesmoke` | `tests/render_pipelines_headless_g_pipesmoke.rs` | yes (skips with a reason if none) | `scene_sample_count` = 4 where the adapter reports X4 for the format and `Depth32Float`, else 1; one headless frame through `ensure_offscreen` raises no validation error at either count (closes the doc's first gap) |
| `every_render_pipeline_builds_on_a_headless_adapter_g_pipesmoke` | same | yes | unchanged, now with `face_mesh_pipeline`, `id_pipeline` and the new layouts; doc list updated (5 shader modules, 4-5 layouts) |
| `a_hover_change_adds_zero_enriched_builds_sel1` | preferred: a `--lib` test in `app/gpu_upload/meshes.rs` with the headless device (`app/gpu_upload` is not a public module); alternative: `tests/…_sel1.rs` through a `#[doc(hidden)] pub` door | yes | two passes that differ only in the hover give `enriched_builds` delta 0 and `face_state_writes` delta 1; a third identical pass gives 0 and 0 |
| key arm (no device) | `render/upload_cache.rs` `mod tests` | no | the existing test at :570-597 inverts: `EnrichedUploadKey` has no hover field (F132 destructure), and two `FaceStateKey`s that differ in hover differ |
| `face_state_words` unit test | `app/gpu_upload/face_state.rs` `mod tests` | no | hover on model A face 3 sets bit 0 of word 3 of A only; a selected face sets bit 1; a face of model B with the same index stays 0 (the two-model defect of 2.5); a frame change alone moves `FaceStateKey` and rewrites |
| `the_selection_colours_live_in_colors_p4_4` | existing | no | still green: `face_state.rs` and `model_edges.rs` read `colors::`, hold no float triple; the anchor count rises |
| `the_edges_draw_for_visible_models_only_ren1` | `tests/the_edges_draw_for_visible_models_only_ren1.rs` | no | the third struct of `meshes::inputs` (through a `pub` test door) lists the same models as the face slot; a hidden model leaves both; an obstacle model leaves both |
| `overlays_registry`, `the_viewport_dock_reaches_every_option_g_vpdock` | existing | no | the new `show_model_edges` field has one row; the dock count assertion (`rows.len() >= 39`) still holds and the row has one home |
| WGSL compile | covered by G-PIPESMOKE | yes | naga validates each module in `create_shader_module`, which `RenderResources::new` calls. Rule kept: every shader module is created in `new`, never lazily |

The adapter helper (`request_any_adapter`, `block_on`) moves to
`tests/fixtures/gpu.rs`, shared by the device tests above.

## 6. Commit sequence

Each commit builds, passes the render sentries, viz `--lib`, and clippy.

| # | Commit | Files | Green because |
|---|---|---|---|
| 1 | MSAA: sample count field, five pipelines, `ensure_offscreen`, pass descriptor, budget log, headless frame arm | `render/mod.rs`, `render/gpu_safety.rs`, `app.rs`, `tests/render_pipelines_headless_g_pipesmoke.rs`, `tests/fixtures/gpu.rs` | one field feeds all pipelines and textures; the new arm renders a frame |
| 2 | REN-3 + SEL-1 geometry: `FaceMeshVertex`, `face_mesh_pipeline`, `face_state_bgl`, palette in `colors.rs`; the enriched key STILL holds hover and selection, and the build writes the words at build time | `render/mesh_render.rs`, `render/mod.rs`, `render/colors.rs`, `app/gpu_upload/meshes.rs` | same behaviour on a new path; G-PIPESMOKE builds the pipeline |
| 3 | SEL-1 state slot: the key loses hover and selection, `face_state.rs` slot whose key embeds `EnrichedUploadKey`, `(ModelId, FaceGroupId)` inputs, `face_state_writes` stat in the delta log | `render/upload_cache.rs`, `app/gpu_upload.rs`, `app/gpu_upload/face_state.rs`, `app/gpu_upload/meshes.rs`, `app.rs`, `app/viewport.rs`, the sel1 test | the sentry proves zero enriched builds on hover |
| 4 | REN-1 edges: `model_edges` slot, `LineUniforms` 96 B + `line_edge_bind_group`, registry row, `ViewportState` field, `PREFERENCE_OVERLAYS` | `app/gpu_upload/model_edges.rs`, `app/gpu_upload/meshes.rs`, `render/mod.rs`, `render/colors.rs`, `state/viewport.rs`, `ui/overlays/registry.rs`, `app/viewport.rs`, the ren1 test | the registry sentry demands the row and gets it |
| 5 | STL flat-face hover ring (option A) | `app/gpu_upload.rs` | the hover ring finds flat faces |
| 6 | Step 10: id pass, blit outline, readback ring, `CREDITS.md` | `render/mod.rs`, `render/mesh_render.rs`, `app/viewport.rs`, `CREDITS.md`, G-PIPESMOKE arm that reads one pixel | separate and behind a row; off draws as today |

Commits 1, 2-3 and 4 are independent; any order works. Each lands with an
MCP screenshot pair in `planning/viewport_parity_2026-10-28/evidence/`.

Invariants kept:

- F132: two new slots (`face_state`, `model_edges`) and a smaller enriched
  key. Each slot is one file under `app/gpu_upload/`, with an exhaustive
  `SlotInputs::key` and the one `refresh` guard. The face-state key embeds
  the mesh key, so a mesh rebuild always rewrites the state words.
- F173: every colour (palette, ink, accent, edge) is in `colors.rs`; the
  WGSL reads it from a uniform; the shading factors are not colours.
- `gpu_safety`: vertex, index and state buffers go through
  `try_create_buffer`; the offscreen budget is logged; the texture clamp stays.
- WP27: the toolpath draw set does not change; the edges follow
  `ModelVisibility` and `show_model`, not the toolpath selection.
- G-PIPESMOKE: every new pipeline and shader is created in `new`; the
  sentry gains a rendered frame.

## 7. Open questions for the lead

| # | Question | Recommendation |
|---|---|---|
| 1 | Line width: keep 1 px `LineList` or move to quad lines? | Keep 1 px with MSAA; decide on the screenshot pair. If quads, move the hover ring, tab marks and edges together (3.5). |
| 2 | MSAA count: fixed 4 or adapter max? | Fixed 4 with fallback to 1. Only 4 is guaranteed (fact 3); 8 needs a device feature egui does not request, and doubles the 284.8 MiB of 1.5 at 4K. |
| 3 | Keep the unindexed flat path as a fallback? | No. The per-face vertices already give hard creases between faces; the flat path only adds facets inside a curved face. Delete the function `enriched_mesh_gpu_data` in commit 3 (no-legacy ruling 2026-09-16). The type `EnrichedMeshGpuData` stays: `colored_mesh_gpu_data` (reach overlay) returns it; remove the doc line that cites the deleted function. |
| 4 | When to do the ID pass? | After steps 2 (BVH) and 9 (box select). The CPU BVH answers picks; the ID pass earns its cost only for the outline and "visible only". |
| 5 | GL backend: egui gives a GL adapter `downlevel_webgl2_defaults`, which allow zero storage buffers per stage, so the face pipeline fails there. Does the app support GL? | If yes: hold `face_state` in an `R32Uint` texture (width = faces, `textureLoad`), which every backend allows. If no: state "Vulkan, Metal or DX12 only" in the render `CLAUDE.md`. |
| 6 | `gpu_safety` budget: log only, or a gate that drops to 1 sample above a size? | Log only. No VRAM query exists; a gate would use a typed number, which the operator rules out. |
| 7 | Edge offset at far zoom grows with `z² / near`; raise `near` from 0.1? | Raise it with the camera work of step 3 (A_camera.md), where the near plane can follow the scene bounds. |
| 8 | Do STEP faces show item states (Cut, Skipped) as a fill, or only selected and hovered? | Reserve the bits now (2.3); fill item states only after the UX review rules on fill against outline (REPORT A.3b REN-2: UNCONFIRMED in every reference tool). |
