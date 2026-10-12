# `ui/feeds/` — the Explore window and the per-operation inspector card

## Files

- `mod.rs` — the facade: the Explore window AND the inspector card.
- `window.rs` — the Explore window frame; `explore.rs` — the nomogram.
- `compare.rs` — compare and apply; `why.rs` — the lines behind each number.
- `shared.rs` — what more than one feeds job needs.

## Invariants

- The Feeds tab NEVER auto-locks a numeric field; Suggest is an explicit button.
- The apply writes through the core command path; the write stales the result.
- The recommended column is what `⚡ Apply all` writes; the hover has the raw value.
- The card is `DataTable` rows (IC2). A change row shows on the face only when
  it differs; level rows, the stages and the basis go in the card's `Details`.
- Every stage that moves a number is ONE row with its source (ruling R4, now
  in `Details`): `why::suggest_rows`, `why::row_basis_rows`. A problem is one
  status row on the face (`why::Placement`).
- A nomogram readout outside the band abstains; the window fits the screen.
- A chart draws one line at the suggested value, and band lines only where the
  row publishes both limits; no shading.
- Power and the verdict read the cut Apply writes; power through
  `feeds::power_at_operating_point`, never `FeedsResult::power_kw` or a
  feed-scaled copy. Power reads 0 to `available_kw`; kW and source on hover.

## Sentries

Run each with `cargo test -p rs_cam_viz -q --test <name>`:
`the_feeds_modal_holds_one_scope_dc5a`, `the_feeds_window_fits_the_screen_g_feedsfit`,
`the_recommendation_explains_each_row_g_whyrow`, `the_nomogram_readout_abstains_g_hoverbound`,
`the_speeds_apply_holds_the_cut_g_speedsonly`, `the_chipload_verdict_is_one_row_g_chipverdict`,
`feeds_charts_draw_lines_not_shading_g_chartlines`,
`every_stage_that_moves_a_number_is_on_the_card_g_visible`,
`the_recommended_column_is_what_apply_writes_g_recomapplied`,
`a_claimed_row_states_its_claim_on_the_card_g_claimcard`, `the_card_names_every_entry_rule_g10`.

## Do not

- An input can lose focus mid-typing; a wrong-looking value can be that bug.
