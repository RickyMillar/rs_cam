# Geometry selection P4: design (2026-10-19)

Status: DRAFT for the lead. Author: the P4 design agent (read-only).
Inputs: `PLAN.md` (P4, items 4 and 4b, the merge gate, every ruling and
status note), `REQUIREMENTS.md` (R6, S1–S3, S9, S13, S17–S19, S22, S24),
`GAP_ANALYSIS.md` (with the 2026-10-19 open-vector addendum), `DESIGN.md`
(5.4, 6, 7, 10.2–10.5, 15), the commits bba80b4d..16c1b78b, and the code on
`geo-select` at 8e768f4c.

Conventions:

- **NEW** marks a name that does not exist in the code today.
- Core paths start at `crates/rs_cam_core/src/`; viz paths start at
  `crates/rs_cam_viz/src/`.
- **[OP]** marks a decision that changes what is machined. The operator
  rules it. **[LEAD]** marks a UI or structure decision. The lead rules it.
- Each item is one editor session. Each item names its files, the core
  commands and queries (NEW only where missing), its sentries, its MCP
  test and what the operator looks at.

---

## 0. Summary

P4 has ten items. Items 1–3 close the merge gate. Item 4 is the one core
change that most viewport work reads. Items 5–9 add the pick, tab and
import tools. Item 10 closes the docs.

| # | Item | Gate | Visible result |
|---|---|---|---|
| P4.1 | 3D op face pick through the boundary selection; `SelectionTarget` | merge gate | a face click on a 3D op works again |
| P4.2 | Face op onto `Items` (D5) | merge gate | a face click on a Face op works again |
| P4.3 | The Selection section (op and boundary editor) | merge gate | rules, picks, counts, notes, lost picks in the panel |
| P4.4 | Per-item states in `SelectionReport`; viewport highlight | | cut, island, skipped, lost in colour; open paths marked |
| P4.5 | Pick mode, filter bar, hover, shift-click layer | | pick by kind, layer and diameter in the viewport |
| P4.6 | Hand-tab viewport editor | | add, drag and delete tabs on the loop |
| P4.7 | STL flat-region faces | | face pick on an STL |
| P4.8 | Import options: SVG scale, join tolerance, import report | | an import dialog; the report in the model panel |
| P4.9 | Join and close tool (a per-model edit ledger) | | join, close with a line, move ends together |
| P4.10 | Docs, catalog, registry closure | | none |

The merge gate closes after P4.3 and an operator look. P4.3 includes the
"Selection" boundary source button for STEP-only projects; without it a
3D op on a STEP part cannot reach a face pick. The lead can then
merge `geo-select` and run P4.4–P4.10 on a new branch, or keep one
branch. **[LEAD]**

The other merge-gate point is closed: P2.7 (89df3283) fixed
`production_writes_go_through_apply_wp15a` (the CLI `SetDressupField`
row). The lead confirms it with the sentry before the merge.

---

## 1. What exists (the base of every item)

| Piece | Where | P4 reads it for |
|---|---|---|
| `SelectionReport` (`counts`, `islands`, `skipped_by_kind`, `notes`, `lost`, `refusal`, `hole_items`, `holes`, `selected`) | `session/selection_status.rs:104` | counts line, notes, lost picks, refusal |
| `selection_status` memo (key: selection, op type, `ItemUse`, model item sets by `Weak`, setup, stock) | `session/selection_status.rs:227` | every frame read |
| `pick_edit(tc, key)` → `AddPick` or `RemovePick` | `session/selection_status.rs:450` | viewport click |
| `face_pick_key(tc, model, FaceGroupId)` | `session/selection_status.rs:408` | STEP face click |
| `ListItems` / `list_model_items` | `session/command.rs:548`, `:1710` | layer list, MCP picks |
| `EditSelection` / `edit_toolpath_selection` | `session/command.rs:595`, `:1785` | every write |
| `ItemSet::counts`, `layers`, `drawing_curves` | `selection/item.rs:503`, `:520`, `:486` | model panel, layer list |
| `AppEvent::TogglePick`, `AppEvent::EditToolpathSelection` | `ui/mod.rs:345`, `:351` | the GUI write path |
| `PickHit::Item`, `pick_drill_targets`, `hole_display_point` | `interaction/picking.rs:41`, `:313`, `:290` | the one draw-and-pick point |
| Drill hole selector | `ui/properties/operations/drill.rs:39` | becomes part of the section |
| Face picker block ("Face Selection", Clear) | `ui/properties/tab_badges.rs:513` | becomes part of the section |
| Boundary "Model Outline" block | `ui/properties/toolpath_panel.rs:660–790` | becomes the boundary editor |
| Polygon upload, drill markers | `app/gpu_upload.rs:346–590` | highlight colours |
| `last_hover_face` upload-on-change | `app/viewport.rs:262–330` | hover pattern |
| Overlay registry (`curves`, `boundary_outline` with no renderer) | `ui/overlays/registry.rs:717`, `:1072` | new rows |
| Hand tabs `tab_points`, `resolve_hand_tabs`, `nearest_on_loop`, `hand_tab_reach` | `compute/operation_configs.rs:607`, `compute/execute/clearing_2d.rs:451`, `dressup/tabs.rs:583`, `:427` | tab editor |
| DXF chain kernel `chain_open_pieces`, `CLOSE_TOL_MM` | `io/dxf_input.rs:647`, `:589` | join tolerance |
| `geom_cache` (`cached_silhouette`, `Weak` identity) | `maps/geom_cache.rs:338` | STL flat regions |

Defects found on the way (fix inside the named item):

- The drill marker colours are literals in `app/gpu_upload.rs:563–566`.
  `render/CLAUDE.md` says every colour is in `render/colors.rs`. Fix in P4.4.
- The GUI imports an SVG at scale 1.0 (`controller/io.rs:112`). The SVG
  door says its coordinates are CSS px (`io/svg_input.rs:7–15`). So a
  drawing in mm can import 96/25.4 = 3.78 times too large. P4.8 verifies
  it with a probe first. **[OP]** decision O-2.
- Two writers for one field: `drill.rs` writes the draft `cfg.selection`;
  `tab_badges.rs` sends `EditToolpathSelection`. P4.3 removes the draft
  writer (decision L-1).

---

## 2. The items

### P4.1 — Merge gate A: a 3D op takes face picks through its boundary

**Goal.** A face click on a 3D op works again. The click writes a pick
into the boundary selection. The auto top follows those faces. DESIGN 6
(approved) says a 3D op uses a selection only through its boundary.

**Core (NEW unless stated).**

```rust
// selection/mod.rs (or session/command.rs): which selection of a toolpath
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelectionTarget {
    #[default]
    Op,        // OperationConfig::selection_slot
    Boundary,  // ToolpathConfig.boundary.source = BoundarySource::Selection { selection, .. }
}

pub struct EditSelectionArgs   { pub index: usize, pub target: SelectionTarget, pub edit: SelectionEdit }  // + target
pub struct SelectionStatusArgs { pub index: usize, pub target: SelectionTarget }                          // + target
impl ProjectSession {
    pub fn selection_status(&self, tc: &ToolpathConfig, target: SelectionTarget) -> Result<Arc<SelectionReport>, SessionError>; // + target
    pub fn pick_edit(&self, tc: &ToolpathConfig, target: SelectionTarget, key: MatchKey) -> Result<SelectionEdit, SessionError>;  // + target
}
```

- `edit_toolpath_selection` with `target: Boundary` edits the selection
  inside `BoundarySource::Selection`. When the source is not `Selection`,
  the command refuses with a NEW `SessionError` text: "the boundary of
  '<name>' is not a selection: set the boundary source to Selection".
  The command never changes the boundary source.
- The status memo key gains the target and, for `Boundary`, the boundary
  selection and the `holes` flag. The boundary report runs `view` with
  `ItemUse::Regions` (the boundary resolution reads `Regions`,
  `generation.rs:544`). One function builds both reports, so generation
  and status cannot differ.
- The auto top: `generation.rs:183` reads `selection.face_top_z()` of the
  op selection. For an op with no selection slot, it reads the
  `face_top_z` of the boundary selection, after the frame door, when the
  boundary is enabled. The rule "an AUTO top follows, a PINNED top stays"
  does not change. The top is in the setup frame (P2.3b fix).
- `resolve_boundary_selection_polys` and the status share one resolution
  of the boundary selection (`resolve` → `to_setup_frame` → `view`), as
  `resolve_selection_frames` does for the op.

**Viz.**

- `AppEvent::TogglePick { toolpath_id, target, key }` and
  `AppEvent::EditToolpathSelection { toolpath_id, target, edit }` gain the
  target (`ui/mod.rs:345`, `controller/events/mod.rs:320`).
- `app/viewport.rs:211`: a `ModelFace` hit on a toolpath whose op has no
  selection slot sends `target: Boundary`. When the boundary is not a
  `Selection`, the click gives a notice and no edit (decision L-8).
- `app/gpu_upload.rs:30` `selected_face_ids` reads the report of the
  target that holds the faces.

**MCP.** `EditToolpathSelectionParam` and the `selection_status` param
gain `target` (`"op"` default, `"boundary"`) in `rs_cam_mcp/src/server.rs`.
The wire JSON of `SelectionTarget` is the file spelling.

**Tests.**

- Core sentry NEW `a_3d_op_face_pick_sets_its_top_p4_1`: a STEP model, a
  3D op with a `Selection` boundary of one face pick; the top equals the
  face plane Z in a Top setup and in a Bottom (flipped) setup; a pinned
  top stays. Red proof: read the op selection only.
- Core: `selection_refusals_name_the_item_p2_4` gains a boundary arm
  (generation refusal = status refusal).
- Viz controller test in `controller/tests/selection.rs`: a face click on
  a 3D op adds one boundary pick; undo removes it (D11 snapshot).
- MCP: extend `mcp_core_and_gui_give_one_selection_status_p2_7`
  (`app/mcp/commands/selection.rs`) with `target = "boundary"`.

**Operator looks at.** A STEP part, a 3D rough with the boundary set to a
picked face. The top line in the Heights tab follows the face. A flipped
setup gives the face Z of that setup.

**Decision O-1 [OP].** Before P2.6, a face pick on a 3D op set the top
with any boundary source. With this item, the face pick also bounds the
cut to the face. Options:

- (A, recommended for the merge gate) Face picks live in the boundary
  selection. The top follows them. One act gives both effects. It follows
  DESIGN 6 and needs no new height type.
- (B) An explicit top reference `HeightMode::FromFace { key, offset }`
  (NEW), set from the Heights tab, separate from the boundary. It follows
  R4 ("every depth names its reference") and S5. It costs a non-`Copy`
  `HeightMode` and a face lookup in `SetupEvalContext::height_context`.
  P3 can add it later for every op.

Old `face_selection` picks do not convert (retired, P2.6), so no saved
project changes output. The question is what the new click means.

---

### P4.2 — Merge gate B: the Face op reads items (D5)

**Goal.** The Face op holds a selection with the default
`Selection::stock()`. The default gives today's output. A face click on a
Face op adds a face pick, and the auto top follows it (the 2.5D rule).

**Core.**

- `FaceConfig.selection: Selection` with
  `#[serde(default = "Selection::stock", skip_serializing_if = "Selection::is_stock")]`
  and `driver_set = DRIVER_SET_SELECTION` (`compute/operation_configs.rs:214`).
- The exhaustive `OperationConfig::{selection, selection_slot,
  default_selection}` arms for Face (`compute/catalog.rs`).
- `REG_FACE` (`compute/catalog/registry.rs:38`): `item_use: Regions`,
  `GeometryRequirement::Items`. W4 check 9 (slot iff `item_use != None`)
  then holds.
- The Face adapter (`compute/execute/clearing_2d.rs:727`): it faces each
  region of `prepared.regions()`. The `Stock` item gives
  `face_rectangle(stock_bbox, stock_offset)` byte for byte. NEW: the
  region-offset rule for a region that is not the stock (decision O-3).
- The "is stock based" rule the P2 status note names: an op whose
  default selection is `stock()` needs no drawing. A Face op on an STL or
  on no model must still generate. The prologue must not refuse
  `MissingSource` for the toolpath model when the selection reads only
  `SourceRef::Stock`. The editor lists every reader of
  `GeometryRequirement::Stock` first (`rg`).

**Viz.** Nothing new. The face picker (`tab_badges.rs:513`) shows for
any op with a selection slot, so the Face op gets it. P4.3 replaces it.

**MCP.** None new. `get_toolpath_params` returns the Face selection.
`edit_toolpath_selection` reaches Face.

**Tests.**

- G-OPGOLDEN unchanged for every Face case (proof of D5). Red proof: grow
  the stock region by one extra `stock_offset`.
- Core NEW `a_face_op_reads_its_selection_p4_2`: the default faces the
  stock rectangle; a face pick on a STEP model sets the auto top to the
  face Z in the setup frame; a Face op on an STL with the default
  generates.
- `an_absent_selection_key_loads_the_op_default_p2_3` gains the Face arm
  (no key loads `stock()`).
- W4 `every_op_conforms_w4` check 9 with the Face row changed.
- Viz controller: a face click on a Face op adds one pick; undo.

**Operator looks at.** A Face op on a STEP part, a picked top face. The
facing stops at the face Z.

**Decision O-3 [OP].** `stock_offset` on a region that is not the stock.

- (a, recommended) `stock_offset` grows the `Stock` item only. Any other
  region is faced inside its outline (the tool stays inside, as a
  pocket). A face region grown by `stock_offset` would cut into the walls
  round the face.
- (b) Grow every region by `stock_offset`.

**Decision L-6 [LEAD].** A face click on a Face op under the stock
default adds a pick, so the stock and the face are both regions and the
face area is cut twice at one Z (air, no gouge). Options: keep the edit
additive and show a caution in the section ("the stock region holds the
face region"), or make the first pick replace the stock rule (the drill
"first layer pick" rule, `drill.rs:96`). Recommended: the first pick
replaces, as the drill does, so one act gives one region.

---

### P4.3 — Merge gate C: the Selection section (op and boundary)

**Goal.** One panel section shows and edits a selection. It serves the op
selection of every op with a slot and the boundary selection of every op.
It replaces three surfaces: the drill hole selector (its generic parts),
the face picker block, and the "Model Outline" boundary block.

**Panel (idle, op target).** Tokens and components are the ones in
`ui/tokens.rs` and `ui/components/`.

```
 SELECTION                                         [ Pick ]   SectionHeader; Button (ACCENT when on)
 ───────────────────────────────────────────────────────────  HAIRLINE
 Cuts     14 loops  3 islands  2 circles             ✓ OK     CountPill x3 (TEXT_BODY); StatusChip Role::Ok
 Rules    Every drawing item  (default)                        DataTable row, TEXT_MUTED "(default)"
          Layer CUT · closed loops                      ✕     DataTable row; ✕ = RemoveRule(i)
          [ + Rule ▾ ]   [ Reset ]                             Button; Reset disabled at the default
 Picks    2 added · 1 removed                    [ Clear ]    CountPill; Clear = Replace(rules only)
 ▸ Details                                                     Details: skipped by kind, sources, keys
```

**Panel (a note and a lost pick).**

```
 SELECTION                                         [ Pick ]
 Cuts     12 loops  2 islands                        ! 1      CountPill; StatusChip Role::Caution
 ┌─ ! 2 open paths on layer TEXT are not cut. ──[ Show me ]┐  Banner (CAUTION on TINT_CAUTION)
 └─────────────────────────────────────────────────────────┘
 ✕ 1 pick is lost: circle on layer PINS (X 12.000 Y 40.000,   DataRow::note, DANGER
   diameter 5.000 mm). It matched no item after re-import.
                                     [ Remove ]  [ Show me ]
```

**Panel (refused).**

```
 SELECTION                                         [ Pick ]
 Cuts     nothing                                    ✕ Refused StatusChip Role::Danger
 Pocket: nothing to cut. 3 open paths on layer TEXT and       OneLine, DANGER (wraps: blocking error, IC1)
 1 point are not cut. Use a rule with closed loops, close
 the paths, or use Trace.                          [ Reset ]
```

**Panel (boundary target).** The same component, under "Machining
Boundary", when the source is `Selection`. It keeps the "Machine the
holes" checkbox (`toolpath_panel.rs:784`) and adds a model combo for
`+ Rule`.

The source button: today the "Model Outline" button is the only door to
a `Selection` boundary, and it is disabled when no model has a closed 2D
shape (`toolpath_panel.rs:660`). That is every STEP-only project, so the
lost flow (a STEP part, a 3D rough bounded by a picked face) stays
unreachable. P4.3 renames the button "Selection". It is enabled when any
model has a drawing item OR a face item. With a drawing model it writes
`Selection::closed_shapes_of(first model)` as today. With only face
models it writes `Selection::empty()`, which refuses `Empty` (visible in
the section) until the first face pick. The merge gate needs this door.

`SelectionEdit::Reset` on `target: Boundary` has no op default. It
resets to the value the source button writes (above). The editor
passes that value as the `op_default` of `Selection::apply`.

**The `+ Rule` popover.**

```
 ┌ Add a rule ───────────────────────────────┐
 │ Source   [ This model ▾ ]                  │  ChoiceRow: ToolpathModel | Model(id) | Stock
 │ Layers   [x] CUT  [ ] TEXT  [ ] PINS       │  from ListItems / ItemSet::layers()
 │ Kinds    [x] loops [ ] open [x] circles    │  only the kinds the op serves are enabled
 │          [ ] points [ ] faces              │
 │ Circle Ø [  5.00 ] ± [ 0.10 ] mm           │  DiameterRange (S2); shown when circles on
 │ Faces    [ Up ▾ ]  Z [     ] to [     ]    │  FaceFilter; shown when faces on
 │                      [ Cancel ] [ Add ]    │
 └────────────────────────────────────────────┘
```

**Files.**

- NEW `ui/properties/selection_section.rs`: `draw_selection_section(ui,
  SelectionSectionInputs { toolpath_id, target, selection, op_default,
  report: Option<&SelectionReport>, layers, models, item_use }, events)`.
  It reads the report. It sends `AppEvent::EditToolpathSelection` only. It
  never writes `entry.operation` (decision L-1).
- `ui/properties/operations/drill.rs`: `draw_hole_selector` keeps only
  the drill legend and the pin-drill line. The count, Select all, Clear,
  layer combo and refusal go to the section.
- `ui/properties/tab_badges.rs:513`: the face picker block goes.
- `ui/properties/toolpath_panel.rs:738–790`: the outline model combo goes;
  the section draws the boundary selection.
- `ui/properties/mod.rs`: `ToolpathPanelInputs` gains
  `selection_report: Option<Arc<SelectionReport>>` and
  `boundary_report: Option<Arc<SelectionReport>>` (properties invariant:
  add a field there, not a parameter). `holes` stays for the drill legend.
- `state/` NEW `pick.rs` holds `PickMode` (P4.5 fills it; P4.3 only
  stores the on/off and the target).
- `ui/properties/CLAUDE.md`: one file-map line and one invariant ("the
  selection section writes through `EditToolpathSelection` only").

**Core.** None new. `session/command.rs:540`: the GUI surface of
`SelectionStatus` becomes `Reach::Reached`. `ListItems` (`:548`) becomes
`Reached` on the GUI only if the section reads its layers through the
query. If it reads `ItemSet::layers()` directly, the row keeps `Skip`
with that reason. Recommended: the query, so the GUI and MCP list one
answer.

**MCP.** None new. The section reads the same report as
`selection_status`.

**Tests.**

- Viz NEW `the_selection_section_writes_edits_only_p4_3`: each control
  sends one `EditToolpathSelection` (AddRule, RemoveRule, Reset,
  Replace); the draft `entry.operation` does not change in the draw. Red
  proof: put back the draft write in `drill.rs`.
- Viz: `egui_draw_sites_write_through_commands_wp6` and
  `the_inspector_nests_once_dc5`, `the_inspector_text_is_bounded_ic1`,
  `tabular_data_reads_one_table_ic2`, `panels_read_the_token_module_up1`
  stay green.
- Viz: `boundary_controls_always_visible_g_boundaryinherit` and
  `a_setup_map_op_draws_no_boundary_controls_d3` stay green.
- Viz controller tests in `controller/tests/selection.rs`: each edit
  undoes in one step (op and boundary).
- MCP parity: the counts the section prints equal `selection_status`
  (extend the P2.7 parity test with a rule edit).

**Operator looks at.** A Pocket on a DXF with text strokes: the counts,
the open-path note, a layer rule, Reset. A Drill: Select all and Clear
now live in the section. A 3D op: the boundary editor with face picks.
Then the merge.

**Decisions [LEAD].**

- L-1 The section sends edits only. The drill draft writer goes.
- L-2 The refusal has one owner (IC1). Recommended: the section owns the
  refusal text and the fix buttons, because the fix is there. The footer
  row (`precondition.selection_refused`) shows one line that names the
  Geometry tab. The editor checks the footer text first.
- L-3 The pick target, when an op has an op selection and a `Selection`
  boundary: the section whose `Pick` button is on. One `Pick` at a time;
  default `Op`.

---

### P4.4 — Per-item states and the viewport highlight

**Goal.** The viewport shows what the selection does to each item: cut,
island, skipped, not selected, excluded, lost. Open paths have their own
colour and start and end marks (addendum 2026-10-19). "Show me" frames
the items of a note or a lost pick.

**Core (NEW).** The one core addition that P4.4–P4.6 read.

```rust
// session/selection_status.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    Cut,          // a served item the op cuts (an outer region, a curve, a hole)
    Island,       // a served closed ring at odd containment depth (an inner ring)
    Skipped,      // matched by the default rule; the op cannot cut its kind
    Excluded,     // matched by a rule; an exclude removes it
    NotSelected,  // no rule or pick matches it
}
pub struct SelectionReport {
    // ... the fields of today ...
    /// One state per item of the toolpath model, index-aligned with
    /// `LoadedModel::items` (the memo key holds that set by `Weak`
    /// identity, so the alignment cannot go stale).
    pub item_states: Vec<ItemState>,
    /// The selected items that are not in `LoadedModel::items` of the
    /// toolpath model: items of OTHER sources (`Model(other)`, `Stock`)
    /// and every face of a lazy face set (an STL flat region, P4.7).
    /// The key and the state. The viewport finds the geometry by key.
    pub foreign: Vec<(MatchKey, ItemState)>,
}
```

The index alignment covers `LoadedModel::items` only. A face from the
lazy STL set (P4.7) always reports in `foreign`, by key. The P4.4
sentry asserts this, so P4.7 cannot break it.

- `ResolvedItem` gains its index in its source set (NEW field). `resolve`
  knows it when it walks the set.
- `view.rs`: NEW `nesting_depths(served) -> Vec<usize>`, called by
  `regions_of`. The report reads the same depths. One nesting answer, so
  "island" in the viewport is the island the kernel cuts.
- A refused report keeps the states it can give (resolution states) and
  marks no item `Cut`.
- No new memo-key input: the states read the same inputs as the report.

**Viewport colours (NEW constants, `render/colors.rs`).** The line
pipeline has no alpha and no dash (`colors.rs`, obstacles note), so a
faint state is a darker colour. Each value is the token colour of
`ui/tokens.rs` divided by 255, so the panel chips and the lines match.

| State | Constant (NEW) | Token | Line |
|---|---|---|---|
| Cut | `SELECTION_CUT` | `OK` 0x5F 0xBF 0x7A | solid |
| Island | `SELECTION_ISLAND` | `ACCENT` 0x5B 0x9D 0xD9 | solid |
| Skipped | `SELECTION_SKIPPED` | `CAUTION` 0xE0 0xA8 0x3C | solid |
| Excluded | `SELECTION_EXCLUDED` | `INK_35` 0x45 0x4D 0x56 | solid, with an X at the anchor |
| Not selected | `SELECTION_IDLE` | `INK_50` 0x73 0x7C 0x87 | solid |
| Lost pick (ghost) | `SELECTION_LOST` | `DANGER` 0xE8 0x7B 0x77 | ring of diameter `measure` at `anchor` (loop, face, circle); cross at `anchor` (point, path) |
| Open path, no op context | `OPEN_PATH` | NEW token `PATH_OPEN` (magenta, Carbide Create pattern) | solid |
| Path start / end | `PATH_START`, `PATH_END` | `OK` / `DANGER` | a filled triangle at the start pointing along the path; a square at the end |
| Hover | `SELECTION_HOVER` | `TEXT_STRONG` 0xE2 0xE7 0xEC | solid, drawn last |
| Drill marker selected / free | `HOLE_SELECTED`, `HOLE_FREE` | move today's literals | ring |

With a selected toolpath, the colour gives the state, and an open path
keeps its start and end marks (its kind). With no selected toolpath (the
`curves` overlay, a model selected), the drawing keeps `POLYGON_OUTLINE`
and open paths draw in `OPEN_PATH` with their marks.

**Viewport (highlight, Toolpaths workspace, a Pocket selected).**

```
   ┌────────────────────────────── stock ──────────────────────────────┐
   │   ┏━━━━━━━━━━━━━━━━━━━━━━┓         ╭───╮                           │
   │   ┃  ┌─────┐   ┌─────┐  ┃         │ ○ │ ← Lost ghost (DANGER):     │
   │   ┃  │isl. │   │isl. │  ┃         ╰───╯   ring at the pick anchor  │
   │   ┃  └─────┘   └─────┘  ┃  ← Cut (OK) outer, Island (ACCENT) inner  │
   │   ┗━━━━━━━━━━━━━━━━━━━━━━┛                                         │
   │   ▶~~~~~~~~~~~~~~~~~~~~~~■  ← Skipped open path (CAUTION), ▶ start, ■ end
   │   ┌ ─ ─ ─ ┐  ← Not selected (INK_50)                               │
   └────────────────────────────────────────────────────────────────────┘
                                         legend: ■ cut ■ island ■ skipped ■ lost ■ idle
```

**Files.**

- `app/gpu_upload.rs`: the polygon upload reads `item_states` and
  `foreign` and colours each ring by state. The ring transform becomes
  one function shared with the pick: NEW
  `interaction::picking::item_display_ring(frame, stock, ring, z)` (the
  P2.6 rule: the draw and the pick read one point function). The drill
  marker literals move to `render/colors.rs`.
- `app/gpu_upload.rs`: start and end marks per `Path` (and per drawn
  direction of a `Path` only; a loop has no start in P4).
- `ui/overlays/registry.rs`: NEW rows `selection_states` (group
  Geometry, `UploadTime(UPLOAD_DETECTOR)`, default on in Toolpaths) and
  `open_path_ends` (default on). The `boundary_outline` row gets a
  renderer: the boundary regions of the selected toolpath, in the NEW colour
  `BOUNDARY_OUTLINE`. The precondition names why a
  row cannot draw ("the selected toolpath reads no drawing").
- `ui/overlays/legend_rail.rs`: the chips for the states; the mirrored
  render colours (`legend_rail::mirrored`) and the vpstate sentry pin the
  literals.
- `app.rs`: the composite `overlay_upload_key` detector gains the report
  `Arc` pointer of the selected toolpath, so a selection edit uploads once.
- "Show me" (P4.3 button): NEW `ViewportState` request
  `frame_items: Vec<MatchKey>`. The camera fits the union bbox of those
  items in the display frame and the hover colour flashes them for
  `MOTION_SLOW`. For a note, the keys are the `Skipped` items of its kind
  and layer (from `item_states`).

**MCP.** `selection_status` returns `item_states` as a list of
`{key, state}` for the toolpath model and `foreign` (keys, so an agent
can read the same answer). `set_ui_view` gains `frame_items: [key]`
(NEW, optional) so an agent can take the "Show me" screenshot.

**Tests.**

- Core NEW `item_states_match_the_cut_p4_4`: on every G-OPGOLDEN drawing
  and on a drawing with text strokes, the `Cut` and `Island` rings equal
  the rings of `prepared.regions()` (outer and holes). Red proof: compute
  islands by even area order, not by `nesting_depths`.
- Core: `selection_status_is_memoised_p2_4` still holds (no new key).
- Viz: `overlays_registry`, `the_dock_states_read_live_state_g_vpstate`,
  `the_viewport_dock_reaches_every_option_g_vpdock`,
  `render_pipelines_headless_g_pipesmoke`, `viewport_draws_selected_only_wp27`.
- Viz NEW `the_selection_colours_live_in_colors_p4_4`: no `[f32; 3]`
  literal in `app/gpu_upload.rs` (a source scan with a non-vacuity anchor).
- MCP: the parity test compares `item_states` of GUI and MCP.

**Operator looks at.** The text-stroke DXF: the open paths in amber with
their ends; the islands in blue; a re-import that loses a pick shows a
red ring. "Show me" on the note.

**Decisions [LEAD].**

- L-9 Scope: the highlight draws for the selected toolpath only (WP27).
  Items of another model in its selection draw in that model's display
  frame. The stock item draws as a `SELECTION_CUT` stock outline.
- L-10 The colour table above, and the new token `PATH_OPEN`.

---

### P4.5 — Pick mode, filter bar, hover, shift-click layer

**Goal.** The user picks items by kind in the viewport. A filter bar
sets the kinds and the circle diameter. Shift-click adds the clicked
item's layer as a rule. The item under the pointer highlights.

**Viewport (pick mode on, a Pocket, filter loops and circles).**

```
          ┌──────────────────────────────────────────────────────────┐
          │ Pick for Pocket 1 ▸ Selection                             │  floating Area, SURFACE_OVERLAY,
          │ [■ Loops] [□ Open] [■ Circles Ø 5.00 ± 0.10] [□ Faces]    │  SHADOW_OVERLAY, RADIUS_MD
          │ [□ Points]       Click: add/remove · Shift: whole layer   │  chips: ACCENT on, INK_35 off
          │                                       [ Done  Esc ]       │  disabled chip hover: the reason
          └──────────────────────────────────────────────────────────┘
   ┏━━━━━━━━━━━━━━━┓
   ┃  hover ring   ┃ ← SELECTION_HOVER over the item under the pointer
   ┗━━━━━━━━━━━━━━━┛   tooltip: "closed loop on layer CUT, 42.000 mm across — click to remove"
```

**Rules.**

- A chip for a kind the op cannot serve is disabled, and its hover names
  the reason (`use_phrase`, `selection/refusal.rs`). So the GUI cannot
  make a `KindNotCut` pick. MCP still can, and refuses by name.
- A click on an item sends `TogglePick { target, key }`
  (`pick_edit` decides add or remove; today's rule).
- Shift-click sends `AddRule(Rule { source, filter: { layers: [layer],
  kinds: Only(on chips), diameter: circle range if on } })`. Under the
  default selection, the first layer rule replaces (`drill.rs:96` rule).
  An item with no layer gives a notice and no edit.
- Esc, `Done`, or a change of the selected toolpath ends pick mode.
- Pick mode wins over the toolpath pick and the stock pick; an orbit
  drag still works (a click is not a drag).

**Files.**

- `state/pick.rs` (from P4.3): `PickMode { toolpath_id, target, filter:
  PickFilter { kinds: BTreeSet<ItemKind>, circle: Option<DiameterRange> } }`
  on `AppState` (state invariant: a draft with content lives in state).
- `interaction/picking.rs`: NEW `pick_items(ctx, session, mode, frame)`:
  the nearest ring segment in screen space, inside
  `PICK_THRESHOLD_TOOLPATH`, over the toolpath-model items the filter
  admits (the `pick_toolpaths` pattern), through
  `item_display_ring`. `pick_drill_targets` becomes the `Point`/`Circle`
  arm of it. Faces: the ray pick (STEP via `face_pick_key`; STL via P4.7).
- `app/viewport.rs`: the filter bar (NEW `ui/pick_bar.rs`), the hover:
  NEW `last_hover_item: Option<MatchKey>`, set on change only, with the
  `last_hover_face` upload-on-change pattern. The hover ring uploads into
  its own small buffer (NEW `hover_lines`), so a pointer move never
  rebuilds the drawing buffer.
- `ui/CLAUDE.md`, `interaction` (no folder file today): add one line to
  `crates/rs_cam_viz/CLAUDE.md` for `interaction/`.

**Core (NEW).** `ListItemsArgs.near: Option<Near { xy: [f64; 2],
radius_mm: f64 }>`: the items whose ring passes within the radius, by
distance. It is the MCP form of a click.

**MCP.** `list_model_items { near }`. Pick mode and hover are GUI-only
interactions. Parity is on the result: the selection and its numbers.

**Tests.**

- Viz NEW `a_click_picks_the_drawn_loop_p4_5` (the P2.6 test pattern for
  loops, paths and circles, on a Top and a flipped setup): the click
  point from `item_display_ring` picks that item. Red proof: pick in the
  model frame.
- Viz NEW `a_served_kind_filter_blocks_a_refusing_pick_p4_5`: the Pocket
  bar has no enabled `Open` chip; shift-click on a layer with paths
  gives `Only([Loop, Circle])`.
- Core: `list_model_items` with `near` returns the item a click picks
  (same distance rule; a lib test in `selection/`).
- MCP: an MCP `near` pick and a GUI click give one stored pick
  (`one_item_is_one_pick_p2_7` pattern).

**Operator looks at.** Pick shelf-pin circles by Ø 5 ± 0.1 with one
shift-click; pick and unpick one loop; hover text.

**Decision L-4 [LEAD].** The filter bar floats at the top centre of the
viewport, over the 3D view, and takes no width (overlays invariant).

---

### P4.6 — Hand-tab viewport editor

**Goal.** S13. The user adds, drags and deletes tabs on the loop in the
viewport. The editor writes `tab_points` through `SetToolpathParam`.

**Core (NEW).** One tab placement for generation and for the editor.

```rust
// dressup/tabs.rs
pub struct PlannedTab { pub region: usize, pub at: P2 /* setup frame, on the cut ring */, pub source: TabSource }
pub enum TabSource { Auto, Hand(usize /* index in tab_points */) }
pub fn plan_profile_tabs(regions: &[Polygon2], cfg: &ProfileConfig, tool_radius: f64,
                         hand: &[HandTab]) -> Vec<PlannedTab>;   // ProfilePass::prepare calls it

// session: NEW Query row (Query, TabLayout, "tab_layout", TabLayoutArgs { index }, TabLayoutAnswer)
pub struct TabMark { pub at: [f64; 2] /* drawing frame */, pub loop_key: MatchKey, pub source: TabSource }
```

The query runs the selection view, `resolve_hand_tabs`, and
`plan_profile_tabs`, with no kernel. It refuses as generation refuses
(a far hand tab names its point).

**Viewport (tab edit mode, a Profile selected).**

```
   ┏━━━━━━━━━━━━━━━━━[▮]━━━━━━━━━━━━━━━━━━┓   [▮] auto tab (INK_65 bar across the loop)
   ┃                                       ┃   [▮] hand tab (ACCENT bar, a handle ring)
  [▮]                                     [▮]  drag a handle: the bar follows the snapped point
   ┃                                       ┃   click on the loop: a new hand tab there
   ┗━━━━━━━━━━[▮]━━━━━━━━━━━━[▮]━━━━━━━━━━━┛   right-click a hand tab: delete
   Tabs: 4 auto, 2 hand on 2 loops      [ Done ]  OneLine in the pick bar Area
```

**Rules.**

- A click on a loop (not on a tab) adds a hand tab at the nearest point
  of that loop (`nearest_on_loop`), in the drawing frame.
- A primary drag that starts within `PICK_THRESHOLD_POINT` of a tab
  moves it. The camera does not orbit during that drag. The point snaps
  to the loop on each frame (GUI state only). The release sends ONE
  `SetToolpathParam { tab_points }` (one undo step; the height diagram
  drag pattern, `height_diagram.rs:508`).
- A drag of an auto tab first turns the auto tabs of that loop into hand
  tabs at their planned points, then moves the one dragged. Hand tabs
  replace the auto tabs of their loop (P5), so without this the other
  auto tabs of the loop would vanish (decision L-5).
- A right-click on a hand tab deletes it. The delete of the last tab of a
  loop: see O-4.

**Files.** `ui/properties/operations/boundary_2d.rs:287` (the "Hand
tabs: N [Clear]" row gains `[Edit in viewport]`), `state/pick.rs`
(`PickMode::Tabs { toolpath_id, drag: Option<TabDrag> }`),
`interaction/picking.rs` (NEW `pick_tab`), `app/viewport.rs`,
`app/gpu_upload.rs` (tab bars, own buffer), `render/colors.rs`
(`TAB_AUTO`, `TAB_HAND`).

**MCP.** NEW tool `tab_layout { index }` (the marks). The write is
`set_toolpath_param tab_points` (exists).

**Tests.**

- Core NEW `the_tab_layout_is_the_cut_layout_p4_6`: on the P5 fixtures
  (the D shape, a rectangle), the `tab_layout` points equal the lift
  centres of the generated toolpath. Red proof: the query calls
  `auto_tabs` with another clearance.
- Core: `tabs_sit_on_straight_sides_p5` and
  `tabs_bridge_width_and_height_p5b` stay green.
- Viz controller: a drag release writes one `SetToolpathParam`; undo
  restores the old points; an auto-tab drag writes every tab of the loop.
- MCP: `tab_layout` equals the GUI marks (parity test).

**Operator looks at.** A tabbed part: drag one tab off a corner, add one,
delete one. Generate and compare the lift positions.

**Decision O-4 [OP].** `tab_points` cannot hold "this loop has no tab":
a loop with no hand tab gets `tab_count` auto tabs. So the delete of the
last tab of one loop cannot be stored. Options: (a, recommended for P4)
refuse that delete with a notice; (b) a NEW per-loop override (the D10
`item_overrides` hook keyed by `MatchKey`) with "no tabs". (b) lets a
part come loose, so the operator rules it.

**Decision L-5 [LEAD].** The auto-tab drag rule above.

---

### P4.7 — STL flat-region faces

**Goal.** S3 on an STL. A flat region of a mesh is a `Face` item with
`brep: None`. A face pick, a face rule and the face filter work on an STL
as on a STEP model.

**Core (NEW).**

```rust
// maps/geom_cache.rs, beside cached_silhouette
pub fn cached_flat_regions(mesh: &Arc<TriangleMesh>) -> Arc<ItemSet>;
// selection/flat_regions.rs (NEW, pure): the source
pub(crate) fn flat_region_items(mesh: &TriangleMesh) -> Result<ItemSet, ItemError>;
// session: the one door that gives the face items of a model
impl LoadedModel { pub fn face_items(&self) -> Option<Arc<ItemSet>>; }  // STEP: items; STL: the lazy set
```

- The source groups the flat triangles by plane Z, unions each group
  (`Polygon2::union_all`, as the fork does), and gives one `Face` item per
  merged polygon, with its own holes. Order: by Z, then by the lowest
  triangle index in the region (deterministic, so the key is stable).
- Flat test: one rule for STEP and STL, `HORIZONTAL_FACE_MIN_NORMAL_Z`
  (named in P2.4). The Z band: every vertex of a triangle within a band
  of the region Z. The band constant is decision L-7.
- Lazy forcing: `resolve` asks `face_items()` only when the selection
  names a face (a `Face` pick, an `Only` rule with `Face`, or a face
  filter). `KindFilter::Served` admits no face, so the default selection
  never forces it. A 661 k-triangle terrain pays nothing at load or at a
  default generation.
- `resolve` never builds the set. `selection_status` runs `resolve` on
  the frame loop, and a loaded project with a stored STL face pick would
  build the union on the first frame (the WP11b rule in `generation.rs`:
  the resolver "hands out a LAZY index and builds nothing"). With a cold
  cache, `resolve` gives a NEW status `FacesNotReady { model }`: the
  section shows "computing faces…", and generation waits or refuses with
  that name (the editor chooses with the lead).
- A GUI `Job` on the job lane fills the cache (the tier-map preview
  pattern, `AppState::job_run`). The worker generation path may build it
  on its own thread. When the `Arc` lands, the memo key (`Weak` identity)
  moves, so the report builds again once.
- Key: `brep: None`, anchor and measure as for a STEP face. The
  `selection_status` memo keys the lazy `Arc` by `Weak` identity, as
  today.
- `ListItems` lists faces of an STL only when `kind = face` is asked.

**Viz.** The ray hit on an STL mesh (`interaction/picking.rs`) finds the
`Face` item whose plane Z is within the band of the hit Z and whose ring
holds the hit XY. NEW `ProjectSession::face_pick_key_at(tc, model, hit)`.

**MCP.** `list_model_items { kind: "face" }` on an STL; the pick goes
through `edit_toolpath_selection`.

**Tests.**

- Core NEW `stl_flat_regions_are_face_items_p4_7`: the fork's stepped
  mesh (a 20 x 10 top at Z 5, a 4 x 4 boss at Z 8): two faces; the Z 5
  face has the boss as a hole, area 184 mm²; a wall gives no face; two
  loads of one STL give equal keys; a default selection does not force
  the cache (`geom_cache` stats). Red proof: union across Z.
- Core: `a_two_face_pick_keeps_both_faces_p2_4` gains an STL arm.
- Viz controller: an STL face click adds one pick.

**Operator looks at.** An STL with a pocket floor: pick the floor; the
Pocket cuts it; on the rivmap terrain the pick bar says "computing
faces…" and the frame rate holds.

**Decision L-7 [LEAD].** The constants. Flatness: reuse
`HORIZONTAL_FACE_MIN_NORMAL_Z` (one rule for every face source). Z band:
the fork uses 0.01 mm (`refs/remotes/basis/feature-web-server:
crates/rs_cam_server/src/face_region.rs`, `HEIGHT_TOLERANCE_MM`); the repo
has `CLOSE_TOL_MM` = 0.01 mm (`io/dxf_input.rs:589`). Recommended: name
one constant and cite both. Do not type a new number.

---

### P4.8 — Import options: SVG scale, join tolerance, import report

**Goal.** The user sees and sets the SVG scale at import. The join
tolerance (default 0.01 mm) is set per import. The import reports the
pieces joined and the open shapes left (addendum 2026-10-19).

**Step 0 (probe, read-only).** Import an SVG with `width="100mm"
viewBox="0 0 100 100"` and a 100 mm square through `load_svg_data` at
scale 1.0. Record the bbox. The doc comment says px (377.95). The fork's
round-trip test (unitless width = viewBox) gives 1:1. The probe settles
which one the GUI does today. The answer feeds O-2.

**Import dialog (NEW `ui/import_modal.rs`, SVG and DXF).**

```
 ┌ Import drawing — sign.svg ───────────────────────────────────┐
 │ File size     100.00 x 60.00 mm   (width="100mm")             │  KeyValueRow
 │ Scale         [ mm (1:1) ▾ ]  [ 1.000 ]                       │  ModelUnits::PRESETS + Custom
 │ Result        100.00 x 60.00 mm                               │  KeyValueRow, TEXT_STRONG
 │ Join gaps up to [ 0.010 ] mm                                  │  ValueRow
 │ Report        41 pieces → 12 shapes · 3 joined · 2 open left  │  OneLine; CAUTION when open > 0
 │                                         [ Cancel ] [ Import ] │
 └───────────────────────────────────────────────────────────────┘
```

The dialog runs the door again on each change, on the job lane for a
large file. The model panel (`model_sim_panels.rs:128`, Units / Scale)
shows the same report line and the join tolerance after import.

**Core (NEW).**

- `ProjectModelSection.import: ImportOptions { join_tolerance_mm: f64 }`
  (`session/project_file.rs`), default `CLOSE_TOL_MM`, not written at the
  default. A reload and a rescale reuse it.
- The chain kernel moves from `io/dxf_input.rs:647` to NEW
  `selection/join.rs` (`chain_open_pieces(pieces, tol) -> (chains,
  JoinCounts)`), so DXF and SVG share one join. The DXF tolerance stays
  `tol / scale` in drawing units.
- `ImportReport { pieces, joined, closed_by_join, open_left,
  join_tolerance_mm }` on `LoadedModel` (NEW field, not saved; each load
  computes it). `DxfImport` and the SVG door return it.
- NEW `Command::SetModelImportOptions { model_id, units, options }`
  (`set_model_import_options`): it loads again through
  `adopt_model_geometry` and stales every reader (`models_read`). It
  replaces nothing: `RescaleModel` stays for units only, or folds into it
  (lead).
- A changed tolerance changes the chains, so keys move and picks of the
  old pieces flag as lost (S18). That is the designed result.

**MCP.** `ImportModelParam` gains `units` and `join_tolerance_mm`; the
answer gives the bbox and the `ImportReport`. `inspect_model` gives the
report. NEW tool `set_model_import_options`.

**Tests.**

- Core NEW `import_reports_joins_and_open_shapes_p4_8`: the P1b DXF
  fixtures give the expected counts; a tolerance of 0.5 mm joins a gap
  of 0.3 mm and a 0.01 mm default does not; the option round-trips in the
  project file; a reload keeps it. Red proof: drop the option on reload.
- Core: `every_import_door_gives_items_p2_1` (25 drawings, FNV baseline)
  unchanged at the default tolerance for DXF.
- Core: `model_units_survive_reload_g_unitsreload` gains a join arm.
- Viz: `model_refresh_invalidates_results_g_rescalestale` pattern for
  `SetModelImportOptions`.
- MCP: `import_model` with units and tolerance gives the same bbox and
  report as the GUI dialog (parity).

**Operator looks at.** An Inkscape SVG in mm: the dialog size and the
result; a DXF with a 0.05 mm gap: the report, then a join at 0.1 mm.

**Decisions [OP].**

- O-2 The SVG default scale. Today the GUI imports at 1.0. If the probe
  shows px, the options are: (a) keep 1.0 and show it; (b) default to
  25.4/96 (the factor `svg_input.rs:15` cites); (c) read the physical
  `width`/`height` when the file declares units, else 25.4/96. Any change
  moves every new SVG import. Recommended: (c).
- O-5 SVG open subpaths join at the import tolerance. Today the SVG door
  does not chain. With the shared kernel, two open subpaths that meet
  within 0.01 mm become one closed loop, and a Pocket then cuts it.
  Recommended: yes, one join rule for every drawing door.

---

### P4.9 — Join and close tool (a per-model edit ledger)

**Goal.** The user joins open paths by distance, closes a path with a
line, or moves the ends of a path together. No import closes a gap with a
chord in silence. Each act is explicit and stored.

**Architecture (recommended).** A per-model edit ledger, replayed after
the source door.

```rust
// selection/drawing_edit.rs (NEW, pure)
pub enum DrawingEdit {
    /// Chain the open paths (of one layer, or of all) whose ends meet within the tolerance.
    JoinWithin { layer: Option<String>, tolerance_mm: f64 },
    /// Close one open path with a straight line from its end to its start.
    CloseWithLine { path: MatchKey },
    /// Close one open path: move its end point onto its start point (no new segment).
    MoveEndsTogether { path: MatchKey },
    /// Join two open paths end to end with a line between the nearest ends.
    JoinTwo { a: MatchKey, b: MatchKey },
}
pub struct EditOutcome { pub edit: usize, pub result: Result<EditEffect, EditLost> }
pub fn apply_drawing_edits(items: &ItemSet, edits: &[DrawingEdit]) -> (ItemSet, Vec<EditOutcome>);
// session/project_file.rs: ProjectModelSection.drawing_edits: Vec<DrawingEdit>  (NEW, skip if empty)
// session/command.rs: (Command, EditModelDrawing, "edit_model_drawing",
//                      EditModelDrawingArgs { model_id, edit: LedgerEdit::{Add(DrawingEdit), Remove(usize)} }, Effects)
```

- `model_from_geometry` (`io/mod.rs`) runs the ledger after the door and
  before `bound_to`. Every reader (op, boundary, status, viewport) sees
  one item set.
- `Item::new` computes each new key from the edited shape. An unchanged
  re-import replays to the same keys. A pick of a joined piece flags as
  lost (S18).
- An edit whose key matches no item is `EditLost`. The model panel lists
  it as the section lists a lost pick. It is never skipped in silence.
- `JoinWithin` uses the P4.8 kernel. A joined item takes the place of its
  lowest piece (the DXF chain rule) and the layer of its first piece. A
  join across layers refuses (D14).
- "Close with a line" adds a chord. It is an explicit act on one named
  path, so the no-silent-chord rule (P1) holds. The panel states the
  chord length before the act.
- Undo: NEW `UndoAction::ModelDrawingEdits { model_id, before, after }`.

**Panel (model selected, open shapes present).**

```
 OPEN SHAPES                                         2 open   SectionHeader; CountPill CAUTION
 Path on layer TEXT, 86.20 mm, gap 0.31 mm     [ Show me ]    DataTable rows
   [ Close with line (0.31 mm) ]  [ Move ends together ]
 Path on layer 0, 12.00 mm, gap 4.00 mm        [ Show me ]
 [ Join all within [ 0.50 ] mm ]                              Button + ValueRow
 ▸ Details: 3 edits stored (1 lost)                           Details, the ledger with Remove
```

In the viewport, with the model selected, a click on an open path opens
the same two actions in a popover.

**Alternative (not recommended).** Re-import with options (P4.8) covers
"join by distance" only. It cannot close one named path or move its ends.

**MCP.** NEW tool `edit_model_drawing { model_id, edit }`; `inspect_model`
lists the ledger and the outcomes. CLI: `Reach::Skip` with a reason (the
project file carries the ledger).

**Tests.**

- Core NEW `drawing_edits_replay_on_reimport_p4_9`: each edit on a
  fixture; a save and a load give equal keys; a re-import of a changed
  file flags a lost edit; a cross-layer join refuses. Red proof: replay
  after `bound_to`.
- Core: `open_paths_are_never_chord_closed_p1` stays green (the import
  still adds no chord).
- Viz controller: one undo step per edit.
- MCP: `edit_model_drawing` and the GUI give equal item sets (parity).

**Operator looks at.** The text-stroke DXF: close one path with a line,
join the rest at 0.5 mm, then a Pocket cuts them.

**Decision L-11 [LEAD].** The ledger (recommended) or re-import options
only. The tool home: the model panel plus the viewport popover.

---

### P4.10 — Docs, catalog, registry closure

- `FEATURE_CATALOG.md`: the selection section, highlight, pick mode, tab
  editor, STL faces, import dialog, join tool.
- `session/command.rs`: every P4 row `Reached` on GUI and MCP, or `Skip`
  with a reason. `command_registry_surfaces`, `command_surface_completeness`,
  `mcp_core_arm_describes_every_row`, `mcp_wire_surface_pin` green.
- Folder `CLAUDE.md` files (40 lines or fewer): `selection/`,
  `ui/properties/`, `ui/overlays/`, `render/`, `state/`, `app/mcp/`.
- `PLAN.md` status row for P4.

---

## 3. MCP parity (every UI action)

| UI action | MCP | State | Pinned by (P4.10 closure, 2026-10-20) |
|---|---|---|---|
| Add or remove a rule, Reset, Clear picks | `edit_toolpath_selection` | exists; P4.1 adds `target` | one command: `the_selection_section_writes_edits_only_p4_3` (GUI sends only it), `mcp_core_and_gui_give_one_selection_status_p2_7` |
| Click an item (add or remove) | `list_model_items` (+ `near`, P4.5) then `edit_toolpath_selection` | `near` NEW | `an_mcp_near_pick_and_a_gui_click_give_one_pick_p4_5` |
| Shift-click a layer | `edit_toolpath_selection` `add_rule` | exists | `an_mcp_near_pick_and_a_gui_click_give_one_pick_p4_5` (shift-click arm: `layer_rule_edit` = one wire `replace`) |
| Counts, notes, lost picks, refusal | `selection_status` | exists; + `target`, `item_states` | `mcp_core_and_gui_give_one_selection_status_p2_7`, `one_item_is_one_pick_p2_7` (core, `item_states`) |
| Face pick (STEP, STL, Face op, 3D op) | `list_model_items {kind: face}` + `edit_toolpath_selection` (`target`) | STL faces NEW (P4.7) | STL: `an_mcp_face_pick_and_a_gui_click_give_one_pick_on_an_stl_p4_7`; STEP: `an_mcp_face_pick_and_a_gui_click_give_one_pick_on_a_step_p4_d` (added in P4.10); 3D op: `mcp_core_and_gui_give_one_boundary_status_p4_1` |
| Boundary selection editor | `edit_toolpath_selection {target: boundary}`, `set_boundary_config` | target NEW | `mcp_core_and_gui_give_one_boundary_status_p4_1` |
| Face click on a 3D op whose boundary is not a selection (P4.D, L-8 revised) | none: MCP has no click act; `set_boundary_config` then `edit_toolpath_selection` reaches the same state | accepted (P4.D) | GUI only: `w4_a_3d_op_face_click_switches_the_boundary_to_selection_p4_d`; no parity test |
| "Show me" | `set_ui_view {frame_items}` + screenshot | NEW (P4.4) | GAP: one function (`RsCamApp::frame_selection_items`) serves both; GUI side `show_me_frames_the_items_p4_4`; no wire test (no headless `RsCamApp`) |
| Model eye (P4.D) | `set_ui_view {model_visibility}` | NEW (P4.D) | GAP: one command (`UiCommand::SetModelVisibility`; registry row `Skip("set_ui_view carries model_visibility instead")`); GUI side `a_dxf_draws_on_import_and_its_eye_hides_it_p4_d`; no wire test (no headless `RsCamApp`) |
| Overlay rows | `set_ui_view {overlays}` (registry) | automatic | registry sentries (`overlays_registry`) |
| Tab add, drag, delete | `set_toolpath_param tab_points`; read `tab_layout` | `tab_layout` NEW | `mcp_and_gui_give_one_tab_layout_p4_6` (the read); the write is one row (`set_toolpath_param`) |
| Import with scale and join tolerance | `import_model {units, join_tolerance_mm}` | NEW params | `import_model_with_options_matches_the_dialog_p4_8` |
| Change import options | `set_model_import_options` | NEW | `set_model_import_options_matches_the_model_panel_p4_8` |
| Join, close, move ends | `edit_model_drawing` | NEW | `edit_model_drawing_matches_the_model_panel_p4_9` |
| Hover, pick mode, filter bar | none (GUI interaction) | parity on the result | the click and shift-click rows above |

Every NEW tool gets a wire type in `rs_cam_mcp/src/server.rs` whose JSON
equals the project-file JSON, and a parity test in
`app/mcp/commands/selection.rs` (the P2.7 pattern).

---

## 4. Decisions

### For the operator (each changes what is machined)

| # | Question | Recommendation |
|---|---|---|
| O-1 | 3D op face pick: boundary selection plus auto top (A), or an explicit `HeightMode::FromFace` top (B)? | A for the merge gate; B in P3 for every op |
| O-2 | SVG default scale: 1.0, 25.4/96, or the declared physical size? | Probe first; then the declared size, else 25.4/96 |
| O-3 | Face op `stock_offset` on a region that is not the stock | Grow the stock item only; face other regions inside their outline |
| O-4 | Delete the last tab of one loop (a loop with no tab) | Refuse in P4; a per-loop override later |
| O-5 | SVG open subpaths join at the import tolerance | Yes: one join rule for every drawing door |

### For the lead (UI and structure)

| # | Question | Recommendation |
|---|---|---|
| L-1 | Selection write route | The section sends `EditToolpathSelection` only; the drill draft writer goes |
| L-2 | Owner of the refusal text (IC1) | The section; the footer row names the Geometry tab |
| L-3 | Pick target with an op selection and a `Selection` boundary | The section whose `Pick` is on; default `Op` |
| L-4 | Filter bar home | Floating, top centre, no width |
| L-5 | Drag of an auto tab | Convert the loop's auto tabs to hand tabs, then move |
| L-6 | Face op: first face pick under the stock default | Replace (the drill rule) |
| L-7 | STL flat constants | Reuse `HORIZONTAL_FACE_MIN_NORMAL_Z`; one named 0.01 mm band, cited |
| L-8 | Face click on a 3D op with no `Selection` boundary | A notice, no edit |
| L-9 | Highlight scope | Selected toolpath only, with its foreign items |
| L-10 | Colours and the `PATH_OPEN` token | The table in P4.4 |
| L-11 | Join tool architecture | The ledger |
| L-12 | One branch, or merge after P4.3 | Merge after P4.3 and the operator look |

---

## 5. Risks

- **The critical path is P4.4's `item_states`.** The highlight, the hover
  text, "Show me" and the tab editor's loop set read it. The index
  alignment depends on the memo key holding the item set by `Weak`
  identity; the sentry must cover a re-import.
- **P4.1 changes MCP wire types and the memo key.** `target` defaults to
  `op`, so old calls keep working. `mcp_wire_surface_pin` moves by design.
- **The SVG scale may be a live defect today** (3.78 times). The P4.8
  probe runs first. If it confirms, the lead may move the probe and O-2
  ahead of P4.4.
- **STL flat regions on a large mesh.** The union per Z level can take
  seconds on rivmap100. The job lane and the lazy forcing keep it off the
  frame loop and off the default path. `resolve` must never build it
  (`FacesNotReady`), or a saved STL face pick stalls the first frame. Measure on
  `planning/fixtures/` (not a scratch copy).
- **The tab drag fights the orbit drag.** The pick must claim the drag
  before `draw_viewport` orbits (`dragged_by(PointerButton::Primary)`
  in `app/viewport.rs`).
- **The hover must not rebuild the drawing buffer** (the V8/V13 lesson in
  `app/viewport.rs:262`). The separate `hover_lines` buffer holds it.
- **The join ledger is the largest and least certain item.** It goes
  last, after the import kernel move (P4.8) that it reuses.
- **The Face op adapter change (P4.2) risks G-OPGOLDEN.** The default
  must give `face_rectangle` byte for byte; prove it before any other
  change in that item.
