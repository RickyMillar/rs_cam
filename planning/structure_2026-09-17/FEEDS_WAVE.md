# FEEDS_WAVE — the clean-up the power-calcs freeze held back

`crates/rs_cam_core/src/feeds/**` and `crates/rs_cam_core/src/tool_load/**`.

## 1. Method, and the state of the tree

The tech-debt programme of 2026-09-16 excluded both folders because another
agent owned them. The operator lifted that freeze on 2026-09-17. This file
triages every hand-off row against the code as it stands today.

For each candidate this triage opens the current source, then counts callers
with `rg -nw` across `crates/`, `scripts/` and `crates/rs_cam_core/tests/`. A
doc-comment mention is not a caller. Every own-file caller is compared against
the file's `#[cfg(test)]` line, so a production caller is told apart from a
test caller.

Every direct submodule of both folders is a `pub mod`, and `lib.rs:49` and
`lib.rs:101` declare `pub mod feeds;` and `pub mod tool_load;`. Therefore
`dead_code` never fires on a `pub` item in those files. **The rule stops at
`tool_load/optimize/`.** That folder declares several submodules privately —
`optimize/mod.rs:49` says `mod policy;` — so a `pub` item inside them is
already crate-effective, and `dead_code` does fire there.

**Git-status check for the two folders.** At the start of this triage, at
commit `17fc8ac3`, `git status --short crates/rs_cam_core/src/feeds
crates/rs_cam_core/src/tool_load` printed nothing. Both folders were clean.
The S33 hand-off note said the two `depth.rs` rows were "live only through
uncommitted work — do not delete". That uncommitted work no longer exists as
uncommitted work: it landed as commit `70989b97`, and
`crates/rs_cam_core/tests/the_written_depth_is_one_the_machine_cuts_g_stair.rs`
is now a tracked file. S33 is therefore a plain false positive. See §3.

**WARNING. The tree moves under this file. Read this before you start.**
The structure agent holds the cargo lane and moves modules under
`crates/rs_cam_core/src/`. During this triage the branch head advanced from
`17fc8ac3` to `098f0f36` ("P2: maps/ — move 11 modules"), and
`crates/rs_cam_core/src/simulation_cut.rs` became
`crates/rs_cam_core/src/stock/simulation_cut.rs`. A second check then showed
10 modified files inside `tool_load/`, all `use`-line rewrites. Therefore:

- Start this wave only after the mover's work lands.
- Treat every line number below as a number at `17fc8ac3`..`098f0f36`.
  Re-locate each item by symbol name, not by line.
- The two folders do not move. Only their `use` lines change.

## 2. Ranked findings

Rubric: A live defect > B workspace-rule violation > C dead
load-bearing-looking code > D drift-prone duplication > E too-wide visibility
> F size and lint.

**Count. A 0, B 4, C 7, D 0, E 4, F 8.** The deletions remove about 123 lines:
21 for `enumerate_matching_rows`, 28 for `load_dir`, 23 for `evaluate_project`
and `active_axes`, about 15 for `render_label` with its test, about 9 for the
`stock_to_leave_radial` residue, 13 inert attributes and about 14 stale doc
lines. The visibility rows remove nothing.

**Nothing here is tier A.** No row in either folder changes machined output,
loses data or misleads the operator at run time.

| rank | id | tier | size | claim | file:line | fix | proof |
|---|---|---|---|---|---|---|---|
| 1 | FW-02 | B | S | Six production `#[allow(clippy::too_many_arguments)]` carry no `SAFETY:` comment. `:895` carries a reason, but not the `SAFETY:` prefix the workspace rule needs. The other five carry nothing. | `feeds/suggest.rs:895`, `:1047`, `:1075`, `:1103`, `:1219`, `:1263` | Put one `// SAFETY:` line above each allow. Name the arguments the funnel needs. | `cargo clippy -p rs_cam_core --all-targets -- -D warnings`. |
| 2 | FW-03 | B | S | Two production `#[allow(clippy::too_many_arguments)]` carry no comment at all. | `tool_load/optimize/mod.rs:570` (`run_headroom_strategy`), `:648` (`run_retarget_strategy`) | Put one `// SAFETY:` line above each allow. | Same clippy run. |
| 3 | FW-04 | B | S | A production cast allow carries a reason, but not the `SAFETY:` prefix. | `tool_load/optimize/patches.rs:83` | Rewrite the comment with the `SAFETY:` prefix. Keep the reason. | Same clippy run. |
| 4 | FW-05 | B | S | Two production `#[allow(clippy::indexing_slicing)]` carry the reason "best.i stored from this same iteration", but not the `SAFETY:` prefix. | `feeds/vendor_lookup.rs:188`, `:611` | Rewrite both comments with the `SAFETY:` prefix. | Same clippy run. |
| 5 | FW-06 | C | S | S7 / T-1. `enumerate_matching_rows` is dead. `rg -nw` finds the definition and one historical comment. The comment is in the other folder. 21 lines. | `feeds/vendor_lookup.rs:315`; stale comment at `tool_load/optimize/context.rs:139` | Delete the function. Delete the comment clause that names it. | The vendor_lookup set in §6. |
| 6 | FW-07 | C | S | S10. `load_dir` has zero references anywhere. 28 lines. | `feeds/vendor_lut.rs:398` | Delete the method. | The vendor_lut set in §6. |
| 7 | FW-08 | C | S | S22. `evaluate_project` and `active_axes` have zero references anywhere. 23 lines. | `tool_load/mod.rs:567`; `tool_load/optimize/axes.rs:228` | Delete both. | The optimize set in §6. |
| 8 | FW-09 | C | S | A banner comment states the opposite of the build. It says the typed verdict scaffolding has "No consumers". `LoadState` has 35 references outside its file, `ChiploadVerdict` has 181 and `ChipBounds` has 58. The GUI reads all three. | `tool_load/verdict.rs:525-527`; the same claim at `:999` | Delete the "No consumers read these yet" sentence. At `:999` state the current shape: the typed verdict is what every gate returns today. | The verdict set in §6. |
| 9 | FW-10 | C | S | The `stock_to_leave_radial` residue L2 left behind. Two `match` arms accept three param strings. No producer emits any of them: the only producers emit `"entry_style"` and `"clearing_strategy"`. Two docs name the dead string. | `feeds/rationale.rs:360`, `:481`; docs `feeds/rationale.rs:76`, `feeds/suggest.rs:413` | Delete both arms. The `_` sentinel arm already covers them. Correct both docs. **Keep `RationaleParam::StockToLeave`** — `feeds/rationale.rs:461` still produces it. | The rationale tests inside `feeds/rationale.rs` (`:790`, `:810`, `:822`). |
| 10 | FW-11 | C | S | `render_label` is dead in production. Its only caller is its own test at `:338`, below the `#[cfg(test)]` line at `:284`. A doc at `:194` names it. About 15 lines with the test. | `feeds/quantities.rs:275` | Delete the method, its test and the doc clause at `:194`. The 2026-09-16 ruling covers this: delete dead code outright and state the break in the commit body. | `cargo test -p rs_cam_core --lib feeds::quantities`. |
| 11 | FW-12 | C | S | `detect_conflicting_rows` is `pub` and dead in production. Its only caller is its own test at `:800`, below the `#[cfg(test)]` line at `:600`. A same-file test reaches a private item, so no external-crate constraint applies. | `feeds/vendor_lut.rs:551` | Demote to private `fn`. Do not delete: the test is a real LUT-conflict check. | `cargo test -p rs_cam_core --lib feeds::vendor_lut`. |
| 12 | FW-13 | E | M | S29 for `feeds/`. Eight `pub` items have callers only inside their own file, all in production code, none in any test. | `feeds/explain_payload.rs:36` `from_machine`; `feeds/mod.rs:1154` `POWER_LADDER_AP_FLOOR_MM`, `:1158` `POWER_LADDER_AE_FLOOR_MM`; `feeds/predict.rs:79` `COLLET_EXPOSURE_MARGIN_MM`; `feeds/provenance.rs:76` `edge_radius_floor`, `:95` `from_chipload_source`; `feeds/rationale.rs:56` `LEGACY_ESTIMATE_NOTE`; `feeds/vendor_lut.rs:441` `MIN_CHIPLOAD_RANGE_FRACTION` | Demote each to `pub(crate)`. Build. Demote again to private where the compiler accepts it. | `cargo build -p rs_cam_core --all-targets`. The compiler is the proof. |
| 13 | FW-14 | E | M | S29 for `tool_load/`. Six `pub` items have callers only inside their own file, all in production code, none in any test. | `tool_load/locality.rs:54` `first_span_of_kind`; `tool_load/optimize/bounds.rs:259` `resolve_rpm_bounds`; `tool_load/optimize/outcome.rs:498` `marginal_safe`, `:517` `trade_off`, `:649` `first_marginal_safe_index`; `tool_load/verdict.rs:345` `milling_criteria` | Demote each to `pub(crate)`. Build. All six are functions or methods, so no `private_interfaces` risk applies. | `cargo build -p rs_cam_core --all-targets`. |
| 14 | FW-15 | E | S | One coupled visibility group. `SearchPolicy` holds the five policy structs as `pub` fields. `SearchPolicy` has 69 in-crate references and no test code reference — the two test mentions are doc comments, and `strategy/retarget.rs:159` is inside a `#[cfg(test)] mod tests`. `optimize/mod.rs:49` declares `mod policy;` privately, so all six are already crate-effective and the demotion carries no `private_interfaces` risk. | `tool_load/optimize/policy.rs:33` `SearchPolicy`, `:45` `AxesPolicy`, `:68` `FeedPolicy`, `:79` `RetargetPolicy`, `:137` `StagePolicy`, `:144` `FallbackPolicy` | Demote all six together to `pub(crate)`. Demote the container with the fields, or demote none: a `pub(crate)` field type inside a `pub` struct breaks `-D warnings` through `private_interfaces`. | `cargo build -p rs_cam_core --all-targets` and `cargo test -p rs_cam_core --test generator_extremes_fuzz_r1`. |
| 15 | FW-16 | E | S | S25. Five `pub` items have callers only in `crates/*/tests`, and no doc line declares them a fixture. `crates/*/tests` is an external crate, so `pub` must stay. | `feeds/feed_explanation.rs:266` `unit_family`, `:390` `predicted_gate_observation_mm`; `feeds/suggest.rs:1156` `write_to`, `:1264` `preview_field_apply`; `tool_load/verdict.rs:695` `filtered_out` | Add one doc line to each: `Test door: <test file>`. Change no visibility. | The feed_explanation, suggest and verdict sets in §6. |
| 16 | FW-17 | F | S | Q4. A redundant import. Line 10 already imports `OperationParams` by name, so the `as _` import at `:12` does nothing. The `#[allow(unused_imports)]` proves it: the compiler finds no use. | `tool_load/optimize/patches.rs:11-12` | Delete both lines. If rustc then reports line 10's `OperationParams` unused, no trait method is called and the name goes from line 10 too. | `cargo clippy -p rs_cam_core --all-targets -- -D warnings`. |
| 17 | FW-18 | F | S | S27. Nine `#[allow(dead_code)]` sit on `pub` serde fields of `pub struct VendorObservation`, inside `pub mod vendor_lut`, inside `pub mod feeds`, inside a library crate. `dead_code` cannot fire there. The attributes claim a live wire surface is dead. | `feeds/vendor_lut.rs:155`, `:157`, `:159`, `:168`, `:193`, `:216`, `:218`, `:220`, `:225` | Delete all nine attributes. | `cargo clippy -p rs_cam_core --all-targets -- -D warnings`. If one is load-bearing, the build says so at once. |
| 18 | FW-19 | F | S | An inert allow on an enum variant of a `pub` enum. The same argument as FW-18. | `feeds/provenance.rs:289` | Delete the attribute. Keep the "symmetry" reason as a plain doc line. | Same clippy run. |
| 19 | FW-20 | F | S | An inert allow. `clippy::struct_field_names` is a pedantic lint. The root `Cargo.toml` enables no pedantic group, and `crates/rs_cam_core/src/lib.rs` carries no `warn(clippy::pedantic)`. | `tool_load/mod.rs:416` | Delete the attribute. | Same clippy run. |
| 20 | FW-21 | F | S | Eight comments name three functions that no longer exist. A reader who greps `run_stage_0`, `run_stage_1_grid` or `run_stage_f_retarget` finds only the prose. The no-legacy ruling deletes this history. | `tool_load/optimize/mod.rs:429`, `:567`, `:626`, `:772`, `:2889`; `optimize/strategy/retarget.rs:3`; `optimize/strategy/grid.rs:2`; `optimize/strategy/headroom.rs:4`; `optimize/candidate.rs:98` | Trim the "Replaces the legacy `X`" clause from each doc. Keep the behaviour note at `optimize/mod.rs:628`. | Comments only. `cargo build -p rs_cam_core`. |
| 21 | FW-22 | F | L | Q11 / T-5. Long functions. `feeds/mod.rs:1192 calculate` is 1302 lines, `feeds/suggest.rs:72 target_chipload` is 621, `tool_load/chipload.rs:441 evaluate_inner` is 660, `tool_load/optimize/policy.rs:151 default` is 506, `feeds/rationale.rs:191 entry_for_warning` is 371, `tool_load/power.rs:261 evaluate` is 322. | as listed | **A natural seam exists in `calculate` only.** It already carries `// --- Step 1: RPM ---`, `// --- Step 2: Chip load ---`, `// --- Step 2b: Spindle-speedup ---` and further step banners. Each step is a candidate private function. The other five carry no such banner; splitting them is a judgement, not a move. Low priority. Do not schedule it in this wave. | not scheduled |
| 22 | FW-01 | F | S | A test assert message names a file that does not exist. The text says "add an entry to `examples/migrate_ap_rule.rs`". `crates/rs_cam_core/examples/` does not exist. The real file is `crates/rs_cam_cli/examples/migrate_ap_rule.rs`. The assert at `:704` and the doc at `:672` both sit below the `#[cfg(test)]` line at `:600`, so a developer reads this text, not the operator. The production doc at `:207` names the binary without a path and is correct. | `feeds/vendor_lut.rs:704`, doc `:672` | Write the real path `crates/rs_cam_cli/examples/migrate_ap_rule.rs` in both places. | `cargo test -p rs_cam_core --lib feeds::vendor_lut`. |
| 23 | FW-23 | F | M | Three test-only adapters exist only to keep "legacy positional-arg test calls" compiling against the Phase 6 `(ctx, env)` gate signature. All three are the same shape and sit in `#[cfg(test)] mod tests`. | `tool_load/chipload.rs:1119`, `tool_load/deflection.rs:382`, `tool_load/power.rs:601` | Optional. Inline each adapter into its callers, then delete it. The value is low and the call sites are many. Take it only if the chipload / deflection / power sets in §6 are green already. | The chipload, deflection and power `--lib` tests. |

## 3. False positives

| id | claim | reason it is a false positive |
|---|---|---|
| Q3 | `classify_3d_terrain` is unreachable from every shipped surface. | Q1 fixed it. Commit `f4d1a9dc` made both add-toolpath doors pass `SuggestContext::model_bbox` (`viz/controller/events/toolpath.rs:126`, `viz/app/mcp/commands.rs:868`). `feeds/suggest.rs:2553` and `:2641` call `geometry_class::classify(op_type, context.model_bbox)` for 3D op types, so `feeds/geometry_class.rs:84` reaches `:89`. The Q1 remainder stands: eight Suggest sites still pass `SuggestContext::default()`, so the branch is reachable from the two fixed doors only. Keep the function. |
| S33 | `realised_step_down` and `roughing_pass_count` are live only through uncommitted work. | The work is committed. `feeds/suggest.rs:937` calls `crate::depth::realised_step_down` in production, and the test file is tracked at commit `70989b97`. Both stay `pub`. `roughing_pass_count` needs a `Test door:` line if a later wave touches `depth.rs`. **`crates/rs_cam_core/src/depth.rs` is outside both folders and inside the mover's territory. Assign it to neither fix agent.** |
| S27 | The nine `vendor_lut` `#[allow(dead_code)]` mark dead fields. | The fields are live serde provenance on a `pub` struct. The allows are inert. The verdict "dead" is wrong; the attributes still go. Scheduled as FW-18. |
| L16 | `LEGACY_DEGENERATE_RANGE_ROWS` is legacy residue. | It is a shrink-only data allowlist with its own guard. `feeds/vendor_lut.rs:786-790` fails loudly when a listed row stops violating any rule, so the list cannot grow stale silently. This is a healthy pattern. Keep it. Fixing the listed data rows is a LUT data task, not a code wave. |
| D-narrative | `optimize/narrative.rs:105-116` duplicates `optimize/refusal.rs:20-30` at 0.93. | A documented sibling pair. `narrative.rs:107` says the struct "mirrors `refusal::DeflectionSetupPrescription` minus the prose", and `refusal.rs:17` names the mirror from the other side. The wire type carries no `text` field on purpose. |
| D-retargetout | `optimize/strategy/retarget.rs:50-64` duplicates `optimize/mod.rs:758-768`. | A documented sibling pair. `optimize/mod.rs:761` says `RetargetStageOutput` is "Distinct from `strategy::retarget::RetargetStrategyOutput`, whose candidates are un-simulated patch lists". One holds patches, one holds evaluated candidates. |
| D-retargetpower | `optimize/retarget/power.rs:139-238` duplicates `optimize/strategy/retarget.rs:133-232` at 0.88 over 100 lines. | The matched region is the `#[cfg(test)] mod tests` preamble: the same allow block and the same `use` list. The production code above it differs. The instrument matched boilerplate. |
| D-rankdelta | `optimize/rank.rs:117-216` duplicates `optimize/delta.rs:350-449` at 0.88. | The same reason. Both regions start at the `#[cfg(test)]` allow block. |
| D-lutnormalize | `vendor_lut.rs:228-247` duplicates `vendor_normalize.rs:125-141`. | An exhaustive enum-to-enum mapping. `vendor_normalize.rs:126` already declares `op_family_to_lut` the canonical mapping and says why it was extracted. The instrument matched the variant names on both sides. |
| D-suggestcatalog | `feeds/suggest.rs:1641-1658` duplicates `compute/catalog.rs:2584-2673`. | The caller-to-callee delegation class the D findings already ruled a false positive. `operation_feeds_hints` is a documented thin tuple adapter over `OperationConfig::feeds_hints`. `catalog.rs` also sits in the mover's path. |
| in_transit | `stock/simulation_cut.rs:1370` and `:1453` duplicate `locality::is_steady_state_for_gate`. | They are accumulators, not gates. No `SpanLookup` is available there, so the coarse `in_transit_span` flag is the only signal, and both sites document that. All three gates use the canonical predicate: `tool_load/chipload.rs:537`, `:817`, `:820`; `tool_load/power.rs:447`, `:450`; `tool_load/deflection.rs:249`, `:252`. There is no second copy to merge. |
| print_stdout | `tool_load/optimize/retarget_reconciliation_a8.rs:52` and `optimize/rank.rs:124` allow `clippy::print_stdout` against the root rule. | Both sit in `#[cfg(test)]` modules. `optimize/mod.rs:59` gates the A-8 module with `#[cfg(test)]`. Nine other core files use the same pattern (`rest_field.rs`, `adaptive3d/mod.rs`, `crest_lines.rs`, `finish_planner.rs`, `pocket.rs`, `region_mask.rs`, `compute/execute.rs`). This is a repo-wide convention, not a feeds or tool_load defect. See §5 for the ruling. |
| viz-feeds | `viz/ui/feeds/compare.rs:148`, `:258` and `viz/ui/feeds/shared.rs:45` carry allows without `SAFETY:`. | A different crate and a different folder. `rs_cam_viz/src/ui/feeds/` is GUI code that the freeze never covered. Out of scope. The UI review account owns it. |
| S29-blocked | Four more `pub` types have own-file-only callers, so S29 lists them as over-exposed: `feeds/mod.rs:619` `FormulaBreakdown`, `feeds/profile.rs:47` `Predictions`, `:66` `ConstraintEnvelopes`, `tool_load/plunge_stress.rs:44` `PlungeStressWarning`. | `private_interfaces` blocks every one, and the workspace builds with `-D warnings`. Each type is exposed by a `pub` surface whose own callers are external: `FormulaBreakdown` is `pub formula` on `pub struct FeedsDerates` (`feeds/mod.rs:530`); `ConstraintEnvelopes` and `Predictions` are `pub constraints` and `pub predictions` on the profile result (`feeds/profile.rs:134`, `:137`); `PlungeStressWarning` is the return type of `pub fn check_plunge_stress` (`tool_load/plunge_stress.rs:53`). Leave all four `pub`. The `SearchPolicy` group is the one coupled set that does demote — see FW-15. |

## 4. Physics, not this wave

A change to a formula needs the slow `--test` integration sims, not `--lib`.
These rows stay closed until someone schedules that run.

| id | claim | why it waits |
|---|---|---|
| FW-P1 | `feeds/efficiency.rs:309-351` duplicates `feeds/force.rs:271-296` at 0.92. | Both regions are deflection-cap and specific-energy arithmetic. `force.rs:272` and `efficiency.rs:310` each document their own assembly. A merge changes a formula. |
| FW-P2 | D15. Core publishes four types over the same two numbers: `feed_modulation::ChiploadBand:117`, `feeds::ChiploadBounds` (`feeds/mod.rs:433`), `feeds::quantities::VendorChiploadBand:203`, `tool_load::verdict::ChipBounds:927`. | The D findings call this a SIBLING set and route it as an API-surface question, not a cleanup. A standing operator rule blocks a naive merge: Suggest's `ChiploadBounds` must mirror the post-sim gate's piecewise-linear DOC derating, and the canonical scale lives in `feeds::geometry`. Any merge that splits them is a defect. Needs a ruling before any code moves. |
| FW-P3 | The pre-existing red `adaptive_feed_modulation_pipeline_f036b::modulation_raises_cutting_chipload_toward_band`. | **No finding in this wave explains it, and the attribution looks wrong.** `git show --stat e2ecf697` lists seven files, all in `rs_cam_viz`: `ui/components/compare.rs`, `ui/components/provenance.rs`, `ui/feeds/compare.rs`, `ui/feeds/why.rs`, `ui/properties/mod.rs`, `ui/toolpath_panel.rs` and one viz test. That commit touches no core file, so it cannot have changed a core physics result. Re-bisect before anyone treats the red as a physics regression. Do not schedule a fix here. |

## 5. Rulings needed

1. **FW-P2 / D15.** Four public chipload-band types. Merge, or keep as a
   declared sibling set with one doc that names the four and the reason for
   each? A merge is physics work.
2. **`print_stdout` in `#[cfg(test)]` modules.** The root `CLAUDE.md` says
   `println!` stays denied unless a test has a specific `print_stderr`
   allowance. Eleven core files allow `print_stdout` in a test module today.
   Change the convention repo-wide, or write the convention into `CLAUDE.md`?
   Do not change it inside these two folders alone.
No other row needs a ruling. The 2026-09-16 ruling covers FW-11: delete the
dead method with its test, and state the break in the commit body.

## 6. The split: two agents, two disjoint file sets

Agent A owns `feeds/**`. Agent B owns `tool_load/**`. No file appears twice.
The one cross-folder item is FW-06: agent A deletes the function in
`feeds/vendor_lookup.rs`, agent B deletes the stale comment in
`tool_load/optimize/context.rs`. The two edits are independent, so either
agent may land first.

**Neither agent touches these.** `crates/rs_cam_core/src/depth.rs` (S33: live,
outside both folders, in the mover's path). `crates/rs_cam_core/src/
compute/catalog.rs` and `stock/simulation_cut.rs` (the mover's path).
`crates/rs_cam_viz/src/ui/feeds/**` (a different crate).

### Agent A — `feeds/**`

| file | rows |
|---|---|
| `crates/rs_cam_core/src/feeds/vendor_lut.rs` | FW-01, FW-07, FW-12, FW-13, FW-18 |
| `crates/rs_cam_core/src/feeds/suggest.rs` | FW-02, FW-10 (doc), FW-16 |
| `crates/rs_cam_core/src/feeds/vendor_lookup.rs` | FW-05, FW-06 |
| `crates/rs_cam_core/src/feeds/rationale.rs` | FW-10, FW-13 |
| `crates/rs_cam_core/src/feeds/quantities.rs` | FW-11 |
| `crates/rs_cam_core/src/feeds/explain_payload.rs` | FW-13 |
| `crates/rs_cam_core/src/feeds/mod.rs` | FW-13 |
| `crates/rs_cam_core/src/feeds/predict.rs` | FW-13 |
| `crates/rs_cam_core/src/feeds/provenance.rs` | FW-13, FW-19 |
| `crates/rs_cam_core/src/feeds/feed_explanation.rs` | FW-16 |

### Agent B — `tool_load/**`

| file | rows |
|---|---|
| `crates/rs_cam_core/src/tool_load/mod.rs` | FW-08, FW-20 |
| `crates/rs_cam_core/src/tool_load/verdict.rs` | FW-09, FW-14, FW-16 |
| `crates/rs_cam_core/src/tool_load/locality.rs` | FW-14 |
| `crates/rs_cam_core/src/tool_load/optimize/mod.rs` | FW-03, FW-21 |
| `crates/rs_cam_core/src/tool_load/optimize/patches.rs` | FW-04, FW-17 |
| `crates/rs_cam_core/src/tool_load/optimize/axes.rs` | FW-08 |
| `crates/rs_cam_core/src/tool_load/optimize/context.rs` | FW-06 (comment) |
| `crates/rs_cam_core/src/tool_load/optimize/bounds.rs` | FW-14 |
| `crates/rs_cam_core/src/tool_load/optimize/outcome.rs` | FW-14 |
| `crates/rs_cam_core/src/tool_load/optimize/policy.rs` | FW-15 |
| `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` | FW-21 |
| `crates/rs_cam_core/src/tool_load/optimize/strategy/grid.rs` | FW-21 |
| `crates/rs_cam_core/src/tool_load/optimize/strategy/headroom.rs` | FW-21 |
| `crates/rs_cam_core/src/tool_load/optimize/candidate.rs` | FW-21 |
| `crates/rs_cam_core/src/tool_load/chipload.rs` | FW-23 (optional) |
| `crates/rs_cam_core/src/tool_load/deflection.rs` | FW-23 (optional) |
| `crates/rs_cam_core/src/tool_load/power.rs` | FW-23 (optional) |

## 7. Gates

The operator ruled on 2026-09-11: no large test gates. Run the sets below,
not the heavy gate, and not the 117 test files that merely name `feeds::` or
`tool_load::`. Each set is the union of the tests that import the touched
modules.

**CAUTION. Two targets simulate and run long:
`wanaka_suggest_integration` (agent A) and `wanaka_e2e_chipload_gate`
(agent B). Run them last, and ask the operator before you start them. The
2026-09-11 ruling asks before any test run over three minutes.**

**Both agents, always.** `cargo fmt --all -- --check` and
`cargo clippy -p rs_cam_core --all-targets -- -D warnings`. The clippy run is
the proof for every B and F row. One cargo job at a time: check `free -g` and
`pgrep -f "carg[o]"` before you launch.

**Agent A.** `cargo test -p rs_cam_core --lib feeds::` first, then these
integration targets:

- vendor_lut and vendor_lookup (FW-01, FW-05, FW-06, FW-07, FW-12, FW-18):
  `vendor_lut_sub_1mm`, `vendor_sidebyside_chipload`, `lookup_parity`,
  `lut_resolver_census_a6`, `chipload_extrapolation_flag_rider`,
  `chipload_formula_calibration`, `chip_thickness_policy_a9`,
  `rubbing_floor_envelope_band_p1`,
  `rubbing_floor_diameter_scaling_measurement`, `law_magnitude_measurement`,
  `drop_cutter_flat_roughing_row_g_dcflat`.
- suggest (FW-02, FW-10, FW-16): `wanaka_suggest_integration`,
  `suggest_feed_matches_final_geometry`,
  `apply_reports_a_field_it_cannot_hold_g_notheld`,
  `pill_writes_clamped_value_g_pillclamp`, `arc_fit_disposition_a5`.
- feed_explanation and explain_payload (FW-16, FW-13):
  `feed_explanation_record_t1`, `feed_explanation_snapshot_b3`,
  `power_ceiling_parity_f2`.
- rationale, quantities, provenance (FW-10, FW-11, FW-13, FW-19): no
  integration test names these modules. The `--lib` run is the whole gate.

**Agent B.** `cargo test -p rs_cam_core --lib tool_load::` first, then:

- verdict (FW-09, FW-14, FW-16): `gate_population_vacuity_xvac`,
  `chipload_advisory_disclosure_h4`, `chipload_report_wording_t12_t15`,
  `chipload_abstention_cannot_supersede_g_chipgate`,
  `chipload_boundary_g_chip_ulp`, `drill_evidence_wording_d3`,
  `predicted_feed_gates_f035`, `ceiling_advisory_and_clamp_record_a7`,
  `heatmap_two_arc_divergence_a1`.
- locality (FW-14): `arcfit_gate_population_d4`, `arcfit_intent_boundary_f1`,
  `axial_engagement_vs_dpp_detector_f2`.
- optimize (FW-03, FW-04, FW-08, FW-15, FW-17, FW-21): `optimize_smoke`,
  `optimizer_assumption_stamp_a8`, `optimize_toolpath_is_a_job_wp14b`,
  `optimize_reports_progress_wp29`, `feedopt_clamp_never_panics_wp21`,
  `generator_extremes_fuzz_r1`.
- chipload, deflection, power (FW-23, only if taken): the `--lib` run plus
  `wanaka_e2e_chipload_gate`.

One red is expected and is not yours:
`adaptive_feed_modulation_pipeline_f036b::modulation_raises_cutting_chipload_toward_band`.
See FW-P3.
