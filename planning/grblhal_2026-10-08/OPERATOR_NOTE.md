# grblHAL with BitZero and BitSetter: operator note

The board runs the tool change. rs_cam writes `M6 T<n>` on the grblHAL
post. Each claim cites grblHAL/core@c3a887e.

## WARNING: the built-in M6 probe uses the DEFAULT probe input (2026-10-08)

On this board (BTT Scylla, `PROBES=3` = probe + toolsetter, report.c:1044)
the BitSetter is wired to the TOOLSETTER input and the BitZero to the
default probe input. The `$341=3` routine selects the toolsetter only when
the driver installs `grbl.on_probe_toolsetter` (config.h:525-530). This
build does not, so the M6 probe watched the BitZero input and drove the
tool into the BitSetter on 2026-10-08 (operator e-stop). Measured:

- The `Pn:` monitor shows `P` for the BitZero only (it reports the
  selected probe, report.c:1417).
- `G38.2 G91 Z0.5 P1 F25` with the plunger held: ALARM:4 (already
  triggered). Released: moved 0.5 mm, ALARM:5. So the BitSetter works on
  the toolsetter input (`P1`, gcode.c:3899-3910).

`$341` is set back to 0. Do NOT set `$341=3` until one of these is done:

1. Swap the plugs: the BitSetter on the default input, the BitZero on the
   toolsetter input, with a `P100.macro` that probes with `P1`.
2. A firmware build with expressions (`EXPR`) and a `tc.macro` that
   probes with `P1`.

Before any probe move, prove the input with a no-motion `Pn:` test and a
0.5 mm upward `G38.2` test. The steps below apply only after that.

## The controller (2026-10-08)

The real `$I`/`$$` (fixture `crates/rs_cam_core/tests/fixtures/grblhal_dump_2026-10-08/`):
BTT Scylla, H-100 VFD over Modbus, `$340=5.0`, `$342=30.0`, `$341=0`. The
operator now sets `$341=3`, G59.3 on the BitSetter and `$342=40`.

1. `$341=3`: automatic touch off at G59.3. Do not use `$341=4`: the board
   then ignores M6 (gcode.c:1838) and the job runs on with the old tool.
   The export warns when the machine profile says `$341=4`.
2. G59.3 is the BitSetter: `G10 L2 P9 X<x> Y<y> Z<z>` in machine
   coordinates, Z just above the BitSetter. P9 is G59.3 (gcode.h:196,
   gcode.c:3078-3080); L2 stores the values as given (gcode.c:3105-3106).
   The offset lock setting can refuse the write (gcode.c:3086-3089).
3. **Caution, before the first M6:** the probe goes down from the G59.3 Z
   by up to `$342` (tool_change.c:275, 317). On a homed machine the board
   cuts the probe target back to the work envelope (tool_change.c:277-278).
   Make sure that the G59.3 Z minus `$342` (40 mm) is inside the Z soft
   limit, or the probe stops short and can miss the BitSetter.
4. `$30=1000` is the PWM spindle maximum. The H-100 VFD plugin overrides
   it (settings.c:2593), so rs_cam stores `$30` for information and does
   NOT clamp S to it. `$340=5.0` makes M3 wait for the spindle
   (spindle_control.c:787-806), so "Controller waits for spindle" is on by
   default and the file has no extra dwell.
5. In rs_cam: Machine panel > Import GRBL $$. Paste `$I` and `$$` together.
   The import sets the post to grblHAL and stores the facts above.

## Each setup (each per-setup file)

1. Home. Homing clears the tool length reference (tool_change.c:56-62).
2. `M6 T<first tool>` (from the sender, or let the file do it: every
   grblHAL file starts with `M6 T<first tool>` before the first M3). The
   first probe after homing sets the reference (tool_change.c:339-343).
3. Zero X, Y and Z on the part with the BitZero, with the same tool.
4. Run the job. When the controller already holds the first tool, it
   skips the file's first M6 (tool_change.c:433-434, gcode.c:2613-2615).
   At each later `M6 T<n>` the board stops the spindle and the coolant
   (tool_change.c:474-476), moves Z up to home (tool_change.c:171-182)
   and waits. Change the bit and press cycle start. The board probes at
   G59.3 and applies the length difference (tool_change.c:283-356).

Give each tool its own T number: the board skips an M6 to the loaded
number. The export warns on a shared number.

If you zero Z BEFORE the reference is set (skipping step 2 after homing)
the first M6 of the file becomes the reference, and Z is still right only
if the file's first tool is the one you zeroed with.

## Two-sided jobs

An `M0` setup pause is a feed hold (gcode.c:4979-4981). The board refuses
a jog in that state (system.c:239-242), so you cannot re-zero inside one
file. The grblHAL export therefore writes one file per setup by default:

1. Run file 1.
2. Flip the part. Keep the X/Y zero (the files say "X0 Y0 = stock min
   corner" in each header).
3. Zero Z with the BitZero on the new top.
4. Run file 2.

A single-file export is still possible. It keeps the `M0` and says why in
a `(MSG,...)` line.

## Open

- The real `$$` and `$I` output of this controller. The import test uses a
  synthetic dump built from the firmware sources.
