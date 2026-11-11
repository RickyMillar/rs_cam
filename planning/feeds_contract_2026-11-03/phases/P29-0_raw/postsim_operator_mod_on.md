# postsim nz_wanaka_21_operator.toml

targets [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17]; generated {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17}; cell 0.2 mm; modulation true; budget 35.32 N; feed filter 0.95; total 239 s

## idx 0 (id 0) Face 'Face back to T1'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(6488.0); steady feed 6488; rpm Some(18000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 317; capped moves 0; samples 100522; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(AllSamplesAirCutOrRapid)
- verdict Power: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict Deflection: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict DepthOfCut: Within peak 0.0000 bound 4.0948 population Some((0, 100522)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 0.0 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 1 (id 1) 3D Adaptive 'Back bulk rough 18 mm'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 18 Z 2; op feed Some(8475.0); steady feed 8475; rpm Some(18000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 14628; capped moves 181; samples 120331; removing (not air) 52693; effective feed differs from sample feed on 1262
- M1: feed filter (sample feed < 0.95 x steady) drops 47881 of 52693 removing samples (90.9 %); of the steady-span samples 41172 of 45027 (91.4 %); by the effective feed 48053 of 52693 (91.2 %)
- chip, steady-span samples with a chip: with filter n 3855 over 31 under 0 median 0.1532 max 0.1791; without filter n 45027 over 31 under 0 median 0.1053 max 0.1791
- M5: depth population 84417; peak depth Some((10.0, 123156)); samples with force at the gate cut > budget: 38181; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1791 bound 0.1791 population Some((3855, 4812)) reason None
- verdict Power: Within peak 1.1250 bound 1.1250 population Some((45027, 120331)) reason None
- verdict Deflection: Within peak 0.0074 bound 0.2000 population Some((45027, 120331)) reason None
- verdict DepthOfCut: Within peak 10.0000 bound 36.0000 population Some((84417, 120331)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 138.5 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 45027 | 96.6 | 0 | 10.000 / 9.000 | 15000.000 / 8741.016 | 1.125 / 0.626 | 0 | 138.507 / 118.535 | 38181 | 89.750 / 36.878 | 7.366 / 6.487 | 0 | 0.179 / 0.179 | 31 | 0 |
| entry: EntryHelix | 1863 | 4.1 | 0 | 1.969 / 1.944 | 11642.618 / 6448.623 | 0.362 / 0.357 | 0 | 29.649 / 29.283 | 0 | 21.327 / 21.064 | 1.946 / 1.923 | 0 | 0.179 / 0.179 | 0 | 0 |
| entry: EntryRamp | 5508 | 10.1 | 0 | 1.900 / 1.750 | 15000.000 / 12926.003 | 0.323 / 0.223 | 0 | 27.844 / 25.528 | 0 | 19.024 / 13.146 | 1.833 / 1.686 | 0 | 0.179 / 0.179 | 30 | 0 |
| entry: Linking | 109 | 0.7 | 109 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 26 | 0.1 | 0 | 10.000 / 10.000 | 9231.478 / 6739.151 | 0.850 / 0.748 | 0 | 136.770 / 132.106 | 6 | 50.103 / 44.115 | 7.274 / 7.026 | 0 | 0.179 / 0.179 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 160 | 0.4 | 0 | 3.498 / 3.198 | 7900.845 / 7113.495 | 0.080 / 0.073 | 0 | 32.125 / 29.680 | 0 | 4.745 / 4.338 | 2.032 / 1.891 | 0 | 0.179 / 0.165 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 31 | 0.1 | 0 | 10.000 / 10.000 | 6529.643 / 6529.643 | 1.052 / 0.957 | 0 | 136.120 / 123.114 | 18 | 61.997 / 56.420 | 7.239 / 6.738 | 0 | 0.179 / 0.179 | 0 | 0 |
| feed_cap: capped_split_3d | 25 | 0.1 | 0 | 9.000 / 9.000 | 6926.136 / 6925.601 | 0.893 / 0.615 | 0 | 126.425 / 117.883 | 19 | 89.750 / 36.274 | 6.919 / 6.452 | 0 | 0.179 / 0.179 | 0 | 0 |
| feed_cap: planned_turn | 733 | 2.8 | 0 | 10.000 / 10.000 | 5624.762 / 5251.518 | 1.088 / 0.650 | 0 | 118.094 / 96.216 | 384 | 64.128 / 38.306 | 6.360 / 5.151 | 0 | 0.156 / 0.123 | 0 | 0 |
| feed_cap: stock_read_3d | 132 | 0.4 | 0 | 10.000 / 10.000 | 7569.782 / 6944.069 | 1.125 / 1.091 | 0 | 138.507 / 132.453 | 100 | 84.258 / 66.315 | 7.366 / 7.249 | 0 | 0.179 / 0.179 | 0 | 0 |

## idx 2 (id 2) 3D Adaptive 'Back rest 6 mm (cavity, hollow, lakes, pockets)'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 6 Z 2; op feed Some(5279.0); steady feed 5279; rpm Some(18000)
- band min - max 0.1270; deflection limit 200 um (NoLeaveField); force line true
- moves 20044; capped moves 3898; samples 183722; removing (not air) 69034; effective feed differs from sample feed on 7216
- M1: feed filter (sample feed < 0.95 x steady) drops 30143 of 69034 removing samples (43.7 %); of the steady-span samples 20113 of 57551 (34.9 %); by the effective feed 32134 of 69034 (46.5 %)
- chip, steady-span samples with a chip: with filter n 37438 over 4 under 0 median 0.1056 max 0.1270; without filter n 57551 over 4 under 0 median 0.0914 max 0.1270
- M5: depth population 105318; peak depth Some((20.700000762939453, 352870)); samples with force at the gate cut > budget: 3797; samples over the Factor cap: 63
- verdict Chipload: Within peak 0.1270 bound 0.1270 population Some((37438, 38891)) reason None
- verdict Power: Within peak 0.4846 bound 1.1250 population Some((57551, 183722)) reason None
- verdict Deflection: Within peak 0.0210 bound 0.2000 population Some((57551, 183722)) reason None
- verdict DepthOfCut: Exceeds peak 20.7000 bound 12.0000 population Some((105318, 183722)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 84.2 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 57551 | 187.0 | 0 | 20.700 / 6.000 | 15000.000 / 7323.412 | 0.485 / 0.080 | 0 | 84.178 / 37.650 | 3797 | 85.697 / 14.217 | 20.965 / 13.819 | 0 | 0.127 / 0.127 | 4 | 0 |
| entry: EntryApproach | 467 | 3.0 | 467 | 0.020 / 0.020 | 2285.000 / 2285.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 10533 | 34.8 | 0 | 14.514 / 1.701 | 12734.986 / 5715.000 | 0.675 / 0.067 | 0 | 104.831 / 18.062 | 36 | 119.296 / 11.861 | 28.155 / 7.440 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: Linking | 308 | 1.9 | 308 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 140 | 0.4 | 0 | 6.000 / 6.000 | 12601.987 / 10620.202 | 0.111 / 0.043 | 0 | 50.456 / 25.365 | 2 | 19.662 / 7.653 | 18.263 / 10.236 | 0 | 0.127 / 0.127 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 35 | 0.1 | 0 | 3.000 / 3.000 | 4572.000 / 4572.000 | 0.142 / 0.130 | 0 | 35.279 / 32.456 | 0 | 25.048 / 22.962 | 14.009 / 12.983 | 0 | 0.127 / 0.127 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 1335 | 4.8 | 0 | 17.100 / 6.000 | 4885.927 / 3711.818 | 0.457 / 0.121 | 0 | 67.670 / 45.961 | 149 | 80.746 / 21.473 | 19.996 / 16.547 | 0 | 0.123 / 0.098 | 0 | 0 |
| feed_cap: capped_split_3d | 490 | 2.8 | 0 | 4.950 / 4.314 | 3745.181 / 3002.343 | 0.174 / 0.127 | 0 | 38.951 / 34.213 | 10 | 30.834 / 22.382 | 14.576 / 13.058 | 0 | 0.084 / 0.070 | 0 | 0 |
| feed_cap: departure_window | 1277 | 8.2 | 0 | 12.000 / 6.000 | 4881.874 / 4257.060 | 0.271 / 0.094 | 0 | 54.226 / 35.691 | 67 | 47.878 / 16.596 | 17.778 / 12.979 | 0 | 0.113 / 0.094 | 0 | 0 |
| feed_cap: planned_turn | 1585 | 12.2 | 0 | 20.700 / 6.000 | 5278.745 / 2706.392 | 0.485 / 0.155 | 0 | 75.828 / 38.439 | 98 | 85.697 / 27.391 | 19.625 / 13.914 | 0 | 0.127 / 0.064 | 0 | 0 |
| feed_cap: stock_read_3d | 696 | 2.1 | 0 | 20.700 / 6.000 | 5230.792 / 5031.071 | 0.418 / 0.163 | 0 | 84.178 / 50.028 | 265 | 73.908 / 28.764 | 20.515 / 18.153 | 0 | 0.127 / 0.124 | 0 | 0 |

## idx 3 (id 3) Pin Drill 'Pin drill, flip pins'

- family/role Drill/Roughing; rule None; tool D 6 Z 2; op feed Some(1000.0); steady feed 1000; rpm Some(18000)
- band min - max 0.0635; deflection limit 200 um (NoLeaveField); force line true
- moves 118; capped moves 0; samples 0; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict Power: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict Deflection: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict DepthOfCut: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict DrillChipWelding: Within peak 1.0000 bound 4.5000 population Some((5, 5)) reason None
- verdict DrillPeckAdequacy: Within peak 1.0000 bound 5.0000 population Some((5, 5)) reason None
- verdict DrillPlungeFeed: Within peak 166.6667 bound 50.0000 population Some((5, 5)) reason None

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 4 (id 4) Drill 'Drill light holes 6 mm end mill'

- family/role Drill/Roughing; rule None; tool D 6 Z 2; op feed Some(1000.0); steady feed 1000; rpm Some(18000)
- band min - max 0.0635; deflection limit 200 um (NoLeaveField); force line true
- moves 463; capped moves 0; samples 0; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict Power: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict Deflection: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict DepthOfCut: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotApplicableForOp("drill cycle — no continuous engagement"))
- verdict DrillChipWelding: Within peak 1.0000 bound 4.5000 population Some((42, 42)) reason None
- verdict DrillPeckAdequacy: Within peak 1.0000 bound 5.0000 population Some((42, 42)) reason None
- verdict DrillPlungeFeed: Within peak 166.6667 bound 50.0000 population Some((42, 42)) reason None

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 5 (id 11) Project Curve 'River channel 6 mm, pass 7 of 7'

- family/role Trace/Finish; rule None; tool D 6 Z 2; op feed Some(3165.0); steady feed 3165; rpm Some(10610)
- band min 0.1243 max 0.1741; deflection limit 200 um (FinishTarget); force line true
- moves 17261; capped moves 0; samples 63510; removing (not air) 25358; effective feed differs from sample feed on 4466
- M1: feed filter (sample feed < 0.95 x steady) drops 3588 of 25358 removing samples (14.1 %); of the steady-span samples 905 of 22675 (4.0 %); by the effective feed 4354 of 25358 (17.2 %)
- chip, steady-span samples with a chip: with filter n 21770 over 0 under 78 median 0.1741 max 0.1741; without filter n 22675 over 0 under 966 median 0.1741 max 0.1741
- M5: depth population 27174; peak depth Some((14.332464218139648, 464447)); samples with force at the gate cut > budget: 20973; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1741 bound 0.1741 population Some((21770, 21770)) reason None
- verdict Power: Within peak 0.4765 bound 0.6631 population Some((22675, 63510)) reason None
- verdict Deflection: Within peak 0.0366 bound 0.2000 population Some((22675, 63510)) reason None
- verdict DepthOfCut: Within peak 14.3325 bound - population Some((27174, 63510)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 127.6 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 22675 | 58.2 | 0 | 14.332 / 5.713 | 3693.512 / 3693.512 | 0.476 / 0.182 | 0 | 127.578 / 76.905 | 20973 | 142.948 / 54.908 | 36.602 / 28.121 | 0 | 0.174 / 0.174 | 0 | 966 |
| entry: EntryPlunge | 2683 | 30.3 | 2683 | 0.020 / 0.020 | 1347.000 / 1347.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.063 / 0.063 | 0 | 2683 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 6 (id 12) Pocket 'Lake pocket 2 mm, 11 lakes to Z 5.88'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1010.0); steady feed 1010; rpm Some(18000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 30111; capped moves 7350; samples 295599; removing (not air) 31602; effective feed differs from sample feed on 132
- M1: feed filter (sample feed < 0.95 x steady) drops 1238 of 31602 removing samples (3.9 %); of the steady-span samples 0 of 30364 (0.0 %); by the effective feed 1238 of 31602 (3.9 %)
- chip, steady-span samples with a chip: with filter n 30364 over 372 under 0 median 0.0550 max 0.0550; without filter n 30364 over 372 under 0 median 0.0550 max 0.0550
- M5: depth population 51333; peak depth Some((6.467726707458496, 483126)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0550 bound 0.0550 population Some((30364, 30364)) reason None
- verdict Power: Within peak 0.0337 bound 1.1250 population Some((30364, 295599)) reason None
- verdict Deflection: Within peak 0.0280 bound 0.2000 population Some((30364, 295599)) reason None
- verdict DepthOfCut: Within peak 6.4677 bound - population Some((51333, 295599)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 23.4 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 30364 | 413.1 | 0 | 6.468 / 0.663 | 3482.060 / 1202.513 | 0.034 / 0.003 | 0 | 23.429 / 5.134 | 0 | 17.902 / 1.804 | 27.983 / 8.443 | 0 | 0.055 / 0.055 | 372 | 0 |
| entry: EntryPlunge | 1238 | 69.6 | 1238 | 0.020 / 0.020 | 240.000 / 240.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap, no FeedCap item (ClearingCut) | 15968 | 225.5 | 0 | 6.468 / 0.669 | 989.594 / 989.594 | 0.034 / 0.003 | 0 | 23.429 / 5.134 | 0 | 17.902 / 1.818 | 27.983 / 8.442 | 0 | 0.055 / 0.055 | 0 | 0 |

## idx 7 (id 13) Pocket 'Lake pocket 2 mm, 1 lake to Z 3.99'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1258.0); steady feed 1258; rpm Some(18000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 194; capped moves 0; samples 3665; removing (not air) 507; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 432 of 507 removing samples (85.2 %); of the steady-span samples 360 of 435 (82.8 %); by the effective feed 432 of 507 (85.2 %)
- chip, steady-span samples with a chip: with filter n 75 over 0 under 0 median 0.0535 max 0.0550; without filter n 435 over 0 under 0 median 0.0550 max 0.0550
- M5: depth population 616; peak depth Some((5.612386703491211, 763783)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0550 bound 0.0550 population Some((75, 75)) reason None
- verdict Power: Within peak 0.0293 bound 1.1250 population Some((435, 3665)) reason None
- verdict Deflection: Within peak 0.0286 bound 0.2000 population Some((435, 3665)) reason None
- verdict DepthOfCut: Within peak 5.6124 bound - population Some((616, 3665)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 23.4 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 435 | 5.5 | 0 | 5.612 / 5.612 | 1382.840 / 1382.840 | 0.029 / 0.008 | 0 | 23.440 / 10.220 | 0 | 15.546 / 4.488 | 28.595 / 15.755 | 0 | 0.055 / 0.055 | 0 | 0 |
| entry: EntryPlunge | 72 | 4.2 | 72 | 0.020 / 0.020 | 240.000 / 240.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 8 (id 14) Pocket 'ANTENNA_D5.00 pocket 2 mm'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1258.0); steady feed 1258; rpm Some(18000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 640; capped moves 0; samples 2772; removing (not air) 1210; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 1209 of 1210 removing samples (99.9 %); of the steady-span samples 900 of 900 (100.0 %); by the effective feed 1209 of 1210 (99.9 %)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 900 over 0 under 0 median 0.0550 max 0.0550
- M5: depth population 1020; peak depth Some((0.5, 767538)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0000 bound 0.0550 population Some((0, 1)) reason None
- verdict Power: Within peak 0.0026 bound 1.1250 population Some((900, 2772)) reason None
- verdict Deflection: Within peak 0.0065 bound 0.2000 population Some((900, 2772)) reason None
- verdict DepthOfCut: Within peak 0.5000 bound - population Some((1020, 2772)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 3.9 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 900 | 12.7 | 0 | 0.500 / 0.500 | 1006.996 / 990.181 | 0.003 / 0.003 | 0 | 3.888 / 3.888 | 0 | 1.375 / 1.375 | 6.464 / 6.464 | 0 | 0.055 / 0.055 | 0 | 0 |
| entry: EntryRamp | 310 | 4.6 | 0 | 0.500 / 0.427 | 1650.302 / 990.181 | 0.002 / 0.002 | 0 | 3.764 / 3.325 | 0 | 1.174 / 1.134 | 6.258 / 5.555 | 0 | 0.055 / 0.055 | 1 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 9 (id 15) Project Curve 'V rivers and lakes'

- family/role Trace/Finish; rule None; tool D 3.175 Z 1; op feed Some(2286.0); steady feed 2286; rpm Some(18000)
- band min 0.0762 max 0.1778; deflection limit 200 um (FinishTarget); force line true
- moves 22369; capped moves 0; samples 71014; removing (not air) 26079; effective feed differs from sample feed on 5118
- M1: feed filter (sample feed < 0.95 x steady) drops 3500 of 26079 removing samples (13.4 %); of the steady-span samples 50 of 22629 (0.2 %); by the effective feed 3981 of 26079 (15.3 %)
- chip, steady-span samples with a chip: with filter n 22579 over 0 under 0 median 0.1778 max 0.1778; without filter n 22629 over 0 under 8 median 0.1778 max 0.1778
- M5: depth population 22696; peak depth Some((5.588724136352539, 783144)); samples with force at the gate cut > budget: 9; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1778 bound 0.1778 population Some((22579, 22579)) reason None
- verdict Power: Within peak 0.0342 bound 1.1250 population Some((22629, 71014)) reason None
- verdict Deflection: Within peak 0.0673 bound 0.2000 population Some((22629, 71014)) reason None
- verdict DepthOfCut: Within peak 5.5887 bound - population Some((22696, 71014)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 41.6 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 22629 | 96.4 | 0 | 5.589 / 1.450 | 3200.400 / 3200.400 | 0.034 / 0.003 | 0 | 41.624 / 11.261 | 9 | 29.941 / 8.101 | 67.298 / 32.404 | 0 | 0.178 / 0.178 | 0 | 8 |
| entry: EntryPlunge | 3450 | 138.6 | 3450 | 0.020 / 0.020 | 381.000 / 381.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.021 / 0.021 | 0 | 3450 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 10 (id 16) Trace 'Day strip slit V60'

- family/role Trace/Finish; rule None; tool D 6.35 Z 2; op feed Some(2286.0); steady feed 2286; rpm Some(18000)
- band min - max 0.0635; deflection limit 200 um (FinishTarget); force line true
- moves 19; capped moves 0; samples 6540; removing (not air) 3528; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 36 of 3528 removing samples (1.0 %); of the steady-span samples 0 of 3492 (0.0 %); by the effective feed 36 of 3528 (1.0 %)
- chip, steady-span samples with a chip: with filter n 3492 over 0 under 0 median 0.0635 max 0.0635; without filter n 3492 over 0 under 0 median 0.0635 max 0.0635
- M5: depth population 3492; peak depth Some((4.603333234786987, 841274)); samples with force at the gate cut > budget: 1144; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0635 bound 0.0635 population Some((3492, 3492)) reason None
- verdict Power: Within peak 0.0885 bound 1.1250 population Some((3492, 6540)) reason None
- verdict Deflection: Within peak 0.0060 bound 0.2000 population Some((3492, 6540)) reason None
- verdict DepthOfCut: Within peak 4.6033 bound - population Some((3492, 6540)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 36.0 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 3492 | 22.9 | 0 | 4.603 / 4.603 | 2286.000 / 2286.000 | 0.088 / 0.088 | 0 | 36.031 / 36.031 | 1144 | 32.042 / 29.813 | 6.027 / 6.027 | 0 | 0.064 / 0.064 | 0 | 0 |
| entry: EntryPlunge | 36 | 0.7 | 36 | 0.020 / 0.020 | 762.000 / 762.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.021 / 0.021 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 11 (id 17) Pocket 'Face clamp pads 6 mm'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(6488.0); steady feed 6488; rpm Some(18000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 64; capped moves 16; samples 7073; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(AllSamplesAirCutOrRapid)
- verdict Power: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict Deflection: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict DepthOfCut: Within peak 0.0000 bound 4.0948 population Some((0, 7073)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 0.0 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 12 (id 18) Face 'Face front to T'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(6488.0); steady feed 6488; rpm Some(18000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 371; capped moves 0; samples 108504; removing (not air) 91199; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 7 of 91199 removing samples (0.0 %); of the steady-span samples 1 of 90004 (0.0 %); by the effective feed 7 of 91199 (0.0 %)
- chip, steady-span samples with a chip: with filter n 90003 over 0 under 0 median 0.1791 max 0.1791; without filter n 90004 over 0 under 0 median 0.1791 max 0.1791
- M5: depth population 90049; peak depth Some((0.5, 856703)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1791 bound 0.1791 population Some((90003, 91192)) reason None
- verdict Power: Within peak 0.0906 bound 1.1250 population Some((90004, 108504)) reason None
- verdict Deflection: Within peak 0.0005 bound 0.2000 population Some((90004, 108504)) reason None
- verdict DepthOfCut: Within peak 0.5000 bound 2.3690 population Some((90049, 108504)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 7.5 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 90004 | 207.3 | 0 | 0.500 / 0.500 | 7755.602 / 6506.720 | 0.091 / 0.043 | 0 | 7.538 / 7.521 | 0 | 5.341 / 2.510 | 0.512 / 0.511 | 0 | 0.179 / 0.179 | 0 | 0 |
| entry: EntryPlunge | 6 | 0.1 | 6 | 0.020 / 0.020 | 1081.000 / 1081.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 1189 | 2.7 | 0 | 0.500 / 0.467 | 6430.102 / 6430.102 | 0.064 / 0.039 | 0 | 7.523 / 6.801 | 0 | 3.779 / 2.328 | 0.511 / 0.462 | 0 | 0.179 / 0.177 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 13 (id 19) Trace 'Engrave day labels V60'

- family/role Trace/Finish; rule None; tool D 6.35 Z 2; op feed Some(2286.0); steady feed 2286; rpm Some(18000)
- band min - max 0.0635; deflection limit 200 um (FinishTarget); force line true
- moves 314; capped moves 0; samples 3834; removing (not air) 1099; effective feed differs from sample feed on 2
- M1: feed filter (sample feed < 0.95 x steady) drops 58 of 1099 removing samples (5.3 %); of the steady-span samples 0 of 1041 (0.0 %); by the effective feed 58 of 1099 (5.3 %)
- chip, steady-span samples with a chip: with filter n 1041 over 0 under 0 median 0.0635 max 0.0635; without filter n 1041 over 0 under 0 median 0.0635 max 0.0635
- M5: depth population 1041; peak depth Some((0.6000003814697266, 963320)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0635 bound 0.0635 population Some((1041, 1041)) reason None
- verdict Power: Within peak 0.0015 bound 1.1250 population Some((1041, 3834)) reason None
- verdict Deflection: Within peak 0.0010 bound 0.2000 population Some((1041, 3834)) reason None
- verdict DepthOfCut: Within peak 0.6000 bound - population Some((1041, 3834)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 4.7 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 1041 | 6.5 | 0 | 0.600 / 0.600 | 2286.000 / 2286.000 | 0.001 / 0.001 | 0 | 4.696 / 4.679 | 0 | 3.567 / 3.215 | 0.999 / 0.996 | 0 | 0.064 / 0.064 | 0 | 0 |
| entry: EntryPlunge | 58 | 1.1 | 58 | 0.020 / 0.020 | 762.000 / 762.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.021 / 0.021 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 14 (id 20) 3D Adaptive 'Front bulk rough 18 mm'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 18 Z 2; op feed Some(8060.0); steady feed 8060; rpm Some(18000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 22517; capped moves 1840; samples 233013; removing (not air) 96720; effective feed differs from sample feed on 8582
- M1: feed filter (sample feed < 0.95 x steady) drops 45218 of 96720 removing samples (46.8 %); of the steady-span samples 41094 of 90116 (45.6 %); by the effective feed 46999 of 96720 (48.6 %)
- chip, steady-span samples with a chip: with filter n 49022 over 136 under 0 median 0.1345 max 0.1791; without filter n 90116 over 136 under 0 median 0.1388 max 0.1791
- M5: depth population 168611; peak depth Some((5.732474327087402, 1199270)); samples with force at the gate cut > budget: 27596; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1791 bound 0.1791 population Some((49022, 51502)) reason None
- verdict Power: Within peak 0.7694 bound 1.1250 population Some((90116, 233013)) reason None
- verdict Deflection: Within peak 0.0039 bound 0.2000 population Some((90116, 233013)) reason None
- verdict DepthOfCut: Within peak 5.7325 bound 36.0000 population Some((168611, 233013)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 62.5 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 90116 | 193.0 | 0 | 5.732 / 3.666 | 15000.000 / 10305.175 | 0.769 / 0.351 | 0 | 62.494 / 50.079 | 27596 | 45.354 / 21.095 | 3.889 / 3.159 | 0 | 0.179 / 0.179 | 136 | 0 |
| entry: EntryPlunge | 12 | 0.1 | 12 | 0.020 / 0.020 | 2162.000 / 2162.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 2885 | 5.8 | 0 | 1.993 / 1.400 | 15000.000 / 14076.188 | 0.329 / 0.173 | 0 | 29.239 / 20.049 | 0 | 19.411 / 10.222 | 1.920 / 1.335 | 0 | 0.179 / 0.179 | 4 | 0 |
| entry: Linking | 95 | 0.6 | 95 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 524 | 0.9 | 9 | 3.666 / 3.666 | 15000.000 / 14194.174 | 0.253 / 0.151 | 0 | 53.028 / 47.591 | 235 | 14.907 / 8.872 | 3.341 / 2.998 | 0 | 0.179 / 0.179 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 3088 | 6.3 | 0 | 4.166 / 4.166 | 15000.000 / 10904.004 | 0.660 / 0.353 | 0 | 62.494 / 60.884 | 1212 | 38.914 / 20.803 | 3.889 / 3.789 | 0 | 0.179 / 0.179 | 2 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 189 | 0.7 | 0 | 3.666 / 2.587 | 5571.735 / 5036.075 | 0.293 / 0.205 | 0 | 38.644 / 26.524 | 5 | 17.278 / 12.736 | 2.457 / 1.719 | 0 | 0.149 / 0.132 | 0 | 0 |
| feed_cap: capped_split_3d | 238 | 0.9 | 0 | 3.983 / 3.160 | 7052.345 / 6448.623 | 0.505 / 0.395 | 0 | 51.997 / 42.972 | 29 | 32.337 / 25.142 | 3.292 / 2.761 | 0 | 0.179 / 0.179 | 0 | 0 |
| feed_cap: departure_window | 1679 | 9.1 | 0 | 4.166 / 3.906 | 7673.040 / 6448.623 | 0.498 / 0.204 | 0 | 46.190 / 34.411 | 74 | 29.380 / 12.087 | 2.910 / 2.169 | 0 | 0.179 / 0.154 | 0 | 0 |
| feed_cap: planned_turn | 1363 | 6.0 | 0 | 3.666 / 3.666 | 7944.143 / 7372.132 | 0.673 / 0.303 | 0 | 55.056 / 34.956 | 65 | 39.686 / 17.907 | 3.469 / 2.232 | 0 | 0.179 / 0.177 | 0 | 0 |
| feed_cap: stock_read_3d | 1817 | 4.3 | 0 | 4.166 / 3.666 | 8052.132 / 7886.741 | 0.755 / 0.515 | 0 | 62.494 / 54.014 | 564 | 44.502 / 31.915 | 3.889 / 3.387 | 0 | 0.179 / 0.179 | 0 | 0 |

## idx 15 (id 21) 3D Adaptive 'Front 3D rough'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 6 Z 2; op feed Some(5429.0); steady feed 5429; rpm Some(18000)
- band min - max 0.1270; deflection limit 200 um (NoLeaveField); force line true
- moves 40122; capped moves 8595; samples 273291; removing (not air) 75694; effective feed differs from sample feed on 69159
- M1: feed filter (sample feed < 0.95 x steady) drops 19362 of 75694 removing samples (25.6 %); of the steady-span samples 11904 of 67395 (17.7 %); by the effective feed 62177 of 75694 (82.1 %)
- chip, steady-span samples with a chip: with filter n 55491 over 0 under 0 median 0.1113 max 0.1270; without filter n 67395 over 0 under 0 median 0.1012 max 0.1270
- M5: depth population 128953; peak depth Some((6.413309097290039, 1425648)); samples with force at the gate cut > budget: 157; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1270 bound 0.1270 population Some((55491, 56332)) reason None
- verdict Power: Within peak 0.3007 bound 1.1250 population Some((67395, 273291)) reason None
- verdict Deflection: Within peak 0.0249 bound 0.2000 population Some((67395, 273291)) reason None
- verdict DepthOfCut: Within peak 6.4133 bound 12.0000 population Some((128953, 273291)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 69.7 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 67395 | 206.1 | 0 | 6.413 / 1.902 | 12027.336 / 6188.357 | 0.301 / 0.074 | 0 | 69.688 / 21.659 | 157 | 53.168 / 13.583 | 24.890 / 8.868 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: EntryApproach | 58 | 0.3 | 58 | 0.020 / 0.020 | 2285.000 / 2285.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 7104 | 21.9 | 0 | 4.192 / 1.152 | 12734.986 / 5414.896 | 0.126 / 0.046 | 0 | 42.400 / 13.063 | 3 | 22.339 / 8.428 | 16.243 / 5.460 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: Linking | 233 | 1.4 | 233 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 904 | 2.1 | 54 | 0.498 / 0.457 | 9451.091 / 6158.854 | 0.022 / 0.017 | 0 | 5.773 / 4.669 | 0 | 3.961 / 2.944 | 2.464 / 1.998 | 0 | 0.127 / 0.127 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 2756 | 11.3 | 0 | 2.738 / 1.042 | 4784.853 / 3655.084 | 0.106 / 0.021 | 0 | 29.013 / 9.365 | 0 | 18.694 / 4.267 | 11.610 / 3.942 | 0 | 0.120 / 0.096 | 0 | 0 |
| feed_cap: capped_split_3d | 20 | 0.1 | 0 | 2.593 / 2.506 | 4572.000 / 4572.000 | 0.054 / 0.053 | 0 | 24.753 / 23.825 | 0 | 12.453 / 11.850 | 9.948 / 9.600 | 0 | 0.127 / 0.127 | 0 | 0 |
| feed_cap: departure_window | 2859 | 20.8 | 0 | 3.358 / 1.866 | 5355.195 / 4164.337 | 0.104 / 0.030 | 0 | 23.638 / 12.645 | 0 | 18.329 / 5.575 | 9.287 / 5.193 | 0 | 0.127 / 0.092 | 0 | 0 |
| feed_cap: planned_turn | 1956 | 14.4 | 0 | 3.579 / 1.310 | 5391.885 / 3227.716 | 0.086 / 0.024 | 0 | 31.553 / 9.312 | 0 | 15.272 / 4.704 | 12.331 / 3.875 | 0 | 0.116 / 0.076 | 0 | 0 |
| feed_cap: stock_read_3d | 13 | 0.0 | 0 | 2.180 / 2.180 | 5335.614 / 5335.614 | 0.031 / 0.031 | 0 | 19.601 / 19.601 | 0 | 5.448 / 5.448 | 8.045 / 8.045 | 0 | 0.127 / 0.127 | 0 | 0 |

## idx 17 (id 24) Profile 'Outline profile with tabs'

- family/role Contour/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 6 Z 2; op feed Some(3165.0); steady feed 3165; rpm Some(10610)
- band min 0.1243 max 0.1741; deflection limit 200 um (NoLeaveField); force line true
- moves 1750; capped moves 0; samples 145289; removing (not air) 123467; effective feed differs from sample feed on 123143
- M1: feed filter (sample feed < 0.95 x steady) drops 359 of 123467 removing samples (0.3 %); of the steady-span samples 35 of 120546 (0.0 %); by the effective feed 359 of 123467 (0.3 %)
- chip, steady-span samples with a chip: with filter n 120511 over 0 under 0 median 0.1741 max 0.1741; without filter n 120511 over 0 under 0 median 0.1741 max 0.1741
- M5: depth population 121634; peak depth Some((1.75, 1478857)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1741 bound 0.1741 population Some((120511, 123108)) reason None
- verdict Power: Within peak 0.0602 bound 0.6631 population Some((120511, 145289)) reason None
- verdict Deflection: Within peak 0.0106 bound 0.2000 population Some((120511, 145289)) reason None
- verdict DepthOfCut: Within peak 1.7500 bound 2.4231 population Some((121634, 145289)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 25.7 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 120546 | 570.8 | 35 | 1.750 / 1.250 | 3693.512 / 3693.512 | 0.060 / 0.043 | 0 | 25.660 / 18.400 | 0 | 18.070 / 12.907 | 10.567 / 7.686 | 0 | 0.174 / 0.174 | 0 | 0 |
| entry: EntryPlunge | 51 | 0.5 | 51 | 0.020 / 0.020 | 1582.000 / 1582.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 2597 | 12.1 | 0 | 1.250 / 1.195 | 3693.512 / 3693.512 | 0.043 / 0.041 | 0 | 18.400 / 17.596 | 0 | 12.907 / 12.339 | 7.686 / 7.361 | 0 | 0.174 / 0.174 | 0 | 0 |
| phantom: in_transit, no span / EntryPlunge | 273 | 2.5 | 273 | 0.020 / 0.020 | 1582.000 / 1582.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

