# Implementation Plan

This is the active engineering backlog for bringing the current product surface to a cleaner open-source baseline.

## Priority 1: Export and persistence completeness

### 1. Manual G-code injection

- emit `pre_gcode` and `post_gcode` during G-code export
- document ordering rules around tool changes and post headers

### 2. Project round-tripping

- serialize full per-operation parameter state
- serialize dressups, heights, stock-source mode, and feeds auto/manual toggles
- document any intentionally non-persistent fields

### 3. Controller-side compensation

- wire profile “In Control” mode to `G41` / `G42` / `G40`
- document machine assumptions and controller compatibility

## Priority 2: Worker/UI wiring gaps

### 4. Drop-cutter option wiring

- connect `skip_air_cuts`
- connect drop-cutter slope confinement
- update docs once the options are real

### 5. Waterline continuity

- either wire the `continuous` toggle or remove it from state/UI until ready

### 6. Stock-source semantics

- use `StockSource` to alter simulation/compute behavior where intended
- document the exact behavior once shipped

## Priority 3: Verification and feedback

### 7. Rapid-collision display

- render rapid-collision results in the viewport
- link collision reports back to specific toolpaths

### 8. Simulation deviation coloring

- feed deviation data into the renderer
- expose remaining-stock / overcut views in the UI

## Priority 4: Feeds/speeds polish

### 9. Setup context UI

- expose workholding rigidity
- expose overhang / L:D ratio feedback
- polish vendor-source labels

### 10. Vendor LUT management

- add a GUI entry point for loading additional LUT data
- surface active-source and derate breakdowns more clearly

## Priority 5: CLI parity review

- decide which GUI-only operations should gain direct CLI commands
- keep the CLI intentionally smaller where desktop-only workflows make more sense

When a priority is completed, fold it into `FEATURE_CATALOG.md` and remove or demote it here.
