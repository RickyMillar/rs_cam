# `selection/` — the items of a model

The pickable 2D geometry of every source. The principal types are `Item`
and `ItemSet`; the plan is `planning/geometry_selection_2026-10-16/`.

## Files

- `mod.rs` — the facade.
- `item.rs` — `Item`, `Shape`, `FaceItem`, `ItemKind`, `SourceRef`,
  `ItemSet` (`layers`, `bbox`, `iter`, `scaled`, `bound_to`).
- `key.rs` — `MatchKey` and the match rule (`KEY_MATCH_EPS_MM`).
- `stock.rs` — `stock_items`, the stock outline as one `Loop`.

## Invariants

- `Item::new` is the only constructor. It checks the shape and computes
  the key. A `Loop` is closed with 3 or more points and holds no holes; a
  `Path` is open with 2 or more points. A circle is ONE item.
- `Item::new` does not change a ring: no winding fix, no dedup. A source
  calls `ensure_winding` where the import did before P2.1.
- The items keep import order. Containment does not run at import.
- The session binds the items of a model to `SourceRef::Model(id)` where
  the id is set: `model_from_geometry`, `add_model_impl`, the builder and
  `adopt_geometry`.
- A face normal is signed (from the face triangles). The key has no Z.
- P2.3 deletes the two bridges on `LoadedModel`:
  `outlines_pre_selection` and `drill_targets_pre_selection`. Add no
  third bridge.

## Sentries

- `cargo test -p rs_cam_core -q --test every_import_door_gives_items_p2_1`
- `cargo test -p rs_cam_core -q --features step --test every_import_door_gives_items_p2_1`
- `cargo test -p rs_cam_core -q --lib selection`

## Do not

- Do not read a source file here. A door in `io/` builds the items.
