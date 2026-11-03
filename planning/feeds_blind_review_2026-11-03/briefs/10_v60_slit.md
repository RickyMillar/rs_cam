# 10 Day strip slit, 60 deg V-bit (op 10)

Op 10 "Day strip slit V60", setup 1. Group B.

## Tool: 1/4 in 60 deg V-bit (tool id 2, T3)

| Field | Value |
|---|---|
| Type | V-bit, sharp tip |
| Included angle | 60 deg (30 deg half angle) |
| Body / shank | 6.35 mm (1/4 in) |
| Cone length | 5.50 mm |
| Flutes | 2 |
| Stick-out | 30 mm (a SET value, "set and measure"; the tip must reach 26 mm below the back face: ESTIMATE) |
| Material, coating | carbide; coating not known; vendor not stated |

Cutting width at depth d: w = 2 d tan 30 deg. At 0.6 mm: 0.69 mm; at 2.598 mm: 3.00 mm;
at 5.196 mm: 6.00 mm.

## The cut

- Op kind: trace, no compensation, along the slit line (VGROOVE_V60_D25.866 of the mech back).
- Depth: 5.196 mm in two passes of 2.598 mm (as planned). Top Z 4.83 = the slit slot floor
  (block z 20.67); the tip ends 0.366 mm into the spoilboard. The slit is open underneath (it is
  sealed before the pour).
- Radial engagement: a V slot, full width at every depth; 3.0 mm wide after pass 1, 6.0 mm after
  pass 2.
- Entry: no entry dressup; a plunge at the line start.
- Material: fresh oak below the slit slot floor, then the spoilboard in the last 0.366 mm.
- Air share: ESTIMATE low, if the slit slot above is open. Known finding (sim, 2026-11-02): 49.5 %
  of samples bite more than the commanded 2.6 mm, peak 5.27 mm. Open: does op 2 cut the slit slot
  floor (a 7 mm slot to Z 4.83) before this op? If not, the V meets more wood than planned.

## Machine limits that bind

- Spindle rpm range 6000-24000; the effective diameter runs from 0 at the tip to 6 mm.
- Z max feed 2500 mm/min for the plunge.

## Material and finish

- Visible: the slit makes the day-strip line through the block (filled with epoxy). Clean V walls
  matter; tear-out at the exit face (the front, on the spoilboard) shows.
- The exit through the bottom face into the spoilboard can break out fibres at the front edge.

## Questions for the reviewer

1. The V-bits: what chip load suits the real cutting diameter (sharp tip, depths 2.6 and 5.2 mm)?
2. If the slot above is not cleared, the bite is up to 5.27 mm: what feed is safe then?
3. Two passes of 2.6 mm, or three shallower passes for a clean exit?
