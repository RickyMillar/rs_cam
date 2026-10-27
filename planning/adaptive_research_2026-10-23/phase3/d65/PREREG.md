# D65 pre-registration, written before the change and before any run

Tree: geodesic-integration, base `7ee3d3226`. Design: `README.md` (this folder).

## The change

1. 2D (`adaptive/path.rs`): a `LinkLoad` field, the capped rule, that only the geodesic doors set (`geodesic/plan.rs::cleaned`, `geodesic/slice.rs::slice_replay`, through an explicit argument of the three `*_turns` / `replay_after_group_filter` functions; every other caller passes `false`).
   - With it, `emit_capped_walk_with` also admits a step over the ceiling c whose load L = max(swept width, area per mm) / D is at most `FULL_SLOT_LOAD` (1, the full-depth slot: the load of a straight plunge's departure, which the op already takes).
   - `geodesic/slice.rs::fed_runs` finds each such step on replay (swept reading over c, not a turn, not a departure). Its plan window becomes a run of its own, with factor c / L (`feed_factor`, existing).
2. 3D (`adaptive3d/clearing.rs`, the StockDepth large-Z split): for Geodesic Adaptive only (`is_geodesic`), a shape-flagged step that is not deep, with c < `step_load` ≤ `FULL_SLOT_LOAD`, and no engagement change at it, is not split.
   - It goes out as its own `Cut` between `FeedCapStart` and `FeedCapEnd`, with the factor min(c / L, plunge_rate · |step| / (|dz| · F)) and min'd with an open 2D cap.
   - A deep step, a step over 1, an engagement change and every non-geodesic strategy split as before.

No new tuned number: c, D, DPP, the plunge rate and F are the op's; `FULL_SLOT_LOAD` is the definition of a slot reading (1).

## Expected

- Only Geodesic Adaptive changes. CP, AgentSearch, Adaptive (2D-in-3D), ContourSpiral and 2D Adaptive are byte-identical:
  - `adaptive3d_emission_byte_parity` 5/5 plain and research;
  - `every_op_keeps_its_toolpath_g_opgolden` (it has no geodesic row);
  - the `adaptive/` sentries.
- Corne B s2.1, op 1 (to be measured by D10 after D64):
  - the 33 ledge "Reentry2d/Turn" re-entries (step refused, L 0.38–0.52) go;
  - the 63 "step refused" re-entries over the op go (all read ≤ 0.52);
  - of the 231 LargeZ load splits (`over_dpp=0`), those with `step_load ≤ 1` go. This includes the 64 wall-row splits and the 14 at the ledge, if their L ≤ 1 (expected; not logged).
  - Seconds: about 31 s of ledge entry descent, plus retracts and rapids. The LargeZ descents stay (now capped feed cuts at ≤ the plunge rate); their lift-overs and rapids go.
- Will move (re-bless approved for these only):
  - `geodesic_adaptive_keeps_the_research_geometry_s7_4`: each fixture's move count falls where a re-entry or a split went. A re-entry is at least a retract, a rapid and a plunge, so expect at least 3 fewer moves per removed re-entry. It may rise by the extra cap markers? No: markers are not moves. The exact counts are read after the change and reported as old → new with the first differing move.
  - `the_geodesic_preset_keeps_its_plan_s7_3` (research and test-support): the same.
  - `a_geodesic_cap_survives_every_dressup_s7_4` and `a_trace_lists_each_capped_move_s7_4c` must stay green: they check the caps survive, and there are only more caps. If they pin counts, report them.
  - `geodesic_adaptive_holds_the_pass_load_c12` (the C12 judge on Geodesic, step 7.7) will read steps over c that are now admitted with a cap. If it asserts "no step over c" without the cap exemption, it fails by design. Report it; do not change its bar without the lead.
