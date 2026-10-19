# Findings: primitive obsession and missing newtypes (`primitive-newtype`)

Baseline 99b6fb41, branch `geo-select`. Read-only sweep with `rg` and file
reads. No cargo run. Counts are production code (test files and `tests.rs`
excluded) unless a line says otherwise.

Order: tier, then cost. "In flux" means that the primary site is in
`instruments/in_flux_paths.txt`.

| # | Title | Tier | Cost | In flux |
|---|---|---|---|---|
| 1 | One `P3` type for three coordinate frames | B | L | partly |
| 2 | `max_feed_mm_min` names two quantities; the advisor modulates against the travel rate | B | S | yes |
| 3 | The loader binds a toolpath with no `tool_id` / `model_id` to id 0 | B | S | yes |
| 4 | The id newtypes are optional: core records and commands carry raw `usize` ids | B | L | yes |
| 5 | `Feedrate` and `SafeZ` newtypes are dead; the G-code IR is bare `f64` / `u32` | C | S | no |
| 6 | The feeds quantity newtypes stop at the gate; the source records stay bare | D | M | partly |
| 7 | 2D boxes and points as anonymous `[f64; 4]` / tuples | D | M | partly |
| 8 | Angles: degrees in configs, radians in kernels, and no suffix on either | E | M | partly |
| 9 | `cut_summary_metrics` returns the two air-cut percentages as a bare tuple | E | S | yes |
| 10 | Grid kernels take `&[T]` + `rows, cols` beside `Grid2` | E | M | no |
| 11 | `DiagnosticId(pub String)` accepts any string | E | S | no |
| 12 | Units live in names only; ad-hoc `/ 60.0`; long runs of `f64` parameters | E | L | partly |
| 13 | Selection: one `ResolvedSelection` type for the model frame and the setup frame | E | S | yes (selection programme) |

---

## 1. One `P3` type for three coordinate frames (B, L)

**Claim.** World, stock-relative and setup-local points all have the type
`P3`. The two transform doors are not inverses, and only prose says so. Four
frame defects shipped in August 2026, and one of them put the retract plane
inside the stock.

**Evidence.**
- `crates/rs_cam_core/src/compute/transform.rs:273-279`: the doc says that
  `local_to_global` does "not re-add origin; callers that need true world
  coordinates should add it themselves".
- `transform.rs:288` `local_to_global(p: P3) -> P3` gives stock-relative
  coordinates. `transform.rs:301` `world_to_local(p: P3) -> P3` subtracts the
  stock origin. The signatures are identical.
- Past breaks (git log):
  - `6bcc3c97` G-SIM-IDENTITY-FRAME: identity setups went into the
    zero-rooted sim stock in the world frame.
  - `9df20910` G-SAFEZ-LOCAL: `safe_z` was floored in the local frame. With
    `origin_z > 5` the retract plane was inside the stock (a safety defect).
  - `f5afdeca` G-PINDRILL-FRAME: stock-relative pin XY was used as setup-frame
    XY.
  - `3b182398`: the composite renderer joined a world bbox and the
    zero-rooted grid.
- `crates/rs_cam_core/src/compute/alignment_pins/mod.rs:57-61`: `AlignmentPin
  { x, y, diameter }` is stock-relative, with bare `f64`.
- 31 production reads of `stock_origin_[xyz]` outside `transform.rs`. Each
  read is a hand-made frame shift.

**Enforcement now.** DOC (comments) plus sentries for the fixed cases.

**Proposed fix.** Add a frame marker type: `Point<F>` with
`PhantomData<F>` for `World`, `StockRel` and `SetupLocal`. Make each
`SetupTransformInfo` door take one frame and return another. Start at the
session/compute boundary (`SetupEvalContext`, `HeightsConfig`, pin and hole
sources). Keep kernels on plain `P3` inside one frame.

**Fix enforcement.** TYPES.

---

## 2. `max_feed_mm_min` names two quantities; the advisor modulates against the travel rate (B, S)

**Claim.** `MachineProfile.max_feed_mm_min` is the gantry travel rate. Other
structs use the same field name for the cutting ceiling. The strategy-advisor
candidate path gives the travel rate to the feed modulator as its cutting cap.
The production pass gives it the cutting ceiling.

**Evidence.**
- `crates/rs_cam_core/src/machine/mod.rs:182-191`: the doc says the field is
  the TRAVEL rate and "NOT the cutting ceiling". It says that cutting
  consumers must read `cutting_feed_ceiling_mm_min`.
- `crates/rs_cam_core/src/session/compute.rs:1680-1681`:
  `let max_feed = context.machine.max_feed_mm_min.max(1.0); let rapid_feed =
  max_feed;`. It goes to `ModulationContext { max_feed_mm_min: max_feed }` at
  `compute.rs:2064`.
- `crates/rs_cam_core/src/session/compute/simulation.rs:520`: the production
  modulation pass uses `self.machine.cutting_feed_ceiling_mm_min()`.
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:613`: the modulator pushes
  `ctx.max_feed_mm_min` as `BindingConstraint::MachineMaxFeed`, the hard cap
  on a cutting feed. The field doc at `feed_modulation.rs:289` says "Machine's
  `max_feed_mm_min` hard cap", which names the travel rate.
- The same field name holds the cutting ceiling at
  `machine/mod.rs:481` (`LinkKinematics`),
  `crates/rs_cam_core/src/feeds/explain_payload.rs:31,50` (`MachineEnvelope`)
  and `crates/rs_cam_cli/src/nc_replay.rs:44,72`.
- `crates/rs_cam_viz/src/ui/feeds/why.rs:269` tells the operator "The feed hit
  machine.max_feed_mm_min". The clamp is against `machine_cut_ceiling`
  (`crates/rs_cam_core/src/feeds/mod.rs:2531`).
- Not defects: `compute.rs:2194`, `rs_cam_viz/src/controller/events/simulation.rs:376`,
  `rs_cam_viz/src/io/export.rs:111` and `machine/strategy_advisor.rs:141-142`
  clamp a physical move at the travel rate, which is correct for timing.

**Effect.** When `band.max × rpm × flutes` is above the cutting ceiling
(default 6000 mm/min), an advisor candidate can be modulated up to the travel
rate. The advisor then times feeds that production never ships. No fixture
shows the trigger, so the grade is B, not A.

**Known.** T-18 (closed `d47a04d8`) fixed three feeds lifts of this class.
The advisor modulation site and the misnamed fields remain.

**Proposed fix.** Add `TravelRateMmMin` and `CuttingCeilingMmMin` with
private fields. Let only `MachineProfile` methods make them. Rename the
profile field to `travel_rate_mm_min` (breaking changes are ruled OK). Make
`ModulationContext` take `CuttingCeilingMmMin`. Then `compute.rs:1680` does
not compile.

**Fix enforcement.** TYPES.

---

## 3. The loader binds a toolpath with no `tool_id` / `model_id` to id 0 (B, S)

**Claim.** A toolpath section without `tool_id` loads bound to tool 0, and
without `model_id` to model 0. Tool 0 and model 0 usually exist, so the
toolpath silently cuts with another tool. The P2.5 rule says that a wrong
tool refuses by id.

**Evidence.**
- `crates/rs_cam_core/src/session/project_file.rs:699-701`: `tool_id:
  Option<usize>` and `model_id: Option<usize>`, both `#[serde(default)]`.
- `project_file.rs:1030-1031`: `tool_id: tp.tool_id.unwrap_or(0)`,
  `model_id: tp.model_id.unwrap_or(0)`.
- `crates/rs_cam_core/src/session/mod.rs:1835-1846`: "There is no fallback
  to another tool ... a wrong tool is a wrong cut".
- The same loader refuses a toolpath with no operation table because "a
  default operation would cut with values the operator never set"
  (`project_file.rs:1260-1262`).
- Index-as-id fallbacks: `project_file.rs:856` (tool), `:1125` (model),
  `:1252` (setup), `:1259` (toolpath). A file with some ids present and some
  absent can produce two records with one id. No duplicate check was found.
- Latent: `session/save.rs:226-227` always writes `Some(..)`. The shipped
  fixtures carry the keys. Hand-written or agent-written TOML can omit them.

**Proposed fix.** Make `tool_id: ToolId` and `model_id: ModelId` required
in the file section. Refuse a toolpath without them with a typed load error.
Refuse a record without `id`, or a duplicate id.

**Fix enforcement.** TYPES (required field) plus SENTRY (one load test).

---

## 4. The id newtypes are optional: core records and commands carry raw `usize` ids (B, L)

**Claim.** `ModelId`, `SetupId` and `ToolId` exist, but the records that own
the identity store raw `usize`. Every reader wraps or unwraps by hand. Two
command rows hold an index and an id as two `usize` values. R3 fixed this
class for `ToolpathId` only.

**Evidence.**
- Records: `crates/rs_cam_core/src/session/mod.rs:380` (`LoadedModel.id:
  usize`), `:742` (`SetupData.id: usize`), `:814-815`
  (`ToolpathConfig.tool_id/model_id: usize`). `ToolConfig.id` is a `ToolId`
  (`compute/tool_config.rs:161`), so tool identity has two spellings.
- `session/mod.rs:1836,1844`: `find_tool_by_raw_id(raw_id: usize)` and
  `find_model_by_raw_id`.
- Commands: `crates/rs_cam_core/src/session/command.rs:1270, 1411, 1438,
  1712, 1948, 1960`. `SetToolpathToolArgs { index: usize, tool_id: usize }`
  (`:1421-1426`) and `SetToolpathModelArgs { index: usize, model_id: usize }`
  (`:1434-1439`): a swap compiles. Each doc says "NOT its position".
- Other core sites: `diagnostics/mod.rs:108,116` (`Scope::Setup { id: usize }`,
  `Scope::Tool { id: usize }` beside `Scope::Toolpath { id: ToolpathId }`);
  `gcode/mod.rs:124` (`PhaseTool.id: usize`); `maps/reach_map.rs:327-328,
  480-482`; `compute/source_stock.rs:81,295`; `compute/stock_change.rs:437`;
  `compute/stock_change_apply.rs:50`; `compute/alignment_pins/hole_sources.rs:41,83`;
  `session/shoe.rs:30`; `session/pins.rs:499`; `session/multitool.rs:144`;
  `session/stock_change_report.rs:45`; `diagnostics/adapters/from_model_refs.rs:42`.
- Viz sites: `rs_cam_viz/src/ui_command.rs:294` (`toolpath_id: Option<usize>`
  with the doc "This is an id, never an index"); `compute/mod.rs:71,79`
  (`active_toolpath_id` and `active_toolpath_index`, both `Option<usize>`);
  `io/export.rs:684`; `ui/mod.rs:143,151,256` (`PinPartModels(Vec<usize>)`,
  `use_tools: Vec<(f64, usize)>` with a float diameter as a key,
  `setup_id: usize`).
- Cost of the gap: 98 re-wrap sites such as `SetupId(s.id)` and
  `ToolId(tc.tool_id)`; 580 `.0` reads on id fields; 10+ `t.id.0 ==
  tc.tool_id` compares (for example `rs_cam_viz/src/app/gpu_upload.rs:1071`).
- Every id newtype has a `pub` inner field (`ids.rs:24,34,38,44,49`,
  `compute/tool_config.rs:8`), so any caller can mint one from an index.
  Examples: `rs_cam_cli/src/job.rs:641,1254` (`ToolpathId(idx)`). `ToolId`
  lives outside `ids.rs`. `ModelId`, `SetupId` and `ToolId` lack
  `#[serde(transparent)]`; the other two have it.
- Past breaks: `7226703d` B7 (the GUI narrated with another tool's id);
  `3d7b0d35` R3 ("Back Rough is index 1 but id 4"); `gcode/mod.rs:118-120`
  (T-number collisions suppressed a tool change).

**Known.** R3 of the 2026-06-10 review made `ToolpathId`. `ids.rs:7-8`
records the ruling "Indexes stay `usize` on purpose". This finding keeps
that ruling: it types the ids, not the indexes.

**Proposed fix.**
1. Move `ToolId` into `ids.rs`.
2. Make the inner fields private. Add `serde(transparent)` to each id.
3. Let only the session allocators (`next_*_id`) and serde mint an id.
4. Change the record fields and the `*Args` fields to the newtypes.
5. Delete `find_*_by_raw_id`.

The compiler then lists every site. The MCP wire stays a bare integer.

**Fix enforcement.** TYPES.

---

## 5. `Feedrate` and `SafeZ` newtypes are dead; the G-code IR is bare `f64` / `u32` (C, S)

**Claim.** `gcode/post.rs` defines `Rpm`, `Feedrate` and `SafeZ` "to prevent
mixing". `SafeZ` has no use. `Feedrate` types only `PostLimits.max_feed`.
`Rpm` types only `PostLimits`. The IR that the emitter formats carries bare
values.

**Evidence.**
- `crates/rs_cam_core/src/gcode/post.rs:18-52`: three `pub` tuple newtypes.
  The `Feedrate` doc cites "formatting bugs from unit mixing have killed real
  machines".
- `post.rs:69-75`: the only typed fields (`max_rpm`, `min_rpm`, `max_feed`).
- `rg "\bSafeZ\b"` outside `post.rs`: the re-export at `gcode/mod.rs:17` only.
  `Rpm(` outside post.rs: `gcode/wizard_overlay.rs:167,169`.
- `crates/rs_cam_core/src/gcode/ir.rs:24,28` (`spindle_rpm: u32`, `rpm:
  u32`), `:51-75` (`x, y, z, i, j, feed: f64`).

**Proposed fix.** Delete `SafeZ`. Either delete `Feedrate` or use it for the
`feed` field of `Statement::Linear`, `ArcCw` and `ArcCcw`, and use `Rpm` in
`Preamble` and `SpindleSet`. Do not keep a protective type that nothing goes
through.

**Fix enforcement.** TYPES (if adopted) or NONE (if deleted; the dead type
stops reading as protection).

---

## 6. The feeds quantity newtypes stop at the gate; the source records stay bare (D, M)

**Claim.** `feeds/quantities.rs` exists because an arc-mean chip thickness was
compared with an advance-per-tooth band (2026-08-08). The newtypes wrap only
the gate-to-UI boundary. The sample record, the modulator band and 31
hand-written divisions keep the two quantities as bare `f64`.

**Evidence.**
- `crates/rs_cam_core/src/feeds/quantities.rs:1-23`: the module history and
  its one property ("must not compile").
- `quantities.rs:75,96,120,161`: every constructor is `pub const fn new(f64)`
  with no check.
- `crates/rs_cam_core/src/stock/simulation_cut.rs:181,183`:
  `chipload_mm_per_tooth: f64` (commanded advance) beside
  `effective_chip_thickness_mm: Option<f64>` (arc-mean) on one sample. 43
  production reads of the first in 16 files, 67 of the second in 17 files.
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:134-142`: a second band
  type, `ChiploadBand { pub min_mm_per_tooth: f64, pub max_mm_per_tooth: f64 }`.
  Its invariant ("Must be > 0 and ≤ max") is a doc line on `pub` fields.
- 31 hand-written `feed / (rpm * flutes)` or `x * rpm * flutes` sites beside
  `AdvancePerToothMm::from_commanded` (`quantities.rs:127`). Examples:
  `feeds/suggest/aggressiveness.rs:196`, `feeds/operating_point.rs:348`,
  `feeds/predict.rs:393`, `dressup/feed_modulation.rs:504-773` (9 sites),
  `rs_cam_viz/src/ui/feeds/compare.rs:46-47`, `ui/feeds/explore.rs:290,455,1170,1260,1401`.
- Spindle speed has two primitive types: 33 `*rpm*: u32` fields or
  parameters and 44 `*rpm*: f64`.
- `quantities.rs:265` `fraction_of_ceiling` is in
  `instruments/dead_pub_surface.md:52`.

**Known.** T-11 (closed `809d28b9`) was the same class: a depth in mm
multiplied by a fraction of another quantity.

**Proposed fix.**
1. Type `SimulationCutSample.chipload_mm_per_tooth` as `AdvancePerToothMm`.
2. Type `effective_chip_thickness_mm` as `Option<ArcMeanChipThicknessMm>`.
3. Delete `feed_modulation::ChiploadBand` and use `VendorChiploadBand`, with
   a point variant.
4. Add `AdvancePerToothMm::feed_at(rpm, flutes) -> CommandedFeedMmMin` and
   route the 31 sites through the two doors.
5. Give `new` a finite, non-negative check.

**Fix enforcement.** TYPES.

---

## 7. 2D boxes and points as anonymous `[f64; 4]` / tuples (D, M)

**Claim.** No 2D bounding-box type exists. Boxes travel as `[f64; 4]` or
`(f64, f64, f64, f64)`, and each site states the order in a doc line. Four
`polygon_bbox`-style copies exist.

**Evidence.**
- `[f64; 4]` or 4-tuples as boxes: `crates/rs_cam_core/src/polygon.rs:186,1731,1750`,
  `adaptive/material_grid.rs:655`, `geometry/region_mask.rs:344`,
  `finish/finish_planner.rs:1354`, `selection/item.rs:534`,
  `surface/rest_field.rs:189`, `adaptive3d/region_map.rs:31`,
  `surface/merge_tree.rs:88`, `surface/dropcutter.rs:153`,
  `stock/dexel.rs:649`, `adaptive3d/mod.rs:178`, `metrology/union_coverage.rs:99`.
  44 production occurrences of the two shapes in total (some are not boxes).
- `polygon_bbox` bodies at `polygon.rs:1750` and `adaptive/material_grid.rs:655`
  are identical. `region_mask.rs:344` and `finish_planner.rs:1354` are local
  `poly_bbox` copies.
- `(f64, f64)`: 254 production occurrences in 87 files, with mixed meanings
  (points, ranges, value pairs). 29 are in
  `crates/rs_cam_core/src/compute/alignment_pins/walls.rs`.

**Proposed fix.** Add `geo::BoundingBox2 { min: P2, max: P2 }` with
`from_points`, `contains`, `union` and `pad`. Replace the arrays and tuples.
Delete the local copies. Use `P2` where a tuple is a point.

**Fix enforcement.** TYPES.

---

## 8. Angles: degrees in configs, radians in kernels, and no suffix on either (E, M)

**Claim.** Operation configs and most kernels take degrees. The V-bit kernels
and the B-rep mesh take radians. Neither group has a suffix or a type. No
break was found, but 88 `to_radians()` sites carry the convention by hand.

**Evidence.**
- Radians, no suffix: `crates/rs_cam_core/src/ops/vcarve.rs:26`
  (`half_angle`), `ops/inlay.rs:64,93`, `ops/chamfer.rs:39`
  (`tool_half_angle`), `geometry/enriched_mesh.rs:100` (`dihedral_angle`),
  `enriched_mesh.rs:51` (`Cone { half_angle }`, no unit stated),
  `enriched_mesh.rs:156` (`concave_edges(max_dihedral: f64)`).
- Degrees, no suffix: `compute/tool_config.rs:175,177` (`included_angle`,
  `taper_half_angle`), `ops/zigzag.rs:31`, `compute/config.rs:795,899`,
  `finish/pencil.rs:100`, `finish/steep_shallow.rs:42`,
  `finish/horizontal_finish.rs:20`.
- The conversion door is a private helper: `compute/execute/curve_engrave.rs:344-346`.
- The suffixed style also exists: `tool/tapered_ball.rs:30`
  (`taper_half_angle_deg`), `tool/vbit.rs:30` (`half_angle_rad`).
- 88 `to_radians()` and 24 `to_degrees()` production calls.

**Proposed fix.** Add `Degrees(f64)` and `Radians(f64)` with private fields
and `From<Degrees> for Radians`. Use `Degrees` in config and wire structs and
`Radians` in kernel params. Delete `vbit_half_angle`.

**Fix enforcement.** TYPES.

---

## 9. `cut_summary_metrics` returns the two air-cut percentages as a bare tuple (E, S)

**Claim.** LH-1 found that "air cut %" has two denominators. The function
that gives both returns `Option<(f64, f64, f64, f64)>`. A swap at the one
destructure compiles. The comment above it describes the past wrong marker
(D7).

**Evidence.**
- `crates/rs_cam_core/src/trace/narrate.rs:2166-2169`: the signature.
- `narrate.rs:2073-2074`: the destructure into `air_pct_of_cutting,
  air_pct_of_total, cutting_time_s, average_engagement`.
- `narrate.rs:2081-2088`: D7, a marker that "used to set its ⚠ from the
  un-thresholded one".
- T-2 in `planning/TECH_DEBT_REGISTER.md`: the LH-1 sentry is a string check
  that a closure defeated.

**Proposed fix.** Return a named struct. Better: make
`AirCutRatios::air_cut_pct_of_cutting_time()` and
`air_cut_pct_of_total_runtime()` return two distinct newtypes. The T-2
sentry then becomes a type check.

**Fix enforcement.** TYPES.

---

## 10. Grid kernels take `&[T]` + `rows, cols` beside `Grid2` (E, M)

**Claim.** `Grid2<T>` exists so that the dimensions travel with the cells.
Six public kernels still take a raw slice plus `rows, cols`. Callers that hold
a `Grid2` or a `GridSpec` take it apart and swap `nx, ny` into `rows, cols`
by hand. The order is correct today.

**Evidence.**
- Raw kernels: `crates/rs_cam_core/src/geometry/grid_field.rs:79,128,181`,
  `finish/steep_shallow.rs:91,127`, `finish/finish_planner.rs:764`.
- Callers that take a grid apart: `geometry/region_mask.rs:194`
  (`distance_transform_2d(mask.as_slice(), ny, nx)`),
  `finish/unified_finish.rs:1725` (`spec.ny, spec.nx`).
- Two dimension orders: 14 `rows: usize, cols: usize` signatures and 12
  `nx: usize, ny: usize` signatures. Rows correspond to `ny`, so the orders
  are opposite.
- `Grid2` has 42 references in 4 files (`geometry/grid2.rs:11-18` lists the
  planned adopters).

**Proposed fix.** Make the six kernels take `&Grid2<T>` and return
`Grid2<U>`. The `ny, nx` swap then has no call-site form.

**Fix enforcement.** TYPES.

---

## 11. `DiagnosticId(pub String)` accepts any string (E, S)

**Claim.** Diagnostic ids are a `pub String` newtype with `From<&str>` and
`From<String>`. A misspelt id in `supersedes` compiles and never suppresses.
All production sites use the `ids::` constants today, so no break exists.

**Evidence.**
- `crates/rs_cam_core/src/diagnostics/mod.rs:78-100`.
- `diagnostics/supersession.rs:45`: the reducer matches `supersedes` by
  string equality.
- `diagnostics/ids.rs:416`: the `ALL` slice of known ids.
- `diagnostics/adapters/from_tool_load.rs:384`: one dynamic
  `DiagnosticId::from(id_str)`.
- Other `pub`-field newtypes with no construction guarantee:
  `stock/material_slot.rs:32` (`MaterialSlot(pub u8)`; capacity is 8),
  `trace/toolpath_spans.rs:26` (`SpanId(pub u32)`),
  `geometry/enriched_mesh.rs:22` (`FaceGroupId(pub u16)`),
  `diagnostics/evidence.rs:17` (`EvidenceLocality(pub String)`).

**Proposed fix.** Make the field private and the type `&'static str`. Let
the `ids::` module mint each constant (`pub const X: DiagnosticId`). Give
`MaterialSlot` a checked constructor that refuses a slot at or above
`MATERIAL_SLOT_CAPACITY`.

**Fix enforcement.** TYPES.

---

## 12. Units live in names only; ad-hoc `/ 60.0`; long runs of `f64` parameters (E, L)

**Claim.** About 1500 production fields and parameters carry the unit only as
a name suffix, and many carry none. Per-minute to per-second conversions are
49 hand-written `60.0` factors. 84 functions take four or more adjacent `f64`
of different meaning. No unit type exists.

**Evidence.**
- `rg "_(mm|deg|rad|rpm|mm_min|s|um|mm2|mm_s2)\w*: f64"`: 1498 production
  matches. No `struct Mm` or `struct Radians` exists.
- No suffix: `crates/rs_cam_core/src/compute/tool_config.rs:166,173`
  (`diameter`, `corner_radius`) beside `:171` (`corner_radius_mm`). Both
  fields hold mm.
- `machine/kinematics.rs:1286,1516`: `trapezoidal_peak_velocity` and
  `trapezoidal_time(length, v_in, v_out, v_cmd, accel)` take mm/s with no
  suffix. The neighbour `predicted_achieved_feed` (`:1258`) takes `_mm_min`.
  `kinematics.rs` holds 20 of the 49 `60.0` factors; `machine/kinematic_utilization.rs`
  holds 12.
- Adjacent-`f64` runs: `finish/steep_shallow.rs:155` `emit_steep_passes`
  (8: `z_top, z_bottom, z_step, sampling, stock_to_leave, feed_rate,
  plunge_rate, safe_z`), `adaptive3d/clearing.rs:1580` (8),
  `adaptive3d/path.rs:1214` (7), `feeds/force.rs:160,214` (6),
  `dexel_stock/simulation.rs:323-332` (`spindle_rpm: u32, flute_count: u32`
  and `rapid_feed_mm_min: f64, sample_step_mm: f64` adjacent).

**Known.** Q10 of `planning/tech_debt_2026-09-16` (110
`too_many_arguments` rows, F, below the cut line). This finding adds the
unit view: the risk is a silent swap, not the count.

**Proposed fix.** Do not start a units framework. Add `MmPerMin` and
`MmPerSec` with one `From` each way, and use them in `machine/kinematics*`
first. Replace the eight-`f64` signatures with the params structs that the
callers already hold (for example `SteepPassParams`).

**Fix enforcement.** TYPES (kinematics), CONVENTION (the rest).

---

## 13. Selection: one `ResolvedSelection` type for the model frame and the setup frame (E, S, selection programme)

**Claim.** `SelectionFrames` holds two `ResolvedSelection` values, one in the
model frame and one in the setup frame. `face_top_z` is correct only on the
setup one, and only a doc line says so. This is a property of the unfinished
geometry-selection feature.

**Evidence.**
- `crates/rs_cam_core/src/session/selection_status.rs:154-159`:
  `SelectionFrames { model: ResolvedSelection, setup: ResolvedSelection }`.
- `crates/rs_cam_core/src/selection/resolve.rs:180-183`: "Call it after
  `Self::to_setup_frame`". `resolve.rs:205`: `to_setup_frame(self) -> Result<Self, _>`
  returns the same type.
- `crates/rs_cam_core/src/selection/item.rs:104-108`: `FaceItem.plane_z` is
  "in the frame of the item's owner". `selection/rule.rs:76-78,92-96`:
  `ZRange` and `FaceFilter` are in the SETUP frame. Both are bare `f64`.
- Two doors run `resolve` then `to_setup_frame`:
  `session/selection_status.rs:192` and `session/compute/generation.rs:576`.
- `crates/rs_cam_core/src/trace/transform_provenance.rs:68,334` already has a
  sealed typestate that fits this case.

**Proposed fix.** Make `ResolvedSelection<F>` with a frame marker. Let
`to_setup_frame` return `ResolvedSelection<SetupFrame>`, and define
`face_top_z` on that type only. Merge the two doors into one.

**Fix enforcement.** TYPES.

---

## Not filed

- `HeightMode::Manual(f64)` (`compute/config.rs:253`) is an absolute Z with no
  frame tag. It belongs to finding 1.
- Command `index: usize` fields (49 in `session/command.rs`) follow the
  `ids.rs:7-8` ruling. Finding 4 types only the ids beside them.
- `rs_cam_mcp` parameter structs use raw integers on purpose (JSON wire).
