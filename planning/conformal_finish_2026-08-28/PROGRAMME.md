# Conformal spiral + direction-field finishing — feasibility programme

> **Status: research-only charter, 2026-08-28.**
>
> Operator hypothesis: a topology-aware continuous spiral, or paths following a
> locally chosen direction field, might cover organic freeform territory with
> fewer turns and retracts than globally oriented raster/cell plans. This is a
> feasibility programme, **not** a replacement decision for shipped 3D
> operations.

## Why this is a separate programme

The thin-organic evidence now makes two things simultaneously true:

- Region 1's PCA-minor raster + monotone cells cost **875.9 s**, versus the
  0° undivided baseline's **1053.7 s** (1.20×), but it still consists of 69
  straight-sweep cells and 53 kept retracts.
- A single sweep direction visibly imposes its own slices on an organic region.
  It cannot follow every branch.

This programme tests two alternatives in isolation, using the same Wanaka
region and the same F-034 integrator as that evidence. It must not alter
`UnifiedFinish`, `surface_link`, simulation, GUI, or exported G-code until an
operator has accepted a rendered/simulated result.

## Sources and scope

| arm | source | what is being tested | source limits |
|---|---|---|---|
| Direction field | Zou, Wang & Feng, *Length-optimal tool path planning for freeform surfaces with preferred feed directions* (2020), [arXiv:2009.02660](https://arxiv.org/abs/2009.02660) | A preferred local direction field traded against constant scallop spacing; paths solved globally through a Poisson formulation | Abstract verified directly. The paper's discretisation, boundary conditions, singularity treatment and tool assumptions must be read from the full PDF before code is copied into a research arm. |
| Conformal spiral | Shen, Xu, Zhang, Yan & Ding, *Conformal Slit Mapping Based Spiral Tool Trajectory Planning for Ball-end Milling on Complex Freeform Surfaces* (2025), [arXiv:2504.06310](https://arxiv.org/abs/2504.06310) | A continuous, topology-aware ball-end spiral on complex/perforated mesh surfaces | Preprint; abstract verified directly. It is encouraging evidence, not a production guarantee. The slit-map construction, origin optimisation, spacing control and degeneracy cases require a source-faithful reading before implementation. |

The source papers are research references, not a licence to copy code. Any
formula, pseudocode-derived implementation, or bundled fixture added later must
be attributed in `CREDITS.md` and comments at its implementation site.

## Non-goals

- No claim that this replaces every existing 3D operation. It may eventually
  compete for broad freeform ball-end finishing; it does not subsume waterline
  on deliberate steep walls, pencil/crease work, drilling, or operation-specific
  rest strategies.
- No production parameters, GUI controls, serialization changes, or default
  changes during feasibility.
- No conclusion from path length alone. On this Shapeoko-class machine,
  acceleration, turn count, links and plunge motion can invert a geometric
  win.

## Shared experimental contract

Every arm starts from the same captured Wanaka region-1 boundary and a ball-end
control fixture. The current R1 tapered-ball evidence remains a comparison but
is not silently treated as a ball-end result.

For each candidate, report:

1. deterministic path count, move count, cutting distance, turn count and
   kept retracts;
2. F-034 integrated time under the pinned Shapeoko Pro XXL kinematics;
3. spacing/cusp evidence in the candidate's own surface parameterisation;
4. self-intersection, boundary-escape and singularity/termination findings;
5. fine-dexel simulation: collisions, removal/coverage evidence and measured
   residual/overcut; and
6. SVG plus rendered stock comparison for the operator's C4 visual decision.

`link_ceiling: None` is allowed only for the first fresh-stock geometry
experiment and must be labelled. A promising candidate must later re-run with
the corrected rest-stock ceiling before any time claim is made.

## Phase F0 — source-faithful design note

**Cheap falsifier:** can each paper's algorithm be stated precisely enough to
implement without filling a missing step with a convenient invention?

- Read both PDFs fully; capture equations, mesh domain, boundary conditions,
  discontinuities/singularities, stated tool model, and numerical tolerances.
- Make `research/conformal_finish_2026-08-28.md`: one source-labelled
  algorithm sketch per arm, with every repo-authored adaptation explicitly
  labelled.
- Identify each method's required primitives against current capabilities:
  triangular mesh sampling, cutter-centre surface, region boundary with holes,
  sparse linear solve, UV/conformal map, streamlines/iso-curves, and robust
  curve clipping.

**Stop condition:** a material method step is absent or incompatible with a
3-axis cutter-centre path. Record the gap; do not make up a production rule.

## Phase F1 — direction-field prototype first

**Why first:** it is the lower-risk, directly relevant answer to the observed
single-angle cell artifact. It can be evaluated on a single bounded region and
does not require a global conformal parameterisation.

1. Test-local mesh-domain scalar/direction field for the captured region only.
2. Generate a limited set of iso-curves/streamlines at the equal-cusp spacing.
3. Clip to the exact region and prove every emitted segment is inside it.
4. Cost it against 0° raster, PCA raster, and PCA-cell evidence through the
   shared F-034/relink harness.
5. Emit an SVG and a fine-dexel simulation comparison.

**Advance bar:** no new collision or boundary failure; spacing/coverage is
measured rather than assumed; C4 visual result is acceptable; and the candidate
beats the **875.9 s** PCA-cell reference by a material margin, not measurement
noise.

## Phase F2 — conformal-spiral prototype

Begin only if F0 captures an implementable source-faithful construction.

1. Start with a simply connected synthetic surface and a ball-end cutter.
2. Prove continuous path, no self-intersection, bounded curvature, and spacing
   before bringing Wanaka into the loop.
3. Move to one Wanaka region **without holes**, then a region with one hole;
   capture failure classes instead of silently splitting or reconnecting.
4. Compare integrated time, turn/retract count, cutting distance, simulation,
   and rendered surface against the F1 winner and PCA-cell baseline.

**Advance bar:** a genuinely continuous or explicitly accounted-for split path,
no simulation regression, acceptable surface, and a win that survives
acceleration-aware costing. A lower retract count alone is not sufficient.

## Phase F3 — only after both prototypes work

Choose one route for productionisation:

- a direction-field/iso-scallop strategy as an opt-in `UnifiedFinish` band
  strategy; or
- a conformal spiral confined initially to the geometry/topology proven in F2.

Before shipping: core API design, report-only findings, default-off golden,
serialized parameter audit, GUI preview/override, full simulation validation,
and a live operator check. Do not merge both algorithms into one production
change simply because they share a motivation.

## Decision rule

The ambition is appropriate; the replacement claim is not yet evidence. A
conformal spiral can earn a broad freeform-finishing role, but every existing
3D operation stays until this programme proves which specific surface/topology
class it improves and preserves operator-visible quality.
