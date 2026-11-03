# 05 Drill light holes, 6 mm flat end mill (op 4)

Op 4 "Drill light holes 6 mm end mill", setup 1. Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value),
carbide, coating not known. Centre cutting: assumed yes (not stated).

## The cut

- Op kind: drill, peck cycle, with a flat end mill (flat-bottom holes).
- Holes: 42 light holes (layers HOLE_FLAT_3_HELIX_Z15.00 / Z16.00 / Z17.00). The drawn diameter is
  5.81 mm; the 6 mm tool cuts 6.0 mm (`size_tolerance` 0.2 accepts the difference).
- Depth: from the cavity floor (top Z 16.5) to Z 8.5 = 8.0 mm per hole.
- Peck depth 6.0 mm (as planned), dwell 0.5 s, retract amount 0.5 mm, retract Z 2.0.
- Material: fresh oak under the cavity floor. Whether op 2 already opened part of a hole is not
  known (holes near the lakes may sit in the hollow band).
- Air share: about 0 % (ESTIMATE), except holes that start in the hollow band.
- After the pour the holes hold epoxy; the front cut opens their tops (1.0 mm past the front
  surface).

## Machine limits that bind

- Z max feed 2500 mm/min, Z acceleration 550 mm/s^2.
- 42 holes x 2 pecks: Z moves dominate the op time.

## Material and finish

- The hole walls are behind epoxy: not visible as wood. A clean flat bottom matters for light.
- Dwell at the bottom of an end-mill plunge in oak can burn it.

## Questions for the reviewer

1. What plunge feed and rpm suit a 6 mm 2-flute end mill plunging 8 mm in oak?
2. Is the 0.5 s dwell useful for a flat-bottom hole, or does it only burn?
3. Is one peck at 6 mm then 2 mm right, or one full plunge?
