# Simulation rendering review, 2026-10-09

Scope: the simulated-stock display, from the dexel grid to the pixels. Master
`8b6e94df`. This is a review. It changes no code.

Evidence:

- Code: file:line on master `8b6e94df`.
- Measurements: `probe/` in this folder. `probe/src/main.rs` builds a
  rivmap350-sized grid (350 x 500 x 40 mm at 0.2 mm, 4 379 251 cells) with a
  terrain, a vertical-wall pocket, a 45° wall and a through-hole. It times
  each stage and reads normals. `probe/src/bin/shade.rs` renders one uncut
  block through the 6-view composite. `probe/RESULTS.txt` holds the output
  and the machine (Ryzen AI 9 HX PRO 370, release build).
- Run the probe: `CARGO_TARGET_DIR=<dir> scripts/cargo_lane.sh run --release
  --manifest-path planning/sim_render_review_2026-10-09/probe/Cargo.toml`
  (add `--bin shade` for the composite check).
- No live GUI and no MCP run. Each GUI-visible claim below comes from code
  and the probe. A live look must confirm the claims marked "to see".

Three notes on the brief:

- `planning/stock_fill_2026-10-09/DESIGN.md` landed on master during this
  review (`af9e202d`). F2 and F13 refer to it. Its P0 (carry the stock
  across a setup boundary) and its option (c) (a material id per dexel
  segment) are the core halves of P4 and P5 below.
- The playback through-hole defect is FIXED on master (`ddabcdeb`, merge
  `9b594762`). F5 records only what that fix leaves.
- The line numbers below are from `8b6e94df`. The fix moved lines in
  `stock/dexel_mesh.rs` by about 25 after line 33.

---

## 1. The data path

### 1.1 Stock to mesh (core)

| Stage | Where | What it does |
|---|---|---|
| Simulation grid | `dexel_stock/mod.rs:161`, `compute/simulate.rs:1728-1733` | One tri-dexel stock per setup group, FRESH per group, stamped `FromTop` in the setup-local frame. Production stamps the Z grid only. |
| Global playback stock | `simulate.rs:1182`, `stamp_playback_stock` `:1315-1360` | A second stock in the zero-rooted global frame. It takes every Top and Bottom group (`playback_direction`). It skips lateral groups (G-LATERALSCRUB, `:1746-1750`). It is the only stock that carries cuts across setups. |
| Checkpoints | `push_checkpoint` `simulate.rs:1376-1402` | Per toolpath: `mesh_stock` = the group-LOCAL stock (an `Arc`), `stock` = a clone of the GLOBAL stock (or of the local stock for a lateral group). |
| Composite mesh | `finish_group` `simulate.rs:1260-1304` | Per group: `dexel_stock_to_mesh_strided(group_stock)`, drill cylinders, `transform_stock_mesh_to_global`, `composite_mesh.append`. The result is N closed solids appended. |
| Deviations | `simulate.rs:1967-1979`, `compute_deviations` `:2199` | Per vertex of the composite mesh. |
| Full mesh | `stock/dexel_mesh_mc.rs:49-139` | Per-cell envelope, `corner_envelope` `:388-440` (corner = mean of the non-empty neighbour cells), `emit_envelope` `:465-671`: top face, bottom face, perimeter skirt, hole walls, shared corner vertices. |
| Strided mesh | `dexel_mesh_mc.rs:172-326` | The same triangulation on k x k blocks (max top, min bottom). |
| Cavity pass | `emit_cavity_surfaces` `dexel_mesh_mc.rs:704-782` | Runs only when a ray has more than one segment. |
| Playback preview | `dexel_mesh.rs:33-103` | One vertex per CELL CENTRE at the entry-side depth; a fixed-diagonal grid of quads; no bottom, no walls, no skirt. |
| Side grids | `dexel_mesh.rs:184-194`, `:224-322` | Open per-segment sheets. Production has no side grids. |
| Drill cylinders | `dexel_mesh.rs:346` | 16-sided analytic tubes appended to the dexel mesh. |
| Colour | `dexel_mesh.rs:17-22`, `wood_color_at_z` `dexel_mesh_mc.rs:804-811` | Core bakes a tan-to-walnut ramp by depth below the stock top, 3 f32 per vertex. |

### 1.2 Mesh to GPU (viz)

| Stage | Where | What it does |
|---|---|---|
| After a run | `controller/events/compute.rs:837-841`, `:972-976` | `display_deviations` = composite deviations; `display_mesh = None`; playhead to move 0 (or the inspect target). |
| Live stock | `app/simulation.rs:108-477` (`update_live_sim`), called each frame from `app.rs:1243-1251` | Reset from a checkpoint stock clone (`:211-218`) or a fresh grid; replay moves with `simulate_toolpath_range` (`:304`) ON THE UI THREAD. |
| Mesh choice | `app/simulation.rs:345-367` | Playing: the preview mesh. Paused: the full mesh at the run's stride. |
| Throttle | `app/simulation.rs:328-340` | While playing, remesh at most every 50 ms. |
| Frame | `simulate.rs:1075-1095` then `gpu_upload.rs:116-138` | Lateral: local to global. Then global to the PLAYHEAD setup's local frame, per vertex, on the CPU. |
| Colours | `gpu_upload.rs:53-91` | Solid: copy the core colours. Deviation: `deviation_colors(display_deviations)` by index. By height: a ramp. |
| Vertex build | `render/sim_render.rs:390-443` | 36 B vertex (position, normal, colour). Smooth normals: area-weighted sum over SHARED vertices. |
| Upload | `sim_render.rs:167-233` | One buffer pair if each is at most `max_buffer_size`; else `upload_chunked` `:238-309` (a `HashMap` remap per chunk). |
| Reuse | `update_mesh_if_fits` `:319-348`, `update_colors_if_changed` `:358-386` | Rewrite in place only for one chunk. |
| Buffer limit | `render/gpu_safety.rs:14-20`; egui-wgpu 0.36.2 `setup.rs:260-275` | The device asks `wgpu::Limits::default()`: `max_buffer_size` = 256 MiB. |
| Checkpoint load | `app/simulation.rs:20-80`, triggered at `app.rs:1116-1129` | Step back, jump to start, collision click: build the checkpoint mesh, clone it, upload it. |
| Upload pass | `gpu_upload.rs:865-885` | On each `pending_upload` while `has_results()`: recolour or rebuild. |

### 1.3 Draw

| Stage | Where | What it does |
|---|---|---|
| Pipeline | `render/mod.rs:430-459` | `sim_mesh_pipeline`: alpha blend, depth write, `cull_mode: None`, sample count 1 (`:792`). |
| Depth | `render/mod.rs:302-308`, `camera.rs:39-40` | `Depth32Float`, `Less`, near 0.1, far 10 000 (orthographic: -10 000..10 000, `:81`). |
| Light | `app/viewport.rs:449` | One fixed WORLD direction `[0.5, 0.3, 0.8]`. |
| Shader | `render/mod.rs:1466-1522` | Ambient = 0.25 x colour; Lambert; Blinn-Phong 0.2 at power 32; the normal flips TOWARD THE LIGHT (`:1507`). |
| Order | `render/mod.rs:1128-1141` then toolpaths `:1288-1330` | Sim mesh, then overlays, then toolpath lines with depth test and no bias. |
| Move limit | `app/viewport.rs:564-570`, `toolpath_render.rs:127-133` | `toolpath_move_limit = current_move` (global) applied to each toolpath's own move counts. |

### 1.4 The other render paths

| Path | Where | Mesh source |
|---|---|---|
| MCP `screenshot_simulation` PNG | `app/mcp/simulation.rs:196-252` | Global checkpoint stock through `render_stock_composite_in_frame` (`export/fingerprint.rs:677-689`: a fresh full-resolution `dexel_stock_to_mesh`); lateral: `cp.mesh()`. |
| MCP `screenshot_simulation` HTML | `app/mcp/simulation.rs:274-286` | `results.mesh` (the composite). |
| MCP `screenshot_toolpath` with stock | `app/mcp/view.rs:219` | `results.mesh` (the composite). |
| CPU rasteriser | `export/fingerprint.rs:858-927`, `:1149-1375` | Painter sort plus a z-buffer of the triangle CENTROID depth; flat face shading. |
| Rest heatmap, tier preview, By Area | `gpu_upload.rs:1330-1460` | Reuse `SimMeshGpuData` and the same shader. |

---

## 2. Findings, ranked by what the operator sees

Impact: H = a wrong picture or a stall of a second or more on the
operator's projects. M = a visible artefact or a large waste. L = hygiene.

### F1 (H) — Every mesh is inside out; the 6-view composite renders every lit face dark

- Where: top face `dexel_mesh_mc.rs:541` emits `[v00, v10, v01, …]`. `v10`
  is +v (Y), `v01` is +u (X). `(0,c,0) x (c,0,0) = (0,0,-c²)`: the normal
  points DOWN. The bottom face, the skirt and the hole walls are reversed in
  the same way. The doc at `:45-47` says "CCW winding around outward-facing
  normals". The preview has the same winding (`dexel_mesh.rs:94`).
- Evidence: `probe/RESULTS.txt`. A floor vertex at z 5 has normal
  `(0, 0, -1)`. The perimeter vertex at x = -0.1 has normal `(+1, 0, -0.01)`,
  which points into the block. `shade.rs`: the TOP panel of an uncut block
  reads 52.3 (shipped winding) and 137.3 (flipped). 0.35 x 151 = 53 and
  0.91 x 151 = 138: the rasteriser (`fingerprint.rs:1232-1234`, dot clamped
  at 0) lights only the faces that face AWAY from the lamp.
- What the operator sees: each `screenshot_simulation` PNG shows the stock
  with inverted relief. Faces toward the lamp are at ambient; slopes away
  from the lamp are bright. Agents read these PNGs as evidence.
- Why the GUI hides it: the GPU shader flips each normal toward the light
  (`render/mod.rs:1447`, `:1507`, `:1528`) and culls nothing. That flip makes
  the winding invisible, and it causes F9.
- Fix sketch: swap the winding in `emit_envelope` (top, bottom, skirt, walls),
  the cavity pass and the preview. Add a sentry: every top-face normal has
  z > 0 and the signed volume of the uncut block is positive. Then F9 can use
  `front_facing`.

### F2 (H) — Multi-setup: three different pictures of the same stock

- Where: each group starts from a fresh stock (`simulate.rs:1728-1733`). A
  checkpoint mesh comes from the group-LOCAL stock (`build_mesh`
  `simulate.rs:548-561`). The composite appends N full solids
  (`finish_group` `:1291-1303`). Live playback uses the GLOBAL stock.
- What the operator sees:
  - Live scrub forward: every Top and Bottom cut (correct).
  - The checkpoint mesh (step back, jump to start, collision click): only
    the cuts of its own setup on a fresh block. In practice F3 discards it in
    the same frame; the MCP lateral PNG and the HTML still show it.
  - `results.mesh` (MCP HTML, `screenshot_toolpath` with stock): two or
    three overlapped closed solids. For Top + Bottom, setup 2's uncut top
    plane covers setup 1's floor from above. The rivmap block job has three
    setups.
  - Deviations (`simulate.rs:1978`) are computed on that overlap. A vertex
    of the uncut plane of one setup reads "material remaining" over a floor
    that another setup cut.
  - Lateral: the global stock never takes a lateral cut. A lateral group
    replays into a fresh local stock (documented at
    `app/simulation.rs:101-105`). When the last setup is lateral, the end
    state shows ONLY that setup's cuts.
- `stock_fill_2026-10-09/DESIGN.md` §1.6 reaches the same result from the
  epoxy job: the scrub shows the channel holes; a checkpoint shows a fresh
  block with no channels; the setup-3 gates read the fresh stock.
- Fix sketch: one display stock. The DESIGN P0 carry (§2.0) makes each
  group start from the previous group's stock; then append only the LAST
  Z-axis group's mesh (§2.0 item 2) and resume the scrub from
  `prior_stocks` (§2.0 item 4). Without P0, the global playback stock is the
  only true source: build the final mesh and the
  deviations from it and delete the composite append. Each checkpoint
  publishes its global stock as the mesh source. Lateral needs a global stock
  that takes side cuts (STK-13) or a local stock seeded from the previous
  group (a resample). Package P4 carries this.

### F3 (H) — The upload pass rebuilds a chunked sim mesh on every selection click; pause and step stall for about 1.3 s

- Where: `gpu_upload.rs:865-885` runs on every `pending_upload` while
  `has_results()`, in EVERY workspace. For more than one chunk it calls
  `from_heightmap_mesh_colored`, which rebuilds the whole mesh. The sim mesh
  has no `upload_cache` key; `render/CLAUDE.md` says every upload goes
  through `upload_cache.rs`.
- Evidence (`probe/RESULTS.txt`, rivmap350 size at 0.2 mm): full mesh
  8 751 744 vertices and 17 503 488 triangles; GPU 315 MB vertex + 210 MB
  index = 525 MB, above the 256 MiB buffer limit, so 8 chunks. Costs on the
  UI thread: marching cubes 180 ms, normals 100 ms, the chunk `HashMap` remap
  906 ms, then 8 buffer creations. About 1.3 s plus the driver copy.
- The display stride never helps. Every production request sends
  `display_stride: 1` (`controller/events/simulation.rs:383`,
  `session/compute.rs:2046`, `compute/sim_prefix.rs:879`; the fix agent of
  `ddabcdeb` found the same). `display_stride_for` (`dexel_mesh_mc.rs:350`)
  has no production caller.
- Also: step back runs `load_checkpoint_for_move` (`app.rs:1128`), then
  `update_live_sim` in the same frame (`app.rs:1250`) resets from a
  checkpoint and remeshes again (`app/simulation.rs:181`). The paint
  callback reads the resources after both, so the first mesh is never shown.
  A step back costs two full builds.
- Fix sketch:
  1. Key the sim mesh in `upload_cache` (display mesh identity + viz mode +
     frame). Skip the slot outside the Simulation workspace.
  2. Ask the adapter for its real `max_buffer_size` (desktop Vulkan reports
     2 GiB or more) through the `device_descriptor` closure.
  3. Keep chunking as a fallback, but split by corner-row bands (the vertex
     order is row-major) instead of a `HashMap` remap.
  4. Put colour in a separate vertex stream so a mode change writes 12 B per
     vertex and no normals.
  5. Delete the checkpoint mesh load on step back, or make it the only route.
  6. Build the paused mesh on a worker, not on the UI thread.

### F4 (H) — Deviation colours paint the wrong vertices

- Where: `display_deviations` is one value per COMPOSITE vertex
  (`simulate.rs:1978`). `compute_sim_colors` maps it by index onto the
  CURRENT display mesh (`gpu_upload.rs:74-86`): a live, preview or
  checkpoint mesh. `build_vertex_data` hides a length mismatch with
  `colors.get(i).unwrap_or(fallback)` (`sim_render.rs:402`).
- What the operator sees: right after a run the playhead is at move 0
  (`controller/events/compute.rs:975`). The uncut block then shows the FINAL
  deviation colours. During playback the preview has 4 379 251 vertices on a
  cell-centre lattice; the composite has 8 751 744 on a corner lattice
  (probe). Every colour sits on an unrelated vertex. A match happens only for
  one identity setup with the playhead at the end. Multi-setup is never right
  (F2).
- Fix sketch: compute the deviation per displayed vertex, or per grid cell
  from the model's top Z on the same grid (one model query per cell, held
  with the run). Abstain (draw plain, say so in the legend) when the shown
  mesh is not the final one. Assert `colors.len() == vertex_count`.

### F5 (L) — Playback preview: residual after the fix

- Fixed on master (`ddabcdeb`): an empty ray, or one thinner than
  `MIN_MATERIAL_THICKNESS`, is now a gap, so a through-cut no longer draws a
  column of uncut stock with false walls. Do not re-open it.
- What remains (`dexel_stock_to_entry_surface_mesh`):
  - The preview keeps one vertex per cell CENTRE. The paused mesh uses cell
    CORNERS (`corner_envelope`). Each pause and play moves the surface by half
    a cell and swaps a 263 MB and a 525 MB GPU buffer set (probe sizes).
  - A hole edge in the preview is up to one cell wider than the true edge
    (stated in the fix's own doc).
  - The fixed quad diagonal still gives a saw-tooth on a steep diagonal wall
    that does not go through.
  - The winding is inside out, as in F1.
- Fix sketch: none on its own. P3 (one GPU height field for preview and
  paused) removes the second lattice.

### F6 (M) — The toolpath overlay in Simulation trims each toolpath by the global cursor

- Where: `app/viewport.rs:564-570` sets `toolpath_move_limit =
  current_move`, a run-global count. `render/mod.rs:1288` passes it to
  `vertices_for_moves`, which counts LOCAL moves (`toolpath_render.rs:127-133`).
- What the operator sees: a toolpath that starts at global move 50 000
  draws in full when the cursor reaches its first move. With show-all on,
  every later toolpath shows its first N moves early. Entry markers hide
  under any limit (`:1343-1344`).
- Also: the toolpaths, fixtures and stock wireframe take the frame of the
  SELECTED setup (`gpu_upload.rs:177-207`); the sim mesh takes the frame of
  the PLAYHEAD setup (`gpu_upload.rs:121`). When playback crosses into a
  flipped setup and the selection stays, the overlay and the stock disagree
  (to see).
- Fix sketch: pass per toolpath `current_move - boundary.start_move`
  (clamped), from `boundaries()`. Make the playhead setup the frame for every
  layer in Simulation.

### F7 (M) — Playback frame time and the UI thread

- Evidence (probe, rivmap350 at 0.2 mm): one preview refresh = 66 ms mesh +
  50 ms normals + the colour copy + a 263 MB `write_buffer`, plus the stamp.
  That is well over 100 ms. The 50 ms throttle (`app/simulation.rs:328`)
  cannot hold; the UI runs at a few frames per second while it plays.
- The deflection lookup scans the whole cut trace once per move
  (`app/simulation.rs:576-595`; 5 159 038 samples on rivmap350,
  `memory_budget_2026-10-01/BASELINES.md` W2).
- The tool wireframe makes a new GPU buffer for each move
  (`app/simulation.rs:646`).
- Memory: the CPU mesh is 96 B per cell (`budget/estimate.rs:74`), 420 MB at
  this size. The view holds the composite (`Arc`), `display_mesh`, a
  checkpoint mesh in the cache plus its clone (`app/simulation.rs:36`), and
  transient copies (colours 24 B per vertex, `mesh_verts` 36 B per vertex,
  normals 12 B per vertex). The GPU copy is 120 B per cell. The estimator has
  no GPU term. BASELINES W4 records 6.72 GiB at rest after a run; the mesh
  copies are a large part of that (not measured one by one).
- Fix sketch: package P3 (GPU height field) removes the per-frame mesh. In
  the short term: remesh only the dirty row bands, index the deflection by
  move, and update the tool position by a uniform, not a buffer.

### F8 (M) — Edge definition: smooth normals across every crease

- Where: `sim_render.rs:406-440` sums face normals over shared vertices. The
  envelope shares one vertex between the top face, the hole wall and the
  skirt (`emit_envelope` `:498-520`).
- Evidence (probe): the rim vertex of a 16 mm pocket wall has normal
  `(-1.0, 0, -0.03)`; the top vertex of the stock edge has normal
  `(1.0, 0, -0.01)`. The tall wall triangles dominate the area weight, so
  each top-face cell next to a wall shades as a wall. Every edge reads as a
  rounded bead, not a sharp edge.
- Fix sketch: split the normal at a crease (dihedral above about 40°), or
  derive the normal per pixel from the height field (P3). Add an edge or
  cavity darkening term for definition.

### F9 (M) — Lighting: the shader lights both sides of every face

- Where: `render/mod.rs:1507`: `n = select(normal, -normal, dot(normal,
  light) < 0)`. The light is fixed in the world (`app/viewport.rs:449`).
- What the operator sees: no face is ever in shadow. Two pocket walls that
  face opposite ways shade the same. The relief reads flat, most of all from
  low or rear views. The specular is a fixed plastic 0.2 at power 32.
- Fix sketch (after F1): flip by `front_facing` (toward the viewer). Use a key
  light tied to the camera, a fill light and a hemisphere ambient. Lower the
  specular for wood. Confirm the target format: the pipesmoke sentry notes an
  sRGB target is possible on Linux, and the shader writes values with no
  colour-space step.

### F10 (M) — Image quality: no MSAA, toolpath lines z-fight, wide depth range

- Where: sample count 1 on every pipeline (`render/mod.rs:792`, `:803`). The
  line pipeline uses the same depth test with no bias. Near 0.1 and far
  10 000 with a standard (not reversed) `Depth32Float`.
- What the operator sees: jagged edges on the stock silhouette, and cutting
  moves that lie on the cut floor dash in and out of the surface (to see).
- Fix sketch: MSAA 4x on the offscreen target. A small depth bias, or a
  "lines on top with a dim pass for hidden parts" scheme, for the toolpath
  overlay. Reversed-Z, or a near plane from the scene bounds.

### F11 (M) — The CPU rasteriser depth-tests with one depth per triangle

- Where: `fingerprint.rs:1205-1207` sorts by centroid depth;
  `rasterize_triangle` tests `depth > zbuf[zi]` with that constant
  (`:1340`). The skirt and wall triangles are tall (full stock height over
  one cell), so their centroid is far from their visible pixels.
- What the operator sees: walls and floors can paint over each other in the
  composite. The "vertical striping" of RUN_LOG defect 3 has aliasing as its
  stated cause; this test is a second candidate (not measured).
- Fix sketch: interpolate the depth per pixel with the barycentric weights
  the function already computes. Fix F1 first.

### F12 (M) — Drill cylinders sit on top of the dexel walls

- Where: `dexel_mesh.rs:346-420` appends a 16-sided tube from
  `hole.bottom_z` to `hole.top_z`, captured at drill time, inside the dexel
  hole.
- Risk: the dexel hole wall is a one-cell ramp that crosses the tube, so the
  two surfaces z-fight along the hole. A later facing pass lowers the surface
  but not `top_z`, so the tube stands above the new surface (to see).
- Fix sketch: clip the tube to the current column tops, or drop it when the
  cell size is much smaller than the hole radius (0.2 mm cells already
  resolve a 3 mm hole).

### F13 (M) — Wood look and material

- Where: core bakes the colour (`dexel_mesh.rs:17-22`). The ramp maps depth
  below the stock top over the full stock height (`wood_color_at_z`). A 1 mm
  skim reads uncut; a deep floor reads walnut whatever the cut was.
- Against Fusion, Vectric and CAMotics: those show a material base colour
  with lighting doing the depth cue; some tint each tool or operation. rs_cam
  has no species colour, no grain, no per-operation tint and no "cut vs
  uncut" flag.
- Epoxy: no second material exists in the stock model
  (`rivmap_block_job.md:77`). The mesh carries 3 f32 of colour per vertex
  and no material id. `stock_fill_2026-10-09/DESIGN.md` §2.3 proposes
  `DexelSegment.material: u8` and a sibling of `wood_color_at_z` for a fill
  tag.
- Fix sketch: move colour out of core (the `stock/CLAUDE.md` rule:
  "StockMesh container only"). Carry the DESIGN §2.3 tag of the top segment
  per corner (u8) and a "cut" flag, not a baked RGB. A second baked ramp in
  core would repeat today's coupling. The viz palette maps id to base colour and gloss: wood species from
  `material/`, epoxy glossy and translucent. Procedural grain along a stock
  axis in the fragment shader. This saves 23 B per vertex against the 12 B
  of colour now.

### F14 (L) — Undercuts and cavities

- `emit_cavity_surfaces` (`dexel_mesh_mc.rs:704-782`) is unreachable in
  production: no production path calls `ray_subtract_interval`, and Z-only
  stamping from the top or the bottom never splits a ray. If it ran, it
  would emit floors and ceilings on the cell-CENTRE lattice inside an
  envelope on the CORNER lattice, with no cavity walls (the doc at `:696`
  says it emits walls), and the skirt would hide a side opening. Keep it for
  STK-13 or delete it with its doc; do not trust it as is.
- Side grids (`dexel_mesh.rs:184-322`) are test-only in production.

### F15 (L) — Stale-cache and hygiene items

- `ColorFingerprint` (`sim_render.rs:71-101`) compares length, first, last
  and the sum of every 64th colour. A change elsewhere can match it and keep
  stale colours.
- Literal colours in pipelines against the `render/CLAUDE.md` rule:
  `deviation_colors` `sim_render.rs:117-139`, the fallback `[0.65, 0.45,
  0.25]` at `sim_render.rs:156`, `:402` and `gpu_upload.rs:71`, `:84`,
  `deflection_render_color` `app/simulation.rs:748-759`, and the shader
  constants `render/mod.rs:1444-1458`.
- The deviation scale is fixed (±0.1 mm green band, red below -0.5 mm) and
  has no tolerance input.
- Names: `from_heightmap_mesh*` after the heightmap was deleted (C27);
  `z_grid_to_solid_mesh` (`dexel_mesh.rs:209`) is a one-line wrapper;
  `dexel_stock_to_top_surface_mesh` is a test door.
- `display_degrade`, the stride banner (`ui/sim_timeline.rs:46`),
  `z_grid_marching_cubes_strided`, `dexel_stock_to_mesh_strided` above 1 and
  `display_stride_for` are code that production never drives (F3).

---

## 3. Architecture

### 3.1 Duplicate paths

| Concern | Copies |
|---|---|
| Mesh builders | Full MC, strided MC, preview, side sheets, drill tubes, cavity pass. Two vertex lattices (corner, centre). |
| Stock sources | Group-local stock (checkpoints, composite), global stock (live, PNG), lateral local stock. |
| Frame mapping | `transform_stock_mesh_to_global` (core, per vertex), `transform_mesh_to_local_frame` (viz, per vertex, each refresh), `sim_mesh_in_world_frame` (MCP), `SetupFrame` → `SetupTransformInfo` → `FaceUp`/`ZRotation`. Every one is a rigid map: one 4 x 4 model matrix per draw replaces all the per-vertex loops. |
| Renderers | GPU viewport (smooth normals, two-sided light), CPU composite (flat normals, clamped light, centroid depth), three.js HTML. They do not agree on shading (F1). |
| Upload routes | Live sim (`app/simulation.rs:433-463`), checkpoint (`:50-78`), upload pass (`gpu_upload.rs:865-885`). Three copies of "rewrite or rebuild". |

### 3.2 Coupling to core

- Core chooses colours (`dexel_mesh.rs:17-22`). Core is GUI-free by rule.
- Viz reads core's `StockMesh` with colour, then copies it into a GPU vertex.
- Core `SimulationResult` holds a display mesh (`simulate.rs:631`) that only
  MCP and the deviation pass read. The CLI and the GUI viewport never draw it.

### 3.3 Dead or unreachable

- `emit_cavity_surfaces` in production (F14).
- Side grids in production (kept by the STK-13 ruling).
- The display stride above 1 (degrade 4b): all three request builders send
  1 (`controller/events/simulation.rs:383`, `session/compute.rs:2046`,
  `compute/sim_prefix.rs:879`). Either wire it to the preflight with a GPU
  term (120 B per cell, F7), or let P3 replace it with a texture mip level
  and delete it.
- The checkpoint mesh on the step-back path (F3: built, then overwritten).

---

## 4. Recommended plan

Order: P1 → P2 → P4 → P3 → P5. P4 shares its core change with the
stock-fill P0; do them as one package or in strict order. P1 and P2 are safe and quick, and they
remove the worst pictures and stalls. P4 makes the picture true for
multi-setup jobs before P3 rebuilds the renderer.

| Package | Content | Size | Sentries |
|---|---|---|---|
| **P1 Correct pictures** | F1 winding (MC, preview, cavity); F4 gate (abstain off the final mesh, assert the length); F6 move-limit rebase; F11 per-pixel depth; legend says when deviation abstains. | S (2-3 days) | Normal z > 0 on top faces; positive signed volume; deviation length; per-toolpath limit; `shade.rs` turned into a test (top panel brightness above 120). |
| **P2 No stalls** | F3: an `upload_cache` key, the workspace gate, the adapter buffer limit, row-band chunking, a colour stream, delete the wasted step-back build; build the paused mesh on a worker; F7 deflection index and tool uniform. | M (3-5 days) | Upload stats: a selection click gives 0 sim builds; one step back gives 1 build; the probe timing in a bench. |
| **P4 One stock for display** | F2, together with `stock_fill_2026-10-09` P0 (the carry): the final mesh, deviations, PNG, HTML and checkpoints from one carried stock; append only the last Z-axis group's mesh; one frame door as a model matrix; playhead frame for every layer in Simulation. Lateral: decide between seeding the local stock and STK-13. | M (4-6 days) + a ruling on lateral | A Top + Bottom fixture: the final mesh shows both cuts from above and below; checkpoint mesh = live mesh at each boundary. |
| **P3 GPU height field** | The Z grid top and bottom as R32F textures (17.5 MB each at rivmap350 versus 525 MB of mesh). One static tile index buffer; the vertex shader builds the corner envelope; holes by discard; walls and skirt from a static strip. Playback writes only the dirty row bands. Normals per pixel from the texture. Deviation per cell on the GPU from a model-height texture. One path for live, preview, paused and final; the stride becomes a mip level. | L (2-3 weeks) | Pixel parity against the P1 mesh on fixtures; frame time under 16 ms at rivmap350 during playback. |
| **P5 Look and material** | F9 lighting, F10 MSAA and depth, F8 crease normals (free with P3), F13 palette + grain + epoxy colour from the DESIGN §2.3 segment tag, F12 drill tubes, F15 colours into `colors.rs`. | M (1 week) | Screenshot review by the operator; the colour-literal scan in `render/`. |

Not in the plan: a full tri-dexel mesher for true undercuts. It becomes
necessary only when lateral or epoxy-from-below cuts must show on one global
stock.

---

## 5. What to keep

- The corner-envelope extraction (`corner_envelope`, `emit_envelope`). It is
  watertight, it gives sub-cell walls from the coverage-weighted tops, and
  the stride build shares it bit for bit (`stride_tests.rs`).
- M2/M3 lazy checkpoints (`SimCheckpointMesh::build_mesh`, the shared
  `Arc`). They keep memory down; P4 changes only which stock they read.
- The G-LATERALSCRUB local replay. It is the only route that shows a
  lateral cut today.
- `transform_stock_mesh_to_global` as the one core door for the frame, and
  the proven rotations: every `FaceUp` and `ZRotation` arm has determinant +1
  (`compute/transform.rs:113-125`, `:227-234`), so no transform mirrors the
  mesh or flips its winding. The mirrored-panel class of defect is closed.
- `gpu_safety` and chunking as a fallback for a device that really has a
  small buffer limit.
- The 6-view composite camera convention (one frame, one scale, labelled
  panels, supersampling). Fix its shading and depth, not its layout.
- The upload stats counters (`resources.upload_stats`): they make P2
  measurable.
- The playback band dispatch (`dexel_stock/playback.rs`) and the bit-identity
  sentries.
