# Review: Feeds & Speeds Calculator

## Summary
The feeds & speeds calculator is well-implemented with industry-standard formulas, comprehensive tool-type handling (5 families), and conservative safety defaults appropriate for wood routing. The vendor LUT is fully wired in the GUI (contradicting FEATURE_CATALOG), with 61 Amana observations embedded. Main gaps are zero-flute-count validation and the inability to load custom LUT directories at runtime.

## Findings

### Calculator Correctness
- **RPM formula**: `ideal_rpm = (base_cutting_speed * 1000) / (pi * tool_diameter)` — standard surface-speed formula. Fallback 18000 RPM if diameter <= 0. `feeds/mod.rs:149-162`
- **Chip load**: `K0 * D^p * (1/H)^q` with K0=0.024, p=0.61, q=1.26 — Shapeoko empirical data. Vendor LUT checked first, formula as fallback. `feeds/mod.rs:166-183`
- **Feed rate**: `RPM * ChipLoad * Flutes * ChipThinningFactor * DepthTierMultiplier` with setup derates for L/D ratio and workholding. `feeds/mod.rs:275-299`
- **Power limiting**: `Power = (Kc * DOC * WOC * Feed) / (60e6)` — reduces feed proportionally when power exceeded. Supports VFD constant-torque and router constant-power models. `feeds/mod.rs:301-317`
- **Plunge rate**: Material-dependent fraction scaled by hardness index, 100-2000 mm/min range. `feeds/mod.rs:572-580`
- **Chip thinning**: Combined radial (RCTF) + axial thinning, clamped [1.0, 4.0]. `feeds/geometry.rs:17-31, 107-112`

### Tool Type Handling (5 families)
- **Flat**: Nominal diameter, standard RCTF. `feeds/geometry.rs:554`
- **Ball**: Effective diameter from `2*sqrt(ap*(d-ap))` at shallow depth. Axial chip thinning compensates. `feeds/geometry.rs:37-51`
- **Bull**: Below corner radius treated as ball of diameter 2*corner_r, above uses nominal. `feeds/geometry.rs:75-86`
- **VBit**: Width = `tip_d + 2*ap*tan(angle/2)`. Returns Option, safe fallback via `unwrap_or(nominal_d)`. `feeds/geometry.rs:139-146`
- **TaperedBall**: Local radius = `tip_r + ap*tan(taper_angle)`, clamped [0.01, nominal_d]. `feeds/geometry.rs:56-69`
- **Drill**: No dedicated geometry hint — mapped to Flat logic via Pocket/Roughing operation. Noted in INTEGRATION.md:46.

### Material Library (material.rs)
- **10 solid woods**: GenericSoftwood(600 Janka) through Ipe(3510 Janka) — values within published ranges
- **3 plywood grades**: Softwood(600), BalticBirch(1200), HardwoodFaced(1000) — correct progression
- **3 sheet goods**: MDF(1100), HDF(1300), Particleboard(750) — correct ordering
- **5 plastics**: Generic, Acrylic, HDPE, Delrin, Polycarbonate — all H=0.5, Kc=4.0
- **3 foams**: Low(H=0.15), Medium(H=0.25), High/Renshape(H=0.40)
- **Custom materials**: User-defined hardness_index and Kc. `material.rs:166-170`
- **Hardness scaling**: `(Janka/600)^0.4` — empirical, source not cited in code

### Vendor LUT
- **61 observations** across 5 JSON files (all Amana): flat_end(20), 3d_profiling(14), ball_nose(11), vbit(8), facing(8). `data/vendor_lut/`
- **Evidence grading**: Grade A (vendor chart), B (derived), C (community) with scoring weights. `feeds/vendor_lut.rs:24-60`
- **Normalization**: Material→LUT mapping via Janka threshold (800 lbf for soft/hardwood split). Tool family fallbacks (BullNose<->FlatEnd, TaperedBallNose<->BallNose). Diameter tolerance 0.5x-2.0x ratio. `feeds/vendor_normalize.rs:49-99`, `feeds/vendor_lookup.rs:140-151`
- **14 named sources** in `data/vendor_lut/source_manifest.json`: Amana, Onsrud, Harvey, Whiteside, Sandvik, GARR, Autodesk
- **Only Amana data embedded** — other vendors defined in schema but no embedded observations

### GUI Wiring (FEATURE_CATALOG is outdated)
- Embedded LUT lazy-loaded via `LazyLock` in GUI. `rs_cam_viz/src/ui/properties/mod.rs:13-14`
- `calculate_and_apply_feeds()` passes `vendor_lut: Some(&*VENDOR_LUT)` to FeedsInput. `properties/mod.rs:610`
- Results displayed in collapsible feeds card: RPM, Chip Load, Feed, Plunge, DOC, WOC, Power bar, MRR. `properties/mod.rs:645-737`
- Vendor source ID displayed as raw observation ID (e.g., "amana-flat-softwood-adaptive-6000-2f"). `properties/mod.rs:696-702`
- **FEATURE_CATALOG line 115 is inaccurate**: claims "no GUI entry point yet" but embedded LUT is fully operational
- **What's actually missing**: No UI for loading additional custom LUT directories (`load_dir()` at vendor_lut.rs:197 exists but no GUI call point)

### Machine Profiles (machine.rs)
- **3 presets**: Generic Wood Router (0.8kW, 8-24kRPM), Shapeoko VFD (1.5kW VFD constant-torque), Shapeoko Makita (0.71kW, 6 discrete speeds)
- **Power models**: VFD constant-torque (linear below rated RPM) and constant-power (flat). `machine.rs:15-22`
- **Safety factor**: 0.75-0.80 across presets. `machine.rs:82`
- **Rigidity**: DOC rough 0.20-0.25x diameter, WOC rough 0.70-0.80x, WOC max 6.35mm cap. `machine.rs:44-68`
- **Discrete RPM**: Snaps to nearest speed in list. `machine.rs:172-180`

### Safety Checks
- **Slotting detection**: ae > 0.85D caps DOC to 0.25D, emits `SlottingDetected` warning. `feeds/mod.rs:241-250`
- **Flute guard**: ap capped at 80% of flute_length, emits `DocExceedsFlute` warning. `feeds/mod.rs:221-233`
- **Shank check**: Warns if shank > machine.max_shank_mm. `feeds/mod.rs:252-260`
- **Feed rate clamping**: Warns if feed > machine.max_feed_mm_min. `feeds/mod.rs:320-326`
- **Scallop validation**: For Ball/TaperedBall, validates target_scallop < ball_radius. `feeds/mod.rs:194-211`
- **Minimum engagement**: ap >= 0.05mm, ae >= 0.02mm floors enforced. `feeds/mod.rs:236-237`

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Zero flute count silently produces zero feed rate (no warning) | feeds/mod.rs:280 |
| 2 | Low | FEATURE_CATALOG claims vendor LUT GUI "not wired" — this is outdated; it is fully wired | FEATURE_CATALOG.md:115 |
| 3 | Low | Vendor source shown as raw observation ID, not human-readable name | properties/mod.rs:696-702 |
| 4 | Low | Hardness scaling formula `(Janka/600)^0.4` not cited in code (likely Shapeoko reference) | feeds/mod.rs:166 |
| 5 | Low | Only Amana data embedded in vendor LUT; Onsrud/Harvey/Whiteside in schema but empty | data/vendor_lut/ |
| 6 | Low | No dedicated Drill geometry hint — mapped to Flat via operation type | feeds/mod.rs:28-42 |
| 7 | Low | Negative tool_diameter/flute_length not validated (rely on `.max(0.0)` guards, no user warning) | feeds/mod.rs:158 |

## Test Gaps
- No integration tests with actual GUI state (mocking tool/material/machine configs)
- No tests for `load_dir()` runtime loader (vendor_lut.rs:197-213)
- No tests for vendor source label display in GUI
- No tests for warning message generation during edge cases (slotting, power limit, flute guard)
- No test for zero flute count behavior

## Suggestions
- Add validation warning when flute_count = 0 (currently silently zeroes feed)
- Update FEATURE_CATALOG to reflect that embedded vendor LUT is fully GUI-wired; clarify that custom LUT loading is the unwired part
- Consider displaying vendor name + chart title instead of raw observation ID
- Add citation comment for the hardness scaling formula source
- Add edge-case tests for zero flutes, slotting warning, power-limited feed
