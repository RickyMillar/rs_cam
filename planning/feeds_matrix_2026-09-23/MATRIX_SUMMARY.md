# Feeds matrix 2026-09-23: summary (FM1 instrument)

The test `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs` writes this file. It records what the product doors answer. It adds no arithmetic and no verdict.

Run command:

```text
scripts/cargo_lane.sh test -p rs_cam_core -q --test feeds_matrix_instrument_fm1 -- --ignored --nocapture
```

## Walk

- Machine: `MachineProfile::default()` = `Generic Wood Router`. The repository holds no measured rigidity or power profile. The presets `shapeoko_vfd` and `shapeoko_makita` and the kinematics preset `shapeoko_xxl_ricky_tuned` exist; the instrument does not walk them.
- Workholding `Medium`, `SpindleStrategy::default()`, `SuggestContext::default()`, stock context top 0, bottom -18, height 18, padding 2.
- Cells: 5 tool types × 2 diameters × 24 operations × 4 materials = 960 cells. Flutes: 2 on every tool.

Diameters walked (small, common):

- `EndMill`: 3.175 and 6 mm. flat_end wood rows carry 3.175 (16 rows) and 6.0 (8 rows); 6.0 is shared with the other families.
- `BallNose`: 3.175 and 6 mm. ball_nose wood rows carry 3.175 (4 rows) and 6.0 (4 rows).
- `BullNose`: 3.175 and 6 mm. bull_nose wood rows carry only 6.0 (3 rows); 3.175 has NO bull_nose row and is the small size of the other families.
- `VBit`: 6.35 and 12.7 mm. chamfer_vbit 60° rows (the angle the tool uses) carry 6.35 and 12.7 (Whiteside 1540 / 1550).
- `TaperedBallNose`: 3.175 and 6 mm. tapered_ball_nose wood rows carry 3.175 (4 rows) and 6.0 (4 rows) as the tip diameter.
- V-bit flutes: 2. The LUT V-bit rows are mixed (13 single-flute, 12 two-flute); the single-flute rows are 15°-45° and 120° cutters, and the 60° rows are two-flute.

Wood rows in `EMBEDDED_LUT`, read by the instrument:

| LUT tool family | wood-row diameters (mm: rows) | flute counts (flutes: rows) |
|---|---|---|
| BallNose | 0.7940: 2, 1.0000: 2, 1.5000: 1, 1.5875: 1, 12.7000: 2, 3.1750: 10, 4.7625: 1, 6.0000: 4, 6.3500: 8, 9.5250: 1 | 2: 27, 3: 4, 4: 1 |
| BullNose | 6.0000: 3 | 2: 3 |
| ChamferVbit | 12.0000: 3, 12.7000: 3, 6.0000: 4, 6.3500: 11, 9.5250: 1, none: 3 | 1: 13, 2: 12 |
| FacingBit | 22.0000: 4, 25.0000: 2, 25.4000: 1 | 2: 6, 4: 1 |
| FlatEnd | 0.7937: 2, 1.5000: 2, 1.5875: 2, 12.0000: 3, 12.7000: 26, 15.8750: 6, 19.0500: 8, 2.3813: 2, 3.0000: 2, 3.1750: 33, 4.7625: 8, 5.0000: 2, 6.0000: 24, 6.3500: 34, 9.5250: 31 | 2: 152, 3: 33 |
| TaperedBallNose | 1.4420: 1, 2.9867: 1, 3.1750: 20, 6.0000: 4, 6.3500: 16 | 2: 26, 3: 16 |

## Cell classes per tool type

The support arm is one axis: `Refused` (Suggest returned an error), `VendorBacked`, `FormulaOnly`, `Refuse` (the `FeedsSupport` arm). `fires-a-diagnostic` is a separate flag: the cell has one or more diagnostic ids from the three pre-simulation doors. A vendor-backed cell can also fire. The `FeedsWarning` and `SuggestWarning` counts are separate columns.

| class | EndMill | BallNose | BullNose | VBit | TaperedBallNose | all |
|---|---|---|---|---|---|---|
| Refused | 72 | 60 | 120 | 136 | 74 | 462 |
| VendorBacked | 88 | 112 | 72 | 32 | 112 | 416 |
| FormulaOnly | 32 | 20 | 0 | 24 | 6 | 82 |
| Refuse | 0 | 0 | 0 | 0 | 0 | 0 |
| fires-a-diagnostic | 120 | 132 | 72 | 56 | 118 | 498 |
| has a FeedsWarning | 120 | 132 | 72 | 56 | 118 | 498 |
| has a SuggestWarning | 112 | 124 | 64 | 56 | 110 | 466 |
| total | 192 | 192 | 192 | 192 | 192 | 960 |

## Cell classes per operation (all tools, diameters and materials)

| operation | Refused | VendorBacked | FormulaOnly | Refuse | fires-a-diagnostic |
|---|---|---|---|---|---|
| Face | 4 | 32 | 4 | 0 | 36 |
| Pocket | 4 | 32 | 4 | 0 | 36 |
| Profile | 14 | 16 | 10 | 0 | 26 |
| Adaptive | 8 | 32 | 0 | 0 | 32 |
| VCarve | 32 | 8 | 0 | 0 | 8 |
| Rest | 4 | 32 | 4 | 0 | 36 |
| Inlay | 32 | 8 | 0 | 0 | 8 |
| Zigzag | 4 | 32 | 4 | 0 | 36 |
| Trace | 18 | 8 | 14 | 0 | 22 |
| Drill | 40 | 0 | 0 | 0 | 0 |
| Chamfer | 32 | 8 | 0 | 0 | 8 |
| DropCutter | 16 | 24 | 0 | 0 | 24 |
| Adaptive3d | 8 | 32 | 0 | 0 | 32 |
| Waterline | 20 | 16 | 4 | 0 | 20 |
| Pencil | 34 | 0 | 6 | 0 | 6 |
| Scallop | 24 | 16 | 0 | 0 | 16 |
| UnifiedFinish | 24 | 16 | 0 | 0 | 16 |
| SteepShallow | 20 | 16 | 4 | 0 | 20 |
| RampFinish | 16 | 16 | 8 | 0 | 24 |
| SpiralFinish | 24 | 16 | 0 | 0 | 16 |
| RadialFinish | 16 | 16 | 8 | 0 | 24 |
| HorizontalFinish | 16 | 16 | 8 | 0 | 24 |
| ProjectCurve | 12 | 24 | 4 | 0 | 28 |
| AlignmentPinDrill | 40 | 0 | 0 | 0 | 0 |

## Pre-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `compat.ball_nose_on_flat_clearing` | 48 |
| `efficiency.very_fine_stepover` | 90 |
| `feeds.chipload_below_floor` | 80 |
| `feeds.feed_clamped` | 95 |
| `feeds.long_tool_derate` | 470 |
| `feeds.no_vendor_rows_for_routed_operation` | 4 |
| `feeds.shank_too_large` | 87 |
| `feeds.vendor_row_publishes_no_chipload` | 18 |

## FeedsWarning variants (cells that carry each)

| id | cells |
|---|---|
| `ChiploadBelowRubbingFloor` | 80 |
| `FeedRateClamped` | 95 |
| `LongToolDerate` | 470 |
| `NoVendorRowsForRoutedOperation` | 4 |
| `ShankTooLarge` | 87 |
| `VendorRowPublishesNoChipload` | 18 |

## SuggestWarning variants (cells that carry each)

| id | cells |
|---|---|
| `AxialDocClampedByEnvelope` | 24 |
| `CutGeometryFieldNotHeld` | 290 |
| `DeflectionBackoffFigureIsAFloor` | 20 |
| `FeedRescaledToFinalGeometry` | 24 |
| `FinishEnvelopeAdvisory` | 148 |
| `PlungeClampedToFeed` | 26 |
| `ProjectCurveDepthInfeasible` | 8 |
| `RoughingDepthClampedToRigidity` | 168 |
| `StrategyRewrote` | 32 |

## Simulation subset (`matrix_2026-09-23_sim.csv`)

- 2D: Pocket, Profile, Adaptive on a 40 mm square polygon, stock 44 × 44 × 18 below z = 0; end_mill and bull_nose at 6 mm; the four materials.
- 3D: a dome height field (top z = 0, flat base z = -8, 46 mm footprint) in the same stock; heights pinned to top 0 and bottom -8 on all 3D cells (`bottom_z: Auto` collapses a waterline band). Waterline, DropCutter, Adaptive3d with end_mill; Scallop and DropCutter with ball_nose and tapered_ball_nose; 6 mm; softwood and hardwood.
- Simulation: resolution 1.0, metrics on, auto resolution off, other fields from `SimulationOptions::default()`. That default has `adaptive_feed_modulation: true`, so the post-simulation verdicts read the modulated feed, as the GUI default does.
- Cells run: 38; errors: 0; skipped on the 150 s budget: 0; wall-clock of the subset: 162.4 s.

## Post-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `efficiency.very_fine_stepover` | 6 |
| `feeds.chipload_below_floor` | 2 |
| `feeds.feed_clamped` | 18 |
| `feeds.long_tool_derate` | 38 |
| `feeds.shank_too_large` | 4 |
| `load.chipload.within` | 38 |
| `load.deflection.within` | 38 |
| `load.depth.reported` | 12 |
| `load.depth.within` | 26 |
| `load.power.within` | 38 |

## Doors that could not answer

- `lut_evidence_grade`, `lut_row_kind`, `lut_source_id` are not on `FeedsResult`. The instrument joins `matched_lut_row.observation_id` to `EMBEDDED_LUT.observations`. A cell whose id is absent from the LUT records `n/a`.
- A refused cell has no recipe, so its recipe, row and diagnostic columns are empty; its cap columns come from the registry default of the operation.
