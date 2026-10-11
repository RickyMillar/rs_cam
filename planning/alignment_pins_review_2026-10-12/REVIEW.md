# Alignment pins: review and suggested improvements (2026-10-12)

Scope: the alignment-pin (dowel) feature for two-sided jobs. This is a
review. No source file changed. No cargo command ran. The keying numbers
below come from a short Python model of the validator
(`reseat_mismatch_mm` and its three maps), not from a cargo run.

Operator request (verbatim): "the ux is a little clunky at the moment! ITs
hard to see whats going on. id also like existing pins to be ajustable as a
set, not individually."

## 0. Top findings

1. **The keying model is wrong for parts that sit on dowels.**
   `validate_pins_for_flip` tests only the four rectangle seatings about
   the STOCK centre. A part on spoilboard dowels has no edge reference. The
   dowels locate it. Two equal pins always seat after a half-turn about
   their own midpoint. Thus no two-pin pattern with equal diameters keys,
   and "Auto-place keyed pair" makes a pattern that the check calls keyed
   but that is not keyed.
2. **The lead's 3-pin pattern is correct.** One pin on the flip axis plus a
   mirror pair off the axis seats under the flip. It keys under both
   models, unless the three pins are collinear or form an equilateral
   triangle. The current validator passes it (seat 0.0 mm, margin 22.6 mm
   on the 65 x 65 example). The validator also passes the collinear and
   the equilateral cases, which do NOT key.
3. **The validator ignores `z_rotation`.** A `bottom` + `180` setup mirrors
   X, not Y. The check still judges the Y mirror.
4. **The GUI and MCP differ.** MCP `add_alignment_pin` does not write
   `flip_axis` and does not create the "Pin Drill" toolpath. Thus the
   viewport draws no flip axis for pins that MCP adds.
5. **The live example drills its dowel holes from a DXF, not from the
   stock pins.** The two copies of the hole positions must change
   together. Nothing checks that they agree.
6. **UX:** the viewport shows only green 16-segment circles and a dashed
   line. It shows no pin numbers, no status colour, no clear strips, no
   wrong-orientation ghost, and no link from the warning to a fix.

## 1. Code map

### Core model and checks

| What | Where |
|---|---|
| `FlipAxis` (a cache, "derived, not authored") | `crates/rs_cam_core/src/compute/alignment_pins.rs:27-61` |
| `AlignmentPin { x, y, diameter }`, stock-local | `alignment_pins.rs:63-75` |
| `PIN_WALL_MM` 2.0, `PIN_KEYING_ASYMMETRY_MM` 3.0, `PIN_MATCH_TOL_MM` 0.5 | `alignment_pins.rs:95,103,108` |
| `PinPlacementRequest` (X strips only) | `alignment_pins.rs:118-139` |
| `PinFlipReport`, `seats`, `keyed`, `warnings` | `alignment_pins.rs:237-313` |
| `reseat_mismatch_mm` (max over pins of the nearest-pin distance) | `alignment_pins.rs:322-338` |
| `place_keyed_pins` (two pins on y = D/2, slid in X) | `alignment_pins.rs:360-461` |
| `validate_pins_for_flip` (My seat, Mx and R180 keying) | `alignment_pins.rs:467-497` |
| `StockConfig.alignment_pins`, `StockConfig.flip_axis` | `crates/rs_cam_core/src/compute/stock_config.rs:81,99` |
| `plan_keyed_pins` (world bbox to stock-local X range) | `stock_config.rs:185-202` |
| `StockConfig::validate_pins_for_flip(face_up)` | `stock_config.rs:209-211` |
| `FaceUp::Bottom` = `(x, D-y, H-z)`; `ZRotation` applies after it | `crates/rs_cam_core/src/compute/transform.rs:116,228-233`; `session/mod.rs:1997-2013` |
| `XYDatum::AlignmentPins` (a label only) | `crates/rs_cam_core/src/session/mod.rs:743-786` |
| `pin_drill_tool_diameter` (first pin-drill tool, else `tools[0]`) | `session/mod.rs:1762-1775` |

### Session commands (`ProjectSession::apply`)

| Command | Row | Setter |
|---|---|---|
| `AddAlignmentPin` (dedupe 0.01 mm, `drop_all_results`) | `session/command.rs:156,1197,2406` | `session/mutation/entities.rs:605-625` |
| `RemoveAlignmentPin` | `command.rs:166,1208,2415` | `entities.rs:629-640` |
| `SetAlignmentPinDrillHoles` | `command.rs:572,1756,2506` | `entities.rs:267-290` |
| `SetStockConfig` (the GUI uses it for every pin edit) | `command.rs:367,2471` | — |

No command moves, edits or replaces pins. The GUI stock panel edits a draft
`StockConfig` and applies the whole stock (`ui/properties/panel_apply.rs:109-141`).

### Generation, drilling, simulation, export

- The generation pass copies the live stock pins into `cfg.holes`:
  `session/compute/generation.rs:362-368`.
- `pin_holes_in_emission_frame` adds the stock origin; for a non-identity
  setup the origin is (0, 0): `compute/execute/drilling.rs:196-222`.
  `generate_alignment_pin_drill`: `drilling.rs:495-532`. Depth = stock Z +
  `spoilboard_penetration`.
- The simulation treats `AlignmentPinDrill` as a drill: `compute/simulate.rs:271`.
- G-code: `gcode/mod.rs:188` (drill cycle only). The datum note at
  `gcode/mod.rs:288-300` says XY stays stock-relative over the flip because
  "the operator flips the part against the same pins".
- The per-setup file header (`crates/rs_cam_viz/src/io/export.rs:690-720`)
  and the setup sheet (`io/setup_sheet.rs:262-271`) give the flip
  instruction. Neither one names the pins, their positions or a way to
  check the orientation.

### Diagnostics and load warnings

- Load warning (core): `session/project_file.rs:1257-1268`.
- The same check again on the GUI load path: `crates/rs_cam_viz/src/controller/io.rs:476-500`.
- Project diagnostic `AlignmentPinsUnkeyed` with a fix hint:
  `session/compute/diagnostics.rs:1052-1085`.
- Pin-drill precondition: `diagnostics/adapters/from_preconditions.rs:111-118`.

### GUI

- Stock panel, section "Alignment Pins": `crates/rs_cam_viz/src/ui/properties/stock.rs:214-516`.
  `has_flipped_setup` is `any(face_up != Top)`: `ui/properties/mod.rs:902-906`.
- `keyed_pin_plan`, `KeyedPinInputs` (union bbox of all models), `mirror_pin`:
  `stock.rs:552-629`.
- "Two-sided setup" and "Add alignment pins for this flip":
  `controller/events/model.rs:408-507`, `ui/properties/setup.rs:103-118`.
- `sync_alignment_pin_drill` (creates "Pin Drill" in setup index 0):
  `controller/events/model.rs:840-1008`; trigger: `controller.rs:462-473`.
- Viewport: pins and flip axis line: `app/gpu_upload.rs:709-764`.
  Picking: `interaction/picking.rs:232-258`. A pin click selects the
  stock: `app/viewport.rs:174-177`.
- Overlay rows: `ui/overlays/registry.rs:644-683`.
- Setup card and setup panel text: `ui/setup_panel.rs:478-497`, `ui/properties/setup.rs:240-255`.

### MCP and CLI

- Tools `add_alignment_pin`, `remove_alignment_pin`: `crates/rs_cam_viz/src/mcp_server.rs:719-745`;
  params `crates/rs_cam_mcp/src/server.rs:1166-1182`; plan and reply
  `app/mcp/commands.rs:322-354,1919-1972`.
- `inspect_stock` lists the pins and the cached `flip_axis`. It gives no
  seat or keying numbers: `app/mcp/project.rs:450-475`.
- `set_stock_config` has no pin field: `rs_cam_mcp/src/server.rs:762-795`.
- CLI: no pin authoring. It only reads the project file. `rs_cam_cli/src/job.rs:1347`
  treats the op as a drill.

## 2. Correctness

### C1. The validator models an edge-referenced part, not a part on dowels (HIGH)

`alignment_pins.rs:79-91` says "a rectangular blank drops back onto its
dowels four ways: identity, My, Mx, R180". That is true only when the stock
OUTLINE also locates the part (a fence or a corner stop). In this workflow
the dowels stand in the spoilboard and the holes in the part locate it. The
part can take any rigid placement that puts every hole on a dowel.

The correct question is: does the hole pattern have a rotation symmetry of
its own? With the flipped part face down, a wrong seating is the correct
seating times a proper (rotation) symmetry of the pattern.

- Two equal pins: a half-turn about their midpoint swaps them. They are
  never keyed. The 3 mm offset only moves the wrongly seated part by 3 mm
  in X. The doc at `alignment_pins.rs:100-102` ("the hole simply is not
  there") is false: the hole is there; the part is 3 mm off.
- Python check of the current validator against the intrinsic model:

| Pattern (65 x 65 stock) | current seat / margin | intrinsic rotation margin | keyed in fact |
|---|---|---|---|
| live mini: (6, 32.5), (59, 32.5) | 0.0 / 0.0 | 0.0 | no |
| auto-place style: (7.5, 32.5), (60.5, 32.5) | 0.0 / 3.0 | 0.0 | **no (validator says yes)** |
| axis + mirror pair: (55, 32.5), (8, 10), (8, 55) | 0.0 / 22.6 | 6.9 | yes |
| collinear: (8, 10), (8, 32.5), (8, 55) | 0.0 / 49.0 | 0.0 | **no (validator says yes)** |
| equilateral axis + pair | 0.0 / 24.6 | 0.0 | **no (validator says yes)** |

The "intrinsic rotation margin" column comes from the candidate-rotation
method that P1 proposes, in a simplified form. It is the same instrument,
not an independent check. The 0.0 rows follow from geometry alone (a
half-turn about the midpoint, or about the middle pin, or a 120 deg turn).

No pin pattern can detect "the operator forgot to flip". A pattern that
seats under the flip has a mirror symmetry, so the unflipped part also
seats on the same dowels.

Note: for a pattern that seats (My-invariant), Mx = R180 o My, so
`mirror_x_mismatch_mm` and `rot180_mismatch_mm` are the same number. The
report shows one quantity twice.

The validator also ignores `diameter`. Two pins of 6 mm and 8 mm key (the
8 mm dowel does not enter a 6 mm hole), but the validator reads them as a
symmetric pair.

Fix: add an intrinsic check. Rotate about the centroid. The candidate
angles are those that take the pin farthest from the centroid onto each
pin at the same radius and with a compatible diameter. This enumeration is
complete for the rotation symmetries of a finite set. The margin is the
smallest worst-pin miss over the candidates. A bigger pin into a smaller
hole is a block. Keep the stock-centre test as a second check only when a
fence references the outline. Rewrite the warning text.

### C2. "An odd pin count buys nothing" is false (MEDIUM)

`alignment_pins.rs:357-359`, `stock.rs:416-424` and the "+ Add Pin" hover
text (`stock.rs:459-460`) say extra pins never key. That is true only for
pins on the axis. A mirror pair off the axis plus one axis pin is the
smallest equal-diameter pattern that keys. The GUI has no way to make it
except with free drags.

### C3. The validator ignores `z_rotation` (HIGH)

The setup transform is `FaceUp` then `ZRotation` (`session/mod.rs:1997-2013`).
`Bottom` + `Deg180` maps (x, y) to (W - x, y). That is the X mirror that the
`FlipAxis::Vertical` doc calls unreachable (`alignment_pins.rs:33-37`).
`Bottom` + `Deg90/270` maps onto a different footprint unless W = D. Every
caller passes only `setup.face_up`:

- `session/compute/diagnostics.rs:1068`
- `session/project_file.rs:1263`
- `controller/io.rs:490`
- `controller/events/model.rs:426-432,497`
- the stock panel `stock.rs:498-503`

The MCP tool `set_setup_rotation` reaches this state. Fix: pass the full
in-plane map of the setup (face_up and z_rotation) to the validator. Refuse
90/270 on non-square stock as "not modelled".

### C4. Pins have no check against the part, keep-outs or fixtures (MEDIUM)

`validate_pins_for_flip` checks only the stock edge, with zero wall
(`alignment_pins.rs:474-482`; `PIN_WALL_MM` is not used there). Only the
auto-placer checks the model, and only in X (`model_x_range`,
`alignment_pins.rs:130`). A mirror pair off the axis needs the full 2D
model footprint, the setup keep-outs (`SetupData.keep_out_zones`) and the
fixtures. A hand-placed pin can go through the part with no warning.

### C5. Auto-place on the live example (MEDIUM)

- `KeyedPinInputs::from_session` uses `models_union_bbox`
  (`stock.rs:561-568`, `session/mod.rs:1743`). `LoadedModel::bbox`
  includes DXF polygons (`session/mod.rs:502-522`). The mini project loads
  `stock_outline.dxf` as a model at 0..65 x 0..65. Both clear strips are
  0 mm, so "Auto-place keyed pair" is disabled with `StripTooNarrow`.
- No `AlignmentPinDrill` op exists, so `pin_drill_tool_diameter` falls back
  to `tools[0]`, the 3 mm end mill (`session/mod.rs:1762-1775`). If
  auto-place ran, it would replace the 6 mm pins with 3 mm pins
  (`stock.rs:441-442` clears and extends).

Fix: let the operator choose the keep-out source (model list, not "all
models"). Choose the pin tool by type (drill) and diameter, not `tools[0]`.
Warn when the pin diameter differs from the tool that drills the holes.

### C6. Two sources of hole positions in the live example (MEDIUM)

`planning/rivmap_block_job/testpiece_mini.toml` drills the dowels with a
plain `drill` op (`toml:628`, id 4) on `dowel_through.dxf` (two circles at
(6, 32.5) and (59, 32.5)). A disabled helix pocket also exists (`toml:361`).
The stock pins (`toml:22-30`) are a second copy. If the operator edits pins
in the GUI, `sync_alignment_pin_drill` adds a "Pin Drill" op in setup
index 0 as well. Then the machine drills the holes twice with two ops.
For the 3-pin plan, the lead must change BOTH the DXF and the stock pins,
or delete the DXF drill and use the pin-drill op. The recommendation is the
pin-drill op: one source of truth.

### C7. Pin-drill placement assumes setup index 0 is the top setup (LOW)

`sync_alignment_pin_drill` puts the op in setup index 0
(`controller/events/model.rs:858-862`). It does not check that this setup
is `Top`. For a pattern that seats (My-invariant set), drilling in a
flipped setup gives the same SET of holes. The result is correct only
because of that symmetry. It is not correct for a pattern that does not
seat. Put it in the first `Top` setup, or refuse.

### C8. Frame defects in the GUI (LOW)

- Picking uses `(pin.x, pin.y, stock.z)` with no origin and no setup
  transform (`picking.rs:239-243`). The drawing uses origin + the full
  setup transform (`gpu_upload.rs:722-723`). A pick misses on a flipped
  setup or on a stock with origin not at 0.
- The flip axis line reads the `flip_axis` CACHE (`gpu_upload.rs:731`,
  overlay precondition `registry.rs:675`). The panel derives the axis from
  the setups (`stock.rs:270`). A stale or `None` cache draws no line or the
  wrong line.
- `has_flipped_setup` is true for edge-up setups (`ui/properties/mod.rs:906`).
  The panel then says "Horizontal" and judges against `Bottom`, while the
  core diagnostic for a `Front` setup says nothing about keying.
- In a flipped setup the pin circle is drawn at the transformed stock top
  `+0.1` mm (`gpu_upload.rs:722`). `Bottom` maps that Z to the underside.
  Check on screen whether the circles hide under the stock.

### C9. GUI / MCP parity and duplicated logic (MEDIUM)

- MCP `add_alignment_pin` does not write `flip_axis` and does not run
  `sync_alignment_pin_drill` (`app/mcp/commands.rs:1919-1948`). The GUI
  does both (`controller/events/model.rs:473,506`; `controller.rs:468`).
- The GUI load path re-runs the core load check (`controller/io.rs:476-500`
  vs `project_file.rs:1257-1268`). The dedupe hides the copy.
- `mirror_pin` re-implements the flip map (`stock.rs:624-629`) instead of
  `FaceUp::transform_point`. It also keeps a `Vertical` arm that no setup
  makes.
- Pin changes in the GUI go through `SetStockConfig`; MCP goes through
  `AddAlignmentPin`. Both drop all results, but the replies differ: the
  MCP reply reports no stale set (`commands.rs:1950-1955` says so).

## 3. UX: what the operator sees today

Step by step, on the mini example:

1. Load. A modal lists: "Alignment pins do not key the flip: the pattern is
   only 0.0 mm away from seating in the wrong orientation (need 3.0 mm).
   The part can be dropped on 180 deg out." It gives no pin numbers and no
   action.
2. The Setup workspace. The viewport shows two green circles and a dashed
   amber line on y = 32.5, both drawn on the stock top. The circles have
   no labels. Their colour does not change with the status.
3. A click on a pin selects the whole stock (`viewport.rs:174-177`). Nothing
   on the panel shows which pin was clicked.
4. The Stock tab, section "Alignment Pins" (`stock.rs:221-513`): a flip-axis
   label, a "Pin diameter" drag (for all pins), then one row per pin with X
   and Y drags (range 0..stock, no wall clamp), "Mirror" and "Remove".
   Then "Auto-place keyed pair" (disabled here, with a red refusal) and
   "+ Add Pin" (adds one pin at the centre). Then the amber warning again.
5. The fix hint in diagnostics says "Re-place the pins so the pair is
   asymmetric under the mirror the flip is NOT, or let the stock panel place
   them" (`diagnostics.rs:1077-1080`). On this project the stock panel
   refuses to place them, and the advice gives a pattern that still does
   not key (C1).

What is hard to see:

- The relation of the pins to the part. The viewport does not draw the
  model footprint, the clear strips or the keep-out band that the placer
  uses.
- Why the warning fires. Nothing draws the wrong seating. A red "ghost" of
  the pins after the wrong placement would show at once that the ghost
  falls on the real pins.
- Which pin is which. No numbers in the viewport, no hover, no selection.
- The margins. The panel gives no number for the wall to the edge, the
  wall to the part, or the keying margin while the operator drags.
- The flip axis. The line comes from a cache, and the panel text about a
  "derived" and a "stored" axis (`stock.rs:270-311`) explains an internal
  detail, not the job.
- What to do. No warning has a button. The operator must drag single
  coordinates and read the warning again.

## 4. Set editing: design

### Data model

Keep `Vec<AlignmentPin>` as the stored truth. The project file, the drill
snapshot (`generation.rs:362`), the validator and the export all read it.
Do not store a pattern with parameters. A stored pattern would need a
second truth for hand edits. Instead, add pure pattern functions in core
that read a pin set and write a new pin set:

```rust
// crates/rs_cam_core/src/compute/alignment_pins/pattern.rs (new)
pub fn translate(pins: &[AlignmentPin], dx: f64, dy: f64) -> Vec<AlignmentPin>;
pub fn set_diameter(pins: &[AlignmentPin], d: f64) -> Vec<AlignmentPin>;
pub fn snap_to_flip(pins: &[AlignmentPin], map: InPlaneFlip, tol: f64) -> Vec<AlignmentPin>;
pub struct AxisPairPattern { pub axis_x: f64, pub pair_x: f64, pub pair_offset_y: f64, pub diameter: f64 }
pub fn axis_plus_mirror_pair(stock_d: f64, p: AxisPairPattern) -> [AlignmentPin; 3];
pub fn fit_axis_plus_mirror_pair(pins: &[AlignmentPin], stock_d: f64) -> Option<AxisPairPattern>;
```

`fit_*` reads the parameters back from a stored set. The GUI shows the
pattern controls only when the stored set matches a preset within
`PIN_MATCH_TOL_MM`. Otherwise it shows "custom" and the set controls only
(move, diameter, snap).

Presets:

- `axis_plus_mirror_pair` (default; keys with equal diameters).
- `two_mixed_diameters` (two pins on the axis, one larger; keys with only
  two holes, needs two drills).
- `two_on_axis` stays available but shows "not keyed: the part seats
  180 deg out" in red.

### Command

One command: `SetAlignmentPins { pins: Vec<AlignmentPin> }`. It replaces the
whole set in one undo step and one stale set. The GUI, MCP and the pattern
functions all end in it. Keep `AddAlignmentPin` and `RemoveAlignmentPin`
as thin forms, or delete them (operator ruling 2026-09-16 allows breaks).
The setter writes no `flip_axis` (see P1). Decide one blast radius:
`add_alignment_pin` uses `drop_all_results` (`entities.rs:621`). A pin edit
needs only the pin-drill op and the ops whose boundary follows the stock to
go stale. The lead must rule on this.

### GUI controls (stock panel, section "Alignment pins")

1. Preset picker: "3 pins: axis + mirror pair" / "2 pins, mixed diameter" /
   "Custom".
2. For the 3-pin preset: "Axis pin X", "Pair X", "Pair distance from axis",
   "Diameter". For any set: "Move all dX / dY" (nudge buttons of 0.5 mm and
   1 mm), "Snap to flip axis".
3. A read-only table of the derived pins (number, X, Y, diameter, wall to
   edge, wall to part). The per-pin drags move into an "Edit single pins"
   fold.
4. Live status chips, from the ONE core report:
   - "Seats after flip: yes / misses by N mm"
   - "Keyed: yes, margin N mm / NO: seats 180 deg out"
   - "Wall to stock edge: min N mm (need 2.0)"
   - "Wall to part: min N mm" and "Wall to keep-outs: min N mm"
5. The warning gets action buttons: "Use 3-pin preset", "Move the pair off
   the axis".

### Viewport

- Pin numbers beside each circle. Colour by status: green when all checks
  pass, amber for a wall below `PIN_WALL_MM`, red for a pin that does not
  seat.
- The wrong-orientation ghost: draw the pins after the best wrong
  placement as red dashed circles. When a ghost falls on a real pin, the
  pattern does not key.
- The flip axis from the setups (not the cache).
- The clear area: shade the model footprint, the keep-outs and the
  `PIN_WALL_MM` band at the stock edge.
- A drag handle on the pattern (move all) in the Setup workspace. Picking
  must use the same transform as the drawing (C8).

### MCP

- `set_alignment_pins { pins: [...] }` (replaces the set).
- `place_alignment_pattern { preset, axis_x?, pair_x?, pair_offset_y?, diameter? }`.
- `move_alignment_pins { dx, dy }`.
- `inspect_stock` adds the `PinFlipReport` numbers: seats, seat miss, keyed,
  margin, walls, the setup it judged.

## 5. Suggested improvements, ranked, as packages

Each package is one agent task. Correctness comes first.

| # | Package | Problem | Change | Files | Size | Test |
|---|---|---|---|---|---|---|
| P1 | Intrinsic keying check | C1, C2: two-pin patterns read as keyed; collinear and equilateral 3-pin sets read as keyed; diameters ignored | Add the rotation-symmetry check about the centroid, with diameters. `keyed()` uses it. Rewrite the docs at `alignment_pins.rs:79-103,357-359`, the warning text and the fix hint. Mark the two-pin auto-place as not keyed, or replace it with the 3-pin preset (P4) | `compute/alignment_pins.rs`, `session/compute/diagnostics.rs:1077`, `ui/properties/stock.rs:419-460` | M | New `tests/alignment_pin_keying_intrinsic_*.rs`: the five cases in the C1 table, a 6/8 mm pair, a 4-pin rectangle (not keyed). These tests assert the old model and go red: `placed_pins_seat_under_my_and_block_mx_and_r180`, `refuses_when_there_is_no_room_left_to_key`, `unequal_strips_are_already_keyed`, `keyed_pins_raise_no_diagnostic_g_pinauto` (`alignment_pin_keying_g_pinauto.rs:223,313,330,557`) and `panel_plan_seats_the_flip_and_keys_it` (`stock.rs:842`) |
| P2 | Full setup map in the validator | C3: `z_rotation` ignored | `validate_pins_for_flip` takes the setup's in-plane map (face_up + z_rotation). Refuse 90/270 on non-square stock. Update the five callers | `alignment_pins.rs`, `stock_config.rs:209`, `diagnostics.rs:1068`, `project_file.rs:1263`, `controller/io.rs:490`, `events/model.rs:426-497`, `stock.rs:498` | M | Sentry: `bottom` + `180` with Y-mirror pins warns "does not seat"; X-mirror pins seat |
| P3 | Derive the flip axis; delete the cache | C8, C9: line from a stale cache; MCP never writes it | Delete `StockConfig.flip_axis` and `FlipAxis::Vertical`. Viewport, overlay precondition and panel derive the map from the setups (one core fn). Drop the "stored vs derived" panel text | `stock_config.rs:82-99`, `project_file.rs:281`, `save.rs:154`, `gpu_upload.rs:731-764`, `registry.rs:665-683`, `stock.rs:247-311`, `events/model.rs:462-477`, `app/mcp/project.rs:474` | S | `overlays_registry`; a load test that an old file with `flip_axis = "vertical"` loads (key ignored) |
| P4 | Core pattern module + `SetAlignmentPins` | Set editing does not exist | Add `alignment_pins/pattern.rs` (section 4). Add the command row and setter. Route GUI and MCP through it | `compute/alignment_pins.rs` (split to a folder), `session/command.rs`, `session/mutation/entities.rs` | M | Unit tests per function; `command_registry_completeness`; `mutation_paths_invalidate_alike_p0`; round trip `fit(axis_plus_mirror_pair(p)) == p` |
| P5 | MCP parity and tools | C9: MCP adds no pin-drill op and no axis; no set tools; no report | MCP pin edits run the same pin-drill sync as the GUI (move `sync_alignment_pin_drill` behind one controller door). Add `set_alignment_pins`, `place_alignment_pattern`, `move_alignment_pins`. `inspect_stock` reports the `PinFlipReport` | `app/mcp/commands.rs`, `mcp_server.rs`, `rs_cam_mcp/src/server.rs`, `app/mcp/project.rs`, `controller/events/model.rs:840` | S-M | `mcp_core_arm_describes_every_row`, `mcp_wire_surface_pin`; a new test that MCP add and GUI add leave the same toolpath list |
| P6 | Placer inputs and walls | C4, C5: X-only walls; union bbox includes the stock outline; tool = `tools[0]` | Walls in 2D against the chosen models, keep-outs and fixtures. The validator reports walls (edge, part, keep-out). Pin tool: first drill-type tool whose diameter fits; warn on a diameter mismatch | `alignment_pins.rs:118-139,474-482`, `stock.rs:552-568`, `session/mod.rs:1762` | M | `keyed_pin_panel_reads_the_real_strips_g_pinpanel` extended: a stock-outline model does not block placement; mini project picks the 6 mm drill |
| P7 | GUI pattern editor and viewport | Section 3 | Preset picker, set controls, derived table, status chips, warning buttons; viewport numbers, status colours, wrong-orientation ghost, clear-area shading, move handle; picking in the drawing frame | `ui/properties/stock.rs`, `app/gpu_upload.rs`, `interaction/picking.rs`, `app/viewport.rs:174`, `ui/overlays/registry.rs` (a ghost row) | L | egui harness test of the chips on the mini project; `overlays_registry`; a pick test on a flipped setup with origin not at 0; a GUI screenshot look |
| P8 | One source of hole positions | C6, C7 | Put the pin-drill op in the first `Top` setup. Warn when another drill op drills a hole within `PIN_MATCH_TOL_MM` of a stock pin (double drilling) or when a DXF dowel layer disagrees with the stock pins | `controller/events/model.rs:858`, a new static check in `diagnostics/adapters/from_static_checks.rs` | S | Sentry on the mini project: pins + the DXF drill raise "drilled twice"; pins moved and DXF not moved raise "disagree" |
| P9 | Clean-ups | C8, C9 | Delete the GUI load re-check (`controller/io.rs:476-500`); delete `mirror_pin` (use the core flip map); `has_flipped_setup` only for an in-plane flip | `controller/io.rs`, `stock.rs:624-629`, `ui/properties/mod.rs:902-906` | S | Existing load-warning test still sees exactly one keying line |
| P10 | Pins in the export and the setup sheet | Operator has no pin data at the machine | The per-setup header and the setup sheet list each pin (number, X, Y, diameter) and one orientation cue (for example "pin 1 is the axis pin, on the left"). The flipped file header says "seat on pins 1-3" | `io/export.rs:690-760`, `io/setup_sheet.rs:262` | S | `export_parity_core_vs_gui_p0`; a header text test |

Order: P1, P2 (correctness) -> P3, P9 (small, remove traps) -> P4 -> P5,
P6 -> P8 -> P7 -> P10. P7 depends on P1, P3, P4 and P6.

## 6. For the lead: the 3-pin plan on the mini piece

- The plan is sound under both models. Avoid collinear pins (axis pin X
  equal to pair X) and an equilateral triangle. A pair X far from the axis
  pin X gives a large margin.
- Example on 65 x 65 with 6 mm pins and a 2 mm wall: the pin centre must
  be at least 5 mm from each edge. Check the part and cavity footprints
  first; the current code does not (C4).
- Change the dowel DXF and the stock pins together, or switch to the
  pin-drill op and delete the DXF drill (C6). Do not let the GUI add a
  second "Pin Drill" op beside the DXF drill.
- Keep setup 3 at `z_rotation = "0"`. A rotation of 180 changes the flip
  map, and the validator does not see it (C3).
