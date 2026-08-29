# Extraction — arXiv:1308.5351v5 (Nasser 2015, ETNA 44:189–229)

**Paper**: Mohamed M. S. Nasser (Department of Mathematics, Faculty of Science, King Khalid University, Abha, Saudi Arabia). *Fast solution of boundary integral equations with the generalized Neumann kernel.*

**Fetch metadata** (recorded 2026-08-29):
- **Retrieved in full**: `https://arxiv.org/pdf/1308.5351` → cached at scratchpad `nasser1308.5351.pdf`.
- Version fetched: **v5 [math.NA], 14 March 2014**. **33 pages**, own page numbering 1–28 plus references. Published as *Electron. Trans. Numer. Anal.* (ETNA) 44 (2015) 189–229.
- MSC 45B05; 65R20; 30C30. Keywords as printed: *"Generalized Neumann kernel; boundary integral equations; Nyström method; Fast Multipole Method: GMRES; numerical conformal mapping."*

**Why this paper and not Nasser 2019 (J. Sci. Comput. 78:582–606)**

The 2025 paper (arXiv:2504.06310) cites the slit-map machinery as **[11] = Nasser, *Numerical Computing of Preimage Domains for Bounded Multiply Connected Slit Domains*, J. Sci. Comput. 78 (2019) 582–606** — verified verbatim from the 2025 PDF's reference list (p. 33). But the actual construction the 2025 pipeline consumes comes to it through **[14] Shen et al. 2024**, and Shen's Appendix defers its two open items — the fast linear solve behind Eq. A-31, and FMM acceleration of the Cauchy sums A-38/A-39 — to **"(Nasser, 2015)"**, which Shen's bibliography resolves to *this* ETNA paper. Shen also states its overall provenance as *"consistent with the method in Ref. (Yunus et al., 2014; Nasser et al., 2013; Nasser, 2015; Nasser, 2011)"*.

The task brief licenses this substitution ("**OR whichever Nasser paper actually carries the generalized-Neumann-kernel slit-map construction**"). It is recorded explicitly rather than silently: **this is not [11]**. Nasser 2019 is, by its own abstract, the *inverse/preimage* problem — given a slit domain Ω, iteratively find a smooth-bounded G conformally equivalent to it — a different direction of the same machinery. Retrievability of Nasser 2019 is tracked in `reading_set_gate_status.md`.

**Extraction discipline**: everything below is what the fetched v5 text says, with §/Eq cites, from `pdftotext -layout`. Notation is the paper's. Gaps are collected in the mandatory final section.

---

## 0. Headline finding for implementers

This paper is the **solver** layer under Shen's Appendix A-2/A-3. It supplies exactly the four things Shen names and does not give:

1. the **system size and structure** — dense, nonsymmetric, (m+1)n × (m+1)n;
2. the **singularity-subtracted reformulation** (its Eq 37/38) that makes the system FMM-friendly and is what actually makes corners work;
3. the **solve**: GMRES with a matrix-free product evaluated by the Fast Multipole Method, complexity **O((m+1)n ln n)**, with concrete parameter values used in its experiments;
4. the **interior Cauchy evaluation** (its Eqs 84a/85a), which is *identically* Shen's Eq A-38 — including why it is a ratio of two sums.

It also supplies what neither Shen paper has: **measured conditioning, GMRES iteration counts, CPU times, and a stated failure mode**.

The two shipped artefacts are two MATLAB functions, **`FBIE`** (Fig. 2) and **`FBIEad`** (Fig. 3), printed in full in the paper. `FBIE` solves `(I − N)μ = −Mγ` and computes `h`; `FBIEad` solves the adjoint equation `(I + N* + J)μ = γ`. **Shen's construction uses the FBIE arm.**

---

## 1. The two integral equations (§1, Eqs 1–3)

```
(I − N) μ = − M γ                     (1)      ← Shen Eq A-30
h = [ M μ − (I − N) γ ] / 2           (2)      ← Shen Eq A-34
(I + N* + J) μ = γ                    (3)      ← the adjoint arm (not used by Shen)
```

Stated applications of both: numerical conformal mapping, the Riemann–Hilbert problem, the Dirichlet and Neumann problems, mixed BVPs, potential flow.

**System size and structure (§1, verbatim intent):** *"For bounded or unbounded multiply connected domains of connectivity m + 1, discretizing the boundary integral equations (1) and (3) by the Nyström method with the trapezoidal rule yields **dense and nonsymmetric (m+1)n × (m+1)n linear systems** where n is the number of nodes in the discretization of each boundary component."*

**Convergence rate (§1):**
- boundaries of class C^{q+2} and γ of class C^q → Nyström-with-trapezoidal converges at **O(1/n^q)**;
- **analytic** boundaries and analytic γ → **exponential** convergence;
- **domains with corners** → accurate results require the trapezoidal rule *with a grading substitution* (§6, below).

## 2. Domain, parameterization, function spaces (§2.1, Eqs 4–6)

Identical structure to Shen A-1/A-15 (Shen's §A-1 is a specialisation of this):

- G is (m+1)-multiply connected in C̄, **bounded or unbounded**; for bounded G a fixed point **α ∈ G** is assumed. Boundary Γ = ∪_{j=0}^m Γ_j, closed Jordan curves. **Orientation: G is always on the left of Γ.**
- Each Γ_j is parameterized by a **2π-periodic twice continuously differentiable** complex function η_j(t) with **η′_j(t) ≠ 0**, t ∈ J_j = [0, 2π].
- **Eq (4)**: `J = ⨆_{j=0}^m J_j = ⋃_{j=0}^m {(t,j) : t ∈ J_j}` — the disjoint union (Shen's J).
- **Eq (5)**: `η(t,j) = η_j(t)`, t ∈ J_j — **this is Shen's Eq A-14, in its original form.** Shen's A-14 is a verbatim restatement of Nasser's Eq (5); Shen's contribution is the *construction* of the η_j (B-splines + corner grading), not the assembly statement.
- `H` = space of real Hölder continuous 2π-periodic functions on each J_j; `S` ⊂ H the subspace of **real piecewise constant** functions `h(t) = (h₀, h₁, …, h_m)` — **defined over all m+1 components**, resolving the index-range inconsistency flagged as gap 5 in the Shen extraction.

## 3. The kernels (§2.2, Eqs 7–18)

**Eq (7)** — piecewise constant **oblique angle** function `θ(t) = (θ₀, θ₁, …, θ_m)`, given real constants.

**Eq (8)** — the complex-valued function A:
```
A(t) = e^{ i(π/2 − θ(t)) } ( η(t) − α )    if G is bounded
     = e^{ i(π/2 − θ(t)) }                 if G is unbounded
```
> **This resolves the α question raised in the Shen extraction (§3.2 note).** Shen's A-16 writes `A_j(η_j) = e^{i(π/2−θ_j)} η_j` for bounded G — i.e. **α = 0**, consistent with Shen's stipulation O ∈ G. Nasser's Example 1 does the same (*"the function A is defined by (8) with α = 0 for bounded G"*). So α = 0 is a legitimate, exercised choice, not an error — but it is a *choice*, and A depends on it.

**Eq (9)** — the adjoint function `Ã(t) = η′(t) / A(t)`.

**Eq (10)** — **the generalized Neumann kernel** (Shen A-17):
```
N(s,t) := (1/π) Im( ( A(s)/A(t) ) · ( η′(t) / (η(t) − η(s)) ) )
```
**Eq (11)** — the companion kernel (Shen A-19):
```
M(s,t) := (1/π) Re( ( A(s)/A(t) ) · ( η′(t) / (η(t) − η(s)) ) )
```
**Eq (12)** — N is continuous, diagonal (Shen A-18):
```
N(t,t) = (1/π) ( (1/2) Im( η″(t)/η′(t) ) − Im( A′(t)/A(t) ) )
```
**Eq (13)** — M is singular; **for s, t ∈ J_j in the same parameter interval** (no exclusion of j = 0 — Shen's A-20 restricts to j = 1…m, flagged as Shen gap 4; Nasser's statement is the general one):
```
M(s,t) = −(1/2π) cot( (s−t)/2 ) + M₁(s,t)
```
**Eq (14)** — M₁ continuous, diagonal (Shen A-21):
```
M₁(t,t) = (1/π) ( (1/2) Re( η″(t)/η′(t) ) − Re( A′(t)/A(t) ) )
```
**Eqs (15)–(18)** — the adjoint kernels: `Ñ(s,t) = −N(t,s) = −N*(s,t)` and `M̃(s,t) = −M(t,s) = −M*(s,t)`.

## 4. The operators (§2.3, Eqs 19–24)

```
Nμ(s) := ∫_J N(s,t) μ(t) dt        (19)   ← Fredholm; this is the correct form of Shen's A-22
Mμ(s) := ∫_J M(s,t) μ(t) dt        (20)   ← singular; = Shen A-23
N*μ(s) := ∫_J N*(s,t) μ(t) dt      (21)
Jμ(s) := ∫_J δ(s,t) μ(t) dt        (22)
```
with **Eq (23)** `δ(s,t) = 1/(2π)` for s ∈ J_k, t ∈ J_j with k = j, and 0 otherwise; so **Eq (24)** `Jμ` is the vector of boundary-component means — a piecewise constant function.

> **Shen's Eq A-22 is confirmed a typo.** It prints `Nμ(s) = ∫_J M(s,t)μ(t)dt`; Nasser Eq (19) has the kernel N.

## 5. The trapezoidal rule (§2.4, Eqs 25–26)

**n is a given EVEN positive integer.** In each interval J_k, n equidistant nodes
```
s_{k,p} = (p−1)·(2π/n) ∈ J_k ,   p = 1, 2, …, n
```
Total (m+1)n nodes, indexed `t_{kn+p} = s_{k,p}` (Eq 25) — Shen's `p = jn + k` indexing, off by the 1-vs-0 base.

**Eq (26)** — the rule: `∫_J γ(t) dt ≈ (2π/n) Σ_{j=1}^{(m+1)n} γ(t_j)`.

> The **evenness of n** is stated here and is implicit-but-unstated in Shen's A-33 even/odd Wittich split. This is a hard precondition, not a preference.

## 6. Singularity subtraction — the step that makes it work (§3.1, Eqs 36–38)

*"We shall use singularity subtraction to rewrite the operators N and M to make these operators more suitable for using the FMM. This procedure is useful for solving the integral equation (1) for **domains with corners** … It is also useful for … **domains with close boundaries**."*

**Eq (36)** — the constant function is an eigenfunction: `∫_J N(s,t) dt = −1` and `∫_J M(s,t) dt = 0`.

Hence Eq (1) is rewritten as

**Eq (37)**:
```
2 μ(s) − ∫_J N(s,t) [ μ(t) − μ(s) ] dt = − φ(s)
```
**Eq (38)**:
```
φ(s) = ∫_J M(s,t) [ γ(t) − γ(s) ] dt
```
*"The integral equation (37) is **valid even if the boundary Γ is piecewise smooth**."*

> This reformulation is **absent from Shen's appendix**, which discretises the raw Eq A-30 directly. It is the single most important numerical detail in this paper: it is what makes the method work at corners and near-touching boundaries, and the conclusions state *"The singularity subtraction increases the accuracy significantly."* An implementer following only Shen's A-31 as printed will get the un-subtracted system.

## 7. The Nyström discretisation and the matrix (§3.2, Eqs 39–42)

Discretise (37) by (26), set s = t_i:

**Eq (39)**:
```
2 μ(t_i) − (2π/n) Σ_{j=1}^{(m+1)n} N(t_i, t_j) [ μ(t_j) − μ(t_i) ] = − φ(t_i)
```
Because N is continuous, the j = i term vanishes. With x = μ(t), y = φ(t):

**Eq (40)**:
```
( 2 + Σ_{j≠i} (2π/n) N(t_i,t_j) ) x_i − Σ_{j≠i} (2π/n) N(t_i,t_j) x_j = − y_i
```
**Eq (41)** — the matrix B, (m+1)n × (m+1)n:
```
(B)_ij = 0                     if i = j
       = (2π/n) N(t_i, t_j)    if i ≠ j
```
(identical to Shen's N̂ in A-32, up to the 2π/n factor being folded in.)

**Eq (42)** — **the linear system**:
```
( 2 I + diag(B·1) − B ) x = − y
```
The `diag(B·1)` term is the Nyström correction that the singularity subtraction produces; **Shen's A-31 has no counterpart to it**, because Shen discretises the unsubtracted equation.

## 8. Computing the right-hand side y — Wittich's method (§3.3, Eqs 43–45)

Index split `i = kn + p`, k = 0…m, p = 1…n. **Eq (44)** splits φ_k(s_{k,p}) into three pieces:
```
φ_k(s_{k,p}) = ∫_{J_k} −(1/2π) cot((s_{k,p} − t)/2) [ γ_k(t) − γ_k(s_{k,p}) ] dt      ← cotangent part
             + ∫_{J_k} M₁(s_{k,p}, t) [ γ_k(t) − γ_k(s_{k,p}) ] dt                    ← continuous, same component
             + Σ_{l ≠ k} ∫_{J_l} M(s_{k,p}, t) [ γ_l(t) − γ_k(s_{k,p}) ] dt           ← continuous, other components
```
*"The integral with the cotangent kernel in (44) can be discretized by **Wittich's method** [34]. The kernel M₁ is continuous and the kernel M is continuous for l ≠ k. So the integrals with the kernels M₁ and M in (44) are discretized by the trapezoidal rule."*

**Eq (45)** — the discrete form, with `(K)_{pq}` the Wittich quadrature weights:
```
φ_k(s_{kp}) = Σ_q [ −(K)_{pq} ] [ γ_k(s_{kq}) − γ_k(s_{kp}) ]
            + Σ_q (2π/n) M₁(s_{kp}, s_{kq}) [ γ_k(s_{kq}) − γ_k(s_{kp}) ]
            + Σ_{l≠k} Σ_q (2π/n) M(s_{kp}, s_{lq}) [ γ_l(s_{lq}) − γ_k(s_{kp}) ]
```

> **This is the same three-way case split as Shen's A-33** (i = j → 0; same component, even/odd → M₁ with/without the cot correction; different component → M). Shen presents it as matrix entries and calls it "the trapezoidal rule and Wittich's discretize method"; Nasser presents it as a quadrature of the *subtracted* integrand. **Neither paper defines the Wittich weights (K)_{pq}** — both cite it ([34] here, unnamed in Shen). See gap 1.
>
> Stated improvement over the earlier method: this construction *"can be used for all Hölder continuous functions γ **without any differentiability requirement**"*, improving on ref [25] where γ had to be C¹.

## 9. The solve — GMRES + FMM (§3.5, Eqs 55–56; §2.5, Eqs 31/35)

**Matrix-free product (Eq 55)**: `f_B(x) = (2I + diag(B·1) − B) x`, computed **without forming B**, via the FMM.

**FMM (§2.5)**: the MATLAB function **`zfmm2dpart`** from the toolbox **FMMLIB2D** [10] evaluates the complex-charge sums in **O((m+1)n)** operations:
```
Ex = zfmm2dpart(iprec, (m+1)n, a, x^T, 1)                                  (31)
Fx = zfmm2dpart(iprec, (m+1)n, a, x^T, 0,0,0, n̂, d, 1, 0, 0)               (35)   ← targets at n̂ external points
```
where `a = [real(η^T); imag(η^T)]` are the source coordinates. **FMM tolerance by precision flag**: `iprec = 1` → 0.5×10⁻³, `iprec = 2` → 0.5×10⁻⁶, and higher flags tighter (the paper's experiments use **iprec = 4**).

**GMRES (Eq 56)**:
```
x = gmres( @(x) f_B(x), −y, restart, tol, maxit )
```
restarting every `restart` inner iterations, `tol` the relative-residual tolerance, `maxit` outer iterations.

**Complexity (§3.5, verbatim intent)**: computing `f_B(x)` costs **O((m+1)n)**; computing the vector y costs **O((m+1)n ln n)**; hence solving (42) costs **O((m+1)n ln n)**. Confirms Shen's §2.3 quoted figure.

**The shipped function (Fig. 2)**:
```
function [mu,h] = FBIE(et, etp, A, gam, n, iprec, restart, gmrestol, maxit)
```
`et` = boundary parameterization η, `etp` = η′, `A` per Eq (8), `gam` = γ, `n` = nodes per component. Returns both μ and h.

## 10. Recovering h (§3.6, Eq 57)

The discretisations of `(I − N)` and `M` are `(2I + diag(B1) − B)` and `(D − diag(D1) + L̂)` respectively, so

**Eq (57)**:
```
h(t) = ( [ D − diag(D·1) + L̂ ] μ(t) − [ 2 + diag(B·1) − B ] γ(t) ) / 2
```
computable in **O((m+1)n ln n)**. Stated improvement: *"no differentiability of the function μ is required in (57)"*, unlike the earlier [25, Eq. (62)] which needed μ′.

> This is the discrete realisation of Shen's Eq A-34. Shen prints only the operator form and gives no discretisation for it; Eq (57) is it. Note h is then used as `c = e^{h(η₀)}` in Shen's A-36/A-37.

## 11. Interior evaluation — the Cauchy integral formula (§5, Eqs 78–85)

**The problem (§5, verbatim intent)**: the boundary integral equations give the map's values *on Γ only*. Interior values need the Cauchy integral formula
```
f(z) = (1/2πi) ∮_Γ f(η)/(η − z) dη ,           z ∈ G       (bounded G)         (78a)
f(z) = f(∞) + (1/2πi) ∮_Γ f(η)/(η − z) dη ,    z ∈ G       (unbounded G)       (78b)
```
*"However, the integrand in (78) becomes **nearly singular** for points z ∈ G which are close to the boundary Γ. For such case, the **singularity subtraction** can be used to obtain accurate results."*

Using `(1/2πi)∮_Γ dη/(η−z) = 1` for bounded G (**Eq 80a**) and `= 0` for unbounded G (**Eq 80b**), rewrite as **Eq (81a)** `(1/2πi)∮_Γ [f(η) − f(z)]/(η − z) dη = 0` — whose integrand has **no pole at η = z**, so the trapezoidal rule is accurate.

Discretising (Eq 83a) and solving for f(z) gives, **for bounded G**:

**Eq (84a)** — the working formula:
```
             Σ_{j=1}^{(m+1)n}  f(η(t_j)) η′(t_j) / ( η(t_j) − z )
f(z)  ≈     ─────────────────────────────────────────────────────
             Σ_{j=1}^{(m+1)n}           η′(t_j) / ( η(t_j) − z )
```

> **This is Shen's Eq A-38, exactly** — including the ratio-of-two-sums structure. Shen presents the ratio with no explanation; **the reason is here**: the denominator is the trapezoidal discretisation of the constant 1, and dividing by it is the singularity subtraction that keeps the evaluation accurate for z near Γ. An implementer must not "simplify" A-38 by dropping the denominator.

**Eq (84b)** is the unbounded-G variant (with f(∞) and a `1 + …` denominator). **Eq (85a)** vectorises over n̂ target points z₁…z_n̂, in the form the FMM consumes (`Fx`, Eq 35), giving **O((m+1)n + n̂)** — confirming Shen's §2.3 figure.

> **No inverse-map Cauchy formula appears in this paper.** Shen's Eq A-39 (`ω⁻¹(w)`) has no counterpart here; §5 covers only the forward direction f(z) for z in the *preimage* domain. Shen's A-39 remains unverified against a primary source. See gap 3.

## 12. Domains with corners (§6, Eqs 87–91) — the source of Shen's A-11/A-12

Suppose γ is smooth on each J_j except at p_j ≥ 1 points `c_{j,k} = (k−1)·2π/p_j`.

**Eq (87)** — *"the bijective, strictly monotonically increasing and infinitely differentiable function defined by [16 = Kress 1990]"*:
```
ω(t) = 2π · [v(t)]^p / ( [v(t)]^p + [v(2π − t)]^p )
```
**Eq (88)**:
```
v(t) = (1/p − 1/2) ((π − t)/π)^3 + (1/p)((t − π)/π) + 1/2 ,   t ∈ [0, 2π]
```
*"The grading parameter p is an integer such that **p ≥ 2**."*

> **Eqs (87)/(88) are character-for-character Shen's Eqs A-11/A-12** (Shen writes σ where Nasser writes ω, to avoid collision with the mapping function). This independently confirms the rendered-PDF reading of Shen's A-12, which the layout text had mangled. Both attribute it to Kress 1990.

**Eq (89)** — the per-segment grading map (Shen's A-13):
```
δ_j(t) = (1/p_j) ω( p_j (t − c_{j,k}) ) + c_{j,k} ,   t ∈ [c_{j,k}, c_{j,k+1}]
```
with the stated properties `δ′_j(c_{j,k}) = 0` at every corner and `δ′_j(t) ≠ 0` elsewhere.

**Eq (90)** — the graded trapezoidal rule: `∫_J γ(t) dt ≈ (2π/n) Σ_j γ(δ(t_j)) δ′(t_j)`, where `γ̂(τ) = γ(δ(τ))δ′(τ)` is smooth with `γ̂(0) = γ̂(2π) = 0`.

**Eq (91)** — the equivalent, and more practical, formulation: choose a piecewise smooth parameterization ζ_j of Γ_j and define
```
η_j(t) = ζ_j( δ_j(t) ) ,    j = 0, 1, …, m
```
then discretise with the **plain** trapezoidal rule (26).

> **Eq (91) is precisely what Shen does**: ζ_j is Shen's B-spline/arc-length segment parameterization (A-1…A-10) and δ_j is Shen's A-13. Shen's route is an instance of Nasser's Eq (91), which Nasser calls *"an equivalent method"* to Eq (90). Confirms Shen's approach is the sanctioned one, and confirms that after A-13 the plain trapezoidal rule (Shen's A-31) is correct.

## 13. Measured behaviour, conditioning, and the stated failure mode (§7, §8)

**Example 1** — bounded and unbounded domains of **connectivity 1089 (m = 1088)**; the bounded one has 544 circles and 545 squares (i.e. **half the boundaries have corners**). Run with `iprec = 4, restart = 10, gmrestol = 10⁻¹², maxit = 10`; largest run **n = 4096, total 4,460,544 nodes**. Reported: max error norms ‖μ − μ_n‖_∞ and ‖h − h_n‖_∞, total CPU time, and GMRES iteration counts vs node count (Figs. 6, 7). Stated result: *"the accuracy of the numerical results for the unbounded domain is better than for the bounded domain. This is expected since all the boundaries of the unbounded domain are smooth and half of the boundaries of the bounded domain have corners."*

**Examples 2–4** — connectivity 5 (m = 4) with a **variable separation distance ε** between boundaries, ε = 10⁻¹ … 10⁻⁵. Errors plotted for n = 1024, 4096, 16384. **Condition numbers** computed at n = 1024 with MATLAB `condest` (Fig. 11). Stated: *"The condition number increase as ε decreases"*; and in §8, *"For both functions, the number of GMRES iterations, the total CPU time, and **the condition number of the coefficient matrices increase as the distance ε decreases**."*

**Example 3 — the slit maps, and the stated failure mode.** Maps the bounded 5-connected domain onto (a) **the disc with circular slits** (θ constant, θ(t) = π/2 for all t — **this is Shen's case**) and (b) the disc with both circular and radial slits (θ non-constant, θ = (π/2, 0, π/2, 0, π/2)). Solver: `iprec = 4, restart = 25, gmrestol = 10⁻¹², maxit = 40`, n = 4096.

Verbatim failure statement:
> *"the function FBIE gives accurate results for the **constant** function θ. For the **non-constant** function θ, we get accurate results for ε = 10⁻¹, 10⁻². **For ε = 10⁻³ the radial slits goes outside of the unit disc and for ε = 10⁻⁴ the obtained figure is incorrect.** The function FBIEad gives accurate results for both the constant function and non-constant function θ. However, the complexity of the method based on the integral equation with the adjoint generalized Neumann kernel (3) is larger … specially for large m."*

> **Consequence for the 2025/Shen pipeline**: Shen's two slit maps both use **constant θ = π/2**, which is the arm FBIE handles accurately. The documented FBIE breakdown is in the *non-constant* θ (radial-slit) case, which Shen does not use. So the known failure mode does **not** directly apply — but the *cause* (boundaries approaching each other, ε → 0, with condition number, iteration count and CPU time all growing) does, and nothing in either paper bounds it for the constant-θ case. Two holes that nearly touch in a machining region are exactly this geometry.

**§8 Conclusions, quantitative claims**: complexity **O((m+1)n ln n)** for the generalized-Neumann-kernel equation and **O((m+1)n)** for the adjoint one; *"fast, accurate, and can be used for domains with high connectivity, complex geometry, and close boundaries"*; *"The singularity subtraction increases the accuracy significantly."*

## 14. Concrete parameter values usable as defaults

| Parameter | Value(s) used in this paper |
|---|---|
| n (nodes per boundary component) | 1024, 4096, 16384 across experiments; **must be even** (§2.4) |
| FMM precision flag `iprec` | 4 (all reported runs) |
| GMRES `restart` | 10 (Example 1), **25** (Example 3, the slit maps) |
| GMRES `gmrestol` | **10⁻¹²** |
| GMRES `maxit` | 10 (Example 1), **40** (Example 3) |
| Grading parameter p | **p ≥ 2, integer** — no chosen value reported |
| α (bounded G) | **0** in Example 1 |
| θ for the disc-with-circular-slits map | **π/2 for all t** — Shen's case |
| Condition number estimator | MATLAB `condest`, at n = 1024 |
| FMM library | FMMLIB2D [10], function `zfmm2dpart` |

## Underspecified / missing steps (first-class deliverable)

1. **Wittich's quadrature weights `(K)_{pq}` are never given.** Cited as [34] and used in Eq (45); Shen names the method and gives the resulting even/odd matrix entries (A-33) but likewise no derivation. **Shen's A-33 is the more implementable statement of the two** — an implementer should take the weights from Shen and the reason from here. Neither paper states the quadrature's order of accuracy.
2. **No preconditioner, and no statement of when GMRES fails to converge.** `maxit` is 10 in one example and 40 in another with no rule; iteration counts are plotted, not tabulated. The conclusions confirm iteration count grows as ε → 0 but give no bound, threshold, or fallback.
3. **The inverse map ω⁻¹ at interior points is absent.** §5 gives only the forward Cauchy formula (84a/85a). Shen's Eq A-39, which the 2025 pipeline needs to pull iso-circles back into the preimage domain, has **no primary-source verification** and is printed with an apparent defect (undefined `ω′_j(η_j)`). This is now the single largest unclosed item in the reading set.
4. **Constant-θ conditioning is not separately characterised.** The stated failure mode (radial slits leaving the unit disc) is a non-constant-θ phenomenon; the constant-θ arm is asserted accurate throughout Examples 3–4 but no error figure is quoted for it at small ε, only plotted. So "how close can two holes get before the disc-with-arc-slits map degrades" is measured only graphically and never stated as a number.
5. **Slit radii R_j are not recovered here either.** This paper produces boundary values of the map; extracting the arc-slit radii (and the annulus inner radius R_A) is left to the conformal-mapping papers it cites ([20, 23, 28, 37, 38]). The gap flagged in the Shen extraction (gap 6) is **not closed by this paper**.
6. **Everything is MATLAB.** `zfmm2dpart`/FMMLIB2D and `gmres` are the shipped implementation. A Rust port needs either an FMM (none in the workspace, none proposed) or acceptance of a **dense O(((m+1)n)²) matrix-vector product**. At Shen's illustrative n = 128 with m small this is trivial — a 2-hole region gives (m+1)n = 384, a 384×384 dense solve — so **the FMM is not needed at prototype scale**; it becomes necessary only at the connectivity-1089 / n = 4096 scale this paper targets. Neither paper says where the crossover is.
7. **No statement of accuracy requirements for the downstream use.** The maps are validated against analytic test functions (Eqs 92–94) with known exact μ and h ≡ 0; there is nothing about what map accuracy a *path-planning* consumer needs, nor how map error propagates into path spacing.
8. **The annular slit map is not among the examples.** §7 exercises the disc-with-circular-slits and disc-with-circular-and-radial-slits maps. Shen's annular mode (A-26/A-37) has no numerical validation in either paper.
