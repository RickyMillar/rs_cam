# 06 River channel, 6 mm flat end mill, Project Curve slot (op 5)

Op 5 "River channel 6 mm, pass 7 of 7", setup 1. Group A. The name says "pass 7 of 7", but the
project holds only this one pass: one full-depth pass.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value),
carbide, coating not known. Centre cutting: assumed yes (not stated).

## The cut

- Op kind: Project Curve, cutter projection, from above, side centre. The tool centre follows every
  V20 line (rivers and lake outlines: 228 river paths, 15 lake paths in block.json).
- Purpose: a channel for the V-bit. The tip Z = the highest relief-sheet point under the 6 mm disc
  + `web_min` 4.0 mm. The V20 then cuts the last 4.0 mm of web and the overcut.
- Radial engagement: 100 % (a full-width 6 mm slot). Project Curve has no depth per pass and no
  stepover.
- Axial engagement: one pass, varying along the path, below the cavity floor (Z 16.5, the op top):
  - deepest point 10.02 mm below the cavity floor (generator, "under 2 x D");
  - most of the channel 0-2.5 mm below the floor in the valleys; 4.5-5.5 mm near the lakes
    (`planning/rivmap_block_job.md`, 2026-11-02 night).
- Entry: no entry dressup. The tool descends at each path start to the path Z (a plunge, up to the
  local channel depth). Retract at Z 31.0 (clearance plane above the raw top).
- Point spacing 0.25 mm: Z changes at each vertex, so the cut is many short 3D segments.
- Material: fresh oak under the cavity floor. Near the lakes the path enters the hollow band that
  op 2 already cut (air).
- Air share: ESTIMATE, not measured. Where the planned tip is above the cavity floor the tool
  cuts air; where the channel is shallow (0-2.5 mm) the slot is shallow. A large share of the
  length is shallow or air.

## Machine limits that bind

- Spindle power for a full slot up to 10 mm deep (see 00).
- Z max feed 2500 mm/min and Z acceleration 550 mm/s^2: the path changes Z at every 0.25 mm vertex.
- XY acceleration 1200 mm/s^2 on the curved river lines.

## Material and finish

- Not visible: the channel fills with epoxy. Its floor sets the web the V cuts; a gouge or a
  deflected wall changes the V width at the front.
- A full-width slot in oak packs chips and burns; the river lines run in every direction to the
  grain.

## Questions for the reviewer

1. The river channel: a 6 mm full-width slot up to about 10 mm deep in oak in one pass. What feed
   and rpm suit it, and is one pass safe at the deepest point?
2. Should the feed follow the local depth (shallow most of the length, deep in a few places), or
   should the op get depth levels?
3. What plunge rule suits the path starts, where the tool descends straight into the channel
   depth?
