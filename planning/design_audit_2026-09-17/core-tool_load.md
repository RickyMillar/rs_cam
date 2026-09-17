# Design and feature-debt audit — core-tool_load

Group: `crates/rs_cam_core/src/tool_load/` (cutting-load guardrails, gates,
verdict, optimiser). Owner: power session. This audit proposes structure and
shape only; no physics, constants, thresholds or gate numbers are touched.

### TLD-01 Three hand-rolled sample-filter loops instead of the canonical predicate
- kind: design
- pattern: duplicate helpers
- where: `crates/rs_cam_core/src/tool_load/chipload.rs:301-321` (`steady_state_samples_for_toolpath`), `crates/rs_cam_core/src/tool_load/power.rs:369-461`, `crates/rs_cam_core/src/tool_load/deflection.rs:208-263`
- evidence: `rg -n "let mut offered: usize = 0;" power.rs deflection.rs` hits both files; both then repeat `s.toolpath_id != toolpath_id`, `!s.is_cutting`, `s.engagement.radial_woc_fraction < 0.02`, an `any_arc_captured` flag, the `is_phantom_transit`/`is_configured_entry` split and an `entry_peak_*` tracker, independently. `chipload.rs`'s own `steady_state_samples_for_toolpath` repeats the `toolpath_id`/`is_cutting`/`radial_woc_fraction` triple a third way and adds its own feed-threshold test, never calling `locality::is_steady_state_for_gate`. Only the folded DOC lookup at `mod.rs:317-328`/`chipload.rs:530-538` calls the canonical predicate directly.
- proposal: Extract one `locality::walk_toolpath_samples(trace, toolpath_id, span_lookup) -> impl Iterator<Item=(usize,&Sample)>` that yields `(index, sample, Locality)` where `Locality` is `Steady | Entry | Phantom | OutOfScope`, doing the `toolpath_id`/`is_cutting`/`radial_woc_fraction`/phantom/entry classification once. All three gates fold over it instead of re-deriving offered/contributing/entry_peak bookkeeping.
- breaks: none (internal refactor; call sites keep identical outputs)
- effort: M
- risk: medium — the CLAUDE.md invariant ("never write a second copy") is already violated three times; a bug fixed in one copy (e.g. a future locality kind) needs three matching edits to stay in sync
- sentry: `cargo test -p rs_cam_core -q --test gate_population_vacuity_xvac`, `cargo test -p rs_cam_core -q --test predicted_feed_gates_f035`
- owner: power session

### TLD-02 Band-admit re-decision duplicated across delta.rs and narrative.rs, self-flagged drift risk
- kind: design
- pattern: duplicate helpers
- where: `crates/rs_cam_core/src/tool_load/optimize/delta.rs:189-215` (`chipload_within_breaches_strict`), `crates/rs_cam_core/src/tool_load/optimize/narrative.rs:692-726` (`limiting_gates_from_band_admit`)
- evidence: both call `.bounds.exceeds_high(...observed_mm_per_tooth, 0.0)` and `.bounds.below_low(...) == Some(true)` on the same `ChiploadVerdict::Within` shape. `narrative.rs:704-706` says outright: "these two comparisons are verbatim duplicates of `delta.rs`'s `chipload_within_breaches_strict` and must stay in step with it."
- proposal: Move the predicate onto `ChiploadVerdict` itself (e.g. `fn band_admit_breach(&self) -> Option<(ChipSide, &ChiploadMetric)>`) and have both `delta.rs` and `narrative.rs` call the one method instead of re-typing the two comparisons.
- breaks: none
- effort: S
- risk: low — both call sites already agree; consolidating just removes the manual-sync obligation the comment names
- sentry: `crates/rs_cam_core/src/tool_load/optimize/delta.rs` tests (`one_ulp_above_the_ceiling_does_not_demote_the_tier` and neighbours)
- owner: power session

### TLD-03 Two independent verdict-to-text flattenings; one drops to raw Debug output
- kind: design
- pattern: two representations, hand-written translation
- where: `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs:825-892` (`unmodeled_to_diagnostic`), `crates/rs_cam_core/src/gcode/mod.rs:668-742` (`enforce_load_policy`)
- evidence: `from_tool_load.rs` gives every `UnmodeledReason` variant a hand-written, operator-facing sentence (e.g. `"{label}: no vendor LUT row matches..."`). `gcode/mod.rs:723` instead does `crits.push(format!("chipload={reason:?}"))` — the export-refusal error text a CLI/GUI user sees for an unmodeled gate is the bare Rust `Debug` name (e.g. `chipload=SteadyStateSamplesNotPresent`), not a sentence. The Exceeds branch a few lines above (`report.exceeded_criteria()`) already uses a structured `ExceedsEntry{label, reason_label}` — only the Unmodeled branch regressed to `{reason:?}`.
- proposal: Give `UnmodeledReason` a `fn operator_message(&self, label: &str) -> String` (the text already written once in `unmodeled_to_diagnostic`) and have both `unmodeled_to_diagnostic` and `enforce_load_policy` call it, so there is one place that turns a reason into English.
- breaks: none (only error-string wording for unmodeled export refusals changes, from a Debug tag to a sentence)
- effort: S
- risk: low — text-only; no callers pattern-match the refusal message
- sentry: `crates/rs_cam_core/src/gcode/mod.rs` export-policy tests (search `enforce_load_policy` in that file's `#[cfg(test)]` module)
- owner: power session

### TLD-04 DeflectionSetupPrescription and DeflectionSetupDetail are one concept, hand-copied
- kind: design
- pattern: two representations, hand-written translation
- where: `crates/rs_cam_core/src/tool_load/optimize/refusal.rs:20-31` (`DeflectionSetupPrescription`), `crates/rs_cam_core/src/tool_load/optimize/narrative.rs:106-115` (`DeflectionSetupDetail`), `crates/rs_cam_core/src/tool_load/optimize/preflight.rs:84-90`
- evidence: both structs carry the identical `peak_um: f64`, `bound_um: f64`, `target_stickout_mm: f64` triple (`refusal.rs` adds a `text: String`). Both doc comments cross-reference each other as mirrors ("mirrors `refusal::DeflectionSetupPrescription` minus the prose" / "mirrored onto ... `DeflectionSetupDetail`"). `preflight.rs:84-90` builds one from the other field-by-field: `deflection_setup: Some(DeflectionSetupDetail { peak_um: prescription.peak_um, bound_um: prescription.bound_um, target_stickout_mm: prescription.target_stickout_mm })`.
- proposal: Delete `DeflectionSetupDetail` and have `narrative::OutcomeNarrative::deflection_setup` hold `refusal::DeflectionSetupPrescription` directly (or extract the shared three fields into one `DeflectionSetupNumbers` struct that both structs embed), removing the hand-copy at `preflight.rs:84-90`.
- breaks: `OutcomeNarrative` wire shape changes (the `deflection_setup` field's JSON shape gains/loses nothing numerically but comes from a renamed type) — MCP/GUI consumers reading this struct name would need the rename; operator ruling 2026-09-16 allows this without a compatibility shim.
- effort: S
- risk: low — three `f64` fields, already numerically identical; the type consolidation cannot change a value
- sentry: `crates/rs_cam_core/src/tool_load/optimize/orchestration_skip_tests.rs:357` (asserts on the structured `DeflectionSetupDetail`)
- owner: power session

### TLD-05 plunge_stress.rs is a fourth milling gate outside the gate machinery
- kind: design
- pattern: enum dispatch replicated in N places / inconsistent shape
- where: `crates/rs_cam_core/src/tool_load/plunge_stress.rs:1-80`, `crates/rs_cam_core/src/tool_load/mod.rs:17-26,444-493`, `crates/rs_cam_core/src/session/compute/diagnostics.rs:143-161`
- evidence: `tool_load/CLAUDE.md` lists `chipload.rs, power.rs, deflection.rs, plunge_stress.rs` as "the four milling gates", but `mod.rs`'s own module doc says "Three criteria" (line 3) and only `ChiploadGate`/`PowerGate`/`DeflectionGate` implement `MetricEvaluator` (`mod.rs:456-493`). `plunge_stress::check_plunge_stress` is not called from `evaluate_toolpath`, has no `Verdict` variant on `ToolpathLoadVerdict`/`ToolLoadReport`, and is invoked only from `session/compute/diagnostics.rs:159` via an ad-hoc `Vec<(String, f64, f64)>` offenders list — it does not go through `gcode::enforce_load_policy`, so it cannot block export the way the other three gates do.
- proposal: Either (a) fold `plunge_stress` into the `MetricEvaluator` trait as a fourth gate with its own `Verdict` and a `ToolpathLoadVerdict::plunge_stress` field so it gets the same report/diagnostics/export treatment as the other three, or (b) update `tool_load/CLAUDE.md` to stop calling it a "milling gate" and document it as the standalone diagnostics-only check it actually is. Either fixes the doc/code mismatch; (a) also closes the export-gate gap.
- breaks: (a) adds a field to `ToolpathLoadVerdict`/`ToolLoadReport` (wire-visible; no legacy-compat shim needed per 2026-09-16 ruling); (b) breaks nothing
- effort: M (a) / S (b)
- risk: medium — plunge stress currently cannot refuse export, so an operator's project can export G-code with a plunge rate flagged only in a diagnostics list, not in the same policy path chipload/power/deflection use
- sentry: none today; would need a new one alongside `gate_population_vacuity_xvac` if (a) is taken
- owner: power session

### TLD-06 Verdict fixture builders (`within_chipload`/`within_power`/`within_deflection`) reimplemented in five test modules
- kind: design
- pattern: duplicate helpers
- where: `crates/rs_cam_core/src/tool_load/optimize/delta.rs:398-430`, `.../rank.rs:140-185`, `.../narrative.rs:992-1070`, `.../strategy/retarget.rs:281-330`, `.../strategy/headroom.rs:234-265`
- evidence: `rg -n "fn (within_chipload|chipload_within|within_power|power_within|within_deflection|deflection_within|exceeds_chipload|exceeds_power|exceeds_deflection)\("` across `optimize/` returns 15 near-identical function definitions spread across the 5 files above, each independently constructing a `ChiploadVerdict::Within { .. }` / `PowerVerdict::{Within,Exceeds}` / `DeflectionVerdict::{Within,Exceeds}` literal.
- proposal: Add a `#[cfg(test)] pub(crate) mod verdict_fixtures` (e.g. under `optimize/mod.rs` or a new `optimize/test_support.rs`) with one `within_chipload`/`exceeds_chipload`/`within_power`/`exceeds_power`/`within_deflection`/`exceeds_deflection` set; have the five test modules `use` it instead of redefining.
- breaks: none — test-only code
- effort: S
- risk: low — mechanical de-duplication of test scaffolding, no production path touched
- sentry: existing unit tests in each of the five files continue to pass unchanged (they'd call the shared builders with the same signatures)
- owner: power session

### TLD-07 AxisPolicy (12 fields) and RankingPolicy (11 fields) exceed the 8-field parameter guideline
- kind: design
- pattern: parameter struct > 8 fields
- where: `crates/rs_cam_core/src/tool_load/optimize/policy.rs:52-65` (`AxisPolicy`, 12 fields incl. one `PolicyValue<bool>`), `crates/rs_cam_core/src/tool_load/optimize/policy.rs:92-134` (`RankingPolicy`, 11 fields)
- evidence: field count by direct read of the struct bodies (12 and 11 `pub` fields respectively, each wrapped in `PolicyValue<T>`).
- proposal: Low priority — these are declarative provenance tables (each field independently carries a `rationale`/`source`), not procedural argument bags assembled positionally at call sites, and the brief's rule targets the latter. If ever split, group by sub-concern already implied by the doc comments (`AxisPolicy`: candidate-spacing vs. outside-preferred-band fields; `RankingPolicy`: gate-tolerance fields vs. composite-score-weight fields) rather than an arbitrary cut.
- breaks: none proposed
- effort: L (if ever done) — not recommended now
- risk: low — noted for completeness; no action requested given the deliberate provenance-table design
- sentry: n/a
- owner: power session

### TLD-08 `wanaka_e2e_chipload_gate` end-to-end test permanently `#[ignore]`d on open follow-ups
- kind: feature-debt
- pattern: ignored harness
- where: `crates/rs_cam_core/tests/wanaka_e2e_chipload_gate.rs:52`
- evidence: `#[ignore = "blocked on O3 helix-descent + O4 burn-risk edge filter"]` — an e2e test of the chipload gate against a real (wanaka) project is disabled pending two named, still-open follow-ups (O3, O4).
- proposal: Not a code fix for this audit (no physics/gate changes proposed), but the follow-up items O3/O4 should get an owner and a target, or the test should state in one line why it is safe to leave disabled indefinitely. Flagging so it is not silently forgotten as "just another ignored test."
- breaks: none
- effort: S (to triage/schedule; not to fix the underlying O3/O4 work)
- risk: low — the test being off does not corrupt anything, but it means the chipload gate's most realistic e2e check has been dark since it was written
- sentry: the test itself, once un-ignored
- owner: power session

### TLD-09 `optimize_toolpath_inner` is a 296-line god function with its own numbered seams
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/tool_load/optimize/mod.rs:212-508` (`fn optimize_toolpath_inner`)
- evidence: the function body is already divided by ten `// N. <step>` comments (`// 1. Build the evaluation context...` at line 221 through `// 10. Drop the guard explicitly...` at line 475) covering baseline-candidate construction, LUT lookup, pre-flight, cancellation, Stage F closed-form retarget, axis-grid strategy, and Stage-2 refinement. `awk` line-count over `mod.rs` puts this single function at 296 lines, next-largest in the file is 149 (`run_grid_strategy`).
- proposal: Split along the existing numbered seams into named helpers (e.g. `build_baseline_candidate`, `run_preflight_or_continue`, `run_stage_f`, `run_axis_grid_and_stage2`), each taking the small subset of `ctx`/`baseline_*` values it needs. The numbering already documents the intended boundaries; extraction is close to mechanical.
- breaks: none (private function, no signature visible outside the module)
- effort: M
- risk: low — behaviour-preserving extraction; the existing optimizer test suite (`tests.rs`, `stage1_grid_tests.rs`, `orchestration_skip_tests.rs`, `project_rollup_tests.rs`) already exercises every numbered step
- sentry: `crates/rs_cam_core/src/tool_load/optimize/tests.rs`, `.../stage1_grid_tests.rs`, `.../orchestration_skip_tests.rs`
- owner: power session

### TLD-10 `GatePopulation::filtered_out` is pub with no production caller
- kind: design
- pattern: pub item used only from tests
- where: `crates/rs_cam_core/src/tool_load/verdict.rs:693-698`
- evidence: `rg -n "filtered_out" crates/rs_cam_core/src` (from the crate root, covering both `src/` and `tests/`) shows the definition and exactly one call site, `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs:284`. No `src/` caller. The method's own doc comment already names itself a "Test door."
- proposal: Low-priority; the method is small, cheap, and explicitly self-documented as a test door, so leaving it `pub` is defensible. If tidying: move it behind `#[cfg(any(test, feature = "test-support"))]` so the crate's public surface doesn't imply a production consumer that doesn't exist.
- breaks: would break the sentry test's import path unless the feature/cfg is enabled for `cargo test`
- effort: S
- risk: low
- sentry: `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs:284`
- owner: power session

## Top three
1. **TLD-01** (three hand-rolled sample-filter loops) — the CLAUDE.md invariant is already violated three separate ways; one shared walk helper removes the "did I copy this right" risk from every future gate change, and touches only `tool_load/`.
2. **TLD-09** (296-line `optimize_toolpath_inner`) — the ten numbered comments are a ready-made extraction plan; splitting it is close to mechanical and immediately makes the Stage-F / axis-grid / Stage-2 boundaries testable in isolation.
3. **TLD-03** (Debug-formatted export refusal text) — a one-function fix (`UnmodeledReason::operator_message`) that replaces a raw enum-name string shown to operators on G-code export refusal with the sentence the diagnostics adapter already writes.

## Checked and clear
- `feeds::predict::tip_deflection_from_engagement` vs. `tool_load::deflection::sample_tip_deflection_mm` (flagged 0.90/0.89 by the dup sweep) is **not** a duplicate — `deflection.rs:96-108` explicitly calls the shared `feeds::predict` function; the textual similarity is two call sites documenting the same canonical model, which is the intended design (single source of truth for chip-thinning/force physics shared between Suggest and the post-sim gate).
- `optimize::mod::RetargetStageOutput` vs. `optimize::strategy::retarget::RetargetStrategyOutput` (flagged 0.9153) is a deliberate, self-documented pair — one holds un-simulated `CandidatePatch`, the other holds simulated `OptimizeCandidate`; `mod.rs:750-754` explains the distinction. Not a fix target.
- `optimize::retarget::power` vs. `optimize::strategy::retarget` test modules (flagged 0.8820) duplicate test-scaffolding (`Env`, `make_pocket_with_feed`) rather than production logic; lower priority than TLD-06's verdict-fixture duplication and left out of that finding to keep it focused.
- `dead_pub_surface.md`'s listing of `PlungeStressWarning` as dead is a false positive from the census's own limits: its fields are read at `session/compute/diagnostics.rs:159-160` (`w.plunge_rate_mm_min`, `w.safe_cap_mm_min`); the struct is reached via `if let Some(w) = check_plunge_stress(...)`, which the census's textual scan apparently missed.
- `PowerGate`/`ChiploadGate`/`DeflectionGate`'s shared `MetricEvaluator` trait (`mod.rs:444-493`) already gives the three milling gates one shape and one refusal-semantics sentry (`metric_evaluators_share_refusal_semantics`); this part of the "one gate trait or N hand-shaped functions" question is already resolved well. The remaining gap is `plunge_stress.rs` sitting outside it (TLD-05) and the sample-filtering duplication feeding into each gate's own body (TLD-01).
- The `boundary::exceeds_high`/`below_low` epsilon contract (Checkpoint K) is followed everywhere I checked, including the optimizer's own re-decision sites (`delta.rs:229,239`, `retarget/chipload.rs:288`) — `outcome.rs:226-228`'s doc comment claiming delta.rs/narrative.rs "still use bare comparisons" (citing an A-8 census) reads as stale against the current code; both now route through `ChipBounds`/`boundary::` methods (verified by reading `delta.rs:189-215` and `narrative.rs:692-726`, and by the passing `one_ulp_above_the_ceiling_does_not_demote_the_tier` sentry in `delta.rs`). Not filed as a finding since no code change is proposed, but noted so nobody re-reads that comment as current.

## Add-a-thing count
Adding a fifth milling gate (fitting the existing `MetricEvaluator` shape used by chipload/power/deflection) touches at least:
1. `tool_load/<new_gate>.rs` — the gate module (`evaluate(ctx, env) -> Verdict`, `as_criterion_status()`).
2. `tool_load/verdict.rs` — new `Verdict` enum, `CriterionStatus` projection, and a field on `ToolpathLoadVerdict`.
3. `tool_load/mod.rs` — `pub mod` line, a `MetricEvaluator` marker struct + impl, the field assembly inside `evaluate_toolpath`.
4. `tool_load/optimize/delta.rs` — a `GateDelta` field on `GateDeltas` plus its comparison arm.
5. `tool_load/optimize/narrative.rs` — a `GateKind` variant, an arm in `limiting_gates_from_exceeds`, and (if the gate ever band-admits) an arm in `limiting_gates_from_band_admit`.
6. `tool_load/optimize/outcome.rs` — any new candidate-sim-assumption or report field the gate needs recorded.
7. `tool_load/optimize/retarget/<new_gate>.rs` plus a wiring change in `tool_load/optimize/retarget/mod.rs` and `tool_load/optimize/strategy/retarget.rs` — the closed-form retargeter, if the gate should participate in Stage F.
8. `tool_load/optimize/policy.rs` — a new tolerance/headroom field, if the gate needs one.
9. `diagnostics/adapters/from_tool_load.rs` — a new `*_to_diagnostic` function and a call to it from `diagnostics_from_load_verdict`.
10. `gcode/mod.rs` — a new branch inside `enforce_load_policy`'s Exceeds/Unmodeled string-building (see TLD-03).
11. `tool_load/CLAUDE.md` — update the file map and invariants.

That is 9-11 files for a gate that wants full parity with chipload/power/deflection (fewer if it skips the optimizer's retarget stage). `plunge_stress.rs` is the standing counter-example (TLD-05): a gate that skipped essentially all of this list and stayed a diagnostics-only side path.
