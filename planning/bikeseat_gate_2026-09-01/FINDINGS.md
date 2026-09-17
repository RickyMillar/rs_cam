# Track D — bike-seat gate stage: findings

**Instrument:** `crates/rs_cam_core/tests/bikeseat_gate_d1.rs`
(`cargo test --release -p rs_cam_core --test bikeseat_gate_d1 -- --ignored --nocapture`).
**Shared estimator:** `crates/rs_cam_core/tests/common/monge.rs`, extracted
from `wanaka_curvature_anisotropy.rs` + `zone_coherence_census.rs`. The
non-ignored self-check `monge_extraction_recovers_known_curvature_and_axis`
pins the extraction against closed-form surfaces.

## §1. Pre-registration — WRITTEN BEFORE THE RUN

### The fixture

An analytic swept-cosine seat sheet, 160 × 90 mm, closed form (the test's
module doc states each term and its purpose):

- `spine` (convex along the sweep) + `dish` (concave across) give the
  tens-of-mm relief and a clean crest/trough regime split.
- `wave`: cosine corrugation, wavelength 14 mm (bands ≈ 14 stepovers wide),
  cross-curvature 0.20–0.42 /mm varied along the sweep by `A(u)`.
- `fan`: the corrugation's flow lines bend through ±35° across the sheet.
  Without it, one fixed raster angle rides every trough and the ceiling —
  measured AGAINST the best fixed direction — collapses to ~0. A real
  seat's flow lines curve; this is the analytic version.
- `nose`: a convex dome (κ ≈ +0.12 both ways) — pulls `min(s_max)` below
  the flat anchor and makes the sheet a seat.

Fixture requirements, ASSERTED in the instrument:

| Requirement | Bar |
|---|---|
| Facets (max 3-D edge, diagonals included) | ≤ stepover/3 = 0.16207 mm |
| Up-facing heightfield | min face normal.z > 0 |
| Relief | ≥ 15 mm |
| `mean(s_max)/min(s_max)` at R = 1.0 | ≥ 1.10 (printed; sphere fixture reads 1.00 by construction) |
| Simply connected | a rectangular heightfield grid, by construction |

Scale note, registered up front: at R = 1.0 mm, `1/R = 1.0 /mm` anchors
every strip width. On a wood-scale smooth sheet the s_max spread is
physically bounded — `min(s_max)` sits at the flat/convex anchor whenever
any near-developable crest exists. The 1.10 bar is against the sphere's
1.00, not against Wanaka's terrain.

### GATE 1 (anisotropy prize)

Decision cell: fit radius 1.0 mm, ball R = 1.0 mm, scallop 0.03 mm — the
same cell Wanaka's census read.

- **Measure:** median `W_max/W_min` (area-weighted) and the prize ceiling
  `= min over fixed directions (x, y, PCA) of ∫dA/W_fixed ÷ ∫dA/W_max`,
  in percent. Same arithmetic as `wanaka_curvature_anisotropy.rs`.
- **PASS bar: ceiling ≥ 5.0 %.** Justification: Kumazawa's honest prize
  against an iso-scallop is 1.9–7.2 %
  (`preferred_direction_field_research.md` §8); Wanaka region 1 measured a
  9.75 % ceiling (`finishing_synthesis_2026-08-30.md` §11) and the method
  still lost there. A home-turf fixture offering under 5 % — below the
  middle of the literature band and half of Wanaka's — cannot justify the
  arm anywhere.
- Both anti-faceting scale rules (excess rule, decay exponent) must read
  clean, or the ceiling is not quotable. On an analytic mesh whose
  vertices lie exactly on the closed form they should read landscape.

### GATE 2 (coherence)

Stepover pinned at 0.4862 mm (R = 1.0, h = 0.03 — the coherence census's
own pin). Field unit: 0.25 mm lattice cells, Monge fit at radius 1.0 mm,
trusted axis per the census's isotropy floor.

- **G2-a: whole-sheet median coherence length ≥ 3 stepovers = 1.459 mm.**
  Coherence length = distance to the nearest trusted cell whose `t1`
  differs by > 30°, right-censored at 20 stepovers, exactly the census's
  definition. Wanaka read **0.35 mm** against the same stepover.
- **G2-b: both 45° orientation regimes (trusted `t1` within 45° of x,
  resp. of y — the same 45° segmentation angle §F1-2 ran on Wanaka) read
  `w30 ≥ 0.70`, and together cover ≥ 70 % of sheet area.** Wanaka's
  product zones read `w30 ≤ 0.43` everywhere.
- The whole-sheet `w30` is printed but is NOT a gate: crest bands prefer
  the across-feed and trough bands the along-feed (that is Euler's
  theorem, not a defect), so a two-regime surface fails a single-dominant
  w30 by construction. The method under test segments by direction before
  it paths — Kumazawa's own step. Registered so a reviewer cannot read
  the whole-sheet row as a hidden failure or a hidden pass.
- Size control: 16 / 8 / 4 mm tiles, printed. Prediction below.

### Negative control

A 60 × 60 mm patch summing six fixed cosine plane waves (wavelengths
2.6–3.7 mm, scattered angles, deterministic table in the test). Its
direction field turns at sub-stepover scale by construction.

- **Required for meaning: the control FAILS gate 2** (coherence length
  under the bar; whole-patch w30 low). Gate 1 may PASS on it — anisotropy
  without coherence is exactly Wanaka's signature, and reproducing it
  shows the two gates separate.
- Secondary reference (cited, not rerun): Wanaka's own measured
  thresholds — ceiling 9.75 %, w30 ≤ 0.43, coherence 0.35 mm.

### Predictions (refutable)

| Quantity | Prediction |
|---|---|
| Sheet median `W_max/W_min` | 1.10–1.16 |
| Sheet prize ceiling | 5–8 % (the fan is what keeps it over the bar; without it ~3.5 %) |
| Sheet whole median coherence length | ≈ 1.7 mm ≈ 3.5 stepovers (≈ λ/8) |
| Sheet regime w30 | 0.80–0.95 each; coverage > 0.90 |
| Sheet whole w30 | ≈ 0.5 (two-regime construction) |
| Tiles | 4 mm tiles mostly usable; 8 mm marginal (a tile can straddle a band edge); 16 mm fail |
| Control coherence length | < 1 mm (< 2 stepovers) |
| Control whole w30 | < 0.5 |
| Control ceiling | > 8 % (no fixed direction wins on isotropic noise) |

### Decision rule

- **GATES PASS** (G1 and G2 both) → the full F1 pipeline + ×floor + honest
  raster comparison on this fixture is justified (paper target: −13.0 %
  path on the bike seat, Table 1 of `paper_2009.02660_extraction.md` §9).
- **Any gate FAILS** → record why the fixture class cannot express the
  method's advantage, and whether a different construction could. Track D
  stops either way — no pathing, no field solve, no ×floor runs.

## §2. Results

*(to be appended after the run — nothing below this line was written
before it)*

Run: 2026-09-01, commit `aefbd72c` (pre-registration) + this commit.
`cargo test --release -p rs_cam_core --test bikeseat_gate_d1 -- --ignored --nocapture`,
4.2 s wall. Exit 0; every fixture assert held.

### Fixture census (all bars held)

| Quantity | Sheet | Control | Bar |
|---|---|---|---|
| Mesh | 6,819,740 tris / 3,413,718 verts | 720,000 / 361,201 | — |
| Surface area | 16,752 mm² | 3,616 mm² | — |
| Relief | 24.01 mm | 0.30 mm | ≥ 15 (sheet only) |
| Max 3-D edge | 0.1325 mm (0.818× bar) | 0.1443 mm | ≤ 0.16207 |
| Min normal.z | 0.6270 | 0.9757 | > 0 |
| mean(s_max)/min(s_max) | **1.111** | 1.135 | ≥ 1.10 |

s_max on the sheet: mean 0.5149, min 0.4635, p90 0.5741, max 0.6441 mm.
The curvature genuinely varies; the spread bar held, barely (1.111 vs
1.10). See §3 item 4 for why this quantity is physically capped at this
scale.

### GATE 1 — anisotropy prize: **FAIL**

Decision cell r = 1.0 mm, R = 1.0 mm, h = 0.03 mm; 19,673 samples, zero
under-determined, zero gouge:

| Quantity | Sheet | Prediction | Wanaka ref |
|---|---|---|---|
| median W_max/W_min | **1.0756** | 1.10–1.16 (missed low) | 1.0950 |
| bound_x / bound_y / bound_pca | 1.0385 / 1.0481 / 1.0385 | — | — |
| **PRIZE CEILING** | **+3.85 %** | 5–8 % (**refuted**) | +9.75 % |
| excess rule | clean (0.0758 vs coarse 0.0755) | clean | — |
| decay rule | p = 0.009 → landscape | landscape | p flagged there |

3.85 % < the 5.0 % bar. The scale rules are clean, so the number is
quotable: the fixture's fine-scale reading IS the landscape (analytic
vertices; ratio_p50 is flat 1.076 → 1.071 across r = 0.3 → 4.0 mm).

### GATE 2 — coherence: **PASS**

Field: 200,361 cells at 0.25 mm, 100 % fitted, 39 degenerate.

| Zone | area mm² | w30 | dom. dir | coherence length | Verdict |
|---|---|---|---|---|---|
| whole (reference) | 14,527 | 0.468 | 0.6° | **1.677 mm = 3.45 stepovers** | not-usable (registered: not a gate) |
| regime ALONG (t1~x) | 7,707 | **0.887** | 179.7° | 2.531 mm = 5.21 st | USABLE |
| regime ACROSS (t1~y) | 6,817 | **0.811** | 89.5° | 3.260 mm = 6.70 st (11 % censored) | USABLE |

Regime coverage 1.000 (bar 0.70). G2-a: 3.45 ≥ 3 stepovers → pass.
G2-b: 0.887 / 0.811 ≥ 0.70, coverage 1.000 → pass. Both against
Wanaka's 0.35 mm and w30 ≤ 0.43: **this fixture class has exactly the
coherence Wanaka lacks.** Tiles (size control): 16 mm 0.017 of area in
w30-passing tiles (a 16 mm tile straddles band edges), 8 mm 0.506,
4 mm 0.748 — coherence lives at the 7 mm band scale, as constructed.

### Negative control — behaves as required

- **Gate 2 FAILS**: whole-patch coherence length 0.500 mm = 1.03
  stepovers, whole w30 0.320, zero usable tiles at any size. The two
  gates separate; the sheet's gate-2 pass is meaningful.
- Gate 1 on the control: ceiling +3.36 %, median ratio 1.0644 at r = 1.0
  — and the excess rule **FIRES** (fine excess 0.0835 vs coarse 0.0267):
  the control's curvature lives below the fit scale, so its gate-1
  number is not quotable, by the instrument's own rule. Registered
  prediction said the control ceiling would exceed 8 %; measured 3.36 %.
  Refuted — see §3 item 3.
- Prediction check, control: coherence < 1 mm ✓ (0.500), w30 < 0.5 ✓
  (0.320).

## §3. Verdict and mechanism: GATES FAIL

**GATE 1 FAILS (+3.85 % vs 5 %). GATE 2 PASSES. Per the pre-registered
decision rule, Track D stops here.** The fixture class is coherent but
the prize is under the bar: a raster at the right angle already captures
what the direction method would win. Why, in four parts:

1. **The two gates are in tension, and the tension is structural.** The
   prize ceiling is measured against the best FIXED direction. Gate 2
   demands a coherent field; a coherent field is a predictable field; a
   predictable field is capturable by a fixed-angle raster. On the sheet
   the per-point median excess is 7.56 % but the ceiling is 3.85 % —
   half the pointwise prize is already captured by one global angle. On
   Wanaka the ceiling (9.75 %) nearly equals the pointwise excess
   because its field is incoherent — and that same incoherence is what
   killed the method there. The method needs a field coherent enough to
   path but too curved for any raster angle; the window between those
   two conditions is what this fixture measured, and it is worth less
   than 4 %.
2. **The per-zone rotated raster the product already ships eats the
   residual.** The 3.85 % is against ONE global direction. C2's
   per-region PCA sweep rotation (shipped, default-off, C4-pending)
   already picks a per-zone angle. Whatever a per-zone fixed angle
   captures comes out of the field method's 3.85 %; the field's true
   margin over shipped machinery is strictly smaller. Not measured
   here; it does not need to be — 3.85 % is the upper bound.
3. **The prize scales with κ·R, and the bike-seat class does not have
   κ·R.** Every strip width is anchored by 1/R = 1.0 /mm. The sheet's
   corrugation already carries κ = 0.20–0.42 /mm (radii 2.4–5 mm) —
   MORE curved than a real carved seat (radii 30–150 mm, κ·R ≈
   0.01–0.03, prize ≈ 0.1 %). Even so: ceiling 3.85 %. The control's
   refuted >8 % prediction fell to the same physics: at fit scale its
   κ dropped to 0.067 and the convex/concave asymmetry (a concave gain
   is capturable; a convex "gain" is only penalty avoidance anchored at
   the flat width) keeps every ceiling second-order in κ·R.
4. **The same anchor caps the adaptive-spacing spread.** min(s_max)
   sits at the flat/convex anchor wherever any near-developable crest
   exists, so mean/min reached only 1.111 on a sheet built to maximise
   it. The 0.4635–0.6441 mm s_max range is real but narrow.

### Could a different construction pass?

Only by leaving the class. The two levers are:

- **More flow-line curvature (a stronger fan).** The w30 ≥ 0.70 regime
  bar tolerates roughly ±35° of within-zone turning; this fixture
  already uses ±35°. A ±60° fan raises the ceiling and fails gate 2's
  own w30 — the gates close over the gap.
- **More κ·R.** Pushing κ toward 0.6–0.9 /mm (radii 1.1–1.7 mm against
  a 1.0 mm ball) approaches the gouge bound and is carving detail, not
  a swept sheet — and the operator answer to detail that tight is a
  smaller ball, which resets κ·R and the prize with it. Tool selection
  chases the prize away.

Zou's Table 1 (−13.0 % on the bike seat) is not contradicted so much as
un-transferable: the paper publishes no model dimensions, tool radius,
or scallop relative to curvature, so its κ·R is unverifiable; at THIS
programme's operating point (R = 1.0 mm, h = 0.03 mm, 100–250 mm wood
parts) the measured ceiling on a more-curved-than-real seat sheet is
3.85 %, and part of the paper's margin is the global-optimization and
spacing machinery, not direction choice alone.

### What survives

- The fixture + both censuses now run analytically in ~4 s with zero
  external mesh — a permanent, cheap gate for ANY future
  direction-adjacent proposal (`bikeseat_gate_d1.rs`).
- `tests/common/monge.rs`: the Monge estimator as a shared,
  self-checked extraction.
- The gate-2 result is genuinely new knowledge: swept-sheet work HAS
  coherent direction zones (regime w30 0.89/0.81, coverage 1.0). That
  is an argument FOR the shipped per-region rotated raster (C2) on this
  class of work — and against building field machinery to beat it.

**Track D: CLOSED at the gate stage. No pathing, no field solve, no
×floor runs.**
