# Optimizer redesign — 2026-05-08

Self-contained plan capturing the audit, the policy redesign, the implementation survey, and the proposed commit sequence. Written so this work can be paused, context compacted, and resumed without losing the thread.

## Why this exists

The wanaka relief-map project exposed that the toolpath optimizer returns `NoImprovementFound` against unsafe baselines while a clearly-faster, no-worse-than-baseline candidate sits in the evaluated set. Operator's words: *"the optimize button has given me no useful results."* Audit confirmed the optimizer's search-space cleanly covers only one of three gates (power) — chipload-Burn baselines and any L/D > 6 tool are unreachable by design.

Companion docs: `planning/wanaka_audit_2026-05-08.md` (operator-facing findings on the wanaka project), `planning/OPTIMIZER_LOGIC.md` (older spec, drifted from code — keep for historical context only, **do not implement against**), `research/feeds_and_speeds_integration_plan.md` §RCTF (the chipload-thinning math that grounds Stage F).

## What the optimizer is actually for

Given the operator is going to run *something*, propose a small set of alternatives ranked by improvement, where "improvement" is **defined per-gate relative to baseline** rather than absolutely. The optimizer is not a safety officer — the gates are. The optimizer's job is to navigate among candidate parameter sets given that the gates exist.

Three things follow:

1. **The recommendation surface must acknowledge baseline state.** "No improvement found" against an unsafe baseline is a category error.
2. **Search-space generation must match the gate that needs moving.** DOC×stepover sweeps when chipload is the failing gate are noise.
3. **A refusal must point at a knob the user has access to.** If no in-search-space knob can move a failing gate, surface that explicitly with a setup-level prescription.

## Audit summary — what's wrong today

Severity-ranked. Each links to the canonical code site. (Full audit lives in conversation history dated 2026-05-08, condensed here.)

### HIGH

1. **Burn-risk baselines are unfixable.** `optimize.rs:357` skips Stage 0 when chipload Exceeds; `solve_headroom_scale` (`optimize.rs:757`) floors at `k≥1.0`. Neither downshift nor upshift across an Exceeds-Burn baseline is reachable. The only knobs that move chipload are feed and RPM; Stage 1's DOC+stepover can't.
2. **`first_safe` policy is incoherent against an unsafe baseline.** `optimize.rs:182-186` requires `Within` on all 3 gates regardless of baseline status. When baseline already trips ⚠ chipload + ⚠ deflection, no candidate that retains the warnings — even one 116 s faster — can be recommended.
3. **Bipolar detection is dead code.** `RefuseReason::BipolarEngagement` exists (`tool_load/mod.rs:69`) but no caller emits it. The chipload gate computes per-side peaks (`chipload.rs:211-293`) but at line 278 *prioritizes* breakage — the burn signal is silently discarded when both fire.
4. **Stage 1 runs even when its knobs cannot affect the failing gate.** `optimize.rs:395` gate is `has_doc_knob(...)`, no chipload check. Wastes 3 full sims on candidates that can't fix chipload, every one rejected by `first_safe`.
5. **Deflection has no Stage-0 path.** `deflection.rs:42` — `ratio = stickout / diameter`. Both inputs are tool-config, not in the optimizer's search space. Anything with L/D > 6 (every wanaka tool) automatically fails `candidate_is_safe` regardless of feeds/DOC.

### MEDIUM

6. Slotting filter (`stepover > 0.85 × D`) the auto-feeds calculator enforces in `feeds/mod.rs:262` is missing from `build_stepover_variants`.
7. `apply_scale_to_op` mutates `feed_rate` and `spindle_rpm` but not plunge, despite `feeds/mod.rs:357-365` deriving plunge from feed.
8. Variant builders don't know about tapered-ball engaged-diameter geometry; `vendor_normalize.rs::engaged_diameter_at_depth` exists but isn't consulted.

### Latent bugs (predate redesign, fix first)

9. **`feeds_auto.feed_rate` flag isn't cleared when optimizer mutates feed.** `apply_scale_to_op` writes `feed_rate` but leaves the auto-feeds flag set; next regen re-derives feed from the LUT, **overwriting the candidate's value**. May explain why Stage 0 candidates report tiny improvements — they may be evaluating baseline-with-noise. Stage F re-target would hit this much harder.
10. **LUT lookup uses nominal tool diameter for tapered balls.** `find_matched_lut_row:544` calls `tool.diameter()` (= shank) instead of `tool.lookup_diameter_at(commanded_doc)`. Affects every wanaka river/lake/finish op.

## The redesign — policy

### Pre-flight classification (replaces today's "skip Stage 0 if chipload exceeds")

Before any candidate generation, classify the baseline trace:

| State | Detection | Outcome |
|---|---|---|
| Clean | All 3 gates `Within` | Headroom scale-up |
| Single-gate-tunable | One gate `Exceeds`, axis is in-search-space | Run that gate's stage + variance sweep |
| Bipolar chipload | `peak_below < cl_min` AND `peak_above > cl_max` (both already computed in `chipload.rs`) | Refuse with engagement-variance prescription, op-aware |
| Setup-locked | Failing gate is deflection, or bipolar on a no-DOC-knob op | Refuse with setup-level prescription (stickout, tool change, op switch) |
| Multi-gate | ≥2 gates `Exceeds` | Run each tunable-axis stage; present as Pareto trade-offs |

### Three stages, each owning an axis

- **Stage F (feed/RPM)** — the only stage that can move chipload. Two modes: *headroom-up* on Within baselines, *re-target* on Burn/Breakage baselines (RCTF-compensated, plunge tracks feed when delta > 10%, bounded by machine + LUT row brackets).
- **Stage E (engagement variance)** — DOC × stepover grid. Owns power and effective-chipload variance reduction. Slotting filter applied. Anchored on Stage-F's corrected feed/RPM.
- **Stage S (survivor refinement)** — top-N from Stage E re-sim'd at full resolution. Today's Stage 2.

Stages compose by pre-flight state: Within → headroom F → E → S; chipload-Burn → re-target F → E → S; deflection-only → refusal.

### Per-candidate gate-relative scoring

```
GateDelta = Improved | Same | Worsened | Crossed { from, to }
```

`Crossed` captures `Within→Exceeds` or `Exceeds→Within`. Replaces `first_safe`'s absolute predicate. `first_safe` stays for callers that want strict envelope (CLI sweeps); recommendation tier uses the relative one.

### Tiered recommendation (`build_outcome` becomes a dispatcher)

| Tier | Trigger | UI message |
|---|---|---|
| Improvement | ≥1 candidate `Same`-or-`Improved` on all 3 gates AND faster | Top 3 by cycle time. Today's "ranked" surface. |
| Trade-off | `Improved` on failing gate, `Worsened` on a non-failing one | Surface as "trade-off candidates", explicitly labeled. Operator sees what they're trading. |
| Setup prescription | Failing axis unreachable from feeds/RPM/DOC/stepover (deflection L/D, bipolar on no-knob op, no LUT row) | Refusal with typed lever name ("shorten stickout to ~24 mm for L/D=4", "reduce engagement variance via stepover", "no vendor data — measure or accept warning"). |
| No improvement | Baseline clean *and* all candidates slower | Honest "you're at the productivity ceiling". Only emitted in the genuinely-clean case. |

Wanaka TP 1 lands in **Setup prescription** (deflection L/D 7.5 unreachable from search space). The 116 s faster stepover-2.0 candidate appears as the "best trade-off" surface alongside the prescription, not as a refusal.

### What we're explicitly *not* doing

- No new gate. Three gates is the policy.
- No analytical power solver. Sim is the oracle.
- No baseline-trace mutation.
- No prescriptive auto-apply. Operator applies.
- No special wanaka case. Every rule applies symmetrically.

## What to keep from current code

- Provenance gates (`optimize.rs:256-280`)
- LUT row matching (`feeds/vendor_lookup.rs`)
- Sim-as-oracle (no analytical short-circuits)
- Two-stage resolution (1 mm coarse → 0.5 mm fine)
- Session/guard restore pattern in `optimize_toolpath`
- Stage-1 grid builders (`build_doc_variants`, `build_stepover_variants`) including the recent factory-default anchor — useful as Stage E
- `OperationConfig::new_default(op_type)` as variant anchor

## Implementation survey — primitives we lean on

(Full survey in conversation 2026-05-08, condensed here.)

- **`MatchedRow`** (`feeds/vendor_lookup.rs:29`) — public fields cover what we need: `chip_load_min_mm`/`chip_load_max_mm` (Option), `rpm_min`/`rpm_max`/`rpm_nominal` (all Option), `ap_min_mm`/`ap_max_mm`, `ae_min_mm`/`ae_max_mm`. Many Amana rows publish only `chip_load_max_mm`.
- **`chipload_midpoint`** (`vendor_lookup.rs:215`) — already handles all four bound combinations. Currently private; needs exposing.
- **`radial_chip_thinning_factor`** (`feeds/geometry.rs:17`) — `pub fn (ae_mm, diameter_mm) -> f64`. Clamped `[1.0, 4.0]`.
- **`MillingCutter::lookup_diameter_at(axial_doc_mm)`** (`tool/mod.rs:150`) — canonical engaged-diameter accessor. Tapered ball overrides at `tool/tapered_ball.rs:166`.
- **`OperationParams` trait** (`compute/catalog.rs:678-722`) — feed/plunge/stepover/DOC/RPM accessors and setters all present.
- **Steady-state filter** — inline in `chipload::evaluate` (`chipload.rs:137-150`). Not factored. Constant `STEADY_STATE_FEED_FRACTION = 0.95` at `chipload.rs:68`. **Extract before bipolar pre-check lands** — duplicating it would silently drift.
- **`RefuseReason::BipolarEngagement`** — exists, has explanation string at `mod.rs:109` (currently generic, needs op-aware specialization).

## Call sites — blast radius

- **`optimize_toolpath`** called from: `rs_cam_mcp/server.rs:738`, `rs_cam_viz/compute/worker.rs`, `rs_cam_viz/controller/events/compute.rs`, `rs_cam_core/tests/optimize_smoke.rs`.
- **`OptimizeOutcome`** consumed by: `optimize_modal.rs:88-139` (single-TP modal), `optimize_project.rs` (rollup), MCP wire (serialized JSON, `tag="kind", content="detail"`).
- **Public assumptions to preserve:** `Ranked[0]` and `attempted[0]` are baseline; rest of `Ranked` sorted ascending by `cycle_time_s`; `first_safe()` returns the recommendation.
- **MCP wire**: adding new outcome tiers as new variants is non-breaking under serde defaults; consumers branching on `kind` need to handle new values.

## Risks and traps (10 specific ones from the survey)

1. MCP wire schema — adding tiers is serde-safe but `kind`-branching consumers need updates. Description string at `server.rs:715` lists current tags.
2. `Ranked[0]` baseline assumption hardwired in modal at `optimize_modal.rs:148` — must preserve through tier change.
3. `feeds_auto.feed_rate` flag invalidation (latent bug #9) — must clear when mutating feed.
4. `MachineProfile.safety_factor` plunge cap — Stage F plunge update must respect `material.plunge_rate_base() × safety_factor`.
5. `MatchedRow.rpm_max` nullable — Stage F needs same fallback as `solve_headroom_scale:743` (`rpm_nominal × 1.2` then machine ceiling) or refuse with `RpmBracketEmpty`.
6. Re-target with no LUT row — refuse with `NoVendorData`. Decision lives in pre-flight classifier.
7. Stickout prescription needs to point at `ToolConfig.stickout` by name; UI doesn't currently surface "edit stickout" from optimizer modal.
8. Tapered-ball at variable DOC — engaged diameter changes per-pass. Latent bug #10 means `find_matched_lut_row` uses nominal diameter; redesign should fix this.
9. `burn_samples` median collapse at `chipload.rs:246-261` — bipolar detection needs sample counts on each side, not the median.
10. `OptimizeOutcome` Debug strings — safe to add fields/variants, not safe to rename.

## Commit sequence

### Pre-A — Latent bug fixes (independently valuable)

**A1: Clear `feeds_auto.feed_rate` when optimizer mutates feed.** ~10 LOC + 1 test. Files: `optimize.rs::apply_scale_to_op`. Stops candidates evaluating against unintended baseline-recomputed feeds.

**A2: Tapered-ball LUT lookup uses `lookup_diameter_at(commanded_doc)`.** ~5 LOC + tests. Files: `optimize.rs::find_matched_lut_row`. Affects every tapered-ball op's matched row.

### Prep — Mechanical refactor (no behaviour change)

**B: Factor `optimize_toolpath` into stage helpers + extract steady-state filter.** ~100 LOC moved. Files: `optimize.rs` (extract `run_stage_0`, `run_stage_1_grid`, refine `refine_stage_2` signature), `chipload.rs` (extract `steady_state_samples_for_toolpath` as `pub(crate)`). 0 LOC of policy change. Makes commits #1/#2/#3 each ~half the LOC to review.

### #1 — Bipolar pre-check + `DeflectionSetupLocked` refusals

~80 new LOC, ~30 modified. Files: `tool_load/optimize.rs`, `tool_load/chipload.rs` (consume the extracted filter), `tool_load/mod.rs` (new variant + explanation). 6 new tests.
- Wire bipolar predicate into pre-flight, emit `BipolarEngagement` when steady-state samples straddle `cl_min` and `cl_max`.
- New `DeflectionSetupLocked` `RefuseReason` variant with stickout-prescription string.
- Op-aware prescription routing (`OperationFamily` → string).
- **Independently landable.** Wanaka after this commit gets a typed bipolar refusal instead of `NoImprovementFound`.

### #2 — Stage F re-target

~150 new LOC, ~20 modified. Files: `tool_load/optimize.rs` (new `solve_chipload_retarget`, `apply_retarget_to_op`, plunge-tracking helper). 5 new tests; 3 expected-value updates on `apply_scale_to_op` tests.
- Solver: `target_chipload_eff` = LUT midpoint (4-arm match for nullable bounds), `target_chipload_nom = target_eff × RCTF(commanded_ae, engaged_diameter)`, RPM clamped by machine + LUT bracket, feed = `target_nom × rpm × flutes`.
- Plunge update if `|Δfeed/baseline_feed| > 0.10`, capped at `material.plunge_rate_base() × machine.safety_factor`.
- Soft-depends on #1 (so re-target doesn't try to fix bipolar).

### #3 — Per-candidate gate deltas + tiered recommendation

~120 new LOC, ~50 modified. Files: `tool_load/optimize.rs` (new `GateDelta` enum, `classify_candidate_vs_baseline`, tier dispatcher), `rs_cam_viz/ui/optimize_modal.rs`, `rs_cam_viz/ui/optimize_project.rs`, `rs_cam_mcp/server.rs` (description string). 10+ new tests.
- `OptimizeCandidate` carries `(chipload_delta, power_delta, deflection_delta)`.
- `build_outcome` becomes tier dispatcher: Improvement / Trade-off / Setup-prescription / No-improvement.
- UI updates: trade-off tier renders distinct from ranked, prescription tier deep-links to relevant config field where possible.
- **Strictly depends on #1.** Biggest blast radius.

## Worktree plan

Land in `/home/ricky/work/rs_cam_optimizer_redesign` (new sibling to existing `o3`/`o5b`/`o5c`/`o6` worktrees). Shares object database with master, separate `target/` so the perf agent's build cache stays warm. Commits A1, A2, B can land on master directly (low-risk, mechanical) or in the worktree depending on operator preference.

Disk: 127 GB free, repo ~1 GB without target, each worktree's target ends up ~5-15 GB. Safe.

## Status as of 2026-05-08

- Audit complete. Implementation survey complete. Plan written (this doc).
- Recent merged work: factory-default anchor in `build_doc_variants`/`build_stepover_variants` (`optimize.rs:842-970`) lets variant grids reach the operation's default value when baseline drifts. Two regression tests (`*_factory_default_anchored_when_baseline_low`).
- Nothing else from this plan is implemented yet.
- Companion task list (#11 stepover bump TP 1, #12 tapered-ball envelope TP 4/5/7) tracks operator-side empirical work, separate from this redesign.

## Resuming this work after context compaction

1. Re-read this doc.
2. `git -C /home/ricky/personal_repos/rs_cam status` — check whether the worktree exists and what's landed.
3. `cargo test -p rs_cam_core --lib stage1_grid_tests` — confirms the 18 grid tests still pass (baseline check).
4. Pick up at the commit sequence above.
