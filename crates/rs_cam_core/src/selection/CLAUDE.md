# `selection/` — items, selections, resolution

The pickable 2D geometry of every source and the selection of an op. Plan: `planning/geometry_selection_2026-10-16/`.

## Files

- `mod.rs` — the facade. `tests.rs` — the P2.2 lib sentries. `stock.rs` — `stock_items`. `item.rs` — `Item`, `Shape`,
  `FaceItem`, `ItemKind`, `SourceRef`, `ItemSet`; `key.rs` — `MatchKey`, `matches`, `names_same_geometry` (`KEY_MATCH_EPS_MM`).
- `rule.rs` — `Selection` (private fields, NO `Default`), `Rule`, `Filter`, `KindFilter`, `SelectionEdit`, `apply`.
- `resolve.rs` — `resolve` (pure; reports, never refuses), `ResolvedSelection`, `MatchedBy`,
  `to_setup_frame` + `face_in_setup_frame` (THE frame door), `face_top_z`.
- `view.rs` — `view`, `OpItems`, `Regions`/`Curves`/`Holes`, `Shape::serves`;
  `refusal.rs` — `SelectionRefusal`, `RefusalCause` (`fix()`), `ItemRef`, `SelectionNote`.

## Invariants

- `Item::new` is the only item constructor (checks shape, computes key). A
  circle is ONE item. Items keep import order; containment runs in `view`
  over the selected rings only; a face keeps its own inner loops.
- The session binds items to `SourceRef::Model(id)`. A pick names its
  source; `resolve` finds that item set, then compares geometry only.
- `KindFilter::Served` admits the drawing kinds only (no `Face`): a face enters by a pick or a rule that names it.
  `MatchedBy::Default` items the op cannot cut are left out with a `SelectionNote`; a pick or an `Only` rule match refuses `KindNotCut` (D2/D13).
- Two items with equal keys stay two items; a pick on them is ambiguous. Read no source file here.
- A face filter reads the SETUP frame (`ResolveScope::setup`); the frame door
  does not move a stock item. P2.3b: an op config holds `selection` (driver-set, per-op serde default);
  the session resolves `op.selection()` + face picks; `face_top_z` is setup frame. P2.5: `outlines_pre_selection` is a test reference only; `ItemSet::{closed_regions,
  drawing_curves}` share `view`'s nesting; a boundary is `BoundarySource::Selection`. P2.3c: drills read `Holes`; `KEY_MATCH_EPS_MM` is THE pick tolerance.
- P2.4: `ResolveScope.toolpath_model` is `Result<_, ModelId>` (a missing model is `MissingSource(Model(id))`); `view` refuses a face
  under `HORIZONTAL_FACE_MIN_NORMAL_Z` in the setup frame (`FaceNotFlat`; call it after the frame door). P2.6: a face pick is a `Face` pick of the op selection (`face_picks`, `names_faces`); `face_selection` is retired.
- `view` gives a `SelectionNote` for a skipped open path only; `OpItems::skipped_by_kind` counts every kind; `Empty` names every kind.
  `detect_containment` keeps an open path's direction. P2.7: ONE stored spelling per item: `edit_toolpath_selection` stores `Model(own id)` as `ToolpathModel` (`SourceRef::for_toolpath_model`), so an MCP key and a click are one pick.

## Sentries

- `cargo test -p rs_cam_core -q --lib selection`; `--test every_import_door_gives_items_p2_1` (also `--features step`)
- `--test default_selection_equals_the_old_lists_p2_2` (frozen lists); P2.3b `a_layer_rule_cuts_only_its_layer_p2_3`,
  `an_absent_selection_key_loads_the_op_default_p2_3`, `an_op_change_keeps_its_selection_p2_3`, `a_face_pick_cuts_from_its_setup_frame_top_p2_3`;
  P2.3c `drill_picks_resolve_to_targets_g_drillpickstale` (a lost pick refuses), `a_retired_drill_pick_key_is_named_p2_3c`;
  P2.4 `selection_refusals_name_the_item_p2_4` (one arm per cause; status = generation), `a_two_face_pick_keeps_both_faces_p2_4`; P2.5 `a_selection_boundary_confines_g_modeloutline`; P2.6 `selection_round_trips_p2_6`, `a_retired_selection_key_is_named_p2_6`; P2.7 `one_item_is_one_pick_p2_7`; P4.1 `a_3d_op_face_pick_sets_its_top_p4_1` (a 3D op's face pick is a boundary pick)
