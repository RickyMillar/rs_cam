# F-034 — Cycle Time Estimator Brief (fresh session)

**Paste this into a new Claude session to land F-034 without depending on the prior session's context.**

You are the implementer for **F-034 — Acceleration-aware cycle time estimator** in the rs_cam Feed Modulation workstream.

## Context

The acceptance loop closed at 7/7 deflection bar (rounds 04-10). The simulator's verdicts (deflection, chipload, collisions) are now trustworthy. But `total_runtime_s` is computed as naive `distance / feed` — it ignores machine acceleration and jerk, so real cycle time is 30-50% longer than predicted on corner-heavy toolpaths. F-034 introduces a kinematics-aware integrator.

This is the **smallest, purely additive step** in the workstream. Land it first (after F-037 — see preconditions).

## Working directory

`/home/ricky/personal_repos/rs_cam`

## Read in order

1. **`planning/feed_modulation_roadmap.md`** — workstream narrative + feature-flag discipline.
2. **`planning/acceptance_loop/findings/F-034-machine-kinematics-cycle-time.md`** — the finding with fix shape + acceptance test + files.
3. **`planning/acceptance_loop/implementer_contract.md`** — implementer rules. Especially "Test through the production entry point" and the regression-protection rule (added by F-037).
4. **`CLAUDE.md`** at repo root — workspace lint policy.

## Preconditions (check before starting)

- **F-037 must have landed first.** Verify `planning/toolpath_acceptance/baselines/2026-05-26.csv` exists (or whatever baseline date F-037 captured). If not, **STOP and ask the user** — without the baseline, you have no regression net.
- `git log --oneline -3` shows F-037's commit at HEAD.
- `pgrep -af cargo` returns nothing (no concurrent cargo build).
- `cargo test --workspace -q` is clean on master.

## What to do

Follow F-034's fix shape (Part A: extend Machine config with kinematics; Part B: cycle time integrator). Concretely:

1. **Read the current `Machine` definition.** Likely in `crates/rs_cam_core/src/machine.rs` (verify with `rg "pub struct Machine"`). Add `MachineKinematics` struct as specified in F-034.

2. **Add kinematics to existing presets.** The current default ("Generic Wood Router") should get conservative defaults: `acceleration_mm_s2: 250.0` (Shapeoko XXL stock), `jerk_mm_s3: None`. Add a "Shapeoko XXL" preset if reasonable.

3. **Implement `compute_cycle_time`** as specified. Trapezoidal integrator (simplest viable). Walk `LinearMove` + `ArcMove`. Junction velocity = full-stop for v1 (refine later). Cap at `max_feed_mm_min`.

4. **Plumb into `SimulationResult`.** When `Machine::kinematics` is present, set `total_runtime_s = compute_cycle_time(...)`. When absent, leave today's naive calculation. **Purely additive — no flag needed; presence/absence of kinematics IS the flag.**

5. **Write acceptance tests** per F-034. The four tests:
   - `cycle_time_with_accel_model_below_naive_for_curve_heavy_toolpath`
   - `cycle_time_matches_naive_for_pure_straight_line`
   - `cycle_time_calibrated_against_shapeoko_reference` — **load-bearing**; needs a real-machine measurement
   - `flag_off_byte_identical_to_pre_f034`

6. **Calibrate against a real machine.** Ask the user to run a reference toolpath (suggest wanaka's Back Rough or a small AS001 pocket) on their Shapeoko XXL and report wall-clock time. Use that as `REFERENCE_MEASURED_S` in the calibration test, with ±10% tolerance. **DO NOT skip this step.** If the user can't measure right now, mark the test `#[ignore]` and open a follow-up finding for calibration, but capture the test scaffold.

7. **Run `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test -q`** — both clean.

8. **Re-run smoke baseline diff** (F-037's machinery): verify no regression on the 18-case suite + wanaka. Since F-034 only changes `total_runtime_s` (which the baseline tracks but not as a pass/fail), this should show "verdicts unchanged; total_runtime_s changed proportionally on corner-heavy cases." Document the runtime-s diffs in the PR description.

9. **Commit**: `feat(F-034): acceleration-aware cycle time estimator — closes F-034`. Include the calibration measurement in the body.

10. **Update finding frontmatter** to `Status: landed` + linked PR commit. **Append to STATE.md "Implementation log"**. Stop. Do NOT run the full audit smoke suite — that's the auditor's job in the next round.

## Hard rules

- One finding, one PR.
- **Don't touch existing acceptance tests** (`_f0{24,26,27,28,31}.rs`). They must continue to pass byte-identical.
- **Don't introduce a feature flag** — purely additive, behavior controlled by Machine::kinematics presence/absence.
- **Don't bundle F-035 or F-036 work.** Their kinematics-consumption is later.
- If F-034 turns out to be > 2× M effort estimate, stop and flag the user.
- Calibration test is the load-bearing test. If you can't get a real-machine measurement, flag explicitly rather than fabricating a number.

## When to stop and ask

- The `Machine` struct doesn't exist where expected — find it first, ask if unclear
- The trapezoidal integrator's correctness can't be cleanly proven against one of the four acceptance tests
- The calibration measurement deviates from the model by > 30% — the kinematics model needs refinement before F-034 lands
- Implementer-contract's smoke-regression rule (F-037) reports any verdict regression — feature is supposed to be purely additive

## When done

Output a brief summary:
- Commit SHA
- Files touched (verify via `git diff --stat`)
- Calibration measurement and model prediction (with diff %)
- Smoke baseline diff summary (should be: verdicts unchanged, total_runtime_s increases on corner-heavy cases)
- Clippy + tests status

The auditor (separate session) will verify in a subsequent round.
</parameter>
</invoke>