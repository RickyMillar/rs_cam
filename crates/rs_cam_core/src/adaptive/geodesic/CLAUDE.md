# `adaptive/geodesic/` — Geodesic Adaptive, the 2D plan

The research's candidate (a) with the one preset that ships: C35 plan A
(INTEGRATION_PLAN Q8, D52). Design:
`planning/adaptive_research_2026-10-23/integration/step7_design.md`.

## Files

- `mod.rs` the rules (module doc) and the one door, `geodesic_toolpath`.
- `field.rs` the field `T`: source-tracked solve, front branches,
  local-level spacing, the level sets chained whole.
- `wraps.rs` the chains, the meeting-line trim, the cutting order, the core
  spiral and its spacing law.
- `plan.rs` one piece: starter pocket, field, core spiral, wraps, planned
  turns (`plan_piece`), and the 2D cleanup (`cleaned`). Its
  `#[cfg(feature = "research")]` door `research_plan_piece` is research
  test support only (`../research_geodesic.rs`).
- `tests.rs` unit tests of the field, the chaining, the spacing, the
  star test and the core's loops.

## Invariants

- No options: the module plans C35 plan A only. A variant is research
  work (`../research_geodesic.rs`, `research` feature), not a switch here.
- Every item but the door is private to the module. Code the door does not
  reach does not exist here (no `dead_code` expects).
- The plan holds the 2D pass load through the shared cleanup
  (`../path.rs`); the only exemption is a planned turn no longer than
  `turn_bound`, admitted by `LinkLoad::admits_turn`.
- No typed constant: each rule names its derivation (module doc).
- The plan equals the research reference move by move (feeds aside); a
  moved pin is a defect, never a re-pin.

## Sentries

- `cargo test -p rs_cam_core -q --features research,test-support --test the_geodesic_preset_keeps_its_plan_s7_3`
  (the reference pins on the star and the pocket, and the door against the
  reference).
- `cargo test -p rs_cam_core -q --lib adaptive::geodesic` (the unit tests).
- The 2D engine's sentries in `../CLAUDE.md` (the cleanup is shared).
