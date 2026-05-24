# Adaptive3D — small-DPP island bug + "mill shallow areas" feature

Status: scoping doc (not implementation) — post-compact pickup
Owner: TBD (new agent)
Date: 2026-05-23
Repro project: `~/Downloads/wanaka100/wanaka_full_tuned.toml` (Generic Hardwood, 6mm endmill,
Setup 1 Back Rough + Setup 2 3D Rough 6)

---

## Why one document for two features

Both live in `crates/rs_cam_core/src/adaptive3d/`, both touch the per-Z-level
planner, both depend on the `SlopeMap` that's already plumbed through
`ClearZLevelContext`. Easier to refactor once than twice.

The DPP-island bug is **correctness** — fixing it is non-optional. The
shallow-area feature is **net-new UX** — opt-in. Land them in this order:

1. Part A (correctness) — fix the small-DPP filters first so the shallow-area
   feature isn't validating against a buggy baseline.
2. Part B (feature) — add `mill_shallow_areas: bool` + slope threshold and
   wire it into the per-Z-level loop.

---

## Part A — DPP-scaled filters drop islands

### Symptom (user-reported)

> "On small step downs the rough starts to leave islands it does not mill.
> Is there some bug here that might be scaling with stepdown size?"

Yes, three filters drop progressively more material as DPP shrinks. None of
them are intentionally DPP-scaled — they're area / fraction thresholds that
*behave* DPP-scaled because thin slabs produce smaller per-level evidence
than thick slabs.

### Suspect #1 — sub-tool region filter (highest impact)

`crates/rs_cam_core/src/adaptive3d/clearing.rs:1232`

```rust
let tool_diameter = ctx.tool_radius * 2.0;
let min_region_area_mm2 = (tool_diameter * 2.0).powi(2);  // (2D)² → 144 mm² for a 6mm tool
regions.retain(|r| r.area().abs() >= min_region_area_mm2);
```

Documented as a Fusion-style "ignore micro-peaks" filter to suppress
heightmap noise. But at small DPP, the SAME real-island appears at many Z
levels and gets dropped at every one — never milled.

The fix has to change "area threshold" to "volume threshold" or equivalent:

- Option A: `min_region_volume_mm3 = (2D)² × DPP * k` — preserves the
  "noise vs. real feature" intent but unhooks it from DPP.
- Option B: persist a "skipped-but-not-cleared" mask across Z levels and
  force a cleanup pass when a region's footprint at a lower Z exceeds the
  threshold (rest-machining style).
- Option C: drop the filter entirely, accept the air-cut hit, lean on the
  perimeter sweep + small-region pruning that's already there. Cheapest
  fix; needs benchmarking to confirm we don't regress the Wanaka 81%
  air-cut case the comment cites.

Visibility: `ZLevelPlanMetrics.dropped_micro_region_count` is already
populated — the agent can quantify the bug pre-fix and the regression
post-fix without new instrumentation.

### Suspect #2 — per-level early-exit gate

`crates/rs_cam_core/src/adaptive3d/clearing.rs:1136`

```rust
let remaining = if let Some(r) = region {
    material_remaining_in_region(...)  // fraction of bbox cells with material above floor
} else {
    material_remaining_at_level(...)
};
if remaining < 0.005 {
    return Ok(());
}
```

At small DPP the per-level slab is thin → stragglers contribute fewer
cells → the level skips even when there's real material to clear.

Likely fix: change `remaining < 0.005` from a fraction to an absolute
cell-count gate, e.g. `cells_with_material >= 4` (mirrors `min_cells = 4`
in `detect_material_regions`). At DPP=0.5 with a small lingering region,
you'd still attempt the level instead of giving up.

### Suspect #3 — path-split threshold

`crates/rs_cam_core/src/adaptive3d/clearing.rs:1547`

```rust
let z_drop_threshold = ctx.depth_per_pass * 1.1;
```

Path-splitting at >1.1×DPP Z transition is correct in principle (peak →
valley bridges shouldn't drag the cutter through stock) — but at DPP=0.5
this threshold is 0.55mm, and any natural terrain undulation > 0.55mm
between consecutive samples chops the path. AgentSearch then plans many
short paths instead of one long sweep, and the linker may not stitch them
all up before the level ends.

Likely fix: make the threshold `max(DPP × 1.1, 1.0mm)` — a 1 mm floor
preserves the safety logic without over-splitting at small DPP.

### Cross-cutting

The three filters are all using the cell/area cosmos and assuming
"slab area ≈ work to do this pass". At tiny DPP slab thickness is the
missing factor — turn area into volume (or absolute cell count) anywhere
the threshold compares against work.

### Repro recipe

1. Load Wanaka, regenerate Back Rough at DPP = 3.0 (current).
2. Note `dropped_micro_region_count` summed across z-levels via
   `get_generation_debug_trace(span_kind="z_level_clear")`.
3. Set DPP = 0.5, regenerate.
4. Sum dropped counts again. Hypothesis: ≥ 3× the count.
5. Re-sim, compare cut-trace and check residual-cell coverage in the
   simulator's final mesh — un-milled islands should be visible.

### Acceptance criteria (Part A)

- Test: feed a synthetic single-cell-thick "spire" geometry, run
  adaptive3d at DPP = 4mm and DPP = 0.5mm, assert the residual material
  volume after generation is within 10% of each other (today the small-DPP
  case leaves substantially more).
- `dropped_micro_region_count` should converge as DPP shrinks
  (currently grows).
- Existing tests (`test_detect_regions_two_islands`,
  `test_flat_area_detection_finds_shelf`, the agent_search snapshot tests)
  still pass.
- Wanaka Back Rough at DPP = 0.5 produces no more residual islands than at
  DPP = 3.0 (visual check + cut-trace stats).

---

## Part B — "Mill shallow areas" (Fusion-style)

### What the user wants

> "On each pass down, it will do the small step down, not just on the
> bottom. But on low angle areas."

Fusion 360 calls this "Steep and Shallow" or "Flat area detection." The
behavior:

- Adaptive3d normally steps down at DPP and clears the slab at each level.
- On gentle slopes (low surface-normal angle vs Z), DPP-sized steps leave a
  visible staircase. Finish operations later smooth it out, but that
  doubles the work.
- "Mill shallow areas" inserts intermediate fine passes **only in the
  cells where the surface slope is below a threshold**, at the current
  pass. Steep cells get the normal DPP descent; shallow cells get the
  fine descent.
- The result: shallow areas come out smooth from the rough; steep walls
  stay efficient.

### How it differs from existing scaffolding

- `params.detect_flat_areas` — histogram-based, finds horizontal SHELVES
  (cells at the same Z). Doesn't help for gentle terrain.
- `params.fine_stepdown` — currently globally inserts fine intermediates
  at every Z (also currently buggy — see the separate
  `depth_per_pass / fine_stepdown` finding in the Wanaka session report).
  The new feature is regionally scoped, not global.
- `clearing.rs:952` already references slope-aware skipping for
  waterline cleanup ("This avoids re-tracing shallow areas that the
  …"). The same `SlopeMap` is the input we need.

### Proposed schema additions

```rust
// crates/rs_cam_core/src/adaptive3d/mod.rs
pub struct Adaptive3dParams {
    // … existing fields …

    /// When true, insert fine sub-passes on shallow-slope regions within
    /// each major DPP descent. Steep regions step down at DPP as before.
    pub mill_shallow_areas: bool,

    /// Slope angle below which a cell counts as "shallow." Measured from
    /// XY (0° = horizontal). Typical: 30°. None ⇒ disabled.
    pub shallow_angle_deg: Option<f64>,

    /// Stepdown to use for shallow cells. Typical: 0.25–0.5 × DPP. None
    /// falls back to `fine_stepdown` if set, else DPP × 0.3.
    pub shallow_stepdown: Option<f64>,
}
```

UI: a checkbox on the params panel ("Mill shallow areas"), a slope
slider (10°–45°), and a stepdown numeric input. Wire the same way as
existing adaptive3d params.

### Algorithm sketch

Per Z level in the existing top-down loop:

1. Do the normal DPP descent + clear as today.
2. If `mill_shallow_areas` is true AND `shallow_angle_deg` is set:
   a. From the `SlopeMap`, build a bool mask of cells where
      `slope_at(row, col) < shallow_angle`.
   b. Intersect with the per-Z material region (cells that still have
      material above the floor).
   c. If the masked region area is below the noise threshold (use the
      Part-A-fixed threshold — `(2D)² × shallow_stepdown` volume), skip.
   d. For each shallow sub-level at `z_level - k × shallow_stepdown`
      (`k = 1..ceil(DPP / shallow_stepdown)`):
      - Build a bool grid restricted to the shallow mask.
      - Run the existing 2D adaptive on the masked grid.
      - Stamp + emit segments as a normal pass at that sub-Z.

The shallow mask is **static** (a function of the model surface, not the
running material stock) so it can be computed once at start-of-toolpath.
What changes per sub-pass is the intersection with current material —
that's already what `build_material_bool_grid` does, just with an
additional mask AND.

### Touchpoints

- `crates/rs_cam_core/src/adaptive3d/mod.rs` — params struct + a test
  config for the new flag.
- `crates/rs_cam_core/src/adaptive3d/path.rs` — the top-down Z loop;
  wraps `clear_z_level_*` with the shallow sub-passes.
- `crates/rs_cam_core/src/adaptive3d/clearing.rs`:
  - `build_material_bool_grid` — accept an optional shallow-mask
    AND'd into the cell predicate.
  - Maybe a new `clear_shallow_sub_pass` helper that calls the existing
    `clear_z_level_agent_2d_slice` with the masked grid.
- `crates/rs_cam_core/src/slope.rs` (already exists) — `SlopeMap` has the
  per-cell slope. Confirm whether it's currently angle-from-XY or
  angle-from-Z and adapt if needed.
- GUI: `crates/rs_cam_viz/src/ui/properties/operations/` — add the
  checkbox + slope slider + stepdown field to the adaptive3d params
  rendering.
- CLI / MCP: `set_toolpath_param` already accepts arbitrary param names
  via serde round-trip — no new MCP plumbing required for set, but the
  diagnostic surface will need to know the new IDs.
- Optional: a new diagnostic ID `geom.shallow_stepdown_over_dpp` if the
  user sets shallow_stepdown ≥ DPP (silly config).

### Acceptance criteria (Part B)

- Cosmetic: on the Wanaka Back Rough, enabling shallow with
  `shallow_angle_deg = 30` and `shallow_stepdown = 0.5` produces a
  visibly smoother top surface in the simulated mesh than today, without
  affecting cycle time on the steep walls.
- Tests:
  - `test_shallow_disabled_matches_baseline` — flag off ⇒ identical
    output to current.
  - `test_shallow_inserts_subpasses_on_low_slope_mesh` — synthetic mesh
    that's all-shallow ⇒ count of Z levels ≈ DPP / shallow_stepdown × normal.
  - `test_shallow_skipped_on_steep_mesh` — vertical-wall mesh ⇒ no
    extra sub-passes inserted.
- Cycle time on steep-dominated geometry ≤ 5% slower than baseline (the
  cost should be confined to shallow-area cells).
- Air-cut diagnostic: shallow sub-passes shouldn't inflate the air-cut
  percentage materially (each sub-pass should be locally engaged).

---

## Risks / non-goals

- **Don't touch the AgentSearch internals during Part A.** The filters
  are downstream of region detection; the planner doesn't need to know.
- **Don't try to combine "shallow areas" with the broken `fine_stepdown`
  global mode in one PR.** Land Part A + Part B, then file a separate
  task to retire / repurpose `fine_stepdown` once the shallow feature
  covers the cases users actually want.
- The 6mm-endmill deflection limit on hobby stiffness is unrelated and
  shouldn't be in scope here.
- Don't change `detect_flat_areas` — keep the existing histogram
  behavior; the shallow feature is additive.

## Open questions

1. Should shallow sub-passes use the same `stepover` as the parent pass,
   or a tighter one? Fusion uses a separate "shallow stepover" knob.
   Probably defer until users ask.
2. How do we expose this in the GUI without param sprawl? Maybe collapse
   under an "Advanced / Surface quality" section in the operations panel.
3. The shallow mask interaction with `boundary` (the 2D polygon clip):
   confirm we AND with both — not "shallow OR boundary" by mistake.
4. Per-region vs. global shallow mask: if a model has both a steep peak
   and a separate shallow valley, does the shallow sub-pass run on the
   whole shallow region of the model, or per detected `MaterialRegion`?
   Per-region is probably right (consistent with the rest of the
   pipeline) but worth confirming.

## Pickup checklist for next agent

1. Read this doc + the Wanaka session report from 2026-05-23 (the one
   that established the DPP/fine_stepdown bug, ID coverage tests, and
   project-diagnostic bridge).
2. Re-read the three suspect code blocks named above. Decide on
   threshold-vs-volume strategy for Part A Suspect #1.
3. Build the synthetic repro fixtures (spire, all-shallow mesh, vertical
   wall) before writing fixes — they're the acceptance test inputs.
4. Land Part A first (PR small + correctness only). Smoke test on
   Wanaka Back Rough at DPP=0.5 vs DPP=3.0 to confirm islands go away.
5. Land Part B on top. Wire MCP / GUI plumbing only after the core algo
   has the disabled-mode-matches-baseline test green.
6. Update `FEATURE_CATALOG.md` and the operations docs when Part B ships.
