# presim nz_wanaka_21_operator.toml

force budget 35.32 N

| idx | id | op | family/role | rule | D | Z | ap | ae | ae note | rpm | feed | fz | mean N | peak N | peak/budget | depth_at_budget | swept ae | peak N at swept | depth_at_budget swept | defl um | power kW (case) / avail |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0 | Face | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 18000 | 6488 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.043 (nominal, ae 8.00) / 1.125 |
| 1 | 1 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 18 | 2 | 10.000  | 5.500 | stated | 18000 | 8475 | 0.2354 | 43.9 | 145.1 | 4.11 | 2.092 | 5.991 | 149.6 | 2.055 | 7.7 | 0.800 (planner cap, ae 5.99) / 1.125 |
| 2 | 2 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 6.000  | 1.500 | stated | 18000 | 5279 | 0.1466 | 17.4 | 45.3 | 1.28 | 3.593 | 1.638 | 48.3 | 3.439 | 16.4 | 0.105 (planner cap, ae 1.64) / 1.125 |
| 3 | 3 | Pin Drill | Drill/Roughing | None | 6 | 2 | 6.000 (axial value that calc_inputs drops) | - | none | 18000 | 1000 | 0.0278 | - | - | - | - | - | - | - | - | 0.106 (axial plunge, ae 6.00) / 1.125 |
| 4 | 4 | Drill | Drill/Roughing | None | 6 | 2 | 6.000 (axial value that calc_inputs drops) | - | none | 18000 | 1000 | 0.0278 | - | - | - | - | - | - | - | - | 0.106 (axial plunge, ae 6.00) / 1.125 |
| 5 | 11 | Project Curve | Trace/Finish | None | 6 | 2 | 4.000 (axial value that calc_inputs drops) | 6.000 | slot=D | 10610 | 3165 | 0.1492 | 40.6 | 51.2 | 1.45 | 2.697 | 6.000 | - | - | 19.7 | DepthAfterSimulation |
| 6 | 12 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.700  | 0.800 | stated | 18000 | 1010 | 0.0561 | 1.0 | 5.3 | 0.15 | no root below flute | 0.800 | - | - | 8.6 | 0.004 (slot, ae 2.00) / 1.125 |
| 7 | 13 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.500  | 1.000 | stated | 18000 | 1258 | 0.0699 | 0.9 | 4.3 | 0.12 | no root below flute | 1.000 | - | - | 7.1 | 0.003 (slot, ae 2.00) / 1.125 |
| 8 | 14 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 2 | 1 | 0.500  | 1.000 | stated | 18000 | 1258 | 0.0699 | 0.9 | 4.3 | 0.12 | no root below flute | 1.000 | - | - | 7.1 | 0.003 (slot, ae 2.00) / 1.125 |
| 9 | 15 | Project Curve | Trace/Finish | None | 3.175 | 1 | 2.418 (axial value that calc_inputs drops) | 3.175 | slot=D | 18000 | 2286 | 0.1270 | 11.3 | 15.4 | 0.44 | 5.541 | 3.175 | - | - | 34.3 | DepthAfterSimulation |
| 10 | 16 | Trace | Trace/Finish | None | 6.35 | 2 | 2.600  | 6.350 | slot=D | 18000 | 2286 | 0.0635 | 18.1 | 20.4 | 0.58 | 4.511 | 6.350 | - | - | 3.6 | 0.028 (slot, ae 6.35) / 1.125 |
| 11 | 17 | Pocket | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 18000 | 6488 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.096 (slot, ae 18.00) / 1.125 |
| 12 | 18 | Face | Pocket/Roughing | Some(ForceBudget(35.32243336966621)) | 18 | 2 | 0.500  | 8.000 | stated | 18000 | 6488 | 0.1802 | 2.6 | 7.5 | 0.21 | 2.368 | 8.000 | - | - | 0.5 | 0.043 (nominal, ae 8.00) / 1.125 |
| 13 | 19 | Trace | Trace/Finish | None | 6.35 | 2 | 0.600  | 6.350 | slot=D | 18000 | 2286 | 0.0635 | 4.2 | 4.7 | 0.13 | 4.511 | 6.350 | - | - | 1.0 | 0.002 (slot, ae 6.35) / 1.125 |
| 14 | 20 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 18 | 2 | 4.166  | 3.600 | stated | 18000 | 8060 | 0.2239 | 12.6 | 57.7 | 1.63 | 2.455 | 3.940 | 59.0 | 2.402 | 3.6 | 0.230 (planner cap, ae 3.94) / 1.125 |
| 15 | 21 | 3D Adaptive | Adaptive/Roughing | Some(Factor(2.0)) | 6 | 2 | 5.532  | 1.382 | stated | 18000 | 5429 | 0.1508 | 15.3 | 43.3 | 1.23 | 3.698 | 1.510 | 46.2 | 3.517 | 15.9 | 0.092 (planner cap, ae 1.51) / 1.125 |
| 16 | 23 | Scallop Finish | Scallop/Finish | None | 2 | 2 | - (axial value that calc_inputs drops) | - | none | 18000 | 2317 | 0.0644 | - | - | - | - | - | - | - | - | NoRadialEngagement |
| 17 | 24 | Profile | Contour/Roughing | Some(ForceBudget(35.32243336966621)) | 6 | 2 | 1.250  | 6.000 | slot=D | 10610 | 3165 | 0.1492 | 12.7 | 16.6 | 0.47 | 2.697 | 6.000 | - | - | 6.9 | 0.042 (slot, ae 6.00) / 0.663 |
