# Adaptive clearing algorithm review — 2026-06-12

**Trigger:** wanaka200 roughing quality is noticeably worse than wanaka100, raising
the suspicion that the adaptive/AgentSearch stack was overfit to wanaka100.
**Scope:** the per-slice 2D clearing engine (`adaptive/`), the 2.5D slicing layer
(`adaptive3d/`), and what it would take to get "really nice adaptive passes across
all shapes/sizes of cut, minimum travel, as-constant-as-possible load."
**Method:** full code read of `adaptive/search.rs` + `adaptive_shared.rs`, agent-mapped
reads of `adaptive/{mod,path}.rs` and `adaptive3d/{mod,path,clearing,search}.rs`.

---

## 1. What the engine is today

Two lineages coexist:

- **The 2D agent** (`adaptive/`): a reactive greedy walker. Material lives in a
  raster grid (`cell_size = max(R/6, tolerance)`). Each step the agent samples
  candidate directions, measures engagement by **disk-area fraction**
  (`compute_engagement`, `search.rs:25`), scores
  `|eng − target| + 0.03·|Δangle|/π + wall_bias`, steps `R/2`, repeats. Entry is
  a helix at the distance-transform maximum (pass 1) then a boundary walk picking
  the highest-engagement wall point (subsequent passes). Degenerate endings are
  patched by `ResidueMop` / `ContourParallelHybrid` cleanup strategies and a
  `forced_clear` 2R grid-zap on 15 idle steps. This is architecturally the
  FreeCAD-Adaptive / libactp lineage (the ±5% band comment cites libactp).
- **The 3D EDT family** (`adaptive3d/clearing.rs`): per Z slice, a boolean material
  grid from the live tri-dexel stock → true Euclidean distance transform →
  marching-squares iso-contours at stepover increments. `ContourParallel`
  (the **default**) uses fixed offsets + an engagement-blind raster cleanup;
  `Adaptive` adds a curvature-corrected per-cell offset
  (κ-field, `clearing.rs:902-1080`); `AgentSearch` extracts slice polygons and
  delegates wholesale to the 2D agent, then lifts to 3D.

So per slice you currently get either fixed-offset contours with no engagement
control (default) or the greedy agent (AgentSearch).

---

## 2. Findings

### F1 — Units mismatch: target is an *angle* fraction, measurement is an *area* fraction (root-cause class)

`target_engagement_fraction` (`adaptive_shared.rs:6`) derives the target from the
**engagement angle**: `α = acos(1 − s/R)`, target `= α/2π`. That is the fraction of
the cutter *circumference* in contact — the correct load proxy (chip cross-section,
cutting-force arc).

`compute_engagement` (`adaptive/search.rs:25`) measures the fraction of the cutter
*disk area* lying in material — a different physical quantity. The two are then
compared directly in `search_direction_with_metrics` (target_frac flows from the
angle formula at `path.rs:179` into the area-fraction comparison at
`search.rs:200-205`).

These functions do not agree except by coincidence. Steady-state model (cutting
alongside the previous swath, own swath cleared behind): the material in the disk
is a circular segment of height `s`, roughly halved by the own-swath clearance.

- Segment area fraction at `s`: `f(s) = [acos(1−s/R) − (1−s/R)·√(2s/R − s²/R²)]/π`
- Angle fraction at `s`: `g(s) = acos(1−s/R)/2π`

| commanded s | angle target g(s) | area reading (between f(s)/2 and f(s)) | effective s the controller converges to |
|---|---|---|---|
| 0.2 R | 0.103 | 0.026–0.052 | ~0.32–0.52 R (**1.6–2.6× over**) |
| 0.5 R | 0.167 | 0.098–0.196 | ~0.45–0.73 R (up to ~45% over) |
| 2.0 R (slot) | 0.500 | ~0.5 | ≈ matches (the one place they agree) |

At typical adaptive stepovers (10–25% of diameter) the controller seeks an area
fraction that corresponds to a **substantially wider radial cut than commanded** —
i.e. systematically higher load than the user dialed in, with the error *varying by
stepover and by local geometry* (corner wrap, spiral curvature shift the segment
shape). This alone produces "load is not constant and not what I asked for", and it
explains why per-model retuning (wanaka100 constants) appeared to work: the
constants were absorbing a unit error.

**Fix direction:** measure the **leading-arc engagement angle** — sample the cutter
circumference (or just the leading semicircle) at the next position and count the
material arc — and compare against `α/2π` directly. Same grid, same cost order,
correct units. (The docstring's claim that area sampling "is more precise than
circumference-only sampling" is backwards: it is more *stable*, but it measures the
wrong quantity against this target.)

**Stage 0 verification (2026-06-12, `experiment/adaptive-spiral`):**
`compute_engagement_arc` landed behind `EngagementMeasure::LeadingArc`
(default `DiskArea`, untouched behavior). Closed-form oracle tests on an
exact-lattice steady-state scene pass at s ∈ {0.17R, 0.5R, R, 2R}; the
disk-area measure reads <70% of the α/2π target on the identical scene
(`adaptive/search.rs`, `engagement_measure_tests`). A/B sweeps
(`sweep_adaptive_engagement_measure`, `sweep_adaptive3d_engagement_measure`,
artifacts under `target/param_sweeps/*/engagement_measure/`) confirm the
prediction's magnitude: on the rect-pocket fixture at stepover 2.0
(s ≈ 0.63R), DiskArea cleared the pocket with **551 mm** of cutting where
LeadingArc needs **835 mm** — the historical controller was converging on
roughly **1.5× the commanded stepover**. The 6-view stock renders match
(both fully clear), so the delta is pass spacing, not coverage. On the
hemisphere AgentSearch A/B: cutting +13%, rapid distance −4%, rapid
fraction 0.55 → 0.51. Note the corollary: LeadingArc at the same commanded
stepover cuts *more conservatively* (correct load, longer cycle time) —
users effectively had a hidden ~1.5× stepover multiplier, so feeds tuned
under DiskArea bake that in (cf. F-034 anchor drift).

### F2 — Reactive greedy architecture has no global structure

The agent decides one step at a time with no lookahead and no decomposition of the
region. Documented consequences in the code itself:

- Sawtooth/wiggle at spiral end (comment at `path.rs:397-413`; convergence detector
  tried and removed).
- `forced_clear` 2R grid-zap when cornered (`path.rs:523`) — material is marked
  cleared without being cut; residue handled later by mop passes.
- Pass restarts from the boundary wall: every `find_entry_point` re-entry is a
  retract/link + plunge + wall start, not a continuation tangent to the previous
  wrap.
- Two whole cleanup strategies (`ResidueMop`, `ContourParallelHybrid`) exist solely
  to repair degenerate agent output.
- `best_any` fallback (`search.rs:325`): when no in-band direction exists, the
  lowest-*score* direction is accepted regardless of engagement — at a concave
  corner this accepts full-slot engagement rather than inserting a load-limited
  maneuver. This is the corner load spike.

A greedy controller can hold a band in open space; it cannot guarantee load or
coverage in corners, strips, or annuli. That guarantee has to come from
construction, not feedback (see §3).

### F3 — Travel is leaked at every seam

- No tour over regions in 2D (entry = "highest engagement anywhere on the
  boundary", greedy max). 3D has an O(n²) NN tour per level, nothing else.
- Pass restarts (F2) each cost retract + reposition + plunge. The wanaka Back Rough
  symptom in the code comments — *149 plunges, 90 of which cut ≤ 10 mm*
  (`clearing.rs:1534`) — is this leak made visible.
- `is_clear_path` permits stay-down links through **up to 20% material** at full
  feed (`path.rs:66`); these are real cuts at link feed, uncontrolled load, and
  counted as cutting moves.
- `is_clear_path_3d` (`adaptive3d/search.rs:156`) takes `_z_level` and
  `_stock_to_leave` and **ignores both**; block test is `mat_z > travel_z + 1.0 mm`
  hardcoded — a ridge 0.99 mm above the cutter line doesn't count as blocked.
- `MIN_AIR_RUN_MM = 70.0` (`clearing.rs:1942`) — the link/retract crossover is
  hardcoded from one tool/feed/rapid combo; the actual feed and rapid rates are
  available at planning time and the crossover should be computed.

### F4 — Two distance fields, the worse one in the hot path

The 2D engine's `compute_boundary_distances` is **4-connected BFS — a Manhattan
metric** (`material_grid.rs:307`), used for the wall bias, the gradient-mode
switch, and the strip-centerline follower. The code itself notes it over-reads ~50%
at 45° corners (`path.rs:1082`). Meanwhile `adaptive3d/clearing.rs` has a true
Euclidean `distance_transform_2d`. The 2D engine should use the EDT; the
strip-follower and narrow-region gate inherit the corner error today.

### F5 — Resolution and cost cliffs

- Engagement precision is grid-bound: at `cell_size = max(R/6, tolerance)` a 3 mm
  tool at 0.2 mm tolerance has ~7 cells across the radius → quantized engagement,
  truncated disks at grid edges over-read.
- `pass_endpoints` is never pruned; entry scans are O(passes × cells)
  (`search.rs:358-408`) and grow over the run.
- AgentSearch pre-shrinks slice polygons by 0.3 mm inset + 0.3 mm RDP
  (`clearing.rs:1762`) — up to ~0.6 mm boundary error per slice, leaving ribs the
  mop must clean.
- F-038 forecasting dry-runs the full 2D engine on micro-regions *before* deciding
  to skip them (`clearing.rs:1576-1590`) — the expensive work happens for exactly
  the regions that get dropped.

### F6 — Overfit-to-wanaka inventory (confirming the suspicion)

Constants and features that trace to one terrain model, per code comments:
`min_region_area` formula + "terrain.stl micro-peaks" (`clearing.rs:1401-1425`),
`PERIMETER_INSET_MARGIN_MM = 0.25` from "O5b / wanaka 18 mm-DOC"
(`clearing.rs:1542`), `min_region_cut_length_mm = 5.0` from "Wanaka Back Rough
perimeter micro-plunge" (`mod.rs:179`), the `boundary` field from "O5b wanaka
repro" (`mod.rs:143`), `MIN_AIR_RUN_MM = 70` from a 6 mm/3150/5000 combo,
waterline `30°`/`1/3-steep` filters, flat-area `2%` histogram bin. None are wrong
per se, but they are point-calibrations papering over the structural issues above —
which is why wanaka200 (different scale, holes, different region statistics)
regressed.

---

## 3. Recommendation — constructive clearing on the EDT, not a better agent

The modern algorithm (HSMWorks/Fusion "Adaptive Clearing", and the academic line:
Bieterman & Sandström 2003; Held & Spielberger, *A smooth spiral tool path for high
speed machining of 2D pockets*, CAD 2009/2011) is **constructive**: derive the
global structure of the region first, then emit a path whose engagement is bounded
*by construction*. The feedback agent is then unnecessary — and so are its mops,
forced-clears, and per-model constants.

The decisive observation: **rs_cam already owns ~70% of the machinery**, in the 3D
clearing layer of all places — true EDT, marching-squares offset families,
curvature-corrected variable offsets (`Adaptive` strategy), helix-at-DT-max entry,
live rest-material from the dexel stock, NN region ordering. What's missing is the
connective tissue:

1. **One spiral per region instead of N nested contours.** Take the EDT offset
   family (already produced) and morph adjacent offsets into a single continuous
   spiral (Held–Spielberger interpolation between consecutive wavefronts; for
   simply-nested bands it's a radial blend, islands split the family into bands
   linked by short stay-down moves). This kills almost all within-region travel:
   no per-ring entry, no restarts, one plunge per region.
2. **Engagement bounded by construction, locally widened/narrowed.** Spacing
   between successive wraps ≤ the stepover that yields the target α (exact formula,
   F1). The κ-corrected offset already in `clearing.rs:902-1080` is precisely the
   right correction — move it from "alternate strategy" to the spiral generator's
   spacing rule. Concave corners where even corrected spacing would exceed α-max get
   **trochoidal inserts**: circular loops sized from local width (the EDT gives it
   directly) so instantaneous α stays in band. This is the corner answer that
   `best_any` (F2) cannot give.
3. **Narrow regions ride the medial axis.** local width < 2R + s → trochoidal
   slotting along the EDT ridge (the strip-centerline gradient follower already
   approximates the ridge; with the true EDT it becomes reliable). Replaces the
   narrow-gate → contour-parallel special case and the slot zigzag.
4. **Travel as an explicit objective.** Regions per slice: NN tour + 2-opt
   (region counts are small; this is cheap and removes NN ping-pong). Link choice
   (stay-down vs retract) computed from actual feed/rapid rates instead of
   `70 mm`/`6R` constants. Inter-level: `RegionOrdering::ByArea` (depth-first per
   pocket) should be the default for multi-region parts.
5. **Slice-to-slice coherence.** Level N's new material is approximately the band
   between level N's contour and level N−1's floor — an annular offset family the
   spiral generator handles natively. The dexel-backed boolean grid already gives
   exact rest material; the generator just needs to not treat each slice as a
   fresh pocket.
6. **Feed modulation is the second control layer.** Geometry can hold α within
   ±20-ish%; the residual (terrain lift, corner entry/exit transients) should be
   flattened by the existing feed-modulation pipeline (F-036) driven by
   **planner-predicted** per-move engagement, not just simulator measurement. Two
   layers — geometry for the coarse shape of load, feed for the residue — is
   exactly what the commercial systems do.

What survives from the current stack: the material grid + dexel rest-material
plumbing (good), helix-at-DT-max entry (already the literature answer), marching
squares + EDT (the foundation), corner arc blending and DP simplification
(post-processing stays), peck plunge / RapidWithFloor / F-038b stay-down (linking
layer stays). What retires: the per-step direction search, residue mops,
forced-clear, the narrow-gate special case, and most of §F6's constants.

---

## 4. De-overfitting: test against generated geometry, not one mountain

The regression net is anchored on wanaka100 artifacts (cycle-time anchor F-034
already drifting). To make "all shapes/sizes" a tested property:

- **Property tests on random geometry**: generated pockets (random simple polygons,
  L/T/U shapes, annuli, thin slots at 1.1–3× tool diameter, island fields,
  Perlin-noise heightfields as synthetic terrain). For each: max instantaneous
  leading-arc engagement ≤ target × 1.15; coverage ≥ 99.x% of machinable area;
  air-cut % and travel/cut ratio below bars; plunge count ≤ regions + k.
- **Planner-side α validator**: the leading-arc measure from F1 doubles as the
  test oracle, cross-checked against the simulator's per-sample engagement vector
  (Step 2 dexel fidelity work) on a few golden cases.
- Keep wanaka100/200 as two *instances* in the suite, not as the calibration.

---

## 5. Staging

| Stage | Work | Risk |
|---|---|---|
| 0 (days) | **DONE 2026-06-12** (`experiment/adaptive-spiral`, commits ab481a6..0c140ef): F1 leading-arc measure behind `EngagementMeasure` flag + A/B sweeps (DiskArea ran ~1.5× commanded stepover); `is_clear_path_3d` three-tier floor predicate; feed/plunge/depth-derived air-run crossover (retired the 70 mm const); true EDT in the 2D engine — which exposed and fixed a real **output-pass corruption bug in `edt_1d`** (in-place write under-reported distances in corner configurations; the default ContourParallel strategy planned on that field); `EndpointGrid` spatial hash for the entry-exclusion scans | Low — bug-class fixes |
| 1 | **DONE 2026-06-13** (overnight session): `PathStrategy2d::ContourSpiral` in the 2D engine (inside-out EDT iso-contour wraps from the helical starter, one continuous stay-down Cut per region; shares narrow gate/starter/residue-cleanup/emission with the agent, falls back to the agent when no starter fits) + `ClearingStrategy3d::ContourSpiral` routed through the AgentSearch slice dispatch + config/catalog/GUI/CLI plumbing. Property harness (`tests/adaptive_property_harness.rs`): 7 generated shapes × both strategies, replayed on an **independent** oracle (own raster, own leading-arc sampler). Stage 1 contract asserted: spiral plunges ≤ 3 (measures 1 everywhere vs agent restarts), rapids ≤ 5% of cut (measures 0), coverage ≥ 0.975 and within 1.5% of agent, p99 + over-fraction never worse than agent. Measured: spiral p99 0.41–0.47 vs agent 0.47–0.49; over-1.3×target fraction e.g. annulus 8% vs 37%, u-shape 22% vs 36%. **Agent baseline empirically confirms F1/F2: p99 ≈ 2.5× target on every shape.** Absolute α-bounds deferred to Stage 2 — measured excursions are concave-corner wrap-around + EDT side-branch first-contact, exactly the trochoid work. A per-point engagement filter was tried and reverted (drops cascade into the next wrap and swiss-cheese coverage — recorded in `adaptive/spiral.rs`). Hemisphere 3D sweep: contour_spiral 2353 mm cutting vs default ContourParallel's 6569 mm at a third of the moves. | Medium |
| 2 | **Trochoidal inserts DONE 2026-06-13**: where predicted leading-arc engagement exceeds 1.2×target along a wrap, circular loops (radius ≈ stepover, pitch 0.6×stepover) biased toward the most-cleared side bound the bite. Harness: over-1.3×target fraction collapsed 8–22% → **2.8–4.1%** (agent: 11–37%), p99 0.41–0.47 → 0.31–0.35, coverage held, still 1 plunge / 0 rapids; cost ≈ 2–2.7× the agent's cutting distance (inherent trochoid trade — feed modulation gets it back on the machine since load is now flat). Absolute harness bars: over ≤ 5%, p99 ≤ 2×target. **Remaining**: strict 1.3×-target p99 blocked on a structural ~1% loop-tangent transient (true cycloid advance, or accept and let Stage 4 feed-mod flatten it); medial-axis trochoids for slot-class regions (slot exempt from load bars meanwhile); island banding not needed so far (annulus covers at 0.995 via lobes + cleanup). | Medium-high |
| 3 | Region tour 2-opt, ByArea default, slice-coherent annulus clearing | Low |
| 4 | Planner-predicted engagement → feed modulation hookup; retire agent path + mops once sweeps show parity | Medium |

The agent stays available behind its flag until Stage 1–2 beat it on the sweep
suite across *generated* geometry — that's the overfit test.

## wanaka200 head-to-head (2026-06-13, CLI, identical project file)

Both roughs switched to `contour_spiral` vs the original strategies
(`adaptive` EDT on Rough Identity, `agent_search` on Rough Flipped):

| | baseline | contour_spiral |
|---|---|---|
| Verdict | **WARNING: 53.7% air cutting** | **OK** (air-cut 26.0%) |
| Rapid collisions | 0 | 0 |
| Rough Identity rapids | **85,339 mm** | **7,564 mm** (11× less) |
| Rough Identity cutting | 158,251 mm | 299,251 mm |
| Rough Flipped rapids | 8,386 mm | 7,176 mm |
| Rough Flipped cutting | 32,374 mm | 114,872 mm |

Honest ledger: travel and air-cut collapse and the verdict goes clean, but
raw cutting distance roughly doubles (the trochoid trade), so at *equal
feeds* the spiral is ~40% slower wall-clock on the identity rough. The
point of flat load is that feeds need not be tuned for spikes anymore —
Stage 4 (planner-predicted engagement → feed modulation) and a Suggest
re-dial at the now-honest engagement are where the cycle time comes back.
Recommended next validation: cut both on the Shapeoko and compare load
sound/finish, not just wall-clock.
