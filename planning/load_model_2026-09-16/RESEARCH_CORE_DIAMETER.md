# Research: end mill core diameter as a fraction of cutting diameter

Date: 2026-09-17. Scope: solid carbide (and HSS) square end mills.
Question: what bending diameter does a cantilever deflection model use, and
does that diameter change with the flute count?

## 0. The main result first

The research finds two different quantities. The sources mix them. A load
model must not.

| Quantity | What it is | Published range |
|---|---|---|
| **Core diameter** (web, root) | The geometric diameter of the inscribed circle at the flute bottoms. A caliper can measure it. | 0.47 D to 0.85 D |
| **Equivalent diameter** (effective diameter) | The diameter of a plain cylinder with the same bending compliance as the fluted section. | 0.75 D to 0.93 D |

A cantilever deflection model needs the **equivalent diameter**, not the core
diameter. The flutes do not remove material around the whole circumference.
The land material sits far from the neutral axis, and it carries most of the
second moment of area. The core diameter therefore under-states the bending
section.

The engine's constant 0.7 is not either quantity:

- As an equivalent diameter, 0.7 is too small for every flute count found.
  The engine over-reads deflection by 1.3x to 2.6x.
- As a core diameter, 0.7 is too large for most 2-, 3- and 4-flute tools.

**The direction of the flute-count effect reverses between the two
quantities.** More flutes give a **larger** core diameter. More flutes give a
**smaller** equivalent diameter, because each extra flute cuts away another
pocket. A model that mixes the two quantities gets the sign of the flute-count
correction wrong.

---

## 1. Every figure found

### 1a. Equivalent (bending) diameter — the quantity a deflection model needs

| Flutes | Diameter tested | d_eq / D | Source | Kind |
|---|---|---|---|---|
| 2 | 6, 10, 20 mm | **0.888 – 0.889** | Kivanc & Budak, derived (see 1e) | Peer-reviewed model, derived |
| 2 | 16 mm | **0.920 – 0.921** | Kivanc & Budak, derived (see 1e) | Peer-reviewed model, derived; possible table error |
| 3 | 6, 10, 16, 20 mm | **0.840 – 0.841** | Kivanc & Budak, derived (see 1e) | Peer-reviewed model, derived |
| 4 | 6, 10, 16, 20 mm | **0.748 – 0.749** | Kivanc & Budak, derived (see 1e) | Peer-reviewed model, derived |
| 2 and 4 | not stated in the abstract | **≈ 0.80** | Kops & Vo 1990, CIRP Annals 39(1):93–96, `https://www.sciencedirect.com/science/article/abs/pii/S0007850607610105` | Peer-reviewed, abstract only (full text paywalled) |
| 4 (implied) | any | **0.807** | Inscribed-square rule, derived in section 1f | Classical approximation, derived |
| 2 | 8 mm carbide | 0.936 (7.49 mm of 8 mm) | WOTEK, `https://www.endmills-wotek.com/en/blog/detail/75` | Vendor blog, single measurement, flute count not confirmed |
| not stated | 16 mm HSS | 0.973 (15.56 mm) | WOTEK, same URL | Vendor blog, single measurement |
| not stated | 20 mm | 0.975 (19.498 mm) | WOTEK, same URL | Vendor blog, single measurement |

The WOTEK figures are experimental fits of a whole tool assembly. They absorb
the shank and the holder, so they are not comparable with the fluted-section
figures above. Treat them as an upper bound only.

### 1b. Core diameter — manufacturer and patent statements

| Flutes | Diameter | Core / D | Source | Kind |
|---|---|---|---|---|
| 4 (stated as typical) | any | **0.60** | Mitsubishi Materials, "Web thickness", `https://www.mmc-carbide.com/permanent/courses/70/web-thickness.html` | Manufacturer technical reference |
| 3 | long-flute, aluminium | **0.50**, raised from **0.38** | OSG AE-TL-N, reported by MSC, `https://www.mscdirect.com/knowledge-center/articles/osg-aluminum-milling-tools-provide-competitive-edge` | Manufacturer statement, reported second-hand |
| 4 | 12 mm | **0.833** (10 mm core) | Machining Doctor, `https://www.machiningdoctor.com/expert-articles/endmiils-deflection/` | Engineering reference; page returns HTTP 403, read through a search summary only |
| 2 | any | **0.54** | UKO Carbide, `https://tungsten.blog/why-core-thickness-of-end-mills-count-matters/` | Vendor blog, **no citation given** |
| 3 | any | **0.56** | same | same |
| 4 | any | **0.60** | same | same |
| any | any | **0.60** ("generally") | same | same |

### 1c. Core diameter — patent claims (published geometry)

Read a patent claim as a **floor for that patent's design**, not as a census of
the market. A patent that claims "at least 60%" tells you that the prior art
sat below 60%.

| Flutes | Core / OD | Patent | Assignee | URL |
|---|---|---|---|---|
| 4 | 0.47 < CD < 0.60, "about **0.53**" | US9211593B2 | Iscar | `https://patents.google.com/patent/US9211593B2/en` |
| 4 | 0.47 < CD < 0.60, "about **0.53**" | US9211594B2 | Iscar | `https://patents.google.com/patent/US9211594B2/en` |
| 2 | at least **0.60** | US10335870B2, US9227253B1, US12275072B2 | New Tech Cutting Tools | `https://patents.google.com/patent/US10335870B2/en` |
| 3 | at least **0.60** | same | same | same |
| 4 | at least **0.58**, preferably **0.60** | same | same | same |
| 5 | at least **0.61**, preferably **0.64** | same | same | same |
| 6 | at least **0.63**, preferably **0.65** | same | same | same |
| 7 | at least **0.64**, preferably **0.68**; one example **0.6814** | same | same | same |
| 4 | **0.60 – 0.70**, example **0.65** at D = 6.35 mm, for D <= 10 mm | US6997651B2 | OSG | `https://patents.google.com/patent/US6997651B2/en` |
| 3 and 4 | **0.62 – 0.68**; examples **0.62** and **0.65** at D = 10 mm | US6719501B2 | Nachi-Fujikoshi / Sumitomo Electric | `https://patents.google.com/patent/US6719501B2/en` |
| 4 (of 3–8) | no more than **0.70**; examples **0.715** and **0.75** | US8647025B2 (ceramic) | Kennametal | `https://patents.google.com/patent/US8647025B2/en` |
| many (4 shown) | **0.50 – 0.80** | US10259054B2 | Kyocera | `https://patents.google.com/patent/US10259054B2/en` |
| 8 | **0.75 – 0.85** (web thickness) | US11471958B2 | Moldino Tool Engineering | `https://patents.google.com/patent/US11471958B2/en` |

### 1d. Definition of core diameter used by the patents

US9211593B2 defines it precisely, and the definition handles odd flute counts:

> the core diameter DC [is] twice a sum of distances from the center point to a
> closest point of each flute, divided by the number of flutes

Cross-check: 6G Tools and Harvey Performance both define it as "the diameter
measured tangent from the bottom of all flutes"
(`https://www.6gtools.com/technical-info/end-mills/dimensions-geometry.html`,
`https://www.harveyperformance.com/in-the-loupe/tool-deflection-remedies/`).
The two definitions agree for a symmetric tool.

### 1e. How the Kivanc & Budak equivalent diameters were obtained

Kivanc & Budak do not print an equivalent diameter. They print deflections.
This report derives the equivalent diameter from their published table.

Source: E. B. Kivanc, "Modeling Statics and Dynamics of Milling Machine
Components", MSc thesis, Sabanci University, 2004, Table 3.1 and Table 3.2.
URL: `https://research.sabanciuniv.edu/id/eprint/8159/1/kivancevrenburcu.pdf`
The same work is published as Kivanc & Budak, *Int. J. Machine Tools and
Manufacture* 44(11):1151–1161, 2004,
`https://www.sciencedirect.com/science/article/abs/pii/S0890695504000896`

Method:

1. Table 3.2 gives the analytic tip deflection `y` for 24 tools, at F = 50 N,
   for 2, 3 and 4 flutes.
2. Table 3.1 gives E = 200 GPa for HSS and E = 605 GPa for carbide.
3. Their equation 3.15 reduces to `y = F/(3E) * [L1^3/I1 + (L2^3 - L1^3)/I2]`,
   where `I1` is the fluted section and `I2 = pi*D2^4/64` is the plain shank.
4. Solve for `I1`. Then `d_eq / D1 = (64*I1 / (pi*D1^4))^(1/4)`.

The script is at
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/5894046c-4210-48eb-8e0b-5eb659f2ddbf/scratchpad/derive_deq_kivanc.py`.
It is 30 lines. It reproduces the table below. Copy it into the repository if
the engine adopts these constants, because the provenance rule needs it.

| Flutes | I_flute / I_solid | d_eq / D | Spread over 6, 10, 16, 20 mm |
|---|---|---|---|
| 2 | 0.624 | **0.889** | 0.888 to 0.921 |
| 3 | 0.500 | **0.841** | 0.8404 to 0.8409 |
| 4 | 0.314 | **0.748** | 0.7480 to 0.7492 |

Confidence in the derivation: **high, with two caveats.**

- The 4-flute and 3-flute results repeat to four significant figures across
  four diameters, two shank diameters, two flute lengths and two materials.
  A wrong E would not produce that consistency pattern by chance, and 200 GPa
  and 605 GPa are the values Table 3.1 states.
- **Caveat 1.** Two of the eight 2-flute rows (both 16 mm) give 0.920 instead
  of 0.889. The other six agree. This looks like an error in the published
  table, but the report cannot confirm that.
- **Caveat 2.** A 2-flute section is not axisymmetric. The thesis says so
  directly: "the total moment of inertia Ixx and Iyy are different" (equation
  3.12). The real 2-flute stiffness therefore changes as the tool rotates.
  The single figure 0.889 hides that swing. Section 4 returns to this.

### 1f. The inscribed-square rule, derived

Older practice replaces a 4-flute section with the square inscribed in the
cutting circle. That square is the worst case, because it assumes the flutes
cut to the centre on both axes.

- Square side `s = D / sqrt(2)`, so `I = s^4 / 12 = D^4 / 48`.
- Set `pi * d^4 / 64 = D^4 / 48`, so `d = D * (64 / (48*pi))^(1/4)` = **0.807 D**.

This is where the widely quoted "effective diameter is 80% of cutting diameter"
comes from. Kops & Vo's measured 0.80 agrees with it. The agreement is
probably not a coincidence.

---

## 2. What the spread says about flute-count dependence

### 2a. Core diameter rises with flute count, weakly

Pooling the patent claims and the manufacturer statements:

| Flutes | Core / D range found | Sources |
|---|---|---|
| 2 | 0.54 – 0.60 | 1 vendor blog, 1 patent family |
| 3 | 0.50 – 0.62 | 1 vendor blog, 1 manufacturer, 2 patent families |
| 4 | 0.47 – 0.833 | 6 independent sources |
| 5 | 0.61 – 0.64 | 1 patent family |
| 6 | 0.63 – 0.65 | 1 patent family |
| 7 | 0.64 – 0.68 | 1 patent family |
| 8 | 0.75 – 0.85 | 1 patent |

The trend is real and every source agrees on its direction. Its size is small
below 5 flutes. Within the one internally consistent family (New Tech Cutting
Tools) the claim floor moves only from 0.60 at 2 flutes to 0.64 at 7 flutes.

**The spread between makers at a single flute count is larger than the spread
across flute counts.** At 4 flutes the published values run 0.47 (Iscar) to
0.833 (Machining Doctor example). That is a factor of 1.77 in diameter, and
a factor of 9.8 in bending stiffness. No single core constant can cover it.

### 2b. Equivalent diameter falls with flute count, and the effect is large

| Flutes | d_eq / D | Deflection relative to 4-flute |
|---|---|---|
| 2 | 0.889 | 0.50x |
| 3 | 0.841 | 0.63x |
| 4 | 0.748 | 1.00x |

A 2-flute end mill in this model is **twice as stiff** as a 4-flute end mill of
the same cutting diameter. That is the opposite of the machining-forum
consensus, which says a 4-flute is stiffer because its core is bigger.

The two claims are reconcilable. Kivanc & Budak derive `I` from the flute
depth `fd` and appear to hold `fd` fixed across flute counts. At a fixed flute
depth, four pockets remove twice the area of two pockets, so the 4-flute is
less stiff. Real tool makers do not hold `fd` fixed. They grind the 4-flute
shallower, which is exactly what the rising core-diameter trend in section 2a
shows. The two effects oppose each other and partly cancel.

**This is the central unresolved conflict in the literature.** Section 5 says
what the engine should do about it.

### 2c. Contradictions between sources, and which to trust

| Conflict | Verdict |
|---|---|
| Kops & Vo: d_eq ≈ 0.80 for both 2 and 4 flutes. Kivanc & Budak: 0.889 and 0.748. | Both are peer-reviewed. Kops & Vo measured compliance; Kivanc & Budak used FEA and closed-form integration. Kops & Vo is only available as an abstract, so its conditions are unknown. **Trust Kivanc & Budak for the flute-count shape, because its full derivation is readable and its internal consistency is checkable. Trust Kops & Vo's 0.80 as the population average across flute counts.** The two agree that 0.80 is about right in the middle. |
| Iscar 4-flute core "about 0.53" against Nachi/Sumitomo 4-flute core "0.62 to 0.68". | Both are patents with readable text. Neither is wrong. **The makers genuinely differ.** Iscar's titanium tool trades core for chip room; the Nachi tool trades the other way. This is the strongest evidence that no universal core constant exists. |
| Mitsubishi "4-flute web is usually 60%" against Machining Doctor "12 mm 4-flute core may be 10 mm" (0.833). | **Trust Mitsubishi.** It is a tool maker describing its own product line. The Machining Doctor figure is an illustrative example, and its page could not be read directly. |
| UKO Carbide 2-flute 0.54 against New Tech patent 2-flute "at least 0.60". | **Trust neither alone.** The UKO blog gives no source. The patent states a claim floor, not a typical value. Their disagreement is consistent: a patent claiming 0.60 implies the prior art sat near 0.54. |

---

## 3. Does the diameter matter as well?

**For the equivalent diameter: no.** The derived ratios in section 1e are flat
from 6 mm to 20 mm, to four significant figures, for 3 and 4 flutes. The
geometry is self-similar, so the ratio is scale-free. Any diameter dependence
in a real tool comes from the maker choosing a different flute depth at a
different size, not from the mechanics.

**For the core diameter: weakly, and only through design intent.** Two pieces
of evidence:

- OSG's patent US6997651B2 restricts its 0.60–0.70 core range to
  `D <= 10 mm`. That implies OSG treats larger tools differently, but the
  patent does not say how.
- OSG's AE-TL-N shows the real driver is the **length-to-diameter ratio, not
  the diameter**. OSG raised that tool's core from 0.38 D to 0.50 D purely
  because it is a long-flute tool. MSC quotes OSG: the thicker core "is better
  able to counter vibrational forces that increase exponentially as length
  grows relative to diameter".

That 0.38 D figure is the lowest core fraction found anywhere, and it belongs
to a **long** 3-flute tool. A model that assumes 0.7 for such a tool
under-reads its deflection badly.

**Conclusion: stick-out ratio predicts the core fraction better than diameter
does.** No source publishes that relationship as a formula.

---

## 4. What could not be found

These are real gaps, not gaps in the searching.

1. **No manufacturer publishes core diameter in a catalogue dimension table.**
   This report checked Harvey Tool product detail pages, Guhring, Garr,
   Fullerton, M.A. Ford, Fastcut and Dormer catalogues. They all publish
   d1 (cutting), d2 (shank), l1 (overall), l2 (flute length) and corner
   radius. **None publishes core, web or root diameter.** Harvey Tool's own
   tool detail page for a 2-flute tool lists neck diameter but no core
   diameter. Core diameter is treated as proprietary grinding data.
   The only way to get it per tool is to measure the tool.

2. **No standard specifies it.** ASME B94.19 and DIN 6527 define end mill
   dimensions and tolerances. Neither appears to constrain web thickness.
   This report could not read either standard; both are paywalled. The
   negative finding is therefore indirect.

3. **No 2-flute figure of manufacturer quality.** The only 2-flute core
   numbers found are an uncited vendor blog (0.54) and a patent claim floor
   (0.60). **This is the worst gap, because the 2-flute case is where the
   engine's error is largest.**

4. **Machinery's Handbook could not be consulted.** No accessible edition,
   section or page could be located online for a stiffness-correction note on
   end mill core diameter. The report cannot confirm or deny that such a note
   exists.

5. **Kops & Vo 1990 full text is paywalled.** ScienceDirect returns HTTP 403.
   The 0.80 figure, and the claim that it holds for both 2 and 4 flutes, come
   from the abstract and from secondary citations. The tool diameters and the
   flute depths they tested are unknown. **Do not treat 0.80 as verified.**

6. **No source gives the flute-depth-to-flute-count design rule.** Every maker
   clearly has one. None publishes it. Without it, the conflict in section 2b
   cannot be closed from the literature.

7. **No source quantifies the 2-flute stiffness anisotropy.** The thesis states
   that `Ixx != Iyy` for a 2-flute section and gives the integrals, but it
   prints no ratio. A 2-flute tool is therefore stiffer in one direction than
   the other, by an amount this report cannot bound. For a rotating cutter
   that means the tip traces an ellipse, not a line.

---

## 5. What this means for the engine

The report does not average the sources into one number. It reports the
options.

1. **The 0.7 constant is not defensible as written, and its own comment is
   wrong.** No source found gives 0.7 as a core fraction for 2- to 3-flute end
   mills. The 2-flute core sources give 0.54 to 0.60. The 0.7 value is close
   to the OSG **4-flute** range (0.60–0.70) and to the Kennametal ceramic
   ceiling (0.70). The comment appears to attach a 4-flute-ish core value to a
   2-flute claim.

2. **Decide which quantity the model wants.** They are not interchangeable:

   | If the bending diameter is set to | 4-flute deflection against today's 0.7 |
   |---|---|
   | 0.748 (equivalent, Kivanc & Budak) | 0.76x — the engine currently over-reads |
   | 0.807 (inscribed square) | 0.59x |
   | 0.65 (OSG core) | 1.35x |
   | 0.53 (Iscar core) | 3.04x |

   The choice moves the answer by a factor of 4. It matters far more than the
   flute-count correction does.

3. **A defensible interim position:** use the equivalent diameter, make it
   depend on the flute count, and cite Kivanc & Budak.

   | Flutes | d_eq / D | Provenance |
   |---|---|---|
   | 2 | 0.889 | derived from Kivanc & Budak Table 3.2 |
   | 3 | 0.841 | derived from Kivanc & Budak Table 3.2 |
   | 4 | 0.748 | derived from Kivanc & Budak Table 3.2 |
   | 5+ | no source | do not extrapolate |

   State the constant-flute-depth assumption in the source comment. State that
   real makers grind deeper flutes on 2-flute tools, which pushes the 2-flute
   value down from 0.889 towards the 0.80 that Kops & Vo measured.

4. **Do not extrapolate above 4 flutes.** Kivanc & Budak model 2, 3 and 4
   flutes only. For 5+ flutes the only data is core diameter, and core
   diameter is the wrong quantity.

5. **Flag the 2-flute number as the weakest.** It rests on six self-consistent
   rows of one thesis table, two rows of that same table disagree, the section
   is anisotropic, and no manufacturer confirms it.

---

## Source list

Peer-reviewed:
- Kops, L. and Vo, D., 1990, "Determination of the Equivalent Diameter of an End Mill Based on Its Compliance", *Annals of the CIRP* 39(1):93–96. `https://www.sciencedirect.com/science/article/abs/pii/S0007850607610105` (abstract only; HTTP 403 on full text)
- Kivanc, E. B. and Budak, E., 2004, "Structural modeling of end mills for form error and stability analysis", *Int. J. Machine Tools and Manufacture* 44(11):1151–1161. `https://www.sciencedirect.com/science/article/abs/pii/S0890695504000896`
- Kivanc, E. B., 2004, "Modeling Statics and Dynamics of Milling Machine Components", MSc thesis, Sabanci University. `https://research.sabanciuniv.edu/id/eprint/8159/1/kivancevrenburcu.pdf` (full text read; Tables 3.1, 3.2, 3.3 and equations 3.12–3.18)

Manufacturer technical references:
- Mitsubishi Materials, "Web thickness". `https://www.mmc-carbide.com/permanent/courses/70/web-thickness.html`
- MSC Industrial Supply, "OSG Aluminum Milling Tools Provide Competitive Edge". `https://www.mscdirect.com/knowledge-center/articles/osg-aluminum-milling-tools-provide-competitive-edge`
- Harvey Performance, "Tool Deflection and Its Remedies". `https://www.harveyperformance.com/in-the-loupe/tool-deflection-remedies/`
- 6G Tools, "End Mill Dimensions and Geometry Guide". `https://www.6gtools.com/technical-info/end-mills/dimensions-geometry.html`

Patents (published geometry):
- US9211593B2, US9211594B2 — Iscar. `https://patents.google.com/patent/US9211593B2/en`
- US10335870B2, US9227253B1, US12275072B2 — New Tech Cutting Tools. `https://patents.google.com/patent/US10335870B2/en`
- US6997651B2 — OSG. `https://patents.google.com/patent/US6997651B2/en`
- US6719501B2 — Nachi-Fujikoshi / Sumitomo Electric. `https://patents.google.com/patent/US6719501B2/en`
- US8647025B2 — Kennametal. `https://patents.google.com/patent/US8647025B2/en`
- US10259054B2 — Kyocera. `https://patents.google.com/patent/US10259054B2/en`
- US11471958B2 — Moldino Tool Engineering. `https://patents.google.com/patent/US11471958B2/en`

Lower-grade, reported for the spread only:
- UKO Carbide, "Why Core Thickness of End Mills Count Matters". `https://tungsten.blog/why-core-thickness-of-end-mills-count-matters/` (no citations given)
- Machining Doctor, "Endmills Deflection". `https://www.machiningdoctor.com/expert-articles/endmiils-deflection/` (HTTP 403; read through a search summary)
- WOTEK Precision Tools, "Development of analytical solid carbide end mill deflection and dynamics models". `https://www.endmills-wotek.com/en/blog/detail/75`

Checked and found to contain no core diameter data:
- Harvey Tool product detail pages, e.g. `https://www.harveytool.com/products/tool-details-55630`
- Garr Tool end mill catalogue. `https://www.garrtool.com/wp-content/uploads/2018/11/END_MILLS.pdf`
- M.A. Ford end mill technical data. `https://www.maford.com/SiteContent/Documents/End%20Mill%20Technical%20Data.pdf`
- Fullerton metric end mill catalogue. `https://fullertontool.com/media/Fullerton-Metric-EndMill-Catalog.pdf`
- Harvey Performance, "The Anatomy of an End Mill". `https://www.harveyperformance.com/in-the-loupe/end-mill-anatomy/`
