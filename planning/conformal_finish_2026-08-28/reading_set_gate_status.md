# F2 reading-set gate — status and verdict

**Date**: 2026-08-29. **Executed against**: `PROGRAMME.md` Phase F2 step 3 and `research/conformal_finish_2026-08-28.md` §B.4.

The gate line being answered, verbatim from PROGRAMME.md F2 step 3:

> **READING-SET GATE** before any hole/island work: source-read Shen et al. 2024 (IJRR 43, DOI 10.1177/02783649241251385 — boundary parameterization A-14, blend σ(t) A-11) and Nasser 2019 (J. Sci. Comput. 78:582–606 — the generalized Neumann kernel integral equation, discretisation, and interior evaluation). If neither is retrievable in full, F2 stops at the simply-connected phase and records that.

---

## VERDICT: **GATE OPEN**

Both required constructions were retrieved **in full text** and extracted from primary sources:

- **A-14 (boundary parameterization) and A-11 (σ(t))**: recovered **verbatim** from the Shen et al. accepted manuscript, arXiv:2309.10655v2 — see `paper_2309.10655_extraction.md` §2.4, §2.5.
- **The generalized Neumann kernel integral equation, its discretisation, and interior evaluation**: recovered **verbatim**, and from *two* independent primary sources that agree — the same Shen appendix (Eqs A-16…A-39) and Nasser's ETNA 2015 paper, arXiv:1308.5351v5 — see `paper_2309.10655_extraction.md` §3–§4 and `paper_nasser2015_etna_extraction.md`.

**A substitution was made and is declared here, not silently.** The gate names *Nasser 2019 (J. Sci. Comput. 78:582–606)* because that is reference **[11]** of the 2025 paper. The construction the pipeline actually consumes does **not** come from there. Shen 2024 — the paper that supplies the 2025 paper's slit map — **does not cite Nasser 2019 at all**; it defers its solver and FMM details to **Nasser 2015 (ETNA 44:189–229)**, which is openly available as arXiv:1308.5351 and was retrieved in full. Nasser 2019 is, by its own abstract, the *preimage* (inverse-direction) problem: given a slit domain, iteratively recover a conformally equivalent smooth-bounded domain. The task brief's clause "*OR whichever Nasser paper actually carries the generalized-Neumann-kernel slit-map construction*" licenses this; it is recorded rather than assumed.

**One named source was NOT retrievable in full, and it does not change the verdict.** Nasser 2019 (J. Sci. Comput. 78:582–606) is paywalled at €39.95 with **no open-access copy and no arXiv preprint anywhere** — verified against Springer, the author's own arXiv alias page, his Wichita State profile, his Qatar University CV, and the Semantic Scholar / OpenAlex / Unpaywall APIs (all three report `closed`, no repository copy). Only its abstract was retrievable. The gate's fallback ("*If neither is retrievable in full, F2 stops at the simply-connected phase*") does not trigger, because the fallback is conditioned on **neither** being retrievable, and the Shen paper — which alone carries both named constructions — was retrieved in full.

**What the verdict does and does not license.** It licenses proceeding past the simply-connected phase into hole/island work: the map is now specified end to end from retrieved text. It does **not** mean the pipeline is fully derivable from sources — five items remain unspecified in every retrieved paper, listed under "Residual gaps" below. One of them (the **inverse** Cauchy formula, Shen Eq A-39) is load-bearing and has **no primary-source corroboration**.

---

## Retrieval record — every URL tried

### Target 1 — Shen et al. 2024, IJRR 43 (2024) 2183–2203, DOI 10.1177/02783649241251385

| URL / action | Result |
|---|---|
| WebSearch `Shen Mao Xu "Spiral complete coverage path planning based on conformal slit mapping" arXiv preprint` | Located the preprint: **arXiv:2309.10655** |
| `https://arxiv.org/abs/2309.10655` | **Retrieved.** Metadata: v1 2023-09-19, **v2 2024-04-17**; comment *"47 pages, 17 figures, accepted by IJRR on April 10th"*; cs.RO, cs.CG; abstract |
| `https://arxiv.org/pdf/2309.10655` | **FULL TEXT RETRIEVED** — 47-page PDF, cached at scratchpad `2309.10655.pdf`. Appendix A-1/A-2/A-3 on pp. 39–47. Rendered pp. 40–47 read directly (layout text mangles fractions) |
| `https://journals.sagepub.com/doi/10.1177/02783649241251385` | Not fetched — unnecessary; the arXiv v2 is the accepted manuscript and carries the appendix in full |

**Verdict: FULL TEXT RETRIEVED.**

Caveat on citation: the arXiv v2 is the **accepted manuscript**, not the SAGE version of record. It carries only the arXiv DOI; journal pagination 2183–2203 is not in it. Section and equation numbering is the manuscript's own — but it is **the numbering the 2025 paper cites against**, independently confirmed: 2504.06310 cites "Equation A-14" for boundary parameterization and "Equation A-11" for σ(t), and in the fetched v2 those are exactly the boundary parameterization and σ(t) respectively.

### Target 2 — the Neumann-kernel construction

| Source | URL | Result |
|---|---|---|
| Reference [11] identity check | cached `2504.06310.pdf` pp. 32–36 → `pdftotext -f 32 -l 36` | **[11] confirmed verbatim**: *"M.M. Nasser, Numerical Computing of Preimage Domains for Bounded Multiply Connected Slit Domains. J. Sci. Comput. 78 (2019) 582–606. https://doi.org/10.1007/s10915-018-0784-9"*. Also confirmed [14] = Shen et al. IJRR 2024, [28] = Yunus/Murid/Nasser Proc. R. Soc. A, [38] = Kress Numer. Math. 58 (1990) |
| Shen 2024's own Nasser citations | `2309.10655.pdf` bibliography | **Nasser 2019 is absent.** Shen cites Nasser 2015 (ETNA), Nasser–Murid–Sangawi 2013 (TWMS), Nasser 2011 (JMAA), Nasser–Murid–Zamzamir 2008 (CVEE). Shen's appendix defers the fast solve and FMM to *"(Nasser, 2015)"* |
| WebSearch `Nasser "Numerical computing of preimage domains for bounded multiply connected slit domains" arXiv` | — | No arXiv preprint of the 2019 paper surfaced; hits were the Springer landing page, a *different* later paper (arXiv:2204.00726, infinite strip with rectilinear slits), and author pages |
| **Nasser 2015 (ETNA 44:189–229)** | `https://arxiv.org/pdf/1308.5351` | **FULL TEXT RETRIEVED** — arXiv:1308.5351**v5**, 14 Mar 2014, 33 pages, cached as `nasser1308.5351.pdf` |

**Verdict: FULL TEXT RETRIEVED (substituted source, declared above).**

Nasser 2015 is confirmed to be the right source by content, not just by citation: its **Eq (1)** `(I − N)μ = −Mγ` is Shen's **A-30**; its **Eq (2)** `h = [Mμ − (I−N)γ]/2` is Shen's **A-34**; its **Eqs (10)–(14)** are Shen's **A-17…A-21**; its **Eqs (87)–(89)** are character-for-character Shen's **A-11…A-13**; its **Eq (84a)** is Shen's **A-38**; its **Eq (5)** is Shen's **A-14**.

### Targets 3 and 4 — Nasser 2019 (J. Sci. Comput. 78:582–606) and Yunus et al. 2014 (Proc. R. Soc. A 470:20130514)

Retrieval of these two was run in parallel and is recorded under "Secondary retrieval" below: **Nasser 2019 = ABSTRACT ONLY** (paywalled everywhere, no OA copy, no preprint); **Yunus et al. 2014 = FULL TEXT RETRIEVED** (via Europe PMC, after the publisher and NCBI routes were bot-blocked). **Neither is required for the verdict** — the gate's two named constructions are already extracted from Targets 1 and 2 above.

---

## What was extracted, against the gate's own checklist

| Gate requirement (§B.4) | Status | Where |
|---|---|---|
| Boundary parameterization **A-14** | **Verbatim** — `η(t,j) = η_j(t)`, t ∈ J_j = [0,2π], j = 0…m; plus the substance behind it (cubic B-spline fit A-1…A-5, arc-length segments A-7, segment assembly A-8…A-10, corner grading A-11…A-13) | `paper_2309.10655_extraction.md` §2 |
| Blend **σ(t)**, **A-11** | **Verbatim**: `σ(t) = 2π[v(t)]^p / ([v(t)]^p + [v(2π−t)]^p)`, with `v(t) = (1/p − 1/2)((π−t)/π)^3 + (1/p)((t−π)/π) + 1/2`, `p ≥ 2`; properties σ: [0,2π]→[0,2π] bijective, strictly increasing, C^∞, **σ′(0) = σ′(2π) = 0**. Independently corroborated by Nasser 2015 Eqs (87)/(88) | `paper_2309.10655_extraction.md` §2.4; `paper_nasser2015_etna_extraction.md` §12 |
| The **integral equation** | **Verbatim**: `(I − N)μ = −Mγ` (Shen A-30 = Nasser Eq 1), from the Riemann–Hilbert problem `Re[Af] = γ + h` with the winding-number-1 correction h; γ given for **both** the disc map (A-25) and the annular map (A-26) | Shen §3.6–3.7; Nasser §1 |
| The **kernel** | **Verbatim**: `N(s,t) = (1/π)Im( (A(s)/A(t))·η′(t)/(η(t)−η(s)) )` with diagonal `N(t,t)`; companion singular `M(s,t)` with its cotangent split and `M₁`; `A(t) = e^{i(π/2−θ)}(η(t)−α)`, specialised to θ ≡ π/2, α = 0 so **A = η** | Shen A-16…A-21; Nasser Eqs 8–14 |
| **Discretisation** | **Verbatim**: Nyström, trapezoidal rule, n **even** equidistant nodes per component → **(m+1)n nodes**; Wittich's even/odd rule for the cotangent singularity, given as explicit matrix entries (Shen A-31…A-33). Nasser adds the **singularity-subtracted** form (Eqs 37/38) that Shen omits and that makes corners and near-touching boundaries work | Shen §3.8; Nasser §3.1–3.3, §6 |
| **Linear solve** — size, conditioning, FMM or dense | **Recovered from Nasser only.** Dense, nonsymmetric, (m+1)n × (m+1)n; matrix-free GMRES with FMM matrix-vector products; **O((m+1)n ln n)**; condition number, iteration count and CPU time all **grow as boundaries approach each other**; concrete parameters (iprec 4, restart 10/25, gmrestol 1e-12, maxit 10/40, n up to 4096). **Shen states none of this** | `paper_nasser2015_etna_extraction.md` §7, §9, §13 |
| **Interior evaluation** | **Verbatim, and the 2025 extraction's gap 2 is now CLOSED.** Forward: Cauchy integral formula as a **ratio of two trapezoidal sums** (Shen A-38 = Nasser Eq 84a), boundary data only, no interior mesh. Nasser §5 additionally supplies the *reason* for the ratio — it is the singularity subtraction that keeps the evaluation accurate for points near Γ | Shen §4; Nasser §11 |
| **Stated failure modes** | **Recovered from Nasser only.** For non-constant θ (radial slits): accurate at ε = 10⁻¹, 10⁻²; at ε = 10⁻³ *"the radial slits goes outside of the unit disc"*; at ε = 10⁻⁴ *"the obtained figure is incorrect."* Shen's maps use **constant θ = π/2**, the arm reported accurate — but conditioning still degrades as boundaries approach. Neither paper reports any accuracy figure for the map | `paper_nasser2015_etna_extraction.md` §13 |

---

## Residual gaps — what is still NOT derivable from any retrieved source

Ranked by how much they block the F2 hole phase.

1. **The inverse interior map ω⁻¹ (Shen Eq A-39) is corroborated in METHOD but not in FORM.** Nasser 2015 §5 gives only the **forward** Cauchy formula. Shen's A-39 contains an undefined `ω′_j(η_j)` and an integrand factor that does not match a standard change of variables into the w-plane. Yunus et al. 2014 §4–§5 supply the missing *device* — remove Φ⁻¹'s pole analytically (`G(w) = wΦ⁻¹(w)` when the map is normalised to a zero at the origin, or `(w − c)Φ⁻¹(w)`), apply the Cauchy integral formula to the analytic G, divide back — but in the **unbounded** setting with different normalisations, and their printed result carries the `1/w` factor that **Shen's A-39 lacks**. So the shape the correct bounded-case formula must have is now known; the formula itself is not derivable from any retrieved text. The 2025 pipeline needs ω⁻¹ to pull iso-circles back into the preimage domain, so this remains **load-bearing and unresolved — the largest residual gap.** (Mitigation available without new sources: the 2025 paper's own barycentric transfer between the two meshes gives an inverse without evaluating A-39 at all, provided ω has been evaluated at every mesh vertex via A-38.)
2. **The σ(t) "refinement" for bridging is in no retrieved paper.** A-11 is a *corner-grading* map; the 2025 paper reuses it as a bridge tangency blend (its Eq 9) and says "refining", without saying how. Its shape is right for the job (σ′ = 0 at both ends). The grading parameter is left as `p ≥ 2, integer` by both Shen and Nasser 2015 — but **Yunus et al. 2014 Eqs (8.1)/(8.2) fix it at p = 3**, which is the one concrete value in the retrieved set and a defensible starting default for the *boundary* grading. It is **not** a value for the bridge blend, where p controls the bridge's curvature profile and nothing constrains it. So the research doc's `[REPO]` row "*use any C¹ blend for σ until Shen 2024 is read*" is now: functional form sourced, boundary-grading p sourced at 3, bridge-blend p still `[REPO]` under test.
3. **Slit radii R_j — including the annulus inner radius R_A — are never recovered, in any of the three retrieved sources.** Shen A-35 calls them *"pending"*; Nasser 2015 leaves them to the conformal-mapping papers it cites; Yunus Eqs (5.1)/(5.3) call them *"undetermined real constants"* and likewise write no recovery. The 2025 spacing search needs `R_min = R_A` for annular mode. The obvious reading `R_j = |ω(η_j(t))|` on any node of Γ_j is an **inference**, not text. **This is the cleanest unclosed gap** — three primary sources, zero of them write it down.
4. **Wittich's quadrature weights are never defined** in either paper. Shen's A-33 gives enough to *implement* (explicit even/odd matrix entries); nobody gives the derivation or the order of accuracy.
5. **No accuracy requirement or error propagation.** Both papers validate the map against analytic test functions; neither states what map accuracy a path-planning consumer needs, nor how map error propagates into path spacing or scallop.

Two smaller items, both benign but must be resolved before coding: Shen's **Eq A-22** defines the operator N with the kernel M (a typo — Nasser Eq 19 is correct); Shen's **Eq A-9** is missing a factor `p_j/2π`, and A-8/A-10 disagree on 0- vs 1-based segment indexing.

---

## Consequences for the programme documents

These are recorded, not applied — the operator owns the edits.

- **`research/conformal_finish_2026-08-28.md` §B.2**, row *"Interior-point evaluation of the slit map never described | Cannot be invented — part of the reading-set gate (§B.4) | **gate***" — **this row closes.** Interior evaluation is Shen A-38 / Nasser Eq (84a).
- **§B.2**, row *"Bridging magic constants … and σ(t) deferred"* — partially closes: σ(t)'s form is now sourced; its parameter p is not, so the `[REPO]` "adopt as reported constants under test" treatment still stands for p and for the π/10, 2π, 8π/5, π/50 constants.
- **§B.4 and PROGRAMME.md F2 step 3** both name **Nasser 2019** as the integral-equation source. That attribution came from the 2025 paper's [11] and **is not supported by the chain of custody**: Shen 2024 does not cite Nasser 2019, and the construction descends from Nasser 2015/2013/2011/2008. Worth correcting in both documents.
- **`paper_2504.06310_extraction.md` gap 1** ("the slit map itself is external", "minimum reading set: this paper + [14] + [11]") — the minimum reading set is now **this paper + arXiv:2309.10655 + arXiv:1308.5351**, and **gap 2** ("interior-point evaluation of ω_S is never described") is closed by the newer extractions.
- **Both of the 2025 paper's mathematical citations point away from what it uses.** [11] Nasser 2019 is the *preimage* problem on *radial*-slit domains, and Shen 2024 does not cite it. [28] Yunus et al. 2014 — which the 2025 paper offers as the "disk/annulus-with-slits mapping formulation" — is for **unbounded** regions via the **adjoint** kernel with **Gaussian elimination**, i.e. a different domain class, a different integral equation, and a different solver from the one its own pipeline runs. Neither is wrong as a pointer into the literature; both are wrong as *the* source. Anyone reading the 2025 paper's references as an implementation reading list will be sent to the two least applicable papers in the family.
- **§C.1's deferred dependency question is now answerable at prototype scale.** The system is dense (m+1)n × (m+1)n. For a region with 2 holes at Shen's illustrative n = 128, that is **384 × 384** — comfortably inside existing dense `nalgebra` 0.33. **No new sparse dependency (`faer`) is needed for the F2 prototype**, and no FMM is needed; both become relevant only at the connectivity-10³ / n = 4096 scale Nasser targets. Neither paper states where the crossover lies.

---

## Secondary retrieval — Nasser 2019 [11] and Yunus et al. 2014 [28]

Run in parallel. **Neither changes the verdict** — the gate's two named constructions are already extracted from Targets 1 and 2. Recorded because PROGRAMME.md F2 step 3 names Nasser 2019 explicitly.

### Nasser 2019, J. Sci. Comput. 78 (2019) 582–606, DOI 10.1007/s10915-018-0784-9 — **ABSTRACT ONLY**

| URL / query | Result |
|---|---|
| `https://link.springer.com/article/10.1007/s10915-018-0784-9` | 303 → IdP → 302 back with `?error=cookies_not_supported`. **PAYWALLED**: "Buy article PDF 39,95 €". **No free PDF link.** Abstract fully readable |
| `https://arxiv.org/a/nasser_m_1` (author's own arXiv alias page) | HTTP 200, **31 titles listed — this paper is not among them**. The only `preimage` hit is the 2022 strip paper |
| arXiv API `all:"preimage domains"` | 2 hits: 1403.0423 (Schwarz–Christoffel), 2204.00726 (infinite strip). Not this paper |
| arXiv API `ti:"preimage domain"` | 1 hit: 2204.00726 |
| arXiv API `au:"Nasser, Mohamed M S"` | 25 entries; 2016–2019 window inspected item by item. **No preprint of the 2019 JSC paper** |
| arXiv API `all:"canonical slit domains" AND all:Nasser` | 0 entries |
| WebSearch: exact title + `pdf` | No free full text |
| `https://www.wichita.edu/…/Nasser-Mohamed.php` | HTTP 200. Publications list carries it as entry #29; **all citations unlinked plain text, no PDFs** |
| `https://www.qu.edu.qa/…/Nasser-CV.pdf` | HTTP 200, 6 pp. Lists it as journal item #8 **with no link or preprint**. Also records a 2016 ICMA-MU talk of the same name (conference precursor) |
| `https://qspace.qu.edu.qa/search?query=…` | HTTP 200 but 827 bytes of JS shell — **not queryable via curl; inconclusive, not a negative** |
| `https://www.researchgate.net/search/publication?q=…` | **HTTP 403** (Cloudflare). RG gates PDFs behind login regardless |
| Semantic Scholar `api.semanticscholar.org/graph/v1/paper/DOI:…` | HTTP 200. `openAccessPdf: {"url": "", "status": "CLOSED"}` |
| OpenAlex `api.openalex.org/works/doi:…` | HTTP 200. `is_oa: false`, `oa_status: "closed"`, `oa_url: null`, `any_repository_has_fulltext: false` |
| Unpaywall `api.unpaywall.org/v2/…` | HTTP 200. `is_oa: false`, `oa_status: "closed"`, `oa_locations: []`, `has_repository_copy: false` |
| CORE `api.core.ac.uk/v3/search/works/?q=…` | HTTP 500 / query-parse error — **API failure, inconclusive** |

**What was retrievable**: the abstract only, verbatim from the Springer page:
> *"In this paper, for a given bounded multiply connected slit domain Ω, we present an iterative numerical method for computing a conformally equivalent multiply connected domain G bounded by smooth Jordan curves and the conformal mapping w=Φ(z) from G onto Ω. Each iteration of the proposed iterative method requires solving the boundary integral equation with the generalized Neumann kernel. We consider two cases of bounded slit domains, namely the unit disk with radial slit domain and an annulus with radial slit domain. Numerical examples are presented to illustrate that the proposed iterative method converges even for highly connected slit domains."*

**Assessment.** Three independent OA aggregators agree CLOSED with no repository copy, and the author's own arXiv page and CV both list the paper with no preprint. It is not obtainable without payment (€39.95) or institutional access. **Its content is not needed**: the abstract confirms it solves the *inverse/preimage* problem (given a slit domain, iterate to find a conformally equivalent smooth-bounded domain) for **radial**-slit canonical domains — not the forward disc/annulus-with-**arc**-slits map the 2025 pipeline uses. Every element the gate asks for is in Shen 2024 and Nasser 2015. **Do not attribute the extracted construction to Nasser 2019.**

Near-variant, checked and deliberately not conflated: **arXiv:2204.00726**, Nasser 2022, *"Numerical computation of a preimage domain for an infinite strip with rectilinear slits"* — exists, fully retrievable, 24 pp, cached as `nasser2022_strip.pdf`. A different, later paper. Also cached: **AIMS MBE 20(1) 2023**, *"Numerical computation of preimage domains for spiral slit regions"* (gold OA, 17 pp, `nasser2023_mbe_spiral.pdf`) — the closest open-access methodological sibling to the 2019 paper, should the preimage direction ever be needed.

### Yunus, Murid & Nasser 2014, Proc. R. Soc. A 470:20130514 — **FULL TEXT RETRIEVED**

| URL | Result |
|---|---|
| Semantic Scholar / Unpaywall / OpenAlex by DOI 10.1098/rspa.2013.0514 | All three: **open access, `oa_status: "bronze"`**, publishedVersion, pdf_url → royalsocietypublishing |
| `https://royalsocietypublishing.org/doi/pdf/10.1098/rspa.2013.0514` | **HTTP 403** — Cloudflare "Just a moment…" challenge, 5,720 bytes of HTML. Blocked despite being bronze OA |
| Europe PMC REST search by DOI | `pmcid: PMC3896060`, `inPMC: Y`, `hasPDF: Y` |
| `https://pmc.ncbi.nlm.nih.gov/articles/PMC3896060/pdf/rspa20130514.pdf` | HTTP 200 but **1,817 bytes of HTML** — NCBI bot-block page, not a PDF |
| **`https://europepmc.org/articles/PMC3896060?pdf=render`** | **HTTP 200, `application/pdf`, 2,403,280 bytes — SUCCESS.** 24 pages, cached as `yunus2014_pmc.pdf` |

Extracted to `paper_yunus2014_rspa_extraction.md`. **It is not the source of the pipeline's slit map** and should not be cited as such: unbounded domains (not bounded), the **adjoint** kernel arm (not `(I − N)μ = −Mγ`), and **Gaussian elimination** (no GMRES, no FMM — FMM appears only in its introduction as prior work). Its two genuine contributions to the reading set:

- **The only concrete grading-parameter value anywhere in the set**: its Eqs (8.1)/(8.2) are Shen's A-11/A-12 with the exponent **fixed at p = 3**. Shen and Nasser 2015 both say only "p ≥ 2, integer".
- **A justified inverse-map construction** (§4, §5): remove Φ⁻¹'s pole analytically (`G(w) = wΦ⁻¹(w)`, or `(w − c)Φ⁻¹(w)`), apply the Cauchy integral formula to the analytic G, divide back. This is the nearest primary-source anchor for Shen's unexplained Eq A-39 — and it shows A-39 as printed is **missing the `1/w` factor** that device produces. It corroborates the *method*, not the *form*; residual gap 1 stands.

---

## Reading set as it now stands

| Role | Source | Retrievability | Extraction |
|---|---|---|---|
| The 2025 pipeline | arXiv:2504.06310v2 (53 pp.) | full | `paper_2504.06310_extraction.md` |
| The slit map: parameterization, kernel, equation, discretisation, recovery, interior evaluation | **arXiv:2309.10655v2** (47 pp., = IJRR 2024 accepted MS) | **full** | `paper_2309.10655_extraction.md` |
| The solver: system size, GMRES + FMM, conditioning, failure modes, singularity subtraction | **arXiv:1308.5351v5** (33 pp., = ETNA 44:189–229) | **full** | `paper_nasser2015_etna_extraction.md` |
| Grading parameter value; inverse-map pole removal | Proc. R. Soc. A 470:20130514 (24 pp.) | **full** (via Europe PMC) | `paper_yunus2014_rspa_extraction.md` |
| Named by the gate; not the actual source | Nasser 2019, J. Sci. Comput. 78:582–606 | **abstract only** (€39.95 paywall, no OA copy anywhere) | — (not needed) |
| Arm A comparator | arXiv:2009.02660 | full | `paper_2009.02660_extraction.md` |
