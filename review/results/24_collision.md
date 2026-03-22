# Review: Collision Detection

## Summary
Holder/shank collision detection is well-implemented using a drop-cutter approach at holder segment radii against the raw input mesh. The algorithm supports both endpoint-only and interpolated (1mm step) checking, with proper cancellation support. Collision results are rendered both in the viewport (red cross markers) and timeline (red ticks) — correcting the FEATURE_CATALOG note which refers to rapid collisions, not holder collisions. No `unwrap()` in production code, 10 tests covering geometry, functional detection, and rapid collisions.

## Findings

### Algorithm
- **Approach** (`collision.rs:1-9`): models holder/shank as cylindrical/tapered segments, uses drop-cutter at each segment's radius to detect mesh surface contact above the allowed Z
- **Holder geometry** (`collision.rs:17-38`): `HolderSegment` supports cylindrical and tapered (conical) sections. Collision checking conservatively uses `max_radius()` per segment to avoid false negatives
- **Assembly representation** (`collision.rs:40-101`): `ToolAssembly` describes cutter + shank + holder. `segments()` returns `(z_offset, max_radius, length)` tuples ordered from tip upward
- **Detection loop** (`collision.rs:179-274`): for each cutting move, generates sample points, then for each point and each holder segment: creates a virtual flat endmill at segment radius, calls `point_drop_cutter()`, compares mesh surface Z against holder bottom Z. Collision flagged if penetration > 0.01mm
- **Segment filtering** (`collision.rs:233`): skips segments with radius ≤ cutter_radius + 1e-6

### Interpolation
- **Default wrapper** (`collision.rs:142-149`): `check_collisions()` calls interpolated variant with `step_mm=0.0` (endpoint-only, legacy)
- **Interpolated variant** (`collision.rs:151-160, 210-228`): divides move into `ceil(distance / step_mm)` segments, generates linearly interpolated sample points
- **GUI usage** (`compute/worker/helpers.rs:393`): worker uses `step_mm=1.0` (sample every 1mm)
- **Test validation** (`collision.rs:431-461`): `test_interpolated_catches_mid_move()` confirms interpolation catches collisions missed by endpoint checking

### Geometry Checked Against
- **Raw input mesh** (`req.mesh: Arc<TriangleMesh>`), NOT simulation result mesh
- Spatial index built from mesh for efficient triangle lookups (`collision.rs:378`)
- Deliberate architectural choice: collision check doesn't require simulation to have run

### Accuracy
- **False positives**: 0.01mm penetration threshold prevents floating-point noise (`collision.rs:247`). Conservative max_radius on tapered segments can over-estimate collision risk slightly
- **False negatives**: 1mm fixed sampling step could miss collisions on very sharp tool paths (tight spirals <1mm radius). Endpoint-only mode (step_mm=0) misses all mid-move collisions
- **Both thresholds hardcoded**: penetration threshold (0.01mm) and sample spacing (1mm) are not user-configurable

### Integration
- **Separate compute lane** (`controller/events.rs:653-682`): `RunCollisionCheck` handled via dedicated `CollisionRequest` on analysis lane, independent from simulation
- **Why separate from simulation**: different input (raw mesh, not simulated stock), doesn't require simulation results, can run independently (cheaper than full stock simulation)
- **Result storage** (`state/simulation.rs:248-252`): stored in `SimulationChecks` alongside rapid collision data

### Visualization
- **Viewport** (`app.rs:988-1027`): red cross markers at 3D collision positions, rendered via dedicated vertex buffer (`render/mod.rs:702-708`)
- **Timeline** (`ui/sim_timeline.rs:180-188`): red tick marks at holder collision move indices
- **Status bar** (`app.rs:1789, 1829`): collision count displayed
- **Diagnostics panel** (`ui/sim_diagnostics.rs:363-372`): holder collision status and run button

### FEATURE_CATALOG Clarification
- Line 100: "holder/shank collision checks" — confirmed implemented
- Line 113: "Rapid collision rendering" caveat — refers to **rapid collisions**, NOT holder collisions. Holder collision rendering IS implemented (viewport markers + timeline ticks)

### Two Collision Systems
1. **Holder collisions** (this review): dedicated check via `RunCollisionCheck`, checks holder/shank geometry against mesh
2. **Rapid collisions**: detected during simulation, checks rapid moves against stock bounds — separate system

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | 1mm sample spacing hardcoded in GUI worker — not user-configurable, could miss collisions on very short/tight moves | `helpers.rs:393` |
| 2 | Low | 0.01mm penetration threshold hardcoded — may be too lenient for precision work or too strict for rough machining | `collision.rs:247` |
| 3 | Low | Conservative max_radius on tapered segments can slightly over-report collisions on tapered holders | `collision.rs:35-37` |
| 4 | Low | Pure cylindrical/tapered model — no consideration for stepped collets or complex holder geometry | `collision.rs:17-38` |
| 5 | Low | Checks against raw input mesh, not simulated stock — may flag collisions where stock has already been removed by prior operations | `collision.rs:179` |

## Test Gaps
- No test for tapered holder collision detection (only tests tapered segment geometry construction)
- No test for multi-segment holder (e.g., shank + holder with different radii)
- No test for collision near mesh boundary or degenerate triangles
- No performance test with large meshes
- No test for cancellation behavior during collision check

## Suggestions
- Consider making sample spacing (step_mm) configurable from the UI for users who need finer resolution
- Consider making penetration threshold configurable or at least document the 0.01mm value in the diagnostics panel
- Add test for multi-segment tool assembly (shank + holder) collision detection
- For multi-setup workflows, consider optionally checking against simulated stock instead of raw mesh to reduce false positives from already-machined areas
- Document in FEATURE_CATALOG that holder collision visualization IS implemented (viewport + timeline), and clarify that the "not rendered" note applies only to rapid collisions
