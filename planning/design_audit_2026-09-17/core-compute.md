# core-compute — design and feature-debt audit

Group: `crates/rs_cam_core/src/compute/` (33 files, 26 368 lines) — operation
dispatch, the configuration catalogue, simulation orchestration.

Audited 2026-09-17, read-only. Evidence is an `rg` count or a `file:line`
anchor.

### CMP-01 the 24-arm dispatch fallback in `execute.rs` is unreachable
- kind: design
- pattern: dead enum dispatch kept as a compile net
- where: `crates/rs_cam_core/src/compute/execute.rs:618-681`, `crates/rs_cam_core/src/compute/catalog/schema.rs:378-386`, `crates/rs_cam_core/src/compute/catalog/tests.rs:34-59`
- evidence: `rg -c 'generate: Some' registry.rs` = 24; `rg -n 'generate: None' registry.rs` = 0 hits. `execute.rs:618` reads `if let Some(generate) = op.op_type().registry_entry().generate`, so the 24-arm `else { match op { … } }` (`:621-680`) never runs. The sentry `generate_adapter_migration_is_an_explicit_per_op_decision` asserts `generate.is_some()` for every op, which makes the `None` branch unreachable by test as well as in fact.
- proposal: change `OpRegistryEntry::generate` to `GenerateFn` (not `Option<GenerateFn>`), delete the 60-line `else` match and delete the now-tautological sentry. The compile-time net survives: `OperationType::registry_entry` (`catalog.rs:320-345`) is an exhaustive match, so a new variant still fails to compile until it names a row, and a row cannot be written without an adapter.
- breaks: `OpRegistryEntry` is `pub`; the field type changes. No wire key, no file format.
- effort: S
- risk: low — the deleted arms call the same adapter fns the registry holds.
- sentry: `catalog/tests.rs::operation_catalog_is_exhaustive_and_consistent` already walks `OperationType::ALL` and would carry the "every row names an adapter" claim for free once the field is non-optional.
- owner:

### CMP-02 the dispatch entry takes 20 positional arguments; `ExecutionContext` already exists
- kind: design
- pattern: parameter struct with more than eight fields
- where: `crates/rs_cam_core/src/compute/execute.rs:539-576` (20 args), `:449-463` (16-arg wrapper), `:402-415` (14-arg wrapper), `:246-330` (`ExecutionContext`, 22 fields)
- evidence: `execute_operation_annotated_with_regions` declares 20 parameters, each carrying its own paragraph of doc comment; its first act (`execute.rs:578-614`) is to pack 21 of them into `ExecutionContext`. The two wrappers above it exist only to forward `None` for the arguments a caller does not have. Production callers: exactly two, both in core (`session/compute.rs:923` and `session/compute.rs:1215`); `execute_operation` has none (`#[cfg_attr(not(test), allow(dead_code))]` at `execute.rs:399`).
- proposal: make `ExecutionContext` the argument. `session::execute_generation` already owns a `ResolvedGenInputs`/`GenContext` pair and can build the context directly; the advisor call site builds one with the fields it has. Both wrappers and their ~120 lines of "this argument is `None` for callers who…" prose then delete. `execute_operation` stays or goes per §23 ruling 1 — that is a separate decision, not part of this change.
- breaks: `pub(crate)` signatures only; nothing crosses the crate boundary.
- effort: M
- risk: medium — the wrappers are the only thing keeping the advisor's shorter call compiling; every dropped argument must land on a named context field.
- sentry: `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` pins the door's visibility; add a case that the advisor path and `execute_generation` build contexts from the same constructor.
- owner:

### CMP-03 `kind_str()` is a hand-written 24-arm copy of the derived serde name
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/compute/catalog.rs:441-467`, `crates/rs_cam_core/src/compute/catalog/tests.rs:290-335`
- evidence: `OperationType` derives `Serialize` with `#[serde(rename_all = "snake_case")]` (`catalog.rs:214-219`). `operation_type_serde_repr_pinned` asserts, for all 24 variants, `op_type.kind_str() == repr` AND `serde_json::to_value(op_type) == repr`. The two lists are therefore required to be identical, and both are maintained by hand. `name()` (`catalog.rs:434-438`) is a third spelling of the same identity, but that one IS generated from the X-macro.
- proposal: add the snake_case token as a fourth column in `for_each_op!` and generate `kind_str()` beside `name()` and `category()`. The pinned test stays as the one guard that the generated token equals the serde repr.
- breaks: none — the strings are unchanged.
- effort: S
- risk: low — the sentry compares both sides of the identity.
- sentry: `catalog/tests.rs::operation_type_serde_repr_pinned` (existing).
- owner:

### CMP-04 `cutting_levels()` is the one wildcard match in a no-wildcard catalogue, and repeats one call six times
- kind: design
- pattern: enum dispatch replicated / wildcard fallback
- where: `crates/rs_cam_core/src/compute/catalog.rs:1328-1372`
- evidence: `catalog.rs:1370` reads `_ => vec![]`. Every sibling match in the same file names every variant and says so in prose — `entry_probe_leave` ("Every variant is named — no wildcard arm"), `optimization_surface` ("the match has **no wildcard arm**"), `feeds_hints` ("adding an operation does not compile until it decides its hints"). Inside the same function, `Adaptive`, `Zigzag`, `Rest` and `Trace` are four byte-identical `DepthStepping::new(top_z, top_z - cfg.depth.abs(), cfg.depth_per_pass).all_levels()` arms, and `Pocket`/`Profile` are two byte-identical `DepthStepping { … }` literals.
- proposal: name every variant (the 3D ops and VCarve/Chamfer/Inlay/Drill/AlignmentPinDrill collapse into one `=> vec![]` arm-group, which is a recorded decision instead of a fallback) and hoist the four identical arms behind one `depth_and_step(&self) -> Option<(f64, f64)>` helper. A new depth-stepped 2.5D operation then fails to compile rather than silently generating at one Z level.
- breaks: none.
- effort: S
- risk: low — arm-for-arm, no behaviour change.
- sentry: none today. Write `cutting_levels_is_exhaustive_per_op`: for every `OperationType::ALL`, assert the level count matches the op's declared `DepthSemantics`.
- owner:

### CMP-05 the parameter registry states a domain for 5 of 243 params and a description for 8
- kind: feature-debt
- pattern: a guard that reports the wrong thing / half-built validation
- where: `crates/rs_cam_core/src/compute/catalog/registry.rs` (243 `ParamDef::` entries), `crates/rs_cam_core/src/compute/catalog/schema.rs:56-82`, `crates/rs_cam_core/src/session/compute/params.rs:64-79`
- evidence: `rg -o 'ParamDef::(…)' registry.rs | sort | uniq -c` gives `199 required`, `31 optional`, `6 optional_desc`, `5 required_ranged`, `2 required_desc`. So `check_param_range` (`params.rs:64`) returns `Ok(())` unconditionally for 238 of 243 params, and `get_operation_schema` publishes a `range` for 5 and a `description` for 8. `ParamRange`'s own doc says absent means "not measured, not unbounded" — the type is honest, but the catalogue is 98 % unstated.
- proposal: this is a sweep, not a refactor: the ranges must be stated per param by someone who knows the dial. The cheap first cut is the ~40 params whose generator already refuses a value (depth-per-pass, stepovers, z-steps, peck depth, tolerances) — those domains are already written down in the generators as runtime guards, so moving them to `required_ranged` turns a mid-generation failure into a setter refusal. Record the decision for the rest rather than leaving `None` ambiguous.
- breaks: MCP `set_toolpath_param` starts refusing values it used to accept. That is the point; state it in the commit.
- effort: L
- risk: medium — a range set too tight refuses a legitimate operator value; each one needs the generator's own guard as its source.
- sentry: `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs` guards the refusal shape; add a per-param table test as ranges land.
- owner:

### CMP-06 24 hand-written `impl OperationParams` blocks, 432 of 755 lines pure accessor boilerplate
- kind: design
- pattern: hand-replicated per operation
- where: `crates/rs_cam_core/src/compute/operation_configs.rs:1670-2478`
- evidence: 24 `impl OperationParams for …Config` blocks, 755 lines together. A script over the blocks measures 432 lines in the six trivial accessors alone (`feed_rate`, `set_feed_rate`, `plunge_rate`, `set_plunge_rate`, `spindle_rpm`, `set_spindle_rpm` — 3 lines each, 18 lines per impl, present in all 24). Five blocks are byte-identical after whitespace normalisation (`FaceConfig`, `PocketConfig`, `AdaptiveConfig`, `RestConfig`, `ZigzagConfig`), three more are (`SteepShallow`, `SpiralFinish`, `HorizontalFinish`), plus two further pairs (`Profile`/`Trace`, `Scallop`/`UnifiedFinish`).
- proposal: a derive or a declarative macro. `impl_operation_params!(FaceConfig { stepover, depth_per_pass, total_depth: depth, depth_semantics: Explicit(depth) })` covers every block; the six universal accessors need no declaration at all because all 24 configs name those fields identically. The aliasing configs (Waterline `depth_per_pass → z_step`, RampFinish `→ max_stepdown`, Pencil `stepover → offset_stepover`) declare the alias in the macro call, which also makes CMP-08's hidden alias list greppable in one place.
- breaks: none — trait impls only, no serde shape change.
- effort: M
- risk: low — each block is mechanical; the macro expansion is compared against the current impl per op.
- sentry: `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs` already exercises the optional setters' `false` return for every op that lacks a field.
- owner:

### CMP-07 the registry holds five per-op decisions; eight more stayed as hand-written matches
- kind: design
- pattern: enum dispatch replicated in N places
- where: `crates/rs_cam_core/src/compute/catalog/schema.rs:377-390` (the 6-field row), and the eight outside it: `catalog.rs:423` `lateral_raster_stepover`, `:472` `is_drill_kinematics`, `:476` `OperationType::transform_capabilities`, `:571` `air_cut_high_threshold_pct`, `:811` `entry_probe_leave`, `:848` `optimization_surface`, `:1267` `feeds_hints`, plus `generated_empty.rs:186` `feature_selective_exemption`. (`OperationConfig::transform_capabilities` at `:1016` is NOT on the list — it is a one-arm Scallop override that falls through to the type-level table, not a per-op enumeration.)
- evidence: `OpRegistryEntry` carries `spec`, `param_defs`, `tool_constraints`, `dressup_policy`, `generate`. Every other per-op policy is a match or a `matches!` elsewhere. The registry's own header states the goal — "Data-only per-operation metadata table… There are deliberately NO wildcard fallbacks here" (`schema.rs:333-340`) — and the Phase-1 commits record killing wildcards one by one (`tool_constraints_are_an_explicit_per_op_decision`). Three of the eight are `matches!` membership lists that fail OPEN: `lateral_raster_stepover` (`catalog.rs:423-428`), `is_drill_kinematics` (`:472`), `feature_selective_exemption` (`generated_empty.rs:186-191`). A 25th operation joins the false side of each without a compiler word.
- proposal: move the three `matches!` predicates onto `OpRegistryEntry` as named policy fields (`raster_stepover_is_lateral: bool`, `kinematics: Kinematics`, `empty_is_feature_selective: bool`), each defaulted by an explicitly-named constant the way `ToolConstraintsDef::ANY_TOOL` and `DressupPolicy::ANY_DRESSUP` already are. Leave the five exhaustive matches alone — they already fail closed; only the membership lists are the hazard.
- breaks: `OpRegistryEntry` is `pub`; three fields are added.
- effort: M
- risk: low — `drill_kinematics_set_is_pinned` (`catalog/tests.rs:157`) already pins one of the three sets, so the move is verifiable arm by arm.
- sentry: `catalog/tests.rs::drill_kinematics_set_is_pinned` (existing); add the same shape for the other two sets.
- owner:

### CMP-08 four parameter names are accepted by the setter and absent from the published schema
- kind: feature-debt
- pattern: stringly-typed door
- where: `crates/rs_cam_core/src/session/compute/params.rs:225-234`, `crates/rs_cam_core/src/compute/catalog.rs:1184-1194` (`param_names_for_type`), `crates/rs_cam_core/src/compute/catalog.rs:1197-1219` (`schema_for_type`)
- evidence: the `depth_per_pass` arm's own comment: "Note the three ALIAS setters this arm is the only route to: Waterline maps `depth_per_pass` onto `z_step`, RampFinish onto `max_stepdown`, and Pencil maps `stepover` onto `offset_stepover`. **The registry publishes none of those names**". So `get_operation_schema("waterline")` lists `z_step` and not `depth_per_pass`, `set_toolpath_param(…, "depth_per_pass", …)` succeeds anyway, and the refusal message for a genuinely unknown name lists "Valid parameters:" from `param_names()` — a list that is wrong in both directions for those three ops. `debug_enabled` (`params.rs:280`) is a fourth accepted name in no `param_defs` array.
- proposal: give `ParamDef` an `aliases: &'static [&'static str]` field, declare the three aliases on the three ops, and have `param_names_for_type` / `schema_for_type` publish them. `debug_enabled` is not an operation param at all — it writes `tc.debug_options`, not the config — so it belongs in the schema's own section or in a separate command.
- breaks: the published `get_operation_schema` payload gains entries. That is a wire-visible change to an MCP read.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs`; add "every name `set_toolpath_param` accepts appears in `param_names()`", driven off `OperationType::ALL`.
- owner: (straddles core-session — the setter is theirs, the schema is mine; do not double-count)

### CMP-09 two config dials with an incomplete surface set
- kind: feature-debt
- pattern: a dial read but never set from one surface
- where: `crates/rs_cam_core/src/compute/operation_configs.rs:559` (`DropCutterConfig::scallop_height`), `:1240` (`UnifiedFinishConfig::classification_sampler`)
- evidence: my script diffed all 24 `param_defs` arrays against the config struct fields — 243 defs against 250 fields, seven fields unexposed. Five are deliberate (drill picks and `setup_z_flipped`, all driver-set). Two are not. `DropCutterConfig::scallop_height` is absent from `DROP_CUTTER_PARAMS`, so the generic serde arm refuses it as an unknown parameter, yet it drives `feeds_hints` (`catalog.rs:1284`) and has a dedicated GUI route (`rs_cam_viz/src/controller/events/mod.rs:1329 set_drop_cutter_scallop_height`) — a dial an operator can turn and an agent cannot. `UnifiedFinishConfig::classification_sampler` is read at `execute/finish_3d.rs:360` and reaches `finish/unified_finish.rs:1555`, but `rg classification_sampler crates/rs_cam_viz/src` returns nothing and it is in no `param_defs` array: no surface sets it, so it is a research dial frozen at `ClassificationSampler::PRODUCTION`.
- proposal: add `scallop_height` to `DROP_CUTTER_PARAMS` as `ParamDef::optional("scallop_height", "option<f64>")` — one line, and the GUI route then has an MCP twin. For `classification_sampler`, decide: publish it as a param, or delete the field and inline `PRODUCTION` at `finish_3d.rs:360`.
- breaks: `get_operation_schema("drop_cutter")` gains a row; deleting `classification_sampler` breaks project TOML files that carry it.
- effort: S
- risk: low.
- sentry: none today — see CMP-10, which is the sentry that would have found both.
- owner:

### CMP-10 nothing checks that `param_defs` covers the config struct
- kind: design
- pattern: hand-replicated per operation, unguarded
- where: `crates/rs_cam_core/src/compute/catalog/tests.rs:8-28`, `crates/rs_cam_core/src/compute/catalog/registry.rs` (243 `ParamDef` entries), `crates/rs_cam_core/src/compute/catalog.rs:1117-1132` (`params_value_including_nulls`)
- evidence: `operation_catalog_is_exhaustive_and_consistent` asserts only `!entry.param_defs.is_empty()`. Nothing compares a def list to its struct. The gap runs both ways: `params_value_including_nulls` inserts a JSON null for any `ParamDef` name, so a def naming a field that no longer exists would publish a null forever, and a new struct field is simply invisible to MCP (CMP-09 is two live instances). This is the same class the registry closed for tool constraints and dressup policy, left open for the largest table.
- proposal: one test. For every `OperationType::ALL`, serialize `OperationConfig::new_default(op)` and compare the `params` object's keys against `param_names_for_type(op)`; assert every def names a real key, and assert the unexposed fields are exactly a named allow-list with a reason per entry (the drill picks, `setup_z_flipped`). Fields that serde skips when `None` need the default filled with `Some` in the fixture, or the allow-list records them.
- breaks: none.
- effort: S
- risk: low — the test is the whole change.
- sentry: the proposed `param_defs_cover_every_config_field`.
- owner:

### CMP-11 the dispatch docs describe a two-driver world that no longer exists
- kind: feature-debt
- pattern: a "for now" comment with a live consequence
- where: `crates/rs_cam_core/src/compute/execute.rs:3`, `:150`, `:447`, `:485`, `:587`; also `compute/stats.rs:114`, `:149`, `compute/generated_empty.rs:68`
- evidence: `execute.rs:3` says "Both `ProjectSession` and the GUI compute worker delegate here"; `:587` says "both callers (session's `generate_toolpath`, the GUI worker's `generate_via_core`)". `rg generate_via_core crates --type rust` finds it in no production file — only in viz test prose that records its removal (`rs_cam_viz/src/compute/worker/execute/mod.rs:407` "These tests used to drive `generate_via_core`, a viz-side assembly"). `rg 'execute_operation' crates/rs_cam_viz/src crates/rs_cam_cli/src` outside tests returns two doc-comment lines and no call. The real caller set is two functions, both in `crates/rs_cam_core/src/session/compute.rs` (`:923`, `:1215`).
- proposal: rewrite the five anchors to name the two real callers. The consequence is live: CMP-02's argument-bundling change looks far riskier than it is while the docs claim a second driver in another crate, and an engineer choosing between `execute_operation_annotated` and `_with_regions` is told to pick by "which driver you are", a question with one answer now.
- breaks: none — comments only.
- effort: S
- risk: low.
- sentry: none possible for prose. `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` pins the caller set structurally.
- owner:

### CMP-12 the operation-config re-export list is hand-maintained twice and has already diverged
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/compute/mod.rs:38-47`, `crates/rs_cam_viz/src/state/toolpath/configs.rs:3-12`
- evidence: core re-exports 37 names from `operation_configs`; viz re-exports 39 of the same names straight from `rs_cam_core::compute::operation_configs`. The two lists differ by exactly `UnifiedFinishConfig` and `ClaimsReference` — present in viz, absent from core's. So `rs_cam_core::compute::UnifiedFinishConfig` does not resolve while the other 23 config types do, for the newest and largest operation. Nobody noticed because viz bypasses the core list entirely.
- proposal: delete the `pub use operation_configs::{…}` block in `compute/mod.rs` and let both crates name `compute::operation_configs::…`. That is one list, in the module that defines the types, and the divergence cannot recur. (`operation_configs` is already `pub mod`.)
- breaks: `rs_cam_core::compute::FaceConfig` and 36 siblings stop resolving; every in-crate and viz use site re-paths. Operator ruling 2026-09-16 permits it.
- effort: M — mechanical but wide.
- risk: low — the compiler finds every site.
- sentry: none needed; the build is the check.
- owner:

### CMP-13 the two geometry guards drop the operation's identity from the refusal
- kind: design
- pattern: error handling that drops the cause
- where: `crates/rs_cam_core/src/compute/execute/shared.rs:51-57` (`require_polygons`), `:59-61` (`require_mesh`) vs `:69-75` (`require_index`)
- evidence: `require_index` (`:69`) takes `op_name` and produces `"{op_name} requires a spatial index"`. Its two siblings in the same file do not: `require_polygons` (`:51`) returns `MissingGeometry("Operation requires 2D geometry")` and `require_mesh` (`:59`) returns `MissingGeometry("Operation requires a 3D mesh")`, with no way to tell which of the 24 operation KINDS refused. The instance is not lost — `session/compute.rs:880` wraps the error as `SessionError::OperationFailed` beside the toolpath — which is exactly why the fix belongs at the guard and not upstream. `OperationType::name()` (`catalog.rs:434`) exists for precisely this — its doc says "for diagnostics that must name the operation that refused a field".
- proposal: give both guards the same `op_name: &str` parameter `require_index` already has, and pass `op.op_type().name()` at every call site.
- breaks: two `pub(super)` signatures, and the two error strings change. Nothing matches on them.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_core/tests/generated_empty_refusal_g_entryempty.rs` is the nearest refusal-shape sentry; add a case asserting a mesh-less 3D op names itself.
- owner:

## Part 2 — simulation orchestration and configuration

Second auditor, same day. Files: `config.rs` (2 532 lines), `simulate.rs`
(2 304), `tool_config.rs` (1 297), `stock_config.rs` (809), `validate.rs`
(783), `sim_prefix.rs` (780), `transform.rs` (618), `stats.rs` (335),
`collision_check.rs` (181) and `execute/dressup_apply.rs` (800). These are
the files Part 1 listed as unread. Findings are numbered from CMP-14.

### CMP-14 a holder-collision check that FAILED reports zero collisions
- kind: feature-debt
- pattern: a guard that reports the wrong thing
- where: `crates/rs_cam_core/src/session/compute/diagnostics.rs:358-371` (the `.unwrap_or(0)` is `:367`), `crates/rs_cam_core/src/compute/collision_check.rs:63` (the API that can express failure), `crates/rs_cam_core/src/stock/sim_triage.rs:328`
- evidence: `holder_collision_counts` maps each toolpath through `.collision_check(idx, cancel).map(|r| r.collision_report.collisions.len()).unwrap_or(0)`. Its own doc says "Toolpaths whose check fails (missing mesh, etc.) count as 0". The pair `(id, 0)` then flows into `ProjectEvidence::holder_collisions`, and the triage reads `for (tp, count) in inputs.holder_collisions.iter().filter(|(_, n)| *n > 0)` — so a failed check is indistinguishable from a clean one at every consumer. `run_collision_check` returns `Result`, so the compute API already carries the failure; the session drops it.
- proposal: return `Vec<(ToolpathId, Option<usize>)>`, or three states: not-applicable (`SessionError::MissingGeometry` on a 2D op, which the code comment already calls expected), failed, measured. `None` must not reach a `> 0` filter. Commit `70a3db27` fixed this exact shape in the CLI today (`collision_count: Option<usize>` plus `collision_checks_failed`); core still writes the zero that commit's message calls "a clean bill of health asserted with no evidence".
- breaks: `ProjectEvidence::holder_collisions` and `SimTriageInputs::holder_collisions` change type. `pub`, no wire key.
- effort: S
- risk: low — the CLI fix is the reference shape and it landed today.
- sentry: `crates/rs_cam_cli/tests/a_failed_collision_check_is_not_a_clean_one_g_colfail.rs` is the CLI producer guard from `70a3db27`. Write the core twin: a toolpath whose mesh is absent must not appear as a measured zero in `ProjectEvidence`.
- owner: (straddles core-session — the sweep is theirs, the wrapper's `Result` is mine; do not double-count)

### CMP-15 a project file with an unknown `face_up` loads silently as Top
- kind: feature-debt
- pattern: stringly-typed door that fails open
- where: `crates/rs_cam_core/src/compute/transform.rs:65-74` (`FaceUp::from_key`), `:198-206` (`ZRotation::from_key`), `crates/rs_cam_core/src/session/project_file.rs:1039-1040`
- evidence: `FaceUp::from_key` ends `_ => FaceUp::Top` and `ZRotation::from_key` ends `_ => ZRotation::Deg0`. Both return `Self`, not `Option<Self>`. `rg -n 'from_key' --type rust` shows the only production call sites for these two are `project_file.rs:1039-1040`, and neither emits a `tracing::warn!`. The same function warns eleven lines later when an operation section is missing (`project_file.rs:1050`). The ruling these loaders sit under is stated on `tool_config.rs:82-86` (Q4): "file-loading surfaces warn and default to `EndMill`; the MCP mutation surface returns an explicit error". MCP honours it — `app/mcp/commands.rs:412-427` refuses an unknown face and `:447-461` an unknown rotation, each with the valid list. The loader does neither half: it defaults and stays quiet. `datum_from_section` (`project_file.rs:786-806`) shows the author already knew: it guards the empty key and its doc names this exact hazard.
- proposal: make both `from_key` return `Option<Self>`, and have the loader warn with the offending token and take the default — which is the Q4 policy. The consequence is not cosmetic: `face_up` chooses the cut direction, the local stock bbox and the whole emission frame, so a typo turns a bottom setup into a top one with no word to anyone.
- breaks: two `pub fn` signatures. No wire key, no file format.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_core/tests/face_up_names_follow_drafting_convention_g_frontname.rs` pins the variant meanings. Add a load case: a setup section with `face_up = "topp"` must warn and must not be read as a valid orientation.
- owner:

### CMP-16 rest-analysis defaults are copied by hand from the detector and nothing compares them
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/compute/config.rs:1982-1997` (`RestAnalysisConfig::default`), `crates/rs_cam_core/src/surface/rest_field.rs:170-181` (`RestFieldParams::default`), `crates/rs_cam_core/src/compute/config.rs:1931-1933` (the doc that states the rule)
- evidence: the struct doc says "Field defaults mirror `rest_field::RestFieldParams`'s own defaults (`cell_mm` = 0.5, `min_valley_depth` = 0.05, `region_margin_mm` = 0.5) … and should stay numerically in sync". Both files then write those three literals separately. `rg -n 'RestFieldParams::default|RestAnalysisConfig::default'` finds no test that compares the two. The consumer proves the coupling is real: `execute/dressup_apply.rs:58,80-84` builds one `RestFieldParams` from the config's three fields and takes `num_offset_passes_cap` and `min_cut_length` from `RestFieldParams::default()` in the same literal.
- proposal: derive the config defaults from `RestFieldParams::default()` instead of restating them. One `impl Default` reads the detector's own struct and copies the three fields. If the borrow direction is wrong, the cheap alternative is one test that asserts the three pairs are equal — the change is then the test alone.
- breaks: none — the numbers do not move.
- effort: S
- risk: low.
- sentry: none today. Write `rest_analysis_config_defaults_match_the_detector`.
- owner:

### CMP-17 the dressup vocabulary has no schema; its only published list is prose and is wrong in both directions
- kind: feature-debt
- pattern: a dial nothing reads, beside dials no surface names
- where: `crates/rs_cam_viz/src/mcp_server.rs:1157` (the `set_dressup_config` description), `crates/rs_cam_core/src/compute/config.rs:2041-2095` (the 23 fields), `:2083` (`retract_strategy`)
- evidence: `DressupConfig` has 23 fields. The MCP description names 17. A script diff gives the six it omits: `dogbone_angle`, `lead_in_feed_rate`, `lead_out_feed_rate`, `segment_merge`, `segment_merge_tolerance`, `air_bridge_policy`. Two of those six change emitted motion on every roughing operation (`segment_merge` is default-on for the roughing role, `config.rs:2148`). In the other direction, `rg -n 'retract_strategy' --type rust crates` returns nine hits and **none** of them reads the field to decide a retract: two are the GUI dropdown that writes it (`ui/properties/linking_dressup.rs:518-525`), one counts it as an "active dressup" in a badge (`:278`), one is the MCP description, two are the struct and its default, one is a project-file write (`session/mod.rs:2418`) and two are test fixtures. `dressup/` never reads it. So `set_dressup_field(…, "retract_strategy", "minimum")` is accepted, saved and ignored (G-RETRACTDIAL).
- proposal: operations have a `param_defs` registry (CMP-05, CMP-10); dressups have nothing. Give `DressupConfig` the same treatment — one table of field name, type and description — and build the MCP description from it, so the list cannot drift again. Delete `RetractStrategy` and its field in the same pass, or wire it; a saved dial that changes nothing is worse than an absent one, because the operator believes the retract is minimal.
- breaks: deleting `retract_strategy` breaks project TOML files that carry the key and removes an MCP-settable name. Operator ruling 2026-09-16 permits it.
- effort: M (S for the description and the deletion alone)
- risk: low.
- sentry: none today. Write `dressup_field_names_are_published`: every `DressupConfig` field appears in the tool description built from the table.
- owner:

### CMP-18 `run_simulation_memoized` is 609 lines with eight named seams
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/compute/simulate.rs:843-1451`
- evidence: the body runs 609 lines. Its own comment banners name the seams: "S5 prefix memo" (`:882-899`), the group loop (`:967`), the per-entry carve (`:1033`), the global playback stamp (`:1205-1240`), the checkpoint (`:1241-1276`), the end-of-group work (`:1305-1351`), the trace assembly (`:1369-1411`) and the deviation pass (`:1421-1436`). Sixteen mutable accumulators are destructured out of `PrefixState` in one statement (`:901-943`) and every one is threaded through all six stages. The snapshot capture at `:1278-1302` restates all sixteen by hand, field for field. That is the second place the accumulator list is written down.
- proposal: give the loop a `SimRun` struct holding the accumulators, with `PrefixState` as its snapshot form and one `From` in each direction. `carve_entry`, `stamp_playback`, `finish_group` and `assemble_trace` then take `&mut SimRun` and the function reads as the seven steps its own doc comment (`:789-798`) already lists. The snapshot clone becomes `run.snapshot()` and stops being a hand-maintained field list.
- breaks: none — all of it is private to the function today.
- effort: L
- risk: medium — the resume path is bit-identical by construction and any reordering breaks that argument. Do it only with `sim_prefix_memo_s5.rs` green on every step.
- sentry: `crates/rs_cam_core/tests/sim_prefix_memo_s5.rs::resumed_run_is_bit_identical_to_a_full_replay` fingerprints the whole `SimulationResult`; it is the right net for this refactor.
- owner:

### CMP-19 two builders, two mirror type sets and a hand-written translation for one simulation request
- kind: design
- pattern: one concept with two representations
- where: `crates/rs_cam_core/src/session/compute/simulation.rs:72-235` (core builder), `crates/rs_cam_viz/src/controller/events/simulation.rs:92-224` (viz builder), `crates/rs_cam_viz/src/compute/worker.rs:109-192` (`SetupSimToolpath`, `SetupSimGroup`, viz `SimulationRequest`), `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:37-122` (the translation)
- evidence: `SetupSimToolpath` carries nine fields; all nine map onto `SimToolpathEntry`'s eleven, because `tool: ToolConfig` expands into `tool` + `flute_count` + `tool_summary`. `SetupSimGroup` carries four of `SimGroupEntry`'s five. Viz `SimulationRequest` carries all nine of core's fields plus `memoize_prefix`. `build_core_simulation_request` then copies them across one by one. The drift this shape produces is already in the tree, in three places. (a) `metrics_not_applicable`: core tests three signals — `result.is_drill_op() || has_drilling_intent || op-kind` (`session/compute/simulation.rs:149-166`) — and viz tests only the op kind (`controller/events/simulation.rs:167-171`). `rg -n 'MoveIntent::Drilling' crates/rs_cam_core/src` shows only `ops/drill.rs` emits that intent today, so the two agree for every shipped generator; the divergence is structural, not yet live. (b) `SimGroupEntry::direction` — the core builder derives it from `setup.face_up`, the viz worker writes the constant `FromTop` (`execute/mod.rs:97`). The field is read only by the S5 key (`sim_prefix.rs:425`), and only the viz lane runs that cache, so the memo stays self-consistent; the field's doc ("derived from the setup's face-up orientation") is simply untrue on one of its two producers. (c) `local_stock_bbox` is `Option` in core and non-optional in viz, so the translation re-derives F-024's identity-setup rule a third time (`execute/mod.rs:69-75`). The phantom-prior-stock decision was already extracted into `PhantomPriorStockScan` (`simulate.rs:164-207`) for exactly this reason — the extraction stopped at one field.
- proposal: delete `SetupSimGroup`, `SetupSimToolpath` and the viz `SimulationRequest`; let the controller build `rs_cam_core::compute::simulate::SimulationRequest` directly and carry `memoize_prefix` beside it. The `SimBoundary` doc (`simulate.rs:262-266`) records the same consolidation being done for the boundary type on 2026-09-16 and why: "A new field on one copy dropped out at every hop."
- breaks: three `pub` viz types go. Same crate, compiler finds every site.
- effort: M
- risk: medium — the viz builder also feeds `build_playback_data` (`execute/mod.rs:132-205`), which reads the same groups; it must move to the core type with them.
- sentry: `crates/rs_cam_viz/src/controller/tests.rs:1137-1270` already drives `build_simulation_groups` directly; add a case that the two builders agree on `metrics_not_applicable` for a drill toolpath.
- owner: (straddles viz-shell — the mirror types are theirs, the core type is mine)

### CMP-20 `apply_dressups` takes 15 arguments and repeats one stage shape ten times
- kind: design
- pattern: parameter struct with more than eight fields / hand-replicated per stage
- where: `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:338-798`
- evidence: 15 positional parameters, 461 lines. The body is ten `if cfg.<flag>` gates, each wrapping one call in the same `apply_dressup_traced` envelope with a hand-written `DressupTraceInfo` literal: `:374` rapid order, `:450` drill-cycle entry guard, `:469`/`:500` entry, `:534` dogbone, `:555` lead in/out, `:597` link moves, `:630` arc fitting, `:653` segment merge, `:674` rapid order again, `:740` feed optimisation. Each literal restates four strings (`debug_key`, `debug_label`, `kind`, `semantic_label`) that exist nowhere else, so nothing can enumerate the pipeline's stages.
- proposal: two changes, independent. First, bundle the 15 arguments into a `DressupContext` — nine of them are per-generation constants (`tool_diameter`, `safe_z`, `stock_top`, `cutter`, `entry_surface`, `transform_capabilities`, the two trace contexts, `channels`). Second, make the stage list data: a slice of `(gate, DressupTraceInfo, transform)` that the driver folds over. A dressup then becomes one row, and `debug_key` becomes greppable as a set rather than as ten separate literals.
- breaks: one `pub fn` signature inside the crate.
- effort: M
- risk: medium — stage ORDER is load-bearing (segment merge runs after arc fitting; the second rapid-order pass runs after link moves). The table must pin the order, not just hold the rows.
- sentry: `crates/rs_cam_core/tests/dressup_span_invariants.rs` already names stage keys such as `"link_moves"` (`:73`); extend it to assert the stage list and its order.
- owner:

### CMP-21 `ToolpathStats` is a 23-field carrier for 18 per-generator findings, and `config.rs` is also the findings module
- kind: design
- pattern: a struct every layer mutates
- where: `crates/rs_cam_core/src/compute/config.rs:135-637` (`ToolpathStats`), `:743`, `:860`, `:934`, `:998`, `:1093`, `:1115`, `:1176`, `:1241`, `:1340` (nine `*Finding` structs), `:807` (`RetractTripCount`), `:638` (`StockSnapshotStamp`), `crates/rs_cam_core/src/compute/execute.rs:63-182` (`GenerationFindings`, 18 fields)
- evidence: `ToolpathStats` has 23 fields. Four are measured from the move list (`move_count`, `cutting_distance`, `rapid_distance`, `retract_trips`) and one is a caller stamp (`stock_snapshot`). The other 18 are optional reports owned by one generator each — one per field of `GenerationFindings` — `dropped_band`, `tip_float`, `ramp_reach_clamp`, `pencil_link`, `monotone_cells`, `region_cap`, `relink` and so on. The struct declaration spans 503 lines because each slot carries its own paragraph of provenance doc. `rg -l 'ToolpathStats' --type rust crates` returns 69 files. The nine finding types and their `message()` methods live in the same file, which the folder `CLAUDE.md` describes as "the configuration model". A finding type is not configuration.
- proposal: move the finding types and `ToolpathStats` out of `config.rs` into `compute/findings.rs` beside `execute/findings.rs`, leaving `config.rs` the height, boundary, rest, dressup and stock configuration it is named for. That is a file move, not a redesign; it halves the file and it puts the finding family in one place. Collapsing the 18 slots into one `Vec<GenerationFinding>` enum would be the larger change, and it would cost the compile-time guards `stats_with_findings` is built on (see Checked and clear) — do not do it without replacing those.
- breaks: `pub` paths change. Operator ruling 2026-09-16 permits it.
- effort: M — mechanical but wide.
- risk: low — the compiler finds every site.
- sentry: `crates/rs_cam_core/tests/findings_transport_join_h21.rs` guards the join; a move does not touch it.
- owner:

### CMP-22 the S5 memo key is a hand-written field list; the same folder holds the pattern that would guard it
- kind: design
- pattern: hand-replicated, unguarded
- where: `crates/rs_cam_core/src/compute/sim_prefix.rs:410-481` (`hash_entry_scalar`, `hash_group_scalar`, `hash_global_scalar`), `crates/rs_cam_core/src/compute/stats.rs:162-238` (`stats_with_findings`, the pattern)
- evidence: the three key builders name their inputs one by one — `hash_entry_scalar` hashes seven of `SimToolpathEntry`'s eleven fields plus the tool, and `hash_global_scalar` nine floats plus four scalars of `SimulationRequest`'s nine fields. Nothing destructures, so a new field on any of the three types compiles and is silently absent from the key. The module doc (`sim_prefix.rs:42-59`) carries the closure argument as a prose table, and the `PrefixState` doc (`:485-495`) states the matching rule for the state half — but only the state half has a sentry that would fail (`resumed_run_is_bit_identical_to_a_full_replay` fingerprints the result; a key omission produces a wrong HIT, which that test does not construct). `stats.rs:162-238` solves the identical problem three doors away, with a `..`-free destructure of both input structs and a `..Default::default()`-free literal, and its doc names the mechanism as the acceptance gate of H2.1.
- proposal: destructure in each `hash_*` function — `let SimToolpathEntry { id, name, …, annotated: _, semantic_trace: _, drill_op: _ } = entry;` — binding the hashed fields and explicitly `_`-ing the pointer-keyed ones. A new field is then `E0027` until someone decides whether it belongs in the key. That is the whole change; the hashes themselves do not move.
- breaks: none.
- effort: S
- risk: low — no hashed value changes, so no snapshot key changes.
- sentry: `crates/rs_cam_core/tests/sim_prefix_memo_s5.rs` (existing). The destructure is itself the new guard.
- owner:

### CMP-23 the S5 prefix memo reaches one crate; the CLI's own ladder pays the cost S5 exists to remove
- kind: feature-debt
- pattern: a capability missing from one surface
- where: `crates/rs_cam_core/src/compute/sim_prefix.rs:1-18` (the finding it answers), `crates/rs_cam_core/src/session/compute/simulation.rs:72-80` (`ProjectSession::run_simulation`, no memo parameter), `crates/rs_cam_cli/src/project.rs:341-378` (the CLI ladder)
- evidence: `rg -n 'SimMemo|SimPrefixCache' --type rust crates` finds the cache constructed in exactly one production place, `rs_cam_viz/src/compute/worker.rs:525,536`. `ProjectSession::run_simulation` calls `run_simulation(&request, cancel)` (`session/compute/simulation.rs:279`) — the three-argument door — and offers no way to pass a memo. The CLI runs the same fixpoint ladder the memo was built for: `project.rs:351-378` loops `session.run_simulation(&sim_opts, &cancel)` once per round, with its own comment "mirroring MCP `generate_all`'s default". The module doc states the size of what it is missing: "On the reference wanaka200 workload the ladder is 3 rounds / 2 simulations at 0.4 mm, and a standalone `run_simulation` at that resolution is 84 s".
- proposal: give `ProjectSession` an owned `SimPrefixCache` and a `run_simulation_memoized` door, or add a `memo: Option<SimMemo<'_>>` to `SimulationOptions`. `run_simulation_memoized` is already `pub`; only the session wrapper blocks the CLI from it. The same door serves `tool_load/optimize/candidate.rs:435`, which re-simulates per candidate.
- breaks: `SimulationOptions` gains a field, or `ProjectSession` gains a method. No wire key.
- effort: M
- risk: medium — the memo's soundness argument rests on the cache being in-process and single-slot (`sim_prefix.rs:79-83`). `ProjectSession` derives `Clone` and the `optimize_toolpath` job copies the session (`simulate.rs:362-370`), so a session-owned cache is copied per job unless it sits behind an `Arc<Mutex>`. The `SimulationOptions` route avoids that question; prefer it.
- sentry: `crates/rs_cam_core/tests/sim_prefix_memo_s5.rs` (existing) covers the kernel. Add a session-level case that two ladder rounds hit the memo.
- owner: (straddles core-session — the session door is theirs, the memo is mine)

### CMP-24 the collision wrapper rebuilds the spatial index once per toolpath and returns render positions
- kind: design
- pattern: work rebuilt per call / a render type in the core model
- where: `crates/rs_cam_core/src/compute/collision_check.rs:63-110`, `:67`, `:95-105`, `crates/rs_cam_core/src/session/compute/diagnostics.rs:358-371`
- evidence: `run_collision_check` starts `let index = SpatialIndex::build_auto(request.mesh);` (`:67`). `holder_collision_counts` calls it once per computed toolpath, and those toolpaths usually bind the same model — so an N-toolpath project builds the same index N times. The session's own doc admits the cost ("This is the expensive sweep (spatial-index build + interpolated toolpath walk per toolpath)") and answers it by telling callers not to call it, rather than by hoisting the build. Separately, `:95-105` flattens `CollisionEvent::position` into `Vec<[f32; 3]>` — `rg -n 'collision_positions' --type rust crates` shows its only consumers are the viz viewport, the GPU upload and the picker. `crate::geo::P3` is the crate's point type.
- proposal: let `CollisionCheckRequest` carry an optional prebuilt `&SpatialIndex` and have the sweep build one per distinct model. Drop `collision_positions` from the core result; the viz upload path already owns the `[f32; 3]` conversion for every other marker it draws.
- breaks: two `pub` shapes in `compute::collision_check`. Callers are two production sites.
- effort: S
- risk: low.
- sentry: none today. Write `collision_sweep_builds_one_index_per_model` over a two-toolpath one-model fixture.
- owner:

### CMP-25 the stale-default validator has four rules, none newer than 2026-05-20, and no CLI surface
- kind: feature-debt
- pattern: a half-built capability
- where: `crates/rs_cam_core/src/compute/validate.rs:1-58`, `:101-121` (`validate_stale_defaults`), `:122-148` (`validate_one_toolpath`)
- evidence: the rule enum has four variants, all from the Wanaka review of April and May 2026 (`DropCutterMinZPreB1`, `TaperedBallPlungePreFix2`, `WoodAdaptiveStepoverPreFix1`, `ProjectCurveNegativeDepth`). The module header states the convention — "Adding a rule per future B-roadmap entry is the ongoing convention" — and no rule has been added since. Reach is uneven: `rg -n 'validate_stale_defaults|validate_one_toolpath' --type rust crates` puts the session-wide entry point in one place only, `rs_cam_viz/src/app/mcp/project.rs:26`; the per-toolpath one in the GUI properties panel (`ui/properties/mod.rs:587`), the core export precondition (`session/compute/export.rs:132`) and `session/compute/diagnostics.rs`'s `diagnose_toolpath_with_trace`, whose only callers are viz MCP. `rg -n 'stale' crates/rs_cam_cli/src` returns three comment lines and no call — the batch CLI's diagnostics JSON carries no stale-default row at all.
- proposal: two separable pieces. Call `validate_stale_defaults` in the CLI's `project` command and put its findings in the JSON, which costs one call — the adapter (`diagnostics/adapters/from_stale_default.rs`) already turns them into `Diagnostic`. Then decide the rule library's status: either add the rules the convention promises, or write in the header that the library is closed at four, so the next reader does not treat an empty result as "no stale defaults".
- breaks: the CLI diagnostics JSON gains rows.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs` pins the GUI/MCP pair. Add the CLI to it.
- owner: (the CLI half straddles cli-mcp)

### CMP-26 `for_op` hand-writes the ProjectCurve dressup strip that the registry policy applies four lines later
- kind: design
- pattern: one per-op decision in two places
- where: `crates/rs_cam_core/src/compute/config.rs:2175-2193`, `crates/rs_cam_core/src/compute/catalog/registry.rs:971-975`
- evidence: `for_op` contains `if op == OperationType::ProjectCurve { cfg.entry_style = None; cfg.lead_in_out = false; cfg.link_moves = false; }` at `config.rs:2178-2187`, then calls `cfg.normalize_for_op(op)` at `:2193`. `normalize_for_op` reads `op.registry_entry().dressup_policy` (`:2205`), and ProjectCurve's entry is `DressupPolicy::strip_all("Incompatible with Project Curve: each ring would get a phantom diagonal cut.")` (`registry.rs:973-975`). `strip_all` sets exactly those three fields (`config.rs:2213-2229`). So the hand block is a no-op that restates a registry row, with its own rationale comment beside the registry's own. This is the one op-specific arm left in a method whose doc says the decisions moved to the registry in Phase 1 T5 (`config.rs:2205-2211`).
- proposal: delete `config.rs:2175-2187`. The registry row already carries the decision and the user-facing reason the GUI greys the controls with.
- breaks: none — the resulting config is identical.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_core/src/compute/config.rs:2480` `normalize_for_op_applies_registry_policy` (existing, in the file's own test module) already asserts the registry drives the strip. Add a `for_op(ProjectCurve)` case to it before deleting.
- owner:

### CMP-27 alignment-pin keying is validated on the GUI load path only
- kind: feature-debt
- pattern: a diagnostic missing from three of four surfaces
- where: `crates/rs_cam_core/src/compute/stock_config.rs:580-582`, `:712-745`, `crates/rs_cam_viz/src/controller/io.rs:467-473`
- evidence: `StockConfig::validate_pins_for_flip`'s own doc says "Call this on load and publish `PinFlipReport::warnings`", and names the defect it was written for: "a saved project whose pins were centre-symmetric — invariant under a 180 deg ROTATION, which is not the flip — and no load path looked". `rg -n 'validate_pins_for_flip' --type rust crates` shows three production call sites, all in viz: the controller's load (`controller/io.rs:471`), the stock panel (`ui/properties/stock.rs:468`) and the model event (`controller/events/model.rs:455`). `rg -n 'validate_pins_for_flip' crates/rs_cam_core/src/session` is empty — `ProjectSession` never calls it, so the CLI's headless load and the MCP `load_project` hear nothing. MCP `add_alignment_pin` applies the pin with no check at all (`app/mcp/commands.rs:329-350`): the plan records the position for undo and never asks whether the pair still keys.
- proposal: run the check in the core load path and surface it as a `Diagnostic`, the way stale defaults already reach `diagnostics/adapters/from_stale_default.rs`. Then `add_alignment_pin` and `remove_alignment_pin` get the same answer for free through the diagnostics read, and the GUI keeps showing it where it does now. The consequence is registration: a pin pair that is invariant under the wrong symmetry seats in the flipped orientation and in the un-flipped one, and both look right.
- breaks: none — an added diagnostic row.
- effort: S
- risk: low.
- sentry: `crates/rs_cam_core/tests/alignment_pin_keying_g_pinauto.rs` already covers the judgement itself; add a case that a loaded session publishes the warning.
- owner: (the load wiring straddles core-session)

### CMP-28 `stock_config.rs` holds the post-processor config, four id newtypes and the pin-placement algorithm
- kind: design
- pattern: a file that is not what it is named
- where: `crates/rs_cam_core/src/compute/stock_config.rs:7-20` (`ModelId`, `SetupId`, `FixtureId`, `KeepOutId`), `:77-118` (`PostConfig`), `:156-330` (`AlignmentPin`, `PinPlacementRequest`, `PinPlacementError`, `PinSide`), `:605-710` (`place_keyed_pins`), `:434-604` (`StockConfig`)
- evidence: 809 lines, of which `StockConfig` and its methods are 170. The rest is four id newtypes that the crate spine's `ids.rs` is the home for, the G-code post-processor configuration, and the keyed-pin geometry — a 105-line constructive algorithm with its own error enum and its own sentry file (`tests/alignment_pin_keying_g_pinauto.rs`). The folder `CLAUDE.md` lists `stock_config.rs` under "the configuration model" and says nothing about pins or posts.
- proposal: three moves. `ModelId` / `SetupId` / `FixtureId` / `KeepOutId` to `ids.rs`, beside `ToolpathId`. `PostConfig` to `gcode/`, which owns the post-processors. `AlignmentPin` and the pin placement and validation to a new `compute/alignment_pins.rs`, which is then the file the pin sentry names. `stock_config.rs` is left holding `StockConfig`, `ModelKind` and `ModelUnits`.
- breaks: `pub` paths change across three crates. Operator ruling 2026-09-16 permits it.
- effort: M — mechanical, wide.
- risk: low — the compiler finds every site.
- sentry: none needed; the build is the check. The pin sentry moves with the code.
- owner:

### Part 2 top three

1. **CMP-14** — a holder-collision check that failed reports zero collisions. Effort S. The CLI half of this exact defect was fixed today in `70a3db27`, so the shape of the fix is written down and reviewed. Core still writes the zero, and `sim_triage.rs:328` filters it out with `> 0`, so a check that never ran reads as a clean one on the one question that can wreck a machine.
2. **CMP-15** — a project file with an unknown `face_up` loads silently as Top. Effort S. `face_up` chooses the cut direction, the local stock bbox and the emission frame. The crate already has both halves of the answer: MCP refuses the same token with a valid list, and the Q4 ruling on `tool_config.rs:82` tells a loader to warn and default. This loader does neither.
3. **CMP-17** — the dressup vocabulary has no schema. Effort S for the two cheap halves. The published field list is a prose string that omits six fields, two of which change roughing motion by default, and it advertises `retract_strategy`, which nine `rg` hits show nothing reads. An operator sets the retract strategy, saves it, and gets the retract they did not ask for.

CMP-16 is the closest runner-up: one test, no behaviour change, and it closes a hand-synced default pair whose own doc says the two must stay equal.

### Part 2 checked and clear

- `stats.rs` is the model the rest of the group should copy. `stats_with_findings` (`:162-238`) destructures both input structs with no `..` and builds the result with no `..Default::default()`, so a new finding is three compile errors rather than a silent `None`. Its doc names the mechanism and the five GUI patches that motivated it. CMP-22 asks the memo key to use the same trick.
- `sim_prefix.rs` is sound and bounded. The key closure is argued input by input (`:42-59`), the `stamp_dispatch` omission is a stated precondition with its own sentry, `kinematics` is correctly excluded because `apply_kinematics_cycle_time` runs after the loop, pointer keys are `Weak` rather than raw addresses for a recorded ABA reason, and `take_match` removes on hit and on miss. Only the key's field ENUMERATION is unguarded — CMP-22.
- `compute_retract_trips` (`stats.rs:270-335`) reports `None`, not zero, for the in-node split when spans are absent or untrusted, and `compute_stats_with_spans` (`:27-108`) writes a "not measured" comment against every generation-owned slot. This is the honest shape CMP-14 is missing.
- `ToolType` is one vocabulary. `parse_lenient` (`tool_config.rs:86-93`) matches the canonical serde token case-insensitively and returns `Option`; `serde_token` is pinned by `tool_type_serde_repr_pinned`. The `end_mill`-vs-`flat` pair CLI-08 names is `ToolType::EndMill` against `feeds::CutterKind::Flat` — two deliberate vocabularies with a bijection pinned by `tool_type_cutter_kind_round_trips` (`tool_config.rs:47-56`). It is not a core naming defect.
- `PhantomPriorStockScan` and `contributes_simulated_motion` (`simulate.rs:118-207`) are a real extraction: the one decision the two request builders could drift on is a shared type, and G-STICKYEMPTY records what the drift cost. CMP-19 is the observation that the extraction stopped at one field.
- `SimBoundary` is one type since 2026-09-16 (`simulate.rs:262-279`); the viz worker and the viz simulation state no longer carry copies.
- `SetupEvalContext` (`session/eval_context.rs:100-130`) does not duplicate `compute/transform.rs`. It calls `session.setup_transform_info(...)` and `SetupTransformInfo::effective_stock_bbox`, and it is the F-030 consolidation that the core builder, the viz controller and the viz worker all go through.
- Dressup persistence is serde-driven end to end: `ProjectToolpathSection::dressups` is a `DressupConfig` (`project_file.rs:495`), `save.rs:235` clones it, and `set_dressup_field` merges through `serde_json` and refuses an unknown key (`session/mutation/config.rs:73-79`). A new dressup field therefore reaches the project file and MCP for free — only the description string lags (CMP-17). `dressup_changes` (`project_file.rs:703-727`) even names its own blind spot: "a dressup value this reader does not name".
- `datum_from_section` (`project_file.rs:786-806`) guards the empty key before calling `from_key` and its doc names the exact fall-through hazard CMP-15 reports on the two neighbours that do not.
- `collision_check.rs` is not a second home for collision detection. It contains no geometry: `stock/collision.rs` owns the assembly, the interpolation and the obstacle test, and the wrapper only builds the index and flattens positions. Its two defects are CMP-24. (`stock::collision::check_collisions` at `stock/collision.rs:174` is `pub` and called only from that file's own test module — a test-only `pub`, but it is core-stock's row, not mine.)
- `SimGroupEntry::direction` is on the brief's ruled-live list and stays. The producer disagreement is folded into CMP-19 as drift evidence, not proposed as a deletion.
- `MoveIntent::Drilling` has exactly one producer, `ops/drill.rs` (`:106`, `:219`, `:257`). The core builder's extra drill signals and the viz builder's op-kind test therefore agree for every shipped generator today.

### Part 2 add-a-thing count

The extension point for this half is **one dressup**. Files an engineer edits today:

1. `compute/config.rs` — the `DressupConfig` field, its `Default`, and a decision in `for_role` (three role arms).
2. `compute/execute/dressup_apply.rs` — the `if cfg.<flag>` stage plus its `DressupTraceInfo` literal, at the right point in the order (CMP-20).
3. `dressup/<name>.rs` — the transform itself, with a `_with_provenance` twin.
4. `compute/catalog/schema.rs` + `catalog/registry.rs` — only if the dressup needs a per-op policy. `DressupPolicy` carries two decisions today (`strip_all_reason`, `entry`), so a third kind of incompatibility means a new field and 24 rows.
5. `rs_cam_viz/src/ui/properties/linking_dressup.rs` — the control, and the "active dressups" counter (`dressup_active_count`, `:254-282`), which is a second hand-written list of the flags.
6. `rs_cam_viz/src/mcp_server.rs:1157` — the description string, which nothing checks (CMP-17).

**Total: 6 files, 2 crates**, of which two are silent on omission — the MCP description and the GUI active-count both just under-report. Persistence, the MCP setter and the load-time normalisation need no edit at all, because they are serde-driven or registry-driven. CMP-17 removes entry 6 and CMP-20 turns entry 2 into one table row.

## Top three

1. **CMP-01** — the 24-arm dispatch fallback in `execute.rs` is unreachable. Effort S, risk low, deletes 60 lines and one tautological sentry, and turns "migration state" into a type. Do it first: CMP-02 is much easier to read afterwards.
2. **CMP-10** — nothing checks that `param_defs` covers the config struct. Effort S, and the whole change is one test. It would have found both halves of CMP-09 on its own, and it closes the last unguarded hand-maintained table in a catalogue whose every other table is compile-enforced.
3. **CMP-08** — four parameter names are accepted by the setter and absent from the published schema. Effort S. An agent reads `get_operation_schema` and is told the wrong thing in both directions for three operations; the code comment already admits it.

## Checked and clear

- `spans.rs` and `annotate.rs` contain **zero** `OperationType::` / `OperationConfig::` references. They are shape-keyed helper libraries (depth runs, cut runs, drill holes, move intents), not a second dispatch — the `GenerateFn` doc's warning that "an adapter must reproduce its family's span helper" is about choosing one, not writing one.
- `execute/shared.rs` is a clean consolidation: `generated_with_spans` appends the move-intent transit spans for all 24 families in one place, and `require_mesh` / `require_index` / `with_depth_run_annotation` each record the duplication they replaced (R2.4, R2.7). Only the refusal text is weak — CMP-13.
- The `param_defs` arrays are accurate in the outward direction: 243 defs against 250 struct fields, and **no def names a field that does not exist** on any of the 24 configs. Only the inward direction leaks (CMP-09).
- `entry_probe_leave`, `optimization_surface`, `feeds_hints`, `air_cut_high_threshold_pct` and `OperationType::transform_capabilities` are all wildcard-free exhaustive matches. A 25th operation fails to compile at each. They are listed in CMP-07 for placement, not for safety.
- `feature_selective_exemption`'s `matches!` fails to the **safe** side — a new operation lands in the gated set, which is the conservative answer.
- `sim_prefix.rs`'s `generate_all` memo is bounded: `DEFAULT_MAX_BYTES = 1_500_000_000` (`sim_prefix.rs:207`) is checked against an estimate of each snapshot's size before admission. Not an ad-hoc cache.
- `DressupPolicy` really is one source for both compute and the viz panel (`config.rs:2203 normalize_for_op`, guarded by `normalize_for_op_applies_registry_policy` at `config.rs:2480`). The pre-registry hand-synced pair is gone.
- `rs_cam_cli run` is fully registry-driven: `run.rs:264-269` resolves the token off `OperationType::ALL` + `kind_str()`, and `--list-ops` prints the same list. Adding an operation costs zero files there.
- MCP `parse_operation_type` (`rs_cam_mcp/src/server.rs:1049`) parses through serde rather than a hand-written map — but see the add-a-thing list for its hard-coded error string.
- The `for_each_op!` X-macro genuinely generates eight surfaces from one row list: the `OperationType` declaration, `ALL`, `category()`, `name()`, `op_type()`, `new_default()`, `as_params()` and `as_params_mut()`. The catalogue is data-driven at the top and hand-written from `registry_entry()` down.

## Add-a-thing count

The extension point is **one operation**. Files an engineer edits today, derived
from `rg -l 'HorizontalFinish' crates/*/src` (a minimal 6-parameter 3D op;
20 production files) cross-checked against the arms I read. `UnifiedFinish`
touches 42 production files, but that is a subsystem with its own
`finish/unified_finish/` folder — the 20 below is the honest floor.

**Core — compulsory (the build fails without them):**

1. `compute/catalog.rs` — the `for_each_op!` row, the `ALL_2D`/`ALL_3D` list, and **seven** exhaustive-match arms: `registry_entry()`, `kind_str()`, `OperationType::transform_capabilities()`, `air_cut_high_threshold_pct()`, `entry_probe_leave()`, `optimization_surface()`, `feeds_hints()` — seven arms in one file.
2. `compute/catalog/registry.rs` — the `X_PARAMS` array and the `REG_X` const (plus the `use registry::{…}` import back in `catalog.rs`).
3. `compute/operation_configs.rs` — the config struct, its `Default`, its ~31-line `impl OperationParams`.
4. `compute/execute.rs` — the fallback match arm (dead, see CMP-01) and the `pub(crate) use` line.
5. `compute/execute/<family>.rs` — the `GenerateFn` adapter.
6. `compute/mod.rs` — the config-type re-export (and `state/toolpath/configs.rs` in viz, see CMP-12).
7. `finish/<op>.rs` or `ops/<op>.rs` — the generator itself.
8. `feeds/geometry_class.rs` — exhaustive match (owner: power session).

**Core — silent if you forget (the op compiles and behaves wrongly):**

9. `compute/catalog.rs::cutting_levels` (`:1370`) — `_ => vec![]`, so a depth-stepped op silently generates at one Z (CMP-04).
10. `compute/catalog.rs::lateral_raster_stepover` and `::is_drill_kinematics` — `matches!`, default false (CMP-07).
11. `compute/generated_empty.rs::feature_selective_exemption` — `matches!`, default gated.
12. `trace/narrate.rs` — the narration arms (2 sites for a finishing op).
13. `feeds/predict.rs`, `feeds/suggest/axial_envelope.rs`, `feeds/vendor_normalize.rs` — membership lists (owner: power session).

**viz — 6 files:**

14. `state/toolpath/configs.rs` and `state/toolpath.rs` — two hand-maintained re-export lists.
15. `ui/properties/operations/mod.rs` — the panel dispatch arm.
16. `ui/properties/operations/<family>.rs` — the parameter panel.
17. `ui/properties/operations/shape_diagrams.rs` — the diagram arm.
18. `ui/properties/toolpath_panel.rs` — the summary arm.

**cli / mcp — 2 files:**

19. `rs_cam_cli/src/job.rs:505-520` — only if the op should work in job files. The job vocabulary is a hand-written string map covering **6 of 24** ops, with its own token spellings (`"drop-cutter"`, `"finish"`) that do not match `kind_str()`, and its help text still says "all 23 ops" against an `ALL` of 24. `run` needs nothing.
20. `rs_cam_mcp/src/server.rs:1051` — the parse error hard-codes all 24 names as a string literal. Nothing fails if you forget; the error message just lies. It should be built from `OperationType::ALL`.

**Total: 20 files, 3 crates.** Seven of the arms are in one file; five of the
twenty are silent on omission. CMP-01, CMP-04, CMP-07 and CMP-12 each remove
one entry from this list.

## Not verified

I did not read `config.rs` (2 532 lines), `simulate.rs` (2 304),
`tool_config.rs` (1 297), `stock_config.rs` (809), `validate.rs` (783),
`sim_prefix.rs` beyond its cache bound, `transform.rs`, `stats.rs`,
`collision_check.rs` or `execute/dressup_apply.rs`. Nor did I audit the group's two large test modules — `execute/tests.rs` (1 786 lines) and `catalog/tests.rs` (526) — so the brief's test-only-`pub` and `#[ignore]`-research-harness angles are unexamined for this group. Roughly half the group's
lines are unaudited; the simulation-orchestration third of the brief's remit
is essentially untouched. `feeds_hints` and `optimization_surface` return
power-session values — I audited the match shape only and proposed no change
to any number or axis binding.
