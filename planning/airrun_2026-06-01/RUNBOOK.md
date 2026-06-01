# Air-Run Validation — 2026-06-01

Bench-machine validation **without cutting chips**. Purpose: catch
post-processor, tool-change, retract, and cycle-time bugs before any
material engagement. The sim is correct against itself; the question
this answers is "does the machine actually do what the sim said?"

## What's in this package

```
airrun_2026-06-01/
├── RUNBOOK.md                   (this file)
├── cycle_time_log.csv           operator fills in actual wall-clock times
├── 01_marker_test/
│   ├── marker_star.nc           Z=-0.5 mm trace, 5-pointed star ~100 mm
│   ├── marker_star.toml         project (for GUI inspection)
│   └── README.md                marker-test-specific notes
└── 02_wanaka/
    ├── wanaka.toml              project (for GUI inspection)
    ├── wanaka_setup1.nc         back side: drill + rough + holes + V-carve
    ├── wanaka_setup2.nc         front side: 3D rough + 3D finish
    └── README.md                wanaka-specific notes
```

## Predicted cycle times (via F-034 kinematics integrator)

| File | Predicted time | Moves | Tools used |
|------|---:|---:|---|
| `marker_star.nc` | **0m 28s** | 14 | 1 (marker) |
| `wanaka_setup1.nc` | **41m 44s** | 6,570 | T3 End Mill → T11 20° V-bit |
| `wanaka_setup2.nc` | **38m 27s** | 114,457 | T3 End Mill → T2 Tapered Ball |

Kinematics assumption: `shapeoko_xxl_ricky_tuned` (350 mm/s² accel,
max feed 4,000 mm/min, rapid 10,000 mm/min). If your machine is
configured differently, predicted times will be off proportionally.

## Pre-run checklist

Before any of these:

- [ ] **Workpiece removed** from the spoilboard. (Stock can stay if
      you raise Z; but for the cleanest air-run, work area should be
      empty.)
- [ ] **Spoilboard clear** of clamps / dust shoe / probe wires that
      the rapid paths might hit.
- [ ] **Tools laid out** on the bench in the order they'll be called
      (see per-file READMEs). Air-run still needs the actual tools
      installed so the tool-length offset is correct.
- [ ] **Feed-override pot** at 100 % to start. If a move looks wrong,
      slow it before hitting the e-stop.
- [ ] **Stopwatch** ready (phone is fine) — you'll log actual cycle
      times into `cycle_time_log.csv`.

## Order of execution

Start with the smallest, most reversible test and work up:

### 1. Marker test (5 min including setup)

Cheapest first. Validates: coordinate system, work offset, retracts,
post-processor dialect, lead-in/out paths. No tool changes, no
spindle.

1. Tape A4 / letter paper to the spoilboard. Origin (G54 X0 Y0) goes
   on the lower-left corner of the paper, with at least 110 mm clear
   in +X and +Y from there.
2. Install a Sharpie or fineliner in the collet. Spindle should
   stay off; if your GRBL has a spindle relay, unplug it. The
   G-code does emit `M3 S0` (cosmetic) and `M5`.
3. Touch off Z so the marker tip **just kisses the paper** at Z=0.
   Use a sheet of paper as a feeler if you have to.
4. Load `01_marker_test/marker_star.nc`, jog to G54 X0 Y0, hit
   Cycle Start.
5. **Watch for:**
   - 5-pointed star outline drawn cleanly (no missing strokes)
   - Marker lifts to Z=+5 between strokes (it shouldn't, since the
     trace is one continuous path — but if a "Rapid" happens
     mid-stroke, that's a bug)
   - Final retract to Z=+10 at the end
   - Spindle commands accepted but ignored (M3/M5)
6. Log cycle time. Predicted 28 s.

**If anything looks wrong**, stop here and file the finding. No
point air-running the bigger job if the basics fail.

### 2. Wanaka Setup 1 — back side (45 min, includes 1 tool change)

Validates: drill cycle (Pin Drill, Holes), adaptive 3D rough (Back
Rough), V-carve project_curve (Rivers, Lakes), and the M6 tool
change between End Mill and V-bit.

1. **Stock area:** clear the bed. Workpiece is not needed; raise the
   Z so cuts happen above the spoilboard (e.g. raise the work offset
   by 30 mm). Per-toolpath top_z is ~30 mm so the rapids will all
   sit above your spoilboard plus that buffer.
2. **First tool**: T3 = End Mill (whatever's in the project — could
   be a placeholder). Install it; touch off Z to the raised work
   surface (30 mm above spoilboard).
3. Load `02_wanaka/wanaka_setup1.nc`, jog to G54 X0 Y0, hit Cycle
   Start.
4. The job runs Pin Drill → Back Rough → Holes → Rivers (back).
   ~32 min of rapid + simulated cuts.
5. **Tool change happens** before "Lakes (back, inside)" — GRBL
   pauses on `M6 T11`. Swap to T11 = 20° V-bit. Touch off Z again
   (use a touch plate or paper feeler against the V-bit tip). Hit
   Cycle Start to resume.
6. Lakes (back, inside) runs to completion. ~10 min more.
7. Log cycle time per phase (use `cycle_time_log.csv` template).

**Things to watch for:**
- Pin Drill plunges all 4 corners at expected XY positions
- Back Rough: large adaptive sweep across the back. Tool path should
  look continuous, no sudden retracts mid-cut, no diagonal travels
  through the (imaginary) stock.
- Holes: drill cycle pecks (you'll see Z stair-step on each hole).
- Rivers + Lakes (after V-bit change): V-carve traces small features.
  Check that the V-bit goes to right XY positions.
- M6 pause: machine actually stops, message appears on controller
  display, tool can be safely changed.
- After M6 resume: spindle restarts (M3 S<rpm>), motion continues.

### 3. Wanaka Setup 2 — front side (40 min, includes 1 tool change)

Validates: 3D adaptive rough + 3D scallop / drop-cutter finish, M6
between End Mill and Tapered Ball.

1. (Conceptually) flip part. For air-run, just leave the spoilboard
   clear and run.
2. **First tool**: T3 = End Mill. Install + touch off.
3. Load `02_wanaka/wanaka_setup2.nc`. Jog + Cycle Start.
4. 3D Rough 6 runs. ~10 min.
5. **Tool change** before "3D Finish 6" — GRBL pauses on `M6 T2`.
   Swap to T2 = Tapered Ball. Touch off Z. Cycle Start.
6. 3D Finish 6 runs. ~28 min — this is 113,000 moves of fine surface
   scallop pattern. Will look like one continuous sweep.
7. Log cycle time.

**Things to watch for:**
- 3D Rough: should look like an adaptive sweep at multiple Z levels
- 3D Finish: continuous surface scallop, no surprise rapids through
  the 3D model envelope
- Tapered Ball stickout matters here — verify the project's
  declared stickout matches the physical tool

## Post-run report

After each phase, fill in `cycle_time_log.csv`:

```csv
phase,predicted_min,actual_min,notes
marker_star,0.47,,
wanaka_setup1_endmill,32.0,,
wanaka_setup1_vbit,9.7,,
wanaka_setup2_endmill,10.0,,
wanaka_setup2_taperedball,28.5,,
```

(Predicted values are split estimates within each file. The whole-file
predictions are in the table above — single .nc files don't expose
per-phase wall-clock predictions yet, so the per-phase splits are my
estimate based on operation type.)

## What constitutes a finding

File anything that surprises you, no matter how small. Categories:

**Bug (rs_cam side):**
- Rapid path travels through where stock would be
- Tool change M6 doesn't pause / wrong tool number
- Spindle on/off timing wrong (M3 after first cut, M5 before last)
- Coordinate offset wrong (everything shifted by a constant)
- Predicted cycle time off by > 20 % systematically
- Post-processor emits something GRBL refuses to parse

**Process gap (operator side):**
- Tool change instructions unclear
- Touch-off procedure ambiguous
- Order of operations confusing

**Latent quality issue:**
- Engagement at a feature looks wrong (you'd note it on a real cut
  but air-run shows the path)
- Finishing toolpath has visible witness marks in the rapid pattern

For each, note: file (which .nc), phase name, what happened, what
you expected, what the timestamp / line number was if you can catch
it on the controller display.

## Q: Can I validate this in the GUI first?

**Yes.** Open either `.toml` in the GUI:

```
cargo run -p rs_cam_viz --bin rs_cam_gui -- planning/airrun_2026-06-01/marker_star.toml
cargo run -p rs_cam_viz --bin rs_cam_gui -- planning/airrun_2026-06-01/wanaka.toml
```

What you can verify in the GUI:

- Tool list (per-tool name, type, diameter, tool_number)
- Toolpath order + tool assignment per toolpath
- Simulation view: rapid paths, cut moves, retract heights
- Per-toolpath stock simulation (will the rapids clear the stock?)
- "Suggest" buttons on Feeds / Speeds per toolpath — see below

The GUI's "Export G-code" path uses the **same emitter** as the CLI,
so what you'd export from the GUI matches the .nc files in this
package byte-for-byte (modulo timestamps).

## Q: Are we using suggested feeds and speeds?

**No.** The current Wanaka feeds/speeds are the values in the
project file — which came from the 2026-05-27 feed-modulation
bench batch, operator-tuned for that experiment, not LUT-suggested.

The marker test uses `feed_rate = 800 mm/min` — my pick for a clean
marker line, also not LUT-suggested.

For air-run validation, **the feed values don't really matter** —
we're testing post-processor mechanics, tool changes, retracts.
Feeds only matter for cut quality and force, which air-run can't
measure.

If you want to check what the LUT recommends vs what's in the file:

1. Open `wanaka.toml` in the GUI.
2. For each toolpath, look at the Feeds / Speeds modal — it shows
   the current value and the LUT-suggested value with citation.
3. Click "Suggest" to apply, or just note the deltas.

The Suggest values are LUT-driven per (tool, material, operation) —
backed by the bundled 252 vendor LUT rows after the Phase 5
schema-unlock work. Most of the Wanaka toolpaths use End Mill on
hardwood; the LUT match for that is well-covered.

If you want the air-run to also test suggested-vs-tuned-feeds
honesty, the cleanest path is: regenerate the .nc files after
applying Suggest in the GUI, then compare against what's bundled
here. That's not air-run validation though — that's a separate
"do our suggested feeds match the tuned values that worked" check.

## Q: What if a tool change asks for a tool I don't have?

The Wanaka project lists 12 tools but the air-run only ever calls
T2, T3, and T11. The .nc files only emit M6 for those three (M6 T11
in Setup 1, M6 T2 in Setup 2; T3 is the start tool, no M6).

If GRBL asks for a different T number, that's a finding — means
either (a) the project's toolpath→tool assignment is wrong, or
(b) the emitter put the wrong T in the M6 line. Note the line
number and which T it requested.

## Reporting back

When you're done:
1. Save the filled-in `cycle_time_log.csv` somewhere.
2. Take a photo of the marker test result (the star on paper).
3. Note any findings as described above.

Bring it back to a session — we'll triage findings, file the real
bugs, and decide what to validate cutting-side first.
