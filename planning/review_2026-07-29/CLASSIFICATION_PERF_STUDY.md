# M3 — Optimising the classification grid without changing its answer

Research deliverable for `TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` item **M3**.
C-sequence wave 7a, 2026-08-02. HEAD at start: `812c86a`.

**Status: SHIPPED.** Wave 7a (below) was research only. **Wave 7b
(2026-08-02) switched production**, under the human checkpoint ruling
recorded in §10: *"Adopt, COLUMNS-gated"*. Production classification now runs
[`ClassificationSampler::PRODUCTION`] = `TileRaster`; the shipped drop-cutter
probe remains selectable as the fallback and as the sentries' oracle. §10 is
the implementation record — the eight §9.3 gates and how each was met, the
decisive COLUMNS A/B, the re-earned timings, and what moved downstream.

Read §1–§9 as written: they are the wave-7a research, deliberately left in
their original tense so the decision can be re-audited from the evidence that
was in front of it.

Instruments, all committed:

| what | where | how to run |
|---|---|---|
| baseline timings | `crates/rs_cam_core/benches/classification.rs` | `cargo bench -p rs_cam_core --bench classification`; `RS_CAM_M3_HEAVY=1 …` for the real fixture |
| equivalence + the tables below | `crates/rs_cam_core/tests/classification_strategy_m3.rs` | fast rows: `cargo test -p rs_cam_core --test classification_strategy_m3`; study rows: `… --release … -- --ignored --nocapture --test-threads=1` |
| candidate implementations | `crates/rs_cam_core/src/classify_probe.rs` | — |
| pathological fixtures | `crates/rs_cam_core/tests/common/meshes.rs` | — |

Commits: `efabeec` (baseline bench), `05cac97` (candidates + harness),
`6c73810` (this document + the orchestration-log entry).

---

## 1. What the classifier is, and what it actually computes

`finish_setup::build_classification_surface_with_policy_and_cancel` builds the
grid the finish planner decomposes into slope bands. Per cell it runs a
**drop-cutter query with a Ø0.05 mm ball probe** plus a **coverage predicate**,
then derives slopes with the max-of-one-sided-gradients stencil.
`unified_finish.rs` is its only production caller.

Two facts about it, both load-bearing.

**It is two spatial-index queries per cell, not one.**
`dropcutter::sample_grid_cell` calls `SpatialIndex::query(x, y, r)` for the
contact test and `SpatialIndex::query(x, y, 0.0)` again for coverage. At 849²
that is 1 441 602 queries, and each allocates a result `Vec` plus a dedup
bitset `vec![0u64; total_triangles / 64]` sized by the whole mesh (26.9 KB on
the 215 k-triangle terrain).

**It samples the probe's tool-centre surface, not the model surface.** For a
facet of unit normal `n` the drop-cutter puts the CL at
`z_plane(x, y) + R·(1 − n.z)/n.z`. With `R = 0.025 mm`:

| facet slope | CL offset above the true surface |
|---|---|
| 0° | 0 µm |
| 45° | 10 µm |
| 60° | 25 µm |
| 80° | 119 µm |
| 89° | 1 407 µm |

The offset is **slope-dependent**, so it does not cancel in a gradient — it
adds gradient of its own wherever slope varies. The classification cell at
`cusp/4` for the shipped Ø1-tip taper is 0.125 mm, so a 10–25 µm slope-varying
offset is 8–20% of a cell in Z. §5 measures what that does; §5.3 proves it is
the whole of the divergence.

---

## 2. A pre-registered hypothesis, and its refutation

Before choosing an algorithm the obvious suspect was the per-query allocation:
1.44 M queries × 26.9 KB = **38.8 GB of zeroed memory** per 849² classification
of the terrain fixture, i.e. a mesh-sized cost per cell in an algorithm whose
per-cell work is otherwise a handful of triangle tests.

Candidate 0 exists to test exactly that, and the answer is **no**.

| terrain 849² | wall s | CPU s |
|---|---|---|
| shipped | 2.990 | 40.70 |
| shipped + scratch query (candidate 0) | 3.034 | 40.77 |

Zero measurable gain. The reason, visible only once it was measured: at the
probe's 0.025 mm radius against `build_auto`'s 0.61 mm index cells, virtually
every query lands in a **single** index cell, and a single-cell query's dedup
bitset is written to only for the triangles it finds. The allocation is 26.9 KB
of `malloc` + `memset` that the allocator serves from the same recycled block
every time — a few percent, not a bottleneck. The criterion `query_only` row
puts the whole query cost (traversal *and* allocation) at ~11.3 ms serial
against ~50 ms of classifier CPU for the same 143² synthetic grid: **roughly a
fifth of the work is querying; the rest is drop-cutter contact math** — facet,
then three vertex drops and three edge drops per candidate triangle, each with
divisions and square roots.

This is recorded rather than quietly dropped because it is the reason the
recommendation is not "tidy the allocations": there is no version of that
change which reaches M3's 2× bar.

---

## 3. The candidates

Numbering follows the plan's list. Candidates **4 (cached fields)** and
**5 (adaptive refinement)** are deliberately not built — §8.

### Candidate 0 — drop-cutter probe + scratch query

Same contact math, same triangle sets, same order; only the two per-query
allocations are hoisted out of the cell loop (`SpatialIndex::query_into` +
`QueryScratch`). Two fast paths, neither of which changes the output: a query
landing in one index cell skips the bitset entirely (that cell's list is
already duplicate-free), and otherwise the bitset is cleared by walking the
hits rather than re-zeroing the mesh.

**Bit-identical by construction**: candidate sets equal index for index (the
bounds arithmetic, including its off-grid clamping quirk, is reproduced
verbatim) and the reduction is `max` of Z, which is order-independent.

*Lineage*: ordinary scratch-buffer plumbing; clearing a dirty bitset by walking
the hit list is the usual alternative to a generation-stamped array.

### Candidate 1 — direct top-surface triangle rasterisation

Walk the faces; for each, take the cell-centre lattice points in its XY
bounding box (rounded outward by one cell — see §6.6), test point-in-triangle,
evaluate the plane Z, keep the **maximum** per cell. Sequential scatter.

Taking `max` rather than filtering to upward-facing facets is what makes
stacked geometry come out right without an orientation test: along a vertical
ray the topmost point of any surface is the topmost point, whatever the winding
of the facet carrying it.

*Lineage*: the classic top-surface z-buffer burn from mesh voxelisation and
terrain DEM rasterisation.

### Candidate 2 — vertical ray / topmost face via the spatial index

Per cell, take the triangles registered in the **single** index cell containing
the point (a vertical ray can only pierce triangles whose XY bbox contains it,
and `SpatialIndex::build` registers every such triangle in that cell), test
point-in-triangle, keep the maximum plane Z. No contact math, no dedup, no
allocation — `cell_triangles_at` returns a borrowed slice. Gather-shaped, so it
parallelises over cells with no write conflicts.

*Lineage*: point-in-triangle ray casting against a uniform-grid broadphase.

### Candidate 3 — tile-parallel rasterisation

Candidate 1 over disjoint 64 × 64-cell output tiles. Each tile gathers its own
candidate triangles from the index once, rasterises into its own buffer, and
the buffers are stitched. No two threads ever write the same cell, so the
`max`-reduce needs no atomics and does not depend on scheduling.

*Lineage*: binning by output is the standard way to parallelise a scatter;
tiled software rasterisers are the reference shape.

---

## 4. Timings

Release, `--test-threads=1`, one arm at a time, 24-core machine.

**Machine state, honestly:** a foreign `sysml` workspace test run held the
machine for the first ~2 h of the session and every measurement below was taken
**after** it exited (`pgrep` clear, verified before the run started). Swap was
full (8/8 GiB) throughout the session and stayed full during the run; RAM was
24–29 GiB available. The absolute walls are therefore trustworthy to maybe
±10%, and every ratio below is far larger than that.

**A measurement that had to be thrown away.** The first release run took its
timings under cargo's default two test threads. `cpu_time()` reads
`/proc/self/stat`, which is process-wide, and rayon happily gave both tests the
same 24 cores — so the two timing tests measured each other, producing CPU/wall
ratios of 14–25× alongside sub-millisecond walls for arms that cannot be that
fast. The numbers below are from a re-run; the harness now takes a
`TIMING_LOCK` so the mistake cannot recur silently.

### 4.1 Terrain fixture (`tests/fixtures/terrain.stl`, 214 997 triangles, 100 × 100 × 8 mm)

| grid | arm | wall s | CPU s | wall speed-up | peak RSS MB |
|---|---|---|---|---|---|
| 143² | shipped | 0.184 | 1.20 | 1.0× | 93 |
| | + scratch query | 0.161 | 1.20 | 1.14× | 96 |
| | triangle raster | 0.020 | 0.02 | 9.2× | 96 |
| | vertical ray | 0.001 | 0.02 | 184× | 96 |
| | tile raster | 0.027 | 0.05 | 6.8× | 96 |
| 425² | shipped | 0.754 | 10.59 | 1.0× | 98 |
| | + scratch query | 0.735 | 10.24 | 1.03× | 98 |
| | triangle raster | 0.030 | 0.03 | 25× | 101 |
| | vertical ray | 0.006 | 0.11 | 126× | 101 |
| | tile raster | 0.015 | 0.08 | 50× | 101 |
| **849²** | **shipped** | **2.990** | **40.70** | **1.0×** | **102** |
| | + scratch query | 3.034 | 40.77 | 0.99× | 107 |
| | triangle raster | 0.053 | 0.05 | **56×** | 134 |
| | vertical ray | 0.025 | 0.43 | **120×** | 134 |
| | **tile raster** | **0.016** | **0.13** | **187×** | 134 |

### 4.2 Analytic mixed-slope fixture (8 192 triangles)

| grid | arm | wall s | CPU s | wall speed-up |
|---|---|---|---|---|
| 143² | shipped | 0.022 | 0.30 | 1.0× |
| | triangle raster | 0.001 | 0.00 | 22× |
| | vertical ray | 0.000 | 0.01 | >22× |
| | tile raster | 0.001 | 0.00 | 22× |
| 425² | shipped | 0.183 | 3.29 | 1.0× |
| | + scratch query | 0.175 | 3.31 | 1.05× |
| | triangle raster | 0.003 | 0.00 | 61× |
| | vertical ray | 0.004 | 0.09 | 46× |
| | tile raster | 0.001 | 0.01 | 183× |
| **849²** | **shipped** | **0.689** | **13.03** | **1.0×** |
| | + scratch query | 0.694 | 12.80 | 0.99× |
| | triangle raster | 0.005 | 0.01 | 138× |
| | vertical ray | 0.018 | 0.31 | 38× |
| | **tile raster** | **0.003** | **0.03** | **230×** |

Rows at 0.000–0.001 s are at the harness's `Instant` reporting resolution;
their speed-up figures are lower bounds, not measurements. Nothing in the
recommendation rests on them — the 849² rows are the ones that matter and none
of those is sub-millisecond.

### 4.3 Reading the tables

- **Candidate 0 fails the bar outright** — 0.99–1.14×, i.e. nothing, on every
  row. §2.
- **Every direct arm clears M3's 2× target by two orders of magnitude.** The
  smallest direct win anywhere in the study is 6.8×, the largest 230×.
- The single-threaded `triangle raster` beats the 24-thread drop-cutter by 56×
  in **wall** time on terrain at 849² (`CPU/wall ≈ 0.9`, i.e. genuinely one
  core). The shipped classifier is not slow because it is serial.
- Ranking between the direct arms is **fixture-dependent and small compared to
  the gap to the baseline**: tile raster wins on terrain at 849² (0.016 s) and
  on the analytic fixture (0.003 s), vertical ray wins on the small terrain
  grids. Both are far below any human-perceptible threshold, so the choice
  between them should be made on robustness, not on these milliseconds.
- **Memory is bounded and small.** Peak RSS across the whole terrain sweep is
  134 MB, of which the mesh and index are ~100 MB — the classification grids
  are 5.8 MB per `f64` array at 849². The raster arms add ~30 MB of tile
  buffers at 849² and release them. No arm retains anything between calls; §8
  is where retention would have entered.

---

## 5. Equivalence

Grids are identical across arms by construction (`ClassificationGridSpec`,
pinned to the production builder by `grid_spec_matches_production_builder`), so
comparison is cell-for-cell — no resampling, none of Checkpoint B's
cross-resolution nearest-cell machinery.

### 5.1 Candidate 0: exact

**Bit-identical on every fixture and every grid**, analytic and terrain, 143²
through 849². `scratch_arm_is_bit_identical_everywhere` and the terrain row
assert it. Verdict: **EXACT**.

### 5.2 Candidates 1–3: identical to each other, bounded against the oracle

The three direct arms produce **the same labels everywhere** and the same Z to
within 1 pm (§6.6 explains the residual). They are one algorithm, so one row
per fixture below.

| fixture | z max Δ mm | z RMS mm | label Δ | mid regions ≥8 (oracle→arm) | very regions ≥8 | steep regions lost | verdict |
|---|---|---|---|---|---|---|---|
| plateau | 0.000000 | 0.000000 | 0 (0.000%) | 0→0 | 1→1 | 0 | EXACT |
| plate-with-hole | 0.000000 | 0.000000 | 0 (0.000%) | 0→0 | 0→0 | 0 | EXACT |
| stacked-shelf | 0.050000 | 0.018362 | 0 (0.000%) | 0→0 | 1→1 | 0 | BOUNDED |
| grooved-block | 0.025000 | 0.006988 | 386 (0.434%) | 2→2 | 2→2 | 0 | BOUNDED |
| non-manifold-fin | 3.961891 | 0.068962 | 52 (0.166%) | 2→2 | 2→2 | 0 | BOUNDED |
| **mixed-slope** | 0.191900 | 0.013024 | 1362 (**4.347%**) | 8→6 | 2→2 | **2** | **FAILED** |
| terrain@143² | 6.851419 | 0.620366 | 709 (3.467%) | 29→28 | 0→**2** | 0 | BOUNDED |
| terrain@425² | 1.783413 | 0.025460 | 3174 (1.757%) | 96→91 | 22→23 | 0 | BOUNDED |
| **terrain@849²** | 1.783413 | 0.025200 | 17687 (**2.454%**) | 385→**392** | 162→**175** | **5** | **FAILED** |

- **The coverage mask is exact on every arm, every fixture, every grid** (cov Δ
  = 0 throughout). It is the same `contains_point_xy` predicate over the same
  triangles, and it is what keeps a finish pass out of holes and off the margin
  ring, so this is the one hard equality the direct arms do satisfy.
- On the two fixtures with no slope at all (flat plateau top, flat holed plate)
  the direct arms are **bit-identical to the shipped classifier**. The
  divergence is entirely a function of slope, as §1's formula predicts.
- The divergence is **not one-directional**. At 849² the direct arms drop 5
  mid-steep components while gaining 7 others, and gain 13 very-steep
  components (162→175). At terrain 143² they find 2 very-steep components where
  the shipped grid finds **none**. This is redistribution, not collapse — but
  M3's gate is "no loss of narrow steep regions" and 5 are lost, so the honest
  verdict is FAILED-as-specified, not "close enough".

### 5.3 The divergence is the probe, measured

Two candidate causes: sampling error in the candidates, or the oracle's own
slope-dependent CL offset. Shrinking the probe discriminates —
`direct_arm_divergence_is_the_probe_offset`:

| fixture | Ø0.05 (shipped) | Ø0.005 | Ø0.0005 |
|---|---|---|---|
| plateau | 0 labels / 0.000000 mm | 0 / 0.000000 | 0 / 0.000000 |
| grooved-block | **386** / 0.025000 | **0** / 0.002500 | **0** / 0.000250 |
| mixed-slope | **1362** / 0.191900 | **0** / 0.019190 | **0** / 0.001919 |
| plate-with-hole | 0 / 0.000000 | 0 / 0.000000 | 0 / 0.000000 |
| stacked-shelf | 0 / 0.050000 | 0 / 0.005000 | 0 / 0.000500 |
| non-manifold-fin | **52** / 3.961891 | **0** / 0.000837 | **0** / 0.000084 |

Two things at once. **Label disagreement goes to zero at the first 10× shrink,
on every fixture** — a 100×-smaller probe reproduces the direct arms' labels
exactly. And **the maximum Z difference scales precisely 10× per 10× of probe
radius** (0.191900 → 0.019190 → 0.001919; 0.025000 → 0.002500 → 0.000250),
which is the `R·(1 − n.z)/n.z` law and nothing else.

So the direct arms are not approximating the surface — they *are* the surface,
and the difference is the shipped classifier's own probe artefact. That
reverses the usual reading of these numbers: the 2.45% of terrain cells that
change band at 849² are cells the shipped classifier is currently labelling
from a surface 25 µm above the model, not cells a candidate gets wrong.

It also contradicts the standing comment in `finish_setup.rs`, which describes
the probe as "small enough that the probe's own offset is negligible at finish
cell sizes". At `cusp/4` = 0.125 mm it is not negligible: it moves 1.8–4.3% of
cells across a band boundary and, on the fixture with a one-cell terrace step,
manufactures two mid-steep regions that the true surface does not have.

---

## 6. Edge cases

Every case the plan lists, with what was measured. Fixtures are in
`tests/common/meshes.rs`; each isolates one way the "one clean upward-facing
surface per XY point" assumption fails.

**6.1 Stacked / overlapping triangles.** `stacked_shelf`: a ground plate with a
shelf 4 mm above it wound **face down**, so the topmost surface is the one an
orientation test would reject. Every arm reads 4.0 at the shelf centre
(`stacked_triangles_take_the_topmost_surface`). The `max` reduce needs no
orientation test. Interesting side finding: the oracle reads **3.95**, i.e.
`2R` *below* the surface, because `facet_drop` on a down-facing facet puts the
CL under it. Uniform, so no label moves — but the direct arms are right and the
shipped one is 50 µm low on any down-wound face.

**6.2 Vertical and near-vertical walls.** `plateau` (exactly vertical walls,
only the top face reachable) is EXACT on every arm. An exactly vertical facet
projects to a line: `z_at_xy` returns `None` and it contributes no height,
which is the same answer `facet_drop`'s `n.z.abs() < 1e-12` guard gives. It
still counts as **covered** in both — coverage is `contains_point_xy` alone —
and the direct arms reproduce that deliberately.

**6.3 Holes and uncovered cells.** `plate_with_hole` (open mesh, square hole)
is EXACT on every arm including the coverage mask. Coverage is exact on all six
fixtures and all three terrain grids.

**6.4 Non-manifold / open meshes.** `non_manifold_fin`: three faces on one
edge, a free boundary, and a detached flyer belonging to no shell. All arms
agree on labels to within 52 cells (0.166%), no region lost. The 3.96 mm max Z
difference is the oracle's ball riding the flyer's *edge* from outside its
footprint — rim contact, which §5.3 confirms vanishes with the probe (3.96 mm →
0.84 µm at Ø0.005).

**6.5 Max-gradient stencil.** Every label in this study is computed through
`slope_map_max_gradient()`, the stencil `finish_setup` actually selects, at the
shipped 45°/75° thresholds. The mixed-slope fixture carries a deliberate
one-cell terrace step for exactly this reason, and it is the fixture where the
arms diverge most (4.347%) — the interaction between a single-cell stencil and
a slope-dependent Z offset is the sharpest case, not an average one.

**6.6 Deterministic ties.** The direct arms are **not** bit-identical to each
other, and the reason is worth recording. Each keeps the first strictly-greater
candidate; the scatter arm visits faces in mesh order, the gather arm in
index-cell order. Where a cell centre lands on an edge shared by two triangles
— all 386 profile-breakpoint cells of the grooved block — both contain the
point and both evaluate the same plane height to `+0.0` vs `−0.0`, or to one
ulp. Whichever is seen first wins. Each arm is internally deterministic; they
break an exact tie differently, and no consumer can observe it. The harness
therefore asserts equality to 1 pm **and** identical labels, which is tight
enough to have caught a genuine lattice bug: the raster's original tight
`ceil`/`floor` bbox bounds skipped cells whose centres were a fraction of an
ulp inside a triangle. It now rounds outward by one cell and lets
`contains_point_xy` be the only filter.

**6.7 Cancellation and partial-result discard.** Every arm returns `Cancelled`
and discards the partial grid; `cancellation_is_bounded_by_one_chunk` raises
the cancel mid-grid (not before the first poll) and all five arms surface it.
Windows: 8192 cells for the cell-shaped arms (matching the shipped batch),
1024 **faces** for the scatter arm — smaller on purpose, since one face can
burn into an unbounded number of cells — and one tile band for the tiled arm.

**6.8 Determinism across thread counts.** Every arm run at 1 thread and at 8 in
pinned `rayon` pools, Z / coverage / labels diffed exactly:
`arms_are_deterministic_across_thread_counts` passes for all five. The tiled
arm's disjoint output decomposition is what makes this true by construction
rather than by luck.

---

## 7. Rejected and deferred

**Candidate 0 — rejected as M3's answer, recommended on its own merits.**
0.99–1.14×, nowhere near the 2× bar (§2). But `query_into` / `QueryScratch`
sit under 11 `SpatialIndex::query` call sites and 19 modules' worth of
drop-cutter consumers (simulation, collision, pushcutter, scallop, waterline,
pencil, adaptive3d), and the change is provably answer-preserving. It should be
taken as a small independent tidy-up where a profile shows it helps — not as
this item's deliverable, and not on the strength of these numbers, which say it
does not help *here*.

**Candidate 4 — cached classification fields: not built, and not yet
buildable.** The plan puts it after a proven regular-grid equivalent for good
reason, and there is a second precondition it does not state: `TriangleMesh`
has **no revision or identity field**, so there is nothing to key a cache on
today. A cache keyed on `(bbox, cell size, stencil)` alone would silently serve
a stale field after any in-place mesh edit. Preconditions before this is worth
attempting: (a) a mesh revision/identity that is provably bumped on every
mutation path, (b) a documented retention bound — §4 shows one 849² field is
~12 MB of `f64` + `bool`, so an unbounded cache across setups and tools is real
memory, (c) invalidation sentries in the shape of `invalidate_result_chain`'s.
And note the ordering consequence of §4: once a classification takes 16 ms, the
cache is buying a hit rate against 16 ms, and it may never be worth its
invalidation risk at all.

**Candidate 5 — adaptive refinement near thresholds: not built.** Explicitly
last in the fix sequence, and §4 removes its motivation entirely — refining
adaptively to save part of 16 ms, at the cost of a non-uniform grid whose
topology statistics are no longer comparable cell-for-cell with anything, is a
bad trade. Recommend closing it rather than carrying it.

**Filtering to upward-facing facets before the `max` reduce — rejected.** It
would break `stacked_shelf` (§6.1) for no gain: `max` already yields the
topmost surface without an orientation test.

**Deriving coverage from the Z value — rejected**, as it always has been. The
probe rides rims, so a contact height exists where no surface does; that is why
`covered` is a separate mask and why the direct arms compute it from
`contains_point_xy` alone.

---

## 8. What this study cannot decide

The speed question has an unambiguous answer and the equivalence question does
not, because they are not the same question.

Switching production to a direct arm is **not a performance change**. It moves
1.8–4.3% of cells across a band boundary, loses 5 mid-steep regions and gains
13 very-steep ones on the wanaka-class terrain at the production grid — that is
a change in region ownership, which changes which operation cuts which
territory, which changes toolpaths. §5.3 shows the change is *toward* the true
surface and away from a probe artefact, and the module doc says reading the
true surface is the whole reason the classification grid exists — but "the new
answer is better" is a product judgement of exactly the kind Checkpoint B
existed to take, and this wave has no mandate to take it.

The measurement to put in front of that decision is not in this study's scope
and should be built by the implementation wave: an A/B of the two classifiers
through `UnifiedFinish` on the wanaka fixture, scored on the COLUMNS instrument
(pointwise surface deviation), not on cell counts. Cell counts say the labels
moved; only the machined surface says whether that was an improvement.

---

## 9. Recommendation, and handoff to implementation

### 9.1 Winner

**Candidate 3, tile-parallel rasterisation** (`ClassificationSampler::TileRaster`).

187× faster than the shipped classifier at the 849² production row on the
terrain fixture (2.990 s → 0.016 s wall, 40.70 s → 0.13 s CPU) and 230× on the
analytic fixture — against a 2× target. It is chosen over the other two direct
arms, which produce **byte-identical labels** and are equally fast in human
terms, on robustness rather than milliseconds: it is the only one whose
parallelism is correct by construction (disjoint output tiles, no atomics, no
scheduling dependence — §6.8), its cancellation granularity is a tile band
rather than the whole triangle soup (§6.7), and it does not depend on the
spatial index's per-cell registration being a complete candidate set the way
the vertical-ray arm does. Keep `VerticalRay` as the cross-check arm: it is a
different algorithm that must agree, and it is what caught the raster's lattice
bug (§6.6).

**The recommendation is conditional.** The 2× bar is cleared by two orders of
magnitude and every hard invariant holds — coverage exact, deterministic,
cancellable, bounded memory. But the classifier's *answer* changes (§5.2), and
§8 says why that is a checkpoint decision rather than a merge. Recommended
sequence: (1) put §5.3 and §8 in front of the human as a Checkpoint-B-shaped
decision; (2) if approved, land the switch with the parity gates below; (3) if
declined, M3 closes as "no answer-preserving optimisation exists at a
worthwhile ratio" — because candidate 0 is the only answer-preserving option
and it is worth nothing (§2).

### 9.2 Residual risks for the implementation wave

1. **Region ownership moves.** 2.454% of cells at terrain 849²; 5 mid-steep
   components lost, 7 gained, 13 very-steep gained. Downstream this reaches
   `decompose` → `Region` spans → which operation owns which territory. Nothing
   in this study exercises that chain.
2. **The gain is on near-vertical ground.** The largest label movements are
   where the probe offset is largest (80°+), which is precisely where the
   §14q/Checkpoint-B work has been fighting to *recover* steep territory. The
   direct classifier finds more very-steep territory, not less — likely good,
   entirely unmeasured through generation.
3. **Down-wound faces shift by 2R.** §6.1: the oracle reads 50 µm below a
   down-facing facet, the direct arms read the facet. Uniform on the fixture
   here; on a real overhang it is a 50 µm step at the boundary between
   up- and down-wound territory.
4. **Tie-breaking is arm-specific.** §6.6. Harmless today because labels are
   identical, but a future arm that reduced with `f64::max` instead of `>`
   would fold `±0.0` differently. Pin the reduction, not just the result.
5. **`spec_at_side` is not the production cell rule.** The timing table varies
   grid side over a fixed footprint; production sizes the cell from
   `cusp_radius/4`. The ratios are what transfer, not the absolute cell sizes.
6. **One-machine, one-run, swap-full.** §4. Ratios this large do not need
   replication to be believed, but the absolute walls should not be quoted as
   spec numbers.

### 9.3 Parity gates the implementation must pass

Before any production switch:

1. `scratch_arm_is_bit_identical_everywhere` and the terrain EXACT row still
   pass — the answer-preserving arm remains the control that proves the
   harness can detect zero change.
2. `direct_arms_agree_to_the_last_ulp_and_on_every_label` passes on every
   fixture, terrain included: the new production arm and the cross-check arm
   must not have drifted apart.
3. `coverage_mask_is_exact_on_every_arm_and_fixture` — **exact**, no tolerance.
   A moved coverage mask is a finish pass entering a hole.
4. `arms_are_deterministic_across_thread_counts` at 1 and 8 threads, diffed
   exactly on Z, coverage and labels.
5. `cancellation_is_bounded_by_one_chunk`, with the cancel raised mid-grid,
   plus a bound on the observed latency in tiles.
6. `direct_arm_divergence_is_the_probe_offset` still converges monotonically to
   zero label disagreement — if it ever stops, the attribution in §5.3 is wrong
   and the whole recommendation must be re-derived.
7. **New, not in this wave:** an end-to-end `UnifiedFinish` A/B on wanaka,
   old classifier vs new, scored on `SimulationResult::column_deviations`
   (COLUMNS) — with the region mix table and Region spans printed for both.
   §8. The rule from the v3 campaign applies: *never gate on an aggregate
   without rendering the surface*.
8. Re-run the M3 timing rows on the machine of record and record them beside
   §4, so the claimed ratio is re-earned rather than cited.

### 9.4 What must NOT be carried over as fact

- "The probe offset is negligible at finish cell sizes" (`finish_setup.rs`
  doc). Measured false at `cusp/4`; that comment should be corrected by the
  implementation wave whichever way the decision goes.
- "The bitset allocation is the bottleneck." Pre-registered in §2 and refuted
  by measurement in the same section. It is ~a fifth of the query cost, and
  the query is ~a fifth of the classifier.
- Any timing from this study's first release run. It was taken with two test
  threads sharing 24 cores and has been discarded; §4 explains how.

---

## 10. Implementation record — wave 7b, 2026-08-02

Parent commit `c34edfb`. Commits: `0711568` (production switch + provenance +
sentries), `5d8c115` (the COLUMNS A/B and its verdict), plus the docs commit
carrying this section.

### 10.1 The ruling

Human checkpoint, 2026-08-02, on §8's question: **"Adopt, COLUMNS-gated."**
Production switches to candidate 3 behind the eight parity gates of §9.3; the
decisive gate is an end-to-end wanaka-class `UnifiedFinish` A/B scored on
COLUMNS quality, **not on cell counts**; if COLUMNS regresses, production
falls back to the shipped classifier and M3 closes with this study as its
record.

COLUMNS did not regress. Production is on the tile raster.

### 10.2 What shipped

`finish_setup::build_classification_surface_with_policy_and_cancel` builds a
`ClassificationGridSpec` and dispatches through `sample_classification_grid`
at `ClassificationSampler::PRODUCTION`. Three seams were added:

| seam | what it is for |
|---|---|
| `ClassificationSampler::PRODUCTION` | the one constant. The §9.1 fallback is "set it back to `DropCutterProbe`"; nothing else in the pipeline names a sampler. `Default` tracks it. |
| `build_classification_surface_with_sampler_and_cancel` | names the sampler explicitly — how sentries score against the oracle and how the A/B runs both classifiers through one production path. |
| `UnifiedFinishParams`/`UnifiedFinishConfig::classification_sampler` | the op-level thread. Serialised only when non-production (`skip_serializing_if`), so no project file changes and none grows a key. |

Provenance follows the `CellSource` precedent one level up:
`FinishSurface.sampler: SurfaceSampler` is `CutterOffset` for generation
grids and `Classification(sampler)` for classification grids. `CellSource`
says what **sized** the cells; this says what was **measured into** them.

The grid arithmetic that existed twice — in `finish_setup` and in
`ClassificationGridSpec::for_mesh`, with a sentry policing the duplicate —
now exists once. The sentry was rewritten to assert the *contract* (padding
is one envelope radius, the cell is the policy's, the grid reaches the far
padded edge so the mask→polygon extractor keeps its margin ring).

§9.4's correction landed: the `CLASSIFICATION_PROBE_DIAMETER_MM` doc no
longer claims the probe's offset is negligible at finish cell sizes. It
carries the measured numbers instead.

### 10.3 The eight §9.3 gates

| # | gate | result |
|---|---|---|
| 1 | `scratch_arm_is_bit_identical_everywhere` + the terrain EXACT row | **PASS.** Bit-identical on all six analytic fixtures and at terrain 143²/425²/849² (`z max Δ` 0.000000, label Δ 0). The control still detects zero change. |
| 2 | `direct_arms_agree_to_the_last_ulp_and_on_every_label` | **PASS.** Raster, tiled raster and vertical ray agree to 1 pm and on every label, every fixture. |
| 3 | coverage mask exact, no tolerance | **PASS.** `cov Δ = 0` on every arm, every fixture, all three terrain grids, and through the production builder. |
| 4 | thread-count determinism at 1 and 8 | **PASS**, and now at two levels: the arms (`arms_are_deterministic_across_thread_counts`) and the **production builder itself** (`the_production_builder_is_deterministic_across_thread_counts`, diffing Z, coverage and labels, with a band-population non-vacuity check). |
| 5 | cancellation bounded by one chunk, with an observed latency bound | **PASS**, tightened. The test now counts polls and requires the arm to stop on the first poll that reports `true` (+1 for a wrapper re-check), and asserts the fixture is ≥3 tile bands. **That guard caught this wave's own first attempt**, which cancelled on the last band of a 3-band grid and proved nothing. A production-path twin (`the_production_builder_cancels_and_discards_the_partial_grid`) covers the compute worker's cancel flag; `Result` makes the discard structural — there is no half-built `FinishSurface` to leak. |
| 6 | `direct_arm_divergence_is_the_probe_offset` still converges | **PASS.** Monotone to zero label disagreement at the first 10× shrink on every fixture. §5.3's attribution stands. |
| 7 | **end-to-end `UnifiedFinish` A/B on COLUMNS** | **PASS — see §10.4.** |
| 8 | timings re-earned in the final tree | **PASS — see §10.5.** |

Two further gates were added that §9.3 did not ask for, both from the
residual-risk list:

* **§9.2 risk 4, "pin the REDUCTION not just the result".** The per-cell
  reduce is now a named `keep_higher` with its own test, which fails if it is
  ever rewritten as `f64::max`: a signed-zero tie must keep the first-seen
  candidate (first-seen-wins is what makes each arm internally
  deterministic, §6.6), and a NaN must neither win a cell nor be laundered
  out of one.
* **Tripwires.** §5.2's per-fixture label-movement numbers were
  characterisation while the arms were prototypes. Now that one of them is
  production they are ceilings (`LABEL_MOVE_TRIPWIRES`): the three fixtures
  that measured exactly zero are pinned at zero, the rest carry 25% headroom,
  and a fixture with no entry fails rather than passing unbounded. The
  terrain rows re-measured **exactly** — 709 / 3174 / 17687 — so the wave-7a
  table reproduces.

### 10.4 Gate 7: the COLUMNS A/B

`crates/rs_cam_core/tests/classification_columns_ab_m3.rs`. Branch A pins
`DropCutterProbe`, branch B pins `TileRaster`; same mesh, tool, stock, dials,
simulation grid and process. The fixture is the committed
`tests/fixtures/terrain.stl` and the config is built in-test — it does **not**
read `planning/airrun_2026-06-01/wanaka.toml`, because a gate that depends on
a live user-modified project has a verdict that changes when somebody drags a
slider (`wanaka_suggest_baseline` is permanently red for exactly that reason).

Tool and cell are the production ones: Ø1 tip on a Ø6 shank at 7°, so the
classification cell is `cusp/4` = 0.125 mm — the scale the whole decision
lives at. Measurement grid 0.1 mm, comfortably under the 0.5 mm tip radius.
Only the FOOTPRINT is a knob (`M3_AB_WINDOW_MM`).

|  | 30 mm window | **full 100 mm terrain** | tolerance |
|---|---|---|---|
| common columns | 86 637 | **1 001 987** | — |
| p50 \|dev\| | 62.39 → 61.74 µm (−0.65) | **28.59 → 28.69 µm (+0.11)** | +5 µm |
| p90 \|dev\| | 302.01 → 300.53 µm (−1.48) | **219.03 → 223.47 µm (+4.44)** | +10 µm |
| on-size ±10 µm | 13.030 → 12.892% (−0.139 pp) | **26.665 → 26.570% (−0.095 pp)** | — |
| on-size ±25 µm | 27.155 → 27.154% (−0.001 pp) | **47.596 → 47.514% (−0.082 pp)** | −0.5 pp |
| max \|dev\| | 2150.7 → 2491.9 µm | 3870.2 → 3990.4 µm | reported, not gated |
| rapid collisions | 0 → 0 | **0 → 0** | no increase |

Region mix, full terrain (the table §9.3 gate 7 asks for beside the verdict):

| band / strategy | A regions | B regions | A area mm² | B area mm² |
|---|---|---|---|---|
| Shallow / raster | 5 | 4 | 6267.8 | 6358.6 |
| MidSteep / scallop | 1 | 1 | 8127.6 | 8100.2 |
| VerySteep / waterline | 9 | 6 | 3343.1 | **3427.6** |

**Verdict: ADOPT.** Every gated statistic clears by an order of magnitude, in
both directions — this is a wash on quality, not a win. Thresholds were
pre-registered above the assertions and justified against a physical scale
(the dials ask for a 22.5 µm cusp, `0.3²/(8·0.5)`, so p50 may rise a quarter
of that and p90 half of it). `max` is reported and not gated: one column in a
million is not a distribution.

Three things stated rather than buried:

1. **The classifier's speed reaches the user.** Whole-op generation wall on
   the full terrain: 249.0 s → 192.9 s, **−22.5%**. §4's 187× is a
   microbenchmark; this is the same change measured through an operation.
2. **B costs +5.6% cutting time on the full terrain** (7188.8 → 7588.5 s). It
   finds 84.5 mm² more very-steep territory — §9.2 risk 2, predicted — and
   waterline is the most expensive strategy per area. Quality did not pay for
   the extra territory; the clock did. On the 30 mm window the same change
   went the other way (−2.5%), so this is fixture-dependent, not a law.
3. **`max` got worse on both windows** (+341 µm, +120 µm). Leftover, not
   gouge, and one column — but recorded.

Per the v3 campaign's closing rule — *never gate on an aggregate without
rendering the surface* — the harness writes three PNGs to
`target/m3_columns_ab/` before any verdict is read. The B−A map is the phase
texture P2.g characterised: mottled, local mean zero, **no coherent block of
standing material**. 66.9% of columns move between branches, so the two
surfaces really are different surfaces being compared.

### 10.5 Gate 8: timings re-earned in the final tree

Same machine, `--release`, `--test-threads=1`, `pgrep` clear before the run.
Swap was full and ~27 GiB RAM was available, as in wave 7a.

| 849² row | wave 7a wall | **wave 7b wall** | wave 7a CPU | **wave 7b CPU** |
|---|---|---|---|---|
| terrain, shipped | 2.990 s | **3.161 s** | 40.70 s | **40.38 s** |
| terrain, tile raster | 0.016 s | **0.016 s** | 0.13 s | **0.13 s** |
| terrain speed-up | 187× | **198× wall, 311× CPU** | | |
| analytic, shipped | 0.689 s | **0.721 s** | 13.03 s | **12.58 s** |
| analytic, tile raster | 0.003 s | **0.004 s** | 0.03 s | **0.03 s** |
| analytic speed-up | 230× | **180×** | | |

Peak RSS across the terrain sweep: 135 MB (wave 7a: 134). Nothing retained
between calls. The ratios reproduce; the absolute walls wobble by ~6%, which
is what §9.2 risk 6 said to expect from one machine and one run.

### 10.6 Downstream: what moved

Nineteen sentries that consume region ownership were run against the switch —
`unified_finish_semantic_regions`, `checkpoint_a_valley_matrix`,
`coverage_routing_pr5`, `crease_own_region_pr6b`, `generic_rest_routing_pr7`,
`unified_finish_tapered_end_to_end_m21`, `steep_shallow_min_segment_pr8d`,
`finish_resolution_policy_pr3`, `tool_scale_semantics_pr2`,
`tapered_cusp_radius_sentry`, `waterline_shared_finish_setup_c3`,
`finish_planner_wanaka_decompose`, `derived_stepover_pr6a`,
`unified_finish_dropped_band_finding_d1`,
`unified_finish_partial_clip_finding_c8`, `narrate_regions_closed_c8`,
`checkpoint_b_resolution_ab`, `ramp_reach_clamp_pr8b`, `reach_policy_pr4`.

**All green. Not one pinned expectation moved.** That is a real result rather
than a lucky one, and §5.2 says why: the synthetic fixtures these sentries use
are the fixtures where the probe artefact is smallest — flat ground and clean
analytic walls, where the direct arms measured EXACT or within a few hundred
cells. The redistribution §9.2 warned about is a property of *terrain*, and
terrain is where the A/B measured it.

### 10.7 Reading the `verdict` column after this wave

`print_equivalence_rows` scores against whichever grid the caller passed as
the ORACLE. Where that is the shipped Ø0.05 probe, a `FAILED` row means the
arm dropped a region **the probe reports** — which since wave 7b is
characterisation, not a gate. `terrain@849` still prints `FAILED` for all
three direct arms while the suite is green, and that is the expected reading:
`production_loses_no_region_against_the_true_surface` re-scores the same
question against the surface, and `LABEL_MOVE_TRIPWIRES` bounds the movement.
The harness prints this footnote under every table so the next reader does
not have to rediscover it.

### 10.8 The gate that had to be restated, and why

The plan's acceptance gate reads *"no loss of narrow steep regions relative
to the current fine classifier"*. Taken literally the direct arms fail it —
5 mid-steep components lost at terrain 849² (§5.2). §5.3 measured why: those
components are the probe's own slope-dependent CL offset, not surface.
Holding a true-surface classifier to a probe-artefact reference rejects it
for being right.

So the gate is restated with the **surface** as the reference — reached by
shrinking the oracle's own probe 100×, an independent construction rather
than one of the direct arms scoring itself. Against that reference:

* the production sampler moves **0 labels and loses 0 regions** on every
  analytic fixture — it *is* the surface, not an approximation of it;
* the shipped probe moves labels on three fixtures and **fabricates**
  regions rather than losing them (mixed-slope: 6 real mid-steep components
  read as 8).

The direction is the opposite of the plan's phrasing, which is exactly why
the restatement had to be a measurement and not a re-pointing. Both facts are
asserted, so the gate cannot go vacuous in either direction.

### 10.9 Still open

* **Candidate 0** (`query_into` / `QueryScratch`) remains recommended on its
  own merits and unlanded — §7. It is worth nothing *here* and the study says
  so; it should be taken where a profile shows it helps.
* **Candidates 4 and 5** stay closed, for §8's reasons, now reinforced: a
  classification that takes 16 ms is not worth a cache's invalidation risk.
* **`spec_at_side` is still not the production cell rule** (§9.2 risk 5). The
  timing table varies grid side over a fixed footprint; the A/B in §10.4 is
  the row measured at the real `cusp/4`.
* **Down-wound faces still shift by 2R** (§9.2 risk 3) between the two
  families. Uniform on `stacked_shelf`; on a real overhang it is a 50 µm step
  at the up/down-wound boundary. The switch moves production onto the correct
  side of it, and nothing here exercises a real overhang.
