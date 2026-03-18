# Algorithms Reference

Complete catalog of algorithms needed for a 3-axis CAM program, with mathematical foundations.

---

## 1. Drop-Cutter Algorithm

The foundational algorithm for 3D surface finishing. Given a cutter at (x, y), find the maximum Z where it contacts the STL mesh without gouging.

### Core Concept

For each triangle in the mesh, test three contact types:
1. **Vertex contact**: Cutter touches a triangle vertex
2. **Facet contact**: Cutter touches the triangle face
3. **Edge contact**: Cutter touches a triangle edge

The CL (cutter-location) Z is the **maximum** of all contact Z values across all triangles.

### Vertex Test (All Cutters)

For vertex V at XY distance q from CL position:
```
CL.z = V.z + cutter.height(q)
```
Where `height(q)` is the cutter profile function. Accept if q <= cutter.radius.

### Facet Test (General)

For triangle with normal n = (nx, ny, nz) and plane equation ax + by + cz + d = 0:

```
radiusvector = xy_normal_length * xyNormal + normal_length * n_normalized
CC = CL - radiusvector    (XY only)
CC.z = (1/nz) * (-d - nx*CC.x - ny*CC.y)
tip_z = CC.z + radiusvector.z - center_height
```

Where `xyNormal = normalize_xy(nx, ny, 0)`.

Accept if CC lies inside the triangle (barycentric test).

### Edge Test - Dual Geometry Approach

Instead of testing the original cutter against an edge, test a virtual cylinder (VR) against an inflated edge (ER):

| Cutter | VR (virtual radius) | ER (edge radius) |
|--------|--------------------|--------------------|
| Flat (CylCutter) | R | 0 |
| Ball (BallCutter) | 0 | R |
| Bull (BullCutter) | R1 | R2 |

**CylCutter edge**: Circle-line intersection.
```
s = sqrt(R^2 - d^2)    // d = distance from CL to edge
```

**BallCutter edge**: Ray-cylinder intersection (quadratic in t).
```
a*t^2 + b*t + c = 0
```

**BullCutter edge**: Offset-ellipse method using Brent's root-finding.
The torus cross-section creates an ellipse with axes b=R2, a=|R2/sin(theta)|.
Solve for the point on the offset-ellipse (offset by R1) that passes through CL.

**ConeCutter edge**: Hyperbola intersection.
```
ccu = sign(m) * sqrt(R^2 * m^2 * d^2 / (L^2 - R^2 * m^2))
```
Two cases: hyperbola contact (|m| <= |mu|) or circular rim contact.

### Acceleration: KD-Tree

Store triangles in a KD-tree indexed by bounding box coordinates. For each CL point, query only triangles whose bounding boxes overlap the cutter footprint. Reduces O(n) to O(log n + k).

### Parallelization

Drop-cutter is embarrassingly parallel. Each grid point is independent:
```
grid_points.par_iter().map(|pt| drop_cutter(pt, mesh))
```

### Adaptive Sampling

Instead of uniform grid, recursively subdivide based on flatness:
```
flat(p0, p1, p2) = normalize(p1-p0).dot(normalize(p2-p1)) > cos_limit
```
Default cos_limit = 0.999 (about 2.5 degrees).

---

## 2. Push-Cutter Algorithm

Complementary to drop-cutter. Holds cutter at constant Z, pushes horizontally along a Fiber to find contact intervals.

### Concept

A Fiber is a line segment in XY at constant Z, parameterized by t in [0,1].
For each triangle, compute the interval [t_lower, t_upper] where the cutter would gouge.

### Vertex Push
```
h = vertex.z - fiber.z
cwidth = cutter.width(h)    // effective radius at this height
q = XY distance from fiber to vertex
if q <= cwidth:
    ofs = sqrt(cwidth^2 - q^2)
    interval = [t_vertex - ofs, t_vertex + ofs]
```

### Facet Push

Solve a 2x2 linear system to find the CC point on the facet plane that corresponds to the fiber position.

### Edge Push

Cutter-specific:
- **Ball**: Ray-cylinder intersection (fiber ray vs edge cylinder)
- **Bull**: Aligned offset-ellipse method with Brent solver
- **Cone**: Circle-line or circle+cone intersection

### Applications

Push-cutter is the foundation for the Waterline algorithm.

---

## 3. Waterline / Z-Level Contouring

Generate closed contour toolpaths at constant Z heights.

### Algorithm (Fiber + Weave)

1. **Generate fibers**: Grid of X-fibers and Y-fibers at target Z height
2. **Push-cutter**: Run BatchPushCutter on each fiber set
3. **Build Weave**: Half-edge planar graph from fiber interval intersections
   - CL vertices at interval endpoints
   - INT vertices where X and Y intervals cross
   - Edges connect vertices with next/prev pointers for face traversal
4. **Extract loops**: Follow next-edge pointers from CL vertices to build closed contour loops

### Adaptive Waterline

Recursive fiber subdivision based on flatness (same predicate as adaptive drop-cutter). Inserts additional fibers where contour curvature is high.

---

## 4. Contour-Parallel Pocketing

### Offset Approach (Clipper-based)

1. Start with pocket boundary polygon
2. Offset inward by tool radius (first pass)
3. Continue offsetting inward by step-over
4. Stop when offset polygon collapses to nothing
5. Handle islands via boolean difference before offsetting

### Voronoi/Proximity Map Approach (Held)

Use the medial axis (Voronoi diagram) of the pocket boundary to compute offsets that naturally handle global interference. More robust for complex shapes.

### CavalierContours Approach

Seven-step slice-and-stitch algorithm that preserves arcs through the offsetting process, producing G2/G3-compatible output.

### Island Handling

```
effective_pocket = pocket_boundary DIFFERENCE union(island_boundaries)
offset_paths = offset_inward(effective_pocket, step_over)
```

---

## 5. Adaptive Clearing (Constant Engagement)

### Freesteel/Adaptive2d Algorithm

The cutter acts as an "agent" making local decisions:

1. **Find entry point**: Outside-in approach or helix entry into uncleared material
2. **At each step**:
   - Search for direction angle producing target cut area
   - Cut area = engagement angle * radial depth
   - Target: `minCutArea <= cutArea <= maxCutArea`
   - Binary search over angles with 5% tolerance, max 10 iterations
3. **Maintain angle history** (3 points) for directional prediction
4. **When blocked**: Find new entry point for remaining material
5. **Post-process**: Smooth paths, chain segments by proximity

### Key Parameters

```
toolDiameter = 5mm
stepOverFactor = 0.2        // 20% step-over equivalent
tolerance = 0.1mm
AREA_ERROR_FACTOR = 0.05    // 5% tolerance in engagement
MAX_ITERATIONS = 10         // per angle search
keepToolDownDistRatio = 3.0 // link vs retract threshold
```

### Engagement Angle

```
alpha = arccos(1 - WOC/R)
WOC = R * (1 - cos(alpha))    // inverse
```

Where WOC = width of cut (radial), R = tool radius.

---

## 6. Mesh Slicing (Z-Level Cross Sections)

### Triangle-Plane Intersection

For horizontal plane at z_slice:

1. Classify vertices: above (+), below (-), on (0) the plane
2. For edges crossing the plane, interpolate:
   ```
   t = (z_slice - v_a.z) / (v_b.z - v_a.z)
   p = v_a + t * (v_b - v_a)
   ```
3. Each crossing triangle produces one line segment

### Contour Assembly

Chain unordered segments into closed loops by matching endpoints (hash-based adjacency map).

### Optimal Complexity

Sweep-plane algorithms achieve O(n log k + k + m) for n triangles, k slicing planes, m output segments.

---

## 7. Polygon Offsetting

### Minkowski Sum Approach

Offset = Minkowski sum of polygon with circle of offset radius. Conceptually correct but expensive.

### Clipper2 Approach

Move each edge outward/inward by offset distance, resolve self-intersections via Vatti line sweep. Join types:
- **Round**: Circular arc at convex corners (what CAM needs for cutter compensation)
- **Miter**: Extended sharp corners (limited by miter limit)
- **Square**: Right-angle extensions

### Arc-Preserving Offset (CavalierContours)

Offsets polylines with arc segments, preserving arc curvature. Produces better G-code output (G2/G3 arcs instead of many G1 line segments).

---

## 8. Scallop Height Calculation

### Ball End Mill on Flat Surface

```
h = R - sqrt(R^2 - (stepover/2)^2)
stepover = 2 * sqrt(2*R*h - h^2)
Approximation: h ~ stepover^2 / (8*R)
```

### Ball End Mill on Curved Surface

Effective radius accounting for surface curvature:
- Convex: R_eff = R * R_surface / (R + R_surface)
- Concave: R_eff = R * R_surface / (R_surface - R)
- Apply flat-surface formula with R_eff

### Tapered Ball End Mill

The ball tip radius defines scallop height when finishing with the ball portion. The taper only contacts on steeper surfaces where the ball wouldn't reach.

---

## 9. Stock Modeling / Material Removal Simulation

### Heightmap (Z-Buffer)

2D grid storing maximum Z at each (x, y) cell. Updated by computing tool profile at each cutter position:
```
for each grid cell in tool footprint:
    new_z = cutter_z + cutter.height(xy_distance)
    grid[cell] = min(grid[cell], new_z)
```

**Limitation**: Cannot represent overhangs (adequate for 3-axis).

### Dexel Model

Rays along one axis storing (z_enter, z_exit) interval pairs. Can represent complex shapes.

### Tri-Dexel

Three orthogonal dexel sets (X, Y, Z). Much better representation quality. GPU-acceleratable via depth peeling.

---

## 10. Toolpath Linking & Optimization

### Retract Strategies

1. **Full retract**: To safe Z, rapid to next position, plunge. Safest but slowest.
2. **Partial retract**: Retract to clearance plane (just above stock). Faster.
3. **Keep tool down**: If distance to next cut < threshold, stay at cutting Z. Fastest but requires collision checking.

### Path Ordering (TSP)

Model disconnected path segments as cities. Use approximate TSP to minimize total rapid distance:
- Nearest-neighbor heuristic (fast, ~20% suboptimal)
- 2-opt improvement (iterative swap)
- Metric TSP approximation (Boost/Christofides)

### Entry Strategies

- **Ramp**: 2-5 degree slope while moving along first segment
- **Helix**: Spiral at 50-75% tool diameter, descend at configured rate
- **Plunge**: Only for center-cutting tools (ball, drill)

### Arc Fitting (Post-Processing)

Convert dense G1 point sequences to G2/G3 arcs:
1. Douglas-Peucker simplification (tolerance = half machining error)
2. Biarc fitting: pair of tangent-continuous arcs between points
3. Validate deviation < tolerance

---

## 11. V-Carving Algorithm

### Maximum Inscribed Circle Approach

```
for each point inside design:
    r = distance_to_nearest_boundary
    depth = r / tan(half_angle)
```

### Medial Axis Approach

1. Compute Voronoi diagram of boundary line segments
2. Filter to obtain medial axis (skeleton)
3. Tool follows medial axis with depth = distance_to_boundary / tan(half_angle)

### Flat-Bottom V-Carving

When design is wider than max allowed depth:
1. Clamp depth to maximum
2. Compute flat bottom region (where inscribed circle > max_radius)
3. Clear flat bottom with flat end mill
4. V-bit only cuts the tapered edges

---

## 12. Feeds & Speeds Calculation

### Fundamental Equations

```
F = fz * z * N          // feed rate (mm/min)
Vc = pi * D * N / 1000  // cutting speed (m/min)
N = Vc * 1000 / (pi*D)  // RPM from cutting speed
MRR = ap * ae * F       // material removal rate
```

Where: fz = chip load per tooth, z = flute count, N = RPM, D = diameter, ap = axial depth, ae = radial depth.

### Effective Diameter (Ball End Mill)

At axial depth ap:
```
D_eff = 2 * sqrt(ap * (D - ap))
```
Critical: at shallow depths, effective speed approaches zero at the tip.

### Wood Guidelines

| Tool | Chip Load (mm/tooth) | Softwood DOC | Hardwood DOC |
|------|---------------------|--------------|--------------|
| 1/4" | 0.09 | 4-9mm | 2-4mm |
| 1/8" | 0.06 | 2-5mm | 1-3mm |
| 1/16" | 0.03 | 1-3mm | 0.5-2mm |

Plunge rate: typically 50% of horizontal feed rate.
