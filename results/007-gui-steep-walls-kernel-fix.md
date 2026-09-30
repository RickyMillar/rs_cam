---
id: 007
state: running
commit: fd06f407
started: 2026-09-30T22:45:00Z
finished:
---
## Summary (interim; Ricky may run a 0.25 mm aliasing check)

rivmap350 in the release GUI at fd06f407 (MemoryMax=16G). MCP
generate_all at 0.2 mm: 7 generated, 0 errors, 8 steps, 1 simulation.
Ricky then clicked Simulate at 0.2 mm, metrics OFF, resolution unchanged.

Ricky's verdict: "looks better I think... might just be the coarse sim
though. I still see something. Not sure if it's the mesh." The ramped
wall (software-generated, should be smooth) shows a slight "almost
wrinkled" texture. Screenshot: results/007-walls-0.2mm.png.

Runner analysis (hypothesis, not proven): aliasing / moire between the
scallop rings and the 0.2 mm dexel grid. Scallop R1.0 ball, scallop
height 0.1 -> ring pitch along the surface 2*sqrt(2Rh - h^2) = 0.872 mm;
horizontal pitch on a wall of angle a is 0.872*cos(a): 45 deg 0.62,
60 deg 0.44, 70 deg 0.30, 80 deg 0.15 mm, i.e. near the 0.2 mm cell on
steep walls. The screenshot shows even horizontal lines up the ramp and a
wavy band at the wall base, which fits a moire. Discriminator: re-sim at
0.25 mm; a moire changes spacing/direction, a path defect stays.

Numbers:
- Rapid collisions: 8 (as in 006; all in "3D Rough 8" there).
- Cycle time "7:41:30 (cutting only, no accel)": metrics OFF, so no trace
  and no per-op time or deviation (see 006 addendum 3).
- Peak RSS: generate_all 8.06 GiB (the closing sim result); Simulate
  8.6 -> 14.64 GiB peak at 22:54:52 UTC, then 8.94 GiB at rest. The sim
  took ~25 s.

Also seen: drill holes render as solid brown pillars standing in the
stock (display defect; Ricky: "we should fix this one day").
