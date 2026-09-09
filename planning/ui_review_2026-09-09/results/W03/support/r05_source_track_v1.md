# R05 source track — support agent report (v1, received by message 2026-09-09 16:36 +12)

Read-only Claude Code `Explore` subagent `r05-source`. Saved verbatim by the orchestrating
session `rs-cam-b9`. **Leads, not accepted findings** until the R05 reviewer verifies them.
The orchestrator spot-checked: `draw_rest_badge` ignores order/enabled/model (toolpath_panel.rs:722-733) ✔;
`ToggleToolpathEnabled` handler calls only `set_toolpath_enabled` (events/mod.rs:114-118) ✔;
`boundary_inherit` has no generation reader ✔.

---

WORKING-TREE NOTE. Five files carry uncommitted edits by another developer: crates/rs_cam_viz/src/ui/multitool_planner.rs (+128), crates/rs_cam_core/src/session/multitool.rs (+207), crates/rs_cam_core/src/tier_islands.rs (+251), crates/rs_cam_core/src/tier_map_cache.rs (+16), crates/rs_cam_core/src/rest_heatmap_mesh.rs (+6). The edits add the G-OVERLAPFILL band advisory, the per-tier "machines" column and the summary_line under the planner buttons. Every line quoted from those files is working-tree text, not HEAD.

## 1. Vocabulary map

Six terms, six different problems. Only three have a GUI home an operator can find by browsing.

- **REST MACHINING** (`OperationType::Rest`): "+ Add" menu, 2.5D group; inspector "Previous tool" picker; row badge "dep"/"no dep" (toolpath_panel.rs:713-771). Problem: a smaller tool clears corners a NAMED larger tool could not enter. Input: `RestConfig::prev_tool_id` + an earlier enabled op in the same setup on the same model using that tool (properties/operations/mod.rs:1954-1986).
- **USE REMAINING STOCK** (`StockSource::FromRemainingStock`): inspector header checkbox "Use remaining stock" (properties/mod.rs:3951-3966); HIDDEN for Pencil (3942-3948). Hover: "When enabled, prior operations in this setup are simulated to determine remaining material…". Input: a dexel snapshot from a prior SIMULATION (controller/events/compute.rs:560-565).
- **REST ANALYSIS** (`RestAnalysisConfig`): Geometry tab section "Rest Analysis", checkbox "Compute rest heatmap (material left after this op)" (properties/mod.rs:4506-4523); demand-driven "Producing rest regions for: {names}" (4525-4536); "Reference:" combo default "Self / machined stock" (4542-4573).
- **DERIVED REST REGIONS** (`BoundarySource::DerivedRestRegions`): Machining Boundary Source "Rest Regions" + "Rest source:" combo (4302-4333, 4345-4385). Overlays row "Derived rest regions".
- **TIER MAP** (`BoundarySource::PlannedTierRegions`): menu Toolpath > "Plan multi-tool finishing..." (menu_bar.rs:183-203); TERRITORY table; Overlays row "Tier map" (overlays/registry.rs:807-843).
- **REACH MAP** (reach_map.rs): inspector "Show reach map" (properties/mod.rs:3690-3698); Overlays "Model colour: Reach" (registry.rs:903-940), ON by default in Toolpaths. Top-down only.

Findings: (a) "Rest" names three unrelated mechanisms and nothing in the GUI states the distinction. (b) "Use remaining stock" and "rest machining" are separate controls: a Rest op does NOT set stock_source (controller/events/toolpath.rs:157 gives every new op Fresh); generate_all.rs:34-41 calls the stock_source set `rest_op_indices` and the MCP refusal calls them "rest-machining operation(s)" (events/compute.rs:1565). (c) Two of the six Regions overlays have no renderer: "Derived rest regions" and "Boundary outline" refuse with "not drawn yet (no renderer)" (registry.rs:870-895); "Planner islands" refuses (844-869).

## 2. Operator-visible dependency graph

Setup headers only when >1 setup (toolpath_panel.rs:58,63), with ready/total count (72-87). Rows are cards (362-556). Drag and drop per setup (98); a drop inside the source setup emits ReorderToolpath; in another emits MoveToolpathToSetup (152-162). **Indent, arrows and grouping by dependency do not exist.**

The one dependency badge is for Rest ops only (466-469 → `draw_rest_badge`): "dep" green/yellow, "no dep" red. **DEFECT — badge and validator disagree**: `draw_rest_badge` (722-733) accepts ANY other toolpath in the setup whose tool_id matches prev_tool_id — no plan order, enabled or model_id check; `has_prior_rest_source` (operations/mod.rs:1974-1978) checks all three. A green "dep" can sit on an op the inspector refuses to generate.

**A FromRemainingStock op gets no badge at all.** `AwaitingPriorStock` renders as chip "WAIT" (WARNING colour, hover = block.message; 404-406, 416-418) and inspector "Waiting on upstream stock" (properties/mod.rs:3641-3652). `prior_stock_blocker` (events/compute.rs:76-163) builds three message shapes naming the blocker. **The graph is entirely in hover text**, after a failed generate, one predecessor deep.

## 3. Generate All prerequisite

Button toolpath_panel.rs:43, menu_bar.rs:168-171, Shift+G (shortcuts_window.rs:30) → `AppEvent::GenerateAll`. Path (events/toolpath.rs:347-398): `generate_all_scope` (generate_all.rs:27-43); no rest ops → submits EVERY config (351-363); else `plan_fixpoint(true, &rest_op_indices, pinned_simulation_resolution())`; pinned only when `!auto_resolution && finite && > 0` (409-413).

Refusal verbatim (events/toolpath.rs:380-393), Warning toast:
> Generate All needs a pinned simulation resolution: {N} enabled operation(s) take their stock from a simulation, so the ladder has to simulate between generate rounds. Untick "Auto from tool size" in the Simulation panel and set a resolution well below the finishing tool's TIP radius (e.g. 0.1 mm for a 1 mm ball). It is not guessed — collision counts and engagement both move with cell size.

Rendered bottom-right, max width 400, auto-dismiss after 6 s (app.rs:896-929; controller.rs:70-76). No one-click fix, no workspace switch. "Auto from tool size" sits at sim_op_list.rs:88 inside `CollapsingHeader "Setup & run"` whose `default_open` is `sim.boundaries().is_empty()` (41-49) — collapsed after the first simulation. The panel heading is "Verification", not "Simulation". The slider appears only when auto is off (73-84). `rest_op_indices` is computed then only its `len()` is printed. MCP refusal (events/compute.rs:1556-1573) names indices, the rejection reason and the `fixpoint: false` opt-out. MCP path force-enables `debug_options.enabled` on every enabled op (1596-1600); GUI does not.

**Inconsistency**: the no-chain path submits EVERY config regardless of enabled (351-361) and `submit_toolpath_compute` has no enabled gate; the chain path submits only `scope.enabled`. A disabled op reads OFF either way (compute/config.rs:75-80).

## 4. Fixpoint states

Status line via `set_status` at round start / between rounds (events/compute.rs:1626-1628, 1691-1693, 1761-1764), bottom status bar amber (app.rs:287-290, 317-320, 372-375). Completion toast from `generate_all_headline` (generate_all.rs:270-292). **Status line expires after 5 s** (controller.rs:294-299) and progress fires only at round transitions (1803-1809) — blank during any real round. `max_rounds` never shown. **No GUI cancel** for a running ladder. `Next::Simulate` WRITES `state.simulation.resolution` and unticks `auto_resolution` (1688-1690) — the MCP ladder silently re-dials the GUI control.

`tests/generate_all_fixpoint_parity.rs` pins the shared decision (plan shape, refusal wording contains "Auto from tool size" and the count), not the loop; `SilentBackend` never advances past round 1. No test covers intermediate states.

## 5. Break-and-repair semantics

Core `invalidate_output_dependents` (session/mutation.rs:176-292) is correct and transitive. **The GUI runtime does not follow.**

| Operation | Core | GUI | Seen on the card |
|---|---|---|---|
| Disable predecessor (events/mod.rs:114-118 → mutation.rs:302-318) | invalidates dependents | nothing | consumer still OK with old stats |
| Delete predecessor (events/toolpath.rs:320-339 → mutation.rs:92-129) | full invalidation | only derived-rest consumers marked stale (events/compute.rs:810-834) | FromRemainingStock consumers still OK |
| Reorder (events/toolpath.rs:236-293 → mutation.rs:137-174) | both indices invalidated | `mark_edited` only | both OK |
| Move to another setup (events/toolpath.rs:295-318 → mutation.rs:703-733) | invalidates | `pending_upload`, `mark_edited` | OK |
| Duplicate (180-234) | new op | fresh runtime, Pending | correct |

Consequences: card status/stats come from `gui.toolpath_rt` (toolpath_panel.rs:122-132, 275-280, 494-521), never `session.get_result`; preflight counts `rt.result.is_some()` as computed (readiness.rs:57-70); export prefers `session.get_result` and FALLS BACK to the GUI result (io/export.rs:129-132). **No stale marker on the card in any case.**

A FromRemainingStock op with no snapshot never falls back to fresh stock (events/compute.rs:560-578) → `AwaitingPriorStock` + Warning toast. DerivedRestRegions/PlannedTierRegions failures refuse with Error toasts (600-665, 712-738).

**Silent omission on export**: `emitted_toolpaths` returns None for an enabled op with no result (io/export.rs:123-139); whole-project export errors only when `phases.is_empty()` (239-244). A WAIT op vanishes from the program. Single-toolpath export errors correctly (354-366).

Drag-and-drop: a reorder is a **SWAP** (`toolpath_indices.swap(pf, pt)`, mutation.rs:155-162), not an insert; drop index is a linear fraction of zone height (toolpath_panel.rs:631-645); a cross-setup drop discards the index and appends (events/toolpath.rs:299, mutation.rs:726-730).

## 6. Planner path

menu_bar.rs:183-203 → `OpenMultitoolPlanner` → planner.rs:103-150 → dialog → Preview on the Optimize lane (158-183) → Apply plan (195-236) → `session/multitool.rs:304-425`. Menu gate: ≥2 tools, ≥1 mesh, Optimize lane idle.

Planner-owned fields (all editable afterwards, all overwritten on re-apply): name `"Finish tier {tier}{tag} (R{cusp:.1})"` (multitool.rs:383); operation UnifiedFinish/Scallop dialled by `plan_tier_operation` (802-945); boundary `PlannedTierRegions{…}` Center on per-island tiers (946-968), tier 0 default; `boundary_inherit` false deliberately (392-395); `stock_source` FromRemainingStock on EVERY tier (396-400); `heights.bottom_z` Manual(mesh.min.z − 0.2) (339-345); `planner_origin: Some(PlannerOrigin{plan_id, tier, tier_count})` (406-410; session/mod.rs:665-686).

**The ownership marker has no GUI surface.** Only plumbing reads (properties/mod.rs:3434, state/toolpath/entry.rs:163). MCP-only (app/mcp.rs:4331). A duplicate sheds the tag (events/toolpath.rs:214).

**Apply = REPLACE by ownership tag**: `remove_planned_toolpaths` (multitool.rs:709-740) removes EVERY op in the setup with `planner_origin.is_some()`, whatever plan_id. A hand-edited planner tier is destroyed on re-apply, no warning, no retained override. The operator is told only afterwards, in a 4-second Info toast "Planned {names} (replaced {N} prior planner op(s))" (planner.rs:220-228). Emitted ops are NOT generated (planner.rs:42-43) and all carry FromRemainingStock, so Generate All then refuses until a resolution is pinned — the dialog never says so.

Close ("Leaves the project untouched…") cancels an in-flight walk, keeps the ladder/dials/preview (planner.rs:240-258, 96-149). **Undo: none** — `UndoAction` has five variants (state/history.rs:15-42), none structural. Apply plan, Delete, reorder and cross-setup move are irreversible. The boundary Source combo shows "Planned Tier Regions" but offers only Stock / Model Silhouette / Face Selection / Rest Regions (properties/mod.rs:4258-4333) — one click destroys the tier recipe with no way back.

Preview: pure read on the Optimize lane; 150 ms debounce for island-only dials; "map: cached" / "map: rebuilding on next Preview" hint (multitool_planner.rs:575-591). Working-tree `summary_line` (735-761) adds owned vs machined area. **Nothing in the planner reports achieved surface quality**; the reach map is never offered from the planner. **Cold tier walk runs on the frame loop** (events/compute.rs:718-722; ~8 s at 0.6 mm, ~31 s at 0.3 mm per ladder tool, multitool.rs:31-34) — Generate after an un-previewed Apply freezes the GUI.

## 7. Row hidden / disabled / export

Disabled ops are NOT exported (io/export.rs:126-128, app/export.rs:177, export_wizard.rs:464-475). No collapsible setup groups exist. "Hidden" = viewport-hidden only (toolpath_row_controls.rs:34-38: "Simulation still includes it"). Disabled and hidden share the dimmed-name treatment (toolpath_panel.rs:297). `enabled` toggles from the context menu (588-592), the power glyph (row_controls 138-158) and the inspector write-back (properties/mod.rs:3453) → `set_toolpath_enabled` (events/mod.rs:114-118): core invalidates dependents; GUI does nothing.

## 8a. Expert shortcuts to preserve

Shift+G / G; overlay hotkeys incl. ", / ." colour-source stepping; click-anywhere card select with full context menu incl. Move Up/Down (the only reliable reorder); the Sim button jump (toolpath_panel.rs:472-479); planner dialog memory across Close; "map: cached" + debounce; planner optional-dial ticks "(from coarseness)"; rest-source combo "(regions ready)" sorting; `ComputeStatus::effective` as the single status resolver.

## 8b. Questions only a live test can answer

1. Does the 6-second refusal toast survive the round trip to fix it? 2. Does the status line go blank mid-round? 3. Is the stale-card divergence observable end to end (reorder → export → diff)? 4. Green "dep" on a Rest op whose predecessor is below/disabled? 5. Frame-loop freeze on a cold planner tier generate? 6. Boundary Source combo on a planner tier? 7. Cross-setup drag lands where the line says? 8. Same-setup drag swaps or inserts? 9. Planner replacement count readable in 4 s? 10. Does a WAIT op vanish silently from export?

## Defects to report separately from UX findings

1. GUI runtime cache never invalidated on reorder/disable/cross-setup move (events/toolpath.rs:236-318, events/mod.rs:114-118 vs mutation.rs:137-174, 302-318, 703-733).
2. `draw_rest_badge` ignores plan order, enabled and model_id (toolpath_panel.rs:722-733 vs operations/mod.rs:1974-1978).
3. Whole-project export silently omits an enabled op with no result (io/export.rs:123-139, 239-244).
4. Cross-setup drag discards the drop index and appends (events/toolpath.rs:299, mutation.rs:726-730).
5. PlannedTierRegions can be left but never re-entered from the Source combo; no undo (properties/mod.rs:4258-4333).
6. Generate All submits disabled toolpaths without a rest chain, enabled-only with one (events/toolpath.rs:351-363 vs 397).
7. The MCP ladder rewrites `state.simulation.resolution` and unticks `auto_resolution` silently (events/compute.rs:1688-1690).
