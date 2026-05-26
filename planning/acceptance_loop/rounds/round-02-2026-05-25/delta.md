# Round-02 delta — 2026-05-25

**Vs baseline:** `round-01-2026-05-24/baseline.md`
**Smoke run:** `target/acceptance_sweeps/agent_smoke_20260525_0957/`
**Auditor:** autonomous Claude session
**Git HEAD at smoke time:** `2a287c1` (the running MCP binary)
**Git HEAD after smoke:** `ba3f84d` (F-015 landed during this round; **not in the running MCP binary** — verification deferred to round-03 after rebuild)

## Headline numbers

- Cases attempted: 5 (focused subset targeting landed-batch axes)
- Cases generated: 5
- Cases simulated: 5
- Optimizer runs: 1 (AS015)
- Pass: 1 (AS011 drill — same as round-01)
- Warn: 2 (AS001 pocket, AS002 adaptive — both moved up from `fail`)
- Fail: 2 (AS013 adaptive3d, AS015 scallop — chipload/deflection axes unchanged; AS015 is correct-refusal)

## Verdict by finding

### Verified — closing this round

| Finding | Verified via | Evidence |
|---|---|---|
| **F-001** chipload 2D feedopt probe | AS001, AS002 | Both cases moved from `chipload: Unmodeled (steady_state_samples_not_present)` to `chipload: Exceeds_LOW` with `validated` confidence and real vendor-LUT comparison (AS001: 0.0058 vs 0.032–0.055; AS002: 0.0072 vs 0.038–0.07). |
| **F-016** drill chip_welding material-aware | AS011 | `chip_welding` threshold moved from `8.0` (hardcoded softwood) to `6.0` on hardwood stock. `drill_summaries.chip_welding_risk: "low"`, `peck_pattern_adequate: true`. |
| **F-003** vendor LUT singleton collapse | AS001/AS002/AS013 | No regression in vendor LUT lookups across 3 op kinds; all returned validated bounds. Test-level verified pre-round. |
| **F-007** DrillConfig::set_plunge_rate | AS011 | Drill cycle generated with non-default feed_rate (250). Note: schema no longer exposes a separate `plunge_rate` param — the unified `feed_rate` covers both. Test-level verified pre-round. |
| **F-008** compute_stale_set single authority | (no direct smoke signal) | Test-level verified pre-round; mutation envelope `stale_toolpaths` field present and accurate across all add/set_param calls. |
| **F-013** feeds-result invariants | AS002, AS013 | Diagnostic delta `feeds.feed_vs_lut.high` fired consistently (2.7× recommendation on both, identical math). Test-level verified pre-round. |

### Partially verified — needs follow-up

| Finding | Status | Evidence |
|---|---|---|
| **F-002** axial-DOC / plunge-descent split | ~~partial — REOPEN~~ — **see correction below** | Original audit conclusion: `per_kinematics.linear.peak_axial_doc_mm` reads 12.0 while arc/helix read 0.0, therefore linear path stale. **This was wrong.** See "Round-03 correction" below. |

### Round-03 correction (2026-05-25, after F-002 implementer's investigation)

The "linear class still over-reads" conclusion in round-02 was a
misread of the smoke evidence. Per the F-002 implementer's probe
(reproduction of AS001 via `ProjectSession`):

- Per-sample `axial_engagement_mm = 12.0` on **all four kinematics
  classes** (linear/helix/arc/plunge) for a 2mm-DOC pocket pass.
- Summary asymmetry (linear 12.0, arc/helix 0.0) is an artefact of
  `KinematicsAccumulator::observe`'s `if !sample.in_transit_span`
  filter. Arc/helix samples in pocket entry are transit-tagged;
  linear clearing cuts are not.

**F-002's original split landed correctly.** The deflection over-fire
has a different root cause — a Z-frame mismatch in the dexel stock
grid for identity setups. Tracked as **F-024**.

This was a healthy false positive: the implementer refused to land
the band-aid the auditor framed, reproduced the bug end-to-end, and
returned with file-and-line evidence of the real root cause. The
acceptance loop did its job at the framing layer.

### Landed-but-unverified (binary mismatch — resolved late-round-02)

| Finding | Resolution |
|---|---|
| **F-015** op-precondition static validation | Committed at `ba3f84d`. User rebuilt MCP late-round-02. Smoke-verified via two probes: (a) `add_toolpath(rest)` without prior tool now emits blocking `precondition.rest_prev_tool_missing` in `diagnostic_delta` + `gui_banners` + `warnings`; (b) `add_toolpath(project_curve, model_id=1)` against the mesh-only terrain model now emits blocking `precondition.project_curve_no_curve` with an actionable fix message. Drill precondition path covered by F-015's existing integration tests. **VERIFIED.** |

### New findings opened this round

| Finding | Source | Severity |
|---|---|---|
| **F-023** MCP/GUI diagnostic surface asymmetry: "Selected model missing" | User-flagged during AS001; reproduced repeatedly | high |

### Unrelated improvements observed (not from this batch)

- **AS013 rapid collisions: 1041 → 844 (-19%).** Almost certainly from the pre-batch `2e2c5db fix(dressup): pocket lift-bridge — ramp/helix entry no longer rapids through stock`. Worth noting for the F-017 (rapid collisions everywhere) finding.
- **AS001 air-cut: 82% → 77%** and **AS002 air-cut: 75% → 58%.** Likely the Step 1 retract-tagging fix; calibration is firming up.

### Acceptance test contradictions

None. All landed findings either passed their predicted axis or showed the partial-fix pattern noted above for F-002.

### Optimizer gold-standard

**AS015 — unchanged, preserved as designed.**
- `kind: no_safe_improvement`
- `reason.kind: deflection_setup_locked`
- `narrative.explanation` identical to round-01 verbatim
- `peak_mm: 0.4313752` (vs round-01's 0.4313752 reading — identical)

This pins the optimizer's refusal correctness at the byte level across the unification batch.

## Acceptance-bar snapshot

| Bar | Target | Round-01 | Round-02 | Delta |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | not measured | not measured | — |
| Sim chipload calibration (3D ops) | ≥ 95% | 3/3 fired | 1/1 fired (AS013) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 0/7 (all Unmodeled) | **2/2 fired** (AS001/AS002 sample) | **F-001 landed** |
| Sim deflection calibration | ≥ 95% | 4/13 Within | unchanged at sampled cases | **F-002 partial — REOPEN** |
| Optimizer honest-improvement | ≥ 95% | 2/2 | 1/1 (AS015) | stable |
| Optimizer refusal correctness | 100% | 2/2 | 1/1 (AS015) | stable |
| Export gate | 100% | not tested | not tested | — |

## Side observations worth recording

- `get_cut_trace` with `toolpath_id` filter returns empty `drill_summaries` even though they exist when no filter is passed — the filter appears to match against the internal `toolpath_id` (3 in AS011) rather than the documented `toolpath_index` (0). Minor; record under F-023's "discoverability gap" sibling-issues if it surfaces again.
- `add_toolpath` schema docstring `"Model ID … usually 0 for the first model"` is wrong in every project I tested — first model has `id: 1`. Tagged in F-023.
- `stock_top_z` is no longer a valid param on adaptive3d (round-01 set it). Schema evolution; not a regression.
- `run_simulation` does surface `runtime_errors` for missing-model toolpaths; `load_project` does too via warnings. So the data is available — F-023 is purely about routing it into the unified diagnostic stream.

## Recommended next-round actions

1. **User: rebuild rs_cam_viz** so F-015's op-precondition validation lands in the running MCP. Then round-03 smoke can verify AS006 (rest), AS011 (drill precondition), AS018 (project_curve).
2. **Open F-024 (or expand F-002): linear-kinematics peak_axial_doc_mm still reports stock-above-cutter, not commanded DOC.** This is the load-bearing remainder of F-002 — deflection cannot become a useful gate until linear kinematics joins arc/helix on the engagement-vector measurement.
3. **F-023 next pickup** if user wants — small adapter add (`from_runtime_errors.rs` sibling of `from_preconditions.rs`).
4. **F-018 (test_data templates)** can fan out in parallel with F-023 — different files, no collision.
