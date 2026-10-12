# Toolpath optimizer review, 2026-10-12

Scope: `tool_load/optimize` as the GUI "Optimize" rollup and MCP
`optimize_toolpath` run it, on `planning/rivmap_block_job/testpiece_mini.toml`
(65 x 65 x 24.5 mm oak, 3 mm 2-flute flat, 30 mm stick-out). A reasoning
review; no product code changed. Measurements ran on a copy of the
project with the release CLI and a scratch probe built at HEAD
`b20d947d` (`CARGO_TARGET_DIR=target/optreview`). The probe output is in
`PROBE.md` beside this file.

## 1. Verdict

The process is not the right one for "fastest while full cut". The
optimizer is a feed/RPM scaler with a small depth x stepover grid, and
its search design predates the 2026-09 feeds work. It pulls the weakest
lever first (feed: under 10 % of this op's time) and cannot pull the
strong ones (retract height and count, entries, level count). The one
deep-cut lever it has found the win: on the Z22 channel it simulated
dpp 1.5 mm at 371 s against a 1 315 s baseline (-72 %) and then refused
it, because the depth row `Exceeds` the machine rule of thumb
`doc_roughing_factor = 0.25` and the optimizer treats that row as
unsafe. The headline then said "none were faster than baseline by
enough" and the explanation blamed "chipload, power, or deflection",
neither of which is true. On this project the force model refuses the
material, so power and deflection are `Unmodeled` and the optimizer runs
with the chipload point, the depth rule and the machine caps only. It
does not verify the full cut. The red chipload is not a real
over-chipload; at HEAD on this file no chipload sample can be red at
all (section 4).

## 2. The algorithm as it is

Entry `optimize_toolpath_inner`, `crates/rs_cam_core/src/tool_load/optimize/mod.rs:213-456`.

1. Build the context (`context.rs:346`). Skip drills and `Material::Custom`.
2. Score the baseline with the gates against the on-screen trace
   (`mod.rs:277-282`). Cycle time = `toolpath_summaries.total_runtime_s`
   (`context.rs:32-41`), rapids included.
3. Pre-flight (`preflight.rs::preflight_classify`): refuse on deflection locked at the
   minimum-force corner, or on bipolar chipload.
4. Stage F. No gate exceeds: scale `(feed, rpm)` together at constant
   chipload by `k = min(rpm_max/rpm, feed_ceiling/feed, P_max/P_peak,
   lut_rpm_max/rpm)` (`strategy/headroom.rs:59-105`); `lut_rpm_max`
   falls back to `rpm_nominal x 1.2`. A gate exceeds: one retargeter per
   gate (chipload: feed, RPM frozen; power: feed; deflection: DOC).
5. Stage 1. A DOC x stepover x scallop grid at the stage-F feed
   (`strategy/grid.rs:50-140`). DOC points `{0.7, 1.0, 1.2, 1.4} x
   baseline`, plus the factory default (Pocket 1.5 mm,
   `operation_configs.rs:375`), plus `0.85 x / 1.15 x` the LUT
   `ap_min_mm..ap_max_mm` only when a row prints BOTH (`bounds.rs:169`;
   the Spektra rows print `ap_max_factor` only). Each cell: write the
   params, regenerate, simulate at 1 mm with
   `adaptive_feed_modulation: false` (`candidate.rs:160-170`), run the gates.
6. Stage 2. Top 3 by `composite_score = savings_s - 5 x chipload_distance
   - 3 x power_overuse - 2 x deflection_overuse` (`rank.rs:7-24`; no
   depth term), re-simulated at 0.5 mm.
7. Tiering (`outcome.rs:718-857`). `Ranked` needs `> 0.5 s` faster, no
   gate `Exceeds`, no band-admitted `Within`, no regression.
   `candidate_is_safe = !verdict.any_exceeded()` (`delta.rs:119-123`);
   `any_exceeded` reads `milling_criteria`, which holds the DEPTH row
   (`verdict.rs:419-425`).
8. GUI apply (`controller/events/mod.rs:1606-1670`): the `first_safe`
   params go in through `RestoreToolpathSnapshot`; dressups unchanged.
9. Project rollup (`mod.rs:927-982`): one search per enabled toolpath,
   all against one baseline trace, rows independent.

Levers: feed, RPM, depth per pass, stepover, scallop height (`axes.rs`).
Not levers: retract height, link or stay-down, entry style, ramp angle,
helix pitch, ramp feed, plunge rate (coupled only), strategy, tool,
boundary, order (`bounds.rs:320` returns no bounds for the entry axes).
Objective: minimum `total_runtime_s` with every gate inside its bound.
Not "full cut": a candidate is read for cycle time, air fraction and
the gates only (`candidate.rs:398-470`).

## 3. Findings, ranked by cycle-time and safety impact

F1. The optimizer found the deep cut and refused it with a false
reason. MEASURED (probe, `optimize_toolpath` on toolpath 2, 11.6 s
wall). Outcome `NoSafeImprovement`. Survivors: dpp 1.5 / stepover 2.0 /
feed 4 652 / 21 600 rpm at 371 s; dpp 1.5 / 1.17 at 509 s; dpp 1.5 /
1.01 at 538 s; baseline 1 315 s. Every survivor: Chipload `Within`,
Power and Deflection `Unmodeled`, DepthOfCut `Exceeds` (1.5 mm against
the cap 0.75 mm). Headline: "Tried 3 candidates; none were faster than
baseline by enough to recommend." Explanation: "every candidate hit a
gate limit (chipload, power, or deflection)". `GateKind` has no depth
arm (`narrative.rs:171-175`), so `limiting_gates_for_verdict` is empty
for a depth veto and `headline_no_safe` falls to the "not faster"
sentence (`narrative.rs:871-879`); the modal row shows a red badge with
no reading. All three survivors were dead cells: the composite score
has no depth term, so the safe 1.2 x cell (0.713 mm, about -15 %) never
reached stage 2.

F2. The time is in Z rapids and entries, not in the feed. MEASURED
(CLI, setup "1 Back channels", 0.2 mm, modulation on). Z22 channel
1 315 s = rapids 692 s + cutting 624 s (410 s air); by intent: rapid 692,
entry 295, cut 329. 37 levels, 45 247 moves, 9 542 entries (9 250 ramps,
292 plunges; 7 662 in air), 370 retracts to the project `safe_z` 29.5.
Rapid Z travel 9 931 mm at the Z rate 1 000 mm/min = 596 s; XY rapid
722 mm = 4 s. In-material clearing is about 115 s. A feed lever of x1.2
is worth about 100 s; the retract lever about 550 s. The optimizer has
no retract lever: the pocket emits two rapids per ring to
`heights.retract_z` (`ops/pocket.rs`, `compute/execute/clearing_2d.rs:39`),
which in auto mode is the project `safe_z` (`compute/config.rs:312`);
`link_moves` is off and candidates keep the baseline dressups
(`candidate.rs:410`).

F3. Depth per pass is the lever, and a rule of thumb vetoes it.
MEASURED and READ IN CODE. CLI sweep of the Z22 channel: dpp 0.5946
(37 levels) 1 315 s; 0.83 980 s (-26 %); 1.5 590 s (-55 %); 3.0 360 s
(-73 %); advance per tooth 0.1077 mm in every run. The depth row bound
is `0.25 x D = 0.75 mm` (`machine/mod.rs:134-157`, `tool_load/depth.rs:214-297`);
S3 ruled it "reported, never refuses" for export, but it refuses an
optimizer candidate through `candidate_is_safe`. The vendor row prints
`ap_max_factor = 1.0` (1 x D at full feed, 2 x D at 75 %, 3 x D at 50 %;
`data/vendor_lut/observations/amana_flat_end.json`,
`amana-flat-hardwood-pocket-3000-2f-spektra`). The force model, when a
line exists (WhiteOak FPL, probe what-if, ae 0.838, fz 0.1077): 6.1 N /
0.055 mm at 0.595 mm; 8.5 N / 0.077 mm at 0.83; 15.4 N / 0.136 mm at
1.5; 20.5 N / 0.179 mm at 2.0; 30.8 N / 0.262 mm at 3.0, against
`WITHIN 0.050` / `EXCEEDS 0.200` mm. The model's ceiling is about
2 mm; the rule's is 0.75 mm; the baseline already sits above the
validated 0.050 mm.

F4. The force model refuses this project's material; the optimizer runs
blind and silent. MEASURED (probe: `force_line: Err(NoDensity)`; Power
and Deflection `Unmodeled(MaterialUnvalidated)` on all nine ops). The
stock is `SolidWoodByJanka { source_id: "rivmap feeds_oak.toml" }`;
`Material::force_line` needs `source_id == "fpl_ch5_2010"`
(`material/mod.rs:941-945`, `wood_species_library.rs:99-102`). The
headroom solve drops the power cap (`headroom.rs:80-83`), the
deflection pre-flight is skipped, the rank penalties are zero, the
modulator has the chipload point and the machine caps only. The
optimizer skips `Material::Custom` only (`mod.rs:236-238`).

F5. Feed is never re-derived when DOC or stepover move. READ IN CODE.
The grid patches `DepthPerPass` and `Stepover` only (`grid.rs:117-137`).
The DOC de-rate (`feeds::geometry::doc_derating_scale`), the hardness
scale, the dial (`suggest/aggressiveness.rs`) and the G6/G10 ramp and
plunge rules are not consulted. A 2.5 x D cell ships the 1 x D feed and
fails the de-rated band after a full generate and simulate. The ramp
feed stays at the baseline when the feed moves (`stamped_optimizer`
only stamps it).

F6. The optimizer predates the feeds work. READ IN CODE (git). The
search design is 2026-05-08/09 (`policy.rs` "optimizer redesign
2026-05-08"; G16 `cf6a5cb8`..`64dccf0e`). The matrix, dial (R4), force
line (B6), size law (G1), point band (A2) and G6/G10 rules landed
2026-09-23..10-01; commits in `optimize/` since then are plumbing and
refactors. Suggest is not called.

F7. Candidates and baseline are not one measure. MEASURED (probe
`assumptions`: baseline modulated; candidates
`adaptive_feed_modulation: false` at 1.0 / 0.5 mm against the project's
0.2 mm; `candidate.rs:160-170`). Harmless here (the modulator changes no
roughing feed); two operating points on a project with a force line.
The "Optimize resolution gap" (`extrapolation_2026-09-24/PROMPT_NEXT.md`
item 5) is still open.

F8. The full cut is assumed. READ IN CODE. No removed-volume delta,
no rest or uncut check, no collision count on a candidate. DOC changes
the Z ladder (`ops/depth.rs::z_ladder`). The CLI sweep reported "crosses
material an upstream op left standing ... peaking at 1.47 mm" on the
1.5 mm run: the diagnostics see the deeper bite, the optimizer does not
read them.

F9. The headroom stage goes past the chart. READ IN CODE and MEASURED
(every survivor at 21 600 rpm, feed 4 652). `k_lut = rpm_nominal x 1.2`
when a row prints no `rpm_max` (`headroom.rs:86-96`); `spindle_strategy
= match_chart` is not consulted. The chipload gate cannot see it.

## 4. The red chipload at the final passes

- The gate and the card read one quantity, achieved advance per tooth =
  `effective_feed / (rpm x flutes)` (`chipload.rs:49`, `display.rs:40-49`,
  `distribution.rs:616-640`), and the card's bounds are the gate's
  (`distribution.rs:28-33`). Both sides of the ratio are one measure.
- The band is a point per toolpath (A2): 0.1016 mm printed, 0.1077
  after the hardness scale, de-rated once at the peak axial DOC
  (`chipload.rs:586-620`), drawn as a zero-width ceiling.
- MEASURED at HEAD on this file (probe RED CHECK, nine ops): the
  population maximum equals the ceiling to 0 ulps on every op; raw `>`
  count 0; `exceeds_high` count 0. The modulator parks the feed on the
  point (Z22: 66 % of moves `ChiploadMax`, median delta +0.02 %;
  finish: +33 %, from 725 to the band ceiling 967 mm/min). So no
  chipload column can be red from the saved file. Chip thinning is
  deleted (`feed_modulation.rs:472-495`), entries and transit are
  filtered (`chipload.rs:214,309-333`), the card and gate thresholds are
  one; candidates (a), (c), (d) are false.
- What the operator saw is therefore not the saved state. The one
  state this review can reproduce is the optimizer's own result: the
  refined candidates (its "final passes") carry a red badge
  (`optimize_project.rs:503-513`, `optimize_modal.rs:913-929`) and the
  text "every candidate hit a gate limit (chipload, power, or
  deflection)" (`outcome.rs:839`), while the row that tripped is DEPTH.
  An applied deep candidate paints the "Depth of cut" card red the same
  way. A stale GUI binary is the other candidate (operator memory: a
  stale GUI fails silently).
- Latent instrument defect, not firing here: the card paints a column
  red on a raw `value > ceiling` (`histogram.rs:471-479`) while the gate
  uses `exceeds_high` with the boundary slack (`boundary.rs:102-105`,
  G-CHIP-ULP). On a point band a 1-ulp round trip would paint red with
  the gate `Within`.
- Caution: `median_feed_delta_pct` is taken over the moves whose feed
  differs from the commanded feed, plunges included
  (`feed_modulation.rs:388-392`); the cavity op reports -71.8 % while
  15 506 of 15 531 cutting samples run at the commanded 1 673.

Decision: not a real over-chipload. The red is the optimizer's own
depth-vetoed candidates mislabeled as a chipload/power/deflection trip,
or a depth card, or a stale build; the chipload instrument on this file
is flat on the point.

## 5. The right lever order, and where the code departs

1. Strategy and retract policy: stay-down links, per-level retract
   clearance, `safe_z` only between levels and tools. No lever (F2).
2. Deepest DOC and widest WOC inside the envelope: vendor `ap` rule,
   deflection at its bound, power at the ceiling. Anchored at 1.4 x
   baseline and vetoed by the rule of thumb (F1, F3); no force line (F4).
3. Feed for the target chipload at that DOC and WOC: point or band,
   de-rate, hardness, dial, then G6/G10 ramp and plunge. Held (F5).
4. RPM inside the chart bracket, feed scaled with it. Done first and
   alone, past the chart (F9).
5. Simulate at the project resolution with the shipped modulation; fix
   local overload per move, never by a global derate (F7).
6. Verify the full cut: removed volume, uncut area, collisions (F8).

The current order is 4 -> 2 (narrow, then vetoed) -> 5 (another
instrument). Steps 1, 3 and 6 are absent.

## 6. Packages

P1. The depth veto and the narrative (smallest change, largest win on
this file). Scope: the optimizer judges a candidate by the gates with a
bound from a source (`BoundSource` sourced), and the depth rule of thumb
is reported, not a veto, as S3 ruled for export; `GateKind` gains
`Depth` so the row and the explanation name it; the composite score
carries the depth term so a vetoed cell cannot fill stage 2. Acceptance:
on the mini project `optimize_toolpath` on toolpath 2 returns `Ranked`
with dpp >= 1.2 mm and cycle under 700 s, and a depth reading on the
row; `the_depth_that_cut_is_a_measured_load_g_s3depth` green. Risk: a
deep cut on a tool with no force line (P3 must land with or before it).

P2. Retract and link lever. Scope: per-level `retract_z` auto (level +
clearance; `safe_z` between levels and tools) for pocket, face and 2D
clearing; `link_moves` on for same-level ring hops through cut ground
(`dressup/link.rs`). Acceptance: Z22 channel rapid Z travel from
9 931 mm to under 1 500 mm at unchanged removed volume and zero
collisions; `capability_link_moves_safety`, `plunge_guard_p3` green.
Risk: a link through standing stock; keep the corridor test conservative.

P3. The material refusal is visible and routed. Scope: a
`SolidWoodByJanka` stock resolves a density (Janka to an FPL row, or a
named proxy on the card), or the optimizer refuses with
`MaterialUnvalidated` and says why. Acceptance: the mini project shows
power and deflection readings, or a refusal naming `rivmap feeds_oak.toml`.
Risk: a wrong proxy; cite the row.

P4. DOC and WOC bounds from the model. Scope: the grid's DOC bounds come
from `feeds::cutter_constraints` (vendor `ap` rule, deflection at
`EXCEEDS_BOUND_MM`, flute length), anchored on the envelope, not on
baseline x1.4. Acceptance: with P3, toolpath 2 gets a candidate near
2 mm with deflection `Within` and cycle under 500 s. Risk: the 30 mm
stick-out; the bound must be the model's.

P5. Feed follows DOC and WOC. Scope: every grid cell re-derives feed,
plunge and ramp feed through the Suggest recipe at the cell's DOC and
WOC. Acceptance: a 2.5 x D cell ships 0.875 x the feed and passes the
gate on its first simulation; `one_depth_derate_for_feed_and_band_fm3`
green. Risk: two producers; the Suggest clamp must be the one.

P6. One measure. Candidate sims at the project resolution with the
shipped modulation, or the baseline re-scored at the candidate setting.
Acceptance: a candidate sim of the baseline params equals the on-screen
cycle within 1 %. Risk: sim cost (1.3 s here; measure on rivmap100).

P7. Full-cut verification. A candidate carries removed volume, uncut
area, collision count and the "crosses material" finding against the
baseline; a regression disqualifies it. Acceptance: a thin last level
or a gouge is never `Ranked`. Risk: dexel noise; tie the tolerance to
the cell size.

P8. RPM inside the chart. `k_lut` uses the printed RPM (the machine
ceiling only when none is printed) and honours `spindle_strategy`.
Acceptance: on `match_chart` the headroom candidate keeps 18 000 rpm.

P9. The card's boundary rule. `histogram::value_side` uses the gate's
`exceeds_high` / `below_low` with the slack. Acceptance: a population at
the point to 1 ulp draws no red column.

## 7. Measured versus inferred

Measured (this session, HEAD `b20d947d`, release): the setup-1 times
and intent split, retract count and Z travel, entries, the DOC sweep
(0.5946 / 0.83 / 1.5 / 3.0), the gate verdicts and chipload populations
on all nine ops with the ulp-level red check, the modulation summaries,
the deflection what-if on three force lines, one real `optimize_toolpath`
run on toolpath 2 (`PROBE.md`). Read in code: every `file:line` above.
Inferred: the retract saving (Z travel at the Z rate; the sim's own
rapid accounting gives 692 s), and the attribution of the operator's
red graph to the optimizer's candidate badge, a depth card or a stale
build, which only the operator's screen can settle.
