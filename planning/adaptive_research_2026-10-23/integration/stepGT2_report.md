# Step GT2: the compact tab dot has no trailing space

Implementer: I4. Date: 2026-10-28. Base: `5b4a561ae` (GT). The lead's
follow-up to GT: take only the change that costs nothing. The compact dot
loses its trailing space; the 2-pt padding stays.

Patch: `stepGT2_tree_vs_5b4a561ae.patch` (one file, `tab_strip.rs`).

## What changed

`tab_label`: when the labels are below body size (the compact strip), the
dot is "●", not "● ". The full-size strip, and `full_width`, keep "● ".

## Result: no strip that wrapped now fits

The change saves about 3 pt per dot, as estimated. That is not enough for
either dotted strip at 280 points. I did not force a fit.

IC1 harness, the Cut tab at 280 points (x positions of the painted text):

| Strip | Row 1 ends (GT → GT2) | Last tab | Fits? |
|---|---|---|---|
| 3D Rough (1 dot) | 1 row, ends at x 1385 | — | yes (as GT) |
| 3D Finish (2 dots) | `Linking` ends at x 1351 → x 1345 | `Dressup` still on row 2: it needs about 3 pt more | no |
| Pocket (3 dots) | `Linking` ends at x 1364 → x 1355 | `Dressup` still on row 2: it needs about 13 pt more | no |

How I got the numbers: in the 3D Rough strip, `Dressup` (43 pt of text)
fits with its text ending at x 1385. For it to fit after `Linking`, row 1
must end by about x 1340. My GT estimate of "4 pt short" for 3D Finish was
low by about 5 pt. The real shortfall at GT was about 8 pt.

The IC1 line counts do not change for any tab (3D Rough 5/18/17/21/18/9,
3D Finish 7/17/21/25/19/12, Pocket 10/21/21/23/20/11).

The Simulation inspector does not change: its strip is full size at
304 pt and has no dot.

Remaining choices (the lead's decision; I recommend no change):

- The 1-pt padding (12 pt over six tabs), which the lead declined for the
  hit area. With it, 3D Finish fits and Pocket does not.
- Keep GT2. It is harmless, and it gives 3 pt per dot in a narrower
  panel.
- Drop GT2.

## Gates (green)

- `cargo test -p rs_cam_viz --lib -- _gt _d7 tab_badges _f24`: 8 passed.
- `--test the_inspector_text_is_bounded_ic1`, `the_sim_inspector_is_tabbed_ic3`,
  `component_contracts_up2`: green.
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`: clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
