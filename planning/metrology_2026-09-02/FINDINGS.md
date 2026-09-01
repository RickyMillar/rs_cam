# Track M — findings

Charter: `TRACK.md`. Rule: pure promotion, byte-identical shipped
behavior; a divergence between instrument copies becomes an explicit
parameter and is disclosed here.

## M-1. The costing harness — six copies, one divergence (2026-09-02)

Promoted `relink_and_cost` / `relink_and_cost_under`, `CandidateCost`
and `LinkRegime` to `rs_cam_core::metrology::costing`. The source is
the `thin_organic_island_widths.rs` copy — the most complete variant,
with the `LinkRegime` link-ceiling machinery.

Census of the six copies before promotion:

| file | cutter type | extra `CandidateCost` fields | `link_kinematics` |
|---|---|---|---|
| `thin_organic_island_widths.rs` | `TaperedBallEndmill` | `slower_than_retract`, `ceiling_above_safe_z` | `Some` |
| `whole_board_spiral_ledger_g1.rs` | `TaperedBallEndmill` | `rapid_mm`, `path` | `Some` |
| `spiral_finish_compact_c1.rs` | `BallEndmill` | `path` | `Some` |
| `direction_field_wanaka_f1.rs` | `BallEndmill` | — | `Some` |
| `conformal_spiral_synthetic_f2.rs` | `BallEndmill` | `path` | `Some` |
| `monotone_cell_decomposition_c2.rs` | `BallEndmill` | returns `(kept_retracts, rapid_mm)` only | **`None`** |

Findings:

* **Five copies are byte-equivalent** up to the cutter's concrete type
  (the kernel takes `&dyn MillingCutter`, so this is monomorphization,
  not behavior) and which output fields they keep. The promoted
  `CandidateCost` carries the union; every field is computed on every
  call, which is a pure addition.
* **One genuine divergence**: `monotone_cell_decomposition_c2.rs`
  passed `link_kinematics: None` — the relink keeps ANY gouge-safe
  link instead of costing each link against the retract it replaces,
  and no cycle time is integrated. Preserved as the explicit
  `CostingContext::kinematics: Option<&MachineKinematics>` parameter:
  `None` reproduces c2 (`time_s` reads `NaN` — not measured), `Some`
  reproduces the other five.
* c2's feed pins also differ (1000/500 vs the wanaka-tier 735/180).
  Feeds were always per-instrument constants; they ride
  `CostingFeeds`, set by each consumer, unchanged.
* The shared relink parameters are identical across all six and are
  now stated once in the library: `hookup_distance` 25.0,
  `stock_to_leave` 0.0, `sampling` 0.5, `reorder: true`, boundary =
  the region's own polygon.
* `LinkRegime::fresh_stock` (no ceiling, `flush_ride`/`airborne`
  false) is what the five non-thin-organic copies hardcoded — not a
  divergence; both flags are inert without a ceiling
  (`surface_link.rs`).

All six instruments now consume the library through thin adapters
that keep their original call shape. Non-ignored pins in all six
files: green after conversion (`c2` 7/7; the other five suites pass).
