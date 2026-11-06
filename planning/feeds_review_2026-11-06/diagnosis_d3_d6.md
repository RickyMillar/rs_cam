# Diagnosis D3/D6: the depth gate feed (2026-11-06)

Read-only diagnosis. No file under `crates/` changed. Evidence runs:
`target/release/rs_cam_cli project <cell>.toml --sim-artifact --emit-gcode`
(binary mtime 2026-11-06 11:49, after the last `crates/` commit ec5cbe4a9
at 08:00). The outputs are in a scratch folder, not in `planning/`.

## 1. Correction to the question

`matrix/gate_mcp.json` does not give fz 0.10963909 to the depth gate in
both cells. P_r37_sienci has `depth_fz_mm` 0.08922248 = 3578 / (20051 x 2),
the commanded feed. Only P_r37_pick has depth fz 0.10963909. The chip gate
has 0.10963909 in both cells.

## 2. Where 4396.6 mm/min comes from

The feed modulator writes it. It is real in the emitted G-code.

- The simulator copies the IR move feed into each sample:
  `dexel_stock/simulation.rs:645` (`MoveType::Linear { feed_rate }`) passes
  `feed_rate_mm_min: feed_rate` (line 696) to `capture_cutting_segment`.
- After the simulation, `session/compute/simulation.rs:349` runs the
  modulator. It rewrites the IR feeds in `self.results` (lines 830-860).
- It does NOT rewrite the samples. It writes the modulated feed into
  `trace.predicted_feeds` (`session/compute/simulation.rs:1034-1036`).
- The CLI G-code for all three cells says
  `(feeds: modulated from simulation run 1)`. P_r37_pick and P_r37_sienci
  both emit `F4397` on the first ring (move 3) and `F4396` on move 6.
  P_thin_005 emits `F2615` on move 3 (sample feed 5999).

Same tool, same rpm, same binding limit: the modulator puts both r37 cells
at 4396.6 mm/min. That is why the chip gate reads 0.10963909 in both.

## 3. Which sample each gate reads

- Depth gate: `cut_sample` is the first steady-state sample with the
  largest `axial_engagement_mm` (`tool_load/depth.rs:188-194`, `227`).
  In the CLI trace this is move 3 (P_r37_pick sample 34+, P_r37_sienci
  sample 35, P_thin_005 sample 36), the first slot ring after the plunge.
- Depth gate feed: `s.feed_rate_mm_min / (rpm x z)`
  (`tool_load/depth.rs:262-266`). This is the sample field, the
  PRE-modulation feed.
- Chip gate feed: `effective_feed_for_sample` (`tool_load/mod.rs:72-83`),
  through `achieved_advance_per_tooth` (`tool_load/display.rs:39-48`) and
  `peak_chip_per_tooth` (`display.rs:83-97`). It reads `predicted_feeds`
  first, so it reads the MODULATED feed.

The two gates read different feed sources for one sample.

## 4. P_thin_005

The peak-depth sample (move 3, sample 36, depth 6.0 mm) has
`feed_rate_mm_min` 5999, the pre-modulation feed. The G-code emits 2615 on
that move. The depth gate fz 0.29133 = 5999 / (10296 x 2) is a feed that
the machine never runs.

## 5. P_r37_pick in the GUI

The GUI samples carry 4396.6 and the modulator touched 0 moves. The likely
cause: the GUI simulated the already-modulated IR a second time. The
samples then copy 4396.6 from the IR, and the second modulation finds
nothing to change. The depth gate agrees with the chip gate here by
accident. This is an inference; the CLI trace (one simulation) has 3565.

## Verdict

The 4396.6 mm/min feed is real: the modulator emits it as F4397 in the
G-code of both r37 cells. The defective side is the depth gate. It reads
`SimulationCutSample.feed_rate_mm_min`, the feed of the simulation run,
and not `effective_feed_for_sample`, which the chip, power and deflection
gates read. After one simulation, the depth gate grades the pre-modulation
feed (P_r37_sienci 3578, P_thin_005 5999). After a second simulation of the
modulated IR, it grades the modulated feed (P_r37_pick 4396.6). The same
project can thus get two depth verdicts.
