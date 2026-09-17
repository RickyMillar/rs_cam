> **DRAFT — WP13 first deliverable, scout-measured 2026-09-11 at master `2b31b7e5`. Reviewed by nobody yet. WP4 must not move a variant until the nine open calls at the end carry a ruling. Plan line numbers in this file run about 10-12 low against later trees.**

# WP13 deliverable 1 — per-variant classification (input to WP4)
I read the tree at `master` HEAD **`2b31b7e5`** — NOT the task's `dbf572b9`, NOT the plan's `bfe8586b`. Every `file:line` is from `2b31b7e5`; plan numbers run ~10-12 low. Anchor on symbol names.
Kinds: `Command` = sync validated `ProjectSession` mutation; `Query` = sync read; `Job` = capture/execute/adopt; `UiCommand` = viz only. `Comp→Cmd`/`Comp→Job` = hand-written composition (the arm calls `self.controller.*` or dispatches an `AppEvent` instead of a session method), target kind kept visible. `(h:X)` = writes hatch `X`. `Query(viz)` = a read that never touches `ProjectSession`.
`B` = `crates/rs_cam_viz/src/mcp_bridge.rs`; `M` = `crates/rs_cam_viz/src/app/mcp.rs`.
## 1. `McpRequestKind` (`B:442-874`, 76 variants)
| variant | B decl | section (comment line) | kind | evidence: M arm → one fact |
|---|---|---|---|---|
| ProjectSummary | 444 | Reads (443) | Query | M:229→M:997 `session.name/list_tools/toolpath_count`, read only |
| ListToolpaths | 445 | Reads | Query | M:233→M:1019 `session.list_toolpaths` |
| ListTools | 446 | Reads | Query | M:237→M:1059 `session.list_tools` |
| ListToolLibrary | 450 | Reads | Query(viz) | M:241→M:1067 reads per-user library dir, no session |
| ListToolCatalog | 453 | Reads | Query(viz) | M:245→M:1102 library dir, no session |
| ListSetups | 456 | Reads | Query | M:249→M:1235 `session.list_setups` |
| GetToolpathParams | 457 | Reads | Query | M:253→M:1251 `session.get_toolpath_config` |
| GetOperationSchema | 460 | Reads | Query(viz) | M:257→M:1336 catalog only, no session |
| GetDiagnostics | 463 | Reads | Query(viz) | M:261→M:1343 `state.simulation` triage |
| GetToolLoadReport | 464 | Reads | Query(viz) | M:475→M:3344 `state.simulation` trace + `&state.session` |
| GetToolpathDiagnostics | 468 | Reads | Query | M:479→M:3392 `session.diagnose_toolpath_with_trace` |
| GetProjectDiagnostics | 473 | Reads | Query | M:483→M:3415 `session.diagnose_project_with_evidence` |
| OptimizeToolpath | 477 | Reads | **Job** (see §5) | M:487→M:3441 passes `&mut ...session` to `optimize_toolpath` — MUTATES, under Reads |
| GetCutTrace | 480 | Reads | Query(viz) | M:265→M:1668 `state.simulation` only |
| GetGenerationDebugTrace | 499 | Reads | Query | M:287→M:1697 `session.get_toolpath_config` |
| NarrateToolpath | 506 | Reads | Query | M:303→M:1369 `session.get_toolpath_config/post_config/tools` |
| RecommendClearingStrategy | 513 | Reads | Query (30-60 s) | M:307→M:1624 `session.recommend_clearing_strategy` |
| GetSuggestRationale | 521 | Reads | Query | M:311→M:1585 `session.cutter_op_profile` |
| InspectModel | 525 | Reads | Query | M:315→M:1943 `session.models` |
| InspectStock | 526 | Reads | Query | M:319→M:2131 `session.stock_config` |
| InspectMachine | 527 | Reads | Query | M:323→M:2255 `session.machine` |
| InspectBrepFaces | 528 | Reads | Query | M:327→M:2467 `session.models` |
| InspectCollisions | 535 | Reads | Query(viz) | M:331→M:2166 `state.simulation` only |
| InspectSpans | 541 | Reads | Query | M:335→M:2086 `session.toolpath_configs` |
| AddAlignmentPin | 556 | Mutations (555) | Command | M:355→M:2590 `session.add_alignment_pin`; arm M:357 also pushes `SwitchWorkspace` |
| RemoveAlignmentPin | 561 | Mutations | Command | M:362→M:2622 `session.remove_alignment_pin` |
| ImportModel | 564 | Mutations | Comp→Cmd (h:stock_mut) | M:424→M:3017 `controller.import_{stl,svg,dxf,step}_path`; `io.rs:18,54` `stock_mut()` |
| AddSetup | 567 | Mutations | Comp→Cmd (h:find_setup_by_id_mut) | M:373→M:2808 `controller.handle_add_setup`; hatch M:2825 |
| SetSetupFace | 570 | Mutations | Command | M:381→M:2881 `session.set_setup_face`; arm M:386 pushes `SwitchWorkspace` |
| MoveToolpathToSetup | 574 | Mutations | Comp→Cmd | M:413→M:2990 pushes `AppEvent::MoveToolpathToSetup` |
| LoadProject | 578 | Mutations | Comp→Job | M:437→M:3084 `controller.open_job_from_path`; `io.rs:314` submits compute |
| SaveProject | 583 | Mutations | Comp→Cmd | M:451→M:3138 `controller.save_job_to_path` → `io.rs:300 session.save` |
| ExportGcode | 586 | Mutations | Command (h:wizard_mut) | M:457→M:3180 `wizard_mut()` then exports from `&state.session` |
| SetToolpathParam | 596 | Mutations | **Command** (template) | M:491→M:3474 `session.apply(Command::SetToolpathParam)` — WP1 has LANDED here |
| SetToolParam | 601 | Mutations | Command | M:610→M:3919 `session.set_tool_param` |
| AddToolpathViaGui | 609 | Mutations | **Composition** | M:531→M:3601 `handle_internal_event(AppEvent::AddToolpath)` |
| GetNotifications | 614 | Mutations | Query(viz) | M:549→M:3705 `controller.notifications()`, never touches session |
| SetToolpathTool | 622 | Mutations | Command | M:556→M:3745 `session.set_toolpath_tool` |
| SetToolpathModel | 629 | Mutations | Command | M:575→M:3804 `session.set_toolpath_model` |
| SetToolpathHeights | 636 | Mutations | Command | M:592→M:3882 `session.set_heights_config` |
| AddToolpath | 644 | Mutations | Command | M:642→M:4048 `session.add_toolpath` |
| RemoveToolpath | 651 | Mutations | Command | M:673→M:4093 `session.remove_toolpath` |
| AddTool | 658 | Mutations | Command | M:681→M:4155 `session.add_tool` |
| AddToolFromLibrary | 663 | Mutations | Command | M:689→M:1203 `session.add_tool` |
| RemoveTool | 667 | Mutations | Command | M:697→M:4203 `session.remove_tool` |
| SetSetupRotation | 671 | Mutations | Command | M:397→M:2938 `session.set_setup_rotation`; arm M:402 pushes `SwitchWorkspace` |
| SetStockConfig | 677 | Mutations | Command | M:708→M:4323 `session.set_stock_config` |
| SetMachineKinematics | 682 | Mutations | Command (h:machine_mut) | M:701→M:4521 `machine_mut().kinematics`; M:4533 pushes `MachineChanged` |
| SetBoundaryConfig | 685 | Mutations | Command | M:722→M:4643 `session.set_boundary_config` |
| SetRestAnalysisConfig | 700 | Mutations | Command | M:740→M:4763 `session.set_rest_analysis_config` |
| PlanMultitoolFinishing | 717 | Mutations | Comp→Cmd | M:764→M:4809 `controller.apply_multitool_plan` → `planner.rs:48 session.plan_multitool_finishing` |
| PreviewTierMap | 726 | Mutations | **Query** | M:768→M:5039 reads session only; its own doc B:727 says "modify NOTHING" |
| SetDressupConfig | 729 | Mutations | Command | M:772→M:5193 `session.set_dressup_config` |
| SetDressupField | 733 | Mutations | Command | M:776→M:5230 `session.set_dressup_field` |
| SetToolpathEnabled | 738 | Mutations | Command | M:780→M:5263 `session.set_toolpath_enabled` |
| SetStockSource | 742 | Mutations | Command | M:784→M:5299 `session.set_stock_source` |
| SetSpindleStrategy | 749 | Mutations | Command (h:post_mut) | M:788→M:5353 `post_mut().spindle_strategy` |
| ApplyFeeds | 757 | Mutations | Comp→Cmd (h:toolpath_configs_mut) | M:792→M:5418 `controller.apply_feeds_recommendation` → `events/mod.rs:875` hatch |
| GenerateToolpath | 763 | Compute (762) | Job (h:toolpath_configs_mut) | M:798→M:5492 `toolpath_configs_mut()`, then M:5500 pushes `AppEvent::GenerateToolpath` |
| GenerateAll | 768 | Compute | Comp→Job | M:817→M:5524 `controller.mcp_start_generate_all` (`compute.rs:1647` hatch) |
| RunSimulation | 781 | Compute | Job | M:827→M:5539 writes `state.simulation.resolution`; M:5553 pushes `AppEvent::RunSimulation` |
| CollisionCheck | 784 | Compute | Job | M:836→M:5573 pushes `AppEvent::RunCollisionCheck` |
| SimJumpToMove | 789 | Scrubbing (788) | UiCommand | M:842→M:6356 pushes `AppEvent::SimJumpToMove`, no session |
| SimJumpToStart | 792 | Scrubbing | UiCommand | M:849→M:6368 pushes `AppEvent::SimJumpToStart` |
| SimJumpToEnd | 793 | Scrubbing | UiCommand | M:856→M:6380 pushes `AppEvent::SimJumpToEnd` |
| SimScrubToolpath | 794 | Scrubbing | UiCommand | M:863→M:6408 pushes `AppEvent::SimJumpToMove` |
| SimJumpToToolpathStart | 798 | Scrubbing | UiCommand | M:870→`mcp_sim_scrub_toolpath(index, 0.0)`, same push |
| SimJumpToToolpathEnd | 801 | Scrubbing | UiCommand | M:877→`mcp_sim_scrub_toolpath(index, 100.0)`, same push |
| ScreenshotSimulation | 806 | Screenshots (805) | UiCommand | M:886→M:5588 renders viewport, `session.stock_bbox` read only |
| ScreenshotToolpath | 813 | Screenshots | UiCommand | M:902→M:5837 renders viewport, `session.toolpath_configs` read only |
| ReachMap | 831 | Screenshots | **Query** | M:924→M:5741 `session.reach_map_spec`, core measurement |
| ScreenshotGui | 838 | Screenshots | UiCommand | M:928→M:6000 `egui::ViewportCommand::InnerSize` / deferred capture |
| SetUiView | 850 | UI navigation (846) | UiCommand | M:939→M:6145 workspace / overlays / selection writes, no session write |
| ImportMachineSettings | 863 | UI navigation | **Command** (h:machine_mut) | M:957→M:2372 `session.machine_mut()`; M:2380 pushes `MachineChanged` |
| ListMachineLibrary | 868 | UI navigation | **Query**(viz) | M:964→M:2409 per-user library dir, no session |
| LoadMachineFromLibrary | 871 | UI navigation | **Command** (h:machine_mut) | M:968→M:2457 `*session.machine_mut() = profile`; M:2458 pushes `MachineChanged` |
## 2. `McpRequestKind` counts, and the plan check
Sections: Reads 24 (`B:443`), Mutations **34** (`B:555-757`), Compute 4 (`B:762`), Scrub 6 (`B:788`), Screenshots 4 (`B:805`), UI nav 4 (`B:846`) = **76**. By kind (counted from the table's own kind column): **Query 27** (9 of them `Query(viz)`), **Command 26**, **Comp→Cmd 6**, **Job 4**, **Comp→Job 2**, **UiCommand 10**, **Composition 1** (`AddToolpathViaGui`) = 76. Nine rows write a hatch: 5 `Command`, 3 `Comp→Cmd`, 1 `Job`.
| plan claim (IMPLEMENTATION_PLAN §4 WP4 / WP13) | verdict | note |
|---|---|---|
| Mutations section = 34 variants, `:555-761` | **AGREE** on 34 | lines here are `B:555-757`, next header `B:762` |
| 10 `UiCommand` rows = 6 scrub + 3 screenshots + `SetUiView` | **AGREE** on membership | but see §5: 4 Compute rows are `Job`, not `UiCommand`, so the viz-only total is 10 |
| `ReachMap` is a `Query` | **AGREE** | `session.reach_map_spec`, M:5741 |
| `ListMachineLibrary` is a `Query` | **AGREE** with a caveat | it is a `Query` over the library DIR, not over `ProjectSession` (M:2409) |
| `ImportMachineSettings` / `LoadMachineFromLibrary` are `Command` (`machine_mut()`) | **AGREE** | M:2372 and M:2457 exactly |
| `GetNotifications` is a `Query` | **AGREE it is not a mutation; DISAGREE it is a core `Query`** | M:3705 reads `controller.notifications()`, viz state — see Open calls |
| `AddToolpathViaGui` is the hand-written composition | **AGREE** | M:3601 |
| "34 arms become one" is the ceiling | **the mechanical number is 17** | rule: kind `Command`, calls a session method directly, writes no hatch, pushes no `AppEvent`. 26 `Command` rows − 5 hatch (`ExportGcode`, `SetMachineKinematics`, `SetSpindleStrategy`, `ImportMachineSettings`, `LoadMachineFromLibrary`) − 4 `AppEvent` pushers (`AddAlignmentPin`, `SetSetupFace`, `SetSetupRotation`, `SetToolpathParam`) = **17**, and `SetToolpathParam` is already converted |
## 3. `AppEvent` (`crates/rs_cam_viz/src/ui/mod.rs:57-441`, 133 variants)
`E` = `controller/events/mod.rs`; `I` = `app/input.rs`; `md`/`tp`/`io`/`sim`/`cp`/`un`/`pl` = `controller/events/{model,toolpath,io,simulation,compute,undo,planner}.rs` (all under `crates/rs_cam_viz/src/`). **31 variants are handled in `I:13-394`, not in `controller/events/`** — `E:421-452` is a `=> {}` pass-through for exactly those; nine `wizard_mut()` writes and one `setups_mut()` write live in `I`.
| variant | ui/mod.rs | section | kind | evidence |
|---|---|---|---|---|
| ImportStl | 59 | File (58) | Comp→Cmd (h:stock_mut) | E:18→`io.rs:18` `session.add_model` + `stock_mut()` |
| ImportSvg | 60 | File | Comp→Cmd | E:23→`io.rs:34` `session.add_model` |
| ImportDxf | 61 | File | Comp→Cmd | E:28→`io.rs:44` `session.add_model` |
| ImportStep | 62 | File | Comp→Cmd (h:stock_mut) | E:34 `{}`; I:28→`io.rs:54` `add_model` + `stock_mut()` |
| RescaleModel | 63 | File | Comp→Cmd (h:models_mut,stock_mut) | E:35→`io.rs:70` `invalidate_model` + both hatches |
| RemoveModel | 64 | File | Comp→Cmd | E:40→`md:590` `session.remove_model` |
| ReloadModel | 65 | File | Comp→Cmd (h:models_mut) | E:41→`io.rs:138` `invalidate_model` + hatch |
| RelinkModel | 72 | File | Comp→Cmd (h:models_mut) | E:46→`io.rs:209` `invalidate_model` + hatch |
| ExportGcode | 73 | File | UiCommand | I:132 sets `show_preflight`, no session |
| ExportCombinedGcode | 74 | File | Query→file | I:243 `export_combined_gcode_from_session(&state.session)`, read only |
| ExportSetupGcode | 75 | File | Query→file | I:279 `export_setup_gcode_from_session(&state.session)` |
| ExportSetupSheet | 76 | File | Query→file | I:335 `controller.export_setup_sheet_html()` |
| ExportSvgPreview | 77 | File | Query→file | I:333→`io.rs:497` `session.toolpath_configs` |
| SaveJob | 78 | File | Comp→Cmd | I:357→`io.rs:300` `session.save` + `set_post_config` |
| OpenJob | 79 | File | Comp→Job | I:377 `open_job_interactive` → `io.rs:314` submits compute |
| SetGeneratorTraceCaptureAll | 82 | File | Command (h:toolpath_configs_mut) | E:414 `session.toolpath_configs_mut()` |
| Select | 85 | Selection/view (84) | UiCommand | E:54→`handle_select`, `state.selection` |
| SetViewPreset | 86 | Selection/view | UiCommand | I:40 `camera.set_preset` |
| ToggleProjection | 87 | Selection/view | UiCommand | I:41 `camera.toggle_projection` |
| ClearIsolation | 88 | Selection/view | UiCommand | E:141 `state.viewport.isolate_toolpath` |
| PreviewOrientation | 89 | Selection/view | UiCommand | I:42 `camera.pitch/yaw` |
| ResetView | 90 | Selection/view | UiCommand | I:69 `fit_camera_to_first_model` |
| AddTool | 93 | Tools (92) | Comp→Cmd | E:55→`md:67` `session.add_tool` |
| AddToolFromLibrary | 96 | Tools | Comp→Cmd | E:56→`md:77` `session.add_tool` |
| DuplicateTool | 97 | Tools | Comp→Cmd | E:57→`md:222` `session.add_tool` |
| RemoveTool | 98 | Tools | Comp→Cmd | E:58→`md:240` `session.remove_tool` |
| OpenToolLibrary | 103 | ToolLib modal (100) | UiCommand | E:61 `open_tool_library`, library dir snapshot |
| CloseToolLibrary | 105 | ToolLib modal | UiCommand | E:62 `state.tool_library_modal = None` |
| DeleteLibraryTool | 108 | ToolLib modal | **file-store** | E:63 writes per-user catalog file, never session |
| UpdateLibraryTool | 114 | ToolLib modal | **file-store** | E:66 `update_library_tool`, catalog file |
| MoveLibraryTool | 120 | ToolLib modal | **file-store** | E:71 `move_library_tool`, catalog files |
| CreateToolCatalog | 126 | ToolLib modal | **file-store** | E:74 `create_tool_catalog` |
| DeleteToolCatalog | 128 | ToolLib modal | **file-store** | E:75 `delete_tool_catalog` |
| RenameToolCatalog | 130 | ToolLib modal | **file-store** | E:76 `rename_tool_catalog` |
| DedupeToolCatalog | 135 | ToolLib modal | **file-store** | E:77 `dedupe_tool_catalog` |
| OpenMachineLibrary | 139 | MachineLib modal (137) | UiCommand | E:80 `state.machine_library_open = true` |
| CloseMachineLibrary | 141 | MachineLib modal | UiCommand | E:84 `state.machine_library_open = false` |
| ImportMachineFromLibrary | 144 | MachineLib modal | Comp→Cmd (h:machine_mut) | E:85→`md:180` `machine_mut()` + `invalidate_machine` |
| SaveMachineToLibrary | 146 | MachineLib modal | **file-store** | E:86→`md:192` reads `session.machine`, writes library file |
| DeleteMachineFromLibrary | 148 | MachineLib modal | **file-store** | E:87 library file |
| RenameMachineInLibrary | 150 | MachineLib modal | **file-store** | E:88 library file |
| AddSetup | 156 | Setups (155) | Comp→Cmd | E:92→`md:272` `session.add_setup` |
| RemoveSetup | 157 | Setups | Comp→Cmd | E:94→`md:421` `session.remove_setup` — **ZERO emitters, see §4** |
| RenameSetup | 158 | Setups | Comp→Cmd | E:95→`md:458` `session.rename_setup` |
| SetupTwoSided | 160 | Setups | Comp→Cmd (h:stock_mut) | E:93→`md:327` `session.add_setup` + `stock_mut()` |
| AddFixture | 163 | Fixtures (162) | Comp→Cmd | E:96→`md:471` `session.add_fixture` |
| RemoveFixture | 164 | Fixtures | Comp→Cmd | E:97→`md:511` `session.remove_fixture` |
| AddKeepOut | 165 | Fixtures | Comp→Cmd | E:100→`md:532` `session.add_keep_out` |
| RemoveKeepOut | 166 | Fixtures | Comp→Cmd | E:101→`md:567` `session.remove_keep_out` |
| FixtureChanged | 167 | Fixtures | UiCommand (notify) | E:104 `state.gui.mark_edited()` only — fired AFTER a panel write |
| AddToolpath | 170 | Toolpaths (169) | Comp→Cmd | E:110→`tp:12` `session.add_toolpath` |
| DuplicateToolpath | 171 | Toolpaths | Comp→Cmd | E:111→`tp:180` `session.add_toolpath` |
| RemoveToolpath | 172 | Toolpaths | Comp→Cmd | E:131→`tp:340` `session.remove_toolpath` |
| MoveToolpathUp | 173 | Toolpaths | Comp→Cmd | E:112→`tp:236` `session.reorder_toolpath` |
| MoveToolpathDown | 174 | Toolpaths | Comp→Cmd | E:113→`tp:256` `session.reorder_toolpath` |
| ReorderToolpath | 178 | Toolpaths | Comp→Cmd | E:114→`tp:286` `session.reorder_toolpath` |
| MoveToolpathToSetup | 183 | Toolpaths | Comp→Cmd | E:117→`tp:315` `session.move_toolpath_to_setup` |
| ToggleToolpathEnabled | 184 | Toolpaths | Command | E:120 `session.set_toolpath_enabled` inline |
| GenerateToolpath | 185 | Toolpaths | Comp→Job | E:132→`cp:181` `submit_toolpath_compute` |
| GenerateAll | 186 | Toolpaths | Comp→Job | E:133→`tp:377` `handle_generate_all` |
| ToggleToolpathVisibility | 187 | Toolpaths | UiCommand | E:134 `state.gui.toolpath_rt` |
| ToggleIsolateToolpath | 188 | Toolpaths | UiCommand | E:140 `state.viewport.isolate_toolpath` |
| InspectToolpathInSimulation | 189 | Toolpaths | UiCommand | E:145 viewport + playback only |
| RunSimulation | 192 | Simulation (191) | Comp→Job | E:150 `run_simulation_with_all` → `sim:314` |
| RunSimulationWith | 193 | Simulation | Comp→Job | E:153→`sim:335` submits compute |
| ResetSimulation | 194 | Simulation | UiCommand | E:157 `handle_reset_simulation`, cancels lane |
| ToggleSimPlayback | 195 | Simulation | UiCommand | E:154 `state.simulation.playback.playing` |
| SwitchWorkspace | 198 | Workspace nav (197) | UiCommand | I:72 `overlays::registry::switch_workspace(state_mut())` |
| SimStepForward | 199 | Workspace nav | UiCommand | E:159 playback index |
| SimStepBackward | 200 | Workspace nav | UiCommand | E:160 / I:78 playback index |
| SimJumpToStart | 201 | Workspace nav | UiCommand | E:161 / I:86 playback index |
| SimJumpToEnd | 202 | Workspace nav | UiCommand | E:162 playback index |
| SimJumpToMove | 203 | Workspace nav | UiCommand | E:158 / I:94 playback index |
| SimJumpToOpStart | 204 | Workspace nav | UiCommand | E:163 / I:105 playback index |
| SimJumpToOpEnd | 205 | Workspace nav | UiCommand | E:164 playback index |
| ExportGcodeConfirmed | 208 | Pre-flight/Export (207) | Comp→Job | I:137 `export_gcode_with_summary()` |
| OpenExportWizard | 210 | Pre-flight | UiCommand | I:140 reads `session.wizard()`, writes `state.show_export_wizard` |
| CloseExportWizard | 213 | Pre-flight | UiCommand | I:153 `state.show_export_wizard = false` |
| WizardSetStep | 215 | Pre-flight | Command (h:wizard_mut) | I:160 `session.wizard_mut().last_step_visited` |
| WizardSetPost | 217 | Pre-flight | Command | I:230 `session.set_post_config(session_post)` — a setter, no hatch |
| WizardSetOutputLayout | 219 | Pre-flight | Command (h:wizard_mut) | I:214 `wizard_mut().output_layout` |
| WizardSetFilenameTemplate | 223 | Pre-flight | Command (h:wizard_mut) | I:219 `wizard_mut().filename_template` |
| WizardSetWcsOverride | 225 | Pre-flight | Command (h:wizard_mut) | I:164 `wizard_mut().wcs_override` |
| WizardSetUnitsOverride | 227 | Pre-flight | Command (h:wizard_mut) | I:169 `wizard_mut().units_override` |
| WizardSetSafeZOverride | 230 | Pre-flight | Command (h:wizard_mut) | I:174 `wizard_mut().safe_z_override` |
| WizardSetDryRun | 234 | Pre-flight | Command (h:wizard_mut) | I:179 `wizard_mut().dry_run` |
| WizardSetSpindleWarmup | 236 | Pre-flight | Command (h:wizard_mut) | I:184 `wizard_mut().spindle_warmup_secs` |
| WizardSetToolChangeMode | 240 | Pre-flight | Command (h:wizard_mut) | I:189 `wizard_mut().tool_change_override` |
| WizardSetSetupPauseMessage | 246 | Pre-flight | Command (h:setups_mut) | I:194 `session.setups_mut()` |
| WizardSetAllowValidatorErrors | 252 | Pre-flight | Command (h:wizard_mut) | I:206 `wizard_mut().allow_validator_errors` |
| WizardSave | 256 | Pre-flight | Comp→Cmd | I:211 `handle_wizard_save()` |
| SetToolLoadOverride | 260 | Pre-flight | UiCommand | I:232 `state.gui` override flags only |
| SetStaleExportPolicy | 269 | Pre-flight | UiCommand | I:240 `state.gui.stale_export` |
| OpenOptimizeModal | 276 | Optimize (271) | Comp→Job | E:262→`mod:467` takes session, submits optimize |
| CloseOptimizeModal | 278 | Optimize | UiCommand (restores session) | E:265-275; the session was taken by `std::mem::replace` at E:517 (`open_optimize_modal`) |
| ApplyOptimizeCandidate | 283 | Optimize | Comp→Cmd | E:277→`mod:540` `session.apply_toolpath_param_snapshot` |
| ReoptimizeWithAxisOverride | 295 | Optimize | Comp→Job | E:283→`mod:676` re-submits with overrides |
| OpenFeedsModal | 306 | Feeds (301) | Query→UiCommand | E:328→`mod:807` reads `session.toolpath_configs` |
| CloseFeedsModal | 308 | Feeds | UiCommand | E:331 `state.feeds_modal = None` |
| SetFeedsModalMode | 311 | Feeds | UiCommand | E:334 `state.feeds_modal` field |
| SetSpindleStrategy | 317 | Feeds | Command (h:post_mut) | E:339-349 `session.post_mut().spindle_strategy` |
| ApplyFeedsAll | 327 | Feeds | Comp→Cmd (h:toolpath_configs_mut) | E:356→`mod:875` funnel + `invalidate_toolpath_inputs` |
| SetDropCutterScallopHeight | 332 | Feeds | Command (h:toolpath_configs_mut) | E:359→`mod:1013` hatch write |
| ApplyFeedsProject | 338 | Feeds | Comp→Cmd (h:toolpath_configs_mut) | E:362→`mod:1058`→`mod:875` funnel |
| ToggleFeedsProvenance | 340 | Feeds | UiCommand | E:351 `state.feeds_modal` field |
| ApplyFeedsExplore | 344 | Feeds | Comp→Cmd (h:toolpath_configs_mut) | E:365→`mod:875` funnel |
| SetFeedsProjectSort | 350 | Feeds | UiCommand | E:372 `state.feeds_modal` field |
| SetFeedsExplore | 354 | Feeds | UiCommand | E:377 `state.feeds_modal` field |
| ToggleFeedsProjectRow | 356 | Feeds | UiCommand | E:382 `state.feeds_modal` field |
| ApplyFeedsProjectSelected | 358 | Feeds | Comp→Cmd (h:toolpath_configs_mut) | E:389→`mod:1078`→`mod:875` funnel |
| SetFeedsProjectScatter | 360 | Feeds | UiCommand | E:392 `state.feeds_modal` field |
| SetFeedsProjectSelectAll | 362 | Feeds | UiCommand | E:397 `state.feeds_modal` field |
| OpenOptimizeProject | 369 | Optimize project (364) | Comp→Job | E:292→`mod:1164` submits |
| CloseOptimizeProject | 372 | Optimize project | UiCommand (restores session) | E:295; taken by `std::mem::replace` at E:1185 |
| ToggleOptimizeProjectRow | 375 | Optimize project | UiCommand | E:302 `state.optimize_project` field |
| ApplyOptimizeProject | 379 | Optimize project | Comp→Cmd | E:309→`mod:1207` `apply_toolpath_param_snapshot` |
| OpenMultitoolPlanner | 384 | Multitool (381) | UiCommand | E:314 `open_multitool_planner` |
| PreviewMultitoolPlan | 388 | Multitool | Comp→Job | E:317→`pl:158` `request_multitool_preview` |
| ApplyMultitoolPlan | 392 | Multitool | Comp→Cmd | E:320→`pl:195`→`pl:48` `session.plan_multitool_finishing` |
| CloseMultitoolPlanner | 396 | Multitool | UiCommand | E:323 `close_multitool_planner` |
| RunCollisionCheck | 399 | Collision (398) | Comp→Job | E:167→`sim:393` submits collision compute |
| CancelCompute | 402 | Compute (401) | **lane control** | E:168 `self.compute.cancel_all()`, no session |
| CancelToolpathGeneration | 407 | Compute | **lane control** | E:169 cancels toolpath lane only |
| ToggleFaceSelection | 410 | Face selection (409) | Command | E:175-201 `session.set_face_selection(idx, ..)` inline |
| ToggleDrillTarget | 417 | Drill target (416) | Command | E:203-241 `session.set_drill_selected_holes(..)` inline |
| StockChanged | 423 | Edit (422) | Comp→Cmd (notify) | E:247→`md:630` `session.invalidate_stock` + `update_stock_from_bbox` |
| StockMaterialChanged | 424 | Edit | UiCommand (notify) | E:253 `mark_edited()` only — the write happened in the panel |
| HeightPlanesChanged | 432 | Edit | UiCommand (notify) | E:248 `pending_upload = true` only |
| MachineChanged | 433 | Edit | Command (notify) | E:256 `session.invalidate_machine()` |
| Undo | 434 | Edit | Comp→Cmd (h:tools_mut) | E:243→`un:8` `set_machine`/`set_post_config`/`set_stock_config`; `un:105` `tools_mut()` |
| Redo | 435 | Edit | Comp→Cmd (h:tools_mut) | E:244→`un:55`, same three setters + `tools_mut()` |
| ShowShortcuts | 438 | Help (437) | UiCommand | I:390 `state.show_shortcuts = true` |
| Quit | 440 | Help | UiCommand | I:394 `ViewportCommand::Close` |

**`AppEvent` counts** (133, counted from the table's kind column): **UiCommand 46**, **Comp→Cmd 40**, **Command 19**, **Comp→Job 11**, **file-store 10** (tool/machine library files, no `ProjectSession`), **Query→file 4**, **lane control 2**, **Query→UiCommand 1**. 27 rows write a hatch: 14 `Command` (12 of them `wizard_mut`/`setups_mut` in `app/input.rs`), 13 `Comp→Cmd`.
Core-bound (`Command`+`Comp→Cmd`+`Comp→Job`) = **70**; non-core (`UiCommand`+file-store+`Query→file`+lane control) = **63**.
Against `RULING:66-69`'s "~89 project mutations / ~44 GUI-only": **DISAGREE — 70 / 63**, a delta of **19** rows the ruling's reading puts on the mutation side: the 10 file-store library rows, the 4 read-only export rows, the 2 lane-control rows, and 3 notify-only rows (`StockMaterialChanged`, `HeightPlanesChanged`, `FixtureChanged`).
## 4. `RemoveSetup` — zero emitters, confirmed
- Declared `crates/rs_cam_viz/src/ui/mod.rs:157`; handled `crates/rs_cam_viz/src/controller/events/mod.rs:94` → `handle_remove_setup` (`controller/events/model.rs:421`, reaches `session.remove_setup` and `session.remove_toolpath`).
- `rg -n "RemoveSetup" crates/` returns exactly 6 hits: the declaration, the handler, and **4 test sites** — `crates/rs_cam_viz/src/controller/tests.rs:1594, 1602, 1796, 1869`. No production emitter exists; WP13's "give it an emitter or delete it" stands.
## 5. Mismatches with the plan
1. **HEAD is `2b31b7e5`, not `dbf572b9`** (the task's) and not `bfe8586b` (the plan's). All plan `app/mcp.rs` line numbers are ~10-12 low.
2. **WP1 has already landed at `mcp_set_toolpath_param`** (`M:3474` is `session.apply(Command::SetToolpathParam)` with a "WP1: one door" comment). WP4's premise "34 classified arms to rewrite" is now 33, of which 17 are mechanical (§2).
3. **`mcp_apply_stale` has 14 call sites, not 15**: `M:2886,2943,3660,3749,3808,3886,3924,4325,4647,4767,5197,5234,5303,5429`. The plan's 15th (`:3464`, `mcp_set_toolpath_param`) is gone with item 2.
4. **A SEVENTH hatch site in `app/mcp.rs` that WP4 does not own**: `toolpath_configs_mut()` at **`M:5492`**, inside `mcp_generate_toolpath` (it force-enables `debug_options`). The plan's WP4 "Owns these hatch sites" lists six — `2372, 2457, 2815, 3170, 4509, 5341` → here `2372, 2457, 2825, 3180, 4521, 5353`. The same write recurs at `controller/events/compute.rs:1647` (`mcp_start_generate_all`). Nothing in WP4-WP7 claims either.
5. **`PreviewTierMap` is a third non-homogeneous Mutations row.** WP4 names only `GetNotifications` and `AddToolpathViaGui`; `B:726` says "modify NOTHING" and `M:5039` reads only.
6. **`OptimizeToolpath` is a mutation sitting in the Reads section** (`B:477`, `M:3441` hands `&mut ...session` to `optimize_toolpath`, 1-2 min blocking). The plan's "Reads 24" counts it as a read.
7. **Nine `wizard_mut()` writes and one `setups_mut()` write are in `app/input.rs`** (`I:164,169,174,179,184,189,206,214,219` and `I:194`), not in `controller/`. WP6b must reach `app/input.rs`.
8. **Five MCP `Command` arms push an `AppEvent` as a side effect** (`SwitchWorkspace` on `AddAlignmentPin`/`SetSetupFace`/`SetSetupRotation`/`SetToolpathParam`; `MachineChanged` on the three machine writers). WP4's "one delegating arm" must preserve them or the operator's screen stops following the mutation. Worse, `MoveToolpathToSetup` (M:2990) ONLY dispatches — its reply is sent before the mutation runs next frame, and it is the one Mutations row absent from the 14 `mcp_apply_stale` sites.
9. **31 `AppEvent` variants have no arm in `controller/events/*.rs`** (`E:421-452` is a `=> {}` pass-through); WP13's sentry must look in `app/input.rs` too or it will report those as unreached.
## 6. Open classification calls
1. Five reads core cannot answer: `GetNotifications` (`M:3705`, `controller.notifications()`) and `GetCutTrace`/`GetDiagnostics`/`GetToolLoadReport`/`InspectCollisions` (all read `state.simulation`, the viz sim slot, not `session.simulation`; `M:3184-3197` documents the split). `Query` with a viz backing store, or a fifth kind?
2. `OptimizeToolpath` (`B:477`) — a `&mut session` mutation with a 1-2 min body: `Command` (it is synchronous today) or `Job` (its duration and its cancel flag say otherwise)?
3. `RecommendClearingStrategy` (30-60 s) and `PreviewTierMap` (8-31 s) — pure reads that block the frame loop: `Query` or `Job`?
4. `ListToolLibrary` / `ListToolCatalog` / `ListMachineLibrary` and the **10 `AppEvent` file-store rows** — they read and write per-user library FILES and never touch `ProjectSession`. None of the four kinds covers a non-session aggregate.
5. `CancelCompute` / `CancelToolpathGeneration` (`E:168,169`) — lane control, no session. `UiCommand`, or a fifth `JobControl` kind alongside the existing off-channel `cancel_generation`?
6. The five post-write notification events — `StockChanged`, `MachineChanged`, `StockMaterialChanged`, `HeightPlanesChanged`, `FixtureChanged`. Each fires AFTER a hatch write done in egui draw code; they are the "description supplied after the write" pattern the ruling targets. They should CEASE TO EXIST once the panel write returns `Effects` — classify them now, or delete them in WP6?
7. `CloseOptimizeModal` / `CloseOptimizeProject` (`E:265-275`, `:295`) — restore a `ProjectSession` the optimizer took by value (`std::mem::replace`, E:517 / E:1185; `planner.rs:176` does the same). That violates the `Job` shape ("neither step holds `&mut` session"). Which kind, and does the optimizer become a real `Job` first?
8. `WizardSetPost` (`I:224`) — clones `session.post_config()`, edits, writes back. Is the target `Command::SetPostConfig`, or does it join the `wizard_mut()` family?
9. `ExportCombinedGcode` / `ExportSetupGcode` / `ExportSetupSheet` / `ExportSvgPreview` — read-only over the session but write a FILE. `Query` with a side effect, or `Command` because the artifact is the product?
