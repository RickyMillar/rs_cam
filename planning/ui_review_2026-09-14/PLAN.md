# rs_cam GUI — the review pass after the declutter phase

Date: 2026-09-14, afternoon. Follows `planning/ui_declutter_2026-09-14/`
(commit 236be682) and the reviewer's walk of that build. The operator's own
complaints are the primary list. The reviewer's list is folded in where it
names the same surface.

Every item below was confirmed live on 2026-09-14 against the release
binary built at 14:45 (master fb158141) with wanaka200 loaded, at
1920×1165. The captures sit beside this file. The line numbers are from
that commit.

**The rule of the declutter phase holds.** Each package names what it
DELETES. A package that only moves things is not done.

---

## 1. What was confirmed

| # | Complaint | Seen | Cause in code |
|---|---|---|---|
| 1 | The Feeds & Speeds tab is clipped on the left. | `01_feeds_tab_clipped.png`. The inspector content starts under the viewport. "Name" reads "e:", "Generate" reads "erate", "Geometry" reads "metry". Every other tab fits. | Several rows in the feeds card still carry text with an infinite max width: the "configured 0.0833 mm/tooth" trailing caption, the RPM "default (…)" caption, the two vendor warnings and the rubbing-floor line (`ui/properties/mod.rs:2540` onward, `draw_feeds_card`). Commit a2fee8ed wrapped ONE such string. Its sentry `tests/inspector_width_is_tab_independent_up4.rs` measures a synthetic string and passes while the real tab clips. |
| 2 | Errors still show as large extra tabs. | `02_tab_strip_chips.png`. After a simulation with five rapid collisions the strip reads `Simulation · ✕ 5! · Readiness · ✕ 5 COLLISION(S)`. The two chips read as two more tabs, and they carry two different texts for one count. | `ui/workspace_bar.rs:177-181`. Ruling R30 keeps a `StatusChip` for a `Role::Danger` badge; every other badge is a 6-point dot. The Simulation badge formats `" {n}!"`, the Readiness badge `"{n} collision(s)"`. Sentry `tests/the_workspace_bar_is_a_strip_dc3.rs` arm 2 asserts the chip. |
| 3 | The eye and the click-to-isolate overlap. | Toolpaths workspace. Since WP27 the viewport draws the SELECTED toolpath only, so a click on a card already isolates it. | Five routes now drive two overlapping states. Eye click toggles visibility (`ui/toolpath_panel.rs:415`), eye double-click pins an isolate (R28), the `…` menu has "Isolate this toolpath" (`:516`), the viewport bar has an "Isolate" button (`ui/viewport_overlay.rs:137`) and a "Selected only / All toolpaths" toggle (`:151`). Under the WP27 default the eye on a non-selected card changes nothing visible. |
| 4 | Generate All centres its text on hover only. | Toolpaths workspace. At rest the label is left-aligned in the full-width button. On hover it jumps to the centre. | `ui/components/button.rs:161-186`. The hover branch repaints the label at `rect.center()` with `Align2::CENTER_CENTER`; the rest state is egui's own `Button`, which places the label at the left. Every `Button` with a `min_width` has this: Export G-code and Run simulation included. |
| 5 | The `…` next to Tools does not fit. | `03_setup_rail.png`. `Tools 12 […] ›`: a boxed menu button stacked against the navigation chevron. | `ui/setup_panel.rs:276`, `resource_row`. The chevron is drawn unconditionally, then the `…` menu beside it in the same right-to-left row. |
| 6 | Models / Tools rows show a `›` that does not open when the count is 0. | Confirmed in code; the project has 4 models and 12 tools so it was not seen. | `ui/setup_panel.rs:137-147` and `:235-243`. The click selects `.first()`. With an empty list it does nothing, and the row still draws the chevron. |
| 7 | Simulation duplicates the run button. | `04_sim_after_run.png`. "Re-run Simulation" in the left panel, "Re-run" top-right of the viewport, "Re-run" in the bottom Cut-metrics bar. Before a run the same three read "Run Simulation" / "Re-run" / "Enable & re-run". | `ui/sim_op_list.rs:116` and `:203`, `ui/viewport_overlay.rs:211`, `ui/sim_timeline.rs:515` and `:1102`. Also: `sim_op_list.rs:324` still draws the eye · C · R · bullseye glyph row per card that DC1 deleted from the toolpath card, and the selected card hugs its content. |
| 8 | The feeds modal is overwhelming. | `05_feeds_modal.png`. Spindle policy row, tool/material/source line, Recommendation table, Power, MRR, Apply all, "Why is the recommendation here?", "How is this calculated?", Warnings, Feed-vs-RPM chart, colour legend — all in one window. And the Feeds TAB already shows the same Compare block (feed, plunge, RPM, DOC, WOC, both Apply buttons). | DC5a split `feeds_modal.rs` into four files but moved every function VERBATIM (`ui/feeds/mod.rs` says so). The window still draws Compare + Why + Explore. Only the project rollup left. |
| R | Readiness (reviewer, not operator). | `06_readiness.png`. "REVIEW BEFORE CUTTING" banner above a blue Export G-code Primary. "Run sim" appears three times. | `ui/readiness_panel.rs`. |

## 2. The packages

Ordered by what blocks use. Each names its lane. A writer never runs
cargo; one verifier at a time holds the cargo lane.

### UR1 — the inspector cannot be widened by its content

**Deletes** every bare label in a horizontal row inside the feeds card.

- At the inspector root, `ui.set_max_width(panel width)` once, so no child
  can grow the panel.
- Every trailing caption in `draw_feeds_card` goes through the existing
  `wrapped_small_label` (`ui/properties/mod.rs:3752`) or `.truncate()`.
  The rows named in §1 item 1 are the known set; the writer scans the whole
  card.
- Sentry, two arms. (a) A source scan bans a bare `ui.label(` inside a
  `ui.horizontal(` in `draw_feeds_card` and in `ui/feeds/`. (b) The width
  fixture draws the REAL feeds tab on a `ProjectSessionBuilder` project
  whose feeds emit the rubbing-floor and vendor-LUT warnings (Back Rough's
  shape: Ø6 flat, plywood, 3000 mm/min, 18000 RPM) and asserts the
  requested width does not exceed the panel. The synthetic-string arms of
  `inspector_width_is_tab_independent_up4.rs` stay; they are not wrong,
  they are not enough.

Lane: writer + verifier. Files: `ui/properties/mod.rs`, one test.

### UR2 — a Danger badge is a dot too

**Deletes** the `StatusChip` arm in `workspace_tab`.

- Every badge is the 6-point dot in its Role colour, count on hover. A
  Danger dot uses `Role::Danger`. The two badge texts become one format.
- Where the safety voice lives after the chip goes: the Simulation
  Inspector header (`✕ 5 safety — …`), the Readiness banner, the status
  bar `collisions 5 →` pill and the sim card's `rapid×5`. Four surfaces
  say it. The tab strip does not need to.
- Sentry `the_workspace_bar_is_a_strip_dc3.rs` arm 2 flips: the Danger
  block must NOT name `StatusChip`. Arm 1 (no word in the strip) now
  covers Danger as well.

Lane: writer + verifier. Files: `ui/workspace_bar.rs`, one test.

### UR3 — one Run Simulation, and the sim card reads the kit

**Deletes** two of three run buttons and the four-glyph row on the sim
card.

- `Run Simulation` / `Re-run Simulation` becomes a
  `components::Button::primary` at full panel width at the TOP of the
  left panel, the way Generate All sits in Toolpaths. The `Setup & run`
  disclosure sits under it. The stale card's own Re-run folds into the
  primary's label ("Re-run Simulation · params changed").
- The viewport overlay keeps `Reset` and loses `Re-run`
  (`viewport_overlay.rs:211`). The bottom bar's `Enable & re-run` /
  `Re-run` (`sim_timeline.rs:515`, `:1102`) becomes the Cut-metrics
  checkbox alone; a change there marks the simulation stale, and the
  primary is the one route to run it. The menu keeps its entry.
- `sim_op_list.rs:324` stops calling `toolpath_row_controls::draw`. The
  sim card reads the DC1 kit: `swatch · dot · name · tool · verdict`, one
  height, filling the column. The per-card C / R visibility goes behind
  the same `…` the toolpath card uses (R23).
- Sentry `the_simulation_page_is_summary_first_dc6.rs` pins function
  shapes in `sim_timeline.rs`; read it before the strip moves. New arm:
  exactly one `AppEvent::RunSimulation` producer in `ui/sim_*.rs` and
  `ui/viewport_overlay.rs` combined.

Lane: writer + verifier. Files: `ui/sim_op_list.rs`, `ui/sim_timeline.rs`,
`ui/viewport_overlay.rs`, tests.

### UR4 — finish DC5a: the modal is Explore only

**Deletes** Compare and Why from the window.

- The Feeds TAB is authoritative for this operation. It already draws
  Compare. "Why is the recommendation here?" and "How is this calculated?"
  become one expand on the tab (`ui/feeds/why.rs` moves under the tab's
  disclosure, not into a window).
- The window keeps the chart, the drag point, the spindle policy row and
  the colour legend: `ui/feeds/explore.rs`. Title: "Explore feed vs RPM —
  {name}". Default size falls to fit the chart. The tab's button reads
  "Explore…", not "Open Feeds & Speeds modal".
- `ui/feeds/compare.rs` is drawn from the tab only. If the tab's Compare
  and the file's Compare differ, the file's is the one that survives; the
  tab's hand-written block is deleted in favour of it.
- Sentry `the_feeds_modal_holds_one_scope_dc5a.rs` pins today's shape and
  changes with it. New arm: `ui/feeds/mod.rs`'s window body names
  `explore::` and nothing from `compare::` or `why::`.

Lane: scout first (the tab's Compare and `compare.rs` were written apart;
the scout lists the field-by-field differences), then writer + verifier.
Files: `ui/feeds/*.rs`, `ui/properties/mod.rs`, tests.

### UR5 — one visibility model (operator ratifies)

Two operator rulings made on 2026-09-13 encode two models. WP27 says
selection drives what is drawn. R28 says the eye's double-click pins an
isolate. Both are on master. The overlap the operator feels is the seam.

**Proposed model, one sentence.** What is drawn is the selection; the eye
matters only in All-toolpaths mode.

- Default (Selected only): the card's eye is not drawn. A click selects
  and therefore isolates. Nothing else on the card touches visibility.
- All toolpaths mode: the eye is drawn and toggles the toolpath in and
  out of the picture. Double-click does nothing.
- **Deletes**: the isolate pin (`ViewportState::isolate_toolpath`), the
  viewport "Isolate" button, the `…` menu's "Isolate this toolpath", and
  the eye's double-click arm. `UiCommand::ToggleIsolateToolpath` and
  `ClearIsolation` leave the registry; `command_surface_completeness.rs`
  and `command_registry_surfaces.rs` are the census that must stay green
  when they go, and the MCP `set_ui_view` reply drops the field.
- Touches WP27's `ToolpathDrawFilter` and `OverlayUploadKey`
  (`selected_toolpath`, `show_all_toolpaths`); the "isolate pin wins" arm
  of `toolpaths_to_draw` is deleted and its test rewritten.
- Alternative the operator may prefer: keep the pin, delete the eye. That
  is one fewer control too. The proposal above keeps the eye because "hide
  this one while I look at the rest" has no other route in All mode.

Lane: scout (map every reader of `isolate_toolpath`), operator ruling,
then writer + verifier. Files: `ui/toolpath_panel.rs`,
`ui/viewport_overlay.rs`, `state/viewport*.rs`, `render/*` (filter),
`app/mcp.rs`, `ui_command.rs`, tests.

### UR6 — the button label sits in one place

**Deletes** the hover-only repaint of the label.

- `components/button.rs`: draw the button frame with an empty label and
  paint the text at `rect.center()` in EVERY state, or drop the repaint
  and let egui's label stand in both. Centre in both is the operator's
  ask. Covers Generate All, Export G-code, Run simulation and every
  future Primary.
- Sentry: a source scan that `fn ui` in `button.rs` paints text once,
  outside the hover branch.

Lane: writer + verifier. Smallest package. Files: `ui/components/button.rs`,
one test.

### UR7 — the resource row has one affordance

**Deletes** the chevron on a row that carries a `…` menu, and the dead
chevron on an empty row.

- `resource_row`: when `menu` is `Some`, the `…` IS the trailing
  affordance and the chevron is not drawn. Better: the Tools row's menu
  (`Manage library…`, add a tool by type) moves into the sidebar the row
  opens, so no row carries a menu and every row ends in one chevron.
- An empty Models or Tools row reads `Models none` with no chevron. Its
  click runs the add action (File → Import for models; the library for
  tools) instead of selecting nothing.
- Sentry `the_setup_rail_is_one_weight_dc4.rs` counts `fn resource_row`
  and pins the rail; add one arm: the row draws `CHEVRON` and `ELLIPSIS`
  on mutually exclusive paths.

Lane: writer + verifier. Files: `ui/setup_panel.rs`, one test.

### UR8 — Readiness (reviewer's item; the operator did not ask)

**Deletes** two of three "Run sim" buttons and the Primary role on Export
while the banner reads REVIEW.

- One action row under the banner: the Primary is the FIRST unmet
  check's action ("Run simulation", or "Review collisions" when the
  simulation has any). Export is a Quiet button until every check is
  green, then it is the Primary. The per-row buttons go.
- "Simulation: Up to date" gains scope: "Simulation: 3/8 operations".
- Drop this package if the operator does not want Readiness touched now.

Lane: writer + verifier. Files: `ui/readiness_panel.rs`, tests.

## 3. Order

1. UR1 and UR2 first. Both are defects on the first screen a reviewer
   sees, both are small, both flip one sentry.
2. UR6 and UR7 next. Small, no ruling needed.
3. UR3, then UR4. Each is a real subtraction with a scout-sized read
   first.
4. UR5 after the operator ratifies the model. It is the only package that
   deletes a command row.
5. UR8 if wanted.

## 4. What this pass must not do

Same as the declutter phase §4. Nothing in `rs_cam_core`. No command loses
its last route without the census sentry agreeing. Enabled / disabled and
every safety count stay legible on at least one always-visible surface.
Every colour stays a token.

## 5. Two states this walk left in the live GUI

- The simulation ran at **0.60 mm manual** and unticked "Auto from tool
  size". Both persist. Re-tick Auto or set the resolution before the next
  run.
- Back Rough was generated. The source TOML was not written.
