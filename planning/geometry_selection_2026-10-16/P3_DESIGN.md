# Geometry selection P3: depth references (design, 2026-10-20)

Status: DRAFT for the lead. Author: the P3 design agent (read-only).
Inputs: `PLAN.md` (P3, the status table, every ruling, "Operator rulings,
2026-10-20 morning": D9), `REQUIREMENTS.md` (R4, S5, S6), `DESIGN.md`
(7.1–7.5, 9.3, 15, 16 D9), `P4_DESIGN.md` (O-1), and the code on `master`
at 48105367. Another session edits `session/project_file.rs`,
`session/save.rs`, `session/mod.rs`, `gcode/`, `material/` and `machine/`
now. Line numbers in those files can move.

Conventions (as P4_DESIGN): **NEW** marks a name that does not exist
today. Core paths start at `crates/rs_cam_core/src/`, viz paths at
`crates/rs_cam_viz/src/`. **[OP]**: changes what is machined; the operator
rules it. **[LEAD]**: structure, UI or wire; the lead rules it. Each item
is one editor session.

---

## 0. Summary

The ruling D9: Pocket, Profile, Zigzag and Adaptive 2D get ONE floor
field. The field names its reference. The op `depth` and the pinned Heights
Bottom Z stop being two fields that can disagree.

The design in one paragraph. A NEW enum `Floor` replaces `depth` in the
four op configs. Its default variant `Floor::Top { depth }` is today's
`depth`: "this far below the resolved Top Z of the op". So every project
that does not pin a bottom cuts byte for byte as today. The other variants
are the stock top, the stock bottom, the model top, each picked face (S6)
and an absolute Z. ONE session resolver gives the heights to generation,
the GUI and MCP, in the setup frame. For the four ops its Bottom Z IS the
floor. A pinned Heights Bottom Z on the four ops is refused at load and at
every write door. The top keeps `HeightMode` in P3 (no `FromFace`).

| # | Item | Output change | Visible result |
|---|---|---|---|
| P3.1 | One heights resolver for every surface, in the setup frame | refusals only (L-4) | the Heights tab shows the face top that generation cuts from |
| P3.2 | The `Floor` type, the file break, the floor owner | refusals only (an old key, a pin, an inverted floor) | `floor = { from = "top", depth = 6.0 }` in the file |
| P3.3 | Wire: `set_toolpath_floor`, CLI job key, schema | none | MCP and CLI set a floor |
| P3.4 | Up faces and down faces | down-face picks only | a down face names its fix |
| P3.5 | Each face at its own floor: Pocket, Zigzag, Profile (`run_layered`) | new floors only | one op cuts several floors |
| P3.6 | Each face at its own floor: Adaptive 2D | new floors only | the same for Adaptive 2D |
| P3.7 | GUI: the floor row, the Heights tab, the height diagram | none | each depth shows its reference |
| P3.8 | Docs, catalog, parity closure | none | none |

---

## 1. What exists (the base of every item)

| Piece | Where | Fact |
|---|---|---|
| `HeightsConfig::resolve` | `compute/config.rs:334` | Auto bottom = `auto_bottom_z` (`:361`): `top_z - op_depth.abs()`, or the model/stock bottom when `op_depth` is 0 (`ZERO_DEPTH_EPSILON`) |
| `SetupEvalContext::height_context` | `session/eval_context.rs:156` | THE context builder; `heights_stock_bbox` is the emission frame |
| Generation heights | `session/compute/generation.rs:193–236` | resolves, then the auto top follows the picked faces (`:228`), and the auto bottom follows the top |
| The 2.5D ladder | `session/compute/generation.rs:256` → `compute/catalog.rs:1403` `cutting_levels(top_z)` | Pocket/Profile: `DepthStepping { start: top, final: top - depth.abs(), dpp, finishing_passes }`; Zigzag/Adaptive: `uniform(depth, dpp)` |
| `effective_levels` | `compute/execute/layered.rs:643` | takes `cutting_levels` when not empty; the else arm uses `heights.depth()` with `finishing_passes: 0` |
| `generate_adaptive` | `compute/execute/clearing_2d.rs:773` | NOT a `LayeredOp`; its own outline loop, one ladder |
| G-BOTTOMPIN | `compute/execute/pinned_bottom_z_reaches_motion_g_bottompin.rs` | arm 1 (`:142`): a pin at −20 does not move a 6 mm pocket floor |
| `honors_pinned_bottom_z` | `compute/catalog/capabilities.rs:61`; 24 rows in `compute/catalog/registry.rs` | true for Adaptive3d, UnifiedFinish, Waterline |
| Operation tables | `tests/every_project_table_refuses_an_unknown_key_f034.rs` (untracked, other session) | the toolpath and operation tables stay OPEN (no `deny_unknown_fields`), by agreement with this programme |
| GUI heights | `state/job.rs:215` `height_context_from_session`; `ui/properties/operations/mod.rs:311` `draw_heights_params`; `operations/height_diagram.rs:222` | each calls `heights.resolve(ctx)` itself |
| MCP heights | `session/compute/export.rs:306` `height_context_for_toolpath` | a second copy of the viz builder |

Blast radius (measured with `rg`): 127 Rust files name one of the four
config types; 156 struct literals; 241 `depth:` lines in those files; 38
Pocket and 2 Profile toolpaths in tracked TOML files, plus 2 job-file
operations (section 3.4). No tracked file pins a Bottom Z on the four ops
(scan of every tracked `*.toml`). 28 of the tracked Pockets and Profiles
pin a Top Z (`planning/rivmap_block_job/nz_south.toml`, `testpiece.toml`,
`testpiece_mini.toml`).

Defects found on the way (fixed inside the named item):

- **D-A (live, parity).** The Heights tab, the height diagram and the MCP
  diagnostics do not apply the face-top rule of
  `generation.rs:228–235`. With a picked face, the GUI shows Top Z = stock
  top, and generation cuts from the face. Fix in P3.1.
- **D-B (live, parity).** For a drawing model, the GUI and MCP builders
  read `LoadedModel::bbox` (`session/mod.rs:642`: the drawing at Z 0).
  Generation reads `mesh.bbox` (`generation.rs:217`: `None` for a
  drawing, so `ModelTop` falls back to the stock top). On a stock whose
  top is not Z 0, a `ModelTop` reference shows one Z and cuts another.
  Fix in P3.1.
- **D-C.** `face_top_z` reads down faces (`resolve.rs:249`). Fix in P3.4.

---

## 2. The type (question 1)

### 2.1 `Floor`

```rust
// compute/floor.rs (NEW file; compute/ owns HeightReference)
/// The floor of a Pocket, Profile, Zigzag or Adaptive 2D: one value that
/// names its reference (ruling D9). `depth` is always "mm below the
/// reference" (a negative value stands above it). Every Z is in the setup
/// frame (the emission frame).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case", deny_unknown_fields)]
pub enum Floor {                                   // NEW
    /// Below the resolved Top Z of the op (auto: the stock top, or the
    /// top of the picked faces; else the pinned top). Today's `depth`.
    Top { depth: f64 },
    StockTop { depth: f64 },
    /// Below the stock bottom: a positive depth cuts into the spoilboard.
    StockBottom { depth: f64 },
    ModelTop { depth: f64 },
    /// Each face region of the op selection at its own plane Z (S6).
    EachFace { depth: f64 },
    /// An absolute Z.
    Z { z: f64 },
}
```

File spelling: `floor = { from = "top", depth = 6.0 }`,
`floor = { from = "each_face", depth = 0.0 }`, `floor = { from = "z", z = -20.0 }`.
The default of each op is `Top { depth: <today's default depth> }`
(Pocket 3.0 `operation_configs.rs:561`, Profile 6.0 `:662`, Adaptive 6.0
`:753`, Zigzag 3.0 `:1086`). The field has NO serde default: a file must
state it (section 3).

Why named fields, and not `{ reference, offset }`: one word has one
meaning. `depth` is always "below", `z` is always an absolute Z. The
`offset` of `HeightMode::FromReference` is signed (+ up); the users type a
positive depth.

### 2.2 The resolved floor

```rust
// compute/floor.rs
pub enum FloorRef {            // NEW: what the height context carries
    BelowTop(f64),             // Floor::Top
    At(f64),                   // StockTop, StockBottom, ModelTop, Z: resolved Z
    EachFace { deepest: f64 }, // EachFace: the lowest region floor
}
pub struct RegionFloors(Vec<f64>);  // NEW: per region of `Regions`, EachFace only
```

`HeightContext` gains `floor: Option<FloorRef>` (NEW field). One builder
fills it (P3.1 `ProjectSession::toolpath_heights`). `auto_bottom_z`
(`config.rs:361`) reads it first:

- `BelowTop(d)` → `top_z - d.abs()`. This is the expression of today
  (`top_z - ctx.op_depth.abs()` with `op_depth = depth`), so it gives the
  same bits.
- `At(z)` → `z`. `EachFace { deepest }` → `deepest`.
- `None` → today's rule (every other op).

So `ResolvedHeights.bottom_z` of the four ops IS the floor. The
`ZERO_DEPTH_EPSILON` arm (the model bottom) comes after the floor arm, so
a floor that is not `Top` cannot fall into it and deepen the cut.

### 2.3 The ladder reads the floor

`OperationConfig::cutting_levels(top_z)` (`catalog.rs:1403`) changes to
`cutting_levels(&ResolvedHeights)`. The four arms call ONE NEW function
`ops::depth::floor_ladder(top_z, floor_z, dpp, finishing_passes)`:
`DepthStepping { start_z: top_z, final_z: floor_z, max_step_down: dpp,
finishing_passes }.all_levels()`. Zigzag and Adaptive pass
`finishing_passes: 0`, as `uniform` does today (`catalog.rs:1410–1412`).
The other arms read `heights.top_z` and their own depth, unchanged.

The G-BOTTOMPIN mechanism ("the ladder takes the top only") is gone for
the four ops: their ladder takes the floor, and the floor is the bottom.

CAUTION: an inverted floor cuts deeper with no word unless it refuses. A
floor at or above the top (a `Z` above the top, `Top { depth <= 0 }`, a
large negative `StockBottom` depth) gives an empty ladder
(`DepthStepping::total_depth` is `max(0)`, `ops/depth.rs:217`). The empty
ladder then falls into the else arm of `effective_levels`
(`layered.rs:643–656`), which steps to `top - heights.depth()`, and
`depth()` is `(top - bottom).abs()` (`config.rs:398`): a floor 5 mm ABOVE
the top cuts 5 mm BELOW it. So:

- `toolpath_heights` refuses an `OpFloor` op whose resolved floor is at
  or above its top (`SessionError`, NEW text: the toolpath, the floor Z,
  the top Z). Per-region floors refuse `FloorAboveTop` (section 5.1).
- **[LEAD] L-16**: delete the else arm of `effective_levels`. Its two
  callers (`layered.rs:318`, `clearing_2d.rs:779`) get a non-empty ladder
  from every op after P3 (the editor confirms it on the execute tests);
  an empty ladder refuses. G-BOTTOMPIN arm 2 ("the fallback honours the
  pin") goes with it and becomes the inverted-floor arm.

### 2.4 The readers with no height context

Seven readers compute a depth from the op config alone. Each gets an
answer in P3.2:

- `total_depth()` and `depth_semantics()`: `Floor::Top { depth }` →
  `Some(depth)` / `DepthSemantics::Explicit(depth)`, as today. Every other
  variant → `None` / NEW `DepthSemantics::Resolved` ("the session resolves
  the floor; read `ResolvedHeights.bottom_z`"). `DepthSemantics`
  (`catalog.rs:76`) has no wildcard match, so the compiler names each site.
- Suggest snap (`feeds/suggest/apply.rs:159`), Suggest aggressiveness
  (`feeds/suggest/aggressiveness.rs:342`), sim triage
  (`stock/sim_triage.rs:796`): `None` means "no snap", the arm they have
  today for a surface op. **[LEAD] L-6.**
- Depth beyond stock (`diagnostics/adapters/from_static_checks.rs:602`),
  profile through cut (viz `ui/properties/operations/validate.rs:393`),
  setup sheet (viz `io/setup_sheet.rs:10`): read `ResolvedHeights.bottom_z`
  from the one resolver. For `Top` it is the same number as today.
- Generation (`generation.rs:220`, `:233`) and `default_depth_for_heights()`:
  the `Explicit` arm stays; `Resolved` → 0. With `HeightContext.floor` set,
  `op_depth` no longer decides the bottom of the four ops.

### 2.5 The floor owner (replaces `honors_pinned_bottom_z`)

```rust
// compute/catalog/capabilities.rs
pub enum FloorOwner {          // NEW; replaces `honors_pinned_bottom_z: bool`
    HeightsBottom,  // Adaptive3d, UnifiedFinish, Waterline: the Heights Bottom Z
    OpFloor,        // Pocket, Profile, Zigzag, Adaptive: `Floor`
    OpDial,         // the other 17 ops: their own dial or the surface
}
```

An exhaustive enum, not a bool: the GUI note (`operations/mod.rs:265`),
the diagram colour (`height_diagram.rs:238`), the volumetric policy
(`compute/execute/volumetric.rs:155`, `:221`), the diagnostics rule
(`from_static_checks.rs:565`, `diagnostics/ids.rs:228`) each `match` it.

### 2.6 One floor field: who refuses a Bottom pin on the four ops

`HeightsConfig` serves all 24 ops, so `bottom_z` stays. On an `OpFloor`
op it must be `Auto`. ONE NEW check, `ToolpathConfig::floor_conflict()`,
runs at the loader (section 3.2), at every write door that sets heights
or an op (`set_heights_config`, `session/mutation/config.rs:178`;
`replace_toolpath_config`, the GUI door; the op-kind change;
`add_toolpath`), and at generation as a last guard. The refusal names the
toolpath and the fix ("Pocket 'X' takes its floor from the Floor row; set
Bottom Z to Auto").

Rejected **[LEAD] L-1**: delete `bottom_z` and give every op a floor
field (the type-enforced form). It moves 39 lines in `config.rs`, 24
registry rows and the Heights tab of three ops that read the pin
correctly. The ruling names four ops.

### 2.7 The top in P3

The top keeps `HeightMode` and its references; no `FromFace` top
(P4_DESIGN O-1 B). **[LEAD] L-2.** Reasons: the top row already names its
reference, and the auto top already follows the picked faces (O-1 A), so
R4 for the top is met once P3.1 shows the source; no story needs a top
from a face outside the selection; `FromFace` makes `HeightMode`
non-`Copy` and puts a face lookup in five rows of every op.

One rule changes: **under `Floor::EachFace` the auto top does not follow
the faces**; it stays at the stock top, else the depth is 0 (L-15). A
pinned top stays. Under `Floor::Top` the rule of today holds, so a face
pick under the default floor cuts as today: into the face by `depth`.
O-2 asks whether a first face pick switches the floor.

---

## 3. Migration (question 2)

### 3.1 The rule

**Recommendation: refuse with a named message; no conversion** (the
2026-09-16 rule, the D9 text, `project_file.rs:85–92` "nothing converts
an old key", as P2.3c and P2.6 did). `depth = d` → `Floor::Top { depth: d }`
would cut the same, but it is an alias. A pinned Bottom Z on the four ops
never reached the cut (G-BOTTOMPIN): a conversion to `Floor::Z` would cut
deeper with no word (−20 against 6 mm), and a silent drop is a key the
loader ignores. So the pin refuses too. **[OP] O-1.**

### 3.2 The retired rows

`RetiredKey` matches a path and a key only, but `depth` also lives on
Face, Trace, Drill and Project Curve, and `heights.bottom_z` on every op.
P3.2 adds ONE NEW row type in `session/project_file.rs`:
`RetiredOpEntry { kinds, table, key, when: Option<fn(&toml::Value) -> bool>, replacement }`
and `RETIRED_OP_ENTRIES`. `check_retired_keys` (`project_file.rs:263`)
walks `setups[].toolpaths[]`, reads `operation.kind`, and adds one
`RetiredKeyFound` per hit, so one refusal names every old key (C2).

| Kinds | Table | Key | When | Replacement text |
|---|---|---|---|---|
| pocket, profile, zigzag, adaptive | `operation.params` | `depth` | any | "Removed (geometry selection P3, ruling D9): the floor is operation.params.floor. For the same cut write floor = { from = \"top\", depth = <this value> }. Delete the key" |
| pocket, profile, zigzag, adaptive | `heights` | `bottom_z` | `mode` is not `"auto"` | "Removed (P3, D9): the floor of this op is operation.params.floor. A pinned Bottom Z never reached its cut (G-BOTTOMPIN). Set bottom_z to { mode = \"auto\" } and set the floor" |

The check runs before the typed parse (`project_file.rs:295`), so serde
never sees the old key. This matters: the operation tables are open
(F034 agreement). Without the row, serde gives "missing field `floor`"
and names no fix.

### 3.3 What an old file does at load

The two keys on one of the four ops: refused (`SessionError::RetiredKeys`,
each path and its replacement). The same keys on any other op: unchanged.

### 3.4 Files to rewrite (P3.2)

A one-off script (not shipped) writes `floor = { from = "top", depth = d }`
for `depth = d` on the four op kinds only, in each tracked file:
`crates/rs_cam_core/tests/fixtures/{rest_cascade_g_restres,stock_change_cut_as_s6,stock_change_parity_s4}/project.toml`
(2 each), `planning/feed_modulation_calibration/bench_batch_2026-05-27/lead_in_test.toml`
(1), `planning/rivmap_block_job/nz_south.toml` (23) and `testpiece_mini.toml`
(4), and the CLI job file `fixtures/demo_job.toml` (2, NEW job key `floor`,
section 9). `planning/rivmap_block_job/testpiece.toml` (6) is the
operator's dirty file: **never commit it**; the lead tells the operator.
The Rust struct literals (156): the compiler finds each `depth:`. TOML text
in Rust tests: only `tests/every_project_table_refuses_an_unknown_key_f034.rs`
names a pocket; check it.

CAUTION: the other session has `tests/fixtures/*.toml` dirty now. P3.2
starts after the tech-debt W4 commit (PLAN status row).

---

## 4. Byte identity (question 3)

The default floor `Top { depth }` gives the same bits for every op that
does not pin a bottom:

- Ladder: today `final_z = top_z - depth.abs()` in `cutting_levels`;
  after, `floor_z = top_z - depth.abs()` (`BelowTop`) into the same
  `DepthStepping`. The same f64 operations in the same order;
  `finishing_passes` stays on Pocket and Profile. `ResolvedHeights.bottom_z`
  is the same value too.
- The 28 tracked ops with a pinned top keep `top - depth`, because `Top`
  is "below the resolved top". A default of `StockTop` would move all 28.
  **[LEAD] L-3.**
- Per-region code (P3.5, P3.6) runs only under `EachFace`.

Golden cases:

- G-OPGOLDEN: the 2D fixtures take default heights
  (`tests/common/op_fixtures.rs:431–448`). The ten cases of the four ops
  (`pocket/{fresh,rest,zigzag}`, `profile/{fresh,inside,rest}`,
  `zigzag/{fresh,rest}`, `adaptive/{fresh,rest}`) and the rest prior op
  (`flat_prior_op`, `op_fixtures.rs:263`) get `Floor::Top`. **No case
  moves.** P3.5 ADDS cases (section 5.7); a bless writes only the new keys.
- `tests/perf_golden_depth_level_geometry.rs` pins only retract and
  clearance (`:128–129`). No case moves.
- `tests/fixtures/gcode_golden/`: built by hand
  (`tests/common/gcode_fixtures.rs:582`), not from op configs. No case moves.

Output changes on purpose (state them in the commits): a pick of a down
face refuses (P3.4); a pinned Bottom Z on the four ops refuses at load
(P3.2; no tracked file has one); a `ModelTop`/`ModelBottom` reference on a
model with no mesh refuses (P3.1, L-4).

---

## 5. Per-item floors (question 4, S6)

### 5.1 Where the per-region floor comes from

`Regions` (`selection/view.rs:91`) gains `plane_z: Vec<Option<f64>>`
(NEW): for region `i`, the plane Z of its exterior item when that item is
a face, in the setup frame (the frame door has run). `Regions::plane_z(i)`
(NEW accessor). ONE NEW function in `compute/floor.rs`,
`region_floors(regions, floor, top_z) -> Result<Option<RegionFloors>, SelectionRefusal>`,
builds `floor_i = plane_z(i) - depth` under `Floor::EachFace { depth }`
and runs the checks below. The prologue (`compute/execute/prologue.rs`)
calls it, and `selection_status` (which holds the `tc`) calls the same
function after `view`. So status = generation (the P2.4 rule) holds, and
`view` keeps its selection-only arguments.

Refusals (NEW `RefusalCause` arms, `selection/refusal.rs:164`), each with
the item, all from `region_floors`:

- `NoFaceForFloor`: a region whose exterior is a loop, a circle or the
  stock ring, under `EachFace`. Fix: "pick faces only, or use another
  floor reference".
- `FloorAboveTop { floor_z, top_z }`: a region floor at or above the
  resolved top (a single floor refuses in `toolpath_heights`, 2.3).
- `FaceFacesDown`: section 6. It runs for the four floor ops under every
  floor variant, not in `view`: the boundary selection also runs `view`
  with `ItemUse::Regions` (P4.1) and keeps its down faces.

### 5.2 The ladder per region

`floor_ladder(top_z, floor_i, dpp, finishing_passes)` for each region.
`top_z` is the one resolved top of the op. A lower face inside the hole of
an upper face is its own region: it is cut from the top to its own floor.
The upper region holds that hole as an island down to its own floor. The
union clears both (DESIGN 7.3). DepthStepping redistributes the step
evenly (`ops/depth.rs:246`), so two regions with different floors have
different intermediate levels.

### 5.3 The runner (`run_layered`)

- `LayeredOp::ladder(&self, ctx)` becomes
  `ladder(&self, ctx, floor: Option<f64>)`: `None` → today's
  `effective_levels(ctx.cutting_levels, …)`; `Some(z)` →
  `floor_ladder(ctx.heights.top_z, z, self.depth_per_pass(), self.finishing_passes())`.
  `finishing_passes()` is a NEW hook (default 0; Pocket and Profile read
  their config). The spring passes stay (the else arm of `effective_levels`
  drops them, `layered.rs:643`).
- `ByArea`: each outline uses its own ladder. The retract rule
  (`layered.rs:409–434`) is per outline already.
- `Global`: the union of the ladders, top first. Each union entry is a
  pair (Z, repeat index), so the spring passes stay distinct. An outline
  emits at an entry only when its own ladder holds the pair. Compare by
  the index in the outline's own ladder, never by a float tolerance.
- `LevelInfo.final_z`: the outline's own final Z. Profile tabs stand on
  the floor of their own region.
- `area_outlines_less_obstacles` (`layered.rs:481`): group the outlines
  by floor; clip each group at its own lowest level; each clipped piece
  carries its group's ladder. One group (every non-EachFace op) is one
  call, as today.
- `prepare_outline` (`:548`) and the rest plan take the outline's ladder.
- `spans`: snap to the union ladder.

### 5.4 Adaptive 2D (`generate_adaptive`)

It is not a `LayeredOp`. P3.6 gives its outline loop the same rule: the
territory clip per floor group (`clearing_2d.rs:781`), the pieces of
`inset_piece_outlines` (`:794`) carry the floor of their region, and
`level_masks` takes the piece's ladder. Moving Adaptive onto
`run_layered` is larger (its annotation offsets and level masks) and is
not needed for S6. **[LEAD] L-5.**

### 5.5 The order

- `ByArea`: the `Regions` order (nested loops, then the faces in selection
  order: `view.rs:446`). Each region goes from the top to its own floor.
- `Global`: level first over the union ladder; at each level, the regions
  in `Regions` order.
- No sort by floor Z. A sort gives no measured gain, and the union is the
  same in every order. **[LEAD] L-7.**

**[OP] O-2.** Today a face pick on a Pocket cuts `depth` INTO the face
(the auto top follows it). S6 and Fusion read a picked face as the FLOOR.
(A, recommended) no switch: the floor row shows a hint (section 8.1).
(B) the first face pick on a Pocket, Zigzag or Adaptive 2D with the
default floor sets `EachFace { depth: 0 }` in the same undo step, with a
notice (the P4.D L-8 pattern); Profile keeps `Top`.

### 5.6 The frame

Every floor is a setup-frame (emission-frame) Z: the face plane Z after
`to_setup_frame`, the stock from `heights_stock_bbox`, the model from the
mesh in the setup frame, `Floor::Z` as typed (the frame of
`HeightMode::Manual`). The GUI shows the program Z beside a typed Z
(`SetupEvalContext::export_datum_shift`): a flipped setup is zero-rooted
at the stock bottom. **[LEAD] L-8.**

### 5.7 Proof fixtures

- A core lib sentry with hand-built face items (`Item::new`,
  `Shape::Face`): an upper face at −5 with a hole, a lower face at −10 in
  the hole. Both clear to their floors; every move lies inside the union.
- NEW G-OPGOLDEN cases `{pocket,zigzag,profile,adaptive}/each_face` on the
  same faces, and `pocket/each_face_flipped` on a Bottom setup (`Frame`,
  `op_fixtures.rs:276`).
- Viz (feature `step`): `fixtures/gui_step/stepped_block.step` (read by
  `tests/a_2d_op_cuts_a_step_face_p4_d.rs`), its up step faces as floors.

---

## 6. Up faces and down faces (question 5)

Facts: `FaceItem.normal` is signed (`item.rs:106`); `Facing` filters rules
only (`rule.rs:58`); `view` refuses only a face on its side
(`view.rs:296`); `regions_of` admits a down face (`view.rs:446`);
`face_top_z` reads every face (`resolve.rs:249`).

A down face has its material ABOVE its plane. The tool cannot stand on
it, and its plane is not the top of material. So:

1. `face_top_z` reads up faces only (`normal.z > 0`, setup frame).
2. A down face in the `Regions` of the four floor ops refuses
   `FaceFacesDown` (NEW, in `region_floors`, section 5.1): "this face points away from the tool in this
   setup. Pick it in a setup that faces it, or pick the floor face". The
   default rule never gives a face (`KindFilter::Served`).
3. The Selection counts and `selection_status` count faces by facing
   ("3 up faces, 1 down face").
4. A 3D boundary selection keeps down faces (an outline, no Z).

Only an explicit pick of a down face changes: it refuses where it cut
before. A refusal removes nothing from the part (the D2 precedent).
**[LEAD] L-9.**

---

## 7. The setup frame (question 6)

**The model-frame `face_top_z` defect is closed.** Generation reads
`selection.face_top_z()` on the `.setup` resolution, after the frame door
(`session/compute/generation.rs:193–200`, the call at `:197`; doc at `selection/resolve.rs:244–248`).
The boundary faces of a 3D op read the same way
(`session/selection_status.rs:427–441`, P4.1). The sentries are
`a_face_pick_cuts_from_its_setup_frame_top_p2_3` and
`a_3d_op_face_pick_sets_its_top_p4_1`.

**What is still live** is display parity (D-A, D-B). Three builders:
generation (`generation.rs:217–236`: mesh bbox, face top); MCP and
diagnostics (`session/compute/export.rs:306`: `LoadedModel::bbox`, no face
top); the GUI (`state/job.rs:215`, a copy of the MCP builder, then
`heights.resolve` at nine viz sites: `ui/properties/mod.rs:741`,
`tab_badges.rs:38`, `feeds_speeds.rs:708`, `operations/mod.rs:324`,
`height_diagram.rs:230`, `validate.rs:392`, `interaction/picking.rs:395`,
`app/gpu_upload.rs:1467`, `:1631`).

P3.1 replaces the three with ONE (NEW):

```rust
// session/heights.rs (NEW)
pub struct ToolpathHeights {
    pub context: HeightContext,       // with `floor`
    pub heights: ResolvedHeights,     // face top applied; bottom = floor for OpFloor ops
    pub top_from: TopSource,          // Auto(StockTop) | Auto(Faces) | Pinned(HeightMode)
    pub floor: Option<FloorReport>,   // the reference, its label, its Z, per-region floors
}
impl ProjectSession {
    /// THE heights of `tc` (a stored config or a GUI draft) in its setup frame.
    pub fn toolpath_heights(&self, tc: &ToolpathConfig) -> Result<ToolpathHeights, SessionError>;
}
```

- The model box: the mesh bbox in the setup frame (`cached_transform`,
  as generation does). A drawing has no mesh, so `ModelTop` and
  `ModelBottom` have no value. **[LEAD] L-4**: refuse a `ModelTop`/`ModelBottom`
  top or floor on an op whose model has no mesh (today: the stock in
  generation, Z 0 in the GUI). DESIGN 7.1 asked for this refusal.
- Generation calls it in `resolve_generation_inputs`; the face top and
  the floor leave `generation.rs`.
- `height_context_for_toolpath` and `height_context_from_session` are
  deleted. `HeightsConfig::resolve` becomes `pub(crate)`, so no viz site
  can resolve heights another way (compiler-enforced). The viz tests that
  call it move to `toolpath_heights` with a draft config.

---

## 8. The GUI (question 7, S5)

Tokens and components: `ui/tokens.rs`, `ui/components/` (`ValueRow`,
`ChoiceRow`, `ProvenanceBadge`, `StatusChip`, `OneLine`, `Banner`,
`SectionHeader`).

### 8.1 The floor row (op form, Geometry/Params tab)

It replaces the "Depth:" row of Pocket, Profile, Adaptive
(`ui/properties/operations/boundary_2d.rs:100`, `:211`, `:369`) and
Zigzag. It edits the draft `cfg.floor`; the panel writes the config back
through `replace_toolpath_config`, as every op row does.

```
 Floor:   [  6.00 ] mm below [ Top Z ▾ ]        = Z −6.00   ValueRow; ChoiceRow; TEXT_MUTED resolved Z
 Floor:   [  0.50 ] mm below [ Stock bottom ▾ ] = Z −12.50  (a through cut)
 Floor:   Z [ −20.00 ] mm    [ Absolute Z ▾ ]   program Z −20.00 (hover: the setup frame)
 Floor:   [  0.00 ] mm below [ Each picked face ▾ ]         ChoiceRow disabled with a reason when no face is picked
          F1 Z −5.00 · F2 Z −10.00               2 floors   OneLine TEXT_BODY; CountPill
 Depth/Pass: [ 1.50 ] mm                                     unchanged
```

The reference menu: Top Z, Stock top, Stock bottom, Model top, Each
picked face, Absolute Z. A disabled entry gives its reason ("no model
mesh", "no face in the selection"). A change of reference re-expresses
the value, so the resolved Z stays (**[LEAD] L-10**). The O-2 hint (option
A), when faces are picked and the floor is `Top`:

```
 ⓘ Faces set the top. To cut down to each face, choose Each picked face.   OneLine TEXT_MUTED, GLYPH_DETAIL
```

### 8.2 The Heights tab (`toolpath_panel.rs:1200`, `operations/mod.rs:311`)

Each row shows its reference and its source:

```
 HEIGHTS                                                         SectionHeader
 Clearance:  [ 10.00 ] mm from [ Stock Top ▾ ]   Z 15.00        unchanged rows
 Retract:    [  5.00 ] mm from [ Stock Top ▾ ]   Z 10.00
 Feed:       [  3.00 ] mm from [ Stock Top ▾ ]   Z  8.00
 Top:        [  0.00 ] mm from [ Stock Top ▾ ]   Z  0.00  ‹auto: faces›   ProvenanceBadge-like tag, TEXT_MUTED
 Floor:      6.00 below Top Z                    Z −6.00  ‹set in Pocket›  OneLine; no editor (one owner)
```

- The Top tag names its source: `auto: stock top`, `auto: faces` (D-A),
  `pinned`.
- An `OpFloor` op shows the read-only Floor line; the editor is in the op
  form only (one owner, IC1). **[LEAD] L-11.** `OpDial` and
  `HeightsBottom` ops keep today's Bottom row.

### 8.3 The height diagram (`operations/height_diagram.rs:222`)

```
  CZ 15.0 ─────────────────────────────────  DIAGRAM_INK
  RZ 10.0 ─────────────────────────────────  DIAGRAM_INK
  FZ  8.0 ─────────────────────────────────  OK
  TZ  0.0 ═══════════ stock ═══════════════  CAUTION   "TZ 0.0 ‹stock top›"
          ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  DIAGRAM_MATERIAL
  F1 −5.0 ─ ─ ─ ─ ─ ─                         DANGER dashed: one run per distinct floor
  F2 −10.0 ─ ─ ─ ─ ─ ─ ─ ─ ─                  DANGER dashed, "F2 −10.0 ‹face›"
  BZ −12.0 ─────────────────────────────────  stock bottom outline
```

- `EachFace`: one dashed run per distinct floor; the hover gives the
  face count at that Z (DESIGN 7.5). A single floor: one run
  "FL −6.0 ‹Top −6.00›" in `DANGER` (it drives the cut). "BZ (not used)"
  (R33) stays for `OpDial` ops only.
- A drag of the floor line writes `cfg.floor` in its current reference,
  on the 0.1 mm grid (`PIN_STEPS_PER_MM`, `height_diagram.rs:37`). The R5
  tie-break (`pick_line`) keeps the floor over the top.

### 8.4 The Selection section

The per-item rows (P4.4 `item_states`) show the floor Z of each face
region under `EachFace`; a refused item shows `NoFaceForFloor`,
`FaceFacesDown` or `FloorAboveTop` in the P4.3 refusal style.

---

## 9. MCP and CLI parity (question 8)

| Surface | Change |
|---|---|
| Core command | NEW `Command::SetToolpathFloor(SetToolpathFloorArgs { index, floor: Floor })`; registry row: `gui: Skip("the GUI inspector replaces the whole config through replace_toolpath_config")`, `mcp: Reached`, `cli: Reached` |
| W5 schema | `floor` is `#[param(driver_set = DRIVER_SET_FLOOR)]` (NEW reason const, the `DRIVER_SET_SELECTION` pattern, `operation_configs.rs:67`); `get_operation_schema` lists it as driver-set |
| MCP NEW tool | `set_toolpath_floor { index, floor }`; `SetToolpathFloorParam` in `rs_cam_mcp/src/server.rs`; the JSON of `floor` equals the file JSON (`{"from":"each_face","depth":0.0}`) |
| `set_toolpath_param depth` on the four ops | refused: no such param; the message names `set_toolpath_floor` |
| `set_toolpath_param floor` | refused with the driver-set reason |
| `set_toolpath_heights` `bottom_z` on the four ops | refused (`floor_conflict`); the doc of `SetToolpathHeightsParam.bottom_z` (`server.rs:526`) says so |
| `get_toolpath_params` | shows `floor` as an object |
| `list_toolpaths` / `project_summary` / diagnostics heights | NEW `floor: { from, label, z, regions: [{ item, z }] }` and `top_from`, from `toolpath_heights` |
| `selection_status` | `item_states` gains `floor_z` per face region (DESIGN 7.5 `floors`) |
| CLI job file | NEW job key `floor` in `OperationDef::JOB_KEYS` (`rs_cam_cli/src/job.rs:246`), applied by `SetToolpathFloor` after the params, as `selection` is |
| CLI sweep | a sweep over `depth` on the four ops is refused by the job check (`job.rs:268`). A sweep over the floor depth is a follow-up (needs a path syntax) **[LEAD] L-12** |
| `mcp_wire_surface_pin` | moves by design |

Two write routes, on purpose (the heights pattern, not the selection
pattern of P4 L-1): the GUI edits the draft and sends the whole config
(`replace_toolpath_config`); MCP and the CLI send `SetToolpathFloor`.
`driver_set` only makes `set_toolpath_param floor` refuse;
`every_op_conforms_w4.rs` check 6 (`:1218`) tests exactly that and gains
the `floor` row with no change. Both routes run `floor_conflict` and give
one config. **[LEAD] L-17.**

Parity tests (the P2.7 pattern, `app/mcp/commands/selection.rs`): the
MCP floor and a GUI draft write give the same config bytes and the same
`toolpath_heights`.

---

## 10. Items (question 9)

Gates per PLAN: the named sentries, `--lib <filter>`, the folder
sentries, `cargo test -p rs_cam_viz -q --lib`, `cargo test -p rs_cam_mcp -q`,
clippy, fmt, each through `timeout <s> scripts/cargo_lane.sh`; ask before a
run over 3 minutes. A red proof puts the defect back, sees red, restores.

**P3.1 — One heights resolver, in the setup frame** (no output change).
Files: NEW `session/heights.rs`; `generation.rs:193–236` calls it; delete
`export.rs:306` and viz `state/job.rs:215`; the nine viz sites (section
7); `HeightsConfig::resolve` → `pub(crate)`; the L-4 refusal;
`session/CLAUDE.md` (over 40 lines: this owner shortens it). Sentry NEW
`one_heights_resolver_for_every_surface_p3_1`: a face pick on a Pocket
(Top and Bottom setups) and a `ModelTop` top on an STL; generation,
`toolpath_heights` and the MCP diagnostics give one top and one bottom.
Red proof: the GUI builder without the face top. Keep green: G-OPGOLDEN,
`a_face_pick_cuts_from_its_setup_frame_top_p2_3`,
`a_3d_op_face_pick_sets_its_top_p4_1`, viz
`as001_pocket_heights_resolve_in_world_frame_for_identity_setup_f028`,
`the_heights_diagram_fits_f2`. Operator looks at: a STEP face pick on a
Pocket; the Top row shows the face Z and "auto: faces".

**P3.2 — The `Floor` type, the file break, the floor owner.** Files: NEW
`compute/floor.rs`; `HeightContext.floor`; `auto_bottom_z`;
`ops/depth.rs` `floor_ladder`; the four configs (`operation_configs.rs:515`,
`:578`, `:682`, `:1044`) and their `impl_operation_params!` rows
(`:3025–3088`); `DepthSemantics::Resolved`; `cutting_levels(&ResolvedHeights)`;
`FloorOwner` (24 registry rows); `floor_conflict` at each door (2.6);
`RETIRED_OP_ENTRIES`; the readers of 2.4; the rewrite of 3.4. Variants
`Top`, `StockTop`, `StockBottom`, `ModelTop`, `Z`; `EachFace` refuses
"not yet" until P3.5. Sentries:
- G-BOTTOMPIN rewritten, not re-blessed, as
  `the_floor_reaches_the_motion_g_bottompin`: arm 1, `Floor::Z { z: -14.0 }`
  moves the 6 mm pocket floor to −14; arm 2, a pinned Bottom Z on a
  Pocket refuses at `set_heights_config`, `replace_toolpath_config` and
  generation; arm 3, each `OpFloor` ladder ends at `bottom_z`; arm 4, the
  `HeightsBottom` list is the three ops by name; arm 5, a floor above the
  top refuses before any ladder runs (L-16).
- NEW `a_retired_floor_key_is_named_p3_2`: an old `depth` and a pinned
  `bottom_z` on a Pocket refuse with both paths named; `depth` on a Face
  loads. Red proof: delete the rows; serde says "missing field" and names
  no fix.
- NEW `the_default_floor_cuts_as_the_depth_did_p3_2`: each of the four
  ops, `Floor::Top` with a pinned and an auto top, gives the old ladder
  bit for bit. Red proof: resolve `Top` from the stock top; the
  pinned-top arm moves.
- Run once, unchanged: G-OPGOLDEN, `perf_golden_depth_level_geometry`.
  Moved to `FloorOwner`, green: `every_op_states_its_capabilities_w2_4`,
  viz `bottom_z_pin_note_g_bottompin`, `depth_beyond_stock_core_g_depthstockcore`,
  viz `depth_beyond_stock_cautions_g_depthstock`, `one_depth_caution_g_depthstockgui`.

Operator looks at: nothing new; the lead tells the operator about
`testpiece.toml`.

**P3.3 — Wire** (no output change). Files: `Command::SetToolpathFloor`,
its registry row, `DRIVER_SET_FLOOR`; MCP `set_toolpath_floor`,
`SetToolpathFloorParam` and the heights fields of section 9
(`rs_cam_mcp/src/server.rs`, `mcp_server.rs`, `app/mcp/commands.rs`); CLI
job key `floor` (`rs_cam_cli/src/job.rs:246`, applied after the params at
`:1183`). Sentries NEW: `mcp_and_gui_give_one_floor_p3_3` (an MCP write and
a GUI draft write give one config and one `toolpath_heights`);
`a_job_file_sets_a_floor_p3_3` (`rs_cam_cli`). Keep green:
`mcp_mutation_rows_reach_core`, `production_writes_go_through_apply_wp15a`;
`mcp_wire_surface_pin` moves by design. Red proof: route the tool past
`apply`; wp15a goes red.

**P3.4 — Up faces and down faces.** Files: `selection/resolve.rs:249`;
`FaceFacesDown` in `view`; counts by facing in `SelectionReport`
(`session/selection_status.rs:237`); the viz counts line. Sentry NEW
`a_down_face_gives_no_top_and_no_floor_p3_4`: an up face at 0, a down face
at −18; Top setup: `face_top_z` is 0 and a Pocket pick of the down face
refuses; Bottom setup: the same face is up and cuts. Red proof: drop the
facing test. `selection_refusals_name_the_item_p2_4` gains the arm.
Operator looks at: a pick of a STEP underside on a Top setup.

**P3.5 — Each face at its own floor: Pocket, Zigzag, Profile.** Files:
`selection/view.rs` (`Regions::plane_z`); `compute/execute/prologue.rs:128`
(`RegionFloors`); `compute/execute/layered.rs` (section 5.3);
`NoFaceForFloor`, `FloorAboveTop`; the L-15 top rule in
`toolpath_heights`. Sentries: NEW lib `each_face_cuts_to_its_own_floor_p3_5`
(section 5.7; both orderings; spring passes per region; Profile tabs on
each region's floor); red proof: one ladder for every region, the lower
face stops at −5. NEW G-OPGOLDEN cases (5.7); the old keys do not move.
Keep green: `layered_framework_owns_the_ladder_w3`. Operator looks at:
`stepped_block.step`, a Pocket on three up faces under "Each picked
face": three floor runs in the diagram, three floors in the simulation.

**P3.6 — Each face at its own floor: Adaptive 2D.** Files:
`clearing_2d.rs:773–830` (section 5.4). Sentries: the P3.5 lib sentry
gains the Adaptive arm; the `adaptive/each_face` golden case; red proof
as P3.5. Operator looks at: the same part with Adaptive 2D.

**P3.7 — GUI.** Files: the floor row (`operations/boundary_2d.rs`, the
Zigzag form); the Heights tab (`operations/mod.rs:311`;
`bottom_z_pin_note` matches `FloorOwner`); the diagram (8.3); the
selection rows (8.4). Sentries NEW: `every_depth_shows_its_reference_p3_7`
(each `OpFloor` op draws a floor row with its reference; the Top row
shows its source); `a_floor_reference_change_keeps_the_cut_p3_7` (L-10;
red proof: carry the old number into the new reference, the Z moves).
Keep green: `height_diagram_drag_prefers_bottom_and_rounds_r5`,
`the_heights_diagram_fits_f2`, `the_inspector_text_is_bounded_ic1`,
`panels_read_the_token_module_up1`, `egui_draw_sites_write_through_commands_wp6`.
Operator looks at: each reference on a Pocket; a through cut from the
stock bottom; an absolute Z on a flipped setup; the floor drag; the O-2
hint.

**P3.8 — Docs, catalog, parity closure.** `FEATURE_CATALOG.md`,
`selection/CLAUDE.md`, `compute/CLAUDE.md`, `session/CLAUDE.md`,
`ui/properties/CLAUDE.md`, the MCP tool docs, the `PLAN.md` status row;
each row of section 9 pinned by a named test.

---

## 11. Decisions (question 10)

### For the operator (each changes what is machined)

| # | Question | Recommendation |
|---|---|---|
| O-1 | An old project with `depth` or a pinned Bottom Z on the four ops: refuse with a named message, or convert `depth` to `Floor::Top`? | Refuse both (the D9 text and the 2026-09-16 rule). The conversion of `depth` would be output-identical, but it is an alias; a pin conversion would cut deeper. Only `testpiece.toml` of the operator's own files needs a hand edit |
| O-2 | A first face pick under the default floor: switch to "Each picked face" (B), or keep the floor and show a hint (A)? | A: nothing cuts differently until the user chooses |

### For the lead (structure, UI, wire)

| # | Question | Recommendation |
|---|---|---|
| L-1 | Delete `bottom_z` from `HeightsConfig` for every op now? | No: refuse it on the four ops (one check at every door); a follow-up can move all floors into op configs |
| L-2 | `HeightMode::FromFace` top in P3 (P4 O-1 B)? | No (section 2.7) |
| L-3 | The default reference | `Top` (below the resolved top): the 28 pinned-top files keep their cut |
| L-4 | `ModelTop`/`ModelBottom` on a model with no mesh | Refuse (one resolver; DESIGN 7.1) |
| L-5 | Adaptive 2D onto `run_layered`? | No: per-region ladders in its own loop |
| L-6 | Suggest snap for a non-`Top` floor | No snap (`total_depth()` = `None`) |
| L-7 | Region order with several floors | The `Regions` order; no sort |
| L-8 | The frame of `Floor::Z` | The setup (emission) frame, as `Manual`; the GUI shows the program Z |
| L-9 | A down face as a floor-op region | Refuse `FaceFacesDown` (the safe direction) |
| L-10 | A change of reference in the GUI | Re-express the value so the resolved Z stays |
| L-11 | The owner of the floor editor | The op form; the Heights tab shows a read-only Floor line |
| L-12 | CLI sweep over a floor depth | A follow-up (needs a nested path) |
| L-13 | `StockBottom` in the enum (not in R4's list) | Yes: the sheet-parts job cuts "through + skim" (REQUIREMENTS job table) |
| L-14 | A single named face as a floor (`Face { key }`) | Not in P3: `EachFace` meets S6 and R4; a DXF pocket to a STEP face level can use `Z`. Add later if a job needs it |
| L-15 | The auto top under `EachFace` | The stock top (not the face top): else the depth is 0 |
| L-16 | The else arm of `effective_levels` | Delete it; an empty ladder refuses (an inverted floor must never step below the top) |
| L-17 | The GUI write route for `floor` | The draft and `replace_toolpath_config` (as heights); MCP/CLI through `SetToolpathFloor` |

---

## 12. Risks

- **A floor face with a fillet or chamfer at its edge** is smaller than
  the wall opening. The region is the face outline, so the cut leaves the
  fillet band up the wall. P3 does not detect it; the P3.5 look should
  include such a part. P8 can use the wall silhouette.
- **The file break touches the operator's job files** (`nz_south.toml`,
  `testpiece*.toml`). The lead tells the operator first.
- **`HeightsConfig::resolve` to `pub(crate)`** breaks many viz tests. If
  that is too wide for P3.1, the fallback is a source-scan sentry (weaker).
- **The `Global` union ladder:** compare by (Z, repeat index), never by a
  float tolerance, or a spring pass merges or doubles.
- **The per-floor obstacle clip** must be one call for one group, or the
  K3 golden of the area ops moves.
- **The other session** edits `project_file.rs`, `save.rs`, `mod.rs`. P3.2
  waits for its W4 commit, then puts `RETIRED_OP_ENTRIES` on the file as
  it is.
