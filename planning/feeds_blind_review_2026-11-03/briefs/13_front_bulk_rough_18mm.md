# 13 Front bulk rough, 18 mm bulk bit (op 14)

Op 14 "Front bulk rough 18 mm", setup 3 (front up). Group A.

## Tool: 18 mm 2-flute bulk bit (tool id 6, T42)

As in brief 01: 18.0 mm, 2 flutes, 20 mm flute, 6 mm shank, stick-out 25 mm (ESTIMATE), carbide,
coating not known. It does NOT cut at the centre: core 8 mm, blade protrusion 1 mm (both
ESTIMATES, measure).

## The cut

- Op kind: 3D Rough (`adaptive3d`), `geodesic_adaptive`, fresh stock, inside the EDGE_OUTER
  boundary (the relief edge band).
- What it cuts: the bulk of the front relief (terrain), 12.5 mm deep at most, in 3 even levels.
- Levels: depth per pass 4.1667 mm (as planned). A level cuts only where the surface is below it.
- Radial engagement: stepover 3.6 mm (as planned) = 0.2 x D.
- Axial engagement: up to 4.17 mm per level; less at the slopes.
- Stock to leave: 0.5 mm axial.
- Entry: ramp at 3 deg, helix radius factor 0.3, helix pitch 2.0 mm. The limit for this tool is
  atan(1/8) = 7.13 deg. Known finding (sim, 2026-11-02, before G-CENTRECUT): 175 descents over
  1 mm steeper than 7.13 deg. rs_cam now limits them; not re-measured here.
- Material: fresh oak. The lowest level can reach the epoxy where it lies more than 0.5 mm above
  the final surface: the V20 grooves (tips 2.418 mm past the surface), the lake pockets (2.0 mm
  past) and the light-hole tops (1.0 mm past).
- Air share: ESTIMATE moderate: the upper level cuts only the valleys; the mountains stand above it.
  Not measured for this op.

## Machine limits that bind

- Spindle power at the chosen rpm (see 00).
- rs_cam cutting-feed cap 6000 mm/min (software). XY acceleration 1200 mm/s^2 on adaptive arcs.
- Z max feed 2500 mm/min and Z acceleration 550 mm/s^2 on ramps.
- A 6 mm shank on an 18 mm head (ESTIMATE 25 mm stick-out).

## Material and finish

- Not the final surface (0.5 mm left); the 6 mm rough and the R1 passes follow.
- Epoxy in the lowest level: heat can smear it.

## Questions for the reviewer

1. The 18 mm 2-flute bulk bit: is a feed at the cap (about 6000 mm/min) right for up to 4.2 mm deep,
   3.6 mm wide in oak at the available power?
2. What plunge or ramp suits a bit that does not cut at the centre (core 8 mm, protrusion 1 mm)?
3. Does the feed need to drop where the cut meets epoxy?
