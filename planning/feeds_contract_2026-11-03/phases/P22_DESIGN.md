# P22 design note: a shared opening for the adaptive (helix bore, outward spiral, corridor trochoid)

Read-only survey at HEAD f168ffab9 (2026-11-09). No code changed. No cargo job ran. Subject:
`planning/rivmap_block_job/nz_wanaka_21_operator.toml`, idx 1 "Back bulk rough 18 mm"
(`clearing_strategy = "geodesic_adaptive"`, `entry_style = "ramp"` 3 deg, `helix_radius_factor`
0.3, `helix_pitch` 2.0, stepover 5.5, depth 10, `trochoid_cap_mult` 1.6; tool 6: D 18, 2F,
`centre_cutting = no`, core 8, protrusion 1). Evidence of the slot: `P20_power_gap.md` (move 1697,
ap 10, ae 17.72, 1.757 kW peak against 1.125 kW).

## 1. Finding: the 2D code already has the opening; the 3D arm turns it off

The operator asked for (1) a helix into a bore larger than the tool, (2) an outward spiral at the
engagement limit, (3) trochoids in narrow corridors. Items 1 and 2 exist in the 2D adaptive, and
the 3D Geodesic arm calls the same code, but with the helix radius set to 0:

- `adaptive3d/clearing.rs:3799` builds the slice params with `entry_helix_radius: 0.0`. Thus
  `LinkLoad::plunge_departure_exempt` (`adaptive/path.rs:342`) is true. Every entry of the plan is
  a straight plunge in the planner model, and the steps inside R of the plunge use
  `PassLoad::departing()` (no ceiling, `adaptive/search.rs:931`).
- The starter pocket (`emit_spiral_starter_pocket`, `adaptive/path.rs`) spirals out from that
  plunge. Its inner laps stand inside the R hole, so they read up to a full slot at the full
  layer depth. `geodesic::slice::fed_runs` collects them as "departure windows" and caps their
  feed at `F c / L`. It does not cap the width.
- The emitter does not cut the hole that the planner stamps. For this tool G-CENTRECUT turns the
  entry into the shared ramp (`adaptive3d/path.rs:2333`, `dir (1,0)`, no fold): 19.08 mm legs
  (`ENTRY_CLEARANCE / tan 3 deg / 2`) in +X. The planner stamps a disc of R
  (`entry_floor_radius`, `adaptive3d/path.rs:1585`). Air that the plan does not know is safe for
  load, but the plan and the cut disagree.
- D65 admits a step over the ceiling up to `FULL_SLOT_LOAD = 1.0` (a full slot) at two sites:
  the 2D replay (`LinkLoad::admits_capped`, `adaptive/path.rs:243`) and the 3D stock-read split
  (`admits_capped_split`, `adaptive3d/clearing.rs:965`). The feed is capped; the width is not.
- `trochoid_cap_mult` is read in one place only: the pre-sim power case `PlannerCap`
  (`compute/catalog.rs:1346`, `feeds/engagement.rs:797`). The card thus claims a planner cap of
  1.6 x 5.5 = 8.8 mm, while the planner admits 18 mm. That is a false claim today.

Hypothesis (P22a must prove it before any change): the 17.72 mm sample is a departure window or a
D65 step. Move 1697 ends in a 1 mm Z rise (P20 2.2), which fits a D65 stock-read split step.

## 2. Survey

| Piece | Where | 2D-only ties | Reuse | Sentries |
|---|---|---|---|---|
| Helix hole in the planner: `LinkLoad::entry_hole`, `entry_modelled`, `stamp_entry` | `adaptive/path.rs:305-372` | none; reads `entry_helix_radius` and `dressup::contained_helix_radius` (one function for planner and dressup) | as is; 3D passes a non-zero radius | `a_clearing_cut_holds_the_pass_load_g_adaptpassload`, `adaptive_property_harness` |
| Spiral starter pocket: `emit_helical_starter_pocket`, `emit_spiral_starter_pocket`, `starter_spiral_points` | `adaptive/path.rs:2816-2990` | flat 2D grid (`MaterialGrid`) at one Z; the 3D slice already feeds it a per-level grid | as is | the same, plus `the_geodesic_preset_keeps_its_plan_s7_3` |
| Departure exemption: `PassLoad::departing`, `fed_runs` departure windows | `adaptive/search.rs:931`, `geodesic/slice.rs:257-397` | none | becomes dead in 3D after P22b; keep for 2D plunge entries | `a_geodesic_step_over_the_ceiling_is_cut_capped_d65` |
| Outward spiral at the limit: `held_core_spiral`, `spacing_bound` `s'(rho)` | `geodesic/wraps.rs:473-600` | none; tool-centre region and lattice | as is (already the 3D core spiral); `spacing_bound` moves to a shared place in P22d | `--lib adaptive::geodesic`, `geodesic_adaptive_keeps_the_research_geometry_s7_4` |
| Pass ceiling `c D`: `PassLoad::swept_width` | `adaptive/search.rs:895` | none | as is: the one limit for planned steps | `--lib adaptive` (S2) |
| D65 capped admission: `admits_capped`, `admits_capped_split`, `FULL_SLOT_LOAD` | `adaptive/path.rs:70,243`; `adaptive3d/clearing.rs:965` | 3D slice door only (`capped` flag) | after a refactor: the bound becomes a parameter | `a_geodesic_step_over_the_ceiling_is_cut_capped_d65`, `a_geodesic_cap_survives_every_dressup_s7_4` |
| Trochoid insert: `emit_trochoid_loop`, `TrochoidParams` | `adaptive/spiral.rs:266-330` | 2D ContourSpiral only; typed constants (radius `max(s, 0.4R)`, pitch `0.6 s`, cap `0.45`, 16 probe directions) from a 2026-06-13 harness tune | after a refactor (P22d); the tuned values stay for ContourSpiral | `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0` |
| Corridor gate: `is_narrow_machinable`; the starter fit `dt >= 2R` | `adaptive/path.rs:2990`, `:2840` | typed `2 x cutter area` in the first | the starter fit only (it decides "no field") | `adaptive3d_plans_every_slice_piece_g_adapt3dpieces` |
| 3D "no field" piece: `leave_piece` | `adaptive3d/clearing.rs:2532,2567` | 3D | the corridor hook for P22d (the piece goes to the rest op today) | `a_rough_publishes_the_regions_it_leaves_d44` |
| Helix emitter: `dressup::emit_helix`, `emit_floor_lap`, `EntrySafety::helix_floor_lap` | `dressup/entry_descent.rs:924` | none; 3D sets `helix_floor_lap: false` (`adaptive3d/path.rs:2213`) | as is | `adaptive3d_entry_stock_aware`, `--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut` |
| Helix radius rule: `HelixRadius::resolve` (cap at the flat bottom, G10 Q6), `helix_entry_diameter_mm` | `compute/config.rs:994-1061` | none | after a small change: add the core floor (section 4) | `a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10` |
| Centre limits: `NonCentreCuttingLimits` (protrusion, `atan(protrusion / core)`) | `compute/tool_config.rs:387-425` | none | as is | `a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`, `..._k5_centrecut` |
| 3D entry plan: `plan_entry`, `LevelSink::commit_pending`, `entry_floor_radius` | `adaptive3d/clearing.rs:827,2370`; `path.rs:1585` | one global entry style per op | after a refactor: a per-entry helix radius | `adaptive3d_planner_never_ahead_of_emitted_path`, `adaptive3d_plan_matches_merged_path_g_plansimgap` |
| Engagement measures: tool_load power and deflection via `Engagement::from_sample`; planner `step_reading` (max of swept width and area per mm) | `tool_load/`, `feeds/engagement.rs`, `geodesic/slice.rs:126` | none | as is; the gate is the judge of each phase | `one_steady_feed_for_the_cap_the_gates_and_the_card_p20e` |
| `engagement_measure` (`DiskArea` on this op) | `adaptive3d/clearing.rs:382` | load-only; the 2D rule reads the swept width (operator decision 2026-09-26) | not reusable | `adaptive3d/tests.rs:343` (R6 inert pin) |

## 3. Proposed shared components

The operator rule is "re-use the 2D code; make it generic and shared". Most of the code is shared
already. P22 adds two small shared items and one moved item. It writes no parallel 3D path.

**A. `adaptive::opening` (moved, not new).** Move `emit_helical_starter_pocket`,
`emit_spiral_starter_pocket` and `starter_spiral_points` out of `path.rs` into
`adaptive/opening.rs`. Add one type:

```text
pub(crate) struct Bore { centre: P2, helix_radius: f64 }   // hole radius = R + helix_radius
fn plan_opening(grid, mask, distances, R, load: Option<(&PassLoad, s, &LinkLoad)>)
    -> Option<(Vec<AdaptiveSegment>, P2, Option<Bore>)>
```

The input is what a layer plan already hands in: the piece polygon in tool-centre space at one Z
(the 3D slice gives a flat grid per level), the tool radius, the pass load, and the planner grid
as stock. The `Bore` goes out on the `StarterPocket` marker (add `helix_radius` to
`AdaptiveRuntimeEvent::StarterPocket`), so the 3D emitter knows the hole it must cut. 2D stays
byte-identical: a pure move, and the marker field is read only by 3D. Sentries: the 2D list in
`adaptive/CLAUDE.md` and `the_geodesic_preset_keeps_its_plan_s7_3`.

**B. `adaptive::EngagementLimit` (new, one definition).** Two bounds as fractions of D:

- `ceiling`: `PassLoad::swept_width(..).ceiling`, the bound of every planned step (unchanged).
- `capped`: `min(1, trochoid_cap_mult x s / D)`, the bound of a D65 step. It replaces
  `FULL_SLOT_LOAD` in `admits_capped` and `admits_capped_split`. `LinkLoad::with_capped` takes
  the bound. `effective_trochoid_cap_mult` stays the one reader of the field, so the
  `PlannerCap` power case and the planner read one value, and the card claim becomes true.

2D sets no capped rule (`cleaned(.., false)`), so 2D does not change.

**C. `adaptive::trochoid` (moved, then extended in P22d).** Move `emit_trochoid_loop` and
`TrochoidParams` out of `spiral.rs`. ContourSpiral keeps its tuned values through a named
constructor (byte-identical). The corridor constructor derives its values (section 4). Move
`spacing_bound` from `geodesic/wraps.rs` beside it, so the core spiral and the corridor read one
law.

## 4. Numbers, each from a rule

| Quantity | Rule | Source | Wanaka idx 1 |
|---|---|---|---|
| Pass ceiling `c D` | `radial_woc_fraction_from_leading_arc(pass_engagement_ceiling(target_engagement_fraction(s, R)))` | `PassLoad::swept_width` | 0.3329 D = 5.99 mm |
| Capped bound | `min(1, trochoid_cap_mult s / D)` | section 3 B; the `PlannerCap` case | 8.8 mm (0.489 D) |
| Helix radius `r_h` upper cap | `r_h <= flat bottom` (`width_at_height(0)`) | `HelixRadius::resolve`, G10 Q6 | 9 |
| Helix radius lower floor (non-centre tool) | `r_h >= d_core / 2`. The bore centre stays at `r_h` from the tool axis. When `r_h < d_core / 2`, the centre is always inside the core and a post of radius `d_core/2 - r_h` stays. | new derivation; add to `HelixRadius::resolve` | 4; the request 0.3 x 18 = 5.4 passes |
| Helix radius emitted | the request, clamped to [floor, cap], then `contained_helix_radius` at the entry | `helix_radius_for`, `LinkLoad::entry_hole` | 5.4 where it fits |
| Bore radius | `R + r_h` | `entry_hole` | 14.4 mm (bore 28.8 mm) |
| Helix pitch | style Helix: `helix_pitch`; style Ramp: `2 pi r_h tan(ramp angle)` (the same descent angle as the ramp). Then the existing clamp `slope x chord x 36`, slope `protrusion / core` | `emit_helix` G-CENTRECUT clamp | ramp: 1.78 mm; clamp 4.24 mm |
| First spiral spacing out of the bore | `step_within_pass_load` on each step, pitch `min(s, R)` halved up to 3 times | `emit_spiral_starter_pocket` | pitch 5.5 fails at the R lap (w 6.42 > 5.99 by the spacing law); 2.75 holds (max w 5.29) |
| Corridor test | the starter does not fit: no lattice point with `dt >= 2R` (its last lap at R clears 2R; unchanged, since `R + r_h <= 2R`); the piece is "no field" | `emit_helical_starter_pocket` | ‒ |
| Trochoid loop radius `rho` | the tool-centre half-width of the corridor at the loop: the EDT of the tool-centre piece | `grid_field::distance_transform_2d` | ‒ |
| Trochoid advance per loop | `min(s, s'(rho))`, `s'` = `spacing_bound(R, c D, rho)` | `geodesic/wraps.rs:473` | rho 1: 0.62; rho 4: 2.00 |

A corridor with `rho -> 0` (width about D) has `s' -> 2 c rho -> 0`: no trochoid holds the
ceiling. That corridor is a slot by geometry. Per the "never refuse a job" ruling it stays a D65
capped cut under the capped bound if it fits, else it goes to the rest op with a warning.

## 5. 3D integration, per layer and piece

1. **Bore.** `plan_one_outline` sets `entry_helix_radius` = the emitted `r_h` (section 4) in
   `params_2d`. The planner then stamps `R + r_h` at each entry and drops each entry where no
   helix fits (`entry_modelled`, G-ADAPTPASSLOAD round 3). The 3D emitter reads the bore from the
   marker or the entry segment and calls `emit_helix` with `helix_floor_lap: true`, at the
   contained radius of that entry. `entry_floor_radius` becomes per entry (`R + r_h`). The plan
   and the cut then agree (G-PLANSIMGAP, G-PHANTOMSTAMP).
2. **Outward spiral.** No new code: the starter spiral and `held_core_spiral` already hold `c D`.
   Without a plunge, `plunge_departure_exempt` is false and the departure windows are empty.
3. **Corridor trochoid.** For a "no field" piece, plan trochoid loops along the EDT ridge with the
   section 4 values, and replay each loop under the pass load. A piece that the trochoid cannot
   hold stays with `leave_piece` (the D44 rest region), as today.
4. **Rest of the layer.** The wraps, the cleanup and the links stay as they are. D65 reads the
   capped bound. A step over it is a split (a re-entry with a bore), as a step over 1.0 is today.

`engagement_measure`: it stays load-only. Recommendation: hide it in the UI and keep serde for
old files. The 2D rule (swept width, area per mm in `step_reading`) is the one measure.

## 6. Phases (one agent each; the lead verifies)

| Phase | Work | Sentry (new or moved) | Predicted effect on idx 1 |
|---|---|---|---|
| P22-0a | The core-correct stamp: a non-centre-cutting tool stamps only its blade annulus; the core does not cut. Survey, fix shape and blast radius: `P22_sim_core.md` (fix (a), blast radius). A centre-cutting tool stays bit-identical. | `a_helix_inside_the_core_leaves_a_post_g_simcore` (the probe's helix at r 2-3 mm leaves the post at the centre in the sim); `a_centre_cutting_stamp_is_unchanged_g_simcore` | the sim keeps posts that real wood keeps; the tool-6 stock moves (section 7) |
| P22-0b | The per-sample core check on every fed move of every op: wood under the core deeper than the protrusion is a warning with its position, never a refusal. Add the helix-radius warning (`r < d_core / 2`). Survey: `P22_sim_core.md` (fix (b)). | `a_split_descent_strikes_like_one_move_g_simcore` (the probe's 11-move plunge and 22-move ramp each give a finding) | the export names each core strike; idx 1 at r 5.4 gives none |
| P22a | Read-only test readout: classify every fed sample over `c D` by its source (departure window, D65 2D, D65 3D split, ramp entry, other) on a small fixture, and on idx 1 through the readout script. Also (lead, 2026-11-09): read the power of ONE engagement (D 18, ap 10, ae 8.8, 18000 rpm, white oak) from Suggest, `get_safe_envelope` and the power gate, and show they agree. The P20 fit of the envelope grid gives Ks 93.6, F_edge 5.91; the acceptance table and Suggest give Ks 58.516, F_edge 4.595 (P at ae 8.8, 6999 = 0.985 kW). Find the cause (RPM of the grid, a derate, a second law). The P22 power predictions below use the fit and hold only after this check. No product change. | new `--lib` test `every_wide_step_has_a_named_source_p22a` | none; it names the source of move 1697 |
| P22b | Bore: section 5 step 1; the core floor in `HelixRadius::resolve`; the helix floor lap in 3D | new `a_geodesic_layer_opens_with_a_helix_bore_p22b` (no departure window; planner hole equals the cut hole); `adaptive3d_planner_never_ahead_of_emitted_path`, `..._k5_centrecut`, `..._g10` | departure windows 0. The opening helix has ap <= 1.78 at ae <= D: about 0.50 kW at the ramp feed 6448 (P20 fit). Spiral steps <= 5.99 mm: about 1.18 kW at 8475, so the modulator holds 1.125 kW at about 7840. |
| P22c | Capped bound: `EngagementLimit`; replace `FULL_SLOT_LOAD` at both sites | update `a_geodesic_step_over_the_ceiling_is_cut_capped_d65` (bound 0.489, not 1.0); new pin: `PlannerCap` equals the planner bound | no fed step over 8.8 mm (plus one cell, 1.5 mm, of lattice read). At 8.8 mm, ap 10: 1.66 kW at 8475; the power cap gives about 4595 mm/min. More re-entries; cycle time rises. |
| P22d | Moves A and C (pure moves); `spacing_bound` shared | the 2D list in `adaptive/CLAUDE.md`; `contour_spiral_gcode_validity_phase0`; byte parity | none (pure move) |
| P22e | Corridor trochoid for "no field" pieces | new `a_corridor_is_cut_by_trochoids_at_the_ceiling_p22e`; `a_rough_publishes_the_regions_it_leaves_d44` (fewer left regions) | no power change at the peak; fewer D44 rest regions; more cut time |

Order: P22-0a, then P22-0b (the opening's sentries need a sim that sees a post), then P22a, then P22b and P22c (independent), then P22d, then P22e. The P20 modulator fix
(worst-sample power cap, `P20_power_gap.md` 4) is still needed: a feed stage must hold the power
of the path it gets.

Power figures use the P20 fit (`Ks` 93.6 N/mm2, `F_edge` 5.91 N/mm, rectangle section). That fit
reads 5 % low near a slot. The gate on a regenerated trace is the judge, on a GUI built from HEAD.

## 7. Risks

- **Geodesic pins move.** `adaptive/geodesic/CLAUDE.md` says the plan equals the research move by
  move but for D65. P22b changes every 3D entry and the starter. `geodesic_adaptive_keeps_the_
  research_geometry_s7_4` and `the_geodesic_preset_keeps_its_plan_s7_3` (3D readings) need a
  lead re-baseline with a written reason. The 2D door must not move.
- **Fewer re-entries are legal.** The starter fit test (`dt >= 2R`) does not change, because
  `R + r_h <= 2R`. But `entry_modelled` drops each re-entry where no helix fits, so some small
  residues go to the cleanup or to the rest op.
- **P22-0a moves the simulated stock** of every op with a non-centre-cutting tool. On wanaka that
  is tool 6: "Face back to T1", "Back bulk rough 18 mm", "Face clamp pads 6 mm", "Face front to
  T", "Front bulk rough 18 mm", and each `stock_source = machined` op after them. `rg` at
  f168ffab9: the FM1 cells (`tests/common/feeds_cells.rs`) build no such tool; the tests that do
  are `a_non_centre_cutting_tool_never_plunges_into_stock_g_centrecut`,
  `a_rough_plans_its_cut_and_link_descents_to_the_core_limit_k5_centrecut`,
  `project_curve_plunges_at_the_axial_chip_q8`, `a_tool_outside_its_domain_is_refused_at_every_door_f029`
  and `a_job_with_each_finding_exports_and_names_it_k6` (`common/k6_project.rs`). List their
  predicted moves before the re-bless.
- **Cycle time.** P22c turns D65 steps between 8.8 and 18 mm into splits and re-entries. Each
  re-entry is now a helix (slower than the ramp). The C38 timing harness must measure it.
- **The heavy judge.** `geodesic_adaptive_holds_the_pass_load_c12` (about 9 min) must run after
  P22b and P22c; ask the operator first on the operator PC.

## 8. Questions for the operator

1. The op says `entry_style = "ramp"`. Do the geodesic arm's entries always become helix bores
   (the planner needs the bore), with the pitch from the ramp angle (1.78 mm here)? Or only when
   `entry_style = "helix"`, with the ramp kept and the departure windows kept?
2. Is the D65 bound `trochoid_cap_mult x s` (8.8 mm here) the limit you mean? The other choice is
   the pass ceiling `c D` (5.99 mm), which removes D65 and splits every wider step.
3. May the corridor trochoid (P22e) wait until P22a-c are graded on wanaka? Today those pieces go
   to a rest op, so they do not cause the overload.

## 9. Operator rulings (2026-11-09)

- Order: fix A (worst-sample power cap), then P22-0a, P22-0b, P22a, P22b, P22c, P22d; one agent per
  phase, each graded before the next. Go.
- Q1 entries: no lie to the user. An op that says `ramp` gets a ramp; a new adaptive op defaults to
  helix. Where a region has no room for the bore, fall back (helix, then ramp, then plunge) and
  emit a finding that names the toolpath, the place and the fallback. Generic to every setup.
- Q2 width limit: "not a magic number; choose the most correct one". Lead ruling: the pass ceiling
  `c D` (the swept width of the stated stepover, `PASS_ENGAGEMENT_TOLERANCE` 0.05 from the libactp
  reference; 5.99 mm on wanaka idx 1). `trochoid_cap_mult` 1.6 has no source (the default of the
  removed ContourSpiral strategy, `compute/operation_configs.rs:1985`): retire it. Suggest, the
  card and the acceptance row P3 read the pass ceiling (an operator-ruled table change).
- Q3 the corridor trochoid: explained to the operator; lead recommends P22e after P22c, sized by
  the P22a readout. Operator to pick.
- Region too small for the helix radius floor (`r_h >= d_core / 2` on a non-centre-cutting tool):
  the Q1 fallback applies (ramp, then plunge), and the existing G-CENTRECUT rules bind the
  fallback (the ramp at the limit angle; a plunge deeper than the protrusion warns). The finding
  names the toolpath, the place, the fallback and the reason ("helix radius below the core
  radius"). Never a refusal.
- Q3 ruled (operator, 2026-11-09): P22e waits; run P22-0a..P22c first, then decide P22e from the
  P22a readout (the share of wide or slow cutting that comes from corridors).
- P22-0c added (operator direction, 2026-11-09): one tool body model for every tool with
  non-cutting parts. The tool is a list of axial zones (z range, inner and outer radius, cuts or
  not); the simulation stamps only cutting zones (P22-0a's blade ring is the first case); ONE
  check flags any non-cutting zone that meets the stock, as a warning with its position, never a
  refusal. It folds in today's four separate checks: `check_collisions` (shank, holder, head
  bands; `ToolAssembly` in stock/collision.rs), `tool.flute_overreach` (the shank above the
  flutes), `core_strike` (collision.rs:1096) and the P22-0b core check. Each zone kind keeps its
  own finding id and text. Order: after P22-0b, before P22a.

## 10. Lead note after the P22a readout (2026-11-09 night, commit 6fd852527)

The wide samples of wanaka idx 1 are not D65 splits. The 3D clearing "Perimeter sweep"
(`adaptive3d/clearing.rs clear_one_region`, the shipped path where `grown_outlines` returns None)
gives 72 % of the samples past the pass ceiling: 22 moves in three loops right after a ramp
entry, a full slot (ae 17.80 mm at ap 9.00 mm) at the full 8475 mm/min with no cap. D65 3D
splits: 0. The order changes: a new phase P22f (stop the geodesic slice from cutting the
sweep: find the None path, fix the grow or plan the band under the pass load) comes BEFORE
P22b. P22b still removes the departure windows (79 samples, capped). P22c stays as a rule;
its idx 1 effect depends on P22f. P22e has no case on idx 1 (ruling Q3 holds). The exact
source door (P22a-2, `FeedCapSource`) is designed in `P22a.md` and deferred. The P22f design
is `P22f_DESIGN.md` (in progress).
