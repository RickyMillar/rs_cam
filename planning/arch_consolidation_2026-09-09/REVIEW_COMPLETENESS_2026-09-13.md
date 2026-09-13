INCOMPLETE — 1 blocker.

# Completeness review — architecture consolidation

## A. Scope, method, and definition of done

Pin: `61c16b75c68facf24a806619666486b9cea0a320`. All source paths and lines below are at this pin. HEAD later advanced to `795ac079`; later work is excluded. This report is a static audit. PASS means the pinned source, tree shape, or checked git ancestry met the stated static check. It does not mean a current test run passed. NOT MEASURED means this review did not verify it.

I read, in order at the pin: the adopted ruling, `IMPLEMENTATION_PLAN.md` §7 and §§12–28, the STATUS work-package table, the 2026-09-12 completeness review, and the 2026-09-12 tech-debt review. The older reviews and STATUS are historical claims, not facts by themselves. I checked the primary sentry/fix pairs with `git merge-base --is-ancestor`, their tree files with `git cat-file`, pin content with `git show`/`git grep`, source locations with `rg`, and the pinned tree/inventory with Python. No Cargo command ran in this review.

### §7 definition of done

| Clause | Static result |
|---|---|
| Hatch producer calls outside core | PASS: no inter-crate hatch callers; no core public `*_mut` hatch; no viz compute mirror. |
| `MutationKind` / `compute_stale_set` deletion | NOT DONE, ledgered holdout. This is not a new blocker. |
| `ResolvedGenInputs` producer | PASS: exactly one, `core/session/compute.rs:2523`. |
| `ResolvedHeights` | PASS: two distinct types, config `:1656`, diagnostics `:58`. |
| Public loose generation entries | PASS: `execute.rs:3358,3406,3498` are crate-private. |
| No viz compute mirror | PASS. |
| `AppEvent::RemoveSetup` | PASS: deleted; only test comment remains. |
| Named sentries | PASS for current sources and relocated/renamed sentries. Eight P0 tests have real assertion files, not only STATUS prose. |
| Runtime gates | NOT MEASURED. The latest closing normal core dev loop is not executed by this review. Heavy gate is not requested. |

The four accepted handwritten MCP residuals are `app/mcp.rs:369` load project, `:383` export, `:433` multitool, and `:441` apply feeds. The stale helper is old `core/session/compute.rs:41-87`; `app/mcp.rs:2236` is the stamp consumer and the actual producers are `:2870,3341`. The advisor loose call at `core/session/compute.rs:1874` remains a named residual. The 14-argument `execute_operation` at `execute.rs:3358` has seven test callers: six in its file at `5230,5263,5297,5331,5364,5399` and `execute/pinned_bottom_z_reaches_motion_g_bottompin.rs:109`. The three session `mem::replace` sites are `events/mod.rs:626,1326` and `planner.rs:176`. The feed-optimisation geometric plunge cap is absent at `feedopt:145-204`. These are recorded residuals or excluded follow-up scope, not further findings.

Typed-setter production bypass and hatch scans found zero unledgered same-shaped bypasses. Effects intentionally ignored by permitted wrappers remain `events/mod.rs:59-66`, `ForgetResult` at `compute.rs:184`, debug MCP `:3407`, and init IO `:645,751`. This lexical scan is not a formal proof.

## B. Work-package evidence and historical red/green record

The tracker has 25 physical rows: 21 full DONE rows, two partial rows (WP14 and WP15), and two TODO/in-flight rows (WP19 and WP22). The primary-pair ancestry check passed for every ordinary pair listed below. The table distinguishes recorded historical red/green from what this review executed: this review executed no tests.

| Package | Recorded red evidence -> fix evidence | Static ancestry at pin |
|---|---|---|
| WP1 | `255f6fe3` -> `90c8e90e`; core registry and viz surface sentries | PASS |
| WP2a | `8f4faf43` -> `ecee1ee5`; wire snapshot | PASS |
| WP3 | `0d22acaf` -> `25f37b74`; stale completion sentry | PASS |
| WP4 | `4dfde9cc` -> `1c399a71`; core reach sentry. Viz describe sentry was added by the fix, not by the sentry commit. | PARTIAL pair, recorded exception |
| WP5 | `973834f7` -> `caf8fbe6`; signature gate | PASS |
| WP6 | `fd883efa` -> `9b887707`; draw-site scan | PASS |
| WP6b | `b84f2601` -> `2cfc2b9a`; non-egui and model-adoption sentries | PASS |
| WP7 | `72e912bf` -> `7dff635b`; hatch sentry | PASS |
| WP7a | `76dbfb41` -> `084b9b50`; builder sentry | PASS |
| WP8 | `5c32ced7` -> `e079e192`; restore sentry | PASS |
| WP9 | `720626b2` -> `5173c923`; cycle-time query sentry | PASS |
| WP10 | `b952e6ee` -> `3ff3cffa`; three-step job sentry | PASS |
| WP11a | `181f5c13` -> `a9ef73d6`; one-producer sentry | PASS |
| WP11b | `df963567` -> `4b53576b` and `bab065ce` -> `276e5c13`; two independent N12 sentries | PASS |
| WP12 | `89f81cb4` -> `77627c33`; loose-executor sentry | PASS |
| WP13 | `50f975e2` -> `35ae77ad`; surface sentry | PASS |
| WP14 | WP14a: `5a43a365` -> `6e9a1326`; Job-row sentry. WP14b is in-flight. | PARTIAL |
| WP15 | WP15a: `a4c0abd7` -> `ad06c690`; setter-row sentry. WP15b is in-flight. | PARTIAL |
| WP16 | `ff81b712` -> `183900e2`; shared surface sentry | PASS; historical same-commit requirement was not met. |
| WP17 | `91bc6c39` -> `525fe504`; save/simulation sentry | PASS |
| WP18 | `3654826a` and `514fd0fc`; independent sentry-only repair work | PARTIAL; no invented fix/sentry pair |
| WP19 | No pair at pin | TODO, excluded in-flight work |
| WP20 | `9b3f75de`; prose pass, no new sentry | PASS, documented exception |
| WP21 | `10c2ad37` -> `4aeaeda3`; feedopt clamp sentry | PASS |
| WP22 | No pair at pin | TODO, excluded in-flight work |

WP6 §19.10 is per-widget, not one batch: stock `a86f2df1`, setup `97e4eaf1`, properties `9b887707`, and tool `4a49b434`. WP17's recorded RED-OUTPUT heading has no colon but is still a recorded failure. The prior review's claimed missing-regex issue is not copied: named suites exist at the corrected core or viz paths. The standard labels for seven historical red blocks are metadata only: WP1, WP5, WP6, WP6b, WP7, WP9, and WP11a.

## C. Numbered ruling audit

Every numbered ruling has a separate row. Excluded means the user excluded in-flight/later work; it is not a master-branch gap.

| Ruling | Static state at pin | Path or protocol |
|---|---|---|
| §12.1 | PASS | `command.rs:745,762-770` |
| §12.2 | PASS | one `with_effects` construction site in `command.rs` |
| §12.3 | PASS | `Effects` is `#[must_use]` |
| §12.4 | PASS | `AdoptResult` carries revision |
| §12.5 | PASS | `crates/rs_cam_core/tests/adopt_result_rejects_stale_completion.rs` exists |
| §12.6 | PASS | recorded producer-return conversion is present |
| §12.7 | NOT MEASURED | two-writer protocol is source history, not a runtime fact |
| §13.WP11a.1 | PASS | public bundle, private fields, no `Default` |
| §13.WP11a.2 | PASS | one producer sentry exists |
| §13.WP11a.3 | PASS | external naming test exists |
| §13.WP9.1 | PASS | one registry list and union |
| §13.WP9.2 | PASS | answer column and `QueryAnswer` |
| §13.WP9.3 | PASS | `ToolpathCycleTime` row |
| §13.WP9.4 | PASS | `ui/readiness.rs:400` constructor path |
| §13.WP9.5 | PASS | `query_cycle_time_one_answer.rs` |
| §13.WP9.6 | NOT MEASURED | implementation order is historical protocol |
| §14.1 | PARTIAL | Recommend and Preview are Job rows; Optimize is excluded to WP14b. |
| §14.2 | PASS | `UiQuery` kind |
| §14.3 | PASS | post-write events are deleted; `SetPostConfig` is used |
| §14.4 | NOT DONE | artifact-export Query rows are a ledgered holdout; do not call all export work met. |
| §15.1 | PASS | per-row core args |
| §15.2 | PASS | describe step is keyed by `CommandId` |
| §15.3 | PASS | `Effects.created` |
| §15.4 | PASS | four hand-written MCP residuals recorded |
| §15.5 | PASS | direct move row |
| §15.6 | PASS | separate machine payload rows |
| §15.7 | PASS | tool parameter and remove-tool effects |
| §15.8 | PASS | event side effects are in describe |
| §15.9 | PASS | toast sentry retarget recorded |
| §15.10 | NOT MEASURED | writer protocol is not independently provable from source |
| §16.1 | PASS | `start` and job handles |
| §16.2 | PASS | submit captures the handle |
| §16.3 | PASS | `execute_job` is core-only |
| §16.4 | PASS | adopt uses revision |
| §16.5 | PASS | core convenience and CLI path |
| §16.6 | PARTIAL | generate-all remains outside this first Job row |
| §16.7 | PASS | debug options row |
| §16.8 | PASS | job sentry exists; execution was not rerun here |
| §17.WP8.1 | PASS | restore snapshot row |
| §17.WP8.2 | PASS | GUI restore callers and stale stamp path |
| §17.WP8.3 | PASS | drill pick invalidates chain |
| §17.WP8.4 | PASS | narrow optimizer helper remains core-only |
| §17.WP8.5 | PASS | P0 arm retarget is recorded |
| §17.WP5.1 | PASS | signature gate in core |
| §17.WP5.2 | PASS | inspector projection path |
| §17.WP5.3 | PASS | panel stamps `Effects.stale` |
| §17.WP5.4 | PASS | viz derives panel side effects |
| §17.WP5.5 | PASS | projection preserves stored fields |
| §17.WP5.6 | PASS | core and in-crate viz sentries exist |
| §17.WP5.7 | PASS | ApplyFeeds keeps the core gate |
| §18 | PASS | builder is present; the operator decision is recorded historical evidence |
| §19.1 | PASS | producer corrections carried forward |
| §19.2 | PASS | whole-profile machine row |
| §19.3 | EXCEPTION | `SetSetupName`: red `ff81b712` is not an ancestor in the same fix commit; caller arrives in `183900e2`. |
| §19.4 | PASS | free stale helper |
| §19.5 | PASS | WizardState is viz-owned |
| §19.6 | PASS | three setup rows |
| §19.7 | PASS | stock-bbox update invalidates through core |
| §19.8 | NOT MEASURED | per-widget runtime drag behaviour was not run here |
| §19.9 | PASS | id handling path present |
| §19.10 | PASS | per-widget commits: `a86f2df1` stock, `97e4eaf1` setup, `9b887707` properties, `4a49b434` tool. |
| §20.1 | PASS | builder package |
| §20.2 | PASS | no export-wizard row |
| §20.3 | PASS | in-crate reproduction route |
| §20.4 | PASS | live sites use rows |
| §20.5 | PASS | hatch sentry and crate-private declarations |
| §21.1 | PASS | second viz registry |
| §21.2 | PASS | fifth kind |
| §21.3 | PASS | GetOperationSchema core Query |
| §21.4 | DEVIATION, benign | `RsCamApp::ui_query(&self)`, not `AppState`; the receiver is still read-only. |
| §21.5 | PASS | library/file-store rows classified |
| §21.6 | NOT MET | seven valid MCP-only UiCommands are GUI Skip: SimScrubToolpath, SimJumpToToolpathStart, SimJumpToToolpathEnd, ScreenshotSimulation, ScreenshotToolpath, ScreenshotGui, SetUiView. STATUS records this deviation but does not amend the adopted wording. |
| §21.7 | PASS | MCP Ui wrapper |
| §21.8 | PASS | RemoveSetup deleted |
| §21.9 | PARTIAL | sentry is static, not runtime; B1 shows its constructor census excludes view rows. |
| §21.10 | PASS | named optimizer records remain |
| §22.1 | PASS | `GenObserver` |
| §22.2 | PASS | lazy index path |
| §22.3 | PASS | per-submit cancel path |
| §22.4 | PASS | `execute_generation` |
| §22.5 | PASS | core assembly wins the mapped branches |
| §22.6 | PASS | submit-time refusal path |
| §22.7 | PASS | ComputeRequest handle/viz split |
| §22.8 | PASS | sentry files exist; not rerun here |
| §22.9 | NOT MEASURED | package order is historical protocol |
| §22 addendum | PASS | `Command::AdoptSimulation` `command.rs:794-802`; one common GUI adoption at `controller/events/compute.rs:801` after modulation shares the Arc. The result serves both described branches; it is not a missing second physical site. |
| §23.1 | PASS | loose executor is crate-private |
| §23.2 | PASS | tests moved in-crate |
| §23.3 | RESIDUAL | advisor loose call at `core/session/compute.rs:1874` remains named |
| §23.4 | PASS | corrected grep contract; two legitimate references are allowlisted by `loose_executor_is_crate_private_wp12.rs:281-301`, scanner `:324-384` includes comments |
| §23.5 | PASS | crate-private sentry |
| §23.6 | NOT MEASURED | closing normal suite was not executed by this review |
| §24.1 | PASS | WP14a Job rows |
| §24.2 | EXCLUDED | WP14b was in-flight and excluded by scope |
| §24.3 | NOT MEASURED | timing/cost protocol was not rerun |
| §25.1 | PASS | WP15a rows and production scan |
| §25.2 | EXCLUDED | WP15b was in-flight and excluded by scope |
| §25.3 | PASS | post-config sites ledgered under WP17 |
| §25.4 | RESIDUAL | compute doors are explicitly later work |
| §25.5 | NOT MEASURED | order is historical protocol |
| §26.1 | PASS | no timeout/schema change at pin |
| §26.2 | PASS | uniform Job answer shape |
| §26.3 | RESIDUAL | Job lane is not seen by toolpath cancellation/status |
| §26.4 | EXCLUDED | GUI planner migration is WP14b |
| §26.5 | PASS | recorded ordering behaviour |
| §27.1 | EXCLUDED | WP19, WP22, WP14b, WP15b are after the pin or in-flight scope exclusions |
| §27.2 | NOT MEASURED | closing core dev loop was ordered, not run by this review |
| §27.3 | EXCLUDED | WP22 is after pin/in-flight |
| §27.4 | NOT MEASURED | verifier runtime protocol |
| §28.1 | EXCLUDED | WP19 is after pin/in-flight |
| §28.2 | EXCLUDED | WP19 helper consolidation is after pin/in-flight |
| §28.3 | EXCLUDED | WP19 panel side effect is after pin/in-flight |
| §28.4 | EXCLUDED | WP14b executor signature is after pin/in-flight |
| §28.5 | EXCLUDED | WP14b trace handling is after pin/in-flight |
| §28.6 | EXCLUDED | WP14b project rollup change is after pin/in-flight |
| §28.7 | EXCLUDED | WP14b GUI preview reach is after pin/in-flight |
| §28.8 | EXCLUDED | policy decision is after pin/in-flight |

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
