# Vendor LUT GUI Surfacing Plan

What was built in the vendor LUT session and what still needs GUI exposure.

## What Was Implemented (Backend)

### 1. Vendor LUT System (`feeds/vendor_lut.rs`)
- **61 embedded observations** from 5 JSON files (Amana, Onsrud, Whiteside)
- **6 tool families**: FlatEnd, BallNose, TaperedBallNose, BullNose, ChamferVbit, FacingBit
- **12 material families**: Softwood, Hardwood, PlywoodSoftwood, PlywoodHardwood, Mdf, Hdf, Particleboard, Acrylic, Hdpe, Polycarbonate, Delrin, Aluminum
- `VendorLut::embedded()` — loads at startup via `include_str!`
- `VendorLut::load_dir(path)` — loads additional vendor JSON from a directory

### 2. Lookup + Scoring (`feeds/vendor_lookup.rs`)
- `lookup_best(lut, query) -> Option<LookupResult>`
- 8-component scoring: tool family, row kind, evidence grade, flute count, diameter proximity, hardness proximity, subfamily match, pass role
- Returns: chipload midpoint, RPM nominal/min/max, AP/AE ranges, observation ID, vendor name, score

### 3. Type Normalization (`feeds/vendor_normalize.rs`)
- `to_lookup_query(input) -> LookupQuery` maps our Material/ToolGeometryHint to LUT enums

### 4. Pipeline Integration (`feeds/mod.rs`)
- `FeedsInput` gained two new fields:
  - `vendor_lut: Option<&VendorLut>` — pass `Some` to enable LUT lookup
  - `setup: SetupContext` — physical setup derating context
- `SetupContext { tool_overhang_mm: Option<f64>, workholding_rigidity: WorkholdingRigidity }`
- `WorkholdingRigidity` enum: `Low`, `Medium`, `High`
- `FeedsResult` gained: `vendor_source: Option<String>` (observation ID when LUT was used)
- Pipeline changes:
  - Step 2 tries LUT first, falls back to formula
  - Vendor RPM overrides formula RPM when available
  - Step 5b applies L/D and workholding derates before power check

### 5. Current GUI State (Minimal Wiring)
- Global `VENDOR_LUT` via `LazyLock` in `properties/mod.rs`
- `calculate_and_apply_feeds` passes `vendor_lut: Some(&*VENDOR_LUT)` and `setup: { tool_overhang_mm: Some(tool.stickout), workholding_rigidity: Medium }`
- Feeds card shows `"Source: {observation_id}"` label when LUT was used

## What Needs GUI Work

### A. Workholding Rigidity Selector
**Where:** Machine/setup section of properties panel (or a new "Setup" collapsible section)
**What:** ComboBox or radio buttons for Low/Medium/High
**Backend:** `WorkholdingRigidity` enum already exists, currently hardcoded to `Medium`
**State:** Needs a `workholding_rigidity: WorkholdingRigidity` field on the job/project state (not per-toolpath — it's a machine setup property)

### B. Tool Overhang Display
**Where:** Tool properties panel, near the existing stickout DragValue
**What:** Show the computed L/D ratio and whether a derate is active
**Backend:** `tool.stickout / tool.diameter` gives L/D. Derates kick in at >4.0 (12%) and >6.0 (25%)
**Example:** "L/D: 5.2 (feed -12%)" in small text below stickout

### C. Vendor Source in Feeds Card (Already Done — Polish)
**Where:** Feeds & Speeds collapsing section
**What currently shows:** `"Source: amana-flat-softwood-adaptive-6000-2f"` (raw observation ID)
**Polish needed:**
- Format the observation ID more readably, e.g. "Amana 6mm Flat, Softwood Adaptive" instead of the raw ID
- Maybe show "Formula" when vendor_source is None to make it clear which path was used
- Tooltip on source label showing the full observation details (chipload range, RPM range, evidence grade)

### D. Vendor LUT Loading UI (Optional / Future)
**Where:** File menu or a Settings panel
**What:** "Load Additional Vendor Data..." button that opens a directory picker
**Backend:** `VendorLut::load_dir()` already works. Need to make the global LUT mutable or use a `RwLock`
**Priority:** Low — embedded data covers most common scenarios. User-supplied data is for power users

### E. Feeds Card Enhancements
**Where:** The existing `draw_feeds_card` function
**Possible additions:**
- Show whether chipload came from LUT or formula (indicator icon or color)
- Show vendor RPM vs formula RPM when they differ
- Show setup derate breakdown: "Feed derated: L/D 5.2 (-12%), Workholding: Medium"
- Show the LUT match score or confidence level (e.g., score 1800/2000 → "High confidence")

### F. Setup Derates Summary
**Where:** Could be in feeds card, or a separate "Setup" section
**What:** Show active derates and their combined effect
**Example:**
```
Setup Derates:
  L/D ratio: 6.7 → -25% feed
  Workholding: Low → -15% feed
  Combined: -36.25% feed
```

## Data Files

Embedded data lives at `crates/rs_cam_core/data/vendor_lut/`:
- `source_manifest.json` — 13 vendor sources with URLs
- `observations/amana_flat_end.json` — 20 rows
- `observations/amana_ball_nose.json` — 11 rows
- `observations/amana_3d_profiling.json` — 14 rows (tapered ball + bull nose)
- `observations/amana_vbit.json` — 8 rows (V-bit + chamfer, incl. Whiteside)
- `observations/amana_facing.json` — 8 rows (facing/surfacing, incl. Whiteside)

Additional vendor data (Whiteside, Carbide3D, Onsrud) can be added by creating new JSON files following the same schema and using `VendorLut::load_dir()`.
