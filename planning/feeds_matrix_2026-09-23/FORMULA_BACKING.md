# Feeds matrix R1: formula backing per cell class

Date: 2026-09-23. Status: agent judgement for ruling R1. The operator decides.
Inputs: `RULINGS.md` (R1, R5), `EVIDENCE.md` (sections 2, 3.1 to 3.5, 4.4, 6, 7),
`matrix_2026-09-23.csv`, `crates/rs_cam_core/src/feeds/support.rs`,
`crates/rs_cam_core/src/compute/catalog/registry.rs`,
`planning/load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md` (MRC). No code
changed. No cargo ran.

Headline: 118 of the 584 `FormulaOnly` cells are BACKED. 466 are CLUELESS.
Of the 466, 120 cells are refused by the engine's own tool rule, 230 have no
published figure, and 116 have figures that put the formula outside 0.5x to 2x.
With the 48 cells that refuse today, 514 of 960 cells would refuse.

## 1. Method and definitions

### Method

1. The agent grouped the 584 `FormulaOnly` rows of the matrix by tool family,
   feeds family, pass role and material, with the registry map of each operation.
2. For each class the agent took the formula chipload before derates
   (`r_chip_load_mm`) at both walked diameters and divided it by the midpoint
   of each admitted published band.
3. A class is BACKED only when BOTH walked diameters have at least one admitted
   figure with a ratio from 0.5 to 2.0.
4. The agent split out the cells that the engine's own tool rule refuses, and
   the agent judged the drill classes on drilling or plunge figures only.
5. Where one class carries two chiploads (V-bit Parallel: RampFinish against the
   other three operations, EVIDENCE 3.4-13), the worse chipload decides.

### Definitions

- **BACKED**: at least one published figure exists for the same tool family, a
  comparable operation family and the same material family. The pre-derate
  chipload lies from 0.5x to 2x of that figure's midpoint. A known low or high
  ratio inside that range is still BACKED; the ratio goes to R4.
- **CLUELESS**: one of three reasons.
  - `NO FIGURE`: no published figure exists for the tool family in a comparable
    operation family for the material.
  - `OUTSIDE`: the only admitted figures put the formula below 0.5x or above 2x.
  - `TOOL RULE`: the engine's own tool rule says the tool is wrong for the
    operation, and Suggest still ships a recipe.

### Rules the verdicts use

1. **Admitted sources.** Tooling-vendor charts: Onsrud, Amana, Freud, Techno.
   The bit retailer IDC Woodcraft (grade c; see judgement call 1). The machine
   maker's measured chart: Carbide 3D (EVIDENCE 6-3).
2. **Sources not admitted.**
   - The Shapeoko A-to-Z table (6-1, 6-2, 6-4, 6-5). It is a community table,
     and the formula is a fit to it, so it cannot check the formula.
   - LUT rows that have no printed source (4.1-12, 4.1-15, 3.3-17, 3.5-6, 6-11).
   - Sienci (3.4-2 to 3.4-5). It is a hobby table with no flute count and no
     diameter.
   - The PreciseBits 3 % rule. It is a start point for a calibration test, not
     a chart (EVIDENCE 7, section 3.5, item 4).
   - CAM-vendor prose (Vectric, Fusion, Mastercam).
3. **Material.** The figure must name the material, or name a set that contains
   it. "Wood/Plywood" covers plywood_hardwood. "Wood, MDF" covers MDF. A plain
   "Wood" row covers softwood and hardwood only.
4. **Operation.** A published chipload per tooth at 1 x D is comparable for every
   milling family of the same tool family. EVIDENCE 3.3-1 gives the reason: the
   per-tooth quantity transfers, and no wood vendor publishes a correction for a
   lighter pass (MRC section 2.4). Two exceptions:
   - Drill and AlignmentPinDrill need a drilling or straight-plunge figure.
   - A V-bit on Adaptive has no comparable cut. The V-bit rows are shallow
     groove rows; the adaptive recipe cuts past the cone (3.4-8, 3.4-9).
5. **Diameter.** The figure must be at the walked diameter or at the nearest
   printed size (1/8 in for 3.175 mm, 1/4 in or 6 mm for 6.0 and 6.35 mm).
   A V-bit chart that prints one band per angle and no diameter covers both
   V-bit diameters.
6. **Midpoint.** For a band, the midpoint. For a single printed or derived
   figure, the figure.

### Source key

| Key | Source | Figure (mm/tooth) | Where recorded |
|---|---|---|---|
| AC | Amana Solid Carbide Compression Spiral v8, 2 flute, 18 000 RPM, 1 x D | 1/8 in: wood 0.0279, MDF 0.0559, plywood 0.0279. 1/4 in: wood 0.0787, MDF 0.155, plywood 0.0787 | MRC 1.3; EVIDENCE 3.1-1, 3.1-2 notes |
| AS | Amana Spektra Spiral Plunge v24, 2 flute | Wood/Plywood 1/8 in 0.1016, 6 mm 0.127. MDF 1/8 in 0.127, 6 mm 0.1524 | 3.1-1 to 3.1-8 |
| ON | Onsrud 52-200/57-200, 2 flute upcut | sw 1/8 0.152-0.203, 1/4 0.178-0.229; hw 0.076-0.127, 0.127-0.178; mdf 0.127-0.178, 0.152-0.203; ply 0.127-0.178, 0.152-0.203 | 3.1-1 to 3.1-8 |
| C3D | Carbide 3D measured chart, #201 1/4 in 3 flute slot | pine 0.0295, mahogany 0.0270, MDF 0.0423, plywood 0.0415 | 6-3, 6-5, 6-6, 6-7 |
| AB7 | Amana Spiral Ball Nose v7, 2 flute | sw 1/8 0.127-0.178, 1/4 0.178-0.229; hw 0.076-0.127, 0.127-0.178; mdf 0.127-0.178, 0.152-0.203; no plywood | 3.2-1 to 3.2-7 |
| AZ | Amana ZrN / Spektra 2D-3D carving, "Wood, MDF, Sign-Foam" | 3 flute ball 1/8 in 0.038-0.064; 3 flute extra-long ball 1/4 in 0.102-0.152; 2 flute ball 1/4 in 0.178-0.229 | 3.3-1 note, 4.1-26, 4.4-11 to 4.4-14; MRC 2.3 |
| IDC | IDC Woodcraft, benchtop, "soft, medium and moderately hard wood" | ball 1/8 in 0.0346, 1/4 in 0.0468; 60 deg V 1/4 in 0.0346 | 3.2-2 note, 3.2-14, 3.4-2, 3.4-12 |
| AV | Amana 2 flute V-groove 15/60/90 deg, soft and hard wood; AMS-159 60 deg 2 flute | IPM-derived 0.0353-0.0882 (mid 0.0617); printed column 0.0762-0.1778 (mid 0.127), which is 1-flute arithmetic (4.4-1); AMS-159 hw 0.0635 | 4.4-1, 4.4-2 |
| O77 | Onsrud 77-100 tapered ball, Hard Wood, Soft Wood, MDF, Hard Plywood | 1/8 in 0.076-0.127 (mid 0.1016); 1/4 in 0.127-0.178 (mid 0.1524) | 3.5-1 to 3.5-5 |
| ARD | Amana v24 "Ramp Down" rule, 2 flute, derived per tooth | wood/ply 1/8 in 0.0512, 6 mm 0.0635; MDF 1/8 in 0.0635, 6 mm 0.0758 | 3.1-10 to 3.1-14 |

Material shorthand: sw softwood, hw hardwood, mdf MDF, ply plywood_hardwood.
Diameters: d3 = 3.175 mm, d6 = 6.0 mm (V-bit: d6 = 6.35 mm, d12 = 12.7 mm).

## 2. The table

Where four materials share one verdict, one reason and one figure set, one row
carries them and the cell count is the sum. Ratios are `r_chip_load_mm` divided
by the published midpoint, written d3 / d6. The ratio that decides is first.

### 2.1 EndMill (80 cells)

| Tool family | Feeds family / role | Material | Operations covered | Cells | Verdict | Backing figure (source, row) | Ratio r_chip_load / published mid | EVIDENCE rows | Reason |
|---|---|---|---|---|---|---|---|---|---|
| EndMill | Trace / Finish | sw | Trace | 2 | BACKED | AC 1/8 in wood; AC 1/4 in wood and AS 6 mm | 1.74 / 0.91 (AC); AS 0.48 / 0.56; Onsrud 64/65-000 d3 0.64; ON 0.27 / 0.35; C3D d6 2.43 | 3.1-1, 3.1-20, 6-3 | AC backs both diameters. The 2 flute Onsrud series is 0.27 to 0.35. The machine maker's pine row is 2.4x. Stepover and depth have no source (3.1-20). |
| EndMill | Trace / Finish | hw | Trace | 2 | BACKED | AC 1/8 in wood; C3D mahogany | 1.11 / 1.70 (C3D); AC d6 0.58; ON 0.31 / 0.30 | 3.1-2, 3.1-17, 6-5 | AC and C3D back it. Onsrud and Freud put it near one third. |
| EndMill | Trace / Finish | mdf | Trace | 2 | BACKED | AC 1/8 in MDF; C3D MDF | 0.64 / 1.25; AC d6 0.34; ON 0.23 / 0.30 | 3.1-3, 6-6 | Backed. Every chart puts MDF above wood; the formula puts it below (3.1-3). |
| EndMill | Trace / Finish | ply | Trace | 2 | BACKED | AC 1/8 in plywood; C3D plywood | 1.23 / 1.22; AC d6 0.64; ON 0.22 / 0.28 | 3.1-4, 6-7 | Backed. |
| EndMill | Parallel / Finish | sw | HorizontalFinish, RadialFinish, RampFinish | 6 | BACKED | as Trace sw | 1.74 / 0.91 | 3.1-5, 3.1-21 | Same chipload and figures as Trace. A finish pass has no published chipload correction. |
| EndMill | Parallel / Finish | hw | HorizontalFinish, RadialFinish, RampFinish | 6 | BACKED | as Trace hw | 1.11 / 1.70 | 3.1-6, 3.1-21 | As Trace hw. |
| EndMill | Parallel / Finish | mdf | HorizontalFinish, RadialFinish, RampFinish | 6 | BACKED | as Trace mdf | 0.64 / 1.25 | 3.1-7, 3.1-21 | As Trace mdf. |
| EndMill | Parallel / Finish | ply | HorizontalFinish, RadialFinish, RampFinish | 6 | BACKED | as Trace ply | 1.23 / 1.22 | 3.1-8, 3.1-21 | As Trace ply. |
| EndMill | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | CLUELESS (TOOL RULE) | none | n/a | none (new: registry.rs Chamfer, Inlay, VCarve `required_kinds: &[CutterKind::VBit]`; `curve_engrave.rs` `vbit_half_angle` returns `InvalidTool`) | Engine's own tool rule. The generator refuses a flat end mill on these operations; Suggest still ships a recipe. |
| EndMill | Trace / Finish | sw, hw, mdf, ply | Pencil | 8 | CLUELESS (TOOL RULE) | none | n/a | 3.1-22, 4.4-15, 4.4-16 | Engine's own tool rule: Critical `compat.end_mill_on_scallop_pencil`. No source requires a ball for pencil (4.4-16); see judgement call 7. |
| EndMill | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | CLUELESS (NO FIGURE) | none for a straight plunge; ARD is a ramp | vs ARD: sw 2.37 / 2.82, hw 1.52 / 1.81, mdf 1.41 / 1.74, ply 1.67 / 1.99. vs Onsrud 72-000 (boring drill): sw 0.48 / 0.50 | 3.1-9 to 3.1-16, 6-8, 6-9, 4.3-14 | No wood figure exists for a plunge drill with an end mill. The multiplier 2.5 is unsourced. The ramp rule is a different cut; see section 2.6. |

### 2.2 BallNose (128 cells)

| Tool family | Feeds family / role | Material | Operations covered | Cells | Verdict | Backing figure (source, row) | Ratio r_chip_load / published mid | EVIDENCE rows | Reason |
|---|---|---|---|---|---|---|---|---|---|
| BallNose | Adaptive / Roughing | sw | Adaptive, Adaptive3d | 4 | BACKED | AZ 3 flute ball 1/8 in; AZ 3 flute XL ball 1/4 in; IDC ball | 0.95 / 0.56 (AZ); IDC 1.40 / 1.53; AB7 0.32 / 0.35 | 3.2-3, 3.2-8 | Two vendors back both diameters. The 2 flute Amana ball chart puts it near one third. |
| BallNose | Adaptive / Roughing | hw | Adaptive, Adaptive3d | 4 | BACKED | AZ 3 flute ball 1/8 in; IDC ball 1/4 in | 0.61 / 0.98 (IDC only at d6); AZ XL d6 0.36; AB7 0.31 / 0.30 | 3.2-1, 3.2-8 | Backed. At d6 only IDC backs it; see judgement call 1. |
| BallNose | Adaptive / Roughing | mdf | Adaptive, Adaptive3d | 4 | CLUELESS (OUTSIDE) | AZ, AB7 (MDF named) | d3 0.70 (AZ); d6 0.41 (AZ XL), 0.26 (AZ 2 flute), 0.30 (AB7) | 3.2-5 | At d6 every MDF ball figure puts the formula below half. IDC does not name MDF. |
| BallNose | Adaptive / Roughing | ply | Adaptive, Adaptive3d | 4 | CLUELESS (NO FIGURE) | none | n/a | 3.2-7 | No vendor publishes a ball-nose plywood row. |
| BallNose | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | BACKED | as Adaptive sw | 0.95 / 0.56 | 3.2-4, 3.2-9, 4.4-10, 4.4-11 | As Adaptive sw. The flat-clearing advisory is a Hint, not a tool rule. |
| BallNose | Pocket / Roughing | hw | Face, Pocket, Rest, Zigzag | 8 | BACKED | as Adaptive hw | 0.61 / 0.98 | 3.2-2, 3.2-9, 4.4-12 | As Adaptive hw. |
| BallNose | Pocket / Roughing | mdf | Face, Pocket, Rest, Zigzag | 8 | CLUELESS (OUTSIDE) | as Adaptive mdf | 0.70 / 0.41 | 3.2-6, 4.4-13 | As Adaptive mdf. |
| BallNose | Pocket / Roughing | ply | Face, Pocket, Rest, Zigzag | 8 | CLUELESS (NO FIGURE) | none | n/a | 3.2-7, 4.4-14 | As Adaptive ply. |
| BallNose | Contour / Roughing | sw | Profile | 2 | BACKED | as Adaptive sw | 0.95 / 0.56 | 3.2-4 | As Adaptive sw. |
| BallNose | Contour / Roughing | hw | Profile | 2 | BACKED | as Adaptive hw | 0.61 / 0.98 | 3.2-2 | As Adaptive hw. |
| BallNose | Contour / Roughing | mdf | Profile | 2 | CLUELESS (OUTSIDE) | as Adaptive mdf | 0.70 / 0.41 | 3.2-6 | As Adaptive mdf. |
| BallNose | Contour / Roughing | ply | Profile | 2 | CLUELESS (NO FIGURE) | none | n/a | 3.2-7 | As Adaptive ply. |
| BallNose | Contour / SemiFinish | sw | Waterline | 2 | BACKED | IDC ball (a 3D modelling row); AZ | IDC 1.40 / 1.53; AZ 0.95 / 0.56 | 3.2-10, 3.2-11, 3.2-15 | IDC is the nearest finishing figure. The depth 1.0 mm has no source (3.2-11); R2 owns it. |
| BallNose | Contour / SemiFinish | hw | Waterline | 2 | BACKED | IDC ball; AZ 1/8 in | 0.90 / 0.98 (IDC); AZ 0.61 / 0.36 | 3.2-10, 3.2-11, 3.2-14 | Backed. At d6 only IDC backs it. |
| BallNose | Contour / SemiFinish | mdf | Waterline | 2 | CLUELESS (OUTSIDE) | AZ, AB7 | 0.70 / 0.41 | 3.2-16 | As Adaptive mdf. |
| BallNose | Contour / SemiFinish | ply | Waterline | 2 | CLUELESS (NO FIGURE) | none | n/a | 3.2-17 | As Adaptive ply. |
| BallNose | Contour / Finish | sw | SteepShallow | 2 | BACKED | as Waterline sw | 1.40 / 1.53 | 3.2-11, 3.2-12, 3.2-15 | As Waterline sw. |
| BallNose | Contour / Finish | hw | SteepShallow | 2 | BACKED | as Waterline hw | 0.90 / 0.98 | 3.2-11, 3.2-12, 3.2-14 | As Waterline hw. |
| BallNose | Contour / Finish | mdf | SteepShallow | 2 | CLUELESS (OUTSIDE) | AZ, AB7 | 0.70 / 0.41 | 3.2-16 | As Adaptive mdf. |
| BallNose | Contour / Finish | ply | SteepShallow | 2 | CLUELESS (NO FIGURE) | none | n/a | 3.2-17 | As Adaptive ply. |
| BallNose | Trace / Finish | sw | Trace, Pencil | 4 | BACKED | as Waterline sw | 1.40 / 1.53 | 3.2-13, 3.2-15 | As Waterline sw. A ball is the usual pencil tool (4.4-16). |
| BallNose | Trace / Finish | hw | Trace, Pencil | 4 | BACKED | as Waterline hw | 0.90 / 0.98 | 3.2-13, 3.2-14 | As Waterline hw. |
| BallNose | Trace / Finish | mdf | Trace, Pencil | 4 | CLUELESS (OUTSIDE) | AZ, AB7 | 0.70 / 0.41 | 3.2-16 | As Adaptive mdf. |
| BallNose | Trace / Finish | ply | Trace, Pencil | 4 | CLUELESS (NO FIGURE) | none | n/a | 3.2-17 | As Adaptive ply. |
| BallNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | CLUELESS (TOOL RULE) | none | n/a | 3.2-13, 3.2-14 (new: registry V-bit rule) | Engine's own tool rule. The generator refuses a ball on these operations. |
| BallNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | CLUELESS (NO FIGURE) | none; nearest Z figure IDC ball plunge 381 / 762 mm/min | shipped plunge / IDC plunge: 2.9-3.3 / 2.1-3.1 | 3.2-18, 3.2-19 | No vendor publishes a ball-nose drilling row. The shipped plunge is 2.3x to 2.7x the engine's own ball plunge cap. |

### 2.3 BullNose (120 cells)

| Tool family | Feeds family / role | Material | Operations covered | Cells | Verdict | Backing figure (source, row) | Ratio r_chip_load / published mid | EVIDENCE rows | Reason |
|---|---|---|---|---|---|---|---|---|---|
| BullNose | Parallel / Finish | sw, hw, mdf, ply | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 32 | CLUELESS (NO FIGURE) | none for a bull nose | with flat rows as stand-in: as EndMill (0.64 to 1.74) | 3.3-1 to 3.3-8, 3.3-10, 3.3-16, 3.3-19 | No wood vendor publishes a bull-nose or corner-radius row. See judgement call 2. |
| BullNose | Scallop / Finish | sw, hw, mdf, ply | SpiralFinish | 8 | CLUELESS (NO FIGURE) | none | as above | 3.3-11, 3.3-16 | No bull-nose figure exists. |
| BullNose | Scallop / Finish | sw, hw, mdf, ply | Scallop, UnifiedFinish | 16 | CLUELESS (TOOL RULE) | none | n/a | 2-7 (registry `required_kinds: &[Ball, TaperedBall]`; `finish_raster.rs:178`, `finish_3d.rs:294` refuse at generation) | Engine's own tool rule. The generator refuses a bull nose; the feeds path accepts it. |
| BullNose | Trace / Finish | sw, hw, mdf, ply | Trace, Pencil, ProjectCurve | 24 | CLUELESS (NO FIGURE) | none | as above | 3.3-12, 3.3-18, 4.4-8 | No bull-nose figure exists for a trace, pencil or projected-curve pass. |
| BullNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | CLUELESS (TOOL RULE) | none | n/a | 3.3-12 (new: registry V-bit rule) | Engine's own tool rule. |
| BullNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | CLUELESS (NO FIGURE) | none | n/a | 3.3-15, 6-8 | No wood figure exists for a plunge drill with a bull nose. |

### 2.4 VBit (128 cells)

| Tool family | Feeds family / role | Material | Operations covered | Cells | Verdict | Backing figure (source, row) | Ratio r_chip_load / published mid | EVIDENCE rows | Reason |
|---|---|---|---|---|---|---|---|---|---|
| VBit | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | BACKED | AV 60 deg 2 flute soft wood (derived mid) | 1.20 / 1.83 (AV derived); AV printed 0.58 / 0.89; IDC d6 2.14 | 3.4-1, 3.4-2, 3.4-6, 4.4-1 | AV backs both diameters. IDC is above 2x at d6. The chip is priced at the nominal diameter (3.4-6), and the stepover leaves ridges (3.4-1); those are not chipload defects. |
| VBit | Pocket / Roughing | hw | Face, Pocket, Rest, Zigzag | 8 | BACKED | AV hard wood; AMS-159; IDC | 0.77 / 1.18 (AV); IDC d6 1.37; AMS-159 0.75 / 1.14 | 3.4-3, 4.4-2 | Three figures back it. |
| VBit | Contour / Roughing | sw | Profile | 2 | BACKED | as Pocket sw | 1.20 / 1.83 | 3.4-10 | A V-bit profile is a V-groove along a boundary. EVIDENCE compared a 90 deg insert (not_comparable); this judgement uses the 60 deg groove chart. |
| VBit | Contour / Roughing | hw | Profile | 2 | BACKED | as Pocket hw | 0.77 / 1.18 | 3.4-11 | As above; 3.4-11 found no profile row, the groove chart applies. |
| VBit | Trace / Finish | sw | ProjectCurve | 2 | BACKED | as Pocket sw | 1.20 / 1.83 | 4.4-6, 4.4-7 | A projected curve is a trace at variable Z (4.4-6). The trace chart is the same operation family. The RPM is 0.47 to 0.56 of the tool's own anchor (4.4-7). |
| VBit | Trace / Finish | hw | ProjectCurve | 2 | BACKED | as Pocket hw | 0.77 / 1.18 | 4.4-6, 4.4-7 | As above. |
| VBit | Adaptive / Roughing | sw, hw, mdf, ply | Adaptive, Adaptive3d | 16 | CLUELESS (NO FIGURE) | none comparable | IDC clear pass is Z-level clearing at 1.27 mm, not adaptive | 3.4-8, 3.4-9 | No vendor publishes adaptive clearing with a V-bit. The shipped depth passes the end of the cone. |
| VBit | Parallel / Finish | sw | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 8 | CLUELESS (OUTSIDE) | AV, IDC | RampFinish 0.0172: AV 0.28, IDC 0.497. The other three: AV 1.20 / 1.83 | 3.4-12, 3.4-13 | RampFinish prices the chip at the engaged width, below half of every figure. The worse chipload decides the class. |
| VBit | Parallel / Finish | hw | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 8 | CLUELESS (OUTSIDE) | AV, IDC | RampFinish 0.0110: AV 0.18, IDC 0.32 | 3.4-12, 3.4-13 | As sw. |
| VBit | Contour / SemiFinish | sw | Waterline | 2 | CLUELESS (OUTSIDE) | IDC (1/4 in only); AV | d6 IDC 0.76; d12 AV 0.42 | 3.4-12, 3.4-13 | At d12 the only figure puts it below half. No vendor prints a V-bit 3D finish chart. |
| VBit | Contour / SemiFinish | hw | Waterline | 2 | CLUELESS (OUTSIDE) | IDC; AV | IDC 0.486; AV 0.27 | 3.4-12, 3.4-13 | Below half of every figure. |
| VBit | Contour / Finish | sw | SteepShallow | 2 | CLUELESS (OUTSIDE) | as Waterline sw | d6 0.76; d12 0.42 | 3.4-12, 3.4-13 | As Waterline sw. |
| VBit | Contour / Finish | hw | SteepShallow | 2 | CLUELESS (OUTSIDE) | as Waterline hw | 0.486; 0.27 | 3.4-12, 3.4-13 | As Waterline hw. |
| VBit | Pocket / Roughing; Contour / Roughing; Contour / SemiFinish; Contour / Finish; Parallel / Finish; Trace / Finish | mdf, ply | Face, Pocket, Rest, Zigzag, Profile, Waterline, SteepShallow, DropCutter, HorizontalFinish, RadialFinish, RampFinish, ProjectCurve | 48 | CLUELESS (NO FIGURE) | none | n/a | 4.4-3, 4.4-4, 4.1-25 | No V-bit chart prints an MDF or plywood row. IDC names wood only. |
| VBit | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | CLUELESS (NO FIGURE) | none; nearest IDC V plunge 508 mm/min | shipped plunge / IDC plunge: d6 3.0 to 4.8 | 3.4-14, 3.4-15 | No V-bit drilling figure exists. A V-bit makes a cone, not a bore, so AlignmentPinDrill makes no pin hole. |

The VBit mdf/ply row holds 48 cells (12 operations x 2 diameters x 2
materials). VBit Adaptive mdf/ply cells are in the Adaptive row.

### 2.5 TaperedBallNose (128 cells)

The Onsrud 77-100 row prints one band for all four materials. The formula splits
them by hardness, so only softwood reaches the band.

| Tool family | Feeds family / role | Material | Operations covered | Cells | Verdict | Backing figure (source, row) | Ratio r_chip_load / published mid | EVIDENCE rows | Reason |
|---|---|---|---|---|---|---|---|---|---|
| TaperedBallNose | Adaptive / Roughing | sw | Adaptive, Adaptive3d | 4 | BACKED | O77 Soft Wood 1/8 and 1/4 in | 0.51 / 0.51 | 3.5-2, 3.5-8, 3.5-11 | Backed at the lower edge. The only published tapered-ball wood row. |
| TaperedBallNose | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | BACKED | O77 Soft Wood | 0.51 / 0.51 | 3.5-2, 3.5-9, 3.5-10, 3.5-19 | As Adaptive sw. |
| TaperedBallNose | Contour / Roughing | sw | Profile | 2 | BACKED | O77 Soft Wood | 0.51 / 0.51 | 3.5-2 | As Adaptive sw. |
| TaperedBallNose | Trace / Finish | sw | Trace, Pencil | 4 | BACKED | O77 Soft Wood | 0.51 / 0.51 | 3.5-5, 3.5-14 | As Adaptive sw. No vendor prints a trace figure; the 1 x D row sets an upper bound (3.5-5). |
| TaperedBallNose | Contour / SemiFinish | sw | Waterline | 2 | CLUELESS (OUTSIDE) | O77 Soft Wood | 0.46 / 0.39 | 3.5-6, 3.5-15 | At the engaged diameter the formula is below half of the band. The LUT tapered rows have no printed source. |
| TaperedBallNose | Contour / Finish | sw | SteepShallow | 2 | CLUELESS (OUTSIDE) | O77 Soft Wood | 0.46 / 0.39 | 3.5-6, 3.5-12 | As Waterline sw. |
| TaperedBallNose | Adaptive / Roughing | hw, mdf, ply | Adaptive, Adaptive3d | 12 | CLUELESS (OUTSIDE) | O77 Hard Wood, MDF, Hard Plywood | hw 0.33 / 0.32; mdf 0.38 / 0.37; ply 0.36 / 0.36 | 3.5-1, 3.5-3, 3.5-4 | Onsrud puts the formula near one third of its band. |
| TaperedBallNose | Pocket / Roughing | hw, mdf, ply | Face, Pocket, Rest, Zigzag | 24 | CLUELESS (OUTSIDE) | O77 | as Adaptive | 3.5-1, 3.5-3, 3.5-4 | As Adaptive hw. |
| TaperedBallNose | Contour / Roughing | hw, mdf, ply | Profile | 6 | CLUELESS (OUTSIDE) | O77 | as Adaptive | 3.5-1, 3.5-3, 3.5-4 | As Adaptive hw. |
| TaperedBallNose | Contour / SemiFinish | hw, mdf, ply | Waterline | 6 | CLUELESS (OUTSIDE) | O77 | hw 0.29 / 0.25; mdf 0.34 / 0.29; ply 0.32 / 0.28 | 3.5-6 | Below one third at the engaged diameter. |
| TaperedBallNose | Contour / Finish | hw, mdf, ply | SteepShallow | 6 | CLUELESS (OUTSIDE) | O77 | as Waterline | 3.5-6 | As Waterline hw. |
| TaperedBallNose | Trace / Finish | hw, mdf, ply | Trace, Pencil | 12 | CLUELESS (OUTSIDE) | O77 | as Adaptive | 3.5-5 | As Adaptive hw. |
| TaperedBallNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | CLUELESS (TOOL RULE) | none | n/a | 3.5-18, 3.5-14 (new: registry V-bit rule) | Engine's own tool rule. A tapered ball does not cut a flat chamfer, and the generator refuses it. |
| TaperedBallNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | CLUELESS (NO FIGURE) | none | n/a | 3.5-16, 3.5-17 | No vendor publishes drilling with a tapered ball. The shipped plunge is 2.7x the engine's own tapered plunge cap. |

### 2.6 The drill classes (80 cells, all five tool families)

No drilling figure for wood backs the shipped plunge feed on any tool family.

- The engine has no twist-drill tool type. Every Drill and AlignmentPinDrill
  cell drills with a milling cutter.
- The one wood drill chart, Onsrud 72-000, is for a boring drill on a gang
  borer (3.1-15, 6-9). It is a different tool family. Against it the formula
  is 0.31 to 0.50.
- No publisher states a drill-to-mill chipload ratio (6-8). The multiplier 2.5
  and the plunge envelope are repo-authored (4.3-14).
- The Amana "Ramp Down" rule (ARD) is the nearest end-mill figure. It is a ramp
  entry, not a straight plunge (EVIDENCE 7, section 3.1, item 3). Against it the
  EndMill drill chip is 1.41 to 1.99 in hw, mdf and ply, and 2.37 to 2.82 in sw.
- The ball, tapered-ball and V-bit plunge rates that IDC publishes are milling
  plunges. The shipped drill feed is 2.1x to 4.8x of them (3.2-18, 3.4-14).
- The engine's own ball and tapered plunge caps are 150 x D. Step 9c then sets
  the plunge equal to the drill feed, 2.3x to 2.7x above the cap (3.2-19,
  3.5-16).

## 3. Counts

| Tool family | Cells | BACKED | CLUELESS | TOOL RULE | NO FIGURE | OUTSIDE |
|---|---|---|---|---|---|---|
| EndMill | 80 | 32 | 48 | 32 | 16 | 0 |
| BallNose | 128 | 44 | 84 | 24 | 38 | 22 |
| BullNose | 120 | 0 | 120 | 40 | 80 | 0 |
| VBit | 128 | 24 | 104 | 0 | 80 | 24 |
| TaperedBallNose | 128 | 18 | 110 | 24 | 16 | 70 |
| **Total** | **584** | **118** | **466** | **120** | **230** | **116** |

NO FIGURE includes the 80 drill cells. By material, the BACKED cells are: sw 60,
hw 42, mdf 8, ply 8.

## 4. The encoding for the R1 editor

### 4.1 The tool-rule cells (120): no new table

The registry already holds the rule. The editor makes
`feeds::validate_tool_for_operation` read
`OperationType::registry_entry().tool_constraints.allows(kind)` when
`input.operation_kind` is `Some`. That is R1 condition 2 in `RULINGS.md`.

- VCarve, Inlay, Chamfer: `required_kinds: &[CutterKind::VBit]` refuses
  EndMill, BallNose, BullNose and TaperedBallNose (96 cells).
- Scallop, UnifiedFinish: `required_kinds: &[Ball, TaperedBall]` refuses
  BullNose (16 cells).
- Pencil is `ANY_TOOL` today. The static check at
  `diagnostics/adapters/from_static_checks.rs:163-185` calls a flat end mill
  Critical. To refuse EndMill/Pencil (8 cells), the Pencil row must exclude
  `CutterKind::Flat`. See judgement call 7 first.

The error is `FeedsError::WrongToolForOperation`. It must carry the
`OperationType`, and the V-bit arm must print a sentence, not Debug (2-6).
Proposed text:

`"{operation} does not accept a {tool family} cutter: the engine's own tool rule refuses the pair, and no published figure exists for it."`

### 4.2 The evidence cells: one table inside `feeds_support`

Mechanism: one static table in `crates/rs_cam_core/src/feeds/support.rs`,
`FORMULA_BACKING`. The key is (`vendor_lut::ToolFamily`, `OperationFamily`,
`PassRole`, `vendor_lut::MaterialFamily`). The value is `Backed` or
`Clueless { reason: &'static str }`. `support_for_lookup` reads it only after
the row lookup finds no row:

1. A row matched: `VendorBacked`. The table does not act (R5 owns these cells).
2. No row, and the key is `Backed`: `FormulaOnly { source }`, as today.
3. No row, and the key is `Clueless`: `Refuse { reason }`.
4. No row, and the key is absent: `Refuse` with the default reason below.

Why this mechanism and not `OperationSpec::feeds_formula_source`:

- The verdicts change with tool family and material inside one operation. Only
  Drill and AlignmentPinDrill are CLUELESS on every tool and material. A static
  per-operation field cannot express the other classes.
- The inputs already carry every key part: `input.tool_geometry.cutter_kind()
  .lut_family()`, `input.operation`, `input.pass_role` and
  `vendor_normalize::material_to_lut(input.material)`.
- Drill goes in the same table, so one mechanism decides every evidence refusal.
  `feeds_formula_source` stays the provenance string.
- The key has no diameter. The judgement used d3.175 and d6.0 (V-bit 6.35 and
  12.7). Other diameters inherit the class verdict.

Two changes outside the table:

- `validate_tool_for_operation` calls `feeds_support` today only when
  `feeds_formula_source` is `None` (feeds/mod.rs:969-979). The editor removes
  that condition, so a `Clueless` key refuses for every operation.
- A sentry pins `r_chip_load_mm` at the walked cells. A change to k0, p, q or
  a material hardness then fails the sentry and forces a new judgement pass.

### 4.3 Refuse reason texts

The error prints the operation, tool family and material beside the reason, so
a text that covers several materials names the tool and operation family only.

| Key (tool family, operation family / role, materials) | `Refuse { reason }` |
|---|---|
| BallNose; Adaptive/R, Pocket/R, Contour/R, Contour/SF, Contour/F, Trace/F; Mdf | "No published figure backs the formula for a ball-nose cutter on adaptive, pocket, contour or trace passes in MDF: the 1/4 in charts put it below half their band." |
| BallNose; same six families; PlywoodHardwood | "No published chipload exists for a ball-nose cutter on adaptive, pocket, contour or trace passes in plywood." |
| BullNose; Parallel/F; all four | "No published wood chipload exists for a bull-nose cutter on a parallel finish pass." |
| BullNose; Scallop/F; all four | "No published wood chipload exists for a bull-nose cutter on a scallop-family finish pass." |
| BullNose; Trace/F; all four | "No published wood chipload exists for a bull-nose cutter on a trace, pencil or projected-curve pass." |
| VBit; Adaptive/R; all four | "No published figure exists for a V-bit on adaptive clearing, and the recipe depth passes the end of the cone." |
| VBit; Parallel/F; Softwood, Hardwood | "No published figure backs the formula for a V-bit on parallel finish passes: at the engaged width the chip is below half of every V-bit figure." |
| VBit; Contour/SF and Contour/F; Softwood, Hardwood | "No published figure backs the formula for a V-bit on waterline or steep-shallow passes: no vendor prints a V-bit 3D finish chart." |
| VBit; Pocket/R, Contour/R, Contour/SF, Contour/F, Parallel/F, Trace/F; Mdf, PlywoodHardwood | "No published V-bit chipload exists for MDF or plywood on pocket, contour, parallel or trace passes." |
| TaperedBallNose; Adaptive/R, Pocket/R, Contour/R, Trace/F; Hardwood, Mdf, PlywoodHardwood | "No published figure backs the formula for a tapered ball-nose on adaptive, pocket, contour or trace passes: Onsrud 77-100 puts it near one third of its band." |
| TaperedBallNose; Contour/SF and Contour/F; all four | "No published figure backs the formula for a tapered ball-nose on waterline or steep-shallow passes: it is below half of the Onsrud 77-100 band." |
| EndMill; Drill/R; all four | "No published wood figure exists for a plunge drill with a flat end mill, and the drill multiplier 2.5 is unsourced." |
| BallNose; Drill/R; all four | "No published wood figure exists for a plunge drill with a ball-nose cutter, and the drill multiplier 2.5 is unsourced." |
| BullNose; Drill/R; all four | "No published wood figure exists for a plunge drill with a bull-nose cutter, and the drill multiplier 2.5 is unsourced." |
| VBit; Drill/R; all four | "No published wood figure exists for a plunge drill with a V-bit, and a V-bit cuts a cone, not a bore." |
| TaperedBallNose; Drill/R; all four | "No published wood figure exists for a plunge drill with a tapered ball-nose, and the drill multiplier 2.5 is unsourced." |
| Default (key absent) | "No agent judgement covers this tool family, operation family and material, so the formula has no checked basis." |

`Backed` keys: EndMill Trace/F and Parallel/F (all four); BallNose Adaptive/R,
Pocket/R, Contour/R, Contour/SF, Contour/F, Trace/F (Softwood, Hardwood); VBit
Pocket/R, Contour/R, Trace/F (Softwood, Hardwood); TaperedBallNose Adaptive/R,
Pocket/R, Contour/R, Trace/F (Softwood).

## 5. Judgement calls for the operator

1. **IDC Woodcraft counts as a vendor chart.** IDC is a bit retailer with one
   benchtop row for all woods. BallNose hardwood at d6 rests on IDC alone. If
   IDC does not count, 22 BallNose hardwood cells become CLUELESS (OUTSIDE,
   0.30 to 0.36). The other BACKED classes keep a second source.
2. **BullNose is CLUELESS on every class.** The rule "same tool family" excludes
   flat-end rows. The engine already backs bull roughing with flat rows
   (VendorBacked, 3.3-16), and Amana's metal chart prints one column for
   square and corner-radius tools. If flat rows count for a bull nose, the 64
   NO FIGURE bull cells take the EndMill ratios and become BACKED. This verdict
   also overrides R5 recommendation 5 ("stay formula-only for bull finishing").
3. **The Amana compression chart carries EndMill at d3.175.** It is the only flat
   2 flute vendor row in range at 1/8 in (0.64 to 1.74). MRC 1.3 records that it
   is 3x to 5x lower than Onsrud, and Phase 2 did not fetch it again. Without it,
   the 24 BACKED EndMill cells in hardwood, MDF and plywood become CLUELESS
   (OUTSIDE, 0.22 to 0.47 at d3.175). Softwood keeps Onsrud 64/65-000 at d3.175
   (0.64, a single-flute series) and AS 6 mm at d6 (0.56).
4. **A 1 x D chipload per tooth counts for finish and trace passes.** EVIDENCE
   3.3-1 says the per-tooth quantity transfers; 3.2-14 to 3.2-17 call the same
   comparison not_comparable. This judgement follows 3.3-1.
5. **Both diameters must pass.** BallNose MDF passes at d3.175 (0.70) and fails at
   d6 (0.41). The table key has no diameter, so the class refuses at every
   diameter.
6. **Edge ratios.** TaperedBallNose softwood is BACKED at 0.51. V-bit RampFinish
   softwood (0.497) and V-bit hinted hardwood (0.486) are CLUELESS by less than
   0.02. The IDC tapered row (1/4 in body, 0.76 to 2.29 mm tip, 0.0401 mm/tooth)
   would back TaperedBallNose hardwood (0.84 / 1.23), but its tip does not match
   the walked tips, so this judgement does not admit it.
7. **EndMill on Pencil.** The engine's static check calls it Critical, but no
   source requires a ball for pencil milling (4.4-16). The other option: make
   the check a Hint, and let EndMill Pencil ship under the BACKED Trace/F class.
8. **Drill refusal has a product cost.** AlignmentPinDrill with an end mill gets
   no recipe. If a ramp-down figure counts for a plunge, EndMill drill in
   hardwood, MDF and plywood (12 cells, 1.41 to 1.99) becomes BACKED.
9. **An absent key refuses.** Material families outside the four walked ones
   (plywood_softwood, hdf, particleboard, plastics, aluminum, composites) have no
   judgement. Under the operator's rule they refuse until an agent judges them.
10. **R4 and R5 move these verdicts.** R5 adds Onsrud 77-100 rows, so many
    TaperedBallNose cells become VendorBacked and leave this table. A change to
    k0 or the hardness exponent changes every ratio above. Re-run this judgement
    after either change.
11. **V-bit d12.7 rests on a chart with no diameter.** Rule 5 lets the Amana
    15/60/90 deg chart cover 12.7 mm. EVIDENCE 3.4-7 found no 1/2 in 60 deg
    clearing figure, and MRC 5.5 calls the Amana V-bit depth condition unusable.
    If rule 5 is rejected, the 12 V-bit softwood BACKED cells become CLUELESS.
    The 12 hardwood cells stay BACKED only if AMS-159 (hw 0.0635, ratio 1.14) is
    a printed 1/2 in row. This is the call the agent is least sure of.
12. **BullNose on Scallop and UnifiedFinish takes one side of 2-7.** The registry
    refuses a bull nose; Fusion accepts it. If 2-7 resolves toward a bull nose,
    those 16 cells stay CLUELESS, but the reason moves from TOOL RULE to NO
    FIGURE, and the table in section 4.2 carries them.
13. **RampFinish refuses the whole V-bit Parallel class.** DropCutter,
    HorizontalFinish and RadialFinish in softwood and hardwood (12 cells) are
    inside AV on their own (1.20 / 1.83 and 0.77 / 1.18). The hinted RampFinish
    chip (3.4-13, a declared defect) is below half, and the key has no
    operation. The other option: fix 3.4-13 first, then judge the class again.
14. **Most TOOL RULE cells come from a code reading, not from Phase 2.** EVIDENCE
    names 3.1-22, 4.4-15 and 3.5-18 (16 cells: EndMill/Pencil 8, tapered
    Chamfer 8). The other 104 cells come from the registry V-bit rule on
    VCarve, Inlay and Chamfer and the ball-tip rule on Scallop and UnifiedFinish.
    This is R1 condition 2 in `RULINGS.md`.
