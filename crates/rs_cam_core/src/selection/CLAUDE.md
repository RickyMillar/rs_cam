# `selection/` — items, selections, resolution

The items of every source and the selection of an op. Plan: `planning/geometry_selection_2026-10-16/`.

## Files
- `mod.rs` facade; `tests.rs` P2.2 lib sentries; `stock.rs` `stock_items`; `key.rs` `MatchKey`, `matches`, `CAD_ROUNDING_MM` (L-13); `item.rs`
  `Item`, `Shape`, `FaceItem`, `ItemKind`, `SourceRef`, `ItemSet` (`has_selectable_item`: the 2D gate).
- `rule.rs` — `Selection` (private fields, NO `Default`), `Rule` (`layer_rules`), `Filter`, `KindFilter`,
  `SelectionEdit` (`add_rules`: THE replace-under-default rule); `near.rs` — THE pick rule (`polyline_distance`).
- `join.rs` — THE join (`CLOSE_TOL_MM`); `drawing_edit.rs` — the ledger; `flat_regions.rs` — STL flat regions as faces (`FLAT_REGION_Z_BAND_MM`, L-7; Z order, then lowest triangle).
- `resolve.rs` — `resolve` (pure; reports, never refuses), `face_top_z`, `to_setup_frame` (THE frame door; F013: `ResolvedSelection<ModelFrame | SetupFrame>`, `to_setup_frame(Option<&SetupTransformInfo>)` gives `<SetupFrame>`, `face_top_z`/`face_bottom_z` exist only on `<SetupFrame>`); `view.rs` — `view`, `view_boundary`, `OpItems`, `Regions` (`is_stock`, `exterior_item`), `Curves`, `Holes`; `refusal.rs` — `RefusalCause`; `hole_fit.rs` — `hole_fit`, `fit_holes` (P6.5); `extend.rs` — `extend_to_stock` = `extend_ends` (L-3 reuse) + `close_to_stock`, `PathSide`, `StockXy`, `CannotExtend` (P6.4).

## Invariants
- `Item::new` is the only item constructor; it checks the shape and computes the key. A circle is ONE item. Items keep import order. Two items with equal keys stay two; a pick on them is ambiguous.
- The session binds items to `SourceRef::Model(id)`; `resolve` compares geometry only (`KEY_MATCH_EPS_MM`).
  The command stores `Model(own id)` as `ToolpathModel`: a key and a click are one pick. Read no file here.
- `view` nests the selected rings only (`ItemSet::closed_regions`, `drawing_curves` share it). A face keeps its
  holes; an open path its direction. The stock ring nests as a drawing ring; an island is a ring that
  `nest_closed_rings_at` consumed as a hole.
- `KindFilter::Served` admits no `Face`. A default item that the op cannot cut is left out (a `SelectionNote`
  for an open path only); a named item refuses `KindNotCut` (D2/D13); `Empty` names every kind. A face serves `Curves`
  (`curves_of`); drills read `Holes`; `outlines_pre_selection` is a test reference only.
- A face reads the SETUP frame (`face_in_setup_frame`); the frame door does not move a stock item. `view` runs after
  it: a face under `HORIZONTAL_FACE_MIN_NORMAL_Z` refuses `FaceNotFlat`. `Facing::admits` is THE facing rule (L-9);
  `face_top_z` reads UP faces only; `view` refuses a down `Regions` face (`FaceFacesDown`), `view_boundary` keeps it. `Regions::plane_z` (setup frame) and `face_bottom_z` feed the "each face" floor. Only `compute::floor::region_floors` gives `NoFaceForFloor` and `FloorAboveTop`; it removes a face at the top (`OpItems::skip_regions`, role `Skipped`) and gives a `SelectionNote` of `NoteCause::AtTheTop` that names that one face (`first`).
- P6.5 (O-3): `hole_fit` is THE size rule (`DrillSize`: the tool diameter and the Drill `size_tolerance`, default `CAD_ROUNDING_MM`, P6.5b; `LeavesCore` first); `fit_holes` runs after `view` (it needs the tool) in the prologue, the drill-op view and `selection_status` (`StatusKey.drill_size`). `HoleFitRule::ToolSize` (Drill): a default misfit circle is `Skipped` with one `NoteCause::HoleFit` note each (L-18); a named one refuses `HoleDoesNotFit`. Pin Drill: `None`. P6.6: `LargerThanTool` (Bore) keeps `Larger` only (`HoleFitRule::keeps`); a point does not serve it (`ItemKind::serves`); `HoleDoesNotFit` holds the `rule`, and each fix text reads (rule, fit).
- P6.4 (O-2): `RegionsExtendOpen { side }` (Pocket `open_paths`) closes each served path by the stock edge in `view` (`extended_regions`): the stock is `ResolvedSelection::stock_xy` (SETUP frame only, from `ResolveScope.stock_bbox`; a model-frame view refuses `NoStock`), grown by the tool radius (`with_stock_grown`) in the prologue and `selection_status` (`StatusKey.extend_radius`); a mirroring door (`mirrored`, `is_z_flipped`) flips the side. The path is the exterior (`Cut`); a ring inside the area is its island; a ring that touches it refuses `CrossesItem`; each area nests alone (L-10). Every 3.4 row refuses by the path, default rule too.
- `ResolveScope.mesh_faces` gives STL faces; a cold set gives `faces_not_ready` (no lost pick, no build), and `view`
  refuses `FacesNotReady`. A missing model is `MissingSource(Model(id))`. `FacesByFacing` counts the report faces.
- The ledger is in FILE units (the join tolerance frame, `io/CLAUDE.md`); `DrawingEdit::scaled` converts, and
  the replay runs in model mm. A failed edit is an `EditLost` outcome, never dropped.

## Sentries
- `cargo test -p rs_cam_core -q --lib selection`; `--lib p4_5`; `--lib p4_9`; `--test every_import_door_gives_items_p2_1` (also `--features step`)
- `--test`: `default_selection_equals_the_old_lists_p2_2` (frozen), `a_layer_rule_cuts_only_its_layer_p2_3`, `an_op_change_keeps_its_selection_p2_3`,
  `an_absent_selection_key_loads_the_op_default_p2_3`, `a_face_pick_cuts_from_its_setup_frame_top_p2_3`, `a_retired_drill_pick_key_is_named_p2_3c`,
  `drill_picks_resolve_to_targets_g_drillpickstale`, `selection_refusals_name_the_item_p2_4` (status = generation, except `NoSuchRule`, `FacesNotReady`),
  `a_two_face_pick_keeps_both_faces_p2_4`, `a_selection_boundary_confines_g_modeloutline`, `selection_round_trips_p2_6`, `one_item_is_one_pick_p2_7`,
  `a_retired_selection_key_is_named_p2_6`, `a_down_face_gives_no_top_and_no_floor_p3_4`, `a_3d_op_face_pick_sets_its_top_p4_1`, `a_face_op_reads_its_selection_p4_2`, `item_states_match_the_cut_p4_4`,
  `import_reports_joins_and_open_shapes_p4_8`, `drawing_edits_replay_on_reimport_p4_9`, `stl_flat_regions_are_face_items_p4_7` (`--features test-support`), `a_drill_drills_only_the_tool_size_p6_5`, `a_bore_cuts_the_stated_size_p6_6`, `a_pocket_extends_an_open_path_to_the_stock_p6_4` (`--lib selection::extend`)
