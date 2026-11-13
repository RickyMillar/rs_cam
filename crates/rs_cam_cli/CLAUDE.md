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
- Generated G-code must use the core export path, including coolant. F067:
  `project --emit-gcode` and `run` take `ProjectSession::export_gcode_checked`
  (the machine-safety pass, the rapid rewrite). K6 (ruling Q6): no finding
  refuses and no flag overrides one; the program header and `summary.json`
  `warnings` name each finding. Only an `EmitError` (`emit.*`) stops the file.
- Treat a missing diagnostic row as absent evidence, not a clean verdict.
  A blocked operation is LISTED in `summary.json` under
  `awaiting_prior_stock`, never left absent.
- Generation order is core's: walk `session::generation_plan::plan`. Do not
  hold a CLI ladder or a cap.
- The simulation cell size is the project file's stored value
  (G-RESTRES, ruling 2026-09-24); a rest cascade needs no `--resolution`.
  `--resolution` overrides it through `SetSimulationResolution`, and
  `summary.json` says `override`. A cell too coarse for the rest is refused
  with `ProjectSession::rest_resolution_refusal`, the sentence MCP gives.
- One vocabulary per concept across `job` TOML, `run` and MCP. A tool
  type is `ToolType`'s own serde token (`end_mill`, `ball_nose`,
  `bull_nose`, `v_bit`, `tapered_ball_nose`) on every surface; do not
  add a CLI-only spelling. K3 (feeds contract P6): a job tool's facts (`flute_count`, `taper_angle`, `shank_diameter`, `stickout`, `centre_cutting`, `v_tip_diameter`) take the project-file forms (`{ stated = .. }`, catalogue, estimated; an absent key is `Unknown`); a bare value and the old key `non_centre_cutting` refuse in one message (`check_job_tool_facts`); the flute length is `Estimated` (`DEFAULT_CUTTING_LENGTH_FACTOR`). `run --tool-set centre_cutting=<JSON>` uses the generic setter. D12: an op whose tool does not state `centre_cutting` (and a V-bit `v_tip_diameter`) stops with the core reason.
- A named machine profile comes from `io::machine_library`, the library
  the GUI and MCP share. Do not add a hardcoded preset to a subcommand.
  The one exception is `nc-time` with no `--machine`: it keeps the
  built-in `shapeoko_xxl_ricky_tuned` preset (CLI-06, `nc_replay.rs`).
- C2: a job `[[operation]]` reads the W5 schema. `type` is the registry
  token, every other key that is not in `OperationDef::JOB_KEYS` is a
  schema parameter, and `[operation.dressups]` holds `DressupConfig::FIELD_DEFS`
  names. The core setters apply them in schema order. Do not add a CLI
  spelling, a per-op map or a default number: S11 parity, a job op (and
  `run`) is the core draft (`ProjectSession::draft_toolpath` with
  `DraftRequest::new(..).tool(..).model(..)`, both exact because the job
  names its tool and its model; the GUI and MCP door), an unset key keeps the draft value, a value that no rule
  gives stays unset and the op refuses as on the GUI, and
  `parse_job_file` refuses an unknown key. The job key `stock_z` (`run
  --stock-z`) states the thickness of a drawing stock. The job key
  `selection` (P2.7) has the project-file shape and goes through `EditSelection`;
  F7: the job key `boundary` (the project-file boundary block, `SetBoundaryConfig`) and
  `boundary_selection` (the selection shape, `EditSelection` with `target: Boundary`, after `boundary`;
  a boundary that is not a selection refuses with the MCP text);
  the job key `floor` (P3.3) has the project-file shape and goes through `SetToolpathFloor`.

- `rough-score` labels an entry run by its strongest intent (`EntryHelix` >
  `EntryRamp` > `LeadIn` > `EntryPlunge`); `entry_runs_led_by_plunge` counts
  the ramp and helix runs that start with a plunge (FC0).
- Feeds contract P5 (K2b): `project --apply-suggest` applies `Command::ApplyFeeds` with no
  pick, so it writes the speeds for the cut each op keeps; `--apply-suggest-pick proposal`
  also writes the envelope proposal first. The apply logs each Unsourced rule it accepts and
  each outside-envelope warning. `suggest` prints `envelope` (the `get_safe_envelope` JSON)
  and `profile.vendor_band` (P16 item 8; `basis.tied_rows` deleted). P29-10: `suggest` prints
  `limit_rows` (the RPM rows) and `project` writes `summary.json` `per_toolpath[].limit_rows` (the
  verdict rows), both from `ProjectSession::limit_rows`, the door MCP reads. P29-10z: `suggest` runs no simulation, so its rows state the default run (`SimulationOptions::default().run_modulation()`, printed as `run_modulation`); `project` states the run of its flags.
- `suggest` prints `peck_depth` for a Drill and a Pin Drill. `shipped_power`
  reads the calculator point where the operation has no stepover, depth or
  rpm, as the GUI does; `point_source` names the source of each value. That
  figure is a default cut, not the cut of the operation (FC0).

## Verification and sweeps

- Dev test: `cargo test -p rs_cam_cli -q`.
- The full pipeline sweep command is
  `cargo run -p rs_cam_cli -- sweep job.toml --param X --values "..." --output-dir out/`.
- Sweep mechanics/fingerprints are implemented in core; keep CLI output and
  core fingerprints aligned. See `toolpath_stress_test/agents/` for existing
  analysis tooling.
- A CLI claim about shipped capability must be checked against
  `FEATURE_CATALOG.md` and current core behaviour, not an old planning report.
