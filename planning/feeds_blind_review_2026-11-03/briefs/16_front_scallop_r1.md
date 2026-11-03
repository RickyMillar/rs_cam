# 16 Front scallop finish, R1 tapered ball (op 17)

Op 17 "Front scallop finish", setup 3. Group C.

## Tool: R1 x 6 x 20 mm 2-flute tapered ball (tool id 4, T5)

As in brief 15: R1.0 ball tip, taper half angle 5.71 deg (ESTIMATE), 20 mm flute, 6 mm shank,
2 flutes, stick-out 30 mm (ESTIMATE), carbide, coating not known. Possible match: endmill.com.au
TBN-R1.0-6-20.

## The cut

- Op kind: scallop finish (constant scallop height), direction outside in, not continuous,
  iso-field on, slopes 0-90 deg, inside the EDGE_OUTER boundary.
- Scallop height 0.1 mm (as planned), tolerance 0.05 mm. On a flat area that gives a step of
  2 sqrt(2 x 1 x 0.1 - 0.1^2) = 0.87 mm between passes; on slopes the step changes.
- Axial engagement: nominal 0.5 mm stock (left by ops 15 and 16). Cut diameter at 0.5 mm deep on a
  flat: 1.73 mm. On a flat area the ball centre (zero cutting speed) touches the surface.
- Known finding (sim, 2026-11-02, before op 16 was added): the finish crossed standing material,
  13.7 % of samples over 1.08 mm, peak 5.39 mm. Op 16 is meant to remove that; not re-measured.
- Entry: lead-in and lead-out arcs (lead radius 2.0 mm). Intra-pass hookup 3.0 mm.
- Segment merge 0.02 mm: about 466 k moves (a measured count).
- Material: oak, with epoxy lines at the V grooves, the lake pockets and the light-hole tops. This is
  the pass that cuts the visible oak/epoxy boundary.
- Air share: about 0 % on the relief (ESTIMATE); links and lead arcs only.

## Machine limits that bind

- XY acceleration 1200 mm/s^2 and junction deviation 0.1 mm: the op was measured "accel-bound"
  (2026-11-01): 466 k short moves do not reach the commanded feed.
- Spindle rpm max 24000 with a 1.7-2 mm cut diameter.

## Material and finish

- The final visible surface of the terrain. Finish matters most here.
- Oak: fuzz and tear-out on slopes that run across the grain; burn marks where the feed drops.
- Epoxy: smear, chipping and a raised edge at the oak/epoxy boundary.
- Ball centre on flats: rubbing, not cutting.

## Questions for the reviewer

1. What chip load suits a 2-flute R1 tapered ball at 0.5 mm deep (cut diameter about 1.73 mm) in
   oak and epoxy?
2. Since acceleration limits the real feed, what commanded feed is useful, and what rpm keeps the
   chip load right at the real feed?
3. What load is safe if the ball still meets a deep bite (up to about 5 mm)?
