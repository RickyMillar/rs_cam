Here's a structured reading list, building from foundations up to the good stuff:

## Foundation Layer — Computational Geometry

You need these concepts before anything else:

**Polygon offsetting** — this is the core of contour-parallel toolpaths. The math is: offset each edge by tool radius, handle self-intersections at concave vertices. Held et al. (1994) covered pocket machining using contour-parallel paths generated via proximity maps, and Voronoi diagrams — that paper is a goldmine for 2D pocketing.

**Key equations you'll need:**
- **Offset curves** — shifting a boundary inward by tool radius, detecting and trimming self-intersections
- **Drop-cutter** — for 3D finishing: given (x,y), find the Z where a ball/flat/bull-nose cutter just touches the STL mesh
- **Scallop height** — `h = r - sqrt(r² - (stepover/2)²)` for ball endmill, determines your stepover spacing
- **Cutter-location (CL) point** — offsetting from the cutter-contact point along the surface normal by tool radius

## The Key Survey Papers

1. **Dragomatz & Mann (1997)** — "A Classified Bibliography of Literature on NC Milling Path Generation" in Computer-Aided Design. This is THE starting bibliography. Everything references it.

2. **Lasemi et al. (2010)** — Covers research developments in CNC machining of freeform surfaces from 1997 to 2008, picking up where Dragomatz left off.

## Specific Algorithm Papers by Strategy

**2D Pocketing (your hangboard bread and butter):**
- Held et al. — contour-parallel paths via proximity maps
- Jeong & Kim (1998/1999) — free-form pocket machining using Voronoi diagrams and z-buffer approaches
- Stori & Wright (2000) — "Constant Engagement Tool Path Generation for Convex Geometries" — this is the precursor thinking to adaptive clearing

**3D Finishing:**
- The cross-section/drop-cutter approach: generate a 2D pattern, then for each (x,y) point drop the cutter onto the surface model
- Constant cusp/scallop height strategies — iteratively producing offset curves in configuration space

**Adaptive/Stock-aware:**
- The Freesteel approach: model the 3D stock as a z-map, toolpath algorithm acts as an "agent" with constraints — don't collide with the part, don't remove too much material per step

## Open Source Code to Study

- **libactp** — GPL implementation of the Freesteel adaptive clearing algorithm as C library functions
- **OpenCAMLib** (Anders Wallin) — C++ library with Python bindings providing cutter-projection algorithms against triangulated surfaces
- **PyCAM** — 3-axis toolpath generator that loads STL/DXF/SVG, good reference implementation

## My Recommended Build Order

For your use case, I'd start with:

1. STL loading → triangle mesh in memory
2. 2D slicing at Z levels (layered roughing)
3. Polygon offsetting for contour-parallel pocketing (use the Clipper library — it's battle-tested)
4. Drop-cutter for 3D finishing passes
5. G-code output

That's a genuinely achievable scope with AI-assisted coding. The Clipper polygon library alone solves the hardest computational geometry problem you'll face for 2.5D work.

Want me to sketch out an architecture or start on any of these pieces?
