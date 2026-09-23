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
| BallNose | 0.7940: 2, 1.0000: 2, 1.5000: 1, 1.5875: 1, 12.7000: 2, 3.1750: 4, 4.7625: 1, 6.0000: 4, 6.3500: 2, 9.5250: 1 | 2: 15, 3: 4, 4: 1 |
| BullNose | 6.0000: 3 | 2: 3 |
| ChamferVbit | 12.0000: 3, 12.7000: 3, 6.0000: 4, 6.3500: 11, 9.5250: 1, none: 3 | 1: 13, 2: 12 |
| FacingBit | 22.0000: 4, 25.0000: 2, 25.4000: 1 | 2: 6, 4: 1 |
| FlatEnd | 0.7937: 2, 1.5000: 2, 1.5875: 2, 12.0000: 3, 12.7000: 22, 15.8750: 4, 19.0500: 7, 2.3813: 2, 3.0000: 2, 3.1750: 16, 4.7625: 7, 5.0000: 2, 6.0000: 8, 6.3500: 17, 9.5250: 24 | 2: 111, 3: 9 |
| TaperedBallNose | 1.4420: 1, 2.9867: 1, 3.1750: 4, 6.0000: 4 | 2: 10 |

## Cell classes per tool type

The support arm is one axis: `Refused` (Suggest returned an error), `VendorBacked`, `FormulaOnly`, `Refuse` (the `FeedsSupport` arm). `fires-a-diagnostic` is a separate flag: the cell has one or more diagnostic ids from the three pre-simulation doors. A vendor-backed cell can also fire. The `FeedsWarning` and `SuggestWarning` counts are separate columns.

| class | EndMill | BallNose | BullNose | VBit | TaperedBallNose | all |
|---|---|---|---|---|---|---|
| Refused | 24 | 0 | 0 | 24 | 0 | 48 |
| VendorBacked | 88 | 64 | 72 | 40 | 64 | 328 |
| FormulaOnly | 80 | 128 | 120 | 128 | 128 | 584 |
| Refuse | 0 | 0 | 0 | 0 | 0 | 0 |
| fires-a-diagnostic | 93 | 144 | 105 | 130 | 185 | 657 |
| has a FeedsWarning | 64 | 113 | 79 | 121 | 181 | 558 |
| has a SuggestWarning | 160 | 186 | 184 | 168 | 188 | 886 |
| total | 192 | 192 | 192 | 192 | 192 | 960 |

## Cell classes per operation (all tools, diameters and materials)

| operation | Refused | VendorBacked | FormulaOnly | Refuse | fires-a-diagnostic |
|---|---|---|---|---|---|
| Face | 0 | 16 | 24 | 0 | 20 |
| Pocket | 0 | 16 | 24 | 0 | 20 |
| Profile | 0 | 16 | 24 | 0 | 26 |
| Adaptive | 0 | 16 | 24 | 0 | 28 |
| VCarve | 0 | 8 | 32 | 0 | 40 |
| Rest | 0 | 16 | 24 | 0 | 14 |
| Inlay | 0 | 8 | 32 | 0 | 40 |
| Zigzag | 0 | 16 | 24 | 0 | 20 |
| Trace | 0 | 8 | 32 | 0 | 24 |
| Drill | 0 | 0 | 40 | 0 | 24 |
| Chamfer | 0 | 8 | 32 | 0 | 24 |
| DropCutter | 0 | 24 | 16 | 0 | 40 |
| Adaptive3d | 0 | 16 | 24 | 0 | 25 |
| Waterline | 0 | 16 | 24 | 0 | 30 |
| Pencil | 0 | 8 | 32 | 0 | 29 |
| Scallop | 16 | 16 | 8 | 0 | 18 |
| UnifiedFinish | 16 | 16 | 8 | 0 | 18 |
| SteepShallow | 0 | 16 | 24 | 0 | 38 |
| RampFinish | 0 | 16 | 24 | 0 | 29 |
| SpiralFinish | 16 | 16 | 8 | 0 | 24 |
| RadialFinish | 0 | 16 | 24 | 0 | 25 |
| HorizontalFinish | 0 | 16 | 24 | 0 | 40 |
| ProjectCurve | 0 | 24 | 16 | 0 | 37 |
| AlignmentPinDrill | 0 | 0 | 40 | 0 | 24 |

## Pre-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `compat.ball_nose_on_flat_clearing` | 48 |
| `compat.end_mill_on_scallop_pencil` | 8 |
| `efficiency.very_fine_stepover` | 200 |
| `feeds.chipload_clamped_to_floor` | 333 |
| `feeds.drill_feed_clamped_to_envelope` | 32 |
| `feeds.feed_clamped` | 30 |
| `feeds.no_vendor_rows_for_routed_operation` | 16 |
| `feeds.shank_too_large` | 180 |
| `feeds.vendor_row_publishes_no_chipload` | 22 |

## FeedsWarning variants (cells that carry each)

| id | cells |
|---|---|
| `ChiploadClampedToFloor` | 333 |
| `DrillFeedClampedToEnvelope` | 32 |
| `FeedRateClamped` | 30 |
| `NoVendorRowsForRoutedOperation` | 16 |
| `ShankTooLarge` | 180 |
| `VendorRowPublishesNoChipload` | 22 |

## SuggestWarning variants (cells that carry each)

| id | cells |
|---|---|
| `AxialDocClampedByEnvelope` | 16 |
| `CutGeometryFieldNotHeld` | 672 |
| `DeflectionBackoffFigureIsAFloor` | 56 |
| `FeedRescaledToFinalGeometry` | 16 |
| `FinishEnvelopeAdvisory` | 256 |
| `PlungeClampedToFeed` | 85 |
| `ProjectCurveDepthInfeasible` | 13 |
| `RoughingDepthClampedToRigidity` | 204 |
| `StrategyRewrote` | 40 |

## Simulation subset (`matrix_2026-09-23_sim.csv`)

- 2D: Pocket, Profile, Adaptive on a 40 mm square polygon, stock 44 × 44 × 18 below z = 0; end_mill and bull_nose at 6 mm; the four materials.
- 3D: a dome height field (top z = 0, flat base z = -8, 46 mm footprint) in the same stock; heights pinned to top 0 and bottom -8 on all 3D cells (`bottom_z: Auto` collapses a waterline band). Waterline, DropCutter, Adaptive3d with end_mill; Scallop and DropCutter with ball_nose and tapered_ball_nose; 6 mm; softwood and hardwood.
- Simulation: resolution 1.0, metrics on, auto resolution off, other fields from `SimulationOptions::default()`. That default has `adaptive_feed_modulation: true`, so the post-simulation verdicts read the modulated feed, as the GUI default does.
- Cells run: 37; errors: 0; skipped on the 150 s budget: 1; wall-clock of the subset: 157.8 s.

## Post-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `efficiency.very_fine_stepover` | 5 |
| `feeds.chipload_clamped_to_floor` | 6 |
| `feeds.feed_clamped` | 5 |
| `feeds.shank_too_large` | 3 |
| `load.chipload.within` | 37 |
| `load.deflection.within` | 37 |
| `load.power.within` | 37 |

## Doors that could not answer

- `lut_evidence_grade`, `lut_row_kind`, `lut_source_id` are not on `FeedsResult`. The instrument joins `matched_lut_row.observation_id` to `EMBEDDED_LUT.observations`. A cell whose id is absent from the LUT records `n/a`.
- A refused cell has no recipe, so its recipe, row and diagnostic columns are empty; its cap columns come from the registry default of the operation.
