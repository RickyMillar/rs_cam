# More menu review (R5c)

Status: the review and the clear fixes, 2026-10-29. The operator ruling
(live look round 5): "review the whole menu; it needs work; check what is
valid when and how the UI could be polished." The row ids and the MCP
fields do not change. Section 4 lists the questions for the operator.

Source of each row: `ROWS` in `crates/rs_cam_viz/src/ui/overlays/registry.rs`.
The More popover: `draw_more` in `crates/rs_cam_viz/src/ui/viewport_overlay.rs`.
The sentry of the counts: `more_lists_a_row_only_where_it_can_apply_g_vpdock`.

## 1. The counts

| Measure | Before (21b1dc275) | After |
|---|---|---|
| Rows in More, Setup | 37 | 24 |
| Rows in More, Toolpaths | 37 | 28 |
| Rows in More, Simulation | 37 | 29 |
| Rows that can never draw in the workspace but show there (greyed) | 13 in Setup, 9 in Toolpaths, 8 in Simulation | 0 |
| Rows with no renderer that show in More | 2 | 0 |
| Rows ready (not greyed) where nothing draws | 5 | 0 |
| Labels that are a code name | 3 in the catalogue, 3 in More | 0 |
| Label that names the wrong thing | 1 (`curves`) | 0 |
| Hover texts with development history or code words | 4 | 0 |
| Section titles over an empty section | 0 (the hide would leave Inspect empty in Setup) | 0 (`more_rows` hides the title) |

The registry holds 43 rows; the bar owns 6 (`BAR_ROWS`); More holds the
other 37. The catalogue still lists all 43 in every workspace, greyed with
the reason where a row cannot draw.

## 2. The mechanism

A row now has a scope (`RowScope`: `Any`, `Only(ws)`, `Not(ws)`, `Never`):
the workspaces in which its renderer can ever draw. The compiler holds it,
because every row literal must name one.

- `OverlayRow::precondition` is THE precondition: the row's own condition
  first (its precise reason wins), then the scope. A row whose own
  condition holds out of its scope is refused with the scope reason
  ("draws in the Simulation workspace only"). The field is private, so no
  reader can skip the scope.
- `OverlayRow::listed_in(ws)`: More lists the row only in its scope; a
  caption group (Model colour, Stock colour) shows only where its rows
  apply; a section with no listed row shows no title (`more_rows`).
- The catalogue and MCP are unchanged in what they list; MCP refuses an
  out-of-scope row with the reason, as it refuses any not-ready row.

## 3. Every row in More

"Valid" is when the row can draw. "Fix" is a change in this commit;
"Q" refers to section 4.

### View

| Row | Today's precondition | Valid | Label, hover | Finding and fix |
|---|---|---|---|---|
| `orientation_gizmo` View cube | always | every workspace | right (R5b) | none |

### Scene

| Row | Today's precondition | Valid | Label, hover | Finding and fix |
|---|---|---|---|---|
| `model` Model | refused in Simulation; needs a 3D model | Setup, Toolpaths | hover said "Applies to both draw loops" (a code word) | Fix: scope `Not(Simulation)`, hover "The imported STL or STEP surface." |
| `origin_axes` Origin axes | always | every workspace | right | none |
| `datum` Datum crosshair | needs a setup with a datum | every workspace | right | none |
| `obstacles` Obstacles | needs an enabled obstacle | every workspace | right | none |
| `alignment_pins` Alignment pins | needs a pin | every workspace | right | none |
| `flip_axis` Flip axis | needs a flipped Bottom setup | every workspace | right | none |
| `pin_ghost` Wrong-orientation ghost | needs pins that do not key | every workspace | the label is long (Q7) | none |
| `curves` Curves (DXF/SVG) | a drawing or STEP edges | every workspace | the label named DXF/SVG; the row also gates the STEP edge lines | Fix: label "Curves and edges"; the curves-off reason names the new label |
| `selection_outline` Selection outline | refused in Simulation, model off, no mesh, wireframe | Setup, Toolpaths | right | Fix: scope `Not(Simulation)` |
| `selection_states` Selection states | curves on; a toolpath with a selection | where a toolpath is in focus | right; close to "Selection outline" (Q8) | none |
| `open_path_ends` Open path ends | curves on; an open path | where a toolpath is in focus | right | none |
| `derived_rest_regions` Derived rest regions | never (no renderer) | none | hover said "the render work is ledgered, not built" | Fix: scope `Never` (out of More), hover "no renderer draws them yet" (Q4) |
| `boundary_outline` Boundary outline | curves on; a Selection boundary | where a toolpath is in focus | right | none |
| `simulated_stock` Simulated stock | Simulation with results | Simulation | right | Fix: scope `Only(Simulation)`; its group placement is Q1 |

### Paths

| Row | Today's precondition | Valid | Label, hover | Finding and fix |
|---|---|---|---|---|
| `all_toolpaths` Draw Selected / All | a generated toolpath | every workspace | right | none |
| `entry_markers` Entry markers | a selected toolpath | where a toolpath is in focus | right | none (Q3) |
| `height_planes` Height planes | a selected toolpath | where a toolpath is in focus | right | none |
| `tool_profile_ghost` Tool-profile ghost | a selected toolpath | where a toolpath is in focus | right | none |
| `move_colour_palette` Palette | a generated toolpath | where moves draw | right | none |
| `move_colour_engagement` Engagement | a generated toolpath | where moves draw | right | none |
| `move_colour_advance_per_tooth` Advance / tooth | a generated toolpath and a simulation | where moves draw | right | none |
| `span_entry` Spans: Entry | Palette, a generated toolpath | Palette only | right | none (Q9) |
| `span_lead_out` Spans: LeadOut | as above | Palette only | "LeadOut" is the enum name | Fix: "Spans: Lead-out" (More: "Lead-out") |
| `span_link_bridge` Spans: LinkBridge | as above | Palette only | "LinkBridge" is the enum name | Fix: "Spans: Link bridge" (More: "Link bridge") |
| `span_dressup` Spans: DressupArtifact | as above | Palette only | "DressupArtifact" is the enum name | Fix: "Spans: Dressup" (More: "Dressup") |

### Inspect

| Row | Today's precondition | Valid | Label, hover | Finding and fix |
|---|---|---|---|---|
| `reach_map` Model colour: Reach | Toolpaths, model on, a finishing op | Toolpaths | right | Fix: scope `Only(Toolpaths)`; the Model colour group (with Off) shows in Toolpaths only |
| `rest_heatmap` Model colour: Rest | Toolpaths, a rest grid | Toolpaths | right | Fix: scope `Only(Toolpaths)` |
| `tier_map` Tier map | Toolpaths, a planner preview of the active setup | Toolpaths | right | Fix: scope `Only(Toolpaths)` |
| `planner_islands` Planner islands | never ("drawn by Inspect ▸ Tier map") | none | right | Fix: scope `Never` (out of More) (Q4) |
| `area_regions` By Area regions | Toolpaths, a By Area 3D Adaptive | Toolpaths | right | Fix: scope `Only(Toolpaths)` |
| `stock_colour_solid` Stock colour: Solid | a simulation result | Simulation (the simulated stock draws there only) | right | Fix: scope `Only(Simulation)`. Before, it read ready in Toolpaths and Setup after a run, and nothing changed on screen. The Stock colour group shows in Simulation only |
| `stock_colour_deviation` Stock colour: Deviation | a result with deviations | Simulation | right | Fix: as above |
| `stock_colour_by_height` Stock colour: By height | a simulation result | Simulation | right | Fix: as above |
| `tool_deflection` Tool deflection | Simulation with results | Simulation | right | Fix: scope `Only(Simulation)` |
| `generator_steps` Generator steps | a recorded trace | Simulation (the timeline band and the outline draw there only) | right | Fix: scope `Only(Simulation)`. Before, it read ready in Toolpaths with a trace |
| `active_step_highlight` Highlight active step | a trace and Generator steps on | Simulation | right | Fix: as above (Q2) |

### Hover texts with development history (fixed)

- `stock_box`: "…before P6 the gate required a MESH and the checkbox did
  nothing on an SVG or DXF project." → "The stock as a wireframe box, from
  the stock settings. It draws on a 2D job too."
- `stock_solid`: "Split from the stock box — one checkbox used to remove the
  box, the block and the axes together." → "The stock as an opaque block."
- `model` and `derived_rest_regions`: see the tables.

## 4. Questions for the operator

1. **The simulated stock and its colour are in two sections.** In
   Simulation the Simulated stock row (with its opacity slider) is in
   Scene, and the Stock colour choice is in Inspect. One "Stock" group in
   Simulation would put the stock, its opacity and its colour together.
2. **Generator steps and Highlight active step are one choice.** The second
   row only applies when the first is on. It could be a child check under
   the first, shown only while the first is on.
3. **Paths in Setup.** Toolpaths draw in the Setup workspace, so the Paths
   rows apply there and stay. Should Setup draw toolpaths at all? If not,
   Paths would leave More in Setup.
4. **The two rows with no renderer** (`planner_islands`,
   `derived_rest_regions`) now show only in the catalogue, greyed. Delete
   them from the registry (and from the MCP overlay id list) until a
   renderer exists?
5. **Model colour "Off".** The Off choice clears Reach and Rest. Reach is
   the Toolpaths default; is Off needed in More, or is the catalogue
   enough?
6. **Selection outline in Setup.** It is on by default in Toolpaths only;
   it applies in Setup too. Keep it listed there?
7. **"Wrong-orientation ghost"** is a long label for a pin check. A shorter
   label: "Pin ghost (wrong turn)"?
8. **"Selection states" beside "Selection outline".** The names are close
   but the rows draw different things (drawing state colours versus a face
   outline). A rename: "Drawing states"?
9. **The span filter applies in Palette only.** The four span checks could
   show only while Palette is the move colour, under it.
10. **Collisions on the bar outside Simulation.** The bar row is greyed in
    Setup and Toolpaths until a collision check runs; that bar control was
    not in this review's scope (it is not in More).
