# Sienci Labs machine-stiffness data (fetched 2026-11-06)

Operator request 2026-11-06. This folder stores the Sienci Labs deflection tests as a
cited reference for hobby frames. `assert_claims.py` checks each number in this file
against `stiffness_table.csv` or a stored text (`python3 -I assert_claims.py`).
`build_table.py` builds the CSV from the texts in `sources/`.

## Sources and status

| Source | Status | Stored text (`sources/`) |
|---|---|---|
| Sienci, *Stiffness Rating with Commentary* (PDF, 8 pages), <https://sienci.com/wp-content/uploads/2022/09/Stiffness-Rating-with-Commentary.pdf> | 200, stored | `sienci_stiffness_rating_with_commentary.txt` |
| Sienci, earlier draft of the same report, <https://sienci.com/wp-content/uploads/2022/09/Stiffness-Rating-deflection-testing-Google-Docs.pdf> | 200, stored; the CSV cites the Commentary PDF | `sienci_stiffness_rating_google_docs_draft.txt` |
| Sienci blog, *Stiffness and Deflection Testing on the LongMill MK2* (2022, edited 2026-03-18), <https://sienci.com/2022/09/09/stiffness-and-deflection-testing-on-the-longmill-mk2/> | 200, stored | `sienci_blog_mk2_stiffness_2022.txt` |
| Sienci raw data, `MK2-Deflection-Testing.xlsx` (linked from the MK2 post), <https://sienci.com/wp-content/uploads/2022/09/MK2-Deflection-Testing.xlsx> | 200, stored | `sienci_mk2_deflection_testing_xlsx.txt` |
| Sienci blog, *Rigidity Testing on the LongMill MK3* (2026-03-30), <https://sienci.com/2026/03/30/rigidity-testing-on-the-longmill-mk3/> | 200, stored | `sienci_blog_mk3_rigidity_2026.txt` |
| Sienci forum, *Longmill rigidity 2*, <https://forum.sienci.com/t/longmill-rigidity/26150> | 200, stored; one user, not staff; no measured number; not used | `sienci_forum_26150_longmill_rigidity.txt` |
| THIRD PARTY: Carbide 3D forum, *Backlash, Deflection and Vibration* (user LiamN), <https://community.carbide3d.com/t/backlash-deflection-and-vibration/28669> | 200, stored; linked from the Sienci PDF; not in the CSV | `thirdparty_carbide3d_forum_28669_backlash_deflection.txt` |

No fetch failed. The MK3 post shows its cutting-force plots (Millalyser) only as images.
The text keeps only the ranges that the post writes in prose.

## What Sienci measured and how

MK2 protocol (PDF text lines 57-104):

- The gauge is a 0.001 in dial indicator on a magnetic stand. The stand sits on a steel
  plate that clamps to the wasteboard. The wasteboard is the datum, so bench and table
  flex are in the result.
- The load acts on a 0.25 in end mill that is held inverted, with 1.25 in stick-out,
  1 in above the wasteboard. A force gauge pulls the bit through a strap.
- The procedure load is 10 N. Sienci also loaded 50 N (see the next section).
- The machine is at the centre of its travel. Only the stepper holding torque holds it.
- The result is the mean of the positive and negative readings, 3 runs per axis. It
  includes backlash and hysteresis (line 333).
- The test measures X and Y only. It prints no Z table. The xlsx has a Z breakdown at
  98 N with no deflection unit; this file does not use it.
- Sienci prints no stiffness value. Every table is force and deflection. The column
  `stiffness_derived_N_per_mm` is force / deflection, computed in `build_table.py`.

MK3 protocol (MK3 post line 256): a lead-screw jig pushes a force gauge against the
router; a dial indicator on the other side reads the motion. The load point is the router,
not a bit. The table gives each direction at 25, 50, 75 and 100 N, for a LongMill MK3
48x30 (prototype parts) and an AltMill 4x4.

## The 25 N / 50 N label

The PDF (line 144) labels its second MK2 table "25N". The MK2 post (lines 245, 261)
strikes 25 N and prints 50 N; the xlsx raw data is at 50 N. The data agrees with 50 N.
On the 48x30 X axis the xlsx gives 139 N/mm at 10 N and 139 N/mm at 50 N. Read at 25 N,
the same deflection gives 69 N/mm, half the 10 N value. The CSV keeps the PDF rows as
printed (25 N) with a note. The PDF 10 N mm values are the xlsx run averages, rounded.

## Printed values (derived k = force / deflection)

| Machine | Worst axis, secant at 50 N | Fit slope, 25-100 N |
|---|---|---|
| LongMill MK2 12x30 / 30x30 / 48x30 (Y) | 108 / 106 / 95 N/mm | no data above 50 N |
| LongMill MK3 48x30 (Y-) | 164 N/mm | X 176.5, Y 182.3 N/mm |
| AltMill 4x4 (Y+) | 492 N/mm | X 546.8, Y 546.8 N/mm |

- The MK2 48x30 Y axis softens with load: 124 N/mm at 10 N, 95 N/mm at 50 N.
- The MK3 fit lines do not pass through zero (intercept X -25.4 um, Y +50.8 um). The
  readings are whole thou, so one thou of reading error moves the low-load secant much.
- The xlsx *Linearity* sheet (size not named) gives a secant of 89.5-91.6 N/mm on X from
  25.6 to 150 N. Sienci uses it to say that deflection is linear in load (PDF line 349).
- Sienci's statements: the MK3 is 65-80 % stiffer than the MK2 (MK3 line 262); the AltMill
  is 2-4x stiffer than the LongMill (line 467); keep the force under 25 N to stay inside
  0.005 in (line 435).

Force statements by Sienci: 10 N is the "typical" cut force (PDF line 298). This is a mean
gauge reading along the feed, rounded to 5 N, with rolling resistance subtracted (line 249).
The theoretical maximum for a 1.25 HP router is 93 N (line 210). Near a stall or in chatter
the force can be an order of magnitude higher (line 302). The MK3 post gives Millalyser mean
and peak feed forces of 3.8-10.4 N (line 443).

## Limits

- Static load only. "Machine behavior in motion is not evaluated" (PDF line 664). The data
  gives no damping, no natural frequency and no chatter limit.
- One unit per build: LongMill MK2 12x30, 30x30 and 48x30 (lead screw, V-wheels), a MK3
  48x30 from prototype parts, and an AltMill 4x4. Sienci says that V-wheel wear and a
  gantry swap changed the MK2 results (PDF lines 157-168).
- The MK2 and MK3 tests load different points (the bit; the router body).
- No Z-axis stiffness value.
- All data is the vendor's own and has no independent check.

## Shapeoko or other belt machines

No Shapeoko Pro, no Shapeoko and no other belt-and-rail machine has a row in any Sienci
table. The PDF names the Shapeoko 3 once, in words only (line 179): the 30x30 MK2 "performs
similarly to the Shapeoko 3 without the stiffer SO3 Z-axis upgrade". It cites the Carbide 3D
thread above. That thread is THIRD PARTY: one user, one Shapeoko XXL with an HDZ axis,
a 50 N load (line 35), and most numbers in images. Its text gives only a bound
("< 0.4mm or 15 thou", line 24). The PDF belt section (lines 603-616) gives belt tensions,
not machine deflection.

## Use for rs_cam

The feeds model has a hobby force budget of 35.3224 N (`explainer_p13c`, from
`feeds/force_budget.rs`) and a tool-deflection target of 200 um (step 0, F201 Unsourced).
Step 0 B planned the frame as a spring in series with the tool beam:
`delta = F_peak x (1/k_frame + c_tool) <= target`.

The two forces are different measures. The budget is a computed peak engaged-edge resultant.
Sienci's 10 N is a mean gauge reading along the feed. Do not compare them as equal.

What the data can give:

1. A cited k_frame for a hobby lead-screw frame. The LongMill worst axes, 95-164 N/mm, lie
   inside the band of 71-182 N/mm that step 0 back-solved from the hobby charts. All are
   above the 79 N/mm that step 0 needs for 200 um at the Pocket convention. This is a
   consistency check, not a proof.
2. A check of the budget against the target. At 35.32 N the frame alone deflects 371 um
   (MK2 48x30), 215 um (MK3 48x30) and 72 um (AltMill 4x4). For 200 um of frame deflection
   alone at the budget, the frame needs 176.6 N/mm. The LongMill MK2 does not meet this,
   and the MK3 is at the limit. The force that gives 200 um of frame deflection is 19 N
   (MK2), 33 N (MK3) and 98 N (AltMill).
3. A worked example of the step 0 B dial-gauge protocol. Sienci uses the same parts:
   a dial indicator on a base on the bed, a force gauge, a short bit in the collet, the
   motors powered, and the machine centre. Sienci's procedure is less complete. It uses one
   position and one load, and it has no loading branch or intercept fit.

What the data cannot give:

- A default stiffness for a Shapeoko-class belt machine. No such row exists.
- A Z-axis or plunge stiffness.
- A chatter or stability limit, or a dynamic stiffness.
- A stiffness for the operator's grblHAL machine. Only protocol B on that machine gives it.

Suggestion: keep the force budget as the hobby default. When a user states k_frame (from
protocol B, or a cited row such as these), use the series-spring check. Show the LongMill
rows as named, cited examples, not as a class default.
