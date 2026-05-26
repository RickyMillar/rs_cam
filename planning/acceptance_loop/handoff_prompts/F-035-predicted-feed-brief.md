# F-035 — Predicted-Effective-Feed in Gates Brief (fresh session)

**Paste this into a new Claude session to land F-035 without depending on prior session context.**

You are the implementer for **F-035 — Predicted-effective-feed in chipload / power / deflection gates** in the rs_cam Feed Modulation workstream.

## Context

F-034 introduced `MachineKinematics` and the cycle-time integrator. F-035 makes the simulator's gates consume that kinematics model: when a `use_predicted_feed_in_gates` flag is on, the chipload / power / deflection gates use the *achieved* feed (what the machine actually reaches through corners under accel/jerk limits) instead of the commanded feed.

The bug this catches: on hobby-class machines (Shapeoko XXL with stock 250 mm/s² accel), the controller decelerates through corners. Commanded feed = 4000 mm/min, achieved feed = 2000 mm/min through a tight turn. Today's gate uses 4000 → reports chipload Within. Reality: chipload at 2000 = Exceeds_LOW → rubbing.

## Working directory

`/home/ricky/personal_repos/rs_cam`

## Read in order

1. **`planning/feed_modulation_roadmap.md`** — workstream narrative.
2. **`planning/acceptance_loop/findings/F-035-predicted-feed-in-gates.md`** — the finding with fix shape + acceptance test + files.
3. **F-034's commit** (`git show <F-034 SHA>`) — read the `MachineKinematics` struct and the cycle-time integrator. F-035 reuses both.
4. **`planning/acceptance_loop/implementer_contract.md`** — implementer rules + F-037's smoke-regression rule.
5. **`CLAUDE.md`** — workspace lint policy.

## Preconditions

- **F-034 must have landed.** Verify `MachineKinematics` struct exists in `crates/rs_cam_core/src/machine.rs` (or wherever F-034 placed it). If not, **STOP**.
- **F-037 must be in place.** Baseline + smoke diff machinery must exist.
- `git log --oneline -5` should show F-037 and F-034 commits at HEAD.
- `pgrep -af cargo` returns nothing.
- `cargo test --workspace -q` clean.

## What to do

Follow F-035's three steps:

1. **Step A — predicted-feed function.** Implement `predicted_achieved_feed(move, prev, next, kinematics, max_feed) -> f64` in the same module that hosts F-034's integrator (`crates/rs_cam_core/src/machine_kinematics/` or similar). Algorithm: given start velocity (junction with prev), commanded feed, end velocity (junction with next), find peak velocity reached within the move's distance under accel/jerk limits. For long moves, returns commanded; for short moves between corners, returns < commanded.

2. **Step B — feature flag on `SimulationOptions`.** Add `pub use_predicted_feed_in_gates: bool` (default `false`). Find `SimulationOptions` in `crates/rs_cam_core/src/compute/simulate.rs` or similar.

3. **Step C — gate consumption.** Update chipload + power + deflection gate sample loops to read the predicted feed when flag is on. **Extract a shared helper** to avoid drift between the three gates:

   ```rust
   fn effective_feed_for_sample(sample: &Sample, move_idx: usize, toolpath: &Toolpath, machine: &Machine, options: &SimulationOptions) -> f64 {
       if options.use_predicted_feed_in_gates && machine.kinematics.is_some() {
           predicted_achieved_feed(&toolpath.moves[move_idx], ..., machine.kinematics.as_ref().unwrap(), machine.max_feed_mm_min)
       } else {
           sample.move_feed_rate
       }
   }
   ```

   All three gates (chipload, power, deflection) call this helper.

4. **Write acceptance tests** per F-035. The four tests:
   - `flag_off_byte_identical_to_pre_f035` — load AS001, run with kinematics set + flag off, verify verdicts byte-identical to pre-F-035 HEAD
   - `flag_on_corner_decel_drops_chipload_below_band` — synthetic tight-corner toolpath, flag-off Within, flag-on Exceeds_LOW
   - `flag_on_straight_line_chipload_unchanged` — straight cut, flag-on == flag-off
   - `flag_on_extends_existing_f024_test_invariants` — AS001 with flag on, peak_axial_doc_mm still matches commanded DOC

5. **Run `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test -q`** — clean. ALL existing tests pass byte-identical (flag is default-off).

6. **Re-run smoke baseline diff** (F-037 machinery): verify zero regression. Since flag is default-off in smoke, this MUST show "verdicts unchanged."

7. **Commit**: `feat(F-035): predicted-effective-feed in chipload/power/deflection gates — closes F-035`. Body explains the bug class + new flag + protection.

8. **Update finding frontmatter** to `Status: landed`. **Append to STATE.md "Implementation log"**. Stop.

## Hard rules

- **One finding, one PR.**
- **`use_predicted_feed_in_gates` defaults to `false`.** This is non-negotiable — protects the loop's calibration.
- **Don't touch existing acceptance tests** (`_f0{24,26,27,28,31}.rs`). They run flag-off and must continue to pass byte-identical.
- **Extract the feed-resolution helper.** All three gates use the same helper; don't duplicate logic. Drift between gates is a class of bug we already had (F-024 site duplication) and F-030 retired it.
- **Don't bundle F-036's modulation.** F-035 only changes gate evaluation; the emitted G-code is untouched.
- If F-035 turns out to be > 2× M effort, stop and flag the user.

## When to stop and ask

- F-034's `MachineKinematics` doesn't have the fields F-035 needs — go back to F-034
- The three gate files have different sample-loop shapes and the extracted helper doesn't fit cleanly — flag the architectural mismatch
- The smoke baseline shows a regression in flag-off mode — that's a bug in F-035's plumbing, fix before declaring done

## When done

Output:
- Commit SHA
- Files touched
- Before/after AS001 chipload Within values (should be byte-identical flag-off, may differ flag-on if AS001 has corners)
- Smoke baseline diff summary (should be: zero regressions)
- Clippy + tests status
</parameter>
</invoke>