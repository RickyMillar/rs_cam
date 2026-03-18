# Tool Geometry Mathematical Reference

Complete mathematical definitions for all CNC tool types, extracted primarily from OpenCAMLib source code.

---

## Generic Tool Model

Any axially-symmetric tool is a **surface of revolution** defined by a 2D profile:

- `height(r)` -- profile height at radial distance r from tool axis
- `width(h)` -- profile radius at height h above tool tip
- These are inverse functions of each other

### Key Parameters (per OpenCAMLib)

| Parameter | Meaning |
|-----------|---------|
| `diameter` | Total tool diameter |
| `radius` | = diameter / 2 |
| `center_height` | Height of cutter's "center" above tip |
| `normal_length` | Offset from CC to CL along surface normal |
| `xy_normal_length` | Offset from CC to CL in XY along projected normal |

### Facet Contact Formula (Universal)

```
radiusvector = xy_normal_length * xyNormal + normal_length * surfaceNormal
CC = CL - radiusvector
```

This single formula works for ALL tool types with the right parameter values.

---

## 1. Flat End Mill (CylCutter)

### Definition
Cylinder with flat bottom. Parameters: diameter d, length l.

### Profile
```
height(r) = 0.0          (r <= R)
width(h)  = R             (all h)
```

### Parameters
```
center_height    = 0
normal_length    = 0
xy_normal_length = R
```

### Facet Contact
```
radiusvector = R * xyNormal    (pure XY offset)
```
The CC point is always directly beneath the tool rim.

### Edge Contact
Circle-line intersection in canonical coordinates:
```
s = sqrt(R^2 - d^2)     // d = perpendicular distance to edge
```
Two candidate CC points at (s, d) and (-s, d).

### Offset
```
offsetCutter(d) -> BullCutter(D+2d, d, L+d)
```
Offsetting a flat produces a bull-nose (the sharp edge becomes rounded).

---

## 2. Ball End Mill (BallCutter)

### Definition
Hemisphere of radius R. Parameters: diameter d, length l.

### Profile
```
height(r) = R - sqrt(R^2 - r^2)
width(h)  = sqrt(2*R*h - h^2)      (h < R)
          = R                        (h >= R)
```

### Parameters
```
center_height    = R
normal_length    = R
xy_normal_length = 0
```

### Facet Contact
```
radiusvector = R * surfaceNormal    (pure normal offset)
```
The most elegant contact geometry: the offset is simply R along the surface normal.

### Edge Contact
Sphere-line intersection. Slice sphere at distance d from center to get circle of radius s:
```
s = sqrt(R^2 - d^2)
```
Find point on circle where slope matches edge slope.

### Edge Push
Ray-cylinder intersection (quadratic):
```
a*t^2 + b*t + c = 0
```
The fiber (raised by R) is a ray; the edge is surrounded by a cylinder of radius R.

### Scallop Height
```
h = R - sqrt(R^2 - (stepover/2)^2)
stepover = 2 * sqrt(2*R*h - h^2)
```

### Offset
```
offsetCutter(d) -> BallCutter(D+2d, L+d)
```
Offset of a ball is a larger ball.

---

## 3. Bull Nose / Toroidal (BullCutter)

### Definition
Flat bottom with rounded corner. A torus (donut) section. Parameters: diameter d, corner radius r, length l.

### Derived Values
```
R  = d/2           // total radius
R1 = d/2 - r       // flat bottom radius (torus ring center)
R2 = r              // corner radius (torus tube radius)
```

### Profile
```
height(r) = 0.0                                     (r <= R1, flat region)
          = R2 - sqrt(R2^2 - (r - R1)^2)            (R1 < r <= R, torus region)

width(h)  = R                                        (h >= R2)
          = R1 + sqrt(R2^2 - (R2 - h)^2)            (h < R2)
```

### Parameters
```
center_height    = R2
normal_length    = R2
xy_normal_length = R1
```

### Facet Contact
```
radiusvector = R1 * xyNormal + R2 * surfaceNormal
```
Two-component offset: R1 in XY (flat part) + R2 along normal (torus part).

### Edge Contact - The Offset Ellipse Method

This is the most complex contact calculation in CAM.

When a vertical plane slices the torus around an edge, it creates an **ellipse**:
```
theta = atan(edge_slope_in_XZ)
b_axis = R2                    // short axis (always R2)
a_axis = |R2 / sin(theta)|    // long axis (grows as edge becomes more horizontal)
```

The CL must lie on an **offset ellipse** (original ellipse expanded by R1 along normals). The offset ellipse has no closed-form equation.

**Solution**: Use Brent's root-finding method to find the ellipse parameter where the offset point lands at CL = (0, 0).

The ellipse is parameterized by "diamond angle" d in [0, 4]:
```
s = cos_like(d)    // x-component
t = sin_like(d)    // y-component
s^2 + t^2 = 1
```

Error function: `oePoint(d).y = 0` (seek the offset-ellipse point with y = 0).
Bracket: d in [0, 3]. Tolerance: 1e-10.

### Generalization

The bull nose is a **generalization** of flat and ball:
- C(d, 0) = Flat end mill (R2=0, R1=d/2)
- C(d, d/2) = Ball end mill (R2=d/2, R1=0)
- C(d, r) where 0 < r < d/2 = Bull nose

### Offset
```
offsetCutter(d) -> BullCutter(D+2d, R2+d, L+d)
```

---

## 4. V-Bit / Cone (ConeCutter)

### Definition
Cone with half-angle alpha. Parameters: diameter d, half-angle alpha (radians), length l.

### Profile
```
height(r) = r / tan(alpha)
width(h)  = h * tan(alpha)     (h < center_height)
          = R                   (h >= center_height)
```

### Parameters
```
center_height    = R / tan(alpha)
normal_length    = 0
xy_normal_length = R
```

### Facet Contact - Two Modes

1. **Tip contact** (FACET_TIP): CC at CL position projected onto facet plane
2. **Rim contact** (FACET_CYL): CC at CL - R*xyNormal projected onto facet

Both tested; highest valid one used.

### Edge Contact - Hyperbola Intersection

Vertical plane slicing a cone creates a **hyperbola**.

```
d = distance from CL to edge
xu = sqrt(R^2 - d^2)
m = edge slope
mu = (center_height/R) * xu / sqrt(xu^2 + d^2)    // max hyperbola slope at xu
```

**Case 1** (|m| <= |mu|): Contact on conical surface
```
ccu = sign(m) * sqrt(R^2 * m^2 * d^2 / (L^2 - R^2 * m^2))
```

**Case 2** (|m| > |mu|): Contact at cone/shaft boundary (circular rim)
```
ccu = sign(m) * xu
```

### Offset
```
offsetCutter(d) -> BallConeCutter(2*d, D+2d, alpha)
```
Offsetting a cone produces a tapered ball (sharp tip becomes a ball).

---

## 5. Tapered Ball End Mill (BallConeCutter - Composite)

### Definition
Spherical tip + conical taper. The critical tool for detailed wood carving.

Parameters: ball diameter diam1, shaft diameter diam2, half-angle alpha.

### Geometric Construction

Ball and cone meet at a tangent point:
```
R_ball = diam1 / 2
rcontact = R_ball * cos(alpha)                        // contact ring radius
h_contact = R_ball * (1 - sin(alpha))                 // height at contact
cone_offset = -(rcontact / tan(alpha) - h_contact)    // Z offset for cone sub-cutter
```

### Profile
```
For r from 0 to rcontact:    Ball profile
    height(r) = R_ball - sqrt(R_ball^2 - r^2)

For r from rcontact to R_shaft: Cone profile (with offset)
    height(r) = r / tan(alpha) + cone_offset
```

### CC/CL Computation

Uses CompositeCutter delegation:
1. Determine which sub-cutter (ball or cone) the CC point falls on via `radius_to_index(r)`
2. Apply the appropriate sub-cutter's contact math with its Z-offset
3. Validate that CC is within the sub-cutter's valid radial range

For ball region: identical to BallCutter (offset = R_ball * normal).
For cone region: ConeCutter math with cone_offset applied.

### Why This Tool Matters

- Ball tip gives smooth 3D surface finish
- Taper provides structural strength (less deflection)
- Essential for sign-making, relief carving, fine detail work
- Can reach deeper pockets than straight ball end mills of equivalent tip size

---

## 6. Tapered Bull Nose (BullConeCutter - Composite)

Parameters: diam1, corner_radius, diam2, half-angle.

### Geometric Construction
```
h1 = corner_radius * sin(angle)
rad = sqrt(corner_radius^2 - h1^2)
rcontact = (diam1/2) - corner_radius + rad
cone_offset = -(rcontact / tan(angle) - (corner_radius - h1))
```

### Profile
- Below rcontact: BullCutter profile (flat + torus)
- Above rcontact: ConeCutter profile with offset

---

## 7. Compound Taper (ConeConeCutter - Composite)

Two different cone angles. Parameters: diam1, angle1, diam2, angle2.
Constraint: angle2 < angle1, diam2 > diam1.

```
height1 = (diam1/2) / tan(angle1)
tmp = (diam1/2) / tan(angle2)
cone_offset = -(tmp - height1)
```

---

## 8. Flat + Taper (CylConeCutter - Composite)

Flat bottom transitioning to conical taper.

```
cone_offset = -(diam1/2) / tan(angle)
```

---

## 9. Generic/Arbitrary Tool Profile

For form tools or non-standard shapes, the profile can be discretized:

```
profile = [(r0, h0), (r1, h1), (r2, h2), ...]
height(r) = linear_interpolate(profile, r)
width(h) = linear_interpolate(inverse_profile, h)
```

### Trade-offs
- **Analytical** (specific cutter types): Exact CC/CL math, fastest computation
- **Discretized** (generic profile): Approximate CC/CL via sampling, universal but slower

### Recommended Approach

Implement analytical types for the common cutters (flat, ball, bull, cone, and the composites). Provide a generic discretized fallback for exotic tool shapes. Use a trait to abstract over both.

---

## Summary Table

| Type | height(r) | center_height | normal_length | xy_normal_length |
|------|-----------|---------------|---------------|------------------|
| Flat | 0 | 0 | 0 | R |
| Ball | R - sqrt(R^2 - r^2) | R | R | 0 |
| Bull | 0 or R2-sqrt(R2^2-(r-R1)^2) | R2 | R2 | R1 |
| Cone | r/tan(a) | R/tan(a) | 0 | R |
| Tapered Ball | ball then cone | composite | composite | composite |
| Tapered Bull | bull then cone | composite | composite | composite |

### The Offset Cutter Chain

Offsetting transforms cutters:
```
Flat   --offset(d)--> Bull(D+2d, d)
Ball   --offset(d)--> Ball(D+2d)
Bull   --offset(d)--> Bull(D+2d, R2+d)
Cone   --offset(d)--> BallCone(2d, D+2d, alpha)
```
