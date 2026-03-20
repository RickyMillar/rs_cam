# Feeds & Speeds Integration Reference

How every toolpath operation connects to the feeds calculator, and how to add a new one.

## Architecture Overview

```
ToolConfig + Material + MachineProfile
         |
         v
  FeedsInput { tool_diameter, flute_count, flute_length, shank_diameter,
               tool_geometry, material, machine, operation, pass_role,
               axial_depth_mm, radial_width_mm, target_scallop_mm }
         |
         v
  feeds::calculate()  — 10-step pipeline, sub-ms
         |
         v
  FeedsResult { rpm, chip_load_mm, feed_rate_mm_min, plunge_rate_mm_min,
                ramp_feed_mm_min, axial_depth_mm, radial_width_mm,
                power_kw, available_power_kw, warnings, ... }
         |
         v
  Auto-write into OperationConfig (gated by FeedsAutoMode per-field flags)
```

## Current Operation Mapping (14 operations)

| Operation | OperationFamily | PassRole | Hints Wired | Feed Fields Auto-Written |
|-----------|----------------|----------|-------------|--------------------------|
| **Pocket** | Pocket | Roughing | — | feed, plunge, stepover, depth_per_pass |
| **Profile** | Contour | Roughing | — | feed, plunge, depth_per_pass |
| **Adaptive** | Adaptive | Roughing | — | feed, plunge, stepover, depth_per_pass |
| **VCarve** | Trace | Finish | max_depth → axial | feed, plunge, stepover |
| **Rest** | Pocket | Roughing | — | feed, plunge, stepover, depth_per_pass |
| **Inlay** | Trace | Finish | — | feed, plunge, stepover |
| **Zigzag** | Pocket | Roughing | — | feed, plunge, stepover, depth_per_pass |
| **DropCutter** | Parallel | Finish | — | feed, plunge, stepover |
| **Adaptive3d** | Adaptive | Roughing | — | feed, plunge, stepover, depth_per_pass |
| **Waterline** | Contour | SemiFinish | z_step → axial | feed, plunge |
| **Pencil** | Trace | Finish | — | feed, plunge |
| **Scallop** | Scallop | Finish | scallop_height → target_scallop | feed, plunge |
| **SteepShallow** | Contour | Finish | z_step → axial | feed, plunge, stepover |
| **RampFinish** | Parallel | Finish | max_stepdown → axial | feed, plunge |

## OperationFamily Default Profiles (DOC/WOC as factor × tool diameter)

| Family | Roughing | SemiFinish | Finish |
|--------|----------|------------|--------|
| **Adaptive** | 1.50 ap, 0.12 ae | 0.90 ap, 0.10 ae | 0.70 ap, 0.08 ae |
| **Pocket** | 0.70 ap, 0.35 ae | 0.35 ap, 0.20 ae | 0.20 ap, 0.08 ae |
| **Contour** | 0.80 ap, 0.18 ae | 0.45 ap, 0.10 ae | 0.30 ap, 0.05 ae |
| **Parallel** | 0.25 ap, 0.08 ae | 0.16 ap, 0.05 ae | 0.10 ap, 0.03 ae |
| **Scallop** | 0.20 ap, 0.07 ae | 0.14 ap, 0.05 ae | 0.08 ap, 0.025 ae |
| **Trace** | 0.15 ap, 0.05 ae | 0.10 ap, 0.03 ae | 0.06 ap, 0.02 ae |
| **Face** | 0.08 ap, 0.65 ae | 0.06 ap, 0.55 ae | 0.04 ap, 0.45 ae |

**Adaptive overrides** per tool geometry (roughing):
- Flat/BullNose: 1.20 ap, 0.14 ae
- Ball: 0.80 ap, 0.10 ae
- TaperedBall: 0.70 ap, 0.08 ae

## Calculation Pipeline (10 steps)

1. **RPM**: surface speed / (pi * D) → clamp to machine spindle range
2. **Chip load**: K0 * D^p * (1/H)^q (empirical formula)
3. **DOC/WOC**: operation matrix × tool geometry overrides × machine rigidity
4. **Scallop override**: if target_scallop_mm provided, compute stepover from ball radius
5. **User overrides**: apply any explicit axial_depth_mm / radial_width_mm
6. **Flute guard**: cap DOC to 0.8 × flute_length
7. **Slotting**: if WOC > 85% of D, reduce DOC to 0.25 × D
8. **Feed**: RPM × chipload × flutes × RCTF × axial_thinning × depth_tier
9. **Power check**: Kc × DOC × WOC × feed / 60e6 — reduce feed if over limit
10. **Clamp + safety**: machine max feed → safety factor → plunge rate

## How to Add a New Toolpath Operation

### Step 1: Define the config struct (`state/toolpath.rs`)

```rust
pub struct MyNewConfig {
    pub stepover: f64,
    pub depth_per_pass: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    // ... operation-specific params
}
```

Include `feed_rate` and `plunge_rate` always. Include `stepover` and `depth_per_pass`
if the operation uses them — this enables auto-write from the feeds calculator.

### Step 2: Add the variant to `OperationConfig`

```rust
pub enum OperationConfig {
    // ...
    MyNew(MyNewConfig),
}
```

### Step 3: Extend get/set helpers

Add arms to `feed_rate()`, `set_feed_rate()`, `plunge_rate()`, `set_plunge_rate()`.
If it has stepover: add to `stepover()`, `set_stepover()`.
If it has depth_per_pass: add to `depth_per_pass()`, `set_depth_per_pass()`.

### Step 4: Map to feeds family (`ui/properties/mod.rs`)

In `operation_to_feeds_family()`, add:

```rust
OperationConfig::MyNew(_) => (OF::Pocket, PR::Roughing), // pick appropriate family + role
```

**How to choose:**
- **Roughing ops** (bulk removal): Pocket, Adaptive
- **Finishing ops** (surface quality): Parallel, Scallop, Contour(Finish), Trace
- **High engagement**: Pocket (wide + moderate DOC)
- **Constant engagement**: Adaptive (deep + narrow WOC)
- **Surface following**: Parallel (shallow + narrow)
- **Scallop control**: Scallop (very fine, ball tool specific)
- **Wall following**: Contour (moderate depth, narrow WOC)
- **Engraving/tracing**: Trace (very shallow + narrow)
- **Surfacing**: Face (very shallow + very wide)

### Step 5: Wire operation-specific hints

In `operation_feeds_hints()`, extract any params that should constrain the calculation:

```rust
OperationConfig::MyNew(cfg) => (Some(cfg.depth_per_pass), None, None),
```

- `axial_depth_mm`: pass z_step, max_depth, max_stepdown, depth_per_pass — whatever
  constrains the axial engagement
- `radial_width_mm`: pass if the operation has a fixed WOC (rare)
- `target_scallop_mm`: pass scallop_height for ball tool finishing ops

### Step 6: Draw parameters in the UI

Add `draw_mynew_params()` in `properties/mod.rs` using the `dv()` helper for each field.
The feeds auto-write will handle populating feed_rate, plunge_rate, stepover, depth_per_pass
automatically when the user has auto mode enabled.

### Step 7: Add to compute worker

Wire the new operation in `compute/worker.rs` to call the appropriate rs_cam_core function.

## What the Calculator Does NOT Handle (by design)

These are left to the user or operation-specific logic:

- **Total depth** (Pocket depth, Profile depth): set by part geometry, not feeds
- **Tolerance**: geometric accuracy, not a feeds parameter
- **Direction** (climb/conventional): affects surface quality, not feeds math
- **Entry style** (plunge/ramp/helix): the ramp_feed_mm_min covers ramp entry
- **Dressups** (tabs, dogbone, lead-in): post-processing, no feeds impact
- **Stock source** (fresh vs remaining): affects material state, not feed rate
- **Specific spindle speed**: calculator computes optimal RPM; post-processor uses
  the global spindle_speed from PostConfig (auto-write not yet wired)

## Known Limitations vs Reference Implementation

Features in `reference/shapeoko_feeds_and_speeds/` not yet ported:

| Feature | Impact | Complexity |
|---------|--------|-----------|
| Vendor LUT chipload seeding | Real-world accuracy from manufacturer data | Very High |
| Setup derates (overhang L/D, runout, workholding) | Machine-specific safety | Medium |
| Material condition (moisture, abrasiveness) | Exotic material accuracy | Low |
| Entry mode angle derates | Ramp/helix entry feed tuning | Medium |
| High-feed mode & feed-opt controls | Advanced linking parameters | Medium |
| Evidence/provenance tracing | Audit trail for calculated values | Low |

These can be added incrementally without changing the core architecture.
