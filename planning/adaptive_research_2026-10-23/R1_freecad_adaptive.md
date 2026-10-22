# R1 — FreeCAD Adaptive: answers with source lines

| Item | Value |
|---|---|
| Source | `https://raw.githubusercontent.com/FreeCAD/FreeCAD/be69b03edd4e4b754410424919e974944f333426/src/Mod/CAM/libarea/Adaptive.cpp` (also `Adaptive.hpp` and `src/Mod/CAM/Path/Op/Adaptive.py` from the same commit) |
| Commit | `be69b03edd4e4b754410424919e974944f333426`. The GitHub API gives it as the last commit that changed `Adaptive.cpp` (author Jacob Oursland, 2026-09-20). The last commit that changed `Adaptive.py` is `e850a9b1342c74a3468c2490c42ffff2dd4e532b` (2026-09-05), so the copy at `be69b03e` is that version. |
| Fetch date | 2026-10-23 |
| Licence | `SPDX-License-Identifier: LGPL-2.1-or-later` in all three files (`Adaptive.cpp:1`, `Adaptive.hpp:1`, `Adaptive.py:1`). rs_cam is MIT OR Apache-2.0. This note takes ideas only. It contains no code from the source. |
| Line count | `Adaptive.cpp` 4626, `Adaptive.hpp` 291, `Adaptive.py` 2003 |
| Local copy (untrusted, not executed) | `/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/a5fdffa7-9517-40c1-b382-2e96c28e2e27/scratchpad/r1_freecad/` |

Citation form: `cpp:N` = `Adaptive.cpp` line N, `hpp:N` = `Adaptive.hpp`, `py:N` =
`Adaptive.py`. Each citation is to the commit above.

Units. The C++ works in integer Clipper units. `scaleFactor` is
`MIN_STEP_CLIPPER / tolerance / min(1, stepOverFactor·D)` units per mm (`cpp:1847`,
`MIN_STEP_CLIPPER` = 48, `hpp:275`). The UI stepover `StepOverPercent` is a percent of
the diameter: `stepOverFactor = 0.01·StepOverPercent` (`py:451`), and the helix spiral
uses `D·StepOverPercent/100` as its step (`py:262`). In this note, s = `stepOverFactor`,
R = tool radius, D = 2R.

**Trap in the source.** The internal `stepOverScaled` is `R·s` (`cpp:1856`). That is
half of the UI stepover width s·D. Every constant below that uses `stepOverScaled` is in
units of s·R. This note writes the unit each time.

---

## 1. Who wrote it, and when?

- The header of `Adaptive.cpp` and `Adaptive.hpp` says "Copyright (c) 2018 Kresimir
  Tusek" (`cpp:4`, `hpp:4`). The header names one author and one year.
- `Adaptive.py` names a second holder: "Copyright (c) 2019-2021 Schildkroet" (`py:5`).
- The header does not name later authors. Some parts are clearly later work (the
  Clipper2 passages, for example `cpp:2150-2153` "This code will go away soon when we
  convert all of adaptive to clipper 2"). The header does not say who wrote them. The
  last commit to the file is by Jacob Oursland (GitHub API, header block above).
- Neither C++ file contains the word "Freesteel" (grep for `Freesteel|freesteel` is
  empty). The only literature reference in the file is a future-work note in
  `ResolveLinkPath` that cites DOI 10.1002/net.3230140304 (`cpp:2808-2819`). The source
  does not support the Freesteel lineage that `research/02_algorithms.md` states.

## 2. How does it grow a pass?

It grows step by step, from the cleared area. It does not offset the boundary.

- `Execute` (`cpp:1831`) splits the tool-centre region (`toolBounds`, the input inset by
  R plus the finishing allowance, `cpp:2374-2415`) into connected components. It calls
  `ProcessPolyNode` once for each component (`cpp:2446-2567`).
- First entry of a component: `getEngagePoint({})` (`cpp:4137`). If no cleared area
  exists to engage from, `FindEntryPoint` (`cpp:2580`) helixes down. It offsets the
  uncleared tool-centre region inward in steps until the offset is empty. It takes the
  centroid of the last non-empty offset (`cpp:2605-2624`), that is, a point near the
  deepest point of the region. Then it finds the largest helix that fits by bisection
  between `helixRampMinDiameter` (at least D/8, `cpp:1865`) and the target
  (`cpp:2655-2675`).
- The pass loop is `for (long pass ...)` (`cpp:4176`). The point loop inside it is
  `for (long point_index ...)` (`cpp:4221`). Each point calls the lambda
  `iterateNextStep` (`cpp:3596`, call at `cpp:4229`).
- One step: the step length is fixed for the step. The heading is the previous heading,
  averaged over 3 points (`gyro`, `DIRECTION_SMOOTHING_BUFLEN` = 3, `hpp:280`,
  `cpp:4227`), and turned by an angle in [-45°, +45°] (`cpp:1174-1175`). The angle is
  chosen so that the cut area matches the target (answer 3).
- The pass ends at the first step that fails (`cpp:4283-4286`). A step fails when:
  - the cut is mostly conventional (`cpp:3880`),
  - the cut area is less than 1 unit² (`cpp:3881`),
  - the step overcuts (`cpp:3873-3876`), or
  - 180 rotations cannot bring the step back inside the tool-centre region
    (`cpp:3838-3858`).
- After each pass, `AppendToolPath` adds the pass and a lead-out (`cpp:4309`). Then
  `getEngagePoint({toolPos})` picks the next start (`cpp:4328`). The candidate starts
  are points on the frontier of the cleared area: the cleared polygon offset by -(R + 2)
  then by +(2 + protrusion) (`cpp:3944-3953`), clipped to the tool-centre region
  shrunk by 2 units (`cpp:3984`). The candidates are sampled every `MIN_STEP_CLIPPER`
  along that frontier (`cpp:3986-4026`). A candidate is valid only when
  `initToolDir` finds a heading whose first step does not fail (`cpp:3888-3925`).
- The protrusion is derived: the circle-segment area that one minimum step must cut
  at the target, solved for the angle with a Taylor expansion (`cpp:4101-4109`). It is
  capped at the finishing allowance.
- The candidates are sorted by straight distance (`cpp:4031-4038`). A candidate gets a
  link only when its straight distance is below the best cost so far (`cpp:4046`). Each
  is scored by the length of its full link plus lead-in. A link that is not clear adds 10000 mm
  (`cpp:4045-4085`). The cheapest wins.
- The region is complete when no candidate is valid (`cpp:4335-4359`).

## 3. How does it measure engagement, and against what target?

**Quantity: area cut per unit of travel.** It is exact polygon geometry, not a grid.

- `CalcCutArea(c1, c2, cleared)` (`cpp:1329-1587`) returns the area inside the new tool
  disc at c2 that is outside the old disc at c1 and outside the cleared polygons. The
  method is in the comment at `cpp:1290-1328`. It rotates the step to +y. It collects
  every x where a polygon vertex, a polygon–circle crossing or a circle tangent occurs.
  On each x interval it sorts the crossings on a vertical line. It integrates the
  exact circle segments and polygon trapezoids where the point is inside c2 and outside
  all other shapes.
- It also returns the part of that area on the left of the travel (`xtest < c2.X`,
  `cpp:1536`, `cpp:1576`). This is the conventional part. A step with a conventional
  fraction of 0.51 or more is rejected (`cpp:3643`, `cpp:3799`, `cpp:3880`). The method
  is climb-only by a hard rule.
- `areaPD = area / stepScaled` (`cpp:3804`). The error is `areaPD - optimalCutAreaPD`
  (`cpp:3805`).

**Target.** `optimalCutAreaPD = 2·s·referenceCutArea / R` (`cpp:1895`). The variable name
`slotCut` is misleading: `referenceCutArea` is the area of the tool disc minus the same
disc shifted by R/2 (`cpp:1887-1894`), a crescent. Derivation of what the target means:

- The overlap (lens) of two radius-R discs at distance d = R/2 is
  2R²·acos(d/2R) − (d/2)·sqrt(4R² − d²) = 2.1521·R².
- The crescent is πR² − 2.1521·R² = 0.98948·R².
- `optimalCutAreaPD` = 2·s·0.98948·R²/R = 0.98948·s·D.

So the target is the area per mm of a straight side cut of radial width 0.98948·s·D,
that is, the UI stepover width to about 1 %. (Computed with `python3 -I`, 2026-10-23.)

**Tolerance and search.**

- The step is accepted when |error| < 5 % of the target (`AREA_ERROR_FACTOR` = 0.05,
  `hpp:277`, `cpp:3639`, `cpp:3808`) and the cut is not conventional.
- The search uses at most 30 iterations (`MAX_ITERATIONS`, `hpp:276`):
  - iteration 0: the average of the last 3 accepted angles (`cpp:3638`, `cpp:3655-3657`);
  - iteration 1: −45°, the most engagement (`cpp:3659-3661`);
  - iteration 2: +45° or an interpolation (`cpp:3663-3671`);
  - later iterations: a secant interpolation on the angle, with the fraction clamped
    to [0.2, 0.8], a compromise between bisection and pure secant (`cpp:1246-1265`).
- When no area is found by iteration 3, it expands the cleared polygon, finds the
  nearest uncleared point in a forward ±45° triangle and aims at it
  (`cpp:3673-3718`). If that also cuts nothing, the step has zero area
  (`cpp:3719-3725`).
- The `Interpolation` class keeps one bracket point on each side of zero error and
  prefers climb points (`cpp:1171-1281`).

**Sampling.**

- The step length is `MIN_STEP_CLIPPER` (48 units) within max(R/4, 8·48 units) of the
  boundary or the entry. Elsewhere it is 48/|angle| units. It is clamped to
  [48 units, min(R/4, 8·48 units)] (`cpp:3615-3633`). At the scale of `cpp:1847`,
  48 units = `tolerance`·min(1, s·D) mm.
- The overcut safety test fails a step when area > step·target **and**
  areaPD > 2·target (`cpp:3872-3876`).

**Caveat: the cleared polygon is updated lazily.** The tool path of the pass collects
in `toClearPath`. It goes into the cleared polygon only when:

- the heading turns more than 45° from the last expansion (`cpp:4246-4252`),
- iteration 3 of a search runs (`cpp:3675`), or
- the pass ends (`cpp:4289-4292`).

Between those events, a step is measured as "new disc minus the previous disc minus
older cleared area". It is not measured against the full swept path of the pass.

## 4. How does it decide a link?

`FindLinkPath` (`cpp:3097-3266`) builds a lead-in and a link. The calling code picks the
link with the lowest cost (`cpp:4045-4085`).

- **Lead-in.** `MakeLeadPath(leadIn = true)` (`cpp:2982-3095`) walks backwards from
  the pass start toward a "beacon" point. The beacon is offset sideways by
  max(1.5·min(s·R, linkDistance/2), 8·48 units) (`cpp:3119-3143`). Each step must pass
  `IsAllowedToCutTrough` (`cpp:3037`). The walk ends when it is inside the cleared area
  shrunk by R + step (`cpp:3060-3070`). The path is reversed into a lead-in.
- **Keep-down link.** `ResolveLinkPath` (`cpp:2801-2980`) links the previous point to
  the start of the lead-in.
  - It tests the straight segment with `IsClearPath` (`cpp:2912`).
  - When the test fails, it bisects the segment. It moves the midpoint
    perpendicular in steps of `scanStep` (between 0.01 mm and 0.1 mm,
    `cpp:2828-2834`) and tries the side farther from the cleared boundary first. Then
    it queues both halves (`cpp:2922-2971`).
  - It fails when the total length is more than `keepToolDownDistRatio` × the direct
    distance (`cpp:2914-2916`, `cpp:2968-2970`). The default ratio is 3.0 (`hpp:124`,
    `py:405`).
  - It also fails on a self-intersection (`cpp:2875-2889`), a segment shorter than
    4 units (`cpp:2923-2926`), 10000 points (`cpp:2866-2871`) or a time limit
    (`cpp:2852-2864`).
- **The test along the link.** `IsClearPath` (`cpp:2721-2739`) sweeps the tool disc,
  enlarged by a clearance, along the segment. It subtracts the cleared polygon. The
  link is clear when the remaining area is less than 1 unit². The clearance is s·R. At
  the two ends the test starts 2·s·R inside the segment (`cpp:2837-2842`,
  `cpp:2893-2910`). When 2·s·R is more than half the direct distance, the end offset is
  half the distance and the clearance is 0.
- **The three outcomes.**
  - `ResolveLinkPath` succeeds → `mtLinkClear` (`cpp:3168-3169`). It is smoothed
    (`cpp:3237-3239`). `Adaptive.py` emits it as a **G0 at `passEndDepth +
    LiftDistance`** (`py:306-316`). `stepUp` is max(`LiftDistance`, 0) (`py:181`). So
    with `LiftDistance` = 0 the G0 runs at cut depth through cleared area. The link is
    a rapid, not a cut. But the planner adds it to the cleared polygon at the start of
    the next pass: the pass loop expands `mtCutting` and `mtLinkClear` parts
    (`cpp:4186-4197`). Two other places expand `mtCutting` parts only: `getEngagePoint`
    (`cpp:4119-4130`) and the finishing loop (`cpp:4547-4558`).
  - `ResolveLinkPath` fails and the gap is shorter than 2·s·R and
    `IsAllowedToCutTrough` passes → `mtCutting`, a straight feed at depth
    (`cpp:3206-3223`). `IsAllowedToCutTrough` (`cpp:2741-2799`) walks the segment in
    steps of min(0.5·s·R, 8·48 units). It fails when a step cuts more than 1.5 × step ×
    target area (2 × that factor for a short step), or when a point leaves the
    tool-centre region.
  - Otherwise → `mtLinkNotClear`. `Adaptive.py` emits it as a full retract to
    `ClearanceHeight` and a G0 (`py:318-323`).
- **Choice.** The cost is the link length plus the lead-in length in mm, plus 10000 for
  each `mtLinkNotClear` part (`cpp:4063-4077`). Any keep-down route beats any retract.
- **Lead-out.** After each pass, `MakeLeadPath(leadIn = false)` walks from the pass
  end toward a beacon. It updates the cleared area as it goes (`cpp:3038-3048`,
  `cpp:3296-3362`).
- **Return.** At the end of a region, the return to the entry point is clear or not
  clear by `IsClearPath` (`cpp:4562-4566`).

## 5. Concave corner, and a slot narrower than 2R

**Concave corner.** The source has no code for corners.

- Near the boundary, the step drops to the minimum step (`cpp:3616-3619`).
- When the chosen step leaves the tool-centre region, it turns the heading toward the
  boundary direction in 2° increments, at most 180 times. Then it computes the area
  again (`cpp:3826-3870`).
- When the step then overcuts, is conventional or cuts nothing, the pass ends. The next
  pass starts from the frontier of the cleared area (answer 2).
- So a corner is cleared by more ordinary passes. Each pass starts from a frontier
  engage point and links by the routed, lifted link of answer 4.
- The finishing profile (answer 6) walks the corner of the tool-centre region offset
  outward by the allowance, with round joins (`cpp:2500-2504`).

**Slot narrower than 2R.**

- The tool centre has no position in such a slot. The tool-centre region is
  disconnected there. Each part is a separate component with its own entry
  (`cpp:2435-2440`).
- The source states: "Thin channels that just barely/exactly fit the tool will not be
  cut ... such regions must be cut by a slotting operation instead" (`cpp:2475-2482`).
- A component needs a helix of at least D/8 to fit (`cpp:1865`, `cpp:2655-2658`).
  Otherwise the result has `StartPointNotFound` set (`cpp:2698-2701`) and the
  component is not cut.
- **Inference, not stated in the source:** in a channel wider than 2R but too narrow
  for a side cut, a step forward cuts close to the full width D per mm. That is about
  1/s times the target. For s < 0.5, the overcut test (`cpp:3873`) fails the step and
  the pass ends. The channel stays uncut, and `UnclearedAreaRemains` can be set
  (`cpp:4356`).

## 6. What residue does it leave, and how does it clean it up?

- **Wall allowance.** The interior clearing stays off the input walls by
  `finishPassOffsetScaled` = s·R/10 (`cpp:1870`, `FINISHING_THICKNESS_SCALE` = 1/10 at
  `hpp:285`). Note: the comment at `hpp:240-243` says the finishing stepover is
  2·R·s/10. The code makes it R·s/10.
- **Finishing profile.** The finishing paths are the tool-centre region offset outward
  by the allowance (`cpp:2500-2504`). They run only on edges whose two end vertices have
  Z = 1 (`cpp:1979-1991`). Edges in already-cleared area and edges added for travel
  outside the stock get Z = 0 (`cpp:2106-2370`). The edges are split into closed and
  open paths (`cpp:4414-4459`). `PopNextFinishingPass` picks the nearest one each time
  (`cpp:766-883`). Each is linked by `FindLinkPath` (`cpp:4523-4544`). Finishing passes
  that do not touch the stock are skipped (`cpp:4482-4488`).
- **Interior.** There is no mop. Passes continue while a valid engage point exists on
  the frontier (`cpp:4328-4359`). The exact area target cuts the cusp between passes
  as part of the next pass.
- **Checks without repair.**
  - The code shrinks the tool-centre region by allowance + 48 units and subtracts the
    cleared positions. A non-empty result sets `FailedToSetUpFinishingPass` and a
    warning "some cuts may be above optimal step-over" (`cpp:4362-4385`).
  - No engage point with area left sets `UnclearedAreaRemains` (`cpp:4356`).
  - Neither flag starts a repair.
  - The "remaining area" test at `cpp:4337-4350` loops over the paths of the cleared
    polygon. In Clipper output these paths include the holes of the cleared area, and a
    hole is uncleared material. The test takes the first vertex of each path. A path
    counts as remaining when that vertex is inside the tool-centre region and more than
    2R from the bound. It is a one-vertex-per-path test.
- **Region skip.** A component whose bound is already fully cleared is skipped
  (`cpp:2552-2561`). The comment there says the finishing work of such a region is not
  checked.

## 7. Which failure modes match V1, V2, V3?

| Defect (PLAN.md Q0) | Verdict | Reason |
|---|---|---|
| V1 corner mop loops, one plunge-cut-retract per loop | **Does not apply** (structure). Count of short corner passes: **unknown from the source**. | No mop or concentric corner loops exist. Corner residue is cleared by ordinary passes from frontier engage points (`cpp:4328`). The link to each pass is routed through cleared area and is a low G0 at `LiftDistance` (`cpp:2801-2980`, `py:306-316`). A full retract occurs only when no route of ≤ 3 × the direct length exists (`cpp:2914`, `py:318-323`), and it costs +10000 (`cpp:4064-4066`). The source does not show how many short passes a square corner takes. |
| V2 "full circles, half air" (corner-blend G2/G3 arcs) | **Does not apply** to arcs. A related cost exists. | `Adaptive.py` emits G0/G1 only (`py:298-323`). No arc output exists. The related cost: the lead-in and lead-out curves (`cpp:2982-3095`) are fed moves (`mtCutting`, `cpp:3256`, `cpp:3349`) that run mostly through cleared area. A lead-out can be as long as max(2 × beacon distance, 10 steps) (`cpp:3026`). |
| V3 does not grow into spaces | **Unknown from the source.** | Growth is greedy. The next pass starts at the frontier point with the cheapest link from the current position (`cpp:4031-4085`). Engage points cover the whole frontier, so a side lobe is a candidate as soon as its frontier exists. The code gives no evidence on when lobes fill compared with the main area. |

## 8. What does this method need that rs_cam does not have today?

rs_cam lookup on 2026-10-23 at HEAD (`df6de926`, working tree), with `rg` in
`crates/rs_cam_core/src/adaptive/`.

1. **Exact cleared geometry.** FreeCAD measures against exact cleared polygons
   (`ClearedArea`, `cpp:1047-1159`, Clipper booleans). rs_cam measures on a raster
   `MaterialGrid` with cell = max(R/6, tolerance) (`path.rs:699`, also `path.rs:1642`,
   `path.rs:1752`). An exact-area measure needs a polygon cleared-area model with a
   local window cache (`cpp:1094-1135`), or an equal analytic measure on the grid.
2. **A different engagement quantity.** FreeCAD matches area per mm to the target
   ±5 % (answer 3). rs_cam holds a swept width to a ceiling (`compute_swept_width`
   `search.rs:625`, `measure_step` `search.rs:899`, `step_within_pass_load`
   `search.rs:955`) and searches 7 fixed headings at 0°, ±15°, ±30°, ±45° plus a
   bracket refinement (`search.rs:1168-1180`). FreeCAD searches a continuous angle with
   a secant on the area error (`cpp:1246-1265`). The two quantities agree on a
   straight side cut (answer 3 derivation). They differ on a curved or head-on bite.
3. **Routed keep-down links.** FreeCAD bends a link around stock, to at most 3 × the
   direct length (`cpp:2801-2980`). rs_cam links are straight only: `feed_link_checked`
   walks one straight segment (`path.rs:303-362`). The keep-down range is 6R
   (`path.rs:999`). `rg -n -i 'detour|shortest|astar|a_star|dijkstra|route_link|routed'
   adaptive/ adaptive3d/` finds no router (one comment hit, `path.rs:3111`, is not a
   router).
4. **Link semantics differ.** rs_cam's `WithinPassLoad` link feeds at depth and stamps
   the grid (`path.rs:270-362`, `mod.rs:155-170`). FreeCAD's clear link does not cut.
   It is a G0 at `passEndDepth + LiftDistance` through cleared area (`py:306-316`), and
   it is added to the cleared polygon at the next pass start (`cpp:4186-4197`). Only a
   short (< 2·s·R) link may feed (`cpp:3206-3223`). With `LiftDistance` = 0 (`py:181`)
   that G0 runs at cut depth, which the rs_cam sentry
   `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` forbids. To use the FreeCAD link, rs_cam needs a "low lifted link"
   kind with a clearance test against planner stock. The rapid-safety rules of
   `dressup` and `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` then apply to
   it.
5. **Frontier candidate set ranked by link cost.** FreeCAD samples the whole frontier
   and ranks by full link cost with a retract penalty (`cpp:3927-4096`). rs_cam
   `frontier_hop` takes the stock cell nearest `from` and tries stand-offs around that
   one cell, up to 32 cells (`path.rs:2254-2319`). Other passes use
   `find_entry_point` (`path.rs:954`, `path.rs:972`).
6. **Climb-only rule.** `rg -n -i 'climb|conventional'` over `path.rs search.rs
   spiral.rs mod.rs` finds nothing. rs_cam's adaptive has no conventional-cut test.
7. **Lead-in and lead-out curves.** `rg -n -i 'lead.?in|lead.?out|leadin|beacon'` over
   `path.rs search.rs mod.rs` finds only "leading arc" hits (`mod.rs:97`, `mod.rs:118`
   and others). rs_cam has no lead curves.
8. **Finishing allowance.** FreeCAD keeps the clearing s·R/10 off the walls and then
   profiles them (answer 6). rs_cam has a wall walk on the region boundary within the
   pass load (`finish_walls_within_pass_load`, `path.rs:1891`). A finishing allowance
   was not found in the files read (`rg -n -i
   'finish.?allow|stock_to_leave|wall_allowance|finishing_thick'` is empty in
   `path.rs mod.rs search.rs`).
9. **Already present in rs_cam.**
   - Per-component processing: `inset_piece_outlines` (`mod.rs:388`), the D2 rule in
     `adaptive/CLAUDE.md`.
   - Rest input: `standing_material` (`mod.rs:201`), where FreeCAD has initial cleared
     polygons (`cpp:1935-1946`).
   - Helix and distance-transform entries exist: `emit_helical_starter_pocket`
     (`path.rs:2497`) and `find_entry_via_distance_transform` (`search.rs:1501`). This
     note did not check whether they enter at the deepest point.
10. **Step length.** FreeCAD steps at most min(R/4, 8·48 units) (`cpp:3628-3630`).
    rs_cam steps 3 cells = R/2 at the R/6 cell (`path.rs:751`).

Neither method has a time or acceleration model. FreeCAD link cost is length in mm only
(`cpp:4059-4077`).

## Ideas usable in rs_cam

- Measure each step as the exact new area per mm against a derived target
  (0.98948·s·D for a straight side cut). Compare it with the swept-width ceiling on the
  fixtures before you choose one.
- Search the heading as a continuous angle with a clamped secant (fraction in
  [0.2, 0.8]) and a 3-point angle prediction. Do not use a fixed fan.
- Reject a step whose conventional fraction is 0.51 or more. This makes climb a planner
  rule.
- Collect next-pass candidates along the whole cleared frontier at a fixed spacing.
  Score each by its full link cost, with a large penalty for a retract.
- Route keep-down links around stock by bisection with perpendicular offsets, capped
  at k × the direct length (FreeCAD k = 3). This addresses V1: corner passes then join
  without retracts.
- Add a "lifted link" kind: a low rapid at floor + a lift greater than 0, through
  cleared area only, with a clearance test against planner stock. Keep the fed, stamped
  link for short gaps.
- Keep the clearing a small allowance off the walls. Finish the walls in one profile
  that runs only on edges that still need it.
- Derive the first-bite protrusion of an engage point from the target area with the
  circle-segment formula. Do not type a constant.
