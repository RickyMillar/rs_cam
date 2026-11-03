# 14 Front 3D rough (rest of the 18 mm), 6 mm flat end mill (op 15)

Op 15 "Front 3D rough", setup 3. Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value),
carbide, coating not known. (The project also holds a 1/4 in 2-flute flat, tool id 3, named "the
front rough"; no enabled op uses it.)

## The cut

- Op kind: 3D Rough (`adaptive3d`), `geodesic_adaptive`, from remaining stock (after op 14).
- What it cuts: the rest of the front relief after the 18 mm bulk: the narrow valleys and the
  18 mm terraces. Boundary EDGE_OUTER with the tool centre on it.
- Radial engagement: stepover 1.382 mm (as planned) = 0.23 x D. The operator's GUI value.
- Axial engagement: depth per pass 5.532 mm (as planned). The operator's GUI value. Its terraces on
  every slope are the main source of the deep bites the scallop finish meets later.
- Stock to leave: 0.5 mm axial.
- Entry: ramp at 3 deg, helix radius factor 0.3, helix pitch 2.0 mm.
- Material: oak rest stock, and epoxy where it lies above the surface + 0.5 mm (V grooves, lakes,
  light-hole tops; see brief 13).
- Air share: ESTIMATE high (a rest op after the bulk); not measured for this op.
- Known finding (sim, 2026-11-02, still open): an entry cuts up to 4.62 mm of standing wood at
  (168.9, 338.9, Z 20.38).

## Machine limits that bind

- XY acceleration 1200 mm/s^2 on adaptive arcs.
- Spindle power at 5.5 mm deep x 1.4 mm wide (see 00).
- Z max feed 2500 mm/min on ramps.
- rs_cam cutting-feed cap 6000 mm/min (software).

## Material and finish

- Not the final surface (0.5 mm left). The terrace height sets what the finish ball meets.
- Epoxy and oak in one cut: smear and edge chipping.

## Questions for the reviewer

1. What feed and rpm suit 1.38 mm radial x 5.53 mm axial (23 % radial, chip thinning) for a 6 mm
   2-flute in oak?
2. Is a smaller depth per pass better here (smaller terraces for the finish ball), and what feed
   goes with it?
3. What load is safe for the 4.6 mm entry bite if it stays?
