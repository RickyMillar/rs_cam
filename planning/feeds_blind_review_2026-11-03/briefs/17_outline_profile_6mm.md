# 17 Outline profile with tabs, 6 mm flat end mill (op 18)

Op 18 "Outline profile with tabs", setup 3 (last op). Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value),
carbide, coating not known. Up-cut.

## The cut

- Op kind: 2.5D profile, outside the BLOCK_OUTLINE (358 x 443 mm), climb, compensation in the
  computer, no finishing pass.
- Depth: 25.0 mm, from Z 25.0 to the spoilboard (Z 0), no penetration into the spoilboard.
- Axial engagement: depth per pass 1.5 mm (as planned), about 17 levels.
- Radial engagement: 100 % (a full-width 6 mm slot on every level).
- Tabs: 8, each 6 mm wide and 2 mm tall (rs_cam defaults).
- Entry: ramp at 3 deg (dressup), helix pitch 1.0 mm.
- Material: fresh oak. The outline edges at Y min and Y max are end grain (the grain runs along Y).
  The flute (25 mm) equals the full depth at the last level.
- Air share: about 0 % (ESTIMATE).

## Machine limits that bind

- Spindle power for a full slot (see 00).
- XY acceleration 1200 mm/s^2 at the four corners only; long straight sides.
- rs_cam cutting-feed cap 6000 mm/min (software).

## Material and finish

- Visible: the block sides. Burning on long sides in a full slot, tear-out and break-out at the
  end-grain edges and at the top edge (an up-cut tool lifts fibres at the top face).
- A 25 mm deep, 6 mm wide slot packs chips; no coolant.
- The part breaks free at the tabs; the tabs hold it while the last levels cut.

## Questions for the reviewer

1. What feed and rpm suit a 6 mm 2-flute full-width slot in oak at 1.5 mm per level?
2. Which depth x feed pairs are better (for example deeper levels at a lower feed), and does the
   flute length allow them?
3. Is a finishing pass with a light radial cut worth it for the visible sides?
