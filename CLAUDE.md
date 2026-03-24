# rs_cam Agent Notes

## What this repo is

`rs_cam` is a Rust CAM workspace for 3-axis wood routers.

It has three product layers:

- `crates/rs_cam_core`: CAM engine and shared data model
- `crates/rs_cam_cli`: batch CLI
- `crates/rs_cam_viz`: desktop CAM app (`rs_cam_gui`)

## Architecture guardrails

- keep the core library independent from GUI concerns
- treat the toolpath IR as the boundary between planning and post-processing/output
- keep import, tool modeling, operation generation, dressups, simulation, and export as distinct layers
- prefer extending the existing core + worker + UI wiring path instead of creating parallel one-off flows

## Current doc map

- product overview: `README.md`
- capability surface: `FEATURE_CATALOG.md`
- attribution and source lineage: `CREDITS.md`
- design docs: `architecture/`
- research notes: `research/`
- status and backlog: `planning/`

## Session workflow

1. Read `planning/PROGRESS.md`.
2. Check `FEATURE_CATALOG.md` before making claims about shipped functionality.
3. Update docs when the visible product surface changes.
4. Keep `CREDITS.md` current when adding external datasets, formulas, or algorithm references.

## Dependency reality

Use the actual manifests as source of truth:

- workspace: `Cargo.toml`
- core: `crates/rs_cam_core/Cargo.toml`
- CLI: `crates/rs_cam_cli/Cargo.toml`
- GUI: `crates/rs_cam_viz/Cargo.toml`

Do not document or rely on crates that are not currently in those manifests.

## Implementation expectations

- tests live close to the code they validate
- if GUI state adds a field, audit setup-sheet, project-IO, and any test initializers for required updates
- if a feature is only present in UI/state and not end-to-end wired, document that honestly

## Lint policy — zero warnings enforced

All 16 clippy lints below are **deny** at workspace level (`Cargo.toml`). Clippy must pass with zero warnings before committing.

| Lint | What it catches |
|------|-----------------|
| `unwrap_used` | `.unwrap()` — use `?`, `.unwrap_or()`, or `#[allow]` + SAFETY comment |
| `expect_used` | `.expect()` — same; `#[allow]` OK for provably-safe cases with comment |
| `panic` | `panic!()` in non-test code |
| `todo` / `unimplemented` | Placeholder code must not ship |
| `indexing_slicing` | `arr[i]` — use iterators, `.get()`, or `#[allow]` + SAFETY comment |
| `dbg_macro` | No `dbg!()` in production |
| `print_stdout` / `print_stderr` | Use `tracing` instead of `println!`/`eprintln!` |
| `map_err_ignore` | `.map_err(\|_\| ...)` — preserve the original error |
| `needless_pass_by_value` | Take `&[T]`/`&str` not `Vec<T>`/`String` when not consumed |
| `large_enum_variant` / `result_large_err` | Keep enums and error types small |
| `redundant_clone` | Don't `.clone()` what you already own |
| `unsafe_code` | No `unsafe` in this codebase |

**When you hit a lint:** run `/lint-fix` for approved fix patterns. Prefer fixing the code. If the pattern is provably safe (e.g. indexing bounded by a loop, `.expect()` after a `.is_some()` check), use `#[allow(clippy::the_lint)]` with a `// SAFETY:` comment on the specific line or block — never file-level.

**Test code** is exempt: test modules carry `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`.

## Dev workflow quick reference

| Task | Command |
|------|---------|
| Run GUI | `cargo run -p rs_cam_viz --bin rs_cam_gui` |
| Run CLI | `cargo run -p rs_cam_cli -- <subcommand>` |
| Test all | `cargo test -q` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --check` |
| Bench | `cargo bench -p rs_cam_core` |

Run `/dev` for the full reference. Run `/verify` before committing.

## Agent skills

Project-level Claude Code customizations in `.claude/`:

| File | Type | Purpose |
|------|------|---------|
| `skills/verify/SKILL.md` | `/verify` | Run the CI quality gate locally |
| `skills/dev/SKILL.md` | `/dev` | Build, test, run, and module quick reference |
| `skills/sim-analysis/SKILL.md` | `/sim-analysis` | Simulation diagnostic interpretation guide |
| `skills/lint-fix/SKILL.md` | `/lint-fix` | Fix clippy lint violations with approved patterns |
| `agents/cam-navigator.md` | Agent | Codebase navigation: find operations, trace pipelines |
| `agents/sim-diagnostics.md` | Agent | Simulation diagnostic analysis and interpretation |

## Parallel agent teams

Use `TeamCreate` to spin up agent teams for tasks that benefit from parallel work:

- **Parameter sweeps**: 4 agents split by operation family (2D contour, 2D clearing, 3D raster, 3D contour) — see `toolpath_stress_test/agents/AGENT_INSTRUCTIONS.md`
- **Defect investigation**: one agent per finding from `toolpath_stress_test/FINDINGS.md`, each in an isolated worktree
- **Multi-crate refactors**: separate agents for core, CLI, and viz changes working on independent worktrees
- **Test + fix cycles**: one agent runs tests / sweeps, another fixes issues as they're reported

Teams share a task list for coordination. Use worktree isolation (`isolation: "worktree"`) when agents edit overlapping files. Agents go idle between turns — this is normal; send them messages to wake them.

## Parameter sweep infrastructure

Toolpath validation tooling lives in `crates/rs_cam_core/src/fingerprint.rs` and `crates/rs_cam_core/tests/param_sweep.rs`:

| Command | What it does |
|---------|-------------|
| `cargo test --test param_sweep` | Run all 54 parameter sweeps across 22 operations |
| `cargo test --test param_sweep sweep_pocket` | Run sweeps for one operation family |
| `cargo run -p rs_cam_cli -- sweep job.toml --param X --values "..." --output-dir out/` | Full-pipeline sweep with dressups/depth stepping |
| `python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/` | Automated verdict analysis |

Sweep output goes to `target/param_sweeps/{op}/{param}/` with JSON fingerprints, diffs, toolpath SVGs, and 6-view composite stock PNGs.
