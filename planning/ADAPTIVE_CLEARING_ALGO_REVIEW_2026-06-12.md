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

## Stage 4 implementation spec (2026-06-14)

Goal: drive the existing F-036/F-039 feed-modulation pipeline with
**planner-predicted** leading-arc engagement so the spiral's flat load
buys back the trochoid cutting-distance trade. The modulator already
raises feed in light engagement — Stage 4 gives it a *correct,
geometry-derived* engagement signal instead of the cylinder-side
`radial_woc_fraction` scalar that reads ~10× low for adaptive ops
(CLAUDE.md caveat), and that today is also off by default.

### The conversion bridge (exact, landed first)

`adaptive_shared::target_engagement_fraction(s, R) = acos(1 − s/R)/2π`
maps stepover → leading-arc fraction `f = α/2π`. Its inverse, expressed
as the radial width-of-cut fraction `a_e/D` the modulator's
chip-thinning model (`1/√woc`) consumes, is:

```
radial_woc_fraction = (1 − cos(2π · f)) / 2,   f ∈ [0, 0.5]
```

Checks: full slot f=0.5 → 1.0; half-immersion f=0.25 → 0.5; air f=0 → 0.
Lives in `adaptive_shared` with a closed-form round-trip oracle test.

### Carrier — geometric sampler, NOT an index-parallel vector

A `Vec<engagement>` parallel to `toolpath.moves` desyncs: lead-in/out
dressups and `arcfit` change move counts, and `simplify_path_3d` (inside
`push_segment_with_stamp`) drops points. Adding a field to
`AnnotatedToolpath` also rippling to ~15 constructors that each reshape
moves. So carry predicted engagement **geometrically**: a small set of
`(x, y, z, f_arc)` frontier samples the spiral already computes
(`compute_engagement_arc` per emitted point, line ~172 of
`adaptive/spiral.rs` — currently discarded after the trochoid decision).
At modulation time the per-move engagement is a nearest-sample lookup
(spatial hash, keyed near in XY at the move's slice Z) — invariant to
simplify/blend/arcfit because it's positional, not index-based.

Why the planner value beats recomputing at assembly time: within one
region the spiral emits a *single* continuous `Cut` path that
`push_segment_with_stamp` only stamps into the dexel *after* the whole
sub-run is pushed, so sampling `material_stock` mid-run reads full
material on both sides → ~full-slot everywhere (wrong). The spiral's
live 2D `MaterialGrid` is the only place that sees the frontier (inner
side cleared by the previous wrap), so the value must originate there.

### Wiring

1. `spiral.rs`: collect the `eng` it already computes into a per-region
   `Vec<(P2, f64)>` and surface it (alongside the slice Z) up through
   the 3D clearing assembly to a per-toolpath sampler.
2. `session/compute.rs::apply_adaptive_feed_modulation`: when a toolpath
   has a planner sampler, build `PerMoveEngagement.radial_woc_fraction`
   from the nearest sample via the bridge above; keep `axial_doc_fraction`
   from the trace. Fall back to the sim-measured aggregation when no
   sampler exists (agent path, finishing ops) — fully back-compatible.
3. Default `adaptive_feed_modulation` stays opt-in; the wanaka200
   head-to-head + sweeps run with it ON to measure recovery.

### Verification (before each commit)

- `adaptive_property_harness`: assert (a) the planner sampler is
  populated and in-band (`f ≤ target × 1.15` off trochoid loops) on the
  generated shapes, and (b) modulated cut time ≤ unmodulated at equal
  safety (load gates still pass).
- wanaka200 CLI head-to-head with modulation ON: cutting-time delta vs
  the equal-feed baseline; confirm 0 rapid collisions and load-gate
  verdict stays OK/clean.
- `param_sweep` + `analyze_sweep.py` parity report on generated
  geometry. The agent path retires only once the spiral wins there.

### Then: Suggest re-dial + agent retirement (Stage 4 tail) → Stage 3.

## Stage 4 v1 results (2026-06-14) — premise partly falsified

Carrier-free v1 landed: the modulator (`apply_adaptive_feed_modulation`)
floors per-move radial WOC at the planner target for `ContourSpiral`
lateral clearing cuts (`max(sim_measured, stepover/D)` via the F1
bridge). Gated to the spiral strategy; `max` keeps any genuine spike the
sim resolves. wanaka200, identical feed/LUT/kinematics, only
`clearing_strategy` differing (D=6 mm, stepover 1.2 → planner floor
radial = 0.20 vs the sim's under-reported avg 0.071):

| config | cycle time | cut mm | rapid mm | air % | collisions |
|---|---|---|---|---|---|
| spiral, **mod ON (safe)** | **21 533 s** | 421 704 | 30 931 | 17.3 | 0 |
| original (adaptive+agent), **mod ON (safe)** | **17 839 s** | 230 713 | 86 995 | 39.6 | 0 |
| spiral, mod OFF (reckless 6000 mm/min) | 14 338 s | 421 704 | 30 931 | 26.0 | 0 |

**Honest finding.** At equal *safe* (modulated) feeds the spiral is
~21 % **slower** wall-clock than the spiky strategies. Chip-thinning
feed elevation does **not** recover the trochoid's 1.83× cutting-distance
trade (421 704 vs 230 713 mm) — the distance dominates. The Stage 4
premise ("feed modulation is where cycle time comes back") is **partly
false** on this part.

What Stage 4 v1 *does* deliver, and why it still ships:

1. **Safety / correctness.** Without the planner floor, modulation runs
   the spiral on the sim's ~10×-low engagement, so chip-thinning
   (`1/√woc`) over-estimates the safe feed by √(0.20/0.071) ≈ 1.7× —
   the spiral would cut at ~1.7× the real chip load (tool abuse). The
   floor makes the modulated feeds physically correct.
2. **The spiral's real wins are travel + load shape, not wall-clock:**
   2.8× less rapid travel (30.9 k vs 87 k mm), lower air-cut (17 % vs
   40 %), and constant spike-free load (the Stages 0-2 result) — better
   finish / quieter cut / less deflection risk, verdict OK with 0
   collisions.

**Implication for the product decision** (carry into Stage 3's ByArea
default question + the agent-retirement gate): the contour-spiral should
**not** be promoted to default on a wall-clock basis. Promote it where
load constancy / finish / travel matter more than raw cycle time, and
keep the spiky strategies available where wall-clock dominates. The
agent path does **not** retire on these numbers.

Open levers that could still close the wall-clock gap (future work, not
v1): reduce the trochoid distance penalty (only insert loops where
engagement *actually* exceeds cap per-point rather than per-wrap — needs
the per-point planner sampler from the spec above so the modulator and
the trochoid trigger share one engagement field), and the per-point
sampler would also let feeds rise through the genuinely-light frontier
samples the uniform floor currently holds at target.

## Stage 4 v2 results (2026-06-15) — per-point sampler: no wall-clock gain

Built the per-point planner-engagement sampler end to end: `spiral.rs`
collects the leading-arc engagement it already computes at each emitted
cut point → threaded through the 2D engine sink → 3D slice assembly
(positional `(x,y,z)` lookup, survives RDP / arc-fit / dressup / TSP) →
`Adaptive3dSegmentsResult` → `AnnotatedToolpath.planner_engagement` →
the modulator looks it up per cut move (uniform target floor as
fallback). Verified flowing: wanaka200 carried **824 202** planner
samples (612 450 + 211 752) into modulation. Gate green (clippy
`-D warnings`, fmt, full `rs_cam_core` suite, new sink unit test).

wanaka200, modulation ON, identical inputs:

| config | cycle time | Δ vs v1 |
|---|---|---|
| spiral **v2 (per-move sampler)** | 21 533.16 s | **+0.1 s (+0.0005 %)** |
| spiral v1 (uniform target floor) | 21 533.06 s | — |
| original (adaptive+agent) | 17 839 s | −17 % |

**Finding: the per-point sampler closes none of the gap.** v2 and v1 are
identical to 0.1 s, and every per-toolpath metric is byte-identical. The
reason is structural: the constructive spiral holds leading-arc
engagement at *exactly* the commanded target on its steady wraps
(spacing = stepover ⇒ α = target by construction), so the per-move
planner value equals the uniform floor on the overwhelming majority of
cut moves — there is no genuinely-light frontier to feed up and no spiky
region to feed down beyond the ~4 % trochoid samples, whose feed
reductions are negligible in aggregate. Feed modulation cannot recover
the ~21 % wall-clock gap because the load is already flat at target; the
gap is the trochoid's 1.83× **distance**, not feed.

What v2 leaves in the tree: a correct, tested per-move planner-engagement
channel (trochoid zones now feed for their actual engagement, a small
safety refinement) and the accurate foundation for any future strategy
whose load is *not* uniform. But on the contour-spiral itself it buys no
wall-clock, so the simpler v1 uniform floor is functionally equivalent.

**Decision pending (kept on `experiment/adaptive-spiral` either way):**
keep the per-point sampler for correctness + foundation, or revert the
~7-file threading and retain the leaner v1. The real lever for the
spiral's wall-clock is **reducing the trochoid distance** (cycloid
advance / wider cap), not feed modulation — that's the next experiment
if cycle time must improve.

## Stage 4 trochoid-cap sweep (2026-06-15) — the gap IS recoverable

Made the trochoid trigger tunable: `AdaptiveParams.trochoid_cap_mult`
(loops fire above `target × mult`, clamped 0.45; default 1.2 = prior
behaviour). Exploratory harness sweep `trochoid_cap_distance_load_tradeoff`
(`#[ignore]`, run with `--ignored --nocapture`) maps cutting distance vs
load as the cap relaxes (target α/2π = 0.190):

| shape | cap 1.2 (cut / p99 / over%) | cap 2.0 | cap 3.0 | agent |
|---|---|---|---|---|
| square60 | 3023 / .328 / 3.9 | 2033 / .391 / 6.9 | 1631 / .422 / 9.8 | 1621 / .469 / 10.9 |
| star_a | 2266 / .336 / 4.1 | 1092 / .414 / 9.3 | 847 / .445 / 13.1 | 680 / .477 / 16.6 |
| star_b | 2595 / .312 / 3.8 | 1232 / .391 / 9.4 | 999 / .406 / 12.5 | 804 / .477 / 17.3 |
| l_shape | 2166 / .312 / 3.1 | 1627 / .375 / 6.7 | 1211 / .430 / 11.2 | 795 / .484 / 25.4 |
| u_shape | 2122 / .320 / 2.8 | 1704 / .367 / 6.6 | 995 / .430 / 20.3 | 689 / .484 / 36.0 |
| annulus | 2723 / .352 / 3.6 | 2052 / .383 / 5.7 | 1649 / .406 / 8.0 | 1019 / .492 / 37.4 |

Coverage holds ~constant (0.978–0.995) across the whole sweep. cap ≥ ~2.4
saturates (the 0.45 clamp binds → cap 3.0 ≈ cap 10.0 ≈ trochoids nearly
off). narrow_slot is flat (routes to contour-parallel, no trochoids).

**Finding: the spiral's wall-clock gap is recoverable via geometry, not
feeds.** At **cap ≈ 2.0** cutting distance drops **30–50%** vs the 1.2
default while p99 load stays at ~0.37–0.41 — still comfortably *under*
the agent's 0.47–0.49 — and the spiral keeps its stay-down / low-rapid
wins. At cap 3.0 the distance approaches the agent's outright (square60
1631 vs 1621) at lower peak load and far lower over-target fraction. The
1.2 default was tuned for flattest-possible load; **cap ~1.5–2.0 is the
balanced knee** — most of the speed, load still flatter than the spiky
strategies.

This reframes the Stage 4 v1/v2 conclusion: feed modulation can't recover
wall-clock (load already flat), but **relaxing the trochoid cap can** —
it trades a little load-constancy for a large distance cut. Recommended
next: wire `trochoid_cap_mult` to `Adaptive3dConfig` (currently hardcoded
1.2 in the 3D slice), default ~1.6, and re-run the wanaka200 head-to-head
to confirm the 3D wall-clock recovery. The load bars in the gated harness
test stay on the 1.2 default; the sweep documents the speed band above it.
