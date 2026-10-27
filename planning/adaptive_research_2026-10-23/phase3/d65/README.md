# D65: tidy the op-1 re-entry rows with a feed cap (design only, no code)

Code read: the PRODUCT on geodesic-integration `7ee3d3226` (`adaptive/path.rs`, `adaptive/search.rs`, `adaptive/geodesic/slice.rs`, `adaptive3d/clearing.rs`, `adaptive3d/mod.rs`). Data: fix34 (Corne B s2.1, op 1 = geodesic, its log and `descent_causes_corne_B_s2p1.json`). All line numbers are at `7ee3d3226`.

## 1. The rows and their refusals

The C40 records of op 1 (`index=0`, 398 records) put into the rows of the operator's image (`g2_v6/img/corne_entries_after.png`):

| row | records | causes | Z (levels) | drop |
|---|---|---|---|---|
| inner ledge, y −70..−66, x > 85 | 50 | Reentry2d/Turn 33, LargeZ 14, HeldLinkRefused 2, Waterline 1 | 18.5, 11.0, 5.0, 4.0 | 0.5–15 mm |
| bottom wall row, y −81..−76, x > 88 | 26 | LargeZ 22, Reentry2d/Turn 4 | 17.0–17.56 | 5.9–6.5 mm |
| top wall rows, y > −10 | 53 | LargeZ 42, Reentry2d OverBound 6 / Mixed 4 / CoreHandOff 1 | 17.0–18.2 | 5.3–6.5 mm |

### Row A: the ledge, "Reentry2d/Turn". A 2D step refused over the pass ceiling.

- The refusal: `emit_capped_walk_with` (`adaptive/path.rs:568`), `let holds = reading <= step_pass.ceiling || load.admits_turn(a, b);` (`:631`).
  - A step over the ceiling that is not on a planned turn ends the span (the `else` arm, `flush`).
  - The walk re-enters further on with `AdaptiveSegment::Rapid(a)` (`:701`), and its hole is stamped (`stamp_entry`).
  - `admits_turn` (`:217`) admits only steps on a planned turn no longer than `turn_bound` (`:60`, `s + (1 + √2)·cell`).
- The readings (fix34 `c22_lines_corne_B_s2p1.txt`, the §19.3 Part 4 probe, "step refused"): 12 distinct sites at y = −68.264, x 88.0–136.4, repeated on 3 levels (36 lines).
  - The readings are 0.383–0.518 against `ceiling` 0.3805 (s 2.1, R 3). All have `on_turn_mm=None`: they are straight steps, not turns. The C40 label "Turn" is the cause class, not the geometry.
  - The lead quoted 0.22–0.32 against 0.219. Those numbers are from a lower-stepover run, not from fix34; the same rule reads them.
  - Over the whole op there are 63 "step refused" lines, all with readings ≤ 0.52, and 131 "link refused" lines (keep-down links, `feed_link_to_legal`, `:389`).
- What the reading is: the replay's `measure_step` under `PassLoad::swept_width` (`search.rs:845`). The product's capped-feed reading is `slice::step_reading` (`geodesic/slice.rs:123`), the larger of the swept width and the area per mm over D (the one load `L`). On a straight full-depth cut both equal the radial extent over the diameter, so a full slot reads 1.

### Rows B and C: the walls, "LargeZ". A 3D draped step split by its load.

- The refusal: the 3D slice's split of a lifted path (`adaptive3d/clearing.rs:3601-3690`).
  - A step is shape-flagged when `dz > max(1.1·DPP, 1 mm)` or its slope is over `PLUNGE_SLOPE_LIMIT` 0.3 (`:3601`, `:3620`).
  - Under `StepSplit::StockDepth` (the Geodesic default, `adaptive3d/mod.rs:333`), the flagged step is split when `deep` (`step_axial_depth > DPP + stay_down_clearance`) OR `step_load(..) > ceiling c` (`:3628-3655`).
  - The split emits `Adaptive3dSegment::Rapid(p3)` at the step (`:3687`). `plan_entry` makes that a LiftOver entry with cause LargeZ.
- Which branch fired: op 1's log says "C22 split entries (stock below the entry, planner read) split_entries=481 over_dpp=0 max_stock_mm=6.000". No split entry had more than one DPP (6.0) of stock below it, so `deep` never fired. **Every LargeZ split in op 1 is a load split (`step_load > c`)**, and the wall-row drops (5.3–6.5 mm) are one DPP.
- What it is: at the outside of the part walls, the draped path steps down the wall into the layer that op 1 is clearing. The step's chip-area load, `chip_load_fraction` (`tool_load/load_fraction.rs:36`; 1 is the full-depth slot), is over c but not deep.

## 2. The rule: admit a refused step at a capped feed, up to the full slot

The product already admits two kinds of step over the ceiling with a cap:
- a planned turn (`admits_turn`);
- a departure out of a straight plunge (`PassLoad::departing`, `search.rs:931`: the ceiling is set to infinity).
`feed_caps` (`slice.rs:173`) then writes `Move::feed_cap` with the factor `feed_factor(c, L) = c / L` (`slice.rs:144`), which holds the steady removal rate (`F' = F·c/L`).

The rule extends that set to one more kind, for Geodesic Adaptive only:

> **A step whose load reads `c < L ≤ 1` is admitted, at the feed `F · c / L`.**

- **The hard limit `L ≤ 1` is derived, not typed.** 1 is the full-depth slot, the largest load the op already cuts: every straight plunge's departure is a slot out of its own hole (the `departing` exemption), and it is cut today at the capped feed. A step over 1 (only possible on the area term at a tight turn) stays refused. There is no new constant: c is the op's pass ceiling, 1 is the slot, and the factor is the existing `feed_factor`.
- **A 2D step (row A).** In `emit_capped_walk_with`, `holds` becomes `reading <= ceiling || admits_turn(a, b) || load.admits_capped(a, b, reading)`. `admits_capped` is true only when the load carries the capped rule (a `LinkLoad` field set by the geodesic slice door, as `with_turns` is) and the step's `step_reading` (the max of the two terms) is ≤ 1. `fed_runs` (`slice.rs:254`) adds each such step as a run, with its own reading, so `feed_caps` caps it. The invariant "a step over the ceiling carries a cap" holds by the same replay.
- **A 3D step (rows B, C).** In the StockDepth branch, a flagged step with `!deep` and `c < step_load ≤ 1` is not split. It is emitted as its own `Cut` between `FeedCapStart`/`FeedCapEnd` markers, with the cap `min(F·c/L, plunge_rate · |step| / |dz|)`.
  - The second term holds the step's DESCENT rate at or under the op's plunge rate, so a steep step down the wall descends no faster than the vertical plunge it replaces. The radial load is at most a slot, and the axial is at most one DPP plus the clearance (not `deep`). No new number: `plunge_rate` is the op's.
  - `deep` stays a hard split, as now.
- **The keep-down links (131 "link refused").** These are not in scope. The same rule could admit a link whose load reads `≤ 1`, but a link is a transit by design, and its refusal guards the O5b and G-PHANTOMSTAMP class. That should be its own item.

## 3. Expected effect on Corne B s2.1 (fix34; an estimate, to be measured)

- Entries removed:
  - row A: the 33 Turn records ("step refused", L 0.38–0.52);
  - rows B and C: the 64 wall-row LargeZ splits (22 + 42), and the 14 LargeZ at the ledge, all load splits. Each is admitted if its `step_load ≤ 1`. That is expected, because the axial is ≤ one DPP, but the instrument does not log `L` per split, so this is unconfirmed.
- Seconds (from `descent_causes`: mean descent time per record, LargeZ 0.395 s, Reentry2d/Turn 0.931 s):
  - Row A: about 33 × 0.93 ≈ 31 s of entry descent (60 % air), plus their retracts and rapids. The capped steps add little: each is one pass step (3 cells), at a factor c/L of 0.73–0.99.
  - Rows B and C: the descent itself stays (the capped step descends at ≤ the plunge rate, like the plunge). What goes is the retract, the rapid and the lift-over of each entry. Not measured: `descent_causes` holds descent moves only. The upper bound of the descent time involved is 78 × 0.395 ≈ 31 s.
  - Op 1 total in fix34: 1075 s (D54 table). Expect about 3–5 % off op 1, mostly from row A.
- Load change: an admitted step reads up to 0.52 (row A) or up to 1 (rows B, C) at the feed c/L. Its removal rate per mm of travel × feed is held at c·F (the shear term of the power model, as for the turns now). The radial engagement is higher than c on those steps, and the chip thickness per tooth is lower by c/L. Peak `L` over the op rises from c to the largest admitted reading. The audit (`a_geodesic_cap_survives_every_dressup_s7_4`, the cap audit of fix41 `cap_audit.py`) must show every step over c carries a cap ≤ c/L.

## 4. What it touches, and what must not move

- `adaptive/path.rs`: the `holds` test in `emit_capped_walk_with` and a new `LinkLoad` field and method (`admits_capped`). This is the SHARED cleanup of every 2D strategy, so the rule is off unless the field is set, and only the geodesic slice door sets it (as `with_turns`).
- `adaptive/geodesic/slice.rs`: `fed_runs` adds the capped steps; `feed_caps` is unchanged.
- `adaptive3d/clearing.rs`: the StockDepth branch at `:3628-3690`. StockDepth is the default only for Geodesic Adaptive (`effective_step_split`, `adaptive3d/mod.rs:333`). Gate the admit on the strategy too, so an explicit StockDepth on another strategy keeps the split.
- Must stay byte-identical:
  - CP, AgentSearch, Adaptive, ContourSpiral: `adaptive3d_emission_byte_parity` (all five cases are non-geodesic) and the op golden (its adaptive3d rows are CP/AgentSearch).
  - 2D Adaptive: `adaptive/` sentries.
- Will move (pre-register before code): the geodesic pins, `the_geodesic_preset_keeps_its_plan_s7_3` and `geodesic_adaptive_keeps_the_research_geometry_s7_4`. They hold the plan move by move, and fewer re-entries change them. Report the moved pins with their first differing move; re-bless only on the lead's word.

## 5. Sentries

1. Unit (`adaptive/path.rs` tests): a planted step with `c < L ≤ 1` on a load with the capped rule is admitted (no `Rapid`); the same step with the rule off is refused (as now); a step with `L > 1` is refused with the rule on.
2. Unit (`geodesic/slice.rs` tests): `feed_caps` gives an admitted capped step the factor `c / L` exactly, and a step at or under c gets no cap.
3. Session (Geodesic, a ledge fixture: a pocket with a step in it like Corne's inner ledge): every emitted step over the ceiling carries `feed_cap ≤ F·c/L`; no step over 1 is emitted; the re-entry count at the ledge falls (red on `7ee3d3226`).
4. Session (3D, a wall fixture): a load-split step at the outside of a wall (not deep, `L ≤ 1`) becomes one capped cut whose descent rate is ≤ the plunge rate; a DEEP step (stock > DPP + clearance) still splits into an entry.
5. Parity: `adaptive3d_emission_byte_parity` 5/5 and the op golden unchanged, which proves CP and the others do not move.
6. Measurement (D10): Corne B s2.1 and wanaka s3.5 on the product before and after: op-1 entries by cause, the entry seconds, max `L`, and the cap audit (every step over c capped).

## 6. Build (lead approval), on geodesic-integration, base `7ee3d3226`

Patch: `integration/d65_tree_vs_7ee3d3226.patch` (12 files, md5 in the report). The pre-registration is `PREREG.md`. Not committed.

### What is built

- `adaptive/path.rs`:
  - `FULL_SLOT_LOAD` (1, the full-depth slot; doc derives it);
  - `LinkLoad::capped_area` and `with_capped`, and `admits_capped` (both terms of `L` at most a slot);
  - `holds` gains `|| load.admits_capped(..)`;
  - the three `*_turns` / `replay_after_group_filter` functions take `capped`, which goes on the REPLAY load only, as the turns do. The residue contours, the wall pass and the mop keep the strict ceiling.
- `adaptive/geodesic/plan.rs` `cleaned(.., capped)`. `slice.rs` passes `true` (`slice_cleanup`, `slice_replay`). The 2D door `geodesic_toolpath` passes `false`, because it writes no feed cap. `research_geodesic.rs` passes `false`.
- `adaptive/geodesic/slice.rs` `fed_runs`: a replayed step off every turn and departure whose swept width reads over c makes its plan window a capped run, of its own key, before the departures.
- `adaptive3d/clearing.rs`:
  - `admits_capped_split` (Geodesic only, not deep, c < L ≤ 1, inside one engaged run);
  - `capped_step_factor` = min(c/L, plunge·|step|/(|dz|·F), the open 2D cap, 1);
  - the admitted step goes out as its own `Cut` between `FeedCapStart` and `FeedCapEnd`, and the 2D cap is reopened after it.

Found while building: `feed_caps` matches a step to its run by POSITION, not direction (`run_of`). A cut that retraces a capped window exactly gets the same cap. That is conservative (a lower feed), and real plans do not retrace a window. The unit test walks back beside the cut.

### Sentries

| sentry | what it holds | red |
|---|---|---|
| `adaptive::path::link_load_tests::a_step_over_the_ceiling_is_admitted_only_capped_and_up_to_a_slot_d65` | admitted with the rule on (reading 0.75 D, over c, under a slot); refused with the rule off; refused at 2 × slot | fails with `admits_capped` disabled |
| `adaptive::path::link_load_tests::the_walker_cuts_a_capped_run_in_one_span_d65` | a 0.75 D edge run: one entry and one span with the rule on; nothing with it off | fails with the rule disabled |
| `adaptive::geodesic::tests::feed_caps_caps_a_step_over_the_ceiling_d65` | a cut through stock is capped (0 < f < 1); a cut beside it under c has no cap | fails with the `fed_runs` capped runs disabled |
| `adaptive3d::tests::a_draped_step_is_capped_only_up_to_a_slot_and_never_when_deep_d65` | deep splits; over a slot splits; at or under c is no admission; an engagement change splits; AgentSearch, CP, Adaptive and ContourSpiral split | pure predicate |
| `adaptive3d::tests::a_capped_draped_step_descends_no_faster_than_the_plunge_d65` | shallow step factor c/L; steep step descent ≤ plunge rate; the 2D cap binds; a climb has no descent term | pure function |
| `tests/a_geodesic_step_over_the_ceiling_is_cut_capped_d65.rs` (session, shelf fixture) | entries < 46 (base pin; this tree: 18); every ClearingCut fed below F carries its cap | base: 46 entries, fails `n < 46`; with the 2D rule disabled: 34 (3D part alone) |

### Shipped output

- Byte-identical, by test: CP, AgentSearch, Adaptive (2D-in-3D) and ContourSpiral (`adaptive3d_emission_byte_parity`, 5 of 6 cases unchanged under bless); the op golden 1/1 (it has no geodesic row). 2D Adaptive: its sentries pass, except two failures that are identical on the base (below).
- Moved, re-blessed (approved), Geodesic only:

| pin | old → new | first differing move |
|---|---|---|
| `tests/fixtures/adaptive3d_emission/geodesic_adaptive.txt` | 2019 → 1829 moves | move 27: old `Linking` lift to (−1.500, 1.232, 12.500) at F; new `ClearingCut` to (−2.000, 1.801, 10.445), capped (feed = cap ≈ 0.513 F) |
| s7_4 "island pocket, one level" | (978, 14452218181729557987) → (957, 5931675112365951456) | move 210: old lift to (−9.144, −8.883, 10.5) at F, no cap; new cut to (−8.238, −8.883, 4.5) at cap 631.0 |
| s7_4 "island pocket, two levels" | (2063, 3602713506842892098) → (1976, 17577482464951561279) | move 212: old lift to Z 10.5; new cut to (−8.238, −8.883, 7.0) at cap 631.0 |
| s7_4 "shelf, one level" | (1056, 14786195423799852005) → (1008, 6172482053443525791) | move 210: as the one-level pocket |
| s7_4 "channel, two levels" | (0, 675868731199239589) unchanged | — |

  The s7_4 module doc now says the pins leave the research numbers on purpose for D65, and keeps the research counts in the text.
- Not moved: `the_geodesic_preset_keeps_its_plan_s7_3` (2/2; it pins the 2D door, where the rule is off); `a_geodesic_cap_survives_every_dressup_s7_4`; `a_trace_lists_each_capped_move_s7_4c`.
- The C12 pass-load judge on Geodesic, heavy tests run by name:
  - `geodesic_adaptive_holds_the_pass_load_at_two_levels` ok (511 s; capped moves keep the feed rule);
  - `the_stock_read_split_holds_the_exact_bound_on_draped_steps` ok (32 s);
  - `the_stock_read_split_adds_no_over_move_on_the_shelf` ok (113 s);
  - `no_entry_plunges_more_than_a_pass_into_stock` ok (16 s);
  - `no_held_link_cuts_standing_stock_with_the_group_filter_on`: not run to the end (over the 290 s step limit).

### Gates (all through this worktree's `scripts/cargo_lane.sh`, in the foreground)

- fmt; clippy, full features with and without research: ok.
- Parity 6/6 plain and research (after the geodesic re-bless); the op golden 1/1.
- The 13 adaptive3d folder sentries; f152; f140: ok.
- `rs_cam_core --lib` 2719 plain, 2751 research: ok.
- 2D adaptive sentries: ok except `a_straight_plunge_entry_is_exempt_only_leaving_its_hole` (pass-load test) and `a_rapid_leaves_cut_depth_straight_up` (rapid-lift test). BOTH fail identically on `7ee3d3226` (checked with the base code of all 8 touched source files): they existed before, and D65 does not cause them.

### For D10 (after D64)

Corne B s2.1 and wanaka s3.5 on this patch against `7ee3d3226`:
- op-1 entries by cause (expect the ledge Reentry2d "step refused" and the load-split LargeZ to fall);
- the entry seconds;
- the max `L` of the capped moves (≤ 1);
- the cap audit (every step over c capped).
