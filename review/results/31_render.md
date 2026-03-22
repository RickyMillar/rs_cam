# Review: Render Pipeline

## Summary

The rs_cam render pipeline is a well-structured wgpu+egui-based 3D visualization system featuring three parallel rendering paths (mesh, colored-mesh, and line-based) with offscreen rendering to a depth buffer, committed via a fullscreen blit. The architecture properly decouples GPU resource management from the frame loop and implements a clean callback-based integration with egui. However, there are gaps in line rendering quality, missing visual regression testing, and incomplete buffer re-upload optimization patterns.

## Findings

### Architecture & Integration

**Strengths:**
- Clean separation of concerns: `RenderResources` encapsulates all GPU state, pipelines, and buffer management independently from UI or application logic
- Callback-based rendering pattern (egui-wgpu) keeps 3D viewport isolated within egui frame flow
- Offscreen rendering with depth buffer provides proper Z-ordering for complex scene composition
- Three distinct pipelines (mesh, colored-mesh, line) with appropriate blend modes:
  - Mesh & sim-mesh pipelines: depth testing, CCW culling, Phong lighting
  - Line pipeline: transparent line list, no culling
  - Blit pipeline: fullscreen triangle sampling offscreen texture
- ViewportCallback encapsulates per-frame uniform writes and render pass coordination

**Issues:**
- Line rendering uses `LineList` topology with 2-vertex segments — no geometry expansion or line width specification in shader. All toolpath, grid, and fixture lines render at 1-pixel width
- Offscreen target is recreated every frame when size changes but dimensions are passed per-frame in ViewportCallback (redundant double-check in `ensure_offscreen`)

### Camera

**Strengths:**
- Perspective projection with 45deg FOV, near=0.1, far=10000 (suitable for large CAM workspaces)
- Orbit (yaw/pitch) with clamped pitch to +/-90deg
- Pan scales with distance for intuitive navigation
- Zoom uses exponential scaling for smooth feedback
- `fit_to_bounds()` auto-frames scene
- `project_to_screen()` and `unproject_ray()` for picking/cursor feedback
- 4 preset views: Top, Front, Right, Isometric
- Unit test (`unproject_round_trip`) validates projection/unprojection consistency with <0.1mm error

**Issues:**
- No orthographic mode (projection is always perspective)
- `orbit()` input sensitivity (0.005 rad/pixel) is hardcoded; no configuration
- `fit_to_bounds()` uses fixed 1.8x multiplier; may be too tight for small models or too loose for large scenes

### Mesh Rendering

**Strengths:**
- Flat shading with per-face normals preserves faceted geometry aesthetic
- Phong lighting: ambient (0.15), diffuse (Lambert), specular (Blinn-Phong with 32x shininess)
- Two-sided lighting: flips normal if back-facing, enabling visualization of watertight or non-watertight models

**Issues:**
- `from_mesh()` duplicates vertices per triangle (3x memory overhead). No deduplication or index reuse. For large STL files (50k+ triangles), this increases VRAM and upload time
- Lighting parameters (ambient, diffuse_color, specular intensity) are hardcoded in shader. No ability to switch between material modes
- No wireframe or edge-rendering modes for model inspection
- Mesh upload happens in `app.rs` context, not in render coordinator. No invalidation tracking; old GPU buffer persists until explicitly replaced

### Toolpath Rendering

**Strengths:**
- Deterministic 8-color palette with modulo cycling for per-toolpath coloring
- Z-depth blending (30% depth influence): darker at bottom, brighter at top, improving visual stratification
- Separate cut/rapid buffers with move-indexed counts enable partial scrubbing during simulation
- Cut moves color-coded by Z-depth; rapids dimmed (35% intensity)
- Entry path preview (ramp/helix/lead-in indicators) with cyan overlay
- Entry marker arrowheads show approach direction and entry point
- Selected toolpaths brighten by +30% for visual distinction

**Issues:**
- Lines are 1-pixel width. For dense multi-operation jobs, individual rapid/cut lines may be hard to distinguish
- Entry path preview config mirrors DressupEntryStyle without serde to avoid coupling — brittle if dressup definitions change
- `palette_color()` uses hard-coded 8-color array. No way to customize or accent specific toolpaths
- Move count tracking is cumulative; scrubbing always starts from move 0. No random-access to arbitrary move ranges

### Simulation Rendering

**Strengths:**
- Per-vertex colored mesh enables rich visualization of simulation results
- Three deviation color modes:
  - **Deviation**: Green (on-target +/-0.1mm), Blue (remaining), Yellow (slight overcut), Red (major overcut)
  - **Height gradient**: Blue (low Z) -> Green (mid) -> Red (high Z)
  - **Operations**: Placeholder (uniform wood tone), documented as unimplemented
- Per-face normal accumulation from triangles for smooth lighting on heightmap mesh
- Configurable opacity (default 0.18 for solid stock) with alpha blending
- Tool geometry wireframes (FlatEnd, BallNose, BullNose, VBit, TaperedBallNose) with shank and holder visualization

**Issues:**
- Deviation color thresholds (+/-0.1mm, +/-0.5mm) are hardcoded. No way to adjust sensitivity for coarse vs. fine tools
- Deviation color computation is done per-vertex on CPU every frame, then re-uploaded. No caching of color buffers if simulation geometry hasn't changed
- Tool wireframe generation is verbose (~400 lines for tool geometry drawing). No LOD for distant tools

### Stock, Fixture & Reference Geometry

**Strengths:**
- Stock visualized as both wireframe (always visible) and solid (semi-transparent, spatial context)
- Fixture and keep-out zones as colored wireframe boxes
- Pin markers as cyan crosshairs
- Ground grid at Z=0 with configurable spacing
- Axis indicators (X=red, Y=green, Z=blue) with dynamic length scaling based on stock dimensions
- Height planes at key Z heights (clearance, retract, feed, top, bottom) with intuitive color coding

**Issues:**
- Grid generated once at startup; no dynamic update if stock bounds change
- Grid and axis colors hardcoded; no high-contrast mode
- Height planes only shown when a toolpath is selected; no independent toggle
- Plane quads always span full stock bounds. No clipping to operation bounding boxes
- Keep-out and fixture boxes both use the same wireframe drawing, making them visually indistinct unless color differs

### Performance

**Buffer Upload Patterns:**
- All geometry uses `BufferInitDescriptor` (one-shot, CPU->GPU copy at creation)
- No streaming, persistent mapping, or dynamic buffer updates
- Mesh: only re-uploaded when model changes (rare)
- Toolpaths: recreated for each CPU result update (once per operation compilation)
- Simulation mesh: full re-upload on each simulation step or color mode change
- Grid: uploaded once at construction

**Line Rendering Bottleneck:**
- `LineList` topology with per-vertex colors means every line segment requires 2 vertices
- For a 1000-operation job with 5000 moves per operation: ~10M vertices, ~240 MB GPU memory
- WGPU does not support `gl_LineWidth`. Line thickness must be achieved via geometry expansion, compute shader rasterization, or SDF rendering. Current implementation uses none of these.

**Large Model Risks:**
- STL mesh with 100k triangles -> 300k vertices after duplication -> 7.2 MB VRAM
- No frustum culling: all geometry rendered every frame regardless of visibility
- No LOD system for simulation meshes

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Line rendering always 1-pixel width; dense toolpaths become illegible | mod.rs:327, LINE_SHADER_SRC |
| 2 | High | Mesh vertex duplication (3x overhead per triangle); no index reuse | mesh_render.rs:45-70 |
| 3 | High | Simulation mesh colors recomputed + re-uploaded every frame even if geometry unchanged | sim_render.rs:62-84 |
| 4 | Med | No isolation mode in render pipeline; cannot hide all but one toolpath | toolpath_render.rs, mod.rs:721-748 |
| 5 | Med | Deviation color thresholds hardcoded; no sensitivity adjustment | sim_render.rs:66-77 |
| 6 | Med | Height planes only visible when toolpath selected; no independent toggle | app.rs:1440-1441 |
| 7 | Med | Offscreen target size validated twice per frame | mod.rs:440-504 |
| 8 | Low | Lighting parameters hardcoded in shader; no material switching | mod.rs:825-839 |
| 9 | Low | Grid and axis colors hardcoded; no high-contrast mode | grid_render.rs:16, 44-71 |
| 10 | Low | No orthographic projection mode | camera.rs |
| 11 | Low | Entry path preview duplicates DressupEntryStyle without serde coupling | toolpath_render.rs:206-225 |
| 12 | Low | Tool wireframe generation verbose (~400 lines); no LOD | sim_render.rs:267-633 |

## Test Gaps

- **No GPU integration tests:** Cannot validate rendering without a headless GPU context
- **No visual regression:** No snapshot or diff-based tests for frame output
- **No stress tests:** No validation of 10k+ line segment rendering or multi-megabyte mesh uploads
- **No shader validation:** Shaders are embedded as strings; typos or logic errors only caught at runtime
- **Camera tests are minimal:** One round-trip test exists, but no tests for orbit edge cases, fit-to-bounds with degenerate bounds, or preset view orientations

## Suggestions

### Short-Term (High Impact)
1. **Add configurable line width via geometry expansion** — expand line segments to 2-triangle quads in vertex shader or via pre-processing. All toolpaths become visible and professional-looking.
2. **Cache simulation mesh colors** — store both deviation and height gradient variants in GPU; switch via uniform instead of re-uploading.
3. **Add selective toolpath visibility (isolation mode)** — add `hidden_toolpath_indices` bitset to ViewportCallback; skip draw calls for hidden toolpaths.
4. **Fix height plane visibility toggle** — decouple from toolpath selection; add standalone `show_height_planes` flag.

### Medium-Term
5. **Optimize mesh vertex duplication** — use indexed rendering with per-face normals to halve VRAM for large STL.
6. **Add frustum culling** — CPU-side bounding box tests to skip off-screen geometry.
7. **Expose lighting & material parameters** — move hardcoded lighting to uniforms; add UI sliders for material preview modes.
8. **Add shader validation tests** — compile shaders to SPIR-V at build time and validate entry points.

### Long-Term
9. **SDF line rendering** for smooth lines at arbitrary widths on any DPI.
10. **GPU-driven rendering & indirect dispatch** — compute shaders to cull geometry on GPU for 100k+ line segments.
11. **Visual regression testing** — reference frame captures for camera presets, toolpath modes, etc.
