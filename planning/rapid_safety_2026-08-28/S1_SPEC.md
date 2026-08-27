# S1 instrument spec — shipped-G-code rapid replay

> Spec for `crates/rs_cam_core/tests/rapid_replay_shipped_gcode_s1.rs`
> (evidence instrument, `#[ignore]`). Companion to `PLAN.md` Phase S1.
> Facts marked `[RECON]` are filled from `S1_RECON.md`.

## Question the instrument answers

Does any RAPID (G0) segment in the two shipped programs
(`planning/airrun_2026-08-19/wanaka200_1_Setup_1.nc`,
`wanaka200_2_Setup_2___front.nc`) intersect material that existed at the
moment the rapid executed?

The detector is `dexel_stock::max_clearance_tip_z_for_profile` — NOT
`rapid_collision_count`, which is the instrument under indictment (S-b,
circular).

## Verdict vocabulary (PLAN.md tri-classification)

- **STRIKE** — fine-tier confirmed: tip z below required clearance.
- **NEAR-MISS** — no strike, but material inside the swept envelope disc
  within a stated vertical margin (the blind-radius class the point probe
  cannot see).
- **CLEAN** — nothing close.

## Design decisions (settled with advisor 2026-08-28)

1. **Two margins per flagged rapid — kerf-graze classification.** Drill peck
   re-entries and slot re-entries ride their own kerf: radial clearance is
   exactly zero, so a conservative envelope-radius probe reads the un-cut rim
   as a wall. Report margin at `r = envelope` AND `r = envelope −
   cell_diagonal`. Clears at the shaved radius → `KERF-GRAZE (benign by
   construction)`. Persists → real candidate. Never fix false positives by
   globally shrinking the probe radius — that deletes the sliver sensitivity
   the instrument exists for.
2. **Frame from code, not prose.** The exported-frame transform comes from
   the exporter's own contract ([RECON] Q1 /
   `tests/export_datum_setup_frame.rs`), not from RUN_LOG tables. Empirical
   tripwires INSIDE the instrument, asserted not eyeballed:
   - (a) op 1's pin drill lands at the pin's transformed position;
   - (b) every op's replay removes material > 0 mm³;
   - (c) every cutting move falls inside the transformed stock box
     (+ small tolerance).
   A wrong Z anchor reads as all-air or all-buried instantly.
3. **Two-tier: coarse flags, fine adjudicates.** Coarse full replay at
   0.3 mm can only bin rapids into skip / flag (its half-diagonal
   conservatism is ±0.21 mm — sub-cell "strikes" are discretisation). Only
   the fine tier (0.05 mm, windowed) may say STRIKE. Fine window = rapid
   segment bbox ⊕ (tool envelope radius + 2 mm); stamp all PRIOR cutting
   moves intersecting the window, then probe.
4. **Early-out.** Before any disc probe: track current stock global max-z;
   skip any rapid whose lowest tip z along the segment clears global max-z
   by > flag threshold. Kills ~95% of Setup 2's 12,236 G0s.
5. **Setup 2 initial stock: conservative full box.** Errs toward FINDING.
   Expected adjudication burden: front rapids over back-side through-features
   (pins, pilot holes). Only build replay-Setup-1-then-flip if strikes land
   in back-machined volume.
6. **Commit the instrument when it lints clean, before the evidence run.**
   A red result is information.

## Parser scope

Modal parser, exactly: `G0/G1/G2/G3` with `X Y Z I J F`; `G17 G21 G90 G54`
accepted and ignored; `M` words ignored; `(comment)` lines carry structure:

- `(LOAD: <tool name> [Tn])` / `(TOOL CHANGE: <tool name> [Tn])` — tool
  name matches toml `[[tools]] name` verbatim [RECON Q2e].
- `(N <op name>)` — toolpath boundary; names match toml ops (parenthesis
  style differs: toml `(…)` vs nc `[…]` — match on the leading index N).

No G8x canned cycles (census verified: pecks are pre-expanded G0/G1).
I/J semantics per [RECON Q2a]; handle general 3D segments regardless.

Empirical census (2026-08-28, awk over both files): **zero** G0 blocks
combine XY and Z motion — Setup 1: 1042 xy-only / 1494 z-only / 0 combined;
Setup 2: 4686 / 6972 / 0. So every descent is a pure vertical drop at fixed
XY: one disc query per descending rapid, not a swept corridor. XY-only
traverses still sweep, but run at height where the early-out clears them.

Sampling: along-segment step ≤ half a fine cell for the fine tier; ≤ half a
coarse cell for the coarse tier.

## Report

Evidence table to stderr (file-level `#![allow(clippy::print_stderr)]`,
same opt-in as `power_ceiling_parity_f2.rs`): per flagged rapid — op, nc
line, tool, tier, margin@envelope, margin@shaved, classification. Headline:
tri-classification counts per op, then the single worst rapid per op.
Skip-if-missing guard on both `.nc` files and the toml (same pattern as
`thin_organic_island_widths.rs`).

Results land in `planning/rapid_safety_2026-08-28/S1_RESULTS.md` — written
by the orchestrator after the run, honouring the "report latent plainly,
don't talk it up" clause.

## Instrument notes — deviations taken while implementing

Written by the implementing agent, 2026-08-28. Everything below departs from
the text above; each says why.

1. **The fine window is intersected with the stock box in XY.** The spec says
   the window is `segment bbox ⊕ (envelope radius + 2 mm)` and stops there.
   But `TriDexelStock::from_bounds` fills **every** cell with material, so an
   unclamped window around a rapid near a board edge invents phantom stock
   outside the board and manufactures strikes there. `fine_window` therefore
   clamps to `SetupSpec::stock` and abstains (`WindowOffStock`) when the
   intersection is empty. Reported, never silent.

2. **Shaved radius = envelope − 1.5 fine-cell diagonals**, not one (decision 1
   says "− cell_diagonal"). At 0.05 mm cells one diagonal is 0.071 mm, and the
   probe's own half-cell dilation (`reach = radius + cs*0.5`) puts visited
   cells back out to 2.954 mm against a 3.000 mm kerf wall — 0.011 mm of
   slack, inside the cell quantisation it is trying to see past. 1.5 diagonals
   gives 0.046 mm. `SHAVE_CELL_DIAGONALS` is a named constant; if the peck
   re-entries still read `STRIKE` with `margin_shaved ≈ −(peck step)`, raise it
   rather than raising the primary probe's radius.

3. **The active tool is tracked per MOVE, not per op.** The spec's parser
   section implies one tool per toolpath section. The shipped files put the
   op-boundary comment **before** the tool change
   (`wanaka200_1_Setup_1.nc:12158` `(4 Rivers [back, V-bit])`, `:12160`
   `(TOOL CHANGE: … [T20])`), so an op-level tool would attribute op 4 to T1.
   `OpSection::tool` is back-filled from the section's first move.

4. **The early-out uses the analytic stock top, not a tracked global max-z**
   (decision 4 says "track current stock global max-z"). Required clearance is
   `max over cells of (conservative_top − height_at_radius)`, and
   `conservative_top` starts at `bbox.max.z` and only ever lowers while
   `height_at_radius ≥ 0` — so required clearance can never exceed the stock
   top. Comparing against the constant is therefore **exact**, not an
   approximation, and costs nothing per rapid.

5. **Fine-tier budget.** 400 adjudications, flagged rapids sorted
   worst-coarse-margin-first so the cap keeps the strongest candidates. The
   number dropped and the worst dropped margin are both printed.

6. **Rapids before the machine position is established** (the leading Z-only
   safe-Z retract, which spells no XY) are counted and skipped rather than
   probed against a fabricated start point.

7. **Run it `--release`.** The coarse pass stamps ~460k segments for setup 2
   and the fine tier rebuilds a windowed 0.05 mm grid per adjudication.

## v2 — retract exemption (2026-08-28)

**The first run's 651 STRIKEs were retract-start artefacts.** The orchestrator
read the raw G-code around the ten worst (setup 2 nc:9610/2361/2119/2720/2929/
10083/10479/4976, setup 1 nc:12882/5193) and **all ten** were the same shape: a
Z-only *ascending* `G0 X.. Y.. Z12` / `Z30` straight up out of the position the
preceding fed move had just cut. The along-segment sweep's minimum margin was
landing on the retract's **buried start point** — a point the cutter was
already legitimately occupying.

**Why the exemption is a proof, not a tolerance.** Every tool in this job has a
radius-monotone profile (`height_at_radius` non-decreasing in `r`: flat is
constant 0, the V-bit is `r/tan(half)`, the three tapered balls are ball-then-
cone). For such a profile the tool body at tip `z1` occupies, at each radius
`r`, the column `[z1 + h(r), ∞)`. With the same XY and `z1 > z0` that is a
strict **subset** of `[z0 + h(r), ∞)` — the space the cutter already occupied
at the start of the move, which the preceding fed move validly occupied. A pure
ascent can therefore only ever vacate material, never enter it. This covers the
CUTTER; a holder or shank strike above the cutting length on a retract is real
and is Phase S4's question (`PLAN.md` S-c), deliberately outside this
instrument's.

**Consequences for reading the report.**

- `RapidKind::RetractAscent` (fixed XY, rising tip) is counted per op and in the
  headline, and is **never probed**. **`STRIKE` now means a DESCENDING or
  TRAVERSING rapid only.** Descents keep the existing evaluation — for a pure
  descent the min-margin sample IS the destination, which is the right
  question. A rapid that changes XY *and* rises still sweeps as a traverse
  (the census says none exist; the monotonicity argument does not cover
  lateral motion).
- `MAX_FINE_ADJUDICATIONS` raised 400 → 20,000. The budget was being spent on
  retract artefacts; with those exempt the real population fits and nothing
  that could be a strike is dropped. The cap and its drop-count reporting are
  unchanged.
- The headline now prints a **STRIKE depth histogram** over `margin@shaved`,
  binned `(-0.05, 0]`, `(-0.15, -0.05]`, `(-0.5, -0.15]`, `(-1.0, -0.5]`,
  `<= -1.0`, labelled against the instrument's own conservatism budget
  (~0.10–0.15 mm at the fine tier: cell half-diagonal 0.035 + half-cell disc
  dilation 0.025 + `conservative_top` sub-cell over-read up to one 0.05 mm
  cell). A strike in the first band is at the noise floor; only the last two
  bands are deeper than anything discretisation can manufacture.
- Every strike / near-miss detail line and every per-op worst-rapid line now
  states its kind (`DESCENT` / `TRAVERSE`), and the per-op table carries a
  `retract` column.
