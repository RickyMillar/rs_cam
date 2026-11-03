# 04 Pin drill, flip pins, 6 mm flat end mill (op 3)

Op 3 "Pin drill, flip pins", setup 1. Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value,
not stated as measured), carbide, coating not known. Centre cutting: assumed yes (not stated).

## The cut

- Op kind: `alignment_pin_drill`, peck cycle. The 6 mm end mill plunges a 6 mm hole (tool diameter
  = hole diameter, no interpolation).
- Holes: 5 pins (3 on the left long edge at y 190 / 230 / 270, 2 on the right at y 110 / 350), in
  the stock margin outside the block outline.
- Depth: through the 25.5 mm stock plus 4.5 mm into the spoilboard = 30 mm total (block.json
  `cam.pin_depth` 30.0 = the flat_6 reach).
- Peck depth: 6.0 mm (as planned); retract Z 2.0.
- Material: fresh oak, then the spoilboard (material not stated). The plunge is along Z, across
  the fibres (the grain runs along Y).
- Air share: none during the plunge.
- Geometry concern: at 30 mm depth with a 30 mm stick-out the collet nut reaches the stock top.
  The flute is 25 mm, so the top 5 mm of the hole sees the shank.

## Machine limits that bind

- Z max feed 2500 mm/min, Z acceleration 550 mm/s^2 (peck retracts).
- Spindle rpm range 6000-24000.

## Material and finish

- The hole fits a 6 mm pin: size and a clean wall matter for the flip accuracy. Heat in a deep
  end-mill plunge can burn the wall and swell the fit.
- Chips must leave a 30 mm deep, 6 mm hole through a 2-flute up-cut; peck retracts clear them.

## Questions for the reviewer

1. What plunge feed and rpm suit a 6 mm 2-flute flat end mill plunging 30 mm in oak and a
   spoilboard?
2. Is a 6 mm peck right, or should the peck be shorter as the hole gets deeper?
3. Is the 30 mm stick-out at 30 mm depth safe (collet nut at the stock top)?
