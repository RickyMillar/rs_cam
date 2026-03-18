# UX Terminology Reference

Maps internal/algorithmic terms to what users actually see in popular CAM software.
Use this when designing user-facing features, CLI flags, TOML config keys, and documentation.

---

## Recommended rs_cam Terms

Based on the most common usage across tools, weighted toward the hobbyist wood CNC audience.

### Operations

| Internal/Algorithm Term | Recommended UX Term | Rationale |
|------------------------|---------------------|-----------|
| Contour-parallel pocketing | **Pocket** | Universal across all tools |
| Profile / contour cutting | **Profile** | VCarve, Carveco, FreeCAD use "Profile". Fusion uses "Contour" but that's ambiguous with 3D contour |
| Adaptive clearing | **Adaptive** | Fusion 360 and FreeCAD both use this |
| 3D layered roughing | **3D Rough** | VCarve, Carbide Create, Kiri:Moto all use "Rough" |
| Drop-cutter raster finishing | **3D Finish** | Most hobbyist tools. Don't say "parallel" or "raster" at the top level |
| Waterline / Z-level contour | **Waterline** | FreeCAD uses this. VCarve calls it "Offset" pattern within 3D Finish. Keep as a strategy within 3D Finish |
| Constant scallop finishing | **Scallop** | Fusion 360's term. Rarely seen in hobbyist tools |
| Pencil finishing | **Pencil** | Fusion 360's term. Rarely seen in hobbyist tools |
| V-carving | **VCarve** | Universal |
| Rest machining | **Rest Machining** | Fusion 360, VCarve, Carbide Create all use this exact term |
| Drilling | **Drill** | Universal (Estlcam uses "Hole" but Drill is dominant) |
| Facing / surfacing | **Face** | Fusion 360, FreeCAD. Kiri:Moto uses "Level". VCarve does it via Pocket |
| Trace / follow path | **Engrave** | Most tools use "Engrave". VCarve also has "Fluting" for follow-path |
| Inlay | **Inlay** | VCarve's "VCarve Inlay Toolpath", Carveco's "Inlay" |

### Parameters

| Internal/Algorithm Term | Recommended UX Term | Also Acceptable | Avoid |
|------------------------|---------------------|-----------------|-------|
| Radial step-over | **Stepover** | Step Over | "Radial depth of cut", "ae" |
| Axial step-down | **Depth per Pass** | Step Down, Pass Depth | "Axial depth of cut", "ap" |
| Horizontal feed rate | **Feed Rate** | | "Cutting feedrate", "F" |
| Vertical feed rate | **Plunge Rate** | Plunge Feed | "Vertical feed rate" |
| Spindle angular velocity | **Spindle Speed** | RPM | "Angular velocity", "N" |
| Safe retract height | **Safe Z** | Clearance Height, Safety Height | "Retract plane" |
| Finishing allowance | **Stock to Leave** | Allowance | "Finishing offset" |
| Cusp height | **Scallop Height** | Cusp Height | "Surface deviation" |
| Machining tolerance | **Tolerance** | Accuracy | "Epsilon" |
| Entry strategy angle | **Ramp Angle** | | "Helix pitch angle" |
| Holding bridge height | **Tab Height** | Bridge Height | |
| Holding bridge width | **Tab Width** | Bridge Width | |
| Tool engagement angle | **Engagement** | Optimal Load (Fusion term) | "WOC", "ae" |
| Maximum material thickness per operation | **Max Depth** | Final Depth, Cut Depth | |
| Top of material Z | **Top of Stock** | Material Top, Start Depth | |

### Cutting Patterns (within operations)

| Internal Term | Recommended UX Term | Notes |
|--------------|---------------------|-------|
| Zigzag (bidirectional raster) | **Zigzag** | Fusion, FreeCAD use this. VCarve/Carveco call it "Raster" |
| One-way raster | **One Way** | FreeCAD calls it "Line" |
| Contour-parallel offset | **Offset** | Universal |
| Archimedean/Fermat spiral | **Spiral** | Fusion, Carveco |
| Climb milling direction | **Climb** | Universal |
| Conventional milling direction | **Conventional** | Universal |

### Tool Types

| Internal Term | Recommended UX Term | Also Acceptable |
|--------------|---------------------|-----------------|
| CylCutter / flat endmill | **End Mill** | Flat End Mill, Square End Mill |
| BallCutter / spherical | **Ball Nose** | Ball End Mill |
| BullCutter / toroidal | **Bull Nose** | Corner Radius, Radiused End Mill |
| ConeCutter / V-bit | **V-Bit** | V-Cutter, Engraving Bit |
| BallConeCutter / tapered ball | **Tapered Ball Nose** | Tapered Ball End Mill |
| CompositeCutter / tapered flat | **Tapered End Mill** | |
| Generic discretized profile | **Form Tool** | Custom Profile |

### Workflow Concepts

| Internal Term | Recommended UX Term | Rationale |
|--------------|---------------------|-----------|
| CamOperation | **Toolpath** | Hobbyist tools (VCarve, Carbide Create, Carveco) all say "Toolpath". Use "Toolpath" in user-facing contexts, "Operation" internally. |
| Job / project file | **Job** | FreeCAD uses "Job". Simple and clear. |
| Stock definition | **Stock** | Engineering-origin term but widely understood. VCarve/Carveco use "Material" but "Stock" is less ambiguous. |
| Post-processor | **Post Processor** | Universal |
| Material removal simulation | **Simulation** | Fusion, Carbide Create, Carveco. VCarve uses "Preview" but Simulation is more descriptive. |
| Dressup modification | **Modification** | "Dressup" is FreeCAD-specific jargon. Users think of these as "adding tabs" or "adding ramp entry", not "applying a dressup". |

### Entry Strategies

| Internal Term | Recommended UX Term |
|--------------|---------------------|
| Helix entry | **Helix** |
| Ramp entry | **Ramp** |
| Plunge entry | **Plunge** |
| Profile ramp (along first segment) | **Profile Ramp** |

---

## Naming Conventions for CLI and Config

### CLI Flags
Use the UX terms in kebab-case:
```
--stepover 3.0
--depth-per-pass 5.0
--feed-rate 2000
--plunge-rate 1000
--spindle-speed 18000
--safe-z 10.0
--stock-to-leave 0.5
--scallop-height 0.1
--tab-height 2.0
--tab-width 5.0
```

### TOML Config Keys
Use the UX terms in snake_case:
```toml
stepover = 3.0
depth_per_pass = 5.0
feed_rate = 2000
plunge_rate = 1000
spindle_speed = 18000
safe_z = 10.0
stock_to_leave = 0.5
```

### Operation Type Names (in TOML)
```toml
type = "pocket"
type = "profile"
type = "adaptive"
type = "3d_rough"
type = "3d_finish"
type = "waterline"
type = "vcarve"
type = "drill"
type = "face"
type = "engrave"
type = "inlay"
```

---

## Cross-Software Terminology Tables

### Operation Names Across Tools

| Concept | Fusion 360 | VCarve/Aspire | Carbide Create | Carveco | FreeCAD | Kiri:Moto | Estlcam | **rs_cam** |
|---------|-----------|---------------|----------------|---------|---------|-----------|---------|------------|
| 2D pocket | 2D Pocket | Pocket Toolpath | Pocket | Area Clearance | Pocket Shape | Pocket | Pocket | **Pocket** |
| Profile cut | 2D Contour | Profile Toolpath | Contour | Profile | Profile | Outline/Trace | Part/Carve | **Profile** |
| Adaptive clear | 2D Adaptive | -- | -- | -- | Adaptive | -- | -- | **Adaptive** |
| 3D roughing | 3D Adaptive | 3D Rough Toolpath | 3D Rough | Machine Relief | 3D Pocket | Rough | 3D | **3D Rough** |
| 3D raster finish | Parallel | 3D Finish (Raster) | 3D Finish | Machine Relief (Raster) | 3D Surface (Line/Zigzag) | Contour (linear) | 3D | **3D Finish** |
| 3D waterline | Contour (3D) | 3D Finish (Offset) | -- | Machine Relief (Spiral) | Waterline | Contour (waterline) | -- | **Waterline** |
| V-carving | Engrave/Trace | V-Carve Toolpath | VCarve | V-Bit Carving | Vcarve | -- | Engrave | **VCarve** |
| Rest machining | Rest Machining | Rest Machining | Rest Machining | -- | -- | -- | -- | **Rest Machining** |
| Drilling | Drill | Drilling Toolpath | Drill | Drilling | Drilling | Drill | Hole | **Drill** |
| Facing | Face | (via Pocket) | (via Pocket) | Area Clearance | Face | Level | (via Pocket) | **Face** |
| Follow path | Trace | Quick Engraving / Fluting | -- | Fluting | Engrave | Trace | Engrave | **Engrave** |

### Parameter Names Across Tools

| Concept | Fusion 360 | VCarve | Carbide Create | FreeCAD | **rs_cam** |
|---------|-----------|--------|----------------|---------|------------|
| Radial DOC | Stepover | Stepover | Stepover | Step Over | **Stepover** |
| Axial DOC | Maximum Stepdown | Pass Depth | Depth per Pass | Step Down | **Depth per Pass** |
| XY feed | Cutting Feedrate | Feed Rate | Feed Rate | Horizontal Feed Rate | **Feed Rate** |
| Z feed | Plunge Feedrate | Plunge Rate | Plunge Rate | Vertical Feed Rate | **Plunge Rate** |
| RPM | Spindle Speed | Spindle Speed | RPM | Spindle Speed | **Spindle Speed** |
| Retract Z | Clearance Height | Safe Z | Retract Height | Clearance Height | **Safe Z** |
| Finish allowance | Stock to Leave | Allowance | -- | Stock to Leave | **Stock to Leave** |
| Cusp target | Cusp Height | -- | -- | -- | **Scallop Height** |
| Holding bridges | Tabs | Tabs | Tabs | Tabs (via Dressup) | **Tabs** |

### Tool Names Across Tools

| Concept | Fusion 360 | VCarve | Carbide Create | Carveco | **rs_cam** |
|---------|-----------|--------|----------------|---------|------------|
| Flat | Flat End Mill | End Mill | Square End Mill | End Mill | **End Mill** |
| Ball | Ball End Mill | Ball Nose | Ball End Mill | Ball Nose | **Ball Nose** |
| Bull | Bull Nose End Mill | Radiused End Mill | -- | Radiused End Mill | **Bull Nose** |
| V-shaped | Chamfer Mill | V-Bit | V-Bit | V-Bit | **V-Bit** |
| Tapered + ball | Tapered Mill | Tapered Ball Nose | -- | -- | **Tapered Ball Nose** |

---

## Key Insight: "Toolpath" vs "Operation"

The word choice here signals your audience:

- **"Toolpath"** = hobbyist/maker audience (VCarve, Carbide Create, Carveco, Estlcam)
- **"Operation"** = engineering/professional audience (Fusion 360, FreeCAD)

**Recommendation for rs_cam**: Use **"Toolpath"** in all user-facing contexts (CLI help text, docs, error messages). Use "Operation" only in internal code (struct names, trait names). The target audience is hobbyist wood CNC operators.

## Key Insight: Hobbyist tools are simpler

Most hobbyist tools (VCarve, Carbide Create, Estlcam) have far fewer options than Fusion 360. They hide complexity:

- No separate "clearance height" vs "retract height" vs "safe height" -- just **Safe Z**
- No separate "radial stock to leave" vs "axial stock to leave" -- just **Stock to Leave**
- No named cutting strategies like "Scallop" or "Pencil" -- just basic patterns
- Adaptive clearing is absent from most hobbyist tools

**Recommendation**: Start simple. One Safe Z, one Stock to Leave, basic pattern names. Add Fusion-level granularity as advanced options later.
