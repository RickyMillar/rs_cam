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

## M-2. The floor integrand — three copies converged (2026-09-02)

Promoted `region_floor`, `FloorReport`, `AreaWeighted`/`area_weighted`
and the `× floor` score (`FloorReport::times_floor`) to
`rs_cam_core::metrology::floor`, from the
`conformal_spiral_synthetic_f2.rs` copy (the full two-basis form).

* Disclosed divergence, closed by the promotion:
  `spiral_finish_compact_c1.rs`'s restated copy computed only the
  `κ_min` basis and tested only that basis for degeneracy. The library
  computes both bases and counts a triangle degenerate when EITHER
  collapses (the F2 rule). On c1's umbilic fixtures `κ_min = κ_max`,
  so c1's numbers do not move; the adapter in c1 says so.
* `whole_board_spiral_ledger_g1.rs`'s flat-law reference floor
  (`area / s_flat`) now reads its area term from
  `metrology::floor::mesh_area_mm2`. The flat law itself stays a
  stated reference, not an exact floor.
* The curvature source stays a CALLBACK parameter: the analytic
  fixtures pass their closed forms, so no estimator sits inside the
  floor. New unit pins: area-weighted median follows area (moved from
  f2) and a flat-square closed-form floor check (new).

## M-3. Achieved surface spacing — the Track B ruler (2026-09-02)

Promoted to `rs_cam_core::metrology::spacing`:

* `measure_raster_spacing` + `SpacingSample` + `SpacingMeasurement` +
  `dist_point_segment` from `tests/shipped_raster_spacing_b1.rs`,
  verbatim; the fixture's analytic closed forms ride in as
  `ContactMaps`, so the ruler still carries no estimator.
* `path_structure` + `PathStructure` + `quantile` from
  `tests/common/scallop_oracle.rs`, verbatim (indexing rewritten to
  `.get()` for the production lint gate — same arithmetic);
  `scallop_oracle` re-exports them so every M4 consumer keeps its
  import path.

**Sentinel proof (charter requirement).** The b1 acceptance instrument
(`shipped_shallow_raster_spacing_on_analytic_fixtures`, release,
`--ignored --nocapture`) was run before and after the conversion and
its full transcript diffed. The ONLY differing line is the harness
wall-clock ("finished in 0.09s" vs "0.07s"); every measured number —
all three fixture verdicts (CLEAN), every band row, every Δy census,
`max achieved / s_max` (sphere 0.9391, plane20 1.0000, plane40
1.0000), and the whole × FLOOR table (1.277 / 1.075 / 1.009) — is
byte-identical. Transcripts: scratchpad `b1_before.txt` /
`b1_after.txt` (session artifacts; the numbers above are the record).
`shallow_raster_slope_derate` sentries: 2/2 green after conversion.
