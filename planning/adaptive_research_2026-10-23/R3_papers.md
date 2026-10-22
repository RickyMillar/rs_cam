# R3 — papers for the candidate fields (2026-10-23)

Reading agent R3. This note answers the R3 questions in `PLAN.md`. It
gives one section per source, then a V1–V3 table. Ideas only: no code was
copied. libactp and Freesteel are GPL; rs_cam is MIT OR Apache-2.0.

Downloads (data only, not executed):
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/a5fdffa7-9517-40c1-b382-2e96c28e2e27/scratchpad/r3_papers/`.

## The rs_cam lookup (HEAD, 2026-10-23, by `rg`)

Each "needs" line below refers to these rows. Paths are under
`crates/rs_cam_core/src/`.

| Id | What | Where |
|---|---|---|
| L1 | Raster EDT (Felzenszwalb–Huttenlocher), cell units | `geometry/grid_field.rs:85` `distance_transform_2d` |
| L2 | EDT level-set curvature, box blur | `geometry/grid_field.rs:137` `edt_curvature_field`, `:193` `smooth_grid` |
| L3 | Marching squares on a bool grid (threshold, no interpolation) | `geometry/contour_extract.rs:253` `marching_squares_bool_grid`; tables in `geometry/marching_squares.rs` |
| L4 | Contour spiral: EDT iso-wraps at stepover from the EDT max, radial seams, trochoid inserts | `adaptive/spiral.rs:67` `spiral_passes`, `:339` `extract_center_wrap` |
| L5 | Pass load: `PassLoad`, `measure_step`, `step_within_pass_load` | `adaptive/search.rs:737`, `:899`, `:955` |
| L6 | Entry at the EDT max; helical starter pocket | `adaptive/search.rs:1501` `find_entry_via_distance_transform`; `adaptive/path.rs:2497` `emit_helical_starter_pocket` |
| L7 | Eikonal solver, Godunov fast sweeping, signed field, interpolated iso-contours; seeded from the region boundary. **Live, not feature-gated** (`finish/mod.rs:31`): the scallop `iso_field` dial (`compute/operation_configs.rs:2141`, default off) reaches it through `compute/execute/finish_raster.rs:116`. The module header's "Nothing here is on a production path" is stale | `finish/scallop_isofield.rs:122` `build_field`, `:252` `seed_boundary`, `:317` `fast_sweep` (`MAX_ROUNDS = 24`), `:441` `iso_contour` |
| L8 | Sparse Laplacian, Jacobi-preconditioned CG, on a 3D triangle region mesh. Research only | `finish/direction_field.rs:506` `build_region_mesh`, `:1088` `assemble_poisson`, `:1227` `cg_solve` |
| L9 | Mean-value disk map by Gauss–Seidel; spiral bridge between rings. Research only | `finish/conformal_spiral.rs:1174` `plan_spiral`; `finish/spiral_finish_compact.rs:340` `bridge_nested_levels` (refuses branched level sets) |
| L10 | Raster skeleton (Zhang–Suen thinning) and skeleton tracing | `surface/rest_field.rs:1458` `zhang_suen_thin`, `:1889` `trace_skeleton` |
| L11 | Voronoi diagram or medial axis of segments and arcs | **None.** `rg -i "voronoi|medial|skeleton"` finds only comments, L2 and L10. The deps `cavalier_contours` 0.7.0, `geo` 0.32.0 and `i_overlay` 4.0.7 have none. |
| L12 | Constrained Delaunay triangulation of a polygon with holes | Not used by rs_cam. Available in `geo` 0.32 (`TriangulateSpade`, default feature `spade`, `algorithm/triangulate_spade.rs`) |

---

## Source 1 — Held and Spielberger, spiral HSM of pockets

- **Primary read (full PDF):** M. Held, C. Spielberger, "Improved Spiral
  High-Speed Machining of Multiply-Connected Pockets", *Computer-Aided
  Design & Applications* 11(3):346–357, 2014,
  doi:10.1080/16864360.2014.863508. The copy came from
  `http://www.cad-journal.net/files/vol_11/CAD_11(3)_2014_346-357.pdf`.
  Held's project page (`https://www.cosy.sbg.ac.at/~held/projects/hsm/hsm.html`)
  lists the papers but has no PDFs.
- **Not read (closed access):** M. Held, C. Spielberger, "A smooth spiral
  tool path for high speed machining of 2D pockets", *CAD* 41(7):539–550,
  2009, doi:10.1016/j.cad.2009.04.002. OpenAlex reports it closed, with
  no repository copy. **All statements on the 2009 mechanism are
  SECONDARY.**
- **Secondary sources (full text):**
  - M. Held, S. de Lorenzo, "On the generation of spiral-like paths within
    planar shapes", *J. Comput. Des. Eng.* 5(3):348–357, 2018 (gold open
    access, `https://academic.oup.com/jcde/article/5/3/348/5728966`). Read
    through WebFetch; the PDF download was blocked.
  - M. Abrahamsen, "Spiral Toolpaths for High-Speed Machining of 2D
    Pockets with or without Islands", arXiv:1412.5034v4, 2015 (full PDF).
    The author wrote the morphed spiral for Autodesk HSMWorks, Inventor
    HSM and Fusion 360 (§1, footnote 1).

### How is the spiral built? Is it a morph between consecutive wavefronts?

- The 2009 method does NOT interpolate between discrete offset contours.
  It defines one wave that depends on a time t. "They define a wave that
  starts at a point in the Voronoi diagram of the pocket at time 0 …
  at time 1, it reaches the boundary everywhere … the spiral is obtained
  by traveling around the wave while the time increases from 0 to 1"
  (Abrahamsen §1, p. 2). "The wave defined in [Held 2009] is the union of
  growing disks placed on the Voronoi diagram of the pocket" (same page).
- Held–de Lorenzo §1.2: "A series of circles are placed on the medial axis
  whose radii increase as time progresses. Portions of these circles are
  interpolated and connected by other circular arcs to form a
  G1-continuous path."
- The only primary hint in the 2014 paper: the ratio dmax/dmin "can be
  interpreted as the maximum impulse speed divided by the minimum M-disk
  growth rate" (2014 §3.2, p. 352). Thus the 2009 wave has an impulse that
  moves along the medial axis, and disks ("M-disks") that grow on it.
- This note does not reconstruct the 2009 arc interpolation. The source is
  not in hand.
- **Correction to PLAN.md, "The gap the June review left open":** the
  review attributes "morphs between consecutive wavefronts" to
  Held–Spielberger. That mechanism (interpolation between two contours at
  each angle) is Bieterman–Sandström's (Source 2). Held's mechanism is one
  continuous wave parameterised by time.
- Held calls the 2009 algorithm "difficult to analyze theoretically and
  even more difficult to implement reliably" (Held–de Lorenzo §1.3). The
  2018 paper replaces it with a simpler algorithm. Each medial-axis edge
  becomes a polygonal chain (§3). An impulse starts at the root at t = 0
  and reaches all leaves at t = 1. On a radial path its speed is the
  height of the root (§4). The wavefront w(t) is the closed polygon of
  the impulse positions at time t (§4). The spiral is a polygonal path
  (G0, Leroy 2024 Table 1); the abstract says the paper discusses smoothing. **For candidate (c), the 2018 paper is the
  implementation source, not the 2009 paper.**

### What structure drives it?

- The medial axis of the tool-centre boundary ∂S. The medial axis is the
  Voronoi diagram of ∂S minus the edges at reflex vertices (2014 §2.2,
  p. 348). It has O(n) edges and takes O(n log n) time (same page).
- The medial axis of a simply connected ∂S is a tree. The algorithm roots
  this tree at the spiral start r (2014 §2.2, p. 349).
- The start point is the "height-balanced root". The height of a node u is
  h(u) = max over descendant leaves v of d_T(u,v) + clr(v), where d_T is
  arc length along the tree and clr is the clearance (2014 §2.2–3.1,
  pp. 349–350). The balanced root is unique and takes O(n) time (§3.1,
  p. 350).
- A good root matters. On 290 pockets, every quality value got worse as r
  moved away from the balanced root: path length, step-over variation,
  engagement range and curvature (2014 §4.1, p. 354, Fig. 5).
- Lap count: during lap i the tool is at most s·i from r (triangle
  inequality). Thus a long arm of the tree sets the lap count. For an
  l × w rectangle the lap count is about ⌈max(x, l − x)/s⌉ (2014 §3.1,
  p. 349).
- Step-over bound: the maximum step-over is a hard user bound (2014 §2.1,
  p. 348; Abrahamsen §1 property vi). In 2018 the impulse moves at most Δ
  along the tree per lap time step. Thus consecutive laps are within
  Hausdorff distance Δ. Δ is an upper bound because the distance is
  measured along medial-axis edges (Held–de Lorenzo §4–5).

### Islands

- 2009 is for simply connected pockets only (Abrahamsen §1, p. 2: "It is
  only described by Held and Spielberger [10] how to handle
  simply-connected pockets").
- 2014 adds islands by **bridges** (§3.3, pp. 352–353):
  1. A bridge point is a Voronoi point with a local minimum of clearance
     whose two clearance lines end on different contours.
  2. The bridge is the two clearance lines. Its length is twice the
     clearance, which is the width of the bottleneck.
  3. A graph G has one node per contour. Each edge is the shortest bridge
     between two contours, weighted by its length.
  4. A minimum spanning tree of G selects k bridges for k islands.
  5. Each bridge is traversed twice. Thus ∂S becomes one curve, and the
     medial axis becomes a tree again.
- 2014 also splits complex pockets into sub-pockets along "split curves"
  (pairs of clearance lines at a medial-axis point). The objective
  penalises step-over variation, the number of sub-pockets (μ) and split
  angles away from π (ν). Suggested values: μ = 0.8, ν = 0.5 (§3.2,
  pp. 351–352; §4.2, p. 354). These are tuning constants with no
  derivation.
- Abrahamsen §3 gives an alternative for one island: the Voronoi diagram
  of a pocket with one island has exactly one cycle. The spiral morphs the
  island to the outer boundary. For many islands, Abrahamsen joins the
  islands into one island by Dijkstra-grown bridges (§5, p. 17).

### Input

- ∂S as straight-line segments and circular arcs, simply connected (or
  made simply connected by bridges). Normally ∂S is the pocket offset by
  the tool radius (2014 §2, p. 348).
- An exact Voronoi diagram of segments and arcs: VRONI / ArcVRONI (2014
  §4, p. 353). Freeform boundaries must first be approximated, because
  "no efficient algorithm to compute the medial axis of a NURBS curve … is
  known" (Held–de Lorenzo, via WebFetch).
- The 2014 design finishes by "machining along ∂S" after the spiral
  (§2, p. 348; Fig. 1 caption).

**What this method needs that rs_cam does not have:**

- An exact Voronoi diagram or medial axis of a segment-and-arc boundary
  (L11: none in rs_cam or its deps). rs_cam has only a raster EDT (L1)
  and a raster thinning skeleton (L10). Neither gives clearance lines,
  edge arc lengths or a tree.
- A medial-axis tree with arc-length edge weights, the height function
  and the balanced-root search (no equivalent; L6 picks the EDT max cell,
  which is a different point).
- The bridge finder and the MST for islands (none).
- Noise control on terrain slices: a raster region boundary gives a
  medial axis with many short spurs. Neither paper discusses pruning (the
  2018 paper has no such statement). This risk is the PLAN's expected
  risk for (c). The sources do not resolve it.

---

## Source 2 — Bieterman and Sandström, curvilinear PDE tool path

- **Journal paper (ABSTRACT ONLY):** M. B. Bieterman, D. R. Sandström,
  "A Curvilinear Tool-Path Method for Pocket Machining", *ASME J. Manuf.
  Sci. Eng.* 125(4):709–715, 2003, doi:10.1115/1.1596579. Closed access.
  Abstract (OpenAlex): "uses the solution of an elliptic partial
  differential equation boundary value problem defined on a pocket region
  … helps morph a smooth low-curvature spiral path in a pocket interior to
  one that conforms to the pocket boundary".
- **Primary full text used instead:** US patent 6,591,158 B1, "Methods
  and apparatus for defining a low-curvature tool path", inventors
  Michael Brady Bieterman and Donald R. Sandstrom, assignee Boeing, filed
  2000-06-09, granted 2003-07-08
  (`https://patents.google.com/patent/US6591158B1/en`). Citations below
  are to its steps and equations.
- **Secondary:** C. Leroy, S. Lavernhe, É. Rivière-Lorphèvre, "Hermite
  Quartic Splines for Smoothing and Sampling a Roughing Curvilinear Spiral
  Toolpath", *Appl. Sci.* 14:7492, 2024 (full PDF, open access);
  P. Zelinski, "Curvilinear Tool Paths For Pocket Machining", *Modern
  Machine Shop*, 2002-07-01 (trade article).

### The PDE, boundary conditions and mesh

- The field is the **principal eigenfunction of the Dirichlet Laplacian**
  on the offset pocket Ω (tool-centre region) (patent step 205, Eqs. 1–3):
  - −∇²φ = λφ in Ω;
  - φ = 0 on ∂Ω;
  - normalised so that max φ = 1. The smallest eigenvalue λ₁ is used.
- φ vanishes on the boundary and has a unique maximum of 1 at the pocket
  centre. The centre is not known in advance (step 205). For a rectangle,
  φ = cos(πx/L)·cos(πy/(BL)) (Eq. 5).
- Mesh: a triangle mesh, solved with piecewise-linear finite elements in
  the MATLAB PDE Toolbox (step 205).
- Leroy 2024 §2.2 uses a related field: the steady heat equation with a
  uniform source and T = 0 on the border, which is a Poisson problem. It
  is also solved by FEM (12 103 nodes, §3).

### How level sets become passes; how the spacing is set

1. Contours Γ = {φ = C} for C from 1 down to 0 (step 210). Near the
   centre the contours are round and smooth. Toward the boundary they
   take the boundary shape (Fig. 8, Fig. 9 text).
2. Each contour is parameterised by the angle of equally spaced rays from
   the centre (step 300). Each orbit has 180 or 360 points at equal angles
   and starts on a fixed ray (step 215).
3. Each orbit spirals from a Beginning Contour Γ₁ (φ = C₁) to an Ending
   Contour Γ₂ (φ = C₂). At angle θ the tool centre is
   (1 − θ/2π)·Γ₁(θ) + (θ/2π)·Γ₂(θ) (step 315). **This is the "morph
   between consecutive wavefronts" that PLAN.md attributes to Held.**
4. Spacing: C₂ is searched by iteration so that the maximum estimated
   radial depth of cut over the orbit is as close as possible to the user
   target (steps 310–325). The estimate uses a polyline of the uncut
   material boundary, its normals, and ray tracing between orbits
   (step 320 text). The search also checks an area-per-length bound for
   spindle power (Eqs. 6–7).
5. The last orbit equals the offset boundary (step 330 text, Fig. 12).
6. Known defect: the outer orbits bunch, with very small rdoc, "because
   once an orbit is created, it is never created again". The fix is
   3-point smoothing from orbit to orbit and from point to point (step
   220). This smoothing raises rdoc near the boundary and lowers it
   elsewhere, and it lets the boundary's high curvature into the inner
   orbits (same step).
7. The patent then optimises the feed along the path under axis velocity,
   axis acceleration and power limits by a two-pass sweep (Eqs. 17–28).
   This step is separate from the field.

### Islands and narrow necks

- The patent and the abstract do not mention islands. The mechanism
  needs one centre and one ray per angle. Thus every contour must be
  star-shaped about that centre.
- "the methods only work for star-shaped pockets" (Abrahamsen §1, p. 2,
  citing Held 2009). Leroy 2024 Table 1: "Suitable only for globally
  convex pocket … Not suitable for pocket with island".
- Narrow necks and lobes: Held 2014 §1.2, p. 347 says the PDE morph
  "works nicely for simple 'nearly convex' pockets" but "it is difficult
  to apply to more general pockets". It also says Laplace isoparametrics
  "tend to be unevenly distributed if the pocket has bottlenecks".
- On a non-convex pocket the field "has two global maxima": two start
  points are possible, and the polar (winding-angle) parameterisation
  "is not functional" (Leroy 2024 §5, p. 12). Thus the level sets above
  the saddle value split into two loops.
- Trade article: "there are restrictions that affect what pockets can be
  programmed this way". Aluminium time savings were 10–30 %. In titanium,
  tool life rose by nearly 100 % at a small cycle-time cost (Zelinski
  2002).

**What this method needs that rs_cam does not have:**

- A 2D triangle mesh of the slice region. Not used by rs_cam today. `geo`
  0.32 already gives a constrained Delaunay triangulation (L12).
- An eigenvalue solver (inverse iteration on the Dirichlet Laplacian).
  rs_cam has a CG solver and a Laplacian assembler (L8), but on a 3D
  surface mesh with Neumann conditions and one pinned vertex. It would
  need a 2D Dirichlet assembler. The Poisson variant (Leroy) needs only
  one linear solve with L8-class numerics.
- Interpolated iso-contours of a scalar field on the slice. L3 only
  thresholds a bool grid. L7 `iso_contour` interpolates, but on the
  isofield grid only.
- A star-shape check about the centre, with a typed refusal. L9
  `bridge_nested_levels` already refuses branched or non-star level sets
  (finishing only).
- The per-orbit rdoc search. rs_cam can replace this search with the
  planner stock and `step_within_pass_load` (L5), so this item is not a
  real gap.

---

## Source 3 — geodesic distance by fast marching (candidate (a))

- **Primary read (full text, author's own copy):** J. A. Sethian,
  "Advancing Interfaces: Level Set and Fast Marching Methods", ICIAM 1999
  proceedings review,
  `https://math.berkeley.edu/~sethian/2006/Papers/sethian.iciamproceedings.1999.pdf`.
- **ABSTRACT ONLY:** J. A. Sethian, "A fast marching level set method for
  monotonically advancing fronts", *PNAS* 93(4):1591–1595, 1996; R. Kimmel,
  J. A. Sethian, "Computing geodesic paths on manifolds", *PNAS*
  95(15):8431–8435, 1998. The PMC and Europe PMC PDFs were behind a
  download gate. The Berkeley copy of the 1996 paper has broken font
  encoding.
- **Primary read for the meeting-line question (full PDF):** Y.-T. Zhang,
  H.-K. Zhao, J. Qian, "High Order Fast Sweeping Methods for Static
  Hamilton–Jacobi Equations", *J. Sci. Comput.*, 2005,
  doi:10.1007/s10915-005-9014-3 (`https://www3.nd.edu/~yzhang10/ZZQ_JSC.pdf`).
  Fast sweeping is the solver family that rs_cam already uses (L7).

### What it solves; what grid or mesh it needs

- The arrival time T solves |∇T|·F = 1 with T = 0 on the start curve Γ.
  With F = 1, T is the distance (Sethian 1999 §3.1, Eq. 3.1).
- For the geodesic distance inside a region with islands, Γ is the source
  (the entry disk, or the cleared area). Each island is an obstacle with
  F = 0, that is, cells the front cannot enter. This use is standard in
  path planning ("robotic navigation and path planning", §2, §5.3).
  Sethian 1999 states no obstacle formula; this sentence is my reading.
- Grid: a rectangular orthogonal grid with first-order upwind differences
  (§4.1, Eq. 4.3). Kimmel–Sethian extend the method to triangulated
  domains "with the same computational complexity" (1998 abstract; 1999
  §6).

### Cost

- Fast marching: O(N log N) for N grid points, with a min-heap of the
  "Considered" band (Sethian 1999 §5.2).
- Fast sweeping (rs_cam L7): Gauss–Seidel with four alternating sweep
  orders, O(N) per sweep (Zhang–Zhao–Qian §1). The sweep count grows when
  the characteristics turn many times. L7's own header says it converges
  "in a fixed small number of sweeps on fields without deep spiral
  topology", and L7 caps the rounds at 24. A field that wraps around many
  islands is exactly the "deep" case. Thus the cap needs a test on the
  fixtures.

### Accuracy where the front meets itself (behind an island)

- The solution "need not be differentiable, even with arbitrarily smooth
  boundary data". The scheme is built to give the viscosity (entropy)
  solution (Sethian 1999 §3.4). T stays single-valued when the front
  merges or splits (§3.3).
- Where two fronts collide, the characteristics meet and form a **shock**.
  "The errors incurred at these singularities will not pollute solutions
  in other regions since no characteristics flow out of such
  singularities" (Zhang–Zhao–Qian Remark 4).
- At a shock the L∞ error is first order only. The L1 error over the
  domain stays second order "since the singular set has a measure of
  O(h)" (Zhang–Zhao–Qian §3, the example of the distance to two circles, Tables IV–V).
- A **rarefaction** is different. Characteristics fan out from a point
  source or from a convex corner of the start set. Errors made there
  "propagate out and pollute the solution in the whole computational
  domain". The fix is to assign exact values in a fixed small region
  around the source (Remark 4; Example 4, Tables VII–VIII).
- For candidate (a), the entry point is a point source, and each convex
  corner of an island that the front bends around is a rarefaction source.
  Thus a grid geodesic field needs exact seeding around the entry disk and
  a fine grid near island corners. (The mapping to islands is my
  inference from the cited remark.)

### What a level set looks like at the meeting line

- The viscosity solution follows Huygens' principle: the front is the
  envelope of circles from each point of the previous front (Sethian 1999
  §5.2). Where two fronts arrive from the two sides of an island,
  T = min(T₁, T₂) (my derivation from the envelope rule).
- Thus the level set {T = c} has a **corner** on the meeting line. The
  corner points back toward the source (a re-entrant kink), and ∇T jumps
  across the line. Zhang–Zhao–Qian Example 5 shows "the sharp shock
  transition" of such a field (Fig. 4).
- Consequence for a tool path: a wrap traced along {T = c} has a sharp
  turn at every meeting line. Also, after the meeting, the wraps close
  around the island one at a time, so the level-set topology changes (two
  arcs join into a loop). L7's header already records this: "A saddle in
  D merges or splits level sets … a hazard for anything downstream that
  assumes ring identity".
- Level sets of the distance from the entry are not parallel to the
  walls. They meet the outer wall at an angle. Thus they leave a residue
  band along every wall, and the method needs a closing pass along the
  wall by construction.

**What this method needs that rs_cam does not have:**

- A geodesic solver seeded from a source with the region boundary and
  islands as obstacles. The solver exists (L7, `fast_sweep`), but
  `seed_boundary` seeds the region edge as T = 0, which is the EDT case.
  The change is to seed the entry disk (or the cleared area) and to mark
  island cells as unreachable.
- Exact seeding in a fixed neighbourhood of the entry and of island
  convex corners (rarefactions), as above.
- A test of L7's 24-round cap on multi-island slices.
- A wall pass and a rule for the kinks at meeting lines. The existing
  trochoid insert in L4 and the pass-load rule L5 can bound the bite at a
  kink. No source in this read addresses the kink as a tool path.

---

## Source 4 — libactp (`Heeks/libactp-old`)

- **Read (full):** the repository metadata, `debian/control`,
  `debian/copyright`, `debian/changelog`, `freesteel/fslicense.txt`,
  `freesteel/INSTALL.txt`, the file tree, and the headers of
  `freesteel/src/pits/CoreRoughGeneration.{h,cpp}`,
  `freesteel/src/cages/S1stockcircle.cpp` and `PythonLib/PythonStuff.cpp`
  (`https://github.com/Heeks/libactp-old`). The repository has no README.
  No code was copied.

### Who wrote it

- GitHub description: "The libactp (Adaptive Clearing Tool Path Library)
  is an implementation of the GPL'ed algorithm demonstrated in FreeSteel
  as a set of C library functions".
- `debian/control`: "FreeSteel is a program originally written by Julian
  Todd and Martin Dunschen and released under the GPL. It generated an
  adaptive clearing strategy tool path for screen display."
- File headers: "FreeSteel -- Computer Aided Manufacture Algorithms,
  Copyright (C) 2004 Julian Todd and Martin Dunschen", GPL v2 or later.
  `CoreRoughGeneration.cpp` adds "with modifcations by Dan Heeks 2009".
  `PythonStuff.cpp`: "written by Dan Heeks, January 2009, license: GPL
  version 3".
- `debian/copyright`: upstream author Dan Heeks, 2009, from
  `code.google.com/p/libactp`. Packaging by Joachim Steiger (2009) and
  Sebastian Kuzminsky (2010). The last commit is 2010-03-27; the GitHub
  copy was pushed on 2011-07-07.
- `fslicense.txt` says that a private (non-GPL) licence is available from
  CIMCO Integration, which holds the Todd and Dunschen code "under
  separate terms".

### What the code is

- The engine is `CoreRoughGeneration` ("core rough"). It works against a
  triangle surface (`SurfX`, `SurfXboxed`, weave cells `S2weaveCell*`) at
  a step-down. It is a Z-level rough of a 3D model, not a 2D polygon
  library.
- The tool steers by the arc of the tool circle that touches the stock
  (`ChangeBearing`, `CircleIntersectNew`). Fixed turn constants decide
  the change of direction (`dchangright = 0.17`, `dchangleft = -0.41`,
  and others, labelled "fixed values controlling the step-forward of the
  tool and changes of direction"). Links are tested against stock and
  contour (`TrackLink`), with curl, link and retract builders.

### Relation to FreeCAD Adaptive.cpp and to Freesteel

- libactp says that it is Freesteel's algorithm wrapped as a library. It
  says nothing about FreeCAD; its last commit (2010) is older than
  FreeCAD's adaptive. R1 covers `Adaptive.cpp`.
- **Defect in `research/04_open_source_reference.md` §2:** that section
  names libactp as "Adaptive2d", lists the constants `AREA_ERROR_FACTOR`,
  `MAX_ITERATIONS`, `ANGLE_HISTORY_POINTS` and `MIN_STEP_CLIPPER`, and
  says the code "Entirely depends on Clipper". `rg` on
  `CoreRoughGeneration.cpp` finds none of these names and no Clipper. The
  file tree has no Adaptive2d file. Thus §2 describes a different code
  (probably FreeCAD's Adaptive2d; R1 can confirm) under libactp's name.
  `CREDITS.md` repeats the "libactp / Adaptive2d" label. This note does
  not edit either file.

**What this method needs that rs_cam does not have:**

- Nothing for Phase 2. libactp is an agent-style steering rough, like
  rs_cam's existing `adaptive/path.rs` walker. It gives no scalar field,
  so it is not a candidate (a), (b) or (c) source.
- If the operator wants Freesteel ideas, the source is the 2004 Todd and
  Dunschen code (GPL; read for ideas only). The constants in it have no
  derivation, so the operator ruling of 2026-10-23 forbids copying them
  as defaults.

---

## V1–V3 against the sources

V1 = corner mop loops; V2 = corner-blend arcs that cut air; V3 = wraps
do not grow into side lobes (`PLAN.md`, Q0).

| | 1 Held (medial-axis spiral) | 2 Bieterman (PDE morph) | 3 Geodesic / fast marching | 4 libactp |
|---|---|---|---|---|
| V1 | **addresses**: the spiral ends on ∂S, and the design adds a pass along ∂S (2014 §2, p. 348; Fig. 1) | **partly**: the last orbit equals the offset boundary (step 330), but the outer orbits bunch and need smoothing (step 220) | **does not address**: level sets from the entry cross the walls, so a residue band forms along every wall; a wall pass is needed by construction | does not address (steering rough; no field) |
| V2 | **addresses**: the path is G1 by construction; the arcs are the path, not inserted blends (Abrahamsen §1 property v) | **addresses**: low curvature by construction; smoothing, no inserted arcs (steps 215–220) | **partly**: smooth wraps away from meeting lines, but each meeting line gives a kink (shock) that needs a blend or a trochoid | does not address |
| V3 | **addresses**: the wave reaches every leaf of the medial axis at t = 1; the balanced root and the decomposition handle long arms (2014 §3.1–3.2) | **does not address**: one centre, star-shaped contours; bottlenecks bunch the level sets (Held 2014 §1.2); non-convex pockets give two maxima (Leroy §5) | **addresses**: the front reaches each lobe at its geodesic distance and grows into it from the cleared side | does not address (agent walk; lobes fill when the walker reaches them) |

## Three findings that matter most

1. **Candidate (c) has the largest build gap.** rs_cam and its deps have no
   Voronoi diagram or medial axis of segments (L11). The 2009 paper is
   closed and covers simply connected pockets only. Its own author calls
   it hard to implement reliably. Islands are 2014 (bridges plus MST).
   Build from Held–de Lorenzo 2018 (open, polygonal impulse), not from
   2009.
2. **Candidate (b) fails V3 by construction.** The Bieterman morph needs
   one centre and star-shaped contours. Bottlenecks bunch the level sets,
   and a non-convex pocket gives two maxima. It is weak for branched
   terrain slices. Also: the "morph between consecutive wavefronts" in
   PLAN.md is Bieterman's radial interpolation, not Held's mechanism.
3. **Candidate (a) has the smallest build gap but two known risks.** The
   fast-sweeping solver is already in rs_cam (L7); it needs point or
   cleared-area seeding with islands as obstacles. The entry and island
   corners are rarefactions, which need exact local seeding. Meeting lines
   are shocks: the error stays local, but every wrap has a kink there. The
   level sets ignore the walls, so V1 needs a wall pass.

Also: `research/04_open_source_reference.md` §2 describes another code
under libactp's name. libactp is Freesteel's 2004 Z-level rough (Todd and
Dunschen), wrapped by Dan Heeks in 2009.
