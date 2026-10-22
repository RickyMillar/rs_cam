# A Geometry tab: design study (2026-10-22)

Owner: the lead session. Reader: the operator. Status: planning only. No code
changes come from this file until the operator approves revision 2 and rules
the open questions GT-Q7 to GT-Q9 (section R2.9). A plain "Q1" to "Q4" in this
file is a UX_REVIEW.md question, ruled on 2026-10-22.

Mockup: `geom_tab/mockup.html` (open it in a browser; no build step). Revision
2 shows the Geometry tab in the toolpath inspector. The three-concept switcher
of revision 1 is gone.

File references use two short roots: **V** = `crates/rs_cam_viz/src/`,
**C** = `crates/rs_cam_core/src/`. Revision 2 line numbers are at HEAD
bd7d5caa. Revision 1 line numbers (sections 1 to 7) are at HEAD 669a3b56.

---

## Revision 2 (operator direction 2026-10-22)

### R2.1 The operator's words

> "I meant it should be its own tab in the toolpath menu, not in the
> workspace. but. great look. I dont think we should list all available
> things, just selected ones. Just because some models will blow this out
> really quickly."

### R2.2 Rulings and lead decisions

PLAN.md "Operator rulings 2026-10-22 on the Geometry tab study" holds these.

| Question | Revision 1 recommendation | Revision 2 (ruled or decided) |
|---|---|---|
| GT-Q5 Placement | A fifth workspace (W) | **Operator:** a tab of the toolpath inspector. The workspace bar stays Setup / Toolpaths / Simulation / Readiness. |
| GT-Q6 Browse | Browse every item with no op selected | **Operator:** the tab lists the selected items only. No browse view. |
| GT-Q2 Name | Workspace "Geometry"; inspector tab "Cut" | **Lead:** the new inspector tab is "Geometry". The present "Geometry" tab keeps the tool, input and op parameters and is renamed "Cut". |
| GT-Q1 Concept | B with C's tree, sets later | **Lead:** B (op editor). C's group-by-facts becomes "make a rule from this picked item". A (named sets) waits. |
| GT-Q3 One door | Pick only in the tab | **Lead:** Pick moves to the Geometry tab. |
| GT-Q4 Named sets | Decide after a real job | Waits with A. |

The rulings of the same day still bind: O-PICK (the first pick replaces the
default rule), Q2 (the first face pick sets "Each picked face", depth 0), Q3
(the STEP default takes the face rings; on a DXF the default takes the kinds
that the op cuts), Q4 (the floor reference "A picked face"), and "auto magic
is a wizard".

**The wizard ruling.** The "Rule…" offers (R2.6, L-R4) are not recognition.
They read the facts of one item that the operator picked: its kind, its
diameter, its face Z. Each offer is one `Filter` (`C/selection/rule.rs:121`).
Nothing proposes an item that the operator did not pick. So the tab obeys
the ruling and gets no wizard button.

### R2.3 Layout

The Toolpaths workspace does not change. The inspector gets six tabs.

```
 Setup | Toolpaths | Simulation | Readiness            (no new workspace)
┌ TOOLPATHS ─────┬─────────────── viewport ──────────────┬ DRILL 1      [Generate] ─┐
│ Setup 1        │ Pick for Drill 1: (Faces)(Rings)(Edges)│ Cut | Geometry | Feeds & │
│  ● Pocket 1  4 │   [Circles] [Points] [Done]            │ Speeds | Linking |       │
│  ● Profile 1 4 │ ┌──────────┬──────────────────┐        │ Heights | Dressup        │
│  ● Drill 1  25 │ │  ┌────┐  │ ○ ○ ○ ○          │        │ Cuts      25 circles ✓   │
│  ● Chamfer 1 4 │ │  │    │  │ ○ ○ ○ ○     ◯    │        │ Selected  25 items       │
│                │ │  └────┘  │ ○ ○ ○ ○  (24 Ø6, │        │ [Pick] [Reset]           │
│                │ │          │ ○ ○ ○ ○   1 Ø10) │        │ [Filter the selected…  ] │
│                │ └──────────┴──────────────────┘        │ SELECTED ITEMS (25)      │
│                │                                        │ ▸ All circles Ø 6.00  24×│
│                │  "2 items are here. Choose one."       │ ▾ Picked circles       1 │
│                │                                        │   Ø 10.00 at (106, 40)   │
│                │                                        │     Cut [Rule…] [×]      │
│                │                                        │ NOTES                    │
│                │                                        │ Rule added … (24).       │
└────────────────┴────────────────────────────────────────┴──────────────────────────┘
```

Tab order: **Cut | Geometry | Feeds & Speeds | Linking | Heights | Dressup**.
Cut stays first because a new op needs a tool before it can cut. Question: the
operator may want Geometry first (one click less). The mockup keeps Cut
first and opens Geometry.

### R2.4 What moves between the two tabs

Today's tab is `draw_geometry_tab` (`V/ui/properties/toolpath_panel.rs:553`).

| Part today | Function | Revision 2 tab |
|---|---|---|
| Tool, input model, stock source | `draw_geometry_wiring` (`V/ui/properties/tab_badges.rs:487`) | Cut |
| Start from (rest stock) | `draw_start_from` (`tab_badges.rs:553`) | Cut |
| Op parameters and the ⚡ Suggest pills | the per-op draw after the Selection section | Cut |
| The op Selection section | `draw_selection_section` (`V/ui/properties/selection_section.rs:151`) | Geometry (rebuilt as R2.5) |
| "Each picked face" hint | `draw_each_face_hint` (`toolpath_panel.rs:1247`) | Goes. Q2 sets the floor at the first face pick. |
| Boundary pick row | `draw_boundary_pick_row` (`toolpath_panel.rs:518`) | Open: GT-Q7 |
| Floor reference (P3.7) | Heights tab, the height diagram (`draw_heights_tab`, `toolpath_panel.rs:1312`; `V/ui/properties/operations/height_diagram.rs`) | Heights. The Geometry tab shows the floor face only. |

### R2.5 The Geometry tab, top to bottom

1. **Summary.** "Cuts" (the `counts_text` line of today, with ✓ or the fix),
   "Selected N items", and "Floor" on an op that has a floor.
2. **Actions.** "Pick" turns the pick bar on in the viewport. "Reset" brings
   the default rule back. The `+ Rule` card goes (R2.6, L-R4 replaces it).
3. **Filter box.** It filters the rows of the list. It does not search the
   model.
4. **The selected list**, in this order:
   - the default row, while the op holds its default rule;
   - one row per rule, with its match count and "×";
   - one group per picked kind ("Picked circles 3").
   An item row shows the item label, its state word (Cut, Island) and its
   acts: "Rule…", "Use as floor" (a face on a floor op), "×".
5. **Floor face.** On a floor op only: the face that "Use as floor" set, with
   "Clear".
6. **Notes.** The findings of today (`draw_findings`), the hole size rows,
   the lost picks and the refusal of the last act. A one-line fix action such
   as "Add a Bore for these" stays (the wizard ruling allows it).

### R2.6 Behaviour rules (proposed, new in revision 2)

- **L-R1 Selected items only.** The list never shows an item that the op does
  not name. The list has one header row per group. A group with more than 8
  rows starts collapsed. An open group shows 8 rows, then "Show N more". The
  GUI draws the rows with `egui::ScrollArea::show_rows`, so it lays out only
  the visible rows. The filter keeps the full count on the group row ("6 of
  24").
- **L-R2 The default row.** It is one collapsed row: "Default: every face
  ring (4)". N is the count of the default rule's members. On a DXF the row
  names the kinds that the op cuts. On Drill with a STEP model the row reads
  "Default: nothing that Drill 1 cuts (0)". The first pick replaces the row
  (O-PICK). Reset brings it back.
- **L-R3 A pick inside a rule.** A click on an item that a rule holds changes
  nothing. The note names the rule. To drop one item of a rule, remove the
  rule (GT-Q8 asks about an exclusion).
- **L-R4 Make a rule from a pick.** "Rule…" on a picked row offers rules from
  the facts of that item:

  | Picked item | Offers | Filter |
  |---|---|---|
  | Circle Ø 6 | "All circles Ø 6.00 on this model"; "All circles on this model" | `diameter` range; `kinds` |
  | Face at Z 20 | "All up faces at Z 20" | `FaceFilter { facing, z }` |
  | Ring, edge, point at Z 32 | "All edges at Z 32"; "All edges on this model" | Z: needs GT-0; `kinds` |

  Each offer shows its match count. The new rule replaces the picks that it
  holds and every rule whose members it holds. An offer that an existing rule
  already holds changes nothing. The act is one `SelectionEdit::Replace`
  (`C/selection/rule.rs:286`), so it is one undo step.
- **L-R5 Use as floor.** It shows on a picked face row of a floor op. A row
  in the default row or in a rule row has no acts (L-R3 gives the reason). "Use as floor" sets
  the Q4 floor reference "A picked face" with that face key. The face leaves
  the picks, so the op does not cut it (the revision 1 journey "cut round the
  top face down to the step face"). The floor face slot shows it with
  "Clear". Clear sets "Each picked face" when face picks remain, else the op
  default. This is the one door for a floor face; it replaces the floor-row
  Pick of UX_REVIEW.md 2.4.
- **L-R6 Refusal.** A click on an item of a kind that the op cannot cut
  changes nothing. The Notes block shows the core reason
  (`kind_not_cut_reason`, `C/selection/refusal.rs:162`) until the next act.
  The pick bar greys that kind's chip and gives the same reason as hover.
  The click stays possible, so the operator sees why.
- **L-R7 Coincident items.** A click on a line that holds more than one item
  opens the chooser ("4 items are here. Choose one."). Section 1.3 gives the
  key requirement (GT-0).
- **L-R8 The first face pick.** Q2: on a floor op whose floor is the op
  default, the first face pick sets "Each picked face", depth 0, and writes a
  one-line note.

### R2.7 Core, MCP and CLI

- **Core.** No new selection state. The tab sends `EditToolpathSelection`
  only (L-1). It needs UX-3 (O-PICK in core), UX-6 (Q2), UX-7 with GT-0 (the
  Z on BREP ring, edge, circle and point keys) and UX-8 (the floor reference
  "A picked face"). A Z rule on a ring, an edge or a point needs a Z range in
  `Filter` for those kinds (GT-0b).
- **The tab key break.** `ToolpathTab` (`V/ui/properties/mod.rs:187`) gains
  `Cut`; `Geometry` now means the new tab. The MCP `set_ui_view
  properties_tab` wire type (`PropertiesTabParam`,
  `crates/rs_cam_mcp/src/server.rs:914`) gains `"cut"`, and `"geometry"` opens
  the new tab. No alias stays: the operator ruling of 2026-09-16 ("no legacy,
  breaking OK") covers the break. The commit names it.
- **MCP and CLI parity.** The selection tools do not change:
  `list_model_items`, `edit_toolpath_selection` (`add_rule` with the offer's
  filter, `replace`), `selection_status`. The CLI goes through core as today.

### R2.8 The mockup (revision 2)

- The app frame: the workspace bar, the Toolpaths list, the viewport, the
  inspector with six tabs. The Geometry tab opens first. Cut shows
  placeholder values. The other four tabs are inert.
- The model: the stepped block of revision 1, with a grid of 24 holes Ø 6
  and one hole Ø 10 on the step face (44 items). Drill 1 shows how a rule row
  keeps 24 holes in one row.
- Acts: switch ops; click items in the view to pick and unpick; the chooser;
  the kind chips; "Rule…"; "Use as floor"; "Clear"; Reset; the filter box; a
  refusal (for example a face click on Drill 1).
- The self-check footer asserts:
  1. every model item has one view element;
  2. the keys without Z collide for the pairs of section 1.3;
  3. every default rule takes kinds that its op cuts;
  4. the list length equals the selected count (each member is in one group
     only), the default row count equals the default rule's members, and a
     rule row's count equals its matches, for every op;
  5. the drawn list agrees with the list model;
  6. a scripted session of 11 acts (picks, two rules, a pick inside a rule,
     Q2, Use as floor, a refusal, a pick of one of two coincident edges,
     Reset) keeps claim 4 true
     and gives the named results.
- Simplifications: a hole boundary is a circle item only, not also a face
  ring (GT-Q9). The holes have no bottom circle. One model only.
- Headless Chrome 2026-10-22: the footer reads `data-result="pass"` at load
  and after scripted clicks (a Ø 6 pick and its rule, a Ø 10 pick, Use as
  floor, a refusal, the filter "(66"). At 500 px width the page has no
  horizontal scroll.

### R2.9 Open questions for the operator

1. **GT-Q7 — The boundary selection.** A Pocket boundary can come from a
   selection (`SelectionTarget::Boundary`, `draw_boundary_pick_row`). Does
   the Geometry tab hold it too?
   - **Recommendation:** yes, as a second block "Boundary" under the
     selected list, with the same rows. One tab owns every pick.
2. **GT-Q8 — Drop one item of a rule.** For example, drill 23 of 24 holes.
   (a) An exclusion list in `Selection` (core change). (b) "Rule → picks":
   turn the rule into 24 picks, then remove one.
   - **Recommendation:** (b) now; (a) only if a real job asks.
3. **GT-Q9 — Hole boundaries on a STEP.** Is a round hole edge a face ring,
   a circle, or both (two coincident items)? UX-7 decides. The mockup uses a
   circle only.
4. **Tab order.** Cut first (the mockup) or Geometry first?

### R2.10 Packages (revision 2; replaces section 7.3)

| WP | Content | Needs | Waits for |
|---|---|---|---|
| GT-0 | UX-7 key requirement: BREP ring, edge, circle and point keys carry Z or a topology id | core | nothing |
| GT-0b | A Z range in `Filter` for rings, edges and points | core | GT-0 |
| GT-1 | The tab split: `ToolpathTab::Cut`, the new Geometry tab, the moves of R2.4, the MCP key `"cut"` | GUI + MCP wire | approval of revision 2 |
| GT-2 | The selected list: groups, default row, cap and "Show N more", `show_rows`, filter box, row-viewport hover, the chooser | GUI | GT-0, GT-1 |
| GT-3 | "Rule…" offers (L-R4), one `Replace` per act | GUI | GT-2 (GT-0b for Z offers) |
| GT-4 | "Use as floor" and the floor face slot (L-R5) | GUI | UX-8 |
| GT-5 | Named sets (A) | core + GUI + MCP | GT-Q4, after a real job |

Each package follows the PLAN.md per-item flow: an editor, a sentry with a
red proof, the lead review, then the operator look on a release GUI built
from HEAD.

---

# History: revision 1 (2026-10-22, before the operator direction)

The sections below are the first study. **Superseded:** the workspace
placement (W and M in section 2), concept A as the build target (section 3),
the browse view, and the recommendations of section 7. They stay as the
record of the options. Concept B (section 4) and the facts of concept C
(section 5) feed revision 2.

## 1. The problem

### 1.1 The operator's words (2026-10-22)

> "I think this is complex enough that we might need to reimagine the ui and
> have a seperate geom tab. thats more intuative"

The earlier words of the same day:

> "the picker isnt ideal for steps. it just selects faces, the edges of faces
> are also loops! and the corners are points! but I cannot seem to select
> this?"

> "I did a profile on 2 faces, it seems like it cut both to the same height?
> not to the face? im confused"

> "i would have thought that because I didnt select the circle in the op. it
> would not have been considered?"

### 1.2 What a tab fixes, and what it does not fix

None of the three quotes above is a layout defect. Each has a core cause, and
the operator has already ruled each fix:

| Quote | Friction | Cause | Fix (ruled) | Does a tab fix it? |
|---|---|---|---|---|
| "the edges of faces are also loops" | 2.F3, 2.F4, 2.F5 | A STEP gives face items only (`C/io/mod.rs:263-308`) | Q3 → UX-7: rings, edges, circles, points become items | **No.** A tab without UX-7 has nothing new to show. |
| "cut both to the same height" | 2.F2 | The floor "Top Z" reads one op top | Q2 → UX-6: the first face pick sets "Each picked face" | **No.** |
| "I didnt select the circle" | 1.F2 | A pick adds to the default rule | O-PICK → UX-3 | **No.** |
| (review) | 2.F8 | A new op on a STEP starts red: the default rule takes no face | Q3 → UX-7: the STEP default takes the face rings | **No.** |
| (safety, measured) | 2.F1 | "auto: faces" top below the stock top | Q1 → UX-1 | **No.** |

So a tab is the second layer. It is worth its cost only after UX-1, UX-3, UX-6
and UX-7 land. It then fixes these frictions:

| # | Friction (UX_REVIEW.md) | How a tab fixes it |
|---|---|---|
| 1.F3 | A click and a shift-click act by two rules | The tree has a row per layer and a row per item. A click on a layer row adds a rule; a click on an item row adds a pick. The act is visible, not a modifier key. |
| 1.F6 | The `+ Rule` card and the kind chips are hard to understand | The tree groups are the rule filters (layer, kind, diameter, face Z). A group row is the rule. The card goes. |
| 1.F9 | "Selection" names three things | The tab owns the word for the item set; the inspector shows one summary line. Question GT-Q2 (section 7.2) settles the names. |
| 2.F7 | No place to pick "a face as the floor" | A face row has "Use as floor" (the Q4 reference "A picked face"). It replaces the floor-row Pick of UX-8, so one door sets a floor face. |
| 3.F3 | An STL flat face has no highlight after a pick | A tree row is the highlight: the row and the face light together. |
| NEW | Coincident items (section 1.3) | The tree shows two rows where the top view shows one line. |
| NEW | "What does this model hold?" | *(Withdrawn by GT-Q6: the tab lists the selected items only.)* |

### 1.3 A new problem that UX-7 brings: coincident items

`MatchKey` has no Z (`C/selection/key.rs:42-58`). Two keys match when kind,
layer, BREP id, anchor XY and measure agree (`key.rs:112-119`). The BREP id is
set for a `Face` only (`key.rs:76-80`). On a stepped block after UX-7:

1. The inner ring of the top face (the pocket rim, Z 32) and the outer ring of
   the pocket floor face (Z 24) have one XY ring. Both are `Loop`, layer
   `None`. So both get one anchor and one measure: **equal keys**.
2. The two vertices of a vertical edge are two `Point` items at one XY with
   measure 0: **equal keys**.
3. The top edge and the bottom edge of a vertical step wall are two `Path`
   items with one XY line: **equal keys**.
4. A vertical edge projects to a path of length 0. `Item::new` checks the
   shape invariants (`item.rs:278`), so the source must drop it or the item
   must keep its Z.

The invariant "two items with equal keys stay two; a pick on them is
ambiguous" (`C/selection/CLAUDE.md`) then makes each of these picks a
lost pick. **Requirement for UX-7:** a BREP ring, edge, circle and point key
carries its Z, or a BREP topology id as a tie-break (as `Face` does). This
holds for every concept below.

The top view also draws these items on one line. A viewport click cannot
choose between them. A tree with one row per item can. This is the strongest
reason for a list-based tab.

---

## 2. Facts that bind every concept

- **Rulings.** O-PICK (a first pick replaces the default rule), Q1-Q4 of
  2026-10-22, D2/D13 (a named item that the op cannot cut refuses), and
  "auto magic is a wizard" (PLAN.md). P8 waits.
- **One writer.** Every edit goes through `ProjectSession::apply(Command)`.
  The Selection section sends `AppEvent::EditToolpathSelection` only
  (L-1, `V/ui/properties/selection_section.rs:10-13`). A tab does the same.
- **The report.** `selection_status` gives one `ItemState` per item: Cut,
  Island, Skipped, Excluded, NotSelected (`C/session/selection_status.rs:148`).
  The viewport and MCP read it. A tab reads it too; it adds no second
  answer.
- **The name clash.** The first inspector tab of an op is already
  "Geometry" (`ToolpathTab::Geometry`, `V/ui/properties/mod.rs:187`; MCP
  `set_ui_view properties_tab: "geometry"`). A workspace with the same name
  repeats friction 1.F9. Question GT-Q2 (section 7.2) asks which one changes.
- **Placement.** *(Superseded by GT-Q5: the tab is in the inspector.)*
  Revision 1 weighed (W) a fifth workspace, with six GUI touch points
  (`Workspace::ALL`, the layout and shortcut dispatch, the MCP workspace key,
  the badge arm), and (M) a mode inside Toolpaths, which keeps the pick bar
  gate (`V/ui/pick_bar.rs`).
- **Layout today.** Setup and Toolpaths both draw a left rail of 240 px, a
  right inspector of 280 px and the viewport between (`V/app.rs:510-651`).

---

## 3. Concept A: an item browser with named geometry sets (superseded, waits)

The Geometry workspace listed every item of each model in a tree. The
operator saved picks as **named sets** ("Outline", "Ø6 holes"), and an op
named one set. Two core forms existed: A-ref (`GeometrySet` in the project,
`Selection.sets`, four commands, a file key; an edit of a set stales every op
that names it) and A-copy (a set writes `SelectionEdit::Replace` into the op).
The risks: two homes for geometry, a delete that touches many ops, and new
browse state. A rule-only set can travel in a P7 template; a pick set cannot.
Revision 2 drops the browse tree (GT-Q6). Sets wait for GT-Q4.

---

## 4. Concept B: the Geometry tab edits one op

### 4.1 Idea

The Geometry tab is the large editor of the selected op's selection. The
inspector keeps one summary line and an "Edit geometry" button. The tab shows
the op's item states (Cut, Island, Skipped, Excluded, Not selected) in the
tree and the viewport at once. No new core state.

### 4.2 Layout (M: a mode of Toolpaths, or W with the op list on the left) (superseded by R2.3)

```
 Setup | Geometry | Toolpaths | Simulation | Readiness
┌ OPS ────────────────┬──────────── viewport ───────────┬ PROFILE 1 ───────────┐
│ Setup 1             │                                 │ Cuts  2 faces   ✓ OK │
│  ● Pocket 1         │   (items in their state colour) │ Floor Each picked    │
│  ● Profile 1   ◂    │                                 │       face, depth 0  │
│  ● Drill 1          │                                 │ Rules none           │
│─────────────────────│                                 │ Picks 2 faces        │
│ ITEMS of the model  │                                 │ [Reset] [Clear]      │
│ ▾ Faces             │                                 │──────────────────────│
│   ✓ Z 32 top   Cut  │                                 │ Notes                │
│   ✓ Z 20 step  Cut  │                                 │ none                 │
│     Z 24 floor  —   │                                 │                      │
│ ▸ Circles           │                                 │ [Back to Toolpaths]  │
│ ▸ Points (grey:     │                                 │                      │
│   Profile cannot cut│                                 │                      │
│   a point)          │                                 │                      │
└─────────────────────┴─────────────────────────────────┴──────────────────────┘
 Toolpaths inspector:  Geometry  2 faces · Cut · floor each face  [Edit geometry]
```

A tree row has a check box. On means "the op names this item". The state
word on the right is the report state. A row of a kind that the op cannot
cut is greyed with the core reason (`kind_not_cut_reason`, as the chip does
today).

### 4.3 Journeys

- **DXF sign.** Select Pocket 1; click "Edit geometry". The tree shows every
  item with "Cut" (the default rule). Click the TEXT layer row: O-PICK makes
  the first act replace the default, so only TEXT stays "Cut". The other
  items go grey at once.
- **STEP part (after UX-7).** Select Drill 1; Edit geometry. Faces are grey
  ("Drill cuts points and circles"). Open Circles; tick "Ø 6.00 × 2". Select
  Chamfer 1 in the op list (the tab stays open). Open Edges; tick "Step edge
  · Z 32". The coincident "Step edge · Z 20" row stays off.
- **Two-faced Profile.** Select Profile 1; tick "Z 32 top" and "Z 20 step".
  Q2 fires on the first tick: the panel line reads "Floor: Each picked face,
  depth 0 (set by the face pick)". Both faces read "Cut · floor Z 32" and
  "Cut · floor Z 20" (the report `floor_z`). For "round the top down to the
  step": untick "Z 20 step", then on the "Z 20 step" row choose "Use as
  floor" (Q4). The row gets a floor mark; the panel line reads "Floor: face
  Z 20".

### 4.4 Core model

No core change. The tab sends the same `EditToolpathSelection` edits as the
section (`AddPick`, `RemovePick`, `AddRule`, `Reset`), and O-PICK applies in
core. "Use as floor" sends the floor command of UX-8 (the Q4 reference "A
picked face", with the face key; not built). This button replaces the
floor-row Pick that UX_REVIEW.md 2.4 proposes, so one door sets a floor
face. The tree reads `list_model_items` data
(`ItemSet` per model) and the report `item_states`.

### 4.5 Risks

- The tab means nothing with no op selected. *(Resolved: an inspector tab
  shows only with an op selected.)*
- Two doors edit one selection (the tab and the inspector Pick). *(Resolved:
  Pick moves to the Geometry tab.)*

---

## 5. Concept C: a facts browser; recognition stays in a wizard (its facts feed L-R4)

### 5.1 Idea

The tree groups items by plain geometric facts: kind, layer, circle
diameter, face facing and face Z. Each group IS a rule filter that exists
today (`Filter { layers, kinds, diameter, face: FaceFilter { facing, z } }`,
`C/selection/rule.rs:121-180`). So a click on "Circles · Ø 6.00" adds a
diameter rule; it is not recognition.

Job words ("Pocket floors", "Ø6 holes for dowels", "Outline") are
recognition. They come only from a **"Find features…" wizard**, entered from
the setup `+` menu, the empty state or the File menu (the 2026-10-22 ruling).
The wizard writes ordinary ops with ordinary selections, or named sets if
concept A also lands. The tab gets no button for it.

### 5.2 Layout

```
┌ GEOMETRY · by facts ─┬──────────── viewport ───────────┬ PROFILE 1 ───────────┐
│ Group by [Kind ▾]    │                                 │ Rule: faces up,      │
│ ▾ Faces up      3    │                                 │   Z 20 to 20         │
│   Z 32          1    │                                 │ Cuts 1 face          │
│   Z 24          1    │                                 │                      │
│   Z 20          1  ◂ │                                 │ Picks none           │
│ ▸ Faces down    1    │                                 │                      │
│ ▾ Circles       3    │                                 │                      │
│   Ø 6.00        2    │                                 │                      │
│   Ø 10.00       1    │                                 │                      │
│ ▸ Edges · Z 32  4    │                                 │                      │
│ ▸ Edges · Z 20  4    │                                 │                      │
│ ▸ Points        8    │                                 │                      │
└──────────────────────┴─────────────────────────────────┴──────────────────────┘
 Recognition ("Find features…") opens from the setup + menu: a wizard.
```

### 5.3 Journeys

- **DXF sign.** Group by Layer. Click "TEXT · closed shapes": a layer rule.
  Same result as B; the group row tells the operator it is a rule ("every
  closed shape on TEXT, now and after a re-import").
- **STEP part (after UX-7).** Group by Kind. "Circles · Ø 6.00 (2)" → Drill
  1 gets the rule "circles Ø 6.00 ± 0.01". A re-import with a third Ø6 hole
  drills it too (S17). "Faces up · Z 20" → Pocket gets a face rule.
- **Two-faced Profile.** Shift-click (or check) "Z 32" and "Z 20": two face
  rules, or one rule "faces up, Z 20 to 32" when the operator drags across
  both rows. Q2 and Q4 work as in B.

### 5.4 Core model

GUI only for the groups: every group maps to an existing `Filter`. One gap:
an edge group by Z needs `FaceFilter`-like Z on edges, so UX-7 must give an
edge item its Z (the same requirement as 1.3). The wizard is P7/P8 work and
waits for the operator review (P8 ruling).

### 5.5 Risks

- A group that holds one item looks like a pick, but it is a rule. A
  re-import can add items to it. That is the point (S17), but it must be
  said in the row hover.
- The "Group by" control is one more choice. Two group orders (Kind, Layer)
  are enough.

---

## 6. The concepts side by side (revision 1)

| Facet | A: sets | B: op editor | C: facts browser |
|---|---|---|---|
| Placement | W (needs browse with no op) | M or W | W or M |
| Per-op Selection section | Becomes one row: a set combo plus "New from pick" | Becomes one summary line plus "Edit geometry" | As B; the group rows replace `+ Rule` |
| Core change | A-ref: `GeometrySet`, `Selection.sets`, 4 commands, file key, freshness. A-copy: storage only | None (needs UX-3, UX-6, UX-7, UX-8) | None for the groups; edge Z from UX-7 |
| MCP parity | NEW `list_geometry_sets`, `edit_geometry_set`; `edit_toolpath_selection {use_set}` | Existing `list_model_items`, `edit_toolpath_selection`, `selection_status`; `set_ui_view workspace: "geometry"` | As B; `add_rule` with the group filter |
| CLI parity | Through core (the project file holds the sets) | Through core (no new state) | Through core |
| P7 templates | A rule-only set travels by name; a pick set does not | No change | A group rule travels as any rule |
| Coincident items (1.3) | Tree rows solve it | Tree rows solve it | Groups by Z solve it |
| New GUI state | Browse pick, hover, set draft | Focus op (exists: the selected toolpath) | Group order |
| Cost (rough) | Core + GUI + MCP: large | GUI: medium | GUI: medium |
| Main risk | Two homes for geometry | Tab is empty without an op | Rule looks like a pick |

The three concepts share one part: **the item tree**. Every concept needs
it, and it is GUI only. The tree has one row per item and a group row per
filter value. A row and its viewport item highlight together. A click on a
coincident line in the viewport opens a small chooser ("2 items here").

---

## 7. Recommendation and open questions

### 7.1 Recommendation (superseded by R2.2)

1. **Order of work.** Land UX-1, UX-3, UX-6, UX-7 (with the key requirement
   of 1.3) and UX-8 first. The tab has little to show before UX-7.
2. **Build B with C's tree, as a fifth workspace "Geometry"**, between Setup
   and Toolpaths. With an op selected, it is the op editor (B). With no op
   selected, it browses the items with neutral colours. The tree groups by
   facts (C), so a group row is a rule and an item row is a pick. GUI only.
3. **The inspector** keeps one summary line per selection ("2 faces · Cut ·
   floor each face") and an "Edit geometry" button. The `+ Rule` card goes.
   The Pick button moves to the tab (one door; Q3).
4. **Named sets (A) wait.** They are the only part that needs a new core
   concept, and they answer reuse across ops (S27), which no operator report
   asks for yet. Decide after the operator has used the tab on a real job.
   If they come, A-ref with "a click edits the set" is the cleaner model, but
   it moves geometry out of the op.
5. **Recognition** stays in a P8 wizard. The tab gets no button for it.

### 7.2 Open questions for the operator (ruled or decided; see R2.2)

1. **GT-Q1 — Which concept?** (A) sets, (B) op editor, (C) facts browser, or
   the recommendation (B with C's tree, sets later).
2. **GT-Q2 — The name.** The op inspector's first tab is "Geometry". Choose
   one:
   - (a) The workspace is "Geometry"; the inspector tab becomes "Cut".
   - (b) The workspace is "Items"; the inspector tab stays "Geometry".
   - **Recommendation: (a).** The operator said "geom tab".
3. **GT-Q3 — One door or two.** Does Pick stay in the inspector, or does it
   live only in the Geometry tab?
   - **Recommendation:** only in the tab. The inspector shows the summary
     and "Edit geometry".
4. **GT-Q4 — Named sets (only if A).** (a) By reference: edit once, every op
   follows. (b) By copy: a set is a starting point. And with a set bound,
   does a click edit the set or detach the op?
   - **Recommendation:** decide after a real job with the B tab.
5. **GT-Q5 — Placement.** A fifth workspace (W), or a mode inside Toolpaths
   (M)?
   - **Recommendation: W.** A browse with no op selected needs its own page.
6. **GT-Q6 — The browse view.** With no op selected, does the tab browse
   (neutral colours, item facts), or show "Select an op"?
   - **Recommendation:** browse. "What does this model hold?" is a real
     question before the first op.

### 7.3 Proposed packages (superseded by R2.10)

| WP | Content | Needs | Waits for |
|---|---|---|---|
| GT-0 | UX-7 key requirement: BREP ring, edge, circle and point keys carry Z or a topology id | core | nothing (UX Q3 is ruled) |
| GT-1 | The item tree component (rows, groups by facts, state words, row-viewport sync, "2 items here" chooser) | GUI | GT-0 |
| GT-2 | The Geometry workspace (W touch points, pick bar gate lifted, browse colours) | GUI | GT-Q1, GT-Q2, GT-Q5 |
| GT-3 | The inspector summary line and "Edit geometry"; the `+ Rule` card goes | GUI | GT-Q3 |
| GT-4 | "Use as floor" on a face row (the Q4 reference) | GUI | UX-8 |
| GT-5 | MCP `set_ui_view workspace: "geometry"`, `focus_toolpath` | GUI (MCP host) | GT-2 |
| GT-6 | Named sets (A) | core + GUI + MCP | GT-Q4 |

Each package follows the PLAN.md per-item flow: an editor, a sentry with a
red proof, the lead review, then the operator look on a release GUI built
from HEAD.
