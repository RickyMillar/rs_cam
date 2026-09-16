# L — legacy, compatibility and migration residue

Axis: the 526 production lines in `evidence/legacy_residue.md`, judged against
the no-legacy ruling (core writes and reads `format_version = 3` only). Line
numbers are working-tree at 2026-09-16; `operation_configs.rs`, `catalog.rs` and
`suggest.rs` are under concurrent edit by the power-calcs agent.

**The one fact that decides class 1.** `build_session_from_project` calls
`check_format_version` on its first line (`session/project_file.rs:841`). Every
pre-v3 shim in that file sits *after* the refusal, so a file that declares any
version but 3 never reaches them. Deleting a pre-v3 reader changes no loadable
input.

**The rule that disposes of most hits.** `#[serde(default)]` on a field added
*after* v3 was declared is not legacy support. It is how v3 stays one version.
That disposes of `operation_configs.rs` (29 hits), `catalog.rs` (17) and
`config.rs` (12). Only a *flipped* default is debt, and the one flip
(`default_unified_finish_monotone_cell_decomposition:1367`) carries an operator
ruling that names existing files. Not debt.

## Header table

| id | tier | cost | class | file:line | one-line claim | consumer that breaks | OWNER |
|---|---|---|---|---|---|---|---|
| L1 | A | S | 1 | `core/src/session/project_file.rs:699` | the load-time dressup migration rewrites a v3 operator value and reports only to `tracing::info!` | none (add a warning) | — |
| L2 | A | S | 1+2 | `core/src/compute/operation_configs.rs:602` | `stock_to_leave_radial` is inert, still `ParamDef::required`, and emits no `DeprecatedDialFinding` | none for the S fix | — |
| L3 | B | S | 1 | `core/src/session/project_file.rs:510` | `_legacy_feeds_auto` reads a pre-v3 block the refusal already blocks | none | — |
| L4 | B | S | 1 | `core/src/session/project_file.rs:36` | the top-level `toolpaths` pre-setup reader reads a pre-v3 shape | 98 saved TOMLs lose a `toolpaths = []` line | — |
| L5 | B | S | 2 | `core/src/session/mod.rs:2133` | `standing_material_mm2` is emitted beside the new key although core documents that name as measuring the wrong quantity | external MCP agents; CLI report scripts | — |
| L6 | B | M | 1 | `core/src/session/project_file.rs:123` | the whole `machine_ref` chain is dead end to end: nothing in production writes it, save persists it, load drops it | none | — |
| L7 | C | S | 2 | `core/src/gcode/mod.rs:400` | a trace with no `provenance` block is treated as fresh; the one production producer is the CLI | none today | — |
| L8 | C | S | 1 | `core/src/session/command.rs:642,655,779` | three `Command` variants have no production constructor; all are `Skip` on GUI, MCP and CLI | two tests build `SetProjectName` | — |
| L9 | E | S | 2 | `core/src/compute/tool_config.rs:88` | `parse_lenient` keeps five aliases of a deleted viz loader, and is also the MCP mutation parser | MCP clients sending `ball`/`flat`/`tapered_ball` | — |
| L10 | E | S | 2 | `core/src/session/mod.rs:2203` | `air_cut_percentage` duplicates `air_cut_pct_of_total_runtime` | external MCP agents; CLI report scripts | — |
| L11 | E | S | 2 | `core/src/session/mod.rs:1417` | `ProjectDiagnostics::verdict` duplicates the top `verdicts` headline | CLI report readers | — |
| L12 | E | S | 1 | `core/src/machine.rs:326` | `MachineProfile::from_key` is `pub` for a deleted loader; its one caller is a test shim | `tests/literature_matrix/shim.rs:127` | — |
| L13 | E | S | 3 | `core/src/simulation.rs:1` | a module whose own doc calls itself legacy is the live import path for `StockMesh` | three viz production sites | — |
| L14 | E | S | 2 | `cli/src/job.rs:266` | the `entry_style` serde alias accepts a pre-T9 job-file key | job files using the old key | — |
| L15 | E | S | 2 | `core/src/gcode/post.rs:230` | `default_tool_change` supplies pre-template `M5`/`M6` to a post TOML with no `tool_change` | third-party post TOMLs | — |
| L16 | F | S | 3 | `core/src/feeds/vendor_lut.rs:719` | `LEGACY_DEGENERATE_RANGE_ROWS` is a shrink-only allowlist with its own sentry | the LUT sentry | power-calcs |

## Evidence

### L1 — a migration that rewrites a v3 value, and says so only to the log

```
699:    // One-shot migration: projects saved before operation-specific dressup
...
706:    if dressups.normalize_for_op(op_type) {
707:        tracing::info!(... "Normalized incompatible dressups on load");
```

Every write door already normalises — `mutation.rs:491`, `:521`, `:617`, and
`config.rs:2079` (`DressupConfig::for_op`). But the registry policy has grown
*within* v3: `adaptive3d_interior_cell_parity_f029.rs:45` and
`drop_cutter_off_mesh.rs:212` both record a rule added later. So the call fires
on every v3 file saved before its rule shipped — the common case, not a rare
one — and it overwrites what the operator stored, with no warning.

Consumer: none. Sentries: `tests/toolpath_fields_round_trip_c11.rs:108`,
`tests/adaptive3d_interior_cell_parity_f029.rs:45`,
`tests/drop_cutter_off_mesh.rs:227` (this one relies on the load-time call).
The sibling `parse_tool_type` (`project_file.rs:569`) shows the correct
channel: it pushes a `ProjectLoadWarning::UnknownToolType`. Propose: keep the
normalisation (it prevents a trench) and give it a `ProjectLoadWarning`. Do not
delete it — `drop_cutter_off_mesh` depends on it.

### L2 — an inert dial the operator can still set

`operation_configs.rs:602` reads "Deprecated + inert sidewall leave allowance.
NOT honored by the planner." `adaptive3d_effective_stock_to_leave` (`compute/execute.rs:1885`) returns
`cfg.stock_to_leave_axial` alone. The GUI dial went in 2026-07-06 and no viz
panel names the field. But `catalog.rs:1750` still declares
`ParamDef::required("stock_to_leave_radial", "f64")`, and `cli/src/job.rs:852`
writes it into every adaptive3d job. `feeds/rationale.rs:360` and `:481` name
it in the recipe vocabulary.

So an MCP or CLI caller sets a radial leave, the planner ignores it, and nothing
tells them. That is the failure mode `DeprecatedDialFinding`
(`config.rs:1002`) exists to prevent, and `route_width_factor`
(`execute.rs:2400`) already uses it.

Two fixes. **S, no ruling:** emit a `DeprecatedDialFinding` from
`generate_adaptive3d`, as `route_width_factor` does. **L, needs a ruling:**
delete the field; that changes the persisted file and the MCP param schema, and
breaks `tests/toolpath_fields_round_trip_c11.rs:178-300`.

### L3 — `_legacy_feeds_auto`

`project_file.rs:513` is
`#[serde(default, rename = "feeds_auto", skip_serializing)]`, so nothing writes
it. `ProjectFile` carries no `deny_unknown_fields`, so after deletion an unknown
`feeds_auto` key is simply ignored — the same outcome. Deletion plan: drop the
field and the three `_legacy_feeds_auto: None` initialisers (`save.rs:248`,
`session/mod.rs:2334`, `:2585`). No test names it. A pure read-side no-op.

### L4 — the top-level `toolpaths` pre-setup reader

```
36:    /// Legacy: top-level toolpaths (pre-setup format).
1037:        // Legacy: top-level toolpaths -> single default setup
```

`save.rs:325` writes `toolpaths: Vec::new()` ("format_version=3 uses setups,
not top-level toolpaths"). The reader at `:1037-1074` is the `else` arm of
`if !project.setups.is_empty()`, so it fires only on a v3 file with setups
absent and toolpaths present. No in-repo TOML carries a `[[toolpaths]]` section
(0 of 102). Deletion plan: drop `ProjectFile::toolpaths`, the `else` arm, the
`project.toolpaths.is_empty()` term in `validate_looks_like_cam_project:824`,
and the field in all eight `ProjectFile` literals (`save.rs:319`;
`session/mod.rs:2247`, `:2265`, `:2282`, `:2300`, `:2361`, `:2398`, `:2527`).
Sentry: `session/mod.rs:2438` asserts the error text, which names "toolpaths".

**Caveat.** 98 in-repo TOMLs contain `toolpaths = []`; the `Vec` has no
`skip_serializing_if`. Deletion removes that line from every file this build
writes. Harmless, but it is a persisted-file change.

### L5 / L10 / L11 — the three deprecated wire keys

`viz/tests/snapshots/mcp_wire_surface.json` carries tool **input** schemas only;
none of these keys appears in it. The response keys are pinned by
`core/tests/standing_material_channel_am9.rs`, `air_cut_denominators_lh1.rs` and
`viz/src/controller/results_parity_tests.rs:307`.

- **L5 `standing_material_mm2`** (`session/mod.rs:2124-2133`). Tier B, not E,
  because `compute/config.rs:145-152` states the old name is wrong: *standing*
  means "reached, left high", while the field sums ground no cutter entered. A
  reader who trusts the key name gets the wrong quantity. The CLI mirrors it
  (`cli/src/project.rs:76-78`, `:1097`).
- **L10 `air_cut_percentage`** (`session/mod.rs:2203`, `compute.rs:5211`,
  `cli/src/project.rs:192`, `viz/controller/events/compute.rs:2098`,
  `viz/app/mcp.rs:4572`). Same denominator it always had, so the value is
  honest. Tier E: a duplicate, not a lie.
- **L11 `ProjectDiagnostics::verdict`** (`session/mod.rs:1417`,
  `compute.rs:5448`, `:6982`). Derived from the top `verdicts` entry. Tier E.

None is deletable without naming a consumer, and the consumers are outside the
repo: external MCP agents and the operator's CLI report scripts.
`cli/src/project.rs:1032` calls the key names "a compatibility surface".

### L6 — the `machine_ref` chain is dead end to end

`SetMachineRef` is the only writer, and its registry row (`command.rs:779`) is
`Skip` on all three surfaces. So `session.machine_ref` is always `None`.
`save.rs:174` writes it (skipped when `None`); `project_file.rs:1105` drops it
on load; `viz/app/mcp/commands.rs:1316` reports `machine_library_link_cleared`,
which is therefore always `null`.

Deletion plan (one commit, cost M): `ProjectJobSection::machine_ref`
(`project_file.rs:128`) and the drop branch (`:1102`); `ProjectSession::
machine_ref` (`session/mod.rs:1463`), `machine_ref()` (`:1708`),
`set_machine_ref()` (`:1714`); `Command::SetMachineRef` + args + registry row +
dispatch arm (`command.rs:779`, `:1940`, `:2467`); the two clears
(`mutation.rs:1757`, `:1784`); `save.rs:174`; the always-null field
(`viz/app/mcp/commands.rs:1293-1316`); two tests (`session/mod.rs:2358`,
`:2393`); doc comments in `machine_library.rs:8`, `:205`, `command.rs:1514`,
`:1529`, `viz/ui/properties/mod.rs:371`, `:1861`, `cli/src/project.rs:246`.
Check first that `viz/tests/command_surface_completeness.rs` and
`effects_are_stamped_wp19.rs` do not pin the row.

### L7 — a fallback that would suppress the staleness check

```
400:    let Some(provenance) = trace.provenance.as_ref() else {
403:        // the legacy behaviour of treating it as fresh; ...
404:        return true;
```

`compute/simulate.rs:1393` sets `provenance = Some(...)`, so every simulator
trace carries one. The `None` arm has one production producer:
`cli/src/main.rs:798` (`from_samples`). That trace goes to the printed report
and to a `SimulationCutArtifact` on disk, and no production code deserialises
that artifact back. So the arm never reaches `project_load_report` today. Tier
C, not A: dead, but it reads as load-bearing. Inverting it to "stale" changes a
gate verdict, so it needs a ruling.

### L8 — three commands with no production constructor

`ReplaceSetupsAndToolpaths` (`command.rs:642`), `SetProjectName` (`:655`) and
`SetMachineRef` (`:779`) are all `Skip` on GUI, MCP and CLI; the first two cite
"C01 deleted the legacy project reader". The only `SetProjectName` constructor
left sits in a `#[cfg(test)]` module (`viz/src/app/export.rs:481-500`). The
sweep inherited the first two; `SetMachineRef` is new here, and it goes with L6.
Two tests build `SetProjectName` and need another door
(`core/tests/toolpath_fields_round_trip_c11.rs:155`,
`viz/src/controller/tests.rs:603`); check
`viz/tests/command_surface_completeness.rs` does not pin the three rows.

### L9 / L14 / L15 — the three read-side aliases

- **L9** `ToolType::parse_lenient` (`tool_config.rs:85-97`) accepts `endmill`,
  `flat`, `ballnose`, `ball`, `bullnose`, `vbit`, `taperedballnose` and
  `tapered_ball`. `save.rs` writes canonical tokens only, but the same function
  is the MCP mutation parser, so deleting an alias breaks any client that sends
  it. All 13 tokens are pinned by `parse_lenient_vocabulary_is_pinned`.
- **L14** `cli/src/job.rs:266`, `#[serde(alias = "entry_style")]`. Job files are
  the CLI's own format; the ruling does not cover them.
- **L15** `gcode/post.rs:230` gives a post TOML that omits `tool_change` the
  pre-template `M5`/`M6` pair. A separate format. Pinned at `post.rs:577`.

## Classes 3 and 4

Class 3, the domain vocabulary that merely uses the word, is the largest group
and is **not debt**. The counts below come from `rg -c` over all of `crates/`,
test modules included, so they do not sum against the evidence file's 526
production lines. `FinishResolutionMode::LegacyEnvelopeQuarter` and its policy
helper account for 45 hits across `finish_setup.rs`, `scallop.rs`,
`steep_shallow.rs`, `ramp_finish.rs` and `finish_surface_cache.rs`; the name is
a deliberate ANSWERED-BY-NAME choice recorded at `finish_setup.rs:463`.
`deprecated_dial` and `DeprecatedDialFinding` (44 hits) are the *mechanism that
reports* an inert dial, not residue. `CleanupStrategy::Legacy` (10) is a live
strategy variant, selectable in the GUI
(`viz/ui/properties/operations/boundary_2d.rs:327`). `COMPAT_*` ids and the
`*_compatible` predicates (17) are feasibility rules. `LEGACY_ESTIMATE_LABEL`
(8) labels a pre-simulation estimate honestly. Add the `material.rs:1734` SYP
alias, the `legacy_fast_path` ULP oracle in `dexel_stock/stamping.rs`, and
"fallback" as an algorithm branch in `vendor_lookup.rs`. Class 4, comments
recording what was removed, covers roughly 120 lines — the 18 "Migrated to the
registry GenerateFn (T11)" arms in `compute/execute.rs:3578-3633`, the
byte-parity notes in `gcode/emitter.rs` and `program_builder.rs`, and the pre-v3
notes in `unified_finish.rs`. Both classes are fine. Count them and move on.

## NEEDS A RULING

1. **L4** — deleting the `toolpaths` field removes a `toolpaths = []` line from
   every project file this build writes (98 in-repo TOMLs carry it).
2. **L2 deletion arm** — deleting `stock_to_leave_radial` changes the persisted
   file and the MCP param schema, and breaks
   `toolpath_fields_round_trip_c11.rs`. The reporting arm needs no ruling.
3. **L5 / L10 / L11** — retiring a deprecated wire key changes a value that
   external MCP agents and the operator's CLI scripts read.
4. **L9** — retiring a `parse_lenient` alias changes what MCP accepts.
5. **L7** — inverting the absent-provenance default changes a gate verdict.
6. **L8** — deleting the three no-constructor `Command` variants.

## What I would delete first

1. L3 and L4 in one commit: the two pre-v3 readers behind the format refusal.
   Cost S, no behaviour change, one ruling (the `toolpaths = []` line).
2. L2's reporting arm: a `DeprecatedDialFinding` for `stock_to_leave_radial`.
   Cost S, no ruling, and it stops an MCP caller setting a dial that does
   nothing.
3. L1: give the dressup migration the `ProjectLoadWarning` its sibling has.
4. L6: the `machine_ref` chain, one commit, cost M. It is dead end to end and
   it currently makes an MCP reply field permanently null.
5. Leave every wire key (L5, L10, L11, L9) until the operator rules. The
   consumers are outside this repo.
