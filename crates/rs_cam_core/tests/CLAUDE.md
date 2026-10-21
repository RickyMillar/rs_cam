# `tests/` — the core integration tests and sentries

About 330 flat files. Each file is one claim about the engine, with a name
of the form `<claim>_<programme code>.rs`. The code (`f036c`, `m4`, `wp28`,
`g_drillflip`) names the plan that pre-registered the claim; the claim
part says what the file protects. Search by claim words, not by code.

## Layout

- `common/` — shared fixtures (meshes, chains, reference plate, band map, fingerprint, offset lab). D3: one op's toolpath
  comes from `op_fixtures::generated_op` (the session door); one 2.5D level from `kernels.rs`.
- `fixtures/` — the pinned projects, `terrain.stl`, the STEP samples, the
  golden files. A project or baseline a test loads lives HERE, not in
  `planning/` (not `planning/fixtures/` either), as
  `fixtures/<name>_<date>_<hash>/` with relative model paths; re-bless a
  golden on a measured cause. A test that runs reads no `planning/` or
  `/home/` path and writes output under `CARGO_TARGET_TMPDIR`
  (`gate_tests_read_no_operator_paths_f155`, a viz twin). A test that
  needs a large operator file is `#[ignore = "evidence: .."]`.
- `literature_matrix/` — the cited feeds cells and their invariants; the
  `/refresh-lit-matrix` skill maintains `sources.toml`.
- `ARCHIVED_HARNESSES.md` — two mega harnesses that left this directory.

## How to run

- One sentry: `cargo test -p rs_cam_core -q --test <name>`. Each folder
  `CLAUDE.md` under `src/` names the sentries for that folder.
- `heavy-tests` (twelve heavy binaries plus one table set), `research` (8)
  and `test-support` (66 that bind a door, ten of them also heavy or
  research) gate targets in `Cargo.toml`. A test build turns `test-support`
  on by itself (the self dev-dependency in `Cargo.toml`); a named heavy or
  research run without its feature errors. No CI exists. Locally, name one
  binary and ask the operator first.
- `#[ignore]` marks an instrument or an evidence run, not a broken test.
  The reason starts with `instrument:`, `evidence:`, `heavy:`,
  `external:` or `serial:` (`every_ignore_states_its_kind_f152`). A known
  red is an inverted assertion that runs and fails when the fix lands; a
  retired claim is deleted. Run one by name with `-- --ignored`. Never use
  `--include-ignored`.
- A gate test does not skip on a missing fixture: `common::require_fixture`
  panics and names the path.
- A `planning/…` path in a doc comment may no longer exist; the tag
  `planning-pre-purge-2026-09-17` holds it.

## Writing a sentry

- A source-scanning test needs a non-vacuity anchor: assert the needle is
  present before you assert its property, and do not match comments.
  `common/source_scan.rs` (included by `#[path]`) strips comments and
  finds the items a running test reaches. `common/folder_maps.rs` holds the
  F140 folder-map rules (`folder_maps_name_real_files_f140`, a viz twin).
- A scan of viz source lives in `rs_cam_viz/tests`, so the viz gate runs
  it. Do not re-check a `pub(crate)` rule by a scan: the compiler holds it.
- Inject the guarded defect once and confirm the test goes red before you
  trust it.
- The lint allowances for a test module are in `../CLAUDE.md`.
- Tests that need the wanaka project run for minutes. Say so in the
  folder file that names them.
