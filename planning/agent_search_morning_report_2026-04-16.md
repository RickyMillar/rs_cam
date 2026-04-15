# AgentSearch overnight iteration — morning report

## What I discovered

Research into the 2D adaptive (our working reference) revealed the
**root structural cause** of "scattered passes, no sweeping arcs":

The 2D `find_entry_point` has **two phases**:

1. **Phase 1 — walk the material boundary polygon**. Samples points along
   the outer polygon edge, picks the boundary point with highest tool
   engagement not too close to a prior endpoint. This makes every new
   pass start "outside-in" at the remaining material frontier.

2. **Phase 2 — grid scan fallback**. Only runs if no viable boundary
   point exists (the interior-only case).

The 3D `find_entry_3d` had **only Phase 2**. Every first pass started at
"nearest material to last position", often in the middle of virgin stock.
This is why:
- Passes start in random scattered positions
- Each pass spirals inward briefly then exits
- Next pass picks another random spot → no chaining
- Visual pattern = "scattered blobs" not "one expanding spiral"

## What I changed

**`crates/rs_cam_core/src/adaptive3d/`**

1. **`search.rs`** — added `polygons: Vec<Vec<P2>>` to `BoundaryField`
   (the per-Z-level snapshot of material boundaries).
2. **`clearing.rs`** — `clear_z_level` now runs `marching_squares_bool_grid`
   on the material bool grid and stores the boundary polygons.
3. **`search.rs`** — `find_entry_3d` now has a **Phase 1** that walks each
   polygon edge, samples points at `cell_size * 2` spacing, computes
   engagement via `compute_engagement_3d`, skips points near prior
   endpoints, and picks the point with highest engagement. Only falls
   back to the existing grid scan if Phase 1 finds nothing.
4. Added `boundary: Option<&BoundaryField>` parameter to `find_entry_3d`
   (other test callers pass `None` — unchanged behavior for tests).

Build: clean. `cargo clippy -D warnings`: clean for core, viz-lib, and
viz-bin. 41 adaptive3d tests: pass.

**I could not visually verify this change tonight because the MCP server
is stdio-based and Claude Code auto-spawns it. I don't have a way to
trigger client reconnect after killing the old binary.** See "Verify in
morning" below.

## All the live-tuning knobs you have now

Set via `set_toolpath_param index=0 param=<name> value=<float>`:

| knob | default | purpose |
|------|---------|---------|
| `agent_angle_weight` | 0.03 | angle-penalty in scorer |
| `agent_tolerance` | 0.05 | engagement tolerance band (fraction of target_frac) |
| `agent_wall_bias_weight` | 0.15 | tangent-along-wall bias |
| `agent_wall_threshold_mul` | 2.0 | wall bias kicks in within `mul × tool_radius` |
| `agent_uturn_threshold` | 0.85 | normalized angle above which U-turn surcharge starts |
| `agent_uturn_weight` | 5.0 | U-turn surcharge slope |
| `agent_min_engagement_floor` | 0.0 | reject candidates with `eng < floor × target_frac` |

All atomic u64 — writes have no perf cost, reads happen per scoring call.
No restart needed between sweeps.

## All the arc-quality metrics

Per-pass counters (`get_generation_debug_trace`, filter
`span_kind=adaptive_pass`):

- `mean_angle_delta` (rad) — avg |Δangle|/step. Good arc: <0.15 rad.
  Zigzag: >0.3 rad.
- `angle_delta_std` — consistency of curvature.
- `sign_flip_rate` — 0–1. Low = smooth spiral; high = zigzag.
- `path_length` (mm)
- `sinuosity` = path_length / bbox_perimeter. **Close to 1.0 = ideal
  spiral.** >10 = "stuck wandering in tiny bbox".
- `mean_engagement`, `engagement_std`, `max_engagement`,
  `over_target_rate` — are we actually cutting at target?

Diagnostics aggregate (`.diagnostics.arc_quality` and
`.diagnostics.engagement`):

- `avg_mean_angle_delta`, `avg_sinuosity`, `max_sinuosity`,
  `zigzag_passes`, `worst_arc_passes` (top-10 worst)
- `target_frac`, `avg_mean_engagement`, `max_engagement`,
  `high_engagement_passes`, `avg_over_target_rate`

## Verify in morning

1. `/mcp` to spawn fresh GUI (pulls in the boundary-walk change)
2. Run with a known-good tuning to baseline:
   ```
   load_project ...sample_3d_project.toml
   import_model fixtures/terrain_small.stl
   add_tool EndMill 2mm
   set_stock_config 110 83 55
   add_toolpath adaptive3d, 2mm, model 2
   set stock_top_z=55, clearing_strategy=agent_search,
       depth_per_pass=10, stepover=0.5, debug_enabled=1
   set agent_angle_weight=0.15, agent_uturn_threshold=0.7,
       agent_uturn_weight=10, agent_tolerance=0.3
   generate_toolpath 0
   run_simulation; screenshot_simulation
   ```
3. Compare against run #13 baseline
   (`planning/probe_artifacts/2026-04-15_13_engfloor_025/`):
   - **Expected improvement**: passes chain into longer paths
     (pass_count should drop from ~566 toward ~100–200), and the
     toolpath top-down should show fewer disjoint short spirals.
   - **Metric to watch**: `avg_sinuosity` — if it goes from 1.6
     toward ~1.0, we're producing true outward spirals.
   - **Visual check**: sim top-down should show **continuous
     concentric-like contours** emanating from stock edges instead of
     scattered patches.

## If the boundary-walk change is not enough

Two more structural levers to try (listed in order I'd try them):

### A. Slot clearing seed (FreeCAD Adaptive2d does this)

The 2D adaptive also has `slot_clearing`: generate sparse zigzag lines
at wide spacing across the polygon, cut them FIRST as seeds, then run
adaptive passes that expand outward from the slot boundaries. This
gives each Z-level a head-start void so the first adaptive pass is
already at a cleared frontier.

Concrete plan:
- At start of each Z-level, after pre-stamp, before pass loop:
  - Get the material polygon (already have it in `boundary_field.polygons`)
  - Call `crate::zigzag::zigzag_lines` with spacing = `tool_radius * 4.0`
  - For each line: stamp along the line (mimicking a cut) and append
    segments.push(`Adaptive3dSegment::Cut(slot_points)`)
- Should be ~100 LOC. `zigzag_lines` is already written for 2D adaptive.

### B. Pass chaining ("continue at frontier" instead of exit)

When a pass is about to exit with `no material`, scan for a frontier
cell within 2–3 tool radii; if found, emit a Link segment and restart
the pass step loop from that cell. This turns many short passes into
fewer long paths that visually look like one continuous outward spiral.

More invasive (~200 LOC) but probably the cleanest visual win.

### C. Force outside-in via pass ordering

Introduce an explicit "first-pass-is-outermost" rule: at each Z-level,
the first pass must start on the OUTER stock boundary (not any
boundary). Subsequent passes can start anywhere. This mirrors Fusion
360's "ForceInsideOut = true" flag.

## Reference implementations for deeper reading

- **libactp / Adaptive2d** (GPL, C++):
  https://github.com/Heeks/libactp-old — archived but has the original
  Freesteel algorithm
- **FreeCAD Adaptive2d** (LGPL):
  lives in FreeCAD's `src/Mod/CAM/libarea/Adaptive.cpp` — modern
  maintained fork
- Our 2D port: `crates/rs_cam_core/src/adaptive/` — 2809 LOC, works
  correctly, produces the expected spiral pattern

## File diff summary

```
crates/rs_cam_core/src/adaptive3d/clearing.rs  +12 lines (polygons from MS)
crates/rs_cam_core/src/adaptive3d/search.rs    +65 lines (Phase 1 boundary walk)
crates/rs_cam_core/src/adaptive3d/mod.rs       +2 lines (test fixes)
crates/rs_cam_viz/src/app/mcp.rs                ~5 lines (collapsible_if fix)
```

All changes gated so default-tuning runs produce same behavior when
`boundary` is `None` (legacy test paths). The real test is when
`clear_z_level` passes the populated `BoundaryField` — Phase 1 fires
and first-pass entries anchor to the stock boundary.
