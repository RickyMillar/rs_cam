# Parameter Sweep Agent Instructions

You are a validation agent for rs_cam parameter testing. Your job is to run
parameter sweeps, analyze the results, and report verdicts.

## Quick start

```bash
# 1. Run your assigned sweep tests
cargo test --test param_sweep sweep_pocket -- --nocapture

# 2. Analyze results
python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/pocket/ > results.json

# 3. Read the diff JSONs for detailed analysis
cat target/param_sweeps/pocket/stepover/variant_0.5_diff.json
```

## What you're checking

For each parameter sweep, you verify:
1. The parameter change produced an effect (fingerprint diff is non-empty)
2. The RIGHT things changed (stepover should change move_count, not z_levels)
3. The WRONG things didn't change (feed_rate change should NOT affect geometry)
4. No visual artifacts in the SVG output

## Work partitions

### Agent A: 2D contour operations
```bash
cargo test --test param_sweep sweep_pocket -- --nocapture
cargo test --test param_sweep sweep_profile -- --nocapture
cargo test --test param_sweep sweep_zigzag -- --nocapture
cargo test --test param_sweep sweep_trace -- --nocapture
cargo test --test param_sweep sweep_chamfer -- --nocapture
```

### Agent B: 2D clearing operations
```bash
cargo test --test param_sweep sweep_adaptive_ -- --nocapture  # note trailing underscore to avoid adaptive3d
cargo test --test param_sweep sweep_face -- --nocapture
cargo test --test param_sweep sweep_vcarve -- --nocapture
cargo test --test param_sweep sweep_rest -- --nocapture
cargo test --test param_sweep sweep_inlay -- --nocapture
cargo test --test param_sweep sweep_drill -- --nocapture
```

### Agent C: 3D raster operations
```bash
cargo test --test param_sweep sweep_dropcutter -- --nocapture
cargo test --test param_sweep sweep_spiral_finish -- --nocapture
cargo test --test param_sweep sweep_radial_finish -- --nocapture
cargo test --test param_sweep sweep_horizontal_finish -- --nocapture
cargo test --test param_sweep sweep_project_curve -- --nocapture
```

### Agent D: 3D contour operations
```bash
cargo test --test param_sweep sweep_waterline -- --nocapture
cargo test --test param_sweep sweep_pencil -- --nocapture
cargo test --test param_sweep sweep_scallop -- --nocapture
cargo test --test param_sweep sweep_steep_shallow -- --nocapture
cargo test --test param_sweep sweep_ramp_finish -- --nocapture
cargo test --test param_sweep sweep_adaptive3d -- --nocapture
```

## How to read a diff JSON

```json
{
  "changed_fields": [
    {"field": "move_count", "before": 42, "after": 168, "delta_percent": 300.0}
  ],
  "unchanged_fields": ["min_z", "max_z", "feed_rates", "z_levels"]
}
```

- `changed_fields`: what differed between baseline and variant
- `delta_percent`: percentage change (positive = increased)
- `unchanged_fields`: what stayed the same

## Expected effects by parameter type

| Parameter | What SHOULD change | What should NOT change |
|-----------|-------------------|----------------------|
| stepover ↓ | move_count ↑, cutting_distance ↑ | z_levels, feed_rates, min_z |
| feed_rate | feed_rates, max_feed_rate | move_count, z_levels, bbox |
| depth/cut_depth | min_z, z_levels | move_count per level |
| safe_z | max_z, rapid_distance | min_z, cutting_distance |
| side (profile) | bbox shifts | move_count stays similar |
| climb toggle | path reverses (SVG diff) | aggregate metrics may be identical |
| tolerance ↓ | move_count ↑ | z_levels, feed_rates |
| bool toggles | something changes | if nothing changes = possible bug |

## Visual inspection (SVG files)

Look at the toolpath SVGs for:
- **Gouges**: unexpected dark spots in stock heightmap
- **Missed regions**: light patches in areas that should be cleared
- **Boundary violations**: cuts outside the stock boundary
- **Air cutting**: many toolpath lines over uncut stock

The SVGs are text-based. You can `diff baseline.svg variant.svg` for structural comparison.

## Reporting verdicts

For each parameter variant, report one of:
- **PASS**: Expected changes happened, no unexpected side effects
- **FAIL**: Expected changes did NOT happen (parameter had no effect when it should)
- **NO_EFFECT**: Nothing changed at all (parameter may be dead code)
- **UNEXPECTED**: Unexpected side effects (e.g., feed_rate change altered geometry)

Write your verdicts to `target/param_sweeps/{op}/verdicts.json`.

## CPU monitoring

Before launching heavy sweeps, check system load:
```bash
cat /proc/loadavg | awk '{print $1}'
nproc  # number of cores
```

If load average > (cores × 0.8), wait before launching more tests.
