# rs_cam GUI — premium design audit

Date: 2026-09-13. Branch: `master` at `78bb7b01`.
Crate under review: `crates/rs_cam_viz` (egui 0.34.3, wgpu 29).
Author role: design lead.

## 0. How this audit was made

Two kinds of evidence appear below. The words are exact.

- **"I read"** — a statement about source. Every one carries a `file:line`.
  No cargo command ran. This session does not own the cargo lane.
- **"I saw"** — a statement about the running product. The release binary
  `target/release/rs_cam_gui --mcp` (built 2026-09-13 17:30) loaded
  `~/Downloads/wanaka200/wanaka200.toml`. I generated `Back Rough`
  (24 527 moves), ran one simulation at a 0.6 mm cell, and captured 16 PNG
  files into this directory. Each finding names its file.
  The session made no save and no export.

The window measured 1600 x 1000 logical points, except `shot_15` and
`shot_16`, which measured 1400 x 900.

### Screenshot index

| File | Surface |
|---|---|
| `shot_01_setup_on_load.png` | Toolpaths workspace, first frame after load |
| `shot_02_setup_workspace.png` | Setup workspace, nothing selected |
| `shot_03_toolpaths_geometry.png` | Toolpaths, Geometry tab, 3D Rough |
| `shot_04_generating_state.png` | The same panel while the lane computes |
| `shot_05_feeds_tab.png` | Toolpaths, Feeds & Speeds tab |
| `shot_06_heights_tab.png` | Toolpaths, Heights tab and height diagram |
| `shot_07_dressup_tab.png` | Toolpaths, Dressup tab, path drawn |
| `shot_08_linking_tab.png` | Toolpaths, Linking tab and entry diagram |
| `shot_09_readiness.png` | Readiness workspace |
| `shot_10_tool_library_modal.png` | Tool Library window |
| `shot_11_export_wizard.png` | Export Wizard window, step 1 |
| `shot_12_simulation.png` | Simulation workspace at t = 0 |
| `shot_13_simulation_end.png` | Simulation workspace at the last move |
| `shot_14_simulation_metrics.png` | Simulation timeline with the signal strip |
| `shot_15_feeds_modal.png` | Feeds & Speeds modal at 1400 x 900 |
| `shot_16_machine_panel_1400.png` | Setup workspace, Machine Setup panel |

### What this audit does not do

It proposes no behaviour change. It renames no control, moves no control
between panels and changes no number, threshold or default. Those questions
belong to `planning/ui_fix_2026-09-09/` and to the IA items D1-D6, which
stay open. This audit reads the product as a visual instrument only.

### Findings already closed elsewhere, and not repeated here

I read `planning/ui_fix_2026-09-09/PLAN.md` and `STATUS.md`. The following
are closed and are excluded: the inspector header wrap (F1.15), the feeds
card labels (F1.3), the boundary checkbox (F1.5), the properties-panel
placeholder wording (F1.13), the MCP toast tense (F1.1) and the per-field
pill value (F1.2). The open IA items (named findings, the context header,
decision-first forms) are information architecture and stay with that
programme. Where a visual finding below touches one of those surfaces, it
says so.

---

## 1. The global picture

**The product has a token layer and a component layer, and most of the
GUI does not use them.** That is the single structural fact behind almost
every finding in this audit.

- I read `crates/rs_cam_viz/src/ui/theme.rs:1-50`. It declares **20 colour
  constants** and one frame helper, `card_frame`.
- I read `crates/rs_cam_viz/src/ui/components/mod.rs:20-34`. It exports
  **nine components**: `CompareRow`, `Freshness`, `FreshnessGate`,
  `CountPill`, `PrecedenceField`, `ProvenanceBadge`, `SummaryCard`,
  `UiExt`, `SuggestButton`, `ValueRow`.
- `grep -c Color32:: crates/rs_cam_viz/src` returns **473**. The theme
  module holds 20 of them. **453 colour literals sit outside the token
  module.** The three heaviest files are
  `ui/properties/mod.rs` (96), `ui/properties/operations/mod.rs` (72) and
  `ui/sim_timeline.rs` (54).
- 31 of the crate's 128 source files name `theme::` at all.

So the work is not "invent a design system". The work is **finish the one
that exists, widen it to cover type, spacing, elevation and motion, and
migrate the 453 sites**.

### D-01 The type scale has two rungs, and 77 % of text sits on the lower one

I read `crates/rs_cam_viz/src/app.rs:1060-1082`, `configure_theme`. It sets
ten `Visuals` colours and one spacing value, `item_spacing = (6, 4)`. It
sets **no text styles at all**. The crate therefore runs egui's defaults,
which I read at `egui-0.34.3/src/style.rs:1402-1406`:

| Style | Size |
|---|---|
| `Small` | 9.0 |
| `Body` | 13.0 |
| `Button` | 13.0 |
| `Heading` | 18.0 |
| `Monospace` | 13.0 |

Counted over `crates/rs_cam_viz/src`:

| Call | Count |
|---|---|
| `RichText::new` | 670 |
| `.small()` | 516 |
| `.strong()` | 164 |
| `.size(N)` | 2 |
| `ui.heading(` | 34 |
| `.monospace()` | 8 |

**516 of 670 styled strings render at 9.0 points.** A 9-point Ubuntu-Light
glyph on a dark ground is the smallest readable text most desktop products
ship, and this product spends it on the sentences that carry the most
meaning. I saw the effect in `shot_03_toolpaths_geometry.png`: the tool-load
caution, "Chipload clamped to rubbing floor: 0.0183 -> 0.0250 mm/tooth
(post-derate chipload below chip-formation threshold; expect honest output
above floor instead of ploughing recipe)", is the most important sentence on
the panel and it is the smallest type on the panel. I saw the same in
`shot_09_readiness.png`: the cycle-time caution, which contains the words
"SEVERAL TIMES faster than the machine", renders at 9 points below a
26-point banner that says only "REVIEW BEFORE CUTTING".

The scale has effectively two rungs, 9 and 13, plus a rarely used 18. A
premium instrument needs four to six rungs, and its floor must be 11.

### D-02 The typeface is egui's default, and it is a light weight

I read `crates/rs_cam_viz/src/app.rs:1085-1111`, `configure_fonts`. It
starts from `FontDefinitions::default()` and appends two Noto symbol
fallbacks. It replaces neither family. I read
`epaint-0.34/src/text/fonts.rs:330-360`: the default proportional face is
**Ubuntu-Light** and the default monospace face is **Hack**.

Ubuntu-Light is a *light* weight. At 9 and 13 points on a near-black ground
its stems thin out and the text loses contrast. `.strong()` is the only
weight axis available, and it is a synthetic bolding of a light face, so
"strong" and "normal" sit closer together than they should.

The crate already ships font assets. I read
`crates/rs_cam_viz/assets/fonts/` and it holds two `.ttf` files loaded with
`include_bytes!`. **A typeface change therefore adds no Cargo dependency.**

### D-03 Numbers do not align, and the product is made of numbers

Only 8 sites call `.monospace()`. Every measured value in the inspector, the
readiness rows, the diagnostics panel and the status bar renders in the
proportional face. I saw this in `shot_03_toolpaths_geometry.png`: the value
column reads `1.2 mm`, `5.0 mm`, `4.00 mm`, `0.10 mm`, `0.0 mm`, and the
decimal points do not line up.

**A constraint the spec must carry: egui 0.34 exposes no OpenType feature
switch**, so the `tnum` figure set cannot be turned on for a proportional
face. Tabular figures in this toolkit come from the monospace family or from
nowhere. That makes "every measured value renders in the mono family" a
design rule, not a preference.

### D-04 There is no button hierarchy anywhere in the product

Counted: 85 `ui.button(` sites and 47 `Button::new` sites, so **132 buttons**.
Of those, **3 carry a custom fill** — one each in `ui/preflight.rs`,
`ui/workspace_bar.rs` and `ui/properties/mod.rs`. The other 129 render as
egui's default grey.

The consequence is visible in every capture. In `shot_09_readiness.png` I
saw `Export G-code…` and `Run simulation` side by side at identical weight;
the workspace exists to answer one question and it does not say which button
answers it. In `shot_03_toolpaths_geometry.png` I saw `Generate`, the primary
action of the whole panel, drawn exactly like the `Reset to recommended`
button on the Dressup tab.

### D-05 Nothing in the product moves

`grep -c "animate_bool\|animate_value" crates/rs_cam_viz/src` returns **0**.
`grep -c Shadow` returns **0**.

Every state change in this GUI is a hard cut: a hover, a tab switch, a
disclosure, a toast arriving, a toast leaving. I read the toast site at
`crates/rs_cam_viz/src/app.rs:918-957`: the frame appears at full opacity and
vanishes at full opacity, and `request_repaint_after(1 s)` means a toast can
stay up to one second past its own TTL. No element in the product carries a
shadow, so a window, a toast and a popup sit on the same plane as the panel
behind them.

Motion and elevation are the two cheapest signals of quality in a desktop
application, and this product spends neither.

### D-06 Six corner radii and twelve spacing steps

Radii in play: **2** (egui's widget default, read at
`egui-0.34.3/src/style.rs:1628-1673`), **6** (egui's window default, read at
`:1502`), and **3, 4, 5, 6** from the crate's own `corner_radius(N)` calls
(4 appears 12 times, 6 three times, 3 three times, 5 once).

`add_space(N)` appears **318 times across 12 distinct values**: 4.0 (122),
8.0 (84), 2.0 (38), 6.0 (37), 12.0 (20), 10.0 (6), 18.0 (5), 28.0 (2),
16.0, 14.0, 40.0, 48.0.

A 4-point grid is already latent — 4.0 and 8.0 are 206 of the 318 calls —
but 2.0, 6.0, 10.0, 14.0 and 18.0 break it.

### D-07 Two background colours, declared two ways

I read `crates/rs_cam_viz/src/app.rs:1063-1064`: `panel_fill` and
`window_fill` are `rgb(30, 30, 36)`. I read `app.rs:346-350`, `:377-381`,
`:432-436` and `:508-512`: all four workspace layouts give the
`CentralPanel` a frame filled `rgb(26, 26, 38)` as an inline literal,
repeated four times.

So the viewport ground is not the panel ground, the difference is 4 levels
of blue, and neither value lives in `theme.rs`. The hue bias of the whole
product — a faint violet — is an accident of two literals that were never
compared.

### D-08 One status vocabulary, two visual forms on one row

I read `crates/rs_cam_viz/src/ui/toolpath_panel.rs:679-714`, `status_chip`.
It is a clean pure function over `FreshnessState` with seven states and
theme colours: `PEND`, `GEN`, `OK`, `STALE`, `WAIT`, `OFF`, `ERR`. It is the
best piece of state design in the product.

I read its draw site at `toolpath_panel.rs:409-418`. It renders as
`ui.label(RichText::new(text).small().strong().color(c))` — **plain text at
9 points, with no fill, no padding and no border.** Two lines later, at
`:443`, `draw_trace_badge` draws `TRACE` as a bordered chip. I saw both on
one row in `shot_03_toolpaths_geometry.png`: `OK` is loose text and `TRACE`
is a box. The less important signal carries the stronger form.

I read `toolpath_panel.rs:446-451`: the operation name renders in
`Color32::from_rgb(190, 190, 200)`, which is neither `theme::TEXT_STRONG`
(200, 205, 220) nor `theme::TEXT_HEADING` (180, 180, 195). There are three
"primary text" greys.

### D-09 Section headers have four forms

I read `ui/components/section.rs:35-42`. `UiExt::named_section` is the
canonical header: `.small().strong().color(TEXT_HEADING)`. Its own doc
records that the audit before this one found the same idiom inlined 105
times.

In `shot_05_feeds_tab.png` alone I saw four header forms in 300 vertical
points:

1. `▼ Geometry` — an egui `CollapsingHeader` at body size.
2. `Feeds & Speeds` — a collapsing header at body size.
3. `SPEED — how fast` — 9-point grey with a bare em dash.
4. `Derived` and `Machining Boundary` — 9-point grey, no rule, no spacing
   above.

A reader cannot learn which mark means "this is a group" because four marks
claim it.

---

## 2. Setup workspace

Draw sites I read: `app.rs:307-355` (`draw_setup_layout`),
`ui/setup_panel.rs` (left), `ui/properties/mod.rs` and
`ui/properties/setup.rs` / `stock.rs` (right).
Evidence: `shot_02_setup_workspace.png`, `shot_16_machine_panel_1400.png`.

### D-10 The right panel is 280 points wide, 900 points tall and holds one line

I saw `shot_02_setup_workspace.png`. Nothing is selected. The right panel
renders the single italic sentence "Select an operation, tool, setup or
model" at the top, then roughly 880 points of flat dark grey.

I read the draw site at `ui/properties/mod.rs:594-598`. It is one
`RichText` with `.italics()`.

The sentence is correct — F1.13 fixed its wording — but an empty state is
not a sentence. On the product's largest resting surface the reader gets no
title, no icon, no shape and no next action. I counted 49 `.italics()` sites
across the crate; the same pattern repeats in `setup_panel.rs:100`, `:227`,
`:238` and `toolpath_panel.rs:107` ("No toolpaths"), `:203` ("No tools
defined").

**Empty is the first state every operator meets, and the product treats it
as an error message.**

### D-11 The setup card mixes four type roles in 70 points of height

I saw `shot_02_setup_workspace.png`. The `Setup 1` card carries, top to
bottom: the name at body weight, `[Bottom]` in blue 9-point, then
`Orient: Bottom   XY: Corner (Front-Left)` in blue 9-point, then `Pins: 2`
in grey 9-point, then `Flip 180 deg on X axis` in italic 9-point. The
`Setup 2` card ends with `⚠ uncut stock` in amber 9-point.

Five different treatments say five different things, and four of them are
9 points. The card has no interior structure: no label column, no rule, no
grouping. The eye has to read every line to find the one that matters.

### D-12 The Machine panel is the only panel with a proper title

I saw `shot_16_machine_panel_1400.png`. "Machine Setup" renders as an
18-point heading with a horizontal rule below it. No other right-hand panel
does this. The Toolpath inspector opens on `Name:` and a text box
(`shot_03`), and the Simulation inspector opens on the word `Inspector`
(`shot_12`).

Three panels occupy the same rectangle in three workspaces and title
themselves three ways.

### D-13 Read-only and editable rows are indistinguishable until the eye reaches the value

I saw `shot_16_machine_panel_1400.png`. `RPM Range: 8000 - 24000` and
`Power: 1.50 kW` are plain labels. `Max Feed: [10000 mm/min]` and
`Max Shank: [6.35 mm]` are edit boxes. The four labels are identical in
colour, size and position, so the only cue that two rows are editable is the
box, which sits 90 points to the right of the label.

The same mixture appears in every properties panel I captured.

---

## 3. Toolpaths workspace

Draw sites I read: `app.rs:389-437` (`draw_toolpath_layout`),
`ui/toolpath_panel.rs` (left), `ui/properties/mod.rs` (6 440 lines) and
`ui/properties/operations/mod.rs` (2 691 lines) (right).
Evidence: `shot_03`, `shot_04`, `shot_05`, `shot_06`, `shot_07`, `shot_08`.

### D-14 The inspector is a flat list of 25 rows with no hierarchy

I saw `shot_03_toolpaths_geometry.png`. Below the Geometry tab the panel
renders, in one column and one weight: Stepover, Depth/Pass, Stock to Leave,
Tolerance, Min Cut Radius, Entry Style, Helix Radius, Helix Pitch, Fine
Stepdown, Detect Flat, Ordering, Strategy, Z Blend, Mill Shallow, then a
preview box, then Machining Boundary with three more rows, then Rest
Analysis.

`Stepover` decides the finish of the part. `Mill Shallow` is a checkbox most
operators never touch. They are drawn identically. Nothing on the panel is
primary.

The IA programme's D3 ("decision-first forms") owns the question of which
rows belong in a disclosure. This finding is narrower and is purely visual:
**even with today's row order, a weight and a rule would separate the four
rows that set the cut from the ten that refine it.**

### D-15 The label column has two indents on one panel

I saw `shot_03_toolpaths_geometry.png`. `Tool:` and `Input:` start at
x = 1253. `Stepover:` and every row below start at x = 1237. The two groups
sit inside the same panel with no visible reason for the step.

### D-16 Trailing text is clipped, and the panel resizes between tabs

I saw `shot_05_feeds_tab.png`. Two strings are cut by the panel edge:
`configured 0.0833 mm/b` (the word is `mm/tooth`) and `default | 18C` (the
value is `18000`). Both are the *comparison* value — the number that makes
the recommendation meaningful.

I also measured the panel's left edge across the captures: 1232 on the
Geometry tab (`shot_03`), 1185 on the Feeds tab (`shot_05`), 1180 on the
Dressup tab (`shot_07`). **The inspector changes width when the operator
changes tab**, because its content drives its width against
`SIDE_PANEL_MAX_WIDTH = 420.0` (read at `app.rs:107`). The viewport resizes
under the cursor as a side effect of a tab click.

F1.15 wrapped the inspector *header*. These are value-row trailing notes and
they are a different site.

### D-17 The layout jumps when generation starts

I compared `shot_03_toolpaths_geometry.png` with
`shot_04_generating_state.png`. In the first, the line "This operation
requires manual generation. Press G or click Generate." sits under the gates
line. In the second it is gone, and every element below it — `Hints (1)`,
the whole Geometry group, the tab strip and all 25 rows — has moved up by
about 22 points.

Pressing Generate moves the control the operator's pointer is over.

### D-18 "Computing…" is a word, not a state

I saw `shot_04_generating_state.png`. The panel reads `Generate  Computing…`.
There is no spinner beside it, no progress, no elapsed time and no disabled
treatment on the Generate button. The elapsed time exists — I saw
`TP running · Back Rough (3D Rough) · 10.5s` in the status bar, 940 points
away at the bottom of the window, and `Back Rough (3D Rough)  Cancel All` in
the top strip, 900 points above.

The three parts of one event are drawn in three corners of the screen, and
the one the operator is looking at carries the least.

### D-19 The row action icons are unlabelled single letters

I saw `shot_03_toolpaths_geometry.png`. The selected card reveals six small
square buttons: an eye, `C`, `R`, a bullseye, a power glyph and a loop
glyph. `C` and `R` are cutting moves and rapids. I read
`ui/toolpath_row_controls.rs` as the draw site.

Six 18-point squares of identical size and weight, two of them bare capital
letters, are the densest control cluster in the product and the least
legible. They also appear only on the selected row, so their meaning cannot
be learnt by comparison.

### D-20 The Heights diagram uses five fully saturated hues

I saw `shot_06_heights_tab.png`. The diagram draws `CZ` in pure blue, `RZ`
in cyan, `FZ` in green, `TZ` in yellow and `BZ` in red, each with a
two-letter label at 8 points. The same five colours carry the product's
safety meanings elsewhere: red is a collision, amber is a caution, green is
"within".

**A height plane is not a verdict.** Spending the verdict palette on an
ordered set of five heights teaches the operator that colour does not mean
anything in this product.

### D-21 The Dressup tab is four rows in a 900-point panel

I saw `shot_07_dressup_tab.png`. Below the tab strip the panel holds
`3/8 dressups active`, a `Reset to recommended` button, the label
`Path Quality`, one checkbox with one child field, and one more checkbox.
Roughly 780 points below that are empty.

The Linking tab (`shot_08`) and the Heights tab (`shot_06`) have the same
shape. Four of the five tabs use less than a third of the panel; the
Geometry tab overflows it. The tab strip promises five equal places and
delivers one crowded one and four near-empty ones.

### D-22 Two controls on two tabs carry one name and two values

I saw `Entry Style: Helix` on the Geometry tab (`shot_03`) and
`Entry Style: None`, greyed, on the Linking tab (`shot_08`), for the same
selected operation in the same session. The Linking control gives no reason
for being disabled.

I have not established which is authoritative, and this audit proposes no
change to either. It is recorded because two controls with one label are a
visual defect whatever the underlying rule is, and because a disabled
control with no reason breaks the rule the Overlays panel already keeps
(every disabled row states why).

---

## 4. Simulation workspace

Draw sites I read: `app.rs:439-530` (`draw_simulation_layout`),
`ui/sim_op_list.rs` (left, 1 283 lines), `ui/sim_diagnostics.rs` (right,
1 828 lines), `ui/sim_timeline.rs` (bottom, 2 285 lines).
Evidence: `shot_12`, `shot_13`, `shot_14`.

### D-23 The viewport is the loudest surface in the product

I saw `shot_13_simulation_end.png`. The simulated stock renders in a
saturated orange-brown and the toolpath renders in a near-pure green, at
full opacity, over the whole part. The result is the highest-chroma image
the application can produce, and it carries no information that a calmer
image would not.

I read `crates/rs_cam_viz/src/render/colors.rs:9-18`. `TOOLPATH_PALETTE`
holds eight maximally separated hues at high chroma: blue, green, orange,
purple, yellow, cyan, red, lime. I read `STOCK_SOLID_FACE` at `:47` as
`[0.65, 0.50, 0.30]`.

The architecture here is good — the 3D colours are already centralised in
one module. The **values** are a category wheel, and a category wheel spends
every hue the semantic layer needs. Toolpath 6 is red. Red is also
`COLLISION_POINT` at `colors.rs:51`.

### D-24 The timeline is a progress bar

I saw `shot_12_simulation.png` and `shot_13_simulation_end.png`. The
timeline is one full-width bar that fills green from left to right as
playback advances, with an orange tail. It has no time axis, no tick marks,
no operation boundaries and no labels. In `shot_14_simulation_metrics.png` a
second, thinner blue bar appears below it and does carry per-operation
segments, but it is unlabelled and half the height.

The product's most spatial dataset — 25 199 moves across three operations
over 1:26:45 — is drawn as a loading indicator.

### D-25 The signal chart has no scale and no legend

I saw `shot_14_simulation_metrics.png`. Below the timeline the panel draws
`advance/tooth vs band max`: a filled blue area, a filled tan area, orange
spikes and a red dashed line. The y axis carries the single label `1`. The x
axis carries `0`, `10000`, `20000` with no unit. Nothing says what blue is,
what tan is or what the dashed line is.

The same panel prints `▶ Signal graphs (5)` at the very bottom, cut off by
the window edge.

### D-26 "No cutting metrics for this run" is wrong about its own scope

I saw `shot_13_simulation_end.png`. The message reads "No cutting metrics
for this run." with a `Re-run` button below it. The run produced metrics for
two of three operations. The playhead was inside `Holes`, a drill, and drill
operations publish drill-native metrics instead of engagement metrics.

The sentence says "run" where it means "operation", and it offers a `Re-run`
that would change nothing. This is a visual-copy defect on a surface whose
whole job is honesty about what was measured.

### D-27 The inspector ends in a paragraph of help text

I saw `shot_12_simulation.png`. Below `Selected: —` the panel prints the
header `View` and then four lines of 9-point grey prose: "Stock opacity,
stock and move colour modes, collisions, the deflection panel and the
generator-step overlay are in the viewport 'Overlays' panel (shortcut: O).
Per-toolpath cutting / rapid visibility: each row's C / R."

This is a permanent signpost to controls that moved. It occupies the bottom
of the product's densest panel on every frame, forever.

### D-28 One badge set is drawn twice, 700 points apart, in two grammars

I saw, in the right Inspector of `shot_12_simulation.png`:
`✓ within 2/8` as a green outlined pill, `⚠ unmodeled 6/8` as an amber
outlined pill, and `collisions 5` as plain red text with no pill.

I saw, in the transport bar of the same capture:
`✓ load 2/8`, `⚠ unmodeled 6/8`, `collisions 5 →`, `traces 3` — four pills,
including an actionable one.

Two surfaces report overlapping counts in two forms. Within one of those
surfaces, one of three sibling counts is not a pill. I read
`ui/components/pill.rs:1-30`: `CountPill` already distinguishes `Verdict`
from `Observation` and `ReadOnly` from `Actionable`. The Inspector's
`collisions 5` does not use it.

### D-29 The operation card packs three unrelated facts on one 9-point line

I saw `shot_12_simulation.png`. The `Back Rough` card's second line reads
`⚠ rapid × 5   +4   ⊙ 98% planned`. Three separate measurements — a safety
count, an unexplained `+4`, and a feeds-provenance percentage — share one
line, one size and one baseline, distinguished only by two glyphs.

The safety count is the reason this workspace exists. It is the same size as
the `+4`.

---

## 5. Readiness workspace

Draw site I read: `app.rs:357-387` (`draw_readiness_layout`) and
`ui/readiness_panel.rs`.
Evidence: `shot_09_readiness.png`.

### D-30 The workspace uses 35 % of the window and centres everything

I read `app.rs:381-386`: the central panel calls `ui.vertical_centered` and
then `ui.set_max_width(560.0)`. I saw the result. At 1600 points wide the
content occupies a 560-point column in the middle and 1040 points are empty
black. There is no left panel, no right panel and no viewport.

A centred 560-point column is a web article layout. This is the screen an
operator reads immediately before cutting a part.

### D-31 Every row is amber, so no row is urgent

I saw six rows, each led by an identical amber `⚠` and each with amber
value text: Operations, Simulation, Rapid collisions, Holder clearance, Tool
load, Est. cycle time.

Two of those are safety questions. Two are procedure steps that have simply
not been run yet ("Not run", "Not checked"). One is an estimate quality
note. The product spends one colour and one glyph on all five meanings.

A "not yet run" step and an "unanswered safety check" are different states
and need different marks. The banner above them says `⚠ REVIEW BEFORE
CUTTING` in the same amber, so the summary and every detail share one tone.

### D-32 The row actions form a ragged column

I saw the buttons `Toolpaths`, `Run sim`, `Re-check`, `Run sim` right-
aligned to the column edge at x = 1077. Their left edges land at 1017, 1029,
1022 and 1029. Two rows have no button at all, so the column has holes.

### D-33 The two closing actions have equal weight

I saw `Export G-code…` and `Run simulation` as two default grey buttons side
by side. The banner has just said the job needs review. Nothing says which
button the operator should press, and the destructive-in-effect one — export
— is on the left, in first reading position.

---

## 6. Windows, modals and overlays

I read twelve `egui::Window::new` sites outside tests:

| Window | Draw site |
|---|---|
| Project Load Warnings | `app.rs` |
| Unsaved Changes | `app/export.rs` |
| Export Wizard | `ui/export_wizard.rs` |
| Export Readiness (pre-flight) | `ui/preflight.rs` |
| Feeds & Speeds | `ui/feeds_modal.rs` |
| Optimize — *toolpath* | `ui/optimize_modal.rs` |
| Optimize project | `ui/optimize_project.rs` |
| Tool Library | `ui/tool_library_modal.rs` |
| Machine Library | `ui/machine_library_modal.rs` |
| Plan multi-tool finishing | `ui/multitool_planner.rs` |
| Overlays | `ui/overlays/panel.rs` |
| Keyboard Shortcuts | `ui/shortcuts_window.rs` |

### D-34 No window is visually modal

I saw `shot_10_tool_library_modal.png` and `shot_11_export_wizard.png`. Both
windows float over a fully lit viewport with a pure-green toolpath behind
them. Neither dims the background. Neither carries a shadow — `grep Shadow`
returns 0 crate-wide. The window fill is `rgb(30, 30, 36)`, which is the
panel fill, so on the left edge of `shot_10` the window boundary is nearly
invisible against the panel behind it.

I read one exception at `ui/optimize_project.rs:572-574`: that window alone
sets a dimmed `Visuals` and restores it. So the product knows how to do
this, in one place, for one window.

### D-35 The load-warnings window covers the workspace switcher

I saw it in **every one of the sixteen captures**. The Project Load Warnings
window opens near the top-left corner and covers the workspace tab bar and
the left panel's own header. It stayed open through a generate, a
simulation, four workspace switches and three modal opens. It is the first
thing the operator sees and it hides the control they need next.

I read the draw site at `crates/rs_cam_viz/src/app.rs`. It is a plain
`egui::Window` with no anchor away from the chrome.

The three warnings it carries are serious — two alignment pins outside the
stock, a flip that cannot be re-seated, a pattern that does not key the
orientation. They deserve a surface that is not in the way.

### D-36 The Export Wizard's seven steps are a text breadcrumb

I saw `shot_11_export_wizard.png`. The steps render as
`1. Post › 2. Output layout › 3. Coordinate & units › 4. Tool change &
spindle › 5. Setup pauses › 6. Preview & validate › 7. Save`. The current
step is a filled blue rectangle; the other six are plain text at body size.

Nothing distinguishes a completed step from a future one. Seven labels
averaging 17 characters consume the full width of the window at body size,
so the step names compete with the step content for attention. `◀ Back`,
`Next ▶` and `Cancel` are three default grey buttons, and `Next` — the only
one the operator wants — is not the strongest.

### D-37 The Feeds modal is the most designed surface and the most crowded

I saw `shot_15_feeds_modal.png` at 1400 x 900.

- The nomogram draws nine colours: red, pink, orange, green, yellow, blue,
  magenta, grey and white.
- Labels collide. `max 6000 mm/min` overlaps `max 24000 RPM` at the top
  right. `VENDOR BAND`, `iso-advance 0.0548 mm/tooth` and `safe derate`
  overlap at the lower left.
- The legend, headed `What the colours mean`, is a ten-row table and its
  last row is cut off by the window edge.
- The window is taller than the 900-point viewport and is clipped at the
  bottom with no outer scrollbar visible.

The recommendation table on the left is the best component in the product:
four aligned columns, `Current` against `Recommended`, with a delta column.
It should be the model for every comparison in the application. It is
trapped inside one modal.

### D-38 The Overlays panel is the product's own best pattern

I read `ui/overlays/registry.rs:43-48` and counted **40 `OverlayRow`
entries** in four groups: Geometry, Toolpath, Regions, Analysis.

I could not photograph the panel — it opens from a viewport button and from
the `O` key, and neither is reachable over MCP. I record it from source
because the rule it implements is the one this whole audit wants:
**every row is always listed, a row that cannot draw is greyed, and the grey
row states the reason and, where one exists, carries the button that fixes
it.**

That rule should govern every disabled control in the product. Today it
governs 40 rows in one window. `Entry Style` on the Linking tab (D-22) is
greyed with no reason, six rows away from a panel that would never allow it.

---

## 7. Chrome

### D-39 The workspace bar hides its own hint text at 9 points

I read `ui/workspace_bar.rs:31-40`. The bar right-aligns
`current.hint()` in `.small()` and `theme::TEXT_FAINT` — 9-point text in the
faintest grey the theme declares. I saw the results: "Operations, tools,
generation", "Stock, orientation, workholding", "Verify, animate, export",
"Is this safe to cut?".

These four strings are the clearest statement of purpose in the product and
they are rendered to be unread.

I read `ui/workspace_bar.rs:88-115`. The active tab is drawn with two
literals, `rgb(65, 72, 95)` and `rgb(220, 225, 240)`, neither in `theme.rs`,
plus a hand-painted indicator line and an asymmetric corner radius
`{nw: 4, ne: 4, sw: 0, se: 0}`.

### D-40 The status bar is a pipe-separated sentence

I read `ui/status_bar.rs:24-40`. It formats
`"Models: {}  |  Triangles: {}"` as one string with literal pipes, then adds
`Toolpaths: {}/{}` after an `ui.separator()`. So one row carries two
different separators: a text pipe and a drawn rule.

I saw the result: `Models: 4 | Triangles: 661212 | Toolpaths: 3/9 | SIM | 5
collisions`. `661212` is printed without digit grouping. `5 collisions` is
red, next to `Modified` in italic on the far right (`shot_16`). Five
different meanings share one type size and one baseline.

### D-41 The status bar block is copied four times

I read `app.rs:336-344`, `:364-372`, `:414-422` and the simulation layout.
The same eight lines — lane snapshot, collision count, `status_bar::draw`,
separator, status message in `Color32::from_rgb(255, 200, 80)` — appear four
times, once per workspace, with the colour literal spelled out each time.

---

## 8. What is already right, and must survive

A redesign that breaks these would be a regression.

1. **`render/colors.rs` is a real token module for the 3D layer.** The
   values need retuning; the structure does not.
2. **`status_chip` is a pure function over seven states** with correct
   colour semantics and hover text that explains itself
   (`toolpath_panel.rs:679-714`). Only its rendering is weak.
3. **`CountPill` already separates verdict from observation and read-only
   from actionable** (`ui/components/pill.rs`). It needs adoption, not
   redesign.
4. **The Overlays registry rule** — always list, grey with a reason, carry
   the fix (`ui/overlays/registry.rs`).
5. **The `Current / Recommended / Δ` table in the Feeds modal.**
6. **Honest absence.** The product prints `—` for a value it did not
   measure, `NOT MEASURED` strips, and `Tool load: advance/tooth — power —
   L/D —`. That discipline is rare and valuable. The spec must give absence
   a *designed* form rather than remove it.

---

## 9. Ranked summary

| Rank | Finding | Why it ranks here |
|---|---|---|
| 1 | D-01, D-02, D-03 type | 516 of 670 strings at 9 pt in a light face. It touches every screen and every finding below it. |
| 2 | D-23, D-20 colour discipline | The verdict palette is spent on non-verdicts, in the viewport and in the Heights diagram. |
| 3 | D-04 no button hierarchy | 129 of 132 buttons identical. No screen names its primary action. |
| 4 | D-30, D-31 Readiness | The pre-cut screen uses a third of the window and one colour for five meanings. |
| 5 | D-05 no motion, no elevation | Zero animate calls, zero shadows, no modal scrim. |
| 6 | D-10 empty states | 49 italic one-liners standing in for designed empty states. |
| 7 | D-14, D-16, D-17 the inspector | Flat 25-row list, clipped comparison values, layout jump on Generate. |
| 8 | D-24, D-25 simulation readout | The richest dataset drawn as a progress bar and an unlabelled chart. |
| 9 | D-35 load-warnings window | Covers the workspace switcher in all 16 captures. |
| 10 | D-06, D-07, D-08, D-09 token drift | 12 spacing values, 6 radii, 2 backgrounds, 3 text greys, 4 header forms. |
