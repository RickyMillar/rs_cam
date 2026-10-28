# Step GT: the Feeds tab label and the compact tab strip

Implementer: I4. Date: 2026-10-28. Base: `5974e642c` (G-D). Operator ruling
on stepGT: option (b). Rename "Feeds & Speeds" to "Feeds", and make the
shared `TabStrip` compact at the inspector width.

Patch: `stepGT_tree_vs_5974e642c.patch` (6 files).

**Result: partial.** The 3D Rough strip now fits on one line at 280 points,
as the ruling asks. A strip with two or more tab dots still wraps to two
lines (3D Finish with 2 dots, Pocket with 3). I did not force a fit; see
"Open" below.

## What changed

| File | Change |
|---|---|
| `ui/components/tab_strip.rs` | `full_width` measures the strip at full size on one line: each label at `SIZE_BODY` with its padding, at least `TAB_MIN_WIDTH`, and the `SPACE_1` gap. When that is more than the width, `show` draws the strip compact: `SIZE_CAPTION` labels and badges, no minimum width, a 2-point button padding (`COMPACT_PADDING_X = SPACE_1`) and no gap. Labels do not change. The strip still wraps when even the compact form does not fit (UR1). Two new lib tests. |
| `ui/properties/mod.rs` | `ToolpathTab::FeedsSpeeds.label()` is "Feeds". The comment that lists the tabs changes too. |
| `ui/properties/tests.rs` | The D7 test reads "Feeds" in the fourth position. |
| `ui/properties/toolpath_panel.rs`, `feeds_speeds.rs` | Doc comments that name the tab. |
| `FEATURE_CATALOG.md` | The Geometry tab row names the six tabs with "Feeds", and records the old name and the reason. |

These do not change: `ToolpathTab::FeedsSpeeds`, the MCP key `"feeds"`, the
alias `"feeds_speeds"` and the file name `feeds_speeds.rs`. Only the D7 test
ties to the label. The IC1 comments that tell the history of the
"Feeds & Speeds" tab stay as they are.

## Measurements

### Strip widths (lib probe at the inspector widths)

| Strip | Full size, one line | Width given | Form drawn | Rows |
|---|---|---|---|---|
| Toolpath, 6 tabs, 1 dot (Feeds) | 365.2 pt | 264 | compact | 1 |
| Toolpath, old label "Feeds & Speeds", compact | 427.2 pt full | 264 | compact | 2 |
| Simulation, 4 tabs, a count on Warnings | 275.7 pt | 288 | full size (no change) | 1 |

The IC1 harness paints the 3D Rough strip compact from x 1131 to x 1385,
so it is 254 pt wide in a width of about 264 pt.

### IC1 visible lines at 280 points (fixture: boundary off)

| Tab | 3D Rough before → after | 3D Finish before → after | Pocket before → after |
|---|---|---|---|
| Geometry | 6 → 5 | 8 → 7 | 10 → 10 |
| Cut | 19 → 18 | 17 → 17 | 21 → 21 |
| Heights | 18 → 17 | 21 → 21 | 21 → 21 |
| Feeds | 22 → 21 | 25 → 25 | 23 → 23 |
| Linking | 19 → 18 | 19 → 19 | 20 → 20 |
| Dressup | 10 → 9 | 12 → 12 | 11 → 11 |

The body op (3D Rough, Cut tab) with the boundary on goes from 22 to 21
after G-D. Before G-D it was 23 (the design's measure). D-1 removed one
line and this step removes one more.

The 3D Finish and Pocket strips still wrap, because of the tab dots. Each
dot (`● `) adds about 10 pt.

- 3D Finish (dots on Cut and Feeds): row 1 ends at x 1351, and "Dressup"
  needs about 46 pt more. That is about 4 pt past the width.
- Pocket (dots on Cut, Heights and Feeds): row 1 ends at x 1364. That is
  about 17 pt past the width.

The Simulation inspector (IC3) does not change at its default width of
304 pt. Its strip fits at full size, so the compact form does not start.
The compact form starts there only when the panel is narrower than about
292 pt (275.7 pt plus the 16 pt of margins).

## Open: the strip with dots (the lead's decision)

The ruling says to stop and report, not to force a fit. Two changes would
recover about 20 pt, which fits Pocket with a small margin:

- the compact padding goes from 2 pt to 1 pt (12 pt over 6 tabs);
- the compact dot is drawn without its trailing space (about 3 pt per dot).

Neither change shortens a label. Both shrink the hit area of a tab. With
5 or 6 dots the strip wraps again.

## Tests and gates (green)

- New `--lib`:
  - `the_toolpath_tabs_fit_one_line_compact_gt`: one row at 264 pt and
    caption size. It also checks that the full size does not fit
    (non-vacuity).
  - `a_strip_that_fits_keeps_the_full_size_gt`: the Simulation tabs at
    288 pt keep body size on one row.
- `--lib -- _d7 tab_strip tab_badges _f24`: green.
- `--test` sentries, all green:
  - `the_inspector_text_is_bounded_ic1`
  - `the_sim_inspector_is_tabbed_ic3`
  - `inspector_width_is_tab_independent_up4`
  - `component_contracts_up2`
  - `chrome_reads_the_kit_up3`
- `cargo clippy --no-deps -p rs_cam_viz --all-targets -- -D warnings`:
  clean.
- `cargo fmt -p rs_cam_viz -- --check`: clean.
