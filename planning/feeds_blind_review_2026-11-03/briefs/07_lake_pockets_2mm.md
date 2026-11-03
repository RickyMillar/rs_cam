# 07 Lake pockets, DMM 2 mm 1-flute (ops 6 and 7)

Op 6 "Lake pocket 2 mm, 8 lakes to Z 5.91" and op 7 "Lake pocket 2 mm, 1 lake to Z 3.99",
setup 1. Same tool and parameters; only the floor and the top differ. Group C.

## Tool: DMM DSP1f-D2.0-L08 (tool id 5, T6)

| Field | Value |
|---|---|
| Type | flat end mill, single flute |
| Diameter | 2.0 mm |
| Flutes | 1 |
| Cutting length (flute) | 8 mm |
| Shank | 3.175 mm (ESTIMATE: "assumed: measure") |
| Stick-out | 20 mm (project value; not stated as measured: treat as ESTIMATE) |
| Material, coating | carbide; coating not known |
| Centre cutting | not stated (find out from the DMM data) |
| Vendor | DMM, product code DSP1f-D2.0-L08 |

## The cut

- Op kind: 2.5D pocket, contour pattern (offset rings), climb, one finishing contour, region
  ordering global.
- What it cuts: the closed lakes wider than the tool, as flat-floor pockets with vertical walls.
  9 of 15 lakes are cut (8 in op 6, 1 in op 7); the narrow ones stay V-only.
- Floors: op 6 Z 5.91; op 7 Z 3.99 (the deepest lake of the group - 2 x exposure margin 1.0).
- Tops: op 6 Z 14.46 (wall 8.55 mm); op 7 Z 11.38 (wall 7.39 mm). The top includes one 1.2 mm rest
  step of air above the shore hollow. The wall in wood stays within the flute less 0.5 mm
  (7.5 mm).
- Radial engagement: stepover 1.0 mm (as planned) = 0.5 x D. The first ring of each level is a
  full-width slot (100 %).
- Axial engagement: depth per pass 0.5 mm (as planned) = 0.25 x D. About 18 levels (op 6) and 15
  levels (op 7).
- Entry: plunge (no ramp). A ramp entry dressup caused 21 rapid collisions (rs_cam defect
  G-POCKETRAMP), so the operator uses the plain plunge. Each level plunges 0.5 mm.
- Retract: Z 31.0 (clearance plane).
- Material: oak. Op 2 (6 mm) already opened each lake where its 6 mm disc fits, down to 1.0 mm
  past the front surface. The pocket goes one more 1.0 mm deeper and flat to the deepest lake of
  the group. So the upper levels are mostly air or thin rest stock at the walls; the lowest levels
  are full rings in fresh oak.
- Air share: ESTIMATE high on the upper levels (wide lakes already open), low on the last 1-2 mm.

## Machine limits that bind

- Spindle rpm max 24000: a 2 mm tool wants high rpm; the top of the range binds.
- XY acceleration 1200 mm/s^2 on small rings: the real feed stays under the commanded feed.
- Z max feed 2500 mm/min for the plunges.

## Material and finish

- Visible: the lake wall is the lake outline at the front. A clean, vertical wall with no fuzz
  matters. One finishing contour cleans it.
- The floor is front waste (the front cut removes it), then epoxy.
- A 1-flute 2 mm tool at 20 mm stick-out in oak: deflection, burning at low chip load, breakage at
  high chip load.

## Questions for the reviewer

1. The DMM 2 mm single flute at 20 mm stick-out in oak: what chip load, rpm and feed?
2. A full-width first ring at 0.5 mm deep, then 50 % rings: one feed for both, or a lower one for
   the slot?
3. Is a straight 0.5 mm plunge per level safe for this tool (centre cutting not confirmed)?
4. What depth x stepover pairs suit the 8 mm flute and a 7.5 mm wall (for example deeper levels
   and a narrower step)?
