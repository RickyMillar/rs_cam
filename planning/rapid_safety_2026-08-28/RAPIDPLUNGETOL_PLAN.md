# G-RAPIDPLUNGETOL — design (2026-09-25; Option A implemented, see RESULTS)

Decision (lead, operator delegated 2026-09-25): **Option A**. Implement
after G10 Part B lands (one cargo build at a time; the disk is tight).

Defect: `RapidClearanceCheck::strikes` flags iff
`pz < max_clearance_tip_z_for_profile(..) − cs·(1/√2 + 0.5 + 1.0)`
(`crates/rs_cam_core/src/stock/collision.rs:664,735,743-746`). A tip 2.207
cells into material under the whole footprint is not flagged (1.104 mm at
0.5 mm cells, 0.552 mm at 0.25 mm).

## 1. Root cause — the three slack terms and their true Z effect

Notation: axis c, tip z, profile h(r) non-decreasing, envelope R, cell cs,
hd = cs/√2, cell centre distance d. True clearance
C* = sup_{|p−c|≤R} [T(p) − h(|p−c|)]. Computed (dexel_stock/mod.rs:487-545):
C_high = max over cells with d ≤ R + cs/2 (mod.rs:498,529) of
conservative_top_k − h(max(d − hd, 0)) (mod.rs:533-540).

a) Half-diagonal (mod.rs:516,533). Lateral: the cell's material is placed at
   r_near = d − hd. Z effect = h(r_true) − h(r_near) ≤ h(min(d+hd,R)) − h(d−hd).
   Flat tool: h ≡ 0 for r ≤ R (tool/flat.rs:67-73), so **0**. Ball near the
   rim: √(2·R·hd) (recorded in tests/rapid_live_check_crest_s2.rs header).
b) Half-cell dilation (mod.rs:498). Lateral: visits cells whose centre lies up
   to R + cs/2 away. For a flat tool r_near ≤ R − 0.207cs, h = 0, and the cell
   is charged at its full top. If that cell's material lies outside R (a kerf
   wall 0 to 0.5 cells past the rim), the Z effect is the **full wall height
   above the tip**: unbounded, not 0.5 cs. If it lies inside R: **0**.
c) "One cell of conservative_top over-read". conservative_top is lowered
   only when one stamp covers the cell (coverage ≥ FULL_COVERAGE,
   stamping.rs:340; dexel.rs:291-293), to depth_max + h(d + hd)
   (stamping.rs:375-380). Z over-read on a fully covered cell =
   (depth_max − local tip) + (h(d+hd) − h(d)): **0** for a flat tool on a
   horizontal pass. On a partly covered cell it keeps the pre-cut top, which
   is exact as a maximum over the cell (the uncovered part still stands).
   The real Z residue is coverage-union cells, which several partial stamps
   clear together. That residue is not bounded by one cell.

Conclusion: none of the three terms is a Z quantity proportional to cs. For a
flat tool the query's errors are 0 or a full-height lateral false positive at
the rim. A constant Z tolerance fixes neither; inside the footprint it only
buys false negatives (shallow plunges; the header of
tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs:22-25).
Minor lateral gap, out of scope: a rim point at 45° can sit in a cell whose
centre is R + hd > R + cs/2 away and is unvisited (worst 0.207 cs radially);
changing the reach moves every link-planner caller of the primitive.

## 2. Options

A. **Two-channel check.** Add `TriDexelStock::clearance_bounds_for_profile
   (cx, cy, radius, cutter, lut) -> Option<(Option<f64> low, f64 high)>`,
   sharing the existing cell loop. high is byte-identical to today. low = max
   over cells with d + hd ≤ min(radius, lut radius) and a non-empty ray of
   ray_top_k − h_lut(d + hd). Flag iff `z < low − ε` OR `z < high − τ`
   (τ unchanged).
   - Conservative: a superset of today's flags (the OR keeps the old test).
     low ≤ C*: ray_top is the cell's area-weighted mean (stock/dexel.rs:86-97),
     mean ≤ max, so material stands at ≥ ray_top somewhere in the cell, at
     ≤ d + hd from the axis and inside R, where the tool is ≤ h(d + hd) above
     its tip.
   - ε = 2·f32::EPSILON·max(|low|, 1) (the ray is f32). h comes from the SAME
     `RadialProfileLUT::from_cutter(cutter, LUT_SAMPLES)` the stamp used
     (compute/simulate.rs:1523, radial_profile.rs:16), built once in
     `RapidClearanceCheck::new`, so a cell stamped at z' + h_lut(d') and
     re-entered on the same axis reads ≤ z'.
   - Benign classes: the rim / own-kerf class lives in partly covered cells at
     d > R − hd, which the low channel excludes; τ still suppresses exactly
     what it does today. New exposure: (i) coverage-union residue at kerf-edge
     crossings (≤ depth/4); (ii) the swept kernel evaluating a descending
     segment's tip at t_center (swept.rs:38-41,601), residue behind a ramp end
     that a later horizontal stamp erases. Both are measured on rivmap100 (§4).
   - Cost: one ray_top read per inside cell in a loop that already runs. The
     stale-then-flush evaluation (simulation.rs:179-190,540-558) stays sound:
     stamping only lowers ray_top, so low is monotone.
B. **Per-cell slack on conservative_top** (h at near and far points, τ = 0 on
   inside cells). Does nothing for flat tools (h ≡ 0 both ways) and flags pure
   conservatism with τ = 0. Strictly worse than A.
C. **τ = 0 plus a sub-cell-exact query** (per-cell coverage masks). Fixes the
   rim false positive too, but changes the data model and the stamping hot
   path. Keep as the follow-up to S2's "sub-cell-aware clearance query".

## 3. Sentries (A)

New file `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs`
(re-use the `board` / `wall_state` helpers of the g_rapid6497 file). Flat Ø6,
R = 3, TOP = 8, FLOOR = 0, cs ∈ {0.5, 0.25}, phases {0, .25, .5, .75}·cs.
τ(0.5) = 1.10355, τ(0.25) = 0.55178 (cs·(1/√2 + 1.5)).

1. `a_shallow_plunge_under_the_whole_footprint_is_a_strike`: fresh board,
   descend 19 → 8 − e, e ∈ {0.3, 1.0}. The axis cell has d ≤ hd, so
   d + hd ≤ 0.707 cs ≤ R: inside. ray_top = 8, low = 8, 8 − e < 8 − ε.
   Red before for (0.3, both cs) and (1.0, 0.5): 7.7 ≥ 6.896 / 7.448 and
   7.0 ≥ 6.896. (1.0, 0.25) is already green (7.0 < 7.448).
2. Same through `simulate_toolpath_with_lut_metrics_rapid_checked` and the
   playback walk: `vec![1]`.
3. `a_shallow_plunge_half_under_a_block_is_a_strike`: `wall_state(face = cx)`,
   e = 0.3. Cells with centre in [cx, cx + R − hd] are Full and inside: low = 8.
4. Benign `a_descent_to_a_cut_floor_is_clear`: all Floor, descend to z = 0.0
   exactly. low = high = 0; 0 < 0 − ε is false. Pins the sign of ε.
5. Benign `a_wall_just_outside_the_rim_is_not_a_plunge_strike`:
   `wall_state(cx + R + 0.05)`, descend to 8 − τ/2. Every Sliver/Full cell has
   d ≥ R + 0.05 − cs/2 > R − hd: none inside, low = 0 < z; high = 8 and
   z ≥ 8 − τ: silent before and after. The one class τ still exists for.
6. Primitive: `low ≤ high` on every fixture above.
7. Primitive, curved tool: a tapered ball cuts three horizontal passes
   (y = 8, 12, 16) at z_c; query at (x_mid, 12); low ≤ z_c + ε.

Must stay green: g_rapid6497 (b)(c)(d), rapid_live_check_crest_s2 (all 4),
air_filter_tool_aware_s3, entry_descent_profile_b2. Update the g_rapid6497
header "Known limit" paragraph and the collision.rs:644-663 doc.

## 4. Blast radius and measurement

- Production caller: only `strikes` (collision.rs:735). The tolerance fn has
  no other user. `max_clearance_tip_z_for_profile` is unchanged for its other
  callers (dressup/air_cut.rs:94, dressup/mod.rs:776,
  finish/surface_link.rs:173, compute/execute/project_curve_chaining.rs:549).
- `RapidClearanceCheck::new` builds the LUT internally; construction sites are
  unchanged. `RapidCollision` is unchanged. Count readers (simulate.rs,
  sim_prefix memo, diagnostics, sim_triage, sim_measurability, CLI
  project.rs:817/877 verdict "error", GUI preflight / readiness / status bar,
  MCP app/mcp/simulation.rs) need no code change; a new flag turns a project
  verdict to error.
- Tests pinning zero / baseline counts that may go red (triage each as a
  strike first, never raise τ): adaptive3d_lift_bridge_b1,
  pocket_lift_bridge_b1, adaptive_feed_modulation_pipeline_f036b,
  face_stock_top_frame_f028, p1_headless_ab_wanaka, perf_golden_sim_metrics,
  classification_columns_ab_m3, ramp_contained_in_region_g_rampcontain.
- rivmap100 (`planning/fixtures/rivmap100/rivmap100_live_0925.toml`, release
  CLI, resolution 0.5 then 0.25): rapid_collision_count per toolpath before
  and after. For each new flag dump XY, z, low, high, depth = low − z and the
  argmax cell's ray_top / CT / cover. TRUE strike if it persists at cs/2 and
  cs/4; residue if its depth shrinks with cs. Residue is fixed in the emitter
  or the sim kernel, never with a Z constant.

## 5. 2D Adaptive rapid reorder: a load defect (separate package)

- Its catalog row (compute/catalog.rs:589) leaves `allows_rapid_reorder`
  true; barriered reorder (dressup_apply.rs:504-521) permutes runs inside each
  depth level (barriers at DepthPass starts, trace/toolpath_spans.rs:846-851).
- The planner is order-dependent: one MaterialGrid per level, cleared as
  passes are emitted (adaptive/material_grid.rs:140-156); keep-down `Link`
  feeds are admitted only through cells cleared by EARLIER runs
  (adaptive/path.rs:401-407, 1029, 1290). The TSP splits only at rapids
  (dressup/tsp.rs:116), so a Link can run before the run that cleared its
  corridor: a Linking feed through uncut material at full depth, and
  over-engaged passes. The rapid check cannot see it (rapids stay at safe Z);
  the cut trace's engagement can.
- Action: measure a 2D adaptive fixture with reorder on vs off (peak radial
  engagement, Linking samples in material), then veto as Adaptive3d does
  (`.without_rapid_reorder()`) with a sentry modelled on
  `session_rough_keeps_the_planner_order_for_its_entries`. Check Rest too.
- RESULTS (2026-09-26, on 73af1a2c): the defect is real. Fixture (in
  `tests/adaptive_keeps_its_planner_order_g_adaptorder.rs`, instrument
  `measure_reorder_on_against_off`): 120 x 80 pocket, six r 8 islands, 6 mm
  end mill, Depth/Pass 3 over 6, sim 0.5 mm. Reorder off / on: cut order
  changed; links over 6 x R (admitted only through a clear corridor) cut
  61.7 / 349.3 mm^3 in 18 / 122 samples, peak radial 0.30 / 0.64; all 22
  links: 1056 / 1162 mm^3, 172 / 268 samples; ClearingCut peak radial 0.93 /
  0.93; straight EntryPlunge in material 0 / 2 (a 1.8 mm^3 sliver at the
  wall); cycle 263.3 / 253.4 s. The planner order is not link-clean: 20 of
  22 links cut material by design (a link under 6 x R may cross material).
  Adaptive split out of the shared catalog row with `.without_rapid_reorder()`.
  Rest: each scan segment is framed by its own rapids (`ops/rest.rs`
  `emit_rest_segment`), no keep-down link, no planner stock: not vetoed.
- G-ADAPTLINKLOAD (2026-09-26, operator ruling "don't plough unless the
  link can genuinely do a legit cutting move with defined load"): every
  keep-down link of the 2D Adaptive operation is walked in pass steps on the
  planner grid and admitted only when no step reads above the pass ceiling
  (`adaptive/search.rs` `pass_engagement_ceiling`, target x 1.05, in the
  planner's engagement measure); an admitted link is stamped, a refused one
  retracts and re-enters. The cleanup replay decides every kept link again.
  adaptive3d slices keep the old rule (`KeepDownLinks::RetractedByCaller`:
  they lift every 2D link to a retract). Same fixture, planner order, before
  / after: fed links 22 / 14, in material 20 / 12, samples 172 / 34, volume
  1056 / 89 mm^3, peak link radial 0.90 / 0.42, links over 6 x R unchanged
  (6, 18 samples, 61.7 mm^3, 0.30), retracts 32 / 40, cycle 263.3 / 277.7 s.
  Sentry `tests/a_link_feeds_only_within_the_pass_load_g_adaptlinkload.rs`.

## RESULTS (2026-09-25, Option A, uncommitted working tree on 07777325)

Code: `TriDexelStock::clearance_bounds_for_profile` and the private
`clearance_scan` (dexel_stock/mod.rs) — one loop; `max_clearance_tip_z_for_profile`
now returns its `high`. `RapidClearanceCheck` builds the LUT in `new` and
flags `z < low − ε || z < high − τ` (stock/collision.rs).

Every file:line claim in §1–§4 checked against 07777325: correct.

Sentries, `tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs`
(7 tests). With the low channel switched off (the pre-fix verdict) items 1, 2
and 3 fail (first failing case of each: cs 0.5, phase 0, e 0.3; the walk
returns `[]`); items 4–7 pass. With both channels: 7/7 pass.

Kept green: g_rapid6497 4/4, rapid_live_check_crest_s2 4/4,
air_filter_tool_aware_s3 4/4, entry_descent_profile_b2 4/4,
profile_link_ceiling 4/4.

§4 pinned-count tests:

| Test | Result | Rapid count, high only → both |
|---|---|---|
| adaptive3d_lift_bridge_b1 | pass | 0 → 0 |
| pocket_lift_bridge_b1 | FAIL, pre-existing | 3 → 3 (moves 114, 268, 423, all high-channel) |
| adaptive_feed_modulation_pipeline_f036b | pass | 0 → 0 |
| face_stock_top_frame_f028 | pass | 0 → 0 |
| perf_golden_sim_metrics | FAIL, pre-existing | 59 fields moved (entry/helix metrics), the same 59 with the low channel off; no rapid field among them |
| classification_columns_ab_m3 (`--release --ignored`) | pass | A 0 → 0, B 0 → 0 |
| ramp_contained_in_region_g_rampcontain | pass | unchanged |
| p1_headless_ab_wanaka (`--ignored`) | FAIL, pre-existing | not reached: "ladder made no progress at round 1; still pending: [4, 5, 6, 8]", identical with the low channel off |

rivmap100 (`rivmap100_live_0925.toml`, release CLI `rough-score`, no
`--toolpath`; Scallop and Project Curve are disabled in the file, so two
toolpaths are simulated). `rapid_collision_count`:

| Toolpath | 0.5 before | 0.5 after | 0.25 before | 0.25 after |
|---|---|---|---|---|
| Face 5 (id 7) | 0 | 0 | 0 | 0 |
| 3D Rough ladder 8 > 2 (id 1) | 0 | 0 | 0 | 0 |

New flags to triage: none (the low-channel-only diagnostic printed no
record on any run). The residue classes (i) and (ii) of §2 A did not appear
on this fixture.

G-ENTRYORDER (2026-09-25): `pocket_lift_bridge_b1`'s 3 flags at 1.0 mm
cells are the first measured instance of the high-channel residue class: a
union-cleared cell held at the pre-cut height, 4 mm (one level) > tau 2.2 mm;
0 flags at 0.5 / 0.25 / 0.1 mm (PROGRESS.md roughing stream 2a).

