# `tests/` — the core integration tests and sentries

About 330 flat files. Each file is one claim about the engine, with a name
of the form `<claim>_<programme code>.rs`. The code (`f036c`, `m4`, `wp28`,
`g_drillflip`) names the plan that pre-registered the claim; the claim
part says what the file protects. Search by claim words, not by code.

## Layout

- `common/` — shared fixtures: meshes, chains, the reference plate, the
  band map, the fingerprint helper, the offset lab.
- `fixtures/` — the pinned projects, `terrain.stl`, the STEP samples, the
  golden files. A project or baseline a test loads lives HERE, not in
  `planning/`, dated and hashed; re-bless a golden on a measured cause.
- `literature_matrix/` — the cited feeds cells and their invariants; the
  `/refresh-lit-matrix` skill maintains `sources.toml`.
- `ARCHIVED_HARNESSES.md` — two mega harnesses that left this directory.

## How to run

- One sentry: `cargo test -p rs_cam_core -q --test <name>`. Each folder
  `CLAUDE.md` under `src/` names the sentries for that folder.
- `heavy-tests` (twelve heavy binaries plus one table set), `research` (8)
  and `test-support` (8, which bind a door) gate targets in `Cargo.toml`; a
  named run without its feature errors. CI runs them. Locally, name one
  binary and ask the operator first.
- `#[ignore]` marks an instrument or an evidence run, not a broken test.
  Run one by name with `-- --ignored`. Never use `--include-ignored`.
- A `planning/…` path in a doc comment may no longer exist; the tag
  `planning-pre-purge-2026-09-17` holds it.

## Writing a sentry

- A source-scanning test needs a non-vacuity anchor: assert the needle is
  present before you assert its property, and do not match comments.
- Inject the guarded defect once and confirm the test goes red before you
  trust it.
- The lint allowances for a test module are in `../CLAUDE.md`.
- Tests that need the wanaka project run for minutes. Say so in the
  folder file that names them.
