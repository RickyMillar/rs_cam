# Conformal spiral + direction-field finishing — feasibility programme

> **Status: PROGRAMME CONCLUDED, 2026-09-01.** Arm A (direction field)
> is CLOSED on the Wanaka geometry class with the mechanism understood
> (`FINDINGS.md` §F1-1..3: the field turns inside one stepover; no
> decomposition repairs it); its premise is confirmed real (synthesis
> §11) and the arm reopens only for a surface that passes the
> zone-coherence census. Arm B (conformal spiral) works mechanically and
> is NOT RECOMMENDED as built (`FINDINGS_F2.md` §F2-4: all retracts
> removed, 1.87× slower on branched geometry; slit map proven necessary
> for real regions and not worth building for a losing ring scheme).
> Salvage inventory, fixture audit and ranked avenues:
> `planning/finishing_status_2026-09-01.md`. The synthesis is
> `planning/finishing_synthesis_2026-08-30.md` (§12 = closure).
>
> Original F0 note (2026-08-29): Both papers source-read in full;
> extractions at `paper_2009.02660_extraction.md` and
> `paper_2504.06310_extraction.md`, repo capabilities at
> `primitives_inventory.md` (all in this directory). The F0 design note is
> `research/conformal_finish_2026-08-28.md` — per-arm verdicts: **arm A
> (direction field) GO; arm B (conformal spiral) CONDITIONAL GO**, its
> hole/island phases gated on source-reading Shen 2024 IJRR + Nasser 2019
> (the slit map itself is not in the arXiv paper). Charter as originally
> written, 2026-08-28, follows.
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
| Direction field | Zou, Wang & Feng, *Length-optimal tool path planning for freeform surfaces with preferred feed directions* (2020), [arXiv:2009.02660](https://arxiv.org/abs/2009.02660) | A preferred local direction field traded against constant scallop spacing; paths solved globally through a Poisson formulation | **Full text read 2026-08-29** (v1, 12 pp.; no journal reference on the abs page) — see `paper_2009.02660_extraction.md`. Poisson claim confirmed in mechanics, with a correction: "globally optimal" attaches to the surrogate vector-field fit (Eq. 14), not the stated bicriteria objective (Eq. 11), and the scallop constraint is soft (< 4% measured error at 0.01–0.1 mm constraints). The direction field D itself is **not computed by the paper** — it is assumed pre-assigned. 14 catalogued gaps, all with bounded repo fills. |
| Conformal spiral | Shen, Xu, Zhang, Yan & Ding, *Conformal Slit Mapping Based Spiral Tool Trajectory Planning for Ball-end Milling on Complex Freeform Surfaces* (2025), [arXiv:2504.06310](https://arxiv.org/abs/2504.06310) | A continuous, topology-aware ball-end spiral on complex/perforated mesh surfaces | **Full text read 2026-08-29** (v2, 53 pp.; no journal reference on the abs page) — see `paper_2504.06310_extraction.md`. **The slit map construction is not in this paper**: it is consumed from Nasser 2019 (J. Sci. Comput. 78) / Yunus et al. 2014 / Shen et al. 2024 (IJRR 43). The printed scallop formula is dimensionally inconsistent as-printed; there is no gouge handling anywhere; the measured scallop overshot nominal by up to 12% in the paper's own trial. 15 catalogued gaps; F2's hole phases are gated on the extended reading set. |

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

**F0 outcome (2026-08-29):** all three deliverables exist — the two
extraction files and `primitives_inventory.md` here, and the design note at
`research/conformal_finish_2026-08-28.md`. The stop condition does **not**
fire for the direction-field arm (ball-end 3-axis is the paper's own best
case; every gap has a labelled, bounded repo fill). For the conformal-spiral
arm it fires narrowly: the slit map is absent *from the paper* but present in
its named primary sources, so F2's hole/island phases carry a reading-set
gate (Shen 2024 IJRR + Nasser 2019) instead of an invented substitute; the
simply-connected phase is not gated. Three findings from the primitives
inventory shape the phases below:

1. **No sparse solver exists in the tree** (dense `nalgebra` only). F1 uses
   a matrix-free preconditioned CG on the cotan Laplacian (implemented
   2026-08-29 in the `src/` research module
   `crates/rs_cam_core/src/direction_field.rs`, `scallop_isofield` precedent —
   see the design note §A.3 for why not test-local) — no
   new dependency; the repo precedent is `scallop_isofield.rs`'s matrix-free
   fast sweeping. Any manifest change (e.g. `faer` for F2's flattening) is
   an operator decision raised at the F2 gate.
2. **The "captured Wanaka region-1 boundary" this contract names does not
   exist as an artifact** — it is recomputed transiently inside the
   `#[ignore]`d evidence test from a machine-local 661k-triangle STL.
   Serialising it is F1 step 0.
3. **`cell_membership_matches` cannot police non-lattice candidates** (it
   compares points on the baseline raster lattice). For iso-curve/spiral
   candidates, coverage equivalence is established by the fine-dexel
   residual measure instead — amendment recorded in the design note §C.3.

## Phase F1 — direction-field prototype first

**Why first:** it is the lower-risk, directly relevant answer to the observed
single-angle cell artifact. It can be evaluated on a single bounded region and
does not require a global conformal parameterisation.

Concretised from F0 (design note §A; every [REPO] fill is labelled there):

0. **Capture the region-1 boundary**: extend the evidence test to serialise
   the `Polygon2` boundary (exterior + holes) and pinned parameters to
   `test_data/` JSON (precedent: `m5_terrain_mid_steep_polygon.json`); the
   F1 instrument loads the capture and, when the Wanaka mesh is present,
   recomputes and asserts agreement (drift sentry).
1. Test-local mesh-domain field on the captured region: lift the
   Rusinkiewicz per-vertex curvature tensor from `crest_lines.rs`;
   **D = t₁ (maximum-curvature principal direction)** — the repo-derived
   ball-end max-strip-width choice; BFS orientation-consistency with
   unfold–translate–fold transport for flat triangles; no segmentation (a
   detected inconsistency loop is recorded, not segmented around).
2. Build V (direction D⊥, magnitude √((k_s + 1/r)/8), clamped where
   k_s + 1/r ≤ 0 with a clamp count reported); solve Δφ = ∇·V with a
   matrix-free Jacobi-CG (research module `direction_field.rs`) on the
   cotan Laplacian, one pinned
   vertex. **Cheap falsifier before any toolpath is built**: count level-set
   components and saddles — if iso-curves fragment worse than the PCA-cell
   evidence (141 fragments), stop here.
3. Extract iso-curves by marching triangles (generalising the
   `crest_lines::march_valley_segments` mechanism, with saddles counted
   rather than silently dropped); min-increment level schedule at
   h = 0.03 mm; CL via `point_drop_cutter` (repo convention). Clip with
   `clip_annotated_to_boundary_set` and prove every emitted segment is
   inside the region.
4. Cost against 0° raster, PCA raster, and PCA-cell evidence through the
   same `relink_and_cost` harness (`thin_organic_island_widths.rs:1862`) on
   a raw `Toolpath`; coverage equivalence for this non-lattice candidate is
   via the fine-dexel residual measure (design note §C.3), not
   `cell_membership_matches`.
5. Emit an SVG and a fine-dexel simulation comparison. Primary runs use a
   true `BallEndmill` control fixture; the R1.5 tapered-ball stays as the
   comparison row it already is.

**Advance bar:** no new collision or boundary failure; spacing/coverage is
measured rather than assumed; C4 visual result is acceptable; and the candidate
beats the **875.9 s** PCA-cell reference by a material margin, not measurement
noise.

> **F1 OUTCOME (2026-08-30): FALSIFIED on region 1 at the cheap-falsifier
> stage — see `FINDINGS.md` §F1-1.** 6,589 polylines vs the 141-fragment
> reference (46.7×); stages C–E never ran, no time was measured. Cause is
> structural (level sets of one global scalar on a branched ribbon) plus a
> noise-floor problem (curvature-derived D has no signal on a Shallow
> region) plus the conservative increment rule (253 levels vs ~51
> expected). Consistent with thin-organic §0j/§0k/D3. Arm A is dead on
> this region class; the module remains as F2 substrate.

## Phase F2 — conformal-spiral prototype

F0 verdict: **conditional go.** The paper's own extractable core —
coverage-driven ring spacing (Eqs. 1–4, binary search against sampled 3D
coverage) and log-rectangle bridging (Eqs. 7–9) — works over any conformal
disk parameterisation and is not gated. The slit map (what makes holes work)
is external to the paper, so:

1. Start with a simply connected synthetic surface and a ball-end cutter,
   using a **harmonic disk map** (a labelled [REPO] substitution — no slits
   exist in the simply-connected case, and the paper's own spacing
   mechanism is a sampled 3D coverage check that compensates ANY
   distortion, so conformality buys ring smoothness, not correctness; the
   instrument therefore REPORTS map distortion — flipped triangles, area
   and angular distortion — so a bad map can be separated from a bad
   mechanism, the same attribution split that made F1's verdict clean).
   Boundary correspondence is plain arc-length to the unit circle; nothing
   from Shen 2024 Appendix A is used except σ(t) (A-11). Spacing by the
   paper's coverage-checked binary search; spacing/scallop formulas from
   the repo's `scallop_math` (the paper's printed Eq. 13 is dimensionally
   inconsistent as-printed); CL via drop-cutter (the paper has no gouge
   handling at all). In-repo evidence fixture: `fixtures/terrain_small.stl`
   (40k triangles, 100×73 mm) with a hole-free region polygon, keeping
   phase 1 repo-portable; `make_test_flat`/`make_test_hemisphere` carry the
   closed-form unit tests.
   **Cheap falsifier**: measure bridge-section coverage and total-length
   inflation here — the paper's own trial overshot nominal scallop by up to
   12%, treat that as the expected floor.
2. Prove continuous path, no self-intersection, bounded curvature, and spacing
   before bringing Wanaka into the loop.
3. **READING-SET GATE — OPEN as of 2026-08-29** (see
   `reading_set_gate_status.md` and the three new extractions in this
   directory). Retrieved in full: Shen et al. 2024 as its own preprint
   **arXiv:2309.10655v2** (Appendix A carries the ENTIRE slit-map
   construction, σ(t) = A-11 and the boundary assembly A-1…A-14 recovered
   verbatim); **Nasser 2015 (ETNA 44, arXiv:1308.5351v5)** — the solver
   Shen 2024 actually defers to; Yunus et al. 2014 as corroboration.
   **Citation correction:** the 2025 paper's [11] (Nasser 2019, J. Sci.
   Comput. 78) is paywalled with no OA copy, but Shen 2024 never cites it —
   it is the *preimage/radial-slit* problem, not this pipeline's solver; the
   gate is satisfied without it. Implementer notes that survive to F2:
   discretise Nasser 2015's singularity-subtracted Eq (37)→(42), not
   Shen's raw A-31; the linear system is dense nonsymmetric
   (m+1)n×(m+1)n (≈384² for two holes at n=128), so **no new dependency
   at prototype scale** — dense `nalgebra` suffices; invert interior
   points by the 2025 paper's barycentric transfer (Shen A-39 is defective
   as printed); slit radii / annulus R_A recovery is written down in NO
   source and must be derived + labelled [REPO].
4. Move to one Wanaka region **without holes**, then a region with one hole;
   capture failure classes instead of silently splitting or reconnecting.
5. Compare integrated time, turn/retract count, cutting distance, simulation,
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
