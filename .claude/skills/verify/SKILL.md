---
name: verify
description: Run the CI quality gate; the core suite needs the operator's go-ahead
allowed-tools: Bash
---

# /verify — Pre-Merge Quality Gate

Run these steps in order. They cover the same ground as `.github/workflows/ci.yml`, with the same feature flags — but per-crate rather than workspace-wide (see step 2), so the commands are not byte-identical to the workflow's.

## Steps

### 1. Format check
```bash
cargo fmt --check
```
Fix: `cargo fmt` then review changes.

### 2. Test suite (per-crate — avoid workspace-wide)

This is the CI gate. Locally, run it only with the operator's go-ahead: the core line takes 30+ minutes. The usual local loop is the folder sentries, the focused crate tests and step 3.

```bash
cargo test -p rs_cam_core --features heavy-tests,research --no-fail-fast -- -q
cargo test -p rs_cam_cli -q
cargo test -p rs_cam_viz -q
cargo test -p rs_cam_mcp -q
```
Run per-crate — workspace-wide `cargo test` from the repo root can loop / swap-thrash on this repo (see operational guardrails).
`--features heavy-tests` on core is load-bearing: the 12 heaviest binaries are compiled only under that feature, and the gate must not lose those sentries. Do NOT reach for `--include-ignored` — `#[ignore]` here marks instruments and evidence runs (one family needs an externally installed validator), which are not gate material.
Fix: run failing test alone with `cargo test -p <crate> <name> -- --nocapture`.

### 3. Clippy (zero warnings)
```bash
cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research -- -D warnings
```
Both features belong here: without `heavy-tests` the 12 heavy binaries are never linted, and without `research` the three `finish/` research arms (`conformal_spiral`, `direction_field`, `spiral_finish_compact`) and their eight harnesses are never linted. Either way the code drifts silently.
Fix: address each warning — the project enforces `-D warnings`.

### 4. Demo job smoke test
```bash
mkdir -p demos && cargo run -p rs_cam_cli -- job fixtures/demo_job.toml
```
Fix: check `fixtures/demo_job.toml` and `crates/rs_cam_cli/src/job.rs`.

### 5. Viz regression harness
```bash
cargo test -p rs_cam_viz controller::tests::
cargo test -p rs_cam_viz compute::worker::tests::
```
Fix: check `crates/rs_cam_viz/src/controller/tests.rs` and `compute/worker.rs`.

## All pass?
Ready to commit. All five steps must pass — this is what CI enforces on every PR.
