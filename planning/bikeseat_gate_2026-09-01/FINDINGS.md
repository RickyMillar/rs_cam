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
