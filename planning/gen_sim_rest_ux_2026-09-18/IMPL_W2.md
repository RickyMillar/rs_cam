# W2 implementation brief: one rest path in the inspector

Package W2 of `PLAN.md` (§4.3, §5 D5, §6). Written 2026-09-18 from a read-only pass.
Ruling R4 is assumed YES.

Owner set: `ui/properties/{tab_badges,toolpath_panel,mod}.rs`,
`ui/properties/operations/{surface_3d,registry}.rs`, `ui/overlays/{registry,panel}.rs`,
one new component, one new sentry. Out of the set: `ui/toolpath_panel.rs` (W3),
`controller/events/*` (W1), `planning/ui_premium_2026-09-13/` (the other account).

## 0. Five gaps in the plan. Read this first

1. **Rest Analysis has four controls, not three.** The `Reference` combo
   (`toolpath_panel.rs:928-961`) writes `rest_analysis.reference_tool_id`, which
   generation reads (`session/compute/generation.rs:330`). Keep it, inside the
   disclosure, gated like the pencil picker: visible only under "Stock", because
   `resolve_rest_reference` (`compute/execute/dressup_apply.rs:51-56`) prefers the
   machined stock whenever it overlaps, so under "After previous ops" the combo is
   inert.
2. **"Shown only under Stock" is wrong for two pencil detectors.** Only `rest_depth_arm`
   reads `initial_stock` (`finish/pencil/detectors.rs:223`). `dihedral_arm` (`:371-382`)
   and `curvature_arm` (`:133-144`) always call `resolve_reference_cutter`. The gate is
   `detector != RestDepth || stock_source == Fresh` (§2.2).
3. **The overlap fallback is not an `EdgeState`.** Plan §4.1 defines `Broken` as a
   static config read; the fallback (`detectors.rs:223-244`) is known only at
   generation. PLAN.md §6 says W2 does not depend on W0, and that holds: ship the row
   with no hover if W0 has not landed, and add the hover later (§2.3).
4. **Turning the heatmap overlay ON is not a reachable demand.** The row refuses ON
   without a grid (`ui/overlays/registry.rs:824-836`), so the only route is the disabled
   row's action button (§3.4).
5. **Nothing runs the producer hook on load.** `project_file.rs:826-833` copies
   `boundary` and `rest_analysis` verbatim. The draw-time flip was the only cover (§4).

## 1. The "Start from" row

### 1.1 No kit component renders a two-way choice

`rg "selectable_label|SegmentedControl|radio|Toggle"` over `ui/components/` returns
nothing; `ui/components/mod.rs:44-54` lists the whole kit. Every two-way choice in the
app is a raw widget: the generic checkbox (`tab_badges.rs:570`), the pencil pair
(`surface_3d.rs:552-574`). Both break the kit rule. **Add `ui/components/choice_row.rs`
with `ChoiceRow`,** following `value_row.rs`: a leaf widget, `impl egui::Widget`, no
state, tokens only, sized for a `param_grid` cell. `response.changed()` is true on a
real move. `T` needs `PartialEq + Copy` only: `StockSource` is `Copy`, and
`state/toolpath/entry.rs` passes it by value everywhere.

```rust
pub struct ChoiceRow<'a, T: PartialEq + Copy> {
    label: &'a str,
    value: &'a mut T,
    options: &'a [(T, &'a str, &'a str)], // value, label, hover
}
```

Do **not** add it to `covered` in `component_contracts_up2.rs`: that list is pinned to
`SPEC_COMPONENTS = 12` against `DESIGN_SPEC.md` §4, which the other account owns. Ask
for the spec row in the commit body; pin the component in the sentry instead (§7.2 arm
5).

### 1.2 Placement and removal hunks

In `draw_geometry_wiring` (`tab_badges.rs:466-585`), replace the whole `if
!matches!(entry.operation, OperationConfig::Pencil(_))` block (`:555-584`) with an
unconditional row in the same place:

```rust
ui.add_space(8.0);
let resp = ui.add(ChoiceRow::new("Start from", &mut entry.stock_source, &[
    (StockSource::Fresh, "Stock", HOVER_FRESH),
    (StockSource::FromRemainingStock, "After previous ops", HOVER_PRIOR),
]));
if resp.changed() { entry.stale_since = Some(std::time::Instant::now()); }
```

`HOVER_FRESH` and `HOVER_PRIOR` are the two hover strings listed in §5.2.

| File | Lines | Action |
|---|---|---|
| `tab_badges.rs` | 555-584 | Delete the pencil guard, the checkbox, the comment. The row goes here. |
| `tab_badges.rs` | 450-456 | The giant-region caption cites "(Use remaining stock)". Reword to "(Start from: After previous ops)". |
| `surface_3d.rs` | 538-576 | Delete `Rest reference:`, its hover, both `selectable_label` calls. |
| `surface_3d.rs` | 577-592 | Delete the fallback caption (§2.3). |
| `surface_3d.rs` | 373-388 | Delete the comment describing the deleted pair. |
| `toolpath_panel.rs` | 607-609, 630-632 | See §1.4. |

### 1.3 How the write reaches core

**Keep `ReplaceToolpathConfig`. Do not call `Command::SetStockSource` from the panel.**
`project_entry_onto` copies `entry.stock_source` (`mod.rs:667`) and
`write_entry_config_to_session` (`mod.rs:700-716`) applies `ReplaceToolpathConfig` on
**every** open frame, so a `SetStockSource` call from draw would be a second write of
one field in one frame. `generation_inputs_signature` (`session/mod.rs:942-953`) already
contains `self.stock_source`, so the command drops the result and the downstream chain
when the field moves; the invalidation is not weaker than `set_stock_source`'s. The "one
door" rule holds: `ReplaceToolpathConfig` is the panel's door, `SetStockSource` is the
MCP tool's (`mcp_server.rs:1206`, `app/mcp/commands.rs:616-623`), and both end in
`invalidate_result_chain`.

### 1.4 `OpDrawCtx`

`stock_source` and `stock_source_changed` (`registry.rs:68-73`) exist so pencil can own
the field. After §2 pencil no longer writes it. Drop `stock_source_changed` from
`OpDrawCtx`, drop `stale` from `draw_pencil_params`, delete `toolpath_panel.rs:630-632`,
and delete the now-false comment at `:607-609`. Keep `stock_source` as a read-only
`StockSource`: pencil reads it to gate its picker, and UnifiedFinish already reads it by
value (`registry.rs:281`, `surface_3d.rs:752`). The signature break has two call sites
(`registry.rs:267-270`, `:281`) plus the builder at `toolpath_panel.rs:618-628`.

## 2. Pencil

### 2.1 What `stock_source` gates

`GenContext::generator_seed_stock` (`session/compute.rs:222-227`) hands `prior_stock` to
the generator only under `FromRemainingStock`. It arrives in pencil as `initial_stock`.

| Detector | Reads `initial_stock`? | Reference used |
|---|---|---|
| `RestDepth` | yes, `detectors.rs:223` | machined stock when the XY bboxes overlap, else `resolve_reference_cutter` |
| `Dihedral` | no, `:371-382` | always `resolve_reference_cutter` |
| `Curvature` | no, `:133-144` | always `resolve_reference_cutter` |

`resolve_reference_cutter` reads `PencilConfig::reference_tool_id` (resolved at
`session/compute/generation.rs:316-328`) and falls back to a nominal ball at
`reference_tool_diameter`. **The operator loses nothing when the pair goes:** the pair
wrote the same `stock_source` the generic row writes, in other words.

### 2.2 The picker gate

```rust
let reference_is_live = cfg.detector != PencilDetector::RestDepth
    || *stock_source == StockSource::Fresh;
```

Draw `Reference:` and `Reference Tool Ø:` when `reference_is_live`. Gate on the detector
as well as the source: the plan's words, applied literally, hide a live control on two
of three detectors.

### 2.3 The overlap fallback

Today it is a caption (`surface_3d.rs:578-592`). The plan makes it `EdgeState::Broken`
plus a hover, which W0's static function cannot answer (§0 item 3).
`rest_reference_mode` (0 nominal, 1 tool, 2 stock) exists only as a tracing field and a
debug counter (`detectors.rs:250,312,323`); it never reaches `ToolpathStats` the way
`claims_reference` does (rendered at `surface_3d.rs:832-860`). So W2 ships one read from
W0, `edge_state_for(session, id, EdgeKind::Stock)`, hung on the "Start from" row as a
hover, with no caption and no colour word; if W0 has not landed, ship the row with no
hover. Promoting `rest_reference_mode` onto `ToolpathStats`, so the panel can print what
generation resolved, is a core follow-up for the commit body. Delete the caption either
way: it is prose repeating a state the row carries.

## 3. Rest Analysis becomes a demand-titled disclosure

### 3.1 Consumers and the overlay input

`ProjectSession::rest_region_consumers` (`session/mod.rs:1612-1625`) already answers the
consumer question, mapped to names at `ui/properties/mod.rs:417-424` and carried as
`ToolpathPanelInputs::rest_region_consumers` (`mod.rs:321`). No new core function.
`ToolpathPanelInputs` gains `pub rest_heatmap_on: bool` (the overlay is on **and** this
toolpath is selected). `toolpath_panel_inputs` (`mod.rs:351-357`) takes `session` and
`gui` only, so add a `bool` argument and pass `state.viewport.show_rest_heatmap` from
the call site at `mod.rs:1199`, where `state` is in hand. The properties rule ("add a
field there, not a parameter") governs `draw_toolpath_panel`, which keeps its two
structs.

### 3.2 The new shape

Replace `toolpath_panel.rs:868-999` with:

```rust
let show_rest_dials = !rest_region_consumers.is_empty() || rest_heatmap_on;
if show_rest_dials {
    let title = if rest_region_consumers.is_empty() {
        "Rest regions \u{2192} heatmap".to_owned()
    } else {
        format!("Rest regions \u{2192} {}", rest_region_consumers.join(", "))
    };
    ui.disclosure("rest_regions", &title, false, |ui| {
        // RestDepth pencil: the title line only. Every other op:
        // Reference (only under "Stock"), Cell Size, Min Valley
        // Depth, Region Margin.
    });
}
```

`ui.disclosure` is `UiExt::disclosure` (`ui/components/section.rs:51-63`), so no raw
`CollapsingHeader` enters the inspector. The `is_rest_depth_pencil` test (`:884-895`)
moves inside the closure: that op's dials are `PencilConfig` fields already in the
pencil grid, so its body is a title and nothing else. A RestDepth pencil with the
heatmap on and no consumer therefore draws an empty disclosure. That is the intended
shape, not a defect: the title names the demand, and the dials sit one grid above.
Deleted with the checkbox:
`SectionHeader::new("Rest Analysis")` (`:896`), the checkbox (`:897-907`), the
"Producing rest regions for" label (`:919-925`), the flip (`:911-918`).

### 3.3 The overlay route

In `ui/overlays/registry.rs`, rename `OverlayAction::OpenRestAnalysis` to
`EnableRestAnalysis` (`:127-130`); its label (`:140`) becomes "Compute rest". Reword the
refusal (`:828-832`) and the hover (`:842-847`): both name a "Geometry > Rest Analysis"
that will not exist. Use "this toolpath has no rest grid yet". `apply_overlays` reuses
the refusal, so the MCP reply follows for free. The string at `:924` survives unchanged.

In `ui/overlays/panel.rs:274-281` the `run_action` arm stops navigating:

```rust
OverlayAction::EnableRestAnalysis => {
    if let Selection::Toolpath(id) = state.selection {
        events.push(AppEvent::EnableRestAnalysisFor(id));
        events.push(AppEvent::GenerateToolpath(id));
    }
}
```

`AppEvent::GenerateToolpath(ToolpathId)` already exists (`:144`). The new
`EnableRestAnalysisFor` arm must apply `Command::AutoEnableRestAnalysis` with the
`stamp_stale` plus `mark_edited` plus `invalidate_simulation` triplet that
`ui/properties/mod.rs:1291-1305` already carries. Do **not** copy that triplet into
`panel.rs`; a panel does not apply commands. **Lift it into `pub(crate) fn
apply_auto_enable(state, source_id)` in `ui/properties/mod.rs`** and call it from both
the boundary hook and the new event arm. `mod.rs` is in W2's owner set, so the package
stays self-contained and the triplet keeps one home.

### 3.4 A legacy project with `enabled == true` and no demand

The analysis costs a rest-field solve at generation (`compute/execute.rs:491-507`), so a
stale `true` is real cost with no reader. **Recommendation, under the operator ruling of
2026-09-16: normalise at load, in core**, as a post-pass over the built session (the
rule asks a cross-toolpath question, so it cannot sit inside `to_toolpath_config`):

```
for each toolpath tc:
    if tc is a RestDepth pencil: leave it alone
    else: tc.rest_analysis.enabled = !rest_region_consumers(tc.id).is_empty()
```

Emit a `ProjectLoadWarning` per change, shaped like `DressupsNormalized`
(`project_file.rs:805-816`). That pass is core work outside W2's owner set; recommend W0
takes it. **Do not put the rule in the disclosure.** A draw that writes a config field
is exactly the defect D5 names.

## 4. D5: the draw-time flip

Delete `toolpath_panel.rs:911-918`, the `if !entry.rest_analysis.enabled` block that
sets the flag and stamps `stale_since`.

| Case | Cover after W2 |
|---|---|
| GUI boundary combo picks "Rest Regions" | `ui/properties/mod.rs:1264-1305` applies `Command::AutoEnableRestAnalysis`. |
| MCP `set_boundary_config` | `session/mutation/config.rs:198-224` calls the hook inside the setter. |
| Heatmap wanted, no consumer | The overlay's "Compute rest" action (§3.3). |
| **Project file whose consumer predates this session** | **Not covered.** |

`to_toolpath_config` (`project_file.rs:818-841`) copies `boundary` and `rest_analysis`
verbatim, and no caller runs `set_boundary_config` on load. So removing the flip
regresses one scenario: an old project whose consumer points at a producer with `enabled
== false` refuses at generation (`session/compute/generation.rs:565-582`). That refusal
is loud and names the source, which beats a silent draw-time write, but it is a
regression. The §3.4 load pass is the fix: it turns the flag **on** for every consumer
as well as off for every orphan. Land it with W0; if W2 lands first, state the gap in
the commit body and open a ledger row.

## 5. Strings

### 5.1 Before: four Geometry-tab rest routes

| Route | Sites |
|---|---|
| 1. checkbox "Use remaining stock" | `tab_badges.rs:570`, cited again in the caption at `:454` |
| 2. boundary source "Rest Regions" + "Rest source:" combo | `toolpath_panel.rs:712-723, 749-787` |
| 3. section "Rest Analysis" + checkbox "Compute rest heatmap (material left after this op)" + label "Producing rest regions for: ..." + four dial hovers | `toolpath_panel.rs:896-997` |
| 4. pencil "Rest reference:" + "Machined stock (requires simulation)" + fallback caption | `surface_3d.rs:548-592` |

Not rest routes, untouched: the pencil detector option "Rest depth (recommended)"
(`surface_3d.rs:416`), "Rest Cell:" (`:437`), the UnifiedFinish "Rest Claims" block
(`:754-830`), the Rest 2D prev-tool combo (`boundary_2d.rs:425`).

### 5.2 After: two routes, plus the new strings

Route 2 stays as the one consumer declaration; the disclosure "Rest regions >
<consumers>" is the second, drawn on demand only. Routes 1, 3 and 4 go, and the caption
at `tab_badges.rs:454` is reworded.

- Row label `Start from`.
- `Stock`, hover: `Cut the full stock block. Prior operations in this setup are
  ignored.`
- `After previous ops`, hover: `Cut what prior operations in this setup leave. The
  system simulates them first.`
- Disclosure hover: `The rest field this operation leaves, and the regions the
  operations above take as their boundary.`
- Overlay action label: `Compute rest`.

No caption survives. Every hover is one or two short sentences.

## 6. Existing tests that pin the current controls

`rg "Use remaining stock|Machined stock|Compute rest heatmap|Rest
Analysis|rest_analysis.enabled"` over `rs_cam_viz/src`, `rs_cam_viz/tests` and
`rs_cam_core/tests` finds **no test that asserts a control label**:

| Site | What it does | Move |
|---|---|---|
| `viz/src/compute/worker/tests.rs:916,922` | sets the field, asserts regions attach | none; never draws |
| `viz/src/controller/tests/freshness.rs:200` | flips the field to check invalidation | none |
| `viz/src/controller/tests/rest_dependency.rs:151` | asserts the hook enabled the source | none; it is the net for §4 |
| `core/src/session/mutation/tests.rs:437-479` | `rest_region_consumers` | none |
| `viz/tests/the_inspector_nests_once_dc5.rs:176` | `rfind("draw_geometry_wiring(")` | none; keep that call and its name |
| `viz/tests/overlays_registry.rs` | registry completeness | re-run after the `OverlayAction` rename; it reads the enum, not the label |

`ui_string_hygiene.rs` scans only for baked indentation, and `component_contracts_up2`
does not fail on a component absent from `covered` (§1.1). **W2 breaks no existing
test. That is the finding: the four rest controls were never pinned.**

## 7. Sentry

### 7.1 Where it lives, and how it drives the panel

The plan names `ui/properties/tests/one_start_from_row_g_startfrom.rs`, which cannot run
as `cargo test -p rs_cam_viz -q --test <name>`, the way every sentry in
`ui/properties/CLAUDE.md` is invoked. Write it at
`crates/rs_cam_viz/tests/one_start_from_row_g_startfrom.rs` and add the row to
`ui/properties/CLAUDE.md`.
`crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs` is the model: it
builds an `AppState` from a `ProjectSessionBuilder` (`:190-260`), then runs
`properties::draw(ui, state, &mut events)` inside `ctx.run_ui(egui::RawInput::default(),
..)`. `ctx()` (`:160-167`) installs `tokens::apply` and `tokens::apply_fonts` and runs
one warmup pass. Harvest painted text from `out.shapes` through
`egui::epaint::Shape::Text(text)` and `text.galley.job.text` (`:305-310`, and
`the_legend_reads_as_words_g_legendwrap.rs:158-168`). Run three passes and read the
third: egui settles layout over two.

### 7.2 The arms

1. **`the_geometry_tab_names_rest_twice_g_startfrom`.** Fixture: a Scallop producer with
   an enabled `DerivedRestRegions` consumer pointing at it. A Scallop draws neither the
   pencil grid nor the UnifiedFinish claims block, so the only rest strings belong to
   the two surviving routes. Collect every painted string whose lowercase holds "rest",
   dedupe, assert the set **equals** a two-entry allow-list. Equality, not `<= 2`: a
   `<=` arm passes when the disclosure vanishes.
2. **`a_draw_never_writes_rest_analysis_g_startfrom`.** The red-first arm for D5. Same
   fixture, producer's `rest_analysis.enabled` set to `false` by hand. Snapshot it, run
   one `properties::draw` frame, assert it is still `false`. Fails today at
   `toolpath_panel.rs:911-918`.
3. **`pencil_hides_its_reference_under_prior_stock_g_startfrom`.** Pencil, `RestDepth`,
   `FromRemainingStock`: assert neither "Reference:" nor "Nominal Ø" paints, and
   "Machined stock (requires simulation)" paints nowhere. Flip to `Fresh`, redraw,
   assert "Reference:" paints; the flip makes the arm non-vacuous.
4. **`a_crease_detector_keeps_its_reference_g_startfrom`.** Pencil, `Dihedral`,
   `FromRemainingStock`: assert "Reference:" **is** painted. This is §0 item 2; without
   it the obvious implementation hides a live control.
5. **`the_start_from_row_is_the_kit_row_g_startfrom`.** Render `ChoiceRow` alone:
   `changed()` fires on a click, both option labels paint. This replaces the
   `component_contracts_up2` row W2 cannot add.
6. **Non-vacuity.** Arm 1's painted set contains "Start from", so a fixture that renders
   nothing cannot pass arms 1 and 3.

## 8. Blast radius and risks

`rg -n stock_source`, production files only:

| Area | Sites | Effect |
|---|---|---|
| core generation | `session/compute.rs:161,192,218-227`; `compute/simulate.rs:280-286`; `compute/catalog.rs:1425-1427`; `compute/generated_empty.rs:96-98,289` | none; the meaning is unchanged |
| core invalidation, IO, diagnostics | `session/mutation/config.rs:159-173`; `mutation/toolpath.rs:334`; `session/mod.rs:951`; `project_file.rs:511,826`; `save.rs:235`; `compute/diagnostics.rs:660` | none |
| viz controller | `controller/generate_all.rs:42`; `events/compute.rs:209,280`; `events/simulation.rs:148`; `events/toolpath.rs:250` | none; W1 owns them |
| viz UI | `tab_badges.rs:568-583`; `surface_3d.rs:552-574`; `registry.rs:68-73,267-270,281`; `readiness.rs:153` | two writers become one; `OpDrawCtx` loses the `&mut` |
| viz MCP and state | `mcp_server.rs:1203-1206`; `app/mcp/commands.rs:616-623,2348-2353`; `state/toolpath/entry.rs:35,126,151,234` | none; W5 owns the wire shape |

| Risk | Mitigation |
|---|---|
| Removing D5 regresses the stale-project case | Land the §3.4 core load pass with W0. Until then state the gap and open a ledger row. The failure is a loud refusal naming the source, not wrong geometry. |
| Hiding the pencil reference on a crease detector hides a live control | Sentry arm 4. Gate on detector **and** source. |
| `rest_analysis.reference_tool_id` dropped by accident | It moves into the disclosure under the same "only under Stock" gate. Name it in the commit body. |
| The overlay demand drops the producer's cached result (`auto_enable_rest_analysis_for_source` calls `drop_result`) | The action pushes `GenerateToolpath(id)` straight after, so the operator sees one regenerate, not a silently emptied row. |
| `ChoiceRow` drifts from a kit spec it is not in | Sentry arm 5 pins it; ask the other account for the `DESIGN_SPEC.md` §4 row. |
| `dc5` arm 2 fails open if `draw_geometry_wiring(` is renamed | Keep the name and the call site. |
| W0 lands after W2, so the Stock-edge hover has no source | Ship the row with no hover. The hover is a one-line follow-up in `tab_badges.rs`. |

## 9. Order of work

`choice_row.rs` plus the `mod.rs` export, then the sentry red first (arms 2, 3 and 4
fail on today's code), then `tab_badges.rs` (row in, checkbox out, caption reworded),
`surface_3d.rs` (pair and caption out, picker gated), `registry.rs` plus
`toolpath_panel.rs:607-632` (narrow `OpDrawCtx`), `toolpath_panel.rs:868-999` (the
disclosure, and D5 out), `ui/properties/mod.rs` (the `rest_heatmap_on` input),
`ui/overlays/{registry,panel}.rs` (the demand and the strings), and
`ui/properties/CLAUDE.md` (add the sentry row).

Local loop: the properties sentries, `overlays_registry`, `component_contracts_up2`,
`the_inspector_nests_once_dc5`, `ui_string_hygiene`, then `cargo test -p rs_cam_viz -q`,
then clippy. No core gate; ask before any run over three minutes.
