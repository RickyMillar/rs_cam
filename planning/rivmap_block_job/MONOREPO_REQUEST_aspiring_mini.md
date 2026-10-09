# Prompt for the monorepo agent: an "aspiring-mini" two-sided test block

Paste the text below into an agent session in `/home/ricky/personal_repos/project_rivmap_mono`.

---

You work in the RivMap monorepo (`/home/ricky/personal_repos/project_rivmap_mono`). Make a SMALL
real map block, "aspiring-mini", that tests the full two-sided epoxy light-pipe process on real
terrain and real rivers before the full nz-south block is cut. Write prose in ASD-STE100. Commit
by path. Do not push.

## Why

The epoxy test piece (`manufacture/block/TESTPIECE.md`) uses flat slots and a planar ramp. The real
block cuts its lines with a 20° V-bit from the back (rough slot, then the V tip past the front
surface by the overcut), and the front is real terrain with steep valley walls. The operator wants
a mini block that does exactly that.

## The region

The operator's earlier one-sided job "Aspiring" (rivmap100) used this export:
`~/Downloads/aspiring/rivmap_export/rivmap_data.toml` (the old rivmap_studio format). Its bounds:

- west 168.510361, south -44.552803, east 169.032211, north -44.179742
- physical width 100 mm, square (lock_aspect = true)
- rivers: merge_tolerance 2.0, min_length 8.2, simplify_tolerance 0.17 (visvalingam)

Make the same region a map in the current pipeline (`design/maps/aspiring-mini.toml`) and run the
current terrain export, so that the block stage gets the files that nz-south has (`export.toml`,
`paths.json`, the holes and the terrain).

## The block

Use the nz-south process and parameters (`manufacture/block/map.toml`), scaled to a mini block:

- the block about 100 x 100 mm of relief plus a rim; thickness T = 25 mm, relief depth as nz-south
  (or less if the 100 mm scale needs it; say what you chose and why);
- a cavity on the back sized for this block with a rim of at least 12 mm, the depth rule and the
  web_min of 4.0 mm as nz-south;
- the 20° V-bit rivers and lake shores with the 0.8 mm front line, the rough slots where the V body
  needs them, the light holes if the design has LEDs under this region (else say so);
- two epoxy groups if possible (for example the west half sealed + white tint, the east half
  unsealed + clear), as in the test piece, so the light and bubble comparison stays.

## Outputs (the same set as `build/block/nz-south/`)

`block.json`, `back_channels.dxf`, `vgroove_paths.csv`, `holes.csv`, `channels.stl`, the terrain
STL in the block frame, the mech back cuts if any, `checks.txt`, `plan.png`, and `epoxy.stl`
(the epoxy volume in the block frame; rs_cam can place it as a stock-change model).

## Items rs_cam asked for (see `rs_cam/planning/rivmap_block_job.md`, "The list for the monorepo")

Please include at least these for the mini block:

1. a `cam` object in `block.json`: T1 (faced back thickness, 25.5), raw_top_max, face_front, the
   stock margins and two pin positions on the flip axis in the X margins, pin_depth;
2. nested rough-slot layers (each ROUGH_SLOT_Z<k> inside Z<k-1>), or a `z_start` per layer;
3. the flip convention of rs_cam: a 180° turn about a line parallel to machine X through the stock
   centre (north and south change places);
4. one dowel moved by at least 3 mm along the flip axis so that the pair keys the flip;
5. through cuts to T1, not to T.

## Report

The map file, the commands, the output folder, the block numbers (size, cavity, slots, V paths,
holes, epoxy ml per group), and the checks. The rs_cam lead then builds the two-sided rs_cam job
from the output folder.
