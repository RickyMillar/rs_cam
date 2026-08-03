# Second technical-debt programme — research and fix plan

Date: 2026-08-04  
Basis: `RESEARCH_COMMISSION.md`, the completed radius/instrument programme, and HEAD `7a84472`  
Audience: one orchestrator and sequential implementation agents  
Status: **PLANNED — research-only until the named human checkpoints rule otherwise.**

## 0. Executive decision

This programme starts by restoring a trustworthy gate baseline, then treats the feeds/Suggest stack as the first system-wide census. It must not repeat the prior programme's error: changing a machining behaviour while measuring the instrument that will judge it.

The main dependency graph is:

```text
R3 permanent-red verdicts ───────────────────────────────┐
                                                          ├─> clean baseline
R1 feeds census + B3 reconciliation ──> Checkpoint B ────┤
                                                          ├─> common pre-/post-sim feed model
R2 adversarial 2D fixtures ──────────────────────────────┤
                                                          ├─> 2D per-operation fixes
R4 simulation issue inventory ──> typed report channel ──┤
                                                          ├─> gate/UX decisions
R5 drill evidence audit + source refresh ─> Checkpoint D ┤
                                                          ├─> recalibration, if warranted
R6 reference-fixture quality + P7 policy ────────────────┼─> re-open B1/B2 only if qualified
R7 structural hygiene ─> arcfit intent fix ──────────────┼─> all intent-keyed metrics
                                                          └─> final live validation
R8 bounded scouts ────────────────────────────────────────> ranked next-programme intake
```

The programme has two distinct outputs:

1. **Research records**: censuses, reproducible fixtures, reference renders, evidence packs, and explicit `NOT FIXED`/`NOT RE-RUN` decisions.
2. **Small, independently reviewable fixes**: only after a checkpoint approves the decision, with one mechanism per behavioural PR.

No global feed-model rewrite, diagnostic-count retune, threshold recalibration, broad `offset_polygon` rollout, or reference-fixture replacement may be bundled with unrelated output changes.

## 1. Inherited state — do not reopen settled work

The first programme is closed. Its historical strategy verdicts are tracked in `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`; its ruled choices are not hypotheses for this programme to revisit.

### 1.1 Items inherited as open work

| Ledger / source | This programme item | Treatment |
|---|---|---|
| B3 | Four disagreeing chipload numbers | **R1-H1**, highest product-risk census; reconcile before moving any operator-actionable value. |
| B4 / A/L2 | Rivers 6.07 mm axial-DOC spike | **R4-M1** diagnosis only; first test upstream coverage, not arcfit. |
| B5 | `arcfit` intent inheritance | **R7-H2**, structural/output fix before any new intent-selected population is trusted. |
| B7 | GUI worker hand-copies findings | **R7-H1**, structural adapter replacement, report wiring only. |
| D-16.1 | UnifiedFinish band run-off overcut | **R7-H3**, reproduce/render first; planner change needs checkpoint. |
| D-16.2 | UnifiedFinish Shallow ignores `stock_to_leave` | **R7-H4**, red-first dial sentry then checkpoint; behaviour changes output. |
| D-LV.1 | `screenshot_toolpath` exporter omits new emission | **R7-M2**, exporter-only repair; viewport already passed. |
| `rest_grid_resolution_c9` | Refinement finds less / reach collapses | **R6-M3**, preserve anomaly and isolate detector geometry before proposing resolution. |
| VerySteep clip limitation | MidSteep/Shallow partial clips invisible | **R7-L1**, instrument census; do not claim all bands covered. |
| cavalier `Shape` panic mapping | Panic can become a silent collapsed offset | **R2-H2**, adversarial reproduction and explicit failure policy. |
| Criterion baseline absence | Offset/classification regressions lack recorded baseline | **R2-M2**, establish repeatable characterization protocol, not a numerical CI threshold. |
| P7 | Three ignored mega-harnesses rot | **R6-M2**, decide split/archive/cadence; no fourth unowned mega-harness. |
| B1/B2 | v3 closure and scaled/cascade-invariance questions | **Deferred to R6 checkpoint**; only a qualified reference fixture can reopen them. |

### 1.2 Ruled or closed — record, do not redo

- B6 remains **Info** for the extrapolated chipload burn advisory unless an operator explicitly reopens severity as a product decision.
- B8 (untouched/standing split in diagnostic and MCP summary) is complete.
- M3 production tile-raster classifier, C1 transform provenance, C2 `GridZ`, C3 width-model unification, C4 typed semantic keys, C5 boxing, C6 fixture library, C8 findings collections, C9 shipped reach ruling, M5 arc-carrying offset cascade, A/M6 `ClaimsReference::Auto`, and live validation C1–C7 are closed baselines.
- The sampled-cross-section reach model remains **non-production dark code** for its stated reason: its routing/coverage bars require a 0.002–0.010 mm rest cross-section pitch. Do not switch it incidentally.
- The steep/shallow resolution selector is deferred with its written discriminating-fixture condition. Do not treat prior zero-delta fixtures as an exoneration.

## 2. Non-negotiable rules

1. **Research first.** Production is untouched during a research wave. Candidate algorithms live behind test-only hooks or a non-production strategy enum. If the premise is refuted, publish and re-scope; do not implement the brief faithfully anyway.
2. **Pointwise quality only.** Quality verdicts use `SimulationResult::column_deviations` or the scallop oracle, never area/cell aggregates. A result must name domain, stage, resolution, common population, and whether `resolution_clamped` is false.
3. **Normal-domain gouge only.** Surface-quality gouge/deviation comparisons use surface-normal deviation, never vertical Z deviation. The display-only vertex colours are not a quality oracle.
4. **Render before verdict.** Write and read a surface/toolpath render before publishing an aggregate conclusion. Record the output path in the evidence table. A warning unseen by a user is not a warning.
5. **Population at source.** Select test populations by geometry or source intent/typed semantic roles, never a post-transform label or inferred move intent. `arcfit` invalidates label-selected populations until R7-H2 lands.
6. **Two fixtures for strategy comparisons.** One fixture is not a strategy verdict. At least one fixture must contain the geometric failure class at issue; smooth/convex fixtures cannot decide concave/reflex defects.
7. **Pre-register the bars.** Tolerances, performance ceilings, common population, resolution, and non-vacuity condition go in the test/report before evidence is run. A changed instrument requires re-measured thresholds, not copied pins.
8. **No cross-resolution collision clearance.** Compare the same fixed toolpath at matched resolutions; tip-matched simulation is authoritative. A coarse-only collision is quantified as coarse quantisation, not cleared by assertion.
9. **Wanaka is a read-only play-file.** Never edit, stage, revert, save, or use `planning/airrun_2026-06-01/wanaka.toml` as a CI/gate dependency. `wanaka_suggest_baseline` remains environmental/permanently red.
10. **One Cargo job machine-wide.** Before any Cargo command, run `free -g` and `pgrep -af "carg[o]"`. The bracket matters; a bare `pgrep cargo` self-matches. Parallel research agents run no Cargo.
11. **No workspace-wide tests and no release inside waves.** Use the smallest target first, then per-crate targets. `adaptive3d_interior_cell_parity_f029` is about ten minutes when healthy; use `ps -L` rather than process-level `ps` to judge rayon liveness. Release is reserved for the final live-validation preparation, never a normal wave.
12. **Zero-warning code.** Run `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` before each commit. Keep test-only lint exemptions at module/test scope under the existing repository pattern.
13. **Red first; evidence in the commit.** Each fixed defect needs a reproducing test that fails on the parent revision. Record the failing observation and mechanism in the commit body; an assertion that cannot fail under the reverted behaviour is invalid.
14. **Re-pins are evidence, not housekeeping.** Any fingerprint update carries old/new values, the mechanism, the exact build, and a grep-based downstream-consumer census. Never capture a pin mid-wave.
15. **P7/P11/P12/P13 are practice rules.** Keep mega harnesses split or scheduled, update stale rationales when mechanisms change, prove fixtures can exhibit the defect, and name the operator/condition behind every deferral.
16. **Commit instruments promptly.** Once an instrument lints and its focused test is green, commit it before behaviour changes. Each agent appends its own dated `ORCHESTRATION_LOG.md` entry after commit; the orchestrator only reconciles status.
17. **Live validation is end-of-programme by default.** The operator waived per-behavioural-batch live validation last programme in favour of a single final MCP pass. Use that again unless a checkpoint explicitly marks a change crash-class/high-risk and requires an earlier live check.

---

# HIGH PRIORITY

## H0 / R3. End the three permanent adaptive3d reds

### Problem

The core-library gate has carried three accepted failures throughout the previous programme:

- `adaptive3d::path::tests::peck_plunge_progresses_when_depth_per_pass_equals_retract_clearance`;
- `adaptive3d::path::tests::rapid_segment_lifts_to_safe_z_before_traverse`;
- `adaptive3d::tests::planner_sim_dexel_parity_agent_search`.

A permanent exception list makes every later test result less trustworthy and hides new regressions behind familiar noise.

### Research questions

1. What exact production contract does each test assert, and is it still the intended CAM/machine-safety contract?
2. On which commit did each go red? Bisect only if history and the initial reproduction identify a finite window.
3. Does each fail deterministically on a quiet, single-Cargo machine? Separate wall-clock flakiness from a semantic failure.
4. Is the failure in planner geometry, rapid/peck emission, simulator interpretation, fixture assumptions, or a stale assertion?
5. Does an operation-level/toolpath render confirm the test population reaches the claimed geometry?

### Research deliverables

Create `planning/review_2026-08-04/ADAPTIVE3D_RED_BASELINE.md`, one section per test containing:

- current command, duration, deterministic result, and failure text;
- test/source provenance and first-bad commit or an honest `NOT BISECTED` reason;
- intended contract in operator language;
- minimal reproducer and a render/simulation artifact where geometry is involved;
- verdict: `FIX_CODE`, `FIX_TEST`, or `RETIRE_WITH_REASON`;
- a proposed red-first sentry and blast-radius list.

### Preferred fix shape

1. First make the existing tests isolate one contract each; do not weaken their assertion to green them.
2. For a code defect, add a compact integration sentry that fails against the parent revision before changing production.
3. For a stale test, replace it with the current contract and preserve the old failure evidence in the commit body.
4. For retirement, delete only after an operator approves the documented reason and a successor safety/property test covers the intended risk.

### Acceptance gates

- Baseline has **zero unclassified** adaptive3d failures.
- After the approved fixes, `cargo test -p rs_cam_core --lib` has no accepted-red allowlist.
- No test is converted to ignored merely to clean the count.
- Planner/simulator parity tests compare the same stock frame, toolpath and resolution.
- A rapid test verifies geometry and intent, not only move count.

### Scope and risk

- Primary files: `crates/rs_cam_core/src/adaptive3d/{mod.rs,path.rs,clearing.rs}`, simulation/session callers, focused tests.
- Size: S–M research; unknown implementation.
- Risk: high if rapid/peck semantics move; otherwise medium.
- Dependency: **first execution wave; blocks claiming a clean baseline for later behaviour work.**

---

## H1 / R1. Feeds/Suggest census and one shared pre-/post-simulation model

### Problem

The operator can see multiple incompatible numbers for one operation: narration nominal, Suggest clamp/recalibration, simulated gate observation, and cited band. The prior campaign repaired the extrapolated-low-side disclosure but did not reconcile the four values. Tapered width was already shown to drift until C3 unified it, and width parity alone does not prove parity for RPM, power, deflection, machine limits, or provenance.

The current code spans `feeds/{mod,suggest,profile,rationale,geometry}.rs`, `tool_load/{chipload,power,deflection,verdict,mod}.rs`, `session/compute.rs`, diagnostics adapters, and optimisation. `tool_load::effective_feed_for_sample` is an existing correct precedent: chipload, power and deflection must evaluate the same effective feed.

### Research questions

1. Enumerate every user-visible or gate-driving “tool load/feed” number. For each: owner, units, stage, formula, input state, provenance, serialization path, and consumers.
2. Reconcile B3's four chipload readings line-by-line. Are they nominal commanded chipload, a pre-/post-Suggest target, predicted achieved sample chipload, a robust statistic, or a different operation/tool context?
3. At every LUT lookup, where do row selection, hardness adjustment, DOC derating, clamp, extrapolation, weak-provenance advisory, and rejection occur? Confirm the policy that hardness dials values rather than hard-rejecting rows.
4. Does Suggest project the same physical model later used by chipload, power, and deflection gates, or merely share selected inputs? Enumerate divergences for RPM, spindle caps, flute count, radial/axial engagement, kinematics, power, deflection, and DOC.
5. What does `feeds::geometry` mirror today, and which post-sim geometry terms are absent from pre-sim estimation?
6. Where does static Suggest necessarily differ from observed simulation because pre-sim lacks a material/kinematic state? Define that uncertainty rather than forcing false equality.
7. Are vendor-LUT and extrapolated-LUT gate semantics internally consistent at evidence, verdict, diagnostic wording, severity, and optimizer use?

### Research deliverables

Create `planning/review_2026-08-04/FEEDS_CENSUS.md`, modelled on `TOOL_SCALE_SEMANTICS.md`:

- an implementation census with `file:symbol` provenance and a count of distinct implementations per concept;
- a directed data-flow map: catalog/tool/material/machine → LUT/model → Suggest → persisted params → generated IR → predicted feed → trace → gates → diagnostic/optimizer;
- a B3 reconciliation table with all four live values, their definitions and units, and whether comparison is valid;
- a LUT state-machine table (`matched`, `scaled`, `clamped`, `extrapolated`, `rejected`, advisory); 
- a parity matrix across pre-sim Suggest, post-sim chipload, power, deflection, and optimizer;
- a source inventory for each literature-matrix feed/drill row touched by formula/threshold work;
- a fix sequence separated into report-only, structural parity, numeric/operator-visible, and calibration decisions.

Add a test-only “feed explanation snapshot” assembler if needed. It must label stages rather than choose a winner in prose.

### Preferred fix shape

The target architecture is **one canonical physical/load model**, with Suggest as a pre-simulation projection and gate evaluation as the same model supplied with observed engagement and achieved feed. It is not a demand for falsely identical results.

1. Introduce explicit stage/provenance-bearing explanation records before moving numbers.
2. Extract genuinely shared formulas into a model module only after the census proves two implementations express the same concept.
3. Make each divergence explicit: unavailable observed input, policy choice, different statistic, or defect.
4. Let simulation chipload remain the operational arbiter when observed data exists; Suggest reports its prediction plus uncertainty/provenance.
5. Preserve LUT evidence and weak-provenance low-side advisory policy unless Checkpoint B changes it.

### Acceptance gates

- The B3 diagnostic fixture emits four explicitly named values and explains every delta; no bare `chipload` label remains in the touched diagnostic path.
- Same canonical inputs produce parity across Suggest projection and gate-model projection where observed-only inputs are substituted with the same assumptions.
- Chipload, power, and deflection share effective-feed selection; a new RPM/power/deflection parity sentry proves it.
- A tapered shallow-DOC matrix exercises ball/taper controls and catches width-model regression.
- Vendor row ID, provenance, extrapolation status, advisory, verdict, and diagnostic wording agree in one end-to-end fixture.
- Literature-matrix tests remain green; stale citations are refreshed through `/refresh-lit-matrix`, not silently replaced.

### Scope and risk

- Primary files: `feeds/*`, `tool_load/*`, `session/compute.rs`, `diagnostics/adapters/from_tool_load.rs`, `narrate.rs`, optimiser modules, literature-matrix tests.
- Size: L research; XL structural/calibration implementation.
- Risk: **high**: this moves operator-actionable feed/RPM/DOC numbers and potentially gate/optimizer behaviour.
- Dependency: H0 baseline. **No numeric/default change before Checkpoint B.**

---

## H2 / R7. Structural hygiene before new measurements depend on it

### Problem

Three proven defect factories remain in shared output paths:

1. The viz worker maps `GenerationFindings` to `ToolpathStats` field by field, patched five times but structurally capable of silently dropping the next field.
2. `arcfit::fit_arcs` groups a run by feed and inherits intent from the first source move, relabelling lead/entry geometry as `FinishingCut` across the repository.
3. Finishing output has two user-affecting, reproduced gaps: D-16.1 band run-off overcut and D-16.2 Shallow `stock_to_leave` omission. The new exporter-only D-LV.1 and slow parameter/narration read paths are adjacent agent-workflow debt.

These need separate PRs: one is report transport, one is repo-wide geometry/intent output, and two alter machining geometry.

### Research questions

1. For the GUI worker mapping, can the CLI's exhaustive destructuring serde-view pattern be used without dragging GUI-only state into core? What fields are core-owned, GUI-owned, deprecated-wire-only, or computed after generation?
2. For arcfit, what is the exact run key? Must intent, move type, semantic scope, feed, Z, and span barrier all participate? What arcs are intentionally allowed across each boundary?
3. Which downstream gates/diagnostics select by post-arcfit intent, and which fingerprints include fitted geometry? Use an impact census before any code change.
4. For D-16.1, which operation stage lets a band extend/run off the stock footprint? Reproduce on the committed grooved block and read the rendered normal-deviation map.
5. For D-16.2, identify every Shallow raster generation path and its `stock_to_leave` semantics relative to MidSteep scallop, claims, and surface links.
6. For D-LV.1, locate the screenshot exporter’s move-classification divergence from the viewport and prove the viewport path stays untouched.
7. Can `narrate_toolpath` and parameterized reads produce bounded snapshots/incremental results without violating the current nonblocking cancel/status guarantees?

### Research deliverables

Create:

- `FINDINGS_PIPELINE_CENSUS.md`: every field copied at core/session/worker/GUI/MCP/CLI boundaries and proposed compile-time structural join;
- `ARCFIT_INTENT_EVIDENCE.md`: source-run matrix, affected consumer census, baseline fingerprints, and a minimal mixed-intent fixture;
- `FINISHING_OPEN_DEFECTS_EVIDENCE.md`: D-16.1, D-16.2, D-LV.1, MidSteep/Shallow clip observability, and slow-read diagnosis, each with reproduction, render, owner, and checkpoint requirement.

### Preferred fix shape

Split in this order:

1. **H2.1 findings transport**: core owns a complete report payload; the GUI worker derives its view through one exhaustive constructor/destructure. A new core field must produce a compiler error at the worker boundary. Preserve existing JSON keys deliberately.
2. **H2.2 arcfit intent**: break arc candidates at any intent boundary, then derive an arc’s intent from a homogeneous run. Do not “repair” downstream labels while the source transform remains wrong.
3. **H2.3 screenshot exporter**: reuse the viewport’s classification/intent decision or a shared renderer adapter; limit the change to exported images with an image/geometry sentry.
4. **H2.4 / H2.5 finishing geometry**: repair D-16.1 and D-16.2 one at a time after an operator selects intended output semantics. Never couple either to an arcfit fingerprint sweep.
5. **H2.6 agent reads**: prefer immutable published snapshots and incremental/bounded narration; do not hold the GUI frame loop for a full narration scan.

### Acceptance gates

- Adding a synthetic `GenerationFindings` field fails compilation in every required adapter until consciously handled.
- A mixed `FinishingCut → LeadOut → Entry` fixture produces no arc spanning the boundary; homogeneous arcs remain fit.
- Arcfit’s re-pin package includes direct/indirect consumers discovered by impact/grep, including `arc_raster` and any mixed strategy fingerprints.
- D-16.1 fixture has a non-empty rendered run-off population and normal-domain tail gate; its reproduction cannot pass on a convex/no-boundary fixture.
- D-16.2 is red-first: only `stock_to_leave` differs, Shallow output Z must differ after fix, and MidSteep/Ball controls preserve their established semantics.
- Screenshot regression compares the exporter output with the intended cutting geometry and demonstrates D-LV.1 red on the parent revision.
- Long-generation cancel/status regressions remain <1 s while narration/parameter-read work is tested.

### Scope and risk

- Primary files: `compute/{execute,config,stats}.rs`, `rs_cam_viz/src/compute/worker/execute/mod.rs`, `arcfit.rs`, `unified_finish.rs`, raster/dropcutter helpers, `narrate.rs`, `app/mcp.rs`, exporter/render modules.
- Size: S (exporter) to XL (arcfit/finishing paths), split into at least six PRs.
- Risk: H2.1/H2.3 low; H2.2/H2.4/H2.5 high because they move generated or post-processed machining paths.
- Dependency: H0. H2.2 blocks any new label-/intent-keyed research result. **H2.2, H2.4 and H2.5 each require Checkpoint F approval.**

---

## H3 / R2. Adversarial 2D-operation campaign

### Problem

Pocket, adaptive, profile, trace, zigzag, inlay, v-carve, rest, and 2D-driven drill have not undergone the hostile fixture treatment that exposed 3D defects. A twelve-vertex reflex cross demonstrated the risk: Pocket previously ran more than 13 minutes and 386 MB until the M5 cascade repair. The remaining single-shot offset consumers and their failure behaviour are not systematically known.

### Research questions

1. Which geometric primitives and operation families are GUI/CLI reachable and which have independent boundary/offset/path implementations?
2. Can a procedurally generated family cover: reflex comb/dendrite; holes within holes; disconnected islands; nearly coincident/self-near-touching walls; short/near-collinear edges; tiny islands; thin slots; high-curvature arcs; and valid/invalid source contours?
3. For each operation, what is the correct oracle: analytic offset distance, containment, no-gouge/non-empty path, topology preservation, input diagnostics, or drill-target count?
4. Which consumers are iterative cascades versus one-shot offsets, and which may adopt `OffsetRingSet`/one explicit flatten policy without losing required sampling density?
5. Can the release `cavalier_contours` `Shape` panic be reproduced under a controlled input? What is the correct contract: typed generation failure/finding, retry/alternate path, or explicitly unsupported input — never silent collapsed completion?
6. Where are termination/cancellation/memory limits checked, and can an early collapse look like a successful empty toolpath?

### Research deliverables

Create:

- `planning/review_2026-08-04/ADVERSARIAL_2D_FIXTURE_SPEC.md` with generators, valid geometry contracts, parameter ranges, expected operation applicability, and reference images;
- `crates/rs_cam_core/tests/common/` additions only after bit-identity donor tests prove unchanged existing helpers;
- `ADVERSARIAL_2D_FINDINGS.md`, one row per operation × fixture: command, wall time, peak RSS method, cancellation result, output topology/fidelity, render path, and severity ordered by GUI reachability;
- `OFFSET_CONSUMER_ROLLOUT.md`: each `offset_polygon` consumer, whether it cascades, its required flatten/sampling contract, candidate migration, and incompatible reason;
- a separate `CAVALIER_SHAPE_FAILURE.md` with minimal reproduction, library/version context, current mapping, and proposed explicit failure contract.

### Preferred fix shape

1. Add test-only fixtures and per-operation time/memory watchdog seams without changing production algorithms.
2. Fix one operation family at a time; a shared primitive change is allowed only after all direct consumers have an explicit policy decision.
3. Use `OffsetRingSet` only where the operation treats the polygon as geometry. Retain/re-establish sampling at the emission boundary where vertices carry sampling, as Scallop/Trace/ProjectCurve taught.
4. Replace “collapsed offset” masking with a typed, surfaced generation finding/error only after the failure contract is approved. Do not turn a panic into a clean-looking empty path.

### Acceptance gates

- Every 2D operation has at least one applicable adversarial fixture and one non-vacuity assertion.
- A test fails if an operation returns success with an unexpected empty/early-terminated path.
- Iterative cascades have an explicit bounded termination condition, wall-clock ceiling, and cancellation test; memory is recorded.
- Analytic offset rings remain within pre-registered erosion/containment tolerances.
- Concave/reflex fixtures prove they contain the mechanism before they gate offset behaviour.
- Any migration to an arc-carrying cascade uses a single declared `FlattenPolicy`; no global cleanup is silently applied.
- Broad path changes pass the relevant operation-family parameter sweeps with before/after fingerprint explanations.

### Scope and risk

- Primary files: `pocket.rs`, `adaptive/*`, `profile.rs`, `trace.rs`, `zigzag.rs`, `inlay.rs`, `vcarve.rs`, `rest.rs`, `drill.rs`, `polygon.rs`, `boundary.rs`, tests/common and sweeps.
- Size: L research; XL total fixes.
- Risk: high: topology, containment, and toolpath emission; isolate operation-family worktrees.
- Dependency: H0; may research in parallel with H1. Behavioural fixes require Checkpoint C per family or shared primitive.

---

# MEDIUM PRIORITY

## M1 / R4. Simulation issue-channel hygiene and measurable-resolution warnings

### Problem

A green Wanaka simulation can still emit around 99,000 “issues,” dominated by every out-of-material sample. The channel buries collisions and actionable hotspots in emission noise. In addition, a tip-matched dexel cell may still be too coarse to measure an operation’s finest physical cut depth, making finish air-cut/engagement values unmeasurable even where collision detection remains useful.

### Research questions

1. Enumerate every issue producer, consumer, reducer, GUI/MCP/CLI serialization route, and count aggregation rule.
2. Classify each issue as: safety/error, action-required finding, bounded advisory, diagnostic sample, or noise-by-construction.
3. What is the user’s page-one triage order? What bounded list of collisions, hotspots, typed findings, and summary counts answers “what should I act on?”
4. Which legacy gate/report consumers depend on raw `issue_count` or `AirCut` emissions, and which need compatibility fields?
5. For each operation type, what physical quantity establishes a minimum resolvable feature/cut-depth scale? Distinguish what dexel can judge: rapid collision, gross removal, engagement, air cut, drill-native metrics, and fine cusp finishing.
6. Can simulation report `Measurable`, `Degraded`, or `NotMeasurable` with an explicit reason rather than printing a precise-looking percent from an unresolvable grid?
7. Does the Rivers spike reproduce after grouping by upstream stock coverage and transit/source semantic role?

### Research deliverables

Create `SIMULATION_ISSUE_CHANNEL_CENSUS.md` containing:

- typed producer/consumer inventory with current counts from a synthetic fixture and read-only characterisation;
- proposed issue taxonomy, severity, deduplication key, bounded retention policy, and compatibility plan;
- page-one wireframe/data contract for operator/agent triage;
- a resolution-measurability matrix by operation and metric, naming what remains valid when a fine cut cannot be resolved;
- Rivers spike probe results and a `NOT REPRODUCED` or localised source/move/coverage result.

### Preferred fix shape

1. Add report-only typed findings and a summary first; retain raw samples/tracing only as an internal/debug channel.
2. Deduplicate bounded user-facing items by kind + toolpath + semantic/source region + spatial bucket, keeping worst severity/evidence.
3. Make `issue_count` explicitly legacy/noise if it must remain wire-compatible; never silently change a gate threshold due to a channel redesign.
4. Add a simulation measurability finding before considering a warning/refusal. It says which metric is untrustworthy and which safety measures remain valid.
5. Keep drill-native metrics separate from engagement metrics.

### Acceptance gates

- An all-air/out-of-material synthetic trace cannot create an unbounded user-facing list.
- One rapid collision, one holder collision, and one material-removal warning remain visible when thousands of air samples exist.
- Deduplication preserves the worst evidence and never hides a distinct safety event.
- A scallop-height fixture at cell size above its cut depth reports `NotMeasurable` for air/engagement rather than a valid-looking verdict; rapid-collision detection remains available.
- GUI, MCP, CLI, and narration consume one typed summary contract.
- Existing gates retain their semantics until an operator checkpoint approves any policy change.

### Scope and risk

- Primary files: `simulation_cut.rs`, `compute/{simulate,config}.rs`, diagnostics adapters, `narrate.rs`, viz simulation state/UI, MCP adapters.
- Size: M research; L implementation.
- Risk: medium: operator diagnostics and potential gate consumers.
- Dependency: H2.2 for intent-selected populations; Checkpoint D before modifying a gate, severity default, or numeric action threshold.

---

## M2 / R5. Drill subsystem evidence and literature re-validation

### Problem

Drill’s region roles are typed, but its material-banded thresholds — depth/diameter, per-peck D/d, plunge-feed envelopes, and chip-welding risk — were calibrated before the broader instrument corrections. The drill gate/evidence relationship has not had the audit applied to chipload reporting.

### Research questions

1. For each `DrillToolpathSummary` and `drill_gates` verdict, enumerate the evidence, formula, threshold source, statistic, units, material classification, and diagnostic wording.
2. Does every gate verdict agree with the displayed evidence, including “Within,” advisory, `NotApplicable`, and uncertainty cases?
3. Are depth/diameter, per-peck D/d, plunge rate, chip-welding, cycle-time, and tool geometry inputs using the same depth/diameter/material definitions in Suggest and simulation?
4. Which literature-matrix cells and sources justify each threshold? Are citations current under `/refresh-lit-matrix` policy, and is any recalibration a literature change versus an implementation bug?
5. Can a drill fixture distinguish a shallow/valid cycle, per-peck excess, depth excess, high/low plunge, and a material-boundary transition without relying on a label?

### Research deliverables

Create `DRILL_GATE_EVIDENCE_AUDIT.md`:

- gate-vs-evidence matrix for every drill verdict and all material bands;
- exact source/cell references and freshness result;
- diagnostic wording audit, including contradictory evidence cases;
- synthetic drill matrix results and any re-calibration candidates;
- recommendation: `NO_CHANGE`, `REPORT_FIX`, `MODEL_FIX`, or `THRESHOLD_RECALIBRATION` per gate.

### Preferred fix shape

1. Fix evidence/wire/wording contradictions separately from physical thresholds.
2. Reuse one material/geometry normalization layer where the census proves duplicate computation.
3. Any changed physical threshold comes with primary-source lineage, a stated uncertainty, and a before/after matrix. Do not tune to one simulation run.
4. Preserve drill-native metrics’ separation from generic engagement/air-cut measures.

### Acceptance gates

- A fixture deliberately outside each low/high gate produces `Exceeds` or a documented advisory matching the evidence.
- A Within verdict cannot display an observed value outside a hard bound without an explicit weak-provenance/advisory explanation.
- Literature-matrix drill sentries and source-freshness report pass; changed source IDs are documented in `CREDITS.md`.
- Drill hole/peck role selection uses `RegionSpanRole`, never labels.
- No drill policy/default/threshold moves before Checkpoint D.

### Scope and risk

- Primary files: `drill_metrics.rs`, `tool_load/drill_gates.rs`, `feeds/{suggest,predict,vendor_lut}.rs`, material model, diagnostics/narration, literature-matrix fixtures.
- Size: M research; M–L implementation.
- Risk: high if thresholds move; low for report-only corrections.
- Dependency: H1 feeds census; shares Checkpoint D with M1 only if both policy packages are ready.

---

## M3 / R6. Reference-fixture quality, rest-grid anomaly, and mega-harness policy

### Problem

The old terrain fixture is a coarse TIN: 1.8% of its triangles cover 40.8% of area, and its fine-quality bins are below repeatability. It can reveal strategy structure but cannot adjudicate fine quality. The rest-grid anomaly also shows current refinement behaviour is not understood. Meanwhile, three ignored campaign harnesses are compile-checked but load-bearing, carrying stale analysis prose and unstable/live inputs.

### Research questions

1. What analytic reference part covers mixed slopes, flat/convex/concave classes, tip-scale grooves, reflex boundaries, band run-off, and known surface-normal ground truth?
2. What tessellation/chord tolerance is necessary so mesh discretisation error is materially below each selected quality bin? Measure it rather than assuming a fixed triangle count.
3. Which simulation resolution and bin widths are repeatable over regeneration? Identify a minimum reportable bin and a non-repeatable region.
4. Can procedural generators in `tests/common/` supply deterministic vertex/triangle order and a reference evaluator without committing a large opaque blob?
5. Why does rest refinement detect less and collapse reach? Is the issue cross-section walk, ridge extraction, mask threshold, rim quantisation, interpolation, or an incorrect premise? Preserve the current anomaly sentry until explained.
6. For `v3_cascade_ab`, `p2c_headless_ab_wanaka`, and `strategy_comparison_h4`, which pieces are reusable loader/instrument code and which are dated campaign prose? Should each be split, archived, or scheduled?

### Research deliverables

Create:

- `REFERENCE_FIXTURE_SPEC.md`: analytic equations, feature catalogue, tool/dial ranges, tessellation rule, ground-truth evaluators, procedural API, render gallery, and intended gate classes;
- `REFERENCE_FIXTURE_REPEATABILITY.md`: regeneration/tessellation/resolution study, normal-domain residual distributions, and pre-registered defensible bins;
- `REST_GRID_ANOMALY_STUDY.md`: instrumented cross-section traces at 0.5/0.25/0.1 mm, each intermediate geometric quantity, and an evidence-based next owner;
- `MEGA_HARNESS_POLICY.md`: per harness `split`, `archive`, or `scheduled` decision; dependencies, cadence/owner, immutable fixture requirements, and historical-doc destination.

### Preferred fix shape

1. Build generator and ground-truth evaluator in test support first; use C6-style bit-identity/contract tests for existing helpers.
2. Promote fixture tiers: fast unit geometry, medium deterministic integration, ignored characterization, and read-only live validation. Do not turn an ignored mega harness into CI by accident.
3. Do not reopen B1/B2/v3 strategy claims until Checkpoint E approves both fixture fidelity and a repeatable, pre-registered quality bin.
4. Do not change rest-grid production resolution as a response to the anomaly. Fix/replace the measurement model only after its individual pipeline stages are observed.

### Acceptance gates

- Procedural reference geometry has stated analytic normal and surface truth; tessellation error is below the smallest adopted quality bin.
- Every fixture claims the failure mechanism it can exhibit, with a non-vacuity test (concavity/reflex/band/run-off as applicable).
- Repeatability study establishes bins from repeated runs, not a desired dial.
- Existing `terrain.stl` remains available as characterization, but no fine-quality winner gate depends on it.
- `rest_grid_resolution_c9` remains green until a replacement explains and supersedes it.
- Every mega harness gets a named owner, action, and cadence/archival pointer; no unowned ignored analysis body remains load-bearing.

### Scope and risk

- Primary files: `tests/common/{meshes,session,fingerprint}.rs`, fixture tests, H4/campaign harnesses, `rest_field.rs` research hooks, planning docs.
- Size: L research/infrastructure; implementation varies.
- Risk: medium: test oracle quality can misdirect high-risk work.
- Dependency: can research after H0 in parallel; **Checkpoint E blocks re-opening B1/B2 and any new strategy winner claim.**

---

## M4 / R8. Bounded untouched-territory scouting

### Problem

Import/mesh, project IO, export/post-processing, and GUI state/default consistency have recurring defect shapes but no ranked map. A broad deep-dive would derail the programme; doing no scan guarantees the next campaign rediscovers obvious serialization/default traps.

### Research questions

1. Import/mesh: what are the trust boundaries and high-risk transformations in STL, STEP/BREP, SVG/DXF import and face selection?
2. Project IO: where are `Serialize`-only fields, dual-key emitters, manual wire views, version migrations, and values written but never read?
3. Export/post: which output paths independently interpret move type/intent/coordinates, as D-LV.1 did?
4. GUI state: where do `Default` implementations, registry defaults, serde defaults, and UI initializers provide independent values for the same setting?

### Research deliverables

Create `UNTOUCHED_TERRITORY_RISK_MAP.md`, maximum one page per area:

- asset/data-flow map;
- top five risks ranked by GUI reachability, silent-failure potential, and blast radius;
- exact symbols/files and a one-sentence reproduction idea;
- `fix now`, `candidate for next programme`, or `ruled out` disposition.

### Preferred fix shape

Research only. A directly discovered crash/data-loss/security defect is logged separately with a minimal reproducer; otherwise do not begin fixes within this programme.

### Acceptance gates

- Every listed risk has a concrete code location and an evidence/reproduction hypothesis, not a generic fear.
- A Default/serde audit names both values and the load path before declaring divergence.
- No feature or schema change lands from this scouting work.

### Scope and risk

- Primary files: importers, `viz/io`, post-processors, `rs_cam_viz` state/config, project tests.
- Size: S–M.
- Risk: low if kept bounded.
- Dependency: none; run in a no-Cargo research lane.

---

# LOW PRIORITY / CONTINUOUS

## L1. Documentation, provenance, and benchmark hygiene

### Problem

P11 shows stale rationale can outlive the code it explained; P7 shows large ignored analysis bodies silently rot; fresh benchmarks without committed context cannot identify a regression. The prior programme correctly preserved historical records rather than rewriting them; this programme needs the same discipline while it adds evidence.

### Research questions

1. Which touched comments/docs make a numeric, mechanism, default, or performance claim that the current experiment changes?
2. Which benchmark context must be recorded to make a comparison repeatable: revision, profile, fixture, threads, CPU/load, debug/release, warm/cold state, timing method, RSS method?
3. Which external formula/algorithm sources are newly adopted and require primary-source verification/CREDITS lineage?

### Research deliverables

Each wave appends an `Errata / NOT FIXED, STATED / Deferred to` subsection to the programme log. At the end, produce `TECH_DEBT_2_CLOSEOUT.md` with:

- per-item verdict and links to sentries/evidence;
- all approved/rejected checkpoint decisions;
- stale-rationale sweep of touched modules/docs;
- benchmark provenance index;
- live-validation report and remaining operator-owned work.

### Preferred fix shape

Documentation follows executable evidence in the same PR or immediately following docs-only commit. Preserve historical numbers with a dated banner/erratum; do not rewrite prior records into current claims.

### Acceptance gates

- Every behavioural PR contains a mechanism and before/after evidence pointer.
- No planning/capability document promotes a strategy/quality winner without qualified fixture, two-fixture comparison when relevant, source population, and render.
- New external source/formula work updates `CREDITS.md`.
- No `NOT FIXED` line omits an owner and evidence condition.

### Scope and risk

- Spread across planning, module docs, tests, benchmark code, `CREDITS.md`, product docs.
- Size: continuous.
- Risk: low technically, high if neglected.

---

# 3. Tracking and orchestration

## 3.1 Programme tracker

The orchestrator maintains this table in this file and appends factual details to `planning/review_2026-08-04/ORCHESTRATION_LOG.md` after every completed wave. Status values are `NOT_STARTED`, `RESEARCHING`, `AWAITING_CHECKPOINT`, `IMPLEMENTING`, `VALIDATING`, `DONE`, `RE-SCOPED`, or `DEFERRED(owner/condition)`.

| ID | Priority | Work | Status | Blocking checkpoint | Evidence / commit | Next owner action |
|---|---|---|---|---|---|---|
| W0 | H | R3 adaptive3d red baseline | NOT_STARTED | A | — | reproduce and classify all three reds |
| W1 | H | R7 findings transport census/fix | NOT_STARTED | none for report wiring | — | exhaustive core→worker adapter design |
| W2 | H | R7 arcfit intent evidence/fix | AWAITING_CHECKPOINT | F1 | `planning/review_2026-08-04/ARCFIT_INTENT_EVIDENCE.md`; fixture `crates/rs_cam_core/tests/arcfit_intent_boundary_f1.rs` (4 exhibits, green = defect present); all baselines + controls green at `894e060`, clippy clean | operator rules F1 Q1–Q4 (evidence doc §6); PR-6 blocked until then |
| W3 | H | R1 feeds census/B3 | NOT_STARTED | B | — | build implementation/data-flow census |
| W4 | H | R2 adversarial fixtures/findings | NOT_STARTED | C | — | fixture generators and per-op inventory |
| W5 | M | R4 simulation issue census | NOT_STARTED | D | — | producer/consumer and measurability inventory |
| W6 | M | R5 drill evidence/literature audit | NOT_STARTED | D | — | run source freshness and gate matrix |
| W7 | M | R6 reference fixtures/P7/rest anomaly | NOT_STARTED | E | — | procedural fixture/repeatability study |
| W8 | M | R7 finishing defects/export/read performance | NOT_STARTED | F2/F3 | — | reproduce each defect in isolation |
| W9 | M | R8 bounded scouts | NOT_STARTED | none | — | one-page maps only |
| W10 | L | final evidence/docs/live validation | NOT_STARTED | G | — | consolidate after approved fixes |

### Required status update format

Every log entry includes:

```markdown
## W<N> — <item>, YYYY-MM-DD
Status: RESEARCHING | AWAITING_CHECKPOINT | DONE | RE-SCOPED | DEFERRED
Commit(s): <hashes or none>
Parent/revision measured: <hash>
Question and pre-registered bars:
Fixture/population/resolution:
Render/artifact paths:
Result (fact), interpretation, and uncertainty:
Red-first evidence / fingerprints changed:
Verification (focused commands + exact known-red state):
NOT FIXED, STATED — owner and re-open condition:
Next action / checkpoint request:
```

Agents update only their row/entry and never mark a checkpoint approved. The orchestrator updates merge order and checkpoint status after an operator ruling.

## 3.2 Work-package lanes

| Lane | Work packages | May run in parallel with | Must not overlap |
|---|---|---|---|
| A — baseline | W0 adaptive3d reds | B/C/D/E/F research | any behaviour-changing implementation |
| B — feeds | W3 census, B3 fixture, literature inventory | W0 research, W4 fixtures, W7 research | R5 threshold changes; shared diagnostics edits |
| C — 2D geometry | W4 fixtures/findings, offset rollout census | W3/W5/W6/W7 research | production `polygon.rs` changes and pocket/adaptive fixes in other worktrees |
| D — simulation/drill | W5 issue census, W6 drill audit, B4 diagnosis | W3/W4/W7 research | diagnostics UI/MCP implementation with W1 |
| E — fixtures/history | W7 reference fixture/P7/rest anomaly, W9 scouts | all research lanes | `tests/common` migrations without the C6 bit-identity gate |
| F — structure/finish | W1 findings boundary, W2 arcfit, W8 exporter/finishing/read paths | research lanes | one another’s `unified_finish.rs`, worker, render, and diagnostic files |
| G — evidence/docs | L1 documentation and tracker reconciliation | all | simultaneous edits to the same log/plan section |

Parallelism is research-only. There is still one Cargo slot; agents who are not holding it collect source inventories, design fixtures, write docs, and prepare commands.

## 3.3 Merge order

The numbers are sequencing, not a request to force unrelated work into one PR.

1. **PR-0 — programme scaffolding and clean-red baseline research**
   - W0 reproductions/report; no code change except a test isolation seam if it is behaviour-preserving.
2. **Checkpoint A — adaptive3d red verdicts**
   - operator approves code/test/retirement disposition per red.
3. **PR-1..3 — one adaptive3d resolution per PR**
   - one test/contract, red-first, no bundled refactor.
4. **PR-4 — findings transport structural adapter**
   - W1; report/wire parity only.
5. **PR-5 — arcfit evidence harness, no output change**
   - W2 baseline + fingerprints/census.
6. **Checkpoint F1 — arcfit run boundary**
   - approve semantic output/fingerprint re-pin package.
7. **PR-6 — arcfit intent-boundary fix**
   - one broad but isolated transform PR, then re-run all declared consumers.
8. **PR-7 — feeds census/explanation instrument**
   - W3 report-only, B3 reconciliation fixture, no Suggest/gate numbers move.
9. **Checkpoint B — feeds/Suggest physical-model decision**
   - approve shared-model scope, allowed policy changes, and any operator-visible numeric/default impact.
10. **PR-8..11 — feeds fixes in causal order**
    - formula/provenance parity;
    - Suggest projection/explanation;
    - gate/optimizer parity;
    - documentation/UI only after values are settled.
11. **PR-12 — adversarial 2D fixture library and campaign harnesses**
    - W4 research only; no consumer migration.
12. **Checkpoint C — 2D findings triage**
    - approve per-operation fix order and the `Shape` failure contract; shared offset primitive only if the consumer matrix supports it.
13. **PR-13 onward — one 2D operation/primitive policy per PR**
    - Pocket/cascade first if still defective; no global flatten change.
14. **PR-? — simulation issue report-only taxonomy**
    - W5; does not retune gates.
15. **PR-? — drill report/evidence fixes**
    - W6 wording/parity only until policy approval.
16. **Checkpoint D — diagnostic and drill policy**
    - separately rule issue severity/defaults/gate changes and drill threshold changes.
17. **PR-? — approved simulation/drill policy changes**
    - one gate/default/threshold concept at a time.
18. **PR-? — reference fixture generator and P7 harness split/archive**
    - W7 evidence/infrastructure, followed by rest-grid anomaly diagnosis only if independently ready.
19. **Checkpoint E — qualified fixture and re-measurement readiness**
    - decide whether B1/B2 can reopen; no v3/strategy verdict without approval.
20. **PR-? — finishing defect research and approvals**
    - screenshot exporter (low risk), then D-16.1 and D-16.2 in separate checkpoint-approved PRs; parameter/narration reads only after nonblocking contract proves safe.
21. **Checkpoint F2/F3 — finishing planner/default/read-path decisions**
    - F2 D-16.1 run-off; F3 Shallow stock-to-leave and any default/serialization impact.
22. **Final PR — docs, final regression pack, and end-of-programme live validation**.

## 3.4 Human checkpoints

### Checkpoint A — adaptive3d baseline recovery

**Decider:** operator.  
**Required evidence:** three deterministic reproductions, contract/bisect verdict, minimal fixture/render, and proposed `FIX_CODE`/`FIX_TEST`/`RETIRE_WITH_REASON` per red.  
**Decision:** approve the disposition.  
**No code/test retirement or behavioural adaptive3d fix before approval.**

### Checkpoint B — feeds/Suggest source of truth

**Decider:** operator.  
**Required evidence:** `FEEDS_CENSUS.md`, B3 reconciliation, parity matrix, source/provenance state machine, a list of values that would move, and a rendered/operator-facing diagnostic example.  
**Decision:** approve the shared model and any change to Suggest, gate, optimizer, RPM/feed/DOC number, warning, or default.  
**No feed number an operator acts on moves before approval.**

### Checkpoint C — 2D adversarial findings and failure contract

**Decider:** operator.  
**Required evidence:** per-operation findings ranked by reachability/severity; adversarial SVG/render gallery; time/memory/cancellation records; `Shape` panic reproduction and explicit failure alternatives; consumer rollout matrix.  
**Decision:** choose fix priority and whether failures are surfaced/retired/retried; approve each shared primitive policy.  
**No broad offset/flatten/default change before approval.**

### Checkpoint D — simulation diagnostic and drill policy

**Decider:** operator.  
**Required evidence:** issue-channel redesign, page-one triage sample, measurability matrix, all affected gate consumer census, drill gate-vs-evidence table, literature freshness/source changes, and before/after operator messages.  
**Decision:** approve severity, gate, warning, threshold, or default changes; report-only plumbing may proceed independently.  
**No numeric gate/threshold/severity/default moves before approval.**

### Checkpoint E — qualified reference fixture / B1-B2 re-opening

**Decider:** operator.  
**Required evidence:** reference fixture spec/generator, tessellation error proof, repeated-run bin study, source-normal quality oracle, P7 policy, and two-fixture strategy protocol.  
**Decision:** either keep B1/B2 closed/deferred or authorize named re-measurements.  
**No new “strategy wins” claim or v3 closure re-run without approval.**

### Checkpoint F1 — arcfit semantic boundary

**Decider:** operator.  
**Required evidence:** mixed-intent red fixture, complete impacted-fingerprint census, expected fitted-geometry deltas, and downstream intent-consumer audit.  
**Decision:** approve output move/re-pin scope.  
**No arcfit output change before approval.**

### Checkpoint F2 — band run-off repair

**Decider:** operator.  
**Required evidence:** D-16.1 reproduction on committed concave/band-run-off fixture, normal-domain render, common population, candidate behaviours, and collision/residual/runtime table.  
**Decision:** choose intended boundary semantics.  
**No planner geometry change before approval.**

### Checkpoint F3 — Shallow `stock_to_leave` and agent-read defaults

**Decider:** operator.  
**Required evidence:** D-16.2 red fixture, affected project/default/serde matrix, MidSteep/Shallow controls, expected output shifts, and read-path concurrency evidence if narration/params are included.  
**Decision:** approve the dial contract/default/back-compat and any blocking/read consistency trade-off.  
**No existing-project behaviour/default change before approval.**

### Checkpoint G — final live validation

**Decider:** operator.  
**Required evidence:** all approved behavioural waves are green, known-red baseline is zero or has an explicit new operator ruling, per-wave renders/evidence are linked, and a live MCP plan is ready.  
**Decision:** authorize release build and read-only live characterization; results may re-open a defect but do not retroactively alter a committed fixture gate without a new evidence wave.

---

# 4. Verification matrix

## 4.1 Every code PR

```bash
free -g
pgrep -af "carg[o]"
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Run the smallest relevant target first, then the affected crate. Do **not** run `cargo test` at workspace scope. A clean log reports the exact state of the three inherited adaptive3d tests until H0 resolves them; it must never merely say “known reds.”

## 4.2 Baseline targeted suites

| Work | First focused gates |
|---|---|
| H0 adaptive3d | named tests in `adaptive3d::path::tests` and `adaptive3d::tests`; use `ps -L` for the slow simulation target |
| H1 feeds | `literature_matrix`, `predicted_feed_gates_f035`, `chipload_advisory_disclosure_h4`, tapered-width parity, focused `feeds::` and `tool_load::` unit tests |
| H2 arcfit/worker | `arcfit::tests`, transform provenance fingerprints, dressup/span invariants, `capability_link_moves_safety`, viz worker/MCP escape-hatch tests |
| H3 2D | operation-specific tests plus `offset_polygon_degenerate_inputs_r1`, property/fuzz targets, and selected `param_sweep` family |
| M1 issues | `simulation_cut`/diagnostic adapter tests, rendered synthetic trace fixture, viz diagnostics tests |
| M2 drill | drill gate tests, drill-specific literature matrix, semantic role/annotation tests |
| M3 fixtures | common fixture smoke, reference evaluator/tessellation/repeatability targets |

## 4.3 Behavioural and fingerprint rules

- A broad generated-path PR includes focused before/after FNV fingerprints and a consumer census. Fingerprint equality is required only where output was meant to remain identical.
- A deliberate delta needs a table of move/cut/rapid/runtime and pointwise quality/collision metrics, plus source-level mechanism and render. No one metric crowns a winner.
- Use parameter sweeps by affected family, not as a blanket ritual:

```bash
cargo test -p rs_cam_core --test param_sweep <family-filter>
```

- Do not run a release build in an implementation wave. Criterion/performance changes record comparable debug characterization if necessary and defer release benchmarking to approved live-validation preparation.

## 4.4 Final read-only live validation

Only after Checkpoint G:

1. Confirm clean staging and leave `wanaka.toml` untouched.
2. Run `cargo build --release -p rs_cam_viz --bin rs_cam_gui` and wait for completion before MCP connection.
3. Load Wanaka read-only; restore only documented session overrides and never save.
4. Use tip-matched simulation resolution. Regenerate rest-dependent toolpaths after re-simulation.
5. For each approved behavioural batch: inspect params/status, generate, simulate, narrate only through the approved bounded path, capture simulation/toolpath renders, and record collisions at matched resolutions.
6. Confirm the final operator-facing feed/drill/issue wording and screenshot exporter output; compare fixed-path outputs, not a stale toolpath against fresh stock.
7. Append a factual live report; classify paths as `PASS`, `NOT EXERCISED`, `CONCERN`, or `FAIL` — never turn unexercised into pass.

---

# 5. Definition of done

The programme closes only when:

1. the three adaptive3d permanent reds have an approved, executed, and sentried disposition; no inherited red is hidden behind an exception;
2. `FEEDS_CENSUS.md` reconciles B3 and identifies one canonical physical model with explicit legitimate stage differences;
3. Suggest, gates, and optimizer cannot silently use divergent chipload/RPM/power/deflection physics for equivalent inputs;
4. all GUI-reachable 2D operation families have adversarial fixture coverage, bounded termination/cancellation evidence, and a ranked findings disposition;
5. a cascade/library failure cannot silently masquerade as a successful collapsed/empty operation;
6. simulation diagnostics present bounded typed findings and say when a requested metric is not measurable at the selected resolution;
7. drill verdicts agree with evidence and their cited threshold sources are fresh/traceable;
8. the GUI worker’s findings path is structural rather than field-by-field copy debt;
9. arcfit does not cross intent boundaries, and every affected fitted-path fingerprint has an evidence-backed disposition;
10. D-16.1, D-16.2, D-LV.1, B4, remaining clip visibility, and agent-read debt are either fixed with sentries or explicitly deferred to a named decider/condition;
11. a procedural reference fixture has qualified the bins and fixture classes used for any reopened strategy/quality question; B1/B2 remain closed unless Checkpoint E rules otherwise;
12. each ignored mega harness has a named split/archive/schedule policy and owner;
13. bounded scouting has delivered a ranked next-programme map without scope creep;
14. every behavioural output/default/operator-number change passed its checkpoint, focused gates, documented re-pins, and final read-only live validation;
15. `cargo fmt --check` and zero-warning workspace clippy pass, and `PROGRESS.md`, `FEATURE_CATALOG.md`, `AI_MACHINIST_ANALYSIS_REFERENCE.md`, `CREDITS.md`, tracker, and close-out accurately describe the resulting surface.

## Explicitly out of scope

This programme does not automatically implement every risk discovered by R8, replace the production reach model with the sampled model, reopen the v3 strategy closure, change B6 chipload advisory severity, or re-run historical Wanaka campaigns. Each requires the named evidence and decider above. New unrelated defects are logged with a reproduction and owner condition; they do not expand a wave in progress.
