# DELTA — gen-w3 (distance-field lane), finding G9 + the face half of G2

Lane: **DISTANCE-FIELD**. Owned files: `crates/rs_cam_core/src/vcarve.rs`,
`inlay.rs`, `rest.rs`, `face.rs`, the new
`crates/rs_cam_core/src/edge_distance.rs`, its one-line registration in
`lib.rs`, and one added arm in `benches/hot_paths.rs`.
Baseline: `BASELINES.md` (Phase 0, captured 2026-08-19).

**Result in one line: `gen_vcarve_field/frame60_9holes_tol005` 193.67 ms →
15.869 ms (−91.8%, 12.2×) — of which indexing bought 2.98× and parallelism a
further 4.10×; `gen_face_levels` 264.89 µs → 85.179 µs (3.11×). Output
bit-identical everywhere, pinned by seven new sentries.**

## What G9 said, and what was actually there

Two of the three call sites the finding names were as described. The third was
not, and it matters for the prescription.

- `vcarve.rs:138` — **as described.** `point_to_polygon_distance` walked every
  exterior and hole edge per sample.
- `inlay.rs:215` — **as described**, under the name `point_to_polygon_boundary`;
  byte-for-byte the same algorithm with the rings passed separately. The
  triplication claim is right: two verbatim copies of one function.
- `rest.rs:126` — **not a distance field.** That line samples
  `point_in_any_polygon` → `Polygon2::contains_point`, an even-odd ray cast
  against the *previous tool's reachable region*. It is the same family of
  defect (a per-sample linear walk over every edge) but it is a **containment**
  query, and it cannot share `EdgeDistanceField`: a distance index answers a
  different question, and `contains_point` already has a bbox early-out that a
  distance query does not. Verified against the pre-G2 file
  (`git show 473c3d1f^:crates/rs_cam_core/src/rest.rs`), so this is not an
  artifact of the G2 restructure moving lines around.

So "one indexed EdgeDistanceField shared by all three" is a **two**-site
consolidation, not three. Rest gets the other half of the prescription — the
per-scan-line parallelism, which is item 1 on the review's missed-parallelism
list — and nothing else.

## What landed

### 1. `edge_distance.rs` — the shared indexed field (G9 half one)

`EdgeDistanceField` is a uniform-grid edge index (CSR: count, prefix-sum, fill)
with an expanding-ring query. Cells are sized for roughly one edge each;
segments are stamped by walking their columns and clipping to each column's x
slab, so a long diagonal costs O(cells crossed) rather than O(bbox area).

Both `vcarve.rs` and `inlay.rs` now build **one field per polygon** and query it
per sample, replacing the two copies of the linear helper.

**The exactness argument, because v-carve turns this number straight into a Z.**
An approximate distance here is a wrong cut, not a slow one. The query returns
the bit-identical `f64` the linear scan returned:

- The edge set is built in the same order and with the same closing rule
  (`windows(2)` then the `(last, first)` wrap), so it is the same multiset of
  segments — including the degenerate one-vertex case, where dropping the wrap
  edge would turn a `0.0` into an `INFINITY`.
- The reduction is `f64::min`, which ignores NaN and is order-independent for
  everything else. Evaluation order therefore cannot move the answer, and ties
  at equal distance are decided by value rather than by index.
- The ring search only *prunes*. The closest point on a segment lies **on** that
  segment, and every cell a segment passes through is stamped (plus a one-cell
  halo), so the cell holding that closest point is always visited before the
  bound can terminate the search.

Pinned by `edge_distance_matches_linear_scan_bitwise` (40 randomised
frame-plus-holes polygons; interior scatter, far-outside points, exact vertices,
edge midpoints and integer grid lines — `to_bits`, not `==`),
`edge_distance_handles_degenerate_boundaries` (empty ring, one vertex, two
vertices, collinear ring with a zero-height bbox, four coincident vertices,
empty and single-vertex holes, explicitly-closed ring),
`edge_distance_matches_on_extreme_aspect_ratio` (4000×2 mm, the axis-clamp
path), and `from_rings_matches_from_polygon`.

The generators are pinned end to end as well, since an exact field can still be
wired in wrong: `vcarve_indexed_field_is_bit_identical_to_linear_scan` and
`inlay_male_indexed_field_is_bit_identical_to_linear_scan` compare whole
toolpaths, `to_bits` per axis, over three parameter sets each.

`edge_distance_actually_prunes` asserts the index does its job in a way a wall
clock cannot: `query` returns the point-segment evaluation count alongside the
distance, and the test asserts the mean stays well under the linear edge count.

### 2. Per-scan-line parallelism (G9 half two)

`vcarve`, `inlay`'s male plug and `rest_segments` now map their scan lines
across rayon, gated on the `parallel` feature exactly as `dropcutter.rs` does,
with a sequential `iter()` arm so the non-`parallel` build still compiles.
Order is preserved by `collect()`, so emission — and therefore the G-code — is
byte-identical either way.

**Cancellation.** `vcarve_toolpath_with_cancel` documented "polls once per scan
line", and that contract had to survive. Rather than widen the public signature
to `&(dyn CancelCheck + Sync)` — which would push a Sync bound onto every
caller for a closure that never needs to run inside the parallel region — the
lines are processed in batches of `SCAN_CHUNK = 32` and the poll sits at the
batch boundary, on the calling thread. Cancellation latency is therefore bounded
by *one batch spread across the pool*, i.e. by wall clock, not by polygon size:
strictly better than the pre-G9 per-line cadence on any machine with more than
one core. A cancel flag set before the call still short-circuits before any
sampling, which is what `cancellable_families_honour_a_preset_cancel_flag`
pins. `rest_segments` has no cancel parameter, so it takes the plain
`par_iter()`.

`parallel_rest_segments_match_the_serial_walk` compares against a verbatim copy
of the pre-parallel loop over four fixtures — including the "large tool cannot
fit at all" fallback branch and an angled scan — because the thing a
reordering would corrupt is *where the segments break*, which a point count
would not notice.

### 3. `face.rs` — the G2 defect the G2 lane found and could not own

`face_toolpath_with_cancel` built the facing rectangle once but then called
`zigzag_toolpath` / `oneway_toolpath` inside the per-Z-level closure, and both
of those start with `zigzag_lines` — the inset and the row slicing — so a
depth-stepped face redid all of it once per level. Same shape as `473c3d1f`:
`face_scan_lines` is the Z-independent half (inset, slicing, and the `OneWay`
endpoint normalisation, which only permutes endpoints within a row and is
Z-independent by the same argument); `lines_to_toolpath` stamps `cut_depth` in
the closure.

`face_hoisted_scan_lines_are_bit_identical` compares against a **verbatim copy
of the pre-hoist `oneway_toolpath` and the real `zigzag_toolpath`** — not
against the new helper, so it can catch a divergence introduced *inside* the
refactor and not merely a misplaced call. Seven cases: single/multi pass,
Zigzag/OneWay, non-zero stock top, an empty inset (tool wider than stock), and
stepover wider than the stock.

## Numbers

All runs serialised on the shared `flock /tmp/rs_cam_cargo.lock` build lane;
criterion baselines `w3_par`, `w3_1thread`, `w3_face_prehoist`.

### G9 — `gen_vcarve_field/frame60_9holes_tol005`

| Variant | Mean | Bounds | vs Phase 0 |
|---|---:|---|---:|
| Phase 0 (linear scan, serial) | 193.67 ms | [191.02, 196.56] | 1.00× |
| Indexed, `RAYON_NUM_THREADS=1` | **65.084 ms** | [63.978, 65.662] | **2.98×** |
| Indexed + parallel scan lines | **15.869 ms** | [15.688, 15.996] | **12.20×** |

**The two halves, separated.** The single-thread arm isolates the index:
indexing alone is **2.98×** (−66.4%). Parallelism on top is a further
**4.10×**. Neither half would have been worth much alone at this size — and
the split matters for reading the result, because the review's thesis is that
*pruning* beats allocator work. It does: the index allocates once per polygon
and the query allocates nothing, and the 2.98× is entirely a reduction in
point-segment evaluations. `edge_distance_actually_prunes` measures that
directly rather than by wall clock — mean **285.7** evaluations per query
against **1924** boundary edges on its fixture, i.e. **6.7× fewer**.

Note the 2.98× wall-clock and the 6.7× evaluation-count are different
fixtures and are not expected to match: the bench polygon has ~870 edges (nine
96-vertex holes plus the frame) against the test fixture's 1924, and the wall
clock also carries scan-line generation, emission and vector growth that no
amount of pruning touches.

### G2 (face side) — `gen_face_levels`, new bench arm

600×400 mm stock, Ø12 tool, 1 mm stepover, 10 mm depth at 0.5 mm/pass = 20 Z
levels. Pre-hoist measured by stashing **only** `face.rs`
(`git stash push -- crates/rs_cam_core/src/face.rs`) — the bench arm uses only
the public `face_toolpath`, so it compiles unchanged against both shapes.

| Arm | Pre-hoist | Post-hoist | Speedup |
|---|---:|---:|---:|
| `zigzag_20levels` | 264.89 µs | **85.179 µs** | **3.11×** |
| `oneway_20levels` | 258.90 µs | **85.402 µs** | **3.03×** |

**3.1×, not the ~20× the other G2 families showed, and that is the correct
answer rather than a disappointing one.** Pocket/profile/zigzag hit 20–22×
because the thing repeated per level was a 1400-vertex offset cascade that
dwarfed emission. Face repeats a *rectangle* inset plus a scan-line slice, and
then emits ~800 rows × 20 levels. The hoist removes 19 of 20 rebuilds of the
level-invariant 68%; the remaining 32% is emission, which is genuinely per
level and cannot be hoisted. The lane brief predicted "a smaller win than the
others" and that is what was measured.

The `oneway` arm confirms the normalisation is hoistable too — it lands within
noise of the `zigzag` arm post-hoist, where pre-hoist it carried the same
redundant work.

### Not separately benched

`inlay`'s male plug shares `EdgeDistanceField` and the same scan-line
parallelism, and the female pocket routes through `vcarve_toolpath_with_cancel`
— so inlay collects the G9 win on both halves, and the review's "inlay runs the
whole thing twice" is now "inlay runs the fast thing twice". No arm was added:
it would measure the same two code paths already measured.

`rest_segments` got the parallelism but not the index (see the correction
below) and has no bench arm. Its per-sample cost is a containment test against
an offset polygon at a 0.25–0.5 mm sample step, an order of magnitude fewer
samples than v-carve; adding an arm was not worth the surface.

## What did not land, and why

- **No allocator work.** `CLASSIFICATION_PERF_STUDY.md` measured the
  "per-query allocation" hypothesis at 0.99× and the review repeats the
  warning. The index allocates once per polygon and the query allocates
  nothing; that is a consequence of the design, not a claimed win.
- **No max-depth early-out.** V-carve clamps `dist / tan_half` to `max_depth`,
  so in wide-open regions the exact distance is discarded. Terminating the ring
  search once the bound exceeds `max_depth * tan_half` would prune hard there —
  but the clamp comparison is a floating-point one, and "provably ≥ the cutoff"
  is not the same predicate as "`dist / tan_half >= max_depth` after rounding".
  Wave 1's G3 shipped a reject that looked provably sound and was not. Not
  attempted without a bit-exact argument.
- **No containment index for rest.** See the correction at the top: it is a
  different query. An indexed even-odd ray cast (y-bucket the edges, keep only
  those spanning the query's y) would be exact by the same parity argument, but
  rest's sample step is clamped to 0.25–0.5 mm — 5–10× coarser than v-carve's
  0.05 — so it is a much smaller target, and it is not what G9 asked for.

## Review corrections (left UNSTAGED in `PERF_REVIEW.md`)

Other lanes have uncommitted hunks in that file, so the edit is deliberately
not staged with this commit.

1. **G9's rest citation.** `rest.rs:126` is `point_in_any_polygon` /
   `contains_point`, not `point_to_polygon_distance`. The helper is
   **duplicated**, not triplicated, and the shared field covers two call sites,
   not three.
2. **G9's magnitude, for the record.** The finding is rated MED-HIGH and the
   fix returned 12.2× on its own bench — the largest single-arm generation
   number in the campaign so far. The half the review called "mechanical"
   (`scan_lines.par_iter()`) turned out to be the *larger* half at 4.10× vs
   the index's 2.98×, which inverts the emphasis of the finding's own wording.

## Verification

- `cargo clippy -p rs_cam_core --lib --benches --test <the eight below> -- -D warnings`:
  clean. The full `--tests` run fails on `tests/pushcutter_band_query_g1.rs`
  (`print_stdout`), which is the PUSH-CUTTER lane's in-flight file — not
  touched, per the ownership protocol. (It was fixed by that lane in `e4379dd5`
  mid-wave.)
- `cargo test -p rs_cam_core --lib`: **2313 passed, 0 failed.**
- Phase 0 goldens: `perf_golden_sim_metrics` 5/5,
  `perf_golden_depth_level_geometry` 2/2.
- Sentries: `vcarve_lift_bridge_b1` 1/1, `face_stock_top_frame_f028` 4/4,
  `rest_grid_resolution_c9` 2/2, `generic_rest_routing_pr7` 6/6,
  `rest_routing_probe_e9` 3/3, `zero_removal_rest_pass_a4` 2/2.
- New sentries (7): `edge_distance_matches_linear_scan_bitwise`,
  `edge_distance_handles_degenerate_boundaries`,
  `edge_distance_matches_on_extreme_aspect_ratio`,
  `from_rings_matches_from_polygon`, `edge_distance_actually_prunes`,
  `vcarve_indexed_field_is_bit_identical_to_linear_scan`,
  `inlay_male_indexed_field_is_bit_identical_to_linear_scan`,
  `parallel_rest_segments_match_the_serial_walk`,
  `face_hoisted_scan_lines_are_bit_identical`.
