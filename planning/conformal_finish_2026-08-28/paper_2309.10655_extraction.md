# Extraction — arXiv:2309.10655v2 (= Shen et al. 2024, IJRR 43:2183–2203)

**Paper**: Shen, Changqing; Mao, Sihao; Xu, Bingzhou; Wang, Ziwei; Zhang, Xiaojian (corresponding); Yan, Sijie; Ding, Han (State Key Laboratory of Intelligent Manufacturing Equipment and Technology, HUST, Wuhan). *Spiral Complete Coverage Path Planning Based on Conformal Slit Mapping in Multi-connected Domains.*

**Fetch metadata** (recorded 2026-08-29):
- **Retrieved in full**: `https://arxiv.org/pdf/2309.10655` → cached at scratchpad `2309.10655.pdf`.
- Version fetched: **v2**, submitted 2023-09-19 (v1) / **2024-04-17 (v2)**. arXiv comment: *"47 pages, 17 figures, accepted by IJRR on April 10th"*. Categories cs.RO; cs.CG. PDF producer: Microsoft Word 2021; A4; **47 pages**, own page numbering 1–47, **Appendix pp. 39–47**.
- This is the **accepted-manuscript** version of the IJRR paper cited as **[14]** by arXiv:2504.06310. The arXiv abs page lists only the arXiv DOI (10.48550/arXiv.2309.10655); it does **not** carry the SAGE DOI 10.1177/02783649241251385 or the journal pagination 2183–2203. Section/equation numbering is the manuscript's own.
- **Numbering cross-check (the reason this fetch closes the gate)**: 2504.06310 cites "Equation A-14 of Shen et al. [14]" for boundary parameterization and "Equation A-11" for the blend σ(t). In the fetched v2, **A-14 is exactly the 2π-periodic boundary parameterization** `η(t,j) = η_j(t)` and **A-11 is exactly σ(t)**. The numbering the 2025 paper relies on is confirmed against *this* version.

**Extraction discipline**: everything below is what the fetched v2 text says, with §/Eq/page cites. Every equation reproduced here was read from the **rendered PDF pages** (Read tool, pp. 40–47 — the whole of Appendix A-1/A-2/A-3 from Eq A-2 onward), not from `pdftotext` layout output, because the layout extraction mangles every fraction. (A-1 and Fig. A1's caption are prose/summation-only and were taken from the layout text; A-1's closed-curve index `P_{j+i−k·floor((j+i)/k)}` was confirmed on the rendered p. 40 via A-4, which repeats it.) Where the printed text is internally inconsistent it is recorded **as printed** and flagged, not repaired. Gaps are collected in the mandatory final section.

---

## 0. Headline finding for implementers

**This paper contains the complete conformal-slit-map construction that arXiv:2504.06310 omits.** Appendix A-1/A-2/A-3 (pp. 39–47) give, in order: cubic-B-spline boundary fitting (A-1…A-5), 2π-periodic reparameterization with corner grading (A-6…A-14), the complex-valued function A (A-16), the **generalized Neumann kernel N** (A-17, A-18) and companion singular kernel M (A-19…A-21), the operators (A-22, A-23), the Riemann–Hilbert formulation and the right-hand sides γ for **both** the disc and annular slit maps (A-24…A-26), the **boundary integral equation** (A-30), the **Nyström discretisation with the trapezoidal rule and Wittich's method** including the exact diagonal/near-diagonal corrections (A-31…A-33), recovery of the piecewise-constant h (A-34), recovery of the mapping function ω (A-35…A-37), and **interior-point evaluation of ω and ω⁻¹ by the Cauchy integral formula** (A-38, A-39).

Two things it does **not** carry, and defers explicitly to *Nasser 2015* (ETNA — retrieved separately, see `paper_nasser2015_etna_extraction.md`):
1. the fast solve of the (m+1)n-order dense linear system (A-33 note: *"Nasser has proposed a method for fast solving (m+1)n-ordered linear equations in Eq. A-31, and see (Nasser, 2015) for details"*);
2. FMM acceleration of the Cauchy sums (A-39 note: *"The discrete form of the integral calculation in Eq. A-38 and Eq. A-39 can be accelerated by the fast multipole method, see (Nasser, 2015) for details"*).

The paper also states its own provenance: *"The CSM and its inverse mapping solution method used in this study are consistent with the method in Ref. (Yunus et al., 2014; Nasser et al., 2013; Nasser, 2015; Nasser, 2011) **except for the boundary 2π-periodic parameterization part**"* (p. 12). The B-spline/corner-graded parameterization (A-1…A-14) is the authors' own contribution on top of Nasser's machinery.

**A correction to the reading set's own premise.** `research/conformal_finish_2026-08-28.md` §B.4 and PROGRAMME.md F2 step 3 name **Nasser 2019 (J. Sci. Comput. 78:582–606)** as the integral-equation source, taking that from the 2025 paper's reference [11]. **Shen 2024 does not cite Nasser 2019 at all.** Its full set of Nasser citations is: **Nasser 2015** (ETNA, fast solution — the solver), **Nasser, Murid & Sangawi 2013** (TWMS, mapping via the *adjoint* kernel), **Nasser 2011** (JMAA, Koebe's second/third/fourth canonical slit domains), and **Nasser, Murid & Zamzamir 2008** (Complex Var. Elliptic Equ., RH problem in domains with corners — the σ(t) attribution). The construction actually used by the 2025 pipeline therefore descends from Nasser **2015/2013/2011/2008**, not 2019; the 2019 paper is, by its own abstract, the *preimage* (inverse) problem — given a slit domain, iteratively recover a smooth-bounded conformally equivalent domain. Whether [11] in the 2025 paper is a mis-citation or an additional reference cannot be determined from either text.

---

## 1. Setup and notation (Appendix A-1, p. 39)

- G is a **bounded** open m-multiply (m > 1) connected domain in C ∪ {∞}, with origin **O ∈ G**, and (m+1) boundary components ∂G = Γ = Γ₀ ∪ Γ₁ ∪ … ∪ Γ_m, all closed Jordan curves, each **smooth or segmentally smooth with p_j corner points**. Ḡ = G ∪ ∂G.
- Γ₀ is the outer curve **and it encloses the origin**. Γ₁ … Γ_m are the inner curves.
- **Orientation (load-bearing)**: Γ₀ counterclockwise; Γ₁ … Γ_m **clockwise**.
- For the **annular** slit map, an extra point **Z₁** enclosed by an inner curve is specified; that inner curve is designated Γ₁.

## 2. Boundary parameterization (A-1, pp. 39–43) — this is what 2504.06310 cites as "A-14"

### 2.1 Cubic B-spline boundary fitting (A-1 … A-5)

Boundaries in engineering "usually do not have an explicit expression, but can be fit with a combination of arcs, straight segments, and splines". Based on the Nyström method they introduce **cubic B-splines**:

**Eq (A-1)** — a cubic B-spline P(t) with k ≥ 4 control points P₀…P_{k−1}, in two forms:
- open curve: `P(t) = Σ_{j=0}^{k−4} Σ_{i=0}^{3} P_{j+i} F_{i,3}(t−j) δ(t−j)`, `t ∈ [0, k−3]`
- closed curve: `P(t) = Σ_{j=0}^{k−1} Σ_{i=0}^{3} P_{(j+i) − k·floor((j+i)/k)} F_{i,3}(t−j) δ(t−j)`, `t ∈ [0, k]`

**Eq (A-2)** — uniform cubic basis, as printed:
```
F_{0,3}(t) = (1/6)(1 − t)^3
F_{1,3}(t) = (1/6)(3t^3 − 6t^2 + 4)
F_{2,3}(t) = (1/6)(−3t^3 + 3t^2 + 3t + 1)
F_{3,3}(t) = (1/6) t^3
```
(Printed after Eq A-1 as "F_{i,3}  i = 1, 2, 3" though four bases i = 0…3 are given — an index typo, recorded not repaired.)

**Eq (A-3)** — `δ(t−j) = 1 if 0 ≤ t−j ≤ 1, else 0` (the span indicator).

**Eq (A-4)/(A-5)** — first derivative P′(t), same two forms, with
```
F′_{0,3}(t) = −(1/2)(1 − t)^2
F′_{1,3}(t) = (3t^2)/2 − 2t
F′_{2,3}(t) = −(3t^2)/2 + t + 1/2
F′_{3,3}(t) = (1/2) t^2
```

### 2.2 The smooth (corner-free) case: A-6

If p_j = 0, Γ_j is fitted to a **closed** cubic B-spline η_j(t) with k non-coincident control points, t ∈ [0, k]. Substitute t̂ for t via

**Eq (A-6)**: `t(t̂) = t̂ k / (2π)`, `t̂ ∈ [0, 2π]` (closed curve).

Renaming t̂ → t yields η_j(t), t ∈ J_j = [0, 2π]: a **2π-periodic, twice continuously differentiable parameter curve with non-vanishing first derivative η′_j(t) ≠ 0**. That non-vanishing/C² requirement is the precondition the whole integral equation rests on.

### 2.3 The cornered case: A-7 … A-10

If p_j ≠ 0, Γ_j splits into p_j segments Γ_{j,i}, i = 1…p_j, at corner points CP_{j,1} … CP_{j,p_j}. Segments may be straight lines, arcs, or splines.

- **Eq (A-7)**: a straight segment or arc is parameterized by **arc length**: `|η′_{j,i}(t)| = 1`, `t ∈ [0, Len]`, Len = total length of that segment/arc.
- A spline segment is an **open** B-spline with k control points, t ∈ [0, k−3] (per A-1).
- **Eq (A-8)**: the piecewise definition over the p_j intervals J_{j,1} … J_{j,p_j}.
- **Eq (A-9)**: per-segment rescaling, as printed:
  `t(t̂) = (t̂ − 2πi/p_j) · sup(J_{j,i})`, `t̂ ∈ [2πi/p_j, 2π(i+1)/p_j]`,
  where sup(·) is the upper bound of the interval. *(As printed this is missing the factor p_j/2π that would make each segment's parameter interval of width 2π/p_j map onto [0, sup(J_{j,i})]; see gap 3.)*
- **Eq (A-10)**: the assembled η_j(t̂), each segment i occupying `t̂ ∈ [2πi/p_j, 2π(i+1)/p_j]`, so the whole of η_j is now 2π-periodic — **but with equal parameter share per segment, not graded**.

### 2.4 The grading function σ(t) — **Eq (A-11)**, the one 2504.06310 cites

Printed rationale (p. 42): *"grading mesh parameter transformations are still needed to make arc length changes more slowly with t around corners, which allows the later solution of the slit map to converge."*

> Suppose that σ(t) represents the bijective, strictly monotonically increasing and infinitely differentiable function maps with σ(t): [0,2π] → [0,2π], **σ′(0) = σ′(2π) = 0**, defined by (Nasser et al., 2008; Kress, 1990):

**Eq (A-11)** — verbatim from the rendered page:
```
σ(t) = 2π · [v(t)]^p / ( [v(t)]^p + [v(2π − t)]^p )
```
where **p ≥ 2 is the constant grading parameter**, and

**Eq (A-12)** — verbatim from the rendered page:
```
v(t) = (1/p − 1/2) · ((π − t)/π)^3 + (1/p)·((t − π)/π) + 1/2 ,    t ∈ [0, 2π]
```

*(A-12 is the one equation where the `pdftotext` layout output was unreadable — `(𝑝 − 2)(𝜋) + 𝑝𝜋 + 2` — and the rendered page was decisive. It is the standard Kress corner-grading substitution; the paper attributes it to Nasser–Murid–Zamzamir 2008 and Kress 1990.)*

**Eq (A-13)** — corner-graded reparameterization of η_j (attributed to Liesen, Sète & Nasser 2017), piecewise per segment k = 0 … p_j − 1:
```
        σ(p_j · t̂)/p_j                                              t̂ ∈ [0, 2π/p_j]
t̂̂ =    σ(p_j (t̂ − 2π/p_j))/p_j + 2π/p_j                            t̂ ∈ [2π/p_j, 4π/p_j]
        ⋮
        σ(p_j (t̂ − 2π(p_j−1)/p_j))/p_j + 2π(p_j−1)/p_j             t̂ ∈ [2π(p_j−1)/p_j, 2π]
```
Renaming t̂̂ → t gives η_j(t), t ∈ J_j = [0, 2π], with the two stated corner properties:
```
η_j(i·2π/p_j) = CP_{j,i}   and   η′_j(i·2π/p_j) = 0 ,    i = 0, 1, …, p_j.
```
So the corner points land on evenly divided parameter values and the parameterization **stalls (zero speed) exactly at each corner**, clustering discretisation nodes there. Fig. A1 illustrates Γ₁ of Fig. 2(a) discretised into **128 points**, "the discrete points near the corners are denser".

### 2.5 **Eq (A-14)** — the citation target

```
η(t, j) = η_j(t)    t ∈ J_j = [0, 2π]    j = 0, 1, …, m
```

i.e. A-14 is not a formula with content of its own: it is the **assembly statement** that, after A-1…A-13, all m+1 boundary components (smooth or segmentally smooth) carry one common 2π-periodic parameterization η indexed by the boundary index j. The *content* the 2025 paper leans on is A-1…A-13 (B-spline fit + arc-length segments + corner grading), which A-14 names.

## 3. Solving the slit map on the boundary (A-2, pp. 43–46)

### 3.1 Parameter domain

`J = ⨆_{j=0}^{m} J_j = ⋃_{j=0}^{m} {(t,j) : t ∈ J_j}` — the **disjoint** union, space [0,2π] × {0,…,m}; the index j is carried implicitly with t. **Eq (A-15)** writes η in the disjoint piecewise form.

### 3.2 The function A — **Eq (A-16)**

```
A_j(η_j) = e^{ i(π/2 − θ_j(η_j)) } · η_j     if G is bounded
         = e^{ i(π/2 − θ_j(η_j)) }           if G is unbounded
```
for j = 0, 1, … m, where θ_j are **oblique angles** on Γ_j.

Specialisation actually used (p. 43): *"Since we only use two typical types of slit maps with bounded G and constant oblique angles (see Eq. 1.3 in (Nasser, 2015)), θ_j = π/2, j = 0,1,…,m, so only the complex-valued function A_j needs to be defined on Γ_j by A_j = η_j, which means **A(t) = η(t), t ∈ J**."*

> **Implementer note (cross-source, flagged as inference):** Nasser 2015 Eq (8) writes the bounded case as `A(t) = e^{i(π/2 − θ(t))}(η(t) − α)` with α a fixed point of G. Shen's A-16 drops the `− α`, which is the **α = 0** specialisation — consistent with A-1's stipulation that the origin O ∈ G. Shen never says "α = 0"; this is the only reading that makes A-16 and Nasser's Eq (8) agree, and it matters because A appears in every kernel.

### 3.3 The generalized Neumann kernel — **Eqs (A-17), (A-18)**

```
N(s,t) = (1/π) Im( A_s η′_t / ( A_t (η_t − η_s) ) )
       = (1/π) Im( η_s η′_t / ( η_t (η_t − η_s) ) )       (s,t) ∈ J × J      (A-17)
```
(the second form is the A = η specialisation). N is **continuous**, with the diagonal value

```
N(t,t) = (1/π)( (1/2) Im(η″_t / η′_t) − Im(A′_t / A_t) )
       = (1/π)( (1/2) Im(η″_t / η′_t) − Im(η′_t / η_t) )                       (A-18)
```

Note both A-17 and A-18 need **η″**, hence the C² requirement on the B-spline parameterization. (A cubic B-spline is C² at knots, so A-1 is exactly at the smoothness floor.)

### 3.4 The companion kernel M — **Eqs (A-19), (A-20), (A-21)**

```
M(s,t) = (1/π) Re( A_s η′_t / ( A_t (η_t − η_s) ) )
       = (1/π) Re( η_s η′_t / ( η_t (η_t − η_s) ) )       (s,t) ∈ J × J      (A-19)
```
M is **singular**. Within a single subspace J_j × J_j:
```
M(s,t) = −(1/2π) cot((s−t)/2) + M₁(s,t)     (s,t) ∈ J_j × J_j , j = 1,2,…,m   (A-20)
```
with M₁ continuous and diagonal value
```
M₁(t,t) = (1/π)( (1/2) Re(η″_t / η′_t) − Re(A′_t / A_t) )
        = (1/π)( (1/2) Re(η″_t / η′_t) − Re(η′_t / η_t) )                      (A-21)
```
*(A-20 is printed with the range "j = 1,2,…,m", omitting j = 0; Nasser 2015 Eq (13) states it for s,t in the same J_j with no such exclusion. Recorded as printed — see gap 4.)*

### 3.5 The operators — **Eqs (A-22), (A-23)**

```
Nμ(s) = ∫_J M(s,t) μ(t) dt      (A-22)   ← AS PRINTED
Mμ(s) = ∫_J M(s,t) μ(t) dt      (A-23)
```
**A-22 is a printed typo**: the Fredholm operator N is defined with the kernel M. Nasser 2015 Eq (19) has `Nμ(s) := ∫_J N(s,t)μ(t)dt`. Recorded as printed; the intended kernel is N. This is the only place in the appendix where the printed math is outright wrong rather than merely ambiguous.

### 3.6 Riemann–Hilbert formulation and the two right-hand sides — **Eqs (A-24) … (A-27)**

Solving ω(z) analytic in G is transformed into the RH problem
```
Re[A f] = γ   on Γ                                                             (A-24)
```
with f the intermediate function and γ real-valued on Γ.

- **Disc slit map** ω: Ḡ → D̄ (unit disc with concentric circular-arc slits):
  ```
  γ(t) = − Re[ log( η(t) ) ]                                                   (A-25)
  ```
- **Annular slit map** ω: Ḡ → Ā (unit annulus with arc slits), with the designated interior point z₁:
  ```
  γ(t) = − Re[ log( 1 − η(t)/z₁ ) ]                                            (A-26)
  ```

**Solvability**: *"Eq. A-24 is not uniquely solvable for the winding number 𝓀 = 1 in our cases (see Eq. 1.2 in (Nasser, 2015)), but a unique undetermined real piecewise function h(t) = C_j, t ∈ J_j, j = 1,2,…,m, C_j ∈ ℝ can be found"* which makes
```
Re[A f] = γ + h   on Γ                                                         (A-27)
```
uniquely solvable. *(h is stated over j = 1…m; it is used at j = 0 in A-37's `c = e^{h(η₀)}`, so the index range as printed is inconsistent — see gap 5.)*

### 3.7 The integral equation — **Eqs (A-28), (A-29), (A-30)**

```
μ = Im[A f]   on Γ                                                             (A-28)
f = (γ + h + i μ) / A   on Γ                                                   (A-29)
```
and μ is the unique solution of
```
μ(t) − N(s,t) μ(t) = − M(s,t) γ(t)     t ∈ J, (s,t) ∈ J × J                    (A-30)
```
i.e., in operator form, **(I − N)μ = −Mγ** — identical to Nasser 2015 Eq (1). *(A-30 is printed in pointwise-looking notation with both s and t free; it is an operator equation, integrals over t, evaluated at s. Read via A-22/A-23.)*

### 3.8 Discretisation — **Eqs (A-31), (A-32), (A-33)**

Boundary parameterized by A-14, then each J_j is discretised into **n evenly divided parameters**:
```
J = ⋃_{j=0}^{m} ⋃_{k=0}^{n−1} ( 2πk/n , j )
```
so J has **(m+1)n** nodes and J × J has (m+1)²n². Node index `p = jn + k`.

*"By using the **trapezoidal rule and Wittich's discretize method**, Eq. A-30 can be written in the form of a matrix equation"*:

**Eq (A-31)**, as printed:
```
μ(t)  −  (2π/n) · [ N̂(s_i, t_j) ] · μ(t)   =   −(2π/n) · [ M̂(s_i, t_j) ] · γ(t)
```
with both matrices (m+1)n × (m+1)n and both vectors (m+1)n × 1.

**Eq (A-32)** — the N̂ matrix (diagonal dropped, since the trapezoidal weight times a continuous kernel is handled by the Nyström identity):
```
N̂(s_i, t_j) = 0            for i = j
             = N(s_i, t_j)  for i ≠ j
```

**Eq (A-33)** — the M̂ matrix, **Wittich's rule** for the cotangent singularity:
```
M̂(s_i, t_j) =
   0                                        for i = j
   M₁(s_i, t_j)                             for ⌊i/n⌋ = ⌊j/n⌋ and (rem(i,n) − rem(j,n)) is EVEN
   M₁(s_i, t_j) − (1/π) cot((s−t)/2)        for ⌊i/n⌋ = ⌊j/n⌋ and (rem(i,n) − rem(j,n)) is ODD
   M(s_i, t_j)                              otherwise
```
where ⌊·⌋ is the rounding-down operator and rem(i,n) the remainder of i divided by n. So: **same boundary component** (⌊i/n⌋ = ⌊j/n⌋) → use the split form A-20 with the Wittich even/odd rule on the cotangent part; **different components** → the kernel M is smooth, use it directly.

*(The `−(1/π)cot((s−t)/2)` in the odd branch carries a different constant from A-20's `−(1/2π)cot`; this is the Wittich quadrature weight, not a restatement of A-20. Note also `s`,`t` appear unsubscripted inside the branch — read as s_i, t_j.)*

**Solve**: *"Nasser has proposed a method for fast solving (m+1)n-ordered linear equations in Eq. A-31, and see (Nasser, 2015) for details."* No solver, size guidance, or conditioning statement in this paper.

### 3.9 Recovering h, f, and ω — **Eqs (A-34) … (A-37)**

```
h = [ Mμ − (I − N)γ ] / 2                                                      (A-34)
```
(identical to Nasser 2015 Eq (2)). Then f from A-29.

**Disc slit map** — want ω mapping Γ₀ onto the unit circle R₀ = 1 and Γ₁…Γ_m onto circular arc slits of **pending radii** R₁…R_m. The boundary values satisfy

```
Re[ ln( ω(η_j(t)) ) ] =  0            , j = 0
                      =  ln(R_j)  , 0 < R_j < 1 , j = 1,2,…,m                  (A-35)
```

With the normalisation **ω(0) = 0 and ω′(0) > 0**, ω has the unique solution
```
ω(z) = c · z · e^{ z f(z) }        z ∈ Γ                                       (A-36)
```

**Annular slit map** — Γ₀ → unit circle R₀ = 1, Γ₁ → circle of radius R₁ < 1, Γ₂…Γ_m → arc slits of pending radii R₂…R_m; boundary values also satisfy A-35. With the normalisation **ω(0) > 0**,
```
ω(z) = c · ( 1 − z/z₁ ) · e^{ z f(z) }     z ∈ Γ                               (A-37)
```
and in both A-36 and A-37, **c = e^{h(η₀)}**.

> **Note for the 2025 pipeline**: the slit radii R_j (and in particular the annulus inner radius R_A, which 2504.06310's spacing search needs as `R_min`) are called *"pending"* in A-35 and are **never explicitly recovered**. The natural reading is that R_j falls out of the computed boundary values — `ln R_j = Re[ln ω(η_j(t))]`, constant along Γ_j by construction, hence `R_j = |ω(η_j(t))|` for any node on Γ_j — but the paper does not say so. See gap 6.

## 4. Interior evaluation — A-3 (p. 47) — **Eqs (A-38), (A-39)**

*"The internal points z ∈ G, ω(z) can be solved by the Cauchy integral formula:"*

**Eq (A-38)** — forward map at an interior point:
```
             Σ_{j=0}^{m} ∫_0^{2π}  ω(η_j(t)) η′_j(t) / (η_j(t) − z)  dt
ω(z)  =     ─────────────────────────────────────────────────────────       z ∈ G
             Σ_{j=0}^{m} ∫_0^{2π}            η′_j(t) / (η_j(t) − z)  dt
```

**Eq (A-39)** — inverse map at an interior image point:
```
ω⁻¹(w) = (1/2πi) Σ_{j=0}^{m} ∫_0^{2π}  η_j(t) η′_j(t) ω′_j(η_j) / ( ω_j(t) − w )  dt
                                                            w ∈ D or w ∈ A
```

*"The discrete form of the integral calculation in Eq. A-38 and Eq. A-39 can be accelerated by the fast multipole method, see (Nasser, 2015) for details."*

**Both are quadratures of the boundary data only** — no interior mesh, no PDE solve. Discretised by the same trapezoidal rule on the (m+1)n nodes, A-38 is a ratio of two O((m+1)n) sums per query point.

- **A-38 is exactly the singularity-subtracted Cauchy formula** of Nasser 2015 Eq (84a) (bounded G): dividing by the second sum, which is the trapezoidal discretisation of `(1/2πi)∮ dη/(η−z) = 1`, is what removes the near-singular error for z close to Γ. Shen presents the ratio without saying why; the reason is in Nasser 2015 §5.
- **A-39 is printed with a defect**: `ω′_j(η_j)` appears in the numerator with no definition of ω′ on the boundary, and the factor `η_j(t) η′_j(t)` (rather than `η_j(t) ω′_j(t)`) does not match the change-of-variable that a Cauchy formula in the w-plane would produce. Recorded as printed — see gap 7.

## 5. Complexity, as the paper states it (§2.3, p. 22)

- CSM on the boundary ω(Γ): **O((m+1) n log n)**.
- Cauchy interpolation for the forward map ω(n̂) **and** the inverse map ω⁻¹(n̂) at n̂ interior points: **O((m+1)n + n̂)**.
- Both figures attributed to (Nasser, 2015). m = number of boundaries, n = discrete points per boundary, n̂ = number of points solved by Cauchy interpolation.
- The dominant cost of the *path-planning* stage is elsewhere: determining the iso-parameter radii {R_{C0}…R_{Ck}} by dichotomy, `O(log(1/ε))` per radius, each iteration requiring a **maximum inscribed circle** on a multiply connected domain (a Delaunay-triangulation-based construction, citing a MATLAB File Exchange entry, Birdal 2023).

## 6. Numerical parameters actually stated

| Quantity | Value stated | Where |
|---|---|---|
| Nodes per boundary component, n | **not stated** for any run; Fig. A1 shows one boundary "discretely into 128 points" | Fig. A1, p. 41 |
| Grading parameter p | **p ≥ 2**, "constant"; no chosen value | A-11, p. 42 |
| Oblique angles θ_j | **π/2 for all j** (both slit-map types used) | A-16, p. 43 |
| Normalisation | disc: ω(0)=0, ω′(0)>0; annular: ω(0)>0 | A-36/A-37, p. 46 |
| Linear-solve method / tolerance / iteration count | **not stated** — deferred to Nasser 2015 | A-33, p. 46 |
| Hardware | Intel i5-10400F CPU, NVIDIA GeForce GTX-1600 GPU [sic] | p. 24 |
| Path-spacing limit in the worked example | 12 mm max path spacing | p. 22 |
| Headline result | CSM path length 12581.84 vs PDE-based 14412.83 (Fig. 1, units not stated); "12.34% and 22.78%" reductions in machining time and steering impact (abstract) | Fig. 1 p. 8; abstract |

## 7. Relationship to arXiv:2504.06310 (the 2025 paper this gate serves)

| 2025 paper's use | What A-11/A-14 actually are here |
|---|---|
| "boundary parameterization adheres to the standard approach outlined in **Equation A-14**" (2025 p. 9) | Confirmed: A-14 is the assembly of the 2π-periodic parameterization; the substance is A-1…A-13 (cubic B-spline fit, arc-length segments, corner grading). The 2025 paper's separate mention of "the Nyström parameterization method [38 = Kress 1990]" for C¹-discontinuous corners is **the same mechanism** — A-11/A-12 are Kress's grading function, attributed here to (Nasser et al., 2008; Kress, 1990). |
| "maintaining tangency to both parallel lines by refining the function **σ(t) in Equation A-11**" (2025 p. 16, Eq (9)) | **σ(t) is used for a different purpose in this paper.** Here it is a *corner-grading reparameterization* of the boundary, not a bridge blend. What transfers is its shape: σ: [0,2π] → [0,2π], bijective, strictly increasing, C^∞, with **σ′(0) = σ′(2π) = 0** — exactly the property a tangency-preserving blend between two parallel lines needs. So the 2025 paper's Eq (9) is coherent with A-11, but the **"refining"** it mentions is specified in neither paper. See gap 1. |
| "the generalized Neumann kernel method [11, 28]" with no equations (2025 §2.1) | Fully supplied here: A-16…A-21 (kernels), A-24…A-30 (equation), A-31…A-33 (discretisation), A-34…A-37 (recovery). |
| "How interior mesh vertices of S^F are pushed through ω_S is not described" (gap 2 of the 2025 extraction) | **Closed by A-38.** Interior points go through the Cauchy integral formula, boundary data only. The 2025 paper's barycentric transfer then handles the mesh side. |
| `R_min = R_A for annular` in the 2025 spacing search | The annulus inner radius R_A = R₁ is a **"pending radius"** in A-35 and its recovery is not written out. See gap 6. |

## Underspecified / missing steps (first-class deliverable)

1. **The σ(t) "refinement" for bridging is in neither paper.** A-11 is a corner-grading map on [0,2π]. The 2025 Eq (9) uses `σ(2πt)` scaled by `(P_Start^{i+1} − P_End^i)/(2π)` as a tangency-preserving blend. The grading parameter p is unconstrained (p ≥ 2) and *no value is given in either paper*; p directly controls how flat the blend is at its ends and therefore the bridge's curvature profile. An implementer must choose p and validate; **[REPO]** territory.
2. **No linear solver anywhere.** A-33 defers to Nasser 2015 for the (m+1)n-order solve. No statement of system size in practice, conditioning, dense-vs-iterative, preconditioning, or accuracy target. (Recovered from Nasser 2015 — see `paper_nasser2015_etna_extraction.md` — but *not* from this paper.)
3. **Eq (A-9) is dimensionally wrong as printed.** `t(t̂) = (t̂ − 2πi/p_j)·sup(J_{j,i})` maps an interval of width 2π/p_j onto [0, (2π/p_j)·sup(J_{j,i})], not onto [0, sup(J_{j,i})]. The factor p_j/2π is missing. Intent is unambiguous; the printed formula is not usable as-is. **Segment indexing is also inconsistent between A-8 and A-10**: A-8 lists segments η_{j,1} … η_{j,p_j} (1-based) while A-10 lists η_{j,0} … η_{j,p_j−1} (0-based), and A-9's `i` is 0-based. Fig. A1 shows a 4-corner boundary with segments labelled Γ_{1,1} … Γ_{1,4}. Cosmetic, but it must be resolved before coding the interval map.
4. **Eq (A-20) index range excludes j = 0** ("j = 1,2,…,m") with no stated reason. Nasser 2015 Eq (13) states the same decomposition for any single J_j. Either a typo or an unstated special treatment of the outer boundary.
5. **h's index range (A-27: j = 1…m) versus its use at j = 0 (A-37: c = e^{h(η₀)}).** As printed, h is undefined on Γ₀ yet evaluated there. Nasser 2015 defines h = (h₀, h₁, …, h_m) over all m+1 components, which is the consistent reading, but Shen does not say so.
6. **Slit radii R_j — including the annulus inner radius R_A — are never recovered.** A-35 calls them "pending" and no equation produces them. This blocks the 2025 spacing search directly (it needs `R_min = R_A`). The obvious reading `R_j = |ω(η_j(t))|` on any node of Γ_j is an **inference, not the paper's text**.
7. **Eq (A-39) (the inverse Cauchy formula) is not trustworthy as printed.** `ω′_j(η_j)` is undefined — ω′ on the boundary is never constructed; and the integrand's `η_j η′_j` factor does not match a standard change of variables into the w-plane. Nasser 2015 §5 gives only the *forward* interior formula (its Eq 84a/85a); the inverse formulation is Shen's own presentation and this is the one place where the appendix's construction cannot be transcribed with confidence. **Load-bearing for the 2025 pipeline**, which needs ω⁻¹ to pull iso-circles back.
8. **Wittich's method is named, never defined.** A-33 gives the resulting even/odd matrix entries, which is enough to *implement*, but the quadrature's derivation, order of accuracy, and the requirement that **n be even** (implicit in the even/odd split) are unstated. Nasser 2015 cites it as [34] and likewise gives only the discretisation.
9. **No value of n is given for any experiment.** Fig. A1's "128 points" is one boundary in one illustration. There is no rule relating n to boundary complexity, corner count, or accuracy — and no accuracy measurement of the map at all in this paper.
10. **Eq (A-22) defines the operator N with the kernel M** — a printed error. Corrected only by reference to Nasser 2015 Eq (19).
11. **No conditioning or failure discussion for the map itself.** The paper never reports slit-map residuals, convergence behaviour, or what happens for near-touching boundaries or extreme aspect ratios. Nasser 2015 does (Examples 2–4) and reports a genuine failure mode; nothing of that reaches this paper.
12. **The C² / η′ ≠ 0 precondition has no enforcement procedure.** A-6 asserts it follows from a closed cubic B-spline with "k non-coincidence control points"; there is no test, and no statement of what happens if a fitted boundary violates it (self-intersection, cusp, coincident control points).
13. **Corner detection is assumed.** p_j and the corner locations CP_{j,i} are inputs; how corners are found on a real boundary (angle threshold? from the CAD source?) is not addressed. The 2025 paper inherits this — it needs corners on a BFF-flattened mesh boundary.
14. **Orientation is stated but not enforced.** Γ₀ CCW / Γ_{1..m} CW is load-bearing for every kernel sign; there is no check or normalisation step.
15. **Choice between disc and annular mode is a user input** (whether the reference point lands inside a hole). No criterion for which produces a better path; the 2025 paper inherits this and adds only its O^F energy criterion.
