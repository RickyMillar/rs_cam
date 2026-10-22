# Synthesis of R1–R3 (lead, 2026-10-23)

Sources: `R1_freecad_adaptive.md` (FreeCAD `Adaptive.cpp`, commit be69b03e,
LGPL, Kresimir Tusek 2018), `R2_freesteel.md` (Freesteel blog, 19 posts
through the Wayback CDX index), `R3_papers.md` (Held–Spielberger 2014 full,
2009 secondary; Bieterman–Sandström by abstract and patent; fast marching
by Sethian 1999 and Zhang 2005; libactp headers). The lead checked two or
more cited lines of each note against its source. Ideas only; no code from
any source enters rs_cam.

## 1. What each candidate field needs (Phase 2)

| Candidate | Field | rs_cam has (R3 lookup) | Missing | Known risk |
|---|---|---|---|---|
| (a) geodesic wavefront from the entry | Eikonal distance, islands as obstacles | the fast-sweeping Eikonal solver, live in `finish/scallop_isofield.rs` (`build_field`, `fast_sweep`, `iso_contour`); the EDT; marching squares; pass-load rule; trochoid inserts | seeding from the entry disc or the cleared area instead of the region edge; island cells marked unreachable; exact seeding near the entry and island convex corners; a test of the 24-round cap on many-island slices | each meeting line behind an island is a shock: every wrap has a kink there; level sets cross the walls, so a wall pass is needed (V1) |
| (b) Bieterman–Sandström PDE morph | principal Dirichlet Laplace eigenfunction on a triangle mesh, laps blended by angle about one centre | a CG solver and a Laplacian assembler (3D, Neumann, research only); `geo` 0.32 has a constrained Delaunay triangulation (unused) | a 2D Dirichlet assembler and an eigen-solver (or the Poisson variant: one linear solve); interpolated iso-contours on a triangle mesh; a star-shape check with a typed refusal | one centre; contours must be star-shaped; level sets bunch at bottlenecks; two maxima on a non-convex pocket; no islands. **Fails V3 by construction** on branched terrain slices |
| (c) Held medial-axis spiral | time-driven wave on the medial-axis tree (2018 polygonal form) | a raster EDT and a raster thinning skeleton only | an exact Voronoi diagram or medial axis of segments and arcs (none in rs_cam, `cavalier_contours`, `geo` or `i_overlay`); the tree with arc-length weights and the balanced root; bridges and an MST for islands; spur pruning on raster-derived boundaries | the 2009 method is closed access and, by its author, "difficult ... to implement reliably"; build from Held–de Lorenzo 2018 if at all. Largest build gap |

Correction to PLAN.md: "a spiral that MORPHS between consecutive
wavefronts" is Bieterman–Sandström's radial interpolation, not Held's
mechanism. Held grows one continuous wave on the medial axis (R3, Source 1).

## 2. Which source addresses V1, V2, V3

| | V1 corner mop loops (one plunge per loop) | V2 corner-blend arcs in air | V3 does not grow into spaces |
|---|---|---|---|
| FreeCAD Adaptive (R1) | does not apply: no mop, no corner code. Corner residue is cleared by ordinary passes from the cleared-area frontier. Each pass is linked by a routed keep-down link (bent around stock, ≤ 3× direct); a retract costs +10000 in the link score | does not apply: G0/G1 only, no arcs. Lead-in and lead-out curves are fed moves through cleared area | unknown: greedy frontier growth; the next start is the frontier point with the cheapest link |
| Freesteel (R2) | opposite mechanism: the cycle skips near-empty sections and leaves the residue; the residue then blocks the stay-down link (the same mechanism that makes rs_cam reject the link) | not discussed; the default link is roll-off, retract, roll-on | by construction no: passes grow from engagement against the remaining stock, not from distance to the wall |
| (a) geodesic (R3) | does not address; needs a wall pass | partly: smooth wraps, but a kink at each meeting line | addresses: the front reaches each lobe from the cleared side |
| (b) PDE morph (R3) | partly: the last orbit is the offset boundary; outer orbits bunch | addresses: low curvature by construction | does not address |
| (c) Held (R3) | addresses: the spiral ends on the boundary plus one boundary pass | addresses: G1 path by construction | addresses: the wave reaches every leaf of the tree |

## 3. What the three reads agree on

1. **The two production adaptives grow from the cleared area, not from
   the boundary.** FreeCAD steps forward and measures exact new area per
   mm against a derived target (0.98948·s·D, R1 §3); Freesteel measures
   engagement at every node against the implicit remaining stock (R2 Q2).
   Neither offsets the wall. rs_cam's `ContourSpiral` offsets the wall
   (EDT iso-wraps), which is the mechanism behind V3.
2. **V1 is a LINK problem more than a FIELD problem.** FreeCAD clears
   corners with short ordinary passes and joins them by routed keep-down
   links; rs_cam's links are straight only (`adaptive/path.rs`
   `feed_link_checked`) with a 6R range, so residue between corner loops
   rejects them and each loop retracts. Freesteel built an A* stay-down
   linker for the same reason. Every Phase 2 candidate page must show its
   links (Phase 3), because no field fixes V1 alone.
3. **No source has a mop.** FreeCAD leaves a wall allowance (s·R/10) and
   profiles the walls once; residue flags are raised, not repaired.
   Freesteel skips near-empty sections.
4. **Neither production method has a time or acceleration model** (R1 §8,
   R2 Q3). The accel model that decides a wall-clock win in rs_cam stays
   the rs_cam instrument.

## 4. Ideas the lead carries into Phase 2 (ideas only)

- Candidate (a) is the cheapest to try: the solver exists; the change is
  the seed and the obstacles. Its page must show the wall pass and the
  meeting-line kinks.
- Candidate (c) is the strongest on paper for V1–V3 and the most costly
  to build (no medial axis in the repo). Rank it third for the build order
  and keep it on the table for G1.
- Candidate (b) goes to the 2D renders as planned, but R3 predicts it
  fails V3 on branched slices. The render decides.
- For every candidate: routed keep-down links with a retract penalty
  (FreeCAD) and a frontier candidate set scored by full link cost are the
  link-stage ideas. They apply to the baselines too.
- A climb-only rule (FreeCAD rejects a step ≥ 51 % conventional) is a
  planner rule rs_cam does not have.

## 5. Repo prose to correct (not done; for the operator)

- `research/02_algorithms.md` §5 states a Freesteel lineage for FreeCAD
  Adaptive; the source header and text do not support it (R1 §1).
- `research/04_open_source_reference.md` §2 and `CREDITS.md` describe
  another code under libactp's name; libactp is Freesteel's 2004 Z-level
  rough wrapped by Dan Heeks in 2009 (R3 Source 4).
- `finish/scallop_isofield.rs` module header says "nothing here is on a
  production path"; the `iso_field` dial reaches it (R3 L7).
