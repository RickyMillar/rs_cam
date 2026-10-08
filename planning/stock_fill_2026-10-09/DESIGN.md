# Stock fill between setups (epoxy pour): design

Date: 2026-10-09. Status: DESIGN ONLY. No code changed, no commit.

Need (operator, 2026-10-09): "can we add/remove models in setups and have the
sim deal with that? then we could add in the epoxy pours."

Case: the two-sided RivMap test piece
(`planning/rivmap_block_job/testpiece.toml`, report
`planning/rivmap_block_job.md`, process sheet
`project_rivmap_mono/manufacture/block/TESTPIECE.md`).

1. Setup "1 Back channels" (Top) mills the channels from the back.
2. Off the machine, the operator pours epoxy into the channels, 1 mm over the
   back face, and lets it cure.
3. Setup "2 Back cavity" (Top) faces the overfill and pockets the cavity.
4. Setup "3 Front" (Bottom, flipped on the dowels) faces the front and cuts
   the relief. The relief exposes the epoxy.

Today the simulation shows the channels as open holes through the front.
Nothing in the model knows about the pour.

Line numbers are at master `8b6e94df`.

---

## 1. How the stock and the simulation work now

### 1.1 The dexel stock

- `DexelSegment { enter: f32, exit: f32 }` is one material interval on one
  ray (`crates/rs_cam_core/src/stock/dexel.rs:15`).
- `DexelRay = SmallVec<[DexelSegment; 1]>` is a sorted list of segments that
  do not overlap (`stock/dexel.rs:39`). A fresh ray holds one inline segment.
- The ray operations remove material only: `ray_subtract_above` (`:45`),
  `ray_subtract_below` (`:66`), `ray_blend_above/below` (`:97`, `:132`),
  `ray_subtract_interval` (`:168`). No operation adds material.
- `DexelGrid` (`stock/dexel.rs:266`) holds the rays and `conservative_top`, a
  per-cell upper bound on the material top. Its contract is "lowered only"
  (`lower_conservative_top`, `:499`). The rapid-clearance checks read it.
- `DexelGrid::from_bounds` (`stock/dexel.rs:378`) fills every ray with one
  segment over the bbox. Cell nodes sit at `origin + i * cell`
  (`cell_to_world`, `:438`), `rows = ceil(D / cell) + 1`.
- `TriDexelStock` (`dexel_stock/mod.rs:86`) holds the Z grid and two optional
  side grids. Production stamps the Z grid only.
- The mesh extraction (`stock/dexel_mesh_mc.rs`) reads the bbox Z range, and
  colours a surface by its depth below the stock top
  (`wood_color_at_z`, `dexel_mesh_mc.rs:804`). There is one material colour
  ramp.

### 1.2 Two stocks in one run, and only one of them carries

`run_simulation_memoized` (`compute/simulate.rs:1607`) keeps two stocks.

| Stock | Built at | Frame | Carries across setups? | Readers |
|---|---|---|---|---|
| per-setup `group_stock` | `simulate.rs:1731`: `TriDexelStock::from_bounds(local_bbox, ..)` for EVERY group | setup-local (world for an identity setup, F-024) | **No.** Each group starts from a full block. | the carve and the cut trace (`carve_entry`, `:1440`), every metric and load gate, rapid checks, `prior_stocks` (`record_pre_carve`, `:1556`), the checkpoint display mesh (`SimCheckpointMesh::mesh_stock`, `:476`, `build_mesh`), the composite mesh (`finish_group`, `:1257`, append at `:1305`) |
| `global_stock` | `cold_prefix_state`, `simulate.rs:1182`, zero-rooted bbox at `:1638` | stock-relative global | **Yes.** `stamp_playback_stock` (`:1319`) stamps every Top/Bottom group into it in turn. | `SimCheckpointMesh::stock`, the GUI live scrub (`rs_cam_viz/src/app/simulation.rs:108`, `update_live_sim`, mesh at `:366`), the S5 prefix memo |

The doc comment at `dexel_stock/mod.rs:104-112` (2026-08-22) says this in
plain words: rest generation reads `prior_stocks`, and those are clones of
the per-setup local `group_stock`.

### 1.3 "After previous ops" (`StockSource::FromRemainingStock`)

- The generator reads `simulation.prior_stocks[tc.id]`
  (`session/compute.rs:2244-2250`), only when the snapshot is current
  (G-RESTSTALE, `session/rest_stock.rs`).
- `prior_stocks[id]` is the group stock immediately before that toolpath
  (`record_pre_carve`, `simulate.rs:1556-1586`).
- For the first toolpath of setup N+1, that group stock is a fresh block
  (`simulate.rs:1728-1734`). No code seeds it from setup N. The consumer has
  no fallback to another snapshot (`session/compute.rs:2244-2256`).

**Finding F1 (claim against code).** `FEATURE_CATALOG.md:84` says "a setup's
first such operation reads the previous setup's finished stock
(2026-09-19)". `session/dependencies.rs:33-45` and commit `1d361563` say "the
simulator runs setups sequentially on ONE stock". The code does not do this
for the stock that generation, metrics and gates read. R2 changed the
dependency EDGES only. `git log -L1725,1736` on `simulate.rs` since
2026-09-18 shows no change to the group stock init. The wanaka "3D Rough 6"
evidence in `1d361563` is a generation that ran, not a check of the stock it
read. I found no test that asserts the carry (searched `tests/` for Bottom
setups with `prior_stocks`, engagement or `removed_volume` assertions).

The fresh stock per setup was a design, not a slip: commit `01236429`
(2026-04-08, "Port per-setup multi-stock simulation from viz to core") gives
"each group gets its own TriDexelStock". The commit records no reason against
a carry. The later R2 ruling (2026-09-19) states the current intent: a setup
starts from the stock the setup before it finished. This design follows R2,
treats the gap as a defect against the catalogue claim, and fixes it in P1
(section 3).

### 1.4 The flip

- `FaceUp::Bottom` maps a point to `(x, D - y, H - z)` (`compute/transform.rs:116`).
  The map is its own inverse (`:137`).
- The frame code maps points (`transform.rs:113`), toolpaths
  (`transform_toolpath`, `:570`), drill ops (`group_drill_op_to_global`,
  `simulate.rs:1040`) and meshes (`transform_stock_mesh_to_global`,
  `simulate.rs:1075`). Nothing maps a dexel stock from one frame to another.
- An identity setup (Top, 0°) has no transform; its group grid is in the
  WORLD frame (`SetupEvalContext::sim_local_stock_bbox`,
  `session/eval_context.rs:227`). A non-identity setup uses a zero-rooted
  local bbox.
- A per-setup stock always stamps `FromTop` (`simulate.rs:1736`). The global
  stock stamps in the setup's real direction (`SetupTransformInfo::cut_direction`,
  `transform.rs:543`). A lateral group never stamps the global stock
  (G-LATERALSCRUB).

### 1.5 The request and the caches

- Two request builders: `ProjectSession::run_simulation_memoized`
  (`session/compute/simulation.rs:97`, group push at `:244`) and the GUI
  controller (`rs_cam_viz/src/controller/events/simulation.rs:297`).
  `SimGroupEntry` (`simulate.rs:82`) has no field for an initial stock.
- S5 prefix memo: `GroupKey` (`compute/sim_prefix.rs:276`) and
  `hash_group_scalar` (`:446`). The latter destructures `SimGroupEntry`, so a
  new field does not compile until the key handles it.
- G-RESTRES: `SourceStock { cell_mm, after: Vec<SourceEntry> }`
  (`compute/source_stock.rs:133`), built by `snapshot_sources` (`:153`).
  `try_with_effects` drops a rest result whose `SourceStock` no longer matches
  (`session/rest_stock.rs:1-30`).
- `drop_simulation(cause)` is the one door that clears the simulation.
  `SimulationDropCause` has five variants and `COUNT = 5`
  (`session/mod.rs:1247`, `:1265`).
- Per-cell memory: `dexel_cell_bytes() = size_of::<DexelRay>() + 4`
  (`budget/estimate.rs:56`).

### 1.6 What the operator sees, and why

- Live scrub replays into the carried `global_stock`. Setup 1 removes the
  channel columns from the back. Setup 3 removes the relief from the front.
  Where the relief goes below a channel ceiling, the column is empty: an open
  hole through the front.
- A jump to a checkpoint (`load_checkpoint_for_move`,
  `rs_cam_viz/src/app/simulation.rs:20`) shows the LOCAL group mesh. For setup
  3 that is a fresh block with the relief and no channels. The two views give
  two answers for the same move.
- The setup-3 gates read the fresh local stock. They see solid oak in the
  channels: material present, wrong material. They also do not see the
  setup-2 cavity. On this part the relief does not reach the cavity (web
  check: least web 5.5, `TESTPIECE.md`), so the cavity error does not move a
  number here. It moves numbers on any part where it does.

### 1.7 The materials and the load model

- `Material` (`material/mod.rs:550`) has no epoxy or cast-resin variant.
- `Material::force_line()` is defined for solid wood (Curti 2021 × ρ) and MDF
  (Goli 2018). Every other material refuses; power and deflection then report
  `MaterialUnvalidated` (`material/CLAUDE.md`, `tool_load/CLAUDE.md`).
- `ToolpathLoadContext.material` is one material per toolpath
  (`tool_load/mod.rs:532`). A cut sample carries no material.
- The stamp kernel sums `removed_volume` per sample
  (`dexel_stock/stamping.rs:1266`, `:1621-1627`). It has no per-material split.

---

## 2. Design options

Common rule for all options: a fill is NOT a toolpath. It has no moves, no
tool and no G-code. It is a stock edit at a setup boundary.

### 2.0 Prerequisite for every option: carry the stock across the setup boundary (P0)

A fill adds material to a stock. Today the stock that the next setup's gates
read is a fresh block, so a fill there changes nothing. The fill has a
target only if setup N+1 starts from setup N's stock.

Change:

1. Add `start_stock` to the group loop: at the start of group N+1, build the
   group stock from group N's final stock, mapped into group N+1's frame.
   - Top/Bottom pair: reverse the row order (`y -> D - y`), map every segment
     `[a, b] -> [H - b, H - a]`, reverse the segment order in each ray, and
     set `conservative_top` from the new ray tops (raise allowed here; see
     2.4). This runs in place: no second grid.
   - Identity group (world frame) to non-identity group (zero-rooted frame):
     shift by the stock origin. The cell index does not change.
   - Z rotation 90° or 270°: transpose. This needs one transient grid.
   - Exact only when `D / cell` (and `W / cell` for a rotation) is an integer.
     Else the mirrored nodes miss the grid by
     `ceil(D / cell) * cell - D`; use the nearest node and record the offset
     on the result. Test piece: 120 / 0.2 = 600. nz-south: 354 / 0.25 = 1416.
   - A lateral group (Front/Back/Left/Right) keeps a fresh stock, as today
     (G-LATERALSCRUB: side grids do not boolean). The card says so.
2. Composite mesh (`finish_group`, `simulate.rs:1305`): with a carried stock,
   each group's final mesh contains the earlier groups' cuts. Append only the
   LAST Z-axis group's mesh, not every group's mesh. Else the earlier solids
   overlay the later ones.
3. `prior_stocks` for setup N+1's first toolpath becomes the carried stock.
   The catalogue claim F1 then holds. `SourceStock.after` already lists every
   earlier group's entries, so G-RESTRES needs no new rule for the carry.
4. The scrub: when the playhead crosses into group N+1, resume from
   `prior_stocks[first entry of N+1]` (local frame, mapped by the group
   transform), not from group N's last global checkpoint. With the carry, the
   global stock and the local stocks hold the same cuts. A later package can
   delete `global_stock` (memory: one full grid). Not in this design.

Behaviour change to expect: every multi-setup project changes its setup-2+
metrics, its adaptive feed modulation and so its emitted F-words (the
modulation post-pass reads the trace, `session/compute/simulation.rs`
F-036b note). Re-measure one two-sided project live after P0.

### 2.1 Option (a): a mesh fill per setup

A `StockFill` item on a setup holds a closed mesh (a model id) and a frame.
The simulation unions the mesh volume into the stock before the setup's
first toolpath.

| Area | Change |
|---|---|
| Core model | `SetupData` (`session/mod.rs:848`) gains `fills: Vec<StockFill>`; `StockFill { id, name, enabled, geometry: FillGeometry::Mesh { model_id }, material }`. The mesh is in the setup frame (identity setup: world). |
| Kernel | New: per cell, cast a vertical ray through the mesh, collect ALL hits, sort, pair them even-odd into intervals. No such cast exists (`rg "from_mesh|all_hits|ray_hits"` finds only `surface/slope.rs`). `SpatialIndex::build_auto` gives the triangle candidates. New `ray_union_interval` in `stock/dexel.rs`. |
| Command | `AddStockFill`, `RemoveStockFill`, `ReplaceStockFill` rows, the twins of `AddFixture` (`session/command.rs:594-630`). The mutation drops `FromRemainingStock` results in this and later setups and calls `drop_simulation(SimulationDropCause::StockFill)` (`COUNT` 5 -> 6). |
| Simulation | `SimGroupEntry` gains `fills_before: Vec<Arc<ResolvedFill>>` (resolved per-cell intervals). Applied to the group stock before entry 0, and to `global_stock` through `local_to_global`. |
| Caches | `hash_group_scalar` hashes the fill (mesh digest + frame). `SourceEntry` gains a fill variant so G-RESTRES sees a fill edit. |
| Project file | `[[setups.fills]]` with `model_id`. Additive, `serde(default)`. |
| Surfaces | GUI: a "Fills" list in the setup panel, beside fixtures. MCP: `add_stock_fill`, `remove_stock_fill`. CLI: project file only. |
| Tests | Ray-cast intervals on a closed box and on a box with a hole; union keeps oak where the mesh overlaps oak; non-watertight mesh refuses by name. |
| Cost | Largest of the three. New ray-cast kernel, watertight check, frame import. |

Good: it is the general "add a model" the operator asked for, and it takes
`epoxy.stl` as the monorepo writes it. Bad:

- The mesh must agree with the cut. A pour fills what was CUT, including an
  overcut. A mesh fills what was DESIGNED. Where the two differ, the mesh is
  wrong and the pour is right.
- `epoxy.stl` is in the block frame. It needs the `derive.py` frame map
  (`x = 228 - X`, `y = Y + 313`, `z = T1 - Z`) before import.
- A non-watertight mesh gives wrong intervals.

### 2.2 Option (b): fill the cut volume inside outlines up to a level (no mesh)

A `StockFill` item on setup N says: "after setup N, fill every empty space
open to the top, inside these outlines, up to setup-local Z = h". Gravity is
setup N's −Z. This is what a pour does.

Rule, per cell whose centre is inside the outline union, in setup N's frame,
after setup N's last toolpath:

1. Let `t` = the top of the highest segment, or `bbox.min.z` for an empty
   ray (a through cut; the spoilboard or the tape holds the epoxy).
2. If `t < h`, union `[t, min(h, bbox.max.z)]` into the ray, as fill
   material.
3. Do not fill a gap under material. Gravity cannot reach it in that column.
   (A side channel under a bridge is not a 3-axis case.)
4. Raise `conservative_top` of the cell to `min(h, bbox.max.z)`.

| Area | Change |
|---|---|
| Core model | `StockFill { id, name, enabled, geometry: FillGeometry::OutlineToLevel { model_ids: Vec<ModelId>, level_z: f64 }, material: FillMaterial }`. The outline resolver is the existing "Model Outline" boundary source (G-MODELOUTLINE, `FEATURE_CATALOG.md:89`), transformed to the setup frame like a 2D drawing (`transform_drawing_polygons_to_setup`, `session/mod.rs:2012`). |
| Kernel | `ray_union_interval` in `stock/dexel.rs`, and a cell scan. Reuse `for_each_covered_cell` (`dexel_stock/stamping.rs:661`, STK-01) or extend it; do not write a second cell loop. Point-in-polygon per cell centre. |
| Command | As (a): three rows, `SimulationDropCause::StockFill`, invalidation to fixpoint through `invalidate_output_dependents_of_set`. A fill edit stales the `FromRemainingStock` ops of setup N+1 onward, not setup N. |
| Simulation | `SimGroupEntry` gains `fills_after: Vec<Arc<ResolvedFill>>` (polygons + level, setup-N frame). Applied after the group's last entry, BEFORE `finish_group` and before the P0 carry. Also applied to `global_stock` through `local_to_global` (a Bottom setup fills from below in the global frame). The result reports the added volume per fill. |
| Caches | `hash_group_scalar` for group N hashes `fills_after` (polygon digest, level, material). `SourceEntry` gains a `Fill { id, digest }` variant, pushed into `carved` after group N in `snapshot_sources`. |
| Project file | `[[setups.fills]]`: `name`, `model_ids`, `level_z`, `material`. Additive under `format_version = 3`. |
| Surfaces | GUI: a "Fills after this setup" list in the setup panel. MCP: `add_stock_fill { setup_id, name, model_ids, level_z, material }`, `remove_stock_fill`; `list_setups` and `inspect_stock` show fills and the added volume. CLI: project file; `rs_cam_cli project` prints one line per fill with the added volume. |
| Tests | See section 3. |
| Cost | Small to medium. One ray primitive, one cell scan, one model field, three command rows, one wire tool pair. |

Good: no mesh. The test piece already holds the outlines (models 3..6,
`back_channel_z17.00.dxf` .. `z20.00.dxf`). The fill follows the cut, so an
overcut is filled as the real pour fills it. The volume is a free check: the
process sheet gives the milled volume per group, 16.5 ml (`TESTPIECE.md`,
"Step 2" table); the sim gives the added volume below the back face.

Bad: it cannot add a solid that was never cut (an inlay block, a glued
riser). That is (a).

### 2.3 Option (c): a material id per dexel segment

This is an attribute, not a geometry source. (a) and (b) both need it for the
two outcomes the operator asked for: a different colour, and a load that
knows it cuts epoxy.

| Area | Change |
|---|---|
| Data | `DexelSegment { enter: f32, exit: f32, material: u8 }`. `0` = the stock material; `k` = fill slot `k`. Three production construction sites (`stock/dexel.rs:123`, `:182`, `:384`). A split copies the tag. |
| Ray rules | Subtract and blend change `enter`/`exit` only, so the tag stays. A union never merges two touching segments of different material. No merge code exists today (`rg "merge|coalesce" stock/dexel.rs` is empty), so the rule is a new invariant, not a change. |
| Stamp kernel | The metric route sums removed volume per tag: `removed_volume` (`stamping.rs:1266`) becomes stock + fill parts; `band_batch.rs` keeps one dispatcher. The sample gains `fill_removed_mm3: f32` (4 bytes per sample; the trace is G-SIMMEM-budgeted, so add it to `trace_sample_step_mm`'s estimate). |
| Gates | P2 rule: a sample whose fill part is above a set fraction joins the FILL population. The stock gates (chipload, power, deflection) read the stock population only. The fill population reports by name ("cuts epoxy: no force line, not judged") until a sourced line exists. |
| Mesh colour | `wood_color_at_z` gets a sibling for a fill tag. The MC top-face pass reads the tag of the top segment at each corner. |
| Memory | See section 4.3: no change per plain cell; +4 bytes per heap segment. |
| Physics | A cured-epoxy force line needs a printed source in the repo. Operator item. No number from memory. |
| Command | None of its own. The material rides on the fill's `material` field (the (a)/(b) commands). A stock with no fill keeps tag `0` everywhere. |
| Project file | `material` key on `[[setups.fills]]` (a label in P1; a `Material` in P2). No new top-level key. |
| Surfaces | GUI: the fill colour in the viewport and a "cuts fill material" line on the toolpath card. MCP/CLI: `fill_removed_mm3` in the cut trace and the per-toolpath fill-sample count. |
| Tests | Tag survives subtract, blend and interval split; a union never merges oak and fill; `size_of::<DexelRay>() == 24`; samples over a fill carry `fill_removed_mm3 > 0`; the stock gate population excludes them. |
| Cost | Medium. Small in `dexel.rs`; the sample and gate split is the larger part. |

### 2.4 Invariants that change in any option

- `conservative_top` is "lowered only" today (`stock/dexel.rs:499`). A fill
  RAISES material, so the fill must raise the bound. Else a rapid over the
  overfill reads clear. New contract: "a stamp lowers it; a fill or a carry
  sets it".
- A fill above `stock_bbox.max.z` has no grid to live in, and the mesh
  extraction clips to the bbox (`stock/dexel_mesh.rs:148`). See 4.1.

---

## 3. Recommendation and phased plan

**Recommend (b) + (c), on top of P0 (the carry).** (b) models the pour as the
pour works, needs no mesh, and the test piece already holds its outlines.
(c) is what makes the result visible and what makes the load honest. Keep
`FillGeometry` an enum, so (a) lands later as `FillGeometry::Mesh` with no
change to the command rows, the file key or the caches.

### P1: the smallest useful slice (carry + outline fill + tag for colour)

The operator's acceptance: the front holes show as epoxy, and the front cut's
load sees material there. P1 delivers both in a form that does not lie.

1. P0 carry (section 2.0), Top/Bottom and identity/rotation. Lateral stays
   fresh.
2. `StockFill` with `FillGeometry::OutlineToLevel` only, on `SetupData`,
   applied after the setup (section 2.2).
3. `DexelSegment.material: u8` and the colour. The setup-3 scrub and
   checkpoint meshes show the channels in a fill colour.
4. The cut trace: the metric route records `fill_removed_mm3` per sample. The
   stock gates exclude samples that are mostly fill; the toolpath card reports
   "N samples cut fill material (epoxy): not judged, no force line". Engagement
   and air-cut read total material, so a channel is not air.
5. Commands: `AddStockFill`, `RemoveStockFill`, `ReplaceStockFill`;
   `SimulationDropCause::StockFill`.
6. File: `[[setups.fills]]`.
7. Surfaces: GUI setup-panel list; MCP `add_stock_fill` / `remove_stock_fill`
   and the fill block in `list_setups`; CLI reads the file and prints the
   added volume.
8. Docs: rewrite `FEATURE_CATALOG.md:84` so the cross-setup claim matches the
   code after the carry, and add a catalogue line for the fill.
9. Overfill: clip `level_z` to the stock top and warn ("fill level 26.5 above
   stock top 25.5: 1.0 mm not modelled"). See 4.1.

Test piece set-up after P1: one fill on "1 Back channels", `model_ids =
[3, 4, 5, 6]`, `level_z = 26.5` (clipped to 25.5 with the warning), material
label "epoxy (cast)". `derive.py` writes it.

P1 sentries (new, each its own `--test`):

- `stock_carry_two_sided_g_setupcarry`: Top pocket, then a Bottom setup whose
  first op is `FromRemainingStock`. Its `prior_stocks` holds the pocket at
  `(x, D - y)` and `H - z`. A flip twice is bit-identical when `D / cell` is
  an integer. This is the sentry F1 never had.
- `stock_fill_fills_the_cut_g_stockfill`: pocket to depth d inside an outline,
  fill to the top, flip, face from below past `H - d`. Assert:
  - no empty column inside the outline in the setup-2 group stock;
  - every cell inside the outline carries the fill tag above the old floor;
  - the cut samples over the outline have `removed_volume > 0` and
    `fill_removed_mm3 > 0`;
  - the added volume equals the pocket volume within one cell ring of the
    outline perimeter.
- `stock_fill_keys_the_memo`: a fill level edit misses the S5 prefix at the
  group after the fill and hits before it (extend `sim_prefix_memo_s5`).
- Extend `command_registry_completeness`, `mutation_paths_invalidate_alike_p0`,
  `rest_stock_identity_g_restres` (a fill edit drops a later rest result),
  and a project round-trip (save, load, equal fills).

Existing sentries that must stay green: `band_stamping_determinism_s3`,
`playback_band_dispatch_s6`, `swept_stamping_s1`, `sub_cell_stamping_fa`,
`dexel_stock_z_frame_f024`, `drill_flip_removal_g_drillflip`,
`lateral_scrub_playback_stock_g_lateralscrub`, `sim_prefix_memo_s5`,
`sim_trace_sample_rate_g_simmem`, `sim_peak_memory_g_simmem`,
`--lib -- the_six_cell_loops`.

P1 live check (GUI on HEAD, rebuilt first): load the test piece with the
fill, run the simulation, scrub setup 3 to its end and jump to its last
checkpoint. Both views show the channels in the fill colour on the front.
Compare the reported fill volume with 16.5 + 16.5 = 33.0 ml (both groups,
`TESTPIECE.md`). The difference must be explained by the cell (perimeter ×
cell / 2 × depth is the order of the bound).

### P2: the fill material in the load model

1. A `FillMaterial` that names a `Material`. Add a cast-epoxy material only
   with a printed source for its force line in the repo (an operator task: no
   number from memory, `material/CLAUDE.md`).
2. The gates evaluate the fill population against its own line, so the
   front-cut card reports one verdict for oak and one for epoxy.
3. The feed modulation post-pass reads the per-sample material, so a move in
   epoxy is modulated by the epoxy band, not the oak band.

### P3: mesh fill and mesh remove (option a)

`FillGeometry::Mesh { model_id }` with the all-hits ray cast and a watertight
refusal. A `StockEdit::Remove` twin answers "remove models in setups" (for
example a part drilled off-machine). This takes `nz-south/epoxy.stl` once the
import applies the `derive.py` frame map.

### P4 (optional): delete the parallel global stock

After P0 the local stocks carry the same cuts. The global playback stock is
then one full grid of duplicate memory. Delete it, and make the scrub read
the local stocks plus the group transforms.

---

## 4. Risks

### 4.1 Can the dexel model hold material added above the cut?

- Inside the bbox: yes. A ray is a free list of f32 intervals. A fill on top
  of a single-segment ray adds a second segment (or extends the first, if
  (c) is not in).
- Above `stock_bbox.max.z`: no. The grid has no Z bound per ray, but the
  bbox drives the mesh range (`dexel_mesh.rs:148`), the colour ramp, the
  flip `H - z` (it goes negative) and `conservative_top`'s start value. The
  test piece pours 1 mm over the back face, and `testpiece.toml` models the
  stock at T1 = 25.5 with the raw top 27.0 in air (`rivmap_block_job.md`,
  "Frame and datum"). Two choices:
  1. Clip the fill at the stock top and warn (P1 default). The setup-2
     overfill face then cuts air in the sim. Its load is low by that pass.
  2. Model the raw blank (`z = 27.0`) in `derive.py`. The overfill then fits
     in the bbox, and the setup-1 face pass removes real material. This is a
     project choice, not a core change.
- Gaps under material: rule 3 of 2.2 does not fill them. A 3-axis pour into
  cuts from the top has none. A pour after a flipped setup that cut from the
  other side can have them. The fill card states the rule.
- Cell resolution: the outline test is per cell centre. The fill edge is
  within half a cell of the outline. The channels here are >= 3.2 mm wide
  at a 0.2 mm cell.
- The carry is exact only for `D / cell` integer (2.0). Else a half-cell
  shift. The result records the offset.

### 4.2 Multi-material physics

- No epoxy force line exists, and the rules forbid a typed one. P1 therefore
  does not JUDGE the epoxy cut. It counts it, names it and keeps it out of the
  oak population. A gate that reads "Within" with the epoxy samples removed
  can be vacuous: check the population count (`tool_load/CLAUDE.md`).
- A sample that cuts both materials needs a split rule. P1 uses the dominant
  part. P2 can weight by volume.
- The feed modulation (default on) reads the trace. In P1 the epoxy samples
  leave the oak band alone; the modulator must not raise the feed over epoxy
  because "no band" reads as "no limit". Hold the programmed feed there.
- Epoxy behaves unlike wood at the edge (chipping, heat, clogging). Nothing
  in rs_cam models that. The process sheet already says "listen for chatter".

### 4.3 Memory

- `size_of::<DexelRay>()`: smallvec 1.15.1 with `union`
  (`Cargo.toml:31`) is `capacity: usize` + a union of the inline array and
  the heap pair `(ptr, usize)` (`smallvec-1.15.1/src/lib.rs:622`, `:772`).
  On a 64-bit target: 8 + max(8, 16) = 24 bytes today.
- With `material: u8` the segment is 12 bytes (align 4). The inline array is
  12 bytes, still under the 16-byte heap pair: `DexelRay` stays 24 bytes, and
  `dexel_cell_bytes()` stays 28. Confirm with a `size_of` assert in the P1
  sentry.
- A filled cell has two segments (oak + epoxy), so it spills to the heap:
  2 × 12 = 24 bytes plus the allocator overhead, per filled cell. The cost
  scales with the fill footprint, not with the grid. Add the term to the
  budget estimator as `fill_cells × heap_ray_bytes`.
- A spilled ray without a tag costs 8 bytes per segment; with the tag, 12.
  Rays spill today only at undercuts and through-cuts with islands.
- The carry: a Top/Bottom remap runs in place. A 90° rotation needs one
  transient grid (`rows × cols × 28` bytes at the peak).
- The sample field `fill_removed_mm3` adds 4 bytes per cut sample. The trace
  step already scales with the budget (G-SIMMEM); include the field in the
  estimate.

### 4.4 Other risks

- P0 changes the numbers of every multi-setup project (2.0). Re-measure one
  live. A merge without that check repeats the "never trust a stale number"
  lesson.
- An old build reads a new file and ignores `[[setups.fills]]` without a
  word (no `deny_unknown_fields`). If that is not acceptable, bump
  `SUPPORTED_FORMAT_VERSION` (`session/project_file.rs:40`) to 4. The
  operator ruling of 2026-09-16 permits the break.
- Two request builders (core and GUI). Both must pass `fills_after`. Put the
  resolution in one shared function in `simulate.rs`, as the folder file
  requires for a shared decision (`compute/CLAUDE.md`).
- The fill is a stock edit that no toolpath card owns. The plan view must show
  it, or a stale rest op after a fill edit reads WAIT with no visible cause.
