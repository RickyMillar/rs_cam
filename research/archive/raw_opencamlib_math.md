# OpenCAMLib Complete Mathematical Reference

Extracted from the actual C++ source at https://github.com/aewallin/opencamlib
and Anders Wallin's blog posts at anderswallin.net.

---

## 1. Architecture Overview

OpenCAMLib is organized into these key modules:

- **`src/cutters/`** - Cutter geometry classes (MillingCutter base + subclasses)
- **`src/dropcutter/`** - Drop-cutter algorithms (PointDropCutter, BatchDropCutter, PathDropCutter, AdaptivePathDropCutter)
- **`src/algo/`** - Higher-level algorithms (Waterline, AdaptiveWaterline, BatchPushCutter, Weave, Fiber, Interval)
- **`src/geo/`** - Geometric primitives (Point, Triangle, STLSurf, CLPoint, CCPoint, Line, Arc, Path, Bbox)
- **`src/common/`** - Utilities (KDTree, Numeric, Brent solver, LineCLFilter)

### Design Pattern

The `MillingCutter` base class uses the **Template Method** pattern. The base class implements the general algorithm (vertex/facet/edge testing), and each subclass provides only:
- `height(r)` - profile height at radius r
- `width(h)` - profile width at height h
- `singleEdgeDropCanonical()` - edge-drop in canonical coordinates
- `generalEdgePush()` - edge-push for push-cutter
- Constructor parameters that set `center_height`, `normal_length`, `xy_normal_length`

---

## 2. Base Class: MillingCutter

### Member Variables

```
double diameter;          // cutter diameter
double radius;            // = diameter/2
double length;            // cutter length (shaft)
double center_height;     // height of cutter center above tip along Z
double normal_length;     // distance from CC to CL along surface normal direction
double xy_normal_length;  // distance from CC to CL in XY plane along surface normal
```

### The Facet Contact Geometry

For a triangle facet with upward normal **n** = (nx, ny, nz) and plane equation:
```
a*x + b*y + c*z + d = 0    where d = -n . p0
```

The CC (cutter-contact) point is offset from the CL (cutter-location) point by:
```
radiusvector = xy_normal_length * xyNormal + normal_length * n_normalized
```

where `xyNormal = normalize_xy(nx, ny, 0)` is the XY-projected, XY-normalized normal.

The CC-point lies in the facet plane:
```
cc = cl - radiusvector       (XY coordinates only)
cc.z = (1/nz) * (-d - nx*cc.x - ny*cc.y)
```

The cutter tip Z position:
```
tip_z = cc.z + radiusvector.z - center_height
```

### The Dual Geometry for Edge Tests

Instead of testing the original cutter against an infinitely thin edge, we test a virtual CylCutter with radius VR against an infinite ER-radius cylinder around the edge. This reduces to 2D.

| Cutter Type | VR (virtual radius) | ER (edge cylinder radius) | OR (offset) |
|---|---|---|---|
| CylCutter(R) | R | 0 | R |
| BallCutter(R) | 0 | R | 0 |
| BullCutter(R1,R2) | R1-R2 | R2 | R1-R2 |

The edge-drop process:
1. Translate so CL = (0,0) in XY
2. Rotate edge so it lies along the X-axis
3. Call `singleEdgeDropCanonical(u1, u2)` - subclass handles this
4. Rotate/translate back
5. Update CL if CC lies within edge

### The Push-Cutter Edge Cases

Three cases are tested in order:
1. **Horizontal edge** (`horizEdgePush`): The cutter acts as a cylinder with effective radius = `width(h)` where h = edge_z - fiber_z
2. **Shaft contact** (`shaftEdgePush`): Contact with the cylindrical shaft above the shaped lower part
3. **General edge** (`generalEdgePush`): Subclass-specific implementation

### The Facet Push Geometry

For push-cutter, we solve a 2x2 linear system. Given a fiber and triangle facet:

For X-fiber:
```
[ (v1y-v0y)  (v2y-v0y) ] [ u ] = [ -v0y - r2*ny - r1*xy_n.y + p1y ]
[ (v1z-v0z)  (v2z-v0z) ] [ v ] = [ -v0z - r2*nz + p1z + r2        ]
```

For Y-fiber:
```
[ (v1x-v0x)  (v2x-v0x) ] [ u ] = [ -v0x - r2*nx - r1*xy_n.x + p1x ]
[ (v1z-v0z)  (v2z-v0z) ] [ v ] = [ -v0z - r2*nz + p1z + r2        ]
```

where r2 = normal_length, r1 = xy_normal_length.

---

## 3. CylCutter (Flat End Mill)

### Constructor
```cpp
CylCutter(double d, double l)
  diameter = d
  radius = d/2
  length = l
  xy_normal_length = radius    // facet CC is one radius away in XY
  normal_length = 0.0          // no offset along surface normal
  center_height = 0.0          // center is at the tip
```

### Profile Functions
```cpp
height(r) = (r <= radius) ? 0.0 : -1.0    // flat bottom
width(h)  = radius                          // constant width at all heights
```

### offsetCutter
```cpp
offsetCutter(d) -> BullCutter(diameter+2*d, d, length+d)
```
Offsetting a flat endmill produces a bull-nose cutter with corner radius = d.

### Edge Drop (singleEdgeDropCanonical)
In canonical coordinates (CL at origin, edge along X-axis at distance d = u1.y):
```
s = sqrt(radius^2 - u1.y^2)     // half-width of cutter at distance d
cc1 = ( s, u1.y, projected_z)   // two candidate CC points
cc2 = (-s, u1.y, projected_z)
cl_z = max(cc1.z, cc2.z)        // pick the higher one
```

The CC point is simply the intersection of the flat bottom circle with the vertical plane containing the edge.

### Vertex Push (special override)
CylCutter adds z-slice intersection points: where the triangle edges cross the z-height of the fiber, creating additional virtual vertices at the exact cutter height.

---

## 4. BallCutter (Ball Nose End Mill)

### Constructor
```cpp
BallCutter(double d, double l)
  diameter = d
  radius = d/2
  length = l
  normal_length = radius       // CC to center is one radius along normal
  xy_normal_length = 0.0       // no XY offset (sphere is symmetric)
  center_height = radius       // center of sphere is radius above tip
```

### Profile Functions
```cpp
height(r) = radius - sqrt(radius^2 - r^2)

width(h) = (h >= radius) ? radius : sqrt(radius^2 - (radius-h)^2)
         = sqrt(2*radius*h - h^2)    // simplified
```

### offsetCutter
```cpp
offsetCutter(d) -> BallCutter(diameter+2*d, length+d)
```
Offset of a ball is a larger ball.

### Edge Drop (singleEdgeDropCanonical)
The plane at distance d from the sphere center slices a circle of radius:
```
s = sqrt(radius^2 - d^2)
```

The normal to the edge line (dz, -du) determines where the sphere contacts:
```
normal = normalize(u2.z - u1.z, -(u2.x - u1.x), 0)
cc = (-s * normal.x, d, projected_z)
cl_z = cc.z + s * normal.y - radius
```

This finds the point on the circular cross-section where the slope matches the edge slope.

### Edge Push (generalEdgePush)
Uses **ray-cylinder intersection**. The fiber (raised up by radius) is treated as a ray, and a cylinder of radius=radius is placed around the edge.

The ray-cylinder intersection is a quadratic equation:
```
P(t) = O + t*V                          (ray from fiber)
Cylinder [A, B, r] = edge p1->p2        (radius = ball radius)

X = (O - A) x (B - A)
Y = V x (B - A)
d = r^2 * (B - A)^2

a*t^2 + b*t + c = 0  where:
  a = Y . Y
  b = 2 * (X . Y)
  c = X . X - d

discriminant = b^2 - 4ac
t = (-b +/- sqrt(discriminant)) / (2a)
```

Validation: the CC point must be on the lower hemisphere (ball center Z >= CC Z).

---

## 5. BullCutter (Bull Nose / Toroidal End Mill)

### Constructor
```cpp
BullCutter(double d, double r, double l)
  diameter = d               // total diameter
  radius = d/2               // total radius
  radius1 = d/2 - r          // cylindrical part radius (center of torus tube ring)
  radius2 = r                // corner radius (tube radius of torus)
  length = l
  xy_normal_length = radius1  // CC to CL in XY is the torus ring radius
  normal_length = radius2     // CC to CL along normal is the tube radius
  center_height = radius2     // center of torus tube is radius2 above tip
```

### Profile Functions
```cpp
height(r):
  if r <= radius1:  return 0.0                                    // flat bottom
  if r <= radius:   return radius2 - sqrt(radius2^2 - (r-radius1)^2)  // torus
  else: error

width(h):
  if h >= radius2:  return radius
  else:             return radius1 + sqrt(radius2^2 - (radius2-h)^2)
```

### offsetCutter
```cpp
offsetCutter(d) -> BullCutter(diameter+2*d, radius2+d, length+d)
```
Offset of a bull is a bull with larger corner radius.

### Edge Drop - The Offset Ellipse Method

This is the most mathematically complex part of OpenCAMLib. When a radius2-cylinder around an edge is sliced by an XY plane, it produces an **ellipse**:

```
theta = atan((u2.z - u1.z) / (u2.x - u1.x))   // edge slope in XZ plane
b_axis = radius2                                 // short axis
a_axis = |radius2 / sin(theta)|                  // long axis
```

The CL point must lie on an **offset ellipse** (the ellipse expanded by radius1 along its normals).

For an ellipse parameterized by angle (s, t) where s^2 + t^2 = 1:
```
ePoint(s,t) = center + a*s*x_dir + b*t*y_dir           // point on ellipse
normal(s,t) = normalize(b*s*x_dir + a*t*y_dir)          // outward normal
oePoint(s,t) = ePoint(s,t) + offset * normal(s,t)       // offset ellipse point
```

The offset ellipse has no closed-form representation, so **Brent's root-finding method** is used to find the (s,t) where the offset-ellipse point lands at the CL position (0,0).

The error function for the standard (non-aligned) case:
```
error(diangle) = oePoint(diangle).y   // seek y=0 for the offset point
```

The solver brackets the root in diangle range [0, 3], then uses Brent's method with tolerance 1e-10.

Two symmetric solutions exist (contact on upper/lower torus). The one with the higher Z center is chosen.

### Edge Push - The Aligned Ellipse Method

For push-cutter, the edge is not aligned with the X-axis, so an `AlignedEllipse` is used:

```
tplane = (fiber.z + radius2 - p1.z) / (p2.z - p1.z)   // where edge crosses fiber+r2 plane
ell_center = p1 + tplane * (p2 - p1)                    // ellipse center on edge
theta = atan((p2.z - p1.z) / |p2-p1|_xy)               // edge slope
major_length = |radius2 / sin(theta)|
minor_length = radius2
major_dir = normalize_xy(p2 - p1)                        // along edge in XY
minor_dir = major_dir.xyPerp()                            // perpendicular
```

The aligned solver finds the ellipse position where the offset-ellipse point lies on the fiber. Error function:
```
error(diangle) = (target - oePoint(diangle)) . error_dir
```
where `error_dir` is perpendicular to the fiber direction.

---

## 6. ConeCutter (V-bit / Engraving)

### Constructor
```cpp
ConeCutter(double d, double a, double l)
  diameter = d
  radius = d/2
  angle = a                          // half-angle in radians
  length = radius/tan(angle) + l     // total length including cone
  center_height = radius/tan(angle)  // height of shaft above tip
  xy_normal_length = radius
  normal_length = 0.0
```

### Profile Functions
```cpp
height(r) = r / tan(angle)         // linear slope from tip
width(h) = (h < center_height) ? h * tan(angle) : radius   // grows linearly, then constant
```

### offsetCutter
```cpp
offsetCutter(d) -> BallConeCutter(2*d, diameter+2*d, angle)
```
Offset of a cone is a ball-cone composite (the sharp tip becomes a ball).

### Facet Drop (special override)

The cone has TWO possible facet contact points:
1. **Tip contact** (`FACET_TIP`): CC at (cl.x, cl.y) projected onto the plane
2. **Cylindrical edge contact** (`FACET_CYL`): CC at cl - radius*xy_normal projected onto the plane, then tip_z = cc.z - length

Both are tested and the highest valid one is used.

### Edge Drop - Hyperbola Intersection

When a vertical plane slices a cone, the intersection curve is a **hyperbola**.

```
d = u1.y                                          // distance from CL to edge
xu = sqrt(radius^2 - d^2)                         // outermost cutter point at this distance
m = (u2.z - u1.z) / (u2.x - u1.x)               // edge slope
mu = (center_height/radius) * xu / sqrt(xu^2 + d^2)   // max slope of hyperbola at xu
```

Two cases:
1. **Hyperbola case** (|m| <= |mu|): Contact on the conical surface
   ```
   ccu = sign(m) * sqrt(R^2 * m^2 * d^2 / (L^2 - R^2 * m^2))
   cl_z = cc.z - center_height + (radius - sqrt(ccu^2 + d^2)) / tan(angle)
   ```

2. **Circular rim case** (|m| > |mu|): Contact at the cone/shaft boundary
   ```
   ccu = sign(m) * xu
   cl_z = cc.z - center_height
   ```

### Edge Push - Circle/Cone Intersection

The push-cutter slides an "inverted tool object" (ITO) cone along the edge. Where it pierces the fiber's Z-plane creates a 2D shape:

- If the edge is steep: the shape is a **circle** (the base circle of the cone)
- If shallow: a **circle plus cone** (ice-cream cone shape)

For the circle case, uses the standard **circle-line intersection** formula:
```
det = (f1.x-center.x)*(f2.y-center.y) - (f2.x-center.x)*(f1.y-center.y)
discr = radius^2 * dr^2 - det^2
x = (det*dy +/- sign(dy)*dx*sqrt(discr)) / dr^2
y = (-det*dx +/- |dy|*sqrt(discr)) / dr^2
```

For the cone part, finds tangent lines from the tip to the base circle using **circle-circle intersection** geometry (law of cosines), then tests these tangent lines against the fiber.

---

## 7. CompositeCutter Architecture

### Data Structures
```cpp
vector<MillingCutter*> cutter;    // sub-cutters
vector<double> radiusvec;         // radial boundaries: cutter[n] valid from r=radiusvec[n-1] to r=radiusvec[n]
vector<double> heightvec;         // height boundaries: same logic
vector<double> zoffset;           // axial offset for each sub-cutter
```

### Delegation Logic

```cpp
height(r):
  idx = radius_to_index(r)          // find which sub-cutter covers radius r
  return cutter[idx]->height(r) + zoffset[idx]

width(h):
  idx = height_to_index(h)          // find which sub-cutter covers height h
  return cutter[idx]->width(h - zoffset[idx])
```

### Drop-Cutter Override

CompositeCutter overrides `facetDrop` and `edgeDrop`. For each sub-cutter:
1. Create temporary CL offset by zoffset[n]
2. Run the sub-cutter's facetDrop/edgeDrop
3. Validate that the CC point falls within this sub-cutter's radial range (`ccValidRadius`)
4. If valid and higher than current CL, update CL (subtract zoffset back)

### Push-Cutter Override

For each sub-cutter:
1. Create a copy of the fiber, offset by zoffset[n]
2. Run the sub-cutter's vertexPush/facetPush/edgePush
3. Validate CC height falls in this sub-cutter's height range (`ccValidHeight`)
4. Collect all valid contacts, then merge into the master interval

### Composite Cutter Types

#### CylConeCutter(diam1, diam2, angle)
Flat bottom + conical taper + cylindrical shaft.
```
cone_offset = -(diam1/2) / tan(angle)
cyl_height = 0.0
cone_height = (diam2/2)/tan(angle) + cone_offset
```

#### BallConeCutter(diam1, diam2, angle)
Spherical tip + conical taper + shaft. The sphere and cone meet tangentially.
```
rcontact = radius1 * cos(angle)                              // contact ring radius
height1 = radius1 - sqrt(radius1^2 - rcontact^2)            // height at contact
cone_offset = -(rcontact/tan(angle) - height1)               // Z offset for cone
height2 = radius2/tan(angle) + cone_offset                   // cone outer height
```
The contact point radius where sphere tangent = cone tangent: `rcontact = R_ball * cos(half_angle)`

#### BullConeCutter(diam1, radius1, diam2, angle)
Toroidal tip + conical taper + shaft. The torus and cone meet tangentially.
```
h1 = radius1 * sin(angle)                                    // drop below torus ring center
rad = sqrt(radius1^2 - h1^2)                                // horizontal distance from ring
rcontact = (diam1/2) - radius1 + rad                         // contact ring radius
cone_offset = -(rcontact/tan(angle) - (radius1 - h1))       // Z offset for cone
height1 = radius1 - h1                                       // torus region height
height2 = (diam2/2)/tan(angle) + cone_offset                 // cone outer height
```

#### ConeConeCutter(diam1, angle1, diam2, angle2)
Inner steep cone + outer shallow cone + shaft. Assumes angle2 < angle1 and diam2 > diam1.
```
height1 = (diam1/2) / tan(angle1)                           // inner cone height
tmp = (diam1/2) / tan(angle2)                               // where outer cone would start
cone_offset = -(tmp - height1)                               // Z offset
height2 = (diam2/2)/tan(angle2) + cone_offset               // outer cone height
```

---

## 8. The Drop-Cutter Algorithm

### PointDropCutter

The simplest unit of work:

1. **KD-tree search**: `root->search_cutter_overlap(cutter, &cl)` returns triangles whose bounding boxes overlap the cutter's bounding box at the CL position
2. **Overlap test**: `cutter->overlaps(cl, triangle)` - fast XY bounding box check
3. **Below test**: `cl.below(triangle)` - is the CL point currently below the triangle?
4. **Drop**: `cutter->dropCutter(cl, triangle)` which calls:
   - `facetDrop` first. If facet contact found, skip vertex/edge (optimization: facet contact means no edge/vertex contact possible)
   - If no facet: `vertexDrop` then `edgeDrop`

### dropCutter() Logic (Template Method)

```
if cl is below triangle:
    if facetDrop(cl, t):       // try facet first
        return true            // facet contact precludes edge/vertex
    else:
        vertexDrop(cl, t)      // try all 3 vertices
        if cl still below t:
            edgeDrop(cl, t)    // try all 3 edges
```

### BatchDropCutter

Operates on a vector of CLPoints against the STL surface.

**Version 5** (default, most optimized):
```
#pragma omp parallel for schedule(dynamic)
for each CLPoint:
    tris = kdtree.search_cutter_overlap(cutter, cl)
    for each found triangle:
        if cutter.overlaps(cl, tri) && cl.below(tri):
            cutter.dropCutter(cl, tri)
```

Uses OpenMP with dynamic scheduling for load balancing.

**Version 4** splits vertex/facet/edge into separate passes over triangles (allows early termination optimization).

### PathDropCutter

Samples a Path (sequence of Line/Arc Spans) uniformly:
```
for each Span in path:
    num_steps = span.length2d() / sampling + 1
    for i = 0 to num_steps:
        fraction = i / num_steps
        point = span.getPoint(fraction)
        cl = CLPoint(point.x, point.y, minimumZ)
        -> delegate to BatchDropCutter
```

### AdaptivePathDropCutter

Recursive subdivision based on flatness:
```
adaptive_sample(span, start_t, stop_t, start_cl, stop_cl):
    mid_t = (start_t + stop_t) / 2
    mid_cl = dropcutter at span.getPoint(mid_t)

    if step > sampling:           // above minimum resolution
        subdivide both halves
    elif !flat(start, mid, stop): // not flat enough
        if step > min_sampling:   // haven't reached min resolution
            subdivide both halves
    else:
        accept mid_cl as final point
```

Flatness predicate:
```
flat(start, mid, stop):
    v1 = normalize(mid - start)
    v2 = normalize(stop - mid)
    return v1.dot(v2) > cosLimit    // default cosLimit = 0.999
```

---

## 9. The Push-Cutter Algorithm

### Concept

Push-cutter holds the cutter at a constant Z and pushes it horizontally along a **Fiber** (a line segment in the XY plane at constant Z). For each triangle, it computes an **Interval** [lower, upper] along the fiber where the cutter would gouge/violate the triangle.

```
pushCutter(fiber, interval, triangle):
    vertexPush(fiber, interval, triangle)   // test 3 vertices
    facetPush(fiber, interval, triangle)    // test facet plane
    edgePush(fiber, interval, triangle)     // test 3 edges
```

### Vertex Push

For each vertex p within the cutter's height range:
```
if p.z >= fiber.z && p.z <= fiber.z + cutter.length:
    h = p.z - fiber.z
    cwidth = cutter.width(h)           // effective cutter radius at this height
    q = XY distance from fiber to p
    if q <= cwidth:
        ofs = sqrt(cwidth^2 - q^2)    // distance along fiber
        interval expands by [pq - ofs, pq + ofs]
```

### Facet Push

Solves the 2x2 system described in section 2 to find the CC point on the facet and the corresponding position along the fiber.

### BatchPushCutter

Pushes the cutter along many fibers, using KD-tree search:

```
#pragma omp parallel for schedule(dynamic)
for each fiber:
    cl = (fiber center for KD search)
    tris = kdtree.search(cutter, cl)
    for each triangle:
        interval = new Interval()
        cutter.pushCutter(fiber, interval, triangle)
        fiber.addInterval(interval)     // merge into fiber's interval list
```

KD-tree dimensions depend on fiber direction:
- X-fibers: search in YZ plane
- Y-fibers: search in XZ plane

---

## 10. The Fiber / Interval Data Structures

### Fiber
A line segment from p1 to p2 at constant Z, parameterized by t in [0,1]:
```
point(t) = p1 + t*(p2 - p1)
tval(p) = (p - p1).dot(p2-p1) / (p2-p1).dot(p2-p1)
```

Fibers accumulate a vector of Intervals representing gouging regions.

### Interval
A parameter range [lower, upper] along a fiber, with associated CC points:
```
lower: double           // lower t-value
upper: double           // upper t-value
lower_cc: CCPoint       // CC point at lower bound
upper_cc: CCPoint       // CC point at upper bound
```

Interval merging handles overlaps: when adding interval i to fiber:
- If fiber contains i: skip
- If i is completely missing: append
- If partial overlap: merge all overlapping intervals into one

---

## 11. The Waterline Algorithm

### Overview

Waterline generates contour toolpaths at constant Z heights by:
1. Creating a grid of X-fibers and Y-fibers at the target Z height
2. Running BatchPushCutter on each set of fibers
3. Building a Weave graph from the fiber intervals
4. Extracting loops from the Weave via face traversal

### Fiber Generation
```
xfibers: for each y in [miny..maxy step sampling]:
    fiber from (minx, y, zh) to (maxx, y, zh)

yfibers: for each x in [minx..maxx step sampling]:
    fiber from (x, miny, zh) to (x, maxy, zh)
```

Where bounds are extended by 2*cutter_radius beyond the STL bounding box.

### Weave Graph Construction (SimpleWeave)

The Weave is a planar graph using a **half-edge data structure** (boost adjacency_list with next/prev pointers).

Vertex types:
- **CL**: Cutter-location points at interval endpoints on fibers
- **INT**: Internal intersection points where X and Y intervals cross

For each X-interval on each X-fiber:
1. Add CL vertices at the lower and upper endpoints
2. Connect them with edges (forward and reverse)
3. For each Y-fiber that crosses this X-interval:
   - For each Y-interval on that Y-fiber:
     - If they actually intersect (XY check):
       - Add Y-interval CL endpoints if not already in weave
       - Create an INT vertex at the intersection point
       - Rewire edges: the new INT vertex connects to its x-lower, x-upper, y-lower, y-upper neighbors
       - Set next/prev pointers to maintain planar face structure

### Face Traversal

Once the weave is built, contour loops are extracted by following the `next` edge pointers:

```
while unprocessed CL vertices remain:
    start at any unprocessed CL vertex
    loop:
        add current CL vertex to loop
        mark as processed
        follow the single out-edge
        follow next pointers until we reach another CL vertex
    until we return to start vertex
    output the loop
```

CL vertices have exactly one out-edge (they're at interval endpoints), so the traversal is unambiguous.

### AdaptiveWaterline

Uses recursive subdivision instead of uniform fiber spacing:

```
adaptive_sample(span, start_t, stop_t, start_fiber, stop_fiber):
    mid_t = (start_t + stop_t) / 2
    mid_fiber = push_cutter at mid position

    if step > sampling:
        subdivide
    elif !flat(start_fiber, mid_fiber, stop_fiber):
        if step > min_sampling:
            subdivide
    else:
        accept
```

Flatness check for fibers:
- All three fibers must have the same number of intervals
- For each interval, the upper and lower CL-points must be "flat" (dot product > cosLimit)

Uses `FiberPushCutter` instead of `BatchPushCutter` for single-fiber evaluation, and OpenMP task parallelism to process X and Y fibers simultaneously.

---

## 12. The KD-Tree

A spatial search structure for quickly finding triangles that overlap with a cutter.

### Construction
```
build_node(triangles, depth, parent):
    if triangles.size <= bucketSize or spread is zero:
        return leaf node containing all triangles

    spread = calc_spread(triangles)    // find dimension with largest spread
    cutvalue = spread.start + spread.value / 2    // cut in the middle

    split triangles into hilist and lolist based on cutvalue

    node.hi = build_node(hilist, depth+1, node)
    node.lo = build_node(lolist, depth+1, node)
    return node
```

### Search Dimensions

The KD-tree uses 6 bounding-box dimensions: [xmin, xmax, ymin, ymax, zmin, zmax]

- **Drop-cutter** (XY search): dimensions 0,1,2,3 (x and y)
- **X-fiber push** (YZ search): dimensions 2,3,4,5 (y and z)
- **Y-fiber push** (XZ search): dimensions 0,1,4,5 (x and z)

### Cutter Overlap Search
```
search_cutter_overlap(cutter, cl):
    r = cutter.radius
    bbox = Bbox(cl.x-r, cl.x+r, cl.y-r, cl.y+r, cl.z, cl.z+cutter.length)
    return search(bbox)
```

---

## 13. The Ellipse Solver (for BullCutter)

### EllipsePosition

Parameterizes a point on the unit circle using "diamond angle" (diangle) in [0,4] which avoids trigonometric functions:

```
diangle d in [0,4]:
  p.x = (d < 2) ? 1-d : d-3
  p.y = (d < 3) ? ((d > 1) ? 2-d : d) : d-4
  normalize p to unit circle
  s = p.x    // cos-like component
  t = p.y    // sin-like component
```

Invariant: s^2 + t^2 = 1

### Ellipse Geometry

For the standard (axis-aligned) Ellipse:
```
ePoint(s,t) = center + (a*s, b*t, 0)        // point on ellipse
normal(s,t) = normalize(b*s, a*t, 0)         // outward normal
oePoint(s,t) = ePoint + offset * normal      // offset ellipse point
```

For the AlignedEllipse (arbitrary orientation):
```
ePoint(s,t) = center + a*s*major_dir + b*t*minor_dir
normal(s,t) = normalize(s*b*major_dir + t*a*minor_dir)
oePoint(s,t) = ePoint + offset * normal
```

### Brent Solver (Drop-Cutter)

For drop-cutter, seeks diangle where `oePoint.y = 0`:
- Bracket in [0, 3] (these always bracket the root)
- Uses Brent's root-finding with tolerance 1e-10
- Finds first solution, then tries sign-flipping (s,-t), (-s,t), (-s,-t) for second solution
- Chooses the solution with higher Z center

### Aligned Solver (Push-Cutter)

For push-cutter, seeks diangle where oePoint lies on the fiber:
- Finds positions where ellipse tangent is parallel to fiber:
  ```
  for X-fiber: t1 = sqrt(b^2*minor_dir.y^2 / (a^2*major_dir.y^2 + b^2*minor_dir.y^2))
  for Y-fiber: t1 = sqrt(b^2*minor_dir.x^2 / (a^2*major_dir.x^2 + b^2*minor_dir.x^2))
  ```
- Tests all 4 sign combinations (+/-s, +/-t) to bracket the root
- Uses Brent's method twice for two solutions

---

## 14. CCPoint Types

The complete enumeration of contact types:

```
NONE                    // no contact
VERTEX                  // contact with triangle vertex
VERTEX_CYL              // contact with z-slice point (CylCutter special)
EDGE                    // generic edge contact
EDGE_HORIZ              // horizontal edge contact
EDGE_SHAFT              // contact with cylindrical shaft
EDGE_HORIZ_CYL          // horizontal edge, cylindrical part
EDGE_HORIZ_TOR          // horizontal edge, torus part
EDGE_BALL               // ball cutter edge contact
EDGE_POS, EDGE_NEG      // ellipse solutions (+/-)
EDGE_CYL                // cylindrical edge contact
EDGE_CONE               // cone surface edge contact
EDGE_CONE_BASE          // cone/shaft boundary edge contact
FACET                   // generic facet contact
FACET_TIP               // cone tip facet contact
FACET_CYL               // cone cylindrical rim facet contact
ERROR                   // error state
```

---

## 15. Other Algorithms

### LineCLFilter

Reduces CL-point count by detecting co-linear points:
```
for points p0, p1, p2:
    project p1 onto line p0-p2
    if distance < tolerance:
        skip p1 (it's co-linear)
    else:
        keep p1 as a breakpoint
```

### TSPSolver

Uses Boost's `metric_tsp_approx` for approximate Euclidean TSP on 2D points. This is for optimizing rapid/linking moves between toolpath segments.

### ZigZag

Simple zigzag pattern generator:
```
perp = dir.xyPerp().normalize()
max_d = (bbox.max - origin).dot(perp)
min_d = (bbox.min - origin).dot(perp)
for d = min_d to max_d step stepOver:
    output point = origin + d * perp
```

---

## 16. OpenVoronoi (Separate Project)

OpenVoronoi by Anders Wallin (https://github.com/aewallin/openvoronoi) computes 2D Voronoi diagrams for point and line-segment sites using an incremental topology-oriented algorithm.

Key applications:
- **2D offset generation** for pocket milling
- **Medial axis** extraction (filtering the Voronoi diagram)
- **V-carving** toolpaths: position V-cutter along medial axis at depth corresponding to clearance-disk radius

---

## 17. Summary Table: All Cutter Parameters

| Cutter | Parameters | height(r) | width(h) | center_height | normal_length | xy_normal_length |
|--------|-----------|-----------|----------|---------------|---------------|------------------|
| CylCutter | d, l | 0 (if r<=R) | R | 0 | 0 | R |
| BallCutter | d, l | R - sqrt(R^2-r^2) | sqrt(R^2-(R-h)^2) | R | R | 0 |
| BullCutter | d, r, l | 0 (r<=R1), R2-sqrt(R2^2-(r-R1)^2) | R (h>=R2), R1+sqrt(R2^2-(R2-h)^2) | R2 | R2 | R1 |
| ConeCutter | d, angle, l | r/tan(a) | h*tan(a) (h<CH), R | R/tan(a) | 0 | R |

Where R = d/2, R1 = d/2 - r (BullCutter), R2 = r (BullCutter), CH = center_height.

---

## Sources

- [OpenCAMLib GitHub Repository](https://github.com/aewallin/opencamlib)
- [Anders Wallin - CAM page](https://www.anderswallin.net/cam/)
- [Drop-Cutter toroid edge test blog post](https://www.anderswallin.net/2014/02/drop-cutter-toroid-edge-test/)
- [Drop-Cutter blog category (18 posts)](https://www.anderswallin.net/category/cnc/cam/drop-cutter-cam/)
- [Waterline blog category (14 posts)](https://www.anderswallin.net/category/cnc/cam/waterline-cam/)
- [OpenVoronoi GitHub](https://github.com/aewallin/openvoronoi)
- [OpenCAMLib ReadTheDocs](https://opencamlib.readthedocs.io/en/stable/cutters.html)
- [OpenCAMLib on PyPI](https://pypi.org/project/opencamlib/)
- [Offset Ellipse blog post](https://www.anderswallin.net/2010/03/offset-ellipse-2/)
- [FreeCAD V-carving with OpenVoronoi](https://www.anderswallin.net/2019/05/freecad-v-carving-with-openvoronoi/)
