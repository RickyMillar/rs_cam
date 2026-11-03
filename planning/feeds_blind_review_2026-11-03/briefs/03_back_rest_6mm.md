# 03 Back rest, 6 mm flat end mill (op 2)

Op 2 "Back rest 6 mm (cavity, hollow, lakes, pockets)", setup 1. Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

| Field | Value |
|---|---|
| Type | flat end mill, up-cut |
| Diameter | 6.0 mm |
| Flutes | 2 |
| Cutting length | 25 mm (operator, 2026-11-02) |
| Shank | 6 mm |
| Stick-out | 30 mm (a SET value: the pin depth needs 30 mm reach; not stated as measured) |
| Material, coating | carbide; coating not known; vendor not stated |
| Centre cutting | assumed yes (not stated) |

## The cut

- Op kind: 3D Rough (`adaptive3d`), `geodesic_adaptive`, from remaining stock (after op 1).
- What it cuts: the rest after the 18 mm bit: the cavity corners (3 mm radius), the terraces, the
  narrow parts, the mech pockets, the through holes, and the hollow band around the lakes. Inside
  the lake outlines the target goes 1.0 mm past the front surface (the lakes open from the back).
- Radial engagement: stepover 1.2 mm (as planned) = 0.2 x D.
- Axial engagement: depth per pass 1.2 mm (as planned). The generator sets this from the V20 budget:
  a terrace step must not take the V past its cone.
- Stock to leave: 0.0. Region filter: `min_region_cut_length_mm` 15.
- Entry: ramp at 3 deg, helix radius factor 0.3, helix pitch 2.0 mm.
- Material: fresh oak in the corners and the narrow parts; the 18 mm terraces elsewhere.
- Air share: about 62-63 % (MEASURED in an earlier variant with a full hollow, 2026-11-01:
  "17 levels of 1.2 mm (63 % air)"). The current "trace" hollow is smaller; the share now is an
  ESTIMATE, likely similar or higher.
- Known finding (sim, 2026-11-02, still open): an entry cuts up to 14.5 mm of standing wood at
  (211.7, 41.6, Z 11.0). The target there is a pocket floor at Z 11.0 beside a wall up to Z 25.5;
  the 18 mm levels do not take it. Links cross up to 13.05 mm (2.7 % of samples over the
  commanded 1.2 mm).

## Machine limits that bind

- XY acceleration 1200 mm/s^2: short adaptive arcs and many levels; the real feed stays under the
  commanded feed in the corners.
- Z max feed 2500 mm/min, Z acceleration 550 mm/s^2 on the many ramps.
- Spindle power is low at 1.2 x 1.2 mm, except at the deep entry above (14.5 mm).
- rs_cam cutting-feed cap 6000 mm/min (software).

## Material and finish

- Not visible: the cavity fills with epoxy. The hollow must keep `web_min` 4.0 mm of oak under the
  front surface; a gouge thins the web.
- Narrow slots and corners: chip packing and burning in oak.

## Questions for the reviewer

1. What feed and rpm suit 1.2 mm radial x 1.2 mm axial (20 % radial, chip thinning) for a 6 mm
   2-flute in oak?
2. Is a deeper axial x narrower radial pair better for this rest (fewer levels, less air)?
3. What load is safe for the 14.5 mm deep entry and the 13 mm link crossings if they stay?
4. What ramp angle and helix suit the 6 mm in oak?
