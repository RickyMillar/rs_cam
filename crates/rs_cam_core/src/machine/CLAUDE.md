# `machine/` — the machine profile and its kinematics

The machine model that feeds, cycle time and the strategy choice read. The
principal type is `MachineProfile` in `mod.rs`.

## Files

- `mod.rs` — the machine profile: spindle, power, per-axis rates and accel.
  `head.rs` — the machine head and the dust shoe (H1); `HeadBand`, `HeadBandKind` (P22-0c: the machine owns them; `machine/` imports no `stock/`). `domain.rs` — F210: `MachineDimension`, `HeadDimension` (`domain` = math bounds, `ui_range` = GUI drag only).
- `kinematics.rs` — the kinematics model for cycle-time estimation.
- `kinematic_utilization.rs` — the machine-confidence instrument.
- `strategy_advisor.rs` — suggests the clearing strategy from the profile.

## Invariants

- The per-axis maximum rate is per axis. A single scalar rate over-estimates a diagonal move. F002: `cutting_ceiling()` (`CuttingCeilingMmMin`) caps a cut, `travel_rate()` (`TravelRateMmMin`) a rapid; the modulator takes the ceiling type. P16 (D9): no default cap; the ceiling is `max_cutting_feed_mm_min`, else the smaller kinematics X/Y rate, else the travel rate (`CuttingCeilingSource`; the last warns `CuttingCapUnstated` where it binds).
- P20c: every machine limit on every move. `rate_bound_feed_mm_min` / `rate_bound_feed_of_move` read the one projection (`effective_max_rate_mm_min`); `cap_feeds_at_axis_rates` runs once in generation (`JobPhases::run_stages`, after the entry split) and the modulator caps last; a per-axis rate change drops every result. The gates grade the reached feed (`use_predicted_feed_in_gates` default on; the modulation stamp writes the reached feed). Sentry `every_machine_limit_runs_through_every_move_p20c`.
- P20e: `op_cutting_ceiling(feed_max_rate)` is THE cut ceiling of one op (the modulator and the advisor cap at it); `op_steady_feed` = `min(op feed, that ceiling)` with its `SteadyFeedLimit` is the reference of the chip gate filter, the histogram and the Suggest power figure. Sentry `one_steady_feed_for_the_cap_the_gates_and_the_card_p20e`.
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
- P11c (D13 items 3, 6): every machine table states `class` (`MachineClass::{Hobby, Industrial}`); `MachineProfileWire` is the one class rule (hobby + no `[rigidity]` = `RigidityProfile::default()`, industrial + none refuses `INDUSTRIAL_NEEDS_RIGIDITY`). `RigidityProfile::default()` is the ONE set (hobby grade, Shapeoko port, Unsourced); the presets read it. `rigidity_basis().text()` is the text every surface prints beside a rigidity value. The three `woc_*` fields are deleted (no reader); P16 (D12) deleted `slot_doc_factor` (a retired key, refused in a project and a machine-library file). P13c (O1): `force_budget_n` (N) replaces `doc_roughing_factor` and `doc_finishing_factor` (retired keys); the hobby default is derived (`feeds::force_budget::hobby_budget`, the largest printed hobby slot force); `RigidityProfileWire` gives a hobby block with no budget the derived one and refuses an industrial one (`INDUSTRIAL_NEEDS_FORCE_BUDGET`); `depth_cap_rule` is the rule (Adaptive factor, else the budget); sentry `the_hobby_force_budget_recomputes_from_the_printed_rows_p13c`. Sentries `an_industrial_machine_states_its_rigidity_p11c`, `the_rigidity_default_is_hobby_grade_p11c`.
- An arc is timed at its arc length (`move_length`, `hypot(r * sweep, dz)`), never its chord;
  both integrators read `move_length` / `move_directions`.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name: `kinematics_per_axis_rate_p1`, `kinematic_utilization_p2`, `kinematic_surfacing_p4`, `a_downward_traverse_rounds_down_g_rpmdown`,
`power_ceiling_parity_f2`, `the_machine_head_guards_the_checks_h1` (H1), `a_value_outside_its_domain_is_refused_at_every_door_w6b` (F210). Do not compare a wall clock across arms without a fresh measurement on the same profile.
