# Bench Protocol — Feed Modulation Calibration

**Three runs, ~45 min total bench time. Dry-run is safe (spindle off, no
stock). Goal: wall-clock elapsed for each .nc file.**

See `RUNBOOK.md` (also in this folder) for the deeper context — this file
is the short operational checklist.

These three files have been pre-validated by CAMotics 1.2.0 (Docker
container, GRBL-strict arc tolerance = 0.01 mm) and are clean. Any future
regeneration should be run through `./validate.sh` before leaving the dev
machine — see "Pre-flight validation" below.

---

## Files in this folder

| File | What | Length |
|---|---|---|
| `gcode/f034_reference_wanaka_pin_drill.nc` | F-034 cycle-time reference. 6-hole peck drill. | ~60-90 s |
| `gcode/f036c_wanaka_back_rough_modulation_off.nc` | F-036c baseline. Back Rough at commanded feeds. | ~13-20 min |
| `gcode/f036c_wanaka_back_rough_modulation_on.nc` | F-036c with adaptive feed modulation. | ~13-20 min |
| `validate.sh` | Run files through Dockerised CAMotics before sending to the machine. | — |
| `validator-Dockerfile` | Builds the `camotics-validator:1.2.0` image. | — |

## Pre-flight validation (do this before each USB trip)

```
cd planning/feed_modulation_calibration
./validate.sh
```

Returns exit 0 if all clean, exit 1 if any file flagged with the
offending line / error message printed to stderr. The validator runs the
full CAMotics interpreter (not just a lexical scan) so it catches what
GRBL / gSender would catch on the workshop machine — without requiring
the trip. Threshold mirrors GRBL's default `$12=0.010 mm`.

If the image is missing (`docker images camotics-validator:1.2.0`
returns nothing) rebuild it from the Dockerfile in this folder — the
script will print the command.

---

## Machine prep (~5 min, one-time)

1. Power on Shapeoko XXL, home (`$H`).
2. In your sender, type `$$` and copy these into a note for me:
   ```
   $110, $111, $112  ← max feed X/Y/Z
   $120, $121, $122  ← accel X/Y/Z
   $130, $131, $132  ← max travel X/Y/Z
   ```
3. **Dry-run setup:**
   - Empty collet (or dummy tool)
   - No stock
   - Spindle off — either physically (E-stop the VFD) or comment out
     `M3 S18000` on line 3 of each .nc file
4. Pick a workspace origin with at least **150 mm +X, 160 mm +Y, 30 mm
   +Z** clearance. (Wanaka's stock anchors at (-20, -25, -20) so this
   range is what the toolpaths sweep through.)

---

## Run 1 — F-034 reference × 3 (~5 min)

**File:** `f034_reference_wanaka_pin_drill.nc`

Run three times, stopwatch each. Take the mean.

```
Run 1.1 elapsed: __:__
Run 1.2 elapsed: __:__
Run 1.3 elapsed: __:__
Mean:            __:__
```

Start timer on **Cycle Start press**. Stop timer when machine **returns to
end position + sender says complete**. Don't trust the sender's own
runtime display on the longer runs — use a phone/stopwatch.

---

## Run 2 — F-036c unmodulated (~13-20 min)

**File:** `f036c_wanaka_back_rough_modulation_off.nc`

```
Start time:  __:__:__
End time:    __:__:__
Elapsed:     __:__
Notes:       _________________________________
```

---

## Run 3 — F-036c modulated (~13-20 min)

**File:** `f036c_wanaka_back_rough_modulation_on.nc`

This file has **1755 distinct F-words** (vs. 2 in the unmodulated). Watch
for GRBL planner-buffer alarms (alarm 8 / 9). If one fires, note the
move/line number — that's a controller-side issue, not a G-code bug,
and I'll need to know about it.

```
Start time:  __:__:__
End time:    __:__:__
Elapsed:     __:__
Notes:       _________________________________
```

---

## What to send back

Paste into the chat:

```
GRBL settings:
  $110/$111/$112: ... ... ...
  $120/$121/$122: ... ... ...
  $130/$131/$132: ... ... ...

F-034 reference (Pin Drill):
  Run 1: m:ss
  Run 2: m:ss
  Run 3: m:ss

F-036c unmodulated (Back Rough off): m:ss
F-036c modulated   (Back Rough on):  m:ss

Notes: (anything weird — alarms, pauses, weird sounds even on dry run)
```

---

## If something goes wrong

- **Buffer alarm on the modulated file**: note the move number, stop the
  run. Tells us we need a GRBL-buffer advisory in F-036's pipeline.
- **Numbers wildly off (>30% from naive)**: probably a `$$` setting
  changed since last calibration. Send me the `$$` dump and we'll
  regenerate.
- **Aborted mid-run**: note where it stopped (move # or rough %). We can
  either retry or use it as a lower bound.

---

## Interpretation cheat-sheet (what each outcome means)

**F-034 Pin Drill mean wall-clock vs. integrator prediction:**

- Within ±15%: F-034 closes, test unflags ✓
- 15-25% off: still lands but tightening goes into a follow-up finding
- >25% off: probably the `$$` defaults don't match the preset. Send dump.

**F-036c Back Rough modulated vs. unmodulated:**

- Modulated shorter by 20-50%: canonical Fusion-HSM win ✓
- Modulated ≈ unmodulated (±5%): wanaka was already tuned near the LUT
  band midpoint. F-036c lands but the narrative changes.
- Modulated *longer* than unmodulated: wanaka was over-fed; modulation
  lowered feeds for tool-life safety. Algorithm working as designed,
  just protective rather than accelerative. F-036c lands and we'd open
  a follow-up for a "speed-priority" modulation mode.

The summary diagnostics already hint at the third outcome (modulated
mean feed 1539 vs. unmodulated 3371 mm/min) — but actual wall-clock
depends on how much corner-decel time the unmodulated path was already
losing.
