# `io/` — the import doors and the TOML libraries

Every door that reads a file. The entry point is `io::load_model_file`.

## Files

- `mod.rs` — import helpers, size guard, `brep_face_items` (P2.1 STEP faces), `default_import_units` (P4.8).
  `import_options.rs` — P4.8 `ImportOptions` (saved), `ImportReport` (not saved), `DrawingImport`.
- `dxf_input.rs` — DXF entities to items, one per entity with its layer (a circle is ONE item); the
  open pieces join through `selection::join` (P1b rule, P4.8 kernel). `svg_input.rs` — SVG paths
  to items; layer = id of the nearest `<g>`; P4.8: subpaths join; `SVG_DEFAULT_UNITS`, `svg_declared_size`.
  O-2: every transform and the declared size apply (CSS units, 96 px = 25.4 mm; unitless = px).
- `step_input.rs` — STEP import through the `truck` crate.
- `tool_library.rs`, `machine_library.rs` — the two TOML catalogues (folder:
  `crate::settings::paths`); `named_toml_library.rs` — their directory code.

## Invariants

- An import states its units and its `ImportOptions`. A reload, rescale and relink pass both. P4.9: the project door replays the edit ledger after the door, on the bound items (`LoadedModel::with_drawing_edits`); the ledger is in the file units of the join tolerance.
- File units: DXF after `$INSUNITS`; SVG in mm after its declared size and every transform. The join, the ledger and the flatten tolerance use them; the user scale comes after.
- A model path is stored as written and resolved on load. A rebind goes through the session command.
- A drill target picked from a DXF resolves in the setup frame. A pick that
  stales must refuse, not guess.
- No door runs `detect_containment` (P2.1); `ItemSet::scaled` keeps nesting.
- Every importer refuses a file over `MAX_IMPORT_FILE_SIZE` before the parser
  reads it. The three doors share `file_size_over_limit` and each keeps its
  own `FileTooLarge` variant.

## Sentries

- `cargo test -p rs_cam_core -q --test step_import --test every_import_door_gives_items_p2_1`
- `cargo test -p rs_cam_core -q --test every_import_door_refuses_an_oversized_file_edg03`
- `cargo test -p rs_cam_core -q --test step_project_load --test model_units_survive_reload_g_unitsreload --test import_reports_joins_and_open_shapes_p4_8 --test drawing_edits_replay_on_reimport_p4_9`
- `cargo test -p rs_cam_core -q --test drill_picks_resolve_to_targets_g_drillpickstale`
- `cargo test -p rs_cam_core -q --test model_path_round_trip_g_modelrelink`

## Do not

- Do not add a second project-file loader. Core refuses `format_version` that
  is not 3.
