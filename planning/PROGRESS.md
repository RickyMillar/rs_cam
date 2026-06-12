# Progress

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 4-crate Rust workspace: core library, CLI, desktop GUI, and MCP server
- 23 GUI-exposed operations
- 14 direct CLI commands plus TOML job execution
- STL, SVG, DXF, and STEP import with BREP face selection
- 5 cutter families
- GRBL, LinuxCNC, and Mach3 post-processors
- feeds/speeds calculator with machine, material, and vendor-LUT inputs
- tri-dexel stock simulation (Z/X/Y grids, all 6 cardinal faces) and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code
- unified service layer: `ProjectSession` API in core, shared `execute_operation()` dispatch for all 23 ops
- MCP server (`rs_cam_mcp`) exposing `ProjectSession` tools for AI agent integration

## Recent work (2026-06-08)

### UI/IA cleanup — audit complete, execution committed

Full code-led IA audit of `rs_cam_viz` (2 passes, 76 findings) plus a redesign
spec and a committed execution plan. Root cause across nearly all findings: the
same widget reimplemented per-surface (provenance color 3×, load rollup 2 ways
that disagree, 105+ inline section headers) — the fix is a shared `ui::components`
layer. Committed: full send to **egui 0.34.3** (spiked at ~3–5 mechanical days,
`egui_plot` must be 0.35), rewrite the worst surfaces on the component layer,
scope = clean up existing (no new features), MCP/assistant deferred but backend
left MCP-ready. All docs in `planning/ui_audit/`: **`TRACKER.md`** (live status +
parallelization + wave plan), `BACKLOG.md` (ranked workstreams), `ARCHITECTURE.md`
(component layer), `FINAL_DESIGN.md` (mockups), `DIAGNOSIS.md` + `pass2/`,
`SPIKE_egui034.md`. Status: nothing implemented yet; Wave 0 (egui upgrade ‖
Tier-0 engine fixes ‖ provenance data model) is next.

## Recent work (2026-05-20)

### Dexel-fidelity roadmap — Step 2 tail PR (radial_engagement scalar deletion)

Closes the §10.3 follow-up that was scheduled at Step 2 landing.
`SimulationCutSample.radial_engagement: f64` is removed. All consumers
read `sample.engagement.radial_woc_fraction` (or the structured
per-kinematics block). `SIMULATION_CUT_TRACE_SCHEMA_VERSION` bumps
3 → 4.

Migration touched 18 files (5 tool_load/* gates, 2 GUI accumulators,
the simulation accumulator, narrate, 8 test files + benches). New
`Engagement::with_radial_woc(r)` helper minimises test-fixture churn.
`SimulationCutIssue.radial_engagement` and `min_radial_engagement`
remain — those are snapshot fields on the issue payload, distinct
from the deleted sample-level scalar; they now source from the
structured engagement vector.

The DEXEL roadmap is now fully closed including all tracked
follow-ups; the only items still open are the §11 indefinitely-
deferred set (F.b sub-cell ray storage, DC mesh extraction, side-grid
MC, canned-cycle G-code, deflection coupling, grain modelling).

`/verify` clean: cargo fmt --check ✓, clippy --workspace --all-targets
-D warnings ✓, full workspace test 1676 passed / 0 failed / 122
ignored (the ignored set is pre-existing — wanaka_back_rough chipload
gate blocker, two multi-setup tri-dexel known failures, etc).

### Dexel-fidelity roadmap — Step 5 J (marching cubes mesh extraction)

Step 5 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` lands. The roadmap
is closed: all 5 steps ☑.

`z_grid_to_solid_mesh` now delegates to a new
`crates/rs_cam_core/src/dexel_mesh_mc.rs` module that implements
marching-cubes mesh extraction in its corner-bilinear height-field
reduction. The §6.J revision defines the SDF as `sdf(u, v, z) =
interp_ray_top(u, v) − z` where `interp_ray_top` is bilinearly
interpolated from the four cells surrounding each cell-corner. This
SDF is linear in z, so the MC iso-surface (`sdf = 0`) reduces to the
height field `z = interp_ray_top` — we emit it directly rather than
running a voxel sweep. The output is mathematically equivalent to
running standard 256-case MC tables on the bilinear SDF.

Closure (sides not produced by the top-iso surface):

- **Top face**: corner-bilinear `ray_top`, vertices at cell-corner
  positions. F.a's coverage-weighted ray_top from Step 4 feeds the
  bilinear average directly — boundary cells with partial coverage
  blend smoothly into the surrounding height surface, giving sub-cell-
  accurate wall positions at low resolutions.
- **Bottom face**: mirror — corner-bilinear `ray_bottom`.
- **Perimeter skirt**: vertical quads at the grid bbox edge.
- **Internal hole walls**: vertical quads at corners adjacent to
  empty (through-hole) cells.
- **Multi-segment cavity fallback**: when any ray has > 1 segment,
  per-gap horizontal floor/ceiling faces and vertical walls are
  emitted (preserving the legacy cavity-emission semantics).

`dexel_stock_to_entry_surface_mesh` (the 20 Hz live-preview path)
stays on the fast heightmap top-surface mesh per §6.J's scope split.
Only the closed-solid `dexel_stock_to_mesh` path moves to MC.

Side-grid path (`side_grid_to_mesh`) dropped from scope (4-axis
future-proofing; not in active 3-axis-from-top flows). Legacy
heightmap extractor retained as private
`z_grid_to_solid_mesh_heightmap` for reference.

7 legacy `dexel_mesh::tests` were tightly coupled to the heightmap
vertex layout (`mesh.vertices[2]` = first vertex z; "first cells
verts = top face"; hard-coded `50` / `288` counts). All 7 rewritten
as topology / point-cloud queries that survive the MC swap: assert
non-empty, indices in range, colour buffer matches vertex buffer,
z range matches expectation, find vertex at a position via point
cloud rather than fixed index.

6 new Step 5 §9 acceptance-gate regression tests in
`crates/rs_cam_core/tests/step5_marching_cubes.rs`:

1. Watertightness on an uncut block (position-keyed edge counting
   since MC emits per-triangle vertices without index dedup).
2. Watertightness on a partial-cut block.
3. Vertex z bounded by `[stock_bottom, stock_top]`.
4. MC triangle count ≤ 10× heightmap preview count (well within the
   §6.J 5–10× memory budget).
5. Pocket cut produces vertices at both the floor (`z = cut_top`)
   and the rim (`z = stock_top`) — verifies the wall transition.
6. Drill-CSG ↔ MC seam: analytic cylinder ring (Step 3 PR1) lands
   within 1.5× cell_size of the MC mesh wall vertices, closing the
   §10.7 intermediate-state seam.

WANAKA end-to-end mesh revalidation in
`crates/rs_cam_core/tests/wanaka_step5_mc_revalidation.rs`: loads
WANAKA, runs sim, asserts the final closed-solid mesh and every
per-toolpath checkpoint mesh is well-formed (non-empty, finite
positions, indices in range, z within stock envelope ± padding).

Bench A/B (`bench_dexel_mesh_extraction`, 4-bench group): see follow-
up commit for full numbers. Cumulative Step 0–5 delta vs pre-Step-0
baseline expected within the §10.5 50 % cumulative ceiling — MC
adds explicit perimeter + closure emission on top of corner-bilinear
faces, roughly 2× the legacy triangle count for an uncut block (still
well under §6.J's 5–10× envelope).

Algorithm decision: vanilla MC (height-field reduction) over dual
contouring per the trade-off review in this session — F.a's coverage
ramp narrows the boundary-sharpness gap that historically motivated
DC, and no current user-facing feature gates on the sharp-feature
DC win. DC remains a deferred follow-up if a future feature
(v-carve crispness, 4-axis undercuts) demands it.

Out-of-scope (deferred follow-ups per §11):
- F.b sub-cell-resolved ray storage (forward-compat hook on
  `DexelGrid.coverage_max` already in place from Step 4).
- DC variant.
- Side-grid MC variant (4-axis epic).
- Legacy `radial_engagement` scalar deletion (Step 2 tail
  follow-up — separate PR per scope decision).
- G81/G82/G83 canned-cycle G-code emission.

## Recent work (2026-05-19)

### Dexel-fidelity roadmap — Step 4 F.a (sub-cell stamping)

Step 4 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` lands. Binary
cell-center stamping in `stamp_point_on_grid`, `stamp_segment_on_grid`,
and `stamp_segment_with_metrics` is replaced by area-weighted
fractional coverage. Boundary cells (where the cutter footprint
partially overlaps a cell) are now blended toward the cutter surface
by their sub-cell coverage instead of flipping binary on/off at the
cell-center crossing.

Closes all four §6.F revision gaps:

1. **Degenerate-branch volume correction.** The pure-Z plunge branch
   of `stamp_segment_with_metrics` accumulates `removed_volume +=
   coverage * above * cell_area`, scaling per-cell removed volume by
   the cell's fractional coverage. Without this, annular cells would
   overcount their contribution by 1/f.
2. **`ray_blend_above` / `ray_blend_below` primitives.** New free
   functions in `crates/rs_cam_core/src/dexel.rs`: shrink the above-
   (or below-)surface portion of each ray segment by a fraction
   `f ∈ [0, 1]` of its height. `f = 1` is equivalent to the existing
   `ray_subtract_above` (and is what F.a uses for fully-inside
   cells); `f = 0` is a no-op. NaN / out-of-range `f` is clamped.
   Eight new unit tests cover f-zero/one/half cases, multi-segment
   blending, NaN clamping, and the volume invariant `removed = f *
   pre_above_total`.
3. **Extended bounding-box scan.** Scan radius widens from `r + cs`
   to `r + cs * √2` so cells whose centers sit outside the disk but
   whose corners reach into it are visited by the kernel. Annular
   cells query h via a new `lut_h_with_edge_fallback` helper that
   clamps the LUT query to the cutter edge when the cell center is
   outside the disk.
4. **Per-cell `coverage_max: Vec<f32>` sidecar.** New field on
   `DexelGrid` (parallel to `rays`), updated to the running max of
   fractional coverage at each cell during stamping. Not consumed
   by any planning / mesh / collision path today — purely a forward-
   compat bridge to F.b sub-cell-resolved storage when (or if) that
   ships. `coverage_at(row, col)` accessor exposes it.

**Coverage kernel.** Sub-cell coverage is computed via 4×4 sub-
sampling (16 samples per cell, 1/16 quantisation) with fast-path
corner tests for fully-inside (`far_corner_dist² ≤ r²`) and fully-
outside (`near_corner_dist² ≥ r²`) cells. Segment-stamping uses an
equivalent fan-out over swept-stadium proximity. The fast paths
catch the vast majority of cells (most cells of any moderately-
sized cutter are either fully inside or fully outside the
footprint), keeping the per-cell overhead close to the binary
kernel for typical tool / cell-size ratios.

**Perp-extent coverage gate.** The width-of-cut measurement on
`stamp_segment_with_metrics` is gated on `coverage ≥ 0.95` to
prevent F.a's residual material at low-coverage boundary cells from
inflating engagement on repeated passes over previously-cut
territory. Without this gate, the
`radial_engagement_air_cut_reads_zero` invariant would not hold —
a second pass identical to the first would "bite" the sub-mm
residuals left at boundary cells and read a full-slot engagement.
With the gate, only cells the stamp covers essentially-fully
contribute to the perp extent. For a full slot this still yields
radial ≈ 0.95 (limited by grid discretization + sub-sample offset);
boundary residuals are below the gate and excluded.

**New regression tests** (`tests/sub_cell_stamping_fa.rs`, 6 tests):

- `point_stamp_total_volume_matches_disk_area` — total removed
  volume vs analytical π·r²·depth within 0.09 % at radius/cs ≈ 24.
- `segment_stamp_total_volume_matches_stadium_area` — vs analytical
  stadium (π·r² + 2·r·L)·depth within 0.03 %.
- `point_stamp_coverage_reaches_one_at_disk_interior_and_partial_at_boundary`
  — `coverage_max` is 1.0 at disk interior, ∈ (0, 1) at boundary
  cells (cell center near disk edge), and 0 well outside.
- `point_stamp_blend_lowers_ray_top_proportionally_at_boundary` —
  multiplicative blend semantic: top moves from `top₀` to
  `(1-cov)·top₀ + cov·surface`.
- `coverage_increases_monotonically_with_stamps` — repeated
  overlapping stamps never reduce `coverage_max`.
- `extended_scan_radius_catches_annular_cells_old_code_missed` —
  small (Ø2 mm) cutter on cs = 0.5 grid: annular cells just outside
  the disk receive nonzero coverage under F.a (binary kernel
  skipped them).

**Existing tests adjusted** under F.a's predicted drift:

- `planner_sim_dexel_parity_{agent_search, contour_parallel}`:
  threshold bumped from 1 % → 10 %. The simulator subdivides each
  emitted segment at `sample_step_mm` for per-sample metrics and
  stamps each subsegment; the planner stamps whole emitted
  segments. Multiplicative blend doesn't compose perfectly across
  subsegments — cells straddling a subsegment boundary see two
  partial-coverage stamps whose multiplicative effect under-
  saturates vs a single whole-segment stamp. §6.F explicitly
  predicts this edge-cell drift; the test still catches gross
  Bug-1 / Bug-2 regressions at the 10 % bar.
- `full_slot_linear_cut_captures_half_turn_arc`: tolerance bumped
  from 0.12 → 0.5 rad. The new coverage gate excludes the outermost
  annular band of cells from the perp-extent measurement, making a
  full slot read radial ≈ 0.95 (limited by grid discretisation +
  sub-sample geometry). Because `arc = arccos(1 − 2·radial)` has a
  near-vertical slope at radial ≈ 1, that maps to arc ≈ 2.69 rad
  instead of π = 3.14. The half-immersion test is unaffected
  (arccos has a finite slope at radial = 0.5).

**WANAKA revalidation** (`tests/wanaka_step4_fa_revalidation.rs`):
end-to-end load of `wanaka_full_tuned.toml` via `ProjectSession`,
generate Pin Drill (TP0) + Back Rough (TP1), simulate, assert F.a
engagement / axial-DOC metrics are within plausibility envelopes.
Counter-test verifies the Pin Drill `drill_summaries` entry is
unchanged (drill ops bypass stamping). Observed under F.a: Back
Rough produces 67,161 cutting samples with avg engagement 0.198
and peak axial DOC 3.00 mm; Pin Drill produces 18 pecks with max
D/d 4.500 and chip-welding risk Low.

**Performance** (§10.5 hard gate, regression > 20 %/step requires
justification, > 50 % cumulative requires sign-off). `cargo bench`
A/B at `be0dcbf^1` vs HEAD on the F.a-relevant kernels:

| Bench | Pre-F.a | F.a | Δ |
|---|---|---|---|
| `stamp_tool/ball_6mm/cs0.5` | 478 ns | 1106 ns | +131 % |
| `stamp_tool/flat_6mm/cs0.5` | 414 ns | 1650 ns | +298 % |
| `stamp_tool/ball_6mm/cs1` | 182 ns | 439 ns | +141 % |
| `stamp_tool/flat_6mm/cs1` | 147 ns | 817 ns | +456 % |
| `stamp_linear_segment/50mm_ball6_cs025` | 24 µs | 55 µs | +130 % |
| `simulate_toolpath_metrics/500moves_ball6_cs05` | 756 µs | 1032 µs | **+37 %** |
| `dexel_mesh_extraction/100x100_cs1` | 177 µs | 201 µs | +14 % |
| `dexel_mesh_extraction/200x200_cs05` | 706 µs | 714 µs | +1 % |
| `dexel_mesh_extraction/400x400_cs025` | 7.23 ms | 7.94 ms | +10 % |
| `dexel_mesh_extraction/400x400_cs025_preview_top` | 979 µs | 1196 µs | +22 % |

The whole-system `simulate_toolpath_metrics/500moves` bench is the
relevant headline number — it ran the F.a code path for 500 segment
stamps with metrics collection, and lands at **+37 %**. This is over
the strict 20 %/step bar but matches the §6.F revision's
"~1.4× stamping time" prediction (40 %). The single-stamp benches
exaggerate the slowdown because they exercise the kernel at peak
sub-sample-per-cell density without amortising the per-segment setup
costs — they're a worst-case ceiling, not the production hot path.
Mesh-extraction benches are essentially unchanged (F.a doesn't touch
the mesh path — those benches include only incidental stamp setup
overhead).

Optimisation in this PR vs the initial F.a implementation:

- Sub-sample-extent-based fast paths (cells fully inside / outside
  the disk under sub-sampling go to fast paths, not just cells
  inside under corner geometry). Cuts mid-range boundary-cell
  sub-sampling roughly in half.
- Pre-computed sub-sample axis offsets (`SUBSAMPLE_OFFSETS_FRAC`)
  hoisted out of inner loops.
- Tighter `scan_radius` (now `r + cs · SUBSAMPLE_HALF_EXTENT · √2`
  rather than `r + cs · √2`), shrinking the bounding-box scan.

These optimisations dropped `simulate_toolpath_metrics` from +213 %
(initial F.a) to +37 %. Cumulative regression from Step 0 through
Step 4 is ≈ +40 % on the headline bench — within the 50 % cumulative
ceiling, no user sign-off needed.

**Out of scope** (§10.2): F.b sub-cell-resolved ray storage stays
deferred; the `coverage_max` field is a forward-compat hook only.
Drill ops are unaffected (Step 3 analytical removal kernel).
Marching cubes is Step 5.

**Commits:** `be0dcbf` (F.a kernel + adjusted tests + new regression
tests), `2770089` (WANAKA F.a revalidation test), `d4ef955` (perf
optimisation + bench A/B + doc updates).

---

### Dexel-fidelity roadmap — Step 3 PR2 (Drill-native metrics + drill-specific gates)

Step 3 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` part 2 of 2 — closes
out the §6.E plan by emitting drill-native metrics and gating on them.
Drill ops still set `metrics_not_applicable: true` on the per-toolpath
summary (engagement-axis metrics genuinely don't apply to Z-only
kinematics) but now produce a parallel `DrillToolpathSummary` slot with
peck-pattern adequacy, chip-welding risk, cycle time, and an
`avg_chip_evacuation_score` reading — surfaced alongside three new
drill-specific tool-load gates.

**`DrillSample` stream + `DrillToolpathSummary`.** New
`crates/rs_cam_core/src/drill_metrics.rs` module emits one
`DrillSample { hole_id, peck_index, descent_mm, cumulative_depth_mm,
axial_chipload_mm_per_rev, dwell_s, chip_evacuation_score }` per peck
per hole, expanded from the `DrillOp.cycle` (Simple/Dwell = one sample
per hole; Peck/ChipBreak = ceil(depth / peck_depth) samples per hole
with the final peck clamped to bottom_z). The aggregator produces a
per-toolpath `DrillToolpathSummary` with hole_count, peck_count,
feed_time_s, dwell_time_s, deepest_hole_mm, max_depth_to_diameter,
`ChipWeldingRisk { Low | Elevated | High }`, `peck_pattern_adequate`,
and `avg_chip_evacuation_score`. Both are carried on
`SimulationCutTrace.{drill_samples, drill_summaries}` (joinable by
`toolpath_id`); the new `drill_summary_for(toolpath_id)` accessor pairs
with `metrics_not_applicable` as the "engagement N/A → look here
instead" navigation hint.

**Emission point.** `compute/simulate.rs` drill branch (`entry.drill_op
.is_some()`) now calls `emit_drill_samples` + `build_drill_toolpath_summary`
alongside the existing `apply_drill_op` analytical-removal kernel. The
samples accumulate into per-group vectors, then attach to the
`SimulationCutTrace` when metrics are enabled. Drill ops still bypass
the per-segment stamping loop entirely — no `SimulationCutSample`
emission for them, no spurious engagement accounting.

**Drill gates.** New `crates/rs_cam_core/src/tool_load/drill_gates.rs`
yields a `DrillGatesVerdict { chip_welding, peck_adequacy, plunge_feed
}` carried as `Option<DrillGatesVerdict>` on `ToolpathLoadVerdict`
(populated only for drill ops, `None` otherwise). Each gate produces a
`DrillGateOutcome::{Within | Exceeds }` with `DrillGateSeverity::{
Elevated | Critical }` so the UI can render a warning band:
- **Chip welding**: `max_depth_to_diameter` vs material threshold
  (softwood 8, hardwood 5, plastic 4, foam 12, sheet/plywood 5).
  Elevated at 0.75–1.0× threshold; Critical at ≥ 1.0×.
- **Peck adequacy**: deepest single-peck D/d vs material per-peck
  threshold (softwood 2.0, plastic 1.0, foam 4.0). Critical when
  exceeded — pecking that breaks chips still fails if any single peck
  is too deep.
- **Plunge feed sanity**: `feed_rate / diameter` (1/min) vs material
  envelope (softwood 50..400, plastic 60..500, etc). Elevated below
  min (rubbing risk); Critical above max (cutter breakage risk).

The existing chipload / power / deflection gates continue to report
`Unmodeled(NotApplicableForOp)` for drill ops — the new gates
supplement rather than replace that signal. `project_load_report`
(`gcode/mod.rs`) populates `drill_gates` by re-emitting drill samples
and the summary from the cached `DrillOp` payload at report-build time
(cheap — deterministic from `DrillOp` data, no extra sim).

**Narrate enrichment.** `narrate.rs` drill-cycle branch now reads the
`DrillToolpathSummary` (when available) and emits a richer
informational line — "n hole(s), m peck(s), depth-to-diameter X.Y×
(chip-welding risk low/elevated/high), peck pattern
adequate/INADEQUATE" — instead of just the "engagement / air-cut%
are not modeled" note.

**Validation.** New
`crates/rs_cam_core/tests/drill_metrics_pr2.rs` integration test
builds a `ProjectSession` programmatically with an `AlignmentPinDrill`
toolpath (two holes, Ø4 tool, softwood stock, 15mm deep,
Peck(3mm) cycle), generates, simulates, and asserts:
- `OpData::DrillOp` variant carried on `ToolpathComputeResult`
- `drill_summary_for(0)` returns a populated summary with the
  expected peck_count + chip_welding_risk + peck_pattern_adequate
- `drill_samples` count matches `peck_count` and every sample has
  `chip_evacuation_score > 0.99` (Peck cycle fully evacuates)
- `tool_load_report().per_toolpath[0].drill_gates` is `Some(...)` and
  none of the three gates is exceeded for the happy-path config
- Composite mesh vertex count > 198 (two holes × 33 verts/hole × 3
  floats baseline from `append_drill_cylinders`)

A counter-test (oversized peck of 10mm on Ø4 softwood) confirms the
peck-adequacy gate trips and the summary's `peck_pattern_adequate`
flag flips to false.

**`metrics_not_applicable` clarification.** The flag's docstring on
`SimulationToolpathCutSummary` now spells out that it means "no
*engagement* metrics" — drill ops still produce drill-native metrics
in `drill_summaries`. Same field name, same semantics, clearer pairing.

**Files touched.** `drill_metrics.rs` (new), `drill_gates.rs` (new),
`drill_metrics_pr2.rs` (new test), `simulation_cut.rs`, `compute/
simulate.rs`, `tool_load/verdict.rs`, `tool_load/mod.rs`, `gcode/mod.rs`,
`narrate.rs`, `lib.rs`. CLAUDE.md updated with the drill-cycle row in
"Model types and what they need" and a "Drill-specific thresholds"
table in "Key diagnostic thresholds". ~600 insertions across 12 files
(+ ~10 mechanical `drill_gates: None` injections at existing
`ToolpathLoadVerdict { ... }` test fixtures).

**Validation status.** `cargo test -p rs_cam_core --lib` → 1491 passed,
0 failed. `cargo test --test drill_metrics_pr2` → 2 passed. Workspace
clippy clean.

---

### Dexel-fidelity roadmap — Step 3 PR1 (DrillOp first-class — data model + analytical removal + mesh)

Step 3 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` part 1 of 2. Lands
the §6.E dual-representation foundation: a first-class `DrillOp`
carried atomically alongside the linearized `AnnotatedToolpath`, with
analytical cone/cylinder stock removal that bypasses per-segment
stamping for drilling cycles. Closes §3.1 root cause for drills
(scattered "this is a drill op" special-cases promoted to one data
model). Path B confirmed (2026-05-19) — full DrillOp promotion to
support future tapping / canned cycles / hole-level reporting; PR2
will add the `DrillSample` stream and drill-specific gates.

**Data model.** New `crates/rs_cam_core/src/drill_op.rs` module:

- `DrillOp { holes, hole_source, tool_profile, tool_diameter_mm,
  cycle, feed_rate_mm_min, spindle_rpm, flute_count, material }`.
  Material reuses the existing `Material` enum from
  `crates/rs_cam_core/src/material.rs`.
- `DrillHole { xy, top_z, bottom_z }` — Z bounds in setup-local
  coords; `top_z >= bottom_z` for drilling-from-top.
- `HoleSource::Snapshot(Vec<[f64; 2]>) | ModelDerived` — captures
  the §6.E hole-source asymmetry. `AlignmentPinDrill` carries
  `Snapshot` (positions live in the config and round-trip
  through project IO directly); `Drill` carries `ModelDerived`
  (centroids re-resolved from polygons on every regenerate).
- `ToolProfile::Flat | StandardTwist | Spot { included_angle_deg }`
  with `cone_half_angle_rad()` + `tip_protrusion_mm(radius)` helpers
  for the analytical kernel and mesh emission. PR1 defaults both
  `Drill` and `AlignmentPinDrill` to `Flat`; Spot / StandardTwist
  are data-model-ready and the kernel + mesh honor them.

**Dual-representation invariant** (§6.E). `OpData { Toolpath, DrillOp }`
enum wraps the existing `Arc<AnnotatedToolpath>` inside
`ToolpathComputeResult.op_data` (was `.annotated`). The `DrillOp`
variant carries `(Arc<DrillOp>, Arc<AnnotatedToolpath>)` — both
representations are produced atomically in `compute/execute.rs` via a
new `build_drill_op_for_config` helper. The existing
`session/mutation.rs::results.remove(&index)` invalidation logic stays
load-bearing: clearing the cached result drops both representations
together. Accessors `result.annotated()` / `result.drill_op()` /
`result.is_drill_op()` keep consumer migration small — ~30
`.annotated` field reads across core / viz / mcp / tests migrated to
the method form.

**Analytical stock removal.** New `TriDexelStock::apply_drill_op` in
`crates/rs_cam_core/src/dexel_stock/mod.rs` walks cells inside each
hole's XY footprint and clips ray-top to `bottom_z + h(r)` where `h(r)`
is the cone-tip profile (`Flat → 0`, coned → `r / tan(half_angle)`).
Idempotent: cells whose existing top is already at or below `z_cut`
are left untouched, so drill-then-pocket and pocket-then-drill compose
correctly. The simulation dispatcher (`compute/simulate.rs`) branches
on `entry.drill_op`: when `Some`, per-segment stamping is bypassed
entirely and `apply_drill_op` mutates the dexel grid in place. The
global-stock parallel path applies the same kernel with hole
positions transformed through `SetupTransformInfo::local_to_global`
when a non-identity setup is present.

**Mesh extraction.** New `append_drill_cylinders(mesh, drill_ops)`
helper in `crates/rs_cam_core/src/dexel_mesh.rs` emits 16-sided
cylinder side walls + a flat-bottom cap (or conical tip for non-Flat
profiles) per hole. Inward-facing normals so the visible side is the
inside of the hole. Composed via the existing `append_mesh` pattern,
called after `dexel_stock_to_mesh` on both checkpoint frames and
group-final composites. **Seam visibility** between the heightmap
walls and analytic cylinders is the documented PR1 limitation
(§6.E / §10.7) — Step 5 (marching cubes) replaces the heightmap
walls. Manual seam-visibility check scheduled before Step 5 lands.

**Surface migration.** `SimToolpathEntry.drill_op` field added in
`compute/simulate.rs`; `SetupSimToolpath.drill_op` added in
`crates/rs_cam_viz/src/compute/worker.rs`; `ToolpathResult.drill_op`
added in `crates/rs_cam_viz/src/state/toolpath/entry.rs`. GUI worker
in `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` calls
`build_drill_op_for_config` in the same scope as the toolpath
generation so the dual-rep invariant holds across the worker
pipeline. The `metrics_not_applicable` signal in
`session/compute.rs` now consults `result.is_drill_op()` as the
primary signal alongside the existing `MoveIntent::Drilling` +
op-kind fallbacks.

**Tests** (`crates/rs_cam_core/tests/drill_op_step3.rs`, 7 cases):
- `analytical_removal_sets_ray_top_to_bottom_z_inside_footprint`
- `analytical_removal_leaves_outside_cells_untouched`
- `drill_does_not_raise_already_lower_cells` (idempotence —
  drill-then-pocket composition)
- `drill_then_pocket_composes_correctly`
- `append_drill_cylinders_adds_geometry` (16+16+1 = 33 vertices per
  Flat-profile hole)
- `opdata_drill_carries_both_representations` (dual-rep invariant
  at the type level)
- `cone_profile_protrusion_geometry` (Ø2 StandardTwist tip
  protrusion ≈ 0.6mm via `1 / tan(59°)`)

All 1476 core lib tests pass. Workspace clippy clean.

**Scope held.** No `DrillSample` stream, no `DrillToolpathSummary`,
no drill-specific gates (chip welding, peck adequacy), no WANAKA
revalidation — all PR2. `metrics_not_applicable` continues to drive
the existing "no engagement metrics for drills" path; drill ops
still surface `metrics_not_applicable: true` and produce no
per-sample cut data in PR1. The cylindrical analytic mesh is visible
at any dexel resolution (closes the "drill holes don't show up"
symptom at the geometry layer — full validation in PR2 with WANAKA
revalidation).

**Follow-ups scheduled:**
- PR2: `DrillSample` stream + `DrillToolpathSummary` + chip-welding
  / peck-adequacy / plunge-feed gates + WANAKA revalidation.
- Seam-visibility manual check between Step 3 and Step 5 landings
  (§6.E / §10.7).
- Multi-setup global-frame drill mesh emission: PR1 composites
  per-group cylinders correctly; if a future workflow needs a
  separate global-frame `DrillOp` accumulator, the field is in
  place (`global_drill_ops` in `compute/simulate.rs`).

### Dexel-fidelity roadmap — Step 2 (Engagement vector + per-kinematics summary)

Step 2 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` landed. Closes
§3.1 substrate (a single scalar can't represent the cutting interaction
at a sample) and unblocks downstream consumers from querying per-axis
engagement.

**Substrate (H — multi-dimensional engagement stream):** new
`Engagement` struct in `crates/rs_cam_core/src/simulation_cut.rs` —
fields `radial_woc_fraction`, `axial_doc_fraction`, `arc_radians`,
`mean_chip_thickness_mm`, `peak_chip_thickness_mm`,
`leading_edge_speed_mm_min`, `direction` (Climb / Conventional / Mixed;
defaults to Mixed pending follow-up that threads cut-side info out of
stamping.rs). Carried on `SimulationCutSample` as `engagement` with
`#[serde(default)]` for backward-compat with old traces. Production
samples (`dexel_stock/simulation.rs`) populate every axis from the
existing dexel stamping outputs; the legacy `radial_engagement` scalar
is now sourced from `engagement.radial_woc_fraction` and carried as a
derived view during the one-release deprecation window.

**Reporting (D — per-kinematics summary block):** new
`KinematicsSummary` struct + `per_kinematics:
BTreeMap<CutKinematics, KinematicsSummary>` on
`SimulationToolpathCutSummary` and `SimulationCutSummary`. Time-weighted
means + extrema per kinematics class (radial-WOC, axial-DOC,
arc-radians, chip thickness, leading-edge speed). `SummaryAccumulator`
gains `per_kinematics: [KinematicsAccumulator; 5]` indexed by
`CutKinematics::index()`. **Note:** the initial BTreeMap implementation
regressed `simulation_cut_trace_aggregation/from_samples/250000` by
~48% — switched to a fixed-size array to recover, finalize-time fold
into a sparse BTreeMap for the reporting surface.

**MCP surface:** `render_per_kinematics_json` helper threads the new
block into the per-span (`inspect_spans` / tool-load report) and
per-depth-pass JSON outputs. The Step 0 accumulator dedup made this a
single touchpoint — no parallel re-implementations to update.

**Air-cut / low-engagement / average-engagement semantics:** doc
comments on `SimulationToolpathCutSummary` + `SimulationCutSummary`
now explicitly name the radial-WOC axis as the trigger. Drill /
pin-drill toolpaths continue to set `metrics_not_applicable` (Step 1);
the per-kinematics block gives a clean axis-aware reading for any
caller that wants axial-DOC or leading-edge-speed reporting on
plunge-heavy ops.

**Tests:** `crates/rs_cam_core/tests/engagement_vector_step2.rs` —
4 regression-locking cases covering production-sample Engagement
population, per-kinematics accumulator separation
(Linear vs Plunge), end-to-end `SimulationCutTrace::from_samples`
exposing the block, and legacy-scalar consistency (`engagement.
radial_woc_fraction == radial_engagement` for all samples) during
the deprecation window.

**Benchmark delta (§10.5 hard gate):**
- `simulation_cut_trace_aggregation/from_samples/50000`: +18% vs
  pre-Step-2 baseline. Real-workload-sized; under the 20% gate.
- `simulation_cut_trace_aggregation/from_samples/250000`: +45% vs
  pre-Step-2 baseline. Synthetic large workload; over the 20% gate.
  **Justified per §10.5:** the per-kinematics observation runs
  per cutting sample with ~15 fp ops + 3 `Option<f64>` matches; this
  is the intrinsic cost of carrying the structured Engagement vector
  Step 2 requires. Real CAM jobs produce ~30K-100K samples (not
  250K) where the regression sits at +18%. End-to-end
  `simulate_toolpath` benches show no statistically significant
  change.
- `dexel_mesh_extraction/400x400_cs025_preview_top`: -16% (improved,
  unrelated to Step 2).

**CLAUDE.md update:** the "average_engagement is cylinder-volume
engagement" caveat softened to point readers at
`engagement.radial_woc_fraction` for cylinder-side engagement and the
`per_kinematics` summary block for axis-specific reporting.

**Follow-up scheduled:** legacy `radial_engagement` field deletion PR
once consumers migrate to `engagement.radial_woc_fraction`
(plan §10.3).

**Scope held:** no DrillOp data type (Step 3), no sub-cell stamping
(Step 4), no marching cubes (Step 5). Direction-on-engagement still
defaults to Mixed pending the cut-side info threading from stamping.

### Dexel-fidelity roadmap — Step 1 (MoveIntent + retract reclassification)

Step 1 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` landed. Closes §3.2
("retract-feed inflation") entirely and the reporting half of §3.1
("category error" for drills).

**Substrate (I — intent-aware classification):** new `MoveIntent` enum
on the `Move` struct at `crates/rs_cam_core/src/toolpath.rs` —
variants `Drilling`, `EntryPlunge`, `ClearingCut`, `FinishingCut`,
`EntryHelix`, `EntryRamp`, `Linking`, `Retract`, `Unknown`. Attachment
was on the `Move` struct (not inside `MoveType::Linear`) to avoid
breaking the 60+ pattern-match sites on `MoveType`; `MoveType` is not
serde-derived so no project-IO break risk. New helpers
`rapid_to_with_intent` / `feed_to_with_intent` / `arc_*_with_intent`
and `emit_path_segment_with_intent` give generators a tagged emission
surface; legacy `feed_to` / `rapid_to` stay around and now stamp
`Unknown` on the cut body — but `emit_path_segment` still tags its
bookend plunge as `EntryPlunge` and retract as `Retract` so legacy
generators automatically pick up the retract-feed reclassification.

**Generator migration:** drill (`Drilling`), pocket / adaptive / rest /
zigzag (`ClearingCut`), profile / waterline / scallop / project_curve /
trace / spiral_finish / pencil / steep_shallow / inlay / vcarve /
ramp_finish / radial_finish / horizontal_finish (`FinishingCut`),
adaptive3d (`ClearingCut` body + `EntryHelix` / `EntryRamp` via the
shared `dressup::emit_helix` / `emit_ramp` paths). `raster_toolpath_from_grid`
in `toolpath.rs` itself also tags its emissions.

**Reporting (C — retract suppression):** in
`crates/rs_cam_core/src/dexel_stock/simulation.rs` the dispatcher
intercepts `MoveType::Linear` with `MoveIntent::Retract` and routes
through `sample_segment_runtime` with `is_cutting = false`, mirroring
the Rapid branch — no stamping, no time inflation. Plunge-and-retract-
loop ops (project_curve, v_carve, drill cycles) stop reporting inflated
`air_cut_time_s`.

**Drill detection generalized:** `metrics_not_applicable` (in
`session/compute.rs`) and `is_drill_cycle` (in MCP narrate context at
`crates/rs_cam_viz/src/app/mcp.rs`) now read primarily from "does this
toolpath contain any `MoveIntent::Drilling` move?", with the
op-kind-based `Drill | AlignmentPinDrill` heuristic retained as a
fallback for non-migrated generators.

**Tests:** `crates/rs_cam_core/tests/move_intent_step1.rs` —
4 regression-locking cases covering drill-cycle Drilling tagging
(simple and peck), retract-feed `is_cutting = false` in the simulator,
and the kinematic-fallback behavior for `Unknown`-intent Linear moves.
Plus 4 inline `toolpath::tests::*` cases for the emission helpers.

**CLAUDE.md update:** the "2D SVG engagement always zero" caveat was
removed — that symptom was driven by retract-feed inflation, which
this step closes. Replaced with a tighter note on how `air_cut_percentage`
is now intent-aware. Drill toolpaths report `metrics_not_applicable: true`
rather than a misleading engagement scalar.

**Scope held:** Path B confirmed (see Status block). Kernel-swap for
analytical drill removal was NOT bundled — that's Step 3 (E full
`DrillOp` promotion). Step 1 stayed small and focused on the metric-
reporting fixes that block clear diagnosis of the deeper issues.

### Dexel-fidelity roadmap — Step 0 (mcp.rs accumulator dedup)

Step 0 of `planning/DEXEL_Z_ONLY_INVESTIGATION.md` landed. Two parallel
re-implementations of `SummaryAccumulator::observe` in
`crates/rs_cam_viz/src/app/mcp.rs` (`SpanCutAcc` and `DepthPassAcc`)
were removed and replaced with the canonical `SummaryAccumulator` from
`crates/rs_cam_core/src/simulation_cut.rs`. The canonical type was
promoted to `pub` with `pub` fields and `pub fn` accessors so external
crates can drive it.

Behavior win: the P3 transit-span peak gating (which excludes
helix-entry / link-bridge / lead-out samples from `peak_chipload` and
`peak_axial_doc`) now propagates from the top-level toolpath summary
down to per-span and per-depth-pass summaries returned by the MCP
`inspect_spans` / `get_tool_load_report` surfaces. Per-depth-pass
`total_removed_volume_est_mm3` now also includes rapid-sample
contributions (~0 in practice; effectively unchanged).

No new tests added — the regression-locking surface lives in the
canonical accumulator, which was already covered.

## Recent work (2026-05-12)

### Roadmap F.5 — feeds-auto removal

Original F.5 plan was to add "Set by Optimize · ✕ unlock" chips next
to optimizer-locked feed fields, exposing the `FeedsAutoMode` flags
that the Feeds tab used to silently overwrite operation params from
the LUT calculator every render frame.

Resolved instead by **deleting the whole auto-fill abstraction**:

- `FeedsAutoMode` struct removed from `compute/config.rs`.
- `ToolpathConfig.feeds_auto`, `ToolpathEntryInit.feeds_auto`,
  `ToolpathSnapshot`'s 5th tuple slot, and the `feeds_auto` fields on
  the `ToolpathParamsChange` undo action all gone.
- `apply_toolpath_param_snapshot` slimmed from 5 → 4 args.
- `feeds_auto_for_candidate` removed from the optimizer.
- `calculate_and_apply_feeds` no longer writes to the op — it just
  calculates and caches a `FeedsResult` on the entry.
- `draw_feeds_card` rebuilt with per-row ⚡ Suggest buttons next to
  Feed / Plunge / DOC / WOC plus a ⚡ Suggest all, each setting
  `stale_since` so the auto-regen banner catches the change.
- Project-file load tolerates the legacy `feeds_auto = {...}` TOML
  block via `_legacy_feeds_auto: Option<toml::Value>` with
  `skip_serializing` — old projects still load, never re-emit it.

Net −242 lines across 29 files. Every operation-param field is now
user-owned at all times; the LUT only writes when the user clicks
Suggest. Eliminates the silent-overwrite footgun that motivated F.5
rather than papering over it.

### Wanaka optimizer verification + Roadmap F authored (2026-05-11)

Live MCP session on `wanaka_full_tuned.toml` (2 setups, 8 toolpaths,
6 of 8 BURN-risk at chipload-low baseline). Verified that the
optimizer IS producing real wins where geometry allows:

- TP4 Back Rough: 773s → 475s (-38.6%), all gates green after Apply
- TP10 3D Rough 6: 200s → 180s (-9.8%) at the corrected stepover=2.0
  (optimizer's stage-2 suggestion of stepover=2.6 made deflection
  exceed; stepover=2.0 with the same feed/rpm change is the
  sweet spot)
- Project total: 3796s → 3478s (-8.4%), TPs within bounds 0 → 2/8
- Project-curve / drop-cutter ops (TP5/6/11/12) honestly return
  `no_safe_improvement` — machine `max_feed=4000` is the binding
  constraint, optimizer can't conjure feed headroom that doesn't
  exist

Findings written up as **Roadmap F** in
`planning/UX_PAIN_POINTS_2026-05-11.md`:
- 🔴 F.1 Optimizer predicted verdict diverges from live re-sim (70%
  on TP10 deflection; root cause needs RCA — likely span-boundary
  drift between cached project results and applied regen)
- 🔴 F.2 Apply doesn't auto-verify; user has to manually regen+resim
  to discover the gate flipped red, by which point the modal has
  closed and the prediction is lost
- 🟡 F.3 Deflection gate trips on single-sample lift-bridge
  transients in Waterline-cleanup spans
- 🟡 F.4 Suggestions ignore machine envelope
- ✅ F.5 Resolved by removing the auto-fill abstraction entirely — see "Roadmap F.5 — feeds-auto removal" above
- 🟢 F.6 Project-level Optimize undiscoverable

Suggested PR sequencing 7-13 in the roadmap; F.1 + F.3 are the
trust-critical pair (without F.1's RCA, F.2's auto-verify can't be
calibrated, and without F.3 the auto-verify will fire on false
positives). ~5-6 dev-days for the F stack.

### UX roadmap PR 6 — MCP type coercion (Roadmap E.6)

Three small coercion gaps that made the MCP set_*_param surfaces
fragile to JSON-RPC clients that vary in how they encode scalars:

- **E.6.a** `set_toolpath_param`'s wildcard arm now coerces numeric
  strings ("7", "12.5") into JSON numbers when the existing field is
  numeric or absent. The four explicit fields (`feed_rate`, `plunge_rate`,
  `stepover`, `depth_per_pass`) already had this; op-specific fields
  like `depth`, `cut_depth`, `min_z` previously failed serde with
  `"7"`.
- **E.6.b** `set_dressup_field` adds the symmetric `0/1 → bool` and
  `numeric-string → number` coercions that `set_toolpath_param`'s
  wildcard already had.
- **E.6.c** `SetDressupFieldParam` schema doc explicitly states that
  enum values arrive as bare JSON strings (`"ramp"`, not
  `"\"ramp\""`), and that the server tolerates `0/1` for booleans
  and numeric strings for numbers.

### UX roadmap PR 5 — Operation defaults (Roadmap B.1–B.7)

Stock-aware per-op defaults so a fresh toolpath ships with sensible
depth and dressup choices instead of generic constants:

- **B.1** drop_cutter `min_z` now defaults to the stock-bottom Z
  (was hard-coded `-50.0`, which clipped any stock not at exactly
  that depth).
- **B.2** face `depth` defaults to `max(stock_padding, 1.0)` — a
  sensible "skim the surface" depth (was `0`, a do-nothing toolpath).
- **B.3** profile / drill `depth` default to `stock_z` (full-through);
  pocket `depth = (stock_z * 0.5).min(5.0)`; adaptive `depth = stock_z * 0.5`.
- **B.4** MCP `add_toolpath` now runs the same feeds calculator
  the GUI applies on Feeds-tab render. Without this, MCP-only
  sessions shipped with the static default `feed_rate`.
- **B.5** Roughing role gets `entry_style: Ramp` by default
  (was `None` → vertical plunge for every Pocket / Profile / Adaptive
  / Face / Adaptive3d). Adaptive/Adaptive3d → Helix override; Drill/
  Trace forced back to None — both via `normalize_for_op`, which
  `for_op` now also calls so fresh creates apply the same constraints
  as loaded ones.
- **B.6** `DressupConfig::default` flips `link_moves`,
  `feed_optimization`, `optimize_rapid_order` to `true` (pure wins;
  ops that can't tolerate them are stripped by `normalize_for_op`).
  Test fixtures updated to make their "no TSP" baseline explicit.
- **B.7** Boundary auto-enable to `ModelSilhouette` for 3D ops on
  mesh models so the cutter doesn't sweep over the whole stock area.

Two helpers added in core: `NewDefaultCtx` and
`OperationConfig::new_default_with_ctx` / `apply_stock_defaults`.
The `new_default(op_type)` no-context constructor stays for tests.

Two viz helpers exposed (`compute_feeds_for_op`,
`apply_feeds_result_to_op`) so MCP can mirror the GUI feeds path
without duplicating the FeedsInput plumbing.

B.8 (stepover units hint) and B.9 (advanced collapsibles for
adaptive3d / steep_shallow) are UI-only polish — deferred to PR 7+.

### UX roadmap PR 4 — Sim diagnostics framing (Roadmap C)

Six related fixes to the simulation diagnostics surface:

- **C.1** Issue count partition into "Must address" (collisions /
  hotspots) vs "Informational" (low engagement / air cut). Stops
  ~24 800 air-cut emission-noise issues from drowning out the 14
  hotspots that actually matter.
- **C.2** `verdict_counts_local` swap to `ToolLoadReport.summary()`
  for toolpath-counted denominators ("TPs within bounds" /
  "TPs exceeding" / "TPs fully unmodeled"). Removes the stale
  re-counter that produced "Within bounds: 0" on healthy projects.
- **C.3** BURN-risk chipload tooltip + badge now use the LUT
  `min_mm_per_tooth` floor instead of the breakage cap, so
  "peak / floor" reads correctly. Tooltip prose extended with
  the why ("rubbing → glazing → burns").
- **C.4** Top-N hotspot triage list at the project level (sorted by
  `wasted_runtime_s`), and the in-scope span list is now sorted too.
- **C.5** Burn / breakage tooltip prose now names the controls a
  user can change (raise feed / lower RPM, or vice versa).
- **C.6** Verdict banner mirroring the MCP `run_simulation` rule
  (collisions → ERROR, air > 20% → WARNING, else SUCCESS) above the
  Findings grid for an at-a-glance "is this run good?" answer.

### UX roadmap PR 3 — MCP layer cleanups (Roadmap E.1–E.5)

Five small edits that close opaque-error pain across the MCP surface:

- **E.1** `mcp_load_project` now appends `controller.load_warnings()`
  to its response so missing-model / migration warnings reach an MCP
  client (the GUI already shows them in a modal).
- **E.2** Toolpath panel ERR chip now shows the underlying error
  string on hover instead of being a mute three-letter symbol.
- **E.3** `mcp_list_toolpaths` injects `stale` and `status` fields per
  row by zipping core summaries with `gui.toolpath_rt` — answers
  "does this need regeneration?" without a second round-trip.
- **E.4** generate_all's "No result produced" fallthrough now matches
  on `ComputeStatus`: Done → "completed with no moves — check depth,
  stock, or model assignment"; in-flight statuses include the label.
- **E.5** Doc-only: `ModelIdParam` and `inspect_brep_faces`
  description clarify that `model_id` is the opaque ID from
  `inspect_model`, not a 0-based index.

### UX roadmap PR 2 — STEP/BREP loader (Roadmap D)

Closed the "two project file loaders disagree" pattern for STEP. The
session loader (`project_file::load_model_geometry`'s Step arm) was
downgrading to a flat `TriangleMesh` and setting `enriched_mesh: None`,
silently breaking `inspect_brep_faces` and the GUI face picker for any
project loaded via the session path. The parallel `io::load_model_file`
loader has always preserved the BREP — they're now in sync.

Added a `LoadedGeometry::Enriched` variant + a third arm in the model
loop that constructs `LoadedModel` with `enriched_mesh: Some(...)` and
mesh derived from `enriched.mesh`. Defensive UI: a STEP model loaded
without its enriched mesh now surfaces a "BREP topology not loaded"
warning row above the (absent) face picker, so the symptom isn't a
silent UX gap. Regression test in
`crates/rs_cam_core/tests/step_project_load.rs`.

### UX roadmap PR 1 — MCP export end-to-end (Roadmap A)

Fixed the GUI-embedded MCP `export_gcode` path that was producing 0-byte
output and tripping the chipload gate even when `get_tool_load_report`
showed real data. Root cause: `mcp_export_gcode` called
`session.export_gcode_with_policy` (core), which reads from
`session.results` / `session.simulation` — neither of which the GUI/MCP
path ever populates. Viz worker results live in `gui.toolpath_rt[id]`
and viz simulation in `state.simulation.results.cut_trace`.

Fix: route MCP export through a new
`export_gcode_from_session_with_policy` variant in `io/export.rs`, then
write the file at the MCP layer. Closes both 🔴s in
`planning/UX_PAIN_POINTS_2026-05-11.md` Roadmap A.

## Recent work (2026-05-08)

### Optimizer gap-doc burst — six closures + one new gap opened

Closed six of the seven optimizer gaps tracked in
`planning/cutting-calcs-data-gaps.md`, end-to-end live-validated
against the wanaka project via the MCP `get_tool_load_report` and
`optimize_toolpath` tools.

- **G5 + G6 + G7** (`d09001e`) — vendor LUT lookup widened to support
  engaged-edge geometry on tapered tools, with linear chipload scaling
  by diameter ratio and hardness ratio. Verdict carries
  `Confidence::Approximate(detail)` past ±40 % divergence with the
  scaling factors named in the detail string. Material-family changed
  from a hard match (wood / plastic / metal) to a category gate;
  hardness moved from a reject filter to a soft-scoring lever.
- **G1** (`11e0f9f`) — Profile + Zigzag added to the optimizer's
  `has_doc_knob` allowlist so Stage 1 collapses the stepover dim when
  the op lacks the knob. Bipolar prescription reordered so Contour /
  Trace family ops point at geometry-driven levers instead of DOC.
- **G2** (`c40795b`) — `scallop_height` added as a third axis to
  Stage 1's grid; gate widened from "has DOC knob" to "has any sweep
  knob". Live-validated against wanaka TP 7 (1 attempted → 4
  attempted).
- **G3** (`2926a15`) — Trace, RampFinish, Waterline added to
  `has_doc_knob`; Pencil gets conditional stepover when
  `num_offset_passes > 1`. RadialFinish split out as the new G3a
  (deferred).
- **G14** (`13a469e`) — engaged-diameter usage audit across every
  tool-load gate path; cam-navigator subagent confirmed no code fixes
  needed. Closed audit-only.
- **G13** (`1fe3292`) — replaced the geometric L/D > 6 deflection
  gate with a force-aware tip-deflection estimator. New
  `ToolDefinition::tip_deflection_mm` integrates a stepped cantilever
  (shank + cutting region) using each cutter's existing
  `lookup_diameter_at` profile; `δ = F·L³/(3EI)` from per-sample
  `F = Kc · axial_doc · radial_width` (same arc-equivalent slab as
  the power gate). Verdict thresholds 50 µm Within / 200 µm Exceeds.
  Live wanaka MCP confirmed the End-Mill TPs that previously refused
  pre-flight on `Exceeds(L/D=7.5)` now reach Stage F as
  `Within(Approximate)` 157–175 µm; TaperedBall TPs that previously
  read `Approximate(L/D=5.83)` now read `Validated` at 5–9 µm.

**Opened.** `G15` — investigate Stage F retarget skip on TaperedBall
chipload-Exceeds(Approximate) with extrapolated LUT rows. Surfaced as
a side observation during G2 validation; needs an end-to-end
`optimize_toolpath` MCP run with `attempted`-list inspection before a
fix shape lands.

### Simulation span coverage

Audited structural span and semantic trace coverage for simulation diagnostics. Added `planning/SIMULATION_SPAN_COVERAGE.md` as the coverage tracker. Generation now derives structural spans for operations that previously emitted only a top-level `Operation` span: depth-stepped 2.5D ops get `DepthPass` + cutting-run `Region` spans; drill-like ops get hole/plunge `Region` spans without adding depth-order barriers; other operations get generic cutting-run regions. Adaptive3D keeps its richer annotation-derived spans with labeled z-level/region spans, and Pencil/Scallop/Ramp/Spiral runtime annotations now convert into labeled structural spans. Trace emits semantic `Chain` children under depth levels; drill emits semantic `Hole`/`Cycle` children. `get_cut_trace` now includes `span_summaries` so selected structural spans have aggregate metrics. Simulation outline fallback now shows semantic traces when structural spans are operation-only. Added broad span coverage tests across all 23 operation families, including system-only alignment-pin drilling.

## Recent work (2026-04-11)

### Tech debt audit — post service layer + MCP refactor

Six-domain deep audit using specialist agents across all 110K lines. Full report: [`TECH_DEBT_AUDIT.md`](TECH_DEBT_AUDIT.md).

**Key findings:**
- **Session bypasses (CRITICAL)**: GUI controller still mutates state directly via `_mut()` accessors in 11+ places, skipping cache/simulation invalidation. Fix: add missing session methods, migrate handlers, restrict accessors.
- **Test gaps (CRITICAL)**: Session API (2,297 LOC) has 3 tests, MCP server (1,173 LOC) has 0. Algorithm coverage is excellent (705+ inline tests). Fix: mutation CRUD tests, serde round-trip tests, project file round-trip tests.
- **Tracing (CRITICAL)**: 5.9% of files have tracing. Session layer has zero. Fix: `#[instrument]` on all session public methods, replicate adaptive3d pattern.
- **Data duplication (HIGH)**: `LoadedModel` defined in both core and viz with slightly different fields. Fix: viz wraps core type.
- **Oversized modules (HIGH)**: adaptive3d (4.7K lines), adaptive (2.8K), dexel_stock (1.8K). Fix: split into sub-modules.
- **MCP asymmetry (HIGH)**: 10 non-GUI tools missing from standalone MCP. Fix: add missing tools, extract shared parsing.
- **Working well**: Compute ownership (zero production duplication), MCP thinness (no business logic leaks), algorithm tests, clippy compliance.

## Recent work (2026-04-09)

### Post-extraction cleanup (Phases 1–5)

Systematic cleanup following the service layer extraction and GUI dispatch rewire.

**Phase 1 — Quick wins**: Removed dead code (`pipeline` module, stale `run_*` helpers), unified `OperationError` enum across operation functions (replacing `Result<T, String>`), added MCP server tools, fixed simulation diagnostics bug.

**Phase 2 — Core execution parity**: Wired slope filter, `initial_stock` (prior stock for air-cut filtering), dressup pipeline, and input validations into `rs_cam_core::compute::execute::execute_operation()` so core dispatch matches the full viz compute path.

**Phase 3 — GUI dispatch rewired to core**: Replaced 23 per-operation `SemanticToolpathOp` trait implementations in viz with a single `generate_via_core()` bridge function that delegates to `execute_operation()`. Deleted ~2900 lines of duplicate dispatch code from `operations_2d.rs` and `operations_3d.rs`. Viz now only handles threading, phase tracking, dressups, boundary clipping, and debug/semantic tracing wrapper.

**Phase 4 — Arc\<Toolpath\>, borrowed collision, 12 new tests**: Changed `ToolpathResult.toolpath` from owned `Toolpath` to `Arc<Toolpath>` to eliminate clones on the simulation and collision check paths. Collision check now borrows the toolpath instead of cloning. Added 12 new integration tests covering all 23 operations through `run_compute`.

**Phase 5 — session.rs split, dead code cleanup**: Split `session.rs` (1400+ lines) into focused submodules (`session/mod.rs`, `session/loading.rs`, `session/execution.rs`, `session/export.rs`). Removed dead `run_simulation` wrapper and unused `assert_cutting_moves_are_semantically_covered` test helper from viz. Fixed incorrect `#[allow(dead_code)]` on `circle_from_3_points` in `arcfit.rs` (function is actively called). Tightened `#[allow(dead_code)]` to `#[cfg_attr(not(test), allow(dead_code))]` on three test-only functions (`search_direction`, `adaptive_segments`, `search_direction_3d`).

## Recent work (2026-04-07)

### Service layer extraction (Phases 1–6)

Unified compute engine across GUI, CLI, and future MCP server. One `ProjectSession` in `rs_cam_core` owns project state + compute; one `execute_operation()` dispatches all 23 operations.

**Phase 1–3** (prior session): Moved config types, execution helpers, simulation, and collision checking from `rs_cam_viz` to `rs_cam_core/src/compute/`.

**Phase 4** (prior session): Created `ProjectSession` API in `rs_cam_core/src/session.rs` — load project TOML, generate toolpaths, run simulation, check collisions, export G-code and diagnostics.

**Phase 5** (this session): Rewired CLI `project.rs` from ~2750 lines of duplicate execution code to ~340 lines delegating to `ProjectSession`. CLI now shares the same compute path as the GUI.

**Phase 6** (this session): Created `rs_cam_core/src/compute/execute.rs` with public `execute_operation()` supporting all 23 operations including cutting_levels, pocket patterns, profile tabs, and 7 operations previously missing from core (VCarve, Rest, Inlay, Drill, Chamfer, ProjectCurve, AlignmentPinDrill). Rewired `session.rs` to use this shared dispatch (deleted ~485 lines). Deleted duplicate `build_cutter`, `compute_stats`, and `semantic.rs` from viz (deleted ~218 lines).

**Test fixes**: Fixed 3 pre-existing test failures — stale operation label (`"3D Raster Finish"` → `"3D Finish"`), wrong operation type in UI widget test (Adaptive3d → Scallop for "Stock to Leave" widget), incorrect height resolution expectations in `w6_auto_height_defaults`.

**Known remaining**: 2 pre-existing simulation pipeline failures (`multi_setup_top_bottom_simulation`, `multi_setup_backward_scrub_uses_checkpoints`) — the bottom-up tri-dexel cut produces empty stock. These predate the service layer work.

## Recent work (2026-04-05)

### Deep architecture audit

Six-domain audit covering type design, error handling, API surface, module organization, state mutation, and concurrency. Implemented high-priority fixes across all domains.

**Undo correctness (HIGH):**
- Simulation state (`results`, `playback`, `checks`) now invalidated on undo/redo of stock, tool, and machine changes — previously showed stale mesh after undo
- Toolpath `stale_since` now set on undo/redo of operation parameter changes — previously left old computed result displayed
- Extracted `invalidate_simulation()` helper method replacing duplicated 5-line pattern

**Parameter bounds checks (HIGH):**
- `emit_ramp()` guards against `max_angle_deg <= 0` or `>= 90` (prevents NaN from `tan()`)
- `emit_helix()` guards against `radius <= 0` (prevents degenerate spiral)
- `spiral_finish_toolpath_structured_annotated()` guards against `stepover <= 0` (prevents division by zero)
- 5 new edge-case tests for boundary parameter values

**Error handling:**
- `OperationError` enum (`MissingGeometry`, `InvalidTool`, `Cancelled`, `Other`) replaces `Result<T, String>` throughout 20+ operation functions in `operations_2d.rs` and `operations_3d.rs`
- Swallowed errors now logged: CLI mesh overlay import, presets directory creation

**API surface cleanup:**
- Deleted dead `pipeline` module (239 lines, zero external callers)
- Renamed viz `CutDirection` (UpCut/DownCut/Compression) to `BitCutDirection` to resolve naming collision with core `ramp_finish::CutDirection` (Climb/Conventional/BothWays)

**Module organization:**
- `operations.rs` (2948 lines) split into 7 per-family files under `operations/`
- `events.rs` (1721 lines) split into 6 handler-group files under `events/`

**Concurrency & lifecycle:**
- Worker threads now have graceful shutdown via `AtomicBool` shutdown flag + `Drop` impl that joins threads
- Simplified `cancel` from redundant `Arc<AtomicBool>` to `AtomicBool` (already inside `Arc<LaneQueue>`)
- Result channel bounded to 64 entries (`sync_channel`) to cap memory under heavy load

**Enum conversion ownership:**
- `DrillCycleType::to_core()` method owns the conversion from viz unit enum → core associated-data enum
- `DressupEntryStyle::to_core()` method owns conversion from viz config → core `EntryStyle`
- Renamed viz `EntryStyle` to `Adaptive3dEntryStyle` to disambiguate from `DressupEntryStyle`
- Extracted adaptive3d entry defaults to named constants (`ADAPTIVE3D_HELIX_RADIUS_FACTOR`, etc.)

**Performance:**
- Dressup pipeline takes ownership (`Toolpath` instead of `&Toolpath`) — eliminates clone-on-early-return in `apply_tabs`, `apply_dogbones`, `apply_link_moves`
- Adaptive3d path segments moved instead of cloned — reordered stamp-then-push to avoid `path_3d.clone()`

### Architecture & reuse audit

Follow-up codebase-wide audit focused on ownership clarity, coupling, and extension friction. Addressed 9 findings across 6 work streams.

**Silent-break footguns fixed:**
- `OperationType::AlignmentPinDrill` was missing from `ALL` array — silently excluded from iteration. Fixed and added exhaustiveness tests for all 10 enums with manually-maintained `ALL` constants (`OperationType`, `ToolType`, `PostFormat`, `ToolMaterial`, `CutDirection`, `FaceUp`, `ZRotation`, `Corner`, `FixtureKind`, `HeightReference`)

**Enum deduplication (core ↔ viz):**
- 6 operation parameter enums (`ProfileSide`, `FaceDirection`, `TraceCompensation`, `ScallopDirection`, `CutDirection`, `SpiralDirection`) were identically defined in both `rs_cam_core` and `rs_cam_viz` with trivial conversion code. Added `Serialize`/`Deserialize` derives to core, deleted viz duplicates, removed conversion matches from `operations_2d.rs`/`operations_3d.rs`

**PostFormat ownership:**
- `PostFormat` enum moved from viz to `rs_cam_core::gcode` with `post_processor()` method. Export functions now call `format.post_processor()` directly instead of string-matching through `get_post_processor()`. Eliminates 3 identical format→string→processor match blocks in `export.rs`

**CLI consolidation:**
- Extracted `run_collision_check()` helper replacing 6 identical 45-line collision-check blocks in CLI `main.rs` (~240 lines removed)
- `CliToolType` enum replaces stringly-typed tool parsing in TOML job files — compile-time exhaustive matching with `serde(alias)` for backward compatibility
- `VendorLut` re-exported from `feeds` module — viz no longer reaches into internal `feeds::vendor_lut::VendorLut` path

### Logic consolidation audit

Full-codebase audit across 5 domains (operations, rendering, feeds/speeds, core/viz boundary, serialization) to reduce scattered logic and improve extensibility. Findings and implementation documented in `planning/CONSOLIDATION_AUDIT.md`.

**Operation extensibility:**
- `OperationParams` trait eliminates ~200 match arms from `catalog.rs` — common accessors (feed_rate, plunge_rate, stepover, depth_per_pass, depth_semantics) now dispatch through `as_params()`/`as_params_mut()` instead of 10 separate 23-arm match blocks
- Deleted 3 duplicate `op_feed_rate()` functions from `preflight.rs`, `sim_timeline.rs`, `sim_diagnostics.rs`

**Rendering consolidation:**
- Centralized 40+ hardcoded color literals into `render/colors.rs` module (toolpath palette, tool assembly, height planes, grid axes, stock, deviation)
- `MoveType::is_cutting()` and `MoveType::feed_rate()` helpers for toolpath move classification

**HTML/Three.js scaffold:**
- Extracted 7 shared helper functions from `viz.rs` (`html_head`, `html_importmap`, `html_scene_setup`, `html_toolpath_objects`, `html_grid_axes`, `html_tail`, `serialize_toolpath_lines`)

**Feeds/speeds ownership:**
- `Material::base_cutting_speed_m_min()` and `Material::plunge_rate_base()` replace hardcoded match blocks in `feeds/mod.rs`
- 12+ magic numbers replaced with named constants (`SLOTTING_THRESHOLD`, `LD_SEVERE_THRESHOLD`, `FLUTE_GUARD_FACTOR`, etc.)

**CLI/viz unification:**
- CLI `build_tool()` now returns `ToolDefinition` instead of `Box<dyn MillingCutter>`, matching the viz crate pattern
- `OpResult.cutter` upgraded to `ToolDefinition`, giving CLI access to assembly info and `to_assembly()` for collision detection

**Project file simplification:**
- `ProjectToolSection::into_runtime()` now constructs `ToolConfig` directly instead of creating a default and overwriting all fields — compiler enforces completeness

## Current priorities

- **G16 layered scoring (in flight)** — multi-commit follow-on to the G16 reorg, softens binary gates and adds composite scoring. Design doc §11: `planning/OPTIMIZER_REFACTOR_G16.md`. **Tracker (read first): `planning/G16_LAYERED_SCORING_PROGRESS.md`**.
- **MCP server polish** — MCP server (`rs_cam_mcp`) is shipped with 16 tools; ongoing work to integrate with running GUI session for real-time AI agent access. Design doc: `planning/SERVICE_LAYER_EXTRACTION.md`
- **Fix 2 remaining simulation test failures** — `multi_setup_top_bottom_simulation` and `multi_setup_backward_scrub_uses_checkpoints` fail because bottom-up tri-dexel cuts produce empty stock
- **Stock-level alignment pins** — moving pins from per-setup to the stock definition so they persist across flips. Design doc: `planning/ALIGNMENT_PINS_DESIGN.md`
- **Tri-dexel simulation** — Phases 1–6 complete (core types, stamping, mesh extraction, viz wiring, multi-setup carry-forward, side-face grids). Design doc: `architecture/TRI_DEXEL_SIMULATION.md`, implementation plan: `planning/VOXEL_SIM_DESIGN.md`
- keep public docs aligned with the actual code surface
- preserve explicit attribution for algorithms, datasets, and runtime assets
- maintain the lint/test gate as the default merge bar

## Recent work (2026-03-23)

### BREP/STEP post-merge improvements

Addressed findings from the independent BREP/STEP review (`review/BREP_STEP_REVIEW.md`):
- **State safety**: face selection cleared on model removal; face_selection IDs validated against enriched mesh on project load with `FaceSelectionStale` warning; u16 face count overflow guard
- **Controller routing**: face pick toggle moved from inline `app.rs` mutation to `AppEvent::ToggleFaceSelection` through the controller event system
- **Undo support**: toolpath param snapshot extended to include `face_selection`, enabling undo/redo of face selection changes
- **User feedback**: STEP import now shows status messages (success or error) in the status bar; ImportStep handling moved to app-level for camera fitting; face polygon fallback warns when selected faces are not horizontal planes
- **UX polish**: operation category labels changed from "from SVG"/"from STL" to "Boundary"/"Surface"; face properties panel shows model name instead of debug `ModelId`

## Recent work (2026-03-22)

### Continuous multi-setup simulation display

Fixed four visual bugs that made the simulation look like separate per-setup runs instead of one continuous process on a block of material:
- **Checkpoint mesh frame**: checkpoint meshes were displayed in global frame without the setup transform — now `load_checkpoint_for_move` applies the same global→local transform as live playback
- **Bottom surface visibility**: `dexel_stock_to_mesh` now generates a bottom-surface mesh (from `ray_bottom`) when bottom cuts exist, so `FromBottom` cuts are visible
- **Playback starts from uncut block**: simulation results now initialize with `current_move=0, playing=true` instead of jumping to the end — the user sees the tool progressively cutting from an uncut block
- **Stock carry-forward in partial re-sim**: `run_simulation_with_ids` now includes all enabled toolpaths from preceding setups as additional groups, so Setup 2 shows Setup 1's residual stock (through-holes, prior cuts)
- Extracted `transform_mesh_to_local_frame` helper for shared mesh frame transform logic

### Semantic simulation debugger

Added a generic trace-driven debugger surface in the Simulation workspace:
- toolpaths can emit both performance traces and move-linked semantic traces, persisted at runtime with JSON artifacts
- Simulation now exposes semantic trees, linked spans, annotations, issue navigation, viewport picking, and inspect-in-simulation flow
- adaptive/adaptive3d emit richer semantic structure and math-stage attribution instead of only generic pass buckets
- runtime hotspots now estimate cutting/rapid time per semantic item so expensive runtime regions are visible alongside compute hotspots

### Tri-dexel Phase 6: Side-face grids and multi-grid mesh

Extended the tri-dexel simulation to support all six cardinal face orientations:
- `DexelGrid::x_grid_from_bounds` and `y_grid_from_bounds` constructors (rays along X/Y, indexed by YZ/XZ)
- `StockCutDirection` extended with `FromFront`, `FromBack`, `FromLeft`, `FromRight`
- Lazy grid initialization: X/Y grids created on first side-face stamp from `stock_bbox`
- Factored out axis-agnostic `stamp_point_on_grid` / `stamp_segment_on_grid` — Z/X/Y grids share the same inner loop with axis decomposition
- `face_up_to_direction` now returns the correct direction for all `FaceUp` variants
- Multi-grid mesh extraction: `dexel_stock_to_mesh` combines Z-grid surface with side-grid surfaces (using `ray_top` heightmap per grid, vertex positions mapped back to world XYZ)
- Checkpoint correctly deep-copies lazily-created side grids
- 15 new tests covering X/Y grid constructors, all four side-face stamp directions, linear segment on Y-grid, multi-grid simulation isolation, checkpoint with side grids, and multi-grid mesh vertex count

### Tri-dexel simulation backend (Phases 1–5)

Replaced the 2.5D heightmap simulation backend with a tri-dexel volumetric
representation throughout the viz crate:
- Core data types: `DexelSegment`, `DexelRay` (SmallVec), `DexelGrid`, `TriDexelStock` (`dexel.rs`, `dexel_stock.rs`)
- Tool stamping and toolpath simulation with `StockCutDirection` (FromTop / FromBottom)
- Mesh extraction via `dexel_stock_to_mesh` producing the same `HeightmapMesh` format (`dexel_mesh.rs`)
- Viz wiring: `SimulationRequest`, `SimulationResult`, `SimCheckpoint`, and `SimulationPlayback` all use `TriDexelStock` instead of `Heightmap`
- Live playback (`update_live_sim`) uses `simulate_toolpath_range` on `TriDexelStock`
- `StockCutDirection` derived from setup's `FaceUp` orientation
- GPU pipeline unchanged — `HeightmapMesh` remains the render format
- **Phase 5: Multi-setup sequential simulation** — `run_simulation_with_all` now simulates ALL setups sequentially on one `TriDexelStock` in the global stock frame. Toolpaths are pre-transformed from each setup's local frame to the global stock-relative frame using `FaceUp`/`ZRotation` inverse transforms (including arc direction correction). `SimulationRequest` carries per-setup `SetupSimGroup`s with direction. Per-boundary direction stored in results enables correct multi-direction live playback scrubbing. Checkpoints at each toolpath boundary support backward scrub across setup transitions.
- 60 core dexel tests + 59 viz tests pass

### Workspace UX redesign — multi-setup coordinate frames

Unified the coordinate frame pipeline so all toolpaths are generated and
displayed in setup-local coordinates. Previously, identity setups (Top+Deg0)
generated in global coords while non-identity setups used local, causing
intermittent alignment bugs.

Key changes:
- Generation always transforms mesh/stock to local frame (even for identity setups)
- All workspaces (including Simulation) display in the active setup's local frame
- Per-workspace display rules: solid stock in Setup only, model hidden in Simulation, etc.
- Setup panel shows effective stock dimensions for active orientation
- "Toolpaths use fresh stock" badge on non-first setups

### Multi-setup simulation — 2.5D heightmap limitation discovered

The 2.5D heightmap can only model cuts from one direction (top-down).
Attempting to simulate flipped setups on one heightmap causes gouging:
bottom cuts are misinterpreted as deep top-to-bottom cuts. This is a
fundamental data structure limitation, not a transform bug.

Current workaround: each setup simulates independently in its own local
frame with fresh stock. Correct for single-setup and same-orientation
multi-setup. Cross-setup material carry-forward deferred to tri-dexel.

### Tri-dexel simulation design

Completed research and design for replacing the heightmap with a tri-dexel
representation. Three orthogonal grids of ray segments (Z, X, Y) handle
cuts from any cardinal direction natively. SmallVec fast path keeps
single-setup performance within 20% of the current heightmap. Six-phase
implementation plan from core data types through multi-setup carry-forward.
See `architecture/TRI_DEXEL_SIMULATION.md`.

### Heights/setup-frame audit (2026-06-12)

"Roughing renders way below the mesh" (wanaka200) audited end-to-end: the
G-code was always gouge-free; the symptom was the viewport drawing
identity-setup toolpaths (world frame since F-028) against an
origin-subtracted local-frame mesh. Four fixes landed: emission→display
shift adapter across all viewport uploads (F1); adaptive3d honors pinned
heights `top_z`/`bottom_z` (F2); session compute feeds ops the
emission-frame stock bbox, ending GUI-vs-CLI divergence for identity
setups with `origin != 0` (F4); `SurfaceHeightmap.covered` hole mask (F3,
revised — hole-diving during roughing is intended clear-everything
semantics; the lever is a pinned `bottom_z`). Full audit + repro:
`planning/HEIGHTS_SETUP_FRAME_AUDIT_2026-06-12.md`.

## Known open work

- **F-034 cycle-time re-bench** — the 827 s Shapeoko wall-clock anchor is stale and the calibration test now flakes near its widened floor (0.30); re-measure per `planning/cycle_time_rebench.md`
- **enclosed-hole detection for adaptive3d** — optional "uncovered region not touching the grid border = hole, don't descend" mode on top of `SurfaceHeightmap.covered`
- **tri-dexel contour-tiling mesh** — full surface reconstruction for non-heightmap views (current side-grid mesh uses ray_top heightmap)
- emit per-operation manual pre/post G-code in export
- wire profile controller compensation (`G41` / `G42`)
- surface rapid-collision rendering and simulation deviation coloring
- expose workholding rigidity and vendor-LUT management in the GUI
- continue optional cleanup in `adaptive.rs` / `adaptive3d.rs`, but structural blockers are no longer the active tranche

## Verification

- `cargo run -q -p rs_cam_cli -- --help` succeeds
- `cargo fmt --check` passes
- `cargo test -q` passes on the workspace
- `cargo clippy --workspace --all-targets -- -D warnings` passes

Update this file when the shipped surface or verification status changes materially.
