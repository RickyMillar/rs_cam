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

(appended as they land)
