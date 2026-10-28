# Step GH: the Waterline label reads "Helical:"

Implementer: I4. Date: 2026-10-28. Base: `aaa0496d6` (G-C). This is the
lead's item after W1b (`WaterlineConfig::continuous` is renamed `helical`).

Patch: `stepGH_tree_vs_aaa0496d6.patch` (2 files).

## What changed

- `operations/surface_3d.rs`, `draw_waterline_params`:
  - The label "Continuous:" now reads "Helical:".
  - The label and the checkbox take the registry help of `helical` as
    their hover (`help_for(Waterline, "helical")`, UI-04: keyed on the
    parameter name). The help text is core's `HELP_WATERLINE_HELICAL`:
    "Cut each stack of matching rings (a hole, a wall) with one entry: the
    tool descends along the lower ring from level to level. Off: one entry
    per ring and level."
  - The line I5 wrote (`cfg.helical`) is kept.
- `ui/properties/tests.rs`: `the_waterline_row_reads_helical_w1b`.

The Scallop editor's own "Continuous:" row (`ScallopConfig::continuous`)
is a different field, so it does not change.

## Line count

No change: the same row with a new word.

## Gates (green)

- `--lib the_waterline_row_reads_helical_w1b`.
- `--test the_help_key_is_the_param_name_ui04`, `the_inspector_text_is_bounded_ic1`,
  `operations_registry_ui05`: green.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
