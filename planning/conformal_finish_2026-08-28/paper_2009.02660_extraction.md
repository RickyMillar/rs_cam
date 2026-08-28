# Extraction — Zou, Wang & Feng, "Length-optimal tool path planning for freeform surfaces with preferred feed directions" (arXiv:2009.02660)

**Extraction date**: 2026-08-29. **Source fetched**: arXiv PDF, v1 (submitted 6 Sep 2020; PDF dated 8 Sep 2020), 12 pages, A4.
**Authors**: Qiang Zou (Manchester + UBC, corresponding), Charlie C. L. Wang (Manchester), Hsi-Yung Feng (UBC).
**Journal status**: the arXiv abs page lists **no journal reference and no journal DOI** (only the arXiv DOI `10.48550/arXiv.2009.02660`). The PDF footer says "Preprint submitted to Elsevier". Whether a published journal version exists was NOT verified in this extraction — do not cite one from memory.
**Extraction method**: full PDF read (all 12 pages as page images) cross-checked against `pdftotext -layout` output for every load-bearing numeral. Every claim below carries its section/equation/figure number. Items the paper does not state are collected in the mandatory "Underspecified / missing steps" section — nothing there has been filled in from outside the paper.

---

## 1. Problem formulation (§1, §3)

**Objective**: generate tool paths of minimized overall path length for machining a freeform surface, where minimum length is achieved by finding "the closest satisfaction of constant scallop height and preferred feed directions" (§6; also §3 opening).

**Precise statement (§3, second paragraph)**: Given a surface S ⊆ R³ to be machined, a preferred feed direction field D ⊆ S² on tangent planes of S, and a scallop height constraint h ∈ R⁺, find tool paths {C_i}ⁿ_{i=1}, C_i ⊆ S, minimizing two error terms:
1. the error between D and the direction field of tool path tangents {C_i′(p)/‖C_i′(p)‖}, p ∈ C_i;
2. the error between h and the actual scallop height between adjacent paths C_i, C_{i+1}.

**Why length-optimal = this tradeoff (§1, §3)**: constant scallop height avoids redundant machining between adjacent paths; maximum-strip-width feed directions maximize material removal per pass. Kumazawa et al. [4] showed the two are incompatible in general ("tool paths having constant scallop height deviate considerably from those following feed directions of maximum strip width", §1), so minimum length lies in the globally optimal tradeoff. Fig. 2 motivates the direction sensitivity: a cylinder (cutter radius 5 mm, scallop 0.05 mm) needs 8 paths / 125.6 mm total axially vs 19 paths / 190 mm circumferentially — ">50% more" (§3.2).

**Where preferred feed directions come from (§3.2)**: at each cutter contact point, the preferred feed direction is the one that maximizes machining strip width (the strip's reach perpendicular to the feed direction under the cutter swept envelope, Fig. 3a). Crucially, **the paper does not generate them**: "We omit the details here since we do not consider generating preferred feed directions as new results. It is supposed in this work that preferred feed directions have been pre-assigned" (§3.2), citing [25] (Barakchi Fard & Feng) for efficient algorithms. §3.3 notes the framework also accepts other direction fields, e.g. the kinematics-derived field of [28] (Kim & Sarma). The literature convention "optimal feed directions = preferred feed directions" is adopted (§1).

## 2. Mathematical formulation — the "Poisson" claim: CONFIRMED, with one correction

**Charter check**: the charter's "paths are solved globally through a Poisson formulation" is **confirmed in mechanics** — the solved problem is a linear least-squares gradient fit whose Euler–Lagrange equation is a standard Poisson equation (Eqs. 14–15), discretised with FEM (cotangent mesh Laplacian, Eqs. 16–17) and solved as one well-conditioned sparse linear system (abstract, §3.3, §4.1). **Correction on what "globally optimal" attaches to**: the genuinely bicriteria problem (Eq. 11, E_Align + E_Scallop) is declared "hard to solve due to the high nonlinearity" (§3.2) and is NOT what gets solved. Instead the two energies "push ∇φ towards" a single target vector field V (paper's own wording, §3.2), and the globally-optimal solve is Eq. 14, the least-squares fit to V. Note (verifiable by expanding in the orthonormal frame {D, D⁹⁰°}): ‖∇φ − V‖² = (D·∇φ)² + (D⁹⁰°·∇φ − ‖V‖)², whereas E_Scallop penalises (‖∇φ‖ − ‖V‖)²; the two coincide only where alignment is perfect. The paper offers no equivalence proof and no user-tunable weight between the two goals in the main method (the "lean" variants are Appendix A). So: **globally optimal for the surrogate vector-field-fit; heuristic fusion of the two stated criteria.**

**Implicit tool path representation (§3.1)**: a scalar function φ : S → R; tool paths are the iso-level curves {p ∈ S | φ(p) = l_i} for a set of level values {l_i}ⁿ_{i=1} (Fig. 1). Tangent directions and path intervals are expressible in terms of φ without extracting curves (cites [6]). Claimed benefits (§1): no path ordering/initial-path problem, automatic handling of singularities and self-intersections in tool paths, no "tedious, error-prone topological operations".

**Alignment energy (§3.2)**: the tangent of an iso-level curve is perpendicular to ∇φ, so aligning iso-curves with D means making ∇φ ⊥ D:

- Eq. (1): E_Align(φ) = ∫_S ‖D · ∇φ‖² ("·" the inner product).
- Eq. (12): equivalently E_Align(φ) = ∫_S ‖D⁹⁰° × ∇φ‖², where D⁹⁰° rotates D by 90° about the surface normal and "×" is the cross product. This form "encourages ∇φ to point in the direction of D⁹⁰°".

**Scallop height model (§3.2, Fig. 4)**: ball-end milling, cutter radius r (Eq. 2, cites [11] Koren & Lin):

  h = (k_s + 1/r)/8 · ‖p₂ − p₁‖² + O(‖p₂ − p₁‖³)

where k_s is the normal curvature in the direction perpendicular to the feed direction at p₁ (positive if the design surface is convex, negative if concave), p₂ the corresponding cutter contact point on the adjacent path, and ‖p₂ − p₁‖ the side-step.

For general cutters the effective cutting radii differ at p₁, p₂ (r₁ ≠ r₂, Fig. 4b). Two auxiliary circles c₃ (radius r₂, tangent to the bottom circle through the scallop point p) and c₄ (radius r₁) give (Eq. 3):

  h = (k_s + 1/r₁)/8 · ‖p₄ − p₁‖² + O(³)  and  h = (k_s + 1/r₂)/8 · ‖p₂ − p₃‖² + O(³)

Line-of-symmetry relation (Eq. 4): ‖p₂ − p₁‖ = (‖p₄ − p₁‖ + ‖p₂ − p₃‖)/2 + O(³) — justified by approximating side-steps with arcs, "generally acceptable because in practice ‖p_i − p_j‖ is much smaller than the radius 1/k_s". Substituting gives the **general side-step formula** (Eq. 5):

  ‖p₂ − p₁‖ = √h · ( √(2/(k_s + 1/r₁)) + √(2/(k_s + 1/r₂)) ) + O(‖p₂ − p₁‖³)

"only a second-order approximation to the actual scallop height" — error analysed in §5 (case study 3). Footnote 1: the effective-cutting-circle notion is a poor estimate if feed directions at p₁, p₂ differ significantly; "This work thus assumes a smooth variation of feed directions."

**From side-step to gradient magnitude (§3.2)**: Taylor expansion along the level set (Eq. 6): l₂ − l₁ = (∇φ)ᵀ(p₂ − p₁) + O(²); since ∇φ ⊥ C₁'s tangent, Eq. (7): |l₂ − l₁| = ‖∇φ‖·‖p₂ − p₁‖ + O(²), i.e. Eq. (8): ‖∇φ‖ = lim_{‖p₂−p₁‖→0} |l₂ − l₁| / ‖p₂ − p₁‖. Endowing the level increment with the physical meaning |l₂ − l₁| = √h and combining with Eq. 5 (Eq. 9, with r₂ = r₁ as p₂ → p₁):

  ‖∇φ‖ = 1 / ( √(2/(k_s+1/r₁)) + √(2/(k_s+1/r₂)) ) = 1 / (2√(2/(k_s+1/r₁))) = √( (k_s + 1/r₁)/8 )

"Under this equation, two iso-level curves with increment √h are two tool paths of constant scallop height h." Scallop energy (Eq. 10):

  E_Scallop(φ) = ∫_S ( ‖∇φ‖ − √((k_s + 1/r₁)/8) )²

**Fused problem**: Eq. (11) min_φ E_Align + E_Scallop — abandoned as too nonlinear. Target vector field (Eq. 13):

  V.direction ← D⁹⁰°,  V.magnitude ← √((k_s + 1/r₁)/8)

**Solved problem (Eq. 14)**: min_φ ∫_S ‖∇φ − V‖² — "a well studied problem in computer graphics and computational mechanics" (§3.3). **Euler–Lagrange (Eq. 15)**: Δφ = ∇·V, "a standard Poisson equation — a second-order linear partial differential equation" (Δ Laplacian, ∇· divergence, cites [27] Botsch et al., Polygon Mesh Processing).

## 3. Domain and discretisation (§4.1)

- Surface representation: parametric surfaces **or** triangular meshes (abstract, §1). The Poisson equation "may be numerically solved using the finite element method or the iso-geometric analysis method; both … sound"; **this work employs FEM**, "and this makes the proposed method applicable to both parametric surfaces (with an additional meshing step [27]) and triangular mesh surfaces" (§4.1). I.e. parametric input is meshed and everything runs on the triangle mesh.
- Scalar field: φ at mesh vertices. Direction/vector fields: per-triangle (the consistency propagation of §4.2 operates on triangles; Eq. 17 sums per-triangle vectors V_j).
- **Cotangent Laplacian (Eq. 16)**: (Δφ)_i = 1/(2A_i) · Σ_j (cot α_ij + cot β_ij)(φ_j − φ_i), A_i the Voronoi area of vertex p_i, sum over neighboring vertices p_j, angles α_ij, β_ij as in Fig. 5a (standard definitions, cites [27]).
- **Divergence (Eq. 17, transcribed verbatim)**: (∇·V)_i = 1/(2A_i) · Σ_k cot θ₁(e₁ · V_j) + cot θ₂(e₂ · V_j), "where the sum is taken over all incident triangles k each with vector V_j", θ₁, θ₂, e₁, e₂ as in Fig. 5b. **Apparent paper typo**: the summation index is k but the summand uses V_j — presumably the per-incident-triangle vector; flagged, not silently fixed.
- With Eqs. 16–17, Eq. 15 "becomes a sparse system of linear equations, which can be effectively solved with existing linear algebra libraries such as Eigen" (§4.1). Abstract: "well-conditioned sparse linear system".
- **Boundary conditions: never stated anywhere in the paper.** No Dirichlet/Neumann discussion, no mention that pure-Neumann Poisson determines φ only up to an additive constant (rank-deficient system needing a pinned vertex or regularised solve). See gaps.

## 4. Singularities, discontinuities, degeneracies (§4.2, §4.3)

- **Orientation consistency (§4.2)**: machining strip widths are identical for opposite feed directions "if gouging is not a concern [4]", so D is really a line field needing consistent orientation. Procedure: pick an arbitrary seed triangle, propagate its direction to neighboring triangles (flipping where inconsistent), repeat until the mesh is covered — "conceptually similar to breadth-first search (BFS)". If some triangles have unique (unflippable) feed directions due to e.g. gouge avoidance, all such triangles are used as seed triangles.
- **Singular regions (§4.2)**: regions (e.g. flat regions) where any feed direction gives the same largest strip width, so no preferred direction can be determined. Handling: extrapolate from neighboring well-defined regions — during propagation, copy (not just flip) directions from parent to child triangles. Since neighbors are generally not coplanar, copying uses the three-step **unfold–translate–fold** transport of Fig. 6. Parenthetical caveat: these steps "do not factor in the holonomy [29] in vector transportation, possibly resulting in slightly non-smooth feed directions"; the neat fix (trivial connections) is rejected as requiring "significant introduction overheads of differential geometry"; instead "this work opts for post-processing of the directions using Laplacian smoothing [27]".
- **Degenerate points of the direction field (§4.3)**: "degenerate points often have a very low number (e.g., two or three), and are sparsely distributed over the surface. As a result, they have a very limited impact on the global optimization method presented. Thus, no special procedure is needed to handle them for the proposed method, although they are the primary concern in previous work."
- **Abrupt direction changes → surface segmentation (§4.3)**: sources: (a) convex region meeting concave region (Fig. 7 top — direction flips rightward→upward); (b) multiple seed triangles with mutually inconsistent directions meeting at BFS-region borders; (c) degenerate points in the preferred field [4]. Where abrupt changes occur the surface is **cut into patches** within which directions change smoothly, and the whole method is applied per patch. Segmentation pipeline (three steps): (1) neighbor-closeness metric, Eq. (18): exp(−(1 − d₁·d₂)²/(2σ²)) with d₁, d₂ the directions at two neighboring points and σ a free parameter "that can be set to 0.67 (≈ 2/3, and max(1 − d₁·d₂) = 2) in this work"; (2) **Laplacian Eigenmaps** [30]: build a Laplacian weighted by the metric, take the eigenvector of the smallest non-zero eigenvalue → maps surface points to a 1D line so low-variation neighbors stay close; (3) **K-Means** [31] clusters the 1D points into k clusters; the partition is sent back to the 3D surface as patches (Fig. 7). §4.3 closing note: these implementation details "are only viable methods, not necessarily … the only or the best ones… the main result, i.e., Eq. 14, of this work remains unchanged."
- Tool-path-level singularities/self-intersections: handled "automatically" by the implicit representation (§1, citing [6]) — no explicit procedure given or needed per the authors.

## 5. Path extraction (§3.3)

After solving Eq. 15 for φ, determine level values {l_i}ⁿ_{i=1} "by a modification of the method to calculate path intervals for iso-parametric tool paths": (1) sample "a certain number of points" from an iso-level curve C_i; (2) at each sampled point compute the level increment |l_{i+1} − l_i| with respect to the given scallop height h; (3) choose the **smallest** level increment as the increment between C_i and its next path. Given l_{i+1}, the iso-level curve on the surface is extracted with "the marching triangle algorithm presented in Ref. [6]" (Zou et al., CAD 2014).

Summary of the whole method (§3.3): 1. construct optimal vector field V (Eq. 13); 2. solve the Poisson equation (Eq. 15); 3. extract iso-level curves of φ.

**Not addressed**: how many points per curve, the starting level l₁, the traversal/machining order of the extracted curves, linking moves between curves, or the cutting direction along each curve (climb vs conventional). §1 frames the absence of ordering as a feature ("no particular order among them … no need to deal with … determining the initial tool path"), but that statement is about the optimisation, not about sequencing a machine program. Per-sample increment formula is implied by Eq. 9 (|Δl| = √h at constant-scallop gradient magnitude) but the exact evaluation used at step (2) is not restated.

## 6. Scallop / spacing control (§3.2, §5)

- Tool model: general — cutter-specific only through the **effective cutting shape** at the contact point: circular arc for ball-end, elliptical arc for flat-end (Fig. 3b, c); approximated to second order by the **osculating circle** of the effective cutting shape (§3.2, cites [24] Lo, CAD 1999). No formula for the effective radius r₁ as a function of cutter geometry/tilt/inclination is given — deferred to [24].
- Spacing formula: Eq. 5 (side-step vs h, k_s, r₁, r₂), collapsing to the classic ball-end Eq. 2 when r₁ = r₂ = r. §5.1: "Eq. 5 is identical to the classic formula (i.e., Eq. 2) if ball-end mills are used, which indicates the best-case scenario. And the flat-end mill represents the worst-case scenario; other end mills fall in between."
- **Guaranteed or approximated?** Approximated, three ways, all acknowledged: (a) Eq. 5 is itself second-order ("We will analyze its approximation error in Section 5"); (b) the scallop requirement enters Eq. 14 only as a soft least-squares target on ‖∇φ‖, traded off against alignment; (c) extraction picks the smallest increment over finitely many sampled points (conservative at samples only). Measured accuracy (§5.2, Fig. 12): "maximum relative errors are all below 4%"; at constraint 0.01 mm the worst case is "0.01 ± 0.0004mm, which can provide satisfactory precision control". Validity band: "this only holds for scallop height constraints in between 0.01mm and 0.1mm. When the constraint is made much larger, say 1mm, the approximation error is likely to have a significant increase. Fortunately, a scallop height constraint larger than 0.1mm is not commonly used in surface finish."
- k_s sign convention: positive convex, negative concave (§3.2 after Eq. 2). The paper never discusses what happens when k_s + 1/r₁ ≤ 0 (deeply concave region vs large effective radius), where √((k_s+1/r₁)/8) is undefined — see gaps.

## 7. Boundary handling

- Patch borders (from segmentation): the acknowledged weak point. §5.2: "this method did not produce satisfactory tool paths near the borders between the segmented surface patches. In particular, there is no continuous, smooth transition across the borders… additional constraints should be imposed on tool paths near the borders… the challenge is to avoid affecting the interior tool paths' optimality. Such constraints remain unknown, and further development is required. As such, this issue can be considered as a serious limitation of the current work." §6 adds: Ref. [19] may help "but does not fit in our implicit tool path optimization framework"; future work will plan paths across borders so interiors "remain unchanged (or take the least change)". Tool engagement/disengagement marks at patch transitions are also named (§6).
- Outer surface boundary: **no treatment stated** — no discussion of trimming iso-curves at the surface boundary, of open vs closed iso-curves, or of PDE boundary conditions.
- Multiply-connected regions (holes/islands): **never mentioned**. All test surfaces are simply-connected open sheets (blade suction surface, bike seat, saddle).
- Gouging: explicitly out of scope ("if gouging is not a concern [4]", §4.2); gouge-avoidance appears only as a possible source of unique seed directions.

## 8. Numerical tolerances and parameters — complete inventory of stated numbers

| Where | Parameter | Value |
|---|---|---|
| §4.3, Eq. 18 | segmentation metric σ | 0.67 (≈ 2/3; max(1 − d₁·d₂) = 2) |
| §5 | hardware | C++ implementation, 2.4 GHz Intel Core i5, 8 GB memory |
| Fig. 2 | motivating cylinder | scallop 0.05 mm, cutter radius 5 mm; 8 paths/125.6 mm vs 19 paths/190 mm |
| §5.1 case 1 | blade suction surface (GrabCAD) | flat-end mill radius 2 mm; scallop constraint 0.1 mm; constant tilt 0°, inclination 30°; no segmentation |
| §5.1 case 2 | bike seat (GrabCAD) | ball-end mill radius 10 mm; scallop constraint 0.5 mm; 3 patches (proposed), 4 patches (Su's); classic iso-scallop unsegmented |
| §5.1 case 3 | saddle surface, error analysis of Eq. 5 | flat-end mill radius 2 mm; scallop constraints 0.01 / 0.05 / 0.1 mm; errors vs the accurate method of Ref. [14], from **500 pairs** of randomly sampled cutter contact points |
| §5.1 | typical finish tolerance context | "often falls in [0.01mm, 0.05mm]", 0.1 mm "an upper bound" |

**Not stated anywhere**: mesh vertex/triangle counts, solver iteration counts or tolerances (direct vs iterative unspecified beyond "Eigen"), runtimes/timings, number of sampled points per iso-curve in extraction, k for K-Means, Laplacian-smoothing parameters for direction post-processing, Appendix A.1's λ value.

## 9. Experiments (§5, Figs. 8–12, Table 1)

Comparators: iso-parametric, classic iso-scallop [2] (Feng & Li 2002), preferred feed direction method [4] (Kumazawa et al.), enhanced preferred feed direction [23] (Su et al. 2020 — called "the state-of-the-art").

**Table 1 (path lengths, improvement relative to iso-scallop):**

| Method | Turbine blade (mm) | Δ | Bike seat (mm) | Δ |
|---|---|---|---|---|
| Iso-parametric | 4057.01 | +9.10% | 5405.56 | +21.89% |
| Iso-scallop [2] | 3718.67 | — | 4434.79 | — |
| Preferred direction [4] | 3693.11 | −0.67% | 4345.22 | −2.02% |
| Enhanced preferred direction [23] | 3589.91 | −3.46% | 4297.28 | −3.10% |
| **Proposed** | **3343.33** | **−10.09%** | **3857.68** | **−13.01%** |

§5.2: "the proposed method is seen to generate the shortest tool paths" in all comparisons; vs state-of-the-art the reduction is "above 7%, depending on the specific surface". (Note: Table 1 shows the vs-[23] margins as 6.9% and 10.2%; the "above 7%" is the authors' own characterisation.)

**Direction alignment** (Figs. 9, 11; mismatch angle distributions): blade — proposed avg 2.05° (scale max 5.14°) vs Su's avg 16.43° (scale max 28.81°); bike seat — proposed avg 4.79° (max 30.55°) vs Su's avg 12.75° (max 41.21°). Authors' explanation (§5.2): sequential path generation in [23] accumulates mismatch error; the global optimization (Eq. 14) "can evenly distribute mismatch errors over the surface".

**Scallop approximation error** (case 3, saddle; Fig. 12; relative error in %, per-vertex histograms): constraint 0.01 mm → min 0.0024, max 1.55, avg 0.43; 0.05 mm → min 0.0048, max 2.29, avg 0.77; 0.1 mm → min 0.0067, max 3.57, avg 1.05.

No machining-time figures, no physical cutting experiments, no runtimes — validation is geometric (path length, mismatch angle, scallop error) on 3 surfaces.

## 10. Stated limitations and future work (authors' words, closely paraphrased)

1. **Patch-border transitions (§5.2, §6)**: no continuous, smooth transition across segmented-patch borders; the constraints needed "remain unknown"; "a serious limitation of the current work"; machining efficiency affected and engagement/disengagement marks left on the surface; Ref. [19] may help but doesn't fit the implicit framework; future work: a new mechanism so borders are planned carefully while interior paths remain unchanged or change least.
2. **Over-segmentation (§6)**: "when the surface becomes very complex, the proposed method would segment the surface into many small patches, which could affect machining efficiency. Improving the surface segmentation algorithm is among the future research studies."
3. **Sharp corners / non-smoothness (§6)**: minimum length "may lead to seemingly sharp corners in the generated tool paths"; non-smoothness "could affect the machining dynamics and consequently reduces machining efficiency. This states a serious limitation … but also offers huge potential for improvement." Balancing length and smoothness is touched in Appendix A; including machining dynamics in path generation is future work.
4. **Scallop approximation band (§5.2)**: accuracy demonstrated only for constraints 0.01–0.1 mm; ≫0.1 mm (e.g. 1 mm) likely significantly worse.
5. Implicit assumptions restated as scope limits: smooth variation of feed directions (footnote 1); gouging not a concern (§4.2); preferred directions pre-assigned (§3.2).

## Appendix A — extensibility (three variants, stated for completeness)

- **A.1 Smoothness**: min_φ ∫_S ‖∇φ − V‖² + λ‖Δφ‖², λ a weight; still linear least squares. (λ value never given.)
- **A.2 Lean fully to preferred directions**: min_φ ∫_S ‖D·∇φ‖² s.t. ∫_S ‖∇φ‖² = 1 (forbids the trivial φ = const); Lagrange multipliers → generalized eigenvector of the smallest generalized eigenvalue. Pointed critique of existing work [19, 23], which minimize ∫‖∇φ − D⁹⁰°‖²: "this cannot give expected results. This energy encourages ‖∇φ‖ = 1 between adjacent streamlines, which pushes streamlines towards geodesic parallels. Real streamlines of a direction field are, however, far from geodesic parallels."
- **A.3 Lean fully to iso-scallop**: min_φ ∫_S ‖D·∇φ‖² s.t. ‖∇φ‖ = √((k_s + 1/r₁)/8) — a hard pointwise constraint needing "more sophisticated" methods; augmented Lagrangian with penalty [33]; procedures from the authors' prior work [6] adaptable.

---

## Underspecified / missing steps (first-class deliverable)

Everything an implementer must invent, ordered roughly by how load-bearing it is:

1. **The preferred feed direction field D itself.** Assumed pre-assigned (§3.2); computing max-strip-width directions is delegated wholesale to [25] (five-axis flat-end context) and the effective-cutting-shape machinery to [24]. For any implementation this is an entire missing front-end: cutter/orientation → effective cutting shape → osculating radius r₁ → per-point direction maximizing strip width. No formulas from [24]/[25] are reproduced.
2. **Poisson boundary conditions and the constant nullspace.** No BCs are ever stated. The natural reading of unconstrained Eq. 14 is a pure-Neumann problem: φ determined only up to an additive constant, so the discrete system is rank-deficient — the implementer must pin a vertex, project out the constant, or use a least-squares solve, and must also handle the compatibility condition (∫∇·V vs boundary flux). None of this is discussed.
3. **Path extraction schedule.** "A certain number of points" per iso-curve — count unspecified; starting level l₁ unspecified; the exact per-point increment formula at step (2) unspecified (implied |Δl| = √h·(local rate), but not written); behaviour when an iso-level splits into several components unspecified beyond citing the marching-triangle algorithm of [6].
4. **Path ordering, linking, and machining direction.** No traversal order over {l_i}, no link-move generation between curves or between components of one level set, no climb/conventional choice, no feed-direction sign along a curve. The "no ordering needed" claim (§1) concerns the optimisation, not producing a machinable program.
5. **√(k_s + 1/r₁) degeneracy.** Where k_s + 1/r₁ ≤ 0 the target magnitude (Eqs. 9/10/13) is undefined and the scallop model breaks. Never mentioned. For a 3-axis ball-end this coincides with the local gouge condition (concave radius of curvature ≤ r), so it is guardable by gouge-checking; for flat-end (large effective radius on concave regions) it is a live degeneracy with no stated handling.
6. **k_s estimation on a mesh.** Normal curvature perpendicular to the feed direction, per point/triangle — no discrete estimator named (curvature tensor fitting? cotan-based? from [27]?).
7. **Segmentation degrees of freedom.** k for K-Means never stated and no selection rule given ("three patches were identified" — how k = 3 was chosen is not explained); the Eigenmaps graph construction details (which Laplacian, neighborhood) beyond "using the above metric as the weight function" unspecified; behaviour of the 1D embedding for surfaces where one eigenvector is insufficient unaddressed.
8. **Direction-field post-processing.** Laplacian smoothing of transported directions (§4.2) — iteration count/weights/stopping criterion unspecified; interaction between smoothing and the consistency/flip step unspecified.
9. **Singular-region boundary.** How "singular region" membership is decided numerically (a strip-width isotropy threshold?) is not stated; only the extrapolation mechanism (copy + unfold–translate–fold) is given.
10. **Vector field placement and Eq. 17's indexing.** Whether D/V live per-triangle or per-vertex is only implied (per-triangle by Fig. 5b and §4.2); Eq. 17 sums over triangles k but uses V_j inside — an apparent typo the implementer must resolve (standard cotan divergence from [27] is the obvious intent, but that is inference, not the text).
11. **Solver specifics.** Direct vs iterative, tolerances, conditioning claims ("well-conditioned") unquantified; no timings, no mesh sizes, so no cost model can be extracted.
12. **Outer-boundary path behaviour.** No discussion of trimming, of paths meeting the surface boundary, of margin/containment, or of multiply-connected domains (holes) anywhere in the paper.
13. **Curved-distance vs chordal side-step.** Eqs. 2–5 use Euclidean ‖p₂ − p₁‖ with arc-chord conflation acknowledged only via the O(³) terms and the Eq. 4 parenthetical; validity when side-step is not ≪ 1/k_s is only bounded empirically (case 3, ≤ 0.1 mm constraints).
14. **Constant tilt/inclination assumption.** Case 1 fixes tilt 0°/inclination 30°; how the framework behaves under varying cutter orientation (which changes r₁ pointwise and couples orientation planning to the field) is not developed — orientation is treated as input.

## Implementability note for rs_cam (3-axis ball-end, triangular mesh region)

Favourable: ball-end is the paper's own best case (Eq. 5 degenerates to the classic Eq. 2; the effective-cutting-shape machinery of [24] becomes unnecessary — r₁ = r exactly); every computational block is standard (per-vertex curvature estimation for k_s, cotan Laplacian + divergence, one sparse SPD-style solve, marching-triangles iso-extraction); 3-axis fixed orientation removes gap 14. What must be invented locally: BC/nullspace pinning (gap 2), extraction schedule + linking + machining direction (gaps 3–4), the D field itself (gap 1 — though any repo-chosen field, e.g. curvature- or kinematics-aligned, slots in per §3.3), the k_s + 1/r ≤ 0 guard (gap 5, coincides with gouge check for ball-end), and boundary/hole handling (gap 12). The paper plans **cutter-contact** paths on the design surface; a ball-end cutter-**centre** path is the r-offset along the surface normal — trivial per point, but it is an rs_cam step, not a paper step, and self-intersection of the offset in tight concave regions is again outside the paper's scope. Patch-border transitions are the authors' own admitted "serious limitation"; a single smoothly-varying region avoids segmentation entirely (case 1 needed none).
