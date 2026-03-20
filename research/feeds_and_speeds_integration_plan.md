# Feeds & Speeds Integration Plan for rs_cam

> Reference implementation: `reference/shapeoko_feeds_and_speeds/`
> This document is a UX and feature implementation plan — not a roadmap with timelines.

---

## 1. Machine Setup & Profiles

### What exists in shapeoko reference
The reference defines `MachineProfile` (`reference/shapeoko_feeds_and_speeds/src/machine_profile.rs`) with:
- **Spindle control**: Variable RPM (VFD) or Discrete RPM steps (router with fixed dials)
- **Power model**: VFD constant-torque curve (power scales with RPM) or constant-power (router)
- **Spindle curve**: Interpolated calibration points for real power-at-RPM lookup
- **Feed limits**: Max feed rate, max supported shank diameter
- **DOC/WOC profiles**: Per-machine rigidity-dependent depth/width defaults
- **Profile tiers**: Conservative / Balanced / Aggressive (scale safety factor + limits)
- **Builder pattern**: `MachineProfile::custom("name").variable_rpm(6000, 24000).power_model(...).build()`

### What rs_cam currently has
- `PostConfig` in `crates/rs_cam_viz/src/state/job.rs:204` stores only `format`, `spindle_speed: u32`, `safe_z`
- No machine model, no power curve, no feed limits
- Spindle speed is a flat number with no relationship to tool/material

### What to build

**A. `MachineProfile` struct in `rs_cam_core`**

Add a new module `crates/rs_cam_core/src/machine.rs`:

```
MachineProfile {
    name: String,
    spindle: SpindleConfig,        // Variable{min,max} | Discrete{speeds}
    power: PowerModel,             // VfdConstantTorque | ConstantPower
    spindle_curve: Vec<(f64,f64)>, // optional calibration points
    max_feed_mm_min: f64,
    max_shank_mm: f64,
    rigidity: RigidityProfile,     // DOC/WOC factors
    safety_factor: f64,            // 0.7-0.9 range
}
```

Key functions to port from `reference/shapeoko_feeds_and_speeds/src/machine_profile.rs`:
- `clamp_rpm()` — snap to discrete speeds or clamp to range (line 309)
- `power_at_rpm()` — interpolate spindle curve or use power model (line 333)
- `torque_at_rpm_nm()` — derived from power (line 391)
- `is_shank_supported()` — validate tool fits collet (line 400)
- `apply_safety()` — multiply calculated values by safety factor (line 405)

**B. Built-in presets + custom builder**

Ship with presets that users recognize:
- Shapeoko (VFD 1.5kW) — the reference's `shapeoko_vfd_1500w()` (line 127)
- Shapeoko (Makita router) — the reference's `makita_router()` (line 231)
- "Generic 3-axis router" — conservative defaults
- Custom builder for anything else

Each preset should have Conservative/Balanced/Aggressive tiers like the reference (lines 199-223).

**C. UX: Machine Setup wizard in viz**

First-run or menu-accessible panel:
1. Pick machine preset or "Custom"
2. If Custom: spindle type (VFD vs router), RPM range, rated power, collet size
3. Aggressiveness slider (conservative → aggressive) — maps to safety_factor + DOC/WOC scaling
4. Save to project TOML alongside existing PostConfig

The machine profile should be **project-level state**, sitting next to `PostConfig` in `JobState`. Every feeds/speeds calculation flows through it.

**Implementation hints:**
- The reference `MachineProfileBuilder` (line 420) is a clean pattern to copy
- Serde derives already work in the reference — the TOML serialization in rs_cam's existing save/load will absorb this naturally
- The three tiers in the reference just scale `safety_factor`, `max_feed_rate_mm_min`, and DOC/WOC factors by fixed multipliers

---

## 2. Material Library

### What exists in shapeoko reference
`reference/shapeoko_feeds_and_speeds/src/params/mod.rs` defines:
- **Material enum** with 6 variants: SolidWood (10 species), Plywood (3 grades), SheetGood (MDF/HDF/Particleboard), Plastic (5 types), Metal (aluminum), Custom
- **Hardness index**: `hardness_index()` (line 435) — normalized to 1.0 = soft wood baseline, uses `(Janka/600)^0.4` for wood
- **Specific cutting force (Kc)**: `kc_n_per_mm2()` (line 446) — per-species/material values for power calculation
- **Janka database**: Per-species values from 500 (Radiata Pine) to 3510 (Ipe) lbf

### What rs_cam currently has
- `StockConfig` in `job.rs:222` — dimensions only, no material properties
- No material type, no hardness, no Kc values

### What to build

**A. `Material` enum in `rs_cam_core`**

New module `crates/rs_cam_core/src/material.rs`:

```
Material {
    SolidWood { species: WoodSpecies },
    Plywood { grade: PlywoodGrade },
    SheetGood { kind: SheetGoodKind },
    Plastic { family: PlasticFamily },
    Foam { density: FoamDensity },     // rs_cam addition for sign-making
    Custom { name, hardness_index, kc },
}
```

Port directly from `reference/shapeoko_feeds_and_speeds/src/params/mod.rs`:
- `WoodSpecies` enum with `janka_lbf()` method (line 226)
- `PlywoodGrade` with `effective_janka_lbf()` (line 256)
- `SheetGood` with `effective_janka_lbf()` (line 273)
- `Material::hardness_index()` (line 435) — the universal normalized hardness
- `Material::kc_n_per_mm2()` (line 446) — specific cutting force per material

Drop the `Metal` variant for now (rs_cam is wood-router focused per CLAUDE.md) but keep the trait boundary flexible so it can be added later.

Add `Foam` for sign-making (common wood-router use case, not in the reference).

**B. Link material to StockConfig**

```
StockConfig {
    // existing dimensions...
    material: Material,            // NEW
    grain_direction: Option<GrainDirection>,  // Along X, Along Y, N/A
    moisture_content: Option<f64>, // % — affects chip load (reference: MaterialCondition)
}
```

The reference's `MaterialCondition` struct (line 89) has `wood_moisture_percent` and `abrasiveness_factor` — these are real adjustments that affect tool life.

**C. UX: Material picker**

In the Stock properties panel (existing):
- Material dropdown grouped by category (Wood > species, Plywood > grade, etc.)
- When SolidWood selected: species sub-picker showing Janka hardness values
- "Custom" option with manual hardness + Kc entry
- Grain direction selector (for feeds/speeds: cross-grain needs different approach)
- Optional moisture content slider

**Implementation hints:**
- The reference `Material::shapeoko_catalog()` (line 413) returns a labeled list of common materials — use this pattern for the dropdown
- Hardness index is the single number that flows into all chip load calculations — get this right and everything else follows
- Kc values for wood are small (6-28 N/mm²) vs metals (800+) — the power limiter only matters for hard materials on underpowered spindles, but it's still worth implementing for safety

---

## 3. Tool Library Overhaul

### What exists in shapeoko reference
`reference/shapeoko_feeds_and_speeds/src/params/mod.rs` `Tool` struct (line 53):
- `material: ToolMaterial` (Carbide / HSS)
- `coating: Option<ToolCoating>` (TiN, TiAlN, AlTiN, DLC, Diamond)
- `cut_direction: CutDirection` (UpCut, DownCut, Compression, Neutral)
- `flute_count: u32`
- `flute_length_mm: f64`
- `overall_length_mm: f64`
- `stickout_mm: Option<f64>`
- `vendor: Option<String>`
- `product_id: Option<String>`
- `geometry: ToolGeometry` (6 variants including FacingBit and ChamferMill)

### What rs_cam currently has
`ToolConfig` in `crates/rs_cam_viz/src/state/job.rs:108`:
- `tool_type`, `diameter`, `cutting_length` — geometry only
- `holder_diameter`, `shank_diameter`, `shank_length`, `stickout` — collision detection
- No flute count, no material, no coating, no cut direction, no vendor info

### What to build

**A. Extend `ToolConfig` with cutting parameters**

Add to the existing `ToolConfig`:

```
// Cutting parameters (NEW)
pub flute_count: u32,
pub tool_material: ToolMaterial,      // Carbide | HSS
pub coating: Option<ToolCoating>,
pub cut_direction: CutDirection,      // UpCut | DownCut | Compression
pub flute_length: f64,               // mm — distinct from cutting_length for flute guard

// Vendor/catalog (NEW)
pub vendor: Option<String>,
pub product_id: Option<String>,
pub notes: Option<String>,
```

These fields directly mirror `reference/shapeoko_feeds_and_speeds/src/params/mod.rs` lines 53-68.

Flute count is the single most important addition — it's a multiplier in every feed rate calculation (`feed = RPM × chipload × flutes`). Without it, no automatic feeds/speeds are possible.

**B. Tool Library (persistent, project-independent)**

Currently tools live inside `JobState` and are per-project. Add a **global tool library**:

```
ToolLibrary {
    tools: Vec<ToolConfig>,
    path: PathBuf,  // ~/.rs_cam/tool_library.toml
}
```

Users should be able to:
- Define tools once in the library
- Import tools from library into a project
- Save project tools back to library
- Import from Fusion 360 `.tools` JSON (the reference has full schema at `reference/shapeoko_feeds_and_speeds/src/fusion_schema.rs`)

**C. UX: Tool library panel**

New panel accessible from menu or toolbar:
- List view with columns: Name, Type, Diameter, Flutes, Material, Vendor
- "Add to Project" button
- "Import from Fusion 360" button (reads the JSON format defined in `reference/shapeoko_feeds_and_speeds/src/fusion_schema.rs`)
- Inline editing with the existing tool cross-section preview
- Filter/search by type, diameter range, vendor

In the existing tool properties panel, add:
- Flute count spinner (prominent — this is critical)
- Material selector (Carbide/HSS)
- Cut direction selector (UpCut/DownCut/Compression)
- Optional vendor/product ID fields (collapsed by default)

**Implementation hints:**
- Flute count defaults: 2 for End Mill, 2 for Ball Nose, 2 for V-Bit, 2 for Bull Nose, 2 for Tapered Ball Nose — matches the reference's constructor defaults
- The reference's `Tool::flat_end()`, `Tool::ball_nose()` etc. (lines 71-205) are good patterns for builder/convenience constructors
- Cut direction matters for chip evacuation (downcut = cleaner top, upcut = cleaner bottom, compression = both) — affects quality warnings but not the core calculation
- The flute guard (`0.8 × flute_length`) from the reference is a critical safety check — DOC should never exceed this

---

## 4. Core Feeds & Speeds Calculator

### What exists in shapeoko reference
`reference/shapeoko_feeds_and_speeds/src/calcs.rs` — the full calculation pipeline:

1. **Chip load**: `K₀ × D^p × (1/H)^q` (line 169, via `MachineProfile::calculate_chip_load` at machine_profile.rs:259)
2. **Feed rate**: `RPM × ChipLoad × Flutes` (line 10)
3. **RPM selection**: Based on surface speed → `RPM = (V × 1000)/(π × D)` clamped to machine (machine_profile.rs:280)
4. **Plunge rate**: Material-dependent (line 197), ~50-100% of feed for wood, 30% for metal
5. **DOC/WOC**: Operation-family-specific matrices (line 427, 467)
6. **Power**: `Kc × AP × AE × VF / (60 × 1e6)` kW (line 499)
7. **Chip thinning**: Radial (line 672) and axial for ball nose (line 700)
8. **Effective diameter**: Ball (line 523), tapered ball (line 601), bull nose (line 633), V-bit width at depth (line 571)
9. **Scallop stepover**: `2 × R × sin(acos(1 - scallop/R))` for ball nose (line 547)
10. **Slotting detection**: If AE ≈ D, reduce DOC (referenced in warnings)
11. **Power limiting**: If calculated power > available, reduce feed proportionally

### What rs_cam currently has
- `FeedOptParams` in `crates/rs_cam_core/src/feedopt.rs` — post-process feed optimization using RCTF (radial chip thinning)
- Individual operation params have `feed_rate` and `plunge_rate` as flat user-entered values
- No automatic calculation, no chip load model, no power check

### What to build

**A. `FeedsCalculator` in `rs_cam_core`**

New module `crates/rs_cam_core/src/feeds.rs`:

```
pub struct FeedsRequest {
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    operation: OperationContext,
    setup: Option<SetupContext>,
}

pub struct FeedsResult {
    pub rpm: f64,
    pub chip_load_mm: f64,
    pub feed_rate_mm_min: f64,
    pub plunge_rate_mm_min: f64,
    pub ramp_feed_mm_min: f64,
    pub axial_depth_mm: f64,
    pub radial_width_mm: f64,
    pub power_kw: f64,
    pub power_limited: bool,
    pub mrr_mm3_min: f64,
    pub warnings: Vec<FeedsWarning>,
}
```

Port the core functions from `reference/shapeoko_feeds_and_speeds/src/calcs.rs`:

| Reference function | Line | What it does | Priority |
|---|---|---|---|
| `calculate_feedrate()` | 10 | `RPM × CL × flutes` | Must have |
| `calculate_chip_load_janka()` | 169 | Empirical chip load from Janka | Must have |
| `estimate_plunge_rate()` | 197 | Material-based plunge | Must have |
| `get_depth_of_cut()` | 427 | Operation × material → DOC | Must have |
| `get_width_of_cut()` | 467 | Operation × material → WOC | Must have |
| `calculate_cutting_power_kw()` | 499 | Power consumption check | Must have |
| `ball_effective_diameter_mm()` | 523 | Effective D at shallow DOC | Must have |
| `scallop_stepover_for_ball_mm()` | 547 | Scallop-based stepover | Must have |
| `radial_chip_thinning_factor()` | 672 | RCTF for thin engagement | Should have |
| `axial_chip_thinning_factor_for_ball()` | 700 | Axial thinning for ball | Should have |
| `tapered_ball_effective_diameter_mm()` | 601 | Tapered ball effective D | Should have |
| `bull_nose_effective_diameter_mm()` | 633 | Bull nose effective D | Should have |
| `vbit_cut_width_at_depth_mm()` | 571 | V-bit width from DOC | Should have |

The reference's `calculate_recommendation()` (line 1643) and `calculate_recommendation_with_inputs()` (line 1674) are the top-level orchestrators — they call all of the above in sequence with clamping and warning generation. Port this as `FeedsCalculator::calculate()`.

**B. `OperationContext` mapping**

Map rs_cam's existing operations to the reference's `OperationFamily` + `PassRole`:

| rs_cam operation | → OperationFamily | → Default PassRole |
|---|---|---|
| Pocket | Pocket | Roughing |
| Profile | Contour | Roughing |
| Adaptive | Adaptive | Roughing |
| VCarve | Trace | Finish |
| Face | Face | Roughing |
| 3D Rough (Adaptive3D) | Adaptive | Roughing |
| 3D Finish (Parallel/Scallop) | Parallel / Scallop | Finish |
| Waterline | Contour | SemiFinish |
| Ramp Finish | Parallel | Finish |

The reference's DOC/WOC matrices at `calcs.rs:427` and `calcs.rs:467` return different defaults based on these combinations — e.g., Adaptive roughing uses deep-narrow (high DOC, low WOC), Pocket roughing uses wide-shallow (low DOC, high WOC).

**C. Calculation pipeline (order matters)**

Port the reference's clamping order (tested in lib.rs):
1. Select RPM from material surface speed + tool diameter → clamp to machine range
2. Calculate chip load from RPM + material hardness + tool diameter
3. Calculate feed = RPM × chipload × flutes
4. Calculate DOC/WOC from operation family + pass role + machine rigidity
5. Check power: `Kc × DOC × WOC × feed / 60e6` — if exceeds `machine.power_at_rpm(rpm)`, reduce feed proportionally
6. Clamp feed to machine max feed rate
7. Calculate plunge rate (material-based fraction of feed)
8. Apply safety factor
9. Collect warnings

**Implementation hints:**
- The reference already has a clean `Recommendation` struct (line 134) — adapt it to rs_cam's naming conventions
- The chip thinning factor from `feedopt.rs` already exists in rs_cam — the reference's `radial_chip_thinning_factor()` (line 672) computes the same RCTF. These should share code.
- The scallop-based stepover at `calcs.rs:547` is exactly what rs_cam's 3D finish operations need for automatic stepover from target scallop height
- The flute guard (`0.8 × flute_length`) prevents DOC from exceeding safe engagement — implement this as a hard cap with a warning

---

## 5. Warnings & Guardrails System

### What exists in shapeoko reference
`reference/shapeoko_feeds_and_speeds/src/calcs.rs` `Warning` enum (line 23):
- `FeedRateClamped` — machine can't go that fast
- `PowerLimited` — spindle can't handle the load, feed was reduced
- `UnsupportedShank` — tool won't fit the collet
- `SlottingAdjusted` — full-width cut detected, DOC reduced for safety
- `AxialDepthCapped` / `RadialWidthCapped` — user request exceeded safe limits
- `InvalidScallopHeight` — scallop target too large for tool radius
- `VBitWidthCappedByShank` — V-bit engagement capped by shank diameter

### What to build

**A. `FeedsWarning` enum in `rs_cam_core`**

Port all warnings from the reference. Add rs_cam-specific ones:

```
FeedsWarning {
    FeedRateClamped { requested, actual },
    PowerLimited { required_kw, available_kw },
    ShankTooLarge { shank_mm, collet_mm },
    SlottingDetected { doc_reduced_to },
    DocExceedsFlute { requested, capped, flute_length },
    ScallopInvalid { target, max_possible },
    ChipLoadTooHigh { calculated, max_safe },
    ChipLoadTooLow { calculated, min_useful },
    StickoutExcessive { stickout, diameter },  // deflection risk
}
```

The last three are rs_cam additions. Chip load bounds catch gross errors (wrong flute count, wrong material). Excessive stickout is a common hobbyist mistake — the reference's `SetupContext.tool_overhang_mm` (line 81) feeds into this.

**B. UX: Warning display**

In the toolpath properties panel and the feeds/speeds suggestion panel:
- Yellow warning badges next to calculated values
- Tooltip with explanation ("Feed rate was reduced from 3200 to 2500 mm/min because the spindle can only deliver 0.75 kW at 12000 RPM")
- Red stop icon for dangerous conditions (DOC > flute length, shank > collet)

**Implementation hints:**
- Warnings should be non-blocking — the user can override. But DOC > flute length should be a hard error in the toolpath generator.
- The reference generates warnings during calculation and stores them in `Recommendation.warnings` — follow this pattern

---

## 6. Auto-Suggest UX Flow

### The core user experience

When a user creates a toolpath, they currently must manually enter: feed_rate, plunge_rate, spindle_speed, stepover, depth_per_pass. With feeds/speeds integration, the flow becomes:

1. User selects **tool** (from library — has flute count, material, geometry)
2. User selects **material** (from stock — has hardness, Kc)
3. User selects **operation type** (Pocket, Adaptive, 3D Finish, etc.)
4. System **auto-calculates**: RPM, feed, plunge, DOC, WOC, stepover
5. User sees calculated values **pre-filled** with option to override
6. Warnings shown inline if anything is clamped or limited

### What to build

**A. "Suggest" button on every operation parameter panel**

Next to feed_rate, plunge_rate, stepover, depth_per_pass fields:
- "Auto" toggle (default ON for new toolpaths)
- When Auto is ON: field shows calculated value, greyed background, editable
- When user edits: Auto turns OFF for that field, value is user-locked
- "Reset to suggested" button to re-enable Auto

**B. Feeds/speeds summary card**

Collapsible panel in toolpath properties:
```
┌─ Feeds & Speeds ──────────────────────┐
│  RPM:         18,000                   │
│  Feed:        2,400 mm/min             │
│  Plunge:      1,000 mm/min             │
│  Chip load:   0.067 mm/tooth           │
│  DOC:         6.0 mm                   │
│  WOC:         1.6 mm (25% stepover)    │
│  Power:       0.42 kW (of 1.12 avail) │
│  MRR:         23,040 mm³/min           │
│  ⚠ Feed clamped to machine max        │
└────────────────────────────────────────┘
```

This mirrors the reference's `Recommendation` struct fields (line 134).

**C. "What-if" mode**

Let users explore tradeoffs without committing:
- Slider for "aggressiveness" (maps to safety factor)
- Toggle between roughing/finishing pass defaults
- Show power bar (green/yellow/red) relative to machine capacity
- Show MRR for comparing strategies

**Implementation hints:**
- The auto-suggest should be lazy — recalculate only when tool, material, operation, or machine changes
- Store whether each field is "auto" or "manual" in the toolpath entry
- The reference's `OperationContext` (line 251) already has optional overrides for every parameter — use the same pattern: `None` = auto-calculated, `Some(x)` = user override

---

## 7. Operation-Specific Defaults

### What the reference provides

The reference's `get_depth_of_cut()` (`calcs.rs:427`) and `get_width_of_cut()` (`calcs.rs:467`) encode a matrix of defaults based on `OperationFamily × PassRole × Material`:

**Depth of cut (DOC) patterns:**
- Adaptive roughing: deep & narrow → `DOC = deep_narrow_roughing_factor × diameter` (e.g., 2×D)
- Pocket roughing: wide & shallow → `DOC = wide_shallow_roughing_factor × diameter` (e.g., 0.25×D)
- Contour roughing: wide & shallow → same as pocket
- Parallel finish: `DOC = wide_shallow_finishing_factor × diameter` (e.g., 0.1×D)
- All metal: fixed absolute values (e.g., 0.635mm roughing, 0.3175mm finish)

**Width of cut (WOC/stepover) patterns:**
- Adaptive roughing: narrow → `WOC = deep_narrow_roughing_factor × diameter` (e.g., 0.25×D)
- Pocket roughing: wide → `WOC = wide_shallow_roughing_factor × diameter` (e.g., 0.8×D)
- Contour roughing: full width (1.0×D)
- Finish: narrow (e.g., 0.1×D)

### What to build

Map these defaults into rs_cam's operation parameter structs. When a user creates a new operation and has Auto-suggest ON:

| Operation | DOC default | WOC/Stepover default |
|---|---|---|
| Pocket | 0.25 × D | 0.8 × D (capped at 6.35mm) |
| Adaptive | 2.0 × D | 0.25 × D |
| Profile | 0.25 × D | 1.0 × D (full engagement) |
| Face | 0.25 × D | 0.8 × effective_D |
| VCarve | max_depth from angle | stepover from angle |
| 3D Rough | 2.0 × D (adaptive) | 0.25 × D |
| 3D Finish (parallel) | scallop-based | scallop-based stepover |
| 3D Finish (scallop) | scallop-based | scallop-based stepover |

For 3D finish operations, the reference's `scallop_stepover_for_ball_mm()` (`calcs.rs:547`) computes stepover from target scallop height:
```
stepover = 2 × R × sin(acos(1 - scallop_height / R))
```

This is particularly valuable — users set a target surface quality (scallop height in mm) and the system derives the stepover automatically.

**Implementation hints:**
- The DOC/WOC profiles in `MachineProfile.doc_profile` and `woc_profile` (machine_profile.rs:81-110) let different machines have different default aggressiveness — a rigid machine gets bigger default DOC
- The flute guard (`0.8 × flute_length`) should hard-cap DOC regardless of the matrix default
- Slotting detection: if WOC ≈ diameter, automatically reduce DOC (the reference emits `SlottingAdjusted` warning)

---

## 8. Effective Diameter & Chip Thinning

### What the reference provides

For non-flat tools, the "effective cutting diameter" depends on depth of cut. This affects RPM selection and chip load:

**Ball nose** (`calcs.rs:523`):
```
D_eff = 2 × sqrt(D × ap - ap²)
where ap = axial depth, D = nominal diameter
```
At shallow cuts, D_eff << D, so RPM must increase to maintain surface speed.

**Tapered ball nose** (`calcs.rs:601`):
```
If ap <= tip transition height: use ball formula with tip diameter
Else: D_eff scales with taper angle
```

**Bull nose** (`calcs.rs:633`):
```
D_eff depends on whether cut is in corner radius region or flat region
```

**Radial chip thinning** (`calcs.rs:672`):
```
RCTF = 1 / sqrt(1 - (1 - 2×ae/D)²)
```
When stepover < 50% of diameter, actual chip thickness is less than programmed chip load. Feed can be increased by RCTF to maintain actual chip thickness.

**Axial chip thinning for ball nose** (`calcs.rs:700`):
Additional thinning factor for ball nose at shallow cuts.

### What rs_cam currently has
- `feedopt.rs` already implements RCTF (radial chip thinning factor) as a post-process dressup
- `tool/ball.rs`, `tool/tapered_ball.rs` etc. have the geometric profile functions but not effective-diameter-for-feeds

### What to build

**A. Effective diameter functions on `ToolConfig`**

Add methods to `ToolConfig` (or the `MillingCutter` trait) that return effective cutting diameter at a given depth:

```
impl ToolConfig {
    fn effective_diameter_at_depth(&self, axial_depth: f64) -> f64;
}
```

This feeds directly into RPM calculation — the feeds calculator uses effective D, not nominal D, for RPM and chip load.

**B. Unify chip thinning with feedopt**

The existing `feedopt.rs` already computes RCTF. The feeds calculator also needs RCTF for initial calculation. Extract the shared math into a common function and use it in both places.

**Implementation hints:**
- The reference's effective diameter functions are pure geometry — they match the profile equations already in rs_cam's tool modules, just applied differently
- For the "suggest" UI, show both nominal and effective diameter so users understand why RPM is different from what they'd expect
- RCTF > 1.0 means "you can go faster" — this is an optimization, not a safety concern

---

## 9. Evidence & Vendor Data (Future)

### What the reference provides

The reference has a sophisticated vendor lookup table (VLT) system:
- `reference/shapeoko_feeds_and_speeds/src/vendor_lut.rs` — observation data structures
- `reference/shapeoko_feeds_and_speeds/src/vendor_lookup.rs` — scoring algorithm
- `reference/shapeoko_feeds_and_speeds/data/vendor_lut/observations/` — 5 JSON files with real vendor data (Amana, Onsrud, Harvey, etc.)
- Evidence grading (A/B/C) based on source quality
- Observation matching by tool family, diameter, flute count, material, operation

The `Recommendation` includes an `evidence_trace` with full provenance: which observations were consulted, which formula was used, what derates were applied.

### What to build (later)

This is the most ambitious piece and should come after the core calculator is solid:

**A. Vendor observation database**

Port the JSON observation format and scoring algorithm. Ship the Amana/Onsrud datasets as built-in data.

**B. Evidence-aware suggestions**

When suggesting feeds/speeds, prefer vendor-published data when available for the specific tool/material combo. Fall back to the empirical formula when no vendor data matches.

Show the evidence source in the UI: "Based on Amana Tool published data for 1/4" upcut in hardwood" vs "Calculated from empirical formula".

**C. User observations**

Let users record their own successful cuts as observations that improve future suggestions. "This worked well" button that saves the current parameters as a user observation.

**Implementation hints:**
- The scoring algorithm in `vendor_lookup.rs` is ~150 lines — it's portable
- The JSON observation format is well-documented in `vendor_lut.rs`
- Start with just the legacy formula fallback path. Add VLT scoring once the core is proven.
- The reference's `RecommendationOrigin` enum (calcs.rs:115) tracks whether the result came from VLT or formula — important for user trust

---

## 10. Fusion 360 Tool Library Import

### What the reference provides
- `reference/shapeoko_feeds_and_speeds/src/fusion_schema.rs` (16KB) — complete Fusion 360 `.tools` JSON schema
- `reference/shapeoko_feeds_and_speeds/src/fusion.rs` (39KB) — import/export logic, preset regeneration
- Handles both raw JSON and base64-encoded `.tools.zip` archives
- Maps Fusion's tool types to the internal representation

### What to build

**A. Fusion tool library importer**

Parse Fusion 360 `.tools` JSON files and convert to rs_cam `ToolConfig` entries.

Key mappings from `fusion_schema.rs`:
- Fusion `"flat end mill"` → rs_cam `EndMill`
- Fusion `"ball end mill"` → rs_cam `BallNose`
- Fusion `"bull nose end mill"` → rs_cam `BullNose`
- Fusion `"chamfer mill"` / `"engrave"` → rs_cam `VBit`
- Fusion `"tapered mill"` with ball tip → rs_cam `TaperedBallNose`

Extract: diameter, flute count, flute length, shank diameter, overall length, material, coating.

**B. UX: Import wizard**

File → Import Tool Library → select `.tools` file → preview tools → select which to import → add to library.

**Implementation hints:**
- The Fusion JSON format nests tool geometry under `"geometry"` with fields like `"DC"` (diameter), `"LCF"` (flute length), `"NOF"` (flute count)
- The reference's `fusion_schema.rs` already has all the serde structs — adapt rather than reinvent
- Many hobbyists have Fusion 360 tool libraries already — this is a high-value import path

---

## 11. Setup Context & Workholding

### What the reference provides
`SetupContext` (`calcs.rs:79`):
- `tool_overhang_mm` — actual stickout (affects deflection risk)
- `measured_runout_mm` — real-world spindle runout (affects effective chip load)
- `workholding_rigidity` — Low/Medium/High (derates all cuts)
- `coolant_mode` — None/AirBlast/Mist/Flood (affects chip evacuation)

### What to build

**A. Setup panel in viz**

Per-project setup that affects all calculations:
- Workholding rigidity selector: "Tape & glue" (Low), "Clamps" (Medium), "Vacuum table" (High)
- Coolant: None / Air blast / Dust collection — for wood routers, this is really about chip clearing
- Optional: measured spindle runout (advanced users)

**B. Derating logic**

Low workholding → reduce DOC by 20%, reduce feed by 15%
High workholding → allow 10% more aggressive parameters

These multipliers should be tunable in the machine profile.

**Implementation hints:**
- Most wood router users have clamps (Medium) or tape+glue (Low)
- The reference applies workholding as a multiplier on safety_factor — simple and effective
- Don't over-complicate: three tiers is enough

---

## 12. Integration Points with Existing rs_cam Features

### Feed optimization dressup (`feedopt.rs`)

The existing feed optimization already does post-process RCTF adjustment. With the new feeds calculator:
- The initial feed rate comes from the calculator (with chip thinning already considered)
- The feedopt dressup further optimizes per-move based on actual engagement from simulation
- These compose: calculator sets the baseline, feedopt fine-tunes per-move

### Toolpath generation

Every operation's `Params` struct has `feed_rate` and `plunge_rate`. These should:
- Default to `None` (auto-calculated)
- Accept `Some(value)` for user override
- Be resolved at generation time from the feeds calculator

### G-code post-processing

`PostConfig.spindle_speed` should auto-populate from the calculator's RPM recommendation. The existing `emit_gcode_phased()` already handles per-operation spindle speed — wire it up.

### Simulation

The simulation can show power consumption in real-time using `calculate_cutting_power_kw()` with actual engagement from the heightmap. This is a compelling visualization.

---

## 13. Data Flow Summary

```
┌──────────────────────────────────────────────────────────┐
│                      User Inputs                          │
│  Machine Profile  ←  preset or custom                     │
│  Material         ←  stock config                         │
│  Tool             ←  library (with flute count, material) │
│  Operation        ←  toolpath type + pass role            │
│  Setup            ←  workholding, coolant                 │
└──────────────────┬───────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────┐
│               FeedsCalculator::calculate()                │
│                                                           │
│  1. RPM = surface_speed × 1000 / (π × D_eff)            │
│     → clamp to machine spindle range                      │
│  2. ChipLoad = K₀ × D^p × (1/H)^q                      │
│  3. Feed = RPM × ChipLoad × Flutes × RCTF               │
│  4. DOC/WOC from operation matrix × machine rigidity     │
│  5. Power = Kc × DOC × WOC × Feed / 60e6                │
│     → if power > available: reduce feed                   │
│  6. Feed = min(feed, machine_max_feed)                   │
│  7. Plunge = material_plunge_ratio × feed                │
│  8. Apply safety_factor                                   │
│  9. Collect warnings                                      │
└──────────────────┬───────────────────────────────────────┘
                   │
                   ▼
┌──────────────────────────────────────────────────────────┐
│                     FeedsResult                           │
│  rpm, feed, plunge, doc, woc, power, mrr, warnings       │
│                                                           │
│  → Auto-fills operation params (if Auto mode ON)          │
│  → Displayed in summary card                              │
│  → Warnings shown inline                                  │
│  → Overridable per-field by user                         │
└──────────────────────────────────────────────────────────┘
```

---

## 14. File/Module Organization

```
crates/rs_cam_core/src/
  machine.rs          NEW — MachineProfile, SpindleConfig, PowerModel, builder
  material.rs         NEW — Material enum, WoodSpecies, hardness_index, kc
  feeds.rs            NEW — FeedsCalculator, FeedsRequest, FeedsResult, warnings
  feeds_geometry.rs   NEW — effective diameter, chip thinning factors
  tool/mod.rs         MODIFY — add flute_count, tool_material, coating, cut_direction
  feedopt.rs          MODIFY — share RCTF math with feeds_geometry.rs
  toolpath.rs         no change (already has per-move feed rates)
  gcode.rs            no change (already receives spindle_speed)

crates/rs_cam_viz/src/
  state/job.rs        MODIFY — add MachineProfile, Material to JobState/StockConfig
  state/toolpath.rs   MODIFY — add auto/manual flag per parameter
  ui/properties/
    machine.rs        NEW — machine setup panel
    material.rs       NEW — material picker in stock panel
    feeds_card.rs     NEW — feeds summary card in toolpath panel
    tool.rs           MODIFY — add flute count, material, vendor fields
  ui/menu_bar.rs      MODIFY — add Tool Library menu item

reference/shapeoko_feeds_and_speeds/   REFERENCE ONLY — not compiled into rs_cam
```

---

## 15. Testing Strategy

Port tests from `reference/shapeoko_feeds_and_speeds/src/lib.rs` (100+ tests). Key categories:

- **Chip load correctness**: soft wood 6mm 2F → ~0.067mm, hard wood → ~0.031mm (reference test values)
- **Feed rate math**: RPM × chipload × flutes
- **Power calculation**: known Kc × DOC × WOC × feed → expected kW
- **DOC/WOC monotonicity**: roughing DOC > finishing DOC for same tool/material
- **Effective diameter**: ball nose at 1mm DOC with 6mm tool → known D_eff
- **Scallop stepover**: 3mm ball, 0.1mm scallop → known stepover
- **Machine clamping**: calculated RPM outside range → clamped + warning
- **Power limiting**: high engagement → feed reduced + PowerLimited warning
- **Flute guard**: DOC > 0.8 × flute_length → capped + warning
- **Slotting detection**: WOC ≈ D → DOC reduced + warning

Each test should use concrete tool/material/machine combinations with expected output values derived from the reference implementation.

**Implementation hints:**
- The reference's `refresh_golden` binary regenerates golden test data — useful pattern for regression testing
- Test against the reference implementation's output for the same inputs to ensure parity
