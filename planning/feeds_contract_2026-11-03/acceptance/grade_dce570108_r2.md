# Grade of build dce570108 (P21c), second run after the operator rulings of 2026-11-10

Same build and same app values as `grade_dce570108.md` (the APP dict did not change; no MCP
call in this run). The table changed by the six operator rulings of the afternoon only. The
build note of `grade_dce570108.md` applies.

- Grade: PASS 92, FAIL 6, NEEDS-RULING 0, N/A 10 (108 rows).
- Before the rulings: PASS 64, FAIL 6, NEEDS-RULING 28, N/A 10.
- Status counts: DERIVED 89, SOURCED 6, UNSOURCED 2, POLICY 1, N/A 10 (before: DERIVED 58,
  UNSOURCED 33). The 2 UNSOURCED rows are the unmodeled power rows of idx 5 and 9; the POLICY
  row is the idx 16 feed (P4).

## Table changes (each from a ruling recorded in the header)

| Ruling | Rule in the script | Source keys | Value |
|---|---|---|---|
| U1 | SPK37 chip x (18 / 12.7)^0.61 | SPK37, SIZEC | 0.17910 mm |
| U3 tool 6 | 1000 / h x clamp(18/6, 0.25, 3), h = (1360 / 600)^0.4; min with Z 2500 | PLGTXT, PLGBASE, PLGH, PLGJANKA, MACH | 2163 mm/min |
| U2 chip | log-log between SPK12 and SPK17 at 2.0 mm | SPK12, SPK17, SIZEA | 0.05501 mm |
| U2 plunge (tool 5) | the same plunge rule, clamp(2/6) | as U3 | 240 mm/min |
| U3 tool 1 | the same plunge rule, clamp(3.175/6) | as U3 | 381 mm/min |
| U3 tool 4 | the same plunge rule, clamp(2/6); tip cap 150 x 2.0 = 300 | as U3, PLGTIP | 240 mm/min |
| H1 | the ramp rule (chip x z x n, lift 1) grades the helix entry feed | - | 8597 mm/min at 24000 |

The deleted candidates: the tool 6 bracket 0.1448-0.2286, the tool 5 bracket 0.0508-0.0584,
the tool 5 plunge `n x chip` (1219-1402), the tool 6 plunge bound 0-2500, the empty candidate
sets of the tool 1 and tool 4 plunge, the helix "no rule" row. Each plunge row says "repo
rule, no vendor source". The plunge constants quote product source lines (the repo marks
them `FormulaConstant::unsourced`).

## Rows that changed grade

NEEDS-RULING to PASS, 25 rows:

- chip: idx 0, 1, 11, 12, 14 (expected 0.17910, app 0.17913); idx 6, 7, 8 (0.05501, 0.05501).
- feed: idx 0, 12 (8651, 8651); idx 11 (8597, app 8651, +0.6 %); idx 7, 8 (1320, 1320).
- ramp or helix entry feed: idx 0, 1, 12, 14 (8597, 8597); idx 8 (1320, 1320).
- plunge: idx 0, 1, 11, 12, 14 (2163, app 2162); idx 9 (381, 381); idx 16 (240, 240).

FAIL to PASS, 3 rows: plunge of idx 6, 7, 8 (expected 240, app 240).

NEEDS-RULING to FAIL, 3 rows (the bracket hid each difference):

- idx 1 feed: expected 8599, app 9332 (+8.5 %). 9332 / 8597 = 1.0855, the lift at the nominal
  stepover 5.5 mm. P3 ruled the cap width 8.8 mm (lift 1.0002).
- idx 14 feed: expected 9215, app 10747 (+16.6 %). 10747 / 8597 = 1.250, the lift at the
  nominal stepover 3.6 mm. The cap width 5.76 mm gives lift 1.0719.
- idx 6 feed: expected 1320, app 1347 (+2.0 %). The app lifts at ae 0.8 mm in a pocket whose
  first ring is a slot (check C3).

## FAIL rows (6)

- P3 lift at the nominal stepover: idx 1 feed, idx 14 feed. The same lift is on idx 2 and 15
  (7039 / 6096 = 1.1547 = the lift at 1.5 mm); those rows pass only because the Onsrud band
  0.127-0.178 mm is wide.
- Lift in a pocket that slots: idx 6 feed.
- idx 16 chip: 0.06437 against SpeTool 0.0762; idx 16 feed: 3089 against 3658 (P4a) or 6096
  (P4b); P4 is open.
- idx 5 feed: stored 3165 against 6096-8534 (Suggest prints an RPM and no feed).

## NEEDS-RULING rows

None.

## Findings (not edited)

1. Two Janka values for white oak are in the repo: 1360 in `material/mod.rs` line 153 (the
   plunge rule reads it) and 1348.9 in `data/wood_species.toml` line 379 (FPL). With 1348.9
   the tool 6 plunge is 2170, not 2163 (+0.3 %, inside the tolerance).
2. The `1000.0`, `600.0`, `0.4`, `0.25`, `3.0` and `150.0` constants of the plunge rule are
   marked unsourced in the product source ("ported from reference/ (not in the repo)"). The
   table quotes the code line, as the lead instructed.
3. The U2 chip is a 2F chip on a 1F tool. The app says `winner_outside: "flutes"`. The ruling
   accepts it; no source confirms it.
