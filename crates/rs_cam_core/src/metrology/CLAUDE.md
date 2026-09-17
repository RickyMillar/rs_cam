# `metrology/` — the finishing programme's rulers

The shared instruments that measure a finished surface. The entry point is
`metrology::mod`, which re-exports each ruler.

## Files

- `mod.rs` — the facade.
- `spacing.rs` — the achieved surface spacing the emitted passes left.
- `floor.rs` — the finish-length floor and the `× floor` ratio.
- `costing.rs` — the candidate costing harness: production relink, then the
  kinematics cycle time.
- `census.rs` — the two strategy-gate censuses: direction coherence and
  curvature.
- `monge.rs` — Monge-quadric curvature on an up-facing heightfield.
- `union_coverage.rs` — the union-coverage audit.

## Invariants

- Both sides of a ratio must be the same measure. A changed instrument makes
  its own docstring a lie.
- A candidate cost must use the production relink kernel, never a private
  copy.
- A geometric primitive belongs to `geo.rs`. A ruler calls it; it does not
  keep its own copy. Point-segment distance is
  `geo::point_to_segment_distance_3d`.

## Sentries

- `cargo test -p rs_cam_core -q --test bikeseat_gate_d1`
- `cargo test -p rs_cam_core -q --test zone_coherence_census`
- `cargo test -p rs_cam_core -q --test union_coverage_m1`
- `cargo test -p rs_cam_core -q --test wanaka_curvature_anisotropy`
- `cargo test -p rs_cam_core -q --test spacing_prize_split_f1`

## Do not

- Do not change an instrument and keep a claim that cited the old one.
