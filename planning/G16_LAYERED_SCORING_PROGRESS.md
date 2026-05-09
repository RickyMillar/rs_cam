# G16 Layered Scoring — Progress Tracker

**Plan:** `planning/OPTIMIZER_REFACTOR_G16.md` §11.
**Started:** 2026-05-10.
**Status:** Layer 2a — landed (composite_score module + α/β/γ + power_warning_fraction; no call sites yet). Next: 2b rewire ranking.

This doc is the execution checklist for §11 of the G16 design doc.
Survives context compaction. **Update after every commit.**

---

## Snapshot

| Phase | Status | Hash | Date |
|---|---|---|---|
| 1. Tolerance bands | ✅ done (deflection wrap deferred — note §1) | `53cb252` | 2026-05-10 |
| 2a. composite_score additive | ✅ done (callable, no call sites) | _pending_ | 2026-05-10 |
| 2b. Rewire ranking to composite | ⏳ pending | — | — |
| 2c. Calibrate α/β/γ vs wanaka + 3 fixtures | ⏳ pending | — | — |
| 3. MarginalSafe outcome tier | ⏳ pending | — | — |
| 4. Adaptive3d LUT-family routing | ⏳ pending | — | — |

Legend: ⏳ pending • 🟡 in-progress • ✅ done • 🚫 blocked • ⏭️ skipped

**Hard gates per commit** (CLAUDE.md + design doc §5):
- `cargo build --workspace --all-targets` clean
- `cargo test -p rs_cam_core --lib` ≥ 1293 pass
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `wanaka_tp4_burnrisk_emits_feed_up_candidate` passes
- Wanaka MCP snapshot diff in commit message body

---

## Commit-level plan

### 1. Tolerance bands on hard gates

**Reference:** §11.4 "Layer 1", §11.6.2 risk.
**Effort:** ~1h. **Files:** 4. **LOC:** ~30 + tests.

- [x] Add `breakage_tolerance`, `burn_tolerance`, `power_breach_tolerance`,
      `deflection_breach_tolerance` to `optimize/policy.rs::RankingPolicy`
      (or sibling). PolicyValue defaults: 0.05 / 0.05 / 0.0 / 0.0.
      `PolicySource::TuningChoice` with hypothesis citing wanaka TP4 May 2026.
- [x] Wrap `tool_load/chipload.rs:344` (`if cl > max`) with breakage_tolerance.
- [x] Wrap `tool_load/chipload.rs:367+` (`median_cl < min_value`) with burn_tolerance.
- [x] Wrap `tool_load/power.rs:153` (`peak_power > peak_available_at_peak`).
- [ ] **Deferred:** wrap `tool_load/deflection.rs:158`
      (`peak_delta_mm > EXCEEDS_BOUND_MM`). Operator had unstaged WIP in
      `deflection.rs` (`sample_tip_deflection_mm` + pub constants); to keep
      Layer 1 free of WIP entanglement we left the deflection gate strict
      and `ToleranceBands` exposes only 3 fields (no `deflection_breach`).
      `RankingPolicy::deflection_breach_tolerance` stays as a reserved field
      so the follow-on is a one-line wrap once the WIP lands.
- [x] Plumb `&ToleranceBands` (not `&SearchPolicy` — kept the optimize-policy
      types out of `tool_load`) through chipload + power evaluators and
      `evaluate_toolpath` / `evaluate_project`. New helper
      `optimize::tolerance_bands_from_policy` builds the band from
      `RankingPolicy`.
- [x] Tests: 4 chipload tests at the four band edges + 1 power sanity.
      6 new tests pass; 1298 lib tests total.
- [ ] **Pending:** Wanaka MCP smoke. MCP not connected during commit; re-run
      `optimize_toolpath` on TP 1 + TP 6 next session. Expected: candidates
      that were `NoSafeImprovement` now land `Within` → `Ranked`.

**Compact prompt:** see end of this doc § "Compact prompt — Layer 1".

### 2a. composite_score (additive)

**Reference:** §11.3, §11.7 "Layer 2a".
**Effort:** ~½d. **Files:** 1 new. **LOC:** ~150.

- [x] New `crates/rs_cam_core/src/tool_load/optimize/rank.rs`. Defines
      `composite_score`, `chipload_distance_penalty`,
      `power_overuse_penalty`, `deflection_overuse_penalty`. Code shape
      per §11.3. Module carries `#![allow(dead_code)]` for Phase 2a; the
      allow comes off at 2b when call sites land.
- [x] Added `alpha_chipload_distance` (5.0), `beta_power_overuse` (3.0),
      `gamma_deflection_overuse` (2.0), `power_warning_fraction` (0.80)
      to `RankingPolicy`. PolicyValue + TuningChoice provenance citing
      §11.3 calibration plan and CADEM S1/S6 motor convention.
- [x] No call sites yet — module is callable but unused (verified by
      `cargo build -p rs_cam_core --lib` warnings on the 4 helpers, all
      now silenced by the file-level `#![allow(dead_code)]`).
- [x] Tests (4): all pass. 1302 total lib tests (vs 1298 baseline).
      `composite_score_prefers_midpoint_when_cycle_times_equal` checks
      mid_score=20.0 vs edge_score=15.0 (= 20 - 1.0×α at the bound).
      `composite_score_prefers_faster_when_chipload_equal` confirms the
      cycle-time term still dominates with equal penalties.
      `power_penalty_zero_at_below_warning_threshold` walks 50/80/90/100%
      points (0, 0, 0.5, 1.0). `deflection_penalty_ramps_in_band` walks
      30/50/125/200/250 µm (0, 0, 0.5, 1, 1).

### 2b. Rewire ranking to composite_score

**Reference:** §11.4 "Layer 2", §11.7 "Layer 2b".
**Effort:** ~½d. **Files:** 3. **LOC:** signature + 1 call site + ~3 fixtures.

- [ ] Change `select_stage2_candidates` signature in
      `optimize/candidate.rs:391` to `(stage1_winners, baseline, policy, n)`.
- [ ] Update single call site at `optimize/mod.rs:306`.
- [ ] Replace `cycle_time_s.total_cmp` sort in `optimize/outcome.rs:164`
      (inside `build_outcome`) with composite-score comparator.
- [ ] Update `select_stage2_candidates` tests at
      `optimize/mod.rs:1613-1624` for new signature.
- [ ] Re-baseline param-sweep snapshots (`crates/rs_cam_core/tests/param_sweep.rs`,
      54 sweeps). Document diff distribution in commit message — how many sweeps
      reordered, how many unchanged.
- [ ] Wanaka MCP smoke: candidate ordering may shift within Ranked. Review by hand once.

### 2c. Calibrate α/β/γ

**Reference:** §11.6.1 mitigation.
**Effort:** ~½d. **Files:** policy.rs only. **LOC:** literal-tweaks.

- [ ] Run wanaka MCP optimize on TPs 1, 4, 5, 6, 7. Capture per-candidate
      score breakdowns (cycle_savings, chipload_pen, power_pen, defl_pen).
- [ ] Run on 3 fixture projects (TBD — pick from `projects/` or stress test).
- [ ] If any default obviously misbehaves (e.g. score collapses to cycle-time
      only, OR optimizer becomes too cautious to surface speedups), retune.
- [ ] Commit message records: defaults chosen, before/after candidate ordering
      per project, score-breakdown table.
- [ ] No code change beyond literal values in `policy.rs::RankingPolicy::default()`.

### 3. MarginalSafe outcome tier

**Reference:** §11.4 "Layer 3", §11.7 "Layer 3".
**Effort:** ~½d. **Files:** ~12. **LOC:** 1 enum variant + match arms.

- [ ] Add `MarginalSafe { recommended: Vec<OptimizeCandidate>, explanation: String }`
      to `optimize/outcome.rs::OptimizeOutcome`. Variant order: `Ranked` →
      `MarginalSafe` → `TradeOff` → `NoSafeImprovement` → `Skipped`.
- [ ] `first_safe` (outcome.rs:74-83): keep strict (Ranked-only). Add
      `first_marginal_safe(&self) -> Option<&OptimizeCandidate>`.
- [ ] `first_safe_index` (outcome.rs:110-121): same — strict, parallel `first_marginal_safe_index`.
- [ ] `build_outcome` (outcome.rs:138): new tier-dispatch branch for
      tolerance-band-admitted candidates that exceed strict LUT.
- [ ] UI match arms in `crates/rs_cam_viz/src/ui/optimize_modal.rs:101-139`
      (yellow caution stripe, "verify on a scrap" copy).
- [ ] UI match arms at `optimize_project.rs` lines 198-212, 241, 273-330,
      533-615 (six sites).
- [ ] `controller/events/mod.rs:336, 485` and `events/compute.rs:712`:
      decide auto-Apply per site (recommend: no, require explicit click).
- [ ] `compute/worker.rs:838-841`: pass-through arm.
- [ ] MCP descriptions: `rs_cam_mcp/src/server.rs:715`,
      `rs_cam_viz/src/mcp_server.rs:493`, `rs_cam_viz/src/mcp_bridge.rs:41` —
      add `MarginalSafe` to the documented variant list.
- [ ] Tests: `build_outcome_emits_marginal_safe_when_inside_tolerance_band`,
      `marginal_safe_does_not_auto_apply_in_first_safe`.

### 4. Adaptive3d LUT-family routing

**Reference:** Doc §10 sign-off ("single-rule addition to bounds.rs's row-matching policy").
**Effort:** ~½d. **Files:** 1 — likely `tool_load/chipload.rs::routed_lookup_family` or `bounds.rs`. **LOC:** small.

- [ ] Investigate: where does Adaptive3d currently route? `routed_lookup_family`
      in `tool_load/chipload.rs` is the prime suspect.
- [ ] Identify the single rule needed. Probably "Adaptive3d → LutOperationFamily::Adaptive"
      with a stricter clamp on radial engagement, or a fall-through to Parallel
      when no Adaptive row matches the diameter.
- [ ] Add the rule + test for the routing.
- [ ] Wanaka MCP smoke: TP indices 1, 6 (Adaptive3d ops) should now match
      better LUT rows.

---

## Compact prompt — Layer 1

Paste verbatim into a fresh session to start phase 1. Self-contained.

```
G16-L1: tolerance bands on hard gates. Plan in planning/OPTIMIZER_REFACTOR_G16.md §11
(specifically §11.4 "Layer 1 — tolerance bands" and §11.6.2 risk).
Tracker at planning/G16_LAYERED_SCORING_PROGRESS.md — update it when done.

CONTEXT
The post-G16 wanaka MCP test (May 2026) showed every roughing toolpath returns
`NoSafeImprovement` despite candidates 50% faster than baseline. Root cause:
chipload high-side gate at crates/rs_cam_core/src/tool_load/chipload.rs:344
fires on ANY single sample over chip_load_max_mm. One transient at 0.0707 vs
max 0.07 (1.05% over) marks the candidate `Exceeds(High)`. The low-side gate
(line 367+) uses median-stat and is robust. Asymmetric in the wrong direction.

DO
1. Add four PolicyValue<f64> fields to optimize/policy.rs (find existing
   RankingPolicy or sibling; mirror the PolicyValue<T> rationale-string
   convention used elsewhere). Default values per §11.3:
   - breakage_tolerance: 0.05
   - burn_tolerance: 0.05
   - power_breach_tolerance: 0.0
   - deflection_breach_tolerance: 0.0
   PolicySource: TuningChoice with hypothesis "calibrated against wanaka
   transient TP4 5/2026; vendor charts publish ≥20% material variability,
   CNCCookbook tool-life-cliff data shows 40% loss at 50% allowance".

2. Wrap four gate-trigger conditions:
   - chipload.rs:344  `if cl > max` -> `if cl > max * (1.0 + breakage_tolerance.value)`
   - chipload.rs:367+ `median_cl < min_value` -> `median_cl < min_value * (1.0 - burn_tolerance.value)`
   - power.rs:153     `peak_power > peak_available_at_peak` -> wrap with power_breach_tolerance
   - deflection.rs:158 `peak_delta_mm > EXCEEDS_BOUND_MM` -> wrap with deflection_breach_tolerance
   Each gate evaluator needs access to SearchPolicy. Check if already plumbed;
   if not, add a `&SearchPolicy` parameter through the evaluate signature
   (caller in optimize/candidate.rs::evaluate_candidate has it).

3. Add tests close to each gate change:
   - chipload_high_just_above_max_within_tolerance_is_within (cl = 1.04 × max)
   - chipload_high_above_tolerance_is_exceeds (cl = 1.06 × max)
   - chipload_low_just_below_min_within_tolerance_is_within (median = 0.96 × min)
   - chipload_low_below_tolerance_is_exceeds (median = 0.94 × min)
   Power and deflection get one each (defaults are 0, so just sanity that
   tolerance > 0 widens the gate).

HARD RULES
- One commit. cargo build --workspace --all-targets clean. cargo test -p
  rs_cam_core --lib all pass. cargo clippy --workspace --all-targets -- -D
  warnings clean.
- Wanaka regression must pass: cargo test -p rs_cam_core --lib
  wanaka_tp4_burnrisk_emits_feed_up_candidate
- Operator has unstaged WIP in rs_cam_viz - DO NOT touch:
  crates/rs_cam_viz/src/compute/worker.rs, worker/execute/mod.rs,
  worker/tests.rs, controller/events/simulation.rs. 3 viz lib tests fail on
  master because of this WIP - pre-existing, ignore.
- git add per file. Never git add -A.
- Stage cargo fmt -p rs_cam_core, then git checkout -- any unrelated files
  fmt drifted (workspace has pre-existing fmt churn).

VERIFY (commit message body)
After committing, run wanaka MCP smoke check via the rs-cam MCP if connected,
or note in commit msg "MCP smoke deferred". Capture before/after for
optimize_toolpath on wanaka TP indices 1 (Back Rough) and 6 (3D Rough 6).
Expected: NoSafeImprovement on master -> Ranked or attempted-set with Within
candidates on this commit.

UPDATE TRACKER
After committing: edit planning/G16_LAYERED_SCORING_PROGRESS.md, mark phase 1
status ✅ done, fill in commit hash and date. Check the per-task boxes.

Once green, post the diff stat and commit hash.
```

---

## Notes on multi-compact workflow

This work spans multiple compaction boundaries. Workflow:

1. Each session starts by reading this tracker + §11 of the design doc.
2. Take the next pending phase. Use its compact prompt (or write one matching the Layer 1 template).
3. Implement, commit, run hard gates.
4. Update this tracker — status, hash, date, checkboxes.
5. Compact and continue with next phase.

If a phase reveals the design needs adjustment, edit §11 of the design doc
*and* note the change in this tracker's "Notes" section below. Don't let the
design doc drift from what was actually built.

## Notes / deviations log

(Append entries here as phases land. One line per deviation from the plan,
with date and reasoning.)

- **2026-05-10 — Phase 1.** Plumbed `&ToleranceBands` (a new tool_load-level
  struct) instead of `&SearchPolicy` through gate evaluators, to keep
  `tool_load::chipload`/`tool_load::power` decoupled from the
  `optimize::policy` types. Conversion lives in
  `optimize::tolerance_bands_from_policy`. Same observable behaviour.
- **2026-05-10 — Phase 1.** Deflection gate wrap held back; operator had
  unstaged WIP in `deflection.rs` (`sample_tip_deflection_mm` +
  `pub`-promotion of `WITHIN_BOUND_MM` / `EXCEEDS_BOUND_MM`). The
  policy field `deflection_breach_tolerance` remains as a reservation;
  `ToleranceBands` exposes only `breakage` / `burn` / `power_breach`. Wire
  the deflection gate after the operator's WIP lands.
- **2026-05-10 — Phase 1.** Wanaka MCP smoke deferred. MCP wasn't
  connected during the commit. Re-run `optimize_toolpath` on TP indices
  1 + 6 in the next session and append a note here with verdict / cycle-
  time deltas.
- **2026-05-10 — Phase 2a.** Module ships with file-level
  `#![allow(dead_code)]`. The 4 `pub(crate)` helpers are only consumed
  by the in-module test suite; without the allow, `cargo clippy
  --workspace --all-targets -- -D warnings` flags them as dead at the
  lib level (the test compilation isn't enough to satisfy lib dead-code
  analysis). Phase 2b removes the allow when `build_outcome` and
  `select_stage2_candidates` start consuming `composite_score`.
- **2026-05-10 — Phase 2a.** Operator's WIP in `rs_cam_viz` (3 files
  modified, viewport-overlay automation harness wiring + adaptive3d
  semantic-trace test cleanup) was untouched. All `git add` per file;
  no global fmt run; pre-existing workspace fmt drift in
  `verdict.rs`/`bounds.rs`/`patches.rs`/etc. left intact for whoever
  picks them up. Only `rank.rs` was rustfmt'd (single trailing-newline
  fix).

