# Feeds matrix 2026-09-23: summary (FM1 instrument)

The test `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs` writes this file. It records what the product doors answer. It adds no arithmetic and no verdict.

Run command:

```text
scripts/cargo_lane.sh test -p rs_cam_core -q --test feeds_matrix_instrument_fm1 -- --ignored --nocapture
```

## Walk

- Machine: `MachineProfile::default()` = `Generic Wood Router`. The repository holds no measured rigidity or power profile. The presets `shapeoko_vfd` and `shapeoko_makita` and the kinematics preset `shapeoko_xxl_ricky_tuned` exist; the instrument does not walk them.
- `SpindleStrategy::default()`, `SuggestContext::default()`, stock context top 0, bottom -18, height 18, padding 2.
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
| BallNose | 0.7940: 2, 1.0000: 2, 1.5875: 3, 12.7000: 5, 15.8750: 3, 19.0500: 3, 3.1750: 10, 4.7625: 1, 6.0000: 4, 6.3500: 8, 9.5250: 4 | 2: 41, 3: 4 |
| BullNose | 12.7000: 3, 6.0000: 1, 6.3500: 3 | 2: 7 |
| ChamferVbit | 12.0000: 3, 12.7000: 3, 19.0500: 2, 25.4000: 4, 4.7625: 2, 6.0000: 4, 6.3500: 3, 9.5250: 4, none: 20 | 1: 17, 2: 28 |
| FacingBit | 22.0000: 4, 25.0000: 2, 25.4000: 1 | 2: 6, 4: 1 |
| FlatEnd | 0.7937: 2, 1.5000: 4, 1.5875: 2, 12.0000: 5, 12.7000: 30, 15.8750: 6, 19.0500: 6, 2.3813: 2, 3.0000: 4, 3.1750: 33, 4.7625: 8, 5.0000: 4, 6.0000: 24, 6.3500: 34, 9.5250: 35 | 2: 164, 3: 35 |
| TaperedBallNose | 0.5000: 3, 0.7937: 6, 1.0000: 6, 1.4420: 1, 1.5000: 6, 1.5875: 9, 2.0000: 3, 2.9867: 1, 3.0000: 3, 3.1750: 17, 4.0000: 3, 4.7625: 3, 6.0000: 4, 6.3500: 7 | 2: 50, 3: 13, 4: 9 |

## Cell classes per tool type

The support arm is one axis: `Refused` (Suggest returned an error), `VendorBacked`, `Extrapolated` (a G1 size claim), `FamilyTransferred` (a G3 family claim), `DrillTransferred` (the G6 drill claim), `FormulaOnly`, `Refuse` (the `FeedsSupport` arm). `fires-a-diagnostic` is a separate flag: the cell has one or more diagnostic ids from the three pre-simulation doors. A vendor-backed cell can also fire. The `FeedsWarning` and `SuggestWarning` counts are separate columns.

| class | EndMill | BallNose | BullNose | VBit | TaperedBallNose | all |
|---|---|---|---|---|---|---|
| Refused | 56 | 94 | 84 | 152 | 40 | 426 |
| VendorBacked | 72 | 54 | 33 | 20 | 25 | 204 |
| Extrapolated | 16 | 24 | 18 | 10 | 25 | 93 |
| FamilyTransferred | 0 | 0 | 57 | 0 | 102 | 159 |
| DrillTransferred | 16 | 0 | 0 | 0 | 0 | 16 |
| FormulaOnly | 32 | 20 | 0 | 10 | 0 | 62 |
| Refuse | 0 | 0 | 0 | 0 | 0 | 0 |
| fires-a-diagnostic | 136 | 98 | 108 | 40 | 152 | 534 |
| has a FeedsWarning | 136 | 98 | 108 | 40 | 152 | 534 |
| has a SuggestWarning | 136 | 98 | 108 | 40 | 152 | 534 |
| total | 192 | 192 | 192 | 192 | 192 | 960 |

## Cell classes per operation (all tools, diameters and materials)

| operation | Refused | VendorBacked | Extrapolated | FamilyTransferred | DrillTransferred | FormulaOnly | Refuse | fires-a-diagnostic |
|---|---|---|---|---|---|---|---|---|
| Face | 8 | 20 | 10 | 0 | 0 | 2 | 0 | 32 |
| Pocket | 8 | 20 | 10 | 0 | 0 | 2 | 0 | 32 |
| Profile | 10 | 5 | 5 | 14 | 0 | 6 | 0 | 30 |
| Adaptive | 10 | 16 | 3 | 11 | 0 | 0 | 0 | 30 |
| VCarve | 34 | 4 | 2 | 0 | 0 | 0 | 0 | 6 |
| Rest | 8 | 20 | 10 | 0 | 0 | 2 | 0 | 32 |
| Inlay | 34 | 4 | 2 | 0 | 0 | 0 | 0 | 6 |
| Zigzag | 8 | 20 | 10 | 0 | 0 | 2 | 0 | 32 |
| Trace | 8 | 4 | 2 | 14 | 0 | 12 | 0 | 32 |
| Drill | 32 | 0 | 0 | 0 | 8 | 0 | 0 | 8 |
| Chamfer | 34 | 4 | 2 | 0 | 0 | 0 | 0 | 6 |
| DropCutter | 12 | 15 | 1 | 12 | 0 | 0 | 0 | 28 |
| Adaptive3d | 10 | 20 | 10 | 0 | 0 | 0 | 0 | 30 |
| Waterline | 12 | 5 | 5 | 14 | 0 | 4 | 0 | 28 |
| Pencil | 28 | 0 | 0 | 8 | 0 | 4 | 0 | 12 |
| Scallop | 28 | 2 | 2 | 8 | 0 | 0 | 0 | 12 |
| UnifiedFinish | 28 | 2 | 2 | 8 | 0 | 0 | 0 | 12 |
| SteepShallow | 12 | 5 | 5 | 14 | 0 | 4 | 0 | 28 |
| RampFinish | 12 | 7 | 1 | 12 | 0 | 8 | 0 | 28 |
| SpiralFinish | 28 | 2 | 2 | 8 | 0 | 0 | 0 | 12 |
| RadialFinish | 12 | 7 | 1 | 12 | 0 | 8 | 0 | 28 |
| HorizontalFinish | 12 | 7 | 1 | 12 | 0 | 8 | 0 | 28 |
| ProjectCurve | 6 | 15 | 7 | 12 | 0 | 0 | 0 | 34 |
| AlignmentPinDrill | 32 | 0 | 0 | 0 | 8 | 0 | 0 | 8 |

## Pre-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `compat.ball_nose_on_flat_clearing` | 42 |
| `efficiency.very_fine_stepover` | 96 |
| `feeds.aggressiveness_engagement` | 246 |
| `feeds.feed_clamped` | 30 |
| `feeds.long_tool_derate` | 509 |
| `feeds.rpm_lowered_for_ceiling` | 184 |
| `feeds.shank_too_large` | 101 |

## FeedsWarning variants (cells that carry each)

| id | cells |
|---|---|
| `FeedRateClamped` | 30 |
| `LongToolDerate` | 509 |
| `RpmLoweredForFeedCeiling` | 184 |
| `ShankTooLarge` | 101 |

## SuggestWarning variants (cells that carry each)

| id | cells |
|---|---|
| `AggressivenessNotApplied` | 288 |
| `AxialDocClampedByEnvelope` | 22 |
| `CutGeometryFieldNotHeld` | 346 |
| `DeflectionBackoffFigureIsAFloor` | 10 |
| `DeflectionBackoffUnmodeled` | 37 |
| `EngagementReducedForAggressiveness` | 246 |
| `FeedRescaledToFinalGeometry` | 60 |
| `FinishEnvelopeAdvisory` | 164 |
| `PlungeClampedToFeed` | 13 |
| `ProjectCurveDepthInfeasible` | 6 |
| `RampFeed` | 518 |
| `RoughingDepthClampedToRigidity` | 157 |
| `RpmLoweredForFeedCeiling` | 184 |
| `StrategyRewrote` | 30 |

## Simulation subset (`matrix_2026-09-23_sim.csv`)

- 2D: Pocket, Profile, Adaptive on a 40 mm square polygon, stock 44 × 44 × 18 below z = 0; end_mill and bull_nose at 6 mm; the four materials.
- 3D: a dome height field (top z = 0, flat base z = -8, 46 mm footprint) in the same stock; heights pinned to top 0 and bottom -8 on all 3D cells (`bottom_z: Auto` collapses a waterline band). Waterline, DropCutter, Adaptive3d with end_mill; Scallop and DropCutter with ball_nose and tapered_ball_nose; 6 mm; softwood and hardwood.
- Simulation: resolution 1.0, metrics on, auto resolution off, other fields from `SimulationOptions::default()`. That default has `adaptive_feed_modulation: true`, so the post-simulation verdicts read the modulated feed, as the GUI default does.
- Cells run: 38; errors: 0; skipped on the 150 s budget: 0; wall-clock of the subset: 163.8 s.

## Post-simulation diagnostic ids (cells that fire each id)

| id | cells |
|---|---|
| `efficiency.very_fine_stepover` | 6 |
| `feeds.aggressiveness_engagement` | 28 |
| `feeds.feed_clamped` | 5 |
| `feeds.long_tool_derate` | 38 |
| `feeds.rpm_lowered_for_ceiling` | 27 |
| `feeds.shank_too_large` | 4 |
| `load.chipload.unmodeled` | 25 |
| `load.chipload.within` | 13 |
| `load.deflection.unmodeled` | 8 |
| `load.deflection.within` | 30 |
| `load.depth.reported` | 12 |
| `load.depth.within` | 26 |
| `load.power.unmodeled` | 8 |
| `load.power.within` | 30 |

## Doors that could not answer

- `lut_evidence_grade`, `lut_row_kind`, `lut_source_id` are not on `FeedsResult`. The instrument joins `matched_lut_row.observation_id` to `EMBEDDED_LUT.observations`. A cell whose id is absent from the LUT records `n/a`.
- A refused cell has no recipe, so its recipe, row and diagnostic columns are empty; its cap columns come from the registry default of the operation.
- `force_line` is the form id of the material's force line, or `refused:<variant>` (ruling B6). `chip_regime` is the chip regime at the shipped point from `force_at_operating_point`; it is empty on a refused cell and where the door gives no point.
