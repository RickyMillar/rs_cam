# Multi-tool finishing optimization — research log

**Question driving this:** Can a *reach-radius spectrum* of a freeform surface
(medial-axis / maximal-inscribed-disk distribution of local accessible radius)
drive selection of an optimal *tool set* + per-region assignment that removes all
material fastest, while preserving fine detail? Origin: tuning wanaka200's slow
single-1mm-ball finish; hypothesis is "big bit for open areas, small bit only for
the leftover detail, decided by analysing area-vs-tool-radius across the surface."

Running log of verified findings from deep-research sweeps. Confidence + vote are
from the adversarial-verification harness (need 2/3 refutes to kill a claim).

---

## Sweep 1 — 2026-06-24 (108 agents, 25 sources, 20/25 claims confirmed)

### The idea has a mathematical home: the medial axis
- **Veeramani & Gau (2000), "Voronoi mountain," *IIE Transactions*** (10.1023/A:1007615217634).
  Distance-to-boundary field over the Voronoi diagram (= medial axis); height =
  inscribed radius. Directly gives per-tool **removable volume + leftover volume**
  → the big/small rest handoff. **Verified high (3-0).** *Caveat: 2.5D pockets,
  planar medial axis — isomorphic to, not a solution for, 3D freeform finishing.*

### Tool-SET / sequence selection as time-minimization (2.5D)
- **D'Souza, Séquin & Wright (2004), *Computer-Aided Design* 36(7)** — graph-based
  lowest-cost tool **sequence** for free-form pockets (via 2.5D varying-thickness
  layers). Verified high. *(D'Souza = the half-remembered "D'Souza/Sarma".)*
- **Ahmad, Rahmani & D'Souza (2008/2010), *J. Intelligent Manufacturing***
  (10.1007/s10845-008-0201-6) — GA tool-sequence; "larger tools clear open spaces,
  smaller tools clean up… minimize total cost." Verified high.
- **Yingjie & Shangning (2008)** — optimal tool set for polygonal surfaces "in
  terms of least machining time." Verified high.
- **Zhang & Ge (2009), *IJAMT* 42:233-241** — cutter *sharing* across features in
  one setup. Paper real; one framing claim refuted.
- ⚠️ **Refuted specifics** (do NOT cite as fact): exact objective function;
  "Dijkstra-on-a-DAG"; "dynamic-programming not set-cover"; accessible-area handoff
  in the GA paper. Only the general "cost/time min via graph/GA" framing survived.

### Genuinely 3D sculptured surfaces — roughing
- **Balasubramaniam & Sarma (MIT MS thesis, 1999; Sanjay Sarma)** & **Lin & Gian
  (1999), *IJAMT* 15:387-398** (10.1007/s001700050081) — accessible-area-driven
  multi-tool selection for 3-axis sculptured **roughing**, and both **explicitly
  reject the naïve "largest tool that fits"** (reach vs swept-area diverge; weigh
  tool-change penalty vs MRR). Verified high (3-0). *Caveat: roughing, slab
  accessible-area — NOT a medial-axis disc spectrum.*

### Finishing efficiency: constant-scallop beats raster
- **Suresh & Yang (1994), *ASME J. Eng. for Industry* 116(2):253-259**
  (10.1115/1.2901938) — seminal iso-scallop.
- **Feng & Li / Kim & Choi (2002), *Computer-Aided Design* 34** — constant-cusp
  paths **~7-21% shorter** than iso-parametric/raster for the same tolerance.
  Verified high. *Caveat: path LENGTH, not accel-aware cycle time; iso-scallop is
  costlier to generate.*

### THE OPEN GAP (= the user's idea)
1. **No one unifies "area-vs-tool-radius / medial-axis reach spectrum → optimal
   *finishing* tool kit" for freeform 3D.** Voronoi-mountain = 2.5D pockets;
   sculptured multi-tool = roughing; finishing work = path density per *single*
   tool. The exact framing appears unclaimed.
2. **No cited time model is acceleration-aware.** All count path length / MRR; none
   model cornering/air-move overhead on a low-accel (~500 mm/s² Shapeoko) machine.

### rs_cam connection
rs_cam already ships the primitives: IPW ("use remaining stock"), steep/shallow,
scallop (constant-cusp), pencil, rest, AND an **acceleration-aware cycle-time
integrator** (strategy advisor). That integrator is precisely what the literature
lacks → rs_cam is well-positioned to close gap #2 as a small novel contribution.

### Not verified in sweep 1 (absence ≠ refuted) → SWEEP 2 targets
Lin & Koren 1996 iso-scallop · Lasemi/Xue/Gu 2010 CAD survey · pencil-milling /
valley-seam detection · Elber & Cohen accessibility/gouge · link/retract
minimization + disconnected-region ordering · accel-aware cycle-time estimation ·
steep/shallow hybrid quantified time/quality vs single-strategy.

---

## Sweep 2 — 2026-06-24 (109 agents; verification CUT OFF by session token limit)

⚠️ **Methodology note:** search+fetch succeeded, but the adversarial-verification
agents hit the account session limit mid-run and **abstained**. The harness logged
those as "killed," but abstention ≠ refutation. So below: a few items reached full
3-0 verification; the rest are **FOUND + quoted from the source, single-source,
NOT triple-checked → medium confidence**. None were actually refuted.

### VERIFIED (3-0) — closes item 1
- **Lin, R.S. & Koren, Y. (1996), "Efficient Tool-Path Planning for Machining
  Free-Form Surfaces," ASME J. Engineering for Industry 118(1):20-28.** Confirmed
  exists (U. Michigan MEAM). Plans each pass as a **non-constant offset of the
  previous path to hold scallop height constant → "no redundant motion"** (no
  double-machined overlap). Stepover = f(cutter radius, scallop height, local
  radius of curvature); builds on Vickers & Quan (1989); notes Suresh & Yang's
  interval expression is more complex/iterative. **The citation I gave was correct.**
- **Feng & Li (2002)** re-confirmed with pages: *Computer-Aided Design* 34(9):647-654,
  DOI 10.1016/S0010-4485(01)00136-1.

### FOUND but verification abstained (medium confidence — quoted, single-source)
- **Item 2 survey — Lasemi, Xue & Gu (2010), "Recent development in CNC machining
  of freeform surfaces: A state-of-the-art review," *Computer-Aided Design*
  42(7):641-654, DOI 10.1016/j.cad.2010.04.002.** Taxonomy = (1) tool-path
  generation, (2) tool-orientation identification, (3) tool-geometry selection.
  (dblp record corroborates existence.) **Citation looks correct.**
- **Item 2 recent (2015-2024):** Liu et al. (2015), *CAD* 66:1-13 — "machining
  strip-width tensor," region partition + iso-scallop within regions. Region-based
  strip-width (2018), *IJAMT*, DOI 10.1007/s00170-018-2427-6 — partitions the
  surface because **optimal feed direction varies, so whole-surface single-strategy
  finishing only reaches local optima** (directly supports region-based > single
  strategy). Zou, Wang & Feng (2020), arXiv 2009.02660 — **Poisson-energy GLOBAL
  optimization** of feed-direction field + constant scallop + minimum total path
  length (a strong modern formulation worth reading).
- **Item 5 pencil milling.** ⚠️ **Citation correction (2026-06-25, Crossref-checked
  during the pencil literature gap-review):** the previously-listed *"Park & Chung
  (2006), 'Pencil curve detection from visibility data,' CAD 38(3):223-231"* **could
  not be verified** — a Crossref title search for Park + Chung + pencil returns no
  CAM/toolpath paper (the real Park S.C. & Chung Y.C. CAD collaboration is on
  pocket/profile **offset** machining, not pencil). Treat that title as
  unverified/likely-misattributed; do NOT cite it as fact. The verified primary
  anchors for pencil/valley milling are:
  - **Park, J.W., Chung, Y.C., Kim, B.H. & Choi, B.K. (1999), "Pencil Curve Tracing
    via Virtual Digitizing,"** in *Machining Impossible Shapes* (IFIP/Kluwer),
    pp. 279-292, DOI 10.1007/978-0-387-35392-0_30 — the foundational ZMap method
    (the paper Wikipedia's "Pencil milling" cites). Pencil curve = ball-CENTER
    trajectory along concave edges; **detected via self-intersection of the offset
    (CL) mesh — valid pencil curves lie on the offset's "outer skin."**
  - **Choi, B.K. & Jerard, R.B. (1998), *Sculptured Surface Machining: Theory and
    Applications*, Kluwer, ch. 9 "clean-up machining"** — pencil-/fillet-curve
    detection, tracing, refinement; offset surface `r°=r+δ·n`.
  - **Lee, Y.-S. (2000), "Rolling-ball method and contour marching… identifying
    critical regions," *Computers in Industry* 41** (S0166361599000421) — the
    geometric reach/accessibility definition of the leftover (pencil/rest) region.
  - **Ren, Zhu & Lee (2004/05), "Material side tracing and curve refinement for
    pencil-cut machining of complex polyhedral models," *Computer-Aided Design*** —
    material-side-tracing for pencil-cut on meshes. (Authors/title/venue verified;
    exact volume/pages — previously listed 37(10):1015-1026 — NOT independently
    confirmed, treat as low-confidence.)
  - **Clean line extraction** (the "thick noisy band → one clean centerline" step we
    lack): **Ohtake, Belyaev & Seidel (2004), "Ridge-Valley Lines on Meshes via
    Implicit Surface Fitting," ACM TOG 23(3):609-612**; **Yoshizawa, Belyaev &
    Seidel (2005), "Fast and Robust Detection of Crest Lines on Meshes," Proc. ACM
    SPM '05:227-232.**
  - **Linking many short valley fragments** (our dead `hookup_distance`): **Castelino,
    D'Souza & Wright (2003), "Toolpath optimization for minimizing airtime during
    machining," *J. Manufacturing Systems* 22(3):173-180** — GTSP-with-precedence,
    Noon-Bean → LKH.
- **Item 4 Elber & Cohen — CONFIRMED (manual, 2026-06-24).** "Tool path generation
  for freeform surface models," *Computer-Aided Design* 1994 (PII 0010448594900701)
  / 2nd ACM Symp. Solid Modeling 1993 (10.1145/164360.164500); uses "Second-Order
  Surface Analysis Using Hybrid Symbolic and Numeric Operators." Produces
  **gouge-free iso-parametric toolpaths with a direct quantitative scallop-height
  bound, no auxiliary check/drive surfaces.** BONUS directly relevant to our idea:
  **Elber classifies the surface into convex / concave / saddle regions and assigns
  a flat-end tool to convex regions, ball-end elsewhere → better MRR + smaller
  scallops, gouge-free** — i.e. curvature-driven TOOL-TYPE selection per region,
  the closest published cousin to "reach-radius spectrum → tool choice."
- **Items 6 (accel-aware cycle-time) & 7 (steep/shallow):** sources fetched
  (PowerMill "Steep and Shallow" help; Mastercam "3D Hybrid"; a JMS&E paper) but
  all claims abstained — **still UNVERIFIED**, no quantified time/quality comparison
  confirmed yet.

### Item 6 refined (manual, 2026-06-24) — IMPORTANT correction
Accel/jerk-aware cycle-time **models are MATURE**, not missing: feedrate scheduling
+ corner smoothing under axis velocity/accel/jerk limits (Altintas/Erkorkmaz
lineage; e.g. "Accurate prediction of machining feedrate and cycle times
considering interpolator dynamics," *IJAMT* 2021 / arXiv 2102.02062, >90% accurate;
analytic minimum cornering-feedrate models). So rs_cam's accel-aware integrator is
**not novel as a model.** The actual gap is narrower and still real: **using an
accel-aware cycle-time model as the OBJECTIVE inside multi-tool SET / region
selection** — all the verified tool-selection work (sweep 1) optimizes path length
or MRR, not accel-aware time. That coupling is the open opportunity.

### Still genuinely open after both sweeps
- The headline gap stands: **"reach-radius / medial-axis spectrum → optimal
  FINISHING tool-kit for freeform 3D, scored by accel-aware cycle time"** — no
  single source unifies it. Veeramani Voronoi-mountain (2.5D) + Elber curvature-
  region tool-type selection + mature accel-aware time models are the three
  ingredients, unjoined.
- Quantified steep/shallow time vs single-strategy: commercial (PowerMill "Steep
  and Shallow", Mastercam "3D Hybrid") + threshold-angle study (ResearchGate
  365421528) exist; a clean academic time/quality number still soft.
