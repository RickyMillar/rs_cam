# Review: Inlay Operation

## Summary

The inlay operation generates matching male and female V-carved toolpaths for wood inlays. The algorithm is functional with correct V-bit geometry integration, proper glue gap support, and zero `unwrap()` calls. Key issues: the male region construction ignores polygon holes (line 131), the GUI merges male and female into a single toolpath instead of keeping them separate (unlike CLI which produces two files), and the `flat_depth` parameter creates an undocumented asymmetry between male and female depths.

## Findings

### Algorithm Correctness

**Female pocket** (`inlay.rs:54-99`):
- Delegates to `vcarve_toolpath()` with the polygon, half-angle, and pocket depth
- Depth at any point = `distance_to_boundary / tan(half_angle)`, clamped to `pocket_depth`
- Optional flat-bottom clearing for the wide interior (inset by `pocket_depth * tan(half_angle)`, line 77)

**Male plug** (`inlay.rs:101-177`):
- Creates rectangular margin around design, treats design exterior as a hole in that margin (line 131)
- Generates zigzag scan lines across this annular region
- Depth formula (line 163): `depth = ((dist - gap_offset) / tan_half + flat_depth).clamp(0.0, pocket_depth)`
  - `gap_offset = glue_gap / tan(half_angle)` (line 139)
  - `flat_depth` adds a constant offset to all male depths

**Half-angle integration**: correctly used throughout; V-bit's `included_angle` properly halved and converted to radians in both CLI (`main.rs:2725`) and GUI (`execute.rs:582`)

**Glue gap**: 0.1 mm default (`configs.rs:328`), converted to XY offset via `glue_gap / tan(half_angle)` — makes male plug slightly undersized relative to female pocket

### Corner Handling
- V-bit cannot reach sharp interior corners — this is a physics limitation, not a code bug
- `point_to_polygon_boundary()` (line 158) correctly computes minimum distance to edges, so corners naturally get shallow depth
- No special corner detection or user warning

### V-Bit Geometry Integration
- Female: passes `half_angle` directly to `vcarve_toolpath()` — correct
- Male: uses `tan_half = tan(half_angle)` for depth scaling — correct
- Tool type validated in GUI (`execute.rs:581-583`) and CLI

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | **High** | GUI merges female + male moves into a single `Toolpath` (`out.moves.extend(r.female)` then `out.moves.extend(r.male)`). CLI correctly produces two separate files. Users cannot separate male/female in GUI output. | `execute.rs:603-604` |
| 2 | **Med** | Male region ignores polygon holes: `Polygon2::with_holes(outer, vec![polygon.exterior.clone()])` — only uses `polygon.exterior`, not `polygon.holes`. Designs with islands (e.g., letter "O") will produce incorrect male ridges: the inner island boundary won't act as a depth reference. | `inlay.rs:131` |
| 3 | **Med** | `flat_depth` parameter creates asymmetry: adds constant depth to male but not female. If `flat_depth > 0`, the male plug is deeper than the female pocket at corresponding points. The purpose/intent of this parameter is undocumented. | `inlay.rs:163` |
| 4 | Low | No feature size validation: very small features (narrower than `2 * gap_offset`) will produce invalid geometry silently | `inlay.rs` |
| 5 | Low | No material thickness validation: `pocket_depth` not checked against available stock | `inlay.rs` |
| 6 | Low | Semantic tracing treats merged male+female as single operation — no distinction in debug view | `execute.rs:1864-1885` |

## Test Coverage

**8 tests** in `inlay.rs:230-416`:
1. `test_circle_inlay_female` (line 266): verifies female generates moves
2. `test_circle_inlay_male` (line 282): verifies male generates moves
3. `test_female_depth_bounded` (line 298): depth <= pocket_depth
4. `test_male_depth_bounded` (line 320): depth <= pocket_depth
5. `test_letter_o_with_island` (line 339): hole handling (but doesn't catch the male region bug since it only checks move count > 0)
6. `test_flat_clearing_adds_moves` (line 359): flat clearing verification
7. `test_glue_gap_affects_male_depth` (line 383): glue gap parameter effect
8. `test_polygon_bounds` (line 407): utility function

**Zero unwrap() calls** — clean error handling throughout.

## Test Gaps

- No test verifying male and female geometric complementarity (e.g., that male plug fits in female pocket)
- No test for male region with polygon holes (would expose issue #2)
- No test for sharp interior corner behavior
- No test for very small features that can't be inlayed
- No test for `flat_depth` interaction with flush fit
- No integration test comparing CLI dual-output vs GUI merged output
- No test for depth exceeding material thickness

## Suggestions

- **Fix issue #1**: Either split GUI output into two named toolpaths (female/male), or add sub-path markers so users can identify and export them separately
- **Fix issue #2**: Change line 131 to include `polygon.holes` in the male region construction: `Polygon2::with_holes(outer.exterior.clone(), [vec![polygon.exterior.clone()], polygon.holes.clone()].concat())`
- Document `flat_depth` purpose — if it's meant to create a mechanical lip/shelf for glue, say so
- Add a complementarity test that generates male and female at sample points and verifies matching depths
- Consider warning users when features are too small for the selected V-bit angle
