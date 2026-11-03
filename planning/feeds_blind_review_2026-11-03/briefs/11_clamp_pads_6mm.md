# 11 Face clamp pads, 6 mm flat end mill (op 11)

Op 11 "Face clamp pads 6 mm", setup 1b (pins in, top clamps off, side pushers hold). Group A.

## Tool: 6 mm flat end mill (tool id 0, T1)

As in brief 03: 6.0 mm, 2 flutes, 25 mm cutting length, 6 mm shank, stick-out 30 mm (a set value),
carbide, coating not known.

## The cut

- Op kind: 2.5D pocket, contour pattern, climb, no finishing pass, ordering by area.
- What it cuts: the four pads that op 0 left under the top clamps. Each pad is the clamp keep-out
  (40 mm along the edge, 10 mm onto the stock) grown by 10 mm (1 mm clearance + the 18 mm face
  radius): about 60 x 50 mm, of which 20 mm is on the stock and 30 mm is past the stock edge.
- Depth: from the measured raw top Z 26.0 to T1 Z 25.5: 0.5 mm, one pass (as planned).
- Radial engagement: stepover 2.4 mm (as planned) = 0.4 x D.
- Entry: none (no ramp); the pocket plunges 0.5 mm, or starts in air past the stock edge.
- Material: the raw sawn face of fresh oak.
- Air share: about 60 % (ESTIMATE: 30 of the 50 mm pad width is past the stock edge).
- Workholding: the top clamps are off; only the side pushers and the pins hold the stock.

## Machine limits that bind

- Low power (0.5 mm deep).
- XY acceleration 1200 mm/s^2 on small rings.
- rs_cam cutting-feed cap 6000 mm/min (software).

## Material and finish

- The pads become the contact faces of the flipped back: flatness and a match with the op-0 face
  matter. The pad edges are not visible in the end.
- Lower holding force: the cut force must not move the stock.

## Questions for the reviewer

1. What feed and rpm suit a 6 mm 2-flute at 0.4 x D radial and 0.5 mm axial in oak?
2. Does the weaker workholding (pushers only) set a lower limit than the tool?
