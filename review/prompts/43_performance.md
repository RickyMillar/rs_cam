# Review: Performance & Parallelism

## Scope
Hot paths, rayon usage, memory allocation patterns, benchmark coverage.

## Files to examine
- `crates/rs_cam_core/benches/perf_suite.rs` (268 LOC)
- Rayon usage: grep for `par_iter`, `rayon`, `parallel`
- Large algorithm files: adaptive.rs, feeds/mod.rs, simulation.rs, dropcutter.rs
- Known feedback: "bitset dedup wins big, sqrt-per-row regresses small tools"
- `planning/Performance_review.md`

## What to review

### Hot paths
- Dropcutter: per-point mesh query — is spatial index used effectively?
- Simulation: per-move stamp operation — vectorizable?
- Adaptive: engagement tracking — data structure efficiency?
- Polygon offset: cavalier_contours performance for complex shapes?

### Rayon parallelism
- Where is rayon used? (feature-gated as optional)
- Where could it be used but isn't?
- Granularity: is work partitioned well (not too fine, not too coarse)?
- Cancellation interaction with rayon

### Memory
- Large allocations: Vec growth patterns, pre-allocation
- Arc usage for shared data: appropriate?
- K-d tree memory for large meshes
- Dexel grid memory at fine resolution

### Benchmarks
- What's in perf_suite.rs? Which operations benchmarked?
- Are benchmarks representative of real workloads?
- Missing benchmarks for known hot paths?

### Known issues
- sqrt-per-row regression for small tools — is this fixed?
- bitset dedup — where is this and does it still apply?

## Output
Write findings to `review/results/43_performance.md`.
