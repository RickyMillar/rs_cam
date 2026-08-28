# Extraction — arXiv:2504.06310v2

**Paper**: Shen, Changqing; Xu, BingZhou; Zhang, Xiaojian (corresponding); Yan, Sijie; Ding, Han (State Key Laboratory of Intelligent Manufacturing Equipment and Technology, HUST, Wuhan). *Conformal Slit Mapping Based Spiral Tool Trajectory Planning for Ball-end Milling on Complex Freeform Surfaces.*

**Fetch metadata** (recorded 2026-08-29):
- Version fetched: **v2** (v1 submitted 2025-04-08, v2 revised 2025-04-13; v2 comment: "The revised manuscript has improved the quality of the figures").
- Category: cs.GR. **No journal reference or journal DOI listed** on the abs page — only the arXiv DOI 10.48550/arXiv.2504.06310. Word-produced manuscript PDF, **53 pages** (A4, double-spaced; body ≈ pp. 1–31, references pp. 32–36, figure captions pp. 37–38, Appendices A/B/C pp. 39–53).
- Companion prior work this paper extends (and repeatedly defers to): **[14] C. Shen, S. Mao, B. Xu, Z. Wang, X. Zhang, S. Yan, H. Ding, "Spiral complete coverage path planning based on conformal slit mapping in multi-connected domains", Int. J. Robot. Res. 43 (2024) 2183–2203, DOI 10.1177/02783649241251385** — the 2D CSM-SCCPP method.

**Extraction discipline**: everything below is what the fetched v2 text says, with §/Eq/page cites. Where the printed text is internally inconsistent it is recorded **as printed** and flagged, not repaired. Gaps are collected in the mandatory final section.

---

## 0. Headline finding for implementers

**The conformal slit map construction itself is NOT in this paper.** The generalized Neumann kernel method — the actual integral equation, kernel, discretization, solver, and interior-point evaluation — is entirely deferred to [11] Nasser, *J. Sci. Comput.* 78 (2019) 582–606; [28] Yunus, Murid & Nasser, *Proc. R. Soc. A* 470 (2014) 20130514; and the authors' own [14] (boundary parameterization "adheres to the standard approach outlined in Equation A-14 of Shen et al. [14]", p. 9; the bridging blend function σ(t) is "as outlined … in Equation A-11" of [14], p. 16). This paper's own contributions are: (i) a BFF-based mesh front-end so the 2D boundary-input slit mapper can serve a triangular mesh; (ii) barycentric transfer for inverse mappings; (iii) binary-search trajectory spacing against sampled 3D coverage; (iv) log-rectangular spiral bridging that dodges slits; (v) an energy criterion + modified gradient descent for placing the origin-mapped point O^F. An implementation needs at minimum [14] and [11] (and/or [28]) in addition to this paper.

---

## 1. Problem formulation (§1.1, pp. 3–4; §2.1, pp. 7–9)

- **Surface class**: freeform surfaces represented as **triangular meshes** with **m+1 boundaries** Γ_i, i = 0,1,…,m (m > 0), i.e. **multiply connected / perforated ("3D perforated surface milling")** (Abstract; §2.1 p. 8). Also demonstrated: low-quality meshes with elongated facets (Fig. 7(d)–(f)) and a genus-3 surface after cutting along a tunnel loop (Fig. 7(i), §3.1 p. 24). One boundary Γ_0 is selected as the outer boundary of the mapped domain; "Typically, the longest boundary is chosen as Γ_0" (p. 8).
- **What the spiral must satisfy** (§1.1, §2.2.2, §4.1): a **single continuous spiral** trajectory embedded in the perforated surface **without cellular decomposition or additional boundaries** (Abstract); complete coverage — every sampled point of the iso-scallop surface swept (§2.2.1, Eqs 1–4); scallop height kept below a preset maximum h (achieved band stated in Summary §4.1 as "within the range of −0.5% to 12%" of nominal); no passing through holes (bridge curves shifted until they avoid slits, §2.2.2/Fig. 3(c)(f)); smooth transitions, minimized tool lifts (§2.2.2 p. 14, §4.1 p. 31). Self-intersection avoidance is claimed as a property of the concentric iso-parametric structure ("conform to the boundaries of S, avoid intersecting holes, and do not overlap", §2.2 p. 10) — no explicit self-intersection test is described.
- **Three stated challenges** (§1.1 pp. 3–4): (a) CSM via generalized Neumann kernel takes only 2D boundary inputs, not meshes; (b) trajectory spacing under scallop constraint when spacing "may experience sudden variations due to surface discontinuities"; (c) placement of the origin-mapped point, "an optimal strategy for positioning this point remains unexplored".

## 2. The conformal slit mapping construction (§2.1, pp. 7–10; Fig. 1)

Pipeline as stated (all notation as printed):

1. **Flattening**: the BFF algorithm ("Boundary First Flattening", Sawhney & Crane [22]) flattens mesh S to planar mesh S^F; mapping ω_F : S → S^F; boundaries map to Γ_i^F (p. 8). Which BFF boundary-condition mode is used is not stated.
2. **Boundary parameterization**: Γ^F = Γ_0^F ∪ … ∪ Γ_m^F, each Γ_i^F(t) parameterized 2π-periodically, t ∈ J_i = [0, 2π], "adheres to the standard approach outlined in Equation A-14 of Shen et al. [14]" (p. 9). If Γ_i^F(t) has C1-discontinuous corner points, the **Nyström parameterization method** [38 = Kress, *Numer. Math.* 58 (1990) 145–161] is used "to enhance the accuracy of convergence"; worked example: 128 uniformly distributed parameter points Γ_4^F(i/128 · 2π), i = 1…128 (p. 9, Fig. 1(b)).
3. **2D conformal slit mapping** of S^F by the **generalized Neumann kernel method** [11, 28], in one of two modes (p. 9):
   - **Disk conformal mapping**: reference point O^F selected within S^F − Γ^F → maps S^F onto the **unit disk D̄ with concentric arc slits** (each interior hole boundary Γ_i^F, i ≥ 1, becomes a circular-arc slit).
   - **Annular conformal mapping**: reference point Z_1 chosen inside a hole region S^F(Γ_1^F)+…+S^F(Γ_m^F) − Γ^F → maps onto the **unit annulus Ā** (inner circular boundary of radius R_A) with the remaining holes as arc slits.
   - Notation standardized: O^F ∈ S^F(Γ_0^F) − Γ^F; whether disk or annular mode results "depends on the placement of O^F" (if it lands inside a hole → annular; Fig. 1(b)(d) shows both). The slit mapping is ω_S : S^F → (S^S; O^F); O^F maps to the origin O^S of S^S.
   - **No equations for ω_S are given in this paper** — no integral equation, no kernel, no linear system. (The generalized Neumann kernel approach in [11,28] is a boundary integral equation solved on the parameterized boundaries; this paper only consumes it.) How interior mesh vertices of S^F are pushed through ω_S (interior evaluation of the map) is not described.
4. **Inverse mappings via barycentric transfer** (p. 10): for P ∈ S^S, find its containing triangle in S^S, express P in barycentric coordinates, read off Cartesian coordinates of the same triangle index in S^F → ω_S^{-1}(P); likewise ω_F^{-1} : S^F → S. (This implies both meshes share connectivity and that ω_S has been evaluated at every mesh vertex — not stated explicitly.)
5. **Composites** (p. 10): "the composite mapping ω_FS = ω_F ∘ ω_S : S → (S^S; O^F)" — **as printed**; note the composition order contradicts the stated domain/codomain (S → S^S requires ω_S ∘ ω_F). Reverse: ω_SF^{-1} = ω_S^{-1} ∘ ω_F^{-1} : S^S → S.
6. **Normal-offset surfaces** (p. 10): ω_I : S → (S^I; I) transforms S along its normal by distance I. With I = h (max scallop height): **iso-scallop surface** S^h = ω_I(S, h). Maps S^I ↔ S^S: ω_I^{-1} ω_FS : S^I → (S^S; O^F) and ω_SF^{-1} ω_I : S^S → (S^S; I) [as printed; the last codomain is presumably S^I]. No offset-surface construction method, and no self-intersection handling for the offset, is given.

**What a "slit map" is here**: the classical conformal map of an (m+1)-connected planar domain onto the unit disk (or annulus) where the extra boundary components become concentric circular **arc slits**; iso-parameter circles centered at the origin then wind around the slits. The alternative mesh-native slit map of Yin/Dai/Yau/Gu [13] is discussed and rejected: it "allows discrete control over the central mapped position by removing a triangular facet from the surface. However, if the removed facet is excessively elongated, severe distortion may occur" (§1.3 p. 5; demonstrated Fig. 7(d)–(f)).

## 3. Spiral generation in the mapped domain (§2.2, pp. 10–17)

- **Iso-parametric curves** (§2.2 p. 10): concentric circles C_S = {C_1^S,…,C_k^S} on S^S centered at O^S with decreasing radii R_S = {R_1^S,…,R_k^S}; pulled back to S as TC = ω_SF^{-1}(C_S) — these "conform to the boundaries of S, avoid intersecting holes, and do not overlap … maintain a concentric arrangement that facilitates bridging" (pp. 10–11). The spiral is NOT an Archimedean spiral drawn in the plane; it is a set of concentric circles subsequently **bridged** into a spiral.

### 3.1 Spacing control (§2.2.1, pp. 12–14; Pseudocode A-1 pp. 39–41)

- Initial tool **center** trajectory: C_1^T = ω_SF^{-1} ω_I (C_1^S(R_1^S), K_c), K_c = tool radius; R_1^S ∈ [R_min, 1] with **R_min = 0 for disk mappings, R_min = R_A for annular** (p. 12).
- Sample the iso-scallop surface S^h into point set P^h = {P_1^h,…,P_{N_S}^h}, each point stored with its mesh element + barycentric coordinates "to eliminate redundant coordinate transformations" (p. 12). How the N_S samples are placed on S^h is not stated.
- **Eq (1)**: P^{h,C_1^T,K_c+} = { P_i^h ∈ P^h | ‖P_i^h − C_1^T‖₂ > K_c } — the points of S^h at Euclidean distance > K_c from the tool-center curve, i.e. **not swept by the ball cutter** (a scallop-surface point within K_c of the ball-center path is covered).
- **Eqs (2)–(3)**: map P^h and the uncovered subset into the slit domain: P^S = ω_I^{-1} ω_FS (P^h, O^F); P^{S,C_1^T,K_c+} = ω_I^{-1} ω_FS (P^{h,C_1^T,K_c+}, O^F).
- **Eq (4)**: P^{P^S,C_1^S} = { P_i^S ∈ P^{S,C_1^T,K_c+} | ‖P_i^S − O^S‖₂ > R_1^S } — uncovered points lying **outside** circle C_1^S.
- **Binary search** on R_1^S within [R_min, 1] "ensuring that P^{P^S,C_1^S} is exactly empty" (p. 12) — i.e. the outermost circle is pushed inward until every point it fails to cover lies strictly inside it (to be covered by later passes). Then set P^h ← P^{h,C_1^T,K_c+}, search R_2^S in [R_min, R_1^S], and repeat "for each successive R_i^S until P^h = ∅" (p. 13). Termination tolerance: pseudocode A-1 line 12 stops each binary search when distance(C_{k_old}^T, C_k^T) < ε.
- **This is the entire scallop/distortion-compensation mechanism**: there is no conformal-distortion-factor formula anywhere; correct 3D spacing under mapping distortion is achieved empirically by checking Euclidean coverage of 3D sampled points. KD-trees on the discretized tool-center curve (N_C points) accelerate the distance queries (Pseudocode A-1 lines 6–7).
- Outputs per pass (p. 13, Eq (6)): tool contact trajectories TC = {ω_SF^{-1}(C_i^S)}, tool center trajectories TB = {C_i^T}, tool axis directions TA = {C_i^A→}; milling bands BP = {BP_1,…,BP_k} on S^h (band = points first covered by that pass; A-1 line 20: BP_i = P^h − P^{h,C_1^T,R_C+}).
- **Complexity** (p. 13 & Appendix A pp. 41–42): O(k · N_S · log(N_C) · log(1/ε)); "the average value of N_S stabilizes at roughly one-third of its initial value" across iterations.
- **Tool axis** (Eq (5), p. 13): C_i^A→ = N_i^A→ cos(β) + T_i^A→ sin(β); N_i^A→ = surface normal at contact point, T_i^A→ = feed direction, β = lead angle. "A commonly used tilt angle of 0° and a lead angle of 15° are chosen" (citing Sun & Altintas 2016 [9]).

### 3.2 Spiral bridging (§2.2.2, pp. 14–17; Fig. 3; Pseudocode A-2 pp. 41–44)

- **Eq (7)**: S_0^R = arg(S^S) + i·|S^S| — a log-polar-style unrolling of the slit domain into a rectangle: real axis = angle, imaginary axis = radius/modulus. **Arc slits become linear (horizontal) slits.** Mapping ω_R : S^S → S_0^R. Translate copies by 2πi along the real axis: S^R = … ∪ S_{−1}^R ∪ S_0^R ∪ S_1^R ∪ … (p. 14). Iso-parameter circles C_i^S become horizontal lines at imag = R_i^S (dashed lines, Fig. 3(a)).
- **Eq (8)** (inverse): S^S = imag(S^R) · e^{i·real(S^R)}; ω_R^{-1} : S^R → S^S. (Consistent inverse of Eq (7): real(w) = angle, imag(w) = modulus.)
- **Bridge construction, Step 1** (pp. 15–16): the spiral is formed by connecting line i to line i+1 with a smooth transition curve. Endpoints P_End^1 (on imag = R_1^S) and P_Start^2 (on imag = R_2^S) with real(P_Start^2) = D_{1−2}^{Real} + real(P_End^1); P_Start^1 = P_End^1 − 2π; the segment P_Start^1→P_End^1 is straight line L_1^1. Initial D_{1−2}^{Real} = π/10. Transition curve (**Eq (9)**):
  L_{i+1}^i = real( P_End^i + t(P_Start^{i+1} − P_End^i) ) + i·imag( P_End^i + (P_Start^{i+1} − P_End^i)/(2π) · σ(2πt) ), t ∈ [0,1]
  where σ(t) is a blend "maintaining tangency to both parallel lines by refining the function σ(t) in Equation A-11 as outlined by She et al. [14]" [sic — "She" for "Shen"]. σ is not given in this paper.
- **Slit avoidance** (= hole avoidance): "when L_2^1 intersects a slit on S^R, the bridge trajectory … crosses through the gap, resulting in discontinuity. To maintain continuity … D_{1−2}^{Real} can be incrementally increased, shifting L_2^1 to the right until it bypasses the slit" (p. 16; Fig. 3(c)(f)). Increment step: π/50 (Pseudocode A-2 line 17).
- **Coverage repair at bridges, Step 2** (p. 16): between successive rings, additionally increase D_{i−i}^{Real} (the along-line shift before the transition) from 0 "until the milling bands of the ball center trajectory ω_SF^{-1} ω_I (ω_R^{-1}(L_1^0 ∪ L_1^1 ∪ L_2^1 ∪ L_3^2), K_c) fully encompass the milling band BP_2 on S^h" — i.e. the spiralized path (which departs from the pure circles) is re-verified to sweep each band; the shift is grown until no unswept region remains between bands (Fig. 3(b)(e)). Then re-check slit intersection; iterate.
- **Near-center smoothing** (p. 17): "as the bridge trajectories approach the center of the spiral, the abrupt turns in the corresponding C_i^T make it challenging to maintain smooth transitions … when the turning radius of C_i^T is small, the initial value of D_{i−i+1}^{Real} is increased from π/10 to 2π". Pseudocode A-2 lines 6–9 give the concrete rule: **if R_i^S > 0.3 then D_{i−i+1}^{Real} = π/10, D_{i+1−i+1}^{Real} = 8π/5; else D_{i−i+1}^{Real} = 2π, D_{i+1−i+1}^{Real} = 0.**
- **Start-angle selection** (p. 17; A-2 lines 3, 30–35): real(P_Start^1) swept over [0, 2π] in steps of **π/50**; for each candidate the whole spiral is built and the total 3D length of ω_SF^{-1} ω_R^{-1}(∪L) evaluated; the minimum-length spiral L_best is returned. The prose calls this "trial and error within the interval [0, 2π]".

## 4. Origin-mapped point optimisation (§2.3, pp. 17–22; Appendices B–C, pp. 45–53)

**Motivation** (§2.3 p. 17): placing O^F at the centroid O_1^F of S^F "results in non-uniform spacing, producing sparser trajectories on the left and denser ones on the right … amplifies variations in trajectory scallop height and extends the overall trajectory length (1214.21 in Fig. 2(b) vs. 1013.11 in Fig. 2(d))" [no units stated].

### 4.1 Evaluation criterion (§2.3.1, pp. 18–19)

- Construct scalar field T on S with |dT| ≠ 0 on S − Γ. For P^S ∈ S^S at distance x(P^S) ∈ [R_min, 1] from O^S: **Eq (10)** T^S(P^S) = f(x); **Eq (11)** f(R_min) = 0 and f′(x) > 0, f otherwise unknown monotone. T transfers to S by topological equivalence of the meshes.
- Adjacent isocurves CL_i = {P ∈ S | T(P) = T_i}, CL_{i+1} likewise. Two machining points P_i, P_{i+1} on adjacent isocurves are "adjacent machining points" when they lie on the same meridian: **Eq (12)** real(exp(ω_FS(P_i, O^F))) = real(exp(ω_FS(P_{i+1}, O^F))) [as printed; prose: "belong to the same meridian on S^S" — see gaps for the operator-chain concern].
- **Scallop height, Eq (13)** [citing 31 = T. Kim, *CAD* 39 (2007) 477–489]:
  h = (K_s + K_c)/8 · ‖P_iP_{i+1}→‖₂² + o(‖P_iP_{i+1}→‖₂³)
  "K_c represents the ball-end mill radius, K_s denotes the normal curvature of the surface S at P_i in the direction of P_iP_{i+1}→" (p. 18). **As printed this sums a curvature (1/length) and a radius (length)** — dimensionally inconsistent; the standard iso-scallop relation would read (κ_s + 1/K_c)·ℓ²/8. Recorded verbatim; flagged in gaps. No convex/concave sign discussion appears.
- **Eq (14)** (Taylor, P_{i+1}→P_i along ∇T): ‖∇T(P_i)‖₂ · ‖P_iP_{i+1}→‖₂ + o(‖·‖²) = |T_{i+1} − T_i|.
- **Eq (15)**: h = |T_{i+1} − T_i|² · (K_s + K_c) / (8‖∇T(P_i)‖₂²). Since |T_{i+1} − T_i|² is constant between two isocurves, per-point scallop variation is captured by g := (K_s + K_c)/(8‖∇T‖₂²). The authors stress Eq (15) "excludes the spacing between adjacent tool paths, ensuring it remains unaffected by abrupt increases in actual tool path spacing" (e.g. at points either side of a slit, P_4/P_5 in Fig. 2) (p. 19).
- **Eq (16)**: Avg = (1/A_S) ∫_{S−Γ} g dS. **Eq (17)** symmetric energy: E^S = ∫_{S−Γ} ( g + 1/g · … ) dS, printed as E^S = ∫_{S−Γ} ( (K_s+K_c)/(8‖∇T‖₂²) + 8‖∇T‖₂²/(K_s+K_c) ) dS. Appendix B (pp. 45, Eqs B-1…B-6) proves via Lagrange multiplier / Euler–Lagrange that ∫(f + 1/f) dS under a fixed-mean constraint is minimized by the constant f = Avg — so E^S is minimal exactly when g is uniform, i.e. **uniform scallop height per unit isocurve increment**.
- **Eq (18)**: E^S_min(O^F) = min over f ∈ {f | f(R_min)=0, f′(D^S)>0 for D^S ∈ [R_min,1]} of E_S(f, O^F). Appendix C solves this inner problem:
  - Discretize f at p+1 nodes P_j = R_min + j(1−R_min)/p (Eq C-7), linear interpolation (Eqs C-8/C-9).
  - Discrete energy on the mesh (Eqs C-1…C-5): per-face gradient ∇T(F_i) from vertex values and rotated edges over 2A_{F_i} (Eq C-3); directional curvature K_s(∇T(F_i)) from second-fundamental-form coefficients {E,F,G} [39 = Rusinkiewicz 2004] (Eq C-4); E^S = Σ_i A_{F_i} ( (K_s+K_c)/(8‖∇T‖²) + 8‖∇T‖²/(K_s+K_c) ) (Eq C-5).
  - Iterative coordinate-descent by perturbation: for interior nodes j, three candidate perturbations δf(P_j) ∈ { (f(P_{j−1})−f(P_j))/(2D_f(Idx)), 0, (f(P_{j+1})−f(P_j))/(2D_f(Idx)) } with decay D_f(Idx) = (1.01)^{Idx} (Eq C-11); energy change δE^S computed **locally** over only the faces in the annulus An_j affected by node j (Eq C-10, complexity O(N_F^j)); fit a quadratic Qf_j(x)=Ax²+Bx+C through the three (perturbation, δE) pairs (Eq C-12) and take its constrained minimizer (Eq C-13). For the last node j=p, seven candidate perturbations (Eq C-14: δf(P_p^k) = (f(P_{p−1})−f(P_p))(1−0.9^k), k=1,2,3; δf(P_p^k) = (1.1^{k−4}−1)f(P_0) [as printed], k=4..7). Apply all node updates (Eq C-15), iterate until Σ_j |δE^S(δf(P_j))| < E_ε^S (Eq C-16).
  - Complexity: each sweep O(7N_F^p + 3Σ N_F^j) ≈ O(N_F) (Eqs C-17/C-18); iterations ~O(log(1/E_ε^S)); total **O(N_F log(1/E_ε^S))**. Fig. C-1: E^S converges exponentially and the limiting f is independent of initialization (tested f(x)=x and f(x)=100x; E^S_min 448.01 vs 449.63).

### 4.2 Outer optimisation of O^F (§2.3.2, pp. 20–22; Pseudocode C-2 p. 53)

- **Eq (19)**: O^F_opt = argmin_{O^F ∈ S^F(Γ_0^F) − Γ^F} E^S_min(O^F).
- Landscape (observed, Fig. 5): E^S_min "exhibits smoothness and convexity in the vicinity of the optimal point"; but **inside hole regions S^F(Γ_i^F), i=1..m, E^S_min is gradient-free (constant)** — "positional variations of O^F within a specific S^F(Γ_i^F) do not influence the outcomes of the conformal slit mapping calculation" (p. 21; Figs 1(b)/1(d): O_1^F vs O_5^F identical results).
- **Eq (20)**: numerical gradient by forward differences along any two orthogonal unit vectors u→, (u→)^⊥ with step ε.
- **Eq (21)**: iterative update O^F ← O^F + [ S_min^F / (100 ‖∇E^S_min(O^F)‖₂ (1.01)^{Idx}) ] ∇E^S_min(O^F), where S_min^F = radius of the largest inscribed circle in S^F(Γ_0^F). [Sign as printed — written as "+" although descent requires stepping against the gradient; Pseudocode C-2 line 9 likewise prints O^F = O^F + (λ/(1.01)^{Idx})∇E^S_min.] Stop "until E^S_min increases with respect to the position of O^F" (p. 21); C-2 line 2: while E^S_min_old − E^S_min_new > E_ε^S.
- **Hole escape, Eq (22)**: if O^F falls inside a hole S^F(Γ_i^F), jump to O^F_{i−off} = argmin_{O^F ∈ Γ_{i−off}^F} E^S_min(O^F), where Γ_{i−off}^F is Γ_i^F offset **outward** (a curve in S^F), and restart iteration (pp. 21–22, Fig. 6(a)).
- Measured cost (p. 20, Fig. 5): traversal baseline = 3,225 sampled candidate points, 1.53 s per E^S_min evaluation, ≈ **1.37 h** total, on an **Intel i5-10400F CPU + "NVIDIA GeForce GTX 1600" [sic]** desktop. Gradient method from three different starts: **67, 33, 38 evaluations; 104.98 s, 56.29 s, 60.32 s** — all ending near O^F_opt (Fig. 6(b)). §4.1 (p. 31) restates: single energy computation ≈ 1.86% of trajectory generation time; gradient search needs 2.08% of the evaluations of traversal (67/3225 ≈ 2.08%); optimisation time ≈ 2.12% of traversal time. Symmetric-surface sanity check (Fig. 9): for a centrally symmetric surface the optimum is the symmetry centre, O^F_opt = ω_F(O), and multi-start iterations converge to its vicinity.

## 5. Scallop / spacing control — guarantee status

- Formula basis: Eq (13)/(15) as above ((K_s+K_c)/8 as printed; flagged).
- Mechanism: binary-search each ring radius so the sampled iso-scallop surface is fully covered by balls of radius K_c around the tool-center path (Eqs 1–4); bridge sections re-verified for band coverage (§2.2.2 Step 2). **The bound is sampling-based and approximate**, controlled by N_S, N_C, ε: Table 1 Case 1.4 (N_C = 100) "sparse trajectory points cause computational errors in milling bands" (algorithm fails, no k reported); Case 1.5 (N_S = 8,377) "overly sparse discrete points, resulting in a small k-value and excessively large trajectory spacing" (p. 25). No rule for choosing N_S/N_C is given.
- Achieved accuracy (measured, §3.2 p. 29): nominal range [0, 0.2 cm]; Workpiece 1 (traditional) actual [−0.021, 0.216 cm] = error ratio [−0.5%, 8.0%]; Workpiece 2 (proposed) [0.013, 0.224 cm] = [0.0%, 12.0%]. "The machining scallop height error for both workpieces remained below 12%" — i.e. **the proposed method overshot its nominal scallop bound by up to 12%**; the bound is not strict.

## 6. Holes / islands / topology handling

- Holes = interior boundaries Γ_1..Γ_m; the slit map turns each into a **concentric arc slit** in the disk/annulus (one hole can instead become the annulus inner circle, §2.1 p. 9). Iso-parameter circles pass around slits by construction, so ring trajectories never enter holes (§2.2 p. 10).
- Bridges are the only place hole-crossing can occur: in the unrolled rectangle S^R a bridge L crossing a **linear slit** would "cross through the gap, resulting in discontinuity"; the fix is to **shift the bridge along the ring (increase D^{Real}) until it clears the slit** (§2.2.2, Fig. 3(c)(f)). Paths are therefore **never split and never lifted**: continuity is preserved by rerouting in the parameter domain, and the Summary claims "avoiding surface holes, thus eliminating unnecessary tool lifts" (§4.1 p. 31).
- Coverage next to slits: the same P^h coverage machinery is the only check; Eq (15)'s point-local measure is explicitly designed so that spacing jumps across slits (P_4/P_5, Fig. 2(c)) don't corrupt the uniformity metric (p. 19).
- High genus (§3.1 p. 24, Fig. 7(i)): a genus-3 surface is first **cut along a tunnel loop** ("cutting along the red tunnel loop transforms a genus-3 surface into a surface with six holes"), then poles defined and the method applied; presented as a parameterization case study (binary/monomial parameterization, 3D-printing application) rather than a machining result. How the tunnel loop is computed is not stated.

## 7. Degeneracies and failure cases (as mentioned by the authors)

- **Elongated removed facet in mesh-native CSM [13]**: "if the removed facet is excessively elongated, severe distortion may occur in the mapping near the center" — motivates using the boundary-input Neumann-kernel CSM instead (§1.3 p. 5; Fig. 7(d)–(f): low-quality mesh, CSM maps elongated central facets "causing severe iso-parametric deformation"; proposed method avoids removing the facet, "ensuring a higher conformality", lengths 284.26 vs 226.11 [no units]).
- **Corner points (C1 discontinuities) on flattened boundaries** degrade convergence of the slit-map solve → Nyström parameterization remedy (p. 9).
- **Gradient-free plateaus** of E^S_min inside hole images; remedied by Eq (22) offset-curve jump (p. 21).
- **Sparse sampling failures**: Table 1 Cases 1.4/1.5 (§3.1 p. 25), as in §5 above.
- **Abrupt turns near spiral centre** when ring turning radius is small → initial bridge shift raised to 2π for R_i^S ≤ 0.3 (p. 17; A-2 lines 6–9).
- Not addressed at all: slit-map solver failure modes, offset-surface self-intersection, gouging in concave regions, extremely elongated whole domains (beyond the O^F optimisation), non-manifold/degenerate mesh input.

## 8. Tool model and machining assumptions

- **Ball-end mill**, radius K_c (main text) / R_c (Pseudocode A-1) — symbol collision, same quantity.
- **Cutter-contact vs cutter-centre**: distinguished. Contact path C_i = ω_SF^{-1}(C_i^S(R_i^S)) lies on S; centre path C_i^T = ω_SF^{-1} ω_I (C_i^S(R_i^S), K_c) lies on the K_c-offset surface (§2.2.1 pp. 12–13, Fig. 2(b)).
- **Tool axis**: two free DOFs; fixed **tilt 0°, lead 15°** via Eq (5) (§2.2.1 p. 13). Execution on an **ABB IRB6600 robotic arm** milling platform (§3.2 p. 28) — i.e. multi-axis/robotic, **not 3-axis**; nothing in the geometry pipeline depends on Eq (5), which only orients the tool along the computed centre path.
- **Machining strip/band model**: milling band BP_i = points of the sampled iso-scallop surface S^h within K_c of centre path C_i^T (A-1 line 20); scallop model Eq (13). No flank/engagement/force model.

## 9. Numerical parameters, solver details, runtimes

- Hardware: Intel i5-10400F CPU, "NVIDIA GeForce GTX 1600" [sic — presumably GTX 1660]; nothing states which parts (if any) use the GPU (p. 20).
- **Table 1** (spacing control, p. 25): columns (h, k, N_S, N_C, ε, t₁, STR₁ = (k/t₁)N_S log(N_C) log(1/ε)):
  - 1.1: h=0.5, k=17, N_S=79842, N_C=1000, ε=0.01 → 57.19 s (7.55e5)
  - 1.2: h=0.2, k=25, same → 43.80 s (1.45e6)
  - 1.3: h=0.1, k=34, same → 61.93 s (1.39e6)
  - 1.4: h=0.1, N_C=100 → **fails** (k, t₁ blank)
  - 1.5: h=0.1, N_S=8377 → 16.46 s but k=28 with excessive spacing (failure)
  - 1.6: h=0.1, N_C=10000 → 319.03 s (correct, expensive)
  - 1.7: h=0.1, N_S=320251 → 613.51 s
  - 1.8: h=0.1, ε=0.1 → 11.42 s
  STR₁ ∈ [3.61e5, 3.78e6] "within a single order of magnitude" [as printed — that span is an order of magnitude in the ratio sense only loosely] (p. 25).
- **Table 2** (E^S_min, p. 26): (N_F, E_ε^S, t₂, E^S_min): 1406/0.1/0.31 s/452.14; 2033/0.1/1.15 s/451.14; 5039/0.1/1.85 s/450.66; 8646/0.1/4.1 s/453.61; 8646/0.01/7.54 s/453.99; 8646/0.001/8.61 s/453.99. Mesh-density sensitivity of E^S_min < 1% (450.66–453.99) (p. 27).
- Slit-map solve times, BFF times, and total end-to-end pipeline time are **not reported**.
- Bridging constants: initial D = π/10 (or 2π near centre, threshold R^S = 0.3; secondary constant 8π/5), increments π/50, start-angle sweep step π/50 (§2.2.2; Pseudocode A-2).
- Energy-solver constants: decay base 1.01 (both D_f(Idx) and the O^F step, Eq 21/C-11), step scale S_min^F/100, boundary-node perturbation constants 0.9^k / 1.1^{k−4} (Eq C-14), p (number of f nodes) **never given a value**.

## 10. Experiments

- **Numerical** (§3.1, pp. 23–27): synthetic freeform surfaces. (i) Perforated disk-like surface with ~4 holes (Fig. 7(a)): conventional annular+disk decomposition vs proposed — trajectory length **294.24 vs 245.47** [no units]; claimed smoother and more uniform. (ii) Low-quality elongated-facet mesh: CSM [13] vs proposed — **284.26 vs 226.11**. (iii) Genus-3 parameterization case study (no machining). (iv) Face-like surface (Fig. 8) for Table 1 parameter study. (v) Centrally symmetric surface (Fig. 9) validating O^F optimisation. No third-party baseline beyond "conventional techniques" (their own decomposition-based implementation, Fig. 7(b)) and CSM [13].
- **Physical cutting trial** (§3.2, pp. 28–30): ABB IRB6600 robot mill; PCB triaxial accelerometer on spindle @ 2000 Hz; 3-coordinate laser measurement platform, accuracy 20 µm. Two workpieces: WP1 = traditional trajectory (Fig. 7(b)-style), WP2 = proposed (Fig. 7(c)-style); disk-shaped pieces ≈10 cm across (axes in Fig. 11 in cm), several through-holes. Nominal scallop range [0, 0.2 cm].
  - Scallop accuracy: WP1 [−0.021, 0.216 cm] (ratio [−0.5%, 8.0%]); WP2 [0.013, 0.224 cm] (ratio [0.0%, 12.0%]) (p. 29).
  - **Uniformity metric Eq (23)**: V_max = (1/N) Σ (h_max(sp_i, R_c) − h̄_max)², h_max over a local region of radius R_c around sampled red points [R_c value not given; symbol collides with tool radius]. WP2 vs WP1: **0.135 vs 0.160, −15.63%** (p. 30).
  - From accelerometer traces (Fig. 12): machining time **242.20 s → 224.38 s (−7.36%)**; average spindle impact **1.21e−4 → 8.74e−5 m/s² (−27.79%)**; impact variance **3.43e−8 → 1.51e−8 (−55.98%)** (p. 30).
  - **Not reported**: tool radius/diameter used, workpiece material, spindle speed, feed rate, depth of cut, number of retracts (the method claims zero lifts within the spiral but no count is stated), machining-time breakdown.

## 11. Stated limitations and future work (§4.2 Outlook, pp. 31–32, close paraphrase)

- The algorithm "offers valuable insights for applications such as non-spherical tool milling, surface polishing, and three-dimensional printing" — i.e. non-ball tools are future work, not covered.
- For high-genus surfaces: "Effectively cutting and defining the north and south poles of the surface to achieve uniform parameterization is a complex higher-dimensional search problem. The optimization technique used to determine the optimal position of O^F in our approach provides a helpful reference for speeding up this process" — pole/cut-loop selection on high-genus surfaces is acknowledged open.
- No other limitations are explicitly conceded in §4; the sampling-failure modes (Table 1) and the 12% scallop overshoot are reported in §3 without being framed as limitations.

---

## Underspecified / missing steps (first-class deliverable)

Everything an implementer must source elsewhere or invent:

1. **The slit map itself is external.** No integral equation, kernel, discretization, or linear system for ω_S appears in this paper; all deferred to [11] Nasser 2019, [28] Yunus/Murid/Nasser 2014, [14] Shen et al. 2024 (Eq A-14 boundary parameterization; Eq A-11 σ(t)). Minimum reading set to implement: this paper + [14] + [11].
2. **Interior-point evaluation of ω_S is never described.** The barycentric inverse (p. 10) presupposes every mesh vertex of S^F has an image in S^S, but how vertices (as opposed to boundary points) are pushed through the boundary-integral map — Cauchy-integral evaluation, accuracy near slits/boundary — is unstated.
3. **Eq (13)/(15) as printed are dimensionally inconsistent** ((K_s + K_c)/8 mixes curvature and radius; the standard form is (κ_s + 1/r_tool)ℓ²/8). Sign/branch handling for concave vs convex regions (κ_s of either sign; gouging when concave radius < K_c) is entirely absent — no gouge check anywhere in the pipeline.
4. **Eq (12) same-meridian condition**: printed operator chain real(exp(ω_FS(P, O^F))) does not obviously extract an angular coordinate (exp of a complex point then real part); the prose intent ("same meridian") is clear, the printed math is not trustworthy as-is.
5. **Normal-offset surfaces (S^h and the K_c cutter-centre offset)**: construction on a triangular mesh, normal estimation, and self-intersection/trimming in concave regions are unspecified.
6. **Sampling rules absent**: how the N_S points on S^h are distributed, and how to choose N_S, N_C, ε for a target reliability — Table 1 shows both failure modes exist (1.4, 1.5) but gives no selection rule.
7. **Bridging magic constants** unjustified: initial D = π/10, near-centre switch at R^S ≤ 0.3 with D = 2π, secondary constant 8π/5, increments π/50, start-angle sweep step π/50. σ(t) itself deferred to [14].
8. **BFF configuration unstated**: BFF has multiple boundary-condition modes (free/prescribed); which is used, and any mesh-quality preconditions, are not given.
9. **Composition/notation defects**: ω_FS = ω_F ∘ ω_S printed with order contradicting its domains under the usual right-to-left ∘ convention (benign if read as left-to-right application — ω_F first, then ω_S — which also makes ω_SF^{-1} = ω_S^{-1} ∘ ω_F^{-1} consistent); K_c vs R_c symbol collision (tool radius) and R_c reused again in Eq (23) for the measurement-region radius (value never given); Eq (21)/C-2 print gradient *ascent* sign ("+∇E") for what the stop rule requires to be a descent step — no benign reading, the sign as printed is wrong.
10. **f-discretization node count p never given a value**; convergence of the perturbation scheme is empirical (Fig. C-1), no proof; the E^S_min landscape's convexity is observed on two examples only, and stopping "when E^S_min increases" plus multi-start is the only global-optimality hedge.
11. **Γ_0 selection heuristic** ("typically the longest boundary") — no criterion for when it is wrong, no fallback.
12. **Genus-> 0 preprocessing**: tunnel-loop computation for cutting high-genus surfaces is not described.
13. **Cutting-trial parameters missing**: tool radius, material, spindle speed, feed, DOC, retract counts; trajectory-length units in all comparisons; "GTX 1600" GPU [sic] with no statement of GPU use.
14. **End-to-end runtime** (BFF + slit map + spacing + bridging + O^F optimisation) never totalled; slit-map and BFF solve times absent.
15. **Scallop bound is approximate**: their own measurement overshoots nominal by 12% (WP2), and the claimed "−0.5% to 12%" band (§4.1) mixes both workpieces; nothing bounds the overshoot a priori.

## Implementability note for rs_cam (3-axis, cutter-centre, triangular mesh region)

The pipeline is genuinely implementable in principle for 3-axis ball-end finishing: drop Eq (5) (tool axis fixed +Z; ball-end scallop geometry is axis-symmetric so contact/centre relations survive), keep contact path + K_c normal-offset centre path, and the coverage-driven spacing needs only Euclidean distance queries (KD-tree) — all mesh-native. The hard dependency is the conformal slit map solver (generalized Neumann kernel per Nasser [11]/[14]): a nontrivial boundary-integral implementation not present in this paper. Steep-wall regions (3-axis reachability) are outside the paper's scope entirely — its demos are shallow, mostly height-field-like surfaces; nothing addresses tool-axis reachability or gouging.
