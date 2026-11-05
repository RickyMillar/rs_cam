# P13 step 0: the inputs and assumptions behind each output (2026-11-05)

Read-only research at `36fd50c54` (HEAD later `9ca624b13`; no `crates/` change between).
Page: `index.html`. Checks: `assert_claims.py` (24/24 PASS, `python3 -I`). The Python port of
the shipped laws (`shipped_laws.py`) matches the release CLI and the FM1 golden at 56 points.
The research agent wrote the analysis; the lead saved this text (the agent could not write it).

Operator focus (2026-11-05): "less need to check the math, but the assumptions. If our end
result is 'x', have we gathered all we need to work that out?"

## Answer

The model cannot compute a Pocket stepover: no limit in the model or the repo falls with the
width. The stepover needs a ruling, not more data. A computed depth is possible from data
already in the repo (the printed hobby slot rows).

## G. Inputs and assumptions per output

37 factors (`inventory.json`; each row cites its code site at `36fd50c54`):
8 user-stated, 2 derived, 15 a typed constant stands in, 12 ignored with no stand-in.
No site at all: runout (G06), offset-corner coverage (G33), gantry thrust (G34).
Typed constants outside F201: helix 30 degrees, carbide E 600 000 N/mm2, bending fraction 0.80,
the default feed cap 6000 mm/min (C18).

| Machine | Gaps most likely to decide the result |
|---|---|
| Hobby | Frame stiffness X/Y and Z/spindle mount (ignored; the beam is "clamped at x = 0 (the collet face)", `tool/mod.rs:747`); chip evacuation (ignored; only drill pecks credit it); the feed cap (binds at the FM1 point, 3999 of 4000 mm/min); the Unsourced conventions depth 0.25 D, Pocket stepover 0.35 D, slot cap |
| Industrial | Tool life (ignored); chip evacuation; spindle power; chatter (ignored) |

| Output | Has what it needs? | Reason |
|---|---|---|
| Stepover | No | No limit falls with the width |
| Depth | Partly | The vendor depth table and the flute guard bound it; the hobby depth has no physics |
| Feed | Mostly | Gaps: runout, wear, acceleration |
| rpm | Mostly | Gaps: chatter, heat |
| Plunge, ramp | Mostly | The 3 degree ramp is a repo rule; Z stiffness ignored |

Why no limit picks a narrow Pocket width: a Pocket's power is judged at ae = D for every width
(`PowerCase::Slot`); the peak force is almost flat in width (15.33 N at 0.35 D, 15.92 N at D,
+3.9 %) while the removal rate rises 2.86x.

Can stay as stated assumptions with a visible warning: helix, E, bending fraction, grain,
moisture, climb mode, runout, coating, long-tool derate.

## A. Force and power range (light check)

FM1 6 mm hardwood Pocket: 15.3253 N, 0.0600 kW (7.5 % of the 0.8 kW generic spindle). Nine
printed full-slot points (Amana, Harvey, Sienci, Carbide 3D) are inside 0.8 kW and the 200 um
tool limit (highest 0.548 kW, 17.2 um); the Carbide 3D points are inside the 0.71 kW Makita
preset the chart names. The force coefficients match the Curti and Goli tables; the code reads
Curti's helix-0 rows for every tool. Curti's helix-30 rows give 0.65x the shear coefficient and
an edge term near zero, so the shipped force is high for a 30 degree tool: "nothing binds" is
robust.

## B. Frame compliance

The repo prints no machine, belt, screw or collet stiffness. It would enter as a spring in
series with the tool beam: delta = F_peak x (1/k_frame + c_tool) <= target. The target today is
200 um (F201 Unsourced); a better target is one the user states (the finish allowance).

Back-solve, IMPLIED AND NOT A SOURCE (stickout 20-60 mm moves the 200 um value 78-90 N/mm):

| Target | Pocket convention | Adaptive convention | Vendor slot 1 x D |
|---|---|---|---|
| 200 um | 79 N/mm | 101 N/mm | 322 N/mm |
| 100 um | 162 N/mm | 209 N/mm | 714 N/mm |
| 50 um | 344 N/mm | 450 N/mm | 1831 N/mm |

Printed hobby charts imply 71-182 N/mm and industrial charts 309-569 N/mm if each point sits on
the limit. With any of these springs the best removal rate is still full width at a shallow
depth (at 79 N/mm: ae = D, ap = 0.2 D). Frame compliance explains a shallow depth, not a
narrow width. Sienci and Carbide 3D print hardwood at full width, 0.39-0.40 D deep.

Measurement protocol (operator):
1. Dial indicator (0.01 mm or finer) on a magnetic base on the bed, not the gantry.
2. Clamp a short steel pin of the largest diameter the collet takes; subtract its computed
   compliance (the shipped beam) later.
3. Motors powered, as in a cut.
4. X and Y: pull the pin along the axis with a spring scale or a weight over a pulley;
   indicator on the far side of the pin, on the same line.
5. Z: pull up and down at the collet nut; indicator on the nut face.
6. 6 loads from 0 to about 120 N (covers the 14-104 N peaks of A); load up, then unload;
   3 runs per axis.
7. Repeat at the gantry centre and both ends of Y travel, Z high and Z low.
8. Fit deflection against force on the loading branch. Slope minus the pin = compliance (mm/N);
   k = 1/slope (N/mm). Report the intercept as backlash. Use the worst position.

## C. Chatter

A stability-lobe model needs the tool-tip frequency response (natural frequency, damping, modal
stiffness per direction) and the radial coefficient K_r (the code sets 0). None is printed in
the repo. A phone-microphone tap test gives the frequency, the ring-down an estimate of the
damping, protocol B the static stiffness; a real FRF needs a force hammer per tool and
stickout. Out of scope for P13: state "the cut is stable" on the card; revisit after B.

## D. Chip evacuation

No printed numeric rule in the repo limits a wood or plastic slot for chips or heat. Only:
Sorotec "up to 2x diameter" slot depth (no reason), Harvey's miniature step-down (not wood),
qualitative upcut advice (Onsrud, Vortex), PreciseBits test depths. It stays a stated
assumption: "chips always clear".

## E. The removal-rate aim

Maximise MRR = ae x ap x vf, vf = min(rpm x Z x fz x lift(ae), cap). The answer is full width in
all 5 cells and all 20 sweeps (feed cap 4000 mm/min to unlimited, spindle 0.8-6 kW), industrial
included. D 6 hardwood: the convention (0.35 D, 0.25 D) removes 12 599 mm3/min; the optimum in
the computed limits (D, 1 x D) 144 000 (11.4x); with the typed 0.25 D cap (D, 0.25 D) 36 000
(2.86x). Below the feed cap MRR grows as ae/sin psi, at the cap as ae; only a limit that falls
steeply with width gives a narrower optimum. The one sub-D result: the 12 mm tool with power per
pass, 0.95 D. Printed scale (Adams): "Belt driven - 4,000 x 4 x 4 = 64 cm3/min" at 100 % width.

## F. Synthesis and options

- The depth convention is most likely set by frame and Z compliance, then chip evacuation.
- The stepover convention is not explained by missing force physics. Chip evacuation, wall
  finish, chatter and tool life could explain it; none has a number in the repo.
- Corner geometry gives a derivable bound: with sharp 90 degree inward-offset corners, a
  stepover up to (1 + 1/sqrt 2) r = 0.854 D leaves no uncut material.

| Option | Gather | Changes | Code | Recommendation |
|---|---|---|---|---|
| O1 Force budget per machine class from the printed hobby slots | Nothing new | A derived hobby depth replaces the typed 0.25 D | Small | First |
| O2 Measured frame stiffness | Protocol B; a user-stated deflection target (finish allowance) | Frame spring in series with the tool beam; a depth per machine | Small | Second |
| O3 Stepover from the corner bound plus a stated assumption | An operator ruling | 0.35 D stays Unsourced, or full width up to 0.854 D, with a visible "no width limit is modelled" warning | Small | Rule now |
| O4 Removal-rate pick in the envelope | Nothing | The proposal is the most removal inside the limits | Small | After O1 or O2 (before that it proposes a 1 x D slot) |
| O5 Chip evacuation, chatter, tool life | A printed numeric source; modal data | Real width limits | Large | Out of scope |

Order: O3 ruling, O1, O2 measurement, O4; O5 only when a source appears.
