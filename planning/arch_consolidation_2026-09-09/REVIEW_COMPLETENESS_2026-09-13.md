INCOMPLETE — 1 blocker.

# Completeness review — architecture consolidation

## A. Scope, method, and definition of done

**Pin and method.** This is a source-and-history audit at
`61c16b75c68facf24a806619666486b9cea0a320`; HEAD later advanced and is not
used for conclusions.  Every source path and line below is from that pin.
Historical evidence was read with `git show` at the named sentry/fix commits;
for each ordinary pair below I checked `git merge-base --is-ancestor SENTRY FIX`
and `git merge-base --is-ancestor FIX PIN`, then checked that the named file is
present at the pin and read its asserted contract.  **RESULT is source/history
only, not a runtime result.** No Cargo command, test binary, build, or runtime
injection was executed; those are **NOT MEASURED**. Historical green-gate
claims belong to the recorded STATUS, not to this audit.

### §7 definition of done — source evidence

| Criterion | RESULT and source anchor |
|---|---|
| Hatch producer calls outside core | PASS (source scan scope): no production hatch call in viz/CLI under the corrected hatch pattern. The compiled-privacy sentry is `crates/rs_cam_core/tests/hatches_are_crate_private_wp7.rs:242` (declarations) and `:288` (outside-core names). This does **not** substitute for a build here. |
| Hatches are not public | PASS (source): the same sentry asserts each retained hatch is `pub(crate)` or absent; no `pub fn *_mut` remains in the session scan. “No public hatch” does not claim every mutating property is covered. |
| `MutationKind` / `compute_stale_set` deletion | **NOT DONE, ledgered/excluded (WP19)**: producer helper `crates/rs_cam_core/src/session/compute.rs:41-87`; MCP consumer `crates/rs_cam_viz/src/app/mcp.rs:2236`; remaining producers `:2870,3341`. Do not report these greps as zero. |
| One `ResolvedGenInputs` producer | PASS (source): `crates/rs_cam_core/src/session/compute.rs:2523`; sentry assertion `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs:221`. |
| Two `ResolvedHeights` types understood | PASS (source): configuration type `crates/rs_cam_core/src/compute/config.rs:1656`; diagnostic adapter type `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:58`. This is deliberately two, not a failed one-type count. |
| Loose generation entries not public | PASS (source): `execute_operation` `crates/rs_cam_core/src/compute/execute.rs:3358`, annotated `:3406`, regions `:3498`, all crate-private; sentry `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs:222`. |
| No viz compute mirror | PASS only for corrected source scope: `session/compute.rs` under `crates/rs_cam_viz/src/compute`, comments excluded, is zero. Sentry/allowlist check: `loose_executor_is_crate_private_wp12.rs:322`. |
| `AppEvent::RemoveSetup` | PASS (source): variant deleted. The only hit is a test comment at `crates/rs_cam_viz/src/controller/tests.rs:1736`, not a path producer. |
| Runtime gates | **NOT MEASURED.** No current normal suite, heavy suite, build, or historical gate was re-run. |

**Residuals deliberately not promoted to blockers.** The four accepted handwritten
MCP residuals are `app/mcp.rs:369` (load), `:383` (export), `:433`
(multitool), and `:441` (apply feeds). The advisor loose call is
`core/session/compute.rs:1874`; accepted ignored-Effects wrappers are
`events/mod.rs:59-66`, `ForgetResult` `compute.rs:184`, debug MCP `:3407`, and
init IO `:645,751`. The `mem::replace` sites are `events/mod.rs:626,1326` and
`planner.rs:176`. These are ledgered/excluded scope, not evidence that a
runtime property holds.

### Named-sentry mapping (the ten requested old names)

The plan’s old filenames are not all files at the pin. “Exists” alone is not
coverage: in particular WP13’s B1 constructor census has a known view-
constructor hole. This maps every requested name to the actual pin source and
an assertion anchor.

| Old requested sentry name | Actual pin file and assertion anchor | Source-only RESULT |
|---|---|---|
| `command_registry_completeness.rs` | `crates/rs_cam_core/tests/command_registry_completeness.rs:80` `every_wire_name_is_unique_and_snake_case`; row identity `:152`, query identity `:186`, answer variants `:214`, Effects parity `:384-487` | present; contracts read |
| `resolved_gen_inputs_has_one_producer.rs` | `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs:167` privacy, `:221` sole producer, `:261` external nameability | present; contracts read |
| `command_registry_surfaces.rs` | `crates/rs_cam_viz/tests/command_registry_surfaces.rs:48` MCP-reached tool; `:76` positive scan; `:93` skip reason | present; contracts read |
| `mcp_wire_surface_pin.rs` | `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs:185` snapshot equality; `:263` every MCP-reached core row | present; contracts read |
| `adopt_result_rejects_stale_completion.rs` (old viz location) | **relocated:** `crates/rs_cam_core/tests/adopt_result_rejects_stale_completion.rs:231` stale refusal; `:283,:321,:343,:390` independent-edit/absent/producer/enable contracts | present at core, not viz |
| `inspector_door_is_one_command.rs` | requested file absent; replacement evidence is `crates/rs_cam_core/tests/replace_toolpath_config_gates_on_the_signature.rs:228,:287,:335,:391` | substitute, not a rename claim |
| `inspector_emits_one_command_per_change.rs` | requested file absent; actual in-crate evidence `crates/rs_cam_viz/src/controller/tests.rs` test `an_open_panel_that_edits_nothing_drops_no_result_wp5` | substitute; source contract only |
| `cycle_time_query_one_answer.rs` (old viz location) | **renamed/relocated:** `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs:279,:289,:298,:306,:314,:328` | present at core, not viz |
| `job_steps_hold_no_session_borrow.rs` | requested file absent; replacement `crates/rs_cam_core/tests/job_three_steps_equal_generate_toolpath.rs:164,:222,:276,:312` | substitute, not a rename claim |
| `command_surface_completeness.rs` | `crates/rs_cam_viz/tests/command_surface_completeness.rs:182` GUI reach, `:208` skip, `:260` reason, `:299` view reach, `:398` no direct write, `:497-547` MCP map | present; B1 limitation remains |

### P0 contracts (eight actual tests; flip status is source/history, not assumed)

| P0 file | Main asserted contract at pin | Flip status recorded/read |
|---|---|---|
| `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` | `:415` setter/replacement drop `{0,1}`; `:435` drop equals revision bump; `:460` undo; `:484,:507` drill chains; `:531` `Effects.stale`; `:577` retained narrow divergence | flips WP1/WP5/WP8; source shows post-flip contract |
| `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs` | `:173` absent field refuses; `:239` refusal preserves result/provenance; `:390` named numeric arms use range helper | recorded unchanged through WP1 |
| `crates/rs_cam_core/tests/disconnected_finish_retract_structure_p0.rs` | `:256` disconnected islands retract/link safely | no flip claimed here |
| `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` | `:444` retimed total includes drill; `:478` published runtimes agree; `:496` drill runtime real | recorded unchanged; WP9 target is query, not this contract |
| `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` | `:488` byte-identical agreeing properties; `:601` coolant equality | recorded unchanged |
| `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` | `:298` cached result; `:329` downstream chain; `:346` export stale; `:377,:409` project/MCP routes | recorded unchanged |
| `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs` | `:258,:300,:344,:376,:401,:427` identical diagnostic-ID scenarios | no unverified flip inference |
| `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs` | `:407` geometry with optimisation off; `:428` shipped-default feed identity | test 2 recorded flipped to identity at WP11b; source is post-flip |

### Corrected §7 grep ledger (nine source anchors, not “nine zeroes”)

1. Hatch calls in viz/CLI production scope: zero under the named hatch-call pattern.
2. `pub fn *_mut` in session sources: zero (the historical `session/mod.rs` scan is too narrow; scan session `*.rs`).
3. `MutationKind` in viz: **five**, all `app/mcp.rs`; excluded WP19, not zero.
4. `compute_stale_set` workspace: non-zero; declaration `core/session/compute.rs:54`, live MCP `app/mcp.rs:2238`; excluded WP19, not zero.
5. `-> Result<ResolvedGenInputs` in core: one, `session/compute.rs:2523` (not the old broad return grep).
6. `pub struct ResolvedHeights` in core: two, `compute/config.rs:1656` and `diagnostics/adapters/from_static_checks.rs:58`.
7. Public generation entries with `#[allow(too_many_arguments)]`: zero; five allow sites remain, but the three generation entries are `pub(crate)` and public `apply_dressups` is not a generation entry.
8. `session/compute.rs` only under viz `src/compute`, comment lines excluded: zero (do not scan all viz source or comments).
9. `AppEvent::RemoveSetup` in viz: one literal in `controller/tests.rs:1736`, comment only; producer/variant scope is zero.

## B. Work-package evidence and historical red/green record

**Counting and evidence rule.** The tracker has 25 physical rows: 21 fully
DONE rows, two partial rows (WP14/WP15), and two TODO rows (WP19/WP22).
WP11b is one physical row with two completed fix/sentry pairs. The 22 ordinary sentry→fix pairs below all pass both
ancestor checks described in §A; the sentry file exists at the pin and its
assertion was read. “Recorded red” means the named historical commit/STATUS
record contains the red observation, not that this audit re-executed it.

In the table, `core/` means `crates/rs_cam_core/` and `viz/` means
`crates/rs_cam_viz/`.

`S→F; A/P; file:line; red-kind; status` means: sentry then fix ordering checked
(`A`), fix ancestor of pin (`P`), actual pin assertion anchor, recorded red
kind, and tracker status. “compile” and “assertion” are historical categories.

| Unit | Evidence / main pin assertion / recorded red / status |
|---|---|
| WP1 | `255f6fe3→90c8e90e; A/P; core/tests/mutation_paths_invalidate_alike_p0.rs:531; compile; DONE` |
| WP2a | `8f4faf43→ecee1ee5; A/P; viz/tests/mcp_wire_surface_pin.rs:185; assertion (missing snapshot); DONE` |
| WP3 | `0d22acaf→25f37b74; A/P; core/tests/adopt_result_rejects_stale_completion.rs:231; compile (14 errors); DONE` |
| WP4 | `4dfde9cc→1c399a71; A/P; core/tests/mcp_mutation_rows_reach_core.rs:231; compile (9 errors); DONE`. Additional viz `viz/tests/mcp_core_arm_describes_every_row.rs:77` was introduced by the **fix**; it is not falsely described as red-first evidence. |
| WP5 | `973834f7→caf8fbe6; A/P; core/tests/replace_toolpath_config_gates_on_the_signature.rs:228; compile; DONE` |
| WP6 | `fd883efa→9b887707; A/P; viz/tests/egui_draw_sites_write_through_commands_wp6.rs:167; assertion (compiled, 1 pass/4 fail); DONE`. Per-widget commits are historical follow-ons, not replacements for this pair; `4a49b434` is the tool widget (verified from git log), alongside stock `a86f2df1`, setup `97e4eaf1`, properties `9b887707`. |
| WP6b | `b84f2601→2cfc2b9a; A/P; viz/tests/non_egui_sites_write_through_commands_wp6b.rs:206; assertion plus core compile red; DONE` |
| WP7 | `72e912bf→7dff635b; A/P; core/tests/hatches_are_crate_private_wp7.rs:242; assertion (9 public/17 external names); DONE` |
| WP7a | `76dbfb41→084b9b50; A/P; core/tests/session_builder_preserves_ids.rs:168; compile (missing builder); DONE` |
| WP8 | `5c32ced7→e079e192; A/P; core/tests/restore_snapshot_invalidates_like_the_setter_n14.rs:223; compile; DONE` |
| WP9 | `720626b2→5173c923; A/P; core/tests/query_cycle_time_one_answer.rs:279; compile; DONE` |
| WP10 | `b952e6ee→3ff3cffa; A/P; core/tests/job_three_steps_equal_generate_toolpath.rs:164; compile; DONE` |
| WP11a | `181f5c13→a9ef73d6; A/P; core/tests/resolved_gen_inputs_has_one_producer.rs:167; compile; DONE` |
| WP11b.1 | `df963567→4b53576b; A/P; core/tests/gen_inputs_one_assembly_n12.rs:165; compile plus assertion (28 feed mismatches); DONE` |
| WP11b.2 | `bab065ce→276e5c13; A/P; core/tests/adopt_simulation_stores_prior_stocks.rs:177; compile plus viz assertion; DONE` |
| WP12 | `89f81cb4→77627c33; A/P; core/tests/loose_executor_is_crate_private_wp12.rs:182; assertion (arms a/b); DONE` |
| WP13 | `50f975e2→35ae77ad; A/P; viz/tests/command_surface_completeness.rs:182; compile (13 errors); DONE`. It proves static declaration/reach contracts, not correct handler behavior; B1’s constructor hole remains. |
| WP14a | `5a43a365→6e9a1326; A/P; core/tests/job_rows_recommend_and_preview_wp14a.rs:181; compile (14 errors); PARTIAL` (WP14b in flight/excluded). |
| WP15a | `a4c0abd7→ad06c690; A/P; core/tests/setters_have_rows_wp15a.rs:251 and viz/tests/production_writes_go_through_apply_wp15a.rs:456; assertion/source-scan red; PARTIAL` (WP15b in flight/excluded). The core sentry enumerates **24** row-less setters at `:43-52`; a claimed 25th is the `set_toolpath_param` wrapper exemption (`:89-92,:301-322`), not a 25th missing setter. |
| WP16 | `ff81b712→183900e2; A/P; viz/tests/command_surface_completeness.rs:182; assertion (“set_setup_name” absent constructor); DONE`. The older §19.3 same-commit phrasing is a separate historical caller/diff issue, not an A/B ancestor failure. |
| WP17 | `91bc6c39→525fe504; A/P; core/tests/save_keeps_the_simulation_wp17.rs:237; assertion; DONE`. The historical header `RED-OUTPUT` is a valid label without a colon. |
| WP18 | **not an original red→fix pair:** `3654826a` is independent injection repair; `514fd0fc` is coverage work. Actual sources: `core/tests/loose_executor_is_crate_private_wp12.rs:322` (injection-sensitive source scan) and `core/tests/feed_optimization_refusals_wp18.rs:279` (coverage). No invented pair/ancestor claim; DONE as recorded. |
| WP19 | no sentry/fix pair at pin; TODO, explicitly excluded WP19. |
| WP20 | `9b3f75de` prose pass; no new sentry. Existing `viz/tests/command_surface_completeness.rs` covers the exemption deletion; prose/toast work is not misreported as a new red pair; DONE. |
| WP21 | `10c2ad37→4aeaeda3; A/P; core/tests/feedopt_clamp_never_panics_wp21.rs:163; runtime panic red, message anchored in historical status as `min > max, or either was NaN. min = 6000.0, max = 3000.0`; DONE. |
| WP22 | no sentry/fix pair at pin; TODO, explicitly excluded WP22. |

### Historical red-block qualification

There are **seven literal `RED`-style historical headings/labels** in the
recorded material: WP1, WP5, WP6, WP6b, WP7, WP9, and WP11a. They are metadata
labels, not seven extra tests and not runtime evidence from this audit. WP17’s
parenthetical `RED-OUTPUT` label is also valid historical evidence despite not
being a literal `RED:` heading. Do not invent eight missing blocks merely to
meet a literal-heading count: compile-red and assertion-red records above are
real failed-block evidence under other labels. The additional WP4 viz sentry,
WP18 injection/coverage evidence, and WP20 prose/existing-test exception are
recorded separately precisely so none becomes a false blocking pair.

## C. Each binding ruling at the pinned tree

Each PASS below is a source or history check, not a test run. Evidence names the relevant production body or sentry source. Later amendments supersede transitional signatures and ownership rules. In-flight outcomes are excluded, not passed.

| Ruling | Requirement | Result | Pinned evidence |
|---|---|---|---|
| §12.1 | Optional surviving same-index revision | PASS | `crates/rs_cam_core/src/session/command.rs:1096` — `pub revision: Option<u64>,`; `crates/rs_cam_core/src/session/command.rs:2646` — `revision: index` |
| §12.2 | One Effects construction site and shared combinator | PASS | `crates/rs_cam_core/src/session/command.rs:2635` — `pub(crate) fn try_with_effects<E>(`; `crates/rs_cam_core/src/session/command.rs:2657` — `pub(crate) fn with_effects(` |
| §12.3 | Effects is must-use | PASS | `crates/rs_cam_core/src/session/command.rs:1067` — `#[must_use]` |
| §12.4 | Required revision and typed stale-adopt refusal | PASS | `crates/rs_cam_core/src/session/command.rs:2265` — `if current != revision {`; `crates/rs_cam_core/src/session/command.rs:888` — `/// [`SessionError::StaleCompletion`], inserting nothing.` |
| §12.5 | Core stale-adopt sentry; GUI drain adoption | PASS: sentry source; not executed | `crates/rs_cam_core/tests/adopt_result_rejects_stale_completion.rs:54` — `fn tc(` |
| §12.6 | Sim-only mutations also return Effects | PASS | `crates/rs_cam_core/src/session/mutation.rs:1477` — `pub fn invalidate_machine(&mut self) -> Effects {`; `crates/rs_cam_core/src/session/command.rs:1083` — `pub simulation_cleared: bool,` |
| §12.7 | Two writers, one worktree, sentry then fix | NOT MEASURED: process | Source and commit ancestry do not establish writer count or worktree discipline. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §12.7. |
| §13 correction | Public nameable input bundle; private construction | PASS | `crates/rs_cam_core/src/session/compute.rs:202` — `pub struct ResolvedGenInputs {`; `crates/rs_cam_core/src/session/compute.rs:2519` — `pub fn resolve_generation_inputs(` |
| §13.WP9.1 | One registry list and CommandId union | PASS | `crates/rs_cam_core/src/session/command.rs:121` — `macro_rules! for_each_command {`; `crates/rs_cam_core/src/session/command.rs:2153` — `pub const ALL: &[CommandId] = &[$(CommandId::$id,)+];` |
| §13.WP9.2 | Sixth answer column and query door | PASS | `crates/rs_cam_core/src/session/command.rs:2085` — `(Command, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),`; `crates/rs_cam_core/src/session/command.rs:2523` — `pub fn query(&self, query: Query) -> Result<QueryAnswer, SessionError> {` |
| §13.WP9.3 | Cycle-time Query; explicit surfaces | PASS | `crates/rs_cam_core/src/session/command.rs:442` — `(Query, ToolpathCycleTime, "toolpath_cycle_time", ToolpathCycleTimeArgs,` |
| §13.WP9.4 | Five viz consumers use the core answer | PASS | `crates/rs_cam_viz/src/ui/readiness.rs:390` — `pub fn toolpath_cycle_time(`; `crates/rs_cam_viz/src/ui/readiness.rs:400` — `let query = Query::ToolpathCycleTime(ToolpathCycleTimeArgs {` |
| §13.WP9.5 | Frozen-oracle cycle-time and registry sentry | PASS: sentry source; not executed | `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs:67` — `fn oracle_toolpath_cycle_time(` |
| §13.WP9.6 | WP3 before WP9 | PASS: commit order | Checked `25f37b74` precedes `5173c923` at the pin. This establishes landing order, not when editing began. |
| §14.1 | Recommend/Preview/Optimize are Jobs | PARTIAL; Optimize is excluded WP14b | `crates/rs_cam_core/src/session/command.rs:804` — `(Job, RecommendClearingStrategy, "recommend_clearing_strategy",`; `crates/rs_cam_core/src/session/command.rs:816` — `(Job, PreviewTierMap, "preview_tier_map", PreviewTierMapArgs,` |
| §14.2 | Viz-owned reads use UiQuery | PASS | `crates/rs_cam_core/src/session/command.rs:843` — `UiQuery,`; `crates/rs_cam_viz/src/ui_command.rs:913` — `pub enum UiQuery {` |
| §14.3 | Delete five post-write events; wizard takes SetPostConfig | PASS | `crates/rs_cam_viz/src/app/input.rs:111` — `let command = rs_cam_core::session::Command::SetPostConfig(`; `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs:271` — `fn the_app_event_enum_dropped_the_five_post_write_events() {` |
| §14.4 | Export artifacts use Query; viz writes files | NOT DONE: ledgered export holdout | `crates/rs_cam_viz/src/app/mcp.rs:383` — `McpRequestKind::ExportGcode {`; `crates/rs_cam_viz/src/ui_command.rs:352` — `(UiCommand, ExportGcode, "open_export_preflight", NoArgs, (),` |
| §15.1 | Core Args payloads; wire conversion stays in viz | PASS | `crates/rs_cam_core/src/session/command.rs:1285` — `pub struct MoveToolpathToSetupArgs {`; `crates/rs_cam_viz/src/app/mcp/commands.rs:493` — `Command::MoveToolpathToSetup(MoveToolpathToSetupArgs {` |
| §15.2 | Describe by CommandId after apply | PASS | `crates/rs_cam_viz/src/app/mcp/commands.rs:1818` — `pub(crate) fn describe_core(`; `crates/rs_cam_viz/src/app/mcp.rs:359` — `let described = self.describe_core(id, outcome, &before);` |
| §15.3 | Created identity in Effects for add rows | PASS | `crates/rs_cam_core/src/session/command.rs:1118` — `pub created: Option<usize>,`; `crates/rs_cam_core/src/session/mutation.rs:140` — `effects.created = created;` |
| §15.4 | Three handwritten holdouts remain | PASS: stated exception; LoadProject is a fourth ledger item | `crates/rs_cam_viz/src/app/mcp.rs:433` — `McpRequestKind::PlanMultitoolFinishing { spec } => {`; `crates/rs_cam_viz/src/app/mcp.rs:441` — `McpRequestKind::ApplyFeeds { index, scope } => {` |
| §15.5 | Move setup replies with actual mutation Effects | PASS | `crates/rs_cam_viz/src/app/mcp/commands.rs:493` — `Command::MoveToolpathToSetup(MoveToolpathToSetupArgs {`; `crates/rs_cam_viz/src/app/mcp/commands.rs:1745` — `&#124; CommandId::MoveToolpathToSetup` |
| §15.6 | Separate kinematics and import rows | PASS | `crates/rs_cam_core/src/session/command.rs:373` — `(Command, SetMachineKinematics, "set_machine_kinematics",`; `crates/rs_cam_core/src/session/command.rs:380` — `(Command, ImportMachineSettings, "import_machine_settings",` |
| §15.7 | Tool setters/removal return Effects | PASS | `crates/rs_cam_core/src/session/compute.rs:1532` — `pub fn set_tool_param(`; `crates/rs_cam_core/src/session/mutation.rs:863` — `pub fn remove_tool(&mut self, index: usize) -> Result<Effects, SessionError> {` |
| §15.8 | Post-write effects move to describe | PASS for post-write effects; pre-display work is separate | `crates/rs_cam_viz/src/app/mcp/commands.rs:1818` — `pub(crate) fn describe_core(`; `crates/rs_cam_viz/src/app/mcp/commands.rs:194` — `pub(crate) fn mcp_before_core(&mut self, request: &CoreRequest) {` |
| §15.9 | Retarget the toast sentry | PASS: sentry source; not executed | `crates/rs_cam_viz/tests/mcp_toasts_report_outcome_g_mcptoast.rs:94` — `fn every_command_row_toast_is_returned_not_pushed() {` |
| §15.10 | Two writers and one fix commit | NOT MEASURED: process | Source and commit ancestry do not establish writer count or worktree discipline. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §15.10. |
| §16.1 | WP9 before WP10 | PASS: commit order | Checked `5173c923` precedes `3ff3cffa` at the pin.  |
| §16.2 | start captures inputs/context and revision | PASS as amended by §16 addendum | `crates/rs_cam_core/src/session/command.rs:2597` — `pub fn start(&mut self, job: Job, cancel: &AtomicBool) -> Result<JobHandle, SessionError> {`; `crates/rs_cam_core/src/session/compute.rs:3015` — `pub(crate) fn start_generate_toolpath(` |
| §16.3 | Session-free executor | PASS as amended by §22 observer parameter | `crates/rs_cam_core/src/session/compute.rs:611` — `pub fn execute_job(` |
| §16.4 | Adopt uses handle revision | PASS | `crates/rs_cam_core/src/session/compute.rs:3250` — `revision: handle.revision,` |
| §16.5 | Core and CLI use the inline three steps | PASS | `crates/rs_cam_core/src/session/compute.rs:3227` — `pub fn generate_toolpath(`; `crates/rs_cam_core/src/session/compute.rs:3248` — `let _ = self.apply(Command::AdoptResult(AdoptResultArgs {` |
| §16.6 | generate_all keeps its control loop | PASS: specified scope | `crates/rs_cam_core/src/session/compute.rs:3730` — `pub fn generate_all(` |
| §16.7 | Debug options has a Command | PASS | `crates/rs_cam_core/src/session/command.rs:302` — `(Command, SetToolpathDebugOptions, "set_toolpath_debug_options",` |
| §16.8 | WP10 core-only; WP11b migrates worker assembly | PASS: transitional rule superseded by WP11b | `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:343` — `let outcome = rs_cam_core::session::execute_job(&req.handle, &observer, cancel);` |
| §16 addendum | Cancellation reaches start; start does not bump revision | PASS: capture body | `crates/rs_cam_core/src/session/compute.rs:3015` — `pub(crate) fn start_generate_toolpath(`; `crates/rs_cam_core/src/session/compute.rs:3205` — `revision: self.toolpath_revision(index),` |
| §17.WP8.1 | Restore writes and invalidates unconditionally | PASS | `crates/rs_cam_core/src/session/command.rs:2454` — `Command::RestoreToolpathSnapshot(args) => {` |
| §17.WP8.2 | Undo/redo restore and stamp every returned stale index | PASS | `crates/rs_cam_viz/src/controller/events/undo.rs:181` — `let restored = self.state.session.apply(Command::RestoreToolpathSnapshot(`; `crates/rs_cam_viz/src/state/stale.rs:37` — `for id in ids {` |
| §17.WP8.3 | Drill picks invalidate the result chain | PASS | `crates/rs_cam_core/src/session/mutation.rs:1167` — `pub fn set_drill_selected_holes(` |
| §17.WP8.4 | Optimizer retains core-only narrow path | PASS | `crates/rs_cam_core/src/session/mutation.rs:1867` — `pub(crate) fn apply_toolpath_param_snapshot_narrow(` |
| §17.WP8.5 | Retarget P0 invalidation arms | PASS: sentry source; not executed | `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs:136` — `fn tc(` |
| §17.WP5.1 | Replacement writes config; signature gates invalidation | PASS | `crates/rs_cam_core/src/session/command.rs:2486` — `Command::ReplaceToolpathConfig(args) => {`; `crates/rs_cam_core/src/session/mod.rs:892` — `pub fn generation_inputs_signature(&self) -> String {` |
| §17.WP5.2 | Inspector applies; core decides whether inputs changed | PASS | `crates/rs_cam_viz/src/ui/properties/mod.rs:4224` — `pub(crate) fn write_entry_config_to_session(` |
| §17.WP5.3 | Panel stamps Effects.stale | PASS | `crates/rs_cam_viz/src/ui/properties/mod.rs:206` — `crate::state::stale::stamp_stale(state, &effects.stale);`; `crates/rs_cam_viz/src/state/stale.rs:31` — `pub fn stamp_stale(state: &mut AppState, stale: &BTreeSet<usize>) {` |
| §17.WP5.4 | Field-dependent side effects stay in viz | PASS | `crates/rs_cam_viz/src/state/mod.rs:222` — `pub struct PanelSideEffects {` |
| §17.WP5.5 | Projection preserves fields absent from inspector | PASS | `crates/rs_cam_viz/src/ui/properties/mod.rs:4146` — `pub(crate) fn project_entry_onto(` |
| §17.WP5.6 | Core replacement sentry and viz projection checks | PASS: sentry source; not executed | `crates/rs_cam_core/tests/replace_toolpath_config_gates_on_the_signature.rs:67` — `fn tc(` |
| §17.WP5.7 | Feeds funnel uses the core signature | PASS | `crates/rs_cam_core/src/session/mod.rs:892` — `pub fn generation_inputs_signature(&self) -> String {`; `crates/rs_cam_core/src/session/mutation.rs:2076` — `pub fn invalidate_toolpath_inputs(&mut self, index: usize) -> Effects {` |
| §18 | Builder for setup; Commands for live edits | PASS | `crates/rs_cam_core/src/session/builder.rs:55` — `pub struct ProjectSessionBuilder {`; `crates/rs_cam_core/src/session/builder.rs:102` — `pub fn model(mut self, model: LoadedModel) -> Self {` |
| §19.1 | Remaining producers return Effects; adds carry created id | PASS: current command/mutation paths | `crates/rs_cam_core/src/session/command.rs:1118` — `pub created: Option<usize>,`; `crates/rs_cam_core/src/session/mutation.rs:777` — `pub fn add_model(&mut self, model: super::LoadedModel) -> Effects {`; `crates/rs_cam_core/src/session/mutation.rs:126` — `pub fn add_toolpath(` |
| §19.2 | One whole-machine row | PASS | `crates/rs_cam_core/src/session/command.rs:365` — `(Command, SetMachine, "load_machine_from_library", SetMachineArgs, Effects,`; `crates/rs_cam_core/src/session/mutation.rs:1746` — `pub fn set_machine(&mut self, machine: crate::machine::MachineProfile) -> Effects {` |
| §19.3 | Surface flip and caller in the same commit | HISTORICAL EXCEPTION: ff81b712 flips; 183900e2 adds caller | `crates/rs_cam_core/src/session/command.rs:204` — `(Command, SetSetupName, "set_setup_name", SetSetupNameArgs, Effects,`; `crates/rs_cam_viz/src/controller/events/model.rs:476` — `let command = Command::SetSetupName(SetSetupNameArgs { setup_index, name });` |
| §19.4 | Free stale helper over AppState | PASS | `crates/rs_cam_viz/src/state/stale.rs:31` — `pub fn stamp_stale(state: &mut AppState, stale: &BTreeSet<usize>) {` |
| §19.5 | WizardState is viz-owned; wizard hatch is deleted | PASS | `crates/rs_cam_viz/src/state/runtime.rs:330` — `pub wizard: crate::state::wizard::WizardState,` |
| §19.6 | Name/datum/models have distinct invalidation rules | PASS | `crates/rs_cam_core/src/session/command.rs:214` — `(Command, SetSetupDatum, "set_setup_datum", SetSetupDatumArgs, Effects,`; `crates/rs_cam_core/src/session/mutation.rs:1279` — `pub fn set_setup_models(` |
| §19.7 | Stock auto-fit invalidates all results | PASS | `crates/rs_cam_core/src/session/mutation.rs:1631` — `pub fn update_stock_from_bbox(&mut self, bbox: &BoundingBox3) -> Effects {` |
| §19.8 | Drag commits on stop/focus loss; clicks on change | PASS: static guards; runtime NOT MEASURED | `crates/rs_cam_viz/src/ui/properties/mod.rs:445` — `self.committed &#124;= response.drag_stopped() &#124;&#124; response.lost_focus();`; `crates/rs_cam_viz/src/ui/properties/mod.rs:444` — `self.changed &#124;= response.changed();` |
| §19.9 | Builder precedes final hatch migration | PASS: commit order | Checked `084b9b50` precedes `7dff635b` at the pin.  |
| §19.10 | Per-widget landing points | PASS: commit order | Checked `a86f2df1` precedes `97e4eaf1` at the pin. Properties follows in `9b887707`; separate file commits remain in history. |
| §20.1 | Builder preserves supplied IDs/order; no automatic stock fit | PASS: builder methods | `crates/rs_cam_core/src/session/builder.rs:102` — `pub fn model(mut self, model: LoadedModel) -> Self {`; `crates/rs_cam_core/src/session/builder.rs:142` — `pub fn build(mut self) -> ProjectSession {` |
| §20.2 | No SetExportWizard row; use viz state | PASS: registry census and viz owner | `crates/rs_cam_viz/src/state/runtime.rs:330` — `pub wizard: crate::state::wizard::WizardState,` |
| §20.3 | Core un-commanded reproduction stays in-crate; viz one removed | PASS: test-only remaining reproduction | `crates/rs_cam_core/src/session/mutation.rs:3299` — `fn arm_inspector_door() -> Observation {` |
| §20.4 | Remaining hatch users move to commands/builder | PASS: sentry source; not executed | `crates/rs_cam_core/tests/hatches_are_crate_private_wp7.rs:104` — `fn core_root() -> PathBuf {` |
| §20.5 | Final privacy gate follows builder and migration | PASS: commit order | Checked `2cfc2b9a` precedes `7dff635b` at the pin. Primary privacy sentry is `hatches_are_crate_private_wp7`; no build was rerun here. |
| §21.1 | Separate viz registry and one union | PASS | `crates/rs_cam_viz/src/ui_command.rs:345` — `macro_rules! for_each_ui_command {`; `crates/rs_cam_viz/src/ui_command.rs:1053` — `pub enum SurfaceId {` |
| §21.2 | Fifth kind UiQuery | PASS | `crates/rs_cam_core/src/session/command.rs:843` — `UiQuery,` |
| §21.3 | GetOperationSchema is a core Query | PASS | `crates/rs_cam_core/src/session/command.rs:501` — `(Query, GetOperationSchema, "get_operation_schema", GetOperationSchemaArgs,` |
| §21.4 | Read-only UI query door | DEVIATION: RsCamApp, not the ruled AppState owner | `crates/rs_cam_viz/src/app/mcp.rs:657` — `fn ui_query(&self, query: UiQuery) -> UiQueryAnswer {` |
| §21.5 | Library/file-store reads and writes remain view-owned | PASS: scoped per-user-store classification | `crates/rs_cam_viz/src/ui_command.rs:826` — `(UiQuery, ListToolLibrary, "list_tool_library", NoArgs, String,`; `crates/rs_cam_viz/src/ui_command.rs:704` — `(UiCommand, DeleteLibraryTool, "delete_library_tool", DeleteLibraryToolArgs, (),` |
| §21.6 | All UiCommand rows declare GUI Reached and CLI Skip | NOT MET: seven MCP-only commands; see M1 | `crates/rs_cam_viz/src/ui_command.rs:498` — `(UiCommand, SimScrubToolpath, "sim_scrub_toolpath", SimScrubToolpathArgs, (),` |
| §21.7 | MCP/UI event wrappers | PASS | `crates/rs_cam_viz/src/app/mcp.rs:513` — `McpRequestKind::Ui(command) => match command {`; `crates/rs_cam_viz/src/controller/events/mod.rs:383` — `AppEvent::Ui(cmd) => match cmd {` |
| §21.8 | Delete dead AppEvent RemoveSetup | PASS for AppEvent; core Command is a different row | `crates/rs_cam_viz/src/controller/tests.rs:1736` — `// `AppEvent::RemoveSetup`: no control emitted it, no MCP tool named` |
| §21.9 | Constructing-site/non-vacuity and no-write sentries | PARTIAL: literal core scan present; view construction hole is B1 | `crates/rs_cam_viz/tests/command_surface_completeness.rs:182` — `fn every_gui_reached_core_row_is_constructed_in_the_view() {`; `crates/rs_cam_viz/tests/command_surface_completeness.rs:299` — `fn every_view_row_is_reached_by_some_surface() {` |
| §21.10 | Explicit optimizer exemption list and package order | PASS: named record; retirement is excluded WP14b | `crates/rs_cam_viz/tests/command_surface_completeness.rs:339` — `const P3_NAMED_EXEMPTIONS: &[&str] = &["CloseOptimizeModal", "CloseOptimizeProject"];` |
| §22.1 | Observer parameter on session-free executor | PASS for the observer; documented field-placement deviations keep debug options on the handle and artifact writes with the caller | `crates/rs_cam_core/src/session/compute.rs:495` — `pub struct GenObserver<'a> {`; `crates/rs_cam_core/src/session/compute.rs:611` — `pub fn execute_job(` |
| §22.2 | Session memo keyed by model/revision; shared lazy cell forced off-loop | NOT MET literally: ordinary execution forces lazily, but PlannedTierRegions also forces inside start; cells wrap a global weak-identity cache, not the ruled session-owned shared cell | `crates/rs_cam_core/src/session/compute.rs:225` — `spatial_index: Option<Arc<crate::geom_cache::LazyIndex>>,`; `crates/rs_cam_core/src/geom_cache.rs:345` — `pub fn lazy_auto_index(mesh: &Arc<TriangleMesh>) -> Arc<LazyIndex> {`; `crates/rs_cam_core/src/geom_cache.rs:170` — `static TABLE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();`; `crates/rs_cam_core/src/session/compute.rs:2884` — `let tier_index = spatial_index.as_ref().map(&#124;lazy&#124; Arc::clone(lazy.force()));` |
| §22.3 | Fresh per-submit flag stored on core handle | DEVIATION: fresh flag exists, but VizExtras/JobRequest owns it; core start and execute receive a borrowed flag | `crates/rs_cam_viz/src/compute/worker.rs:90` — `pub cancel: Arc<AtomicBool>,`; `crates/rs_cam_viz/src/controller/events/compute.rs:309` — `let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));`; `crates/rs_cam_core/src/session/compute.rs:388` — `pub struct GenerateToolpathHandle {` |
| §22.4 | Narrow session executor; private loose implementation | PASS as corrected by §23 | `crates/rs_cam_core/src/session/compute.rs:1035` — `pub fn execute_generation(`; `crates/rs_cam_core/src/compute/execute.rs:3406` — `pub(crate) fn execute_operation_annotated(` |
| §22.5 | Worker uses core assembly; core wins differences | PASS: shared execution route | `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:343` — `let outcome = rs_cam_core::session::execute_job(&req.handle, &observer, cancel);` |
| §22.6 | Preconditions/refusals occur at start | PASS | `crates/rs_cam_core/src/session/compute.rs:3015` — `pub(crate) fn start_generate_toolpath(` |
| §22.7 | Request carries handle and viz extras | PASS | `crates/rs_cam_viz/src/compute/worker.rs:63` — `pub struct ComputeRequest {` |
| §22.8 | Behavioral N12 check and worker/executor source contract | PASS: sentry source; not executed | `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs:76` — `fn pocket_op() -> OperationConfig {` |
| §22.9 | WP4 precedes WP11b | PASS: commit order | Checked `1c399a71` precedes `4b53576b` at the pin. Observer and lazy-index changes are in the WP11b package. |
| §22 addendum | Adopt prior stocks and shared trace after GUI simulation | PASS: one common GUI drain serves both branches | `crates/rs_cam_core/src/session/command.rs:2276` — `Command::AdoptSimulation(args) => {`; `crates/rs_cam_viz/src/controller/events/compute.rs:801` — `let adopt = Command::AdoptSimulation(args);` |
| §23.1 | Loose generation entries are crate-private | PASS | `crates/rs_cam_core/src/compute/execute.rs:3406` — `pub(crate) fn execute_operation_annotated(`; `crates/rs_cam_core/src/compute/execute.rs:3498` — `pub(crate) fn execute_operation_annotated_with_regions(` |
| §23.2 | Move raw-output tests in-crate; no external loose callers | PASS: sentry source; not executed | `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs:89` — `fn is_comment(line: &str) -> bool {` |
| §23.3 | Advisor loose call remains explicitly ledgered | PRESENT: named residual; Job label alone did not retire it | `crates/rs_cam_core/src/session/compute.rs:1874` — `let result = crate::compute::execute::execute_operation_annotated(` |
| §23.4 | Use corrected producer/type/allowance/mirror criteria | PASS: sentry source; not executed | `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs:89` — `fn is_comment(line: &str) -> bool {` |
| §23.5 | Source-scan declaration privacy and external references | PASS: sentry source; not executed | `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs:89` — `fn is_comment(line: &str) -> bool {` |
| §23.6 | Normal suites/lint replace the per-package heavy precondition | NOT MEASURED: no execution in this review | `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md:1521` records the operator waiver; historical results are not new test runs. |
| §23 addendum | Five allows; no public generation entry; wide wrapper is test-only | PASS: separate generation from dressup entry | `crates/rs_cam_core/src/compute/execute.rs:3358` — `pub(crate) fn execute_operation(`; `crates/rs_cam_core/src/compute/execute.rs:3980` — `pub fn apply_dressups(` |
| §24.1 | Recommendation and preview are no-adopt Jobs | PASS as amended by §26 wire policy | `crates/rs_cam_core/src/session/command.rs:804` — `(Job, RecommendClearingStrategy, "recommend_clearing_strategy",`; `crates/rs_cam_core/src/session/command.rs:816` — `(Job, PreviewTierMap, "preview_tier_map", PreviewTierMapArgs,` |
| §24.2 | Optimize cloned-session Job and clone-cost measurement | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §24.2; no later worktree implementation is assessed. |
| §24.3 | WP14a then WP14b after setter work | EXCLUDED: WP14b close-out order | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §24.3; no later worktree implementation is assessed. |
| §25.1 | Rows and production-setter bypass scan | PASS: sentry source; not executed | `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs:128` — `fn viz_root() -> PathBuf {` |
| §25.2 | Narrow typed setters after builder/test migration | EXCLUDED: WP15b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §25.2; no later worktree implementation is assessed. |
| §25.3 | Post shadows use SetPostConfig; WP17 owns semantics | PASS | `crates/rs_cam_core/src/session/command.rs:389` — `(Command, SetPostConfig, "set_spindle_strategy", SetPostConfigArgs, Effects,`; `crates/rs_cam_core/src/session/mutation.rs:1717` — `pub fn set_post_config(&mut self, post: ProjectPostConfig) -> Effects {` |
| §25.4 | Compute doors belong to later Job work | PRESENT: explicitly ledgered scope | `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md:1619-1622`; generation/simulation convenience methods remain, not a claim that all actions are Commands. |
| §25.5 | WP16 → WP20 → WP14a → WP15a | PASS: commit order | Checked `183900e2` precedes `9b3f75de` at the pin. Also checked `9b3f75de → 6e9a1326 → ad06c690`. |
| §26.1 | No new timeout wire field; pending oneshot waits | PASS: unchanged parameter types; poll API remains deferred | `crates/rs_cam_mcp/src/server.rs:83` — `pub struct IndexParam {`; `crates/rs_cam_mcp/src/server.rs:862` — `pub struct PreviewTierMapParam {` |
| §26.2 | Boxed JobAnswer and handle convention | PASS | `crates/rs_cam_core/src/session/command.rs:2063` — `pub enum JobAnswer {`; `crates/rs_cam_core/src/session/command.rs:2217` — `pub enum JobHandle {` |
| §26.3 | FIFO Job lane; generation status/cancel remains Toolpath-only | PASS for FIFO; stated status/cancel residual remains | `crates/rs_cam_viz/src/compute/worker.rs:605` — `let job_lane = LaneQueue::new(ComputeLane::Job);`; `crates/rs_cam_viz/src/compute/worker.rs:753` — `fn submit_job(&mut self, request: JobRequest) {` |
| §26.4 | Move the planner off its borrowed-session Optimize lane | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §26.4; no later worktree implementation is assessed. |
| §26.5 | Tool resolution precedes mesh check in preview | PASS: capture body | `crates/rs_cam_core/src/session/multitool.rs:588` — `pub(crate) fn capture_preview_tier_map(` |
| §27.1 | Close-out order WP19 → WP22 → WP14b → WP15b | EXCLUDED: named in-flight packages | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §27.1; no later worktree implementation is assessed. |
| §27.2 | One capped closing core dev loop after open packages | NOT MEASURED | `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md:1669-1672`; the verifier lane owns execution. This review ran no Cargo command. |
| §27.3 | Feedopt geometric plunge cap | EXCLUDED: WP22 | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §27.3; no later worktree implementation is assessed. |
| §27.4 | Verify in debug while release GUI runs | NOT MEASURED for the other verifier | `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md:1678-1680`; this review did not build or touch `target/release`. |
| §28.1 | Toolpath edits clear the view simulation | EXCLUDED: WP19 | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.1; no later worktree implementation is assessed. |
| §28.2 | Unify stale stamping and catalog defaults | EXCLUDED: WP19 | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.2; no later worktree implementation is assessed. |
| §28.3 | Discharge simulation invalidation once per frame | EXCLUDED: WP19 | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.3; no later worktree implementation is assessed. |
| §28.4 | Mutable optimize handle | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.4; no later worktree implementation is assessed. |
| §28.5 | Session trace captured at submit; refuse after edit | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.5; no later worktree implementation is assessed. |
| §28.6 | Project optimization clones; remove session replacement | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.6; no later worktree implementation is assessed. |
| §28.7 | Planner submits existing preview Job and flips GUI reach | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.7; no later worktree implementation is assessed. |
| §28.8 | Keep optimizing placeholder until a separate UI ruling | EXCLUDED: WP14b | The user excludes this in-flight outcome. `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §28.8; no later worktree implementation is assessed. |

## D. Full registry construction map

The following inline map is the full 141-row construction census at the pin. Counts: 71 core rows (66 Command, 2 Query, 3 Job) and 70 view rows (62 UiCommand, 8 UiQuery). Surface union: GUI 99 Reached, 42 Skip; MCP 51 Reached, 90 Skip; CLI 11 Reached, 130 Skip. This is not the 78-tool MCP wire count. It is static path evidence, not runtime execution. Two declared GUI reaches lack a production constructor and are the blocker in §E.


### 1. `Command::SetToolpathParam` — `set_toolpath_param`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:126`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Reached
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:541 | CLI reached: crates/rs_cam_cli/src/run.rs:161, crates/rs_cam_cli/src/command.rs:46, crates/rs_cam_cli/src/smoke.rs:603, crates/rs_cam_cli/src/job.rs:662

### 2. `Command::AdoptResult` — `adopt_result`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:134`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: a completion is adopted by the GUI drain, not by a wire tool; CLI=Skip: the CLI generates synchronously and never adopts a completion
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/compute.rs:527

### 3. `Command::AddAlignmentPin` — `add_alignment_pin`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:144`
* **Declared G/M/C:** GUI=Skip: no GUI control calls this setter; the pin placer writes the stock; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:344

### 4. `Command::RemoveAlignmentPin` — `remove_alignment_pin`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:154`
* **Declared G/M/C:** GUI=Skip: no GUI control calls this setter; the pin placer writes the stock; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:355

### 5. `Command::AddModel` — `import_model`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:164`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:83, crates/rs_cam_viz/src/controller/io.rs:98, crates/rs_cam_viz/src/controller/io.rs:113, crates/rs_cam_viz/src/controller/io.rs:135 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:389 | CLI reached: crates/rs_cam_cli/src/run.rs:91, crates/rs_cam_cli/src/job.rs:532

### 6. `Command::AdoptModelGeometry` — `adopt_model_geometry`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:170`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool refreshes a model in place; the wire imports a new one; CLI=Skip: the batch CLI imports each model once and never refreshes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:59

### 7. `Command::AddSetup` — `add_setup`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:180`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:296, crates/rs_cam_viz/src/controller/events/model.rs:362 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:396

### 8. `Command::SetSetupFace` — `set_setup_face`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:188`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:258 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:430

### 9. `Command::SetSetupRotation` — `set_setup_rotation`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:196`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:264 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:464

### 10. `Command::SetSetupName` — `set_setup_name`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:204`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes this; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:476

### 11. `Command::SetSetupDatum` — `set_setup_datum`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:214`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes this; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:270

### 12. `Command::SetSetupModels` — `set_setup_models`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:224`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes this; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:276

### 13. `Command::SetSetupPauseMessage` — `set_setup_pause_message`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:234`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes this; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:499

### 14. `Command::MoveToolpathToSetup` — `move_toolpath_to_setup`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:245`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/toolpath.rs:356 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:493

### 15. `Command::SaveProject` — `save_project`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:254`
* **Declared G/M/C:** GUI=Skip: the GUI saves through its own controller door; MCP=Reached; CLI=Skip: the batch CLI emits G-code and writes no project file
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:516

### 16. `Command::SetToolParam` — `set_tool_param`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:264`
* **Declared G/M/C:** GUI=Skip: the GUI tool panel commits a whole draft config, not one parameter; MCP=Reached; CLI=Reached
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:555 | CLI reached: crates/rs_cam_cli/src/run.rs:110

### 17. `Command::SetToolpathTool` — `set_toolpath_tool`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:272`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:567

### 18. `Command::SetToolpathModel` — `set_toolpath_model`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:282`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:578

### 19. `Command::SetToolpathHeights` — `set_toolpath_heights`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:292`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:623

### 20. `Command::SetToolpathDebugOptions` — `set_toolpath_debug_options`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:302`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes this; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/mod.rs:99, crates/rs_cam_viz/src/controller/events/compute.rs:1407

### 21. `Command::AddToolpath` — `add_toolpath`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:313`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:818, crates/rs_cam_viz/src/controller/events/toolpath.rs:171, crates/rs_cam_viz/src/controller/events/toolpath.rs:231 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:907 | CLI reached: crates/rs_cam_cli/src/run.rs:127, crates/rs_cam_cli/src/smoke.rs:567, crates/rs_cam_cli/src/job.rs:624

### 22. `Command::RemoveToolpath` — `remove_toolpath`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:319`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:827, crates/rs_cam_viz/src/controller/events/toolpath.rs:370 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:643

### 23. `Command::AddTool` — `add_tool`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:327`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:74, crates/rs_cam_viz/src/controller/events/model.rs:248 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:961 | CLI reached: crates/rs_cam_cli/src/run.rs:102, crates/rs_cam_cli/src/job.rs:558, crates/rs_cam_cli/src/job.rs:575

### 24. `Command::AddToolFromLibrary` — `add_tool_from_library`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:333`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:89 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1033

### 25. `Command::RemoveTool` — `remove_tool`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:341`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:267 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:652

### 26. `Command::SetStockConfig` — `set_stock_config`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:349`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:31, crates/rs_cam_viz/src/ui/properties/mod.rs:201, crates/rs_cam_viz/src/controller/events/model.rs:434, crates/rs_cam_viz/src/controller/events/undo.rs:17, crates/rs_cam_viz/src/controller/events/undo.rs:80 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1154 | CLI reached: crates/rs_cam_cli/src/smoke.rs:686, crates/rs_cam_cli/src/smoke.rs:1139, crates/rs_cam_cli/src/job.rs:548

### 27. `Command::SetStockSource` — `set_stock_source`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:355`
* **Declared G/M/C:** GUI=Skip: the operation panel writes the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:675

### 28. `Command::SetMachine` — `load_machine_from_library`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:365`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:364, crates/rs_cam_viz/src/controller/events/model.rs:196, crates/rs_cam_viz/src/controller/events/undo.rs:52, crates/rs_cam_viz/src/controller/events/undo.rs:115 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:700

### 29. `Command::SetMachineKinematics` — `set_machine_kinematics`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:373`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:386 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1317 | CLI reached: crates/rs_cam_cli/src/project.rs:251

### 30. `Command::ImportMachineSettings` — `import_machine_settings`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:380`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:407 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1374

### 31. `Command::SetPostConfig` — `set_spindle_strategy`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:389`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:360, crates/rs_cam_viz/src/app/input.rs:111, crates/rs_cam_viz/src/ui/properties/mod.rs:646, crates/rs_cam_viz/src/controller/events/mod.rs:324, crates/rs_cam_viz/src/controller/events/undo.rs:27 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:523, crates/rs_cam_viz/src/app/mcp/commands.rs:744 | CLI reached: crates/rs_cam_cli/src/run.rs:176, crates/rs_cam_cli/src/project.rs:266, crates/rs_cam_cli/src/job.rs:686

### 32. `Command::SetBoundaryConfig` — `set_boundary_config`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:395`
* **Declared G/M/C:** GUI=Skip: the boundary picker writes the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1457

### 33. `Command::SetRestAnalysisConfig` — `set_rest_analysis_config`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:405`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:1520

### 34. `Command::SetDressupConfig` — `set_dressup_config`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:416`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:764

### 35. `Command::SetDressupField` — `set_dressup_field`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:426`
* **Declared G/M/C:** GUI=Skip: the GUI inspector replaces the whole config through replace_toolpath_config; MCP=Reached; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:776

### 36. `Command::SetToolpathEnabled` — `set_toolpath_enabled`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:436`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/mod.rs:175 | MCP reached: crates/rs_cam_viz/src/app/mcp/commands.rs:788 | CLI reached: crates/rs_cam_cli/src/smoke.rs:707

### 37. `Query::ToolpathCycleTime` — `toolpath_cycle_time`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:442`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: get_cut_trace and narrate_toolpath report other quantities; CLI=Skip: the CLI project report prints the simulation total, not per-toolpath
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/readiness.rs:400

### 38. `Command::RestoreToolpathSnapshot` — `restore_toolpath_snapshot`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:453`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: undo and redo are GUI actions; MCP has no history; CLI=Skip: the CLI holds no undo history
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/mod.rs:731, crates/rs_cam_viz/src/controller/events/mod.rs:879, crates/rs_cam_viz/src/controller/events/mod.rs:1404, crates/rs_cam_viz/src/controller/events/undo.rs:181

### 39. `Command::ReplaceToolpathConfig` — `replace_toolpath_config`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:462`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: the MCP door edits one named parameter through set_toolpath_param; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:4232, crates/rs_cam_viz/src/controller/events/mod.rs:1080, crates/rs_cam_viz/src/controller/events/mod.rs:1172 | CLI reached: crates/rs_cam_cli/src/project.rs:796

### 40. `Command::ReplaceTool` — `replace_tool`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:471`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: the MCP door edits one named parameter through set_tool_param; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:146, crates/rs_cam_viz/src/controller/events/undo.rs:147

### 41. `Command::ReplaceFixture` — `replace_fixture`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:481`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a fixture; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:304

### 42. `Command::ReplaceKeepOut` — `replace_keep_out`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:491`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a keep-out zone; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:325

### 43. `Query::GetOperationSchema` — `get_operation_schema`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:501`
* **Declared G/M/C:** GUI=Skip: the GUI inspector draws the catalog directly; no panel asks for a schema; MCP=Reached; CLI=Skip: the batch CLI exposes no schema tool
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp.rs:996

### 44. `Command::ReorderToolpath` — `reorder_toolpath`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:517`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool re-orders the plan; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/toolpath.rs:268, crates/rs_cam_viz/src/controller/events/toolpath.rs:289, crates/rs_cam_viz/src/controller/events/toolpath.rs:326

### 45. `Command::RemoveModel` — `remove_model`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:527`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool removes a model; the wire imports one only; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:667

### 46. `Command::SetFaceSelection` — `set_face_selection`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:537`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool picks BREP faces; the wire has no such mutation; CLI=Skip: the batch CLI picks no faces
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/mod.rs:220

### 47. `Command::SetAlignmentPinDrillHoles` — `set_alignment_pin_drill_holes`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:547`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes the pin holes; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:840

### 48. `Command::SetDrillSelectedHoles` — `set_drill_selected_holes`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:558`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool picks drill holes; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/mod.rs:261

### 49. `Command::AddFixture` — `add_fixture`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:569`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a fixture; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:541

### 50. `Command::RemoveFixture` — `remove_fixture`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:579`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a fixture; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:564

### 51. `Command::AddKeepOut` — `add_keep_out`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:589`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a keep-out zone; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:605

### 52. `Command::RemoveKeepOut` — `remove_keep_out`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:599`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool writes a keep-out zone; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/model.rs:628

### 53. `Command::AutoEnableRestAnalysis` — `auto_enable_rest_analysis`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:609`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: the MCP door writes the whole block through set_rest_analysis_config; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:1202

### 54. `Command::ForgetResult` — `forget_result`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:620`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: a refusal is forgotten by the GUI drain, not by a wire tool; CLI=Skip: the CLI generates synchronously and forgets no completion
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/compute.rs:183

### 55. `Command::ReplaceSetupsAndToolpaths` — `replace_setups_and_toolpaths`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:630`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool replaces the whole plan; the wire edits one row at a time; CLI=Skip: the batch CLI builds its session once and replaces nothing
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:751

### 56. `Command::SetProjectName` — `set_project_name`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:641`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no MCP tool renames the project; the wire has no such mutation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/io.rs:645

### 57. `Command::SetToolpathOperation` — `set_toolpath_operation`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:651`
* **Declared G/M/C:** GUI=Skip: no GUI control changes an operation kind in place; MCP=Skip: no MCP tool changes an operation kind; add_toolpath adds a new operation; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 58. `Command::RemoveSetup` — `remove_setup`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:664`
* **Declared G/M/C:** GUI=Skip: no GUI control removes a setup; MCP=Skip: no MCP tool removes a setup; the wire adds one only; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 59. `Command::InvalidateStock` — `invalidate_stock`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:676`
* **Declared G/M/C:** GUI=Skip: no GUI control drops the results alone; set_stock_config writes and drops; MCP=Skip: no MCP tool drops the results alone; set_stock_config writes and drops; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 60. `Command::InvalidateMachine` — `invalidate_machine`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:688`
* **Declared G/M/C:** GUI=Skip: no GUI control drops the simulation alone; set_machine writes and drops; MCP=Skip: no MCP tool drops the simulation alone; set_machine writes and drops; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 61. `Command::InvalidateTool` — `invalidate_tool`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:700`
* **Declared G/M/C:** GUI=Skip: no GUI control drops a tool's results alone; replace_tool writes and drops; MCP=Skip: no MCP tool drops a tool's results alone; set_tool_param writes and drops; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 62. `Command::InvalidateModel` — `invalidate_model`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:712`
* **Declared G/M/C:** GUI=Skip: the three model refresh doors take adopt_model_geometry, which also drops; MCP=Skip: no MCP tool refreshes a model in place; the wire imports a new one; CLI=Skip: the batch CLI imports each model once and never refreshes it
* **Constructor evidence:** No reached surface declared.

### 63. `Command::InvalidateToolpathInputs` — `invalidate_toolpath_inputs`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:724`
* **Declared G/M/C:** GUI=Skip: the feeds Apply funnel writes one replace_toolpath_config, which drops; MCP=Skip: the apply_feeds holdout takes the same funnel and writes one config row; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 64. `Command::UpdateStockFromBbox` — `update_stock_from_bbox`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:737`
* **Declared G/M/C:** GUI=Skip: no GUI control sizes the stock from a bounding box; MCP=Skip: no MCP tool sizes the stock from a bounding box; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 65. `Command::ReplaceTools` — `replace_tools`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:750`
* **Declared G/M/C:** GUI=Skip: no GUI control replaces the whole tools list; MCP=Skip: no MCP tool replaces the whole tools list; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 66. `Command::SetFeedsProvenance` — `set_feeds_provenance`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:762`
* **Declared G/M/C:** GUI=Skip: the optimizer carries the stamp in restore_toolpath_snapshot since WP8; MCP=Skip: no MCP tool stamps a provenance on its own; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 67. `Command::SetMachineRef` — `set_machine_ref`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:775`
* **Declared G/M/C:** GUI=Skip: no GUI control writes the library reference on its own; MCP=Skip: load_machine_from_library writes the machine; nothing writes the name; CLI=Skip: the batch CLI exposes no such command
* **Constructor evidence:** No reached surface declared.

### 68. `Job::GenerateToolpath` — `generate_toolpath`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:787`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Reached
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/compute.rs:311 | MCP reached: crates/rs_cam_viz/src/app/mcp.rs:447 -> :466 -> mcp_generate_toolpath :3380 -> AppEvent::GenerateToolpath :3419 -> crates/rs_cam_viz/src/controller/events/compute.rs:311 (Job::GenerateToolpath) | CLI reached: crates/rs_cam_cli/src/run.rs:184 (ProjectSession::generate_toolpath convenience wrapper); core convenience implementation: crates/rs_cam_core/src/session/compute.rs:3227 (generate_toolpath performs the start/run/adopt sequence)

### 69. `Command::AdoptSimulation` — `adopt_simulation`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:794`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: the MCP run_simulation tool simulates through the GUI lane, which adopts; CLI=Skip: the CLI simulates through run_simulation, which stores the result itself
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/controller/events/compute.rs:801

### 70. `Job::RecommendClearingStrategy` — `recommend_clearing_strategy`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:804`
* **Declared G/M/C:** GUI=Skip: no GUI panel asks the advisor; the verdict surfaces on the MCP wire only; MCP=Reached; CLI=Skip: the batch CLI exposes no strategy advisor command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp.rs:1309

### 71. `Job::PreviewTierMap` — `preview_tier_map`
* **Registry:** `crates/rs_cam_core/src/session/command.rs:816`
* **Declared G/M/C:** GUI=Skip: the planner dialog lends the session to the Optimize lane; WP14b moves it; MCP=Reached; CLI=Skip: the batch CLI exposes no planner preview command
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/app/mcp.rs:3218

### 72. `UiCommand::ExportGcode` — `open_export_preflight`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:352`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: the export_gcode tool exports; it opens no pre-flight panel; CLI=Skip: the batch CLI draws no pre-flight panel
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/readiness_panel.rs:213, crates/rs_cam_viz/src/ui/menu_bar.rs:28, crates/rs_cam_viz/src/ui/menu_bar.rs:100

### 73. `UiCommand::Select` — `select_in_tree`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:360`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: set_ui_view carries the selection instead; CLI=Skip: the batch CLI draws no tree
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/setup_panel.rs:41, crates/rs_cam_viz/src/ui/setup_panel.rs:68, crates/rs_cam_viz/src/ui/setup_panel.rs:109, crates/rs_cam_viz/src/ui/setup_panel.rs:262, crates/rs_cam_viz/src/ui/toolpath_panel.rs:211

### 74. `UiCommand::SetViewPreset` — `set_view_preset`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:366`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool moves the camera to a preset; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:251, crates/rs_cam_viz/src/ui/menu_bar.rs:257, crates/rs_cam_viz/src/ui/menu_bar.rs:263, crates/rs_cam_viz/src/ui/menu_bar.rs:269, crates/rs_cam_viz/src/ui/viewport_overlay.rs:50

### 75. `UiCommand::ToggleProjection` — `toggle_projection`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:372`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool switches the camera projection; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/viewport_overlay.rs:88, crates/rs_cam_viz/src/ui/viewport_overlay.rs:100

### 76. `UiCommand::ClearIsolation` — `clear_isolation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:378`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool clears the viewport isolation; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/toolpath_panel.rs:599, crates/rs_cam_viz/src/ui/viewport_overlay.rs:133, crates/rs_cam_viz/src/ui/toolpath_row_controls.rs:125

### 77. `UiCommand::PreviewOrientation` — `preview_orientation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:384`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool previews a setup orientation; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/setup.rs:77

### 78. `UiCommand::ResetView` — `reset_view`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:390`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool refits the camera; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:246, crates/rs_cam_viz/src/ui/viewport_overlay.rs:69

### 79. `UiCommand::SwitchWorkspace` — `switch_workspace`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:396`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: set_ui_view carries the workspace instead; CLI=Skip: the batch CLI draws no workspace
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_op_list.rs:174, crates/rs_cam_viz/src/ui/readiness_panel.rs:66, crates/rs_cam_viz/src/ui/readiness_panel.rs:101, crates/rs_cam_viz/src/ui/preflight.rs:31, crates/rs_cam_viz/src/ui/preflight.rs:66

### 80. `UiCommand::ToggleToolpathVisibility` — `toggle_toolpath_visibility`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:402`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool hides one toolpath in the viewport; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/toolpath_panel.rs:608, crates/rs_cam_viz/src/ui/toolpath_row_controls.rs:42, crates/rs_cam_viz/src/app/input.rs:478

### 81. `UiCommand::ToggleIsolateToolpath` — `toggle_isolate_toolpath`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:409`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool isolates one toolpath; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/toolpath_panel.rs:602, crates/rs_cam_viz/src/ui/viewport_overlay.rs:140, crates/rs_cam_viz/src/ui/toolpath_row_controls.rs:131, crates/rs_cam_viz/src/app/input.rs:469

### 82. `UiCommand::InspectToolpathInSimulation` — `inspect_toolpath_in_simulation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:415`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: sim_jump_to_toolpath_start carries the same intent on the wire; CLI=Skip: the batch CLI draws no simulation workspace
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/toolpath_panel.rs:478, crates/rs_cam_viz/src/ui/toolpath_panel.rs:588

### 83. `UiCommand::ShowShortcuts` — `show_shortcuts`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:424`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool opens the shortcut window; CLI=Skip: the batch CLI draws no shortcut window
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:278

### 84. `UiCommand::Quit` — `quit`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:430`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool closes the window the MCP server is embedded in; CLI=Skip: the batch CLI runs to completion and exits
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:127

### 85. `UiCommand::ResetSimulation` — `reset_simulation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:440`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: run_simulation replaces a run; no wire tool clears one; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:234, crates/rs_cam_viz/src/ui/viewport_overlay.rs:177

### 86. `UiCommand::ToggleSimPlayback` — `toggle_sim_playback`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:446`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool starts or stops playback; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_timeline.rs:1106, crates/rs_cam_viz/src/app/input.rs:631

### 87. `UiCommand::SimStepForward` — `sim_step_forward`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:452`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: sim_jump_to_move names the move instead; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_timeline.rs:1113, crates/rs_cam_viz/src/app/input.rs:612

### 88. `UiCommand::SimStepBackward` — `sim_step_backward`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:458`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: sim_jump_to_move names the move instead; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_timeline.rs:1089, crates/rs_cam_viz/src/app/input.rs:607

### 89. `UiCommand::SimJumpToStart` — `sim_jump_to_start`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:464`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/app/input.rs:619 | MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1402, crates/rs_cam_viz/src/app/mcp.rs:4292

### 90. `UiCommand::SimJumpToEnd` — `sim_jump_to_end`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:470`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/app/input.rs:624 | MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1413, crates/rs_cam_viz/src/app/mcp.rs:4306

### 91. `UiCommand::SimJumpToMove` — `sim_jump_to_move`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:476`
* **Declared G/M/C:** GUI=Reached; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_op_list.rs:663, crates/rs_cam_viz/src/ui/sim_op_list.rs:673, crates/rs_cam_viz/src/ui/sim_op_list.rs:855, crates/rs_cam_viz/src/ui/sim_op_list.rs:867, crates/rs_cam_viz/src/ui/sim_diagnostics.rs:264 | MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1389, crates/rs_cam_viz/src/app/mcp.rs:4277, crates/rs_cam_viz/src/app/mcp.rs:4335

### 92. `UiCommand::SimJumpToOpStart` — `sim_jump_to_op_start`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:482`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: sim_jump_to_toolpath_start names the toolpath, not the boundary; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/sim_op_list.rs:303, crates/rs_cam_viz/src/app/input.rs:340

### 93. `UiCommand::SimJumpToOpEnd` — `sim_jump_to_op_end`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:490`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: sim_jump_to_toolpath_end names the toolpath, not the boundary; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** GUI reached: NOTPROVEN (no non-test qualified constructor found by static scan)

### 94. `UiCommand::SimScrubToolpath` — `sim_scrub_toolpath`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:498`
* **Declared G/M/C:** GUI=Skip: the timeline drags through sim_jump_to_move, which names the move; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1429

### 95. `UiCommand::SimJumpToToolpathStart` — `sim_jump_to_toolpath_start`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:506`
* **Declared G/M/C:** GUI=Skip: the operation list jumps through sim_jump_to_op_start; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1447

### 96. `UiCommand::SimJumpToToolpathEnd` — `sim_jump_to_toolpath_end`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:515`
* **Declared G/M/C:** GUI=Skip: the operation list jumps through sim_jump_to_op_end; MCP=Reached; CLI=Skip: the batch CLI holds no playback state
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1465

### 97. `UiCommand::ScreenshotSimulation` — `screenshot_simulation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:526`
* **Declared G/M/C:** GUI=Skip: the operator reads the viewport; no GUI control writes a PNG; MCP=Reached; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1489, crates/rs_cam_viz/src/app/mcp.rs:544

### 98. `UiCommand::ScreenshotToolpath` — `screenshot_toolpath`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:535`
* **Declared G/M/C:** GUI=Skip: the operator reads the viewport; no GUI control writes a PNG; MCP=Reached; CLI=Skip: the batch CLI draws no viewport
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1519, crates/rs_cam_viz/src/app/mcp.rs:560

### 99. `UiCommand::ScreenshotGui` — `screenshot_gui`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:543`
* **Declared G/M/C:** GUI=Skip: the operator reads the window; no GUI control captures it; MCP=Reached; CLI=Skip: the batch CLI draws no window
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1571, crates/rs_cam_viz/src/app/mcp.rs:582

### 100. `UiCommand::SetUiView` — `set_ui_view`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:551`
* **Declared G/M/C:** GUI=Skip: a GUI control writes the one field it owns, never a view bundle; MCP=Reached; CLI=Skip: the batch CLI draws no view
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1598, crates/rs_cam_viz/src/app/mcp.rs:592

### 101. `UiCommand::OpenToolLibrary` — `open_tool_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:561`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: list_tool_library reports the same catalogs; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/toolpath_panel.rs:197, crates/rs_cam_viz/src/ui/menu_bar.rs:209

### 102. `UiCommand::CloseToolLibrary` — `close_tool_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:567`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool closes a modal it cannot open; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:58

### 103. `UiCommand::OpenMachineLibrary` — `open_machine_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:573`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: list_machine_library reports the same machines; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/properties/mod.rs:1675

### 104. `UiCommand::CloseMachineLibrary` — `close_machine_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:579`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool closes a modal it cannot open; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/machine_library_modal.rs:40

### 105. `UiCommand::OpenExportWizard` — `open_export_wizard`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:585`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: export_gcode exports without the wizard; CLI=Skip: the batch CLI draws no wizard
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:23, crates/rs_cam_viz/src/ui/menu_bar.rs:92

### 106. `UiCommand::CloseExportWizard` — `close_export_wizard`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:591`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool closes a wizard it cannot open; CLI=Skip: the batch CLI draws no wizard
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/export_wizard.rs:71, crates/rs_cam_viz/src/ui/export_wizard.rs:113

### 107. `UiCommand::OpenMultitoolPlanner` — `open_multitool_planner`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:597`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: plan_multitool_finishing emits the ladder directly; CLI=Skip: the batch CLI draws no planner
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/menu_bar.rs:202, crates/rs_cam_viz/src/ui/overlays/panel.rs:275

### 108. `UiCommand::CloseMultitoolPlanner` — `close_multitool_planner`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:603`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool closes a planner it cannot open; CLI=Skip: the batch CLI draws no planner
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/multitool_planner.rs:70, crates/rs_cam_viz/src/ui/multitool_planner.rs:831

### 109. `UiCommand::CloseFeedsModal` — `close_feeds_modal`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:611`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: apply_feeds writes the recipe without a modal; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:66, crates/rs_cam_viz/src/ui/feeds_modal.rs:102

### 110. `UiCommand::SetFeedsModalMode` — `set_feeds_modal_mode`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:617`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool switches a modal's own mode; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:125, crates/rs_cam_viz/src/ui/feeds_modal.rs:135

### 111. `UiCommand::ToggleFeedsProvenance` — `toggle_feeds_provenance`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:623`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: get_suggest_rationale reports the same provenance; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:959

### 112. `UiCommand::SetFeedsProjectSort` — `set_feeds_project_sort`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:629`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool sorts a table it cannot see; CLI=Skip: the batch CLI draws no table
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:2929, crates/rs_cam_viz/src/ui/feeds_modal.rs:2937, crates/rs_cam_viz/src/ui/feeds_modal.rs:2945

### 113. `UiCommand::SetFeedsExplore` — `set_feeds_explore`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:636`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool drags a chart overlay; CLI=Skip: the batch CLI draws no chart
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:1842, crates/rs_cam_viz/src/ui/feeds_modal.rs:2150, crates/rs_cam_viz/src/ui/feeds_modal.rs:2234, crates/rs_cam_viz/src/ui/feeds_modal.rs:2248, crates/rs_cam_viz/src/ui/feeds_modal.rs:2259

### 114. `UiCommand::ToggleFeedsProjectRow` — `toggle_feeds_project_row`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:642`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: apply_feeds names the toolpath it writes; CLI=Skip: the batch CLI draws no table
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:2973

### 115. `UiCommand::SetFeedsProjectScatter` — `set_feeds_project_scatter`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:649`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool switches a chart overlay; CLI=Skip: the batch CLI draws no chart
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:2872

### 116. `UiCommand::SetFeedsProjectSelectAll` — `set_feeds_project_select_all`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:656`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: apply_feeds names the toolpath it writes; CLI=Skip: the batch CLI draws no table
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/feeds_modal.rs:2895

### 117. `UiCommand::SetToolLoadOverride` — `set_tool_load_override`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:663`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: export_gcode carries both accept flags per call; CLI=Skip: the batch CLI carries its own export flags
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/preflight.rs:502, crates/rs_cam_viz/src/app/input.rs:389

### 118. `UiCommand::CloseOptimizeModal` — `close_optimize_modal`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:672`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: optimize_toolpath answers without a modal; CLI=Skip: the batch CLI draws no modal
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/optimize_modal.rs:58, crates/rs_cam_viz/src/ui/optimize_modal.rs:86, crates/rs_cam_viz/src/ui/optimize_modal.rs:99, crates/rs_cam_viz/src/ui/optimize_modal.rs:289, crates/rs_cam_viz/src/ui/optimize_modal.rs:567

### 119. `UiCommand::CloseOptimizeProject` — `close_optimize_project`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:678`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool opens the project rollup; CLI=Skip: the batch CLI draws no rollup
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/optimize_project.rs:41, crates/rs_cam_viz/src/ui/optimize_project.rs:91, crates/rs_cam_viz/src/ui/optimize_project.rs:105, crates/rs_cam_viz/src/ui/optimize_project.rs:133, crates/rs_cam_viz/src/ui/optimize_project.rs:235

### 120. `UiCommand::ToggleOptimizeProjectRow` — `toggle_optimize_project_row`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:684`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool selects a rollup row; CLI=Skip: the batch CLI draws no rollup
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/optimize_project.rs:368

### 121. `UiCommand::SetStaleExportPolicy` — `set_stale_export_policy`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:693`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: export_gcode carries accept_previous_geometry per call; CLI=Skip: the batch CLI carries its own export flags
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/preflight.rs:426

### 122. `UiCommand::DeleteLibraryTool` — `delete_library_tool`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:704`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:395

### 123. `UiCommand::UpdateLibraryTool` — `update_library_tool`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:710`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:548, crates/rs_cam_viz/src/controller/events/mod.rs:392

### 124. `UiCommand::MoveLibraryTool` — `move_library_tool`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:716`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:439

### 125. `UiCommand::CreateToolCatalog` — `create_tool_catalog`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:722`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:151

### 126. `UiCommand::DeleteToolCatalog` — `delete_tool_catalog`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:728`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:219

### 127. `UiCommand::RenameToolCatalog` — `rename_tool_catalog`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:734`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:198

### 128. `UiCommand::DedupeToolCatalog` — `dedupe_tool_catalog`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:740`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user tool library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/tool_library_modal.rs:212

### 129. `UiCommand::SaveMachineToLibrary` — `save_machine_to_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:746`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user machine library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/machine_library_modal.rs:137

### 130. `UiCommand::DeleteMachineFromLibrary` — `delete_machine_from_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:753`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user machine library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/machine_library_modal.rs:217

### 131. `UiCommand::RenameMachineInLibrary` — `rename_machine_in_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:760`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: no wire tool writes the per-user machine library; CLI=Skip: the batch CLI reads the library and never writes it
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/machine_library_modal.rs:248

### 132. `UiCommand::CancelCompute` — `cancel_compute`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:769`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: cancel_generation cancels the toolpath lane alone, off the frame loop; CLI=Skip: the batch CLI computes on its own thread
* **Constructor evidence:** GUI reached: crates/rs_cam_viz/src/ui/viewport_overlay.rs:160

### 133. `UiCommand::CancelToolpathGeneration` — `cancel_toolpath_generation`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:777`
* **Declared G/M/C:** GUI=Reached; MCP=Skip: A/M12: cancel_generation is served through GenerationControl on \ the MCP server thread, so it never queues behind the generate it aborts; CLI=Skip: the batch CLI computes on its own thread
* **Constructor evidence:** GUI reached: NOTPROVEN (no non-test qualified constructor found by static scan)

### 134. `UiQuery::GetDiagnostics` — `get_diagnostics`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:788`
* **Declared G/M/C:** GUI=Skip: the diagnostics panel reads the simulation slot without this door; MCP=Reached; CLI=Skip: the CLI project report reads the core session
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:505

### 135. `UiQuery::GetToolLoadReport` — `get_tool_load_report`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:796`
* **Declared G/M/C:** GUI=Skip: the readiness panel reads the simulation slot without this door; MCP=Reached; CLI=Skip: the CLI project report reads the core session
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:871

### 136. `UiQuery::GetCutTrace` — `get_cut_trace`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:804`
* **Declared G/M/C:** GUI=Skip: the simulation panels read the trace in the slot without this door; MCP=Reached; CLI=Skip: the CLI holds no view simulation slot
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:575, crates/rs_cam_viz/src/app/mcp.rs:683

### 137. `UiQuery::InspectCollisions` — `inspect_collisions`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:812`
* **Declared G/M/C:** GUI=Skip: the diagnostics panel reads the simulation slot without this door; MCP=Reached; CLI=Skip: the CLI holds no view simulation slot
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:671

### 138. `UiQuery::GetNotifications` — `get_notifications`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:820`
* **Declared G/M/C:** GUI=Skip: the toast stack draws itself; no panel reads it; MCP=Reached; CLI=Skip: the batch CLI pushes no toasts
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:963, crates/rs_cam_viz/src/app/mcp.rs:677

### 139. `UiQuery::ListToolLibrary` — `list_tool_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:826`
* **Declared G/M/C:** GUI=Skip: the Tool Library modal loads its own snapshot of the catalog files; MCP=Reached; CLI=Skip: the batch CLI lists no library
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:441

### 140. `UiQuery::ListToolCatalog` — `list_tool_catalog`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:834`
* **Declared G/M/C:** GUI=Skip: the Tool Library modal loads its own snapshot of the catalog files; MCP=Reached; CLI=Skip: the batch CLI lists no library
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:455

### 141. `UiQuery::ListMachineLibrary` — `list_machine_library`
* **Registry:** `crates/rs_cam_viz/src/ui_command.rs:842`
* **Declared G/M/C:** GUI=Skip: the Machine Library modal loads its own snapshot of the library files; MCP=Reached; CLI=Skip: the batch CLI lists no library
* **Constructor evidence:** MCP reached: crates/rs_cam_viz/src/mcp_server.rs:1648

## §21 WP13 numbered-ruling audit at this pin

1. **Second viz registry:** implemented: `for_each_ui_command!` / generated view payloads and `SurfaceId` are in `crates/rs_cam_viz/src/ui_command.rs` (70 rows: 62 `UiCommand`, 8 `UiQuery`).
2. **`UiQuery` kind:** implemented as the fifth `CommandKind` variant in core registry machinery.
3. **`GetOperationSchema` core Query:** implemented; row 43 is `Query::GetOperationSchema` at `session/command.rs:501`, MCP construction `app/mcp.rs:996`.
4. **Read door:** residual wording deviation: pin uses `RsCamApp::ui_query(&self, UiQuery) -> UiQueryAnswer` (`app/mcp.rs`), not the ruling's `AppState` receiver. The sentry explicitly measures the actual shared receiver (`command_surface_completeness.rs:457-490`).
5. **Library/file-store classification:** implemented as view rows; this map retains their kind and reach columns rather than relabeling them Store.
6. **UiCommand reach/snapshot requirement:** deliberate measured deviation, not silently false `Reached`: exactly seven MCP-only `UiCommand` rows are GUI Skip/MCP Reached — `SimScrubToolpath` (`ui_command.rs:498`), `SimJumpToToolpathStart` (`:506`), `SimJumpToToolpathEnd` (`:515`), `ScreenshotSimulation` (`:526`), `ScreenshotToolpath` (`:535`), `ScreenshotGui` (`:543`), and `SetUiView` (`:551`). The pin's sentry checks every view row is reached by GUI **or** MCP (`command_surface_completeness.rs:295-319`), not ruling 6's universal GUI reach.
7. **MCP Ui wrapper:** implemented by the `McpRequestKind::Ui(UiCommand)` / UiQuery routing; constructor evidence above records only actual wire/router construction, not request enum declarations.
8. **RemoveSetup deletion:** no `RemoveSetup` registry row at pin; not counted among 141.
9. **Cross-surface sentry:** present at `crates/rs_cam_viz/tests/command_surface_completeness.rs`; its P1 core scan is source-level and comments-stripped, but it is not a runtime proof. This map improves the requested per-row locator result and does not treat test tokens as constructors.
10. **Optimizer named exemptions:** `CloseOptimizeModal` and `CloseOptimizeProject` remain named historical records in P3 (`command_surface_completeness.rs:339`); P3 says they pass unaided, so they are not exclusions from this audit.

## Residual ledger

* `PreviewTierMap` GUI Skip is a **forward promise wording residual**: its pin reason at `session/command.rs:819-821` says “WP14b moves it”. WP14b was excluded from the in-flight user scope, so this is not reported as an implementation gap.
* `UiCommand::ExportGcode` is retained as a UI command (`open_export_preflight`), while core export remains distinct; do not conflate it with the §14 artifact-query residual ledger.
* The union count is **141 names** (71 core + 70 view), not a claim about MCP wire-tool count.
* Two declared GUI reaches lack a non-test qualified constructor under this audit’s strict rule: `SimJumpToOpEnd` (`ui_command.rs:490`) and `CancelToolpathGeneration` (`ui_command.rs:777`). They are explicitly `NOTPROVEN` above; no recommendation silently flips their reach declarations.


## E. Ranked findings

### BLOCKER

- **B1 — two GUI Reached rows have no live constructor.** `crates/rs_cam_viz/src/ui_command.rs:490` (`SimJumpToOpEnd`) and `:777` (`CancelToolpathGeneration`) claim GUI Reached, but no non-test constructor exists. All hits for SimJump are payload `:99`, registry row `:490`, events import `events/mod.rs:19`, and handler `:448`; `SimJumpToOpStart` is the actual GUI control at `ui/sim_op_list.rs:303`. All Cancel hits are row `:777`, handler `events/mod.rs:452`, and only test/comment construction at `controller/tests.rs:1048,1061`; GUI Cancel All emits `CancelCompute` at `ui/viewport_overlay.rs:160`. MCP `cancel_generation` directly calls `generation.request_cancel` at `mcp_server.rs:1324`. The P1 sentry `command_surface_completeness.rs:146-235` censes only core constructors; view P2 only requires some surface plus a handler. **Fix:** remove the two unused rows/handlers, or connect ruled controls and extend the view-constructor census. Do not change both rows to Skip; that would retain orphan handlers and violate P2.

### MAJOR

- **M1 — §21.6 says every UiCommand has GUI Reached, but seven legitimate MCP-only rows do not.** `crates/rs_cam_viz/src/ui_command.rs:498,506,515,526,535,543,551`. **Fix:** obtain an explicit ruling that narrows the assertion, then amend the adopted text and sentry contract rather than mark real MCP rows falsely Reached.
- **M2 — §22 ownership and off-loop rules differ from the implementation.** `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md:1459-1465` demands a session-owned model/revision memo, no index build in `start`, and a flag on the handle. `crates/rs_cam_core/src/session/compute.rs:2884,3152` force `LazyIndex` inside the resolver/start path for `PlannedTierRegions`; `crates/rs_cam_core/src/geom_cache.rs:318-348` wraps a process-global weak-identity cache (`static TABLE:170`) and creates a fresh `LazyIndex` for each handle; `crates/rs_cam_viz/src/compute/worker.rs:90,435` puts the flag in `VizExtras`/`JobRequest`, while `crates/rs_cam_core/src/session/compute.rs:388-406` has no flag on `GenerateToolpathHandle`. The normal executor forces work off-loop, but the universal rule that `start` never builds is false. This is a documented implementation departure, not proof of wrong geometry or a cancellation bug; observer-field placement differences are explicit in §C, not a separate bug, and this is outside excluded WP14b/WP19 work. No runtime benchmark is claimed. **Fix:** Record an explicit amendment for these ownership and frame-loop exceptions, or change the implementation to meet §22 and add sentries for the chosen contract.

### MINOR

- **N1 — SetSetupName did not meet §19.3's same-commit red-to-fix requirement.** `crates/rs_cam_viz/src/controller/events/model.rs:468-480`; sentry `ff81b712` changes Skip to Reached before caller fix `183900e2`. **Fix:** record the exception; do not rewrite history.
- **N2 — PreviewTierMap has forward-promise wording.** `crates/rs_cam_core/src/session/command.rs:819-821`. **Fix:** replace “WP14b moves it” with a present-tense scope note; WP14b is excluded, so this is not an implementation gap.

## F. Core test inventory not named by this review evidence

Pinned inventory: 320 top-level core targets. Evidence names 36. The 284 unmentioned targets are 272 normal and 12 `heavy-tests`. A name absent from this report does not mean it never ran before phase N1. Default STEP-feature zero-test output is NOT MEASURED. An ignored-only target is also NOT MEASURED without an explicit ignored selection.

### Normal targets (272)

- `_litmatrix_drill_rpm_ceiling`
- `_litmatrix_drill_rpm_diameter_tier`
- `_litmatrix_ipe_janka_scaling`
- `_litmatrix_milling_rpm_diameter_tier`
- `_litmatrix_rpm_only_lut_chipload`
- `_litmatrix_rubbing_floor_clamp`
- `_litmatrix_scallop_refuses_flat`
- `adaptive3d_boundary_clear_parity`
- `adaptive3d_commanded_ladder`
- `adaptive3d_entry_coalescing_f038`
- `adaptive3d_keep_down_link_f038b`
- `adaptive3d_lift_bridge_b1`
- `adaptive3d_post_tsp_z_monotonicity`
- `adaptive3d_subtool_channel_gouge`
- `adaptive_feed_modulation_gcode_f036a`
- `adaptive_feed_modulation_pipeline_f036b`
- `adaptive_property_harness`
- `adversarial_2d_campaign_r2`
- `agent_search_axial_doc`
- `agent_search_coverage`
- `air_cut_denominators_lh1`
- `air_cut_one_time_base_g_airdenom`
- `air_filter_tool_aware_s3`
- `air_ladder_emitted_z_levels_g_airladder`
- `alignment_pin_keying_g_pinauto`
- `arcfit_intent_boundary_f1`
- `arcfit_intent_key_cost_f1`
- `axial_doc_step_multiple_h4`
- `axial_engagement_vs_dpp_detector_f2`
- `band_cell_ownership_g2`
- `band_run_off_reproduction_d16_1`
- `band_stamping_determinism_s3`
- `banded_raster_costed_f2`
- `bikeseat_gate_d1`
- `boundary_clip_escape_f1`
- `boundary_reentry_plunge_rate_g_boundaryplunge`
- `bull_nose_cusp_radius_g_bullcusp`
- `capability_link_moves_safety`
- `catchment_basin_census_w0`
- `cavalier_shape_failure_r2`
- `ceiling_advisory_and_clamp_record_a7`
- `checkpoint_a_valley_matrix`
- `checkpoint_c9_sampled_reach`
- `chip_thickness_policy_a9`
- `chipload_abstention_cannot_supersede_g_chipgate`
- `chipload_advisory_disclosure_h4`
- `chipload_boundary_g_chip_ulp`
- `chipload_extrapolation_flag_rider`
- `chipload_formula_calibration`
- `chipload_report_wording_t12_t15`
- `chipload_thinning_magnitude_survey`
- `classification_columns_ab_m3`
- `classification_strategy_m3`
- `common_fixtures_smoke_c6`
- `composite_render_convention`
- `conformal_spiral_synthetic_f2`
- `constrained_max_modulation_f039`
- `contour_spiral_gcode_validity_phase0`
- `coverage_routing_pr5`
- `crease_own_region_pr6b`
- `cut_direction_matches_transform_g_lateralsign`
- `depth_beyond_stock_core_g_depthstockcore`
- `derived_stepover_pr6a`
- `descent_resolution_stability_am10`
- `dexel_stock_z_frame_f024`
- `dexel_stock_z_frame_f026`
- `direction_field_wanaka_f1`
- `dressup_span_invariants`
- `drill_cycle_time_integration_g_drilltime`
- `drill_entry_dressup_g_wanaka_drill_ramp`
- `drill_evidence_wording_d3`
- `drill_fed_descents_motion`
- `drill_flip_removal_g_drillflip`
- `drill_material_plumbing_f016`
- `drill_metrics_pr2`
- `drill_no_targets_refuses_g_drillcentroid`
- `drill_op_step3`
- `drill_peck_depth_g_wanaka_peck`
- `drill_pick_emission_frame_g_drillpick`
- `drop_cutter_flat_roughing_row_g_dcflat`
- `drop_cutter_off_mesh`
- `end_to_end`
- `engagement_chip_thickness_labels_f4`
- `engagement_denominator_m3`
- `engagement_vector_step2`
- `entry_descent_profile_b2`
- `entry_moves_stock_aware_g_rampterrain`
- `export_disabled_cached_n1`
- `exporter_span_classifier_x1`
- `face_stock_top_frame_f028`
- `face_up_names_follow_drafting_convention_g_frontname`
- `feed_explanation_record_t1`
- `feed_explanation_snapshot_b3`
- `findings_transport_join_h21`
- `finish_planner_wanaka_decompose`
- `finish_resolution_policy_pr3`
- `finish_surface_cache`
- `flipped_setup_axial_doc_repro`
- `frozen_snapshot_regeneration_s4`
- `gate_population_vacuity_xvac`
- `gcode_emulator_validation`
- `gcode_phase0_capture`
- `gcode_validator_baseline`
- `generated_empty_refusal_g_entryempty`
- `generator_extremes_fuzz_r1`
- `generic_rest_routing_pr7`
- `geometry_cache_g8`
- `graded_raster_e1`
- `graded_raster_tier0_e2`
- `grid_z_uncovered_contract_c2`
- `heatmap_two_arc_divergence_a1`
- `identity_setup_emission_frame_audit`
- `inert_claims_dial_f4`
- `island_stay_down_links_o3`
- `isoclip_entry_ramp_g_isoclipentry`
- `isoclip_link_rapid_g_isocliprapid`
- `kinematic_surfacing_p4`
- `kinematic_utilization_p2`
- `kinematics_histogram`
- `kinematics_per_axis_rate_p1`
- `lateral_scrub_playback_stock_g_lateralscrub`
- `law_magnitude_measurement`
- `lead_in_out_feed_rates_f040`
- `lead_out_retract_lifts_from_the_arc_f2`
- `lift_probe_g3`
- `link_counters_visible_g_linkvisible`
- `link_stage_g_linkstage`
- `literature_matrix`
- `literature_parity`
- `lookup_parity`
- `lut_resolver_census_a6`
- `lut_resolver_purposes_a7`
- `measurability_abstention_r8`
- `model_units_survive_reload_g_unitsreload`
- `monotone_cell_decomposition_c2`
- `move_intent_step1`
- `move_to_setup_honours_index_g_dropindex`
- `multitool_plan_emission_o1`
- `multitool_plan_roundtrip_o1b`
- `multitool_preview_u1`
- `narrate_regions_closed_c8`
- `narration_cost_probe_h26`
- `narration_denominator_and_hints_d7`
- `nn_order_scaling_g5_g6`
- `offset_polygon_degenerate_inputs_r1`
- `op_model_ref_static_validation_f023`
- `op_precondition_static_validation_f015`
- `optimize_smoke`
- `optimizer_assumption_stamp_a8`
- `p1_headless_ab_wanaka`
- `param_sweep`
- `pencil_entry_ramp_g_entryload`
- `pencil_hop_dial_g_pencilhop`
- `pencil_spine_ab_p1`
- `pencil_surface_link_g_linkload`
- `pencil_tip_float_channel_d1`
- `per_point_claims_fan_c9`
- `perf_golden_depth_level_geometry`
- `perf_golden_sim_metrics`
- `pill_writes_clamped_value_g_pillclamp`
- `pindrill_emission_frame_g_pindrill`
- `planned_tier_regions_boundary_o2`
- `playback_band_dispatch_s6`
- `plunge_guard_ab_p3`
- `plunge_guard_p3`
- `pocket_lift_bridge_b1`
- `post_format_round_trip_p1`
- `power_ceiling_parity_f2`
- `predicted_feed_gates_f035`
- `profile_link_ceiling`
- `profile_side_survives_setup_flip_g_profile_flip`
- `project_curve_depth_sign`
- `project_curve_deviation`
- `property_tests`
- `pushcutter_band_query_g1`
- `radial_finish_ranges_n10`
- `ramp_contained_in_region_g_rampcontain`
- `ramp_reach_clamp_pr8b`
- `rapid_check_wanaka_link_shape`
- `rapid_collision_detector_population`
- `rapid_live_check_crest_s2`
- `rapid_replay_shipped_gcode_s1`
- `reach_map_p5`
- `reach_map_residual_p5_1`
- `reach_policy_pr4`
- `reach_tolerance_source_p5_1`
- `reference_plate_contract`
- `reference_repeatability`
- `region_cap_honesty_f3`
- `remap_interval_index_c9`
- `reorder_toolpath_inserts_g_dropindex`
- `rest_grid_resolution_c9`
- `rest_region_tip_radius_dilation_f1`
- `rest_routing_probe_e9`
- `retime_respects_no_kinematics_n7`
- `retract_intent_move_type_census_w6`
- `retract_trip_channel_am7`
- `ring_sample_bound_w14`
- `rubbing_floor_diameter_scaling_measurement`
- `rubbing_floor_envelope_band_p1`
- `rubbing_floor_never_exceeds_band`
- `safe_z_emission_frame_g_safez_local`
- `sample_source_intent_r11`
- `save_temp_path_unique_f124`
- `scallop_intra_pass_relink_am7`
- `scallop_iso_field_config`
- `scallop_solo_vs_unified_s1`
- `scallop_trace_survives_relink_g_linktrace`
- `scallop_untouched_standing_h4`
- `schema_enum_values_g_schemaenum`
- `shallow_band_stock_to_leave_exhibit_d16_2`
- `shallow_raster_slope_derate`
- `shipped_raster_spacing_b1`
- `sim_chipload_invariant`
- `sim_identity_setup_playback_frame`
- `sim_prefix_memo_s5`
- `sim_radial_engagement_density`
- `smoke_baseline_regression_f037`
- `spacing_prize_split_f1`
- `span_summary_single_pass_c1`
- `spiral_finish_compact_c1`
- `standing_material_channel_am9`
- `steep_shallow_min_segment_pr8d`
- `step5_marching_cubes`
- `step_import`
- `step_project_load`
- `strategy_comparison_h4`
- `sub_cell_stamping_fa`
- `suggest_feed_matches_final_geometry`
- `suggest_power_ceiling_after_pass9_g_suggest_powerstale`
- `swept_stamping_s1`
- `swept_wanaka_ab_s1`
- `tapered_ball_relief_profile_probe`
- `tapered_cusp_radius_sentry`
- `tapered_width_model_parity_c3`
- `test_data_smoke_csv_alignment`
- `thin_organic_island_widths`
- `tier_band_overlap_g_overlapfill`
- `tier_islands_i1`
- `tier_map_cache_t3`
- `tier_map_slope_t2`
- `tier_map_walk_t1`
- `tool_geometry_hygiene`
- `tool_scale_semantics_pr2`
- `transform_provenance_fingerprints`
- `tsp_synthesized_rapid_intents`
- `unified_finish_dropped_band_finding_d1`
- `unified_finish_partial_clip_finding_c8`
- `unified_finish_planner_dials_f2`
- `unified_finish_tapered_end_to_end_m21`
- `unified_topo_dump_e3`
- `union_coverage_m1`
- `valley_branch_falsifier_h1`
- `valley_prize_census_h0`
- `vcarve_lift_bridge_b1`
- `vendor_lut_sub_1mm`
- `vendor_sidebyside_chipload`
- `wanaka_axial_doc`
- `wanaka_boundary_diag`
- `wanaka_curvature_anisotropy`
- `wanaka_defaults_validation`
- `wanaka_e2e_chipload_gate`
- `wanaka_region_capture_f1`
- `wanaka_step4_fa_revalidation`
- `wanaka_step5_mc_revalidation`
- `wanaka_suggest_integration`
- `wanaka_z_layer_render`
- `waterline_shared_finish_setup_c3`
- `whole_board_spiral_ledger_g1`
- `wood_species_library_provenance`
- `zero_removal_rest_pass_a4`
- `zone_coherence_census`

### Heavy targets (12)

- `adaptive3d_interior_cell_parity_f029`
- `adaptive3d_planner_stock_xy_f027`
- `air_cut_family_calibration_w5bf4`
- `checkpoint_b_resolution_ab`
- `feed_modulation_cycle_time_f036c`
- `machine_kinematics_cycle_time_f034`
- `offset_candidates_m5`
- `offset_growth_m5`
- `scallop_candidates_m4`
- `scallop_isofield_gouge_m4`
- `scallop_oracle_validation_m4`
- `strategy_advisor_smoke`

## What I did not read

I did not read later-than-pin work, run Cargo, inspect `target` or `release`, inspect or edit `.mcp.json`, run a GUI/MCP session, reproduce historical red/green tests, or prove runtime behaviour from static scans.
