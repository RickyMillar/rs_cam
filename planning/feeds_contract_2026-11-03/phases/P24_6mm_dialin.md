# P24 dial-in: the two bulk roughs on the 6 mm flat end mill (2026-11-11)

Operator ruling 2026-11-11: "lets use the 6mm! see if you can dial that path in."

Working file: `planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml`.
The operator file `nz_wanaka_21_operator.toml` is unchanged (sha256 `f5149e33…912717` before
and after).

## 1. Summary

- The working file is the operator file with idx 1 and idx 14 on the 6 mm flat end mill, the
  dialled cut, and Suggest applied on all 18 toolpaths at the file's own policy
  (`constant_chip`, ceiling 24000).
- The dial-in rule gives a stepover of 1.2 mm (20 % of the diameter) on the two bulk roughs.
  The roughing chain then takes 4588.8 s of fed time. Variant A took 2113.1 s and variant C
  took 2007.7 s (both at ceiling 18000).
- The simulation ran on 16 of 18 toolpaths. Idx 15 generates no moves and is disabled. Idx 16
  (scallop finish) generated, but the whole-project simulation with idx 16 was killed for
  memory. Idx 16 has no simulation numbers.
- Simulation headline (16 toolpaths): "ERROR: 5 holder/shank or obstacle collisions detected".
  The five collisions are on idx 6 (the 2 mm lake tool). They are not from the roughing change.
- Two findings stay on the roughing chain: a caution on idx 1 (engaged depth 12.0 mm against
  the vendor limit of 6.0 mm) and a critical on idx 2 (entry load). One adjustment on idx 2
  did not clear the critical.

## 2. Method and state

- Build: `target/release/rs_cam_gui`, built 2026-11-10 23:41 from `910dcf6d5`.
  `binary.newer_than_process` was false at each start. HEAD is `6b4a74f74`; the only change
  under `crates/` since the build is one test file
  (`micro_extrapolation_refuses_fm9.rs`).
- Start: `gui_restart`, then `load_project` of the operator file with `discard_unsaved: true`.
- No project-wide setting changed. The simulation resolution is the stored 0.2 mm.
- The measured state of this report is run 1 (section 5). It is the state of the saved file,
  with idx 16 disabled for the simulation only.
- The saved file has idx 16 enabled and idx 15 disabled. All other differences from the
  operator file are in section 4. A script compared the two files: no key outside the
  toolpaths differs.
- The GUI holds no project now: the last call was `gui_restart`.

### The memory stop

- The first `generate_all` (all 18 enabled) generated all 18 toolpaths. Idx 14 took about
  8 minutes.
- The GUI was then killed at the last plan step (21 of 21, the whole-project simulation):
  kernel OOM killer, SIGKILL, `anon-rss` 17.5 GB.
- The machine has 53 GB. Other processes held 36 GB and the swap (8 GB) was full. The cgroup
  limit of 28 GB was not reached.
- Log lines of that session:
  - idx 15: "Generated an empty toolpath, and that is expected here toolpath=Front 3D rough
    operation=3D Adaptive reason="it reads remaining stock, and the upstream operations left
    nothing to cut""
  - idx 16: "Scallop toolpath complete moves=559468 cutting_mm="162150.5" rapid_mm="22311.2""
- Second session: load of the working file, idx 15 and idx 16 disabled, `generate_all` (about
  9 minutes), `run_simulation`. This run completed. Peak GUI memory seen: 8.7 GB after the
  generation.

## 3. Dial-in decisions and their sources

### 3.1 Rebind

- `set_toolpath_tool` idx 1 and idx 14 to tool id 0 ("6 mm flat end mill (the cavity, the mech
  pockets), 30 mm stick-out"). Both replies were `ok: true`.
- Idx 1 reply, caution: "Cut 10.00 x 5.50 mm is outside the safe envelope: vendor axial bound
  The point is 67 % past the vendor axial bound limit. The value is written; it is a warning
  (ruling Q6)."
- Idx 1 reply, hint: "The stepover is 92 % of the tool diameter, past the corner-coverage bound
  of 85 %: a sharp offset corner keeps uncut material."
- Both replies, info: "The tool sticks out 30.0 mm, which is 5.0 × its diameter (6.000 mm).
  The calculator lowers the load target to 88 % to limit deflection. This limit has no
  published source. The feed rate does not change."

### 3.2 The add door (temporary toolpath, removed)

- `add_toolpath`: setup 0, `adaptive3d`, `tool_index` 0, `model_id` 0 (the model of idx 1).
  It became index 18 (id 25). `remove_toolpath` 18 removed it.
- The add door gave: depth per pass 3.0 mm, stepover 1.2 mm (20 % of the diameter),
  `clearing_strategy = contour_parallel`, `entry_style = plunge`, 24000 RPM, feed 7619,
  plunge 2500, `bottom_z` 0.5.
- The add reply carried a caution on its own point: "Cut 3.00 x 1.20 mm is outside the safe
  envelope: move budget The point is 46 % past the move budget limit. The value is written; it
  is a warning (ruling Q6)."
- `get_safe_envelope` of the temporary toolpath: `stated` 3.0 x 1.2; `operator` outside,
  binding `runtime`, margin -0.455; `proposal` 3.0 x 1.2 (equal to stated);
  `proposal_binding` null.

### 3.3 The envelope limits of the 6 mm tool (idx 1, read after the pick)

| Limit (`kind`) | Value | Basis |
|---|---|---|
| `vendor_axial` | ap <= 6.0 mm | printed, row `amana-flat-hardwood-pocket-6000-2f-spektra` |
| `rigidity` | ap <= 12.0 mm | machine rigidity depth cap (2 x D), machine file |
| `flute_guard` | ap <= 20.0 mm | 0.8 x flute, rule with no source |
| `cutting_length` | ap <= 25.0 mm | tool fact (estimated) |
| `vendor_depth_table` | ap(ae) curve: 25 below ae 0.268; 6.81 at ae 1.196; 6.0 from ae 1.314 | derived from the printed row |
| `power` | ap(ae) curve: 25 to ae 4.29; 22.7 at ae 5.15; 19.3 at ae 6.0 | machine power curve |
| `deflection` | unmodelled: "no deflection limit below the flute" | rule with no source (200 µm) |
| `diameter` | ae <= 6.0 mm | tool fact (stated) |
| `runtime` (move budget) | ae floor: 2.23 at ap 2.96; 1.83 at ap 3.61; 1.37 at ap 4.81; 1.10 at ap 5.99 | 500000 moves, rule with no source |
| `engagement_floor` | ap >= 0.05, ae >= 0.02 | rules with no source |
| `chip_band` | per point | printed row |

The width limits are `diameter` (ceiling) and `runtime` (floor). The envelope has no
corner-coverage limit for this op; the 85 % bound is a hint only.

### 3.4 The chosen cut

| Op | Chosen point | Source | Envelope |
|---|---|---|---|
| idx 14 Front bulk rough | ap 3.0, ae 1.2 | the add door point | `apply_feeds` reply: `outside_envelope: null` (inside) |
| idx 1 Back bulk rough | ap 6.0, ae 1.2 | the envelope: the add door point is outside for this op | `operator: inside`, margin 0.0 |

- Idx 1, first try, the add door point 3.0 x 1.2: `apply_feeds` wrote it and warned:
  `outside_envelope: {binding: "move budget", margin: -0.455}`.
- The add door and the envelope disagree for idx 1. The lead rule then takes the inside point
  with the smaller stepover. The stepover stays 1.2 mm. At 1.2 mm the move-budget floor needs
  a depth of about 5.5 mm or more, and `vendor_axial` limits the depth to 6.0 mm. The point
  6.0 x 1.2 is inside, on the `vendor_axial` limit (margin 0.0).
- The other inside option was depth 3.0 mm with a stepover of 2.3 mm or more. It has the
  larger stepover, so the rule does not take it.
- The envelope of idx 14 was not read again at ceiling 24000 (each reply is about 60 kB). The
  variant C read at ceiling 18000 gave the same limit names, and a move-budget floor of
  0.66 mm at ap 2.99 for idx 14.
- Applied speeds at both points: 24000 RPM, feed 7371, plunge 2500, ramp feed 6095.

### 3.5 Entries (step 3)

- Idx 1 keeps `entry_style = helix`. Idx 14 keeps `entry_style = ramp`.
- The 6 mm tool gets no entry finding on idx 1 or idx 14: no `safety.helix_inside_core`, no
  centre-core finding, no `project.entry_load`.
- The app gives no count of entries by kind. The tool-load report gives these populations:

| Op | Plunge moves | Ramp moves (steepest) | Samples in helix motion (time) |
|---|---|---|---|
| idx 1 | 2031 | 23317 (75.0 deg) | 42962 (114.3 s) |
| idx 14 | 244 | 26407 (74.9 deg) | 446080 (1087.6 s) |
| idx 2 | 114 | 339 (74.9 deg) | 13258 (53.4 s) |

### 3.6 Rest ops (step 4)

- Idx 2: enabled, `apply_feeds` with no pick.
- Idx 15: generated 0 moves in the first session (log line in section 2). It is disabled in
  the saved file. The operator decides if it is deleted.

### 3.7 Suggest on all 18 (step 5)

- `apply_feeds` with no pick ran on the 16 other toolpaths. No call refused. No call warned.
- Eight replies named one rule with no source, `material.plunge_rate_base.solid_wood`: idx 0,
  6, 7, 8, 9, 11, 12, 16. No reply on the 6 mm tool named one.
- Idx 3 and idx 4 (drill cycles) got 19685 RPM. All other toolpaths got 24000 RPM.

## 4. Per-op table (run 1, the state of the saved file)

Applied values. Format of the cut: depth per pass x stepover, mm.

| idx | Name | Tool | Status | RPM | Feed | Plunge | Ramp feed | Cut | Moves | Fed time s |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Face back to T1 | 18 mm 2F | Done | 24000 | 8651 | 2162 | 8597 | 0.5 x 8 | 317 | 172.3 |
| 1 | Back bulk rough 18 mm | 6 mm flat | Done | 24000 | 7371 | 2500 | 6095 | 6 x 1.2 | 85581 | 1983.1 |
| 2 | Back rest 6 mm | 6 mm flat | Done | 24000 | 6841 | 2500 | 6095 | 6 x 1.5 | 8381 | 202.9 |
| 3 | Pin drill, flip pins | 6 mm flat | Done | 19685 | 2499 | - | - | - | 118 | 4.5 |
| 4 | Drill light holes | 6 mm flat | Done | 19685 | 2499 | - | - | - | 463 | 14.8 |
| 5 | River channel 6 mm, pass 7 of 7 | 6 mm flat | Done | 24000 | 7161 | 2500 | - | - | 17261 | 93.5 |
| 6 | Lake pocket 2 mm, 11 lakes | 2 mm 1F | Done | 24000 | 1347 | 240 | - | 0.7 x 0.8 | 30111 | 4142.2 |
| 7 | Lake pocket 2 mm, 1 lake | 2 mm 1F | Done | 24000 | 1320 | 240 | - | 0.5 x 1 | 194 | 99.8 |
| 8 | ANTENNA_D5.00 pocket 2 mm | 2 mm 1F | Done | 24000 | 1320 | 240 | 1320 | 0.5 x 1 | 640 | 30.2 |
| 9 | V rivers and lakes | V 20 deg | Done | 24000 | 3048 | 381 | - | - | 22369 | 670.4 |
| 10 | Day strip slit V60 | V 60 deg | Done | 24000 | 3048 | 1016 | - | 2.6 | 19 | 21.9 |
| 11 | Face clamp pads 6 mm | 18 mm 2F | Done | 24000 | 8651 | 2162 | - | 0.5 x 8 | 64 | 7.4 |
| 12 | Face front to T | 18 mm 2F | Done | 24000 | 8651 | 2162 | 8597 | 0.5 x 8 | 371 | 176.2 |
| 13 | Engrave day labels V60 | V 60 deg | Done | 24000 | 3048 | 1016 | - | 0.6 | 314 | 10.5 |
| 14 | Front bulk rough 18 mm | 6 mm flat | Done | 24000 | 7371 | 2500 | 6095 | 3 x 1.2 | 112999 | 2402.8 |
| 15 | Front 3D rough | 6 mm flat | Disabled (0 moves) | 24000 | 7022 | 2500 | 6095 | 5.532 x 1.382 | 0 | - |
| 16 | Front scallop finish | R1 taper ball | Generated, not simulated | 24000 | 3657 | 240 | - | - | 559468 | not measured |
| 17 | Outline profile with tabs | 6 mm flat | Done | 24000 | 7161 | 2500 | 7161 | 1.25 | 1750 | 255.8 |

The names of idx 1 and idx 14 still say "18 mm". The brief said to rename nothing.

Verdicts with peaks. Power in kW of the available 1.5 kW; deflection in µm of 200 µm;
chipload as observed / bound in mm per tooth; depth in mm.

| idx | Power | Deflection | Chipload | Depth |
|---|---|---|---|---|
| 0 | unmodeled (`arc_engagement_not_captured`) | unmodeled | unmodeled (`all_samples_air_cut_or_rapid`) | within 0.00 (vacuous: 0 of 100522 samples) |
| 1 | within 0.388 | within 23.9 | within 0.1270 / 0.1270 | within 12.00 of 12.00 |
| 2 | within 0.296 | within 16.7 | within 0.1270 / 0.1270 | exceeds 16.95 of 12.00 |
| 3 | unmodeled (drill cycle) | unmodeled | unmodeled | unmodeled |
| 4 | unmodeled (drill cycle) | unmodeled | unmodeled | unmodeled |
| 5 | within 0.563 | within 32.8 | within 0.1741 / 0.1741 | reported 8.32 |
| 6 | within 0.005 | within 8.4 | within 0.0550 / 0.0550 | reported 0.66 |
| 7 | within 0.003 | within 6.4 | within 0.0550 / 0.0550 | reported 0.49 |
| 8 | within 0.003 | within 6.5 | within 0.0550 / 0.0550 | reported 0.50 |
| 9 | within 0.011 | within 42.0 | within 0.1778 / 0.1778 | reported 2.42 |
| 10 | within 0.118 | within 6.0 | within 0.0635 / 0.0635 | reported 4.60 |
| 11 | unmodeled (`arc_engagement_not_captured`) | unmodeled | unmodeled (all air) | within 0.00 (vacuous: 0 of 7073 samples) |
| 12 | within 0.121 | within 0.5 | within 0.1791 / 0.1791 | within 0.50 |
| 13 | within 0.002 | within 1.0 | within 0.0635 / 0.0635 | reported 0.60 |
| 14 | within 0.196 | within 14.0 | within 0.1270 / 0.1270 | within 3.35 of 12.00 |
| 15 | not measured | not measured | not measured | not measured |
| 16 | not measured | not measured | not measured | not measured |
| 17 | within 0.136 | within 10.6 | within 0.1741 / 0.1741 | within 1.75 |

Tool-load summary: 16 toolpaths, 15 within, 1 exceeds (idx 2, depth of cut).

Modulation summary.

| idx | Moves touched / total | Median feed change | Binding distribution |
|---|---|---|---|
| 0 | 0 / 60 | 0.0 % | machine_max_feed 1.00 |
| 1 | 84400 / 85272 | -1.4 % | chipload_max 0.45, kinematic_reach 0.24, machine_max_feed 0.21, feed_cap 0.08, plunge_rate 0.02 |
| 2 | 8276 / 8325 | -10.9 % | chipload_max 0.40, machine_max_feed 0.25, kinematic_reach 0.19, feed_cap 0.16 |
| 3, 4 | none (drill cycle) | - | - |
| 5 | 16461 / 16636 | +16.7 % | chipload_max 0.64, kinematic_reach 0.34, machine_max_feed 0.01 |
| 6 | 17174 / 27580 | -2.0 % | machine_max_feed 0.51, chipload_max 0.24, feed_cap 0.20, kinematic_reach 0.04 |
| 7 | 98 / 144 | +14.5 % | chipload_max 0.83, kinematic_reach 0.16, machine_max_feed 0.01 |
| 8 | 310 / 600 | +1036.4 % | machine_max_feed 0.68, chipload_max 0.17, kinematic_reach 0.15 |
| 9 | 21601 / 21680 | +40.0 % | chipload_max 0.77, kinematic_reach 0.23 |
| 10 | 0 / 6 | 0.0 % | chipload_max 1.00 |
| 11 | 24 / 32 | -0.6 % | machine_max_feed 1.00 |
| 12 | 180 / 182 | -25.7 % | machine_max_feed 0.36, kinematic_reach 0.32, chipload_max 0.32 |
| 13 | 15 / 189 | -3.9 % | chipload_max 0.91, kinematic_reach 0.08, machine_max_feed 0.01 |
| 14 | 109989 / 111417 | -5.6 % | chipload_max 0.52, kinematic_reach 0.33, feed_cap 0.11, machine_max_feed 0.05 |
| 17 | 1417 / 1494 | +16.7 % | chipload_max 0.91, machine_max_feed 0.05, kinematic_reach 0.04 |

## 5. Simulation and totals

| | Run 1 (16 toolpaths) |
|---|---|
| Headline | "ERROR: 5 holder/shank or obstacle collisions detected" |
| `collision_count` | 5 (all on idx 6) |
| `rapid_collision_count` | 0 |
| `collision_checks_failed` | 0 |
| `total_runtime_s` | 13401.4 (3 h 43 min), without idx 15 and idx 16 |
| Air cutting, of total runtime | 51.0 % |
| Sum of fed time, 16 toolpaths | 10288.3 s |

Fed time of the roughing chain, against P24_compare.md. Variants A and C ran at ceiling 18000;
this run is at ceiling 24000.

| Setup | A (18 mm, 18000) | C (6 mm, 6.0 x 5.5 and 4.166 x 3.6, 18000) | Dial-in (6 mm, 6.0 x 1.2 and 3.0 x 1.2, 24000) |
|---|---|---|---|
| Back (idx 1 + idx 2) | 837.3 s | 1068.7 s | 2186.0 s |
| Front (idx 14 + idx 15) | 1275.7 s | 939.0 s | 2402.8 s |
| Back + Front | 2113.1 s (35.2 min) | 2007.7 s (33.5 min) | 4588.8 s (76.5 min) |

- The dial-in chain takes 2.17 times the fed time of A and 2.29 times the fed time of C.
- Cutting distance: idx 1 is 234.9 m (C: 59.6 m). Idx 14 is 265.6 m (C: 70.1 m).
- Peak power on the two bulk roughs: 0.388 kW and 0.196 kW (26 % and 13 % of 1.5 kW).
- Fed time of the full setups in this run: Back setup (idx 0 to 10) 7435.6 s; clamp pads
  (idx 11) 7.4 s; Front setup without idx 15 and idx 16 (idx 12, 13, 14, 17) 2845.3 s.
- Idx 6 alone takes 4142.2 s of fed time.

## 6. The one adjustment (step 8)

The only critical finding on idx 1, 2, 14 or 15 is `project.entry_load` on idx 2.

- Adjustment: idx 2 `entry_style` from `ramp` to `helix` (`set_toolpath_param`). Then
  `generate_all` (1 generated, 15 current) and `run_simulation` (run 2).

| Idx 2 | Before (ramp) | After (helix) |
|---|---|---|
| Moves | 8381 | 10538 |
| Fed time | 202.9 s | 170.1 s |
| `project.entry_load` (critical) | 391 of 13590 entry samples over 1.15 mm, peak 14.50 mm at (211.3, 42.0, 11.00) | 386 of 8280 entry samples over 1.15 mm, peak 14.50 mm at (211.3, 42.0, 11.00) |
| `project.sim_outside_envelope` (caution) | 12 of 28706 samples, depth 16.95 mm | 8 of 27456 samples, depth 16.95 mm |
| `load.depth.exceeds` (caution) | 16.95 of 12.00 mm | 16.95 of 12.00 mm |

- The adjustment did not clear the critical. The peak and its position did not change.
- The saved file keeps `entry_style = ramp` on idx 2 (the operator's value). The helix try is
  not in the file.
- No adjustment ran for the idx 1 caution. The brief allows one adjustment.

## 7. Findings: caution, critical, blocking

No finding has the severity blocking.

### 7.1 From the roughing change

| Severity | Toolpath | Id | Finding |
|---|---|---|---|
| caution | idx 1 | `project.sim_outside_envelope` | 802 of 696121 cutting samples outside the envelope; worst: vendor axial bound +100 % (move 70277: depth 12.00 mm, width 0.20 mm; limit 6.00 mm) |
| caution (project) | idx 15 | `project.generated_empty` | 0 moves after the 6 mm bulk rough (seen in variant C and in the first session of this run); now disabled |
| not a finding | idx 1, idx 14 | - | fed time 1983.1 s and 2402.8 s at the chosen stepover of 1.2 mm |

The depth verdict of idx 1 is `within` at 12.000 of 12.000 mm: it is on the rigidity limit.

### 7.2 Pre-existing: tool or job facts for the operator

| Severity | Toolpath | Id | Finding |
|---|---|---|---|
| **critical** | idx 6 | `project.holder_collision` | 5 fed moves put the shank into the stock; worst: move 22134 at X111.281 Y189.899 Z7.200, 1.30 mm of wood over its bottom, 8.00 mm above the tip. Others: moves 19980 (0.64 mm), 24288 (0.15 mm), 26988 (0.82 mm), 27143 (0.82 mm) |
| **critical** | idx 2 | `project.entry_load` | 391 of 13590 entry samples remove more than 1.15 mm; peak 14.50 mm at (211.3, 42.0, 11.00). In variant A (18 mm bulk): 182 of 46299, peak 14.70 mm |
| caution | idx 2 | `load.depth.exceeds` | depth of cut 16.950 mm, limit 12.000 mm (2 x D, no published source). Variant A: 20.70 mm |
| caution | idx 2 | `project.sim_outside_envelope` | 12 of 28706 samples; vendor axial bound +182 % (move 2065: depth 16.95 mm, width 1.20 mm) |
| caution | idx 3 | `tool.flute_overreach` | flute 25.00 mm: 5 moves with wood above the flute; worst move 91 at X388.750 Y350.000 Z-4.500, 30.00 mm of wood over the tip |
| caution | idx 3 | `tool.reach_to_holder` | "stickout 30 mm, estimated: cuts 30 mm below the top (by the stock box)" |
| caution | idx 6 | `project.sim_outside_envelope` | 6057 of 40923 samples; corner coverage +7 % (move 10875: width 1.83 mm, limit 1.71 mm) |
| caution | idx 7 | `tool.reach_to_holder` | "stickout 20 mm, estimated: cuts 21.506 mm below the top (by the stock box)" |
| caution | idx 7 | `project.sim_outside_envelope` | 180 of 432 samples; corner coverage +6 % (move 5: width 1.81 mm, limit 1.71 mm) |
| caution | idx 8 | `project.sim_outside_envelope` | 770 of 840 samples; corner coverage +5 % (move 61: width 1.80 mm, limit 1.71 mm) |
| caution | idx 9 | `tool.flute_overreach` | flute 8.15 mm: 205 moves; worst move 13841 at X110.618 Y189.126 Z5.856, 10.64 mm of wood over the tip |
| caution | idx 9 | `compat.project_curve_point_projection` | "Point projection does not protect the far surface: the tool body can cut it." |
| caution | idx 10 | `tool.flute_overreach` | flute 5.50 mm: 7 moves; worst move 15 at X342.833 Y95.000 Z-0.970, 7.44 mm of wood over the tip |
| caution | idx 10 | `geom.depth_beyond_stock` | "The cut goes 0.97 mm below the bottom of the stock" |
| caution | idx 10 | `project.sim_outside_envelope` | 1144 of 3492 samples; flute guard +5 % (move 4: depth 4.60 mm, limit 4.40 mm) |
| caution | idx 10 | `project.crosses_standing_material` | 33.1 % of cutting samples remove more than the commanded 1.93 mm depth per pass; peak 4.60 mm |
| caution | idx 12 | `project.sim_outside_envelope` | 211 of 90004 samples; corner coverage +15 % (move 143: width 17.60 mm, limit 15.36 mm) |

Toolpaths with no finding of caution level or more: idx 0, 4, 11, 13, 14, 17.

Lower severities, for the record: hint `project.air_cut_high` (idx 0 84 %, idx 2 58 %, idx 6
77 %, idx 7 84 %, idx 8 52 %, idx 11 60 %); info `tool.name_size_mismatch` on the 20 degree
V-bit.

### 7.3 App defects suspected

Each item is an observation. No code was read.

1. The add door writes a point that its own envelope calls outside. `add_toolpath` gave
   3.0 x 1.2 with the caution "46 % past the move budget limit". The envelope `proposal` of
   that toolpath is the same point, with `proposal_binding` null, while `operator` is
   `outside` (binding `runtime`).
2. The engaged depth of idx 1 is twice its depth per pass. The op holds 6.0 mm and its levels
   are Z 19.5, 13.5 and 11.0. The simulation measured 12.00 mm at move 70277 (width 0.20 mm).
   The envelope calls the stated point inside. In variant C the same op measured 9.0 mm. The
   log has lines "C38 piece left to the rest pass … why="F-038 forecast"".
3. Idx 2 cuts stock at its full height in one step: depth 16.95 mm, and entry samples of
   14.50 mm at Z 11.00. Its levels are Z 8.55, 7.5 and 1.5 only. The entry style does not
   change the finding (section 6). The steepest ramp of idx 2 is 74.9 degrees with
   `ramp_angle_deg = 3`.
4. `stale.project_curve_negative_depth` (caution) on idx 5 and idx 9 says "it cuts air … so it
   cuts nothing". The simulation measured cutting on both: idx 5 peak power 0.563 kW and depth
   8.32 mm; idx 9 depth 2.42 mm. The fix payload would set idx 5 depth to +4.0 and idx 9 to
   +2.418. I did not apply it.
5. `project.generated_empty` on idx 15 names the setup orientation, the stock position and a
   Project Curve depth as causes. The log names the true cause: the upstream operations left
   nothing to cut.
6. The modulation summary of idx 8 gives a median feed change of +1036.4 %.
7. The whole-project simulation with the scallop finish (559468 moves) needs more than 17 GB
   at the 0.2 mm resolution. The memory budget of the app (28 GB) did not stop it before the
   kernel did.
8. `feeds.long_tool_derate` says "lowers the load target to 88 %" and "The feed rate does not
   change" on every 6 mm toolpath. The limit has no published source (the app says so).

## 8. Not measured

- Idx 16 (scallop finish) on the new rough stock: no simulation, no load verdicts, no
  findings. Its generation numbers are from the first session.
- The stock that the 6 mm bulk roughs leave for the finish, against the stock that the 18 mm
  chain leaves. No comparison ran.
- The envelope of idx 14 at ceiling 24000 (section 3.4).
- A count of entries by kind. The app gives populations of moves and samples only.
- The diagnostics of run 1 for the toolpaths other than idx 1, 2 and 14. I read them in run 2
  (idx 2 on helix). Run 1 and run 2 differ only on idx 2 (and 2.3 s of fed time on idx 6); the
  run 2 simulation verdict list is the same as run 1.
- Tool-change time. `get_suggest_rationale` was not read.
- Each state ran one time.
- The variant C cut (6.0 x 5.5 and 4.166 x 3.6) at ceiling 24000. It ran at 18000 only.

## 9. What blocks a move into the operator file

- The chosen cut is slow: 76.5 min of roughing against 35.2 min (A) and 33.5 min (C). The
  operator must rule on the stepover. The add door value (1.2 mm) and the variant C values
  (5.5 mm, 3.6 mm) are the two measured options.
- Idx 16 is not verified on the new stock (memory).
- Idx 15 is empty. The operator decides: delete it or keep it disabled.
- The idx 1 engaged depth of 12 mm (caution) and the idx 2 entry load (critical) are open.
  The idx 2 critical is in variant A too.
- Suggest changed the stored speeds of all 18 toolpaths (step 5). Section 4 has the values;
  for example idx 5 went from 10610 RPM / 3165 to 24000 RPM / 7161.
