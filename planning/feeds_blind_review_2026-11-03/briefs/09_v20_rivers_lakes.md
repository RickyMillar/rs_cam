# 09 V rivers and lakes, 20 deg V-bit with a 0.3 mm flat tip (op 9)

Op 9 "V rivers and lakes", setup 1. Group B.

## Tool: 3.175-20deg-0.3 V-bit (tool id 1, T2)

| Field | Value |
|---|---|
| Type | engraving V-bit, flat tip |
| Included angle | 20 deg (10 deg half angle) |
| Tip | 0.3 mm flat (rs_cam models a sharp tip; the generator puts the tip into the depth) |
| Body / shank | 3.175 mm (1/8 in); map.toml says "measure" the body |
| Cone length | 8.15 mm from the tip to the 3.175 mm body ((3.175 - 0.3) / 2 / tan 10 deg) |
| Usable cone | 7.65 mm (cone less 0.5 mm `body_margin`) |
| Flutes | 1 (map.toml v20) |
| Stick-out | 25 mm (ESTIMATE: "measure") |
| Material, coating | carbide; coating not known; vendor not stated |

Cutting width at depth d below the tip: w = 0.3 + 2 d tan 10 deg.

| Depth d (mm) | 0.5 | 1 | 2 | 3 | 4 | 5 | 6.42 | 7.65 |
|---|---|---|---|---|---|---|---|---|
| Width w (mm) | 0.48 | 0.65 | 1.00 | 1.36 | 1.71 | 2.06 | 2.56 | 3.00 |

## The cut

- Op kind: Project Curve, point projection, from below, side centre. The tip follows the front
  relief surface (seen from the back) at a fixed offset along every V20 line: 228 river paths and
  15 lake paths.
- Tip Z: the front surface - 2.418 mm (the overcut for a 0.85 mm line at the front, plus the
  1.0 mm exposure margin). One pass.
- Axial engagement: the V cuts from the channel floor (op 5 leaves `web_min` 4.0 mm of oak over the
  front surface) to the tip: nominal 6.42 mm of cone, at most 7.65 mm. Where the sheet under the
  6 mm disc slopes, the web over the line is thicker than 4.0 mm (ESTIMATE: the V then cuts deeper
  in the cone, up to the usable cone).
- Radial engagement: a V slot, full width at every depth; top width about 2.56 mm at 6.42 mm deep.
- Entry: no entry dressup; the tool descends at each path start. Retract at Z 31.0 (clearance
  plane), so each path start is a long Z move.
- Point spacing 0.25 mm: Z follows the surface at each vertex.
- Material: fresh oak (the web). On the lake outlines the lake pocket (ops 6-7) has already removed
  the inside, so one side of the V meets air.
- Air share: low on the rivers; about half the V section on the cut lake outlines (ESTIMATE).

## Machine limits that bind

- Spindle rpm max 24000: the effective diameter is small (0.3 mm at the tip, up to 2.56 mm at the
  top), so the cutting speed at the real diameter is low at any rpm.
- Z max feed 2500 mm/min and Z acceleration 550 mm/s^2: Z changes at every vertex.
- XY acceleration 1200 mm/s^2 on curved lines.

## Material and finish

- Highly visible: the V tip makes the river line at the front. The line width at the front is set
  by the tip depth past the surface; tool deflection or a wrong Z moves it.
- The groove fills with epoxy; tear-out at the groove edge shows through at the front (the front
  cut exposes the line 2.418 mm above the tip).
- A 1-flute 20 deg V with a 0.3 mm flat tip is fragile; the tip burns easily at low chip load.

## Questions for the reviewer

1. The V-bits: what chip load suits the real cutting diameter (a 0.3 mm flat tip, depths 2-5 mm,
   up to 6.42 mm here)?
2. At which diameter does the reviewer set the chip load: the tip, the mean, or the top width?
3. Is one pass of 6.42 mm (nominal) safe for a 20 deg 1-flute V, or does it need two depths?
4. What plunge rule suits the path starts?
