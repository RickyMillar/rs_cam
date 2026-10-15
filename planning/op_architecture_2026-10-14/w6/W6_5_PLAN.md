# W6.5 — the Volumetric and Curve wrappers (implementation plan)

Status: PLAN ONLY. No code changed. Nothing here is decided until the lead
rules on it.

Base: `a0db1ecb` plus the uncommitted W6.2 work in the checkout
(`compute/execute/territory.rs`, `trace/produced.rs`, edits to
`finish_3d.rs`, `curve_engrave.rs`, `adaptive3d/mod.rs`). All line numbers
are working-tree lines at the time of writing. The W6.2 editor can move
them. Search by symbol name if a line does not match.

## Goal and limit

W6.5 puts the last two families under a declared shape:

- **Volumetric**: 3D Rough (`Adaptive3d`).
- **Curve**: Project Curve (`ProjectCurve`).

The wrappers are thin. They change no output. The only intended new
behaviour is one FINDING (the Curve engagement guard), which writes no move.
G-OPGOLDEN must stay green with NO re-bless.

The wrappers are honest. A wrapper does not claim to own a thing that the
planner owns. Each shape states, in one place, what the planner owns, and a
sentry holds that statement equal to the registry row.

## 1. Volumetric shape (Adaptive3d)

### 1.1 The shape

A new file `compute/execute/volumetric.rs`, next to `layered.rs`:

```rust
/// What a planner that owns its stock states about itself (W6.5).
pub(crate) struct PlannerOwns {
    pub stock: bool,          // seeds its own dexel stock from initial_stock
    pub level_anchor: bool,   // G-LADDERANCHOR, path.rs:316
    pub entries: bool,        // plan_entry; dressup entry FORCED off
    pub links: bool,          // stay-down links; LinkCeilingNeed::NoLink
    pub segment_merge: bool,  // G-PLANSIMGAP; planner_applies_segment_merge
}

/// How the planner reads the resolved heights.
pub(crate) struct HeightsPolicy {
    pub top: TopAnchor,        // StockBox: ctx.stock_bbox.max.z, never heights.top_z
    pub pinned_bottom: bool,   // == Capabilities::honors_pinned_bottom_z
}

pub(crate) trait VolumetricOp {
    const OWNS: PlannerOwns;
    const HEIGHTS: HeightsPolicy;
    type Plan;                          // the kernel's param struct
    fn plan_params(&self, ctx: &ExecutionContext<'_>, inputs: &VolumetricInputs<'_>) -> Self::Plan;
    fn run(&self, plan: &Self::Plan, mesh: MeshInput<'_>, ctx: &ExecutionContext<'_>)
        -> Result<VolumetricOutput, OperationError>;   // toolpath, spans, Produced
}

pub(crate) fn run_volumetric<O: VolumetricOp>(op: &O, ctx, prepared) -> Result<GeneratedToolpath, _>;
```

`VolumetricInputs` is what the runner resolves ONCE and hands over: the
mesh, the `PlannerBounds` territory (outline and centre), the seed stock
(`ctx.initial_stock`, NOT guarded, see 1.4), the top Z from
`HeightsPolicy::top`, the floor from `HeightsPolicy::pinned_bottom`, and the
two planner-only channels (`segment_merge_tolerance`, the helix radius).

`run_volumetric` owns:

- the read of the territory (it is already W6.2; the runner moves the
  `match prepared.territory().xy` out of the adapter, `finish_3d.rs:54-57`);
- the top Z and the floor, from `HEIGHTS` (moves `finish_3d.rs:149` and
  `:155` out of the struct literal);
- the cancel closure (`ctx.cancel_fn()`, `finish_3d.rs:190`);
- the spans and the semantic annotation of the output;
- the attach of `Produced` (`finish_3d.rs:201-202`);
- the record of a planner report through `ctx.record` (none today; the
  door is there for B4 or a later planner report).

The op (`Adaptive3dRough`, in `finish_3d.rs` or a new `rough_3d.rs`) owns:

- the config to kernel param build (`finish_3d.rs:107-180`);
- the three enum maps (`finish_3d.rs:59-98`). Replace each `match` with a
  `From<operation_configs::X> for adaptive3d::Y` impl next to the kernel
  enum. This is the one real thinning of the adapter. Same values, so no
  output change;
- the engagement radius (`finish_3d.rs:103-106`) and the axial leave
  (`adaptive3d_effective_stock_to_leave`, `finish_3d.rs:31-35`).

### 1.2 What stays in the planner (file:line)

| Concern | Where | Why it stays |
|---|---|---|
| Seed stock clone, or a fresh block at `stock_top_z` | `adaptive3d/path.rs:648-676` | `RestUse::Planner`: the planner stamps each cut into its own stock |
| F-027 grid widening to the world stock XY | `path.rs:619-643`, input `finish_3d.rs:125-130` | the planner grid must cover the simulator grid |
| Boundary pre-clear of the planner stock | `path.rs` after `:700` (cells outside `geometry.boundary`) | O5b; the stock must reflect the boundary |
| Level anchor (G-LADDERANCHOR) | `path.rs:316` `level_anchor_z`, called at `:846-852` | reads the planner stock, not the snapshot |
| Z ladder | `path.rs:348` `step_levels`; shelves `path.rs:35` `flat_shelf_levels`; floor `path.rs:841-843` | ONE Depth/Pass ladder (adaptive3d/CLAUDE.md); not the Layered ladder |
| Merge before stamp (G-PLANSIMGAP) | `adaptive3d/mod.rs:171` `cut_simplify_tolerance`; `path.rs:1683` | the planner stamps the path that ships |
| Entries | `adaptive3d/clearing.rs:776` `plan_entry`; `:2877` `coalesce_level_entries`; peck `path.rs:162-171` | stock-aware; F-031: a dressup entry breaks stamp parity |
| Links (stay-down) | `path.rs:1185` `resolved_stay_down_mm`; `:1231` `try_emit_stay_down_link`; `:1400` `segments_to_toolpath` | own corridor ceiling, out of R1 scope (`shared.rs:89-91`) |
| Centre clip (G-BOUNDARYPHANTOM) | `path.rs:907` `CentreClip::new`; `centre_clip.rs:50` | the planner stamps only what the session clip keeps |
| Region order (Global / ByArea) | `path.rs:956+`; `region_map.rs` | R4: 3D Rough keeps its own default (Global, level-first) |

The wrapper does not call `run_layered`, and it does not build a ladder. The
Layered `RegionOrdering` and the planner `RegionOrdering` stay two enums
joined by the new `From` impl.

### 1.3 The heights policy (F-027 / finding 2)

`stock_top_z: ctx.stock_bbox.max.z` (`finish_3d.rs:149`) IGNORES a pinned
`top_z` by design (heights audit 2026-06-12 finding 2, F-027 shape, F-034
calibration anchor wanaka100 Back Rough). The runner must NOT read
`ctx.heights.top_z` for this op. `HeightsPolicy::top = TopAnchor::StockBox`
states it. The pinned bottom IS honoured (`z_floor`, `finish_3d.rs:155`).

New sentry (pure, no generation): `HEIGHTS.pinned_bottom ==
OperationType::Adaptive3d.capabilities().honors_pinned_bottom_z`, and the
GUI Heights tab text for a pinned top on 3D Rough states that it is not
read (check `ui/properties` for the existing hint; add none if it exists).

### 1.4 Produced items

Already on `Produced` after W6.2 (`adaptive3d/mod.rs:534-575`,
`trace/produced.rs:35` and `:51`):

- `planner_engagement` — Stage 4 `(cut_point, α/2π)`; ContourSpiral only;
  the feed modulator reads it after the dressups.
- `area_regions` — the ByArea detection; `None` for Global; the viewport
  overlay and MCP `inspect_spans` read it.

W6.5 only names them in `VolumetricOutput`. `rest_grid` and `rest_regions`
stay empty; the generic rest analysis in the dispatch (`execute.rs:604-622`)
still fills them when it is on.

### 1.5 RestUse

- Today: `RestUse::Planner` (`catalog/registry.rs:553`).
- Target: `RestUse::Planner`. No change.

`Territory::stock` skips the W2.1 guard for a `Planner` op on purpose
(`territory.rs:76-81`, `:100-105`). The runner passes `ctx.initial_stock`
raw. Do not route it through `Territory::reference_stock()`: the guard can
drop a stock that the planner must seed from, and that changes output.

### 1.6 The declaration sentry

`OWNS` must agree with four flags that live in four places today. New
`--lib` test `compute::execute::volumetric::the_declaration_matches_the_row`:

| `OWNS` field | Must equal |
|---|---|
| `entries` | `registry.rs:539` `DressupPolicy::FORCE_NO_ENTRY` |
| `segment_merge` | `TRANSFORM_ADAPTIVE_3D.planner_applies_segment_merge()` (`capabilities.rs:382-385`) |
| `links` | `Capabilities::link_ceiling == LinkCeilingNeed::NoLink` and `!transform.rapid_reorder` |
| `stock` | `Capabilities::rest_use == RestUse::Planner` |

Prove it red once: flip one flag in the test fixture, see the failure,
restore.

### 1.7 Not in W6.5

- B4 (AgentSearch / ContourSpiral double inset, ~2.4x wall-band wood): an
  output change. It stays in the backlog. The wrapper gives it a report
  door only.
- The strategy advisor (`session/compute.rs:1398`, `:1458`) clones the op
  and sets `clearing_strategy`. It goes through the same dispatch, so it
  gets the wrapper for free. Its `match` on `OperationConfig::Adaptive3d`
  stays (see the remainder list).

## 2. Curve shape (ProjectCurve)

### 2.1 The shape

A new file `compute/execute/curve.rs`:

```rust
pub(crate) struct CurveFrame { pub z_flipped: bool }

pub(crate) trait CurveOp {
    fn plan(&self, curves: &[Polygon2], heights: Option<&HeightQuery<'_>>,
            frame: CurveFrame, cancel: &dyn CancelCheck) -> Result<Toolpath, OperationError>;
    fn link(&self, tp: Toolpath, ctx, link: &CurveLinkInputs<'_>) -> Toolpath;   // default: none
    fn span_label(&self) -> &'static str;                                       // "Projected curve"
}

pub(crate) fn run_curve<O: CurveOp>(op: &O, ctx, prepared, op_type) -> Result<GeneratedToolpath, _>;
```

`HeightQuery`: if W6.3 has landed, use its type. If not, define it here as a
newtype over `MeshInput` and let W6.3 take it over. `Option`: `None` is
RESERVED for a curve op with no surface. No op is that today
(`GeometryRequirement::Both`, `registry.rs:929`; Trace is a Layered op).
Recommendation for the lead: take `&HeightQuery`, not `Option`, until a
caller with `None` exists. If the lead keeps `Option`, the runner passes
`Some` always and no `None` arm is built.

`run_curve` owns:

- the frame (2.3);
- the per-curve loop and the concatenation (`curve_engrave.rs:345-351`);
- the link stage inputs: the territory regions (W6.2, `:357`), the ONE
  link ceiling `op_link_ceiling(op_type, ctx)` with the op type from the
  config (fixes the hard-coded `OperationType::ProjectCurve` at `:438`),
  `ctx.link_kinematics`;
- the engagement guard finding (2.4);
- the cut-run spans and the depth-run annotation (`:359-362`).

The op owns:

- the three enum maps (`curve_engrave.rs:305-331`) → `From` impls, as in 1.1;
- the kernel params (`:332-343`) and the boxed cutter (`:304`);
- the link body `chain_project_curve` (`:365-467`): its `RelinkParams`
  choices (`flush_ride: false`, `airborne_links_may_leave_territory:
  false`, `reorder: true`) are op policy.

The kernel keeps (`ops/project_curve.rs`): resampling, the side offset,
Cutter/Point projection, the hole test `point_over_triangle` (`:116`), the
standalone `FromBelow` mesh flip (`:219-243`), and the emission.

### 2.2 Produced items

None. The relink report (`curve_engrave.rs:447-458`) goes only to
`tracing`. Optional, no output change: record it through the existing
`RelinkTotals` finding as Unified Finish does (`findings.rs:308`). List it
as a lead choice, not as W6.5 scope.

### 2.3 The frame: `setup_z_flipped` from the context

Today:

- `ProjectCurveConfig::setup_z_flipped` (`operation_configs.rs:2570-2573`),
  `#[serde(skip)]`, `#[param(driver_set = "the setup transform writes this
  from the setup face")]`. W1.6 (`3ccc05bb`) made it driver-set: the
  schema does not offer it and the setter refuses it. W5 (`95ee4b3c`) moved
  that into the derive (`ParamTable::driver_set`).
- The session writes it on a clone before dispatch
  (`session/compute/generation.rs:330-339`) from
  `SetupEvalContext::is_z_flipped()` (`session/eval_context.rs:158-162`).
- The adapter copies it into the kernel params (`curve_engrave.rs:342`) and
  reads it in the chain frame guard (`:385-397`).

The value is already on the context. `ExecutionContext::setup_transform` is
`ctx.local_to_global` (`generation.rs:493`, `session/compute.rs:1205`), and
`SetupEvalContext::is_z_flipped()` is exactly
`local_to_global.is_some_and(SetupTransformInfo::is_z_flipped)`
(`compute/transform.rs:564`). So:

1. `run_curve` computes `CurveFrame { z_flipped:
   ctx.setup_transform.is_some_and(SetupTransformInfo::is_z_flipped) }`.
   Add `pub(crate) fn z_flipped(&self) -> bool` on `ExecutionContext` so the
   rule has one home.
2. The kernel param `ProjectCurveParams::setup_z_flipped`
   (`ops/project_curve.rs:90`) stays. It is a kernel frame input, and five
   direct kernel tests set it (`project_curve_chaining.rs:328`,
   `param_sweep.rs:2200`, `project_curve_deviation.rs:346`,
   `project_curve_depth_sign.rs:87`, `capability_link_moves_safety.rs:1136`).
3. DELETE the config field, its default (`operation_configs.rs:2625`) and
   the session patch (`generation.rs:330-339`). The comment at
   `generation.rs:344` that names it changes. Operator ruling 2026-09-16: no
   legacy. The field is `serde(skip)`, so no project file changes. State
   the break: an MCP read no longer shows `setup_z_flipped`.
4. `chain_project_curve` reads `frame.z_flipped`, not
   `cfg.setup_z_flipped`.
5. `project_curve_chaining.rs:857-860` `standalone_from_below_refuses_to_chain`
   drops `cfg.setup_z_flipped = false` and relies on `setup_transform: None`
   in its context.

Parity for each caller:

- the session door: old and new both read `local_to_global` of the same
  setup (`generation.rs:338` and `:493`), so the value is the same;
- every other caller (the strategy advisor probe, direct tests): old read
  `false` from the config default (`operation_configs.rs:2625`); new reads
  `false` from `setup_transform: None` (`ExecutionContext::new`,
  `execute.rs:469`).

Check before you delete: other driver-set fields exist
(`operation_configs.rs:322`, `:328`, `:357`, `:385`, `:389`: drill
selected holes and layers, pin drill). So
`session/compute/tests.rs:406` (`driver_set_read >= 1`) and W4 check6
(`driver_count > 0`, `every_op_conforms_w4.rs:1224`) stay non-vacuous.

The trap: today NO test reaches the `setup_z_flipped == true` arm through
the session door. Every site sets `false`, and W4 check4 (flipped frame)
runs the default `FromAbove`, where the flag is inert. A sentry for that
arm comes FIRST (step 0, section 3).

### 2.4 Rest: the engagement guard finding

- Today: `RestUse::LinkSafetyOnly` (`registry.rs:957`). The remaining stock
  builds the link ceiling only.
- Target: `RestUse::LinkSafetyOnly`, plus a finding. Do NOT add a RestUse
  variant: the guard does not change what the op cuts, and a new variant
  moves the frozen W2.4 table (`every_op_states_its_capabilities_w2_4`)
  and W4 `check2_rest_rule_per_op` for no behaviour.

The finding `BuriedCurveFinding` (in `compute/toolpath_stats.rs` beside
`ZeroRemovalFinding`):

- Measure: `dressup::reference_engagement_of_cutting_moves(&tp, stock,
  cutter)` (`dressup/mod.rs:2140`) on the PRE-link toolpath, against
  `ctx.initial_stock`.
- Fire when `deepest_mm > |cfg.depth| + floor`, with `floor` from the same
  cell rule as `zero_removal_engagement_floor_mm` (`findings.rs:403`;
  `cell² / (2 · tip radius)`, min 1 µm). Make that function
  `pub(crate)` and call it; do not copy it.
- Fields: `deepest_engagement_mm`, `commanded_depth_mm`, `sampled_positions`,
  `floor_mm`.
- Only when `ctx.initial_stock.is_some()`. On fresh stock, write nothing
  (X-19: no measurement, no claim). Open for the lead: also measure on fresh
  stock against the analytic stock top
  (`dressup::fresh_stock_top_z`)? That fires on every engrave below an
  uncut stock top, which is the normal case, so the recommendation is NO.
- Door: a `GenerationFindings::buried_curve: Option<_>` slot, a
  `RecordFindings` impl, `ToolpathStats` carry, and one diagnostics adapter
  row (warning: "standing stock above the curve is deeper than the engrave
  depth; rough it first").
- LEAD DECISION, blocks this item: the threshold. The rule above fires on
  the normal rough-then-engrave case. The op_fixtures rest prior
  (`tests/common/op_fixtures.rs:437-450`) is a default 3D Rough with a flat
  end mill: leave 0.5 mm (`stock_to_leave_axial`), depth per pass 3.0 mm,
  so the cap keeps 0.5 mm plus flat-mill terraces up to about 3 mm. A
  1.0 mm engrave on that stock sees 1.5 mm or more and FIRES. Two choices:
  - (A) Fire there on purpose. The feeds and the chip load were set for
    `depth`; the tool really cuts `leave + depth` or more. The finding is
    then a load warning, and it is true.
  - (B) Fire only on a buried curve: threshold `|depth| + leave_ref +
    floor`, where `leave_ref` is a stated allowance (for example the
    largest `stock_to_leave` of the prior ops in the carve order). This
    needs a new read of the prior ops, which the adapter does not have.
  Recommendation: (A), with the text "the tool cuts N mm, not the
  commanded D mm", because it needs no new input and it states a measured
  fact.
- Sentry `a_buried_project_curve_is_named_w6_5`, cases set from the real
  fixture, not from intent: (a) fresh → `None`; (b) the G-OPGOLDEN rest
  arm (the prior above) → `Some` under (A), `deepest_mm` near
  `0.5 + depth` or more; (c) a rest arm whose prior is a finish to the
  mesh (leave 0) → `None` under both choices. Prove red once by forcing
  the threshold to `+inf`.

### 2.5 Side finding (not W6.5 scope)

The ProjectCurve row has `relink_dial: None` (`registry.rs:955`), but
`chain_distance_mm` is its hookup dial. Only the narration lever line reads
the field (`trace/narrate.rs:1416`). Text change only; the W2.4 table moves.
Lead to decide.

## 3. Proof plan

### Step 0 (before any refactor): close the golden gaps

Run step 0 only AFTER W6.2 is committed and its G-OPGOLDEN is green, not on
the mixed checkout.

G-OPGOLDEN today holds `adaptive3d/fresh`, `adaptive3d/rest`,
`project_curve/fresh`, `project_curve/rest`, all at default config
(ContourParallel, Global, Plunge; chain 0, FromAbove, TopUp). That leaves
the enum maps and the frame arm unproven. Add arms, bless them on the
unchanged code, and the lead commits them before the refactor:

| Arm | Covers |
|---|---|
| `adaptive3d/by_area_helix_spiral` (ByArea, Helix, ContourSpiral) | three enum maps, `area_regions`, `planner_engagement` path |
| `adaptive3d/ramp_agent` (Ramp, AgentSearch) | the other two map arms |
| `project_curve/chained` (`chain_distance_mm = 3.0`) | the link stage, the ceiling op type |
| `project_curve/from_below_flipped` (FromBelow, `Frame::Flipped`) | the `setup_z_flipped == true` arm |
| `project_curve/from_below_topup` (FromBelow, `Frame::TopUp`) | the standalone flip arm |

`Golden` does not hash `Produced`. Add a small digest for the two
adaptive3d arms only (engagement sample count + hash of quantized points;
area region count), in the same arm entry. If the lead prefers not to
change the golden format, put it in a new sentry
`the_volumetric_wrapper_keeps_its_produced_w6_5` with its own blessed file.

Step 0 mechanics:

- `run()` hard-codes `frame: Frame::TopUp`
  (`every_op_keeps_its_toolpath_g_opgolden.rs:227`). Add a `frame` field
  to `Case` for the `from_below_flipped` arm.
- After the bless, `git diff` the golden file. Only the new arm entries may
  appear; a bless rewrites every entry, so an old entry that moved is a
  defect in the tree, not a new truth.
- Keep the 2 s budget. AgentSearch and ContourSpiral on the cap can be
  slow. A refusal is a valid golden result; a slow arm is not. If an arm
  costs more than about 1 s, use a smaller tweak (one strategy per arm).

New sentry for the frame (also step 0):
`project_curve_from_below_on_a_flipped_setup_w6_5` — through
`generate_toolpath`, `FromBelow` on `Frame::Flipped` must equal
`FromAbove` on `TopUp`, moved by `-origin`, within the golden quantum, and
every feed move must stay inside the stock. Why that holds:

- The `setup_z_flipped == true` arm of `project_curve_toolpath_with_cancel`
  (`ops/project_curve.rs:220-224`) runs the SAME body as `FromAbove`
  (no internal flip, `z_flip = false`).
- `Frame::Flipped` mirrors every world input first, so the Bottom setup
  transform takes each input back onto the top-up input, moved by
  `-origin` (`tests/common/op_fixtures.rs:22-30`).

Why it discriminates: a flag stuck at `false` takes the internal-flip arm
(`:225-243`). That arm drops the cutter on the OTHER face of the mesh and
negates Z, so the toolpath differs from the top-up cut. The sentry must be
green before and after the config field goes.

### After each wrapper

Run in this order, each through `timeout <s> scripts/cargo_lane.sh`:

1. `cargo test -p rs_cam_core -q --test every_op_keeps_its_toolpath_g_opgolden`
   (2 s). NO bless. Any diff is a defect.
2. `--test every_op_conforms_w4` (~8 s): check4 flipped frame, check6
   driver-set refusal (the ProjectCurve field leaves the driver-set list),
   check5 determinism.
3. `--test every_op_states_its_capabilities_w2_4` (no change expected).
4. `--test one_link_ceiling_for_every_op_g_linkceil`,
   `--test capability_link_moves_safety`,
   `--test retract_intent_move_type_census_w6`.
5. `--lib compute::execute` (holds `project_curve_chaining`, the layered
   tests, `cancellable_families_honour_a_preset_cancel_flag`,
   `every_generator_polls`), `--lib compute::execute::volumetric`,
   `--lib catalog`, `--lib session::compute::tests::operation_schema`.
6. Curve: `--test project_curve_depth_sign --test project_curve_deviation`
   and the two step-0 sentries and the buried-curve sentry.
7. Volumetric, the folder sentries (adaptive3d/CLAUDE.md):
   `adaptive3d_emission_byte_parity` (KERNEL-DIRECT:
   `adaptive_3d_toolpath_with_cancel`, line 157; it proves the planner did
   not move, NOT the wrapper), `adaptive3d_plan_matches_merged_path_g_plansimgap`,
   `adaptive3d_planner_never_ahead_of_emitted_path`,
   `adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom`,
   `adaptive3d_entry_stock_aware`, `adaptive3d_keep_down_link_f038b`,
   `adaptive3d_by_area_cells_confine_jobs`,
   `adaptive3d_by_area_matches_global_stock`, `adaptive3d_commanded_ladder`.
   Skip the heavy ones (`_f027` 58 s, `_f029` 48 s) unless the wrapper
   touches the stock seed.
8. viz: `cargo test -p rs_cam_viz -q --test area_regions_overlay_by_area`.
9. Full clippy line and `cargo fmt --all -- --check`.

The rg that found the sentries:

```text
ls crates/rs_cam_core/tests | grep -i "project_curve\|adaptive3d\|agent_search"
rg -l "ProjectCurve|project_curve" crates/rs_cam_core/tests crates/rs_cam_viz/tests
rg -n "setup_z_flipped" --type rust
rg -n "driver_set" crates/rs_cam_core/src/compute/operation_configs.rs
```

Live check after W6.5: the mini project and one rivmap100 3D Rough
(`planning/fixtures/`), export identical to the pre-W6.5 export.

### Order of work (one editor at a time)

1. Step 0 arms + frame sentry (lead blesses, commits).
2. Curve frame move (2.3) — smallest, fully covered by step 0.
3. `run_curve` + `From` impls (2.1). G-OPGOLDEN no change.
4. Buried-curve finding (2.4). G-OPGOLDEN no change (findings are not
   hashed).
5. `run_volumetric` + declaration sentry (1.1, 1.6). G-OPGOLDEN no change.

## 4. The honest remainder after W6.3, W6.4 and W6.5

Assumption (PLAN.md gives W6.3/W6.4 no op list; this is the AUDIT table):
W6.3 = DropCutter, Scallop, Horizontal, Radial, Spiral, Ramp, and VCarve
only if W6.3 takes the analytic query; W6.4 = UnifiedFinish, SteepShallow,
Pencil.

### By op: still no declared shape

| Op | Where | Why |
|---|---|---|
| Drill, Pin Drill | `drilling.rs:535`, `:574` | the AUDIT "Point" shape was never scheduled |
| 2D Adaptive | `clearing_2d.rs:627` | own level loop with `level_masks` (R6); not on `run_layered` |
| Inlay, Inlay Plug | `curve_engrave.rs:33`, `:76` | fit no shape (AUDIT); per-polygon / whole-set loops |
| VCarve | `curve_engrave.rs:120` | per-polygon loop, unless W6.3 takes it |

### By mechanism: hand-written things outside the framework

- Config to kernel enum maps in each adapter (Adaptive3d and ProjectCurve
  become `From` impls in W6.5; the others stay unless their wave does the
  same).
- Op-specific span builders: `spans_from_adaptive3d_annotations`
  (`compute/spans.rs:489`) + `annotate_adaptive3d` (`annotate.rs:217`);
  the pencil event spans (`finish_3d.rs:~250-265`); the 2D Adaptive
  annotation offsets.
- Planner-only context channels: `segment_merge_tolerance` and
  `entry_helix_radius` (`execute.rs:396-408`, built at
  `session/compute.rs:1215-1224`); `reference_tool_cfg` (pencil).
- Session-side pre-generation patches on a cloned config: the pin-drill
  holes (`generation.rs:347-352`). (The ProjectCurve frame patch leaves in
  W6.5.)
- Session-side territory inputs: the single boundary polygon for the
  planner pre-clear (`generation.rs:~357-490`) and `boundary_centre`
  (`session/compute.rs:128`); W6.2 resolves them, the session still builds
  them.
- Post-generation stages that are not in a shape: the boundary clip and the
  entry-descent split `clip_and_split_entries` (`session/compute.rs:873`,
  AUDIT item 8); the generic rest analysis (`execute.rs:604-622`).
- Op-type matches outside the registry (rg counts, non-test files):
  Adaptive3d 70 sites (CLI `job.rs` 11, viz `surface_3d.rs` 10,
  `generated_empty.rs` 7, `validate.rs` 6, `feeds/ramp.rs` 5, the strategy
  advisor `session/compute.rs:1398`, `:1458`, `:1763`, `:1775`, feeds and
  tool_load files); ProjectCurve 44 sites (`validate.rs` 8,
  `feeds/vendor_normalize.rs` 4, viz `project.rs` 3, `gpu_upload.rs` 2,
  `from_preconditions.rs` 2, others). Many are W2.4 candidates
  (`capabilities()` reads); some are real per-op UI or feeds math.
- Planner-internal entries, links and segment merge (Adaptive3d, 2D
  Adaptive) — correct to stay (AUDIT "what the first design got wrong" 6),
  but outside `DRESSUP_PIPELINE` by declaration.
- Two `RegionOrdering` enums (Layered and adaptive3d) joined by a `From`.
- The ProjectCurve relink report goes to `tracing` only (2.2).
- B4 (AgentSearch / ContourSpiral wall band) stays open.

### Open for the lead

These three block implementation:

1. The buried-curve threshold: (A) fire when the tool cuts more than the
   commanded depth (fires after a normal rough), or (B) add a leave
   allowance (2.4). Recommendation: (A).
2. `Option<&HeightQuery>` or `&HeightQuery` for the Curve shape (2.1).
   Recommendation: no `Option` until a caller needs `None`.
3. The golden format: a `Produced` digest in G-OPGOLDEN or a separate
   sentry (section 3, step 0).

These do not block:

4. Buried-curve finding on fresh stock: recommendation NO (2.4).
5. `relink_dial` for ProjectCurve (2.5).
6. Record the ProjectCurve relink totals as a finding (2.2).
