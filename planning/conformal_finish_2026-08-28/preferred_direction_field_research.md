# The preferred feed direction field — what Zou's refs actually say

> **Why this file exists.** `paper_2009.02660_extraction.md` gap #1 of 14: Zou,
> Wang & Feng's method needs a preferred feed direction field `D` and §3.2
> explicitly refuses to compute one ("It is supposed in this work that preferred
> feed directions have been pre-assigned"), delegating to its refs. Our F1
> implementation substituted `D = t1` and measured 2.339×–9.001× the
> cutting-distance floor (`finishing_synthesis_2026-08-30.md` §8). This file
> retrieves the delegated refs and asks whether a computable `D` exists for a
> **3-axis ball-end** tool at all.

**Research date**: 2026-08-31.
**Rule observed**: only what a fetched source states. Everything not retrieved is
in "Underspecified / missing / not retrievable" with the exact URLs tried. One
paraphrase returned by a search engine (about Lo 1999) was **discarded** rather
than recorded, because it could not be verified against the paper and appeared to
conflate Lo's work with Chiou & Lee's — see §7.

---

## 0. What was fetched, and from where

Zou's reference list was re-read from the arXiv PDF itself (`pdftotext -layout`,
page 11–12) so the targets are the paper's own strings, not ours:

| Zou ref | Full citation as printed in arXiv:2009.02660v1 |
|---|---|
| [4] | G. H. Kumazawa, H.-Y. Feng, M. J. B. Fard, *Preferred feed direction …* (CAD 2015; truncated in the two-column PDF, completed from the UBC lab publication list: **Computer-Aided Design, Vol. 67-68, pp. 1–12**) |
| [21] | G. H. Kumazawa, *Generating efficient milling tool paths according to a preferred feed direction field*, **Master's thesis, University of British Columbia (2012)** |
| [24] | C.-C. Lo, *Efficient cutter-path planning for five-axis surface machining with a **flat-end cutter***, Computer-Aided Design 31 (9) (1999) 557–566 |
| [25] | M. J. Barakchi Fard, H.-Y. Feng, *Effective determination of feed direction and tool orientation in **five-axis flat-end milling***, ASME J. Manufacturing Science and Engineering 132 (6) (2010) |
| [28] | T. Kim, S. E. Sarma, *Toolpath generation along directions of maximum kinematic performance: a first cut at machine-optimal path*, Computer-Aided Design 34 (6) (2002) 453–468 |

**First finding, from the titles alone**: the two refs the task named as the
max-strip-width front-end — [24] and [25] — are **both five-axis flat-end**
papers. Neither is about a 3-axis ball-end tool. The 3-axis ball-end
instantiation is Zou's **[4]/[21]**, the Kumazawa line, which the task did not
name and which turned out to be the most on-point retrievable source in the
citation graph.

### Retrieved in full

| # | Source | Route | Notes |
|---|---|---|---|
| A | **Zou, Wang & Feng 2020**, arXiv:2009.02660v1 | `https://arxiv.org/pdf/2009.02660v1` | 12 pp. Used here only for the reference list; the method is already extracted in `paper_2009.02660_extraction.md`. |
| B | **Kim, Taejung (2001)**, *Time-optimal CNC tool paths — a mathematical model of machining*, PhD thesis, MIT Dept. of Mechanical Engineering, advisor Sanjay E. Sarma, submitted 12 Jan 2001 | MIT DSpace REST API: handle `1721.1/8861` → item `7248174c-…` → bitstream `https://dspace.mit.edu/server/api/core/bitstreams/b5a4d4b1-7359-47c1-9aaa-1ecdb41c94bc/content` | 188 pp, **scanned + OCR**. All load-bearing equations below were re-read as **rendered page images** (pp. 64, 80) rather than trusted to OCR. This is the thesis behind [28]. |
| C | **Kumazawa, Guillermo Hiroki (2012)**, *Generating efficient milling tool paths according to a preferred feed direction field*, MASc thesis, UBC Mechanical Engineering, Nov 2012 | **Wayback Machine**, capture `20200217025735` of `https://open.library.ubc.ca/media/download/pdf/24/1.0073405/1`, fetched as `https://web.archive.org/web/2020/https://open.library.ubc.ca/media/download/pdf/24/1.0073405/1` | 102 pp, born-digital text. This is Zou's **[21]**, and the thesis version of **[4]**. **Three-axis ball-end.** |
| D | Zou & Zhao, *Iso-level tool path planning for free-form surfaces*, arXiv:1811.07580 | `https://arxiv.org/pdf/1811.07580` | Fetched; contains exactly one relevant sentence (quoted in §6). |

### Retrieved as abstract only

| # | Source | Route |
|---|---|---|
| E | **Barakchi Fard & Feng 2010** ([25]) | OpenAlex `abstract_inverted_index` for DOI `10.1115/1.4002766`, reconstructed to running text. Publisher full text is paywalled. |
| F | Kumazawa 2012 thesis abstract | OpenAlex for DOI `10.14288/1.0073405` — used only to cross-check that capture C is the right document. It matches. |

### Not retrievable — see §7 for the URL log

[24] Lo 1999 (no abstract obtained either), the CAD-2002 journal version of [28],
the ASME full text of [25], and the CAD-2015 journal version of [4].

---

## 1. The quantity being maximised

Both retrieved sources maximise the same thing, under two different names.

**Kim 2001 (source B), §2.5.1, printed p. 63–64.** The *side-step-limit* `w₀` is
"the length of the interval [along the normal section perpendicular to a
streamline] … when the tool positions are arranged at a particular interval …
[such that] the cusp height reaches the specified cusp-height-limit h₀". Its
footnote states the synonymy explicitly: "Our notion of side-step-limit w₀ is
close to the meaning of **machined strip width** [53], allowable side-step [18]
and so on."

**Kumazawa 2012 (source C), §2.** "It is hypothesized that the tool path that
follows the direction that removes the most material will have the shortest
overall length. In this work, the **Machining Strip Width** is chosen as a metric
of material removal. Its maximum defines the preferred direction."

So `D` = argmax over feed direction of the cusp-height-limited stepover. Zou's
§3.2 description of `D` ("maximizes machining strip width … the strip's reach
perpendicular to the feed direction") is the same quantity.

---

## 2. The closed form — Kim 2001 Eq. (23), read from the page image

Printed p. 64, verified against the rendered page (not OCR):

```
                       ┌────────┐        ┌──────────────────────────────────────────────────────────────┐
                       │  8h₀   │        │                            8h₀                               │
  w₀(η,u,v)  ≈    sqrt │ ────── │  =sqrt │ ──────────────────────────────────────────────────────────── │   (23)
                       │ κ_b−κ_s│        │ (κ_b−κ₁)·sin²{η − η₀(u,v)} + (κ_b−κ₂)·cos²{η − η₀(u,v)}       │
                       └────────┘        └──────────────────────────────────────────────────────────────┘
```

Kim's own gloss, verbatim: "where `h₀` is the given cusp height limit, `κ_b (≡
1/R)` is the curvature of the cutting tool, `κ_s` is the *normal curvature* of
the surface **in the direction of `n × t`** and `κ₁/κ₂` is the
maximum/minimum principal normal curvature at a given point. Note that `κ_s ≡
κ_r(u)(n × t) = κ₁ sin²η̃ + κ₂ cos²η̃` according to *Euler's formula* [115], where
`η̃ (= η − η₀(u,v))` is the principal direction angle of the vector `t`". He
attributes the approximation to **Lin and Koren** [his ref 22] and notes: "both
the normal section and the ball are approximated by parabolas … the gap is
measured as a Euclidean distance (instead of being measured along the normal
section) … **The formula diverges when the curvature `κ_s` approaches the
curvature `κ_b` of the ball.**"

**Sign convention — read this before porting anything.** The divergence
condition fixes it: `κ_s → κ_b` is a *concave* normal section whose radius
approaches the ball radius (perfect conformity → unbounded strip). So in Kim's
convention **positive κ = concave**. His Appendix C says so directly: "`κ₁` and
`κ₂` are the **most concave and convex** principal curvature, respectively."
Zou's Eq. 2 uses the opposite convention (`k_s` positive if the design surface is
**convex**), with denominator `k_s + 1/r`. The two are the same formula with
`k_s = −κ_s`.

**Same result in Zou's convention** (the one our code should be in):

```
  s_max(direction) = sqrt( 8h / ( k_perp + 1/r ) ),   k_perp = normal curvature ⊥ to feed
  D = argmax s_max  ⇔  argmin k_perp  ⇔  feed along the MOST CONVEX principal direction
```

Kim states the conclusion twice, in his own words:

- §2.1, printed p. 29: "Note that the side-step-limit depends on in which
  direction the cutting tool proceeds — **the most convex direction is the most
  preferable one** from the viewpoint of an individual tool path in that it
  allows widest cut, in other words, the largest side-step-limit."
- Appendix C, Eq. (C-7), printed p. 165: any admissible replacement formula for
  `w₀` must satisfy `dw₀/dη ≥ 0` on `η ∈ [0, π/2]`, "**This is because the widest
  cut is made in the most convex direction.**"

Kim also gives a refined non-parabolic `w₀` in Appendix C (Eq. C-1 … C-2), a
piecewise expression requiring a quartic root-find `F(β_L) = (κ_bκ_s)²β⁴ −
2κ_bγ²β³ + … = 0` per direction, and closes it with: "**For most cases, `w₀` ≈
(23) is accurate enough.**"

**Kumazawa computes the same maximum numerically instead of in closed form**
(source C §2.1). For a 3-axis ball-end of radius `r`, cutter-location path
`CL(t)`, `P_CL = P_CC + n_CC · r` (his Eq. 1): the *swept profile* is the
half-sphere section by the plane spanned by `n_CL` and `t_CL` (Eqs. 5–6); it
meets the scallop surface `P_scall(u,v) = P_S(u,v) + n_S h` at two points `P_a`,
`P_b` obtained by solving `P_swept(θ) − P_scall(u,v) = 0` (his Eq. 7) — "not
solved analytically for most surfaces … the **Newton-Raphson** method was
applied" — and then

```
  W = | P_a P_b · t_CL |        (his Eq. 9)
```

`W` is swept over the feed angle `φ` (angle in the tangent plane from a reference
`f_ref = z_T × n_CC`); his Fig. 4 is a `W` vs `φ` plot; the argmax is the PFD.

**Both routes therefore answer the same question, one analytically and one by
sampling.** Kim's Eq. 23 is the closed form; Kumazawa's Eq. 9 is the sampled
ground truth that Eq. 23 approximates.

---

## 3. Does it degenerate on umbilic / flat regions? — yes, and both sources say so

This is the exact failure mode of our F1 run, and both retrieved sources name it.

**Kim 2001**, §2.2 (tangent-space bases), printed p. 34: "It is a routine in differential geometry to
find the principal directions, `ĩ_u` and `n × ĩ_u` **except, of course, at
umbilical points**." A footnote to the same paragraph derives principal-direction
orthogonality and opens "We only consider the case when the two eigenvalues are
different, i.e. `κ₁ ≠ κ₂`." **Kim states the exclusion and provides no handling.**

**Kumazawa 2012**, §4.1.3, printed p. 38, is the fuller treatment and is worth
quoting at length because it is the answer to our question:

> "In the case of 3-axis machining, a degenerate point on the design surface
> corresponds to a CC point where there is **not one single preferred direction,
> because all directions will be the preferred**. Ideally, the value of `W` will
> be the same for all directions at a degenerate point. This happens, for
> example, when the surface at that point is completely planar. A completely
> plane will not have any preferred direction, since `W` will be the same for any
> direction at any point. **In that case, it could be said that all points in a
> surface are degenerate points.**"

Formally he treats the PFD field as a **2-D symmetric tensor (line) field**, not a
vector field, "because [each point has] two vectors … a PFD field cannot be
analyzed as a vector field" (§2.2). Per sampled point, from the preferred
direction `(x, y)` projected to the XY plane:

```
  T(x,y) = [ x²   xy  ]        (his Eq. 14)
           [ xy   y²  ]
```

with bilinear interpolation of the tensor components between samples (Eq. 17). A
**degenerate point** is where the two eigenvalues coincide, i.e. (his Eqs. 20–22)

```
  T11(P) − T22(P) = 0     and     T12(P) = 0
```

and his practical detector is the physically meaningful one: "A simple method to
detect the vicinity of degenerate points is to find the regions of the design
surface where **the difference between the maximum value of `W`, `W_max`, and the
minimum one, `W_min`, is close to zero**." Grid cells that trip the test are
subdivided and re-tested.

**What he does about them** (§4.1.3 → §4.2, and the flowchart of his Fig. 7):
degenerate points are classified by the sign of a discriminant `δ` computed from
the partial derivatives of the tensor — `δ < 0` → **trisector** (three hyperbolic
sectors, three separatrices); `δ > 0` → **wedge** (one sector, one separatrix);
`δ = 0` → **merged wedge**, a detected coincidence of two nearby wedges. The
**separatrices are then traced and used as region boundaries**, segmenting the
surface into patches of coherent PFD flow. Each patch is filled by conventional
sequential **iso-scallop** paths, seeded from a "principal tool path" chosen to
minimise *iso-scallop drift* (§4.3.1: the progressive deviation of sequentially
offset paths away from the PFD).

Note the flowchart's other arm: **"Degenerate Points Detection → Detected? → No →
Principal Toolpath Generation"** — if no degenerate point is found, the whole
surface is one region. His Case Study 1 (a cone-like surface) took that arm.

**What neither source does**: give a rule for a region that is *entirely*
degenerate. Kumazawa states the condition ("all points in a surface are
degenerate points") and then does not return to it. A sphere and a plane are both
in that class. There is **no fallback direction** in either source.

---

## 4. [28] Kim & Sarma — the kinematic field, and what it optimises

The CAD 2002 paper is paywalled; source B is Kim's MIT thesis of the previous
year, whose Chapter 4 is titled "**GREEDY TOOL PATHS — A FIRST CUT FOR THE TIME
OPTIMAL TOOL PATH**" against the paper's "*a first cut at machine-optimal
paths*". Treat the identification as very strong but **not proven** (§7).

**The optimised quantity is the *sweep rate*, not the strip width** (§4.1,
printed p. 98, verbatim):

> "A local measure of material removal rate is what we define as the **sweep
> rate**: the **product of the speed and the side-step-limit**. The sweep rate is
> denoted by `F₀`, namely `F₀ = ϑ₀ · w₀`. The direction of maximum sweep rate is
> also the direction of greatest area coverage locally, and effectively, greatest
> material removal."

`ϑ₀(η,u,v)` is the maximum attainable speed in tangent direction `η`. It comes
from a **velocity polygon**: per-motor speed limits `|ω_i| ≤ ω̄_i` mapped back
into the surface tangent space through the machine Jacobian, giving "simple
half-planes of feasibility" whose intersection "forms a **symmetric polygon**"
(§2.5.2, printed p. 66–67, his Eq. 28). The greedy direction is the **binding
point**: "we need to find the point on the velocity polygon with the highest
sweep rate … the corresponding directions are called greedy directions". Because
the polygon is symmetric there is always a ± pair — again a line field, not a
vector field.

**Acceleration is deliberately excluded from the direction choice.** §2.5.3,
printed p. 68, verbatim: "**In the greedy approach, tool paths are generated in
two phases. We ignore acceleration limits in the first analysis, where we only
consider the speed limits — we account for acceleration limits in the second
phase by smoothing the tool path locally, removing small circular motions and
reducing the speed.**" Cutting-force, stiffness, bandwidth and tool-wear limits
are also dropped, on the stated grounds that finishing cuts are light.

**The result that matters most to us**, Appendix C, printed p. 165–166, verbatim:

> "Conventional tool path generation schemes do not take account of the kinematic
> aspect of machining, which is equivalent to the assumption that the velocity
> limit is not a polygon but a **circle** in `(u̇,v̇)`-space, namely `ϑ = ϑ₀ =
> const` represents the velocity polygon. **For such an isotropic case, the most
> convex direction (η-axis) is the direction of maximum sweep rate**, as shown in
> Figure 39-(c)."

That is the explicit bridge: **[28]'s field collapses onto [24]/[25]/[4]'s
max-strip-width field exactly when the machine's velocity envelope is isotropic.**
The kinematic field is the geometric field *warped* by the velocity polygon.

Two further pieces of Kim that bear directly on our programme:

- **Cutting-time functional**, Eq. (32), printed p. 80, read from the page image:
  `T_c = ∫∫_P ‖r_u × r_v‖ dudv / [ w(u,v)·ϑ(u,v) ] + ½{ ∫ (τ₀/w(u,v))·(1−γ)·|(n×t)ᵀ dr| + ∮ ‖dr‖/V₀ }` along all grain boundaries.
  The first term is **area ÷ (side-step × speed)**. With `ϑ` constant, this is
  our own `L_min = ∫∫ dA / s_max` (`finishing_synthesis_2026-08-30.md` §1) —
  the same floor, reached independently. The second term is the non-effective
  (link/retract) time, and it lives **on the region boundaries**, which is the
  same place our `surface_link` costs sit.
- **Region decomposition is part of the method, not an add-on.** §4.3, printed
  p. 105: streamlines need *continua*; Kim partitions the surface into **maximal
  basins**, each "a **simply connected** open set … [whose] INLET … is connected
  … [and] OUTLET … must be connected", found from the critical and turning
  points of the generating function. "**We point out that the minimal partition
  is not unique.**"

---

## 5. [25] Barakchi Fard & Feng 2010 — abstract only, and it is five-axis-only

Full abstract as published (reconstructed from OpenAlex's inverted index for DOI
`10.1115/1.4002766`; the ASME full text was not obtainable):

> "This paper addresses the challenging problem of determining feed direction and
> tool orientation at a given cutter contact (CC) point in five-axis free-form
> surface machining with flat-end mills. The objective is to efficiently
> determine a feed direction and tool orientation that will avoid both local and
> global tool gouging and yield a near maximum machining strip width at the CC
> point. Concurrent determination of the optimal feed direction and tool
> orientation is a very computationally intensive task and searching for the
> correct solution would involve exhaustive evaluations of the machining strip
> width at many feed directions and tool orientations. In this paper, the optimal
> feed directions and analytical solutions for the optimal tool orientations in
> five-axis flat-end milling of spherical, cylindrical, and toroidal surfaces are
> identified first. A **toroidal surface inscription method** is devised to
> approximate the local surface geometry at a CC point on a free-form surface by
> an inscribed toroidal surface. Analytical solutions for toroidal surface
> machining are then employed to position the flat-end mill at the CC point with
> the tool feeding in the best toroidal surface inscribing direction. Case
> studies have demonstrated that the proposed method can efficiently determine a
> feed direction and tool orientation, corresponding to a near maximum machining
> strip width."

**What this establishes.** [25] is a *joint* (feed direction, tool orientation)
optimiser for a **flat-end mill on five axes**. Its whole difficulty — and its
whole contribution, the toroidal inscription trick — exists because the effective
cutting shape of a tilted flat-end mill is an **ellipse whose size and orientation
depend on the tool's lead/tilt**. On a 3-axis machine the orientation is fixed;
with a ball-end tool the effective cutting shape is a **circle of radius r,
independent of feed direction**. **Both of the degrees of freedom [25] optimises
are absent from our configuration.** Nothing in [25] is portable to us, and
nothing in it is needed: the geometric part it shares with our case is exactly
Kim's Eq. 23 with `r₁ = r₂ = r`, which is Zou's classic Eq. 2 — and Zou himself
says so (`paper_2009.02660_extraction.md` §6: "Eq. 5 is identical to the classic
formula … if ball-end mills are used, which indicates the **best-case
scenario**").

*Not retrievable and therefore not asserted*: whether [25] discusses umbilic
degeneracy. The abstract does not mention it.

---

## 6. Direction fields on multiply-connected (holed) regions

Recorded per source, as absences rather than one blanket line:

- **Zou 2020**: never mentions holes, islands, or multiply-connected domains
  anywhere; all three test surfaces are simply-connected open sheets. (Already
  gap #12 in `paper_2009.02660_extraction.md`.)
- **Kim 2001**: the parameter domain is *defined* as simply connected — "a smooth
  one-to-one map … for a **simply-connected compact set** `T`" (§2.2, "Designed
  Surface"). Every decomposition unit inherits it: a **GRAIN** is "a
  **simply-connected** open set in the designed surface `S`", and a **BASIN** is
  "a **simply connected** open set on the parameter space `P`" (§4.3, "Maximal
  Basins", printed p. 105). **The topology is an assumption of the formulation,
  not an oversight in an example.** Kim offers nothing about holes.
- **Kumazawa 2012**: no occurrence of hole, island, multiply-connected, genus, or
  trimmed boundary. His six case studies are all single NURBS patches. The only
  "trimming" in the thesis is of *pseudo-separatrix segments* against other
  separatrices during segmentation (§4.2), which is a field-topology operation,
  not a domain-topology one.
- **Zou & Zhao, arXiv:1811.07580** (source D): the only PFD-relevant sentence is
  "Theoretically, tool paths following the direction of maximum machining strip
  width are the shortest in total length, since they maximize material removal.
  **But such strategy often leads to irregular tool paths which are neither
  direction/contour parallel nor spiral.**" Nothing on holes.

**Nothing retrieved says how to build a preferred-direction field over a
multiply-connected region.** The nearest thing to a mechanism in any source is
Kim's basin decomposition, which *produces* simply-connected pieces from a field
that is already defined — it does not tell you how to define one across a hole.

---

## 7. Underspecified / missing / not retrievable

**7.1 Not retrieved at all.**

- **[24] Lo 1999**, *Efficient cutter-path planning for five-axis surface
  machining with a flat-end cutter*, CAD 31(9) 557–566, DOI
  `10.1016/S0010-4485(99)00052-4`. Tried: Semantic Scholar Graph API by DOI
  (`openAccessPdf.status = "CLOSED"`); OpenAlex (`is_oa: false`, no OA location,
  **no abstract in the index**); `https://www.sciencedirect.com/science/article/abs/pii/S0010448599000524`
  (HTTP 403 via WebFetch). **Not even the abstract was obtained.** A web-search
  engine returned a plausible-sounding paraphrase of it; that paraphrase's second
  half described a "machining potential field", which is the title concept of
  Zou's ref [3] (Chiou & Lee, CAD 2002), so it is **discarded as unverifiable and
  probably conflated**. Everything this file says about [24] is limited to its
  title, venue, and Zou's own one-line characterisation of it (osculating-circle
  approximation of the effective cutting shape).
- **The CAD-2002 journal version of [28]** (DOI `10.1016/S0010-4485(01)00116-6`)
  — S2 `CLOSED`, OpenAlex no abstract. **Substituted** by Kim's MIT PhD thesis of
  Feb 2001 (source B), same author, same advisor, same subject, and a chapter
  title that mirrors the paper's subtitle. That substitution is an **inference**;
  no line-by-line correspondence between thesis and paper was verified, and the
  paper may contain material the thesis does not.
- **Full text of [25]** — `https://asmedigitalcollection.asme.org/manufacturingscience/article/132/6/061011/433510/…`
  returned HTTP 403 via WebFetch. Abstract only (§5).
- **The CAD-2015 journal version of [4]** (Kumazawa, Feng & Barakchi Fard, CAD
  67-68, 1–12, DOI `10.1016/j.cad.2015.04.011`) — S2 `CLOSED`; OpenAlex has
  **no abstract**; ScienceDirect 403. Substituted by the 2012 MASc thesis
  (source C) by the same first author on the same method. The journal paper is
  three years later and may differ.

**7.2 Access routes that failed (recorded so nobody repeats them).**

- UBC Open Collections is behind a Cloudflare "Browser Verification | UBC
  Cybersecurity" interstitial that defeats both `curl` (any UA) and WebFetch.
  Blocked: `https://open.library.ubc.ca/media/stream/pdf/24/1.0073405/1`,
  `…/media/download/pdf/24/1.0073405/1`,
  `https://open.library.ubc.ca/soa/cIRcle/collections/ubctheses/24/items/1.0073405`,
  and `http://hdl.handle.net/2429/43653` (which redirects there). Also dead:
  `https://circle.ubc.ca/bitstream/handle/2429/43598/…`.
  **The working route is the Wayback Machine** — the CDX API
  (`http://web.archive.org/cdx/search/cdx?url=open.library.ubc.ca/media/download/pdf/24/1.0073405/1&output=json`)
  lists two `application/pdf` 200 captures (2020-02-17 and 2024-07-24, identical
  digest `C7HWG62K…`). Note the naive `archive.org/wayback/available` probe on
  the `/media/stream/` spelling returned **no snapshots**; the `/media/download/`
  spelling is the one that is archived.
- `core.ac.uk` — HTTP 403 via WebFetch, CDN challenge via curl.
- `scholar.archive.org` — usable via WebFetch, CDN challenge via curl. Its search
  is what revealed that an archived copy of the Kumazawa thesis existed.
- Semantic Scholar Graph API rate-limits hard (HTTP 429) on a tight loop; ~4 s
  between calls worked.
- MIT DSpace `/handle/…` returns HTTP 405 to WebFetch; the REST API
  (`/server/api/pid/find?id=hdl:…` → `/core/items/{uuid}/bundles` → `ORIGINAL`
  bundle → bitstream `content`) works with plain curl.

**7.3 Things an implementer would still have to invent.**

1. **A rule for a wholly degenerate region.** Both sources name the condition and
   neither resolves it (§3). Zou's own §4.2 extrapolation-from-neighbours
   (unfold–translate–fold transport from well-defined regions, then Laplacian
   smoothing) is the only stated remedy anywhere in the three papers, and it
   presupposes that a well-defined neighbouring region **exists**. On a sphere or
   a plane, it does not.
2. **A degeneracy *threshold*.** Kumazawa's detector is "`W_max − W_min` close to
   zero" and the tensor residuals "below a threshold (very close to zero)". **No
   numeric value is given anywhere in the thesis**, and he lists the sampling
   resolution as his first stated limitation: "degenerate points may not be
   detected if the resolution is too low. **An optimal resolution that is suitable
   for a general case has not been determined.**"
3. **Discrete curvature on a mesh.** Both sources work on parametric NURBS
   (`P_S(u,v)` with analytic `P_Su`, `P_Sv`). Neither says how to estimate `κ₁`,
   `κ₂`, `η₀` on a triangle mesh. Same as Zou gap #6.
4. **Kim's `δ` discriminant and the separatrix tracer** are cited to Delmarcelle &
   Hesselink (Kumazawa refs [26–27]) and not reproduced in usable form here; only
   the sign convention (`δ<0` trisector / `δ>0` wedge / `δ=0` merged) was read.
5. **The velocity polygon for a 3-axis Cartesian router.** Kim's construction is
   general (Jacobian × per-axis speed limits), but he instantiates it only for a
   parallel hexapod. What the polygon looks like for an XYZ gantry cutting a
   sloped surface — and therefore whether the kinematic field differs from the
   geometric one on our machine — **is not in any retrieved source.** Deriving it
   is repo work, not citation.
6. **Sign convention when porting `t1`.** The rule is "feed along the **most
   convex** principal direction". Whether our `t1` is that direction depends on
   our normal orientation and on whether `t1` is max *signed* curvature or max
   *magnitude*. On a saddle these differ. Not a source gap — a repo check.

---

## 8. Closing assessment

### 8.1 Is there a computable preferred-direction field for a 3-axis ball-end tool, or is the concept 5-axis-only?

**It is computable, and it is not 5-axis-only — but the *interesting* part of it
is.** Only one retrieved source is literally our configuration: Kumazawa's entire
MASc thesis is **3-axis ball-end** ("A new method … to generate **ball-end**
milling tool paths for the efficient **three-axis** machining of sculptured
surfaces"). Kim's is a 5-axis formulation that *restricts to ball-end mills* ("In
the current formulation, **only ball-end mills are considered** for simplicity"),
so his Eq. 23 transfers to 3-axis rather than being stated for it — legitimately,
because a ball's effective cutting circle is orientation-invariant, so at fixed
orientation `w₀` is unchanged; but that transfer is our step, not his sentence.

The reason the concept survives the loss of tool orientation is that the ball's
rotational symmetry is not what makes strip width anisotropic — **the
surface's** curvature is. The effective cutting circle has radius `r` in every
direction; what varies with feed direction is `κ_perp`, the normal curvature the
scallop is measured across. Hence Kim Eq. 23, and hence:

> **`D` = the most convex principal direction.** In Zou's convex-positive
> convention, the principal direction of **maximum** normal curvature.

**This means our F1 substitution `D = t1` was not a wrong guess — it is the
literature's answer**, in closed form, for exactly our tool and axis count.
(Subject to the sign-convention check in §7.3 item 6.) The 2.339×–9.001× result
therefore does **not** indict the direction rule.

What *is* five-axis-only is [24] and [25]: the effective-cutting-shape /
osculating-circle machinery and the toroidal-inscription orientation solver exist
to handle a **tilted flat-end** tool whose cutting ellipse changes with lead and
tilt. For a ball-end on three axes that machinery collapses to `r₁ = r₂ = r` and
disappears. **Zou's delegation to [25] is, for our configuration, a delegation to
a paper we do not need.** The task's hypothesis that "Zou's method may have no
meaningful `D` for our tool" is **not** what the sources say — but the two refs
the task named are indeed inapplicable, and the applicable one ([4]/[21]) is a
different citation.

### 8.2 Does it degenerate on umbilic/flat regions, and does the source say what to do?

**Yes, catastrophically, and no.**

Kumazawa's statement is the diagnosis of our sphere and band fixtures word for
word: at a degenerate point "there is not one single preferred direction, because
all directions will be the preferred … This happens, for example, when the
surface at that point is completely planar … **In that case, it could be said
that all points in a surface are degenerate points.**" Kim excludes umbilics by
assumption (`κ₁ ≠ κ₂`).

The sources' response to degeneracy is **structural, not substitutive**: they do
not supply a replacement direction, they **cut the surface at the separatrices of
the degenerate points** and run a conventional sequential iso-scallop fill inside
each patch. Isolated degeneracies are handled by segmentation; a *region* that is
uniformly degenerate has **no treatment in any retrieved source**.

The practical consequence for us is sharp. Our F1 pipeline fed a raw argmax field
straight into the Poisson solve. The literature pipeline is
**field → detect degeneracies → classify → trace separatrices → segment →
per-patch sequential iso-scallop with a drift-minimising seed path**. Zou adds a
third layer on top (orientation propagation by BFS, unfold–translate–fold
transport into singular regions, Laplacian smoothing, then Eigenmaps + K-Means
segmentation at abrupt changes). **We implemented the direction rule and none of
the four stages that make it usable.** That is the actual gap #1, restated
precisely.

### 8.3 Would a proper `D` have produced the paper's clean figures on our fixtures?

**No — and Kumazawa's own numbers say why, more usefully than his figures do.**

His Table 1 (r = 10 mm, h = 0.2 mm, six NURBS surfaces) reports the proposed
PFD-segmented method against four baselines. Against a **conventional iso-scallop
started from the better of the two patch borders**, the improvements are:

| case | surface | vs iso-scallop, better border |
|---|---|---|
| 1 | cone-like, **no degenerate points** | −2.6 % |
| 2 | one trisector | −5.1 % |
| 3 | two wedges | −1.9 % |
| 4 | one merged wedge | −5.4 % |
| 5 | three trisectors + two wedges + one merged wedge | −7.2 % |
| 6 | bicycle-seat mould (practical case) | −3.3 % |

The large numbers he quotes (−39.6 %, −61.1 %, −48.9 %) are all against
**iso-parametric** and **iso-planar** paths. **Against an honest iso-scallop the
PFD field buys 1.9 %–7.2 %.** And his own conclusion states the precondition:
"this method benefits from surfaces that have **a large number of features such
as mounts and valleys**, where each region may have a preferred flow … also
useful for cases where **the difference between the maximum and minimum `W` are
notable**."

That is the same conditional our §8 arrived at by measurement — a locally-varying
stepover can only pay where `s_max` actually varies — reached here from the
method's originating author. It also explains the paper figures directly:
**Zou's turbine blade and bike seat are exactly the "many mounts and valleys,
large `W_max − W_min`" class.** Our SPHERE is `W_max − W_min = 0` by
construction (umbilic, `s_max` = 0.47431 everywhere, min = median = max), and
RIBBON/BAND are gentle. On those fixtures a *correct* `D` field would still have
had a near-zero anisotropy to exploit; the smoothness of the published figures is
a property of **their surfaces**, and the segmentation-and-smoothing stack around
the field, far more than of the argmax rule itself.

**A fair prediction, stated so it can be falsified.** With the full stack —
degeneracy detection, separatrix segmentation, per-patch iso-scallop with a
drift-minimised seed — the field arm should stop fragmenting (that is what the
segmentation is *for*: ribbon 674 → 98 fragments already moved that way on the
`D=sweep` change) and should land in the neighbourhood of a good iso-scallop,
i.e. **within a few percent of the raster's 1.10–1.31×**, not at 2.34–9.00×. It
should **not** be expected to beat the raster by a wide margin on any of the four
current fixtures, because the source's own measured margin over iso-scallop on
its best fixture is 7.2 %.

### 8.4 The one genuinely new lever: [28] is a *machine* field, and our machine is anisotropic

Kim's field maximises `F₀ = ϑ₀ · w₀` — speed × strip width — where `ϑ₀` comes
from the machine's velocity polygon. He proves the collapse case himself: with an
**isotropic** velocity envelope, max sweep rate = most convex direction, i.e.
`[28]`'s field degenerates into `[4]`'s. So on a machine whose feasible speed is
direction-independent, there is nothing extra to gain.

Two cautions before anyone reaches for this on the Shapeoko:

- **Kim's greedy field uses speed limits only and explicitly discards
  acceleration**, deferring it to a second smoothing/slow-down phase. Our
  finding that long straight passes win is an **acceleration** result. Kim's
  field is *not* the field that encodes it; his own general formulation (which
  does carry acceleration) is the one he calls "challenging to solve analytically
  or numerically" and does not solve.
- Whether an XYZ gantry's velocity polygon on a sloped surface is anisotropic
  enough to move the argmax is **not established by any retrieved source** (§7.3
  item 5).

There is, however, a clean structural payoff that *is* in the source. Kim's
cutting-time functional Eq. (32) is `∫∫ dA / (w·ϑ) + boundary link terms` —
literally our `L_min` floor with a per-direction speed in the denominator, plus a
link cost that lives on region boundaries. **Our floor and our
`surface_link` accounting are the two terms of a functional that was written in
2001.** If the programme wants a single scalar to optimise that unifies distance,
feasible feedrate and link cost, Eq. (32) is it, and it is citable.

---

## 9. Files fetched (kept in scratchpad, not committed)

```
zou2020.pdf / .txt          arXiv:2009.02660v1                     1.6 MB, 12 pp
zou1811.pdf / .txt          arXiv:1811.07580                       — , 998 lines text
kim2001_thesis.pdf / .txt   MIT DSpace 1721.1/8861                 28.5 MB, 188 pp (scanned)
p64-064.png, p80-080.png    rendered pages of the above (Eq. 23, Eq. 32)
kumazawa2012.pdf / .txt     Wayback capture 20200217025735          1.7 MB, 102 pp
```
