# S2 results — the detector sees the strike class (2026-08-28)

## What changed

Rapid collision checking for stamped (non-drill) toolpaths moved from a
frozen pre-op point probe into the **live replay walk**: each rapid is
evaluated against the stock as it exists at that moment of playback, with
the profile-aware disc query (`max_clearance_tip_z_for_profile`) and a
cell-scaled tolerance. Drill entries keep the original pre-pass
(`check_rapid_collisions_against_stock`, byte-unchanged — its module tests
and `tests/rapid_check_wanaka_link_shape.rs` still pin it).

Key pieces (`collision.rs`, `dexel_stock/simulation.rs`,
`compute/simulate.rs`):

- `RapidClearanceCheck` — ascending-Z-only rapids exempt (monotone-profile
  proof), ≤1 mm sampling incl. endpoints, per-sample profile clearance,
  collision iff `tip_z < clearance − rapid_clearance_tolerance_mm(cell)`.
- `rapid_clearance_tolerance_mm(cell) = cell × (1/√2 + 0.5 + 1.0)` ≈
  2.2 cells — the stacked over-reads of the conservative query (half-diag
  profile shift + half-cell disc dilation + one `conservative_top` cell).
  At 0.3 mm sim: 0.66 mm. **The count is therefore resolution-scaled**:
  coarser sims report only deeper interference. No radius shaving.
- Both replay walks (metrics and non-metrics) instrumented via
  `_rapid_checked` siblings; old entry points delegate unchanged; the
  global-playback walk opts out. Two-stage evaluation (stale grid first,
  flush queued stamps and re-ask only on a candidate hit) keeps the flush
  off the hot path — sound because stamping only removes material.
- F3's same-XY walk-back is NOT ported to the live path: the live stock
  already knows a just-cut column is empty. It remains in the drill
  pre-pass, where the snapshot is frozen.

## Falsification — the phase's own test, passed

Same command, same project, before → after
(`cargo run -p rs_cam_cli --release -- project
planning/airrun_2026-08-19/wanaka200.toml --resolution 0.3`):

| op | before | after |
|---|---|---|
| 1 Pin Drill / 3 Holes (drill pre-pass) | 0 | 0 ✓ |
| 2 Back Rough / 6 3D Rough | 0 | 0 |
| 4 Rivers (V-bit) | 0 | 0 |
| 5 Lakes (R1.0 taper) | 0 | **1** |
| 7 3D Finish (R1.5 drop_cutter) | 0 | **179** |
| 8 Pencil detail (R0.5 taper) | 0 | **22** |

Project verdict: `WARNING: 202 rapid-through-stock collisions`. The
distribution matches S1's depth histogram read through the 0.66 mm
tolerance (S1 fine-tier: 142 strikes deeper than 0.5 mm, 634 beyond
discretisation); the shallow tail is deliberately below this resolution's
reporting floor. Op 5's single hit was not seen by S1 (different chain
state: S1 replayed the shipped programs; this run generates today's) — it
is the same class and falls to S3's emitter work.

Sentries: `tests/rapid_live_check_crest_s2.rs` — the off-axis crest flags
(and the old point probe provably does not, documenting S-b); a crest the
profile clears does not flag; own-kerf re-entry on live stock does not flag
with no F3 heuristic; flat-endmill anchor vs the point-probe verdicts.

Suites: full dev loop green with the fix (no existing fixture regressed);
full heavy gate run before commit (result in the commit message).

## Known limits, deliberately kept

- **Kerf-rim over-read on vertical-flank tools** (editor finding, kept
  as-specified): near a tool's envelope rim the conservative query's
  over-read scales as `√(2·R·half_diag)` — square-root in cell size against
  a linear tolerance. A ball/flat tool re-entering a kerf of exactly its
  own width can exceed the tolerance from conservatism alone. The measured
  wanaka class rides conical flanks (linear over-read) and is unaffected.
  If a green fixture turns red on a flat/ball tool, check this mechanism
  FIRST (arithmetic in the sentry file header). Candidate follow-up: a
  sub-cell-aware clearance query. On S3's docket.
- **No mip early-out** on long safe-Z traverses — one disc query per ≤1 mm
  sample per non-exempt rapid. Kept mechanical; measure before optimizing.
- The S5 prefix cache still restores collision results computed when the
  prefix was first simulated (`sim_prefix.rs`) — a resumed prefix is not
  re-checked. Pre-existing behaviour, unchanged by S2; ledgered for S4/S5.

## What this does NOT fix

The emitters still plan these descents (S3): the finish link planner
descends to the resume Z with zero clearance, and `optimize_entry_descents`
trusts a point-sampled ceiling. S2 makes the defect VISIBLE on every
surface that reads `rapid_collision_count`; S3 makes it stop happening.
