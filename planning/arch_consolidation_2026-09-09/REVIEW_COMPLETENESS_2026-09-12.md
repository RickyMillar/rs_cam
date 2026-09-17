# Completeness review — arch consolidation 2026-09-09

**HEAD read:** `12358bee` ("docs(status): WP7 DONE — programme implementation
complete"), branch `master`; the tree carries one modified file, `.mcp.json`.
**I ran no cargo.** Every verdict comes from reading the code or from an `rg` /
`git` check I ran. I say "I read", never "verified".
**Verdict: COMPLETE WITH RESIDUALS.** See §6.

---

## 1. §7 definition of done, row by row (as corrected by §23)

### Greps

| # | Check | Target | Measured at HEAD | Verdict |
|---|---|---|---|---|
| 1 | `rg "\.(stock_mut\|machine_mut\|tools_mut\|models_mut\|post_mut\|wizard_mut\|setups_mut\|find_setup_by_id_mut\|find_toolpath_config_by_id_mut\|toolpath_configs_mut\|insert_result)\("` over `crates/rs_cam_viz/src crates/rs_cam_cli/src` | 0 | **0** (rg exit 1) | MET |
| 2 | `rg -c "^\s*pub fn [a-z_0-9]*_mut" crates/rs_cam_core/src/session/mod.rs` | 0 | **0**; also 0 over `session/*.rs` | MET |
| 3 | `rg "MutationKind" crates/rs_cam_viz/src` | 0 | **5**, all `app/mcp.rs` | **NOT MET** |
| 4 | `rg "compute_stale_set" crates/` | 0 | **non-zero**; one production call, `crates/rs_cam_viz/src/app/mcp.rs:2216`; declaration `crates/rs_cam_core/src/session/compute.rs:54` | **NOT MET** |
| 5 | `rg -- "-> Result<ResolvedGenInputs" crates/rs_cam_core/src` (§23 r4) | 1 | **1** — `session/compute.rs:1993` | MET |
| 6 | `rg "pub struct ResolvedHeights" crates/rs_cam_core/src` (§23 r4: 2) | 2 | **2** — `compute/config.rs:1656`, `diagnostics/adapters/from_static_checks.rs:58` | MET |
| 7 | No `#[allow(too_many_arguments)]` on a **pub** generation entry (§23 r4) | 0 | **0**. `execute.rs` carries 5 allows; the three generation entries at `:3358`, `:3406`, `:3498` all read `pub(crate) fn`. `apply_dressups` (`:3980`) is `pub` but is a dressup pass, not a generation entry | MET |
| 8 | `rg "session/compute.rs" crates/rs_cam_viz/src/compute`, comments excluded (§23 r4) | 0 | **0** occurrences of any kind | MET |
| 9 | `rg "AppEvent::RemoveSetup" crates/rs_cam_viz/src` | 0 or ≥1 emitter | **1**, a comment at `controller/tests.rs:1732`. The variant is deleted | MET |

Rows 3 and 4 are **NOT MET at HEAD**. §23 corrected four §7 rows, not these
two, so `IMPLEMENTATION_PLAN.md:674-675` still states them as 0. STATUS WP4
(`:244`) and §5 WP4 record the holdouts honestly; the checkpoint
(`STATUS.md:231-233`) says "greps at 0", which is false. See M1.

### Named sentries (§7 list)

| §7 name | State at HEAD |
|---|---|
| core `command_registry_completeness.rs` | PRESENT |
| core `resolved_gen_inputs_has_one_producer.rs` | PRESENT |
| viz `command_registry_surfaces.rs` | PRESENT |
| viz `mcp_wire_surface_pin.rs` | PRESENT |
| viz `adopt_result_rejects_stale_completion.rs` | Lives in **core** (§12 ruling 5 moved it). MET, relocated |
| viz `inspector_door_is_one_command.rs` | ABSENT. §17 WP5 ruling 6 replaced it with core `replace_toolpath_config_gates_on_the_signature.rs`. MET by substitute |
| viz `inspector_emits_one_command_per_change.rs` | ABSENT. Replaced by the in-crate `an_open_panel_that_edits_nothing_drops_no_result_wp5`. MET by substitute |
| viz `cycle_time_query_one_answer.rs` | ABSENT. Core `query_cycle_time_one_answer.rs` (§13 ruling 5 names it). MET, renamed |
| viz `job_steps_hold_no_session_borrow.rs` | ABSENT. Core `job_three_steps_equal_generate_toolpath.rs::execute_job_holds_no_session` (§16 ruling 8). MET by substitute |
| viz `command_surface_completeness.rs` | PRESENT, 14 arms |

No property in the list is unsentried. Five of the ten names drifted. §7 was
never rewritten to match; a reader who greps §7's names finds four missing files.

### Full heavy gate
**NOT MEASURABLE by me** (I run no cargo) and **NOT RUN** by the programme.
Every WP row states "No heavy gate: operator ruling 2026-09-11"; §14 and §23
ruling 6 waive it. §7 demands 3801+/1/288. WP12 moved 20 tests in-crate and
deleted four integration files with no rerun. A residual, not a blocker.

### `CommandId::ALL`, kinds, `Surfaces` literals

- The registry is one X-macro at `session/command.rs:98-527`. I counted
  **45 rows** (43 `Command`, 2 `Query`, 1 `Job`); muncher arms at `:1458`,
  `:1475`, `:1492`.
- `CommandKind` (`:535-546`) carries **five** variants: `Command`, `Query`,
  `Job`, `UiCommand`, `UiQuery`. §21 ruling 2 asked for the fifth. MET.
- `Surfaces` is a struct with three fields (`:562-569`), so an omitted surface
  fails to compile. `Reach::Skip` carries `&'static str` (`:554`).
- `rg 'Skip("")'` over both registries returns **0**. Every `Skip` carries a
  reason; `every_skipped_surface_carries_a_reason`
  (`command_registry_completeness.rs:117`) pins it, and
  `all_holds_one_entry_per_declared_row` (`:140`) pins the `ALL` union.
- The view registry (`crates/rs_cam_viz/src/ui_command.rs`) reads 62
  `UiCommand` + 8 `UiQuery` = **70 rows** (72 matches minus 2 muncher arms at
  `:960`, `:975`). It agrees with the STATUS WP13 row.

**Ten `Reached` rows sampled, constructor sought by `rg`:**

| Row | Surface | Constructor found |
|---|---|---|
| `SetPostConfig` | cli | `crates/rs_cam_cli/src` names `Command::SetPostConfig` |
| `SetStockConfig` | cli | `Command::SetStockConfig` in CLI |
| `SetMachineKinematics` | cli | `Command::SetMachineKinematics` in CLI |
| `SetToolpathEnabled` | cli | `Command::SetToolpathEnabled` in CLI |
| `ReplaceToolpathConfig` | cli | `Command::ReplaceToolpathConfig` in CLI |
| `SetToolpathParam` | cli | `Command::SetToolpathParam` in CLI |
| `AdoptSimulation` | gui | `controller/events/compute.rs:783` |
| `SetToolpathDebugOptions` | gui | `controller/events/mod.rs:63` |
| `AdoptModelGeometry` | gui | `controller/io.rs:58` |
| `GenerateToolpath` | gui | `controller/events/compute.rs:305` |

The CLI constructs exactly the six `Command::*` rows that declare
`cli: Reached`. The seventh cli-Reached row is the `Job` `GenerateToolpath`,
which the CLI reaches through `generate_toolpath` running the three steps
inline (§16 ruling 5), never by naming the row. No sentry pins that.

**One row no surface reaches:** `SetSetupName` (`command.rs:182-193`) declares
all three surfaces `Skip`, and its gui reason reads *"the GUI calls the session
setter directly; WP6 adopts this row"*. WP6 is DONE and did not adopt it:
`crates/rs_cam_viz/src/controller/events/model.rs:453` still calls
`self.state.session.rename_setup(idx, name)`. See M2.

## 2. Rulings §12–§23

| § | Ruling | State | Evidence |
|---|---|---|---|
| §12 | 1 `Effects.revision: Option<u64>`; 3 `#[must_use]` | APPLIED | `command.rs:762-770`, `:745` |
| §12 | 2 one construction site (combinator) | APPLIED | `pub(crate) fn with_effects` `command.rs:1898`; every producer wraps its body in it |
| §12 | 4 `AdoptResult` requires the revision; 5 refusal sentry in core; 6, 7 | APPLIED | row `:106`; `core/tests/adopt_result_rejects_stale_completion.rs` |
| §13 | WP11a triple (no `Default`, no `pub` field/setter, one producer) | APPLIED | `session/compute.rs:202` — no derive above the struct, every field private, the only `impl` fn is the private `fn spatial_index(&self)`; producer grep = 1 |
| §13 | WP9 1-6 (`Query` kind, answer column, row 1, five viz sites, sentry) | APPLIED | row `:442`; `query_cycle_time_one_answer.rs` |
| §14 | 1 `OptimizeToolpath` / `RecommendClearingStrategy` / `PreviewTierMap` become `Job` | **NOT APPLIED** | all three remain bare `McpRequestKind` arms: `app/mcp.rs:397`, `:276`, `:425`. See finding M3 |
| §14 | 2 fifth kind `UiQuery` | APPLIED | `command.rs:545` |
| §14 | 3 five post-write events deleted; `WizardSetPost` → `SetPostConfig` | APPLIED | WP6 sentry `the_app_event_enum_dropped_the_five_post_write_events` |
| §14 | 4 export events are `Query` rows | **NOT APPLIED** | no export `Query` row in the registry; `McpRequestKind::ExportGcode` stands at `app/mcp.rs:371`. §15 ruling 4 names `ExportGcode` a holdout, so this is recorded |
| §15 | 1 core `*Args` per row; 3 `Effects.created`; 5-10 | APPLIED | 45 payload structs; `command.rs:796` |
| §15 | 2 viz describe step keyed by `CommandId` | APPLIED | `describe_core` at `app/mcp/commands.rs:1735`, exhaustive `match id` at `:1745`; sentry `mcp_core_arm_describes_every_row.rs` |
| §15 | 4 three holdouts + `load_project` | APPLIED as written | `app/mcp.rs:357` `LoadProject`, `:371` `ExportGcode`, `:421` `PlanMultitoolFinishing`, `:429` `ApplyFeeds` |
| §16 | 1-8 (`start` / `execute_job` / `AdoptResult`, core-only) | APPLIED | `pub fn execute_job` `session/compute.rs:611`; `pub fn start` `command.rs:1846` |
| §17 | WP8 1-5, WP5 1-7; §18 Q5 builder | APPLIED | `RestoreToolpathSnapshot` `:453`, `ReplaceToolpathConfig` `:462`, `generation_inputs_signature` in core; `session/builder.rs:70-142`, ten `pub fn` |
| §19 | 1-10 (incl. `WizardState` to viz, `Surfaces` flip with the caller) | APPLIED, one gap | `GuiState.wizard`; `wizard_mut` deleted. Ruling 6's `SetSetupName` row exists but no caller adopted it (finding M2) |
| §20 | 1-5 (WP7a builder; nine hatches) | APPLIED | `hatches_are_crate_private_wp7.rs:79-89` lists nine names; both greps 0 |
| §21 | 1-10 (view registry, `UiQuery`, `GetOperationSchema` a core `Query`, `RemoveSetup` deleted) | APPLIED | `GetOperationSchema` row `command.rs:501`; view registry 70 rows |
| §22 | 1-9 (`GenObserver`, lazy index, per-submit cancel, `execute_generation`, core wins) | APPLIED | `pub struct GenObserver<'a>` `session/compute.rs:495`; `pub fn execute_generation` `:1035` |
| §23 | 1-6 (loose entry `pub(crate)`, tests in-crate, advisor residual, §7 rows, sentry, no heavy gate) | APPLIED | `execute.rs:3358/3406/3498` all `pub(crate) fn`; `loose_executor_is_crate_private_wp12.rs` |
| §23 add. | `AdoptSimulation` row + two GUI adoption sites | **PARTLY APPLIED** | The row exists (`command.rs:519`) and `apply` runs at `controller/events/compute.rs:783`. I found **one** adoption site, not the two the addendum names. The single site sits after the modulation pass and shares the trace by `Arc`, which is what the ruling wanted. Intent MET; the "two sites" wording is stale |

### The named deviations, judged against intent

| Deviation | Intent satisfied? | Why |
|---|---|---|
| `CoreRequest` wrapper on `McpRequestKind`, not `Command` | **YES** | One dispatch arm, `app/mcp.rs:335`, which calls `apply`. The conversions run on the GUI thread because twenty read the session. One producer still holds. |
| `GuiState.wizard`, not `AppState` | **YES** | The ruling asked that `WizardState` leave core. It did; `wizard_mut` is deleted and the hatch count fell to nine. Which viz struct owns it does not touch the hatch. |
| `RsCamApp::ui_query`, not `AppState::ui_query` | **YES** | `app/mcp.rs:645` takes `&self`, so no view read can write. Two of the eight reads need the toast stack, which `AppState` does not hold. |
| `AdoptSimulation` args carry the whole result | **YES** | The ruling asked for one simulation state. A whole-result payload keeps the session and the viewport on one `Arc`. |
| `Effects.created` index-vs-id split | **YES** | One field, `command.rs:796`, whose doc (`:788-795`) states the split per row: `AddTool` / `AddSetup` report an INDEX, `AddModel` an ID, because a model is named by id everywhere else. `add_model` (`mutation.rs:752-759`) writes `add_model_impl`'s id. The doc says "Read the row before reading the number". One channel, documented asymmetry. |
| `execute_operation` retained | **YES (as a residual)** | `pub(crate)`, zero production callers, `#[cfg_attr(not(test), allow(dead_code))]` at `execute.rs:3357`. It cannot be reached from outside core, so "no hatch" holds. §23 addendum records it. |

## 3. The seven `PLAN.md` properties (`:495-504`) and their sentries

| Property | Sentry file and test | New? |
|---|---|---|
| One generation post-step implementation | `core/tests/gen_inputs_one_assembly_n12.rs` — `one_core_function_returns_the_resolved_bundle`, `the_narrowed_executor_reads_the_resolved_bundle`, `no_viz_source_names_the_loose_executor`; `resolved_gen_inputs_has_one_producer.rs` (3); `loose_executor_is_crate_private_wp12.rs` (3) | new |
| One export phase-builder change | `viz/tests/export_parity_core_vs_gui_p0.rs`; `core/tests/export_honors_coolant_p0d1.rs`, `export_disabled_cached_n1.rs` | pre-programme |
| One geometry-loading implementation per import format | `core/tests/model_units_survive_reload_g_unitsreload.rs`, `step_project_load.rs::both_doors_apply_a_step_models_declared_units` | pre-programme; `adopt_model_geometry_invalidates_bound_toolpaths.rs` (4) folds the third door in |
| A supported parameter cannot ignore writes | `core/tests/set_param_refuses_absent_field_n5.rs` (5) | pre-programme |
| Mutation callers cannot forget invalidation | `core/tests/mutation_paths_invalidate_alike_p0.rs` (8), `restore_snapshot_invalidates_like_the_setter_n14.rs` (4), `replace_toolpath_config_gates_on_the_signature.rs` (4), `adopt_model_geometry_invalidates_bound_toolpaths.rs` (4) | mostly new |
| Timing aggregations use one clock | `core/tests/air_cut_one_time_base_g_airdenom.rs`, `drill_runtime_survives_retime_n2.rs`; `query_cycle_time_one_answer.rs` (6) pins one answer | new arm |
| Stale display geometry cannot become output | `core/tests/adopt_result_rejects_stale_completion.rs` (5), `viz/tests/feeds_apply_drops_result_n13.rs`, `core/tests/export_disabled_cached_n1.rs` | new arm |

**Every property carries a sentry.** Two of the seven rest entirely on
pre-programme sentries. The §4 WP6 ask for a frame-time measurement
(`PLAN.md` rule 5) has no sentry; the STATUS WP6 row says so ("NOT PRESENT").

## 4. Residuals and holdouts

| Residual | State at HEAD | Recorded honestly? |
|---|---|---|
| MCP holdouts `ApplyFeeds`, `PlanMultitoolFinishing`, `ExportGcode` | Hand-written arms at `app/mcp.rs:429`, `:421`, `:371` | YES — §15 ruling 4, §5 WP4, STATUS WP4 |
| `load_project` keeps its own arm | `app/mcp.rs:357` | YES — STATUS WP4 |
| `compute_stale_set` / `MutationKind` / `StaleSet` retained | Declaration `session/compute.rs:54`, re-export `session/mod.rs:43`, one production caller `app/mcp.rs:2216` | YES in §5/STATUS WP4 and the N15 row; **NO** in the checkpoint at `STATUS.md:231-233` and in §7 rows `:674-675` |
| Strategy advisor's loose call | `session/compute.rs:1716` calls `pub(crate) execute_operation_annotated` | YES — §23 ruling 3, STATUS WP12 |
| `execute_operation` retained, 0 production callers | `execute.rs:3358`, dead-code allow | YES — §23 addendum, STATUS WP12 |
| `GenerateToolpath` `gui: Reached` exemption | `P1_EXEMPT` at `command_surface_completeness.rs:181` still lists it. Its stated removal condition (WP11b switches worker and drain) is DISCHARGED: viz constructs the row at `controller/events/compute.rs:305` | **NO** — the exemption is dead and nothing records that. Finding N1 |
| `CloseOptimize*` `mem::replace` | `P3_NAMED_EXEMPTIONS` at `:342`; the STATUS WP13 row says both were MEASURED to pass unaided | YES |
| `OptimizeToolpath` / `RecommendClearingStrategy` / `PreviewTierMap` not `Job` rows | Three bare `McpRequestKind` arms | **PARTLY** — §5 WP10 records "optimize keeps its own submit". Nothing records the two slow reads, and nothing records §14 ruling 1 as deferred. Finding M3 |
| Full heavy gate not run | Waived | YES — every WP row and §23 ruling 6 |

## 5. Drift risk — a new mutation can still bypass the registry

The adopted ruling states the bar: *"`apply(Command) -> Effects` must be the
only write path"* (`RULING_ONE_COMMAND_SURFACE_DRAFT.md:125`). Its own
containment list names six hatches (`:137-140`); §1 raised that to eleven and
§20 settled on nine. The programme closed the nine and left the setters.

`rg -n "pub fn [a-z_0-9]*\(\s*&mut self" crates/rs_cam_core/src/session/*.rs`
returns **27**; `session/mutation.rs` alone declares **56 `pub fn`**. Only two
are registry doors: `apply` (`command.rs:1574`) and `start` (`:1846`). Every
other mutating setter stays `pub` and needs no row.

Production viz and CLI code calls those setters **directly**. A non-test scan
finds some thirty sites in eight viz files and three in the CLI, among them:

- `crates/rs_cam_viz/src/controller/events/undo.rs:14,21,43,68,75,97` —
  `set_stock_config`, `set_post_config`, `set_machine`. All three have rows.
- `crates/rs_cam_viz/src/controller/events/mod.rs:139` —
  `set_toolpath_enabled`. It has a row.
- `crates/rs_cam_viz/src/controller/events/mod.rs:176` —
  `set_face_selection` (`mutation.rs:1081`). **No row exists.**
- `crates/rs_cam_viz/src/controller/io.rs:699` —
  `replace_setups_and_toolpaths` (`mutation.rs:2067`). **No row exists.**
- `crates/rs_cam_viz/src/controller/events/model.rs:453` — `rename_setup`,
  while `SetSetupName` sits unreached.
- `crates/rs_cam_viz/src/controller/events/model.rs:516,535,572,591` —
  `add_fixture`, `remove_fixture`, `add_keep_out`, `remove_keep_out`. The
  registry carries `ReplaceFixture` / `ReplaceKeepOut`, not these.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:643` — `set_post_config` inside
  a draw file, above its `#[cfg(test)]` at `:4079`, beside WP6's sites.
- `crates/rs_cam_cli/src/job.rs:530`, `run.rs:88`, `smoke.rs:567` —
  `add_model`, `add_toolpath`.

**Is that a gap in the sentries? Yes.** All three scan sentries match hatch
NAMES only: `hatches_are_crate_private_wp7.rs:79-89` lists the nine `*_mut`
names; `egui_draw_sites_write_through_commands_wp6.rs:47-52` lists four;
`non_egui_sites_write_through_commands_wp6b.rs:67-78` lists ten. None scans
for `session.set_*(` / `session.add_*(`. A new `pub fn set_foo(&mut self) ->
Effects` on `ProjectSession`, called from a draw site, passes every sentry.
The WP7 commit subject reads "every surface mutates through
`ProjectSession::apply`". At HEAD that holds for the nine hatches and not for
the setters. Each caller binds `let _ =`, so it drops the `Effects` and stamps
no `stale_since`. I did not run the GUI, so I do not claim a stale card.

## 6. Findings, ranked

**BLOCKER** — none. No §7 row that changes behaviour is unmet, and no ruling
is unapplied with a wrong-cut consequence.
**MAJOR**

- **M1 — the checkpoint claims greps at 0; two are not.**
  `planning/arch_consolidation_2026-09-09/STATUS.md:231-233`.
  `MutationKind` reads 5 and `compute_stale_set` has a live production caller
  at `crates/rs_cam_viz/src/app/mcp.rs:2216`. §23 corrected four §7 rows and
  not these two, so `IMPLEMENTATION_PLAN.md:674-675` still demands 0.
  *Smallest fix:* amend the checkpoint sentence to "seven of nine greps at 0;
  the `MutationKind` / `compute_stale_set` rows stay open on the
  `add_toolpath_via_gui` and `apply_feeds` holdouts", and add the same
  condition to the §7 rows.

- **M2 — `SetSetupName` is a row no surface reaches, and its own reason names
  a landed WP.** `crates/rs_cam_core/src/session/command.rs:182-193` says
  "the GUI calls the session setter directly; WP6 adopts this row"; WP6 is
  DONE and `crates/rs_cam_viz/src/controller/events/model.rs:453` still calls
  `rename_setup` directly. *Smallest fix:* one line at `model.rs:453` calling
  `Command::SetSetupName`, then flip `gui` to `Reached`.

- **M3 — §14 ruling 1 is not applied and not recorded as deferred.**
  `OptimizeToolpath`, `RecommendClearingStrategy` and `PreviewTierMap` stay
  bare `McpRequestKind` arms (`crates/rs_cam_viz/src/app/mcp.rs:397`, `:276`,
  `:425`). §5 WP10 records the optimizer only. The two slow reads still block
  the frame loop for 8–60 s, which is the operator-visible consequence.
  *Smallest fix:* one §5 line naming all three and the WP that takes them.

- **M4 — the public setters are an open door no sentry watches.** The ruling
  demands `apply` be "the only write path"
  (`RULING_ONE_COMMAND_SURFACE_DRAFT.md:125`); §5 above lists the bypasses.
  Two of them (`set_face_selection`, `replace_setups_and_toolpaths`) have no
  row at all. *Smallest fix:* extend
  `non_egui_sites_write_through_commands_wp6b.rs` with a fourth arm scanning
  for `session.<setter>(` over the `pub fn` list in `session/mutation.rs`,
  with a named exemption list; then record the residual in §5.

- **M5 — the full heavy gate is not run at HEAD.** WP12 moved 20 tests
  in-crate and deleted four integration files without one; the waiver is
  explicit (§23 ruling 6). *Smallest fix:* run the capped gate once
  (JOBS=2, test-threads=2) before the merge call.

**MINOR**

- **N1 — `P1_EXEMPT` is stale.**
  `crates/rs_cam_viz/tests/command_surface_completeness.rs:181` exempts
  `GenerateToolpath`; viz constructs it at
  `crates/rs_cam_viz/src/controller/events/compute.rs:305`. The arm skips a
  row it would now likely pass; I did not run it. *Fix:* delete the constant
  and its guard at `:189`.
- **N2 — §7's sentry list names four files that do not exist** (the two
  `inspector_*`, `cycle_time_query_one_answer.rs`,
  `job_steps_hold_no_session_borrow.rs`) and puts
  `adopt_result_rejects_stale_completion.rs` under viz, where it is a core
  test. Each property has a renamed or substituted file. *Fix:* rewrite the
  §7 list with the names on disk.
- **N3 — the §23 addendum says "the two GUI adoption sites call it"**; I found
  one, `controller/events/compute.rs:783`. *Fix:* correct the wording.
- **N4 — `Effects.created` means an index on two rows and an id on a third.**
  The field doc states the split (`command.rs:788-795`) and `controller/io.rs`
  reads the `AddModel` id correctly. Documented, not a defect; a reader who
  skips the doc still gets it wrong.
- **N5 — no sentry pins the CLI's `GenerateToolpath` reach.** The row declares
  `cli: Reached` and the CLI never names it; it calls `generate_toolpath`,
  which runs the three steps inline. The claim is true and unpinned.

### Verdict

**COMPLETE WITH RESIDUALS.** The structural goal stands at HEAD. One registry
carries 45 core rows and 70 view rows over five kinds. `Surfaces` is a struct,
and every `Skip` states a reason. `Effects` is `#[must_use]` and has one
construction site. `ResolvedGenInputs` is public with private fields and one
producer. The three generation entries are crate-private. All nine `*_mut`
hatches left viz and the CLI; both hatch greps read 0. Every one of the seven
`PLAN.md` properties carries a sentry.

The residuals are real and bounded. Two §7 greps stay open on holdouts the
working documents name (M1). One registry row has no caller, although its own
reason says a landed package adopts it (M2). One operator ruling is unapplied
and unrecorded (M3). The heavy gate is waived, not run (M5). M4 is the one I
would act on first: the sentries guard the hatch names, not the setters, so
the ruling's "only write path" holds for the hatches alone while some thirty
production sites write the session directly. None of the five emits wrong
motion, so none is a blocker. All five belong in §5 before anyone calls the
programme closed.
