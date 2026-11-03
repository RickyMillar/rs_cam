# D — The toolpath IR, entries, findings and gates (research, 2026-11-03)

Read-only research for the feeds contract. Paths are under `crates/rs_cam_core/src/` unless they
name another crate. Line numbers are at HEAD `40cf411a7`. The files that another session edits
(`ops/project_curve.rs`, `mesh.rs`, `surface/dropcutter.rs`, `finish/scallop/ring_generation.rs`)
were read with `git show HEAD:<path>`. Sections 1 to 4 hold facts. Section 5 holds the suggestions.

Background read: `planning/feeds_blind_review_2026-11-03/rootcause/INDEX.md`, reports 02, 03, 05.

---

## 1. The IR (`toolpath.rs`)

### 1.1 Types

| Item | Where | What it holds |
|---|---|---|
| `MoveType` | `toolpath.rs:30-39` | Kinematic class: `Rapid`, `Linear { feed_rate }`, `ArcCW`, `ArcCCW`. |
| `MoveIntent` | `toolpath.rs:56-89` | Per-move class, set by the producer. 12 variants (below). `#[default] Unknown` (`:87-88`). |
| `Move` | `toolpath.rs:126-139` | `target`, `move_type`, `intent`, `feed_cap: Option<f64>`. All fields are `pub`. |
| `Toolpath` | `toolpath.rs:280-282` | `moves: Vec<Move>`. |
| `RingMaterial`, `ClosedPassSide` | `toolpath.rs:168-195` | The material side of a closed pass (P6.0c). |

`MoveIntent` variants (`toolpath.rs:56-89`):

| Variant | Doc meaning | Writer count (rg, all files incl. tests) |
|---|---|---|
| `Drilling` | Drill-cycle plunge | 20 hits / 10 files |
| `EntryPlunge` | "Milling op entering material with a near-pure plunge" | 83 / 24 |
| `ClearingCut` | Roughing feed | 45 / 15 |
| `FinishingCut` | Finishing feed | 93 / 33 |
| `EntryHelix` | Helical entry | 21 / 11 |
| `EntryRamp` | Ramped entry | 27 / 10 |
| `Linking` | "Transition at feed (not material removal)" | 98 / 32 |
| `Retract` | Lift before a rapid | 76 / 27 |
| `LeadIn`, `LeadOut` | F-040 lead arcs | 9 / 6, 8 / 6 |
| `ObstacleClearance` | K3 first rapid to the obstacle plane | 13 / 8 |
| `Unknown` | Deprecation marker, the default | 32 / 12 |

No variant exists for a step-down (a descent to the next level), for an air approach at feed, or
for "this move is the whole entry". The IR has no per-entry record: an entry is a run of moves with
entry intents.

### 1.2 How each motion is tagged

| Motion | Tag | Where |
|---|---|---|
| Untagged emit | `Unknown` | `rapid_to`, `feed_to`, `arc_*_to` (`toolpath.rs:289-303`) |
| Generic open path: approach rapid | `Linking` (a rapid, though the doc says "at feed") | `toolpath.rs:428` |
| Generic open path: vertical descent | `EntryPlunge` at the op `plunge_rate` | `toolpath.rs:429` |
| Generic closed contour: descent | `EntryPlunge` | `toolpath.rs:484` |
| Raster from grid: descent | `EntryPlunge` | `toolpath.rs:890, 909, 1029` |
| Dressup ramp | lead feed `EntryPlunge`, ramp `EntryRamp` | `dressup/entry_descent.rs:468-476, 596, 675, 712, 720` |
| Dressup helix | lead feed `EntryPlunge`, helix `EntryHelix` | `entry_descent.rs:800-811, 954, 964` |
| Dressup stock-aware plunge | `EntryPlunge` | `entry_descent.rs:497-498` |
| Ramp invalid-angle fallback | `EntryPlunge` | `entry_descent.rs:533` |
| 3D Rough peck ladder | `EntryPlunge` per peck, `Retract` between | `adaptive3d/path.rs:213-236` |
| 3D Rough stay-down link (descends too) | `Linking` | `adaptive3d/path.rs:1726-1746` |
| Pencil own entry | air lead `EntryPlunge`, crease ramp `EntryRamp` | `finish/pencil/emission.rs:353-371` |
| Unified Finish / surface link landing | `EntryPlunge` (or `Linking` when it rides) | `finish/surface_link.rs:294, 1255, 1263` |
| Boundary clip re-entry | `EntryPlunge` at the op plunge rate | `geometry/boundary.rs:494, 532, 536` |
| Tab descent after the bridge | `EntryPlunge` (by design, `tabs.rs:60`) | `dressup/tabs.rs:217, 256` |
| Bore helix | `ClearingCut` / `FinishingCut`, never `EntryHelix` (by design) | `ops/bore.rs:26-27, 244-284` |
| Drill | `Drilling` | `ops/drill.rs:128, 251, 284` |
| Obstacle step first rapid | `ObstacleClearance` | `compute/obstacle.rs` (K3) |

Who sets intents: the generator, the dressups, and three session stages (boundary clip, obstacle
step, entry-descent split). `Move::intent` is a public field, so any stage can rewrite it.

### 1.3 Readers of the entry intents

- Simulation triage, entry samples: `EntryPlunge | EntryRamp | EntryHelix` (`stock/sim_triage.rs:1358-1368`).
- Load gates skip configured-entry samples (`tool_load/locality.rs:156-160`, `is_steady_state_for_gate`).
- Feed modulator skips `Drilling | EntryPlunge | … | LeadIn | LeadOut` (`dressup/feed_modulation.rs:445-453`).
- Spans: `EntryPlunge/EntryHelix/EntryRamp/LeadIn` → `SpanKind::Entry` (`compute/spans.rs:319-336`).
- Entry-descent split: only a rapid followed by an `EntryPlunge` is split (`dressup/mod.rs:861`).
- G-CENTRECUT pipeline fallback: "planner made entries" = any `EntryRamp | EntryHelix` (`compute/execute/dressup_apply.rs:664-670`).
- Narration, entry audit, segment merge: `trace/narrate.rs:235-246`, `dressup/entry_audit.rs:39-42`, `dressup/condition.rs:131`.
- CLI `rough-score`: labels an entry run by the intent of its FIRST move (`crates/rs_cam_cli/src/rough_score.rs:453-461`).

### 1.4 Facts about the tag model that matter for a contract

1. **The entry dressup does not read intents.** `apply_entry` replaces every fed descent that
   `is_plunge` finds by geometry: dz < -0.1 mm and XY < epsilon (`dressup/mod.rs:428-431, 1107-1118`).
   A `Linking` or `ClearingCut` vertical descent is also replaced.
2. **Air lead mis-tag.** `rapid_to_entry_top` feeds straight down through air (or through the gap
   between the conservative and the cell-centre top) with `EntryPlunge` (`entry_descent.rs:468-476`).
   The ramp or helix then follows. Pencil does the same (`finish/pencil/emission.rs:353-357`). A
   reader that takes the first move of the run reads "plunge" (report 05, `rough_score.rs:453-461`).
3. **Step-down is not a kind.** A Layered op emits each ring of each level as a new
   `Linking`-rapid → `EntryPlunge` → body → `Retract` (`toolpath.rs:428-436, 482-489`). A 3D Rough
   level change is a peck ladder (`EntryPlunge`) or a stay-down link (`Linking`). A reader cannot
   tell a first entry from a level step.
4. **A cutting link is tagged `Linking`.** The doc says `Linking` removes no material
   (`toolpath.rs:69-70`), but a 3D Rough stay-down descent can cut; G-LINKPLOUGH grades it by
   geometry (`stock/sim_triage.rs:1340-1350`, `link_cut_finding` `:1723`).
5. **`Unknown` is still the default** of the untagged emitters and of `Default` (`toolpath.rs:87-88,
   289-303`). The doc claims "All in-tree generators emit non-`Unknown` tags" (`:49-51`).

---

## 2. Entry handling

### 2.1 Policy types

| Type | Where | Fact |
|---|---|---|
| `EntryStylePolicy` | `compute/catalog/schema.rs:603-614` | `AnyEntry`, `ForceNone`, `DefaultHelix`. |
| `DressupPolicy` | `schema.rs:621-664` | `strip_all_reason: Option<&str>` + `entry`. Consts `ANY_DRESSUP`, `FORCE_NO_ENTRY`, `DEFAULT_HELIX`, `strip_all(reason)` (`:647-652`, `entry: AnyEntry`). `entry_style_applies()` (`:661-663`). |
| `DressupEntryStyle` | `compute/config.rs:926-930` | `None`, `Ramp`, `Helix` (the operator's choice). |
| `DressupConfig` | `config.rs:1180-1230` | `entry_style`, `ramp_angle`, `helix_radius`, `helix_pitch`, `entry_clearance_mm`, `lead_in_out`, `link_moves`, … Default `entry_style: None` (`:1528-1551`). |
| `EntryStyle` (core) | `dressup/mod.rs:94-107` | `Ramp`, `Helix`, `Plunge` (stock-aware plunge). |
| `EntryStyle3d` / `Adaptive3dEntryStyle` | `adaptive3d/mod.rs:114-122`; `compute/operation_configs.rs:1646` | 3D Rough's own field; default `Plunge` (`operation_configs.rs:1801`). |
| `EntryProbeRule` / `EntryProbe` | `compute/catalog/capabilities.rs:576-611`; resolved `compute/catalog.rs:1122-1135` | `Prism`, `SurfaceFixed { leave_mm }`, `SurfaceFromConfig`. It only sets the floor the ramp legs may not go under. It does not choose a style. |
| `stock_aware_plunge`, `helix_floor_lap`, `ramp_fold_lap_cap` | `compute/catalog.rs:161-191`; set on `TRANSFORM_ADAPTIVE_2D` only (`capabilities.rs:781-782`) | The stock-aware plunge step runs only for 2D Adaptive under entry None. |
| `ramp_descent` | `capabilities.rs:100-103` | Narration flag only (Ramp Finish true). It is not an entry rule. |

### 2.2 Where the policy acts

- `DressupConfig::for_role` (`config.rs:1599-1645`): Roughing → `Ramp`; SemiFinish and Finish → `None` (R10).
- `DressupConfig::for_op` (`config.rs:1649-1674`): `DefaultHelix` → `Helix`, then `normalize_for_op`.
- `normalize_for_op` (`config.rs:1682-1720`): `strip_all` sets entry None, leads None, links None; `ForceNone` sets entry None.
  Callers: `session/project_file.rs:1347`, `session/mutation/config.rs:33, 150`, `session/mutation/toolpath.rs:729`.
- **The pipeline does not read `DressupPolicy`.** `rg strip_all|dressup_policy` outside the registry,
  schema and config finds only `compute/execute/volumetric.rs:222`. `PipelineRun::effective_entry_style`
  (`compute/execute/dressup_apply.rs:647-683`) reads `cfg.entry_style` and two overrides:
  - a drill cycle (any `Drilling` move) → None (G-WANAKA-DRILL-RAMP, `:651-659`);
  - a non-centre-cutting tool with entry None and no planner entries → `Ramp` (`:664-680`). This
    override also fires on a `strip_all` op, because the pipeline does not know the op stripped.
- The three entry steps: `ramp_entry` (`:754-787`), `helix_entry` (`:790-833`), `plunge_entry`
  (`:839-870`, needs `stock_aware_plunge`). At most one runs.

### 2.3 G-CENTRECUT and G-POCKETRAMP

- Tool data: `ToolConfig::non_centre_cutting: Option<NonCentreCuttingGeometry>`; doc "`None` means
  centre cutting" (`compute/tool_config.rs:478-480`). There is no "not known" state.
- The one check: `NonCentreCuttingLimits::clamp_descent_angle_deg`, `allows_plunge`,
  `is_steeper_than_limit`, `plunge_refusal` (`tool_config.rs:360-400`).
- Emitters clamp the ramp angle and ignore `contact_top` for such a tool (`entry_descent.rs:446-450, 523-526`).
  3D Rough maps `Plunge` → `Ramp` at the limit (`adaptive3d/path.rs:2062-2072`).
- Three stages: generation refuses (`session/compute.rs:735-742`, `OperationError::NonCentreCutting`);
  the simulation records descents (`stock/collision.rs:772`, `compute/simulate.rs:828, 845`); export
  refuses (`session/compute/export.rs:104-122`). No diagnostic id exists for it (`rg -i centre diagnostics/ids.rs`
  finds only a valley-centreline row).
- G-POCKETRAMP: `rg POCKETRAMP` finds no code or sentry name at HEAD. The ramp fold for a closed
  pocket ring is G-RAMPCONTAIN (`dressup/mod.rs:474-489`, `RampFold`).

### 2.4 Per-op entry table

"Role" is `UiProcessRole` from the registry. "Default" is `DressupConfig::for_op`. Registry lines
are in `compute/catalog/registry.rs`.

| Op | Registry policy (line) | Allowed dressup entry | Default entry | Own entry (planner) | Entry when None / stripped |
|---|---|---|---|---|---|
| Face | ANY (78) | None/Ramp/Helix | Ramp (Roughing) | — | straight `EntryPlunge` per ring |
| Pocket | ANY (127) | None/Ramp/Helix | Ramp | — | straight plunge per ring per level |
| Profile | ANY (178) | None/Ramp/Helix | Ramp | — | straight plunge per level |
| Zigzag | ANY (462) | None/Ramp/Helix | Ramp | — | straight plunge |
| Adaptive (2D) | DEFAULT_HELIX (232) | None/Ramp/Helix | Helix | helix floor lap; contained helix shrinks or becomes ramp (`dressup/mod.rs:452-468`) | stock-aware plunge: rapid through air, then feed (`dressup_apply.rs:839`) |
| VCarve | strip_all (292) | none | None | — | straight plunge at zero depth on the outline |
| Inlay, Inlay Plug | strip_all (351, 410) | none | None | — | straight plunge |
| Trace | FORCE_NO_ENTRY (514) | none | None | — | straight plunge |
| Drill, Pin Drill | FORCE_NO_ENTRY (567, 1416) | none | None | `Drilling` cycle | the descent is the cut |
| Bore | strip_all (642) | none | None | own helix tagged `ClearingCut` | n/a |
| Chamfer | ANY (706) | None/Ramp/Helix | None (Finish) | — | straight plunge |
| Drop Cutter | strip_all (759) | none | None | — | straight plunge per raster segment |
| 3D Rough | FORCE_NO_ENTRY (816) | none (own field) | own field `Plunge` (`operation_configs.rs:1801`) | peck ladder / ramp / helix (`adaptive3d/path.rs:200-236`) | peck ladder at `plunge_rate` |
| Waterline | ANY (865) | None/Ramp/Helix | None (SemiFinish) | — | straight plunge per contour |
| Pencil | ANY (919) | None/Ramp/Helix | None | crease ramp on rest stock (`pencil/emission.rs:345-372`) | straight plunge |
| Scallop, Steep/Shallow, Ramp/Spiral/Radial/Horizontal Finish | ANY (971, 1090, 1142, 1198, 1247, 1296) | None/Ramp/Helix | None | — | straight plunge |
| Unified Finish | strip_all (1036) | none | None | surface-link landing `EntryPlunge` (`surface_link.rs:294, 1255`) | straight plunge |
| Project Curve | strip_all (1354) | none | None | — | straight `EntryPlunge` to full chain depth (`ops/project_curve.rs:379-397` at HEAD → `toolpath.rs:429`) |

Plunge feed rule, by path:

| Path | Feed | Where |
|---|---|---|
| Generic emitters (all straight plunges in the table) | op `plunge_rate` (required: `cut_feeds` refuses `NotSet`) | `compute/catalog.rs:1647-1667`; `toolpath.rs:429, 484` |
| Dressup ramp / helix / stock-aware plunge, air lead and fallback | `min(plunge move feed, entry_plunge_rate)`; `entry_plunge_rate` = 0.5 x the first linear feed of the toolpath, else 500 | `dressup/mod.rs:478, 496, 509`; `dressup_apply.rs:711-722` |
| Dressup ramp / helix moves in material | `entry_feed_rate()` = `min(ramp_feed_rate, feed_rate)`, else the lead feed | `compute/catalog.rs:1688-1692`; `entry_descent.rs:534` |
| 3D Rough pecks / ramp | `plunge_rate` / `ramp_feed_rate` | `compute/execute/rough_3d.rs:68, 181-183` |
| Boundary clip re-entry | op `plunge_rate` | `session/compute.rs` clip call `Some(self.context.plunge_rate)`; `geometry/boundary.rs:532` |
| Suggest's written plunge | side feed / flutes (G10 claim) | `feeds/plunge.rs:308`; `feeds/suggest/apply.rs:265` (report 02 (c)) |

Note: the entry dressup's `entry_plunge_rate` is not the op's plunge rate. On a toolpath whose first
linear move is the generated `EntryPlunge`, it is 0.5 x the op plunge rate.

---

## 3. Findings and gates

### 3.1 Every finding / verdict family

| Family | Type and severity scale | Where made | Who reads it |
|---|---|---|---|
| Unified diagnostic | `Diagnostic` (`diagnostics/mod.rs:229-260`), `Severity { Info, Hint, Caution, Critical, Blocking }` (`:133-139`), plus `Category`, `Confidence`, `DiagnosticState`, `Source` | 13 files build `Diagnostic { … }`: adapters `from_generation` 19, `from_feeds` 17, `from_static_checks` 14, `from_tool_load` 13, `from_preconditions` 10, `from_stale_default` 2, `from_project_diagnostics` 2, `from_model_refs` 1; `stock/sim_triage.rs` 13; `adaptive3d/search.rs` 2; `session/compute/diagnostics.rs` 1; `session/diagnostics_types.rs` 1 | GUI panels, MCP `get_toolpath_diagnostics` / `get_project_diagnostics` (serde of the struct, `rs_cam_viz/src/app/mcp/diagnostics.rs:681-712`), `run_state::refuses_generation` |
| Ids | `diag_ids!` → 107 const strings + `ALL` (`diagnostics/ids.rs:16-30`) | — | `presentation::TABLE` (`presentation.rs:196`): `(id, Anchor, face)`. **No severity and no consequence column.** |
| Triage | `TriageClass { Safety, Action, Advisory }` (`stock/sim_triage.rs:173-180`) on `Finding` (`:184-193`), which wraps a `Diagnostic` | Rules in `sim_triage.rs`: collisions `:449-519` (Safety), copied diagnostics `:530-557` (Action), air cut `:656` (Advisory), rest slower `:757`, standing material `:1046`, rough islands `:1185`, entry load `:1462`, link cuts stock `:1758`, plunge class load `:1860` (Action) | GUI warnings tab, CLI `print_triage_report` (`rs_cam_cli/src/project.rs:1095-1102, 1498`), MCP. **Export reads none.** |
| Entry load (G-ENTRYLOAD) | `Caution`, or `Critical` at peak >= 1.0 mm (`sim_triage.rs:1266, 1455-1459`); class `Action`, `fix: None` (`:1462, 1504`) | `entry_load_finding` `:1444`; armed only for `FromRemainingStock` toolpaths (`session/compute/diagnostics.rs:429-437`) | Report only (`:1258-1259`: "it does NOT block export"). A fresh-stock op (Project Curve op 5) is never graded. |
| Legacy verdicts | `VerdictSeverity { Critical, Important, Polish }`, `VerdictKind` (`session/diagnostics_types.rs:104-150`); `ProjectVerdict` (`:448-466`) | `session/compute/diagnostics.rs:1346, 1367, 1506, 1543, 1562` | Adapter `from_project_diagnostics.rs:80-113` (exhaustive `id_and_category_for`; `severity_from_legacy`). Doc of `Critical` says "Block export until resolved" (`:106-107`); no export code reads it. |
| Per-toolpath summary | `ToolpathDiagnostic` (`session/diagnostics_types.rs:25-60`): counts, not verdicts | session | CLI destructures it (`rs_cam_cli/src/project.rs:50-130`) |
| Tool load | `LoadState { Within, Exceeds, Unmodeled }` (`tool_load/verdict.rs:928-932`), `CriterionKind` (`:938-956`), typed `ChiploadVerdict` / `PowerVerdict` / `DeflectionVerdict` / `DepthVerdict` (`:1674, 1800, 1945, 2069`), `DrillGateSeverity { Elevated, Critical }` (`tool_load/drill_gates.rs:80-84`); `BoundSource::gates_export` (`verdict.rs:1142-1144`); `CriterionStatus::refuses_export` (`:1276-1282`) | `tool_load::evaluate_toolpath` | Export (`gcode/mod.rs:827-880, 1054-1072`); adapter `from_tool_load` |
| Simulation issues | `SimulationCutIssueKind { AirCut, LowEngagement }` (`stock/simulation_cut.rs:139-142`); legacy `issue_count` | sim | counts only (`ChannelCounts`, `sim_triage.rs:206-249`) |
| G-CENTRECUT descents | `ToolpathCoreDescent` in `SimulationResult::non_centre_cutting_descents` (`compute/simulate.rs:828`) | `stock/collision.rs:772` | export refusal only; no `Diagnostic` |
| Model gouge | `ModelGougeFinding::blocks_export` (`compute/model_gouge.rs:253-269`) | generation stats | export refusal + diagnostic `safety.cuts_into_model` |
| G-code validator | `export::gcode_validator::Severity { Info, Warning, Error }`, `FindingKind` (`export/gcode_validator.rs:42-75`) | `validate_machine_safety` in `export_program` (`session/compute/export.rs:156-178`) | Logged and returned in `ExportedGcode::machine_safety`; never refuses ("advisory on every surface", `:48-49`). Doc of `Error`: "do not run without fixing". |
| Feeds / Suggest | `FeedsWarning` (`feeds/mod.rs:777`), `SuggestWarning` (`feeds/suggest.rs:250`) incl. `PlungeEntryUnstableAtDpp` (`feeds/suggest/adaptive_entry.rs:95-125`) | feeds | adapter `from_feeds` |
| Pins, load warnings | `PinFindingKind` (`compute/alignment_pins/mod.rs:299`), `ProjectLoadWarning` (`session/project_file.rs:572`) | — | verdicts / load reply |
| Generation findings | `GenerationFindings` struct of `Option` fields (`compute/execute.rs:169-…`) | `ctx.record` | adapter `from_generation` |

Severity is chosen at each construction site. Tally of literal choices: `from_feeds` Caution 8 /
Critical 1 / Hint 1 / Info 4; `from_generation` Caution 12 / Info 4; `from_preconditions` Blocking 9;
`from_static_checks` Caution 4 / Critical 3 / Hint 4 / Info 2; `from_tool_load` Caution 2 / Critical 1 /
Info 7; `sim_triage` Caution 3 / Critical 1 / Hint 4 / Info 1. One id can carry different severities
at different sites or readings (entry load: Caution or Critical).

### 3.2 What blocks what

**Generation** refuses on a current finding with `Severity::Blocking`, or the id
`compat.tool_kind_refused` (`session/run_state.rs:141-148`), read from `diagnose_toolpath_gate`
(`session/compute/export.rs:422`; `run_state.rs:225-245`). The gate list has no simulation finding.
Separately, `JobPhases::run_stages` refuses on `Produced::non_centre_cutting_refusal`
(`session/compute.rs:735-742`).

**Export** (`ProjectSession::export_program`, `session/compute/export.rs:65-197`) refuses on exactly
these `ExportRefusal` variants (`gcode/mod.rs:94-161`):

| Refusal | Source read | Override (`override_needed`, `gcode/mod.rs:166-181`) |
|---|---|---|
| `HeightOrder` | `height_order_refusal` (`export.rs:84-92`) | none |
| `CutsIntoModel` | `stats.model_gouge.blocks_export()` (`export.rs:93-102, 191-206`) | none |
| `NonCentreCuttingDescent` | `non_centre_cutting_refusal_rows` (`export.rs:103-122`) | none |
| `InchUnits`, `BadFeed`, `ControllerCompensation` | `emit_checked` (`gcode/mod.rs:1098-1108`) | none |
| `LoadExceeded` | `CriterionStatus::refuses_export` | `AcceptExceeded` |
| `LoadUnmodeled`, `StaleSimulation` | unmodeled rows | `AcceptUnmodeled` |

Overrides: `ToolLoadExportPolicy { accept_unmodeled, accept_exceeded }` (`gcode/mod.rs:29-32`); spellings
from `LoadOverride::mcp_key` / `cli_flag` / `gui_label` (`gcode/mod.rs:48-88`). CLI flags
`--accept-unmodeled-tool-load`, `--accept-exceeded-tool-load` (`rs_cam_cli/src/main.rs:255-263,
345-353` at HEAD); MCP `ExportParam.accept_*` (`rs_cam_viz/src/app/mcp.rs:795-799`); GUI checkboxes.

`export_program` reads **no** `Diagnostic`, **no** `Severity`, **no** `TriageClass`. These do not
block export on any surface: rapid / holder / obstacle collisions (triage Safety), entry load,
link cuts stock, plunge class load, plunge stress, machine-safety `Error`.

**Per-surface gates outside the core door:**
- GUI pre-flight: collisions need an "I understand the risks" checkbox and "Export Anyway"
  (`rs_cam_viz/src/ui/preflight.rs:241-300`). CLI and MCP have no such step.
- GUI and MCP: an enabled op with no current result refuses (G-EXPORTSKIP / G-STALEXPORT,
  `rs_cam_viz/src/io/export.rs:222-262, 380-410`). The core `export_gcode_checked` (the CLI door)
  still collects with `filter_map` over `result_at` (`session/compute/export.rs:229-243`).

### 3.3 Is the finding → consequence map explicit?

No. It is scattered over at least five places:

1. `Severity` doc claims (`diagnostics/mod.rs:117-129`, `is_actionable` "used by the export gate"
   `:153-157`) — the export does not read it.
2. `run_state::refuses_generation` — `Blocking` plus one named id.
3. `ExportRefusal` variants, each built by its own check in `export_program` / `emit_checked`.
4. `BoundSource::gates_export` — the one typed, exhaustive consequence rule (tool load only).
5. GUI-only pre-flight checks.

Typed precedents that already force a decision: `ExportRefusal::override_needed` (exhaustive
match), `BoundSource::gates_export`, `id_and_category_for` (exhaustive over `VerdictKind`),
`TriageClass` routing ("a new class does not compile on a surface", `sim_triage.rs:164-171`),
`DRESSUP_PIPELINE` with `all_variants!` (`dressup_apply.rs:287-299`), the command `Surfaces { gui,
mcp, cli }` rows with `Reach::Skip(reason)` (`session/command.rs:128-934`), and `diag_ids!` + the
`TABLE` test (W1).

---

## 4. The command contract and the Job lane

- `ProjectSession::apply(Command) -> Result<Effects, SessionError>` (`session/command.rs:2731`).
  The `for_each_command!` list (`:128-934`) generates `Command` (`:2449`); kinds at HEAD: 74
  `Command`, 6 `Query`, 5 `Job`. Each row states `Surfaces { gui, mcp, cli }`.
- `Job` rows: `GenerateToolpath` (`:876`), `RecommendClearingStrategy`, `PreviewTierMap`,
  `BuildModelFaces`, `BuildModelPickIndex`. Generation runs `execute_job` (`session/compute.rs:543`)
  → `JobPhases::run_stages` (`:719-744`): `generate` → `apply_dressups` → `clip_and_split_entries`
  → the G-CENTRECUT refusal; then `finish` (`:1206`) joins `GenerationFindings`.
- Export and simulation are not command rows. Export is the method `export_program` /
  `export_gcode_checked`; simulation is `run_simulation` plus `AdoptSimulation` (`command.rs:883`).
- `Effects` (`command.rs:1280-…`) carries stale sets and revisions, not findings.

Where a typed gate would hook in:
- **Generation side:** `JobPhases::run_stages`, after `clip_and_split_entries` (the G-CENTRECUT
  refusal at `session/compute.rs:738` is the model). The final moves exist there; the dressup
  pipeline has run.
- **Export side:** the top of `export_program` (`export.rs:84-122`), beside the three session
  refusals. All three surfaces pass through it (GUI/MCP `rs_cam_viz/src/io/export.rs:481`; CLI
  `rs_cam_cli/src/project.rs:1162`, `run.rs:259`).
- **Typed cut description:** `Produced` (`trace/produced.rs:29-…`) is the typed side channel of a
  kernel and survives the dressups by contract (`translated`). An entry record would ride there.

How surfaces read findings today:
- MCP: serde of `Diagnostic` (ids, severity, face) and the triage; export refusal `body()` text.
- GUI: `Diagnostic` lists (badges by `Severity`, `rs_cam_viz/src/ui/properties/tab_badges.rs:136`),
  the triage, and its own pre-flight reads of `sim.checks` and the load report (strings such as
  "Rapid collisions", `ui/preflight.rs:114-126`).
- CLI: `summary.json` with its own `ToolpathDiagnostic` wire, the triage print, and
  `GcodeExportSummary`; `rough-score` counts intents with its own rule.

One vocabulary exists for ids (`diag_ids!`). The consequence vocabulary is not shared: each surface
decides what blocks, and the core export door does not use the id vocabulary.

---

## 5. Suggestions (not facts)

### 5.1 Each op states how it enters (compile-time)

Replace `DressupPolicy` + `EntryStylePolicy` + 3D Rough's own field with one registry field that has
no default and no wildcard:

```rust
pub enum EntryContract {
    /// The dressup builds the entry. `allowed` is a non-empty set.
    Dressup { allowed: EntrySet, default: RoleDefault },
    /// The planner builds its entries and states their kinds.
    Planner { kinds: EntrySet },
    /// The descent is the cut (Drill, Pin Drill, Bore).
    DescentIsCut,
    /// A straight plunge is the only entry. The row must name the
    /// reason and the feed rule, and a non-centre-cutting or unknown tool
    /// refuses at generation.
    StraightPlunge { reason: &'static str, feed: PlungeFeedRule },
}
```

- `strip_all(reason)` then cannot leave an op with no stated entry: Project Curve must pick
  `StraightPlunge { … }` (today's behaviour, now visible) or a new `Planner` on-curve ramp (F-e1).
- `effective_entry_style` reads the contract, not `cfg.entry_style` alone, so the G-CENTRECUT
  fallback cannot add a ramp to an op whose contract forbids one.
- A `PlungeFeedRule` (op `plunge_rate`, G6 axial chip, …) makes the plunge feed a stated choice,
  and replaces the hidden 0.5 x first-feed rule in `entry_plunge_rate`.
- A sentry iterates `OperationType::ALL` and asserts each `StraightPlunge` row has a matching
  finding (5.3), so a straight plunge is never silent.

### 5.2 The IR states each entry as one record

- Add an entry record per entry (kind, start, end, depth into stock, feed), emitted by the builder
  that makes it (`apply_entry`, the 3D Rough ladder, pencil, surface link, the generic emitters).
  Carry it in `Produced` or as `SpanKind::Entry { kind }`. Readers (`rough-score`, triage, the
  modulator) read the record, not the first move.
- Add `MoveIntent::EntryApproach` (air lead at feed) and `MoveIntent::StepDown`, or keep the move
  tags and let the record carry the kind. The blast radius of a new variant: 24 files read
  `EntryPlunge`; every `match` on `MoveIntent` without a wildcard fails to compile, which is the wanted effect.
- Make `apply_entry` select by intent as well as geometry, so a `Linking` descent is not rewritten.

### 5.3 Each finding kind states its consequence (compile-time)

- Add a mandatory column to `diag_ids!`: `consequence: Consequence` with
  `BlockGeneration`, `BlockExport { override: Option<LoadOverride> }`, `Warn`, `Info`. The macro
  then emits `fn consequence(id) -> Consequence`; a row without it does not compile. Keep
  `Severity` for display only, or derive it from the consequence for the safety ids.
- Give each export refusal an id (G-CENTRECUT descent has none today) so the export door and the
  panels name one thing.
- Make `export_program` compute its refusals from the current findings: for each emitted toolpath,
  each finding whose id maps to `BlockExport` refuses, unless the policy holds its override. The
  load gate already works this way per row (`refuses_export`); this widens it to every id.
- Decide per id, in the one table: `project.entry_load` Critical, `project.link_cuts_stock`,
  `project.rapid_collision`, `project.holder_collision`, `project.obstacle_collision`, machine-safety
  `Error`. Today these block nothing in the core door; the GUI checkbox becomes an override row,
  so the CLI and MCP get the same gate.
- Arm `entry_load` for fresh-stock ops too, or state in its row why a fresh entry is not graded.
- Add a finding for a plunge with `non_centre_cutting = None` on a tool with no stated centre
  data, which needs a three-state tool field (`CentreCutting::{Yes, No(limits), Unknown}`).

### 5.4 Order of work (lowest risk first)

1. `rough-score` counter reads any-intent-in-run (CLI only, report 05 option 1).
2. `consequence` column on `diag_ids!` with today's behaviour written down (no change in result);
   a sentry that `export_program` refuses exactly the `BlockExport` ids.
3. `EntryContract` in the registry with today's behaviour; the pipeline reads it.
4. Change decisions row by row (Project Curve entry, entry-load refusal, collision refusal).
