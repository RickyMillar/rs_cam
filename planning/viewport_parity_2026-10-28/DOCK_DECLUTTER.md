# Dock declutter — proposal (FOLLOW_UPS §2)

Status: PROPOSAL for the operator's review, 2026-10-29. No product file
changes with this document. The counts are at HEAD `76fb3a2f4`.

## 1. What the dock is today

The dock is a floating bar at the bottom centre of the 3D view
(`crates/rs_cam_viz/src/ui/viewport_overlay.rs`). It has four text
sections in a fixed order, `View`, `Scene`, `Paths`, `Inspect`, and a last
button that opens the "All viewport options" catalogue. A click on a
section opens its popover. Each popover lists every registry row of its
section (`registry::rows_in_section`), so a row in the registry is always in
one popover and in the catalogue (sentry
`the_viewport_dock_reaches_every_option_g_vpdock`).

To switch the grid or the rapids, the operator opens a popover and finds a
row among 13 to 17. That is the clutter.

## 2. The counts at HEAD

The registry holds **43 rows** (`ROWS` in
`crates/rs_cam_viz/src/ui/overlays/registry.rs`). The catalogue lists all
43 in every viewport workspace (Setup, Toolpaths, Simulation); the Readiness
workspace renders no viewport.

| Dock section | Rows |
|---|---|
| View | 1 |
| Scene | 17 |
| Paths | 13 |
| Inspect | 12 |
| **Total** | **43** |

| Workspace | Rows on by default | Rows with a workspace default (on or off) |
|---|---|---|
| Setup | 10 | 21 |
| Toolpaths | 16 | 21 |
| Simulation | 5 | 22 |
| Readiness | 0 | 0 |

"On by default" counts the rows whose `default_for` gives `Some(true)` in
that workspace. Two more rows start on with no workspace default (`model`
and `curves`: their flags are `true` in `ViewportState::new` and
`default_for` is `no_default`). The other 22 rows (`no_default`) keep the
value the operator gave them.

The script that gives these numbers (run it from the repository root; it
reads `registry.rs` only and mirrors `registry::dock_section`):

```sh
python3 -c "import re;s=open('crates/rs_cam_viz/src/ui/overlays/registry.rs').read();R=re.findall(r'OverlayRow \{\s*id: \"([^\"]+)\",\s*group: OverlayGroup::(\w+),.*?surface: OverlaySurface::(\w+),.*?default_for: (\w+),',s[s.index('pub const ROWS'):],re.S);D={f:{w:v for a,v in re.findall(r'((?:Workspace::\w+(?: \| )?)+) => (Some\(\w+\)|None)',b) for w in re.findall(r'Workspace::(\w+)',a)} for f,b in re.findall(r'fn (\w+_default)\(ws: Workspace\) -> Option<bool> \{(.*?)\n\}',s,re.S)};sec=lambda i,g,u:'View' if i=='orientation_gizmo' else 'Scene' if i in('derived_rest_regions','boundary_outline','simulated_stock') else 'Paths' if u=='Moves' or g=='Toolpath' else 'Scene' if g=='Geometry' else 'Inspect';print(len(R),'rows;',{k:sum(sec(i,g,u)==k for i,g,u,_ in R) for k in('View','Scene','Paths','Inspect')});[print(w+': on by default',sum(D.get(f,{}).get(w)=='Some(true)' for *_,f in R),'| a workspace default',sum(D.get(f,{}).get(w,'None')!='None' for *_,f in R)) for w in('Setup','Toolpaths','Simulation','Readiness')]"
```

Its output at `76fb3a2f4`:

```text
43 rows; {'View': 1, 'Scene': 17, 'Paths': 13, 'Inspect': 12}
Setup: on by default 10 | a workspace default 21
Toolpaths: on by default 16 | a workspace default 21
Simulation: on by default 5 | a workspace default 22
Readiness: on by default 0 | a workspace default 0
```

The section counts agree with the vpdock sentry (View 1, Scene 17,
Paths 13, Inspect 12).

## 3. The proposed grouping

Two levels:

1. **The bar: one-click controls.** The controls the operator switches
   most, each one click with no popover:
   - the display-mode control (Wireframe · Shaded · Edges, FOLLOW_UPS §1;
     today at the top of the Scene popover);
   - `grid` (Grid);
   - `stock_box` (Stock — box), shown as "Stock";
   - `cutting_moves` (Cutting moves);
   - `rapids` (Rapids);
   - `collisions` (Collisions).

   Each toggle reads and writes its registry row through the shared row
   renderer (`panel::draw_control`), so the precondition, the reason and
   the workspace defaults stay one source. A toggle that cannot draw is
   disabled, and its hover gives the row's reason, as in a popover.
2. **"More": one popover.** Every other row, grouped under today's section
   titles (View, Scene, Paths, Inspect) in today's order, with today's
   section suffixes (`Paths: Selected`, `Inspect: Reach`) on the group
   titles. The camera controls (presets, projection, Reset view, Fit
   selection) stay at the top of the View group. The last line opens the
   "All viewport options" catalogue, as the last dock button does today.

The catalogue does not change: it lists all 43 rows by section, with the
search box and the `All` / `Changed` / `Cannot draw` filter.

## 4. Text mockup

The bar (bottom centre of the 3D view), in Toolpaths:

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Wireframe|Shaded|Edges]  ☑ Grid  ☑ Stock  ☑ Cutting moves  ☑ Rapids  ☐ Collisions  │  More ▴ │
└──────────────────────────────────────────────────────────────────────────────┘
```

In Setup the moves toggles are off by default (`moves_default`) and the
Collisions toggle is disabled with the row's reason; in Simulation the
geometry toggles are off by default (`geometry_default`) and Collisions is
on (`simulation_only_default`).

The "More" popover (opens upward from `More ▴`):

```text
┌ More ─────────────────────────────────────────┐
│ VIEW                                          │
│  [Top] [Front] [Right] [Iso]                  │
│  Projection  (Perspective) (Orthographic)     │
│  [Reset view] [Fit selection]                 │
│  ☑ Orientation gizmo                          │
│ SCENE                                         │
│  ☑ Model          ☐ Stock — solid             │
│  ☑ Origin axes    ☑ Datum crosshair           │
│  ☑ Obstacles      ☑ Alignment pins            │
│  ☑ Flip axis      ☑ Wrong-orientation ghost   │
│  ☑ Curves (DXF/SVG)                           │
│  ☑ Selection outline  ☑ Selection states      │
│  ☑ Open path ends                             │
│  ☐ Derived rest regions  ☐ Boundary outline   │
│  ☐ Simulated stock   Opacity [────●──]        │
│ PATHS: Selected                               │
│  (Selected) (All)                             │
│  ☑ Entry markers  ☑ Height planes             │
│  ☐ Tool-profile ghost                         │
│  Move colour (Palette) (Engagement) (Adv/tooth)│
│  Spans ☐ Entry ☐ LeadOut ☐ LinkBridge ☐ Dressup│
│ INSPECT: Reach                                │
│  Model colour (Reach) (Rest)                  │
│  Stock colour (Solid) (Deviation) (By height) │
│  ☐ Tier map  ☐ By Area regions                │
│  ☐ Planner islands                            │
│  ☐ Tool deflection                            │
│  ☐ Generator steps  ☐ Highlight active step   │
│ ───────────────────────────────────────────── │
│ All viewport options…                         │
└───────────────────────────────────────────────┘
```

The two-column layout in the popover is a mockup choice; the build can keep
one row per line if a narrow viewport needs it.

## 5. The rows that move

Out of a section popover onto the bar (5 rows and 1 control):

| Row / control | Today | Proposed |
|---|---|---|
| display-mode control | top of the Scene popover (and the catalogue Scene section) | the bar, first |
| `grid` | Scene popover | the bar |
| `stock_box` | Scene popover | the bar, label "Stock" |
| `cutting_moves` | Paths popover | the bar |
| `rapids` | Paths popover | the bar |
| `collisions` | Inspect popover | the bar |

Into the one "More" popover, under their present section title (the other
38 rows): the 1 View row (`orientation_gizmo`); 15 Scene rows (`model`,
`stock_solid`, `origin_axes`, `datum`, `obstacles`, `alignment_pins`,
`flip_axis`, `pin_ghost`, `curves`, `selection_outline`,
`selection_states`, `open_path_ends`, `derived_rest_regions`,
`boundary_outline`, `simulated_stock` with its opacity slider); 11 Paths
rows (`all_toolpaths`,
`entry_markers`, `height_planes`, `tool_profile_ghost`, the three move
colours, the four span rows); 11 Inspect rows (`rest_heatmap`,
`reach_map`, `tier_map`, `area_regions`, `planner_islands`, the three
stock colours, `tool_deflection`, `generator_steps`,
`active_step_highlight`).

Check: 6 bar items = 5 rows + the control; 5 + 1 + 15 + 11 + 11 = 43 rows.

No row leaves the registry, and no row changes its id, its default or its
precondition. MCP `set_ui_view overlays` and `display_mode` do not change.

## 6. What the build must keep (if the operator takes this)

- One home per row: a bar row is not also listed in "More" (the vpdock
  sentry changes from four popovers to the bar plus one popover, and
  still proves that the bar and "More" together cover the registry).
- The catalogue still lists every row.
- The section suffixes keep their glyph rules (`SuffixStatus`), now on the
  group titles in "More".
- The bar takes no width from the 3D view (an `egui::Area`, as today) and
  never covers the legend or the gizmo.

## 7. Questions for the operator

1. Collisions on the bar in every workspace (disabled with a reason outside
   Simulation), or only in Simulation?
2. "Stock" on the bar: the box only, or a three-way (Off · Box · Solid)
   that also takes `stock_solid`?
3. Entry markers: on the bar beside Rapids, or in "More"?
4. Keep the section popovers as a second way in, or "More" only? The
   rule "declutter removes a control; it does not hide a duplicate route"
   (`crates/rs_cam_viz/src/ui/CLAUDE.md`) says "More" only.
