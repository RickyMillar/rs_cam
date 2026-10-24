# P2 C19 — the streamline march and the harmonic (e), clipped and split per lobe (2026-10-24)

Status: PROTOTYPE COMPLETE (Python only; no Rust edited, no cargo run, no
commit). Page: `farside_ideas_v6.html` (v5 is frozen; §"Correction" says what changed). Scripts: `phase2/c19/` (`clip.py`,
`build.py`, `run_c19.py`, `page_c19.py`, `assert_c19.py`). Run `run_c19.py`
(about a minute), then `page_c19.py`, then `assert_c19.py`. `page_c19.py`
writes this note and the page; `assert_c19.py` recomputes every quoted
number. C15's and C17's scripts are imported and not edited.

## 1. The question

The operator, on C17's march renders: "The paths look great, is the issue
that down both arms is one move? So the tool traverses both arms at the same
time? If we could somehow clip the overlap and detect the split and traverse
the paths differently the shape is perfect."

## 2. Method

Inputs: C17's march loops (`curves_c17.npz`, run `h_step`), C15 (a)
(`curves_whole.npz`), C15's replay, c·D = 2.175868 mm (c = 0.3626), the C14
turn bound s + (1 + √2)·cell = 3.207 mm (cell 0.5 mm), c plus one cell
0.3793.

- **Clip.** C17's replay of each march loop in order; a sample is engaged when its area per mm over one pass step is more than one cell width (0.1 mm). Each engaged run is a cut piece. The pieces lie on the C17 loops: the largest distance of a cut point from its loop is 1.4e-14 mm (star) and 1.4e-14 mm (pocket). Three rules, each from the raster or the tool: a closed loop whose air lies only within R of arc before its seam is a ring (that air is the loop's own start); a run of one sample, or one that removes no cell, is not a piece (1 star, 2 pocket); two pieces of one loop and one stock part with an air gap shorter than one pass step are one piece, because the split reads over one pass step (1 star, 3 pocket).
- **Detect the split.** Before each level the standing stock is labelled by edge connectivity. A piece belongs to the part from which it removes the most cells. A lobe is a chain of parts with one child each; it ends where its part splits. No tuned number.
- **Order.** Rings in the core with C17's seam join. Then lobes depth first, each to its end. In a lobe a piece may be cut when every lower-level piece of the same lobe whose tool discs overlap it (centre paths within 2R) is cut; among those, the nearest entry next. A piece of the same loop next along the loop is cut next through the loop's own air run when that is the nearest step.
- **Link.** C14 one-way: each piece in climb (stock on the right; check: 2442 samples right, 0 left, star); a keep-down return through what was just cut (the shorter of the way back along the piece and C17's string-pulled route); then the turn to the next piece. A turn longer than the bound is a plunge (the brief). A link that reads over c is a plunge too: FreeCAD's test leaves 2·s·R at each end out, and the rs_cam replay admits no step over c that is not a turn. Zigzag as a second variant.
- **Count.** Entries = 1 (the starter) + plunges + refusals. A refusal is a run of cut samples over c + one cell (0.3793) that is not a turn, where the rs_cam replay would refuse the step; the run that leaves a plunge is exempt, as in the replay.

**The answer in one line.** Yes: when the loops are cut only where they remove stock and one arm at a time, the march shape stays exactly and its air falls from 4084 mm to 727 mm (star). But the march still engages 1.39 times the length of (a), and its pieces start with a head-on run along the wall. The same clip on C15's harmonic loops (e) (operator's addition) gives the fewest entries of all plans here (35 star, 23 pocket), but its cut is thin: it engages 2.62 (star) and 3.06 (pocket) times the length of (a).

## Cards (one sentence each, as on the page)
- Star, Full march (C17): The march cuts every loop whole, so the tool runs again along all the wall that earlier loops cut: 74% of its path cuts nothing. Engaged / air mm 1422 / 4084; Retracts (FreeCAD rule) 0; Time, air at cut feed 3.67 min.
- Star, March, clipped and split, one-way: The same loops cut only where they remove stock, one arm at a time, with a keep-down return and a short turn between pieces: the shape stays, and the time falls from 3.67 to 1.50 min. Engaged / keep-down mm 1243 / 718; Plunges / turns 10 / 93; Time, air at cut feed 1.50 min.
- Star, Harmonic (e), clipped and split, one-way: C15's harmonic level lines with the same clip, split and links: the time falls from 3.67 to 2.16 min, but the cut is thin (median swept width 0.123 D), so it engages 2.62 times the length of (a). Engaged / keep-down mm 2345 / 749; Plunges / turns 11 / 79; Time, air at cut feed 2.16 min.
- Star, C15 (a), same linking: The exact-distance arcs of C15 (a) with the same links: 893 mm of cut against 1243 mm for the march, because the march makes more and thinner passes into the tips. Engaged / keep-down mm 893 / 472; Plunges / turns 17 / 47; Time, air at cut feed 1.10 min.
- Pocket, Full march (C17): The march cuts every loop whole, so the tool runs again along all the wall that earlier loops cut: 83% of its path cuts nothing. Engaged / air mm 1735 / 8584; Retracts (FreeCAD rule) 0; Time, air at cut feed 6.88 min.
- Pocket, March, clipped and split, one-way: The same loops cut only where they remove stock, one lobe at a time, with a keep-down return and a short turn between pieces: the shape stays, and the time falls from 6.88 to 2.25 min. Engaged / keep-down mm 1601 / 1567; Plunges / turns 10 / 62; Time, air at cut feed 2.25 min.
- Pocket, Harmonic (e), clipped and split, one-way: C15's harmonic level lines with the same clip, split and links: the time falls from 7.08 to 4.15 min, but the cut is thin (median swept width 0.062 D), so it engages 3.06 times the length of (a). Engaged / keep-down mm 3828 / 2249; Plunges / turns 22 / 85; Time, air at cut feed 4.15 min.
- Pocket, C15 (a), same linking: The exact-distance arcs of C15 (a) with the same links: 1251 mm of cut against 1601 mm for the march, because the march makes more and thinner passes. Engaged / keep-down mm 1251 / 1054; Plunges / turns 15 / 43; Time, air at cut feed 1.68 min.

## 3. Tables

Columns: entries = 1 + plunges + refusals. Plunges by kind: seam (core ring joins), turn over the bound, lobe change with no keep-down route, in-lobe link with none. Head: the piece's leading run on the wall. Departure: within R of arc after the piece leaves the wall. Interior: the rest more than R from the piece ends. Readings are swept width / D.

Star:

<!-- star:begin -->
| Plan | Entries | Plunges = seam / turn over bound / lobe / in-lobe | Refusals | Turns (over c) | Turn feed factor, median | Keep-down mm | Engaged mm | Air mm | Time, air at cut feed, min | Time, air at link feed, min | Head max D | Departure max D | Interior max D (over) | Coverage left mm² (unreachable) | Swept width of engaged samples, median D | Piece mean width, median / min D | Mean width D |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C17 full march (sweep.json) | — | 0 (FreeCAD rule) | — | — | — | 18 | 1422 | 4084 | 3.67 | 9.12 | — | — | 0.4988 (89) | 57.1 (55.6) | — | — | — |
| C17 full harmonic (e) (sweep.json c15_e) | — | 0 (FreeCAD rule) | — | — | — | 16 | 2389 | 3119 | 3.67 | 7.83 | — | — | 0.3575 (0) | 57.6 (56.1) | — | — | — |
| march, one-way, C14 straight turns (the brief as written) | 157 | 8 = 8 / 0 / 0 / 0 | 148 | 93 (0) | 1.00 | 786 | 1396 | 794 | 1.54 | 2.60 | 0.700 | 0.625 | 0.364 (0) | 56.9 (55.5) | 0.316 | 0.252 / 0.018 | 0.300 |
| march, one-way, head as the turn, C14 bound | 74 | 10 = 8 / 2 / 0 / 0 | 63 | 93 (77) | 0.71 | 718 | 1243 | 727 | 1.50 | 2.47 | 0.700 | 0.622 | 0.363 (0) | 56.9 (55.5) | 0.318 | 0.247 / 0.018 | 0.305 |
| march, one-way, head as the turn, C16 slanted bound | 70 | 8 = 8 / 0 / 0 / 0 | 61 | 95 (79) | 0.71 | 718 | 1238 | 727 | 1.50 | 2.47 | 0.700 | 0.622 | 0.363 (0) | 56.9 (55.5) | 0.318 | 0.247 / 0.018 | 0.305 |
| march, zigzag, C14 straight turns | 153 | 9 = 8 / 1 / 0 / 0 | 143 | 91 (6) | 1.00 | 121 | 1347 | 179 | 1.10 | 1.34 | 0.700 | 0.625 | 0.364 (0) | 56.9 (55.5) | 0.318 | 0.259 / 0.018 | 0.306 |
| C15 (e) harmonic, one-way, C14 straight turns (the brief as written) | 34 | 11 = 11 / 0 / 0 / 0 | 22 | 79 (0) | 1.00 | 763 | 2372 | 780 | 2.18 | 3.22 | 0.769 | 0.608 | 0.352 (0) | 57.4 (56.1) | 0.121 | 0.125 / 0.019 | 0.175 |
| C15 (e) harmonic, one-way, head as the turn, C14 bound | 35 | 11 = 11 / 0 / 0 / 0 | 23 | 79 (6) | 1.00 | 749 | 2345 | 767 | 2.16 | 3.19 | 0.769 | 0.587 | 0.349 (0) | 57.4 (56.1) | 0.123 | 0.128 / 0.019 | 0.176 |
| C15 (e) harmonic, one-way, head as the turn, C16 slanted bound | 35 | 11 = 11 / 0 / 0 / 0 | 23 | 79 (6) | 1.00 | 749 | 2345 | 767 | 2.16 | 3.19 | 0.769 | 0.587 | 0.349 (0) | 57.4 (56.1) | 0.123 | 0.128 / 0.019 | 0.176 |
| C15 (e) harmonic, zigzag, C14 straight turns | 35 | 11 = 11 / 0 / 0 / 0 | 23 | 80 (0) | 1.00 | 294 | 2329 | 354 | 1.86 | 2.33 | 0.780 | 0.562 | 0.351 (0) | 57.4 (56.1) | 0.127 | 0.133 / 0.019 | 0.178 |
| C15 (a), one-way, C14 straight turns (the brief as written) | 85 | 17 = 8 / 0 / 5 / 4 | 67 | 47 (47) | 0.39 | 472 | 893 | 478 | 1.10 | 1.73 | 0.396 | 0.519 | 0.361 (0) | 77.3 (56.1) | 0.341 | 0.369 / 0.361 | 0.382 |
| C15 (a), one-way, head as the turn, C14 bound | 85 | 17 = 8 / 0 / 5 / 4 | 67 | 47 (47) | 0.39 | 472 | 893 | 478 | 1.10 | 1.73 | 0.396 | 0.519 | 0.361 (0) | 77.3 (56.1) | 0.341 | 0.369 / 0.361 | 0.382 |
| C15 (a), one-way, head as the turn, C16 slanted bound | 85 | 17 = 8 / 0 / 5 / 4 | 67 | 47 (47) | 0.39 | 472 | 893 | 478 | 1.10 | 1.73 | 0.396 | 0.519 | 0.361 (0) | 77.3 (56.1) | 0.341 | 0.369 / 0.361 | 0.382 |
| C15 (a), zigzag, C14 straight turns | 82 | 17 = 8 / 0 / 5 / 4 | 64 | 47 (47) | 0.39 | 0 | 893 | 7 | 0.78 | 0.79 | 0.396 | 0.495 | 0.361 (0) | 77.2 (56.1) | 0.340 | 0.369 / 0.362 | 0.382 |
| (a) + one-way in Rust (D12, geodesic_local_oneway) | 34 | 19 join + 14 mop | — | 45 (3 over the bound) | 0.45 | 967 | 1193 (cut length) | — | 1.44 | 2.73 | — | — | 0.4638 (any non-turn step) | mop | — | — | — |
<!-- star:end -->

Pocket:

<!-- pocket:begin -->
| Plan | Entries | Plunges = seam / turn over bound / lobe / in-lobe | Refusals | Turns (over c) | Turn feed factor, median | Keep-down mm | Engaged mm | Air mm | Time, air at cut feed, min | Time, air at link feed, min | Head max D | Departure max D | Interior max D (over) | Coverage left mm² (unreachable) | Swept width of engaged samples, median D | Piece mean width, median / min D | Mean width D |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C17 full march (sweep.json) | — | 0 (FreeCAD rule) | — | — | — | 11 | 1735 | 8584 | 6.88 | 18.32 | — | — | 0.4396 (91) | 1.7 (0.0) | — | — | — |
| C17 full harmonic (e) (sweep.json c15_e) | — | 0 (FreeCAD rule) | — | — | — | 110 | 3852 | 6774 | 7.08 | 16.12 | — | — | 0.3632 (0) | 0.0 (0.0) | — | — | — |
| march, one-way, C14 straight turns (the brief as written) | 117 | 7 = 5 / 2 / 0 / 0 | 109 | 65 (0) | 1.00 | 1562 | 1719 | 1566 | 2.25 | 4.34 | 0.544 | 0.422 | 0.439 (5) | 1.8 (0.0) | 0.305 | 0.284 / 0.021 | 0.293 |
| march, one-way, head as the turn, C14 bound | 65 | 10 = 5 / 5 / 0 / 0 | 54 | 62 (58) | 0.72 | 1567 | 1601 | 1572 | 2.25 | 4.35 | 0.496 | 0.419 | 0.439 (5) | 1.8 (0.0) | 0.306 | 0.288 / 0.021 | 0.297 |
| march, one-way, head as the turn, C16 slanted bound | 56 | 5 = 5 / 0 / 0 / 0 | 50 | 67 (62) | 0.72 | 1567 | 1588 | 1572 | 2.26 | 4.35 | 0.496 | 0.419 | 0.439 (5) | 1.4 (0.0) | 0.307 | 0.289 / 0.021 | 0.297 |
| march, zigzag, C14 straight turns | 124 | 11 = 5 / 2 / 1 / 3 | 112 | 64 (15) | 1.00 | 1427 | 1657 | 1494 | 2.17 | 4.16 | 0.546 | 0.419 | 0.439 (5) | 1.8 (0.0) | 0.308 | 0.289 / 0.023 | 0.299 |
| C15 (e) harmonic, one-way, C14 straight turns (the brief as written) | 23 | 22 = 10 / 11 / 0 / 1 | 0 | 85 (0) | 1.00 | 2249 | 3828 | 2288 | 4.15 | 7.21 | 0.164 | 0.049 | 0.362 (0) | 3.3 (0.0) | 0.062 | 0.112 / 0.016 | 0.130 |
| C15 (e) harmonic, one-way, head as the turn, C14 bound | 23 | 22 = 10 / 11 / 0 / 1 | 0 | 85 (0) | 1.00 | 2249 | 3828 | 2288 | 4.15 | 7.21 | 0.164 | 0.049 | 0.362 (0) | 3.3 (0.0) | 0.062 | 0.112 / 0.016 | 0.130 |
| C15 (e) harmonic, one-way, head as the turn, C16 slanted bound | 15 | 14 = 10 / 3 / 0 / 1 | 0 | 93 (0) | 1.00 | 2248 | 3813 | 2303 | 4.20 | 7.27 | 0.164 | 0.049 | 0.362 (0) | 3.3 (0.0) | 0.063 | 0.113 / 0.016 | 0.130 |
| C15 (e) harmonic, zigzag, C14 straight turns | 19 | 18 = 10 / 8 / 0 / 0 | 0 | 89 (0) | 1.00 | 451 | 3792 | 526 | 2.95 | 3.66 | 0.129 | 0.049 | 0.362 (0) | 3.5 (0.0) | 0.063 | 0.110 / 0.017 | 0.131 |
| C15 (a), one-way, C14 straight turns (the brief as written) | 75 | 15 = 5 / 5 / 1 / 4 | 59 | 43 (43) | 0.39 | 1054 | 1251 | 1059 | 1.68 | 3.10 | 0.560 | 0.401 | 0.360 (0) | 50.0 (0.0) | 0.313 | 0.331 / 0.098 | 0.342 |
| C15 (a), one-way, head as the turn, C14 bound | 75 | 15 = 5 / 5 / 1 / 4 | 59 | 43 (43) | 0.39 | 1054 | 1251 | 1059 | 1.68 | 3.10 | 0.560 | 0.401 | 0.360 (0) | 50.0 (0.0) | 0.313 | 0.331 / 0.098 | 0.342 |
| C15 (a), one-way, head as the turn, C16 slanted bound | 72 | 12 = 5 / 2 / 1 / 4 | 59 | 46 (46) | 0.39 | 1054 | 1251 | 1059 | 1.70 | 3.11 | 0.560 | 0.401 | 0.360 (0) | 49.2 (0.0) | 0.313 | 0.329 / 0.098 | 0.339 |
| C15 (a), zigzag, C14 straight turns | 78 | 14 = 5 / 4 / 1 / 4 | 63 | 44 (44) | 0.39 | 25 | 1251 | 30 | 1.00 | 1.04 | 0.560 | 0.393 | 0.362 (0) | 49.9 (0.0) | 0.313 | 0.331 / 0.098 | 0.341 |
| (a) + one-way in Rust (D12, geodesic_local_oneway) | 41 | 29 join + 11 mop | — | 50 (16 over the bound) | 0.40 | 975 | 1488 (cut length) | — | 1.64 | 2.94 | — | — | 0.3574 (any non-turn step) | mop | — | — | — |
<!-- pocket:end -->

## The split

- Star: 110 cut pieces in 6 lobes. Loop 8 is the first loop with more than one cut piece. At that level the stock splits into the five arms (lobes 1 to 5). The pieces are 26% of the loop length (1406 of 5490 mm). Each piece removes at least 0.94 of its cells from one part.
- Pocket: 98 cut pieces in 5 lobes. Loop 5 is the first loop with more than one cut piece (it reaches the bottom and left walls), but the stock stays one part round the island. It splits at levels 31, 33, behind the island where the two fronts meet. Each piece removes at least 0.91 of its cells from one part. The pieces are 17% of the loop length (1724 of 10313 mm).
- The pocket's core lobe holds two pieces per level (above and below the island) for 26 levels. Connectivity cannot tell the two sides apart; the overlap rule of the order lets the tool do one side ahead of the other.


## The wall departure overload

C17 read 0.50 D (star) and 0.44 D (pocket) where a loop leaves a wall. The clip does not remove it. The cause is now clear: each piece starts with a head, a run along the wall into new stock, before it leaves the wall. That run is the step-over at the arc end, the same step that C14 makes a counted turn. Under C14's straight turn it stays inside the piece at full feed.

- Star, C14 straight turns: the head runs on the wall for 1.9 mm (median) and reads up to 0.700 D in 95 pieces, at full feed. With the head as the turn (C14 bound 3.207 mm): 93 turns, 2 over the bound (each a plunge), feed factor median 0.71; the head then reads up to 0.700 D only in pieces that a plunge enters (20 pieces).
- Star, departure: within R of arc after the piece leaves the wall the cut reads up to 0.622 D (69 samples over c in 41 pieces). The gap to the loop before is 1.25 mm (median). Past the departure, the interior reads at most 0.363 D (0 over).
- Pocket, C14 straight turns: the head runs on the wall for 2.0 mm (median) and reads up to 0.544 D in 67 pieces, at full feed. With the head as the turn (C14 bound 3.207 mm): 62 turns, 5 over the bound (each a plunge), feed factor median 0.72; the head then reads up to 0.496 D only in pieces that a plunge enters (9 pieces).
- Pocket, departure: within R of arc after the piece leaves the wall the cut reads up to 0.419 D (72 samples over c in 32 pieces). The gap to the loop before is 1.16 mm (median). Past the departure, the interior reads at most 0.439 D (5 over).

Two derived fixes (the second is not built):

- The head is the turn: the turn runs from the previous departure along the head to this departure, its feed is cut by c / max(w, a) (C14), and its bound is C14's. Built as the plan of the cards. The few turns over the bound are counted in the next section.
- The departure corner: extend the counted turn to the last sample over c within R after the departure. That adds 1.30 mm (star) and 1.30 mm (pocket) at the median; the longer turn stays within its bound in 2 of 39 (star) and 0 of 31 (pocket). The other way is a fillet. C15 (c)'s bound is for a corner where the cleared side is concave; at the departure the reading is over the gap, so the cleared side is convex and the C5 law gives a lower bound instead: ρ ≥ ρ(g) with s'(ρ(g)) = g. At the median gap that is 3.2 mm (star) and 2.8 mm (pocket), at most 4.0 and 4.6 mm. A fillet changes the march shape, so this note prefers fix 2 by the turn.


## The slanted bound, per family (for a ruling)

The lead ruled C16's slanted bound s / cos θ + (1 + √2)·cell unnecessary for (a). The counts below are for one-way with the head as the turn, the same order and links for all three families. "Need" counts the turns of the slanted-bound plan that are longer than the C14 bound 3.207 mm.

| Shape | Family | Turns, C14 bound | Over the C14 bound (plunges) | Of those, within s / cos θ + (1 + √2)·cell | Plunges, C14 bound | Turns, slanted bound | Turns longer than the C14 bound (need the slanted bound) | Plunges, slanted bound |
|---|---|---|---|---|---|---|---|---|
| Star | march | 93 | 2 | 2 | 10 | 95 | 2 | 8 |
| Star | C15 (e) harmonic | 79 | 0 | 0 | 11 | 79 | 0 | 11 |
| Star | C15 (a) | 47 | 0 | 0 | 17 | 47 | 0 | 17 |
| Pocket | march | 62 | 5 | 5 | 10 | 67 | 5 | 5 |
| Pocket | C15 (e) harmonic | 85 | 11 | 8 | 22 | 93 | 8 | 14 |
| Pocket | C15 (a) | 43 | 5 | 3 | 15 | 46 | 3 | 12 |

So the march needs the slanted bound for 2 turns (star) and 5 (pocket), the harmonic for 0 and 8, and (a) for 0 and 3. Without it each of those turns is a plunge. No family needs it for more than a few turns per shape.

## The harmonic (e), clipped and split

The operator: "I think the same thing could be done with harmonic too." It can. The clip, the split, the order and the links run on the (e) loops with no change. What the clip cannot fix is the uneven gap of (e): a level line of the harmonic field lies close to the one before where the field is steep, so most of each loop cuts a thin strip. The clip removes the air, not the thin cut.

- Star: loop 12 is the first loop with more than one cut piece; the stock splits at level 16; 118 pieces in 6 lobes. The pieces are 43% of the loop length. 47 runs are too small to be pieces and 29 gaps are merged (the march: 1 and 1): the thin cut breaks into many short runs.
- Star, time: 3.67 min for the full (e) and 2.16 min clipped (air at the cut feed); 1.10 min for (a) with the same links. Entries 35 = 1 + 11 plunges (11 core seams) + 23 refusals. Turns 79, of them 6 over c: a thin cut gives a light turn.
- Star, engagement (one-way, head as the turn): engaged length 2345 mm, against 1243 for the march and 893 for (a). Median swept width of the engaged samples 0.123 D (march 0.318, (a) 0.341); mean width per piece, median 0.128 and smallest 0.019 D (march 0.247 / 0.018, (a) 0.369 / 0.361).
- Star, load: head up to 0.769 D, departure up to 0.587 D, interior at most 0.349 D (0 over). Coverage left 57.4 mm² (56.1 unreachable); the full (e) leaves 57.6 mm².
- Pocket: loop 13 is the first loop with more than one cut piece; the stock splits at level 46; 156 pieces in 3 lobes. The pieces are 37% of the loop length. 77 runs are too small to be pieces and 69 gaps are merged (the march: 2 and 3): the thin cut breaks into many short runs.
- Pocket, time: 7.08 min for the full (e) and 4.15 min clipped (air at the cut feed); 1.68 min for (a) with the same links. Entries 23 = 1 + 22 plunges (10 core seams) + 0 refusals. Turns 85, of them 0 over c: a thin cut gives a light turn.
- Pocket, engagement (one-way, head as the turn): engaged length 3828 mm, against 1601 for the march and 1251 for (a). Median swept width of the engaged samples 0.062 D (march 0.306, (a) 0.313); mean width per piece, median 0.112 and smallest 0.016 D (march 0.288 / 0.021, (a) 0.331 / 0.098).
- Pocket, load: head up to 0.164 D, departure up to 0.049 D, interior at most 0.362 D (0 over). Coverage left 3.3 mm² (0.0 unreachable); the full (e) leaves 0.0 mm².

On the pocket the clipped (e) leaves stock that the full (e) cuts: the runs too small to be pieces (one sample, or no cell) hold it.
Which is faster depends on the cost of an entry. With the counts of this replay, clipped (e) is faster than (a) with the same links only when one entry costs more than 1.3 s (star) or 2.9 s (pocket); the clipped march only when it costs more than 2.2 s (star) or 3.4 s (pocket). The cost of an entry (lift, travel, re-entry at the plunge feed) is not measured here.

## Verdict

- The operator's reading is right. The full march cuts every loop whole, so it runs again along all the wall that earlier loops cut. Clipped, split per lobe and linked one-way, the same loops (to 1.4e-14 mm) take 1.50 min against 3.67 (star) and 2.25 min against 6.88 (pocket), air at the cut feed.
- It is still slower than (a) with the same links: 1.10 min (star) and 1.68 min (pocket). The march engages 1243 mm against 893 mm (star) and 1601 against 1251 mm (pocket). No link removes that: it is the march's thinner passes.
- Plunges: 10 (star) and 10 (pocket) against 17 and 15 for (a). The core rings give 8 and 5 of them (the seam step reads over c).
- Load: with its heads as turns the march keeps the C14 bound (the slanted bound would save 2 plunges on the star and 5 on the pocket). Its departure corners still read up to 0.622 D (star) and 0.419 D (pocket). In the same replay the refusals are 63 and 54 for the march, 67 and 59 for (a); the entries are 74 and 65 against 85 and 75. The Python replay over-reads the Rust one: for (a) + one-way, Rust has 19 and 29 join entries. So on entries the clipped march is not worse than (a); on time it is.
- Harmonic (e), clipped: the fewest entries (35 star, 23 pocket, against 85 and 75 for (a)) and the lightest load (pocket: 0 refusals). But it takes 2.16 and 4.15 min against 1.10 and 1.68: the clip cannot make a thin cut thick.
- Not a candidate for Rust as it stands. If the operator wants the march shape, the next step is fix 2 (the departure as part of the turn); it would still engage more length than (a).

## Correction to v5

v5 said that with the C14 bound 51 (star) and 37 (pocket) of the march's head turns are over the bound. That was a defect of the C19 builder, not of the march: after a plunge the one-way return went back to the piece start, not to where the piece leaves the wall, so every later turn was one head longer and over the bound too. The return now goes to the departure point in every case. With the fix: 2 (star) and 5 (pocket). v5 stays as it was; this page and the note replace it.

## Limits

- The entry counts of this replay are higher than Rust's for the same family, on both sides of each break-even; the break-even values compare like with like, not with D12.
- Two shapes, one tool, one stepover. The replay is C15's 0.1 mm raster, not the planner, and it reads the (a) ends harder than the Rust replay does.
- Times are length over feed (cut feed 1500 mm/min, link feed 500 mm/min, turns at their reduced feed), with no acceleration; plunges are counted, not timed.
- The core rings keep C17's seam join; a spiral morph of the rings is not tried.
- No mop: stock that the plan leaves is reported as coverage left.


## Lobes, star

| Lobe | Parent | Levels | Children | Pieces | Levels with 2+ pieces |
|---|---|---|---|---|---|
| 0 | — | 0–7 | 1, 2, 3, 4, 5 | 8 | 0 |
| 1 | 0 | 8–25 | — | 18 | 0 |
| 2 | 0 | 8–28 | — | 21 | 0 |
| 3 | 0 | 8–26 | — | 19 | 0 |
| 4 | 0 | 8–29 | — | 22 | 0 |
| 5 | 0 | 8–29 | — | 22 | 0 |

## Lobes, pocket

| Lobe | Parent | Levels | Children | Pieces | Levels with 2+ pieces |
|---|---|---|---|---|---|
| 0 | — | 0–30 | 1, 2 | 58 | 26 |
| 1 | 0 | 31–31 | — | 1 | 0 |
| 2 | 0 | 31–32 | 3, 4 | 4 | 2 |
| 3 | 2 | 33–47 | — | 29 | 14 |
| 4 | 2 | 33–35 | — | 6 | 3 |
