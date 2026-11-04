# `ui/feeds/` — the envelope plot, the chart and the card of the Passes tab, and the vendor data window

## Files

- `mod.rs` — the facade and the map. No feeds window holds the chart (W3i deleted Explore). P5b: the Feeds tab merged into Passes.
- `envelope.rs` (K2b, P5b, P5c) — `draw_envelope`: header `SAFE CUT` + ⚡ chip, the (ae, ap) plot (`EnvelopeScene`; the frame is the hard stops: width 0..D, depth never past the cutting length or flute guard; the view never follows the candidate; a line only for the limit that binds, a crossed limit or a limit near a mark; Runtime only when it binds; each drawn line `TEXT_BODY` 2 pt over a `BORDER` grid with ONE short-name label (`place_labels`); colour rule above `setpoint`: crossed line + outside dot `CAUTION`, no DANGER; P5c round 2: a drag moves the WHITE dot (the setpoint draft), a `TEXT_MUTED` hollow ghost marks the stored point while a candidate exists; ACCENT = only the proposal diamond, drawn when not at Now) or a one-row bar (`SAFE DEPTH`/`SAFE WIDTH`, same marks and colours); a click or drag sets the candidate (`state.panels.envelope_candidates`, no write; a drag off the plot holds it on the frame edge); ⚡ sends `AppEvent::ApplyFeedsPick` (candidate, else proposal); Esc drops it; hover = the line name + `readout`.
- `explore.rs` — the nomogram, inline on the Passes tab (`draw_inline`, `draw_chart_c`; P5b: it draws the `candidate_speeds` of the candidate ring); the label kit (`place_labels`, `paint_labels`, `axis_title_overhang`) is shared through `shared.rs` with the envelope plot; both charts read the tick rule `shared::nice_steps` (P5c: the feed axis names its ticks); W3k: no navigation (every egui_plot zoom/drag/reset flag off, bounds set each frame), labels on the chart (`layout_labels`: along each line, never across a drawn line, ≤ `MAX_LABELS`; a moved mark label takes a leader), no past-cap axis marks, Min RPM = an amber wall, hover = one line; `explore_tests.rs` the label fit test; the plot starts `AXIS_LABEL_GAP` of the title height in (up4 reads the rotated edge).
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
- A nomogram readout outside the axes abstains; the hover opens only on the plot. Recommended = ACCENT ring; Now = neutral dot (R23). The memo builds the envelope (`FeedsMemo::envelope`); a frame only reads it (F216).
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
`feeds_charts_draw_lines_not_shading_g_chartlines`, `every_stage_that_moves_a_number_is_on_the_card_g_visible`,
`the_recommended_column_is_what_apply_writes_g_recomapplied`,
`a_claimed_row_states_its_claim_on_the_card_g_claimcard`, `the_card_names_every_entry_rule_g10`; P5b: `the_envelope_plot_marks_the_operator_and_the_proposal_k2b`, `the_envelope_hover_abstains_outside_k2b`, `a_plot_click_writes_nothing_until_apply_k2b`, `a_one_axis_op_draws_a_bar_k2b`; P5c: `the_envelope_plot_shows_only_what_limits_the_cut_p5c`;
`--lib _w3i _w3k` (lock × edit, label fit), `the_feeds_memo_fills_on_every_tab_w3i`, `the_feeds_lock_holds_one_field_w3k`. Trap: an input can lose focus mid-typing.
