# Review: Unwired / Partial Features

## Summary
Of 7 candidates investigated, only 2 are genuinely unwired (pre/post G-code injection and workholding rigidity UI). Cutter compensation G41/G42 is a moot UI toggle for 3-axis work. Three candidates (collision rendering, deviation coloring, feed optimization) were confirmed fully wired — false positives. Code hygiene is good: zero TODOs/FIXMEs in Rust source; 28 `#[allow(dead_code)]` annotations are justified (forward-compat and experimental).

## Findings

### Confirmed Unwired

#### 1. Pre/Post G-code Injection
- **Current state:** Editable multiline text fields in UI; stored in toolpath state and persisted to JSON project files
- **Wiring gap:** Export pipeline (`emit_gcode*()`) does not read or emit these fields. No injection point between GcodePhase list and emission. Uses only post-processor preamble/postamble.
- **Effort:** Moderate — add optional string fields to Toolpath struct; emit after preamble / before postamble in `emit_gcode_phased()`
- **Risk:** **High** — pre/post G-code could contain setup M-codes (M7, M9) that are silently dropped. User won't know their custom codes aren't in the output.

#### 2. Workholding Rigidity UI
- **Current state:** `WorkholdingRigidity` enum supports Low/Medium/High in `feeds::SetupContext`. Calculator uses rigidity derate in DOC/WOC calculations.
- **Wiring gap:** Hardcoded to `Medium` in UI (`ui/properties/mod.rs:613`). No ComboBox or slider exposed. Backend fully supports all levels.
- **Effort:** Trivial — add one ComboBox
- **Risk:** Low — incorrect rigidity assumptions produce underoptimized feeds, not catastrophic failures

### Partially Unwired (Moot)

#### 3. Cutter Compensation G41/G42 "In Control"
- **Current state:** UI ComboBox on Profile operation (ProfileConfig::compensation) with InComputer vs InControl options
- **Wiring gap:** Profile operation always offsets tool center path by tool_radius (In-Computer). InControl setting is read but never used. No G41/G42 codes generated.
- **Assessment:** For 3-axis wood routing, In-Computer compensation is the correct approach. G41/G42 is primarily for metalworking with control-side comp. This toggle is misleading but not functionally harmful.
- **Effort:** Moderate (if wiring needed) — but likely should just remove the InControl option
- **Risk:** Moderate — user may toggle it expecting behavior change; nothing happens

### Verified Fully Wired (No Issues)

#### 4. Collision Marker Rendering
- Collision detection computes markers, uploads to GPU vertex buffer (`collision_vertex_buffer`), pickable via `pick_collision_markers()`. Rendered in `app.rs:1010-1031`. Fully wired.

#### 5. Simulation Deviation Coloring
- `deviation_colors()` function exists, `display_deviations` in simulation state, wired to renderer (`app.rs:423-425`). Green/yellow/red/blue mesh coloring based on deviation from CAD model. Fully functional.

#### 6. Feed Rate Optimization
- UI checkbox enabled, execution wired. Correctly gated: `feed_optimization_unavailable_reason()` blocks remaining-stock, rest operations, 3D mesh-derived surfaces. Limitation is documented and enforced with user-visible feedback. Phase 1 scope is complete.

#### 7. Vendor LUT
- Fully loaded and embedded (`VENDOR_LUT::embedded()`), used in feed calculation. 8 vendors included. No UI needed for current scope — data is used automatically. Debug/selection UI would be nice-to-have but not a wiring gap.

### Code Quality Metrics

#### Dead Code (`#[allow(dead_code)]`)
28 annotations across 9 files, all justified:
- `feeds/vendor_lut.rs` (4): Metadata fields deserialized from JSON but not displayed — forward reference
- `pencil.rs` (2): Edge-face adjacency tracking unused in current algorithm
- `adaptive.rs` (2) + `adaptive3d.rs` (1): Experimental direction search functions superseded
- `arcfit.rs` (1): Geometry utility kept for future use
- `rs_cam_cli/job.rs` (6): Job JSON fields not yet consumed by CLI — forward compatibility
- `compute/worker/helpers.rs` (2): Wrapper variants with different signatures
- `compute/worker/execute.rs` (6): Functions called via enum dispatch, invisible to compiler
- `compute/worker/semantic.rs` (1): Debug instrumentation kept for future tracing

#### TODO/FIXME/HACK Count
- **Zero** in Rust source files (good hygiene)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Pre/post G-code fields edited in UI but silently dropped at export | UI state → gcode.rs (missing link) |
| 2 | Med | Cutter compensation InControl toggle has no effect | ProfileConfig::compensation |
| 3 | Low | Workholding rigidity hardcoded to Medium; backend supports all 3 levels | ui/properties/mod.rs:613 |

## Test Gaps
- No test verifies that pre/post G-code fields are (or should be) emitted
- No test verifies InControl compensation behavior

## Suggestions

### High Priority
1. **Wire pre/post G-code injection** — Add fields to Toolpath struct, emit in `emit_gcode_phased()`. Users editing these fields expect the codes in output.

### Medium Priority
2. **Remove or disable InControl option** — Either wire G41/G42 (unlikely needed for wood routing) or remove the misleading ComboBox option

### Low Priority
3. **Add WorkholdingRigidity ComboBox** — Trivial UI addition, backend already supports it
4. **Optional vendor LUT debug view** — Show which vendor/row matched during feed calculation
