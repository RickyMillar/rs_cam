# Performance review — generation, simulation, viewport (2026-08-19)

Read-only review by three parallel Opus 5 agents (no edits, no cargo runs; another
agent was active in the repo). Reference workload throughout: wanaka200 —
8 toolpaths over a 661,212-triangle terrain STL, 12.6k-move passes, ~70k–600k-sample
cut traces, ~40 min `generate_all` wall clock observed.

**Headline per area:**

- **Simulation** — the stamp kernel does ~26× redundant cell work, is single-threaded,
  and the fixpoint loop re-simulates the whole project per round. Biggest wall-clock lever.
- **Generation** — the time is in *pruning failures and repetition*, not allocation:
  the spatial index prunes nothing for waterline fibers, and every depth-stepped 2.5D op
  recomputes its full 2D geometry once per Z level.
- **Viewport** — GPU side is healthy (dirty-flagged uploads, batched draws). Nearly all
  cost is CPU analysis re-derived from the full cut trace *every frame*; the existing
  trace-pointer cache pattern just isn't applied at several call sites.

A recurring meta-finding: **the bench suite misses every hot path found here.**
`perf_suite.rs` benches the non-metric stamp variants (~25× cheaper than production),
seeds the offset cascade once (cannot see the per-level repetition), and has no bench
for the 2.5D pipeline, orchestration, `run_simulation` end-to-end, or any dressup.
Baseline benches should land before the metric-changing fixes.

Also: `CLASSIFICATION_PERF_STUDY.md` already refuted the "per-query allocation" hypothesis
(query_into + scratch measured 0.99×). Allocation churn is mostly NOT where the time is
on this codebase; pruning, repetition, and parallelism are.

---

# Part 1 — Simulation (`rs_cam_core` dexel sim + metrics)

Hot path: `simulate_toolpath_with_lut_metrics_cancel` → `capture_cutting_segment` →
`estimate_and_stamp_cutting_subsegment` → `stamp_segment_with_metrics`
(`dexel_stock/simulation.rs:137,412,560` → `dexel_stock/stamping.rs:490`).
Per-cell cost ≈ 40–100 ns (coverage sqrt×2, LUT probe, 3 ray walks, coverage RMW).

## S1. Per-subsegment full-footprint stamping — ≈ 2R/s × redundant. HIGH

`simulation.rs:442-446`: subsegments = max(len/sample_step, z_drop/0.02mm).
Each subsegment stamps the full tool-radius-inflated bbox (`stamping.rs:590-601`).
Ø6 tool, cs=0.1, s=0.25 → ~15,400 cell-visits/mm vs ~600 needed (**≈26×**, scales as 2R/s).
The `by_z` arm turns a 5 mm plunge into 250 identical stamps (~1M cell-visits/hole) —
coverage and h(d) are depth-independent on a vertical descent.

Fix (two halves):
- (a) Swept-volume stamping: stamp a whole move/chunk in one stadium pass;
  `segment_cell_coverage` already returns `t_center` per cell — bin each cell's
  removal/perp/penetration into `floor(t_center·n)` metric bins (all reductions are
  associative). 10–30× fewer cell-visits on lateral moves.
- (b) Kill by_z analytically: min of `z(t)+h(d(t))` over a cell's in-reach interval is at
  t_center or the descending endpoint — evaluate both (2 LUT probes) instead of subdividing.
  Pure plunges: one XY pass + per-cell clamp.
- **Risk:** merging subsegments changes `pre_fresh` per sample (density-independence,
  `stamping.rs:471-486`); F-XXX / litmatrix sentries need deliberate re-baselining.

## S2. No air-cut early-out. HIGH

No "can this stamp remove anything here?" test anywhere in the stamp path — full
coverage+LUT+ray cost over cells the cutter can't reach, in the *dominant* regime
(air-cut thresholds tolerate 30–40% on finish/clearing, 97% on ProjectCurve).

Fix: coarse max-top mip over the z-grid (one f32 per 16×16 tile, ~4 KB/1M cells).
`tile_max_top ≤ depth_min` ⇒ skip tile, exactly (h(r) ≥ 0). Tile maxima only decrease
under top-down stamping — maintain with a monotone min where `lower_conservative_top`
is called (`dexel.rs:554`). 2–10× on finish passes, multiplicative with S1.

## S3. Stamping is 100% single-threaded. HIGH

rayon is in the crate and used in dropcutter/slope/pencil/rest_field/classify_probe and
the deviation passes — nothing in `dexel_stock/` touches it. The kernel is purely per-cell
(`stamping.rs:642-687`), no cross-cell coupling.

Fix: row-band decomposition over a whole toolpath (`rays.par_chunks_mut(band_rows*cols)`
+ matching side arrays); each worker replays all moves clipped to its band, skipping
moves whose bbox misses. Per-cell mutation order preserved ⇒ **bit-identical** results
(matters: `ray_blend_above` with f<1 is non-commutative). Metrics reduce across bands.
Expected 6–12× desktop.

## S4. Per-toolpath full-grid clones + mesh extraction: O(k·cells). HIGH

`compute/simulate.rs`, per toolpath: line 707 unconditional grid clone (`prior_stocks`);
848-856 duplicate LUT rebuild + second `global_stock` stamp; 860 full MC mesh;
870 full mesh transform copy; 878 checkpoint clone. 32 B/cell ⇒ 128 MB/clone at 4M cells;
12-op project ≈ 3 GB of memcpy.

Fixes, easiest first:
- `prior_stocks` snapshotted for EVERY toolpath (line 713) but consumed only by
  `FromRemainingStock` generation + dressup air filter — snapshot only requested ids.
- **`coverage_max` has zero consumers** (written `stamping.rs:365,460,568,673`, read by
  nothing; its own doc says "purely observational"). Delete/cfg-gate: −4 B/cell in every
  clone AND removes a third cache stream from the inner loop.
  **CORRECTION (wave 1 SIM): "read by nothing" is wrong.** It was read by the PUBLIC
  accessor `DexelGrid::coverage_at`, which `tests/sub_cell_stamping_fa.rs` — the F.a
  sub-cell-stamping sentry suite — read in six places. The accurate statement is the
  narrower one the field's own docstring makes: **no production reader** (verified by grep
  across core/viz/mcp/cli). DONE anyway, with the sentry **converted, not dropped**: on
  fresh stock `coverage = (top₀ − top) / depth` recovers the same number from the blended
  ray top, which is the channel the simulator actually consumes. The conversion also added
  an assertion the sidecar reading never made (partial coverage must be a multiple of 1/16,
  pinning the 4×4 sub-sample fan). See `DELTA_sim_w1.md` §1a.
- `global_stock` redundant for single identity group (translation of group_stock —
  derivable O(1)); checkpoints could store a top-Z field (4 B/cell) instead of full rays.
- `append_transformed` (`stock_mesh.rs:34-64`) pushes one f32 at a time, no reserve;
  short-circuit identity path when stock_min == 0.
- duplicate `playback_lut` at 848 (== `lut` at 728) — delete.

## S5. Fixpoint loop re-simulates the whole project from fresh stock per round. HIGH on rest chains

`controller/events/compute.rs:1439-1455` → `run_simulation_with_all` →
`simulate.rs:676` fresh grid + full replay. k rest ops ⇒ k full-project sims (O(k²) op-sims);
toolpaths 1..j-1 byte-identical across rounds, nothing memoized.

Fix: provenance hashes already exist (`hash_toolpath` :381, tool hash :421-429,
`operation_config_hash` :368). Cache prefix-hash → post-carve grid; start round n+1 from
the deepest matching cached prefix. Round 2 of a 10-op project stamps 1 op, not 10.
Adjacent: `session/compute.rs:930 simulate_candidate_isolated` runs a FULL run_simulation
(global stock, checkpoints, mesh, snapshots) just to harvest one cut_trace — needs a
metrics-only mode.

## S6. Cut-trace pipeline: per-sample allocs + full clone + pretty JSON every run. MED-HIGH

- `span_path: to_vec()` per sample (`simulation.rs:550`, `stamping.rs:827`) — 70k mallocs
  of near-identical content. Intern: `Arc<[SpanId]>` or `SmallVec<[SpanId;4]>`.
  **DEFERRED (wave 1 SIM), with a prerequisite the finding does not name:** neither type
  serializes on this workspace as configured. `serde`'s `Arc` impls are behind its `rc`
  feature and `Cargo.toml:27` enables only `derive`; smallvec has `union`, not `serde`
  (`Cargo.toml:33`). `SimulationCutSample` IS serialized, so the change starts with a
  workspace-manifest edit affecting all lanes — and even with `rc` on, serde deserializes
  each `Arc` separately, so sharing does not survive a round-trip and only the in-process
  trace gains. It is also a public field type change whose broken constructors include two
  `#[cfg(test)]` modules in `rs_cam_viz`. Measured sizing: ~5–8 ms per 12.6k-move toolpath,
  i.e. single-digit percent. Schedule as its own commit. See `DELTA_sim_w1.md` §3b.
- samples Vec reserved at `moves.len()*2` (`simulation.rs:162`) vs actual Σ subsegments
  (5–20× larger) — estimate from cutting_distance/sample_step.
  **DONE (wave 1 SIM):** `estimate_sample_count` reproduces the emitter's own arithmetic
  (`max(⌈len/step⌉, ⌈|Δz|/0.02⌉)`, length-only for rapids and `MoveIntent::Retract`), exact
  except that arcs are counted by chord — so it is never an over-estimate and needs no cap.
- `execute/mod.rs:427` deep-clones the whole trace, then `simulation_cut.rs:1502` writes
  `to_vec_pretty` JSON synchronously on the analysis lane, EVERY simulation including every
  fixpoint round — hundreds of MB. Make opt-in, serialize by reference, drop pretty.

## S7. Inner-loop micro-costs. MED (multiplies everything above)

- `stamping.rs:130-138`: two per-cell sqrts avoidable — both fast-path tests exact in
  squared space vs hoisted `(r±ext_diag)²`. (`point_cell_coverage` already sqrt-free.)
  **CORRECTION (wave 1 SIM): "exact in squared space vs hoisted `(r±ext_diag)²`" is
  FALSE.** Over the reals it holds; in `f64` the two rounding paths differ by an ULP either
  side of BOTH bounds, in both directions. Measured in exact IEEE754 semantics: **4
  disagreements in 14,544 probes**, and **32 in 302,900** at a denser sweep with ULP
  neighbourhoods. Clearest case: at `d = fl(r + ext_diag)` the squared form sees
  `d² == outer_sq` and fast-paths to `Some(0.0)`, while the sqrt form evaluates
  `fl(fl(r+ext) − ext)`, which lands below `r`, and falls through to sub-sampling. All four
  strict/non-strict variants were swept (32 / 114 / 117 / 199 disagreements) — **no
  comparison operator makes the algebraic bound exact.** This is the same class of defect
  as G3's `bbox.max.z <= cl.z`: a proof about exact arithmetic applied to floating-point
  code, and it was caught the same way, by a bit-level oracle rather than a shape check.
  **DONE (wave 1 SIM) in a corrected form.** Both legacy predicates are *monotone in
  `d_sq`*, so an exact flip point exists; `CoverageFastPath::new` **solves** for it (walk
  ULPs from the naive bound until the legacy predicate's own answer flips, bounded at 64)
  instead of deriving it. Exact **by construction** — the threshold is defined as the
  boundary of the old test. 0/402,072 disagreements in the f64 model. Two further notes:
  the "fully inside" bound also needs a `-1.0` sentinel when `ext_diag > r` (Ø0.5 tool on a
  1 mm grid — a real fine-tool regime), where squaring a negative flips the sense and
  reports FULL coverage near the tool axis; and `r_sq.sqrt()` was a **loop invariant**
  being recomputed per cell. Sentried by `squared_fast_paths_agree_with_the_sqrt_form`
  against a verbatim copy of the pre-S7 body: 11 radii × 11 cells × 600 steps plus a ±4 ULP
  neighbourhood at every boundary (~73k probes, exact `Option<f32>` equality), with
  `sentinel_rows > 0` and `probes_checked > 70_000` so the sweep cannot be thinned into
  passing.
  **Measured: 1.04–1.07× on the lateral kernel, and NOTHING on either plunge arm**
  (p = 0.08 / 0.76) — the plunge fixture drives the degenerate branch, which routes through
  the already-sqrt-free `point_cell_coverage`. Read that as the calibration: the inner
  loop's cost is three ray walks, a LUT probe and a read-modify-write, not two square roots.
  S7 is a rounding error next to S2/S3, and its real value here was the sentry it forced.
- `cell_upper_bound_surface` (:241-250): third sqrt + LUT probe per covered cell —
  precompute a dilated LUT `h(√d² + cs·√2/2)` once per tool/resolution.
  **DECLINED (wave 1 SIM).** Three reasons. (1) The formula as written is dimensionally
  wrong — it adds a length to an area under the root; the code computes
  `h((√(d²) + cs·√2/2)²)`, a different function. (2) It is **not metric-neutral**:
  `cell_upper_bound_surface` feeds `conservative_top`, which feeds
  `check_rapid_collisions_against_stock`. A dilated interpolated table rounds differently
  than the exact evaluation and the failure direction is "a clearance ceiling that is no
  longer an upper bound" — a safety channel, not a metric, and it does not belong inside a
  change advertised as no-behaviour-change. (3) It is already guarded by
  `from_high && coverage >= FULL_COVERAGE`, so it never runs on the boundary band. Re-scope
  as its own item with its own net.
- f64 throughout kernel where result truncates to f32 on write (:663); LUT Vec<f64>×4096
  = 32 KB ≈ L1 — f32 halves it.

## S8. Ray representation 3× larger than its data. MED (big refactor, big payoff)

`dexel.rs:39` `SmallVec<[DexelSegment;1]>` = 24 B carrying 8 B for essentially every column
(multi-segment only via `ray_subtract_interval`, unused by stamping). 3× grid footprint,
3× inner-loop bytes, len+branch per cell.
Fix: SoA `enter/exit: Vec<f32>` + sparse overflow map; top-down stamp then touches only
`exit[idx]` (4 B stride, vectorizable). Makes S3's bands bandwidth-efficient.

## S9. Also noted

- Generation strictly serialized: one toolpath lane (`compute/worker.rs:727`) — independent
  ops in `generate_all` never overlap.
- Bench suite covers only non-metric stamp variants — no bench of the production sim path.

## Already good (sim)

RadialProfileLUT (dist²-indexed, interpolated, threaded through); `point_cell_coverage`
(exact squared-space fast paths); analytic drill removal (`dexel_stock/mod.rs:332-387` —
the model for S1b); arc scratch-buffer reuse; cheap rapid-collision scan with early break;
rayon deviation passes with thresholds; MC corner-vertex sharing (~6× taken);
issue coalescing + Bounded triage caps; cancellation threaded into the stamp loop.

## Sim order of attack

1. Free/no-behaviour-change: delete coverage_max + duplicate LUT; de-sqrt coverage;
   reserves; opt-in JSON dump, no clone.
2. Structural, no metric change: selective prior_stocks; metrics-only candidate sim;
   skip zero-shift mesh transform.
3. Metric-neutral multipliers (baseline first): tile early-out (S2), row-band parallel (S3).
4. Metric-changing: swept-volume stamping (S1) with deliberate sentry re-baseline.
5. Longer horizon: fixpoint prefix memoization (S5), SoA rays (S8).

---

# Part 2 — Toolpath generation (`rs_cam_core` + worker execute)

Coverage caveat: the reviewing agent's adaptive/adaptive3d, 3D-finishing, and mesh/dexel
sub-dives did not all return; scallop/unified_finish/adaptive3d/rest_field had spot-checks
only. All findings below individually verified by reading code.

## G1. Push-cutter spatial query radius = half the part width — index prunes NOTHING. HIGH

`pushcutter.rs:43`: `query_r = fiber.length()/2 + r`; waterline fibers span the full mesh
extent (`waterline.rs:288-301`, bbox-derived). Every fiber query returns ALL 661k triangles
+ a 5.3 MB Vec + 82 KB bitset. `waterline_toolpath_with_cancel` (:186) loops Z levels
serially, ~800 fibers each → ~26e9 triangle tests, ~200 GB transient alloc at 50 levels.
Feeds waterline, unified_finish very-steep band, steep_shallow, adaptive3d cleanup.
Benched today (`push_cutter_batch/*`).

Fix: X-fiber at row y touches only cells in band y±r — add a row/rect query (or walk cells
along the fiber, clipping sub-intervals). ~√N reduction. Also: fiber grid XY identical at
every Z — compute per-row candidates once, reuse across levels.

## G2. Every depth-stepped 2.5D op recomputes its 2D geometry once per Z level. HIGH

Definitive: NO offset cache exists. `depth.rs:282-295` calls `operation(z)` per level;
`execute.rs:1201/1127/1016/1075/837` (pocket/profile/zigzag/trace/rest) pass params where
everything but `cut_depth` is loop-invariant; `pocket.rs:79-81` re-runs compensation +
the full `OffsetRingSet` cascade; `cut_depth` is consumed in exactly one place
(`pocket.rs:390-393`, the Z stamp). Waste = level count L; 20-level pocket does 20× offset
work. `CASCADE_WALL_CLOCK = 120s` is per cascade → 20-min worst case, 90% redundant.
This is also the compute half of the documented `offset_library_failures` ×L over-count.

Fix nearly free — the seam exists: `pocket_contours`/`profile_contour`/`zigzag_lines` +
pure emission fns are already pub. Hoist geometry above `toolpath_at_levels_with_cancel`;
closure stamps Z only. Identical output by construction.
Bench blind spot: `perf_suite.rs:295 run_cascade` seeds rings once — cannot see the L×;
no bench exists for any *_toolpath or dressup.
**FIXED in wave 2 (473c3d1f) — prescription held exactly as written.** L20/L1
**22.2×/22.5×/20.4× → 1.04×/1.14×/1.01×** (pocket/profile/zigzag), i.e. L20 is
**21.7–21.9× faster**. `depth.rs` needed no change: the choke point was already right,
the repetition was at the call sites. Three things the finding did not name:
(1) **`truncated_core_mm2` was multiplied by L too** — `generate_pocket` accumulated it
inside the per-level closure, so a bounded cascade reported 20× the material it left
standing on a 20-level pocket. Quantitative, on a surface that reaches narrate/MCP.
(2) **The `offset_library_failures` channel needs NO adjustment** — it always counted
offset *calls*; the compute was genuinely making L× of them, so it self-corrects.
The CLAUDE.md caveat is now stale for these five families.
(3) **`face.rs` has the same defect, unfixed** (`face_toolpath_with_cancel` re-insets the
facing rectangle per level) — outside wave 2's file ownership, small win, same finding.
Also: the Phase 0 golden pins the PRE-fix call shape and therefore cannot see the hoist;
six new bit-for-bit hoisted-vs-naive property sentries were added alongside it.
Numbers and caveats: `DELTA_gen_w2.md`.

## G3. `drop_cutter` lacks bbox/monotone-Z early-outs + virtual call per triangle. HIGH

`tool/mod.rs:428`: no cheap reject before facet + 3 vertex + 3 edge drops (divisions,
sqrts). `Triangle.bbox` precomputed (`geo.rs:154-158`), never consulted.
`CLPoint::update_z` is monotone increasing ⇒ a max-Z bbox reject skips most triangles
after first contact; XY-AABB reject vs cl±radius similarly.
**CORRECTION (wave 1, ca92d767): the bare `tri.bbox.max.z <= cl.z` form is NOT sound** —
edge_drop accepts edge params in ±1e-8 slack (contact can land outside the bbox in Z), and
flat-tip vertex_drop makes `cl.z` exactly a vertex height, so `bbox.max.z == cl.z` is
SYSTEMATIC for every triangle sharing that vertex. Caught by `steep_shallow_fingerprint`
with an identical move count and a different hash. Landed form pads both rejects by
`DROP_CONTACT_SLACK_MM = 1e-4`. CLASSIFICATION_PERF_STUDY.md:91 measured contact math = 4/5 of classify cost.
Also: `ToolDefinition` (`tool/mod.rs:596,691`) dispatches through `Box<dyn MillingCutter>`
per triangle — blocks inlining. Fix: early-outs + dispatch once per batch on concrete
cutter; sort cell lists by descending max-Z at index build.

## G4. `Polygon2::contains_point` has no bbox reject — inner predicate of ~12 generators. HIGH

`polygon.rs:202` → full ray cast over exterior + all holes; no cached AABB on Polygon2;
`RegionSet::contains` (`region_set.rs:63`) layers `.any()` on top → O(regions×V)/point.
Worst: `boundary.rs:291-297` per MOVE (rings ~1400 verts at 0.5 mm cell, up to 64 regions).
Other sites: `toolpath.rs:658`, `scallop.rs:2260`, `waterline.rs:225`,
`steep_shallow.rs:253,465`, `horizontal_finish.rs:244`, `ramp_finish.rs:772`,
`spiral_finish.rs:204`, `adaptive3d/path.rs:416`, `adaptive/material_grid.rs:47,84`,
`surface_link.rs:318`, `rest.rs:49`.
Fix: cache AABB on Polygon2 (one change, twelve sites); then y-bucketed edge index;
memoize (last_xy,result) in the boundary closure for plunge/retract runs.
FIXED in wave 1 (1e3c5d8c): lazy OnceLock exterior-only bbox + invalidation at the five
post-construction exterior-mutation sites; NaN poisons the box (disables reject, never
changes an answer). Measured: contains_point 3.90×, RegionSet path **51×**.
G3 FIXED in wave 1 (ca92d767, with the slack correction above): batch drop-cutter
**6.5× flat / 6.1× ball**; query_only control flat — the index query now bounds
classification cost. Devirtualization + cell-sort-by-maxZ remain follow-ups.

## G5. Six hand-rolled O(n²) greedy NN path orderers. HIGH at scale

`tsp.rs:309`, `surface_link.rs:249`, `pencil.rs:579`, `adaptive3d/clearing.rs:1368`,
`ramp_finish.rs:320`, unified_finish routing. tsp's O(N³) 2-opt IS capped
(MAX_2OPT_SEGMENTS=500) but the quadratic NN seed has no guard. Reachability: on by
default (`config.rs:1703`); whole-path branch (`execute.rs:3183`) fires for
DropCutter/Drill/PinDrill/ProjectCurve; DropCutter spans are SpanKind::Region (not a
barrier) → single group of 10³–10⁴ segments. surface_link documents 12,780 fragments on
wanaka ×2 → 163M distance evals.
Fix: one shared `nearest_neighbour_order` on a grid/k-d tree with deletion — O(n log n),
removes the need for caps; six sites collapse to one.
FIXED in wave 3b (gen-w3b): shared `nn_order::NearestPicker` (CSR grid, rebuild-on-halving
deletion, strict ring early-out). Output **bit-identical at every site** — the tie-break is
lexicographic `(distance, index)`, reproduced exactly, and pinned against verbatim copies of
the replaced loops plus a pre-change A/B fingerprint. Measured: `optimize_rapid_order`
exponent **2.028 → 1.074** (5k→20k, 32.6× faster at 20k); `relink_fragments` **1.540 → 1.227**
(12.5× faster at 6k fragments). **Two corrections to the text above.** (1) The count is FIVE
sites, not six: `ramp_finish.rs:320` is a greedy bipartite MATCH, not a tour — its nearest
candidate can be rejected by an extent test and must stay available for the next query, so it
uses the shared type with acceptance-driven rather than selection-driven deletion; and
`unified_finish.rs:2434` is not a distance problem at all — its per-candidate cost is a
mesh-drape link TIME from `compute_cycle_time`, not monotone in XY, so a geometric orderer
would change its route. (2) "Removes the need for caps" is true of the SEED only.
`MAX_2OPT_SEGMENTS` guards the O(N³) 2-opt refinement, which nothing here touches; it must
stay. Separately, `unified_finish`'s quadratic is already bounded by an existing net —
`finish_planner_wanaka_decompose.rs:213,454` assert `region_count <= 24` on wanaka and
`<= 100` across the dial sweep — so it is not the uncapped class this finding names. Its
own fix (prune `choose_link` with the admissible bound `cost_s >= xy/v_max`, comparing
lexicographic `(cost_s, index)`) is recorded as follow-up in `DELTA_gen_w3b.md`.

## G6. surface_link rescans provenance per fragment: O(fragments × total_moves). HIGH

`surface_link.rs:391` inside the per-fragment loop scans all `n_in` moves; at 12,780
fragments × 10⁵ moves ≈ 10⁹ iterations writing known info.
Fix: invert once — owner→rapids in one O(n_in) pass before emit; O(n_in + F).
FIXED in wave 3b (gen-w3b), landed as its own commit ahead of G5. The same single O(n_in)
walk that already built `owner_of_rapid` now stores `rapids_of_frag` instead; only the
storage shape changed, so the provenance is identical by construction (each rapid has exactly
one owner in both formulations) and is pinned by an A/B fingerprint over `(moves, provenance)`.
Counted: 6,000 fragments × 41,999 moves = 251,994,000 iterations → 11,999.

## G7. Arc fitting quadratic on both branches. MED-HIGH (runs on every toolpath)

`arcfit.rs:212-225`: window rebuilt from scratch at sizes 3..b, `try_fit_arc` re-runs
least-squares + full tolerance loop per size — Θ(b²) per accepted arc.
`arcfit.rs:173-183`: run-boundary rescan restarts after every i+=1 on non-fitting runs —
Θ(L²) on linear/raster paths.
Fix: precompute run boundaries O(n); Kåsa fit is a moment sum — extend O(1), check only
the new point. Θ(b²)→Θ(b).

## G8. Spatial index rebuilt per toolpath; Vec<Vec<usize>> cells; silhouette + mesh copy repeated. MED-HIGH

`session/compute.rs:1138-1141` + `viz execute/mod.rs:54` build the 661k-tri index inside
per-toolpath resolution → 8 rebuilds per generate_all (× fixpoint rounds). Mesh is already
Arc — cache on (Arc::as_ptr, cell size). Build (`mesh.rs:527-546`) uses ~82k heap Vecs
(~330k reallocs) — convert to CSR (count, prefix-sum, fill).
Same region: `model_silhouette` (`boundary.rs:385`, called `execute/mod.rs:153`) rasterizes
all faces serially per toolpath; `transform_mesh_to_setup` (`session/compute.rs:1071`)
deep-copies the mesh per toolpath — ~95 of ~111 MB is the redundant expanded `faces` array.
Fix: build index/silhouette/transformed mesh once per (model, setup), thread through.
**FIXED + SEVERITY CORRECTED (wave 4 gen, `DELTA_gen_w4.md`).** All three repetitions
were real and all three are gone (`crate::geom_cache`, memoised on mesh identity; counts
8→1, 16→1, 8→1 per round, asserted by a test that drives eight real `generate_toolpath`
calls). Three corrections to the row.
(1) **MED-HIGH is wrong — this is LOW on wall clock.** Measured paired same-run on the
661k terrain: index build 37.9 ms, silhouette 33.0 ms, mesh transform 27.9 ms. Removing
every repetition saves **0.97 s** per 8-op round in the most favourable project shape and
**0.28 s** in the common one — **0.012–0.040 %** of a ~40 min `generate_all`. Its real
value is memory (888 MB of deep-copy churn per round → 111 MB) and structural, not speed.
(2) **The proposed `Arc::as_ptr` key is UNSOUND** (ABA: an `Arc` can be dropped and a new
one land at the same address). Not hypothetical — a test that drops one `Arc` and
allocates 64 more observes address reuse on this machine. Fixed by keying on a
`Weak<TriangleMesh>`, which pins the allocation and so makes collision impossible rather
than unlikely. `cached_simulation_triage`'s bare-pointer key (flagged in `DELTA_viz_w2.md`)
still carries the hazard.
(3) **`spatial_index/build_terrain` measures the wrong resolution.** It builds a
few-hundred-cell grid, where CSR *loses* 8 % (two passes lose to one when there is nothing
to allocate). The 82,944-cell grid `build_auto` produces — the only resolution any
production call site uses — is where CSR wins **1.92×**, and nothing benches it. A
`build_terrain_auto` arm is proposed in the delta doc.

## G9. V-carve/inlay/rest brute-force distance field per sample. MED-HIGH

`vcarve.rs:138`, `inlay.rs:215`, `rest.rs:126`: `point_to_polygon_distance` linear over
all edges per sample; default tolerance 0.05 → sample_step 0.05 mm → 400k samples on
100×100 mm; ×2000-edge lettering = 8e8 point-seg evals, single-threaded; helper
triplicated; inlay runs twice.
Fix: one indexed EdgeDistanceField shared by all three; `scan_lines.par_iter()` is
mechanical.
**CORRECTED + FIXED (wave 3 gen, `DELTA_gen_w3.md`).** Two corrections to the row.
(1) **`rest.rs:126` is not a distance field.** It is `point_in_any_polygon` →
`Polygon2::contains_point`, an even-odd ray cast against the previous tool's reachable
region — same family (per-sample linear walk over all edges) but a *containment* query
that cannot share the distance index, and `contains_point` already has a bbox early-out.
Verified against the pre-G2 file, so it is not an artifact of `473c3d1f` moving lines.
The helper is **duplicated** (vcarve + inlay), not triplicated; the shared field covers
**two** call sites. Rest got the parallelism half only.
(2) **The emphasis is inverted.** Measured on `gen_vcarve_field`: the index alone is
2.98×, the "mechanical" `par_iter()` is a further **4.10×**. Combined 193.67 ms →
15.869 ms, **12.2×**. The parallelism was the bigger half.

## G10. Serial full-grid tail after parallel drop-cutter. MED

`compute/execute.rs:2288`, `unified_finish.rs:1920`: after rayon row build, serial
`for pt in &mut grid.points` doing radius-0 `index.query` + contains per point (160k on
400²). Amdahl tail. `cell_triangles_at` (`mesh.rs:682`) exists for exactly this, used only
by classify_probe; also applies at `dropcutter.rs:229`, `project_curve.rs:162`,
`compute/simulate.rs:1274`.
Calibration: the ALLOCATION half was A/B-tested at 0.99× (CLASSIFICATION_PERF_STUDY.md:80) —
the PARALLELISM is the real win; cell_triangles_at is the free tidy-up.

## G11. detect_containment: O(n²) area/bbox recomputation + pass-2 redo. MED

`polygon.rs:1324`: sort comparator recomputes full shoelace per comparison (:1335);
pass 1 rebuilds both bboxes per `polygon_contains_polygon` call (:1425); pass 2
(:1379-1392) recomputes pass 1's discarded results. Hot caller `adaptive3d/clearing.rs:1559`
per Z level on marching-squares micro-peak contours.
Fix: decorate-sort-undecorate with precomputed (bbox, area); bitset pass-1 results.

## G12. Smaller verified items

- `polygon.rs:459`: unconditional O(V²) `has_self_intersection` + full clone on the clean
  path of every offset — compounds with G2's L×.
- `dressup.rs:636`: `apply_tabs` position() scan inside enumerate — both ascending, use a
  cursor: O(n²)→O(n).
- `dressup.rs:1381`: `bridge_corridor_is_swept` re-filters the same 4096-move lookback per
  corridor sample — hoist the exact pre-pass.
- `polygon.rs:1087`: `flatten_for_containment` inside a find closure — O(H·G) flattenings.
- `contour_extract.rs:73`: `Fiber::is_blocked` linear scan of a sorted list — binary search;
  Vec<Vec<bool>> vs sibling's flat slice.
- `dropcutter.rs:292-308`: flat_map+collect+unzip → `par_chunks_mut` over preallocated
  buffers + per-thread QueryScratch.
- Needless clones: `pocket.rs:349,385`, `profile.rs:137`, `trace.rs:66`.

## Missed parallelism (generation)

Only 8 files in rs_cam_core genuinely use rayon (classify_probe, compute/simulate,
dropcutter, pencil, pushcutter, rest_field, slope, waterline) — all low-level primitives.
NO generator parallelizes its outer structure. Highest payoff, in order:
1. per-scan-line sampling (vcarve/inlay/rest — pairs with G9) — **DONE wave 3 gen**,
   and it was the larger half of G9 (4.10× vs the index's 2.98×)
2. drop-cutter coverage tail (G10)
3. waterline Z levels (`waterline.rs:186`, independent)
4. per-group offsets (`polygon.rs:1271` — independent by construction, catch_unwind already
   per-group, OffsetFailure::merge associative; ordered collect preserves stability)
5. per-Z-level generation (`depth.rs:282`) once G2 lands
6. model_silhouette rasterization (`boundary.rs:402`)

## Already good (generation)

`query_into`+`QueryScratch` and `cell_triangles_at` exist and are correct — gap is adoption;
`build_auto` density-aware sizing with memory guard; RemapIndex/C9 (the pattern to copy);
`chain_segments`' quantized hash index (proof the codebase knows this trick — G1/G4/G9 are
the same problem it already solved); iterative Douglas-Peucker; `Fiber::add_interval`;
geometric_ring_cap + CASCADE_WALL_CLOCK; the arc-carrying offset cascade (fine internally,
just invoked L times); `union_all` via unary_union; MAX_2OPT_SEGMENTS (needs its NN sibling).

---

# Part 3 — Viewport & rendering (`rs_cam_viz`)

GPU side healthy; cost is CPU in the egui update loop, mostly full-trace re-derivation
per frame.

## Tier 1 — per frame, scales with scene size

- **V1. `simulation_triage` rebuilt every frame** (`ui/sim_diagnostics.rs:588-589`, inside
  default_open(true)): full ProjectDiagnostics + MeasurabilityReport (full trace pass per
  toolpath) + SimulationTriage::build (another scan + height collect+SORT per toolpath) +
  build_cutter per toolpath + project_evidence allocs. O(samples×toolpaths) w/ sorts, per
  frame, to render one strip. Fix: the existing (Arc::as_ptr(trace), edit_counter) cache
  pattern. **Largest single win, smallest diff.** FIXED in wave 1 (42ed4774):
  `SimulationTriageCache`, returns `&SimulationTriage` (no clone on hit — unlike the
  sibling caches, see Tier-4: cached_load_report/envelopes still clone on hit, easy wave-2
  follow-up in the same file).
- **V2. Signal-spine re-derives the trace 6×/frame** (`sim_timeline.rs:295-310,675-710`):
  per-sample pointer Vec (~4.8 MB/frame @600k) + each of 6 tracks re-walks all samples into
  a fresh Vec before decimating to ≤1600 pts. Memoize GroupPoints per
  (trace_ptr, focus, x_range, track) alongside span_aggregates. Minor: format!("signal_track_{label}") per track/frame.
- **V3. Deflection lookup scans full trace/frame BEFORE its early-out**
  (`app/simulation.rs:422-437` scan; the `tool_gpu_move == Some(current)` guard at :478).
  With MCP's 100 ms heartbeat this burns continuously.
  **CORRECTION (wave 1, 42ed4774): do NOT naively hoist the whole guard** — the pre-guard
  block is not pure: it publishes six playback fields (tool_position, deflection, radius,
  label, stickout, cutting_length) that the 2D tool overlay in viewport.rs reads every
  frame; a full hoist silently stales that overlay. The landed fix gates only the
  expensive `peak_deflection_for_move` trace scan (reusing `playback.tool_deflection_mm`
  when the move index is unchanged — sound because tool_gpu_move and the deflection field
  are written in lockstep and reset together). FIXED in wave 1.
- **V4. `sim.issues()` deep-clones ~25k issues (with Strings) 3–4×/frame on cache HIT**
  (`state/simulation.rs:1563-1567`); `sim_timeline.rs:117-121` clones the vec just to count
  hotspots; issue_cache_key re-hashes all collision indices per call. Fix: return
  &[..]/Arc<[..]>; cached count. FIXED in wave 1 (42ed4774): `Arc<[SimulationIssue]>` —
  note `&[..]` does NOT work here (two call sites hold the list across later `&mut sim`
  calls); `issue_hotspot_count()` added; issue_cache_key re-hash left as-is (O(handful),
  needs a dirty-flag mechanism, not worth it).
- **V5. O(spans²) span tree per frame** (`sim_op_list.rs:502,541,679-686`), force-expanded
  for the focused op during playback, unbounded rows, format!+clone per row (gated on
  sim.debug.enabled). Memoize per (toolpath_id, spans_ptr).
- **V6. Hover picking = 2 full-mesh ray casts/frame** (`app/viewport.rs:297-334,339-377`
  both call pick; `mesh.rs:443-460 ray_pick_triangle` is a linear all-triangle scan;
  SpatialIndex unused). Fix: share one pick/frame; BVH or reuse SpatialIndex; throttle to
  pointer-moved frames. (Toolpath picking already bounded to 200 samples.)
- **V7. Per-row full issue scans in op list** (`sim_op_list.rs:316→915→954`): 4 filter-counts
  per toolpath row (~2M cmp/frame @25k×20) + issue_kind_detail rescans. One pass/frame into
  HashMap<ToolpathId,[usize;4]>.

## Tier 2 — per upload (interaction hitches)

- **V8. No per-object dirty flags** (`gpu_upload.rs:134-1029`): any pending_upload clears
  and rebuilds ALL meshes/toolpaths/stock/fixture buffers via fresh create_buffer_init
  (contrast SimMeshGpuData which reuses). Triggers include selection clicks, filter/colour
  toggles, and EACH completed toolpath during generate_all (`controller/events/compute.rs:840`)
  → 8-op generate rebuilds the scene 8×. Fix: per-resource dirty bits + per-toolpath GPU
  data keyed by result generation.
  **FIXED in wave 2** (`DELTA_viz_w2.md`). **CORRECTION: the "dirty bits" half of the
  prescription was NOT taken.** Dirty bits put the burden on ~40 `pending_upload = true`
  setters to name the right resource, and a setter that names too few silently stales the
  viewport — which is the exact defect V13 already is. The landed fix is content keys
  (`render/upload_cache.rs`): each expensive resource records what it was last built from
  and rebuilds only when *those values* move. Strictly stronger (a coarse "everything"
  fire is now cheap too) and the 40 setters are untouched. Cheap resources (stock, axes,
  fixtures, polygons, height planes) are deliberately still rebuilt unconditionally —
  keying them costs more than the rebuild. Counts: 8-op generate_all 36→8 toolpath builds
  and 8→0 mesh builds; a selection click N→2 toolpath builds and 1→0 mesh builds.
- **V9. Full AnnotatedToolpath deep-clone per toolpath per upload — twice for selected**
  (`gpu_upload.rs:867-869,1010-1014,1040-1045` → `toolpath_spans.rs:518-533 translated`).
  has_display_shift is true for identity setups with non-zero stock origin — the NORMAL
  config. :1010 clones the whole thing just to reach .rest_grid (behind an Arc). Fix: apply
  shift at vertex emission; clone the Arc + shift grid origin fields.
- **V10. `span_paths_by_move()` = one heap Vec per move** (`toolpath_render.rs:204` →
  `toolpath_spans.rs:774-788`): 12k+ allocs per toolpath per upload. Flatten to CSR or
  precompute Vec<SpanClass> at generation. (SpanKindFilter::all_visible doc claiming the
  renderer can skip classify is misleading — classify is needed for colouring regardless.)
- **V11. Chipload heat-map inputs re-scan the trace per upload, uncached**
  (`gpu_upload.rs:828-831` → `display.rs:82-107` full scan into nested HashMaps) — the
  timeline caches the identical computation (`cached_chipload_envelopes`); route both
  through it.
- **V12. Collision-marker density O(n²)** (`gpu_upload.rs:705-718`) — fine at 0–10
  collisions, 10⁶–10⁷ dist tests on pathological projects. 5 mm spatial hash → O(n).
- **V13. STEP/enriched mesh: 3 unindexed verts/tri + linear selected_faces test per tri**
  (`mesh_render.rs:223-264`; STL path at :64 is properly indexed). **Correctness bug:**
  `last_hover_face` written (`viewport.rs:296,332`) but nothing sets pending_upload → BREP
  hover highlight only appears when something else redraws, and always one frame stale
  (upload runs before draw). Fix flag only AFTER V8, plus HashSet for selected_faces.
  **FIXED in wave 2** (`DELTA_viz_w2.md`): hover now sets `pending_upload` on a *change* of
  hovered face, paired with a repaint conditional on the same change (G-LV.1 intact); with
  V8's keys in place that rebuilds one enriched mesh and nothing else, and a pointer move
  *within* a face rebuilds nothing. HashSet landed. **Two corrections:**
  (a) *"always one frame stale" is resolved to a one-repaint lag, not eliminated.* The
  upload pass runs near the top of `update()` and the viewport draws below it, so frame N's
  hover reaches the GPU at the top of frame N+1. The highlight now appears and is correct
  while the pointer rests; killing the last frame of lag needs a second upload pass after
  `handle_events`, which reorders the frame for all ~40 upload sites — judged a worse trade
  than 16 ms on a hover tint. (b) *Indexing the enriched path is structural, not contained
  — do not attempt it as written.* That path is flat-shaded (per-triangle face normal) and
  per-face-group coloured, so two triangles share a vertex only when they share both normal
  and face group; indexing means either switching to smooth shading (wrong for a BREP part,
  where face boundaries should read as creases) or a (position, normal, colour) dedupe pass
  whose hashing cost on a 661k-tri mesh plausibly exceeds the VRAM saved. The STL path is
  indexed precisely because it *is* smooth-shaded.
- **V14. Properties panel per frame:** rest-grid coverage full surface_z scan per PEER
  toolpath (`properties/mod.rs:461-466` → `rest_field.rs:249-251`; ~1.1M float checks/frame
  worst case) + deep clones of DXF layers/drill_targets (:533-537). Cache footprint area on
  RestGrid at construction; pass slices.

## Tier 3 — dexel→mesh generation

- **V15. Full serial MC + 3 more full passes per scrub step** (`app/simulation.rs:244-335`):
  `z_grid_marching_cubes` (whole grid, no rayon, no dirty regions) → transform pass →
  compute_sim_colors (whole new Vec; Solid mode is a pure repack copied AGAIN by
  build_vertex_data) → build_vertex_data (another Vec + normals accumulator + 2 passes).
  ~46 MB churn per step at 800². Playback IS well-throttled (50 ms interval, cheap preview
  mesh, scrub deferral) — bites on pause/single-step/checkpoint load.
  Fix: rayon MC rows; persistent scratch Vecs; fold colours into build_vertex_data;
  longer-term dirty-rect the grid. Also `update_colors_if_changed` (`sim_render.rs:365-394`)
  recomputes ALL normals for a colour-only change — split colour buffer or strided write.

## Tier 4 — smaller

- cached_load_report / cached_chipload_envelopes clone on cache hit, called 3×/frame → &T.
- hotspots snapshot + sort_by per frame for 10 rows, even with header closed
  (`sim_diagnostics.rs:792-814`); per-span hotspot/issue filter+sort uncached (:1575-1602);
  six issue passes for badges (:650-659).
- `outline_kind_for_toolpath` scans all spans per row (`sim_op_list.rs:336→463`).
- O(N²) trace-availability test in card loop (`toolpath_panel.rs:419-424`) — hoist.
- Tool-library file I/O re-read every frame a menu popover is open (`toolpath_panel.rs:222,229`).
- automation::record clones snapshot BTreeMap per call — only 6 sites, NOT worth fixing,
  noted so it isn't scaled up.
- Screenshot paths are CPU rasterisers off the wgpu path entirely; only cost is running
  synchronously on the GUI thread.

## Already good (viewport) — don't regress

Upload gating via take_pending_upload (~20 event sites, consistent); repaint scheduling
clean (no unconditional request_repaint; G-LV.1 Wayland fix intact; 100 ms MCP heartbeat
deliberate + gated); SimMeshChunk capacity-tracked in-place write_buffer + ColorFingerprint;
draw structure excellent (one render pass, ~20 draws, two buffers per toolpath, O(1) scrub
truncation via prefix counts); native LineList (~576 KB/12k moves, honest about line width);
GpuLimits + try_create_buffer + chunked upload + auto-stride guards; offscreen target reuse;
live-sim throttling; the trace-keyed cache pattern (SpanAggregateCache etc.) — V1/V2/V5/V11
are all "the cache exists, the call site doesn't use it"; picking downsampling; timeline
max-per-bucket decimation at 1600 pts.

---

# Consolidated priority (cross-area)

Quick wins (small diffs, no behaviour change):
1. V1 cache simulation_triage; V3 hoist deflection early-out; V4 issues() by reference.
2. S4 partial: delete coverage_max + duplicate playback_lut; S6: opt-in JSON dump, no trace
   clone; S7 de-sqrt.
3. G2 hoist 2.5D geometry out of the Z loop (pure refactor, ÷L on all 2.5D ops).
4. G1 push-cutter row query (bounded change, existing bench).
5. G3 drop_cutter early-outs; G4 Polygon2 bbox cache.

Structural (bigger, mostly metric-neutral):
6. S2 tile max-top early-out; S3 row-band parallel stamping (bit-identical).
7. S4 rest: selective prior_stocks, checkpoint as top-Z field.
8. V8 per-object dirty flags (+unblocks V13 hover fix); V9 shift at vertex emission.
9. G8 index/silhouette/mesh once per (model,setup); G5 shared NN orderer.

Metric-changing / long horizon (baseline benches FIRST):
10. S1 swept-volume stamping (+ sentry re-baseline).
11. S5 fixpoint prefix memoization.
12. S8 SoA rays; V15 parallel MC + dirty rects.

# Phase 0 — Baselines (do this before ANY fix lands)

Two existing bench targets (`perf_suite`, `classification`) cover primitives only; none
of the top findings above is visible to them. Phase 0 adds perf baselines AND correctness
goldens so every subsequent fix can be verified on both axes: "faster" and "same output".

## 0A. Criterion benches — new target `benches/hot_paths.rs` in rs_cam_core

One bench per finding cluster, sized to run in minutes not hours (synthetic fixtures,
criterion `sample_size` tuned down where needed):

| Bench | Exercises | Verifies findings |
|---|---|---|
| `sim_kernel_lateral` | `simulate_toolpath_with_lut_metrics_cancel`, raster toolpath, Ø6 + Ø12, cs 0.1 & 0.25 | S1a, S2, S3, S7, S8 |
| `sim_kernel_plunge` | same fn, plunge-and-retract-heavy toolpath (project_curve shape) | S1b (by_z subdivision) |
| `sim_e2e_small` | `run_simulation` end-to-end, 3-toolpath project incl. clone/mesh/artifact tail | S4, S6 |
| `gen_depth_stepped` | `pocket_toolpath` (+ profile, zigzag) at L=1 vs L=20, ~1400-vertex marching-squares-scale ring | G2 — ratio ≈ L before fix, ≈ 1 after |
| `gen_waterline_full` | `waterline_toolpath_with_cancel` on terrain mesh, multi-level (existing `push_cutter_batch` only measures one fiber batch) | G1 |
| `gen_contains_point` | boundary clip / `contains_point` on 1400-vertex ring + multi-region RegionSet | G4 |
| `gen_rapid_order` | `optimize_rapid_order` NN pass at 5k and 20k segments; surface_link ordering if callable in isolation | G5, G6 |
| `gen_vcarve_field` | v-carve sampling loop on a lettering-like multi-hole polygon | G9 |
| `viz_triage_build` | `ProjectSession::simulation_triage` + `MeasurabilityReport::from_trace` on a synthetic 600k-sample trace | V1 (core side) |

Existing benches to keep as-is: `arc_fitting` (already feeds the quadratic branch via
`make_linear_toolpath`), `spatial_index/build_terrain`, drop-cutter + classification
(check `classification.rs` covers the contact-math 4/5 — supplement only if not).

Viz per-frame findings (V2–V7, V14) are NOT criterion-benchable in the GUI loop; they get
verified by the cache-hit design itself + the core-side `viz_triage_build` bench. A
frame-time tracing span pass is optional follow-up, not Phase 0.

## 0B. Correctness goldens

- **Golden sim-metrics snapshot** (new integration test, `tests/` + committed JSON):
  fixture project → per-toolpath aggregates — removal volume, air-cut % (both
  denominators), engagement summary, collision/holder counts, sample count, per_kinematics
  totals — compared within stated tolerances. This is the net that lets S2 (exact tile
  skip) and S3 (bit-identical bands) claim "metric-neutral", and the thing that gets
  DELIBERATELY re-baselined for S1.
  **EXTENDED (wave 1 SIM, `b2a5e661`).** The Phase 0 fixture is 2.5D only and emits **no
  `CutKinematics::Arc` sample at all** — a net that pins only that arm cannot cover the arc
  or ball-tip stamp branches S1/S2/S3 reshape. A second arm (hemisphere mesh, Ø6 ball nose,
  drop cutter + waterline with `arc_fitting` on, 0.5 mm cells) now sits beside it in
  `perf_golden_sim_metrics_3d.json`, and **both** arms record `per_kinematics`.
  `three_d_arm_covers_arc_and_helix_kinematics` asserts the Arc and Helix classes stay
  populated, so the arm cannot decay back into a Linear-only fixture while staying green.
  Measured on the new arm: 765 Arc / 4402 Helix samples, avg engagement 0.265, 3423.7 mm³.
  Note the 2.5D arm already emitted `Helix` (ramp entries); `Arc` is what the 3D arm
  uniquely adds.
- **Toolpath-fingerprint equality for G2**: depth-stepped fixtures through the existing
  fingerprint infra — hoisted geometry must emit byte-identical moves.
- Existing nets stay load-bearing: F-XXX sentries, `_litmatrix_*`, 56 param sweeps.

## 0C. Wanaka wall-clock protocol (end-to-end, real workload)

Criterion can't hold the 40-minute workload. Separately record, on an idle machine,
wanaka200 (`planning/airrun_2026-08-19/wanaka200.toml`): `generate_all` (fixpoint) total +
per-round, `run_simulation` at the run's resolution, per-toolpath generate times from
`generation_status`/logs. Two runs, numbers into `BASELINES.md` with machine state noted.
Re-run after each structural fix (S2/S3/S5, G1/G2/G8).

All baseline numbers land in `planning/perf_review_2026-08-19/BASELINES.md`.

## Fix-verification loop (per finding, once Phase 0 is in)

1. baseline bench number (from BASELINES.md) → 2. implement fix → 3. goldens + sentries +
targeted `cargo test -p rs_cam_core` green → 4. re-run the finding's bench, record delta →
5. for structural fixes, re-run 0C wanaka wall clock. Metric-changing fixes (S1) get an
explicit re-baseline commit of the golden JSON with the reason in the message.
