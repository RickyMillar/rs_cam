# R03 census verification (source-only)

Scope: verify `results/R03/census.md` and the R03 brief's source anchors
against the code. Method: read-only. Every claim below cites a repo-relative
`file:line`. Items marked **HYPOTHESIS** were inferred, not read.

Files read: `crates/rs_cam_core/src/compute/catalog.rs` (15-70, 255-290,
1125-1145, 1350-1420, 1905-2440, 2465-2490), `crates/rs_cam_core/src/feeds/mod.rs`
(770-830), `crates/rs_cam_core/src/feeds/suggest.rs` (658-800, 1407-1416),
`crates/rs_cam_core/src/compute/execute.rs` (1040-1060, 2130-2160, 2232-2250),
`crates/rs_cam_core/src/compute/config.rs` (1464-1600),
`crates/rs_cam_core/src/compute/operation_configs.rs` (453-470, 557-570),
`crates/rs_cam_core/src/compute/tool_config.rs` (29-55, 259-271),
`crates/rs_cam_core/src/tool_library.rs` (1-30, 50-95, 130-215),
`crates/rs_cam_core/src/diagnostics/adapters/from_preconditions.rs` (55-65, 165-190),
`crates/rs_cam_viz/src/ui/toolpath_panel.rs` (210-245, 380-470, 500-525, 640-770),
`crates/rs_cam_viz/src/controller/events/toolpath.rs` (1-180),
`crates/rs_cam_viz/src/controller/events/compute.rs` (34-52, 200-250),
`crates/rs_cam_viz/src/controller/events/model.rs` (77-107),
`crates/rs_cam_viz/src/controller.rs` (50-80, 255-290),
`crates/rs_cam_viz/src/app.rs` (888-945),
`crates/rs_cam_viz/src/ui/properties/mod.rs` (80-150, 295-330, 1660-1690,
2960-3060, 3324-3375, 3571-3660, 3780-3990, 3986-4080, 4240-4300, 4540-4580,
4647-4700, 4750-4815, 4900-5000),
`crates/rs_cam_viz/src/ui/properties/operations/mod.rs` (100-330, 1720-1990),
`crates/rs_cam_viz/src/ui/properties/operations/*.rs` (function anchors),
`crates/rs_cam_viz/src/ui/properties/tool.rs` (1-200, 355-390),
`crates/rs_cam_viz/src/ui/tool_library_modal.rs` (1-30, 329-560),
`crates/rs_cam_viz/src/ui/sim_debug.rs` (3-32).

---

## 1. Census verification

### Registry check (Needs / Tool / Pass role)

Every registry entry sits at `catalog.rs:1916-2426`. I compared the census's
`Needs` column with `spec.geometry`, the `Tool` column with
`tool_constraints`, and the `Pass role` column with `feeds_pass_role`.

**All 24 rows agree with the registry.** No correction is required on
those three columns. Anchors per row:

| Op | `geometry` | `tool_constraints` | `feeds_pass_role` | Line |
|---|---|---|---|---|
| Face | `Stock` | `ANY_TOOL` | `Roughing` | 1919 / 1927 / 1924 |
| Pocket | `Polygons` | `ANY_TOOL` | `Roughing` | 1938 / 1946 / 1943 |
| Profile | `Polygons` | `ANY_TOOL` | `Roughing` | 1957 / 1965 / 1962 |
| Adaptive | `Polygons` | `ANY_TOOL` | `Roughing` | 1976 / 1984 / 1981 |
| VCarve | `Polygons` | `required_kinds: &[CutterKind::VBit]` | `Finish` | 1997 / 2005-2008 / 2002 |
| Rest | `Polygons` | `ANY_TOOL` | `Roughing` | 2019 / 2027 / 2024 |
| Inlay | `Polygons` | `required_kinds: &[CutterKind::VBit]` | `Finish` | 2038 / 2046-2049 / 2043 |
| Zigzag | `Polygons` | `ANY_TOOL` | `Roughing` | 2060 / 2068 / 2065 |
| Trace | `Polygons` | `ANY_TOOL` | `Finish` | 2079 / 2087 / 2084 |
| Drill | `Polygons` | `ANY_TOOL` | `Roughing` | 2099 / 2107 / 2104 |
| Chamfer | `Polygons` | `required_kinds: &[CutterKind::VBit]` | `Finish` | 2119 / 2127-2130 / 2124 |
| DropCutter | `Mesh` | `ANY_TOOL` | `Finish` | 2141 / 2149 / 2146 |
| Adaptive3d | `Mesh` | `ANY_TOOL` | `Roughing` | 2166 / 2174 / 2171 |
| Waterline | `Mesh` | `ANY_TOOL` | `SemiFinish` | 2191 / 2199 / 2196 |
| Pencil | `Mesh` | `ANY_TOOL` | `Finish` | 2210 / 2218 / 2215 |
| Scallop | `Mesh` | `required_kinds: &[CutterKind::Ball, CutterKind::TaperedBall], supports_v_bit: false` | `Finish` | 2229 / 2237-2240 / 2234 |
| UnifiedFinish | `Mesh` | same as Scallop | `Finish` | 2256 / 2264-2267 / 2261 |
| SteepShallow | `Mesh` | `ANY_TOOL` | `Finish` | 2288 / 2296 / 2293 |
| RampFinish | `Mesh` | `ANY_TOOL` | `Finish` | 2307 / 2315 / 2312 |
| SpiralFinish | `Mesh` | `ANY_TOOL` | `Finish` | 2326 / 2334 / 2331 |
| RadialFinish | `Mesh` | `ANY_TOOL` | `Finish` | 2345 / 2353 / 2350 |
| HorizontalFinish | `Mesh` | `ANY_TOOL` | `Finish` | 2364 / 2372 / 2369 |
| ProjectCurve | `Both` | `ANY_TOOL` | `Finish` | 2383 / 2391 / 2388 |
| AlignmentPinDrill | `Stock` | `ANY_TOOL` | `Roughing` | 2406 / 2414 / 2411 |

Labels and descriptions in the census match the `label:` / `description:`
strings at the same entries (checked all 24). The Rest description uses a
Unicode apostrophe (`couldn\u{2019}t`, `catalog.rs:2016`); the census renders
it as `couldn't`. Cosmetic only.

`ALL_2D` (`catalog.rs:259-271`) and `ALL_3D` (`catalog.rs:273-286`) match
the census's two menu lists in content and order. `AlignmentPinDrill` is in
neither list. Confirmed system-only.

### Form anchors

All 24 form anchors in the census are correct. The functions are declared
`pub(in crate::ui::properties) fn draw_<op>_params` at exactly the cited lines:
`boundary_2d.rs:11,63,123,219,298,340,392,458`; `engrave.rs:7,44`;
`drill.rs:89,182`; `surface_3d.rs:18,50,323,344,617,870,962`;
`finishing.rs:10,78,127,165`; `project.rs:8`.

### Corrections and refinements (the only changes I would make)

1. **Tool column legend is incomplete for the ball/tapered rows.** The
   census says "ball/tapered = Suggest refuses at add time, op never
   created." That is true for **Flat and V-bit** tools. It is **not** true
   for a **Bull Nose** tool. Two predicates decide the two doors and they
   disagree on Bull:
   - Add-time (Suggest): `feeds/mod.rs:822-841` `validate_tool_for_operation`
     accepts `ToolGeometryHint::Ball | Bull { .. } | TaperedBall { .. }` and
     refuses only `Flat | VBit`.
   - Generate-time: `execute.rs:2146-2153` (Scallop) and `execute.rs:2238-2245`
     (Unified Finish) call `.tool_constraints.allows(cutter_kind)`;
     `catalog.rs:1387-1391` `allows` returns true only for
     `required_kinds.contains(&kind)`, and `required_kinds` is
     `[Ball, TaperedBall]` (`catalog.rs:2238`, `2265`). `ToolType::BullNose`
     maps to `CutterKind::Bull` (`tool_config.rs:51`).
   So a Bull Nose first tool passes the add door, the op is created, and
   Generate fails with `Scallop requires a ball-tip tool (Ball Nose or
   Tapered Ball Nose)` (`execute.rs:2152`). The static validator has no
   Scallop / Unified Finish arm (`operations/mod.rs:1840-1902`), so the
   Generate button is **enabled** in that state. Suggested legend text:
   "ball/tapered = Suggest refuses Flat/V-bit at add time; Bull Nose passes
   add and is refused at Generate with an ERR chip."

2. **`3D Finish` (DropCutter) Tool column reads `any` — correct at add time,
   conditional afterwards.** The same feeds validator also refuses
   `OperationFamily::Parallel` when `target_scallop_mm.is_some()`
   (`feeds/mod.rs:823-824`). DropCutter's hint is `cfg.scallop_height`
   (`catalog.rs:2481-2483`), which defaults to `None`
   (`operation_configs.rs:570`). So the add succeeds with a flat tool, but
   if the user later sets a scallop height on a flat-tool 3D Finish the
   Feeds tab shows `Feeds unavailable: scallop requires curved tip …`
   (`properties/mod.rs:1668-1677`) and hides the recipe. Generation itself is
   not refused (registry is `ANY_TOOL`). Worth a footnote on the row.

3. **Rest row "Tool" cell** ("any + previous tool larger + earlier op") is
   correct but under-specified: the earlier op must be **enabled**, in the
   **same setup**, use the **previous tool** and the **same model**
   (`operations/mod.rs:1976`, message at 1891-1893). The default
   `prev_tool_id` is `None` (`operation_configs.rs:468`), so a fresh Rest op
   always starts validation-blocked with `Previous tool not selected`
   (`operations/mod.rs:1879`).

4. **Census line 9-11 (menu hover text)** is verified verbatim:
   `toolpath_panel.rs:698-707`. Note the greyed hover is the description plus
   a newline plus the reason; the enabled hover is the description alone
   (`toolpath_panel.rs:692`).

5. **Census observation "Nothing in the menu says which tool an op needs"**
   is confirmed: `add_op_menu_item` (`toolpath_panel.rs:671-710`) reads
   `spec.geometry` only and never reads `spec` tool constraints or the tool
   list. `add_toolpath_menu` computes only `has_mesh` / `has_polygons`
   (`toolpath_panel.rs:655-656`).

---

## 2. Add path

### Menu (`crates/rs_cam_viz/src/ui/toolpath_panel.rs`)

```rust
// toolpath_panel.rs:655-668
let has_mesh = state.session.models().iter().any(|m| m.mesh.is_some());
let has_polygons = state.session.models().iter().any(|m| m.polygons.is_some());

ui.menu_button("+ Add", |ui| {
    ui.label(egui::RichText::new("2.5D (from SVG)").strong());
    for &op in OperationType::ALL_2D { add_op_menu_item(...); }
    ui.separator();
    ui.label(egui::RichText::new("3D (from STL)").strong());
    for &op in OperationType::ALL_3D { add_op_menu_item(...); }
});
```

Click path (`toolpath_panel.rs:689-697`): pushes `AppEvent::Select(Selection::Setup(setup_id))`
then `AppEvent::AddToolpath(op)`. The menu button is never disabled for
"no tools"; that check happens in the handler.

### Handler (`crates/rs_cam_viz/src/controller/events/toolpath.rs`)

Target setup: `toolpath.rs:13-43`. Derived from the current selection
(`Toolpath`, `Setup`, `Fixture`, `KeepOut`), else `.or(Some(0))` (first
setup, line 43).

Tool pick (first tool, no compatibility check):

```rust
// toolpath.rs:45-52
let Some(tool_id) = self.state.session.tools().first().map(|t| t.id.0) else {
    tracing::warn!("Cannot add toolpath: no tools defined");
    self.push_notification(
        "Cannot add toolpath: no tools defined".into(),
        super::super::Severity::Warning,
    );
    return;
};
```

Suggest call and refusal:

```rust
// toolpath.rs:77-107
let (operation, feeds_provenance) = match rs_cam_core::feeds::suggest::suggest_params(
    SuggestParamsInput { op_type, tool, machine, material, workholding, lut, stock_ctx,
                         spindle_strategy: SpindleStrategy::default(),
                         context: SuggestContext::default() },
) {
    Ok(s) => (s.operation, s.provenance),
    Err(e) => {
        // Engine refused the tool × operation combination
        // (e.g. flat endmill on a Scallop op — no tip radius
        // means the scallop-stepover formula is undefined).
        let msg = format!("Cannot add toolpath: {e}");
        tracing::warn!("{msg}");
        self.push_notification(msg, super::super::Severity::Warning);
        return;
    }
};
```

The `{e}` Display text is (`feeds/mod.rs:801-811`):
`scallop requires curved tip (need ball|bull|tapered_ball; got Flat on Scallop)`.
So the toast reads, for example:
`Cannot add toolpath: scallop requires curved tip (need ball|bull|tapered_ball; got Flat on Scallop)`.
Note the message names the **feeds family** (`Scallop`), so a refused
Unified Finish add also says `on Scallop` (its `feeds_family` is
`FeedsOperationFamily::Scallop`, `catalog.rs:2260`). Note also
`TODO(v1.2)` at `toolpath.rs:90-93`: Suggest at add time runs with a
default `SuggestContext` because the model is chosen after the tool.

Geometry check and model pick:

```rust
// toolpath.rs:111-125
if !operation.is_stock_based() && self.state.session.models().is_empty() {
    let msg = format!("Cannot add {} toolpath: import geometry first", operation.label());
    ...
    self.push_notification(msg, super::super::Severity::Warning);
    return;
}
let model_id = self.state.session.models().first().map(|m| m.id).unwrap_or(0);
```

Defaults set on the new config (`toolpath.rs:127-165`): name
`"{label} {n}"` where n is total toolpath count + 1 (line 129-133);
`heights: HeightsConfig::default()` (all `Auto`, line 136);
`boundary` enabled with `BoundarySource::ModelSilhouette` when the op is 3D
and a mesh exists, else default disabled (lines 146-158);
`boundary_inherit: true` (159); `stock_source: StockSource::Fresh` (161).
Runtime entry is created with `ToolpathRuntime::new(tc.operation.default_auto_regen())`
(line 173-176) and the new toolpath is selected (177).

### Notification rendering

Struct and lifetime: `crates/rs_cam_viz/src/controller.rs:62-79`.

```rust
pub fn ttl(&self) -> std::time::Duration {
    match self.severity {
        Severity::Info => Duration::from_secs(4),
        Severity::Warning => Duration::from_secs(6),
        Severity::Error => Duration::from_secs(8),
    }
}
```

All add-path refusals use `Severity::Warning`, so they live **6 seconds**.
`push_notification` is `controller.rs:269-275`; expiry filter
`active_notifications` at 278-280.

Rendering: `crates/rs_cam_viz/src/app.rs:892-933`. An `egui::Area` named
`toast_notifications`, `Order::Foreground`, anchored
`Align2::RIGHT_BOTTOM` with offset `(-12.0, -12.0)`, max width 400 px
(`app.rs:900-904`). Warning colours: background `rgb(80,60,10)`, text
`rgb(255,220,100)` (`app.rs:910-913`). No close button, no click action,
no log panel: once the 6 s pass the message is gone (`gc_notifications`,
`app.rs:894`). `tracing::warn!` also fires, which reaches the terminal only.

---

## 3. Refusal designs

### Design A: engine refusal at Suggest (add time)

Raised in `crates/rs_cam_core/src/feeds/mod.rs:822-841` `validate_tool_for_operation`:

```rust
let scallop_relevant = input.operation == OperationFamily::Scallop
    || (input.operation == OperationFamily::Parallel && input.target_scallop_mm.is_some());
if scallop_relevant {
    match input.tool_geometry {
        ToolGeometryHint::Ball | ToolGeometryHint::Bull { .. } | ToolGeometryHint::TaperedBall { .. } => {}
        ToolGeometryHint::Flat | ToolGeometryHint::VBit { .. } => {
            return Err(FeedsError::WrongToolForOperation { operation, actual_geometry, required: "ball|bull|tapered_ball" });
        }
    }
}
```

Called from `suggest.rs:789` inside `feeds_result_for_operation`, which
`suggest_for_operation` (`suggest.rs:683-691`) and therefore
`suggest_params` (`suggest.rs:658-671`) propagate with `?`.

Ops covered: **Scallop**, **Unified Finish** (both `feeds_family: Scallop`,
`catalog.rs:2233`, `2260`), and **3D Finish only when a scallop height is
set** (`Parallel` family, `catalog.rs:2145`, hint at `catalog.rs:2481-2483`).
Spiral Finish also has `feeds_family: Scallop` (`catalog.rs:2330`) but its
`feeds_hints` arm is not shown in my read; **HYPOTHESIS**: Spiral Finish
with a flat first tool is also refused at add time. A live test should
confirm this.

Consequence: the op is **never created**. The user sees a 6 s toast.

### Design B: static validator (Generate disabled, op exists)

`crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1840-1902`
`validate_toolpath` (and its session twin `validate_toolpath_config`,
1726-1838, used by the controller at `controller/events/compute.rs:216-225`
on every Generate submit).

Rules:

- `No tool selected` (1844).
- Geometry rules via `validate_geometry_selection` (1849; body 1905+):
  `Selected model must provide both 2D geometry and a 3D mesh`,
  `Selected model has no 3D mesh`, `Selected model has no 2D geometry`,
  `Selected model is missing` (mirrored in the config twin at 1748-1781).
- Pocket / Adaptive: `Stepover must be less than tool diameter` (1852-1861).
- **VCarve / Inlay / Chamfer**: `"<Op> requires a V-Bit tool"` when
  `tool.tool_type != ToolType::VBit` (1862-1876).
- Rest: see Design C.

Effect in the inspector (`properties/mod.rs:3619-3627`):

```rust
let validation_errors = validate_toolpath(entry, validation);
let can_generate = !tools.is_empty() && validation_errors.is_empty();
ui.add_enabled(can_generate, egui::Button::new("Generate"))
```

The G key path runs the config twin and, on errors, calls
`fail_toolpath_submit(tp_id, errs.join("; "))` (`compute.rs:220-224`),
which sets `ComputeStatus::Error(msg)` and clears the result
(`compute.rs:34-43`). So a keyboard Generate on a blocked op produces an
**ERR chip whose hover is the joined validator text**, not a toast.

Ops in Design B for the *tool* rule: **VCarve, Inlay, Chamfer** only.
Scallop and Unified Finish have **no** static arm, which is why the Bull
Nose case (section 1, item 1) reaches Generate.

### Design C: Rest — a third design

Rest's rule is enforced in **three independent places** with three
different surfaces:

1. Static validator (Design B surface): `Previous tool not selected`
   (`operations/mod.rs:1879`), `Previous tool must be larger than current tool`
   (1889), and the earlier-enabled-same-setup-same-model rule (1891-1893,
   predicate `has_prior_rest_source` 1952-1978).
2. Core precondition diagnostic (Diagnostics list, `Severity::Blocking`):
   `diagnostics/adapters/from_preconditions.rs:166-184`
   `Rest machining previous tool ({prev:.1} mm) must be larger than current tool ({curr:.1} mm) — there is no leftover material to clean.`
3. Generator refusal: `execute.rs:1046-1048`
   `Previous tool not set for rest machining` when `ctx.prev_tool_radius` is
   `None`.

Plus a queue-card badge that is not a refusal but a state indicator:
`draw_rest_badge` (`toolpath_panel.rs:713-770`): green `dep` when a
same-setup toolpath uses the previous tool and is generated; yellow `dep`
when that dependency `needs_generation()` or is stale (750-757); red
`no dep` when no same-setup toolpath uses the previous tool (759) or when
`prev_tool_id` is `None` (762). Note the badge checks **same setup + same
tool id** only; it does not check `enabled` or same model, so the badge and
the validator can disagree (badge green, Generate disabled) when the
dependency is a disabled op or targets another model. **HYPOTHESIS** on
that disagreement being observable; the code paths differ, the live state
was not exercised.

Design D (runtime refusal, ERR chip): Scallop / Unified Finish
`execute.rs:2146-2153`, `2238-2245`, reached only by a Bull Nose tool (or by
switching a created op's Tool dropdown to a flat or V-bit tool after the
add, since neither the static validator nor the Tool combo re-runs Suggest).

Summary table:

| Design | Where | Ops | User surface | Op exists? |
|---|---|---|---|---|
| A. Suggest refusal | `feeds/mod.rs:822` via `suggest.rs:789` | Scallop, Unified Finish, 3D Finish with scallop set (Spiral: HYPOTHESIS) | 6 s bottom-right toast | No |
| B. Static validator | `operations/mod.rs:1840` | VCarve, Inlay, Chamfer (+ stepover, geometry) | Generate disabled; red list above tabs; ERR chip if G pressed | Yes |
| C. Rest triple | validator 1877-1895; diag 166-184; `execute.rs:1048` | Rest | Generate disabled + Blocking diagnostic + `dep`/`no dep` badge | Yes |
| D. Generator refusal | `execute.rs:2146`, `2238` | Scallop, Unified Finish with Bull Nose or post-add tool switch | ERR chip on queue card and inspector | Yes |

---

## 4. Inspector structure (`crates/rs_cam_viz/src/ui/properties/mod.rs`)

### Above the tabs (always visible), `draw_toolpath_panel` from 3571

1. `Name:` text edit (3608-3611).
2. `Generate` button (enabled per section 3) and status word: `Ready`
   (Pending), `Computing...`, green `Done`, amber `Waiting on upstream stock`
   with the blocking op on hover, grey `Disabled`, or the error text
   (3619-3660).
3. Diagnostic rows: actionable (yellow tier), stateful (neutral), and a
   collapsed `Hints (n)` header (3812-3852). The `MAN` rule is synthesised
   here:

```rust
// properties/mod.rs:3790-3805
if !entry.auto_regen && matches!(entry.status, ComputeStatus::Pending) {
    diagnostics.push(Diagnostic { id: "workflow.needs_generation", category: State, severity: Info,
        message: "This operation requires manual generation. Press G or click Generate." ... });
}
```

   `entry.auto_regen` is seeded from `OperationSpec::default_auto_regen`
   (`toolpath.rs:175`). Every 2.5D entry has `default_auto_regen: true`,
   every 3D entry has `false` (`catalog.rs:1921` … `2385`; e.g. DropCutter
   2144, Adaptive3d 2169). So the notice appears on every fresh 3D op and
   never on a 2.5D op.

4. `Geometry` collapsing header (3856-3959), default-open while
   `entry.result.is_none()`:
   - `Tool:` combo (3860-3873). Label text is `ToolConfig::summary()`
     (`toolpath_panel.rs:418-422` builds the tuple; `tool_config.rs:259-271`
     formats `"{:.2}mm {type label}"`, Bull Nose adds `(r={:.1})`).
     Fallback `(none)`. The combo shows **every** project tool; it does not
     filter or mark incompatible ones.
   - `Input:` combo (3876-3889), label is the model name, fallback `(none)`.
     Shows every model.
   - STEP-only `Face Selection` block: `N face(s) selected` + `Clear`, or
     the placeholder `Pick faces in viewport ↗` (3908-3934).
   - `Use remaining stock` checkbox (hidden for Pencil) with hover text
     (3944-3959).

### Tab bar

Enum `ToolpathTab` at 2967-2973; labels at 2984-2992:
`Geometry`, `Feeds & Speeds`, `Linking`, `Heights`, `Dressup`. Default tab
is `Geometry` (3976). A coloured dot badge can prefix `Feeds & Speeds`,
`Heights` and `Dressup` (3010-3023); `Geometry` and `Linking` never carry a
badge. Heights badge: red if `bottom_z > top_z` or `clearance_z < retract_z`,
yellow if `feed_z < top_z` or `retract_z < feed_z` (3034-3044).

### Tab contents

- **Geometry** (3986-4640): stale-default `Fix (set to …)` banners
  (3993-4033); cached feeds result for the inline lightning pills
  (4041-4058); italic op description from `spec.description` (4060-4065);
  the per-op form (`draw_<op>_params`, 4076+); then `Machining Boundary`
  (`Enable boundary`, `Inherit from stock`, `Source:` combo with `Stock`,
  `Model Silhouette`, `Face Selection`, …; 4240-4300) and the rest-analysis
  block with `Reference:` and `Cell Size:` (4540-4580).
- **Feeds & Speeds** (4647-4753): `Open Feeds & Speeds modal` button,
  `calculate_and_apply_feeds` card, `Show the math` disclosure, vendor
  cutting-data viewer. Engine refusal renders here as
  `Feeds unavailable: {e}` with a manual `SPEED — how fast (manual)` grid
  (1668-1690).
- **Linking** (4755-4759): `draw_linking_params` (entry/exit, move
  optimisation, retract strategy).
- **Heights** (4761-4767): `draw_heights_params` then `draw_height_diagram`.
- **Dressup** (4769-4811): `n/m dressups active`, `Reset to recommended`
  (role-based), `draw_dressup_params`.

### Heights: where values come from

`HeightsConfig` (`config.rs:1561-1567`) holds five `HeightMode`s:
`clearance_z, retract_z, feed_z, top_z, bottom_z`; all default to
`HeightMode::Auto` (1569-1579). `HeightMode` is `Auto | Manual(f64) |
FromReference(ReferenceOffset)` (1535-1542). The panel
(`operations/mod.rs:186-256`) resolves the auto values with the **core**
resolver (`heights.resolve(ctx)`, line 198) so the displayed number is the
one the generator uses. Each row (`draw_height_row`, 105-163) is a
`DragValue` offset in mm, a reference combo labelled `above/below <ref>`
(`Stock Top`, `Stock Bottom`, `Model Top`, `Model Bottom`;
`config.rs:1481-1484`) and a dim hint `= <z> (auto)` while the row is still
`Auto`, or `= <z>` once pinned (149-159). Any edit of the drag value or a
click on a reference pins the row (`commit_height_row`, 147). There is no
"back to auto" control in this code (HYPOTHESIS: none exists; I did not
read `commit_height_row` or `height_row_display` at 1-100). Row tooltips
are at 205-250, e.g. `Bottom:` "Deepest cut depth… Left on auto, the
operation decides its own floor — 3D operations drive it from the model,
and pinning this row overrides that."

### Glossary tooltips

`tooltip_for` (4900-4996) maps field labels to one-line tooltips
(`Stepover`, `Depth`, `Depth/Pass`, `Scallop Height`, `Stock to Leave`,
`Tolerance`, `Max Angle`, `Retract (R)`, …). The list is keyed by exact
label text, so a field whose label is not in this list has no tooltip.

---

## 5. Status vocabulary (`crates/rs_cam_viz/src/ui/toolpath_panel.rs`)

Status chip, `toolpath_panel.rs:398-418`:

| Badge | Meaning (`ComputeStatus`) | Colour | Hover |
|---|---|---|---|
| `PEND` | `Pending` (never generated or stale) | `TEXT_DIM` | none |
| `GEN` | `Computing` | `WARNING` | none |
| `OK` | `Done` | `SUCCESS_BRIGHT` | none |
| `WAIT` | `AwaitingPriorStock(block)` | `WARNING` | `block.message` (names the blocking op) |
| `OFF` | `Disabled` (`enabled: false`) | `TEXT_FAINT` | none |
| `ERR` | `Error(msg)` | `ERROR` | `msg` |

`MAN` (420-429): shown when `!auto_regen`; hover text:
`Manual generation — press G to generate this operation. 3D operations are not auto-regenerated on parameter change.`

Trace badges (`sim_debug.rs:3-19`, drawn by `draw_trace_badge` 21-32):
`SEM` (semantic trace only), `PERF` (performance trace only), `TRACE`
(both), `PART` (partial). No hover text. They render only when the
toolpath's availability differs from the rest of the queue
(`toolpath_panel.rs:431-444`); if every op is identical the badge is hidden.

Rest badge (`toolpath_panel.rs:712-770`): `dep` green (resolved), `dep`
yellow (dependency pending/stale/awaiting), `no dep` red (missing or not
configured). No hover text.

Stats line (500-520): `"{n} moves · {time} · {dist} m"`, hover
`Estimated time ({qualifier}). {caveat}` when a cycle-time basis exists.

Not present: there is no `STALE`, `EMPTY` or `DONE` badge string. A stale
op returns to `PEND`; an empty result is `OK` with `0 moves`
(**HYPOTHESIS** for the zero-move display; I did not read the empty-result
arm).

---

## 6. Tool inspector

### Fields (`crates/rs_cam_viz/src/ui/properties/tool.rs`)

`draw_tool_fields` (115-383): `Name:` (118), `Type:` combo over
`ToolType::ALL` (139-147; labels `End Mill`, `Ball Nose`, `Bull Nose`,
`V-Bit`, `Tapered Ball Nose`, `tool_config.rs:29-33`), then a grid:
`Diameter:` (163), `Cutting Length:` (172), `Flutes:` (182), `Helix:` (192),
type-specific `Corner Radius:` (202/237), `Material:` (213), `Cut Dir:`
(224), `Included Angle:` (247), `Taper Half-Angle:` (257), `Upper shaft ⌀
(taper top):` (284), `Cross-Section Preview:` (297), holder group `Holder
Diameter:` (322), `Shank ⌀ (in collet):` (333), `Shank Length:` (342),
`Stickout:` (351). A type switch normalises geometry and back-fills the new
type's defining fields (148-153).

`Catalog metadata` (366-381) is a collapsing section with two editable
text fields: `Vendor:` and `Product ID:`. Nothing else; no catalog name,
no source path, no "imported from" marker.

### Apply / Revert (`tool.rs:22-63`)

The panel edits a draft clone. When `modified` is true it shows
`● modified` plus `Apply` (hover `Commit these edits to the project tool.`)
and `Revert` (hover `Discard these edits and restore the saved values.`).
When not modified it shows `✓ saved`. Draft plumbing is in
`properties/mod.rs:295-330`.

### Navigation-away auto-commit (`properties/mod.rs:90-101`)

```rust
/// TOO-003 — flush the pending tool draft if the user navigated away from
/// the tool. Edits are pending-until-committed in the panel, but navigating
/// away **auto-commits** them (the spec's accepted fallback) so a stray
/// click elsewhere can't silently drop edits.
fn flush_tool_draft(state: &mut AppState) {
    let Some((tool_id, draft)) = state.history.tool_draft.take() else { return; };
    if matches!(state.selection, Selection::Tool(id) if id == tool_id) {
        state.history.tool_draft = Some((tool_id, draft));
        return;
    }
    commit_tool_draft(state, tool_id, draft);
}
```

`commit_tool_draft` (106-142) is a no-op when `draft == committed`;
otherwise it pushes `UndoAction::ToolChange`, writes the tool, calls
`session.invalidate_tool(tool_id.0)` (all dependent toolpaths go stale),
and marks the project edited. So "Revert" is only available while the tool
stays selected; clicking elsewhere commits silently (undo is available).

### Save to library (`tool.rs:64-112`)

Row `Save to library:` with a catalog-name text box (hint `catalog e.g.
endmills`) and a `Save` button (hover `Add to the catalog, or overwrite the
matching entry if one exists.`). Calls
`rs_cam_core::tool_library::add_or_replace_tool(&trimmed, tool.clone())`
(89) and prints `Saved '<name>' in <path>` or `Updated …` below the row.

Write location (`crates/rs_cam_core/src/tool_library.rs`): `library_dir()`
(56-78) resolves, first match wins, `$RS_CAM_TOOL_DIR`, then
`$XDG_CONFIG_HOME/rs_cam/tools`, then `$HOME/.config/rs_cam/tools`.
`add_or_replace_tool` (204-210) → `add_or_replace_to` (181-201) →
`save_to` (136-150) writes `<dir>/<catalog>.toml` (`path_in`, 88-93) with
`std::fs::create_dir_all` then `std::fs::write`. Replace key is
`dedupe_key` (geometry signature, 359). `save_library` (152-155) is the
whole-catalog variant used by the modal's edit/move/delete paths.

Caution for the live test: with no `RS_CAM_TOOL_DIR` set, `Save` writes
into Ricky's real `~/.config/rs_cam/tools/`. Set `RS_CAM_TOOL_DIR` to a
scratch directory before launching the GUI for task card 6.

### Library import: is the snapshot nature told to the user?

- Module doc says so (`tool_library.rs:5-9`): "the chosen tool is copied
  into the project (snapshot), so a generated toolpath never changes behind
  the operator's back when a catalog file is later edited. Re-import to
  pick up a catalog change." That is developer-facing.
- User-facing text: the modal's `➕ Add to project` button hover reads
  `Copy a snapshot of this tool into the open project.`
  (`tool_library_modal.rs:374-375`). That is the **only** place the word
  "snapshot" reaches the user, and only on hover.
- The `+ Add Tool ▸ From library ▸ <catalog> ▸ "<name> — ⌀d mm"` menu
  (`toolpath_panel.rs:216-245`) has no hover text and no snapshot note.
- After import (`controller/events/model.rs:77-86`) the tool's `id` is
  reset, it is added to the session, and it is selected. No notification is
  pushed and no field on the imported `ToolConfig` records the source
  catalog. The tool inspector therefore looks identical for a hand-made
  tool and a library copy; `Catalog metadata` shows only `Vendor` /
  `Product ID` if the catalog entry carried them.

Conclusion: nothing in the inspector tells the user that a tool is a
detached copy of a catalog entry.

Modal detail (`tool_library_modal.rs:329-442`): heading `<name>` + `in
<catalog>`, preview, read-only grid (`Type`, `Diameter`, `Cutting length`,
`Flutes`, type-specific angle/radius rows, `Shank diameter`, `Material`,
`Cut direction`, optional `Vendor`, `Product ID`; 444-492), then `➕ Add to
project`, `✏ Edit`, `🗑 Delete` (two-step confirm), and `Move to:` combo.
Edit form (500-548) reuses `draw_tool_fields` with `💾 Save` / `Cancel`
and writes back to the catalog through `AppEvent::UpdateLibraryTool`.

---

## 7. Questions only a live test can answer

1. **Bull Nose on Scallop.** With a Bull Nose as the first project tool,
   does `+ Add ▸ Scallop Finish` create the op (Design A passes), and does
   Generate then show `ERR` with `Scallop requires a ball-tip tool…`? Is the
   Generate button enabled beforehand? (Predicted yes / yes / yes.)
2. **Spiral Finish with a flat first tool.** Refused at add time or not?
   (Registry `ANY_TOOL` but `feeds_family: Scallop`; HYPOTHESIS refused.)
3. **Toast visibility.** Is a 6 s bottom-right toast noticed when the user
   is looking at the top-left `+ Add` menu? Does anyone find the reason once
   it has expired?
4. **Tool dropdown after add.** Switching a created Scallop op's `Tool:` to
   an End Mill: no validator arm exists, so does Generate stay enabled and
   fail with `ERR`? Does the Feeds tab show `Feeds unavailable:` first?
5. **Heights "back to auto".** Once a row is pinned, is there any UI to
   return it to `Auto`? (Not found in the lines read; check
   `commit_height_row` / `height_row_display`, `operations/mod.rs:1-100`.)
6. **Rest badge vs validator disagreement.** With the dependency op
   disabled or on another model, does the card show green `dep` while the
   inspector says `Rest machining requires an earlier enabled operation…`?
7. **Empty result display.** What do the `OK` chip and stats line show for
   a zero-move result (e.g. a Pocket whose boundary collapses)? Is there any
   "empty" word anywhere?
8. **MAN badge comprehension.** Does a user understand `MAN` + `PEND` on a
   fresh 3D op as "you must press G", given the notice is an Info row above
   the tabs?
9. **Auto-commit surprise.** Edit a tool, click a toolpath: the edit
   commits and dependents go stale without a message. Does the user notice
   the `PEND` flip? Does Ctrl+Z restore the tool?
10. **Save to library destination.** Does the `Saved '<name>' in <path>`
    line make clear that the write went to the user's config directory, and
    does the live session have `RS_CAM_TOOL_DIR` set to a scratch dir?
11. **Menu grouping when both geometries are loaded.** With SVG and STL both
    imported, are all 23 menu entries enabled, and does `Project Curve`
    default `surface_model_id` sensibly with `models().first()` as the
    model? (Not read; `project.rs:8` form would show.)
12. **Toast overlap.** Two refusals in quick succession stack vertically
    (`app.rs:929`); does the second push the first off-screen at small
    window sizes with `set_max_width(400.0)`?
