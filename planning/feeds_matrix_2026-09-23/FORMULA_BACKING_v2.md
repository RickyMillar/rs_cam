# Formula backing, v2: v1 filtered against the post-R1/R3/R5 matrix
Date: 2026-09-23 (evening). Status: mechanical derivation, not a new judgement.
## 1. Method

FORMULA_BACKING.md (e18d8155) judged every formula-only cell of the 2026-09-23
matrix against a published band, with the operator's threshold: outside 0.5x to
2x is CLUELESS. Since then R5 landed the printed vendor rows, R3 changed the
depth scale of the feed and the band, and R1 made Suggest refuse the tools the
registry rows refuse. None of these moved the formula's pre-derate chipload
(`r_chip_load_mm`), so a v1 verdict still holds for a cell that is still
formula-only. This file filters v1 against the re-run matrix:

- a cell that now matches a vendor row leaves the table (VendorBacked);
- a cell the registry tool rule now refuses is TOOL_RULE (it refuses already);
- every other cell keeps its v1 verdict.

Two Sonnet judgement agents stalled tonight without writing; this derivation
replaces the re-judgement they were asked for. It adds no new figure.

## 2. The table (v1 rows, with the current cell count and state)

| Tool family | Feeds family / role | Material | Operations covered | v1 cells | Now formula-only | Now vendor-backed | Now refused (tool rule) | Verdict (v1) | Ratio (v1) | EVIDENCE rows |
|---|---|---|---|---|---|---|---|---|---|---|
| EndMill | Trace / Finish | sw | Trace | 2 | 2 | 0 | 0 | BACKED | 1.74 / 0.91 (AC); AS 0.48 / 0.56; Onsrud 64/65-000 d3 0.64; ON 0.27 / 0.35; C3D d6 2.43 | 3.1-1, 3.1-20, 6-3 |
| EndMill | Trace / Finish | hw | Trace | 2 | 2 | 0 | 0 | BACKED | 1.11 / 1.70 (C3D); AC d6 0.58; ON 0.31 / 0.30 | 3.1-2, 3.1-17, 6-5 |
| EndMill | Trace / Finish | mdf | Trace | 2 | 2 | 0 | 0 | BACKED | 0.64 / 1.25; AC d6 0.34; ON 0.23 / 0.30 | 3.1-3, 6-6 |
| EndMill | Trace / Finish | ply | Trace | 2 | 2 | 0 | 0 | BACKED | 1.23 / 1.22; AC d6 0.64; ON 0.22 / 0.28 | 3.1-4, 6-7 |
| EndMill | Parallel / Finish | sw | HorizontalFinish, RadialFinish, RampFinish | 6 | 6 | 0 | 0 | BACKED | 1.74 / 0.91 | 3.1-5, 3.1-21 |
| EndMill | Parallel / Finish | hw | HorizontalFinish, RadialFinish, RampFinish | 6 | 6 | 0 | 0 | BACKED | 1.11 / 1.70 | 3.1-6, 3.1-21 |
| EndMill | Parallel / Finish | mdf | HorizontalFinish, RadialFinish, RampFinish | 6 | 6 | 0 | 0 | BACKED | 0.64 / 1.25 | 3.1-7, 3.1-21 |
| EndMill | Parallel / Finish | ply | HorizontalFinish, RadialFinish, RampFinish | 6 | 6 | 0 | 0 | BACKED | 1.23 / 1.22 | 3.1-8, 3.1-21 |
| EndMill | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | 0 | 0 | 24 | CLUELESS (TOOL RULE) | n/a | none (new: registry.rs Chamfer, Inlay, VCarve `required_kinds: &[CutterKind::VBit]`; `curve_engrave.rs` `vbit_half_angle` returns `InvalidTool`) |
| EndMill | Trace / Finish | sw, hw, mdf, ply | Pencil | 8 | 0 | 0 | 8 | CLUELESS (TOOL RULE) | n/a | 3.1-22, 4.4-15, 4.4-16 |
| EndMill | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | vs ARD: sw 2.37 / 2.82, hw 1.52 / 1.81, mdf 1.41 / 1.74, ply 1.67 / 1.99. vs Onsrud 72-000 (boring drill): sw 0.48 / 0.50 | 3.1-9 to 3.1-16, 6-8, 6-9, 4.3-14 |
| BallNose | Adaptive / Roughing | sw | Adaptive, Adaptive3d | 4 | 0 | 4 | 0 | BACKED | 0.95 / 0.56 (AZ); IDC 1.40 / 1.53; AB7 0.32 / 0.35 | 3.2-3, 3.2-8 |
| BallNose | Adaptive / Roughing | hw | Adaptive, Adaptive3d | 4 | 0 | 4 | 0 | BACKED | 0.61 / 0.98 (IDC only at d6); AZ XL d6 0.36; AB7 0.31 / 0.30 | 3.2-1, 3.2-8 |
| BallNose | Adaptive / Roughing | mdf | Adaptive, Adaptive3d | 4 | 0 | 4 | 0 | CLUELESS (OUTSIDE) | d3 0.70 (AZ); d6 0.41 (AZ XL), 0.26 (AZ 2 flute), 0.30 (AB7) | 3.2-5 |
| BallNose | Adaptive / Roughing | ply | Adaptive, Adaptive3d | 4 | 0 | 4 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-7 |
| BallNose | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | 0 | 8 | 0 | BACKED | 0.95 / 0.56 | 3.2-4, 3.2-9, 4.4-10, 4.4-11 |
| BallNose | Pocket / Roughing | hw | Face, Pocket, Rest, Zigzag | 8 | 0 | 8 | 0 | BACKED | 0.61 / 0.98 | 3.2-2, 3.2-9, 4.4-12 |
| BallNose | Pocket / Roughing | mdf | Face, Pocket, Rest, Zigzag | 8 | 0 | 8 | 0 | CLUELESS (OUTSIDE) | 0.70 / 0.41 | 3.2-6, 4.4-13 |
| BallNose | Pocket / Roughing | ply | Face, Pocket, Rest, Zigzag | 8 | 0 | 8 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-7, 4.4-14 |
| BallNose | Contour / Roughing | sw | Profile | 2 | 2 | 0 | 0 | BACKED | 0.95 / 0.56 | 3.2-4 |
| BallNose | Contour / Roughing | hw | Profile | 2 | 2 | 0 | 0 | BACKED | 0.61 / 0.98 | 3.2-2 |
| BallNose | Contour / Roughing | mdf | Profile | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.70 / 0.41 | 3.2-6 |
| BallNose | Contour / Roughing | ply | Profile | 2 | 2 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-7 |
| BallNose | Contour / SemiFinish | sw | Waterline | 2 | 2 | 0 | 0 | BACKED | IDC 1.40 / 1.53; AZ 0.95 / 0.56 | 3.2-10, 3.2-11, 3.2-15 |
| BallNose | Contour / SemiFinish | hw | Waterline | 2 | 2 | 0 | 0 | BACKED | 0.90 / 0.98 (IDC); AZ 0.61 / 0.36 | 3.2-10, 3.2-11, 3.2-14 |
| BallNose | Contour / SemiFinish | mdf | Waterline | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.70 / 0.41 | 3.2-16 |
| BallNose | Contour / SemiFinish | ply | Waterline | 2 | 2 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-17 |
| BallNose | Contour / Finish | sw | SteepShallow | 2 | 2 | 0 | 0 | BACKED | 1.40 / 1.53 | 3.2-11, 3.2-12, 3.2-15 |
| BallNose | Contour / Finish | hw | SteepShallow | 2 | 2 | 0 | 0 | BACKED | 0.90 / 0.98 | 3.2-11, 3.2-12, 3.2-14 |
| BallNose | Contour / Finish | mdf | SteepShallow | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.70 / 0.41 | 3.2-16 |
| BallNose | Contour / Finish | ply | SteepShallow | 2 | 2 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-17 |
| BallNose | Trace / Finish | sw | Trace, Pencil | 4 | 4 | 0 | 0 | BACKED | 1.40 / 1.53 | 3.2-13, 3.2-15 |
| BallNose | Trace / Finish | hw | Trace, Pencil | 4 | 4 | 0 | 0 | BACKED | 0.90 / 0.98 | 3.2-13, 3.2-14 |
| BallNose | Trace / Finish | mdf | Trace, Pencil | 4 | 4 | 0 | 0 | CLUELESS (OUTSIDE) | 0.70 / 0.41 | 3.2-16 |
| BallNose | Trace / Finish | ply | Trace, Pencil | 4 | 4 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.2-17 |
| BallNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | 0 | 0 | 24 | CLUELESS (TOOL RULE) | n/a | 3.2-13, 3.2-14 (new: registry V-bit rule) |
| BallNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | shipped plunge / IDC plunge: 2.9-3.3 / 2.1-3.1 | 3.2-18, 3.2-19 |
| BullNose | Parallel / Finish | sw, hw, mdf, ply | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 32 | 32 | 0 | 0 | CLUELESS (NO FIGURE) | with flat rows as stand-in: as EndMill (0.64 to 1.74) | 3.3-1 to 3.3-8, 3.3-10, 3.3-16, 3.3-19 |
| BullNose | Scallop / Finish | sw, hw, mdf, ply | SpiralFinish | 8 | 0 | 0 | 8 | CLUELESS (NO FIGURE) | as above | 3.3-11, 3.3-16 |
| BullNose | Scallop / Finish | sw, hw, mdf, ply | Scallop, UnifiedFinish | 16 | 0 | 0 | 16 | CLUELESS (TOOL RULE) | n/a | 2-7 (registry `required_kinds: &[Ball, TaperedBall]`; `finish_raster.rs:178`, `finish_3d.rs:294` refuse at generation) |
| BullNose | Trace / Finish | sw, hw, mdf, ply | Trace, Pencil, ProjectCurve | 24 | 16 | 0 | 8 | CLUELESS (NO FIGURE) | as above | 3.3-12, 3.3-18, 4.4-8 |
| BullNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | 0 | 0 | 24 | CLUELESS (TOOL RULE) | n/a | 3.3-12 (new: registry V-bit rule) |
| BullNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.3-15, 6-8 |
| VBit | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | 8 | 0 | 0 | BACKED | 1.20 / 1.83 (AV derived); AV printed 0.58 / 0.89; IDC d6 2.14 | 3.4-1, 3.4-2, 3.4-6, 4.4-1 |
| VBit | Pocket / Roughing | hw | Face, Pocket, Rest, Zigzag | 8 | 8 | 0 | 0 | BACKED | 0.77 / 1.18 (AV); IDC d6 1.37; AMS-159 0.75 / 1.14 | 3.4-3, 4.4-2 |
| VBit | Contour / Roughing | sw | Profile | 2 | 2 | 0 | 0 | BACKED | 1.20 / 1.83 | 3.4-10 |
| VBit | Contour / Roughing | hw | Profile | 2 | 2 | 0 | 0 | BACKED | 0.77 / 1.18 | 3.4-11 |
| VBit | Trace / Finish | sw | ProjectCurve | 2 | 2 | 0 | 0 | BACKED | 1.20 / 1.83 | 4.4-6, 4.4-7 |
| VBit | Trace / Finish | hw | ProjectCurve | 2 | 2 | 0 | 0 | BACKED | 0.77 / 1.18 | 4.4-6, 4.4-7 |
| VBit | Adaptive / Roughing | sw, hw, mdf, ply | Adaptive, Adaptive3d | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | IDC clear pass is Z-level clearing at 1.27 mm, not adaptive | 3.4-8, 3.4-9 |
| VBit | Parallel / Finish | sw | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 8 | 8 | 0 | 0 | CLUELESS (OUTSIDE) | RampFinish 0.0172: AV 0.28, IDC 0.497. The other three: AV 1.20 / 1.83 | 3.4-12, 3.4-13 |
| VBit | Parallel / Finish | hw | DropCutter, HorizontalFinish, RadialFinish, RampFinish | 8 | 8 | 0 | 0 | CLUELESS (OUTSIDE) | RampFinish 0.0110: AV 0.18, IDC 0.32 | 3.4-12, 3.4-13 |
| VBit | Contour / SemiFinish | sw | Waterline | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | d6 IDC 0.76; d12 AV 0.42 | 3.4-12, 3.4-13 |
| VBit | Contour / SemiFinish | hw | Waterline | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | IDC 0.486; AV 0.27 | 3.4-12, 3.4-13 |
| VBit | Contour / Finish | sw | SteepShallow | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | d6 0.76; d12 0.42 | 3.4-12, 3.4-13 |
| VBit | Contour / Finish | hw | SteepShallow | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.486; 0.27 | 3.4-12, 3.4-13 |
| VBit | Pocket / Roughing; Contour / Roughing; Contour / SemiFinish; Contour / Finish; Parallel / Finish; Trace / Finish | mdf, ply | Face, Pocket, Rest, Zigzag, Profile, Waterline, SteepShallow, DropCutter, HorizontalFinish, RadialFinish, RampFinish, ProjectCurve | 48 | 96 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 4.4-3, 4.4-4, 4.1-25 |
| VBit | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | shipped plunge / IDC plunge: d6 3.0 to 4.8 | 3.4-14, 3.4-15 |
| TaperedBallNose | Adaptive / Roughing | sw | Adaptive, Adaptive3d | 4 | 0 | 4 | 0 | BACKED | 0.51 / 0.51 | 3.5-2, 3.5-8, 3.5-11 |
| TaperedBallNose | Pocket / Roughing | sw | Face, Pocket, Rest, Zigzag | 8 | 0 | 8 | 0 | BACKED | 0.51 / 0.51 | 3.5-2, 3.5-9, 3.5-10, 3.5-19 |
| TaperedBallNose | Contour / Roughing | sw | Profile | 2 | 2 | 0 | 0 | BACKED | 0.51 / 0.51 | 3.5-2 |
| TaperedBallNose | Trace / Finish | sw | Trace, Pencil | 4 | 4 | 0 | 0 | BACKED | 0.51 / 0.51 | 3.5-5, 3.5-14 |
| TaperedBallNose | Contour / SemiFinish | sw | Waterline | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.46 / 0.39 | 3.5-6, 3.5-15 |
| TaperedBallNose | Contour / Finish | sw | SteepShallow | 2 | 2 | 0 | 0 | CLUELESS (OUTSIDE) | 0.46 / 0.39 | 3.5-6, 3.5-12 |
| TaperedBallNose | Adaptive / Roughing | hw, mdf, ply | Adaptive, Adaptive3d | 12 | 0 | 16 | 0 | CLUELESS (OUTSIDE) | hw 0.33 / 0.32; mdf 0.38 / 0.37; ply 0.36 / 0.36 | 3.5-1, 3.5-3, 3.5-4 |
| TaperedBallNose | Pocket / Roughing | hw, mdf, ply | Face, Pocket, Rest, Zigzag | 24 | 0 | 32 | 0 | CLUELESS (OUTSIDE) | as Adaptive | 3.5-1, 3.5-3, 3.5-4 |
| TaperedBallNose | Contour / Roughing | hw, mdf, ply | Profile | 6 | 8 | 0 | 0 | CLUELESS (OUTSIDE) | as Adaptive | 3.5-1, 3.5-3, 3.5-4 |
| TaperedBallNose | Contour / SemiFinish | hw, mdf, ply | Waterline | 6 | 8 | 0 | 0 | CLUELESS (OUTSIDE) | hw 0.29 / 0.25; mdf 0.34 / 0.29; ply 0.32 / 0.28 | 3.5-6 |
| TaperedBallNose | Contour / Finish | hw, mdf, ply | SteepShallow | 6 | 8 | 0 | 0 | CLUELESS (OUTSIDE) | as Waterline | 3.5-6 |
| TaperedBallNose | Trace / Finish | hw, mdf, ply | Trace, Pencil | 12 | 16 | 0 | 0 | CLUELESS (OUTSIDE) | as Adaptive | 3.5-5 |
| TaperedBallNose | Trace / Finish | sw, hw, mdf, ply | Chamfer, Inlay, VCarve | 24 | 0 | 0 | 24 | CLUELESS (TOOL RULE) | n/a | 3.5-18, 3.5-14 (new: registry V-bit rule) |
| TaperedBallNose | Drill / Roughing | sw, hw, mdf, ply | Drill, AlignmentPinDrill | 16 | 16 | 0 | 0 | CLUELESS (NO FIGURE) | n/a | 3.5-16, 3.5-17 |

## 3. Counts (current matrix, unique cells; a per-material v1 row outranks an "all" row)

| Tool family | Formula-only BACKED | Formula-only CLUELESS | Formula-only not covered by a v1 row | Formula-only total |
|---|---|---|---|---|
| EndMill | 32 | 16 | 0 | 48 |
| BallNose | 20 | 36 | 0 | 56 |
| BullNose | 0 | 64 | 0 | 64 |
| VBit | 24 | 104 | 0 | 128 |
| TaperedBallNose | 6 | 50 | 0 | 56 |
| **Total** | **82** | **270** | **0** | **352** |

Whole matrix (960 cells): refused today 192; formula-only 352; vendor-backed 416.

**If the CLUELESS verdicts were encoded today**, the refusals would be:
refused today 192 + formula-only CLUELESS 270 = **462 of 960**
(plus 0 formula-only cells no v1 row judged, which the rule "no judgement = clueless" would also refuse:
**462 of 960** at most).

## 4. Encoding

Unchanged from v1 section 4.2: one static table keyed by (tool family,
operation family, pass role, material family), read by `feeds_support` only
when no vendor row matched. The tool-rule cells need no table (R1 engine-rule
part, landed). The encoding waits for the operator's look at the count above.

## 5. Judgement calls

Unchanged from v1 section 5. The three that move the most cells: the 12.7 mm
V-bit backing (an Amana chart that prints no diameter), the Amana compression
chart as the 3.175 mm end-mill backing, and IDC Woodcraft counted as a vendor
chart. None of R3, R5 or R1 touched those questions.
