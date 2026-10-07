# rivmap350 fixture

The full-size rivmap board, 350 x 500 mm (stock 380 x 510 x 26 mm). The
operator approved the push on 2026-10-01 ("the rivmap350, yeah push it")
and again on 2026-10-08. The tiered-finish trial
(`planning/tier_trial_2026-10-01/`) uses this board to confirm the winners
from the x3.5 rivmap100 proxy.

## Files

- `rivmap350.toml`: the operator's project, copied unchanged on
  2026-10-08. All model paths are relative to this folder.
- `terrain.stl`, `lakes.dxf`, `rivers_aligned.dxf`, `holes.dxf`,
  `machinable_edge_band.dxf`, `rivmap_data.toml`: the rivmap export.
- `tool_library/`: a copy of the operator's tool library
  (`~/.config/rs_cam/tools/*.toml`) on 2026-10-08. The library is a
  catalogue. It also holds generic sizes (for example the ball end mills
  from 3 mm to 25 mm), so a row does not prove that the operator owns that
  tool. The project file holds the tools that the operator uses on this
  board:
  - Ø6.35 end mill (roughing)
  - tapered ball R1.59, shank Ø6.35
  - tapered ball R1.0 x 3.175 x 15 mm, taper half angle 2.2°
  - 6 mm ball nose, 20° V-bit, 6 mm end mill (pins)

## Measured on the runner PC (2026-10-01, job 009)

The CLI simulation of the whole project at 0.2 mm cells peaks at 7.26 GB
RSS. At 0.125 mm cells it peaks at 15.19 GB, which is near the 16 GB cap of
the MCP launch.
