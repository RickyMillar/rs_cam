# 02 Back bulk rough, 18 mm bulk bit (op 1)

Op 1 "Back bulk rough 18 mm", setup 1 (back up). Group A.

## Tool: 18 mm 2-flute bulk bit (tool id 6, T42)

Same tool as brief 01: 18.0 mm, 2 flutes, 20 mm flute, 6 mm shank, stick-out 25 mm (ESTIMATE),
carbide, coating not known. It does NOT cut at the centre: core 8 mm, blade protrusion 1 mm (both
ESTIMATES, measure). rs_cam limits the plunges and descents of this tool from these two values
(G-CENTRECUT).

## The cut

- Op kind: 3D Rough (`adaptive3d`), strategy `geodesic_adaptive` (constant-engagement adaptive
  clearing), engagement measure `DiskArea`.
- What it cuts: the bulk of the back target. The target is the mech block back (the cavity, the
  mech pockets and the through holes) plus a hollow band around the lakes. The cavity is
  309 x 309 mm with 3 mm corner radii, 9 mm deep (floor at setup-1 Z 16.5).
- Levels: depth per pass 9.0 mm (as planned) = T1 25.5 to the cavity floor 16.5; the second level
  stops at `bottom_z` 11.0 (the board-pocket floor), so it cuts 5.5 mm, only in the board pocket.
  No flat-area levels.
- Radial engagement: stepover 3.6 mm (as planned) = 0.2 x D (the adaptive target width).
- Axial engagement: 9.0 mm on level 1, 5.5 mm on level 2; flute 20 mm.
- Stock to leave: 0.0 axial; the 6 mm rest (op 2) follows.
- Entry: ramp at 3 deg, helix radius factor 0.3 (x D), helix pitch 2.0 mm, entry clearance 0.5 mm.
  rs_cam must keep each descent at or under atan(protrusion / core) = atan(1/8) = 7.13 deg.
- Material: fresh white oak (the back face is already faced). The cavity first level is open
  pocket clearing in solid wood; the walls run in both grain directions.
- Air share: low on level 1 (ESTIMATE: the adaptive clears solid wood inside the cavity; links and
  the region edges are the air). Level 2 is a small region.
- Known finding (sim, 2026-11-02, before the core model): the op emitted 89 straight plunges of
  more than 3 mm into wood. G-CENTRECUT (rs_cam c0f5d2567) now limits the entries; this is not
  re-measured here.
- Known finding (sim, 2026-11-02): the peak power of this op exceeded the power available at the
  planned spindle speed on the power curve.

## Machine limits that bind

- Spindle power: full 9 mm depth with an 18 mm cutter. See 00 for power at rpm
  (0.375 kW at 6000 rpm to 1.5 kW at 24000 rpm). This op is the most likely to hit the power curve.
- rs_cam cutting-feed cap 6000 mm/min (software). XY acceleration 1200 mm/s^2: the adaptive has
  many short arcs, so the real feed may stay under the commanded feed.
- Z max feed 2500 mm/min and Z acceleration 550 mm/s^2 on the ramps.
- A 6 mm shank on an 18 mm head at an estimated 25 mm stick-out: deflection and chatter.

## Material and finish

- Not visible: the cavity fills with epoxy. Wall and floor finish do not matter. The 6 mm rest
  cleans the corners.
- Chip evacuation from a 9 mm deep pocket with an up-cut 2-flute bit; no coolant.
- Burning at the ramps if the feed per tooth is low.

## Questions for the reviewer

1. The 18 mm 2-flute bulk bit: is a feed at the cap (about 6000 mm/min) right for 9 mm deep,
   3.6 mm wide in oak at the available power?
2. What plunge or ramp suits a bit that does not cut at the centre (core 8 mm, protrusion 1 mm,
   both estimates)? Is 3 deg under the atan(1/8) = 7.13 deg limit enough margin, given the
   estimates?
3. Which depth x stepover pairs (deep and narrow against shallow and wide) stay under the power
   curve, and what feed goes with each?
4. Does the 6 mm shank or the power limit this tool first?
