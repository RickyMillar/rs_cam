# F-037 — Smoke baseline + wanaka regression case + CI gate

- **Stage:** loop machinery / process (foundation for F-034/35/36)
- **Severity:** medium (preserves the acceptance loop's calibration from feature-work regressions)
- **Status:** open — **lands first** in the Feed Modulation workstream
- **First found in:** round-10 user discussion (2026-05-26)
- **Effort:** M
- **Linked PRs:** —
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Prerequisite for:** F-034, F-035, F-036
- **Source audits:** round-10 wanaka audit + user discussion

## Symptom

The acceptance loop closed at 7/7 deflection bar via 10 rounds of calibration work. The regression net for that calibration is currently:

- 5 cargo acceptance tests (`_f0{24,26,27,28,31}.rs`)
- 2 viz-path tests (`compute/worker/tests.rs`, `controller/tests.rs`)
- 18-case smoke CSV (`cases_agent_smoke.csv`) run ad-hoc by the auditor

What's missing:
1. **No baseline snapshot** of today's verdicts. A regression in the smoke suite is only detectable by auditor judgment, not by automated comparison.
2. **No CI integration.** Smoke runs are auditor-triggered, not pre-merge.
3. **No real-world project in the regression net.** The 18 smoke cases are synthetic; wanaka exposed real-world setups (face_up=Bottom, bull-nose endmill, multi-setup) that the synthetic suite doesn't cover.

Adding F-034/F-035/F-036 introduces risk. Without F-037 first, a feed-modulation regression could land silently and only surface in an auditor's next manual round.

## Hypothesised root cause

Process gap. The loop's machinery (finding files, implementer contract, audit runbook) was designed for the calibration phase where auditor judgment was the primary regression catch. For feature-work phases, the same machinery + automated baseline diff is needed.

## Fix shape

**Three parts**:

### Part A — capture today's baseline

Run the full 18-case smoke suite + wanaka. Snapshot every per-toolpath verdict (chipload kind + observed_mm_per_tooth, deflection kind + peak_mm, power kind + peak_kw, rapid_collision_count, average_engagement, peak_axial_doc_mm, drill_gates if applicable).

Output: `planning/toolpath_acceptance/baselines/2026-05-26.csv` (or .json — pick the format that diffs cleanly).

This is the verdict snapshot. Future PRs must produce identical verdicts (or improved verdicts with explicit justification).

### Part B — add wanaka to the regression matrix

The wanaka project is a real-world test case that uncovered:
- Setup 1 = `face_up=Bottom` non-identity (F-025 territory, no smoke case had this)
- "End Mill" with `corner_radius=2mm` (bull-nose mis-classified as flat endmill)
- Multi-setup workflow (front + back machining)
- All 7/7 deflection Within at last verification (round-10)

Add as smoke case `WANAKA` or as a separate regression file. Capture its baseline verdicts alongside AS001-AS018.

Concrete: copy a sanitized subset of `wanaka_full_tuned.toml` into `test_data/wanaka_subset.toml` (or use the existing project file directly if licensing allows). Add to `cases_agent_smoke.csv`:

```csv
WANAKA,multi_setup,multiple,test_data/wanaka_subset.toml,...
```

Baseline verdicts (from round-10):
- 7 cutting toolpaths, all deflection Within (0.005-0.114 mm peaks)
- 0 rapid collisions across 8 toolpaths
- 2 chipload Within (vendor LUT), 2 chipload Exceeds_LOW (deliberate user low-feed)
- 1 drill gate elevated (Pin Drill chip_welding 4.5/6.0)

### Part C — CI gate

Add a CI job that runs the smoke suite and diffs against the baseline. Fail on any verdict regression.

Mechanics:
1. `cargo run -p rs_cam_cli -- smoke --baseline planning/toolpath_acceptance/baselines/2026-05-26.csv` exits non-zero on regression
2. GitHub Actions workflow runs this nightly (smoke is slow; not every PR)
3. PRs touching `crates/rs_cam_core/src/{simulation,tool_load,compute,toolpath}/` OR `crates/rs_cam_viz/src/{compute,controller}/` trigger the smoke check pre-merge

The implementer contract update (from F-037's PR):

> "Any PR touching simulator-adjacent code (sim, tool_load, compute, toolpath, viz worker, viz controller) must run the full smoke suite locally and attach the results.csv to the PR description. Pass criterion: every existing case's verdict (chipload kind, deflection kind, rapid_collision_count) is unchanged or improved. Worse verdicts require justification + finding cross-link."

This makes the loop's calibration formally protected.

## Acceptance test

Three checks:

```rust
// tests/smoke_baseline_regression_f037.rs

#[test]
fn baseline_file_exists_and_parses() {
    let baseline = load_baseline("planning/toolpath_acceptance/baselines/2026-05-26.csv");
    assert!(!baseline.is_empty());
    // Verify schema: case_id, deflection_kind, deflection_peak_mm, chipload_kind, ..., rapid_collision_count
}

#[test]
fn wanaka_subset_loads_and_simulates() {
    // Load test_data/wanaka_subset.toml (or wherever wanaka lives in regression).
    // Generate + simulate.
    // Assert: 7 cutting toolpaths report deflection Within.
    // Assert: 0 rapid collisions across all toolpaths.
}

#[test]
fn smoke_diff_against_baseline_fires_on_regression() {
    // Synthetic test: run a known toolpath with the SAME params as the baseline.
    // Diff against baseline.
    // Assert: 0 regressions reported.
    // Mutate one verdict (e.g. flip deflection Within → Exceeds).
    // Re-diff.
    // Assert: 1 regression reported (the mutated case).
}
```

Plus the CI integration test runs as a separate workflow, not a cargo test.

## Files

- `planning/toolpath_acceptance/baselines/2026-05-26.csv` — new (the baseline snapshot)
- `test_data/wanaka_subset.toml` — new (or import-from-source if user OK with it)
- `planning/toolpath_acceptance/cases_agent_smoke.csv` — new row for WANAKA
- `crates/rs_cam_cli/src/commands/` — new `smoke` subcommand that diffs against baseline
- `.github/workflows/smoke.yml` — new CI workflow (or extend existing)
- `crates/rs_cam_core/tests/smoke_baseline_regression_f037.rs` — new
- `planning/acceptance_loop/implementer_contract.md` — update with the smoke-required rule

## Risk

M.

- **Risk**: capturing the baseline requires running the full smoke suite, which is slow (~10-30 min depending on setup). Mitigation: one-time cost; results.csv is checked in.
- **Risk**: wanaka project file licensing / privacy. Mitigation: check with user; possibly use a sanitized geometric subset.
- **Risk**: CI infrastructure setup. Mitigation: keep it nightly, not per-PR; only require pre-merge run for simulator-adjacent PRs.
- **Mitigating**: F-037 lands first in the workstream. Once it's in place, F-034/35/36 can land knowing if anything breaks the loop's calibration, the CI gate catches it.

## Notes

- **Lands first.** F-034/35/36 should not start before F-037 is in place. The risk of silent regression is too high.
- **The baseline is mutable.** When a finding LEGITIMATELY changes a verdict (e.g. F-031 took AS013 deflection 0.637 → 0.105), the baseline gets updated as part of that PR. Implementer contract should specify "baseline updates require a finding-file linkage explaining the verdict change."
- **Wanaka is a real-world reference.** Other user projects could join the regression net over time. F-037 just sets the pattern; the user can add more projects later.
- **CI gate is the LAST piece.** Parts A and B (baseline + wanaka in matrix) can land first. Part C (CI) can land in a follow-up PR if the CI infra needs separate work.
</parameter>
</invoke>