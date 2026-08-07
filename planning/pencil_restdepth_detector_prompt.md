# Build the rest-depth-field pencil detector (detector #4) + rest-driven routing

Implement pencil detection the way commercial CAM does it: from a **rest-depth
field** (dual tool-offset comparison), not from design-surface features. Then
route detected regions by width — narrow → pencil centerlines, wide → adaptive3d
rest-clearing — so pencil runs **only where it is needed**.

Read `planning/pencil_postmortem_and_rest_driven_design.md` first (the verified
research + post-mortem this plan implements). One-line version: our three prior
detectors (dihedral, drainage, curvature crest lines) all detected features of
the DESIGN surface and were tool-radius-blind; industry (Mastercam bitangent
double-contact, PowerMill Reference-tool rest areas, Inventor CAM dual-tool
inputs) and the canon (Choi & Jerard Z-map; Park/Choi offset self-intersection
"virtual digitizing", material-side tracing) define pencil in TOOL-OFFSET space,
where the tool radius is a built-in morphological noise filter.

## The core computation

```
rest(x, y) = drop_z(reference_tool, x, y) − drop_z(pencil_tool, x, y)
```

evaluated on a regular XY grid over the mesh bbox with the existing
`crate::dropcutter::point_drop_cutter` (parallel via rayon, `feature =
"parallel"` — copy the dispatch pattern from `pencil.rs::lift_to_surface`).

- `pencil_tool` = the op's actual cutter (`ctx.tool_def`, may be tapered ball).
- `reference_tool` = `BallEndmill::new(params.reference_tool_diameter, 25.0)` —
  the existing param (default 6.0). Same construction as the current
  reference gate in `pencil_toolpath_structured_annotated`.
- `rest > 0` exactly where the pencil tool reaches deeper than the reference
  tool could — i.e. material the finish pass left that the pencil can remove.
  Flat areas, texture smaller than the pencil radius, and everything the
  reference already reached all read ≈ 0 **by construction**. No smoothing, no
  box-blur, no gates. If you find yourself adding a denoise step, stop — the
  tool radius already did it; investigate what's wrong instead.

Semantics note: with this detector the field threshold IS the existing
`min_valley_depth` param ("extra depth past the reference tool", default via
`reach_gap_threshold()` = 0.05). The main dial is therefore already wired
end-to-end (config/catalog/execute/GUI). The per-polyline
`polyline_passes_depth` rest gate is REDUNDANT for this detector (it re-checks
the same quantity pointwise) — skip it in the RestDepth branch.

## Pipeline (new module `crates/rs_cam_core/src/rest_field.rs`)

1. **Grid build**: cell size = new param `rest_cell_mm` (default 0.5). Grid =
   mesh bbox + 1 cell margin. For each cell centre, drop BOTH tools; store
   `rest`, plus a `valid` mask (either drop non-contact ⇒ invalid). ~160k cells
   × 2 drops on wanaka at 0.5mm — same order as the old drainage DEM build,
   which took seconds in release. Reference drop of the field is per-op work;
   don't cache across ops in v1.
2. **Region mask**: `rest > min_valley_depth` on valid cells. 8-connected
   components (flood fill). Drop components smaller than a few cells.
3. **Per-region width**: chamfer distance transform (two-pass, 1 / √2 costs)
   of the mask → each mask cell's distance to the nearest non-mask cell =
   local half-width in cells.
4. **Skeleton**: Zhang-Suen morphological thinning of the mask to a 1-cell
   skeleton, then trace into polylines: nodes = skeleton cells with 8-degree ≠
   2, walk node→node with a directed-edge visited set, then sweep pure loops.
   (These three algorithms existed in the deleted, never-committed
   `valley_network.rs` — re-implement from the spec in the appendix; the tricky
   details are listed there.)
5. **Centerline Z**: skeleton cells carry XY; set Z from the surface (either
   the pencil-tool drop already computed, or leave interpolation to
   `lift_to_surface` downstream, which re-solves Z gouge-safely anyway — do the
   latter, it's what the drainage detector did).
6. **Route by width**: at each skeleton polyline, take the median distance-
   transform value along it → region half-width `w`.
   - `w ≤ route_width_factor × pencil_radius` (new param, default ~2.0):
     **pencil** — emit the polyline as a centerline; `num_offset_passes`
     applies as today.
   - wider: **clearing region** — in v1 do NOT emit pencil passes for it;
     count it, log it (`info!`), and stash its boundary (march the mask
     component's outline or just its bbox in v1) in the debug trace. Phase D
     turns these into adaptive3d `FromRemainingStock` boundaries.
7. **Output**: `Vec<Vec<P3>>` centerlines → existing pipeline unchanged:
   `min_cut_length` filter → `resample_polyline` → `paths_from_sampled`
   (fair → lift → offset → link → emit).

## Detection significance / quality metrics (the agent-feedback fix)

Emit a `RestFieldReport` (struct in `rest_field.rs`, logged via `info!` and
attached to the debug trace):
- `total_rest_volume_mm3`: Σ rest × cell² over the mask — what SHOULD be removed.
- `pencil_region_count` / `clearing_region_count` (by the width route).
- `skeleton_length_mm` total vs `traced_length_mm` after min_cut_length —
  **coverage** = traced/skeleton.
- Per-region peak rest depth (the significance ranking — deep = important).

These make "is this pencil path good?" quantifiable for the first time:
after simulation, residual = re-evaluate Σ max(0, rest_after) (Phase D wiring;
v1 just reports the before-field numbers).

## Integration points (exact)

- `PencilDetector` enum (`pencil.rs`): add `RestDepth` variant. `parse`:
  accept `"rest_depth" | "restdepth" | "rest"`; keep `Dihedral` default and
  `Curvature` as-is (Curvature stays as the A/B baseline; decide with the user
  about stripping it AFTER RestDepth validates — mirror the drainage decision).
- `PencilParams`: add `rest_cell_mm: f64` (default fn `rest_cell_default() ->
  0.5`) and `route_width_factor: f64` (default fn → 2.0). `min_valley_depth`
  and `reference_tool_diameter` are reused as-is.
- Wire the two new params through, following the exact pattern of
  `valley_saliency` (added 2026-06-26, see git diff of that change once
  committed — or grep `valley_saliency` for the six touch points):
  `operation_configs.rs` PencilConfig + `#[serde(default = …)]` + Default impl;
  `catalog.rs` PENCIL_PARAMS; `execute.rs` generate_pencil; test literals in
  `pencil.rs` (6 sites), `tests/param_sweep.rs`, `tests/capability_link_moves_safety.rs`.
- Branch in `pencil_toolpath_structured_annotated` next to the Curvature arm:
  build the field, detect, route, then feed kept lines through the same
  `resample_polyline` + `paths_from_sampled` calls the Curvature arm uses. Skip
  `polyline_passes_depth` (redundant, see above). For RestDepth, `reference
  cutter` must be built even when `reference_tool_diameter <= pencil diameter`
  — in that degenerate case fall back to self-referenced rest (reference =
  bare-surface probe `BallEndmill::new(0.1, 10.0)`), matching how the drainage
  DEM probed the raw surface.
- GUI (`crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`,
  `draw_pencil_params`): the Detector combo gains "Rest depth (recommended)".
  When selected show: Rest Cell, Route Width ×, Min Valley Depth, Reference
  Tool Ø (the last two already exist; Reference Tool Ø currently shows only
  when `min_valley_depth > 0` — for RestDepth show it always, it's the core
  input). Keep Curvature's controls as they are.
- MCP: nothing new needed — `set_toolpath_param` handles the new fields via
  serde round-trip automatically once they're in PencilConfig + catalog.

## Validation protocol (durable user instruction: render and LOOK, do not lean on unit tests)

1. Unit floor (cheap, not the bar): V-valley yields one centerline on the
   trough; tent ridge yields none; gentle reachable surface (radius ≫ both
   tools) yields none; raising `min_valley_depth` monotonically thins; flat
   plate empty. Model on `crest_lines.rs` tests.
2. **Hillshade overlay harness** (the key view): `#[ignore]` test
   `render_restfield_hillshade` copying `crest_lines.rs::render_crest_hillshade`
   — grey slope-shaded terrain, detected centerlines green, CLEARING-routed
   regions outlined in a second colour (e.g. orange) so routing is visible.
   Env: `RS_CAM_REST_FIXTURE/OUT/CELL/MVD/REFD/ROUTEW`. Fixture:
   `/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl` (661k tris).
   Also print the RestFieldReport to the sidecar .txt. Sweep MVD 0.05/0.2/0.5
   and REFD 2/6/12 — the field must visibly respond to the REFERENCE tool
   (bigger reference ⇒ more rest ⇒ more/wider regions), which no previous
   detector could do. `xdg-open` the PNGs.
3. **Live MCP** on wanaka200 (`load_project /home/ricky/Downloads/wanaka200/
   wanaka200.toml`, pencil op is the "Pencil (fixed+fast)" entry — index may
   shift; find it via `list_toolpaths`). Set `detector=rest_depth`, generate,
   `narrate_toolpath`, `screenshot_toolpath`, and judge in the live 3D view.
   Expected shape: coherent valley centerlines like the drainage detector's
   best output, but positioned by reachability, plus a report of how much area
   routed to clearing.
4. Bar: (a) centerlines sit in grooves on the hillshade; (b) REFD dial visibly
   moves the detection; (c) live wanaka pencil is coherent (rapid:cutting far
   below the dihedral era's ~1:1); (d) RestFieldReport coverage > ~80% of
   skeleton length at defaults.

## Phases + gates

- **A**: `rest_field.rs` (grid/field/regions/DT/skeleton/trace/report) + unit
  tests + hillshade harness. Gate: hillshade renders pass the bar on terrain.stl.
- **B**: wiring (enum/params/config/catalog/execute/test literals/GUI). Gate:
  clippy clean workspace, per-crate tests green.
- **C**: live wanaka validation + dial sweeps. Gate: user says the paths look
  right in the GUI.
- **D** (separate approval): wide-region routing → adaptive3d
  `FromRemainingStock` boundaries + residual-volume metric post-sim; then
  RE-RUN the multi-tool A/B (the 2026-06-24 "rest does not pay" verdict is
  INVALID — it was measured while FromRemainingStock was dead code; see memory
  `project_multitool_finishing`).

## Constraints (durable)

- ONE cargo job at a time: `free -g` + `pgrep -af "[c]argo (check|test|build)"`
  before every launch; yield to the user's GUI release builds. Concurrent heavy
  cargo has crashed this machine three times.
- Zero-warning clippy (16 deny lints; `#[allow(clippy::indexing_slicing)]` +
  `// SAFETY:` for provably-bounded grid indexing — the old DEM code used
  exactly this pattern heavily; tests are exempt via the standard test-mod allow).
- Per-crate tests only (`cargo test -p rs_cam_core --lib`), never workspace-wide.
- `cargo fmt` cascades into `strategy_advisor*.rs` — revert those before commit.
- Commit ONLY when the user asks. Sign-off: `Co-Authored-By: Claude Opus 4.8
  <noreply@anthropic.com>`. wanaka200 live params are LIVE-ONLY — never save.
- GUI validation trap #1: the running GUI uses the OLD binary until the user
  rebuilds + restarts; `project_summary.build` git_desc/timestamp can mislead —
  the live `param_schema` from `get_toolpath_params` is the ground truth for
  "which code is running".
- GUI validation trap #2 (found 2026-06-26): the properties panel writes its
  drawn values back every frame — setting a param via MCP while the pencil
  panel is visible gets clobbered. `set_ui_view workspace=setup` first, then
  set params, then generate.

## Uncommitted state you inherit (commit FIRST, with user approval)

Working tree (branch `experiment/adaptive-spiral`) currently holds three
logical features, all validated, none committed:
1. Curvature crest-line detector: `crest_lines.rs` (new), `pencil.rs`
   (Curvature variant, Drainage stripped, `valley_network.rs` deleted),
   operation_configs/catalog/execute + test literals.
2. GUI pencil controls: `surface_3d.rs` (Detector combo, Valley Saliency,
   Curv. Smoothing, Min Valley Depth, Reference Tool Ø).
3. Rest-machining fix: `controller/events/compute.rs` (prior_stock from sim
   checkpoints + StockSource import + fallback warning) and
   `session/compute.rs` (hoisted prior_stock → `initial_stock` when
   FromRemainingStock). This fix is what Phase D builds on.
Plus planning docs (`pencil_postmortem_and_rest_driven_design.md`, this file,
`pencil_curvature_detector_prompt.md`). Recommend three commits in that order.
Ask the user before committing.

## Appendix — algorithm specs for the deleted raster machinery

These lived in `valley_network.rs` (never committed; do not search git).
Re-implement in `rest_field.rs`:

- **Chamfer distance transform** (mask → f32 per cell): init set-cells to a
  big value, unset to 0. Forward raster pass relaxing from W, N, NW, NE
  neighbours with costs 1/1/√2/√2; backward pass from E, S, SE, SW. Result =
  distance in cells to nearest unset cell (local half-width on a region mask).
- **Zhang-Suen thinning** (mask → 1-cell skeleton): iterate two sub-passes
  until no change. Neighbours p2..p9 clockwise from North. Delete a set cell
  when: 2 ≤ B(p) ≤ 6 set neighbours; A(p) = exactly one 0→1 transition around
  the ring; sub-pass 0: ¬(p2∧p4∧p6) ∧ ¬(p4∧p6∧p8); sub-pass 1: ¬(p2∧p4∧p8) ∧
  ¬(p2∧p6∧p8). Collect deletions per sub-pass, apply after the scan
  (synchronous update). Out-of-bounds neighbours read as unset.
- **Skeleton tracing** (skeleton mask → polylines): degree = count of set
  8-neighbours. Nodes = cells with degree ≠ 2. For each node, for each set
  neighbour, walk cell-to-cell through degree-2 cells (never back to the cell
  you came from; mark each directed edge (from,to) AND (to,from) visited)
  until hitting a node or a dead end; the visited-set prevents retracing.
  After node-anchored walks, sweep remaining untraced edges the same way to
  pick up pure loops. Emit world-XY points at cell centres.
- Determinism: iterate cells in row-major order everywhere (no HashMap
  iteration in a path-affecting position) — see the T14 lessons in `pencil.rs`.
