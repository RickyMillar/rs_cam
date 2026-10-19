# Error-handling debt — findings (2026-10-19)

Category: `error-handling`. Evidence baseline 99b6fb41, branch `geo-select`.
Every fact below comes from `rg` or a file read. Caller counts come from `rg`.
The order is by tier, then by cost.

Grades: TYPES | LINT | SENTRY | DOC | CONVENTION | NONE.

---

## EH-1 (B, S) — the export load gate drops its advisory note at both production doors

**Sites**
- `crates/rs_cam_core/src/gcode/mod.rs:1201` `let _note = enforce_load_policy(report, &policy)?;`
- `crates/rs_cam_core/src/gcode/mod.rs:1331` the same line in the multi-setup door
- `crates/rs_cam_core/src/gcode/mod.rs:922-931` with `accept_exceeded = true` the refusing lines are not in the return value
- `crates/rs_cam_core/src/gcode/mod.rs:853-858, 1131-1136` the contract text

**Claim.** The doc of `enforce_exceedance_policy` says `Ok(Some(note))` is
a line "the caller logs" and "the operator must still see". The two
production callers bind the note to `_note` and drop it. `rg -n "\b_note\b"
crates` gives only these two sites. The weak arm is reachable now:
`tool_load/verdict.rs:2070` builds `BoundSource::RigidityRuleOfThumb` from
`DepthVerdict::Exceeds` (`tool_load/depth.rs:288`). The doc at
`gcode/mod.rs:864` still says that arm "has no producer". Also, an override
(`accept_exceeded = true`) returns only the advisory block, so the record of
what the operator overrode is lost too.

**Fix.** Return a `#[must_use] LoadPolicyOutcome { advisories, overridden }`
and carry it inside the export result (`ExportedGcode`), so a door that
ignores it has to destructure it. Correct the stale doc at `:864`.
Now: DOC. After: TYPES. In flux.

## EH-2 (B, S) — `rs_cam_cli project --emit-gcode` accepts both load overrides with no flag

**Sites**
- `crates/rs_cam_cli/src/project.rs:1046-1049` `accept_unmodeled: true, accept_exceeded: true`
- `crates/rs_cam_cli/src/main.rs:211-217` the flag doc (no word about the overrides)
- `crates/rs_cam_core/src/gcode/mod.rs:929` the core text calls the override "known-dangerous"

**Claim.** `crates/rs_cam_cli/CLAUDE.md` says CLI G-code must use the core
path "including ... explicit safety overrides". This door hard-codes both
overrides. An exceeded tool load therefore writes a `.nc` file with no
refusal, no log of the override, and (by EH-1) no note. The flag doc says
the output "matches what the GUI export would produce", but the GUI refuses
the same state. Compare `run.rs:209-221`, which accepts only the unmodelled
tier and says why.

**Fix.** Add `--accept-exceeded` (default false) and `--accept-unmodeled`.
Give `ToolLoadExportPolicy` private fields and named constructors
(`ToolLoadExportPolicy::refuse_all()`, `::operator_accepts(Override)`), so a
literal `true` cannot be typed at a call site.
Now: CONVENTION. After: TYPES. Not in flux.

## EH-3 (B, S) — the project loader binds a missing `tool_id` or `model_id` to id 0

**Sites**
- `crates/rs_cam_core/src/session/project_file.rs:1030-1031` `tool_id: tp.tool_id.unwrap_or(0)`, `model_id: tp.model_id.unwrap_or(0)`
- `crates/rs_cam_core/src/session/project_file.rs:699-701` both fields are `Option<usize>` with `#[serde(default)]`
- `crates/rs_cam_core/src/session/mutation/config.rs:761` and `mutation/entities.rs:118` the first tool gets id 0

**Claim.** `session/CLAUDE.md` says "a model id and a tool id are EXACT (no
first-item fallback)" (P2.4/P2.5). The loader refuses a toolpath with no
`operation` table (`SessionError::MissingOperation`, "rs_cam does not invent
the parameters"). But a toolpath with no `tool_id` key loads bound to the
first tool, with no `ProjectLoadWarning`. Save always writes `Some`
(`save.rs:226-227`), so a hand-edited or agent-written file triggers it.

**Fix.** Make both fields required (breaking change is ruled fine), or refuse
with a typed `SessionError::MissingBinding { toolpath, field }`.
Now: NONE. After: TYPES. In flux.

## EH-4 (B, L) — typed errors become `String` at each layer boundary

**Sites**
- `crates/rs_cam_core/src/session/compute.rs:753` every `OperationError` except `Selection` becomes `SessionError::OperationFailed(e.to_string())` (loses `Cancelled`, `OverBudget`, `InvalidTool`, `MissingGeometry`)
- `crates/rs_cam_core/src/session/compute.rs:950,965`; `session/compute/generation.rs:129,415,434`; `session/compute/diagnostics.rs:367`; `session/shoe.rs:182`; `session/selection_status.rs:193` the same `OperationFailed(e.to_string())`
- `crates/rs_cam_core/src/session/mutation/config.rs:603` and `mutation/entities.rs:401` typed validation errors become `InvalidParam(e.to_string())`
- `crates/rs_cam_core/src/session/compute/export.rs:71` `ExportError` becomes `SessionError::Export(e.to_string())`
- `crates/rs_cam_core/src/compute/simulate.rs:2370,2379` the `ObstacleRefusal` enum (`compute/obstacle.rs:239`) becomes `SimulationError::ObstacleRefused(String)`
- `crates/rs_cam_core/src/io/mod.rs:79-117` `load_geometry` turns `MeshError`/`SvgError`/`DxfError`/`StepImportError` into `format!("STL load failed: {e}")`; `SessionError::ModelLoad { detail: String }` carries it
- `crates/rs_cam_core/src/session/mod.rs:141-260` 63 `InvalidParam`, 38 `OperationFailed`, 18 `MissingGeometry` producers in core src
- `crates/rs_cam_core/src/session/mod.rs:330` `impl std::error::Error for SessionError {}` has no `source()`, so `Io` and `Simulation` drop their chain
- `crates/rs_cam_viz/src/compute/mod.rs:441-455`, `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:256-265`, `crates/rs_cam_viz/src/compute/worker.rs:542`

**Claim.** The cause type exists one layer down and is lost at the boundary.
The cost is visible: two side channels exist only because the type lost the
variant. `map_session_error` reads the cancel FLAG because "SessionError
carries no cancel variant" (`worker/execute/mod.rs:250-255`), and the lane
re-reads `guard.stop_reason()` for the budget stop (`worker.rs:404,542`).
Tests can only match text: `gcode/mod.rs:2815-2876,2988,3033,3079`. Map
leads core-session #16 and core-io-output #33 record parts of this.

**Fix.** `SessionError::Operation(OperationError)`,
`SessionError::Export(ExportError)`, `SimulationError::ObstacleRefused(ObstacleRefusal)`,
`ModelLoad { name, cause: ModelLoadError }` with `#[from]` per door. Delete
the `.to_string()` flattens. Implement `source()` (or move to `thiserror`).
Now: CONVENTION. After: TYPES. In flux (session, simulate, io/mod.rs).

## EH-5 (B, M) — `SessionError::MissingGeometry` is three meanings and is used as control flow

**Sites**
- `crates/rs_cam_core/src/session/compute/diagnostics.rs:285-289` the "nothing to check" collision answer is an `Err(MissingGeometry)`
- `crates/rs_cam_core/src/session/compute/diagnostics.rs:556`, `crates/rs_cam_cli/src/project.rs:778` match it as "the 2D case, expected"
- `crates/rs_cam_core/src/session/compute.rs:2612` `generate_all` treats it as "skip"
- `crates/rs_cam_core/src/session/mutation/entities.rs:77`, `session/mutation/toolpath.rs:669` "Model index N not found" uses the same variant
- `crates/rs_cam_viz/src/app/mcp/commands.rs:1923` picks the refused field from it (`"model_id"`)

**Claim.** One variant means "no model with that id", "the model has no
mesh" and "the check does not apply". Three readers branch on the variant.
A new producer of `MissingGeometry` on the collision path reads as
`NotApplicable` and the CLI hides it. The op-level
`OperationError::MissingGeometry` arrives as `OperationFailed` (EH-4), so
`generate_all`'s skip arm sees only the session-level checks.

**Fix.** `collision_check` returns `Ok(CollisionCheck::NotApplicable)`.
Add `SessionError::ModelNotFound(ModelId)`. Keep `MissingGeometry` for "the
model carries no mesh".
Now: CONVENTION. After: TYPES. In flux.

## EH-6 (B, M) — GUI apply doors drop a core refusal with no trace

**Sites**
- `crates/rs_cam_viz/src/controller/events/mod.rs:100-112` `apply_quietly`: `Err(_) => None`, no log
- 17 `let _ = self.apply_quietly(..)`: `controller/events/undo.rs:25,50,97,122,153,168,186,208`; `controller/events/toolpath.rs:308,329,366,397,450`; `controller/events/model.rs:411,694,870`; `controller/events/mod.rs:270`
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:397-400` and `model.rs:869-880` after a refused `MoveToolpathToSetup` / `RemoveModel` the code still marks the project edited and clears the selection
- `crates/rs_cam_viz/src/ui/properties/panel_apply.rs:239-241` a refused panel edit is a `tracing::warn!` only
- `crates/rs_cam_viz/src/ui/properties/mod.rs:834` `.ok()?`; `:1609` `if let Ok`
- `crates/rs_cam_core/src/session/command.rs:1100-1102` `Effects` is `#[must_use]` and its doc tells the caller to write `let _ =`, which also drops the `Err`

**Claim.** Three GUI doors treat one refusal three ways: a toast
(`apply_controller_command`), a log (`apply_panel_command`), or nothing
(`apply_quietly`). `SetMachineHead` (`mutation/config.rs:602`),
`SetToolpathShoe`, `MoveToolpathToSetup` (`mutation/toolpath.rs:555,558`) and
`RemoveModel` (`mutation/entities.rs:77`) can refuse, and their undo/redo and
event arms discard the answer. An undo then pops history with no change to
the session. The `#[must_use]` on `Effects` cannot catch this, because the
sanctioned `let _ =` discards the whole `Result`.

**Fix.** Make `apply_quietly` return `Result<Effects, SessionError>` and log
every `Err` at warn in the one door. Deny `clippy::let_underscore_must_use`
in `controller/` and make a caller write `if let Err(e)` or an explicit
`.ok()` with a reason.
Now: CONVENTION. After: LINT. In flux (events/mod.rs, undo.rs, toolpath.rs, panel_apply.rs, properties/mod.rs).

## EH-7 (B, S) — the CLI smoke harness drops the resolution refusal and stamps the row anyway

**Sites**
- `crates/rs_cam_cli/src/smoke.rs:751-755` `let _ = session.apply(SetSimulationResolution(..))` with no `?`
- `crates/rs_cam_cli/src/smoke.rs:610-612` the row records `resolution_mm = format!("{resolution}")`
- `crates/rs_cam_core/src/session/rest_stock.rs:604-611` the setter refuses a cell that is not finite and positive
- `crates/rs_cam_cli/src/main.rs:328-330` `resolution: f64` with no value parser
- `crates/rs_cam_cli/src/sweep.rs:328,377` `let _ = std::fs::write(..)` and `let _ = img.save(..)` for sweep artifacts

**Claim.** `--resolution 0` (or `nan`) is refused by core and the refusal is
dropped. The run measures at the stored cell, and the row names a cell that
was never used. The smoke doc at `smoke.rs:609` says "resolution is a
measurement without its instrument setting". The sweep artifacts fail to
write silently.

**Fix.** `?` the apply. Parse `--resolution` into a `PositiveMm` newtype at
the clap boundary. Return `Result` from `write_toolpath_svg` and the PNG
save. Now: NONE. After: TYPES. In flux (smoke.rs).

## EH-8 (B, M) — the T-4 sentinel shape recurs in the axial envelope

**Sites**
- `crates/rs_cam_core/src/feeds/cutter_constraints.rs:262-284` `invert_deflection -> f64`: an absent chip returns the upper bracket ("deflection does not bind"); bad inputs (`limit <= 0`, `stickout <= 0`) return `0.0`, the same value as "no depth is safe"
- `crates/rs_cam_core/src/feeds/suggest/axial_envelope.rs:153-158` `spindle_rpm().unwrap_or(0)`, so `feed_per_tooth_mm = 0.0` when the op holds no RPM
- `crates/rs_cam_core/src/feeds/predict.rs:462-474` `lateral_cutting_force(..).unwrap_or(0.0)` (a breakdown figure only; the predicted deflection comes from the integrated model)

**Claim.** T-4 (register, closed 2026-09-18) made a deflection refusal an
`Err` (sentry `a_refused_deflection_is_not_a_zero_g_t4`). The inverse
function still encodes "unmodelled" as two numbers. 24 op configs default
`spindle_rpm: None` (`operation_configs.rs`). When pass 0 runs on such an op,
the deflection cap does not bind and the envelope reads as "no limit". I did
not verify whether the caller writes an RPM before pass 0.

**Fix.** `invert_deflection -> Result<f64, DeflectionUnmodeled>` and an
`Option<Rpm>` that the envelope must match. Now: DOC. After: TYPES.
Not in flux. known_id T-4.

## EH-9 (C, S) — `ProjectSession::generate_all` cannot fail and has no production caller

**Sites**
- `crates/rs_cam_core/src/session/compute.rs:2587-2622` returns `Result<(), SessionError>`; every error is a `tracing` line; the end is `Ok(())`
- callers: `crates/rs_cam_core/src/session/mod.rs:9` (doc example) and tests only: `tests/union_coverage_m1.rs:91` (`.expect("generate_all")`), `tests/project_curve_deviation.rs:226`, `tests/simulation_issue_channel_m1.rs:521,535`, `tests/arcfit_gate_population_d4.rs:693,700`, `tests/a_descent_over_two_overlapping_swaths_g_staletop.rs:265`

**Claim.** The signature says a failure reaches the caller. It never does.
The `.expect` in `union_coverage_m1.rs` cannot fire, so a failed op in that
fixture passes the test. It also walks the configs in index order, not
`generation_plan::plan`. The debt scanner missed it because the name
`generate_all` also appears in viz production code.

**Fix.** Delete it. Give tests a helper that walks the plan and returns
`Vec<(ToolpathId, SessionError)>`. Now: NONE. After: TYPES. In flux.

## EH-10 (C, S) — `VizError` keeps four typed variants that nothing builds

**Sites**
- `crates/rs_cam_viz/src/error.rs:8-33` `StlImport`, `SvgImport`, `DxfImport`, `StepImport` with `#[from]`; doc "Preserves typed error information from core"
- `crates/rs_cam_viz/src/io/import.rs:14-16` every import error becomes `VizError::Other(err.to_string())`
- `crates/rs_cam_viz/src/error.rs:52-60` `From<String>` and `From<SessionError> -> ProjectLoad`
- `crates/rs_cam_viz/src/controller/io.rs:81,231,287,401,411` five more `VizError::Other`

**Claim.** `rg` finds no construction of the four import variants outside
`error.rs`. Core never hands viz those types (EH-4: `load_geometry` turns
them into strings first). The doc and the variants read as load-bearing.
`ProjectSave` also has no constructor; saves use `Other(format!("Save
failed: .."))`. `Other` says "escape hatch for incremental migration".

**Fix.** Delete the four import variants and `ProjectSave` now, or land EH-4
and map `ModelLoadError` into one `Import` variant. Delete `From<String>`.
Now: NONE. After: TYPES. Not in flux.

## EH-11 (D, M) — `ExportError` is one string for five refusals, and its text holds MCP spellings

**Sites**
- `crates/rs_cam_core/src/gcode/mod.rs:36-53` `pub struct ExportError { message: String }`
- producers: `gcode/mod.rs:931` (tool load exceeded), `:1127` (unmodelled or stale simulation), `:1212` (inch units), `:1244` (controller compensation)
- `crates/rs_cam_core/src/gcode/mod.rs:929,1121-1124` text says "Pass `accept_exceeded=true`" and "`accept_unmodeled=true`"
- `crates/rs_cam_viz/src/io/export.rs:638,936,1023` and `crates/rs_cam_core/src/session/compute/export.rs:71` wrap it again as a string

**Claim.** A caller cannot tell an overridable load refusal from a units
refusal except by text. The core sentence names MCP parameter spellings, so
the GUI and the CLI show an operator a flag that does not exist on their
surface (map lead core-io-output #28). The knowledge "which override fixes
which refusal" lives in a string and in each surface.

**Fix.** `enum ExportRefusal { LoadExceeded{..}, LoadUnmodeled{stale: bool, ..},
InchUnits, ControllerCompensation{..} }` with `fn override_needed() ->
Option<Override>`. Each surface renders its own flag name.
Now: NONE. After: TYPES. In flux.

## EH-12 (E, M) — an MCP refusal is a string in a success reply, in three shapes

**Sites**
- `crates/rs_cam_viz/src/mcp_bridge.rs:1095-1097` `McpResponse { result: Result<String, String> }`; `Err` only for a closed channel or missing tracking (`app/mcp/generation.rs:996`, `simulation.rs:169,212`, `view.rs:297`, `controller/events/compute.rs:1292,1654`)
- shapes: `mutation_error_json` (43 calls), a top-level `"error"` key (63 hits in `app/`, `controller/`, `mcp_server.rs`), plain text (`app/mcp.rs:832,913,938,966,974` `Export failed: ..`; `app/mcp/commands.rs:1940` `Save failed: ..`)
- `crates/rs_cam_viz/src/mcp_bridge.rs:1124-1140` `from_json_response`: text that is not JSON is `Succeeded`
- `crates/rs_cam_viz/src/mcp_server.rs:400-411` `format_result`; 29 tools return `String`, so no tool sets the protocol error flag
- `crates/rs_cam_viz/src/app/mcp/commands.rs:1914,1924` `_ =>` picks `"index"` for every other `SessionError`

**Claim.** The toast classifier and the agent both parse text to learn about
a refusal. The guards hold today (`SaveProject` states its outcome), but a
new plain-text refusal in a toast-bearing arm reads as success. No break
found.

**Fix.** `enum McpReply { Ok(Value), Refused { summary, field } }` from every
handler; the server maps `Refused` to the MCP error result in one place.
Now: CONVENTION. After: TYPES. In flux.

## EH-13 (E, M) — `OperationError::Other(String)` mixes internal defects with operator refusals

**Sites**
- `crates/rs_cam_core/src/compute/execute.rs:54,263-266` `Other(String)` and `impl From<String> for OperationError`
- defects: `execute.rs:515` (`config_guard!`, 24 uses), `execute/prologue.rs:180,264`, `execute/height_field.rs:109,306`, `session/compute/generation.rs:772`
- operator refusals: `execute/curve_engrave.rs:95` (glue gap vs pocket depth), `execute/finish_3d.rs:509` (a dial outside its range)

**Claim.** A wiring defect and a bad dial reach the operator as the same
variant. `From<String>` turns any `?` on a `Result<_, String>` into `Other`
with no mark at the call site. Map leads core-compute #9, #10, #15.

**Fix.** `OperationError::InvalidParam { name, value, range }` for the dial
refusals. Move the defects to typestate (map #10) or an `Internal` variant.
Delete `From<String>`. Now: CONVENTION. After: TYPES. In flux.

## EH-14 (E, M) — `Result<_, String>` in library APIs

**Sites**
- `crates/rs_cam_core/src/settings/mod.rs:543,680,833,960` (warnings are `Vec<String>`)
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:545` `validate_observation`
- `crates/rs_cam_mcp/src/server.rs` 4 helpers; `crates/rs_cam_viz/src/mcp_server.rs` 3; `crates/rs_cam_mcp_proxy/src/{proxy.rs,config.rs,procinfo.rs,log.rs}` 10, plus `proxy.rs:1064` `Result<Value, Value>`
- `crates/rs_cam_cli/src/memory_budget.rs:35`
- 52 `Result<_, String>` signatures in `crates/*/src`

**Claim.** No production caller branches on these errors today; each is
shown as a sentence. The style differs from the typed enums beside them
(`ItemError`, `MeshError` use `thiserror`; `SessionError` uses a hand
`Display`). No break found.

**Fix.** One `thiserror` enum per module (`SettingsError`, `LutError`).
Keep `String` only at the MCP wire edge. Now: NONE. After: TYPES. Mostly not in flux.

## EH-15 (E, S) — a serialise failure folds to an empty string inside change detection

**Sites**
- `crates/rs_cam_core/src/session/mod.rs:911-922` `generation_inputs_signature` uses `serde_json::to_string(..).unwrap_or_default()` three times
- `crates/rs_cam_core/src/compute/simulate.rs:939` `hash_operation_config` uses `to_vec(op).unwrap_or_default()`

**Claim.** Two configs that both fail to serialise compare equal, so an edit
would not invalidate. `serde_json` writes `null` for a non-finite float, so
the arm is close to unreachable for these structs. The debt is that the code
claims a fallback it does not need, and NaN and `None` both become `null`.
(`trace/semantic_trace.rs:504` drops a failed value too, but its doc states
it on purpose.)

**Fix.** A typed digest (destructure plus a `Hash` impl over `f64::to_bits`),
which also forces a new field in. Now: NONE. After: TYPES. In flux.
