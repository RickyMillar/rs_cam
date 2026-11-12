# P29-8e: the frame of the setup obstacles in the wanaka operator file

Written 2026-11-13 on HEAD `0fad0599d`. No product code changed. A throwaway probe loaded
`planning/rivmap_block_job/nz_wanaka_21_operator.toml` read-only, printed each setup frame and
generated idx 0 and idx 17. The probe was deleted. The file was not saved or edited.

## Verdict

**The file is wrong. The code is consistent.** The author computed the four clamp boxes in the
frame of setup "1 Back" (the world frame, which is the emission frame of a Top / 0 degree setup)
and wrote the same numbers into setup "3 Front". Setup 3 is a Bottom setup, so its emission frame
is the stock-corner frame. All consumers read an obstacle in the emission frame of its setup, and
they agree. No consumer applies the stock origin to an obstacle, and no consumer mirrors an
obstacle in Y.

The code holds one frame trap (section 4, F-OF1): the emission frame of a setup is the world frame
on Top / 0 degrees and the stock-corner frame on every other face. So the same physical clamp has
coordinates that differ by the stock origin between setup 1 and setup 3. No text that an operator
or an agent reads says this.

## 1. The frame of each consumer

"Emission frame" is `SetupEvalContext::heights_stock_bbox`
(`crates/rs_cam_core/src/session/eval_context.rs:71-76`, `:114-118`): the world stock box for an
identity setup (Top, 0 degrees), the zero-rooted local box for every other setup.

| Consumer | Frame | Where |
|---|---|---|
| The resolver (every consumer below calls it) | the values as written, no transform | `compute/obstacle.rs:478-575` (`ObstacleSet::resolve`; module doc `:7-13`) |
| Generation: the obstacle step, the clip walk | emission frame (the toolpath is in it) | `session/compute/generation.rs:100-109`; `session/compute.rs:1199-1210`; `geometry/boundary.rs:389` |
| Link ceiling, territory pre-clip | the same `ObstacleToolView` | `compute/obstacle.rs:34-40`; `compute/execute/territory.rs:28-49` |
| Simulation check | emission frame (`entry.annotated.toolpath`) | `compute/simulate.rs:177-182`, `:2956` |
| `collision_check` | emission frame | `compute/collision_check.rs:150` |
| Export | reads no obstacle geometry; only the emitted moves and the obstacle plane Z | `gcode/program_builder.rs:285-287`, `:693-705` |
| Pin walls | maps the obstacle to the world frame: as is on an identity setup, `local_to_world` (flip included) on another setup | `session/pins.rs:158-211` |
| GUI viewport | emission frame plus `emission_to_display_shift`, the same shift the toolpaths take: `-origin` on Top / 0 degrees, 0 on another setup | `rs_cam_viz/src/app/obstacle_marks.rs:40-70`; `rs_cam_viz/src/state/job.rs:111-130` |
| GUI editor fields | "this setup's frame: the frame that the toolpaths of this setup use" | `rs_cam_viz/src/ui/properties/obstacles.rs:65-67` |
| MCP `add_obstacle`, `edit_obstacle` | "Every value is in the SETUP frame (the frame the setup's toolpaths use)" | `rs_cam_viz/src/mcp_server.rs:965`, `:979` |

The stock origin enters only through the emission frame itself: on an identity setup the model
and the stock stay in the world frame (`generation.rs:115-120`, the transform runs only when
`ctx.needs_transform()`). The pin walls are the only consumer that maps an obstacle (to the world
frame); that map applies the flip, which is correct for a check against pins in the part frame.

## 2. Do the consumers agree?

Yes. The clip, the link ceiling, the simulation and `collision_check` read the same resolved set in
the emission frame. The viewport draws the box with the shift of the toolpaths of the same setup,
so a box draws where the clip applies it. No code defect of the kind "the viewport draws one frame
and the clip uses another" exists.

The viewport did show the error: in setup 3 the X min boxes draw 8.5 mm outside the stock, and
the X max boxes draw 10 mm over the part edge.

## 3. Where each box is (measured by the probe)

Stock 395 x 460, origin (-18.5, -8.5). The part (`back_target_s1.stl`) is X 0..358, Y 0..443 in the
world frame.

| Setup | Emission frame: stock | Part | X min boxes (-38.5..-8.5) | X max boxes (366.5..396.5) | Box centres Y (144.83, 298.17) |
|---|---|---|---|---|---|
| 1 Back (Top) | X -18.5..376.5, Y -8.5..451.5 | X 0..358 | 10 mm on the stock, 8.5 mm clear of the part | 10 mm on the stock, 8.5 mm clear of the part | stock-corner Y 153.33 and 306.67 (the thirds of 460) |
| 3 Front (Bottom) | X 0..395, Y 0..460 | X 18.5..376.5, Y 8.5..451.5 | 8.5 mm outside the stock | 28.5 mm on the stock, 10 mm over the part | stock-corner Y 144.83 and 298.17 (8.5 mm low) |

Emitted moves:

- **Setup 1, Face idx 0** (18 mm tool, rows along X every 8 mm, X -27.5..385.5). The rows at Y
  126.5 to 158.5 run X 1.62..356.38; the rows at 118.5, 166.5 and 174.5 stop on the round corners of
  the grown footprints. The clamps clip it, and the clip is symmetric: 1.62 + 356.38 = 358, the
  centre is the part centre X 179. The Face holds 0 `EntryPlunge`, and each kept row starts with a
  pipeline `EntryApproach`: so the territory pre-clip (`compute/execute/territory.rs:28-49`, the
  obstacle cutter footprints as holes in the area outline, before the dressups) cut the rows, not
  the clip walk after the dressups. It reads the same `ObstacleToolView`. No asymmetry exists in
  setup 1.
- **Setup 3, Profile idx 17**: gaps on the X max edge only (Y 120.8..168.9 and 274.1..322.2), no clip
  on the X min edge, 42 `EntryPlunge` (the lead's measurement, repeated by the probe).
- **Setup 3, Profile idx 17 with the corrected boxes** (in memory, `Command::ReplaceObstacle`, not
  saved): 1560 moves (from 1750), 4 `EntryPlunge` (from 42), no gap on any edge above the floor.
  The 4 plunges are on the floor level, 2 on each side (X 15.5 at Y 185.67 and 327.25; X 379.5 at
  Y 274.33 and 132.75): the head zone of P29-8e_TAB_REPLUNGE.md section 2.5 (the default head band,
  radius 12.5) reaches the grown clamp when the tip is below Z 0.5, on both sides.

## 4. Who wrote the boxes, and in which frame

`planning/rivmap_block_job/derive_wanaka21.py` wrote them (commit `8860fbe86`, 2026-11-02,
"nz-wanaka-21 clamps, five pins, clamp-pad setup, clearance retract"; `git log -S"Clamp X max"`
gives `8860fbe86` as the first commit). The script, lines 242-247:

```
# The clamp keep-outs in the setup-1 frame (stock-local - the stock origin offset). The four boxes
# are symmetric about the stock centre in X and in Y, so the flipped setup 3 uses the same numbers.
...
clamp_boxes.append((x0 - mx, yc - CLAMP_SPACE_ALONG / 2.0 - my, x1 - mx, yc + CLAMP_SPACE_ALONG / 2.0 - my))
```

with `mx, my = 18.5, 8.5` (line 184) and `CLAMP_Y_STOCK = (460 / 3, 2 x 460 / 3)` (line 77). Line
752 calls the same `clamps()` for setup 3. So the author computed in the world frame. That is
right for setup 1. The symmetry argument is right (the set is symmetric about the stock centre), but
the frame of setup 3 is not the frame of setup 1: it is the stock-corner frame, so the boxes must
not subtract the origin there.

The datum note of setup 1 says "Top clamps at y 153 and 307": the script prints
`CLAMP_Y_STOCK` rounded (153.33 and 306.67). The note is in the stock-corner frame (the frame of the pin
position in the same note, "pin 1 is at (6.25, 230.0)"), not in the frame of the boxes.

**F-OF1 (code, a frame trap, not a consumer disagreement).** The value frame of an obstacle is the
emission frame, and the emission frame moves with the face: world on Top / 0 degrees, stock corner
on every other face. The GUI hover, the MCP description and the module doc say "the frame the
setup's toolpaths use" and do not say that this frame includes the stock origin on a Top / 0
degree setup. The datum frame and the viewport (zero-rooted) frame of setup 1 are the stock-corner
frame (the export shifts an identity setup's moves by `-stock_bbox.min` in XY,
G-EXPORT-DATUM, `tests/export_datum_setup_frame.rs:30-34`), so on setup 1 the editor numbers match
neither the G-code nor the screen. G-EXPORT-DATUM fixed the same frame split for the G-code; the
obstacle values still have it. A rule that makes
the obstacle frame the stock-corner frame on every face (as the alignment pins, `StockRelXy`,
`compute/alignment_pins/mod.rs:59-65`) removes the trap. That is a lead decision, outside P29.

**F-OF2 (doc).** `SetupTransformInfo::apply_to_polygons` (`compute/transform.rs:535-557`) says that
"fixture and keep-out footprints" go through `world_to_local`. No obstacle consumer calls it; K1
reads an obstacle with no transform. The text is stale.

## 5. The corrected boxes

The clamps are fixed to the table. After the flip about X on the pins, the stock covers the same
table rectangle, so a clamp has the same stock-corner coordinates in both setups. The stock-corner
values are the script's own numbers before the origin subtraction:

- X min: X -20 to 10 (20 mm past the edge, `CLAMP_SPACE_ONTO` 10 mm onto the stock);
- X max: X 395 - 10 = 385 to 395 + 20 = 415;
- Y: 460 / 3 +- 20 = 133.3333 to 173.3333, and 2 x 460 / 3 +- 20 = 286.6667 to 326.6667.

Setup 1 (Top / 0 degrees: the emission frame is the world frame, so subtract the origin
(-18.5, -8.5)): **no change**.

| Box | Before = after (min x, min y, max x, max y) |
|---|---|
| Clamp X min 1 | -38.5, 124.8333, -8.5, 164.8333 |
| Clamp X max 1 | 366.5, 124.8333, 396.5, 164.8333 |
| Clamp X min 2 | -38.5, 278.1667, -8.5, 318.1667 |
| Clamp X max 2 | 366.5, 278.1667, 396.5, 318.1667 |

Setup 3 (Bottom: the emission frame is the stock-corner frame):

| Box | Before | After |
|---|---|---|
| Clamp X min 1 | -38.5, 124.8333, -8.5, 164.8333 | -20.0, 133.3333, 10.0, 173.3333 |
| Clamp X max 1 | 366.5, 124.8333, 396.5, 164.8333 | 385.0, 133.3333, 415.0, 173.3333 |
| Clamp X min 2 | -38.5, 278.1667, -8.5, 318.1667 | -20.0, 286.6667, 10.0, 326.6667 |
| Clamp X max 2 | 366.5, 278.1667, 396.5, 318.1667 | 385.0, 286.6667, 415.0, 326.6667 |

Z stays (setup 1: 0 to 30.0; setup 3: 0 to 29.5).

What the flip does: the Bottom map is (x, y) -> (x, 460 - y) for a point of the part. A clamp is
fixed to the table, so no consumer maps it, and it must not be mirrored. The corrected set is
symmetric about Y 230 (133.3333 + 326.6667 = 460), so a mirror would give the same set; the
author's symmetry argument holds for the Y map. The error is only the origin subtraction.

The derive script needs the same correction (setup 3 takes the boxes without `- mx` and `- my`).
This note does not edit it: a peer session holds uncommitted edits in it.
