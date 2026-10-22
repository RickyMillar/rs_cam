# UX review by workflow (2026-10-22)

Owner: the lead session. Reader: the operator. Status: planning only. No code
changes come from this file until the operator rules the open questions in
section 9.

## 0. Why this review

The operator reviewed the geometry selection work (P0–P6) on screen on
2026-10-22. The operator said:

> "Overall I think we need a ux review for different workflows"

The other words of that review:

> "the picker isnt ideal for steps. it just selects faces, the edges of faces
> are also loops! and the corners are points! but I cannot seem to select
> this?"

> "I also did a profile on 2 faces, it seems like it cut both to the same
> height? not to the face? im confused."

> "i would have thought that because I didnt select the circle in the op. it
> would not have been considered?" (now ruled O-PICK: a pick replaces the
> default rule)

> "add toolpath Bore ... it does nothing. no side panel opens" (a refused add;
> a fix is in progress in the working tree)

> "if you look at the screenshot it missed a corner" (a hole plus the tool
> radius left wall material, and nothing said so)

This file follows six workflows through the code as it is at HEAD 1aeb5373
(plus the uncommitted add-op fix where it says so). Each workflow has three
parts:

1. The journey: the clicks, the panels, and what the operator must know.
2. The friction table, with a rank for each row.
3. The proposal: the smallest change that fixes it, and whether it is GUI
   only or needs core.

### 0.1 Method and limits

- The lead and three read-only agents read the code, the plans and the
  instruction files. Nobody drove the running GUI (the operator uses it).
- A click path that the code does not prove is marked "not verified in code".
- File references use two short roots:
  - **V** = `crates/rs_cam_viz/src/`
  - **C** = `crates/rs_cam_core/src/`
- Line numbers are at HEAD. The file
  `V/controller/events/toolpath.rs` has uncommitted edits, so this review
  cites its behaviour, not its line numbers.

### 0.2 Ranks

| Rank | Meaning |
|---|---|
| **Blocks the job** | The operator cannot make the part with the GUI. |
| **Wrong cut risk** | The GUI can emit a cut that the operator did not intend, and nothing says so clearly. |
| **Confusing** | The operator can make the part, but must know a hidden rule or meets two names for one thing. |
| **Cosmetic** | The text or the layout is poor; the cut is not affected. |

### 0.3 The rulings that bind every proposal

- **"Auto magic is a wizard"** (PLAN.md, 2026-10-22). A feature that proposes
  ops, selections or tools opens as a wizard: its own draft, one apply, then
  ordinary ops. Entry points: the setup `+` menu, the empty-state button, the
  File menu. The main panels get no new rows or buttons for it. A one-line fix
  action that shows only with its note is allowed (the P6.7 "Add a Bore for
  these" pattern).
- **O-PICK** (PLAN.md, 2026-10-22). On an op whose selection is still the
  default rule, the first pick clears that rule. Later clicks add or remove.
  Shift-click or `+ Rule` adds a layer. Reset brings the default back. The
  same rule holds on every surface.
- **D2/D13**: the default rule takes the kinds the op can cut; an item that
  the user names and the op cannot cut refuses.
- **P8 waits** for the operator review of P0–P7. No proposal here starts P8.

---

## 1. Workflow: a 2.5D sign or plaque from a DXF or SVG

### 1.1 The journey today

1. File ▸ Import a DXF or SVG. The import dialog opens
   (`V/ui/import_modal.rs:199` "Import drawing — {name}"). It shows the
   declared size for an SVG (`:205`), a Scale combo (`:88`), the Result size
   (`:233`), "Join gaps up to" (`:132`) and a Report row that turns caution
   while open shapes stay (`:147-157`). Import (`:265`).
2. The drawing draws at once (P4.D). The Setup rail has an eye per model
   (`V/ui/setup_panel.rs:194-218`).
3. Add the tools. The tool list order matters (see 1.2, row 1.F1).
4. The setup header `+` (`V/ui/toolpath_panel.rs:1136`) or the empty-state
   "Add toolpath" (`:1149`) opens the menu. The 2.5D group reads "2.5D (from a
   drawing or STEP faces)" (`:1157`).
5. Add Pocket. The new op takes:
   - the first model of the project (add flow in
     `V/controller/events/toolpath.rs`);
   - the first tool that Suggest accepts (working tree; HEAD takes the first
     tool of the list).
6. The Geometry tab opens with the tool and Input rows
   (`V/ui/properties/tab_badges.rs:497-515`), then the Selection section
   (`V/ui/properties/toolpath_panel.rs:584-614`).
7. The Selection section shows the default rule "This model · every layer ·
   every item it can cut (default)" (`V/ui/properties/selection_section.rs:577-622`, `:731`).
   The Cuts row counts the items (`:293-339`).
8. To limit the op to the letters, the operator must do one of these:
   - `+ Rule` (`:226-243`), tick a layer, Add. Under the default, the first
     rule replaces the default (`C/selection/rule.rs:339-371`).
   - Pick (`:256-274`), then shift-click one letter: a layer rule, which also
     replaces the default.
   - Pick, then click a letter: this ADDS a pick to the default rule (see
     1.F2).
9. Profile for the cut-out: Side Outside, "Tabs" with Count and hand tabs
   ("Edit in viewport", `V/ui/properties/operations/boundary_2d.rs:585-639`).
   The floor row "Floor from:" (`boundary_2d.rs:53-150`) needs "Stock bottom"
   and a depth for a through cut.
10. V-carve the text (VCarve, `boundary_2d.rs:750-769`) or follow single-line
    text with Trace (`V/ui/properties/operations/engrave.rs:14-60`):
    "On the line", "Left of the drawn direction", "Right of the drawn
    direction".
11. Generate each op (the row dot, or "Generate All",
    `V/ui/toolpath_panel.rs:450-503`).

What the operator must know:

- The default rule cuts every item that the op can cut, on every layer.
- A click adds; a shift-click replaces (until O-PICK lands).
- The tool order in the project decides the tool of a new op.
- "Stock bottom" is the through-cut reference; "Top Z" is the default.
- A Profile Inside on a ring with a hole cuts two walls (review note N1,
  `planning/tech_debt_2026-10-19/GUI_REVIEW_JOURNEYS.md`).

### 1.2 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 1.F1 | A new op takes the first tool of the project. A V-bit first in the list gives a Profile a V-bit (N2) and makes Bore refuse. | HEAD `tools().first()`; working-tree fix and test `crates/rs_cam_viz/tests/a_new_op_takes_the_first_tool_it_accepts.rs` | Wrong cut risk |
| 1.F2 | A click adds a pick to the default rule, so the op still cuts every other item. The operator expected "only what I picked". | `C/session/selection_status.rs:1018-1048` (replace only on the stock default, L-6); ruled O-PICK, not built | Wrong cut risk |
| 1.F3 | A click and a shift-click act by two rules under the default: the rule replaces, the pick adds. | `C/selection/rule.rs:339-371` against `selection_status.rs:1018-1048` | Confusing |
| 1.F4 | The tool cannot reach a corner between a wall and a near hole. The op leaves material and says nothing (N4, "it missed a corner"). | PLAN.md corner note; no finding exists | Wrong cut risk |
| 1.F5 | A through cut needs the operator to know "Stock bottom" in "Floor from:". The default "Top Z" with a depth does not follow the stock thickness. | `C/compute/floor.rs:68-85`; the "Through cut of a N mm board" line shows only after the floor reaches the stock bottom (FEATURE_CATALOG Profile row) | Confusing |
| 1.F6 | The `+ Rule` card and the kind chips are hard to understand (operator, parked 2026-10-20). | `selection_section.rs:925-1091`; `V/ui/pick_bar.rs:73-81` | Confusing |
| 1.F7 | "Island" on a Profile hole ring reads "the op cuts round it". On a near hole the Profile cuts a second wall, which surprised the operator (N1). | `C/selection/view.rs:189`; GUI_REVIEW_JOURNEYS N1 | Confusing |
| 1.F8 | Trace Left/Right on a closed loop has no side mark. The side ring shows only with the selection-states overlay on. | PLAN.md P6.7a gaps | Confusing |
| 1.F9 | The Selection section, the Machining Boundary "Selection" source and the pick bar title "▸ Selection \| Machining boundary" use one word for two things. | `selection_section.rs:158-163`; `V/ui/properties/toolpath_panel.rs:773-847`; `pick_bar.rs:119-126` | Confusing |
| 1.F10 | The HoleFit note (one per circle) and the size rows both show. | PLAN.md P6.7a gaps | Cosmetic |

### 1.3 Proposal

- **1.F1**: finish the working-tree fix: take the first tool that the op
  accepts; refuse only when no tool fits, and name the tool kinds. GUI only
  (the acceptance rule is the core Suggest door). Also grey a tool in the
  Tool combo that the op does not accept, with the reason on hover
  (`tab_badges.rs:497-510` lists every tool today). GUI only.
  - Residual: a Profile accepts a V-bit, so the fix still binds a V-bit
    first in the list to a Profile (N2). The add picks a tool that runs, not
    the tool that the operator intends. Mitigation within the wizard rule:
    the success toast names the choice ("Profile added · tool T0 V-bit"),
    so the operator sees it without opening the row. GUI only.
- **1.F2, 1.F3**: build O-PICK in core: `pick_edit` gives `Replace` with the
  one pick while the op selection equals its op default. This is the L-6 rule
  of the Face op (`selection_status.rs:1018-1048`), made general. One core rule
  gives GUI, MCP and CLI parity. Needs core.
- **1.F4**: a generation finding "Material is left at N corner(s): the tool
  does not fit between the wall and <item>", with Show me. Measure the cut
  ring against the region boundary at generation (PLAN.md corner note). Needs
  core. The finding shows as a note row in the Selection section, so no new
  control.
- **1.F5**: no new control. The P7 job types (Sign, Sheet parts) set "Stock
  bottom" and ask for the depth (P7 ruling O-2). In the plain panel, the floor
  combo hover already names each reference. Lead decision: no change in the
  main panel.
- **1.F6**: rename the chips and the card rows (see section 7.3). GUI only.
- **1.F7**: run the held generation test (PLAN.md "HOLD the Profile Island
  role fix"), then rename the role for Profile to "Wall round it". Core
  (role text) and GUI.
- **1.F8**: draw the side mark for a Trace Left/Right op always, not only with
  the overlay on. GUI only.
- **1.F9**: see section 7.3.

---

## 2. Workflow: a part from a STEP model

The operator had not demoed BREP before today. This workflow has the most
friction.

### 2.1 The journey today

1. File ▸ Import STEP. No dialog opens; only DXF and SVG open one
   (`V/controller/events/mod.rs:327,330`; `V/controller/io.rs:143`). A later
   rescale of a STEP refuses (`V/controller/io.rs:237`).
2. The STEP source gives **face items only**: one flat `Face` per BREP face
   group (`C/io/mod.rs:263-308`, set at `:234-235`). It gives no loop, no open
   path, no circle and no point (`C/selection/item.rs:89-95`).
3. Add a 2.5D op (enabled, because the model has face items:
   `add_menu_offers`, `V/ui/toolpath_panel.rs:1187-1200`). The default rule
   takes no face (`KindFilter::Served` admits no `Face`,
   `C/selection/rule.rs:24-40`). So a new op refuses `Empty`, and the section
   shows "Fix: pick a face of the model." (`selection_section.rs:65`,
   `:208-217`).
4. Pick, then click a face. The face picks by the triangle hit and its face
   group (`V/interaction/picking.rs:233-287`).
5. Heights: the auto top moves to the highest picked up-face ("auto: faces",
   `C/session/heights.rs:80-89`, `:319-331`). The floor stays "Top Z" with
   its depth. A hint under the section offers "Each picked face"
   (`V/ui/properties/toolpath_panel.rs:1236-1279`): "Faces set the top. To cut
   down to each face, choose Each picked face."
6. Pocket a step: pick the floor face, choose "Each picked face" depth 0, and
   the pocket clears down to that face (P3.5).
7. Profile of a step: pick a face. Profile cuts the outline of that face
   (`ItemUse::Regions`, `C/compute/catalog/registry.rs:128`). Its floor is
   "Top Z" (the face top less the depth), or "Each picked face" (that face's
   own plane).
8. Chamfer of an edge: Chamfer is a `Regions` op
   (`C/compute/catalog/registry.rs:629`, `:671`). It can chamfer the whole
   outline of a picked face. It cannot chamfer one edge.
9. Holes from cylinders: Drill and Bore read `Circle` and `Point` items only
   (`C/selection/view.rs:60-90`). A STEP gives none. A blind hole has a flat
   floor face, so a Pocket on that face can mill it (not verified by a run).
   A through hole has no item at all.
10. 3D ops: a face click on a 3D op switches its boundary to Selection, with
    the notice "Boundary set to Selection: 1 face." (L-8 revised,
    `C/session/selection_status.rs:168-196`;
    `V/controller/events/mod.rs:135-155`, `:190-193`).

What the operator must know:

- A STEP gives faces only. An edge or a corner is not an item.
- A new 2D op on a STEP starts refused until a face pick.
- A face pick moves the TOP, not the floor, until "Each picked face" is set.
- "Each picked face" means "down to the plane of the face that I picked". For
  a Profile round the top of a step, that is no depth at all.

### 2.2 The measured case (PLAN.md, operator review 2026-10-22)

`stepped_block.step`, a Profile on faces 3 and 5, floor "Top Z" depth 12:

| Fact | Value |
|---|---|
| Stock top | Z 32 |
| Auto top ("auto: faces", the highest picked face) | Z 20 |
| Floor of both faces (20 − 12) | Z 8 |
| First level | Z 18 |
| Peak axial depth of cut (narrate) | 14.00 mm against 2.00 mm per pass |
| Heavy engagement | 72 % |

So the two faces cut to one height because the floor reads the one op top.
And the first level buries the tool, because the auto top sits 12 mm below the
stock top with no prior op. The rule is `apply_rule`
(`C/session/heights.rs:301-333`). It sets `top_z` to the face top with no check
against the material that stands. The same function already keeps the stock
top under "Each picked face" (L-15, `:310-316`).

### 2.3 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 2.F1 | "auto: faces" puts the top below the stock top when no op ran before. The first level cuts 14 mm deep in a 2 mm-per-pass op. | 2.2; `C/session/heights.rs:319-331` | Wrong cut risk (SAFETY) |
| 2.F2 | Two picked faces cut to one floor under the default "Top Z". The operator expected each face to set its own floor. | 2.2; `C/compute/floor.rs:68-85`; hint `toolpath_panel.rs:1236-1279` | Wrong cut risk |
| 2.F3 | Face boundaries (loops and open edge chains), circular edges and vertices are not items. The operator cannot pick an edge or a corner. | `C/io/mod.rs:263-308`; `V/interaction/picking.rs:24-59` (no edge or vertex hit) | Blocks the job |
| 2.F4 | Chamfer cannot take one edge; it chamfers the whole outline of a face. | `registry.rs:671` (`Regions`) | Blocks the job |
| 2.F5 | Holes in a STEP give no circle, so Drill and Bore have nothing to cut. | `C/selection/view.rs:60-90`; `C/io/mod.rs:298-306` | Blocks the job |
| 2.F6 | A Profile of the part outline needs one face whose outline is the silhouette. A part with an overhang or several top faces has none. No silhouette item exists for 2D ops (Model Silhouette is a 3D boundary source only). | `toolpath_panel.rs:773-847` | Blocks the job |
| 2.F7 | No floor reference "a face I pick as the floor". The Floor enum has Top, Stock top, Stock bottom, Model top, Each picked face, Absolute Z (`floor.rs:68-85`). REQUIREMENTS R4 lists "picked face" as a reference. A Profile round a step top down to the lower face needs an Absolute Z typed by hand. | `floor.rs:101-106` | Confusing |
| 2.F8 | A new 2D op on a STEP starts red ("Refused") until a face pick. | D2; `selection_section.rs:208-217` | Confusing |
| 2.F9 | The 3D add group reads "3D (from STL)", but a STEP also gives a mesh. | `V/ui/toolpath_panel.rs:1175`; disabled hover "Requires 3D mesh (STL/STEP)" `:1225` | Cosmetic |
| 2.F10 | A STEP import has no dialog, so units and scale are not shown at import. | `V/controller/events/mod.rs:327,330` | Cosmetic |
| 2.F11 | The floor row offers "Each picked face" under Profile side On; the choice refuses with no message. | GUI_REVIEW_JOURNEYS B3 known gap | Confusing |

### 2.4 Proposal

- **2.F1 (first, safety)**: an auto top never sits below the stock top unless
  the op starts from the remaining stock ("After previous ops"). Options in
  question Q1. Needs core (`heights.rs` `apply_rule`, one arm). The Heights
  line "Top: auto: faces" (`V/ui/properties/operations/mod.rs:467-471`)
  stays, with the rule in its hover.
- **2.F2**: make the face floor the default when the first face pick enters a
  floor op whose floor is still the op default (question Q2). Needs core (the
  pick door sets the floor in the same command, one undo step). The O-2 hint
  then goes, because the state it hints at no longer arises by default.
- **2.F3, 2.F4, 2.F5**: the BREP source gives more item kinds, with no new
  kind (question Q3):
  - each face boundary ring → a `Loop` (outer and inner);
  - each chain of BREP edges between two faces → a `Path` (open) or a `Loop`
    (closed);
  - each circular edge → a `Circle` (this gives the holes);
  - each BREP vertex → a `Point`.

  This is the extension test of DESIGN.md §11.2 (a new item kind touches one
  place) in a cheaper form: a new source of existing kinds touches the source
  only (DESIGN.md §2.1). Drill, Bore, Trace, Chamfer and Profile then accept
  STEP edges through the existing `ItemUse` rules, with no op change. Needs
  core (`C/io/step_input.rs`, `C/io/mod.rs`) and GUI: the drawing and the pick
  read one plane today (`drawing_plane_z`, viz CLAUDE.md). An edge item has
  its own Z, so the draw and the pick must use the item Z.
- **2.F6**: a silhouette item from the model mesh (one `Loop` per outer
  boundary of the projected mesh). The 3D boundary source "Model Silhouette"
  already computes it (`maps/geom_cache.rs` `cached_silhouette`). Needs core.
  Question Q3 option C.
- **2.F7**: a floor reference `Floor::Face { key, depth }`: one picked face
  sets the floor of the whole op. Needs core (question Q4). The pick target is
  the floor row, not the Selection section; the floor row gets a Pick button
  for this reference only.
- **2.F8**: keep the refusal (D2). Change the chip from "Refused" to "Pick a
  face" (caution colour, not danger) when the cause is `Empty` on a face
  model. GUI only.
- **2.F9**: rename the group "3D (from a mesh: STL or STEP)". GUI only.
- **2.F10**: show the STEP size in the model panel ("Dimensions",
  `V/ui/properties/model_sim_panels.rs:93`); no dialog. Lead decision: no
  change; the panel already shows it.
- **2.F11**: grey "Each picked face" under side On with the refusal on hover,
  as the side combo already does in the other direction
  (`boundary_2d.rs:434`). GUI only.

---

## 3. Workflow: a 3D relief from an STL terrain

### 3.1 The journey today

1. File ▸ Import STL. No dialog. The model panel shows "Dimensions (after
   scaling)" and "Units / Scale" with "Import as:" and "Custom scale:"
   (`V/ui/properties/model_sim_panels.rs:93`, `:138-168`). A hint offers ×1000
   when the model is under 1 mm (`:112`).
2. Set the stock (stock panel, `V/ui/properties/stock.rs`).
3. Add "3D Rough" (Adaptive3d) from "3D (from STL)"
   (`C/compute/catalog/registry.rs:732`). Boundary: "Machining Boundary",
   "Enable boundary", "Source:" Stock, Model Silhouette, Selection, Rest
   Regions (`V/ui/properties/toolpath_panel.rs:753-847`).
4. Add a finish: "3D Finish" (drop cutter, `registry.rs:679`), Waterline,
   Scallop, Pencil, Steep/Shallow, Unified Finish and others.
5. Rest work has four doors, each with its own name:
   - "Start from": "Stock" or "After previous ops"
     (`V/ui/properties/tab_badges.rs:466-468`, `:576`);
   - the boundary "Rest Regions" with "Rest source:" and "(regions ready)"
     (`toolpath_panel.rs:773-847`, `:915`);
   - Pencil "Rest depth (recommended)" and "Rest Cell:"
     (`V/ui/properties/operations/surface_3d.rs:360-396`);
   - Unified Finish "Rest Claims", "Rest Reference:" Auto (derive) /
     Analytic self-probe / Machined prior stock (`surface_3d.rs:666-737`).
6. Generate All (`V/ui/toolpath_panel.rs:450-503`). When the simulation cell
   is too coarse for a rest op, a modal asks for the finer cell or Cancel
   (`V/ui/generation_resolution_modal.rs:1-40`).
7. Run Simulation (`V/ui/menu_bar.rs:225`; the sim op list "Re-run
   Simulation · stale", `V/ui/sim_op_list.rs:58-69`).
8. A flat region of the STL picks as a face in pick mode only
   (`V/app/CLAUDE.md`; `PickHit::MeshFace`).

What the operator must know:

- Which of the four rest doors applies to which op.
- A rest op reads the machined stock of the ops before it. A change upstream
  stales it (core `invalidate_output_dependents_of_set`), but a re-run of the
  simulation does not regenerate it.
- An STL flat face picks only in pick mode, and has no highlight (P4.7
  follow-up).

### 3.2 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 3.F1 | Four rest doors with four names: "After previous ops", "Rest Regions", "Rest depth", "Rest Reference". The operator must know which op reads which. | 3.1 step 5 | Confusing |
| 3.F2 | "3D Finish" names the drop-cutter op only, but every op in the group is a 3D finish. "Adaptive" (2D) and "3D Rough" (Adaptive3d) hide that they are one strategy in two forms. | `registry.rs:185`, `:679`, `:732` | Confusing |
| 3.F3 | An STL flat face has no viewport highlight after a pick. | PLAN.md P4.7 follow-up | Confusing |
| 3.F4 | A cold STL face build can stall a frame when the operator presses Generate during "Computing faces…". | PLAN.md P4.7 follow-up | Cosmetic |
| 3.F5 | The resolution question is a modal with a cell size, not a statement of what the coarse cell would miss. | `generation_resolution_modal.rs:24-40` | Cosmetic |

### 3.3 Proposal

- **3.F1**: one word, "rest", and one place to read it. The "Start from" row
  stays the one control in the main panel. The other three doors name the
  start-from row in their hover ("reads the stock that 'Start from: After
  previous ops' gives"). GUI only. A merge of the doors into one model is out
  of scope (core, large); the lead records it as an idea, not a package.
- **3.F2**: rename "3D Finish" to "Drop-cutter finish" (or "Raster finish").
  Question Q8 lists the names. GUI and core label (`registry.rs`); the wire
  name stays.
- **3.F3, 3.F4**: the P4.7 follow-ups as written. GUI only for the highlight;
  the face build moves into the worker (viz compute lane).
- **3.F5**: no change. Lead decision.

---

## 4. Workflow: holes (drill, bore, pin drilling, mixed sizes)

### 4.1 The journey today

1. Import a DXF with circles and points. Circles come from CIRCLE and full
   ARC entities; points from POINT (`C/io/dxf_input.rs:184-195`). An SVG gives
   circles from circle-like rings and no points (`C/io/svg_input.rs:506-570`).
2. Add Drill. The legend reads "Holes are DXF points and circle/arc centres,
   and circles in SVG drawings." (`V/ui/properties/operations/drill.rs:13-14`).
   Rows: "Cycle:" (G81, G82, G83, G73, `:51-64`), "Depth:" (`:158`), "Size
   Tolerance:" (`:183`).
3. The Selection section shows one row per hole size
   (`selection_section.rs:178-189`, `:357-421`): "Ø 6.000 × 4 drilled",
   "larger than the tool", "smaller than the tool", "points × 2 drilled at
   Ø 6.000". A click on a row is Show me.
4. A Drill with larger holes shows "N holes to bore" and "Add a Bore for
   these" (`selection_section.rs:470-516`). It adds one Bore after the Drill,
   with the Drill's tool and floor (P6.7b ruling).
5. Bore: "Pitch:", "Climb:", "Finish Allowance:"
   (`V/ui/properties/operations/bore.rs:31-55`); the size rows read "helix r
   …" or "leaves a core" (Danger).
6. Mixed sizes: a Drill drills only the circles of its tool size (± the size
   tolerance); a misfit default circle is skipped with a note (P6.5, O-3). To
   drill two sizes, the operator adds two Drills with two tools and a
   diameter rule each (`+ Rule` "Limit the circle diameter",
   `selection_section.rs:925-1091`; or the pick bar diameter row,
   `pick_bar.rs:286-321`).
7. Pin drilling is separate: the stock panel "Alignment pins" ▸ "Set up
   pins…" (`V/ui/properties/pins/mod.rs:167-183`) opens the pin wizard (see
   workflow 5). The Pin Drill op reads the project pins
   (`drill.rs:192-235`).

What the operator must know:

- A drill drills only its own size. A second size needs a second Drill and a
  second tool.
- Points carry no size, so a Bore never takes a point (P6.6).
- The Bore takes the Drill's tool at "Add a Bore for these". The operator
  must change it to a smaller end mill (a Ø 6 drill bit cannot helix a
  Ø 10 hole as a mill; not verified in code whether a drill-type tool refuses).
- A STEP part gives no holes (2.F5).

### 4.2 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 4.F1 | "Add toolpath ▸ Bore" refused, and the operator saw nothing ("it does nothing. no side panel opens"). The cause: the first tool was a V-bit. Correction to PLAN.md ("goes to the log only"): HEAD pushes a 6 s Warning toast for this Suggest refusal (HEAD `controller/events/toolpath.rs:146`; `V/controller.rs:572-579`; drawn bottom right, `V/app.rs:1297-1333`). `get_notifications` returns unexpired toasts only (`V/app/mcp.rs:1030-1036`), so it was empty later. The path that WAS silent at HEAD: `apply_quietly` = `self.apply(command).ok()` (`V/controller/events/mod.rs:95-97`), used by add, duplicate and `AddBoreFromDrill`. A command-level refusal there reached no surface. The working tree replaces it with `apply_add_or_toast`. | PLAN.md "Add Bore does nothing"; working-tree fix | Blocks the job |
| 4.F2 | Mixed sizes need one Drill per size, each with a diameter rule typed by hand. | 4.1 step 6 | Confusing |
| 4.F3 | "Add a Bore for these" copies the Drill's tool into the Bore. A drill bit as a helix tool is a likely mistake. | P6.7b ruling ("the Bore takes the Drill's tool") | Wrong cut risk (not verified in code) |
| 4.F4 | Points and circles mix in one Drill with no visible size for a point ("drilled at Ø 6.000" is the tool size). | `selection_section.rs:380-421` | Cosmetic |
| 4.F5 | Pin Drill has no size tolerance and lives behind the pin wizard, not the Add menu. Two doors for "drill a hole". | `drill.rs:192-235`; `registry.rs:1307` (SystemOnly) | Cosmetic |

### 4.3 Proposal

- **4.F1**: finish the working-tree fix (first tool that fits; a toast that
  names the needed tool kinds). Also: a refused act gets a toast that stays
  until a click, not 6 s (question Q5). GUI only.
- **4.F2**: a "Holes" job in the P7 wizard: one Drill per size that a project
  tool fits, and one Bore for the rest, all from one draft. This is "auto
  magic", so it is a wizard (ruling), entered from the setup `+` menu. Needs
  core (a P7 job type) and GUI (the P7 wizard). In the main panel: no change.
- **4.F3**: "Add a Bore for these" takes the first project tool that the Bore
  accepts (the same rule as 1.F1), not the Drill's tool. Needs core
  (`AddBoreFromDrill`, `C/session/mutation/toolpath.rs`). If no tool fits,
  the line names the tool that the Bore needs.
- **4.F4, 4.F5**: no change. Lead decision.

---

## 5. Workflow: a two-sided job (flip, alignment pins, a second setup)

### 5.1 The journey today

1. The Setup rail `+` ("Add a setup", `V/ui/setup_panel.rs:89`;
   `V/controller/events/model.rs:285`) adds "Setup N" with face Top.
2. Setup properties: "Face Up:" and "Z Rotation"
   (`V/ui/properties/setup.rs:58-82`). For a non-Top face, a flip line shows
   (`:95`). With no pins, the button "Add alignment pins for this flip"
   (`:101`) runs `SetupTwoSided` (`model.rs:393`): it adds a Bottom setup if
   none exists and places pins.
3. "Datum / Alignment": "XY Method:" and "Z Method:" (`setup.rs:111-203`).
4. Or the stock panel "Alignment pins" ▸ "Set up pins…" opens the pin wizard
   (`V/ui/pin_wizard.rs:220`): Dowels, then Place, then Apply; "Add a Bottom
   setup" (`:349`).
5. The second setup starts from the carried stock of the first
   (`C/compute/stock_carry.rs:1-30`). Its ops depend on the previous setup
   (`C/session/dependencies.rs:30-48`).
6. Add ops in the second setup through its own `+`. A new op takes the first
   model of the project (add flow), and its items draw in the toolpath's setup
   frame. Trace Left/Right and Pocket extend sides mirror with the setup
   (`C/selection/CLAUDE.md`, `mirrored`).
7. "Models" checklist per setup (`setup.rs:245-290`).

What the operator must know:

- Two doors add pins: the setup button and the stock panel wizard.
- A floor "Absolute Z" is a Z in the setup frame. Its help says
  "frame-dependent" (`C/compute/floor.rs:639`). A toolpath moved to the other
  setup keeps the number, not the meaning.
- The datum of a flipped setup zeros at the presented top (StockTop).

### 5.2 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 5.F1 | An "Absolute Z" floor or a pinned Top on an op moved to a flipped setup keeps its number in the new frame. | `floor.rs:639`; MCP `move_toolpath_to_setup` | Wrong cut risk |
| 5.F2 | A new op in setup 2 takes the first model of the project, not the model of that setup. | add flow, `V/controller/events/toolpath.rs` | Confusing |
| 5.F3 | Two doors for pins: "Add alignment pins for this flip" (setup panel) and "Set up pins…" (stock panel, the wizard). | `setup.rs:101`; `pins/mod.rs:167-183` | Confusing |
| 5.F4 | Each new setup is "Setup N", face Top. The operator must open it and set "Face Up:". | `model.rs:285` | Cosmetic |

### 5.3 Proposal

- **5.F1**: a move of an op to a setup with another frame refuses while the
  op holds an "Absolute Z" floor or a pinned height, and names the field. Or
  it re-expresses the value (the L-10 `re_express` rule of the floor row).
  Question Q6. Needs core.
- **5.F2**: the add takes the model that the setup's "Models" list allows,
  then the first model that serves the op (a drawing for 2D, a mesh for 3D).
  Question Q7. GUI only (the add flow) if the rule reads the setup model list.
- **5.F3**: the setup button opens the pin wizard at the Place step with a
  Bottom setup in the draft. One door, one draft. GUI only.
- **5.F4**: no change. Lead decision.

---

## 6. Workflow: edit and re-run

### 6.1 The journey today

| Edit | Where | What goes stale (core) |
|---|---|---|
| Tool of an op | Geometry tab "Tool:" (`tab_badges.rs:497-510`) | the op and every op that reads its result (`invalidate_output_dependents_of_set`, `C/session/CLAUDE.md`) |
| A tool's own fields | the tool panel (`V/ui/properties/tool.rs`) | every op on that tool and their dependents |
| Stock | the stock panel | every op of the setup and the later setups (stock carry) |
| Selection of an op | the Selection section | that op only (DESIGN.md §9.1), and its dependents |
| Re-import ("Reload from disk", `V/ui/setup_panel.rs:174`) | the Setup rail | every reader of the model (`drop_results_for_model`); rules resolve again; a lost pick shows "Lost pick: <kind>" with Show me and Remove (`selection_section.rs:812-856`) |
| Simulation re-run | menu, sim op list | nothing; toolpaths do not regenerate |

How the operator learns it:

- The toolpath row dot changes colour (`V/ui/toolpath_panel.rs:720-770`). The
  word ("STALE", "ERR", "OK") is on hover only (`:1041-1080`). The STALE
  hover reads "Inputs changed after this was generated. … Regenerate."
- The Readiness panel: "{computed}/{enabled} current — {N} edited since
  generation" (`V/ui/readiness_panel.rs:153-157`).
- The sim op list: "Re-run Simulation · stale" (`V/ui/sim_op_list.rs:58-69`).
- "Generate All" skips each op that holds a current result
  (`V/controller/events/compute.rs:1836-1853`).
- Export refuses stale enabled geometry unless the operator accepts the
  override (`V/app/CLAUDE.md`).
- A generation failure sets ERR on the dot with the message on hover. A plain
  `ComputeError::Message` gives no toast (`compute.rs:597-611`).

### 6.2 Friction

| # | Friction | Evidence | Rank |
|---|---|---|---|
| 6.F1 | A failed generation shows only a dot colour; the reason is on hover. No toast. | `compute.rs:597-611`; `toolpath_panel.rs:720-770` | Confusing |
| 6.F2 | The STALE hover says "Inputs changed", not which input (tool, stock, selection, an op above). | `toolpath_panel.rs:1041-1080` | Confusing |
| 6.F3 | A selection refusal shows in two places with two texts: the section (full) and the footer ("…the selection refuses. See Selection on the Geometry tab."). | `tab_badges.rs:271-272`, `:319-323` | Cosmetic |
| 6.F4 | The Tool combo lists every tool. A pick of a tool that the op refuses is found only at generation (not verified in code). | `tab_badges.rs:497-510` | Confusing |

### 6.3 Proposal

- **6.F1**: the row shows the state word beside the dot when the state is ERR
  or STALE (no word for OK). GUI only. A toast for a failure that the
  operator started (a click on Generate), not for a background regenerate
  (question Q5).
- **6.F2**: the stale hover names the cause. Core keeps the cause of the last
  stale per op (the edit that set `stale_since`); the GUI reads it. Needs core
  (a small field beside `Effects.stale`).
- **6.F3**: no change; IC1 (one owner) holds.
- **6.F4**: as 1.F1: grey the tools that the op does not accept. GUI only.

---

## 7. Cross-cutting

### 7.1 The selection model: rules, picks, kinds, the pick bar

What the code does:

- A selection holds rules, then picks (`C/selection/rule.rs`). The default
  rule is "This model · every layer · every item it can cut"
  (`selection_section.rs:577-622`).
- A rule under the default replaces the default (`rule.rs:339-371`). A pick
  under the default adds to it, except on the Face op (stock default, L-6).
- The pick bar chips are Loops, Open, Circles, Faces, Points
  (`V/ui/pick_bar.rs:73-81`). A chip for a kind the op cannot cut is greyed,
  with the core reason (`pick_bar.rs:55-61`; `C/selection/refusal.rs:58-85`).
- A click toggles one item (`TogglePick` → core `click_edit`); a shift-click
  adds a layer rule (`PickLayer` → `layer_rule_edit`,
  `V/controller/events/mod.rs:488-547`).
- A face is picked by the ray; a drawing item by its ring in screen space
  (`V/interaction/picking.rs:309-380`).

Findings:

1. O-PICK is ruled and not built (1.F2). It also removes the rule split
   (1.F3): under the default, every first act (rule, layer, pick) replaces.
2. The kinds are right; the sources are short. A STEP gives faces only
   (2.F3). The fix is in the source, not in a new chip (2.4).
3. The operator reads "Faces" as "a face", and expects "Loops" to include the
   face edges. After 2.F3, a face edge is a Loop or Path item, so the chips
   mean what the operator expects.
4. The chip state is per pick session (`V/state/pick.rs`). The operator must
   turn chips on before a click. That is the parked "hard to understand"
   point (1.F6).

Proposal (lead decision, GUI only unless named):

- Build O-PICK (core).
- Rename the chips by the operator's words: "Closed shapes", "Open lines",
  "Circles", "Faces", "Points" (and, after 2.F3, "Edges" as a filter that
  admits the Path and Loop items of a BREP edge chain). Question Q8.
- In pick mode, a click on an item of a kind whose chip is off, but that the
  op can cut, turns the chip on and picks the item. The operator then does not
  need to set chips before a click. GUI only.

### 7.2 The floor and heights model

What the code does:

- Top: auto = the stock top, or "auto: faces" = the highest picked up-face
  (`C/session/heights.rs:70-90`, `:301-333`); pinned = a typed value.
- Floor (Pocket, Profile, Zigzag, Adaptive 2D, Bore): one value with a
  reference (`C/compute/floor.rs:68-85`): "Top Z" (default), "Stock top",
  "Stock bottom", "Model top", "Each picked face", "Absolute Z"
  (`floor.rs:101-106`).
- The floor row is on the Geometry tab ("Floor from:",
  `boundary_2d.rs:53-150`). The Heights tab shows a read-only "Floor: … · set
  in <op>" line (`V/ui/properties/operations/mod.rs:481-498`).

Why two faces cut to one height (2.F2): "Top Z" means "below the op top".
With a face pick, the op top is the highest face. So every face gets the same
floor. "Each picked face" gives each face its own plane, but only by the hint
button.

Why the first level buried the tool (2.F1): "auto: faces" moves the top down
to the face with no check against the stock above it.

Findings:

1. The default floor reference ("Top Z") and the auto top together make the
   face pick move the floor too. The operator reads a face pick as "the
   floor", not "the top". REQUIREMENTS S6 says "each picked face sets its own
   floor".
2. "Top Z" is a floor reference name and also the Top row name. One word, two
   meanings.
3. "Each picked face" means two things by op: for a Pocket, "clear down to
   this face"; for a Profile, "cut round this face down to its own plane".
   A Profile round the top of a step down to the lower face needs a third
   meaning: "down to that other face" (2.F7).

Proposal:

- Q1 (safety) first.
- Q2: the face floor becomes the default on the first face pick.
- Q4: a floor reference "A picked face" (one face sets the floor).
- Rename the floor reference "Top Z" to "Op top" (question Q8).

### 7.3 Names and words

| Thing | Names today | Where | Proposal |
|---|---|---|---|
| The items an op cuts | "Selection" (section), "Selection" (boundary source), "▸ Selection" (pick bar) | `selection_section.rs:158`; `toolpath_panel.rs:773-847`; `pick_bar.rs:119-126` | Section "Items to cut" (its first row stays "Cuts", `selection_section.rs:315-339`); boundary source "Picked items"; pick bar "Pick for <op> ▸ Items to cut \| Boundary" |
| The floor reference below the op top | "Top Z" | `floor.rs:101` | "Op top" |
| The source of the top | "auto: stock top", "auto: faces", "pinned" | `heights.rs:80-89` | "auto: stock top", "auto: highest picked face", "typed" |
| The pick buttons | "Pick", "Pick faces" | `selection_section.rs:256-274`; `toolpath_panel.rs:505-551` | "Pick" for both |
| Profile and Trace side | Profile "Side" + "Compensation"; Trace "Compensation" with "Left/Right of the drawn direction" | `boundary_2d.rs:437-549`; `engrave.rs:14-32` | Trace "Side"; "Compensation" only for In Computer / In Control |
| The 2D and 3D adaptive | "Adaptive", "3D Rough" | `registry.rs:185`, `:732` | "Adaptive 2D", "Adaptive 3D (rough)" |
| The drop-cutter finish | "3D Finish" | `registry.rs:679` | "Raster finish" |
| The 3D add group | "3D (from STL)" | `toolpath_panel.rs:1175` | "3D (from a mesh: STL or STEP)" |
| Rest | "After previous ops", "Rest Regions", "Rest depth", "Rest Reference" | 3.1 step 5 | keep the controls; each hover names "Start from" (3.3) |
| A hole ring in a Profile region | "Island" ("the op cuts round it") | `C/selection/view.rs:189` | after the held test: "Wall round it" for Profile |
| The pick-bar kinds | "Loops", "Open", "Circles", "Faces", "Points" | `pick_bar.rs:73-81` | "Closed shapes", "Open lines", "Circles", "Faces", "Points" |

All rows are GUI only, except the registry labels (a core label; the wire
name stays) and the "Island" role text (core).

### 7.4 Refusals and notes: where the operator sees them

| Kind | Where it shows today | Gap |
|---|---|---|
| A refused add (Suggest refusal) | a Warning toast, 6 s, bottom right (`V/controller.rs:572-579`, `V/app.rs:1297-1333`) | far from the menu; gone before the operator looks (4.F1) |
| A refused add, duplicate or Add Bore (command refusal) | nothing at HEAD (`apply_quietly`, `V/controller/events/mod.rs:95-97`); a toast in the working tree | silent at HEAD (4.F1) |
| A refused generation | the dot colour; the text on hover (`toolpath_panel.rs:720-770`) | no toast, no word (6.F1) |
| A selection refusal | the Selection section (danger text, `selection_section.rs:857-859`); the footer line | good |
| A selection note (skipped open paths, a hole misfit, a face at the top) | a caution row with Show me (`selection_section.rs:786-811`) | good; the HoleFit notes repeat the size rows (1.F10) |
| Uncut material at a corner | nowhere | no finding (1.F4) |
| A buried first level | nowhere at generation; the simulation shows load later | no check (2.F1) |

Rule to adopt: Q5 (B).

### 7.5 The add-op flow and the tool choice

What the code does (working tree):

- The menu greys an op when the models lack its geometry
  (`V/ui/toolpath_panel.rs:1187-1229`). It does not check tools.
- The add takes the first model of the project and the first tool that the
  Suggest door accepts; it refuses when none fits, with a toast that names the
  first tool's reason.
- The new op is selected and its Geometry tab opens only when the add
  succeeds.

Proposal:

- The menu greys an op that no project tool can run, with the reason on hover
  ("Bore needs a flat end mill or bull nose; the project has none"). GUI only
  (it calls the same acceptance door as the add).
- The add takes the model by the rule in Q7.
- Tool advice ("the largest tool that fits the tightest corner") stays in P8,
  behind the wizard ruling.

---

## 8. Top ten frictions, by rank

Order rule: a wrong cut tagged SAFETY comes before a blocked job, because a
blocked job emits no cut. Then "Blocks the job", then "Wrong cut risk".

| Order | # | Friction | Rank | Fix needs |
|---|---|---|---|---|
| 1 | 2.F1 | "auto: faces" top below the stock top buries the first level (14 mm against 2 mm per pass) | Wrong cut risk (SAFETY) | core |
| 2 | 2.F3 | STEP edges, circular edges and vertices are not items | Blocks the job | core + GUI |
| 3 | 2.F5 | STEP holes give no circle; Drill and Bore have nothing to cut | Blocks the job | core (with 2.F3) |
| 4 | 4.F1 | A refused Add Bore looked silent; the first tool decided | Blocks the job | GUI (in progress) |
| 5 | 2.F6 | No silhouette item for a Profile of a STEP part outline | Blocks the job | core |
| 6 | 2.F2 | Two picked faces cut to one floor under "Top Z" | Wrong cut risk | core |
| 7 | 1.F2 | A pick adds to the default rule (O-PICK ruled, not built) | Wrong cut risk | core |
| 8 | 1.F4 | A missed corner leaves material with no note | Wrong cut risk | core |
| 9 | 5.F1 | An Absolute Z floor keeps its number in a flipped setup | Wrong cut risk | core |
| 10 | 1.F1 | A new op takes a tool it can run but the operator did not choose (V-bit for a Profile) | Wrong cut risk | GUI (in progress) |

Next in line: 6.F1 (a failed generation shows only a dot), 2.F7 (no "a picked
face" floor), 3.F1 (four rest doors), 1.F6 (the `+ Rule` card wording).

---

## 9. Open questions for the operator

Each question changes what is machined, or overturns a lead ruling. The lead
decides every other point in this file.

1. **Q1 — The auto top under a face pick (SAFETY, 2.F1).**
   - (A) The auto top is the higher of the face top and the stock top. An op
     that starts from the remaining stock ("After previous ops") skips the air
     levels through its remaining-stock clip.
   - (B) The auto top is the top of the material that stands when the op
     starts (the stock chain). Exact, but needs the prior result at heights
     time.
   - (C) Refuse generation when the auto face top sits below the stock top and
     the op starts from the stock.
   - **Recommendation: (A) now**, (B) later if (A) cuts too much air. (A) is
     the L-15 rule of "Each picked face", made general.

2. **Q2 — The default floor when the selection holds a face (2.F2).**
   - (A) Today: the floor stays "Top Z"; a hint offers "Each picked face"
     (lead ruling O-2, 2026-10-20).
   - (B) The first face pick on a floor op whose floor is still the op default
     sets "Each picked face", depth 0, in the same undo step, with a one-line
     note.
   - (C) "Top Z" refuses while the selection holds a face.
   - **Recommendation: (B).** It matches S6 and the operator's reading. It
     overturns the lead ruling O-2.

3. **Q3 — Which STEP features become items (2.F3–2.F6).**
   - (A) Face boundary rings only (outer and inner loops).
   - (B) (A) plus edge chains as paths and loops, circular edges as circles,
     vertices as points.
   - (C) (B) plus one silhouette loop per model.
   - **Recommendation: (B) first, then (C)** as its own package. (B) gives
     holes, edge chamfers and step profiles; (C) gives the part outline.

4. **Q4 — A floor reference "A picked face" (2.F7).**
   - (A) No; type an Absolute Z.
   - (B) A new reference: one picked face sets the floor of the op (the face
     key is stored, so a re-import follows it).
   - **Recommendation: (B).** REQUIREMENTS R4 names "picked face" as a
     reference; it is the only safe way to profile a step down to the face
     below it.

5. **Q5 — How a refusal reaches the operator (4.F1, 6.F1).**
   - (A) Today: a 6 s toast for an act; a dot colour for a state.
   - (B) An act refused by core gives a toast that stays until a click; a
     failed or stale state shows its word on the row; no toast for a state.
   - (C) (B), plus a toast for every failed generation.
   - **Recommendation: (B).**

6. **Q6 — An op moved to a setup with another frame (5.F1).**
   - (A) Refuse the move while the op holds an Absolute Z floor or a pinned
     height; name the field.
   - (B) Re-express the value so that the cut stays at the same physical
     place (the L-10 rule).
   - **Recommendation: (A).** (B) guesses the intent.

7. **Q7 — The model of a new op (5.F2).**
   - (A) Today: the first model of the project.
   - (B) The model selected in the Setup rail, else the first model that the
     setup allows and that serves the op.
   - **Recommendation: (B).**

8. **Q8 — Names (sections 7.1–7.3).** The operator approves or changes the
   table in 7.3 and the chip names in 7.1. **Recommendation:** take the table
   as written. Wire names and file keys do not change.

9. **Q9 — The Bore tool from "Add a Bore for these" (4.F3).**
   - (A) Today: the Drill's tool (P6.7b lead ruling).
   - (B) The first project tool that the Bore accepts; refuse with the needed
     kind when none fits.
   - **Recommendation: (B).** It overturns the P6.7b lead ruling.

10. **Q10 — A "Holes" and a "STEP part" job type in the P7 wizard.**
    - (A) P7 as designed: Sign, Sheet parts, Inlay pocket, Inlay plug, Flatten.
    - (B) (A) plus "Holes" (one Drill per size that a tool fits, Bore for the
      rest).
    - (C) (B) plus "STEP part" (pocket each up face, profile the silhouette,
      drill the circles) after Q3.
    - **Recommendation: (B)** in P7; (C) after Q3 lands.

---

## 10. Proposed order of work packages

Each package follows the PLAN.md per-item flow: an editor, a sentry with a
red proof, the lead review, then the operator look.

| WP | Content | Fixes | Needs | Waits for |
|---|---|---|---|---|
| UX-1 | Auto top never below the stock top (Q1) | 2.F1 | core | Q1 |
| UX-2 | Finish the add-op fix: first tool that fits; the success toast names the tool; menu greys an op with no fitting tool; Tool combo greys refused tools | 1.F1, 4.F1, 6.F4 | GUI | nothing (in progress) |
| UX-3 | O-PICK in core (`pick_edit` replaces the op default) | 1.F2, 1.F3 | core | nothing (ruled) |
| UX-4 | Corner finding with Show me | 1.F4 | core + GUI note row | nothing (ruled) |
| UX-5 | Refusal visibility: a sticky toast for a refused act; the state word on the row | 4.F1, 6.F1 | GUI | Q5 |
| UX-6 | Face floor default on the first face pick | 2.F2 | core | Q2 |
| UX-7 | STEP items: rings, edge chains, circular edges, vertices; draw and pick at the item Z | 2.F3, 2.F4, 2.F5 | core + GUI | Q3 |
| UX-8 | Floor reference "A picked face" | 2.F7 | core + GUI | Q4, UX-7 |
| UX-9 | Silhouette item | 2.F6 | core | Q3 (C) |
| UX-10 | Names pass (7.3), chip names, the auto chip on click, the `+ Rule` card wording | 1.F6, 1.F9, 2.F9, 3.F2, 7.1 | GUI (+ registry labels) | Q8 |
| UX-11 | Small GUI gaps: Each picked face greyed under On; Trace side mark always on; HoleFit note merge; STL face highlight; "Pick a face" chip | 1.F8, 1.F10, 2.F8, 2.F11, 3.F3 | GUI | nothing |
| UX-12 | Setup moves: refuse a frame-dependent value; add takes the setup model; one pin door | 5.F1, 5.F2, 5.F3 | core + GUI | Q6, Q7 |
| UX-13 | Stale cause in the hover | 6.F2 | core + GUI | nothing |
| UX-14 | Bore tool from "Add a Bore for these" | 4.F3 | core | Q9 |
| P7 | Templates and job types, with the Q10 additions | 4.F2, 1.F5 | core + GUI | Q10 |

Order: UX-1 first (a measured 14 mm bury). Then the ruled or in-progress
packages UX-2, UX-3, UX-4. Then UX-5 and UX-6 after Q5 and Q2. The STEP
package UX-7 to UX-9 gets a short design note first, because the
draw-and-pick plane changes. UX-10 and UX-11 go in one GUI pass with one
operator look. P7 comes after UX-7; P8 waits for the operator review.

The release GUI is rebuilt from HEAD before each operator look (root
CLAUDE.md).
