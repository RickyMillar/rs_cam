# G-ADAPTPASSLOAD — 2D Adaptive's load measure admits ~2x the commanded width (design, 2026-09-26)

Status: **decided 2026-09-26 (§5) and implemented** (RESULTS at the end).
The §1 quadrature numbers are now asserted by the S2 unit tests in
`adaptive/search.rs`; one correction: the disk-area band is met from a
stepover of 3.6 mm, not 3.7.

## 1. The measures

- Target (ops/adaptive_shared.rs:10-14): f* = acos(1 − s/R) / 2π, an ANGLE
  fraction. s = 2, R = 3: f* = 0.19591; accept ceiling f*·1.05 = 0.20571
  (adaptive/search.rs:223, 228); in radial units
  `radial_woc_fraction_from_leading_arc` (:37) = 0.36264. Commanded s/D = 0.333.
- DiskArea (default; search.rs:25-64): material cells ÷ disc cells at the next
  position. With disc(cur) already cleared this is the crescent the step
  removes, ≈ (w/π)·fill for step L = R/2 (path.rs:261, 310). Steady side cut,
  R 3, L 1.5 (quadrature): s = 1 / 2 / 3 / 4 / 6 → 0.051 / 0.104 / 0.158 /
  0.211 / 0.315. The band [0.186, 0.206] is met at s ≈ 3.7-3.9 mm:
  **w ≈ 0.62-0.65, about 1.9x the commanded 0.33** (the quantitative form of
  finding F1, PROGRESS_HISTORY.md:1757). Thin material lets w rise further: a
  head-on bite of p = 1.5 mm reads 0.1955 (in band) at sideways spread 0.866.
- LeadingArc (search.rs:84-106): in-material share of the leading semicircle,
  x0.5. Exact on one-sided side cuts; head-on it admits w = sin 37° = 0.60; a
  sliver inside the new disc reads 0.
- Sim radial (dexel_stock/stamping.rs:1816-1824, 1392-1397): sideways extent of
  fresh material in the subsegment's midpoint disc ÷ D. A full slot reads
  ≤ 5.625/6 = 0.9375 on 0.5 mm cells.
- Physics (stored source: planning/UNIFIED_LOAD_MODEL_2026-06-18.md §4): h =
  fz·sinθ; mean tangential force ∝ ∫sinθ dθ = sideways extent / R; MRR =
  v_f·ap·extent. **The sim's w is the mean-force fraction of a slot and the
  normalised MRR.** Peak chip thickness saturates (sin 74° = 0.96 at w =
  0.36) and cannot define load. Define load as radial immersion w = sideways
  material extent ÷ D.

## 2. The 0.93 is real full-width contact

Its sources: slot-clearing lines (path.rs:320-356, default on,
operation_configs.rs:476; on the fixture, full slots at feed 1500 on both
levels); the out-of-band fallback `best_any` (search.rs:468, 508); gradient
mode (path.rs:612, no load read); contour-parallel residue loops (path.rs:1400);
the mop walk step (path.rs:1260); forced clear (path.rs:698) wipes 2R on the
planner grid with no motion, so planner stock runs ahead. Steady agent passes
sit at w ≈ 0.62-0.65 by design of the band itself. The trace separates slot
lines and entries (semantic_item_id, source_intent, move_index) but not agent
/ gradient / fallback / contour loop / mop (no Marker).

## 3. Options

- A. Default LeadingArc: partial; saved projects keep DiskArea (serialised);
  slots, mop, gradient, fallback and loops stay unchecked.
- **B. `SweptWidth` (recommended)**: the sim's radial model on the planner grid
  (w = max − min sideways coordinate of material cells in disc(next) ÷ D), one
  predicate `step_within_pass_load` with ceiling 0.3626 for agent accept
  (fallback becomes a refusal), gradient steps, contour loops (split over-cap
  spans), the mop walk (a capped search toward residue), and
  `feed_link_within_pass_load`. It applies to 2D whatever engagement_measure
  says; Adaptive3d keeps the historical rule (byte parity unmoved).
  Cycle time goes UP (up to ~x1.9 on bulk agent passes: the passes honour the
  commanded stepover).
- C. Hold load by feed (v_f · min(1, 0.3626/w)): smallest time cost but it does
  not stop ploughing as geometry; backstop only.

Plus trace markers `ResidueContour` / `ResidueMop` and gradient / fallback
counters (trace only).

## 4. Sentries

- S1 `a_clearing_cut_holds_the_pass_load_g_adaptpassload` (six-island
  fixture): every ClearingCut Linear/Arc sample ≤ 0.3626 + (0.5 + 0.5 +
  0.1)/6 = 0.546 (reuse the G-ADAPTLINKLOAD functions, moved to common).
  Red today (the mop walk: 1362 samples; slot lines ≈ 0.93; agent median
  predicted ≈ 0.62).
- S2 unit (search.rs, closed form): a head-on wall at p = 1.5 mm; DiskArea
  reads in band (documents the defect); no accepted step has SweptWidth >
  0.3626 + cell/D (red today: 0.866). Oracle: SweptWidth reads s/D ± cell/D
  for s ∈ {0.5, 1.5, 3, 6}.
- S3 (optional): per-kind peaks via the new markers.
- Guards: G-ADAPTLINKLOAD stays; adaptive3d byte parity and perf_golden
  unmoved.

## 5. Operator decisions (DECIDED 2026-09-26)

Decided: (1) option B approved; (2) slot-clearing lines off by default
(option a), the option kept; a project that saved `slot_clearing = true`
keeps it.

1. Approve B: 2D Adaptive passes hold the commanded stepover as real radial
   immersion; slower (up to ~1.9x on clearing), lower tool load.
2. Slot-clearing lines: off by default (Adaptive3d already passes false,
   clearing.rs:2843), or keep them as a declared full-slot load at the feed
   that holds the same mean force, 1500 x 0.3626 / 0.9375 ≈ 580 mm/min
   (limit ÷ the sim's slot reading).

## RESULTS (2026-09-26, implemented on a59b246b, not committed)

### What changed

- `search.rs`: `compute_swept_width` (sideways extent of material lattice
  points in disc(next), relative to the heading, / D; 1e-6 when the disc
  holds one cell), `StepMeasure`, `PassLoad` (`historical` for Adaptive3d,
  `swept_width` for 2D: target s/D, ceiling
  `radial_woc_fraction_from_leading_arc(pass_engagement_ceiling(f*))`),
  `step_within_pass_load`. Under the 2D load the direction search treats a
  step over the ceiling as no candidate; the old `best_any` fallback may
  only take a lighter step; no step -> `NoStep::OverPassLoad`, the pass
  ends ("over pass load"). Candidates are read at all four lattice points
  (`is_machinable_strict`).
- `path.rs`, all under `KeepDownLinks::WithinPassLoad` (2D only):
  - agent: gradient steps checked, else the capped search; forced clear
    removed (a stall limit of 3 passes replaces it); later passes enter
    where a helix fits (region inset by R + helix radius);
  - starter pocket: plunge at the medial point, Archimedean spiral at pitch
    min(s, R) out to R, then one lap (it was a full-slot circle);
  - contour-parallel loops and the boundary loop: `emit_capped_walk` cuts
    the spans that hold the load, re-entering past an over-load span by a
    load-held link or an entry;
  - replay of kept cuts after the short-cut filter: re-walked the same way
    (a cut can stand on stock a dropped cut had taken); slot lines and
    `ContourSpiral` wraps are replayed as they are;
  - mop: a capped direction search (72 headings, full then half step,
    scored on band error with a turn penalty), a retreat when none holds;
    patch starts by a stand-off link, a link, or an entry whose hole reaches
    the cell and whose helix fits; a start that removes nothing twice gives
    the cell up (it stays stock); the historical unseen wipes
    (`clear_circle(mx, my, R)` on an unmachinable cell, an abandoned plunge)
    are gone; a final capped wall pass (plus mop) takes the slivers along
    the region boundary;
  - `feed_link_within_pass_load` reuses the predicate;
  - re-entries stamp R + the entry dressup's helix radius
    (`AdaptiveParams::entry_helix_radius`, from `ExecutionContext`);
  - emission simplifies with segment distance (a reversal tip is kept).
- Straight-plunge / ramp entries (no helix radius): under the swept-width
  model no step out of a hole of the cutter's own size reads under half the
  diameter, so the steps inside the plunge hole are exempt
  (`PassLoad::departing`; trace counter `plunge_departure_steps`).
- Dressup: `OperationTransformCapabilities::helix_floor_lap` (Adaptive only):
  a helix ends with one lap at the target Z, also after a straight feed
  through air, so the hole is a clean R + r disc. Without it the last turn
  left a helical step up to one pitch high and the first cut out of the hole
  read 0.58. 2D Adaptive also sets `planner_applies_segment_merge`: a 0.3 mm
  merge after the planner moved cuts off its stock (0.65 on the fixture).
- Trace only: markers `StarterPocket`, `ResidueContour`, `ResidueMop`
  (semantic items); pass counters `gradient_steps`,
  `gradient_over_pass_load`, `fallback_steps`, `refused_over_pass_load`,
  `plunge_departure_steps`; exit reason "over pass load".
- Settings: `AdaptiveConfig::default().slot_clearing = false`. The field
  has no serde default, so every saved file carries it and loads as saved
  (an old `true` stays true); only new operations start off. The saved
  `engagement_measure` still loads and round-trips; 2D does not read it
  (registry help says so; the GUI never showed it). The slot-clearing box
  has a hover text naming the full-width load.
- Adaptive3d: every change is gated on `WithinPassLoad`;
  `adaptive3d_emission_byte_parity` is unmoved.

### Six-island fixture (Ø6, s 2, dpp 3 x 2 levels, sim 0.5 mm, planner order)

| measure | before (a59b246b) | after |
|---|---|---|
| ClearingCut samples | 8614 | 18778 |
| over 0.3626 + 0.1833 = 0.546 | 4254 | 0 |
| peak ClearingCut radial | 0.927 | 0.496 |
| median ClearingCut radial | 0.503 | 0.097 |
| per producer (over, peak) | contour 1868 / 0.927, mop 1362 / 0.920, slot 807 / 0.833, agent 205 / 0.891, starter 12 / 0.834 | contour 0 / 0.351, mop 0 / 0.496, agent 0 / 0.417, starter 0 / 0.253, wall pass 0 / 0.000, slot off |
| cutting time by producer | slot 19.0 s, agent 8.4 s, residue 135.4 s | agent 26.7, contour 4.5, mop 301.8, starter 2.6 s |
| retracts | 40 | 310 (mop 302) |
| fed Linking moves | 82 | 1736 (mostly air moves the air-cut pass relabels, no longer merged); peak link radial 0.51 -> 0.499 |
| material removed (sim) | 51.6 cm^3 | 50.0 cm^3 (pocket 50.4) |
| entry helices cutting a wall | 72 moves, to 1.81 mm | 0 |
| cycle time | 287.8 s | 614.4 s (2.13x) |

Before per producer is a59b246b with the markers added and nothing else
(same moves: 4254 / 8614, cycle 287.8 s on the pristine commit).

S2 (unit, `adaptive/search.rs`): swept width reads s/D within one cell / D
at s 0.5, 1.5, 3, 6; disk area on a steady side cut reads the §1
quadrature (0.051 / 0.104 / 0.158 / 0.211 / 0.315 within 0.005, fine
lattice) and meets the band only for s in 3.6-3.9; head-on p = 1.5: disk
area 0.1955 (in band), swept width 0.866. Facing that wall the 2D rule's
step stays within 0.3626 + 0.083; with the historical rule in its place the
same assertion is red (a step 35.6 deg off the normal, width 0.610).

### adaptive_property_harness (no helix, straight plunges), RED

Agent coverage before -> after: 0.993 / 0.983 / 0.993 / 0.990 / 0.991 /
0.995 / 0.987 -> 0.993 / 0.978 / 0.990 / 0.990 / 0.991 / 0.995 / 0.987;
agent p99 alpha/2pi 0.47-0.49 -> 0.23-0.26; agent plunges 1-4 -> 7-76.
`contour_spiral_dominates_agent` fails: the spiral's p99 (0.27-0.31) is now
worse than the load-held agent's, and the capped residue cleanup raises
spiral plunges to 4-10 on four shapes. The comparison was not loosened;
the operator decides whether `ContourSpiral` also holds the pass load.

### Open

1. `ContourSpiral` wraps and trochoids are not held to the swept-width load
   (the harness above).
2. Cycle time 2.13x and 310 retracts: the agent clears little (8 passes,
   468 steps, every one ends "no direction"); the capped mop does the bulk.
3. Straight-plunge and ramp entries: the exemption above; a ramp's own
   geometry is not modelled.
4. Explicit slot lines still cut at full width (declared).
5. The sim reads the end of a short move that turns hard as a wide cut; the
   mop's turn penalty (0.2) keeps the fixture under the bound.

## RESULTS, round 2 (2026-09-26, lead review: "passes honour the stepover")

### Diagnosis (evidence)

- Round 1 agent: 8 passes, 468 steps, every one ending "no direction". A
  dump of the 36 headings at each stop: pass 1 stopped at (-7.35, 7.24),
  0.45 mm inside the island's machinable ring, with stock only on the island
  side (readings 0.31 / 0.24 / 0.07 on headings the machinable test
  refused, 0.00 on every legal one); passes 2-8 stopped on step 1 at
  corner entries (e.g. (-55.2, -35.2)) whose nearest stock lay 4.83 mm away,
  beyond one pass step from the 4.8 mm helix hole. The band was not the
  cause: in pass 1 the planner read 397 of 410 steps in band.
- The band floor was nonetheless a fifth of one lattice quantum (0.017 of
  D against cell / D = 0.083): now derived, floor = (s - cell) / D, the
  lowest reading a true side cut at the stepover can give on the lattice
  (the S2 oracle pins [s - cell, s]).
- After the agent could carry on (below), three more defects showed:
  gradient steps walked cut stock (a centreline walk that removes nothing),
  the short-cut filter and the orphan-link filter dropped stamped cuts and
  links so replay met stock generation had cut (98 replay splits), and the
  mop re-entered once per wall sliver (290 helix entries for 0.04 % of the
  volume) because the lattice machinable read kept the cutter a cell off
  the walls.

### Changes

- Agent: when no step holds the load, a frontier hop (a load-held
  keep-down link to a stand-off one cell outside the cutter's reach of the
  nearest stock; stand-off directions `ceil(2 pi (R + cell) / step)`, the
  finest set whose neighbours lie a pass step apart) and the pass carries
  on; a later pass starts by the same hop before any boundary entry. A hop
  after which nothing was removed gives its cell up (it stays stock).
- Gradient steps must cut stock and hold the load, else the capped search.
- Cleanup under the 2D rule keeps every cut and every link (no short-cut
  filter, no orphan filter): replay then reproduces generation.
- The machinable test is exact (`Polygon2::contains_point` on the region),
  not the lattice read (both the 4-point and the 1-point reads were wrong
  by up to a cell).
- A re-entry stamps the helix hole only where the dressup helixes (stock
  within R of the entry), as `rapid_to_entry_top` decides.
- Wall pass before the mop.
- Mop scoring: the agent's own heading-change weight
  (`search::HEADING_CHANGE_WEIGHT`, 0.03, historical) replaces the tuned
  0.2 turn penalty and the 0.05 toward-residue term.
- ContourSpiral: its wrap trigger is `step_within_pass_load` under the 2D
  rule and its cut is re-walked in replay, so it holds the same load.

### Six-island fixture

| measure | a59b246b | round 1 | round 2 |
|---|---|---|---|
| samples over 0.546 | 4254 / 8614 | 0 / 18778 | 0 / 20098 |
| peak ClearingCut radial | 0.927 | 0.496 | 0.490 |
| median, all samples | 0.503 | 0.097 | 0.008 |
| median / p90, samples that cut | - | - | 0.029 / 0.295 |
| median by removed volume | - | - | 0.261 |
| median / p90 of each cutting move's peak | - | - | 0.274 / 0.361 |
| volume by producer | slot, agent, residue | mop bulk | agent 98.4 %, mop 0.8 %, starter 0.5 %, wall 0.2 % |
| retracts | 40 | 310 | 142 (agent 34, mop 106) |
| cycle time | 287.8 s | 614.4 s | 612.0 s (2.13x) |
| ClearingCut into a wall | 6 moves, 0.01 mm | - | 18 moves, 0.015 mm |
| entry helices into a wall | 72, 1.81 mm | 0 | 0 |

Why the per-sample medians sit far under s/D: a sample reads the fresh
stock in one 0.47 mm sub-segment's disc, and the sim's coverage gate drops
cells the stamp does not cover to 95 %; along a steady side cut most
samples read a sliver of the band, and air samples (links inside cuts,
re-traversal) read 0. The per-move peak (0.27 median, 0.36 p90) is the
width a move cut; the planner read 0.25-0.36 on 93 % of agent steps (the
band). The lattice reads a true cut at s as s - [0, cell], and the sim's
own lattice the same, so a cut at the stepover reads about 0.29-0.33.

Cycle time: the agent's ClearingCut time is 332 s; the pocket area
(120 x 80 - 6 pi 8^2 = 8394 mm^2) at a 2 mm stepover is 4197 mm of path per
level, 336 s for two levels at 1500 mm/min: the cutting is at the stepover,
and cutting alone takes longer than the whole old job (the old passes were
about 0.62 wide). The other 280 s: helix entries 73 s, links 141 s,
retracts 31 s, mop and wall passes 26 s. The operator's expected ~1.9x is
the cutting ratio; the overhead is entries and links.

### adaptive_property_harness

Rewritten as `both_strategies_hold_the_pass_load`: an independent
swept-width oracle, the same load bars for both strategies (p99 <=
ceiling + (planner cell + oracle cell + tolerance) / D = 0.537; spiral
p99 <= agent p99 + one oracle cell / D) and the coverage bars. The harness
now models the product's default entry (helix 0.3 D with the floor lap).
Load: all shapes pass (p99 0.33-0.41; slot-class exempt). Coverage is RED
on star_a (spiral 0.9686, agent 0.9723) and star_b (agent 0.9743) against
0.975: the uncut cells sit 0.5-1.5 mm from the wall in star tips narrower
than a helix-safe entry (2 (R + r)), where no step within the load
reaches either. Before, a plunge anywhere reached them (and the helix
dressup cut the wall there). Bar kept; decision needed.
The spiral travel contract (<= 3 plunges, rapids <= 5 %) moved to
`contour_spiral_travel_contract`, ignored with its reason: the trochoid
inserts meet the frontier across a chord of a pitch-deep cap (0.8 of D at
pitch 0.6 s), over the swept-width ceiling, so the replay splits them;
44-70 plunges.

### Straight-plunge entries

S1 reports `exempt` (samples after a stock-cutting straight plunge, within
R of it): 0 on the helix fixture (asserted). Instrument
`a_straight_plunge_entry_is_exempt_only_leaving_its_hole` (entry None):
1580 exempt samples, peak 0.72, and 14 non-exempt over the bound (peak
0.92), cycle 1514 s. Open.

## RESULTS, round 3 (2026-09-27, lead decisions of round 2)

### 1. Helix containment and wall cuts

- The entry dressup shrinks a 2D Adaptive helix to the largest radius whose
  circle stays inside the machinable region (`dressup::contained_helix_radius`
  against `DressupContext::entry_containment`); where none fits it becomes
  a ramp at `DRESSUP_RAMP_ANGLE_DEG`. The planner stamps the same fitted
  hole (`LinkLoad::entry_hole`, same function, same region) and enters only
  where a helix fits, so no ramp is emitted on the fixtures.
- Scope: gated to 2D Adaptive. `entry_containment` is set only for an op
  with the `helix_floor_lap` capability (Adaptive alone); every other op
  passes `None` and its entries are unchanged (their sentries in the list
  below are green).
- One region builder, `adaptive::tool_centre_pieces`: the part offset by R
  on the arc-carrying cascade, arcs flattened once at
  `FlattenPolicy::UNTOLERANCED_MM` (10 µm, points on the true arc), every
  piece. The planner (`ToolCentreRegion`) and the dressup read it. Before,
  the planner read only the first piece of `offset_polygon`.
- A wall-cut check measured against the part polygon with arcs followed
  (S1 `wall_cuts`, now asserted) found ClearingCut 28 moves to 0.081 mm and
  Linking 6 to 0.041 mm. Causes and fixes: (a) a straight step between two
  legal ends cuts an island's inset arc by its sagitta: a move is now legal
  only when the whole segment is inside the region
  (`ToolCentreRegion::contains_segment`: no ring edge crossed, midpoint
  inside) in the search, gradient, mop and link steps; (b) emission's
  0.1 mm simplification chord; (c) the arc-fit dressup bulging a link run
  into the wall: `fit_arcs_within` keeps an arc only when it lies inside
  the containment (read at a 1 µm sagitta). After: deepest 0.0063 mm
  (EntryHelix), ClearingCut 0.0047, Linking 0.0011, all under the region's
  own flattening (0.010 mm, the sentry's derived epsilon).

### 2. ContourSpiral travel claim

Retired. `contour_spiral_travel_contract` stays ignored with the reason
"retired claim"; the spiral module doc and the `PathStrategy2d` doc no
longer state one plunge per region; FEATURE_CATALOG never stated it. The
spiral re-enters 56-119 times on the harness shapes.

### 3. Straight-plunge entries: fixed at the root

Diagnosis, each over sample read on the exact geometry
(`print_exact_width_along_move`: a 0.05 mm lattice cut by every earlier
move as a capsule, arcs followed, no coverage blend):

- Real, lattice-hidden slivers. Two thin tips of stock either side of a
  disc (the exact width 0.96 of D on move 2207 of the helix run): the
  centre lattice (0.5 mm) held no point in either, so the planner read the
  step in band. Fix: the grid keeps a fringe under the 2D rule (4 x 4
  sub-points per cell; a cell whose centre is cut but whose sub-points
  still hold stock is `CELL_FRINGE`), and a step is also held to
  `ceiling + cell / D` on the swept width read over those sub-points
  (`measure_step`, `compute_swept_width_with_slivers`). The extra cell is
  the quantum the centre reading already carries (the band floor's); the
  sliver reading within it is the edge of the cut, over it is stock the
  lattice hid. Steps stamp the capsule they sweep (`clear_segment`), not a
  disc per step, so the sub-points hold no scallop the machine does not
  leave.
- Emission slivers. The emitted chord (simplified to the 0.1 mm operation
  tolerance) left up to 0.1 mm the planner had stamped; a later mop step
  crossed it at 0.65 of D (move 3313). Fix: under the 2D rule emission
  drops only collinear points (`MIN_EMITTED_SEGMENT_MM`), so the emitted
  path is the walked path.

`a_straight_plunge_entry_is_exempt_only_leaving_its_hole` is now a real
sentry (not ignored, about 25 s): 0 non-exempt samples over the bound of
14172, peak 0.494; exempt 4700 (peak 0.920); no wall cut.

Why the None-entry job is slower (992.8 s here, 1514 s in round 2, against
705 s with the helix): not inherent to a straight plunge. 2D Adaptive
emits a plunge as a rapid to safe Z and a feed to depth at the plunge rate
(500 mm/min): 13-16 mm of feed, 1.6-1.9 s per entry, most of it in air,
and with entry style None no dressup shortens it (the helix dressup rapids
to `entry_clearance_mm` over the stock first). EntryPlunge time 355.7 s,
retracts 99.2 s, links 194.9 s, cutting 343.1 s. A stock-aware plunge
(rapid to the clearance over the stock, as the helix does) would save
about 1.2 s per entry.

### Six-island fixture (helix entries, the sentry)

| measure | round 2 | round 3 |
|---|---|---|
| samples over 0.546 | 0 / 20098 | 0 / 19632 |
| peak ClearingCut radial | 0.490 | 0.422 |
| median / p90, samples that cut | 0.029 / 0.295 | 0.139 / 0.275 |
| median by removed volume | 0.261 | 0.209 |
| median / p90 of each cutting move's peak | 0.274 / 0.361 | 0.256 / 0.332 |
| volume by producer | agent 98.4 % | agent 99.2 %, starter 0.6 %, mop 0.2 % |
| retracts | 142 | 152 (agent 72, mop 78, wall 2) |
| fed links | - | 1806 |
| cycle time | 612.0 s (2.13x) | 705.0 s (2.45x of 287.8 s) |
| wall cut (any move, over 0.010 mm) | 18 moves, 0.015 mm | 0 (deepest 0.0063) |

Time, 705.0 s: ClearingCut 345.8 (agent 335.1), EntryHelix 187.2 (152
entries), Linking 118.7, Retract 33.1, EntryPlunge 20.3. Against round 2
(612 s): the sliver ceiling refuses more links and entries (152 against
122 with it off, which reads 615.8 s and fails both sentries).

### adaptive_property_harness

Coverage reachability was wrong: it read the distance transform of the
NON-machinable cells (`distance_transform_2d` measures to the nearest
`true` cell), so "reachable" was the band within R of the unreachable
set, star tips included, and the interior was not read. On star_a 43 of
the agent's 110 uncut cells lay over R from any legal centre (up to
4.6 mm). Now: legal = exact wall distance >= R (the harness's own
`wall_distance`), reachable = within R of a legal lattice centre. Bar
unchanged (0.975). With the old reading the exact planner scores star_a
agent 0.9661 and star_b agent 0.9738 (the 0.2 mm emission chord of the
harness's tolerance used to reach further into the tips).

| shape | spiral cov | agent cov | spiral p99 / max width | agent p99 / max width |
|---|---|---|---|---|
| square60 | 1.0000 | 1.0000 | - | - |
| star_a | 0.9999 | 0.9964 | 0.339 / 0.463 | 0.367 / 0.405 |
| star_b | 0.9991 | 0.9969 | 0.394 / 0.467 | 0.371 / 0.446 |
| l_shape | 0.9940 | 0.9990 | 0.315 / 0.394 | 0.372 / 0.445 |
| u_shape | 0.9998 | 0.9957 | 0.315 / 0.393 | 0.372 / 0.408 |
| annulus | 0.9996 | 0.9991 | 0.395 / 0.410 | 0.384 / 0.439 |
| narrow_slot (slot-class, exempt) | 1.0000 | 1.0000 | 0.459 / 0.580 | 0.466 / 0.580 |

Load bar 0.537 (p99); every shape passes.

### 4. Overhead levers (not this round)

1. Helix entries, 187 s of 705: 152 entries, median fitted arc radius
   0.20 mm, 0.71 s per entry median (to 4.2 s). Most re-entries (mop and
   frontier) sit beside a wall where the helix shrinks to nearly a plunge
   and runs accel-limited on a 0.2 mm radius. Lever: re-enter where the
   full helix fits (the agent's `entry_mask`) and link from there, or link
   instead of re-entering; up to about 100 s.
2. Links, 118.7 s of 705: 1806 fed links at the cutting feed through
   cleared stock. Lever: a link feed above the cutting feed where the link
   reads no stock (the link dressup's `link_feed_rate`); at 2x the cutting
   feed about 60 s.

### Open

- `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` is RED: 1 live
  rapid strike (move 4514, a vertical descent to -2.5 at (-5.68, 25.79),
  level 2). The exact geometry holds no level-1 stock in its footprint
  (0 of 11283 points, 0.05 mm lattice); at a 0.25 mm simulation cell the
  strike is gone (0 strikes). It is the simulation's coverage blend at the
  fixture's 0.5 mm cell (a partly covered dexel keeps 1 - f of its height
  per stamp). Green at a59b246b; the path moved.
- `arcfit_intent_key_cost_f1` is red at a59b246b too (arc_raster moves 148
  against 72 pinned; an Adaptive3d fixture this package does not reach).
- ContourSpiral's corner blend (0.3 R, G2/G3) is not replayed by the
  planner: the emitted spiral cuts less at its corners than the planner
  stamped. Its harness load and coverage pass.

## RESULTS, round 4 (2026-09-27)

### A. The rapid-lift strike (COVERAGE_UNION_PLAN Step 0)

Move 4514, a Z-only rapid from 10 to -2.5 at (-5.676, 25.791) on level 2.
Replayed (moves 1..4513, fresh 0.5 mm stock) it strikes; per-move and
whole-range replays of the cells agree bit for bit. First flagged sample
12/13 at z -1.538: HIGH only (high 0.0000, tau 1.1036, 0.4349 under
high - tau); low -5.8654 (eps 1.4e-6), 4.33 mm under it, no flag. High
argmax cell (137, 123) at (-3.5, 23.5): centre 3.1593 from the axis (near
point 2.806 < R), conservative_top 0.0, ray_top -3.183. No stamp covered
it whole (at most 13/16, the helix 1209-1216 at z 0.47 to -3); the union of
every mask misses sub-samples 2, 3 and 7, 0.04-0.15 mm off the island. The
cell holds island (0, 16) itself (0.95 % of its area, to z 0): the
conservative top is true. The exact distance from the axis to the island
polygon is 3.3173: the cutter clears it by 0.317 mm. Class: the high
channel's rim over-read (half-diagonal plus dilation) against a real wall,
1.54 mm over the tip (> tau). Not union residue (M is not FULL); the
coverage-union fix would not remove it. Lead: COVERAGE_UNION_PLAN not
triggered.

Sentry change (lead decision): `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift`
now judges every rapid on exact geometry (walls: the part polygons to the
stock top, exact segment distance; pocket: a point stands above the tip
unless an earlier fed move's capsule reached it at or below the tip, found
by a quadtree split to 10 um; slack the region flattening, 10 um). 455
rapids, 0 enter material. The simulation's flags must all be oracle-clear,
their count pinned at 1 (move 4514, 0.317 mm). Injected: a descent 0.5 mm
under the level-1 floor reads -0.5; a descent moved to 2.5 mm from a wall
reads -2.5 (the pocket there) - both red.

### B. Cycle-time levers

1. Re-entry placement: NOT implemented. My round-3 lever was wrong. At a
   fixed pitch (1 mm per turn) a helix's path is turns x 2 pi r, so a
   squeezed helix is SHORTER, and its steep moves run at the plunge rate
   (feedopt's plunge cap). Measured by largest arc radius:

   | radius | entries | time | per entry | descent per entry |
   |---|---|---|---|---|
   | < 0.5 | 84 | 35.3 s | 0.42 s | 2.95 mm |
   | 0.5-1.0 | 4 | 9.0 s | 2.24 s | 3.5 mm |
   | 1.0-1.7 | 14 | 42.3 s | 3.02 s | 3.5 mm |
   | >= 1.7 (full 1.8) | 50 | 100.7 s | 2.01 s | 3.5 mm |

   Moving the 84 squeezed entries to full helixes would add about
   84 x (2.0 - 0.42) = 130 s. The time lever is fewer entries, not bigger
   ones. Whether a near-plunge helix is acceptable for the tool is a
   separate question (lead).
2. Air links: NOT implemented; the codebase already has the policy and it
   runs. The feed-optimisation dressup (default on) gives a move reading
   under `air_cut_threshold` the operator's `feed_max_rate` (3000), ramped
   by `feed_ramp_rate` (200 mm/min per mm). On the fixture 510 fed link
   moves already run at 3000 and most others are on the ramp (fed links
   about 73 s, XY rapids 45 s). A planner-side air feed would be a second
   air policy.
3. Stock-aware straight plunge: done. `dressup::EntryStyle::Plunge` =
   `rapid_to_entry_top` (the helix's first half, on the op's own replayed
   stock) + one straight feed; run under entry style None for ops with the
   new `stock_aware_plunge` capability (2D Adaptive only). The XY is the
   plunge's own, so the planner's hole is unchanged: the None-entry sentry
   reads the same samples (exempt 4700, 0 over of 14172, peak 0.494).
   None-entry cycle 992.8 -> 882.0 s; EntryPlunge 355.7 -> 228.9 s.
   `the_fresh_split_target_is_the_stock_top_not_a_pinned_top`
   (G-PECKSPLIT) now accepts the entry dressup's clearance (stock top +
   0.5, the helix rule) as well as the descent pass's (+ 2.0); it still
   refuses anything under the real stock top + 0.5.

Six-island fixture, helix entries (the default): unchanged, 705.0 s
(ClearingCut 345.8, EntryHelix 187.2, Linking 118.7 incl. XY rapids 45.4,
Retract 33.1, EntryPlunge 20.3); the round-3 table stands.

None-entry fixture, time by intent (s):

| intent | before | after |
|---|---|---|
| ClearingCut | 343.1 | 343.1 |
| EntryPlunge | 355.7 | 228.9 |
| Linking | 194.9 | 210.7 |
| Retract | 99.2 | 99.2 |
| total | 992.8 | 882.0 |

(The before split is round 3's; Linking now carries the rapid down to
the clearance, tagged Linking by `rapid_to_entry_top`.)
