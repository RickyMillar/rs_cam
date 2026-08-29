# Extraction — Yunus, Murid & Nasser 2014 (Proc. R. Soc. A 470:20130514)

**Paper**: Yunus, A. A. M. (UTM Malaysia / Ibb University, Yemen); Murid, A. H. M. (corresponding, UTM Malaysia); Nasser, M. M. S. (King Khalid University, Saudi Arabia). *Numerical conformal mapping and its inverse of unbounded multiply connected regions onto logarithmic spiral slit regions and straight slit regions.*

**Fetch metadata** (recorded 2026-08-29):
- **Retrieved in full.** Bronze OA (Unpaywall `oa_status: bronze`, publishedVersion). The publisher PDF at `royalsocietypublishing.org/doi/pdf/10.1098/rspa.2013.0514` returned **HTTP 403** (Cloudflare challenge) and the NCBI PMC direct link returned an HTML bot-block page; the working route was **`https://europepmc.org/articles/PMC3896060?pdf=render`** → 2.4 MB `application/pdf`. Cached at scratchpad `yunus2014_pmc.pdf`.
- **24 pages** (`pdfinfo`; the `file` command misreports 10). Received 2 Aug 2013, accepted 4 Nov 2013. PMCID PMC3896060, PMID 24511251. DOI 10.1098/rspa.2013.0514.
- This is **reference [28]** of arXiv:2504.06310 (verified verbatim from that paper's reference list, p. 36). Shen 2024 also cites it, as "Yunus et al., 2014", among the works its slit-map method is *"consistent with"*.

**Extraction discipline**: what the fetched text says, with §/Eq cites. This was priority 3 of the reading-set gate ("if useful and quick"), so the extraction is scoped to what an implementer of the 2025 pipeline would need and what it says about the gate's four required items. Gaps section is mandatory and present.

---

## 0. Headline finding for implementers

**This is not the paper the 2025 pipeline's slit map comes from, and it should not be cited as such.** Three structural mismatches with Shen 2024 / arXiv:2504.06310:

| | Shen 2024 (what the pipeline uses) | Yunus et al. 2014 |
|---|---|---|
| Domain | **bounded** (m+1)-connected G, origin O ∈ G | **unbounded** m-connected Ω⁻ |
| Integral equation | `(I − N)μ = −Mγ` — the **generalized Neumann kernel** arm | `(I + Ñ + J)ϑ = …` — the **adjoint** generalized Neumann kernel arm (Eq 3.7) |
| Linear solve | deferred to Nasser 2015 (GMRES + FMM) | **Gaussian elimination in MATLAB** (§8, §9) |

What it *does* contribute to the gate, and contributes uniquely:

1. **A concrete value for the corner-grading parameter.** Its Eqs (8.1)/(8.2) are the same Kress grading function as Shen's A-11/A-12 and Nasser 2015's (87)/(88) — but written with the exponent **fixed at 3**. Shen and Nasser both leave it as "p ≥ 2, integer" with no chosen value. This is the only primary source in the retrieved set that pins a number.
2. **An explicit inverse-map Cauchy construction** — the item Shen's Eq A-39 states without justification and which Nasser 2015 §5 does not cover at all. Yunus's route is *different from Shen's*: remove the pole analytically first, then apply the Cauchy formula.
3. Independent confirmation of the Nyström + trapezoidal + equidistant-nodes discretisation and of the graded-mesh treatment of corners.

---

## 1. Setup (§2, Eqs 2.1–2.2)

- Ω⁻ is an **unbounded** multiply connected region of connectivity m; boundary Γ = Γ₁ ∪ … ∪ Γ_m, m Jordan curves, **all clockwise** (Fig. 1).
- Each Γ_j parameterized by a **2π-periodic twice continuously differentiable** η_j(t), t ∈ [0,2π], with **η′_j(t) ≠ 0** (Eq 2.2). Same smoothness contract as Nasser 2015 §2.1 and Shen A-6.
- Prescribed points: z₁ inside Γ₁, z₂ inside Γ₂, α ∈ Ω⁻.
- Φ(z) is determined by computing **two real functions**: an unknown S_j(t) and a piecewise constant real R_j.

**Four canonical regions** (Fig. 2): U₁ annulus with spiral slits; **U₂ unit disc with spiral slits**; U₃ spiral slits region; U₄ rectilinear slits region. Stated to be *"the same as the first 13 canonical regions shown in [36, figs 1–13]"* (Koebe's classes).

> **Relation to the arc-slit map the pipeline needs — flagged as inference, not the paper's text.** Slits are defined by `Im(e^{−iθ_j} log Φ(η_j(t))) = R_j` (Eqs 4.1, 5.1, 6.1). Setting **θ_j = π/2** gives `Im(−i log Φ) = −Re(log Φ) = −ln|Φ|`, i.e. **|Φ| constant along Γ_j — a circular arc slit**. So Shen's disc-with-circular-arc-slits map is the θ ≡ π/2 specialisation of this family, which is exactly why Shen's A-16 says *"we only use two typical types of slit maps with bounded G and **constant oblique angles** … θ_j = π/2"*. The domain class (unbounded here, bounded there) still differs.

## 2. The adjoint generalized Neumann kernel (§3)

- **Eq (3.1)**: adjoint function `Ã_{p,j}(t) = η′_j(t) / A_{p,j}(t)` — identical in form to Nasser 2015 Eq (9).
- A_{1,j}(t) and A_{2,j}(t) are assumed **constant functions**.
- `Ñ*_{p,j,l}(t,s)` is stated to be the adjoint of `N_{p,l,j}(s,t)`; the real kernel `M̃_{p,l,j}` is defined alongside.
- **Eq (3.7)** — the integral equation actually solved:
  ```
  ϑ_j(t) + Σ_l ∫ [ N_{p,j,l}(t,s) + J_{j,l}(t,s) ] ϑ_l(s) ds = − χ^{[k]}(η_j(t)) ,   j = 1,…,m
  ```
  This is Nasser 2015's **Eq (3)** arm `(I + N* + J)μ = γ`, not the `(I − N)μ = −Mγ` arm Shen discretises.
- Piecewise constants h_k, ν_k are computed from Eqs (3.9)/(3.10).

**Two integral equations are solved per canonical region** (abstract: *"For each canonical region, two integral equations are solved before one can approximate the boundary values of the mapping function"*).

## 3. The unit-disc-with-slits map (§5) — the closest analogue to Shen's disc mode

- **Eq (5.1)**: Φ maps Γ₁ onto the unit circle |Φ| = 1 and Γ_j (j = 2…m) onto logarithmic-spiral slits `Im(e^{−iθ_j} log Φ(η_j(t))) = R_j`, with **R₂ … R_m undetermined real constants**. θ₁ is chosen = π/2.
- **Eq (5.2)**: boundary values satisfy `A_{1,j} log Φ(η_j(t)) = R̂_j + i S_j(t)`, with **Eq (5.3)** `R̂_j = 0` for j = 1 and `= −R_j` for j = 2…m.
- **Normalisation**: `Φ(∞) = 0` and `lim_{z→∞} zΦ(z) > 0`. **Eq (5.4)**: `Φ(z) = c/(z − z₁) · e^{F(z)}`, F analytic in Ω⁻ with F(∞) = 0, c = lim zΦ(z) an undetermined positive real constant.
- Boundary values recovered as `Φ(η_j(t)) = e^{A_{2,j}(R̂_j + i S_j(t))}`.

> Structurally the same shape as Shen's A-35/A-36/A-37: undetermined radii on the slits, a normalisation, an exponential of a solved analytic function, and an undetermined multiplicative constant. **Here too the slit radii R_j are "undetermined constants" and no equation extracts them** — the same gap flagged in the Shen extraction (gap 6) and unclosed by Nasser 2015 (gap 5 there). Three retrieved primary sources, none of which writes down the R_j recovery.

## 4. Interior evaluation — forward and inverse (§4–§7)

**Forward, Eq (5.13)** (the U₂ case; analogous formulas at Eqs 4.17, and in §6, §7):
```
w = Φ(z) = (1/2πi) Σ_{j=1}^{m} ∫_0^{2π}  Φ(η_j(t)) η′_j(t) / ( η_j(t) − z )  dt ,   z ∈ Ω⁻
```
*"Then for all z ∈ Ω⁻, by Cauchy's integral formula [46] we have …"*

> This is the **plain** Cauchy formula, **not** the singularity-subtracted ratio-of-two-sums that Shen's A-38 and Nasser 2015's Eq (84a) use. For an unbounded region with the `1/2πi ∮ dη/(η−z) = 0` identity (Nasser Eq 80b), the subtraction takes a different form; Yunus applies no subtraction and says nothing about accuracy for z near Γ. Do **not** transplant Eq (5.13) into Shen's bounded setting — the correct bounded-case formula is A-38's ratio.

**Inverse, §5** (the construction Shen's A-39 lacks a justification for):
> *"For computing the inverse map, note that the mapping function Φ⁻¹(w) = z is analytic in the region U₂ with a **simple pole at w = 0**. Let G(w) be an analytic function in U₂ defined as **G(w) = w Φ⁻¹(w)**."*

Then, by the same Cauchy reasoning,
```
Φ⁻¹(w) = (1/2πw) Σ_{j=1}^{m} ∫_0^{2π}  Φ(η_j(t)) η′_j(t) / ( Φ(η_j(t)) − w ) · A_{2,j} S_j(t) Φ(η_j(t))  dt
```
*(as printed in the fetched text; the leading `1/(2πw)` and the trailing product factor are transcribed as they appear.)*

In §4 (annulus case) the same device is used with the pole removed at `w = c` via `G(w) = (w − c)Φ⁻¹(w)`.

> **The method is: identify where Φ⁻¹ has its pole in the canonical region, multiply it out to get an analytic G, apply the Cauchy integral formula to G, divide back.** That is the missing justification for Shen's A-39 — Shen's disc map is normalised with `ω(0) = 0`, so `ω⁻¹` has a pole at w = 0, exactly this case. **But Shen's A-39 as printed does not carry the `1/w` (or `1/(w − c)`) factor that this device produces**, which is consistent with A-39 being defective as printed. This does **not** repair A-39 — the domains differ (bounded vs unbounded) and the normalisations differ — but it identifies the shape the correct formula must have and is the nearest primary-source anchor available.

## 5. Discretisation and solver (§8)

Verbatim: *"As the boundaries Γ_j are parametrized by η_j(t) which are 2π-periodic function, the reliable method to solve the integral equations are by means of **Nyström method with trapezoidal rule** [47]. Each boundary will be discretized by **n number of equidistant points**. The resulting linear system is then solved by using **Gaussian elimination** method."*

**Corners** — the integral equations *"can be also used to compute the conformal mapping for the region that contains corner points. However, the integral equation needs to be modified slightly when η_j(t) is a corner point"*. With p_j ≥ 1 corners at 2kπ/n:

**Eq (8.1)** — the grading function, **with the exponent fixed at 3**:
```
ϖ(t) = 2π · [v(t)]^3 / ( [v(t)]^3 + [v(2π − t)]^3 )
```
**Eq (8.2)**:
```
v(t) = (1/3 − 1/2) ((π − t)/π)^3 + (1/3)((t − π)/π) + 1/2 ,   t ∈ [0, 2π]
```
with the stated properties `ϖ′(0) = ϖ′(2π) = 0`, `v(0) = 0`, `v(2π) = 1`. The per-segment map δ_j(t) is then built piecewise as `(1/p_j)ϖ(p_j t)`, `(1/p_j)ϖ(p_j t − 2π) + 2π/p_j`, … — **identical in form to Shen's A-13 and Nasser 2015's Eq (89)**.

> **This is Shen's A-11/A-12 with p = 3.** Substituting p = 3 into Shen's A-11/A-12 (or Nasser's 87/88) reproduces Yunus's 8.1/8.2 exactly. It is the only concrete grading-parameter value in the retrieved reading set.

**Eqs (8.10)–(8.14)** carry the graded-mesh substitution through into the integral equations, ending at **Eq (8.14)**, which *"can be solved by means of Nyström method with trapezoidal rule"*. Eq (3.7) is stated to be modifiable by the same procedure.

## 6. Numerical results (§8)

- Test regions of **connectivity 3 and 4** only.
- Hardware/software: Windows 7 64-bit, Intel Quad-core 2.33 GHz, 4 GB DDR3, **MATLAB R2011a**.
- **Example 8.1**: unbounded region bounded by three circles `Γ₁ = 2 + e^{−it}`, `Γ₂ = −1 + i√3 + 0.5e^{−it}`, `Γ₃ = −1 − i√3 + 1.5e^{−it}`; special points z₁ = 2, z₂ = −1 + i√3, α = 0; θ₁ = π/2, θ₂ = π/2, θ₃ = π/4; **n = 256 points**. Images onto all four canonical regions in Fig. 3, inverse transformations in Fig. 4.
- **Fig. 5** compares the **condition number** of this method's matrices against ref. [15] (the charge-simulation method). CPU times reported in seconds.
- A comparison table (p. 22 region) benchmarks against "Nasser [37]" values.

**§9 Conclusion, verbatim**: *"This paper presented two integral equations with adjoint generalized Neumann kernels … The integral equations are discretized using Nyström's method with the trapezoidal rule. For regions with corners, we use a quadratic formula based on a graded mesh. The resulting linear systems are solved by Gaussian elimination in Matlab. … The advantage of this method is that it allows to compute the values of the conformal mapping **as well as the values of its inverse**."*

## Underspecified / missing steps (first-class deliverable)

1. **Wrong domain class for the pipeline.** Every construction here is for an **unbounded** Ω⁻. Shen's G is bounded with O ∈ G. The normalisations (Φ(∞) = 0, lim zΦ(z) > 0), the pole locations, and the Cauchy identity (∮dη/(η−z) = 0 rather than 1) all differ. Nothing here transfers to the bounded case without rederivation.
2. **Wrong integral-equation arm.** This is the adjoint (`I + N* + J`) formulation. Nasser 2015 §13 measures the adjoint arm as **more expensive** than the `(I − N)` arm Shen uses (*"the number of GMRES iterations as well as the CPU time … is much larger"*), though also more robust for non-constant θ. Citing this paper for Shen's solve would be a category error.
3. **No fast solver.** Gaussian elimination on a dense system; connectivity 3–4 and n = 256 only. Nothing here about scaling, and FMM appears only in the introduction as prior work.
4. **Slit radii R_j are again "undetermined constants" with no recovery equation** (Eq 5.1, 5.3). Consistent with Shen A-35 and Nasser 2015. **Three primary sources, zero of them write down how to read R_j (or the annulus inner radius R_A) off the solved boundary values.** This remains the cleanest unclosed gap in the reading set.
5. **The interior forward formula (5.13) is un-subtracted** and the paper says nothing about accuracy for evaluation points near the boundary. Shen A-38 / Nasser 84a's ratio form is the one to implement.
6. **The inverse formula as printed carries an unexplained trailing factor** `A_{2,j} S_j(t) Φ(η_j(t))` beyond the pole-removal device the prose describes. Transcribed as printed; not independently verifiable against another source. So Shen's A-39 remains **corroborated in method but not in form**.
7. **No accuracy target or error analysis for corners**; the graded mesh is asserted, the "quadratic formula based on a graded mesh" (§9) is not derived here (deferred to [42], [48]).
8. **p = 3 is used without justification.** Nasser 2015 and Shen both permit any integer p ≥ 2; no source in the reading set explains what p trades off or how to choose it.
