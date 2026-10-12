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
