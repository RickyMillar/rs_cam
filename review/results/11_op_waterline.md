# Review: Waterline Operation

## Summary

The waterline operation generates closed contour toolpaths at constant Z heights using push-cutter contact detection and marching-squares contour extraction. The implementation consists of three core modules: `waterline.rs` (Z-slicing and toolpath assembly), `contour_extract.rs` (topological contour extraction via marching squares), and `pushcutter.rs` (horizontal fiber-mesh contact detection). The algorithm is fully end-to-end wired in both CLI and GUI. Marching squares replaces the earlier nearest-neighbor chaining, providing topologically correct contour extraction.

## Findings

### Algorithm Design

- **Z-level selection**: Waterline correctly steps downward from `start_z` to `final_z` in `z_step` increments (waterline.rs:125-154). The loop uses `z >= final_z - 1e-10` to handle floating-point precision.
- **Contour extraction method**: Uses **push-cutter contact detection** on fiber grids, not mesh slicing. At each Z level, two orthogonal fiber sets (X-fibers and Y-fibers) are generated spanning the expanded mesh bounding box (waterline.rs:176-190). Push-cutter runs on both fiber sets in parallel (waterline.rs:192-193).
- **Topological correctness**: Contour extraction uses **marching squares on a boolean grid** (contour_extract.rs:28-52). The boolean grid cell `(row, col)` is "inside" (blocked) if both the X-fiber at that row and Y-fiber at that column are blocked at their intersection. This produces topologically correct, non-crossing contours with correct loop nesting.
- **Saddle case handling**: Marching squares handles ambiguous configurations (cases 5 and 10, contour_extract.rs:144-151, 163-169) by emitting two segments. Comment notes "Disambiguate by center value" but the actual implementation uses a fixed split without center evaluation.
- **Tool offset**: Cutter radius is applied during push-cutter contact detection. Fibers are expanded by cutter radius when creating the fiber grid (waterline.rs:47-50).
- **Linking between Z levels**: Each Z level is fully retracted to `safe_z` after completing all contours (waterline.rs:150-151), then rapids to the next contour's entry point. **No ramping or continuous linking between levels** — full retract/rapids between each Z level.

### Contour Extraction

- **Marching squares grid**: Built from fiber interval intersections (contour_extract.rs:55-80). The implementation correctly tests both X and Y fiber blocking status at each grid cell intersection.
- **Edge point positioning**: Contour segment endpoints are placed at **exact interval boundary positions** (not approximated to cell centers). `edge_point_x` and `edge_point_y` (contour_extract.rs:194-231) search for actual interval endpoints within the cell's boundary range.
- **Fallback logic**: If no exact interval endpoint is found, the code falls back to the cell edge midpoint (contour_extract.rs:250-251, 270-271). This is reasonable for rare boundary cases.
- **Segment chaining**: Chain segments into closed loops via endpoint matching with epsilon tolerance `1e-6` (contour_extract.rs:273-340). Loop closure is detected when tail returns within epsilon of head (contour_extract.rs:303). Loops shorter than 3 points are discarded (contour_extract.rs:334).
- **Multiple loop support**: The algorithm correctly handles multiple disconnected loops and nested (island) contours at a single Z level via the segment chaining algorithm.

### Edge Case Handling

- **Flat areas**: When no contour change occurs between successive Z levels, `waterline_contours` returns empty contours. The toolpath gracefully skips empty contour sets (waterline.rs:131-134).
- **Overhangs/undercuts**: Push-cutter only detects upward-facing contacts at constant Z via fiber-mesh intersection. Downward-facing surfaces (undercuts) are **not captured** since fibers at a given Z cannot contact geometry below that Z. This is implicit in the algorithm design; no explicit check exists.
- **Very steep walls**: Many Z levels with minimal contour change between levels. Each level incurs a full retract cycle, which is expected behavior.
- **Islands at certain Z levels**: Correctly handled by the marching-squares grid and segment chaining, which produce multiple distinct loops per Z level.
- **Empty contour regions**: Tests confirm that Z levels above or far below the mesh produce empty contour sets. Contours with fewer than 3 points are silently skipped (waterline.rs:132).
- **Fiber grid discretization**: Fiber count is computed as `((max - min) / sampling).ceil() + 1` (waterline.rs:176, 184). For very fine sampling or large parts, this could generate large arrays; no explicit limits exist.

### Integration (CLI/GUI)

- **CLI**: Waterline is fully wired as a subcommand `waterline` (CLI main.rs:628-680). Takes input STL, tool specification (type:diameter), and parameters (z_step, sampling, feed/plunge rates, safe_z, arc tolerance). Output is GCode via `export_gcode`.
- **GUI**: Waterline appears as `OperationConfig::Waterline` in the worker execution path (execute.rs:46, 737-765). Integrated as `SemanticToolpathOp` trait implementation (execute.rs:2151-2229), supporting cancellation, debug tracing, and semantic phase tracking.
- **WaterlineConfig struct** (GUI configs.rs): Contains `z_step`, `sampling`, `start_z`, `final_z`, `feed_rate`, `plunge_rate`, and a `continuous` boolean flag. The `continuous` flag is stored but **not implemented in the core algorithm** (waterline.rs uses only start_z, final_z, z_step).

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | `unwrap()` in library chaining code on non-empty chain — safe but violates "avoid unwrap()" guideline | waterline.rs:223, 252 |
| 2 | Medium | Saddle case (MS cases 5 & 10) uses fixed split; comment says "disambiguate by center value" but no center evaluation is performed | contour_extract.rs:144-151, 163-169 |
| 3 | Low | `continuous` flag in WaterlineConfig is stored but never used; algorithm always retracts between Z levels | configs.rs; waterline.rs:150-151 |
| 4 | Low | No explicit validation that z_step > 0 or that start_z >= final_z; silently produces empty toolpath if misconfigured | waterline.rs:125-126 |
| 5 | Low | Fiber grid generation has no upper bound check; extremely fine sampling or large parts could exhaust memory | waterline.rs:176, 184 |
| 6 | Medium | No explicit downward-facing (undercut) detection; algorithm relies on implicit Z constraint, which may confuse users expecting full 3D coverage | waterline.rs |

## Test Gaps

- No end-to-end waterline tests in `crates/rs_cam_core/tests/end_to_end.rs`
- No complex terrain tests (current tests use only simple hemisphere and grid geometries; no real-world STL data)
- No saddle case unit tests for marching squares (cases 5 & 10 are critical for correct topology but have no dedicated test)
- No parameterization tests (e.g., boundary cases for z_step, sampling very small/large)
- No island/nesting tests (multiple disconnected loops and nested contours not explicitly validated)
- No undercut behavior tests (whether undercuts are intentionally excluded should be documented with a test)

## Suggestions

1. **Replace unwrap() calls** (waterline.rs:223, 252) with safe alternatives like `.ok_or()`
2. **Implement or document saddle disambiguation**: Add a comment explaining why the current split is chosen, or implement the center-value test mentioned in the comment
3. **Remove or implement the `continuous` flag**: Either remove it from WaterlineConfig if not planned, or implement continuous linking (ramp/helix between Z levels instead of retract)
4. **Add parameter validation** in waterline_toolpath_with_cancel for z_step > 0 and start_z >= final_z
5. **Document undercut behavior** explicitly in waterline.rs module docs — waterline is for upward-facing surfaces only
6. **Add fiber grid size validation** to warn or fail gracefully if the fiber grid would be excessively large (e.g., > 100K fibers)
7. **Add unit tests** for marching squares saddle cases, multiple disjoint contours, nested contours, and extreme parameter values
