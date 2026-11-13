# P29-10i: the point band (F-PointBand)

Trace note for the lead. HEAD `4a6050c6b`. No product code changed.
"Measured" means a probe printed the number at HEAD. "Reasoned" means the
number comes from reading the code, with no run.

The probe was a throwaway core test. It loaded the four project files
read-only through `ProjectSession::load`, then printed
`ProjectSession::safe_envelope(..).rows()`, `cutter_op_profile(..).feeds`
and the stored op values. The probe is deleted. A copy and its raw output
are in the agent scratchpad `p29_10i/` (`zz_probe_point_band_p29_10i.rs`,
`probe_out.txt`, `summary.txt`). The operator file checksum stayed
`f6a2b388...4e41` before and after the run.

## 1. Where the planned `ChipMin` floor comes from

The chain from the vendor row to the `Check`:

1. `feeds/vendor_lookup.rs:927-935`: a row with min == max (or one value)
   gives `chip_load_min_mm: None`. A2 holds here.
2. `feeds/calculator/chip.rs:143,155-166,194`: the calculator keeps a
   band only with both limits (`RequireBoth`). A point gives
   `chipload_bounds: None` and `chipload_point_mm: Some(v)`. A2 holds here.
3. `feeds/envelope.rs:3327-3331` (`SafeEnvelope::value_rows`): the envelope
   reads `chipload_bounds`, and with no band it takes
   `chipload_point_mm.map(|v| (v, v))`. **This line makes a band from one
   value.** `band_min = v`.
4. `feeds/envelope.rs:3357-3358`: `judge_declared_cuts::<ChipMinPlanned>`
   with `ChipEnv { bound_mm: band_min }`.
5. `feeds/planned.rs:995-1013,1026-1034`: `peak_chip` gives the advance per
   tooth over the lift at the cut's width, with the bound `v`.
6. The row basis is `Basis::Printed(row)` (`envelope.rs:4973-4983`,
   `per_point_limits`). `Basis::gates_export` is `true` for `Printed`
   (`feeds/sourced.rs:277-288`). So `Tone::of` (`limits/row.rs:341-355`)
   gives `Exceeds`, the gating tone, when no caveat softens the row.

Commit: `eb6a4a4a7` (2026-11-12, "P29-7 commit 7c: every limit judges
every declared cut before the simulation"). `git log -S
"chipload_point_mm.map(|v| (v, v))"` gives only this commit. Before 7c the
envelope had no chip-minimum check: the old file reads the point only as a
band maximum (`table_band_max`, `vendor_depth_limit`) and lists `ChipBand`
as a per-point limit with no value check. The A2 commit is `fe1fbbd38`
(2026-09-24). So 7c came after A2 and did not follow it on the min side.
The 7c phase file saw the effect (`P29-7_DESIGN.md` 16.9: "0.1229 against
a point band of 0.127 mm") and logged it for P29-10.

Measured on the operator file, idx 1 (body, 18 mm 2F flat, white oak):

| Quantity | Value |
|---|---|
| Row | `amana-flat-hardwood-pocket-12700-2f-spektra`, `Point`, `is_extrapolated` true, size scale x1.2371, `ae_min`/`ae_max` none |
| Point `v` | 0.179128 mm |
| Suggest rpm, feed | 24000, 9123.025 mm/min (written 9123) |
| Lift at the feed width (Step 5) | 1.06104 = Garr at 5.9913 mm |
| Lift at the proposal width | 1.08544 = Garr at 5.5 mm |
| Body row | 0.175102 against 0.179128, margin -2.248 %, `Exceeds`, tone `Exceeds` |
| Ratio check | 1.06104 / 1.08544 = 0.97752, so -2.248 % (the lift ratio alone) |

The same rule on the other operator roughs (measured):

| idx | Row | Lift at feed width | Lift at proposal width | Body value | Bound | Margin |
|---|---|---|---|---|---|---|
| 1 | 12700 point, extrapolated | 1.06104 | 1.08544 | 0.17510 | 0.17913 | -2.25 % |
| 2 | 6000 point | 1.12233 | 1.15470 | 0.12344 | 0.12700 | -2.80 % |
| 14 | 12700 point, extrapolated | 1.20926 | 1.25000 | 0.17329 | 0.17913 | -3.26 % |
| 15 | 6000 point | 1.15204 | 1.18752 | 0.12321 | 0.12700 | -2.99 % |

The finding named idx 1, 14 and 15. idx 2 is a fourth case with the same
cause (measured).

On the same ops the `planner cap` cut row is `ChipMin` 0 % (exactly `v`),
and the first-slot row is `ChipMax` +6 % to +21 % over `v` (fed at the op
feed with no lift). Against a zero-width band no single feed holds all
three cuts inside.

## 2. The sample gate floor for the same op and row

Reasoned (no simulation ran; a gate with no trace stops before the row
lookup, `tool_load/chipload.rs:348-355`):

- `tool_load/chipload.rs:406-410`: `validate_chipload_bounds(None, Some(v),
  AllowHalfBand)` keeps `min_mm_per_tooth: None`.
- `chipload.rs:425-428,449-450`: a point gets
  `ChipBoundsSource::VendorLutPointPreset`.
- `tool_load/verdict.rs:1866-1870` (`burn_reference_mm`): the burn side
  reads `v` itself for a point.
- `chipload.rs:715-723`: the gate compares the MEDIAN sample peak chip to
  `v` with `ToleranceBands::burn` (0.0 at every product door,
  `tool_load/mod.rs:407-423`).
- `verdict.rs:1786-1793` (`low_side_is_advisory`): `true` for a point. So a
  median under `v` gives `Within` with a `burn_advisory`
  (`chipload.rs:795-817`), never `Exceeds(Low)`. The diagnostic is
  `LOAD_CHIPLOAD_WITHIN`, severity `Info`, with the words "BELOW the ...
  printed point ... advisory only" (`diagnostics/adapters/from_tool_load.rs:389-410`).
- `limits/project.rs:37-60` (`of_criterion`): the gate row is `ChipMin`
  only on `Exceeds(Low)`. For a point the gate row is always `ChipMax`. The
  gate has NO `ChipMin` row and no floor in its bound source
  (`BoundSource::VendorChipBand { floor_mm_per_tooth: None }`, clause "the
  printed point 0.1791 mm/tooth, no band").

The gate row is the same row as Suggest's by design (`chipload.rs:363-371`,
F3.4; `FeedsExplain::published_chipload_point` gives the same `v`,
measured). So:

| idx | Planned body `ChipMin` floor | Gate floor (`min`) | Gate burn reference | Gate low side |
|---|---|---|---|---|
| 1 | 0.17913, tone `Exceeds` | none | 0.17913 (median) | advisory only |
| 14 | 0.17913, tone `Exceeds` | none | 0.17913 (median) | advisory only |
| 15 | 0.12700, tone `Exceeds` | none | 0.12700 (median) | advisory only |

They do not agree. The gate holds the number `v` as an advisory burn
reference on the median, with no floor. The planned row holds `v` as a
gating floor on the peak. The gate would also make the low side advisory
for `VendorLutExtrapolated` and `VendorLutMissingAe`. Every Amana Spektra
row here prints no `ae` window, so a band from those rows would still be
advisory at the gate (reasoned).

## 3. The chip floor that Suggest holds

Reasoned from the code, values measured:

- Step 5 (`feeds/calculator/feed.rs:106-129`): `feed = rpm x chip_load x Z
  x lift(full_feed_width)`. For a point `chip_load = v`. The point is a
  TARGET of the peak chip at the planner-cap width, not a floor.
- Step 7 (`feed.rs:213-294`, the Q1 arms): arm S lowers the chip. It reads
  no chip minimum ("an arm with a chip under the rubbing floor is inside").
- The power ladder rung 4 and Step 9b (`feeds/calculator.rs:102-110`,
  `calculator/close.rs:160-195`): the floor is `rubbing_floor_at(band,
  point)` = `min(0.025, v)`. For every point row on these files `v >
  0.025`, so the floor is the repo constant 0.025 (`RepoConstant`,
  measured on all 18 operator ops). It warns only.
- The apply (`feeds/suggest/apply.rs:84-87`): the feed rounds DOWN to a
  whole mm/min. An applied feed is under `v` by up to `1/F` (idx 1: 9123.025
  to 9123).
- Pass 9 (`suggest/invariants.rs:131-140`,
  `suggest/axial_envelope.rs:145-163`): `RequireBoth` gives no band for a
  point. `chip_point_for_dpp` (`axial_envelope.rs:167-174`) reads the point
  only as the rubbing-floor input of `adaptive_entry.rs:410-425`.

No Suggest step reads a minimum from a point row. The only "too thin" rule
Suggest holds is the rubbing floor 0.025.

## 4. Planned `ChipMin` Exceeds rows on the four files

Measured. All rows are at the proposal. "Tone" is the row tone (`Exceeds`
gates, `Advisory` = `ExceedsAdvisory`). A ramp or helix that exceeds is
softened by `WorstCase` or `MayOccur`, except where the tone column says
`Exceeds`.

| File | idx | Cut | Value | Bound | Margin | Tone | Row id | Kind |
|---|---|---|---|---|---|---|---|---|
| operator | 0 | ramp, helix | 0.17908 | 0.17913 | -0.01 % | Advisory | 12700-2f | point, extrap. |
| operator | 1 | body | 0.17510 | 0.17913 | -2.25 % | Exceeds | 12700-2f | point, extrap. |
| operator | 2 | body | 0.12344 | 0.12700 | -2.80 % | Exceeds | 6000-2f | point |
| operator | 6 | ramp, helix | 0.01333 | 0.05501 | -75.7 % | Advisory | 2381-2f | point |
| operator | 7 | ramp, helix | 0.01333 | 0.05501 | -75.7 % | Advisory | 2381-2f | point |
| operator | 11 | ramp, helix | 0.06006 | 0.17913 | -66.5 % | Advisory | 12700-2f | point, extrap. |
| operator | 12 | ramp, helix | 0.17908 | 0.17913 | -0.01 % | Advisory | 12700-2f | point, extrap. |
| operator | 14 | body | 0.17329 | 0.17913 | -3.26 % | Exceeds | 12700-2f | point, extrap. |
| operator | 15 | body | 0.12321 | 0.12700 | -2.99 % | Exceeds | 6000-2f | point |
| operator | 16 | helix | 0.00667 | 0.07620 | -91.2 % | Advisory | spetool-tapered-2000-2f | point |
| nz_wanaka_21 | 1 | body | 0.17329 | 0.17913 | -3.26 % | Exceeds | 12700-2f | point, extrap. |
| nz_wanaka_21 | 2 | body | 0.12286 | 0.12700 | -3.26 % | Exceeds | 6000-2f | point |
| nz_wanaka_21 | 2 | ramp, helix | 0.01298 | 0.12700 | -89.8 % | Advisory | 6000-2f | point |
| nz_wanaka_21 | 14 | body | 0.17329 | 0.17913 | -3.26 % | Exceeds | 12700-2f | point, extrap. |
| nz_wanaka_21 | 15 | body | 0.12321 | 0.12700 | -2.99 % | Exceeds | 6000-2f | point |
| nz_wanaka_21 | 15 | ramp, helix | 0.03234 | 0.12700 | -74.5 % | Advisory | 6000-2f | point |
| nz_wanaka_21 | 16 | helix | 0.00582 | 0.07620 | -92.3 % | Advisory | spetool-tapered-2000-2f | point |
| nz_wanaka_21 | 17 | ramp, helix | 0.01298 | 0.12433 | -89.6 % | Advisory | onsrud-60-200-1_4-finish | BAND |
| rivmap100 | 0 | ramp, helix | 0.01389 | 0.12700 | -89.0 % | Advisory | 6000-2f | point |
| rivmap100 | 1 | ramp | 0.01389 | 0.12700 | -89.0 % | Advisory | 6000-2f | point |
| rivmap100 | 1 | helix (Always) | 0.01389 | 0.12700 | -89.0 % | Exceeds | 6000-2f | point |
| rivmap100 | 2 | helix | 0.00500 | 0.07620 | -93.4 % | Advisory | spetool-tapered-2000-2f | point |
| live_0925 | 0 | ramp, helix | 0.01389 | 0.12700 | -89.0 % | Advisory | 6000-2f | point |
| live_0925 | 1 | ramp | 0.06667 | 0.12700 | -47.5 % | Advisory | 6000-2f | point |
| live_0925 | 1 | helix (Always) | 0.06667 | 0.12700 | -47.5 % | Exceeds | 6000-2f | point |
| live_0925 | 2 (disabled) | helix | 0.00500 | 0.07620 | -93.4 % | Advisory | spetool-tapered-2000-2f | point |

Row ids are short: `amana-flat-hardwood-pocket-<size>-2f-spektra`,
`spetool-tapered-hardwood-parallel-2000-2f`,
`onsrud-hardwood-60-200-1_4-finish`.

Summary: 36 rows over the four files (a ramp and a helix count as two). 34
read a point row; 2 (nz_wanaka_21 idx 17) read a band. 10 rows have the
gating tone: 8 body rows (all point, cause = the lift at two widths) and 2
helix rows on rivmap100 idx 1 (point, entry chip about 89 % and 47 % under
`v`). The entry rows that sit far under `v` are also under the rubbing
floor 0.025 where the value is under 0.025 (operator 6, 7, 16; nz 2, 16;
rivmap100 0, 1, 2), and those carry a `RubbingFloor` row too.

## 5. The stated point with today's floor rule (prediction for 10i)

Today the stated chip rows read `NotJudgedHere` (`limits/project.rs`
`of_envelope`, `Edge::PerPoint`). The table gives the stated peak chip that
a stated body row would read: stored feed / (rpm x Z) / lift at the stated
width (`RadialThinning::of(feeds_style)`, `EngagedDiameter::of_hint`). The
floor is `v` (point) or the band min. "hi" is the chip at `f + 1` mm/min
(the `StoredFeedChip` interval of P29-9).

| idx | Op | Row kind | Stated peak | Floor | Margin | `[f, f+1)` holds the floor | Source |
|---|---|---|---|---|---|---|---|
| 0 | Face | point | 0.17911 | 0.17913 | -0.010 % | yes | measured |
| 1 | 3D Rough | point | 0.21689 | 0.17913 | +21.1 % | no | measured |
| 2 | 3D Rough | point | 0.12699 | 0.12700 | -0.008 % | yes | measured |
| 3 | Pin drill | point | 0.02778 | 0.06350 | -56 % | no | reasoned, see note |
| 4 | Drill | point | 0.02778 | 0.06350 | -56 % | no | reasoned, see note |
| 5 | Project Curve | band | 0.14915 | 0.12433 | +20.0 % | no | reasoned (finish, lift 1) |
| 6 | Pocket | point | 0.05498 | 0.05501 | -0.055 % | yes | measured |
| 7 | Pocket | point | 0.06989 | 0.05501 | +27.0 % | no | measured |
| 8 | Pocket | point | 0.06989 | 0.05501 | +27.0 % | no | measured |
| 9 | Project Curve V | band | 0.12700 | 0.07620 | +66.7 % | no | reasoned (finish, lift 1) |
| 10 | Trace V60 | point | 0.06350 | 0.06350 | 0 % | yes | reasoned (finish, lift 1) |
| 11 | Pocket | point | 0.17911 | 0.17913 | -0.010 % | yes | measured |
| 12 | Face | point | 0.17911 | 0.17913 | -0.010 % | yes | measured |
| 13 | Trace V60 | point | 0.06350 | 0.06350 | 0 % | yes | reasoned (finish, lift 1) |
| 14 | 3D Rough | point | 0.17911 | 0.17913 | -0.010 % | yes | measured |
| 15 | 3D Rough | point | 0.12699 | 0.12700 | -0.008 % | yes | measured |
| 16 | Scallop | point | 0.06436 | 0.07620 | -15.5 % | no | reasoned (finish, lift 1) |
| 17 | Profile | band | 0.14915 | 0.12433 | +20.0 % | no | reasoned (no stepover, lift 1) |

Prediction with today's floor rule (a stated `ChipMin` row reads `v` as a
floor):

- **idx 16** gives a stated `ChipMin` Exceeds, -15.5 %, tone `Exceeds`
  (point row, `Printed` basis). This is a real line on the card face.
- **idx 0, 2, 6, 11, 12, 14, 15** sit 0.008 % to 0.055 % under `v`. The
  stored feeds came from an earlier chip-first apply that rounded down. If
  the stated row reads the exact stored feed, all seven are `Exceeds` with
  a face line "chip min < 0.1 % under floor". If the stated row reads the
  high end of `[f, f+1)` (as an entry row does since P29-9), all seven are
  `Within` with `Caveat::StoredFeed`. 10i must rule which.
- **idx 3, 4** (drills): `limits::scope` does not judge the chip on a drill
  cut (P28-20), but the planned list of these ops holds a `body` cut that
  IS judged (0.0635 against 0.0635, `Within`, measured). If a stated body
  row follows the planned body row, both drills read -56 %, tone
  `Exceeds`. Not verified: what 10i declares as the stated cuts of a drill.
- idx 1, 7, 8 are above the point by 21 % to 27 %. They have no `ChipMin`
  line. They would have a stated `ChipMax` Exceeds line (not this finding).

## 6. The FM1 matrix

Reasoned (FM1 not run):

- No column of `matrix_2026-09-23.csv` or `_sim.csv` holds a planned
  `ChipMin` state. `envelope_columns`
  (`tests/feeds_matrix_instrument_fm1.rs:328-358`) prints
  `proposal_binding`, `proposal.ap_mm`, `proposal.ae_mm`. A value check
  draws no ceiling, so it moves none of the three.
- `diagnostic_ids` (`recipe_diagnostics`, lines 655-675) reads the static
  checks, the feeds result, the hints and the Suggest warnings. Ruling L2
  (P29-10): no diagnostic comes from a planned row. `OutsideEnvelope` reads
  `placing_margins` (the boundary edges), not the value rows.
- `chipload_bounds_min_mm`, `chipload_bounds_max_mm` and
  `chipload_point_mm` print the calculator fields. Options (a) to (d) below
  do not change these fields.
- The `_sim.csv` `chipload_state` column reads the gate. The gate is not in
  the scope of any option.

So no FM1 cell moves under any option (reasoned). For scale: 166 of the
1000 cells carry a `chipload_point_mm` (all with status ok); these are the
cells whose planned `ChipMin` row would change.

## 7. Options (no choice)

**(a) The planned row follows A2: a point has no floor.**
The `ChipMin` row of a point row is `NoNumber` with a named reason; the
`RubbingFloor` row stays the judge of "too thin".

- Code: `envelope.rs:3327-3331` keeps `band_max = v` for `ChipMax` and gives
  `ChipMin` no bound for a point.
- Reason, choice 1: a new `NoNumberReason`, for example "the vendor row
  prints one value, no floor". It is a fact of the source (like
  `NoVendorRow`), not a scope exemption, so it needs no `EXEMPTIONS` row and
  the pin `MAX_NEEDS_RULING` (26) does not move. Choice 2: `NoBound` with
  row `S-NoBound`; that row today means the depth of a finish pass (ruled
  R2), so this widens a ruled row. `NoVendorRow` is close but false: a row
  exists.
- What moves (measured rows): the 8 gating body rows and the 2 gating helix
  rows go to `NoNumber`. The 24 advisory entry rows on point rows go to
  `NoNumber`. The 2 band rows (nz idx 17) stay. The `RubbingFloor` rows
  stay. The stated prediction of section 5: no stated `ChipMin` line on any
  op, idx 16 included.
- Gap: between 0.025 and `v` nothing judges a thin chip before the
  simulation. Examples on the files: nz_wanaka_21 idx 2 stated 0.0311
  against `v` 0.127 (-75 %), rivmap100 idx 0 and 1 stated 0.052 and 0.053
  against 0.127 (-59 %, -58 %), operator idx 16 0.0644 against 0.0762. Each
  is above 0.025, so each is `Within` on the rubbing floor. The gate (after
  a simulation) gives only an `Info` burn advisory there.
- Agrees with the gate: yes (no floor on either side).

**(b) The planned row uses the floor of the gate's point preset.**
That floor is `v` itself, advisory, on the median.

- Before the simulation there is no median. The nearest form keeps `v` as
  the bound and makes the tone advisory for a point (a softening caveat or a
  basis that does not gate).
- What moves: the 8 body rows and 2 helix rows go from `Exceeds` to
  `ExceedsAdvisory`. The numbers do not move. Each stays in the default view
  (`in_default_view` keeps any row that exceeds), so each stays in the
  drawer. The stated prediction keeps idx 16 (and possibly the seven
  rounding rows) as a face line, advisory.
- Agrees with the gate: same number, same tone class; the statistic differs
  (peak against median).

**(c) Keep `v` as a floor, but judge the body at the width the feed was set
for.**

- The planned body row reads `lift(full_feed_width)` for a point. The body
  value is then `v` (margin 0) on idx 1, 2, 14, 15.
- The `planner cap` cut row already judges exactly this (0 %, measured). So
  (c) makes the body row a copy of that row on the min side.
- What moves: the 8 body rows go to `Within`. The entry rows and the 2
  gating helix rows stay. The stated rows of section 5 do not change: the
  stated idx 14 feed was set at lift(3.6 mm), not at the planner width, so
  the stated row depends on the stated width.
- Risk: two widths for one cut in one table. The `ChipMax` row of the same
  cut reads the proposal width.
- Agrees with the gate: no (the gate still has no floor).

**(d) Other option the code suggests: the planned low-side tone reads
`ChipBoundsSource::low_side_is_advisory`.**

- The envelope classifies the row as the gate does (point, extrapolated, no
  `ae` window) and softens every `ChipMin` `Exceeds` of that class.
- What moves: the same 10 rows as (b), plus any band row that is
  extrapolated or has no `ae` window. Every Amana Spektra row here has no
  `ae` window, so a later band from that family would be advisory too.
- Agrees with the gate on every row class, not only on the point.

**(e) Other: Step 5 sizes the feed at the proposal width for a point.**
Listed for completeness. This moves a cutting number (the feed) and
reverses P23a (sentry `the_lift_reads_the_pass_ceiling_width_p23a`); the
`planner cap` cut row would then go under `v` instead. Not a display
change.

## 8. Not verified

- No simulation ran, so the gate numbers of section 2 are reasoned. The
  gate row lookup (`matched_chip_envelope`) is `pub(crate)`; the probe did
  not call it. The claim "same row" rests on F3.4 and `lookup_parity`.
- The stated values of idx 3, 4, 5, 9, 10, 13, 16, 17 are hand
  computations (finish pass or drill: lift 1; Profile with no stepover:
  lift 1). The others are measured with the body lift rule.
- What 10i declares as the stated cuts (body only, or every declared cut;
  drills) and whether a stated chip reads the stored-feed high end.
- FM1 was not run. The "no cell moves" claim is from the column sources.
- The viz crate and the GUI were not built or opened.

## Lead rulings (2026-11-14)

- **One classification of the low side, for the planned row and the gate.** Ruling A2 stands: a
  row that prints one chip value is a point, not a band. The sample gate already treats the low
  side as advisory for a point, an extrapolated row, or a row with no `ae` window
  (`low_side_is_advisory`). The planned `ChipMin` row follows the same rule through the SAME
  function: options (b) and (d). It keeps its value and the printed value as a reference, so a
  chip between the rubbing floor and the printed value is visible before a simulation. Its tone is
  advisory, and a caveat names the reason ("one printed value", "extrapolated", "no width
  window").
- **No silent band.** `envelope.rs` must not write `(v, v)`. The chip floor that the envelope
  hands to the planned check is a typed value: a band minimum, or a reference from a point. The
  check reads the type; the tone rule reads the type. A future limit cannot make a floor from a
  point without a new variant.
- **Not option (a):** it hides a chip of 0.031 against a printed 0.127 until a simulation.
  **Not (c):** one cut with two widths in one table. **Not (e):** it moves a feed and reverses
  P23a; the feed rule is not in question here.
- **Stated chip rows (10i)** read the stored feed as the entry rows do (`StoredFeedChip`, the
  interval [f, f+1)): a row that only the rounding puts under the reference is `Within` with the
  `stored feed` caveat.
- **Expected after the change (to verify, not to assume):** the 8 body rows and the 2 helix rows
  with the gating tone become advisory; no state changes; FM1 and the op golden do not move. After
  10i the face line of wanaka idx 16 reads its stated chip under the printed value, in caution.

## 9. Step PB: predicted (before the code)

Baseline: HEAD `702c8e97b`. A throwaway probe (`zz_probe_p29_10hi.rs`, deleted;
copy and dumps in the agent scratchpad `p29_10hi/`) loaded the four files
read-only and wrote every row of `ProjectSession::limit_rows` (envelope and
verdict) as JSON. The prediction comes from that dump and the rule below; it
is not the output of the new code.

The rule of the step:

- One classification: `ChipBoundsSource::of_row(&LookupResult)` (the F3.3
  rule that the gate now holds inline). The gate and the envelope call it.
- The planned `ChipMin` check reads a typed floor (`planned::ChipFloor`): a
  band minimum, or the printed point of a one-value row. No `(v, v)`.
- A `ChipMin` row with a reading takes the caveat `AdvisoryFloor` when the
  low side of its row is advisory (a point, an extrapolated row, a row with
  no width window). The caveat softens the tone. Its face word is "one
  printed value", "extrapolated" or "no width window"; it ranks after
  "stored feed" and before "may not occur".

On the four files every matched row is a point or a band with no width
window (section 1 and the trace probe), so every `ChipMin` row with a
reading takes the caveat.

| Quantity | operator | nz_wanaka_21 | rivmap100 | live_0925 | Sum |
|---|---|---|---|---|---|
| `ChipMin` rows with a reading (take the caveat) | 60 | 44 | 13 | 13 | 130 |
| Rows that change tone `Exceeds` to `ExceedsAdvisory` | 4 | 4 | 1 | 1 | 10 |
| Rows that change state | 0 | 0 | 0 | 0 | 0 |

Faces that change: 106 of the 130 (the caveat word is added, or it
replaces "may not occur" on an entry row; a face that already shows "worst
case", "lower bound" or "stored feed", or that would pass 60 characters,
keeps its face).

The 10 tone changes (the trace table of section 4):

| File | idx | Cut | Margin | Face after |
|---|---|---|---|---|
| operator | 1 | body | -2.25 % | body · chip min 2.2 % under floor · one printed value |
| operator | 2 | body | -2.80 % | body · chip min 2.8 % under floor · one printed value |
| operator | 14 | body | -3.26 % | body · chip min 3.3 % under floor · one printed value |
| operator | 15 | body | -2.99 % | body · chip min 3.0 % under floor · one printed value |
| nz_wanaka_21 | 1 | body | -3.26 % | body · chip min 3.3 % under floor · one printed value |
| nz_wanaka_21 | 2 | body | -3.26 % | body · chip min 3.3 % under floor · one printed value |
| nz_wanaka_21 | 14 | body | -3.26 % | body · chip min 3.3 % under floor · one printed value |
| nz_wanaka_21 | 15 | body | -2.99 % | body · chip min 3.0 % under floor · one printed value |
| rivmap100 | 1 | helix entry | -89.0 % | helix entry · chip min 89 % under floor · one printed value |
| live_0925 | 1 | helix entry | -47.5 % | helix entry · chip min 47 % under floor · one printed value |

The face line of the Passes card (the worst stated row that exceeds, by
`CheckRow::rank_cmp`): no change on any of the 18 operator ops. The stated
rows hold no `ChipMin` row today (the stated chip band row is `ChipMax`,
`NotJudgedHere`). Before and after:

| idx | Face line |
|---|---|
| 1 | stated point · depth 387 % over force cap (advisory) |
| 2 | stated point · depth 74 % over force cap (advisory) |
| 14 | stated point · depth 73 % over force cap (advisory) |
| 15 | stated point · depth 57 % over force cap (advisory) |
| 0, 3 to 13, 16, 17 | none |

Readiness tool-load rows: no change. They read the gate rows, and the gate
keeps its verdicts (the gate builds the same `ChipBoundsSource`). FM1 and
the op golden: no cell and no row moves (section 6).

## 10. Step PB: measured beside predicted

The same probe ran on the PB tree. "Measured" is the diff of the two dumps.

| Quantity | Predicted | Measured |
|---|---|---|
| `ChipMin` rows that take the caveat (operator, nz_wanaka_21, rivmap100, live_0925) | 60, 44, 13, 13 = 130 | 60, 44, 13, 13 = 130 |
| Tone `Exceeds` to `ExceedsAdvisory` | 10 (the table of section 9) | 10, the same rows |
| State, value or bound changes | 0 | 0 |
| Rows of other limits that change | 0 | 0 |
| Faces that change | 106 | 0 (departure, below) |
| Face line of the 18 operator ops | no change | no change |
| Verdict rows (Readiness) | no change | no change |

The face departure. The first build put the caveat word on the face and
measured 108 face changes against the 106 predicted. The two extra rows were
operator idx 0 and idx 12, the helix entry `ChipMin` row: with the word the
face has 61 characters, so the face builder drops every caveat word ("may
not occur" too). The prediction script kept the old face there; the
prediction was wrong. Then the viz sentry
`the_card_shows_one_planned_row_and_a_drawer_p29_10` failed: the drawer row
"chip min 100 % of floor · one printed value" is clipped at the rail width
(340 points). So the caveat has no face word, as `Caveat::Advisory` has
none: an exceeding row shows the caution tone and the chip word "advisory",
and the hover detail names the reason ("the chip floor is a reference, not
a floor that gates: the vendor row prints one chip value, a point and not a
band (A2)"). The measured face changes are then 0. The lead decides if the
face must carry the word (a shorter word, or a wider layout).

Caveat sets measured: `advisory_floor` alone on 84 rows; with `may_occur`
on 24; with `stored_feed` and/or `worst_case` on the rest.
