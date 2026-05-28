# Cycle-time re-bench checklist (Wanaka Back Rough)

**Why this exists.** The cycle-time model is calibrated against a real
wall-clock of the Wanaka **Back Rough** toolpath. Since that anchor was
measured, the planner's emitted toolpath changed substantially —
F-038 / F-038b (entry-plunge fragmentation filter + keep-tool-down
links), ContourParallelHybrid in adaptive3d, helical entry, and #149b
(nearest-vertex offset-loop ordering). Each made the program *shorter*,
so the model now predicts well under the old wall-clock and the
calibration tests sit on temporarily-widened tolerances. A fresh bench
re-anchors them.

This is **time-accuracy work, not safety** — it does not block a first
cut. Do it whenever you're next at the machine.

## What gets measured

Two runs of **Back Rough (toolpath id 4, Setup 1)** on the tuned
Shapeoko XXL:

| Run | Feed modulation | Updates constant |
|-----|-----------------|------------------|
| A | OFF | `MEASURED_UNMODULATED_S` / `BACK_ROUGH_MEASURED_S` (both 827.0) |
| B | ON  | `MEASURED_MODULATED_S` (1224.0) |

Old values are stale — expect both to drop.

## Steps

1. **Produce the exact G-code.** Load `/home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml`,
   generate Back Rough (id 4), export GRBL G-code. Do this twice — once
   with feed-modulation OFF (run A) and once ON (run B) — via the GUI
   export wizard or the MCP `export_gcode` tool. Machine config for the
   bench: `MachineKinematics::shapeoko_xxl_ricky_tuned()`, `max_feed_mm_min = 10000`.
   - The export now runs the machine-safety pass and logs any findings —
     confirm the log is clean before sending either file to the machine.
   - The emitted GRBL program now sets `G54` explicitly (commit `e6fa90b`).

2. **Run + time each on the machine.** Stopwatch from cycle start to the
   final retract. Record `run A` and `run B` wall-clock in seconds.

3. **Update the measured constants:**
   - `crates/rs_cam_core/tests/machine_kinematics_cycle_time_f034.rs:247`
     — `BACK_ROUGH_MEASURED_S` → run A seconds.
   - `crates/rs_cam_core/tests/feed_modulation_cycle_time_f036c.rs:64`
     — `MEASURED_UNMODULATED_S` → run A seconds (same value as above).
   - `crates/rs_cam_core/tests/feed_modulation_cycle_time_f036c.rs:67`
     — `MEASURED_MODULATED_S` → run B seconds.

4. **Re-tighten the tolerances back to ±15 %** (they were widened to
   `[0.45, 1.25]` as a holding measure):
   - `machine_kinematics_cycle_time_f034.rs:~315` — `(0.45..=1.25)` →
     `(0.85..=1.15)`, and drop the `⚠️ NEEDS RE-MEASURE` note.
   - `feed_modulation_cycle_time_f036c.rs:~193` — `(0.45..=1.25)` →
     `(0.85..=1.15)`, and drop the `⚠️ NEEDS RE-MEASURE` note.

5. **Verify:**
   ```bash
   cargo test -p rs_cam_core --test machine_kinematics_cycle_time_f034 \
     --test feed_modulation_cycle_time_f036c
   ```
   Both should pass inside the tightened ±15 % band. If a run lands
   outside ±15 %, the model (not the constant) needs attention —
   per-axis kinematics for Z-dominated motion is the known next lever
   (see the F-034 doc comment).

## Current model predictions (fill from a local run)

Run this to print the model's current prediction vs. the stale anchors
without editing anything:

```bash
cargo test -p rs_cam_core --test machine_kinematics_cycle_time_f034 \
  --test feed_modulation_cycle_time_f036c -- --nocapture
```

Both tests now `eprintln!` a `… REBENCH: model predicted Xs vs measured
Ys (ratio Z)` line on every run.

Current model predictions (2026-05-29, post-#149b):

- F-034 (unmodulated): model ≈ **360 s** vs 827 s anchor (ratio 0.435).
- F-036c (modulated):  model ≈ **667 s** vs 1224 s anchor (ratio 0.545).

Use these to sanity-check the bench: the new wall-clock should land
near the model prediction (the model is what we're trusting), and well
inside ±15 % of it after you update the constants.

> Note: F-034's lower tolerance bound was dropped 0.45 → 0.40 on
> 2026-05-29 because the post-#149b model (0.435) fell under 0.45.
> F-036c (0.545) is still inside `[0.45, 1.25]`. Both bounds go back to
> ±15 % (`0.85..=1.15`) after the re-bench.
