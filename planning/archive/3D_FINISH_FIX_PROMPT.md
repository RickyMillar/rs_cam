# Agent Prompt: Fix 3D Finish Toolpath Bugs

## Context

A guided fuzz test of the 3D finishing toolpaths identified 3 bugs. The full findings
are in `planning/3D_FINISH_BUGS.md` — read that first for reproduction steps, data
tables, and investigation guidance.

This is a 3-axis CAM application for wood routers. Read `CLAUDE.md` for project
conventions, lint policy, and dev workflow.

## Bugs to fix (priority order)

### Bug 1: Slope filtering generates full raster grid with retracts (Medium)

**File**: `crates/rs_cam_core/src/compute/` — find the drop_cutter operation implementation

**Problem**: When `slope_from`/`slope_to` restrict which surface slopes get machined,
the raster grid still covers the full stock XY area. Excluded regions become rapids
instead of being trimmed from the grid. This produces rapid distances 2-4x the cutting
distance.

**Fix**: After generating raster lines with Z values from the surface, partition each
line into segments that are within the slope range. Only emit cutting moves for
in-range segments. Use a single retract between discontinuous in-range segments
instead of per-step retracts over excluded areas.

**How to verify**:
1. Use MCP tools or write a test: drop_cutter with slope_from=30, slope_to=90 on a
   terrain mesh
2. Before fix: rapid distance >> cutting distance
3. After fix: rapid distance should be < cutting distance (only linking retracts)

### Bug 2: min_z doesn't reduce move count (Low)

**File**: Same drop_cutter implementation

**Problem**: `min_z` clamps each move's Z coordinate but doesn't eliminate passes
where the entire pass is at the clamped height (zero engagement). Move count is
always identical regardless of min_z.

**Fix**: After Z clamping, scan each raster line: if a contiguous segment is entirely
at min_z (meaning the surface is below the clamp everywhere along that segment), skip
those moves or convert them to a single retract.

**How to verify**:
1. Drop_cutter with min_z=6 on terrain with Z range 0–6.5mm
2. Before fix: 10,815 moves (same as min_z=-50)
3. After fix: significantly fewer moves (most of the surface is below Z=6)

### Bug 3: Systemic rapid collisions across all 3D finish operations (Medium)

**File**: Heights calculation, retract logic, or simulation collision detection

**Problem**: Every 3D finish operation produces rapid collisions (48–2088 depending
on parameters). Even the best combination (Spiral finish) has 48 rapid collisions.

**Investigation steps**:
1. Check how retract Z is computed for 3D finish operations — is it stock top or
   model-aware?
2. Check if roughing stock-to-leave (0.3mm) is causing residual material that
   rapids clip
3. Check the simulation's rapid collision detection — is it too sensitive? Does it
   account for the fact that finishing follows the roughed surface, not the original
   stock?
4. Check if rapids at stock XY boundaries clip through unroughed corners

This one needs investigation before fixing. It may be a heights config issue, a
roughing clearance issue, or a simulation detection sensitivity issue.

## Testing

After each fix:
```bash
cargo test -q                    # All tests pass
cargo clippy --workspace --all-targets -- -D warnings  # Zero warnings
```

For Bug 1 and 2, write regression tests in the relevant test module that verify
the fix. Use the existing parameter sweep infrastructure pattern from
`crates/rs_cam_core/tests/param_sweep.rs` if helpful.

For Bug 3, if the fix is in simulation detection sensitivity, add a test case that
confirms zero rapid collisions for a well-formed roughing + finishing pair.

## Approach

1. Start with Bug 1 (slope filtering) — most impactful, clearest fix path
2. Then Bug 2 (min_z) — similar root cause (post-generation filtering)
3. Investigate Bug 3 last — needs root cause analysis before fixing
4. Each bug should be a separate commit

## Key files to read first

| File | What it contains |
|------|-----------------|
| `planning/3D_FINISH_BUGS.md` | Full bug descriptions with data |
| `CLAUDE.md` | Project conventions and lint policy |
| `crates/rs_cam_core/src/compute/` | Operation implementations (find drop_cutter) |
| `crates/rs_cam_core/src/compute/operation_configs.rs` | DropCutterConfig struct |
| `crates/rs_cam_core/src/compute/config.rs` | HeightsConfig, retract settings |
| `crates/rs_cam_core/tests/param_sweep.rs` | Existing sweep test patterns |
| `crates/rs_cam_core/src/fingerprint.rs` | ToolpathFingerprint for comparison |
