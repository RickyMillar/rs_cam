# Review: CLI Commands & Job Runner

## Summary

The rs_cam CLI is well-structured with 14 distinct operations covering 2D pocket, profile, adaptive, V-carve, rest machining, 3D adaptive, waterline, and several finishing strategies. The TOML job runner supports multi-setup work and per-operation parameter overrides. The implementation is generally solid with good error handling (zero unwrap() calls, anyhow::Context throughout), but there is a critical tool radius calculation mismatch between CLI and job runner for Adaptive3d, inconsistent entry parameter naming, and no CLI integration tests.

## Findings

### Command Surface

All 14 commands are well-documented with descriptive help text. Parameter naming is mostly consistent:

**2D Operations (Pocket, Profile, Adaptive, Vcarve, Rest):**
- Standard parameters: `feed_rate`, `plunge_rate`, `spindle_speed`, `safe_z`, `depth`, `depth_per_pass`
- Optional outputs: `--output` (required), `--svg`, `--view`, `--simulate`, `--sim_resolution`
- Entry dressing: Consistent use of `entry` field with values: `plunge|ramp|helix`

**3D Operations (DropCutter, Adaptive3d, Waterline, RampFinish, SteepShallow, Pencil, Scallop):**
- STL scaling: `--units` (mm|m|cm|inch|ft) with `--scale` override
- Similar core parameters to 2D variants
- Adaptive3d uses `entry_style` instead of `entry` (inconsistent)

**Special Parameters:**
- `Rest`: requires both `--tool` and `--prev_tool` with validation that prev_tool is larger
- `Inlay`: generates dual outputs (female + male) with `--male_output` defaulting to `<output>_male.nc`
- `Vcarve`: requires tool in form `vbit:diameter:included_angle` (e.g., `vbit:6.35:90`)
- Collision detection: `--holder_diameter`, `--shank_diameter`, `--shank_length`, `--stickout` (DropCutter, Waterline, Ramp, Steep, Pencil, Scallop)
- Link moves: `--link_moves` (DropCutter, Waterline, Rest, Pencil, Scallop)

**Defaults are well-chosen:**
- Feed rate: 1000 mm/min
- Plunge rate: 500 mm/min
- Spindle speed: 18000 RPM
- Safe Z: 10.0 mm
- Simulation resolution: 0.25 mm
- Tool cutting length: `diameter * 4.0`

### Job Runner

**Job configuration structure (job.rs):**
- `[job]`: Global settings (output, post, spindle_speed, safe_z, view, svg, simulate, sim_resolution)
- `[tools.name]`: Tool definitions with type and parameters
- `[[setup]]`: Optional multi-setup definitions with per-setup output files
- `[[operation]]`: Array of operations with type, input, and tool reference

**Strengths:**
- Full validation: checks all referenced tools and setups exist (job.rs:201-224)
- Relative path resolution: operations and output paths resolve relative to job file directory (job.rs:303-307, main.rs:1309-1313)
- Multi-setup support: auto-generates output filename if not specified (main.rs:1354-1360)
- Per-operation parameter override: all params marked `Option<T>` with sensible defaults
- Stacked simulation: correctly handles multi-phase visualization with per-phase cutters

**Weaknesses:**
- No schema documentation in comments (only example in job.rs docstring)
- Setup definitions have unused fields: `face_up`, `z_rotation` marked `#[allow(dead_code)]` (job.rs:68-73)
- 6 operation types in job file vs 14 CLI commands (pocket, profile, adaptive, rest, adaptive3d, drop-cutter only)

### CLI vs GUI Parity

**CLI-only features not found in GUI:**
- Batch job runner with multi-setup support
- Waterline finishing (constant-Z contouring)
- Ramp finishing (continuous descent on slopes)
- Steep & Shallow (hybrid approach for mixed terrain)
- Pencil finishing (crease tracing)
- Scallop finishing (constant scallop height)
- Inlay operations (male/female V-carve generation)
- Collision detection with holder/shank geometry

**Parameter name inconsistencies:**
- `entry` vs `entry_style`: Job file uses `entry` for pocket/profile/adaptive but `entry_style` for adaptive3d; CLI uses `entry` uniformly for 2D and `entry_style` for Adaptive3d
- Side parameter: CLI accepts "inside|in" and "outside|out" (main.rs:1745-1748); job file handles "inside|in" vs default "outside" (job.rs:371-373)

### Error Handling

**Strong patterns:**
- All I/O uses `anyhow::Context` with descriptive messages (job.rs:192-194)
- Tool parsing validates all required parameters (job.rs:231-263)
- Job file validation is comprehensive (job.rs:197-224)
- Zero unwrap() calls found in CLI or job.rs code
- Collision detection provides min_safe_stickout and specific move indices (main.rs:1577-1598)
- Post-processor validation suggests supported options (main.rs:1253-1256)

**Issues:**
- CLI silently overwrites existing output files. No warning or `--force` flag
- No suggestion to check path when input file is missing

### Magic Numbers & Hardcoded Values

| Value | Location | Purpose | Issue |
|-------|----------|---------|-------|
| 170.0 | job.rs:360, 413; main.rs:1701, 1806 | Dogbone corner angle (degrees) | Should be constant/configurable |
| 4.0 | job.rs:234; main.rs:1076 | Tool cutting length multiplier | Should be constant |
| 40.0 | main.rs:1570, 2388, 2526, 2659, 2851, 2985 | Holder length for collision checks | Hardcoded 6 times; should be parameter or constant |
| 0.4 | main.rs:2112 | Stepover for prev_tool in rest sim | Magic number |
| 1_048_576 | main.rs:2277 | MB conversion constant | Correct but could use constant |

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | **Critical** | Tool radius calculation mismatch: job.rs passes `diameter/2.0` to Adaptive3dParams while main.rs Adaptive3d CLI passes `cutter.radius()` directly. Both feed same struct, so job file adaptive3d operations may compute at wrong tool radius. | job.rs:294,550 vs main.rs:2221 |
| 2 | High | Job file schema uses `entry` for 2D ops but `entry_style` for 3D ops; CLI uses `entry` for all 2D and `entry_style` for Adaptive3d. Inconsistent API surface. | job.rs:158,183 |
| 3 | Medium | Hardcoded holder length (40.0 mm) appears 6 times in collision checks. Should be a named constant or extracted parameter. | main.rs:1570, 2388, 2526, 2659, 2851, 2985 |
| 4 | Medium | CLI silently overwrites existing output files without warning or `--force` flag. | main.rs:1261 and all emit_and_write calls |
| 5 | Low | Setup definitions in TOML have unused fields `face_up` and `z_rotation` marked with `#[allow(dead_code)]`. | job.rs:68-73 |
| 6 | Low | No TOML schema documentation in codebase. Only inline example in job.rs docstring. | fixtures/demo_job.toml, job.rs:6-28 |
| 7 | Low | Dogbone angle (170.0) and tool cutting length (diameter*4.0) are magic numbers scattered through code. Should be constants. | job.rs:234, 360, 413; main.rs:1076, 1701, 1806 |

## Test Gaps

- **No CLI integration tests found** in crates/rs_cam_cli/src/ or tests/ directory
- **Demo job not tested as part of build**: fixtures/demo_job.toml exists but no test runs it
- **No parameterization tests**: verify that job file with same params produces same toolpath as equivalent CLI command
- **No edge case tests**: job file with missing tool reference, zero operations, output file already exists
- **No tool spec parsing tests**: malformed tool strings not validated in tests
- **No multi-setup job tests**: verify output file naming and separation
- **No collision detection regression tests**: especially for different holder geometries

## Suggestions

1. **Fix tool radius bug immediately**: In job.rs:294, verify whether `cutter.radius()` and `diameter/2.0` actually differ (they shouldn't for a correct Cutter impl). If Cutter::radius() does something non-trivial, standardize on one approach
2. **Standardize entry parameter naming**: either rename all to `entry_style` or split the Adaptive3d special case. Update TOML schema and CLI to match
3. **Extract magic numbers to constants**: `DOGBONE_ANGLE_DEG = 170.0`, `TOOL_CUTTING_LENGTH_MULTIPLIER = 4.0`, `DEFAULT_HOLDER_LENGTH_MM = 40.0`
4. **Add overwrite protection**: warn before overwriting existing output files; add `--force` or `--overwrite` flag
5. **Document TOML schema explicitly**: list all operation types and their required/optional parameters with commented examples
6. **Add CLI integration tests**: run demo_job.toml as part of CI; test cross-parity between CLI and job file for standard operations
7. **Clean up dead code**: remove or populate `face_up`, `z_rotation` in SetupDef, or document intended future use
8. **Add --holder-length parameter** to collision check commands instead of hardcoding 40.0 mm six times
