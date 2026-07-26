# Unified Finish v3 — design: claims pipeline + fused rest router

Status: DRAFT 2026-07-09 (Fable, from the v3 research pass). Parent:
`planning/unified_v3_design_prompt.md` (vision + why-the-drift), grandparent
`planning/unified_finish_planner_design.md` (P2 design — much of v3 was in it
and never built). Branch `experiment/adaptive-spiral`.

Foundations status (both tails CLOSED 2026-07-09 late):

- **Instrument**: the three-way probe (`p2g_three_way_probe`) proved the
  COLUMNS instrument trustworthy — sim column tops equal an isolated
  re-stamp of the as-read toolpath EXACTLY (0.0 µm over 159 792 columns),
  the stored toolpath is not mutated post-sim, and the B75-vs-D band gap
  is REAL machined geometry (mid-steep median +1.9 µm, |d|>10 µm on 41 %
  of columns). The earlier "envelopes equal" counter-evidence was a
  cross-run comparison of different regenerations. v3 A/Bs may trust
  group-filtered FIDELITY-COLUMNS.
- **The gap's mechanism (same night, offline)**: 97 % of B75's excess is
  raster-owned FLATS mislabeled mid-steep by the verdict's dilated band
  map — B75 delivers its raster dial there (0.3 mm → 22.5 µm cusp,
  deliberate A-parity) while D's over-dense rings (0.171 mm, min-stepover
  driven by the whole model's steepest points) leave 7.3 µm. Where rings
  run, B75 == D; on textured steeps BOTH sit at ~5× dial (the true
  frontier). Three design inputs fall out, folded into §2.2/§2.4/§4
  below: per-territory cusp-consistent dials, Region-span attribution as
  a MUST, and texture-on-steeps as the shared quality ceiling.
- **TP15 collisions** — REVISED 2026-07-13: two real classes. (1)
  Staleness (chain edits keeping downstream rest results) — fixed by
  `invalidate_result_chain` + sentries. (2) The one actually observed
  live: **descent-ceiling resolution asymmetry** — `optimize_entry_descents`
  measures its ceiling on the 0.5 mm generation-time snapshot; the GUI
  verifies at auto 0.1 mm where thin ridge tops survive; entry descents
  end ≤~0.3 mm from fine-grid tops at uncut columns. Repro sweep:
  0 @ 0.5 / 15 @ 0.25 / 20 @ 0.1 mm (`p2g_live_v2_collision_repro`).
  OPEN FIX: pad descent targets by ≥ the snapshot cell size. The router
  design below adopts the generalized rule: **link/descent safety checks
  must be resolution-honest** — verified against at least the finest
  resolution the project will simulate at, or padded by the measuring
  grid's cell.

## 0.a THE END GOAL, REFRAMED (user, 2026-07-13) — prove the PROCESS

This is a theoretical/process-proving project, not a wanaka-production
project. Ball size is a free parameter (Ø0.5 if we like); the model may
be SCALED (×2 is fine) to put tool-vs-texture ratios wherever the
process needs them. The thing to prove:

> **A ball finishing pass that covers most of the terrain, followed by
> ONE unified rest-clearing pass (mixed strategies over the rest
> islands: shallow patches, steep faces, creases), beats an all-over
> small-tool finish on time at equal COLUMNS quality — with Region
> spans proving each strategy cut its own territory.**

The PROCESS-PROOF experiment (supersedes the S-ordering below where they
conflict; everything not needed for this is deferred):

1. **Fixture: wanaka ×2** (scale the mesh in the harness — same terrain,
   friendlier tool/texture ratio; measured basis: at scale 1 every ball
   Ø2–6 leaves 87–96 % of the mid-steep band as rest AREA, so no ball
   "manages most of the terrain" there — at ×2 a Ø2–3 ball behaves like
   a Ø1–1.5 at scale 1 and real rest ISLANDS appear). Wall fixture stays
   queued for the contour question but is NOT needed for the proof.
2. **Op A**: ball all-over scallop (size = sweep parameter, start Ø3).
3. **Op B**: unified op in REST-CLEARER mode against Op A's machined
   stock: region-LEVEL territory (drop whole conditioned islands whose
   measured rest share is below the dial — never cell holes), creases
   from the STOCK-REFERENCED rest detector (honest against a
   ball-finished reference — the R2-validated configuration; the
   "claims are geometric" rule was for rough chains), emitted additively.
4. **Score**: cascade (A+B) total vs all-over-tip baseline (D) at equal
   effective cusp: total time, group-filtered COLUMNS (on-size + tails),
   collisions 0, and the Region-span table showing the mix actually
   fired (raster/scallop/pencil each owning territory).
5. **Then**: ball-size sweep (Ø2/3/4) for the "optimal ball" curve.

Needed to run it (the ruthless list): region-level territory filter
(decompose-then-filter on the existing telemetry), stock-referenced
crease claims re-enabled for finish-quality references, the scaled-mesh
fixture, and the cascade A/B harness. Contour/wall fixture, GUI dials,
router (S3), carving — all deferred; not blockers for the proof.

## 0. The vision, restated as architecture

Two ops, one cascade:

- **Op A — Ø2 ball bulk finish**: all-over scallop (or advisor-chosen
  pattern). Removes most VOLUME even where rest AREA stays high (dendritic
  flanks: the ball floats over concavities but still cuts the convex mass).
- **Op B — fused rest finish** (the v3 op): ONE toolpath, one small tool
  (Ø1 tapered tip), three strategies with a claims pipeline:
  1. rest analysis on Op A's **machined stock**,
  2. **pencil claims creases FIRST**, corridors subtracted from the map,
  3. remaining islands classified **per-island** (wall-like → contour,
     open → scallop),
  4. everything routed as **one segment graph** minimizing retracts.

P2's UnifiedFinish (raster+scallop band decomposition, one tool) remains as
the shipped op; v3 extends the same orchestrator skeleton rather than
forking a parallel flow (architecture guardrail: extend core+worker+UI wiring).

## 1. What already exists (research pass, 2026-07-09)

The striking finding: **every leaf primitive for the claims pipeline already
exists**; what's missing is the wiring. Inventory with pointers:

### Planner (`finish_planner.rs`)
- `decompose(slope_map, covered, creases, tool_radius, params)` — 7-step
  conditioned banding (hysteresis flood, morph close, three-way label grid,
  min-area absorption, crease corridors, per-band `region_polygons_from_mask`
  with `overlap_mm` dilation). **Already accepts `creases: &[RestCenterline]`**
  — `unified_finish.rs:211` passes `&[]` ("crease routing folds in later").
- `apply_crease_corridor` (`finish_planner.rs:657`) is the claim-then-subtract
  TEMPLATE: wide creases (`half_width ≥ corridor_k·r`) get an `own_region`
  corridor polygon (points→mask→`region_polygons_from_mask` dilated by
  half_width) AND their cells carved out of the band label grid
  (distance-transform ≤ half_width → label `None`) BEFORE polygon extraction.
  v3 generalizes this to ALL claimed creases, not only canyon-width ones.
- `PlannedRegion` is `{ band, polygon }` ONLY — no smoothness, no vertical
  extent, no area. Curvatures + normals exist on `SlopeMap` but are never
  consulted; `band_z_range` (vertical extent) is computed only after banding,
  only for waterline ladders (`unified_finish.rs:783`).

### Pencil / rest field (`rest_field.rs`, `crease_paths.rs`)
- `detect_rest_valleys(mesh, index, pencil, reference, params)` — the R2
  ridge extractor (NMS + prominence + hysteresis + thin + trace +
  per-branch-median gate). Output `RestCenterline { points, half_width_mm }`
  — **polylines + scalar width, NOT corridor polygons** (gap, closed by
  §2.1 step 3).
- `RestReference::Stock(&TriDexelStock)` — machined-stock reference; prior
  stock arrives via `prior_stocks[op_id]` when `StockSource::FromRemainingStock`
  (R2 validation: analytic Ø-reference overestimates ~14× when the finish
  reuses the tool family — machined-stock reference is v3's default).
- `crease_paths::centerline_cut_paths` — the emission tail, factored out of
  pencil EXPLICITLY "so the P2 unified-finish planner's crease pass can reuse
  it". Sizes offset passes by per-centerline half-width.
- Generic rest analysis (`attach_generic_rest_analysis`, `execute.rs:1954`)
  is POST-generation attach for downstream `DerivedRestRegions` boundary
  clipping — cross-op and post-hoc. **No op today computes rest mid-generation
  and routes its own sub-strategies off it** (gap, see §2.1).

### Orchestrator + router (`unified_finish.rs`)
- Per-region generation via single-polygon `RegionSet`; band → strategy match
  at `:232`; `RegionPath { region_index, band, tp, head_strip/entry,
  tail_strip/exit }` = routable node with strippable preamble metadata.
- `route_greedy` + `choose_link`: greedy nearest-by-integrated-link-time
  (F-034 integrator, never distance/feed), surface-link vs retract-link
  costing via `build_surface_link` (gouge-checked, boundary-confined),
  2-opt deliberately deferred pending A/B evidence.
- Serpentine stay-down lives in `raster_toolpath_from_grid` (deferred
  retract, `link_max = hypot(x_step,y_step)·1.05` one-diagonal guard).
- `SpanKind::Region` + `SpanPayload::Region { region_id }` EXIST in
  `toolpath_spans.rs` but the unified op emits only scallop-event spans;
  per-region provenance lives in `UnifiedFinishReport` only (gap, see §2.4).

### Polygon booleans
- `Polygon2::difference/intersection/union/union_all` all present
  (`polygon.rs:216,206,189,232`, i_overlay-backed). `RegionSet` lacks a
  `subtract` — small wrapper gap.

### Link kinematics / retract safety (`machine_kinematics.rs`, `unified_finish.rs`, `dressup.rs`, `collision.rs`)
- `LinkKinematics { kinematics, max_feed_mm_min, rapid_feed_mm_min }`
  (`machine_kinematics.rs:327`) with the two candidate costs:
  `retract_link_time` (synthesizes lift→traverse→descend toolpath,
  integrates it) and `surface_link_time` (integrates a feed along a
  sampled surface-follow path). Consumers: pencil `emit_paths`
  (`pencil.rs:854-900`, keep surface link iff `surface_t ≤ retract_t`) and
  the unified `route_greedy`/`choose_link` — **already a strategy-agnostic
  cross-region router prototype**. Built once per toolpath in
  `session/compute.rs:1388` from the real machine profile.
- Region generators floor ALL their rapids at
  `effective_safe_z = max(retract_z, stock_top + 5 mm)`; surface-link
  junctions are FEED moves riding `cl.z + stock_to_leave`
  (`surface_link.rs:47`) — invisible to the rapid-collision checker.
- The only post-passes that can put a RAPID below stock top:
  `optimize_entry_descents` (`dressup.rs:238-249`, inserts descent to
  `max_top_z_in_disc + 2.0 mm` — **19.996 = ceiling 17.996 + 2.0**, prime
  TP15 suspect) and `apply_link_moves` keep-down (feeds, checker-exempt).
- `check_rapid_collisions_against_stock` (`collision.rs:450-544`) samples
  rapids against the FROZEN pre-toolpath dexel; carve-outs are same-XY
  only (vertical retract; F3 same-XY descent re-entry). An inter-region
  descent at a DIFFERENT XY over area this same op already cut is
  structurally a false positive; a crossing of never-cut stock is real —
  the checker cannot distinguish (no intra-toolpath cut mask).
- Existing ordering/graph pieces reusable for the fused router: unified
  `route_greedy` + `RegionPath` strip metadata; `tsp.rs`
  `optimize_rapid_order` (2-opt, safe-z interstitials, F-038b diagonal
  guard, `RapidOrderBarrier` spans); adaptive3d `nearest_neighbor_order` +
  `try_emit_stay_down_link` (`max_stay_down_distance_mm`, clearance-lifted
  link Z); dressup `apply_link_moves` (3-move collapse, barrier-aware).
- Safety invariants pinned by: `capability_link_moves_safety.rs`,
  `adaptive3d_keep_down_link_f038b.rs` (no Linking feed through uncut
  stock), `adaptive3d_post_tsp_z_monotonicity.rs`,
  `dressup_span_invariants.rs`, and the harness gate
  `BASELINE_RAPID_COLLISIONS = 4`.

## 2. Design

### 2.1 Claims pipeline (inside Op B generation)

Order of operations inside `unified_finish_toolpath_with_cancel` (extended):

1. **Classification surface** as today (true-surface probe, max-gradient
   slope map).
2. **Rest analysis first-class** — AMENDED by the S1 A/B failure
   (2026-07-09/13): **claims are GEOMETRIC, territory is MATERIAL.**
   Crease detection ALWAYS runs against the analytic self-probe
   (design-surface valleys — what pencil corridors are for); on a
   rough→finish chain a stock-referenced detector reads roughing
   TERRACES as a dendritic phantom crease network (measured: 10 k new
   uncut mid-steep columns when those phantoms claimed corridors). The
   machined stock feeds ONLY the territory mask, computed directly as
   `stock_top − pencil_drop` from the analytic run's own drop field —
   one detector pass serves both. Untrusted (NaN) samples KEEP coverage
   (the detector's boundary-erosion rim must never amputate band area).
   The R2 "prefer machined-stock reference" lesson still holds for the
   pencil's own CUT targets — that refinement is S3 scope, distinct
   from claiming.
3. **Pencil claims**: feed centerlines into `decompose`'s existing
   `creases` param — but ONLY centerlines that survive the emission
   gates (S1 lesson: claim-carve-abandon — a corridor carved from a band
   whose paths the emitter then length-gates away is leftover nobody
   owns; pre-apply the same gate before decompose). Extend
   `apply_crease_corridor`:
   - Claim corridors for **every** pencil-routed crease (today only
     canyon-width ones carve). Narrow-crease corridor = centerline
     buffered by `max(half_width_mm, pencil_footprint)` where
     `pencil_footprint` = offset-pass fan width `centerline_cut_paths`
     will actually cut (passes × stepover, capped by `num_offset_passes`).
   - Carve claimed cells from the label grid exactly as today (the
     mechanism is already correct: `overlap_mm` dilation at extraction
     deliberately reaches back over claims so band passes blend into
     corridor territory without owning it).
   - Keep `corridor_k` promotion: wider than `k·r_tool` → own clearing
     region (routed to scallop, not pencil).
4. **Territory = rest islands, not whole-surface bands**: intersect band
   label grid with the rest mask (cells where Op A left material above
   `min_rest_depth`). On smooth terrain this shrinks Op B to near-nothing
   (correct — Op A already finished it); on wanaka flanks it keeps the
   87–96 % rest share honestly. Implementation: AND the rest mask into
   `covered` before decompose Step 3 (one line; the conditioning pipeline
   then does its normal job on the intersected mask). NOTE: rest AREA vs
   VOLUME — the mask thresholds on rest DEPTH (`min_valley_depth` analog,
   separate dial `min_rest_depth_mm`, default ~ Op A cusp height × 2) so
   sub-cusp residue doesn't summon the tip tool everywhere.
5. **Per-island classification** (§2.2) on the remaining islands.
6. **Generation** per island/corridor: pencil via `centerline_cut_paths`,
   wall-like via waterline/contour, open via scallop — all single-polygon
   `RegionSet` calls as today.
7. **Routing** (§2.3) across ALL emitted paths (pencil corridors included).

### 2.2 Per-island classification rule

Replace the per-cell three-way label (slope only) with island-level
attributes computed after connected components (`banded_components` hook):

```
PlannedRegion {
    band: FinishBand,          // provisional, from slope masks as today
    polygon: Polygon2,
    stats: IslandStats {       // NEW
        area_mm2: f64,
        slope_p50_deg: f64,    // median of slope_map.angles over cells
        slope_p90_deg: f64,
        curv_p90_abs: f64,     // smoothness: p90 |curvature| over cells
        z_extent_mm: f64,      // max−min heightmap z over cells
    },
}
```

Routing rule (dials on the config, defaults from the sweep harness):

- **wall-like → contour/waterline** iff
  `slope_p50 ≥ steep_threshold_deg` AND
  `curv_p90_abs ≤ wall_smoothness_max` AND
  `z_extent ≥ wall_min_z_extent` (default `3 × z_step` — a Z-ladder must
  amortize its per-level entry cost, wanaka's 1 606 s lesson).
- **else → scallop** (including steep-but-textured — the wanaka lesson:
  slope-only routed dendritic flanks to contour where it loses badly).
- Shallow islands stay raster/scallop per current B75 evidence.

This preserves the shipped op's behavior when the new dials are wide open
(wall_smoothness_max = ∞ reduces to slope-only), so the A/B can isolate the
rule's effect.

**Per-territory cusp consistency** (from the +.05-tail RCA): every
strategy an island routes to derives its stepover from the SAME cusp
dial and the island's OWN geometry — raster stepover = `√(8·R_tip·h)`
when the island is finish-quality territory (0.3 mm "A-parity" leaves a
22.5 µm cusp on the Ø1 tip vs the 0.011 dial), and scallop's
min-stepover computed over the ISLAND's samples, not the whole model
(D's global min over-delivers ~2.6× on flats and pays generation + cut
time for it). Uniform-cusp is the default; per-band override dials stay
for the speed tier.

### 2.3 Fused router

Every emitted path — pencil corridor path, contour ring-stack, scallop
region, raster segment-run — becomes a `RegionPath` node (the struct
already carries the needed strip metadata: `head_strip`/`entry`,
`tail_strip`/`exit`); `route_greedy` + `choose_link` extend across
strategy kinds unchanged in shape (cost = integrated link time via
`retract_link_time`/`surface_link_time`, surface links gouge-checked and
boundary-confined, ties to surface). Pencil corridors are natural
"highways" — a crease often connects two islands, so corridor ENDPOINTS
enter the graph as first-class junction candidates (entry/exit at either
end of the corridor path) rather than pencil being an appended op.
2-opt stays deferred behind the same A/B evidence bar as P2.d.

Additions over today's router:

- **Node granularity**: contour ring-stacks and scallop regions stay one
  node each (their internal order is the strategy's business); pencil
  corridors may expose BOTH endpoints as entries (reversible paths) —
  cheap way to let the greedy pass thread corridors between islands.
- **Link-safety invariant** (from the TP15 class): any candidate link that
  is not a verified surface link travels at `effective_safe_z`; any
  post-pass that lowers a rapid below stock top must verify against the
  SAME stock snapshot and resolution the collision checker will use
  (`initial_stock`), along the whole XY track plus tool radius — not just
  the endpoint disc. (TP15 RCA outcome: emission was innocent — the
  incident was a stale cached result surviving an upstream disable, now
  fixed by `invalidate_result_chain`. The rule stays as the router's
  design constraint because the fused router multiplies link count.)

**Emission**: one `Toolpath`; per-strategy spans via the EXISTING
`SpanKind::Region` / `SpanPayload::Region { region_id }` (§1: currently
unused) so the fidelity instrument, per-band timing attribution, and the
GUI see who cut what. Region ids index into the op report's region table
(band, stats, strategy) — closes the P2.g Task 2 attribution gap too.

### 2.4 Side-data and spans

- Emit `SpanKind::Region` spans per routed node (id → report table).
  This is a MUST, not nice-to-have: the +.05-tail RCA showed verdicts
  re-deriving territory from a dilated band map mislabel raster-owned
  flats as scallop territory and manufacture phantom quality gaps —
  attribution must come from the op's own record of who cut what.
- Carry `rest_grid` + `rest_regions` from the in-op detector to the
  annotated result (same slots the pencil arm uses) so the GUI heatmap and
  `DerivedRestRegions` consumers keep working.
- Scallop runtime annotations shift into the stitched frame as today.

## 3. Wall-part fixture (contour must be tested honestly)

Wanaka has NO smooth walls — "contour is dead" is a terrain-specific
verdict. New deterministic fixture, generated in-code (no binary in repo):

- `tests/common/wall_part.rs`: procedural heightmap → `TriangleMesh`
  (~100×100×20 mm):
  - two planar walls at 80–85° meeting the floor with a 3 mm fillet
    (contour's home turf; heightmap-representable),
  - a smooth 55° dome flank (mid-steep, smooth → the rule must send this
    to contour or scallop on z-extent/economics, not noise),
  - a V-groove crease, 90° included angle, 1.5 mm deep (pencil claim),
  - a shallow 10° curved cap (raster/scallop territory),
  - a textured band (sinusoidal ripple, wanaka-like) on ONE wall section
    to exercise the smoothness gate.
- Acceptance on this part: per-island rule routes walls→contour,
  textured wall→scallop, groove→pencil; COLUMNS instrument gates quality.

## 4. Validation / A/B harness plan

Branches (both parts, wanaka + wall part):

| branch | description |
|---|---|
| v3 | Ø2 ball all-over scallop + fused rest op (tip tool) |
| D  | all-over scallop, tip tool, 0.011 cusp (quality ceiling) |
| B75/live-v2 | current shipped UnifiedFinish (speed tier) |
| stack | Op A + separate scallop-rest + pencil ops (fusion OFF baseline — isolates the router's contribution) |

Metrics, in gate order:

1. **collisions == 0** (hard gate; staleness-chain sentries landed with
   the TP15 fix),
2. **FIDELITY-COLUMNS, group-filtered** (landed 2026-07-09) — verdicts read
   COLUMNS only, never vertex histograms; instrument trust ESTABLISHED by
   the three-way probe (see header),
3. total time (F-034 integrator): v3 two-op stack vs D vs B75 vs stack,
4. router KPI: `entry_s` + retract/link counts by `runtime_by_intent`
   (the fused-vs-stack delta IS the router's measured value),
5. per-strategy quality attribution via Region spans.

Measurement rules (hard-won): same-lattice paired columns; assert the
targeted op is the ENABLED finish op (project file evolves under live
sessions); never cross-run comparisons of regenerated toolpaths
(ladder-dependent air-cut filtering changes ~40 moves); attribute
quality by the op's OWN Region spans, never a re-derived (dilated) band
map; compare branches at equal EFFECTIVE cusp — a branch whose
min-stepover over-delivers (D on flats) is spending time, not proving
quality.

## 5. Risk register

- **R1 — island explosion on textured flanks.** Rest-mask ∩ band grid on
  wanaka may fragment into O(100) islands. Mitigation: the existing
  conditioning pipeline runs AFTER the intersection; min-area absorption +
  sliver cap are the backstop; region-count stat gates the A/B. Per-region
  surface rebuild duplication (scallop/waterline rebuild per call) was
  accepted at O(1) regions — v3 must lazily share surfaces if counts grow
  (flagged in unified_finish comments).
- **R2 — pencil over-claiming.** Dendritic wanaka networks could claim the
  whole flank as corridors. Mitigation: claims capped by per-branch median
  gate (exists), `min_rest_depth_mm`, and a claim-share telemetry stat
  (claimed_area / island_area) with a sweepable cap.
- **R3 — contour still loses on the wall part.** Then wall-like islands
  route to scallop and v3 ships as claims+router only; the per-island rule
  keeps the door open without betting the op on it.
- **R4 — router link safety.** The TP15 class (rapids below fresh stock
  top) is exactly what the fused router multiplies — even though the
  incident itself was staleness (fixed), a router bug would look the same.
  Emission-time stock-clearance check (§2.3) + the staleness-chain
  sentries + the sim collision gate.
- **R5 — side-data stitching.** Region spans + rest_grid carry (§2.4);
  budget a PR; fiddly not hard (P2 R5 precedent).
- **R6 — Ø2 bulk op economics.** On big smooth parts Op A at Ø2 may be
  slower than Ø6 rough-finish + rest cascade. Out of scope for v3 core:
  Op A is user-chosen (advisor hint later); v3's contract is only "Op B
  finishes whatever rest the chosen Op A leaves".
- **R7 — instrument trust.** CLOSED 2026-07-09: the three-way probe proved
  COLUMNS pointwise-exact against re-stamps and envelopes in-run (see
  header). Residual rule: never compare across regenerations — capture and
  measure within one session run.

## 6. Build order (slices, each headless- then live-validated)

1. **S0 — foundations** (tails): three-way probe verdict; TP15 collision
   fix + sentry. [DONE 2026-07-09]
2. **S1 — claims pipeline, no routing change**: in-op `detect_rest_valleys`
   (stock reference), corridors for all claimed creases, rest-mask ∩ bands,
   pencil emission via `centerline_cut_paths`, band-major concat as today.
   A/B checkpoint: quality (COLUMNS) must not regress vs live v2; pencil
   corridors visible in report.
   **[DONE 2026-07-13, commits 78bc140/b7b1ea2/f36ac7f/4b41dc9→0fbaede.**
   First A/B FAILED its quality gate → the geometric-claims amendment
   above; rerun PASSES: collisions 0/0, mid-steep on-size −1.7 pp (the
   honest `min_rest_depth 0.02` skip price, +.05 share flat), project
   −0.7 %. Landed beyond plan: Region spans + report region_table,
   rest_grid/rest_regions carry-through, per-group FIDELITY-COLUMNS,
   enabled-finish-op harness retarget, `s1_claims_ab` +
   `s1_claims_mask_probe`. OPEN for S2: crease claiming on wanaka finds
   no claimable valleys at default detector dials (R2's tuned dials are
   the starting point — sweep alongside the wall fixture); GUI panel
   doesn't expose `pencil_claims`/`min_rest_depth_mm` yet.]**
3. **S2 — per-island classification**: `IslandStats` + routing rule +
   wall-part fixture; sweep `wall_smoothness_max` / `wall_min_z_extent`.
4. **S3 — fused router**: cross-strategy `RegionPath` graph + Region spans
   + emission-time link-safety check. KPI: entry_s/retract count vs S1.
5. **S4 — cascade A/B**: Ø2 + v3 vs D vs B75 vs stack on both parts; lock
   defaults; catalog + docs update (FEATURE_CATALOG, MCP guidance).

## 7. Process-proof campaign log (2026-07-13 → 07-27)

The §0.a build list, run end to end on the wanaka ×2 fixture. Every slice
committed with gates green (workspace clippy zero-warning, `cargo fmt`,
core lib tests past the 3 known adaptive3d reds).

### What shipped

| commit | slice |
|---|---|
| `69d66f6` | ×2 fixture + harness skeleton (runtime-generated STL + project TOML under `target/v3_scaled/`, never committed) |
| `789df66` | S2 region-level territory filter (`min_region_rest_share`, decompose-then-drop, whole islands only) |
| `27be687` | S3 stock-referenced crease claims (`CreaseReference::MachinedStock`, self-probe stays default) |
| `50b7295` | **deviation-instrument frame fix** (core bug, see below) |
| `19a3e1b`→`784b975` | S4 rest-territory confinement, four measured iterations |
| `1d92ce7` | cascade A/B scoring harness + three campaign diagnostics |

### The frame bug (`50b7295`) — a real core defect the campaign surfaced

`SimulationRequest::model_mesh` arrives in the sim's stock-relative frame
(world model translated by `−stock_origin`). Non-identity setup groups'
`local_to_global` outputs match that frame; IDENTITY groups' dexel grids
are WORLD-framed (F-024) and were compared **unmapped** — mis-registering
every column and vertex deviation by exactly the stock origin. On the ×2
fixture (origin −5,−5,−5) that read as a uniform ~−4 mm "overcut" in every
band, next to 0 collisions and sane removed volumes.

Every identity-setup project with a non-zero stock origin was affected.
Wanaka scale-1 never tripped it because its measured setups are
non-identity, and the pre-existing unit sentry supplied its flat model in
WORLD coordinates — masking the very case it should have caught. The
sentry now supplies the model in the request frame and fails on the old
code. Proven independently by ray-casting the source STL at the probed
XYs: `pred = 2·z₁((x−5)/2, (y−5)/2) + 5` matched the implied model
reference to 1e-3 mm at every probe.

**Rule**: an instrument that has only ever been exercised on one frame
class has not been validated. Fixture diversity IS instrument validation.

### S4: four dead ends before the mechanism worked

The goal — confine Op B to rest islands — is one line in §2.1 step 4.
Getting there cost four measured iterations, each killed by evidence:

1. **S2 whole-island drop alone** (+47% vs baseline). `decompose`'s
   conditioned islands on this terrain are whole-band-sized; every giant
   island contains above-dial rest *somewhere*, so keep-or-drop keeps
   everything and Op B runs all-over at tip dials.
2. **Polygon clip vs the detector's `region_polygons`** (quality gate
   failed, tails 2-3×). Those polygons are gated on TRUSTED above-dial
   cells; steep faces read NaN in the stock-referenced field, fell outside
   every polygon, and the clip amputated the mid-steep and very-steep
   bands whole — Op B emitted 3 shallow pieces. Skipped territory shows up
   as the `>+.5` TAIL, never as an on-size shift (the S1 lesson, again).
3. **Polygon clip vs a NaN-keeping keep-mask** (same failure, different
   cause). Dendritic rest masks fragment into hundreds of islands;
   `region_polygons_from_mask` keeps the largest `MAX_REST_REGIONS` (64)
   and warns. The silently-dropped area is the tail. **Any mechanism with
   a polygonization step inherits that cap** — the tail probe proved the
   detector saw the material (tail columns sat on above-dial rest, mean
   1.2-1.7 mm), so the loss was plumbing, not detection.
4. **Mask-AND sourced from the S2 verdict grid** (Op B all-over again,
   54 k s). That verdict is a 5-point footprint max-top vs min-drop across
   two grids — deliberately keep-biased for drop-safety, which is correct
   for S2's island shares but SATURATES as a mask: on sloped terrain the
   neighbourhood z-span alone (0.5 mm at 45°) dwarfs an mm-class dial.
5. **Mask-AND dilated by `cutter.radius()`** (all-over again, 58.5 k s).
   `MillingCutter::radius()` is `diameter()/2` = the WIDEST cutting point;
   on a Ø1-tip tapered ball that is the 3 mm shaft. A 3.5 mm dilation
   welds a dendritic keep-mask into full coverage.

**What works** (`784b975`): keep-mask from the detector's own
stock-referenced rest FIELD (`NaN ∪ rest ≥ min_rest_depth_mm`), dilated at
the SAMPLING scale (one rest-grid cell + `region_margin_mm`),
nearest-resampled and ANDed into `covered` **before** `decompose` — so
conditioning normalizes the rest islands itself and there is no
polygonization to cap. Design doc §2.1 step 4 prescribed exactly this; the
detour was ours.

Transferable rules, all paid for:
- **A quantity tuned for one decision is not a general-purpose signal.**
  Keep-biased measurements make bad masks.
- **`radius()` is the widest point, not the tip.** Anything scaling a
  tolerance off tool size on a tapered tool must say which radius.
- **Caps are silent by construction.** A `MAX_*` constant upstream of your
  data path will not fail your test; it will quietly change your answer.

### Where the proof stands

Collisions: **0/0 on every run, every branch** — the safety gate never
wavered. Tails: cascade **beats** D (1.2-1.5 k vs 3.0-4.4 k `>+.5` columns
per band) — the confinement is honest, not skipping material. Mid-steep
on-size: **parity** (51.0% vs 52.3%). Open: shallow and very-steep on-size
trail by 3-4 pp, and Op B's time is dominated by air — 18.3 k s of rapids
against 17.7 k s of cutting (~51%), the many-island retract tax the fused
router (S3) exists to solve. The ball-size sweep (Ø2/3/4) is the next
measurement; a smaller ball shrinks rest area, which attacks fragment
count and air time directly.

**The process is not disproven — it is instrumented and honest, and the
remaining gap has a named owner (S3's router).** What the campaign
delivered besides the mechanism: a trustworthy deviation instrument on
identity setups (a core fix that outlives this experiment), a reproducible
scaled fixture, and three diagnostics that each converted a mystery into a
measurement.

### Ball-size sweep (§0.a item 5) — the decisive measurement

Cascade-only, wanaka ×2, Op B dials fixed (`v3_ball_sweep`):

| ball Ø | Op A s | Op B s | finish stack s | Op B cutting / rapid | Op B removed mm³ | mid-steep on-size | mid-steep tail |
|---|---|---|---|---|---|---|---|
| 2.0 | 24 403 | 39 401 | 63 804 | 15 190 / 21 243 (58% air) | 9 743 | 53.7% | 786 |
| 3.0 | 11 071 | 40 747 | 51 817 | 17 676 / 18 297 (51% air) | 10 507 | 51.0% | 1 234 |
| 4.0 | 8 176 | 40 833 | 49 009 | 20 252 / 15 797 (44% air) | 10 806 | 50.6% | 1 377 |
| — | D (all-over tip) | | **39 871** | 35 917 / 3 129 (8% air) | 50 944 | 52.3% | 4 288 |

**Op B's total time is INVARIANT to ball size** (39.4 / 40.7 / 40.8 k s — a
3.6% spread) while Op A's varies 3× (24.4 → 8.2 k s). Op B's removed volume
is likewise flat (9.7 / 10.5 / 10.8 k mm³) even as the preceding ball
doubles in diameter. What moves is only the INTERNAL mix: smaller ball →
finer, more numerous rest islands → more air, less cutting; bigger ball →
fewer, larger islands → less air, more cutting. The total is pinned.

Two conclusions, both structural:

1. **On this terrain, ball size is not a lever.** The rest Op B cuts lives
   in valleys narrower than any practical ball, so the rest field is
   TIP-determined, not ball-determined (consistent with the pre-campaign
   measurement that every ball Ø2–6 leaves 87–96% of the mid-steep band as
   rest area). Op A's own time is the only thing ball size buys — hence the
   best cascade is the biggest ball, Ø4 at 49 009 s finish stack.
2. **Op B runs at a fixed traversal budget, not a fixed workload.** Halving
   its material does not halve its time; it converts cutting into air. That
   is the many-island retract tax stated as a law, and it is exactly what a
   fused cross-strategy router (S3) exists to remove.

### Verdict: NOT PROVEN on this fixture — with a named blocker

Best cascade (Ø4) 49 009 s vs all-over-tip 39 871 s: **+22.9%**. Quality is
a genuine mixed result — the cascade leaves far less standing material
(mid-steep tail 1 377 vs 4 288 columns >0.5 mm; the Ø2 branch reaches 786)
and matches on mid-steep on-size (50.6–53.7% vs 52.3%), while trailing 3–4
pp on shallow and very-steep on-size. Collisions were 0/0 on every branch of
every run.

So the process as specified — ball all-over + ONE unified rest-clear — does
NOT beat all-over-tip on time here, and the sweep proves the gap cannot be
dialled away with ball size. The blocker is Op B's traversal, quantified:
44–58% air against 8% for the all-over pass. Note also that Op B's dressup
block is inert (a `link_moves`/`retract_strategy` change produced a
byte-identical 40 746.8 s), so this air is intrinsic to the op's own
emission and routing — not something a dressup can fix. S3's fused router
owns it, and the sweep now gives that work a precise target: bring Op B's
air share to the all-over pass's ~8% and the Ø4 cascade lands near 33 k s,
comfortably under D.

**What the campaign proved instead:** the mechanism (rest-territory
confinement) works and is honest — tails beat the baseline, meaning the
confinement skips finished material without abandoning uncut material; the
instrument is now trustworthy on identity setups (a core fix); and the
remaining gap is a single, measured, owned structural cost rather than a
mystery.
