# Review: G-code Output & Arc Fitting

## Summary

G-code generation supports three dialects (GRBL, LinuxCNC, Mach3) via a clean `PostProcessor` trait with proper preamble/postamble, multi-setup M0 pauses, modal feed optimization, and high-feedrate mode. Arc fitting uses a mathematically sound hybrid algorithm (least-squares + 3-point circle). The system is production-ready with 22 tests across gcode.rs and arcfit.rs. Main gaps: no tool change (M6) or coolant support, and pre/post G-code custom fields are stored but not emitted.

## Findings

### G-code Correctness

**PostProcessor Trait** (`gcode.rs:10-49`):
- Defines `preamble()`, `postamble()`, `rapid()`, `linear()`, `arc_cw()`, `arc_ccw()`, `comment()`
- `decimal_places()` method (default 3, overridable per dialect)
- `program_pause()` for M0 setup pauses (lines 24-31): emits M5 → comment → M0
- Canned drilling cycle methods (G81/G82/G83/G73/G80)

**Header/Footer Per Dialect:**

| Feature | GRBL | LinuxCNC | Mach3 |
|---------|------|----------|-------|
| Preamble | `G17 G21 G90 G40 G49 G80` + `M3 S{rpm}` | Same + `G54` | Same + `G4 P2` spindle dwell |
| Postamble | `M5`, `G0 Z10`, `M30` | `M5`, `G53 G0 Z0`, `M2` | `M5`, `G0 Z10`, `G28 G91 Z0`, `M30` |
| Decimal places | 3 | 4 | 4 |
| File | `gcode.rs:54-94` | `gcode.rs:99-144` | `gcode.rs:188-237` |

**Move Emission** (`gcode.rs:146-186`):
- G0: Rapid with XYZ coordinates
- G1: Linear with XYZ + F (feed rate in mm/min)
- G2: Clockwise arc with XYZ + IJK + F
- G3: Counter-clockwise arc with XYZ + IJK + F
- **Modal feed optimization**: Tracks `last_feed` to suppress redundant F parameters (lines 161-171)

**Spindle**: M3 S{rpm} in preamble, M5 in postamble, RPM changes between phases emit new M3 (`gcode.rs:263, 347-358`)

**Multi-Setup M0 Pauses** (`gcode.rs:313-401`):
- Retracts to `safe_z` before pause (line 339)
- Calls `program_pause()` with setup label (line 340)
- Restarts spindle with next phase's RPM after pause (line 347)
- Resets modal feed state between setups

**Not implemented**: Tool change (M6), coolant (M7/M8/M9)

### Dialect Differences

Differences are cleanly separated via the trait pattern:

| Feature | GRBL | LinuxCNC | Mach3 |
|---------|------|----------|-------|
| Coordinate system | None | G54 | None |
| Home retract | `G0 Z10` | `G53 G0 Z0` (machine coords) | `G28 G91 Z0` (relative home) |
| Spindle startup | Immediate | Immediate | `G4 P2` dwell |
| Program end | M30 | M2 | M30 |

Documentation quality: Good for trait design and extensibility. Gap: No comments explaining why LinuxCNC uses G54 or G53 for home.

### Arc Fitting (`arcfit.rs:16-127`)

**Algorithm**: Greedy longest-run fitting with hybrid circle computation:
- ≥5 points: Least-squares circle fit (Kåsa's algebraic method, `arcfit.rs:188-246`)
- 3-4 points: 3-point circle fit (determinant formula, `arcfit.rs:250-278`)
- Extends greedily until tolerance is exceeded, then emits arc and starts next segment

**Tolerance**: User-specified parameter. All intermediate points validated against fitted radius. Degenerate arcs rejected (radius > 1e6, `arcfit.rs:154`).

**G2 vs G3 Direction**: Cross product test on first/middle/last points (`arcfit.rs:168-179`). Negative = CW (G2), positive = CCW (G3).

**IJK Center Calculation**: Relative offsets from arc start to center (`arcfit.rs:95-97`):
```
i = arc.cx - start.x
j = arc.cy - start.y
```
Standard G-code convention.

**XY Plane Only**: Z-level constancy check (within tolerance) at `arcfit.rs:47-60`. Z changes break arc fitting. No 3D arcs.

**Edge cases handled**: Empty toolpath (returns empty), single/two-move segments (skipped), existing arcs (passed through unchanged), different feed rates (break arc), collinear points (rejected).

### High Feedrate Mode

- **Implementation**: `replace_rapids_with_feed()` (`gcode.rs:403-419`) — text-based post-processing
- Replaces lines starting with `G0 ` or `G0X` → `G1` + appends `F{high_feedrate}`
- **Configurable**: GUI state `job.post.high_feedrate_mode` (bool) + `job.post.high_feedrate` (f64, mm/min)
- Called from all three export functions (`export.rs:37-38, 87-88, 129-130`)
- **Fragility**: Text-based regex substitution; no structural validation of the line

### Export Wiring

**GUI** (`export.rs:1-135`): Three export functions:
1. `export_gcode()`: All enabled toolpaths from all setups as single file → `emit_gcode_phased()`
2. `export_combined_gcode()`: All setups with M0 pauses → `emit_gcode_multi_setup()`
3. `export_setup_gcode()`: Single setup → `emit_gcode_phased()`
- All check for empty toolpaths and return `Err("No computed toolpaths...")` if none

**CLI** (`main.rs`): Same core emission functions. Supports single-operation, per-setup, and all-in-one export.

**Post-processor lookup**: Case-insensitive name matching (`gcode.rs:421-429`)

### Pre/Post G-code Custom Fields

- **Stored**: `ToolpathEntry.pre_gcode` / `post_gcode` (`entry.rs:129-130`)
- **UI**: Editable text fields in properties panel (`properties/mod.rs:895-906`)
- **NOT EMITTED**: Documented honestly in `FEATURE_CATALOG.md:109` as a known partial area
- Would require extending the emission interface to inject per-operation custom code

### Code Quality

- **`gcode.rs`**: Zero unsafe `unwrap()` — two uses of `unwrap_or()` with safe defaults (`gcode.rs:329, 346`)
- **`arcfit.rs`**: Zero `unwrap()` in production code; all in tests
- **String writing**: `let _ = write!(...)` pattern documented as safe (String writes are infallible, OOM panics regardless) (`gcode.rs:3-5`)
- **Error handling**: Export functions return `Result<String, String>` with descriptive messages

### Test Coverage

| Module | Tests | Coverage |
|--------|-------|---------|
| `gcode.rs` | 8 | Preamble/postamble per dialect, arc output, phased emission (same/different RPM), multi-setup M0, program_pause |
| `arcfit.rs` | 14 | Circle fit, arc detection, tolerance, CW/CCW, G-code output, straight-line rejection, Z-breaks, empty paths |

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | No tool change (M6) support — multi-tool jobs require manual intervention | N/A (not implemented) |
| 2 | Medium | No coolant support (M7/M8/M9) | N/A (not implemented) |
| 3 | Medium | Pre/post G-code fields stored and editable but not emitted during export | `entry.rs:129-130`, `export.rs` |
| 4 | Low | `replace_rapids_with_feed()` is text-based — fragile for unusual G-code formatting | `gcode.rs:409` |
| 5 | Low | No documentation of LinuxCNC G54 workoffset or G53 home choice | `gcode.rs:108` |
| 6 | Low | Modal feedrate comparison uses exact f64 equality — could miss floating-point noise | `gcode.rs:161-171` |

## Test Gaps

- No test for high-feedrate mode (`replace_rapids_with_feed()`)
- No test for empty toolpath producing valid (header-only) G-code
- No test for very long toolpaths (performance / memory)
- No end-to-end test from operation → dressup → G-code emission

## Suggestions

- Wire `pre_gcode` / `post_gcode` emission or remove the UI fields to avoid confusing users
- Add a test for `replace_rapids_with_feed()` covering typical GRBL output
- Consider epsilon-based feed rate comparison for modal optimization
- Document dialect-specific choices (G54, G53, G28) with inline comments
