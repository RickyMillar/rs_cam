# Wood Database Species Sweep — 2026-05-30

Per-species Janka hardness (lbf at 12% MC) from The Wood Database
(https://www.wood-database.com/). Each row is read directly from a fetched page
and stored with a verbatim quote. **No value is invented or estimated.**

Phase 3 / B beat of the data ingest plan
(`planning/feeds_data_ingest_2026-05-30_phased_plan.md`). Mirrors the format of
`planning/data_ingest_2026-05-29/hardness.md` section 1b. Touches no live code
or data files.

Test definition (verbatim, from The Wood Database `janka-hardness/` article):

> "the amount of pounds-force (lbf) or newtons (N) required to imbed a .444″
> (11.28 mm) diameter steel ball into the wood to half the ball's diameter"

Ball 0.444 in (11.28 mm), embedded to half diameter, 12% MC, side grain.

These species are NOT in the current `WoodSpecies` enum
(`crates/rs_cam_core/src/material.rs`: GenericSoftwood, RadiataPine,
LongleafPine, GenericHardwood, HardMaple, Walnut, Birch, WhiteOak, Jarrah,
Ipe). They are candidates for future expansion of the per-species Janka
override table.

Evidence grade: all Grade A (direct, single-source manufacturer / authoritative
database).

**On the verbatim quotes:** Each quote is the rendered (visible) page text of
the canonical Janka infobox row, e.g. `"Janka Hardness: 1,220 lbf (5,430 N)"`.
The underlying HTML uses `lb<sub>f</sub>` and inserts a
`<span class="ejm">:</span>` between "Janka Hardness" and the colon; the
quote above is the human-readable text after HTML rendering. Both the lbf
number and the N number are read verbatim from the page — neither is
converted nor inferred.

## Species table

| Species | Janka (lbf) | Source URL | Verbatim quote | Notes |
|---------|-------------|-----------|----------------|-------|
| Red Oak (Northern) | 1220 | https://www.wood-database.com/red-oak/ | "Janka Hardness: 1,220 lbf (5,430 N)" | Quercus rubra |
| Black Cherry | 950 | https://www.wood-database.com/black-cherry/ | "Janka Hardness: 950 lbf (4,230 N)" | Prunus serotina |
| Yellow Poplar | 540 | https://www.wood-database.com/poplar/ | "Janka Hardness: 540 lbf (2,400 N)" | Liriodendron tulipifera |
| White Ash | 1320 | https://www.wood-database.com/white-ash/ | "Janka Hardness: 1,320 lbf (5,870 N)" | Fraxinus americana |
| Honduran Mahogany | 900 | https://www.wood-database.com/honduran-mahogany/ | "Janka Hardness: 900 lbf (4,020 N)" | Swietenia macrophylla |
| Douglas-Fir | 620 | https://www.wood-database.com/douglas-fir/ | "Janka Hardness: 620 lbf (2,760 N)" | Pseudotsuga menziesii |
| Red Alder | 590 | https://www.wood-database.com/red-alder/ | "Janka Hardness: 590 lbf (2,620 N)" | Alnus rubra |
| American Beech | 1300 | https://www.wood-database.com/american-beech/ | "Janka Hardness: 1,300 lbf (5,780 N)" | Fagus grandifolia |
| Shagbark Hickory | 1880 | https://www.wood-database.com/shagbark-hickory/ | "Janka Hardness: 1,880 lbf (8,360 N)" | Carya ovata |
| Western Red Cedar | 350 | https://www.wood-database.com/western-red-cedar/ | "Janka Hardness: 350 lbf (1,560 N)" | Thuja plicata |
| Sapele | 1360 | https://www.wood-database.com/sapele/ | "Janka Hardness: 1,360 lbf (6,060 N)" | Entandrophragma cylindricum |
| African Padauk | 1710 | https://www.wood-database.com/african-padauk/ | "Janka Hardness: 1,710 lbf (7,580 N)" | Pterocarpus soyauxii |
| Purpleheart | 2520 | https://www.wood-database.com/purpleheart/ | "Janka Hardness: 2,520 lbf (11,190 N)" | Peltogyne spp. |
| Wenge | 1930 | https://www.wood-database.com/wenge/ | "Janka Hardness: 1,930 lbf (8,600 N)" | Millettia laurentii |
| Bubinga | 2410 | https://www.wood-database.com/bubinga/ | "Janka Hardness: 2,410 lbf (10,720 N)" | Guibourtia spp. |
| Zebrawood | 1830 | https://www.wood-database.com/zebrawood/ | "Janka Hardness: 1,830 lbf (8,160 N)" | Microberlinia brazzavillensis |
| Bloodwood | 2900 | https://www.wood-database.com/bloodwood/ | "Janka Hardness: 2,900 lbf (12,900 N)" | Brosimum rubescens |
| Cocobolo | 2960 | https://www.wood-database.com/cocobolo/ | "Janka Hardness: 2,960 lbf (14,140 N)" | Dalbergia retusa |
| Bocote | 2010 | https://www.wood-database.com/bocote/ | "Janka Hardness: 2,010 lbf (8,950 N)" | Cordia spp. |
| Lacewood | 840 | https://www.wood-database.com/lacewood/ | "Janka Hardness: 840 lbf (3,740 N)" | Panopsis spp. |
| Yellow Birch | 1260 | https://www.wood-database.com/yellow-birch/ | "Janka Hardness: 1,260 lbf (5,610 N)" | Betula alleghaniensis |
| Sweet Cherry | 1150 | https://www.wood-database.com/sweet-cherry/ | "Janka Hardness: 1,150 lbf (5,120 N)" | Prunus avium |
| Spanish Cedar | 600 | https://www.wood-database.com/spanish-cedar/ | "Janka Hardness: 600 lbf (2,670 N)" | Cedrela odorata |
| Loblolly Pine | 690 | https://www.wood-database.com/loblolly-pine/ | "Janka Hardness: 690 lbf (3,070 N)" | Pinus taeda |
| Soft Maple | 950 | https://www.wood-database.com/soft-maple/ | "Janka Hardness: 950 lbf (4,230 N)" | Acer rubrum / Acer saccharinum |
| European Beech | 1450 | https://www.wood-database.com/european-beech/ | "Janka Hardness: 1,450 lbf (6,460 N)" | Fagus sylvatica |
| Teak | 1070 | https://www.wood-database.com/teak/ | "Janka Hardness: 1,070 lbf (4,740 N)" | Tectona grandis |
| Gaboon Ebony | 3080 | https://www.wood-database.com/gaboon-ebony/ | "Janka Hardness: 3,080 lbf (13,700 N)" | Diospyros crassiflora |
| East Indian Rosewood | 2350 | https://www.wood-database.com/east-indian-rosewood/ | "Janka Hardness: 2,350 lbf (10,440 N)" | Dalbergia latifolia |
| Brazilian Rosewood | 2790 | https://www.wood-database.com/brazilian-rosewood/ | "Janka Hardness: 2,790 lbf (12,410 N)" | Dalbergia nigra |
| Light Red Meranti | 550 | https://www.wood-database.com/light-red-meranti/ | "Janka Hardness: 550 lbf (2,460 N)" | Shorea spp. |
| Dark Red Meranti | 800 | https://www.wood-database.com/dark-red-meranti/ | "Janka Hardness: 800 lbf (3,570 N)" | Shorea spp. |
| White Meranti | 1050 | https://www.wood-database.com/white-meranti/ | "Janka Hardness: 1,050 lbf (4,670 N)" | Shorea spp. |
| Yellow Meranti | 700 | https://www.wood-database.com/yellow-meranti/ | "Janka Hardness: 700 lbf (3,120 N)" | Shorea spp. |

## Summary

- **Species collected:** 34
- **Source:** The Wood Database (https://www.wood-database.com/)
- **Grade:** A (direct authoritative database)
- **Test method:** ASTM-style Janka, side hardness, 12% MC

## Cross-checks with `hardness.md` (2026-05-29)

Five species below were already present in the 2026-05-29 hardness pull
(section 1b) — values reproduced exactly from the same wood-database.com
endpoints, confirming data stability:

- Red Oak (Northern): 1,220 lbf — matches 2026-05-29 row.
- Black Cherry: 950 lbf — matches.
- Yellow Poplar: 540 lbf — matches.
- White Ash: 1,320 lbf — matches.
- Honduran Mahogany: 900 lbf — matches.
- Douglas-Fir: 620 lbf — matches.

The remaining 28 rows are new contributions toward the Phase 3 / B
"species sweep" target.

## Notes on species selection

- **Loblolly Pine** (`loblolly-pine`) was fetched specifically (vs. the generic
  "Southern Yellow Pine" trade group) per the brief — `690 lbf` is the
  single-species published Janka, which aligns with the repo's existing
  `SouthernYellowPine` value of 690 (flagged as "tracking loblolly" in the
  2026-05-29 review).
- **Sweet Cherry** vs. **Black Cherry**: both listed. Sweet Cherry (1,150) is
  the European/orchard species; Black Cherry (950) is the North American
  furniture wood. The repo has no Cherry variant; if added, Black Cherry is
  the typical default.
- **Soft Maple** (red/silver maple, 950) provides a counterpoint to the
  existing `HardMaple` (sugar, 1,450).
- **Meranti** is logged as four sub-groups (Light Red 550, Dark Red 800, White
  1,050, Yellow 700). Wood-database.com does not have a unified `/meranti/`
  page; only the four colour-group pages.
- **Brazilian Rosewood** is CITES Appendix I and **East Indian Rosewood** is
  CITES Appendix II — included for completeness; their use in router work
  carries legal restrictions noted on the source pages.

## Sources

- The Wood Database — https://www.wood-database.com/ (per-species pages)
- Janka method article — https://www.wood-database.com/wood-articles/janka-hardness/
