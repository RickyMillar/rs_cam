# T-9 implementation — a bound feed rounds down, and the export validator checks the feed

Written 2026-09-18 on `master`, HEAD `59822cab`. Nothing is staged.
Nothing is committed.

---

## 1. The red run

The sentry was written first. To make the file compile, the new helper
`round_suggestion_value_down` was added to `suggest.rs` BEFORE the red run.
That is a pure addition: `apply.rs` still called `round_suggestion_value`
for the feed and the plunge, so arm (b) ran against the unfixed apply path.

```
running 3 tests
.. 2/3
the_quantisation_never_raises_a_clamped_feed_g_feeddown --- FAILED

failures:

---- the_quantisation_never_raises_a_clamped_feed_g_feeddown stdout ----

thread 'the_quantisation_never_raises_a_clamped_feed_g_feeddown' (1542408) panicked at crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs:279:9:
Generic Wood Router / Walnut: the calculator returned 2562.504018155792 mm/min, every ceiling satisfied, and the funnel shipped 2563 mm/min — 0.4959818442080177 mm/min ABOVE it. The quantisation must never raise a feed a clamp already bound. T-9.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    the_quantisation_never_raises_a_clamped_feed_g_feeddown

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The defect reproduces at **+0.4959818442080177 mm/min**, which is 99.2 % of
the theoretical worst case of +0.5 mm/min. The measured pair is Generic
Wood Router / Walnut, not the register's Shapeoko / Ipe pair; this sweep
uses the sibling instrument's Ø12 slot fixture, and the sweep reports its
own worst case rather than the register's single point.

Arms (a) and (c) passed on that run, as expected: they test the new helper
and the untouched nearest helper, neither of which the apply path change
affects.

---

## 2. The apply-site changes

### 2.1 The helper

`crates/rs_cam_core/src/feeds/suggest.rs`, beside `round_suggestion_value`:

```rust
#[must_use]
pub fn round_suggestion_value_down(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).floor() * step
}
```

Both doc comments now carry the choice rule. `round_suggestion_value`'s doc
gained a paragraph headed "Which of the two to call": a value with no upper
bound at the point of the call rounds to the nearest, a value a clamp
already bound from above takes the down twin. The down twin's doc names the
Step 6 power gate, the Step 7 machine ceiling, the T-9 register entry and
`MachineProfile::next_rpm_at_or_below` as the same fix on the RPM axis.

`round_suggestion_value` itself is unchanged.

### 2.2 The two call sites that changed

`crates/rs_cam_core/src/feeds/suggest/apply.rs`, in `apply_feeds_subset`:

```rust
scratch.set_feed_rate(round_suggestion_value_down(result.feed_rate_mm_min, 1.0));
scratch.set_plunge_rate(round_suggestion_value_down(result.plunge_rate_mm_min, 1.0));
```

A comment above them states the reason, the measured before-number and the
cost, and names both sentries.

### 2.3 The stepover and the depth — assessed, NOT changed

The brief asked whether either is bound from ABOVE at the point of the
rounding. **Neither is.** The two cases are structurally different:

| Value | Where its ceiling is applied | Relative to the rounding |
|---|---|---|
| feed, plunge | `feeds::calculate` Step 6 and Step 7 | UPSTREAM — bound when rounded |
| stepover, depth | `enforce_invariants`, called at `apply.rs:154` | DOWNSTREAM — not yet bound |

`apply_feeds_subset` rounds all four values onto the scratch clone and only
then calls `enforce_invariants`. Every geometry clamp therefore runs after
the rounding and moves geometry DOWNWARD, so a +0.0005 mm nearest rounding
cannot leave a bound value above its bound — the clamp has not run yet.
`suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs` states the same
direction: "`enforce_invariants` clamps geometry DOWNWARD".

The depth has a second reason. After the 0.001 rounding it goes through
`crate::ops::depth::realised_step_down`, which snaps it to a reachable
staircase value (`total / ceil(total / per_pass)`). That snap moves the
number by far more than 0.0005 mm, so the rounding direction is not what
decides the shipped depth.

Three existing tests also pin the nearest rounding for the depth —
`pill_writes_clamped_value_g_pillclamp.rs:114` and
`apply_contract_a3.rs:572,666,1013` recompute
`round_suggestion_value(result.axial_depth_mm, 0.001)` as their expected
value. The case for changing them is not clear, so they were left alone, as
the brief directed.

---

## 3. The mirror hazard — outcome

The hazard: a feed the rubbing floor RAISED is bound from BELOW, and the
round-down moves it away from that floor by up to 1 mm/min.

**It did not fire on any shipped fixture.**

- `rubbing_floor_never_exceeds_band` — 2 passed, 0 failed.
- `rs_cam_core --lib` — 2520 passed, 0 failed, 12 ignored.

No floor test tripped, so no fixture in either suite sits within 1 mm/min of
its rubbing floor. No tuning was needed and none was done.

The sentry pins the cost explicitly rather than leaving it implicit: the
comment at the apply site states that the floor is an advisory band and the
power gate is a physical limit, so the trade is correct in that direction.

---

## 4. How the machine profile reached the validator

All four `machine_safety_pass` callers already hold `session:
&ProjectSession`. `ProjectSession::machine()` (`session/mod.rs:1470`)
returns `&MachineProfile`, and `max_feed_mm_min` is a plain `f64` field
(`machine/mod.rs:95`). The sibling precedent in the same crate is
`crates/rs_cam_viz/src/ui/sim_op_list.rs:243`:
`let max_feed = session.machine().max_feed_mm_min;`.

The helper gained a third parameter and every caller passes
`session.machine().max_feed_mm_min`:

```rust
fn machine_safety_pass(gcode: &str, safe_z: f64, max_feed_mm_min: f64) -> Vec<Finding> {
    let findings = validate_machine_safety(
        gcode,
        MachineSafety {
            clearance_z: safe_z,
            min_z: None,
            max_feed_mm_min: Some(max_feed_mm_min),
        },
    );
```

The four call sites are `export.rs:532`, `:605`, `:668` and `:755` after
formatting.

The doc comment no longer says the check waits for plumbing. It now names
the axis and the reason: `max_feed_mm_min` is the gantry TRAVEL rate, GRBL's
`$110`/`$111`, and NOT `cutting_feed_ceiling_mm_min()`. The validator checks
every `F` word, and `replace_rapids_with_feed` rewrites rapids as feeds at
the travel rate, so the cutting ceiling would raise a finding on every
rewritten rapid. A separate paragraph says the depth-floor check stays off
because no caller has a stock-bottom limit to hand it.

`min_z: None` is unchanged.

---

## 5. Neither viz file was skipped

`git status --short` was run on both `crates/rs_cam_viz/src/io/export.rs`
and `crates/rs_cam_viz/src/ui/feeds/explore.rs` before the first edit and
again immediately before the export.rs change. Both printed nothing, so both
were clean and both were edited.

---

## 6. `explore.rs` — before and after

### 6.1 `Ceiling::OffScale`, near line 306

**Before**

```
/// This is the ORDINARY case, and it is not an error. Measured on the
/// reference fixture (6 mm two-flute flat, 4.20 DOC, 2.10 WOC, generic
/// softwood) the ceiling is 9.36 mm/tooth against a chart that draws
/// 0.1176 mm/tooth at most — about eighty times off scale. A stubby
/// carbide cutter in wood is not deflection-limited; the binding
/// constraints are the feed cap, the rubbing floor and rigidity.
```

**After**

```
/// This is the ORDINARY case, and it is not an error. Measured on the
/// reference fixture (6 mm two-flute flat, 45 mm stickout, 4.20 DOC,
/// 2.10 WOC, generic softwood) the ceiling is **2.4315 mm/tooth**
/// against a chart that draws **0.12353 mm/tooth** at most — about
/// twenty times off scale. A stubby carbide cutter in wood is not
/// deflection-limited; the binding constraints are the feed cap, the
/// rubbing floor and rigidity.
///
/// This pair was 9.36 against 0.1176 until 2026-09-18. Neither number
/// reproduced: the ceiling was 3.85x the measured value, and 0.1176 is
/// the chart top at 18000 RPM while this cut runs 17000.
/// `the_corridor_bounds_the_band_g_corridor` measures the pair; the
/// derivation is in `planning/load_model_2026-09-16/CORRIDOR_REPAIR.md`.
```

The stickout of 45 mm was added because `CORRIDOR_REPAIR.md` §4 and the
sentry's own module doc both state it, and the cut is not defined without
it. 2.4315 / 0.12353 = 19.7, hence "about twenty times".

### 6.2 `Ceiling::OnChart`, near line 324

**Before**

```
/// Modelled and inside the plot's range, so the upper wedge IS drawn.
/// Compliance rises with the cube of stickout, so a long or a thin tool
/// brings the ceiling down onto the chart.
```

**After**

```
/// Modelled and inside the plot's range, so the upper wedge IS drawn.
///
/// **The diameter is the lever here, not the stickout.**
/// `ToolDefinition::tip_deflection_mm` models a STEPPED cantilever: a
/// fixed 25 mm flute section at the cutter diameter, below a 6.35 mm
/// shank that fills the rest of the stickout. `I` goes with the fourth
/// power of the diameter, so at Ø2 mm the shank is about 100x stiffer
/// per unit length than the flute section. The flute section carries
/// nearly all of the compliance, and its length does not change with
/// the stickout. Measured at Ø2 mm: 40 mm of stickout gives 2.618e-2
/// mm/N and 120 mm gives 3.767e-2 mm/N — 3x the length for 1.44x the
/// compliance, not 27x.
///
/// So a THIN tool brings the ceiling down onto the chart; a long one
/// barely moves it. Any prose that says the compliance goes with the
/// cube of stickout describes a plain cantilever, not this model.
/// Derivation: `planning/load_model_2026-09-16/CORRIDOR_REPAIR.md` §6.
```

Every number is from `CORRIDOR_REPAIR.md` §6 and the sentry's doc block at
`the_corridor_bounds_the_band_g_corridor.rs:42-52`.

### 6.3 No test pins the old strings

```
rg -n "9\.36|0\.1176|cube of stickout" --glob '!target' .
```

Outside `explore.rs` itself the strings appear in exactly two live places,
and neither is an assertion:

- `crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs:23-26`
  and `:50` — module DOC comments that cross-refer to `explore.rs` and say
  to re-derive it.
- `planning/…` evidence documents, which are records, not tests.

The other hits are coincidental decimals in a G-code file, a contrast table
and a vendor provenance note.

---

## 7. Verification — every line verbatim

`scripts/cargo_lane.sh fmt --all -- --check`: after the two files were
formatted, the check reports **0 `Diff in` lines**. `rustfmt --edition 2024`
was run on exactly two explicit paths — `export.rs` and the new sentry —
rather than `cargo fmt --all`, because the tree is shared.

The new sentry, `scripts/cargo_lane.sh test -p rs_cam_core -q --test
a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`:

```
running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`scripts/cargo_lane.sh test -p rs_cam_core -q --test rubbing_floor_never_exceeds_band`:

```
running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`scripts/cargo_lane.sh test -p rs_cam_core --lib -q`:

```
test result: ok. 2520 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 35.01s
```

`scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features
heavy-tests,research,test-support -- -D warnings`:

```
   Compiling rs_cam_core v0.1.0 (/home/ricky/personal_repos/rs_cam/crates/rs_cam_core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.89s
```

`scripts/cargo_lane.sh check -p rs_cam_viz -j 2 --all-targets`:

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.40s
```

`scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test modulated_feeds_reach_gcode_g_modexport`:

```
running 2 tests
test viz_export_refuses_the_worker_result_when_the_session_slot_is_invalidated ... ok
test viz_export_emits_the_modulated_feed_schedule ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test export_parity_core_vs_gui_p0`:

```
running 2 tests
test core_and_gui_export_agree_on_coolant ... ok
test core_and_gui_export_are_byte_identical_on_the_agreeing_properties ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test mcp_export_names_a_safety_finding_edg06`:

```
running 4 tests
test a_clean_export_reports_exactly_the_text_it_always_did ... ok
test a_warning_only_pass_does_not_claim_an_error ... ok
test an_mcp_export_that_trips_an_error_finding_names_it ... ok
test the_reporting_door_emits_the_same_program ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

No blessed output needed a re-bless. `T9_CEILING.md` §6.7 predicted that
`tests/fixtures/perf_golden_*.json` and `literature_matrix/` cells that pin
a feed to the whole mm/min might move. Neither the core lib suite nor the
three viz tests reported such a failure, so no golden was touched. The
`literature_matrix` and `literature_parity` targets were NOT run; they are
outside the brief's allowed command set, and they are the place a moved feed
would surface if one did.

---

## 8. Anything unanticipated

1. **The sweep's worst case is not the register's pair.** The register
   measures Shapeoko / Ipe / Ø12 slot at +0.3007 mm/min. This sentry's
   sweep, over the same Ø12 slot fixture the sibling instrument uses, finds
   Generic Wood Router / Walnut at +0.4959818 mm/min. The larger number does
   not contradict the register: the register reports one named point, the
   sweep reports the maximum over 30 pairs. Both are below the +0.5 mm/min
   bound `T9_CEILING.md` §2 derives.

2. **`Material::wood(species)` does not exist.** The first draft of the
   sentry used it. The shipped constructor is the struct variant
   `Material::SolidWood { species }`, which is what the sibling instrument
   uses.

3. **`crates/rs_cam_viz/src/ui/properties/pills.rs` is Site 2 and is
   untouched.** `T9_CEILING.md` §2 Site 2 records the same defect in the
   GUI fallback branch of `suggestion_for`: with no funnel preview it calls
   `round_suggestion_value(calculator, 1.0)` on the raw calculator feed,
   which is an independent instance. That file is outside this session's
   allowed set, so it was not edited. **It still carries the defect.**

4. **The corridor sentry's cross-reference is now stale in the good
   direction.** `crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs:23-26`
   says "`explore.rs`'s `Ceiling::OffScale` doc still prints 9.36 mm/tooth
   against 0.1176 … Re-derive that comment before citing it." That is now
   done. The lines are a doc comment, not an assertion, so nothing fails —
   but the sentence is no longer true. That file is outside this session's
   allowed set and was not edited.

5. **`crates/rs_cam_core/src/feeds/CLAUDE.md` does not name the new
   sentry.** `T9_CEILING.md` §7 asks for it to be listed there as the
   folder's sentry for the quantisation direction. That file is outside the
   allowed set and was not edited.

6. **`rustfmt` reported touching `machine/mod.rs`; it did not.** The harness
   noted `crates/rs_cam_core/src/machine/mod.rs` as modified after the
   format run. `git diff` on that path shows only the T-18 agent's new
   `commanded_cutting_feed_ceiling_mm_min` function, a pure addition that
   pre-dates this session's format command. `rustfmt` was given two explicit
   paths and neither was that file.

---

## 9. Paths changed or created

Changed:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/io/export.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `/home/ricky/personal_repos/rs_cam/planning/TECH_DEBT_REGISTER.md`

Created:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/T9_IMPLEMENTATION.md`

Nothing is staged. Nothing is committed.

---

## 10. Follow-ups, 2026-09-18 — the four review items

The orchestrator accepted T-9 and asked for four changes. The allowed file
set gained `crates/rs_cam_viz/src/ui/properties/pills.rs` and
`crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`.
`crates/rs_cam_core/src/feeds/CLAUDE.md` and
`planning/TECH_DEBT_REGISTER.md` were NOT edited again; the orchestrator
adds the sentry lines and closes the entries.

`git status --short` on `pills.rs` and on the corridor sentry printed
nothing before the edits, so both were clean. `explore.rs` showed as
modified by this session's own earlier change, as expected.

### 10.1 The shank stiffness ratio was stated on the wrong diameter

`explore.rs` `Ceiling::OnChart` said the shank is "about 100x stiffer per
unit length than the flute section" at Ø2 mm. That figure uses the RAW
cutting diameter: `(6.35 / 2.0)^4 = 101.62`.

Since T-17 the flute section does not bend on the raw diameter.
`ToolDefinition::tip_deflection_mm` (`tool/mod.rs:723-740`) integrates the
cutter section at `bending_fraction * lookup_diameter_at(..)`, and for every
fluted shape `bending_fraction` is
`crate::feeds::predict::ENDMILL_EQUIVALENT_DIAMETER_FRACTION` = **0.80**
(`feeds/predict.rs:259`). The shank loop is separate and takes the full
6.35 mm. The ratio the model actually carries is therefore

```text
(6.35 / (0.80 * 2.0))^4 = 3.96875^4 = 248.09
```

The comment now shows that arithmetic in a fenced block, names the constant
`ENDMILL_EQUIVALENT_DIAMETER_FRACTION` and its value, states **248x**, and
says the 101x figure is what the raw diameter would give. The rest of the
paragraph — the stepped cantilever, the 25 mm flute section, the measured
2.618e-2 / 3.767e-2 mm/N pair — is unchanged.

### 10.2 Site 2 — the feed pill's fallback branch

`crates/rs_cam_viz/src/ui/properties/pills.rs`. The no-preview branch of
`suggestion_for` quantised the raw calculator feed with
`round_suggestion_value`, an independent instance of T-9 in the GUI.

`suggestion_for` and the private `PillSuggestions::suggestion` each gained a
`round: fn(f64, f64) -> f64` parameter, which is the shape `T9_CEILING.md`
§6.4 proposed. `feed_rate()` passes `round_suggestion_value_down`;
`stepover()` and `depth_per_pass()` pass `round_suggestion_value` and each
now says why — their clamps run below that point, so neither value is bound
from above there. `suggestion_for` has no caller outside this file, checked
with `rg`.

`crates/rs_cam_viz/src/ui/properties/CLAUDE.md` names **no** sentry for
`pills.rs`; its seven listed sentries cover the Bottom-Z note, boundary
controls, nesting, width, the depth caution, the help key and the operations
registry. The coverage that does exist was run instead:

- `pills.rs`'s own `#[cfg(test)] mod tests`, which asserts the DEPTH
  fallback against `round_suggestion_value` — reached by
  `test -p rs_cam_viz --lib -j 2`.
- `crates/rs_cam_core/tests/pill_writes_clamped_value_g_pillclamp.rs`, the
  G-PILLCLAMP sentry for the pill's write path.

### 10.3 The corridor sentry's cross-reference

`crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`, module
doc, lines 23-26.

**Before**

```
//! `explore.rs`'s `Ceiling::OffScale` doc still prints 9.36 mm/tooth
//! against 0.1176 for that same stated cut. This file measures 2.4315
//! against 0.12353 and does not reproduce the pair. Re-derive that comment
//! before citing it.
```

**After**

```
//! `explore.rs`'s `Ceiling::OffScale` doc printed 9.36 mm/tooth against
//! 0.1176 for that same stated cut. Neither number reproduced here, and the
//! comment was corrected on 2026-09-18 (T-9): it now carries this file's
//! measured pair, 2.4315 against 0.12353. The two are in agreement, so a
//! change to either must move both.
```

Doc-only. No arm, fixture or assertion changed.

### 10.4 Verification of the follow-ups

`scripts/cargo_lane.sh fmt --all -- --check` — the whole check, read-only.
It reports **no `Diff in` line**, so every file in the workspace is
formatted.

One note on that check: an earlier invocation in the same minute reported
two `Diff in` lines in
`crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs`,
which is the T-18 agent's file and not this session's. A re-run reports
clean, so that agent formatted it in between. No file of this session's ever
appeared in that output.

`scripts/cargo_lane.sh test -p rs_cam_core -q --test
a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`:

```
running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test the_corridor_bounds_the_band_g_corridor`:

```
running 4 tests
test the_ceiling_decision_is_made_on_the_charts_own_range_g_corridor ... ok
test the_rubbing_floor_wedge_is_drawn_g_corridor ... ok
test the_corridor_draws_both_bounds_when_the_ceiling_is_on_chart_g_corridor ... ok
test the_upper_wedge_is_absent_when_the_ceiling_is_off_scale_g_corridor ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

`scripts/cargo_lane.sh test -p rs_cam_viz --lib -j 2 -q`:

```
running 396 tests
test result: ok. 396 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.32s
```

`scripts/cargo_lane.sh test -p rs_cam_core -q --test pill_writes_clamped_value_g_pillclamp`:

```
running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`scripts/cargo_lane.sh check -p rs_cam_viz -j 2 --all-targets`:

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.80s
```

### 10.5 What the follow-ups leave open

- `crates/rs_cam_core/src/feeds/CLAUDE.md` still does not list
  `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` as the folder's
  sentry for the quantisation direction. The orchestrator owns that line.
- `crates/rs_cam_viz/src/ui/properties/CLAUDE.md` names no sentry for
  `pills.rs`, and `pills.rs` now carries a T-9 rule in its fallback branch
  that only its own inline test module touches. A viz sentry for the pill's
  fallback rounding direction would close that gap. Not written here: the
  brief scopes this round to the four named items.
- Core clippy was not re-run after the follow-ups. Only `pills.rs`,
  `explore.rs` and the corridor sentry changed, all in `rs_cam_viz`, and
  `check -p rs_cam_viz -j 2 --all-targets` is clean. The core clippy run in
  §7 still stands for the core change.

---

## 11. Full path list

Changed:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/io/export.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/properties/pills.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`
- `/home/ricky/personal_repos/rs_cam/planning/TECH_DEBT_REGISTER.md`

Created:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/T9_IMPLEMENTATION.md`

Nothing is staged. Nothing is committed.

---

## 12. Follow-up 5 — a tolerance that encoded the NEAREST rounding

The T-18 agent found `a_raised_stepover_does_not_move_the_feed_at_all` red in
the working tree. The cause is the T-9 round-down, so the item belongs here.
`crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs` was clean at
`git status --short` before the edit.

### 12.1 The red, and what it measures

```
── 3D Finish 6 (Tapered Ball 2mm tip / 7° / 6mm shank)
   calculator : ae=0.03000 ap=0.1000 feed=404.759 rpm=19000 factor=1.00000 advance=0.010652
   shipped    : ae=Some(0.22781250000000003) ap=None feed=404.000 rpm=Some(19000) advance=0.010632
   band       : Some(ChiploadBounds { min_mm_per_tooth: 0.005325779517051008, max_mm_per_tooth: 0.010651559034102016 })

thread 'a_raised_stepover_does_not_move_the_feed_at_all' panicked at
crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs:332:5:
3D Finish 6: the stepover moved 0.03000 → 0.22781 mm and the feed moved with it, 404.7592 → 404.0000 mm/min.

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.79s
```

The warning stream carries `FinishEnvelopeAdvisory`,
`StepoverRaisedForRuntime` and `CutGeometryFieldNotHeld` — no lift warning —
so none of T-18's sites runs on this case. The gap of 0.759 mm/min is the
quantisation alone.

### 12.2 Assertion 1 — re-derived, not widened

The old bound was `(shipped_feed - calculator_feed).abs() <= 0.5 +
calculator_feed * 1e-9`. The 0.5 is the half-step of a NEAREST rounding to
1 mm/min, where the half-step IS the whole error. A floor has a one-sided
error of up to a FULL step, so 0.5 is the wrong tolerance, not a slack that
needs widening.

The new form:

```rust
let quantisation_gap = calculator_feed - shipped_feed;
assert!(
    quantisation_gap >= 0.0 && quantisation_gap < 1.0 + calculator_feed * 1e-9,
```

This is **stronger** than the old `abs()`: it pins the direction as well as
the magnitude, so a future change that raises a feed here fails even by half
a step. The comment states the derivation, names T-9, and records the
measured 404.759 → 404.000.

### 12.3 The comment near line 362 described the same thing, and it was wrong

That block read: "`apply_feeds_subset` rounds the feed to 1 mm/min and can
push it a hair over. Measured 1.001×, which is 0.5 mm/min on a 405 mm/min
feed", and it built its tolerance from that: `let rounding = 0.5 / (rpm *
flutes)`.

That upward allowance no longer exists. Step 9b pins the advance AT the band
ceiling and the apply path now only ever floors from there, so the
quantisation can move the advance DOWN and nothing else. The run above
measures the advance at **0.010632** against a ceiling of **0.010652** — it
now sits 1.9e-5 mm/tooth BELOW the ceiling, where it used to sit 1.001×
above.

The tolerance is re-derived as what actually remains, float noise in the
pinning arithmetic, and renamed so the code says which it is:

```rust
let float_noise = band.max_mm_per_tooth * 1e-9;
```

`rpm` and `flutes` were read only to build the old `rounding`, so both lets
went with it. The assertion message now states the rule rather than the
allowance: Step 9b pins the advance at the ceiling and the apply path only
floors from there, so nothing may sit above it.

`scripts/cargo_lane.sh test -p rs_cam_core --test suggest_feed_matches_final_geometry`:

```
running 2 tests
test a_raised_stepover_does_not_move_the_feed_at_all ... ok
test dpp_clamp_does_not_leave_a_stale_depth_tier_derate_in_the_feed ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.71s
```

### 12.4 The sibling sweep — `rg -n "0\.5 \+"`

Seven hits. **Six are not a feed quantisation** and were left alone:

| Where | What the 0.5 is |
|---|---|
| `src/adaptive/mod.rs:352` | a fraction bound, `(0.0..=0.5 + 1e-9).contains(f)` |
| `src/tool/flat.rs:276` | a comment on a parametric `t` |
| `tests/face_stock_top_frame_f028.rs:199,203` | a 0.5 mm depth-per-pass plus a 0.1 mm grid margin |
| `tests/tier_map_cache_t3.rs:218` | a grid cell centre |
| `src/stock/collision.rs:664` | a clearance tolerance in CELLS |
| `src/finish/ramp_finish.rs:1088` | a 0.5 mm Z drop plus 0.05 |

A wider sweep — `0.5` on any line naming a feed, `mm_min`, `advance` or
`per_tooth` — found one more production hit,
`src/dressup/feed_modulation.rs:334`:
`(feed - commanded_feed_mm_min).abs() > 0.5`. That is a "did this move
materially" threshold for a summary statistic, not a tolerance against a
calculator feed. **Listed, not changed.**

### 12.5 One sibling WAS the same quantisation, and it was red

`crates/rs_cam_core/tests/arc_fit_disposition_a5.rs:620` asserts
`(r.shipped_feed - fx.expected_suggest_feed).abs() >= 0.5` with the message
"Suggest must ship the un-lifted feed {} ± 0.5". Same quantisation, and red:

```
  DC-1 Ø3 ball / HardMaple / stock ceiling: Suggest must ship the un-lifted feed 881 ± 0.5, got 880
  DC-2 Ø2-tip tapered ball / WhiteOak / small router: Suggest must ship the un-lifted feed 638 ± 0.5, got 637

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.73s
```

The file was clean before the edit. **The fix here is a re-bless, not a
tolerance change**, because `expected_suggest_feed`'s own doc says it is
"what `suggested_operation.feed_rate()` returns" — a pinned SHIPPED value,
not a calculator value the tolerance has to absorb a rounding against. Both
constants moved down by exactly one step, each with a comment naming T-9 as
the measured cause:

- DC-1: `881.0` → **`880.0`**
- DC-2: `638.0` → **`637.0`**

The other two fixtures did not move. A3D-1 ships 1000.0 and A3D-2 ships
1806.7 — a non-integer, because pass 9 re-solves the feed after the
quantisation on a fixture with a mutated DPP. The two DropCutter fixtures
command no axial step, so pass 9 does not fire and they carry the quantised
value directly. That is why exactly those two moved, and it is the asymmetry
the file's own comment at `:326-335` already predicted.

```
running 3 tests
test retired_lift_leaves_feed_at_the_calculator_value ... ok
test modulation_default_is_on_and_closes_the_two_dropcutter_residuals ... ok
test arc_fit_arms_gate_observation ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.27s
```

### 12.6 Two more sentries on the apply door, both green

`rg` for the apply-door symbols across `crates/rs_cam_core/tests/` names nine
files. Two had not been run and are directly exposed:

```
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
(`suggest_power_ceiling_after_pass9_g_suggest_powerstale` — the file whose
0.2 % bound the register says exists "to allow exactly one such step"; the
floor moves the shipped point away from the ceiling, so the bound holds with
more room than before.)

```
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
(`apply_reports_a_field_it_cannot_hold_g_notheld`.)

### 12.7 Re-run of the gates

`scripts/cargo_lane.sh fmt --all -- --check` — no `Diff in` line.

`scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets --features
heavy-tests,research,test-support -- -D warnings`:

```
   Compiling rs_cam_core v0.1.0 (/home/ricky/personal_repos/rs_cam/crates/rs_cam_core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.76s
```

### 12.8 An open risk the orchestrator should weigh

Two red core integration sentries have now been found by other people rather
than by this session, because a core integration test is its own target and
`--lib` does not reach it. `rg` names nine files that touch the apply door;
of those, three remain unrun here:

- `crates/rs_cam_core/tests/wanaka_suggest_integration.rs` — minutes, needs
  the operator's go-ahead.
- `crates/rs_cam_core/tests/literature_matrix/shim.rs` and
  `_litmatrix_scallop_refuses_flat.rs` — feature-gated, and
  `T9_CEILING.md` §6.7 names `literature_matrix/` as a place a whole-mm/min
  feed cell could move. The matrix's own runner compares against
  `snap.feed_rate_mm_min`, the CALCULATOR value, which the change does not
  touch — but that is an inference from one `rg` hit, not a run.

The `rg` sweep is also not exhaustive: neither
`suggest_feed_matches_final_geometry.rs` nor `arc_fit_disposition_a5.rs`
appears in it, because both reach the funnel through a local `suggest_case`
helper. **The core full gate is what settles this**, and CI owns it. I did
not run it and did not ask, because the brief reserves that decision.

---

## 13. Full path list (final)

Changed:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/arc_fit_disposition_a5.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/io/export.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/properties/pills.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`
- `/home/ricky/personal_repos/rs_cam/planning/TECH_DEBT_REGISTER.md`

Created:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/T9_IMPLEMENTATION.md`

Nothing is staged. Nothing is committed.

---

## 14. Follow-up 6 — the pills test that pins both directions

`crates/rs_cam_viz/src/ui/properties/pills.rs`, inside the existing
`#[cfg(test)] mod tests`. One test,
`the_feed_floors_while_the_depth_rounds_to_the_nearest`, in the module's
existing style: it reuses the `demo_pocket()` fixture and the module's own
`assert!` + message idiom, and adds no helper.

### 14.1 What the demo pocket measures, and why the test has two arms

A probe on `demo_pocket()` showed the feed pill takes the **preview** branch,
not the fallback:

```
clamped=true field=Some(FeedRate) rec=1290 calc=1290.9375
```

So the fallback branch is not reachable through `feed_rate()` on this
fixture. That turned out to be useful rather than limiting, because the
preview value comes from the core apply funnel, which now floors. The test
therefore has two arms:

**Arm 1 — end to end, through the real `feed_rate()` accessor.** The
calculator gives **1290.9375** mm/min and the pill offers **1290**. It
asserts the offered value is at or below the calculator value, that it is
exactly the floor, and — the non-vacuity partner — that
`round_suggestion_value(1290.9375, 1.0)` really does go UP, so the first two
assertions are not an identity. This arm pins the WIRING, not just the
helper.

**Arm 2 — the fallback branch, both directions side by side.** It calls
`suggestion_for(None, ..)` twice: the feed at 2562.504 with
`round_suggestion_value_down` must give 2562.0 and must not exceed the
calculator value; the depth at 4.2005 with `round_suggestion_value` must go
UP, to 4.201. 2562.504 is the worst case the T-9 core sweep measured. This
arm pins that `suggestion_for` honours the quantiser it is handed, on both
directions at once, so an edit that unifies them fails.

The doc comment states which arm pins what, including the limitation that
arm 2 passes its quantiser explicitly and so does not by itself pin the
accessors' wiring — arm 1 does that for the feed.

### 14.2 Arm 1 confirmed RED on the pre-T-9 behaviour

Not derived — measured. The feed line in `apply.rs` was temporarily reverted
to `round_suggestion_value`, the test was run, and the line was restored:

```
thread 'ui::properties::pills::tests::the_feed_floors_while_the_depth_rounds_to_the_nearest'
panicked at crates/rs_cam_viz/src/ui/properties/pills.rs:339:9:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 396 filtered out; finished in 0.00s
```

`apply.rs:95-96` was checked after the restore and carries
`round_suggestion_value_down` on both the feed and the plunge.

### 14.3 Verification

`scripts/cargo_lane.sh test -p rs_cam_viz --lib -j 2 -q` — 397 tests, one
more than the 396 in §10.4, which is the new test:

```
running 397 tests
test result: ok. 397 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s
```

`scripts/cargo_lane.sh fmt --all -- --check` — no `Diff in` line.

`scripts/cargo_lane.sh check -p rs_cam_viz -j 2 --all-targets`:

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.91s
```

`scripts/cargo_lane.sh test -p rs_cam_core -q --test
a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`, re-run after the
temporary revert and restore:

```
running 3 tests
...
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

---

## 15. Complete path list

Changed:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/suggest/apply.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/arc_fit_disposition_a5.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/io/export.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/feeds/explore.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/src/ui/properties/pills.rs`
- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_viz/tests/the_corridor_bounds_the_band_g_corridor.rs`
- `/home/ricky/personal_repos/rs_cam/planning/TECH_DEBT_REGISTER.md`

Created:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs`
- `/home/ricky/personal_repos/rs_cam/planning/load_model_2026-09-16/T9_IMPLEMENTATION.md`

Changed by the orchestrator, not by this session, and listed only so the
commit set is complete:

- `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/feeds/CLAUDE.md`

Nothing is staged. Nothing is committed.
