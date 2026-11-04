# `dressup/` — post-generation transforms

The passes run after an op generates a toolpath. `apply_dressups` in
`compute/execute/dressup_apply.rs` drives them from a `DressupContext`;
`DRESSUP_PIPELINE` there lists every `DressupStep` in run order (F128: one fn and one exhaustive arm per step). Only these run outside
it: `apply_tabs`, in `ProfilePass::emit_level` of `compute/execute/clearing_2d.rs`
(each level below the tab top lifts, P5b), and `optimize_entry_descents_annotated` and
`adaptive_feed_modulate_with_materials`, in `session/compute.rs` (prior stock, engagement). 3D Rough calls the emitters `emit_helix` and `emit_ramp`. Add no other pass outside it.

## Files

- `mod.rs` — the configuration and the facade. `entry_descent.rs` — the ramp and helix emitters. `entry_audit.rs` — a fed move's burial audit.
  `lead_side.rs` — the side of a lead arc (P6.0c, SAFETY): away from the `RingMaterial` the op states per closed pass (`Produced::closed_passes`); an arc off the air side keeps the plunge; no side and no probe: no lead.
- `link.rs` — link against retract. `arcfit.rs` — lines to arcs, within the
  tolerance in 3D (G-TIERBURIAL). `feedopt.rs`, `feed_modulation.rs`,
  `air_cut.rs`, `tsp.rs`. `tabs.rs` — tab placement on straight sides (P5), the lift from the bridge width (P5b) and `apply_tabs`; P4.6: `plan_profile_tabs` is THE placement (`ProfilePass::prepare` and the `tab_layout` query; it reads `ops::profile`) and `resolve_hand_tabs` the one hand-tab resolver. `condition.rs` — segment merge (roughing; off where
  `planner_applies_segment_merge`: 3D Rough, 2D Adaptive; G-PLANSIMGAP; P6.0b: on a `finished_edge` op at most `FlattenPolicy::UNTOLERANCED_MM`).

## Invariants

- G-RETRACTDIAL is closed (CUT-03): the retract COUNT is the lever. `filter_air_cuts` bridges an
  air run only when `machine::kinematics::retract_saves_time` says the retract is faster (10-15).
- Every linking arm calls the shared `relink_fragments` kernel in `finish/surface_link.rs`. Do not add a second linking implementation.
- `ModulationContext::cutting_ceiling` is `CuttingCeilingMmMin`, made only by `MachineProfile::cutting_ceiling` (F002). G-FEEDMAX: the session lowers it to the op's `feed_max_rate` (`capped_at`, only lowers), so the dial caps the modulator as it caps feedopt. F195: the folder denies `cast_possible_truncation` and `cast_sign_loss`; a count from a user length goes through `util::steps`, or its `#[expect]` reason names the bound.
- A helix or ramp starts `entry_clearance_mm` over the material and takes
  the full depth at `ramp_feed_rate`; air is straight (rulings 09-24/25). A
  `helix_floor_lap` helix (2D Adaptive) ends on a flat lap and shrinks to
  fit `entry_containment`, arc fit inside it; under entry None a
  `stock_aware_plunge` op rapids through air first (G-ADAPTPASSLOAD). G-CENTRECUT: `EntrySafety::centre_limits` comes from the tool (`MillingCutter::non_centre_cutting`). With it the emitters clamp the ramp angle and the helix pitch to the limit, ignore `contact_top` (the conservative top only), and return a `NonCentreCuttingRefusal` for a fallback plunge deeper than the protrusion; `apply_entry` puts it in `Produced`. Entry None becomes the ramp for such a tool on a `Dressup` contract only, unless the planner made its own entries; a `StraightPlunge` op keeps its plunges, and the feasibility stage warns on each one deeper than the protrusion (K5 P7 C). K5 P7 C: under the G-RAMPTERRAIN clip a `No` tool re-plans the ramp over the clip floor at its limit slope (`replan_descent_at_limit`: only up, never below the floor; other directions before the plunge fallback). No emitter keeps its own formula.
- K4 (feeds contract P3): the air lead above an entry is `MoveIntent::EntryApproach` (`rapid_to_entry_top`, the `emit_ramp` lap ladder, the G-ISOCLIPENTRY rest ramp in `optimize_entry_descents`, the Pencil crease ramp). Every reader that names `EntryPlunge` names `EntryApproach` in the same arm (the descent split, the modulator skip, `is_entry_intent`), so the G-code does not move; only the span label differs (`compute/spans.rs`: "entry approach"). Sentry `--test an_air_lead_is_an_approach_not_a_plunge_k4`.
- The modulator skips a plunge by geometry; K7 (P9): its power and deflection caps build one `feeds::engagement::Engagement` per move from `ModulationCutter` (shape, D, shank, helix), so a V-bit reads its triangular section; P11a: `d_eng` is `effective_diameter` at the MOVE depth, and the deflection cap is `fz_cap_for_peak_force` (the peak engaged-edge force); the descent pass splits no entry retract (G-PECKSPLIT). C38 §15: `Move::feed_cap` binds last in every feed stage (modulator, feedopt) and only lowers; arc fit and segment merge join only moves with equal caps.
- `tsp.rs`: a reordered group's fed first move is never fed from elsewhere
  (G-TIERBURIAL); rapid travel never grows: a group keeps its order and
  framing unless its rebuild is shorter, else the input comes out as is.

## Sentries

- `cargo test -p rs_cam_core -q --features test-support --test capability_link_moves_safety --test dressup_span_invariants --test tabs_sit_on_straight_sides_p5 --test tabs_bridge_width_and_height_p5b --test the_tab_layout_is_the_cut_layout_p4_6`
- `... --test constrained_max_modulation_f039 --test entry_moves_stock_aware_g_rampterrain --test finishing_defaults_have_no_ramp_entry_r10 --test adaptive3d_entry_stock_aware`
- `cargo test -p rs_cam_core -q --test lead_in_out_feed_rates_f040 --test plunge_guard_p3 --test a_feed_cap_binds_every_feed_stage_c38 --test an_entry_ramp_follows_the_arc_g_rampchord` (a ramp laps on the arc); `--lib a_non_centre_cutting_tool_ramps_at_its_limit_g_centrecut`
- `... --test a_peck_ladder_is_never_split_into_a_rapid_into_stock_g_pecksplit --test fed_chords_ride_the_surface_g_tierburial --test a_finished_edge_keeps_its_corner_p6_0b --test a_lead_curls_away_from_the_part_p6_0c`
