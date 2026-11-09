# P22f-2d: the wide samples read by section, and a design survey (2026-11-10)

Read-only on product. Base `7b09f8d88` (P22f-2c). Test code only: the P22a readout,
`tests/common/wide_samples.rs`, and the test-support step probe (one more out-field). No bless, no
commit. One wanaka run (both arms in one invocation), one fixture run.

## 0. Summary

- **By section, no sample of wanaka idx 1 is wide.** 3731 samples are wide by rectangle (the
  gate's `ap x ae`, ae from the arc); 0 are wide by section (`mrr / (F / 60)` against the pass
  section `c D dpp` = 59.91 mm2, plus the read precision in area). The worst section ratio of any
  class is 1.027 (D65 2D, ap 5.0, at a capped feed), then 0.989 (a joint) and 0.911 (an uncapped
  flat cut), all under 1 plus the precision. So the 3731 are a rectangle overstatement, by this
  read. On the shelf fixture: 441 by rectangle, 1 by section (a capped planned turn, ratio 2.14).
- **By section, every class's worst sample is at or under 0.80 kW** (the P20i law at
  `ae = section / ap`): the joint 0.78, the uncapped flat cut 0.73, the planned turn 0.49, the D65 2D
  step 0.40; the ramp entries 0.05. No real class stays over `c` by section.
- **The open item of P22f-2c is settled**: the deep cells of the side band at 1620 (depth 7.2-10 mm)
  lie on the rim of the disc, their centres 0.12-0.67 mm inside the edge. The planner's 1.5 mm cell
  counts each as a whole strip at full depth; the simulation (0.2 mm cells) removes only the
  sliver inside the disc. The planner over-reads (the safe side); the simulated removal is the
  measure.
- **Caveat**: the per-sample removal is noisy. The first and last sample of a move read a removal
  ratio of 2-6 (dropped). Inside a move a few samples read a section over the full-width rectangle
  `D x ap` (physically impossible): 1 on wanaka idx 1, 9 on the fixture; they are counted and left
  out. A section read in a product door needs the same filter or a per-move removal.
- **Design**: five core readers judge a simulated sample by the rectangle. The one-definition rule
  favours option (B), a measured section variant of `EngagedSection` read by the same power and
  force doors; the chip-thinning reader keeps the arc.

## 1. Part 1: the readout re-read

### 1.1 Choices

- (a) **Mid-move samples only**: the first and last sample of each move are dropped from the
  removal and section reads. Why: P22f-2c measured the removal ratio 2-6 at a move end (the
  segment time there does not match the volume the stamp removes), 0.04-0.12 inside the move; the
  trace gives the volume per sample, not per move, so the per-move removal would carry the same
  end samples.
- (b) **Section**: `section_mm2 = mrr / (F / 60)` (the sample's removed area per mm of travel). The
  section ceiling is `c D dpp`. A sample is wide by section when its section exceeds the ceiling
  plus `(planner cell + sim cell) x ap`. A section over `D x ap` is physically impossible; such a
  sample is counted in its own column and left out.

### 1.2 Wanaka idx 1 (Back bulk rough 18 mm; c D dpp 59.91 mm2)

| Class | Wide by rectangle (> c D + both cells) | Wide by section | Impossible section reads | Worst section sample: move, ap, section (mm2), ratio, ae = section / ap, feed | Power at the worst section (P20i law) |
|---|---|---|---|---|---|
| phantom transit | 7 | 0 | 0 | 14976, 0.30, 1.2, 0.019, 3.92, 8475 | 0.02 kW |
| ramp entry | 3142 | 0 | 0 | 8284, 0.67, 4.0, 0.067, 5.95, 6448 | 0.05 kW |
| planned turn (exact) | 79 | 0 | 0 | 5227, 9.00, 55.0, 0.918, 6.11, 3934 | 0.49 kW |
| D65 2D (exact) | 20 | 0 | 0 | 6817, 5.00, 61.5, 1.027, 12.31, 2561 | 0.40 kW |
| departure window (exact) | 129 | 0 | 0 | 378, 2.81, 1.6, 0.027, 0.58, 2975 | 0.03 kW |
| D65 3D split (exact) | 11 | 0 | 0 | 1972, 7.60, 13.7, 0.228, 1.80, 6653 | 0.21 kW |
| 3D stock read (exact) | 74 | 0 | 0 | 1620, 6.88, 11.0, 0.183, 1.59, 7924 | 0.19 kW |
| perimeter sweep (inferred witness) | 7 | 0 | 0 | 11547, 2.45, 9.4, 0.157, 3.84, 8475 | 0.14 kW |
| joint | 75 | 0 | 0 | 8695, 9.00, 59.2, 0.989, 6.58, 8475 | 0.78 kW |
| uncapped draped step | 79 | 0 | 1 | 1229, 9.00, 41.7, 0.696, 4.63, 2702 | 0.35 kW |
| OTHER: uncapped flat cut | 108 | 0 | 0 | 7460, 9.00, 54.6, 0.911, 6.06, 8475 | 0.73 kW |
| **total** | **3731** | **0** | 1 | | |

The bound of a planned step at the opening (ap 10, ae `c D` 5.99, F 8475) is 0.80 kW. Power law:
`P = Ks ap ae F / 6e7 + F_edge ap (pi D n)(z psi / 2 pi) / 6e7`, `psi = acos(1 - 2 ae / D)`,
`Ks` 58.516, `F_edge` 4.595 (P20i), D 18, n 18000, z 2; available 1.125 kW.

### 1.3 Shelf fixture (D 6; c D dpp 13.06 mm2)

| Class | Wide by rectangle | Wide by section | Impossible | Worst section: move, ap, section, ratio, ae, feed |
|---|---|---|---|---|
| phantom transit | 105 | 0 | 5 | 1082, 1.76, 8.6, 0.659, 4.90, 500 |
| ramp entry | 169 | 0 | 1 | 9, 1.37, 8.1, 0.619, 5.90, 500 |
| planned turn (exact) | 48 | 1 | 1 | 264, 5.14, 28.0, 2.141, 5.44, 560 (capped) |
| D65 2D (exact) | 31 | 0 | 0 | 262, 1.95, 8.5, 0.651, 4.36, 988 |
| departure window (exact) | 15 | 0 | 0 | - |
| D65 3D split (exact) | 2 | 0 | 0 | - |
| 3D stock read (exact) | 24 | 0 | 1 | 558, 2.39, 9.7, 0.745, 4.07, 1410 |
| joint | 28 | 0 | 0 | 998, 2.39, 1.3, 0.100, 0.54, 1500 |
| uncapped draped step | 13 | 0 | 1 | 307, 2.27, 11.9, 0.911, 5.23, 1500 |
| OTHER: uncapped flat cut | 6 | 0 | 0 | 998, 2.99, 1.6, 0.120, 0.53, 1500 |
| **total** | **441** | **1** | 9 | |

### 1.4 The side band at 1620 (the P22f-2c open item)

The probe's per-cell read of the worst sample (strips -6 to -3): the deep cells stand at
y 171.0, 8.33-8.88 mm from the tool axis (0.12-0.67 mm inside the edge, R 9), tops 22.72-25.50,
depth 7.22-10.00. Every other band cell is the 0.56 mm layer (top 16.06, floor 15.50). The
planner's strip section at that sample (about 36 strip-mm x 1.5 mm) counts the rim cells whole;
the simulated section at 1621 is 6-13 mm2 per mm mid-move. The simulation's stock top at the same
XY before the move is not exposed (the result keeps a mesh), so the sliver width is inferred from
the cell distances, not read.

### 1.5 Answers for the morning

- **How much of the 3731 is a rectangle overstatement?** All of it, by the section read: 0 samples
  are wide by section; the worst class ratio is 1.03 (within the precision).
- **Does any real class stay over `c` by section?** No. The ramp entries (3142 by rectangle) read at
  most 0.067 of the pass section; the joints 0.989; the uncapped flat cuts 0.911.
- **The planner is right at 1620/1621**: its area load (0.06-0.36) and the simulated removal agree
  in kind; where they differ, the planner over-reads the rim.

## 2. Part 2: the design survey

### 2.1 Every reader that judges a simulated sample by `ap x ae`

| Reader | Path | What it reads from the sample | Does the sample carry the section? | If the section replaced the rectangle |
|---|---|---|---|---|
| Power gate | `tool_load/power.rs` `sample_power_kw` -> `Engagement::from_sample` -> `power_inputs` -> `EngagedSection::Rectangle` (`feeds/section.rs`) | ap = `axial_doc_mm`, ae = `r (1 - cos psi)` at the arc, psi = the arc | yes: `mrr_mm3_s`, `removed_volume_est_mm3` (noisy at move ends) | the shear term falls to the removed area (3731 wide samples fall to 0 on idx 1); the edge term needs the engaged edge, which the arc still gives |
| The engagement door | `feeds/engagement.rs` `Engagement::from_sample` | as above; builds the `Rectangle` section | yes | the one door every power-type reader goes through |
| Deflection gate | `tool_load/deflection.rs` `sample_engagement` | ap = `axial_engagement_mm` (the peak), ae from the arc | yes | the peak engaged-edge force falls with the area; the force centre height reads the depth |
| Depth gate, force budget | `tool_load/depth.rs` (about line 255) | ae from `from_sample` | yes | the budget's depth at the measured width rises |
| Envelope placement (P20i) | `compute/simulate.rs` `envelope_placement` -> `MeasuredPoint` (ap, ae) and `gate_sample_power` | the gate's power and the sample's (ap, ae) | yes | the measured point moves left on the width axis; `project.sim_outside_envelope` counts fall |
| Modulator, per-sample caps (P20g) | `dressup/feed_modulation.rs` `SampleEngagement::of_sample` | the power and the deflection engagements of each sample | yes | the power, deflection and force caps of a move rise (fewer slowed moves) |
| Chip-thinning lift | `tool_load/display.rs` `sample_thinning_lift` | the arc width on the engaged diameter | yes, but the chip needs the arc | **keep the arc**: a flute in a thin full-front layer cuts a full chip, so the arc is the right input for the chip (the chip and chipload gates) |
| P22a readout, P22f-2a sentry | `tests/every_wide_step_has_a_named_source_p22a.rs`, `tests/a_geodesic_arc_end_reads_the_stock_p22f2.rs`, `tests/common/wide_samples.rs` | the rectangle width; the removal ratio | yes | the readout now prints both; the sentry still reads the rectangle and the removal ratio with end samples (P22f-2c side finding) |

`rs_cam_viz` has no `from_sample` caller (rg). The MCP cut-trace payload carries
`radial_woc_fraction` and the arc; it judges nothing.

### 2.2 The acceptance table

`planning/feeds_contract_2026-11-03/acceptance/wanaka_21.md` grades Suggest and shipped values
(chip, rpm, feed, plunge, ramp feed, and the modelled power at the app point, read through MCP
`get_suggest_rationale` and `get_toolpath_params`). No row reads a simulated sample, so no row
moves under any option below. The post-simulation numbers that would move are not in that table:
the power verdict and peak of `get_tool_load_report`, the deflection peak, the envelope finding
count (`project.sim_outside_envelope`), the modulated feeds (and the G-code F words), and the FM1
sim cells (`power_kw`, `deflection_peak_mm` of the sim subset).

### 2.3 Three options

| Option | Mechanics | Cost | What moves |
|---|---|---|---|
| (A) The gate reads `section / ap` as its ae | `Engagement::from_sample` builds the rectangle from `ae = section / ap` (psi then from that ae), section from the sample's removal | smallest code (one door); but the chip-thinning reader and the edge term would read a width the arc does not show, so a thin full-front layer reads a thin chip (a feed lift on a full chip); the per-sample removal noise (end samples, impossible reads) enters every gate | every reader of the door, the chip ones included (wrongly) |
| (B) `EngagedSection::Measured { area_mm2, depth_mm }` for simulated samples; the rectangle stays for planned cuts | `from_sample` builds `Measured` (area from the removal per mm, depth from the sample); `power_inputs` and `edge_force` read the area for the shear term and the arc (psi) times the depth for the edge term; the chip readers keep the arc; a per-move or mid-move removal filter in the door | medium: one variant through `feeds/section.rs`, `power_inputs`, `edge_force`, the deflection force centre; FM1 sim cells re-bless; sentries of the power and deflection gates read both | the sim-side power, deflection and depth verdicts, the envelope finding, the modulator caps, FM1 sim cells; no pre-sim number |
| (C) The rectangle stays; every finding also prints the section as a second line | add the section figure to the power, deflection and envelope findings and their MCP rows | small; no number moves | nothing is judged differently: the 3731 stay over the ceiling, the P20g modulator keeps slowing those moves |

### 2.4 Which the one-definition rule favours

The rule (one load and engagement definition codebase-wide, in `tool_load/` and `feeds/`; new
measures build on the sim engagement) favours **(B)**: the area is the planner's own load
(`tool_load::load_fraction::chip_load_fraction`, the area over `D dpp`), so the gate and the planner
then judge one quantity through one section type. (A) mixes two widths in one field and breaks the
chip readers. (C) keeps two definitions side by side, one judged and one printed.

## 4. Lead rulings (2026-11-10, night; the operator confirms in the morning)

1. **Option (B).** A simulated sample is judged by its measured section, one new variant of
   `EngagedSection` (`Measured { area_mm2, depth_mm }`), through the same power and force doors
   the rectangle uses. The rectangle stays for a planned cut (no sample exists). One definition:
   the area is the planner's own load quantity (`chip_load_fraction` = area / (D dpp)).
2. **The removal read** (open question 1): per sample, mid-move samples only; a section over
   `D x ap` clamps to `D x ap`; a move with fewer than three samples reads the volume of the move
   over its length. The filter lives in the one door (`Engagement::from_sample`), not in each gate.
3. **The edge term** (open question 2): the engaged edge length from the arc, the depth as the
   mean `area / ae_arc`; the section type states it.
4. **The chip readers keep the arc.** A flute in a thin full-front layer cuts a full chip; the
   chip and chipload gates and the chip-thinning lift read the arc as now.
5. **The sentries** (open question 3): after (B) lands, the P22f-2a sentry and the P22a readout
   read the section through `common::wide_samples`; until then the sentry's rectangle claim stays.
6. **The phase plan.** (B) is **P22f-2e** and waits for the operator's Go: it moves every
   simulated power and deflection verdict, the envelope finding count, the P20g modulated feeds
   (and so the G-code F words) and the FM1 sim cells, while no acceptance-table row moves.
   P22f-2a stays (a correct planner-side rule). P22f-3 is dropped (no real wide class by section).
   P22f-2b waits for one section read of ops 20 and 21 after P22f-2e; the four region-levels that
   still sweep are a full slot by geometry, so they may stay real.
7. **The one finding to carry forward**: on wanaka idx 1 the app today slows moves that are not
   heavy. The P20g modulator and the P20i envelope read a rectangle that is 8 to 25 times the
   removed section at the band; the planner's own read was right.

## 3. Open questions

1. The per-sample removal is noisy (end samples; a few impossible reads inside a move). Does (B)
   read the removal per move (the volume of the move over its length), or per sample with the
   mid-move filter? Recommendation: per sample, mid-move, with the impossible reads clamped to
   `D x ap`; a move with fewer than three samples reads its mean.
2. The edge term of (B): the arc times the depth overstates the edge of a thin layer the same way
   the rectangle overstates its area. Recommendation: read the edge length from the arc and the
   mean depth `area / ae_arc`, and state it in the section type.
3. The P22f-2a sentry still reads the rectangle and the removal ratio with end samples.
   Recommendation: after the ruling on (A)/(B)/(C), move it to the section read through
   `common::wide_samples` (it already shares the classifier).

## 5. Operator ruling (2026-11-10 morning)

"If P22f is more correct, then 100 % do it." The Go on option (B), P22f-2e: the simulated
sample reads its measured section (`EngagedSection::Measured`) through the same power and force
doors; the planned cut keeps the rectangle; the chip readers keep the arc. Order: after P22b.
