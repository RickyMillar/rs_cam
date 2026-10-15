# W6.3 — the Height-field shape: implementation plan

Status: PLAN ONLY (read-only research, 2026-10-15, branch `op-arch` at
`a0db1ecb` plus the uncommitted W6.2 work in the tree). No code changed.

Sources read: root and `rs_cam_core` `CLAUDE.md`; folder files for
`compute/`, `finish/`, `ops/`, `surface/`; `planning/op_architecture_2026-10-14/`
`AUDIT.md` and `PLAN.md`; `compute/execute/layered.rs`, `territory.rs`,
`prologue.rs`, `shared.rs`, `finish_raster.rs`, `finish_3d.rs`,
`curve_engrave.rs`; `trace/produced.rs`; the seven kernels.

Note: `AUDIT.md` holds only the merged verdict table. The per-group
"curve/raster" and "3D" agent tables are not in the repo; this plan
re-derives them from the code.

## 0. W6.2 names this plan uses (fill in on the W6.2 commit)

The W6.2 work is in the working tree, not committed. This plan uses the
names as they stand there. Each `[W6.2]` mark below is a name to confirm.

| Name | Where (tree, 2026-10-15) |
|---|---|
| `Territory<'a> { xy: XyTerritory, mask: Option<SurfaceMask>, stock: Option<StockReference> }` `[W6.2]` | `compute/execute/territory.rs:71` |
| `Territory::regions()`, `Territory::reference_stock()` `[W6.2]` | `territory.rs:115`, `:123` |
| `Prepared::territory()` `[W6.2]` | `prologue.rs:205` (file lines 205-207) |
| `SurfaceMask::{SlopeBand, Flat, …}`, `OperationConfig::surface_mask()` `[W6.2]` | `territory.rs:51`, `catalog.rs:848` |
| `Produced` `[W6.2]` | `trace/produced.rs:28` |
| `AnnotatedToolpath::produced` `[W6.2]` | `trace/toolpath_spans.rs:465` |
| `Capabilities::territory_xy`, `TerritoryXy` `[W6.2]` | `catalog/capabilities.rs:79`, `:172` |

If W6.2 renames any of these, change only the names. The design does not
depend on the field layout except: `territory.regions()` gives
`Option<&RegionSet>`. `territory.stock` is filled when the rest rule is
`Reference` OR when the generic rest analysis is enabled
(`territory.rs:100-105`), so it can be `Some` for DropCutter and Scallop.
No height-field op reads it: the link ceiling reads `ctx.initial_stock`
through `op_link_ceiling(ctx)`. Do not build a condition on `territory.stock`
in this shape.

## 1. The load-bearing rules

1. **The HeightQuery is a handle, not a sampled grid.** The seven kernels
   use three query kinds: per-point `point_drop_cutter` (Horizontal,
   Radial, Spiral, Ramp's reach clamp, Scallop's `RingLiftCtx`); a grid at
   a resolution the op picks (`cached_finish_surface` at
   `scallop_generation_resolution` or `ramp_finish_generation_resolution`;
   DropCutter's `batch_drop_cutter_with_cancel` at `cfg.stepover`); and an
   analytic field (VCarve's `EdgeDistanceField`). If the runner builds one
   grid and gives it to every kernel, every fingerprint moves. So the
   runner gives a handle, and each kernel keeps its own sampling and its
   own resolution policy.
2. **Kernel signatures do not change in W6.3. Only the adapters change.**
   `finish/unified_finish.rs:106` imports the scallop kernel;
   `unified_finish.rs` and `benches/perf_suite.rs` call
   `raster_toolpath_from_grid`; SteepShallow and the research seams
   (`*_with_resolution`, `tests/checkpoint_b_resolution_ab.rs`) call the
   kernels directly. W6.4 (composed surface) owns those callers.
3. **The `generate_*` adapter names stay.** The registry rows
   (`catalog/registry.rs`) and `--lib every_generator_polls_a_preset_cancel_flag_past_the_prologue`
   name them. Each adapter becomes "build the op struct, call
   `run_height_field`".
4. **The kernel is not moved.** Each `plan` impl calls the existing kernel
   function. The one exception is DropCutter, whose kernel glue lives in the
   adapter today (§3.5).
5. **No intended output change.** Every step proves "no change" (§6). A
   behaviour change (a rest rule, a centre from the territory, a V-carve
   depth ladder) is a separate item with its own ruling and re-bless.

## 2. The trait and the runner

New file `compute/execute/height_field.rs` (beside `layered.rs`), with a
line in `compute/CLAUDE.md` Files and Invariants.

```rust
/// What a height-field kernel queries. A handle: the kernel samples it
/// itself, at its own resolution (rule 1).
#[derive(Clone, Copy)]
pub(crate) enum HeightQuery<'a> {
    /// The mesh drop cutter: the mesh, its index and the op's cutter.
    /// A kernel may also read the push cutter or the triangle normals
    /// from the same handle (Ramp, Horizontal).
    Mesh { surface: MeshInput<'a>, cutter: &'a ToolDefinition },
    /// An analytic Z field over 2D outlines (V-carve). No mesh.
    Analytic(&'a dyn AnalyticField),
}

/// An analytic height field (V-carve's V-depth field).
pub(crate) trait AnalyticField: Sync {
    /// The outlines the field is defined over, in input order.
    fn outlines(&self) -> &[Polygon2];
}

/// What `plan` gets besides the query and the territory.
pub(crate) struct PlanCtx<'c> {
    pub cancel: &'c dyn CancelCheck,
    pub debug: Option<&'c ToolpathDebugContext>,
    /// The shared finishing link stage, for an op whose link rule is
    /// `LinkRule::InPlan` (Scallop). `None` for every other rule.
    pub link_stage: Option<&'c FinishingLinkStage<'c>>,
}

/// What `plan` returns.
pub(crate) struct Planned<A, R> {
    pub toolpath: Toolpath,
    /// Per-move labels for `SpanRule::LabeledEvents`; empty otherwise.
    pub annotations: Vec<A>,
    /// The kernel's report. The runner records it (W2.3).
    pub report: R,
    /// The link geometry the plan used, for `LinkRule::AfterPlan`.
    /// DropCutter's sampling is its grid step, which only the plan knows.
    pub link_geometry: Option<LinkGeometry>,
    /// `[W6.2]` side channel. Empty for all seven ops (§4).
    pub produced: Produced,
}

pub(crate) enum LinkRule {
    /// No finishing link stage (Horizontal, Radial, Spiral, Ramp, VCarve,
    /// and iso-field Scallop).
    Off,
    /// The runner relinks the planned toolpath (`relink_in_adapter`) when
    /// `hookup_mm > 0`, and records the totals only when it ran
    /// (G-LINKVISIBLE). DropCutter.
    AfterPlan { hookup_mm: f64 },
    /// The runner builds the stage and gives it to the kernel, which links
    /// inside, because its index-carrying ring annotations are reconciled
    /// beside the relink. Offset-cascade Scallop. ESCAPE HATCH (§5).
    InPlan { hookup_mm: f64 },
}

pub(crate) enum SpanRule {
    /// `generated_with_cut_run_spans(label)` + `with_depth_run_annotation`.
    /// DropCutter "Raster row", Radial "Radial ray", Horizontal
    /// "Horizontal slice", VCarve "V-carve run".
    CutRuns(&'static str),
    /// `spans_from_labeled_events` over `Planned::annotations` + the op's
    /// `annotate` hook. NO depth-run annotation (today's behaviour).
    /// Scallop, Spiral, Ramp.
    LabeledEvents,
}

pub(crate) trait HeightFieldOp {
    /// `RuntimeLabel` is implemented only for the three `*RuntimeAnnotation`
    /// structs, not for `()`. A `CutRuns` op names a unit type `NoLabel`
    /// with an `impl RuntimeLabel` that is never called; the runner reads
    /// `annotations` only under `SpanRule::LabeledEvents`.
    type Annotation: RuntimeLabel;
    type Report: RecordFindings;        // `()` for none

    fn op_type(&self) -> OperationType;
    /// The query handle. Default: `HeightQuery::Mesh` from
    /// `prepared.mesh()` and `ctx.tool_def`. VCarve overrides.
    fn query<'a>(&'a self, ctx: &ExecutionContext<'a>, prepared: &Prepared<'a>)
        -> Result<HeightQuery<'a>, OperationError>;
    /// An op-specific refusal that the prologue does not cover. Default
    /// `Ok(())`. Radial's N10 range check (§3.3).
    fn refuse(&self) -> Result<(), OperationError> { Ok(()) }
    fn link_rule(&self) -> LinkRule { LinkRule::Off }
    fn span_rule(&self) -> SpanRule;
    /// Default: nothing. Scallop / Spiral / Ramp call their
    /// `annotate_*` function.
    fn annotate(&self, _a: &[Self::Annotation], _tp: &Toolpath,
                _sem: &ToolpathSemanticContext) {}
    fn plan(&self, query: HeightQuery<'_>, territory: &Territory<'_>,
            pctx: &PlanCtx<'_>)
        -> Result<Planned<Self::Annotation, Self::Report>, OperationError>;
}

/// THE runner of a height-field op.
pub(crate) fn run_height_field<O: HeightFieldOp>(
    op: &O,
    ctx: &ExecutionContext<'_>,
    prepared: &Prepared<'_>,
) -> Result<GeneratedToolpath, OperationError>;
```

The runner, in order:

1. `op.refuse()?`.
2. `let territory = prepared.territory();` `[W6.2]`
3. `let query = op.query(ctx, prepared)?;`
4. Build the stage once: for `InPlan`/`AfterPlan`,
   `finishing_link_stage(op.op_type(), ctx, territory.regions(), hookup_mm)`
   (move this fn and `relink_in_adapter` from `finish_raster.rs:90-162`
   into `height_field.rs` or `shared.rs`; Waterline's `link` hook calls the
   same two functions).
5. `let planned = op.plan(query, &territory, &PlanCtx { cancel: &ctx.cancel_fn(), debug: ctx.debug_ctx, link_stage })?;`
6. `ctx.record(&planned.report);` (unconditional; `()` records nothing,
   `RampReachClamp` must be recorded unconditionally — PR-8b).
7. `AfterPlan` with a stage: `relink_in_adapter(...)`, then
   `ctx.record(&totals)`. No stage: no record (G-LINKVISIBLE).
8. Spans by `span_rule()`. For `LabeledEvents`: call `annotate` BEFORE the
   spans (order of today; neither mutates the path).
9. `generated.produced = planned.produced;` `[W6.2]`

What the runner does NOT do: no cancel poll of its own beyond the
prologue (each kernel polls inside; an extra poll is harmless but adds
nothing), no ladder, no rest step, no boundary clip (the session clips
after generation), no dressup.

Size: the seven adapters are ~330 lines today (`finish_3d.rs:535-703`,
`finish_raster.rs:164-397`, `curve_engrave.rs:116-152`). The runner adds
~150; the adapters shrink to ~170. The gain is consistency (one span
tail, one record tail, one link-stage door), not size. Same verdict as
AUDIT "Size".

## 3. Per op

Columns used in each block: kernel; already in the framework; moves to
the runner in W6.3; stays op-specific; RestUse; hooks; risk.

### 3.1 HorizontalFinish

- **Kernel:** `finish::horizontal_finish::horizontal_finish_toolpath_with_cancel`
  (`finish/horizontal_finish.rs:97`). It classifies flat triangles by
  normal, groups them by Z, rasters each group, and keeps a sample only
  when it is in the regions, contacted and over a flat triangle of the group.
- **Already moved (W2.2/W6.2):** cancel first, mesh + index check, tool
  check (ANY_TOOL), `territory.regions()`.
- **Moves to the runner:** the cut-run span tail ("Horizontal slice") and
  the depth-run annotation (`finish_3d.rs:699-702`).
- **Stays op-specific:** `HorizontalFinishParams` from `cfg.params(OpMotion)`;
  the triangle-normal flat test. The territory's `SurfaceMask::Flat`
  carries the same `angle_threshold` but the kernel tests triangle
  normals, not slope cells. Do NOT switch the kernel to the mask: that is
  an output change.
- **RestUse:** today `None`; W6.3 target `None`.
- **Hooks:** none. `query` default, `SpanRule::CutRuns`.
- **Risk:** lowest. One kernel call, no annotations, no report.

### 3.2 RadialFinish

- **Kernel:** `finish::radial_finish::radial_finish_toolpath_with_cancel`
  (`finish/radial_finish.rs:84`). Spokes from the mesh bbox centre.
- **Already moved:** as 3.1.
- **Moves to the runner:** the "Radial ray" cut-run tail (`finish_3d.rs:667-670`).
- **Stays op-specific:** the N10 range refusal (`finish_3d.rs:633-649`)
  goes into `refuse()` unchanged. It is a shared-service candidate for the
  W5 derive (every op could refuse an out-of-range dial at generate), but
  that changes other ops; record it as a backlog row, not W6.3. The spoke
  centre stays the mesh bbox centre (a centre from the territory is an
  output change).
- **RestUse:** `None` → `None`.
- **Hooks:** `refuse`.
- **Risk:** low. Only the order "range check, then mesh" changes to "mesh
  (prologue), then range check"; the prologue already checked the mesh
  before the adapter ran, so the order of refusals is the same.
  `radial_finish_ranges_n10` pins the text.

### 3.3 SpiralFinish

- **Kernel:** `finish::spiral_finish::spiral_finish_toolpath_structured_annotated_with_cancel`
  (`finish/spiral_finish.rs:146`). Returns `(Toolpath, Vec<SpiralFinishRuntimeAnnotation>)`.
- **Already moved:** as 3.1.
- **Moves to the runner:** `annotate` call order, `spans_from_labeled_events`,
  `generated_with_spans` (`finish_3d.rs:607-616`).
- **Stays op-specific:** `annotate` hook → `annotate_spiral_finish`.
  The centre stays the mesh bbox centre.
- **RestUse:** `None` → `None`.
- **Hooks:** `SpanRule::LabeledEvents`, `annotate`, `Annotation = SpiralFinishRuntimeAnnotation`.
- **Risk:** low. The first op on `LabeledEvents`; it proves that rule
  before Ramp and Scallop use it.

### 3.4 VCarve

- **Kernel:** `ops::vcarve::vcarve_toolpath_with_cancel` (`ops/vcarve.rs:153`),
  called once per polygon in input order (`curve_engrave.rs:131-147`).
  The field is `top_z - min(dist / tan(a), max_depth)` over an
  `EdgeDistanceField` per polygon.
- **Already moved:** cancel first, polygons check, V-bit refusal
  (registry `required_kinds: [VBit]`), the lead strip (G-VCARVELEAD lives
  in the registry `dressup_policy`, not in the adapter — no hook needed).
- **Moves to the runner:** the "V-carve run" cut-run tail
  (`curve_engrave.rs:148-151`).
- **Stays op-specific:** `query` override → `HeightQuery::Analytic(&VCarveField)`,
  where `VCarveField` holds the checked polygons; `plan` loops the
  polygons and calls the kernel per polygon (the same loop, the same
  order). `vbit_half_angle` (`curve_engrave.rs:282`) stays shared with
  Inlay and Chamfer.
- **RestUse:** `None` → `None`. Territory XY stays `ClipAfter`; the field
  domain is the input outline itself, not a territory.
- **Hooks:** `query` (Analytic). Reserved, NOT built: "depth stepping by
  clamp" (`z_k = max(field, level_k)` down a `ConstantStep`/`EvenRedistributed`
  `z_ladder`). It needs a new config dial (W5 `#[param]`), an operator
  ruling and a re-bless. When it comes, it is a runner `ladder` hook on
  the Analytic arm only, not a VCarve-private loop.
- **Risk:** low for output (same loop). Design risk: VCarve is the only
  `Analytic` op, so the enum arm has one user. ESCAPE HATCH (§5.3): if the
  operator does not want a one-user arm, VCarve stays in the curve family
  and W6.3 has six ops.

### 3.5 DropCutter (3D Finish)

- **Kernel today (spread over two places):**
  - `surface::dropcutter::batch_drop_cutter_with_cancel` (`surface/dropcutter.rs:110`);
  - the floor `effective_min_z` and the off-mesh zeroing loop — ADAPTER code
    (`finish_raster.rs:261-294`);
  - the `SlopeMap::from_z_grid` conversion (`finish_raster.rs:306-328`);
  - `toolpath::raster_toolpath_from_grid` (`toolpath.rs:607`) or
    `raster_toolpath_from_grid_with_slope_filter` (`toolpath.rs:773`).
- **Already moved:** cancel first, mesh + index, tool, `territory.regions()`,
  `territory.mask` (`SlopeBand` only when active), the link ceiling builder
  (`op_link_ceiling`, R1).
- **Moves to the runner:** the link stage (`finish_raster.rs:364-392`) via
  `LinkRule::AfterPlan { hookup_mm: cfg.hookup_mm }`, with
  `Planned::link_geometry = Some(LinkGeometry { sampling: grid.x_step.max(0.01), stock_to_leave: 0.0, link_tolerance: None, feed, plunge, safe_z })`;
  the totals record; the "Raster row" cut-run tail.
- **Becomes kernel (not framework):** move the floor + off-mesh loop
  (`finish_raster.rs:256-298`) into one function in `surface/dropcutter.rs`
  (for example `drop_cutter_raster_grid(mesh, index, cutter, stepover, min_z, stock_bottom_z, cancel) -> (DropCutterGrid, f64)`),
  so the `plan` impl is: grid → (mask?) slope map → raster. The loop body
  moves byte for byte (the `#[allow(clippy::indexing_slicing)]` keeps its
  `SAFETY:` line). `geometry/monotone_cells.rs` also calls
  `batch_drop_cutter_with_cancel`; it does not change.
- **Stays op-specific:** `cfg.min_z`, `cfg.stepover`, the slope band from
  `territory.mask`.
- **RestUse:** today `LinkSafetyOnly` (the stock reaches only the link
  ceiling, and only when `hookup_mm > 0`); W6.3 target `LinkSafetyOnly`.
  Candidate later: a per-sample material filter (§7).
- **Hooks:** `LinkRule::AfterPlan`, `Planned::link_geometry`.
- **Risk:** medium. It is the first op whose link stage moves into the
  runner, and it has a rest arm in G-OPGOLDEN. The default `hookup_mm` is
  `0.0`, so G-OPGOLDEN does not exercise the stage: `link_stage_g_linkstage`,
  `island_stay_down_links_o3` and `link_counters_visible_g_linkvisible`
  must run.

### 3.6 RampFinish

- **Kernel:** `finish::ramp_finish::ramp_finish_toolpath_structured_annotated_with_cancel`
  (`finish/ramp_finish.rs:520`) → `..._with_resolution` (`:550`). Inside:
  `cached_finish_surface` at `ramp_finish_generation_resolution`; its OWN
  Z ladder, `ops::depth::z_ladder(z_top, z_bottom, max_stepdown, ConstantStep { epsilon: step/2, snap_to_bottom: true })`
  (`:617`), from `bbox.max.z + stock_to_leave` down to the reach-clamped
  holdable bottom (`min_covered_z`, `:603`); push-cutter
  `waterline_contours` per level (`:643`); the between-level blend; the
  per-point `point_drop_cutter` reach clamp (`:751`); the slope + region
  keep (`:655`, `:785-800`).
- **Already moved:** cancel first, mesh + index, tool, `territory.regions()`.
- **Moves to the runner:** `ctx.record(&reach_clamp)` (unconditional,
  PR-8b), the labeled-event spans, the `annotate` order
  (`finish_3d.rs:563-573`).
- **Stays op-specific:** the whole ladder, the terraces, the clamp, the
  slope band (read from `params.slope_from/to`; the territory mask holds
  the same two config values through `surface_mask`, so the two readers
  cannot differ today).
- **RestUse:** `None` → `None`.
- **Hooks:** `Report = RampReachClamp`, `LabeledEvents`, `annotate` →
  `annotate_ramp_finish`. Own ladder: NOT a runner hook (§5.2).
- **Risk:** low for output (one kernel call). The finding must stay
  unconditional: `ramp_reach_clamp_pr8b` pins it.

### 3.7 Scallop

- **Kernels:**
  - offset cascade: `finish::scallop::scallop_toolpath_structured_annotated_with_cancel_and_stage`
    (`finish/scallop.rs:1100`) → `scallop_toolpath_research_with_stage`
    (`finish/scallop/research.rs:191`);
  - iso field: `scallop_toolpath_iso_field_with_cancel` (`research.rs:76`).
- **Territory as PLANNER INPUT:** the regions are the ring boundaries
  (`research.rs:365`: one ring set per region, the mesh-bbox rectangle when
  there are none), and the keep predicate tests them again
  (`research.rs:447`). This is why the runner passes the whole `Territory`
  to `plan`, before planning, and never clips the plan afterwards.
- **Already moved:** cancel first, mesh + index, ball-tip refusal (registry),
  `territory.regions()`, the ceiling (`op_link_ceiling` inside
  `finishing_link_stage`).
- **Moves to the runner:** building the stage (`finish_raster.rs:210`),
  `ctx.record(&scallop_report)`, the `annotate` order and the
  labeled-event spans (`finish_raster.rs:225-240`).
- **Stays op-specific:** the `iso_field` switch inside `plan` (iso field
  ignores `pctx.link_stage`, as today: `LinkRule::InPlan` is built for
  both, and the iso arm does not pass it — or `link_rule()` reads
  `cfg.iso_field` and returns `Off`; the second is clearer and is the
  same output); the link INSIDE the kernel; `annotate_scallop` with
  `ScallopRegionGrouping::ByBoundaryRegion`; the capabilities that read the
  instance (`continuous`, `catalog.rs:751`) stay in `capabilities()`.
- **RestUse:** today `LinkSafetyOnly`; W6.3 target `LinkSafetyOnly`.
- **Hooks:** `LinkRule::InPlan` (escape hatch, §5.1), `Report = ScallopReport`,
  `LabeledEvents`, territory as input.
- **Risk:** highest of the seven, last in the order: two kernels, the
  in-kernel stage, the ring annotations reconciled across the relink, and
  the kernel is shared with UnifiedFinish (do not touch it).

## 4. The `Produced` side channel for this shape

None of the seven ops fills `planner_engagement`, `rest_grid`,
`rest_regions` or `area_regions`. Each `plan` returns
`Produced::default()` `[W6.2]`. The real side channel of this shape is
`Planned::annotations` (per-move labels) and `Planned::report` (findings).
The dispatch's generic rest analysis (`execute.rs:614-628`, keyed on
`produced.has_no_rest_output()`) keeps working unchanged, because an empty
`Produced` is what these ops give it today. The field stays in `Planned`
so that W6.4 (Pencil, Unified) uses the same return type.

## 5. What does not fit, and the honest escape hatches

### 5.1 Scallop links inside its kernel

The ring annotations carry move indices. The relink changes the indices,
and the reconcile sits beside the relink in `research.rs:781-800`. To move
the link into the runner, the kernel would expose its annotations as a
provenance channel (the alternative the doc at `scallop.rs:1081-1093`
rejected). Escape: `LinkRule::InPlan` — the runner OWNS the stage
(one builder, one ceiling, one boundary, one kinematics), the kernel only
RUNS it. The rule "one link stage" holds; "the runner runs the link" does
not, for this op. State it in the trait doc.

### 5.2 Ramp owns its Z ladder

Ramp cuts BETWEEN two levels (terrace = level pair i, i+1), the ladder
bottom comes from the surface reach clamp, and the ladder is the crate's
one `z_ladder` (`ConstantStep`) already. It does not fit Layered:
`emit_level` is per level and holds no state across levels; a pair hook
would bend Layered for one op. It does not need a height-field ladder hook
either: nothing outside the kernel reads the ladder. Escape: Ramp is a
height-field op whose ladder is internal. If a later item wants the ladder
visible (spans by terrace already come from the annotations), add a
read-only `ladder()` report then, not now.

### 5.3 VCarve is the only Analytic op

`HeightQuery::Analytic` has one user, and VCarve is a 2D op
(`OperationFamily::TwoPointFiveD`, `GeometryRequirement::Polygons`). The
gain is the shared tail and a home for the future clamp ladder. Escape:
leave VCarve in the curve family (`curve_engrave.rs`) and drop the
`Analytic` arm; the plan then has six ops and no other change. Recommend:
migrate it (cost ~30 lines, no output risk), but ask the operator (R-W6.3a).

### 5.4 The Horizontal `Flat` mask is not a cell mask

`SurfaceMask::Flat` states the threshold, but the kernel tests triangle
normals per group. The territory mask is information for this op, not an
input. No escape needed; record it in the trait doc so nobody "unifies"
it into a cell mask without a re-bless.

### 5.5 Not in this shape

Waterline is Layered (W6.1). SteepShallow, UnifiedFinish, Pencil are W6.4
(composed). Project Curve is W6.5 (curve). Adaptive3d is W6.5 (volumetric).

## 6. Migration order and proof

### 6.0 Prerequisite W6.3.0: close the gap in G-OPGOLDEN (before any move)

G-OPGOLDEN (`tests/every_op_keeps_its_toolpath_g_opgolden.rs`) hashes the
moves, the feed and the intent. It does NOT cover:

- the spans and their labels;
- the findings;
- a boundary-region territory (the fixtures have no `boundary_regions`),
  so Scallop's ring boundaries and every `regions.contains` keep are
  unprotected;
- an active slope band, `iso_field = true`, `hookup_mm > 0`, Spiral
  `OutsideIn`, Ramp `order_bottom_up`.

W6.3.0 adds, on the CURRENT code, before any adapter moves:

1. a span digest in each golden entry: the count and an FNV-1a hash of
   `(kind, label, start, end)` per span (`common/fingerprint.rs`);
2. a `regions` arm for each `TerritoryXy::Regions` op of this shape
   (DropCutter, Scallop, Horizontal, Radial, Spiral, Ramp): one boundary
   rectangle inside the cap fixture (`common/op_fixtures.rs`);
3. non-default arms: DropCutter `slope_band` + `hookup`, Scallop `iso_field`,
   Spiral `outside_in`, Ramp `bottom_up`.

Bless once, with the commit body "added fields and arms; no op change".
After that, every W6.3 step must keep the golden byte-identical.

Findings stay with the finding sentries (§6.2); a findings digest in the
golden is optional (it needs a stable `Debug` of `GenerationFindings`; do
not add it if it does not exist).

### 6.1 Order (least risk first)

| Step | Op | Why here |
|---|---|---|
| W6.3.1 | runner + Horizontal | pure kernel call, cut-run spans, no report |
| W6.3.2 | Radial | adds `refuse()` |
| W6.3.3 | Spiral | first `LabeledEvents` + `annotate` |
| W6.3.4 | VCarve (after R-W6.3a) | first `Analytic` query; no mesh |
| W6.3.5 | Ramp | one kernel call; first unconditional report |
| W6.3.6 | DropCutter | first `AfterPlan` link, kernel glue moves to `surface/dropcutter.rs`, rest arm |
| W6.3.7 | Scallop | `InPlan` link, territory as input, two kernels |

One editor at a time, one step per commit (the lead commits by path).

### 6.2 Per-step proof

Every step: `cargo fmt --all -- --check`; the full clippy line; then

- always: `--test every_op_keeps_its_toolpath_g_opgolden` (no bless),
  `--test every_op_conforms_w4`, `--test every_op_states_its_capabilities_w2_4`,
  `--lib every_generator_polls` (cancel past the prologue),
  `--lib cancellable_families_honour_a_preset_cancel_flag`,
  `--test retract_intent_move_type_census_w6`, `--test capability_link_moves_safety`,
  `--lib compute::execute::height_field` (the runner's own unit tests: a
  test op that records its report once, records link totals only when the
  stage ran, and keeps the span labels).

Per op (all exist; none below is behind `heavy-tests`):

| Op | Sentries |
|---|---|
| Horizontal | `the_printed_tip_row_serves_every_3d_finish_g3v8` (do not use `air_cut_family_calibration_w5bf4`: it is behind `heavy-tests`) |
| Radial | `radial_finish_ranges_n10`, `set_param_refuses_absent_field_n5` |
| Spiral | `narrate_regions_closed_c8` (labeled spans reach narration) |
| VCarve | `vcarve_cuts_nothing_outside_the_outline_g_vcarvelead`, `vcarve_lift_bridge_b1`, `inlay_pocket_and_plug_fit_g_inlaysplit` (shares `vbit_half_angle`) |
| DropCutter | `drop_cutter_off_mesh`, `drop_cutter_flat_roughing_row_g_dcflat`, `link_stage_g_linkstage`, `island_stay_down_links_o3`, `link_counters_visible_g_linkvisible`, `one_link_ceiling_for_every_op_g_linkceil`, `a_link_through_stock_is_flagged_g_linkplough`, `fed_chords_ride_the_surface_g_tierburial` |
| Ramp | `ramp_reach_clamp_pr8b`, `ramp_contained_in_region_g_rampcontain`, `finish_resolution_policy_pr3` |
| Scallop | `scallop_intra_pass_relink_am7`, `scallop_trace_survives_relink_g_linktrace`, `a_scallop_ring_connector_rides_the_surface_g_tierburial`, `scallop_iso_field_config`, `scallop_untouched_standing_h4`, `scallop_solo_vs_unified_s1`, `link_counters_visible_g_linkvisible`, `one_link_ceiling_for_every_op_g_linkceil` |

CAUTION: `scallop_isofield_gouge_m4` is behind `heavy-tests` (about 237 s).
Ask the operator before you run it. Run it once, after W6.3.7, only if the
iso-field arm of 6.0 does not already pin the iso path.

Live check after W6.3.7: rebuild release, load `testpiece_mini`, generate
all, simulate, compare with the W6.2 baseline (expect no change).

## 7. RestUse: today and target

| Op | Today | W6.3 target | Later candidate (separate item, re-bless) |
|---|---|---|---|
| DropCutter | `LinkSafetyOnly` | `LinkSafetyOnly` | per-sample material filter |
| Scallop | `LinkSafetyOnly` | `LinkSafetyOnly` | material filter in the ring keep predicate |
| Horizontal | `None` | `None` | per-sample material filter |
| Radial | `None` | `None` | per-sample material filter |
| Spiral | `None` | `None` | per-sample material filter |
| Ramp | `None` | `None` | per-sample material filter |
| VCarve | `None` | `None` | skip a sample where the stock top is already below the cut Z |

The later candidate is one new `RestUse` (working name `SampleFilter`):
the Waterline `ContourFilter` rule applied to height-field samples (keep a
sample where material stands above `z + FRESH_MATERIAL_THRESHOLD_MM` within
`R + cell`, split a run only when the retract saves time). It would be a
runner step between `plan` and the link (the samples are the planned
moves), it needs `Territory::resolve` to fill `stock` for that rule
`[W6.2]`, and each op gets a G-RESTLEVELS case. It changes output on rest
arms, so it is NOT W6.3. Proposed as W6.3b for an operator ruling.

## 8. Rulings to ask for

- **R-W6.3a:** VCarve in this shape (Analytic arm), or keep it in the curve family?
- **R-W6.3b:** the `SampleFilter` rest rule for height-field ops: yes/no, and which ops first.
- **R-W6.3c:** the W6.3.0 golden extension (span digest + regions arm + non-default arms) before the moves: approve the one bless.

## 9. Backlog rows this plan opens

- Radial N10 range refusal → a shared "every op refuses an out-of-range
  dial at generate" service from the W5 derive (changes other ops' refusals).
- Radial and Spiral centre from the territory, not the mesh bbox (output change).
- Scallop and Ramp read the slope band from `territory.mask`, not from
  their params (needs a kernel signature change; do it with W6.4, which
  owns the other kernel callers).
- V-carve depth stepping by clamp (needs a dial and a ruling).
