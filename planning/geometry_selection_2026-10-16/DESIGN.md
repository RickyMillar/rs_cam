# Geometry selection: P0 design (2026-10-19)

Status: APPROVED by the lead on 2026-10-19 under the operator autonomy
ruling, with the rulings in `PLAN.md` (section "Lead rulings on DESIGN.md").
Those rulings override this file where they differ (D2, D13, section 13).
Author: the P0 design agent (read-only).
Inputs: `PLAN.md` (P0 questions 1–12), `REQUIREMENTS.md` (S1–S27, R1–R9),
`GAP_ANALYSIS.md`, `planning/op_architecture_2026-10-14/PLAN.md`, and the
code on branch `geo-select` at d8407731.

Conventions in this file:

- **NEW** marks a name that does not exist in the code today.
- Paths are relative to `crates/rs_cam_core/src/` unless they start with
  `crates/`.
- "Golden" means `tests/every_op_keeps_its_toolpath_g_opgolden.rs`
  (G-OPGOLDEN).

---

## 0. Summary

The design has six parts.

1. **One item type.** A NEW folder `selection/` holds `Item`. An item is a
   loop, an open path, a point, a circle or a face. Each item carries its
   source, its layer and a `MatchKey`. The import doors fill
   `LoadedModel::items`. That field replaces `polygons`, `drill_targets` and
   `layers`. Containment (islands) moves from import time to resolution
   time, so a selection nests only the loops it selects.
2. **One selection type.** `Selection` holds rules and explicit picks. It is
   a field of each op config that cuts drawing geometry (the Drill configs
   already store their picks there). `face_selection`, `selected_holes`
   and `selected_layers` go. Each op states its own default selection:
   "every drawing item of the toolpath's model" for most ops, empty for
   the Pin Drill extras. Each default gives today's output.
3. **Resolution in two halves.** The session resolves the selection
   against the models. The resolution is pure, and the session moves the
   result into the setup frame through one door. The prologue then checks
   each item against the op's `ItemUse` (NEW capability) and fills
   `Prepared` with typed views: `regions()`, `curves()` and `holes()`. An
   adapter cannot read the model. An adapter cannot receive an open path
   when its op cuts regions.
4. **No silent fallback.** A lost pick, an empty result, a kind that the
   op cannot cut and a face that is not flat all refuse. The refusal names
   the item and the fix. One query (`SelectionStatus`, NEW) gives the same
   answer before generation, for the GUI badge and for MCP.
5. **Selection is "what", territory is "where".** For a 3D op the
   selection feeds only the boundary: `BoundarySource::Selection` (NEW)
   replaces `FaceSelection` and `ModelOutline`.
6. **Depth references in the setup frame.** A face item carries its plane
   Z and normal, both moved into the setup frame. P3 adds a per-region
   floor to `run_layered`.

P1 Trace answer: Trace can cut an open path on the line in P1 when its
`compensation` is `None`. Section 13 gives the files.

---

## 1. Items (question 1)

### 1.1 Where they live

The items get a NEW folder: `selection/` (a sibling of `geometry/`). It
gets its own `CLAUDE.md` of 40 lines or fewer (operator preference,
2026-09-17). The folder holds:

| File (all NEW) | Holds |
|---|---|
| `selection/mod.rs` | the facade |
| `selection/item.rs` | `Item`, `Shape`, `ItemKind`, `SourceRef`, `ItemSet` |
| `selection/key.rs` | `MatchKey` and the match rule |
| `selection/rule.rs` | `Selection`, `Rule`, `Filter`, `SelectionEdit` |
| `selection/resolve.rs` | `resolve` (pure) and `ResolvedSelection` |
| `selection/view.rs` | `ItemUse`, `OpItems`, `Regions`, `Curves`, `Holes`, the prologue check |
| `selection/refusal.rs` | `SelectionRefusal`, `SelectionNote` |
| `selection/stock.rs` | the stock source (section 2) |

`geometry/` keeps derived geometry. `polygon.rs` keeps `Polygon2`. An item
holds `Polygon2` rings; it does not replace `Polygon2` in the kernels.

### 1.2 The type

```rust
// selection/item.rs  (all NEW)

/// Where an item comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceRef {
    /// The model of the toolpath (`ToolpathConfig::model_id`).
    ToolpathModel,
    /// A named project model (`LoadedModel::id`). Exact id; no fallback.
    Model(ModelId),
    /// The stock of the setup, in the emission frame.
    Stock,
}

/// One pickable piece of 2D geometry.
#[derive(Debug, Clone)]
pub struct Item {
    source: SourceRef,          // stored as Model(id) once loaded
    layer: Option<Arc<str>>,    // DXF layer, SVG group label; None for STEP and stock
    key: MatchKey,              // computed by the source, never by hand
    shape: Shape,
}

#[derive(Debug, Clone)]
pub enum Shape {
    /// A closed ring. No holes: islands come from containment at resolution.
    Loop(Polygon2),                       // invariant: closed, >= 3 points
    /// An open polyline.
    Path(Polygon2),                       // invariant: !closed, >= 2 points
    /// A location: DXF POINT.
    Point(P2),
    /// A circle: DXF CIRCLE, a full-circle ARC, a circle-like SVG ring.
    Circle { centre: P2, diameter: f64, ring: Polygon2 },
    /// A flat BREP face (or, in P4, a flat region of a mesh).
    Face(FaceItem),
}

#[derive(Debug, Clone)]
pub struct FaceItem {
    /// Outer ring first, then the face's OWN inner loops (not containment).
    pub rings: Polygon2,
    /// The plane Z and the unit normal, in the frame of the item's owner
    /// (model frame in `LoadedModel`, setup frame after `to_setup_frame`).
    pub plane_z: f64,
    pub normal: V3,
    /// The BREP id, as a tie-break hint for the match only (section 9.3).
    pub brep: Option<FaceGroupId>,
}

/// The fieldless mirror of `Shape`, for rules, keys and refusal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind { Loop, Path, Point, Circle, Face }

/// The items of one source, in import order (the order matters: see 4.4).
pub struct ItemSet { items: Vec<Item> }
```

The fields of `Item` are private. A source builds an item through
`Item::new(source, layer, shape)`. That constructor computes the key and
checks the shape invariants. So no item holds a key that does not match
its shape, and no `Loop` is open.

### 1.3 Circles are one item

Today a DXF CIRCLE gives two records: one closed `Polygon2` and one
`DrillTarget` (`io/dxf_input.rs` ~209–231). A full ARC does the same
(~245–265). An SVG circle-like ring gives a polygon and a target
(`io/svg_input.rs::circle_like_drill_targets`). The new model gives ONE
`Shape::Circle`. It serves a region op as a loop and a drill op as a hole
(section 4.2). This removes the double record and the drift risk between
the two.

### 1.4 What replaces `Polygon2`-only input

| Today | New |
|---|---|
| `LoadedModel.polygons: Option<Arc<Vec<Polygon2>>>` (`session/mod.rs:376`) | `LoadedModel.items: Arc<ItemSet>` (NEW) |
| `LoadedModel.drill_targets: Arc<Vec<DrillTarget>>` (:380) | deleted; points and circles are items |
| `LoadedModel.layers: Arc<Vec<String>>` (:383) | deleted; `ItemSet::layers()` (NEW) lists the layers of every item, not only of targets |
| `LoadedGeometry::Polygons(Vec<Polygon2>, Vec<DrillTarget>, Vec<String>)` (`session/mod.rs:352`) | `LoadedGeometry::Drawing(ItemSet)` (NEW) |
| `DrillTarget`, `DrillTargetKind` (`io/dxf_input.rs:43–60`) | deleted after P2.3c |
| `ExecutionContext.polygons`, `.drill_targets` (`compute/execute.rs:320`, `:326`) | `ExecutionContext.selection: SelectionInputs` (NEW, section 4) |

Readers of `LoadedModel.polygons` outside generation (found with `rg`):
`session/mod.rs` (7), `session/compute.rs` (4), `session/save.rs` (3),
`session/pins.rs`, `session/multitool.rs`, `session/compute/export.rs`
(`height_context_for_toolpath`, the model bbox),
`session/compute/diagnostics.rs`, `gcode/mod.rs`, `session/load_report.rs`,
`io/mod.rs` (`model_from_geometry`), the viz renderer and
`crates/rs_cam_viz/src/app/gpu_upload.rs`. Each reads a bbox or draws the
drawing. Each gets `ItemSet::bbox()` or `ItemSet::iter()`. The editor of
P2.1 lists every caller in the report.

---

## 2. Sources (question 2)

### 2.1 The rule

A source is a function from a file (or the stock) to an `ItemSet`. There
is no source trait object: a source has one caller (the import dispatch in
`io/mod.rs::model_from_geometry` / `LoadedModel::from_file`). A source
calls only `Item::new`. No op, rule or resolver names a source.

| Source | Function | Change |
|---|---|---|
| DXF | `io/dxf_input.rs::extract_dxf` returns `DxfImport { items: ItemSet }` | Each entity gives one item with its layer (today a polygon loses its layer). LINE chains take the layer of their first segment; a chain across two layers splits at the layer change. `detect_containment` no longer runs here. |
| SVG | `io/svg_input.rs::load_svg_data` returns `ItemSet` | Open subpaths become `Path` items (today they are dropped, ~92–135). Layer = the label of the nearest ancestor group. Containment no longer runs here. The explicit scale stays as today (`ModelUnits`); P4 shows it at import. |
| STEP | NEW `selection/` reads `EnrichedMesh` in `io/step_input.rs` after `build_enriched_mesh` | One `Face` item per planar face group whose 2D loops exist (`enriched_mesh.rs:~450`, the `|normal.z| < 0.95` gate at :461). The item carries the signed normal, so up and down faces differ (today `abs` hides it). |
| STL flat regions (P4) | NEW lazy source over the mesh, cached by mesh identity in `maps/geom_cache` | P4 only. It gives `Face` items with `brep: None`. Idea from the fork's `face_region.rs`, rebuilt in core. Lazy, because a 661 k-triangle terrain must not pay at load. |
| Stock | `selection/stock.rs::stock_items(stock_bbox)` (NEW) | One `Loop` (the stock rectangle in the emission frame), `SourceRef::Stock`. It is built at resolution, not at load, because the stock is per setup. |

### 2.2 Import defects closed here

- A partial DXF ARC is closed with a chord (`dxf_input.rs` ~261–265:
  `Polygon2::new(arc_entity_to_points(..))`). It must become a `Path`.
  P1 step 0 fixes it (section 13), because P1 cannot see the open path
  otherwise.
- A closed DXF polyline with fewer than 3 points is dropped in silence
  (~189, ~203). It becomes a `Path` of 2 points, and the resolver names it
  if a region op selects it.
- SVG open subpaths are dropped in silence. They become `Path` items.

---

## 3. Selection (question 3)

### 3.1 Where it is stored

`Selection` is a field of every op config whose op cuts drawing geometry:

```rust
// compute/operation_configs.rs — for Pocket, Profile, Adaptive, Zigzag,
// Trace, Chamfer, VCarve, Inlay, InlayPlug, Drill, AlignmentPinDrill,
// ProjectCurve (and Face, if decision D5 is yes)
#[serde(default = "Selection::drawing", skip_serializing_if = "Selection::is_drawing")]
#[param(driver_set = DRIVER_SET_SELECTION)]   // NEW reason const
pub selection: Selection,
// AlignmentPinDrillConfig uses `Selection::empty` / `Selection::is_empty`;
// FaceConfig (D5) uses `Selection::stock` / `Selection::is_stock`.
```

```rust
// compute/catalog.rs — exhaustive, so a new variant must decide
impl OperationConfig {
    pub fn selection(&self) -> Option<&Selection> { match self { .. } }   // NEW
    pub(crate) fn selection_slot(&mut self) -> Option<&mut Selection> { .. } // NEW, crate-private
}
```

Why the op config and not `ToolpathConfig` (decision D1 gives the
alternative):

- A mesh op (`DropCutter`, `Waterline`, …) has no field, so it cannot
  hold a selection that does nothing. The compiler enforces it.
- The Drill picks already live in the op config (`DrillConfig:323`,
  `AlignmentPinDrillConfig:383`). One home for all picks.
- `generation_inputs_signature` (`session/mod.rs:873`), the GUI undo
  snapshot (`RestoreToolpathSnapshotArgs.operation`), the project file
  (`operation.params`) and a P7 template all carry the operation already.
  So they carry the selection with no new code.
- W5: `driver_set` keeps the field out of `set_toolpath_param`
  (`session/compute/params.rs:304–316`); only the selection command
  writes it.

The cost: `SetToolpathOperation` must carry the selection when the op type
changes (Pocket to Adaptive). The setter copies `old.selection()` into
`new.selection_slot()`. When the new op has no slot (a 3D op), the
selection is dropped and `Effects` reports it. The editor adds a sentry.

### 3.2 The type

```rust
// selection/rule.rs  (all NEW)

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    /// Union of the rule matches.
    rules: Vec<Rule>,
    /// Items added by hand, matched by geometry.
    picks: Vec<MatchKey>,
    /// Items removed by hand from the rule matches.
    excludes: Vec<MatchKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub source: SourceRef,
    pub filter: Filter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// Empty = every layer.
    pub layers: Vec<String>,
    /// `Drawing` = Loop, Path, Point, Circle (not Face).
    pub kinds: KindFilter,
    /// Circles only: diameter range in mm (S2).
    pub diameter: Option<DiameterRange>,   // { nominal_mm, tolerance_mm }
    /// Faces only: facing up or down in the SETUP frame, and a Z range.
    pub face: Option<FaceFilter>,          // { facing: Facing, z: Option<ZRange> }
}

pub enum KindFilter { Drawing, Only(Vec<ItemKind>) }
```

The fields of `Selection` are private. The only writer is
`Selection::apply(&mut self, SelectionEdit) -> Result<(), SelectionRefusal>`
(section 10.3). The constructors are `Selection::drawing()`,
`Selection::empty()`, `Selection::stock()` and `Selection::from_picks(..)`.

### 3.3 The default rule

`Selection` has NO `impl Default`. Each op states its own default, so a
file with no `selection` key loads as that op's default and never as the
default of another op. (A Pin Drill that loaded the drawing default would
drill every circle of the drawing in silence.)

```rust
impl Selection {
    /// Every drawing item (Loop, Path, Point, Circle) of the toolpath model.
    pub fn drawing() -> Self {
        Self {
            rules: vec![Rule { source: SourceRef::ToolpathModel,
                               filter: Filter { layers: vec![], kinds: KindFilter::Drawing,
                                                diameter: None, face: None } }],
            picks: vec![],
            excludes: vec![],
        }
    }
    /// No item. Pin Drill extras.
    pub fn empty() -> Self;
    /// The stock rectangle. Face (D5).
    pub fn stock() -> Self;
    /// The default of an op: `OperationConfig::default_selection()` (NEW,
    /// exhaustive) returns one of the three. `SelectionEdit::Reset` reads it.
    pub fn is_drawing(&self) -> bool;  // and is_empty, is_stock: the skip tests
}
```

The op's `ItemUse` (section 4.2) then decides which matched items it
cuts. The drawing default gives today's behaviour:

- A region op (Pocket …) gets every closed loop and circle, nested as
  today. A Face item is not a drawing kind, so a STEP model with no pick
  gives an empty selection and a refusal, as today ("requires 2D
  geometry").
- Drill gets every point and circle: today's `selected_holes = None`
  ("every target").
- Trace and Project Curve get loops, circles and paths.

### 3.4 What it replaces

| Today | New |
|---|---|
| `ToolpathConfig.face_selection: Option<Vec<FaceGroupId>>` (`session/mod.rs:830`) | `Selection` with `Face` picks, or a face rule |
| `DrillConfig.selected_holes`, `.selected_layers` (`operation_configs.rs:323`, `:329`) | `Selection` (layer rule, picks) |
| `AlignmentPinDrillConfig.selected_holes`, `.selected_layers` (:383, :387) | `Selection` whose default is EMPTY for this op (decision D6) |
| `DRIVER_SET_SELECTED_HOLES`, `DRIVER_SET_SELECTED_LAYERS` (:66–69) | `DRIVER_SET_SELECTION`. The test at `session/compute/tests.rs:437` (`driver_set_read >= 1`) still holds |
| "Clear the last hole = all holes" in the GUI (`controller/events/mod.rs:327`) | Gone. "Reset" is an explicit `SelectionEdit::Reset` |

---

## 4. Resolution (question 4)

### 4.1 The two halves

```
session::resolve_generation_inputs                       prologue::check_inputs
──────────────────────────────────                       ──────────────────────
op.selection()                                           ctx.selection (SelectionInputs)
  │ selection::resolve(sel, &ResolveScope)  ──►  ResolvedSelection
  │      pure; no frame; no refusal                        │ selection::view(resolved, caps.item_use)
  │ ResolvedSelection::to_setup_frame(info)                │   refuses (section 5)
  ▼                                                        ▼
ResolvedGenInputs.selection ──► ExecutionContext ──►  Prepared { CheckedGeometry::Items(OpItems) }
```

```rust
// selection/resolve.rs (NEW)
pub struct ResolveScope<'a> {
    pub toolpath_model: Option<&'a LoadedModel>,  // exact id; None when the id is missing
    pub models: &'a [LoadedModel],
    pub stock_bbox: &'a BoundingBox3,             // emission frame, for SourceRef::Stock
}

pub fn resolve(sel: &Selection, scope: &ResolveScope<'_>) -> ResolvedSelection;

pub struct ResolvedSelection {
    items: Vec<ResolvedItem>,      // import order, deduplicated by key
    lost: Vec<LostPick>,           // picks with no match, or two matches
    missing_sources: Vec<SourceRef>,
    unmatched_layers: Vec<String>, // rule layers that name no item (S21)
}
pub struct ResolvedItem { item: Item, by: MatchedBy }   // MatchedBy::{Rule(usize), Pick}: for the refusal text

impl ResolvedSelection {
    /// THE frame door: rings, points, centres AND the face plane Z and
    /// normal. It calls `SetupTransformInfo::apply_to_drawing_polygons`
    /// for the rings, as `transform_drawing_polygons_to_setup` does today.
    pub fn to_setup_frame(self, info: &SetupTransformInfo) -> Self;
}
```

`resolve` does not refuse. It reports. One function (`view`, below)
turns the report into a refusal. So the prologue and the
`SelectionStatus` query (section 5.3) give the same answer.

`to_setup_frame` replaces three frame doors that disagree today:
`transform_drawing_polygons_to_setup` for polygons (`generation.rs:190`),
`pick_to_emission_frame` for drill picks (`execute/drilling.rs:165`), and
no transform at all for `face_top_z` (`generation.rs:96`, the model-frame
defect). After P2 the drill adapter does not read `ctx.setup_transform`
for picks.

### 4.2 What `GeometryRequirement` becomes

`GeometryRequirement` (`compute/catalog.rs:46`) keeps its role (the
prologue's "what must exist") and loses the two drawing variants:

```rust
pub enum GeometryRequirement {
    Stock,
    Items,          // was Polygons and DrillTargets
    Mesh,
    ItemsAndMesh,   // was Both (Project Curve)
}
```

What the op does with the items is config-dependent (Trace with
`compensation = Left` cannot follow an open path; Profile with side On,
P6, can). So it lives in `Capabilities` (W2.4), where
`OperationConfig::capabilities()` (`compute/catalog.rs:743`) can override
it per config:

```rust
// compute/catalog/capabilities.rs (NEW field and enum)
pub struct Capabilities<P = EntryProbe> {
    ..,
    /// How the op cuts its selected items (P1 introduces it).
    pub item_use: ItemUse,
}

pub enum ItemUse {
    /// The op reads no items (mesh and stock ops).
    None,
    /// Closed areas, nested by containment: Loop, Circle, Face.
    Regions,
    /// Lines to follow: Loop, Circle, Path (closed and open).
    Curves,
    /// Hole positions: Point, Circle. `may_be_empty`: Pin Drill.
    Holes { may_be_empty: bool },
}

impl Shape {
    /// Exhaustive. A new kind must state each use.
    pub fn serves(&self, use_: ItemUse) -> bool { match (self, use_) { .. } }
}
```

The frozen table `tests/every_op_states_its_capabilities_w2_4.rs` gains
the column. A sentry holds `GeometryRequirement::Items` iff
`item_use != None`.

### 4.3 What `Prepared` gives

```rust
// compute/execute/prologue.rs
enum CheckedGeometry<'a> {
    None,
    Items(OpItems),                                   // replaces Polygons(&[Polygon2])
    Mesh(MeshInput<'a>),
    ItemsAndMesh { items: OpItems, mesh: MeshInput<'a> },
}

impl<'a> Prepared<'a> {
    pub fn regions(&self) -> Result<&Regions, OperationError>;  // NEW
    pub fn curves(&self)  -> Result<&Curves,  OperationError>;  // NEW
    pub fn holes(&self)   -> Result<&Holes,   OperationError>;  // NEW
    pub fn mesh(&self)    -> Result<MeshInput<'a>, OperationError>;
    pub fn territory(&self) -> &Territory<'a>;
    // polygons() is DELETED.
}
```

```rust
// selection/view.rs (NEW). Private fields; only `view` builds them.
pub struct Regions { polygons: Vec<Polygon2>, depth: Vec<Option<ItemDepth>> } // all closed, nested
pub struct Curves  { polygons: Vec<Polygon2> }    // closed (nested) and open, in containment order
pub struct Holes   { holes: Vec<Hole> }           // Hole { xy: P2, diameter: Option<f64>, layer }
impl Regions { pub fn as_slice(&self) -> &[Polygon2]; pub fn depth(&self, i: usize) -> Option<ItemDepth>; }
impl Curves  { pub fn as_slice(&self) -> &[Polygon2]; }
```

`regions()` requires `ItemUse::Regions`. `curves()` serves `Curves` AND
`Regions`: a closed region is also a closed curve, and for a `Regions` op
the curve view holds only closed polygons. So Trace with
`compensation = Left` (`item_use: Regions`) still reads `curves()`.
`holes()` requires `Holes`. Any other call errors with the existing
"registry adapter mismatch" text. This is the same
pattern as today's `polygons()` and `mesh()`. The type `Regions` can only
come from `view`, which checked every polygon is closed. A kernel that
takes `&[Polygon2]` gets `regions.as_slice()`.

`ItemDepth` is empty in P2 (always `None`). P3 fills it (section 7).

### 4.4 Containment at resolution, and why the golden does not move

Today `detect_containment` runs at import (`dxf_input.rs:159`,
`svg_input.rs:59`) over every polygon. After P2 it runs in `view` over
the SELECTED loops only. So a pick on layer A does not get a hole from a
loop on layer B unless layer B is also selected.

For the default rule the input of `detect_containment` is the same list
as today: every drawing item in import order, with `Loop` and `Circle` as
closed rings and `Path` as open rings. `detect_containment`
(`polygon.rs:~1575`) sorts by area with a stable sort and skips open
paths. Same input, same function, same output. The P2.2 sentry proves
`view(default) == old polygons` on every fixture drawing, byte for byte.

A `Face` item keeps its own inner loops. Containment does not run across
face items (each face is its own region; section 7.3).

### 4.5 `ExecutionContext`

```rust
// compute/execute.rs
pub struct ExecutionContext<'a> {
    ..,
    // pub polygons: Option<&'a [Polygon2]>,        DELETED
    // pub drill_targets: &'a [DrillTarget],         DELETED
    /// C3 pattern: private fields, only the prologue reads them.
    pub selection: SelectionInputs<'a>,              // NEW
    ..
}
```

`SelectionInputs` copies the `TerritoryInputs` pattern (`territory.rs`,
C3): its fields are private to `prologue.rs`, so the compiler refuses an
adapter that reads the resolved items around the check. The 29 callers of
`ExecutionContext::new` (19 in `compute/execute/tests.rs`) get a
test-only `SelectionInputs::from_polygons(&[Polygon2])` that applies the
default rule.

### 4.6 The adapters (one door)

| Op | Adapter (today) | Reads after P2 |
|---|---|---|
| Pocket, Zigzag, Profile, Chamfer | default `LayeredOp::outlines` (`layered.rs:305`) | `prepared.regions()?.as_slice()` in the default `outlines` |
| Trace | default `outlines` | `prepared.curves()?.as_slice()` (Trace overrides `outlines`) |
| Adaptive 2D | `clearing_2d.rs:627` | `prepared.regions()` |
| VCarve | `curve_engrave.rs:130` | `prepared.regions()` |
| Inlay, InlayPlug | `curve_engrave.rs:43`, `:86` | `prepared.regions()` |
| Project Curve | `curve.rs:92` `run_curve` | `prepared.curves()` |
| Drill | `drilling.rs:558`, `drill_holes_for_config` (:435) | `prepared.holes()`; keeps `unique_holes` (:249) |
| Pin Drill | `drilling.rs:597`, `pin_holes_in_emission_frame` (:203) | `ctx.alignment_pins` + `prepared.holes()` (extras) |
| Face | `clearing_2d.rs:513`, `FacePass::outlines` (:569) | D5: `prepared.regions()` with the stock rule as default; else unchanged |

The session refusal before the prologue (`generation.rs:129`:
`!is_3d() && !is_stock_based() && polygons.is_none()`) is deleted. It is
the reason a Drill on a mesh model refuses before the prologue today. One
refusal site remains: the prologue.

---

## 5. Refusal (question 5)

### 5.1 One structured refusal

```rust
// selection/refusal.rs (NEW)
pub struct SelectionRefusal {
    pub op: &'static str,
    pub item: Option<ItemRef>,   // { source, layer, kind, anchor, measure }: enough to highlight it
    pub cause: RefusalCause,
}
pub enum RefusalCause {
    Empty { matched_but_not_served: usize },
    LostPick,                         // no item matches the key after re-import
    AmbiguousPick { matches: usize }, // two items match one key (duplicate entity)
    KindNotCut { kind: ItemKind, use_: ItemUse },  // open path in a region op, etc.
    FaceNotFlat,                      // a picked face that is not a planar horizontal face
    MissingSource(SourceRef),         // a model id that does not exist; no first-model fallback
    UnknownLayer(String),             // a rule layer that names no item (S21)
}
impl SelectionRefusal { pub fn fix(&self) -> &'static str; }  // exhaustive: one fix text per cause
// compute/execute.rs: NEW variant
pub enum OperationError { .., Selection(SelectionRefusal) }
```

`Display` gives one sentence with the item, the cause and the fix. For
example: "Pocket: open path on layer 'TEXT' (starts at X 12.0 Y 40.5,
86.2 mm long) — Pocket cuts closed loops only. Use Trace or Project Curve
for this path, close it in the drawing, or remove it from the selection."

### 5.2 Refuse, and the operator question

Operator ruling 2026-10-19: "every closed-only op refuses an open path".
The design follows it. An item that the selection matched and that the op
cannot cut refuses, by any rule or pick. With the drawing default, a
Pocket on a DXF that holds text strokes and part outlines refuses. The fix
text names the edit: "set the selection to closed loops only (a rule with
kinds = loop, circle), or remove the path".

`SelectionNote` (NEW finding through the W2.3 door) is still useful: a
rule that names kinds leaves other items out, and the note counts them
("2 open paths on layer TEXT are not in the selection").

Decision D2 asks the operator one question: may the DEFAULT selection of a
closed-only op name the closed kinds only (`Only([Loop, Circle])`)? Then
an open path is never in the selection, so it is never given to the op;
the note counts it, and the drawing pockets with no edit. That is a
change of what the default selects, so it needs the operator, not the
lead. Until a ruling, the design refuses.

### 5.3 The silent fallbacks this removes

| Fallback today | Where | After |
|---|---|---|
| A face pick with no polygon logs a warning and is ignored | `generation.rs:110–118` | `FaceNotFlat` names the face |
| A `FaceSelection` boundary with no pick takes the stock rectangle | `generation.rs:~757` (`resolve_containment_polygon`) | `BoundarySource::Selection` resolves or refuses (`Empty`) |
| A multi-face union keeps only the largest polygon | `enriched_mesh.rs:~298–335` | Each face is a region; nothing is dropped |
| One bad face drops the whole pick | same | Only the bad face refuses, by name |
| Face geometry is used only when the model has no polygons | `generation.rs:108` | The selection holds faces and drawing items together |
| `find_model_by_raw_id` takes the first model when the id is missing | `generation.rs:61` | `MissingSource` for every selection source |
| A stale drill pick on a model with no targets still drills | `drilling.rs` (G-DRILLPICKSTALE scope note, F4.8 residual) | `ResolveScope` knows the model resolved; the pick is `LostPick` |
| `Some(empty)` drill pick vs `None` | `drilling.rs:289` | No `Option`: an empty explicit selection is `Empty` |
| Trace silently closes an open path, and drops a 2-point line | `ops/trace_path.rs:98–114`, `toolpath.rs:312` | P1 (section 13) |
| SVG open paths dropped; DXF partial arc closed | section 2.2 | Items |

### 5.4 Feedback without generation (R6, S22)

```rust
// session/command.rs — NEW Query row
(Query, SelectionStatus, "selection_status", SelectionStatusArgs, SelectionReport, ..)

pub struct SelectionReport {
    pub counts: BTreeMap<ItemKind, usize>,   // "14 loops, 3 islands, 2 open"
    pub islands: usize,
    pub skipped_by_kind: BTreeMap<ItemKind, usize>,
    pub lost: Vec<ItemRef>,
    pub refusal: Option<SelectionRefusal>,   // what generation would refuse with
}
```

It runs `resolve` + `to_setup_frame` + `view`, with no kernel. The GUI
badge, the viewport highlight (P4) and MCP read it.

---

## 6. Selection and territory (question 6)

- **Selection** = what to cut (the op's input items).
- **Territory** = where the tool may go (`compute/execute/territory.rs`:
  boundary, obstacles, stock). Unchanged.

They combine as today: the op plans on its selected items. For the 2.5D
ops (`TerritoryXy::ClipAfter`) the session clips the result to the
boundary after generation. The area ops subtract obstacle footprints
through `Territory::area_outlines` (K3) from `regions().as_slice()`.

A 3D op has no selection field. It uses a selection only through its
boundary:

```rust
// compute/config.rs
pub enum BoundarySource {
    Stock,
    ModelSilhouette,
    /// NEW. Replaces `ModelOutline { model_id, holes }` and `FaceSelection`.
    /// The regions are the selected regions (loops, circles, faces), nested.
    /// `holes`: the regions are the holes of their union (today's
    /// `ModelOutline.holes` rule, kept as a region transform).
    Selection { selection: Selection, holes: bool },
    DerivedRestRegions { source_toolpath_id: ToolpathId },
    PlannedTierRegions { .. },
}
```

- `ModelOutline { model_id: 7, holes }` becomes
  `Selection { selection: Rule { source: Model(7), kinds: Only([Loop, Circle]) }, holes }`.
  `resolve_model_outline_polys` (`generation.rs:~560`) becomes a call to
  `resolve` + `view(ItemUse::Regions)` + the union and the holes step it
  has today. G-OPGOLDEN has `ModelOutline` boundary cases (the `regions`
  arms, `every_op_keeps_its_toolpath_g_opgolden.rs:715`). They must not
  move; P2.5 proves it.
- `FaceSelection` becomes `Selection { picks: [face keys] }`.
- A boundary selection with `SourceRef::ToolpathModel` resolves against
  the toolpath that owns the boundary. (`StockConfig` holds no boundary
  table in the project file today; `boundary_inherit` is a flag only. The
  P2.5 editor checks what `boundary_inherit` reads before it relies on
  this.)
- `DerivedRestRegions` and `PlannedTierRegions` are not selections. They
  stay.

The selection never widens the territory. A region op whose selected
region lies outside the boundary cuts nothing there, as today.

---

## 7. Depth references (question 7, built in P3)

### 7.1 References name a frame

```rust
// compute/config.rs
pub enum HeightReference {
    StockTop, StockBottom, ModelTop, ModelBottom,
    /// NEW. Per region: the plane Z of the face item that made it, in the
    /// SETUP frame. A region that is not a face refuses (`RefusalCause::NoFaceForReference`).
    ItemFace,
}
```

`HeightMode::FromReference(ReferenceOffset)` already takes a reference
and an offset (`compute/config.rs:247`). A top or a floor names one of
five references, or `Manual(z)` (typed, setup frame), or `Auto`.
`HeightReference::resolve_z` for `ModelTop`/`ModelBottom` falls back to
the stock in silence (`config.rs:~200`). P3 makes that a refusal when the
op's model has no bbox.

### 7.2 The setup frame

`ResolvedSelection::to_setup_frame` moves `FaceItem::plane_z` and
`normal` through the setup transform. "Facing up" is
`normal.z > 0` in the setup frame. So a face that points down in the
model faces up in a Bottom setup, and the `FaceFilter::facing` rule reads
it correctly. This also fixes `face_top_z` (`generation.rs:88–106`, model
frame). In P2 the "auto top follows the picked faces" rule
(`generation.rs:249`) moves into the session resolution as
`ResolvedSelection::face_top_z()` (NEW), in the setup frame. That changes
output only for a face pick on a non-identity setup; no golden case has
one.

### 7.3 A floor per region through `run_layered`

`run_layered` (`compute/execute/layered.rs:375`) computes one ladder
(`op.ladder(ctx)`) for every outline. P3 changes the hook:

```rust
// layered.rs
fn ladder(&self, ctx: &ExecutionContext<'_>, depth: Option<ItemDepth>) -> Vec<f64>; // changed
pub struct ItemDepth { pub top_z: f64, pub floor_z: f64 }                           // NEW
```

- `ByArea` (region-first) already loops per outline. It calls `ladder`
  per outline.
- `Global` (level-first) uses the union of all ladders, sorted top first.
  An outline emits at a level only when the level is in its own ladder.
- `area_outlines_less_obstacles` reads the lowest level per outline.
- The default `spans` hook snaps to the union ladder.
- `Regions::depth(i)` gives `ItemDepth` for outline `i`. `None` means "the
  op ladder".

Faces at different Z: each face item is its own region with its own
inner loops. Containment does not run across face items. A lower face
inside the hole of an upper face is cut by its own region from the top to
its floor. The upper face's region treats that hole as an island down to
its own floor. The result clears both. P3's multi-floor STEP fixture
proves it.

### 7.4 Which ops honour a per-item floor

NEW capability `Capabilities::item_floor: bool`. True for Pocket,
Adaptive 2D, Zigzag and Profile (P3 list). The prologue refuses an
`ItemFace` floor on an op without it. Drill hole floors (from a face) are
a P6 item. VCarve takes its depth from the V angle; Trace and Chamfer
keep a typed depth.

These four ops ignore a pinned bottom today
(`honors_pinned_bottom_z: false`; sentry
`compute/execute/pinned_bottom_z_reaches_motion_g_bottompin.rs`). P3
makes the bottom reference reach the motion for them. Decision D9.

### 7.5 GUI

The heights tab (`crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:1113`
`draw_heights_tab`, `operations/mod.rs:311` `draw_heights_params`) shows
each reference by `HeightReference::label()` and its resolved Z. The
height diagram (`operations/height_diagram.rs:222`) draws one band per
distinct floor and names the face. Read-only data comes from
`SelectionReport` (NEW field `floors: Vec<(ItemRef, f64)>` in P3).

---

## 8. Params (question 8)

| Stays in W5 `OpParams` (op config) | Belongs to the selection |
|---|---|
| side, climb, compensation, stepover, depth, depth per pass, finishing passes, region ordering | which items, by rule and by pick |
| tab count, width, height; hand-placed tab positions (P5, `#[param]`, snapped to the nearest selected loop at generation) | layer, kind, diameter and face filters |
| the drill cycle, peck, dwell | the item's floor source (by the `ItemFace` reference, section 7) |

Per-item overrides (side, cut direction, start point per loop; GAP #8)
are not in P2. The hook: a NEW op-config field
`item_overrides: Vec<(MatchKey, ItemOverride)>` with `driver_set`, read
by `view` into `Regions`. It keys on the same `MatchKey`, so it survives
a re-import exactly as a pick does. The selection stays "which geometry";
W5 stays the owner of every machining number. Decision D10.

---

## 9. Freshness (question 9)

### 9.1 A selection edit

The selection lives in `operation`, so `generation_inputs_signature`
(`session/mod.rs:873`) changes with it. The edit command calls
`invalidate_result_chain(index, ..)` (`session/mutation/toolpath.rs:237`)
as every toolpath edit does. Only that op and its dependents go stale.
A boundary selection edit goes through `SetBoundaryConfig`, which
already does the same.

### 9.2 A re-import

`adopt_model_geometry` (`session/mutation/config.rs:467`) replaces the
model's `items` and calls `drop_results_for_model` (:401). That function
today matches `tc.model_id` and an enabled `ModelOutline`. It reads a NEW
function instead:

```rust
impl ToolpathConfig {
    /// Every model this toolpath reads: its model, every `SourceRef::Model`
    /// of its op selection and of its boundary selection.
    pub fn models_read(&self) -> BTreeSet<usize>;   // NEW
}
```

`dependencies::edges` and `set_toolpath_model` read the same function.

- A rule resolves again at the next generation. It follows the file
  (S17).
- An explicit pick that no longer matches is `LostPick`. The op is not
  changed: tool, feeds, linking, heights and the other picks stay (S18).
  `SelectionStatus` reports it immediately, so the GUI flags the op after the
  `Effects` of the re-import. Generation refuses until the user re-picks
  or removes it.
- A face pick has no Z in its key (section 9.3), so a face that moves in
  Z in the CAD model still matches (S19). Its floor then follows the new
  Z.
- `set_toolpath_model` (`session/mutation/toolpath.rs:~633`, where a
  `FaceGroupId` today points into the new model) needs no special case:
  picks re-resolve against the new model and flag when they miss.

### 9.3 The match key

```rust
// selection/key.rs (NEW)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchKey {
    pub source: SourceRef,
    pub kind: ItemKind,
    pub layer: Option<String>,
    /// Loop, Face: area centroid. Path: first vertex. Point: the point. Circle: the centre.
    pub anchor: [f64; 2],
    /// Loop, Face: sqrt(|area|) (mm). Path: length. Circle: diameter. Point: 0.
    pub measure: f64,
    /// Face only: the BREP id, a tie-break when two faces share anchor and measure.
    pub brep: Option<u16>,
}
```

Two keys match when source, kind and layer are equal and anchor and
measure agree within `DRILL_PICK_MATCH_EPS_MM` (`execute/drilling.rs`,
1e-6 mm). That is today's pick rule, moved into `selection/key.rs`, and
the drill constant moves with it. A re-import of an unchanged file gives
the same tessellation and so the same key. A moved item is lost and
flagged, not guessed. The face key has no Z. `FaceGroupId` is
topological order, so it is never the key (R2: the FreeCAD naming
problem). A looser match (by tolerance or by nearest) is decision D3.

---

## 10. Wire and file (question 10)

### 10.1 Project file

The selection serializes inside `[setups.toolpaths.operation.params]`:

```toml
[setups.toolpaths.operation]
kind = "pocket"
[setups.toolpaths.operation.params]
depth = 3.0
# ...
[[setups.toolpaths.operation.params.selection.rules]]
source = "toolpath_model"
filter = { layers = ["CUT"], kinds = { only = ["loop", "circle"] } }
[[setups.toolpaths.operation.params.selection.picks]]
source = "toolpath_model"
kind = "circle"
layer = "PINS"
anchor = [12.0, 40.0]
measure = 5.0
```

The per-op `skip_serializing_if` (`is_drawing`, `is_empty`, `is_stock`)
keeps the op's own default out of the file, so a project that uses only
defaults saves as today. A file with no `selection` key loads as the
op's own default (section 3.3).
`format_version` stays 3.

`RETIRED_KEYS` rows (`session/project_file.rs:86`):

| Table | Key | Replacement text (short) |
|---|---|---|
| `setups/[]/toolpaths/[]` | `face_selection` | "Removed (P2): face picks are in operation.params.selection. Re-pick the faces" |
| `setups/[]/toolpaths/[]/operation/params` | `selected_holes` | "Removed (P2): drill picks are in operation.params.selection" |
| `setups/[]/toolpaths/[]/operation/params` | `selected_layers` | "Removed (P2): use a layer rule in operation.params.selection" |
| `setups/[]/toolpaths/[]/boundary/source` | `model_outline` | "Removed (P2): use source.selection with a model rule" |

`source = "face_selection"` is a VALUE, not a key, so `RetiredKey`
cannot find it; serde would give "unknown variant" with no fix. NEW
sibling table `RETIRED_VALUES: &[RetiredValue { table, key, value,
replacement }]`, checked in `check_retired_keys`. Decision D8.

Breaks to state in the P2.6 commit: old `face_selection` picks do not
convert (FaceGroupId has no geometry key). Old `selected_holes` could be
converted to keys, but the no-legacy ruling says delete and refuse.

### 10.2 Commands

| Row | Kind | Wire name | Replaces |
|---|---|---|---|
| `EditSelection` (NEW) | Command | `edit_toolpath_selection` | `SetFaceSelection` (`command.rs:573`), `SetDrillSelectedHoles` (:583) |
| `SelectionStatus` (NEW) | Query | `selection_status` | — |
| `ListItems` (NEW) | Query | `list_model_items` | the face list of `inspect_brep_faces` for picking |
| `SetBoundaryConfig` | Command | unchanged | carries `BoundarySource::Selection` |

```rust
pub struct EditSelectionArgs { pub index: usize, pub edit: SelectionEdit }   // NEW
pub enum SelectionEdit {                                                       // NEW
    Replace(Box<Selection>),
    AddRule(Rule), RemoveRule(usize),
    AddPick(MatchKey), RemovePick(MatchKey),   // RemovePick of a rule match adds an exclude
    TogglePick(MatchKey),                       // the viewport click
    Reset,                                      // back to OperationConfig::default_selection()
}
```

One row with an edit enum keeps the registry small and makes each edit a
pure, testable `Selection::apply`. It refuses on an op whose config has
no selection slot. Surfaces: GUI, MCP and CLI all `Reached`.

### 10.3 Undo

- `RestoreToolpathSnapshotArgs.face_selection` (`command.rs:1006`) is
  deleted. The selection is inside `operation`, which the snapshot
  already restores.
- `UndoAction::ToolpathParamChange` (`crates/rs_cam_viz/src/state/history.rs:40`)
  loses `face_selection` for the same reason.
- The boundary is not in any undo snapshot today. With a selection inside
  the boundary, a boundary pick should undo. Decision D11 (recommend: add
  `boundary` to the snapshot in P2.6).

### 10.4 MCP (`crates/rs_cam_mcp`, `crates/rs_cam_viz/src/mcp_server.rs`)

- NEW tool `edit_toolpath_selection` with a wire type
  `EditSelectionParam { index, edit: SelectionEditWire }` in
  `rs_cam_mcp/src/server.rs`. The wire `MatchKey` is the same JSON as the
  file.
- NEW tools `selection_status { index }` and `list_model_items { model_id,
  layer?, kind? }`. The item list gives each key, so an agent picks by
  copying a key.
- `get_toolpath_params` (`app/mcp/project.rs:253`) returns the selection
  inside the op params (it does not return `face_selection` today).
- `inspect_model` (`project.rs:314`) adds item counts per kind and layer
  (it reports no targets or layers today).
- `SetBoundaryConfigParam` (`server.rs:825`): `source = "selection"` with
  a `selection` field; `model_id` and `outline_holes` retire; `holes`
  replaces `outline_holes`. `"face_selection"` was never accepted on the
  wire.

### 10.5 GUI and CLI

- `AppEvent::ToggleFaceSelection` and `AppEvent::ToggleDrillTarget`
  (`crates/rs_cam_viz/src/ui/mod.rs:343/350`) become one
  `AppEvent::TogglePick { toolpath_id, key }` (NEW), applied as
  `SelectionEdit::TogglePick`.
- `draw_drill_target_selector` (`ui/properties/operations/drill.rs:40`)
  becomes the generic selection panel in P4; in P2 it writes
  `SelectionEdit`s.
- `ToolpathEntry.face_selection` (`state/toolpath/entry.rs:37`, `:156`)
  is deleted.
- CLI: NEW job key `selection` in `OperationDef` (`crates/rs_cam_cli/src/job.rs:192`,
  `JOB_KEYS`), the same TOML shape. The CLI builders that write
  `face_selection: None` (job.rs:1153, run.rs:145, smoke.rs:536,
  rough_score.rs:548) lose that line.

### 10.6 Retired names (summary)

File keys: `face_selection`, `selected_holes`, `selected_layers`,
`boundary.source.model_outline`; value `boundary.source = "face_selection"`.
Commands: `SetFaceSelection`, `SetDrillSelectedHoles`. MCP fields:
`SetBoundaryConfigParam.model_id`, `.outline_holes`. Types:
`DrillTarget`, `DrillTargetKind`, `DxfImport.layers`,
`BoundarySource::ModelOutline`, `BoundarySource::FaceSelection`,
`GeometryRequirement::{Polygons, DrillTargets, Both}`,
`Prepared::polygons`, `ExecutionContext::{polygons, drill_targets}`,
`ResolvedGenInputs::{polygons, drill_targets, face_boundary}`,
`EnrichedMesh::faces_boundary_as_polygon`.

---

## 11. Extension test (question 11)

### 11.1 A new source: 3MF

1. Write `io/threemf_input.rs` (NEW). It returns a mesh. If the file
   carries 2D slices or flat regions, it also returns an `ItemSet` built
   with `Item::new`.
2. Add one arm to the import dispatch (`LoadedModel::from_file` /
   `io::model_from_geometry`) and one `ModelKind` variant.

Untouched: every op, `Selection`, `resolve`, `view`, the prologue, the
file format of the selection, MCP, the GUI selection panel. A mesh-only
3MF gets flat-region items from the P4 lazy mesh source with no 3MF code.
The one place: the import door.

### 11.2 A new item kind: text

Case A, text as outlines or single-line strokes (the usual case). The DXF
or SVG source emits the glyphs as `Loop` and `Path` items with the text
entity's layer, and a NEW optional `group: Option<GroupKey>` on `Item`
(one group per TEXT entity) plus a `Filter::group`. No new kind. Ops,
`view` and the prologue are untouched.

Case B, a true new kind (`Shape::Text { string, baseline, glyphs }`):

1. Add the variant to `Shape` and `ItemKind` (`selection/item.rs`).
2. The compiler then names every site that must decide: `Shape::serves`
   (which `ItemUse` it serves), `MatchKey` construction, the `view`
   arms, `RefusalCause` text, the wire enum. All are in `selection/`.
3. The source that produces it (one door).

Untouched: every op adapter, the catalog rows, `run_layered`, the
session. An op that wants text as such changes its own `item_use`.

### 11.3 A new op that accepts loops

1. A config struct with `#[derive(OpParams)]` and the field
   `selection: Selection` (`driver_set = DRIVER_SET_SELECTION`).
2. Its catalog row: `geometry: GeometryRequirement::Items`,
   `capabilities.item_use: ItemUse::Regions`.
3. Its arms in the exhaustive `OperationConfig::selection()` and
   `default_selection()`. The build fails until they exist.
4. Its adapter reads `prepared.regions()`.

All four are the op's own definition, which a new op writes anyway.
Untouched: sources, `Selection`, `resolve`, `view`, refusals, the project
file shape, `EditSelection`, MCP tools, the GUI selection panel (it reads
`item_use`), `SelectionStatus`, W4 (its selection check iterates
`OperationType::ALL`).

---

## 12. Migration per op (question 12)

"Default" = the default selection. Golden column = expected G-OPGOLDEN
effect of P2 for that op.

| Op | Today's input | New input | `item_use` | Golden |
|---|---|---|---|---|
| Face | stock box (`face_rectangle`) | D5 yes: `regions()` with default `Rule { source: Stock }`; else unchanged | Regions (D5) | none (prove it) |
| Pocket | all model polygons; face outline when no polygons | `regions()` | Regions | none |
| Profile | all polygons | `regions()` (P6: `Curves` when side On) | Regions | none |
| Adaptive 2D | all polygons | `regions()` | Regions | none |
| Zigzag | all polygons | `regions()` | Regions | none |
| Trace | all polygons, open ones closed | `curves()` | Curves (compensation None), Regions (Left/Right, until P6) | none (fixtures are closed only, `tests/common/op_fixtures.rs:158`, `:228`) |
| Chamfer | all polygons | `regions()` | Regions | none |
| VCarve | all polygons | `regions()` | Regions | none |
| Inlay, InlayPlug | all polygons | `regions()` | Regions | none |
| Drill | `selected_holes` or all `drill_targets` | `holes()` | Holes { may_be_empty: false } | none |
| Pin Drill | pins + `selected_holes` | pins + `holes()`; default selection EMPTY | Holes { may_be_empty: true } | none |
| Project Curve | all polygons + surface mesh | `curves()` + `mesh()` | Curves | none |
| 3D ops (12) | mesh + boundary | mesh + boundary (`BoundarySource::Selection` for the old `ModelOutline` cases) | None | none (the `regions` arms must stay byte-identical) |

Output changes that P1/P2 cause on purpose, with no golden case:

- An open path in a closed-only op: refused from P1 on (D2 asks the
  operator about the default).
- Trace on an open path: cut on the line (P1).
- A face pick on a non-identity setup: the auto top in the setup frame
  (section 7.2).
- A multi-face pick: all faces, not only the largest union piece.
- A DXF partial arc: a `Path`, not a chord-closed loop (P1 step 0).

---

## 13. P1: open-path honesty, and the Trace answer

**Answer: Trace on-line open-path cutting fits P1, for
`compensation = None`.** It needs no selection model, because
`Polygon2.closed` (`polygon.rs:118`) already carries the open flag from
the DXF door to `Prepared::polygons()`.

P1 items (one editor):

0. **Import fix.** `io/dxf_input.rs` ~261–265: a partial ARC becomes
   `Polygon2::open_path`. Without this, the P1 research cannot see the
   arc as open. Sentry: the arc's polygon is open.
1. **The capability.** Add `Capabilities::item_use: ItemUse` (section 4.2)
   now, with today's payload. `prologue::require_polygons`
   (`prologue.rs:139`) refuses an open polygon when the op's `item_use`
   is `Regions`, with the text of section 5.1 (no layer yet: polygon
   index, start point, length). P2 then swaps the payload under the same
   check; P1 adds no flag that P2 must delete. Trace overrides it in
   `OperationConfig::capabilities()`: `Curves` for `None`, `Regions` for
   `Left`/`Right`. Project Curve: `Curves`. Every other drawing op:
   `Regions`. Drill: `Holes`. The frozen W2.4 table gains the column.
2. **Trace emits open paths.** `ops/trace_path.rs::trace_polygons_at_z` /
   `trace_ring` (~98–114) branch on `poly.closed`: an open path uses
   `Toolpath::emit_path_segment_with_intent` (`toolpath.rs:262`), not
   `emit_closed_contour_with_intent` (:304), and the `< 3` point guard
   (`toolpath.rs:312`) becomes `< 2` on the open branch, so a 2-point
   line is cut.
3. **Compensation on an open path.** `offset_polygon_reported`
   (`polygon.rs:586`) offsets through a closed cavalier polyline
   (`polygon.rs:269`). So `Left`/`Right` on an open path refuses in P1
   (by item 1: Trace's `item_use` is `Regions` then). The open-offset
   kernel is P6, with Profile side On.
4. **Rest Trace.** `TracePass::reach_band` (`clearing_2d.rs:~355`) gives
   the rings to `standing_material::levels_in_reach` (`standing_material.rs:326`),
   whose band mask treats a ring as closed. For an open path the closing
   chord adds band, so the rule sees MORE reach: it cuts more levels,
   never fewer. That is the safe direction. P6 passes the open flag for an exact band.
5. **Entry and link.** Project Curve's open paths already pass the
   dressups; the editor checks a Trace open path through
   `clip_and_split_entries` and the ramp entry once, by the P1 research
   fixture.

Sentries: one per op family (open path refused in a region op; Trace cuts
an open line and a 2-point line; Trace Left on an open path refuses). Red
proof by restoring `emit_closed_contour_with_intent`. Golden: none
(fixtures are closed only).

---

## 14. P2 items

Each item is sized for one editor session. Each runs the item's sentry,
the folder sentries it touches, `cargo test -p rs_cam_core -q --lib`
filters for the touched modules, viz `--lib` when core types move
(memory rule), `cargo test -p rs_cam_mcp -q`, then clippy and fmt.
"Golden" = G-OPGOLDEN must not move unless the row says so.

| # | Item | Main files | Sentry (NEW unless named) | Golden |
|---|---|---|---|---|
| P2.1 | `selection/item.rs`, `key.rs`, `stock.rs`, folder `CLAUDE.md`. Sources fill `LoadedModel.items` (DXF with layers, SVG with open paths and group labels, STEP faces with signed normal and plane Z). `polygons`, `drill_targets`, `layers` deleted; their readers use `ItemSet`. A transitional `LoadedModel::outlines_pre_selection()` gives exactly today's polygon list (DXF closed + open, SVG closed only) until P2.3b deletes it. | `io/dxf_input.rs`, `io/svg_input.rs`, `io/step_input.rs`, `io/mod.rs`, `session/mod.rs`, readers in 1.4 | `every_import_door_gives_items_p2_1`: layer on every DXF loop and path; one item per circle; SVG open path kept; STEP face normal sign; `outlines_pre_selection()` equals the old polygon list on each fixture drawing | none |
| P2.2 | `Selection`, `Rule`, `Filter`, `SelectionEdit`, `resolve`, `ResolvedSelection::to_setup_frame`, `view`, `Regions`/`Curves`/`Holes`, `SelectionRefusal`. Pure; no wiring. | `selection/rule.rs`, `resolve.rs`, `view.rs`, `refusal.rs` | `--lib selection`: each filter; pick match and lost pick; ambiguous pick; containment over the selected loops only; `view(default)` equals `outlines_pre_selection()` on the fixtures; face key has no Z | n/a |
| P2.3a | The views, fed by the transitional list: `ItemUse` payload moves from `Polygon2.closed` to `Item`; `GeometryRequirement::{Items, ItemsAndMesh}` on all 24 rows; `Prepared::{regions, curves}`; `SelectionInputs` on `ExecutionContext` (built from `outlines_pre_selection()` with the drawing default); every adapter in 4.6 except Drill; `SelectionInputs::from_polygons` for the 29 `ExecutionContext::new` callers. Delete `Prepared::polygons` and `ctx.polygons`. | `compute/execute/*.rs`, `compute/catalog*.rs`, `session/compute.rs` | `every_op_states_its_capabilities_w2_4` (new column); `--lib catalog`; `cancellable_families_honour_a_preset_cancel_flag` green | none |
| P2.3b | The selection field: `selection` on each config (`driver_set`, per-op serde default), `OperationConfig::{selection, default_selection}`, session resolution in `resolve_generation_inputs`, `to_setup_frame`. Delete `generation.rs:129`, `outlines_pre_selection`, the face block (`face_boundary`, `face_top_z` → `ResolvedSelection::face_top_z()` in the setup frame). W4 gains check 11 (selection). | `operation_configs.rs`, `compute/catalog.rs`, `session/compute/generation.rs`, `session/compute.rs` | `a_layer_rule_cuts_only_its_layer_p2_3` (2-layer DXF, Pocket); `an_absent_selection_key_loads_the_op_default_p2_3` (Pocket, Drill, Pin Drill); `every_op_conforms_w4` check 11 | none |
| P2.3c | Drill and Pin Drill onto `holes()`. Delete `selected_holes`, `selected_layers`, `drill_targets_refusal`, `stale_drill_picks_refusal`, `resolve_drill_picks`, `pick_to_emission_frame`, `DrillTarget`. Move `DRILL_PICK_MATCH_EPS_MM` to `selection/key.rs`. The F4.8 residual closes. Update `diagnostics/adapters/from_preconditions.rs:80` and `session/compute/diagnostics.rs:1652` to `SelectionStatus`. | `compute/execute/drilling.rs`, `operation_configs.rs`, `diagnostics/` | `drill_picks_resolve_to_targets_g_drillpickstale` rewritten on keys (the empty-target exemption becomes a refusal); `pin_holes_are_drilled_by_size_r1`, `dowel_holes_have_one_source_p8`, `a_drill_hole_is_cut_once_g_drilltwice` stay green | none |
| P2.4 | Refusals and feedback: `OperationError::Selection`, `SelectionNote` finding, `SelectionStatus` query, `MissingSource` (no first-model fallback), `FaceNotFlat`, multi-face regions. Delete `EnrichedMesh::faces_boundary_as_polygon`. | `compute/execute.rs`, `findings.rs`, `session/command.rs`, `geometry/enriched_mesh.rs` | `selection_refusals_name_the_item_p2_4` (one arm per `RefusalCause`); `a_two_face_pick_keeps_both_faces_p2_4` | none |
| P2.5 | `BoundarySource::Selection` replaces `ModelOutline` and `FaceSelection`; `resolve_model_outline_polys` and `resolve_containment_polygon` read it; `ToolpathConfig::models_read()` drives `drop_results_for_model` and `dependencies::edges`. | `compute/config.rs`, `session/compute/generation.rs`, `session/compute.rs`, `session/mutation/config.rs`, `session/dependencies.rs`, `session/multitool.rs` | the `ModelOutline` sentries renamed onto `Selection`; `a_reimport_stales_every_reader_of_the_model_p2_5`; `stale_set_has_one_answer_wp28` green | none: the `regions` arms stay byte-identical (prove it) |
| P2.6 | File and commands: `RETIRED_KEYS` rows, `RETIRED_VALUES` (NEW), `EditSelection` row, `ListItems` row, `SetToolpathOperation` carries the selection, undo snapshot (delete `face_selection`; add `boundary` if D11), GUI `TogglePick`. Delete `SetFaceSelection`, `SetDrillSelectedHoles`. | `session/project_file.rs`, `session/save.rs`, `session/command.rs`, `session/mutation/*.rs`, viz `state/`, `controller/events/` | `command_registry_completeness`, `setters_have_rows_wp15a`, `selection_round_trips_p2_6`, `a_retired_selection_key_is_named_p2_6`, viz undo test, `an_op_change_keeps_its_selection_p2_6` | none |
| P2.7 | MCP and CLI: wire types, `edit_toolpath_selection`, `selection_status`, `list_model_items`, `get_toolpath_params` and `inspect_model` additions, `SetBoundaryConfigParam` change, CLI job key `selection`. | `crates/rs_cam_mcp/src/server.rs`, `crates/rs_cam_viz/src/mcp_server.rs`, `app/mcp/*.rs`, `crates/rs_cam_cli/src/job.rs` | `cargo test -p rs_cam_mcp -q`; a viz MCP test per tool; a CLI integration case with a layer rule; GUI/MCP number parity on `selection_status` | none |

Order: P2.1 → P2.2 → P2.3a → P2.3b → P2.3c → P2.4 → P2.5 → P2.6 → P2.7. P2.2
can run as a read-only design check in parallel with P2.1, but only one
editor works in the checkout.

A live GUI check by the operator closes P2 (visible: the drill picker, the
face pick, the counts).

---

## 15. Clean hooks for later phases (ruled, not built here)

| Phase | Hook in this design |
|---|---|
| P3 depth refs | `HeightReference::ItemFace`, `Regions::depth`, `ItemDepth`, `ladder(ctx, depth)`, `Capabilities::item_floor`, setup-frame `plane_z` |
| P4 UI + sources | `SelectionStatus`, `ListItems`, `SelectionEdit::TogglePick`, lazy mesh-flat source, `ItemRef` for highlight |
| P5 tabs | hand tabs as W5 `#[param]` points, snapped to the nearest `Regions` loop at generation; `ProfilePass::emit_level` per contour |
| P6 open paths, holes | `ItemUse::Curves` for Profile side On (config override); open-offset kernel; "extend open path to stock" as a `view` step for Pocket (NEW `ItemUse::RegionsExtendOpen`, decided in P6); helix-mill holes larger than the drill from `Holes` with `diameter` |
| P7 templates | a template = op configs whose selections use rules only (no picks); `unmatched_layers` names the missing layers (S21); tool match by property stays in P7 |
| P8 recognition | a recognizer is a function `&ItemSet -> Selection` (picks or rules) that the user reviews through `EditSelection`; it never cuts by itself |

---

## 16. Open decisions (for the lead)

| # | Question | Recommendation | Reason |
|---|---|---|---|
| D1 | Selection on the op config, or on `ToolpathConfig`? | Op config (section 3.1). | A mesh op cannot hold one (compiler). Drill picks already live there. Signature, undo, file and templates carry it with no new code. Cost: one carry-over in `SetToolpathOperation`. |
| D2 | OPERATOR QUESTION (revisits the 2026-10-19 ruling). May the default selection of a closed-only op name closed kinds only, so an open path in the drawing is left out with a note, not refused? | Ask the operator. My view: yes. | The ruling stands until then (refuse). With "yes", a DXF with text and outlines pockets by default, and every open path the user names still refuses. It changes what the default selects, so the lead cannot rule it. |
| D3 | Match tolerance for a pick | Exact (`DRILL_PICK_MATCH_EPS_MM`), no Z in a face key. | It is today's rule and a cited constant. A moved item flags (safe). A looser or nearest match is a P4 option with a reviewed constant. |
| D4 | Can one selection read several models? | Yes: `SourceRef::Model(id)` beside `ToolpathModel`. | It is how `ModelOutline` already works (a DXF boundary on an STL op). It costs only the source field. |
| D5 | Face onto `Items` with a stock rule in P2? | Yes, in P2.3b, with a golden proof. | One model for every 2.5D op; "face a region" (S, or L as a region) needs no extra code. If the golden moves, defer Face to P4 and keep `Stock`. |
| D6 | Pin Drill extras onto the selection? | Yes, default EMPTY (`may_be_empty`). | R1: one model. The pins stay live through `ctx.alignment_pins`. |
| D7 | `BoundarySource::Selection` in P2 or P4? | P2.5. | It is the last reader of `face_selection` and of `ModelOutline`; without it, the old fields must stay for one more phase. |
| D8 | `RETIRED_VALUES` for an enum value in the file | Add it. | Without it, `source = "face_selection"` fails with a serde error that names no fix (C1 spirit). |
| D9 | P3: a bottom reference reaches the motion for Pocket, Adaptive 2D, Zigzag, Profile | Yes, and re-bless those four by name. | S6 needs it. A project with a pinned bottom on these ops changes output: state it in the P3 commit and rewrite G-BOTTOMPIN. |
| D10 | Per-item overrides (side, direction, start point) | Not in P2. Later: an op-config `item_overrides: Vec<(MatchKey, ItemOverride)>`. | The selection stays "which geometry"; W5 stays the schema owner of machining numbers; keys make overrides survive re-import. |
| D11 | Boundary in the undo snapshot | Yes, in P2.6. | A boundary pick is now an edit the user expects to undo. |
| D12 | Inlay and Inlay Plug "share one selection" | By copy through a template (P7), not by reference. | The plug runs in its own project (ruling R2, op-arch). A cross-project reference has nothing to point at. |
| D13 | P1: Trace `Left`/`Right` on an open path | Refuse in P1; open offset in P6. | The offset kernel closes the ring (`polygon.rs:269`). |
| D14 | Layer of a DXF LINE chain that crosses layers | Split the chain at the layer change. | A chain then belongs to one layer, so a layer rule is exact. It can change the chain count of such files; no fixture has one. |

---

## 17. Evidence and checks for the editors

- The default selection reproduces today's lists only if import order and
  the stable area sort are kept (section 4.4). P2.1 and P2.2 prove it by
  equality, not by the golden alone.
- G-OPGOLDEN fixtures hold closed loops only (`op_fixtures.rs:158`,
  `:228`, and the mirror at `:284–299` keeps `closed`). The `regions`
  arms use a `ModelOutline` boundary (P2.5 must keep them).
- `generation_inputs_signature` already covers `operation` and `boundary`
  (`session/mod.rs:873`); no new freshness code is needed for edits.
- `execute_operation_annotated` callers that resolve no model
  (`compute/execute.rs`, the `&[]` drill-target path named in
  G-DRILLPICKSTALE) get an empty `SelectionInputs`. A Drill there then
  refuses with `Empty`. The P2.3c editor lists those callers first.
