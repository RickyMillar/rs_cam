# Review: Dressups (Toolpath Modifiers)

## Summary

The dressup system uses pure function composition (not a trait framework) — each modifier is a standalone function taking a `Toolpath` and returning a new `Toolpath`. Application order is hardcoded in `helpers.rs:47-304` in a correct sequence but **undocumented**. All 9 modifiers are well-implemented with zero `unwrap()` in production code and 51 total tests. Edge cases (empty toolpaths, degenerate geometry, missing stock data) are handled gracefully throughout.

## Findings

### Framework

- **Design pattern**: Pure function composition, not trait objects (`dressup.rs:1-30`)
- Each dressup: `fn apply_*(toolpath: &Toolpath, params...) -> Toolpath`
- Immutability enforced by design — each step produces a new toolpath
- **Application order** (hardcoded in `helpers.rs:47-304`):
  1. Entry styles (ramp/helix) — replaces plunges (lines 57-123)
  2. Dogbones — adds overcuts at corners (lines 124-147)
  3. Lead-in/out — arc approaches at pass boundaries (lines 148-171)
  4. Link moves — consolidates nearby passes (lines 172-203)
  5. Arc fitting — compresses linear segments to G2/G3 (lines 204-227)
  6. Feed optimization — adjusts rates based on engagement (lines 228-278)
  7. Rapid ordering — TSP reorders segments (lines 279-302)
- Order is **correct** (entry detects plunges first, link moves after entry, arc fitting late, TSP last) but **not documented or asserted**
- Each step is conditional based on `DressupConfig` flags
- Semantic tracing integration: each step reports to trace for debugging

### Individual Modifiers

**Ramp Entry** (`dressup.rs:31-76, 107-144`)
- Replaces vertical plunges with ramped descent at configurable angle
- Splits ramp into forward-down and back-to-plunge segments
- Plunge detection: Z drop > 0.1mm, XY < 0.01mm (`is_plunge()` line 85)
- If no XY direction found ahead, defaults to (1.0, 0.0) — safe fallback
- 3 tests

**Helix Entry** (`dressup.rs:31-76, 146-193`)
- Replaces plunges with helical spiral descent around plunge point
- Parameterized by radius, pitch, N points per revolution
- Degeneracy handling: dz < 0.01 or pitch < 0.01 → falls back to straight feed (lines 163-166)
- Returns to center at final Z (line 192)
- 3 tests

**Dogbone Overcuts** (`dressup.rs:537-625`)
- Inserts overcuts at inside corners sharper than `max_angle_deg`
- Corner detection via 3-move window of consecutive linear moves at same Z
- Bisector direction computed from normalized edge vectors
- Overcut distance = tool_radius exactly
- Handles: short toolpaths (< 3 moves), zero-length edges, collinear points
- 4 tests

**Lead-in/Lead-out** (`dressup.rs:406-523`)
- Quarter-circle arcs at start/end of cutting passes for tangential approach/departure
- Arc constructed parametrically over 8 fixed steps
- Lead-in: detects plunge, looks ahead for XY direction, offsets perpendicular + backward
- Lead-out: detects pass exit (linear → rapid up > 1.0mm), arcs away tangentially
- 2 tests (lowest coverage of any modifier)

**Tabs / Bridges** (`dressup.rs:218-392`)
- Lifts toolpath at tab positions using cumulative arc-length parameterization
- Tab zones defined as (start_dist, end_dist, tab_z) along cutting perimeter
- State machine tracks `in_tab` flag for step-up/step-down emissions
- Handles: empty tabs, < 2 cutting moves, zero-length paths
- Feed rate fallback: defaults to 1000 mm/min if no linear move found (line 297)
- 6 tests

**Link Moves** (`dressup.rs:651-727`)
- Replaces short retract→rapid→plunge sequences with direct feed moves
- Safety: never links before first cut (lines 665-674)
- Checks same Z before/after (within 0.1mm) and XY distance < max_link_distance
- Uses configured link feed rate, not plunge rate
- 5 tests

**Arc Fitting** (`arcfit.rs:16-127`)
- Greedy longest-run fitting: least-squares (≥5 pts, Kåsa's algebraic method) or 3-point circle fit
- Tolerance-validated: all intermediate points must be within tolerance of fitted circle
- Z-level filtering: only fits arcs at constant Z (within tolerance)
- CW/CCW via cross product of first/last segments
- Degenerate rejection: radius > 1e6, collinear points
- Passes through existing arcs unchanged (idempotent)
- 14 tests (best coverage)

**Feed Optimization** (`feedopt.rs:91-156`)
- Adjusts feed rates based on material engagement using RCTF (Radial Chip Thinning Factor)
- 3-pass: engagement estimation (24 circumference samples per move) → RCTF adjustment → smoothing (forward/backward ramp limiting)
- Air-cut threshold: < 5% engagement → max feed
- Heightmap out-of-bounds: gracefully skips sample points
- Graceful skip if heightmap unavailable (`helpers.rs:263-268`)
- 7 tests

**Rapid Order Optimization** (`tsp.rs:94-199`)
- Nearest-neighbor heuristic + 2-opt improvement (up to 100 iterations)
- Splits toolpath into segments (consecutive cutting moves), discards original rapids
- Reassembles with retract/rapid/plunge between reordered segments at safe_z
- 6 tests

### Application Pipeline (GUI)

- **Config**: `DressupConfig` struct in `state/toolpath/support.rs:164-208`
- **UI**: `properties/mod.rs:2485-2674` — `draw_dressup_params()` exposes all toggles and params
- **Wiring**: `ToolpathEntry.dressups` → `ComputeRequest.dressups` → `apply_dressups()` in worker
- **Entry point**: `execute.rs:284-289` — called after operation generation, before boundary clipping
- **Feed optimization gating**: Disabled at UI level for incompatible ops (RestMachining, FromRemainingStock, 3D mesh ops) with hover text explanation (`catalog.rs:717-737`)
- **No other incompatibility checks** — system assumes all combinations are valid

### Edge Cases — All Handled

| Edge Case | Status | Details |
|-----------|--------|---------|
| Empty toolpaths | ✓ All modifiers | Each checks for empty/too-short input |
| Arc fitting on existing arcs | ✓ | Passes through unchanged (arcfit.rs:30) |
| Tabs on short contours | ✓ | Clamps start/end to bounds (dressup.rs:266-267) |
| Feed optimization without stock | ✓ | Gracefully skips with warning (helpers.rs:263-268) |
| Zero-length edges (dogbone) | ✓ | Skipped (dressup.rs:574-576) |
| Collinear points (dogbone) | ✓ | Skipped (dressup.rs:598-600) |
| Zero pitch helix | ✓ | Falls back to linear (dressup.rs:163-166) |

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | Dressup application order is undocumented — a maintainer could silently break the pipeline by reordering | `helpers.rs:47-304` |
| 2 | Low | Lead-in/out arc resolution hardcoded to 8 steps — coarse for large radii | `dressup.rs:453, 502` |
| 3 | Low | `max_angle_deg` parameter name for dogbones is misleading — it's the threshold above which corners DO get dogbones | `dressup.rs:537` |
| 4 | Low | Tab processing: if first cutting move starts inside a tab zone, Z is not adjusted to tab height | `dressup.rs:288-292` |
| 5 | Low | Tab feed rate fallback of 1000 mm/min is silent — no warning if triggered | `dressup.rs:297` |

## Test Gaps

- **Lead-in/out**: Only 2 tests — lowest coverage of any modifier. No test for large radii or degenerate directions.
- **Tabs on first move inside tab zone**: Not tested.
- **Dressup composition**: No integration test applying all dressups in sequence to validate ordering correctness.

## Suggestions

- Add a block comment at the top of `apply_dressups()` documenting the application order and why it matters
- Add 2-3 more lead-in/out tests covering edge cases
- Consider adding an integration test that applies all dressups to a representative toolpath
