# `ui/components/` — the shared component set

One renderer per repeated UI element; `DESIGN_SPEC.md` gives the sections.

## Files

- `mod.rs` — the facade. `section.rs` — `UiExt`, param grids, `SummaryCard`.
- `card.rs`, `notice.rs` — `SectionHeader` (`small_caps`: 11 px letter-spaced caps, faint, no rule; a tab section label), `Card`, `EmptyState`, `Banner`,
  `NoticeStack`, `NotMeasured` (W6 `text`: a caption on the glyph line). W1: `NoticeStack::issues` + `show_issues` is the IssueStack (one line, glyph, truncated face, one `Notice::fix`, cap 3; a Danger outranks the cap; R1 overflow). `list_box.rs` — `ListBox` (W3g: inset list, `list_header` with the count at the right, `list_row` hover + separator; W3h: `Act::Button`/`Act::Menu` (`…` popup) at the right, header `Mark` (role glyph, hover), `list_placeholder`, `ListBox::rows` (scroll with a cap), `list_line` (no surface)). `notes_counter.rs` — `NotesCounter` ("ⓘ n", popover; 0 draws nothing). `data_table.rs` — `DataTable`, `Details` (IC2; P29-10: `show_held`, the open state held by the caller); a clickable row's labels are not selectable (P4.4).
- `prop_row.rs` — `PropRow`, THE field row (W1): label `LABEL_COL_WIDTH` muted, control in one well at `ROW_ACTION`,
  unit inside the well, `show_read_only`, one quiet `action` (+ `action_hover`; W3k `lock`: a painted padlock at the label column's right end); W3g: `numeric` = a `NUMBER_WELL_WIDTH` well with a fixed `UNIT_SLOT_WIDTH` slot (numbers right-aligned, one x), `checkbox` = the box alone, no well; `state(Role)` outline, hover = label + explanation + detail; W6: `PropRow::note` (a hint under a row), `well_value` (a value in a well); `label_cell_sized` is kit-private. `number.rs` (W3f) — THE numeric control: `NumberSpec`, `number_field`, `optional_number_field` (unset = empty text, "—" when it does not fit), `PropRow::number`/`optional_number`, `near_match`, `suggest_hover`.
  `kv_row.rs`, `value_row.rs` — COMPAT rows of the Setup-side panels (`KeyValueRow`, `ValueRow` (`enabled`: a disabled well, the row still ends its grid row), `OptionalValueRow`, `label_cell`); the inspector may not use them (`the_inspector_draws_only_kit_rows_w6`). Add no new caller. `chip.rs`, `pill.rs` — `Role`, `StatusChip`, `CountPill`; `role_of_tone` (P29-10) is THE map from a core `limits::Tone` to a `Role`, the one viz match on a row state.
- `status_line.rs` — `StatusLine` (W6): the inspector header line: word, meta, `NotesCounter`, one button; `keep_whole` for "Done · <danger>" (K6). `choice_row.rs` — `ChoiceRow` (G-STARTFROM). `line.rs` — `OneLine`, a note. `freshness.rs` — the staleness cue. `histogram.rs`, `sparkline.rs` — charts; `histogram.rs` is the style bar. `chart.rs` (P17) — `PlaneChart`, THE painter-drawn x/y chart (no `egui_plot`): `Axis` (a named edge, D5), `ChartLine`, `Mark`, `Label`, one `Mesh` fill, `ChartMap` (screen <-> data), `nice_steps`; `chart/labels.rs` the label kit (`place_labels`, `paint_labels`, `place_label`; P17b item 2: `LabelAnchor::LineEnd` = a label ON its line at its last point, then the first, then beside it, never tested against its own line; `LabelGround::Ground` = a chip of the chart ground `SURFACE_BASE`), which the histogram reads and `PlaneChart::show_with_labels` gives back. A kit module imports nothing from `ui/feeds`.
- `provenance.rs`, `precedence.rs` — `ProvKind`, `ProvenanceBadge`,
  `PrecedenceField`. `suggest.rs`, `compare.rs` — Suggest and the deltas.
- `tab_strip.rs` — `TabStrip` (IC3): ONE row always (full, compact, then a `»` menu for the tail; the active tab stays; a dot is painted in the tab corner and adds no width; lib `every_painted_glyph_is_in_the_loaded_fonts` in `glyph_tests.rs` holds each non-ASCII literal of the kit and the inspector to the loaded fonts); `focus_ring.rs` — the keyboard focus ring; `format.rs` — shared text formatting; `button.rs`, `motion.rs`, `text.rs`, etc.

## The rule: tabular data uses DataTable; derivation goes in Details

- Derived data (a recommendation, a reading, a verdict, a count) is rows of
  `DataTable`: name, value and unit, status (`Role`), source (`ProvKind` or
  a setting). One line per row; no cell wraps; the row hover explains.
- A recommendation row shows only when it differs at the printed precision.
- The default view: the values the operator sets, the differences, and one
  status row (`DataRow::note`) per problem. Derivation and provenance go in
  ONE closed `Details` per section; `Details::label` may name its content (F28). A chart sits above its table.
- A value not measured reads "N/A", no chip; the reason opens the hover (IC3).
- Units: mm 2 decimals (coordinates 3), mm/tooth 4, mm/min and RPM 0, % 0-1.

## Invariants

- One renderer per element; it reads `ui/tokens.rs`. Staleness has one cue,
  `Freshness`; provenance one vocabulary, `ProvKind`.
- A long-job button reports through `Button::progress`; it paints its label once. W2c: a disabled `Button` of every variant takes `Variant::disabled_fill`/`disabled_text` (no accent); `disabled_reason` is its hover.

## Sentries (`cargo test -p rs_cam_viz -q --test <name>`)

`component_contracts_up2`, `chrome_reads_the_kit_up3`, `panels_read_the_token_module_up1`,
`tabular_data_reads_one_table_ic2`, `the_sim_inspector_is_tabbed_ic3`,
`the_toolpath_card_is_five_elements_dc1`, `cut_metric_cards_read_the_gate_g_cutcards`, `the_issue_stack_is_capped_w6` (the cap), `inspector_header_wraps_g_reachwrap` (the status line); W1 kit: `--lib components` (`_w1`).
