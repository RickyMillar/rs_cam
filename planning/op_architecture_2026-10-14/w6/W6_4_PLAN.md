# W6.4 — the composed-surface shape: implementation plan

Status: DESIGN ONLY (2026-10-15). No code changed. Branch `op-arch`.
Sources: `planning/op_architecture_2026-10-14/AUDIT.md` (shape table, "classifier →
kernel per region → one router/relink stage"), `PLAN.md` (W6 row), the working tree
with the W6.2 edits of the other agent (`compute/execute/territory.rs`,
`trace/produced.rs`, untracked). All line numbers are from that working tree. They
drift when W6.2 lands; the editor re-reads each site before it edits.

## 0. Verdict and the smallest honest version

The three ops do NOT share one pipeline today. They share three pieces:

| Piece | UnifiedFinish | SteepShallow | Pencil |
|---|---|---|---|
| Classifier | true surface, `cusp_quarter`, `decompose` → polygons | offset surface, `legacy_envelope_quarter`, `classify_steep_shallow` → bool grid | none (a detector) |
| Region scope | single-polygon `RegionSet` per region | two cell masks (`steep_expanded`, `shallow_expanded`) | none |
| Sub-kernels | `waterline_toolpath_with_cancel`, `scallop_…_with_cancel`, `raster_toolpath_from_grid` | private `generate_steep_passes_with_cancel`, `generate_shallow_passes_with_cancel` | `centerline_cut_paths` / dihedral / curvature arms |
| Link stage | `relink_fragments` per region + `route_greedy` + stitch | none (dressup TSP inside barriers) | `emit_paths_with_entry_stock_reported` (`PencilJunction`, `plan_link_lift`) |
| Order spans | `unified_finish_spans` → `region_node_barriers` | `steep_shallow_spans` → `region_node_barriers` | labelled events, no barrier |
| Rest | `Reference` (claims), ceiling | `None`, `NoLink` | `Reference` (RestDepth), ceiling |

So "one composed shape" is honest only as a thin trait over these pieces. The gain is
(a) one `order_constraints` hook that builds the barriers for every composed op,
(b) one rest DETECTOR stage that three call sites share, (c) named seams inside
UnifiedFinish (classify / per-region kernel / route) that tests can address.

**Smallest honest version (recommended): W6.4.0 – W6.4.4 below.** Every item is
byte-identical on every golden and sentry. Nothing changes a cut. W6.4.5 (swap
SteepShallow onto the W6.3 kernels) and W6.4.6 (SteepShallow rest) change output and
need operator rulings; they are listed, not scheduled.

## 1. The trait shape

```rust
/// W6.4: classifier → one sub-kernel per region → one route stage.
pub(crate) trait ComposedSurfaceOp {
    /// What the classifier gives each region.
    type Band: Copy + Eq + std::fmt::Debug;
    /// The per-region record the route stage reads (entry/exit, strips).
    type Report: RecordFindings;

    /// Classify the surface into regions. Reads `Prepared::territory()`
    /// (xy + mask + stock). Owns the op's own surface builder and
    /// resolution; there is no shared classifier.
    fn classify(&self, ctx: &ExecutionContext<'_>, prepared: &Prepared<'_>, cancel: &dyn CancelCheck)
        -> Result<Classified<Self::Band>, OperationError>;

    /// Cut one region. The runner polls the cancel BEFORE each call; the
    /// sub-kernel polls inside its own loops.
    fn cut_region(&self, region: &ClassifiedRegion<'_, Self::Band>, shared: &mut RegionMemo,
        report: &mut Self::Report, cancel: &dyn CancelCheck) -> Result<RegionFragment<Self::Band>, OperationError>;

    /// Order the fragments and stitch them. The default: classifier order,
    /// native links. UnifiedFinish overrides it (route_greedy + surface links).
    fn route(&self, fragments: Vec<RegionFragment<Self::Band>>, ctx: &ExecutionContext<'_>,
        report: &mut Self::Report, cancel: &dyn CancelCheck) -> Result<Routed, OperationError> { … }

    /// W6.4 NEW HOOK. The order constraints of the stitched path: one
    /// `RegionNode` per node. The runner turns them into spans through the
    /// ONE builder `compute::spans::region_node_barriers` (spans.rs:195).
    fn order_constraints(&self, routed: &Routed) -> Vec<RegionNode>;
}

pub(crate) enum RegionScope<'a> {
    /// A polygon region (UnifiedFinish: `decompose` output, one polygon).
    Polygons(RegionSet<'a>),
    /// A cell mask on the classifier grid (SteepShallow: a point is in the
    /// region when its cell is true). Converting this to polygons CHANGES
    /// output (contour run splits, raster run ends); keep both forms.
    Cells { grid: &'a SlopeMap, keep: &'a [bool] },
}
pub(crate) struct ClassifiedRegion<'a, B> { pub index: usize, pub band: B, pub scope: RegionScope<'a>,
    pub area_mm2: Option<f64> }
/// `A` = the op's annotation type: `ScallopRuntimeAnnotation` for
/// UnifiedFinish (§7.8), `()` for SteepShallow (it emits none).
pub(crate) struct RegionFragment<B, A> { pub region_index: usize, pub band: B, pub tp: Toolpath,
    pub anns: Vec<A>, pub depth_ordered: bool }
```

The trait carries `type Annotation;` and uses `RegionFragment<Self::Band,
Self::Annotation>`. The fragment holds no entry/exit/strip fields. `route()` derives
them with `strippable_preamble` (region_routing.rs:299) and `trailing_retracts` (320)
into today's private `RegionPath` (33-51). Both fns read only `tp`, so this is
byte-identical; it is where the code does it today (2449-2450).

`order_constraints` does NOT exist today (rg finds no `order_constraints`). Its
content exists twice: `unified_finish_spans` (`finish/unified_finish.rs:1303-1358`,
the `RegionNode` list at 1343-1354) and `steep_shallow_spans`
(`finish/steep_shallow.rs:624-669`, nodes at 653-667). Both call
`region_node_barriers`. The hook makes that the only route.

`run_composed::<O>(op, ctx, prepared)` owns: the cancel poll before `classify` and
before each `cut_region` (today `unified_finish.rs:1485, 1508, 1537, 1937`); one
`RegionMemo` (the shared 0° raster grid, today `shallow_grid` at 1926/2185-2201);
the route call; the spans (`order_constraints` → barriers, plus the op's own label
spans); `ctx.record(&report)`; and `generated.produced`.

Do NOT make `run_composed` generic over three ops at once in one step. Land it with
UnifiedFinish alone, then SteepShallow. Pencil does NOT implement this trait (see §4).

## 2. UnifiedFinish

Entry: `finish/unified_finish.rs:1469` `unified_finish_toolpath_with_cancel_and_ceiling`
(1187 lines in one fn). Adapter: `compute/execute/finish_3d.rs:540` `generate_unified_finish`.

### 2.1 Classifier = Steps 1 – 3.5

| Step | Lines | Becomes |
|---|---|---|
| 1 classify on the TRUE surface | 1500-1507 | `classify()`: `build_classification_surface_with_sampler_and_cancel` (finish_setup.rs:571) at `unified_finish_classification_resolution` (1372) |
| 2 coverage ∧ boundary | 1511-1536 | `classify()`: reads `territory.regions()` (W6.2 already passes it as `machining_boundary`, finish_3d.rs:688) |
| 2.5 claims detector + crease paths | 1543-1723 | `classify()` calls the DETECTOR stage (§5); the crease cut paths are a classifier OUTPUT (claim == emitted, 1645-1653) |
| 2.6 S4 territory mask-AND | 1750-1821 | `classify()`; a private fn `apply_territory_clip(&mut covered, grid, cfg)` |
| 3 decompose | 1836-1840 | `classify()`: `finish_planner::decompose` (finish_planner.rs:390) with `&[]` creases |
| 3.4 region-count guard | 1849-1859 | `classify()` |
| 3.5 crease node emission | 1867-1901 | `classify()` returns `crease_tp`; route appends it (see 2.3) |

`Classified` = `{ surface, covered, planned: PlannedRegions, crease_tp, claims_report, produced }`.

### 2.2 Sub-kernels = Step 4 arms (1936-2461)

Each `match region.band` arm becomes one named fn in a new sibling module
`finish/unified_finish/band_kernels.rs`, with the SAME arguments the arm reads now:

| Band | Lines | New fn | Kernel it calls | Scope |
|---|---|---|---|---|
| VerySteep | 1944-2033 | `cut_very_steep` | `ops::waterline::waterline_toolpath_with_cancel` (ops/waterline.rs:266) + `band_z_range` (region_routing.rs:511) + height-clip record (1965-1999) + `refine_band_chords(…, true)` | `Polygons` |
| MidSteep | 2034-2089 | `cut_mid_steep` | `scallop_toolpath_structured_annotated_with_cancel` (scallop.rs:1060) | `Polygons` |
| Shallow | 2090-2302 | `cut_shallow` | derate (2107-2128) + C2 frame (2135-2142) + `build_shallow_raster_grid` (region_routing.rs:75) + `raster_toolpath_from_grid` (toolpath.rs:607) / monotone cells (2217-2294) + `refine_band_chords(…, false)` | `Polygons` |

The per-region relink (2311-2375, `relink_fragments` with `link_ceiling`) and the
height-clip bookkeeping (2390-2436) stay INSIDE `cut_region`, after the band fn, in
this order. Reason: `strippable_preamble`/`trailing_retracts` (2449-2450) read the
relinked path. Moving the relink into `route()` reorders work and changes output.

### 2.3 Router = Steps 5 – 6 (2473-2633)

`route()` = `route_greedy` (region_routing.rs:343) when `link_kinematics` is `Some`,
else steep-first band-major (2488-2498); then the stitch loop (2524-2613) with the
region table; then the crease node appended LAST with a native link (2618-2633).

`order_constraints()` = the `RegionNode` list of `unified_finish_spans` (1343-1354):
`depth_ordered` only for `RegionKind::Band(VerySteep)`; the crease node has
`depth_ordered: false`. Keep `unified_finish_spans` as a pub fn that the hook calls;
`capability_link_moves_safety` and other sentries call it by name.

### 2.4 The ceiling

One ceiling: `op_link_ceiling` (`compute/execute/shared.rs:126`) → `link_ceiling_for`
(106). UnifiedFinish passes it to the intra-region relink (2347) and to the crease
`PencilParams::link_fallback_top_z` (1882). `route_greedy`'s inter-region surface link
reads NO ceiling at planning time: `choose_link` (region_routing.rs:420) calls
`build_surface_link` (surface_link.rs:36) with the mesh only (450-462). The comment
at 439-442 is about the retract DESCENT, not this link. The op-agnostic W1.1b
simulation triage (G-LINKPLOUGH) can flag such a link AFTER simulation, but its
fixture (`a_link_through_stock_is_flagged_g_linkplough`) covers a raster, a pencil
and 2D Adaptive, not a UnifiedFinish inter-region link. Do NOT fix it inside W6.4
(it changes `unified_finish/rest`); record it as backlog B16 (§11).

## 3. SteepShallow

Entry: `finish/steep_shallow.rs:708` `steep_shallow_toolpath_split_with_resolution`.
Adapter: `finish_3d.rs:757` `generate_steep_shallow`.

| Role | Today | Lines |
|---|---|---|
| Classifier | `cached_finish_surface` (maps/finish_surface_cache.rs:337) at `steep_shallow_generation_resolution` (555, `legacy_envelope_quarter`); `classify_steep_shallow` (surface/slope.rs:781); `dilate_grid` (85) / `erode_grid` (121) → `steep_expanded`, `shallow_expanded` | 748-791 |
| Sub-kernel steep | `generate_steep_passes_with_cancel` | 184-369 |
| Sub-kernel shallow | `generate_shallow_passes_with_cancel` | 415-538 |
| Router | concat, `steep_first` order | 837-856 |
| Order | `steep_shallow_spans` | 624-669 |

`territory.mask` already states `SurfaceMask::SteepShallowSplit { threshold_deg }`
(territory.rs:58). The classifier reads the threshold from it.

**The two SteepShallow emitters are NOT UnifiedFinish's kernels and NOT W6.3's.**
The steep pass has the 30° contour-majority filter (203, 222-237), a per-point keep
on the steep GRID (243-252), `split_runs` with `RunTopology::Closed` (264-269), the
PR-8d minimum-segment floor (309-316), `snap_to_bottom: false` (208-216) and
`z_bottom = min_z_or_bbox_floor()` (791). The shallow pass calls `batch_drop_cutter`
directly (433) and walks the zigzag by hand (437-536), with no `refine_band_chords`.
A swap to `waterline_toolpath_with_cancel` / `raster_toolpath_from_grid` changes
output. So:

- **W6.4.3 (byte-identical):** SteepShallow implements `ComposedSurfaceOp` with its
  two PRIVATE emitters as sub-kernels and `RegionScope::Cells`. Two regions, fixed
  order. `route()` = the default (concat). `order_constraints()` = the two nodes.
- **W6.4.5 (output change, NOT recommended now):** swap the emitters for the W6.3
  kernels. Blocked by the PR-8c written deferral (555-570: classification is 10.6–
  13.3 % wrong on this grid and a finer grid costs 2.9–7.6× for no measured delta)
  and by `steep_shallow_min_segment_pr8d`. Needs a ruling and a re-bless.

## 4. Pencil: a detector stage, not a composed op

Pencil has no classifier and no per-region kernel. It is
`detect → chain/offset paths → order_paths_nearest → emit with its own link stage`
(`finish/pencil.rs:658-733`). Give it a `Detector` STAGE, not the composed trait.

| Role | Today | File:line |
|---|---|---|
| Detector dispatch | `match params.detector` | pencil.rs:675-692 |
| RestDepth arm | `rest_depth_arm` | pencil/detectors.rs:213-347 |
| Dihedral / Curvature arms | `dihedral_arm`, `curvature_arm` | detectors.rs:350, 133 |
| Path ordering | `order_paths_nearest` | pencil.rs:422 |
| Link stage | `emit_paths_with_entry_stock_reported` (`PencilJunction`, `plan_link_lift`) | pencil/emission.rs:672, 394, 490 |

Do NOT route pencil links through `relink_fragments` in W6.4. The pencil link lift
and the entry ramp (G-ENTRYLOAD, G-LINKLOAD, G-PENCILHOP) have their own sentries
and their own ceiling input (`link_fallback_top_z`, finish_3d.rs:479).

**Decision: Pencil stays on its own adapter. The Detector stage (§5) is the whole
W6.4 scope for Pencil.** Pencil does NOT implement the W6.3 height-field `SurfaceOp`
(`plan(HeightQuery, Territory)`). Three facts make it a poor fit:

1. Pencil reads no XY territory. Its row states `territory_xy: ClipAfter`
   (registry.rs, Pencil row); the session clips the result after generation.
2. The detector reads `territory.reference_stock()` (finish_3d.rs:504) and builds a
   rest FIELD before any height query runs. The output of the detector is a path
   list (`Vec<PencilPath>`), not a territory.
3. The "plan" half is `paths_from_sampled` (chain_paths.rs:269) /
   `centerline_cut_paths` (crease_paths.rs) + `order_paths_nearest` (pencil.rs:422) +
   the pencil emitter. The emitter is also the link stage. A W6.3 `plan` that takes a
   territory and returns paths has no slot for a path-list input or an own link stage.

If a later wave wants Pencil on a shared trait, the requirement on W6.3 is: a
`plan` that accepts a pre-computed path list (from a detector) and an op-owned link
stage. W6.4 does not ask W6.3 for that.

What W6.4 DOES give Pencil: the shared `RestDetector` call (§5) in `rest_depth_arm`,
and a named `PencilDetectorOutput { paths, produced, tip_float }` that the dispatch at
pencil.rs:675-692 returns for all three arms. The `Produced` item (rest grid + rest
regions) is the item other ops consume through `DerivedRestRegions` (§5).

## 5. The rest DETECTOR stage and the `Produced` item

Three sites build `RestFieldParams`, pick a `RestReference`, call
`surface::rest_field::detect_rest_valleys`, and attach `rest_grid` + `rest_regions`:

| Site | Lines | Reference policy (KEEP each) |
|---|---|---|
| Pencil `rest_depth_arm` | detectors.rs:213-347 | territory stock → real reference tool → nominal ball → self-probe (sign flip) |
| UnifiedFinish claims | unified_finish.rs:1580-1627, 1721-1722 | `CreaseReference::SelfProbe` (default) or `MachinedStock`; own `CLAIMS_OFFSET_PASS_CAP` and policy stepover |
| Generic rest analysis | compute/execute/dressup_apply.rs:36-128 | territory stock → reference tool → self-probe (NO nominal tier); records `DerivedStepoverFinding`, `region_cap` |

W6.4.1 adds `surface::rest_field::RestDetector` (or `finish/rest_detector.rs`):

```rust
pub(crate) struct RestDetection { pub field: RestFieldResult /* centerlines, report, region_cap */,
    pub produced: Produced /* rest_grid + rest_regions */ }
pub(crate) fn detect(mesh, index, cutter, reference: RestReference<'_>, params: &RestFieldParams)
    -> RestDetection;
```

Each caller keeps its own reference choice and params. The stage removes only the
copied "call, then wrap two `Arc`s into `Produced`" tail. Merge the PLUMBING, not the
POLICY; the three policies differ on purpose (the surface/CLAUDE.md "valley-reach
policy is canonical" rule covers only the stepover, which already goes through
`surface::reach::suggested_offset_stepover_mm` at all three sites).

The CONSUMER path already exists. Do not add new plumbing:

- `Produced::rest_regions` → `BoundarySource::DerivedRestRegions`
  (`session/compute/generation.rs:567`);
- the dependency edge `EdgeKind::Regions` (`session/dependencies.rs:244`);
- one-source precedence: `Produced::has_no_rest_output` (`trace/produced.rs`) read
  at `compute/execute.rs:606`.

## 6. Claims pipeline and multi-tool tiers

- Claims stay INSIDE UnifiedFinish's classifier. The claims pipeline is `pencil_claims`
  (default `false`, operation_configs.rs:2045), `territory_clip` (default `false`,
  2074), `crease_reference` resolved in the adapter (finish_3d.rs:593-600). The adapter
  keeps building `ClaimsConfig` (finish_3d.rs:569-649) from `territory.reference_stock()`.
- The crease node stays appended AFTER the routed bands with a native link
  (1615-1633). Threading it through `route_greedy` is the deferred S3 "fused router";
  W6.4 does not do it.
- Multi-tool tiers are an op CHAIN of k UnifiedFinish ops (`session/multitool.rs`
  module doc, `tier_boundary` at 1099). Each tier reads its tier regions as
  `BoundarySource::PlannedTierRegions` → `Territory.xy = Regions`. W6.4 changes nothing
  there if `classify()` reads `territory.regions()` at Step 2. The G-TIERCONTINUOUS
  predicate (multitool.rs:1076) couples to `monotone_cell_decomposition`; keep that
  field name and meaning.

## 7. Do NOT move (risk is higher than payoff)

1. The claims pipeline out of UnifiedFinish, or into a separate op (T3 §1: one-op
   precedent cost +22.5 % for zero gain; multitool.rs:13-17).
2. The crease node into `route_greedy` (S3, deferred on record).
3. The per-region relink out of `cut_region` into `route()` (see 2.2).
4. The shared 0° raster memo rule (2185-2201: unwindowed, only for non-rotated,
   non-derated regions).
5. The two classifiers into one (UnifiedFinish true surface `cusp_quarter`, §14q;
   SteepShallow offset surface `legacy_envelope_quarter`, PR-8c).
6. `RegionSpanRole::Node` vs `GeneratorPass` id spaces and the node range that OPENS
   before the incoming surface link (2551-2569, the TSP span-tiling fix).
7. SteepShallow emitters onto W6.3 kernels (W6.4.5) and pencil links onto
   `relink_fragments`.
8. The `ScallopRuntimeAnnotation` type as UnifiedFinish's annotation channel
   (C8 FLAT grouping, finish_3d.rs:719-728).

UnifiedFinish is large and delicate. The plan EXTRACTS functions in place (same
inputs, same order, same file or a sibling module) and does not rewrite logic.

## 8. W6.3 kernels as sub-kernels: what W6.4 needs

The sibling plan was not visible. These are REQUIREMENTS on W6.3, not guessed names.

| W6.3 op kernel | Used by | Adapter W6.4 needs | Requirement on W6.3 |
|---|---|---|---|
| Scallop | Unified MidSteep | single-polygon `RegionSet`; slope window `SLOPE_FILTER_MIN_DEG..MAX_DEG`; `continuous: true`; `intra_pass_hookup_mm: 0.0`; `link_kinematics: None` (2035-2063) | keep `scallop_toolpath_structured_annotated_with_cancel(…, Option<&RegionSet>, cancel)` callable with a region scope that is NOT the op territory; return `ScallopReport` (uncut core, untouched, standing: 2083-2087) |
| DropCutter (raster) | Unified Shallow | per-region stepover (cos derate), direction (C2 frame), sampling window; a whole-mesh lattice shared across regions | expose grid BUILD apart from row EMIT: `(stepover, direction_deg, window) → DropCutterGrid` with the same origin/phase as `batch_drop_cutter`. If W6.3 does not split them, Unified keeps `build_shallow_raster_grid` and only the emit is shared |
| Waterline (W6.1 Layered) | Unified VerySteep | per-region Z range from `band_z_range`; heights clamp + height-clip finding | none. Unified calls the KERNEL `waterline_toolpath_with_cancel`, not `run_layered`. W6.3 must not fold that fn into the Layered runner |
| Horizontal / Radial / Spiral / Ramp / VCarve | none | — | none |

`RegionScope::Cells` has NO W6.3 consumer. Only SteepShallow's private emitters read it.

## 9. Proof of "no change": what covers what

### 9.1 G-OPGOLDEN coverage (checked)

`tests/every_op_keeps_its_toolpath_g_opgolden.rs` builds each case from
`OperationConfig::new_default` (line 215). The session always sets
`link_kinematics` (`session/compute.rs:2376`), so `route_greedy` runs. Defaults:

| Case | Covers | Does NOT cover |
|---|---|---|
| `unified_finish/fresh`, `/rest` | Steps 1-6, C2 cells (on), intra-region relink (6.0 mm on), ceiling on rest | claims (`pencil_claims` off), S4 `territory_clip`, `MachinedStock`, crease node |
| `steep_shallow/fresh` | both halves | rest (no rest arm: `rest_use: None`) |
| `pencil/fresh`, `/rest` | Dihedral detector (default, pencil.rs:77) | RestDepth arm, Curvature arm, the detector stage |

The editor prints the region count and band mix of `unified_finish/fresh` once. If
the fixture has fewer than two regions, `route_greedy` is only nominally covered.

### 9.2 W6.4.0: new golden arms, blessed BEFORE any refactor (W0 pattern)

Add variants to `cases()` (test line 176). Bless only the new keys.

- `unified_finish/claims` — `pencil_claims = true` (fresh).
- `unified_finish/claims_rest_clip` — rest, `pencil_claims`, `claims_reference = MachinedStock`, `territory_clip = true`.
- `unified_finish/no_c2_no_hookup` — `monotone_cell_decomposition = false`, `intra_region_hookup_mm = 0.0`.
- `pencil/rest_depth` and `pencil/rest_depth_rest` — `detector = RestDepth`.
- `pencil/curvature` — `detector = Curvature`.
- Optional: a `produced` digest per case (rest_grid cell count + hash, rest_regions
  count + area) so the detector stage is proved, not only the moves.

### 9.3 Sentries per op (rg over `tests/`, non-heavy, non-ignored unless noted)

Run per item. Each is a separate `--test` binary; all are fast unless marked.

UnifiedFinish — structure and route:
`every_op_keeps_its_toolpath_g_opgolden`, `capability_link_moves_safety` (17),
`narrate_regions_closed_c8`, `retract_trip_channel_am7`, `island_stay_down_links_o3`
(ceiling arm vs fresh arm equality), `link_counters_visible_g_linkvisible`,
`disconnected_finish_retract_structure_p0`, `one_link_ceiling_for_every_op_g_linkceil`,
`retract_intent_move_type_census_w6`.

UnifiedFinish — bands and kernels:
`monotone_cell_decomposition_c2` (7), `shallow_raster_slope_derate` (1 run, 1 ignored),
`fed_chords_ride_the_surface_g_tierburial`, `unified_finish_dropped_band_finding_d1`,
`unified_finish_partial_clip_finding_c8`, `unified_finish_planner_dials_f2`,
`unified_finish_tapered_end_to_end_m21` (8 + 1 ignored), `finish_resolution_policy_pr3`,
`classification_strategy_m3`, `shallow_band_stock_to_leave_exhibit_d16_2`.

UnifiedFinish — claims:
`claims_reference_cascade_am6` (8), `crease_own_region_pr6b`, `per_point_claims_fan_c9`,
`zero_removal_rest_pass_a4`, `inert_claims_dial_f4`, `derived_stepover_pr6a`,
`coverage_routing_pr5`, `rest_region_tip_radius_dilation_f1`.

Multi-tool tiers:
`multitool_plan_emission_o1`, `multitool_plan_roundtrip_o1b`,
`planned_tier_regions_boundary_o2`, `tier_fine_burial_sources_g_tierburial`,
`tier_ops_fit_arcs_within_half_the_cusp_f4a`.

SteepShallow:
`steep_shallow_min_segment_pr8d` (5), `capability_link_moves_safety`,
`waterline_shared_finish_setup_c3`, `finish_surface_cache`,
`grid_z_uncovered_contract_c2` (`--features test-support`), `--lib finish::steep_shallow` (13).

Pencil and the detector stage:
`pencil_entry_ramp_g_entryload`, `pencil_surface_link_g_linkload`,
`pencil_tip_float_channel_d1`, `pencil_hop_dial_g_pencilhop`,
`a_link_through_stock_is_flagged_g_linkplough`, `an_entry_ramp_follows_the_arc_g_rampchord`,
`a_rest_entry_over_cleared_stock_takes_no_ramp_r2`, `a_rest_op_slower_than_fresh_is_told_r3`,
`checkpoint_a_valley_matrix`, `coverage_routing_pr5`, `generic_rest_routing_pr7`,
`rest_routing_probe_e9`, `--lib finish::pencil` (33 fns), `--lib session::compute`
(`derived_rest_regions_*`, tests.rs:2181-2273), `dependency_edges_are_the_walker_dep1`.

Framework: `every_op_conforms_w4` (~8 s), `every_op_states_its_capabilities_w2_4`,
`--lib cancellable_families_honour_a_preset_cancel_flag`, `--lib every_generator_polls`,
`--lib finish::unified_finish` (36 fns), `--lib compute::execute` (includes
`unified_finish_ring_collapse_g_unifiedcrash`, `unified_finish_semantic_regions`).

NOT in the local loop (heavy, operator ruling "no large gates"):
`checkpoint_b_resolution_ab` (86 s), `air_cut_family_calibration_w5bf4` (62 s),
`finish_planner_wanaka_diagnostics`; the ignored `scallop_solo_vs_unified_s1`,
`band_cell_ownership_g2` arms; `p1_headless_ab_wanaka`.

Each item ends with: the item's sentries, `cargo clippy` (full line), `cargo fmt
--all -- --check`, all through `timeout <s> scripts/cargo_lane.sh`.

## 10. Work items (one editor at a time; agents never commit)

| Item | Work | Output change | Proof |
|---|---|---|---|
| W6.4.0 | New golden arms (§9.2), blessed first | none (new keys only) | lead approves the new keys by path |
| W6.4.1 | `RestDetector` stage; three sites call it; policies unchanged | none | golden incl. new pencil/claims arms + produced digest; claims and pencil sentries; `generic_rest_routing_pr7`; `--lib session::compute` |
| W6.4.2 | `order_constraints` hook + `RegionNode` from both ops through `region_node_barriers` only | none | `capability_link_moves_safety`, `narrate_regions_closed_c8`, `retract_trip_channel_am7`, golden |
| W6.4.3a | UnifiedFinish: extract `classify` (Steps 1-3.5), the three `cut_*` band fns (new `unified_finish/band_kernels.rs`), `route` (Steps 5-6); the 1187-line fn becomes a 60-line sequence | none | all Unified sentries (§9.3) + golden (6 Unified keys) |
| W6.4.3b | `ComposedSurfaceOp` + `run_composed`; UnifiedFinish implements it; adapter `generate_unified_finish` calls `run_composed` | none | same + `every_op_conforms_w4`, cancel tests |
| W6.4.4 | SteepShallow implements it with its private emitters and `RegionScope::Cells` | none | SteepShallow sentries + golden |
| W6.4.5 | (ruling) SteepShallow emitters → W6.3 kernels | YES | re-bless `steep_shallow/*` only; PR-8d; a rendered surface |
| W6.4.6 | (ruling) SteepShallow `rest_use: Reference/LinkSafetyOnly` + `link_ceiling` | YES on rest | new `steep_shallow/rest` arm first; W2.4 table update |

Order: W6.4.0 → W6.4.1 → W6.4.2 → W6.4.3a → W6.4.3b → W6.4.4. W6.4.1 and W6.4.2 are
independent. W6.4.3a is the large one; split it into three commits (classify, band
fns, route) with the golden green after each.

Folder docs to update: `finish/CLAUDE.md` (file map: `band_kernels.rs`, the detector
stage; invariant: "a composed op builds its barriers only through
`order_constraints`"), `compute/CLAUDE.md` (W6.4 line beside W3/W6.2).

## 11. Rulings for the operator

- R-W6.4a: SteepShallow onto W6.3 kernels (W6.4.5). Recommendation: NO now.
- R-W6.4b: SteepShallow rest (W6.4.6). It has `rest_use: None` and `NoLink` today, so
  on `FromRemainingStock` it cuts the whole surface again. Recommendation: a new rest
  arm and a measurement before a decision.
- B16 (new backlog): `route_greedy`'s inter-region surface link reads no link ceiling
  at planning (`choose_link`, region_routing.rs:420-462). On rest stock it can ride
  the mesh through standing material. The W1.1b triage can flag it after simulation;
  it does not lift or refuse it. First add a UnifiedFinish rest arm to
  `a_link_through_stock_is_flagged_g_linkplough` and measure. Then decide: pass the
  op ceiling into `choose_link` (re-bless `unified_finish/rest`) or keep the flag.

## 12. Risks

- Line drift: W6.2 is mid-edit in `finish_3d.rs`, `unified_finish.rs`,
  `pencil/detectors.rs`. Start W6.4.1+ only after W6.2 is committed.
- Extraction can change float order (a sum moved across a fn boundary, an iterator
  re-chained). The golden hash catches it; keep each accumulation in its old order
  (`uncut_core_mm2`, `untouched_mm2`, `standing_mm2`, `RelinkTotals::add`).
- `#[allow(clippy::too_many_arguments)]` grows on the band fns. Pass one
  `BandCtx<'_>` struct (mesh, index, cutter, params, surface, covered, top_z,
  bottom_z, cancel) instead; it needs no `SAFETY:` allow.
