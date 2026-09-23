# `ui/feeds/` — the feeds and speeds surfaces

The Explore window and the per-operation inspector card; entry `ui::feeds::mod`.

## Files

- `mod.rs` — the facade: the Explore window AND the inspector card.
- `window.rs` — the Explore window frame and its size policy.
- `explore.rs` — the feed-versus-RPM nomogram.
- `compare.rs` — compare and apply, the per-operation job.
- `why.rs` — the sentences that explain one recommended number.
- `shared.rs` — what more than one feeds job needs.

## Invariants

- The Feeds tab NEVER auto-locks a numeric field; the operator applies a
  recommendation through an explicit Suggest button.
- The apply writes through the core command path; the write stales the result.
- A nomogram readout outside the measured band must abstain, not extrapolate.
- The window fits the screen at the smallest supported size.
- A chart draws one line at the suggested value and fainter band lines
  only when the row publishes both limits; no shading, no invented width.
- Power reads through `feeds::power_at_operating_point` on the operation, never
  `FeedsResult::power_kw` or a feed-scaled copy. The card's power row is 0 to
  `PowerFigure::available_kw`; the kW pair and the provenance are on its hover.

## Sentries

- `cargo test -p rs_cam_viz -q --test the_feeds_modal_holds_one_scope_dc5a`
- `cargo test -p rs_cam_viz -q --test the_feeds_window_fits_the_screen_g_feedsfit`
- `cargo test -p rs_cam_viz -q --test the_recommendation_explains_each_row_g_whyrow`
- `cargo test -p rs_cam_viz -q --test the_nomogram_readout_abstains_g_hoverbound`
- `cargo test -p rs_cam_viz -q --test the_speeds_apply_holds_the_cut_g_speedsonly`
- `cargo test -p rs_cam_viz -q --test the_chipload_verdict_is_one_row_g_chipverdict`
- `cargo test -p rs_cam_viz -q --test feeds_charts_draw_lines_not_shading_g_chartlines`

## Do not

- A numeric input can lose focus while the operator types. A wrong-looking
  value is sometimes that bug, not operator error.
