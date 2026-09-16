# rs_cam agent index

`rs_cam` is a Rust CAM workspace for 3-axis wood routers.

## Read the instruction file closest to the work

| Area | Instruction file | Owns |
|---|---|---|
| Workspace-wide rules and this index | `CLAUDE.md` | architecture, discovery, quality gates |
| CAM engine / session / simulation / export | `crates/rs_cam_core/CLAUDE.md` | machining invariants and core tests |
| Desktop GUI / controller / embedded MCP server | `crates/rs_cam_viz/CLAUDE.md` | UI state, worker wiring, live control |
| Batch commands and job files | `crates/rs_cam_cli/CLAUDE.md` | CLI parity, replay and sweeps |
| MCP wire types | `crates/rs_cam_mcp/CLAUDE.md` | schema compatibility |
| Plans, status and historical evidence | `planning/CLAUDE.md` | active-vs-archived planning material |

Read the relevant child file before changing that area. Do not load every
package instruction speculatively.

## Workspace shape

- `crates/rs_cam_core`: CAM engine and shared data model.
- `crates/rs_cam_cli`: batch CLI.
- `crates/rs_cam_viz`: desktop application (`rs_cam_gui`) and embedded MCP
  server.
- `crates/rs_cam_mcp`: shared MCP parameter types; it is **not** the server.

## Architecture and mutation contract

- Keep core independent of GUI concerns.
- Treat the toolpath IR as the boundary between planning and post-processing /
  output.
- Keep import, tool modelling, operation generation, dressups, simulation and
  export as distinct layers.
- Extend the existing core → worker → UI wiring path; do not add a parallel
  one-off flow.
- Every product surface mutates `ProjectSession` through
  `ProjectSession::apply(Command)`. Do not reintroduce public `*_mut` escape
  hatches.

## Sources of truth

- Manifests, not prose, define dependencies: root `Cargo.toml` and each crate's
  `Cargo.toml`.
- `FEATURE_CATALOG.md` defines shipped capability claims.
- `planning/PROGRESS.md` is the current status entry point; consult the
  relevant plan before acting on a package.
- Update visible-product docs with visible surface changes.
- Update `CREDITS.md` when adding external datasets, formulas or algorithm
  references.
- A `planning/…` path in a doc comment or a document that no longer exists was
  deleted by the structure purge of 2026-09-17. Retrieve it with
  `git show planning-pre-purge-2026-09-17:<path>`; `planning/DELETED_INDEX.md`
  says what each deleted package decided.

## Codebase discovery

This project is indexed by SocratiCode. Start exploration with broad
`codebase_search`; use `rg` only for a known exact string/regex. Before a
refactor, deletion or import-graph change, inspect impact with the graph /
symbol tools. Read files only after search narrows the area. If search is
empty, inspect index status rather than assuming the symbol is absent.

## Quality gates

`Cargo.toml` is the lint policy source of truth: zero warnings, including the
workspace's denied clippy lints and `unsafe_code`.

| Need | Command |
|---|---|
| Format | `cargo fmt --all -- --check` |
| Focused crate tests | `cargo test -p <crate> -q` |
| Core full gate | `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q` |
| Full lint | `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` |

Do not use workspace-wide `cargo test`; it can loop in this repository. Run the
smallest relevant test first, then the appropriate gate before committing.

Test modules may allow only `unwrap_used`, `expect_used`, `panic` and
`indexing_slicing`; `println!` / `eprintln!` remain denied unless a test has a
specific `print_stderr` allowance. Prefer a local, documented allow with a
`SAFETY:` comment over a file-wide production-code allow.

## Regression sentries

The F-024..F-040 acceptance findings remain protected by tests under
`crates/rs_cam_core/tests/`, including `_litmatrix_*.rs`. When a change touches
one of those behaviours, find and run its sentry; see
`crates/rs_cam_core/CLAUDE.md` for the core-specific rules.
