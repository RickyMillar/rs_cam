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

## M-4. The Monge estimator and the two strategy censuses (2026-09-02)

* `tests/common/monge.rs` PROMOTED to `rs_cam_core::metrology::monge`,
  verbatim (indexing rewritten to `.get()` or covered by SAFETY-commented
  allows for the production lint gate — same arithmetic). The common
  module is now a named re-export shim, so `bikeseat_gate_d1`'s import
  path is unchanged; its closed-form validation pin
  (`monge_extraction_recovers_known_curvature_and_axis`) passes against
  the promoted code.
* `rs_cam_core::metrology::census` carries the two censuses' shared
  kernels: `TriField`, `TurnGrid` (nearest-turn search), `census_zone` +
  `ZoneStats` + `ZoneVerdict` (the coherence census), and `prize_cell` +
  `PrizeCell` (the anisotropy prize). Gate thresholds are named,
  documented constants citing their evidence: `W30_COHERENT` (0.70),
  `COHERENCE_LENGTH_MIN_STEPOVERS` (10), `NOT_USABLE_W30_BELOW` (0.50),
  `PRIZE_CLOSE_BELOW` (1.05), `PRIZE_ABOVE_LITERATURE` (1.25),
  `WANAKA_REGION1_PRIZE_CEILING_PCT` (9.75).
* `zone_coherence_census.rs` now consumes the library estimator AND the
  library census. Disclosed non-divergence: its local
  `Outcome::UnderDetermined` carried no payload where the library's
  carries the starved point count; the census never read it.
* `wanaka_curvature_anisotropy.rs` consumes the library `kappa_perp_zou`
  / `strip_width` / `quantiles` (through a `Fit::to_monge` map) and
  `prize_cell`. TWO disclosed divergences preserved locally, stated in
  the file:
  1. Its `Fit` carries `gather_rms` (RMS XY gather distance), a
     diagnostic `MongeFit` does not; the local estimator stays for it.
  2. Its plain `median` returns the UPPER middle on an even population
     where the library's averages the two middles. Converting would move
     its printed diagnostic medians, so the copy stays, disclosed.
* `bikeseat_gate_d1.rs` consumes the library `TriField`, `TurnGrid`,
  `ZoneVerdict` and `ratio`; its census wrapper keeps the GATE's own
  bars (w30 uses the shared `W30_COHERENT`; the 3-stepover length bar is
  the gate's own and stays local, documented at the library constant).
* New unit pins in `metrology::census`: uniform-vs-alternating field
  separation (verdict + coherence length + w30) and isotropic-vs-
  cylinder prize cells.

## M-5. Union-coverage audit (G-UNIONCOV's fix) — PRE-REGISTRATION

`rs_cam_core::metrology::union_coverage::audit_stock_vs_model`: the
whole-board comparison of a final simulated stock against the model
top surface plus `stock_to_leave`, per grid column, NO relevance
filter (the module doc states why `SimulationResult::column_deviations`
is not this measurement). Standing is VERTICAL. Areas are XY-projected
and resolution-conditional; the report carries `cell_mm`.
`UnionCoverageReport::assert_within(mm2)` is the loud failure API and
its `Display` names the located hotspots.

First consumer: `tests/union_coverage_m1.rs` (`#[ignore]` evidence),
which runs the full generate→simulate fixpoint headless on:

* ARM PRODUCTION — `planning/multitool_2026-08-23/wanaka200_mt2.toml`
  (the production two-tool tier chain, overlap 2.0 mm);
* ARM REJECTED — `wanaka200_mt2_overlap02.toml` (overlap 0.2 mm,
  region floors 100/50 mm² — the variant G-UNIONCOV rejected).

Pinned dials, registered before any run: simulation resolution 0.3 mm;
`stock_to_leave` 0.0; `spec_tolerance` 0.35 mm (0.03 mm commanded cusp
+ ~one 0.3 mm cell of column quantisation); gouge tolerance 0.35 mm;
variant allowance 500 mm².

Pre-registered expectations (written before the run):

1. **ARM REJECTED MUST FAIL** `assert_within(500 mm²)` — §12's
   arithmetic put the missing coverage at roughly 1,800 mm² (16,112 mm
   of tier-1 cutting covering ~7,800 mm² of 9,652 mm² owned), so the
   above-spec area is expected in the high hundreds to thousands of
   mm², concentrated in fine-detail hotspots the report must locate.
2. **ARM PRODUCTION is characterized, not gated.** Expected: above-spec
   fraction well under the variant's — registered bar: the variant's
   above-spec area exceeds production's by at least 3×. Production's
   own residual (rim bands, facet/quantisation noise, any real seam)
   is reported honestly, whatever it reads; a surprise here is a
   finding, not a calibration knob.

Results land below this section after the run, unedited.

## M-5 RESULTS (2026-09-02, release run, 2,297 s wall) — both pre-registrations HELD

Run: `cargo test --release -p rs_cam_core --test union_coverage_m1 --
--ignored --nocapture`. Both arms simulated at cell 0.300 mm
(unclamped), 444,889 columns, 40,040 mm² compared (the full board
footprint). Fixpoint: 3 rounds per arm (4 → 6 → 8 of 8 generated).
Known caught offset-library panics (`cavalier_contours` pline.rs:139)
fired during generation on both arms and were absorbed by the
Checkpoint C guard; report-only, as designed.

| arm | above spec (> 0.35 mm) | fraction | patches | largest patch | max standing | p99 | gouged | cut-through cols |
|---|---|---|---|---|---|---|---|---|
| PRODUCTION (overlap 2.0) | 3,062.0 mm² | 7.65 % | 3,422 | 114.6 mm² | 3.94 mm | 0.844 mm | 303.3 mm² | 752 |
| REJECTED (overlap 0.2) | 18,341.5 mm² | 45.81 % | 831 | **17,161.5 mm²** | 5.43 mm | 2.930 mm | 262.3 mm² | 441 |

* **Pre-registration 1 HELD**: the rejected variant FAILED
  `assert_within(500 mm²)` loudly, and the failure locates the defect:
  one connected 17,161 mm² patch spanning the fine-detail territory
  (bbox [21.9, 26.4]..[218.7, 223.5]) at up to 5.43 mm standing — the
  patch the operator saw by eye and no per-op wire reported.
* **Pre-registration 2 HELD**: rejected / production = **5.99×** ≥ the
  registered 3× discriminator.
* **Production finding, characterized honestly (not gated):**
  production is NOT union-clean. 3,062 mm² (7.65 %) stands above
  0.35 mm in 3,422 small patches (largest 114.6 mm², around (48, 90)),
  while every per-op wire reads clean. Candidate explanations, NOT yet
  attributed: territory owned by no op under the production floors;
  the tier blend-band fringe; rough-only ground the tiers never claim;
  and 0.3 mm column quantisation on steep walls (the 0.35 mm bar is
  only one cell above spec — standing here is VERTICAL, so a steep
  wall's normal excess is smaller by cos θ). Attribution is follow-up
  work; the ruler now exists to run it one dial at a time.
* The gouge columns (303 / 262 mm², max undercut ≤ 1.11 mm) and the
  cut-through columns (752 / 441) include the through-pilot holes and
  back-face interactions; report-only in this instrument.

G-UNIONCOV's gap is closed: a boundary-config change can no longer
pass silently — `union_coverage_m1` is the instrument, and
`UnionCoverageReport::assert_within` is the gate a comparison run
consumes.
