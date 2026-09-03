# Pencil watershed-spine — findings and pre-registered bars

Charter: `CHARTER.md`. Rule of this file: each bar is written BEFORE
its instrument runs; results append below, a result never edits a bar.

The claim under test: **D8 flow-accumulation on the rest field extracts
better pencil centerlines than the current NMS+hysteresis+Zhang-Suen
pipeline in `rest_depth_arm`** — more coherent, no coverage loss, no
higher cost, spine no further from the valley bottom.

## Baseline (the number to beat, established by `29a6d61`)

The current extractor's coverage on textured relief is ~0.80
(`RestFieldReport::coverage`, traced ÷ skeleton length surviving
`min_cut_length`). The old hairball was 0.137. **Bars are against the
0.80 pipeline, not the hairball.** A tie is a real, adoptable-as-"no"
outcome — the current path is shipped and simpler.

## P1 metrics — measured on ONE shared rest field, both extractors

For each fixture the instrument computes the rest field once, then runs
extractor A (current) and extractor B (flow-accumulation), and reports:

- **M1 coverage** — `RestFieldReport::coverage` for each extractor.
- **M2 fragmentation** — count of kept centerline polylines and the
  MEDIAN polyline length (mm). Fewer, longer = more coherent. (The
  `29a6d61` pain was a 0.7 mm median; the fix raised it.)
- **M3 total traced length** — `traced_length_mm`, and the fraction of
  skeleton length lost to `min_cut_length` (the shredding tax).
- **M4 valley-bottom fidelity** — for a uniform sample of centerline
  points, the rest depth AT the point ÷ the local cross-section MAX
  rest depth (both extractors read the same field). 1.0 = the spine
  sits exactly at the deepest reachable point (where the ball nestles);
  < 1.0 = the spine rides up a wall.
- **M5 path cost** — cut + link cost via `relink_and_cost_under` under
  the machined-stock `link_ceiling` regime. Fragments cost retract
  links, so a coherent spine should link cheaper.
- **M6 coverage of the OTHER's seams** — the fraction of extractor A's
  covered footprint that extractor B also covers, and vice versa. This
  catches "B is coherent but MISSED a seam A caught".

## Bars (pre-registered, both fixtures)

Adopt-to-opt-in requires ALL of:

- **B1 no coverage regression.** `M1(B) ≥ M1(A) − 0.03`.
- **B2 coherence win.** `M2(B)` median polyline length `≥ 1.30 ×
  M2(A)` median, AND `M2(B)` polyline count `≤ M2(A)` count. Both, so a
  win is genuinely "fewer AND longer", not one traded for the other.
- **B3 no seam loss.** `M6`: extractor B covers `≥ 0.95` of extractor
  A's covered footprint. A coherent spine that drops a real seam A
  caught is a regression, not a win.
- **B4 valley-bottom not worse.** median `M4(B) ≥ M4(A) − 0.05`.
- **B5 cost not worse.** `M5(B) ≤ M5(A) × 1.02`.
- **B6 the render agrees** (X2). On both fixtures, a hillshade-overlay
  render of both spine sets is produced and shows B's spines following
  valley bottoms with no confetti and no missed trunk the eye can see.
  A number that the picture contradicts voids the bar.

## Decision rule

- **All of B1–B6 pass** → adopt flow-accumulation as an OPT-IN
  `PencilSpineExtractor` on the rest arm (P2), default unchanged; a
  promote-to-default decision is separate and needs its own A/B on more
  than these two fixtures.
- **B1 or B3 or B5 fails** → REJECT. Flow-accumulation regressed
  coverage, dropped a seam, or cost more; the current pipeline wins.
- **B1/B3/B5 pass but B2 fails (only a tie on coherence)** → DO NOT
  ADOPT and record the tie: the shipped NMS pipeline already captured
  the win `29a6d61` was after, and a second extractor is not worth the
  maintenance.
- **The render (B6) contradicts a passing number** → the number is
  void; re-examine the instrument before any ruling (Track H rule).

## Synthetic ground-truth fixture (B-side of X4)

A `height_field` mesh carrying a KNOWN valley network — e.g. a Y-shaped
trunk-plus-two-tributaries groove of known length and depth, plus a
broad shallow basin the reference tool fully reaches (rest ≈ 0, MUST
NOT be traced by either extractor). This gives fragment count and
connectivity a right answer:

- the traced spine length should match the groove centreline length
  within the offset tolerance for BOTH extractors (a hard correctness
  check, not a comparison);
- the broad basin must stay untraced by BOTH (rest gate working);
- the Y-junction is the connectivity test: extractor A is expected to
  shred at the junction (degree-3 node — the `29a6d61` failure mode),
  extractor B to keep the trunk continuous through it. If A does NOT
  shred here, the whole premise is weaker than stated — record that.

## Non-goals (scope fence)

- Not testing watershed as a detection CRITERION (rest stays the
  criterion; the postmortem settled this).
- Not reaching the canonical offset-self-intersection method.
- Not a spacing/scallop claim (so the wanaka facet rule does not bar
  wanaka200 here).
- Not resurrecting the Track H tree-tracing strategy (refuted).

## P1 design amendments — pre-registered before the instrument (2026-09-03)

These fix extractor B's construction BEFORE it runs. They are bars, not
results: written ahead of the instrument, never edited by an outcome. The
`P0` module (`crate::flow_accum`) is the shared hydrology they build on.

- **A1 — B runs on `surface_z`, rest-gated; NOT on the rest field.**
  Extractor B computes D8 flow-accumulation on `RestGrid::surface_z` — the
  real drop-cutter topography, which has real outlets — not on the rest
  field or its negation. The rest field is a CLOSED-BASIN field (rest → 0 at
  every rim), so `priority_flood_epsilon` on `-rest` fills each valley flat
  and routes only along the artificial epsilon gradient — confetti, not a
  spine. `surface_z` is the DEM the Track H kit was verified on. The pencil
  CRITERION is preserved by **rest-gating the trunks**: a trunk cell is kept
  only where the field clears A's own thresholds — mask at `threshold`,
  extended to the hysteresis LO floor `0.5 × threshold` to match A's reach.
  This is what "rest-gated by construction" means operationally. X3 holds:
  both fields (`surface_z`, `rest`) come off the one `RestGrid`, computed
  once. **Falsifier:** P1's first assertion is a 1-D toy (`rest = bump,
  rim = 0`) — if `priority_flood_epsilon` on `-rest` yields a non-flat,
  routable gradient, the closed-basin argument is wrong; record and
  reconsider A1 before ruling.

- **A2 — trunk→skeleton threshold `T`, pre-registered.** `{acc ≥ T}` is
  closed under the D8 receiver map, so the kept set is already an ~1-cell
  forest; B traces it by FOLLOWING RECEIVERS, no Zhang-Suen. `T` sets
  fragment count and coverage directly, so it is fixed here, not after the
  bars: `T = π·(d/cell)² / 4` upstream cells, the area of a disc one pencil
  DIAMETER `d` across (rationale: a trunk must drain at least the tool's own
  footprint to earn a pass). Report M1–M6 at `T` PLUS a sensitivity sweep at
  `{0.5·T, T, 2·T}`; the single pick is reported-not-load-bearing. Expected
  failure locus: **B3 (seam loss)** at short, steep gullies whose small
  upstream area falls under `T`.

- **A3 — smoothing asymmetry, stated.** `rest_grid.rest` is the RAW
  continuous field; extractor A box-smooths it internally before NMS.
  Field CONSTRUCTION (the dual-tool drop) is shared; per-extractor
  CONDITIONING is each pipeline's own — A: box-smooth; B: priority-flood +
  flat-resolve. M4 (valley-bottom fidelity) samples `rest_grid.rest` (raw)
  for BOTH extractors, so the depth read that M4 compares is shared.

## Results

### P0 — flow-accum module promoted (2026-09-03)

`crate::flow_accum` holds the D8 hydrology (`priority_flood_epsilon`,
`resolve_flats`, `d8_receivers`, `d8_accumulation`, `neighbour`), promoted
byte-identical from three Track H copies. All three `#[ignore]` wanaka
censuses reproduce their baselines (w0b `mean/p1 2.780` / 49 basins; h0
`D_pot 7.27 pp`; h1 pass). Commit `17e9e74c`.

### P1 — instrument + synthetic A/B (2026-09-03)

Instrument: `crates/rs_cam_core/tests/pencil_spine_ab_p1.rs`.

**A1 falsifier — PASSES.** An isolated `-rest` dome fills flat under the
priority flood (interior spread `1.2e-5 mm < 1e-3`). The closed-basin
argument holds; B correctly runs on `surface_z`, not `-rest`.

**Two lying metrics fixed before ruling** (advisor review): the pencil
diameter now reads the tip `cusp_radius()` (Ø1), not the shank (Ø6) — the
first run's `T_disc 28 mm²` and 3 mm M6 stamp were 6× too large. A
ground-truth `recall` was added (fraction of the KNOWN Y centreline within
one tip-radius of any traced point) — the honest coverage measure the
within-extractor `traced/skeleton` ratio (M1) cannot give while A is a
hairball.

Synthetic Y-valley, cell 0.25 mm, pencil tip Ø1, `T_disc 0.79 mm²`:

| fixture | extractor | lines | median_len | recall | valley_fid |
|---|---|---|---|---|---|
| flat-closed | A NMS | 86 | 1.301 | 0.680 | 0.969 |
| flat-closed | B @ T | 229 | 3.213 | 1.000 | **0.396** |
| sloped-exit | A NMS | 87 | 1.258 | 0.602 | 0.972 |
| sloped-exit | B @ T | **32** | 2.867 | **0.975** | 0.846 |

Renders: `target/pencil_spine_ab_p1/synthetic_{flat,sloped}_overlay.svg`
(A blue, B red, rest field purple). **Read before ruling:**

- **Flat-closed is a decisive B LOSS.** With no along-valley slope, flow
  accumulation has no along-valley direction, so it drains each cell
  SIDEWAYS to the nearest wall — the render shows B as 229 short hatches
  running ACROSS the groove, not a spine along it (valley_fid 0.396). The
  diagnostic confirms the topology, not the tracer: 197 distinct outlets on
  the flat floor. **A flat-floored (constant-depth) rest crease is a
  worst-case for flow-accum, and it is a common pencil case.**
- **Sloped-exit is a coherence + recall WIN for B, with one weakness.** B
  traces the trunk + both tributaries as one continuous spine to the outlet
  — 32 lines vs A's 87, recall 0.975 vs 0.602 (B covers far more of the true
  groove). But B carries short WALL-SPURS (visible as red ticks up the
  walls) that A's `cleanup_ridge_graph` would prune; they drag B's
  valley_fid to 0.846 (< A's 0.972) and inflate its line count. Spur-pruning
  is an obvious, unimplemented B lever.

**A2 sensitivity: T does not bind here.** {0.5T, T, 2T} give identical
surviving-spine counts on both fixtures; the rest-gate (rest ≥ 0.5×
threshold) selects, not the accumulation threshold. On the flat fixture mask
`acc p50 4.6 / p95 9.6 mm²`; on the sloped fixture `p50 0.6 / p95 385 mm²`
(the slope concentrates flow into a real trunk). A2 is reported, not
load-bearing on these fixtures.

**Separate A defect (one line, not part of the ruling):** both A and B trace
the shallow basin's RIM (a sharp 0.3 mm step — a fixture artifact), but the
basin FLOOR stays untraced by both, so the rest gate works. A real basin
with a smooth edge would not trip this.

### P1 — wanaka A/B (real terrain, the operator's fixture) + RULING

Wanaka terrain, cell 0.5 mm, pencil tip Ø1, `T_disc 0.79 mm²`:

| extractor | lines | median_len | traced_mm | valley_fid |
|---|---|---|---|---|
| A NMS | 1139 | 1.754 | **3,084** | 0.935 |
| B @ T | 3919 | 9.288 | **71,010** | 0.784 |

`B diag @ T`: kept 127,947 cells, **2,083 distinct outlets**; mask acc p50
15.2 / p95 80.8 mm². M6: B covers 0.959 of A's footprint, but A covers only
**0.123** of B's.

Render `target/pencil_spine_ab_p1/wanaka_spines_only.png` (rest rects
stripped so it rasterises; the 12 MB full overlay is
`wanaka_overlay.svg`). **The picture is decisive:** B (red) fills nearly the
whole board with dense parallel hatching — it traces the entire terrain
DRAINAGE NETWORK (71 km, 2083 outlets, draining to the grid edges), not a
sparse pencil-seam set. A (blue) is the faint sparse network of the actual
significant rest ridges underneath.

> ⚠ **RETRACTED — this ruling ran on a FLOODED board (the 71 km / 23× figure
> is a flood artifact). See the CORRECTION and RE-RULING sections below. The
> reject DIRECTION survives, but on 8.7×, not 23×, and for a corrected
> reason.**

**RULING — REJECT (flow-accumulation is not adopted).**

By the decision rule ("B1 or B3 or B5 fails → REJECT"), and directly:

- **B5 (cost) fails by ~23×.** B traces 71,010 mm vs A's 3,084 mm. The cost
  bar is `≤ 1.02× A`; a 23× traced-length blow-up fails it by any measure.
  `relink_and_cost_under` (M5) was NOT run — a 23× length gap makes the
  costed number a foregone conclusion, and the heavy rig would only quantify
  a rejection already settled by the raw length and the render.
- **B2 (coherence) fails.** B has MORE lines than A (3919 > 1139), not
  fewer. The per-line median is longer (9.29 vs 1.75), but the bar requires
  fewer AND longer; B trades one for the other.
- **B4 (valley-bottom fidelity) fails.** 0.784 < A − 0.05 = 0.885. On real
  terrain B rides drainage slopes, not valley bottoms.
- **M6 exposes the mechanism:** B does not LOSE A's seams (it covers 0.959
  of A), it DROWNS them — A covers only 0.123 of B, i.e. ~8× of B's
  footprint is terrain drainage that is not a tool-relevant seam.

**This is the drainage-deletion lesson (`53293c96`), confirmed empirically.**
The charter's hope — that rest-gating keeps flow-accumulation "clear of the
drainage-deletion ruling" — is FALSIFIED. Rest-gating at the natural floor
(`0.5 × min_valley_depth`) does not constrain flow-accumulation on real
terrain, where shallow rest against the reference exists almost everywhere,
so flow accumulation recovers the whole hydrology network rather than the
sparse pencil seams. The synthetic sloped-valley coherence win (32 vs 87
lines, recall 0.975) was a single clean valley in isolation; it does not
survive contact with a full terrain drainage field.

**Raising T does not fix it — the failure is SELECTION, and A2's T is the
wrong knob.** On wanaka the mask `acc p95 = 80.8 mm²`, so `2T = 1.58 mm²`
selects essentially everything; to cut 71 km down to A's 3 km you would need
`T` roughly two orders of magnitude larger, and at that point B keeps only
the DEEP major terrain valleys — which are exactly the ones A's ridge
detector already finds. The gate that would actually separate a tool-relevant
seam from terrain drainage is rest-RIDGE detection — and that IS A's NMS.
Flow-accumulation adds nothing on top of it. The NMS+hysteresis+Zhang-Suen
pipeline (`29a6d61`, coverage 0.80) stands as the pencil-spine extractor.

**Bounded positive, stated so it is not a lead:** on a single sloping valley
IN ISOLATION (one junction, no competing drainage), flow-accumulation traces
a more coherent, higher-recall spine than NMS (with wall-spurs A would
prune). But no real-part fixture has been found where that isolation holds —
every real rest valley sits in a terrain field with competing drainage, which
is what wanaka showed. This is not a follow-up lead.

---

## ⚠ CORRECTION (2026-09-03) — the first wanaka ruling was on a FLOODED board

**The operator looked at the wanaka render and said it was "pure fuzz" — not
the dendritic river lines Track H produced. He was right; the instrument had
a bug, and the REJECT above is RETRACTED pending the corrected re-run.**

**The bug.** Extractor B ran D8 flow-accumulation on the raw `surface_z`
without masking the board. A wanaka relief carries a raised machining rim
(Track H's w0 header documents this), so the whole surface is ONE CLOSED
BASIN with no outlet. The priority flood then fills ~all of it, and the
flat-resolver ramps that giant flat toward the rim — producing perfectly
CARDINAL parallel lines (every B spine had constant x in the SVG), a flood
artifact, not drainage. The "71 km / 23× over-trace" measured a flooded
board. This is the exact failure Track H's w0 census had already diagnosed
and fixed with `land_view`; extractor B skipped that step.

**Proof (no re-run needed — measured on the fast synthetic fixtures):** a
`raised_fraction` diagnostic (`filled > z + eps`, Track H's own flood test)
plus a receiver-direction histogram now run in `flow_diag`.

| synthetic fixture | raw surface RAISED by flood | receiver directions |
|---|---|---|
| sloped-exit (real outlet) | **0.2 %** | mostly one (down-slope) |
| flat-closed (no outlet) | **97.9 %** | ramped |

The closed groove floods 97.9 %; the one that exits the edge floods 0.2 %.
That is the mechanism.

**Amendment A4 — the general fix (no wanaka constant).** Extractor B now sets
`nodata` wherever `rest < 0.5 × threshold` — it routes INSIDE THE REST MASK
(A's own hysteresis-LO domain), so every rest valley drains to its own rim
and gets a real outlet. This is defensible on any part, not a terrain-
specific sea mask, and it is the same rest gate B applies afterwards. A
generic machined part is ALWAYS a closed basin (bounded stock), so flow
accumulation needs an outlet the part geometry does not supply; the rest mask
is that outlet set.

**Corrected synthetic results with A4** (the fix helps B substantially —
fewer, longer, coherent spines):

| fixture | extractor | lines | median_len | recall | valley_fid |
|---|---|---|---|---|---|
| flat-closed | A NMS | 86 | 1.301 | 0.680 | 0.969 |
| flat-closed | B @ T (A4) | **13** | 25.854 | 0.989 | 0.666 |
| sloped-exit | A NMS | 87 | 1.258 | 0.602 | 0.972 |
| sloped-exit | B @ T (A4) | **7** | 27.255 | 0.975 | 0.854 |

B is now coherent on BOTH synthetics (flat 229→13 lines, sloped 32→7).

### Corrected wanaka result with A4 + the RE-RULING

Flood fixed: raw surface RAISED 95.9% → A4-masked surface RAISED **8.9%**;
receiver directions now spread across all 8 bins. Render
`target/pencil_spine_ab_p1/wanaka_a4_spines.png` shows real branching rivers
(matching Track H), NOT the earlier cardinal fuzz.

| extractor | lines | traced_mm | valley_fid |
|---|---|---|---|
| A NMS | 1,139 | 3,084 | 0.935 |
| B @ T (A4) | 5,360 | 26,763 | 0.919 |

T-sweep (the A2 "why not raise T" falsifier, now with real numbers): traced
length **0.5T = 41,024 mm · T = 26,763 mm · 2T = 18,813 mm**. Even 2T is
**6.1× A**. No threshold rescues it.

**RE-RULING — REJECT (same direction as the retracted ruling, corrected
mechanism and magnitude).**

The sharp finding: **surface drainage lines and rest-RIDGE lines are different
curves, and pencil wants the ridge.** B traces the dendritic surface drainage
within each rest region (5,360 lines, 26.8 m); A traces the rest ridges (1,139
lines, 3.1 m). B4 tying at 0.919 is consistent — a drainage line sits low in
the valley, so it reads deep rest, but it is NOT the medial line of the rest
region. That is why B is 8.7× the length yet not simply "A's job done longer":
it is the wrong curve family for the feature.

- **B5 (cost) fails.** Read on TRACED LENGTH as a lower bound on cost (links
  only add, and B's 5,360 fragments link worse than A's 1,139): 26,763 mm vs
  3,084 mm = **8.7×**, against a ≤ 1.02× bar. The full `relink_and_cost_under`
  was not run; an 8.7× length gap makes the costed number moot, and the length
  is a floor, not the whole cost.
- **B2 (coherence) fails.** 5,360 lines > A's 1,139 — more, not fewer.
- **B4 passes** (0.919 ≥ 0.885) and **B1 passes** — but a metric passing on
  the wrong curve family is not a reason to adopt.
- **B3/M6 is NOT cited.** The footprint stamp is one cell at 0.5 mm and A's
  polylines are resampled, so B-covers-A is biased low; the number is not
  load-bearing and is dropped from the ruling. B5 carries the verdict alone.

**Why raising T cannot fix it (A2, resolved):** the T-sweep above — even 2T
stays 6.1× A. And the gate that separates a tool ridge from a drainage line is
ridge detection, which IS A. Flow-accumulation adds nothing A does not already
have.

The drainage-deletion lesson (`53293c96`) holds — now demonstrated on CORRECT
dendritic drainage, not a flood artifact. The shipped NMS pipeline (`29a6d61`)
stands. **Credit: the operator caught the flood by eye ("pure fuzz, nothing
like the river lines") — the aggregate numbers alone read as a confident,
wrong REJECT.**
