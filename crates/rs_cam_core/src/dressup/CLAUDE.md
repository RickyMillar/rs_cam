# `dressup/` — post-generation transforms

The passes run after an op generates a toolpath. `apply_dressups` in
`compute/execute/dressup_apply.rs` drives them from a `DressupContext`;
`DRESSUP_PIPELINE` there lists every `DressupStep` in run order (F128: one fn and one exhaustive arm per step). Only these run outside
it: `apply_tabs`, in `ProfilePass::emit_level` of `compute/execute/clearing_2d.rs`
(each level below the tab top lifts, P5b), and `optimize_entry_descents_annotated` and
`adaptive_feed_modulate_with_materials`, in `session/compute.rs` (prior stock, engagement). 3D Rough calls the emitters `emit_helix` and `emit_ramp`. Add no other pass outside it.

## Files

- `mod.rs` — the configuration and the facade. `entry_descent.rs` — the ramp
  and helix emitters. `entry_audit.rs` — a fed move's burial audit.
- `link.rs` — link against retract. `arcfit.rs` — lines to arcs, within the
  tolerance in 3D (G-TIERBURIAL). `feedopt.rs`, `feed_modulation.rs`,
  `air_cut.rs`, `tsp.rs`. `tabs.rs` — tab placement on straight sides (P5), the lift from the bridge width (P5b) and `apply_tabs`; P4.6: `plan_profile_tabs` is THE placement (`ProfilePass::prepare` and the `tab_layout` query; it reads `ops::profile`) and `resolve_hand_tabs` the one hand-tab resolver. `condition.rs` — segment merge (roughing; off where
  `planner_applies_segment_merge`: 3D Rough, 2D Adaptive; G-PLANSIMGAP).

## Invariants

- G-RETRACTDIAL is closed (CUT-03): the retract COUNT is the lever. `filter_air_cuts` bridges an
  air run only when `machine::kinematics::retract_saves_time` says the retract is faster (10-15).
- Every linking arm calls the shared `relink_fragments` kernel in
  `finish/surface_link.rs`. Do not add a second linking implementation.
- A helix or ramp starts `entry_clearance_mm` over the material and takes
  the full depth at `ramp_feed_rate`; air is straight (rulings 09-24/25). A
  `helix_floor_lap` helix (2D Adaptive) ends on a flat lap and shrinks to
  fit `entry_containment`, arc fit inside it; under entry None a
  `stock_aware_plunge` op rapids through air first (G-ADAPTPASSLOAD).
- The modulator skips a plunge by geometry; the descent pass splits no entry retract (G-PECKSPLIT).
- `tsp.rs`: a reordered group's fed first move is never fed from elsewhere
  (G-TIERBURIAL); rapid travel never grows: a group keeps its order and
  framing unless its rebuild is shorter, else the input comes out as is.

## Sentries

- `cargo test -p rs_cam_core -q --features test-support --test capability_link_moves_safety --test dressup_span_invariants --test tabs_sit_on_straight_sides_p5 --test tabs_bridge_width_and_height_p5b --test the_tab_layout_is_the_cut_layout_p4_6`
- `... --test constrained_max_modulation_f039 --test entry_moves_stock_aware_g_rampterrain --test finishing_defaults_have_no_ramp_entry_r10 --test adaptive3d_entry_stock_aware`
- `cargo test -p rs_cam_core -q --test lead_in_out_feed_rates_f040 --test plunge_guard_p3 --test an_entry_ramp_follows_the_arc_g_rampchord` (a ramp laps on the arc)
- `... --test a_peck_ladder_is_never_split_into_a_rapid_into_stock_g_pecksplit --test fed_chords_ride_the_surface_g_tierburial`
