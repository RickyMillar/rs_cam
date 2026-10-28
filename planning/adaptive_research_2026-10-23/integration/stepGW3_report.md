# Step GW3: the Simulation air-cut row names the skin

Implementer: I4. Date: 2026-10-28. Base: `2833d66bc`. This is the lead's
W3 item: I5's W3 (`5a80bd1b2`) gives
`SimulationCutTrace::air_cut_removing_share`. The true air is
`air × (1 − share)`, and the skin is `air × share`. The figure changes in
place, with no new line.

Patch: `stepGW3_tree_vs_2833d66bc.patch` (one file:
`ui/sim_diagnostics.rs`).

## What changed

- The Simulation inspector's Warnings tab has the Informational table. Its
  `Air cut` row value comes from `air_cut_value(air_pct, skin_share)`:
  - with share 0: `12% of total runtime` (the value of today);
  - with a share above 0: `14% (+71% skin)`, the true air and the skin, as
    shares of the TOTAL runtime.
- The row hover gains one line, `air_skin_note`, only when there is a skin:
  "Skin: 71% of the total runtime is flagged air but removes material, cut
  under 2% WOC by the flank above the tip. The first figure is the air
  that removes nothing." The 2 % comes from core
  `AIR_CUT_RADIAL_WOC_FRACTION`, not from a literal.
- The share is `trace.air_cut_removing_share(None)`. The row reads
  `trace.summary`, which is the whole run, so the share is the whole run's
  too. A per-toolpath share (`Some(id)`) belongs to a per-toolpath figure;
  this table has none.

## A change from the lead's form

The lead's form `14% (+74% skin cut under 2% WOC)` does NOT fit. I
measured it in the value column of a `DataTable` at 288 points, which is
the inside of the 304-point Simulation inspector. Elided (cut short) or
fitting:

| Value | Fits? |
|---|---|
| `14% (+71% skin cut under 2% WOC)` | no (elided) |
| `14% (+71% skin under 2% WOC)` | no (elided) |
| `14% (+71% skin, WOC < 2%)` | yes, but a 3-digit figure would not |
| `14% (+71% skin)` | yes |
| `14% of total runtime` (today) | yes |

So the face is `14% (+71% skin)`, and the WOC words are on the hover (rule
2: the explanation goes behind a hover). With a skin, the face no longer
says "of total runtime". The hover's denominator line still says it, and
the LH-1 sentry stays green.

## Line count

No change: the same row, in place. The Informational table keeps its two
rows. The hover gains one line, and only when there is a skin.

## Tests

New `--lib the_air_row_names_the_skin_w3`:

- It uses the W3 core test's D69 Corne split (279.3 s of 335.9 s of air
  removes material) at 84.8 % air. The value is `14% (+71% skin)`.
- The hover note names `71% of the total runtime` and `under 2% WOC`.
- With share 0, the value is `12% of total runtime` and the note is empty.
- A `DataTable` render at 288 points shows the value on one row, not
  elided.

## Gates

All cargo runs went through `scripts/cargo_lane.sh`, in the foreground.

- `--lib the_air_row_names_the_skin_w3`: passed.
- `--test` sentries, all passed, run before core broke (see below):
  - `air_cut_labels_name_their_denominator_lh1`
  - `the_sim_inspector_is_tabbed_ic3` (10)
  - `the_simulation_page_is_summary_first_dc6`
  - `panels_read_the_token_module_up1`
  - `ui_string_hygiene`
- `cargo fmt -p rs_cam_viz -- --check`: clean.
- Once core compiled again, the first clippy run found one lint in my
  test: `clippy::panic` (an `unwrap_or_else(|| panic!(..))`; this test
  module allows no panic). The test now asserts with `is_some_and`. I
  re-froze the patch, so the md5 changed.
- Then `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`:
  clean.
- `cargo test -p rs_cam_viz --lib` (`-j 2`): 607 passed, 2 ignored.
