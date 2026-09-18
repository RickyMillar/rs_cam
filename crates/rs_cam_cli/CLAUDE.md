# rs_cam_cli instructions

`rs_cam_cli` owns batch commands, job-file execution, reports, replay and
parameter-sweep entry points. Read root `CLAUDE.md` first and
`../rs_cam_core/CLAUDE.md` for engine/simulation semantics.

## CLI contract

- Route mutations and export through the same `ProjectSession` command/core
  doors used by the GUI. Do not add CLI-only semantics that drift from the
  embedded application.
- Reports must preserve core evidence states: current vs stale, measured zero
  vs not measured, and planned vs emitted feeds provenance.
- Generated G-code must use the core export path, including coolant,
  stale-geometry refusal and explicit safety overrides.
- Treat a missing diagnostic row as absent evidence, not a clean verdict.
  A blocked operation is LISTED in `summary.json` under
  `awaiting_prior_stock`, never left absent.
- Generation order is core's: walk `session::generation_plan::plan`. Do not
  hold a CLI ladder or a cap.
- The simulation cell size is refused, never defaulted, when the plan
  simulates. `compute::config::REST_NEEDS_RESOLUTION` is the one sentence
  this command and the MCP refusal both say.
- One vocabulary per concept across `job` TOML, `run` and MCP. A tool
  type is `ToolType`'s own serde token (`end_mill`, `ball_nose`,
  `bull_nose`, `v_bit`, `tapered_ball_nose`) on every surface; do not
  add a CLI-only spelling.
- A machine profile comes from `io::machine_library`, the library the
  GUI and MCP share. Do not hardcode a preset in a subcommand.

## Verification and sweeps

- Dev test: `cargo test -p rs_cam_cli -q`.
- The full pipeline sweep command is
  `cargo run -p rs_cam_cli -- sweep job.toml --param X --values "..." --output-dir out/`.
- Sweep mechanics/fingerprints are implemented in core; keep CLI output and
  core fingerprints aligned. See `toolpath_stress_test/agents/` for existing
  analysis tooling.
- A CLI claim about shipped capability must be checked against
  `FEATURE_CATALOG.md` and current core behaviour, not an old planning report.
