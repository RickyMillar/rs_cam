# core-session — design and feature-debt audit

Group: `crates/rs_cam_core/src/session/` (21 files, 21 292 lines) —
`ProjectSession`, the command registry, mutation, compute, project file.
Read-only pass, 2026-09-17.

### SES-01 Hand-rolled `Serialize` impls duplicate what `derive` already proves
- kind: design
- pattern: one concept two representations
- where: `crates/rs_cam_core/src/session/mod.rs:2093-2195` (five `impl serde::Serialize` blocks for `ToolpathDiagnostic`, `VerdictEvidence`, `Verdict`, `ProjectDiagnostics`; `VerdictSeverity`/`VerdictKind` are legitimate custom string tags)
- evidence: read `impl serde::Serialize for ToolpathDiagnostic` (mod.rs:2093-2129), `for VerdictEvidence` (2148-2157), `for Verdict` (2159-2171) and `for ProjectDiagnostics` (2173-2195). Every one calls `serialize_field` once per struct field, in exactly declared field order, with the exact Rust identifier as the JSON key — no rename, no `skip_serializing_if`, no flattening, no computed field. `rg -c "s.serialize_field" crates/rs_cam_core/src/session/mod.rs` → 27 calls across the four hand-written impls.
- proposal: replace the four impls with `#[derive(Serialize)]` on `ToolpathDiagnostic`, `VerdictEvidence`, `Verdict` and `ProjectDiagnostics` (keep the two enum impls, or derive them with `#[serde(rename_all = "snake_case")]`). Saves ~100 lines and removes a manual-sync hazard: today a field added to one of these structs compiles without a serializer error and is silently dropped from the MCP/CLI wire — the opposite of the `None`-means-not-measured discipline the doc comments insist on everywhere else in this file.
- breaks: none (the derived output is byte-identical to the current output — field names, order and `null` semantics for every `Option` are unchanged)
- effort: S
- risk: low — the round-trip is easy to characterize with a snapshot test before and after
- sentry: none exists today; write a test that serializes a populated `ProjectDiagnostics` and asserts the JSON has every current key, then convert to `derive` and rerun it unchanged
- owner:

### SES-02 Session-to-project-file translation is two hand-written god functions with no shared field list
- kind: design
- pattern: one concept two representations
- where: `crates/rs_cam_core/src/session/save.rs:141` `to_project_file` (141-344, 203 lines) and `crates/rs_cam_core/src/session/project_file.rs:882` `build_session_from_project` (882-1137, 255 lines)
- evidence: `to_project_file` builds `ProjectToolpathSection` from `ToolpathConfig` field-by-field (save.rs:227-251, 15 named fields incl. `feeds_provenance`, `rest_analysis`, `planner_origin`); `build_session_from_project` performs the inverse mapping the same way. Neither function is generated from, or checked against, the other's field list — `ProjectFile` itself is `#[derive(Serialize, Deserialize)]` (project_file.rs:24), but the session⇄file boundary is not.
- proposal: no code change proposed (a serde-round-trip refactor of a 250-line loader is out of scope for a read-only audit and risks the exact drift this finding warns about). Record it as a known risk and add the sentry below so a field that is written on one side and forgotten on the other fails a test instead of shipping.
- breaks: none (documentation-only proposal)
- effort: M
- risk: medium — a forgotten field here is a silent project-file data loss, not a compile error; `ToolpathConfig` alone carries 18 fields (mod.rs:865-905) that must agree in both functions
- sentry: `session/save.rs` has per-feature round-trip tests (`toolpath_round_trip`, `stock_config_round_trip`, `multi_setup_round_trip`, `setup_pause_message_round_trip`, `toolpath_with_spindle_rpm_round_trip` — save.rs tests module) but none asserts every field of `ToolpathConfig` survives a round trip generically; a reflective or macro-generated "every field changes the signature or the file" test would close the gap
- owner:

### SES-03 `mod.rs` facade carries ~340 lines of diagnostic-result types inline
- kind: design
- pattern: god module
- where: `crates/rs_cam_core/src/session/mod.rs:1187-1421` (`ToolpathDiagnostic`, `VerdictSeverity`, `VerdictKind`, `VerdictEvidence`, `ProjectEvidence`, `Verdict`, `ProjectDiagnostics`) plus their five `Serialize` impls at `mod.rs:2093-2195`
- evidence: `crates/rs_cam_core/src/session/compute/diagnostics.rs:1-19` already holds the logic that PRODUCES these types (`run_collision_check`, the air-cut/plunge-stress scans) and imports them back from `crate::session::{ProjectDiagnostics, ProjectEvidence, ... }` — i.e. the split done for every other subsystem (compute logic in `compute/`, mutation logic in `mutation/`) was not done for this one: the types stayed in the 2 852-line `mod.rs` while the P4 split moved their logic out to `compute/diagnostics.rs`.
- proposal: move the seven type/enum declarations and their `Serialize` impls into a new `session/diagnostics_types.rs` (or fold into `compute/diagnostics.rs`, which already owns the logic and already imports them), and re-export from `mod.rs`. Shrinks the facade by ~440 lines with no behavior change.
- breaks: none — same crate-public path if re-exported, or a rename of the import path if not (already ruled acceptable: "no legacy support")
- effort: S
- risk: low — pure move, no logic change
- sentry: `cargo test -p rs_cam_core -q --test stale_set_has_one_answer_wp28` and the existing diagnostics unit tests in `compute/tests.rs` do not touch file layout; a plain `cargo check` after the move is the real verification (out of scope here, read-only)
- owner:

### SES-04 `execute_job` is a 412-line function with four visible phase seams already named
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/session/compute.rs:472-883` (`pub fn execute_job`)
- evidence: read the whole function. It already narrates its own structure through `observer.set_phase(...)` calls at four points: `context.op_label` (generation, line 490), `"Apply dressups"` (line 540), `"Clip to boundary"` (line ~643), `"Compute stats"` (line ~807, exact line via `grep -n 'observer.set_phase' crates/rs_cam_core/src/session/compute.rs` inside the function body). Each phase already reads/writes a bounded slice of local state (`annotated`, `findings`, `channels`, `feed_opt_stock`) rather than the whole function's locals.
- proposal: extract each phase into a private function taking `&GenerateToolpathHandle`, `&GenObserver`, the running `annotated`/`findings`/`channels` state, and returning the next state — mirroring the split `session/compute/generation.rs`'s doc comment already describes ("the recorders + dressup/persist tail"). This is a pure decomposition; no behavior changes. Cuts the top-level function to roughly 60-80 lines of orchestration.
- breaks: none — private helpers, same public signature
- effort: M
- risk: medium — the function is the one path every generated toolpath goes through (`GenerateToolpath` job, all three surfaces reach it per `command.rs:754-760`), so a slip here is felt everywhere; the existing golden/acceptance tests (`tests/perf_golden_*.json`, the `_f0{24,26,27,28,31}` family) are the regression net
- sentry: `cargo test -p rs_cam_core -q --test resolved_gen_inputs_has_one_producer` plus the acceptance-loop `_f0*` tests exercise this path already; no new sentry needed for a pure extraction, but a "phase boundaries are named the same four strings" test would catch an accidental phase reordering
- owner:

### SES-05 Fixtures, keep-out zones and plan reordering are entirely unreachable from MCP and CLI
- kind: feature-debt
- pattern: missing surface for an existing command
- where: `crates/rs_cam_core/src/session/command.rs:257-265` (`MoveToolpathToSetup`... reachable), `:529-620` (`ReorderToolpath`, `AddFixture`, `RemoveFixture`, `ReplaceFixture`, `AddKeepOut`, `RemoveKeepOut`, `ReplaceKeepOut`)
- evidence: six `Surfaces` literals read `mcp: Reach::Skip("no MCP tool writes a fixture; the wire has no such mutation")` / `"...a keep-out zone..."` / `"no MCP tool re-orders the plan; the wire has no such mutation"`, each paired with `cli: Reach::Skip("the batch CLI exposes no such command")` — all six commands show `gui: Reach::Reached` and both other surfaces `Skip`. Cross-checked against the live MCP tool list for this session (`mcp__rs-cam__*`, 60+ tools): there is no `add_fixture`, `remove_fixture`, `replace_fixture`, `add_keep_out`, `remove_keep_out`, `replace_keep_out` or `reorder_toolpath` tool.
- proposal: not proposing new wire tools (out of scope for a read-only session-only audit — the wire schema is `rs_cam_mcp`'s and the tool registration is `rs_cam_cli`'s/viz's), but flagging that any MCP-driven workflow (including agent-automated ones, like the one running this audit) cannot configure workholding, keep-out safety zones, or plan order without a human at the GUI. Given the crate's own collision-safety invariants (holder/fixture clearance checks read `Fixture`/`KeepOutZone` at simulation time), this is a real automation gap, not just a nicety.
- breaks: none — additive
- effort: L (new wire types in `rs_cam_mcp`, tool registration in the MCP server, schema docs — outside this group)
- risk: low to add, since the `Command` rows, setters and invalidation already exist and are exercised by `mutation/tests.rs`
- sentry: `cargo test -p rs_cam_core -q --test command_registry_completeness` already enforces that any new MCP row states its `Surfaces` truthfully; no core-side sentry is missing, the gap is upstream in `rs_cam_mcp`/`rs_cam_cli`
- owner:

### SES-06 `plan_multitool_finishing` is a second 267-line function doing validate+resolve+build in one body
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_core/src/session/multitool.rs:306-573`
- evidence: read the function. It runs, in order: `self.validate_multitool_spec(spec)?` (310), ladder resolution + `TierLadder::new` validation (315-329), `bottom_z` height derivation (331-347), band-advisory lookup (349-353), plan-id allocation and old-plan removal (360-361), then a per-tier loop building each `ToolpathConfig` (363-onward, continues past line 400). Five distinct concerns share one function and one set of locals (`plan_mesh`, `heights`, `model_bbox`, `plan_id`).
- proposal: same shape as SES-04 — extract the pre-loop setup (ladder + heights + band advisories + plan-id) into a small `MultitoolPlanContext` built once, and the per-tier body into a `build_tier_config(&ctx, tier, tool, spec)` helper the loop calls. No behavior change.
- breaks: none
- effort: M
- risk: medium — `tests/multitool_plan_emission_o1.rs` pins the exact stepover/cusp numbers this function produces, so an extraction must keep every intermediate value bit-identical
- sentry: `cargo test -p rs_cam_core -q --test multitool_plan_emission_o1`
- owner:

### SES-07 Tool-edit and model-refresh invalidation skip the downstream chain walk every other mutation path uses
- kind: design
- pattern: invalidation chain has two implementations that diverge
- where: `crates/rs_cam_core/src/session/mutation/config.rs:314-341` (`drop_tool_results`), `:365-377` (`drop_results_for_model`), vs. the canonical walker `crates/rs_cam_core/src/session/mutation/toolpath.rs:260-341` (`invalidate_output_dependents`, called through `invalidate_result_chain` at `toolpath.rs:238-247`)
- evidence: `session/CLAUDE.md:25-26` states the invariant: "A parameter, tool, model, stock or setup edit invalidates the affected cached result chain. Do not preserve an old result under new inputs." `invalidate_output_dependents` is the one place that walks same-setup downstream `StockSource::FromRemainingStock` consumers (toolpath.rs:283-304) and `BoundarySource::DerivedRestRegions` consumers (toolpath.rs:306-321) to a fixpoint (`loop { ... if newly.is_empty() { break } }`). It is called from `set_dressup_config` (mutation/config.rs:36), `set_toolpath_enabled` (toolpath.rs:353+), `RestoreToolpathSnapshot` and `ReplaceToolpathConfig` (command.rs, both via `invalidate_result_chain`/its own signature-gated call). By contrast `drop_tool_results` and `drop_results_for_model` each independently `filter` the toolpath list for a *direct* dependency (`tc.tool_id == tool_id` / `tc.model_id == model_id`) and call `self.drop_result(idx)` in a flat loop — never `invalidate_result_chain` or `invalidate_output_dependents` — so a same-setup `FromRemainingStock` or `DerivedRestRegions` toolpath that is only *indirectly* downstream of the edited tool/model keeps its stale cached result. Three live, surface-reachable callers hit the narrow path: `set_tool_param_impl` (compute/params.rs:559-561, `SetToolParam` row — `mcp: Reached`, `cli: Reached`), `replace_tool` (mutation/config.rs:576-589, `ReplaceTool` row — `gui: Reached`, the tool panel's Apply button), and `adopt_model_geometry` (mutation/config.rs, `AdoptModelGeometry` row — `gui: Reached`, all three model-refresh doors).
- proposal: route `drop_tool_results` and `drop_results_for_model` through `invalidate_output_dependents` for the directly-affected set (call it once per affected index, or extend `invalidate_output_dependents` to accept a *set* of seeds so the fixpoint loop runs once over all of them) instead of calling `drop_result` directly. This is the same fix shape WP1/N15 already applied to the MCP stale-set duplication this file's own comments describe.
- breaks: none — `Effects::stale` only grows (more honest), no caller currently depends on the narrower set
- effort: M
- risk: medium — touches three live production paths (two GUI, one MCP+CLI); needs a live check that a tool-diameter edit on an upstream roughing pass now correctly stales a downstream rest-machining pass
- sentry: `tests/mutation_paths_invalidate_alike_p0.rs` proves the *setter/snapshot/replace* trio agree (its own table, lines ~14-16), but names no arm for `set_tool_param`, `replace_tool` or `adopt_model_geometry` — `rg -n "invalidate_tool|set_tool_param|remove_tool|invalidate_model|replace_tool|drop_tool_results|drop_results_for_model" crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` returns only a comment-table mention, zero test arms. Add a fourth arm to that file: edit a tool's diameter (or replace it) upstream of a `FromRemainingStock` toolpath and assert the downstream index goes stale too.
- owner:

## Top three

1. **SES-07** (tool-edit/model-refresh invalidation skips the chain walk) —
   highest benefit per effort: it is a live, surface-reachable gap against
   this folder's own stated invariant ("a tool ... edit invalidates the
   affected cached result chain"), the fix is a small refactor onto an
   existing helper, and it comes with a ready-made place to add the missing
   test arm in an existing sentry file.
2. **SES-01** (hand-rolled `Serialize` impls) — smallest effort of any
   finding here (mechanical `derive` swap), removes a real silent-data-loss
   hazard on every future field addition, and is fully reversible.
3. **SES-04 / SES-06** (the two >200-line functions with visible phase
   seams, `execute_job` and `plan_multitool_finishing`) — same shape, same
   fix, moderate effort; grouped together because splitting one makes the
   pattern for the other obvious, and both already carry the sentries that
   would catch a regression during the split.

## Checked and clear

- `Command` is one enum with one `match` in `apply` (command.rs:2190-2433);
  `Query` and `Job` each get their own single-match door (`query` at
  command.rs:2440, `start` at command.rs:2520). No scattered dispatch.
- `Effects.stale` has exactly one construction site,
  `try_with_effects`/`with_effects` (command.rs:2562-2597), which diffs a
  revision snapshot rather than trusting a caller-supplied list. Every
  `Command` arm and every `mutation/*.rs` setter goes through it.
- `resolve_generation_inputs` (compute/generation.rs) and
  `ResolvedGenInputs` (compute.rs:57) are genuinely the one producer — no
  second assembly of tool/mesh/heights inputs exists in this group.
- `height_context_for_toolpath` (compute/export.rs:275) and the generation
  path both delegate to `SetupEvalContext::build_for_setup`, so the MCP
  diagnostics view and the actual generation math read the same stock-frame
  decision (F-030) — not a duplicated translation.
- `ProjectFile` and its section types (`project_file.rs:24-490`) are plain
  `#[derive(Serialize, Deserialize)]` with no residual `format_version`
  compatibility shims; `SUPPORTED_FORMAT_VERSION == 3` is the only version
  this build reads, refused otherwise (`check_format_version`,
  project_file.rs:52-59).
- `set_toolpath_param`'s `#[allow(dead_code)]` (compute/params.rs:119) looked
  like dead-code drift on first read; it is a deliberately kept wrapper a
  named sentry (`setters_have_rows_wp15a::every_wrapper_exemption_calls_the_door`)
  requires to exist, documented as such in its own doc comment.
- `planned_tier_boundary_polys` (multitool.rs:634) is pub and called only
  from `tests/multitool_preview_u1.rs` — but its own doc comment says so
  ("Test door... No production path reads it"), so this is a documented
  choice, not hidden debt.
- The `dead_pub_surface.md` entries for `ToolpathSummary`/`ToolSummary`
  (session/mod.rs:1001, 1026) do not hold up on inspection: `list_toolpaths()`
  and `list_tools()` are called from `rs_cam_viz/src/app/mcp/{project,commands}.rs`
  in production and serialized straight onto the MCP wire; the evidence file
  is tracking the qualified type name, not the (live) call sites that use it
  through inference.

## Add-a-thing count

Adding one new `Command` row (the group's main extension point) touches, at
minimum, inside this folder:

1. `command.rs` — one row in the `for_each_command!` list (the wire name,
   payload type, answer type and the `Surfaces` literal for all three
   product surfaces).
2. `command.rs` — the `*Args` payload struct, if the row needs one not
   already declared.
3. `command.rs` — one arm in `apply` (or `query`/`start` for the other
   kinds), almost always a one-line delegation to a setter.
4. The setter's home file — `mutation.rs`/`mutation/{entities,config,
   toolpath}.rs` for a CRUD write, or `compute.rs`/`compute/{params,
   generation,simulation,export,diagnostics}.rs` for a compute-adjacent one
   — where the actual field write and the `try_with_effects`/`with_effects`
   call live.

`tests/command_registry_completeness.rs` and
`tests/setters_have_rows_wp15a.rs` then fail at compile/test time if a row
or a setter is missing its counterpart, so the count above is enforced, not
just documented. Reaching a fourth surface (a new MCP tool, a CLI flag, a
GUI control) is additional work in `rs_cam_mcp`, `rs_cam_cli` and
`rs_cam_viz` respectively, outside this group.
