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
moves up to 0.356 mm. The source is measured in "Fine-tier burial"
below.

The sentry `an_arc_fit_buries_no_deeper_than_the_source_path_plus_its_tolerance`
(burial(on at t) ≤ burial(off) + t + 1e-6, t ∈ {0.05, 0.015}) holds, so it
is in the gate (57 s, debug). It runs on the mesa alone (22 x 22 mm, no
knolls): arcs off 0.0513 mm, arcs 0.05 → 0.0882 mm (450 arcs), arcs 0.015 →
0.0513 mm. On the 40 mm fixture it reads 0.0882 ≤ 0.1031 and
0.0531 ≤ 0.0681. The bound holds while the defect is present, so the bound
alone does not name it.

## Fine-tier burial (G-TIERBURIAL) — source found, one cause fixed

Instrument `tests/tier_fine_burial_sources_g_tierburial.rs` (release-fast,
about 3 minutes per arm). rivmap100 fine tier, stock Fresh, arcs off. Every
fed linear move is sampled every 0.05 mm against the tier tool's
drop-cutter floor. A penetration estimate per sample is min(vertical depth,
the shortest horizontal shift that clears the floor): at a near-vertical
step of the drop-cutter surface the vertical depth overstates the gouge.
Simulation: the tier alone, 0.1 mm cells, `column_deviations`.

**The audit floor is correct.** It is built from the op's own tool (id 6,
R1.0 / 5.7° / Ø6 taper). At the 40 deepest samples the taper floor equals
a plain R1.0 ball floor: the ball tip is the contact. Not a false alarm:
the simulation cuts 0.679 mm (48.3, 24.6) and 0.662 mm (65.8, 7.3) below
the model at the two deepest sites.

**Stage.** The burial is the same with the relink off
(`intra_region_hookup_mm` 0) and with the dressups off (rapid order, feed
optimisation): 711 long-chord samples, max 0.836, in each arm. The
generator emits them. The deepest sit in the MidSteep band (the scallop
rings); after the fix no MidSteep long chord is deeper than 0.1 mm (the
500 left are Shallow 445, VerySteep 55).

**Mechanism.** The MidSteep band runs the scallop with `continuous: true`
(`finish/unified_finish.rs:1969`). The continuous branch joins ring i to
ring i + 1 with a "helical" connector: one straight 3D feed from the tool's
position to the next ring's nearest kept point, allowed up to 3 x cusp_r =
3.0 mm (`finish/scallop/research.rs:552` at `c85c12e7`). Ring chords are
refined against the drop-cutter surface (`refine_ring_chords`); the
connector was not. The two deepest are 2.560 mm and 2.143 mm long. Where a
ring set splits into loops, the connector can also jump from one loop to
the next across the ground between them (the sentry fixture below); which
case each rivmap100 connector is was not measured.

**Fix** (no default change). `ring_generation::refine_connector` refines
the connector with the ring chord's own `refine_chord` and
`RingLiftCtx` (`RingLiftCtx::new`, same probe step and tolerance as the
rings). `refine_chord` now reports whether every piece met the tolerance.
A connector that does not, that crosses a coverage gap, or whose inserted
points fail the keep predicate, retracts instead
(`finish/scallop/research.rs:564-590`).

| rivmap100 fine tier, arcs off | before | after |
|---|---:|---:|
| fed samples > 0.1 mm deep | 2 078 | 1 866 |
| max vertical depth, all fed | 0.836 | 0.516 |
| cut chords > 0.25 mm: samples > 0.1 / max / max estimate | 711 / 0.836 / 0.791 | 500 / 0.325 / 0.162 |
| cut chords ≤ 0.25 mm: samples > 0.1 / max / max estimate | 884 / 0.516 / 0.060 | 886 / 0.516 / 0.060 |
| other fed (links): samples > 0.1 / max / max estimate | 483 / 0.356 / 0.147 | 480 / 0.356 / 0.147 |
| sim, min dev within 0.5 mm of (48.3, 24.6) / (65.75, 7.3) | −0.679 / −0.662 | −0.028 / −0.193 |
| sim, min dev near long-chord samples | −0.679 | −0.334 |
| sim, columns below −0.1 mm (of 847 k) | 24 016 | 23 726 |
| moves | 77 375 | 77 459 |

Sentry `tests/a_scallop_ring_connector_rides_the_surface_g_tierburial.rs`:
a dumbbell region (two 8 mm lobes, 1.0 x 0.6 mm neck) over a Gaussian ridge
(1.5 mm, half-width 0.8) splits the rings across the ridge. Straight
connector 2.526 mm long, 0.821 mm under the surface (non-vacuity). With
the straight connector injected back the sentry is red (move 219, 0.8210 mm
against the tolerance 0.05); with the fix the deepest fed sample is
0.0320 mm.

**Open at `5b34e710`** (the next section closes 2–4 and attributes 1
and 5):

1. Short cut chords (≤ 0.25 mm, 886 samples, vertical to 0.516 mm): the
   penetration estimate is at most 0.060 mm (84 samples over 0.03). They
   cross near-vertical steps of the drop-cutter surface; the vertical depth
   is mostly the metric, not the gouge.
2. Shallow-band raster rows (445 samples, max 0.325, estimate to 0.162):
   `raster_toolpath_from_grid` feeds straight between lattice points and
   across row turnarounds, with no chord check. VerySteep waterline chords:
   55 samples, max 0.158.
3. Relink surface links (479 `Linking` samples, max 0.356, estimate to
   0.147): `build_surface_link` samples at `sampling` (0.5 mm) and feeds
   straight between samples. It is the shared kernel; a change moves every
   linking arm.
4. With `intra_region_hookup_mm` 0 the op emits a 29.36 mm `Linking`
   feed 3.453 mm under the surface, right after an `EntryPlunge`. With
   rapid order and feed optimisation off it is gone (other fed max
   0.142). Feed optimisation moves no XY, so the rapid-order pass is the
   suspect; not isolated further. Not the fixture's setting (25 mm), but
   an operator dial.
5. The simulation still reads 23 726 columns below −0.1 mm; 1 194 have no
   buried sample within about 0.5–1 mm. The deepest (−0.612 mm) run along
   the board edge at y = 0.0–0.1. Normal-direction overcut (vertical dev x
   cos slope, after the fix): 14 086 columns over 0.1 mm, max 0.364 mm on
   that edge row. Not attributed yet.

## Fine-tier burial, part 2 (G-TIERBURIAL) — the open sources

Same instrument (`tier_fine_burial_sources_g_tierburial`, arms `sim` and
`nohookup`; it now also prints the fine tier's cycle time,
`compute_cycle_time` with the machine's kinematics and rapid = max feed,
and attributes the deepest simulation columns to a move). Working tree over
`5b34e710`, release-fast.

**Shared refinement.** The scallop's chord refinement moved to
`surface/chord_refine.rs` (`ChordRefineCtx`, `refine_chord`, the constants,
unchanged for the rings). Two options were added for the new callers:
`ChordSide::Below` (split a chord only where it is below the surface; a
chord above it over a concave stretch leaves a cusp, not a gouge) and
`ChordInsert::Push` (a waterline). The probe step of a gouge check is
`probe_step_for_tool`: `sqrt(8 r t)`, the chord whose sagitta over the tip
sphere (`r` = `cusp_radius_mm`) is the accept threshold `t` = 0.7 x
tolerance; for the R1.0 tip at 0.05 it is 0.529 mm. A chord is still probed
at eight intervals or more.

| source | real? | mechanism | fix |
|---|---|---|---|
| 1 raster rows | yes | `raster_toolpath_from_grid` (`toolpath.rs:715`) feeds straight between lattice points and across row turnarounds | `unified_finish::refine_band_chords` refines every fed `FinishingCut` chord of the Shallow band (`refine_fed_cut_chords`, over-mesh coverage) |
| 2 waterline chords | yes | a contour feeds straight between fiber crossings at one Z; a dropped split point would climb the wall | the VerySteep band uses `ChordRefineCtx::level_check`: the split point is pushed off the wall at the contour's Z (`push_to_level`, both sides, at most half the chord away) |
| 3 surface links | yes | `build_surface_link` feeds straight between its `sampling` (0.5 mm) samples | `build_surface_link(.., tolerance: Some(t))` refines each feed (`refine_track`, contact coverage) or refuses the link; `RelinkParams`/`LinkGeometry::link_tolerance` carry `t`. Some(op tolerance): the unified finish (relink and router) and the scallop relink. None (unchanged): pencil, curve engrave, project curve, standalone raster and waterline, costing, the research rigs |
| 4 hookup-0 link | yes | a region span starts at the router's link into that region, so a rapid-order barrier splits "cut, link, next region" before the link. The group before is reordered and ends retracted over another segment; the next group (one segment) was copied as it is (`dressup/tsp.rs:336` at `5b34e710`), so its first move, the link, was fed from there: 29.36 mm, 3.453 mm under the surface | `tsp::rejoin_displaced_fed_start`: when a copied group's first move is fed and the tool is not where the input had it, retract and rapid over the move's target first (the shape `rebuild_group` gives every segment). A group whose predecessor is in place is copied byte-identically |
| 5 sim overcut | yes, a real cut | see below: the drop-cutter floor is wrong | applied in part 3 (lead decision) |

Sentry `tests/fed_chords_ride_the_surface_g_tierburial.rs`, four tests.
Each was red with its fix removed and is green with it:

| test | fixture | straight (non-vacuity) | fix removed | fix |
|---|---|---:|---:|---:|
| raster | terrace 1.5 mm, all Shallow | 0.7067 | 0.1439 | 0.0375 |
| waterline | 80° mesa, all VerySteep | 0.2194 | 0.2194 | 0.0250 |
| link | 75° ridge, 10 sample phases | 0.2416 | 0.2416 | 0.0366 |
| rapid order | 5 + 1 segments behind a barrier | — | link fed from (43, 5, 10) | vertical descent |

(mm under the drop-cutter surface, tolerance 0.05.)

**rivmap100 fine tier** (arcs off, stock Fresh):

| | `5b34e710` | working tree |
|---|---:|---:|
| fed samples > 0.1 mm deep | 1 866 | 913 |
| long cut chords > 0.1 (Shallow / VerySteep), max | 445 / 55, 0.325 | 0 |
| links > 0.1: samples, max vertical, max estimate | 480, 0.356, 0.147 | 6, 0.174, 0.025 |
| short cut chords > 0.1: samples, max estimate | 886, 0.060 | 907, 0.112 |
| sim columns below −0.1 mm (min) | 23 726 (−0.612) | 23 694 (−0.612) |
| moves | 77 459 | 79 788 |
| cycle time (s) | 2 989.5 | 3 061.9 (+2.4 %) |
| `nohookup`: max depth, links > 0.1 | 3.453, 288 | 0.516, 0 |
| `nohookup`: cycle time (s) | 4 773.6 | 4 805.5 |

The short-chord estimate 0.112 is one waterline vertex (68.500, 88.036,
z 8.252), 0.112 under the drop-cutter floor: a push-cutter vertex, not a
chord. It was in the long-chord class at `5b34e710` (its chord is now
split). Open.

**Source 5 is a drop-cutter defect.** The simulation hardly moved, so the
overcut is not the chords. The instrument now takes the deepest columns
and finds the move whose tool reaches lowest over each:

- (15.7–16.4, 0.0), −0.612: MidSteep ring 2, move at y = 0.763, z 11.034.
  The tool's ball reaches 0.612 below the model at the board edge. The
  drop-cutter floor at y = 0.75 is 11.05; a plain R1.0 ball reads 11.66
  there. `TaperedBallEndmill::facet_drop` (`tool/tapered_ball.rs:276-282`)
  sets the tip on the facet under the axis and reports `found` for a
  sloped facet, which is not a tangency. `MillingCutter::drop_cutter`
  (`tool/mod.rs:548`) then skips that triangle's edges and vertices. In the
  interior a neighbour triangle supplies the edge; at the mesh boundary
  (the board edge here, a 100 mm triangle) nothing does. On a 0.5 mm grid
  over the board the floor of tool 6 (the tier's R1.0 taper) is below the
  plain R1.0 ball's at 681 of
  40 401 points, all but one within 1 mm of the edge, by up to 0.499 mm.
- (61.5, 3.8), −0.562: MidSteep ring 5; the floor has a 0.02 mm wide,
  0.56 mm deep notch at x = 61.53. It is not geometry. `edge_drop` of the
  ball (`tool/ball.rs:168`), the bull nose (`tool/bullnose.rs:261`) and
  the taper's ball region (`tool/tapered_ball.rs:328`) places the contact
  at `t_closest + s cos_a` with `cos_a = −slope / sqrt(1 + slope²)` for the
  upper solution. The tangent point of a circle resting on a line of slope
  m is at `+ s m / sqrt(1 + m²)`: the code takes the mirrored point, which
  is downhill, so the floor is low on every sloped edge (exact on a
  horizontal one). Against a 200 000-point sampled truth on a single edge:
  slope 0.5, 0.27–0.45 mm low; slope 1.174, 1.07–1.79 mm low; slope 0, exact.
  The burial audit (`EntrySurfaceProbe::floor_z`) calls the same
  drop-cutter, so it cannot see these gouges; that is why the columns had
  no buried sample near them, and why the "short chords over near-vertical
  steps" of part 1 existed: the steps are this error.

Measured with both kernel fixes applied temporarily (`dt = −s cos_a /
edge_len_xy` in the three `edge_drop`s, and the taper's tip branch no
longer `found`), then reverted:

| rivmap100 fine tier | `5b34e710` + kernel | working tree + kernel |
|---|---:|---:|
| fed samples > 0.1 mm deep | 348 | 1 |
| long cut chords > 0.1: raster / waterline, max | 81 / 93, 0.189 | 0 |
| links > 0.1: samples, max, max estimate | 174, 0.164, 0.105 | 0 |
| sim columns below −0.1 mm (min) | 10 (−0.133) | 6 (−0.111) |
| moves | 67 558 | 68 946 |
| cycle time (s) | 2 640.9 | 2 663.5 (+0.9 %) |

So the kernel error is the bulk of the simulated overcut, and the chord
fixes of this section are still needed once it is fixed. (With the kernel
fixed at `5b34e710`, `nohookup` reads max 0.189 and no long link: the
29.36 mm link needs the barrier and reorder of the unfixed geometry. The
rapid-order mechanism stands on its own; the sentry shows it.) The kernel fix is
not a default change, but it moves every drop-cutter path of a ball, bull
or tapered tool. The lead decided to apply it: part 3.

**Moved by this change.** Every test in the requested set passes except
two. `scallop_trace_survives_relink_g_linktrace` is red identically at
`5b34e710` (digest `0xb5906eb187c207c6` on both; checked on an extracted
HEAD tree). `tier_arcfit_burial_f4`'s sentry is red: arcs off 0.0513 →
0.0358 mm (the waterline chord fix removed the source path's sag), arcs at
0.05 still 0.0882, bound 0.0858. Its doc foresaw this: the 0.05 arc
excess (0.052 mm now) is the F4 mechanism, which the source sag used to
hide. At 0.015 (F4a) the same fixture reads 0.0358 (204 arcs, arc moves
0.0134), inside its bound. The assert is not changed; F4a is below.

## Fine-tier burial, part 3 (G-TIERBURIAL) — the kernel and the arc fitter

Lead decision: both are correctness fixes, not default changes. Working
tree over `5b34e710` with part 2.

**Drop-cutter kernel.** The ball (`tool/ball.rs:168`) keeps its closed
form with the sign fixed: the tangent point of a circle resting on a line of
slope `m` is at `u = + s m / sqrt(1 + m^2)`, the centre at `z_c + s sqrt(1
+ m^2)`. The bull nose and the tapered ball did their own closed forms with
the same sign error, and the bull nose's torus form ("simplified approach")
was also off by up to 1.15 mm after the sign fix; the taper's cone part by
up to 0.033 mm. Both now call one exact helper,
`tool/mod.rs::edge_drop_by_profile`: the touch height `z(w) - height(rho)`
along the edge is concave in `w` (convex profile), so its maximum is the
unconstrained argmax clamped to the edge and the reach. The taper's argmax
is closed-form (ball part `w = m s / sqrt(1 + m^2)`; cone part `w = q d /
sqrt(1 - q^2)`, `q = m tan(alpha)`; the shank rim for `|q| >= 1`); the bull
nose's is a bisection on the slope of the touch height (the torus contact
is a quartic). `TaperedBallEndmill::facet_drop` no longer reports `found`
for the tip-on-facet point of a sloped facet, so `drop_cutter` tests that
triangle's edges.

Sentry `tests/edge_drop_matches_the_sampled_profile_g_tierburial.rs`: each
tool against a profile truth (dense scan plus ternary search over the edge,
for edges of slope 0, 0.5, 1.174 and CL offsets across the ball, torus,
flat and cone parts), and the taper on a sloped boundary triangle through
`drop_cutter`. Before: worst error ball 1.787 mm, bull nose 2.897 mm, taper
1.787 mm; boundary triangle 11.0508 against 11.6614. After: 6.2e-15, 5.3e-15,
7.1e-15 mm; boundary triangle exact.

**Arc fitter** (`dressup/arcfit.rs::try_fit_arc`). Two defects past the
three one-axis tests:

- The budgets add: a vertex `tolerance` off the circle beside a chord
  whose sagitta is `tolerance`. Now every source segment is matched to the
  arc span between its vertices' swept angles and the 3D distance (XY and
  helix Z together) must hold `tolerance` at 9 points per segment.
- A half circle's direction is decided by rounding (both halves have the
  same length). The `arc_raster` fixture's rows at y0 = 8 and 12 were
  emitted as the MIRRORED half circle, 11.3 mm off the path. Now the
  source's own sweep (`cum`) must equal the emitted sweep to 1e-6 rad.

Sentry `tests/an_arc_fit_holds_its_source_in_3d_g_tierburial.rs`: the
`arc_raster` half circles (red at `5b34e710`: 11.3137 mm; now 0.0171 mm) and
a circle with vertices ±0.035 mm off it in blocks (red: 0.0679 mm; now
0.0485 mm). `tier_arcfit_burial_f4` is green without touching its assert:
arcs off 0.0348, arcs at 0.05 → 0.0560 (bound 0.0848; 0.0878 with the 3D
check removed), arcs at 0.015 → 0.0348.

**rivmap100 fine tier** (`tier_fine_burial_sources_g_tierburial`, arcs off
as before, so the fitter does not act here):

| | `5b34e710` | part 2 | part 2 + 3 |
|---|---:|---:|---:|
| fed samples > 0.1 mm deep (max vertical) | 1 866 (0.516) | 913 (0.516) | 1 (0.112) |
| sim columns below −0.1 mm (min) | 23 726 (−0.612) | 23 694 (−0.612) | 6 (−0.111) |
| moves | 77 459 | 79 788 | 68 945 |
| cycle time (s) | 2 989.5 | 3 061.9 | 2 663.5 |
| `nohookup`: max depth, cycle time (s) | 3.453, 4 773.6 | 0.516, 4 805.5 | 0.112, 4 496.5 |

The one sample left is the waterline vertex at (68.500, 88.036, z 8.252),
0.112 mm under the floor; the six columns are around it. A push-cutter
contour point the drop-cutter reads as buried. Open.

**Tests that moved, and why.** Of 118 binaries in the kernel's reach (`ls
tests | grep -iE "drop|raster|waterline|scallop|pencil|unified|finish|adaptive3d|rest|tier|arcfit|link|surface|wanaka|arc"`,
all features, plus the ones named by the lead), these moved; each was
attributed by re-running with the kernel fix or the fitter change held
back:

| test | cause | action |
|---|---|---|
| `finish_resolution_policy_pr3` scallop / ramp / steep_shallow fingerprints | kernel (the ridge crest is a sloped convex edge under the taper) | re-pinned with history: scallop 2820 → 2583 moves; ramp and steep_shallow keep their counts (277, 913), Z moved |
| `perf_golden_sim_metrics` 3D arm (31 fields) | kernel (hemisphere facet edges); the fitter does not move it | re-baselined, logged in its doc: DropCutter removed 1365.26 → 1352.68 mm³, Waterline 2016.10 → 2027.38 mm³ |
| `scallop_oracle_validation_m4::tool_reach_floor_error_is_mesh_faceting_and_shrinks_with_it` | kernel: the "faceting" error (−22.93 / −5.56 / −0.00 µm at mesh steps 0.20 / 0.10 / 0.05) was the edge sign; now 0 to rounding at every step | renamed `tool_reach_floor_has_no_negative_error_at_any_mesh_step`; asserts every step > −1e-6 µm (stronger than before) |
| `feeds_matrix_instrument_fm1` sim subset, 4 cells | Adaptive EndMill / BullNose plywood_hardwood `depth_peak_mm` 4.5000 → 4.2188, 4.4255 → 4.1996: the fitter (the old peak came from mis-fitted arcs: with arc fitting off the same cells read 4.2188 / 4.2015; the old fitter emitted a half-turn entry helix of R 1.8 whose source sweep read 110 rad against an emitted pi, three R 0.1 half turns alike, and arcs up to 0.061 mm off their source). Scallop TaperedBallNose hardwood / softwood `power_peak_kw` 0.1238 → 0.1240, 0.0739 → 0.0740: kernel | re-blessed; the 960-cell pre-simulation CSV did not move |
| `transform_provenance_fingerprints::arc_raster_full_dressups_fingerprint`, `arcfit_intent_key_cost_f1` (arc_raster) | fitter: the mirrored half circles (rows y0 = 8, 12) now fit `ArcCW` over 23 chords plus one linear chord | re-pinned 72 → 74 moves, arcs 21 unchanged |
| `capability_link_moves_safety::unified_finish_node_barriers_allow_intra_region_reorder_and_pin_depth` | kernel: the hemisphere finish now emits 1 879 moves (was 2 277, cut 1 992.6 → 1 575.8 mm) and its three multi-segment groups are already in nearest-first order (TSP rapid 562.4 → 562.4). The sentry's "TSP must reduce rapids" has nothing to reduce | part 4: fixture re-derived (boss on a plate), green |
| `capability_link_moves_safety::steep_shallow_split_barriers_allow_intra_half_reorder_and_pin_depth` | kernel: the TSP still reorders (5 groups of 4, one of 50 segments) but the rapid total rises 2 850.4 → 2 860.7 (old 2 784.0 → 2 745.7). The TSP minimises XY hops inside a group and ignores the hop to the next group's first segment; the new geometry puts the fixture on the losing side of that margin | part 4: the TSP no-regression guard, green on the same fixture |
| `heatmap_two_arc_divergence_a1::the_retired_measure_still_reproduces_the_defect` | not this change: red identically on an extracted `5b34e710` tree (feeds and tool-load only) | none |
| `scallop_trace_survives_relink_g_linktrace` | not this change: red identically at `5b34e710` (digest `0xb5906eb187c207c6`) | none |

**Roughing benchmark** (`planning/fixtures/rivmap100/arm.sh`, 3D Rough,
flat end mill, so only the fitter acts; release CLI, `5b34e710` →
working tree):

| arm | moves | total s | cut | entry | rapid | vol mm³ |
|---|---:|---:|---:|---:|---:|---:|
| by_area | 14 268 → 14 877 | 1 915 → 2 173 | 542 → 542 | 1 122 → 1 380 | 130 → 130 | 49 339 → 49 338 |
| global | 17 108 → 17 834 | 2 345 → 2 655 | 614 → 614 | 1 424 → 1 735 | 187 → 187 | 49 782 → 49 781 |

The +609 moves are all `EntryHelix` (+596 arcs): helix arcs whose flat lap
and descent the old fitter bridged within 0.05 on each axis but not in 3D
are now two arcs. The helix path length is unchanged (20 174.2 → 20 175.2
mm). The +258 s is the cycle-time model, not the path: the integrator
treats an arc as a straight move and takes "the chord tangent for
junction-velocity geometry" (`machine/kinematics.rs` module doc), so each
new arc-to-arc junction on a tangent-continuous helix reads as a corner at
the junction-deviation limit. With feed modulation off the step is the
same (entry 1 367 → 1 664 s). A tangent-aware arc junction in the
integrator is the fix for the reading; not done here.

**wanaka** (`p1_headless_ab_wanaka`, release): `rapid_collisions=0
(baseline 0)`, project 14 066.4 s.

## Fine-tier burial, part 4 — the TSP no-regression guard

Lead decision: the barriered TSP must never make rapid travel longer than
the planner's own order.

**Guard** (`dressup/tsp.rs`). Per group, the rebuilt group (TSP order,
every segment re-framed at `safe_z`) replaces the input group only when its
rapid travel, measured from where the tool is, is strictly shorter;
otherwise the group is copied as it is, order and framing, move for move
(with the part 2 rejoin when its fed first move was displaced). A group
kept as it is ends where the input ended, which moves the next group's
entry, so the pass also compares totals: an output whose rapid travel is
not strictly shorter than the input's is discarded and the input returned
unchanged (index-preserving). Unit tests: `a_toolpath_already_in_its_best_order_comes_out_unchanged`
(framed at `safe_z` and at 2 mm; bit-identical) and
`the_guard_keeps_an_ordered_group_and_still_reorders_its_neighbour`; both
red with the guard removed.

| sentry | before the guard | after |
|---|---:|---:|
| SteepShallow rapid, planner -> TSP (mm) | 2 850.4 -> 2 860.7 | 2 850.4 -> 2 725.3 |
| UnifiedFinish, bare hemisphere (mm) | 562.4 -> 562.4 | 562.4 -> 562.4 (unchanged input) |
| UnifiedFinish, boss on a plate (mm) | — | 1 225.2 -> 1 144.6 (-6.6 %) |

The UnifiedFinish sentry's fixture is now a 10 mm hemisphere boss on a
50 mm plate: the plate is a Shallow node of seven cut runs around the boss,
and that node's rapid goes 312.8 -> 256.8 mm (every other node unchanged).
The assertion stays strict. SteepShallow keeps its four islands.

Two other fixtures met the guard: each was in its best order, or its
reorder cost more than it saved, so the guarded pass returned it unchanged: `fed_chords_ride_the_surface_g_tierburial`'s rapid-order test
(3 + 1 segments; the rejoin back to the link cost more than the reorder
saved: 108 -> 120 mm), and `tsp_synthesized_rapid_intents` (two segments in
order). Both fixtures were re-derived so the reorder wins (5 + 1
interleaved segments; three segments at x 0, 100, 10). Both are green and
the rejoin sentry is red with the rejoin removed.

**Release instruments after the guard.** rivmap100 fine tier
(`tier_fine_burial_sources_g_tierburial`, release-fast): arm `sim` 68 945
-> 68 935 moves, cycle time 2 663.5 -> 2 659.1 s, still 1 fed sample over
0.1 mm (0.112, the waterline vertex) and 6 sim columns below -0.1 mm (min
-0.111); arm `nohookup` 4 496.5 -> 4 495.7 s, max depth 0.112. wanaka
(`p1_headless_ab_wanaka`, release): `rapid_collisions=0 (baseline 0)`;
Unified Finish 11 998.9 -> 11 982.3 s (linking 1 377.2 -> 1 362.2, rapid
2 699.1 -> 2 697.5), every other op unchanged.

**Full core gate after the guard** (418 binaries, all features, debug):
1 816 passed, 24 failed. 18 of the 24 were red at the 2026-09-27 gate
(feeds, session scans, calibration; of these only the heatmap and
linktrace tests were re-checked at `5b34e710`, both red there, and both
queued for the full-gate triage). The other six:

| test | at `5b34e710` | cause | state |
|---|---|---|---|
| `tsp_synthesized_rapid_intents` | green | the guard (fixture in order) | fixture re-derived, green |
| `crease_own_region_pr6b` byte identity | red (952 / 660 moves vs pinned 951 / 659) | pre-existing; this change moves the taper to 957 | open, not re-pinned |
| `checkpoint_b_resolution_ab::ramp_finish_geo_mean_policy_halves_the_descent_chords` | green | kernel (green with the `5b34e710` tool files) | red: narrow ridge, a cutting point 0.0258 mm under the reference tool-centre surface. Open |
| `ramp_reach_clamp_pr8b::a_truncated_descent_is_reported_with_its_magnitudes` | green | kernel | red: pinned "4.231" mm, now 4.235. Open |
| `steep_shallow_min_segment_pr8d::the_floor_is_inert_where_nothing_was_degenerate` | green | kernel | red: narrow valley cutting length 3 625.5 -> 3 624.11 mm. Open |
| `a_clearing_cut_holds_the_pass_load_g_adaptpassload` | green | not the guard, not the kernel (red with either held back): the arc fitter is the remaining suspect | red: peak 0.674 vs limit 0.363 + 0.183, 2 samples over. Open |

## Fine-tier burial, part 5 — the 2D Adaptive overload, two re-pins, the arc junction

Working tree over `5b34e710` with parts 2–4.

**2D Adaptive overload** (`a_clearing_cut_holds_the_pass_load_g_adaptpassload`,
red in part 4). Move 2586, (14.63, 2.25) -> (15.60, 1.10) at z -3 (move
6093 at z -6), is one 1.5 mm Agent step. On the exact geometry of the
emitted path (`print_exact_width_along_move`) it cuts 0.278 / 0.678 /
0.698 / 0.174 of D at its four samples; the simulation reads 0.674. The
planner read a step at its end disc only, and the lump of stock behind the
step's middle is outside both end discs. Not a link stamped differently
from what was emitted, not stale state: the new read sees that stock on
the planner grid and refuses the step. Fix (`adaptive/search.rs::measure_step`):
under the swept-width measure a step is also read at each planner cell
along it, the disc there with the stretch already swept (the capsule from
the start to the previous point) read as cut; the reading is the largest.
The end reading is unchanged, so no step the end refused is admitted. Unit
sentry `a_step_is_held_along_its_length_not_at_its_end` (two stock points
5.5 mm apart either side of a 3 mm step, out of both end discs): red with
the along-step loop removed (end reading 0), green now (0.92 of D).

| six-island fixture | before | after |
|---|---:|---:|
| S1 samples over 0.546 | 2 | 0 |
| S1 peak radial | 0.674 | 0.415 |
| S1 cycle (s) | 687.1 | 752.5 |
| plunge-entry arm: sim samples over 0.546 | 0 | 2 (sim only) |
| plunge-entry arm: cycle (s) | 881.5 | 815.7 |

On the plunge-entry arm (`a_straight_plunge_entry_is_exempt_only_leaving_its_hole`)
two samples read 0.558, on move 1682 (-28.82, 4.93) -> (-30.31, 4.75) and
its z -6 twin 4885. The exact width of that move is 0.396 / 0.363 / 0.278
(0.407 on move 1681): in bound. The simulation lumps the move's removal
into one sample (0.016, 0.096, then 7.407 mm^3) and reads the extent of
that sample's disc. Lead decision: the test now clears each sim sample
over the bound by the exact width at every sample of its move (bound plus
the polygon flattening, 10 um, over D), fails on a sample the exact
geometry also reads over, and pins the sim-only count at 2. Both S1 tests
are green with the load asserts unchanged.

**Two re-pins, kernel** (each green with the four `tool/` files of
`5b34e710`, red with the kernel fix): `ramp_reach_clamp_pr8b::a_truncated_descent_is_reported_with_its_magnitudes`,
max lift "4.231" -> "4.235" and holdable bottom "-2.407" -> "-2.352" (the
corrected edge contact wedges the taper 0.055 mm higher in the cone);
`steep_shallow_min_segment_pr8d::the_floor_is_inert_where_nothing_was_degenerate`,
narrow valley cutting length 3 625.5 -> 3 624.1 mm (shortest segment
unchanged). `crease_own_region_pr6b` stays red (red at `5b34e710`).

**Fault injection, `checkpoint_b_resolution_ab`** (temporary edit,
reverted): the highest narrow-ridge cutting point, move 43 at (0.0274,
8.3896, 3.8122), exact drop 3.8122, lowered 0.05 mm:
`deepest_below_exact_drop` 0.000000 -> -0.050000 and the gate fails. The
exact reference sees a 0.05 mm gouge at one point.

**Cycle-time model, arc junctions** (`machine/kinematics.rs`). The
integrator took an arc's chord direction for the junction geometry. On the
roughing benchmark's G-code (`rivmap100_ladder_demo.toml`, the arm the
part 3 numbers came from: no `--set`, depth per pass 2) the 5 537
arc-to-arc junctions read:

| junction angle (deg) | <= 0.1 | <= 1 | <= 5 | <= 20 | <= 90 | <= 180 |
|---|---:|---:|---:|---:|---:|---:|
| chord directions | 0 | 0 | 18 | 29 | 2 659 | 5 537 |
| 3D end tangents | 451 | 921 | 1 948 | 3 678 | 5 537 | 5 537 |
| XY end tangents | 4 559 | 4 797 | 5 338 | 5 506 | 5 537 | 5 537 |

(cumulative). The XY tangents are continuous; the 3D angles come from the
Z slope, which changes where the terrain-clamped helix changes slope (an
arc's Z is linear; the source polyline has the same corners). So the
junctions are not a fitter defect. Fix: `move_end_tangents` gives each
move's unit tangent at its start and end (an arc's own tangent, a helix's
with its slope; a line's chord), and the integrator and
`kinematic_utilization` read them at every junction. No angle threshold:
the junction-deviation rule reads the true angle; at 500 mm/s^2, delta
0.02 mm and 40 mm/s it runs a junction at feed up to 12.8 degrees. Unit
tests `a_tangent_arc_junction_runs_at_the_commanded_feed` and
`a_helix_slope_change_is_read_as_its_3d_angle`, both red with chord
junctions. Re-pinned: `kinematics_per_axis_rate_p1` EDG-07 bits (the
fixture's `ArcCW` (40, 5) -> (45, 10) about (40, 10) is a 270 degree
arc: `cutting_s` 1.272391 -> 1.315781 s, move 9 feed 775.84 -> 653.39
mm/min).

| rivmap100 roughing, total s (moves) | old fitter, chord model | new fitter, chord model | old fitter, tangent | new fitter, tangent |
|---|---:|---:|---:|---:|
| ladder demo, by_area, dpp 2 | 1 915 (14 268) | 2 173 (14 877) | 1 852 | 2 106 |
| ladder demo, global, dpp 2 | 2 345 (17 108) | 2 655 (17 834) | 2 274 | 2 582 |
| ladder demo, by_area, dpp 8 | — | 1 799 (11 799) | 1 497 (11 205) | 1 716 |
| ladder demo, global, dpp 8 | — | 1 904 (12 521) | — | 1 818 |
| live_0925, by_area, dpp 8 | — | 809 (8 935) | 690 (8 707) | 696 |
| live_0925, global, dpp 8 | — | 860 (9 510) | — | 739 |

The tangent read removes 67 s of the 258 s; the fitter's step stays
+254 s. The rest is the integrator costing an arc at its CHORD length
(`chord_length` ignores I/J; the module doc said "equal arc length"). Arc
chord against arc length on the ladder-demo G-code: old fitter 9 935 /
19 062 mm, new fitter 12 418 / 19 059 mm. Splitting a long arc made the
reading less wrong, not the path longer. An experiment with arc length
(reverted): old fitter 2 525 s, new fitter 2 617 s (+3.6 %); dpp 8
2 182 -> 2 229 s; live_0925 840 -> 799 s. Lead decision: arc length is the
next change, committed on its own.

**FM1 re-bless** (`feeds_matrix_instrument_fm1`, sim subset; each cell
attributed by holding the 2D Adaptive change or the kinematics change
back; the 960-cell pre-simulation CSV did not move):

| cell | column | before -> after | cause |
|---|---|---|---|
| EndMill Adaptive mdf | power, deflection, depth | 0.0264 -> 0.0239, 0.0042 -> 0.0043, 4.5000 -> 4.2188 | planner (depth, power); deflection both |
| EndMill Adaptive plywood_hardwood | depth | 4.2188 -> 4.5000 | planner |
| BullNose Adaptive softwood | power, deflection, depth | 0.0252 -> 0.0283, 0.0053 -> 0.0056, 4.1432 -> 4.4424 | planner |
| BullNose Adaptive hardwood | power, deflection, depth | 0.0467 -> 0.0455, 0.0071 -> 0.0075, 3.8666 -> 4.4154 | planner |
| BullNose Adaptive mdf | power, deflection, depth | 0.0266 -> 0.0361, 0.0053 -> 0.0060, 4.0881 -> 4.1631 | planner |
| BullNose Adaptive plywood_hardwood | depth | 4.1996 -> 4.4910 | planner |
| EndMill Adaptive softwood | power, deflection | 0.0162 -> 0.0210, 0.0025 -> 0.0035 | both (kinematics alone 0.0171, planner alone 0.0161): the junctions no longer slow the predicted feed the gates read |
| EndMill Adaptive hardwood | power, deflection | 0.0271 -> 0.0352, 0.0042 -> 0.0059 | both (0.0286 / 0.0269) |
| EndMill Waterline softwood | power, deflection | 0.0534 -> 0.0552, 0.0059 -> 0.0064 | kinematics |

Every verdict state is unchanged.

**Verification** (cloud, gate features `heavy-tests,research,test-support`
unless noted): 88 binaries (every `tests/` file matching
tier|arcfit|link|raster|waterline|scallop|drop|adaptive|tsp|capability|checkpoint|ramp_reach|steep_shallow,
plus `transform_provenance_fingerprints`, `perf_golden_sim_metrics`, FM1,
the machine sentries and `crease_own_region_pr6b`): all green except
`scallop_trace_survives_relink_g_linktrace` and `crease_own_region_pr6b`,
both red at `5b34e710`. `cargo test -p rs_cam_core --lib`: 2 640 passed;
`rs_cam_cli`: 58 passed; `rs_cam_viz --no-fail-fast`: 1 036 passed, 0
failed; `cargo fmt --check` clean; the full clippy line of `CLAUDE.md`
clean (one lint in the new unit test fixed: a manual slice fill).

## Fine-tier burial, part 6 — an arc is timed at its arc length (2026-10-01)

Working tree over `0617bf8e`. The part 5 lead decision, on its own.
`machine/kinematics.rs`: `move_length` gives the arc length of an arc
(`hypot(r * sweep, dz)`, a full flat turn `2 pi r`; a line or a rapid its
chord); `move_directions` gives the chord direction (accel and rate
ceiling read it; a full flat turn reads its start tangent) and the two end
tangents. One helper, `arc_span`, holds the arc geometry for both and for
`move_end_tangents`. `kinematic_utilization` reads the same two functions.
Unit test `an_arc_is_timed_at_its_arc_length`: a half circle of r 10 costs
a line of 10 pi at the same feed; red with the chord (1.080 s against
1.651 s), green now. A full flat turn costs a line of 20 pi (the chord
gave 0).

Re-pin, `kinematics_per_axis_rate_p1::one_shared_pass_leaves_both_integrators_bit_identical_edg07`:
the fixture's 270 degree `ArcCW` is 23.562 mm, its chord 7.071 mm; the
extra 16.491 mm at its 40 mm/s cruise is 0.412 s. `cutting_s` 1.315781 ->
1.728053 s, `total_s` 6.486188 -> 6.898460 s; every other field and every
predicted feed is bit-identical. `machine_kinematics_cycle_time_f034` is
red before and after (stale wall-clock reference); its model value is now
1 619.6 s against 827 s measured (ratio 1.958). Left for triage.

rivmap100 roughing (`arm.sh`, release CLI, resolution 0.5; before = a
detached worktree at `0617bf8e`):

| arm | moves | before total (entry) s | after total (entry) s | change |
|---|---:|---:|---:|---:|
| ladder demo, by_area, dpp 2 | 14 877 | 2 106 (1 313) | 2 617 (1 824) | +24.3 % |
| ladder demo, global, dpp 2 | 17 834 | 2 582 (1 662) | 3 287 (2 367) | +27.3 % |
| ladder demo, by_area, dpp 8 | 11 799 | 1 716 (1 316) | 2 229 (1 829) | +29.9 % |
| ladder demo, global, dpp 8 | 12 521 | 1 818 (1 395) | 2 382 (1 960) | +31.0 % |
| live_0925, by_area, dpp 8 | 8 935 | 696 (295) | 799 (398) | +14.8 % |
| live_0925, global, dpp 8 | 9 510 | 739 (317) | 846 (424) | +14.5 % |

Moves, volume and peak DOC are unchanged; cut and rapid time are
unchanged; only entry time moves (the arcs on this stream are the helix
entries). The "before" column equals part 5's "new fitter, tangent"
column; the by_area dpp 2 / dpp 8 and live by_area values equal the part 5
experiment (2 617, 2 229, 799 s).

Verification (cloud): `kinematics_per_axis_rate_p1` 9 passed; `--lib
machine` 62 passed; the 24 kinematics / cycle / modulation / rpm / power
sentries (`heavy-tests,research,test-support`) all green except F-034
(above); FM1 (`heavy-tests`) 2 passed, no re-bless; `--lib` 2 680
passed; `cargo fmt --check` clean; `clippy -p rs_cam_core --all-targets`
with the gate features clean.

## Changes that wait for approval, with the measured effect

1. (Applied 2026-10-01, "Defaults applied".) F1: `TierIslandParams::max_close_raises` default 3 → 0. 350 mm proxy:
   owned 47 901 → 17 538 mm² (−63 %), machining 90 152 → 57 241 mm²
   (−37 %); 87 islands (2 981 mm²) go back to the coarse tier. 100 mm
   proxy: no change (the cap does not act).
2. (Applied 2026-10-01, "Defaults applied".) F4a: tier-op arc tolerance 0.05 → cusp/2 = 0.015 (in
   `plan_tier_operation`'s dressups only). Mesa: burial 0.0882 → 0.0531 mm,
   simulated gouge 0.082 → 0.026 mm. rivmap100: fed samples deeper than
   0.05 mm 2.30 % → 1.59 %.
3. F3b (cells reached only by the body leave the fine tier): no measured
   effect on either proxy (0 binding cells, also at L_c 15). Low priority on
   this terrain.
4. F2 (band): not decided here. With F1 at 0 the band ratio is 3.26x on the
   350 mm proxy.
5. (Applied, part 3.) G-TIERBURIAL kernel: the `edge_drop` contact and the
   taper's tip branch.
6. (Resolved without F4a, part 3.) `tier_arcfit_burial_f4` is green at 0.05
   and 0.015: the arc fitter now holds its tolerance in 3D.

## Defaults applied (2026-10-01)

Operator approval 2026-10-01 ("I'll take your recommendations on the
finishing work"). Two default changes, each its own commit. Working tree
over `a0f75b27`. Every number below comes from ONE release-fast build of
that tree. A "before" arm writes the old value explicitly, which runs the
same code path as the old default.

**F1.** `TierIslandParams::max_close_raises` defaults to
`DEFAULT_MAX_CLOSE_RAISES` = 0 (was `MAX_CLOSE_RAISES` = 3, which stays
the ceiling). The struct-level `#[serde(default)]` reads `Default`, so a
file without the key reads 0, and a file that states the key keeps it
(`a_saved_raise_bound_survives_the_default_change`). The key is always
written, so every project saved since the dial landed states 3. In the
repo that is one file, `planning/fixtures/rivmap100/rivmap100_tiered_finish.toml`
(changed to 0 here, see "Fixture"). These files carry tier recipes without
the key, and now read 0: `planning/multitool_2026-08-23/wanaka200_mt2.toml`,
`wanaka200_mt2_overlap02.toml`, `wanaka200_iso_scallop.toml`,
`planning/deep_doc_modulation_2026-09-08/T3b_r10_scallop_islands_relink3.toml`,
`crates/rs_cam_core/tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml`.

**F4a.** A planner tier op fits arcs at `cusp_height_mm / 2`
(`session::multitool::tier_arc_tolerance_mm`, applied by
`plan_tier_dressups` in `build_tier_config`): 0.015 mm at the planner's
0.03 mm. A cusp height that is not positive and finite turns the tier's
arcs off. The Finish role and every op that is not a tier op keep 0.05.
Sentry `tests/tier_ops_fit_arcs_within_half_the_cusp_f4a.rs`: red with
`DressupConfig::for_op` put back (tier 0 reads 0.05, want 0.015).

**Fixture.** `rivmap100_tiered_finish.toml` holds "the tier ops the
planner emits". The writer (`write_rivmap100_tiered_finish_fixture`) now
emits `max_close_raises = 0` and `arc_tolerance = 0.015` on both tier
ops. It also gives the ops new ids and new Suggest feeds for tier 1 (the
vendor table moved after 09-30). So only the three values the two changes
own were changed in the file: 1 line for F1, 2 lines for F4a. The feeds
stay as measured on 09-30.

### Territory (`rivmap100_tiered_finish_step0`, `rivmap350_tiered_finish_step0_preview`)

Areas in mm². The 350 mm map is not the map of the 09-30 table (raw
19 659 → 14 531, raw islands 14 594 → 5 268): the part 3 drop-cutter
kernel fix moved the tier map.

| board | raise bound | owned | machining | kept | raises | final radius | dropped (area) | holes owned → machining |
|---|---|---:|---:|---:|---:|---:|---|---|
| 100 mm | 3 (before) | 1 543 | 7 563 | 11 | 0 | 0.500 | 0 (0) | 38 → 49 |
| 100 mm | 0 (F1) | 1 543 | 7 563 | 11 | 0 | 0.500 | 0 (0) | 38 → 49 |
| 350 mm | 3 (before) | 39 543 | 80 991 | 6 | 3 | 1.688 | 0 (0) | 463 → 602 |
| 350 mm | 0 (F1) | 7 193 | 30 145 | 24 | 0 | 0.500 | 99 (4 599.5) | 185 → 63 |

On the 100 mm board the cap does not act at either bound (11 islands), so
F1 changes nothing there. On the 350 mm board F1 cuts owned area by 82 %
and machining area by 63 %; 99 islands (4 599.5 mm²) go back to the
coarse tier. The band ratio grows from 2.05x to 4.19x of the smaller owned
set (plan F2). The 350 mm tier was not generated in this round, so its
link, move and time effect is not measured.

### Toolpaths

CLI: `rs_cam_cli project rivmap100_tiered_finish.toml` (the whole chain:
tier ops on remaining stock, simulation 0.2 mm, modulation on). The CLI
gives move counts and the project runtime. It gives no per-op time or
link count. Those come from `arc_fit_burial_on_the_rivmap100_fine_tier`
and `tier_fine_burial_sources_g_tierburial`: the fine tier alone, stock
Fresh, cycle time from `compute_cycle_time` (machine kinematics, rapid =
max feed). Arm A (0.05) is before F4a, arm C (0.015) is after; the arm
`sim_arcs` (new flag `arcs`) simulates with the fixture's own arcs.
Links: fed `Linking` moves, and rapid runs (one per air link).

| measure | before | after F1 | after F1 + F4a |
|---|---:|---:|---:|
| CLI project runtime (s) | 12 180.8 | 12 180.8 | 12 624.8 (+3.6 %) |
| CLI moves, tier 0 / tier 1 | 38 791 / 40 624 | 38 791 / 40 624 | 42 280 / 54 434 |
| CLI cutting / rapid distance (mm) | 172 196 / 109 095 | 172 196 / 109 095 | 172 321 / 109 092 |
| CLI holder collisions / rapid collisions | 0 / 0 | 0 / 0 | 0 / 0 |
| fine tier: moves (arcs) | 40 621 (14 523) | same | 54 406 (10 892) |
| fine tier: fed `Linking` moves / rapid runs | 3 399 / 149 | same | 3 410 / 149 |
| fine tier: cycle time (s) | 2 022.6 | same | 2 353.4 (+16.4 %) |
| fine tier: fed samples > 0.05 mm (arc + cut + other) | 3 405 (3 400 + 5 + 0) | same | 60 (54 + 6 + 0) |
| fine tier: fed samples > 0.1 mm | 256 (255 + 1 + 0) | same | 1 (0 + 1 + 0) |
| fine tier: deepest fed sample (mm) | 0.287 (arc) | same | 0.1125 (line) |
| fine tier sim, 0.1 mm: gouge (−min dev, mm) | 0.256 | same | 0.111 |
| fine tier sim: columns below −0.05 / −0.1 mm | 604 / 51 | same | 32 / 6 |
| fine tier: `check_collisions` strikes / body strikes | 0 / 0 | 0 / 0 | not run |

"same": on this board the F1 territory is identical (table above), so the
fine tier is the same op. Reference, arcs off: 68 935 moves, 2 659.1 s,
6 samples > 0.05 mm, 1 > 0.1 mm, sim gouge 0.111 (29 / 6 columns). The
one sample over 0.1 mm and the six columns are the open waterline vertex
at (68.500, 88.036); every arm has them.

40 x 40 mm knoll and wall fixture
(`arc_fit_burial_against_arcs_off_on_knolls_and_a_wall`, R1.0 ball,
0.2 mm simulation, 13 347 mask columns):

| arm | moves | arcs | burial max | on arcs | sim gouge | dev p0.1 | dev p1 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 0.05 (before) | 14 143 | 443 | 0.0560 | 0.0560 | 0.0477 | −0.0400 | −0.0212 |
| 0.015 (F4a) | 14 573 | 220 | 0.0346 | 0.0134 | 0.0214 | −0.0152 | −0.0110 |
| off | 15 044 | 0 | 0.0346 | — | 0.0214 | −0.0152 | −0.0107 |

**The cost of F4a.** At 0.015 the fitter makes fewer arcs, and more of
the path stays linear: the fine tier gains 13 785 moves and 330.8 s
(+16.4 %), the chain 444 s (+3.6 %). In return it removes 3 345 of the
3 405 fed samples deeper than 0.05 mm and every arc sample deeper than
0.1 mm, and the simulated gouge falls to the arcs-off value. Arcs off
would cost a further 305.7 s for 54 fewer samples over 0.05 mm.

### Moved pins

| pin | cause |
|---|---|
| `tier_islands_speck_weld`: the green arm now reads the default (no explicit bound) and asserts `DEFAULT_MAX_CLOSE_RAISES` = 0. `the_dial_cannot_raise_the_bound_past_the_ceiling` asserts default = 0 (was "= `MAX_CLOSE_RAISES`, no default change in this round") | F1 |
| MCP `max_regions_per_tier` description (`rs_cam_mcp/src/server.rs`, two specs), `rs_cam_viz/tests/snapshots/mcp_wire_surface.json`, and the GUI hover text of "Max islands / tier" | F1: by default the cap no longer raises the merge radius |

`tier_islands_i1` set the bound explicitly on 09-30 and did not move. No
assert was weakened. The instruments `tier_band_overlap_g_overlapfill`
(both step-0 tests now print the bound 3 and bound 0 arms explicitly),
`tier_arcfit_burial_f4` (prints links and cycle time per arm) and
`tier_fine_burial_sources_g_tierburial` (flag `arcs`) changed their
output only.
