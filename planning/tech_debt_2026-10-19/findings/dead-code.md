# Dead code, unused dependencies and legacy residue (`dead-code`)

Baseline: 99b6fb41, branch `geo-select`. Read-only review. Every claim below
comes from `rg` and file reads in this session. Caller counts come from `rg`,
not from the graph tools.

Ranking: tier first, then cost. `in_flux` means that a path is in
`instruments/in_flux_paths.txt`.

---

## DC-1. Test instruments and test-only wrappers compile into the product library

- Tier C, cost M, in_flux: yes (`lib.rs`, `polygon.rs`).
- Enforcement now: NONE. Proposed: TYPES (`#[cfg(any(test, feature = "test-support"))]`).

**Claim.** A set of `pub` items has no production caller. Only the
integration tests and benches call them. The items are not behind a gate, so
every product build compiles them, and a reader sees them as product API.

**Sites.**

- `crates/rs_cam_core/src/lib.rs:51` `pub mod metrology;` — 2663 lines in
  `metrology/`. `rg -l metrology crates` outside `src/metrology/` gives only
  `tests/*.rs`, `finish/scallop/research.rs` (doc), and four doc comments
  (`geo.rs:377`, `dressup/mod.rs:1122`, `operation_configs.rs:1630`,
  `rs_cam_mcp/src/server.rs:1268`). No production code reads it.
- `crates/rs_cam_core/src/adaptive3d/mod.rs:488,499,536,618,634` — the
  `adaptive_3d_toolpath*` wrapper chain. Production calls only
  `adaptive_3d_toolpath_output_with_cancel` (`:571`). `adaptive_3d_toolpath`
  has 39 test callers and 0 production callers. `:618` and `:634` have no
  caller outside this chain.
- `crates/rs_cam_core/src/dressup/entry_audit.rs:142,189` —
  `OutsideRegionChord`, `fed_moves_outside_region`, and `buried_fed_chords`.
  Only `tests/disconnected_finish_retract_structure_p0.rs`,
  `tests/unified_finish_partial_clip_finding_c8.rs` call them.
- `crates/rs_cam_core/src/polygon.rs:166,931,984` — `with_holes_closed`,
  `cleanup_collinear`, `simplify_bounded`. Their own docs say "No production
  path calls this".
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:159-167` —
  `dexel_stock_to_top_surface_mesh`, a "Test door" by its own doc.
- `crates/rs_cam_core/src/maps/reach_map.rs:1854` — `reach_map_for_mesh`
  (only `rs_cam_viz/tests/reach_overlay_p5.rs` and core tests).
- `instruments/test_only_pub_api.md` — 44 rows. I spot-checked 6
  (`generate_all_without_peer`, `request_compute`, `reach_map_for_mesh`,
  `chipload_envelope_for_toolpath`, `with_holes_closed`,
  `predicted_gate_observation_mm`). All 6 are test-only.

**Evidence that the fix exists.** The repo already uses the gate: 31
`feature = "test-support"` sites, the sentry `the_test_doors_are_gated_fld0405`,
and `#[cfg(test)]` shims in `gcode/mod.rs:203,1339`. The items above did not
get that treatment.

**Fix.** Gate each item with `#[cfg(any(test, feature = "test-support"))]`,
or move `metrology/` behind the `research` or `test-support` feature and add a
`required-features` row to each harness that reads it. A product build then
cannot name the item, and the compiler shows each real caller.

---

## DC-2. `pub` items with zero references anywhere

- Tier C, cost S, in_flux: partly (`state/shoe_plan.rs`).
- Enforcement now: NONE. Proposed: TYPES (delete).
- Known: S1 hold (`TECH_DEBT_PLAN.md:53`) for `COLLISION_POINT` and `SPACE_0`.

**Claim.** These items have no reference outside their definition line. `rg -w`
over `crates/` gives one hit each.

**Sites (verified one by one).**

- `crates/rs_cam_core/src/budget/guard.rs:147` `BudgetGuard::shared_flag`
- `crates/rs_cam_core/src/budget/mod.rs:88` `MemoryBudget::from_system_fraction`
- `crates/rs_cam_core/src/machine/head.rs:334` `MachineHead::holder_diameter_mm`
- `crates/rs_cam_core/src/stock/dexel.rs:285` `ray_has_added_material`
- `crates/rs_cam_viz/src/state/shoe_plan.rs:204` `resolved_text`
- `crates/rs_cam_viz/src/ui/components/line.rs:45` `OneLine::detail_opt`
- `crates/rs_cam_viz/src/ui/tokens.rs:757` `accent_wash`
- `crates/rs_cam_viz/src/render/colors.rs:44` `COLLISION_POINT`
- `crates/rs_cam_viz/src/ui/tokens.rs:36` `SPACE_0`
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:856`
  `resolve_operation_invariants` — one re-export (`feeds/suggest.rs:26`), one
  test (`feeds/suggest/tests.rs:649`), doc mentions only. The optimizer that
  called it was culled on 2026-10-13.

**What changed since 2026-09-16.** The S1 hold said that the ui-premium plan
names `COLLISION_POINT` and `SPACE_0`. UP8 landed (`16df6dcd`). The UP8 sentry
`crates/rs_cam_viz/tests/viewport_palette_up8.rs` that was to read
`COLLISION_POINT` does not exist. The hold has no owner now.
`render/CLAUDE.md:14` still names `COLLISION_POINT` as a live constant.

**Instrument false positives (not filed).** `derive_op_params` and
`derive_param_enum` (`rs_cam_derive/src/lib.rs:46,55`) are proc-macro entry
points. `ray_top_material` has a same-file caller (`stock/dexel.rs:732`).

**Fix.** Delete each item. Remove the `COLLISION_POINT` line from
`render/CLAUDE.md`.

---

## DC-3. Enum variants that no code constructs

- Tier C, cost S, in_flux: no.
- Enforcement now: NONE. Proposed: TYPES (delete the variant; every match arm fails to compile).

**Claim.** A `pub` enum in a library crate hides a dead variant from the
`dead_code` lint. Three enums carry variants that nothing builds. Their match
arms and doc lines read as live behaviour.

**Sites.**

- `crates/rs_cam_viz/src/error.rs:9-21` — `VizError::{StlImport, SvgImport,
  DxfImport, StepImport, ProjectSave}`. `rg "VizError::"` shows only `Export`,
  `Other` and `ProjectLoad` (through `From<SessionError>`) built.
  `crates/rs_cam_viz/src/io/import.rs:14-16` maps every import error to
  `Other`, so the four `#[from]` impls and the `user_message` arms at
  `error.rs:38-45` never run. Two mappings of one `SessionError` also exist:
  `From<SessionError>` gives `ProjectLoad`, `core_err_to_viz` gives `Other`.
- `crates/rs_cam_core/src/export/gcode_validator.rs:58-67` —
  `FindingKind::WrongProgramEndCode` has no constructor.
  `MissingProgramBrackets` is built only behind
  `PostInvariants::requires_percent_brackets` (`:124`), which is `false` on
  all four posts (`:158,169,180,195`), so `rule_missing_program_brackets`
  (`:400-435`) always returns at `:406`. The doc at `:62` says "kept for
  downstream matches / serialized findings", but `Finding` (`:101`) derives
  only `Debug, Clone`: nothing serialises it. The module doc at `:19-22`
  still lists both rules as active.
- `crates/rs_cam_core/src/diagnostics/mod.rs:106-123` —
  `Scope::Setup { id }` and `Scope::MoveRange { .. }`. No code constructs
  either. One reader matches `MoveRange` (`stock/sim_triage.rs:445`).
  `Scope` is `Serialize`, so the wire schema advertises two shapes that never
  occur.

**Fix.** Delete the variants, the dead `From` impls and `user_message` arms,
`requires_percent_brackets`, the two validator rules and their module-doc
lines. Use `From<SessionError>` in `io/import.rs` or delete one of the two
mappings.

---

## DC-4. `chipload_envelope_for_toolpath` is documented as a production door, but only a sentry calls it

- Tier C, cost S, in_flux: no.
- Enforcement now: SENTRY (it tests the wrapper, not the consumer). Proposed: TYPES.

**Claim.** `crates/rs_cam_core/src/tool_load/mod.rs:253-275` says the strategy
advisor reads this function. No production code calls it. The modulator and
the session read `chip_target_for_toolpath` directly
(`session/compute.rs:1696,2044`, `tool_load/mod.rs:226`). The only caller is
`tests/every_consumer_reads_one_claimed_band_g1.rs:237`. Its header (`:14-16`)
calls the function "the modulator's and the advisor's door". So the G1 sentry
proves a property of a door that no consumer uses.

**Fix.** Delete the wrapper. Point the G1 arm at `chip_target_for_toolpath`
followed by `band_range()`, which is the real consumer path. Correct the
sentry header.

---

## DC-5. `ui/theme.rs` alias layer outlived its migration

- Tier D, cost M, in_flux: no.
- Enforcement now: DOC ("Do not add a name here"). Proposed: TYPES (delete the module).

**Claim.** `crates/rs_cam_viz/src/ui/theme.rs:1-10` says the module "exists to
keep the old call sites compiling while UP2 to UP8 migrate them, and it shrinks
as they do". UP2 to UP8 all landed (`76faed3d` .. `16df6dcd`). The module still
has 22 names and 221 `theme::` uses in 24 files. It keeps several names for one
token: `WARNING`, `WARNING_MILD`, `WARNING_TEXT` all equal `tokens::CAUTION`
(`:27-29`); `ERROR` and `ERROR_MILD` equal `tokens::DANGER`; `SUCCESS` and
`SUCCESS_BRIGHT` equal `tokens::OK`. Six names (`ERROR_MILD`, `UNKNOWN`,
`LANE_IDLE`, `LANE_QUEUED`, `LANE_RUNNING`, `LANE_CANCELLING`) have no
production reader. Only `tests/panels_read_the_token_module_up1.rs:392-407`
reads them. A new contributor copies `theme::WARNING_MILD` and believes it is a
different tone.

**Fix.** Rewrite each `theme::X` use to its `tokens::` name (mechanical, 24
files), then delete `theme.rs`. The compiler finds every missed site.

---

## DC-6. Legacy read paths survive the 2026-09-16 "no legacy" ruling

- Tier E, cost S, in_flux: yes (`gcode/mod.rs`).
- Enforcement now: NONE. Proposed: TYPES (delete; the old spelling then fails to parse).

**Claim.** Four read-side compatibility shims remain in production code.

**Sites.**

- `crates/rs_cam_core/src/gcode/mod.rs:1426,1428` — the only two production
  `#[serde(alias)]` attributes in the workspace: `grbl_hal`, `linux_cnc` on
  `PostFormat`. `PostFormat::from_token` (`:1491-1492`) accepts the same old
  spellings. Tests pin them: `gcode/mod.rs:1914,1916,1951`,
  `rs_cam_viz/src/state/runtime.rs:423-424`.
- `crates/rs_cam_core/src/gcode/mod.rs:106-112` — `emit_program`, "re-export
  for backward-compatible call sites". `rg "emit_program\("` gives no caller
  of `gcode::emit_program`; every caller uses `emitter::emit_program`.
- `crates/rs_cam_core/src/material/mod.rs:1704-1711` — the
  `southern_yellow_pine` alias (see DC-7).
- `crates/rs_cam_core/src/gcode/post.rs:308-311` — `default_tool_change`, the
  "backward-compat default" for a post TOML with no `tool_change`. This is
  L15 of the 2026-09-16 plan, deferred below its cut line; unchanged since.

**Fix.** Delete the aliases and the extra `from_token` arms, and update the
tests that pin them. Delete the `gcode::emit_program` re-export. Decide L15
with the operator (it touches third-party post TOMLs).

---

## DC-7. `Material::to_key` / `from_key`: 250 lines with no production caller

- Tier C, cost S, in_flux: no.
- Enforcement now: NONE. Proposed: TYPES (delete; the test shim moves to serde).

**Claim.** `crates/rs_cam_core/src/material/mod.rs:1585` says "Serialization
key for TOML project files". That is false. `Material` derives `Serialize` and
`Deserialize` (`:549`), and project IO uses serde. `rg "Material::from_key|\.to_key\(\)"`
gives only the file's own tests (`:2260,2269,2333-2334`) and the test shim
`crates/rs_cam_core/tests/literature_matrix/shim.rs:101`. `from_key` also
returns softwood for an unknown key (`:1691`, `:1829`), and the shim comment
(`shim.rs:102-110`) shows that it cannot detect that silent default. The legacy
alias at `:1704-1711` goes against the operator ruling.

**Fix.** Delete both functions and their tests. Rewrite the shim to parse with
serde, so that an unknown material is an error and not softwood.

---

## DC-8. The `pub` surface regrows after each sweep; no gate holds the line

- Tier E, cost M, in_flux: no.
- Enforcement now: CONVENTION (a periodic `debt_scan.py` run). Proposed: SENTRY (ratchet).
- Known: T-1 (the pattern; T-1 itself is closed), S29 of the 2026-09-16 plan.

**Claim.** In a library crate, `pub` hides an item from `dead_code`. The
2026-09-16 programme demoted 127 items and deleted 64 (`TECH_DEBT_PLAN.md`
W2/W4). One month later, `instruments/dead_pub_surface.md` again lists 11
zero-reference items and 141 own-file-only items, and
`instruments/test_only_pub_api.md` lists 44 test-only items. I checked the
first 40 own-file rows: each has a same-file caller (count 2 or more), so
those are visibility debt, not dead code. The zero-reference rows are DC-2.
`rs_cam_viz` is a library that only its own binary and its tests consume
(`src/lib.rs`, `src/bin/main.rs`), so every `pub` there is invisible to the
lint. Nothing in `Cargo.toml` lints this (no `unreachable_pub`).

**Fix.** Turn the zero-reference and test-only counts of `debt_scan.py` into
a sentry with a ratchet (a count may fall, never rise), with an allowlist file
for intentional test doors. For `rs_cam_viz`, demote modules to `pub(crate)`
where no external test reads them, so `dead_code` sees the items.

---

## DC-9. Two optional STEP dependencies have no `use`

- Tier F, cost S, in_flux: no.
- Enforcement now: NONE. Proposed: LINT (`cargo machete` in the local loop).

**Claim.** `crates/rs_cam_core/Cargo.toml:61-62` declares `truck-geometry` and
`truck-polymesh`, and the `step` feature (`:11`) enables them.
`rg "truck_geometry|truck_polymesh" crates` gives no hit.
`io/step_input.rs:13-14,89` uses only `truck_meshalgo`, `truck_stepio` and
`truck_topology`. `instruments/cargo_machete.txt` reports the same two crates.
The geometry-selection code does not use them either (`rg truck crates/rs_cam_core/src/selection`
gives nothing).

**Fix.** Remove both from `[dependencies]` and from the `step` feature list.
Verify with `cargo check -p rs_cam_core --features step` (not run here).

---

## DC-10 (selection programme). `outlines_pre_selection` outlived its planned deletion

- Tier C, cost S, in_flux: yes. Selection programme: yes.
- Enforcement now: DOC. Proposed: TYPES (delete after P2.x).

**Claim.** `crates/rs_cam_core/src/selection/item.rs:638-642` says "P2.3
deletes this". P2.3 is done (`cf9b7ff3` reports P2.6). The method stays.
`crates/rs_cam_core/src/session/mod.rs:550-555` says "REFERENCE ONLY (P2.5): no
production code reads it", but production code computes it:
`ItemSet::scaled` (`item.rs:564`) and `with_circles` (`item.rs:603`) carry the
memo through every scale and SVG import. `LoadedModel::outlines_pre_selection`
is `pub`, and its only readers are about 12 test files.

**Note for the programme.** Decide whether the frozen pre-P2.1 list is still a
needed oracle. If not, delete the memo field, the two production carries and
the `pub` door, and move the oracle into `tests/common/`.

---

## Not filed (checked and rejected)

- `CleanupStrategy::Legacy` (`adaptive/mod.rs:82`): the GUI offers it
  (`boundary_2d.rs:393`) and adaptive3d branches on it. A live mode with an
  old name.
- `ClassificationSampler` candidate arms (`finish/classify_probe.rs:103-133`):
  on the W5 schema as a documented escape hatch (`operation_configs.rs:1998-2005`).
- `ReferenceEngagement`, `enforce_unmodeled_policy`, `MoveKinematics`:
  instrument false positives with production callers.
- `viz_project_evidence` (map lead): five production callers.
- `scallop/research.rs`: its name says research, but
  `scallop_toolpath_iso_field_with_cancel` is production (`finish_raster.rs:119`).
- W4 residue items (`semantic_runtime_metrics`, `RadialProfileLUT` re-export):
  already gone.
- `MoveIntent::Unknown` doc claim: production `feed_to`/`rapid_to` calls outside
  test modules are only kinematics link estimates, NC replay and a legend
  sample, not generators.
- `pre_gcode` / `post_gcode`: export reads them (`gcode/mod.rs:447`) and
  `ui/properties/mod.rs:781` writes them. The comment at
  `ui/properties/toolpath_panel.rs:1185` ("UI removed until export is
  implemented") is stale: export is implemented.
