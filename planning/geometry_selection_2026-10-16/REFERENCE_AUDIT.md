# Reference audit: geometry selection against Kiri:Moto, FreeCAD and Fusion (2026-10-23)

Goal (operator): "make sure that this program feels intuitive and like other
tools, acts how people expect". This file compares each decision that a user
sees or feels with the three reference tools. It is research only. Nobody
drove the GUI or MCP for it, and nobody ran cargo.

## 1. Method

1. The auditor read `PLAN.md`, `UX_REVIEW.md`, `FR1_FLOW_REVIEW.md`,
   `FR2_FLOW_REVIEW.md`, `P3/P4/P6/P7_DESIGN.md`, `GAP_ANALYSIS.md`,
   `INDUSTRY_DEFAULTS.md`, the folder `CLAUDE.md` files of `selection/` and
   `ui/properties/`, and the op defaults in code.
2. The auditor read reference source at the newest commit on 2026-10-23
   and the Fusion manuals.
3. Each row gets one verdict:
   - **MATCH**: rs_cam acts as the references do.
   - **DIFFERS-KEEP**: rs_cam differs for a stated reason that still holds.
   - **DIFFERS-CHANGE**: rs_cam differs with no good reason. A proposed change follows.
   - **UNCLEAR**: no source. The row says what settles it.
   - **SETTLED Qn**: `INDUSTRY_DEFAULTS.md` question n settles the row. This file does not redo it.
4. Build state at HEAD c924192b plus the working tree: **[built]**,
   **[wt]** (working tree, not committed), **[ruled]** (ruled, not built).
   A MATCH on a [ruled] row means that the ruling matches. The GUI does not match yet.
5. The three references are model-first: Fusion and FreeCAD work on solids,
   and Kiri works on meshes. None is drawing-first like the rs_cam DXF sign
   path. Where the real peer of a drawing flow is Vectric or Carbide Create,
   the row says so.

Source keys (each cell names a file or page; the tag gives the grade):

| Key | Root |
|---|---|
| Kd | Kiri `src/kiri/app/conf/defaults.js` (grid-apps master) |
| Kl | Kiri `web/kiri/lang/en.js` (UI text) |
| Kw | Kiri `src/kiri/mode/cam/work/` (`op-area.js`, `op-trace.js`, `op-register.js`) |
| F | FreeCAD `src/Mod/CAM/Path/` (main) |
| Fu | `help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/<page>.htm` |
| ID | `INDUSTRY_DEFAULTS.md` in this folder |
| rs | rs_cam paths: `C/` = `crates/rs_cam_core/src/`, `V/` = `crates/rs_cam_viz/src/` |

Grades: [code] source read; [docs] manual; [forum] forum post; [UI] common
knowledge with no written source (weakest).

## 2. Comparison table

### 2.1 Selection defaults

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| S1 | A new 2D op on a drawing takes every item that it can cut, on every layer (PLAN D2/D13; `C/compute/operation_configs.rs` `Selection::drawing()`) [built] | Feature ops are empty until "select" (Kl `cc_sele_l`; Kw `op-area.js:75` reads `op.areas` only) [code] | Pocket with no Base cuts nothing (F `Op/PocketShape.py:125`, no else arm on main; release 1.0 pocketed the whole model, see the ID header caveat); Engrave with no Base takes every flat model shape (F `Op/Engrave.py:275-283`) [code] | No default; the user selects contours (Fu `2D-CONTOUR-STEPS`) [docs] | DIFFERS-KEEP. Reasons: O-PICK makes the first pick act as the references do; the Cuts count and the highlight show the scope before Generate. |
| S2 | A new 2D op on a STEP model takes the face rings, each cut to its own face (ruling Q3, UX-7b) [built] | Pocket needs a picked surface (Kl `cc_sele_l`) [code] | Pocket with no Base cuts nothing (as S1) [code] | No default [docs] | DIFFERS-KEEP. Operator ruling Q3. The each-face floor and the gouge guard keep it safe. |
| S3 | A STEP Chamfer takes the convex top edges and rims by default (R-C) [built] | No chamfer op (ID Q1) | Deburr with no Base returns (F `Op/Deburr.py:213`) [code] | 2D Chamfer needs picked edges (ID Q1) [docs] | DIFFERS-KEEP. Operator ruling R-C. Convex only is the safe filter. |
| S4 | A DXF Drill takes every point and every circle of the tool size; it skips a misfit with a note (P6.5, O-3) [built] | One click: "Select Matching Diameter", area within 5 % (Kl `cd_sela_l`) [code] | "Auto-select": every hole with radius >= tool radius (ID Q9) [code] | Pick one hole, "Select Same Diameter" (Fu `LP-STEP-IRONSTRATEGY_DRILL`) [docs] | MATCH (the Kiri and Fusion size rule, with no click). |
| S5 | A STEP Drill takes the rim circles that fit the tool (R-B) [wt] | — | — | — | SETTLED Q2, Q9. |
| S6 | A 3D op takes the whole model; the boundary is the stock | Global ops use every part (ID Q6) [code] | Every Job model (ID Q6) [code] | The setup model (ID Q6) [docs] | MATCH. |
| S7 | Profile on a mesh model: the part outline, Outside, to the model bottom (R-A) [ruled] | — | — | — | SETTLED Q3. |
| S8 | A new op takes the first model of the project [built]; ruled: the setup models | — | — | — | SETTLED Q6. |
| S9 | A new op takes the first project tool that it accepts (f9a3d2b1; `V/controller/events/toolpath.rs:92-150`) [built] | Each op has its own tool default (Kd `camPocketTool`, `camOutlineTool`) [code] | A new op takes the tool controller of the previous op (ID Q8; F `Op/Base.py` `setDefaultValues`) [code] | The user picks the tool first, on the Tool tab [UI] | DIFFERS-CHANGE (1 of 3: FreeCAD only): take the tool of the last op in the setup when the op accepts it, else the first that fits. Motion (tool choice), small. |
| S10 | At the add, Suggest writes the stepover and the depth per pass for the bound tool (`C/feeds/suggest.rs:900-930`) [built] | Stepover is a fraction of the tool diameter: Pocket 0.25, Outline 0.4 (Kd 563, 575) [code] | Pocket StepOver 50 % (F `Op/PocketShape.py:103`) [code] | Stepover and stepdown come from the tool preset [UI] | MATCH. With no feeds recipe (R1 "Unbacked"), the raw default is 2 mm absolute. |
| S11 | Floors at the add (`C/feeds/suggest/apply.rs:1137-1149`): Profile = top less the stock thickness; Drill = the stock thickness; Pocket = min(half the stock, 5 mm) [built] | Outline to the part bottom (ID Q3); Pocket to the selected surface [code] | Profile FinalDepth = stock bottom (ID Q3) [code] | Bottom height = the selected contour (ID Q3) [docs] | MATCH (Profile: through, no spoilboard); UNCLEAR (Pocket, see 4). |
| S12 | Auto stock = the union of every model plus 5 mm at the sides and the top; the bottom is flush (F31; `C/compute/stock_config.rs:109, 282-296`) [built] | 5 mm on X, Y and Z (Kd `camStockX/Y/Z`) [code] | 1 mm on every side (F `Main/Stock.py:177`) [code] | Relative size box with an offset (Fu `MFG-REF-SETUP-STOCK`) [docs; value forum] | MATCH. |
| S13 | Profile default: side Outside, climb (`operation_configs.rs:892-901`) | `camOutlineOut` true, `camOutlineIn` false (Kd 559-560) [code] | Side "Outside", Direction "CW" = climb outside (F `Op/Profile.py:324-328`) [code] | Side by the arrow; climb [UI] | MATCH. |
| S14 | Chamfer default: side Outside, width 1 mm, tip offset 0.1 mm (`operation_configs.rs:674-687`) | No chamfer op (ID Q1) | Deburr: Side "Outside", Width 1 mm, ExtraDepth 0.5 mm (F `Op/Deburr.py:437-444`) [code] | Width and tip offset fields; values not found [docs gap] | MATCH (side, width). The tip goes 0.1 mm below the edge, FreeCAD 0.5 mm. |

### 2.2 Picking

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| P1 | The first pick replaces the default rule (O-PICK, da0dd189) [built] | The selection is what the user picks [code] | Base holds what the user adds (F `Op/Gui/Base.py:910`) [code] | The selection list holds the picks [docs] | MATCH. |
| P2 | A click adds or removes one item (`V/ui/pick_bar.rs` help) [built] | Click toggles a line, loop or surface [UI] | 3D view selection, then "Add" or "Remove" (F `Op/Gui/Base.py:910-990`) [code] | Click adds, click again removes [UI] | MATCH. |
| P3 | Shift-click adds the whole layer of the item as a rule (`V/ui/pick_bar.rs` `PICK_HELP`) [built] | n/a (no layers) | n/a; Ctrl-click adds to the selection [UI] | n/a; Ctrl or Shift adds to the selection [UI] | DIFFERS-CHANGE: Shift-click adds one item, as in CAD. "All on this layer" moves to a row act or a right-click. UI, small. |
| P4 | A click picks only kinds whose chip is on; the user sets the chips first (UX_REVIEW 7.1 finding 4) [built] | The op decides what a click selects: lines, loops or surfaces (Kl `cc_sele_l`) [code] | Any sub-element; the op checks support (F `Op/Gui/Base.py:897`) [code] | The click target decides: an edge gives a chain, a face gives its loops (Fu `GUID75B6821B...` "Selecting a Face creates toolpaths on all the edges") [docs] | DIFFERS-CHANGE: a click on an item that the op can cut picks it, whatever the chip state; the chips stay as a filter. UI, small. |
| P5 | A face click on a 2D Profile cuts its outer ring and the walls round its hole rings ("Island") | n/a | Profile `processHoles` False: outer ring only (F `Op/Profile.py:330`) [code] | "Selecting a Face creates toolpaths on all the edges" (Fu `GUID75B6821B...`) [docs] | MATCH (Fusion). FreeCAD differs. |
| P6 | A face click on a 3D op sets its boundary to Selection (L-8 revised) [built] | n/a (global ops) | Surface op Base faces [UI] | Machining boundary "Selection" (Fu `MFG-GEOMETRY-SELECTIONS`) [docs] | MATCH. |
| P7 | The Geometry tab lists the selected items only, grouped by rule then pick (GT-Q6) [built] | n/a | Base Geometry list of sub-elements (F `Op/Gui/Base.py:797`) [code] | The selection shows a count and a list [UI] | MATCH. |
| P8 | To drop one item from a rule, the rule turns into picks (GT-Q8) [built] | n/a | n/a | "Select Same Diameter" cannot exclude one hole (forum idea, see 5) [forum] | DIFFERS-KEEP. rs_cam does what Fusion users ask for. |
| P9 | "Use as floor" on a face row; floor "A picked face" (UX-8) [built] | n/a | Depths page: depth from the selection (F `Op/Gui/Base.py:1383-1406`) [code] | Bottom height From "Selection" (Fu `MFG-REF-BOTTOM-HEIGHT`) [docs] | MATCH. |
| P10 | A row act "Rule…" makes "all circles Ø6" from a picked item (GT concept C) [built] | "Select Matching Diameter" (Kl `cd_sela_l`) [code] | n/a | "Select Same Diameter" (Fu drill steps) [docs] | MATCH. |
| P11 | STL flat regions pick as faces, in pick mode only; no highlight (P4.7) [built] | Pocket picks a mesh surface with a curvature limit (Kl `cp_foll_l`) [code] | Mesh is not native [UI] | Mesh is not native [UI] | MATCH (Kiri). The highlight gap is the P4.7 follow-up. |

### 2.3 Depths and floors

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| D1 | Floor references: Top Z, Stock top, Stock bottom, Model top, Each picked face, Absolute Z, A picked face, Each hole floor (`C/compute/floor.rs:70-104`) | z bottom: an offset from the stock bottom (Kl `ou_zbot_l`) [code] | FinalDepth: an absolute Z (ID Q2) [code] | From: Top height, Model top, Model bottom, Stock top, Stock bottom, Selection, Origin, Hole top/bottom (Fu `MFG-REF-BOTTOM-HEIGHT`) [docs] | DIFFERS-CHANGE: add "Model bottom" (Fusion has it; R-A needs it). UI, small; motion only when chosen. |
| D2 | The first face pick on a floor op sets "Each picked face", depth 0 (Q2, UX-O) [built] | Pocket clears to the selected surface [code] | Depth from the selection, by a button [code] | Bottom height default "Selected contour(s)"; one cut per Z level (ID Q3; forum "One level pocket") [docs + forum] | MATCH. |
| D3 | Under a face pick the auto top is the higher of the face top and the stock top (UX-1) [built] | z top: offset from the stock bottom; 0 = no override (Kl `ou_ztop_l`) [code, inferred] | StartDepth = stock top (ID Q2) [code] | Default not found [docs gap] | MATCH (Kiri, FreeCAD). |
| D4 | A one-depth op (Chamfer, V-Carve, Inlay) on a face below standing material refuses (UX-1c) [built] | Clips to the part shadow at each Z (Kw `op-area.js:181-196`) [code] | No check [code, inferred] | No check [UI] | DIFFERS-KEEP. Safety: the references bury the tool or cut air in silence. |
| D5 | A depth is "mm below the reference"; negative is above (`floor.rs:61`) | Offsets in workspace units [code] | Expressions on Op values (ID Q5) [code] | Signed offset from the "From" reference [docs] | MATCH. |
| D6 | The floor is edited on the Cut tab ("Floor from:"); the Heights tab shows a read-only line (P3 L-11) [built] | z top and z bottom sit together on the op [code] | Depths page holds Start and Final depth [code] | Top and Bottom height sit on the Heights tab [docs] | DIFFERS-CHANGE: edit the floor as "Bottom height" on the Heights tab, under "Top height" (extends F19). UI, medium. |
| D7 | Tab order: Geometry, Cut, Feeds & Speeds, Linking, Heights, Dressup (GT ruling) [built] | n/a (one panel) | Base Geometry, Location, Heights, Tool controller (F `Op/Gui/Base.py:1531-1557`) [code] | Tool, Geometry, Heights, Passes, Linking [UI] | DIFFERS-CHANGE: move Heights to third, after Cut. UI, small. |
| D8 | A face at the top under "Each picked face" is skipped with a note (P3.7 lead ruling) [built] | A zero-depth area gives no level [code] | — | — | MATCH (the note is more than the references give). |
| D9 | Drill top, depth and break-through | — | — | — | SETTLED Q2 [wt]. |
| D10 | An op moved to another setup keeps an absolute value | — | — | — | SETTLED Q5 [ruled]. |

### 2.4 Holes

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| H1 | A Drill drills only circles within `size_tolerance` (default 0.01 mm, `C/selection/key.rs:38`) of the tool; a picked misfit refuses (P6.5, P6.5b) [built] | Area within 5 % (about 2.5 % of the diameter) (ID Q9) [code] | Every hole with radius >= tool radius: it drills a large hole undersize (ID Q9) [code] | Groups by diameter (ID Q9) [docs] | DIFFERS-KEEP. Operator ruling P6.5b: strict default, a per-op tolerance. |
| H2 | A point drills at the tool size | Points select (Kl `cd_points`) [code] | Vertex targets [code] | Points have no diameter; Same Diameter needs faces (forum) [forum] | MATCH. |
| H3 | Holes larger than the tool give "N holes to bore: Add a Bore for these" (P6.7b) [built] | Separate Helical op (Kl `ch_selc_l`) [code] | Separate Helix op (ID Q8) [code] | Hole recognition: actions per group (ID Q9) [docs] | MATCH (a one-line fix act, no wizard). |
| H4 | "Add a Bore" takes the Drill's tool [built] | — | — | — | SETTLED Q8 (a flat end mill that fits). |
| H5 | Bore keeps holes larger than the tool; a point does not serve (P6.6) [built] | Holes that are too small are ignored (Kl `ch_selc_l`) [code] | "find all": radius >= tool radius (ID Q8) [code] | Tool fits or a smaller substitute (ID Q8) [docs] | MATCH. |
| H6 | Bore climb = true (`operation_configs.rs:533`) | `camHelicalClockwise: true` (Kd 501): CW in a hole with M3 = conventional [code] | Helix CutMode "Conventional" (F `Op/Helix.py:360`) [code] | Climb by default [UI] | UNCLEAR (see 4). |
| H7 | Drill cycle default Peck (G83), peck 3 mm (`operation_configs.rs:396`) | Pecks: `camDrillDown` 5, lift 2 (Kd 478, 483) [code] | `PeckEnabled` off: plain G81 (F `Op/Drilling.py:234, 711`) [code] | "Drilling - rapid out" (G81) [UI] | UNCLEAR (see 4). |
| H8 | Mixed sizes: one Drill per size; the Holes job in the P7 wizard | — | — | — | SETTLED Q9. |
| H9 | One hole is one row: "Ø6 hole, 8 deep" (R-E) [wt] | n/a | Base list per face [code] | Hole recognition list per diameter (ID Q9) [docs] | MATCH. |

### 2.5 Open paths

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| O1 | A closed-only op skips an open path under the default rule with a note; a picked one refuses (D2) [built] | Trace "follow" takes lines; clear takes loops (Kl `cu_type_l`) [code] | Pocket of an open wire: no face, no cut [UI] | Discards an open contour in silence (GAP addendum) [forum] | DIFFERS-KEEP. GAP addendum: a silent discard is the defect to avoid. |
| O2 | Trace default is on the line; Left/Right follow the drawn direction (O-1) [built] | `camTraceOffset` "none", type "follow" (Kd 623-628) [code] | Engrave follows the wire (F `Op/Engrave.py:254-283`) [code] | 2D Contour follows an open chain; side by the arrow [docs] | MATCH. |
| O3 | Pocket "extend to stock" for an open path, off by default (P6.4, O-2) [built] | n/a | Pocket Extensions grow an edge outward (F `Op/FeatureExtension.py`) [code] | Tangential extension; Stock contours (Fu `GUID75B6821B...`) [docs] | MATCH. |
| O4 | No lead past the drawn ends of an open Trace (P1 lead ruling) [built] | No extension (Kd `camTraceOffOver` 0) [code] | No extension [code] | Tangential extension 0 unless set [UI] | MATCH. |
| O5 | Import join 0.01 mm, settable, with a report (P4.8) [built] | — | — | — | DIFFERS-KEEP (GAP addendum, already checked). |

### 2.6 Tabs

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| T1 | Tabs off by default (`tab_count: 0`) | — | — | — | SETTLED Q3 (MATCH). |
| T2 | Auto tabs on straight runs, one or more per side, plus hand tabs in the viewport (P5, P4.6) [built] | Radial auto tabs by count and angle, or a click (Kl `ct_numb_l`, `ct_nabl_l`) [code] | Tags dressup: 2 to 4 per wire by length, positions editable (F `Dressup/Tags.py:733`) [code] | By distance or by points (Fu 2D contour "Tabs") [docs] | MATCH. |
| T3 | A delete of the last tab of a loop refuses (P4 O-4) [built] | Any tab can go [UI] | A tag can be disabled [code] | "By points" with no point on a contour leaves it free [UI] | DIFFERS-CHANGE: allow the delete; a note says "this part is freed at the last pass". This overturns lead ruling P4 O-4 (refuse; a per-loop override later). UI, small. |
| T4 | `tab_width` = the material bridge (P5b); height holds on every level | Width 5, height 5 (Kd `camTabsWidth/Height`) [code] | Width and height from preferences [code] | Width and height fields [docs] | MATCH. |

### 2.7 Import

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| I1 | SVG at its declared size, every transform applied (O-2, 0458a586) [built] | SVG imports as an extruded part [UI] | Draft SVG import by units [UI] | Insert SVG with a scale [UI] | MATCH (the SVG spec). |
| I2 | A drawing draws at import; each model has an eye (P4.D) [built] | Shown at import [UI] | Shown at import [UI] | Shown at import [UI] | MATCH. |
| I3 | STEP: no dialog; the size shows in the model panel (UX_REVIEW 2.F10) | STL: no dialog [UI] | Units from the file [UI] | Units from the file [UI] | MATCH. |
| I4 | A second import keeps its file coordinates; no note (FR-1 F15) | Parts are placed apart on the bed [UI] | File coordinates [UI] | File coordinates [UI] | MATCH (FreeCAD, Fusion). |

### 2.8 Refusals and notes

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| R1 | A refused or blocked op shows BLOCKED in red on the row and a dot on the owning tab (F20, F24) [built] | — | — | — | SETTLED Q4 (MATCH). |
| R2 | An add with no fitting tool refuses with a toast (f9a3d2b1) [built] | n/a | The op is made with the previous tool (ID Q8) [code] | The op is made; it does not generate until a tool is picked (ID Q8) [docs] | DIFFERS-CHANGE: make the op and give it the error state "no tool fits: needs <kinds>" (extends Q8 to every op). It needs a toolpath state with no tool, against the P6.7b note "a toolpath must have a tool". UI + core, medium. |
| R3 | The model gouge guard blocks the result and the export (UX-7b) [built] | Avoids by the shadow clip (Kw `op-area.js:181-196`) [code] | No model check in 2D ops (ID Q1, Deburr) [code] | No model check in 2D ops [UI] | DIFFERS-KEEP. Safety. |
| R4 | Uncut corner finding with Show me (UX-C) [ruled] | Nothing [UI] | Nothing [UI] | Nothing; rest machining finds it later [UI] | DIFFERS-KEEP. More information, no motion. |
| R5 | An empty selection refuses and names the skipped items and the fix (D2) [built] | A slice error toast (ID Q4) [code] | Error state on the object (ID Q4) [code] | Error icon, message on click (ID Q4) [docs] | MATCH. |
| R6 | A pick lost at re-import shows "Lost pick" with Show me and Remove (P4.3) [built] | n/a | Topological naming can break in silence [UI] | "Contour selection has missing selections" error (ID Q4 source) [docs] | MATCH (Fusion). |
| R7 | Stale: the dot colour; the word on hover only (UX_REVIEW 6.F1) | — | — | — | SETTLED Q4 (the word on the row) [ruled]. |
| R8 | The stale hover does not name the cause (6.F2) | Every change re-slices all [UI] | Touched mark, no cause [UI] | Out-of-date mark, no cause [UI] | MATCH. UX-13 would go past the references. |

### 2.9 Labels

| Id | rs_cam (record) | Kiri:Moto | FreeCAD | Fusion | Verdict |
|---|---|---|---|---|---|
| L1 | "Top Z", "Floor from:", "Selection" section (`floor.rs:118`, `boundary_2d.rs:20`) | — | — | — | SETTLED Q7 [ruled; HEAD still reads "Top Z"]. |
| L2 | Boundary source "Selection" (`V/ui/properties/toolpath_panel.rs:643`) | n/a | n/a | Machining boundary "Selection" [docs] | MATCH (after Q7 renames the section to Geometry). |
| L3 | Chip names "Loops", "Open", "Circles", "Faces", "Points" (`V/ui/pick_bar.rs:75-76`) | "lines", "loops", "surface" (Kl `cc_sele_l`) [code] | Edges, Faces, Vertexes [UI] | Contour, chain, face, point [docs] | DIFFERS-CHANGE: "Closed contours", "Open contours". UI, small. |
| L4 | Trace "Compensation": Left/Right of the drawn direction (`V/ui/properties/operations/engrave.rs:14-32`) | trace offset (Kd `camTraceOffset`) [code] | Profile Side [code] | "Sideways compensation" Left/Right [docs] | MATCH. |
| L5 | "Start from: After previous ops" is the rest door (UX_REVIEW 3.F1) | No true rest (GAP) | Adaptive "Use stock" [UI] | "Rest machining", source "From previous operation(s)" (GAP sources) [docs] | DIFFERS-CHANGE: name the row "Rest machining: from previous ops". UI, small. |
| L6 | Op names "3D Finish" (drop cutter), "Adaptive" (2D), "3D Rough" (`C/compute/catalog/registry.rs:185, 679, 732`) | "contour", "rough" [code] | "3D Surface", "Adaptive" [code] | "Parallel", "2D Adaptive Clearing", "Adaptive Clearing" [docs] | DIFFERS-CHANGE: "Parallel finish", "2D Adaptive", "3D Adaptive". UI, small; wire names stay. |
| L7 | Add menu group "3D (from STL)" while a STEP also serves (`V/ui/toolpath_panel.rs:1211`) | n/a | n/a | n/a | DIFFERS-CHANGE: "3D (from a mesh: STL or STEP)". UI, small. |
| L8 | A hole ring in a Profile region reads "Island" ("the op cuts round it", `C/selection/view.rs:189`) | n/a | Holes are "processHoles" [code] | "Inner contour"; islands are a pocket word [UI] | UNCLEAR (see 4). |

### 2.10 Journeys (steps estimated from FR-1, FR-2 and UX_REVIEW; not driven)

Counts are steps, not measured clicks. The reference order comes from their
docs and panel order.

| Id | Journey | rs_cam steps (order) | Reference order | Verdict |
|---|---|---|---|---|
| J1 | DXF/SVG sign: pocket, profile, V-carve | 1 import (dialog); 2 + Pocket (default: every closed loop, nested); 3 Pick + Shift-click the letter layer if only letters; 4 + Profile (floor through at add); 5 tabs count; 6 + VCarve, pick the layer; 7 Generate All. About 10-14. | Fusion: insert DXF in a sketch, setup and stock, then per op tool, then select each profile, then heights (about 5 per op plus one per letter). FreeCAD: import, Job, faces from wires, Base per face, depths. Kiri: SVG is an extruded part; Outline global, Pocket per surface. Vectric (peer): select vectors, op, calculate. | MATCH on order. Fewer steps than Fusion/FreeCAD. P3 (Shift = layer) is the step that would surprise. |
| J2 | STEP part: pocket the steps, profile round, drill, chamfer | 1 import; 2 + Pocket: rings at each face Z (3 steps); 3 + Profile: outline needs R-A [ruled] (today: not possible, FR-1 F4); 4 + Drill: rim circles, each hole floor [wt]; 5 + Chamfer: convex edges [built], refuses on a buried face until a wall op runs (F34). | Fusion: Pocket = pick floor faces, bottom = selected contour; Contour = Silhouette or chain; Drill = hole faces + Same Diameter; Chamfer = edges + clearance. FreeCAD: Pocket Base faces; Profile empty Base = envelope; Drilling auto-select; Deburr edges. | MATCH after R-A and R-B land. Today the Profile step is blocked (FR-1 click table). |
| J3 | 3D relief from STL | 1 import; 2 stock; 3 + 3D Rough; 4 + finish; 5 Generate All; 6 simulate. About 6. | Kiri: import, rough, contour (global, 0 picks). FreeCAD: mesh to shape, Job, 3D Surface. Fusion: mesh to body first. | MATCH (Kiri). The four rest doors (L5) are the friction. |
| J4 | Holes of mixed sizes | 1 + Drill tool A (fits drilled, others noted); 2 "Add a Bore for these"; 3 a second size: + Drill tool B, rule Ø (about 4 steps). | Fusion: hole recognition groups by diameter in one dialog. Kiri: one Drill per tool, "Select Matching Diameter". FreeCAD: one Drilling, all holes >= tool. | MATCH (Kiri). The Fusion one-dialog form is SETTLED Q9 (P7 wizard). |
| J5 | Two-sided job with pins | 1 setup + (Face Up Bottom); 2 "Add alignment pins for this flip" or the pin wizard; 3 ops in setup 2; carried stock. Two pin doors (UX_REVIEW 5.F3). | Kiri: "register" op drills registration holes, "flip" op (Kl `cd_regi_l`, `cf_menu`; Kw `op-register.js`). FreeCAD: no pin or flip flow; a second Job or work plane. Fusion: a second setup with its own WCS; no pin tool. | DIFFERS-KEEP: rs_cam is ahead. One pin door (UX_REVIEW 5.F3) stays a lead item. |
| J6 | Edit and re-run | Edit; the op and its dependents go stale; Generate All skips current ops; export refuses stale. | Fusion: out-of-date mark, user generates. FreeCAD: recompute regenerates touched ops. Kiri: re-slice all. | MATCH. |

## 3. Proposed changes (DIFFERS-CHANGE only), ranked by how much a user feels it

| Rank | Id | Change | Kind | Size |
|---|---|---|---|---|
| 1 | P4 | A click on an item that the op can cut picks it, whatever the chip state. The chips stay as a filter. | UI | small |
| 2 | P3 | Shift-click adds one item, as in CAD. "All on this layer" moves to a row act or a right-click. | UI | small |
| 3 | D6 | Edit the floor as "Bottom height" on the Heights tab, under "Top height". | UI | medium |
| 4 | R2 | An add with no fitting tool makes the op and gives it the state "no tool fits: needs <kinds>". | UI + core | medium |
| 5 | D7 | Tab order: Geometry, Cut, Heights, Feeds & Speeds, Linking, Dressup. | UI | small |
| 6 | D1 | Add the floor reference "Model bottom". | UI (motion only when chosen) | small |
| 7 | L3 | Chip names "Closed contours", "Open contours". | UI | small |
| 8 | T3 | Allow the delete of the last tab of a loop, with a note that the part is freed. | UI | small |
| 9 | L5 | Name the rest row "Rest machining: from previous ops". | UI | small |
| 10 | L6 | Op names "Parallel finish", "2D Adaptive", "3D Adaptive" (wire names stay). | UI | small |
| 11 | L7 | Add menu group "3D (from a mesh: STL or STEP)". | UI | small |
| 12 | S9 | A new op takes the tool of the last op in its setup when it accepts it, else the first that fits (FreeCAD only, 1 of 3). | motion (tool choice) | small |

Rows 1, 2, 5, 7, 9, 10 and 11 can go in one GUI pass with UX-10 (the names
pass). Row 3 needs the F19 heights work first.

## 4. UNCLEAR rows

| Id | Question | What settles it |
|---|---|---|
| S11 | Pocket depth at the add is min(half the stock, 5 mm). No record gives the reason. A drawing has no depth, so every reference needs a typed value. | The operator states the wanted default, or the P7 job types set it. |
| H7 | Drill cycle default Peck (G83). FreeCAD and Fusion use G81; Kiri pecks. No record gives the reason. | An operator ruling (wood chip packing argues for Peck), or a Fusion new-Drill screenshot for the Fusion default. |
| H6 | Bore direction. Kiri and FreeCAD helix in conventional by default; rs_cam climbs, as its Pocket and Profile do. | A Fusion Bore default (the manual or a new op), then an operator ruling. Climb is the usual wood choice for the other rs_cam ops. |
| L8 | "Island" on a Profile hole ring. A held generation test (PLAN.md 2026-10-22) decides whether the role is right. | Run the held test (near hole and far hole, Inside and Outside). Then pick "Island" or "Inner wall". |
| D3 (Fusion cell) | The Fusion default Top height reference is not in the manuals found. The verdict holds on Kiri and FreeCAD. | Open a new 2D Pocket in Fusion and read the Heights tab. |
| S12 (Fusion cell) | The Fusion stock offset default is not in the manuals found. | Open a new Setup in Fusion and read the offset fields. |

## 5. Sources not in INDUSTRY_DEFAULTS.md

- Kiri: github.com/GridSpace/grid-apps master, `src/kiri/app/conf/defaults.js`
  l.445-640; `web/kiri/lang/en.js` l.520-690, 830-845;
  `src/kiri/mode/cam/work/op-area.js` l.30-240 [code].
- FreeCAD: github.com/FreeCAD/FreeCAD main, `src/Mod/CAM/Path/Op/PocketShape.py`
  l.100-230, `Op/Profile.py` l.276-332, `Op/Drilling.py` l.212-240, 709-720,
  `Op/Engrave.py` l.243-300, `Op/Deburr.py` l.210-215, 437-444,
  `Op/Helix.py` l.360-366, `Main/Stock.py` l.177-182,
  `Dressup/Tags.py` l.733-765, 950, `Op/Gui/Base.py` l.797-1557 [code].
- Fusion: `GUID75B6821B-DE26-4E3B-AF10-4A54131CD9E4` (2D Contour reference),
  `MFG-REF-TOP-HEIGHT`, `MFG-REF-BOTTOM-HEIGHT`, `LP-STEP-IRONSTRATEGY_DRILL`,
  `MFG-REF-SETUP-STOCK` [docs]; forums.autodesk.com "one-level-pocket/td-p/5468004"
  (bottom from contours), "cam-select-same-hole-allow-exceptions/idi-p/6954988"
  (no exclusion), "change-defaults/td-p/11918336" (stock offsets) [forum].
