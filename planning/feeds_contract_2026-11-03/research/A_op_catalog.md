# A — The operation catalog and the compile-time rules that exist (2026-11-03)

Read-only research for the feeds contract. Paths are under `crates/rs_cam_core/src/` unless they
say otherwise. Line numbers are from the working tree on 2026-11-03. The files that another
session edits now (`mesh.rs`, `surface/dropcutter.rs`, `ops/project_curve.rs`,
`finish/scallop/ring_generation.rs`) are not cited. `compute/`, `feeds/` and `session/mod.rs`
have no local change against HEAD.

Sections 1 to 5 are facts. Section 6 holds the suggestions.

## 1. How an operation is defined, end to end

### 1.1 The parts and their order

| Step | Item | Where | Made by |
|---|---|---|---|
| 1 | The X-macro list: variant, config type, menu category, serde token | `compute/catalog.rs:382-414` (`for_each_op!`) | hand |
| 2 | `OperationType` enum, `ALL`, `category`, `name`, `kind_str` | `compute/catalog.rs:430-485` | `define_operation_type!` from the list |
| 3 | `OperationConfig::{op_type, new_default, as_params, as_params_mut}` | `compute/catalog.rs:487-517` | `define_operation_config_dispatch!` from the list |
| 4 | `OperationConfig` enum (`#[serde(tag = "kind", content = "params")]`) | `compute/catalog.rs:891-917` | hand ("serde-attribute regression risk", `catalog.rs:370-373`) |
| 5 | The config struct, `#[derive(OpParams)]` | `compute/operation_configs.rs:231-3400` | hand fields; the derive writes `PARAMS` |
| 6 | `impl OperationParams` (the feeds accessors) | `compute/operation_configs.rs:3565-3880` (`impl_operation_params!`) | macro, one declaration per config |
| 7 | The registry row `REG_*: OpRegistryEntry` | `compute/catalog/registry.rs:62-1440` | hand, full struct literals |
| 8 | `OperationType::registry_entry` | `compute/catalog.rs:596-624` | hand, exhaustive `match` |
| 9 | `OperationConfig::capabilities` (row + config overrides) | `compute/catalog.rs:1006-1123` | hand, exhaustive `match` |
| 10 | Other per-op `match` functions on the config | `catalog.rs`: `outline_use` (`:926`), `surface_mask` (`:1140`), `unset_depths` (`:1551`), `cutting_levels` (`:1953`) and others | hand, exhaustive |

The comment at `catalog.rs:590-595` states the intent of step 8: "Exhaustive match — adding an
`OperationType` variant does not compile until it has a registry entry, and an entry cannot be
written without explicitly deciding its spec, settable-param schema, and tool constraints."

### 1.2 `OpRegistryEntry` (static, per op TYPE)

`compute/catalog/schema.rs:668-692`. Fields:

- `op_type: OperationType`
- `spec: OperationSpec` (`catalog.rs:92-114`): `label`, `description`, `family`, `geometry`,
  `default_auto_regen`, `ui_family`, `ui_process_role`, `feeds_family`, `feeds_pass_role`,
  `feeds_formula_source`.
- `params: ParamTable` (`schema.rs:127-136`): written by `#[derive(OpParams)]` as
  `<Config>::PARAMS`.
- `tool_constraints: ToolConstraintsDef` (`schema.rs:436-520`).
- `dressup_policy: DressupPolicy` (`schema.rs:620-664`): `strip_all_reason: Option<&str>` and
  `entry: EntryStylePolicy` (`AnyEntry`, `ForceNone`, `DefaultHelix`, `schema.rs:602-616`).
  Constants: `ANY_DRESSUP`, `FORCE_NO_ENTRY`, `DEFAULT_HELIX`, `strip_all(reason)`.
- `policy: OpPolicy` (`schema.rs:549-596`): `raster_stepover_is_lateral`, `kinematics`
  (`Milling` / `DrillZOnly`), `empty_is_feature_selective`.
- `capabilities: RowCapabilities` = `Capabilities<EntryProbeRule>` (`capabilities.rs:289`).
- `generate: GenerateFn` (the family adapter).

Fact: `OperationSpec::feeds_formula_source` doc says "Every row is `Some` today"
(`catalog.rs:110-113`). This is stale: Drill and Pin Drill hold `None` (`registry.rs:562`,
`registry.rs:1405`), and `tests/every_cell_declares_its_feeds_support_fm0.rs:157-159` pins `None`.

### 1.3 `Capabilities` (static row, with config overrides)

`compute/catalog/capabilities.rs:46-164`. The struct has no `Default` derive
(`capabilities.rs:41`), so each `REG_*` literal names all 26 fields. No row uses `..` struct
update (rg `\.\.[A-Z_]` in `registry.rs`: no hit). Fields and their kind:

| Field | Type | Static or config |
|---|---|---|
| `transform` | `OperationTransformCapabilities` | row; Scallop config picks `TRANSFORM_DISCRETE_SCALLOP` (`catalog.rs:1014-1019`) |
| `air_cut_high_threshold_pct` | `Option<f64>` | row |
| `floor_owner` | `FloorOwner` (`HeightsBottom`, `OpFloor`, `OpDial`; `capabilities.rs:231-245`) | row |
| `entry_probe` | `EntryProbeRule` in the row, `EntryProbe` after resolve (`capabilities.rs:576-603`) | row rule; `SurfaceFromConfig` reads `stock_to_leave` (`catalog.rs:1126-1137`) |
| `link_ceiling` | `LinkCeilingNeed` | row |
| `feeds_hints` | `FeedsHints` | row is `NONE` for all 25; config fills 7 ops (`catalog.rs:1020-1069`) |
| `move_count` | `MoveCountModel` (`capabilities.rs:617-631`) | row |
| `doc_basis` | `DocBasis` (`DepthPerPass`, `SurfaceHeights`, `CurveOnSurface`, `PeckDepth`, `VBitDepth`; `capabilities.rs:637-650`) | row; narration only |
| `air_cut_hint` | `AirCutHint` | row |
| `relink_dial` | `Option<&'static str>` | row |
| `axial_envelope` | `AxialEnvelope` (`NotApplied`, `Roughing3d`, `VBitGroove`, `CurveRough`, `Finish3d`; `capabilities.rs:667-681`) | row |
| `rest_use`, `territory_xy`, `lut_route`, `item_use`, `model_gouge`, `face_top` | enums | row; `item_use` overridden for Profile side On and Pocket open paths (`catalog.rs:1073-1093`) |
| `plans_bands`, `ramp_descent`, `adaptive_woc`, `flat_floor_clearing`, `top_on_stock_box`, `floor_inside_stock`, `ladder_includes_top` | `bool` | row |
| `planner_uniform_engagement` | `bool` | row `false`; Adaptive 2D ContourSpiral sets `true` (`catalog.rs:1094-1100`) |
| `clearing_strategy_dial` | `Option<&'static str>` | row |

### 1.4 `FeedsHints`

`compute/catalog.rs:1910-1936`:

```rust
/// Operation-specific hints handed to the feeds calculator. Hints are
/// config values (z-step, scallop target, …), so the registry row holds
/// [`FeedsHints::NONE`] and [`OperationConfig::capabilities`] fills
/// [`Capabilities::feeds_hints`] from the config (W2.4).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FeedsHints {
    pub axial_depth_mm: Option<f64>,
    /// Operation-imposed radial width (mm); none of the current ops set
    /// this, but the slot is part of the calculator contract.
    pub radial_width_mm: Option<f64>,
    pub target_scallop_mm: Option<f64>,
}
```

`FeedsHints::NONE` doc (`catalog.rs:1928-1930`): "Referenced explicitly per op so the no-hint set
is a recorded decision, not a wildcard fallback."

Facts about the strength of this rule:

- `FeedsHints` derives `Default`, and every override arm writes `..FeedsHints::NONE`
  (`catalog.rs:1022, 1029, 1039, 1046, 1053, 1060, 1067`). A new hint field gets `None` in every
  arm with no compiler word.
- Every override arm writes `..row`. The no-override arm (`catalog.rs:1104-1122`) lists 18
  variants in one or-pattern that returns `row`. A new op joins that or-pattern with one line and
  states no hint.
- No row and no config fills `radial_width_mm` (`catalog.rs:1918-1920`; sentry below).

### 1.5 Per-instance values: `OperationParams`

The trait (`catalog.rs:797-889`) gives the feeds layer its per-instance view. It has default
bodies that answer `None` for `ramp_feed_rate`, `stepover`, `depth_per_pass`, `total_depth` and
`scallop_height`. Required: `feed_rate`, `plunge_rate`, `plunge_field`, `depth_semantics`,
`spindle_rpm` and their setters.

`impl_operation_params!` (`operation_configs.rs:3529-3626`) makes each role a required clause.
The doc at `operation_configs.rs:3538-3542`: "The first five and `depth_semantics` are required
(F163): each config states each decision, and a new config that leaves one out does not compile.
`none` is the explicit 'this config has no such field'".

`DepthSemantics` (`catalog.rs:76-90`): `Explicit(f64)`, `DerivedStockTop(f64)`, `None`,
`Resolved` (a `Floor` that is not `Top`), `Unset` (S11).

Facts: the clause value `none` hides real geometry in four places.

- Project Curve: `total_depth: none` although `depth: Option<f64>` exists
  (`operation_configs.rs:3874-3881`, field at `:3314`).
- VCarve: `total_depth: none` although `max_depth` exists (`:3672-3679`).
- Chamfer: `total_depth: none`, depth is `chamfer_width` (`:3734-3741`).
- 3D Rough: `total_depth: none`; the cut goes from the stock box top to the Heights bottom
  (`:3761-3768`, `top_on_stock_box`).

The trait has no slot for radial engagement other than `stepover`, and none for "the tool cuts a
slot" or "the tool follows a line".

## 2. Per-op cut geometry (the input table for a typed cut description)

Abbreviations: `oc` = `compute/operation_configs.rs`, `reg` = `compute/catalog/registry.rs`.
"Hints" is what `OperationConfig::capabilities().feeds_hints` gives today
(`catalog.rs:1006-1123`), pinned by `feeds/suggest/tests.rs:2544-2593` and
`tests/every_op_states_its_capabilities_w2_4.rs:413-455`. "Slot" is my reading of the generator
contract from the field docs; it is not a measurement.

| Op | Feeds family / role (reg) | Radial engagement source | Axial engagement source | Total depth / bottom | Can be a full slot (ae = D) | Entry / dressup policy (reg) | Hints today |
|---|---|---|---|---|---|---|---|
| Face | Pocket / Roughing (`:62`) | `stepover` (`oc:234`) | `depth_per_pass` (`oc:238`) | `depth` (`oc:236`), `FloorOwner::OpDial` | no, ae = stepover | `ANY_DRESSUP` (`:78`) | NONE |
| Pocket | Pocket / Roughing (`:111`) | `stepover` (`oc:821`); `pattern` (`oc:847`) | `depth_per_pass` (`oc:829`) | `floor: Floor` (`oc:827`) | yes: the first pass into fresh stock | `ANY_DRESSUP` (`:127`) | NONE |
| Profile | Contour / Roughing (`:162`) | no field; `side` (`oc:914`), `finishing_passes` (`oc:982`) | `depth_per_pass` (`oc:922`) | `floor` (`oc:920`) | yes: side On always; Outside/Inside when stock stands on both sides | `ANY_DRESSUP` (`:178`) | NONE |
| Adaptive (2D) | Adaptive / Roughing (`:213`) | `stepover` (WOC target, `oc:1031`) | `depth_per_pass` (`oc:1039`) | `floor` (`oc:1037`) | yes when `slot_clearing` (`oc:1069`, doc: "full radial immersion") | `DEFAULT_HELIX` (`:232`) | NONE |
| VCarve | Trace / Finish (`:267`) | `stepover` = scan spacing (`oc:1142`); V-bit width from depth | 0 to `max_depth` (`oc:1140`), V-bit | `max_depth` | groove (V) | `strip_all` (`:292`) | axial = `max_depth` |
| Inlay | Trace / Finish (`:328`) | `stepover` (`oc:1261`), `floor_overlap`, `floor_ridge` | `pocket_depth` (`oc:1232`), one depth | `pocket_depth` | V walls | `strip_all` (`:351`) | NONE |
| InlayPlug | Trace / Finish (`:387`) | `stepover` (`oc:1377`) | `depth_per_pass` (`oc:1371`) | `total_cut_depth()` (`oc:1423`) | V walls | `strip_all` (`:410`) | NONE |
| Zigzag | Pocket / Roughing (`:446`) | `stepover` (`oc:1460`) | `depth_per_pass` (`oc:1468`) | `floor` (`oc:1466`) | yes: the first line into fresh stock | `ANY_DRESSUP` (`:462`) | NONE |
| Trace | Trace / Finish (`:497`) | no field; `compensation` (`oc:332`) | `depth_per_pass` (`oc:315`) | `depth: Option` (`oc:313`) | yes: compensation None puts the centre on the line | `FORCE_NO_ENTRY` (`:514`) | NONE |
| Drill | Drill / Roughing (`:548`) | none (axial only, ae = D) | `peck_depth` (`oc:381`), `cycle` | `floor` (`oc:378`) | n/a (plunge) | `FORCE_NO_ENTRY` (`:567`) | NONE |
| Bore | Pocket / Roughing (`:619`) | hole diameter from the item and `finish_allowance` (`oc:554`); no field | helix `pitch` (`oc:540`) | `floor` (`oc:529`) | near slot: hole up to 2 x D (`reg:626-627` description) | `strip_all` (`:642`) | NONE |
| Chamfer | Trace / Finish (`:687`) | `chamfer_width` (`oc:714`), V-bit | from width, angle, `tip_offset` (`oc:720`) | `chamfer_width` | V groove on the edge | `ANY_DRESSUP` (`:706`) | NONE |
| DropCutter | Parallel / Finish (`:739`) | `stepover` (`oc:1528`) or `scallop_height` (`oc:1572`) | surface; `min_z` (`oc:1549`) | surface | no | `strip_all` (`:759`) | scallop = `scallop_height` |
| Adaptive3d (3D Rough) | Adaptive / Roughing (`:794`) | `stepover` (`oc:1616`) | `depth_per_pass` (`oc:1618`), `stock_to_leave_axial` (`oc:1625`) | stock box top to Heights bottom (`FloorOwner::HeightsBottom`) | report 01: ae about 14 mm on an 18 mm tool; entry legs | `FORCE_NO_ENTRY` (`:816`); planner entry in `entry_style`, `ramp_angle_deg`, `helix_*`, `entry_clearance_mm` (`oc:1646-1681`) | NONE |
| Waterline | Contour / SemiFinish (`:849`) | surface (no stepover) | `z_step` (`oc:1905`) | Heights bottom | no | `ANY_DRESSUP` (`:865`) | axial = `z_step` |
| Pencil | Trace / Finish (`:898`) | `offset_stepover` when `num_offset_passes > 1` (`oc:1998-2003`) | surface valley | surface | valley (both walls touch) | `ANY_DRESSUP` (`:919`) | NONE |
| Scallop | Scallop / Finish (`:952`) | from `scallop_height` (`oc:2219`) | surface | surface | no | `ANY_DRESSUP` (`:971`) | scallop = `scallop_height` |
| UnifiedFinish | Scallop / Finish (`:1009`) | `scallop_height` (`oc:2375`), `raster_stepover` (`oc:2382`) | `z_step` (`oc:2384`) in steep bands | Heights bottom | no | `strip_all` (`:1036`) | scallop only; `z_step` not passed |
| SteepShallow | Contour / Finish (`:1074`) | `stepover` (`oc:2872`) | `z_step` (`oc:2874`) | surface | no | `ANY_DRESSUP` (`:1090`) | axial = `z_step`; stepover not passed |
| RampFinish | Parallel / Finish (`:1126`) | surface | `max_stepdown` (`oc:2961`) | surface | no | `ANY_DRESSUP` (`:1142`) | axial = `max_stepdown` |
| SpiralFinish | Scallop / Finish (`:1175`) | `stepover` (`oc:3038`) | surface | surface | no | `ANY_DRESSUP` (`:1198`) | NONE |
| RadialFinish | Parallel / Finish (`:1231`) | `angular_step` (`oc:3103`), varies hub to rim | surface | surface | no | `ANY_DRESSUP` (`:1247`) | NONE |
| HorizontalFinish | Parallel / Finish (`:1280`) | `stepover` (`oc:3175`) | surface (flats) | surface | no | `ANY_DRESSUP` (`:1296`) | NONE |
| ProjectCurve | Trace / Finish (`:1336`) | no field; `side` Center/Inside/Outside (`oc:3344`) | `depth: Option` (`oc:3314`), one pass, no `depth_per_pass` | `depth` below the projected surface | yes: side Center is a groove of width D | `strip_all` (`:1354`, "phantom diagonal cut") | NONE |
| AlignmentPinDrill | Drill / Roughing (`:1391`) | `diameter` (`oc:624`) | `peck_depth` (`oc:633`) | stock Z + `spoilboard_penetration` (`oc:630`) | n/a (plunge) | `FORCE_NO_ENTRY` (`:1416`) | NONE |

Counts: 7 ops give one hint (Scallop, UnifiedFinish, DropCutter, Waterline, SteepShallow, VCarve,
RampFinish). 18 give none. No op gives `radial_width_mm`. All 25 rows hold `FeedsHints::NONE`.

Observations that come from the table:

- Five ops always or often cut a full slot and have no radial field: Profile, Trace, Project
  Curve, Bore, and Pocket/Zigzag on the first pass. Adaptive 2D has a bool for its slot lines.
- Four ops hold an operator depth that the calculator never sees: Face (`depth_per_pass`),
  Trace (`depth_per_pass`), Project Curve (`depth`) and 3D Rough (`depth_per_pass`).
- `DocBasis` already classifies the axial basis per op, but only the narration reads it.

## 3. The enforcement patterns that exist

### 3.1 Compile time

| Pattern | Where | Intent text |
|---|---|---|
| X-macro op list generates `OperationType`, its `ALL`, `name`, `kind_str`, and the config dispatch | `catalog.rs:358-517` | "GENERATED from the `for_each_op!` list — edit the list, not this block." (`:434-435`) |
| Exhaustive `registry_entry` | `catalog.rs:589-624` | quoted in 1.1 |
| Full `REG_*` struct literals, no `Default` on `OpRegistryEntry`, `Capabilities`, `OperationSpec` | `schema.rs:668`, `capabilities.rs:41-46`, `catalog.rs:92` | "Every registry row states one [`Capabilities`] value, so a new operation does not compile until it states each field." (`capabilities.rs:3-4`) |
| `with_entry_probe` names every field | `capabilities.rs:308-340` | "The body names every field, so a new field cannot be dropped here." |
| Exhaustive `match` with no wildcard on `OperationConfig` / `OperationType` | `capabilities()` `catalog.rs:1006`; `cutting_levels` `:1953`; `outline_use` `:926` and `OperationType::outline_use` `:771`; `unset_depths` `:1551`; `surface_mask` `:1140`; `apply_stock_defaults` `feeds/suggest/apply.rs:1110-1186` | `cutting_levels`: "Every variant is named — no wildcard arm. An operation that does not step down uniformly says so in the last arm, which is a recorded decision and not a fallback: a new 2.5D operation fails to compile until it decides" (`catalog.rs:1940-1944`) |
| Class enums matched by consumers | `FloorOwner`, `FaceTopRule`, `ItemUse`, `OutlineUse`, `RestUse`, `AxialEnvelope`, `MoveCountModel`, `DocBasis` (`capabilities.rs`) | `FloorOwner`: "Each reader `match`es it, so a new owner names every reader." (`:228-229`); module doc: "A consumer that maps a class to its own numbers or words matches on the class enum, which is exhaustive." (`:13-14`) |
| CMP-07: `matches!` lists moved to row fields (`OpPolicy`) | `schema.rs:525-596` | "These three failed open: a new operation joined the false side of each in silence." (`schema.rs:542-544`) |
| F163 required clauses in `impl_operation_params!` | `operation_configs.rs:3538-3542` | quoted in 1.5 |
| F185 domain rule in `#[derive(OpParams)]` (const panic at the field type) | `crates/rs_cam_derive/src/lib.rs:353-395` | "A field with none of the three does not compile." (`lib.rs:55-56`) |
| W5 field partition: every field is published, `driver_set` or `skip` | derive `lib.rs:277-351`; `ParamTable` `schema.rs:122-136` | "every field of the struct is in exactly one of [`Self::defs`], [`Self::driver_set`] and [`Self::skipped`]" (`schema.rs:124-126`) |
| `ParamType` bound on each published field | derive `lib.rs:40-41` | "Its type must implement `ParamType`; the compiler refuses a field of another type." |
| Sealed traits | `trace/transform_provenance.rs:68-83` (typestate `Unreconciled`/`Reconciled`); `selection/resolve.rs:178-190` (`SelectionFrame`); `compute/transform.rs:249-256` (`PointFrame`, F001) | `SelectionFrame`: "The trait is sealed: [`ModelFrame`] and [`SetupFrame`] are the only frames." |
| Unit newtypes | `feeds/quantities.rs:71-185` (`CommandedFeedMmMin`, `AchievedFeedMmMin`, `AdvancePerToothMm`, `ArcMeanChipThicknessMm`); `gcode/post.rs:67,116` (`Feedrate`, `SafeZ`); `machine/mod.rs:184,201` | "handing an [`ArcMeanChipThicknessMm`] to a band comparison or a colour scale that wants an [`AdvancePerToothMm`] must not compile." (`feeds/quantities.rs:15-17`) |
| `Capabilities<P>` type parameter: a row holds a rule, a config holds the value | `capabilities.rs:35-46, 289` | "the row states a RULE for the leave, never a placeholder number" (`:37-38`) |
| W16 lints | root `Cargo.toml:64-80`: `allow_attributes`, `allow_attributes_without_reason`, `match_wildcard_for_single_variants`, `fn_params_excessive_bools`, `struct_excessive_bools` (deny); `unsafe_code` (forbid) | "F172 (W16): a `_` arm that stands for exactly one variant names it." (`:71`) |
| WP7 mutation contract: `ProjectSession::apply(Command)` is the one door; `*_mut` hatches are `pub(crate)` or gone | `session/command.rs:2731`; `session/mod.rs:2348-2366` | "WP7 makes the reach a compile error: each of the nine is `pub(crate) fn`, or it is gone." (`tests/hatches_are_crate_private_wp7.rs:12-13`) |
| Command registry X-macro | `session/command.rs:128` (`for_each_command!`) | "a `Surfaces` literal that omits a field does not compile, and a `match` over `CommandId` that omits a variant does not compile" (`tests/command_registry_completeness.rs:7-9`) |

Gaps in the compile-time net (facts):

- `wildcard_enum_match_arm` is not in the lint list (`Cargo.toml:31-80`). A `_` arm over two or
  more variants compiles. Example in the feeds path: `apply_drill_defaults` ends with `_ => {}`
  (`feeds/suggest/apply.rs:1105`).
- `#[non_exhaustive]` is not used in the workspace (rg: only `finish_non_exhaustive` in `Debug`
  impls).
- `FeedsHints` derives `Default` and the arms use `..FeedsHints::NONE` / `..row` (section 1.4).
- `OperationParams` defaults answer `None`, and the macro clause `none` is a legal answer for an
  op that has the geometry (section 1.5).
- `OperationConfig` itself is hand-written, outside the X-macro (`catalog.rs:370-373`).

### 3.2 Test time (registry walks and frozen tables)

| Sentry | What it holds |
|---|---|
| `compute/catalog/tests.rs:14` `operation_catalog_is_exhaustive_and_consistent` | the registry rows against `ALL` |
| `compute/catalog/tests.rs:53` `every_config_field_is_published_driver_set_or_skipped` | W5 partition |
| `compute/catalog/tests.rs:149` `every_declared_range_holds_the_default` | default inside range |
| `compute/catalog/tests.rs:206` `tool_constraints_are_an_explicit_per_op_decision` | per-op tool kinds |
| `compute/catalog/tests.rs:328, 382, 408` | `drill_kinematics_set_is_pinned`, `lateral_raster_stepover_set_is_pinned`, `feature_selective_exemption_set_is_pinned` |
| `compute/catalog/tests.rs:352` `cutting_levels_is_exhaustive_per_op` | levels agree with `depth_semantics` + `depth_per_pass` |
| `compute/catalog/tests.rs:431` `operation_partitions_cover_all_variants_once` | `ALL_2D`/`ALL_3D` against the X-macro category |
| `compute/catalog/tests.rs:546` `operation_type_serde_repr_pinned` | serde token = `kind_str` |
| `feeds/suggest/tests.rs:2544` `operation_feeds_hints_is_an_explicit_per_op_decision` | exhaustive `match` with no wildcard, one arm per op; the doc says "adding a new operation fails to compile here, forcing the author to decide (and record) what hints the new op feeds the calculator" (`:2537-2541`). It is a test, so the compile error shows only when the test target builds. |
| `tests/every_op_states_its_capabilities_w2_4.rs:278-455` | frozen capability tables; `the_feeds_hints_read_the_ops_own_fields` pins the 7 hinted ops and asserts all three hints `None` on the other 18 (`:440-454`) |
| `tests/every_op_conforms_w4.rs` | conformance per `OperationType::ALL` through `generate_toolpath` |
| `tests/every_op_keeps_its_toolpath_g_opgolden.rs` | output fingerprint per op |
| `tests/every_cell_declares_its_feeds_support_fm0.rs:155` | exhaustive `match` on `feeds_formula_source` |
| `tests/suggest_refuses_what_the_registry_refuses_fm4.rs` | Suggest refusal = registry tool refusal |
| `tests/a_zero_feed_is_refused_on_every_op_f185.rs` | a zero feed refuses on every op |
| `tests/command_registry_completeness.rs` | command X-macro, wire names, effects |
| `tests/hatches_are_crate_private_wp7.rs` | grep gate for `*_mut` hatches outside core |
| doctests `compute/catalog/schema.rs:694+` (F206) | every derive refusal as `compile_fail` |

Fact: the hint sentries freeze the current answer, NONE included. They do not ask whether the
op has a radial or axial value that the calculator needs.

## 4. The derive crate (`crates/rs_cam_derive`)

`crates/rs_cam_derive/src/lib.rs` (537 lines), proc-macro, deps `syn 2` (full), `quote`,
`proc-macro2`.

What it generates today:

- `#[derive(OpParams)]` (`lib.rs:65-72, 277-351`): `impl crate::compute::catalog::OpParams for
  <Config>` with `const PARAMS: ParamTable { defs, driver_set, skipped, fields }`, plus one
  `const _: () = if <kind>.is_number() { panic!(..) };` per published number that has no domain
  (`lib.rs:353-395`). Field keys: `help`, `range`, `min`, `gt`, `max`, `optional`,
  `frame_dependent`, `unbounded = "<reason>"`, `skip = <reason>`, `driver_set = <reason>`.
- `#[derive(ParamEnum)]` (`lib.rs:74-81, 485-537`): `impl ParamType` with the `enum:a|b` type
  string from the variants and the serde `rename_all` rule.
- Errors are `syn::Error` at the span of the bad attribute (`lib.rs:13-16`).
- The generated code names `crate::compute::catalog::…`, so it works only inside `rs_cam_core`
  (`lib.rs:10-11`).

What it does not do today:

- It reads no struct-level attribute. It writes no `OperationParams` impl; that is
  `impl_operation_params!` (`operation_configs.rs:3565`).
- It does not touch `OperationConfig`, the registry row, or `FeedsHints`.

Whether it can force a per-op method (fact about the mechanism, not a plan):

- A derive can refuse to compile when a struct-level attribute is missing: it returns a
  `syn::Error` at the struct span. The F185 rule already does the same for a field.
- A derive can emit an `impl SomeTrait for Config` from field attributes (as `OpParams` does), so
  it can write accessors that today come from `impl_operation_params!`.
- A derive can emit a const check that names a trait bound, for example
  `const _: fn() = || { fn need<T: CutModel>() {} need::<Config>(); };`. Then the config does not
  compile until a hand `impl CutModel for Config` exists. This forces a hand impl without
  writing it.
- A derive sees only the struct tokens. It cannot see the registry row or the generator.

## 5. Where the per-instance config reaches `feeds`

### 5.1 The doors

- `ProjectSession::suggest_slots` (`session/mod.rs:2294-2302`) builds `SuggestSlots { stock:
  StockContext, model_bbox }` (`feeds/suggest.rs:74-80`). `SuggestSlots::context`
  (`feeds/suggest.rs:85-97`) lends a `SuggestContext` with `model_bbox`, `stock`, `dressups`.
- `ProjectSession::cutter_op_profile` (`session/mod.rs:2323-2347`) passes `operation:
  &tc.operation` (the whole `OperationConfig`), the tool, machine, material, LUT, spindle
  strategy and that context to `CutterOpProfile::for_combo` (`feeds/profile.rs:68-78, 143-200`).
- `SuggestContext` (`feeds/suggest.rs:150-226`) holds `model_bbox`, `stock`,
  `upstream_leftover_stock_mm`, `neighboring_strategy_hint`, `chipload_bounds`,
  `matched_lut_row`, `effective_diameter_mm`, `calculator_operating_point`, `policy`,
  `dressups`. None of these describe the cut of the op. The doc says the v1.1 fields
  "upstream_leftover" and "neighboring_strategy_hint" are not used.

### 5.2 What the calculator receives

`feeds_input_for_operation` (`feeds/suggest.rs:986-1022`) is the one builder of `FeedsInput` for
Suggest and Explain. From the op it reads only:

- `feeds_style()` → `(family, pass_role)` (registry, static);
- `op_type()` → `operation_kind` (LUT routing);
- `operation_feeds_hints` → `axial_depth_mm`, `radial_width_mm`, `target_scallop_mm`
  (`feeds/suggest/apply.rs:1196-1206`, a tuple over `capabilities().feeds_hints`).

It does not read `stepover()`, `depth_per_pass()`, `total_depth()`, the floor, `side` or
`compensation`. The calculator then takes `(ap, ae)` from `default_engagement` of the family
profile (`feeds/mod.rs:2075-2077`), the scallop hint (`:2080-2100`), and overrides with the hints
(`:2103-2108`).

### 5.3 Where the op's own values are read later

- `enforce_invariants` reads `operation.stepover()` / `depth_per_pass()` after the calculator
  values are written to a scratch copy (`feeds/suggest/invariants.rs:224, 410, 495, 566, 607`).
- `ApplyScope::Speeds` restores the operator's stepover and depth on the scratch
  (`feeds/suggest/apply.rs:233-240`); `CutGeometry` and `Both` keep the calculator values
  (`apply.rs:150-183`, the overwrite the review found).
- `aggressiveness.rs:201, 345` reads `depth_per_pass()` and `total_depth()`.
- `axial_envelope.rs:152, 184` reads `stepover()` and the axial hint.
- `predict.rs:354-370, 712-723` reads `depth_per_pass()`, `stepover()`, `move_count`.
- `operating_point.rs:289-306` (`resolve_operating_point`) reads `stepover()` and
  `depth_per_pass()`; with neither and no fallback it returns `NoRadialEngagement` /
  `NoDepthPerPass`. Profile, Trace and Project Curve have no `stepover()`.

## 6. Suggestions (not facts)

1. Put the cut on the type, not in a hint bag. Add one per-op method that returns a typed cut,
   for example `fn cut(&self, tool: &ToolConfig) -> CutModel`, where `CutModel` is an enum with
   no `Default`: `Area { ae: RadialEngagement, ap: AxialStep, total: DepthTotal }`,
   `Slot { ap, total }` (ae = D by construction), `LineFollow { offset, ap, total }`,
   `Surface { ae: Scallop | Stepover, ap: SurfaceDriven | ZStep(f64) }`, `VGroove { depth }`,
   `Plunge { peck, depth }`, `Helix { pitch, hole_d }`. A full slot is then a variant, not a
   missing number.
2. Make the method required at compile time. Two ways that fit what exists:
   - add a required clause `cut:` to `impl_operation_params!` with no `none` form; or
   - make `#[derive(OpParams)]` emit the trait-bound const check of section 4, so a config
     without `impl CutModelSource` does not compile.
   Do not give the trait a default body.
3. Feed `FeedsInput` from that method in `feeds_input_for_operation`, and delete
   `FeedsHints` (or make it a projection of `CutModel`). Remove `Default` from whatever replaces
   it, and remove the `..FeedsHints::NONE` / `..row` pattern for this field.
4. Split "the operator stated it" from "Suggest may choose it" in the type, for example
   `Stated(f64)` vs `Open`. `apply_feeds_subset` may write only an `Open` value. Then the
   overwrite at `feeds/suggest/apply.rs:150-183` cannot compile against a `Stated` depth.
5. Add `clippy::wildcard_enum_match_arm = "deny"` in `Cargo.toml`, or at least in `feeds/` and
   `compute/catalog`, and fix `feeds/suggest/apply.rs:1105`. This closes the `_` hole that
   `match_wildcard_for_single_variants` leaves.
6. Fix the `none` answers that hide geometry (Project Curve `total_depth`, VCarve, Chamfer,
   3D Rough), or let the new `CutModel` own the total.
7. Replace the frozen hint table sentries with a sentry that asserts each op's `CutModel`
   carries the operator's own config values (set a field to 0.437, read it back through the cut),
   in the style of `the_feeds_hints_read_the_ops_own_fields`.
8. Correct the stale doc at `catalog.rs:110-113` ("Every row is `Some` today").
