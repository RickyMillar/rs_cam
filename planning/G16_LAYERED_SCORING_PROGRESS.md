# G16 Layered Scoring — Progress Tracker

**Plan:** `planning/OPTIMIZER_REFACTOR_G16.md` §11.
**Started:** 2026-05-10.
**Status:** Phase 4 (Adaptive3d → Pocket LUT routing) landed. Remaining: 2c calibration (MCP-blocked), 3 MarginalSafe tier.

This doc is the execution checklist for §11 of the G16 design doc.
Survives context compaction. **Update after every commit.**

---

## Snapshot

| Phase | Status | Hash | Date |
|---|---|---|---|
| 1. Tolerance bands | ✅ done (deflection wrap closed in follow-up) | `53cb252` + `00e889b` | 2026-05-10 |
| 2a. composite_score additive | ✅ done (callable, no call sites) | `f873440` | 2026-05-10 |
| 2b. Rewire ranking to composite | ✅ done | `b4dc8df` | 2026-05-10 |
| 2c. Calibrate α/β/γ vs wanaka + 3 fixtures | 🚫 blocked (needs MCP) | — | — |
| 3. MarginalSafe outcome tier | ⏳ pending | — | — |
| 4. Adaptive3d LUT-family routing | ✅ done | `3c937e5` | 2026-05-10 |

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
- [x] Wrapped `tool_load/deflection.rs:185`
      (`peak_delta_mm > EXCEEDS_BOUND_MM * (1.0 + tolerance.deflection_breach)`)
      after the operator's WIP landed. Threaded `tolerance: &ToleranceBands`
      into `deflection::evaluate` (8 call-site updates: 1 in
      `tool_load/mod.rs`, 1 in `gcode.rs`, 7 in deflection's own tests
      passing `&ToleranceBands::default()`). Added `pub deflection_breach: f64`
      field to `ToleranceBands` and removed the "intentionally not plumbed"
      doc note. Updated `tolerance_bands_from_policy` to populate the field.
      Sanity test `deflection_breach_tolerance_widens_exceeds_trigger`
      verifies a strict-Within result stays Within when widened, and that
      the trigger never flips Within → Exceeds in the wrong direction.
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

- [x] Changed `select_stage2_candidates` signature in
      `optimize/candidate.rs:397` to `(stage1_winners, baseline, policy, n)`.
      Sorts by `composite_score` descending (highest-score = best).
- [x] Updated single runtime call site at `optimize/mod.rs:329` (was 326
      in tracker — drifted by Stage 2 comment expansion).
- [x] Replaced `cycle_time_s.total_cmp` sort in `optimize/outcome.rs:164`
      (inside `build_outcome`) with composite-score comparator. Borrows
      `&baseline` for the comparator key, uses `search_policy()` directly.
- [x] Renamed `select_stage2_keeps_top_n_by_cycle_time` →
      `select_stage2_keeps_top_n_by_composite_score` and threaded baseline
      + policy through all three test call sites at `optimize/mod.rs`.
      Added `select_stage2_prefers_midpoint_over_band_edge_at_close_cycle_time`
      to demonstrate the composite-score reorder vs pure cycle time
      (76s @ chipload max scores 119; 80s @ midpoint scores 120).
- [x] Param sweeps untouched: 54/54 pass. The fingerprint harness
      doesn't touch `tool_load/optimize`, so candidate ordering changes
      can't shift toolpath fingerprints. **No re-baseline needed.** The
      tracker's "re-baseline" task was over-anticipated impact.
- [x] `#![allow(dead_code)]` removed from `rank.rs` since the helpers
      are now consumed.
- [ ] **Pending:** Wanaka MCP smoke. Defer to 2c (which already needs
      MCP for the calibration sweep).

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

- [x] Located routing dispatcher: `routed_lookup_family` at
      `tool_load/chipload.rs:462`. Tracker / design doc said "single-rule
      addition to bounds.rs's row-matching policy", but bounds.rs only
      consumes already-matched `MatchedRow`s — actual routing lives in
      chipload.rs (which calls into vendor_lookup criteria). Doc text was
      sloppy about location; the rule itself is one branch.
- [x] Single rule: when `operation_kind == OperationType::Adaptive3d`
      AND incoming `operation_family == LutOperationFamily::Adaptive`,
      override to `LutOperationFamily::Pocket` (pass_role unchanged).
      Confirmed against the embedded LUT data
      (`data/vendor_lut/observations/amana_flat_end.json`): for a 6mm
      flat in hardwood the Adaptive rows publish ae_max ≈ 0.95–1.2mm
      while the Pocket rows publish ae_max ≈ 2.2–2.5mm, matching the
      operator-wanted regime in design doc §1.3.
- [x] 4 tests added in `chipload.rs` tests module:
      `adaptive3d_reroutes_from_adaptive_to_pocket_family`,
      `adaptive3d_reroute_preserves_pass_role`,
      `adaptive3d_with_non_adaptive_family_passes_through` (defensive),
      `pocket_op_with_adaptive_family_passes_through` (gate by op kind).
- [x] Updated both doc-comments: `routed_lookup_family` now lists the
      two rerouted ops (ProjectCurve + Adaptive3d); `evaluate`'s upstream
      doc-block mirrors the same.
- [x] 1308 lib tests pass (1304 baseline + 4 new). 54/54 param sweeps
      green. Wanaka burn-risk regression green. Clippy clean.
- [x] Param sweeps untouched: the routing change affects LUT row
      selection in the chipload guardrail / F&S calculator, not toolpath
      generation. Sweep fingerprints don't pass through the optimize
      lookup path. **No re-baseline needed.**
- [ ] **Pending:** Wanaka MCP smoke. MCP not connected during commit;
      re-run `optimize_toolpath` on TP indices 1 + 6 next session and
      append delta here. Expected: Adaptive3d candidates match Pocket
      LUT rows with wider ae bands → Stage 2 surfaces 2–3mm stepover
      candidates that were unreachable under Adaptive's 0.95mm cap.

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
- **2026-05-10 — Phase 2b.** Existing test fixture
  `within_chipload_verdict(peak)` sets `approach_to_min: None`, which
  forces `chipload_distance_penalty` into the synthetic-midpoint
  fallback (`max * 0.5`). For the new
  `select_stage2_prefers_midpoint_over_band_edge_at_close_cycle_time`
  test, that fallback skews the midpoint to 0.0525 (not 0.054) and the
  arithmetic stops working. The test inlines a chipload verdict with
  both `approach_to_min` and `approach_to_max` populated so the LUT
  bracket midpoint is genuine. The shared fixture wasn't changed —
  other tests rely on the no-min-bound behaviour.
- **2026-05-10 — Phase 4.** Doc §10 says "single-rule addition to
  bounds.rs's row-matching policy", but bounds.rs is the row-resolver
  (consumes already-matched `MatchedRow`s) — the actual routing is in
  `chipload.rs::routed_lookup_family`. Implemented the rule there. The
  doc text in §10 should be updated when convenient; not done in this
  commit to keep the patch minimal.

