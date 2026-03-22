# Review: Profile Operation

## Summary

The Profile Operation implements tool radius compensation for contour cutting with offset-based path generation. Tabs and dogbones are integrated as post-process dressups. Core offset logic is sound, but tabs exhibit a high-severity bug in multi-pass scenarios where tabs are applied to all depth levels instead of just the final pass. G-code compensation (G41/G42) is present in the UI configuration but not emitted to output, per documented limitations.

## Findings

### Correctness

- **Offset direction**: ProfileSide::Inside offsets inward (positive distance) at `profile.rs:57`. ProfileSide::Outside offsets outward (negative distance) at `profile.rs:58`. Matches cavalier_contours sign convention documented at `polygon.rs:128-130`. Tests verify bounds at `profile.rs:138-141, 154-160`.
- **Tab height**: Tab Z height correctly computed as `cut_depth + tab.height` at `dressup.rs:268`. Sign convention: negative cut_depth (e.g., -6) + positive tab_height (e.g., 2) = -4 (4mm above final cut). Interpolation along segments handles boundary crossings at `dressup.rs:311-383`.
- **Dogbone bisector math**: Dot product angle calculation at `dressup.rs:584-586`. Bisector computation at `dressup.rs:595-596`. Overcut distance: `tool_radius` along bisector at `dressup.rs:613-614`. Correctly identifies sharp interior corners (angle < pi - max_angle_rad) at `dressup.rs:589`.
- **Climb vs conventional**: Contour reversal for climb mode at `profile.rs:78-81`. Test verifies directional difference at `profile.rs:265-298`.
- **Collapse handling**: Inside profile on small polygon correctly returns None at `profile.rs:45-69`. Test confirms at `profile.rs:164-175`.

### Integration

- **Tabs on all passes (HIGH)**: `apply_tabs` called after `depth_stepped_toolpath` at `execute.rs:461-480`. With depth_per_pass=2 and depth=6, the depth-stepped toolpath generates moves at Z=-2, -4, -6. `apply_tabs` filters by `(m.target.z - cut_depth).abs() < 0.01` at `dressup.rs:229`, so tabs are applied to any pass at the final Z level. Tabs should only appear on the final finishing pass, not roughing passes that happen to reach the same depth.
- **G41/G42 not emitted (AS DESIGNED)**: `ProfileConfig` has `compensation: CompensationType` field at `configs.rs:207-218`. UI exposes both InComputer and InControl options. Semantic trace logs the selection at `execute.rs:1746`. G41/G42 NOT emitted — documented limitation at `FEATURE_CATALOG.md:111`. No G-code generation for InControl mode found in `gcode.rs`.
- **Dressup application order**: Entry (ramp/helix) → Dogbones → Lead-in/out → Link moves → Arc fitting → Feed opt → Rapid ordering at `helpers.rs:47-304`. Order preserves geometry integrity.
- **No open contour validation**: Profile assumes closed loop for perimeter-based tab positioning at `profile.rs:97-101`. Open contours from malformed DXF/SVG could produce incorrect tab positions.

### Edge Cases

- **Very small features**: Inside profile correctly collapses at `profile.rs:164-175`. Outside profile handles small features (negative offset always grows) at `profile.rs:137-142`.
- **Tab on short segments**: `apply_tabs` checks `seg_len < 1e-10` for zero-length segments at `dressup.rs:306-308`. Tab zone boundaries may extend beyond segment length; handled via clamping at `dressup.rs:266-267`.
- **Dogbone at obtuse corners**: Angle threshold treats all corners uniformly at `dressup.rs:589`. For max_angle_deg=170°, threshold is ~10° interior angle. No test for obtuse exterior corners (> 180° interior), which should NOT get dogbones.
- **Polygon with holes**: `offset_polygon` supports holes at `polygon.rs:144-199`. Tab position based on exterior perimeter only — hole perimeters not accounted in `even_tabs` spacing at `dressup.rs:238-246`.

### Code Quality

- **Error handling**: profile.rs uses `unwrap()` only in tests, avoided in production. dressup.rs has defensive checks on slice lengths, segment lengths at `dressup.rs:234, 306, 574-575`. polygon.rs has no unwraps; early returns on invalid state.
- **Test coverage**: profile.rs has 7 tests covering basic functionality, contour direction, collapse, L-shapes. dressup.rs has 5 tab tests and 5 dogbone tests — solid dressup coverage.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Tabs applied to all depth passes, not just final pass: with depth_per_pass < depth, tabs appear on roughing passes at the final Z level instead of only the finishing pass | `execute.rs:475-480`, `dressup.rs:229` |
| 2 | Medium | G41/G42 compensation not emitted: UI exposes InControl mode and logs selection, but G-code does not emit G41/G42. Documented as incomplete. | `FEATURE_CATALOG.md:111`, `execute.rs:1746` |
| 3 | Medium | No validation for closed polygon in profile: assumes closed contour for tab perimeter calculation, open contours produce incorrect tab positions | `profile.rs:97-101` |
| 4 | Low | No test for dogbone on exterior/obtuse corners (> 180° interior angle), which should not receive dogbones | `dressup.rs:1054-1135` |
| 5 | Low | Tab positioning based on exterior perimeter only — hole perimeters not accounted in even_tabs spacing | `dressup.rs:238-246` |

## Test Gaps

- Multi-pass profile with depth_per_pass > 0 and tabs enabled (critical for issue #1 verification)
- Profile operation end-to-end through worker/execute layer
- Tab positioning on polygon with holes
- Dogbone behavior on mixed corner angles (acute vs obtuse in same polygon)
- Tab on very short contour segments (< tab width)
- Profile inside operation on polygon with multiple holes of different sizes
- Tab + dogbone interaction on same contour

## Suggestions

1. **Fix tab multi-pass bug**: Apply tabs only to the final depth level. Modify `run_profile` to track which passes are final and apply tabs conditionally, or split depth-stepped toolpath into rough + finish and only apply tabs to the finish pass.
2. **Add polygon closure validation**: In `profile_contour`, assert that first and last vertices are coincident or add a closing segment.
3. **Disable or label InControl mode**: Either disable InControl in the UI until G41/G42 emission is implemented, or add a tooltip: "G41/G42 not yet emitted; use InComputer."
4. **Extend dogbone tests**: Add cases for obtuse corners, mixed-angle polygons, and verify angle threshold matches intent.
5. **Add multi-pass + tab integration test**: Create test exercising `depth_stepped_toolpath` with depth_per_pass < depth + tabs, verifying tabs only on final pass.
