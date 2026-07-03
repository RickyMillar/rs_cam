# Reference fidelity for the rest-depth pencil detector (R1 real tool, R2 real stock)

Successor prompt to `planning/pencil_restdepth_detector_prompt.md`. Phases A+B of
that plan are DONE and validated (2026-07-03, uncommitted): `rest_field.rs` +
`PencilDetector::RestDepth` wired end-to-end; offline hillshade sweeps on
terrain.stl pass the bar (MVD dial 0.2→carpet / 1.0→coherent dendritic network /
2.0→deepest-only; REFD dial 2/6/12 → rest volume 1420→4188→17387 mm³); live
wanaka generation works on the rebuilt GUI.

## Why this plan exists (live finding, 2026-07-03)

Live on wanaka idx 8, RestDepth at REFD=6/MVD=0.4 produced dense cuts on the
near-vertical lake walls. Ricky: *"it seems to have got the very vertical walls.
maybe the finish before it got all the rest?"* — correct, and it exposed the two
fidelity gaps in what the detector calls "reference":

1. **R1 — the reference is a nominal straight ball.** `pencil.rs` RestDepth arm
   does `BallEndmill::new(params.reference_tool_diameter, 25.0)` regardless of
   what tool actually ran before. A flat end mill, vbit, or tapered ball leaves
   a completely different rest shape than a ball of the same diameter. We have
   the real tool data (`ToolConfig` + `build_cutter`) — use it.
2. **R2 — the reference ignores what the prior op actually CUT.** wanaka's
   "3D Finish 6" uses the SAME 2mm tapered ball as the pencil op, so the true
   rest after it is ≈ nothing on reachable walls — but no nominal reference
   tool can know which regions the finish swept, skipped, or boundary-clipped.
   Only the machined stock knows. `ExecutionContext.initial_stock:
   Option<&TriDexelStock>` is ALREADY plumbed (FromRemainingStock fix,
   commit 21692ec) and generate_pencil currently ignores it.

Also confirm-and-keep: the pencil side already uses the REAL cutter geometry
(`ctx.tool_def` → `TaperedBallEndmill` profile: hemispherical tip + tangent
cone — not the shank). Known limitation to note in docs, not fix now:
`drop_cutter` models the cutting envelope only; shank/holder rub in deep
narrow valleys is not checked (ToolDefinition has shank/holder dims if we
ever want a reach gate).

## R1 — reference = a real tool from the library

Mirror the existing `RestConfig.prev_tool_id` pattern exactly (grep
`prev_tool_id` for all six touch points; it is the proven template for
"op references another tool").

- `PencilConfig` (`operation_configs.rs`): add
  `#[serde(default, skip_serializing_if = "Option::is_none")]
  pub reference_tool_id: Option<ToolId>` (same type/serde shape as
  `RestConfig.prev_tool_id`, ~line 400). `None` = legacy nominal-diameter
  behaviour, so old project files load unchanged.
- `catalog.rs` PENCIL_PARAMS: optional param — copy the exact type token the
  REST_PARAMS entry uses for `prev_tool_id`.
- **Resolution happens UPSTREAM, not in generate_pencil** (ExecutionContext has
  no tool list). Follow `prev_tool_radius` plumbing but carry the full config:
  - `ExecutionContext` (`execute.rs` ~191): add
    `pub reference_tool_cfg: Option<ToolConfig>` (owned clone; it's small).
    The two context-builder fns at ~1575/~1616 (which already take
    `prev_tool_radius`) gain the parameter.
  - Core path: `session/compute.rs` ~1066 — next to the Rest prev_tool_radius
    resolution, add: if `OperationConfig::Pencil(cfg)` and
    `cfg.reference_tool_id = Some(id)` → `self.tools.iter().find(...)`
    → clone into the context. Missing id → `warn!` + fall back to None
    (nominal diameter), never an error.
  - GUI path: `rs_cam_viz/src/compute/worker.rs` request struct +
    `controller/events/compute.rs` (same place Rest's prev tool resolves) +
    `worker/execute/mod.rs` pass-through (~139) + its test initializers.
- `PencilParams` (`pencil.rs`): add
  `pub reference_cutter: Option<ToolDefinition>` (ToolDefinition implements
  MillingCutter — `tool/mod.rs:481` — so it drops directly). ~10 test literals
  get `reference_cutter: None` (pencil.rs ×7, param_sweep.rs,
  capability_link_moves_safety.rs; grep `curvature_smoothing:` for the sites).
- `execute.rs` generate_pencil: `reference_cutter:
  ctx.reference_tool_cfg.as_ref().map(crate::compute::cutter::build_cutter)`.
- `pencil.rs`: unify reference construction in ONE helper used by ALL THREE
  detector arms (today the Dihedral/Curvature gate builds its own ball at
  ~line 1213 and the RestDepth arm builds another at ~1297):
  1. `params.reference_cutter` if Some → use it (real geometry).
  2. else `reference_tool_diameter > cutter.diameter()` → nominal ball
     (today's behaviour).
  3. else → 0.1mm bare-surface probe + `reference_is_surface_probe = true`
     (RestDepth) / self-referenced gap (gates).
  NOTE the probe-mode sign flip in `rest_field.rs` (`RestFieldParams::
  reference_is_surface_probe` doc) — with a real reference tool, "same tool as
  pencil" must yield rest ≡ 0 (correct: nothing to clean), NOT probe mode.
  Only fall to probe mode when there is neither a real tool nor a bigger
  nominal diameter.
- GUI (`surface_3d.rs` `draw_pencil_params`): "Reference:" combo — first entry
  "Nominal Ø" (shows the existing Reference Tool Ø DragValue), then every
  library tool by name. Copy the prev-tool picker combo from
  `boundary_2d.rs`. Show for ALL detectors (the dihedral/curvature rest gate
  benefits identically).
- MCP: serde round-trip via set_toolpath_param handles the new field; verify
  with one live set/get.

## R2 — reference = the actual machined stock (the true answer)

Semantics: **when `detector = rest_depth` AND `ctx.initial_stock` is Some
(i.e. the toolpath's stock source is FromRemainingStock and a prior sim
exists), the reference is the stock — no new param.** The existing per-
toolpath Stock Source UI + the controller fallback warning (21692ec) already
express user intent. Tool/nominal reference (R1) remains the path when stock
isn't available. `info!` which reference mode ran; put it in the debug-trace
counters too (`rest_reference_mode`: 0=nominal, 1=tool, 2=stock).

- `rest_field.rs`: give `detect_rest_valleys` a reference source enum:
  `enum RestReference<'a> { Cutter { tool: &'a dyn MillingCutter, is_surface_probe: bool }, Stock(&'a TriDexelStock) }`
  (replaces the `reference: &dyn MillingCutter` arg +
  `reference_is_surface_probe` param field; update the 9 unit tests + harness).
  In Stock mode the per-cell computation is:
  `rest(x,y) = stock_top_z(x,y) − drop_z(pencil, x, y)`, clamped ≥ 0
  - stock query: `stock.z_grid.world_to_cell(x, y)` → `top_z_at(row, col)`
    (`dexel.rs:419/461`). No material in the column OR pencil non-contact ⇒
    invalid cell (same invalid handling as today).
  - This is better than any tool drop because it bakes in the prior TOOLPATH
    pattern (scallop cusps, skipped boundaries, walls the finish never
    visited), not just the prior tool's shape.
- **Frame check (the F-024 lesson):** the stock z_grid and the mesh the pencil
  drops against must be in the SAME frame. Identity setups are world-frame
  (per the F-024 fix in `session/compute.rs`); assert overlap of
  `stock.stock_bbox` and `mesh.bbox` XY and `warn!` + fall back to R1 if they
  don't intersect — a silent frame mismatch would read garbage rest
  everywhere.
- Grid-resolution note: stock cell size ≠ rest-field cell size; nearest-cell
  lookup is fine at v1 (stock cells are typically ≤ rest cells). Do NOT
  bilinear-interpolate dexel tops across steep walls (smears the cliff edge).
- Erosion note: in Stock mode keep the existing contact-based boundary erosion
  (it guards the PENCIL drop's edge overhang, which is unchanged); the
  reference side no longer needs tool-radius erosion since there is no
  reference ball to overhang. The existing `erode_cells` formula uses
  `pencil.radius().max(reference.radius())` — in Stock mode use just
  `pencil.radius()`.
- Validation (live wanaka, the scenario that started this):
  1. idx 8 stock source = FromRemainingStock, prior = idx 7 "3D Finish 6"
     (2mm tapered ball, same as pencil): run_simulation through idx 7 →
     regenerate idx 8 → expect NEAR-EMPTY pencil (finish already got it) —
     this is the honest confirmation of Ricky's hypothesis.
  2. Then the multi-tool premise: disable idx 7 / prior = the 6mm rough or a
     6mm-ball finish → expect walls+valleys similar to today's REFD=6 render,
     minus whatever the rough actually cleared.
  3. Hillshade harness: add `RS_CAM_REST_STOCKREF=1` mode that builds a stock
     by stamping a synthetic prior pass (or loads a checkpoint) — optional;
     live validation may be sufficient, judge at implementation time.

## R3 — rest of original Phase D (unchanged, separate approval)

Wide `ClearingRegion`s → adaptive3d FromRemainingStock boundary polygons
(`set_boundary_config` / `ExecutionContext.boundary`); post-sim residual
rest volume Σ max(0, rest_after) as a per-toolpath metric (model on the
`drill_summaries` slot pattern on `SimulationCutTrace`); then RE-RUN the
2026-06-24 multi-tool A/B (its "rest does not pay" verdict is INVALID —
measured while FromRemainingStock was dead code).

## Order + gates

- **R0 (first): commit the uncommitted Phase A+B work** (rest_field.rs + wiring
  + the crest_lines clippy fix), with user approval — R1/R2 build on it.
- **R1** gate: clippy clean, per-crate tests green, live wanaka set/get of
  `reference_tool_id` via MCP works, GUI combo renders.
- **R2** gate: live wanaka scenario 1 above (same-tool prior → near-empty
  pencil) + scenario 2 sanity; Ricky judges in the 3D view.
- **R3**: separate approval after R2 validates.

## Constraints (durable — same as predecessor plan)

- ONE cargo job at a time (`free -g` + `pgrep -af "[c]argo (check|test|build)"`
  first); yield to user GUI release builds. Concurrent heavy cargo has crashed
  this machine three times.
- Zero-warning clippy (16 deny lints); per-crate tests only, never
  workspace-wide `cargo test`.
- `cargo fmt` cascades into `strategy_advisor*.rs` — revert those two files
  before commit (it happened again today; `git checkout --` them).
- Commit ONLY when the user asks. Co-Authored-By per house rules. wanaka200
  live params are LIVE-ONLY — never save the project from the GUI/MCP.
- GUI trap #1: after a rebuild the running GUI is old until restarted;
  `get_toolpath_params` param_schema is the ground truth (rest_cell_mm present
  ⇒ new binary). Trap #2: `set_ui_view workspace=setup` BEFORE
  set_toolpath_param on a pencil op, or the visible panel clobbers it.
- Live session state right now: GUI runs the NEW binary; idx 8 params were
  changed live (detector=rest_depth, min_valley_depth=0.4, min_cut_length=2.0)
  and NOT saved — a project reload restores the on-disk values.

## Current uncommitted state (R0 inherits this)

Working tree on `experiment/adaptive-spiral`:
- `crates/rs_cam_core/src/rest_field.rs` (new): detector #4 module.
- `pencil.rs` / `operation_configs.rs` / `catalog.rs` / `execute.rs` /
  `lib.rs`: RestDepth variant + rest_cell_mm/route_width_factor wiring.
- `crest_lines.rs`: test-mod print allows (pre-existing clippy gate fix).
- `param_sweep.rs` / `capability_link_moves_safety.rs`: literal updates.
- `rs_cam_viz/.../surface_3d.rs`: detector combo + RestDepth controls.
- `planning/pencil_reference_fidelity_prompt.md` (this file).
Validation renders: `~/Downloads/pencil_restfield/` (offline sweeps + live
shots); scratchpad copies exist for this session only.
Suggested commits: (1) core rest_field + wiring + tests, (2) GUI panel,
(3) planning doc. Ask before committing.
