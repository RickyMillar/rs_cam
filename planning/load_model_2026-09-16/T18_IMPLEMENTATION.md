# T-18 — a feed lift caps at the CUTTING ceiling

Written 2026-09-18. Branch `master`, parent `59822cab`. Not committed; the
orchestrator reviews and commits.

---

## 1. The rule applied

The code names two axes (`feeds/mod.rs`, the F-2 block):

- RAW axis — the feed before Step 9.
- COMMANDED axis — the feed the machine receives, after Step 9.

Step 7 caps the RAW feed at `cutting_feed_ceiling_mm_min()` with no factor.
Step 9 multiplies the feed by `safety_factor`. The calculator's output
invariant is therefore

```text
commanded_feed <= cutting_feed_ceiling_mm_min() * safety_factor
```

Every lift below runs AFTER Step 9, so every lift works on the COMMANDED
axis. Their `* safety_factor` was the right shape. Their base quantity was the
wrong one: the gantry TRAVEL rate instead of the cutting ceiling.

`MachineProfile::commanded_cutting_feed_ceiling_mm_min()` states the
right-hand side once, beside `cutting_feed_ceiling_mm_min`. Its doc names both
axes with the F-2 block's words and says which step produces each.

---

## 2. The red run on the parent

The sentry was written first and run on `59822cab` before any source change.
Command:

```
scripts/cargo_lane.sh test -p rs_cam_core -q \
  --test a_feed_lift_caps_at_the_cutting_ceiling_g_t18 -- --nocapture --test-threads 1
```

Output:

```text
running 4 tests
step 9c: rpm 8000, feed 600.000 mm/min, cutting cap 400.000, travel cap 8000.000, clamped true

thread 'the_drill_envelope_clamp_caps_at_the_commanded_cutting_ceiling' panicked at
crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs:250:5:
the drill envelope clamp restored 600.000 mm/min, above the commanded cutting ceiling
400.000 mm/min; it capped against the travel rate 8000.000 mm/min instead
the_drill_envelope_clamp_caps_at_the_commanded_cutting_ceiling --- FAILED
preset: feed 889.342 mm/min against the fixture cap 400.000 mm/min
.step 9b: rpm 15000, feed 750.000 mm/min, cutting cap 400.000, travel cap 8000.000, lifted true

thread 'the_rubbing_floor_lift_caps_at_the_commanded_cutting_ceiling' panicked at
crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs:214:5:
the rubbing-floor lift restored 750.000 mm/min, above the commanded cutting ceiling
400.000 mm/min; it capped against the travel rate 8000.000 mm/min instead
 2/4
the_rubbing_floor_lift_caps_at_the_commanded_cutting_ceiling --- FAILED
pass 9: feed 800.000 mm/min, cutting cap 400.000, travel cap 8000.000, lifted true
pass 9 geometry: calculator ap 9.0000 ae 1.2000, final ap Some(4.096000000000001)
  ae Some(1.2), rpm 16000
pass 9 warnings: [DppCappedByDeflection { requested_mm: 8.0, capped_mm: 4.096000000000001,
  predicted_um_at_requested: 325.84463271705744, predicted_um_at_capped: 173.1195879668656,
  iterations: 3 }, FeedRescaledToFinalGeometry { requested_mm_per_min: 800.0,
  rescaled_mm_per_min: 500.0, factor_at_calculator: 0.75, factor_at_final: 1.0,
  cap_hit: Some(MaxFeed) }, FeedClampedToChiploadFloor { requested_mm_per_tooth: 0.015625,
  floor_mm_per_tooth: 0.025, band_capped_from: None }]

thread 'the_suggest_floor_lift_caps_at_the_commanded_cutting_ceiling' panicked at
crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs:329:5:
pass 9's floor lift restored 800.000 mm/min, above the commanded cutting ceiling
400.000 mm/min; it capped against the travel rate 8000.000 mm/min instead
the_suggest_floor_lift_caps_at_the_commanded_cutting_ceiling --- FAILED

failures:
    the_drill_envelope_clamp_caps_at_the_commanded_cutting_ceiling
    the_rubbing_floor_lift_caps_at_the_commanded_cutting_ceiling
    the_suggest_floor_lift_caps_at_the_commanded_cutting_ceiling

test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Three arms red, each by the exact defect. The fourth arm (the preset
equality, the non-vacuity anchor) passed on the parent and must keep passing.

That run carried six `println!` calls, which is how the numbers above reached
this report. They are **gone from the shipped sentry**: the crate rule
(`crates/rs_cam_core/CLAUDE.md`, "Tests and evidence") allows a test module
only `unwrap_used`, `expect_used`, `panic` and `indexing_slicing`, and keeps
`println!` denied. Every number a print carried now sits in the assertion
message that would report it — the rpm and the shipped feed in each arm's
message, and the calculator-against-final geometry in the pass 9 non-vacuity
message, where it is the first thing to read because that pass short-circuits
when the depth tier does not move. Section 8 quotes the re-run.

The red numbers name themselves. The fixture profile has travel 10 000
mm/min, an explicit cutting ceiling of 500 mm/min, and `safety_factor` 0.8, so
the commanded cutting ceiling is 400 mm/min and a fraction of the travel rate
is 8 000 mm/min.

| Lift | Target it wanted | Shipped on the parent | Ceiling it broke |
|---|---|---|---|
| Step 9b rubbing floor | 0.025 × 15 000 × 2 = 750 | 750 | 400 |
| Step 9c drill envelope | 50 mm/min/mm × 12 = 600 | 600 | 400 |
| Pass 9 floor lift | 0.025 × 16 000 × 2 = 800 | 800 | 400 |

Each shipped feed sits 1.5× to 2× above the ceiling Step 7 enforced.

---

## 3. The four sites, before and after

| # | Site | Before | After |
|---|---|---|---|
| 1 | `feeds/mod.rs` Step 9b, rubbing-floor lift | `machine.max_feed_mm_min * machine.safety_factor` | `machine.commanded_cutting_feed_ceiling_mm_min()` |
| 2 | `feeds/mod.rs` Step 9c, drill envelope clamp | `machine.max_feed_mm_min * machine.safety_factor` | `machine.commanded_cutting_feed_ceiling_mm_min()` |
| 3 | `feeds/suggest/adaptive_entry.rs`, pass 9 floor lift | `machine.max_feed_mm_min * machine.safety_factor` | `machine.commanded_cutting_feed_ceiling_mm_min()` |
| 4 | `feeds/suggest/adaptive_entry.rs`, "Step 7 re-applied" | `machine.cutting_feed_ceiling_mm_min()` (RAW) | `machine.commanded_cutting_feed_ceiling_mm_min()` |

Each site's comment now names the cap and the axis. The old wording, "the
machine cap (post-safety) still wins", said neither.

The helper carries the doc that states the invariant, names both axes and says
which step produces each. Its unit test,
`commanded_cutting_ceiling_is_the_ceiling_after_the_safety_factor`, sits in the
machine module's existing test block and pins the helper against all three
presets plus the fixture profile.

---

## 4. Did any preset number move? No.

All three presets carry a travel rate under
`DEFAULT_CUTTING_FEED_CAP_MM_MIN` (4 000, 5 000 and 5 000 against 6 000), so

```text
cutting_feed_ceiling_mm_min() == max_feed_mm_min
```

on every one. The old expression and the new one are therefore the same
number, exactly, and the swap is a no-op there. Two tests assert this:

- `the_fix_is_silent_on_every_shipped_preset` in the sentry.
- `commanded_cutting_ceiling_is_the_ceiling_after_the_safety_factor` in
  `machine/mod.rs`.

The same test also runs the sentry's operation on a preset and shows it ships
889.342 mm/min, well above the fixture's 400 mm/min cap. The fixture arms
therefore measure the fixture, not an accident of the operation.

One caution for the reviewer: the pinned project fixture
`tests/fixtures/wanaka_2026-08-16_f530995a.toml` carries `max_feed_mm_min =
10000.0` with no explicit cutting cap, so its ceiling is 6 000 and its two
caps are 7 500 (old) and 4 500 (new). A lift on that project COULD move. It
does not: `suggest_feed_matches_final_geometry` prints the only lift that
fires there as `FeedRescaledToFinalGeometry { rescaled_mm_per_min: 1000.0,
cap_hit: None }`, which is under both caps. The wanaka project is not a
shipped preset, so this is a note, not an exception to the claim above.

---

## 5. `adaptive_entry.rs:421` — the "Step 7 re-applied" cap

Moved to the helper. It was the same axis mismatch in the other direction.

`rescaled` derives from `calc.feed_rate_mm_min`, which calculator Step 9 has
already multiplied by `safety_factor`, so `rescaled` sits on the COMMANDED
axis. The RAW cap there let a rescaled feed reach `cutting_feed_ceiling`,
which is `1 / safety_factor` (1.25× to 1.33×) above the highest feed the
calculator itself can emit.

**No existing test failed from this change.** No test pinned the RAW cap
there, so the ledger-and-leave branch of the instruction did not apply. The
measurement:

- `suggest_power_ceiling_after_pass9_g_suggest_powerstale` — 5/5 green. This
  is the file that documents pass 9's re-applied clamps.
- `suggest_feed_matches_final_geometry` — the one arm that reads this site
  (`Back Rough`) prints `cap_hit: None` both before and after, because 1 000
  mm/min sits under 6 000 and under 4 500 alike.
- The core lib suite, 2 521 green.

The site's comment now records why the axis is COMMANDED there.

---

## 6. One guard, or four caps? An assessment

**A single guard is the better structure, and this change does not build it.**

The shape is now explicit. Four sites, plus T-15's fifth, each re-state a
ceiling that an earlier step already enforced, and each one can be individually
correct while the composition is not. The register's own words apply: a fix
that only patches its own site leaves the shape intact. T-18 patched four
sites.

Where such a guard would sit: at the very end of `feeds::calculate`, after
Step 9c, as the last statement before the `FeedsResult` is assembled — and
again at the end of `enforce_invariants`, after pass 9, because Suggest's
passes run outside the calculator and can move the feed after it returned.
Two sites, not one, because there are two places where "the last lift" can
happen.

What it would need to know, and this is why it is not a small job:

1. **Every ceiling, re-evaluated at the final operating point.** The machine
   cutting ceiling is cheap: one function of the profile. The power ceiling is
   not — `PowerTerms` needs the final `ap`, `ae`, RPM and feed, and pass 9 can
   move `ap` and `ae` after the calculator sized it. That is exactly the open
   `G-SUGGEST-POWERSTALE` row, and it is why pass 9 declines to re-check power
   today.
2. **Which direction each ceiling binds from, and what to do when two
   conflict.** The rubbing floor pushes UP and the ceilings push DOWN. Today
   each lift resolves its own conflict locally and emits the warning that says
   so. A single guard has to resolve them in one place, and it must not silently
   delete a warning that an earlier step already filed. A guard that lowers a
   feed the floor lift just raised, without a word, would replace a visible
   conflict with an invisible one.
3. **The rung it must not undo.** T-15 is the power ladder's version. The
   ladder lowers RPM, depth and width before it touches the feed; a guard that
   only lowers the feed at the end would leave the ladder's other rungs
   pointing at an operating point that no longer exists.

So the recommendation: a single guard is worth building, but it belongs to
T-15's work, not to T-18's, because T-15 owns the hard half (the power
ceiling re-evaluation). T-18's four caps are the cheap half, they are correct
now, and they give the guard a helper to call rather than an expression to
re-derive.

---

## 7. What the plan did not anticipate

1. **A fourth site, not three plus a question.** The plan asked the agent to
   measure `adaptive_entry.rs:421` and report before changing behaviour. The
   measurement came back clean, so the change landed. The register entry now
   lists four rows.

2. **The sentry's third arm needed a tier crossing, not just a clamp.** Pass
   9 short-circuits unless the depth tier actually moves, and
   `depth_tier_multiplier` is a four-step staircase at `ap / d` of 1, 2 and 3.
   A pocket rough on a 6 mm cutter sits at tier 1.0 both before and after the
   rigidity clamp, so the pass never ran. The arm needed an **adaptive** rough
   (the calculator gives it `ap = 1.5 × d`, tier 0.75) with a 90 mm stickout,
   so the deflection back-off brings the depth under one diameter and the tier
   moves to 1.0. That is a real operating point, but it is a narrower target
   than the plan implies.

3. **The fixture rpm is not 18 000.** The plan's worked example assumed 18 000
   rpm × 2 flutes for a 900 mm/min floor target. The engine picks 15 000 for
   the pocket arm, 16 000 for the adaptive arm and 8 000 for the drill arm. The
   targets are 750, 800 and 600 mm/min. All three still sit between 400 and
   8 000, so the fixture works; the numbers in the sentry's doc block are the
   measured ones.

4. **The drill arm needs a 12 mm cutter, not a 6 mm one.** Step 9c's envelope
   floor is `50 mm/min/mm × d` for solid wood. On a 6 mm drill that is 300
   mm/min, which is BELOW the fixture's 400 mm/min cap, so neither cap binds
   and the arm proves nothing. A 12 mm drill gives 600 mm/min and separates
   them.

5. **An unrelated red, from a concurrent session.**
   `suggest_feed_matches_final_geometry::a_raised_stepover_does_not_move_the_feed_at_all`
   is RED in the working tree, and **not from this change.** The T-9 session
   has landed a round-DOWN in `feeds/suggest/apply.rs`
   (`round_suggestion_value_down`). The wanaka `3D Finish 6` case now ships
   404.000 mm/min against a calculator feed of 404.759, a gap of 0.759, and
   that test's tolerance is `0.5 + calculator_feed * 1e-9` — the half-step of
   a NEAREST rounding. A round-down needs a tolerance of one full step. The
   assertion's own message is about a geometry multiplier in the feed
   expression, which is not what happened.

   Evidence it is not T-18: the failing case files no lift warning at all
   (`FinishEnvelopeAdvisory`, `StepoverRaisedForRuntime`,
   `CutGeometryFieldNotHeld` only), so none of the four sites executes on it,
   and the whole delta is the rounding step. **This is the T-9 session's to
   fix; I did not touch that file.**

---

## 8. Verification

Every command ran through `scripts/cargo_lane.sh`.

```
scripts/cargo_lane.sh fmt --all -- --check
```
No output. Clean.

```
scripts/cargo_lane.sh test -p rs_cam_core -q --test a_feed_lift_caps_at_the_cutting_ceiling_g_t18
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

```
scripts/cargo_lane.sh test -p rs_cam_core --lib -q
test result: ok. 2521 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 34.69s
```

```
scripts/cargo_lane.sh test -p rs_cam_core -q --test rubbing_floor_never_exceeds_band
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

scripts/cargo_lane.sh test -p rs_cam_core -q --test lookup_parity
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

scripts/cargo_lane.sh test -p rs_cam_core -q --test lut_resolver_census_a6
test result: ok. 3 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Four further sentries, run because they read the changed sites:

```
scripts/cargo_lane.sh test -p rs_cam_core -q --test suggest_power_ceiling_after_pass9_g_suggest_powerstale
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

scripts/cargo_lane.sh test -p rs_cam_core -q --test power_ceiling_parity_f2
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

scripts/cargo_lane.sh test -p rs_cam_core -q --test drill_runtime_survives_retime_n2
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s

scripts/cargo_lane.sh test -p rs_cam_core -q --test suggest_feed_matches_final_geometry
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.93s
```

The last one is the T-9 round-down described in section 7, item 5. It is red
for a cause outside this change.

```
scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets \
  --features heavy-tests,research,test-support -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.11s
```
No warnings.

### Re-run after the `println!` removal

The six `println!` and the `clippy::print_stdout` allow came out of the
sentry, and the three commands the reviewer named ran again:

```
scripts/cargo_lane.sh fmt --all -- --check
```
No output. Clean.

```
scripts/cargo_lane.sh test -p rs_cam_core -q --test a_feed_lift_caps_at_the_cutting_ceiling_g_t18
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

```
scripts/cargo_lane.sh clippy -p rs_cam_core --all-targets \
  --features heavy-tests,research,test-support -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.97s
```
No warnings.

The core lib suite and the other sentries above are unaffected: the removal
touches the sentry's own assertion messages only, and no source file moved
with it.

Not run, by the standing rules: the core full gate, workspace-wide tests and
`wanaka_suggest_integration`.

---

## 9. Paths changed or created

Changed:

- `crates/rs_cam_core/src/machine/mod.rs`
- `crates/rs_cam_core/src/feeds/mod.rs`
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs`
- `planning/TECH_DEBT_REGISTER.md` (the T-18 row and the T-18 entry only)

Created:

- `crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs`
- `planning/load_model_2026-09-16/T18_IMPLEMENTATION.md`

Nothing committed. Nothing staged.
