# V4 — the power bar returns, as a 0-to-limit bar

Implemented 2026-09-18, against `SURFACE_IMPL.md` §2 step V4 and the
operator's ruling 2 of 2026-09-18 (`RESUME_PLAN.md` §7). The Feeds card
carries one power row again. The sentry arm that banned it is replaced by a
test of the reason it was banned.

Not committed. The orchestrator reviews and commits.

---

## 1. What the row is

One row beside the chipload verdict row, in the same idiom:

| Part | Content | Source |
|---|---|---|
| Label | `Power \u{24D8}` | `tokens::GLYPH_DETAIL`, as every explained row |
| Face | a 0-to-limit bar, `required_kw` to `available_kw`, with the percent printed on the bar | `PowerFigure` |
| Caption | `machine` | `BoundSource::MachinePowerCurve::setting()` |
| Hover | the kW pair, the percent, the provenance clause, the setting, and the four figures of the operating point | `PowerFigure` + `BoundSource::clause()` |

The figure comes from
`feeds::power_at_operating_point(operation, tool, material, machine,
fallback)` on the operation the card is showing — the state the machine
receives after `enforce_invariants` — with the preview's recommendation as the
`CalculatorOperatingPoint` fallback. It never reads `FeedsResult::power_kw`.

No number on the face is typed. The percent is formatted from `required_kw`
and `available_kw`. The GUI owns no limit: the ceiling is
`PowerFigure::available_kw`, and `BoundSource::MachinePowerCurve` is built
from the figure's own `rpm` and the machine's own `safety_factor` — the two
numbers the post-simulation power gate stores on its verdict (S4).

The bar's FILL is clamped to the ceiling, because a bar cannot draw past its
end. The TEXT is not clamped, because a cut over its ceiling must read over
its ceiling.

A `PowerUnmodeled` paints its own `clause()` after `tokens::GLYPH_UNKNOWN`, in
the dim abstention style the chipload row uses for a refusal. No bar, no zero,
no blank. The hover names the missing input and the typed refusal variant.

---

## 2. Red first

The four new arms were run against the unmodified `compare.rs`:

```
test result: FAILED. 8 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.19s

failures:
    the_power_bar_is_informative_g_chipverdict
    the_power_row_reads_zero_to_the_limit_g_chipverdict
    the_power_row_states_a_real_reading_g_chipverdict
    the_power_row_states_its_refusal_g_chipverdict
```

The first failure names the defect directly:

```
assertion `left == right` failed: the thin fixture painted 0 power rows. One
operating point has one power reading.
  left: 0
 right: 1
```

### The spread arm was not red by construction, so it gained a rendered arm

The brief expected the spread arm to be red on HEAD because no bar exists.
It was not. S2 landed `power_at_operating_point`, so an arm that measures
utilisation through the door is green before any row renders it. The arm
therefore gained a fourth assertion that measures the BAR: the heaviest recipe
in the population is rendered on the card it ships on, and the painted percent
must pass half scale. With that assertion the arm is red on HEAD with the
other three.

---

## 3. The measured spread

Taken from the run, through the same doors the row uses: the Suggest funnel
writes each operation, `power_at_operating_point` reads the point it ships.
The record prints under `--nocapture`:

```
G-CHIPVERDICT power spread | 270 recipes: min 0.6 %, median 12.1 %, p90
56.6 %, peak 100.0 %; 34 over 50 %. Peak recipe: Shapeoko (1.5kW VFD) /
Jarrah / Ø12 / adaptive 3d
```

| | Pre-R1 (the ban's evidence) | The review, post-R1 | This run |
|---|---|---|---|
| Population | 3 presets × 10 species × 3 diameters | 162 recipes | 270 recipes |
| Median | about 1 % | 17.8 % | **12.1 %** |
| p90 | — | 89.4 % | **56.6 %** |
| Peak | 23.6 % | 100.0 % | **100.0 %** |
| Over 50 % | — | 25 % | **12.6 %** (34 of 270) |

The two post-R1 columns disagree on the middle of the distribution and agree
on both ends. Two causes, and neither is a model difference:

1. **The population is wider.** This run walks all ten shipped species, not
   six. The four extra species are lighter, so they sit low and pull the
   median and the p90 down.
2. **The reading path is the CARD's, not the calculator's.** The review swept
   `feeds::calculate` output. This run takes each recipe through
   `feeds::suggest::apply_feeds_result_to_op` first and then reads the shipped
   operation, which is what the row does. The rigidity clamp lowers the depth
   on the heavier recipes, and a lower depth draws less power.

The reinstatement condition holds on both readings: the peak reaches the
ceiling exactly, 34 recipes pass half scale, and the spread runs from 0.6 % to
100 %. The assertion is written against the weaker half of the review's
condition — that half scale is reachable at all — because the FRACTION above
half scale is a property of the population, and this population is wider than
the one the fraction was measured on.

The peak sitting at exactly 100.0 % with nothing above it is the Step 6 power
ladder working: it clamps a power-limited recipe onto the ceiling rather than
letting it through. That is why a 100 % reading on this row is a real reading
and not an absent constraint painted as an all-clear, and the row's doc says
so.

---

## 4. Every arm of the chipverdict file, before and after

| Arm | Before | After |
|---|---|---|
| `the_feeds_tab_paints_exactly_one_verdict_row_g_chipverdict` | one `Chip` row on all four fixtures | unchanged |
| `the_verdict_row_carries_its_own_explanation_g_chipverdict` | detail mark and a multi-line hover | unchanged |
| `the_verdict_changes_state_across_fixtures_g_chipverdict` | thin ≠ in-band | unchanged |
| `the_units_stay_on_the_hover_g_chipverdict` | no `J/mm³` on the face | unchanged |
| `no_power_gauge_returns_to_the_card_g_chipverdict` | scans `compare.rs` for `ProgressBar`, `power_bar`, `power_color`, `rail_power_row`; asserts no ` kW` anywhere on the card | **replaced by the four arms below** |
| `no_fabricated_number_survives_a_refusal_g_chipverdict` | the no-Kc chip row states its refusal | unchanged |
| `the_verdict_row_states_real_numbers_g_chipverdict` | non-vacuity for the chip row | unchanged |
| `each_absent_field_states_its_own_abstention_g_chipverdict` | the no-band row | unchanged |
| `a_near_total_headroom_never_prints_as_a_flat_hundred_g_chipverdict` | no flat `100 %` on the chip face | unchanged |
| `the_power_row_reads_zero_to_the_limit_g_chipverdict` | — | **new**: exactly one power row on three fixtures; a percent on the face; no ` kW` on the face; the setting beside it; the hover carries the clause, the setting, the kW pair and the four operating-point figures |
| `the_power_row_states_a_real_reading_g_chipverdict` | — | **new**, non-vacuity: the in-band face is a positive percent under 100, and it equals `power_at_operating_point`'s own ratio to within 0.05 points |
| `the_power_row_states_its_refusal_g_chipverdict` | — | **new**: acrylic refuses `MaterialUnvalidated`; the face carries the door's own clause, no percent and no digit; the hover names the input and the variant |
| `the_power_bar_is_informative_g_chipverdict` | — | **new**: the ban's successor — the spread over 270 shipped recipes, plus the peak recipe rendered |

The `≈`-free arm the brief named is not in this file at HEAD; V1 has not
landed. Nothing was removed on its account.

### What the replacement kept from the ban, and what it dropped

**Kept: the reason.** The ban's four identifiers named the WAY the gauge was
built, not the reason it was wrong. Two of them — `rail_power_row` and
`power_color` — are now the names of the row that ships. The reason was a
measurement, and the measurement is what the successor holds. The successor
also drops the source scan the ban used, because it asserts on what the card
paints instead.

**Dropped, deliberately.** The ban asserted that the whole card painted no
` kW` run at all. The ruling puts the kW pair on the HOVER, so a card-wide
check would now be false. The successor asserts the narrower true thing: no
` kW` on the FACE, and the kW pair present on the hover.

### The harness changes the new arms needed

- `state_for(fixture)` now delegates to `state_for_recipe(machine, tool,
  material, operation)`, so the spread arm can build a session on any shipped
  preset, diameter and family. The four fixture builders are unchanged.
- `painted_text(fixture)` now delegates to `painted_text_of(state)`.
- `compare_source()` was deleted with the ban that used it.

---

## 5. What `explore.rs` did

`explore.rs:1276` read
`(rec.power_kw * (explore_feed / rec.feed_rate_mm_min)).max(0.0)` — the
CALCULATOR-geometry power, rescaled by a feed ratio. That is the same
class-1 defect S2 fixed on the card: 11.96× off on a shipped Ø12 roughing
preset, and then rescaled.

The chart DOES have the operation, the tool, the material and the machine in
reach. `draw_modal_body` holds `state` and `toolpath_id`, and already reads
the corridor through `read_cut_efficiency` on exactly that route. The fix
follows that idiom and does not restructure the chart:

- a new `read_power_figure(state, toolpath_id, preview)` beside
  `read_cut_efficiency`, returning `Option<PowerFigure>`;
- `Option<&PowerFigure>` threaded through `draw_chart_c` into
  `draw_explore_controls`, beside `Option<&CutEfficiency>`;
- the readout now calls `figure.required_kw_at_feed(explore.feed_mm_min)`.
  Power is affine in the feed, so this is exact at any explored feed rather
  than a first-order preview;
- `preview_power_kw` is deleted.

**One more thing was wrong at that site, and it is fixed with it.** The
denominator was `env.max_power_kw` — the machine's RATING, with no safety
factor. A COMMANDED-axis numerator over a RAW-axis ceiling reads the cut as
having `1/safety_factor` (1.25× to 1.33×) more room than the gate allows. That
is F-2's defect in a second neighbourhood. Both figures now come off one
`PowerFigure`, so the numerator and the ceiling carry the factor once each.
The readout reads `power X of Y kW (Z% of the limit)`, and a refused figure
paints `power not modelled at this operating point` rather than
`0.00 kW (0% of cap)`.

The Explore readout keeps its kW pair on the face. It is a chart readout at an
operator-dragged point, not a 0-to-limit row, so the ruling about faces does
not reach it. Say if it should also become a percent.

---

## 6. Verification

Every command through `scripts/cargo_lane.sh`, every `rs_cam_viz` command with
`-j 2`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | no diff in any file this step owns (see §7.5) |
| `check -p rs_cam_viz -j 2 --all-targets` | clean |
| `clippy -p rs_cam_viz -j 2 --all-targets -- -D warnings` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 26.88s` |

| Target | Result |
|---|---|
| `the_chipload_verdict_is_one_row_g_chipverdict` | `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.23s` |
| `test -p rs_cam_viz --lib -j 2 -q` | `test result: ok. 408 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s` |
| `the_corridor_bounds_the_band_g_corridor` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s` |
| `the_feeds_window_fits_the_screen_g_feedsfit` | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s` |
| `the_feeds_modal_holds_one_scope_dc5a` | `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `the_recommendation_explains_each_row_g_whyrow` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.61s` |
| `the_nomogram_readout_abstains_g_hoverbound` | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s` |
| `the_speeds_apply_holds_the_cut_g_speedsonly` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |

The last four are the remaining sentries of `ui/feeds/`, run because
`explore.rs` is in this step's file set.

---

## 7. What the plan did not anticipate

### 7.1 A spread arm that measures the door is green before the bar exists

Covered in §2. The arm now measures both the door and the bar.

### 7.2 `env.max_power_kw` was the second half of the explore defect

Covered in §5. The brief named the numerator; the denominator was on the wrong
axis. Fixing one without the other would have left a mixed-axis ratio, so both
moved together.

### 7.3 `components::compare::power_bar` was never in scope of the ban

The ban scanned `ui/feeds/compare.rs` only. `ui/components/compare.rs` has
carried `power_bar` and `power_color` throughout, and `power_bar` has no
caller — it is dead code. Its face prints `X / Y kW (Z %)`, which is what the
ruling now forbids on a face, so it must not be adopted as-is by a later
surface. The new row therefore builds its own bar and reuses only
`power_color`, whose only job is the colour ramp. `ui/components/` is not in
this step's file set, so `power_bar` was left alone. **Recommendation:** delete
it, or rewrite its face as a percent, in a step that owns that folder.

### 7.4 `draw_comparison_card` carried an `allow` with no `// SAFETY:` line

It already had `#[allow(clippy::too_many_arguments)]` before this step, with
no justification comment, against the workspace rule in root `CLAUDE.md`. The
comment is now there, and it states the same cause
`draw_inspector_comparison`'s allow states: the verdict rows are statements
about the CUT, and bundling the session's values into a struct would build a
second data model of them. `draw_chart_c` and `draw_explore_controls` crossed
the threshold in this step and carry the same allow with the same reason.

### 7.5 Four neighbouring files are unformatted, and one broke the build once

`fmt --all -- --check` still reports diffs in files this step does not own:

- `crates/rs_cam_viz/src/ui/readiness_panel.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/tests/readiness_shows_the_limits_g_readylimits.rs`
- `crates/rs_cam_viz/tests/the_limit_rows_read_their_own_bound_g_ownbound.rs`
- `crates/rs_cam_viz/tests/zz_dump_tmp.rs` (untracked; looks like a scratch
  file left behind)
- `crates/rs_cam_viz/src/controller/tests/sim_stale_is_the_core_answer_g_freshnessdisagree.rs`
  (untracked)

All belong to the `v123-limit-rows` and W1 sessions. None was touched. The
list is a snapshot: it moved between runs, and a final check also reported
`crates/rs_cam_mcp/src/server.rs`. No file this step owns appears in it at any
point after §6's `fmt` run.

`readiness_panel.rs` also failed to compile once, mid-step
(`cannot find function draw_limit_rows`), and `the_corridor_bounds_the_band_g_corridor`
failed to compile once against a transient lib state. Both cleared on a re-run
a minute later, as the brief's rule expects.

### 7.6 The folder instruction file reached its cap exactly

`crates/rs_cam_viz/src/ui/feeds/CLAUDE.md` did not list
`the_chipload_verdict_is_one_row_g_chipverdict` among the folder's sentries.
It now does, and it states the power invariant. The file is **exactly 40
lines**, its stated ceiling. A later step adding a line there must remove one.

---

## 8. Paths

Changed:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/compare.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/CLAUDE.md`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/the_chipload_verdict_is_one_row_g_chipverdict.rs`

Added:

- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/V4_IMPLEMENTATION.md` (this file)

No new sentry file was needed: the ban's successor sits where the ban sat.

---

## 9. Two sentries red after V4 and T-9 — repaired 2026-09-18

V4 landed at `05848280`. Two `rs_cam_viz` sentries outside the first round's
run were red. Both belong to this programme, and each had a different cause.

### 9.1 `component_contracts_up2` — the power row was a fourth hand-rolled chain

```
ui/feeds/compare.rs is allowed 3 hand-rolled chains and holds 4. An allowance
that no longer matches its file lets a new hand-rolled header in under an old
number.
  left: 4
 right: 3
```

Two arms failed: `a_section_header_is_the_kit_element_ui03` (over budget) and
`the_hand_rolled_emphasis_list_is_not_vacuous_ui03`, which asserts the count
EQUALS the allowance, so an under-count fails too.

The four chains in `compare.rs` were the `rail_row` label, the `rail_efficiency_row`
label, the new `rail_power_row` label and the Apply column's refusal notice.
The first three were three copies of one `RichText` chain.

**The repair.** One renderer for one element, which is the kit's own invariant
(`ui/components/CLAUDE.md`): a private `verdict_row_label(ui, label, hover)`
in `compare.rs`, called by `rail_efficiency_row` and `rail_power_row`. The
chipload verdict and the power reading are the same element — a named verdict
whose workings sit on a hover — so they now paint through one function. The
count returns to **exactly 3**: the comparison row's label, the shared verdict
label, and the refusal notice. The allowance was not touched, in either
direction.

**What was not done, and why.** The brief asked for the face, the caption and
the hover to be rebuilt through "the same kit components `rail_efficiency_row`
uses". `rail_efficiency_row` uses no kit component for its label: it
hand-rolled the same chain, which is why the file's allowance was 3 rather
than 0. The nearest kit rung is `components::text::body_strong`, and it is 13
points where the rail's rung is `small` at 11. A 13-point row does not fit the
240-point rail. The helper therefore keeps the chain and states that reason in
its doc.

**One request for the kit, not taken this round.** The kit has no dense
emphasis rung. If `ui/components/text.rs` gained one — a `small` + `strong` +
`TEXT_HEADING` constructor — the three remaining chains in this file and the
matching chains in `sim_op_list.rs`, `sim_timeline.rs` and `readiness_panel.rs`
could all read it, and several allowances would drop to zero. That is a
`ui/components/` change and outside this round's file set.

### 9.2 `apply_contract_a3` — a pinned plunge moved with its cause

```
assertion `left == right` failed: plunge
  left: 793.0
 right: 794.0
```

**Cause confirmed** at `6a9330dc` (T-9), in
`crates/rs_cam_core/src/feeds/suggest/apply.rs`:

```rust
-    scratch.set_feed_rate(round_suggestion_value(result.feed_rate_mm_min, 1.0));
-    scratch.set_plunge_rate(round_suggestion_value(result.plunge_rate_mm_min, 1.0));
+    // T-9: the feed and the plunge round DOWN, not to the nearest.
```

`apply_feeds_subset` rounded the feed and the plunge to the nearest whole
mm/min after every clamp had bound them, so a clamped value shipped up to
+0.5 mm/min above the ceiling the clamp exists to enforce. It now floors both.
The Pocket fixture's unrounded plunge is 793.75, so it ships as 793.

**The repair**, the same one `arc_fit_disposition_a5` took at that commit
(881 → 880, 638 → 637):

- `pocket_fixture_recipe_fingerprint_is_unmoved` asserts `793.0`, with a
  comment naming T-9 and the reason. It is still an equality on one value; no
  tolerance was widened.
- That test's doc block records the move: which figure moved, its cause, its
  commit, and why the other four of the five did not — the stepover and the
  depth keep the nearest rounding because their clamps run below the
  quantisation, and the feed lands on a whole number.
- `modal_apply_all_writes_what_the_panel_writes` quotes `794` in a table. Its
  assertions compare the two write routes to each other, not to an absolute
  value, so it stayed green. Its doc now says the plunge column reads 793 from
  T-9 onward and points at the test that carries the reason.

**Three `794` sites were left alone, deliberately.** Lines 248, 297 and 322
are each headed *"Pre-fix (measured 2026-08-12)"* and record what the
pre-Checkpoint-I defect wrote. Re-blessing a dated measurement of a defect
would falsify the record it exists to keep. 793.75 at line 248 is the
unrounded calculator value and did not move either.

### 9.3 Verification for this round

| Command | Result |
|---|---|
| `fmt --all -- --check` | no diff in any file this round owns; one remains in `crates/rs_cam_mcp/src/server.rs`, another session's |
| `clippy -p rs_cam_viz -j 2 --all-targets -- -D warnings` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 29.11s` |
| `--test component_contracts_up2` | `test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s` |
| `--test apply_contract_a3` | `test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `--test the_chipload_verdict_is_one_row_g_chipverdict` | `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.46s` |
| `test -p rs_cam_viz --lib -j 2 -q` | `test result: ok. 409 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.21s` |

Every arm of the power row still passes after the kit rebuild, including the
rendered face, hover, refusal and spread arms.

### 9.4 Paths, this round

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/compare.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/apply_contract_a3.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/V4_IMPLEMENTATION.md` (this section)

`crates/rs_cam_viz/tests/component_contracts_up2.rs` was read and **not**
edited: no allowance moved, in either direction.
