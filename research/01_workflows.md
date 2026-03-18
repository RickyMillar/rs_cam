# Supported Workflows

Comprehensive catalog of CAM workflows this program should support, organized by complexity tier.

---

## Tier 1: 2.5D Operations (Foundation)

### 1.1 Pocket Clearing

**What**: Remove material from an enclosed 2D region at one or more Z depths.

**Input**: 2D boundary polygon (from SVG, DXF, or mesh slice) + depth parameters.

**Algorithm**: Contour-parallel offsetting (inward) using Clipper2/i_overlay.

**Patterns**:
- Offset/contour (outside-in or inside-out)
- Zigzag/raster fill
- Spiral

**Key parameters**: step-over (40-60% roughing, 10-20% finishing), step-down, tool diameter.

**Island handling**: Pockets may contain raised islands. The toolpath must route around them. Computed via polygon boolean difference (pocket boundary minus island boundaries).

**Entry**: Helix or ramp entry to avoid straight plunges (critical for flat end mills that don't cut on center).

---

### 1.2 Profile / Contour Cutting

**What**: Cut along the outside or inside boundary of a 2D shape, typically through the full material thickness.

**Input**: 2D contour + depth + inside/outside selection.

**Algorithm**: Offset contour by tool radius (left for climb, right for conventional). Multi-pass with step-down.

**Features**:
- Tabs/bridges to hold pieces in place (rectangular, triangular, or rounded)
- Lead-in/lead-out arcs for smooth entry/exit
- Climb vs conventional direction selection

---

### 1.3 Facing

**What**: Flatten the top surface of stock to a uniform height.

**Input**: Stock boundary + target Z height.

**Algorithm**: Zigzag or offset pattern covering the full stock area. Simple variant of pocketing.

---

### 1.4 Drilling

**What**: Create holes at specified XY coordinates.

**Input**: List of (X, Y, depth) coordinates + optional peck parameters.

**Algorithm**:
- Simple: G0 to position, G1 plunge to depth
- Peck drilling: Incremental plunges with retracts for chip clearing
- Helical: Spiral into material for larger holes

---

### 1.5 Trace / Follow Path

**What**: Follow an SVG/DXF path at a specified depth with a specified tool.

**Input**: Vector path (lines, arcs, beziers) + depth + offset side (left/right/center).

**Algorithm**:
1. Convert beziers to polyline approximations (adaptive subdivision)
2. Offset by tool radius if not center-line
3. Apply depth stepping
4. Generate G1/G2/G3 moves

**Use cases**: Sign lettering, decorative borders, PCB isolation routing.

---

## Tier 2: V-Carving & Inlay

### 2.1 V-Carving

**What**: Carve designs using a V-bit where depth varies according to the width of the design at each point.

**Input**: 2D design boundaries (text, logos, artwork).

**Algorithm**:
- For each point inside the design, find the maximum inscribed circle (distance to nearest boundary)
- Depth = inscribed_radius / tan(half_angle)
- Tool follows the medial axis (Voronoi skeleton) of the design
- Depth varies continuously along the path

**Flat-bottom V-carving**: For wide areas where V-bit would cut too deep:
- Clamp max depth
- Use flat end mill to clear the flat bottom
- V-bit only cuts the sloped edges near boundaries

---

### 2.2 Inlay Operations

**What**: Create matching male/female V-carved pieces that fit together.

**Algorithm**:
- Female (pocket): Standard V-carve of the design
- Male (plug): V-carve the negative space, cut proud
- Gap compensation: Offset male inward by half desired gap (0.001" - 0.005")
- After glue-up, sand/route flush

---

## Tier 3: 3D Surface Machining

### 3.1 3D Roughing (Layered Clearing)

**What**: Remove bulk material from above a 3D surface in horizontal layers.

**Input**: STL mesh + stock definition + tool.

**Algorithm**:
1. Slice mesh at each Z level to get 2D contours (waterline)
2. At each level, compute the union of all contours below
3. Offset inward by tool radius
4. Clear using 2D pocket strategies (zigzag, offset, or adaptive)
5. Step down to next level

**Optimization**: Adaptive clearing at each level for constant tool engagement.

---

### 3.2 3D Finishing - Parallel/Raster

**What**: Final passes that follow the 3D surface contour for smooth finish.

**Input**: STL mesh + ball/tapered ball end mill + step-over.

**Algorithm**: Drop-cutter
1. Generate a grid of parallel XY lines at specified step-over
2. For each point on each line, drop the cutter onto the mesh
3. The CL Z-height is the maximum Z where the cutter contacts without gouging
4. Connect CL points into toolpath segments

**Direction**: Along X, Y, or any angle. Cross-hatching (two perpendicular passes) gives better finish.

**Scallop height**: h = R - sqrt(R^2 - (stepover/2)^2) for ball end mill on flat surface.

---

### 3.3 3D Finishing - Waterline/Contour

**What**: Generate toolpath contours at constant Z heights that follow the model surface.

**Input**: STL mesh + tool + Z step-down.

**Algorithm**: Push-cutter + Weave
1. At each Z height, generate X and Y fibers (scan lines)
2. Push cutter along each fiber to find contact intervals
3. Build Weave graph from intersecting intervals
4. Extract closed contour loops via face traversal

**Best for**: Steep walls and vertical surfaces where raster finishing would have excessive scallop.

---

### 3.4 3D Finishing - Constant Scallop Height

**What**: Maintain uniform surface finish quality across varying surface curvatures.

**Algorithm**:
- Compute effective tool radius accounting for surface curvature
- On convex surfaces: R_eff = R * R_surface / (R + R_surface) -- reduce step-over
- On concave surfaces: R_eff = R * R_surface / (R_surface - R) -- increase step-over
- Generate toolpath with variable step-over to maintain constant scallop height

**Complexity**: Higher than parallel finishing. Requires iterative offset computation on the surface.

---

### 3.5 Pencil Finishing

**What**: Clean up internal corners and high-curvature regions that larger tools missed.

**Input**: STL mesh + small ball end mill.

**Algorithm**: Detect surface regions where curvature exceeds a threshold (concave fillets, internal corners). Generate toolpaths along these features.

---

### 3.6 Rest Machining

**What**: Use smaller tools to clean up material that larger tools could not reach.

**Input**: STL mesh + previous tool's envelope (or stock model after previous ops) + smaller tool.

**Algorithm**:
1. Maintain a stock model (heightmap/dexel) representing material after previous operations
2. Compare stock model to desired finish surface
3. Where difference > threshold, generate toolpaths with the smaller tool
4. Only cut where the previous tool left material

---

## Tier 4: Adaptive / High-Performance

### 4.1 Adaptive Clearing (Constant Engagement)

**What**: Roughing strategy that maintains constant tool engagement angle, enabling deeper cuts with less tool wear.

**Input**: Stock boundary + part boundary + tool + engagement parameters.

**Algorithm** (Freesteel/Adaptive2d approach):
1. Start at an entry point (outside material or helix in)
2. At each step, search for the direction that produces the target cut area
3. Dynamically adjust path to maintain engagement between min/max thresholds
4. When blocked, find new entry points for remaining uncleared material
5. Chain resulting paths to minimize rapid moves

**Key benefit**: 10-25% radial step-over but full-depth axial cuts. Faster material removal, less tool wear.

**Target engagement angle**: alpha = arccos(1 - WOC/R), typically targeting alpha corresponding to 10-25% step-over equivalent.

---

### 4.2 Trochoidal Milling

**What**: Specialized slotting strategy where the tool follows circular arcs (trochoidal motion) to maintain partial engagement.

**Use case**: Cutting slots narrower than the tool diameter with controlled engagement.

---

## Cross-Cutting Concerns (Apply to All Workflows)

### Depth Stepping
- All operations need configurable step-down (axial depth per pass)
- Step-down varies by operation type, tool, and material
- Roughing: 0.5x-2x tool diameter in wood
- Finishing: single pass at final depth

### Entry Strategies
- **Ramp**: Enter at 2-5 degree angle while moving in XY
- **Helix**: Spiral downward (50-75% of tool diameter helix radius)
- **Plunge**: Direct vertical entry (only for ball/drill, not flat end mills)

### Dressup Operations (Post-processing on generated toolpaths)
- **Tabs/Bridges**: Insert holding tabs in profile cuts
- **Dogbone fillets**: Oversize inside corners for square-fitting parts
- **Lead-in/Lead-out**: Smooth arc transitions at cut entry/exit
- **Ramp entry**: Convert plunge moves to ramped entry
- **Boundary limiting**: Constrain toolpath to a specified region

### Toolpath Linking
- Retract to safe Z for rapid repositioning
- Minimize total rapid move distance (TSP optimization)
- Avoid fixture/clamp collisions during rapids
- Keep tool down when safe (reduce retract/plunge cycles)

### Safety
- Never rapid in XY at cutting Z
- Always retract to safe Z before repositioning
- Validate all moves against stock/fixture boundaries
- Soft limit checking against machine travel limits
