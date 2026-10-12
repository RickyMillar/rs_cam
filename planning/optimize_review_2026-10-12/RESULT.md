# Optimizer rebuild, result record (2026-10-12)

Branch `optimizer-rebuild`. Instrument: `crates/rs_cam_core/tests/optimize_z22_headline.rs`
(`#[ignore]`; `cargo test -p rs_cam_core -q --test optimize_z22_headline -- --ignored --nocapture`).
It copies `planning/rivmap_block_job/testpiece_mini.toml` to the scratchpad, generates every
enabled toolpath, simulates at the GUI operating point (cell 0.2 mm, modulation on,
`ConstrainedMax`, scale 1.0) and runs the `optimize_toolpath` Job on the toolpath
"Channel BACK_CHANNEL_Z22.00" (Pocket, 3 mm 2-flute flat, oak, total depth 22 mm).
Dev build; the raw logs are `r0_debug.log` (before G-OVERCUTREF) and `r0_run2.log`
(after) in the session scratchpad.

## Baseline (the on-screen trace, both runs)

| Reading | Value |
|---|---|
| Cycle | 1 315.5 s (rapid 691.9, cutting 328.7, entry 294.8; air 409.9) |
| Params | dpp 0.5946 mm, stepover 0.838 mm, feed 3 877, 18 000 rpm, links off |
| Gates | chipload Within (peak 0.1077 at the point 0.1077), power and deflection Unmodeled, depth Within (0.595 against 0.75) |
| Full cut | removed volume 10 017 mm3, rapid collisions 0, over-cut peak 0.0 mm |

## Run 1 (4a9884f7, before the over-cut fix): `Ranked` 876 s (-33.4 %)

The over-cut finding fired on every deeper cell with a peak equal to the cell's own
dpp (reference: 3x the pass's median bite), and at dpp 0.8148 gave `Holds` at
stepover 1.05 and `Fails` at 0.838. Lead ruling: fix the owner. Done in 98e3de6f.

## Run 2 (d5d1daaa, after G-OVERCUTREF and G-SPEEDSAPPLY): `Ranked` 360 s (-72.6 %)

Headline: "Tried 9 candidates. The fastest safe full cut takes 360 s against 1315 s
(-72.6 %)." Explanation: "The recommended candidate's depth of cut 2.75 mm is over
the 0.75 mm rule of thumb (reported, not a limit)." Unmodeled: "Power and deflection
are not modelled: No force line: no printed density for this species; stock source
`rivmap feeds_oak.toml`". Refused cells: none. Wall 91.6 s (9 simulations).

| # | dpp | stepover | feed | rpm | links | cycle s | depth row | volume mm3 | collisions | over-cut | full cut |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 (recommended) | 2.750 | 0.838 | 3 877 | 18 000 | off | 359.9 | Exceeds 2.75 / 0.75 | 9 987 | 0 | 0.0 | Holds |
| 2 | 1.222 | 0.838 | 3 877 | 18 000 | off | 690.4 | Exceeds | 10 004 | 0 | 0.0 | Holds |
| 3 | 0.815 | 1.050 | 3 877 | 18 000 | off | 876.3 | Exceeds | 10 012 | 0 | 0.0 | Holds |
| 4 | 0.815 | 0.838 | 3 877 | 18 000 | off | 980.0 | Exceeds | 10 012 | 0 | 0.0 | Holds |
| 5 | 0.595 | 1.050 | 3 877 | 18 000 | off | 1 183.5 | Within | 10 017 | 0 | 0.0 | Holds |
| 6 | 0.595 | 0.838 | 3 877 | 18 000 | on | 1 315.5 | Within | 10 017 | 0 | 0.0 | Holds |
| 7 | 0.595 | 0.838 | 3 877 | 18 000 | off | 1 315.5 | Within | 10 017 | 0 | 0.0 | Holds |
| 8 | 2.750 | 1.050 | 3 877 | 18 000 | off | 317.0 | Exceeds | 9 987 | 8 | 0.0 | Fails (collisions) |
| 9 | 1.222 | 1.050 | 3 877 | 18 000 | off | 609.6 | Exceeds | 10 004 | 18 | 0.0 | Fails (collisions) |

Chipload is Within at the point on every candidate (feed 3 877 = 0.1077 x 18 000 x 2;
no de-rate below 1 x D). The volume tolerance is one floor layer,
10 017 x 0.2 / 22 = 91 mm3; the recommended candidate is 30 mm3 short (0.3 %).

## What the numbers say

- The depth lever is the lever: 2.75 mm (0.92 x D, 8 levels) against 0.595 mm
  (37 levels) removes 29 of 37 level changes and their retracts.
- The link lever is a no-op on this pocket (row 6 equals the baseline).
- CAUTION. The DOC bound 2.75 mm is the vendor `ap` rule (1 x D, realised on
  the 22 mm staircase) because the material has no force line; deflection is
  Unmodeled on the recommended cell and Apply is enabled. The -72.6 % is as
  safe as the vendor `ap` rule and no safer. The review's what-if with the
  FPL White Oak line gave 0.26 mm of tip deflection at 3.0 mm and 0.18 mm at 2.0 mm
  against the gate bound 0.200 mm: with an FPL row the bound would sit near 2 mm and
  the deflection gate would judge the cell. The outcome states this.
- Rows 8 and 9 (stepover 1.05): the 18 and 8 rapid collisions are FALSE
  POSITIVES of the dexel upper bound (G-STALETOP, probe
  `tests/optimize_z22_collisions_probe.rs`, commit ff4af15a). Every hit is
  the vertical descent of a ring entry over a column the level already cut;
  an exact disc-against-swept-segments check finds 0 rapids in material. At
  stepover 1.05 two ring-2 passes sit 2.9 mm apart, their 0.1 mm overlap is
  narrower than one 0.2 mm cell, and `conservative_top` lowers only on one
  full cover (`dexel_stock/stamping.rs::for_each_covered_cell`). The veto
  stands (the reading is an upper bound); the fix is a sub-cell coverage
  mask in the stamp. Row 8 (317 s) would be the fastest full-cut candidate if the
  stale-top reading cleared; not measured.
