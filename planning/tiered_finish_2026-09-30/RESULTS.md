# Tiered finishing — results, items 1–4 (2026-09-30)

Scope: plan items 1–4 (Step 0, F1 dial, F3c advisory, F4 A/B). No default
changed. Working tree over master `469067f6`, debug build, cloud container.
Every measured number below comes from a test named in its section.

## Plan references, checked at `469067f6`

All hold, with these line offsets: `DEFAULT_OVERLAP_MM` is
`tier_islands.rs:116` (plan :111), `CLOSE_RADIUS_PER_CUSP_RADIUS` :138
(plan :136), the wanaka docstring :563 (plan :558). One claim is wrong:
the F1 sentry fixture of the plan (2 x 2-cell specks at 2.0 mm gaps) does
**not** weld within three raises. The exact Euclidean close keeps the
lattice centre open until the radius reaches its distance to the nearest
speck, √18 = 4.24 cells; the third raise stops at 0.5 · 1.5³ / 0.4 =
4.22 cells (`the_plans_two_cell_specks_at_two_mm_never_weld`). The sentry
uses 3 x 3-cell specks at 1.6 mm gaps, which weld.

## Step 0 — measurements

Ladder: R2.0 taper (tool 12, copied from the wanaka library) over the
project's R1.0 taper (tool 6). Tolerance 0.15, cell 0.4, overlap 2.0,
slope compensated, cap 24. The operator's 500 x 500 project is not in the
repo. Two proxies: `rivmap100_tiered_finish.toml` (100 x 100 mm) and the
x3.5 terrain of `rivmap100_memory_repro.toml` (350 x 350 mm).
Instruments: `rivmap100_tiered_finish_step0`,
`rivmap350_tiered_finish_step0_preview`
(`crates/rs_cam_core/tests/tier_band_overlap_g_overlapfill.rs`).

Areas in mm². Raw = cells labelled for the fine tier x cell area.

| board, arm | raw islands | raw | owned | machining | owned/raw | mach/owned | mach/raw |
|---|---:|---:|---:|---:|---:|---:|---:|
| 100 mm, raise bound 3 (default) | 781 | 1 709 | 1 742 | 7 770 | 1.02 | 4.46 | 4.55 |
| 100 mm, raise bound 0 | 781 | 1 709 | 1 742 | 7 770 | 1.02 | 4.46 | 4.55 |
| 100 mm, close off | 781 | 1 709 | 1 044 | 6 008 | 0.61 | 5.76 | 3.52 |
| 350 mm, raise bound 3 (default) | 14 594 | 19 659 | 47 901 | 90 152 | 2.44 | 1.88 | 4.59 |
| 350 mm, raise bound 0 | 14 594 | 19 659 | 17 538 | 57 241 | 0.89 | 3.26 | 2.91 |
| 350 mm, close off | 14 594 | 19 659 | 3 529 | 15 082 | 0.18 | 4.27 | 0.77 |

Cap report and holes:

| board, arm | after close | after min area | kept | raises | final radius | dropped | dropped area | holes owned → machining |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| 100 mm, bound 3 | 429 | 8 | 8 | 0 | 0.500 | 0 | 0 | 57 → 46 |
| 100 mm, bound 0 | 429 | 8 | 8 | 0 | 0.500 | 0 | 0 | 57 → 46 |
| 350 mm, bound 3 | 935 | 6 | 6 | 2 | 1.125 | 0 | 0 | 1 228 → 316 |
| 350 mm, bound 0 | 7 475 | 111 | 24 | 0 | 0.500 | 87 | 2 981 | 892 → 149 |
| 350 mm, close off | 14 594 | 177 | 24 | 0 | 0 | 153 | 5 293 | 211 → 19 |

The split of the territory growth (the operator's 2.73x mixes both):

- 100 mm board: the cap never acts (8 islands), so F1 is 1.02x and the
  band (F2) is all of it, 4.46x.
- 350 mm board: F1 (raw → owned, two raises to 1.125 mm) is **2.44x**,
  F2 (owned → machining) is **1.88x**; 4.59x in all. The machining area,
  90 152 mm², is the operator's "90k". With the raise bound at 0, F1 is
  0.89x and the band grows to 3.26x of the smaller owned set: F2 is the
  next lever once F1 is set.

F3 on the generated fine tier (100 mm board; stock source Fresh, because
the remaining-stock chain needs a simulation first; 50 322 moves, 49 794
cutting; generation 320 s):

- `check_collisions` (endpoints): 0 strikes (shank 0, holder 0).
  `ProjectSession::collision_check` (1 mm samples): 0.
- `check_collisions` never tests this tool's shank: it skips a segment no
  wider than `radius()`, and on this taper shank Ø = shaft Ø = 6 mm. The
  same rule with no skip (`body_segment_penetration_mm`, body r 3.0 mm at
  20.0 mm): 0 of 49 794 cutting endpoints strike.
- F3c advisory on the owned cells: 0 binding cells on both boards
  (10 863 and 298 771 cells checked), and 0 on the 350 mm board with a
  15 mm-flute copy of the fine tool (the operator's flute length).

## F1 — the raise-bound dial (default 3, unchanged)

`TierIslandParams::max_close_raises` (serde default 3 =
`MAX_CLOSE_RAISES`, clamped to it); `TierCapReport` now carries the bound
in force and `dropped_area_mm2`. Sentry `tests/tier_islands_speck_weld.rs`:
0.4 mm cells, 30 islands of 13 x 13 cells at 4.0 mm gaps, 14 x 14 specks of
3 x 3 cells at 1.6 mm gaps. The weld pass is measured by closing the field
through `extract_tier_islands`: pass 2 (0.5 · 1.5² = 1.125 mm), pinned.

- Red, bound 3: 31 islands after the min-area filter (30 + the blob),
  3 raises, 24 kept, 7 real islands dropped (189.3 mm²). The blob owns
  **8 628 cells = 1 380.5 mm²** of the field against 1 764 raw speck cells
  (282.2 mm²): 4.9x. Pinned.
- Green, bound 0: no raise, 24 kept, 6 dropped, **162.2 mm²** reported,
  no owned cell in the field, owned = 24 x 27.04 mm² within perimeter x
  cell, owned ⊆ close(raw, 0.5 mm).

## F3c — flute-reach advisory

`maps::tier_flute_reach`: per fine tier, the owned cells where the tier
tool's body above the flutes (`ToolAssembly::non_fluted_body`) strikes by
the rule `stock::collision::body_segment_penetration_mm`, which
`check_collisions` now calls too. `MultitoolPreview::flute_reach`, the GUI
planner panel (a warning line per binding tier) and the MCP
`preview_tier_map` reply (`flute_reach`) show it. Territory unchanged.
Sentry `tests/tier_map_flute_reach.rs`: straight canyon, Ø10 ball over the
R1.0 / 5.7° / Ø6 taper with L_c 15, 0.2 mm cells. At H = L_c + 2.8 each
wall binds a strip of r_body − `width_at_height(L_c)` = 3.0 − 2.40 =
0.60 mm (3 cells, ± 1); at H = L_c − 1 nothing binds. On both proxy boards
the binding area is 0 mm² (Step 0).

## F4 — arc-fit A/B

Instruments in `tests/tier_arcfit_burial_f4.rs`. Burial = the tier tool's
drop-cutter floor minus the path, sampled every 0.05 mm on every fed move.

Plan fixture, 40 x 40 mm, knolls h 0.3 / r 0.4 on the plane and a 60°
wall. The wall runs round a circular mesa: a straight wall gives straight
waterlines, and the fitter made only 8 arcs in 14 952 moves on it (all
three arms then read the same 0.0432 mm). R1.0 ball, the planner's tier op,
boundary off, stock Fresh.

| arm | moves | arcs | burial max | on arcs | on lines | sim gouge (−min dev) | dev p0.1 | dev p1 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| A arcs 0.05 | 14 147 | 450 | 0.0882 | 0.0882 | 0.0531 | 0.0820 | −0.0451 | −0.0215 |
| B arcs off | 15 078 | 0 | 0.0531 | — | 0.0531 | 0.0257 | −0.0159 | −0.0108 |
| C arcs 0.015 | 14 571 | 204 | 0.0531 | 0.0134 | 0.0531 | 0.0257 | −0.0159 | −0.0112 |

Simulation: 0.2 mm cells, 13 347 columns in the knoll/wall mask.

Verdict: the residual mechanism is **confirmed**. burial(A) − burial(B) =
0.0351 mm, above 0.005 and below 0.05 + sag (0.0318 at s 0.5) = 0.0818;
C − B = 0.0000, inside 0.015. The simulated gouge follows: 0.082 mm (A)
against 0.026 mm (B, C).

rivmap100 fine tier (R1.0 taper floor), same three arms, per sample:

| arm | arcs | arc samples > 0.05 | cut samples > 0.05 | all fed > 0.05 / samples | max on arcs | max on lines |
|---|---:|---:|---:|---:|---:|---:|
| A 0.05 | 15 520 | 4 149 | 4 890 | 11 396 / 495 079 (2.30 %) | 0.266 | 0.836 |
| B off | 0 | 0 | 5 961 | 8 318 / 541 519 (1.54 %) | — | 0.836 |
| C 0.015 | 9 712 | 402 | 5 531 | 8 290 / 520 987 (1.59 %) | 0.159 | 0.836 |

Arcs at 0.05 add about 3 100 samples buried deeper than 0.05 mm; at
0.015 they add none. A second finding, larger than the arcs: with arcs off
the fine tier's own `FinishingCut` lines sit up to **0.836 mm** below the
taper's drop surface (1 595 samples deeper than 0.1 mm), and other fed
moves up to 0.356 mm. The source is not measured yet.

The sentry `an_arc_fit_buries_no_deeper_than_the_source_path_plus_its_tolerance`
(burial(on at t) ≤ burial(off) + t + 1e-6, t ∈ {0.05, 0.015}) holds, so it
is in the gate (57 s, debug). It runs on the mesa alone (22 x 22 mm, no
knolls): arcs off 0.0513 mm, arcs 0.05 → 0.0882 mm (450 arcs), arcs 0.015 →
0.0513 mm. On the 40 mm fixture it reads 0.0882 ≤ 0.1031 and
0.0531 ≤ 0.0681. The bound holds while the defect is present, so the bound
alone does not name it.

## Changes that wait for approval, with the measured effect

1. F1: `TierIslandParams::max_close_raises` default 3 → 0. 350 mm proxy:
   owned 47 901 → 17 538 mm² (−63 %), machining 90 152 → 57 241 mm²
   (−37 %); 87 islands (2 981 mm²) go back to the coarse tier. 100 mm
   proxy: no change (the cap does not act).
2. F4a: tier-op arc tolerance 0.05 → cusp/2 = 0.015 (in
   `plan_tier_operation`'s dressups only). Mesa: burial 0.0882 → 0.0531 mm,
   simulated gouge 0.082 → 0.026 mm. rivmap100: fed samples deeper than
   0.05 mm 2.30 % → 1.59 %.
3. F3b (cells reached only by the body leave the fine tier): no measured
   effect on either proxy (0 binding cells, also at L_c 15). Low priority on
   this terrain.
4. F2 (band): not decided here. With F1 at 0 the band ratio is 3.26x on the
   350 mm proxy.
