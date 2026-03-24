# Shared Parameter Systems

Cross-cutting parameter systems that apply to all or many operations.

---

## 1. Heights System

**Config**: `HeightsConfig` in `state/toolpath/support.rs`
**GUI**: Heights tab on every operation

Five-level Z height system controlling vertical tool motion:

| Height | Auto default | Reference | Purpose |
|--------|-------------|-----------|---------|
| `clearance_z` | retract_z + 10 mm | StockTop, StockBottom, ModelTop, ModelBottom | Safe plane for long rapids between features |
| `retract_z` | safe_z (from post config) | same | Quick retract between passes in same feature |
| `feed_z` | retract_z - 2 mm | same | Switch from rapid to feed rate at this Z |
| `top_z` | 0.0 (stock top) | same | Top of material / start of cut |
| `bottom_z` | -(op_depth) | same | Bottom of cut / final depth |

### Height Modes

Each height has three modes:

| Mode | Behavior |
|------|----------|
| `Auto` | Computed from stock/model context (see defaults above) |
| `Manual(f64)` | User-specified absolute Z value |
| `FromReference(ref, offset)` | Offset from a named reference point |

### Height References

| Reference | Resolves to |
|-----------|------------|
| `StockTop` | `ctx.stock_top_z` (usually 0.0 for top-down setups) |
| `StockBottom` | `ctx.stock_bottom_z` |
| `ModelTop` | `ctx.model_top_z` (falls back to StockTop if no model) |
| `ModelBottom` | `ctx.model_bottom_z` (falls back to StockBottom) |

### Expected effect per height change

| Height | When increased | When decreased | Validation |
|--------|---------------|----------------|------------|
| clearance_z | More air travel time, safer rapids | Faster cycle, risk rapid collision | gcode: G0 Z values between features |
| retract_z | More retract travel between passes | Faster cycle, risk rapid collision | gcode: G0 Z values within feature |
| feed_z | Feed starts higher (wasted slow travel) | Feed starts closer to material (faster) | gcode: transition point from G0 to G1 |
| top_z | First cut starts above stock (air cut) | First cut starts in stock (crash risk) | sim: first pass Z level |
| bottom_z | Shallower total cut | Deeper total cut | sim: final pass Z level |

---

## 2. Dressup System

**Config**: `DressupConfig` in `state/toolpath/support.rs`
**GUI**: Dressups section in operation panel (applies to all operations)

### Entry Style

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `entry_style` | None | None, Ramp, Helix | Replaces vertical plunges with ramped/helical entry | gcode: plunge moves become angled/circular |
| `ramp_angle` (if Ramp) | 3.0 deg | 0.5–15 | Steeper = shorter ramp, faster but more tool load | gcode: ramp XZ angle |
| `helix_radius` (if Helix) | 2.0 mm | 0.5–20 | Helix circle radius | gcode: arc radius in helix |
| `helix_pitch` (if Helix) | 1.0 mm | 0.2–10 | Z drop per revolution | gcode: Z change per 360° |

### Dogbone

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `dogbone` | false | bool | Insert overcuts at inside corners | visual: corner extensions appear |
| `dogbone_angle` (if enabled) | 90.0 deg | 45–135 | Corners sharper than this get dogbones | visual: number of corners affected changes |

### Lead-In / Lead-Out

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `lead_in_out` | false | bool | Add arc approach/departure at cut start/end | visual: quarter-circle arcs at entry/exit |
| `lead_radius` (if enabled) | 2.0 mm | 0.5–20 | Arc radius for lead-in/out | gcode: arc radius |

### Link Moves

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `link_moves` | false | bool | Replace short retract→rapid→plunge with direct feed | visual: fewer retracts between close passes |
| `link_max_distance` (if enabled) | 10.0 mm | 1–50 | Max XY gap to bridge with feed instead of retract | gcode: retract count decreases |
| `link_feed_rate` (if enabled) | 500 mm/min | 50–5000 | Feed rate for link moves | gcode: F on link moves |

### Arc Fitting

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `arc_fitting` | false | bool | Fit G2/G3 arcs to linear toolpath segments | gcode: G2/G3 appear, file size drops |
| `arc_tolerance` (if enabled) | 0.05 mm | 0.01–0.5 | Max deviation from original path allowed | gcode: arc accuracy |

### Feed Optimization

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `feed_optimization` | false | bool | Vary feed rate based on stock engagement | gcode: varying F values along cuts |
| `feed_max_rate` (if enabled) | 3000 mm/min | 500–20000 | Maximum boosted feed rate in low-engagement zones | gcode: max F value |
| `feed_ramp_rate` (if enabled) | 200 mm/min/mm | 10–2000 | How fast feed ramps up as engagement decreases | gcode: F gradient |

**Note**: Feed optimization only works for fresh-stock, flat-stock workflows. Remaining-stock uses air-cut filter instead.

### Retract Strategy

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `retract_strategy` | Full | Full, Minimum | Full = always retract to retract_z; Minimum = retract to nearby path + 2mm | gcode: retract Z values |

### Rapid Order

| Config field | Default | Range | Expected effect | Validation |
|---|---|---|---|---|
| `optimize_rapid_order` | false | bool | TSP-optimize order of disconnected path segments | visual: rapid travel distance shrinks |

---

## 3. Feeds/Speeds Auto-Calculation

**Config**: `FeedsAutoMode` in `state/toolpath/support.rs`
**GUI**: Feeds tab on every operation

### Auto toggles

| Toggle | Default | Controls |
|--------|---------|----------|
| `feed_rate` | true | Auto-compute cutting feed rate from tool/material/machine |
| `plunge_rate` | true | Auto-compute plunge rate (typically 50% of feed rate) |
| `stepover` | true | Auto-compute radial engagement from tool diameter |
| `depth_per_pass` | true | Auto-compute axial depth from tool/material |
| `spindle_speed` | true | Auto-compute RPM from tool diameter and SFM |

When enabled, the feeds calculator runs on every Feeds tab render and writes results back into the operation config. When disabled, the user's manual value is preserved.

### Calculator inputs

| Input | Source |
|-------|--------|
| Tool diameter | Active tool config |
| Flute count | Active tool config |
| Cutting length | Active tool config |
| Tool geometry | Derived from tool type (Flat/Ball/Bull/VBit/TaperedBall) |
| Material | Stock material selection |
| Machine profile | Global machine config |
| Operation family | Mapped from operation type |
| Pass role | Roughing for clearing ops, Finish for surface ops |
| Workholding rigidity | Hardcoded to Medium (GUI placeholder) |

### Operation → family mapping

| Operations | Family | Pass Role |
|-----------|--------|-----------|
| Adaptive, Adaptive3D, Pocket | Adaptive/Pocket | Roughing |
| Profile | Contour | Roughing |
| Face | Face | Roughing |
| Trace | Trace | SemiFinish |
| VCarve, Chamfer, Inlay | Trace | Finish |
| DropCutter, Zigzag, HorizontalFinish | Parallel | Finish |
| Waterline, SteepShallow, RampFinish | Contour | Finish |
| Scallop | Scallop | Finish |
| Pencil, Spiral, Radial, ProjectCurve | Parallel | Finish |

---

## 4. Stock Awareness

**Config fields** in `state/toolpath/support.rs`:

| Feature | Config | GUI | Default | Effect |
|---------|--------|-----|---------|--------|
| `StockSource` | per-toolpath | Yes (Mods tab) | Fresh | Fresh = raw stock; FromRemainingStock = sim prior ops first |
| Clip to Stock | per-toolpath | Yes (Mods tab) | false | Restrict toolpath to stock boundary |
| `BoundaryContainment` | per-toolpath | Yes (Mods tab) | Center | Center/Inside/Outside = where tool center/edge must be relative to boundary |

### Expected effects

| Feature | When changed | Validation |
|---------|-------------|------------|
| StockSource → FromRemainingStock | Toolpath only cuts where material remains after prior ops | sim: compare with fresh — air cuts eliminated |
| Clip to Stock ON | Toolpath stops at stock edges | visual: no paths outside stock |
| Containment → Inside | Tool edge stays inside boundary | visual: path shrinks inward by tool radius |
| Containment → Outside | Tool edge stays outside boundary | visual: path extends outward by tool radius |

---

## 5. Depth Stepping

**Struct**: `DepthStepping` in `depth.rs`
**Used by**: Face, Pocket, Profile, Adaptive, Zigzag, Rest, Trace (any multi-pass 2.5D op)

| Field | Source | Effect |
|-------|--------|--------|
| `start_z` | Heights top_z | Top of material |
| `final_z` | Heights bottom_z | Target depth |
| `max_step_down` | depth_per_pass config | Max Z per pass |
| `distribution` | Even (hardcoded) | Even = equal passes; Constant = max except last |
| `finish_allowance` | 0.0 (not exposed) | Leave material for finish pass |
| `finishing_passes` | Profile/Pocket config | Spring passes at final depth |

---

## 6. Face Selection (BREP)

**GUI**: Mods tab for 3D operations with STEP model

| Feature | Effect |
|---------|--------|
| Face selection ON | Operation uses only selected BREP faces as machining region |
| Face selection OFF | Operation uses full model/stock boundary |

Only works for approximately-horizontal planar faces. Non-planar faces fall back to stock bounds.
