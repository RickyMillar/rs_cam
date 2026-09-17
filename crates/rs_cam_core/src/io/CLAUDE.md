# `io/` — the import doors and the TOML libraries

Every door that reads a file. The entry point is `io::load_model_file`.

## Files

- `mod.rs` — the model import helpers `rs_cam_viz` and `rs_cam_mcp` share,
  and the file-size guard every importer calls.
- `dxf_input.rs` — DXF entities to `Polygon2`, including POINT and circle
  centres for drill targets.
- `svg_input.rs` — closed SVG paths to `Polygon2`.
- `step_input.rs` — STEP import through the `truck` crate.
- `tool_library.rs`, `machine_library.rs` — the two on-disk TOML catalogues
  you import from.
- `named_toml_library.rs` — the directory mechanics both libraries share.

## Invariants

- An import states its units. A reload must preserve them.
- A model path is stored as written and resolved on load. A rebind goes
  through the session command, not through a path edit here.
- A drill target picked from a DXF resolves in the setup frame. A pick that
  stales must refuse, not guess.
- Every importer refuses a file over `MAX_IMPORT_FILE_SIZE` before the parser
  reads it. The three doors share `file_size_over_limit` and each keeps its
  own `FileTooLarge` variant.

## Sentries

- `cargo test -p rs_cam_core -q --test step_import`
- `cargo test -p rs_cam_core -q --test every_import_door_refuses_an_oversized_file_edg03`
- `cargo test -p rs_cam_core -q --test step_project_load`
- `cargo test -p rs_cam_core -q --test model_units_survive_reload_g_unitsreload`
- `cargo test -p rs_cam_core -q --test drill_picks_resolve_to_targets_g_drillpickstale`
- `cargo test -p rs_cam_core -q --test model_path_round_trip_g_modelrelink`

## Do not

- Do not add a second project-file loader. Core refuses `format_version` that
  is not 3.
