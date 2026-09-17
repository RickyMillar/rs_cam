# Design and feature-debt audit — cli-mcp

Group: cli-mcp — `crates/rs_cam_cli/`, `crates/rs_cam_mcp/`

### CLI-01 job.rs silently coalesces two differently-named stay-down keys, one of them dead in core
- kind: feature-debt
- pattern: stringly-typed door
- where: `crates/rs_cam_cli/src/job.rs:271-273,292-298,914-916`
- evidence: `job.rs:914` reads `op.max_stay_down_distance_mm.or(op.max_stay_down_dist)` and always emits the param key `"max_stay_down_distance_mm"`. `rg -n "max_stay_down_dist\b" crates/rs_cam_core/src/compute/operation_configs.rs` returns nothing — the live `OperationConfig` (the type every surface's `ParamDef` registry, GUI and MCP `set_toolpath_param` see) has only `max_stay_down_distance_mm`. A same-named but unrelated field `Adaptive3dParams::max_stay_down_dist` (`crates/rs_cam_core/src/adaptive3d/mod.rs:138`, read at `crates/rs_cam_core/src/adaptive3d/path.rs:646`) is a *different* mechanism (legacy default-link-distance heuristic, `tool_radius * 6`) and is hard-coded to `None` at all three construction sites (`adaptive3d/mod.rs` test, `path.rs:1793`, `compute/execute/finish_3d.rs:138`) — no surface, including this CLI, can set it.
- proposal: Drop the `max_stay_down_dist` TOML alias from `OperationDef` (it silently loses to `max_stay_down_distance_mm` when both are set, with no warning) and rename it if a job-file alias is still wanted so it cannot be confused with the dead core field of the same name. Separately (owner: not this group) either wire `Adaptive3dParams::max_stay_down_dist` to a real setter or delete the field.
- breaks: a job TOML using the `max_stay_down_dist` key alone still works if the alias is renamed with a deprecation match arm; removing it outright breaks any existing job file using that key (no legacy support is the operator ruling, so this is acceptable).
- effort: S
- risk: low — the alias is CLI-only sugar; no core signature changes needed for the job.rs half.
- sentry: `crates/rs_cam_cli/src/job.rs` test module (add a case asserting the TOML alias maps to `max_stay_down_distance_mm` and that setting both keys does not silently drop one).
- owner: blank (job.rs half); the dead `Adaptive3dParams` field is `adaptive3d/` territory (core-cutting group)

### CLI-02 job-file `OperationDef` is one 42-field flat struct standing in for a 24-op registry
- kind: design
- pattern: parameter struct with more than eight fields
- where: `crates/rs_cam_cli/src/job.rs:188-298` (struct), `crates/rs_cam_cli/src/job.rs:506-518` (`op_type_for`)
- evidence: `awk '/struct OperationDef/,/^}/' job.rs | grep -c "pub "` = 42 fields, all `Option<T>` (or bare with `#[serde(default)]`) on one struct regardless of operation. `op_type_for` (job.rs:506) hand-matches 6 literal strings (`"pocket"`, `"profile"`, `"adaptive"`, `"rest"`, `"adaptive3d"`, `"drop-cutter"/"drop_cutter"/"finish"`) against the 24-variant core registry (`grep -c "Config," crates/rs_cam_core/src/compute/catalog.rs` region shows 24 `*Config` imports at catalog.rs:7-11); its own doc comment admits this: "Anything else points the user at the generic registry-driven `run` subcommand (all 23 ops)."
- proposal: Either generate `OperationDef` fields per-operation from the core `ParamDef` registry (matching what `run`'s `--set key=value` already does generically), or drop the flat struct for a `[operation.params]` free-form TOML table validated the same way `run --set` is, so a new core operation does not require a new job.rs field and match arm.
- breaks: TOML job files with unknown top-level operation keys currently silently deserialize as unused fields (serde ignores unknowns by default unless `deny_unknown_fields` is set — not checked here, worth confirming); a free-form table changes the schema shape for every existing job file (all fields move under `[operation.params]`).
- effort: L
- risk: medium — job files are a documented user-facing format (`FEATURE_CATALOG.md`/README); a shape change needs a coordinated docs update.
- sentry: existing `crates/rs_cam_cli/src/job.rs` unit tests around TOML parsing; add one asserting an operation type outside the 6-item list gets the documented error, not a silent no-op.
- owner: blank

### CLI-03 `plan_multitool_finishing` has a 13-field MCP param struct with mode-encoding bools/strings, and no CLI surface at all
- kind: feature-debt
- pattern: parameter struct with more than eight fields
- where: `crates/rs_cam_mcp/src/server.rs:776-852` (`PlanMultitoolFinishingParam`)
- evidence: the struct has 13 fields, including `coarse_skips_fine_islands: Option<bool>` and `monotone_cell_decomposition: Option<bool>` (mode flags) and `tier_strategies: Option<Vec<String>>` (stringly-typed per-tier op choice: `"unified_finish" | "scallop" | "iso_scallop"`, not an enum). `rg -rn "multitool|plan_multitool" crates/rs_cam_cli/src` returns zero hits — the capability is live in core (`crates/rs_cam_core/src/session/multitool.rs`), the GUI (`crates/rs_cam_viz/src/controller/events/planner.rs`) and MCP (`server.rs`, `app/mcp/generation.rs`), but `rs_cam_cli` has no `job.rs` operation, no `run` subcommand and no test coverage reaching it.
- proposal: Add a `rs_cam_cli` entry point (a `run multitool-finish` subcommand or a job-file directive) that reaches the same `Command`-layer door `planner.rs` uses, so batch pipelines can drive the ladder planner without the GUI or an MCP client. If the capability is intentionally GUI/MCP-only, say so in `crates/rs_cam_cli/CLAUDE.md` so the gap is not re-discovered.
- breaks: none (additive)
- effort: M
- risk: low — the core door already exists; this is CLI wiring, not new planning logic.
- sentry: a new `crates/rs_cam_cli/tests/integration.rs` case that runs the ladder end to end from a job file or CLI flags.
- owner: blank

### CLI-04 `rs_cam_mcp::response` bounded-array primitive has exactly one real caller; other handlers hand-roll the same cap vocabulary with a different magic number
- kind: design
- pattern: duplicate helper
- where: `crates/rs_cam_mcp/src/response.rs:1-90` (primitives), `crates/rs_cam_viz/src/app/mcp/simulation.rs:548,666,717-801` (the one caller), `crates/rs_cam_viz/src/app/mcp/diagnostics.rs:798-847,898-900` (hand-rolled duplicate)
- evidence: `rg -rln "BoundedResponse|cap_json_values|CappedArray|ResponseBudget" crates/*/src` returns only `crates/rs_cam_mcp/src/response.rs` (the definitions) and `crates/rs_cam_viz/src/app/mcp/simulation.rs` (the only call site). `crates/rs_cam_viz/src/app/mcp/diagnostics.rs:798` reuses the constant `rs_cam_mcp::response::DEFAULT_MAX_TOP_LEVEL_SPANS` but hand-writes the `_total_matching`/`_returned`/`_truncated`/`_cap` fields (diagnostics.rs:826-844) instead of calling `cap_json_values`; the sibling "detail mode" 30 lines below (diagnostics.rs:898) hardcodes `max_spans.unwrap_or(50)` — a magic number with no named constant, distinct from response.rs's own `DEFAULT_MAX_SPAN_SUMMARIES`/`DEFAULT_MAX_TOP_LEVEL_SPANS` (both 200). Across `crates/rs_cam_viz/src/app/mcp/*.rs` there are 223 `json!(` call sites total (`generation.rs`16, `diagnostics.rs`22, `view.rs`13, `simulation.rs`29, `commands.rs`55, `project.rs`73, `tests.rs`15), almost none going through the shared budget type.
- proposal: Give `inspect_spans`' detail mode the same `cap_json_values` call the top-level branch is one step from using, and name its cap constant in `response.rs` instead of the literal `50`. This is the exact class of bug `response.rs`'s own doc comment says it exists to prevent (a 60 MB unbounded `get_cut_trace` response, 2026-08-08) — a second unbounded/inconsistently-capped array elsewhere is one census away from repeating it on a different tool.
- breaks: none if the field names match (`response.rs`'s vocabulary is the same one `diagnostics.rs` already copies by hand); a changed default cap value would break large-project agents relying on the current 50.
- effort: S
- risk: low — mechanical replacement, existing tests should pin the wire shape.
- sentry: `crates/rs_cam_mcp/src/response.rs`'s own unit tests, plus any `inspect_spans` detail-mode integration test in `rs_cam_viz`.
- owner: blank

### CLI-05 sweep's reported "baseline value" is a fabricated `"default"` for 20 of `OperationDef`'s 42 fields, not "not measured"
- kind: feature-debt
- pattern: guard/refusal reports the wrong thing
- where: `crates/rs_cam_cli/src/sweep.rs:60-61` (fallback), `crates/rs_cam_cli/src/sweep.rs:177-201` (`get_op_field`), `crates/rs_cam_core/src/export/fingerprint.rs:1732-1738` (`ParameterSweepResult::base_value`, the wire field)
- evidence: `awk '/fn get_op_field/,/^}/' sweep.rs | grep -c "=>"` = 23 arms (22 named fields + a `_ => None` catch-all) against `OperationDef`'s 42 `pub` fields (CLI-02's count). `sweep.rs:60` reads: `get_op_field(first_op, param_name).unwrap_or_else(|| serde_json::Value::String("default".to_owned()))`. Sweeping an uncovered field — e.g. `min_region_cut_length_mm`, `mill_shallow_areas`, `max_stay_down_distance_mm`, `spindle_speed`, `coolant`, `entry_3d`, `scale`, `prev_tool`, `tabs`/`tab_width`/`tab_height`, `setup` — still patches and runs correctly via the generic text-level `patch_toml_field` (sweep.rs:226), but the JSON report's `base_value` is the literal string `"default"` regardless of the field's real starting value, and this is the exact field an agent reads to know what the sweep varied *from*.
- proposal: Make `get_op_field` exhaustive over `OperationDef` (a `match field { ... }` that covers all 42 keys, or better, serialize `first_op` to a `toml::Value`/JSON map and index by field name generically instead of hand-listing each one) so an unknown/uncovered field is a hard error, never a silent placeholder.
- breaks: none — tightens a currently-silent path into either a correct value or an explicit error.
- effort: S
- risk: low — `patch_toml_field` (the mutation path) is already field-name-generic; only the read-back needs to match it.
- sentry: a `crates/rs_cam_cli` test sweeping `min_region_cut_length_mm` (or any of the other 19 uncovered fields) and asserting `base_value` is not `"default"` unless the field's actual value is the string `"default"`.
- owner: blank

### CLI-06 `nc-time` hardcodes the operator's personal machine profile; it cannot read the machine library GUI/MCP/core all share
- kind: feature-debt
- pattern: missing surface parity
- where: `crates/rs_cam_cli/src/nc_replay.rs:10,29,39` (hardcoded call), `crates/rs_cam_cli/src/main.rs:290-304` (`NcTime` clap args: only `inputs`, `max_feed`, `rapid_feed`)
- evidence: `nc_replay.rs:29` reads `let kinematics = MachineKinematics::shapeoko_xxl_ricky_tuned();` with no parameter to change it; the module doc (`nc_replay.rs:10`) states this in the present tense as "the user's calibrated `shapeoko_xxl_ricky_tuned` kinematics." `main.rs:290-304`'s `NcTime` variant exposes only `max_feed`/`rapid_feed` as separate flags — both of which already live inside the `MachineKinematics` struct the call ignores — with no `--machine` selector. `crates/rs_cam_core/src/io/machine_library.rs` exists and MCP exposes `load_machine_from_library` / `list_machine_library` (per `crates/rs_cam_mcp/src/server.rs:339`); `nc-time` has no path to either.
- proposal: Give `nc-time` a `--machine <name>` flag that loads from the same `io::machine_library` GUI/MCP use, defaulting to the current hardcoded preset only when no library is available, and drop the separate `max_feed`/`rapid_feed` flags in favour of reading them off the loaded `MachineKinematics` (they can still be overridable per-flag if a caller wants to test a hypothetical).
- breaks: `max_feed`/`rapid_feed`'s current default values (4000/10000) stay the same if the default profile is unchanged; a caller relying on those exact CLI flag names would need to keep using them if kept as overrides.
- effort: S
- risk: low — self-contained to one CLI subcommand; the machine-library reader already exists and is exercised elsewhere.
- sentry: a `crates/rs_cam_cli` test running `nc-time` against a library machine profile different from `shapeoko_xxl_ricky_tuned` and asserting the predicted time changes accordingly.
- owner: blank

### CLI-07 the `job` subcommand's whole body (382 lines) is inlined in `main()`'s match arm; every sibling command delegates to its module
- kind: design
- pattern: god function
- where: `crates/rs_cam_cli/src/main.rs:446-828` (`Commands::Job` arm), contrast `crates/rs_cam_cli/src/main.rs:828-857` (`Commands::Run`, 29 lines, pure delegation to `run::run_generic`), `:860-872` (`Commands::Sweep`, 12 lines, delegates to `sweep::run_sweep`)
- evidence: `sed -n '446,828p' main.rs` is 382 lines of path canonicalisation, per-setup export looping, debug-trace artifact writing and diagnostics-flag plumbing, all inline in `main()`, none of it in `job.rs` despite `job.rs` already owning `parse_job_file`/`execute_job`. Every other subcommand (`Run`, `Sweep`, `Project`, `Smoke`, `NcTime`) is a thin call into its own module (`run::run_generic`, `sweep::run_sweep`, etc.) — `Commands::Job` is the one outlier still shaped like pre-refactor code.
- proposal: Extract the `Commands::Job` arm body into a `job::run_job_command(...)` function (mirroring `run::run_generic`/`sweep::run_sweep`), leaving `main()` a thin dispatcher for every subcommand uniformly.
- breaks: none — pure code motion, no behaviour change.
- effort: S
- risk: low — mechanical extraction; `crates/rs_cam_cli/tests/integration.rs` already exercises `job` end to end and would catch a slip.
- sentry: `crates/rs_cam_cli/tests/integration.rs` (existing job-file tests)
- owner: blank

### CLI-08 tool-type has three vocabularies that mean the same five things and spell them differently on the wire
- kind: design
- pattern: one concept, two representations
- where: `crates/rs_cam_core/src/compute/tool_config.rs:86-92,974-981` (canonical `ToolType::parse_lenient`: `end_mill`, `ball_nose`, `bull_nose`, `v_bit`, `tapered_ball_nose`), `crates/rs_cam_cli/src/job.rs:55-64` (`CliToolType`: `flat`, `ball`, `bullnose`, `vbit`, `tapered_ball`), `crates/rs_cam_mcp/src/server.rs:1064-1069` (`parse_tool_type`, wraps `parse_lenient` for `AddToolParam.tool_type: String`)
- evidence: `job.rs:56-64` is a hand-written enum with its own `#[serde(rename_all = "snake_case")]` plus two explicit per-variant renames (`"bullnose"`, `"vbit"`) that do not call `ToolType::parse_lenient` at all — it has its own `to_tool_type()` mapping function (job.rs:79-88) instead. `crates/rs_cam_core/src/compute/tool_config.rs:975-981`'s own pinned test table shows the canonical tokens are `end_mill`/`ball_nose`/`bull_nose`/`v_bit`/`tapered_ball_nose`. A job TOML file's `type = "end_mill"` (the MCP/`run` spelling) is REJECTED by `job.rs`'s deserializer, and a `run --tool flat:6.35` (the job.rs spelling) is rejected by `run.rs`'s `parse_tool_spec` (which calls `ToolType::parse_lenient` directly, per `run.rs`'s own doc comment "goes through the unified `ToolType::parse_lenient` vocabulary (T8)"). The job-file example at the top of `job.rs` (line 15: `type = "flat"`) only works in that one surface.
- proposal: Delete `CliToolType` and its `to_tool_type()` translation; deserialize `ToolDef.tool_type` as the same `ToolType` (or a thin newtype over `parse_lenient`) every other surface already uses, so one token vocabulary works in `run --tool`, MCP `add_tool`, and job TOML alike.
- breaks: every existing job TOML using `flat`/`ball`/`bullnose`/`vbit`/`tapered_ball` breaks and must be rewritten to `end_mill`/`ball_nose`/`bull_nose`/`v_bit`/`tapered_ball_nose` (operator ruling 2026-09-16: no legacy support, breaking changes are fine — state the break in the commit).
- effort: S
- risk: low — mechanical; `job.rs`'s own module-doc example needs updating alongside any fixture job files.
- sentry: a `job.rs` test round-tripping every `ToolType::ALL` variant through `ToolDef` TOML parsing using the canonical token.
- owner: blank

### CLI-09 `set_setup_rotation`'s wire type is a bare `String`; the core type it becomes is a 4-variant enum, parsed by hand downstream
- kind: design
- pattern: unit-less/type-less wire value where a typed enum exists
- where: `crates/rs_cam_mcp/src/server.rs:36-42` (`SetSetupRotationParam.z_rotation: String`), `crates/rs_cam_core/src/compute/transform.rs:164-177` (`ZRotation` enum, `ZRotation::ALL`), `crates/rs_cam_viz/src/app/mcp/commands.rs:447` (hand parse)
- evidence: the MCP param type carries `pub z_rotation: String` with a doc comment enumerating the four legal values ("0", "90", "180", "270") in prose; the core command it feeds, `SetSetupRotationArgs` (`crates/rs_cam_core/src/session/command.rs:1217-1225`), takes `z_rotation: crate::compute::transform::ZRotation` — a real 4-variant enum with a canonical `ALL` list. Between them, `crates/rs_cam_viz/src/app/mcp/commands.rs:447` hand-parses with `p.z_rotation.trim().trim_end_matches("deg").trim()` before matching against the four values. `schemars::JsonSchema` on a `String` field emits an unconstrained `"type": "string"` in the tool's published schema — a client cannot discover the four legal values from the schema, only from the doc comment.
- proposal: Give `ZRotation` (or a thin MCP-side mirror deriving `schemars::JsonSchema` with `#[serde(rename_all = ...)]`) the wire type directly, so the published tool schema enumerates the four legal values and the hand-written `trim`/`trim_end_matches` parse in `commands.rs` is deleted.
- breaks: a caller currently sending `"90deg"` or `"90 deg"` (which the lenient hand-parse accepts) would need to send the exact enum token once a strict enum type is on the wire — acceptable per the no-legacy-support ruling, but worth stating in the commit.
- effort: S
- risk: low — one field, one call site.
- sentry: `crates/rs_cam_viz` MCP surface tests for `set_setup_rotation` (schema + behaviour).
- owner: blank

## Top three

1. **CLI-08** — tool-type has three vocabularies (`ToolType::parse_lenient`, `CliToolType`, MCP's raw string) that spell the same five tool shapes differently across `job` TOML, `run --tool`, and MCP `add_tool`. S effort, fixes a real cross-surface authoring trap (a job file and an MCP call for the "same" tool use different literal tokens) with a pure deletion of a redundant enum.
2. **CLI-05** — `sweep`'s JSON report fabricates the string `"default"` as `base_value` for 20 of 42 job-file fields instead of reporting the real value or refusing. S effort; this is silently wrong data in the artifact an agent reads to interpret a sweep, not a style complaint.
3. **CLI-01** — `job.rs` coalesces a legacy `max_stay_down_dist` TOML key into `max_stay_down_distance_mm` with `.or()`, silently dropping one if both are set, and the same string names a second, dead, unrelated core field (`Adaptive3dParams::max_stay_down_dist`, always constructed `None`). S effort to fix the CLI alias; the finding also flags a core-side dead field for the owning group.

## Checked and clear

- `run` is registry-driven end to end (`crates/rs_cam_cli/src/run.rs`): a brand-new core operation needs zero new CLI code, confirmed by its own doc comment and `FEATURE_CATALOG.md:191-192`. This is the pattern `job.rs` (CLI-02) and `sweep.rs` (CLI-05) do not follow.
- `crates/rs_cam_cli/src/command.rs::apply_command` is the one shared door all three CLI mutation sites (`job.rs`, `run.rs`, `smoke.rs`) take into `ProjectSession::apply` — no parallel mutation path, guarded by its own unit test.
- `crates/rs_cam_cli/src/project.rs`'s `ToolpathDiagnostic` shows up in `planning/structure_2026-09-17/evidence_round2/dup_sweep_0.88_src.md` at 0.9434 similarity to `rs_cam_core::session::mod.rs` L1184-1252, but it is a deliberate serde view with different wire key names, CLI-local fields the core diagnostic doesn't have, and an *exhaustive destructure* (`Self::from_core`) that makes a new core field a compile error here until published — fixed structurally at C3 (2026-08-02), pinned by a byte-stable wire test. Not duplication debt.
- `rs_cam_mcp`'s ~57 param structs are imported by name and reused directly in `crates/rs_cam_viz/src/mcp_server.rs` (`use rs_cam_mcp::server::{...}`, 40+ names) — no shadow copy in `viz`, no drift between the two crates.
- `String` errors returned by `crates/rs_cam_mcp/src/server.rs` (`parse_tool_type`, `resolve_material`, `parse_workholding_rigidity`, `parse_operation_type`, `build_tool_config`) are only ever interpolated into wire text via `mutation_error_json`/`preview_error`; nothing pattern-matches on their content, so the untyped error is not a hidden coupling.
- No `map_err(|_| ...)` cause-dropping, no `#[ignore]` test harnesses, and no `TODO`/`FIXME`/`HACK` comments in either crate (`rg` counts of each are zero).
- `smoke.rs::materialize_case_toolpath` and `sweep.rs::run_sweep` route every mutation through `Command::AddToolpath`/`Command::SetToolpathParam` via `apply_command` — no parallel ad hoc session-mutation path in either harness.
- Fingerprinting/diffing in `sweep.rs` and stock-composite rendering are properly reused from `rs_cam_core::export::fingerprint` (`ToolpathFingerprint`, `diff_fingerprints`, `render_stock_composite`) — not a duplicated implementation.
- `planning/structure_2026-09-17/evidence_round2/dead_pub_surface.md` and `test_only_pub_api.md` have zero entries under `rs_cam_cli` or `rs_cam_mcp`.

## Add-a-thing count

**Add an MCP tool that mutates the session** (traced via `plan_multitool_finishing`, `rg -rln "PlanMultitoolFinishing" crates/rs_cam_mcp/src crates/rs_cam_viz/src`):
1. `crates/rs_cam_mcp/src/server.rs` — new `...Param` struct (`Deserialize` + `schemars::JsonSchema`)
2. `crates/rs_cam_viz/src/mcp_server.rs` — import the param type, add the `#[tool(name = "...")]` method
3. `crates/rs_cam_viz/src/mcp_bridge.rs` — new `McpRequestKind` variant carrying the spec
4. `crates/rs_cam_viz/src/app/mcp.rs` — dispatch match arm routing the variant to its handler
5. `crates/rs_cam_viz/src/app/mcp/<domain>.rs` (e.g. `generation.rs`) — the handler that builds the JSON response
6. `crates/rs_cam_viz/src/controller/events/<domain>.rs` (e.g. `planner.rs`) — the actual `ProjectSession`-mutating door
(a 7th file, `crates/rs_cam_viz/src/ui_command.rs`, is touched only if the tool also needs a GUI-menu/keyboard route.)

**Add a job-file operation type to `rs_cam_cli`'s `job` subcommand** (contrast: `run` needs none of this, see above):
1. `crates/rs_cam_cli/src/job.rs` — new fields on the flat `OperationDef` struct, if the op takes new params
2. `crates/rs_cam_cli/src/job.rs::op_type_for` — new match arm mapping the TOML `type` string to `OperationType`
3. `crates/rs_cam_cli/src/job.rs::job_params_for` — new match arm building the registry param list
4. `crates/rs_cam_cli/src/sweep.rs::get_op_field` — new match arm, or the field silently reports a fabricated `"default"` baseline in sweep reports (CLI-05)
