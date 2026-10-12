# Inspector clean-up review — 2026-10-13

Operator, 2026-10-13: "the inspector panel is obscene. there is so much
text. the UI is horrible. (the graphs are the good bit)".

This file reviews the right-hand inspector (`crates/rs_cam_viz/src/ui/properties/`
and `ui/feeds/`), states the rules of the clean-up, and records the result.

## How the text was counted

The sentry `crates/rs_cam_viz/tests/the_inspector_text_is_bounded_ic1.rs`
renders the real inspector (`properties::draw`) in the 280-point panel. It
reads every painted text galley. Run it with `-- --nocapture` to print the
inventory. The fixture has one 6 mm end mill, an 18 mm softwood board, a
2D outline and a mesh, and three operations that are not generated:

- Pocket (2D), Adaptive3d ("3D Rough"), DropCutter ("3D Finish").

The measures: **lines** (visible text lines; a label, a value and a unit
on one row are one line), **blocks** (galleys that wrap to two rows or
more) and **clipped** (galleys that paint past the panel edge). The tab
strip, the name row, the Generate row and the footer are in every count.

## Before and after

"Lines / blocks / clipped" at 280 points. After IC1 every surface has 0
blocks and 0 clipped text, at 280 points and in the 240-point rail.

| Surface | Before | After (lines) |
|---|---|---|
| Pocket / Geometry | 21 / 0 / 0 | 20 |
| Pocket / Feeds & Speeds | 52 / 14 / 0 | 32 |
| Pocket / Linking | 21 / 0 / 0 | 19 |
| Pocket / Heights | 20 / 1 / 2 | 18 |
| Pocket / Dressup | 10 / 0 / 0 | 10 |
| 3D Rough / Geometry | 25 / 0 / 0 | 20 |
| 3D Rough / Feeds & Speeds | 55 / 15 / 0 | 33 |
| 3D Rough / Linking | 19 / 0 / 0 | 19 |
| 3D Rough / Heights | 19 / 1 / 1 | 17 |
| 3D Rough / Dressup | 10 / 0 / 0 | 10 |
| 3D Finish / Geometry | 29 / 2 / 3 | 18 |
| 3D Finish / Feeds & Speeds | 60 / 16 / 0 | 36 |
| 3D Finish / Linking | 23 / 1 / 0 | 19 |
| 3D Finish / Heights | 25 / 2 / 3 | 20 |
| 3D Finish / Dressup | 16 / 1 / 0 | 12 |
| Stock | 17 / 0 / 1 | 16 |
| Setup | 23 / 0 / 1 | 20 |
| Tool | 17 / 0 / 0 | 17 |
| Post | 7 / 1 / 0 | 6 |
| Machine | 20 / 1 / 2 | 18 |
| Model (2D / 3D) | 5 / 12, 0 / 0 | 4 / 11 |

Before IC1 the Heights grid also painted its fourth column (the absolute
Z) outside the 280-point panel, so egui did not paint it at all.

The fixture does not reach the text that a generation or a simulation adds.
The draw code gives these counts for a generated, simulated operation:

- The footer reach readings: up to 3 wrapped sentences (`toolpath_panel.rs`).
- The Operating point section: 4 grid rows, 1 headline, 3 rows, 1 disclosure.
- The Unified Finish rest-reference block: up to 3 sentences of 2 to 3 rows
  each (`operations/surface_3d.rs`).
- The rest-region pathology caption: 1 sentence of 2 to 3 rows.

## Worst offenders, ranked

1. **Feeds & Speeds: the stage and warning lines.** 7 to 9 lines under the
   Recommendation card, each a sentence of 2 to 5 rows: the plunge claim, the
   long-tool rule, the RPM lowered, the dial record, the feed re-derive, the
   ramp record, the entry notes, the force-line source and its extrapolation.
   Ruling R4 requires each stage on the card face. It does not require a
   paragraph.
2. **Feeds & Speeds: the Recommendation rows break.** At 280 points the
   delta tag (`↑ 4.00×`) and the recommended value wrap onto a second row.
   A legend line ("current → recommended · Δ vs current") repeats what the
   arrow shows.
3. **One finding painted twice.** On DropCutter the stale-default defect is
   a Geometry banner (title, a 3-row detail, Fix) AND a footer row (a 4-row
   sentence, Fix). Both clip.
4. **The context chip** (Tool, Material, Source) wraps to 3 rows; the
   Spindle precedence field wraps its value.
5. **Heights**: the Bottom pin note and the diagram's "Model: the drawing is
   flat…" caption clip. Each of the five rows repeats "above Stock Top".
6. **The footer reach readings** (after generation): 3 sentences.
7. **Machine**: a 3-row note on the kinematics; "Save to library" clips.
8. **Post**: the Safe Z note wraps. **Model**: "Type: Some(Svg)" prints a
   debug value. **Setup**: the "270 deg" button clips.
9. **Geometry**: an italic description line for each operation restates the
   operation name. It is help text, not state.

What already works: the diagrams (pattern, heights, entry, lead, dogbone),
the collapsed hint count (DC5), and the field rows of Linking, Dressup,
Setup, Stock and Tool.

## Design rules applied

1. A field is one row: label, value, unit. The explanation is the hover,
   keyed on the registry parameter name (`help_for`, UI-04).
2. No visible text block is longer than one line. The exception is a
   refusal or an error that blocks work.
3. A sentence that must stay on the face (ruling R4 stage lines, warnings)
   is ONE truncated line. Where the face is written in the GUI, it is short
   and starts with the numbers. The full text and the detail are on the
   hover. The painted galley keeps the full text, so the R4 sentries read
   the same record.
4. Provenance is a badge or a short dim token with the sentence on hover.
5. A warning is one line in the warn or danger colour, with its Fix button.
   A finding has one owner: the panel footer for a diagnostic.
6. Rare parameters go into ONE collapsed "Advanced" section per tab. No
   tab opens a second scroll area. The inspector width does not depend on
   the tab.
7. A diagram that summarises the tab comes first. Text that repeats the
   diagram goes.
8. The panels use `ui/components/` and `ui/tokens.rs`.
9. No capability goes. Every parameter stays settable. The command path
   (`ProjectSession::apply`, the `PanelEdit` doors) does not change.

## Tension with earlier rulings

- Ruling R4 (2026-09-24): "every stage that moves a number is one line on
  the card". Rule 3 keeps one line per stage and truncates it. Core writes
  most stage texts (`card_text()`, `rpm_lowered_text`), so a long one shows
  its first words and puts the rest on hover. Shorter core texts would show
  every number; that is out of scope here.
- G-REACHWRAP (UX-R09-001) made findings WRAP so a caveat was never cut.
  Rule 2 replaces that with one truncated line plus the full hover; the
  ellipsis says that more text exists. The reach over-statement stays a
  face line of its own.

## Done (2026-10-13)

What changed, by rule:

- **One line.** `components::OneLine` (new) truncates a sentence and puts
  the full text and the detail on hover. It carries the feeds stage and
  warning lines (`why::detail_line`, `plain_line`), the force-line lines,
  the context row, the footer reach reading, the rest-region cautions, the
  boundary summary, the Bottom pin note, the Unified Finish readouts, and
  the Post, Machine, Setup and Model notes. The footer finding row is one
  truncated line with its Fix button at the right edge.
- **Rows that fit.** The comparison rows are five fixed cells
  (`compare::rail_line`); the legend line went to the heading hover. Every
  inspector combo truncates, and `INSPECTOR_COMBO_WIDTH` replaces egui's
  100-point minimum. `label_cell` left-aligns every parameter label. The
  spindle override is `[x] [18000] RPM`; the project default is on hover.
- **One owner.** The Geometry stale-default banner is deleted; the footer
  row owns the finding.
- **Hover, not text.** The operation description moved to the Name hover.
  The "Manual stock" caption, "(library empty)", "No fixtures", "No
  keep-out zones" and "No stock changes" lines are gone. The heights row
  shows `auto` or the `Auto` button; the absolute Z is on hover (the
  diagram prints it).
- **Advanced.** One collapsed section per tab: Pocket and Profile (finishing
  passes, compensation), 2D Adaptive, VCarve, Inlay, DropCutter (Min Z,
  slopes), Adaptive3d (tolerance, min cut radius, detect flat, ordering, Z
  blend), Waterline, Scallop, Pencil, Unified Finish, Steep/Shallow, Ramp
  Finish, Project Curve, and Linking (feed-rate optimization).
- **Diagrams first.** The shape diagram, the height diagram and the entry
  diagram now open their tab or section. The flat-model caption under the
  height diagram is gone; the legend says "Model (flat)".

Tests changed on purpose:

- `the_speeds_apply_holds_the_cut_g_speedsonly`: the face is now "⚡ Speeds
  only — holds the cut" (it wrapped to two rows).
- `inspector_header_wraps_g_reachwrap`: two arms now check one line with the
  full text on hover, not a wrapped paragraph.
- `component_contracts_up2`: `boundary_2d.rs` holds 2 raw DragValues, not 3
  (Pocket and Profile share one finishing-pass row).
- New: `the_inspector_text_is_bounded_ic1` (caps from this table plus 3).

Open for the operator: see the report of 2026-10-13.

## Round 2 (2026-10-13): one table, Details, the graphs

Operator, after round 1: "if there is no recommendation, can we just not
recommend or show that line? … Operating point (measured) reads like a
block of text … there is very little consistency in how we display data
like this … simulation.. I see no graphs?"

### The component

`ui/components/data_table.rs` holds the one renderer for tabular data. It
replaces the old `DataTable` of `kv_row.rs`, a header wrapper over
`egui::Grid` that no panel used.

- `DataTable` is a list of rows in fixed columns: name, value and unit,
  status, source. The name column has one width for the whole table.
  Every row is one line of `ROW_DENSE` height. No cell wraps; a long cell
  truncates and the row hover gives the full text.
- `DataRow` has four kinds. A value row (`value`, `measured`,
  `not_measured`). A change row: current → recommended and the ratio
  (`change`). A bar from 0 to a limit (`bar`). A full-width record for a
  sentence that core writes (`record`), or a status line (`note`).
- A row with a hover shows the detail mark after its name.
- The status slot uses the `Role` set (glyph, word, role tint), in
  lower-case caption text.
- The source slot is a `ProvKind` badge or the name of a setting.
- `Details` is the one disclosure for derivation. It is closed by default,
  and its label is "Details · N" in every section.

The rule is in `ui/components/CLAUDE.md`: tabular data uses DataTable;
derivation goes in Details.

### Sections converted

- **Feeds & Speeds, Recommendation** (`feeds/compare.rs`, `feeds/why.rs`).
  The default view shows a change row only when the recommended value
  prints differently from the current value. Below the changes it shows one
  status row for each problem, then the Apply buttons; it draws the buttons
  only when a row differs. When nothing differs, it shows one row:
  "✓ at the recommendation". `Details` holds these rows:
  - the rows at the recommendation;
  - the chip verdict and the power bar, unless one is a problem (a thin or
    heavy chip, or power over 70 % of the limit);
  - MRR;
  - the context (tool, material, vendor row with its source badge);
  - the force line;
  - the row basis (plunge rule with a published-rule or repo-rule badge,
    G1/G2/G3/B5 claims, V-bit key, row material);
  - the engaged diameter, unless flagged;
  - the long-tool share, now with a repo-rule badge in place of "repo rule,
    unsourced";
  - the RPM-lowered record and the no-vendor-data records;
  - the Suggest stages: the aggressiveness record, the feed re-derive, the
    ramp feed with its source badge, and the entry notes with their rules
    on hover.

  A dial record whose target is above the base or not met stays on the face
  as a problem. The deleted renderers are `rail_row`, `rail_line`,
  `rail_cell`, `RailColumns`, `rail_efficiency_row`, `rail_power_row`,
  `rail_force_line_rows`, `rail_mrr_row`, `verdict_row_label`,
  `draw_context_chip`, `detail_line`, `plain_line`, `draw_warnings`,
  `draw_suggest_lines`, `draw_row_basis_lines`, `draw_engaged_diameter_row`
  and `draw_chipload_min_warning`.
- **Operating point (measured)** (`properties/feeds_speeds.rs`). The face
  is one row: the achieved advance per tooth, with the chipload gate
  verdict as its status. `Details` holds the commanded advance, the vendor
  band (with its badge), "Limited by", feed against commanded, modulated
  cuts, the strategy, and one row for each binding constraint. This
  replaces the "Constraint breakdown" disclosure. The deleted renderers are
  `draw_advance_per_tooth_card`, `wrapped_cell` and
  `advance_gate_verdict_text`.
- **Toolpath footer reach readings** (`properties/toolpath_panel.rs`):
  Unreachable and Max gap rows, the grid line on the hover, and the
  over-statement as a caution row.
- **Unified Finish rest reference** (`operations/surface_3d.rs`): the
  reference row has an ok or check status. Territory Clip and fresh stock
  are caution rows.
- **Model panel**: the X, Y and Z extents (range on hover) and the size and
  normal cautions are one table. The mesh, BREP and polygon counts are in
  the shared `Details`.
- **Face selection**, and the **Simulation panel** tool position and
  current operation.
- **Simulation workspace inspector** (`sim_diagnostics.rs`):
  - The limit rows (`verdict_badge` became `limit_data_row`) are table rows.
  - The Project section grids are tables: Global, Must address,
    Informational, not measured, and the top hotspots as clickable rows.
  - The focused hotspot and issue cards are tables.
  - The span facts, metrics and findings are tables.
  - The generator item and generation trace drill-downs are tables.

Not converted: the vendor LUT viewer (a 7-column table, behind its own
disclosure), the machine RPM and power rows (they share a grid with
editable rows), and the parameter editors of Geometry, Linking, Heights,
Dressup, Stock, Setup and Tool. Those editors are fields, not derived data.

### The graphs: root cause and fix

The code shows three causes. A probe on the live project was not run,
because the lead's GUI holds it.

1. **The cut-metric charts were behind a closed disclosure.**
   `draw_cut_metrics_section` opened the section by default only when a
   metric exceeded its limit (`load_with_default_open(…, summary.exceeded > 0)`).
   The open state had one id (`inspector_cut_metrics`) for every operation.
   An operation inside its limits showed one header line,
   "Cut metrics · within limits", and no chart. **Fix:** the section has no
   disclosure now. The measured charts always draw, above the table.
2. **The cards follow the playhead, not the selection**
   (`focused_toolpath()` = `current_boundary()`). After a run from MCP, the
   playhead stands at the end of the run, so the cards show the last
   operation (on the mini test piece, the front 3D finish). Selecting a
   toolpath in another workspace does not move them. **Fix:** an
   "Operation" row under the header names the operation the charts
   describe. The focus rule is unchanged; that change is for the lead to
   decide.
3. **A metric core did not measure painted a text line in place of its
   chart.** **Fix:** the abstentions are one table after the charts, one
   row each with the abstention mark and a "not measured" status. Core's
   reason is on the hover.

The cards do not abstain by operation family; only drill kinematics drops a
card. They do not read the cell-size measurability report
(`MeasurabilityReport`). Round 2 adds that report: when radial engagement,
chip engagement or air cut is `NotMeasurable` or `Degraded` for the focused
operation, the table gets a row such as "not measurable at 0.20 mm cell".
The hover gives core's `describe()` and names the Resolution control (Setup
& run). The table has no "use a finer cell" button. Core's per-tool cell
rule (`auto_tool_rule_for_radius`) is `pub(crate)`, and a cell size typed
in the GUI would be an invented number.

### Before and after (visible lines at 280 points, `ic1` fixture)

| Surface | Round 1 | Round 2 |
|---|---|---|
| Pocket / Feeds & Speeds | 32 | 22 |
| 3D Rough / Feeds & Speeds | 33 | 22 |
| 3D Finish / Feeds & Speeds | 36 | 25 |
| Every other surface | as round 1 | unchanged |

The `ic1` fixture is not generated or simulated. On a simulated operation,
Operating point goes from 4 grid rows, 1 headline, 3 rows and 1 disclosure
to 1 row and `Details`. The `ic1` Feeds caps are now 25 / 25 / 28 (the
after-state plus 3).

### Tension with earlier rulings

- **Ruling R4 (2026-09-24), "every stage that moves a number is one line
  on the card".** It is now one row in the card's `Details`. The row is
  closed by default (lead ruling 2, 2026-10-13). `g_visible` still finds
  every record as one run when the disclosures are open.
- **Lead ruling 6, fields not text.** The Suggest records, the entry notes,
  the plunge basis and the force line come from core as text
  (`card_text()`, `EntryNote::headline`). They stay one record row each,
  with the full text and the rule on the hover. The "no wood or router
  source" clause in the entry notes is core text, so it stays a sentence.
  The plunge basis and the ramp record get a source badge from their enum
  variant. A field-by-field table for the aggressiveness record would need
  a rewrite of `g_visible`, which joins its numbers by run; that is a
  follow-up.

### Tests

New: `tabular_data_reads_one_table_ic2`, with three arms:

- After the apply, a level DOC or WOC row is not drawn by default, and is
  drawn once in `Details`.
- The derivation runs paint only when `Details` is open.
- A source scan: each derived-data file builds `DataTable` rows, and the 15
  deleted renderers stay deleted.

Changed on purpose (a parser follows the new shape):

- `the_power_row_names_its_force_line_b6`, `the_card_names_every_entry_rule_g10`:
  a record row with a hover ends in the detail mark.
- `the_recommendation_explains_each_row_g_whyrow` (ld1): the long-tool row
  is derivation, so it carries no warning mark.
- `inspector_header_wraps_g_reachwrap`: the readings are `DataRow`s.
- `a_tool_size_reads_diameter_and_radius_g_toolsize`: the context row is in
  `Details`.
- `the_inspector_nests_once_dc5`: the non-vacuity anchor also accepts
  `Details::new(`, and no tab may carry the Details label.
- `component_contracts_up2`: the new `DataTable` call; `compare.rs` holds
  1 hand-rolled chain (was 3); the `sim_diagnostics.rs` row is gone (0).
- `cut_metric_cards_read_the_gate_g_cutcards`,
  `the_limit_rows_read_their_own_bound_g_ownbound`: the abstention and the
  limit rows are table rows.
- `the_inspector_text_is_bounded_ic1`: the Feeds caps are tighter.

## Round 3 (2026-10-13): tabs, the default width, N/A

Operator, after round 2: "looks better! far better. now to clean up more.
can we de-clutter by moving the sections into tabs. default to showing cut
metric graphs. Tabs for warning (with the count as an icon) tab for
navigation and similar? Also default the side bar to be opened fully (it
default clips the right side, you dont see things like '-- Not
measurable'). Also instead of that not measurable can you just replace the
-- with 'N/A' and remove the extra tag on the right."

### The tabs

The Simulation inspector (`ui/sim_diagnostics.rs`) keeps the heading, the
verdict line, the air-cut caution and the freshness chip at the top. Under
them is ONE tab strip. A focused hotspot or issue card sits above the strip,
so its Jump, Clear, Prev and Next are on every tab. The tab is
`SimulationState::inspector_tab` (runtime state, not egui memory, not
saved with the project). The tab is the disclosure, so no section inside a
tab is closed.

| Tab | What it holds |
|---|---|
| **Metrics** (default) | The cut-metric section: the Operation row, the charts, the abstention table, Other limits. When the operation has no chart, its limit rows. |
| **Warnings** | The triage items (safety, then actions) as jump rows, or "Nothing to act on"; the not-judged rows; the Findings pills; the not-measured strip; Must address; Informational; Top hotspots (open, clickable). |
| **Navigate** | Now playing (the operation and Jump to start); the Span block with its lock, facts, metrics and in-span findings; Generator item; Generation trace. |
| **Project** | Cycle time with its basis, moves, operations, cut and rapid distance. |

The Warnings label carries the count of the triage's safety and action
items: "Warnings ⚠ 3". The badge is in the danger colour when one item is
a safety item, in the caution colour otherwise, and absent at 0. The count
is the triage's, never `issue_count` (DC6). New: the triage items are now
listed as rows. Before, only the first one was on the verdict line.

The strip is `ui/components/tab_strip.rs` (`TabStrip`, `TabBadge`), made
from the toolpath inspector's strip in `properties/tab_badges.rs`, which
now calls it. The workspace bar keeps its own tabs (chrome; UP3 scans that
file). `DESIGN_SPEC.md` §4.15 describes the component.

The toolpath inspector (`ui/properties/`) already has a tab strip, and its
simulation readouts are single sections, not a stacked layout. It is
unchanged apart from the shared strip.

### The default width

`app.rs` `SIM_DIAGNOSTICS_DEFAULT_WIDTH` = 304 points (was 240). The
sentry `the_sim_inspector_is_tabbed_ic3` draws the Metrics tab of the
simulated `limits_fixture` pocket through `side_panel` and sweeps from 240
in 8-point steps. It reads the painted galleys: an elided run (`truncate`
cut it) or a run past the content edge fails the width. The measurement:

| Width | Cut short |
|---|---|
| 240 | the card peaks ("peak 41 % of limit"), the cell-size rows, "Radial engagement", "Chip engagement" |
| 264 | 3 peaks, "Radial engagement", "Chip engagement" |
| 280, 288 | "Radial engagement" (the 42 % name column) |
| 296 | nothing cut; 6 right-aligned runs 1.16 points past the edge |
| 304 | nothing |

The sentry holds the constant to at least the measured width and at most
one step wider. The user's own resize still persists (egui `PanelState`).
At a 1600-point window the two Simulation rails take 240 + 304 points.

### N/A

`DataTable` (`components/data_table.rs`): a `DataRow::not_measured` value
cell paints "N/A" (`NOT_MEASURED_TEXT`) in `UNKNOWN`. The row has no status
or source slot. A status set on it becomes the first line of the row hover
("Not measurable at 0.50 mm cell"), then the reason. Every table follows,
because the rule is in the component. `NotMeasured` and `KeyValueRow` keep
the dash (`tokens::GLYPH_UNKNOWN`; UP2 holds it).

In `sim_diagnostics.rs`:

- A limit row that is not modelled (for example gantry push: "no
  machine-side thrust rating is published") is an N/A row. Its face keeps
  the dash in the name (`the_gantry_row_paints_a_dash_g_ownbound`).
- A cell-size row that is `Degraded` was measured, so it shows the word
  "degraded at 0.50 mm cell" as its value in the caution colour, not N/A.

### Tests

New: `the_sim_inspector_is_tabbed_ic3` (6 arms): the default tab is Metrics
and paints the four card titles and none of the other tabs' headers; each
tab paints its own anchor; the badge counts and roles; the painted label
"Warnings ⚠ 3" and no badge at 0; a not-measured row paints "N/A" and no
chip, with the status word on the hover; the default width shows the
Metrics tab whole, with the sweep above.

Changed on purpose:

- `every_side_panel_fits_its_width_g_panelfit`: `WIDTHS` also holds
  `SIM_DIAGNOSTICS_DEFAULT_WIDTH`.
- `component_contracts_up2`: the component set holds `TabStrip` (13 to 14).
- `the_simulation_page_is_summary_first_dc6`: no arm changed. The header
  states the new contract: the verdict line opens the page, and the Metrics
  tab is the default and shows the charts first.

### Open for the operator

- The cards still follow the playhead, not the selection (round 2, cause 2).
- The Warnings list shows the triage's safety and action items. Advisories
  are only in the verdict line, as before.
- A timeline focus does not switch the tab; the focused card shows above
  the strip on every tab.
