# Performance Review

This file is the active performance backlog, not a frozen benchmark report.

## Areas worth re-benchmarking

### Spatial index query cost

- revisit triangle-query deduplication in `mesh.rs`
- confirm the current `kiddo`-based approach is still the right tradeoff for large meshes

### Surface-heightmap generation

- benchmark precomputed surface heightmaps used by adaptive 3D, scallop, ramp finish, and steep/shallow
- measure release-build cost across realistic fixture meshes

### Simulation stamping

- profile heightmap stamping and arc linearization in `simulation.rs`
- separate tool stamping cost from mesh-upload/render cost

### Adaptive clearing inner loops

- benchmark material-grid operations, entry-point search, and direction scoring
- confirm current tolerances are not doing excess work

### GPU upload churn in the GUI

- track how often toolpaths, sim meshes, and overlays are rebuilt unnecessarily
- identify cheap caching wins before adding more rendering features

## Benchmark gaps

- end-to-end desktop workflow timing for import -> generate -> simulate -> export
- per-operation criterion coverage for newer 3D finishing ops
- stock-simulation benchmarks on larger work envelopes
- regression benchmarks for feeds/speeds calculation are not necessary yet, but the GUI integration path should remain allocation-light

## Guidance

- prefer measuring release builds with representative fixture meshes
- document machine specs and mesh sizes alongside results
- convert repeatable wins into tests or criterion benches where practical
