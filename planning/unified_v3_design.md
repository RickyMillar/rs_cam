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

## 8. Follow-up measurements (2026-08-03) — the blocker was misattributed

Two hypotheses raised after the campaign, both measured
(`v3_load_and_air_probe`, `v3_recoverable_air_probe`, cascade at Ø4).

### Tool load: real but not binding on this fixture

| op | chipload | power | deflection |
|---|---|---|---|
| Rough (shared) | **Exceeds** 0.0129 mm/tooth | Within 0.0244 kW | Within 0.0211 mm |
| D all-over tip | Within 0.0008 | Within 0.0006 kW | Within **0.0099 mm** |
| Op A ball Ø4 | Within 0.0024 | Within 0.0117 kW | Within **0.0041 mm** |
| Op B tip | Within 0.0007 | Within 0.0003 kW | Within **0.0081 mm** |

The only exceedance is the Rough op, identical in both branches. The
cascade does what it claims for the fragile tool — peak tip deflection
−18% (0.0099 → 0.0081 mm) and 4× the material moved onto a tool running
2.4× stiffer — but every finishing gate is far inside limits here, so the
load advantage is HEADROOM, not a fix. It becomes decisive only where the
tip is the binding constraint (deeper rest, harder stock, longer stickout).

### Air: 100% intra-region — §7's S3 attribution is WRONG

| op | rapids | rapid length | inside regions | between regions |
|---|---|---|---|---|
| D all-over tip | 30 980 | 55 093 mm | **100%** | 0% |
| Op A ball Ø4 | 16 757 | 38 745 mm | **100%** | 0% |
| Op B (Ø4) | 80 367 | **540 801 mm** | **100%** | 0% |

Op B travels 540 m of air against 164 m of cutting, and NONE of it is
between routed regions — its two outer Region spans contain every rapid.
**S3's cross-region fused router, which §7 named as the owner of this
gap, would recover exactly nothing.** The cost is the strategy emitter
retracting between disconnected ring fragments on dendritic rest
territory: mean hop 6.7 mm vs 1.8 mm for the contiguous all-over pass.

### Root cause: the existing reorderer is unreachable for surface ops

`crate::tsp::optimize_rapid_order` (2-opt, safe-Z interstitials, F-038b
diagonal guard) exists and is wired into the dressup pipeline — but
`execute.rs` gates it on `!rapid_order_barriers.is_empty()`, and
`RapidOrderBarrier` spans are emitted ONLY from depth sections and
adaptive3d events. Scallop, waterline, raster and unified_finish emit
none, so the optimizer never fires for the entire surface-finishing
family. `optimize_rapid_order = true` in a project file is silently a
no-op on these ops. (Separately confirmed: `DressupPolicy::strip_all` on
UnifiedFinish strips entry/lead/link only — it is NOT what disables the
reorderer; the barrier gate is.)

### The prize, sized offline (no core changes)

Greedy nearest-neighbour over rapid-separated cut fragments, either
endpoint — a WEAK heuristic, so a conservative lower bound:

| op | fragments | emitted hops | NN order | recoverable |
|---|---|---|---|---|
| Op A ball Ø4 | 869 | 2 438 mm | 854 mm | **65.0%** |
| Op B (Ø4) | **12 774** | 110 046 mm | 11 464 mm | **89.6%** |
| D all-over tip | 1 045 | 3 170 mm | 914 mm | **71.2%** |

Op B's emitted order is near-pessimal: it cuts ring-by-ring, and on
fragmented territory consecutive pieces of the same ring are far apart.
Every branch leaves 65-90% on the table, so this is not a cascade-specific
defect — **the whole surface-finishing family ships unordered rapids.**

Op B's rapid TIME at Ø4 is 15 797 s. Even at half the geometric recovery
that is ~7 900 s off the cascade (finish stack 49 009 → ~41 100 s, near
parity with D at 39 871); approaching the measured 89.6% with keep-down
linking puts it near ~34 000 s — **~14% FASTER than the all-over
baseline**, while keeping the better tails and the −18% tip deflection.

**Revised verdict: the process is not disproven, and the gap is a missing
reorder pass rather than a missing router.** §7's "S3 owns it" stands
corrected.

### The actual root: a capability classification the code flags as provisional

The barrier gate is downstream. `OperationType::transform_capabilities`
(`compute/catalog.rs`) classifies UnifiedFinish alongside Scallop as a
**genuinely continuous trace** — `continuous_path_required: true`, the
same bucket as spiral/helical/projected paths — which makes ALL three
reorder capabilities false:

- `allows_barriered_rapid_reorder()` = `!continuous_path_required` → false
- `allows_unbarriered_rapid_reorder()` → false (already wired at
  `execute.rs:2446`, and `tsp::optimize_rapid_order` itself explicitly
  handles the no-barrier case: "with no barriers there is one group
  covering the whole toolpath")
- `allows_link_moves()` → false

So neither TSP path can ever fire, regardless of barriers. The machinery
is complete and reachable — only the classification stands in the way.
And that classification carries its own TODO:

> "UnifiedFinish mirrors Scallop here (registration checklist decision) —
> its stitched per-band toolpath is a candidate for looser capabilities
> once P2.d routing lands, but until then it inherits Scallop's
> conservative continuous-path treatment."

**P2.d routing landed** (it is the `route_greedy`/`choose_link` router in
the shipped op). The precondition the comment names has been met; the
capability was simply never revisited. And on the merits the
classification is wrong for this op: its toolpath is a STITCHED set of
per-region strategy outputs separated by retracts and router links — the
opposite of a continuous trace. Finishing fragments carry no material-state
dependency on each other, which is exactly the DropCutter rationale
("XY-independent ops: TSP can reorder by proximity safely").

Proposed change, safe by construction:

1. UnifiedFinish emits zero-width `RapidOrderBarrier` spans at each region
   node boundary — and, inside a VerySteep waterline band, at each Z level.
   Barriers are ordering constraints the TSP must respect, so reordering
   is confined to fragments within one Z level of one region: never across
   bands, never against depth order.
2. Reclassify UnifiedFinish off `continuous_path_required` so the
   barriered path can fire.

Scallop (71% recoverable) and the rest of the family are the same
misclassification, but they are SHIPPED ops whose output users have cut —
extending this is a separate, evidence-backed decision, not a drive-by.

## 9. The reorder, shipped and measured (2026-08-03) — the prize was mis-sized

§8 predicted the reorder would take the Ø4 cascade from 49 009 s to
~41 100 s at half recovery, ~34 000 s at full. It shipped. It does not.

### What shipped

| commit | change |
|---|---|
| `74234fe` | ProjectCurve reclassified (fragment-based, not continuous) |
| `6fd2c1a` | `allows_link_moves` decoupled from the reorder predicates |
| `bfedf91` | `apply_link_moves` gouge-checks its bridges |
| `f09ae3c` | **UnifiedFinish region-node barriers + capability flip** |
| `1269f4b` | DropCutter reorder sentry; two harness defects |
| `fb6287b` | SteepShallow split barriers + capability flip |

`spans::region_node_barriers` is the shared mechanism: a zero-width
`RapidOrderBarrier` at each routed node's first move, plus one per Z level
inside nodes whose strategy ladders in Z. The router's cross-node sequence
— costed against the machine envelope, and carrying the surface links —
stays exactly as routed; the TSP reorders runs *within* a node.

### Wanaka ×2, ball Ø3, against §8's own recorded baseline

| | before | after |
|---|---|---|
| Op A ball finish | 11 071 s | 11 070 s |
| **Op B unified rest** | **40 747 s** | **38 868 s** (−4.6%) |
| Op B cutting / rapid | 17 676 / 18 297 | 17 019 / **17 077** (−6.7%) |
| finish stack | 51 817 s | **49 939 s** (−3.6%) |
| D all-over tip | 39 871 s | 39 904 s |
| mid-steep on-size | 51.0% | **51.0%** |
| mid-steep `>+.5` tail | 1 234 | **1 234** |

Quality is byte-identical — as the swept-cut-segment equality in the
sentries predicts, and a useful end-to-end confirmation of it. Op A and D
are unchanged. The reorder is real, safe, and worth −1 878 s.

### Why it is 4.6% and not 20%: the probe measured the wrong distance

Geometrically the reorder did what §8 said it would. At Ø4 Op B's
inter-fragment travel went **110 046 mm → 35 656 mm (−67.6%)**, and its
mean hop **8.6 mm → 2.8 mm** — exactly the density of the ops that were
already reordering (Op A 2.81 mm, D 3.03 mm). Op B is no longer the
outlier.

But `recoverable_air` measures the straight-line distance between one
fragment's EXIT POINT and the next's ENTRY POINT — both on the surface.
The tool does not travel that line. It retracts to safe Z, traverses, and
descends: two Z legs of ~30 mm each against an XY hop of ~8.6 mm. **The XY
component the reorder can address is roughly a tenth of the real cost**,
which is why recovering 67.6% of it bought 6.7% of the rapid time.

**Op B's air is COUNT-bound, not DISTANCE-bound.** 12 780 fragments × one
retract/traverse/plunge round trip each. Reordering cannot remove a single
one of those round trips; it only shortens the flat part in the middle.

### What that leaves

The lever is keep-down linking and fragment count, not ordering:

1. **Surface links WITHIN a region.** `surface_link::build_surface_link`
   already builds gouge-checked, surface-following links — the router uses
   it between REGIONS. Applied between adjacent fragments inside a region
   it removes the Z legs entirely, which is where the 17 077 s lives.
2. **Fewer fragments.** 12 780 pieces on dendritic rest territory is the
   emitter retracting at every ring break. Sliver merging or corridor-aware
   ring generation attacks the count directly.

§8's "~34 000 s at full recovery" should be read as ~38 900 s, which is
what was measured. The process-proof verdict is unchanged (+23.9% project
at Ø3), and the named blocker is now specific: **per-fragment retract
round trips inside Op B's regions.**

### Family audit, closed

| op | classification | why |
|---|---|---|
| Waterline | depth barriers, reorders within level | already correct |
| DropCutter, Drill, AlignmentPinDrill, ProjectCurve | unbarriered global reorder | XY-independent; all four now sentried |
| Scallop (discrete) | global reorder, links forbidden | `6fd2c1a` |
| UnifiedFinish, SteepShallow | barriered reorder within nodes | this section |
| Scallop (continuous), RampFinish, SpiralFinish | blocked | genuinely continuous traces |
| HorizontalFinish | blocked | deliberate high-to-low Z safety order |
| Face, Inlay, VCarve | links forbidden | V-bit width varies with depth |
| **Pencil, RadialFinish, Chamfer** | **never reorder** | permit the barriered path but emit no barriers, and are denied the unbarriered one — the one gap left |

### Two measurement rules this produced

1. **"Dexel removal is monotonic" holds only at full coverage.**
   `dexel::ray_blend_above` subtracts a FRACTION `f` of a partial-coverage
   cell's above-surface span (`new_exit = exit - f * above_part`) — neither
   idempotent nor commutative. A reorder with provably identical geometry
   still moves boundary columns ~0.87 mm on ~0.7% of cells, and unlike
   discretisation it does not shrink with cell size. Reorder neutrality is
   asserted on `swept_cut_segments` (the multiset of `(from, to)` pairs
   each cutting move sweeps), not on a heightmap tolerance.
2. **Depth ORDER needs a structural instrument.** Removal being order-free
   in the interior means cutting the deep pass before the shallow one
   leaves byte-identical stock — the hazard is cutting force, not
   geometry. The sim cannot see it; assert on the move sequence.

## 10. The air-cut filter was the fragment factory (2026-08-03)

§8 blamed Op B's 540 m of air on the strategy emitter retracting between
ring fragments. §9 refined that to "count-bound, 12 780 junctions". Both
were wrong about WHERE the junctions come from.

### How it surfaced

Instrumenting the §9 intra-region linker, not reading the code. Its
telemetry reports what the GENERATOR hands over:

```
fragments=1634  surface_links=1150  retract_links=461
too_far=461     off_surface=0       slower_than_retract=0
```

1 634 fragments — and the shipped toolpath carries **15 373**. The linker
was joining 71% of what it saw; something downstream was manufacturing
nine times as many junctions as the generator ever emitted.

Only one pass converts cutting moves into rapids: `dressup::filter_air_cuts`.
The generators' own logs confirm it independently — Op B's scallop call
reports `rapid_mm=77248.9`, and the shipped op carries 539 017 mm. **The
filter adds 462 m, or 86% of Op B's rapid travel.**

### The mechanism

The filter drops in-air cutting moves and bridges the gap with a retract to
`safe_z`, a traverse, and a descent — about 35 mm of travel on this fixture.
It applied that bridge to EVERY air run, with no test on how much air the
bridge saves. Skipping a 2 mm sliver therefore costs ~17× the distance it
avoids. On a rest-clearer, whose passes cross previously-cut ground
constantly, nearly every run is a sliver.

`AirBridgePolicy::ShorterThanAirPath` vetoes a bridge longer than the air
path it replaces; the vetoed run is emitted verbatim, so the tool simply
cuts the sliver. Distance rather than time on purpose — rapids are never
slower than cutting feeds, so a bridge up to `rapid/feed` times longer
could still win on the clock and this rule declines it. Conservative in one
direction only, and it needs no machine envelope threaded through the
dressup pipeline.

### Measured (wanaka ×2, ball Ø3, Op B; collisions 0 throughout)

| case | Op B s | project s | vs shipped |
|---|---|---|---|
| shipped (bridge always, no links) | 38 868 | 52 070 | — |
| §9 links only | 36 869 | 50 070 | −5.1% |
| §10 cost-aware bridges only | 28 037 | 40 679 | **−27.9%** |
| **both** | **25 474** | **38 116** | **−34.5%** |

Fragments 15 373 → 2 141. Rapid travel 539 017 → 91 880 mm (−83%).

The two levers compound rather than overlap: linking joins fragments the
filter would otherwise bridge back apart, so each makes the other worth
more (−5.1% and −27.9% separately, −34.5% together).

### What this says about the campaign

§7 concluded NOT PROVEN at +22.9% and named S3's cross-region router as the
owner. §8 corrected that to the missing reorder pass. §9 shipped the
reorder, measured +4.6%, and corrected the prize to count-bound. The actual
answer was a missing length test in a dressup neither section had looked at.

The pattern worth keeping: every one of those corrections came from
building an instrument and reading it, and each instrument found something
its own hypothesis had not predicted. The §9 linker's real value was not
its 5% — it was that its telemetry made the 1 634-vs-15 373 discrepancy
visible.

### Not yet claimed

`AirBridgePolicy` defaults to `Always` and `intra_region_hookup_mm` to 0.0.
This changes emitted G-code for every op running `FromRemainingStock`, and
two things must land before the defaults move:

1. **The all-over baseline re-measured under the same policy.** The filter
   is a shared dressup, so D speeds up too. D's rapids are 3 128 s of
   39 904 (8%), so its floor is ~36 776 s — close enough to the cascade's
   36 545 s finish stack that comparing new-cascade against old-D would be
   the isolate-the-variable error this campaign has already paid for twice.
2. **Quality at these dials.** The policy works by cutting through air the
   filter used to skip. That is plausibly free, but it changes what gets
   cut, so it needs the COLUMNS gate — not an argument.

`v3_process_proof_ab` runs both branches at `Dials::V3` and gates on both.

### §10a — the two dials INTERACT; they are not additive

Cascade branch only (`v3_cascade_dial_isolation`), wanaka ×2 ball Ø3,
collisions 0 in every case. Shallow band, over-cut columns (`<-.5`):

| dials | shallow `<-.5` | worst mm | shallow on-size | finish stack |
|---|---|---|---|---|
| shipped | 1 218 | −3.03 | 15.2% | 49 939 s |
| §10 bridges only | 1 534 (+316) | −3.16 | 14.6% | **38 548 s** |
| §9 links only | 1 597 (+379) | −3.12 | 15.1% | 48 012 s |
| both | **4 068 (+2 850)** | **−3.95** | 14.0% | 35 985 s |

**+316 and +379 separately, +2 850 together.** Four times the sum of the
parts, so the damage is an INTERACTION between the two passes, not a
property of either. Any fix aimed at one dial in isolation will measure
clean and still fail in combination — which is exactly how the first
process-proof run got its result.

The time split is just as lopsided in the other direction:

- **bridges alone** −22.8% finish stack (49 939 → 38 548 s), already
  beating D's 39 380 s by 2.1%, for +316 over-cut columns.
- **links alone** −3.9% (49 939 → 48 012 s), for +379 over-cut columns —
  worse quality than bridges for a sixth of the speed.

So the §9 linker is nearly worthless on its own and is half of a
destructive pair. The §10 bridge policy carries essentially all of the
win. They should be decided separately, and the linker does not ship until
the interaction is understood.

### §10b — the veto test compares LENGTH, and it should compare TIME

`filter_air_cuts` phase 1b vetoes a bridge when

```
(safe_z − from.z) + (safe_z − to.z) + xy_dist(from, to)  >=  air_run_length
```

Both sides are millimetres. But the two sides are travelled at different
rates: the air run is a FEED move (3 000 mm/min on this fixture) and the
bridge's three legs are rapids. A bridge of *equal length* is therefore
several times FASTER than the air it replaces, so a length-equal test
vetoes bridges that were worth keeping. The dial as shipped is
mis-signed — it is more aggressive than "cost-aware" implies.

That it still measures −22.8% says the win comes from the sheer COUNT of
round trips (§9's finding), not from the marginal cases the threshold
adjudicates. It also means the measured number is a FLOOR: a time-based
test using the same F-034 integrated-time model
`surface_link`/`machine_kinematics` already provide
(`retract_link_time` vs the run's feed time) should veto strictly fewer
bridges and give back some of the time this heuristic throws away.

Not done, and deliberately not bundled: the length test is what every
number in §10/§10a was measured with, and re-basing the criterion mid-A/B
would invalidate them. Do it as its own change with its own A/B.

### The gate cannot be passed by fixing these dials

Shallow on-size at SHIPPED dials is 15.2% against D's 19.2% — the 2 pp gate
already fails before any of this work. Removing every regression these two
dials introduce lands shallow back at 15.2% and the gate still fails. The
cascade's shallow deficit is pre-existing, recorded in §7's
"shallow/very-steep trail by 3-4 pp", and unchased. Closing it is a
separate piece of work from the air/travel work, and the process proof
needs both.

## 11. The shallow deficit, localized (2026-07-27) — Op B gouges

§10a left the campaign with a passing TIME gate and a failing QUALITY gate
that no §9/§10 dial could reach. This section chases it, and the answer is
not a texture or a stepover story: **the cascade removes material it should
never have touched, in bulk.**

### First, the time gate, settled in ONE run

`v3_process_proof_ab` at `V3_DIALS=bridges` (both branches, one run — the
cross-run comparison §10 refused to bank):

| | D | cascade |
|---|---|---|
| finish stack | 39 380.3 s | **38 548.2 s (−2.1%)** |
| project total | 41 511.1 s | 40 679.0 s (−2.0%) |
| collisions | 0 | 0 |

D is near dial-invariant (39 904 shipped → 39 380 with the policy, −1.3%,
all rapid), so the cascade's margin is real and not an artifact of
comparing against a stale baseline. **§10's bridge policy alone clears the
time bar.** The §9 linker is not needed for it.

### The instrument that found it: `deep_overcut_locator`

The `<-.5` histogram bin counts deep over-cut but cannot say WHERE, and
"scattered" (a systematic depth error) and "clustered" (a few bad moves)
call for opposite investigations. The locator prints the per-band count,
the worst columns' world XY, and a 24×24 occupancy map. It runs on every
scored branch, so no future A/B can hide this class again.

### Deep over-cut by stage (SHIPPED dials, `dev < −0.5 mm`)

`v3_shallow_deficit_localize` scores a PARTIAL chain (`V3_STAGE`), so a
population can be attributed to an op instead of a branch:

| stage | off-region | shallow | mid-steep | very-steep | total | worst |
|---|---|---|---|---|---|---|
| D (Rough + Ø1 tip) | 38 | 8 | 21 | 11 | **78** | −1.26 |
| Rough + Op A only | 71 | 22 | 145 | 0 | **238** | −2.24 |
| full cascade | 4 179 | 1 218 | 375 | 288 | **6 060** | −5.56 |

**The cascade over-cuts 78× more columns than D, and 5.56 mm deep.** Op A
contributes 238; Op B adds ~5 800. The shallow gate deficit is a corner of
a much larger confinement failure — most of it lands OFF-REGION, i.e. on
ground Op B's own territory clip is supposed to have excluded, where the
band gates never looked.

Two exonerations worth recording: the Rough is shared by both branches, so
it cannot produce a cascade-only delta; and D's worst columns
((148.00, 54.25), (150.25, 54.25)) are the SAME coordinates as Op A's
−1.96, so a ~1–2 mm over-cut cluster at x≈148–157, y≈50–55 is terrain
pathology common to every branch, not anyone's bug.

### It is the crease-claims node

Op B's Step 3.5 emits the claimed creases through `pencil::emit_paths` with
`PencilParams::default()` — including `hookup_distance: 5.0`. That emitter
links fragments with `build_surface_link`, which checks only that the
cutter keeps MESH CONTACT. It takes **no territory boundary**: a crease
link is free to leave the rest island it belongs to and feed across ground
the op was confined away from. This is the identical gouge class
`RelinkParams::boundary` was added to prevent (`surface_link.rs`: "on
dendritic rest islands a straight line between two fragments of the SAME
region leaves that region constantly", measured 1 218 → 4 068). **The §9
relink pass got the boundary check. The crease emitter never had one.**

Turning the claims pipeline off entirely (`V3_CLAIMS=off`):

| | claims ON | claims OFF | D |
|---|---|---|---|
| deep total | 6 060 | **2 516** | 78 |
| worst | −5.56 | **−2.24** | −1.26 |
| shallow on-size | 15.2% | 15.7% | 19.2% |
| mid-steep on-size | 51.0% | **54.5%** | 52.3% |
| very-steep on-size | 25.8% | 26.5% | 28.6% |
| Op B | 38 868 s | 51 771 s | — |

The claims node owns **at least 3 544 deep columns** and the ENTIRE
population below −2.25 mm: with it gone the worst column is −2.243 at
(51.25, 123.75), which is Op A's own worst, to the micron and the
coordinate. Mid-steep on-size with claims off (54.5%) BEATS D (52.3%).

**Caveat, stated because it cost this campaign hours twice before: this
probe varies TWO things.** `execute.rs` builds the claims config as
`cfg.pencil_claims.then(..)`, so `pencil_claims = false` disables the
territory clip as well — which is why Op B gets 33% SLOWER (it no longer
confines itself to rest islands). The counts are still a valid LOWER bound
(removing an op cannot create over-cut, and the extra coverage can only
add cuts), but "claims off" is not the isolated experiment.

### The isolated lever: `ClaimsConfig::crease_hookup_mm`

Added so the crease emitter's link cap can move without touching the
claims pipeline or the territory clip. Default 5.0 — byte-identical to
every measurement before 2026-07-27. `0.0` keeps every crease CUT and
removes only the unbounded LINKS, which splits the two candidate
mechanisms:

- if the deep population collapses at 0.0, the links are the mechanism and
  the fix is to thread the region boundary into `pencil::emit_paths` the
  way `relink_fragments` and `choose_link` already carry it;
- if it does not, the crease CUT PATHS themselves carry bad Z, which is a
  claims-pipeline problem and a much deeper one.

Exposed on `UnifiedFinishConfig` (`crease_hookup_mm`, serde-defaulted) and
driven in the harness by `V3_CREASE_HOOKUP`.

### The attribution ladder, complete

`V3_STAGE=rough` was the missing control, and it settles the ambiguity the
site probe could not:

| stage | deep total | worst | where |
|---|---|---|---|
| Rough alone | **37** | −1.256 | (148.00, 54.25) |
| Rough + D | 78 | −1.256 | (148.00, 54.25) — the SAME column |
| Rough + Op A | 238 | −2.243 | (51.25, 123.75) |
| full cascade | 6 060 | −5.565 | (198.75, 52.75) |

**D's finishing pass introduces no gouge at all**: it adds 41 columns and
does not deepen the worst by a micron — D's worst column IS the Rough's
worst column, at the same coordinate. The Rough has ZERO shallow deep
columns, so it cannot be behind the gate deficit either.

Op A adds 201 columns and a NEW deeper population at a new site. Op B adds
5 822 more and takes the worst to −5.57. Both finishing passes in the
cascade gouge; the all-over pass does not. That is the finding, and it is
now measured end to end rather than argued.

### Mechanism: still open, with the candidates narrowed

At the mid-steep site the model top is 0.003 mm and the stock was cut to
−2.240. `v3_gouge_site_probe` shows Op A's ring passes descending a near
vertical wall 1.7 mm away in Y, in 0.18 mm XY steps dropping up to 2.6 mm
in Z each. Two candidates survive:

1. **Unprobed short chords.** `refine_chord` returns immediately when
   `len < 2 × probe_step` (`probe_step = max(cell/2, 0.15)`, so 0.375 mm
   for Op A's Ø3 ball). Ring vertex spacing floors at `cell × 0.75` =
   0.28 mm, so on this op MOST chords are never probed against the
   surface at all — and the ones on a cliff carry the largest Z
   excursions. The chord between two valid CL points can still pass
   inside the material.
2. **The `min_z` fallback.** `ring_to_3d` gives non-contact points
   `min_z + stock_to_leave` (`min_z = mesh.bbox.min.z`), which the scaled
   terrain puts near −3.92 — and Op A's deepest observed move is −3.918.
   The run-splitter is supposed to retract around those points; if a
   partial run ever feeds toward one, the tool descends the whole way.

Ruled OUT, each by measurement rather than argument: the Rough (37
columns, no shallow); the §9 relink pass (off at SHIPPED dials); the
crease node's unbounded links (worst column byte-identical with
`V3_CREASE_HOOKUP=0`); crease path Z (`pencil::lift_to_surface` is a
drop-cutter); and the COLUMNS instrument itself (reported x/y are world,
the model query is frame-shifted to match — `collect_column_deviations`
doc — and the deep-over-cut sites reproduce exactly across runs).

**The instrument this wants next** is a swept-cutter-vs-model gouge check:
replay the toolpath and test whether the cutter swept along each cutting
move penetrates the MODEL beyond tolerance. That answers "was this move
ever legal" directly, for every op, without a dexel sim — and the codebase
has nothing like it today (`bridge_corridor_is_swept` is the closest, and
it is corridor-only and constant-width).

### The chord fix: rejected by measurement (2026-07-27)

`v3_chord_gouge_probe` confirmed the chord candidate directly — every
cutting move's interior probed against the drop-cutter surface it should
ride, `gouge = cl.z − chord_z`:

| op | probes | >0.05 mm | >0.5 mm |
|---|---|---|---|
| Rough | 474 255 | 250 | 26 |
| Op A | 2 809 779 | 6 409 | **279** |
| Op B | 2 173 008 | **253 559** | **3 339** |

The ordering matches the deep-column ladder (37 / +201 / +5 822) measured
independently by COLUMNS, and Op A's worst chord is move #361785 — the
exact Ring 213 move `v3_gouge_site_probe` flagged, diving −0.772 → −3.889
over 0.18 mm of XY with its interior 1.98 mm below the legal surface.

So the fix looked obvious: `refine_chord` sizes its probe count from the
chord's **XY** length, so a 0.18 mm step that falls 3.1 mm scores
`segments < 2` and is never probed at all. Switching to the 3D length
(flat chords unaffected, `dz ≈ 0` reproduces the old test) cut Op A's
chord gouges by 36% (6 409 → 4 038 over 0.05 mm; 279 → 179 over 0.5 mm;
worst 1.98 → 1.16 mm).

**And the COLUMNS gate got WORSE.**

| | before | after |
|---|---|---|
| Op A stage, deep total | 238 | 293 |
| cascade shallow deep columns | 1 218 | **1 615** |
| cascade shallow on-size | 15.2% | **14.3%** |
| Op A / Op B time | 11 070 / 38 868 s | 11 275 / 41 363 s |
| Op B chord gouges >0.5 mm | 3 339 | **15 583** |

Reverted. Two things this teaches, both worth more than the fix would
have been:

1. **Chord fidelity and over-cut are DIFFERENT phenomena.** Refining a
   cliff chord inserts points ON the CL surface, so the tool now follows
   the wall down instead of cutting the straight line — and following it
   sweeps the ball's FLANK through material the chord skipped. A more
   faithful path is not automatically a less destructive one when the
   cutter's side is doing the cutting. The chord probe measures a
   NECESSARY condition for a legal path, not a sufficient one.
2. **Op B is downstream of Op A's stock**, so changing Op A re-cuts Op B's
   rest regions entirely — Op B's own chord gouges went up 4.7×. Any fix
   aimed at Op A must be measured on the CASCADE, never on Op A alone.

The mechanism is still open. What is now excluded, each by measurement:
the Rough, the §9 relink, the crease node's links, crease path Z, the
COLUMNS instrument, the `min_z` fallback (mesh bbox min is −4.0653 and Op
A's deepest move is −3.918 — never equal), and chord infidelity as the
PRIMARY cause. The leading remaining candidate is the ball's flank on
concave/steep geometry, which points back at the wanted instrument: a
swept-cutter-vs-model check, not a centreline-vs-surface one.

### The flank probe: one defect, two sides

`v3_flank_gouge_probe` checks the swept CUTTER rather than its
centreline. `height_at_radius` gives the profile above the tip, so a model
vertex between `z_t + h(r)` and the flute top is inside the cutter solid,
and the excess is the penetration depth.

| op | samples | >0.05 mm | >0.5 mm | worst |
|---|---|---|---|---|
| Rough | 169 818 | 78 | 12 | 1.124 |
| Op A | 1 322 032 | 2 329 | **819** | **2.349** |
| Op B | 1 143 562 | **58 970** | 1 052 | 1.633 |

The worst penetrations sit on the SAME moves the chord probe flagged
(Op A #347651/#361785, Op B #122565, Rough #12169) and are LARGER there
than the chord error (2.349 vs 1.977 on #347651). That is what a chord
putting the centreline low and the flank doing the cutting looks like.
**So the two probes are one defect seen from two sides, not two
populations** — the flank hypothesis in the previous section is not a
separate mechanism after all.

**Which leaves the campaign's sharpest open contradiction, stated
plainly:** chord infidelity is real, measured, and co-located with the
gouges; refining it reduces the chord reading by 36% and the flank
penetrations with it; and the COLUMNS gate still gets WORSE, on Op A's own
stage (238 → 293 deep columns) as well as on the cascade. Those cannot all
be true of a single mechanism, so one of the three measurements is
answering a different question than it appears to. That is the next
thread, and it should be pulled before any more fixes are attempted —
this campaign has now twice built the obvious fix for a confirmed
mechanism and had the gate reject it.

Two candidate resolutions, neither tested: (a) refinement adds points, so
the dexel does more partial-coverage blends per column, and per the
2026-07-27 review those propagate through `prior_stocks` into
remaining-stock generation — the drift only creates leftover, but it moves
where Op B decides to cut; (b) the deep COLUMNS population is dominated by
something neither probe samples — the probes check emitted CUTTING moves,
and neither looks at entry plunges, descents, or rapids.

Candidate (b) is cheap to test and has not been: filter both probes to
`MoveIntent::EntryPlunge` and the descent legs, and see whether the deep
sites are even in the cutting set.

### §11a — the ladder names Op B, and then contradicts its toolpath

`v3_column_ladder_probe` reads one column out of successive
`prior_stocks` snapshots, so it names the op that removed the material
directly instead of inferring it from totals. wanaka ×2, SHIPPED dials,
the four worst deep columns:

| site | model_z | before Op A | before Op B | FINAL |
|---|---|---|---|---|
| (198.75, 52.75) | 7.335 | +0.255 | **−0.008** | **−5.565** |
| (35.50, 149.75) | 2.192 | +1.608 | **+0.091** | **−3.027** |
| (99.00, 129.75) | 5.673 | +0.767 | +0.767 | **−2.326** |
| (51.25, 123.75) | 0.003 | +1.217 | −2.243 | −2.243 |

**Op B owns three of the four.** At the worst, Op A had already finished
the column to within 8 µm of the model; the REST clearer then removed
5.5 mm from it. That is a rest pass cutting where there is demonstrably no
rest — which is a territory question, not a chord or flank question, and
it reframes everything above.

**And then the toolpath refuses to corroborate it.** At
(198.75, 52.75), `v3_gouge_site_probe` at r = 4.0 mm — beyond the Ø1
tapered ball's full `radius()` of 3.0 — reports Op B's LOWEST Z anywhere
near the site as **2.899**, against a column that ends at **1.770**. No
cutting move, and no rapid, goes deep enough to remove that material.
Checking the geometry rather than assuming it: the tapered profile at
1.6 mm radial offset sits ~9 mm above the tip, so the flank cannot reach
either, and a hypothetical full-shaft cylinder stamp still leaves every
candidate move more than 3 mm away in XY.

So one of these is wrong, and it must be settled BEFORE any further
mechanism work, because every gouge figure in §11 rests on it:

1. **Op B's stamping** removes more than its cutter geometry should — a
   simulation defect, in which case the 6 060 "gouged columns" are partly
   an artifact and the quality gate has been reading one.
2. **`prior_stocks[Op B]` is not "the stock before Op B carved"** in the
   sense assumed here, in which case the ladder's attribution is wrong
   (though the FINAL column values, which the gate reads, still stand).
3. Something removes material between the snapshot and the final stock
   that is not any enabled op's toolpath.

The discriminating test is cheap and has not been run: stamp Op B's
emitted moves onto the "before Op B" snapshot INDEPENDENTLY, with the same
cutter, and compare against the sim's final stock at these four columns.
Equal → the sim is faithful and the geometry reasoning above is wrong.
Different → the sim's Op B stamp is the defect. This is the same
three-way shape that closed P2.g Task 1 (`pre == read`, `sim == re-stamp`,
cross-branch delta ≡ re-stamp delta), and it is the right instrument here
for the same reason.

## 12. The gouge was arc-fit emitting REFLEX arcs (2026-07-28)

§11a's contradiction — Op B removing 5.5 mm from a column its toolpath
could not reach — resolved, and the answer was a live G-code defect.

### How it was cornered

Three probes, each ruling out one layer:

1. `v3_restamp_probe` — re-stamping Op B onto its own pre-carve snapshot
   reproduced the sim to ~0.5 µm at all four ladder columns. **The
   simulation is faithful; the defect is in the toolpath.**
2. `v3_move_attribution_probe` — stamping move-by-move named ONE move:
   `#159780 ArcCW { i: −18.2399, j: −54.0468 }`, whose XY distance to the
   gouged column is **97.2 mm**.
3. Geometry: centre (158.544, 93.391), radius 57.04 for both endpoints,
   which are 3.55 mm apart. Start angle 71.34°, end 74.90° — increasing,
   so the path is COUNTER-clockwise over 3.56°. Tagged `ArcCW`, it
   commands the reflex arc: **356.4°, 354 mm of travel**. The gouged
   column sits 57.2 mm from that centre, i.e. exactly ON the bogus
   circle.

Isolated with the dials: `V3_ARCFIT=off` → column untouched;
`V3_REORDER=off` → the identical bogus arc, same `i/j`. **TSP reassembly
is exonerated** (review Finding 2 remains a real latent bug, but is not
this one).

### The defect

`arcfit::try_fit_arc` chose CW vs CCW from the cross product of two
chords and never checked the outcome. On a shallow run the three sample
points are nearly collinear, the cross product is rounding noise, and its
sign flips at random. Through the same endpoints on the same circle, the
wrong direction is the REFLEX arc — so the error is not small, it is
~360°. `arc_fitting` is a default-ON dressup, so this was never
campaign-specific, and a machine would have driven a 114 mm circle
through the workpiece.

**The missing invariant: a fitted arc must be about as long as the
polyline it replaces.** Both directions share the endpoints, so length is
the thing that distinguishes them. Now picks the closer direction and
declines the fit if even that is implausible.

### Measured effect (wanaka ×2, ball Ø3, SHIPPED dials)

| | before | after |
|---|---|---|
| deep columns, total | 6 060 | **937** |
| shallow | 1 218 | **87** |
| off-region | 4 179 | 668 |
| mid-steep / very-steep | 375 / 288 | 150 / 32 |
| Op B time | 38 868 s | 38 896 s |

**85% of the cascade's gouging was one arc-direction bug**, at no time
cost.

### And it does NOT pass the quality gate

Shallow on-size 15.2% → **15.5%**, against D's 19.2%. The gate needs
17.2%.

This is worth stating precisely because it is the campaign's most
repeated mistake in miniature: **the gouges were real, serious, and not
what the gate was measuring.** On-size counts columns within ±10 µm;
1 218 gouged columns out of 66 714 is 1.8% of the band, so removing them
could never have moved on-size by the missing 4 pp. The deficit is
distribution-wide, not a tail population — D simply lands more of the
band inside ±10 µm.

So GATE 2 remains open, with the gouge explanation now closed off. The
live hypothesis is the one §11a's ladder pointed at before the arc bug
swamped it: on shallow textured ground a Ø3 ball cannot reproduce what a
Ø1 tip can, and Op B is not clearing the difference — which is a
territory question (`min_rest_depth_mm`, the rest-field keep-mask), or it
is inherent to the tool pairing, in which case that IS the verdict.

## 13. Looking at the surface (2026-07-28) — two bugs the aggregates hid

The user's challenge: are we over-optimising against numbers that hide the
real cut? The answer is yes, and it took rendering the machined surface
once to find two defects that six sections of statistics had missed.

`fidelity_report` had been writing a deviation PNG on every scored run
since P2.c. Nobody had opened one. `write_surface_renders` now emits, from
the SAME column data the gate reads, a hillshaded relief of the machined
surface and a diverging deviation map — on every scored branch.

### What one look showed

1. **D — the BASELINE — left a ~28 mm rectangular block completely
   uncut**, and still scored BETTER than the cascade on the gate (19.2% vs
   15.5%). A gate that ranks a branch with an unmachined block above one
   without is not measuring quality. The `>+0.5 mm` tail did catch it
   (2 964 vs 1 301); the metric being gated on did not.
2. **The cascade scribes over-cut lines along the model's own triangle
   edges** on the flats — Op B's crease detector reading MESH FACETING as
   creases. The model earns it: 1.8% of triangles carry 40.8% of the
   surface area, and 1 749 facets are larger than the Ø3 ball itself.
3. **The gate reads a moiré pattern.** Grid 0.25 mm against D's 0.21 mm
   and Op A's 0.363 mm stepover — both undersampled, aliasing
   DIFFERENTLY. The visible beat is ~1.3 mm = |1/0.21 − 4|. The ±10 µm bin
   comparison is not like-for-like.

### The block was `max_rings`, and it was distorting everything

`scallop::generate_scallop_rings` budgets its ring cap from
`stepover_from_scallop_flat` — the spacing for FLAT ground, hence the
WIDEST the cusp target allows. The loop selects a SMALLER stepover on
every slope, so it consumes more rings than budgeted, and then
`for _ in 0..max_rings` simply ends. Silently. D got 504 rings and needed
~660.

Fixed by budgeting from the loop's own `cusp_r * 0.05` clamp floor (the
smallest stepover it can select) and warning loudly if the guard ever
binds. Costs nothing when the cascade collapses normally.

| D (all-over Ø1 tip) | before | after |
|---|---|---|
| time | 39 904 s | **41 904 s** |
| material removed | 50 944 mm³ | 52 326 mm³ |
| shallow on-size | 19.2% | 19.7% |
| shallow tail | 2 964 | **1 191** |
| mid / very-steep tail | 4 288 / 4 444 | **1 891 / 1 648** |

**D was skipping work, so every time comparison in §7–§12 flattered it by
~5%, and every quality comparison flattered it further** — its standing
material more than halved once it actually finished the part. Op A is a
scallop too (cap 290), so the cascade's numbers move as well. Every
headline in this document predates this and needs re-basing.

### Which rejections this puts back in doubt

| rejected | why | verdict now |
|---|---|---|
| chord fix (§11) | gate worsened 1 218 → 1 615 | **suspect** — measured pre-arcfix on a contaminated baseline; it cut real chord gouges 36% |
| §9 intra-region linker | "+2 850 over-cut, half a destructive pair" | **suspect** — also pre-arcfix; links change which polylines get arc-fit |
| `AirBridgePolicy` (§10) | +613 deep columns on an 87 baseline | **stands** — measured post-arcfix, and deep-column count is a defect metric the renders confirm |
| `pencil_claims` | confounded, 33% slower | **possibly backwards** — the renders show it scribing facet edges |

### The rule this earns

**Never gate on an aggregate without looking at the surface.** Three real
defects — reflex arcs, an uncut core, facet-edge scribing — were all
invisible to a ±10 µm bin count and all obvious in one render. The gate
should be a PANEL (defects, coverage, texture), and texture cannot be
claimed at all while the measurement grid undersamples the stepover.

## 14. The reframed question, and Step 1's answer (2026-07-28)

The user reframed the goal after §13:

> "the time margin is not too much of a concern to me right now, because I
> think that it depends what we are comparing to, and we arent comparing
> to a good thing right now. Id say 'efficiency' is a good metric, with
> bounds. But we have too many levers to pull... So the metrics are what
> have hurt us in the past. What I want to know is 'is there a way that we
> can stagger the finish from one big ball end, to smaller more focused
> passes, and be better than an equivalent scallop in efficiency'... The
> unified finish was to see if we could optimise a rest machining pass to
> do the 2nd part. Because there might be steeps that need contour,
> valleys that need pencil and similar... if there is a sparse amount of
> each, it makes sense to merge them and link together rather than run
> them in sequence. Due to linking."

That is **not** the experiment §0.a–§13 ran. Two questions were conflated:

- **Q1 — staging**: does big-tool bulk + small-tool detail beat one
  all-over small-tool pass? Cascade vs D. This is what was measured, and
  §13/the workplan closed it as not provable on this fixture.
- **Q2 — merging**: given the second stage exists, does folding its
  heterogeneous strategies into ONE linked op beat running them as
  SEPARATE sequential ops? **This is what `unified_finish` was built for,
  and it had never been isolated.**

Q2 has a property Q1 never had: a baseline with **no free levers**. Both
arms share the decomposition, the per-region strategy, the parameters and
the tool, so the only difference is the order regions are visited in.
Material removed is identical and the cut geometry is identical; the delta
is purely travel. That makes the prize exactly

```
(hop cost of a strategy-GROUPED tour) − (hop cost of a MIXED tour)
```

computable offline from the emitted region geometry — no second chain, no
measurement sim. `v3_rest_anatomy` (200 s) does it, costing both tours
with `machine_kinematics::retract_link_time`, the same integrator the
router uses, so the model cancels and only the delta is load-bearing.

### First, the instrument was reading the wrong thing — twice

**Rings are not regions.** `unified_finish_spans` builds the scallop
annotation spans with `spans_from_labeled_events` (each runs to the NEXT
annotation) and then SPLICES the region-node spans in as **siblings**. The
two families interleave instead of nesting, so `outer_region_spans`
returns both: 620 outer spans, **615 of them `Ring N`**. Every mix table
this campaign printed was mostly scallop sub-structure.

**And the routing nodes do not account for the operation.** With
`optimize_rapid_order` at its shipped default, the surviving node spans
cover **12.6% of Op B's cutting length** and sit entirely in
`[206237, 335063)` — the last 38% of the toolpath. `spans_valid` stays
`true`, so every consumer trusts them.

`RUST_LOG=rs_cam_core::tsp=debug` names the culprit exactly. TSP's
foreign-intrusion post-filter drops 878 spans: 865 `Ring N`, 8
`plunge entry`, and **5 routing nodes — 2 `MidSteep band` + 3
`Shallow band`**. Only five, but they are the big ones: the MidSteep
scallop node alone is 206 213 moves and 108 461 mm, 81% of the cutting.

| Op B, shipped fixture | reorder ON (default) | reorder OFF |
|---|---|---|
| node-span coverage of cutting | **12.6%** | **99.6%** |
| routing nodes surviving | 19 | 24 |
| reported scallop share | 0.6% | **80.9%** |
| reported raster share | 25.2% | 13.3% |
| reported pencil share | **74.2%** | **5.8%** |

The shipped mix table is not merely incomplete, it is **inverted**: it
reports pencil as three quarters of the work when pencil is 5.8%, and
scallop as a rounding error when scallop is four fifths.

This is a **telemetry** defect, not a safety one — the dropped nodes are
`depth_ordered: false` (scallop rings and raster rows are single-pass over
a height field), so interleaving them is materially harmless to the cut.
But it destroys the design doc's §2.4 "Region spans are a MUST" evidence,
and §7's H1 refutation ("100% of Op B's rapid travel is INSIDE regions")
was computed on this vector — on the ring-span form, which tiles from the
first annotation to `n_moves`, so *everything* reads as inside a region
by construction. That refutation needs re-earning; the number below does
re-earn it, on a valid vector.

### The answer: the merge premise does not hold here

Measured with valid spans (`V3_REORDER=off`), wanaka ×2, ball Ø3:

```
strategy      regions     cutting_mm     %cut   footprint_mm2
scallop             3         108588    80.9%          15211
raster             20          17842    13.3%           4109
pencil              1           7790     5.8%           6669
waterline           0              0     0.0%              0
```

**There is no contour work at all** — the fixture has no very-steep band —
and 81% of the rest is a single strategy. There is no "sparse amount of
each" to merge; there is one strategy plus trim.

The prize confirms it. 24 nodes, 23 hops:

| inter-region tour | hop seconds |
|---|---|
| as emitted (router's order) | 190.8 |
| greedy NN, mixed = **MERGED** | 189.9 |
| greedy NN, grouped = **UNFUSED** | 190.9 |

**PRIZE = +0.9 s on a 40 767 s operation — 0.002%.** Merging heterogeneous
strategies into one linked op is worth nothing measurable on this part,
and it is worth nothing for a reason that has nothing to do with the idea:
the part does not present the case the idea is for.

### What the measurement DID find: the cost is round-trip COUNT

`EFFICIENCY WITH BOUNDS` — area finished per second, which unlike seconds
is invariant to tool and stepover, and unlike mm³ is meaningful for a
finishing pass:

| pass | seconds | area mm² | **mm²/s** | rapid round trips |
|---|---|---|---|---|
| Op A ball all-over | 11 070 | 34 829 | **3.146** | 862 |
| Op B unified rest | 40 767 | 19 385 | **0.476** | **15 363** |
| cascade (A+B) | 51 837 | 39 197 | 0.756 | 16 225 |
| D all-over tip | 39 904 | 37 433 | **0.938** | 1 046 |

**The rest pass finishes ground at half the rate of just doing the whole
part with the small tool.** That single line explains every cascade result
in §7–§13 without reference to quality at all.

And the mechanism is not strategy, not ordering, not the merge:

```
Op B rapid round trips: 15 311 INSIDE a routing node, 52 BETWEEN nodes
```

Op B pays **0.79 retract round trips per mm² finished; D pays 0.028** — a
28× fragmentation gap. Each round trip costs two ~19 mm Z legs whatever
its XY length (§8's lesson, re-confirmed: the emitted tour covers 30 102
hop-mm against greedy NN's 2 023 for only 8% more time). At shipped dials
Op B is 44% rapid and 12% entry — **55% of the rest pass is not cutting.**

### What this redirects the work to

Eliminating intra-node round trips is worth 30×–1000× more than every
routing question this campaign has chased. If Op B's 15 311 internal
round trips came down toward Op A's ratio, Op B would approach its 17 042 s
of cutting time and the cascade would land near 28 000 s against D's
39 904 — **−30%**, on the metric the user named, with no quality argument
required.

The §9 intra-region linker was aimed at exactly this and returned −3.9%.
That is the under-delivery to explain, and it is now the only lever on the
board with a prize worth the trouble.

**Step 2 (the unfused-stack A/B) is NOT built.** The premise it tests is
absent from this fixture, and building it would measure a 0.9 s effect.

### §14a — steepen the terrain and the mix DOES rebalance (2026-07-28)

The user's read of §14: "if its mostly scalloping then that makes sense.
in areas there its mostly pencil and contour (steep mountains with a fine
tip) i wonder if it inverts? this map has some large flat areas."

Testable for one run. `V3_ZEXAG` scales Z only, so the XY footprint, the
triangle count and the facet topology are untouched and the strategy mix
is the sole variable. A slope θ becomes `atan(k·tanθ)`.

**Reach caps the dial.** At k = 4 the terrain carries 47.8 mm of relief
against a Ø1 tapered ball with 25 mm of cutting length — that run measures
tool reach, not strategy. k = 2 (23.9 mm relief) stays inside it and still
pushes everything originally above 61.8° past the 75° waterline threshold.

| strategy | z ×1 | **z ×2** |
|---|---|---|
| scallop | 80.9% | **55.4%** |
| pencil | 5.8% | **21.3%** |
| raster | 13.3% | 23.3% |
| waterline | 0.0% | **0.0%** |
| routing nodes | 24 | 37 |
| tiles hosting >1 strategy | 94.5% | 59.3% |

**The mix rebalances as predicted** — pencil nearly quadruples, scallop
falls from four fifths to just over half, and the rest starts to look like
the three-way split `unified_finish` was designed for.

**But waterline is still 0.0%.** The very-steep band does not fire even at
k = 2. Either this terrain has genuinely nothing above 61.8°, or the band
is being suppressed. **The "contour on the steeps" leg of the idea remains
untested**, and it is the leg with no substitute — pencil and scallop both
have alternatives, a Z-level ladder on a near-vertical face does not.

**And the merge prize does not move.** With a genuine three-way mix and
59% of tiles hosting more than one strategy, grouped-vs-mixed is **+1.4 s
on 33 225 s**. The reason is structural and now confirmed across two
terrains: 36 inter-node hops against **10 931 intra-node round trips**.
Merging optimises the 36. The cost is the 10 931.

### §14b — the staggering thesis gets BETTER as the ground steepens

The result that was not predicted, and it is the strongest signal this
campaign has produced:

| finish stack | z ×1 | z ×2 | change |
|---|---|---|---|
| **D** (all-over Ø1 tip) | 39 904 s | 50 568 s | **+26.7%** |
| **cascade** (Ø3 ball + Ø1 rest) | 49 967 s | 50 664 s | **+1.4%** |
| cascade vs D | **+25.2%** | **+0.2%** | |

**D scales with slope; the cascade is nearly invariant to it.** Steepening
the terrain closed a 25% gap to nothing — because an all-over pass with a
fine tip pays for every extra millimetre of slope-lengthened surface,
while the ball absorbs the bulk and only the residue reaches the tip. That
is precisely the staggering argument, and this is the first measurement in
the campaign that supports it.

Note the area comparison is NOT the evidence here. The cascade covers
37 837 mm² against D's 36 102, but D's own `max_rings` warning reports
2 144 mm² of uncut core at k = 2 — which accounts for essentially the
whole difference. **The time comparison is clean; the area one is
contaminated by a known defect in D.** Reported on mm²/s the cascade leads
0.747 to 0.714, and that lead should not be quoted until the truncation is
fixed.

**The caveat that blocks calling this a win: the cascade logs 60 rapid
collisions at k = 2, D logs 0.** Unattributed. On terrain this steep a
tapered tool has real collision exposure, and a 0.2% time margin is not
worth anything next to 60 collisions. Attribute before going further.

### What §14a/b change about the plan

1. **The fixture objection now has a fix, not just a complaint.** Slope
   was the missing variable, and it is one line of environment.
2. **Get a very-steep band.** Without waterline the mixed-strategy claim
   is two-thirds tested. Find out whether this terrain simply lacks the
   slope or whether the band is being suppressed.
3. **Attribute the 60 collisions** (Op A or Op B, and whether they are
   descents through the uncut slivers already known from S1).
4. **Intra-node round trips remain the lever**, unchanged by any of this:
   10 931 of 11 030 at k = 2, 15 311 of 15 363 at k = 1.

### §14c — live GUI/MCP validation on the real project (2026-07-28)

Driven through the embedded MCP server against
`planning/airrun_2026-06-01/wanaka.toml` (the user's real, modified
project — loaded read-only, never saved). Full F.4 ladder: generate → sim
→ generate → sim → generate, 9 toolpaths, 2 setups.

**The span defect is REAL in the product, and worse than headless.**
`Unified Finish 6 (live v2)`, 127 251 moves, `spans_valid: true`:

| | live wanaka | headless v3 fixture |
|---|---|---|
| Region spans total | 174 | 620 |
| of which ROUTING NODES | **2** | 19 |
| `Ring N` sub-structure | 172 | 615 |
| `RapidOrderBarrier` | 3 | — |

The two survivors are both `Shallow band`, covering moves
`[56 717, 127 251)`. **Zero MidSteep, zero VerySteep, zero Pencil-claims
nodes survive** — yet `Ring 6`…`Ring 177` occupy moves 45–56 715, so the
first **44.6%** of the toolpath is full of scallop rings belonging to a
mid-steep node that no longer exists in the span vector.

The surviving ring spans are visibly scrambled and partly collapsed —
`Ring 22` at `[45, 733)`, `Ring 17` at `[5820, 5823)` (3 moves), `Ring 6`
at `[47771, 48425)`, `Ring 21` at 6 moves — the bounding-remap signature
of a span whose moves the reorder permuted.

**The user-visible consequence, and it is the diagnostic agents are told
to reach for FIRST.** `narrate_toolpath(8)` reports:

```
Semantic trace: 183 items; depth levels 0, regions 0, rings 178.
Z-level source: raw-move fallback (no DepthPass spans present).
  z=23.198 (1st pass): 1 cut run(s), 0 marching-squares region(s), 3 cutting moves
  … 67 intermediate Z levels compressed …
```

**`regions 0`** for the operation whose entire premise is region-level
routing — and, with no spans to read, it falls back to slicing a surface
finish by raw Z coordinate and inventing a **73-level Z ladder** that does
not exist. No strategy mix is reported at all.

This is not a general narration weakness: on the same project the same
call on `3D Rough 6` reads `Z-level source: DepthPass spans` and returns
per-level gate/planner/floor-cell counters. The machinery works. It is
specifically `unified_finish`'s spans that are destroyed downstream of
generation.

**The ring-truncation warning does NOT reach the user.**
`get_toolpath_diagnostics(8)` returns five entries, all feeds/tool-load
(`chipload_clamped_to_floor`, `feed_vs_lut.high`, three stale-evidence
tool-load rows). There is no `geometry`/`quality` diagnostic for standing
material, so `scallop`'s `uncut_core_mm2` warning — added precisely
because the campaign lost weeks to an invisible 28 mm uncut block — has no
channel into the product at all. It is a `tracing::warn!` and nothing
more. **"A warning nobody sees is not a warning" applies to the product,
not just the harness.**

**Arc-fit: no reflex arcs anywhere.** Narration flags suspicious large
arcs and fired on nothing across `Back Rough` (365 arcs), `3D Rough 6`
(143), `Rivers (back)`, or the unified finish. The `a6841e1` invariant
holds on real geometry.

One lead it did surface, unproven: `Rivers (back)` (Project Curve, 20°
V-bit) reports **peak axial DOC 6.07 mm at move 174, `ArcCCW`**, on an op
that "follows the curve at a fixed surface offset — no commanded DOC". The
narration's own note lists arc-fit overshoot first among the causes. Arc
fitting interpolates Z linearly across a chord it replaced, so an arc
crossing a falling surface travels at heights the arc never passes
through — the same FAMILY as §12, on a visible-surface carve. Not
established; cheap to settle with `get_cut_trace` around sample 4963.

**Param schema passes end-to-end.** `crease_hookup_mm` (5.0) and
`intra_region_hookup_mm` (0.0) appear in `get_operation_schema` AND carry
values through the real project file's round-trip.

### What §14c adds to the build list

1. **Fix the span destruction.** It is now a product defect with a live
   reproduction, not a harness curiosity: the op's own telemetry, the GUI
   panel and the agent-facing narration all read a vector missing most of
   its regions while `spans_valid` says `true`.
2. **Give standing material a diagnostic.** `severity: caution`,
   `category: geometry`, sourced from the same `uncut_core_mm2` the
   warning already computes.
3. **Settle the `Rivers` arc DOC spike** — one `get_cut_trace` call.

### §14d — WHY the node spans are dropped: the remap is fine, the FILTER fires

`RUST_LOG=rs_cam_core::tsp=debug` on the ×1 fixture names all 878 drops:
865 `Ring N`, 8 `plunge entry`, and **5 routing nodes**. Their old and new
ranges are the whole story:

| dropped node | old range | new bounds | dilation |
|---|---|---|---|
| MidSteep band | `0..200924` | `0..200959` | +35 |
| Shallow band | `204401..230041` | `204412..230141` | +100 |
| Shallow band | `230130..234235` | `230143..234326` | +91 |
| Shallow band | `237333..280484` | `237351..280675` | +191 |
| MidSteep band | `284500..284820` | `284523..284875` | +55 |

**The remap is CORRECT.** A span of 200 924 moves comes back as 200 959 —
a dilation of 0.02%, consistent with a handful of re-approach moves
inserted inside it. These spans were not scattered by the reorder. They
were computed accurately and then **thrown away by the foreign-intrusion
post-filter** (`tsp.rs::remap_spans`), which drops any non-Operation span
if *some* old move from outside it lands inside its new bounds.

That reframes the fix completely: this is not "TSP scrambles the region
nodes", it is "the intrusion guard is firing on spans whose bounds are
fine".

**Leading hypothesis for the intruder — the node ranges do not TILE.**
The same log shows gaps between consecutive dropped nodes:
`200924→204401` (3 477 moves), `230041→230130` (89),
`234235→237333` (3 098), `280484→284500` (4 016). Those moves belong to no
region node at all.

`unified_finish.rs` Step 6 explains where they come from: an incoming
surface link is emitted *before* `offset` is taken —

```rust
if let (Some(link), Some(entry)) = (incoming_surface, rp.entry) {
    for p in &link.pts { stitched.feed_to_with_intent(*p, …, Linking); }
    stitched.feed_to_with_intent(entry, …, Linking);
}
let offset = stitched.moves.len();   // ← node range starts AFTER the link
```

…so link moves sit outside every `move_range`, while
`region_node_barriers` puts the barrier at `move_range.start` — *after*
them. Orphaned link moves therefore share a barrier group with the
**preceding** node, where the reorder is free to move them into that
node's range, tripping the guard.

**Not yet proven.** The log gives ranges, not the identity of the
intruding move. Cheap decisive test, not yet run: log the first intruding
old index alongside the drop, and check whether it falls in the gap and
carries `MoveIntent::Linking`. Do that before writing any fix — this
campaign has three times built the obvious fix for a confirmed mechanism
and had it measure worse.

**Two candidate fixes, once the intruder is named:**

1. Make the ranges tile — start each node's `move_range` (and its
   barrier) at the incoming link rather than after it. The link *is* that
   region's approach, so this is the more truthful model, and it leaves
   no move outside a node.
2. Narrow the guard — a span whose own moves all remain contiguous and
   in-order is trustworthy regardless of what else landed in its bounds.
   More general, and riskier: it changes semantics for every span kind.

Prefer (1) unless the intruder turns out not to be a link move.

### §14e — item 2 (standing-material diagnostic) is a plumb, not a one-liner

The number exists (`uncut_core_mm2`, already computed for the warning) but
there is no path from `scallop::generate_scallop_rings_with_cancel` — a
pure geometry function returning `Result<Vec<Vec<(P3, bool)>>, Cancelled>`
— to `diagnostics::ToolpathDiagnoseInputs`. Carrying it needs:

1. the ring generator to RETURN the uncut area, not just log it
   (3 call sites in `scallop.rs`);
2. the scallop op to carry it out alongside `(Toolpath, Vec<ScallopRuntime
   Annotation>)` — and `unified_finish` calls the same generator for its
   MidSteep band, so both paths need it;
3. a carrier on the result. `AnnotatedToolpath` is the wrong one — 54
   construction sites and no `Default`, and dressups destructure it
   field-by-field. **`ToolpathStats` is the right seam: 5 construction
   sites**, and it is already the generation-statistics slot;
4. a new `geom.standing_material` ID plus an adapter reading
   `ToolpathStats`, which today is not among `ToolpathDiagnoseInputs`.

Nothing here is hard; it is simply wider than "the number already exists"
implied, and it should be done as one deliberate change rather than
squeezed alongside §14d.

### §14d CORRECTION — the "ranges don't tile" hypothesis is WRONG

The §14d hypothesis was built on a misreading of its own table. Those
89..4 016-move "gaps" between consecutive dropped nodes are not gaps in
the tiling: the table lists only the **5 dropped** nodes, and the space
between them is occupied by the **19 surviving** ones, which it did not
list. Consecutive rows are not adjacent in the toolpath.

The tiling is fine, and the reorder-off run already said so — node spans
cover **99.6%** of cutting length there. It was in the measurement the
whole time.

A second check kills it independently: a surface link is `link.pts`, a
sampled path of tens of points. **No surface link is 4 016 moves long.**
The magnitude alone should have stopped this.

**What still stands** (it comes from the dilation figures, not the gaps):
the remapped bounds are near-exact — a 200 924-move node returns as
200 959 — so these spans are NOT scattered by the reorder. They are
computed correctly and then discarded by the foreign-intrusion guard.
That part is measured, not inferred.

**What is now open again:** which move intrudes, and why. Fix (1) in
§14d — "make the ranges tile" — has no premise left and should not be
built. `5fb377f` (the guard now logs `intruder_old_idx`,
`intruder_intent`, `before_span`) is exactly the right next step and is
now the ONLY route to the answer.

The lesson is the campaign's own, arriving from a new direction: this one
was not a bad instrument or a subtle frame bug. It was reading a table of
filtered rows as if it were a table of consecutive ones. **Before drawing
a structural conclusion from a list, check that the list is not filtered.**

### §14f — the Rivers DOC spike is NOT arc-fit (2026-07-28)

Item 1 from §14c, settled by isolation on the live project.

`Rivers (back)` is a `project_curve` op, 20° V-bit, **commanded depth
0.4 mm**, and the simulation reported **peak axial DOC 6.07 mm** — 15×
commanded — on an `ArcCCW` move. Since `project_curve` follows a sampled
polyline, every arc in it comes from arc fitting, and arc fitting
interpolates Z linearly across the chord it replaced. That is the §12
family and it looked like a strong lead.

`arc_fitting = false`, regenerate (3 244 moves vs 2 043 arc-fitted, so the
arcs really are gone — 0 arcs at every Z level), re-simulate:

| | arc_fitting ON | arc_fitting OFF |
|---|---|---|
| peak axial DOC | **6.07 mm** | **6.07 mm** |
| position | (111.7, 39.6) | **(111.7, 39.6)** |
| z | 3.884 | 3.885 |
| move | 174, `ArcCCW` | 325, **`Linear`** |
| moves / cutting | 2 043 / 2 498 mm | 3 244 / 2 531 mm |

**Identical value, identical position.** The move that dives is the same
move; only its representation changed. Arc fitting is not the cause, and
the `a6841e1` invariant is not implicated. Cleared.

The spike is still real — 6.07 mm on a 0.4 mm carve — so it belongs to one
of the narration's other two candidates. The likelier is the one the op's
own config points at: `Rivers (back)` runs on **remaining stock**
(`stock_source` rest — it is one of the ops that fails F.4 from fresh
state), and a curve crossing an un-roughed step would engage exactly like
this. That would make it a *planning* condition, not an engine defect.
Distinguishing it from "lift-function bridging" needs the stock height at
(111.7, 39.6) against the projected curve height there — not run.

**This is the fourth time this campaign has confirmed a plausible
mechanism and had the isolation say it was not the cause** (§9's linker,
§11's chord fix, the ring cap, now this). The pattern is stable enough to
plan around: an explanation that fits the evidence is a hypothesis, and
the isolation is cheap compared to the fix.

Worth noting separately, seen while chasing this: `Rivers (back)` spends
**510 s of its 787 s in entry moves** (`runtime_by_intent.entry_s`) against
81 s of cutting. The 6-view render shows why — ~150 tall vertical plunges,
one per curve segment, at `plunge_rate` 150 mm/min. That is 65% of the
operation, and it is not a defect, just an un-tuned linking cost on an op
with 151 separate curve fragments. Same COUNT-bound shape as §14's finding
about the rest pass.

### §14g — FIXED: the intruder is the outgoing link, and the ranges now tile

`5fb377f`'s instrumentation named it on the first run, and all five drops
agree exactly:

| dropped node | intruder idx | == span end? | intent | relocated to |
|---|---|---|---|---|
| MidSteep `0..200924` | 200 924 | **yes** | `Linking` | 70 263 |
| Shallow `204401..230041` | 230 041 | **yes** | `Linking` | 215 049 |
| Shallow `230130..234235` | 234 235 | **yes** | `Linking` | 234 052 |
| Shallow `237333..280484` | 280 484 | **yes** | `Linking` | 239 932 |
| MidSteep `284500..284820` | 284 820 | **yes** | `Linking` | 284 773 |

Every intruder is the **single `MoveIntent::Linking` move immediately
after the node**, pulled deep inside it by the reorder. One move, five
times, no exceptions.

**Mechanism.** A surface link is a *feed* move, and
`tsp::split_into_segments` only ever splits on `MoveType::Rapid`. A link
left outside every node range is therefore glued into a cutting segment
that straddles the node boundary. Reordering relocates that segment —
carrying the link — into the region it just left, and the
foreign-intrusion guard drops the region span for containing a move from
outside itself. The guard is behaving as designed; what was wrong is that
a move belonging to the transition sat outside both nodes.

Note this vindicates the *substance* of the retracted §14d hypothesis
while confirming its arithmetic was wrong. The idea "orphaned link moves
sit in the preceding barrier group" was right; "the ranges have
4 016-move gaps" was a misread of a filtered table. Being right for the
wrong reason is still wrong — the measurement is what settled it.

**Fix** (`unified_finish.rs` Step 6): open the node's `move_range` at
`node_start`, taken *before* the incoming surface link, instead of at
`offset` after it. The link is that region's approach, so this is also the
truthful model. `offset` is retained unchanged for annotation rebasing,
which is expressed in the region toolpath's local frame and must not shift.

**Measured, ×1 fixture, at the SHIPPED default (reorder ON):**

| | before | **after** | reorder-OFF reference |
|---|---|---|---|
| node-span coverage of cutting | 12.6% | **100.0%** | 99.6% |
| routing nodes surviving | 19 | **24** | 24 |
| scallop share | 0.6% | **80.9%** | 80.9% |
| raster share | 25.2% | 13.3% | 13.3% |
| pencil share | 74.2% | **5.8%** | 5.8% |
| region-node spans dropped | 5 | **0** | — |

The mix at the default dials now matches the reorder-off reference
exactly, and coverage is 100.0% — better than reorder-off's 99.6%, because
the ranges now include their own link moves.

**Sentry:** `unified_finish::tests::region_node_ranges_tile_the_stitched_
toolpath` asserts consecutive `region_table` entries are contiguous and
the last reaches the end of the toolpath. It fails on any future change
that orphans a move between nodes, which is the whole precondition.

Gates: clippy clean workspace-wide, 56/56 param sweeps, unified_finish
19/19, `--lib` at the 3 documented adaptive3d reds.

**What this restores.** The design doc's §2.4 "Region spans are a MUST"
evidence, the GUI's region panel, and `narrate_toolpath`'s strategy mix —
which reported `regions 0` on the live project. Every strategy-mix claim
made at shipped dials before this fix was measured through a vector
missing 87% of its regions.

### §14h — standing material: the number now ESCAPES, the channel is a decision

§14e scoped item 2 as a four-step plumb. Steps 1–2 are done; step 3 turned
out to be an architectural choice rather than a mechanical one, so it is
written down here instead of guessed at.

**Done (`ScallopReport`).** `generate_scallop_rings_with_cancel` now
returns `(rings, uncut_core_mm2)` instead of computing that area purely to
build a `tracing::warn!` and throwing it away. It surfaces through
`scallop_toolpath_structured_annotated{,_with_cancel}` as a third tuple
element — matching `unified_finish`'s existing `(tp, anns, report)` idiom —
and `UnifiedFinishReport` gained `uncut_core_mm2`, summed across every
mid-steep region so a truncated cascade in any of them is visible.

Two sentries, deliberately paired so neither can pass vacuously:

- `scallop::tests::ring_cascade_reports_uncut_core_when_capped` — a 50 mm
  square capped at 3 offset iterations reports > 100 mm² standing;
- `test_scallop_rings_converge` — the SAME fixture with an adequate cap
  reports exactly `0.0`.

The first also pins a real off-by-one worth knowing: `max_rings` bounds
the OFFSET LOOP, and the boundary ring is pushed before it, so a bound cap
emits `max_rings + 1` rings. The field logs said so all along
(`max_rings=504 rings_emitted=505`) and nobody had read it as a fact.

**The remaining step, and why it stopped here.** The value now reaches the
op adapter in `compute/execute.rs`. It cannot reach
`diagnostics::ToolpathDiagnoseInputs` because **`GeneratedToolpath` is a
type alias for `AnnotatedToolpath`** — the adapters return the toolpath and
nothing else, so there is no per-op findings channel at all. Three options,
none obviously right:

| option | cost | objection |
|---|---|---|
| field on `AnnotatedToolpath` | **54 construction sites**, no `Default`; every dressup destructures field-by-field | a *diagnostic* finding does not need to survive dressups, and every carrier that must be threaded through them is a `generate_via_core`-class bug waiting to happen |
| make `GeneratedToolpath` a real struct (`{ annotated, findings }`) | ~20 adapter signatures | the honest model — findings are generation output, not toolpath geometry — but it touches every operation family |
| return findings beside the toolpath from `execute_operation_annotated*` | 2 signatures + their callers | narrowest, but adds a second return channel next to an existing one |

`ToolpathStats` (5 construction sites, derives `Default`, not serde) is
still the right *destination* once a channel exists — it is already the
generation-statistics slot, and `ToolpathDiagnoseInputs` would gain one
optional field plus a `geom.standing_material` adapter.

**Recommendation: option 2.** `GeneratedToolpath` being an alias for the
toolpath is exactly why this finding had nowhere to go, and the same gap
will block the next generation-time finding. That is a "consolidate, don't
patch" change and it wants to be its own commit with its own gate run —
not tacked onto this one.

### §14h CONTINUED — DONE, via a fourth option §14h did not list

§14h recommended making `GeneratedToolpath` a real struct. On contact that
was wider than estimated — **32 return sites in `execute.rs` alone**, plus
the viz worker and CLI — and all of it churn in service of one `f64`.

The seam that actually fits was already there: **every adapter receives
`&ExecutionContext`, and the context has exactly one construction site.**
So findings ride on the context as a `Cell`, and no adapter signature moves
at all.

```rust
pub struct GenerationFindings { pub standing_material_mm2: f64 }

pub struct ExecutionContext<'a> {
    pub findings: &'a std::cell::Cell<GenerationFindings>,
    …
}
```

`execute_operation_annotated_with_regions` builds the cell, hands out a
borrow, and returns `(GeneratedToolpath, GenerationFindings)`. The thin
`execute_operation_annotated` wrapper drops findings, which confines the
tuple to the two callers that have a diagnostics pipeline to feed.

**Deliberately NOT on `AnnotatedToolpath`.** A diagnostic finding should
not have to survive the dressup pipeline; every carrier that must be
threaded through it is one missed field-copy from vanishing silently —
which is precisely the `generate_via_core` class of bug this repo has
already paid for once.

**The chain, end to end:**

```
scallop ring cascade  →  ScallopReport.uncut_core_mm2
   → (scallop adapter | unified_finish adapter) → ctx.findings
   → GenerationFindings → ToolpathStats.standing_material_mm2
   → ToolpathDiagnoseInputs.stats
   → from_generation → `geom.standing_material` (Caution / Geometry / Verified)
```

Wired on **both** production paths — the core session (`session/compute.rs`)
and the GUI worker (`rs_cam_viz`'s `generate_via_core` → `compute_stats`).
Wiring only the core path would have put the diagnostic everywhere except
the GUI, which is where §14c found it missing.

`Confidence::Verified`, not `Static`: the area is measured from the
cascade's own residual polygons. And it is not superseded by a simulation —
**a sim cannot see material the toolpath never attempted to cut**, which is
exactly why this defect survived so long.

**Sentries** (`from_generation::tests`): silent at 0.0, silent below the
1 mm² floor, silent on NaN (unmeasured ≠ defect), and reports the wanaka
837 mm² case with the area in the message.

Gates: clippy clean workspace-wide, 56/56 param sweeps, rs_cam_viz 216/216,
`--lib` at the 3 documented adaptive3d reds.

**Item 2 is closed.** The number that shipped a 28 mm uncut block now
reaches the list the user actually reads.
