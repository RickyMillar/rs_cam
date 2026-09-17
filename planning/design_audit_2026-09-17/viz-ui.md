# viz-ui — design and feature-debt audit

Group: `crates/rs_cam_viz/src/ui/` (76 files, 39 315 lines).
Read first: `crates/rs_cam_viz/CLAUDE.md`, `ui/CLAUDE.md`,
`ui/properties/CLAUDE.md`, `ui/feeds/CLAUDE.md`, `ui/components/CLAUDE.md`,
`ui/overlays/CLAUDE.md`.

Out of scope (another account owns them): `planning/ui_premium_2026-09-13`,
`COLLISION_POINT`, `SPACE_0`, `INK_00`, `LANE_SCALE`, `draw_trace_badge`,
`row_hover_tint`.

## Findings

### UI-01 `draw_toolpath_panel` takes 25 parameters and runs 1 255 lines
- kind: design
- pattern: god function + parameter list that wants a snapshot struct
- where: `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:39`
- evidence: a signature parse of all 800 `fn` in `ui/` puts this function
  first at 25 parameters; the next is 11 (`properties/feeds_speeds.rs:166
  draw_feeds_card`) and only 12 functions reach 8. The body ends at line
  1 294, so it is 1 255 lines. Four of the 25 already travel inside
  `ToolpathPanelSnapshot` (`properties/mod.rs:861`); the other 21 do not.
- proposal: move the remaining read-only arguments into
  `ToolpathPanelSnapshot` (or a second `ToolpathPanelInputs` beside it) and
  build them in `toolpath_panel_snapshot`, which already exists and is
  already the sentry-facing assembly point. That leaves
  `(ui, snapshot, show_reach_map, events)`. Then split the body on its own
  tab boundaries — Geometry, Feeds, Linking, Heights, Dressup — which
  `ToolpathTab` already names.
- breaks: `pub fn toolpath_panel_snapshot` signature; the integration
  sentries that call it.
- effort: M
- risk: low — the arguments are already assembled by one caller; the
  compiler finds every site.
- sentry: `the_inspector_nests_once_dc5` and
  `inspector_width_is_tab_independent_up4` already render the panel; add a
  field-presence assertion to `properties/tests.rs` beside the existing
  "the bbox must be the model's own" case.
- owner:

### UI-02 The kit's `param_grid` has 2 callers; 82 raw `egui::Grid::new` remain
- kind: design
- pattern: panels hand-roll a widget the component kit owns
- where: `crates/rs_cam_viz/src/ui/components/section.rs:61`,
  `crates/rs_cam_viz/src/ui/properties/operations/boundary_2d.rs:19`
- evidence: `rg -c param_grid ui/` names only `optimize_modal.rs` (5) and
  `components/section.rs` (2). `rg -o "egui::Grid::new" ui/ | wc -l` = 82,
  across 26 files; `boundary_2d.rs` alone has 10, each repeating
  `.num_columns(2).spacing([SPACE_3, SPACE_2]).min_row_height(ROW_DENSE)`
  verbatim. `ui/properties/CLAUDE.md` says "Do not draw a raw egui widget
  where `ui/components/` has the renderer".
- proposal: replace every 2-column parameter grid with
  `UiExt::param_grid`. Keep raw `Grid::new` only where the column count or
  the striping differs, and say so in a comment. Also drop the duplicated
  `.min_row_height(ROW_DENSE)` call in `section.rs:61`, which is written
  twice.
- breaks: none
- effort: M
- risk: low — `param_grid` is exactly the repeated builder chain.
- sentry: extend `panels_read_the_token_module_up1` with a source scan that
  fails on `egui::Grid::new` outside `components/`, with an allow-list for
  the non-2-column grids.
- owner:

### UI-03 `SectionHeader` has 13 call sites; 100 hand-rolled `.small().strong()` headers
- kind: design
- pattern: panels hand-roll a widget the component kit owns
- where: `crates/rs_cam_viz/src/ui/components/section.rs:39`,
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`,
  `crates/rs_cam_viz/src/ui/optimize_project.rs`
- evidence: `rg -c "SectionHeader::new|named_section" ui/` finds 13
  occurrences in 5 files. A multiline scan for the `.small()` +
  `.strong()` chain finds 100 occurrences across 20 files —
  `sim_diagnostics.rs` 43, `optimize_project.rs` 31, `optimize_modal.rs`
  29, `multitool_planner.rs` 29. `section.rs:41` already records "the 41
  hand-rolled `.small().strong()` headers do not" get the treatment; the
  count has grown since that note was written.
- proposal: convert the header sites to `SectionHeader` / `named_section`.
  A header is the kit's element, so one renderer decides its rung, its
  colour and its spacing; today four panels each decide again.
- breaks: none
- effort: M
- risk: low — visual only, and the sentries render the panels.
- sentry: `chrome_reads_the_kit_up3` and `component_contracts_up2`; add the
  `.small().strong()` scan with an allow-list so the count cannot grow.
- owner:

### UI-04 `tooltip_for` dispatches 60 arms on the visible label string
- kind: design
- pattern: stringly-typed door
- where: `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:159`,
  `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs` (`record_stock_to_leave`)
- evidence: `sed -n '/^fn tooltip_for/,/^}/p' | grep -c '=>'` = 60. The
  key is `label.trim().trim_end_matches(':')`, so `"Stepover:"` from
  `boundary_2d.rs:36` reaches the help text only because the two spellings
  match by hand. `record_stock_to_leave` does the same trick to decide
  whether to emit an automation hook:
  `if label.trim().trim_end_matches(':') == "Stock to Leave"`.
- proposal: key the help text on the registry parameter name, not the
  label. `OpRegistryEntry::param_defs`
  (`rs_cam_core/src/compute/catalog/registry.rs:16`) already lists
  `stepover`, `depth`, `depth_per_pass` per operation; add a `help` field
  to `ParamDef` and have `ValueRow` take `(op_type, param_name)`. Renaming
  a label then cannot silently drop a tooltip or an automation hook.
- breaks: `ParamDef` construction in core (`ParamDef::required` gains an
  argument or a builder); no wire key.
- effort: M
- risk: medium — it crosses into `compute/catalog`, which another group
  audits; do it as one change with that group.
- sentry: a test that every `ParamDef` of every `OperationType` resolves to
  a non-empty help string, and that no `ValueRow` label is used as a key.
- owner:

### UI-05 The operation enum is re-matched in five UI places beside the core X-macro
- kind: design
- pattern: enum dispatch replicated in N places
- where: `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:433`,
  `:571`, `crates/rs_cam_viz/src/ui/properties/operations/validate.rs`,
  `crates/rs_cam_viz/src/ui/properties/operations/shape_diagrams.rs`,
  `crates/rs_cam_viz/src/ui/properties/pills.rs`
- evidence: `rg -c "OperationConfig::" ui/` — `properties/toolpath_panel.rs`
  40, `operations/validate.rs` 17, `operations/shape_diagrams.rs` 9,
  `operations/mod.rs` 9, `pills.rs` 2, `tab_badges.rs` 1. Core solved the
  same problem once: `for_each_op!` at
  `rs_cam_core/src/compute/catalog.rs:165` generates `OperationType`,
  `ALL`, `category()`, `name()`, `op_type()`, `new_default()` and
  `as_params()` from one 24-row list, and `registry_entry()` will not
  compile without a row.
- proposal: add a `draw: fn(&mut egui::Ui, &mut OperationConfig, &OpDrawCtx)`
  (or a viz-side `for_each_op!`-driven table keyed by `OperationType`) so
  the per-op editor, the diagram and the validation arm are three fields of
  one row instead of three matches in three files. The second match at
  `:571` — the diagram fallback with its `_ => {}` arm — is the one that
  fails silently today: a new operation gets no diagram and nothing says so.
- breaks: none at the wire; the `draw_*_params` functions gain a uniform
  signature (Pencil and UnifiedFinish are the two that do not have it now,
  and the panel says so in a comment at `:490`).
- effort: L
- risk: medium — 24 editors move to one shape; the compiler catches the
  arms, the uniform context object is the judgement call.
- sentry: a completeness test in the style of
  `overlays_registry` — every `OperationType::ALL` row has a draw fn, a
  diagram decision and a validation arm.
- owner:

### UI-06 The pending inspector tab travels as a `String` beside a `ToolpathTab` enum
- kind: design
- pattern: stringly-typed door
- where: `crates/rs_cam_viz/src/state/runtime.rs:294`,
  `crates/rs_cam_viz/src/ui/properties/mod.rs:621`,
  `crates/rs_cam_viz/src/ui/overlays/panel.rs:284`,
  `crates/rs_cam_viz/src/app/mcp/view.rs:515`
- evidence: `pub pending_toolpath_tab: Option<(ToolpathId, String)>`. The
  enum it names already exists with `ALL`, `label()` and `parse()` at
  `ui/properties/mod.rs:815-853`. `overlays/panel.rs:284` — a GUI-internal
  caller with no wire to cross — writes
  `Some((id, "geometry".to_owned()))`, allocating a `String` to name a unit
  variant. Worse, `app/mcp/view.rs:515-525` keeps a SECOND copy of the key
  list (`"geometry", … "dressup"`) for its refusal message, so a new tab
  needs both lists edited and nothing fails if only one is.
- proposal: store `Option<(ToolpathId, ToolpathTab)>` and parse once, at the
  MCP boundary, where the string actually arrives. Make
  `ToolpathTab::parse` the only key table and build the refusal message
  from `ToolpathTab::ALL`, so the valid-key list cannot disagree with the
  parser.
- breaks: `GuiState::pending_toolpath_tab` type; `ToolpathTab` becomes
  `pub`. No wire key changes.
- effort: S
- risk: low — three writers, one reader.
- sentry: `controller/tests.rs:519` already sets the field; extend it to
  assert every `ToolpathTab::ALL` key round-trips through the MCP validator
  list.
- owner:

### UI-07 Post config exists twice, with a hand-written translation each way
- kind: design
- pattern: one concept, two representations
- where: `crates/rs_cam_viz/src/ui/properties/post.rs:5`,
  `crates/rs_cam_viz/src/ui/properties/mod.rs:232`,
  `crates/rs_cam_viz/src/state/runtime.rs:346`,
  `crates/rs_cam_core/src/session/project_file.rs:207`
- evidence: `post::draw(ui, &mut state.gui.post, stock_top)` edits
  `crate::state::job::PostConfig`; `GuiState::post_to_session` copies its
  six fields into `rs_cam_core::session::ProjectPostConfig`, and
  `post_from_session` copies them back (`controller/io.rs:414`). The core
  side stores the format as `pub format: String` with a `to_token` /
  `from_token` pair, where the GUI side holds the `PostFormat` enum with
  `ALL` and `label()`. Eight call sites read `state.gui.post.*` directly
  (`export_wizard.rs` ×7, `app/export.rs:24`), so the GUI copy — not the
  session — is what the export wizard reads.
- proposal: delete `state::job::PostConfig`; let the panels edit a draft of
  `ProjectPostConfig` the way the stock panel edits a draft of
  `StockConfig`, and move `PostFormat` into core so `format` is the enum
  rather than a token string. That removes both translation functions and
  the `to_token`/`from_token` pair.
- breaks: `ProjectPostConfig.format` changes from `String` to an enum — a
  project-file wire change. Ruled acceptable (operator 2026-09-16); state
  the break in the commit.
- effort: M
- risk: medium — the project file and every `gui.post` reader move
  together; the compiler finds the readers, the file format needs a
  golden re-bless.
- sentry: `tests/wizard_e2e.rs` already drives the wizard end to end; add a
  round-trip assertion that a saved-and-reloaded project keeps every post
  field.
- owner:

### UI-08 A Post-tab edit never marks the project dirty until you leave the tab
- kind: feature-debt
- pattern: a guard that reports the wrong thing
- where: `crates/rs_cam_viz/src/ui/properties/mod.rs:232`,
  `crates/rs_cam_viz/src/ui/properties/panel_apply.rs:355`,
  `crates/rs_cam_viz/src/app.rs:709`
- evidence: the Post branch ends with
  `let _ = apply_panel_command(state, command);` and no
  `state.gui.mark_edited()`. Every other panel door — `apply_stock_draft`,
  `commit_tool_draft`, `apply_setup_draft`, `apply_fixture_draft`,
  `apply_machine`, `apply_machine_kinematics`, `apply_machine_import` —
  calls `mark_edited()` on success. `flush_post_snapshot`
  (`panel_apply.rs:355`) does call it, but only when the selection has
  already moved off `Selection::PostProcessor`. `app.rs:709` reads
  `if os_close_requested && self.controller.state().gui.dirty` for the
  unsaved-changes guard, and `state/mod.rs:107` reads `if !state.gui.dirty`.
- proposal: call `state.gui.mark_edited()` when the `SetPostConfig` command
  applies, exactly as the other seven doors do. The consequence today: the
  operator changes Safe Z or the post format, closes the window without
  clicking another tree item, and loses the change with no prompt.
- breaks: none
- effort: S
- risk: low — one line, and the compare above it already gates on a real
  change.
- sentry: a `rs_cam_viz` integration test that applies a post edit through
  the panel path and asserts `state.gui.dirty`.
- owner:

### UI-09 Panel drafts live in egui temp memory, and one of them re-parses per frame
- kind: design
- pattern: an ad-hoc cache beside the state module
- where: `crates/rs_cam_viz/src/ui/properties/machine_panel.rs:434`,
  `:475`, `crates/rs_cam_viz/src/ui/properties/tool.rs:81`,
  `crates/rs_cam_viz/src/ui/properties/stock.rs:694`
- evidence: `rg -o "insert_temp|get_temp" ui/ | wc -l` = 40 across 10
  files, `machine_panel.rs` alone 16. `state/CLAUDE.md` names `state/` as
  the home of GUI state, and `AppState` already holds `history.tool_draft`,
  `history.stock_draft` and `history.post_snapshot` for exactly this job.
  The GRBL importer is the sharp end: `machine_panel.rs:475` runs
  `MachineKinematics::from_grbl_settings(&buf)` unconditionally inside the
  draw, so the whole pasted `$$` dump is re-parsed on every frame the
  disclosure is open, and the buffer is cloned into `ui.data` on every
  keystroke.
- proposal: move the drafts that survive a frame into `AppState` beside the
  existing ones, so MCP and the integration harness can read them. Cache
  the GRBL parse on the buffer, or parse only when the `TextEdit` response
  reports `changed()`.
- breaks: none
- effort: M
- risk: low — each `egui::Id` has one reader and one writer in the same
  function.
- sentry: the parse fix is testable directly —
  `from_grbl_settings` is pure; assert the panel calls it once per changed
  buffer, or move the parse behind a `changed()` guard and sentry the
  resulting state.
- owner:

### UI-10 69 raw `DragValue` sites against 4 `ValueRow` call sites
- kind: design
- pattern: panels hand-roll the widget the kit owns
- where: `crates/rs_cam_viz/src/ui/components/value_row.rs:1`,
  `crates/rs_cam_viz/src/ui/properties/tool.rs`,
  `crates/rs_cam_viz/src/ui/properties/setup.rs`,
  `crates/rs_cam_viz/src/ui/properties/stock.rs`
- evidence: `rg -o "DragValue::new" ui/ | wc -l` = 69 in 15 files —
  `properties/tool.rs` 13, `properties/setup.rs` 12, `properties/stock.rs`
  10, `multitool_planner.rs` 8, `properties/machine_panel.rs` 5.
  `ValueRow::new` has 4 call sites, all inside `dv` / `dv_pill`
  (`properties/linking_dressup.rs:18`, `:127`) and `feeds_speeds.rs`.
  `value_row.rs:1` states it is "the one labelled-numeric-input row" and
  `components/CLAUDE.md` makes that an invariant.
- proposal: route the Tool, Setup, Stock, Machine and Planner numeric rows
  through `ValueRow`. One renderer then decides the commit semantics — the
  `PanelEdit::drag` triple of `changed` / `committed` / `in_flight` — for
  every numeric field, which is the mechanism the focus-loss-while-typing
  bug turns on (`ui/feeds/CLAUDE.md`: "A numeric input can lose focus while
  the operator types").
- breaks: none
- effort: M
- risk: medium — the four panels have different commit rules today; the
  conversion must keep each one's `PanelEdit` behaviour, not unify it by
  accident.
- sentry: `component_contracts_up2`; add a source scan that fails on
  `DragValue::new` outside `components/`, with a named allow-list.
- owner:

### UI-11 The two drill editors share 65 of their ~90 lines
- kind: design
- pattern: duplicate helper
- where: `crates/rs_cam_viz/src/ui/properties/operations/drill.rs:132`,
  `crates/rs_cam_viz/src/ui/properties/operations/drill.rs:229`
- evidence: a `difflib.SequenceMatcher` over the stripped lines of
  `draw_drill_params` (97 lines) and `draw_alignment_pin_drill_params`
  (83 lines) reports 65 identical lines, ratio 0.72. The four-way
  `DrillCycleType` combo is written out twice verbatim — `:150-170` and
  `:255-275` — label strings included, and so are the peck, dwell and
  chip-break conditional rows.
- proposal: extract one `draw_drill_cycle_rows(ui, cycle, peck_depth,
  dwell, chip_break)` helper that both editors call, the way
  `draw_drill_target_selector` (`:38`) is already shared between them. The
  pin-drill editor then holds only what is genuinely its own: the pin
  diameter and the spoilboard depth.
- breaks: none
- effort: S
- risk: low — the two blocks are already textually identical.
- sentry: `properties/tests.rs`; render both editors and assert the same
  cycle labels appear, so a future edit to one reaches both.
- owner:

### UI-12 Twenty-three draw functions exceed 200 lines; two exceed 650
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_viz/src/ui/sim_op_list.rs:19`,
  `crates/rs_cam_viz/src/ui/properties/mod.rs:126`,
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:435`
- evidence: a brace-matching scan of every `fn` in `ui/` counts 23 bodies
  over 200 lines. The top five after UI-01 are `sim_op_list.rs:19 draw`
  (865), `properties/mod.rs:126 draw` (680),
  `sim_diagnostics.rs:435 draw_project_section` (471),
  `operations/height_diagram.rs:154 draw_height_diagram` (384) and
  `sim_timeline.rs:265 draw_signal_spine` (383).
- proposal: `properties/mod.rs::draw` has the cleanest seam — it is a
  `match state.selection` with one arm per `Selection` variant, each arm
  carrying its own draft/flush/apply idiom inline. Give each arm its own
  `fn draw_<selection>(state, ui, events)` and the file becomes the
  dispatch it claims to be. `sim_op_list.rs::draw` splits on its row
  kinds, which `draw_span_item_row` and `draw_semantic_item_row` already
  name.
- breaks: none
- effort: M
- risk: low — pure extraction inside one file each.
- sentry: the existing workspace sentries render both functions
  (`the_simulation_page_is_summary_first_dc6`,
  `the_inspector_nests_once_dc5`).
- owner:

### UI-13 `pub use feeds as feeds_modal` is a legacy alias with a live delete note
- kind: feature-debt
- pattern: a "for now" comment with a live consequence
- where: `crates/rs_cam_viz/src/ui/mod.rs:38`,
  `crates/rs_cam_viz/src/app.rs:894`
- evidence: `ui/mod.rs:32-37` says "DC5a split `ui/feeds_modal.rs` into
  `ui/feeds/`. `app.rs` still calls `crate::ui::feeds_modal::draw` … Delete
  this line when that call is repointed". `app.rs:894` still reads
  `crate::ui::feeds_modal::draw(ctx, state, events);`. The brief records
  that re-export shims for moved modules were deliberately NOT added; this
  is the one that was, and the operator ruling of 2026-09-16 removes legacy
  aliases outright.
- proposal: repoint `app.rs:894` at `crate::ui::feeds::draw` and delete the
  alias. Two lines.
- breaks: `crate::ui::feeds_modal` disappears — an internal path only.
- effort: S
- risk: low
- sentry: none needed; the compiler is the check.
- owner:

## Top three

1. **UI-01** — `draw_toolpath_panel` takes 25 parameters and runs 1 255
   lines. The snapshot struct it would collapse into already exists and
   already has a public assembly function, so the benefit is large and the
   mechanism is built.
2. **UI-08** — a Post-tab edit never marks the project dirty. One line,
   and it is the difference between the close guard firing and the operator
   losing a Safe Z change.
3. **UI-02** — 82 raw `egui::Grid::new` against 2 `param_grid` callers.
   The kit renderer exists, the builder chain is identical at every site,
   and a source-scan sentry can hold the line afterwards.

## Checked and clear

- The command door holds. Every session write from `ui/` goes through
  `apply_panel_command` / `session.apply` in `properties/panel_apply.rs`
  and `properties/mod.rs` — `rg "session.apply\(" ui/` finds four
  occurrences, all in those two files. No `*_mut()` escape hatch survives;
  the only `_mut` calls left are `ui.spacing_mut`, `ui.data_mut` and
  `ui.memory_mut`.
- `ui/overlays/registry.rs` is the model the rest of `ui/` should copy: one
  `OverlayRow` list feeds the panel, the MCP `set_ui_view` map and the
  completeness sentry, `UploadTime` rows name their trigger, and an
  unavailable row is refused with a reason. Do not re-audit it.
- Abstention discipline is real. `components::NotMeasured` is used by
  `sim_timeline.rs:1099`, `operations/height_diagram.rs:524` and the
  reach summary (`ReachPanelSummary::NotMeasured`), and
  `sim_diagnostics.rs:110` refuses to read an empty triage as an all-clear.
- Planned-versus-emitted is named where it matters. `sim_op_list.rs:1012`
  appends a " planned" suffix from `FeedsProvenance`, and
  `export_wizard.rs:981` prints the cycle time with
  `CycleTimeBasis::qualifier()` and shows a dash rather than a plausible
  zero. No surface found showing a plan as a measurement.
- Surface parity for the modals is declared, not accidental:
  `ui_command.rs` carries 60 rows each with a `Surfaces { gui, mcp, cli }`
  block, and `Reach::Skip` rows state their reason (for example
  `CloseMultitoolPlanner`: "no wire tool closes a planner it cannot open").
  `tests/command_surface_completeness.rs` and
  `tests/command_registry_surfaces.rs` guard it.
- `dv` and `dv_pill` (`properties/linking_dressup.rs:18`, `:127`) already
  delegate to `ValueRow`; the duplication that remains is outside them
  (UI-10), not in them.
- `OverlayRow::flag` carrying a state path as a string
  (`"viewport.show_grid"`) looked like a stringly door, but `get`/`set` are
  fn pointers and the string is only the sentry's mapping key. Leave it.
- `tokens.rs` discipline holds in the panels that matter; the token module
  is the single colour source and `panels_read_the_token_module_up1`
  guards it.

## Add-a-thing count

**Add one modal or panel** — the files an engineer edits today (9 modals
are drawn from 9 hand-written blocks; there is no modal registry):

1. `crates/rs_cam_viz/src/ui/<new_panel>.rs` — the new file.
2. `crates/rs_cam_viz/src/ui/mod.rs` — the `pub mod` row.
3. `crates/rs_cam_viz/src/state/mod.rs` — the open flag or
   `Option<…State>` field, plus its `Default`.
4. `crates/rs_cam_viz/src/app.rs` — the `if …open { draw(…) }` block.
5. `crates/rs_cam_viz/src/ui_command.rs` — the `Open…` / `Close…` rows and
   their `Surfaces { gui, mcp, cli }` reach declarations.
6. `crates/rs_cam_viz/src/controller/events/…` — the handlers.
7. `crates/rs_cam_viz/src/app/mcp/view.rs` — the `modal` string arm AND the
   `"none"` close list AND the refusal message's valid-name list (three
   hand-written places in one function).
8. `crates/rs_cam_viz/src/ui/menu_bar.rs` or `workspace_bar.rs` — the way
   in.

Eight files, and step 7 is three lists that no compiler compares.

**Add one inspector tab** (a `ToolpathTab`):

1. `crates/rs_cam_viz/src/ui/properties/mod.rs` — the `ToolpathTab`
   variant, `ALL`, `label()` and `parse()` (four edits in one file).
2. `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs` — the body arm
   in the tab match (`:302`, `:1040`, `:1082`, `:1088`, `:1102`).
3. `crates/rs_cam_viz/src/ui/properties/tab_badges.rs:18-21` — the badge
   arm. (`draw_toolpath_tabs` at `:356` loops `ToolpathTab::ALL`, so the
   strip itself is free.)
4. `crates/rs_cam_viz/src/ui/properties/<new_tab>.rs` — the body.
5. `crates/rs_cam_viz/src/app/mcp/view.rs:515-525` — the second copy of the
   valid-key list.

Five files, one of which (step 5) duplicates step 1's `parse()` table.

**Add one operation** (the UI half; core adds one `for_each_op!` row plus a
registry entry):

1. `crates/rs_cam_viz/src/ui/properties/operations/<family>.rs` — the
   `draw_*_params` editor.
2. `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` — the
   `pub(super) use` re-export.
3. `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:433` — the editor
   match arm, and `:571` — the diagram fallback arm (which falls through to
   `_ => {}` in silence if you forget).
4. `crates/rs_cam_viz/src/ui/properties/operations/shape_diagrams.rs` —
   the `StepoverPattern::from_operation` arm.
5. `crates/rs_cam_viz/src/ui/properties/operations/validate.rs` — the
   validation arm (17 `OperationConfig::` sites in that file).
6. `crates/rs_cam_viz/src/ui/properties/pills.rs` — the suggestion arm.
7. `crates/rs_cam_viz/src/ui/properties/linking_dressup.rs:159` — a
   `tooltip_for` arm for every new field label.

Seven UI files after core, and only steps 3-6 are compiler-enforced;
steps 3 (the diagram half) and 7 fail silently. UI-05 is the fix.
