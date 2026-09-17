# `machine/` — the machine profile and its kinematics

The machine model that feeds, cycle time and the strategy choice read. The
principal type is `MachineProfile` in `mod.rs`.

## Files

- `mod.rs` — the machine profile: spindle, power, per-axis rates and accel.
- `kinematics.rs` — the kinematics model for cycle-time estimation.
- `kinematic_utilization.rs` — the machine-confidence instrument.
- `strategy_advisor.rs` — suggests the clearing strategy from the profile.

## Invariants

- The per-axis maximum rate is per axis. A single scalar rate over-estimates
  a diagonal move.
- Machine acceleration decides parallel against spiral. The advisor reads the
  profile; do not hard-code a strategy preference.

## Sentries

- `cargo test -p rs_cam_core -q --test kinematics_per_axis_rate_p1`
- `cargo test -p rs_cam_core -q --test kinematic_utilization_p2`
- `cargo test -p rs_cam_core -q --test kinematic_surfacing_p4`
- `cargo test -p rs_cam_core -q --test a_downward_traverse_rounds_down_g_rpmdown`
- `cargo test -p rs_cam_core -q --test power_ceiling_parity_f2`

## Do not

- Do not compare a wall clock across arms without a fresh measurement on the
  same profile.
