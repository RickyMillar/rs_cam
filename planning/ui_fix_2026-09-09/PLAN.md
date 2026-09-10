# UI/UX fix programme — phased plan for unattended agents

Date: 2026-09-09. Source: the review programme in
`planning/ui_review_2026-09-09/` — `results/R03/REPORT.md` (live findings),
`results/W03/support/*.md` (source tracks, R04/R05/R08), `results/IA/SUMMARY.md`
and `results/IA/PROPOSED_FLOW.md` (IA design changes D1–D6), and the earlier
W01/W02 checkpoints. Every task below cites the finding it closes. Line numbers
are the review's anchors at HEAD ef91cb03–4af7dd96 and MAY HAVE MOVED: locate
the symbol, not the line.

Status of each task lives in `STATUS.md` next to this file. Agents append
there; they do not rewrite it.

## 0. Rules for every agent

Read `CLAUDE.md` first. Then:

1. **One task, one worktree, one commit.** Start from `master` HEAD in a fresh
   worktree. The main checkout carries other developers' uncommitted edits
   (`dressup.rs`, `execute.rs`, `app/mcp.rs`, `multitool_planner.rs`, …); never
   work there and never touch `.mcp.json`.
2. **Gates before commit:** `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`,
   and the dev-loop tests for every crate you touched
   (`cargo test -p rs_cam_core -q`, `-p rs_cam_viz`, `-p rs_cam_mcp`, `-p rs_cam_cli`).
   Never run workspace-wide `cargo test`. Run the FULL gate
   (`cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`)
   once at the end of each phase, not per task.
3. **Every behaviour change ships a sentry test** named after the finding
   (`*_g_<tag>.rs` in `crates/*/tests/` or a unit test beside the code). The
   sentry must fail on the pre-fix code; say in the commit how you checked.
4. **Do not change numeric thresholds, gates or feeds constants.** This
   programme changes what the UI says and what routes exist, plus the listed
   correctness defects. If a task seems to need a threshold change, stop and
   write it up in `STATUS.md` instead.
5. **Research tasks produce a document, not code.** Write it to
   `planning/ui_fix_2026-09-09/research/<task-id>.md` with: the question, what
   the code does today (file:symbol), the options, a recommendation, the
   acceptance test, and what stays out of scope. An implementation task that
   depends on a research task reads that document first and follows its
   recommendation unless it finds a hard blocker, which it records.
6. **Evidence discipline:** generated ≠ simulated ≠ Within ≠ safe. When a task
   claims a fix, show the sentry, not a screenshot alone. Live MCP checks use
   the R03 scratch seeds under
   `planning/ui_review_2026-09-09/results/R03/scratch/` copied into your own
   scratch directory, never the originals.
7. **Docs:** if the visible product surface changes, update
   `FEATURE_CATALOG.md` and, for new operator-visible rules, the relevant
   `CLAUDE.md` caveat paragraph. Keep `CREDITS.md` current if you add a source.
8. **Commit message** = `fix(area): <TAG> — <one line>`, body with finding ids,
   sentry name, and the attribution line from the session reminder.
9. **When blocked, write it down and move to the next task.** Do not spend a
   night on one generator. `STATUS.md` gets: task id, state
   (done / blocked / partial), commit hash, sentry, open question.

## 1. Phase map and dependencies

```text
P0 Research (parallel, no code)      R0.1 freshness model   R0.2 ramp containment
                                     R0.3 refusal contract  R0.4 issue→edit→recheck
                                     R0.5 value roles       R0.6 planner ownership UI
                                     R0.7 missing-model repair route
P1 Clear-spec defects (parallel)     F1.1 … F1.12   (no dependency on P0)
P2 Freshness model (serial core)     F2.1 … F2.5    (needs R0.1)
P3 MCP gap fill (parallel with P2)   F3.1 … F3.5
P4 Correctness needing research      F4.1 ramp (R0.2)   F4.2 refusal contract (R0.3)
P5 IA design changes                 D0..D6 (D2 needs R0.4, D4 needs R0.5, D5 needs R0.6)
P6 Validation                        V6.1 … V6.4
```

P1 and P0 run the first night. P2/P3/P4 the second. P5 after design sign-off
on the research documents. P6 closes each phase.

## 2. Phase P0 — research (documents only)

### R0.1 One freshness model
**Question.** What does "current" mean for a toolpath, a simulation and an
export, and where is that state stored and rendered?
**Today (verified).** Three systems disagree: core `ProjectSession::results`
dropped by `invalidate_result_chain` / `invalidate_output_dependents`
(`session/mutation.rs`); GUI `ToolpathRuntime::stale_since` set at ~20 sites and
READ by one line (`ui/toolpath_panel.rs` inside `draw_rest_badge`) plus the
MCP wire; `GuiState::edit_counter` compared by `SimulationState::is_stale`. The
card's `RuntimeSnapshot` carries no stale field. Export falls back to the GUI
result when the core result is gone (`io/export.rs::emitted_toolpaths`, by
design comment). Reorder / disable / move-to-setup / tool edit / setup flip /
dressup edit / tool-or-model reassignment mark nothing stale on the GUI side.
Details: `W03/support/r08_source_track.md` §1–2, `r05_source_track.md` §5.
**Deliver.** A state model with named states (current, edited-since,
regenerating, waiting-on-upstream, error, no-result, disabled), the single
owner of each transition, which surfaces render which state (card chip,
inspector header, workspace chips, Readiness, preflight, export gate), and the
list of mutations that must set it (from the R08 matrix). Decide whether the
GUI snapshot is derived from the core result cache (preferred) or the core
publishes staleness. Name the sentries P2 will write.
**Out of scope.** New simulation policy, auto-simulate.

### R0.2 2.5D ramp containment
**Question.** How should a ramp entry behave when its straight leg does not
fit inside the operation's region?
**Today (verified live).** `emit_ramp` (`dressup.rs`) draws a straight leg of
length `ENTRY_CLEARANCE` (2.0 mm) ÷ tan(angle) ÷ 2 — 19.08 mm at 3°, independent
of depth per pass (corrected by R0.2, 2026-09-10; the review's "DPP ÷ tan" was
wrong) — from the entry point, unconstrained in XY. On `fixtures/demo_pocket.svg` the leg runs ~11 mm past
the pocket wall and cuts the surrounding stock (`R03/REPORT.md` UX-R03-001,
evidence 10/12, NC in `R03/scratch`). Ledgered as a not-measured follow-on in
`planning/entry_moves_2026-09-03/FINDINGS.md`. G-RAMPTERRAIN clips legs in Z
on surface ops only; prism ops keep blind legs by design.
**Deliver.** Options with geometry: (a) fold the leg along the first contour
(follow the ring), (b) zig-zag within an inscribed segment, (c) degrade to
helix when the inscribed width allows, else plunge at plunge rate, (d) refuse
with a finding. Which ops are affected (Pocket, Profile inside, Adaptive,
Zigzag, Face?). A containment checker for tests: every fed move of an entry
span lies inside the region offset by the tool radius. How the finding surfaces
when containment is impossible. Cost/perf note. Recommend one.
**Out of scope.** Changing default entry angle or any feed.

### R0.3 One refusal contract for tool and geometry preconditions
**Today (verified).** Four designs: Suggest refusal at add (Scallop/Unified
with Flat/V-bit → toast, op never created; Bull passes and fails at Generate);
static validator disables Generate after add (VCarve/Inlay/Chamfer, stepover ≥
diameter, missing geometry); Rest triple (validator + diagnostic + generator);
generator ERR chip. Four texts for the ball condition; Pencil gets a false
Critical row; the diagnostic fires only for `EndMill`. Add binds
`tools().first()` and `models().first()`. Sources: `R03/REPORT.md` UX-R03-002,
-008; `W03/support/r03_census_source_track_v1.md` §3–4, `r03_census_verification.md` §3.
**Deliver.** One contract: the op is always created; a single
`ToolConstraints`-driven check produces one message id; the inspector shows the
blocked state with the Tool/Input dropdown as the fix; Generate disabled;
menu hover names the tool requirement. Rule for tool binding on add (first
compatible tool, else first tool with blocked state; same for model by
geometry requirement). Whether the add-time Suggest refusal remains at all.
Text table: one sentence per condition, used by GUI, MCP and CLI.

### R0.4 Issue → edit → recheck → return (IA D2)
**Today.** Simulation focus follows playback; clicking an op name jumps
playback, not editor selection; the focused card offers Optimize, not Edit;
"Must address: Annotation 15" is a kind/count grid; Esc returns to Toolpaths
without selecting the op. `IA/CURRENT_MAP.md` §2–3, `IA/PROPOSED_FLOW.md` D2,
`W01/CHECKPOINT.md` UX-R06-001, `W02/REPORT.md` UX-R06-003.
**Deliver.** The bookmark data (op id + spatial/semantic context, never a bare
move index), the events (`LocateFinding`, `EditResponsibleOperation`,
`RecheckChanges`, `ReturnToFinding`), what "matching fresh evidence" means and
how a failed remap is shown, and how named findings replace the Annotation
count (which `SimulationIssueKind`s become named subjects). Depends on R0.1
for the freshness wording.

### R0.5 Value roles and Apply scope (IA D4)
**Today (verified).** Feeds card prints `result.chip_load_mm` and
`result.axial_depth_mm` (raw calculator) under "Commanded advance/tooth" and
"DOC"; per-field ⚡ pills write raw values; `apply_feeds_subset` clamps; core
`feeds/provenance.rs` already records per-field origin; pills stamp "manual";
MCP `set_toolpath_param` stamps Manual on feed_rate/plunge_rate/stepover/depth_per_pass/spindle_rpm but not on op-specific aliases (z_step, max_depth, scallop_height) — corrected by R0.5, 2026-09-09. `W03/support/r04_source_track.md`
§1–4, `R03/REPORT.md` UX-R03-005, -014.
**Deliver.** The four roles (configured, proposed, inherited default, emitted)
and how each renders; the affected-fields list per Apply route (R1–R16 table);
which provenance stamp each route writes; what the modal adds over the card.
Wireframe from `IA/PROPOSED_FLOW.md` D4 as the target.

### R0.6 Planner and dialog ownership (IA D5)
**Today (verified).** `planner_origin` has no GUI surface; Apply plan removes
every planner-owned op in the setup, hand-edited or not, with a 4 s toast
afterwards; no undo for structural edits; boundary Source combo cannot return
to Planned Tier Regions; tool draft auto-commits on navigate-away while
showing Apply/Revert; "Save to library" writes the uncommitted draft and
overwrites by geometry signature. `W03/support/r05_source_track.md` §6,
`r03_census_source_track_v1.md` §7, `IA/PROPOSED_FLOW.md` D5.
**Deliver.** The before-Apply change summary (add / replace / manual edits
affected / kept), a planner-owned badge on cards, the commit-boundary decision
for tool drafts, the library save confirmation when a signature match will be
replaced, and what structural undo would need (scope only; implementation is
P5).

### R0.7 Missing-model repair route
**Today (verified).** `load_error` is never rendered; repair is Reload from disk
(same path) or Delete (refused while referenced); no browse-for-new-path; paths
are saved absolute. `W03/support/r08_source_track.md` §5.
**Deliver.** A "Locate file…" action on the model inspector and the load
warning, path relativisation rule on save, and downstream invalidation on
relink. Small; may be folded into P1 if the recommendation is obvious.

## 3. Phase P1 — clear-spec defects (no research needed)

Each item: what, where, acceptance, sentry. All independent; run in parallel.

> **STATUS after night 1 (2026-09-10).** MERGED with sentries on
> `ui-fix-2026-09-09`: **F1.1, F1.2, F1.3, F1.4, F1.5, F1.6, F1.7, F1.8,
> F1.9** — commits and caveats per row in `STATUS.md`, per-task detail in
> `reports/F1.x.md`. STILL TO DO: **F1.10, F1.12, F1.13, F1.15**.
> **F1.11 is CANCELLED as written** — R0.1 replaces the whole GUI
> `stale_since` scheme, so adding more flag sites is rework; its mutation
> matrix becomes P2's F2.1 test list. **F1.14 is FOLDED INTO F4.2** —
> R0.3's binding rule covers it. Do not re-run a merged row.

| ID | Finding | Change | Acceptance / sentry |
|---|---|---|---|
| F1.1 | UX-R03-003, R08 §8 | In `app/mcp.rs::drain_mcp_requests`, push every notification AFTER the handler and from its result; failures push the refusal text at Warning. Nine past-tense toasts affected (Loaded, Saved, Set…, Added toolpath, Removed toolpath, Added tool, Imported tool). | `mcp_toasts_report_outcome_g_mcptoast.rs`: a refused `add_toolpath` yields no "Added" notification and one containing the refusal. |
| F1.2 | UX-R03-014, R04 §1 R3/R4 | Route `ValueRow::suggest` (`ui/components/value_row.rs`) and `dv_pill` through `feeds::suggest::apply_feeds_subset` scoped to the one field, so the pill offers and writes the clamped value; stamp provenance as the recommendation, not "manual". Cover all 24 pill sites. | `pill_writes_clamped_value_g_pillclamp.rs`: on the demo pocket seed the Depth/Pass pill offers 1.2, not 4.2; extend `apply_contract_a3.rs` to cover `ValueRow`. |
| F1.3 | UX-R03-005 | Relabel the feeds card rows "Recommended advance/tooth" and "Recommended DOC / WOC", and add the configured value beside each (`properties/mod.rs::draw_feeds_card`). Same in the Feeds modal DOC row. Full presentation redesign is D4; this is the honest label now. | Unit test on the label strings; MCP-VIEW check on the pocket seed. |
| F1.4 | UX-R03-004, X3 | `drill_holes_for_config` (`compute/execute.rs`): when `selected_holes` is `None` AND the model exposes no `DrillTarget`s, return `MissingGeometry("No drill targets — pick points/circles or import a drawing with circles")` instead of polygon centroids. Keep centroid fallback only when targets exist and none are picked (current documented behaviour) — or remove it; state which in the commit. Inspector: always show "Drill targets: N" (0 allowed) and fix the purpose text to name DXF points and circle/arc centres. | `drill_no_targets_refuses_g_drillcentroid.rs` on `fixtures/demo_star.svg`: Generate is blocked, no move emitted. |
| F1.5 | UX-R03-009, X1 | Remove the dead `boundary_inherit` checkbox from `properties/mod.rs` (~4252); always show Source/Containment/Offset; print "Boundary: model silhouette (auto)" when the controller set it. Keep the field in the project file for compatibility (read and ignore, or write false). | Sentry: the Geometry tab text names the stored `boundary.source`; `rg boundary_inherit crates/rs_cam_viz/src/ui` returns no reader. |
| F1.6 | UX-R03-007 | Static validator rule: depth (or bottom Z) below the stock bottom → caution "Depth exceeds stock thickness by X mm" on header and row; not a block. Applies to every 2.5D depth field. | `depth_beyond_stock_cautions_g_depthstock.rs`. |
| F1.7 | UX-R03-006 | Profile: when depth ≥ stock thickness show "Through cut of a N mm board · Holding: no tabs configured" beside Depth and open the Tabs disclosure by default in that case. Zero tabs stays valid. Add `On` to the Side combo (schema already has it). | Unit test on the summary string; MCP-VIEW on the pocket seed profile. |
| F1.8 | R05 §2, defect 2 | `draw_rest_badge` (`ui/toolpath_panel.rs`): use `has_prior_rest_source`'s rule (earlier, enabled, same setup, same model, previous tool). One predicate, two callers. | Sentry: a Rest op whose only candidate predecessor is below it shows "no dep". |
| F1.9 | R05 §5 export omission | `io/export.rs::emitted_toolpaths`: an enabled op with no result is an error naming the op ("… is still waiting on upstream stock" / "not generated"), not a silent skip; wizard preflight lists it. | `export_refuses_ungenerated_op_g_exportskip.rs`. |
| F1.10 | R05 defects 4, 6, 7 | (a) cross-setup drag honours the drop index (`mutation.rs::move_toolpath_to_setup` insert, not push); (b) same-setup reorder is an insert, not a swap — confirm intent with the Move Up/Down semantics and document; (c) GUI Generate All without a rest chain submits only enabled ops; (d) the MCP fixpoint ladder that rewrites `state.simulation.resolution` / `auto_resolution` pushes a notification saying so. | One sentry per letter. |
| F1.11 | R08 §1 dirty/stale gaps | Make these call `mark_edited` and set stale where the R08 matrix says they do not: toggle enabled (dirty + dependents stale on GUI side), dressup edits (stale), tool/model reassignment (dirty + stale), name/coolant/pre-post G-code (dirty). Setup face/rotation: mark every toolpath in the setup stale (mirror the MCP `SetupChanged` path). | Extend `controller/tests.rs` with one case per mutation asserting `dirty` and `stale_since`. |
| F1.12 | R08 §3 | Ctrl+O / File › Open with unsaved edits: reuse the close-interception dialog (Save / Discard / Cancel). MCP `load_project` with unsaved edits: refuse unless `discard_unsaved: true`. | `controller/workflow_tests.rs` case; MCP schema test. |
| F1.13 | UX-R03-013, IA small repairs | Wording: "Select an item in the project tree" → "Select an operation, tool, setup or model"; Readiness added to the Workspace menu; camera Fit on `load_project`; Getting-started list gains "Simulate and review" before "Export". | Snapshot/unit tests on strings; `overlays_registry`-style completeness test for the Workspace menu. |
| F1.14 | R03 census X2 | Add menu availability uses the model that WILL be bound (see R0.3 for binding), not "any model in the project"; until R0.3 lands, grey the entry when the first model lacks the geometry and say which model. | Unit test on `add_op_menu_item` availability. |
| F1.15 | UX-R09-001, W02 | Wrap the tool-load caution and the reach-map line in the inspector header (`ui.label(RichText).wrap()` or `add(Label::new(..).wrap(true))`), so the missing-guarantee sentence is readable at 1400×900. | MCP-VIEW on the terrain seed scallop; no text clipped at the right edge. |

## 4. Phase P2 — freshness model (after R0.1)

> **OPERATOR DECISIONS, 2026-09-10.** Two of R0.1 §7's open questions are
> answered and they override the research recommendation where they differ.
> **Q1 stock edits: EVERYTHING STALES** — dimensions, pins and material
> alone all drop every result in the setup, on the GUI route and the MCP
> route alike. R0.1 recommended a dimensions-only split; the operator chose
> the simple rule and accepted the cost (every stock nudge regenerates every
> 2.5D op after the 500 ms debounce). **Q2 regenerate on load: 2.5D ops
> only, respecting each op's own auto-regen dial** — 3D manual-regen ops
> load as `NoResult`. That is the new task F2.6 below, not part of F2.1.
> Q3 (kinematics stales the simulation only) and Q4 (reordered `Fresh` ops
> stay `Current`) stand as R0.1 recommends. Q5 and Q6 are still open.

| ID | Change | Acceptance |
|---|---|---|
| F2.1 | Implement the state per R0.1: one `FreshnessState` derived for every toolpath from the core result cache + edit tracking; `RuntimeSnapshot` carries it. | Unit tests over every mutation in the R08 matrix produce the expected state. |
| F2.2 | Render it: card chip ("STALE" beside OK), inspector header ("Done · edited since"), dimmed path in the viewport, workspace chips count stale toolpaths, Readiness and preflight treat stale as not-current. | MCP-VIEW on the terrain seed: edit scallop stepover, screenshot shows the chip; `list_toolpaths.stale` agrees. |
| F2.3 | Export gate: a stale or absent result blocks export unless an explicit accept flag is passed; remove the silent GUI-result fallback in `io/export.rs` or make it refuse with the reason. | `export_refuses_stale_result_g_stalexport.rs`. |
| F2.4 | Late-result guard: a result arriving for a config edited since submission is stored but marked stale, never shown as current (3D manual-regen ops). | Extend the `a_param_edit_mid_generate_*` sentry to the manual-regen arm. |
| F2.5 | Undo restores freshness meaning: `ToolChange` undo calls `invalidate_tool`; every undo arm calls `mark_edited`; `ToolpathParamChange` undo re-stales the simulation. | `controller/tests.rs` cases. |
| F2.7 | **Opened by F2.2, 2026-09-10.** The MCP wire publishes no `freshness` key: `list_toolpaths` still exposes only the legacy `stale` boolean derived from `stale_since`, which F2.1 demoted to a debounce clock. The F2.2 acceptance line ("`list_toolpaths.stale` agrees") cannot be checked until the wire carries `FreshnessState::label()`. | A schema pin that every row publishes a `freshness` value drawn from the enum, and a case asserting an edited op reads `edited_since`. |
| F2.8 | **Opened by F2.2, 2026-09-10.** Colour overlays are exclusive per surface, so a stale path drawn under the engagement or advance-per-tooth palette is a stale palette too, and under the reach overlay every move is already dimmed so the stale dim adds no signal. Not misleading today (both readings are old together) but unresolved. | Decide whether staleness needs a non-colour channel — outline, hatch or a viewport banner — that survives an exclusive palette. |
| F2.6 | Load-time regeneration policy per the operator's answer to R0.1 §7 Q2: `controller/io.rs::open_job_from_path` stops forcing `auto_regen = true` on every op and requests regeneration for 2.5D ops only, respecting each op's `default_auto_regen`. 3D manual-regen ops load as `NoResult` and wait for an explicit Generate. Removes the G-REGEN-RACE precondition. | `load_requests_only_25d_regen_g_loadregen.rs`: a loaded project with one pocket and one scallop submits the pocket and not the scallop; the scallop's state is `NoResult`, not `Error`. |

## 5. Phase P3 — MCP gap fill (parallel with P2)

Goal: make every GUI route the review could not exercise reachable through the
same `AppEvent`s the widgets emit, so defects are reproducible and the P5
changes testable without a pointer driver. Register in `mcp_server.rs`, mirror
the parameter structs in `rs_cam_mcp`, add to `get_operation_schema`-style
schema tests, document in the CLAUDE.md MCP section.

| ID | Tool | Notes |
|---|---|---|
| F3.1 | `add_toolpath_via_gui(operation_type, setup_index?)` | Dispatches `AppEvent::AddToolpath` through `handle_add_toolpath`, so the GUI's tool/model binding and refusal path run. Returns the created index or the refusal text and whether a notification was pushed. |
| F3.2 | `set_face_selection(index, face_ids[])` / clear | Same event the viewport picker emits. Enables the STEP face-selective cases. |
| F3.3 | `undo` / `redo` | Returns what was undone (the `UndoAction` variant and fields). |
| F3.4 | `set_toolpath_row_control(index, control, value)` | visible / locked / auto_regen / enabled via the row-control events. |
| F3.5 | `get_notifications` | Current toast stack with severity and age, so tests can assert what the operator saw (pairs with F1.1). |
| F3.6 | `select(kind, id)` for tool / model / setup / stock / machine | Completes `set_ui_view`; lets `screenshot_gui` reach every inspector. |
| F3.7 | `set_toolpath_tool(index, tool_id)` | **Added by R0.3 §4.** Today `set_toolpath_param` routes an unknown key into op params, so MCP cannot rebind a toolpath's tool. Without this a blocked op is a dead end over MCP and the F4.2 refusal contract cannot be driven from a test. |
| F3.8 | `set_toolpath_model(index, model_id)` | Same, for the model / geometry input. |

Sentries: `mcp_authoring_surface.rs`-style schema pins for each; one live
parity test that `add_toolpath_via_gui` on the flat-first terrain seed
reproduces the R0.3 contract.

## 6. Phase P4 — correctness needing research

> **OPERATOR DECISIONS, 2026-09-10, binding on F4.2.** **R0.3 §7 Q3 — a
> bull-nose tool on Scallop is ALLOWED**; widen `required_kinds` rather
> than keeping the registry's exclusion (against R0.3's recommendation).
> The generator was written for a ball tip, so F4.2 must PROVE from the
> code that the scallop generator drives the tool's corner radius and not
> an assumed ball of the envelope radius. If it does not, F4.2 records a
> blocker and leaves the registry alone: a permissive registry over a
> generator that cuts wrong is a new untruth, which is the one thing this
> programme must not add. **R0.3 §7 Q6 — Spiral Finish joins the ball-tip
> list**, so it refuses a flat tool at the same surface as Scallop and
> Unified instead of generating and objecting only in the Feeds tab.
> Q1, Q2 and Q5 are still open and keep R0.3's recommendations.

| ID | Depends | Change | Acceptance |
|---|---|---|---|
| F4.1 | R0.2 | Implement the recommended ramp containment for prism ops; add `entry_audit::fed_moves_outside_region` (region offset by tool radius) as a report-only finding and a triage safety row when nonzero. | `ramp_contained_in_region_g_rampcontain.rs` on `fixtures/demo_pocket.svg`: zero fed entry moves outside the offset region; sim checkpoint 0 removes nothing outside the 70×50 outline. Keep the ramp option; document the degrade rule in CLAUDE.md beside G-RAMPTERRAIN. |
| F4.2 | R0.3 | Implement the refusal contract: registry-driven check, op always created, validator arm for ball ops, one message table, compatible-tool binding on add, Pencil false-Critical removed, EndMill-only diagnostic generalised to the registry predicate. | `tool_precondition_one_contract_g_refusal.rs`: for each (op, tool type) pair the four surfaces (menu hover, add result, inspector, generator) agree. |
| F4.3 | R0.7 | "Locate file…" on missing models; relative path on save when under the project dir; relink invalidates dependents. | `controller/tests.rs` missing-model case extended with a toolpath reference and a relink. |

## 7. Phase P5 — IA design changes (after design sign-off on R0.x)

Implement in this order; each is its own worktree and lands behind no flag
(the review rejected a mode split).

| ID | Design item | Scope | Acceptance journey (from `IA/PROPOSED_FLOW.md`) |
|---|---|---|---|
| D0 | Freshness everywhere | = P2 | — |
| D2 | Named findings + Locate / Edit operation / Recheck / Return | Simulation inspector focused card; `sim_diagnostics.rs` must-address grid → named subjects; new events per R0.4; bookmark remap rule. Keep Optimize as an alternative. | Locate a finding, edit its op, regenerate, recheck, land on matching fresh evidence without indices. |
| D4 | Current plan vs proposed change | Feeds card and modal per R0.5; Apply buttons name affected fields; provenance stamps correct on every route. | Accept speeds while keeping a manual stepover; predict the changed fields. |
| D3 | Decision-first forms | Primary section + refinement disclosure per family table in D3; targets first on Drill; through-cut/holding line on Profile (F1.7 is the seed); leave/entry/workholding never hidden. | State target, depth/quality intent and consequence without opening every section. |
| D1 | Context header + partial-job next actions | Small header (job / setup / operation / input / tool / path state / checks state) with "Change target…"; start card replacing the Getting-started list; "Edit in Setup" when a setup inspector shows in Toolpaths. | After a workspace switch or playback jump, name the object the next Edit affects. |
| D5 | Preview / Apply / Close ownership | Before-Apply change summary in the planner; planner-owned badge on cards; boundary Source combo can restore Planned Tier Regions; tool draft commit boundary per R0.6; library save confirms signature overwrite and never writes an uncommitted draft. | Name what is replaced and kept before planner Apply or library Save. |
| D6 | Common export handoff | "Review export" summary from Readiness and File with scope, checks, files; both routes disclose the same checks. | Predict exported ops/tools/setups/files from either entry. |

## 8. Phase P6 — validation

| ID | What | When |
|---|---|---|
| V6.1 | FULL core gate + clippy with heavy-tests + fmt on the merged branch. | End of each phase. |
| V6.2 | Re-run the R03 live sequence through MCP on the four scratch seeds (copies), capture the same 29 views, diff against `R03/evidence/`, and record what changed in `planning/ui_fix_2026-09-09/reports/`. | After P1, after P2+P4, after P5. |
| V6.3 | Sentry census: every task id above maps to a named test that exists and passes. | End of programme. |
| V6.4 | Human journeys 1–5 from `IA/PROPOSED_FLOW.md` §"Validate cheaply", unaided, recorded in PROTOCOL trace format. | After P5, before calling D1–D6 done. |

## 9. Suggested overnight schedule

Night 1: all of P0 (seven research agents, read-only) and all of P1 (fifteen
small fix agents, each in a worktree). Merge P1 in id order; conflicts are
expected only in `properties/mod.rs` and `toolpath_panel.rs`.
Night 2: P2 serially on one branch, P3 in parallel, P4 as soon as R0.2/R0.3
are reviewed. V6.1 and V6.2 at the end.
Then a design review of the R0.x documents before any P5 agent starts.

## 10. Out of scope for this programme

Threshold or gate recalibration; new operations; the multitool planner
algorithm; the wanaka production jobs; anything in `planning/linking_2026-09-09`
or `planning/island_clip_2026-09-09` (other developers' active work).

## 11. Follow-on tasks opened by night 1 (not yet scheduled)

These came out of doing the work, not out of the review. Each is small and
carries its evidence.

| ID | Source | What |
|---|---|---|
| F1.16 | F1.7 report | `ProfileSide` has only `Outside` / `Inside` and the generator has no on-the-line arm, yet the operation schema advertises `on`, so `set_toolpath_param side=on` fails at serde. Either implement the arm or drop the schema value. A combo entry that generates the same as Outside would be a NEW untruth — do not add one. |
| F1.17 | F1.9 report | `readiness::operations_check` counts GUI-store results only, so Readiness can read "2/2 computed" beside a blocking export row. Fold into P2's F2.2, which re-renders Readiness from the freshness state anyway. |
| F1.18 | F1.6 report | The depth-beyond-stock caution is GUI-side, so it reaches neither the Operations card row nor MCP `get_toolpath_diagnostics`. Moving the rule into core `heights_checks` would cover both surfaces with one predicate. |
| F1.19 | F1.6 report | 2.5D generators IGNORE a pinned Bottom Z, so a Heights-tab pin below the stock cautions on a number that is not emitted motion. Pre-existing; decide whether the pin should drive the cut or the field should be disabled for 2.5D. |
| F1.20 | R0.5 §2 | The feeds modal's "Apply explored values" also rewrites plunge, while its hover says plunge is "pulled down". Code reading only — verify live before fixing. |
| F1.21 | F1.5 report | No provenance records that the controller assigned a model-silhouette boundary automatically, so the Geometry tab cannot say "(auto)". Needs a flag on the boundary config. |
| — | F1.4 gate | `arcfit_intent_key_cost_f1` and `narrate_regions_closed_c8` fail at master 3d88406b and are NOT this programme's. They need an owner; do not let a worker "fix" them inside a UI task. |
