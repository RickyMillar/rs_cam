# Review: Feeds & Speeds Calculator

## Scope
The feeds and speeds calculation system — material library, vendor LUTs, chip load estimation.

## Files to examine
- `crates/rs_cam_core/src/feeds/mod.rs` (2467 LOC)
- `crates/rs_cam_core/src/feeds/geometry.rs` (tool engagement geometry)
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs`
- `crates/rs_cam_core/src/feeds/vendor_lut.rs` (embedded data)
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs`
- `crates/rs_cam_core/src/machine.rs` (machine profiles)
- `crates/rs_cam_core/src/material.rs` (material properties)
- Research: `research/feeds_and_speeds_integration_plan.md`
- Reference data: `reference/` directory (Shapeoko data)
- GUI: how feeds are displayed/edited in properties

## What to review

### Calculator correctness
- Input: tool type, diameter, flutes, material, machine rigidity, engagement
- Output: RPM, feed rate, plunge rate, chip load
- Are the formulas industry-standard? Check against CREDITS.md sources.
- Does it handle all 5 tool types differently?

### Material library
- What materials are supported? Are properties reasonable for wood routing?
- Hardwood vs softwood vs plywood vs MDF

### Vendor LUT
- What data is embedded? Source? Quality?
- Normalization: how are vendor observations mapped to the calculator?
- FEATURE_CATALOG says vendor LUT GUI is not wired — verify

### Machine profiles
- Rigidity model: Soft/Medium/Hard
- GUI hardcodes "Medium" — implications?
- Spindle speed limits, feed rate limits

### Edge cases
- Very small or very large tools
- Exotic materials
- Zero flute count
- Engagement area = 0

### Testing & code quality

## Output
Write findings to `review/results/17_feeds_speeds.md`.
