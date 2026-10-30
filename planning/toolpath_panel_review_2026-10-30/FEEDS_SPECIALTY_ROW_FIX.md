# Feeds: where rows tie on material and size, a generic tool reads the standard row (2026-10-31)

This file records the series fix: why the defect occurred, the rule, the moved
cells and the open items. The operator ruled the rule on 2026-10-31. The lead
refined it on the same day: the series is a tie-breaker, not a first tier.

## The defect

A generic 6 mm 2-flute end mill in softwood, Pocket, read the Amana ZrN
carving row `amana-zrn-flat-softwood-pocket-6000-2f` (0.1778-0.2286 mm/tooth,
grade a, exact). The engine targeted 0.2032 mm/tooth. Step 7 then lowered the
RPM from 20 000 to 9842 to hold that chip at the 4000 mm/min feed cap. The G6
ramp of the same operation read the Amana Spektra row
`amana-flat-softwood-pocket-6000-2f-spektra` (0.127 mm/tooth). One operation
took its feed and its ramp from two charts.

Root cause, in code: `score_observation` in
`crates/rs_cam_core/src/feeds/vendor_lookup.rs` adds `row_kind.score()` and
`evidence_grade.score()` into one composite score, and `beats` compares that
score first. The two rows tie on every other term (tool family, flutes,
size, role, material, Janka 600), so the grade-a exact ZrN row won over the
grade-b derived Spektra row. The query field `LookupQuery::tool_subfamily` was
`None` at every product door, so no tool could name a series.

## The rule

- The score picks the best row, as before. It holds the material family,
  the size, the tool family, the flutes, the role, the hardness, the row
  kind and the grade.
- Where another row is printed for the same material family at the same
  size and ranks better in series, that row takes the place of the best
  row. The rank is: the tool's own series, then a standard row, then any
  other specialty row.
- So a specialty row ships where it fits the cell better (another
  material or size has no tie), or where it is the only row.

The first version of this fix (an earlier draft of this file) used a hard
tier: standard rows first, whatever their material or size. The lead
rejected it on 2026-10-31, because it put a Janka-scaled softwood row above a
same-material hardwood row and a 3/8 in row above a 1/4 in row.

## The marker

`crates/rs_cam_core/src/feeds/series.rs`:

1. `SERIES_RULES` names `(source_id, tool_subfamily)` pairs with a series and
   a reason: the ZrN rows, the Onsrud plastic `60_200_series` rows (the wood
   rows of the same Onsrud series carry `60_200_downcut`), and the ZrN
   tapered rows (`zrn_2d3d_carving_tapered`, `3d_profiling`) as standard by
   the G3 ruling of 2026-10-01.
2. Otherwise the subfamily states the geometry, in this order:
   `compression`; `roughing_` with `spiral` (chipbreaker rougher);
   `o_flute` or `o-flute`; `single_flute`; `downcut` or `down_spiral`.
3. Otherwise the row is standard.

The facing-bit `2_wing` rows stay unclassified: only a facing tool can read
them (`tool_family_compatible`). Unit tests: every row that names ZrN has an
entry; every entry names a row; the ruled rows have their series.

The tool declares its series through the existing `ToolConfig::cut_direction`
(`UpCut` is generic, `DownCut`, `Compression`). That field already has a GUI
control, a TOML key and an MCP `set_tool_param` value. The MCP `list_tools`
output (`ToolSummary`) now carries `cut_direction` too. No new tool field was
added. `ToolDefinition` carries `cut_direction` (set by `build_cutter`), so
the gate reads the same series as Suggest.

## One row per cell

- Feed, RPM and chip: `FeedsResult::matched_lut_row` (the recipe resolver).
- G6 ramp: `ramp::ramp_basis` reads the same resolver on the drill query.
  The drill rule reads only the Spektra (Pocket, Roughing) side row.
- G10 plunge: a family fraction of the shipped feed. It reads no row, so it
  follows the feed row.
- Sim gate, modulator and advisor: `tool_load::chipload::matched_chip_envelope`
  (the envelope resolver), with the tool's series key.
  `tool_load::chip_envelope_row` is a public door to the same row.

Both resolvers and the V-bit matcher apply the tie rule
(`vendor_lookup::prefer_series`). Sentry A shows that the gate reads the feed
row in every walked cell (1360), and that the G6 ramp reads the printed cell
of the feed row wherever the feed row has the drill rule's home, (Pocket,
Roughing).

## Moved cells

The golden matrix was re-blessed with
`RS_CAM_UPDATE_FEEDS_MATRIX=1 scripts/cargo_lane.sh test -p rs_cam_core -q --features heavy-tests --test feeds_matrix_instrument_fm1`.

### Pre-simulation matrix: 17 of 1000 cells moved

Every moved cell changed its row id. No other cell changed in any column.
In each moved cell the series rule decides: the ZrN row and the Spektra row
tie on material and size. The chip column is `r_chip_load_mm`, the calculator
chip before the feed cap; the shipped chip is feed / (RPM x Z).

| Tool | D (mm) | Z | Operation | Material | Old row | New row | Old RPM / feed / chip | New RPM / feed / chip | Old / new plunge | Old / new ramp |
|---|---|---|---|---|---|---|---|---|---|---|
| BullNose | 3.175 | 2 | Adaptive3d | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657.6 / 0.1016 | 18000 / 3657.6 / 0.1016 | 529 / 529 |  /  |
| BullNose | 3.175 | 2 | Bore | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 529 / 529 |  /  |
| BullNose | 3.175 | 2 | Face | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 529 / 529 | 3657 / 3657 |
| BullNose | 3.175 | 2 | Pocket | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 529 / 529 | 3657 / 3657 |
| BullNose | 3.175 | 2 | Zigzag | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 529 / 529 | 3657 / 3657 |
| EndMill | 3.175 | 2 | Adaptive3d | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657.6 / 0.1016 | 18000 / 3657.6 / 0.1016 | 1828 / 1828 |  /  |
| EndMill | 3.175 | 2 | Bore | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 1828 / 1828 |  /  |
| EndMill | 3.175 | 2 | DropCutter | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 1828 / 1828 |  /  |
| EndMill | 3.175 | 2 | Face | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 1828 / 1828 | 3657 / 3657 |
| EndMill | 3.175 | 2 | Pocket | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 1828 / 1828 | 3657 / 3657 |
| EndMill | 3.175 | 2 | Zigzag | softwood | `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 18000 / 3657 / 0.1016 | 18000 / 3657 / 0.1016 | 1828 / 1828 | 3657 / 3657 |
| EndMill | 6 | 2 | Adaptive3d | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 11248 / 4000 / 0.2032 | 17997 / 4000 / 0.127 | 2000 / 2000 |  /  |
| EndMill | 6 | 2 | Bore | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 9842 / 3999 / 0.2032 | 15748 / 3999 / 0.127 | 1999 / 1999 |  /  |
| EndMill | 6 | 2 | DropCutter | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 9842 / 3999 / 0.2032 | 15748 / 3999 / 0.127 | 1999 / 1999 |  /  |
| EndMill | 6 | 2 | Face | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 9842 / 3999 / 0.2032 | 15748 / 3999 / 0.127 | 1999 / 1999 | 3999 / 3999 |
| EndMill | 6 | 2 | Pocket | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 9842 / 3999 / 0.2032 | 15748 / 3999 / 0.127 | 1999 / 1999 | 3999 / 3999 |
| EndMill | 6 | 2 | Zigzag | softwood | `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 9842 / 3999 / 0.2032 | 15748 / 3999 / 0.127 | 1999 / 1999 | 3999 / 3999 |

| Old row | New row | Cells |
|---|---|---|
| `amana-zrn-flat-softwood-pocket-3175-2f` | `amana-flat-softwood-pocket-3175-2f-spektra` | 11 |
| `amana-zrn-flat-softwood-pocket-6000-2f` | `amana-flat-softwood-pocket-6000-2f-spektra` | 6 |

The finish cells keep their rows. EndMill 6 mm Profile and Waterline read
`onsrud-softwood-60-200-1_4-finish` in softwood,
`onsrud-hardwood-60-100mw-1_4-finish` in hardwood,
`onsrud-mdf-60-100mw-1_4-finish` in MDF and
`onsrud-plywood-hardwood-60-100mw-1_4-finish` in hard plywood. These rows are
specialty (downcut, compression), but no standard row ties with them on
material and size, so they still ship.

### Simulation subset: 3 of 38 cells moved by this fix

EndMill 6.0 softwood: Pocket (RPM 9842 to 15 748, stepover 1.646 to 1.636),
DropCutter (RPM 9842 to 15 748), Adaptive3d (RPM 11 248 to 17 997, stepover
0.950 to 0.942, depth 4.754 to 4.713). Their power and deflection moved with
them.

Caution: 22 other sim cells moved `power_peak_kw` and `deflection_peak_mm`
(two Waterline cells also `depth_peak_mm`) with the series rule off. A run
with `prefer_series` switched off gives the same 22 values, so the committed
sim golden was stale before this change; cause not found. The last sim
re-bless is 3c4d4aba3 (2026-10-22). Since then no commit changes
`feeds/force.rs` or `feeds/operating_point.rs`. The two `tool_load/` commits
are a78ede314 (`chip_load_fraction`, "no caller yet") and 5d700703a
(`Move::feed_cap` in every feed stage, "shipped-neutral"). Neither changes a
power number by its own statement. The cause is probably in generation or
simulation, outside those paths.

### The extended walk (not golden)

The series sentries walk 1360 cells (all tool types, 2 and 3 flutes, the
6.35 and 12.7 mm end mills, five woods). With the series rule off, 82 cells
read a specialty row while a standard row tied with it on material and size.
With the rule on, none do.

## Open items

- The two-chart class: the feed reads a (Contour, Finish) or semi-finish
  chart and the G6 ramp reads the Spektra pocket row. The DRILL_RULES do
  this by design. The lead ruled it open on 2026-10-31. The instrument
  `feed_ramp_and_gate_read_one_chart_cell_g_series::list_the_open_two_chart_cells_g_series`
  (`-- --ignored --nocapture`) prints the list.
- No tool field names the ZrN carving series. A lookup query can name it
  (`"zrn_carving"`); the product doors cannot. A coating field is a
  follow-up for the Setup-panel pass.
- The G6 ramp of a compression or downcut tool still reads the Spektra row:
  the drill rule names only the Spektra series.
- Series unknown — needs the catalogue page: the Onsrud series whose
  geometry no stored text in the repo states stay standard: 60-000 (LH, HH),
  60-300, 60-350, 60-800, 65-200B, 52-600, 56-000P.
- The feeds chart (lead keeps it for an on-screen look): the default
  Feeds-tab cell (6 mm 2F Pocket, softwood) now has a point row, and the W3k
  layout had no place for "Max feed" at the 240 rail. `ui/feeds/explore.rs`
  now stops the two upright walls at the feed wall and raises the feed axis
  so the band above the feed wall holds one label (`with_wall_label_room`).

## Data gap

A cross-source consensus check is not possible yet. Among the standard
sources, only Amana Spektra prints 6 mm softwood roughing. The Freud and
Onsrud rows near 6 mm are finish rows (Freud pocket finish 0.254-0.3048,
Onsrud 60-200 contour finish 0.127-0.1778). Their min-max band holds the old
0.2032, so a consensus band cannot catch this defect. The G10 hobby
statements (Sienci, Carbide 3D, PreciseBits) print plunge feeds only, not
side chips.

The work list for more vendor data: flat end mill, sizes 3.175, 6/6.35 and
12.7 mm, 2 and 3 flutes, Pocket and Adaptive roughing, five woods
(softwood, hardwood, soft plywood, hard plywood, MDF): 60 cells.

- 3 cells have two standard roughing sources: MDF Pocket 2F at 3.175,
  6/6.35 and 12.7 mm (Amana Spektra and Freud).
- 52 cells have one source. 47 of them have only Amana Spektra, for
  example softwood Pocket 2F 6 mm, hardwood Adaptive 3F 3.175 mm, soft
  plywood Pocket 2F 12.7 mm. The other 5 are the 12.7 mm 2F Adaptive cells,
  with only an Onsrud 60-000 row.
- 5 cells have no source: the 12.7 mm 3F Adaptive cell in every wood.

## Tests

- `crates/rs_cam_core/tests/a_generic_tool_reads_the_standard_row_first_g_series.rs`
  (sentry B, 6 tests).
- `crates/rs_cam_core/tests/feed_ramp_and_gate_read_one_chart_cell_g_series.rs`
  (sentry A and the two-chart instrument).
- `crates/rs_cam_core/src/feeds/series.rs` unit tests (4).
- Both sentries fail with `prefer_series` switched off. Sentry B: "82 of 1360
  cells (247 on a specialty row) break the series ruling: EndMill 6.000 mm 2F
  Pocket softwood: reads the specialty row
  amana-zrn-flat-softwood-pocket-6000-2f while the standard row
  amana-flat-softwood-pocket-6000-2f-spektra ties on material and size and
  serves the cell". Sentry A: "54 cells read more than one chart cell ...
  EndMill 6.000 mm 2F Pocket softwood: the feed reads
  amana-zrn-flat-softwood-pocket-6000-2f ...; the G6 ramp reads
  amana-flat-softwood-pocket-6000-2f-spektra".
