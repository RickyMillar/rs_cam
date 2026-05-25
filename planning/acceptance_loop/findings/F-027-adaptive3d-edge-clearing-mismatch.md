# F-027 — Adaptive3d planner stock XY mismatch with simulator stock causes edge axial spikes

- **Stage:** sim / adaptive3d
- **Severity:** medium (residual after F-026 lands; deflection still
  fires Exceeds on auto_from_model 3D ops at <0.5% of samples, but
  rapid-collision count is fixed)
- **Status:** landed
- **First found in:** round-05 follow-up investigation (2026-05-25)
- **Effort:** M
- **Linked PRs:** commit `b1f17fe` (model-edge band closed; interior-cell residual carved out as F-029)
- **Source audits:** F-026 implementer's repro of AS013 against
  `test_data/ux_3d_terrain.toml` via `ProjectSession::run_simulation`
  (tests/dexel_stock_z_frame_f026.rs at the implementer's investigation
  point, before scope-trimming).

## Symptom

After F-026's load-time `auto_from_model` re-derivation lands (stock
auto-grows to enclose model on `ProjectSession::load`), AS013-shape
deflection over-fires on adaptive3d-on-terrain remain.

Reproduction (AS013-shape adaptive3d on terrain via
`ProjectSession::run_simulation`, terrain mesh
`fixtures/terrain_small.stl` bbox `[0, 0, 0]..[100, 73.3, 52.6]`,
adaptive3d depth_per_pass = 3, stepover = 1.2):

- F-026 fixes `rapid_collisions` from **844 → 0** (the most reliable
  signal — full F-026 win).
- ≈ 638 745 of 1 147 279 cutting samples fall in `axial < 5 mm`
  (commanded DPP is 3) — the bulk of the toolpath is healthy.
- ≈ 800 samples report `axial_engagement_mm > 5 mm`, with a tail of
  5–10 samples reading 30–47 mm.
- The outliers cluster at sample positions with Y ≈ 76.3 mm — the
  edge of the terrain bbox + tool radius (`bbox.max.y = 73.3`, `r = 3`,
  so `73.3 + 3 = 76.3`). The model XY extent doesn't reach this Y,
  but the simulator's per-setup dexel grid (sized from the auto-grown
  stock bbox) does.
- The deflection gate picks up the worst single sample
  (`peak_mm ≈ 0.685`) and fires Exceeds, even though the bulk of the
  toolpath sits well below the 0.2 mm bound.

## Root cause (probable)

`adaptive3d::path::adaptive_3d_segments` constructs its
planner-internal `material_stock` from the mesh bbox:

```rust
// crates/rs_cam_core/src/adaptive3d/path.rs:160-198
let bbox = &mesh.bbox;
let origin_x = bbox.min.x - r;
let origin_y = bbox.min.y - r;
let extent_x = bbox.max.x + r;
let extent_y = bbox.max.y + r;
…
TriDexelStock::from_stock(origin_x, origin_y, extent_x, extent_y,
                         bbox.min.z, params.stock_top_z, cell_size)
```

For ux_3d_terrain (mesh bbox `[0, 0, 0]..[100, 73.3, 52.6]`, `r = 3`):
- adaptive3d planner stock XY: `[-3, 103] × [-3, 76.3]`.
- Simulator per-setup stock XY (post-F-026, auto-grown):
  `[-5, 105] × [-5, 96]` — extra ≈ 20 mm strip on the +Y side.

Cells in the simulator grid at Y > 76.3 are never visited by
adaptive3d's planner stamps (they're outside its grid). The simulator
carries them as virgin material `[0, stock_top_z]` for the entire
toolpath. When the cutter sweeps near the model boundary, its
footprint extends into those cells and the simulator's stamp
clears the full pre-stamp ray in one shot — `axial_engagement_mm`
reads the full stock height.

This is the **XY analog** of F-024 (Z-frame mismatch): F-024 fixed
identity-setup Z frame between adaptive3d and the simulator; F-027
needs to align the XY frame too.

## Fix shape (sketch)

Two equivalent framings, both plausible:

- **Adaptive3d-side**: widen the planner's `material_stock` XY to
  the world stock bbox passed in via `Adaptive3dParams` (or a new
  `stock_xy_bbox` field) when one is available. Mesh-bbox-only
  initialization stays as the fallback for tests that don't pass a
  world stock.

- **Simulator-side**: when constructing the per-setup dexel grid,
  trim/clip rays to the model XY footprint when the operation is 3D
  (adaptive3d, scallop, drop_cutter, waterline). Cells outside the
  footprint are not "stock that needs to be cut" by the 3D op —
  they're under workholding / fixturing scope.

The adaptive3d-side fix is more conservative (preserves the existing
simulator semantics) and follows the same pattern F-024 used (align
the planner with the simulator's frame, not the other way around).

## Acceptance test

Reuse the F-026 reproducer setup (load `ux_3d_terrain.toml`,
generate adaptive3d with round-05 baseline params, run through
`ProjectSession::run_simulation`), then assert:

1. Worst-case `axial_engagement_mm` across all cutting samples ≤
   `depth_per_pass + 0.5 mm` margin (post-fix expected ≈ 3 mm; pre-fix
   reads 30–47 mm at the model-edge outliers).
2. `deflection.peak_mm < 0.2` on the tool-load verdict.

The F-026 acceptance test deliberately doesn't enforce these because
they're outside F-026's scope; F-027's landing should add them as a
follow-up to the same test file.

## Risk / notes

- Medium effort: adaptive3d's planner stock initialization is one
  function, but the planner uses `material_stock.z_grid.{origin_u,
  origin_v, cols, rows}` extensively for region detection and entry-
  search. Widening the planner stock changes those dimensions and may
  affect the runtime/memory of large 3D jobs. Bench impact unknown.
- F-026 fixed the rapid-collision part of the round-05 AS013 signature
  cleanly (844 → 0). F-027 is the residual that keeps deflection at
  Exceeds.
- Cross-link: F-017 (rapid collisions) — F-026 already addresses the
  AS013 portion. F-017 can close after F-027 verifies that no
  collision counts remain on 3D ops.
- The F-026 acceptance test file (`dexel_stock_z_frame_f026.rs`) has
  a `Residual scope` note that points here.
