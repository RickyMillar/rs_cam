# Optimizer clean-up and rebuild plan, 2026-10-12

Status: Phase 1 DONE (e3210f80). Phase 2 R1 DONE (4a9884f7), R2 DONE
(79e4100b). Follow-ups from the first Z22 measurement are in §5. The
result record is `RESULT.md` beside this file. Branch `optimizer-rebuild`,
worktree `.claude/worktrees/optimizer-rebuild`, target dir `target/optrebuild`.

The review is `REVIEW.md` beside this file. The operator ruled: remove every
duplicate that is not the new feeds work, then rebuild. The feeds screen
(Suggest) owns the PRE-SIM calculation. The optimizer tunes with the
POST-SIM measurements.

## 1. The cull criterion

One sentence: the Job contract and its instruments survive; every search
stage, every private formula and every narrative that served a deleted
stage goes.

### Survives (the wiring)

| Item | Why |
|---|---|
| `optimize_toolpath`, `optimize_toolpath_observed`, `optimize_project`, `ProgressReporter`, `NoProgress` | The Job contract (`session/compute.rs::execute_optimize_toolpath`, pinned by `optimize_toolpath_is_a_job_wp14b`) |
| `OptimizeOutcome`, `OutcomeKind`, `OptimizeCandidate`, `ProjectOptimizeReport` | The GUI, the MCP reply and the rollup read them |
| `OptimizeProgress`, `SearchPhase` (rungs renamed in Phase 2) | WP29; the workspace bar and the modal read them |
| `SimAssumptionStamp` family, `MachineSnapshot` | A-8 honesty: every outcome states the operating point |
| `context.rs`: `EvaluationContext`, `BaselineRestoreGuard`, the trace readers | Any candidate evaluation needs them |
| `candidate.rs`: `evaluate_candidate`, `candidate_sim_options` | The one sim-and-gate step (its options change in Phase 2) |
| `ParamDelta` (moved into `candidate.rs`) | The GUI row text (`ui/components/format.rs`) |
| `narrative.rs`: `OutcomeNarrative { headline, explanation, limiting_gates, envelope }`, `LimitingGate`, `GateKind`, `SearchEnvelopeReached`, `AxisExtent` | The modal and the rollup render them |

### Goes (file / symbol / why / who owns the concept now)

| File or symbol | Why it goes | Owner now |
|---|---|---|
| `strategy/headroom.rs` | Private RPM and feed scaling; `rpm_nominal x 1.2` goes past the chart (F9) | `feeds::suggest` honours `spindle_strategy` |
| `strategy/grid.rs`, `strategy/retarget.rs`, `strategy/mod.rs` | A DOC grid anchored at baseline multiples (F3, F5); per-gate retargeters | DOC bound: `feeds::cutter_constraints`; feed: `feeds::calculate` |
| `retarget/{chipload,power,deflection,mod}.rs` | Private chipload, power and deflection solves | `feeds::force`, `feeds::operating_point`, the gates |
| `policy.rs` | 2026-05 constants with no source (multipliers, headroom, weights, tolerance bands) | None; the rebuild derives every bound from an owner |
| `rank.rs` | Composite score with weights 5, 3, 2 and no depth term (F1) | Rank by measured cycle time among the full-cut candidates |
| `delta.rs` (`GateDelta`, `candidate_is_safe`, `candidate_is_marginally_safe`) | A second safety predicate; it vetoes on the rule-of-thumb depth row (F1) | `CriterionStatus::refuses_export` (`verdict.rs`) |
| `preflight.rs`, `refusal.rs` | Refusals computed from private formulas before any sim | The gates on the baseline trace |
| `bounds.rs`, `space.rs`, `axes.rs`, `patches.rs` | The private search space and its bound sources | `cutter_constraints`, the op setters |
| `tolerance_bands_from_policy` | The optimizer widened the gates by 5 % (two measures) | `ToleranceBands::default()`, the same as the card |
| `narrative.rs`: `suggest_levers`, `KnobAxis`, `OperatorSuggestion`, `EntryAdvisory`, `DeflectionSetupDetail`, `NarrowChipBandRefusal`, the stale text | Served the deleted stages | The tool-load card shows entries |
| `OutcomeKind::{MarginalSafe, TradeOff}` | Defined by the tolerance bands and the gate deltas | — |
| `SearchStage::{Stage0Headroom, Coarse, Refined}` | Three resolutions, two operating points (F7) | One measure (Phase 2) |
| `RefuseReason::{ChiploadBandNarrowerThanHeadroom, BipolarEngagement, DeflectionSetupLocked}` | Produced by deleted code only | — |
| `retarget_reconciliation_a8.rs`, `tests.rs`, `stage1_grid_tests.rs`, `candidate_eval_tests.rs`, parts of `orchestration_skip_tests.rs`, `project_rollup_tests.rs` | Test the deleted stages | New sentries in Phase 2 |
| `compute/catalog.rs::OptimizationSurface::Optimizable(AxisView)` | The payload is the deleted axis set | Keep the classification with a unit payload |
| GUI: `draw_suggestions`, `suggestion_apply_button`, `format_entry_advisory`, `draw_refusal_section` (band refusal), `AppEvent::ApplyOptimizeSuggestion` and its controller arm | Served the deleted narrative | — |
| GUI: `MarginalSafe` and `TradeOff` rows in `optimize_project.rs`, the "verify on a scrap" copy | The kinds are gone | — |

Not touched: `tool_load/depth.rs` (the gate; S3 says it reports and never
refuses), `session/compute.rs::optimized_candidate` (the strategy advisor).

Phase 1 end state: `optimize_toolpath` scores the baseline, evaluates no
candidate and returns `NoSafeImprovement` with the explanation "the
search has no levers yet (rebuild in progress)". GUI and MCP compile and
show that text. Target: `optimize/` from 15 776 lines to under 3 000.

### Breaks to list in the Phase 1 commit

- MCP `optimize_toolpath` reply: `kind` has three values (`ranked`,
  `no_safe_improvement`, `skipped`); `candidates[].stage` has two
  (`baseline`, `measured`); `candidates[].gate_deltas` is deleted;
  `candidates[].reconciled_*` stays; `narrative.suggestions`,
  `narrative.entry_advisories`, `narrative.improved_gates`,
  `narrative.worsened_gates`, `narrative.deflection_setup`,
  `narrative.chipload_band_refusal` are deleted.
- `OptimizationSurface::Optimizable` carries no `AxisView`.
- `OptimizeProgress` rung names change in Phase 2.

## 2. Phase 2: the post-sim tuner

### The division of labour

Suggest writes the pre-sim recipe and keeps its rigidity clamp
(`invariants.rs::clamp_dpp_to_rigidity`). The optimizer never calls that
clamp. It reads feed, plunge, ramp and RPM for a candidate DOC and WOC
through the read-only door `feeds::suggest::feeds_result_for_operation`
on an operation clone (the calculator honours the `axial_depth_mm` hint,
`feeds/mod.rs:1951`) and writes them with the op setters. A cell the
calculator moved (`result.axial_depth_mm != candidate dpp`: the flute
guard or the slotting cap) is refused, not shipped.

### The search (one measure, lever order from the review §5)

Inputs: the job's session clone, the on-screen baseline trace, the index.

1. Baseline reading. Gates with `ToleranceBands::default()`; cycle time
   and `runtime_by_intent` from the trace; removed volume
   (`total_removed_volume_est_mm3`); this toolpath's rapid collisions
   (`attributed_rapid_collisions`); the "crosses material" finding from
   `simulation_triage`. Material: `force_line()` `Err` is stated on the
   outcome ("power and deflection are not modelled: <reason>, source
   `<source_id>`"); the search continues on the sourced bounds that remain.
2. Lever L1, links: the baseline operation with `dressups.link_moves =
   Some(LinkDressupParams::default())`. One sim. Kept when it passes §3.
3. Lever L2, depth and width: DOC values on the realised staircase
   (`ops::depth::realised_step_down`) from the baseline up to
   `cutter_axial_constraints(...).safe_max_doc_mm()` (vendor `ap` rule,
   deflection at the gate bound when a force line exists, flute guard);
   at most 4 DOC values. Stepover values: baseline and the calculator's
   `radial_width_mm`; at most 2. Each cell: feed, plunge, ramp and RPM
   from `feeds_result_for_operation` at that cell (RPM therefore honours
   `spindle_strategy`; P8 is inside P5). Each cell carries the L1 result.
4. Every candidate simulates with the options the GUI's simulation
   submit builds (R1 reads the submit path in `compute/worker.rs` and the
   controller and copies it: resolution, modulation, strategy, scale).
   The P6 test is the proof. `SearchPhase` rungs: `Links`, `DepthWidth`.
5. Rank: among candidates that pass §3, the minimum `total_runtime_s`.
   `Ranked` when it is faster than the baseline; else `NoSafeImprovement`
   with each candidate's failing check in `limiting_gates`.

### 3. The acceptance predicate (every candidate)

- No gate row `refuses_export()`. The depth row is reported, never a
  veto (`GateKind::Depth` is added so the row names itself).
- Full cut: removed volume within one floor layer of the baseline
  (`|dV| <= V_baseline x resolution_mm / total_depth_mm`; 3D ops with no
  total depth skip the volume check and keep the next two), rapid
  collisions not more than the baseline's, and the "crosses material"
  peak not above the baseline's peak plus one cell.
- The candidate carries these readings (`FullCutCheck`) and the GUI and
  MCP show them.

### Packages, acceptance tests, waves

| Package | Scope | Sentry (new, `tests/`) |
|---|---|---|
| R0 instrument | `#[ignore]` `optimize_z22_headline.rs`: copies the mini project into the scratchpad, runs the job on the Z22 channel, prints cycle, verdicts, volume, collisions | runs by name |
| R1 search core | Steps 1-5 and §3 in `optimize/{mod,search,full_cut,candidate,narrative,outcome}.rs` | `optimizer_judges_by_sourced_bounds_g_opt1` (depth row reported, not a veto); `one_measure_for_baseline_and_candidate_g_opt6` (a candidate sim at baseline params is within 1 % of the on-screen cycle); `a_feed_follows_the_depth_through_the_calculator_g_opt5`; `a_gouge_or_a_thin_cut_is_never_ranked_g_opt7` |
| R2 surfaces | Modal and rollup rows: depth badge, full-cut column, the Unmodeled note, the rungs; MCP feature token `optimize_full_cut_check` | `cargo test -p rs_cam_viz -q --lib`, `per_toolpath_optimize_names_the_missing_trace_w3k`, `cargo test -p rs_cam_mcp -q` |
| R3 docs and measure | `tool_load/CLAUDE.md`, `ui/CLAUDE.md`, `FEATURE_CATALOG.md`, `PROGRESS.md`, this file; run R0 and record the numbers in `RESULT.md` | — |

Waves: W1 = Phase 1 cull (one agent, serial, cargo). W2 = R0 + R1 (one
agent, cargo). W3 = R2 (one agent, cargo, after R1). W4 = review agent
(fresh, reads the diff by path) and R3 (orchestrator). Every cargo call
goes through `timeout <s> scripts/cargo_lane.sh`, `CARGO_TARGET_DIR`
`target/optrebuild`, one job at a time.

Checks per wave: `cargo test -p rs_cam_core -q --lib tool_load`,
`--test optimize_smoke --test optimize_toolpath_is_a_job_wp14b --test
optimize_reports_progress_wp29 --test the_depth_that_cut_is_a_measured_load_g_s3depth
--test a_weak_bound_cannot_refuse_an_export_g_s4weak --test command_registry_completeness`,
`cargo test -p rs_cam_viz -q --lib`, the viz optimize tests, `cargo test
-p rs_cam_mcp -q`, clippy on the changed crates with the three core
features, `cargo fmt --all -- --check`. No full gate.

## 4. Open questions for the operator

- Per-level retract height (review P2, `ops/pocket.rs`): a generator
  change for every pocket. Not in this rebuild; the link lever is.
- The mini project's oak (`rivmap feeds_oak.toml`) has no FPL density.
  Power and deflection stay Unmodeled until the stock names an FPL row.
  No Janka-to-density proxy is invented here.
- P9 (the card's boundary rule in `distribution.rs`) is a card fix, not
  an optimizer change; handed off.

## 5. Follow-ups from the first Z22 measurement (2026-10-12)

The headline run (`tests/optimize_z22_headline.rs`, 96 s) gave `Ranked`
876 s against 1 315 s at dpp 0.815 / stepover 1.05. Three findings:

- The over-cut finding (`stock/sim_triage.rs`, "3x this pass's own
  median bite") fired on every deeper cell with a peak equal to the
  cell's own dpp, and gave opposite readings at one dpp on two stepovers.
  Lead ruling: fix the reference in the owner (the commanded dpp of the
  pass); keep the finding on a real thick bite into standing material.
- Rapid collisions at stepover 1.05 (18 at dpp 1.22, 8 at dpp 2.75;
  none at 0.838). Lead ruling: read the records; real collisions keep
  the veto; a defect goes to its owner. RESULT: false positives of the
  dexel `conservative_top` stamp (G-STALETOP, ff4af15a); owner
  `dexel_stock/stamping.rs`; the veto stands as an upper bound.
- `feeds::suggest::apply_speeds_to_op` re-derived the feed for the
  clamped depth (3877 against the calculator's 2423 at 2.5 x D). Lead
  ruling: a speeds-only apply ships the feed for the depth the operation
  keeps; a feeds-side package with a test. DONE d5d1daaa (G-SPEEDSAPPLY);
  the per-field pill still reads the `Both` dry run (handed off).
- The over-cut reference: DONE 98e3de6f (G-OVERCUTREF).
- Handed off, not in scope: the GUI readiness check and the export
  pre-flight read `ToolLoadReport::any_exceeded`, which counts the
  depth rule-of-thumb row as a Fail (`ui/readiness.rs`, `ui/preflight.rs`);
  the export door itself uses `refuses_export` (S4).

