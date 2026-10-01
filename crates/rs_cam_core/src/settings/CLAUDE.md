# `settings/` — the app settings file

`settings.toml`: the ONE loader and the ONE writer for the GUI and the CLI.
File ▸ Preferences writes it (operator request 2026-10-02).

## Files

- `mod.rs` — `AppSettings` and its sections (`[general]`, `[display]`,
  `[simulation]`, `[paths]`, `[diagnostics]`, plus `memory_limit`), the
  token enums, `limits`, `parse_lenient` / `parse`, `load` / `load_from`,
  and the writer `merged_text` / `save_to`.
- `paths.rs` — the config, cache and settings paths, the library folders
  (env > `[paths]` > default), the artifact folder, and the process cell
  `install_paths` / `installed_paths`.
- `tests.rs` — the round trip of every key, unknown keys, bad values.

## Invariants

- A missing key is the behaviour before the key existed. The one change
  is `save_cut_trace`: OFF (operator ruling 2026-10-02).
- A bad value gives the default for THAT key and a warning; a file that is
  not TOML gives all defaults. The writer refuses a file that is not TOML.
- The writer keeps every key and table it does not own, writes through a
  temporary file and a rename, and removes an owned key at its default.
  `[memory] limit` is always written.
- `[memory] limit` text belongs to `crate::budget::settings`.
- No key here changes a computed number. A feed, timing or instrument
  switch does not belong in this file.
- Every library read goes through `paths::tool_library_dir` /
  `machine_library_dir`, so the GUI, MCP and the CLI agree.
- Tests never `set_var`: every resolver has a pure `_from(env, …)` form.

## Sentries

- `cargo test -p rs_cam_core -q --lib settings::`
- `cargo test -p rs_cam_core -q --lib budget::settings`
- `cargo test -p rs_cam_cli -q the_cli_resolves_the_machine_folder`
