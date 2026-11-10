# P23b survey: the last wanaka fail rows, idx 5 / 9 and idx 16 (2026-11-10)

Read-only. No code changed, no cargo run (the lane was busy). Every statement is read from the
code or from a repo data file; the arithmetic is shown. Sources: `acceptance/wanaka_21.md`,
`acceptance/grade_dce570108_r2.md`, `P21_DESIGN.md` (Q13).

## 1. idx 5 and idx 9 (Project Curve): "Suggest prints an RPM and no feed"

### 1.1 Finding: the calculator does not abstain; the report surface does

- No calculator step abstains on the feed. A surface-offset op (`Axial::SurfaceOffset`,
  `FeedsInput::depth_after_simulation`) skips only the depth default and the flute guard
  (`calculator/cut.rs:68`, `:100`), the power figure (`FeedsInput::power_force_line`,
  `mod.rs:598`) and the MRR (`close.rs:402`). Step 5 still gives `feed = chip x Z x RPM x lift`
  (lift 1: a Project Curve cuts the full width), and Step 8 gives the plunge. The sentry
  `project_curve_plunges_at_the_axial_chip_q8` reads a feed, a plunge and an RPM from Suggest on
  a Project Curve.
- The apply funnel writes the three together: `suggest/apply.rs:86-87` (feed, plunge), `:137`
  (RPM), `:253-256` (the write to the op). No branch writes the RPM alone.
- The gap is in the report. MCP `get_suggest_rationale` prints the Suggest feed only inside
  `basis.shipped_power` (`operating_point.rs::shipped_power_json`: `feed_mm_min`, `rpm`). On a
  surface-offset op that object is `{"unmodeled": "DepthAfterSimulation"}`
  (`operating_point.rs:262`, `:353`, `:407`), so the feed has no field. P21c added `basis.rpm`,
  which does not depend on the power, so the RPM shows and the feed does not.
- The stored 3165 mm/min on idx 5 is the Suggest feed at the old RPM: 0.14919 x 2 x 10610 =
  3165.8. Nobody applied Suggest after the policy changed to `constant_chip`.

### 1.2 What an apply writes today under constant chip (from the code, not run)

- idx 5 (6 mm 2F flat, chip 0.14919, no printed RPM, ceiling 24000): RPM 24000, feed
  0.14919 x 2 x 24000 = 7161 (the design's idx 17 value for the same tool and row), and the
  plunge `min(G10 rule, axial chip 0.0635 x 24000 x 2 = 3048, Z rate 2500)`, so at most 2500.
- idx 9 (20 deg 1F V-bit, chip 0.127, chart 18000): RPM 24000, feed 0.127 x 1 x 24000 = 3048.
- The RPM and the feed never ship apart through `apply_feeds`. They are apart on the wanaka file
  only because the operator can type or keep one field.

### 1.3 Recommended change (one)

Print the Suggest speeds where the RPM is printed: `basis.rpm` gains `feed_mm_min` and
`plunge_mm_min` of the suggested operation (`CutterOpProfile::suggested_operation`), on every
op, independent of `shipped_power`; the CLI `suggest` record already prints
`calculator.feed_rate_mm_min`. No engine change. The grader then reads the feed 7161 for idx 5.

Rulings: Q13 ("Suggest prints the constant-chip RPM on idx 5 and 9; no depth needed") supports
printing, and the feed needs no depth either. The other candidate, "print no RPM where no feed
is printed", works against Q13 and hides a value the engine has.

To check with one run when the lane is free: `get_suggest_rationale` on idx 5, then `apply_feeds`
on a copy, and read the three fields.

## 2. idx 16 (Front scallop finish, R1 tapered ball, tip 2.0 mm, 2F, white oak)

### 2.1 Where 0.06437 comes from

- The matched row is not the SpeTool row. It is the G1 form A size claim
  (`extrapolation/size.rs`: log-log interpolation of the band mids of two adjacent printed sizes
  of one chart series) on the Amana ZrN tapered rows `amana-tapered-hardwood-parallel-1587-2f-zrn-v8`
  (0.03881-0.0635, mid 0.05115) and `amana-tapered-hardwood-parallel-6350-2f-zrn-v8`
  (0.1778-0.2286, mid 0.2032).
- Arithmetic: slope = ln(0.2032 / 0.05115) / ln(6.35 / 1.5875) = 0.99501;
  chip = 0.05115 x (2.0 / 1.5875)^0.99501 = **0.0643702**, the app value to seven digits
  (0.06437017664).
- Why that member ships: the vendor band rule of P16 item 8 (`vendor_lookup::VendorBand`,
  `BandRule::Lowest`). The band holds each printed row at the query size and one form A
  interpolation per series; "the shipped row is the member with the lowest chip middle". The
  SpeTool row `spetool-tapered-hardwood-parallel-2000-2f` prints the exact size, one value
  0.0762 (a point), and is above the Amana interpolation, so the Amana value ships.
- The table's source line: `data/vendor_lut/sources/spetool_2d3d_tapered.txt`, row
  "2.0 | Wood, MDF, Sign-Foam | 18,000 | 0.003 | 108" (SPE23): 0.003 in x 25.4 = 0.0762 mm;
  108 in/min / (18000 x 0.003) = 2 flutes. Header "Depth of cut: 1xCutting diameter" (SPE14).

### 2.2 Defect or ruling

A second ruling, not a defect. The code does what P16 item 8 rules (lowest member of the band;
lead provisional ruling 2026-11-06). The table expects the exact-size printed row. The operator
question: for a generic tool (no vendor on the record), does an exact-size printed row outrank
a lower interpolated value of another vendor's series?

### 2.3 P4a and P4b in plain words

A ball tip that cuts shallower than its radius meets the wood on a slanted edge, so the chip is
thinner than the feed per tooth: peak chip = fz x sin k, with
sin k = sqrt(1 - (1 - ap/R)^2). Here ap 0.2 mm, R 1.0 mm: sin k = sqrt(1 - 0.8^2) = 0.6.

- **P4a: no lift.** The feed per tooth is the chart chip. Feed = chip x Z x n. The chart chip
  stands at every depth. This is the code today (`calculator/feed.rs`: the axial term of a ball
  "stays OBSERVED, NOT APPLIED: the charts print no rule for a depth below 1 x D").
- **P4b: the ball lead-angle lift.** The feed rises by 1 / sin k = 1.6667 so that the peak chip
  is the chart chip at this shallow depth.

| Chip | P4a (no lift) at 24000 | P4b (x 1.6667) at 24000 |
|---|---|---|
| SpeTool 0.0762 (the table) | 0.0762 x 2 x 24000 = 3658 | 3658 x 1.6667 = 6096 |
| Amana interpolated 0.06437 (the app) | 0.06437 x 2 x 24000 = 3089 (shipped) | 3089.8 x 1.6667 = 5149 |

The app's 3089 is P4a with the app's chip. The feed row therefore fails for the chip alone under
P4a, and for the chip and the lift under P4b.

### 2.4 Recommended change (one)

Ask the operator two things, in this order; no code before the answers.

1. The chip row: keep "lowest member of the band" (0.06437), or let an exact-size printed row
   outrank an interpolated member (0.0762)? Recommendation: the exact-size printed row. A printed
   value at the tool's size is a source; an interpolation is a claim. The change is one rule in
   `VendorBand` (members with `SizeBasis::Exact` first), with an FM1 move to list.
2. The feed row: P4a or P4b? Recommendation: P4a until a source prints the shallow-depth rule.
   The SpeTool chart states its chip at a depth of 1 x D, and no chart in the repo prints a lift
   below that depth; P4b is a model (the same geometry as Garr's radial rule, on the axial side),
   and the operator ruled the radial lift in as a model (D11), so P4b is a consistent choice if
   the operator wants the faster finish.

## 3. P23b item 1, coded (lead go, 2026-11-10)

- One door: `feeds::profile::CutterOpProfile::suggested_speeds()` returns `SuggestedSpeeds`
  (`rpm`, `feed_mm_min`, `plunge_mm_min`, `ramp_feed_mm_min` of `suggested_operation`, plus the
  feed terms `chip_load_mm`, `flutes`, `radial_lift`). `None` when Suggest refuses.
- MCP `get_suggest_rationale`: `basis.rpm` holds those fields on every op
  (`app/mcp/generation.rs::suggest_basis_json`). CLI `suggest --json`: `suggested_speeds` (beside `shipped_power`),
  the same struct. The GUI card is not changed: it shows the per-field values of the apply funnel
  (`FieldApplyPreviews`), which the same `suggest_for_operation` call gives.
- No engine change. No param schema change, so the wire snapshot does not move (the tool
  description text changed; the snapshot holds param schemas only).
- Sentry (viz) `the_rationale_prints_the_feed_with_the_rpm_p23b`: on the wanaka idx 5 cut the
  reply holds RPM 24000, a feed and a plunge with `shipped_power` unmodeled; feed =
  `floor(chip x flutes x rpm x lift)` of the same reply; the values equal the core record; a
  Pocket prints the same fields. Red (both tests) when the `feed_mm_min` insert is dropped.
- Gates: fmt clean; the sentry 2 of 2; `rs_cam_mcp` 39; `rs_cam_cli` 107; viz `--lib` 703;
  `mcp_wire_surface_pin` 2 of 2, no diff; `the_card_names_every_entry_rule_g10` 4; full clippy
  clean.
- Item 2 (idx 16) waits for the operator's two answers (section 2.4). Nothing is written for it.

## 4. Operator rulings on idx 16 (2026-11-10, read by the lead)

1. The chip row: "yes". An exact-size printed row outranks a lower interpolated member of
   another series. Phase P23c implements it in the one vendor band rule.
2. The feed row: no ball lead-angle lift (P4a): "finishing feeds are already pretty high
   generally".

## 5. Correction to section 2.1 (found in P23c, 2026-11-10)

Section 2.1 says the SpeTool row "is a higher member" of the idx 16 band. That is wrong. The
band of idx 16 has one member, the Amana ZrN interpolation. The SpeTool row is not a candidate
for a Scallop op: the G3 family rules (`extrapolation/family.rs::FAMILY_RULES`) transfer a
tapered-ball row to the 3D finish families only from the Onsrud 77-100 and the Amana ZrN v8
sources. The arithmetic of 0.06437 stands. See `P23c.md` sections 2 and 4.
