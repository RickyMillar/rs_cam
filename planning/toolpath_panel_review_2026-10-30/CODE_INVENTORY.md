# Code inventory of the toolpath inspector (2026-10-30, HEAD 4d2639487)

Paths are relative to `crates/rs_cam_viz/src/ui/properties/` unless written in full.

I've mapped everything the toolpath inspector draws. The main findings, before the tables:

- **Wrap sites:** three DANGER texts in the toolpath panel can wrap, even though the panel rule (`CLAUDE.md`) says nothing there wraps. The IC1 sentry never sees them because its fixture never triggers them.
- **No-tool state:** the same no-tool sentence is shown three times plus a red tab dot, and the inspector has no "Add tool" action.
- **Layout:** label columns come in three widths, and units sit in four different places.

Paths below are relative to `crates/rs_cam_viz/src/ui/properties/` (TP = `toolpath_panel.rs`, TB = `tab_badges.rs`, SS = `selection_section.rs`, LD = `linking_dressup.rs`, FS = `feeds_speeds.rs`, OPS = `operations/`) unless written in full.

## 1. Header (above the tab strip)

The panel is entered from `mod.rs:237 draw` → `mod.rs:1690 draw_toolpath_selection` → TP:44. Nothing else draws above it.

| Order | What | Where | Text / format |
|---|---|---|---|
| 1 | Name row: raw `ui.horizontal` with label and text box | TP:109-113 | `"Name:"`; hover `"{spec.label}: {spec.description}"` |
| 2 | Separator | TP:115 | |
| 3 | Generate row | TP:142, 1421-1460 | Button `"Generate"`, enabled when the project has tools and validation passes (TP:127). No disabled hover. |
| 3a | State word (truncates) | TP:1714-1822 | `"Manual"`, `"Ready"`, `"Computing..."`, `"Done"`, `"Done · edited since — regenerate"` (CAUTION), `"Waiting on upstream stock"`, `"Disabled"`, `"Refused"` (DANGER), and `BLOCKED_HEADER` `"Done · BLOCKED"`, which never truncates (`ui/toolpath_panel.rs:1058`). |
| 3b | Figures at the right edge, small | TP:1661-1710 | Feed stage: `"planned"` or `"modulated"` (`rs_cam_core/src/session/result_provenance.rs:102`), CAUTION when superseded. Move count: `"{n} moves"` or `"old: {n} moves"`. The cap figures (`"{n} moves at a load cap (planned), lowest {f} mm/min"`, `state/runtime.rs:247`) appear only on hover. |
| 4 | Last-run refusal face (only when there is no validation error) | TP:149-165 | Rest refusals: `"✕ Generate '{name}' first"` or `"✕ '{name}' left no regions"` with a `⟳` button, or `"Refused · source is this op"` (TP:1526-1552). Any other refusal: the raw error as a DANGER `OneLine`, truncated. |
| 5 | Validation refusal: at most one DANGER `OneLine` | TP:168-214 | `REFUSED_SEE_GEOMETRY` = `"Refused · see Geometry"`, `REFUSED_SEE_CUT` = `"Refused · see Cut"`, or else the first validator sentence, truncated. All errors are on the hover. |
| 6 | Tab strip (`TabStrip`, wraps in a narrow rail) with dots | TP:283-295, TB:610-620 | No pills or counts on tabs, only a dot (DANGER, CAUTION or INFO), mapped by `OWNER_TAB` (TB:68-202). Extra dots: Heights for plane order, shoe gap, or any pinned height (INFO); Feeds when the feeds result has warnings; Cut for the D53 refusal. |

**"3/8 dressups active" is not in the header.** It is a small label at the top of the Dressup tab: `"{active}/{total} dressups active"` (TP:1331-1341), OK colour when above 0. The total is `DressupKey::ALL` = 8 (`rs_cam_core/src/compute/config.rs:1359`), but that tab only draws 2 of the 8 (arc fitting and dogbone); the other 6 are on the Linking tab. A `"Reset to recommended"` button sits beside it.

**No pill or badge component (`CountPill`, `StatusChip`) is drawn in the header.**

## 2. The "6 hints" block

| Aspect | Detail |
|---|---|
| Built | TP:229-268, from `operations::collect_diagnostics`. A finding is a hint when it is `DiagnosticState::Current`, severity is not actionable (Info or Hint), and its category is not `State` (TP:255-263). |
| Shown | At the panel footer, after the tab body and after the reach rows and findings: `CollapsingHeader` with text `"{n} hints"`, small and muted, closed by default (TP:484-497). |
| Row | `render_diagnostic_row(..., RowTier::Hint)` (TB:383-498). Face: `"{Category}: {message}"`, muted, truncated. Hover: the message only, with no evidence and no confidence (TB:414-418). |
| Quirks | `"1 hints"`: no singular form. `id_salt("tp_hints")` has no toolpath id, so open/closed carries across toolpaths, unlike the findings expander (TP:461). Hints raise no tab dot (TB:236). |
| Footer findings above it | Actionable, merged gate rows (`"Gates: {suffix}"`, TB:376) and stateful rows. Capped at `FOOTER_FINDINGS_CAP` = 3 plus a `NoticeStack` overflow row (TP:450-476, 1594); critical rows always show. The shoe finding is added to this list (TP:272-273). A selection refusal row reads `"{prefix}the selection refuses. See the Geometry tab."` (TB:457). A Fix button reads `"✓ Fix ({v:.3})"`. |

## 3. Long prose in the panel

**Text that can wrap: live IC1/G-E violations the fixture never triggers**

| Site | Colour | Text | Length |
|---|---|---|---|
| SS:154 `wrapped_small_label` (report read error) | DANGER | the error string | variable |
| SS:1354 `wrapped_small_label` (selection refusal) | DANGER | `refusal.to_string()` | 100–300 chars |
| TB:744-747 raw `ui.label`, vertical layout (no-tool) | DANGER | `"{state}. Set a tool on the Cut tab"` | ~45–90 |
| `ui/feeds/compare.rs:976-1002` `.wrap()` (`theme::ERROR` = `tokens::DANGER`) | DANGER | `"Cannot apply — Suggest refuses this recipe {GLYPH_DETAIL}"` then `err.to_string()`, e.g. `"Suggest has no basis for {op} with a {tool} in {material}: {reason}"` | 60–200 |
| FS:129-133 `.wrap()` in a grid cell (drill plunge) | faint | `"{feed} mm/min"` / `"not set"` | short |
| `OPS/registry.rs:470,482` raw `ui.label` | body | `"Set the pocket depth to draw the diagram."` | 41 |
| `OPS/drill.rs:197` raw `ui.label` | body | `"The stock pins of {d} mm + {n} picked hole(s)"` | ~45 |

**Raw labels inside grid cells (they clip; they do not wrap)**
- LD:208: `caution.message()`, CAUTION. I did not read `DepthBeyondStock::message()`, so its length is unknown.
- LD:240: `through_cut.message()`. Same.

**One-line truncating notes, full text on hover (main ones)**
- TB:775 `"⚠ BREP topology not loaded"`.
- TB:636-649 `"⚠ {n} rest regions"` / `"⚠ One rest region covers {p}% of the part"`.
- TP:811 `"Pick the boundary items in the Geometry tab, under Boundary."` (60).
- TP:1062 `"Grid and regions: from the Rest depth detector"`.
- FS:73 `"Feeds unavailable: {e}"` (DANGER).
- OPS/mod.rs:500 `"Top height: {source}"` and :513 `"Bottom height: {note}"`. The notes are 60–90 chars, e.g. `"Not used by this operation. Its Pocket Depth, Glue Gap and Proud Height set the floor."` (OPS/mod.rs:284-294).
- LD:499 `"  Emits r {:.2} mm: capped {:.2} → {:.2} mm (no-core rule, geometry)"`.
- `OPS/bore.rs:49` `"{n} circle(s) bored"`.
- `OPS/boundary_2d.rs:586` `"open paths cut in drawn direction"`.

**Reach map: drawn in the panel FOOTER, below every tab.** The doc comment at `state/runtime.rs:992-993` says "The Geometry tab… print this one string". That is stale.

| Row | Where | Text | Length / form |
|---|---|---|---|
| Checkbox | TP:345-352 | `"Show reach map"`. Hover ~330 chars: `"Colour the model by what THIS toolpath's cutter can form: …NOT MEASURED and keep the plain model colour."` | Only for ops where `supports_reach_map()` (mesh and not roughing, `rs_cam_core/src/compute/catalog.rs:664`) |
| Status words on the checkbox row | TP:366-387 | `"reach: computing…"` / `"reach: not measured"` (CAUTION) | small |
| `DataTable "toolpath_reach_readings"` | TP:400-447 | `"Unreachable"` `"{p:.1} %"`; `"Max gap"` `"{g:.2}"` `"mm"`. Hover = `area_basis_note` + `grid_line`. | 1 row each |
| Scale note | TP:428-430 | `DataRow::note(Role::Info, reach_scale_note)`, from `state/runtime.rs:996-1016`: `"full colour at one tip radius R = {r} mm · grid floor {f} mm = {x} R: a reported gap under the floor is discretisation, not geometry · grey: not measured (walls, undersides, and the rim band {w} mm wide)"` | ~220–260 chars, truncated to one row |
| Over-statement | TP:431-438 | `DataRow::note(Role::Caution, "⚠ " + over_statement_note)`, from `rs_cam_core/src/maps/reach_map.rs:576-583`: `"the grid over-states gaps by up to its floor ({f} mm), so the true unreachable share is AT OR BELOW {p} %, and the {u} % unresolved is the band the grid cannot classify either way"` | ~190 chars, truncated; only when tolerance is below the floor |
| Failed | TP:441 | `"reach: {message}"` | |

**Panel toggle compared with the viewport dock toggle**

| | Inspector | Dock |
|---|---|---|
| Label | `"Show reach map"` | `"Model colour: Reach"` (`ui/overlays/registry.rs:1462`) |
| Hover | the ~330-char text above | `"Green where this cutter forms the surface inside the operation's tolerance, red where it cannot, neutral where not measured. Clears Scene ▸ Model ▸ Rest — both colour the model."` |
| Write path | Writes `state.viewport.show_reach_map` directly (`mod.rs:1747,1794`). This bypasses `set_overlay`, so Reach and the rest heatmap can both be on until `settle_exclusive_surfaces` runs (`ui/overlays/registry.rs:1923-1926`). | Through `set_overlay`, with preconditions such as `"select a finishing operation"` |
| Legend caveats | | `ui/overlays/legend_rail.rs:1235-1291` prints the same three core strings as separate faint/CAUTION lines, plus `"moves dimmed while reach map is on"` and `"unreachable {p}% {area_basis} · worst gap {g} mm"`. The legend line says "worst gap" at `{:.3}`; the panel says "Max gap" at `{:.2}`. |

## 4. Tabs

**Every op draws all six tabs.** `draw_toolpath_tabs` folds `ToolpathTab::ALL` (TB:614) and nothing filters it. The order is Geometry, Cut, Heights, Feeds, Linking, Dressup (`mod.rs:294-301`). The default tab is Geometry (TP:281).

| Op group | Geometry tab content (`geometry_tab.rs:40-127`) |
|---|---|
| Ops with a selection (`rs_cam_core/src/compute/catalog.rs:1382-1395`): Pocket, Profile, Adaptive, VCarve, Inlay, InlayPlug, Zigzag, Trace, Drill, Bore, Chamfer, ProjectCurve, AlignmentPinDrill, Face | Full Selection section |
| …and with a floor (Pocket, Bore, Drill, Profile, Adaptive, Zigzag; `catalog.rs:1281-1288`) | Also the `"Floor"` line and the `"Floor face"` slot (`geometry_tab.rs:145-228`) |
| Face | Selection section on its stock-outline default. Expect the header `"Default: the part outline"` or `"Default: every …"`, a Cuts row, and Pick/Reset. No floor line. |
| 11 ops with no selection: DropCutter, Adaptive3d, Waterline, Pencil, Scallop, UnifiedFinish, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish (`catalog.rs:1396-1406`) | **One button, `"Pick faces"`** (`geometry_tab.rs:236-266`), and nothing else unless the boundary source is Selection (then the `"Boundary"` section, :95-126). |
| An op with `!session_clip_applies()` and no selection | Empty tab. No current op hits this: Pin Drill has a selection. |

- **Dressup tab:** arc fitting and dogbone for every op, including 3D ops. Its own dogbone hover says `"Not needed for open pockets or 3D surfaces."` (LD:716).
- **Linking tab:** greys controls through `dressup_policy` but never hides the tab.

## 5. No-tool state

**Op with no tool, but the project has tools**

| Surface | Text |
|---|---|
| Header validation `OneLine` (DANGER) | `"No tool set. Set a tool on the Cut tab"` (`OPS/validate.rs:66-74`, TP:207) |
| Cut tab Tool combo | Selected text `"No tool"` (`rs_cam_core/src/session/op_draft.rs:53`). Tools the op refuses are greyed, with the refusal on hover. |
| Cut tab label (DANGER, wrappable raw label) | `"{state}. Set a tool on the Cut tab"` (TB:741-748). It tells you to go to the Cut tab while you are on the Cut tab. |
| Footer, Blocking row | `"Geometry: No tool set. Set a tool on the Cut tab — generation will fail."` (`rs_cam_core/src/diagnostics/adapters/from_preconditions.rs:166-169`, category Geometry) |
| Tab dot | Cut, DANGER (`PRECOND_NO_TOOL` → C, TB:177) |
| Cut tab pills | None (TP:571, 589-598) |
| Feeds tab | `"Explore…"`, the Speed grid (empty fields, hint "set a value"), and the Vendor Cutting Data viewer. `calculate_and_apply_feeds` is skipped (TP:1209). **The viewer uses a made-up `(6.0, EndMill)`** (TP:1195-1196) and prints `"{n} observations for End mill tools (current: 6.0 mm)"`. |

**Project with no tools at all**
- Generate is disabled (TP:127) with no hover saying why.
- The state reads `"No tool fits: needs …"` (`op_draft.rs:588-597`). With no tools that wording is wrong: nothing exists to fit.
- For an op with no kind constraint it becomes `"No tool fits: needs a tool"` (`rs_cam_core/src/compute/catalog/schema.rs:505-506`). I inferred that text from the code; I did not render it.
- The combo list is empty.

**There is no "Add tool" or "open tool library" action anywhere under `properties/`** (grep confirms). Both live only in the Setup rail: `"Manage library…"` and the `"Add tool"` menu (`ui/setup_panel.rs:243-256`).

## 6. Feeds

| State | What draws |
|---|---|
| Tool with no feeds basis (`FeedsError::Unbacked`) | One DANGER `OneLine`: `"Feeds unavailable: Suggest has no basis for {op} with a {tool} in {material}: {reason}"` (FS:69-76). The Feed, Plunge and Spindle rows are empty with the hint "set a value" (FS:109-118). Spindle hover: `"The spindle speed of this operation. Suggest writes it when the tool has feeds data; else set it here."` No Suggest or Apply button appears. |
| Result present | Comparison card (`ui/feeds/compare.rs`). Buttons appear only when something differs (`compare.rs:1005`): `"⚡ Apply all — changes the cut"` → `AppEvent::ApplyFeedsAll` and `"⚡ Speeds only — holds the cut"` → `ApplyFeedsSpeeds` (`compare.rs:38,44,1013-1050`). Also `"Explore…"` (TP:1199). |
| Per-field ⚡ pills (Cut tab) | A `⚡` button beside a field (`ui/components/suggest.rs:93`). It writes only on click (LD:297-327); TP:650-655 then stamps provenance. |

There is no button labelled "Suggest", and nothing applies automatically. Quotes:
- FS:15-18: `"The calculator never writes to the operation here (Roadmap F.5). The canonical comparison's `Apply all` is the only path that pushes calculated values into the operation."`
- FS:95-97: `"This block does not consult the calculator. It sets what the operation runs. `⚡ Apply all` remains the one route from a recommendation into an operation"`.
- The memo refresh (FS:39-50, TP:572-583) only fills `entry.feeds_result` and the previews.

## 7. Layout primitives

**Widths**
- `LABEL_COL_WIDTH` = 116 (`ui/tokens.rs:823`).
- `param_grid` = 2-column `egui::Grid` (`ui/components/section.rs:65-71`).
- DataTable name column = 0.42 × width (`ui/components/data_table.rs:50`): about 117 at 280, about 101 at 240. That is a third label width.

| Area | Primitive |
|---|---|
| Name, Tool, Input, boundary Source/Containment/Offset, Rest source, Rest Reference/Cell Size/Min Valley Depth/Region Margin | Raw `ui.horizontal` + `ui.label("X:")` + combo or `DragValue` (TP:109; TB:716, 751; TP:692, 833, 919, 1000, 1072-1143). No column; each label is its natural width. |
| Op parameters | `param_grid` + `ValueRow`/`dv`/`dv_pill` (`label_cell` 116), mixed with raw `ui.label("Direction:")` + combo or empty-label checkbox in the same grid (`OPS/surface_3d.rs` 94-964, `OPS/boundary_2d.rs` 209-780, `OPS/finishing.rs`, `OPS/engrave.rs`, `OPS/project.rs`) |
| Selection summary, notes, Skipped items | `DataTable` / `Details` (SS:161-187, 1251-1307, 1393) |
| Selection list rows, Floor line | `line_with_actions` (SS:1172-1230): truncated label with quiet buttons on the right, no name column |
| Heights | 4-column `Grid("heights_p")`, raw `ui.label` labels at auto width (OPS/mod.rs:145, 419-479), then a **separate** 2-column `param_grid("heights_floor")` whose `"Bottom height:"` uses `label_cell` 116 (OPS/mod.rs:528; `OPS/boundary_2d.rs:74`). The two label columns do not line up. |
| Feeds Speed | `named_section` + `param_grid` + `OptionalValueRow`. The drill Plunge cell is a raw wrapping label (FS:126-139). |
| Feeds card, operating point | `DataTable` |
| Vendor LUT | Raw 7-column striped `egui::Grid`, 9 pt font, in a horizontal `ScrollArea` (FS:585-596) |
| Linking | `SectionHeader`, raw horizontal `"Entry Style:"` row (LD:423), checkboxes outside any grid, several small `param_grid`s with indented labels (`"  Max Angle:"`, LD:452 and on), an `Advanced` disclosure built inline (LD:634) rather than with `advanced()` |
| Dressup | Checkboxes + one-row `param_grid`s |
| Reach readings | `DataTable` |
| `ChoiceRow` | Not used in any toolpath tab. It appears only in the shoe row on Heights (`shoe_row.rs:72`) and other panels. |

**Units placement**
1. `ValueRow` suffix inside the `DragValue` (`" mm"`, `" deg"`, `" mm/min"`).
2. The heights grid has no unit; it is on the label hover (OPS/mod.rs:148-151).
3. The `DataTable` unit is its own cell (`"Max gap"`, `"mm"`), but `"Unreachable"` puts `" %"` inside the value.
4. Cut-tab Offset and Rest dials use a raw `DragValue` with `.suffix(" mm")`.

The Floor line uses `"Z {:.2}"` with no unit.

## 8. Picked-face count (Selection section)

- **Cuts row** (`DataTable`, SS:832-860): the value is `counts_text`, e.g. `"4 faces (1 down) · 2 islands"` (SS:810-829). The status chip reads `"OK"`, a findings count such as `"3"`, or `"Refused"`. While STL faces compute it reads `COMPUTING_FACES` with `"Wait"`.
- **Selected row** (off the default only): `"{n} item"` / `"{n} items"` (SS:171).
- **Group header** (`line_with_actions`): `"Picked faces ({n})"` or `"Picked faces ({kept} of {n})"` when filtered (SS:586, 620, 375-381), with a `▾`/`▸` button.
- **Each row**: `"Face Z12.50 · Cut"` (`face_label` `"Face Z{z:.2}"`, SS:427, 712). Without a plane it reads `"Face {m:.1} mm"`. Acts on the right: `"Rule…"`, `"Use as floor"`, `"✕"`. The hover is `"Face Z {z} at ({x}, {y}), {m} mm across · {layer} · Cut"`.
- More than 8 rows: a filter box (`"Filter the selected items"`) and `"Show {n} more"`.

## 9. Sentries

**`tests/the_inspector_text_is_bounded_ic1.rs`**

It renders `properties::draw` for a Pocket (2D), an Adaptive3d and a DropCutter on all six tabs at 280 (with line caps) and at 240 (layout only), plus 7 other panels. Core assertions:
```rust
let toolpath_tab = name.contains(" / ");
for p in multi { if toolpath_tab || !may_wrap(p) { failures.push(... "a {}-row text block" ...) } }
// overflow > 0.5 past clip rect; excess_width > 0.5; rows > cap
```
- Line caps are the measured after-state plus 3: Pocket `[14,27,21,25,22,13]`, 3D Rough `[9,23,20,25,22,13]`, 3D Finish `[11,21,23,28,22,15]`.
- Separate tests pin `"Refused"` plus a single-row face (`a_generate_refusal_takes_one_line_g_e`), the rest-refusal face with `⟳`, and the validation error at one line.

What it would not catch:
- **The wrap sites in §3:** the fixture always has a tool, a clean selection, no Apply refusal, no reach map and no Selection boundary, so those sites never render.
- **Truncated text:** a 250-char note shown as `"full colour at one…"` counts as one row, because the galley keeps the full text with rows = 1.
- **Hovers** are never measured.
- **Rail width:** it checks layout but has no line cap.
- **Caps** are after-state plus 3, so they ratchet upward.
- **Layout quality:** label-column alignment, mixed grids, units and duplicate text (the no-tool sentence ×3) are not checked.
- **Hints:** closed by default, so their contents are never counted.

**`tests/tabular_data_reads_one_table_ic2.rs`**

Arms 1 and 2 cover the Feeds tab of one Pocket fixture only:
- After Apply all, the face no longer paints `"DOC ⓘ"` / `"WOC ⓘ"`; with Details open each paints once; and the face shows `"at the recommendation"`.
- Runs starting with `"Aggressiveness 0.85"`, `"Long tool"`, `"Force line:"` or `"Vendor row"` paint only with Details open.

Arm 3 is a source scan:
```rust
let sites = code.matches("DataTable::new(").count() + code.matches("DataRow::").count();
assert!(sites >= min, ...)   // compare.rs 3, why.rs 1, feeds_speeds.rs 2, sim_diagnostics.rs 3
// plus 15 DELETED_RENDERERS names absent; Details::new( ≥2 in compare.rs+feeds_speeds.rs
```
What it would not catch:
- It only requires a minimum count, so raw tables can sit beside the sites: the 7-column vendor LUT `Grid` in `feeds_speeds.rs` passes.
- It does not scan TP, TB, SS, LD, `geometry_tab.rs` or `OPS/*`, so the raw label-and-combo rows on the Cut, Heights and Linking tabs are not checked.
- It renders no tab other than Feeds and no op other than Pocket.
- It does not check label widths, unit placement, or one `Details` per section outside the feeds card.