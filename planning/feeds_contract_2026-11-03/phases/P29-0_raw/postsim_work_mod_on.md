# postsim nz_wanaka_21_6mm_rough_work.toml

targets [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 17]; generated {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 17}; cell 0.2 mm; modulation true; budget 35.32 N; feed filter 0.95; total 470 s

## idx 0 (id 0) Face 'Face back to T1'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(8651.0); steady feed 8651; rpm Some(24000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 317; capped moves 0; samples 100522; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(AllSamplesAirCutOrRapid)
- verdict Power: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict Deflection: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict DepthOfCut: Within peak 0.0000 bound 4.6429 population Some((0, 100522)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 0.0 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 1 (id 1) 3D Adaptive 'Back bulk rough 18 mm'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 6 Z 2; op feed Some(7371.0); steady feed 7371; rpm Some(24000)
- band min - max 0.1270; deflection limit 200 um (NoLeaveField); force line true
- moves 85581; capped moves 7482; samples 1018198; removing (not air) 717064; effective feed differs from sample feed on 22155
- M1: feed filter (sample feed < 0.95 x steady) drops 38367 of 717064 removing samples (5.4 %); of the steady-span samples 20059 of 696121 (2.9 %); by the effective feed 46395 of 717064 (6.5 %)
- chip, steady-span samples with a chip: with filter n 676062 over 721 under 0 median 0.1174 max 0.1270; without filter n 696121 over 721 under 0 median 0.1173 max 0.1270
- M5: depth population 876293; peak depth Some((12.0, 1012779)); samples with force at the gate cut > budget: 59809; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1270 bound 0.1270 population Some((676062, 678697)) reason None
- verdict Power: Within peak 0.3878 bound 1.5000 population Some((696121, 1018198)) reason None
- verdict Deflection: Within peak 0.0239 bound 0.2000 population Some((696121, 1018198)) reason None
- verdict DepthOfCut: Within peak 12.0000 bound 12.0000 population Some((876293, 1018198)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 66.0 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 696121 | 1313.5 | 0 | 12.000 / 6.000 | 15000.000 / 8509.730 | 0.388 / 0.105 | 0 | 66.019 / 36.987 | 59809 | 51.435 / 13.945 | 23.897 / 13.418 | 0 | 0.127 / 0.127 | 721 | 0 |
| entry: EntryApproach | 169 | 1.0 | 169 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryHelix | 15031 | 20.4 | 83 | 5.556 / 1.389 | 11507.204 / 6096.000 | 0.114 / 0.051 | 0 | 31.638 / 14.643 | 0 | 15.134 / 8.097 | 11.983 / 5.989 | 0 | 0.127 / 0.127 | 8 | 0 |
| entry: EntryRamp | 48 | 0.1 | 0 | 1.625 / 1.625 | 6096.000 / 6096.000 | 0.073 / 0.073 | 0 | 19.415 / 19.415 | 0 | 9.666 / 9.666 | 8.024 / 8.024 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: Linking | 596 | 3.7 | 596 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 495 | 0.9 | 0 | 6.000 / 6.000 | 15000.000 / 13539.941 | 0.157 / 0.126 | 0 | 51.801 / 43.631 | 39 | 20.839 / 16.678 | 18.750 / 15.793 | 0 | 0.127 / 0.127 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 4604 | 18.0 | 0 | 13.594 / 6.000 | 8966.430 / 8966.430 | 0.664 / 0.137 | 0 | 90.275 / 37.024 | 286 | 88.048 / 18.145 | 24.868 / 13.483 | 0 | 0.127 / 0.127 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 1007 | 2.8 | 0 | 6.000 / 6.000 | 5434.035 / 4443.144 | 0.202 / 0.150 | 0 | 50.408 / 43.902 | 481 | 26.736 / 19.949 | 18.246 / 15.891 | 0 | 0.110 / 0.085 | 0 | 0 |
| feed_cap: capped_split_3d | 535 | 2.6 | 0 | 6.000 / 5.953 | 7203.256 / 4838.841 | 0.271 / 0.170 | 0 | 43.716 / 29.003 | 21 | 35.963 / 22.500 | 15.824 / 11.190 | 0 | 0.114 / 0.071 | 0 | 0 |
| feed_cap: planned_turn | 2114 | 11.6 | 0 | 9.000 / 6.000 | 5457.531 / 3704.732 | 0.223 / 0.208 | 0 | 49.719 / 42.309 | 565 | 29.601 / 27.529 | 17.131 / 15.315 | 0 | 0.083 / 0.066 | 0 | 0 |
| feed_cap: stock_read_3d | 5476 | 9.9 | 0 | 12.000 / 6.000 | 7369.279 / 7331.125 | 0.384 / 0.124 | 0 | 66.019 / 40.859 | 2961 | 50.926 / 16.412 | 23.897 / 14.815 | 0 | 0.127 / 0.122 | 0 | 0 |

## idx 2 (id 2) 3D Adaptive 'Back rest 6 mm (cavity, hollow, lakes, pockets)'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 6 Z 2; op feed Some(6841.0); steady feed 6841; rpm Some(24000)
- band min - max 0.1270; deflection limit 200 um (NoLeaveField); force line true
- moves 8381; capped moves 1366; samples 79840; removing (not air) 30459; effective feed differs from sample feed on 28994
- M1: feed filter (sample feed < 0.95 x steady) drops 4442 of 30459 removing samples (14.6 %); of the steady-span samples 2854 of 28706 (9.9 %); by the effective feed 10368 of 30459 (34.0 %)
- chip, steady-span samples with a chip: with filter n 25852 over 2 under 0 median 0.1003 max 0.1270; without filter n 28706 over 2 under 0 median 0.0936 max 0.1270
- M5: depth population 53548; peak depth Some((16.949999809265137, 1144456)); samples with force at the gate cut > budget: 92; samples over the Factor cap: 4
- verdict Chipload: Within peak 0.1270 bound 0.1270 population Some((25852, 26017)) reason None
- verdict Power: Within peak 0.2961 bound 1.5000 population Some((28706, 79840)) reason None
- verdict Deflection: Within peak 0.0167 bound 0.2000 population Some((28706, 79840)) reason None
- verdict DepthOfCut: Exceeds peak 16.9500 bound 12.0000 population Some((53548, 79840)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 46.4 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 28706 | 67.3 | 0 | 16.950 / 2.450 | 15000.000 / 8867.082 | 0.296 / 0.054 | 0 | 46.420 / 25.605 | 92 | 39.269 / 7.215 | 16.708 / 10.325 | 0 | 0.127 / 0.127 | 2 | 0 |
| entry: EntryApproach | 132 | 0.7 | 132 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 1339 | 4.0 | 0 | 14.500 / 1.817 | 8728.590 / 7809.185 | 0.639 / 0.085 | 0 | 80.152 / 17.256 | 14 | 84.749 / 11.343 | 22.663 / 7.169 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: Linking | 107 | 0.7 | 107 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 140 | 0.3 | 0 | 2.450 / 2.450 | 13979.277 / 11691.850 | 0.068 / 0.047 | 0 | 27.052 / 25.365 | 0 | 9.005 / 6.221 | 10.918 / 10.236 | 0 | 0.127 / 0.127 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 35 | 0.1 | 0 | 3.000 / 3.000 | 4905.800 / 4905.800 | 0.169 / 0.155 | 0 | 31.045 / 28.557 | 0 | 22.436 / 20.567 | 12.328 / 11.424 | 0 | 0.102 / 0.102 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 557 | 1.5 | 0 | 4.641 / 2.450 | 5138.248 / 4601.665 | 0.164 / 0.068 | 0 | 35.863 / 22.670 | 1 | 21.736 / 9.024 | 13.820 / 9.149 | 0 | 0.100 / 0.092 | 0 | 0 |
| feed_cap: capped_split_3d | 8 | 0.0 | 0 | 2.844 / 2.844 | 2132.496 / 2132.496 | 0.029 / 0.029 | 0 | 16.326 / 16.326 | 0 | 3.860 / 3.860 | 6.513 / 6.513 | 0 | 0.031 / 0.031 | 0 | 0 |
| feed_cap: departure_window | 499 | 2.4 | 0 | 6.000 / 5.000 | 5685.539 / 5273.296 | 0.201 / 0.085 | 0 | 40.461 / 32.545 | 13 | 26.653 / 11.239 | 14.646 / 11.923 | 0 | 0.103 / 0.094 | 0 | 0 |
| feed_cap: planned_turn | 607 | 3.6 | 0 | 6.200 / 2.450 | 3507.185 / 3009.686 | 0.184 / 0.092 | 0 | 46.420 / 18.810 | 1 | 24.381 / 12.139 | 16.694 / 7.590 | 0 | 0.073 / 0.058 | 0 | 0 |
| feed_cap: stock_read_3d | 18 | 0.0 | 0 | 6.200 / 6.200 | 6670.758 / 6670.758 | 0.191 / 0.191 | 0 | 41.814 / 41.814 | 2 | 25.336 / 25.336 | 15.648 / 15.648 | 0 | 0.115 / 0.115 | 0 | 0 |

## idx 3 (id 3) Pin Drill 'Pin drill, flip pins'

- family/role Drill/Roughing; rule None; tool D 6 Z 2; op feed Some(2499.0); steady feed 2499; rpm Some(19685)
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
- verdict DrillPlungeFeed: Within peak 416.5000 bound 613.8333 population Some((5, 5)) reason None

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 4 (id 4) Drill 'Drill light holes 6 mm end mill'

- family/role Drill/Roughing; rule None; tool D 6 Z 2; op feed Some(2499.0); steady feed 2499; rpm Some(19685)
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
- verdict DrillPlungeFeed: Within peak 416.5000 bound 613.8333 population Some((42, 42)) reason None

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 5 (id 11) Project Curve 'River channel 6 mm, pass 7 of 7'

- family/role Trace/Finish; rule None; tool D 6 Z 2; op feed Some(7161.0); steady feed 7161; rpm Some(24000)
- band min 0.1243 max 0.1741; deflection limit 200 um (FinishTarget); force line true
- moves 17261; capped moves 0; samples 63510; removing (not air) 25235; effective feed differs from sample feed on 22568
- M1: feed filter (sample feed < 0.95 x steady) drops 2989 of 25235 removing samples (11.8 %); of the steady-span samples 443 of 22689 (2.0 %); by the effective feed 8824 of 25235 (35.0 %)
- chip, steady-span samples with a chip: with filter n 22246 over 0 under 4361 median 0.1671 max 0.1741; without filter n 22689 over 0 under 4749 median 0.1658 max 0.1741
- M5: depth population 27185; peak depth Some((8.322935104370117, 1258454)); samples with force at the gate cut > budget: 19875; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1741 bound 0.1741 population Some((22246, 22246)) reason None
- verdict Power: Within peak 0.5632 bound 1.5000 population Some((22689, 63510)) reason None
- verdict Deflection: Within peak 0.0328 bound 0.2000 population Some((22689, 63510)) reason None
- verdict DepthOfCut: Within peak 8.3229 bound - population Some((27185, 63510)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 94.6 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 22689 | 29.6 | 0 | 8.323 / 5.610 | 8354.786 / 8354.786 | 0.563 / 0.395 | 0 | 94.601 / 74.249 | 19875 | 74.693 / 52.684 | 32.758 / 27.315 | 0 | 0.174 / 0.174 | 0 | 4749 |
| entry: EntryPlunge | 2546 | 15.5 | 2546 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.052 / 0.052 | 0 | 2546 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 6 (id 12) Pocket 'Lake pocket 2 mm, 11 lakes to Z 5.88'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1347.0); steady feed 1347; rpm Some(24000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 30111; capped moves 7350; samples 295599; removing (not air) 41913; effective feed differs from sample feed on 21333
- M1: feed filter (sample feed < 0.95 x steady) drops 1329 of 41913 removing samples (3.2 %); of the steady-span samples 0 of 40584 (0.0 %); by the effective feed 1350 of 41913 (3.2 %)
- chip, steady-span samples with a chip: with filter n 40584 over 636 under 0 median 0.0550 max 0.0550; without filter n 40584 over 636 under 0 median 0.0550 max 0.0550
- M5: depth population 60381; peak depth Some((0.6626930236816406, 1306728)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0550 bound 0.0550 population Some((40584, 40584)) reason None
- verdict Power: Within peak 0.0047 bound 1.5000 population Some((40584, 295599)) reason None
- verdict Deflection: Within peak 0.0084 bound 0.2000 population Some((40584, 295599)) reason None
- verdict DepthOfCut: Within peak 0.6627 bound - population Some((60381, 295599)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 5.1 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 40584 | 419.1 | 0 | 0.663 / 0.663 | 4642.747 / 1612.798 | 0.005 / 0.004 | 0 | 5.134 / 5.134 | 0 | 1.859 / 1.777 | 8.443 / 8.443 | 0 | 0.055 / 0.055 | 636 | 0 |
| entry: EntryPlunge | 1329 | 74.4 | 1329 | 0.020 / 0.020 | 240.000 / 240.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap, no FeedCap item (ClearingCut) | 19251 | 203.3 | 0 | 0.663 / 0.663 | 1319.785 / 1319.785 | 0.005 / 0.005 | 0 | 5.133 / 5.133 | 0 | 1.859 / 1.802 | 8.441 / 8.441 | 0 | 0.055 / 0.055 | 0 | 0 |

## idx 7 (id 13) Pocket 'Lake pocket 2 mm, 1 lake to Z 3.99'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1320.0); steady feed 1320; rpm Some(24000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 194; capped moves 0; samples 3665; removing (not air) 482; effective feed differs from sample feed on 432
- M1: feed filter (sample feed < 0.95 x steady) drops 50 of 482 removing samples (10.4 %); of the steady-span samples 0 of 432 (0.0 %); by the effective feed 50 of 482 (10.4 %)
- chip, steady-span samples with a chip: with filter n 432 over 0 under 0 median 0.0550 max 0.0550; without filter n 432 over 0 under 0 median 0.0550 max 0.0550
- M5: depth population 616; peak depth Some((0.4923868179321289, 1557969)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0550 bound 0.0550 population Some((432, 432)) reason None
- verdict Power: Within peak 0.0034 bound 1.5000 population Some((432, 3665)) reason None
- verdict Deflection: Within peak 0.0064 bound 0.2000 population Some((432, 3665)) reason None
- verdict DepthOfCut: Within peak 0.4924 bound - population Some((616, 3665)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 3.8 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 432 | 4.4 | 0 | 0.492 / 0.492 | 1843.787 / 1843.787 | 0.003 / 0.003 | 0 | 3.829 / 3.829 | 0 | 1.364 / 1.364 | 6.370 / 6.370 | 0 | 0.055 / 0.055 | 0 | 0 |
| entry: EntryPlunge | 50 | 2.8 | 50 | 0.020 / 0.020 | 240.000 / 240.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 8 (id 14) Pocket 'ANTENNA_D5.00 pocket 2 mm'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 2 Z 1; op feed Some(1320.0); steady feed 1320; rpm Some(24000)
- band min - max 0.0550; deflection limit 200 um (NoLeaveField); force line true
- moves 640; capped moves 0; samples 2772; removing (not air) 1150; effective feed differs from sample feed on 1150
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 1150 removing samples (0.0 %); of the steady-span samples 0 of 840 (0.0 %); by the effective feed 0 of 1150 (0.0 %)
- chip, steady-span samples with a chip: with filter n 840 over 0 under 0 median 0.0550 max 0.0550; without filter n 840 over 0 under 0 median 0.0550 max 0.0550
- M5: depth population 990; peak depth Some((0.5, 1561523)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0550 bound 0.0550 population Some((840, 1150)) reason None
- verdict Power: Within peak 0.0035 bound 1.5000 population Some((840, 2772)) reason None
- verdict Deflection: Within peak 0.0065 bound 0.2000 population Some((840, 2772)) reason None
- verdict DepthOfCut: Within peak 0.5000 bound - population Some((990, 2772)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 3.9 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 840 | 9.1 | 0 | 0.500 / 0.500 | 1342.661 / 1320.243 | 0.003 / 0.003 | 0 | 3.888 / 3.888 | 0 | 1.375 / 1.375 | 6.464 / 6.464 | 0 | 0.055 / 0.055 | 0 | 0 |
| entry: EntryRamp | 310 | 3.5 | 0 | 0.500 / 0.423 | 2200.403 / 1320.242 | 0.003 / 0.003 | 0 | 3.529 / 3.293 | 0 | 1.163 / 1.134 | 5.867 / 5.504 | 0 | 0.055 / 0.055 | 10 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 9 (id 15) Project Curve 'V rivers and lakes'

- family/role Trace/Finish; rule None; tool D 3.175 Z 1; op feed Some(3048.0); steady feed 3048; rpm Some(24000)
- band min 0.0762 max 0.1778; deflection limit 200 um (FinishTarget); force line true
- moves 22369; capped moves 0; samples 71014; removing (not air) 25989; effective feed differs from sample feed on 22631
- M1: feed filter (sample feed < 0.95 x steady) drops 3365 of 25989 removing samples (12.9 %); of the steady-span samples 9 of 22633 (0.0 %); by the effective feed 4972 of 25989 (19.1 %)
- chip, steady-span samples with a chip: with filter n 22624 over 13801 under 38 median 0.1778 max 0.1778; without filter n 22633 over 13801 under 40 median 0.1778 max 0.1778
- M5: depth population 22696; peak depth Some((2.415816307067871, 1607129)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1778 bound 0.1778 population Some((22624, 22624)) reason None
- verdict Power: Within peak 0.0113 bound 1.5000 population Some((22633, 71014)) reason None
- verdict Deflection: Within peak 0.0420 bound 0.2000 population Some((22633, 71014)) reason None
- verdict DepthOfCut: Within peak 2.4158 bound - population Some((22696, 71014)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 18.8 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 22633 | 99.0 | 0 | 2.416 / 1.425 | 4267.200 / 4267.200 | 0.011 / 0.004 | 0 | 18.838 / 11.037 | 0 | 13.551 / 7.941 | 41.953 / 32.113 | 0 | 0.178 / 0.178 | 13801 | 40 |
| entry: EntryPlunge | 3356 | 134.7 | 3356 | 0.020 / 0.020 | 381.000 / 381.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.016 / 0.016 | 0 | 3356 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 10 (id 16) Trace 'Day strip slit V60'

- family/role Trace/Finish; rule None; tool D 6.35 Z 2; op feed Some(3048.0); steady feed 3048; rpm Some(24000)
- band min - max 0.0635; deflection limit 200 um (FinishTarget); force line true
- moves 19; capped moves 0; samples 6540; removing (not air) 3528; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 36 of 3528 removing samples (1.0 %); of the steady-span samples 0 of 3492 (0.0 %); by the effective feed 36 of 3528 (1.0 %)
- chip, steady-span samples with a chip: with filter n 3492 over 0 under 0 median 0.0635 max 0.0635; without filter n 3492 over 0 under 0 median 0.0635 max 0.0635
- M5: depth population 3492; peak depth Some((4.603333234786987, 1635259)); samples with force at the gate cut > budget: 1144; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0635 bound 0.0635 population Some((3492, 3492)) reason None
- verdict Power: Within peak 0.1179 bound 1.5000 population Some((3492, 6540)) reason None
- verdict Deflection: Within peak 0.0060 bound 0.2000 population Some((3492, 6540)) reason None
- verdict DepthOfCut: Within peak 4.6033 bound - population Some((3492, 6540)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 36.0 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 3492 | 17.2 | 0 | 4.603 / 4.603 | 3048.000 / 3048.000 | 0.118 / 0.118 | 0 | 36.031 / 36.031 | 1144 | 32.042 / 29.813 | 6.027 / 6.027 | 0 | 0.064 / 0.064 | 0 | 0 |
| entry: EntryPlunge | 36 | 0.5 | 36 | 0.020 / 0.020 | 1016.000 / 1016.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.021 / 0.021 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 11 (id 17) Pocket 'Face clamp pads 6 mm'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(8651.0); steady feed 8651; rpm Some(24000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 64; capped moves 16; samples 7073; removing (not air) 0; effective feed differs from sample feed on 0
- M1: feed filter (sample feed < 0.95 x steady) drops 0 of 0 removing samples (-); of the steady-span samples 0 of 0 (-); by the effective feed 0 of 0 (-)
- chip, steady-span samples with a chip: with filter n 0 over 0 under 0 median - max -; without filter n 0 over 0 under 0 median - max -
- M5: depth population 0; peak depth None; samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Unmodeled peak - bound - population None reason Some(AllSamplesAirCutOrRapid)
- verdict Power: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict Deflection: Unmodeled peak - bound - population None reason Some(ArcEngagementNotCaptured)
- verdict DepthOfCut: Within peak 0.0000 bound 4.6429 population Some((0, 7073)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 0.0 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 12 (id 18) Face 'Face front to T'

- family/role Pocket/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 18 Z 2; op feed Some(8651.0); steady feed 8651; rpm Some(24000)
- band min - max 0.1791; deflection limit 200 um (NoLeaveField); force line true
- moves 371; capped moves 0; samples 108504; removing (not air) 91199; effective feed differs from sample feed on 91193
- M1: feed filter (sample feed < 0.95 x steady) drops 6 of 91199 removing samples (0.0 %); of the steady-span samples 0 of 90004 (0.0 %); by the effective feed 1196 of 91199 (1.3 %)
- chip, steady-span samples with a chip: with filter n 90004 over 1824 under 0 median 0.1791 max 0.1791; without filter n 90004 over 1824 under 0 median 0.1791 max 0.1791
- M5: depth population 90049; peak depth Some((0.5, 1650688)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1791 bound 0.1791 population Some((90004, 91193)) reason None
- verdict Power: Within peak 0.1208 bound 1.5000 population Some((90004, 108504)) reason None
- verdict Deflection: Within peak 0.0005 bound 0.2000 population Some((90004, 108504)) reason None
- verdict DepthOfCut: Within peak 0.5000 bound 2.3690 population Some((90049, 108504)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 7.5 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 90004 | 156.0 | 0 | 0.500 / 0.500 | 10340.803 / 8675.627 | 0.121 / 0.057 | 0 | 7.538 / 7.521 | 0 | 5.341 / 2.510 | 0.512 / 0.511 | 0 | 0.179 / 0.179 | 1824 | 0 |
| entry: EntryPlunge | 6 | 0.1 | 6 | 0.020 / 0.020 | 1081.000 / 1081.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 1189 | 2.0 | 0 | 0.500 / 0.467 | 6430.102 / 6430.102 | 0.072 / 0.045 | 0 | 6.216 / 5.622 | 0 | 3.196 / 1.992 | 0.422 / 0.382 | 0 | 0.134 / 0.133 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 13 (id 19) Trace 'Engrave day labels V60'

- family/role Trace/Finish; rule None; tool D 6.35 Z 2; op feed Some(3048.0); steady feed 3048; rpm Some(24000)
- band min - max 0.0635; deflection limit 200 um (FinishTarget); force line true
- moves 314; capped moves 0; samples 3834; removing (not air) 1099; effective feed differs from sample feed on 51
- M1: feed filter (sample feed < 0.95 x steady) drops 58 of 1099 removing samples (5.3 %); of the steady-span samples 0 of 1041 (0.0 %); by the effective feed 100 of 1099 (9.1 %)
- chip, steady-span samples with a chip: with filter n 1041 over 0 under 0 median 0.0635 max 0.0635; without filter n 1041 over 0 under 0 median 0.0635 max 0.0635
- M5: depth population 1041; peak depth Some((0.6000003814697266, 1757305)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.0635 bound 0.0635 population Some((1041, 1041)) reason None
- verdict Power: Within peak 0.0019 bound 1.5000 population Some((1041, 3834)) reason None
- verdict Deflection: Within peak 0.0010 bound 0.2000 population Some((1041, 3834)) reason None
- verdict DepthOfCut: Within peak 0.6000 bound - population Some((1041, 3834)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 4.7 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 1041 | 4.8 | 0 | 0.600 / 0.600 | 3048.000 / 3048.000 | 0.002 / 0.002 | 0 | 4.696 / 4.679 | 0 | 3.567 / 3.215 | 0.999 / 0.996 | 0 | 0.064 / 0.064 | 0 | 0 |
| entry: EntryPlunge | 58 | 0.8 | 58 | 0.020 / 0.020 | 1016.000 / 1016.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | 0.021 / 0.021 | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

## idx 14 (id 20) 3D Adaptive 'Front bulk rough 18 mm'

- family/role Adaptive/Roughing; rule Some(Factor(2.0)); tool D 6 Z 2; op feed Some(7371.0); steady feed 7371; rpm Some(24000)
- band min - max 0.1270; deflection limit 200 um (NoLeaveField); force line true
- moves 112999; capped moves 16751; samples 1172267; removing (not air) 694212; effective feed differs from sample feed on 671821
- M1: feed filter (sample feed < 0.95 x steady) drops 60954 of 694212 removing samples (8.8 %); of the steady-span samples 55441 of 681336 (8.1 %); by the effective feed 231067 of 694212 (33.3 %)
- chip, steady-span samples with a chip: with filter n 625895 over 736 under 0 median 0.1134 max 0.1270; without filter n 681336 over 736 under 0 median 0.1111 max 0.1270
- M5: depth population 961681; peak depth Some((3.354959487915039, 2369554)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1270 bound 0.1270 population Some((625895, 633258)) reason None
- verdict Power: Within peak 0.1964 bound 1.5000 population Some((681336, 1172267)) reason None
- verdict Deflection: Within peak 0.0140 bound 0.2000 population Some((681336, 1172267)) reason None
- verdict DepthOfCut: Within peak 3.3550 bound 12.0000 population Some((961681, 1172267)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 35.3 N, budget None, rule no force limit on this op family

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 681336 | 1404.4 | 0 | 3.355 / 3.000 | 15000.000 / 9233.291 | 0.196 / 0.055 | 0 | 35.279 / 27.427 | 0 | 26.053 / 7.314 | 14.009 / 10.904 | 0 | 0.127 / 0.127 | 736 | 0 |
| entry: EntryApproach | 155 | 0.8 | 155 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 4646 | 11.3 | 0 | 1.953 / 1.366 | 15000.000 / 8178.642 | 0.109 / 0.069 | 0 | 20.886 / 14.611 | 0 | 14.432 / 9.211 | 8.601 / 6.100 | 0 | 0.127 / 0.127 | 0 | 0 |
| entry: Linking | 266 | 1.6 | 266 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| phantom: LinkBridge / Linking | 582 | 1.3 | 60 | 3.000 / 3.000 | 13321.730 / 9543.732 | 0.178 / 0.038 | 0 | 32.154 / 24.347 | 0 | 23.651 / 5.082 | 12.768 / 9.680 | 0 | 0.127 / 0.127 | 0 | 0 |
| phantom: WaterlineCleanup / ClearingCut | 7227 | 14.8 | 0 | 3.000 / 2.501 | 13768.221 / 9085.058 | 0.124 / 0.079 | 0 | 31.345 / 28.460 | 0 | 19.949 / 10.429 | 12.548 / 11.469 | 0 | 0.127 / 0.127 | 2 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed_cap: capped_2d | 2661 | 8.5 | 0 | 3.000 / 1.067 | 5876.131 / 4427.047 | 0.141 / 0.033 | 0 | 32.069 / 9.016 | 0 | 18.724 / 4.737 | 12.734 / 3.769 | 0 | 0.122 / 0.087 | 0 | 0 |
| feed_cap: capped_split_3d | 1636 | 4.2 | 0 | 3.000 / 2.777 | 7039.546 / 6096.000 | 0.193 / 0.115 | 0 | 35.279 / 26.562 | 0 | 25.632 / 16.514 | 14.009 / 10.691 | 0 | 0.127 / 0.127 | 0 | 0 |
| feed_cap: departure_window | 2080 | 9.3 | 0 | 3.000 / 3.000 | 7093.104 / 6096.000 | 0.185 / 0.051 | 0 | 34.170 / 19.736 | 0 | 24.507 / 7.014 | 13.609 / 7.857 | 0 | 0.127 / 0.098 | 0 | 0 |
| feed_cap: planned_turn | 4714 | 21.6 | 0 | 3.349 / 3.000 | 7370.128 / 6096.000 | 0.170 / 0.063 | 0 | 34.202 / 21.672 | 0 | 22.574 / 8.560 | 13.582 / 8.658 | 0 | 0.127 / 0.092 | 0 | 0 |
| feed_cap: stock_read_3d | 9640 | 14.6 | 0 | 3.000 / 3.000 | 7369.750 / 7314.893 | 0.196 / 0.089 | 0 | 35.279 / 28.466 | 0 | 25.982 / 13.324 | 14.009 / 11.310 | 0 | 0.127 / 0.127 | 0 | 0 |

## idx 17 (id 24) Profile 'Outline profile with tabs'

- family/role Contour/Roughing; rule Some(ForceBudget(35.32243336966621)); tool D 6 Z 2; op feed Some(7161.0); steady feed 7161; rpm Some(24000)
- band min 0.1243 max 0.1741; deflection limit 200 um (NoLeaveField); force line true
- moves 1750; capped moves 0; samples 145289; removing (not air) 123467; effective feed differs from sample feed on 123143
- M1: feed filter (sample feed < 0.95 x steady) drops 359 of 123467 removing samples (0.3 %); of the steady-span samples 35 of 120546 (0.0 %); by the effective feed 626 of 123467 (0.5 %)
- chip, steady-span samples with a chip: with filter n 120511 over 0 under 18 median 0.1741 max 0.1741; without filter n 120511 over 0 under 18 median 0.1741 max 0.1741
- M5: depth population 121634; peak depth Some((1.75, 2938805)); samples with force at the gate cut > budget: 0; samples over the Factor cap: 0
- verdict Chipload: Within peak 0.1741 bound 0.1741 population Some((120511, 123108)) reason None
- verdict Power: Within peak 0.1362 bound 1.5000 population Some((120511, 145289)) reason None
- verdict Deflection: Within peak 0.0106 bound 0.2000 population Some((120511, 145289)) reason None
- verdict DepthOfCut: Within peak 1.7500 bound 2.4231 population Some((121634, 145289)) reason None
- verdict GantryPush: Unmodeled peak - bound - population None reason Some(NotImplemented("no machine-side thrust rating is published; register T-10"))
- cut_force card: peak 25.7 N, budget Some(35.32243336966621), rule force budget of the machine (the depth-cap rule of this family)

by kind (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| body (steady) | 120546 | 252.4 | 35 | 1.750 / 1.250 | 8354.786 / 8354.786 | 0.136 / 0.097 | 0 | 25.660 / 18.400 | 0 | 18.070 / 12.907 | 10.567 / 7.686 | 0 | 0.174 / 0.174 | 0 | 18 |
| entry: EntryPlunge | 51 | 0.3 | 51 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |
| entry: EntryRamp | 2597 | 5.3 | 0 | 1.250 / 1.195 | 8354.786 / 8354.786 | 0.097 / 0.093 | 0 | 18.400 / 17.596 | 0 | 12.907 / 12.339 | 7.686 / 7.361 | 0 | 0.174 / 0.174 | 0 | 211 |
| phantom: in_transit, no span / EntryPlunge | 273 | 1.6 | 273 | 0.020 / 0.020 | 2500.000 / 2500.000 | - / - | 0 | - / - | 0 | - / - | - / - | 0 | - / - | 0 | 0 |

capped moves, by source (max / p95):

| kind | n | time s | no arc | ap mm | feed | power kW | >avail | peak force N | >budget | mean force N | defl um | >limit | chip mm | >max | <min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

