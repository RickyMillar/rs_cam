# `ui/components/` — the shared component set

One renderer per repeated UI element. `DESIGN_SPEC.md` gives the section
numbers each file cites.

## Files

- `mod.rs` — the component facade.
- `section.rs` — sections, param grids, the `UiExt` trait and `SummaryCard`.
- `card.rs`, `notice.rs` — `SectionHeader`, `Card`, `EmptyState`, `Banner`
  and `NoticeStack`.
- `kv_row.rs`, `value_row.rs` — `KeyValueRow`, `DataTable`, `ValueRow` and
  `row_hover_tint`.
- `chip.rs`, `pill.rs` — `StatusChip` and `CountPill`.
- `choice_row.rs` — `ChoiceRow`, the one closed-choice row (G-STARTFROM).
- `freshness.rs` — `Freshness` and `FreshnessGate`, the single staleness cue.
- `provenance.rs` — `ProvKind` and `ProvenanceBadge`.
- `precedence.rs` — `PrecedenceField`, a per-operation override over a
  project default.
- `suggest.rs`, `compare.rs` — `SuggestButton` and the current-versus-
  recommended comparison.
- `button.rs`, `focus_ring.rs`, `motion.rs`, `text.rs`, `format.rs` — the
  button, the keyboard focus ring, the motion helpers, the type rungs and
  the shared text formatting.

## Invariants

- A component is the ONE renderer for its element. A panel that draws its own
  chip or row breaks the kit.
- Staleness has one cue: `Freshness`. Provenance has one vocabulary:
  `ProvKind`. Do not add a second.
- A component reads a token from `ui/tokens.rs`. It holds no literal colour
  or spacing.

## Sentries

- `cargo test -p rs_cam_viz -q --test component_contracts_up2`
- `cargo test -p rs_cam_viz -q --test chrome_reads_the_kit_up3`
- `cargo test -p rs_cam_viz -q --test panels_read_the_token_module_up1`
- `cargo test -p rs_cam_viz -q --test the_toolpath_card_is_five_elements_dc1`
