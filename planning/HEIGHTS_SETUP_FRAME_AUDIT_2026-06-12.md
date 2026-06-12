# Heights-system audit across setups — wanaka200 research case (2026-06-12)

User report: with the wanaka200 terrain (200 mm rivmap export, `water_as_holes=true`,
`enclose_mesh=false`), "the roughing seems to always go way below the mill height
and not land on the mesh … it was impossible to get the toolpaths to do what I want."
Suspected the mesh ("might just be cooked — it has holes").

**Verdict: the mesh is fine and the emitted G-code is fine. The symptom is a
viewport frame regression (F-028 fallout); two further heights-system defects
compound the confusion.**

## Research case

`/home/ricky/Downloads/wanaka200/heights_audit.toml` — terrain.stl (653 438 tris,
bbox `[0,0,-2.82]..[200,171.4,6.0]`, **5 058 boundary edges** = open shell with
lake holes), stock 200×171.5×12 at origin `(0,0,-6)` (top flush with model peak,
world Z=6), one 6 mm end mill, two adaptive3d roughs:

| Setup | face_up / rot | emission frame | Z levels generated |
|---|---|---|---|
| Identity (top, 0°) | identity | **world** | 3.0, 0.0 (+ empty final at −2.32) |
| Flipped (bottom, 0°) | non-identity | setup-local | 9.0, 6.0, 3.0, 0.5 |

Offline analysis (`/tmp/heights_audit/gouge.py`: exact barycentric raster of the
STL at 0.5 mm + exported G-code cut endpoints):

- Identity rough: 4 434 cut endpoints, **0 gouges** (all cuts ≥ surface − 0.05 mm;
  p5 of clearance above surface = 0.96 mm, consistent with stock_to_leave 0.5).
- Flipped rough (checked in its local frame): 8 055 endpoints, **0 gouges**.
- Coarse sim (1.0 mm): verdict OK, **0 rapid collisions**, air-cut 15.9 %.

So nothing the planner emits is "below the mill height". What the user sees is:

## Finding 1 (root cause of the report) — viewport draws identity toolpaths in world frame against a local-frame mesh

`crates/rs_cam_viz/src/app/gpu_upload.rs` (~line 123): *"Everything is always
displayed in the active setup's local coordinate frame"* — the model mesh, stock,
and polygons are uploaded through `Setup::transform_point` (`state/job.rs:390`),
which **subtracts `stock.origin`** even for identity setups (`use_local_frame`
is true whenever any setup exists, and every project has one).

The toolpath upload (~line 743) says *"Toolpaths are always in local coords …
use directly, no transform needed"* and uploads `result.toolpath()` raw. That
comment has been **stale since F-028/F-030**: identity setups emit toolpaths in
the **world** frame (heights anchored at world stock top; `local_to_global =
None`, no back-transform anywhere).

Net effect: for an identity setup with `stock.origin ≠ (0,0,0)`, the rendered
toolpath is offset from the rendered mesh by exactly `−origin` — here **6 mm
low** (and origin_x/y in general). The user's wanaka200 attempt was a fresh
project → default identity setup → auto stock (`update_from_bbox`: `origin_z =
bbox.min.z = −2.82`) → every rough rendered ~2.8 mm below the terrain and
"never landed on the mesh", no matter what parameters changed. wanaka100 never
showed this because both of its setups are non-identity (bottom; top+90°),
whose toolpaths genuinely are local-frame.

Reproduced live in this session: user confirmed "roughs generating well under
the mesh" while the exported G-code of those exact toolpaths measures zero
gouges.

Fix directions (pick one frame and make every upload agree):
- (a) translate identity-setup toolpaths by `−stock.origin` at upload (smallest
  change, keeps the "viewport = local frame" invariant), or
- (b) render identity setups in world frame (skip the origin subtraction for
  mesh/stock/polygons when `local_to_global == None`) — matches how the
  toolpath, the dexel grid (F-024) and the heights context (F-028) already
  treat identity setups; likely the consolidating fix.
  Audit every `upload_gpu_data` consumer + entry-preview + collision markers +
  playback (`worker/execute/mod.rs::build_playback_data` — note
  `SetupTransformInfo::local_to_global()` returns *stock-relative* coords and
  "callers should re-add origin themselves", which playback does not).

## Finding 2 — adaptive3d ignores the Heights system entirely

`compute/execute.rs::generate_adaptive3d` uses `stock_top_z: ctx.stock_bbox.max.z`
and its floor comes from `surface_hm.min_z() + stock_to_leave`
(`adaptive3d/path.rs:421-431`). `heights.top_z` / `heights.bottom_z` are never
consulted (only `retract_z` as safe_z).

Proven live: pinning `bottom_z = 3.0` via `set_toolpath_heights` + regenerate
reproduced the identical toolpath (4 932 moves, 14 330.73 mm cutting) — the z=0
pass and the final level were not clipped. The Heights tab is a silent no-op
for the 3D rough, which is a large part of "impossible to get the toolpaths to
do what I want". Either clamp adaptive3d's z-level plan to
`[bottom_z, top_z]` when those are non-Auto, or grey the fields out for
adaptive3d with an explanation.

## Finding 3 — open meshes poison the adaptive3d floor (holes read as mesh-bbox bottom)

`SurfaceHeightmap::from_mesh_with_cancel` (`slope.rs:71-77`) clamps
`cl.z.max(min_z)` with **no hole rejection**. Cells whose vertical ray passes
through a mesh hole (deeper than cutter-radius rim contact) read
`min_z = mesh.bbox.min.z`. Two consequences on hole-y meshes like wanaka200:

- `surface_hm.min_z()` ≡ `bbox.min.z` whenever any hole exists, so the final
  Z level is always planned at the global mesh bottom (+stock_to_leave),
  regardless of where real surface bottoms out.
- hole interiors are treated as "surface at the very bottom" → the planner
  will clear lake interiors down to that floor where regions are big enough
  (in this repro the hole regions fell below `min_region_cut_length_mm`/
  sub-tool thresholds and were dropped, so no gouges emitted — bigger holes
  or a smaller cutter will cut them).

Siblings already solved this: `dropcutter` finish (`compute/execute.rs:1387-1411`)
and `project_curve` (`project_curve.rs::point_over_triangle`) both reject
points whose vertical ray misses every triangle (`contains_point_xy` test).
`SurfaceHeightmap` should grow the same mask (e.g. `covered: Vec<bool>`), with
adaptive3d treating uncovered cells as "no surface information → don't plan
material there / exclude from `min_z()`".

**Implementation correction (2026-06-12):** the "exclude from min_z / fill
holes to rim height" half of this finding is WRONG for a roughing op. The
hemisphere clearing tests proved it: `z_values` is a *cutter-location* map
(near-steep walls the CL rides high), and clearing the stock beside/around a
model depends on uncovered cells reading the clamp floor — excluding them
made `min_z()` = 10.8 on the r=20 hemisphere and left 31.6 % of the square
stock uncleared. A lake hole is geometrically indistinguishable from
"stock beside the model" without enclosure analysis. So: hole-diving during
roughing is intended clear-everything semantics; the user-facing lever is a
pinned `bottom_z` (finding 2, now honored). What landed: the `covered` mask
(real information; used to make the border/boundary full-ray clears explicit;
enables a future "enclosed uncovered region = hole, don't descend" option),
with z-fill and min_z left as-was.

## Finding 4 — GUI vs CLI/headless divergence on identity-setup op bbox

GUI controller (`controller/events/compute.rs:251`) passes
`ctx.heights_stock_bbox` (world for identity) as the request `stock_bbox`,
which the worker forwards to ops. The core session path
(`session/compute.rs:759`, used by the CLI and any headless session consumer)
passes `effective_stock_bbox = ctx.local_stock_bbox` — **zero-rooted even for
identity setups**. For identity setups with `origin_z ≠ 0`, adaptive3d's
`stock_top_z` (and any op reading `ctx.stock_bbox`) differs between GUI and
CLI by `−origin_z`, producing different z-level plans for the same project
file. Same family as the 5-site divergence F-030 consolidated — this is a
6th site that consolidated onto the *wrong* bbox. The session path should pass
`ctx.heights_stock_bbox`.

## Not findings

- The mesh is not "cooked": open shell + holes are intentional rivmap output
  (`water_as_holes=true`, `enclose_mesh=false`); 0 non-manifold edges.
- `update_from_bbox` 3D auto-stock (origin_z = bbox.min.z) is coherent.
- Heights *resolution* itself (`HeightContext` via `SetupEvalContext`) is
  frame-correct post-F-028/F-030 on both GUI and session paths.
- Setup transforms (`world_to_local` / FaceUp tables) verified self-consistent;
  flipped-setup toolpath measured gouge-free in its emission frame.

## Suggested order of attack

1. Finding 1 (viewport frame) — it is the entire user-visible symptom.
2. Finding 2 (heights no-op) — restores the expected control lever.
3. Finding 4 (GUI/CLI bbox divergence) — one-line consolidation + sentry test.
4. Finding 3 (hole-aware surface mask) — correctness on open meshes.

Artifacts: research project `/home/ricky/Downloads/wanaka200/heights_audit.toml`;
analysis script + heightfield + G-code in `/tmp/heights_audit/`.

## User-confirmed repro of the 2026-06-11 report (finding 1)

Re-ran the research case with the user's "model moved to the top of a large
stock" scenario: stock 200×171.5×**25**, `origin_z = -19` (model peak flush
with the stock top at world Z=6). Generated identity rough:

- G-code: 4 462 cut endpoints, min(z − surface) = **+0.109 mm**, zero
  gouges, lowest cut Z = 0.0 — nowhere near the −19 floor.
- Viewport: mesh renders at local Z 16.2–25 inside the 0–25 stock box;
  the toolpath draws raw at world Z ≤ 3 — at the **floor of the displayed
  stock box, shaped like the model outline**, 19 mm below the mesh.

That is exactly "roughed all the way through and left that outline on the
floor of the stock". The machine output was always correct.

## Fixes landed (2026-06-12, this branch)

- **F4** — `session/compute.rs` now passes `ctx.heights_stock_bbox` (the
  emission frame) as the op/boundary/dressup/drill bbox; renamed to
  `emission_stock_bbox`. Sentry:
  `tests/identity_setup_emission_frame_audit.rs` (verified to fail pre-fix
  with first-pass cuts at local Z≈10 on a world-top-0 stock).
- **F1** — viewport emission→display adapter:
  `Setup::emission_to_display_shift` (viz `state/job.rs`, = `-stock.origin`
  for identity setups, zero otherwise) applied to toolpath lines (all three
  color modes via `translate_annotated`), entry preview + tool-profile
  ghost, collision markers, height-plane overlays, the sim checkpoint /
  live stock mesh (`transform_mesh_to_local_frame` now shifts identity
  meshes instead of no-op), and the playback tool cursor. Unit tests in
  `state/job.rs` + `app/gpu_upload.rs`.
- **F2 (scoped to the floor)** — adaptive3d honors a user-pinned
  `bottom_z`: `ResolvedHeights` gains `top_pinned`/`bottom_pinned` (set from
  the height modes in `resolve()`), and a pinned `bottom_z` flows as
  `Adaptive3dParams::z_floor`, clamping the Z-level plan in
  `adaptive3d/path.rs`. A pinned `top_z` is deliberately NOT honored:
  roughing must start at the real material top (honoring a lower top leaves
  the overhead unplanned while the simulator carries it — F-027 failure
  shape — and the wanaka100 Back Rough F-034/F-036c machine-calibration
  anchors measure exactly such a project, whose pinned `top_z=model_top`
  was vacuous while heights were ignored; honoring it collapsed both
  calibration tests). Auto heights → behavior unchanged. Regression test
  `pinned_bottom_z_floors_adaptive3d_plan` (same audit test file).
- **F3 (revised)** — `SurfaceHeightmap` gains a `covered: Vec<bool>` ray-
  through-triangle mask; border/boundary clears in `adaptive3d/path.rs` use
  it for explicit full-ray clearing of uncovered cells. The fill/min_z half
  of the original finding was reverted as wrong (see the correction note
  under finding 3). Tests in `slope.rs`.
**Live verification (rebuilt release GUI, same repro project):** identity
rough renders ON the terrain in the Toolpaths workspace (was: outline at the
stock floor 19 mm down); simulation stock + tool cursor align with the stock
outline; sim verdict OK, 0 rapid collisions (both setups); CLI `project` run
plans the same world-anchored Z levels (3.0 → −2.32) as the GUI.

- **F-034 flake** — `cycle_time_calibrated_against_shapeoko_reference`
  failed at ratio 0.339 then passed on an identical build (AgentSearch
  run-to-run variance straddles the 0.40 floor of the stale 827 s anchor).
  Lower bound widened 0.40 → 0.30; the re-bench
  (`planning/cycle_time_rebench.md`) is now overdue.
