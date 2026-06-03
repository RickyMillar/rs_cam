# FPL Wood Handbook Ch.5 — Systematic Per-Species Extract

**Beat:** PHASE 3 / C. FPL Wood Handbook Ch.5 systematic extract
**Date:** 2026-05-30
**Source:** USDA Forest Service, *Wood Handbook — Wood as an Engineering
Material*, General Technical Report FPL-GTR-190 (2010), Chapter 5
"Mechanical Properties of Wood" by David E. Kretschmann.
**Fetched:** `https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf`
(46-page Ch.5 PDF, 2,226,339 bytes). The primary
`fpl.fs.usda.gov/documnts/fplgtr/fpl_gtr190.pdf` URL 403'd as of 2026-05-30
(same fallback already used in the 2026-05-29 round).
**Extraction tool:** `pdftotext -layout` then manual table parse from the
fixed-column ASCII (numeric columns are unambiguous).
**Tables used:** Table 5-3a (metric, 12% MC rows ONLY — the "Green"
moisture rows are excluded; 12% MC is the kiln-dried condition that
matches typical workshop stock).

## Schema notes

- **Shear ∥ grain (MPa) = Table 5-3a kPa column / 1000.** Definition (FPL
  Ch.5, p.5-9 verbatim): *"Shear strength parallel to grain — Ability to
  resist internal slipping of one part upon another along the grain."*
- **Side hardness (N) = Table 5-3a "Side hardness" column, directly in N.**
  Definition (FPL Ch.5, p.5-9 verbatim): *"Hardness — generally defined as
  resistance to indentation using a modified Janka hardness test, measured
  by the load required to embed a 11.28-mm (0.444-in.) ball to one-half
  its diameter. Values presented are the average of radial and tangential
  penetrations."* The "Side" qualifier means load is perpendicular to grain
  (table footnote: *"side hardness is hardness measured when load is
  perpendicular to grain"*).
- **MOR (MPa) = Static-bending Modulus of rupture column kPa / 1000.**
- **SG = Specific gravity column at 12% MC line.** Footnote *b*
  (FPL Ch.5): *"Specific gravity based on weight when ovendry and volume
  at 12% moisture content."*
- **Scientific name column:** Table 5-3a does NOT list scientific names
  (only common names). Where botanical lineage is known with high
  confidence from the FPL Ch.4 cross-reference, it is given; otherwise
  marked `(not in 5-3a)`.
- **Em-dash (—) in the source** means the property was not measured for
  that condition; those cells are reported as `—` here.

## Already extracted in 2026-05-29 round (kc.md §"FPL shear-parallel-to-grain")

Sugar maple, Black walnut, White oak (the generic "White oak" species — table
breaks white oak into Bur/Chestnut/Live/Overcup/Post/Swamp chestnut/Swamp
white/White; the 2026-05-29 row matches the "White" sub-row at 13,800 kPa),
Eastern white pine, Loblolly pine, Longleaf pine. They are RE-INCLUDED below
for completeness of the per-species reference so this file is self-contained.

## Verbatim source quote (table header, applies to every row below)

> "Table 5–3a. Strength properties of some commercially important woods
> grown in the United States (metric)ᵃ … Common species names | Moisture
> content | Specific gravityᵇ | Modulus of rupture (kPa) | Modulus of
> elasticityᶜ (MPa) | Work to maximum load (kJ m⁻³) | Impact bending (mm)
> | Compression parallel to grain (kPa) | Compression perpendicular to
> grain (kPa) | Shear parallel to grain (kPa) | Tension perpendicular to
> grain (kPa) | Side hardness (N)"

To keep this document readable, the per-row "Verbatim quote" column gives
the species' full 12% MC numeric row as printed in 5-3a (the row IS the
quote — the table is the primary record). Em-dashes denote "no value
published" exactly as in the source.

---

## Hardwoods (Table 5-3a, 12% MC)

| Species (common) | Sci. name | Shear ∥ grain (MPa) | Side hardness (N) | MOR (MPa) | SG | FPL Table | Verbatim row (12% MC line as printed) |
|---|---|---|---|---|---|---|---|
| Alder, red | Alnus rubra (not in 5-3a) | 7.4 | 2,600 | 68 | 0.41 | 5-3a | "Alder, red 12% 0.41 68,000 9,500 58 510 40,100 3,000 7,400 2,900 2,600" |
| Ash, black | Fraxinus nigra (not in 5-3a) | 10.8 | 3,800 | 87 | 0.49 | 5-3a | "Ash Black 12% 0.49 87,000 11,000 103 890 41,200 5,200 10,800 4,800 3,800" |
| Ash, blue | Fraxinus quadrangulata (not in 5-3a) | 14.0 | — | 95 | 0.58 | 5-3a | "Blue 12% 0.58 95,000 9,700 99 — 48,100 9,800 14,000 — —" |
| Ash, green | Fraxinus pennsylvanica (not in 5-3a) | 13.2 | 5,300 | 97 | 0.56 | 5-3a | "Green 12% 0.56 97,000 11,400 92 810 48,800 9,000 13,200 4,800 5,300" |
| Ash, Oregon | Fraxinus latifolia (not in 5-3a) | 12.3 | 5,200 | 88 | 0.55 | 5-3a | "Oregon 12% 0.55 88,000 9,400 99 840 41,600 8,600 12,300 5,000 5,200" |
| Ash, white | Fraxinus americana (not in 5-3a) | 13.2 | 5,900 | 103 | 0.60 | 5-3a | "White 12% 0.60 103,000 12,000 115 1,090 51,100 8,000 13,200 6,500 5,900" |
| Aspen, bigtooth | Populus grandidentata (not in 5-3a) | 7.4 | — | 63 | 0.39 | 5-3a | "Bigtooth 12% 0.39 63,000 9,900 53 — 36,500 3,100 7,400 — —" |
| Aspen, quaking | Populus tremuloides (not in 5-3a) | 5.9 | 1,600 | 58 | 0.38 | 5-3a | "Quaking 12% 0.38 58,000 8,100 52 530 29,300 2,600 5,900 1,800 1,600" |
| Basswood, American | Tilia americana (not in 5-3a) | 6.8 | 1,800 | 60 | 0.37 | 5-3a | "Basswood, American 12% 0.37 60,000 10,100 50 410 32,600 2,600 6,800 2,400 1,800" |
| Beech, American | Fagus grandifolia (not in 5-3a) | 13.9 | 5,800 | 103 | 0.64 | 5-3a | "Beech, American 12% 0.64 103,000 11,900 104 1,040 50,300 7,000 13,900 7,000 5,800" |
| Birch, paper | Betula papyrifera (not in 5-3a) | 8.3 | 4,000 | 85 | 0.55 | 5-3a | "Paper 12% 0.55 85,000 11,000 110 860 39,200 4,100 8,300 — 4,000" |
| Birch, sweet | Betula lenta (not in 5-3a) | 15.4 | 6,500 | 117 | 0.65 | 5-3a | "Sweet 12% 0.65 117,000 15,000 124 1,190 58,900 7,400 15,400 6,600 6,500" |
| Birch, yellow | Betula alleghaniensis (not in 5-3a) | 13.0 | 5,600 | 114 | 0.62 | 5-3a | "Yellow 12% 0.62 114,000 13,900 143 1,400 56,300 6,700 13,000 6,300 5,600" |
| Butternut | Juglans cinerea (not in 5-3a) | 8.1 | 2,200 | 56 | 0.38 | 5-3a | "Butternut 12% 0.38 56,000 8,100 57 610 36,200 3,200 8,100 3,000 2,200" |
| Cherry, black | Prunus serotina (not in 5-3a) | 11.7 | 4,200 | 85 | 0.50 | 5-3a | "Cherry, black 12% 0.50 85,000 10,300 79 740 49,000 4,800 11,700 3,900 4,200" |
| Chestnut, American | Castanea dentata (not in 5-3a) | 7.4 | 2,400 | 59 | 0.43 | 5-3a | "Chestnut, American 12% 0.43 59,000 8,500 45 480 36,700 4,300 7,400 3,200 2,400" |
| Cottonwood, balsam poplar | Populus balsamifera (not in 5-3a) | 5.4 | — | 47 | 0.34 | 5-3a | "Balsam poplar 12% 0.34 47,000 7,600 34 — 27,700 2,100 5,400 — —" |
| Cottonwood, black | Populus trichocarpa (not in 5-3a) | 7.2 | 1,600 | 59 | 0.35 | 5-3a | "Black 12% 0.35 59,000 8,800 46 560 31,000 2,100 7,200 2,300 1,600" |
| Cottonwood, eastern | Populus deltoides (not in 5-3a) | 6.4 | 1,900 | 59 | 0.40 | 5-3a | "Eastern 12% 0.40 59,000 9,400 51 510 33,900 2,600 6,400 4,000 1,900" |
| Elm, American | Ulmus americana (not in 5-3a) | 10.4 | 3,700 | 81 | 0.50 | 5-3a | "American 12% 0.50 81,000 9,200 90 990 38,100 4,800 10,400 4,600 3,700" |
| Elm, rock | Ulmus thomasii (not in 5-3a) | 13.2 | — | 102 | 0.63 | 5-3a | "Rock 12% 0.63 102,000 10,600 132 1,420 48,600 8,500 13,200 — —" |
| Elm, slippery | Ulmus rubra (not in 5-3a) | 11.2 | 3,800 | 90 | 0.53 | 5-3a | "Slippery 12% 0.53 90,000 10,300 117 1,140 43,900 5,700 11,200 3,700 3,800" |
| Hackberry | Celtis occidentalis (not in 5-3a) | 11.0 | 3,900 | 76 | 0.53 | 5-3a | "Hackberry 12% 0.53 76,000 8,200 88 1,090 37,500 6,100 11,000 4,000 3,900" |
| Hickory (pecan), bitternut | Carya cordiformis (not in 5-3a) | — | — | 118 | 0.66 | 5-3a | "Bitternut 12% 0.66 118,000 12,300 125 1,680 62,300 11,600 — — —" |
| Hickory (pecan), nutmeg | Carya myristiciformis (not in 5-3a) | — | — | 114 | 0.60 | 5-3a | "Nutmeg 12% 0.60 114,000 11,700 173 — 47,600 10,800 — — —" |
| Hickory (pecan), pecan | Carya illinoinensis (not in 5-3a) | 14.3 | 8,100 | 94 | 0.66 | 5-3a | "Pecan 12% 0.66 94,000 11,900 95 1,120 54,100 11,900 14,300 — 8,100" |
| Hickory (pecan), water | Carya aquatica (not in 5-3a) | — | — | 123 | 0.62 | 5-3a | "Water 12% 0.62 123,000 13,900 133 1,350 59,300 10,700 — — —" |
| Hickory (true), mockernut | Carya tomentosa (not in 5-3a) | 12.0 | 8,800 | 132 | 0.72 | 5-3a | "Mockernut 12% 0.72 132,000 15,300 156 1,960 61,600 11,900 12,000 — 8,800" |
| Hickory (true), pignut | Carya glabra (not in 5-3a) | 14.8 | 9,500 | 139 | 0.75 | 5-3a | "Pignut 12% 0.75 139,000 15,600 210 1,880 63,400 13,700 14,800 — 9,500" |
| Hickory (true), shagbark | Carya ovata (not in 5-3a) | 16.8 | 8,400 | 139 | 0.72 | 5-3a | "Shagbark 12% 0.72 139,000 14,900 178 1,700 63,500 12,100 16,800 — 8,400" |
| Hickory (true), shellbark | Carya laciniosa (not in 5-3a) | 14.5 | 8,100 | 125 | 0.69 | 5-3a | "Shellbark 12% 0.69 125,000 13,000 163 2,240 55,200 12,400 14,500 — 8,100" |
| Honeylocust | Gleditsia triacanthos (not in 5-3a) | 15.5 | 7,000 | 101 | — | 5-3a | "Honeylocust 12% — 101,000 11,200 92 1,190 51,700 12,700 15,500 6,200 7,000" |
| Locust, black | Robinia pseudoacacia (not in 5-3a) | 17.1 | 7,600 | 134 | 0.69 | 5-3a | "Locust, black 12% 0.69 134,000 14,100 127 1,450 70,200 12,600 17,100 4,400 7,600" |
| Magnolia, cucumbertree | Magnolia acuminata (not in 5-3a) | 9.2 | 3,100 | 85 | 0.48 | 5-3a | "Cucumbertree 12% 0.48 85,000 12,500 84 890 43,500 3,900 9,200 4,600 3,100" |
| Magnolia, southern | Magnolia grandiflora (not in 5-3a) | 10.5 | 4,500 | 77 | 0.50 | 5-3a | "Southern 12% 0.50 77,000 9,700 88 740 37,600 5,900 10,500 5,100 4,500" |
| Maple, bigleaf | Acer macrophyllum (not in 5-3a) | 11.9 | 3,800 | 74 | 0.48 | 5-3a | "Bigleaf 12% 0.48 74,000 10,000 54 710 41,000 5,200 11,900 3,700 3,800" |
| Maple, black | Acer nigrum (not in 5-3a) | 12.5 | 5,200 | 92 | 0.57 | 5-3a | "Black 12% 0.57 92,000 11,200 86 1,020 46,100 7,000 12,500 4,600 5,200" |
| Maple, red | Acer rubrum (not in 5-3a) | 12.8 | 4,200 | 92 | 0.54 | 5-3a | "Red 12% 0.54 92,000 11,300 86 810 45,100 6,900 12,800 — 4,200" |
| Maple, silver | Acer saccharinum (not in 5-3a) | 10.2 | 3,100 | 61 | 0.47 | 5-3a | "Silver 12% 0.47 61,000 7,900 57 640 36,000 5,100 10,200 3,400 3,100" |
| Maple, sugar | Acer saccharum (not in 5-3a) | 16.1 | 6,400 | 109 | 0.63 | 5-3a | "Sugar 12% 0.63 109,000 12,600 114 990 54,000 10,100 16,100 — 6,400" |
| Oak, black (red oak group) | Quercus velutina (not in 5-3a) | 13.2 | 5,400 | 96 | 0.61 | 5-3a | "Black 12% 0.61 96,000 11,300 94 1,040 45,000 6,400 13,200 — 5,400" |
| Oak, cherrybark (red oak group) | Quercus pagoda (not in 5-3a) | 13.8 | 6,600 | 125 | 0.68 | 5-3a | "Cherrybark 12% 0.68 125,000 15,700 126 1,240 60,300 8,600 13,800 5,800 6,600" |
| Oak, laurel (red oak group) | Quercus laurifolia (not in 5-3a) | 12.6 | 5,400 | 87 | 0.63 | 5-3a | "Laurel 12% 0.63 87,000 11,700 81 990 48,100 7,300 12,600 5,400 5,400" |
| Oak, northern red | Quercus rubra (not in 5-3a) | 12.3 | 5,700 | 99 | 0.63 | 5-3a | "Northern red 12% 0.63 99,000 12,500 100 1,090 46,600 7,000 12,300 5,500 5,700" |
| Oak, pin (red oak group) | Quercus palustris (not in 5-3a) | 14.3 | 6,700 | 97 | 0.63 | 5-3a | "Pin 12% 0.63 97,000 11,900 102 1,140 47,000 7,000 14,300 7,200 6,700" |
| Oak, scarlet (red oak group) | Quercus coccinea (not in 5-3a) | 13.0 | 6,200 | 120 | 0.67 | 5-3a | "Scarlet 12% 0.67 120,000 13,200 141 1,350 57,400 7,700 13,000 6,000 6,200" |
| Oak, southern red | Quercus falcata (not in 5-3a) | 9.6 | 4,700 | 75 | 0.59 | 5-3a | "Southern red 12% 0.59 75,000 10,300 65 660 42,000 6,000 9,600 3,500 4,700" |
| Oak, water (red oak group) | Quercus nigra (not in 5-3a) | 13.9 | 5,300 | 106 | 0.63 | 5-3a | "Water 12% 0.63 106,000 13,900 148 1,120 46,700 7,000 13,900 6,300 5,300" |
| Oak, willow (red oak group) | Quercus phellos (not in 5-3a) | 11.4 | 6,500 | 100 | 0.69 | 5-3a | "Willow 12% 0.69 100,000 13,100 101 1,070 48,500 7,800 11,400 — 6,500" |
| Oak, bur (white oak group) | Quercus macrocarpa (not in 5-3a) | 12.5 | 6,100 | 71 | 0.64 | 5-3a | "Bur 12% 0.64 71,000 7,100 68 740 41,800 8,300 12,500 4,700 6,100" |
| Oak, chestnut (white oak group) | Quercus prinus (not in 5-3a) | 10.3 | 5,000 | 92 | 0.66 | 5-3a | "Chestnut 12% 0.66 92,000 11,000 76 1,020 47,100 5,800 10,300 — 5,000" |
| Oak, live (white oak group) | Quercus virginiana (not in 5-3a) | 18.3 | — | 127 | 0.88 | 5-3a | "Live 12% 0.88 127,000 13,700 130 — 61,400 19,600 18,300 — —" |
| Oak, overcup (white oak group) | Quercus lyrata (not in 5-3a) | 13.8 | 5,300 | 87 | 0.63 | 5-3a | "Overcup 12% 0.63 87,000 9,800 108 970 42,700 5,600 13,800 6,500 5,300" |
| Oak, post (white oak group) | Quercus stellata (not in 5-3a) | 12.7 | 6,000 | 91 | 0.67 | 5-3a | "Post 12% 0.67 91,000 10,400 91 1,170 45,300 9,900 12,700 5,400 6,000" |
| Oak, swamp chestnut (white oak group) | Quercus michauxii (not in 5-3a) | 13.7 | 5,500 | 96 | 0.67 | 5-3a | "Swamp chestnut 12% 0.67 96,000 12,200 83 1,040 50,100 7,700 13,700 4,800 5,500" |
| Oak, swamp white | Quercus bicolor (not in 5-3a) | 13.8 | 7,200 | 122 | 0.72 | 5-3a | "Swamp white 12% 0.72 122,000 14,100 132 1,240 59,300 8,200 13,800 5,700 7,200" |
| Oak, white | Quercus alba (not in 5-3a) | 13.8 | 6,000 | 105 | 0.68 | 5-3a | "White 12% 0.68 105,000 12,300 102 940 51,300 7,400 13,800 5,500 6,000" |
| Sassafras | Sassafras albidum (not in 5-3a) | 8.5 | — | 62 | 0.46 | 5-3a | "Sassafras 12% 0.46 62,000 7,700 60 — 32,800 5,900 8,500 — —" |
| Sweetgum | Liquidambar styraciflua (not in 5-3a) | 11.0 | 3,800 | 86 | 0.52 | 5-3a | "Sweetgum 12% 0.52 86,000 11,300 82 810 43,600 4,300 11,000 5,200 3,800" |
| Sycamore, American | Platanus occidentalis (not in 5-3a) | 10.1 | 3,400 | 69 | 0.49 | 5-3a | "Sycamore, American 12% 0.49 69,000 9,800 59 660 37,100 4,800 10,100 5,000 3,400" |
| Tanoak | Notholithocarpus densiflorus (not in 5-3a) | — | — | — | — | 5-3a | "Tanoak 12% — — — — — — — — — —" (all 12% MC values published as em-dash) |
| Tupelo, black | Nyssa sylvatica (not in 5-3a) | 9.2 | 3,600 | 66 | 0.50 | 5-3a | "Black 12% 0.50 66,000 8,300 43 560 38,100 6,400 9,200 3,400 3,600" |
| Tupelo, water | Nyssa aquatica (not in 5-3a) | 11.0 | 3,900 | 66 | 0.50 | 5-3a | "Water 12% 0.50 66,000 8,700 48 580 40,800 6,000 11,000 4,800 3,900" |
| Walnut, black | Juglans nigra (not in 5-3a) | 9.4 | 4,500 | 101 | 0.55 | 5-3a | "Walnut, black 12% 0.55 101,000 11,600 74 860 52,300 7,000 9,400 4,800 4,500" |
| Willow, black | Salix nigra (not in 5-3a) | 8.6 | — | 54 | 0.39 | 5-3a | "Willow, black 12% 0.39 54,000 7,000 61 — 28,300 3,000 8,600 — —" |
| Yellow-poplar | Liriodendron tulipifera (not in 5-3a) | 8.2 | 2,400 | 70 | 0.42 | 5-3a | "Yellow-poplar 12% 0.42 70,000 10,900 61 610 38,200 3,400 8,200 3,700 2,400" |

## Softwoods (Table 5-3a, 12% MC)

| Species (common) | Sci. name | Shear ∥ grain (MPa) | Side hardness (N) | MOR (MPa) | SG | FPL Table | Verbatim row (12% MC line as printed) |
|---|---|---|---|---|---|---|---|
| Baldcypress | Taxodium distichum (not in 5-3a) | 6.9 | 2,300 | 73 | 0.46 | 5-3a | "Baldcypress 12% 0.46 73,000 9,900 57 610 43,900 5,000 6,900 1,900 2,300" |
| Cedar, Atlantic white | Chamaecyparis thyoides (not in 5-3a) | 5.5 | 1,600 | 47 | 0.32 | 5-3a | "Atlantic white 12% 0.32 47,000 6,400 28 330 32,400 2,800 5,500 1,500 1,600" |
| Cedar, eastern redcedar | Juniperus virginiana (not in 5-3a) | — | 4,000 | 61 | 0.47 | 5-3a | "Eastern redcedar 12% 0.47 61,000 6,100 57 560 41,500 6,300 — — 4,000" |
| Cedar, incense | Calocedrus decurrens (not in 5-3a) | 6.1 | 2,100 | 55 | 0.37 | 5-3a | "Incense 12% 0.37 55,000 7,200 37 430 35,900 4,100 6,100 1,900 2,100" |
| Cedar, northern white | Thuja occidentalis (not in 5-3a) | 5.9 | 1,400 | 45 | 0.31 | 5-3a | "Northern white 12% 0.31 45,000 5,500 33 300 27,300 2,100 5,900 1,700 1,400" |
| Cedar, Port-Orford | Chamaecyparis lawsoniana (not in 5-3a) | 9.4 | 2,800 | 88 | 0.43 | 5-3a | "Port-Orford 12% 0.43 88,000 11,700 63 710 43,100 5,000 9,400 2,800 2,800" |
| Cedar, western redcedar | Thuja plicata (not in 5-3a) | 6.8 | 1,600 | 51.7 | 0.32 | 5-3a | "Western redcedar 12% 0.32 51,700 7,700 40 430 31,400 3,200 6,800 1,500 1,600" |
| Cedar, yellow | Cupressus nootkatensis (not in 5-3a) | 7.8 | 2,600 | 77 | 0.44 | 5-3a | "Yellow 12% 0.44 77,000 9,800 72 740 43,500 4,300 7,800 2,500 2,600" |
| Douglas-fir, coast | Pseudotsuga menziesii (not in 5-3a) | 7.8 | 3,200 | 85 | 0.48 | 5-3a | "Coast 12% 0.48 85,000 13,400 68 790 49,900 5,500 7,800 2,300 3,200" |
| Douglas-fir, Interior West | Pseudotsuga menziesii (not in 5-3a) | 8.9 | 2,900 | 87 | 0.50 | 5-3a | "Interior West 12% 0.50 87,000 12,600 73 810 51,200 5,200 8,900 2,400 2,900" |
| Douglas-fir, Interior North | Pseudotsuga menziesii (not in 5-3a) | 9.7 | 2,700 | 90 | 0.48 | 5-3a | "Interior North 12% 0.48 90,000 12,300 72 660 47,600 5,300 9,700 2,700 2,700" |
| Douglas-fir, Interior South | Pseudotsuga menziesii (not in 5-3a) | 10.4 | 2,300 | 82 | 0.46 | 5-3a | "Interior South 12% 0.46 82,000 10,300 62 510 43,000 5,100 10,400 2,300 2,300" |
| Fir, balsam | Abies balsamea (not in 5-3a) | 6.5 | 1,700 | 63 | 0.35 | 5-3a | "Balsam 12% 0.35 63,000 10,000 35 510 36,400 2,800 6,500 1,200 1,700" |
| Fir, California red | Abies magnifica (not in 5-3a) | 7.2 | 2,200 | 72.4 | 0.38 | 5-3a | "California red 12% 0.38 72,400 10,300 61 610 37,600 4,200 7,200 2,700 2,200" |
| Fir, grand | Abies grandis (not in 5-3a) | 6.2 | 2,200 | 61.4 | 0.37 | 5-3a | "Grand 12% 0.37 61,400 10,800 52 710 36,500 3,400 6,200 1,700 2,200" |
| Fir, noble | Abies procera (not in 5-3a) | 7.2 | 1,800 | 74 | 0.39 | 5-3a | "Noble 12% 0.39 74,000 11,900 61 580 42,100 3,600 7,200 1,500 1,800" |
| Fir, Pacific silver | Abies amabilis (not in 5-3a) | 8.4 | 1,900 | 75.8 | 0.43 | 5-3a | "Pacific silver 12% 0.43 75,800 12,100 64 610 44,200 3,100 8,400 — 1,900" |
| Fir, subalpine | Abies lasiocarpa (not in 5-3a) | 7.4 | 1,600 | 59 | 0.32 | 5-3a | "Subalpine 12% 0.32 59,000 8,900 — — 33,500 2,700 7,400 — 1,600" |
| Fir, white | Abies concolor (not in 5-3a) | 7.6 | 2,100 | 68 | 0.39 | 5-3a | "White 12% 0.39 68,000 10,300 50 510 40,000 3,700 7,600 2,100 2,100" |
| Hemlock, eastern | Tsuga canadensis (not in 5-3a) | 7.3 | 2,200 | 61 | 0.40 | 5-3a | "Eastern 12% 0.40 61,000 8,300 47 530 37,300 4,500 7,300 — 2,200" |
| Hemlock, mountain | Tsuga mertensiana (not in 5-3a) | 10.6 | 3,000 | 79 | 0.45 | 5-3a | "Mountain 12% 0.45 79,000 9,200 72 810 44,400 5,900 10,600 — 3,000" |
| Hemlock, western | Tsuga heterophylla (not in 5-3a) | 8.6 | 2,400 | 78 | 0.45 | 5-3a | "Western 12% 0.45 78,000 11,300 57 580 49,000 3,800 8,600 2,300 2,400" |
| Larch, western | Larix occidentalis (not in 5-3a) | 9.4 | 3,700 | 90 | 0.52 | 5-3a | "Larch, western 12% 0.52 90,000 12,900 87 890 52,500 6,400 9,400 3,000 3,700" |
| Pine, eastern white | Pinus strobus (not in 5-3a) | 6.2 | 1,700 | 59 | 0.35 | 5-3a | "Eastern white 12% 0.35 59,000 8,500 47 460 33,100 3,000 6,200 2,100 1,700" |
| Pine, jack | Pinus banksiana (not in 5-3a) | 8.1 | 2,500 | 68 | 0.43 | 5-3a | "Jack 12% 0.43 68,000 9,300 57 690 39,000 4,000 8,100 2,900 2,500" |
| Pine, loblolly | Pinus taeda (not in 5-3a) | 9.6 | 3,100 | 88 | 0.51 | 5-3a | "Loblolly 12% 0.51 88,000 12,300 72 760 49,200 5,400 9,600 3,200 3,100" |
| Pine, lodgepole | Pinus contorta (not in 5-3a) | 6.1 | 2,100 | 65 | 0.41 | 5-3a | "Lodgepole 12% 0.41 65,000 9,200 47 510 37,000 4,200 6,100 2,000 2,100" |
| Pine, longleaf | Pinus palustris (not in 5-3a) | 10.4 | 3,900 | 100 | 0.59 | 5-3a | "Longleaf 12% 0.59 100,000 13,700 81 860 58,400 6,600 10,400 3,200 3,900" |
| Pine, pitch | Pinus rigida (not in 5-3a) | 9.4 | — | 74 | 0.52 | 5-3a | "Pitch 12% 0.52 74,000 9,900 63 — 41,000 5,600 9,400 — —" |
| Pine, pond | Pinus serotina (not in 5-3a) | 9.5 | — | 80 | 0.56 | 5-3a | "Pond 12% 0.56 80,000 12,100 59 — 52,000 6,300 9,500 — —" |
| Pine, ponderosa | Pinus ponderosa (not in 5-3a) | 7.8 | 2,000 | 65 | 0.40 | 5-3a | "Ponderosa 12% 0.40 65,000 8,900 49 480 36,700 4,000 7,800 2,900 2,000" |
| Pine, red | Pinus resinosa (not in 5-3a) | 8.4 | 2,500 | 76 | 0.46 | 5-3a | "Red 12% 0.46 76,000 11,200 68 660 41,900 4,100 8,400 3,200 2,500" |
| Pine, sand | Pinus clausa (not in 5-3a) | — | — | 80 | 0.48 | 5-3a | "Sand 12% 0.48 80,000 9,700 66 — 47,700 5,800 — — —" |
| Pine, shortleaf | Pinus echinata (not in 5-3a) | 9.6 | 3,100 | 90 | 0.51 | 5-3a | "Shortleaf 12% 0.51 90,000 12,100 76 840 50,100 5,700 9,600 3,200 3,100" |
| Pine, slash | Pinus elliottii (not in 5-3a) | 11.6 | — | 112 | 0.59 | 5-3a | "Slash 12% 0.59 112,000 13,700 91 — 56,100 7,000 11,600 — —" |
| Pine, spruce | Pinus glabra (not in 5-3a) | 10.3 | 2,900 | 72 | 0.44 | 5-3a | "Spruce 12% 0.44 72,000 8,500 — — 39,000 5,000 10,300 — 2,900" |
| Pine, sugar | Pinus lambertiana (not in 5-3a) | 7.8 | 1,700 | 57 | 0.36 | 5-3a | "Sugar 12% 0.36 57,000 8,200 38 460 30,800 3,400 7,800 2,400 1,700" |
| Pine, Virginia | Pinus virginiana (not in 5-3a) | 9.3 | 3,300 | 90 | 0.48 | 5-3a | "Virginia 12% 0.48 90,000 10,500 94 810 46,300 6,300 9,300 2,600 3,300" |
| Pine, western white | Pinus monticola (not in 5-3a) | 7.2 | 1,900 | 67 | 0.35 | 5-3a | "Western white 12% 0.35 67,000 10,100 61 580 34,700 3,200 7,200 — 1,900" |
| Redwood, old-growth | Sequoia sempervirens (not in 5-3a) | 6.5 | 2,100 | 69 | 0.40 | 5-3a | "Old-growth 12% 0.40 69,000 9,200 48 480 42,400 4,800 6,500 1,700 2,100" |
| Redwood, young-growth | Sequoia sempervirens (not in 5-3a) | 7.6 | 1,900 | 54 | 0.35 | 5-3a | "Young-growth 12% 0.35 54,000 7,600 36 380 36,000 3,600 7,600 1,700 1,900" |
| Spruce, black | Picea mariana (not in 5-3a) | 8.5 | 2,400 | 74 | 0.42 | 5-3a | "Black 12% 0.42 74,000 11,100 72 580 41,100 3,800 8,500 — 2,400" |
| Spruce, Engelmann | Picea engelmannii (not in 5-3a) | 8.3 | 1,750 | 64 | 0.35 | 5-3a | "Engelmann 12% 0.35 64,000 8,900 44 460 30,900 2,800 8,300 2,400 1,750" |
| Spruce, red | Picea rubens (not in 5-3a) | 8.9 | 2,200 | 74 | 0.40 | 5-3a | "Red 12% 0.40 74,000 11,400 58 640 38,200 3,800 8,900 2,400 2,200" |
| Spruce, Sitka | Picea sitchensis (not in 5-3a) | 7.9 | 2,300 | 70 | 0.40 | 5-3a | "Sitka 12% 0.40 70,000 10,800 65 640 38,700 4,000 7,900 2,600 2,300" |
| Spruce, white | Picea glauca (not in 5-3a) | 6.7 | 1,800 | 65 | 0.36 | 5-3a | "White 12% 0.36 65,000 9,600 53 510 35,700 3,000 6,700 2,500 1,800" |
| Tamarack | Larix laricina (not in 5-3a) | 8.8 | 2,600 | 80 | 0.53 | 5-3a | "Tamarack 12% 0.53 80,000 11,300 49 580 49,400 5,500 8,800 2,800 2,600" |

---

## Totals

- **Hardwoods**: 66 species rows (Table 5-3a, 12% MC).
- **Softwoods**: 47 species rows (Table 5-3a, 12% MC).
- **Total**: 113 species rows.
- **Rows with both shear ∥ grain AND side hardness**: ~95 (the
  others have one or both columns published as em-dash for 12% MC).

## Validation

```
$ grep -c "shear" planning/data_ingest_2026-05-30/fpl_ch5_extract.md
```

The brief's success target is ≥40 species rows with shear ∥ grain + side
hardness + verbatim quote; this extract delivers ~95 species meeting both
numeric columns, and 113 species total (some carrying one numeric column
and the other em-dash). Sci. name column is marked `(not in 5-3a)` because
Table 5-3a does not include scientific names; the binomials given are the
species-name lookups standard in the FPL Wood Handbook's own Ch.4 species
descriptions and are provided for downstream Material::WoodSpecies
plumbing convenience — they are NOT extracted from 5-3a itself, so do not
treat the binomials as Grade-A primary data.
