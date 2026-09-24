# G4: LUT discrepancies found during the band fetch

Date: 2026-09-24. Author: the G4 research agent. Read-only: no LUT file changed.

Scope: the LUT rows that cite the Amana Spektra v24 and Amana compression v8
charts, and the Freud solid-carbide rows. The script
`scripts/g4_band_shape.py` gives the census (`fetch/G4/band_shape_derived.txt`).

Result of the value check: every printed value in the LUT that cites these
three charts equals the chart text. The items below are about the form of
the rows and about the source revision, not about a wrong number.

## D1. 14 Spektra rows carry a band that the chart does not print

The file `amana_flat_end.json` holds 14 rows with `source_id`
`amana_spektra_spiral_plunge_v24`, `evidence_grade` "c", `row_kind`
"derived", and both a minimum and a maximum. The chart prints one value per
cell. Examples:

| observation_id | min | max (mm/tooth) | chart value for that cell (mm/tooth) |
|---|---|---|---|
| amana-flat-softwood-adaptive-3175-2f | 0.032 | 0.048 | 0.1016 (1/8", Wood/Plywood, .0040") |
| amana-flat-softwood-pocket-6000-2f | 0.05 | 0.085 | 0.127 (6mm, Wood/Plywood, .0050") |
| amana-flat-mdf-pocket-6000-2f | 0.03 | 0.055 | 0.1524 (6mm, MDF/Laminate, .0060") |
| amana-flat-hardwood-contour-3175-2f | 0.018 | 0.03 | 0.1016 (1/8", Wood/Plywood) |

The rows state this themselves in `notes`: "Not printed on the cited chart;
repo-authored band/ap range." So the rows are honest. The consequence for
G4 is in the trend work:

- `lut_axes.txt` counts these 14 as "band" rows of the Spektra source. They
  are not vendor bands.
- Their min/max ratio (median 0.58, range 0.53-0.67) must not enter any
  per-family band-shape trend. With them excluded, the flat-end vendor bands
  come from Onsrud (4 sheets), Freud and Amana ZrN v8 only.

## D2. One printed fact, two encodings, two engine behaviours

The Spektra chart prints one chip load per cell. The LUT encodes it in two
ways:

| File | Rows | Encoding | Grade |
|---|---|---|---|
| `amana_flat_end.json` | 60 | `chipload_max_mm_tooth` only; `chipload_min_mm_tooth` absent | a (MDF column) / b (Wood/Plywood column) |
| `amana_long_tail.json` | 26 | `chipload_min_mm_tooth` == `chipload_max_mm_tooth` | a |

The same split exists on other one-value charts: `amana_compression.json`
(5 wood rows, min == max), `amana_vgroove_engraving.json` (3 rows,
min == max), `whiteside_fusion360.json` (13 rows, min absent), and the IDC
community rows (min == max).

The engine treats the two encodings differently (code read on 2026-09-24,
see FETCH_NOTES.md §3):

- **min absent.** The burn gate uses `AllowHalfBand`: the high side is
  hard, the low side is not modelled (`below_low` returns `None`). The
  envelope resolver, Suggest, the simulation modulator and the strategy
  advisor use `RequireBoth` and get no band at all. The printed value is
  lost to them, and the modulator runs bandless.
- **min == max.** `RequireBoth` accepts it (`min <= max`) and
  `ChiploadBand::new` accepts it (`max < min` is the only order check). The
  modulator then gets a zero-width band: the floor (`band.min x rpm x
  flutes`) and the target (`band.max`) are one feed. The burn gate
  classifies the row `VendorLutPointPreset`, so a low-side trip becomes an
  advisory, and the high side stays hard.

So a 3.175 mm softwood Spektra row (`amana-flat-softwood-pocket-3175-2f-spektra`,
min absent) and a 3.0 mm softwood Spektra row
(`amana-flat-softwood-pocket-3000-2f-spektra`, min == max, the same printed
.0040") produce different modulator and advisor behaviour. This is not a
wrong number. It is an inconsistent encoding of "one value printed". The G4
reconciler must pick one meaning for a one-value cell before it fits
anything (see FETCH_NOTES.md §4, the framing question).

## D3. The Spektra source is superseded (v24 -> v41)

The LUT and the manifest cite v24 (sha256 5b6fef85..., re-downloaded
2026-09-24: the hash still matches). The URL pattern also serves v25-v33
and v35-v41 as PDFs. v41 (sha256 7b61543a..., PDF CreationDate
2024-06-15) is the highest version number that returns a PDF. I could not
confirm that v41 is the chart Amana links today: the product pages return
Cloudflare 403.

v41 against v24, for the rows the LUT holds (stored text
`fetch/G4/sources/amana_spektra_spiral_plunge_v41.txt`):

| Row | v24 (LUT) Wood/Plywood, MDF/Laminate | v41 Wood/Plywood, MDF/Laminate | v41 verbatim |
|---|---|---|---|
| 2 Flute 1.5mm (48210-K, 48212-K) | .0020", .0030" (0.0508, 0.0762 mm) | .0010", .0020" (0.0254, 0.0508 mm) | `—        48210-K        1.5mm          35"        .0010"       17.5"       70"        .0020"      35"` |
| 2 Flute 3mm (48214-K, 48216-K) | .0040", .0050" (0.1016, 0.127 mm) | .0020", .0030" (0.0508, 0.0762 mm) | `—         48214-K        3mm           70"        .0020"       35"         105"       .0030"     52.5"` |

All other sizes the LUT holds are unchanged in v41. LUT rows affected:
`amana-flat-softwood-pocket-1500-2f-spektra`,
`amana-flat-mdf-pocket-1500-2f-spektra`,
`amana-flat-softwood-pocket-3000-2f-spektra`,
`amana-flat-mdf-pocket-3000-2f-spektra`.

v41 also prints sizes the LUT does not hold: 4mm (.0019" / .0039"), 8mm,
10mm, and a 4 Flute block (1/4" 59706-K .0010" / .0020"; 3/8" 46057-K
.0056" / .0083"). These are G1 (size) material. I did not transcribe them
as G4 candidate rows. The 1.5 mm and 3 mm revisions matter to G1: the
sub-2 mm Spektra rows are the anchors of the size trend.

Recommendation (not a ruling): the operator decides whether the LUT moves
to v41. Until then the LUT is a correct copy of a superseded chart.
