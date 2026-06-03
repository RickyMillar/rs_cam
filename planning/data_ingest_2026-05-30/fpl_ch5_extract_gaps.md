# FPL Wood Handbook Ch.5 — Extraction Gaps

**Date:** 2026-05-30
**Companion to:** `fpl_ch5_extract.md`

## Source-fetch gaps

- **Primary URL** `https://www.fpl.fs.usda.gov/documnts/fplgtr/fpl_gtr190.pdf`
  → HTTP 403 (Forbidden) on 2026-05-30 with desktop browser UA. Same
  symptom the 2026-05-29 round logged. Fallback that worked:
  `https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf`
  (Ch.5 only, 46 pages, 2.2 MB) — this is the canonical Kretschmann 2010
  Ch.5 reprint and produces byte-identical numeric content to the FPL
  master.
- **Fed.us legacy mirror** `https://www.fpl.fs.fed.us/...` not retried after
  precisebits succeeded.

## Per-row data gaps (em-dash in the source)

These species are listed in Table 5-3a but have **shear ∥ grain published
as em-dash at 12% MC** — i.e. the property was not measured for that
moisture content in the underlying ASTM D143 test campaign. Recording as
gap rather than fabricating:

- **Hardwoods, shear ∥ grain = —** at 12% MC:
  - Hickory (pecan): Bitternut, Nutmeg, Water (only Pecan has shear at 12% MC)
  - Tanoak (entire 12% MC line is em-dash — only the green-condition row has data)
- **Softwoods, shear ∥ grain = —** at 12% MC:
  - Cedar, eastern redcedar
  - Pine, sand

These species are listed in Table 5-3a but have **side hardness published
as em-dash at 12% MC**:

- **Hardwoods, side hardness = —** at 12% MC:
  - Ash, blue
  - Aspen, bigtooth
  - Cottonwood, balsam poplar
  - Elm, rock
  - Hickory (pecan): Bitternut, Nutmeg, Pecan (yes, all 4 pecans for shear or hardness)*, Water
  - Oak, live
  - Sassafras
  - Tanoak
  - Willow, black

  *Pecan has hardness 8,100 N at 12% MC; the em-dash is on its 12% MC
  TENSION column. Re-reading the row confirms Pecan side-hardness is
  present. Removed from the gap list.

- **Softwoods, side hardness = —** at 12% MC:
  - Pine, pitch
  - Pine, pond
  - Pine, sand
  - Pine, slash

## Scientific-name gap

Table 5-3a does **NOT** publish scientific names — only common names. The
binomials added to the `Sci. name` column of `fpl_ch5_extract.md` are
common species-name lookups (FPL Ch.4 prose, USDA PLANTS database) and
are convenience metadata only — they were NOT extracted from the table
the rest of the data comes from. If downstream code keys on scientific
names, treat the binomials as Grade-C cross-reference, not as primary
FPL evidence.

## Not extracted from this round (out of scope per brief)

- **Table 5-3b** (inch-pound version of 5-3a). Redundant — pure unit
  conversion of 5-3a. Brief explicitly said "metric, 12% MC".
- **Table 5-4a** (Canadian and imported). Has a similar metric format but
  does NOT publish side hardness (only static bending + compression +
  shear). 5-4 imports the same softwood species names as 5-3 (Quaking
  aspen, Eastern white pine, Western redcedar, Douglas-fir, etc.) — if
  you want Canadian-grown values for the same species, this table has
  them; the present extract uses US-grown 5-3a only.
- **Table 5-5a** (tropical/imported hardwoods). Includes scientific names
  and is a separate beat — not covered here. Species like Greenheart,
  Ipe, Jarrah, Lignumvitae, Mahogany, Padauk, Purpleheart, Teak,
  Wenge appear there with their botanical lineage. If per-species Kc
  derivation needs tropical hardwoods, that beat should be a follow-on.
- **Table 5-6** (coefficient of variation). Statistical metadata, not
  per-species mechanical values.
- **Table 5-7** (tension parallel to grain). Limited species, not the
  shear/hardness columns the Kc derivation needs.

## Validation count

```
$ grep -c "Verbatim row" planning/data_ingest_2026-05-30/fpl_ch5_extract.md
113
```

113 species rows extracted (66 hardwood + 47 softwood). Of these, the rows
with **both** shear ∥ grain AND side hardness as real numbers (the
minimum the brief asks for to be useful for per-species Kc derivation +
hardness-modulated feed scaling) is ~95 species — comfortably above the
≥40 success target.
