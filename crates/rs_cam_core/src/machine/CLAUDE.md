# `machine/` — the machine profile and its kinematics

The machine model that feeds, cycle time and the strategy choice read. The
principal type is `MachineProfile` in `mod.rs`.

## Files

- `mod.rs` — the machine profile: spindle, power, per-axis rates and accel.
  `head.rs` — the machine head and the dust shoe (H1).
- `kinematics.rs` — the kinematics model for cycle-time estimation.
- `kinematic_utilization.rs` — the machine-confidence instrument.
- `strategy_advisor.rs` — suggests the clearing strategy from the profile.

## Invariants

- The per-axis maximum rate is per axis. A single scalar rate over-estimates a diagonal move.
- Machine acceleration decides parallel against spiral. The advisor reads the profile; do not hard-code a strategy preference.
- H1: the head is a machine part, not a tool part; `MachineHead::bands` is the one band builder.
  Hard parts block, soft parts never; a shelf is a Z floor. The generation step reads the fixed
  head (`HeadFit::fixed`); the shoe enters only the checks, at the resolved gap (`compute::shoe`).
- The cycle time and the predicted feed come from ONE pass: `digest_moves`
  then `walk_junctions`, over the one physics site `solve_move`. Do not add a
  second digest loop or a second junction walk (EDG-07).
- A junction reads each move's end tangent (`move_end_tangents`; an arc's own tangent, a helix's
  with its slope), never an arc's chord.
- `MachineProfile::link_kinematics` is the one link-envelope builder. A machine edit compares it
  and stales the link-stage ops and each result that read a stock snapshot (B7, the air filter).
  `retract_saves_time` is the ONE feed-or-retract rule (W6.1 Waterline, the air-cut filter).
- F202: `$341` is `ControllerToolChangeMode`, `$32` is `ControllerMode` (`try_from` the number). The `$$` import lists an
  unknown value in `GrblImport::unknown_values` and leaves the field `None`. It is not `gcode::ToolChangeMode` (one export's handling).
- An arc is timed at its arc length (`move_length`, `hypot(r * sweep, dz)`), never its chord;
  both integrators read `move_length` / `move_directions`.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name: `kinematics_per_axis_rate_p1`,
`kinematic_utilization_p2`, `kinematic_surfacing_p4`, `a_downward_traverse_rounds_down_g_rpmdown`,
`power_ceiling_parity_f2`, `the_machine_head_guards_the_checks_h1` (H1).

## Do not

- Do not compare a wall clock across arms without a fresh measurement on the same profile.
