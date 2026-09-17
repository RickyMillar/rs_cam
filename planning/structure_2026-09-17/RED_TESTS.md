# RED_TESTS — the five pre-existing red tests, diagnosed

Date: 2026-09-17. Branch `master`, HEAD `fdb9979a`.
Author: a read-only diagnosis agent. No code changed. No commits.

Two programmes handed these five reds forward without a diagnosis. This
document gives each one a class, a root cause with evidence, an exact fix and
a proof.

Class key:

- **(a) stale test** — the product deliberately changed. The fix updates the
  assertion to the ruled behaviour.
- **(b) regression** — the product is wrong. The fix is in production code.
- **(c) instrument defect** — the fixture or the measurement is wrong. The fix
  is in the test harness.

A caution about the measurements below. The agent ran every test through
`scripts/cargo_lane.sh`. The working tree carried uncommitted FEEDS_WAVE edits
in `crates/rs_cam_core/src/tool_load/mod.rs` and
`crates/rs_cam_core/src/tool_load/optimize/axes.rs` at the time. Test 5 reads
the tool-load band, so its number is a dirty-tree number.

---

## 1. `the_primary_and_the_builder_agree_about_a_runnable_project_ur3`

`crates/rs_cam_viz/src/controller/tests.rs:1129`

### Failure text

```
panicked at crates/rs_cam_viz/src/controller/tests.rs:1139:5:
fixture 1: the sample project must start ungenerated
```

### Class

**(c) instrument defect.** The test has never passed. It failed from its
first commit.

### Root cause

Fixture 1 asserts that `sample_controller()` starts with no generated result:

- assertion: `crates/rs_cam_viz/src/controller/tests.rs:1139-1147`

The shared fixture seeds a result:

- `sample_project_into` writes `rt.result = Some(ToolpathResult { .. })` at
  `crates/rs_cam_viz/src/controller/tests.rs:329-339`, then inserts the runtime
  at `:340`.

That seed predates the blamed commit. `git log -S 'rt.result = Some(ToolpathResult {'`
names `cbad95c7`, `6eb9ae55` and `f99e9e43`, all older than `f97327c3`.
`git show f97327c3^:crates/rs_cam_viz/src/controller/tests.rs` already carries
the seed at line 327. `git show f97327c3 -- crates/rs_cam_viz/src/controller/tests.rs`
removes one line, the diff header. The commit therefore only ADDED this test.
The premise was false when the author wrote it.

A second fixture in the same test carries the same class of defect. Fixture 4
asserts at `:1226-1235` that the stock-source edit drops the generated result.
It does not. The panel write-backs keep the GUI result on purpose:

- `write_entry_runtime_to_gui` lists `result` among the fields it does not
  touch — `crates/rs_cam_viz/src/ui/properties/mod.rs:3818-3827`.
- `write_entry_config_to_session` writes the session and marks the edit. It
  does not clear `gui.toolpath_rt` — `crates/rs_cam_viz/src/ui/properties/mod.rs:3779-3812`.
- The ruling is F2.2, quoted at `crates/rs_cam_viz/src/ui/readiness.rs:103-107`
  and at `crates/rs_cam_viz/src/ui/workspace_bar.rs:239-243`. Both say the
  same rule: the GUI store is "an operation KEEPS after an edit so the
  viewport can go on drawing it". The core result is the one the edit drops.
- The file already knows this. Three sites clear `rt.result` by hand after an
  edit: `:3407`, `:3974` and `:5326`.

Fixtures 2, 3 and 4 have never executed. The panic stops the test at fixture 1.

### The fix

Two edits, both in the test.

1. Fixture 1, `crates/rs_cam_viz/src/controller/tests.rs:1138`. Take the
   controller as `mut` and clear the seeded result before the assertion:

   ```rust
   let mut controller = sample_controller();
   // The shared fixture seeds a GUI result. Fixture 1 needs the
   // ungenerated state, so it drops that result first.
   for rt in controller.state.gui.toolpath_rt.values_mut() {
       rt.result = None;
   }
   ```

2. Fixture 4, `crates/rs_cam_viz/src/controller/tests.rs:1226-1235`. Replace
   the false precondition with the file's own idiom. Clear the results AFTER
   the `panel_edit`, then assert the state the fixture needs:

   ```rust
   for rt in controller.state.gui.toolpath_rt.values_mut() {
       rt.result = None;
   }
   assert!(
       controller.state.gui.toolpath_rt.values().all(|rt| rt.result.is_none()),
       "fixture 4: the phantom prior stock needs an ungenerated op (F2.2 keeps \
        the GUI result across an edit, so the fixture drops it by hand)"
   );
   ```

Neither edit changes the rules the test measures.

### Why the four fixtures then agree

- Fixture 1: no result, so `admissible == 0` and `phantom_scan.finish()` is
  `None` — `crates/rs_cam_viz/src/ui/readiness.rs:140-166`. The builder pushes
  no group and returns `None` —
  `crates/rs_cam_viz/src/controller/events/simulation.rs:186-212`.
- Fixture 3: verified. The session accepts an unresolvable `tool_id`. The
  `Command::ReplaceToolpathConfig` arm writes the configuration
  unconditionally and validates no tool
  (`crates/rs_cam_core/src/session/command.rs:2403-2432`), so the precondition
  at `:1211-1215` holds. `tool_id` 999 then resolves to no tool, and both
  sides drop the config — `readiness.rs:158-162` and `simulation.rs:157-165`.
  `StockSource::default()` is `Fresh`
  (`crates/rs_cam_core/src/compute/config.rs:6-12`), so the phantom scan
  answers `None` and neither side admits the setup.
- Fixture 4: `phantom_scan.visit(0, true, false, id, FromRemainingStock)`
  returns a phantom, so both sides say `true`.

### Proof

The test itself. Run
`scripts/cargo_lane.sh test -p rs_cam_viz -q --lib the_primary_and_the_builder_agree_about_a_runnable_project_ur3`.
No other sentry guards this pairing; the doc comment on
`simulation_request_is_buildable` names this test as the holder
(`crates/rs_cam_viz/src/ui/readiness.rs:137-139`).

### Risk

Medium. Fixtures 2, 3 and 4 have never run. The fix agent must run the whole
test, not only past fixture 1. Fixture 4 is the named second red; the two
edits above cover it. Fixture 3 is verified above and needs no edit. Fixture 2
remains unverified.
Test-file only. No production code. No machining output. No gate verdict.
No foreign-owned file.

---

## 2. `simulation_staleness_tracks_edits`

`crates/rs_cam_viz/src/controller/tests.rs:862`

### Failure text

```
panicked at crates/rs_cam_viz/src/controller/tests.rs:867:5:
Fresh simulation should not be stale
```

### Class

**(c) instrument defect**, against a rule the product changed on purpose in
(a). The fixture bypasses the submit that the new rule requires.

### Root cause

`f97327c3` derived metric-options staleness from the accepted revision. Its
own message states the rule: "an unstamped, cancelled or errored result can no
longer report a clean capture revision".

The mechanism:

- `is_stale` is a disjunction — `crates/rs_cam_viz/src/state/simulation.rs:1173-1176`.
- `metric_options_are_stale` compares the accepted revision with the live one
  — `crates/rs_cam_viz/src/state/simulation.rs:1165-1169`.
- The drain takes the accepted revision from the SUBMIT stamp —
  `crates/rs_cam_viz/src/controller/events/compute.rs:871-876`.
- Both fields start empty: `metric_options_revision: 0` and
  `submitted_metric_options_revision: None` —
  `crates/rs_cam_viz/src/state/simulation.rs:938-939`.

`inject_sim_results` pushes a `ComputeMessage::Simulation` straight into
`controller.compute.drained` and drains it
(`crates/rs_cam_viz/src/controller/tests.rs:1271-1314`). No submit runs. So
`accepted_metric_options_revision` is `None`, `None != Some(0)` holds, and
`is_stale` returns `true`.

The first disjunct does not cause the failure. `submitted_edit_counter` is
also `None`, so
`last_sim_edit_counter` falls back to the live counter
(`compute.rs:858-862`), and `current > last` is false.

The green sibling shows the pattern the test should use.
`a_simulation_with_no_edit_in_flight_is_current_g_latesim`
(`crates/rs_cam_viz/src/controller/tests.rs:5924`) calls
`generate_all_for_test`, then `AppEvent::RunSimulation`, then
`inject_sim_results`, and asserts `!is_stale`. It passes today.

### The fix

`crates/rs_cam_viz/src/controller/tests.rs:863-864`. Add the two lines that
stamp the run:

```rust
let mut controller = sample_controller();
generate_all_for_test(&mut controller);
// UR3 (f97327c3): an UNSTAMPED result reads stale, never current. A
// fresh-looking run must come through the submit that stamps the
// capture revision, as a real Run Simulation does.
controller.handle_internal_event(AppEvent::RunSimulation);
inject_sim_results(&mut controller, 1);
```

Do NOT change `inject_sim_results`. It has 19 call sites in this file, and
several of them depend on the unstamped path reading stale.

### Proof

The test itself, plus the two sentries that pin both sides of the rule:

- `a_simulation_with_no_edit_in_flight_is_current_g_latesim` — an unedited,
  stamped run is current.
- `metric_capture_toggle_before_first_result_tracks_in_flight_mismatch_without_dirtying_project`
  (`crates/rs_cam_viz/src/controller/tests.rs:888`) — it asserts
  `submitted_metric_options_revision == Some(revision_before + 1)` after
  `AppEvent::RunSimulation`, which proves the stamp fires under
  `ScriptedBackend`.

Both are green today.

### Risk

Low. Test-file only, two lines. No production code. No machining output.
No gate verdict. No foreign-owned file.

---

## 3. `freshness_does_not_outrank_a_collision`

`crates/rs_cam_viz/src/controller/tests.rs:5347`

### Failure text

```
panicked at crates/rs_cam_viz/src/controller/tests.rs:5365:5:
1 safety
```

### Class

**(a) stale test.** The badge text changed on purpose.

### Root cause

The hand-off blames `f97327c3`. The real cause is its neighbour, `e7901838`
("fix(ui): UR2 — a Danger badge is a dot too"). The two commits sit six
seconds apart in the same push: `e7901838` at 08:52:45 and `f97327c3` at
08:52:51 on 2026-09-15.

`e7901838` replaced two per-tab spellings with one shared text. Its own
message: "One shared 'N safety' text replaces the two per-tab spellings." The
diff changed `format!("{collisions} collision(s)")` to `collision_badge(collisions)`:

- the shared producer — `crates/rs_cam_viz/src/ui/workspace_bar.rs:208-210`,
  which returns `(format!("{collision_count} safety"), Role::Danger)`.
- the call site — `crates/rs_cam_viz/src/ui/workspace_bar.rs:257-258`.

`e7901838` updated its two integration sentries,
`crates/rs_cam_viz/tests/chrome_reads_the_kit_up3.rs` and
`crates/rs_cam_viz/tests/the_workspace_bar_is_a_strip_dc3.rs`. It missed this
controller unit test.

The test's second assertion is already correct. `collision_badge` returns
`Role::Danger`, so `assert_eq!(role, Role::Danger)` at `:5369` holds.

### The fix

`crates/rs_cam_viz/src/controller/tests.rs:5365`. Replace

```rust
    assert!(chip.contains("collision"), "{chip}");
```

with

```rust
    // UR2 (e7901838): one shared safety text replaces the two per-tab
    // spellings. The count, not the word "collision", is the claim.
    assert_eq!(chip, "1 safety");
```

`assert_eq!` is tighter than `contains`, and it still pins the SHE-003 order
the test is named for: one collision plus one stale operation reads as the
safety badge, not as "1 stale".

### Proof

The test itself, plus the two integration sentries that already hold the new
spelling: `crates/rs_cam_viz/tests/chrome_reads_the_kit_up3.rs` and
`crates/rs_cam_viz/tests/the_workspace_bar_is_a_strip_dc3.rs`. Both are green.
The order rule keeps its own neighbour sentry,
`freshness_counts_exclude_disabled_and_error`
(`crates/rs_cam_viz/src/controller/tests.rs:5375`).

### Risk

Low. Test-file only, one line. No production code. No machining output.
No gate verdict. No foreign-owned file.

---

## 4. `off_workspace_run_producers_hold_their_recorded_ruling_ur3`

`crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs:368`

### Failure text

```
panicked at crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs:383:9:
assertion `left == right` failed: ui/readiness_panel.rs holds 1 direct
RunSimulation producers, ruled at 3 (the Readiness workspace; no simulation
panel is on screen with it). A new affordance needs a new ruling in this
table, not a silent pass.
  left: 1
 right: 3
```

### Class

**(a) stale test.** The product deliberately removed two producers.

### Root cause

The hand-off note is correct. `b1f5182f` ("feat(ui): UR8 — Readiness leads
with the first unmet action") says so: "One action row below the checks
replaces every per-row remedy button and the second Run simulation."

`readiness_panel.rs` now carries exactly one direct producer. The counter
needle is `AppEvent::RunSimulation`
(`the_simulation_page_is_summary_first_dc6.rs:120-129`), and the file matches
it once, at `crates/rs_cam_viz/src/ui/readiness_panel.rs:50`
(`Self::RunSimulation => AppEvent::RunSimulation`). The other four hits on
`RunSimulation` are the `FirstUnmetAction` variant and its label, which the
needle does not match.

The ruling table was not updated with the change.

### The fix

`crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs:375-379`.
Change the tuple from `3` to `1` and record the ruling that made it 1:

```rust
        (
            "ui/readiness_panel.rs",
            1,
            "UR8 (b1f5182f): the ordered FirstUnmetAction row is the one \
             Readiness route; no simulation panel is on screen with it",
        ),
```

The `ui/menu_bar.rs` row needs no change. The loop reached `readiness_panel.rs`,
so the menu row already passed at 1.

### Proof

The test itself. Run
`scripts/cargo_lane.sh test -p rs_cam_viz -q --test the_simulation_page_is_summary_first_dc6`.
The census keeps its own non-vacuity arm, `the_scan_is_not_vacuous_dc6`
(`the_simulation_page_is_summary_first_dc6.rs:397`), so a moved or renamed
file fails first instead of passing silently.

### Risk

Low. Test-file only, one tuple. No production code. No machining output.
No gate verdict. No foreign-owned file.

---

## 5. `modulation_raises_cutting_chipload_toward_band`

`crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs:417`

### Failure text

```
panicked at crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs:470:5:
flag-ON median chipload 0.0221 should land in band [0.0320, 0.0550]
```

Arm 3 fails. Arms 1 and 2 pass, so the modulator still raises the median feed.

### Class

**(c) instrument defect.** The measurement mixes two populations.

### The attribution is wrong

The hand-off blames `e2ecf697`. That commit is
"fix(ui): the feeds inspector fits the 240-point simulation rail". Its
`--stat` lists only `crates/rs_cam_viz/src/ui/**` files. It cannot reach
`rs_cam_core` feed modulation.

The real cause is `fd135a04`, 2026-09-10, "fix(entry): G-RAMPCONTAIN — a prism
ramp folds along the operation's own following cut". It is an ancestor of
HEAD. Its own report diagnosed this red at the time.

### Root cause, on record since 2026-09-10

The report is `planning/ui_fix_2026-09-09/reports/J2.md` §4c. The structure
programme deleted that directory in `ea4d5bfb` ("docs(structure): purge the
superseded UI packages"). Read it with:

```
git show ea4d5bfb^:planning/ui_fix_2026-09-09/reports/J2.md
```

`a21a4a24` (WP26) cites the same report and calls this arm "red by design".

What J2 §4c measured. The test's `median` helper takes EVERY F word in the
emitted program — `collect_f_words`
(`crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs:252-268`),
used by `median` at `:423-428`. F192 is the ENTRY feed, not a cutting feed.
The ramp fold emits several segments per entry where the blind legs emitted
two. The entry population therefore grew from 42 of 138 F words to 90 of 216,
and the median crossed out of the cutting moves:

| population | median F | chipload | in band? |
|---|---:|---:|---|
| pre-fix, all F words | 1472 | 0.0409 | yes |
| post-fix, all F words | 770 | 0.0214 | no |
| pre-fix, cutting F words only | 1980 | 0.0550 | yes |
| post-fix, cutting F words only | 1980 | 0.0550 | yes |

The assertion's own comment says what it is about: "the bulk of cutting moves
are now correctly fed"
(`adaptive_feed_modulation_pipeline_f036b.rs:467-468`). The population does
not match the claim.

### The 2026-09-10 numbers no longer hold

Today's reading is 0.0221, not J2's 0.0214. The test's own constant is
`RPM_X_FLUTES = 36_000` (`:421`), so the median F word is now about 796
mm/min. That is neither the 192 entry feed nor the 770 nominal. WP26
(`a21a4a24`) names the producer of such a value: WP11b (`80a9cf4d`) routed
`feed_opt_stock` into the core generation door, its engaged arm writes
`nominal × rctf`, and `smooth_feed_rates` caps a move against its neighbour.
Both sit a few mm/min off 770.

Two consequences for the fix agent:

1. The population changed AGAIN after J2. J2's "cutting-only median = 1980,
   in band" is a 2026-09-10 result, not a result on this tree. The cutting-only
   filter is still the right instrument, but it is not guaranteed to go green.
2. The number 0.0221 comes from a dirty tree. FEEDS_WAVE held uncommitted
   edits in `tool_load/` while the agent ran the test.

### The fix

Three steps, in order.

1. Re-measure the F-word histogram on a clean tree, after FEEDS_WAVE lands.
   Do not apply a filter before the histogram says where the median sits.
2. Give `median` a cutting-only population, as J2 recommends. Take the set of
   feeds carried by non-entry moves in the modulated IR and filter the G-code
   F words to that set. Do NOT threshold on a feed value; a threshold silently
   drops a cutting move the modulator legitimately LOWERED. The band, the
   three assertions and the fixture stay as they are.
3. Keep the file's existing precedent. WP26 solved the sibling arm the same
   way: it diffed the pre-modulation and post-modulation move lists instead of
   using a feed proxy. The same IR is available here.

### Proof

The test itself, plus the file's other nine arms, which must stay green:

```
scripts/cargo_lane.sh test -p rs_cam_core -q --test adaptive_feed_modulation_pipeline_f036b
```

`flag_off_emits_identical_gcode_to_pre_f036` is the load-bearing regression
arm. `modulated_path_never_emits_below_min_chipload` is the floor arm that
WP26 repaired.

### Risk

High, and it needs an operator ruling. Reasons:

- J2 §4c itself rules the decision out of scope for a neighbouring session:
  "Re-pointing another programme's instrument — even at its own stated
  population — is a judgement that belongs to the operator or to that
  programme's owner."
- This sentry guards a FEEDS gate claim. A wrong re-point hides a real
  modulation regression. The memory rule applies: never gate on an aggregate
  without rendering the surface.
- The file sits beside live FEEDS_WAVE work. The fix agent must wait for that
  wave to land, then re-measure.

The fix is test-file only. It does not change machining output.

---

## Ranked fix list

Run them in this order. The first four are independent, small, and test-file
only.

| # | Test | Class | Fix size | File |
|---|---|---|---|---|
| 1 | `off_workspace_run_producers_hold_their_recorded_ruling_ur3` | a | one tuple | `crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs:375-379` |
| 2 | `freshness_does_not_outrank_a_collision` | a | one assertion | `crates/rs_cam_viz/src/controller/tests.rs:5365` |
| 3 | `simulation_staleness_tracks_edits` | c | two lines | `crates/rs_cam_viz/src/controller/tests.rs:863-864` |
| 4 | `the_primary_and_the_builder_agree_about_a_runnable_project_ur3` | c | two fixture edits | `crates/rs_cam_viz/src/controller/tests.rs:1138` and `:1226-1235` |
| 5 | `modulation_raises_cutting_chipload_toward_band` | c | re-measure, then re-point | `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs:423-428` |

Gate after fixes 1 to 4:

```
scripts/cargo_lane.sh test -p rs_cam_viz -q --lib
scripts/cargo_lane.sh test -p rs_cam_viz -q --test the_simulation_page_is_summary_first_dc6
```

## Operator rulings needed

1. **Test 5 only.** Re-pointing the f036b band arm to a cutting-only
   population is a change to another programme's sentry. J2 §4c put that
   decision with the operator or with the F-036b owner. The measurement is on
   record and nothing was relaxed.

No other test needs a ruling. Tests 1 to 4 change no production code, no
machining output and no gate verdict.

## Ownership check

- `crates/rs_cam_viz/src/controller/tests.rs` — last three commits are
  `91cf094e`, `81784e04`, `24f1ee4e`, all this structure programme's P2 module
  moves.
- `crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs` — last
  touched by `f97327c3` (2026-09-15).
- `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` — last
  three commits are P2 module moves.
- `planning/ui_premium_2026-09-13` belongs to the operator's other account. No
  fix in this document touches it.
- The worktrees `wt-ur3`, `wt-ur5` and `wt-ur8` sit at `9f074ef0`. Each is 200
  commits behind `master` and 0 ahead. Their UR packages already merged. They
  contain no work at risk.
- Live foreign work: the FEEDS_WAVE agents hold
  `crates/rs_cam_core/src/tool_load/mod.rs` and
  `crates/rs_cam_core/src/tool_load/optimize/axes.rs` dirty. Fix 5 waits for
  them.
