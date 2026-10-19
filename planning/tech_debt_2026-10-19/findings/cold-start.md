# Cold-start traces: enforcement gaps (baseline 99b6fb41)

This file turns the eight cold-start traces in `coldstart/task1.json` to
`coldstart/task8.json` into verified findings. Each task has two traces. The
"with" agent read the `CLAUDE.md` files. The "without" agent did not.

The reviewer checked each claim below in the code with `rg` and file reads.
A claim that the reviewer did not check is not in a finding. The findings
have the ids CS-1 to CS-19. The structured output holds the full site lists.

## 1. Forced and silent sites per task

"Forced" means the trace says the compiler stops the build until the site
changes. "Silent" is the count of silent misses that the trace recorded.

| Task | Change | With: sites / forced / silent | Without: sites / forced / silent |
|---|---|---|---|
| 1 | new 2.5D operation | 24 / 14 / 12 | 27 / 15 / 15 |
| 2 | new numeric param on Pocket | 16 / 2 / 11 | 14 / 2 / 11 |
| 3 | new dressup | 28 / 5 / 13 | 21 / 6 / 13 |
| 4 | new MCP diagnostic + same GUI number | 15 / 5 / 10 | 17 / 5 / 9 |
| 5 | new project-level stock setting | 19 / 5 / 9 | 21 / 5 / 15 |
| 6 | new cutter shape | 49 / 18 / 25 | 42 / 20 / 15 |
| 7 | new simulation warning | 14 / 2 / 10 | 17 / 4 / 12 |
| 8 | new G-code dialect | 26 / 5 / 11 | 20 / 7 / 10 |

Over the 16 traces, the compiler forces about one site in three (120 of
370). The other sites are found by search, by a sentry, or not at all.
Tasks 2, 4, 5 and 7 have the lowest forced share. These four tasks add a
field or a value to an existing shape, and no `match` names fields.

## 2. Root causes

Each root cause is one finding. The task numbers show which traces hit it.

| Id | Tier | Root cause | Tasks |
|---|---|---|---|
| CS-6 | A | `dedupe_key` names 7 of the tool fields, so a library save overwrites a different tool | 6 |
| CS-1 | B | an `ALL` list is written by hand beside its enum; `ids::ALL` already misses 8 ids | 1, 6, 7, 8 |
| CS-2 | B | a token resolver ends `_ => None` and the loader then picks a default | 6, 8 |
| CS-3 | B | the dressup vocabulary has a table, but the GUI, the owner match and the count do not read it | 3 |
| CS-4 | B | surface reach is per command, not per field; `padding`, `post.format`, 4 tool fields and 19 op params miss a surface | 1, 2, 3, 5, 6, 8 |
| CS-5 | B | MCP replies and the CLI report copy core structs field by field in `json!` | 4, 5, 6, 7 |
| CS-7 | B | tool-shape facts are `_ =>` arms and a trait default (`geometry_hint` gives `Flat`) | 6 |
| CS-8 | B | per-op facts sit in `_ =>` decision arms (18 such arms in production; the 5 in `feeds/suggest` break the W2.4 rule) | 1, 2 |
| CS-10 | B | the `OpParams` derive publishes a number with no domain; 107 of 269 have none | 2 |
| CS-12 | B | cache keys list their inputs by hand, so the GUI and MCP can show two numbers | 4, 5, 7 |
| CS-19 | B | "None means not measured" is prose; a plain `f64` compiles | 4, 7 |
| CS-9 | D | two copies of one rule already disagree (stepover check, verdict, cutter factory) | 1, 2, 4, 6 |
| CS-11 | D | the project file has twin structs for stock and tool, with defaults typed twice | 5, 6 |
| CS-13 | E | the triage class is a choice of `Vec`; the Warnings tab reads 2 of 3 buckets | 7 |
| CS-14 | E | one order or one row set is held in two places (pipeline list, `OP_UI_ROWS`) | 1, 3 |
| CS-15 | E | GUI rows name a param by a string; only a text-scan sentry checks it | 1, 2, 3 |
| CS-16 | E | agent and CLI help text lists tokens by hand | 1, 6, 8 |
| CS-17 | E | a post dialect is split over TOML data, a code table and `== GrblHal` gates | 8 |
| CS-18 | F | hard-coded counts and stale "how to add" prose | 1, 6, 8 |

## 3. With-CLAUDE.md against without-CLAUDE.md

The two traces of a task found mostly the same silent sites. The
`CLAUDE.md` files did not make the "with" agent find more sites. They made
it name the reuse traps sooner. Both agents found the reuse helpers by
search in most tasks.

A difference between the traces shows where a rule lives only in prose:

- A miss that only the "without" trace records is a rule that a `CLAUDE.md`
  line holds. The "with" agent knew the rule, so it did not count a miss.
- A miss that only the "with" trace records is a site that the prose points
  at. The "without" agent did not know to look.

### Only the "with" trace knew the rule (prose holds it)

- Task 4. The transit-sample filter `locality::is_steady_state_for_gate`
  (`tool_load/CLAUDE.md:16-17`, "Never write a second copy"). The "with"
  trace says it found this only in the operator's auto-memory, not in a
  `CLAUDE.md` it read. So this rule is held outside the repository index.
- Tasks 4 and 7. `None` means not measured; `Some(0.0)` means measured
  clean (`stock/CLAUDE.md:18-19`, `diagnostics/CLAUDE.md:19-20`). See CS-19.
- Task 6. The feeds matrix CSV must be re-blessed in the same commit
  (`feeds/CLAUDE.md:40`). The test fails with a diff, but the env flag is
  in prose.
- Task 3. A dressup stage runs in one of three named places; "Add no
  fourth" (`dressup/CLAUDE.md:8`).
- Task 1. `dressup_policy_table_is_pinned` ends in a wildcard that passes a
  copied `ANY_DRESSUP` (`compute/config.rs:1706`, read). `config_guard!`
  fails at generation time, not at build time (`compute/execute.rs:510-520`,
  read). These two are coverage: the prose did not name them, but the
  "with" agent read the files the prose pointed at.

### Only the "without" trace recorded the miss

The task brief says that such a miss is a rule held only by prose. The
reviewer checked this with `rg` over the 37 `CLAUDE.md` files
(`instruments/claude_md_paths.txt`). No `CLAUDE.md` line holds any of the
six items below. So these are coverage differences, not prose rules: the
"with" agent did not open the site. The prose did not help, and the code
did not force the site.

- Task 2. `is_integer_param_type` matches type strings
  (`session/compute/params.rs:166`). A new integer type is read as a float.
  No instruction line.
- Task 4. Three separate `40.0` air-cut thresholds, and not-measured reason
  strings copied across a `pub(crate)` wall. See CS-9. No instruction line.
- Task 5. `SimulationProvenance.stock_hash` and the load-report key hold a
  hand list of stock parts. See CS-12. No instruction line.
- Task 6. `ToolType::ALL` has no exhaustiveness check except a hard-coded
  `5` in another crate (`rs_cam_viz/src/state/job.rs:620`). See CS-1. The
  "with" trace cited the pinned token tables (`tool_config.rs:1303-1350`),
  so this item is only partly one-sided. No instruction line.
- Task 7. The MCP `verdict` string is an `if` chain apart from the triage
  (`controller/events/compute.rs:2495-2507`). See CS-9 and CS-13. No
  instruction line.
- Task 8. MCP has no field to set the post dialect, but the `SetPostConfig`
  row says `mcp: Reach::Reached` (`session/command.rs:439-444`). See CS-4.
  No instruction line.

The result: the prose-held rules are the five items in the list above this
one. The one-sided misses in this list are gaps that neither the prose nor
the compiler covers. Both kinds need a type-level fix.

### Prose that misleads

- `gcode/CLAUDE.md:23-24` says "Add a `PostDefinition` row, not a branch".
  No row exists. The real change is a TOML file and six code sites.
- `architecture/high_level_design.md:141-148` says a new operation needs
  four files. Those files no longer hold the dispatch.
- `operation_configs.rs:1171` says `trochoid_cap_mult` is "Surfaced in the
  GUI as the Nibble dial". No GUI code reads the field.
- `ui/properties/operations/registry.rs:7-11` says the `_ => {}` diagram
  fallback is gone. `shape_diagrams.rs:54` still ends `_ => None`.

## 4. Per task: what the compiler forces and what it does not

### Task 1: new 2.5D operation

- Forced: the `OperationConfig` variant, `registry_entry`, `capabilities`,
  `surface_mask`, `stock_to_leave_dial`, the three selection matches,
  `cutting_levels`, `classify`, `bottom_z_pin_note`, the GUI re-exports.
- Silent: `ALL_2D` (CS-1), `apply_stock_defaults`, `axial_envelope`,
  `val_stepover_under_diameter`, `StepoverPattern::from_operation` (CS-8),
  the optional `impl_operation_params!` keys, the MCP "Valid types" text
  (CS-16), the `OP_UI_ROWS` row (CS-14, a runtime `unreachable!`), the
  hard-coded `24` counts (CS-18).

### Task 2: new numeric param on Pocket

- Forced: the `Default` literal, and every full `PocketConfig { .. }`
  literal.
- Silent: a missing `range` (CS-10), a missing `#[serde(default)]` (CS-11),
  the GUI row (CS-4, CS-15), the Zigzag-pattern adapter that copies fields
  one by one, the GUI-only cross-field check (CS-9).

### Task 3: new dressup

- Forced: the `DressupConfig` field, the two `From` conversions, `Default`.
- Silent: `owner_of_value_field`, `is_dressup_enabled`, the GUI control,
  `dressup_active_count`, the MCP description lists (CS-3), the pipeline
  order list (CS-14), the per-field serde default on the wire struct.

### Task 4: new MCP diagnostic with the same GUI number

- Forced: a `UiQuery` row and its `match` arm, or a `ProjectDiagnostics`
  field and its literals.
- Silent: the hand-built `get_diagnostics` JSON (CS-5), the cache keys
  (CS-12), the not-measured rule (CS-19), the three registries for an MCP
  read. No test compares a new GUI number with the MCP number.

### Task 5: new project-level stock setting

- Forced: the two `Default` literals, `stock_from_project`,
  `to_project_file`.
- Silent: the MCP patch struct and its copy function, the replies, the CLI
  (CS-4, CS-5), the twin struct defaults (CS-11), the cache keys (CS-12).
  `padding` is the live proof: GUI and file have it, MCP does not.

### Task 6: new cutter shape

- Forced: the `MillingCutter` impl, `CutterKind` and `ToolGeometryHint`
  matches, `build_cutter`, `build_tool`, the render shape.
- Silent: `geometry_hint` default, `cusp_radius`, `valley_radius_mm`,
  `helix_entry_diameter_mm`, nine `ToolType` wildcards (CS-7), `ALL`
  (CS-1), `dedupe_key` (CS-6), `set_tool_param` and `ToolSummary` (CS-4,
  CS-5), the second cutter factory (CS-9).

### Task 7: new simulation warning

- Forced: a new `TriageInputs` field, or a `VerdictKind` arm.
- Silent: `ids::ALL` (CS-1), the bucket choice and the Warnings tab
  (CS-13), `TriageSessionKey` and `evidence_fingerprint` (CS-12), the
  separate MCP `verdict` (CS-9), the `kinds_must`/`kinds_info` arrays
  (CS-1).

### Task 8: new G-code dialect

- Forced: `label`, `definition`, `to_token`, `invariants_for`,
  `ControllerFirmware::post_format`.
- Silent: `PostFormat::ALL` and `from_token` (CS-1, CS-2), the hand lists in
  the post tests (CS-1), `detect_firmware`, the `== GrblHal` gates, the
  TOML flags against `PostInvariants` (CS-17), the CLI help (CS-16).

## 5. The type-level direction

The repository already has the tools for most fixes:

- `for_each_op!` (`compute/catalog.rs:316`), `for_each_command!`
  (`session/command.rs`) and `setting_token!` (`settings/mod.rs:96`)
  generate `ALL`, the token and the label from one row. Use the same shape
  for `PostFormat`, `ToolType`, `CutterKind`, the diagnostic ids and the
  issue kinds.
- `SettingToken::from_token` (`settings/mod.rs:85`) resolves a token by a
  walk over `ALL`. A resolver of this shape cannot forget a variant.
- `Surfaces { gui, mcp, cli }` with `Reach::Skip(reason)` already gives each
  command a census. Put the same census on each published field.
- `rs_cam_derive` already refuses bad `#[param]` keys with
  `compile_error!`. It can also refuse a number with no domain.
- `DistributionOutcome::NotMeasured` (`tool_load/distribution.rs:339`) is a
  typed not-measured value. Use one such type for every published figure.
- `serde_json::to_value(&triage)` (`controller/events/compute.rs:2564`)
  shows the derive path for a reply. Replace the hand-built `json!` mirrors.

No finding in this file is a property of the geometry-selection programme.
