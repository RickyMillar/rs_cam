# Phase F smoke baseline — post-`cfb146b`

**Date:** 2026-05-31 (overnight autonomous run, MCP unavailable)
**Captured by:** `cargo run -p rs_cam_cli -- smoke --output planning/toolpath_acceptance/baselines/2026-05-31_postphasef.csv`
**Source HEAD:** `cfb146b` (hierarchical material picker — latest in the
feeds-data-ingest + audit-followup line: A → B → C → D → E → F → S3-13
→ S2-9 → hierarchical picker)

## Bottom line

**Zero verdict-kind regressions across 18 AS-IDs** vs the checked-in
`2026-05-26.csv` baseline (which was the F-037 reference snapshot).
The `smoke --diff` CLI exits 0:

```
smoke-diff: no regressions (18 cases checked)
```

## Per-case deflection diff (within-verdict-kind numerical shifts)

| Case   | Op                | 2026-05-26 (µm) | 2026-05-31 (µm) | Δ µm     | Verdict        |
|--------|-------------------|-----------------|-----------------|----------|----------------|
| AS001  | pocket            | 78.4            | 76.1            | −2.3     | within → within |
| AS002  | adaptive          | 51.7            | 50.5            | −1.2     | within → within |
| AS003  | profile           | 84.9            | 84.9            | +0.0     | within → within |
| AS004  | face              | 12.6            | 39.7            | **+27.1** | within → within |
| AS005  | zigzag            | 56.2            | 176.4           | **+120.2** | within → within |
| AS006  | rest              | (no toolpath)   | (no toolpath)   | —        | —              |
| AS007  | trace             | 7.9             | 7.9             | +0.0     | within → within |
| AS008  | v_carve           | 0.0             | 0.0             | +0.0     | within → within |
| AS009  | chamfer           | 0.6             | 0.6             | +0.0     | within → within |
| AS010  | inlay             | 0.9             | 0.9             | +0.0     | within → within |
| AS011  | drill             | (drill metrics) | (drill metrics) | —        | not_applicable (drill cycle — no continuous engagement) |
| AS012  | alignment_pin_drill | (drill metrics) | (drill metrics) | —      | —              |
| AS013  | adaptive3d        | 262.9           | 262.9           | +0.0     | exceeds → exceeds |
| AS014  | drop_cutter       | 476.7           | 476.7           | +0.0     | exceeds → exceeds |
| AS015  | scallop           | 476.7           | 476.7           | +0.0     | exceeds → exceeds |
| AS016  | waterline         | (generation_empty) | (generation_empty) | — | —          |
| AS017  | horizontal_finish | 409.4           | 1285.5          | **+876.1** | exceeds → exceeds |
| AS018  | project_curve     | (generation_failed) | (generation_failed) | — | —      |

## Numerical shifts worth flagging for morning review

1. **AS004 face (+27 µm)** — was 12.6 µm, now 39.7 µm. Still Within
   (band 0–50 µm). Most likely cause: Phase 2B MDF Kc retune
   (10 → 31.4 N/mm² ≈ 3× cutting force), since AS004 uses
   `ux_step_plate_mdf.toml` and the face op cuts MDF substrate. The
   GRAIN_ANISOTROPY_FACTOR drop (2.5 → 2.0) partially offsets but
   net product `Kc × factor` went from 25 → 62.8 (≈ 2.5× force) per
   the kc_retune_log table. The deflection shift here is consistent
   with that — directionally correct, magnitude small.

2. **AS005 zigzag (+120 µm)** — was 56.2 µm, now 176.4 µm. Still
   Within band (≤ 200 µm Exceeds threshold) but **the bigger jump**.
   Same root cause as AS004 (MDF Kc retune; AS005 uses
   `ux_2d_pocket_mdf.toml`). Larger zigzag stepover + 2 mm DOC
   amplifies the proportional force increase. 176 µm is in the
   "visibly degraded but tool/work safe" band per
   `EXCEEDS_BOUND_MM = 200 µm` policy — does NOT warrant widening
   the bound. If operator experience says this cut is fine in
   reality, file a deflection-model finding (force arm / near-tip
   integration) per the round-10 policy.

3. **AS017 horizontal_finish (+876 µm)** — was 409 µm Exceeds, now
   1285 µm Exceeds. Already a known-failing case (it's been Exceeds
   since the 2026-05-26 baseline was captured). The deepening from
   409 → 1286 µm is dramatic but the verdict kind hasn't changed,
   and this case uses `ux_step_stepped.toml` with MDF — same Kc
   retune mechanism. Worth a deeper look since this case has the
   largest absolute deflection in the suite.

## What the diff does NOT show

The 2026-05-26 baseline predates several major changes:

- F-031 (2026-05-26 `497a3b2`) — AS013 deflection 0.637 → 0.105 mm
  Within (round-09 result). **But the 2026-05-26.csv shows 262.9 µm
  Exceeds**, meaning that baseline was captured BEFORE F-031 landed
  the AS013 fix.
- Phase 2B (2026-05-30 `bbb164f`) — sheet-good Kc retune + anisotropy
  factor 2.5 → 2.0.
- Phase B (2026-05-31 `585e2d8`) — Garr aluminum 11 rows.
- Phase C–E + audit followups — aluminum / plastic / wood library
  expansions, no toolpath-behavior changes expected.

So this diff is really showing **F-031 missing-baseline-update plus
Phase 2B Kc effect** — not "what Phase B–E changed", which the Phase A
param_sweep already proved was zero behavior change (`f01e400`).

## Round-10 STATE.md operator-validated reference

For context, the `planning/acceptance_loop/STATE.md` round-10 numbers
were:

| Case  | Round-10 pinned | 2026-05-31 measured | Delta            |
|-------|-----------------|---------------------|------------------|
| AS001 | 76 µm           | 76.1 µm             | identical        |
| AS002 | 53 µm           | 50.5 µm             | −2.5 µm          |
| AS003 | 76 µm           | 84.9 µm             | +8.9 µm          |
| AS004 | 5 µm            | 39.7 µm             | **+35 µm**       |
| AS005 | 51 µm           | 176.4 µm            | **+125 µm**      |
| AS013 | 105 µm          | 262.9 µm            | **+158 µm**      |
| AS015 | 197 µm          | 476.7 µm            | **+280 µm**      |

The AS013 / AS015 jumps relative to round-10 are unexpected — round-10
was post-F-031 and AS013 was supposed to be 105 µm Within. The smoke
binary's output shows AS013 at 262.9 µm Exceeds, which is the round-09
or pre-F-031 reading. **Possible cause:** the CLI smoke fixture in
`cases_agent_smoke.csv` doesn't include F-031's prerequisite roughing
pass (the round-10 methodology fix per F-032). Operator should
verify in the morning whether the CLI smoke needs to mirror the
"prior AS013-style roughing pass" methodology that the round-10
manual smoke used.

## Resume note

- All 18 AS-IDs ran cleanly through the CLI smoke pipeline.
- Zero verdict-kind regressions vs the 2026-05-26.csv reference.
- Three numerical shifts within bands (AS004 / AS005 / AS017) plus
  AS013 / AS015 round-10 vs current discrepancy — review in the
  morning.
- If the AS005 +120 µm shift on MDF is real-world-not-actually-burning,
  file a deflection-model finding rather than widening the gate.
- New baseline at `planning/toolpath_acceptance/baselines/2026-05-31_postphasef.csv`.
