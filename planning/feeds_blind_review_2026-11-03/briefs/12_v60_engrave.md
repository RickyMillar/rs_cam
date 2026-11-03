# 12 Engrave day labels, 60 deg V-bit (op 13)

Op 13 "Engrave day labels V60", setup 3 (front up). Group B.

## Tool: 1/4 in 60 deg V-bit (tool id 2, T3)

As in brief 10: 60 deg included, sharp tip, 6.35 mm body and shank, 2 flutes, cone 5.50 mm,
stick-out 30 mm (a set value: ESTIMATE), carbide, coating not known.

## The cut

- Op kind: trace, no compensation, single-stroke text: 49 open strokes, 41 of them with only two
  points (the generator adds a mid point). The labels are in the chin under the slit.
- Depth: 0.6 mm in one pass (as planned), from block z 25.0 to 24.4 (after the front face).
- Radial engagement: a V slot; 0.69 mm wide at 0.6 mm deep.
- Entry: no entry dressup; each stroke starts with a 0.6 mm plunge. Many short strokes, so many
  plunges and retracts.
- Material: the freshly faced front oak.
- Air share: about 0 % (ESTIMATE).

## Machine limits that bind

- Effective diameter at most 0.69 mm: the cutting speed is low at any rpm up to 24000.
- Z max feed 2500 mm/min and Z acceleration 550 mm/s^2: the short strokes are Z-bound.
- XY acceleration 1200 mm/s^2: short strokes never reach the commanded feed.

## Material and finish

- Visible text on the front. Crisp edges matter; oak pores and rays tear out on a shallow V.
- A sharp 60 deg tip at 0.6 mm depth burns at low chip load.

## Questions for the reviewer

1. The V-bits: what chip load suits the real cutting diameter at 0.6 mm depth (sharp tip)?
2. With strokes of a few mm, does the feed matter, or does acceleration set the time?
3. What plunge rate gives a clean stroke start?
