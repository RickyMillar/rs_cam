# wanaka200 — feeds, speeds and entries on the two new operations

Date: 2026-09-19. Read-only review. The project was not changed.

Project: `~/Downloads/wanaka200/wanaka200.toml` (saved 2026-09-19 11:22).
GUI binary: release build of 2026-09-18 13:48. Both operations are
generated and simulated.

Setup 2 (Top) holds only these two operations. Both read fresh stock.

## 1. What is set

| | Scallop Finish 10 (`iso_field = true`) | Project Curve 7 |
|---|---|---|
| Tool | R1.0 tapered ball: O2.0 tip, 5.7 deg/side, O6 shaft, 2F, 35 mm stickout | 20 deg V-bit O5.5, 2F, 30 mm stickout |
| RPM | 18500 | 18000 |
| Feed | 782 mm/min | 900 mm/min |
| Chipload | 21.1 um/tooth | 25.0 um/tooth |
| Plunge | 170 mm/min (22 % of feed) | 127 mm/min (14 % of feed) |
| Cut | 0.1 mm cusp -> 0.872 mm stepover | 1.0 mm depth |
| Entries | `entry_style = "none"` | `entry_style = "none"`, `link_moves = false`, `lead_in_out = false`, `chain_distance_mm = 0.0` |
| Fed time | 6853.75 s (1 h 54 min) | 1242.80 s (20.7 min) |

## 2. The application arithmetic is correct

Four app numbers were re-derived by hand. All four agree:

| Quantity | Hand calculation | The application |
|---|---|---|
| Stepover from a 0.1 mm cusp | `2*sqrt(2Rh - h^2)` = 0.8718 mm | 0.872 mm |
| Available power at 18500 rpm | 1.5 kW x 18500/24000 x 0.75 = 0.8672 kW | 0.8672 kW |
| Tip deflection, tapered cantilever | 7 um to 13 um | 10.3 um |
| Plunge overspeed on the V-bit | 900 x sin(73.16 deg) = 861 mm/min | 873.8 mm/min (6.88x) |

The deflection figure comes from a numeric moment-area integration of the
real geometry: a cone from O2.0 at the tip to O6.0 at 20 mm, then a O6.0
shank to 35 mm, E = 600 GPa, a 24.5 N resultant force. The force comes from
the reported 68 W at the O3.209 effective cutting diameter, with a radial
force of half the tangential force.

## 3. Finding 1 — Setup 2 has no roughing pass

The O2.0 tapered ball cuts the whole relief from solid Baltic birch.

- Z range of the pass: 27.000 mm down to 17.244 mm. The tool removes
  9.76 mm of stock.
- Peak axial engagement: 7.44 mm (narration), 7.004 mm (emitted report).
- Engagement histogram: 32.1 % of samples are heavy (above 0.70).
- The depth-of-cut gate reports Exceeds on both operations.

Suggest already names the correct figure. `get_suggest_rationale(5)` says:
"scallop envelope max DOC 0.44 mm" and "Coordinate rough's stock_to_leave so
the finish per-pass DOC sits below 0.44 mm". The actual peak is 16x that
number.

The cut survives because the taper is stiff. A straight O2.0 mill at the same
stickout and load deflects 742 um. The cone deflects 10 um, a ratio near 80.
So this is not a deflection problem. It is a heat problem and a time problem.

## 4. Finding 2 — the entries

### Project Curve 7 spends 85 % of its fed time descending

The time-weighted commanded rate is 241.34 mm/min. The feed is 900 mm/min and
the plunge rate is 127 mm/min. Solve `127p + 900(1-p) = 241.34`:

    p = 0.852

So 1059 s of the 1242.8 s fed time is vertical descent at 127 mm/min. That is
17.6 minutes of the 20.7 minute operation. The descent is about 9 mm per
entry, 250 times.

The operation links nothing: `link_moves = false`, `lead_in_out = false`,
`chain_distance_mm = 0.0`. It emits 250 retract round trips for 250 curve
fragments. 66.3 % of its total runtime is air.

### One emitted move descends at 874 mm/min

`kinematic_utilization.plunge.peak_ratio` = 6.880 for toolpath 19. The worst
move is index 3700 at position (100.543, 63.066, 17.397). The cause is a
73.16 degree descent run at the full 900 mm/min feed. The vertical component
is 861 mm/min against a plunge rate of 127 mm/min. The tool at that moment is
a 20 degree V point.

Toolpath 17 carries a `modulation_summary`; toolpath 19 carries none. Feed
modulation held the scallop plunges to ratio 1.000. It did not run on the
project curve.

### The plunge chiploads cannot form a chip

- Scallop: 170 / (18500 x 2) = 4.59 um/tooth in Z.
- Project curve: 127 / (18000 x 2) = 3.53 um/tooth in Z.

Each V-bit entry stays in contact about 3.6 s at that rate. Expect a burn mark
at the start of each of the 250 fragments.

### The scallop declines most of its links

`intra_pass_hookup_mm = 3.0`. The link stage linked 70 of 192 junctions and
declined 122 as too_far. 125 retract round trips remain.

## 5. Finding 3 — the chipload floor conflict on the scallop

The matched vendor band is 9.35 um/tooth to 18.70 um/tooth (row
`amana-tapered-hardwood-scallop-3175-2f`, semi-finish role substituted for
finish, scaled x1.0064 on diameter and x1.0992 on hardness from O3.175).
The chip-formation floor is 25.0 um/tooth. The whole band sits below the
floor.

The application commands 21.1 um/tooth, which is 1.13x the band maximum, and
still warns that burnishing is expected.

Independent check with radial chip thinning makes it worse. At the effective
diameter O3.2086 and a stepover of 0.8718 mm:

    a_e/D = 0.272
    h_max = f_z x 2*sqrt((a_e/D)(1 - a_e/D)) = 21.1 x 0.8897 = 18.8 um

So the true maximum chip is 18.8 um, not 21.1 um. It sits on the band maximum
and well under the floor.

The lever is RPM, not feed. At the same 782 mm/min:

| RPM | Chipload | Available power | Surface speed at O3.209 |
|---|---|---|---|
| 18500 | 21.1 um | 0.867 kW | 186 m/min |
| 15000 | 26.1 um | 0.703 kW | 151 m/min |
| 12000 | 32.6 um | 0.563 kW | 121 m/min |

Power use is 0.068 kW, so no RPM in that range is blocked by power. The cost
of a lower RPM is surface finish. The cusp is 100 um and the chip is 26 um at
15000 rpm, so the cusp still sets what the eye sees by a factor of 4.

Do not make this change before the roughing pass exists. At 7 mm of buried
engagement the whole comparison is moot.

## 6. Finding 4 — the V-bit surface speed is inherent

A 20 degree V-bit at 1.0 mm depth cuts a groove `2 x 1.0 x tan(10 deg)` =
0.3527 mm wide. The maximum surface speed is therefore:

    pi x 0.0003527 m x 18000 = 19.9 m/min (65 SFM)

The speed falls to zero at the point. No RPM changes this. It is a property of
V-carving, not a defect in the setup. Accept it.

The commanded 25.0 um/tooth is exactly the rubbing floor; the application
clamped it there. The machine has headroom: utilisation is 99.9 %, the feed
ceiling is 6000 mm/min, and the report predicts +23 % achieved feed for a
x1.30 commanded rise. That would give 32.5 um/tooth.

## 7. The gates that could stop the job are quiet

| Gate | Scallop | Project curve |
|---|---|---|
| Power | 0.068 of 0.867 kW (8 %) | 0.010 of 0.844 kW (1 %) |
| Deflection | 10 um of 200 um | 6 um of 200 um |
| Chipload | Within band | Unmodeled (no vendor rows for a V-bit curve pass) |
| Depth of cut | **Exceeds**: 7.004 mm against 0.48 mm | **Exceeds**: 2.682 mm against 0.44 mm |

Depth of cut is the only gate that fires. On the scallop it is the missing
roughing pass. On the project curve the peak of 2.68 mm against a commanded
1.0 mm comes from the curve crossing steep relief; check whether that is
intended.

## 8. Recommended order of work

1. Add a 3D rough to Setup 2 with the O6 end mill. Set `stock_to_leave` so the
   scallop per-pass DOC stays below 0.44 mm.
2. Set `chain_distance_mm` on the project curve, and set `link_moves = true`.
   This removes most of the 250 retract and plunge cycles.
3. Raise the project curve plunge rate from 127 mm/min. One third of the feed
   is 300 mm/min.
4. Raise `intra_pass_hookup_mm` on the scallop above 3.0 mm.
5. Find out why feed modulation did not run on the project curve. The
   874 mm/min descent onto a V point is the risk it should have caught.
6. Re-derive the scallop RPM only after step 1 lands.

## 9. Open item

A separate agent is collecting published chipload charts from tooling vendors
to compare against these figures. It writes
`planning/load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md`. That file was
not complete when this report was written.

---

# Addendum — the feeds were dialled in (2026-09-19)

The operator decided that the relief is shallow and that a roughing pass is
not necessary for this job. The operator asked for a feed increase in place of
an RPM decrease. Both RPM values stay as they were.

## What changed

| Operation | Parameter | From | To |
|---|---|---|---|
| Scallop Finish 10 | feed_rate | 782 | **1050** mm/min |
| Scallop Finish 10 | plunge_rate | 170 | **300** mm/min |
| Scallop Finish 10 | spindle_rpm | 18500 | 18500 (held) |
| Project Curve 7 | feed_rate | 900 | **1200** mm/min |
| Project Curve 7 | plunge_rate | 127 | **400** mm/min |
| Project Curve 7 | spindle_rpm | 18000 | 18000 (held) |

A plunge rate of 350 mm/min was refused on the scallop. The application holds
a tool-geometry safety cap of 300 mm/min for a small ball or tapered-ball
flute tip (`stale.tapered_ball_plunge`). The value was set to 300 mm/min.

Both operations were regenerated. A simulation was run at 0.5 mm resolution.

## Result

Total runtime falls from 8722.98 s to 7645.97 s. That is 18 minutes.

| Gate | Scallop before | Scallop after | Curve before | Curve after |
|---|---|---|---|---|
| Power | 0.068 kW | 0.061 kW | 0.010 kW | 0.006 kW |
| Deflection | 10.3 um | 10.3 um | 5.7 um | 6.0 um |
| Observed chipload | 18.7 um | **18.7 um** | unmodeled | unmodeled |

## Finding 5 — feed is NOT the lever for the scallop chipload

The observed feed-per-tooth did not move. It reads 0.0187 mm/tooth before and
after. The achieved/commanded ratio fell from 0.885 to **0.659**.

The feed modulator clamps the scallop feed back to the vendor band ceiling.
A higher commanded feed therefore buys runtime on the moves that the band does
not bind, and buys nothing on the moves that it does. The burnishing is not
fixed.

Only RPM raises the true chip on this operation, because the modulator cannot
clamp RPM. At 782 mm/min and 15000 rpm the chip is 26.1 um, which clears the
25 um floor. The operator has chosen not to do this.

## Finding 6 — the V-bit descent is now marked critical

`project.plunge_class_load` on toolpath 19, severity **critical**:

    1 of 250 vertical-dominant moves exceed 400 mm/min, peaking at
    1165 mm/min (2.9x) at move 3700 (100.5, 63.1, 17.40)

The ratio improved from 6.88x to 2.9x, because the plunge rate rose. The
absolute rate got worse, because the descent follows the lateral feed: it was
874 mm/min and it is now 1165 mm/min. The rate is what breaks a V point, not
the ratio.

The cause is unchanged. The curve crosses a 73 degree slope, and feed
modulation does not run on this operation. Three options exist:

1. Accept it. It is one move of 250.
2. Lower the project curve feed. This only scales the descent rate down.
3. Find why the modulator skips a project_curve operation. This is the real
   fix and it belongs in the register.

## State

The changes live in the GUI session only. The file
`~/Downloads/wanaka200/wanaka200.toml` does NOT hold them. Save the project in
the GUI to keep them.
