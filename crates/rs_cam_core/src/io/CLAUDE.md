# `io/` — the import doors and the TOML libraries

Every door that reads a file. The entry point is `io::load_model_file`.

## Files

- `mod.rs` — import helpers, size guard, `brep_face_items` (P2.1 STEP faces).
- `dxf_input.rs` — DXF entities to selection items, one per entity with its
  layer (a circle is ONE item). P1b: `chain_open_pieces` joins the open
  pieces of one layer; a chain back at its start is a loop.
- `svg_input.rs` — SVG paths to items; layer = id of the nearest `<g>`.
- `step_input.rs` — STEP import through the `truck` crate.
- `tool_library.rs`, `machine_library.rs` — the two TOML catalogues (folder:
  `crate::settings::paths`); `named_toml_library.rs` — their directory code.

## Invariants

- An import states its units. A reload must preserve them.
- A model path is stored as written and resolved on load. A rebind goes
  through the session command, not through a path edit here.
- A drill target picked from a DXF resolves in the setup frame. A pick that
  stales must refuse, not guess.
- No door runs `detect_containment` (P2.1); `ItemSet::scaled` keeps nesting.
- Every importer refuses a file over `MAX_IMPORT_FILE_SIZE` before the parser
  reads it. The three doors share `file_size_over_limit` and each keeps its
  own `FileTooLarge` variant.

## Sentries

- `cargo test -p rs_cam_core -q --test step_import --test every_import_door_gives_items_p2_1`
- `cargo test -p rs_cam_core -q --test every_import_door_refuses_an_oversized_file_edg03`
- `cargo test -p rs_cam_core -q --test step_project_load`
- `cargo test -p rs_cam_core -q --test model_units_survive_reload_g_unitsreload`
- `cargo test -p rs_cam_core -q --test drill_picks_resolve_to_targets_g_drillpickstale`
- `cargo test -p rs_cam_core -q --test model_path_round_trip_g_modelrelink`

## Do not

- Do not add a second project-file loader. Core refuses `format_version` that
  is not 3.
