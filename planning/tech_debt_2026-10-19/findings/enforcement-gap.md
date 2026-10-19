# Findings: enforcement gaps (`enforcement-gap`)

Baseline 99b6fb41, branch `geo-select`. Read-only sweep. No cargo run.
Counts come from `rg`, short Python scans over the source text, and file
reads. Each section gives the command or the method, so a reviewer can run
the count again.

Severity uses the rubric in `CHECKLIST.md` (Impact x current grade). "In
flux" means at least one site is in `instruments/in_flux_paths.txt`.

## Summary

| id | title | tier | cost | now -> fix | sev | in flux | known_id |
|---|---|---|---|---|---|---|---|
| EG-01 | Hand-listed identity keys and reply fields have no destructure guard | A | M | NONE -> TYPES | S1 | yes | — |
| EG-02 | Enum dispatch fails open: `_ =>`, `matches!` and `==` swallow a new variant | B | L | NONE -> TYPES | S1 | yes | — |
| EG-03 | Hand-kept `ALL` arrays beside the enum; `ids::ALL` misses 8 of 100 now | B | M | NONE -> TYPES | S2 | yes | — |
| EG-04 | The op-param macros accept a missing decision (no range, no role key) | B | M | CONVENTION -> TYPES | S2 | yes | — |
| EG-05 | View-side `Effects` mirroring is held by a source-scan ratchet | B | M | SENTRY -> TYPES | S3 | yes | — |
| EG-06 | Lenient token parsers default in silence on project load | B | M | NONE -> TYPES | S1 | yes | — |
| EG-07 | `rs_cam_mcp` wire tokens are `String`, against its own CLAUDE.md | B | M | DOC -> TYPES | S2 | yes | — |
| EG-08 | Hand-built `json!` MCP replies drop a new core field in silence | B | L | NONE -> TYPES | S1 | yes | — |
| EG-09 | Dressup knowledge is held by string keys and parallel lists | B | M | NONE -> TYPES | S2 | yes | — |
| EG-10 | A public mutation door outside `apply` that the scanner cannot see | B | S | SENTRY -> TYPES | S3 | yes | — |
| EG-11 | The `// SAFETY:` rule for `allow(...)` has no enforcer | B | M | CONVENTION -> LINT | S3 | no | Q7-Q10 |
| EG-12 | Cross-crate source-scan sentries run in no per-crate gate | B | M | SENTRY -> SENTRY | S3 | yes | T-3 |
| EG-13 | The lint policy has no design lint for the defect class the cold starts found | E | L | NONE -> LINT | S2 (batch S3) | no | Q12 |
| EG-14 | "No literal colour in a pipeline" has a blind sentry and is broken | E | S | DOC -> TYPES | S3 | no | — |
| EG-15 | "Kept in lockstep" comments and small CLAUDE.md rules that code already breaks | D | S | DOC -> TYPES | S2 | yes | — |
| EG-16 | "`None` means not measured" is held by prose only | E | L | DOC -> TYPES | S2 | no | — |

No finding is a property of the geometry-selection programme.

---

## EG-01 (A) Hand-listed identity keys and reply fields have no destructure guard

**Claim.** Several functions copy the fields of a struct by hand into a key,
a hash, a predicate or a JSON reply. No compiler link binds the list to the
struct. A new field is left out in silence. One site loses operator data
today.

**Live defect.** `crates/rs_cam_core/src/io/tool_library.rs:298`
`dedupe_key` reads 7 fields: `tool_type`, `diameter`, `flute_count`,
`cutting_length`, `taper_half_angle`, `included_angle`, `corner_radius`.
`ToolConfig` (`crates/rs_cam_core/src/compute/tool_config.rs:161-193`) has
more geometry: `shaft_diameter`, `corner_radius_mm`, `helix_deg`,
`shank_diameter`, `stickout`. `shaft_diameter` is cutting geometry: the
tapered-ball cutter reads it (`crates/rs_cam_core/src/compute/cutter.rs:37`).
`add_or_replace_to` (`tool_library.rs:145-165`) REPLACES the first entry
with an equal key. The GUI "save to library" path calls it
(`crates/rs_cam_viz/src/ui/properties/tool.rs:104`). Failure: save a tapered
ball nose that differs from a library entry only in `shaft_diameter` (or in
`stickout`, which drives holder collision). The library entry is overwritten
and the old tool is lost. `dedupe_library` (`tool_library.rs:316`) deletes
one of the two for the same reason.

**Other sites of the same pattern (no defect proved, same failure mode).**
- `crates/rs_cam_core/src/session/mod.rs:911` `generation_inputs_signature`:
  a `format!` of 8 parts with `serde_json::to_string(..).unwrap_or_default()`.
  A serialise error compares as equal, and a new `ToolpathConfig` field is
  not forced in. The GUI replaces the whole config each frame and depends on
  this gate (viz-ui map lead 3). In flux.
- `crates/rs_cam_core/src/session/compute/diagnostics.rs:812-814`: "Every
  session part read here ... must be in the load-report key or in
  `TriageSessionKey`". Prose only. A missed part makes the GUI triage cache
  stale while MCP rebuilds (cold-start task7, both variants). In flux.
- `crates/rs_cam_viz/src/state/simulation/issue_triage.rs:148`
  `evidence_fingerprint`: a hand list of hashed fields (cold-start task7).
- `crates/rs_cam_core/src/compute/simulate.rs:987-995`: the provenance tool
  hash reads diameter, length, shank, stickout and flutes. It omits the tool
  type and the tip geometry. I did not trace the consumer, so the impact is
  not proved. In flux.
- `crates/rs_cam_core/src/gcode/wizard_overlay.rs:123-135` `is_empty`: an
  11-field hand list.
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:734-749`: the
  `toolpath_summaries` reply copies 11 fields of the core summary by name
  (see EG-08). In flux.

**In-repo template.** `LoadedModel::adopt_geometry`
(`crates/rs_cam_core/src/session/mod.rs:494-506`) destructures `fresh` with
`let Self { mesh, items, .., id: _, name: _, units: _ } = fresh;`. A new
field there is a compile error.

**Fix.** At each site, destructure the source struct with no `..`, and name
each excluded field as `field: _` with a comment. Add `shaft_diameter`,
`corner_radius_mm`, `helix_deg`, `shank_diameter` and `stickout` to
`dedupe_key`, or key the replace on the tool name and ask before an
overwrite. Replace `unwrap_or_default()` in the signature with a typed hash
of the destructured fields.

**Enforcement.** Now NONE (DOC at the `TriageSessionKey` site). Fix TYPES.
Impact H (data loss) x Weak = S1.

---

## EG-02 (B) Enum dispatch fails open: `_ =>`, `matches!` and `==` swallow a new variant

**Claim.** Per-operation, per-tool and per-dialect decisions sit in `match`
arms that end in `_ =>`, or in `matches!` / `==` tests. A new variant
compiles and takes the fallback. All eight cold-start tasks (both variants)
report this class as a silent miss.

**Broken rule.** `crates/rs_cam_core/src/compute/CLAUDE.md:20`: "a consumer
reads `OperationConfig::capabilities()`, never a `match` on the op type".
`crates/rs_cam_core/src/feeds/CLAUDE.md` repeats it for feeds. The op
matches below break it.

**Evidence (read).**
- `crates/rs_cam_viz/src/ui/properties/operations/shape_diagrams.rs:54`
  `StepoverPattern::from_operation` ends `_ => None`; the panel then draws no
  diagram (task1, both variants).
- `crates/rs_cam_viz/src/ui/properties/operations/registry.rs:421-428`
  `val_stepover_under_diameter` ends `_ => return`. In flux.
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:999-1020`
  `apply_stock_defaults` ends `_ => {}`. `feeds/CLAUDE.md` says "read
  `capabilities()`, add no op `match`"; this op match breaks that rule.
- `crates/rs_cam_core/src/feeds/suggest/axial_envelope.rs:317-326` and
  `:347-351`: `_ => "finish_3d"` and an outer `_ => {}` that the comment says
  is "kept ... so routing changes fail soft".
- `crates/rs_cam_core/src/tool/mod.rs:330-352` `cusp_radius` and `:396-399`
  `valley_radius_mm` end `_ => self.radius()`. The G-BULLCUSP comment at the
  same place records a release lost to this exact fallback (task6).
- `crates/rs_cam_core/src/compute/tool_config.rs` holds 10 `_ =>` arms
  (`envelope_diameter`, `summary`, `diameter_field_label`, `name_claim`,
  `angle_claim`; task6), and `has_ball_tip` at `:40-42` is
  `matches!(self, BallNose | TaperedBallNose)`.
- `crates/rs_cam_core/src/gcode/mod.rs:1488-1496` `PostFormat::from_token`
  ends `_ => None` (task8; see EG-06 for the load fallback). In flux.
- `==` in place of a capability: `PencilDetector::RestDepth` at
  `crates/rs_cam_core/src/session/project_file.rs:1418`,
  `crates/rs_cam_core/src/session/mutation/config.rs:258`,
  `crates/rs_cam_viz/src/app/mcp/commands.rs:2955`,
  `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs:351-352,483`;
  `PostFormat::GrblHal` at `crates/rs_cam_viz/src/ui/export_wizard.rs:590`;
  `ControllerFirmware::GrblHal` at
  `crates/rs_cam_viz/src/ui/properties/machine_panel.rs:542`.

Count: `rg -c "^\s*_\s*=>" -g '*.rs' crates/*/src` gives 490 lines (not all
over enums). `matches!(` occurs 815 times in `src`.

**In-repo template.** `operation_feeds_hints`
(`crates/rs_cam_core/src/feeds/suggest/apply.rs:1023-1031`) states "the
per-op decision table lives there as an exhaustive match; the old `_ =>
(None, None, None)` wildcard is gone".

**Fix.**
1. Move each per-variant decision into a method on the enum or a
   `Capabilities` field, written as a `match` with every variant named.
2. Replace `x == Enum::A` capability tests with that method
   (`PencilDetector::reads_prior_stock`, `PostFormat::option_panel`).
3. Enable the lint in EG-13 so the next `_ =>` over an enum needs an
   `#[expect(..., reason = "...")]`.

**Enforcement.** Now NONE. Fix TYPES (exhaustive match) plus LINT. Impact H
(G-BULLCUSP shipped a wrong spacing) x Weak = S1. Partly in flux.

---

## EG-03 (B) Hand-kept `ALL` arrays beside the enum

**Claim.** 45 `const ALL` arrays exist in `src`. Four are generated by a
macro: `CommandId::ALL` (`crates/rs_cam_core/src/session/command.rs:2235`),
`OperationType::ALL` (`crates/rs_cam_core/src/compute/catalog.rs:364`),
`UiCommandId::ALL` (`crates/rs_cam_viz/src/ui_command.rs:975`) and
`SettingToken::ALL` (`crates/rs_cam_core/src/settings/mod.rs:110`). The rest
are hand lists. A `&[T]` slice has no length, so a missing variant compiles.

**Live drift.** `crates/rs_cam_core/src/diagnostics/ids.rs:416` `ALL` lists
93 of the 100 `pub const ...: &str` ids in the file. It omits
`GEOM_CLIPPED_BAND`, `GEOM_RAMP_REACH_CLAMP`, `GEOM_OFFSET_LIBRARY_FAILURE`,
`GEOM_BOUNDARY_CLIP_DROPPED`, `GEOM_WATERLINE_LEVELS_BELOW_STOCK`,
`GEOM_WATERLINE_LADDER_EMPTY`, `CONFIG_DERIVED_STEPOVER` and
`CONFIG_CLAIMS_REFERENCE` (Python scan of the file). The uniqueness test
(`crates/rs_cam_core/src/diagnostics/tests.rs:44`) iterates `ALL`, so it
cannot see a duplicate among the eight. In flux.

**Weak guards.** `ToolType::ALL` (`crates/rs_cam_core/src/compute/tool_config.rs:22`)
and `PostFormat::ALL` (`crates/rs_cam_core/src/gcode/mod.rs:1435`) are held
by literal counts in another crate: `crates/rs_cam_viz/src/state/job.rs:617-627`
asserts `len() == 5` and `len() == 4`. The GUI add-tool menu and the type
combo iterate `ToolType::ALL` (`crates/rs_cam_viz/src/ui/properties/tool.rs:165`),
so a missing variant is unreachable and unparseable (task6, task8).
Also `CutterKind::ALL` (`crates/rs_cam_core/src/feeds/mod.rs:256`) and
`SimMetric::ALL` (`crates/rs_cam_core/src/stock/sim_measurability.rs:108`).

**Fix.** Generate each `ALL` from the variant list with a declarative macro
in the `for_each_command!` style, or derive it (`strum::VariantArray`).
Make the diagnostic ids an enum (`DiagnosticId`) with a generated `ALL` and
a `const fn as_str`. Delete the literal-count tests.

**Enforcement.** Now NONE (a cross-crate literal-count SENTRY for two enums).
Fix TYPES. Impact M x Weak = S2.

---

## EG-04 (B) The op-param macros accept a missing decision

**Claim.** The parameter schema is written by `#[derive(OpParams)]` and by
`impl_operation_params!`. Both compile when a contributor leaves out a
decision, and the omission becomes a runtime default.

**Evidence.**
- `crates/rs_cam_derive/src/lib.rs:209-211`: a field with no `range`,
  `min`, `gt` or `max` gets `None`. `check_param_range`
  (`crates/rs_cam_core/src/session/compute/params.rs:64-82`) then accepts any
  value: "A param with no declared range is unchanged". A Python scan of
  `crates/rs_cam_core/src/compute/operation_configs.rs` finds 64 of 203
  published numeric fields with no bound and no `frame_dependent`, `skip`
  or `driver_set`. About 44 of them are `feed_rate` / `plunge_rate` with no
  `#[param]` (example: `operation_configs.rs:221`). `rg` finds no
  non-positive feed guard in `gcode/` or `export/gcode_validator.rs`; the
  downstream path for a value of 0 or less is not verified. Cold-start task2,
  both variants.
- `crates/rs_cam_core/src/compute/operation_configs.rs:2779-2785`:
  `impl_operation_params!` takes `$(stepover: ..)?`, `$(depth_per_pass: ..)?`,
  `$(total_depth: ..)?`, `$(ramp_feed_rate: ..)?` as optional clauses. An
  op that has a `stepover` field and omits the clause returns `None`. Suggest
  then skips the dial and the setter refuses it (task1, both variants).

In flux: `operation_configs.rs`.

**Fix.**
1. Make the derive refuse a published numeric field with no bound. Give an
   explicit escape key, `unbounded = "<reason>"`, beside `frame_dependent`.
2. Give each config field a role in the derive (`#[param(role = stepover)]`)
   and generate the `OperationParams` accessors from it. Delete the optional
   macro clauses.

**Enforcement.** Now CONVENTION (a doc line per field). Fix TYPES
(`compile_error!` at the field). Impact M x Weak = S2.

---

## EG-05 (B) View-side `Effects` mirroring is held by a source-scan ratchet

**Claim.** The viz rule is "mutate core through `apply(Command)` and use
`Effects.stale`" (`crates/rs_cam_viz/CLAUDE.md`, GUI contracts). The
controller has two doors that mirror the answer:
`apply_controller_command` and `apply_quietly`
(`crates/rs_cam_viz/src/controller/events/mod.rs:65-111`). But
`AppState.session` is reachable, and production code calls
`session.apply(` directly and mirrors by hand at:
`crates/rs_cam_viz/src/app/input.rs:115,151`, `app/mcp.rs:373,826`,
`app/mcp/commands.rs:313`, `app/obstacle_marks.rs:127`,
`controller/io.rs:34,398`, `controller/events/model.rs:258,816`,
`controller/events/compute.rs:184,933`, `controller/events/simulation.rs:848`,
`controller/events/undo.rs:266`. Three mirror shapes exist:
`stamp_stale` + `invalidate_simulation`, `adopt_post_effects`
(`app/input.rs:117`), and the panel epilogue in
`ui/properties/panel_apply.rs` (viz-ui map lead 1).

**Guard today.** `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`
scans the source for `let _ =` and `if let Err` shapes against a 21-entry
allowlist. Its doc names its own limit: "A call through a receiver this scan
cannot name is not seen." WP19 found a live red at `app/input.rs`
(`f21aea33`), so the rule has broken before.

**Fix.** Make `session` private to the controller. Give the view one door
whose return type is the mirrored answer, for example
`fn apply(&mut self, c: Command) -> Result<Mirrored, SessionError>`. Or wrap
the core answer in a `#[must_use]` token that only `stamp_stale` consumes.

**Enforcement.** Now SENTRY (text scan). Fix TYPES. Impact M x Medium = S3.
In flux: `controller/events/mod.rs`, `app/mcp/commands.rs`, `panel_apply.rs`.

---

## EG-06 (B) Lenient token parsers default in silence on project load

**Claim.** Some project-file token parsers warn on an unknown token
(`FaceUp::from_key` and `ZRotation::from_key` return `Option`, and the loader
warns at `crates/rs_cam_core/src/session/project_file.rs:810,837`). Others
fall back in silence. The "warn on an unknown token" rule is a convention.

**Silent sites.**
- `crates/rs_cam_core/src/session/mod.rs:613-619` `Corner::from_key`
  (`_ => FrontLeft`).
- `crates/rs_cam_core/src/session/mod.rs:662-672` `XYDatum::from_key`
  (`_ => default`).
- `crates/rs_cam_core/src/session/mod.rs:705-714` `ZDatum::from_key`:
  `offset:<bad>` becomes `FixedOffset(0.0)` and an unknown token becomes
  `StockTop`. The loader calls it at `project_file.rs:1063` with no
  `ProjectLoadWarning`. `FixedOffset` and `MachineTable` are not applied to
  motion yet (`crates/rs_cam_core/src/gcode/mod.rs:337-339`); the header
  names the declared datum, so the wrong value reaches the operator text.
- `project_file.rs:882-888` `parse_tool_material` (`_ => Carbide`) and
  `:890-897` `parse_cut_direction` (`_ => UpCut`).
- `crates/rs_cam_core/src/gcode/mod.rs:1608-1614` `post_format_or_grbl`:
  an unknown post token loads as GRBL. The comment calls it deliberate. A
  GRBL program for a LinuxCNC machine is a safety-relevant fallback.

The input is a hand-edited or out-of-date project file. The operator ruled
that no legacy support is needed (2026-09-16).

**Fix.** Make every token type a serde enum with `rename_all`, or give each
`from_key` the return type `Option<Self>`. Refuse an unknown token at load,
or push a `ProjectLoadWarning` at the one call site. Delete the
`unwrap_or(0.0)`.

**Enforcement.** Now NONE. Fix TYPES. Impact H (dialect, datum text) x Weak
= S1 by the rubric. The input is a hand-edited or old file, so the operator
may choose to schedule it as S2. In flux:
`session/mod.rs`, `session/project_file.rs`, `gcode/mod.rs`.

---

## EG-07 (B) `rs_cam_mcp` wire tokens are `String`, against its own CLAUDE.md

**Claim.** `crates/rs_cam_mcp/CLAUDE.md:7-11`: "A value with a fixed set of
legal tokens takes an enum, not a `String` with the tokens in prose." The
crate breaks this rule at `crates/rs_cam_mcp/src/server.rs:199,466,646`
(`operation_type: String`), `:668` (`tool_type: String`), `:360`
(`workspace`), `:603` (`kind`), `:782,1756,1811` (`material`), `:832`
(`source`), `:1530` (`strategy`), `:1554` (`scope`). The legal values sit
in prose: `server.rs:1973` ("Valid types: face, pocket, ...") and `:1989`
(tool types). The test checks one name only (cold-start task1, both
variants). `OperationType` and `ToolType` exist in core.

**In-repo template.** The S4 stock-change wire enum near `server.rs:1669`
("A mirror of ... `StockChangeOp`") with `#[schemars(inline)]`.

**Fix.** Type each field as the core enum (or a wire mirror with `From`),
derive `JsonSchema` with `#[schemars(inline)]`, and delete the prose lists.
The schema then carries the legal values, and a new variant reaches the
client with no edit.

**Enforcement.** Now DOC. Fix TYPES. Impact M x Weak = S2. In flux:
`server.rs`.

---

## EG-08 (B) Hand-built `json!` MCP replies drop a new core field in silence

**Claim.** The viz MCP handlers build replies with `serde_json::json!`
literals: 279 calls in `crates/rs_cam_viz/src/app/mcp.rs`,
`app/mcp/*.rs` and `controller/events/compute.rs`
(`rg -c "json!\("`; 17 of them are in `app/mcp/tests.rs`). A new field on
the core struct compiles and never reaches MCP. The operator rule "same
project state, same number on every surface" is held by convention.

**Evidence.**
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:734-749`: 11 named fields of
  the per-toolpath summary. The next array in the same reply uses
  `serde_json::to_value(s)` (`:755-760`), so both styles sit side by side.
- `padding` is a `StockConfig` field
  (`crates/rs_cam_core/src/compute/stock_config.rs:77`) with a GUI widget.
  `SetStockConfigParam` (`crates/rs_cam_mcp/src/server.rs:758-790`) has no
  `padding` field, so MCP cannot set it (cold-start task5, both variants).
- Cold-start task4 and task7 name `pin_report_json`
  (`app/mcp/project.rs`), `build_mcp_diagnostics`
  (`controller/events/compute.rs`) and the `inspect_stock` reply as the same
  class.

**Fix.** Give each reply a `#[derive(Serialize)]` struct in `rs_cam_mcp`,
or serialise the core struct. Where a handler must pick fields, destructure
the source with no `..` (EG-01).

**Enforcement.** Now NONE. Fix TYPES. Impact H (two surfaces disagree) x
Weak = S1. In flux: `app/mcp/commands.rs`,
`app/mcp/project.rs`, `server.rs`.

---

## EG-09 (B) Dressup knowledge is held by string keys and parallel lists

**Claim.** `DressupConfig::FIELD_DEFS` exists, but other dressup facts are
string matches and hand lists that fail open.

**Evidence.**
- `crates/rs_cam_core/src/compute/config.rs:1191-1199`
  `owner_of_value_field` ends `_ => None`; `:1220-1228` `is_dressup_enabled`
  ends `_ => false`. A forgotten row lets `set_dressup_field` accept a value
  while the dressup is off (task3, both variants).
- `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:282-283`
  `let total = 8;` beside hand counters. `rg segment_merge
  crates/rs_cam_viz/src` finds no hit outside `mcp_server.rs`, so the newest
  dressup has no GUI control and no count. The badge text is already wrong.
- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:253`
  `DRESSUP_PIPELINE` and the body of `apply_dressups` are two lists of one
  order (task3).

**Fix.** Make the dressup key an enum generated from `FIELD_DEFS`. Put the
owner key and the enabled accessor on the row. Derive the GUI count from the
same list.

**Enforcement.** Now NONE (one generative name test). Fix TYPES. Impact M x
Weak = S2. In flux: `compute/config.rs`.

---

## EG-10 (B) A public mutation door outside `apply` that the scanner cannot see

**Claim.** `crates/rs_cam_core/src/session/CLAUDE.md`: "The one door for
project state and compute is `ProjectSession::apply(Command)`" and "There
are no public `*_mut` hatches". `crates/rs_cam_core/src/compute/validate.rs:164-175`
`pub fn apply_stale_default_fix(session: &mut ProjectSession, ..)` writes
configs through `toolpath_configs_mut()` with no `drop_result` and no
`Command` row. Only tests call it (`validate.rs:631,772`). Its sibling
`apply_stale_default_to_op` (`:182`) is the GUI in-place path
(core-compute map lead 2).

**Why the sentries miss it.** `setters_are_crate_private_wp15b.rs` checks
`fn ... (&mut self` inside `impl ProjectSession` only (its doc, line 19-22).
A free function that takes `&mut ProjectSession` is outside its walk.
`toolpath_configs_mut` is `pub(crate)` and not `#[cfg(test)]`, so any core
module can write configs with no invalidation (core-session map leads 6-7).

**Fix.** Delete `apply_stale_default_fix`, or route it through a `Command`.
Put `toolpath_configs_mut` behind `#[cfg(test)]`, or give it a return type
that drops the result on `Drop`.

**Enforcement.** Now SENTRY (text scan with a blind spot). Fix TYPES. Impact
M x Medium = S3. In flux: `compute/validate.rs`.

---

## EG-11 (B) The `// SAFETY:` rule for `allow(...)` has no enforcer

**Claim.** `CLAUDE.md:110`: "Every production `allow(...)` carries a
`// SAFETY:` line". No lint or test holds it. `scripts/debt_scan.py:93-108`
is an instrument, not a gate. `instruments/allows_without_safety.md` counts
438 of 693 production allows with no `SAFETY` in a 3-line window. A
re-check of those 438 rows: 340 carry a same-line or previous-line comment,
98 carry no comment. 15 of the 98 are one pattern:
`#[allow(clippy::needless_pass_by_value)] Parameters(..)` on rmcp handler
arguments in `crates/rs_cam_viz/src/mcp_server.rs` (the framework fixes the
signature). Other bare clusters: `dexel_stock/simulation.rs` 5,
`surface/slope.rs` 5, `compute/execute.rs` 4, `viz/compute/worker.rs` 4.
`rg "#\[expect\("` finds 0 uses.

**Known.** Q7-Q10 (09-16 plan, below the cut line). What changed: the
instrument still over-counts (Q7 not fixed). The new point: an
`#[expect(lint, reason = "...")]` fails when its lint stops firing, so the
compiler retires a stale suppression. That is the class of the 26 inert
`allow(dead_code)` lines that S26-S28 deleted by hand.

**Fix.**
1. Add `allow_attributes = "deny"` and
   `allow_attributes_without_reason = "deny"` to `[workspace.lints.clippy]`.
2. Convert each production `#[allow(x)] // SAFETY: why` to
   `#[expect(x, reason = "why")]` with a script.
3. Change the CLAUDE.md line to a pointer to the lint.

**Enforcement.** Now CONVENTION. Fix LINT. Impact L x Weak = S3.

---

## EG-12 (B) Cross-crate source-scan sentries run in no per-crate gate

**Claim.** 73 test files read source text (`rg -l` for a `src` path plus
`read_to_string`/`include_str`). At least six core tests read viz sources by
a literal path: `hatches_are_crate_private_wp7.rs:118`,
`setters_are_crate_private_wp15b.rs:383`,
`stale_set_has_one_answer_wp28.rs:81`, `air_cut_denominators_lh1.rs:269-280`,
`gen_inputs_one_assembly_n12.rs:248`, `loose_executor_is_crate_private_wp12.rs:329`
(all under `crates/rs_cam_core/tests/`). `cargo test -p rs_cam_viz` runs
none of them. `crates/rs_cam_viz/tests/dressup_field_names_are_published.rs:30`
is `#![cfg(feature = "mcp")]`, so a build without the feature skips it
(task3).

**Known.** T-3 (open). What changed: T-3 counted ten; the literal-path count
above is a floor, and several of these sentries now hold type-level rules
(visibility, one door) that EG-05 and EG-10 move into types.

**Fix.** Move the rules that are visibility or one-door rules into types
(EG-05, EG-10). Put the rest in one workspace-level test target that both
crate gates run, and name it in the viz folder CLAUDE.md files.

**Enforcement.** Now SENTRY (not run by the gate that matches the change).
Fix SENTRY in a shared target, TYPES where possible. Impact M x Medium = S3.
In flux: most files under `tests/`.

---

## EG-13 (E) The lint policy has no design lint for the defect class the cold starts found

**Claim.** `Cargo.toml` `[workspace.lints.clippy]` denies panic and
hygiene lints only. It does not turn on `wildcard_enum_match_arm`,
`match_wildcard_for_single_variants`, `allow_attributes`,
`allow_attributes_without_reason` or `fn_params_excessive_bools`.
`[workspace.lints.rust]` sets `unsafe_code = "deny"`, which an `allow` can
override; `forbid` cannot be overridden (`rg "allow\(unsafe_code\)"` finds
no use today).

**Known.** Q12 (09-16) assessed six cast and float lints and proposed none.
What changed: Q12 did not assess the wildcard lints, and the eight cold-start
runs show that wildcard arms are the main silent-miss mechanism (EG-02).
This finding disagrees with Q12 on that new evidence.

**Fix.**
1. Set `unsafe_code = "forbid"` (no site to change).
2. Turn on `wildcard_enum_match_arm` per folder as `warn`, convert each
   deliberate arm to `#[expect(..., reason = "...")]`, then raise it to
   `deny`. Start with `compute/`, `tool/`, `feeds/suggest/`, `gcode/` and
   `ui/properties/operations/`.

**Enforcement.** Now NONE. Fix LINT. Impact M x Weak = S2 for the wildcard
part; the `forbid` part is S4. Recorded as S3 for the batch.

---

## EG-14 (E) "No literal colour in a pipeline" has a blind sentry and is broken

**Claim.** `crates/rs_cam_viz/src/render/CLAUDE.md`: "A colour belongs in
`colors.rs`. Do not write a literal colour in a pipeline." The pipelines
write `[f32; 3]` literals:
`crates/rs_cam_viz/src/render/toolpath_render.rs:228-230,471`,
`crates/rs_cam_viz/src/render/sim_render.rs:135,175` (normals excluded).
The only colour sentry, `crates/rs_cam_viz/tests/panels_read_the_token_module_up1.rs:122`,
counts `Color32::from_rgb(` only, so it cannot see an `[f32; 3]`.

**Fix.** Add a `pub struct Rgb([f32; 3])` in `render/colors.rs` with a
constructor private to that module. Make the vertex builders take `Rgb`.

**Enforcement.** Now DOC. Fix TYPES. Impact L x Weak = S3.

---

## EG-15 (D) "Kept in lockstep" comments and small CLAUDE.md rules that code already breaks

**Claim.** Several comments hold a rule that one shared item could hold.

- `crates/rs_cam_core/src/finish/unified_finish.rs:275-290` and
  `crates/rs_cam_core/src/compute/operation_configs.rs:2160-2165`: the core
  default and the serde default are "kept in lockstep" by comment. The
  comment says "a core default and a serde default that disagree is the
  divergence class this programme has already found twice". Fix: the core
  `Default` calls the serde default fn, or both read one `pub(crate) const`.
- `crates/rs_cam_cli/src/nc_replay.rs:51-59` `builtin_machine` hardcodes
  the `shapeoko_xxl_ricky_tuned` preset and two feed caps. `crates/rs_cam_cli/CLAUDE.md`
  says "Do not hardcode a preset in a subcommand". Fix: read the default from
  `io::machine_library`.
- `crates/rs_cam_cli/src/project.rs:309` calls `session.apply` directly; the
  CLI door doc (`crates/rs_cam_cli/src/command.rs:1-8`) says every CLI site
  takes `apply_command`.

**Enforcement.** Now DOC. Fix TYPES (one const) for the defaults; a one-line
change for the CLI items. Impact M x Weak = S2 for the defaults (two past
breaks), S4 for the CLI items. In flux: `operation_configs.rs`.

---

## EG-16 (E) "`None` means not measured" is held by prose only

**Claim.** `crates/rs_cam_core/src/stock/CLAUDE.md:18-19` and
`crates/rs_cam_core/src/diagnostics/CLAUDE.md:20`: `None` means not
measured, `Some(0.0)` means measured zero. The summary fields are
`Option<f64>`, and any reader can collapse them with `unwrap_or(0.0)`.
The X-VAC comment at `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1210-1215`
records a past break: a vacuous gate painted a green "0%". Today the two
`display_peak.unwrap_or(0.0)` reads (`sim_diagnostics.rs:1209,1335`) sit
behind an `is_vacuous()` guard, so no live break is proved. Cold-start task4
and task7 name the rule as a trap.

**In-repo template.** `crates/rs_cam_core/src/tool_load/distribution.rs:310,339`
`DistributionOutcome::NotMeasured(NotMeasuredReason)`.

**Fix.** Add a `Measured<T>` enum with `NotMeasured(reason)` and
`Value(T)`, with no `Default` and no `unwrap_or`. Move the summary fields to
it one struct at a time.

**Enforcement.** Now DOC. Fix TYPES. Impact M x Weak = S2, recorded as E
because no live break is proved.
