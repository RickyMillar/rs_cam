# 15 Front rest R1 (valleys), R1 tapered ball (op 16)

Op 16 "Front rest R1 (valleys)", setup 3. Group C.

## Tool: R1 x 6 x 20 mm 2-flute tapered ball (tool id 4, T5)

| Field | Value |
|---|---|
| Type | tapered ball end mill |
| Tip | ball R1.0 (2.0 mm diameter) |
| Taper half angle | 5.71 deg (ESTIMATE: computed from the size, atan((6/2 - 1) / 20); measure) |
| Flute length | 20 mm |
| Shank | 6 mm |
| Flutes | 2 |
| Stick-out | 30 mm in the project (the library entry says 35 mm; ESTIMATE, measure) |
| Material, coating | carbide; coating not known |
| Possible match | `~/.config/rs_cam/tools/tapered_ball.toml` "R1.0mm x 6mm x 20mm 2F Tapered Ball", vendor endmill.com.au, product TBN-R1.0-6-20, taper 5.7 deg (the project does not say it is this product) |

Cutting diameter at height h above the tip (ball up to h = 0.90 mm, then the taper):

| h (mm) | 0.5 | 1.0 | 1.5 | 2.0 | 2.5 | 3.0 |
|---|---|---|---|---|---|---|
| Diameter (mm) | 1.73 | 2.01 | 2.11 | 2.21 | 2.31 | 2.41 |

## The cut

- Op kind: 3D Rough (`adaptive3d`), strategy `contour_parallel`, from remaining stock (after
  op 15), region ordering by area, no flat-area levels.
- What it cuts: the rest the 6 mm rough left in narrow valleys and on terraces. Region filter:
  `rest_max_region_width_mm` 20 (only regions up to 20 mm wide).
- Radial engagement: stepover 0.66 mm (as planned) = 0.33 x the 2 mm tip.
- Axial engagement: depth per pass 2.0 mm (as planned). At 2.0 mm the ball is buried and the taper
  cuts too (cut diameter about 2.2 mm).
- Stock to leave: 0.5 mm axial.
- Entry: ramp at 3 deg, helix radius factor 0.3, helix pitch 2.0 mm.
- Material: oak rest stock; epoxy in the V grooves and lakes (as in brief 13).
- Air share: ESTIMATE high (a rest op on scattered narrow regions). A similar variant (rest region
  width 6 mm) measured 301 m cut against 676 m of rapids (2026-11-02).
- Note: a `geodesic_adaptive` variant of this op planned for 4 h without an end (rs_cam defect
  candidate G-R1HANG). This op uses `contour_parallel`.

## Machine limits that bind

- Spindle rpm max 24000: the cut diameter is about 2 mm, so the top of the range binds.
- XY acceleration 1200 mm/s^2 on short contours.
- Deflection of a small tip at 30 mm stick-out (the taper stiffens it).

## Material and finish

- Not the final surface (0.5 mm left). It removes the deep bites before the finish.
- Burning at low chip load; smear on epoxy.

## Questions for the reviewer

1. The R1 tapered ball as a rest roughing tool: is it sane, and at what load?
2. What chip load applies at the real cut diameter (about 2.0-2.3 mm at 1-2.5 mm deep)?
3. Which depth x stepover pairs suit it (for example 1.0 mm deep and wider against 2.0 mm deep and
   0.66 mm)?
