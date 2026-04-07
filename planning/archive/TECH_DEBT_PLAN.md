# Tech Debt Plan

Ordered remediation plan for the current code-review findings before a broader open-source push.

## Phase 1: Public-push blockers

Status: complete

### 1. Fix project persistence

- serialize and restore full toolpath state, not just stock/tools shell data
- persist model references and re-import them during open
- cover dressups, heights, boundary settings, stock source, feeds auto/manual flags, and per-operation params
- add round-trip tests for at least one 2.5D job and one 3D job

Exit criteria:
- save/open returns a semantically equivalent job
- project persistence no longer appears in `FEATURE_CATALOG.md` as a partial area

### 2. Remove dead controls or wire them end-to-end

- either support separate radial/axial stock semantics for finishing ops or collapse the GUI to the single stock-to-leave value the core actually uses today
- resolve other exposed-but-unwired controls the catalog already calls out: drop-cutter skip-air-cuts, drop-cutter slope confinement, waterline continuity, and stock-source behavior
- prefer removing or hiding misleading controls over shipping no-op controls

Exit criteria:
- no visible control silently does nothing
- `FEATURE_CATALOG.md` matches the real shipped UI surface

### 3. Make feed optimization trustworthy

- replace the synthetic `-100..100 @ z=0` heightmap with job-derived context
- use actual stock/model geometry where supported
- if an operation is unsupported, disable or clearly label the dressup instead of returning approximate-but-authoritative output
- add tests for offset jobs and non-flat 3D surfaces

Exit criteria:
- feed optimization is geometry-aware on supported ops
- unsupported cases are explicit, not silently approximate

### 4. Establish a clean release gate

- fix the current workspace clippy failures
- add CI for `cargo fmt --check`, `cargo test -q`, and `cargo clippy --workspace --all-targets -- -D warnings`
- keep the gate green before adding more product surface

Exit criteria:
- the workspace passes the full lint/test gate locally and in CI

## Phase 2: Responsiveness and regression safety

Status: complete

### 5. Improve cancellation and worker behavior

- thread cancellation into long-running toolpath kernels where feasible
- support cancellation for simulation and collision jobs
- reduce queue starvation from a single long-running worker job blocking everything behind it
- define and test a bounded cancel latency target on representative heavy jobs

Exit criteria:
- cancel stops active work promptly enough to feel real
- simulation/collision no longer ignore cancel requests

### 6. Add viz-level regression coverage

- add tests around project persistence, dead-control wiring, and operation dispatch assumptions
- add fixture coverage for finish ops with stock-to-leave semantics
- add smoke coverage for save/open and export paths that are currently only exercised manually

Exit criteria:
- the main product-surface regressions are covered without manual GUI testing

Delivered in Phase 2:

- dual-lane compute backend in `rs_cam_viz`: `Toolpath` and `Analysis`
- lane snapshots with `idle`, `queued`, `running`, and `cancelling` states
- cancel propagation through adaptive, adaptive3d, drop-cutter, waterline, simulation, and collision loops
- deterministic renderless viz harness via `AppController` and UI automation IDs
- Linux CI lane dedicated to `rs_cam_viz` harness and compute-lane regression tests

## Phase 3: Structural simplification

Status: complete

### 7. Centralize operation capabilities

- replace repeated `match OperationConfig` switchboards with a more canonical capability layer
- consolidate feed/plunge/stepover/depth accessors so new operations do not require edits in multiple subsystems
- introduce helper constructors/builders for `ToolpathEntry`

Exit criteria:
- adding one operation or one toolpath field touches one canonical place instead of many scattered files

### 8. Split the largest modules

- extract shared adaptive direction-search logic from `adaptive.rs` and `adaptive3d.rs`
- split `app.rs`, `state/toolpath.rs`, and `compute/worker.rs` by responsibility
- keep future modules small enough to review without scrolling through thousand-line files

Exit criteria:
- the current god-modules are reduced to smaller reviewable units
- duplicated algorithm-control code is shared instead of mirrored

Delivered in Phase 3:

- canonical operation spec/accessor layer in `rs_cam_viz::state::toolpath`
- `ToolpathEntryInit` plus centralized runtime-state reconstruction
- controller-first `RsCamApp` shell over `AppController`
- `state/toolpath`, `controller`, and compute worker split into smaller responsibility modules
- shared adaptive support module in `rs_cam_core::adaptive_shared`
- shared angle math, corner blending, target-engagement math, and bracket-refinement logic used by both adaptive engines

## Recommended execution order

1. Operation-model refactor
2. Module decomposition
