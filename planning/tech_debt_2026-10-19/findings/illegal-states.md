# Findings: illegal-states (representable illegal states, validate-not-parse)

Baseline 99b6fb41, branch geo-select. Read-only sweep. Facts come from `rg`
and file reads. Each claim cites `path:line`.

In-flux rule: a finding is "in flux" when one or more cited sites is in
`instruments/in_flux_paths.txt`. Each finding names its in-flux sites.

Order: tier, then severity inside the tier.

Dropped after a check:

- `feeds::calculate` without `validate_tool_for_operation`. The one caller
  that skips the refusal (`feeds/explain_payload.rs:146`) is the documented
  preview path (`feeds/mod.rs:1005-1010`). This is the design.
- `vendor_lut::validate_observation -> Result<(), String>`. Only a test calls
  it, and the LUT is embedded data. Known as L16 below the 09-16 cut line.

---

## 1. An unknown post token becomes GRBL with no message (tier A)

**Claim.** The CLI `run` command and the project loader turn an unknown
post-processor token into `PostFormat::Grbl`. The operator gets GRBL G-code
for a different controller and sees no warning.

**Evidence.**

- `crates/rs_cam_cli/src/main.rs:116-118`: `--post` is a `String` with no
  `value_parser`.
- `crates/rs_cam_cli/src/run.rs:175-176`:
  `PostFormat::from_token(&args.post).unwrap_or(PostFormat::Grbl)`. No
  refusal comes before it in `run.rs` (`rg get_post_definition run.rs`: no
  hit). Compare `run.rs:285-293`: `parse_units` refuses an unknown unit.
- `crates/rs_cam_core/src/gcode/mod.rs:1511` and `:1606-1614`:
  `post_format_or_grbl` reads the project token and falls back to GRBL. Its
  doc says "This is the ONE place that decides", but `run.rs:176` decides a
  second time. The fallback pushes no `ProjectLoadWarning`.
- `crates/rs_cam_cli/src/job.rs:370-375` refuses an unknown job post before
  emission, so the job path is safe. `job.rs:1214-1215` parses the same
  string again with `unwrap_or(Grbl)` (shotgun parse; harmless today).
- Coldstart task8 lists this fallback as a silent miss for a new dialect.

**Enforcement now.** NONE.

**Fix.** Parse once at the boundary.

1. Make the clap arg `post: PostFormat` with a `ValueEnum` or `value_parser`.
2. Make the job field `post: PostFormat` through serde.
3. Make `post_format_or_grbl` an error, or a typed `ProjectLoadWarning` at
   minimum. Under the no-legacy ruling, prefer the error.
4. Delete the second `from_token` call in `job.rs:1214`.

Fix enforcement: TYPES. Cost S. In flux: `rs_cam_cli/src/run.rs`,
`rs_cam_cli/src/job.rs`, `rs_cam_core/src/gcode/mod.rs`.

---

## 2. A tool record can hold geometry that panics the GUI (tier A)

**Claim.** `ToolConfig` is a flat struct. It holds every type's geometry
beside a `tool_type` tag. The cutter constructors check the values with
`assert!`. `set_tool_param` writes any number. The GUI thread then builds the
cutter outside any `catch_unwind`. A wire input can therefore crash the GUI.

**Evidence.**

- `crates/rs_cam_core/src/compute/tool_config.rs:160-194`: `tool_type`, plus
  `corner_radius_mm` (end mill, `:171`), `corner_radius` (bull nose, `:173`),
  `included_angle`, `taper_half_angle`, `shaft_diameter`. Two corner-radius
  fields hold one concept for two types.
- `crates/rs_cam_core/src/tool/vbit.rs:36`,
  `crates/rs_cam_core/src/tool/tapered_ball.rs:46,51`,
  `crates/rs_cam_core/src/tool/bullnose.rs:29,35`: `assert!` on the angle,
  the shaft and the corner radius. The denied `clippy::panic` lint does not
  cover `assert!`.
- `crates/rs_cam_core/src/session/compute/params.rs:542-585`
  (`set_tool_param_impl`): writes `diameter`, `corner_radius`,
  `included_angle`, `taper_half_angle`, `shaft_diameter` after an `as_f64`
  check only.
- GUI-thread builders: `crates/rs_cam_viz/src/ui/properties/tool.rs:407`
  (tool preview, every draw) and `crates/rs_cam_viz/src/app/gpu_upload.rs:1073`.
  The `catch_unwind` sites are all worker or MCP-lifecycle
  (`compute/worker.rs:1499,1613,1734,1840`, `mcp_lifecycle.rs:301`).
- Example: MCP `set_tool_param {included_angle: 0}` on a V-bit, or a bull
  nose `diameter` below `2 x corner_radius`. The next tool-panel draw panics.
- The same domain is checked in five places with four rules:
  `vbit.rs:36`, `rs_cam_mcp/src/server.rs:2170-2181` (creation only),
  `feeds/geometry.rs:503`, `tool_config.rs:457-464`
  (`missing_defining_geometry`, `<= 0.0` only, no upper bound, no
  corner-vs-radius rule), and the GUI type combo
  (`ui/properties/tool.rs:145-175`), which calls
  `missing_defining_geometry`. The CLI checks presence only
  (`rs_cam_cli/src/job.rs:872-873`). Project load checks nothing
  (`rg missing_defining_geometry`: the GUI is the only production caller).

**Enforcement now.** CONVENTION (the GUI combo) plus `assert!` at run time.

**Fix.** Parse the geometry into the type.

1. Replace the flat fields with `enum ToolShape { Flat { corner_radius_mm },
   Ball, Bull { corner_radius }, VBit { included_angle }, TaperedBall {
   taper_half_angle, shaft_diameter } }`. Give each variant private fields and
   a checked constructor.
2. Deserialize with `#[serde(try_from = ...)]` so a bad project file refuses.
3. Make the cutter `new` functions return `Result`, or take the checked shape.
4. Make `set_tool_param` build a new shape and refuse on `Err`.

This deletes `normalize_geometry`, `cross_type_geometry` and
`missing_defining_geometry`. Fix enforcement: TYPES. Cost L. In flux:
`rs_cam_mcp/src/server.rs`, `rs_cam_viz/src/app/gpu_upload.rs`.

---

## 3. Positive quantities are clamped at use, not parsed at the door (tier B)

**Claim.** Operation and machine dials are bare `f64`. The declared
`ParamRange` is checked at one door (`set_toolpath_param`) only. Each kernel
then guards its own input in its own way. The kernels that forgot the guard
are live breaks.

**Evidence.**

- `crates/rs_cam_core/src/compute/catalog/schema.rs:198-224`: "Project TOML is
  deliberately not validated against it ... the generator-side guard ... is
  what catches that path."
- `crates/rs_cam_core/src/session/compute/params.rs:64-83`
  (`check_param_range`): its six callers (`:189,200,211,232,281,420`) are all
  inside `set_toolpath_param`. `ReplaceToolpathConfig` (GUI inspector, CLI
  `project.rs:1245`), `AddToolpath` and project load write the config with no
  range check.
- Break: `crates/rs_cam_core/src/finish/horizontal_finish.rs:220-222` divides
  by `params.stepover` with no guard. The declared range is `0.05..=20.0`
  (`compute/operation_configs.rs:2489`). The adapter copies it with no
  clamp (`compute/operation_configs.rs:2528-2530`). With `stepover = 0` from a project
  file, `(extent / 0.0).ceil() as usize` saturates to `usize::MAX`, and
  `+ 1` overflows. The workspace sets no `overflow-checks`
  (`Cargo.toml:70-99`). Dev panics (caught on the worker). Release wraps to
  `0` lines and gives an empty toolpath.
- Guards that exist, each different: `ops/pocket.rs:137`, `ops/zigzag.rs:104`,
  `finish/spiral_finish.rs:156`, `ops/drill.rs:211`.
- Floors at use: 84 `.max(1e-N)` calls in 40 core files (`rg -c`). Example:
  `machine/kinematics.rs:362,369,373` floors acceleration at `1e-3 mm/s2`.
  A zero acceleration from a bad import then gives a cycle time in years and
  no message. `MachineProfile`, `MachineKinematics` and `PostConfig` have no
  `validate` (map `core-io-output.md:195`).
- Coldstart task2 (both variants) names the missing range and the unchecked
  project TOML as silent misses.

**Enforcement now.** SENTRY for the setter door; NONE for the other doors.

**Fix.**

1. Validate a whole `OperationConfig` against its `ParamDef` ranges in one
   core function. Call it in `ReplaceToolpathConfig`, `AddToolpath` and
   project load. Refuse, do not clamp.
2. Better: have `#[derive(OpParams)]` emit a refined field type (for example
   `Mm<Positive>`) for a field with a strictly positive range, with
   `serde(try_from)`. A kernel then cannot receive `0`.
3. Do the same for `MachineKinematics` at import and load.

Fix enforcement: TYPES (refined fields) or SENTRY (one validator at three
doors). Cost M. In flux: `compute/operation_configs.rs`.

---

## 4. The GUI run record can say "Done" with no result (tier B)

**Claim.** `ToolpathRuntime` stores `status: ComputeStatus` and
`result: Option<ToolpathResult>` as two fields that must agree. Each writer
sets both by hand. One writer sets only one, and readers then report a
generated operation that has no geometry.

**Evidence.**

- `crates/rs_cam_viz/src/state/runtime.rs:27-38`: `status`, `result`,
  `stale_since`, and three trace fields that `ToolpathResult` also carries
  (`state/toolpath/entry.rs:181-190`).
- Paired writes: `controller/events/compute.rs:37-38, 57-58, 296-307,
  498-503 + 612, 620-621, 633-634, 640-641`.
- Break: `crates/rs_cam_viz/src/controller/events/pins.rs:279` sets
  `rt.result = None` and leaves `status`. `state/stale.rs:42-59` writes only
  `stale_since`. So the status stays `Done`.
- Readers of `Done`: `ui/status_bar.rs:71` counts it;
  `controller/events/compute.rs:139` tells the operator "That operation is
  generated".
- `crates/rs_cam_core/src/compute/config.rs:75-91`:
  `ComputeStatus::Disabled` is documented "Never stored", but the type lets a
  writer store it.
- A third truth exists: `ui/toolpath_panel.rs:1028-1037` reads
  `FreshnessState` because `Done` lies after an edit.

**Enforcement now.** CONVENTION.

**Fix.**

1. Fold the pair into `enum ToolpathRun { Pending, Computing,
   Done(ToolpathResult), AwaitingPriorStock(..), Failed(String) }`. Keep the
   traces only inside `ToolpathResult`.
2. Move `Disabled` to a separate display enum that `ComputeStatus::effective`
   returns.

Fix enforcement: TYPES. Cost M. In flux: `controller/events/pins.rs`,
`state/toolpath/entry.rs`.

---

## 5. The stock "fit to model" flag is reconciled at four doors (tier B)

**Claim.** `StockConfig` holds `auto_from_model: bool` beside explicit
dimensions. The rule is "an explicit dimension clears the flag". Core does not
own the rule. Four surfaces copy it, and core `SetStockConfig` stores any
pair.

**Evidence.**

- `crates/rs_cam_core/src/compute/stock_config.rs:67-77`: `x, y, z, origin_*,
  auto_from_model, padding`.
- `crates/rs_cam_core/src/session/mutation/config.rs:494-499`:
  `set_stock_config` stores the struct as given.
- Copies of the rule: `rs_cam_viz/src/ui/properties/stock.rs:28-48`
  (`reconcile_auto_from_model`, six fields listed by hand),
  `rs_cam_viz/src/app/mcp/commands.rs:1161-1197`,
  `rs_cam_cli/src/job.rs:1115`, `rs_cam_cli/src/smoke.rs:1122`.
- Past break, from the doc at `ui/properties/stock.rs:15-16`: an operator
  dragged a dimension under a ticked checkbox, "and the next load re-derived
  the numbers from the model".
- Coldstart task5: a new stock field must be added to the hand list at
  `stock.rs:28` or it does not clear the flag.

**Enforcement now.** CONVENTION.

**Fix.** Replace the flag with `enum StockSize { FitModel { padding },
Explicit { x, y, z, origin } }`. Let core `SetStockConfig` refit a `FitModel`
stock from the model. Delete the four copies. Fix enforcement: TYPES. Cost M.
In flux: `session/mutation/config.rs`, `app/mcp/commands.rs`,
`rs_cam_cli/src/job.rs`, `rs_cam_cli/src/smoke.rs`.

---

## 6. The height plane order is a warning, checked twice with two rules (tier B)

**Claim.** `ResolvedHeights` accepts any order of clearance, retract, feed,
top and bottom. Two checkers test the order with different rules and
severities. No export door reads either check.

**Evidence.**

- `crates/rs_cam_core/src/compute/config.rs:334-347` (`resolve`) and
  `:379-394`: public fields, no order check. `:398-400` `depth()` takes
  `.abs()`, so an inverted range gives a positive depth.
- Checker 1: `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:614-690`.
  It compares against `anchored_top_z` (the stock top for
  `top_on_stock_box` ops, `:606-612`). "Clearance below retract" is
  `Caution`.
- Checker 2: `crates/rs_cam_viz/src/ui/properties/tab_badges.rs:38-45`. It
  compares against raw `h.top_z`. "Clearance below retract" is `DANGER`. So
  the GUI badge and MCP diagnostics can disagree on an Adaptive3d with a
  pinned top.
- A second struct has the same name:
  `from_static_checks.rs:58` `ResolvedHeights`. Its `from_context`
  (`:91-101`) projects the pins away, so three of the four order checks
  cannot fire on its output. `rg` finds no production caller (tests only).
- Export: `session/compute/export.rs:59-75` calls `export_gcode_checked`,
  which gates on tool load only (`gcode/mod.rs:380-384`). `rg
  "Severity::Critical"` finds no export reader. A "Bottom Z above Top Z"
  `Critical` row does not stop a cut.

**Enforcement now.** CONVENTION (two hand checks).

**Fix.**

1. Make `HeightsConfig::resolve` return a `ResolvedHeights` with private
   fields plus a typed `Vec<HeightOrderViolation>`, computed once in core.
2. Make the GUI badge and the diagnostic adapter read that list.
3. Delete `from_context`. Rename or merge the diagnostics struct.
4. Ask the operator if an inverted stack must refuse at export.

Fix enforcement: TYPES. Cost M. In flux: `compute/config.rs`,
`ui/properties/tab_badges.rs`.

---

## 7. Project and config files accept unknown keys (tier B)

**Claim.** The project file, the post TOMLs, the machine TOMLs and the tool
library deserialize with serde defaults and no `deny_unknown_fields`. A
misspelt key loads as the default value with no message. A renamed key is
caught only when a contributor adds it to a hand-kept denylist.

**Evidence.**

- `rg deny_unknown_fields`: five production uses only
  (`rs_cam_mcp/src/server.rs:1085`, `rs_cam_cli/src/job.rs:146`,
  `selection/rule.rs:187`, `material/wood_species_library.rs:29,54`). None is
  on `ProjectFile`, `OperationConfig`, `PostDefinition` or `MachineProfile`.
- `crates/rs_cam_core/src/session/project_file.rs:85-182`: `RETIRED_KEYS`,
  13 entries, plus `RETIRED_VALUES` (`:183`). `check_retired_keys`
  (`:230-262`) refuses only the listed keys. A rename that is not listed
  drops the old value silently.
- Coldstart task2 and task3: a new field without `serde(default)` breaks old
  files. The inverse also holds: a field read under a wrong name gives the
  default.

**Enforcement now.** CONVENTION (the denylist).

**Fix.** Under the no-legacy ruling, put `#[serde(deny_unknown_fields)]` on
every persisted struct. Then delete `RETIRED_KEYS` and `RETIRED_VALUES`.
Check `flatten` and internally tagged enums first, because serde does not
combine those with `deny_unknown_fields`. Fix enforcement: TYPES (serde
refuses). Cost M. In flux: `session/project_file.rs`.

---

## 8. The dressup wire keeps the bool-plus-value shape (tier B)

**Claim.** CUT-13 moved five dressups to `Option<Params>` in memory. The wire
struct keeps one `bool` beside each value. Two string tables with `_` arms
map the value keys to their owners. `feed_optimization` keeps the flat
bool-plus-two-values shape.

**Evidence.**

- `crates/rs_cam_core/src/compute/config.rs:897-929`: `DressupConfig`.
  `:920-925`: `feed_optimization: bool`, `feed_max_rate`, `feed_ramp_rate`,
  left flat because "another session owns" a `tool_load/` fixture.
- `config.rs:931-969`: `DressupConfigWire` exists "so CUT-13 can fix the
  in-memory type without moving a single wire key".
- `config.rs:1191-1200` (`owner_of_value_field`) and `:1220-1229`
  (`is_dressup_enabled`): string matches that end `_ => None` and
  `_ => false`.
- Coldstart task3 (both variants): a new dressup that is missing from these
  tables lets `set_dressup_field` accept a value while the dressup is off. The
  value is then dropped at the `Option` boundary.
- Changed since CUT-13: the operator ruled that breaking wire changes are fine
  (2026-09-16). The reason to keep the wire shape no longer holds.

**Enforcement now.** CONVENTION.

**Fix.**

1. Serialize `Option<Params>` directly. Delete `DressupConfigWire`.
2. Fold `feed_optimization` into `Option<FeedOptimizationParams>`.
3. Replace the two string tables with a typed field enum
   (`DressupField::Dogbone(DogboneField)`), so the owner comes from the type.

Fix enforcement: TYPES. Cost M. Known id: CUT-13
(`planning/design_audit_2026-09-17/SYNTHESIS.md:323,515,725`). In flux:
`compute/config.rs`.

---

## 9. Ordered angle pairs are independent fields (tier E)

**Claim.** Slope bands and band thresholds are two fields, each with its own
range. Nothing checks that the low end is below the high end. An inverted
band admits nothing, and a kernel patches one case by hand.

**Evidence.**

- `slope_from` and `slope_to`, each `range = 0.0..=90.0`:
  `compute/operation_configs.rs:1041-1044` (DropCutter), `:1602-1605`
  (Scallop), `:2304-2307` (RampFinish).
- `compute/catalog.rs:910-919` and `finish/finish_setup.rs:635-637`:
  `slope_filter_active` returns `true` for an inverted band. The mask
  then admits no angle (`finish/ramp_finish.rs:773`,
  `finish/scallop/research.rs:477`).
- `finish/finish_planner.rs:465-467`: "Very-steep must stay a subset of steep
  even under pathological dials (e.g. waterline_threshold_deg <
  steep_threshold_deg)". The kernel repairs the illegal state.
- `rg "slope_from\s*>=?\s*\w*slope_to"`: no order check anywhere.

**Enforcement now.** NONE.

**Fix.** Add `SlopeBand { from_deg, to_deg }` with a checked constructor and
`serde(try_from)`. Use one type for every band pair. Fix enforcement: TYPES.
Cost M. In flux: `compute/operation_configs.rs`, `compute/catalog.rs`.

---

## 10. Registry adapters re-check the variant and the geometry at run time (tier E)

**Claim.** The operation registry hands each adapter the whole
`OperationConfig` and a `Prepared` value. Each adapter re-matches its variant
and asks for its geometry. A mismatch is an `Err` at run time, not a compile
error.

**Evidence.**

- `crates/rs_cam_core/src/compute/execute.rs:510-523`: `config_guard!`
  returns `OperationError::Other("registry adapter mismatch ...")`. 24 uses
  across `compute/execute*` (`rg -c`).
- `crates/rs_cam_core/src/compute/execute/prologue.rs:114-185`:
  `Prepared::{regions, curves, holes, mesh}` return `Err` "only when an
  adapter asks for geometry its own row does not require". 13 call sites.

**Enforcement now.** NONE at compile time. A wrong row fails on first run.

**Fix.** Have `for_each_op!` generate a typed adapter signature per row, for
example `fn(&PocketConfig, &Prepared<RegionsReq>)`. `Prepared<G>` carries the
checked geometry for its requirement. Then the guard and the accessor errors
disappear. Fix enforcement: TYPES. Cost M. In flux: `compute/execute.rs`,
`compute/execute/prologue.rs`.

---

## 11. Toolpath ownership is stored by position in three places (tier E)

**Claim.** The session keeps toolpaths in one flat `Vec`. Each setup keeps
indices into it, and results are keyed by index. Every structural mutation
re-keys all three by hand. Nothing checks that each index is in exactly one
setup. The project file nests toolpaths under setups, so the illegal state
exists only in memory.

**Evidence.**

- `crates/rs_cam_core/src/session/mod.rs:767-768`:
  `toolpath_indices: Vec<usize>`. `session/mod.rs:1322`:
  `results: HashMap<usize, ToolpathComputeResult>`.
- Hand re-keys in `crates/rs_cam_core/src/session/mutation/toolpath.rs`:
  `:63` (add), `:94-114` (remove: decrement the setup indices, rebuild the
  results map), `:175-179` (move), `:568-573` (move to setup). 54 non-test
  references to `toolpath_indices` (`rg`).
- The file form nests toolpaths: the retired-value path
  `["setups", "[]", "toolpaths", "[]", "boundary"]`
  (`session/project_file.rs:183-186`).
- Ruling at `crates/rs_cam_core/src/ids.rs:7-8`: "Indexes stay `usize` on
  purpose — do not newtype them." This finding keeps that rule. The defect is
  the unchecked partition, not the `usize`.

**Enforcement now.** CONVENTION.

**Fix.** Make each setup own its toolpath list (`Vec<ToolpathConfig>` or
`Vec<ToolpathId>`). Key results by `ToolpathId`. A flat index stays a
computed address for the command surface. Fix enforcement: TYPES. Cost L.
In flux: `session/mutation/toolpath.rs`, `session/mod.rs`.

---

## 12. Selection ranges admit inverted and negative values (tier E, selection programme)

**Claim.** The new selection rule types `ZRange` and `DiameterRange` have
public fields and no checked constructor. An inverted `ZRange` or a negative
tolerance matches no item, and nothing says why.

**Evidence.**

- `crates/rs_cam_core/src/selection/rule.rs:44-52` (`DiameterRange`,
  `contains` reads `abs() <= tolerance_mm`) and `:79-88` (`ZRange`).
- `rg "ZRange|DiameterRange"` finds no order or sign check in `selection/` or
  in `rs_cam_viz/src/app/mcp/commands/selection.rs`.
- Good contrast in the same module: `Item` has private fields and one
  checked constructor (`selection/item.rs:245-300`).

**Enforcement now.** NONE.

**Fix.** Give both types private fields, a checked `new`, and
`serde(try_from)`, as `Item` does. A programme note, not debt. Fix
enforcement: TYPES. Cost S. In flux: `selection/rule.rs`.
