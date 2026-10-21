# Geometry selection P7: templates and job types (design, 2026-10-22)

Status: DRAFT for the lead. Author: the P7 design agent (read-only).
Inputs: `PLAN.md` (P7 scope, the operator rulings of 2026-10-16 and
2026-10-19, "P7: all four job types", every lead ruling up to P6.5b),
`REQUIREMENTS.md` (R1, R2, R8, R9, S14, S20, S21, S25, S26, the job table),
`DESIGN.md` (section 15 row P7, D1, D12), `GAP_ANALYSIS.md`,
`P6_DESIGN.md` (format model), and the code on `master` at 3c4d4aba.
Another session can edit files in the tree. Line numbers can move. While
this file was written, a peer change (F013) made `ResolvedSelection`
generic over its frame (`ResolvedSelection<ModelFrame | SetupFrame>`,
`selection/CLAUDE.md`); the P7.4 report must take the `SetupFrame` form.

Conventions (as P3, P4 and P6): **NEW** marks a name that does not exist
today. Core paths start at `crates/rs_cam_core/src/`, viz paths at
`crates/rs_cam_viz/src/`, CLI paths at `crates/rs_cam_cli/src/`. **[OP]**:
the operator rules it (it changes what is machined or the user workflow
materially). **[LEAD]**: structure, UI or wire; the lead rules it. Each
item is one editor session.

---

## 0. Summary

P7 adds one mechanism: the **template**. A template is a list of op
recipes. Each recipe holds an op config whose selection holds rules only,
the toolpath fields that carry no project id, and a **tool slot** that
matches a tool by its properties. A **job type** (sign, sheet parts, inlay,
flatten) is a built-in template with a beginner description. There is no
second mechanism for job types.

1. **Draft, then one apply.** A wizard holds its own draft (the pin-wizard
   pattern). The draft reads one core report. Apply sends ONE core
   command, `ApplyTemplate`, which writes ordinary ops and ordinary tools
   in one `Effects` window (the `PlanMultitoolFinishing` pattern). After
   apply, no op knows the template (R9).
2. **Nothing silent.** A missing layer is named before apply and refuses
   at apply unless the draft maps it or turns the op off (S21). A slot
   with no matching tool refuses. A rule that selects nothing turns its op
   off in the draft, with a note.
3. **One door for every surface.** The GUI wizard, MCP and the CLI read
   the same report (`TemplateReport`) and send the same command.
4. **Save as template** (S20) is in scope: it is the cheapest way to make
   a template, and it is the reverse of apply through the same types.

| # | Item | Output change | Visible result |
|---|---|---|---|
| P7.1 | A nesting filter and a layer exclude on a rule (`Filter.nesting`: outermost or inside; `Filter.not_layers`) | none (new filters only) | `+ Rule` offers "outer loops" / "inner loops" and "not on layer" |
| P7.2 | Core template model: `Template`, `TemplateOp`, `TemplateSelection`, `ToolSlot`, `Derivation`; TOML with `format_version = 1` | none | none |
| P7.3 | The template library folder (`RS_CAM_TEMPLATE_DIR`, `[paths] template_library`) | none | Preferences shows the folder |
| P7.4 | The `template_report` query: per op counts, missing layers, tool candidates, refusals | none | none (read only) |
| P7.5 | The `ApplyTemplate` command and `Effects::applied` | new ops only | none until P7.8 |
| P7.6 | Built-in job types (sign, sheet parts, inlay pocket, inlay plug, flatten) and the inlay floor derivation | new ops only | none until P7.8 |
| P7.7 | Save as template (from the ops of a setup) | none | none until P7.8 |
| P7.8 | GUI: the template wizard, its entry points, undo | none | sketches in section 5 |
| P7.9 | MCP and CLI parity, docs, catalog | none | `rs_cam template` subcommand |

---

## 1. What exists (question 1)

| Piece | Where | Fact |
|---|---|---|
| `Selection` | `selection/rule.rs:264` (private fields), `:375` `drawing`, `:387` `empty`, `:396` `stock`, `:410` `closed_shapes_of`, `:460` `from_rules`, `:519`/`:523`/`:527` `rules`/`picks`/`excludes` | rules, picks (`MatchKey`), excludes; NO `Default`: each op states its own |
| `Rule`, `Filter` | `selection/rule.rs:185` `Rule { source, filter }`; `:129` `Filter { layers, kinds, diameter, face }`; `:24` `KindFilter::{Served, Only}`; `:44` `DiameterRange`; `:121` `FaceFilter` | a filter reads ONE item (`Filter::admits`, `:147`); no filter reads the containment of an item |
| `SourceRef` | `selection/item.rs:32` | `ToolpathModel`, `Model(ModelId)`, `Stock`; `for_toolpath_model` (`:55`) stores a pick on the own model as `ToolpathModel` |
| `ItemKind` | `selection/item.rs:89` | `Loop`, `Path`, `Point`, `Circle`, `Face` |
| Layers | `selection/item.rs:531` `ItemSet::layers` | sorted, deduplicated layer names of a model |
| Resolution | `selection/resolve.rs:28` `ResolveScope`; `:159` `ResolvedSelection`; `:170` `unmatched_layers` (S21); `:533` the rule-layer check | pure; reports, never refuses |
| `UnknownLayer` | `selection/view.rs:402`; `selection/refusal.rs:356` | `view` refuses a rule layer that names no item (lead ruling P2.2: "S21: a missing layer is named, never ignored") |
| Item roles | `selection/view.rs:186` `ItemRole::{Cut, Island, Skipped}` | `selection_status` reads them (P4.4) |
| `ItemUse` | `compute/catalog/capabilities.rs` (P6_DESIGN section 1) | `None`, `Regions`, `Curves`, `Holes { .. }`; `OperationConfig::capabilities` (`compute/catalog.rs:831`) overrides per config |
| Op list | `compute/catalog.rs:343` `for_each_op!` | 25 rows; 13 `Menu2d` (Bore included), 11 `Menu3d`, 1 `SystemOnly` (`AlignmentPinDrill`, `:372`) |
| `OperationConfig` | `compute/catalog.rs:798` | `#[serde(tag = "kind", content = "params")]`: the project-file and job-file shape of an op |
| Selection slot | `compute/catalog.rs:1111` `selection`, `:1144` `selection_slot`, `:1178` `default_selection` | exhaustive; a mesh op has no slot |
| Tool rule | `compute/catalog.rs:515` `OperationType::tool_refusal`; `compute/catalog/schema.rs:436` `ToolConstraintsDef { required_kinds, supports_v_bit }` | the prologue, the static diagnostic and the GUI read one text |
| Floor | `compute/floor.rs:67` `Floor::{Top, StockTop, StockBottom, ModelTop, EachFace, Z}` | D9: one floor field that names its reference (Pocket, Profile, Zigzag, Adaptive 2D, Bore) |
| Tools | `compute/tool_config.rs:310` `ToolConfig` (`tool_type`, `diameter`, `flute_count`, `included_angle`, `corner_radius`, …); `:33` `ToolType` (5 kinds; no drill kind) | a drill is an `EndMill` |
| Tool library | `io/tool_library.rs:78` `ToolCatalog { tools }`; `:84` `library_dir`; `:346` `dedupe_key` | TOML catalogues, a tool is COPIED into the project; the catalogue file has NO version key |
| Library folders | `settings/paths.rs:110` `library_dir_from` (env > `[paths]` > `<config>/<name>`); `:128` tool, `:138` machine; `settings/mod.rs:281` `PathSettings` | one resolver for GUI, MCP and CLI |
| Project file | `session/project_file.rs:47` `SUPPORTED_FORMAT_VERSION = 3`; `:855` `ProjectToolpathSection`; `:94` `RETIRED_KEYS`; `:256` `RETIRED_OP_ENTRIES`; `:317` `check_retired_keys` | retired-key paths are rooted at `setups[].toolpaths[]` |
| `ToolpathConfig` | `session/mod.rs:1153` | 18 fields; `tool_id`/`model_id` exact (F053, P2.5); `planner_origin` (`:1190`) |
| Id fields in configs | `compute/config.rs:680` `DerivedRestRegions.source_toolpath_id`; `:709` `PlannedTierRegions.tool_ids`; `:822` `RestAnalysisConfig.reference_tool_id`; `compute/operation_configs.rs:1715` Pencil `reference_tool_id`; `:2905` Project Curve `surface_model_id`; `:777` Profile `tab_points` (model-frame points) | each one is project-specific |
| Add toolpath | `session/mutation/toolpath.rs:26` `add_toolpath` (runs `floor_conflict` and `domain::validate_toolpath`), `:52` `add_toolpath_impl` (raw append) | the raw half runs inside a caller's window |
| Add tool | `session/mutation/entities.rs:107` `add_tool`, `:119` `add_tool_impl` | `AddToolArgs` (`session/command.rs:1670`): "the catalog READ is a surface-side step" |
| `Effects` | `session/command.rs:1232`; `created` (one index); `planned` (`:1293`, `MultitoolPlanOutcome` only) | WP3: one construction site, `try_with_effects` (`:3087`) |
| One-command multi-op precedent | `session/multitool.rs:348` `plan_multitool_finishing` | validate (`&self`), resolve (`&self`), build in ONE window, outcome on `Effects::planned`; feeds through `suggest_feeds_for` (`:880`) |
| Feeds door | `session/command.rs:2859` `ApplyFeeds`; `session/mod.rs:2062` `suggest_slots` | F114: the one Apply-Suggest row |
| Selection report | `session/selection_status.rs:517` `selection_status` (memo, by toolpath id); `:565` `build_selection_report(tc, target, ctx)` (no memo); `session/eval_context.rs:140` `SetupEvalContext::build_for_setup` | the builder takes a `ToolpathConfig`, so a candidate op can be reported |
| Pin wizard (draft pattern) | `ui/pin_wizard.rs:16–23` (doc), `:90` `PinWizardState` (own draft), `state/panels.rs:183` `panels.pin_wizard`; `ui/mod.rs:180` `AppEvent::PinWizardApply`; `ui/properties/mod.rs:1224` opens it; `app.rs:1115` draws it | Cancel changes nothing; Apply sends ONE GUI event; every number is the core report of the draft (`session/pin_report.rs:160` `pin_set_report_for`) |
| Pin wizard apply | `controller/events/pins.rs:317` `apply_pin_wizard` | a CONTROLLER sequence: `AddTool` rows, `SetAlignmentPins`, binds. The doc (`:311–315`): tool and toolpath adds are outside the undo history |
| Planner dialog (GUI → one command) | `ui/multitool_planner.rs:1–33`, `:833` `AppEvent::ApplyMultitoolPlan`; `controller/events/mod.rs:565` | Preview is a read; Close changes nothing; one Apply; no undo |
| Undo | viz `state/history.rs:22–84` `UndoAction` | no variant adds or removes a toolpath or a tool |
| Add menu | `ui/toolpath_panel.rs:1133` `+` menu, `:1146` empty-state "Add toolpath", `:1184` `add_menu_offers` | no template entry |
| CLI job file | `job.rs:73` `JobFile`, `:162` `ToolDef`, `:208` `OperationDef` (W5 params, `selection`, `floor`), `:254` `JOB_KEYS` | an op recipe with a tool by KEY and one `input` file per op; no property match |
| MCP views | `mcp_server.rs:1852` `set_ui_view` (`modal: 'pin_wizard'` …) | a modal list |
| Template, preset, job type | `rg -i "template\|job_type\|preset"` | NONE for ops. Hits are G-code templates (`gcode/`), the export file name (`state/wizard.rs:80`), the pin presets, and `pin_drill_tool_template` (`session/pins.rs:529`) |
| Inlay | `compute/catalog.rs:351–352` rows; `compute/operation_configs.rs:976` `InlayConfig` (help text: the floor Pocket recipe), `:1074` `InlayPlugConfig` ("own project, on the plug board"); `ops/inlay.rs:1–45`, `:151` `mirror_design` | ruling R2 (`planning/op_architecture_2026-10-14/PLAN.md:123`): walls only; floor = a normal Pocket; plug = own project |
| Face | `compute/operation_configs.rs:226` `FaceConfig` (`depth`, `stock_offset`, `selection` default `Selection::stock()`) | `is_stock_based` (`compute/catalog.rs:533`): no model needed |
| Drill size | `compute/operation_configs.rs:369` `DrillConfig.size_tolerance`; `selection/key.rs:38` `CAD_ROUNDING_MM = 0.01` | P6.5/P6.5b: a Drill cuts only circles of the tool size |

### 1.1 Found on the way (code reading, not run)

- **F-A (pattern).** The task names the pin wizard as "own draft, one apply
  event". That holds for the GUI only. Its apply is a controller sequence
  of commands (`controller/events/pins.rs:317`), so MCP and the CLI cannot
  reach the same path. P7 takes the DRAFT pattern from the pin wizard and
  the CORE apply pattern from `PlanMultitoolFinishing`
  (`session/multitool.rs:348`). F114 (`ApplyFeeds`) is the written reason:
  a multi-step act became one row so that every surface writes one value.
- **F-B (selection gap).** `Filter` reads one item (`selection/rule.rs:147`).
  No rule can say "the outer loops" or "the inner loops". A layer-free DXF
  (most sign and sheet-part drawings) cannot split a part outline from its
  cutouts. Profile cuts only the exterior of each region:
  `profile_contour_reported` keeps the first result's exterior
  (`ops/profile.rs:160–168`). So a sheet-part cutout needs its own
  selection. P7.1 adds the filter.
- **F-C (report wording, for the lead).** A hole ring of a Profile
  Outside/Inside region gets `ItemRole::Island`, documented as "the op
  cuts round it" (`selection/view.rs:189–191`). Profile does not cut round
  it (F-B). Check `item_states` for a Profile on a part with a cutout.
- **F-D (ids).** Six config fields hold a project id or a model-frame
  point (table row "Id fields"). A template that copied them would point
  at nothing in the next project. P7.2 refuses them by a destructure.
- **F-E (inlay recipe).** The Inlay help text tells the user to build the
  floor Pocket by hand, with a boundary offset of
  `−pocket_depth × tan(half the V angle)` (`compute/operation_configs.rs:978–985`).
  That number depends on the V-bit. The inlay job type is the first
  template value that is a formula of the bound tool (section 4.3).

---

## 2. The template data model (question 2)

### 2.1 The types (NEW folder `template/`, with its own `CLAUDE.md`)

```rust
// template/mod.rs (all NEW)

/// A template: op recipes in cut order, and the tool slots they use.
pub struct Template {
    pub name: String,
    pub description: String,
    pub origin: TemplateOrigin,          // BuiltIn(JobType) | Library(file stem); not in the file
    slots: Vec<ToolSlot>,                // private: `Template::new` checks every op names a slot
    ops: Vec<TemplateOp>,
    derivations: Vec<Derivation>,        // section 4.3; empty except the inlay pocket
}

/// One op recipe. Every field of `ToolpathConfig` that carries no project
/// id. `TemplateOp::into_config` destructures `ToolpathConfig` with no
/// `..`, so a new field does not compile until it is decided here.
pub struct TemplateOp {
    pub name: String,
    pub enabled: bool,
    pub slot: SlotKey,                   // the tool slot, by key
    pub operation: OperationConfig,      // its selection is a TemplateSelection (2.2)
    pub dressups: DressupConfig,
    pub heights: HeightsConfig,
    pub boundary: BoundaryConfig,        // source: Stock | ModelSilhouette | Selection(rules on ToolpathModel)
    pub boundary_inherit: bool,
    pub stock_source: StockSource,
    pub coolant: CoolantMode,
    pub rest_analysis: RestAnalysisConfig, // reference_tool_id must be None
    pub shoe: ShoeSetting,
    pub feeds: FeedsAtApply,             // O-1: Suggest | AsSaved
    // NOT held: id, tool_id, model_id, pre_gcode, post_gcode,
    // debug_options, feeds_provenance, planner_origin (L-4).
}

/// A tool chosen by its properties (R8).
pub struct ToolSlot {
    pub key: SlotKey,                    // "cutter", "vbit", "pin_drill"
    pub label: String,                   // "Profile cutter"
    pub matcher: ToolMatch,
}

pub struct ToolMatch {
    pub tool_type: ToolType,             // exact
    pub diameter: Option<RangeMm>,       // both ends included
    pub flutes: Option<RangeU32>,
    pub included_angle: Option<RangeDeg>,// V-bit only
    pub corner_radius: Option<RangeMm>,  // bull nose only
    pub prefer: Prefer,                  // tie-break, section 2.3
}
pub enum Prefer { Nearest, Largest, Smallest }   // Nearest = nearest to the range middle
```

`ToolMatch` reads `ToolConfig` fields only (`compute/tool_config.rs:310`).
No new tool property is needed. A Drill slot is `tool_type = end_mill`
with the hole diameter as its range (no drill `ToolType` exists).

### 2.2 Rules, not item keys

```rust
// template/selection.rs (NEW)
/// A selection that holds rules only, on the op's own model or the stock.
/// It cannot hold a pick, an exclude or `SourceRef::Model(id)`: the
/// constructor refuses them, and serde reads through the constructor.
pub struct TemplateSelection(Selection);
impl TemplateSelection {
    pub fn new(sel: Selection) -> Result<Self, TemplateRefusal>;
    pub fn into_selection(self) -> Selection;
}
```

- A rule source is `SourceRef::ToolpathModel` or `SourceRef::Stock`.
  `Model(id)` refuses (`TemplateRefusal::ModelSource`): an id has no
  meaning in the next project.
- A pick or an exclude refuses (`TemplateRefusal::Pick`): a `MatchKey` is
  the geometry of one drawing (R2).
- The op's `OperationConfig` holds a plain `Selection` (D1). So
  `TemplateOp::new` checks `operation.selection()` through
  `TemplateSelection::new`, and the same for a `BoundarySource::Selection`
  boundary. `TemplateOp` has private fields and one constructor; the check
  cannot be skipped (compiler-enforced: no public field write).
- Rules keep their layer names. The draft edits them (the layer map,
  section 3.3). A template has no "layer slot" vocabulary: that would be
  template-only state, which R9 forbids.
- Examples of rules a template holds (all exist today, except nesting):
  - every closed loop on layer `CUT`: `Filter { layers: ["CUT"], kinds: Only([Loop, Circle]) }`;
  - circles of Ø 5 ± 0.1: `Filter { kinds: Only([Circle]), diameter: Some(5.0 ± 0.1) }`;
  - the outer loops (P7.1): `Filter { kinds: Only([Loop]), nesting: Some(Outermost) }`;
  - the stock (Face): `Selection::stock()`.

### 2.3 The tool match

```rust
// template/tool_match.rs (NEW, pure)
pub struct SlotCandidates {
    pub slot: SlotKey,
    pub project: Vec<usize>,            // tool indices, in preference order
    pub library: Vec<LibraryTool>,      // { catalog, row, tool }, in preference order
}
pub fn match_slots(t: &Template, project: &[ToolConfig], library: &[LibraryTool]) -> Vec<SlotCandidates>;
```

| Matches | Draft | Apply (MCP, CLI with no binding) |
|---|---|---|
| 1 or more project tools | the first in preference order, a chooser shows the rest | that tool |
| 0 project tools, 1 or more library tools | "Add <name> from <catalog>" | the tool is added (a note in the outcome) |
| 0 anywhere | the slot row is red: "No tool matches: end mill, Ø 6.0–6.35 mm"; the ops of the slot cannot apply | refuses `NoToolForSlot { slot, matcher text }` |

- The order: `prefer` first (`Nearest`: the absolute distance of the
  diameter to the range middle; `Largest`; `Smallest`), then the lower
  `tool_number`, then the lower index. The order is total, so two surfaces
  give one pick.
- The user can bind any project tool in the draft, also one outside the
  match. Apply still runs `OperationType::tool_refusal`
  (`compute/catalog.rs:515`) for each op of the slot and refuses by op.
- One slot is ONE tool for all its ops: ops that share a slot share the
  tool after apply.
- The library read stays surface-side (as `AddToolArgs`). The surface
  reads every catalogue of `io::tool_library` and passes the tools in.

### 2.4 Storage

- One template per file, in a NEW library folder (as the machine library,
  one file per machine): `RS_CAM_TEMPLATE_DIR` > `[paths] template_library`
  > `<config>/rs_cam/templates`. NEW `settings::paths::template_library_dir_from`
  through `library_dir_from` (`settings/paths.rs:110`); NEW field
  `PathSettings::template_library` (`settings/mod.rs:281`). NEW
  `io/template_library.rs` on `io/named_toml_library.rs` (list, load, save,
  name check).
- Built-in job types are Rust constructors in `template/builtin.rs`
  (L-8). They return the same `Template` type. A config change that breaks
  one does not compile, or a sentry fails (P7.6). The user can "Save a
  copy" of a built-in; the copy is an ordinary library file.

### 2.5 The file format and its version

```toml
format_version = 1
name = "Cabinet side, 18 mm ply"
description = "Shelf pins, dados, cutouts, then the outline with tabs."

[[slots]]
key = "pin_drill"
label = "Shelf pin drill"
tool_type = "end_mill"
diameter = { min = 4.9, max = 5.1 }
prefer = "nearest"

[[slots]]
key = "cutter"
label = "Profile cutter"
tool_type = "end_mill"
diameter = { min = 6.0, max = 6.35 }
prefer = "largest"

[[toolpaths]]
name = "Shelf pins"
slot = "pin_drill"
feeds = "suggest"
operation = { kind = "drill", params = { depth = 12.0, selection = { rules = [
  { source = "toolpath_model", filter = { kinds = { only = ["circle"] },
    diameter = { nominal_mm = 5.0, tolerance_mm = 0.1 } } } ] } } }
# … the other [[toolpaths]] in cut order
```

- The `operation`, `dressups`, `heights` and `boundary` tables are the
  project-file shapes (`ProjectToolpathSection`, `session/project_file.rs:855`).
  One vocabulary for an op on every file kind.
- `format_version = 1`. The loader refuses any other value and a file with
  no key, with a message that names the version (as F034 for projects).
  No conversion and no alias (ruling 2026-09-16).
- The retired keys: the toolpath part of `RETIRED_KEYS` and all of
  `RETIRED_OP_ENTRIES` apply to each `[[toolpaths]]` table. NEW
  `project_file::check_retired_toolpath_keys(value, at)` takes the
  toolpath-relative path of each row (the rows rooted at
  `setups.[].toolpaths.[]` lose that prefix). The project loader and the
  template loader call it. One list, so a key that leaves the project
  format also leaves the template format (L-7).
- `deny_unknown_fields` on every template table, as F034.

---

## 3. Apply (question 3)

### 3.1 The command

```rust
// session/command.rs, a NEW Command row:
// (Command, ApplyTemplate, "apply_template", ApplyTemplateArgs, Effects, surfaces: GUI, MCP, CLI)

pub struct ApplyTemplateArgs {
    pub template: Box<Template>,          // the DRAFT: the edited template
    pub setup_index: usize,
    pub model_id: ModelId,                // the toolpath model of every op
    pub bindings: Vec<SlotBinding>,       // one per slot, slot order
    pub ops_on: Vec<bool>,                // one per op; false = not added
}
pub enum SlotBinding {
    Existing(usize),                      // a project tool index
    Add(Box<ToolConfig>),                 // a library tool, copied in (id reassigned)
}
```

`ProjectSession::apply_template` (NEW, `session/template.rs`) runs the
`plan_multitool_finishing` order (`session/multitool.rs:348`):

1. **Validate** (`&self`): the setup and the model exist (exact ids,
   P2.5); `bindings.len()` = slots, `ops_on.len()` = ops; every `Existing`
   index exists; every `Add` passes `ToolConfig::validate` (F029).
2. **Resolve** (`&self`): for each op that is on, build the candidate
   `ToolpathConfig` (`TemplateOp::into_config(model_id, tool_id)`), run
   the derivations (section 4.3), then:
   - the feeds (O-1): for a `Suggest` op, run Suggest on the candidate
     with the bound `ToolConfig` value and stamp `feeds_provenance`, as
     the planner does before its append (`build_tier_config`,
     `session/multitool.rs:473–496`). Use the one context assembly
     (`suggest_slots`, `session/mod.rs:2062`, F114); the editor makes it
     take the tool value, because an `Add` tool is not in the session
     before the window. A Suggest refusal keeps the saved feeds and gives
     a note (R-3). The feeds come BEFORE the domain check, so the F185
     zero-feed check reads the numbers that are written;
   - `floor_conflict` and `domain::validate_toolpath` (the `add_toolpath`
     checks, `session/mutation/toolpath.rs:31–34`);
   - `OperationType::tool_refusal` for the bound tool;
   - the selection: `resolve` + `view` through `build_selection_report`
     (`session/selection_status.rs:565`) on the candidate. An
     `UnknownLayer` refuses. Every other selection refusal is reported
     (see 3.2).
   A refusal here writes nothing. The refusal names EVERY failing op and
   cause in one message (as C2 names every retired key).
3. **Build**: ONE `try_with_effects` window. In it only raw halves run
   (WP3: the window is the one `Effects` construction site; no door that
   builds its own `Effects` runs inside it): `add_tool_impl` for each
   distinct `Add` tool, then `add_toolpath_impl` for each op that is on,
   in template order, with the resolved candidate configs.
   `Effects::applied` carries the outcome. Two slots that bind the same
   library tool add it once: the validate step deduplicates the `Add`
   tools by `io::tool_library::dedupe_key` (`io/tool_library.rs:346`; the editor makes it `pub(crate)`).

```rust
// session/command.rs (NEW field on Effects)
pub applied: Option<Box<TemplateOutcome>>,
pub struct TemplateOutcome {
    pub toolpath_ids: Vec<ToolpathId>,   // in cut order
    pub tools_added: Vec<ToolId>,
    pub notes: Vec<TemplateNote>,        // ops left off, tools added, feeds kept
}
```

`Effects::stale` holds the new indices; `simulation_cleared` is true when
a simulation existed (`add_toolpath_impl` drops it). The result is
ordinary ops: no field points back at the template, and a later template
edit changes nothing in the project (R9). `planner_origin` stays `None`.

### 3.2 Refused, noted, or left to generation

| Case | Draft (report) | Apply |
|---|---|---|
| A rule layer that the model does not hold (S21) | the op row names the layer, offers the map (3.3) and the op switch | REFUSE `MissingLayers { op, layers }` for every op that is on (O-4) |
| A slot with no tool bound | red slot row, Apply disabled | REFUSE `NoToolForSlot` |
| A bound tool that the op refuses (a flat mill on VCarve) | red op row with `tool_refusal` text | REFUSE `ToolRefused { op, text }` |
| A value outside its domain (W6b), a floor conflict (P3.2) | red op row | REFUSE (the `add_toolpath` checks) |
| A rule that selects nothing (no Ø 5 circle) | the op is OFF by default, note "nothing to cut: no circle Ø 5.0 ± 0.1" | an op that is off is not added; a NOTE in the outcome (L-11). An op the user turned on is added and refuses `Empty` at generation, as any op |
| Items that the op cannot cut (open paths under a Pocket) | the `SelectionNote` text of `selection_status` | added; the note stays the op's own note |
| A lost pick, a model id source | cannot happen: `TemplateSelection` holds neither | — |
| A library tool added | "Adds <name> from <catalog>" | a NOTE; the tool is in the project |
| Suggest refuses a tool (a micro tool) | the op row says "feeds: as saved (Suggest refused: <reason>)" | the saved feeds stay, with a NOTE (risk R-3) |
| An empty setup is not needed | — | apply appends after the ops the setup holds |

### 3.3 The layer map

- The draft lists each rule layer that the model does not hold. Beside it
  a chooser lists `ItemSet::layers()` (`selection/item.rs:531`) and
  "(turn the op off)".
- A map choice REWRITES the layer name in the draft's rules (the draft is
  a `Template` value). Apply receives rules with the drawing's layer
  names. Nothing of the map stays after apply.
- Layer names match exactly (`Filter::admits`, `selection/rule.rs:148–151`).
  No case folding: one name, one meaning. The chooser shows the case.
- MCP and CLI: `layer_map: { "CUT": "Outline" }` on the call. The surface
  rewrites the rules of the template value through the same core function
  (NEW `Template::map_layer(from, to)`), so the three surfaces cannot
  differ.

---

## 4. Job types (question 4)

### 4.1 One mechanism

A job type is a built-in `Template` with `origin: BuiltIn(JobType)`. The
wizard lists job types first, then the library templates. The apply,
the report and the save are the same code. A job type has no question
page of its own: its "questions" are the ordinary op fields in the draft
(the floor, the tool, the tab count), and each edit writes an ordinary op
field (R9).

### 4.2 The table

Floors use the D9 references (`compute/floor.rs:67`). "Through" =
`Floor::StockBottom { depth: t }`; `t` is O-2.

| Job type | Order (S14: inner before outer) | Op | Rule | Slot | Floor |
|---|---|---|---|---|---|
| **Sign** | 1 | VCarve | closed loops, nesting `Inside` (the letters and the art) | V-bit, `included_angle` 60–90°, prefer largest diameter | VCarve `max_depth` (op field) |
| | 2 | Profile Outside, tabs on | closed loops, nesting `Outermost` (the border) | end mill Ø 3–6.35, prefer largest | through (`t`) |
| **Sheet parts** | 1 | Drill | circles Ø 5 ± 0.1 (shelf pins), any layer | end mill Ø 4.9–5.1 | Drill `depth` (op field) |
| | 2 | Pocket | closed loops on layer `DADO` | end mill Ø 6–6.35 ("cutter") | `StockTop { depth }` (the user sets it; the REQUIREMENTS job table says "half thickness") |
| | 3 | Profile Inside | closed loops, nesting `Inside`, kinds `Loop`, `not_layers = ["DADO"]` (the cutouts) | "cutter" | through (`t`) |
| | 4 | Profile Outside, tabs on | closed loops, nesting `Outermost` | "cutter" | through (`t`) |
| **Inlay pocket** (workpiece project) | 1 | Pocket (floor) | closed loops; boundary `Selection` of the closed shapes of the own model, `Inside`, offset DERIVED (4.3) | end mill Ø 3–6.35 | `Top { depth: D }`, D DERIVED from the Inlay op |
| | 2 | Inlay (V walls) | closed loops | V-bit | `pocket_depth = D` |
| **Inlay plug** (plug project, R2) | 1 | InlayPlug | closed loops | V-bit (the same angle as the pocket) | `pocket_depth = D`, `glue_gap` (op default `INLAY_GLUE_GAP_DEFAULT_MM`) |
| **Flatten** | 1 | Face | `Selection::stock()` (the Face default) | end mill, prefer largest diameter | Face `depth` (op field) |

What exists and what is missing:

- Every op exists: VCarve, Profile (tabs, P5), Pocket, Drill (size fit,
  P6.5), Inlay, InlayPlug (`compute/catalog.rs:351–352`), Face (stock
  default, P4.2). No new op.
- MISSING: the nesting filter (F-B, P7.1). Without it, Sign and Sheet
  parts need layers in the drawing. With layers, the user maps them
  (3.3).
- MISSING: a layer exclude (`Filter.not_layers`, P7.1, L-19). A dado
  loop inside a part is also an inside loop. Without the exclude, the
  cutout Profile cuts every dado through (R-0).
- MISSING: a value derived from the bound tool (F-E, 4.3), for the inlay
  pocket only.
- MISSING: a numeric source for the through depth `t` (O-2). The repo
  has no cited spoilboard skim value.
- Not a job type in P7: the two-sided part (pins: the pin wizard owns it)
  and the STEP part with several floors (an expert flow: `EachFace`).
- Sheet parts with no `DADO` layer: the Pocket rule names a missing
  layer. The draft shows it and the user turns the op off or maps it
  (S21). That is the honest default: a template does not guess a dado.

### 4.3 The one formula: `Derivation`

```rust
// template/derive.rs (NEW). A closed enum: a new formula is a new variant
// with its own sentry. It runs in the draft (on every edit) and at apply.
pub enum Derivation {
    /// The floor Pocket of an inlay: floor depth = the Inlay op's
    /// pocket_depth; boundary offset = -pocket_depth * tan(included_angle / 2) of
    /// the V-bit bound to the Inlay op's slot (ops/inlay.rs:14–26).
    InlayFloor { floor_op: usize, inlay_op: usize },
}
```

- The formula comes from the module doc of `ops/inlay.rs` (the wall at
  depth `t` lies `t · tan α` inside the outline; there `α` is the HALF
  angle, so `tan α = tan(included_angle / 2)` of `ToolConfig::included_angle`)
  and the `InlayConfig`
  help text. The sentry `inlay_pocket_and_plug_fit_g_inlaysplit` already
  checks the floor recipe; P7.6 asserts that the derived numbers equal it.
- After apply the numbers are ordinary op fields. An edit of the Inlay
  depth in the op panel does NOT move the Pocket (R9: no live link). The
  draft says so on the inlay rows.

### 4.4 Inlay: two projects

Ruling R2 puts the plug in its own project (`compute/operation_configs.rs:1071–1072`).
The wizard therefore offers "Inlay pocket" and "Inlay plug" as two job
types. The plug draft shows "Use the pocket depth of the pocket project"
and has a `pocket_depth` field. DESIGN D12 ("by copy through a template")
holds: the copy is the user's number, not a reference. **[OP] O-3.**

---

## 5. The GUI flow, MCP and CLI (question 5)

### 5.1 Where it opens

- The setup `+` menu (`ui/toolpath_panel.rs:1133`) gets a first row "From
  a template or job type…".
- The empty state (`ui/toolpath_panel.rs:1146`) gets a second button
  "Start from a job type…" beside "Add toolpath".
- File ▸ "Save toolpaths as template…" (P7.7).
- MCP `set_ui_view(modal: "template_wizard")` (`mcp_server.rs:1852` list).

### 5.2 The draft state

```rust
// viz ui/template_wizard.rs (NEW), held in AppState::panels.template_wizard
pub struct TemplateWizardState {
    pub step: TemplateStep,              // Choose, Map, Review
    pub setup_id: SetupId,
    pub model_id: Option<ModelId>,
    pub draft: Option<Template>,         // a clone of the chosen template, edited here
    pub bindings: Vec<Option<SlotBinding>>,
    pub ops_on: Vec<bool>,
    pub library: Vec<LibraryTool>,       // read once on open, off the frame (F218 pattern)
}
```

Cancel drops the state. Apply sends ONE `AppEvent::TemplateWizardApply`
(NEW); the controller sends ONE `Command::ApplyTemplate`. Every number
and every red row comes from the core `template_report` of the draft (as
`pin_set_report_for`).

### 5.3 Sketches

Step 1, Choose:

```
┌ New toolpaths from a template ─────────────────────────── ✕ ┐
│ ① Choose   ② Map   ③ Review                                  │
│ JOB TYPES                                                     │
│  ◉ Sign            V-carve the art, cut out the border       │
│  ○ Sheet parts     Pins, dados, cutouts, outline with tabs   │
│  ○ Inlay pocket    Floor pocket + V walls (workpiece)        │
│  ○ Inlay plug      Mirrored plug (plug board project)        │
│  ○ Flatten         Face the whole stock                      │
│ MY TEMPLATES   (~/.config/rs_cam/templates)                   │
│  ○ Cabinet side 18 ply                                        │
│ Into setup [Setup 1 ▾]   Drawing [sign.dxf ▾]                 │
│                                          [Cancel]  [Next ▸]  │
└───────────────────────────────────────────────────────────────┘
```

Step 2, Map (tools and layers):

```
 TOOLS
 Profile cutter   end mill Ø 3.0–6.35   [6.35 mm 2-flute upcut ▾]    Role::Ok
 V-bit            V-bit 60–90°          + Add "90° V 12.7" from vbits Role::Caution
 Shelf pin drill  end mill Ø 4.9–5.1    No tool matches               Role::Danger
 LAYERS
 DADO (Pocket)    not in sign.dxf       [Outline ▾ | turn the op off] Role::Danger
```

Step 3, Review (cut order; each row expands to the op's own fields):

```
 ☑ 1 VCarve        12 loops (3 islands)   V-bit 90°    depth 5.0      feeds: Suggest
 ☑ 2 Profile Out   1 loop  4 tabs         Ø 6.35       floor: Stock bottom + [ ] mm  ⚠ set
 ☐ 3 Drill         nothing to cut: no circle Ø 5.0 ± 0.1                (off)
 Notes: 2 open paths on layer TEXT are not cut (VCarve).
                                   [◂ Back]  [Cancel]  [Apply 2 toolpaths]
```

Apply is disabled while a red row stays on an op that is on. The hover
of the disabled button names the first red row (UP pattern: a disabled
control names the core reason, `ui/pick_bar.rs`).

### 5.4 Undo

`UndoAction` has no toolpath add (`controller/events/pins.rs:311–315`). The
planner apply has no undo either. A beginner applies a job type, looks,
and wants it gone. **[LEAD] L-10** recommends a NEW
`UndoAction::TemplateApplied { toolpath_ids, tools_added }`: undo sends
`RemoveToolpath` for each id that still exists and `RemoveTool` for each
added tool that no op binds; redo sends the same `ApplyTemplate` payload
again (kept in the action).

### 5.5 MCP and CLI parity

| Surface | Change | Pinned by |
|---|---|---|
| MCP `list_templates` | job types and library templates: name, origin, description, slots, op names | NEW `mcp_lists_job_types_and_templates_p7_9` |
| MCP `template_report` | the core `TemplateReport` (Query row) for `{ template, setup, model, bindings?, layer_map?, ops_on? }` | NEW `mcp_and_gui_read_one_template_report_p7_9` |
| MCP `apply_template` | the `ApplyTemplate` row; a missing binding takes the report's first candidate; a library tool is added only when the call says `add_library_tools: true`. MCP sends the template as it is (no draft edit of an op field); an agent edits the ops after apply with `set_toolpath_param` and `set_toolpath_floor`, which are ordinary edits (R9) | NEW `mcp_and_gui_apply_one_template_p7_9` (same project, same ops, byte-equal configs) |
| MCP `save_template` | P7.7: the ops of a setup to a library file | NEW `save_then_apply_gives_the_same_ops_p7_7` (core) + MCP arm |
| MCP `set_ui_view` | `modal: "template_wizard"` | `mcp_wire_surface_pin` moves by name |
| CLI `rs_cam template list` | as MCP | NEW `rs_cam_cli` test |
| CLI `rs_cam template apply <project.toml> --template <name> --setup <name> --model <id> [--map A=B] [--bind slot=tool_id] [--add-library-tools] --out <project.toml>` | loads, applies the row, saves through `SaveProject` | NEW `a_cli_template_apply_equals_the_mcp_apply_p7_9` |
| CLI `rs_cam template save <project.toml> --setup <name> --name <file>` | P7.7 | the same CLI test file |
| Registry | one Command row, one Query row | `command_registry_completeness`, `production_writes_go_through_apply_wp15a`, `mcp_core_arm_describes_every_row`, `effects_are_stamped_wp19` |

The job file (`job.rs:208`) gets no template key (L-14). A job file holds
its own ops; a template applies to a project. Two op-definition formats
must not grow into three.

---

## 6. Save as template (question 6)

**Recommendation: in scope.** S20 says "Save a set of ops with rule-based
picks". A user makes a template most often from a job that cut well. The
reverse direction uses the same types, so the cost is one function, one
surface entry and one sentry.

```rust
// template/save.rs (NEW)
pub fn template_from_ops(session: &ProjectSession, setup_index: usize,
                         ops: &[usize], name: String) -> Result<Template, Vec<SaveRefusal>>;
```

| Field of the op | Save |
|---|---|
| A rule on the own model or the stock | kept |
| A rule on another model (`Model(id)`) | REFUSE, names the op and the model |
| A pick or an exclude | REFUSE, names the op and the count; the fix: "replace the picks by a layer rule (shift-click, P4.5)" |
| `tab_points` (hand tabs) | REFUSE (model-frame points); the fix: "use the automatic tabs (tab_count)" |
| `DerivedRestRegions`, `PlannedTierRegions`, a rest `reference_tool_id`, a Project Curve `surface_model_id` | REFUSE (F-D) |
| The tool | a NEW slot per distinct tool: `ToolMatch::from_tool(&ToolConfig)`: the type; the diameter ± `CAD_ROUNDING_MM` (`selection/key.rs:38`); the flute count; the angle for a V-bit; the corner radius for a bull nose; `prefer = Nearest` |
| The feeds | `feeds = AsSaved` when the user ticks "keep my feeds", else `Suggest` (O-1) |
| `pre_gcode`, `post_gcode`, `debug_options`, `feeds_provenance`, `planner_origin` | not saved (L-4) |

Every refusal is listed at once. Nothing is stripped without a word.

---

## 7. Items (question 7)

Gates per PLAN (named sentries, folder sentries, viz `--lib`,
`rs_cam_mcp`, clippy, fmt, through `scripts/cargo_lane.sh`; ask before
3 minutes). A red proof puts the defect back, sees red, restores.

**P7.1 — The nesting filter** (no output change). Files:
`selection/rule.rs` (NEW `Filter.nesting: Option<Nesting>`, `Nesting::{Outermost, Inside}`;
NEW `Filter.not_layers: Vec<String>` (L-19); both `skip_serializing_if`); `selection/resolve.rs` (one even-odd depth per
closed drawing ring of a source slot, computed once per resolve; the
admit test reads it); `selection/view.rs` (no change: it nests the
selected rings); MCP wire (`rs_cam_mcp` filter type, exhaustive both
ways, F166); viz `+ Rule` card (two kind chips). Sentries: NEW lib
`selection::nesting_*` (a square in a square in a square: depths 0, 1,
2; `Outermost` gives the outer one; `Inside` gives the other two; a path
has no depth and fails a nesting filter; an item on a `not_layers` layer fails the rule; a
`not_layers` layer that the model does not hold gives no `UnknownLayer`); NEW
`a_nesting_rule_selects_outer_or_inner_loops_p7_1` (session path: a
Profile Inside on `Inside` loops cuts the cutout of a part; a Profile
Outside on `Outermost` cuts the outline only; a dado loop on layer `DADO`
is not in an `Inside, not_layers = ["DADO"]` rule); `selection_round_trips_p2_6`
gains the key; `default_selection_equals_the_old_lists_p2_2` does not
move. Red proof: compute the depth over the selected rings only; the
cutout arm fails. Operator looks at: `+ Rule` with "outer loops" and
"inner loops" on a part with a cutout.

**P7.2 — The core template model** (no output change). Files: NEW
`template/mod.rs`, `template/selection.rs`, `template/tool_match.rs`,
`template/derive.rs`, `template/file.rs` (serde, `format_version`),
`template/CLAUDE.md` (40 lines or fewer); `session/project_file.rs`
(NEW `check_retired_toolpath_keys`; the project loader calls it per
toolpath; `RETIRED_KEYS` rows keep their text); `lib.rs`; the core
`CLAUDE.md` folder table. Sentries: NEW lib `template::*` (round trip of
a template file; a pick, an exclude, a `Model(id)` source, each id field
of F-D and `tab_points` refuse by name; `match_slots` order rules,
three ties); NEW `a_template_of_another_version_is_refused_p7_2`; NEW
`a_retired_key_in_a_template_is_named_p7_2` (the `depth` key on a Pocket
template op names the P3 replacement); `every_project_table_refuses_an_unknown_key_f034`
and `a_retired_floor_key_is_named_p3_2` stay green (the shared check).
Red proof: let `TemplateSelection::new` admit a pick; the lib arm fails.
Operator looks at: nothing.

**P7.3 — The template library folder** (no output change). Files:
`settings/paths.rs` (NEW `TEMPLATE_DIR_ENV`, `TEMPLATES_DIR`,
`template_library_dir_from`, `template_library_dir`); `settings/mod.rs`
(`PathSettings::template_library`, the `[paths]` key, the writer);
NEW `io/template_library.rs` on `named_toml_library`; viz
`ui/preferences.rs` (one folder row). Sentries: `settings::` lib tests
(round trip of the new key); NEW lib `io::template_library` (list, load,
save, a bad name, a parse refusal names the file). Red proof: drop the
env arm; the resolver test fails. Operator looks at: Preferences.

**P7.4 — The `template_report` query** (no output change). Files: NEW
`session/template.rs` (`template_report(&TemplateReportArgs) -> TemplateReport`:
per slot candidates; per op: the candidate `ToolpathConfig`, the
`build_selection_report` answer, the missing layers, the tool refusal,
the domain refusal, the derived numbers; no memo); `session/command.rs`
(Query row). Sentries: NEW `the_template_report_is_the_selection_status_p7_4`
(apply a template, then the counts and notes of each new op's
`selection_status` equal the report row of that op, field by field);
NEW lib arms for each red row of section 3.2. Red proof: build the
report with the stock frame in place of the setup frame; the counts
differ on a Bottom setup arm. Operator looks at: nothing.

**P7.5 — The `ApplyTemplate` command** (new ops only). Files:
`session/template.rs` (`apply_template`: validate, resolve, build;
`TemplateOutcome`, `TemplateRefusal`); `session/command.rs` (row,
`ApplyTemplateArgs`, `SlotBinding`, `Effects::applied`); `session/mod.rs`
(re-exports); `SessionError::Template` (exhaustive `field`, F215).
Sentries: NEW `a_template_applies_as_ordinary_ops_p7_5` (the ops equal
hand-built ops field by field; no field names the template; an edit of
the template value after apply changes no op); NEW
`a_missing_layer_is_named_on_apply_p7_5` (S21: the refusal names every
op and layer; the project is unchanged, by `ProjectFile` equality); NEW
`a_template_apply_is_all_or_nothing_p7_5` (a tool refusal on op 3 of 4
adds no tool and no op); NEW `an_empty_rule_leaves_its_op_off_p7_5`;
`command_registry_completeness`, `stale_set_has_one_answer_wp28`,
`mutation_paths_invalidate_alike_p0` gain the row. Red proof: add the
ops one by one with `add_toolpath` (one window each) and refuse on op 3;
the all-or-nothing sentry sees two ops. Operator looks at: nothing (P7.8).

**P7.6 — Built-in job types** (new ops only). Files: NEW
`template/builtin.rs` (five constructors, `JobType` enum with
`ALL`); `template/derive.rs` (`InlayFloor`); NEW fixtures under
`crates/rs_cam_core/tests/fixtures/templates/` (a sign DXF with a border
and letters, a part DXF with a cutout and Ø 5 circles, an inlay
heart). Sentries: NEW `every_job_type_applies_on_its_fixture_p7_6`
(each job type applies, every op generates, no refusal; the Flatten op
cuts the whole stock box to its depth on the simulated stock; the Sheet
parts outline is cut after the cutout: plan order; a dado loop is
pocketed and not profiled: no fed move of the Profile Inside lies on the
dado ring); NEW
`the_inlay_job_type_derives_the_floor_recipe_p7_6` (the derived Pocket
depth and boundary offset equal the G-INLAYSPLIT recipe at 60° and 90°);
`inlay_pocket_and_plug_fit_g_inlaysplit` green. Red proof: use
`tan(included_angle)` in place of `tan(included_angle / 2)`; the inlay arm fails at 60°.
Operator looks at: each job type on its fixture, generated and
simulated (after P7.8).

**P7.7 — Save as template.** Files: NEW `template/save.rs`;
`io/template_library.rs` (save); `session/template.rs` (`SaveTemplate`
is a surface-side write, as a tool catalogue save; no Command row: it
does not mutate the session). Sentries: NEW
`save_then_apply_gives_the_same_ops_p7_7` (save the ops of a setup,
apply into a fresh project with the same drawing: the op configs are
equal except ids); NEW lib arms for each refusal row of section 6. Red
proof: drop the pick check; a template with a pick saves and the
sentry fails. Operator looks at: nothing (P7.8).

**P7.8 — GUI.** Files: NEW `ui/template_wizard.rs`; `state/panels.rs`
(`template_wizard`); `ui/mod.rs` (`AppEvent::TemplateWizardApply`,
open/close); `controller/events/` (NEW `templates.rs`: one
`ApplyTemplate`, the undo action); `state/history.rs`
(`UndoAction::TemplateApplied`, L-10); `ui/toolpath_panel.rs` (the two
entry points); `ui/menu_bar.rs` (Save toolpaths as template…); `app.rs`
(draw). Sentries: NEW `template_wizard_apply_cancel_p7_8` (Cancel leaves
the project unchanged; Apply raises ONE event; the egui harness, as
`pin_wizard_apply_cancel_r4`); NEW `the_template_wizard_reads_the_core_report_p7_8`
(every count and red row is the report text); NEW
`a_template_apply_undoes_in_one_step_p7_8`; keep green
`the_inspector_text_is_bounded_ic1`, `panels_read_the_token_module_up1`,
`ui_string_hygiene`, `egui_draw_sites_write_through_commands_wp6`,
`file_dialogs_run_off_the_frame_f218`. Red proof: send one
`AddToolpath` per op from the controller; the one-event arm fails.
Operator looks at: every sketch of section 5.3 (rebuild the release GUI
from HEAD first).

**P7.9 — Parity and docs.** Section 5.5 rows, each pinned by a named
test; `rs_cam_mcp` wire types (exhaustive conversions, inline enums);
CLI `template` subcommand; `FEATURE_CATALOG.md`; `template/CLAUDE.md`,
`session/CLAUDE.md` (stays at 40 lines or fewer: P4.10 follow-up says it
is over; shorten it in the same edit), `settings/CLAUDE.md`,
`io/CLAUDE.md`; the `PLAN.md` status row.

Order: P7.1 (independent; can run first) → P7.2 → P7.3 → P7.4 → P7.5 →
P7.6 → P7.7 → P7.8 → P7.9. P7.3 and P7.1 touch no shared file with each
other.

---

## 8. Decisions (question 8)

### For the operator

| # | Question | Recommendation |
|---|---|---|
| O-1 | The feeds of a template op at apply. (A) Suggest at apply for the bound tool and the project material (as the planner, `session/multitool.rs:496`), provenance stamped, shown in the draft; a template saved from a tuned job can say `feeds = "as_saved"` per op. (B) Always the saved numbers. | (A). Saved numbers belong to another tool or material. The draft shows each feed before Apply, and a user who tuned a job ticks "keep my feeds" at save. No field locks (the standing Feeds ruling). |
| O-2 | The through depth `t` below the stock bottom for Sign and Sheet parts (`Floor::StockBottom { depth: t }`). The repo holds no cited spoilboard skim value. (A) No default: the draft field is empty and red until the user types it. (B) A default number from a source the operator names. | (A). A number typed from memory breaks the constants rule. The draft asks once per apply; a saved user template keeps the user's value. |
| O-3 | Inlay. (A) Two job types: "Inlay pocket" (floor Pocket with the derived recipe, then Inlay) in the workpiece project; "Inlay plug" in the plug project, with `pocket_depth` typed from the pocket project. (B) One project with a second setup for the plug board. | (A). Ruling R2 says the plug runs in its own project. (B) reopens R2. |
| O-4 | A rule layer that the drawing does not hold (S21). (A) Name it in the draft; refuse Apply until the user maps it or turns the op off. (B) Apply the op anyway; it refuses `UnknownLayer` at generation. | (A). S21 asks the template to name the layer; (A) names it before any op exists, and the P2.2 ruling already refuses at generation. |

### For the lead

| # | Question | Recommendation |
|---|---|---|
| L-1 | The apply pattern | Draft from the pin wizard (own state, Cancel = nothing, one GUI event); core apply from `PlanMultitoolFinishing` (one row, validate/resolve with `&self`, one window). Not the pin-wizard controller sequence (F-A) |
| L-2 | What the command takes | The edited `Template` value, setup, model, one binding per slot, one switch per op. Matching is a pure core function that the report runs; MCP and CLI fill a missing binding from the report |
| L-3 | Rules only | `TemplateSelection` newtype with a refusing constructor; `TemplateOp` private fields (compiler-enforced, not a doc rule) |
| L-4 | Which `ToolpathConfig` fields a template holds | All but `id`, `tool_id`, `model_id`, `pre_gcode`, `post_gcode`, `debug_options`, `feeds_provenance`, `planner_origin`; a destructure with no `..` in `into_config` and `from_config` |
| L-5 | Which ops a template may hold | Every op except `SystemOnly` (`AlignmentPinDrill`: the pin door owns it). A 3D op is allowed when it carries no id field (F-D); job types use 2.5D ops only |
| L-6 | Storage | One file per template, `<config>/rs_cam/templates`, `RS_CAM_TEMPLATE_DIR`, `[paths] template_library` (the tool-folder pattern) |
| L-7 | Version and retired keys | `format_version = 1`, refuse others; one shared retired-key check per toolpath table (`check_retired_toolpath_keys`) |
| L-8 | Built-ins | Rust constructors returning `Template` (a config change does not compile, or the P7.6 sentry fails); "Save a copy" writes TOML |
| L-9 | `Effects` | NEW `applied: Option<Box<TemplateOutcome>>`, as `planned` (F040) |
| L-10 | Undo | NEW `UndoAction::TemplateApplied` (remove the applied ops and the unbound added tools; redo re-sends the payload). The planner has no undo; a beginner flow needs one |
| L-11 | A rule that selects nothing | The op is off in the draft with a note; MCP and CLI leave it off unless `ops_on` turns it on; then it is added and refuses `Empty` at generation as any op |
| L-12 | Tool tie-break | `prefer`, then `tool_number`, then index; a project tool before a library tool; MCP adds a library tool only with `add_library_tools: true` |
| L-13 | Nesting filter meaning | Even-odd depth over ALL closed drawing rings of the source (not only the selected ones); `Outermost` = depth 0, `Inside` = depth ≥ 1; a path, a point and a face fail a nesting filter |
| L-19 | Cutouts and dados are both inside loops (section 4.2 note) | `Filter.not_layers` in P7.1: an item on a listed layer fails the rule. A listed layer that the drawing does not hold excludes nothing and is NOT an `UnknownLayer` refusal. `Template::map_layer` rewrites `layers` and `not_layers` alike |
| L-14 | CLI | A `template` subcommand (list, apply, save); no `template` key in the job file |
| L-15 | Target | One existing setup; the ops append after the ops it holds; a template holds no stock, setup or machine |
| L-16 | Entry points | The setup `+` menu, the empty-state second button, File ▸ Save toolpaths as template…; no prompt after an import (no wizard as the main surface, ruling 2026-10-16) |
| L-17 | `Derivation` | A closed enum with one variant (`InlayFloor`); re-run on each draft edit and at apply; no link after apply |
| L-18 | F-C (`Island` on a Profile hole ring) | Check in P7.1 with the nesting sentry; if the report says "cut round it" where Profile does not cut, give Profile its own role or note |

---

## 9. Risks

- **R-0 (machined, first) A rule that picks the wrong loops.** Two
  cases are known. (a) Sheet parts: a dado loop is also an inside loop,
  so the cutout Profile must exclude the `DADO` layer (L-19), or it cuts
  the dado through. (b) Sign: a drawing with no border loop makes every
  letter `Outermost`, so VCarve selects nothing and Profile Outside cuts
  round each letter through the board. The draft shows it ("VCarve:
  nothing to cut (off)"; "Profile Outside: 12 loops"); the P7.6 fixture
  set adds a sign with no border, and the sentry asserts that the draft
  report gives those two rows. A count in the draft is the only guard a
  beginner reads, so the Review step shows the counts first in each row.

- **R-1 A template bypasses a door.** Apply uses `add_toolpath_impl`, the
  raw half. The resolve step must run every check that `add_toolpath`
  runs (`floor_conflict`, `domain::validate_toolpath`) and the tool
  refusal. The all-or-nothing sentry (P7.5) and a W6b arm hold it.
- **R-2 User template files rot.** An op key retires and an old template
  stops loading. The shared retired-key check names the key and its
  replacement (C1/C2), so the user can edit the file. Built-ins are code
  and cannot rot.
- **R-3 Suggest refuses.** The feeds matrix refuses some tools (memory:
  micro tools until the size law). A Suggest refusal must not block the
  apply: the op keeps the template feeds with a note (section 3.2). A
  sentry arm uses a refused tool.
- **R-4 The nesting cost.** An even-odd depth is a point-in-ring test per
  ring pair: O(n²) per source. A large DXF (thousands of rings) can make
  the report slow on the frame. Compute the depths once per resolve, and
  reuse the containment code of `polygon::detect_containment_tagged`
  (`polygon.rs:1747`).
- **R-5 Layer names.** DXF layers are often upper case; a template made
  on one drawing can miss on the next. The map (3.3) is the answer; no
  case folding.
- **R-6 The inlay recipe drifts.** If the Inlay op changes its wall rule,
  the derivation must change too. The P7.6 sentry compares the derived
  numbers with the G-INLAYSPLIT recipe, so a drift fails a test.
- **R-7 Undo removes a tool the user bound since.** The undo removes an
  added tool only when no op binds it at undo time.
- **R-8 Shared checkout.** P7.2 edits `session/project_file.rs`, and P7.5
  edits `session/command.rs`: both are hot files. One editor at a time;
  diff for foreign hunks before the lead commits.
