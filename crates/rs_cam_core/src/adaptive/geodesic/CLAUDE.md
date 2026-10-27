# `adaptive/geodesic/` — Geodesic Adaptive, the 2D plan

The research's candidate (a), preset C35 plan A only (Q8, D52). Design:
`planning/adaptive_research_2026-10-23/integration/step7_design.md`.

## Files

- `mod.rs` the rules (module doc) and the 2D door, `geodesic_toolpath`.
- `field.rs` the field `T`: source-tracked solve, front branches,
  local-level spacing, the level sets chained whole.
- `wraps.rs` the chains, the meeting-line trim, the cutting order, the core
  spiral and its spacing law.
- `plan.rs` one piece: starter pocket, field, core spiral, wraps, planned
  turns (`plan_piece`), the 2D cleanup (`cleaned`); the research door
  `research_plan_piece` (`research` feature, test support only).
- `slice.rs` the 3D slice door (`../../adaptive3d/clearing.rs`): the piece
  plan, its cleanup and replay, the held links, and the feed caps
  (`feed_caps`, design part F: each turn's cap on its own cut).
- `tests.rs` unit tests of the field, chaining, spacing, star and core.

## Invariants

- No options: C35 plan A only; a variant is research work, not a switch.
- The doors are `geodesic_toolpath` and the `pub(crate)` slice items; the
  rest is private. Unreached code does not exist (no `dead_code` expects).
- A cap reads `slice::step_reading`: the larger of the swept width and the
  area per mm (`L`); no matching of moves by position.
- The plan holds the 2D pass load through the shared cleanup
  (`../path.rs`); the only exemption is a planned turn no longer than
  `turn_bound`, admitted by `LinkLoad::admits_turn`.
- No typed constant: each rule names its derivation (module doc). The plan
  equals the research move by move (feeds aside); a moved pin is a defect.

## Sentries

- `cargo test -p rs_cam_core -q --features research,test-support --test the_geodesic_preset_keeps_its_plan_s7_3`
  (the reference pins on the star and the pocket; the door against them).
- `cargo test -p rs_cam_core -q --lib adaptive::geodesic` (the unit tests).
- `... --test geodesic_adaptive_keeps_the_research_geometry_s7_4 --test a_geodesic_cap_survives_every_dressup_s7_4`
- The 2D engine's sentries in `../CLAUDE.md` (the cleanup is shared).
