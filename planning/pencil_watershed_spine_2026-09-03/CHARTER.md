# Pencil watershed-spine — charter (2026-09-03)

> Operator hypothesis: the Track H watershed finder extracts pencil
> centerlines better than the crease finders on terrain relief.
>
> Status: **CLOSED — REJECT (2026-09-03).** P0 promoted `crate::flow_accum`
> (all Track H censuses green). P1's A/B instrument
> (`tests/pencil_spine_ab_p1.rs`) measured both extractors on one rest field.
> The first wanaka ruling was RETRACTED — the operator caught it running on a
> flooded board (a rimmed part is a closed basin; the flood ramped into
> cardinal fuzz). Amendment A4 routes B inside the rest mask. On the CORRECTED
> data, flow-accumulation traces the dendritic drainage within each rest
> region — 26.8 m vs A's 3.1 m (8.7×), even 2T is 6.1× — because surface
> drainage lines are a different curve family from the rest RIDGES pencil
> wants. The drainage-deletion lesson (`53293c96`) holds; the NMS pipeline
> (`29a6d61`) stands. Full record + RE-RULING in `FINDINGS.md`.

## The question — sharpened

NOT "does a watershed detector beat the crease detector." The
2026-07-03 postmortem
(`planning/pencil_postmortem_and_rest_driven_design.md`) settled the
CRITERION: pencil is a TOOL-INTERACTION feature (offset-surface
self-intersection / bitangent double-contact), and the correct driver
is the dual-tool REST comparison. Dihedral, Curvature, and the deleted
Drainage detector are all design-surface proxies, tool-radius-blind. A
watershed line is in that same class — so as a standalone detector it
would repeat the exact mistake that deleted `Drainage` (`53293c96`:
"hydrology trunks, not tool-relevant seams; needed non-literature
gates").

The real question is one level down:

> **Does D8 flow-accumulation, run on the REST FIELD, extract better
> pencil centerlines than the current NMS + hysteresis + Zhang-Suen
> skeleton pipeline inside `rest_depth_arm`?**

"Better" = more coherent (fewer, longer, connected spines that do not
shred at staircase corners), at no loss of coverage, at no higher path
cost, with the spine sitting at least as close to the valley bottom
where the ball nestles. The rest gate stays the criterion; watershed
is only the SKELETONIZER of the rest field.

## Why this escapes both prior rulings

- **The drainage-deletion ruling** ("needed non-literature gates"):
  the shared pipeline already rest-gates every arm
  (`fair → lift → rest-depth gate → offset → order → emit`,
  `pencil.rs`). Flow-accumulation on the rest field is rest-gated by
  construction — it is not unfiltered hydrology of the design surface.
- **The Track H strategy refutation** (`planning/valley_tracing_2026-09-02/`):
  Track H killed watershed as a FULL-SURFACE area-fill strategy vs
  raster (tree-tracing 3.5× slower via fan overlap; catchment-zones
  fail bar W0-a). A sparse, rest-gated, width-limited pencil pass is a
  different regime; that efficiency math does not bind it. The ONE
  lesson that carries: adjacent dendritic branches' offset fans still
  overlap, so keep pencil's offset-pass count width-gated (it already
  is).

## Standing priors (do not re-derive)

- **Pencil is a tool-interaction feature.** The rest arm (detector #4)
  is the correct one; this experiment improves its EXTRACTION stage,
  not its criterion.
- **The rest mask is a space-filling hairball** (module doc,
  `rest_field.rs`). `29a6d61` already replaced mask-medial-axis with
  NMS+hysteresis ridge extraction and moved coverage **0.137 → 0.80**.
  The baseline to beat is that 0.80 pipeline, NOT the old hairball. A
  marginal win is a real possible outcome and a legitimate "do not
  adopt".
- **Watershed identification is verified clean** (Track H: 49-basin
  map, divide crossings = junction count). The identification is not
  in doubt; only its VALUE for pencil-spine extraction is.
- **The rest field is already a 2.5D drop-cutter DEM** — "the same
  machinery the old drainage DEM used" (`rest_field.rs` doc). Flow
  accumulation applies natively; no new projection, no new blind spot.
  It inherits (does not add) the rest field's existing limit: it
  cannot see non-drainage 3D concave seams (overhangs, wall
  junctions).
- **Ceiling honesty.** Both the current NMS spine and a
  flow-accumulation spine are DEM approximations of the true pencil
  curve (Park et al.: offset the mesh, self-intersections, material-
  side tracing). This experiment improves the approximation; it does
  not reach the canonical method.
- **Reuse is a rebuild.** All Track H hydrology is inline in test
  files (`priority_flood_epsilon`, `d8_receivers`, `d8_accumulation`
  copied across `catchment_basin_census_w0.rs`,
  `valley_prize_census_h0.rs`, `valley_branch_falsifier_h1.rs`); the
  verified instruments emit only labelled grids + scalar stats, no
  centerlines; the one centerline producer lives in the CLOSED V1
  falsifier. Phase 0 promotes a clean library fn and dedupes the
  copies — justified debt paydown independent of the result.

## Phases — each gated on the one before it

- **P0 — promote flow-accumulation to a library module.** Pull
  priority-flood + D8 receivers + D8 accumulation from the test files
  into `crates/rs_cam_core/src/` (candidate: `flow_accum.rs`), switch
  the three Track H test copies to consume it (kills the copy-paste),
  keep every Track H test green byte-for-byte. Lint/fmt clean.
  Enabling step; no behaviour change to any shipped op.
- **P1 — the A/B extraction instrument (X4: measure before building).**
  On a rest-heavy fixture, compute the rest field ONCE via the
  existing machinery, then run BOTH spine extractors on the SAME field
  — extractor A = the current NMS+hysteresis+Zhang-Suen path, extractor
  B = flow-accumulation trunks — and report the pre-registered metrics.
  `#[ignore]` evidence instrument, committed the moment it lints clean.
  Render the surface both ways before any ruling.
- **P2 — ruling + (only if P1 passes) wire the dial.** If the bars
  pass and the render confirms, add a `PencilSpineExtractor` opt-in on
  the rest arm (default unchanged), then a separate decision on
  promote-to-default. If P1 fails or ties, close with the numbers and
  do not wire anything.

## Cross-cutting constraints

- **X1 measure before build (P1 before P2).**
- **X2 render before ruling.** Never gate on an aggregate without
  rendering the surface (Track H rule). A coherence number a picture
  contradicts is void.
- **X3 same field, both extractors.** The comparison isolates
  extraction; field construction is shared, not re-run per arm.
- **X4 fixtures.** wanaka200 for organic terrain coherence/coverage
  (this is NOT a spacing claim, so the facet rule does not bar it) PLUS
  one synthetic rest fixture with a KNOWN valley network, so fragment
  count and connectivity have a ground-truth right answer.
- **X5 cost with the shipped rig.** Path cost uses
  `relink_and_cost_under` under the machined-stock `link_ceiling`
  regime (the same rig the finishing campaigns use), not a raw length.
- **X6 single cargo lane; commit only own files.** Do not touch the
  closed Track H files beyond the P0 library switch, and do not
  resurrect the refuted V1 falsifier's inline centerline code — promote
  a clean fn instead.
- **X7 stop-and-report.** If the spec cannot be implemented as written
  (e.g. the rest field cannot be tapped before extraction without a
  refactor larger than the experiment), stop and report rather than
  widen scope.

## Files

- Pre-registration + results: `FINDINGS.md` (bars fixed before P1 runs).
- Instruments: named per phase in `FINDINGS.md` when written;
  `#[ignore]` under `crates/rs_cam_core/tests/`, committed on lint-clean.
