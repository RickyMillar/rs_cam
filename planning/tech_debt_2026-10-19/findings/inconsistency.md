# Findings: inconsistent design patterns and naming (`inconsistency`)

Baseline: HEAD on branch `geo-select` (evidence baseline 99b6fb41). Read-only
sweep. The sources are `rg` and file reads at HEAD; the map tables in
`maps/*.md` were leads only. Each finding names the dominant pattern, the
deviating sites, the variant that must win, and the enforcement for it.

Method: the ten map recurring-pattern tables have about 13 shared concern rows
(errors, mutation, parameters, units, identifiers, cancellation, caching,
diagnostics, serialization, tolerances, geometry, tests, naming and
registration). A row that shows two or more variants across maps became a
candidate. Each candidate was checked in code. Tolerance and geometry-primitive
rows go to the `primitive-newtype` and `duplication` finders; test-double rows go
to `test-debt`.

"In flux" uses `instruments/in_flux_paths.txt`. No finding is a property of the
unfinished geometry-selection feature.

Summary table (most severe first):

| id | tier | cost | title | in flux |
|---|---|---|---|---|
| INC-01 | B | S | The viz re-derives the drill-family predicate and other op-type rules | yes |
| INC-02 | B | M | The post-apply epilogue has 6 named doors and about 15 inline copies | yes |
| INC-03 | B | M | Closed-set string tokens: 11 method names and 3 unknown-token policies | yes |
| INC-04 | B | M | `ALL` arrays: generated, proven, or bare; `diagnostics::ids::ALL` omits 8 ids | yes |
| INC-05 | B | M | `rs_cam_mcp` takes fixed-token values as `String`, against its own rule | yes |
| INC-06 | B | M | Render pipelines write literal colours, against `render/CLAUDE.md` | yes |
| INC-07 | D | M | Five content-digest recipes for one job (freshness and cache keys) | yes |
| INC-08 | D | M | Engine `*Params` defaults: three conventions, finish keeps a second copy "in lockstep" | yes |
| INC-09 | D | S | Mirror enums convert by `From`, by a free fn, and by inline copies | yes |
| INC-10 | D | S | Three op-type token parsers and two workspace-key parsers | yes |
| INC-11 | D | S | Three "mark above the stock top" helpers with two lifts and a literal | yes |
| INC-12 | E | M | Two cancellation vocabularies; the `FlagCancel` adapter has no production caller | yes |
| INC-13 | E | L | Entry-point suffix stacks (`_with_cancel`, `_traced`, `_annotated`, ...) in 30 files | no |
| INC-14 | E | L | ID newtypes are half adopted; derive sets and homes differ | yes |
| INC-15 | E | M | Five `Severity` vocabularies; the MCP banner re-derives the threshold by string | yes |
| INC-16 | E | M | Error-type style differs inside core and per crate | yes |
| INC-17 | F | M | `ProjectSession` lookup names: `get_` means "by index" and "by id" | yes |
| INC-18 | F | M | Module layout: 44 `mod.rs` against 19 `foo.rs` + `foo/` | yes |

---

## INC-01 — The viz re-derives the drill-family predicate and other op-type rules (B, S)

**Dominant pattern.** A consumer asks the registry. `compute/CLAUDE.md` (W2.4)
says "a consumer reads `OperationConfig::capabilities()`, never a `match` on the
op type". `OperationType::is_drill_kinematics`
(`crates/rs_cam_core/src/compute/catalog.rs:629`) calls itself "THE canonical
drill-family predicate". Its doc says Phase 1 T4 consolidated the open-coded
`Drill | AlignmentPinDrill` matches onto it, and CMP-07 says a `matches!` list
"fails open". `rg` finds 13 callers of it.

**Deviations.** Three viz sites still write the open-coded list:

- `crates/rs_cam_viz/src/app/viewport.rs:72-75` (active drill op for markers)
- `crates/rs_cam_viz/src/ui/properties/feeds_speeds.rs:93-96` (Z-only feed rule)
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1036-1039` (`drill_tool_diameters`)

Other op-specific rules outside core:
`crates/rs_cam_viz/src/controller/events/mod.rs:899` (`DropCutter` scallop
height), `crates/rs_cam_viz/src/app/mcp/commands.rs:2953` (`Pencil` +
`RestDepth` staleness), `crates/rs_cam_viz/src/app/gpu_upload.rs:465-506`
(`Trace` / `ProjectCurve` marker projection).

**Failure.** A third drill-kinematics op joins the registry row. Core gates
see it. The viewport markers, the Z-only feed rule and the pin-wizard diameter
list do not. Nothing fails to compile.

**Winner and enforcement.** Call `op_type().is_drill_kinematics()` at the three
sites. Move the other rules to a capability row field. Then add a source-scan
sentry for `Drill(_) | OperationConfig::AlignmentPinDrill` outside `compute/`.
Current grade: DOC. Fix grade: SENTRY (the call itself is a registry read).

---

## INC-02 — The post-apply epilogue has 6 named doors and about 15 inline copies (B, M)

**Dominant pattern.** The view mirrors `Effects`: after
`ProjectSession::apply` it calls `stamp_stale(&effects.stale)` and then
`invalidate_simulation` when `effects.simulation_cleared`
(`crates/rs_cam_viz/src/controller/CLAUDE.md`, "The view mirrors `Effects`").

**Doors that do this.** `apply_controller_command`
(`crates/rs_cam_viz/src/controller/events/mod.rs:65`), `apply_quietly` (`:100`),
`apply_created` (`:146`), `apply_panel_command`
(`crates/rs_cam_viz/src/ui/properties/panel_apply.rs:227`), `adopt_post_effects`
(`crates/rs_cam_viz/src/controller/io.rs:356`), `finish_pin_edit`
(`crates/rs_cam_viz/src/controller/events/pins.rs:203`). The panel door raises a
flag; the controller doors call the method. The two shapes differ.

**Inline copies.** `rg -n "stamp_stale\("` finds 19 production calls in 10
files (one more is in the root-level test file
`controller/holder_clearance_staleness_g_holderstale.rs`). Inline epilogues sit
at `controller/events/model.rs:258` and `:816`, `controller/io.rs:34`,
`controller/events/undo.rs:266`,
`controller/events/mod.rs:785`, `controller/events/simulation.rs:145`,
`ui/properties/panel_apply.rs:130`, `:339`, `:384`, `ui/properties/mod.rs:834`
and `:1609` (all under `crates/rs_cam_viz/src/`). Direct `.apply` calls that then reach a
door by hand: `controller/io.rs:398`, `app/input.rs:115`, `:151`
(`adopt_post_effects`). `controller/events/compute.rs:184` runs no epilogue:
`let _ = session.apply(command)` drops the `Effects`. The CLI door
`crates/rs_cam_cli/src/command.rs:30` (`apply_command`) is bypassed at
`crates/rs_cam_cli/src/project.rs:309` and `crates/rs_cam_cli/src/smoke.rs:751`.

**Evidence of a past break.** `crates/rs_cam_viz/src/app/input.rs:129-135`
(WP19): "the `Ok` arm did not exist here, so BOTH halves of the answer were
dropped". The sentry `tests/production_writes_go_through_apply_wp15a.rs` is a
source scan that sees only receivers spelled `session.`.

**Winner and enforcement.** One door per crate. In viz, make `AppState::session`
private and give `AppState` one `apply(&mut self, Command) -> Result<Applied,
SessionError>` that stamps, raises the simulation flag and returns the
`Effects`. Then no viz code can reach `ProjectSession::apply` without the
epilogue. Current grade: SENTRY (source scan, receiver-name blind). Fix grade:
TYPES.

---

## INC-03 — Closed-set string tokens: 11 method names and 3 unknown-token policies (B, M)

**Patterns found.** `rg "pub (const )?fn (to_key|from_key|...)\("` (production)
gives about 35 token methods under 11 names: `from_key` 10, `to_key` 6,
`as_str` 6, `key` 3, `wire_name` 3, `parse` 1, `to_token` 1, `from_token` 1,
`token` 1, `kind_str` 1, `as_key` 1 (non-token `key`/`parse` fns left out).

Three policies for an unknown token:

1. Silent default, `from_key(&str) -> Self`:
   `Corner::from_key` (`crates/rs_cam_core/src/session/mod.rs:613`),
   `XYDatum::from_key` (`:662`), `ZDatum::from_key` (`:705`; a bad offset becomes
   `0.0` at `:707`), `Material::from_key`
   (`crates/rs_cam_core/src/material/mod.rs:1692`), `MachineProfile::from_key`
   (`crates/rs_cam_core/src/machine/mod.rs:678`), `parse_tool_material` and
   `parse_cut_direction` (`crates/rs_cam_core/src/session/project_file.rs:882`,
   `:890`).
2. A `ProjectLoadWarning`: tool type, face up, Z rotation
   (`project_file.rs:794`, `:816`, `:843`).
3. `Option` (the caller decides): `PostFormat::from_token`
   (`crates/rs_cam_core/src/gcode/mod.rs:1488`), `FaceUp`/`ZRotation::from_key`
   (`crates/rs_cam_core/src/compute/transform.rs:75`, `:216`),
   `toolpath_spans.rs:293`, `:434`. But `post_format_or_grbl` (`gcode/mod.rs:1608`)
   then maps `None` to GRBL with no warning.

**Evidence of a past break.** The `PostFormat::from_token` doc
(`gcode/mod.rs:1477-1484`, W9 / P-1): three readers open-coded the match, two
did not know `"grblhal"`, and a save and reload "silently downgraded both the
dropdown and the emitted G-code dialect". The serde aliases `grbl_hal` and
`linux_cnc` (`gcode/mod.rs:1426`, `:1428`) are legacy spellings against the
no-legacy ruling.

**Safety note.** `datum_from_section` (`project_file.rs:1053-1066`) passes a
non-empty `z_datum` to `ZDatum::from_key`. A misspelt token gives `StockTop`,
and `StockTop` is the one Z datum that shifts the emitted frame
(`crates/rs_cam_core/src/session/eval_context.rs:222-230`).

**Winner and enforcement.** The repo already has the right shape:
`SettingToken` plus the `setting_token!` macro
(`crates/rs_cam_core/src/settings/mod.rs:78-118`) generates `ALL`, `token`,
`label` and `from_token -> Option<Self>` from one table. It has 4 users, all in
`settings/`. Move it to a core `token` module, use it for every closed set
above, and delete each hand `from_key -> Self`. The loader turns `None` into a
`ProjectLoadWarning` or a refusal, one rule for all. Current grade: CONVENTION.
Fix grade: TYPES (no `-> Self` parser exists to call).

---

## INC-04 — `ALL` arrays: generated, proven, or bare; `diagnostics::ids::ALL` omits 8 ids (B, M)

**Three patterns** (`rg -n "const ALL: "`, about 45 production sites):

1. Generated by a macro, so complete by construction: `OperationType::ALL`
   (`crates/rs_cam_core/src/compute/catalog.rs:364`), `CommandId::ALL`
   (`crates/rs_cam_core/src/session/command.rs:2235`), `UiCommandId::ALL`
   (`crates/rs_cam_viz/src/ui_command.rs:975`), `setting_token!`
   (`crates/rs_cam_core/src/settings/mod.rs:110`).
2. Hand array plus an exhaustive `ordinal()` match as the proof:
   `crates/rs_cam_core/src/trace/semantic_trace.rs:60` with `:97`, and `:309`.
3. Bare hand array, no proof: `diagnostics/ids.rs:416`, `gcode/mod.rs:1435`
   (`PostFormat`), `stock/sim_measurability.rs:108` (`SimMetric`),
   `finish/finish_planner.rs:108` (`FinishBand`), `session/mod.rs:588`
   (`Corner`), `compute/tool_config.rs:22` (`ToolType`), `compute/transform.rs:16`,
   `:184`, `crates/rs_cam_viz/src/state/mod.rs:48` (`Workspace`), and about 25
   more. A `[Self; N]` length checks only that N matches the literal list.

**Break.** A script over `crates/rs_cam_core/src/diagnostics/ids.rs` finds 100
`pub const` ids and 8 that are not in `ALL`: `GEOM_CLIPPED_BAND`,
`GEOM_RAMP_REACH_CLAMP`, `GEOM_OFFSET_LIBRARY_FAILURE`,
`GEOM_BOUNDARY_CLIP_DROPPED`, `GEOM_WATERLINE_LEVELS_BELOW_STOCK`,
`GEOM_WATERLINE_LADDER_EMPTY`, `CONFIG_DERIVED_STEPOVER`,
`CONFIG_CLAIMS_REFERENCE`. Each has 1-6 users outside `ids.rs`. The doc at
`ids.rs:413-415` says `ALL` is "a registry for downstream consumers".
`all_diagnostic_ids_are_unique` (`diagnostics/tests.rs:41`) checks duplicates
only, so it cannot see an omission or a duplicate among the 8. The cold-start
run `coldstart/task7.json` (add a simulation warning) names this gap as a
silent miss: "Nothing but `all_diagnostic_ids_are_unique` reads ALL, and it
checks only duplicates, not omissions".

**Winner and enforcement.** Pattern 1. Turn the diagnostic ids into one macro
table (or an enum) that writes the consts and `ALL`. For a plain enum, add a
`#[derive(EnumAll)]` to `rs_cam_derive` that writes `ALL` and an exhaustive
`ordinal`. Current grade: CONVENTION (pattern 3). Fix grade: TYPES.

---

## INC-05 — `rs_cam_mcp` takes fixed-token values as `String`, against its own rule (B, M)

**Rule.** `crates/rs_cam_mcp/CLAUDE.md`: "A value with a fixed set of legal
tokens takes an enum, not a `String` with the tokens in prose", marked
`#[schemars(inline)]`. `server.rs` has 17 `pub enum` and 25 `schemars(inline)`.

**Deviations** (`crates/rs_cam_mcp/src/server.rs`): `face_up` `:33`,
`operation_type` `:199`, `:466`, `:646`, `workspace` `:360`, `properties_tab`
`:367`, `select` `:371`, `modal` `:375`, `kind` `:603`, `tool_type` `:668`,
`source` `:832`, `strategy` `:1530`, `scope` `:1554`. Each doc comment lists the
tokens in prose. The viz mirror repeats it: `SetUiViewArgs`
(`crates/rs_cam_viz/src/ui_command.rs:179-187`) and
`crates/rs_cam_viz/src/mcp_bridge.rs:1211`. Core already has the enums
(`FaceUp`, `OperationType`, `ToolType`, `SpindleStrategy`, `Workspace`).

**Winner and enforcement.** The enum with `#[schemars(inline)]`. The
`mcp_wire_surface_pin` snapshot changes once (breaking change allowed).
Current grade: DOC. Fix grade: TYPES. A wire-surface sentry can fail on a
`String` field whose doc quotes three or more tokens.

---

## INC-06 — Render pipelines write literal colours, against `render/CLAUDE.md` (B, M)

**Rule.** `crates/rs_cam_viz/src/render/CLAUDE.md:22`: "A colour belongs in
`colors.rs`. Do not write a literal colour in a pipeline."

**Deviations.** `rg` finds about 29 literal `[f32; 3]` colour values in
production `render/` code outside `colors.rs` (test modules, normals and
`[0.0; 3]` placeholders left out), for example
`render/toolpath_render.rs:228-231`, `:471`, `:628`, `:760-779`, `:822`, `:839`,
`:1008`; `render/sim_render.rs:135`, `:175`, `:428`; `render/grid_render.rs:127-149`;
`render/mesh_render.rs:269-270`.
In `ui/`, the token sentry counts only `Color32::from_rgb(`
(`crates/rs_cam_viz/tests/panels_read_the_token_module_up1.rs:122`). So
`Color32::WHITE` (`ui/sim_timeline.rs:591`, `:602`, `:1360`;
`ui/properties/model_sim_panels.rs:310`) and
`from_rgba_unmultiplied(100, 180, 255, ..)` (`ui/toolpath_panel.rs:591`) pass.
Two palettes exist: `render/colors.rs` (`[f32; 3]`) and `ui/tokens` (`Color32`).

**Winner and enforcement.** `colors.rs`. Give vertex colours an `Rgb` newtype
whose constructor is visible only in `colors.rs`; a literal elsewhere then
fails to compile. Extend the `ui/` sentry to `from_rgba_*` and the named
`Color32` constants. Current grade: DOC (render), SENTRY with a blind spot
(ui). Fix grade: TYPES (render), SENTRY (ui).

---

## INC-07 — Five content-digest recipes for one job (D, M)

**Recipes** (all compare "did this input change?"):

1. `DefaultHasher` + `Hash` over fields: `compute/simulate.rs:912-922`,
   `compute/sim_prefix.rs:380`, `compute/obstacle.rs:223`,
   `compute/stock_change.rs:168`, `maps/finish_surface_cache.rs:168`.
2. `DefaultHasher` over `format!("{:?}", ..)`: `machine/head.rs:399-411`
   (`fixed_digest`, `effect_digest`). A `Debug` change moves the digest.
3. `DefaultHasher` over `serde_json::to_vec(op).unwrap_or_default()`:
   `compute/simulate.rs:939`. A serialise error hashes as empty bytes.
4. Hand FNV-1a: `compute/source_stock.rs:149` (`carve_digest`) and
   `compute/toolpath_stats.rs:581` (snapshot stamp).
5. A `String` signature: `session/mod.rs:911-923`
   (`generation_inputs_signature`, `serde_json` + `{:?}` +
   `unwrap_or_default`). A new `ToolpathConfig` field is not forced into it.

Two helpers do one thing: `hash_f64` (`compute/simulate.rs:921`) and
`hash_f64_bits` (`compute/sim_prefix.rs:380`). All paths are under
`crates/rs_cam_core/src/`.

**Winner and enforcement.** One `Digest` type in core with one stable hasher,
and a `Digestible` trait whose impls destructure `let Self { .. } = self` with
no `..`. A new field is then a compile error, as `adopt_geometry` already does
(`session/mod.rs:494`). Current grade: CONVENTION. Fix grade: TYPES.

---

## INC-08 — Engine `*Params` defaults: three conventions, finish keeps a second copy "in lockstep" (D, M)

**Patterns** (script over `pub struct \w+Params` in core, 42 structs): 12
of the finish engines have a hand `impl Default` with their own literals (for example
`finish/unified_finish.rs:262-290`, `finish/pencil.rs`, `finish/scallop.rs`);
`ops/` and `adaptive*/` engines have no `Default` (`ops/pocket.rs`,
`ops/vcarve.rs`, `adaptive/mod.rs`, `adaptive3d/mod.rs`); 6 are `Copy`
(`PocketParams`, `ZigzagParams`, `ProfileParams`, ...), most are not. The
`..*params` restamp rule (`ops/pocket.rs:60-62`) needs `Copy`.

**Drift risk.** `finish/unified_finish.rs:275-287`: "Kept in lockstep with
`operation_configs::default_unified_finish_intra_region_hookup_mm` ...
Production always overrides this from config ... a core default and a serde
default that disagree is the divergence class this programme has already found
twice." The twin comment is at `compute/operation_configs.rs:2160-2163`. So one
op has three default sources: the engine `Default`, the config `Default`, and
the serde `default = "fn"` (27 such fns in `operation_configs.rs`).

**Winner and enforcement.** The ops convention: an engine has no `Default`, and
the adapter builds its params from the config. Tests build params through
`XConfig::default()` and the same adapter fn. Delete the engine `Default`
impls. Current grade: CONVENTION (comment). Fix grade: TYPES (no second
default exists to call).

---

## INC-09 — Mirror enums convert by `From`, by a free fn, and by inline copies (D, S)

**Patterns.** Core has 19 production `impl From<A> for B`. The feeds LUT pair
uses a free fn: `op_family_to_lut`
(`crates/rs_cam_core/src/feeds/vendor_normalize.rs:152`). `gcode/mod.rs:743-750`
copies the same 8 arms inline. `PassRole` to `LutPassRole` is an inline 3-arm
match in four production files: `feeds/vendor_normalize.rs`,
`gcode/mod.rs:753-755`, `tool_load/mod.rs:310-312`,
`crates/rs_cam_viz/src/state/simulation.rs:470-472`. The variant sets are
identical (`feeds/mod.rs:304`, `:324`; `feeds/vendor_lut.rs:270`, `:306`).
`EntryStyle` exists four times in core (`dressup/mod.rs:87`,
`adaptive3d/mod.rs:116`, `compute/config.rs:664`,
`compute/operation_configs.rs:97`) and once in viz render
(`render/toolpath_render.rs:803`).

**Winner and enforcement.** `impl From<PassRole> for LutPassRole` and
`impl From<OperationFamily> for LutOperationFamily`, once each (C-CONV-TRAITS),
or one enum per identical pair. An exhaustive `From` makes a new variant a
compile error in one place. Current grade: TYPES per copy, CONVENTION for "one
copy". Fix grade: TYPES. Note: `feeds/**` and `tool_load/**` had a separate
owner in the 2026-09-16 plan.

---

## INC-10 — Three op-type token parsers and two workspace-key parsers (D, S)

**Sites.** `parse_op_type -> Result` (`crates/rs_cam_cli/src/run.rs:271`, scans
`OperationType::ALL` by `kind_str`); `parse_op_type -> Option`
(`crates/rs_cam_cli/src/smoke.rs:1074`, the same scan); `parse_operation_type`
(`crates/rs_cam_mcp/src/server.rs:1971`, serde, with a hand list of 24 op names
in the error text). The workspace key is parsed at
`crates/rs_cam_viz/src/app/mcp.rs:1054` (four keys plus `"sim"`) and again at
`crates/rs_cam_viz/src/app.rs:375-379` (two keys plus `"sim"`).

**Winner and enforcement.** Generate `OperationType::from_kind_str -> Option`
in `for_each_op!` (`compute/catalog.rs`), and build error text from
`OperationType::ALL`. Give `Workspace` the INC-03 token trait. Current grade:
CONVENTION. Fix grade: TYPES (generated).

---

## INC-11 — Three "mark above the stock top" helpers with two lifts and a literal (D, S)

**Sites.** `hole_display_point` with `HOLE_MARK_LIFT_MM = 0.05`
(`crates/rs_cam_viz/src/interaction/picking.rs:277`, `:290`);
`pin_display_point` with `PIN_MARK_LIFT_MM = 0.1`
(`crates/rs_cam_viz/src/state/pin_view.rs:21`, `:57`); inline `+ 0.05` four
times in `crates/rs_cam_viz/src/app/gpu_upload.rs:474-529` (Trace and Project
Curve markers). Each one finds the top of the stock in the display frame in its
own way (`effective_stock(..).2`, `origin_z + z`).

**Winner and enforcement.** One `display_point_above_stock(frame, stock, xy,
lift)` in `state/job.rs` beside `SetupFrame::emission_to_display_shift`, with
named lifts. Current grade: CONVENTION. Fix grade: CONVENTION (one helper).

---

## INC-12 — Two cancellation vocabularies; the `FlagCancel` adapter has no production caller (E, M)

**Dominant pattern.** `&dyn CancelCheck` (`crates/rs_cam_core/src/interrupt.rs:5`)
in 55 production files: every kernel in `finish/`, `stock/`, `dexel_stock/`,
`maps/`, `ops/`, `adaptive*/`.

**Deviations.** `&AtomicBool` parameters: 39 production sites, mostly
`session/compute.rs` (12), `session/multitool.rs` (7), `compute/simulate.rs` (4),
`session/compute/diagnostics.rs` (3), viz `compute/worker/execute/mod.rs` (3).
Hand closures `|| cancel.load(Ordering::SeqCst)` remain at
`compute/simulate.rs:1614`, `:1774`, `compute/collision_check.rs:104` and in
`ExecutionContext::cancel_fn` (`compute/execute.rs:427-429`).
`FlagCancel` (`interrupt.rs:55`) says it is "that adapter", but `rg` finds only
one test caller (`crates/rs_cam_core/tests/memory_budget_core_b1_b2_b3.rs:383`).

**Known.** Q5 (2026-09-16) closed the `never_cancel` + `expect` copies with
`NeverCancel` and `run_uncancellable`. Since then `FlagCancel` was added, but
nothing adopted it.

**Winner and enforcement.** `&dyn CancelCheck` everywhere below the lane
boundary. A lane or a session door converts its `AtomicBool` once with
`FlagCancel`. Delete `cancel_fn`. Current grade: CONVENTION. Fix grade: TYPES
(signatures).

---

## INC-13 — Entry-point suffix stacks in 30 files (E, L)

**Pattern.** `rg "pub(\(crate\))? fn \w+_(with_cancel|traced|annotated|structured|reported|with_provenance|checked|with_resolution|and_stage)\b"`
finds 65 entry points in 30 core files. Examples:
`adaptive3d/mod.rs:488-634` (8 `adaptive_3d_toolpath*` forms),
`dexel_stock/simulation.rs:39-907` (9 `simulate_toolpath*` forms),
`adaptive/mod.rs:323-348` (4), `finish/scallop.rs` (3), `geometry/boundary.rs` (3),
`gcode/mod.rs` (5).

**Inconsistencies inside the pattern.**

- Suffix order differs: `simulate_toolpath_with_lut_cancel` (`:66`),
  `simulate_toolpath_with_metrics_with_cancel` (`:323`),
  `simulate_toolpath_with_lut_metrics_cancel` (`:361`),
  `simulate_toolpath_with_lut_metrics_rapid_checked` (`:414`).
- Visibility differs between siblings:
  `adaptive_toolpath_structured_annotated_traced_with_cancel` is `pub`
  (`adaptive/mod.rs:348`); its siblings at `:329`, `:337` are `pub(crate)`.
- Some forms have only test callers: `adaptive_3d_toolpath_annotated`
  (`adaptive3d/mod.rs:536`) is called from three test files only.

**Winner.** The repo already uses a request struct where it matters:
`SimulationRequest` and `ExecutionContext`. One entry fn per engine that takes
a request struct (cancel defaults to `NeverCancel`, trace and annotation sinks
are `Option`) replaces the stack. Known: S11 (`_with_cancel` dead wrappers) and
Q10 (110 `too_many_arguments` rows) in the 2026-09-16 plan, below its cut line
for Q10. Current grade: CONVENTION. Fix grade: TYPES. The listed sites are not
in flux (`gcode/mod.rs`, which also has 5 such forms, is in flux and is left
out of the site list).

---

## INC-14 — ID newtypes are half adopted; derive sets and homes differ (E, L)

**Dominant pattern.** `crates/rs_cam_core/src/ids.rs` newtypes (R3: "so the
compiler surfaces every such confusion").

**Deviations.**

- One record, two styles: `ToolpathConfig.id: ToolpathId` but `tool_id: usize`
  and `model_id: usize` (`crates/rs_cam_core/src/session/mod.rs:808-815`);
  `LoadedModel.id: usize` (`:380`); `SetupData.id: usize` (`:742`). The viz
  entry types them (`crates/rs_cam_viz/src/state/toolpath/entry.rs:23-24`).
- Bridges exist because of it: `find_tool_by_raw_id`, `find_model_by_raw_id`
  (`session/mod.rs:1836`, `:1844`); `find_setup_by_id(id: usize)` (`:1797`)
  beside `find_toolpath_config_by_id(ToolpathId)` (`:1745`).
- `rg` counts 69 re-wrap sites (`SetupId(x.id)`, `ModelId(x.id)`,
  `ToolId(x.tool_id)`, ...) outside tests.
- `ToolId` lives in `crates/rs_cam_core/src/compute/tool_config.rs:8`, not in
  `ids.rs`.
- Derives differ: only `ToolpathId` has `Ord` and `Display` (`ids.rs:22-30`);
  `ModelId`, `SetupId` (`ids.rs:33`, `:37`) and `ToolId` have no
  `#[serde(transparent)]`, which the other three have.
- Viz imports one type by two paths: `SetupId` through `state::job` (36 sites)
  and `rs_cam_core::ids` (8); `ToolpathId` through `state::toolpath` (64) and
  core (68).

**Winner and enforcement.** One `define_id!` macro in `ids.rs` that writes the
same derives, `Display` and `transparent` for every id. Type every id field.
One import path. Current grade: TYPES for `ToolpathId`, CONVENTION for the
rest. Fix grade: TYPES. The `primitive-newtype` finder owns the wider
bare-`usize` question; this finding is the inconsistency between ids.

---

## INC-15 — Five `Severity` vocabularies; the MCP banner re-derives the threshold by string (E, M)

**Sites.** `diagnostics::Severity` (Info, Hint, Caution, Critical, Blocking)
(`crates/rs_cam_core/src/diagnostics/mod.rs:143`); `gcode_validator::Severity`
(Info, Warning, Error) (`crates/rs_cam_core/src/export/gcode_validator.rs:43`);
`controller::Severity` (Info, Warning, Error)
(`crates/rs_cam_viz/src/controller.rs:72`); `VerdictSeverity` (Critical,
Important, Polish) (`crates/rs_cam_core/src/session/diagnostics_types.rs:104`)
with an adapter named `severity_from_legacy`
(`crates/rs_cam_core/src/diagnostics/adapters/from_project_diagnostics.rs:68`);
`DrillGateSeverity` (`crates/rs_cam_core/src/tool_load/drill_gates.rs:80`).
The MCP banner reads the JSON reply and keeps a row when
`matches!(severity.as_str(), "caution" | "critical" | "blocking")`
(`crates/rs_cam_viz/src/app/mcp.rs:749`).

**Winner and enforcement.** `diagnostics::Severity` is the richest and is the
one on the wire. Give it `fn is_banner(self) -> bool` and read the typed value
before it becomes JSON. Map the other enums to it by exhaustive `From`, or
retire `VerdictSeverity` (its adapter calls it legacy). Current grade:
CONVENTION. Fix grade: TYPES.

---

## INC-16 — Error-type style differs inside core and per crate (E, M)

**Core.** 17 error types write `Display` and `impl std::error::Error` by hand
(`session/mod.rs:333` `SessionError`, `compute/execute.rs:238`,
`compute/simulate.rs:895`, `gcode/mod.rs:56` `ExportError`,
`compute/obstacle.rs:285`, `:359`, `compute/stock_change.rs:296`,
`machine/head.rs:198`, `selection/refusal.rs:290`, ...). About 10 use
`thiserror`: everything in `io/`, plus `mesh.rs`, `budget/mod.rs`,
`selection/item.rs`, `geometry/enriched_mesh.rs`, `gcode/post.rs`. The split
follows the folder, not a rule.

**Other crates.** viz: `VizError` (thiserror) maps EVERY `SessionError` to
`ProjectLoad` (`crates/rs_cam_viz/src/error.rs:56-60`) and takes any `String`
(`:50`); 23 `Result<_, String>` in viz. CLI: `anyhow` (28 uses). `rs_cam_mcp`:
`Result<_, String>` (`server.rs:1971`, `:1986`, `:2065`, `:2149`). Proxy: 9
`Result<_, String>` and one `Result<Value, Value>`
(`crates/rs_cam_mcp_proxy/src/proxy.rs:1064`).

**Winner and enforcement.** Core and viz: `thiserror` enums (the message stays
next to the variant). Binaries (CLI, proxy): `anyhow` at the top only. Shared
helper fns in `rs_cam_mcp`: a typed error. The `error-handling` finder owns the
`String`-payload inventory. Current grade: CONVENTION. Fix grade: CONVENTION
(no lint enforces `thiserror`); TYPES for the `From<SessionError>` mapping (one
variant per cause).

---

## INC-17 — `ProjectSession` lookup names: `get_` means "by index" and "by id" (F, M)

**Sites** (`crates/rs_cam_core/src/session/mod.rs`): `get_toolpath_params(index)`
`:1542`, `get_result(index)` `:1552`, `get_toolpath_config(index)` `:1619` take an
index; `get_tool(id: ToolId)` `:1547` takes an id. `find_toolpath_config_by_id`
`:1745` and `find_setup_by_id` `:1797` return `(usize, &T)`;
`find_tool_by_raw_id` `:1836` returns `&T`. `list_setups` `:1614` returns
`&[SetupData]`; `list_tools` `:1522` and `list_toolpaths` `:1500` return summary
`Vec`s. These methods have about 120 call sites. Workspace-wide, `get_` is rare
(7 production getters against 114 plain `fn x(&self) -> &T`).

**Winner and enforcement.** `*_at(index)` for a position and `*_by_id(Id)` for
an id; `list_*` always returns a summary. When INC-14 types the ids, the
compiler separates the two forms. Current grade: CONVENTION. Fix grade: TYPES
(after INC-14).

---

## INC-18 — Module layout: 44 `mod.rs` against 19 `foo.rs` + `foo/` (F, M)

**Count.** `find crates -path '*/src/*' -name mod.rs` gives 44. A directory with
a sibling `foo.rs` occurs 19 times: for example
`crates/rs_cam_core/src/compute/execute.rs`, `compute/catalog.rs`,
`session/compute.rs`, `session/mutation.rs`, `finish/scallop.rs`,
`finish/pencil.rs`, `feeds/suggest.rs`, `stock/simulation_cut.rs`;
`crates/rs_cam_viz/src/app.rs`, `controller.rs`, `app/mcp.rs`,
`app/mcp/commands.rs`, `compute/worker.rs`. One folder mixes both:
`compute/mod.rs` beside `compute/execute.rs` + `execute/`; `session/mod.rs`
beside `session/compute.rs` + `compute/`. `Cargo.toml` turns on neither
`clippy::mod_module_files` nor `clippy::self_named_module_files`.

**Winner and enforcement.** `mod.rs` (the majority). Move the 19 files, then
deny `clippy::self_named_module_files` in `[workspace.lints.clippy]`. Do it
after the in-flux sessions land: several of the 19 files are in flux. Current
grade: NONE. Fix grade: LINT.
