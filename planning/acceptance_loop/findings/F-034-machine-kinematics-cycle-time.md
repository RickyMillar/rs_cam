# F-034 — Acceleration-aware cycle time estimator

- **Stage:** sim / machine model (additive feature)
- **Severity:** medium (real-world cycle time misprediction; not a correctness bug per se)
- **Status:** landed 2026-05-26 (commit pending — see Implementation log)
- **First found in:** round-10 wanaka audit (2026-05-26); user-flagged
- **Effort:** S-M
- **Linked PRs:** —
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Source audits:** round-10 wanaka verification + user discussion of Shapeoko XXL feed-rate realism

## Symptom

`SimulationResult.total_runtime_s` is computed as a naive `cutting_distance / feed_rate` integral. It ignores machine acceleration and jerk limits. Consequence: on corner-heavy toolpaths (typical 3D rough or finish), the predicted runtime under-estimates real-world runtime by 30-50%.

Concrete example (wanaka, Back Rough):
- Reported `total_runtime_s` ≈ 3438 s (~57 min) for the project
- Actual machine time on a Shapeoko XXL (stock kinematics, 250 mm/s² accel) likely ~70-85 min based on the toolpath's move density (4226 moves over 18 km of cutting + 9 km of rapids = ~6 mm avg move, many direction changes)

User experience cost:
- Quote estimation is wrong
- Job scheduling is wrong
- Comparing two toolpath variants by runtime is misleading

## Hypothesised root cause

The toolpath emission produces `LinearMove` / `ArcMove` records. The simulator currently sums `(move.length / move.feed_rate)` for each move. This treats every move as if the machine is already at commanded feed — no spool-up, no spool-down, no corner decel.

Real machines obey trapezoidal (with jerk-limited corners) velocity profiles: accelerate from 0 → feed at `acceleration_mm_s2`, cruise at feed, decelerate to 0 (or to next-move-junction-velocity) at the same accel limit. Jerk-limited controllers (GRBL, LinuxCNC, most modern industrial) further smooth the accel ramps.

## Fix shape

**Two parts**:

### Part A — extend `Machine` config with kinematics

Add fields to the existing `Machine` struct (probably in `crates/rs_cam_core/src/machine.rs` or similar — verify location):

```rust
pub struct MachineKinematics {
    /// Linear-axis acceleration limit (mm/s²)
    pub acceleration_mm_s2: f64,
    /// Linear-axis jerk limit (mm/s³). Optional; if None, treat as infinite (pure trapezoidal).
    pub jerk_mm_s3: Option<f64>,
    /// Max junction velocity (mm/min). Optional; if None, derive from cornering geometry.
    pub max_junction_velocity_mm_min: Option<f64>,
}
```

Reasonable Shapeoko XXL defaults (stock, before community tweaks):
- `acceleration_mm_s2: 250.0`
- `jerk_mm_s3: None` (or 5000 if jerk-limited mode)
- `max_junction_velocity_mm_min: None`

These should be Machine-preset specific. Update the "Generic Wood Router" preset to a sensible-but-conservative default; add a Shapeoko XXL preset.

### Part B — cycle time integrator

Add a function (probably in `simulation/` or a new `machine_kinematics/` module):

```rust
pub fn compute_cycle_time(
    toolpath: &Toolpath,
    kinematics: &MachineKinematics,
    max_feed_mm_min: f64,
) -> f64 // seconds
```

Algorithm (trapezoidal):
1. For each consecutive pair of moves, compute the junction velocity (the max feed where direction-change-decel fits within the machine's accel limit).
2. For each move, integrate: time to accelerate from `v_in` to commanded `feed` capped at `max_feed`, cruise at peak feed for the remaining distance, decelerate to `v_out` (which is `v_in` of the next move).
3. If the move is too short to reach peak feed, use trapezoidal-capped-by-distance integration.
4. Sum across all moves.
5. Add fixed time costs for plunges, retracts (these are already individual moves; treated the same).

Set `SimulationResult.total_runtime_s` to this value when the kinematics model is available; fall back to naive computation when not.

## Calibration test (the most important test)

The implementer MUST measure at least one real-machine cycle time and assert the model predicts within 10%.

Suggested protocol:
1. Pick a toolpath (e.g. wanaka's "Back Rough" or a small known reference like `test_data/ux_2d_pocket.toml` AS001 pocket).
2. Run it on a Shapeoko XXL (or whatever the user has). Wall-clock the actual cut time.
3. Assert `compute_cycle_time(toolpath, shapeoko_kinematics, 4000) >= measured * 0.90 && <= measured * 1.10`.

Without this calibration, the model is theoretical. WITH it, the model is grounded.

## Acceptance test

```rust
// tests/machine_kinematics_cycle_time_f034.rs

#[test]
fn cycle_time_with_accel_model_below_naive_for_curve_heavy_toolpath() {
    // Load a toolpath with many corners (e.g. AS013 adaptive3d).
    // Assert: kinematic cycle time > naive cycle time.
    // Difference should be > 10% on corner-heavy paths.
}

#[test]
fn cycle_time_matches_naive_for_pure_straight_line() {
    // Single long linear move.
    // Assert: kinematic == naive (no corners to decel through).
}

#[test]
fn cycle_time_calibrated_against_shapeoko_reference() {
    // Reference toolpath (e.g. wanaka subset).
    // Assert: kinematic_estimate within ±10% of REFERENCE_MEASURED_S const.
    // REFERENCE_MEASURED_S must be measured on a real machine first.
}

#[test]
fn flag_off_byte_identical_to_pre_f034() {
    // With kinematics model disabled (or no kinematics on Machine),
    // assert SimulationResult.total_runtime_s == naive distance/feed sum,
    // byte-identical to pre-F-034 behavior.
}
```

The fourth test protects existing acceptance tests from regression.

## Files

Starting points:
- `crates/rs_cam_core/src/machine.rs` (or wherever `Machine` is defined) — add `MachineKinematics`
- `crates/rs_cam_core/src/compute/simulate.rs` — wire cycle-time computation into `SimulationResult`
- `crates/rs_cam_core/src/toolpath/` — the IR; cycle time integrator walks `LinearMove` + `ArcMove`
- `crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs` — new

Existing tests that must continue to pass byte-identically:
- All `_f0{24,26,27,28,31}.rs`
- All workspace cargo tests

## Risk

S-M.

- **S** if the trapezoidal integrator is well-bounded scope (one new file, one Machine field, one entry point into `SimulationResult`)
- **M** if the calibration test requires user-side machine measurement before landing (which it should)
- **Mitigating**: feature is purely additive — old field stays, new field appears alongside. Can't regress any existing acceptance test.

## Notes

- **Default flag handling**: this is purely additive — when `Machine::kinematics` is `None`, behavior is byte-identical to today. No explicit feature flag needed; the absence/presence of kinematics IS the flag.
- **Calibration is the load-bearing test.** Without a real-machine measurement, the integrator is plausible but unverified. The implementer should NOT skip step 3 of the calibration protocol.
- **Don't model junction-velocity decel from cornering angles in v1.** Use a single `max_junction_velocity` constant or treat corners as full-stop. Refinement comes later if v1's accuracy is insufficient.
- Cross-link to F-035 (predicted-feed gate consumer) and F-036 (modulation post-pass).
</parameter>
</invoke>