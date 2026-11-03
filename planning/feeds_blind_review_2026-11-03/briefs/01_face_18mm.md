# 01 Face back and face front, 18 mm bulk bit (ops 0 and 12)

Op 0 "Face back to T1" (setup 1) and op 12 "Face front to T" (setup 3) have the same tool and the
same parameters. Group A.

## Tool: 18 mm 2-flute bulk bit (tool id 6, T42)

| Field | Value |
|---|---|
| Type | flat end mill, large head on a small shank |
| Diameter | 18.0 mm |
| Flutes | 2 |
| Cutting length (flute) | 20 mm |
| Shank | 6 mm |
| Stick-out | 25 mm (ESTIMATE: flute length + 5, a guess in `my_roughing.toml`; measure) |
| Material, coating | carbide; coating not known |
| Helix | 30 deg in the file (a default, not a printed value) |
| Special | does NOT cut at the centre. Centre core 8 mm diameter, blades 1 mm below the core (both ESTIMATES: operator "around", measure) |
| Source | `~/.config/rs_cam/tools/my_roughing.toml` "18 mm 2F, 20 mm flute, 6 mm shank"; vendor not stated |

## The cut

- Op kind: face, zigzag rows, one pass.
- Radial engagement: stepover 14.4 mm (as planned) = 0.8 x D. Each row overlaps the last by 3.6 mm.
- Axial engagement: 0.5 mm (as planned), one pass.
  - Op 0: from the measured raw top Z 26.0 to T1 Z 25.5.
  - Op 12: from Z 25.5 to Z 25.0 (the raw front side).
- Area: the stock 395 x 460 mm, plus `stock_offset` 18 mm past each edge.
- Entry: ramp at 3 deg (dressup). The cut starts outside the stock, so most rows enter from air.
- Material: the raw sawn face of fresh white oak. Rows run in X, so the cut runs across the grain
  on one row direction and back on the next (zigzag = climb and conventional rows alternate).
- Air share: about 15 % (ESTIMATE: (395 x 460) / (431 x 496) of the swept area is stock); more if
  the board is not flat and a 0.5 mm pass misses low parts.
- Obstacles: op 0 keeps out of the four top clamps (40 mm along the edge, 10 mm onto the stock, at
  y 153 and 307). Op 12 crosses the five pin positions (the pins must be below Z 25.0) and the
  epoxy-filled day-strip slit line (about 0.4-1.0 mm wide at this depth, from the V60 geometry).

## Machine limits that bind

- Spindle power at the chosen rpm (see 00: 0.375 kW at 6000 rpm up to 1.5 kW at 24000 rpm).
- rs_cam cutting-feed cap 6000 mm/min (software); XY travel 15000 mm/min; XY acceleration
  1200 mm/s^2 at each row end of the zigzag.
- A 6 mm shank carries an 18 mm head: deflection and chatter are a concern.

## Material and finish

- The back face (op 0) is the datum face for setup 3: flatness matters more than finish.
- The front face (op 12) is mostly removed later by the front relief roughs; the border outside
  the relief stays visible.
- Raw sawn oak can hold grit; a 0.5 mm skim on a cupped board can leave unfaced spots.

## Questions for the reviewer

1. What chip load per tooth suits an 18 mm 2-flute bit in white oak at 0.8 x D radial and 0.5 mm
   axial? How much chip thinning applies at this engagement (it applies only under 50 % radial)?
2. With 0.5 mm axial the power is low: does the machine feed limit or the rpm range bind first?
3. What ramp or entry is safe for a bit that does not cut at its centre (core 8 mm, protrusion
   1 mm, both estimates), when a row must start inside the stock?
4. Is a feed near the rs_cam cap (about 6000 mm/min) right for this face?
