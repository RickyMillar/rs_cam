# presim nz_wanaka_21_6mm_rough_work.toml

force budget 35.32 N

| idx | id | op | family/role | rule | D | Z | ap | ae | ae note | rpm | feed | fz | mean N | peak N | peak/budget | depth_at_budget | swept ae | peak N at swept | depth_at_budget swept | defl um | power kW (case) / avail |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0 | Face | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 24000 | 8651 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.058 (nominal, ae 8.00) / 1.500 |
| 1 | 1 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 6.000  | 1.200 | stated | 24000 | 7371 | 0.1536 | 15.0 | 39.5 | 1.12 | 3.954 | 1.313 | 42.1 | 3.750 | 14.3 | 0.121 (planner cap, ae 1.31) / 1.500 |
| 2 | 2 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 6.000  | 1.500 | stated | 24000 | 6841 | 0.1425 | 17.2 | 44.7 | 1.27 | 3.670 | 1.638 | 47.6 | 3.512 | 16.2 | 0.138 (planner cap, ae 1.64) / 1.500 |
| 3 | 3 | Pin Drill | Drill/Roughing | None | 6 | 2 | 6.000 (axial value that calc_inputs drops) | - | none | 19685 | 2499 | 0.0635 | - | - | - | - | - | - | - | - | 0.154 (axial plunge, ae 6.00) / 1.230 |
| 4 | 4 | Drill | Drill/Roughing | None | 6 | 2 | 6.000 (axial value that calc_inputs drops) | - | none | 19685 | 2499 | 0.0635 | - | - | - | - | - | - | - | - | 0.154 (axial plunge, ae 6.00) / 1.230 |
| 5 | 11 | Project Curve | Trace/Finish | None | 6 | 2 | 4.000 (axial value that calc_inputs drops) | 6.000 | slot=D | 24000 | 7161 | 0.1492 | 40.6 | 51.2 | 1.45 | 2.697 | 6.000 | - | - | 19.7 | DepthAfterSimulation |
| 6 | 12 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.700  | 0.800 | stated | 24000 | 1347 | 0.0561 | 1.0 | 5.3 | 0.15 | no root below flute | 0.800 | - | - | 8.7 | 0.006 (slot, ae 2.00) / 1.500 |
| 7 | 13 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.500  | 1.000 | stated | 24000 | 1320 | 0.0550 | 0.8 | 3.9 | 0.11 | no root below flute | 1.000 | - | - | 6.4 | 0.004 (slot, ae 2.00) / 1.500 |
| 8 | 14 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.500  | 1.000 | stated | 24000 | 1320 | 0.0550 | 0.8 | 3.9 | 0.11 | no root below flute | 1.000 | - | - | 6.4 | 0.004 (slot, ae 2.00) / 1.500 |
| 9 | 15 | Project Curve | Trace/Finish | None | 3.175 | 1 | 2.418 (axial value that calc_inputs drops) | 3.175 | slot=D | 24000 | 3048 | 0.1270 | 11.3 | 15.4 | 0.44 | 5.541 | 3.175 | - | - | 34.3 | DepthAfterSimulation |
| 10 | 16 | Trace | Trace/Finish | None | 6.35 | 2 | 2.600  | 6.350 | slot=D | 24000 | 3048 | 0.0635 | 18.1 | 20.4 | 0.58 | 4.511 | 6.350 | - | - | 3.6 | 0.038 (slot, ae 6.35) / 1.500 |
| 11 | 17 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 24000 | 8651 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.128 (slot, ae 18.00) / 1.500 |
| 12 | 18 | Face | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 24000 | 8651 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.058 (nominal, ae 8.00) / 1.500 |
| 13 | 19 | Trace | Trace/Finish | None | 6.35 | 2 | 0.600  | 6.350 | slot=D | 24000 | 3048 | 0.0635 | 4.2 | 4.7 | 0.13 | 4.511 | 6.350 | - | - | 1.0 | 0.002 (slot, ae 6.35) / 1.500 |
| 14 | 20 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 3.000  | 1.200 | stated | 24000 | 7371 | 0.1536 | 7.5 | 29.2 | 0.83 | 3.954 | 1.313 | 30.0 | 3.750 | 11.6 | 0.061 (planner cap, ae 1.31) / 1.500 |
| 15 | 21 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 5.532  | 1.382 | stated | 24000 | 7022 | 0.1463 | 15.0 | 42.7 | 1.21 | 3.784 | 1.510 | 45.5 | 3.599 | 15.7 | 0.121 (planner cap, ae 1.51) / 1.500 |
| 16 | 23 | Scallop Finish | Scallop/Finish | None | 2 | 2 | - (axial value that calc_inputs drops) | - | none | 24000 | 3657 | 0.0762 | - | - | - | - | - | - | - | - | NoRadialEngagement |
| 17 | 24 | Profile | Contour/Roughing | Some(ForceBudget(35.32243336966621)) | 6 | 2 | 1.250  | 6.000 | slot=D | 24000 | 7161 | 0.1492 | 12.7 | 16.6 | 0.47 | 2.697 | 6.000 | - | - | 6.9 | 0.096 (slot, ae 6.00) / 1.500 |
