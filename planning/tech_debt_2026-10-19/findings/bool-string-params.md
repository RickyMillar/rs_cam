# bool-string-params: Boolean, Option and stringly-typed parameters

Baseline 99b6fb41, branch geo-select. Read-only sweep. The facts come from `rg` and file reads. The sites are `path:line`.

Sweep method:

1. Run the CHECKLIST detection commands (section 2).
2. Run a multi-line Python scan for `fn` signatures with one or more `: bool` parameters in production code (191 fns, 14 with two or more).
3. Run a scan of bare `true`/`false` call arguments (about 381 production sites).
4. Do a manual pass of the string-dispatch doors: the MCP wire (`rs_cam_mcp/src/server.rs`), the viz MCP handlers (`app/mcp/*`), the CLI (`main.rs`), the core `from_key` parsers and the param-schema type strings.
5. Read the map leads (core-compute rows 130/159, core-session 26, core-strategies 20, core-finish 13, viz-rest 291) and the coldstart task2/task3 notes.

---

## 1. CLI strategy and skip flags accept any string and fall back in silence (Tier A, S)

- `crates/rs_cam_cli/src/main.rs:250` declares `modulation_strategy: String` with no `value_parser`.
- `main.rs:523-528` maps `"band-mid" | "bandmid" | "band_mid"` to `BandMid` and maps everything else (`_ =>`) to `ConstrainedMax`. A typo such as `--modulation-strategy band-midd` runs ConstrainedMax. The feeds in the emitted G-code then differ from what the operator asked for, and no message tells them.
- `main.rs:516-522`: `--skip` splits on commas and drops each token that does not parse (`filter_map(|s| s.trim().parse().ok())`). If the operator types `--skip 3,4a`, toolpath 4 is generated and emitted.
- `main.rs:284` and `main.rs:529-533`: `--spindle-strategy` has a clap `value_parser = ["match_chart","max_speed"]` list, and a second hand-written match ends `_ => None`. These are two copies of one vocabulary.

**Fix.** Use `#[derive(clap::ValueEnum)]` on CLI mirrors of `ModulationStrategy` and `SpindleStrategy`, or derive it on the core enums behind a feature. Parse `--skip` with `value_delimiter = ','` into `Vec<usize>`, so that a bad token is a clap error. Delete the alias spellings (operator ruling: no legacy).
Enforcement: NONE now, TYPES after the fix.

## 2. The setup datum parsers fail open; CMP-15 fixed only the neighbours (Tier B, S)

- `crates/rs_cam_core/src/session/mod.rs:613-620`: `Corner::from_key` maps any unknown token to `FrontLeft`.
- `session/mod.rs:662-673`: `XYDatum::from_key` maps any unknown token to `CornerProbe(FrontLeft)`.
- `session/mod.rs:705-715`: `ZDatum::from_key` maps any unknown token to `StockTop`, and `offset:<garbage>` to `FixedOffset(0.0)` (`val.parse().unwrap_or(0.0)`).
- `session/project_file.rs:1050-1066`: `datum_from_section` calls them and records no `ProjectLoadWarning`. Its doc states the hazard: "both `from_key` implementations fall through to the default".
- The Z datum changes emitted motion. `gcode/mod.rs:310-339` and `session/eval_context.rs:90-95` state that `StockTop` shifts the program Z0 to the stock top. A typo in a hand-edited file, for example `z_datum = "machine_table"` where the file needs `"table"`, gives a G-code program zeroed at the stock top. The operator zeroes at the table. No warning appears.
- The policy that applies is broken. CMP-15 (design_audit_2026-09-17/core-compute.md:185-194) made `FaceUp::from_key` / `ZRotation::from_key` return `Option`. `parse_face_up` / `parse_z_rotation` (`project_file.rs:810-850`) now warn and default (Q4 policy). The datum, which sits beside them and also affects safety, was left on the fail-open shape.

**Fix.** Make all three `from_key` functions return `Option<Self>`, and make `FixedOffset` refuse a bad number. Add `ProjectLoadWarning::UnknownXyDatum` / `UnknownZDatum`, as the face-up parser does.
known_id: CMP-15. That fix covered `face_up` / `z_rotation` only. In flux.

## 3. The dressup field vocabulary is string tables with wildcard arms; the CUT-13 fold is incomplete (Tier B, M)

- `crates/rs_cam_core/src/compute/config.rs:1191-1200`: `owner_of_value_field(key: &str)` is a string match that ends `_ => None`.
- `config.rs:1220-1228`: `is_dressup_enabled(key: &str)` is a string match that ends `_ => false`.
- The only reader is `session/mutation/config.rs:92-93`. If a new `Option<Params>` dressup is not listed in both tables, `set_dressup_field` accepts a value patch while the dressup is off. The `Option` round trip then drops the value (coldstart task3.json:50, :176, :339 record this as a cold-start silent miss).
- `FIELD_DEFS` (`config.rs:1060`) has a completeness test (`config.rs:1484`). The two owner tables have none.
- `config.rs:920-925`: `feed_optimization: bool` + `feed_max_rate` + `feed_ramp_rate` keep the flat bool-plus-value shape. The stated reason is that "its only reader outside `compute/` is a fixture in `tool_load/`, which another session owns". `rg feed_optimization crates/rs_cam_core/src` finds no `tool_load/` reader now. The only readers are `dressup_apply.rs:889-893` and the viz panel `linking_dressup.rs:300,590-608`. The reason is stale.
- The entry-style dials (`ramp_angle`, `helix_radius`, `helix_pitch` at `config.rs:898-905`) have the same shape: they are flat siblings of `entry_style: DressupEntryStyle {None, Ramp, Helix}`.

**Fix.** Fold `feed_optimization` into `Option<FeedOptParams>`. Move the owner relation into `DressupFieldDef` (an `owner: Option<DressupKey>` column, where `DressupKey` is an enum), so that the two match tables are derived and cannot miss a row. Consider `EntryStyle::Ramp { angle }` / `Helix { radius, pitch }`.
known_id: CUT-13 / CMP-17 (design_audit_2026-09-17). In flux.

## 4. The debug-span kind vocabulary is free strings in three crates and has drifted (Tier D, M)

- Core emits kinds as `impl Into<String>` (`trace/debug_trace.rs:386-392`), with 23 `start_span(` sites and the `DressupTraceInfo.debug_key: &'static str` table (`compute/execute/dressup_apply.rs:163-231`).
- In that table, three stages share `debug_key: "entry_style"` (`dressup_apply.rs:177,183,189`).
- Viz help: `ui/sim_debug.rs:156-195` `debug_span_math_summary(kind: &str)` ends `_ => None`. The table has drifted:
  - Four arms name kinds that no core code emits: `pre_stamp`, `preflight`, `widen_band` and `prepare_input`. `rg '"pre_stamp"' crates/rs_cam_core/src` finds nothing, and the other three give the same result.
  - About 15 emitted kinds have no help line: `z_level_clear`, `slot_clearing`, `rest_field`, `obstacle_clip`, `forced_clear`, `contour_spiral`, `boundary_clip`, `boundary_cleanup`, `border_clear`, `rapid_order`, `entry_style`, `dogbones`, `lead_in_out`, `link_moves`, `segment_merge` and `air_cut_filter`.
- MCP: `app/mcp/diagnostics.rs:707-745` holds a third hand list (`"z_level_clear" | "adaptive_pass" | "z_level"` and `"entry_search"`). It maps debug kinds to `SpanKind`, and the doc on that function even names `preflight` / `widen_band` as live kinds.
- The structural `SpanKind` is already an enum with an exhaustive `as_key` (D3, `diagnostics.rs:747-760`). The debug kinds are not.

**Fix.** Add a core `DebugSpanKind` enum with `as_key()`, `help()` and `structural() -> Option<SpanKind>`. Make `start_span` take it. Delete the viz and MCP tables.
Enforcement: NONE now, TYPES after the fix. In flux (`dressup_apply.rs`).

## 5. Arc direction is a positional `clockwise: bool` at about 20 sites in motion and safety code (Tier E, M)

- `crates/rs_cam_core/src/geometry/arc_util.rs:9-32`: `linearize_arc(.., clockwise: bool, ..)` and `linearize_arc_into(.., clockwise: bool, ..)`.
- Each caller matches `MoveType::ArcCW` to `true` and `MoveType::ArcCCW` to `false` by hand, in two near-identical arms:
  - `dexel_stock/simulation.rs:219,237,667,713` (the stock simulation)
  - `compute/obstacle.rs:1043,1046` (the obstacle clip)
  - `dressup/entry_audit.rs:98,101,232,235`
  - `export/viz.rs:746,753`
  - `export/ribbon.rs:208,267`
  - `dressup/entry_descent.rs:51,144,154`
  - `dressup/air_cut.rs:172`
  - `dressup/mod.rs:2231,2464`
- A swapped literal compiles. It mirrors the arc in the simulated stock and in the obstacle check, so a collision verdict could read clean for motion that the machine cuts on the other side.
- The emitter already has a private `enum ArcDir { Cw, Ccw }` (`gcode/emitter.rs:360-372`).

**Fix.** Make `ArcDir` public in `geometry`. Give `MoveType` an `arc(&self) -> Option<(ArcDir, f64, f64)>` accessor, or one `Arc { dir, i, j, feed_rate }` variant. Make `linearize_arc*` take `ArcDir`. The paired arms then become one arm.
Enforcement: CONVENTION now, TYPES after the fix. In flux (`obstacle.rs`).

## 6. `OperationTransformCapabilities` encodes rapid-order freedom as interacting bools, built by `new(bool, bool, bool, bool)` (Tier E, M)

- `crates/rs_cam_core/src/compute/catalog.rs:108-186`: the struct has eight `bool` fields.
- `catalog.rs:190-208`: `pub const fn new(allows_global_rapid_reorder, requires_depth_order, continuous_path_required, allows_link_moves)` is called positionally at 11 sites. Examples: `catalog/capabilities.rs:483 new(true, false, false, true)` and `:504 new(false, false, false, true)`. There is one more site in `tests/capability_link_moves_safety.rs:1517`.
- If the first and last arguments swap, link moves are allowed on an op that gouged with them. `capabilities.rs:491-498` records an 8.21 mm Inlay gouge.
- The reorder fields are one concept: `allows_barriered_rapid_reorder` / `allows_unbarriered_rapid_reorder` (`catalog.rs:268-277`) combine four of them. Only a test keeps the illegal combinations out (`catalog/tests.rs:579-583`: global reorder with depth order, and global reorder with a continuous path).
- `leads_on_closed_passes_only: bool` (`catalog.rs:186`) is converted back to `LeadScope` by `lead_scope()` (`:214-221`). The enum exists, but the struct stores a bool.

**Fix.** Use `rapid_order: RapidOrder { Forbidden, BarrieredOnly, DepthOrdered, Global }` (the illegal pairs become unrepresentable), `links: LinkPolicy { Forbidden, Straight }`, and `lead_scope: LeadScope` stored directly. Replace `new(..)` with named `const` constructors.
Enforcement: SENTRY now (`every_op_states_its_capabilities_w2_4`, `catalog/tests.rs:579`), TYPES after the fix. In flux.

## 7. `SimulationOptions.auto_resolution: bool` is a dead flag whose doc is false (Tier C, S)

- `crates/rs_cam_core/src/session/mod.rs:1052-1060` has `resolution: f64` beside `auto_resolution: bool`, which is a contradictory pair.
- Every production literal sets `auto_resolution: false`: `rs_cam_cli/src/project.rs:405`, `smoke.rs:763`, `rough_score.rs:608`, `benches/hot_paths.rs:881` and `Default` at `session/mod.rs:1166`. Only `tests/project_curve_deviation.rs:229` sets `true`.
- The doc says "matching the GUI's auto-resolution logic", but the branch (`session/compute/simulation.rs:263-267`) calls `auto_resolution_for_groups` (`session/compute.rs:2632`). That function applies the tool rule only. The project `Auto` rule (`session/rest_stock.rs:173-184`, which the GUI reads at `app/simulation.rs:271`) also applies `rest_resolution_required_mm`. A caller that trusts the doc on a rest cascade gets a coarser cell than the GUI uses.
- The typed answer exists: `SimulationResolution { Auto, Fixed(f64) }` (`rest_stock.rs:51`).
- In the same struct, `modulation_feed_scale` is "Ignored when `modulation_strategy == BandMid`" (`mod.rs:1140-1150`). The `modulation_strategy` doc (`mod.rs:1127-1139`) says the modulator fires only when the profile carries `kinematics`. The adjacent `adaptive_feed_modulation` doc (`mod.rs:1093-1103`) corrects exactly that claim.

**Fix.** Use `resolution: SimulationResolution`, resolved through the one session rule. Delete `auto_resolution` and the second auto rule. Use `modulation: Option<Modulation { ConstrainedMax { feed_scale }, BandMid }>` in place of the bool, the enum and the f64.
In flux (`session/mod.rs`, `session/compute.rs`).

## 8. The MCP wire carries `String` where the CLI-09 typed-enum pattern already exists (Tier E, M)

- CLI-09 made `ZRotationParam` and `StockSourceParam` typed `schemars` enums (`rs_cam_mcp/src/server.rs:36-75`, `:1495-1520`).
- The sibling `SetSetupFaceParam.face_up: String` (`server.rs:33`) is still a string. It is hand-parsed in `rs_cam_viz/src/app/mcp/commands.rs:432-447`, which repeats the table that `FaceUp::from_key` (`compute/transform.rs:75`) already holds.
- Other String fields with a core enum behind them:
  - `SetSpindleStrategyParam.strategy` (`server.rs:1530`), parsed at `commands.rs:741-756` with the aliases `"MatchChart" | "matchchart"`
  - `ApplyFeedsParam.scope` (`server.rs:1554`), the field that decides whether the cut changes, parsed at `app/mcp/generation.rs:865-878` with the aliases `"Speeds"`, `"CutGeometry"` and `"cut"`
  - boundary `source` / `containment` (`server.rs:832,834`), parsed at `commands.rs:1514-1620`
  - `operation_type` (`server.rs:199,466,646`)
  - `tool_type` (`server.rs:668`)
  - `span_kind` / `kind` (`server.rs:581,603,628`)
  - `tier_strategies: Vec<String>` (`app/mcp.rs:95-110`, aliases `"unified"` and `"iso"`)
- The error texts repeat each valid list by hand. For example, `parse_operation_type` (`server.rs:1971-1974`) lists all 24 kinds. The list matches `for_each_op` today, and nothing ties the two together.
- The published schema says only `"type": "string"` for these fields. This is the discovery defect that CLI-09 named.

**Fix.** Add a `schemars(inline)` wire mirror (or a feature-gated `JsonSchema` derive on the core enum) for each field. serde then refuses an unknown token, and the published schema lists the values. Delete the hand parsers and the alias spellings (no-legacy ruling).
known_id: CLI-09 (only `z_rotation` was converted). In flux.

## 9. Capabilities name config fields by string; the advisor writes through serde JSON by that name (Tier E, S)

- `crates/rs_cam_core/src/compute/catalog/capabilities.rs:81,111`: `relink_dial: Option<&'static str>` and `clearing_strategy_dial: Option<&'static str>`. The rows are at `catalog/registry.rs:624,684,722` (`Some("hookup_mm")`, `Some("clearing_strategy")`).
- `session/compute.rs:1468-1477`: `with_clearing_strategy` serialises the op, inserts the strategy under the dial name, and deserialises. On any failure it returns `None`, and the candidate is dropped with no message.
- `trace/narrate.rs:1426` reads `relink_dial` as text.
- A sentry holds the names (`tests/every_op_states_its_capabilities_w2_4.rs:538-550` checks that each dial names a schema parameter). The compiler does not.

**Fix.** Store a typed setter, for example `clearing_strategy_dial: Option<fn(&mut OperationConfig, ClearingStrategy) -> bool>`, or generate one with the `OpParams` derive. Keep the name only as a derived label.
Enforcement: SENTRY now, TYPES after the fix. In flux.

## 10. The param-schema type is a string, and the `vec<` branches match a type that no `ParamType` publishes (Tier C, S)

- `crates/rs_cam_core/src/compute/catalog/schema.rs:144-196`: `ParamType::TYPE_NAME` is a free `&'static str`. Core logic then matches on it:
  - `session/compute/params.rs:166-168`: `matches!(ty, Some("usize" | "u32" | "option<usize>" | "option<u32>"))`
  - `params.rs:410`: `ty.starts_with("vec<")` (the null-to-`[]` clear for list fields)
  - `compute/catalog.rs:1226`: `def.type_name.starts_with("vec<")` (the `params_value_including_nulls` doc at `:1215-1218`)
- No impl publishes `vec<`. The one list impl, `Vec<[f64; 2]>`, publishes `"array<[f64;2]>"` (`schema.rs:181-184`). `rg 'pub \w+: Vec<' crates/rs_cam_core/src/compute/operation_configs.rs` finds no list field in any op config: `coarse_steps` was removed (`catalog/tests.rs:894-916`).
- Both `vec<` branches and the `Vec<[f64;2]>` impl are therefore dead, but they read as live behaviour. If a list param comes back, it publishes `array<..>` and silently misses both branches.

**Fix.** Add `const KIND: ParamKind` (an enum: `Float`, `Integer`, `Bool`, `Enum(&[&str])`, `List`) to `ParamType`, and derive the wire `type_name` from it. Match on `KIND` in `params.rs` and `catalog.rs`. Delete the dead `Vec` impl or give it the `List` kind.
Enforcement: NONE now, TYPES after the fix. In flux (`catalog.rs`).

## 11. GUI param rows bind a name string and a field separately (Tier E, M)

- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:28-37,53-66`: `dv(ui, p(OperationType::Pocket, "stepover", "Stepover:"), &mut cfg.stepover, ..)`. The name selects the clamp range (`schema_range`) and the tooltip (`help_for`). The `&mut` field is the value that the row writes.
- There are 81 such call sites in `rs_cam_viz/src`. A multi-line `rg` that compares the name to the bound field finds 0 mismatches today, so the convention holds.
- The UI-04 sentry (`tests/the_help_key_is_the_param_name_ui04.rs:15-35`) checks that the name exists in the registry. It does not check that the name belongs to the field that the row binds, or that the `OperationType` tag belongs to the config. `p(Pocket, "depth", ..)` beside `&mut cfg.stepover` compiles and passes the sentry, and the row then clamps stepover to the depth range.
- coldstart task2.json:230 records the same hazard.

**Fix.** Make `#[derive(OpParams)]` emit typed keys (for example `PocketConfig::STEPOVER: ParamKey<PocketConfig, f64>` with a field accessor). Make `dv` take the key and `&mut PocketConfig`, so that the name, the op and the field come from one token.
Enforcement: SENTRY (partial) now, TYPES after the fix. Not in flux, except `operations/drill.rs` and `operations/mod.rs`.

## 12. `ApplyScope` is unpacked into two positional bools for the provenance stamp (Tier E, S)

- `crates/rs_cam_core/src/feeds/suggest/apply.rs:236-237` turns `ApplyScope` into `write_speeds` / `write_geometry`.
- `apply.rs:360` passes them to `feeds/provenance.rs:170 apply_suggested_subset(result, op, rpm_written: bool, speeds: bool, geometry: bool)`.
- `provenance.rs:163` calls `(.., rpm_written, true, true)`, which is three positional bools in a row.
- The state `(false, false)` is representable and means nothing. A swap of `speeds` and `geometry` restamps the cut-geometry provenance on a speeds-only apply, which is the defect that G-SPEEDSAPPLY separated.

**Fix.** Pass `ApplyScope` and add `ApplyScope::writes_speeds()` / `writes_geometry()`. Make `rpm_written` a two-variant enum, or put it inside the scope.
Enforcement: CONVENTION now, TYPES after the fix. Not in flux.

## 13. Test-door string parsers in the public production API fall back in silence (Tier C, S)

- `crates/rs_cam_core/src/material/mod.rs:1692-1838`: `Material::from_key` has doc "Parse from TOML key. Returns default softwood if unrecognized". It has 31 arms and an inline `solid_wood_by_janka:` mini-parser. `to_key` is at `:1586`.
  - `rg 'Material::from_key|\.to_key\(\)'` finds no production caller. The callers are the in-file tests (`:2260,:2333`) and `tests/literature_matrix/shim.rs:101-102`, which has to detect the silent fallback by key equality.
  - Materials persist through serde. The "TOML key" doc describes a format that no file uses.
- `crates/rs_cam_core/src/machine/mod.rs:678-684`: `MachineProfile::from_key` falls back to the generic router for any key. L12 (tech_debt_2026-09-16) kept it `pub` with a doc-only fix. Since then it has gained no caller.

**Fix.** Return `Option<Self>` from both, or move both behind the `test-support` feature, and delete `Material::to_key` if the shim does not need it.
known_id: L12 (for `MachineProfile::from_key`). Not in flux.

## 14. Positional bool parameters elsewhere; the clippy bool lints are off (Tier F, S)

- `finish/pencil/emission.rs:182` `plan_entry_ramp(.., end_at_start: bool)` is called with bare literals at `dressup/entry_descent.rs:541` (`true`) and `pencil/emission.rs:345` (`false`).
- `finish/unified_finish.rs:221` `refine_band_chords(.., level: bool)` is called at `unified_finish/band_kernels.rs:148,441`.
- `compute/config.rs:756` `HelixRadius::resolve(requested, from_rule: bool, cutter)` is called at `config.rs:1207`, `feeds/ramp.rs:914` and `operation_configs.rs:1313`.
- `rs_cam_cli/src/project.rs:634-650` `run_project_command` has 14 parameters, 5 of them bools, under `allow(clippy::too_many_arguments)`.
- `rs_cam_viz/src/ui/toolpath_panel.rs:896` `card_menu(.., visible, enabled, has_result, ..)`.
- `dexel_stock/simulation.rs:1332` `run_swept_batch_into_samples(.., from_high, capture_arc_engagement, ..)`.
- `Cargo.toml:35-64` (`[workspace.lints.clippy]`) does not enable `fn_params_excessive_bools` or `struct_excessive_bools`.

**Fix.** Turn on both lints at `warn` in `Cargo.toml`. Give each `allow` a `// SAFETY:` line, as the workspace rule demands. Convert the sites above to two-variant enums, or to an options struct (CLI).
Enforcement: NONE now, LINT after the fix.

---

## Not filed (checked and rejected)

- `ViewportState` has 32 `show_*: bool` fields (`state/viewport.rs:57-179`). These are independent toggles, not exclusive modes, and the overlay registry owns them.
- `geometry/marching_squares.rs:94 cell_case(bl, br, tr, tl)` is four bools for the corners. They are the domain value.
- `FaceUp::from_key` / `ZRotation::from_key` return `Option` since CMP-15. `parse_tool_type` refuses unknown names (Q4 carve-out).
- `set_toolpath_param` / `set_dressup_field` duplicate the 0/1-to-bool and string-to-number coercion (`params.rs:352-404`, `mutation/config.rs:107-137`). This is a duplication finding, not a bool/string finding. It is noted here for the duplication sweep.
