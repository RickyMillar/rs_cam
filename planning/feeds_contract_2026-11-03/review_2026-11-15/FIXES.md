# Fixes of the review of 2026-11-15

The lead rulings at the end of `REVIEW.md` set the scope. Each step has a
predictions section, written and measured before its code patch, and a
results section.

## R1 (F1): a fold ramp never goes below the cut it follows

### Cause

`dressup/entry_descent.rs` `collect_following_cut` took a tab lift and the
tab traverse into the run of the fold. `walk_fold_path` dropped the lift
vertex (it has no XY length), and `fold_ramp_points` held the ramp above the
cut only at a vertex. The return chord to the low vertex crossed the tab
below its top.

### Rules

1. The cut that a fold follows ends at a rise: a move of the cut whose end
   is more than `FOLD_RISE_EPS_MM` (1e-6 mm) higher than its start (a tab
   lift, a lead-out, a link). A fold never runs over a tab. A run that is
   too short for the fold takes the fallback that exists: the air lead and
   the stated plunge.
2. Every emitted fold point, and every point of each emitted segment, is at
   or above the followed cut at that position. `walk_fold_path` gives a
   vertex with no XY length the higher Z of the two points at that XY, so
   the vertex clamp holds the chord on both sides of it.

### Predictions (measured with the fix in place, before the patch)

- Operator file `nz_wanaka_21_operator.toml`: ops idx 0, 4 to 13 and 17
  dumped with the reviewer's probe (`zz_review_probe.rs`, no export) at HEAD
  and with the fix. HEAD dumps equal the reviewer's `now_<idx>.csv` byte for
  byte. Prediction: 0 of 12 ops move.
- Op golden (`every_op_keeps_its_toolpath_g_opgolden`): only the known 46
  rows of `opgolden_known_46_rows.txt`. Prediction: no R1 row. No re-bless.
- FM1 (`feeds_matrix_instrument_fm1`): no cell moves (the matrix holds no
  fold ramp).
- The other three project files: not generated (each op takes minutes).

### Tests

- `--lib dressup::tests::a_fold_ramp_never_runs_over_a_tab_review_f1`: the
  reviewer's reproduction. Red at HEAD (a ramp point at x 14.29 over the
  tab span x 3 to 20), green with the fix.
- `--lib dressup::tests::no_fold_ramp_point_is_below_the_followed_cut_review_f1`:
  400 generated cuts with lines, arcs and tabs (vertical and sloped lifts,
  traverses on a line or an arc), four angles, three tool radii, with and
  without the lap cap. Each emitted ramp segment is sampled at 41 points
  against the cut at that XY (arcs sampled from their own centre and
  angles). Red at HEAD (case 0: a sample at z -1.243 against the cut at
  -1.204 at that XY), green with the fix.

### Results

- Operator file: 0 of 12 ops move (all 12 dumps byte-equal to HEAD). As
  predicted.
- Op golden: only the known 46 rows. As predicted. No re-bless.
- FM1: 2 passed, no cell moves. As predicted.
- Gates: the 72 test binaries whose names hold entry, ramp, tab, clip,
  isoclip, boundary, dressup, helix or plunge pass, one at a time. Core
  `--lib`: 2820 passed, 1 failed (`the_envelope_meets_its_time_budget`, the
  known timing red of a parallel run).
- Not verified: the other three project files (no generation). The property
  test reads the lowest cut Z within 0.03 mm of a sample, so where a
  generated cut crosses itself, a floor segment can hide a tab top at the
  crossing.

## R2 to R5: predictions (rows, tones, faces, export warnings)

Measured with the code of each step in place, before the patches, with a
row dump probe (`zz_probe_rows_review.rs`: the P29-11 probe on the door of
P30, `ProjectSession::limit_rows(index, None)`) on the four project files,
READ-ONLY, no simulation: every envelope row (state, tone, caveats, face,
detail), the verdict rows, the Passes card face line (the worst stated row
by `rank_cmp` that exceeds) and, from R4 on, the export warning words of
`stated_cut_over_a_limit`.

### R2 (F2): `WorstCase` only where the value reads the ramp's own depth

- Rows: on each ramp row of a chip maximum, a chip minimum, a rubbing
  floor or a vertical rate, the caveat `worst_case` goes, and nothing else
  on the row changes. Count (planned and stated rows): operator 88,
  wanaka21 88, rivmap100 16, rivmap100_live 16 (22, 22, 4, 4 rows for each
  of the four limits). The depth ceilings and the power keep the caveat.
- Tones: 10 rows move from `exceeds_advisory` to `exceeds` (the safe side).
  No row moves to a softer tone.
  - operator idx 8 ANTENNA: ramp entry chip max (planned and stated);
  - wanaka21 idx 2 and idx 17: ramp entry rubbing floor (planned, stated);
  - rivmap100 and rivmap100_live idx 0 Face 5: ramp entry rubbing floor
    (planned and stated).
- Faces: a row that loses `worst case` shows its next caveat word or none
  (operator 66, wanaka21 82, rivmap100 12, rivmap100_live 12 faces).
- `worst case` becomes a caveat of the row, not of the cut: the drawer
  title of a ramp cut does not name it, and a depth row names it on its own
  line (`face_in_group`).
- Explainer data (`P29-10_explainer/data/`): the CLI `suggest --json` of the
  two files moves only `limit_rows[].caveats` (operator 88, live 16),
  `detail` (88, 16), `face` (66, 12) and `tone` (2, 2), the same rows as
  above. Claim 6 of `assert_claims.py` states the new rule.

### R3 (F3): `LowerBound` softens nothing

- No row of the four files changes (no lower-bound row exceeds there).

### R4 (F4): the export warns for every stated row with the gating tone

- No row changes. The warning `export.stated_feed_over_limit` becomes
  `export.stated_cut_over_limit` (wire change). With no simulation, no op
  has a current measured verdict. Predicted warnings, one per op (from the
  stated rows of the R3 dump with tone `exceeds`: the worst by rank and the
  count of the others; 0 mismatches against the product door):
  - operator: idx 1 (planner cap chip max 24 %, 1 more), idx 2 (chip max
    2.9 %), idx 7 (body chip max 27 %, 1 more), idx 8 (body chip max 27 %,
    2 more), idx 14 (chip max 3.4 %), idx 15 (chip max 3.1 %). The export
    of the reviewer's probe holds results for idx 0, 3 to 13 and 17 only,
    so it warns for idx 7 and idx 8, as at HEAD; ops 1, 2, 14, 15 are left
    out (`export.op_left_out`).
  - wanaka21: idx 2 (stated point, moves 78 % over limit, 1 more), idx 16
    (body chip max 50 %), idx 17 (ramp entry rubbing 48 % under floor; it
    gates since R2).
  - rivmap100: idx 0 (ramp entry rubbing 44 %), idx 1 (helix entry rubbing
    44 %), idx 2 (body rubbing 17 %), idx 3 (body rubbing 17 %, 1 more).
  - rivmap100_live: idx 0 (ramp entry rubbing 44 %), idx 1 (stated point,
    depth 33 % over vendor depth cap), idx 2 (body rubbing 17 %), idx 3
    (body rubbing 17 %, 1 more).
- Op golden: 49 `gcode_hash` rows move (the header warning lines). No
  `moves_hash`, `spans_hash`, feed or count row moves.
- The claim 3 of `a_row_softens_only_under_a_current_modulation_p30_2`
  changes: a fresh simulation with modulation off is a current measured
  verdict, so its export names the measured chip row (`load.chipload.*`)
  and no stated-cut warning.

### R5 (F5, F6)

- No row of the four files changes: no planned deflection row there lacks
  a chip or a width, no cut is past its search span, no file holds a
  current modulation, and no simulation runs (so (d) cannot show).
- Op golden: as after R4.

### The 18 Passes card face lines of the operator file (before; after R2 to R5)

| idx | op | before | after |
|---|---|---|---|
| 0 | Face back to T1 | none | none |
| 1 | Back bulk rough 18 mm | Exceeds: planner cap · chip max 24 % over limit · until modulated | Exceeds: planner cap · chip max 24 % over limit · until modulated |
| 2 | Back rest 6 mm (cavity, hollow, lakes, pockets) | Exceeds: planner cap · chip max 2.9 % over limit · until modulated | Exceeds: planner cap · chip max 2.9 % over limit · until modulated |
| 3 | Pin drill, flip pins | none | none |
| 4 | Drill light holes 6 mm end mill | none | none |
| 5 | River channel 6 mm, pass 7 of 7 | none | none |
| 6 | Lake pocket 2 mm, 11 lakes to Z 5.88 | ExceedsAdvisory: ramp entry · chip min 76 % under floor · worst case | ExceedsAdvisory: ramp entry · chip min 76 % under floor · may not occur |
| 7 | Lake pocket 2 mm, 1 lake to Z 3.99 | Exceeds: body · chip max 27 % over limit · until modulated | Exceeds: body · chip max 27 % over limit · until modulated |
| 8 | ANTENNA_D5.00 pocket 2 mm | Exceeds: body · chip max 27 % over limit · until modulated | Exceeds: body · chip max 27 % over limit · until modulated |
| 9 | V rivers and lakes | none | none |
| 10 | Day strip slit V60 | none | none |
| 11 | Face clamp pads 6 mm | ExceedsAdvisory: ramp entry · chip min 66 % under floor · worst case | ExceedsAdvisory: ramp entry · chip min 66 % under floor · may not occur |
| 12 | Face front to T | none | none |
| 13 | Engrave day labels V60 | none | none |
| 14 | Front bulk rough 18 mm | Exceeds: planner cap · chip max 3.4 % over limit · until modulated | Exceeds: planner cap · chip max 3.4 % over limit · until modulated |
| 15 | Front 3D rough | Exceeds: planner cap · chip max 3.1 % over limit · until modulated | Exceeds: planner cap · chip max 3.1 % over limit · until modulated |
| 16 | Front scallop finish | ExceedsAdvisory: body · chip min 16 % under floor | ExceedsAdvisory: body · chip min 16 % under floor |
| 17 | Outline profile with tabs | ExceedsAdvisory: plunge entry · plunge rate 17 % over limit · may not occur | ExceedsAdvisory: plunge entry · plunge rate 17 % over limit · may not occur |

Two faces change, by word only, with the same tone: idx 6 and idx 11 read
"may not occur" in place of "worst case" (the chip minimum of a ramp is
exact; the cut may not occur).

### R5 (e): cuts marked "may occur" that exceed with no later gate

The "may occur" tone rule does not change (lead ruling). For the operator:
these stated rows exceed and read advisory because the cut may not occur,
and no sample gate judges their limit after a simulation (`gate_of` gives
none for the rubbing floor, the vertical rate and the vendor depth table;
row `S-NoSampleGate`). The planned rows of the same cuts read the same.

| file | op | limit | row | face |
|---|---|---|---|---|
| operator | tp6 Lake pocket 2 mm, 11 lakes to Z 5.88 | rubbing_floor | U-EntryFallback | ramp entry · rubbing 46 % under floor · may not occur |
| operator | tp6 Lake pocket 2 mm, 11 lakes to Z 5.88 | rubbing_floor | U-EntryFallback | helix entry · rubbing 46 % under floor · may not occur |
| operator | tp7 Lake pocket 2 mm, 1 lake to Z 3.99 | rubbing_floor | U-EntryFallback | ramp entry · rubbing 46 % under floor · may not occur |
| operator | tp7 Lake pocket 2 mm, 1 lake to Z 3.99 | rubbing_floor | U-EntryFallback | helix entry · rubbing 46 % under floor · may not occur |
| operator | tp16 Front scallop finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 73 % under floor · may not occur |
| operator | tp17 Outline profile with tabs | plunge_rate | U-EntryFallback | plunge entry · plunge rate 17 % over limit · may not occur |
| wanaka21 | tp2 Back rest 6 mm (cavity, hollow, lakes, pockets) | rubbing_floor | U-EntryFallback | helix entry · rubbing 48 % under floor · may not occur |
| wanaka21 | tp16 Front scallop finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 77 % under floor · may not occur |
| wanaka21 | tp17 Outline profile with tabs | rubbing_floor | U-EntryFallback | helix entry · rubbing 48 % under floor · may not occur |
| rivmap100 | tp0 Face 5 | rubbing_floor | U-EntryFallback | helix entry · rubbing 44 % under floor · may not occur |
| rivmap100 | tp1 3D Rough | rubbing_floor | U-EntryFallback | ramp entry · rubbing 44 % under floor · may not occur |
| rivmap100 | tp2 Scallop Finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 80 % under floor · may not occur |
| rivmap100_live | tp0 Face 5 | rubbing_floor | U-EntryFallback | helix entry · rubbing 44 % under floor · may not occur |
| rivmap100_live | tp2 Scallop Finish | rubbing_floor | U-EntryFallback | helix entry · rubbing 80 % under floor · may not occur |

The other rows that exceed under "may occur" (chip, power, force budget,
deflection) have a sample gate that judges the cut after a simulation.

### R2 results

- Cause: `CheckRow::planned` pushed `WorstCase` on every ramp row with its
  own depth, and the caveat softens.
- Fix: `PlannedOwnDepth::worst_case` holds only where
  `feeds::planned::PlaneRole::reads_the_cut_depth` is true (a depth ceiling
  or the power; each value check reads no depth: the doc states the inputs
  of each judge). `WorstCase` is a caveat of the row.
- Proof: claim 5 of `one_record_on_every_surface_p29_10` asserts literal
  tones: a ramp chip maximum at 1.5 x the band maximum is `Exceeds` with no
  `WorstCase`; the far ramp chip minimum carries only
  `AdvisoryFloor { OnePrintedValue }` and reads `ExceedsAdvisory`; each ramp
  depth row with a number keeps `WorstCase`. Claim 1 names the four limits
  that read the depth. Before the fix the chip maximum row read advisory.
- Rows: as predicted (all four files: only `worst_case` drops, on 208
  rows; 10 tones move to `exceeds`; no tone softens). Explainer data
  regenerated; `assert_claims.py` 11 of 11 PASS.

### R3 results

- Cause: `Caveat::softens` listed `LowerBound`.
- Fix: `LowerBound` softens nothing; it stays on the row to explain the
  value.
- Proof: `every_limit_judges_every_declared_cut_p29_7` claim 7b, a Project
  Curve on a 2 mm end mill (stickout 50 mm) whose lower-bound depth is over
  the deflection root: `Exceeds`, caveat `LowerBound`, tone `Exceeds`. Red
  with the R2 `row.rs` (tone `ExceedsAdvisory`), green with R3.
- Rows: no change on the four files, as predicted.

### R4 results

- Cause: `stated_row_over_until_modulated` kept only the stated rows with
  `UntilModulated`, which only the chip maximum takes.
- Fix: `ProjectSession::stated_cut_over_a_limit` keeps every stated row with
  `Tone::Exceeds` (the worst by `rank_cmp` and the count of the others);
  `Gate::StatedCutOverLimit` warns for an emitted op that has no simulation
  fresh for it (`sim_trace_is_fresh_for`). Wire changes: the warning id
  `export.stated_feed_over_limit` is `export.stated_cut_over_limit`; the
  presentation face "Stated cut over a limit: no current simulation"; the
  detail "<face>; <n> more stated rows over a limit; no current simulation
  measured this op"; `Gate::UnmodulatedOverLimit` is
  `Gate::StatedCutOverLimit`; the session door and its memo hold
  `StatedCutOverLimit`.
- Proof: `a_stated_cut_over_any_limit_warns_review_f4` (a Pocket whose
  stated plunge feed is over the vertical-rate limit and whose chip is in
  its band: one warning that names the plunge rate, in the list and in the
  header; HEAD gave no warning for this op). The P30-2 sentry claim 3 now
  states that a fresh simulation names the measured chip row.
- Warnings: as predicted on the four files (0 mismatches). Op golden: the
  49 predicted `gcode_hash` rows. The 89 dumped golden programs: 0 bytes
  above 127, 0 comment lines with an inner or unbalanced parenthesis; the
  longest warning line is 124 characters.
- Open, for the lead (measured after the patch, not changed): the rule
  skips an op with a simulation that is fresh for it. A limit with no
  sample gate (the vertical rate, the rubbing floor, the vendor depth
  table; row `S-NoSampleGate`) and a stated-point row then warn nowhere:
  the F4 fixture after a simulation with modulation off exports with no
  warning while its stated plunge-rate row gates (294 % over its limit).
  The alternative is to keep those limits in the warning when the op has
  a fresh simulation. The comment in `export.rs` states the gap.
- Departure from P30-2: a superseded modulated result has no current
  measured verdict, so it now gets this warning too (P30-2 gave it no
  second line beside the feeds caution).

### R5 results

- (a), (b) Cause: `DeflectionPlanned::judge` gave one `None` for "no root"
  and "no chip or width", and the envelope read no root as `Within` at any
  depth; the force budget read "no root below the flute" as `Within` at any
  depth. Fix: `DeflectionRoot` (root, no root in the span with its end, no
  width or chip, no signal) from the typed search
  `cutter_constraints::deflection_depth_search` (the same evaluations; the
  other readers keep `invert_deflection`); two reasons, `NoWidthOrChip` and
  `PastTheSearchSpan` (wire change: two `NoNumberReason` values). A row
  past the span end (deflection: `min(0.95 x stickout, flute)`; force
  budget: the flute length) is `NoNumber(PastTheSearchSpan)`; at or above
  it, `Within` stays (no root in the span proves it). Proof: claim 7c of
  `every_limit_judges_every_declared_cut_p29_7` (no chip, no width: the
  typed answer; a 6 mm Project Curve at 20 mm against an 18 mm span: past
  the span). Not built: a force-budget fixture past the flute (the code
  path is the same rule).
- (c) Fix: `RunModulation::after_an_apply` (an apply drops the result, so a
  current or an older modulation becomes `NotRun { export: Stated }`); the
  proposal rows read it. Proof: `a_proposal_row_does_not_soften_under_the_current_modulation_review_f5`
  in the P30-2 sentry (after a modulated run the proposal chip row carries
  `UntilModulated { Stated }`, the stated row `BeforeModulation`).
  Two sentries asserted the old rule and change with it:
  claim 8b of `one_record_on_every_surface_p29_10` (after a modulated run
  the stated rows read "before modulation" and the proposal rows keep
  "until modulated") and claim 3 of
  `every_declared_cut_is_judged_at_the_stated_point_p29_10` (a proposal row
  under a current modulation keeps the capped set as `UntilModulated` and
  the tone of `Off`; its non-vacuity of "softened" now comes from one case
  at three times its stated feed, since no stated capped row of the cases
  exceeds). Both failed on the first gate run with R5 and pass now.
- (d) Fix: the depth gate counts each sample with no force model and the
  detail names the count and the reason (`no_number_clause`). Proof: lib
  test `a_sample_with_no_force_model_is_counted_and_named_review_f5`
  (literal clauses). Not built: a trace with a refused sample beside a
  capped one.
- (e) The list is above; the tone rule does not change.
- Rows: no change on the four files, as predicted. Wire change: a
  proposal row under a current modulation carries `until_modulated`
  (export `stated`), not `before_modulation`. The MCP tool description of
  `get_safe_envelope` states the F2, F3 and F5 (c) caveat rules.

## R6 (F8): every fed move is in a class of `EmittedFeeds`

### Predictions (measured with the code in place, before the patch)

- No move, no row and no feed changes: the record and the header only.
- `EmittedFeeds` gains the classes drill, air lead (`approach`), lead and
  link; each class holds its move count (`EntryFeeds::moves`; wire change
  of MCP and CLI `emitted_feeds`). `FeedClass::of` names every move intent.
- The header of a drill phase gains one line, `(drill feeds run <F>
  mm/min, the stated feed)`, or the largest drill feed against the stated
  one. An air-lead line shows only where the largest air-lead feed is not
  the stated plunge feed: no golden arm and no operator op has one.
- Op golden: `gcode_hash` of the drill arms moves: `alignment_pin_drill/fresh`,
  `drill/fit_mixed`, `drill/fresh`, `drill/simple_cycle` (new rows) and
  `drill/rest` (its R4 value moves again). No other row.
- Operator export (the reviewer's probe, idx 0, 3 to 13 and 17): two new
  header lines, `(drill feeds run 1000 mm/min, the stated feed)`, under idx
  3 and idx 4.
- FM1: no cell moves (FM1 reads no export header).

### R6 results

- Cause: `figures_of` counted only the moves of three cut-kind classes; a
  drill move, an air lead, a lead and a link were in no class.
- Fix: `session::FeedClass::of` (one match over every `MoveIntent`, no
  wildcard) puts every fed move in one class; `EmittedFeeds` holds `drill`,
  `approach`, `lead` and `link` with `EntryFeeds::moves`, and `fed_moves`
  sums them. A drill phase prints `drill_line`; an air-lead line shows
  where its feed is not the stated plunge feed.
- Proof: `every_fed_move_is_in_a_feed_class_review_f8` (every op kind on the
  standard fixture: the class counts sum to the fed moves of the result; a
  drill op's header prints "drill feeds run"); lib test
  `every_feed_class_is_the_class_of_its_cut_kinds_review_f8` (the classes
  against `intents_of`, a literal table).
- Golden: the 5 predicted drill rows. Operator export: the two predicted
  drill lines; every other line equal (below).

## R7 (F9): the guard sees four more ways round a token; narrower doors

### Predictions (measured with the code in place, before the patch)

- Byte-equal: no product behaviour changes. Golden, FM1, the rows and the
  operator moves do not move.
- The scan (`tests/common/op_type_scan.rs`, shared by the core, viz and
  CLI guards) reads four more forms: an alias of a type of
  `TYPE_TOKENS`, an import of its variants, a call of a helper that returns
  an op type, a family or a role (`helper_names`; the viz and CLI guards
  read the core helpers too), and `Self::Variant` in an `impl` of one.
- Reads that the tree held and the old scan did not see:
  - core: three variant imports, in `feeds/geometry_class.rs` `classify`
    (class G) and `feeds/support.rs` `formula_backing` (class V, two
    imports). Both functions have an entry: no new entry, X total 86.
    The core helper outside the tokens is `family` (no call in a guarded
    file).
  - CLI: six callers of the parse helpers `operation_type`,
    `parse_op_type` and `materialize_case_toolpath` (each returns the op
    type): `job.rs` `execute_op_via_session`, `job_op_session`,
    `parse_job_file`; `run.rs` `run_generic`; `smoke.rs`
    `run_single_case_inner`; `sweep.rs` `patch_job_param`. Six new W
    entries (op parsing, the class of the helpers themselves).
  - viz: none.
- Narrowed: `Reentering::none`, `Reentering::stay_plunges` and
  `PlungesStay` are `pub(crate)`; `PlungesStay::NoOperation` exists only
  under `test` or `test-support`; `EntryPlungeFeed::of_op_plunge_rate` is
  `pub(crate)`. Test doors (`test-support`): `Reentering::none_for_tests`,
  `Reentering::plunges_with_no_operation_for_tests`,
  `EntryPlungeFeed::of_op_plunge_rate_for_tests`. Nine integration tests
  move to the doors (no assertion changes).

### R7 results

- Cause: the scan matched 17 tokens as words and cut each `use`, so an
  alias, a variant import, a helper call and `Self::Variant` were not
  reads; the entry doors were `pub`.
- Fix: as predicted. Proof:
  `the_scan_finds_the_four_blind_spots_review_f9` plants one read of each
  form (the scan of HEAD has no function that finds them; each assert
  names the token and the function).
- The core guard: 5 passed (X total 86, no new entry). The CLI twin: six
  new W entries, as predicted. The viz twin: no change.

## Final checks of the whole chain

- Operator file: idx 0, 3 to 13 and 17 dumped with the reviewer's probe on
  the final tree: 13 of 13 dumps byte-equal to the reviewer's
  `now_<idx>.csv`. The export of the same session: every line that is not a
  comment equals the reviewer's `export.nc`; the comment lines differ only
  by the renamed warning (2 lines, R4) and the drill lines (2 lines, R6).
  `gck.py`: 0 bytes above 127, 0 bad comment lines, the longest comment 126
  characters (the reviewer measured 128).
- Gates on the final tree (one cargo job at a time): fmt check clean on the
  changed files; core sentries `one_record_on_every_surface_p29_10` (10),
  `every_limit_judges_every_declared_cut_p29_7` (12),
  `every_declared_cut_is_judged_at_the_stated_point_p29_10` (4),
  `the_exemption_list_only_shrinks_p29_3` (6, pins unchanged),
  `no_limit_reads_the_op_type_p29_last` (5),
  `a_row_softens_only_under_a_current_modulation_p30_2` (4),
  `the_record_states_what_the_gcode_runs_p30_1` (3),
  `one_entry_rule_two_emitters_p29_8` (6),
  `modulated_feeds_read_their_own_prefix_b5` (2), the new
  `a_stated_cut_over_any_limit_warns_review_f4` and
  `every_fed_move_is_in_a_feed_class_review_f8`, and the nine tests that
  moved to the test doors: all pass. Core `--lib`: 2822 passed, 1 failed in
  the parallel run (`the_envelope_meets_its_time_budget`, the known timing
  red); alone 3 of 3 pass. `every_op_conforms_w4`: 11 passed, check 6 fails
  (the known red). `rs_cam_mcp` 39 passed; `rs_cam_cli` all pass (the twin
  guard 2). Viz `--lib` 704 passed (no GUI ran); the viz binaries
  `the_card_shows_one_planned_row_and_a_drawer_p29_10`,
  `the_inspector_text_is_bounded_ic1`, `no_surface_matches_a_limit_p29_10`,
  `readiness_shows_the_limits_g_readylimits`,
  `a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`,
  `export_states_the_feed_stage_w2_5`, `mcp_wire_surface_pin` and the twin
  guard pass. FM1: 2 passed. Op golden: the known 46 rows and the 53
  predicted `gcode_hash` rows (49 of R4, 4 new of R6, `drill/rest` moved by
  both). Full clippy: clean.
- Op golden re-bless (not done here; the lead runs it): a field-only bless
  of `gcode_hash` on the 53 arms of the scratchpad list `bless_arms.txt`.
  Two of them also hold known-46 rows of other fields
  (`adaptive3d/by_area_helix_cp`, `unified_finish/claims_rest_clip`); the
  field-only bless leaves those rows as they are.

## R8: the export warning keeps the limits that no sample gate judges

### Cause

R4 (`155a25ee3`) skips `export.stated_cut_over_limit` for an op that a
simulation covers (`sim_trace_is_fresh_for`), because the sample gates then
judge the cut. The vendor depth table, the rubbing floor and the plunge
rate have no sample gate (`tool_load::groups::gate_of` gives none; row
`S-NoSampleGate`). After a simulation, a stated row of those limits with
the gating tone warns nowhere.

### Rule

1. With no simulation that is fresh for the op, the warning keeps every
   stated row with the gating tone (R4, no change).
2. With a fresh simulation, the warning keeps only the stated rows of a
   limit that `tool_load::groups::judged_only_before_the_simulation` gives:
   a limit with a planned role (a depth ceiling or a value check) and no
   gate in `gate_of`. Claim 1 of `every_limit_judges_every_declared_cut_p29_7`
   asserts that this function equals the limits of the row
   `S-NoSampleGate`. A new sample gate for one of them removes it from the
   function, and its warning stops.
3. The detail after a fresh simulation ends "no sample gate of the
   simulation judges this limit"; with no fresh simulation it ends "no
   current simulation measured this op" (no change).
4. The face of the id is true in both states: "Stated cut over a limit: no
   measured gate" (it was "Stated cut over a limit: no current
   simulation"). The G-code header line prints this face.

### Predictions (measured with the code in place, before the patch)

Probe on the two test fixtures (`zz_probe_r8.rs`, deleted): the stated rows
with the gating tone and the export warnings, with no simulation, a
simulation with modulation off, and a simulation with modulation on.

- Fixture of `a_row_softens_only_under_a_current_modulation_p30_2` (Pocket,
  Ramp, feed 9000 mm/min):
  - No simulation: no change. The warning names "stated point · feed cap
    125 % over limit; 3 more stated rows over a limit; no current
    simulation measured this op".
  - Modulation off (claim 3): GAINS one warning, "Toolpath 'pocket': ramp
    entry · rubbing 44 % under floor; no sample gate of the simulation
    judges this limit". The two chip maximum rows and the feed cap row do
    not count (the chip gate judges the chip; `load.chipload.high` stays).
  - Modulation on (claim 2): GAINS the same warning, same text.
  - So the claims 2 and 3 of the sentry change: each asserts that literal
    text and that no chip row and no "more stated row" clause is in it.
- Fixture of the R4 test (Pocket, no entry style, plunge 9000 mm/min):
  - No simulation: no change ("plunge entry · plunge rate 294 % over
    limit; no current simulation measured this op").
  - Modulation off: GAINS "Toolpath 'pocket': plunge entry · plunge rate
    294 % over limit; no sample gate of the simulation judges this limit"
    (the reviewer's case).
  - Modulation on: GAINS the same warning.
- Op golden: golden arms run no simulation, so no arm gains or loses a
  warning. But the header line prints the face of the id, and the face
  changes: the `gcode_hash` of each arm that carries the warning moves (the
  49 arms of R4). No `moves_hash`, `spans_hash`, feed or count row moves.
  The alternative (keep the old face) moves no row, but its header then
  says "no current simulation" under a fresh simulation.
- FM1: no cell moves (FM1 reads no export warning).
- The four project files with no simulation: the same warnings as after
  R4; only the face word of the header line changes.
- Task 2 (rivmap100, simulated in the GUI): the export after the
  simulation keeps a stated-cut warning for each op whose stated rubbing
  row gates (R4 list: idx 0 to 3), with the R8 detail.
- Open (not in this ruling): a stated row of a limit that is not a limit
  of each cut (the p30_2 fixture: "stated point · feed cap 125 % over
  limit", `CuttingFeedCap`) still warns nowhere after a fresh simulation.

### R8 results

- Fix: `tool_load::groups::judged_only_before_the_simulation` (a planned
  role and no gate in `gate_of`) is the one list. Claim 1 of
  `every_limit_judges_every_declared_cut_p29_7` asserts, for each
  `LimitId`, that it is true exactly for the limits of the row
  `S-NoSampleGate`. `ProjectSession::stated_cut_over_a_limit` takes a
  `SimulationCover`; the memo keeps the gating stated rows worst first
  with their limits, and `FreshSimulation` keeps only the rows of that
  function. The export door passes `FreshSimulation` when the simulation
  is fresh for the op, and does not skip the op.
- Proof: `a_stated_vertical_rate_over_its_limit_warns_after_a_simulation_review_r8`
  in `a_stated_cut_over_any_limit_warns_review_f4` (the reviewer's case:
  the F4 fixture, a simulation with modulation off, literal detail and
  header line). Red with the old skip put back in `export.rs` (no
  warning), green with the fix. Claims 2 and 3 of
  `a_row_softens_only_under_a_current_modulation_p30_2` assert the literal
  rubbing warning; both were red with the old skip.
- Warnings: as predicted (the four gains, the same texts; no change with no
  simulation). rivmap100 in the release GUI of this tree (generate_all, one
  run_simulation, export): four `export.stated_cut_over_limit` warnings
  with the R8 detail, idx 0 "ramp entry · rubbing 44 % under floor", idx 1
  "helix entry · rubbing 44 % under floor", idx 2 "body · rubbing 17 %
  under floor", idx 3 "body · rubbing 17 % under floor; 1 more stated row
  over a limit". As predicted (the R4 list).
- Op golden: the known 46 rows and 49 `gcode_hash` rows, the 49 arms of
  R4 (the face word of the header line). No other field moves. Not
  blessed here: the field-only bless of `gcode_hash` on those 49 arms is
  the lead's step.
- FM1: 2 passed, no cell moves. As predicted.
- Gates: fmt check on the changed files; the core sentries
  `a_stated_cut_over_any_limit_warns_review_f4` (2),
  `a_row_softens_only_under_a_current_modulation_p30_2` (4),
  `the_record_states_what_the_gcode_runs_p30_1` (3),
  `every_limit_judges_every_declared_cut_p29_7` (12),
  `one_record_on_every_surface_p29_10` (10),
  `the_exemption_list_only_shrinks_p29_3` (6),
  `no_limit_reads_the_op_type_p29_last` (5); core `--lib` 2822 passed, 1
  failed (`the_envelope_meets_its_time_budget`, the known timing red of a
  parallel run); `rs_cam_mcp` 39; `rs_cam_cli` all pass; viz `--lib` 704;
  the viz binaries
  `a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`,
  `export_states_the_feed_stage_w2_5`, `mcp_wire_surface_pin` pass; full
  clippy clean.
- Open: a stated row of a limit that is not a limit of each cut (for
  example `CuttingFeedCap`, "stated point · feed cap 125 % over limit" on
  the P30-2 fixture) still warns nowhere after a fresh simulation.
