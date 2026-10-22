# A Geometry tab: design study (2026-10-22)

Owner: the lead session. Reader: the operator. Status: planning only. No code
changes come from this file until the operator rules the questions GT-Q1 to
GT-Q6 (section 7.2). A plain "Q1" to "Q4" in this file is a UX_REVIEW.md
question, ruled on 2026-10-22.

Mockup: `geom_tab/mockup.html` (open it in a browser; no build step). It
shows the three concepts on one small stepped block.

File references use two short roots: **V** = `crates/rs_cam_viz/src/`,
**C** = `crates/rs_cam_core/src/`. Line numbers are at HEAD 669a3b56.

---

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
| NEW | "What does this model hold?" | Today the operator learns the items only through an op. The tab lists them with no op. |

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
- **Placement.** Two placements exist:
  - **(W) A fifth workspace.** Touch points: `Workspace::ALL` is
    `[Workspace; 4]` and `order()` is exhaustive (`V/state/mod.rs:31-71`,
    the G-WSMENU sentry); the layout dispatch `V/app.rs:1100`; the shortcut
    dispatch `V/app.rs:1065`; the MCP workspace key `V/app.rs:392`; the badge
    arm in `V/ui/workspace_bar.rs`. GUI only, six places.
  - **(M) A mode inside Toolpaths.** The pick bar draws only in Toolpaths
    (`V/ui/pick_bar.rs`, `draw`: `state.workspace != Workspace::Toolpaths`
    returns). A mode keeps that gate. A workspace must lift it or host its
    own bar.
- **Layout today.** Setup and Toolpaths both draw a left rail of 240 px, a
  right inspector of 280 px and the viewport between (`V/app.rs:510-651`).

---

## 3. Concept A: an item browser with named geometry sets

### 3.1 Idea

The Geometry workspace lists the items of each model in a tree. The operator
picks items (in the tree or the viewport) and saves them as a **named set**:
"Outline", "Step floor", "Ø6 holes". An op then names ONE set in its Geometry
row, or "New from the current pick". One set can feed many ops.

### 3.2 Layout (W: a fifth workspace)

```
 Setup | Geometry | Toolpaths | Simulation | Readiness
┌ ITEMS ──────────────┬──────────── viewport ───────────┬ SET ─────────────────┐
│ stepped_block.step  │                                 │ Ø6 holes        [✎]  │
│ ▾ Faces       4     │   ┌──────────┬─────────────┐    │ 2 circles · Z 20     │
│   Z 32 top    ●     │   │  ┌────┐  │   ○      ◯  │    │ Rule: circles Ø6.00  │
│   Z 24 floor        │   │  └────┘  │   ○         │    │ Used by: Drill 1     │
│   Z 20 step   ●     │   └──────────┴─────────────┘    │ [Show me] [Delete]   │
│ ▸ Face rings  5     │                                 │──────────────────────│
│ ▸ Edges       8     │  [pick bar: kinds · Done]       │ SETS                 │
│ ▸ Circles     3     │                                 │ Outline      Profile │
│ ▸ Points      8     │                                 │ Step floor   Pocket  │
│ CURRENT PICK  2     │                                 │ Ø6 holes     Drill 1 │
│ [Save as set…]      │                                 │ + New set            │
└─────────────────────┴─────────────────────────────────┴──────────────────────┘
 Toolpaths inspector, Geometry row:  Geometry  [ Ø6 holes ▾ ]  2 circles · Cut
```

### 3.3 Journeys

- **DXF sign.** Open Geometry. The tree shows the layers CUT, TEXT, BORDER.
  Click the layer row TEXT: the current pick is "layer TEXT, every served
  kind" (a rule). Save as "Letters". Add VCarve; its Geometry row names
  "Letters". Add Profile; name "Border". The default rule never shows,
  because an op that names a set holds no default.
- **STEP part (after UX-7).** Open the Faces group. Click "Z 20 step": the
  face lights. Save as "Step floor". Open Circles: "Ø 6.00 × 2" is a group
  row; click it and save "Ø6 holes". Open Edges; the row "Step edge · Z 20"
  and the row "Step edge · Z 32" are two rows at one XY (section 1.3). Pick
  the Z 32 row; save "Step rim" for a Chamfer.
- **Two-faced Profile.** Save the faces Z 32 and Z 20 as "Two tops". A
  Profile names it. Q2 sets "Each picked face", depth 0 (one undo step), so
  each face cuts to its own plane. To cut round the top face down to the step
  face: name the set "Top face", and on the floor row choose "A picked face"
  (Q4). The floor row lists the faces of the project; the operator chooses
  "Z 20 step".

### 3.4 Core model

Two forms; the operator chooses (GT-Q4, section 7.2):

- **A-ref (by reference).** NEW `GeometrySet { id: SetId, name, selection:
  Selection }` in the project, and NEW `sets: Vec<SetId>` in `Selection`
  (a union with the rules and picks). `resolve` expands a set. NEW commands
  `AddGeometrySet`, `EditGeometrySet(SetId, SelectionEdit)`, `RenameGeometrySet`,
  `RemoveGeometrySet` (refuses while an op names the set, and names the ops).
  `ToolpathConfig::models_read` reads the set models; an edit of a set stales
  every op that names it (`invalidate_result_chain` per op). File:
  `[[geometry_sets]]`. Edit once, every op follows (S27 bulk edit).
- **A-copy (by value).** The sets live in the project, but "use a set"
  writes `SelectionEdit::Replace(set.selection)` into the op. No resolver
  change. The op does not follow a later set edit. Storage is still core
  (GUI state is not a data model, viz CLAUDE.md).

A-ref adds state outside the op config. R1 says "one Selection type per op";
A-ref keeps one type, but the op no longer holds all its own geometry. That
is a real break with the P2 design (DESIGN.md §3.1).

**The click question.** With a set bound to an op, what does a viewport
click on that op do? (a) It edits the set, so every op that names it moves.
(b) It detaches the op to its own copy. Question GT-Q4 (section 7.2) asks.

**Item keys and re-import.** A set holds rules and picks as the op does. A
rule follows the file (S17); a pick that misses is a lost pick of the SET,
shown once, on every op that names it.

**Templates (P7).** `TemplateSelection` refuses a pick and a `Model(id)`
source (P7_DESIGN.md §2.2). So a set of rules only can travel in a template;
a template names it the way it names a layer, and the draft maps it (the
layer map, §3.3). A set with picks binds only inside its own project.

### 3.5 Risks

- Two places hold geometry: the set and the op. The operator must know which
  one a click edits.
- A delete or rename of a set touches many ops; the refusal text must name
  them.
- Browse colours with no op selected are NEW GUI state (a browse pick per
  model, a hover). viz CLAUDE.md asks for an audit of the setup sheet, project
  IO, controller fixtures and MCP/UI initialisers for each new field.

---

## 4. Concept B: the Geometry tab edits one op

### 4.1 Idea

The Geometry tab is the large editor of the selected op's selection. The
inspector keeps one summary line and an "Edit geometry" button. The tab shows
the op's item states (Cut, Island, Skipped, Excluded, Not selected) in the
tree and the viewport at once. No new core state.

### 4.2 Layout (M: a mode of Toolpaths, or W with the op list on the left)

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

- The tab means nothing with no op selected. It then shows "Select an op",
  or it browses the items with neutral colours (the A browse state, without
  sets).
- Two doors edit one selection (the tab and the inspector Pick). Question GT-Q3 (section 7.2)
  asks whether Pick leaves the inspector.

---

## 5. Concept C: a facts browser; recognition stays in a wizard

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

## 6. The concepts side by side

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

### 7.1 Recommendation

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

### 7.2 Open questions for the operator

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

### 7.3 Proposed packages (after the rulings)

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
