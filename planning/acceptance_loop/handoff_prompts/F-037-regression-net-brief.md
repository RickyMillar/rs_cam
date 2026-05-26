# F-037 — Smoke Baseline + Regression Net Brief (fresh session)

**Paste this into a new Claude session to land F-037 without depending on prior session context.**

**LANDS FIRST in the Feed Modulation workstream.** F-034, F-035, F-036 all depend on F-037 being in place to protect the acceptance loop's calibration from regression.

You are the implementer for **F-037 — Smoke baseline + wanaka regression case + CI gate** in the rs_cam Feed Modulation workstream.

## Context

The acceptance loop closed at 7/7 deflection bar (rounds 04-10). The regression net is currently 5 cargo acceptance tests + 18-case smoke CSV run ad-hoc. F-037 adds:
1. A baseline snapshot of today's smoke verdicts (a checked-in CSV with every per-toolpath verdict)
2. Wanaka added to the regression matrix (real-world project, real face_up=Bottom non-identity setup)
3. A CI gate that diffs new runs against the baseline and fails on regression

Without F-037, F-034/35/36 land with no automated guarantee that the loop's calibration stays intact.

## Working directory

`/home/ricky/personal_repos/rs_cam`

## Read in order

1. **`planning/feed_modulation_roadmap.md`** — workstream narrative.
2. **`planning/acceptance_loop/findings/F-037-smoke-baseline-and-regression-net.md`** — the finding with the three parts (baseline, wanaka, CI).
3. **`planning/acceptance_loop/rounds/round-10-2026-05-26/delta.md`** — round-10 final delta (baseline verdicts at the loop's close; what F-037 captures).
4. **`planning/acceptance_loop/implementer_contract.md`** — existing implementer rules; F-037 updates this.
5. **`planning/toolpath_acceptance/cases_agent_smoke.csv`** — current smoke matrix (will add WANAKA row).
6. **`CLAUDE.md`** — workspace lint policy.

## Preconditions

- `pgrep -af cargo` returns nothing.
- `cargo test --workspace -q` clean.
- The user is available to:
  - Confirm wanaka project file licensing (can we check in a subset for tests?)
  - Provide a real-machine cycle-time measurement IF the implementer wants to bake that into the baseline (optional; not strictly required for F-037)
- The acceptance loop is closed (`STATE.md` shows round-10 closed with 7/7 bar met).

## What to do

Three parts, each can be a separate PR (sequenced):

### PR 1 — Capture today's baseline

1. **Run the full smoke suite.** Use whatever existing tooling the loop has (likely `cargo run -p rs_cam_cli -- sweep ...` or similar; check `crates/rs_cam_cli/`). Output should be a `results.csv` with one row per toolpath: `case_id, op_kind, chipload_kind, chipload_observed_mm_tooth, deflection_kind, deflection_peak_mm, power_kind, power_peak_kw, rapid_collision_count, avg_engagement, peak_axial_doc_mm, drill_chip_welding_kind, drill_chip_welding_observed, drill_peck_kind, drill_plunge_kind`.

   If no such tooling exists, implement a minimal smoke-runner CLI subcommand (`cargo run -p rs_cam_cli -- smoke --output baseline.csv`) that:
   - Reads `cases_agent_smoke.csv`
   - For each row, loads the template, adds the op with the row's params, generates, simulates
   - Writes the verdicts to baseline.csv

2. **Verify the baseline matches round-10's reported verdicts.** Cross-check against `rounds/round-10-2026-05-26/delta.md`. If any verdict in the baseline differs from round-10's reported value, **STOP** — there's a state issue.

3. **Commit baseline:** `planning/toolpath_acceptance/baselines/2026-05-26.csv` (or use today's date). Commit message: `docs(F-037): capture round-10 smoke baseline as regression net foundation`.

### PR 2 — Add wanaka to the regression matrix

1. **Confirm with user**: can the wanaka project file (or a sanitized subset) be checked into `test_data/`? If yes, copy it as `test_data/wanaka_subset.toml`. If no, document the wanaka run separately (a `WANAKA` row in the smoke CSV that points to a user-local path the CI can mount).

2. **Add `WANAKA` row to `cases_agent_smoke.csv`** with the project template, all 8 toolpaths' params. Use the existing CSV format; if multi-setup needs new columns, add them.

3. **Run smoke on wanaka, capture verdicts** in `baselines/2026-05-26.csv` (or append to it). Expected from round-10 audit:
   - 7 cutting toolpaths, all deflection Within (0.005-0.114 mm peaks)
   - 0 rapid collisions across 8 toolpaths
   - 2 chipload Within (vendor LUT), 2 chipload Exceeds_LOW (deliberate user low-feed on Rivers/Lakes)
   - 1 drill gate elevated (Pin Drill chip_welding 4.5/6.0)

4. **Commit**: `docs(F-037): add wanaka to smoke regression matrix as real-world reference`.

### PR 3 — CI gate

1. **Build the smoke-diff CLI**: `cargo run -p rs_cam_cli -- smoke --diff <baseline.csv> --current <run.csv>`:
   - Loads both CSVs.
   - For each case, compares verdict fields (chipload_kind, deflection_kind, rapid_collision_count are the critical ones).
   - Exits 0 if all match (or improve in a defined direction).
   - Exits non-zero with a clear "regression on case X: chipload Within → Exceeds" output if anything regresses.

2. **GitHub Actions workflow** at `.github/workflows/smoke.yml`:
   - Trigger: nightly + on PRs touching `crates/rs_cam_core/src/{simulation,tool_load,compute,toolpath}/` or `crates/rs_cam_viz/src/{compute,controller}/`.
   - Steps: checkout, build release, run smoke + diff, fail on non-zero exit.

3. **Update `implementer_contract.md`** with the new rule:

   ```
   ### Regression-net rule (added by F-037)
   
   Any PR touching simulator-adjacent code (sim, tool_load, compute, toolpath, viz worker, viz controller) must run the full smoke suite locally and attach results.csv to the PR description. Pass criterion: every existing case's verdict (chipload kind, deflection kind, rapid_collision_count) is unchanged or improved. Worse verdicts require justification + finding cross-link.
   
   The CI gate (.github/workflows/smoke.yml) enforces this automatically on relevant paths.
   ```

4. **Acceptance test** at `crates/rs_cam_core/tests/smoke_baseline_regression_f037.rs`:
   - Test loads the baseline, verifies parse-cleanly.
   - Test runs smoke (or a representative subset), diffs against baseline, asserts no regression.
   - Mutation test: artificially flip one verdict, assert the diff catches it.

5. **Commit**: `feat(F-037): smoke baseline diff CLI + CI gate — closes F-037`.

## Hard rules

- **PR 1 lands before F-034 starts.** PR 2 + PR 3 can sequence after F-034 if needed, but F-037's first PR is the gate.
- **Don't change the smoke methodology.** F-037 just captures and protects today's verdicts; the loop's acceptance criteria don't change.
- **Don't change baseline.csv casually.** Any future PR that needs to update the baseline must include a finding-file linkage explaining why a verdict changed (e.g. F-031 took AS013 deflection 0.637 → 0.105; that was a legitimate improvement).
- **Don't bundle CI infra refactoring with F-037.** Keep the workflow file minimal; refactor later if needed.

## When to stop and ask

- The user can't share the wanaka project file → PR 2 needs alternate approach (user-local config or sanitized geometry)
- The smoke-runner CLI doesn't exist and adding it is its own L-effort task → split into a prerequisite finding
- The CI workflow integration fails to authenticate / build → flag user; CI infra is environment-specific
- The baseline diff catches "regressions" that turn out to be improvements (e.g. AS015 chipload going from Exceeds_LOW to Within because a fix landed) → tighten the diff semantics to allow improvements

## When done

Output (potentially across the 3 PRs):
- Commit SHAs
- baseline.csv path (committed)
- WANAKA row added to smoke matrix
- CI workflow file path and a test PR confirming it fires on a known regression
- implementer-contract update text
- Clippy + tests status

The user will verify the CI gate fires on a known regression scenario.
</parameter>
</invoke>