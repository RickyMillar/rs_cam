# Untouched-territory risk map — W9 / R8 bounded scouts

Date: 2026-08-04
Wave: W9 (M4 / R8), research-only, no-Cargo lane
Parent revision inspected: `63d5e8b` (branch `experiment/adaptive-spiral`)
Method: source reading only — `rg` / `grep` / `Read` / registry sources. **No Cargo command was run**, so nothing here is a runtime observation; every claim is a code-location claim plus a stated reproduction hypothesis.

> **ERRATUM, 2026-08-06 (W10 close-out, plan §L1 stale-rationale sweep).**
> Every "twelve report-only findings" statement in this document was
> correct when written and is now **fourteen**: Checkpoint C landed two
> `ToolpathStats` slots, not one — `offset_library_failures`
> (`crates/rs_cam_core/src/compute/config.rs:420`) **and**
> `boundary_clip_dropped` (`:438`). The contract split moves with it:
> **eleven** share the `None` = not-measured / `Some(0.0)` = measured-clean
> contract and **three** are deliberately outside it (`derived_stepovers`,
> `zero_removal`, `boundary_clip_dropped`), not ten and two. The record
> below stands verbatim per errata discipline; `CLAUDE.md` and
> `FINISHING_OPEN_DEFECTS_EVIDENCE.md` §0 carry the current counts.

> **Second erratum, same date and wave.** Two of this map's three `fix
> now` items were ruled and executed on 2026-08-06 and their present-tense
> risk statements no longer hold: **P-1** (grblHAL downgraded by both
> project-path readers) is fixed by one shared resolver — and a **fourth**
> reader this map did not name, `crates/rs_cam_viz/src/io/project.rs:835`,
> was found by `rg` and routed through it too; **P-2** (setup datum not
> persisted) is fixed by consolidating viz's two byte-identical private
> datum copies onto one core definition and deleting `SetupRuntime`
> outright. **P-3** was downgraded by the orchestrator to *ruled out (no
> live population)* on 2026-08-04 and was not fixed. See
> `TECH_DEBT_2_CLOSEOUT.md` §1 (M4) and `ORCHESTRATION_LOG.md` "io-fixes".

## How to read this

Each area gets one page: a data-flow map, then the top five risks ranked by
`GUI reachability x silent-failure potential x blast radius`.

Dispositions:

- **fix now** — crash or data loss, reachable through the shipped GUI/MCP. Per plan §M4 "Preferred fix shape", these are *logged with a reproducer*, not fixed inside this wave.
- **candidate for next programme** — real, evidenced, but not data loss; needs a checkpoint and a red-first test.
- **ruled out** — inspected and found sound, or intentional and documented. Recorded so the next campaign does not re-scout it.

Explicitly **not** scouted (owned elsewhere, referenced as known):

- GUI worker hand-copies core generation findings (`crates/rs_cam_viz/src/compute/worker/execute/mod.rs`) — **W1 / R7-H1**.
- `arcfit` intent inheritance — **W2 / R7-H2**.
- `screenshot_toolpath` renders new-style UnifiedFinish emission near-empty (D-LV.1) — **W8 / R7-M2**. That defect's *shape* is the search template for area 3; the defect itself is not re-litigated here.

Cross-cutting observation, stated once: **three of the four areas' top risks are the same structural pattern** — a value has two independent interpreters, one complete and one incomplete, and the incomplete one sits on the production path. `P-1`, `X-1`, `X-2`, `G-1` are all that shape.

---

## Area 1 — Import / mesh

### Data-flow map

| Format | Parser | In-memory type | Unit normalisation | Scale applied by |
|---|---|---|---|---|
| STL | `stl_io::read_stl` — `crates/rs_cam_core/src/mesh.rs:46` `TriangleMesh::from_stl_scaled` | `TriangleMesh` | none in-format | caller `scale` multiplies raw floats |
| SVG | `usvg::Tree::from_data` — `crates/rs_cam_core/src/svg_input.rs:46` `load_svg_data` | `Vec<Polygon2>` | usvg emits **CSS px @ 96 DPI**; `load_svg_data_mm` (`svg_input.rs:72`) exists to convert | `io.rs:60` `apply_uniform_scale_2d(units.scale_factor())` |
| DXF | `dxf::Drawing::load_file` — `crates/rs_cam_core/src/dxf_input.rs:104` `load_dxf_full` | `DxfImport{polygons, drill_targets, layers}` | **self-normalising**: `insunits_to_mm_scale` applied inside `extract_dxf` (`dxf_input.rs:117-141`) | `io.rs:80-81`, a *second* scale |
| STEP | `truck_stepio::Table::from_step` + per-face `robust_triangulation` — `crates/rs_cam_core/src/step_input.rs:55-150` | `EnrichedMesh` + `FaceGroupId` per BREP face | **none — file length unit never read** | `io.rs:100-103` `EnrichedMesh::apply_uniform_scale` |

Single funnel: `crates/rs_cam_core/src/io.rs:29 load_model_file(path, id, kind, units)`. GUI wrappers in `crates/rs_cam_viz/src/io/import.rs` pass `units_for_scale(scale)`; every GUI import entry point passes `scale = 1.0`. MCP `import_model` (`crates/rs_cam_viz/src/mcp_server.rs:663-673`, `ImportModelParam { path }`) has **no** units field.

Downstream: `SpatialIndex`, drop-cutter, `dexel_mesh.rs`, `stock_mesh.rs`, and — for STEP only — per-toolpath `face_selection: Option<Vec<u16>>` persisted into the project file.

### Top five risks

| # | Risk | Location | Disposition |
|---|---|---|---|
| I-1 | **STEP `FaceGroupId` is assigned from `HashMap` iteration order across shells, and the primary loader validates nothing.** `step_input.rs:68` iterates `table.shell.values().enumerate()`; `truck-stepio-0.3.0/src/in/mod.rs:71` declares `pub shell: HashMap<u64, ShellHolder>` with `std::collections::HashMap` (import at `:14`), i.e. RandomState — order varies per process. Face IDs within one shell come from a `Vec` and are stable; **multi-shell** files are not. The IDs are persisted (`crates/rs_cam_core/src/session/project_file.rs:427,665-668`) and the primary (core) loader maps `u16 -> FaceGroupId` with **no** bounds or identity check. The only staleness check in the tree — `ProjectLoadWarning::FaceSelectionStale` — lives in `crates/rs_cam_viz/src/io/project.rs:1212-1230`, which is now the *fallback-only* loader (see P-2). Repro hypothesis: save a project with a face-selective toolpath on a multi-shell STEP model, restart the GUI, reload — the toolpath binds to a different physical face with no warning. | `crates/rs_cam_core/src/step_input.rs:68,149`; `crates/rs_cam_core/src/enriched_mesh.rs:19` (doc claims determinism); `crates/rs_cam_core/src/session/project_file.rs:665` | candidate for next programme |
| I-2 | **SVG imports treat CSS pixels as millimetres; the correct converter has zero production callers.** `crates/rs_cam_core/src/io.rs:58` calls `svg_input::load_svg` (px), never `load_svg_data_mm` (`svg_input.rs:72`, `PX_TO_MM = 25.4/96`). A repo-wide grep for `load_svg_data_mm` finds only its own definition and doc references. Default units are `Millimeters` (scale 1.0), and MCP `import_model` cannot override. Repro hypothesis: import any SVG authored `width="100mm"` and read `inspect_model` — the reported extent should be ~3.78x the authored size. **Needs a runtime check before it is called a defect**: usvg applies the viewBox transform, so the exact factor depends on the document, and the 0.1 flattening tolerance is also in px. | `crates/rs_cam_core/src/io.rs:58`; `crates/rs_cam_core/src/svg_input.rs:37-46` vs `:63-87` | candidate for next programme |
| I-3 | **STEP length unit is never read; import always assumes mm.** No code path in `step_input.rs` inspects the STEP unit context. `default_units_for_kind` (`crates/rs_cam_viz/src/io/project.rs:1332-1333`) returns `Millimeters` for every kind including `Step`, and `Controller::rescale_model` explicitly **excludes** STEP from the post-hoc rescale (`crates/rs_cam_viz/src/controller/io.rs:84-86`) — so an inch-unit STEP has no in-product correction at all. Repro hypothesis: import an inch-unit STEP; bbox reads 25.4x small. | `crates/rs_cam_core/src/step_input.rs`; `crates/rs_cam_viz/src/controller/io.rs:84-86` | candidate for next programme |
| I-4 | **DXF unit scale is applied twice on the "Import as:" rescale path.** `extract_dxf` already converts `$INSUNITS` to mm (`dxf_input.rs:117-141`, and its own doc at `:96` says so), then `io.rs:80-81` applies `units.scale_factor()` on top. `rescale_model` guards only `ModelKind::Step` (`controller/io.rs:84-86`), not `Dxf`. Repro hypothesis: import a DXF, then set units to "Inches" in the Properties panel — geometry scales 25.4x on already-correct mm coordinates. | `crates/rs_cam_core/src/io.rs:76-96`; `crates/rs_cam_core/src/dxf_input.rs:117-141`; `crates/rs_cam_viz/src/controller/io.rs:70-100` | candidate for next programme |
| I-5 | **STEP per-face tessellation failures are swallowed into a log line.** `step_input.rs:92-103` wraps `robust_triangulation` in `catch_unwind` and, on panic, `warn!(...); continue;` — the face is permanently absent from `face_groups`. `load_step` errors only if **every** face fails (`step_input.rs:168-170`). No structured warning reaches the GUI modal or the MCP reply, so a partially-tessellated model looks like a clean import with holes in it. Repro hypothesis: import a STEP with a face `truck` cannot triangulate; compare `inspect_brep_faces` count against the CAD source. | `crates/rs_cam_core/src/step_input.rs:92-103,168-170` | candidate for next programme |

Also inspected, **ruled out** as a standalone risk: STL index bounds *are* checked (`mesh.rs:78-89`); `BoundingBox3::expand_to` (`geo.rs:37-44`) is NaN-tolerant. But `Triangle::new` (`geo.rs:161-177`) substitutes `(0,0,1)` for a degenerate normal rather than rejecting, and `fix_winding` seeds on "most upward normal" (`mesh.rs:307-319`) — a degenerate-heavy STL can bias winding repair. Kept as a footnote, not a top-five item, because no reachable user report is attached to it.

---

## Area 2 — Project IO

### Data-flow map

There are **two complete, independently maintained TOML schemas for the same file format**:

- `crates/rs_cam_core/src/session/project_file.rs` (`ProjectFile`, 101 `serde(default)` sites) + writer `crates/rs_cam_core/src/session/save.rs:70 to_project_file`. **This is the primary path.**
- `crates/rs_cam_viz/src/io/project.rs` (`ProjectFile`, `LegacyProjectFile`, 111 `serde(default)` sites, `save_project`, `load_project`). **Fallback-only for load; its `save_project` has no non-test caller.**

Load: `Controller::open_job_from_path` (`crates/rs_cam_viz/src/controller/io.rs:166`) tries `ProjectSession::load`; on **any** error it `tracing::warn!`s and falls through to `crate::io::project::load_project`, then re-materialises a session via `build_session_from_legacy_job` (`controller/io.rs:312-432`).
Save: `Controller::save_job_to_path` (`controller/io.rs:152`) always uses `ProjectSession::save`.
MCP `load_project` / `save_project` (`crates/rs_cam_viz/src/app/mcp.rs:2736,2764`) are thin wrappers over exactly those two.
CLI `rs_cam project` uses `ProjectSession::load` too; `rs_cam run` uses a *third*, unrelated schema (`crates/rs_cam_cli/src/job.rs` `JobConfig`).

No struct anywhere carries `deny_unknown_fields`: an unknown key is silently dropped on load, and the writer rebuilds the file from the struct — so any key the primary schema lacks is erased by the next save.

### Top five risks

| # | Risk | Location | Disposition |
|---|---|---|---|
| P-1 | **grblHAL is silently downgraded to GRBL on reload, and the core export path cannot resolve it at all.** Two writers emit the token `"grblhal"`: `GuiState::post_to_session` (`crates/rs_cam_viz/src/state/runtime.rs:329`) and `AppEvent::WizardSetPost` (`crates/rs_cam_viz/src/app/input.rs:256`). Three readers parse it back, and only one is complete: `gcode::get_post_definition` (`crates/rs_cam_core/src/gcode/mod.rs:862-869`) knows `"grblhal" \| "grbl_hal"` — and is called **only** from `crates/rs_cam_cli/src/main.rs:490` and `sweep.rs:307`, both on the *job-file* path, never on a project. The two readers that are on the project path both fall through to GRBL: `GuiState::post_from_session` (`runtime.rs:311-315`, `"linuxcnc" \| "mach3" \| _ => PostFormat::Grbl`) and `gcode::export_project_gcode` (`crates/rs_cam_core/src/gcode/mod.rs:228-232`, same shape plus `"linux_cnc"`). `PostFormat::GrblHal` is user-selectable (`PostFormat::ALL`, `gcode/mod.rs:833-838`, rendered at `crates/rs_cam_viz/src/ui/properties/post.rs:17-19`) and has its own shipped definition (`post::grblhal()`, `gcode/post.rs:350`). `gcode/mod.rs:1052` already asserts the token resolves — the repo knows it, the production readers do not use the resolver that knows it. Repro: select **grblHAL** in the Post panel, save, reload — the dropdown reads **GRBL** and every subsequent export emits the GRBL definition. | `crates/rs_cam_viz/src/state/runtime.rs:311-315,329`; `crates/rs_cam_core/src/gcode/mod.rs:228-232,862-869` | **fix now** (silent data loss + wrong controller dialect) |
| P-2 | **Setup datum (XY method, Z method, notes) and per-setup model scope are live GUI controls that are never persisted.** `SetupRuntime { datum, model_ids }` (`crates/rs_cam_viz/src/state/runtime.rs:220-224`) is written by the Setup properties panel (`crates/rs_cam_viz/src/ui/properties/setup.rs:113-180`, two ComboBoxes) and read by the viewport datum crosshair (`crates/rs_cam_viz/src/app/gpu_upload.rs:604-645`) and the setup summary chip (`crates/rs_cam_viz/src/ui/setup_panel.rs:180-189`). Its only constructors are `GuiState::new()` -> `setup_rt: HashMap::new()` (`runtime.rs:300`) and `setup_rt_or_default` (`runtime.rs:356`). The primary wire has no home for it: core `ProjectSetupSection` (`crates/rs_cam_core/src/session/project_file.rs:288-305`) and `SetupData` (`crates/rs_cam_core/src/session/mod.rs:426-443`) carry no datum and no `model_ids`. The keys exist **only** in the fallback schema (`crates/rs_cam_viz/src/io/project.rs:301-316`, loaded at `:1075-1108`) — and `build_session_from_legacy_job` (`controller/io.rs:358-408`) drops them again even on that path. Repro: set Z Datum = "Model Top", save, reload — control reads its default and the TOML contains no `z_datum` key. | `crates/rs_cam_viz/src/state/runtime.rs:220-224,300`; `crates/rs_cam_core/src/session/mod.rs:426-443` | **fix now** (data loss on an operator-set, safety-relevant field) |
| P-3 | **`format_version <= 2` alignment pins are silently discarded by the primary loader.** The per-setup -> per-stock pin migration exists only in the fallback loader (`crates/rs_cam_viz/src/io/project.rs:731-752`). Core's `ProjectSetupSection` has no `alignment_pins` field and no `deny_unknown_fields`, so a v2 file parses **successfully** — every field is defaulted — the fallback is therefore never reached, and every `[[setups.alignment_pins]]` table is dropped. The next save writes the pin-less file back. Repro: craft a `format_version = 2` project with setup-level pins, `load_project`, then `inspect_stock` — `alignment_pins` is empty. | `crates/rs_cam_viz/src/io/project.rs:731-752`; `crates/rs_cam_core/src/session/project_file.rs:288-305`; `crates/rs_cam_viz/src/controller/io.rs:166-230` | **fix now** (silent data loss on legacy files) |
| P-4 | **The core `ToolpathDiagnostic` JSON wire is a hand-maintained `serialize_struct` with no exhaustiveness guard, while its CLI sibling has one.** `crates/rs_cam_core/src/session/mod.rs:1429-1470` writes 17 named fields by hand (16 struct fields plus the deliberate `standing_material_mm2` dual key). Adding a field to `ToolpathDiagnostic` (`session/mod.rs:678-731`) compiles and silently never reaches the MCP/JSON consumer. The CLI's parallel view solves exactly this with a `..`-free destructure whose docstring says so (`crates/rs_cam_cli/src/project.rs:104-129`). Counts are correct **today** (verified field-by-field). Separately: of the twelve report-only findings on `ToolpathStats` (`crates/rs_cam_core/src/compute/config.rs:135+`), only six reach `ToolpathDiagnostic`; `clipped_band`, `deprecated_dial`, `derived_stepovers`, `ramp_reach_clamp`, `claims_reference`, `retract_trips`, `zero_removal` do not. `retract_trips` in particular has no `diagnostics/ids.rs` entry and no `from_generation.rs` adapter. Repro: add a field to the struct and observe the JSON is unchanged. | `crates/rs_cam_core/src/session/mod.rs:1429-1470,1514-1536` | candidate for next programme |
| P-5 | **The silent primary->fallback loader fall-through, and the two schemas' field drift.** `controller/io.rs:227-229` degrades to a different schema on any `SessionError` with only a `tracing::warn!`; the user sees no modal saying which loader ran. Field-set diff of the two schemas (`pub <ident>:` extraction, both files): core-only keys `padding`, `workholding_rigidity`, `auto_from_model`, `flip_axis`, `high_feedrate`, `high_feedrate_mode`, `spindle_strategy`, `format`; viz-only keys `visible`, `locked`, `auto_regen`, `xy_datum`, `z_datum`, `datum_notes`, `model_ids`, `stock_x/y/z`, `stock_origin_*`, `tool_index`, `input`, `params`, `warnings`. `visible`/`locked`/`auto_regen` are the same class as P-2 (the session path forces `ToolpathRuntime::new(true)` at `controller/io.rs:180`, ignoring the file's `auto_regen`). Also here: `ProjectStockConfig` defaults every field (`project_file.rs:72-94`) while the GUI-fallback wire is the raw `StockConfig` with **no** `serde(default)` on `x/y/z/origin_*/padding/material` (`crates/rs_cam_core/src/compute/stock_config.rs:140-160`) — asymmetric strictness across the two loaders on the same table. | `crates/rs_cam_viz/src/controller/io.rs:227-229`; `crates/rs_cam_core/src/compute/stock_config.rs:140-160` | candidate for next programme |

**Ruled out, with evidence** (do not re-scout): `machine_ref` is deliberately read-then-dropped under snapshot semantics, and both halves are pinned (`crates/rs_cam_core/src/session/mod.rs:1664-1732`) — the only `set_machine_ref` call in the tree passes `None` (`app/mcp.rs:2117`). `OperationConfig` is adjacently tagged (`#[serde(tag="kind", content="params")]`, `crates/rs_cam_core/src/compute/catalog.rs:536-538`), so no silent variant confusion; an unknown kind fails loudly. Tool wire fields are complete against `ToolConfig` (22 vs 22). `ZRotation::from_key`/`FaceUp` round-trip through `to_key` consistently. `LegacyProjectFile` cannot capture a modern file, because `LegacyJobSection.stock_x/y/z` have no defaults (`crates/rs_cam_viz/src/io/project.rs:389-391`) — the fallback fails loudly rather than mis-parsing.

Footnote-level drift (named for the record, not ranked): `FixtureKind` is written with `format!("{:?}", kind).to_ascii_lowercase()` (`crates/rs_cam_core/src/session/save.rs:189`) and read by `FixtureKind::from_key` (`session/mod.rs:329-336`) whose fall-through is `Clamp` — a new variant would silently become a clamp on reload. `ProjectPostConfig.format` has **two** disagreeing defaults: `#[serde(default)]` -> `""` when `[job.post]` exists but omits the key (`project_file.rs:133-134`), vs `"grbl"` from `impl Default` (`:155`) when the whole table is absent. Benign today only because every project-path reader treats an unknown token as GRBL — which is precisely P-1.

---

## Area 3 — Export / post-processing

### Data-flow map

The **post-processor layer proper is sound and is not a risk site**: four data-driven dialects (`crates/rs_cam_core/posts/{grbl,grblhal,linuxcnc,mach3}.toml`) behind one `PostDefinition` loader (`gcode/post.rs`), one IR translator (`gcode/program_builder.rs`), one emitter (`gcode/emitter.rs`), byte-parity tested, and inch export hard-refused rather than silently scaled (`gcode/mod.rs:669-682 refuse_inch_units`). The risk is in everything *around* it: the sites that re-classify moves or re-derive numbers the IR already holds.

Independent interpreters of "what kind of move is this", found:

- IR-level, `MoveType`: `gcode/program_builder.rs`; `crates/rs_cam_core/src/viz.rs` (six-plus separate inline matches: `:64-72`, `:331-338`, ~`:747`, ~`:809`, ~`:1389`, ~`:1508`).
- Span-level, `SpanKind`: GUI viewport `crates/rs_cam_viz/src/render/toolpath_render.rs:212-242` vs PNG exporter `crates/rs_cam_core/src/stock_mesh.rs:181-199`.
- Text-level, re-parsing emitted G-code: `crates/rs_cam_core/src/gcode/mod.rs:798-819` (`replace_rapids_with_feed`), `crates/rs_cam_core/src/gcode_validator.rs` (~`:424-568`), `crates/rs_cam_cli/src/nc_replay.rs:77-205`, and `gcode/emitter.rs:94-135` (`unsupported_mcode_in_line`, whose own comment says it "mirrors" the validator's word semantics).

### Top five risks

| # | Risk | Location | Disposition |
|---|---|---|---|
| X-1 | **The PNG exporter and the GUI viewport walk the span path in opposite directions and disagree on dressup precedence** — the same *shape* as D-LV.1, at a second site. `stock_mesh.rs:186` walks `path.iter().rev()` ("innermost-first") and early-returns on the first of `Entry/LeadOut/LinkBridge/DressupArtifact`; `toolpath_render.rs:224-236` walks the path forward, early-returns on `Entry/LeadOut/LinkBridge` but for `DressupArtifact` only records `decision` and keeps scanning. A move nested inside two such spans is coloured by the **outermost** kind live and the **innermost** kind in the PNG. The exporter also has no equivalent of the viewport's `DepthPass`-driven `pass_index` gradient — its non-span default is a flat `CUT_COLOR` (`stock_mesh.rs:136,198`). Repro: generate a UnifiedFinish or horizontal-finish path with lead-ins nested inside link bridges, then diff `screenshot_gui` against `screenshot_toolpath` for the same index. (`spans_valid` is correctly guarded on both sides — `stock_mesh.rs:159`, `toolpath_render.rs:213` — so that is not part of this defect.) | `crates/rs_cam_core/src/stock_mesh.rs:181-199`; `crates/rs_cam_viz/src/render/toolpath_render.rs:212-242` | candidate for next programme (coordinate with W8) |
| X-2 | **Four hand-written "estimated time" formulas, none of them the canonical value, and they disagree with each other.** All four compute `stats.cutting_distance / operation.feed_rate() * 60.0`: `crates/rs_cam_viz/src/ui/readiness.rs:132-146`, `crates/rs_cam_viz/src/io/setup_sheet.rs:77-90`, `crates/rs_cam_viz/src/ui/export_wizard.rs:935-938`, `crates/rs_cam_viz/src/ui/toolpath_panel.rs:480,484`. Their guards already differ: readiness and the setup sheet skip a toolpath when `feed <= 0`, while the export wizard uses `feed_rate().max(1.0)` — feed 0 yields a nonsense finite number instead of a skip. Meanwhile `narrate.rs:1603-1681` and `ui/sim_diagnostics.rs:645-646` report the authoritative simulator value, and the kinematics-aware `compute_cycle_time` (`crates/rs_cam_core/src/machine_kinematics.rs:353,460`) is reachable only from the CLI `nc-time` debug tool. The numerator is also wrong in the same direction for everyone: `Toolpath::total_cutting_distance` (`crates/rs_cam_core/src/toolpath.rs:196-210`) measures arcs by **chord**, not arc length, and sums every cutting move regardless of its own per-move feed (dressup links carry `link_feed_rate`, `crates/rs_cam_core/src/dressup.rs:1186`). Repro: on a toolpath whose link/plunge feed differs sharply from its cutting feed, compare the printed setup sheet against `narrate_toolpath`. The **exported setup sheet** is the operator-facing instance and is the reason this ranks second. | `crates/rs_cam_viz/src/io/setup_sheet.rs:77-90` and the three siblings above | candidate for next programme |
| X-3 | **`replace_rapids_with_feed` reclassifies moves by regexing already-emitted text.** `crates/rs_cam_core/src/gcode/mod.rs:798-819` matches only lines whose trim starts with `"G0 "` or `"G0X"`; `G00`, lowercase `g0`, and any rapid a custom postamble emits pass through unchanged. Its own doc (`:792-797`) admits this. Called from every GUI export when high-feedrate mode is on (`crates/rs_cam_viz/src/io/export.rs:191,247,296,363`). Repro: put `G00 Z10` in a per-toolpath post-gcode snippet, enable high-feedrate mode, export, and grep the output — that rapid survives while every emitter-produced `G0 ...` is rewritten. | `crates/rs_cam_core/src/gcode/mod.rs:798-819` | candidate for next programme |
| X-4 | **Three hand-rolled G-code text parsers with no shared word scanner.** `gcode_validator.rs` (machine-safety pass, run non-blocking after every GUI export via `crates/rs_cam_viz/src/io/export.rs:15-38 log_machine_safety`), `crates/rs_cam_cli/src/nc_replay.rs:77-205`, and `gcode/emitter.rs:94-135`. Any dialect quirk one handles and another does not — spacing, case, embedded comments — is a live divergence in a *safety* check whose output is a log line. `nc_replay` additionally ignores G91 and G20 by explicit comment (`nc_replay.rs:129`), so an incremental or inch file is silently reinterpreted as absolute mm. Repro: feed the validator and `nc-time` the same file containing `g0` lowercase or an inline `(comment)` mid-word and compare their move counts. | `crates/rs_cam_core/src/gcode_validator.rs:424-568`; `crates/rs_cam_cli/src/nc_replay.rs:77-205`; `crates/rs_cam_core/src/gcode/emitter.rs:94-135` | candidate for next programme |
| X-5 | **`viz.rs` re-implements the cut/rapid split six-plus times with no shared helper.** `crates/rs_cam_core/src/viz.rs:64-72,331-338`, plus inline matches in `toolpath_to_3d_html`, `toolpath_standalone_3d_html`, `simulation_3d_html`, `stacked_simulation_3d_html`. **Currently consistent** — all six use the `MoveType` kinematic split, so a UnifiedFinish feed-typed link renders as cutting everywhere. Listed as the pre-condition for the next D-LV.1: the first person to add intent-aware colouring will update some and miss others. | `crates/rs_cam_core/src/viz.rs` (six sites) | candidate for next programme (preventive: one `classify_move` helper) |

**Ruled out**: the post-definition layer itself (data-driven, single emitter, byte-parity tested, inch refused loudly). Note that P-1 lives at the *selection* of a post definition, not inside one.

---

## Area 4 — GUI state / defaults

### Data-flow map

A single setting can currently receive a value from up to five independent sources: the runtime struct's `impl Default`; a `#[serde(default = "fn")]` on the wire struct; the wire struct's own `impl Default`; a hardcoded literal in a GUI event handler or widget; and a `clap` `default_value` on a CLI subcommand. The MCP schema is a sixth (`crates/rs_cam_viz/src/mcp_server.rs`) but was checked and found to mirror core.

The `intra_region_hookup_mm` prior is confirmed exactly as described and is now the *documented* template: `UnifiedFinishParams::default()` = `6.0` (`crates/rs_cam_core/src/unified_finish.rs:196`) and `default_unified_finish_intra_region_hookup_mm()` = `6.0` (`crates/rs_cam_core/src/compute/operation_configs.rs:1130-1132`), held together by a comment at `unified_finish.rs:188-195` rather than by a shared constant.

### Top five risks

| # | Risk | Location | Disposition |
|---|---|---|---|
| G-1 | **Simulation resolution: `0.5` from clap vs `0.25` from the job-file serde default and the GUI.** `crates/rs_cam_cli/src/main.rs:169` (`project` subcommand) and `:262` (`smoke`) both declare `#[arg(long, default_value = "0.5")] resolution: f64`; `crates/rs_cam_cli/src/job.rs:143-145 default_sim_resolution()` returns `0.25`; `crates/rs_cam_viz/src/state/simulation.rs:621` initialises `resolution: 0.25`. All three feed the same tri-dexel cell size. Which wins: **(a)** `rs_cam project foo.toml` with no `--resolution` simulates at **0.5 mm**; **(b)** `rs_cam run job.toml` with no `sim_resolution` key, and the GUI's own state, use **0.25 mm**. This matters because collision counts and engagement both move with cell size — the repo already refuses to guess it on the MCP side (`mcp_server.rs:1134`) and plan rule 8 forbids cross-resolution collision clearance. Partial mitigation: the GUI sets `auto_resolution: true` alongside, so the literal is a seed rather than always the operative value — that must be confirmed at runtime before this is called a defect. Repro: simulate one project through both CLI entry points with no flag and compare `rapid_collision_count`. | `crates/rs_cam_cli/src/main.rs:169,262`; `crates/rs_cam_cli/src/job.rs:143`; `crates/rs_cam_viz/src/state/simulation.rs:621` | candidate for next programme |
| G-2 | **`Fixture::size_y` has a dormant third default of `30.0` against a live `15.0`.** `crates/rs_cam_core/src/session/mod.rs:355-360` puts `#[serde(default = "default_fixture_size")]` on **both** `size_x` and `size_y`, and `default_fixture_size()` (`mod.rs:369-371`) returns **30.0**. The on-disk wire disagrees: `ProjectFixtureSection.size_y` uses `default_fixture_size_y()` = **15.0** (`crates/rs_cam_core/src/session/project_file.rs:326-327,343-345`), and the GUI "Add Fixture" handler hardcodes `size_y: 15.0` (`crates/rs_cam_viz/src/controller/events/model.rs:391`). Which wins today: fresh GUI project -> **15.0** (the literal); old file missing the key -> **15.0** (`ProjectFixtureSection`'s fn). The `30.0` is currently unreachable because `build_fixtures` (`project_file.rs:676-694`) and `controller/io.rs:366-388` both copy field-by-field. It becomes reachable the moment anything deserialises a bare `Fixture` — an MCP mutation endpoint, a clipboard/patch API, a `serde_json::from_value::<Fixture>()`. Repro: `serde_json::from_str::<Fixture>()` with `size_y` omitted returns 30.0, double the shipped fixture depth, and fixtures are collision geometry. | `crates/rs_cam_core/src/session/mod.rs:355-371` vs `crates/rs_cam_core/src/session/project_file.rs:326,343` | candidate for next programme |
| G-3 | **Tool-material and cut-direction wires are typed on one loader and free-string on the other, with a silent coercion.** Core wire: `tool_material: String` (`project_file.rs:213`) parsed by `parse_tool_material` (`:519-525`) whose fall-through is **`Carbide`** for anything but `"hss"`; `parse_cut_direction` (`:527-534`) falls through to **`UpCut`**. Fallback wire: typed `ToolMaterial` / `ToolCutDirection` enums (`crates/rs_cam_viz/src/io/project.rs:194,196`) where an unrecognised token is a hard parse error for the whole file. Tokens agree today (`#[serde(rename_all = "snake_case")]`, `crates/rs_cam_core/src/compute/tool_config.rs:102-107,137-143`). A third material or a fourth cut direction would load as Carbide/UpCut on the primary path — silently changing a feeds-model input — while failing the entire file on the fallback path. Repro: hand-edit `tool_material = "cobalt"` and observe the tool loads as carbide with no warning. | `crates/rs_cam_core/src/session/project_file.rs:519-534` vs `crates/rs_cam_viz/src/io/project.rs:193-196` | candidate for next programme |
| G-4 | **`reach.rs` cites the wrong shipped default for `num_offset_passes`.** The production value a fresh GUI Pencil op receives is `1` — `PencilConfig::default()` (`crates/rs_cam_core/src/compute/operation_configs.rs:802-812`), wired at `crates/rs_cam_core/src/compute/catalog.rs:2750`. The library-only `PencilParams::default()` is `0` (`crates/rs_cam_core/src/pencil.rs:198-222`) and its own doc says it is never on the production path. Two comments in `crates/rs_cam_core/src/reach.rs:727,925` nonetheless assert "even at `num_offset_passes = 0` — the shipped `PencilParams` default". Not a runtime bug (the coverage cap takes the value as an argument), but it is a P11-class stale rationale asserting a default that is not shipped. Repro: read the two comments against `operation_configs.rs:810`. | `crates/rs_cam_core/src/reach.rs:727,925` | candidate for next programme (L1 stale-rationale sweep) |
| G-5 | **Two large parallel-literal default surfaces with no shared constant — currently fully in sync.** Tool: 14 literals in `crates/rs_cam_core/src/session/project_file.rs:222-269` vs `ToolConfig::new_default` (`crates/rs_cam_core/src/compute/tool_config.rs:200-232`) — all verified equal (diameter 6.35, cutting_length 25.0, helix 30.0, corner_radius 2.0, included_angle 90.0, taper_half 15.0, shaft 6.35, holder 25.0, shank 6.35, shank_length 20.0, stickout 45.0, flutes 2, carbide, up_cut). Stock: `ProjectStockConfig::default()` + its five serde fns (`project_file.rs:97-128`) vs `StockConfig::default()` (`crates/rs_cam_core/src/compute/stock_config.rs:166-183`) — all verified equal (100/100/25, padding 5.0, `Medium`, `auto_from_model = true`). Listed because a single-sided tuning pass on either surface reproduces exactly the class G-2 already exhibits. | as cited | candidate for next programme (preventive) |

**Ruled out, with evidence**: `MachineKinematics::default().junction_deviation_mm` — the `0.020` elsewhere belongs to the named preset `shapeoko_xxl_ricky_tuned()` (`crates/rs_cam_core/src/machine_kinematics.rs:206`), not a generic default; both generic sides are `0.010`. `ScallopParams::default().intra_pass_hookup_mm = 0.0` vs `default_scallop_intra_pass_hookup_mm() = 3.0` is an **intentional, documented** split (`crates/rs_cam_core/src/scallop.rs:138-143`) with the config value always threaded explicitly (`compute/execute.rs:1566`) — this is the correctly-handled version of the class. `crease_hookup_mm` 5.0 == `PencilParams::default().hookup_distance` 5.0. Rest-analysis MCP schema defaults (`mcp_server.rs:993`) match `compute/config.rs:1332-1334`. CLI `run`'s `--safe_z` / `--spindle_speed` / `--post` clap defaults (`main.rs:109-118`) match `ProjectPostConfig::default()`. `shallow_angle_deg` UI fallback 30.0 matches `execute.rs:1314`.

---

## Roll-up

| Disposition | Count | Items |
|---|---|---|
| fix now | 3 | P-1, P-2, P-3 |
| candidate for next programme | 17 | I-1..I-5, P-4, P-5, X-1..X-5, G-1..G-5 |
| ruled out (recorded, do not re-scout) | 12 | `machine_ref` snapshot semantics; `OperationConfig` adjacent tagging; tool wire completeness; `ZRotation`/`FaceUp` round-trip; `LegacyProjectFile` cannot capture a modern file; STL index bounds; NaN-tolerant bbox; the post-definition layer; `junction_deviation_mm`; scallop hookup intentional split; `crease_hookup_mm`; rest-analysis MCP schema and CLI `run` clap defaults |

**Single promotion, if only one is taken: P-1.** It is the only item that is simultaneously silently triggered by a normal GUI action, loses operator intent on disk, and changes the controller dialect of the emitted G-code — and the correct resolver (`get_post_definition`) already exists and is already unit-tested for the token that both production readers drop.

P-2 and P-3 are the same data-loss class one severity step down: P-2 loses an operator-set field on every project, P-3 loses geometry on legacy files only.

Per plan §M4, **nothing is fixed by this wave**. Each `fix now` item needs a red-first reproducer on the parent revision before any production change, and P-1's fix touches a serialized token — that is a Checkpoint-class decision, not a wave action.

---

## Orchestrator annotations — 2026-08-04

- **P-3 population check executed**: every project TOML under `planning/` is `format_version = 3` (grep over `planning/**/*.toml`, 7 files). No v≤2 file exists for this operator, so P-3 is downgraded per the agent's own handoff: **`ruled out (no live population)`** — the loader defect is real but keep it as a candidate hardening item, not `fix now`. Counts become: fix now 2 (P-1, P-2) · candidate 18 · ruled out 12 + P-3.
- **X-1 handed to W8**: the exporter/viewport span-walk divergence is folded into wave W8's brief alongside D-LV.1 (same defect shape, same files); not opened separately.
- **P-1 (grblHAL silent downgrade)** is surfaced to the operator as a checkpoint-class decision — it changes a serialized token and the emitted G-code dialect; no fix begins without a ruling.
