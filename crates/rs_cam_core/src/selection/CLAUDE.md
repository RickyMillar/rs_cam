# `selection/` — items, selections, resolution

The pickable 2D geometry of every source and the selection of an op. The
plan is `planning/geometry_selection_2026-10-16/` (DESIGN + PLAN rulings).

## Files

- `mod.rs` — the facade. `tests.rs` — the P2.2 lib sentries. `stock.rs` — `stock_items`.
- `item.rs` — `Item`, `Shape`, `FaceItem`, `ItemKind`, `SourceRef`, `ItemSet`;
  `key.rs` — `MatchKey`, `matches`, `names_same_geometry` (`KEY_MATCH_EPS_MM`).
- `rule.rs` — `Selection` (private fields, NO `Default`), `Rule`, `Filter`,
  `KindFilter`, `SelectionEdit`, `Selection::apply`.
- `resolve.rs` — `resolve` (pure; reports, never refuses), `ResolvedSelection`,
  `MatchedBy`, `to_setup_frame` + `face_in_setup_frame` (THE frame door).
- `view.rs` — `view`, `OpItems`, `Regions`/`Curves`/`Holes`, `Shape::serves`;
  `refusal.rs` — `SelectionRefusal`, `RefusalCause` (`fix()`), `ItemRef`, `SelectionNote`.

## Invariants

- `Item::new` is the only item constructor (checks shape, computes key). A
  circle is ONE item. Items keep import order; containment runs in `view`
  over the selected rings only; a face keeps its own inner loops.
- The session binds items to `SourceRef::Model(id)`. A pick names its
  source; `resolve` finds that item set, then compares geometry only.
- `KindFilter::Served` admits the drawing kinds only (no `Face`): a face
  enters by a pick or a rule that names it. `MatchedBy::Default` items the
  op cannot cut are left out with a `SelectionNote`; a pick or an `Only`
  rule match refuses `KindNotCut` (lead ruling D2/D13).
- Two items with equal keys stay two items; a pick on them is ambiguous.
- Do not read a source file here. A door in `io/` builds the items.
- A face filter reads the SETUP frame (`ResolveScope::setup`); the frame door
  does not move a stock item. P2.3b/c delete the two `*_pre_selection` bridges.
- `view` gives a `SelectionNote` for a skipped open path only; `OpItems::skipped_by_kind`
  counts every kind; `Empty` names every kind. `detect_containment` keeps an open path's direction.

## Sentries

- `cargo test -p rs_cam_core -q --lib selection`; `--test every_import_door_gives_items_p2_1` (also `--features step`)
- `cargo test -p rs_cam_core -q --test default_selection_equals_the_old_lists_p2_2`
  (the frozen difference lists name the open points of P2.3)
