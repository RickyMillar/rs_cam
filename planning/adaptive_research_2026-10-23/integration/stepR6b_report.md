# Removal step R6b: boundary parity B2 is strategy-neutral (D67)

Date: 2026-10-28. Branch `geodesic-integration`, base `a757e51dd` (R6).
The implementer did not commit.

Patch: `stepR6b_tree_vs_a757e51dd.patch`, md5 in the lead's message.
1 file, `crates/rs_cam_core/tests/adaptive3d_boundary_clear_parity.rs`
(+67/-39). No product code. No planning file. `git apply --check -R` passes.

## 1. Why

D67 (P15) closed G-GEOWATERLEAK. The old B2 counted the PRE-CLIP endpoints
of the boundary-unaware `waterline_cleanup`, on a plan with an empty
`centre_boundary`. Geodesic Adaptive's finer `emission_chord_tolerance`
cuts the same contour arcs into more, shorter moves. So the count was per
strategy (CP 135, Geodesic 312), and it did not measure a cut outside.

## 2. Change

- `measure` plans the fixture a second time with `centre_boundary` set to
  the effective containment, as `compute/execute/rough_3d.rs` sets it. B2
  counts the cutting endpoints of that plan fully outside the containment.
- B2 asserts 0 on both strategies.
- Non-vacuity: the pre-clip leak (the plan without `centre_boundary`) must
  be greater than 0. So the bar proves that the centre clip removes a leak
  that exists.
- The pins `WATERLINE_LEAK_CUT_MOVES` (135) and
  `WATERLINE_LEAK_CUT_MOVES_GEODESIC` (312) are deleted. The B2 comment and
  the bar table give the reason and cite D67.
- The main plan, and with it B1, B3, B4, P1 and P2, is unchanged. The
  second plan feeds B2 only.
- The D4 arm keeps its inverted pre-clip pin. Its F-B doc now names the
  pre-clip leak (CP 135, Geodesic 312).
- The `Measured::strategy` field had no reader after the change, so it is
  removed.

## 3. Evidence

| Check | Result |
|---|---|
| B2, with `centre_boundary` set | 0 (CP), 0 (Geodesic) |
| pre-clip leak, without it | 135 (CP), 312 (Geodesic), as D67 |
| planted red: `path.rs` centre clip turned off (`centre_clip.filter(\|_\| false)`) | both arms fail: "B2: 135" (CP), "B2: 312" (Geodesic); `path.rs` restored, equal to HEAD |
| `adaptive3d_boundary_clear_parity` | 4 ok, about 13 s |
| `cargo fmt --all -- --check`, clippy `rs_cam_core` all targets, full features | ok |
