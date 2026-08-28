# Conformal finishing — Phase F0 source-faithful design note

> **Status: F0 synthesis, written 2026-08-29.** Inputs: full-text extractions
> of both source papers and a repo primitives inventory, all under
> `planning/conformal_finish_2026-08-28/`:
>
> - `paper_2009.02660_extraction.md` — Zou, Wang & Feng 2020 (direction field),
>   arXiv v1, 12 pp., every claim carrying §/Eq cites.
> - `paper_2504.06310_extraction.md` — Shen, Xu, Zhang, Yan & Ding 2025
>   (conformal slit-map spiral), arXiv v2, 53 pp., same discipline.
> - `primitives_inventory.md` — what the tree provides today, file:line'd.
>
> Labels used below: **[SOURCE]** = stated in the paper at the cited location.
> **[REPO]** = a repo-authored adaptation, substitution, or gap-fill — every
> one is deliberate and listed; nothing is silently invented.
> This note is the design record the charter's Phase F0 requires
> (`planning/conformal_finish_2026-08-28/PROGRAMME.md`).

## F0 verdicts up front

| arm | verdict | one-line reason |
|---|---|---|
| A — direction field (Zou 2020) | **GO** | Ball-end 3-axis is the paper's own best case; every missing step has a bounded, labelled repo fill; all numerics are one SPD sparse solve + marching triangles. |
| B — conformal spiral (Shen 2025) | **CONDITIONAL GO** | The slit map itself is **not in the paper** — it is consumed from Nasser 2019 / Shen 2024 IJRR. F2's hole/island phase is gated on extending the reading set to those sources. The simply-connected prototype phase is NOT gated. |

The F0 stop condition ("a material method step is absent or incompatible with
a 3-axis cutter-centre path") **does not fire for arm A**. For arm B it fires
*narrowly and recoverably*: the absent step (the generalized-Neumann-kernel
slit map) is absent from *this paper* but exists in named primary sources, so
the honest response is a reading-set extension gate, not an invented
substitute. Details in §B.4.

---

# Arm A — direction-field iso-level paths (Zou, Wang & Feng 2020)

## A.1 Source-faithful algorithm sketch

The paper's real content is: represent all tool paths implicitly as iso-level
curves of one scalar field φ on the surface, and obtain φ from a single
global linear solve.

1. **[SOURCE §3.2]** Input: a *pre-assigned* preferred feed direction field D
   (line field, max-strip-width). The paper explicitly does not compute D
   ("It is supposed in this work that preferred feed directions have been
   pre-assigned", delegating to its refs [24]/[25]); §3.3 says any direction
   field slots in.
2. **[SOURCE §4.2]** Orient D consistently: BFS-style propagation from seed
   triangle(s), flipping where inconsistent; singular (e.g. flat) regions
   filled by copying via unfold–translate–fold transport; Laplacian smoothing
   post-pass.
3. **[SOURCE §4.3]** If the field changes abruptly, segment the surface
   (closeness metric Eq. 18 with σ = 0.67 → Laplacian Eigenmaps → K-Means)
   and run per patch. Patch borders are the authors' own admitted "serious
   limitation" — no smooth transition across them.
4. **[SOURCE §3.2, Eq. 13]** Build the target vector field V per triangle:
   direction = D rotated 90° about the normal; magnitude = √((k_s + 1/r₁)/8),
   where k_s is the normal curvature perpendicular to feed (signed: convex
   positive) and r₁ the effective cutting radius — **for a ball-end, r₁ = r
   exactly** (Eq. 5 collapses to the classic iso-scallop Eq. 2).
5. **[SOURCE §3.3–4.1, Eqs. 14–17]** Solve min_φ ∫‖∇φ − V‖² via its
   Euler–Lagrange equation Δφ = ∇·V — a Poisson equation — discretised with
   the cotangent mesh Laplacian (Eq. 16) and per-triangle divergence
   (Eq. 17), one sparse linear system ("Eigen" is all the paper says).
6. **[SOURCE §3.3]** Choose level values: iso-increment √h per constant-scallop
   theory; per curve, sample points and take the **minimum** increment.
7. **[SOURCE §3.3]** Extract iso-level curves by marching triangles.

**Charter correction (carried from the extraction):** the charter line "solved
globally through a Poisson formulation" is confirmed in mechanics, but
"globally optimal" attaches to the *surrogate* vector-field fit (Eq. 14), not
the stated bicriteria objective (Eq. 11, E_Align + E_Scallop), which is
abandoned as too nonlinear. The two goals are heuristically fused into V with
no tunable weight. Expect the scallop constraint to be **soft**: the paper's
own measured scallop error is < 4% for constraints 0.01–0.1 mm, and our
h = 0.03 mm sits inside that validated band.

## A.2 Repo adaptations — every gap fill, labelled

Ordered by the extraction's gap list (14 gaps; the load-bearing ones):

| gap | repo fill | label |
|---|---|---|
| 1. D field is an entire missing front-end | For a ball-end, side-step = √(8h/(k_s + 1/r)) is widest where k_s is smallest, i.e. feed along the **maximum-curvature principal direction t₁** so the perpendicular gets κ₂. First experiment uses D = t₁ from the Rusinkiewicz per-vertex curvature tensor already in `crest_lines.rs`. This matches the paper's max-strip-width *intent* for the ball-end case but is our derivation, not the paper's. | **[REPO]** |
| 2. Poisson BCs / constant nullspace never stated | Pure-Neumann reading; pin one vertex (or project out the constant) and note the compatibility residual. Standard practice, but invented relative to the paper. | **[REPO]** |
| 3. Extraction schedule unspecified ("a certain number of points") | Evaluate the increment at **every** triangle-edge crossing of the current iso-curve (not a sample subset); take the minimum, per the paper's own conservative rule. Start level l₁ = min φ on the region + half-increment. | **[REPO]** |
| 4. No ordering/linking/machining direction | Not invented at all: emit raw fragments and hand them to the existing `surface_link::relink_fragments` + `compute_cycle_time` harness — the same treatment every other candidate in the thin-organic evidence got. Ordering quality is then measured, not assumed. | **[REPO]** (existing machinery) |
| 5. √(k_s + 1/r) undefined where k_s + 1/r ≤ 0 | For a ball-end this coincides with the local gouge condition (concave radius ≤ r). Guard: clamp the magnitude to the smallest positive value present in the region and **count clamped triangles as a report-only finding**. Additionally rs_cam CL points come from drop-cutter (below), which is gouge-free against the mesh by construction. | **[REPO]** |
| 6. k_s estimator unnamed | Rusinkiewicz 2004 tensor (already implemented, private in `crest_lines.rs`) — lift to test-local code for F1. | **[REPO]** |
| 7–9. Segmentation dof / smoothing params / singular-region test | **Avoided in F1**: run on a single Wanaka region where the field varies smoothly; if orientation propagation detects an inconsistency loop, record it and stop rather than segment. Segmentation (with its admitted border defect) is out of F1 scope. | **[REPO]** (scope cut) |
| 10. Eq. 17 index typo (sums over k, uses V_j) | Read as the standard cotan divergence from Botsch et al. (the paper's own ref [27]) — flagged inference, the obvious intent. | **[REPO]** |
| 12. No outer-boundary / hole treatment in the paper | Use the repo's own boundary machinery: solve on the region's triangles, clip emitted curves with `clip_annotated_to_boundary_set`, prove containment with `RegionSet::contains`. `Polygon2` holes are first-class here, so multiply-connected regions cost us nothing even though the paper never mentions them. | **[REPO]** |
| CC→CL conversion (paper plans cutter-contact paths) | Project each iso-curve's XY through `point_drop_cutter` to get the CL height — the same convention every shipped 3D op uses (`scallop.rs` does exactly this with its offset rings). On steep slopes this differs from the paper's normal-offset CC→CL by a lateral shift; spacing/cusp evidence (contract item 3) is measured on the achieved surface, so the substitution is checked, not assumed. | **[REPO]** |

## A.3 Primitives mapping (vs `primitives_inventory.md`)

Have today: mesh + per-triangle normals + `SpatialIndex`; drop-cutter;
`Polygon2`/`RegionSet` with holes; clipping + containment (strongest
primitive); F-034 integrator + test-local `relink_and_cost`;
`equal_cusp_stepover_mm` / `scallop_math`; marching squares (2D);
per-vertex curvature (private); edge→face adjacency (`pencil_dihedral`).

Must build (all test-local for F1):

1. **Triangle-neighbour walking** on top of `build_edge_adjacency` — needed by
   both orientation BFS and marching triangles.
2. **Marching triangles for an arbitrary vertex scalar** — generalise the
   mechanism of `crest_lines::march_valley_segments` (which already does
   exactly this for the valley predicate, but silently drops
   `crossings.len() != 2` triangles). Ours must **handle or at least count
   saddles** — contract item 4 requires singularity/termination findings.
3. **Cotan Laplacian + divergence assembly and a solver** — see §C.1.
4. **Per-vertex curvature, public/test-local lift** from `crest_lines`.

## A.4 Risks the evidence must retire

- **Soft scallop**: ‖∇φ‖ only *tends toward* the constant-scallop magnitude.
  Measured spacing histogram in the candidate's own parameterisation is
  contract item 3; the fine-dexel sim residual is the backstop.
- **Sharp corners**: the authors name minimum-length paths' "seemingly sharp
  corners" as a serious limitation for machining dynamics. On this
  Shapeoko-class machine that is precisely what F-034 punishes — which is why
  the advance bar is integrated time, not path length.
- **Iso-curve fragmentation**: a level set can split into many components in
  a branched organic region; if fragment count explodes, relink cost will
  show it (this is the same failure mode the 875.9 s PCA-cell reference
  already beat down from 564 fragments to 141).

---

# Arm B — conformal slit-map spiral (Shen, Xu, Zhang, Yan & Ding 2025)

## B.1 Source-faithful algorithm sketch

1. **[SOURCE §2.1]** Input: triangular mesh with m+1 boundaries (holes are
   first-class); pick Γ₀ ("typically the longest") as outer boundary.
2. **[SOURCE §2.1]** Flatten to the plane with **BFF** (Sawhney & Crane);
   boundary-condition mode unstated.
3. **[SOURCE §2.1]** Parameterize each flattened boundary 2π-periodically
   (per Eq A-14 of Shen 2024 IJRR); Nyström treatment at C1 corners.
4. **[SOURCE §2.1 — external]** Compute the 2D conformal **slit map** by the
   generalized Neumann kernel method (Nasser 2019; Yunus et al. 2014;
   Shen 2024): reference point O^F → origin; result is the unit disk (or
   annulus) with every hole boundary flattened to a concentric **arc slit**.
   **No equations for this step appear in the paper.**
5. **[SOURCE §2.1]** Inverse maps by barycentric transfer between
   same-connectivity meshes; normal-offset maps give the iso-scallop surface
   S^h and the cutter-centre offset (offset K_c).
6. **[SOURCE §2.2.1, Eqs. 1–4]** Spacing: concentric circles in the slit
   domain, each radius **binary-searched** until every sampled S^h point not
   yet swept lies inside it (Euclidean 3D coverage check with KD-trees).
   **This sampled coverage check is the only conformal-distortion
   compensation — there is no distortion-factor formula.**
7. **[SOURCE §2.2.2, Eqs. 7–9]** Bridging: unroll the slit domain to a
   log-rectangle (angle → real, modulus → imag; arc slits become horizontal
   slits); connect ring i to ring i+1 with a smooth blend (σ(t) deferred to
   Shen 2024 Eq A-11), **shifting each bridge in π/50 steps until it clears
   every slit and its band re-covers S^h**; sweep the start angle over
   [0, 2π] and keep the minimum-total-3D-length spiral.
8. **[SOURCE §2.3, Eqs. 15–22, App. B/C]** Origin O^F placed by minimising a
   scallop-uniformity energy (finite-difference gradient descent with an
   offset-curve jump to escape the gradient-free plateaus inside hole
   images); ~1–2 min vs 1.37 h exhaustive.

**How holes are actually handled:** ring paths avoid holes *by construction*
(they are concentric circles winding around arc slits); bridges are the only
crossing hazard and are **rerouted by shifting, never split, never lifted** —
continuity is genuinely preserved, but verified only by the sampled coverage
check. The paper's cutting trial measured −7.36% machining time, −27.79%
average spindle impact vs a decomposition-based baseline (robot arm, ball-end;
tool radius/material/feeds unreported).

## B.2 Repo adaptations — labelled

| gap | repo fill | label |
|---|---|---|
| Eq. 5 lead/tilt angle (robot arm, lead 15°) | Dropped: 3-axis, tool axis fixed +Z. Ball-end scallop geometry is axis-symmetric, so the contact/centre relations survive. The paper's pipeline nowhere depends on Eq. 5. | **[REPO]** |
| Eq. 13/15 scallop formula printed dimensionally inconsistent ((K_s + K_c)/8 mixes curvature and radius) | Use the repo's own `scallop_math` forms (`stepover_from_scallop_curved` etc.), which carry the correct (κ_s + 1/r)ℓ²/8 structure. Flagged, not silently repaired, in the extraction. | **[REPO]** |
| No gouge handling anywhere in the paper | CL points via drop-cutter projection (gouge-free vs mesh, the repo standard), instead of the paper's unspecified normal-offset surface with no self-intersection handling. | **[REPO]** |
| Sampling rules for N_S / N_C / ε absent — two demonstrated failure modes (Table 1 cases 1.4/1.5) | Choose from our sim-resolution discipline: sample S^h at the fine-dexel cell pitch; ε below the F-034 measurement floor; record both. A parameter study is part of the F2 synthetic step. | **[REPO]** |
| Bridging magic constants (π/10, 2π switch at R ≤ 0.3, 8π/5, π/50) and σ(t) deferred | Adopt as defaults, but *as reported constants under test*, and use any C¹ blend for σ until Shen 2024 is read; bridge quality is judged by F-034 + sim, not by fidelity to unpublished constants. | **[REPO]** |
| Interior-point evaluation of the slit map never described | Cannot be invented — part of the reading-set gate (§B.4). | **gate** |

## B.3 Primitives mapping

Everything arm A needs, **plus** two entirely greenfield numerical components
(`primitives_inventory.md` §5: "no UV chart, no cotangent weights, no
boundary-mapping, no cut-graph, no slit map — 100% greenfield"):

1. **A conformal flattening** (the paper uses BFF — itself a substantial
   implementation: mesh Laplace solves with prescribed boundary data).
2. **The generalized Neumann kernel slit-map solver** — a Nyström-discretised
   boundary integral equation, plus interior evaluation. Not in the paper at
   all.

Both need real sparse linear algebra (§C.1). Also new: KD-tree distance
queries (workspace already pins `kiddo = "4"`, currently unused by any crate).

## B.4 The reading-set gate (F0 stop condition, applied honestly)

The slit map is a *material method step* that is *absent from this paper*.
The charter says: record the gap; do not make up a production rule. The gap
is recoverable because the step exists in named primary sources, so:

- **F2's hole/island phases are gated** on source-reading Shen et al. 2024
  (IJRR 43, DOI 10.1177/02783649241251385) and Nasser 2019
  (J. Sci. Comput. 78:582–606) — at minimum the boundary parameterization
  (A-14), the blend σ(t) (A-11), the integral equation + kernel +
  discretisation, and interior evaluation. If neither is retrievable in
  full, F2 stops at the simply-connected phase and says so.
- **F2's simply-connected phase is NOT gated.** For a simply-connected
  region there are no slits; the load-bearing, fully-extracted parts of the
  paper — coverage-driven ring spacing (Eqs. 1–4) and log-rectangle bridging
  (Eqs. 7–9) — work over *any* conformal disk parameterisation. Using a
  plain conformal flattening there is a **[REPO]** substitution that touches
  nothing the slit map is for.

---

# C. Cross-cutting decisions

## C.1 Sparse solve — the one hard dependency question

`primitives_inventory.md` §4: nothing sparse exists in the tree (dense
`nalgebra` 0.33 only). Decision, staged:

- **F1 (arm A): no new dependency.** The Poisson system (cotan Laplacian,
  SPD after pinning) is solved with a **hand-rolled matrix-free
  Jacobi-preconditioned conjugate gradient** in test-local code. This
  follows the repo's own precedent (`scallop_isofield.rs` chose matrix-free
  fast sweeping for its Eikonal solve for exactly these reasons:
  deterministic, no heap surprises, no dependency). Region-scale vertex
  counts (10⁴–10⁵) are comfortably inside CG territory.
- **F2 (arm B): defer.** BFF/Neumann-kernel work is dominated by dense
  boundary systems (boundary point counts, not vertex counts) and may live
  with `nalgebra` dense solves at prototype scale. If a genuine sparse
  factorisation becomes necessary, adding `faer` (pure Rust, actively
  maintained) is the default proposal — **a manifest change is an operator
  decision, raised at the F2 gate, not taken silently.**

## C.2 The shared experimental contract needs an artifact — first F1 task

The charter's "same captured Wanaka region-1 boundary" **does not exist as a
file** (inventory §3): region 1 is recomputed inside the `#[ignore]`d
evidence test from a 661k-triangle machine-local STL through the whole
tier-map → islands → classify → decompose pipeline. Therefore F1 step 0:

- Serialise the region-1 boundary (`Polygon2` exterior + holes) and the
  pinned run parameters to `test_data/` JSON (precedent:
  `m5_terrain_mid_steep_polygon.json`), written by the existing evidence
  test.
- The F1 instrument then **loads the capture** and, when the Wanaka mesh is
  present, also recomputes and **asserts agreement** — drift detection, same
  pattern as the two-loader sentries.
- The mesh itself stays machine-local (it is 661k triangles and not ours to
  vendor); the capture makes the *boundary* and *parameters* deterministic.

## C.3 Costing harness contract for non-lattice candidates

`relink_and_cost` is directly reusable (input: one raw `Toolpath`), but the
`cell_membership_matches` refusal gate compares candidates on the *baseline
raster lattice* — meaningless for iso-curves or spirals that don't ride that
lattice. **[REPO] amendment for F1/F2:** coverage equivalence for non-lattice
candidates is established by the fine-dexel simulation instead (residual /
uncut-area versus the PCA-cell reference at the same cell size), and the
comparison is refused if the candidate's measured residual exceeds the
reference's. The lattice gate stays authoritative for lattice candidates.
`link_ceiling: None` stays allowed only for the first fresh-stock geometry
run, labelled, per the charter — the 875.9 s reference itself carries that
caveat.

## C.4 Tooling note

The current R1.5 evidence is a **tapered-ball** (charter already flags this).
Both papers are ball-end native. F1/F2 run their primary experiments with a
true ball-end control fixture (`BallEndmill`, cusp radius = r), keeping the
tapered-ball as the comparison row it already is; `cusp_radius_mm` supplies
the spacing radius for both.

## C.5 What would falsify each arm cheaply

- **Arm A:** solve the Poisson system on region 1 and just *count level-set
  components and saddles* before building any toolpath. If the field's
  iso-curves fragment worse than the 69 PCA cells did (141 fragments), the
  premise "follows branches with fewer turns" is already dead — no
  simulation needed.
- **Arm B:** build the spiral on the synthetic simply-connected surface and
  measure bridge-section overcut/undercut in the coverage check. If bridging
  cannot hold the scallop bound without ballooning path length there, holes
  will only be worse. (The paper's own trial overshot nominal scallop by up
  to 12% — treat that as the expected floor, and measure.)

## C.6 CREDITS obligations going forward

Recorded now (research-reference entries updated with full-text read dates
and version pins). If F2 proceeds past its gate, Shen 2024 IJRR, Nasser 2019,
and Sawhney & Crane's BFF each become sources requiring their own CREDITS
entries and implementation-site attribution. Arm A's D-field derivation
(t₁ feed direction) is repo-authored and must be documented as such at its
implementation site, not attributed to Zou et al.
