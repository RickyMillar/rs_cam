# F-029 — Adaptive3d interior-cell planner↔simulator stamp parity gap

- **Stage:** sim / adaptive3d
- **Severity:** medium (residual after F-027 lands; deflection still
  fires Exceeds on AS013 at 0.013% of samples, but the model-edge
  cohort is fully resolved)
- **Status:** open — opened by F-027 implementer during landing
- **First found in:** F-027 implementation verification (2026-05-25)
- **Effort:** M
- **Linked PRs:** (none yet; opened with F-027)
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
