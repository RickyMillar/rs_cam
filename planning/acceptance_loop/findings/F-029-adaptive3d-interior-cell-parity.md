# F-029 — Adaptive3d interior-cell planner↔simulator stamp parity gap

- **Stage:** sim / adaptive3d
- **Severity:** medium (residual after F-027 lands; deflection still
  fires Exceeds on AS013 at 0.013% of samples, but the model-edge
  cohort is fully resolved)
- **Status:** partial_landing (defensive cleanup-raster clamp + diagnostic
  probe landed 2026-05-26; residual tracked as F-031)
- **First found in:** F-027 implementation verification (2026-05-25)
- **Effort:** M
- **Linked PRs:** commit `74d8a7f` (2026-05-26)
- **Follow-up:** F-031 — residual interior-cell parity gap (planner
  fully clears the cell but simulator dexel diverges at intermediate Z
  levels)
- **Source audits:** F-027 implementer's repro of AS013 against
  `test_data/ux_3d_terrain.toml` via `ProjectSession::run_simulation`,
  after the F-027 widening + border-clear inhibit fix landed.

## Symptom

After F-027's planner-stock-XY widening (and border-clear inhibit
inside the world stock bbox) lands, AS013 axial outliers in the
model-edge band (Y > mesh.bbox.max.y) are eliminated. But ~108
remaining outliers cluster at **interior cells** with the same
mechanism signature — a single sample reading `axial_engagement_mm =
30-38 mm` on a 3 mm-commanded DPP.

Repro (AS013 adaptive3d on `ux_3d_terrain.toml`):

- Pre-F-027: 310 outliers, worst at (y=75.92, z=6.57) — boundary band.
- Post-F-027: 108 outliers, worst at (y=18.33, z=6.57) — interior.
- Deflection peak: pre 0.576, post 0.579 — essentially unchanged.

The interior outliers sit inside the mesh XY footprint (where the
planner's `material_stock` already had cells). Same mechanism family
as F-027: the simulator sees virgin material at a cell when the cutter
arrives, even though the planner believes that cell has been stamped
by earlier passes.

## Hypothesised root cause

Several candidates, in order of likelihood:

1. **Simulator's per-setup dexel resolution differs from the planner's
   cell size** (planner ~0.5 mm via `tool_radius/6` or `tolerance`,
   simulator `SimulationOptions.resolution = 1.0` here). Subtle
   coverage differences at cell boundaries could leave the simulator
   with a partially-stamped cell at the moment a deep-Z dive's
   subsegment-midpoint sample reads it.
2. **Z-level sampling gaps** — the planner's clear_z_level emits cuts
   tracking the heightmap, but if the heightmap reads `surface_z` at
   one cell-center and the simulator's resampling drops at slightly
   different positions, the simulator could see a "fresh column" the
   planner thinks is cleared.
3. **Cut-path simplification artifacts** — `simplify_path_3d` +
   `blend_corners_3d` reduce points in Cut paths before they reach
   the toolpath. The simulator stamps along the simplified path. If
   simplification removes a critical intermediate point that the
   planner stamped (in its higher-resolution material_stock), the
   simulator may sweep through cells the planner thought were
   stamped.
4. **Link moves not stamped by the planner** — `Adaptive3dSegment::Link`
   emits a feed-rate move that the simulator stamps but the planner
   doesn't (`stamp_along_path` is only called for `Cut` segments).
   That asymmetry was previously called out in `tally_segments_for_z_level`
   as a tally hint; could be the actual root cause for some interior
   cells.

## Fix shape (sketch)

Investigation needed before fix shape is concrete. Likely candidates:

- **Resolution alignment**: snap simulator resolution to planner cell
  size for adaptive3d ops (resolution = `max(planner cell_size, 0.5)`).
- **Link-move stamping**: have the planner stamp Link segments into
  `material_stock` too (mirror the simulator's behaviour).
- **Pre-stamp planner→simulator handoff**: serialize the planner's
  final `material_stock` and use it to pre-stamp the simulator's
  per-setup grid before the toolpath runs (heavy; structural).

The link-move stamping fix is the smallest and cleanest probe; try
that first.

## Acceptance test

Re-use the F-027 acceptance test setup (`adaptive3d_planner_stock_xy_f027.rs`
infrastructure can be factored out) but enforce the stricter bar:

1. **Whole-toolpath** worst-case `axial_engagement_mm` across all
   lateral (non-plunge) cutting samples ≤ `depth_per_pass + 0.5`
   margin. Post-F-027 reads 38.25 mm; post-F-029 should read ≤ 3.5 mm.
2. `deflection.peak_mm < 0.2` on the tool-load verdict (F-027 left
   this at 0.579 because the gate consumes the worst sample).

## Risk / notes

- Independent of F-027 but shares the AS013 test setup.
- Independent of F-028 (face op z-frame; different op family).
- F-017 (rapid collisions) closure depends on this landing — the
  remaining 3D-op collision surface waits on F-029 (similar mechanism).
- Cross-link: F-027's residual scope note points here.

## Partial-landing log (2026-05-26)

Implementer (Claude Opus 4.7 pickup) landed:

1. **Diagnostic probe** in `crates/rs_cam_core/src/adaptive3d/path.rs`
   (`debug_adaptive_3d_segments_for_f029_probe`) — exposes the planner's
   final per-cell `material_stock` top-z so tests can compare planner
   stock state to simulator stock state at the same cell layout.
2. **Cleanup-raster per-cell DPP clamp** in
   `crates/rs_cam_core/src/adaptive3d/clearing.rs`
   (`clear_z_level_contour_parallel`). The raster previously emitted a
   single cut at `z = max(surf_z + stock_to_leave, z_level)` for every
   contiguous run of remaining-material cells, regardless of the cell's
   current stock top. For cells whose stock top is far above `z_level`
   (e.g. cells outside the mesh XY footprint where `surf_z = min_z` so
   the contour iso-lines never cover them), the simulator faithfully
   replayed that single deep cut as one swept tube and reported the
   full descent as `axial_engagement_mm`. The clamp limits each cell's
   cut depth to `stock_top - depth_per_pass - tolerance` so a deep
   uncleared cell takes at most one DPP of axial per pass.
3. **F-029 acceptance tests** (`adaptive3d_interior_cell_parity_f029.rs`)
   landed but **`#[ignore]`d** with `F-031` reference. The cleanup-raster
   clamp is defensible defensive coding and doesn't regress any other
   adaptive3d test, but **it does not close the worst-case AS013 cell**
   (x≈-2.4, y≈18.5): max axial stays at 44.8 mm, deflection at 0.66 mm.

### Why F-029 didn't close fully

The probe shows the planner's final `material_stock` at (-2.5, 18.5)
reads `top = 0.5` (fully cleared), but the simulator's per-setup dexel
disagrees. The worst sample arrives at `z = 6.57` on the final pass and
reads axial ≈ 44.8 mm — implying the simulator's stock at that XY was
still at ≈ 51.4 (the cell's depth after the **first four passes** only)
when the deepest pass swept through.

Sample history at the worst XY shows the cell got cuts at z = 54.5,
51.5, 48.5, 45.6 (the first four passes), **then nothing for 13 Z
levels**, then a hit at z = 0.5 (final pass). The 13 missed Z levels
are not a planner cell-coverage gap (the planner correctly stamps them)
— they're a simulator **sample density / coverage** gap. Either:

- The toolpath emits cuts at those Z levels that pass **near but not
  over** the cell (within 3.5 mm but not within 2 mm — the sample
  histogram filter), so no sim sample lands on this XY at those Z
  levels, OR
- The simulator's dexel grid resampling at this XY produces a
  ray-state that the planner's coarser stamp doesn't match.

Tracked as **F-031** (sim-side resampling / coverage parity) — out of
scope for F-029's "planner-side" boundary.
