# Provenance — Onsrud + Whiteside ingest (2026-05-29)

All chipload values converted inch -> mm by x25.4. All CED (cutting-edge
diameter) values converted inch -> mm by x25.4. Every row below carries the
verbatim source quote it was read from.

## Cross-cutting source facts

### Onsrud depth-of-cut rule (both plastics data sheets, verbatim)
Soft Plastic sheet header box:
> "DEPTH OF CUT: 1 x D Use recommended chip load / 2 x D Reduce chip load by
> 25% / 3 x D Reduce chip load by 50%"

Hard Plastic sheet header box (identical):
> "DEPTH OF CUT: 1 x D Use recommended chip load / 2 x D Reduce chip load by
> 25% / 3 x D Reduce chip load by 50%"

This is the literal source of every `ap_rule` string on the Onsrud rows.

### Onsrud chipload formula (both sheets, verbatim)
> "FORMULAS: Chip Load = Feed Rate / (RPM x # of cutting edges) / Feed Rate
> (IPM) = RPM x # of cutting edges x chip load / Speed (RPM) = Feed Rate /
> (# of cutting edges x chip load)"
RPM is NOT tabulated on these sheets (except a `* = 12,500 RPM` footnote on
series 37-50/37-60, which were not selected). Therefore RPM is left null on
all Onsrud sheet rows — not derived, not invented.

### Onsrud soft-vs-hard plastic material classification
From Onsrud article "Frequently Asked Questions in the Routing of Plastics #2"
(https://onsrud.com/articles/Frequently-Asked-Questions-in-the-Routing-of-Plastics-2.asp), verbatim:
> "most soft plastics (HDPE, UHMW, Polypropylene, etc.) respond best to
> Conventional Cutting, while some harder materials (Acrylic, Polycarbonate,
> Nylon) [occasionally show improved performance with climb cutting]"

This primary statement is the basis for mapping:
- Soft Plastic (SP) sheet rows -> material_family `hdpe` (HDPE is named
  explicitly as a soft plastic).
- Hard Plastic (HP) sheet rows -> `acrylic` / `polycarbonate` / `delrin`
  (acrylic, polycarbonate, nylon are named explicitly as the harder
  materials; nylon/acetal-class maps to the closest enum `delrin`).

The data sheets themselves are titled only "Soft Plastic" / "Hard Plastic"
and tabulate chipload-per-CED by tool series; they do NOT print individual
material names in the tables. The material_family assignment is therefore
grounded in the FAQ#2 classification quote above, and each material_label
records the SP/HP class membership rather than implying the table named the
specific polymer.

### Column-to-diameter mapping verification
Both sheets share the diameter header (in): 1/16, 3/32, 1/8, 5/32, 3/16,
7/32, 1/4, 5/16, 3/8, 7/16, 1/2, 9/16, 5/8, 3/4, 7/8, 1, 1-1/8, 1-1/4,
1-1/2, 1-3/4, 2. Each cited value's character column was matched to the
nearest header-token column (right-aligned cells) programmatically; the
mapping for every row below was checked, not eyeballed.

---

## SOFT PLASTIC sheet (page 120) — material_family hdpe
Source: https://www.onsrud.com/images/Soft%20Plastic.pdf
(640 KB PDF, FlateDecode-compressed; text extracted with `pdftotext -layout`.)

### onsrud-soft-plastic-hdpe-63750-1f-quarter-singlepass
Single Pass BEST tool for <1/2" diameter (header box: "Single Pass ... BEST
63-750"). Geometry: 63-750 is "SC 1F Upcut 'O' Flute" (Onsrud series page,
21-degree helix, solid carbide), so flute_count=1, o_flute_single_upcut.
Verbatim row:
> "63-750  1xD  .002-.004  .004-.006  .006-.008  .008-.012  .008-.012  .010-.014"
Columns map: 1/16, 1/8, 3/16, 1/4, 3/8, 1/2.
Picked 1/4" CED (= 6.35 mm): chipload ".008-.012" in.
Conversion: 0.008 x 25.4 = 0.2032 mm; 0.012 x 25.4 = 0.3048 mm.

### onsrud-soft-plastic-hdpe-65200b-quarter-finish
Series 65-200B/65-300B (multi-edge finishing class). Verbatim row:
> "65-200B/65-300B  1xD  .002-.003  .002-.003  .003-.004  .003-.005  .003-.005  .004-.006  .006-.008"
Columns map: 1/16, 1/8, 3/16, 1/4, 5/16, 3/8, 1/2.
Picked 1/4" CED (= 6.35 mm): chipload ".003-.005" in.
Conversion: 0.003 x 25.4 = 0.0762 mm; 0.005 x 25.4 = 0.127 mm.
flute_count set to 2 (multi-edge "B" series; conservative minimum). pass_role
finish.

### onsrud-soft-plastic-hdpe-52600-half-singlepass
>=1/2" diameter Single Pass BETTER tool (header: ">= 1/2 DIAMETER TOOL ...
Single Pass ... BETTER 52-600"). Verbatim row:
> "52-600  1xD  .008-.010  .010-.012  .012-.014  .014-.016  .016-.018"
Columns map: 1/4, 3/8, 1/2, 5/8, 3/4.
Picked 1/2" CED (= 12.7 mm): chipload ".012-.014" in.
Conversion: 0.012 x 25.4 = 0.3048 mm; 0.014 x 25.4 = 0.3556 mm.
flute_count 2 (52-x00 family is double-edge). pass_role semi_finish.

### onsrud-soft-plastic-hdpe-60000-half-roughing
Roughing BEST tool (header: "Roughing ... 60-000"). Verbatim row:
> "60-000  1xD  .004-.006  .006-.008  .008-.012  .012-.016"
Columns map: 3/8, 1/2, 5/8, 3/4.
Picked 1/2" CED (= 12.7 mm): chipload ".006-.008" in.
Conversion: 0.006 x 25.4 = 0.1524 mm; 0.008 x 25.4 = 0.2032 mm.
flute_count 2. pass_role roughing, operation_family adaptive.

Soft-plastic footer note (verbatim, recorded for context — not a numeric row):
> "NOTE: To eliminate rewelding increase the feedrate or change to a single
> edge tool. If using a downcut spiral and chip rewelding occurs, cut a slot
> in your spoilboard to allow the chips a place to expand. Incorrect
> chiploads can lead to knife marks occurring."

---

## HARD PLASTIC sheet (page 121) — acrylic / polycarbonate / delrin
Source: https://www.onsrud.com/images/Hard%20Plastic.pdf
(648 KB PDF, FlateDecode-compressed; text extracted with `pdftotext -layout`.)

### onsrud-hard-plastic-acrylic-63700-1f-quarter-singlepass  AND
### onsrud-hard-plastic-polycarbonate-63700-1f-quarter-singlepass
Single Pass BEST tool for <1/2" diameter (header: "Single Pass ... BEST
63-700"). Geometry: 63-700 is "SC 1F Upcut 'O' Flute" (Onsrud series page;
"1 Flute Solid Carbide O Flute Upcut-Spiral ... for hard plastic and solid
surface"), so flute_count=1, o_flute_single_upcut. Verbatim row:
> "63-700  1xD  .002-.004  .006-.008  .008-.010  .010-.012  .010-.012  .012-.016"
Columns map: 1/16, 1/8, 3/16, 1/4, 3/8, 1/2.
Picked 1/4" CED (= 6.35 mm): chipload ".010-.012" in.
Conversion: 0.010 x 25.4 = 0.254 mm; 0.012 x 25.4 = 0.3048 mm.
Two rows emitted (acrylic, polycarbonate) — both are HP-class per FAQ#2; same
numbers, distinct material_family so each polymer family gets a row.

### onsrud-hard-plastic-delrin-56000p-quarter-singlepass
Single Pass GOOD tool for <1/2" diameter (header: "Single Pass GOOD 56-000P").
Verbatim row:
> "56-000P  1xD  .002-.004  .004-006  .004-.006  .006-.008  .008-.010"
Columns map: 1/8, 3/16, 1/4, 3/8, 1/2. (Note the 3/16 cell prints ".004-006"
— a missing decimal typo in the source; the 1/4" cell ".004-.006" used for
this row is unambiguous and was NOT affected.)
Picked 1/4" CED (= 6.35 mm): chipload ".004-.006" in.
Conversion: 0.004 x 25.4 = 0.1016 mm; 0.006 x 25.4 = 0.1524 mm.
Mapped to material_family `delrin` to represent the nylon/acetal-class member
of the HP group (noted in material_label). flute_count 2.

### onsrud-hard-plastic-acrylic-60200-half-finish
>=1/2" Finishing BEST tool (header: ">= 1/2 DIAMETER TOOL ... Finishing
60-200"). Verbatim row:
> "60-200  1xD  .004-.006  .004-.006  .006-.010  .012-.016"
Columns map: 1/4, 3/8, 1/2, 3/4.
Picked 1/2" CED (= 12.7 mm): chipload ".006-.010" in.
Conversion: 0.006 x 25.4 = 0.1524 mm; 0.010 x 25.4 = 0.254 mm.
flute_count 2, pass_role finish.

### onsrud-hard-plastic-polycarbonate-60000-half-roughing
Roughing BEST tool (header: "Roughing 60-000"). Verbatim row:
> "60-000  1xD  .004-.006  .006-.008  .008-.012  .012-.016"
Columns map: 3/8, 1/2, 5/8, 3/4.
Picked 1/2" CED (= 12.7 mm): chipload ".006-.008" in.
Conversion: 0.006 x 25.4 = 0.1524 mm; 0.008 x 25.4 = 0.2032 mm.
flute_count 2, pass_role roughing, operation_family adaptive.

Hard-plastic footer note (verbatim, context):
> "NOTE: When chip rewelding occurs while cutting plastic, increase feedrate
> or go to a single edge tool. Incorrect chiploads can result in cratering."

---

## Onsrud polycarbonate application article — diameter-independent window
Source: https://www.onsrud.com/articles/Routing-Polycarbonate-Material.asp

### onsrud-article-polycarbonate-optimum-chipload-window
Verbatim:
> "The optimum chipload to achieve the best finish seems to be in the range
> of 0.004 to 0.012."
Conversion (chipload in inch -> mm): 0.004 x 25.4 = 0.1016 mm;
0.012 x 25.4 = 0.3048 mm.
Also verbatim from the same article (recorded as ap_rule / behaviour):
> "Chipload = Feedrate/(RPM x # Cutting Edges)"
> "Plunging directly into the part gives no path for chip removal and can
> cause chip wrap, deformity, or melting of chips to the part." (enter "from
> the side or ramp into the part")
> Sheet fabricators should "[employ] upcut spirals with 'O' flute geometry to
> adequately remove chips from the workpiece."
This row is diameter-agnostic (no diameter field), evidence_grade a (vendor
article), row_kind exact. flute_count set to 1 (single-edge O-flute, the
geometry the article recommends).

---

## WHITESIDE — RPM-only rows (evidence_grade b, chipload null)
Whiteside publishes recommended-RPM ranges and material applicability per bit
but does NOT publish chipload tables. All four rows leave chipload null and
are marked evidence_grade "b" / row_kind "derived" (RPM-only). NOTE: the
acquisition doc's recalled "13,000-15,000 rpm" for the 60-deg V-groove was
NOT confirmed; the live product pages give the higher ranges quoted below,
which are what the rows use.

### whiteside-1540-vgroove-60deg-quarter-rpm
Source: https://www.whitesiderouterbits.com/products/1540
Verbatim: recommended RPM "18,000-22,000 (max 24,000)"; "60 included angle";
"1/4 cutting diameter"; materials "Natural woods, composite woods, hard
plastics, thin aluminum".
diameter 1/4" = 6.35 mm. included_angle 60. rpm_min 18000, rpm_max 24000,
rpm_nominal 22000 (top of the recommended band). flute_count 2 (page does not
state count for 1540; the related #1541 is noted as 3-flute "for improved
veining", implying the standard 1540 is 2-flute — recorded as an inference,
not a quote).

### whiteside-1550-vgroove-60deg-half-rpm
Source: https://www.whitesiderouterbits.com/products/1550
Verbatim: recommended RPM "16,000-20,000 (max 24,000)"; 60 degree; 1/2 inch
cutting diameter; materials "Natural woods, composite woods, hard plastics,
thin aluminum".
diameter 1/2" = 12.7 mm. rpm_min 16000, rpm_max 24000, rpm_nominal 20000.
flute_count 2 (same inference caveat as 1540).

### whiteside-ru4000h-roughing-up-spiral-3f-plywood-rpm
Source: https://www.whitesiderouterbits.com/products/ru4000h
Verbatim: recommended RPM "16,000-18,000 (max 24,000)"; "3/8" cutting
diameter; "Three flutes"; materials "plywood, melamine, mdf, and most
engineered products".
diameter 3/8" = 9.525 mm. flute_count 3 (stated). rpm_min 16000, rpm_max
24000, rpm_nominal 18000. material_family plywood_hardwood (plywood explicit).

### whiteside-rd5218h-roughing-down-spiral-3f-rpm
Source: https://www.whitesiderouterbits.com/products/rd5218h
Verbatim: recommended RPM "16,000-18,000 (max 24,000)"; "1/2" cutting
diameter; "Three flutes"; materials "Natural woods, composites".
diameter 1/2" = 12.7 mm. flute_count 3 (stated). rpm_min 16000, rpm_max 24000,
rpm_nominal 18000. material_family hardwood (natural woods).
