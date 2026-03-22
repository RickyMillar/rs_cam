# Review: Interaction (Input, Picking, Camera)

## Summary

The interaction layer is well-structured with clear separation between picking, camera controls, and keyboard input handling. The system uses a straightforward ray-casting approach for 3D object selection, implements an orbit camera with sensible constraints, and maps keyboard shortcuts across three distinct workspaces. However, there are several gaps in testing, incomplete documentation of keyboard shortcuts, and edge cases around modifier key combinations that deserve attention.

## Findings

### Mouse Input

**Strengths:**
- Left-click implements proper ray-casting picking with world-space coordinates (picking.rs:63-71)
- Click-to-select is viewport-aware, using relative coordinates within the viewport rect (app.rs:1241-1248)
- Mouse drag is properly separated by button:
  - Primary (left): Orbit rotation around camera target (app.rs:1397-1399)
  - Secondary (right) or Middle: Pan translation (app.rs:1401-1405)
- Scroll wheel zoom is properly gated to viewport hover detection (app.rs:1408-1410)
- Zoom uses exponential scaling for consistent feel across distance ranges (camera.rs:100-102)

**Issues:**
- Orbit and pan operations don't explicitly check if the pointer is over the viewport. If a user drags from a side panel into the viewport, the camera will rotate/pan.
- Click-to-select threshold is inconsistent: collision markers use 12.0px threshold, toolpaths use 15.0px. This is not documented.
- Scroll zoom exponential formula `(-delta * 0.001).exp()` inverts scroll direction across platforms. No platform normalization.

### Keyboard Input

**Defined Shortcuts:**

| Key | Workspace | Action |
|-----|-----------|--------|
| Ctrl+Z | All | Undo |
| Ctrl+Shift+Z | All | Redo |
| Ctrl+S | All | Save Job |
| Ctrl+Shift+E | All | Export G-code |
| Delete/Backspace | Toolpaths | Remove selected toolpath |
| G | Toolpaths | Generate selected toolpath |
| Shift+G | Toolpaths | Generate all |
| Space | Toolpaths | Switch to Simulation (if results exist) |
| Space | Simulation | Play/pause |
| I | Toolpaths | Toggle isolation mode |
| H | Toolpaths | Toggle visibility of selected toolpath |
| 1-4 | Toolpaths | View presets (Top/Front/Right/Iso) |
| Arrow Left | Simulation | Step backward |
| Arrow Right | Simulation | Step forward |
| Home | Simulation | Jump to start |
| End | Simulation | Jump to end |
| [/] | Simulation | Speed down/up |
| Escape | Simulation | Back to Toolpaths |
| F12 | All | Request screenshot |

**Issues:**
- Space key conflict: Space triggers "Switch to Simulation" in Setup/Toolpaths, but there's no guard against accidental presses.
- G without selection is silently ignored — no feedback to user.
- No keyboard alternatives for pan/zoom/orbit.
- Escape in Simulation workspace while in a text field closes the simulation view, potentially unintended (app.rs:1746).

### 3D Picking System

The picking system is layered (picking.rs) with priority-based hit resolution:

1. **Screen-space picks first** (small, hard-to-click targets): collision markers (Simulation), alignment pins (Setup)
2. **Ray-based 3D picks** (geometric objects): fixtures, keep-out zones, stock faces
3. **Screen-space ray-sampled picks** (toolpaths)

**Quality:**
- Proper ray unproject with near/far plane points (camera.rs:156-161)
- Face normal determination uses epsilon-based matching (picking.rs:244-257)
- Stock face detection finds closest hit via BBox ray intersection (picking.rs:118-127)
- Toolpath picking uses adaptive sampling (step_by calculation)

**Problems:**
- Floating-point epsilon is hardcoded to 1e-4 — for large stock (mm scale), may be too tight
- Toolpath picking samples only 200 moves max: `let step = (moves.len() / 200).max(1);` (picking.rs:220). Large toolpaths are undersampled — thin features can be missed.
- No frustum culling: all objects are tested against the ray
- Collision marker picking doesn't validate marker existence — old indices could cause panic if list is cleared between frames

### Camera Controls

**Strengths:**
- Simple spherical coordinates (distance, yaw, pitch)
- Pitch clamped to prevent gimbal lock: `[-pi/2 + 0.01, pi/2 - 0.01]` (camera.rs:72-74)
- Fit-to-bounds logic properly centers and scales to bounding box (camera.rs:106-114)
- View preset snaps (Top, Front, Right, Iso) work cleanly

**Issues:**
- No smooth transitions for preset views (instant snap)
- Distance limits are very wide: 0.1 to 50,000 mm. Users can zoom to near-zero distance (camera.rs:102), which breaks rendering.
- Right-click pans the camera; no right-click context menu support.
- Up-vector fallback at pitch poles is not documented (camera.rs:90-93)

### Edge Cases & Conflicts

- Text input focus check prevents shortcuts from firing during editing (app.rs:1641-1642, 1717-1718) — good
- Viewport overlay buttons (Top, Front, Right, Iso, toggles) consume clicks before the viewport — correct priority
- Aspect ratio recalculated per frame — safe for window resize
- Viewport rect cached once per frame — safe

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Toolpath picking undersamples large toolpaths (200 move max), thin features can be missed | picking.rs:220 |
| 2 | Med | Scroll zoom direction inverts across platforms — no normalization | camera.rs:100-102 |
| 3 | Med | Escape in Simulation workspace fires even when user is in a text field | app.rs:1746 |
| 4 | Low | Collision marker picking doesn't validate indices against current list | picking.rs:~144 |
| 5 | Low | Floating-point epsilon hardcoded to 1e-4 for face detection — too tight for large stock | picking.rs:244-257 |
| 6 | Low | Camera zoom distance lower limit (0.1mm) too close — breaks rendering | camera.rs:102 |
| 7 | Low | G key without selection gives no feedback | app.rs keyboard handler |
| 8 | Low | Magic numbers scattered: 12.0px, 15.0px thresholds, 0.005 sensitivity, 0.001 zoom scale | picking.rs, camera.rs |

## Test Gaps

- **Zero tests in interaction module** — no tests for picking, face normal detection, or priority ordering
- Camera has one test (`unproject_round_trip()`) — insufficient:
  - No tests for orbit rotation edge cases (gimbal lock, pole singularities)
  - No tests for pan behavior at extreme pitch angles
  - No tests for zoom clamping and exponential scaling
  - No tests for fit-to-bounds edge cases (empty/degenerate bounding boxes)
  - No tests for preset view orientation values
- No integration tests verifying input routing (click → pick → selection state update)

## Suggestions

### High Priority
1. **Add keyboard shortcuts documentation:** Create a visible reference listing all shortcuts by workspace
2. **Test the picking system:** Unit tests for ray-face intersection, face normal detection, isolation mode pick behavior, threshold-based picking
3. **Resolve zoom scroll direction:** Normalize scroll input to platform conventions or add a user preference
4. **Document camera singularities:** Add comments explaining pitch clamp, up-vector fallback, and distance limits

### Medium Priority
5. **Add soft zoom limits:** Clamp when distance approaches 0 — render quality degrades below ~0.5mm
6. **Implement smooth preset transitions:** Interpolate yaw/pitch over 250ms when switching presets
7. **Validate collision marker indices** in pick_collision_markers() before accessing collision_positions array
8. **Optimize toolpath picking:** Use bounding-box culling before sampling, or implement a spatial index for large jobs

### Low Priority
9. **Extract magic numbers to named constants:** Create constants for pixel thresholds and sensitivity values
10. **Document drag cancellation:** Either implement Escape-to-cancel in all workspaces or document that drags end only by releasing the button
