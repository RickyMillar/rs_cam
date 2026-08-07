# Checkpoint D evidence — M5, "fix `offset_polygon` vertex inflation at the source"

**Status: research only. Nothing in this wave changes what any production
path does.** Two new library functions exist (`polygon::cleanup_collinear`,
`polygon::simplify_bounded`) and one new research seam
(`ScallopStepoverPolicy::cleanup`), and no shipped call reaches any of them —
`shipped_ring_cleanup_reproduces_the_shipped_scallop_path` asserts the seam is
byte-identical at its shipped setting.

Instruments:

| file | what it is |
|---|---|
| `crates/rs_cam_core/tests/common/offset_lab.rs` | fixtures, cascade runners (polygon / polyline / `geo`), per-ring attribution, the erosion oracle, SVG + CSV artifacts |
| `crates/rs_cam_core/tests/offset_growth_m5.rs` | step 1–2: growth curves and attribution |
| `crates/rs_cam_core/tests/offset_candidates_m5.rs` | step 3–4: candidate arms, 2D oracle, pocket fidelity, M4 envelope oracle |
| `test_data/m5_terrain_mid_steep_polygon.json` | a real mid-steep band polygon captured headlessly from `tests/fixtures/terrain.stl` |

```text
cargo test --release -p rs_cam_core --test offset_growth_m5     -- --ignored --nocapture --test-threads=1
cargo test --release -p rs_cam_core --test offset_candidates_m5 -- --ignored --nocapture --test-threads=1
```

Artifacts land in `target/m5_offset/` (`growth_curves.csv`, `attribution.csv`,
`candidates.csv`, `rosette_raw_rings.svg`, `rosette_decimated_rings.svg`).

---

## 1. The oracles, and why they need no tolerance dial

M4's lesson is binding here: *isolate the variable under test, and read the
headline comparison before the verdict.* Both this study's oracles are exact
rather than tuned.

**2D — erosion distance.** Erosion composes: eroding a set by `d` twice is
eroding it once by `2d`. The boundary of the erosion of `P` by `d` is the
level set `{ p : dist(p, ∂P) = d }`, so **every** point of it is at distance
`d` from `∂P` — on any shape, convex or not, holed or not. Therefore every
vertex of cascade ring `k` must sit at exactly `k · step` from the ORIGINAL
boundary, and `erosion_error` measures the violation directly. No reference
implementation, no tolerance parameter, valid on the captured fixtures as
well as the synthetic ones.

Validated before use, both directions:

* `the_erosion_oracle_reads_zero_on_a_known_erosion` — 20 rings of a square
  read `< 1e-9 mm`, and a ring scored at the WRONG depth reads exactly the
  offset difference (not vacuously zero).
* `the_area_oracle_agrees_with_the_closed_form` — the rasterised erosion area
  of a 100 mm square inset 5 mm lands within 1% of 8100 mm².

**3D — M4's `EnvelopeOracle`**, unchanged, driven through scallop's own
cascade via the new `RingCleanup` seam, so achieved cusp and gouge are read
off the instrument Checkpoint C already validated.

---

## 2. Consumer census — who actually iterates

`rg 'offset_polygon\('` finds 26 production call sites. The census that
matters is not how many there are but **how many feed the primitive's own
output back into it**, because the defect is compounding:

| consumer | site | iterated? |
|---|---|---|
| `pocket::pocket_contours_with_cancel` | `pocket.rs:90`, `:116` | **YES** — an inline `loop` that re-offsets its own output until collapse |
| `scallop::generate_scallop_rings_with_cancel` | `scallop.rs:1146` | **YES** — and the only consumer that compensates (drop-only decimation, P2.f) |
| `polygon::pocket_offsets` | `polygon.rs:405` | **YES**, but has **no production caller** — benches and in-file tests only |
| adaptive machinability probe | `adaptive/path.rs:132,858,944,1192,1238` | no — single shot per query |
| adaptive3d clearing | `adaptive3d/clearing.rs:1643,1737,1869` | no — one inset per Z level |
| rest / boundary / profile / trace / zigzag / inlay / project-curve | one site each | no |
| `region_set::processed`, `session/compute.rs:1735`, viz worker `execute/mod.rs:169,679` | | no |

So the blast radius splits cleanly:

* **Two consumers inherit the compounding** (pocket, scallop) and one of
  them already pays for it.
* **Every other consumer inherits only the single-shot error** — one
  arc-join chord — which is a *different* defect (§5) and a much smaller one.

Only `polygon.rs` imports `cavalier_contours` in shipping code; every call
site above goes through `offset_polygon` and never sees a library type. That
is what makes a backend swap cheap to consider at all.

---

## 3. Growth: the reproduction

`growth_curves_with_and_without_todays_compensation`, release, 200-ring /
120 000-vertex / 60-second budget, inward offsets only.

| fixture | what it isolates | verts₀ | **raw** %/ring (geo. mean) | worst single ring | verts at stop | stop |
|---|---|---|---|---|---|---|
| square | convex control | 4 | **0.00** | 0.00 | 4 @ ring 200 | ring limit |
| rosette-24 | STRESS: 24 reflex valleys that survive erosion | 720 | **34.74** | 84.06 | 88 320 @ 16 | TIME BUDGET |
| comb-16 | comb, 6 mm fingers | 68 | **36.62** | 99.89 | 70 148 @ 22 | TIME BUDGET |
| dendrite | branching concavity, two scales | 244 | **90.98** | 99.80 | 123 004 @ 10 | VERTEX CAP |
| holed-9 | holes + disjoint outputs (`Shape` path) | 580 | **6.44** | 99.99 | 80 320 @ 69 | TIME BUDGET |
| short-edges | 1600 sub-epsilon segments | 1600 | **−0.26** | 0.00 | 480 @ 200 | ring limit |
| wanaka-slice | CAPTURED REAL, 13 holes | 331 | **1.89** | 90.50 | 17 903 @ 200 | ring limit |
| terrain-midsteep | CAPTURED REAL mid-steep band | 744 | **40.26** | 86.11 | 132 885 @ 16 | VERTEX CAP |

Under today's drop-only decimation, **every one of those runs the full 200
rings in ~0.0 s and ends bounded** (48, 30, 27, 84, 12, 125 vertices at ring
200). The compensation works. It is just not shared.

The plan's "15–25% per ring" was an understatement on every concave fixture
except the two captured ones. The raw per-ring vertex series is not
approximately geometric — it is **exactly** doubling:

```
comb-16     100   164   292   548  1060  2084  4132  8228   ...
increment    +64  +128  +256  +512 +1024 +2048 +4096
dendrite    364   604  1084  2044  3964  7804 15484 30844   ...
increment   +240  +480  +960 +1920 +3840 +7680 +15360
holed-9    1156  2308  4612  9220 18436 36868 73732 73732   ← plateau
```

Two consequences worth stating separately:

* **It saturates, it does not diverge.** `holed-9` plateaus at 73 732 and
  `wanaka-slice` at 17 903 — once the arc-join chords get shorter than
  cavalier's own `pos_equal_eps` (1e-5), its dedupe absorbs them. The growth
  law is logistic, with a ceiling around 10⁵ vertices on these fixtures. That
  is not a comfort: the ceiling is 100–500× the input.
* **The cost is wall-clock, not memory.** A single offset call on the
  saturated rosette takes **10.19 s**. That is the GUI-reachable hang class
  the P2.f decimation was introduced to stop (30 min → 2.9 s), reproduced
  here on a synthetic fixture with no scallop in the picture.

`target/m5_offset/rosette_raw_rings.svg` and `rosette_decimated_rings.svg`
were rendered and read, not merely written. The decimated file carries all 40
rings; the raw one carries only the **16** its budget reached, which is itself
the picture. Over the 16 rings they share, the two are congruent — no lobe
moves, no valley rounds off — which is the geometric claim decimation makes
for itself (it drops points, it never moves one). §6's oracle is the
quantitative version of that eyeball.

---

## 4. Attribution: it is one cause, and it is ours

`where_the_new_vertices_come_from` classifies every vertex the primitive
returns, measured on the flattened output — exactly the vertices
`Polygon2::from_pline` keeps — except the arc counts, which can only be seen
*before* flattening. That is why the instrument talks to cavalier directly.

**Added vertices == arc-join segments, 1:1.** Summed over rings 1–5:

| fixture | added | arc segments | ratio |
|---|---|---|---|
| square | 0 | 0 | — |
| rosette-24 | 8 142 | 8 142 | **1.000** |
| comb-16 | 992 | 992 | **1.000** |
| dendrite | 3 720 | 3 720 | **1.000** |
| holed-9 | 17 856 | 17 856 | **1.000** |
| short-edges | −8 | 0 | — |
| wanaka-slice | 2 721 | 2 773 | 0.981 |
| terrain-midsteep | 2 379 | 2 474 | 0.962 |

(The captured fixtures fall just under 1.0 because `offset_polygon` repairs
their self-intersections into pieces first and re-pairs holes to containers
afterwards; the synthetic ones are exact.)

Against the plan's four candidate causes:

| candidate cause | verdict | number |
|---|---|---|
| upstream cavalier **arc tessellation settings** | **NOT the cause.** We never ask cavalier to tessellate. `Polygon2::exterior_to_pline` writes `bulge = 0.0` on every vertex, so the input has no arcs; the arcs in the output are joins cavalier *created*. | 0 input arcs on every fixture |
| repeated **line/arc flattening** | **THE cause.** `Polygon2::from_pline` drops each join's bulge and keeps its two endpoints, so one reflex corner becomes two shallower reflex corners, each of which arc-joins on the next pass. Doubling. | added == arcs, 1.000 |
| **duplicate / near-collinear** accumulation | a CONSEQUENCE, not a cause. Lossless-removable share is **0.0%** at ring 1 on every concave fixture and **99%+** by ring 15 — the debris appears only after the fan has collapsed onto itself. | rosette 0.0% → 99.6% |
| missing **topology-preserving simplification** between offsets | true as a description of scallop's compensation, but it is a dam, not a repair. | see §6 |

The `short-edges` fixture is the useful negative control: 1600 sub-epsilon
segments, **zero** arc joins, **zero** growth, and 99.5% of its vertices
lossless-removable from the first ring. Near-degenerate input is not a
growth mechanism in this primitive — cavalier's `remove_repeat_pos` (which
`polygon.rs` already calls before every offset) absorbs it.

---

## 5. The side-defect nobody bounded

Discarding a join's bulge and keeping its endpoints is not only a
vertex-count problem: the chord **cuts the corner**. The error is the arc's
sagitta, `r · (1 − cos(θ/2))` for a join of turn angle `θ` at offset radius
`r` — **29% of the offset distance at a 90° join**, unbounded by any
tolerance the calling operation knows about.

`polygon.rs`'s own test comment (`test_offset_non_convex_l_shape`, "corners
are rounded") describes geometry the code then throws away. §6's signed
erosion column measures what is left instead: on every concave fixture the
raw cascade sits *inside* the true erosion, i.e. it over-cuts.

This matters more broadly than the doubling does, because **every** consumer
inherits it — including the 20-odd single-shot ones that never iterate.

---

## 6. The candidates

`candidate_arms_on_every_fixture`, release, 50-ring / 120 000-vertex /
25-second-per-arm budget. Every arm is scored at the SAME depth — the deepest
ring the weakest arm reached, capped at 20 — so the quality columns are a
comparison rather than a collection.

| id | arm | plan item |
|---|---|---|
| C0 | raw — production today | (baseline) |
| C1 | drop-only decimation @ `0.75 × authored spacing` | scallop's shipped compensation |
| C2 | cavalier's own `remove_redundant(1e-5)` | (c) collinear/near-duplicate cleanup |
| C3 | `polygon::cleanup_collinear` @ 1 nm | (c), our own |
| C4 | `polygon::simplify_bounded` (RDP + self-intersection guard) @ 10 µm | (b) tolerance-bounded simplification |
| C5-ctl | polyline cascade, chord-flattened every ring | **the isolator** — same machinery as C5, production's flattening |
| C5 | **arcs preserved end-to-end**, tessellated once at the end | (a) exact arc preservation |
| C6 | arcs tessellated every ring by `arcs_to_approx_lines(1 µm)` | bounded flattening (not on the plan's list) |
| C7 | `geo::Buffer` — i_overlay | (d) alternate backend, MEASURED not estimated |

`err@K` is the erosion oracle in µm: `max` / `p50` of `|dist(v, ∂P) − k·step|`
over every ring-K vertex, plus the signed mean (negative = the ring sits
inside the true erosion, i.e. the cascade **over-cuts**).

### rosette-24 (stress)

| arm | rings | verts_end | %/ring | secs | err max | p50 | signed | stop |
|---|---|---|---|---|---|---|---|---|
| C0 | 13 | 88 264 | +45.16 | 28.5 | 4.6 | 0.4 | −1.0 | TIME BUDGET |
| C1 | 50 | 264 | −2.03 | 0.0 | 7.1 | 0.0 | −0.5 | ring limit |
| C2 | 50 | 2 520 | +1.99 | 0.3 | 4.6 | 0.3 | −1.0 | ring limit |
| C3 | 50 | 17 698 | +6.02 | 13.4 | 4.6 | 0.7 | −1.3 | ring limit |
| C4 | 50 | 312 | −1.55 | 0.0 | 13.7 | 0.0 | −0.7 | ring limit |
| C5-ctl | 50 | 89 488 | +9.59 | 3.7 | 4.6 | 0.4 | −1.0 | ring limit |
| **C5** | **50** | **504** (1 800 flat) | **−1.40** | **0.0** | **0.0** | **0.0** | **−0.0** | ring limit |
| C6 | 15 | 16 327 | +20.85 | 0.7 | 3 689.9 | 79.3 | −143.3 | collapsed |
| C7 | 9 | 169 968 | +86.69 | 0.5 | 400.0 | 0.1 | −0.7 | VERTEX CAP |

### comb-16 / dendrite / holed-9 / terrain-midsteep (headlines)

| fixture | C0 | C1 | C2 | C4 | **C5** | C7 |
|---|---|---|---|---|---|---|
| comb-16 verts@stop | 68 548 (17 rings) | 50 | 840 | 89 | **51** (543 flat) | 145 830 (10 rings) |
| comb-16 err max/p50 µm | 26.7 / 12.3 | 11.5 / 0.0 | 26.7 / 12.3 | 35.6 / 12.6 | **0.0 / 0.0** | 0.4 / 0.3 |
| dendrite verts@stop | 123 004 (10) | 211 | 2 612 | 383 | **276** (1 140 flat) | 138 847 (10) |
| dendrite err max/p50 µm | 300.0 / 34.1 | 287.4 / 0.0 | 300.0 / 29.8 | 300.0 / 0.0 | **0.0 / 0.0** | 0.2 / 0.2 |
| holed-9 verts@stop | 80 104 (43) | 580 | 2 884 | 580 | **1 156** (2 308 flat) | 146 524 (8) |
| holed-9 err max/p50 µm | 0.1 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | **0.0 / 0.0** | 0.1 / 0.1 |
| terrain verts@stop | 132 885 (16) | 317 | 918 | 113 | **369** (879 flat) | 194 143 (10) |
| terrain err max/p50 µm | 226.1 / 5.7 | 270.2 / 0.0 | 26.7 / 4.1 | 40.1 / 12.2 | **0.0 / +0.0** | 154.1 / 0.3 |

### What the table says

1. **C5 — arc preservation — dominates on every axis.** It is the only arm
   that reads **0.0 µm max erosion error** on the concave fixtures: it is not
   "within tolerance", it is *exact*, because nothing ever approximated
   anything. It also produces the fewest vertices (504 vs C0's 88 264 on the
   rosette), runs 50 rings in 0.0 s where C0 needs 28.5 s for 13, and — the
   part that matters for the fix's shape — its vertex count **shrinks**
   (−1.4%/ring), because arcs merge as the boundary erodes.
2. **C5-ctl proves the variable is the flattening, not the machinery.** The
   identical polyline cascade, flattened per ring, reproduces C0's inflation
   (89 488 vertices, +9.6%/ring) and C0's error to the decimal (4.6 / 0.4 /
   −1.0). That is the isolator M4's lesson demands, and it lands.
3. **Cleanup-only (C2, C3) is a 10× brake, not a fix.** `remove_redundant`
   costs nothing and is exactly lossless (its error column equals C0's to the
   decimal), but the cascade still grows 2–4%/ring geometrically. Our own
   `cleanup_collinear` is worse and slower (13.4 s vs 0.3 s). If cleanup is
   all that ships, cavalier's is the one to use, not ours.
4. **Tolerance-bounded simplification (C4) works and is cheap** — 312
   vertices at ring 50, 0.0 s, and an error that stays the same order as
   today's. It is the honest generalisation of C1: same bounding effect,
   with the bound written in world units instead of in grid cells.
5. **The alternate backend (C7) is measurably WORSE here, and this refutes a
   plausible prior.** `geo::Buffer` is already in the dependency graph, so it
   was measured rather than estimated. Its geometry is excellent (0.1–0.3 µm
   p50) but it inflates *faster than production does* — 86–99%/ring, hitting
   the vertex cap in 8–11 rings on four fixtures, and 6.2%/ring even on
   `short-edges`, where cavalier is flat. i_overlay's round joins are emitted
   as densified fans with no arc representation to collapse back into, so a
   swap would trade a fixable representation problem for an unfixable one.
   (Direct `i_overlay::outline_custom` exposes join style and
   `preserve_output_collinear`, which `geo::Buffer` does not; that variant is
   untested here and is the only form in which a swap remains arguable.)
6. **C6 — bounded per-ring tessellation — failed badly and is not
   root-caused.** Re-tessellating each join to 1 µm inflates *and* wanders
   (3 690 µm max on the rosette, 12 186 µm on terrain, early collapse on five
   fixtures). Recorded as measured; nobody should build on it without finding
   out why first.
7. **Near-degenerate input is a non-issue.** On `short-edges` every arm is
   flat and exact; C2/C3/C4 reduce 1 600 vertices to 4. The only arm that
   inflates is C7.

### The area check — and today's compensation is not free

`candidate_arms_keep_the_eroded_area` scores ring 20 against a rasterised
distance-transform truth (0.2 mm cells). Vertices at the right distance are
not enough: an arm can be pointwise honest and still have lost an island.

| fixture | truth mm² | C0 | **C1 (today)** | C2 | C4 | **C5** |
|---|---|---|---|---|---|---|
| rosette-24 | 9 665.4 | — (13 rings) | +0.14% | −0.00% | +0.08% | **−0.00%** |
| comb-16 | 8 523.1 | — (18 rings) | **+9.83%** | +0.02% | +0.33% | **+0.00%** |
| holed-9 | 30 248.3 | +0.02% | +0.04% | +0.02% | +0.04% | **+0.02%** |
| terrain-midsteep | 6 226.8 | — (16 rings) | +0.27% | +0.02% | +0.11% | **+0.01%** |

**C1 leaves 9.83% too much material on the comb.** That is drop-only
decimation's real price, and it appears exactly where it should: the comb's
6 mm fingers are only 16 decimation spacings wide, so dropping every vertex
within 0.375 mm of the last kept one rounds the finger tips off and the ring
stops following them. On fixtures whose features are large relative to the
spacing (rosette, holed) it costs 0.04–0.14%.

This is the quantitative form of the M4 wave-9b finding that decimation
interacts with chord density: the interaction is real, it is bounded by
*feature size ÷ decimation spacing*, and the fix candidates (C2, C4, C5) all
avoid it while staying bounded.

---

## 7. The 200-offset acceptance gate

M5's first acceptance gate: *"vertex count grows linearly or remains bounded
across 200 repeated offsets on the stress fixture."*
`two_hundred_offsets_on_the_stress_fixtures`, 150 000-vertex / 30-second
budget per arm:

| arm | rosette-24 @50 / @100 / @200 | dendrite @50 / @100 / @200 | terrain @50 / @100 / @200 | verdict |
|---|---|---|---|---|
| C0 raw | dies at ring **13** | dies at ring **12** | dies at ring **17** | **FAILS** |
| C1 drop-only | 264 / 72 / 48 | 211 / 58 / 27 | 317 / 242 / 125 | passes |
| C2 remove_redundant | 2 520 / 1 464 / 696 | 2 612 / 807 / 345 | 918 / 632 / 339 | passes |
| C3 cleanup_collinear | 17 698 / 8 776 / 3 122 | dies at 29 | dies at 19 | **FAILS** |
| C4 simplify (RDP) | 312 / 120 / 62 | 383 / 111 / 50 | 113 / 87 / 47 | passes |
| C5-ctl chord/ring | 89 488 / 36 316 / 36 502 | 35 764 / 1 294 / 3 | dies at 67 | marginal |
| **C5 arcs preserved** | **504 / 72 / 72** | **276 / 59 / 39** | **369 / 268 / 136** | **passes** |
| C6 bounded flatten | dies at 15 | dies at 6 | dies at 13 | **FAILS** |
| C7 geo::Buffer | dies at 9 | dies at 11 | dies at 10 | **FAILS** |

C1, C2, C4 and C5 all pass, and all four *shrink* — which is what a healthy
cascade does, because an eroding boundary has less to describe. Our own
`cleanup_collinear` (C3) does not pass and should not be pursued; cavalier's
`remove_redundant` should be used instead.

---

## 8. What a 2D consumer inherits: pocket

`pocket_boundary_fidelity_against_the_analytic_erosion`, on a 12-vertex cross
(four reflex corners, 20 mm arms) at 0.5 mm stepover.

> **`pocket_contours_with_cancel` DID NOT FINISH in 20 seconds** on that
> input. `pocket_offsets` — the same cascade without a cancel hook — was left
> running for 13 minutes and 386 MB during this study before being killed.
> The production path is escapable *only* because someone put a
> `check_cancel` in the loop.

That is a live, currently-shipping pathology in a second consumer, found by
this study rather than reported by a user, on a shape a hobby CNC user would
call trivial.

The raw cascade's geometry, per ring (µm from the analytic erosion):

| ring | verts | err max | err p50 | signed |
|---|---|---|---|---|
| 1 | 16 | 0.00 | 0.00 | +0.00 |
| 2 | 24 | 76.12 | 0.00 | −25.37 |
| 4 | 72 | 113.03 | 38.43 | −49.57 |
| 11 | 8 200 | 134.78 | 62.64 | −63.03 |
| 25 | 41 932 | 141.39 | 59.86 | −60.32 |

The first ring is **exact** — the offset itself is not wrong. The error
appears at ring 2 and converges to ≈140 µm max / −60 µm mean: the corner cut
of §5, accumulating and then saturating as the corner rounds off. It is a
systematic **over-cut** at a 0.5 mm stepover, i.e. ~12% of the stepover, in a
2.5D operation whose users think of it as exact.

Per arm, at the deepest ring each reached:

| arm | rings | verts_end | err max µm | signed µm |
|---|---|---|---|---|
| C0 raw | 27 | 42 148 | 141.77 | −60.29 |
| C1 drop-only | 40 | 20 | **1 832.79** | **−538.69** |
| C2 remove_redundant | 40 | 700 | 143.30 | −66.59 |
| C3 cleanup_collinear | 40 | 4 257 | 143.30 | −65.35 |
| C4 simplify (RDP) | 40 | 53 | 191.23 | −111.93 |
| C5-ctl chord/ring | 40 | 42 900 | 143.30 | −60.65 |
| **C5 arcs preserved** | **40** | **316** | **0.00** | **+0.00** |
| C6 bounded flatten | 23 | 1 297 214 | 11 493.90 | −4 905.21 |
| C7 geo::Buffer | 12 | 74 084 | 2.21 | −1.66, **and it panicked** |

Two things this table says that the fixture tables did not:

* **Drop-only decimation is the WORST arm here** — 1.8 mm of error, a 0.54 mm
  mean over-cut. Decimation is safe only when the ring is already *denser*
  than the decimation spacing; on a coarse 12-vertex boundary it deletes real
  corners. Scallop gets away with it because its rings come from a
  marching-squares grid at a known density and the spacing is derived from
  that same grid. **Generalising C1 to every consumer would be a defect**,
  and this is the measurement that says so.
* **C7 `geo::Buffer` panicked** — `index out of bounds: the len is 238 but the
  index is 9223372036854775807`, i_overlay 4.0.7 `bind/solver.rs:91`, at ring
  13 of a twelve-vertex cross. Contained in the harness the same way
  `offset_one` contains cavalier's. A backend swap would be swapping one
  crash-containment story for another, unfixed one.

---

## 9. What the 3D surface says: the M4 envelope oracle

`scallop_oracle_across_ring_cleanups` runs scallop's own cascade through the
new `RingCleanup` seam and scores it with Checkpoint C's instrument. Dial
20 µm, taper tool, `legacy_envelope_quarter` resolution.

### grooved block

| cleanup | secs | rings | moves | cusp p99 µm | ×dial | gouge-normal µm | unfinished mm² | gouge mm² |
|---|---|---|---|---|---|---|---|---|
| decimate@0.75cell (shipped) | 1.4 | 47 | 5 291 | 62.2 | 3.11 | −58.2 | 6.53 | 1.84 |
| keep-all | 3.7 | 48 | 14 332 | **36.1** | **1.80** | −47.3 | **2.16** | **0.91** |
| collinear+dedup | 2.0 | 45 | 1 655 | 47.6 | 2.38 | −95.7 | 4.00 | 8.92 |
| simplify@tol/10 | 2.0 | 45 | 1 655 | 47.6 | 2.38 | −95.7 | 4.00 | 8.92 |

### narrow ridge

| cleanup | secs | rings | moves | cusp p99 µm | ×dial | gouge-normal µm | unfinished mm² | gouge mm² |
|---|---|---|---|---|---|---|---|---|
| decimate@0.75cell (shipped) | 2.7 | 38 | 3 095 | **87.3** | **4.37** | −219.1 | **2.94** | 28.05 |
| keep-all | 4.0 | 37 | 5 005 | 100.4 | 5.02 | −222.3 | 3.75 | 27.86 |
| collinear+dedup | 1.6 | 29 | 1 085 | 129.0 | 6.45 | −345.2 | 6.26 | 24.75 |
| simplify@tol/10 | 1.7 | 29 | 1 085 | 129.0 | 6.45 | −345.2 | 6.26 | 24.75 |

**The ring cleanup is not a free variable for scallop.** Achieved cusp moves
between 1.80× and 6.45× the dial depending purely on what happens to the ring
vertices between offsets. On the grooved block keeping every vertex is
*better* by a wide margin (1.80× vs 3.11×, a third of the unfinished area,
half the gouge) at 2.7× the moves; on the ridge it is *worse* (5.02× vs
4.37×). Neither cleanup arm wins anywhere — both lose cusp and multiply gouge
area (8.92 mm² vs 1.84 on the groove).

Two honest caveats on this table:

* **`collinear+dedup` and `simplify@tol/10` produce identical numbers on both
  fixtures**, to every digit. That is suspicious enough to name: either both
  reduce these particular rings to the same skeleton and the downstream chord
  refinement re-densifies identically, or one is falling through to the
  other's result. Not root-caused. No conclusion in this document rests on
  distinguishing them.
* **These fixtures cannot exhibit the hang.** Both use scallop's default
  boundary — the mesh-bbox **rectangle**, which is convex, so `keep-all`
  costs 2.6× the time rather than 30 minutes. The exponential needs the
  region-scoped dendritic boundaries P2.f met. This table measures the
  QUALITY consequence of each cleanup, not the runtime one.

---

## 10. Against M5's stated acceptance gates

| gate | status |
|---|---|
| vertex count grows linearly or bounded across 200 repeated offsets on the stress fixture | **measured, §7.** C1/C2/C4/C5 pass on all three stress fixtures; C0/C3/C6/C7 fail |
| Hausdorff/area error inside the selected tolerance | **measured, §6/§8.** C5 is exact (0.0 µm); C2 equals today's error; C4 is bounded by its tolerance and verified by a property test; C1 is NOT bounded in world units (1.8 mm on the pocket cross) |
| existing degenerate-input and property tests pass | **yes** — `offset_polygon_degenerate_inputs_r1`, `property_tests`, `generator_extremes_fuzz_r1` green (§12) |
| parameter sweeps for Pocket/Adaptive/Rest/Profile/Trace/Scallop show no unexplained topology change | **baseline only.** Production is unchanged, so the sweeps are green by construction; the comparison this gate wants belongs to the implementation wave. The baseline is recorded in §12 so that wave has something to diff against |
| Criterion benchmark covers square, concave, holed, repeated-cascade | **NOT DONE.** `benches/perf_suite.rs:259` is still square-only. Deliberately deferred: a benchmark that pins today's exponential would have to be re-pinned by the fix in the same wave |

---

## 11. Rejected and deferred, with reasons

* **Alternate backend (`geo::Buffer` / i_overlay) — REJECTED on measurement.**
  It was cheap to measure because `geo` is already a dependency, so it was
  measured rather than estimated. It inflates *faster than production*
  (86–99%/ring, cap in 8–11 rings on four fixtures; 6.2%/ring even on
  `short-edges` where cavalier is flat), and it panicked with an
  out-of-bounds index on a 12-vertex cross. Its pointwise geometry is
  excellent, which is exactly why the vertex result is decisive rather than
  ambiguous. *Not fully closed:* `i_overlay::outline_custom` exposes
  `LineJoin`, `preserve_output_collinear` and `min_output_area`, which
  `geo::Buffer` hides; that variant is untested here and is the only form in
  which this option remains arguable. Clipper2 (FFI, C++17 toolchain, 0.01 mm
  default quantisation) and `clipper2-rust` (pure Rust, BSL-1.0, six months
  old, single maintainer, exposes a per-vertex delta callback) were surveyed
  and not built.
* **Bounded per-ring tessellation (C6) — REJECTED as measured, NOT
  root-caused.** Worse than the thing it replaces on every axis.
* **Our own `cleanup_collinear` (C3) — REJECTED in favour of cavalier's
  `remove_redundant`**, which is faster (0.3 s vs 13.4 s), stronger, and
  already in the dependency.
* **Generalising drop-only decimation (C1) to all consumers — REJECTED.**
  §8: 1.8 mm error / 0.54 mm mean over-cut on a coarse boundary. It is safe
  only where the ring density is known and the spacing derives from it, which
  is scallop's situation and nobody else's.
* **Deferred: the Criterion suite** (§10) and **the `pocket_offsets`
  retirement** — it has no production caller and duplicates
  `pocket_contours_with_cancel`'s loop without its cancel hook.

---

## 12. Gates

`cargo fmt --check` clean and `cargo clippy --workspace --all-targets -D
warnings` zero before each commit.

| target | result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | zero warnings |
| `-p rs_cam_core --lib` | 2 223 passed / **3 failed — the three known adaptive3d reds** (`peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`, `rapid_segment_lifts_to_safe_z_before_traverse`, `planner_sim_dexel_parity_agent_search`) |
| `--test param_sweep -- --ignored` | **56 / 56 ok** — the Pocket/Adaptive/Rest/Profile/Trace/Scallop baseline this gate wants |
| `--test offset_polygon_degenerate_inputs_r1` | ok |
| `--test property_tests`, `--test generator_extremes_fuzz_r1` | ok |
| `--test scallop_candidates_m4` (M4's seam parity) | 3 ok |
| `--test scallop_oracle_validation_m4` | 9 ok |
| `--test scallop_isofield_gouge_m4` | 2 ok |
| `--test scallop_intra_pass_relink_am7` | 5 ok |
| `--test _litmatrix_scallop_refuses_flat`, `--test finish_resolution_policy_pr3`, `--test unified_finish_semantic_regions` | ok |
| `-p rs_cam_mcp` | ok |
| new: `--test offset_growth_m5`, `--test offset_candidates_m5` | 5 + 6 ok (fast pins) |

No fixture needed re-pinning anywhere, which is the expected consequence of
a research wave that changes no default.

---

## 13. Recommendation

**Fix it at the source, by not throwing the arcs away — but land it as an
explicit policy on an arc-carrying ring type, migrating scallop first and
pocket second, and leave the other twenty consumers on today's behaviour
until each is audited.**

The reasoning, in the order the evidence supports it:

1. The cause is single, ours, and measured 1:1 (§4). There is no upstream
   library defect to work around and no tessellation dial to tune.
2. The only arm that is *exact* is the one that never approximates (§6, §7,
   §8). Every other candidate is a brake of some strength: C2 costs nothing
   and buys 10×, C4 costs a tolerance and buys ~200×, C1 costs geometry it
   does not bound.
3. The blast radius is asymmetric (§2). Two consumers compound; both are
   demonstrably broken today (scallop was patched in P2.f, pocket is still
   live and does not finish on a cross). The other twenty inherit only the
   corner cut, which the same change fixes without touching their topology.
4. The alternate backend lost on measurement, not on taste (§11).

Shape of the fix, consistent with M5's "fix shape" section:

* An arc-carrying ring representation at the cascade boundary — not
  necessarily in `Polygon2`; a `RingSet` newtype over `Polyline<f64>` used
  by the two iterating consumers is enough, and keeps `Polygon2`'s
  vertices-only contract intact for everyone else.
* One explicit **flatten policy** with a world-unit tolerance, applied ONCE
  where the geometry leaves the cascade, defaulting to the calling
  operation's own chord tolerance.
* `remove_redundant(pos_equal_eps)` on offset output as a cheap, exactly
  lossless companion — it is free, it is cavalier's own, and it is a
  ~10× brake on the compounding even without the representation change.
* Keep `offset_polygon`'s current signature and behaviour as the
  compatibility policy for the unaudited consumers, exactly as M5 asks.

Two things this recommendation does NOT say:

* It does not say scallop should stop decimating. §9 shows the ring cleanup
  is a live quality variable there (1.80×–6.45× dial), and the arc change
  moves what the cleanup is operating on. Scallop's cleanup must be
  re-decided ON THE ORACLE after the representation changes, not before.
* It does not say the corner-cut over-cut (§5) is small. It is ~12% of the
  stepover on the pocket cross and it is systematic. If the representation
  work is deferred, the bounded-flatten-at-op-tolerance half should not be.

---

## 14. Checkpoint D decision menu

**What is being decided:** whether `offset_polygon` gets an arc-carrying
cascade path, and how far the change is allowed to reach. Nothing here has
changed production. Step 11 of the sequencing checklist stays unticked until
the implementation wave lands.

| # | option | consequence |
|---|---|---|
| **1** | **Arc-carrying cascade + explicit flatten policy, scallop then pocket** *(recommended)*. New ring type at the cascade boundary; `offset_polygon` unchanged for everyone else; scallop's `RingCleanup` re-decided on the M4 oracle afterwards. | Removes the mechanism rather than damping it. Exact geometry (0.0 µm), 200 rings bounded and *shrinking*, and it fixes the unbounded corner cut for the migrating consumers. Largest single risk: it changes ring vertex sets, so every scallop/pocket fingerprint re-pins, and §9 says the cusp will move — the oracle, not the fingerprint, has to adjudicate. |
| **2** | **`remove_redundant` only** — one line in `offset_polygon_inner`, no representation change, no policy. | Cheap, exactly lossless (its error column equals today's to the decimal), ~10× brake, and it turns the pocket cross from "does not finish" into 700 vertices at ring 40. Does NOT stop the doubling (2–4%/ring persists) and does NOT fix the corner cut. Compatible with option 1 later; a good same-day stopgap. |
| **3** | **Tolerance-bounded simplification as an opt-in policy** (`simplify_bounded`, already written and property-tested). | Bounded in world units, 200 rings, 47–62 vertices, cheapest of all the brakes to reason about. Costs 191 µm on the pocket cross vs raw's 142 — i.e. it is a *brake with a stated price*, which is more than C1 offers. Reasonable if option 1 is judged too large. |
| **4** | **Generalise scallop's drop-only decimation.** | **Actively harmful — measured.** +9.83% eroded area on the comb; 1.8 mm error and 0.54 mm mean over-cut on the pocket cross, the worst of nine arms. Not recommended in any form outside scallop, where the ring density is known. |
| **5** | **Swap the backend to i_overlay / Clipper2.** | **Rejected on measurement** (§11): worse inflation than production and an out-of-bounds panic on a 12-vertex cross. Only `i_overlay::outline_custom` with an explicit `LineJoin` remains arguable, and it is untested. |
| **6** | **Defer M5; keep the evidence.** | Zero risk. The pocket hang stays live and unreported, the corner cut stays unbounded, and scallop remains the only consumer that pays for a shared defect. If chosen, §8's pocket finding should at minimum become a tracked defect with the cross fixture attached. |

Independent sub-decisions:

* **7a. Retire `polygon::pocket_offsets`?** No production caller, duplicates
  `pocket_contours_with_cancel`'s loop *without* its cancel hook, and it is
  the function that ran 13 minutes in this study. *(Recommend: yes, or give
  it the cancel hook.)*
* **7b. Should the pocket non-termination (§8) be filed as a defect now,
  independent of which option is taken?** *(Recommend: yes. It is
  reproducible from a twelve-vertex polygon and the fixture is in the
  harness.)*
* **7c. Should `benches/perf_suite.rs`'s square-only offset benchmark be
  extended** to concave / holed / repeated-cascade cases *as part of the
  implementation wave* rather than now, so it is not written to pin the
  exponential and then immediately re-pinned? *(Recommend: with the
  implementation.)*

---

## 15. What this study could not do

* **No live GUI or wanaka validation, and no rendered machined surface.**
  The 2D rings were rendered and read (§3); the 3D oracle is analytic, not a
  dexel simulation of a real part.
* **The scallop oracle fixtures cannot exhibit the hang** (§9) — their
  boundary is the convex mesh-bbox rectangle. The quality answer and the
  runtime answer come from different fixtures, and no single fixture in this
  study shows both.
* **`collinear+dedup` and `simplify@tol/10` are indistinguishable in the 3D
  table** and that is not root-caused (§9).
* **C6's failure is not root-caused** (§6.6). It is recorded as measured.
* **C5-ctl's hole/winding pairing is imperfect in the harness** — on
  `terrain-midsteep` it reports 8 943 polyline vertices but 0 flattened ones
  at ring 50, and its area lands 7.46% off where every other arm is inside
  0.3%. That arm is an isolator, not a candidate, so no recommendation rests
  on it; but its numbers on that one fixture should not be quoted.
* **`cavalier_contours` panics in RELEASE on the `Shape` path** (`expect
  non-empty polyline`, `shape_algorithms/mod.rs:317`) five times across these
  cascades. Production contains it and maps it to "collapsed offset", which
  means **a cascade can terminate early and silently**. Not investigated
  here; it is a separate finding and probably a separate ticket.
* **No implementation.** Every number above comes from arms behind a test
  hook. What the fix costs to build is estimated in §13, not measured.
