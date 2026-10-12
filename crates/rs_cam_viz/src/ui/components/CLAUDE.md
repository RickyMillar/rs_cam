# `ui/components/` — the shared component set

One renderer per repeated UI element; `DESIGN_SPEC.md` gives the sections.

## Files

- `mod.rs` — the facade. `section.rs` — `UiExt`, param grids, `SummaryCard`.
- `card.rs`, `notice.rs` — `SectionHeader`, `Card`, `EmptyState`, `Banner`,
  `NoticeStack`, `NotMeasured`. `data_table.rs` — `DataTable`, `Details` (IC2).
- `kv_row.rs`, `value_row.rs` — `KeyValueRow`, `ValueRow`, `label_cell`,
  `row_hover_tint`. `chip.rs`, `pill.rs` — `Role`, `StatusChip`, `CountPill`.
- `choice_row.rs` — `ChoiceRow` (G-STARTFROM). `line.rs` — `OneLine`, a note.
- `freshness.rs` — the staleness cue. `histogram.rs`, `sparkline.rs` — charts.
- `provenance.rs`, `precedence.rs` — `ProvKind`, `ProvenanceBadge`,
  `PrecedenceField`. `suggest.rs`, `compare.rs` — Suggest and the deltas.
- `tab_strip.rs` — `TabStrip` (IC3); `button.rs`, `motion.rs`, `text.rs`, etc.

## The rule: tabular data uses DataTable; derivation goes in Details

- Derived data (a recommendation, a reading, a verdict, a count) is rows of
  `DataTable`: name, value and unit, status (`Role`), source (`ProvKind` or
  a setting). One line per row; no cell wraps; the row hover explains.
- A recommendation row shows only when it differs at the printed precision.
- The default view: the values the operator sets, the differences, and one
  status row (`DataRow::note`) per problem. Derivation and provenance go in
  ONE closed `Details` per section. A chart sits above its table.
- A value not measured reads "N/A", no chip; the reason opens the hover (IC3).
- Units: mm 2 decimals (coordinates 3), mm/tooth 4, mm/min and RPM 0, % 0-1.

## Invariants

- One renderer per element; it reads `ui/tokens.rs`. Staleness has one cue,
  `Freshness`; provenance one vocabulary, `ProvKind`.
- A long-job button reports through `Button::progress`; it paints its label once.

## Sentries (`cargo test -p rs_cam_viz -q --test <name>`)

`component_contracts_up2`, `chrome_reads_the_kit_up3`, `panels_read_the_token_module_up1`,
`tabular_data_reads_one_table_ic2`, `the_sim_inspector_is_tabbed_ic3`,
`the_toolpath_card_is_five_elements_dc1`, `cut_metric_cards_read_the_gate_g_cutcards`.
