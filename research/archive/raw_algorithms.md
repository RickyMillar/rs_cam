# CAM Algorithms for 3-Axis CNC Milling: Comprehensive Research

A deep-dive research report on algorithms for an open-source Rust CAM program focused on wood routing.

---

## Table of Contents

1. [Drop-Cutter Algorithms](#1-drop-cutter-algorithms)
2. [Adaptive Clearing / Trochoidal Milling](#2-adaptive-clearing--trochoidal-milling)
3. [Contour-Parallel / Offset Pocketing](#3-contour-parallel--offset-pocketing)
4. [3D Surface Finishing Strategies](#4-3d-surface-finishing-strategies)
5. [Z-Level / Waterline Machining](#5-z-level--waterline-machining)
6. [Scallop Height Calculation](#6-scallop-height-calculation)
7. [Stock Modeling / Material Removal Simulation](#7-stock-modeling--material-removal-simulation)
8. [Toolpath Linking and Optimization](#8-toolpath-linking-and-optimization)
9. [Open-Source Implementations Reference](#9-open-source-implementations-reference)
10. [Rust Ecosystem Libraries](#10-rust-ecosystem-libraries)

---

## 1. Drop-Cutter Algorithms

### Overview

The drop-cutter (also called "axial tool projection") is the foundational algorithm for 3-axis surface finish toolpath generation. Given a cutter positioned at coordinates (x, y), the algorithm determines the maximum Z-height at which the cutter can be placed without gouging (cutting into) the triangulated model. The cutter is conceptually "dropped" along the Z-axis until it makes first contact with the STL mesh.

### When to Use

- **3D surface finishing** (raster/zigzag passes, spiral finishing)
- **Path drop cutter**: applying drop-cutter along a pre-defined XY path
- Any toolpath where the XY trajectory is known and only Z must be computed

### Core Algorithm

For each (x, y) position, the algorithm must test the cutter against every nearby triangle in the STL mesh. Since a triangle has three vertices, three edges, and one facet, there are **seven potential contact elements** per triangle. The algorithm runs all seven tests and selects the one producing the **highest Z value** (the first contact point when dropping from above).

```
function drop_cutter(x, y, triangles, cutter):
    cl_z = -INFINITY
    cc_point = null

    for each triangle T in nearby_triangles(x, y):
        // Test 1: Three vertex tests
        for each vertex V in T.vertices:
            z = vertex_drop(x, y, V, cutter)
            if z > cl_z:
                cl_z = z
                cc_point = V

        // Test 2: Facet test
        z = facet_drop(x, y, T, cutter)
        if z > cl_z:
            cl_z = z

        // Test 3: Three edge tests
        for each edge E in T.edges:
            z = edge_drop(x, y, E, cutter)
            if z > cl_z:
                cl_z = z

    return CLPoint(x, y, cl_z, cc_point)
```

### Cutter Geometry Definitions

Each cutter type is defined by a **height function** h(r) giving the Z-offset of the cutter surface at radial distance r from the axis, and a **width function** w(h) giving the maximum radial extent at height h.

#### Cutter Type Table (OpenCAMLib Notation)

| Cutter          | Notation    | height(r)                                           | width(h)                                         |
|-----------------|-------------|-----------------------------------------------------|--------------------------------------------------|
| Cylindrical     | C(d, 0)    | 0 for all r <= R                                    | R for all h >= 0                                 |
| Ball (Spherical)| C(d, R)    | R - sqrt(R^2 - r^2)                                 | sqrt(R^2 - (R-h)^2) = sqrt(2Rh - h^2)           |
| Bull (Toroidal) | C(d, r2)   | 0 if r<=R1; r2 - sqrt(r2^2 - (r-R1)^2) if R1<r<=R  | R if h>=r2; R1 + sqrt(r2^2 - (r2-h)^2) if h<r2  |
| Cone            | C(d, angle) | r / tan(angle)                                      | h * tan(angle) if h < center_height; else R      |

Where R = d/2 (total cutter radius), R1 = R - r2 (for bull cutter), r2 = corner radius.

#### Offset Equivalences (Critical for Edge Tests)

The key insight in opencamlib's design is the **virtual/offset cutter duality**:

| Cutter Type      | Virtual Radius (VR) | Edge Radius (ER) | Offset Radius (OR) |
|------------------|---------------------|-------------------|---------------------|
| CylCutter(R)     | R                   | 0                 | R                   |
| BallCutter(R)    | 0                   | R                 | 0                   |
| BullCutter(R1,R2)| R1 - R2             | R2                | R1 - R2             |

This means: "dropping a cutter with edge-radius ER against a zero-radius edge is equivalent to dropping a zero-edge-radius cutter against an ER-radius cylindrical edge."

### Vertex Drop Test (All Cutters)

The vertex test is the simplest. Given cutter at (cl.x, cl.y) and vertex at point P:

```
q = xy_distance(cl, P)    // sqrt((cl.x - P.x)^2 + (cl.y - P.y)^2)
if q <= cutter.radius:
    cl_z = P.z - cutter.height(q)
    // Update CL point if cl_z is higher than current
```

The height(q) function returns the Z-offset at radial distance q for the specific cutter geometry.

**Ball cutter**: cl_z = P.z - (R - sqrt(R^2 - q^2))
**Flat cutter**: cl_z = P.z - 0 = P.z
**Bull cutter**: cl_z = P.z - 0 if q <= R1; cl_z = P.z - (r2 - sqrt(r2^2 - (q-R1)^2)) if q > R1
**Cone cutter**: cl_z = P.z - q/tan(angle)

### Facet Drop Test (All Cutters)

The facet test determines the Z-height at which the cutter contacts the interior plane of a triangle.

Given triangle with normal vector n = (a, b, c) and plane equation ax + by + cz + d = 0:

**General case:**
1. Compute a "radius vector" that points from the cutter contact (CC) point to the cutter location (CL) point, projected in the XY plane:
   - `normal_xy = normalize_xy(n)` (the XY projection of the normal, normalized)
   - `radius_vector = xy_normal_length * normal_xy + normal_length * n`
   - Where `xy_normal_length` and `normal_length` are cutter-specific offset parameters
2. CC point: `cc = cl - radius_vector` (the CC lies offset from CL along the radius vector)
3. The CC point must satisfy the plane equation: `cc.z = (-d - a*cc.x - b*cc.y) / c`
4. Verify the CC point lies **inside** the triangle (barycentric test)
5. CL height: `cl_z = cc.z + (some cutter-specific offset)`

**Horizontal plane case** (normal.z ~ 1.0):
- CC point is directly below CL at (cl.x, cl.y, triangle.z)
- For ball cutter: cl_z = triangle.z + R (center is R above the flat surface contact)

**Cone cutter special case**: Contact can occur at the tip OR the circular rim. Both must be tested and the higher result kept.

### Edge Drop Test (All Cutters) -- The Hard Part

The edge test determines where the cutter, dropped at (x,y), contacts a triangle edge (line segment P1-P2). This is the most complex test, especially for toroidal cutters.

#### Canonical Transformation

All edge tests first transform the geometry into a "canonical" configuration:
1. Translate so CL = (0, 0, *)
2. Rotate in XY so edge P1-P2 aligns with the X-axis
3. Resulting edge points: u1, u2 where u1.y = u2.y = d (perpendicular distance from CL to edge)

After solving in canonical form, the result is transformed back.

#### Ball Cutter Edge Test

```
d = u1.y    // perpendicular distance from CL axis to edge
if |d| > R: return NO_CONTACT

s = sqrt(R^2 - d^2)   // half-width of sphere cross-section at distance d

// Normal to edge in the plane containing cL axis and edge
normal = (u2.z - u1.z, -(u2.x - u1.x), 0)
normal.xy_normalize()

// CC point on the edge
cc.x = -s * normal.x
cc.y = d    // = u1.y
cc.z = z_project_onto_edge(cc, u1, u2)  // linear interpolation along edge

// CL height
cl_z = cc.z + s * normal.y - R
```

The contact point is where the sphere cross-section (a circle of radius s in the plane at distance d from the axis) is tangent to the edge.

#### Flat (Cylindrical) Cutter Edge Test

```
d = u1.y
if |d| > R: return NO_CONTACT

s = sqrt(R^2 - d^2)
// Two candidate contact points at x = +s and x = -s
cc1 = (s, d, z_project(s, u1, u2))
cc2 = (-s, d, z_project(-s, u1, u2))
// Choose the higher one
```

#### Bull (Toroidal) Cutter Edge Test -- Offset Ellipse Method

This is the most mathematically complex test. The key insight:

1. **Horizontal edge** (u1.z == u2.z): Simple -- use `CC_CLZ_Pair(0, u1.z - height(u1.y))`

2. **Sloped edge**: Uses the **offset-ellipse method**:
   - When you slice a cylinder of radius r2 (the corner radius) with the plane containing the edge, you get an **ellipse**
   - The ellipse has semi-axes: `b_axis = r2` (short) and `a_axis = |r2 / sin(theta)|` (long), where theta = atan(slope of edge)
   - The center of the ellipse is at `(0, u1.y, 0)`
   - The offset distance is R1 (= R - r2), the distance from cutter axis to torus center circle
   - A point on the offset-ellipse (ellipse point + R1 * outward_normal) must coincide with CL = (0, 0)
   - This requires solving a nonlinear equation using **Brent's root-finding method**
   - Two solutions exist (upper and lower torus contact); select the one with higher Z

```
// Offset-ellipse setup
theta = atan2(u2.z - u1.z, u2.x - u1.x)
a_axis = |r2 / sin(theta)|
b_axis = r2
offset_distance = R1  // = R - r2

// Brent's method finds parameter t such that:
// ellipse_point(t) + offset_distance * normal(t) = (0, 0) in XY
// where ellipse_point(t) = (a_axis * cos(t), b_axis * sin(t))
//       normal(t) = normalized gradient of ellipse at t

solve for t using Brent's method
cc = closest_point_on_edge(ellipse_point(t), u1, u2)
cl_z = ellipse_center_z(t) - r2
```

#### Cone Cutter Edge Test

Two sub-cases depending on edge slope relative to cone half-angle:

```
d = u1.y    // perpendicular distance
m = edge_slope = (u2.z - u1.z) / (u2.x - u1.x)
xu = sqrt(R^2 - d^2)
mu = (center_height / R) * xu / sqrt(xu^2 + d^2)  // max contact slope

if |m| <= |mu|:
    // Hyperbolic contact (cone surface touches edge)
    ccu = sign(m) * sqrt(R^2 * m^2 * d^2 / (L^2 - R^2 * m^2))
    // where L = center_height / tan(angle)
    cl_z = cc.z - center_height + (R - sqrt(ccu^2 + d^2)) / tan(angle)

else:
    // Circular edge contact (rim of cone base)
    ccu = sign(m) * xu
    cl_z = cc.z - center_height
```

### Spatial Acceleration

Testing every triangle for every CL point is O(n*m). Acceleration structures are essential:

- **KD-Tree**: OpenCAMLib uses a KD-tree to spatially index triangles. For each CL point, only triangles whose bounding boxes overlap the cutter's XY footprint are tested.
- **Bucket size**: Configurable leaf node size in the KD-tree trades off construction time vs. query time.
- **OpenMP parallelization**: BatchDropCutter distributes CL points across threads.

### Complexity

- **Per CL point**: O(k) where k = number of nearby triangles (typically small with good spatial indexing)
- **Total for N CL points with spatial index**: O(N * k * log(T)) where T = total triangles
- **Without spatial index**: O(N * T) -- unusable for large meshes

---

## 2. Adaptive Clearing / Trochoidal Milling

### Overview

Adaptive clearing is a roughing strategy that maintains roughly constant tool engagement throughout the cut, enabling aggressive depths of cut (full flute length) while preventing the tool load spikes that cause breakage. It was pioneered by Julian Todd and Martin Dunschen at Freesteel around 2004 and later acquired by HSMWorks/Autodesk.

### When to Use

- **Roughing**: The primary application. Removes the bulk of material before finishing.
- **Pocketing**: Clearing pocket interiors without full-width slotting.
- **Any scenario** where traditional offset clearing would cause engagement spikes at corners.

### The Engagement Angle Problem

With traditional contour-parallel (equidistant offset) roughing:
- Engagement angle on straight sections: `alpha = arccos(1 - ae/R)` where ae = stepover, R = tool radius
- At **convex corners**: engagement drops (less material)
- At **concave (interior) corners**: engagement can spike to 180 degrees (full slotting)
- These spikes cause vibration, deflection, and tool breakage

The goal is to keep the engagement angle constant (typically 40-90 degrees) throughout the entire toolpath.

### Key Formulas

**Engagement angle from radial depth of cut (stepover):**
```
alpha_en = arccos(1 - WOC / R)
```
Where:
- alpha_en = engagement angle (radians)
- WOC = width of cut (radial depth, stepover)
- R = tool radius

**Inverse -- stepover from engagement angle:**
```
WOC = R * (1 - cos(alpha_en))
```

These formulas work for engagement angles > 90 degrees as well.

### Freesteel's Approach

The Freesteel adaptive clearing algorithm is fundamentally different from geometric trochoidal methods:

1. **Stock-model driven**: The algorithm maintains a precise model of remaining (uncut) material at all times. This is the critical differentiator.

2. **Not explicitly trochoidal**: Trochoidal and spiral motions are "emergent properties" of the algorithm, not geometrically prescribed. The algorithm does not attach circles to offset paths.

3. **Core loop** (conceptual):
   ```
   while material_remains:
       current_stock = get_stock_model()
       next_cut = compute_cut_maintaining_engagement(
           current_position, current_stock, max_engagement_angle)
       execute(next_cut)
       update_stock_model(next_cut)
   ```

4. **Key challenge**: Computing remaining stock fast enough without consuming excessive memory. The stock model must be updated after every cut segment.

5. **Result**: The tool can safely cut along the full flute length, keeping helical blades in constant contact with material (reducing vibration), while never encountering too much uncut material.

### Stori & Wright's Constant Engagement Approach

An alternative algorithmic approach by Stori and Wright generates non-equidistant offset paths:

1. Start from the pocket boundary
2. For each point on the current path, compute the **offset distance** that would produce the target engagement angle
3. This offset distance varies along the path based on local curvature:
   - At convex sections: smaller offset (move closer to previous path)
   - At concave sections: larger offset (move farther from previous path)
4. The result is a spiral-like path with continuously varying stepover

### Fast Constant Engagement Offsetting Method (FACEOM)

A more recent algorithm by Biro et al.:

1. **Input**: Previous boundary (polygon), target engagement angle
2. **For each point on the boundary**:
   - Cast a ray normal to the boundary
   - Compute the offset distance d that achieves the target engagement angle
   - `d = R * (1 - cos(alpha_target))` for straight sections
   - For curved sections, adjust based on local curvature of the boundary
3. **Boolean operations**: Build the offset using trapezoids and circular sectors, then union them
4. **Extract boundary** of the union as the next toolpath

**Accuracy**: +/- 1 degree engagement angle is sufficient for uniform tool load.

**Adaptive step size**: Points along the path are spaced adaptively based on curvature, with spline interpolation between them.

### Trochoidal Milling (Geometric Approach)

Pure trochoidal milling prescribes the geometry explicitly:

**Parametric trochoidal path:**
```
x(t) = v_feed * t + r_troch * cos(omega * t)
y(t) = r_troch * sin(omega * t)
```
Where:
- v_feed = linear feed advance rate
- r_troch = trochoidal radius
- omega = angular velocity of the circular component

**Engagement angle for trochoidal motion:**
- Varies depending on the ratio of trochoidal radius to slot width
- Can be controlled by adjusting r_troch and the linear advance per revolution

**Medial axis approach**: For complex pocket shapes, the medial axis of the pocket boundary is computed, then trochoidal circles are placed along the medial axis, with radii sized to fit within the pocket at each point (inscribed circles/ellipses).

### Open-Source Implementations

- **libactp** (GPL): The original Freesteel adaptive clearing algorithm, available at github.com/Heeks/libactp-old. Written in C++.
- **pyactp**: Python 3 bindings for libactp.
- **BlenderCAM**: Uses libactp for adaptive clearing operations.
- **FreeCAD Path**: Has adaptive clearing support.

---

## 3. Contour-Parallel / Offset Pocketing

### Overview

Contour-parallel pocketing generates toolpaths by repeatedly offsetting the pocket boundary inward by the stepover distance. This is the most common 2.5D pocketing strategy.

### When to Use

- **2.5D pocket clearing**: Flat-bottomed pockets with vertical walls
- **Face milling**: Clearing flat areas
- **Any situation** requiring area coverage with a flat-end mill

### The Polygon Offset Problem

The fundamental operation is: given a polygon (possibly with holes/islands), compute a new polygon offset inward by distance d.

**Naive approach**: Offset each edge by d along its inward normal, then find intersections of adjacent offset edges.

**Problems that arise**:
1. **Self-intersections**: At concave vertices (interior corners), offset edges overlap, creating loops
2. **Topology changes**: An offset polygon may split into multiple disjoint polygons
3. **Disappearing features**: Small features vanish when offset distance exceeds their size
4. **Islands**: Interior boundaries (islands) must be offset outward while the exterior boundary is offset inward

### Algorithm 1: Raw Offset + Boolean Cleanup (Clipper Approach)

This is the approach used by the Clipper/Clipper2 library:

```
function offset_polygon(polygon, distance):
    1. For each edge, compute a parallel edge offset by |distance| along the edge normal
    2. At convex vertices: insert join geometry (miter, round, or square)
    3. At concave vertices: edges naturally intersect -- leave the intersection
    4. The result is a "raw offset curve" that may self-intersect
    5. Perform a UNION boolean operation on the raw offset
       - Uses Vatti's sweep-line polygon clipping algorithm
       - Self-intersections are automatically resolved
       - Invalid loops (with non-positive winding numbers) are eliminated
    6. Return the cleaned polygon(s)
```

**Join types for convex vertices**:
- **Miter**: Extend edges to intersection point (with a miter limit to prevent spikes)
- **Round**: Insert circular arc of radius = offset distance
- **Square**: Extend edges by offset distance, then connect with a flat segment
- **Bevel**: Connect offset edge endpoints with a straight line

**Winding number rule**: After creating the raw offset with all self-intersections, compute winding numbers for each region. Regions with winding number <= 0 are invalid and removed. This elegantly handles all topology changes.

**Complexity**: O((n + k) log n) where n = vertices, k = self-intersections. k can be O(n^2) when offset distance approaches the local radius of curvature.

### Algorithm 2: Voronoi Diagram / Proximity Map (Held's Approach)

Martin Held's approach (1991, 1994) uses Voronoi diagrams:

```
function voronoi_pocketing(boundary, stepover, tool_radius):
    1. Compute the Voronoi diagram of the pocket boundary
       (For line segments and arcs, this is the "medial axis transform")
    2. The Voronoi diagram encodes, for every interior point,
       the distance to the nearest boundary element
    3. Offset curves at distance d correspond to the locus of points
       at distance d from the boundary
    4. The Voronoi diagram provides the "proximity map" --
       at each Voronoi vertex, the equidistant boundary elements change
    5. Trace offset curves at distances: tool_radius, tool_radius + stepover,
       tool_radius + 2*stepover, etc.
    6. At Voronoi edges/vertices, handle topology changes
       (curves splitting, merging, disappearing)
```

**Advantages**:
- Handles arbitrary boundary shapes (lines, arcs, free-form curves)
- Naturally tracks topology changes as offset distance increases
- Efficient: Voronoi computed once, then all offsets traced cheaply
- No self-intersection problems since the Voronoi structure encodes the topology

**Open-source**: Anders Wallin's **OpenVoronoi** (github.com/aewallin/openvoronoi) implements a 2D Voronoi diagram for point and line-segment sites using an incremental topology-oriented algorithm (Sugihara-Iri / Held approach). Written in C++ with Python bindings. Licensed LGPL2.1.

### Algorithm 3: Cavalier Contours (Modern Offset Approach)

The CavalierContours library implements a 7-step offset algorithm with native arc support:

```
1. Generate raw offset segments from input polyline
   (Each line segment offsets to a parallel line; each arc offsets to a concentric arc)
2. Create raw offset polyline by trimming/joining adjacent segments
   - Always use arcs to connect adjacent segments at convex joints
   - This maintains constant distance from original polyline
3. If input has self-intersections or is open: compute dual offset
4. Detect self-intersections using spatial indexing (Packed Hilbert R-Tree)
5. Slice the polyline at all intersection points
6. Filter: discard slices that are too close to the original polyline
7. Stitch remaining open polylines into final closed results
```

**Key advantage**: Native arc segment support means no need to approximate curves with line segments. This is significantly more efficient for CNC where arcs are native G-code (G02/G03).

### Island Handling

Islands (holes within pockets) require special treatment:

1. The pocket exterior boundary is offset **inward**
2. Each island boundary is offset **outward**
3. After offsetting, perform **boolean difference**: exterior_offset MINUS island_offsets
4. Repeat for each offset level
5. Islands may merge with each other or with the exterior boundary at deeper offset levels
6. The Voronoi approach handles this naturally since the Voronoi diagram of the boundary+islands encodes all distance relationships

### Generating the Complete Pocketing Toolpath

```
function generate_pocket_toolpath(boundary, islands, tool_radius, stepover):
    offset_distance = tool_radius
    paths = []

    while true:
        offset_result = offset_polygon(boundary, -offset_distance)
        for each island in islands:
            island_offset = offset_polygon(island, +offset_distance)
            offset_result = boolean_difference(offset_result, island_offset)

        if offset_result is empty:
            break

        paths.append(offset_result)
        offset_distance += stepover

    return paths  // List of closed loops at each offset level
```

---

## 4. 3D Surface Finishing Strategies

### 4.1 Raster / Zigzag Finishing

**What it does**: Generates parallel toolpath lines at a fixed XY angle, applying drop-cutter along each line to determine Z heights.

**Algorithm**:
```
function raster_finish(stl_model, cutter, angle, stepover, bounds):
    // Rotate coordinate system by 'angle'
    paths = []
    y = bounds.y_min

    while y <= bounds.y_max:
        path = []
        x = bounds.x_min
        while x <= bounds.x_max:
            z = drop_cutter(x_rotated, y_rotated, stl_model, cutter)
            path.append(CLPoint(x, y, z))
            x += forward_step
        paths.append(path)
        y += stepover

    // Alternate direction for zigzag (reverse every other path)
    for i in 0..paths.len():
        if i % 2 == 1:
            paths[i].reverse()

    return paths
```

**Contact angle guideline**: Raster finishing is best when the cutter contact angle is 0-40 degrees from vertical (i.e., shallow to moderate slopes). Steep areas (40-90 degrees) are better handled by waterline.

**Adaptive step-forward**: Instead of uniform X steps, use adaptive sampling that places more points where the surface changes rapidly and fewer on flat regions. The collinearity criterion: if three consecutive points are nearly collinear in 3D, the middle point can be removed.

### 4.2 Spiral Finishing

**What it does**: Generates a continuous spiral path over the surface, eliminating retract moves.

**Approaches**:

1. **Archimedean spiral**: Starting from center, spiral outward with constant radial stepover
   ```
   r(theta) = a + b * theta   (where b = stepover / (2*pi))
   x(theta) = center_x + r(theta) * cos(theta)
   y(theta) = center_y + r(theta) * sin(theta)
   z(theta) = drop_cutter(x, y, model, cutter)
   ```

2. **Fermat spiral**: Connected Fermat spirals for complex shapes. The key property is that they produce mostly long, low-curvature paths (unlike Peano or Hilbert curves). Formulated by Zhao et al. for layered fabrication but applicable to CNC finishing.

3. **Morphing spiral**: For arbitrary pocket shapes, compute contour-parallel offsets at multiple levels, then interpolate between adjacent offset loops to create a smooth spiral transition. This ensures no retract moves and smooth cutting.

4. **Held & Spielberger medial axis spiral**: Place growing disks on the medial axis of the pocket. The spiral interpolates between these disks, starting inside and spiraling outward. Guaranteed free of self-intersections.

### 4.3 Constant Scallop Height Finishing

**What it does**: Varies the stepover distance to maintain a uniform scallop height across the entire surface, regardless of local surface curvature.

**Why it matters**: With a fixed stepover, scallop height varies dramatically:
- On flat areas: moderate scallops
- On convex surfaces: larger scallops (ball mill rides on top)
- On concave surfaces: smaller scallops (ball mill nests inside)

**Algorithm**:
```
function constant_scallop_finish(model, ball_cutter, target_scallop_h):
    // Start with a seed path (e.g., boundary or previous waterline)
    current_path = seed_path

    all_paths = [current_path]

    while surface_not_covered:
        next_path = []
        for each point P on current_path:
            // Compute local surface curvature at P
            kappa = surface_curvature_perpendicular_to_feed(P)

            // Compute effective radius
            R_eff = effective_radius(ball_cutter.R, kappa)

            // Compute stepover for target scallop height
            stepover = 2 * sqrt(target_scallop_h * (2 * R_eff - target_scallop_h))

            // Offset P by stepover in the cross-feed direction
            next_point = offset_on_surface(P, stepover, cross_feed_direction)
            next_path.append(next_point)

        all_paths.append(next_path)
        current_path = next_path

    return all_paths
```

**Effective radius** (accounts for surface curvature):
```
// For a ball mill of radius R on a surface with principal curvature kappa:
// Convex surface (kappa > 0): R_eff = R * rho / (R + rho)  where rho = 1/kappa
// Flat surface (kappa = 0):   R_eff = R
// Concave surface (kappa < 0): R_eff = R * |rho| / (|rho| - R)  for |rho| > R
```

### 4.4 Pencil Finishing

**What it does**: Traces the cutter along sharp concave corners and fillets where previous finishing passes left excess material. These are the internal corners where larger tools couldn't reach.

**When to use**: After raster or waterline finishing, to clean up concave fillets with radii smaller than or equal to the tool corner radius.

**Algorithm** (Visibility-based pencil curve detection):
```
function detect_pencil_curves(model, ball_cutter):
    // Method: find where the ball-end mill makes simultaneous
    // contact with two or more surfaces

    1. Compute the CL surface (offset of STL by cutter radius R)
    2. The CL surface self-intersects at concave regions
    3. "Pencil curves" = the self-intersection curves of the CL surface
    4. These can be detected by:
       a. Rendering the CL surface from the Z direction (visibility/Z-buffer)
       b. Identifying points where multiple offset triangles overlap
       c. Valid intersections lie on the "outer skin" of the offset mesh
       d. Connect valid intersections into continuous curves
    5. Drop-cutter along each detected pencil curve for final Z values
```

**Alternative approach** (curve-based scanning):
- Scan the model with cutting planes in XZ, YZ, and XY directions
- At each plane, detect where the ball-end mill contacts two surfaces simultaneously
- The locus of these double-contact points forms the pencil curves

### 4.5 Rest Machining

**What it does**: Detects areas where a previous (larger) tool left uncut material and generates toolpaths for a smaller tool to clean those areas.

**Algorithm**:
```
function rest_machining(model, small_cutter, large_cutter, previous_toolpath):
    // Method 1: Stock comparison
    1. Simulate the material removed by the large cutter (build stock model)
    2. Compare stock model against the design model
    3. Where stock_height - design_height > tolerance: material remains
    4. Generate toolpath for small_cutter only in those regions

    // Method 2: Geometric comparison
    1. Compute CL surface for large cutter (CL_large)
    2. Compute CL surface for small cutter (CL_small)
    3. Where CL_small.z < CL_large.z: the small cutter can reach deeper
    4. These regions define where rest machining is needed
```

---

## 5. Z-Level / Waterline Machining

### Overview

Waterline (Z-level, Z-slice, contour) machining generates toolpaths at constant Z-heights. At each Z level, the toolpath is a set of closed contour loops. This is essential for machining steep walls and vertical surfaces.

### When to Use

- **Steep surfaces** (contact angle 30-90 degrees from horizontal)
- **Roughing** (Z-level clearing at each depth)
- **Semi-finishing** before final raster/scallop finish

### Algorithm Overview

There are two main approaches:

#### Approach 1: Mesh Slicing (Direct CL Surface Slicing)

```
function waterline_slice(stl_model, cutter, z_height):
    // Step 1: Build the CL surface
    //   For a ball cutter: offset each triangle vertex by R along its vertex normal
    //   For a flat cutter: offset each triangle upward by 0, but expand edges outward by R
    //   For a bull cutter: combination approach

    cl_surface = offset_mesh(stl_model, cutter)

    // Step 2: Slice the CL surface at z = z_height
    segments = []
    for each triangle T in cl_surface:
        if T crosses z_height:
            seg = intersect_triangle_plane(T, z_height)
            segments.append(seg)

    // Step 3: Assemble segments into closed loops
    loops = assemble_contours(segments)
    return loops
```

**Triangle-plane intersection**: For a triangle with vertices V1, V2, V3 at a horizontal plane z = c:
```
// Classify vertices as above/below the plane
for each vertex: above[i] = (V[i].z > c)

// If all above or all below: no intersection
// If one vertex is separated from the other two:
//   Compute two intersection points by linear interpolation along edges
//   For edge (Va, Vb) where Va.z != Vb.z:
//     t = (c - Va.z) / (Vb.z - Va.z)
//     intersection = Va + t * (Vb - Va)
//   The intersection is a line segment between the two intersection points
```

**Contour assembly**:
```
function assemble_contours(segments):
    // Build a hash map: endpoint -> [adjacent segments]
    // Follow the chain: pick a segment, follow to the next segment sharing
    // an endpoint, continue until the loop closes
    // Handle multiple disjoint loops
    // Time complexity: O(n) with hash-based lookup
```

#### Approach 2: Push-Cutter + Weave (OpenCAMLib Approach)

This avoids explicitly constructing the CL surface:

```
function waterline_pushcutter(stl_model, cutter, z_height, x_sampling, y_sampling):
    // Step 1: Create X-direction fibers (horizontal lines at z_height)
    x_fibers = []
    for y in y_min..y_max step y_sampling:
        fiber = Fiber(direction=X, y=y, z=z_height)
        push_cutter_along_fiber(fiber, stl_model, cutter)
        // Each fiber now contains intervals of valid/invalid CL positions
        x_fibers.append(fiber)

    // Step 2: Create Y-direction fibers
    y_fibers = []
    for x in x_min..x_max step x_sampling:
        fiber = Fiber(direction=Y, x=x, z=z_height)
        push_cutter_along_fiber(fiber, stl_model, cutter)
        y_fibers.append(fiber)

    // Step 3: Build the Weave
    //   The weave is a planar graph where X-fibers and Y-fibers intersect
    //   At each grid intersection, the weave records whether the point is
    //   inside or outside the machinable region
    weave = build_weave(x_fibers, y_fibers)

    // Step 4: Extract contour loops by traversing faces of the weave graph
    //   Uses a half-edge data structure
    //   Follow 'next' pointers around each face
    //   Faces on the boundary between inside/outside produce toolpath loops
    loops = weave.extract_loops()

    return loops
```

**Push-cutter algorithm**: Holds the cutter at constant Z, pushes it along X (or Y) direction, and returns intervals of valid cutter locations. Uses the same three tests (vertex, edge, facet) as drop-cutter but in a radial rather than axial projection.

**Weave data structure**: A planar graph (half-edge structure) built from the grid of X and Y fiber intervals. Contour loops are extracted by face traversal. The planar embedding is implicit in the graph structure -- each edge has a "next" pointer, and following these pointers around a face yields a contour loop.

**Adaptive waterline**: The AdaptiveWaterline adds fibers where geometric detail requires finer sampling. Uses a flatness predicate (angle between consecutive segments) to determine where additional fibers are needed.

### Optimal Mesh Slicing

For slicing a mesh at many Z levels simultaneously (Minetto et al.):

```
function slice_all_levels(triangles, z_levels):
    // Sort triangles by their minimum z-coordinate
    // Sort z_levels

    // Sweep plane from bottom to top
    active_set = {}  // triangles currently intersecting the sweep plane
    results = {}     // z_level -> [segments]

    for each event (vertex z-coordinate or z-level):
        if event is a vertex:
            update active_set (add/remove triangles)
        if event is a z-level:
            for each triangle in active_set:
                compute intersection segment
                add to results[z_level]

    // Complexity: O(n*log(k) + k + m)
    // where n = triangles, k = z-levels, m = total intersection segments
```

---

## 6. Scallop Height Calculation

### Flat Surface, Ball End Mill

The fundamental scallop height formula for a ball-end mill on a flat surface:

```
h = R - sqrt(R^2 - (ae/2)^2)
```

Where:
- h = scallop height (cusp height)
- R = ball end mill radius
- ae = stepover (radial step, path interval)

**Inverse -- stepover from scallop height:**
```
ae = 2 * sqrt(h * (2*R - h))
```

**Simplified approximation** (when ae << R):
```
h ≈ ae^2 / (8*R)
```
or equivalently:
```
ae ≈ sqrt(8*R*h)
```

### Curved Surfaces

On curved surfaces, the scallop height depends on the **effective radius of curvature** in the cross-feed direction:

#### Convex Surface (positive curvature, radius of curvature = rho)

The effective cutting radius is reduced because the ball rides on top of the convex surface:
```
R_eff = (R * rho) / (R + rho)
```

Scallop height increases:
```
h_convex = R_eff - sqrt(R_eff^2 - (ae/2)^2)
```

For the same scallop height, you need a **smaller** stepover on convex surfaces.

#### Concave Surface (negative curvature, radius of curvature = rho, rho > R)

The effective cutting radius increases because the ball nests into the concave surface:
```
R_eff = (R * rho) / (rho - R)     // for rho > R
```

Scallop height decreases:
```
h_concave = R_eff - sqrt(R_eff^2 - (ae/2)^2)
```

For the same scallop height, you can use a **larger** stepover on concave surfaces.

**Note**: If rho = R, the tool perfectly matches the surface curvature and the effective radius is infinite (zero scallop). If rho < R, the tool is larger than the concavity and will gouge.

#### Inclined Flat Surface

On an inclined flat surface with inclination angle phi from horizontal:
```
R_eff = R / cos(phi)   // in the cross-feed direction
```

But the actual scallop height depends on the orientation of the stepover relative to the slope.

### Tapered Ball End Mill

For a tapered ball-end mill (spherical tip radius r, cone half-angle alpha):
- At the spherical tip: scallop height uses radius r
- Where the cone contacts the surface: the effective radius is larger
- The effective radius at the cone portion: depends on the local surface angle relative to the cone angle
- Generally, tapered ball-end mills produce smaller scallops on steep surfaces because the cone section has a larger effective radius

### Theoretical Surface Roughness (Ra)

The theoretical arithmetic average roughness from scallop patterns:
```
Ra_theoretical ≈ h / 4
```

Where h is the scallop height. This is a lower bound; actual roughness is higher due to tool deflection, vibration, material properties, etc.

---

## 7. Stock Modeling / Material Removal Simulation

### Overview

Stock modeling tracks the shape of the workpiece as material is removed. This is essential for:
- **Adaptive clearing**: Knowing where material remains
- **Rest machining**: Finding uncut regions
- **Collision detection**: Preventing tool/holder collisions with stock
- **Simulation/verification**: Visual verification before cutting

### Approach 1: Heightmap / Z-Buffer

The simplest stock model for 3-axis machining:

```
struct Heightmap {
    grid: Vec<Vec<f64>>,  // 2D grid of Z heights
    x_min: f64, y_min: f64,
    resolution: f64,       // grid cell size
    nx: usize, ny: usize,  // grid dimensions
}
```

**Initialization**: Set all heights to the top of the stock block.

**Material removal** (for a single CL point):
```
function remove_material(heightmap, cl_point, cutter):
    // For each grid cell (i, j) within the cutter's XY footprint:
    for i in affected_x_range:
        for j in affected_y_range:
            grid_x = x_min + i * resolution
            grid_y = y_min + j * resolution
            r = sqrt((grid_x - cl.x)^2 + (grid_y - cl.y)^2)

            if r <= cutter.radius:
                cutter_z = cl.z + cutter.height(r)
                heightmap[i][j] = min(heightmap[i][j], cutter_z)
```

**For a toolpath segment** (moving from CL1 to CL2):
- Interpolate CL points along the segment
- Or compute the swept volume envelope analytically
- For 3-axis moves (only Z changes between CL1 and CL2 with same XY, or linear interpolation):
  The swept volume at each grid point is the minimum of the cutter height function along the linear path

**Advantages**:
- Very simple to implement
- O(1) per grid cell update
- Perfect for 3-axis (no undercuts possible)
- Can use GPU rasterization for speed

**Disadvantages**:
- Cannot represent undercuts (only one Z value per XY point)
- Resolution-limited
- Memory: O(nx * ny)

**Heightmap drop-cutter optimization**: Instead of testing against the full STL mesh, you can drop-cutter against the heightmap itself:
```
// For each grid cell under the tool footprint:
// Add the cutter heightmap to the surface heightmap
// Take the maximum Z value
// This is the "convolution" approach
tool_height = max over all (i,j) under tool footprint of:
    heightmap[i][j] + cutter.height(distance_from_center(i,j))
```

### Approach 2: Dexel Model

A dexel (depth element) extends the heightmap to support overhangs and internal cavities:

```
struct Dexel {
    segments: Vec<(f64, f64)>,  // list of (z_bottom, z_top) intervals
    // A dexel represents the solid material along a vertical ray
}

struct DexelGrid {
    dexels: Vec<Vec<Dexel>>,
    // Same XY grid as heightmap, but each cell has a stack of material intervals
}
```

**Material removal**: Subtract the cutter volume from each dexel:
```
function remove_from_dexel(dexel, cutter_z_bottom, cutter_z_top):
    // Remove the interval [cutter_z_bottom, cutter_z_top] from the dexel
    new_segments = []
    for each (bot, top) in dexel.segments:
        if top <= cutter_z_bottom or bot >= cutter_z_top:
            new_segments.push((bot, top))  // no overlap
        else:
            if bot < cutter_z_bottom:
                new_segments.push((bot, cutter_z_bottom))
            if top > cutter_z_top:
                new_segments.push((cutter_z_top, top))
    dexel.segments = new_segments
```

### Approach 3: Tri-Dexel Model

The tri-dexel model uses three orthogonal sets of dexels (along X, Y, and Z axes):

```
struct TriDexelModel {
    x_dexels: DexelGrid,  // rays along X axis
    y_dexels: DexelGrid,  // rays along Y axis
    z_dexels: DexelGrid,  // rays along Z axis (= standard heightmap)
}
```

**Advantage**: Much better surface quality at regions perpendicular to any single dexel direction. The single-direction dexel model has poor resolution for surfaces whose normals are nearly perpendicular to the dexel direction. The tri-dexel model covers all orientations.

**Used by**: Most commercial CAM software uses tri-dexel for machining simulation.

### Approach 4: Voxel Model

Discretize the entire workspace into a 3D grid of voxels:

```
struct VoxelGrid {
    data: Vec<Vec<Vec<bool>>>,  // true = material present
    resolution: f64,
    bounds: AABB,
}
```

**Advantages**: Can represent any geometry. Supports GPU acceleration.
**Disadvantages**: Very high memory consumption. O(n^3) for resolution n.
**Optimization**: Use octree compression for sparse voxel grids.

### GPU Acceleration

For all approaches, the material removal computation is highly parallelizable:
- Each grid cell / dexel / voxel can be updated independently
- GPU compute shaders can process the entire grid in parallel
- Z-buffer rendering hardware can be used directly for heightmap updates (render the cutter from above at each CL position)

---

## 8. Toolpath Linking and Optimization

### Overview

After generating the cutting moves (the actual machining paths), they must be linked together with non-cutting moves (rapids, retracts, plunges). Optimizing these transitions significantly reduces total cycle time.

### Retract Strategies

```
function link_paths(paths, safe_z, clearance_z, model):
    gcode = []
    for i in 0..paths.len():
        path = paths[i]

        if i > 0:
            // Retract from end of previous path
            prev_end = paths[i-1].last()
            next_start = path.first()

            // Strategy 1: Full retract to safe Z
            gcode.append(Rapid(z=safe_z))
            gcode.append(Rapid(x=next_start.x, y=next_start.y))
            gcode.append(Rapid(z=next_start.z + clearance))

            // Strategy 2: Minimum retract
            // Find the maximum surface height between prev_end and next_start
            max_z = max_surface_height_along_line(prev_end, next_start, model)
            retract_z = max_z + clearance
            gcode.append(Rapid(z=retract_z))
            gcode.append(Rapid(x=next_start.x, y=next_start.y))
            gcode.append(Rapid(z=next_start.z + clearance))

            // Strategy 3: Direct traverse (if clear)
            // Check if direct line is collision-free
            if is_clear(prev_end, next_start, model, clearance):
                gcode.append(Rapid(next_start + clearance_offset))

        // Plunge to start of cutting path
        gcode.append(Feed(z=path.first().z))

        // Cutting moves along path
        for point in path:
            gcode.append(Feed(point))

    return gcode
```

### Path Ordering (TSP Optimization)

The order in which toolpath segments are machined dramatically affects total rapid move distance. This is a variant of the **Traveling Salesman Problem (TSP)**.

```
function optimize_path_order(paths):
    // Build a distance graph
    // Nodes = toolpath segments (with start and end points)
    // Edge weights = rapid move distance between segment endpoints

    // Since paths can be traversed in either direction,
    // each path is represented by two "cities" (start, end)

    // Algorithms:
    // 1. Nearest neighbor heuristic: O(n^2), typically within 25% of optimal
    // 2. 2-opt improvement: iteratively swap pairs of edges
    // 3. LKH (Lin-Kernighan-Helsgott): state of the art TSP solver
    // 4. Genetic algorithm
    // 5. Ant colony optimization

    // For CNC, nearest-neighbor + 2-opt is usually sufficient
    order = nearest_neighbor_tsp(paths)
    order = two_opt_improve(order)
    return order
```

**Nearest neighbor TSP heuristic**:
```
function nearest_neighbor_tsp(paths):
    unvisited = set(paths)
    current_pos = (0, 0, safe_z)  // machine home
    order = []

    while unvisited is not empty:
        best_path = null
        best_dist = INFINITY
        best_reversed = false

        for path in unvisited:
            d_start = distance(current_pos, path.start)
            d_end = distance(current_pos, path.end)
            if d_start < best_dist:
                best_dist = d_start
                best_path = path
                best_reversed = false
            if d_end < best_dist:
                best_dist = d_end
                best_path = path
                best_reversed = true

        if best_reversed:
            best_path.reverse()
        order.append(best_path)
        current_pos = best_path.end
        unvisited.remove(best_path)

    return order
```

### Entry/Exit Strategies

- **Helical entry**: Spiral down into the material (for pocketing). Prevents full-width plunge.
  ```
  function helical_entry(center, start_z, end_z, radius, helix_pitch):
      points = []
      z = start_z
      angle = 0
      while z > end_z:
          x = center.x + radius * cos(angle)
          y = center.y + radius * sin(angle)
          points.append((x, y, z))
          angle += delta_angle
          z -= helix_pitch * delta_angle / (2 * PI)
      return points
  ```

- **Ramp entry**: Linear descent along the cutting direction at a shallow angle.

- **Plunge entry**: Straight vertical plunge (only for drill-capable tools or very soft materials like wood).

### Lead-In / Lead-Out Arcs

For contour operations, add tangential arcs at entry and exit:
```
function lead_in_arc(start_point, path_direction, lead_radius):
    // Compute a circular arc that is tangent to the toolpath at start_point
    // and approaches from outside the material
    center = start_point + lead_radius * perpendicular(path_direction)
    arc = circular_arc(center, lead_radius, from=approach_point, to=start_point)
    return arc
```

### Collision Avoidance During Rapids

```
function safe_rapid(from_point, to_point, stock_model, holder_geometry):
    // Check if direct rapid is safe
    // Must check not just the tool but also the holder/collet

    // Simple approach: always retract to safe Z
    // Better approach: compute minimum safe Z along the traverse line

    // Best approach: use the stock model (heightmap) to find clearance
    max_stock_z = 0
    for sample points along line from_point.xy to to_point.xy:
        stock_z = stock_model.height_at(sample.x, sample.y)
        max_stock_z = max(max_stock_z, stock_z)

    safe_traverse_z = max_stock_z + clearance_height
    // If this is below safe_z, use it for a shorter retract
    return min(safe_traverse_z, safe_z)
```

---

## 9. Open-Source Implementations Reference

### Core Libraries

| Library | Language | License | Key Algorithms |
|---------|----------|---------|----------------|
| **OpenCAMLib** | C++ (Python/JS bindings) | LGPL 2.1 | Drop-cutter, push-cutter, waterline, adaptive waterline for ball/flat/bull/cone cutters |
| **OpenVoronoi** | C++ (Python bindings) | LGPL 2.1 | 2D Voronoi diagram for point/line-segment sites, offset curves, medial axis |
| **libactp** | C++ | GPL | Freesteel adaptive clearing algorithm |
| **Clipper2** | C++/C#/Delphi | Boost | 2D polygon clipping (Vatti), offsetting, triangulation |
| **CavalierContours** | C++ | MIT | 2D polyline offsetting with native arc support |

### Complete CAM Applications

| Application | Language | License | Capabilities |
|-------------|----------|---------|--------------|
| **FreeCAD Path** | Python/C++ | LGPL | Full CAM workbench, adaptive clearing, surface ops |
| **BlenderCAM** | Python | GPL | 3-axis and 3+2, roughing, finishing, uses OpenCAMLib |
| **PyCAM** | Python | GPL v3 | 3-axis toolpath from STL/DXF/SVG, drop-cutter, push-cutter |
| **Meshmill** | Go/Electron | MIT | STL to G-code via heightmap approach |
| **Generic CAM** | C++ | GPL | STL/GTS to toolpath |
| **CAMotics** | C++ | GPL v2 | G-code simulation and verification |

### Key Academic References

1. **Held, M.** "On the Computational Geometry of Pocket Machining." Springer, 1991.
2. **Held, M., Lukacs, G., Andor, L.** "Pocket machining based on contour-parallel tool paths generated by means of proximity maps." Computer-Aided Design 26(3):189-203, 1994.
3. **Vatti, B.R.** "A generic solution to polygon clipping." Communications of the ACM 35(7):56-63, 1992.
4. **Chen, X., McMains, S.** "Polygon Offsetting by Computing Winding Numbers." ASME IDETC 2005.
5. **Stori, J.A., Wright, P.K.** "Constant engagement tool path generation for convex geometries." J. Manufacturing Systems 19(3), 2000.
6. **Minetto, R. et al.** "An optimal algorithm for 3D triangle mesh slicing." Computer-Aided Design, 2017.

---

## 10. Rust Ecosystem Libraries

Relevant Rust crates for building a CAM system:

### Geometry & Boolean Operations

| Crate | Purpose | Notes |
|-------|---------|-------|
| **cavalier_contours** | 2D polyline offsetting with arcs | Native Rust. Key for pocketing. No unsafe code. |
| **clipper2-rust** | Bindings to Clipper2 | Polygon clipping and offsetting |
| **geo** | 2D geometry primitives and algorithms | Boolean ops, distance, area, etc. |
| **geo-clipper** | Polygon boolean operations | Uses Clipper under the hood |
| **rust-geo-booleanop** | Martinez-Rueda polygon clipping | Pure Rust boolean operations |
| **csgrs** | Constructive Solid Geometry | 3D boolean operations, integrates with nalgebra/Parry |
| **rgeometry** | Computational geometry library | Points, polygons, lines, segments |

### 3D & Mesh Processing

| Crate | Purpose | Notes |
|-------|---------|-------|
| **nalgebra** | Linear algebra | Vectors, matrices, transforms |
| **parry3d** | 3D collision detection | Triangle meshes, spatial queries, BVH |
| **stl_io** | STL file reading/writing | Parse binary and ASCII STL |
| **meshopt** | Mesh optimization | Mesh simplification, spatial cache |
| **kiss3d** | 3D visualization | Quick debugging/visualization |

### Spatial Indexing

| Crate | Purpose | Notes |
|-------|---------|-------|
| **kiddo** | KD-tree | Fast nearest-neighbor queries |
| **rstar** | R-tree | Spatial indexing for bounding boxes |
| **bvh** | Bounding Volume Hierarchy | Ray-triangle intersection acceleration |
| **rayon** | Parallel iteration | Drop-in parallelism for batch operations |

### G-Code

| Crate | Purpose | Notes |
|-------|---------|-------|
| **gcode** | G-code parsing | Parse G-code files |
| **gcode-rs** | G-code generation | Generate G-code output |

---

## Appendix: Implementation Priority for a Wood Router CAM

Suggested implementation order for a Rust-based wood routing CAM:

### Phase 1: Foundation
1. STL loading and triangle mesh data structure
2. KD-tree or BVH for spatial indexing
3. Basic drop-cutter for ball-end and flat-end mills (vertex, facet, edge tests)
4. Heightmap stock model
5. Raster/zigzag 3D finishing toolpath
6. G-code output

### Phase 2: 2.5D Operations
7. 2D polygon offset (via cavalier_contours or clipper2)
8. Contour-parallel pocketing with island support
9. Profile/contour cutting with lead-in/lead-out
10. Z-level roughing with offset pocketing at each level

### Phase 3: Advanced 3D
11. Bull-nose and cone cutter drop-cutter tests
12. Push-cutter and waterline toolpath generation
13. Adaptive waterline
14. Pencil finishing

### Phase 4: Optimization
15. Toolpath linking and ordering (TSP)
16. Adaptive clearing (constant engagement)
17. Constant scallop-height finishing
18. Rest machining
19. Stock model-based collision avoidance for rapids

### Phase 5: Simulation
20. Material removal simulation (heightmap or tri-dexel)
21. Visual toolpath verification
22. Machining time estimation
