# Mesh Processing & G-code Generation

---

## Part 1: Mesh Processing

### STL File Format

**Binary STL** (dominant in practice):
- 80-byte header
- 4-byte uint32 triangle count (little-endian)
- Per triangle (50 bytes): normal (3xf32) + 3 vertices (3x3xf32) + 2-byte attribute
- Total: 84 + 50*N bytes

**ASCII STL**: Text-based, 10-15x larger. `solid name / facet normal / outer loop / vertex / endloop / endfacet / endsolid`.

**Rust**: `stl_io` (read/write both formats, 2.5M downloads, active).

### Building Efficient Mesh from STL

STL is "triangle soup" -- no shared vertices. To build an indexed mesh:

1. **Vertex welding**: Hash vertex positions (tolerance-based or exact) to find shared vertices
2. **Build indexed mesh**: `Vec<[f32; 3]>` vertices + `Vec<[u32; 3]>` triangle indices
3. **Recompute normals**: `normal = normalize((v1-v0) x (v2-v0))` -- STL normals are often unreliable

### Mesh Repair

| Defect | Detection | Fix |
|--------|-----------|-----|
| Degenerate triangles | Zero/near-zero area | Collapse shortest edge or remove |
| Inconsistent normals | Shared edge winding check | BFS propagation from seed triangle |
| Holes | Boundary edges (referenced by 1 triangle) | Ear-clipping triangulation of boundary |
| Non-manifold edges | Edges shared by >2 triangles | Duplicate vertices to split |

### Spatial Indexing

**KD-Tree** (proven for drop-cutter):
- Store triangles indexed by bounding box coordinates
- Query: find triangles overlapping cutter footprint at (x,y)
- OpenCAMLib uses 6 dimensions: [xmin, xmax, ymin, ymax, zmin, zmax]
- Different search dimensions for drop-cutter (XY) vs push-cutter (YZ or XZ)
- Rust: `kiddo` (SIMD-accelerated, ImmutableKdTree)

**BVH** (alternative):
- Binary tree with AABB at each node
- SAH (Surface Area Heuristic) for optimal splits
- Rust: `bvh` crate (SAH, rayon parallel, f32/f64)

**R-Tree** (for 2D queries):
- Best for 2D polygon/contour spatial queries
- Rust: `rstar` (R*-tree, integrates with geo)

### Mesh Slicing (Z-Level Cross Sections)

For plane at z_slice intersecting triangle (V0, V1, V2):

1. Classify vertices: sign(Vi.z - z_slice)
2. For crossing edges: `t = (z_slice - Va.z) / (Vb.z - Va.z)`, `p = Va + t*(Vb - Va)`
3. Each crossing triangle produces one line segment
4. Chain segments into closed contours via endpoint adjacency map

**Optimal complexity**: O(n log k + k + m) via sweep-plane algorithms.

### Heightmap / Z-Buffer

2D grid storing max Z at each (x,y). For a 100mm x 100mm part at 0.1mm resolution: 1M cells = 4MB (f32).

**Creation**: Software rasterization of each triangle onto the grid.

**Material removal simulation**: For each cutter position, compute tool Z at each overlapping grid cell:
```
new_z = cutter_z + cutter.height(xy_distance)
grid[cell] = min(grid[cell], new_z)
```

**Limitation**: Cannot represent overhangs. Adequate for 3-axis.

### Dexel / Tri-Dexel Models

**Dexel**: Ray along one axis storing (z_enter, z_exit) interval pairs. Can represent complex shapes.

**Tri-dexel**: Three orthogonal dexel sets. Much better quality. GPU-acceleratable via depth peeling.

### SVG Input

`usvg` simplifies all SVG paths to: MoveTo, LineTo, QuadTo, CurveTo, ClosePath. Best choice for CAM.

**Bezier to polyline** (adaptive subdivision):
1. Measure flatness (max distance from control points to chord)
2. If flat enough, output chord
3. Otherwise, split at t=0.5 via De Casteljau and recurse

### DXF Input

DXF uses LINE, ARC, CIRCLE, POLYLINE, LWPOLYLINE, SPLINE entities. Maps better to CNC than SVG's beziers.

Rust: `dxf` crate (ixmilia, read/write, active).

---

## Part 2: G-code Generation

### Core Motion Commands

| Command | Name | Use |
|---------|------|-----|
| G0 | Rapid | Repositioning only, never cutting |
| G1 | Linear feed | Primary cutting command |
| G2 | CW arc | Circular interpolation clockwise |
| G3 | CCW arc | Circular interpolation counter-clockwise |

### Arc Specification (G2/G3)

- **I, J method**: Incremental offsets from start to arc center. `G2 X10 Y5 I5 J0 F100`
- **R method**: Radius directly. Ambiguous for arcs > 180deg (use negative R for major arc).

### Essential G/M Codes

```
G17        XY plane (default for 3-axis)
G20/G21    Inch/metric mode
G90/G91    Absolute/incremental
G40        Cancel cutter compensation
G43 H__   Tool length compensation
G49        Cancel tool length comp
G54-G59    Work coordinate systems
M3 S___    Spindle on CW at RPM
M5         Spindle off
M6 T__     Tool change
M8/M9      Coolant on/off
M30        Program end
```

### Controller Differences

| Feature | GRBL v1.1 | LinuxCNC | Mach3/4 |
|---------|-----------|----------|---------|
| Platform | Arduino/AVR | Linux PC (RT kernel) | Windows PC |
| Max axes | 3 (6 grblHAL) | Unlimited | 6 |
| G-code | Subset RS-274 | Full RS-274/NGC | Fanuc-like |
| Canned cycles | None | G80-G89, O-codes | G73, G76, G80-G89 |
| Subroutines | None | O-codes | VB macros |
| Tool change | T only (no M6) | M6 with HAL | M6start/M6end |
| Arc I,J | Incremental only | Incremental or absolute | I,J,K and R |
| Probing | G38.2-G38.5 | G38.2-G38.5 | G31 |

### Post-Processor Architecture

```
Toolpath (intermediate) -> PostProcessor trait -> Machine-specific G-code
```

Trait methods: `preamble()`, `tool_change()`, `rapid_move()`, `linear_move()`, `arc_move()`, `postamble()`.

Concrete implementations per target: GRBL, LinuxCNC, Mach3.

### Arc Fitting (Post-Processing)

Convert dense G1 sequences to G2/G3:

1. **Douglas-Peucker simplification**: Reduce point count within tolerance
2. **Biarc fitting**: Pair of tangent-continuous arcs between endpoints
   - Given two endpoints with tangent directions, find the biarc midpoint
   - Only requires solving a 2x2 linear system
3. **Validate**: Max deviation < machining tolerance

Benefits: smoother finish, smaller files, faster execution.

### Safety Best Practices

- **Program start block**: `G17 G21 G90 G40 G49 G80 M5 M9` (initialize all modal states)
- **Safe Z**: Always retract before any G0 XY move
- **End-of-program**: Retract, stop spindle, coolant off, return to reference, M30
- **Soft limits**: Validate all moves against machine travel limits
- **Tool length comp**: G43 H must match tool number

---

## Part 3: Visualization

### F3D Format -- NOT FEASIBLE
Fusion 360 .f3d is a ZIP with proprietary binary formats (ShapeManager B-rep, .protein, .toolpath). No public spec. Cannot target for output.

### Recommended Approach: External Tools

| Tool | Integration | Purpose |
|------|-------------|---------|
| **CAMotics** | Read G-code output | 3D cutting simulation, free |
| **NC Viewer** (web) | Upload G-code | Quick toolpath visualization |
| **ParaView** | Read VTK files | Scientific 3D visualization |

### Built-in Visualization (Future)

**Recommended stack**: egui + eframe + wgpu

**Rendering approaches**:
1. **Line rendering**: Colored lines (G0=red, G1=blue, G2/G3=green). Simplest.
2. **Tube rendering**: 3D tubes along toolpath. Better depth perception.
3. **Material removal**: Update heightmap progressively as tool moves. Show cut result.

**Intermediate format**: VTK PolyData with lines (exportable via `vtkio` crate).

### Toolpath Output Formats

For external visualization, output G-code and let tools like CAMotics handle simulation. This is the standard open-source workflow and avoids reinventing visualization.
