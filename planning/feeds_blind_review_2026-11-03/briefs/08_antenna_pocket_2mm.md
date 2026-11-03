# 08 Antenna pocket, DMM 2 mm 1-flute (op 8)

Op 8 "ANTENNA_D5.00 pocket 2 mm", setup 1. Group C.

## Tool: DMM DSP1f-D2.0-L08 (tool id 5, T6)

As in brief 07: 2.0 mm, 1 flute, 8 mm flute, shank 3.175 mm (ESTIMATE), stick-out 20 mm (ESTIMATE),
carbide, coating not known, centre cutting not stated.

## The cut

- Op kind: 2.5D pocket, contour pattern, climb, no finishing pass, region ordering by area.
- What it cuts: the ANTENNA_D5.00 pocket of the mech back (cam_ops.json): 4 mm wide, 5.0 mm deep
  from the top (the back face, Z 25.5, to Z 20.5).
- Radial engagement: stepover 1.0 mm (as planned). In a 4 mm wide pocket with a 2 mm tool the first
  ring is a full-width slot (100 %); the next is about 50 %.
- Axial engagement: depth per pass 0.5 mm (as planned), 10 levels.
- Entry: ramp at 3 deg (dressup), helix pitch 1.0 mm.
- Material: fresh oak. Op 2 (6 mm) cannot enter a 4 mm pocket, so little was removed before.
- Air share: about 0 % (ESTIMATE).

## Machine limits that bind

- Spindle rpm max 24000 (a 2 mm tool).
- XY acceleration 1200 mm/s^2 in a small pocket.

## Material and finish

- Not visible (a back pocket for the antenna). Size and a clean floor matter for the part fit.
- Slot-like cut in oak with a 1-flute 2 mm: chip packing and burning.

## Questions for the reviewer

1. The DMM 2 mm single flute at 20 mm stick-out in oak, in a near-slot: what chip load, rpm and
   feed?
2. What ramp angle suits a 2 mm 1-flute (centre cutting not confirmed)?
3. Is 0.5 mm per level right, or is a deeper level with a lower feed better for a 5 mm deep slot?
