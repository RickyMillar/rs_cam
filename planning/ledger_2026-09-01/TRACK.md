# Track G — the strategy ledger

The operator asked: "I'm not sure what parts are gaining my time and what
are losing me", and "how does [the tiered stack] compare to a spiral over
the whole area in one go with the new spiral method".

The deliverable is ONE table on the real wanaka job
(`planning/multitool_2026-08-23/wanaka200_mt2.toml`). The table prices
every layer of the finishing stack against its simpler alternative,
with a whole-board spiral included.

## The arms

All arms are single-setup, front finish territory. The tool is the
R1.0 tapered ball (tool_id 2) unless the arm states a second tool.

- **A** — plain `Scallop` op, whole board, R1.0. CLI variant:
  `wanaka200_armA_scallop.toml`.
- **B** — one `unified_finish` op, whole board, R1.0, no planned-tier
  boundary. CLI variant: `wanaka200_armB_unified.toml`.
- **C** — production two-tool tiers (R1.5 + R1.0), the unchanged
  `wanaka200_mt2.toml`, re-run on this worktree so every CLI arm shares
  one binary.
- **D1** — whole-board EDT spiral, spec-honest ring spacing (the derate
  rule in FINDINGS.md). Harness instrument.
- **D2** — whole-board EDT spiral, XY spacing at the flat-law stepover
  (the legacy raster's spacing convention). Harness instrument.
- **CAL** — the shipped `unified_finish` generator driven in-harness on
  the same mesh, costed by the harness integrator. This calibrates the
  harness scale against the CLI scale for arm B.

## Method

- CLI arms: `rs_cam_cli project <toml> --resolution 0.3`. The runtime
  wire is `total_runtime_s` plus the cut trace `toolpath_runtimes` —
  the same wire the Track B wanaka table read.
- Harness arms: `crates/rs_cam_core/tests/whole_board_spiral_ledger_g1.rs`
  (`#[ignore]` evidence instrument). Rings come from the closed-form
  rectangle EDT of the board outline. `spiral_finish_compact::
  bridge_nested_levels` joins them. Every point re-drops through the
  drop-cutter (the C1 gouge convention) with the production
  TaperedBallEndmill (ball Ø2.0, taper 5.7°, shaft Ø6.0). Costing is
  the C1/F-034 relink block: hookup 25.0, sampling 0.5, reorder,
  `link_ceiling: None`, the project's machine kinematics.
- The two integrator scales are NOT one column. FINDINGS.md carries the
  calibration and the caveat.

## Files

- Pre-registration + results: `planning/ledger_2026-09-01/FINDINGS.md`
- Variant TOMLs: beside this file.
- Instrument: `crates/rs_cam_core/tests/whole_board_spiral_ledger_g1.rs`
