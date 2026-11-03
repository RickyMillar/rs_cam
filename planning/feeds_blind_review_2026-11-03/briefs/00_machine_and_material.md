# 00 Machine and material (all briefs refer to this file)

Job: nz-wanaka-21, a RivMap oak block, two-sided. Block 358 x 443 x 25 mm, cut from stock
395 x 460 mm. The operator measured the raw stock at 26 mm thick.

## Machine

Source: `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml` (the same values are in the
`[job.machine]` table of `planning/rivmap_block_job/nz_wanaka_21_operator.toml`).

| Field | Value | Note |
|---|---|---|
| Machine | Shapeoko Pro XXL, Scylla grblHAL controller | belt-driven gantry router |
| Spindle | H100 VFD, 1.5 kW rated | `spindle_drive.plugin = "H-100 VFD"` |
| Spindle range | 6000 to 24000 rpm | `[spindle.Variable]` |
| Power model | VFD constant torque, 1.5 kW at 24000 rpm | `[power.VfdConstantTorque]` |
| Max collet / shank | 7.0 mm | `max_shank_mm` |
| Max feed (travel, XY) | 15000 mm/min | `max_feed_mm_min`, `max_rate_xyz_mm_min` X and Y |
| Max feed Z | 2500 mm/min | `max_rate_xyz_mm_min` Z |
| Acceleration X, Y | 1200 mm/s^2 | `acceleration_xyz_mm_s2` |
| Acceleration Z | 550 mm/s^2 | `acceleration_xyz_mm_s2` |
| Acceleration (scalar) | 983.3 mm/s^2 | `acceleration_mm_s2` |
| Junction deviation | 0.1 mm | grblHAL cornering |
| Controller spindle scale | `rpm_max = 1000`, `rpm_min = 0`, mode 0 | grblHAL `$30`/`$31` values from the `$$` import; the file does not say how the VFD plugin maps them |
| Spindle-at-speed wait | yes, 5 % tolerance; spin-up delay 0 s | `controller_waits_for_spindle = true` in the post |
| Tool change | mode 3 (probe), 40 mm probe distance | |
| Coolant | off on every op | air blast or dust shoe only (dust shoe brush 25 mm above the bit end, generator file) |

### Power at the spindle (from the power model, not measured)

rs_cam computes the available power as `P(n) = 1.5 kW x min(n, 24000) / 24000`
(`MachineProfile::power_at_rpm`, `crates/rs_cam_core/src/machine/mod.rs`). Constant torque is
1500 W / (2 pi x 400 rev/s) = 0.597 N m.

| Spindle speed (rpm) | Available power (kW) |
|---|---|
| 6000 | 0.375 |
| 9000 | 0.5625 |
| 12000 | 0.75 |
| 15000 | 0.9375 |
| 18000 | 1.125 |
| 21000 | 1.3125 |
| 24000 | 1.5 |

### Rigidity factors (the operator's planning rules, not limits)

The generator uses these factors to set the planned stepovers and depths. The operator set them on
2026-11-02 ("industry standard, a bit softer").

| Factor | Value |
|---|---|
| `doc_roughing_factor` | 0.25 x D |
| `doc_finishing_factor` | 0.1 x D |
| `woc_roughing_factor` | 0.4 x D |
| `woc_roughing_max_mm` | 6.35 mm |
| `woc_finishing_mm` | 0.635 mm |
| `adaptive_doc_factor` | 2.0 x D |
| `adaptive_woc_factor` | 0.2 x D |

### rs_cam cutting-feed cap (a software policy, not a machine fact)

When a machine states no `max_cutting_feed_mm_min`, rs_cam caps every cutting feed at
`DEFAULT_CUTTING_FEED_CAP_MM_MIN = 6000.0` mm/min (`crates/rs_cam_core/src/machine/mod.rs`). The
code comment gives the reason: vendor wood-routing charts in the literature net stop near
5000-6000 mm/min. This machine file states no cutting cap, so the 6000 mm/min cap applies. Rapids
use the travel rate.

## Material

### Oak

- Species: white oak (`[job.stock.material.SolidWood] species = "WhiteOak"`).
- Specific gravity 0.68 at 12 % moisture: USDA FPL Wood Handbook Table 5-3a, the "Oak, white" row,
  as quoted in `crates/rs_cam_core/src/material/mod.rs` (`FPL_ROW_OAK_WHITE`).
- Janka hardness 1360 lbf: the rs_cam material table (`WoodSpecies::janka_lbf`).
- Grain: the 460 mm stock length runs along the grain. That is block Y (setup-1 frame Y).
  The faces at Y min and Y max of the outline are end grain.
- Moisture of this board: not stated (find out).
- White oak has open pores and medullary rays. Expect tear-out on the edges of shallow cuts across
  the grain and burn marks where a tool dwells (ramps, plunges, slow corners).

### Epoxy

- The pour is a stock change in setup 2: `Custom` material "Epoxy", `feed_scale_factor = 1.0`
  (rs_cam treats it as oak for feeds). The product, the hardness and the cure time are not stated.
- Volume: 161.7 ml for the pour (block.json `epoxy_ml.pour`).
- Where it is: the back cavity (309 x 309 mm, 9 mm deep, filled to its floor + 1.0 mm overfill),
  the V20 grooves, the lake pockets, the 42 light holes, and the day-strip slit (filled up to the
  slit slot floor).
- Which ops meet it: only the setup-3 (front) ops. The front face meets the slit line. The front
  roughs and the scallop finish meet the V grooves (their tips pass the front surface by about
  2.4 mm), the lake pockets (floors 2.0 mm past the front) and the light-hole tops (1.0 mm past the
  front surface). The back ops cut before the pour and meet no epoxy.
- Concerns: heat softens epoxy and it can smear or chip at the oak/epoxy edge. A clean edge on the
  visible front is the goal.

### Spoilboard

Z0 is the spoilboard top on every setup. The pin drill, the slit and the outline profile reach it.
The spoilboard material is not stated in the project.

## Setup order (op index = position in the enabled order of the project file)

| Setup | Ops |
|---|---|
| 1 Back (back face up, top clamps) | 0-10 |
| 1b Clamp pads (pins in, top clamps off, side pushers hold) | 11 |
| 2 Epoxy pour (no cut) | none |
| 3 Front (flipped on the pins, top clamps) | 12-18 |
