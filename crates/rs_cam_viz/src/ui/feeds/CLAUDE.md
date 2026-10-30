# `ui/feeds/` — the Feeds tab chart and card, and the vendor data window

## Files

- `mod.rs` — the facade and the map. No feeds window holds the chart (W3i deleted Explore).
- `explore.rs` — the nomogram, inline on the Feeds tab (`draw_inline`, `draw_chart_c`); W3k: no navigation (every egui_plot zoom/drag/reset flag off, bounds set each frame), labels on the chart (`layout_labels`), hover = one line; `explore_tests.rs` the label fit test; the plot starts `AXIS_LABEL_GAP` of the title height in (up4 reads the rotated edge).
- `compare.rs` — the recommendation (`draw_recommendation`): problem rows, ONE `Details` "Recommended" with `⚡ Apply all` / `⚡ Speeds only` at its top (W3k); `why.rs` — the lines behind each number.
- `vendor.rs` — the vendor cutting data window (`Vendor data…`, MCP `set_ui_view modal: vendor_data`).
- `shared.rs` — what more than one feeds job needs; `OpFeeds` is the tab's inputs.

## Invariants

- No background lock: R1 core seeds an EMPTY feed field from Suggest (⚡). W3k: the operator's padlock (core `FeedsLock`, default Chipload) holds one of Feed/Spindle/Chipload; an edit writes the dependent field in the same frame.
- W3i: the Chipload row reads core `AdvancePerToothMm`. The chart and the card read
  `FeedsMemo::preview` / `::card` (F216); the panel host fills the memo on every tab (`refresh_feeds_memo`).
- The apply writes through the core command path (`Command::ApplyFeeds`, F114); the write stales the result.
- The recommended column is what `⚡ Apply all` writes; the hover has the raw value.
- The card is `DataTable` rows (IC2). The face holds one status row per problem (and
  a refusal); the apply buttons, every recipe row, the stages and the basis are in its ONE `Details`.
- Every stage that moves a number is ONE row with its source (ruling R4, in
  `Details`): `why::suggest_rows`, `why::row_basis_rows`. A problem is one
  status row on the face (`why::Placement`).
- A nomogram readout outside the axes abstains; the hover opens only on the plot. Recommended = ACCENT ring; Now = neutral dot (R23).
- A chart draws one line at the suggested value, and band lines only where the
  row publishes both limits; no shading.
- Power and the verdict read the cut Apply writes; power through
  `feeds::power_at_operating_point`, never `FeedsResult::power_kw` or a
  feed-scaled copy. Power reads 0 to `available_kw`; kW and source on hover.

## Sentries

Run each with `cargo test -p rs_cam_viz -q --test <name>`:
`the_feeds_modal_holds_one_scope_dc5a`, `the_corridor_bounds_the_band_g_corridor`,
`the_recommendation_explains_each_row_g_whyrow`, `the_nomogram_readout_abstains_g_hoverbound`,
`the_speeds_apply_holds_the_cut_g_speedsonly`, `the_chipload_verdict_is_one_row_g_chipverdict`,
`feeds_charts_draw_lines_not_shading_g_chartlines`,
`every_stage_that_moves_a_number_is_on_the_card_g_visible`,
`the_recommended_column_is_what_apply_writes_g_recomapplied`,
`a_claimed_row_states_its_claim_on_the_card_g_claimcard`, `the_card_names_every_entry_rule_g10`;
`--lib _w3i _w3k` (lock × edit, label fit), `the_feeds_memo_fills_on_every_tab_w3i`, `the_feeds_lock_holds_one_field_w3k`. Trap: an input can lose focus mid-typing.
