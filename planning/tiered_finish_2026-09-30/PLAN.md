# Tiered finishing on large terrains — fix plan (2026-09-30)

Status: plan, by reading code at 89503c38 (nothing run; "confirmed" = by
code). Operator symptom (2026-09-28, 500 x 500 board, tolerance 0.15): many
links, bumps/gouges in walls that should be smooth. Operator direction
2026-09-30: implement; a default change lands as its own commit, listed for
approval.

## Step 0 — measure first (no code or default change)

- F1/F2 split: the operator's 90k / 33k = 2.73x mixes raw→owned (the close,
  F1) and owned→machining (the band, F2); `BAND_RATIO_ADVISORY_BOUND` 1.5
  covers only the second. Extend the ignored instrument
  `tests/tier_band_overlap_g_overlapfill.rs:327` to print per tier: raw area
  (`map.tier_area_mm2(k)`), `owned_area_mm2`, `machining_area_mm2`, the cap
  report (raises, dropped) and hole counts. Run on the operator's project if
  supplied, else `planning/fixtures/rivmap100/` at tolerance 0.15.
- F3: run `stock::collision::check_collisions` (`stock/collision.rs:275`,
  models the shank above `cutting_length`) on the fine-tier op; count
  strikes.
- F4: the arc A/B below.

## F1 — the 24-island cap welds specks into blobs (CONFIRMED)

`maps/tier_islands.rs:856-905`: over `max_regions_per_tier` (24, :123) the
raw mask is re-closed at r0·1.5^i (`CAP_CLOSE_RAISE_FACTOR` :127) up to
`MAX_CLOSE_RAISES` = 3 (:134), then truncated to the largest (:903). r0 =
cusp radius x 0.5 (:136): R1.0 gives 0.5 x 1.5³ = 1.6875 mm (the observed
1.69), bridging gaps to ~3.4 mm. Specks under the minimum area ((2·1.0)²·4 =
16 mm², :331) are dropped at pass 0 and come back only welded into a blob;
"keep the largest" then prefers the blob (wanaka at tolerance 0.146: 2 261 →
17 812 mm² owned, docstring :558).

Fix 1a (chosen): new `TierIslandParams::max_close_raises`, serde default 3
(no change on landing). 0 = keep the 24 largest at r0, drop the rest to the
coarse tier; report dropped count AND area. Rejected: 1b (a ratio bound with
no derivation), 1c (raise the cap: more links, still welds).
**Default change (approval):** 3 → 0. Derivation: a raise only merges, never
adds a raw cell; with 0 raises owned ⊆ close(raw, r0).
Sentry `tier_islands_speck_weld`: synthetic TierMap, 0.4 mm cells, cusp radii
[coarse, 1.0]; 30 real islands 13x13 cells (27.0 mm²) with 4.0 mm gaps; a 40
x 40 mm field of 2x2-cell specks (0.64 mm²) at 2.0 mm gaps (bridged at pass 2
by the disc rule — derive the pass from `morphological_close` and the public
constants, not hand figures). Red at dial 3: a ~1 600 mm² blob, owned ≥ 10x
the field's raw 125 mm². Green at dial 0: 24 kept, 6 dropped (162 mm²
reported), no owned cell in the field, owned ≈ 24 x 27 mm² within perimeter x
cell. Re-pin `tier_islands_i1.rs:344` to set the dial explicitly.

## F2 — the overlap band closes the coarse tool's slivers (MECHANISM CONFIRMED; share of 2.73x UNMEASURED)

Each island is dilated by `overlap_mm` (2.0, `tier_islands.rs:111`) at
:959-969 (whole-grid distance transform, `geometry/region_mask.rs:191-195`);
every hole narrower than 4.0 mm closes. The 1.5 bound is reporting only
(:146-162). 2.0 was borrowed from unified_finish, never derived. The band's
job: let a centre-contained fine tool reach every owned cell; contact ≤
R·sinθ ≤ R_cusp from the centre, + one cell: b_min = R_cusp + cell_mm (1.4 mm
at R1.0 / 0.4). Wanaka: 155 owned holes → 255 machining holes. Caution:
closing a hole removes the links around it; holding the band may ADD links.

Options: 2a hole-aware band (outer seam full band; holes under A_absorb
absorbed; inside larger holes the band stops at b_min); 2b bisect overlap to
meet 1.5x (wrong measure after F1: use machining/raw); 2c default overlap =
b_min. Decide after F1 lands and is re-measured, on cycle time and link count
from `metrology/costing.rs` (relink + kinematic time), not area. Expectation 2a.
**Default change (approval):** 2a: `hole_band_mm` = R_cusp + cell_mm, A_absorb
= the tier's `min_region_area_mm2` (16 mm² at R1.0); or 2c:
`DEFAULT_OVERLAP_MM` 2.0 → R_cusp + cell_mm. Update maps/CLAUDE.md "Do not".
Sentry (extend g_overlapfill): 40 x 40 mm fine square, 0.4 mm cells, coarse
slivers 15 mm long, widths {0.8, 2.0, 3.2, 4.8, 6.4} mm, 4 mm walls; bound
machining by owned + P_outer·b + πb² + Σ absorbed + Σ 2·b_min·L (kept) +
P·cell; assert which holes survive (from b_min and widths) and the counts.

## F3 — the tier map ignores flute length (CONFIRMED)

`maps/tier_map.rs:612-633` (`drop_against_candidates`) and `tier_for` :684
label by drop Z only; `tool/tapered_ball.rs:217-285` models ball + cone as
all-cutting; `cutting_length` (:31) is read only by `length()`; nothing in
maps/ reads flute, shank or holder. Walls taller than L_c: the taper's drop is
held by the wall's top edge above L_c — the non-fluted cone/shank rubs,
deflecting the tool (a plausible wall-bump source). The holder/shank check
(`stock/collision.rs`) runs only after generation.

Options: 3a in the walk (per tool, a flat cylinder of the non-fluted body
radius raised by L_c, the rule `check_collisions` uses, reused; new
UNREACHABLE label; the cache key `maps/tool_shape_key.rs` must then carry
shank/holder/stickout); 3b after the map, on fine-tier owned cells only
(cheaper); 3c advisory: report the binding area per tier.
Land 3c first; 3b after approval; holder strikes stay post-generation.
**Behaviour change (approval):** cells reachable only with the non-fluted
body leave the fine tier and are reported as unfinished (no new number).
Sentry `tier_map_flute_reach`: canyon, flat floor, vertical walls H = L_c +
2.8 mm, width > 2 x the coarse ball radius; rivmap100 taper (R1.0, 5.7°,
shaft Ø6) with L_c 15; failing cells = a strip of width r_body − r_contact
(± 1 cell) per wall, from `width_at_height`; 0 cells at H = L_c − 1.

## F4 — arc-fit gouges (knoll mechanism already FIXED; residual CONFIRMED by code)

`dressup/arcfit.rs:553-606`: the per-point Z check is unconditional
(G-RAMPTERRAIN S3), so a flat arc through a knoll is rejected. Residual: XY
(:456-489) and Z each allow ±tolerance, default 0.05
(`compute/config.rs:830-833`); the Finish role turns arcs on
(`config.rs:1257-1264`), tier ops inherit it (`session/multitool.rs:473`):
2 x 0.05 = 0.1 mm peak-to-peak against a planned cusp of 0.03 mm
(`multitool.rs:109`), straight into steep walls on waterline/scallop
contours. Competing: source polyline sag R − √(R² − (s/2)²) = 0.032 mm at R 1,
s 0.5 (present with arcs off).

Options: 4a tier ops only, tolerance h/2 = 0.015 (2·tol ≤ cusp h); 4b arcs off
on tier ops; 4c no change if the A/B shows noise.
**Default change (approval):** tier-op arc tolerance 0.05 → cusp_height/2,
confined to `plan_tier_operation`'s dressups (not the global Finish role).
Measurement: 40 x 40 mm plane + 60° wall with spherical-cap knolls h 0.3 (10
x cusp), base radius 0.4; R1.0 (CL bump width 2√(2Rh − h²) = 1.43 mm). Arms A
arcs 0.05, B off, C 0.015; identical leads. Primary:
`dressup::entry_audit::buried_fed_chords` (`entry_audit.rs:59`), all feeds,
0.05 mm samples, floor = drop-cutter, burial split arc-fit vs linear. Secondary:
sim at 0.2 mm cells (≤ min(1.43/4, stepover/2)), `column_deviations`
(`compute/simulate.rs:480`), gouge = −min(dev), p0.1/p1 in a knoll/wall mask.
Refuted if burial(A) − burial(B) ≤ 0.005; confirmed if > 0.005 and ≤ 0.05 +
sag and C within 0.015. Repeat A/B on the operator's fine-tier op.
Sentry: for every t, burial(arcs on) ≤ burial(off) + t + 1e-6; keep
`entry_moves_stock_aware_g_rampterrain` green.

## Order

Without default change: 1 Step 0; 2 F1 dial (default 3) + sentry; 3 F3c +
sentry; 4 F4 A/B. After approval: 5 F1 flip; 6 F3b; 7 F4a; 8 re-measure,
decide F2. After each: tier_islands_i1, tier_band_overlap_g_overlapfill,
tier_map_walk_t1 / _slope_t2 / _cache_t3 (test-support),
planned_tier_regions_boundary_o2, the arcfit tests.

## Operator questions

1. Can the 500 x 500 project (or its ladder: tool ids, coarse tool, cell,
   overlap) come into the repo, or is rivmap100 at tolerance 0.15 a proxy?
2. Close-raise limit 3 → 0 (drop the smallest islands instead of welding)?
3. Keep the per-tier cap at 24, or a new value?
4. F3: cells the fine tool reaches only with its non-fluted body leave the fine
   tier, reported as unfinished?
5. The 15 mm tapered ball above 15 mm: continuing cone or Ø6 cylinder? Shank
   length and stickout?
6. Tier-op arc tolerance 0.05 → cusp/2 = 0.015 (or arcs off)?
7. If F2 is still needed after F1: choose hole-aware band vs overlap =
   R_cusp + cell on measured cycle time?
