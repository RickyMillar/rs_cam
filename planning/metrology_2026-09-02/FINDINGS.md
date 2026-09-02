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

---

## M2 — band-cell ownership (2026-09-02, handoff from Track H V1 finding 2)

The operator routed three tasks here; Track H is blocked on the first two.

### M2.0 — gate reds fixed

Commit `2e2ef306` left the workspace clippy gate red. Fixed:

- `tests/union_coverage_m1.rs:71` — `ptr_arg`: `run_arm` takes `&Path`.
- `tests/wanaka_curvature_anisotropy.rs:489` — `wrong_self_convention`:
  `Fit::to_monge` takes `self` (`Fit` is `Copy`); the one iterator caller
  adds `.copied()`.

`cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests`
is green again.

### M2.1 — the ownership map and the hardened attribution

**The mechanism (Track H V1 finding 2):** a band's region POLYGON is not
its territory. The `overlap_mm` dilation grows each polygon over
neighbouring bands' cells, extraction can fill small holes, and min-area
absorption relabels small islands. On the V1 region, 42.90 % of the
polygon's 3D area is steeper than the 45° clamp. Any audit that reasons
over region polygons reads a healthy Shallow op as ~half failed by
construction.

**What shipped:**

- `finish_planner::PlannedRegions::labels` — `decompose` now returns its
  post-conditioning band label grid (row-major on the slope-map grid,
  `None` = uncovered). This is the planner's authoritative ownership
  statement; it existed only as a dropped local before.
- `metrology::ownership` — `BandOwnership` wraps that grid with its frame
  (`owner_at` answers at any XY point, resolution-independent up to half a
  classification cell), and `attribute_above_spec` partitions the union
  audit's above-spec population by owner. The `unowned` class is the
  located G-UNIONCOV gap signal: standing material on ground NO band
  claims. The partition never shrinks the union total — it attributes,
  it does not filter.
- Layer precedence is explicit: pass the finer tier first, so tier-seam
  ground is charged to the op expected to finish it.

The M1 production finding ("production is NOT union-clean, 3,062 mm² in
3,422 patches, attribution is follow-up work") now has its attribution
instrument: rerun the M1 arms with per-op `BandOwnership` layers and read
the `unowned` class. Not yet run.

### M2.2 — G2: the two-hypothesis attribution (pre-registered)

`tests/band_cell_ownership_g2.rs`. Decides V1 finding 2's hypotheses (a)
vs (b) by rebuilding the H1 decomposition verbatim and attributing every
in-polygon steep triangle to one of five classes fixed in the file header
before the run:

- `A_fringe` / `A_interior` / `A_uncovered` — the cell's own label is NOT
  Shallow (hypothesis a: the polygon lies, split by whether the overlap
  dilation explains it);
- `B1_underread` — labeled Shallow and the planner's own slope angle is
  ≤ the clamp while the surface is steeper (hypothesis b: the
  classification under-reads the surface — the defect arm);
- `B2_relabel` — labeled Shallow but the planner's own angle EXCEEDS the
  clamp (conditioning relabeled it: hysteresis flood, close, absorption —
  by-design, still unfinishable at spec by a Shallow raster).

The classes partition the steep 3D area, so the totals must reconcile
with V1's 42.90 % on the same region. The instrument also prints the
per-region cell-ownership view (owned vs non-owned in-polygon cells) —
the quantity the M2.1 audit change acts on.

### M2.2 RESULT — RUN 2026-09-02. HYPOTHESIS (a). The planner classification is sound.

Instrument: `tests/band_cell_ownership_g2.rs`, release-fast, 17 s. The
largest Shallow region reproduces V1 exactly (3,104.0 mm² polygon,
42.90 % of 3D area steeper than the 45° clamp), so the two censuses are
on one scale.

**Steep-area attribution, all 16 Shallow regions (4,288.6 mm² steep of
12,182.5 mm² total, 35.20 %):**

| class | mm² | % of steep |
|---|---|---|
| `A_fringe` (non-owned label, within `overlap_mm` of owned) | 4,102.1 | **95.65 %** |
| `A_interior` (non-owned label, beyond overlap) | 24.1 | 0.56 % |
| `A_uncovered` (label `None`) | 0.7 | 0.02 % |
| `B1_underread` (planner grid misses the steepness) | 24.2 | 0.57 % |
| `B2_relabel` (planner saw it; conditioning relabeled) | 137.4 | 3.20 % |

**Ruling: hypothesis (a).** The steep inclusions are ground the planner's
own cell labels assign to MidSteep/VerySteep; they are inside the Shallow
POLYGON almost entirely (95.65 %) because the `overlap_mm = 2.0` dilation
grew the polygon over them. Hypothesis (b) — a planner classification
defect — is 0.57 % (24 mm², sub-cell walls narrower than the 0.3 mm
classification cell; mean under-read 23.4° on those). No planner fix is
warranted. `B2` is min-area absorption doing what it is configured to do
(region #1 carries 73.0 mm² of absorbed steep ground — see the refund
finding below).

**Cell ownership across all regions: 45.83 % of in-polygon covered cells
are owned by Shallow.** A polygon-scoped audit therefore misreads every
dendritic Shallow op as majority-failed. The M2.1 ownership audit
(`metrology::ownership`) is the correct scope; Track H's V1 coverage
readings over region polygons should be re-read with that in mind.

### M2.2b — avenue G's derate refund is 1.000× where it matters (max rule)

The same run prints, per region, the owned-cell slope population under
the production derate filter (mirroring `shallow_region_max_slope_deg`)
and the stepover refund full excision of non-owned cells would buy:
`cos(owned θ_max) / cos(clamp)`.

- The three LARGEST regions (the time carriers): owned θ_max **64.5°,
  76.9°, 80.2°** → refund **1.000×** each. Min-area absorption folds
  small steep islands INTO the owned Shallow set (regions #1/#2 have
  owned-cell p99 at 73°), so θ_max stays at or above the clamp after
  excising every non-owned cell.
- Small regions: refunds 1.02–1.16×; p99-basis readings 1.06–1.19×.

**Consequence for avenue G (`finishing_status_2026-09-01.md` §5 G):** the
~1.41× band-wide derate prize as stated — "excise the non-owned steep
inclusions" — is REFUTED on wanaka tier-1 under the shipped worst-point
(max) derate rule. The inclusions are real and huge (54 % of cells), but
the derate does not move, because the binding tail after excision is the
ABSORBED steep ground inside the owned set, and under a max rule one
steep cell pins the whole region. Two residual routes, neither measured:
(i) stop absorbing steep islands into Shallow (a decomposition dial —
they cost twice: spec-unfinishable B2 ground AND the derate pin), or
(ii) a spacing rule that is not worst-point — bounded-exceedance or
spacing varying along the pass, which is avenue F. The p99-basis column
(1.13–1.19× on several regions) is the size of THAT prize on this board,
and it belongs to avenue F's ledger, not avenue G's.

---

## M3 — the avenue-F prize split (pre-registration, written BEFORE the run)

> Operator go: 2026-09-02, "do it", with the stated suspicion that a
> decomposition route would be marred by "lots of tiny islands". That
> suspicion is bar B-confetti below, fixed before the run.

**Question.** The spacing prize (every pass pays its region's worst
point) has two candidate capture routes: (i) DECOMPOSITION — redraw
regions so each region's worst point is less bad, keep one spacing per
region; (ii) ALONG-PASS — spacing that varies within a pass (the
Eikonal candidate). Which route can capture the prize on wanaka tier-1?

**Population.** Shallow-OWNED cells (the G2 `PlannedRegions::labels`
grid) of the tier-1 fine island, same setup as G2/H1 verbatim. Spacing
model: allowed XY pitch ∝ cos θ(cell), θ from the production
classification slope map, clamped at the 45° threshold. Slope-only —
no curvature refund — stated as conservative; cutting length modelled
as area ÷ pitch, links/retracts excluded (spacing policy only, the
metrology costing scale prices paths, not this).

**Quantities:**

- `L_today` = Σ over regions of (owned cells ÷ cos θ_max,region), with
  θ_max the PRODUCTION derate population (all covered in-polygon
  cells, clamped) — what the shipped raster pays.
- `L_floor` = Σ over owned cells of 1 ÷ cos θ(cell) — per-cell perfect
  spacing. The prize is `P = L_today − L_floor`.
- `L_band(K)` = slope-banded decomposition with fixed edges
  {10°, 20°, 30°, 40°, 45°} and coarser subsets (K = 2, 3, 5): each
  band pays its own upper edge. IDEALIZED (ignores contiguity).
- `L_band_machinable(K)` = same, but each 8-connected component of a
  band smaller than the planner's own `min_region_area_mm2` (16 mm²
  for this tool) is merged UP into the worse neighbouring band before
  costing — the confetti-honest reading. Component censuses printed
  per band (count, area distribution, machinable share).

**Bars (pre-registered):**

- **B-capture:** decomposition route stays open only if
  `L_band_machinable(K ≤ 5)` captures ≥ 70 % of P.
- **B-confetti:** if the machinable-honest capture falls below 50 % of
  the IDEALIZED capture at the same K, the loss is confetti-shaped and
  the decomposition route CLOSES regardless of B-capture.
- Neither bar rules FOR the along-pass route — it only inherits the
  residual prize; its own cost (path code, junction handling) is a
  separate pre-registration that the operator assigns.

### M3 RESULT — RUN 2026-09-02. B-capture PASSES at K = 3. B-confetti does NOT fire.

Instrument: `tests/spacing_prize_split_f1.rs`, release-fast, 17 s.
Population: 68,117 owned Shallow cells (4,257 mm² XY), tier-1. Owned θ:
p50 6.0°, p90 30.0°, p99 at the clamp.

**The prize, measured:** today's per-region worst-point derate pays
**1.354× floor** on spacing policy alone — 26.1 % of the Shallow
cutting length is on the table. (This scale is spacing-policy-only,
owned cells, no links — do not compare it to whole-arm ×floor numbers.)

| ladder | idealized capture | machinable capture | confetti retention |
|---|---|---|---|
| K = 2 (30/45) | 63.3 % | 60.7 % | 96.0 % |
| K = 3 (20/40/45) | 79.4 % | **71.9 %** | 90.6 % |
| K = 5 (10/20/30/40/45) | 92.7 % | 77.0 % | 83.1 % |

- **B-capture (≥ 70 % machinable): PASSES at K = 3.** The decomposition
  route stays open.
- **B-confetti (< 50 % retention closes it): does NOT fire** at any K
  (83–96 %).

**The operator's confetti suspicion, measured:** real in direction, not
fatal in magnitude. The steeper mid-bands ARE confetti (K = 5: the
20–40° bands are 32–44 % machinable, ~1,000+ components each), but the
prize MASS lives in the flat band — p50 slope is 6.0°, and the ≤ 20°
band is 91.6 % machinable. Merging the confetti up costs only 8–17
points of capture.

**What this does NOT price (the next falsifier, F2):** fragments and
links. K = 3's machinable bands still hold hundreds of components, and
the V1/§9 lesson is that fragment count can eat a distance win whole.
Before any planner change, one costed arm comparison on the production
harness (`relink_and_cost_under`, machined-stock link regime): today's
regions + derate vs the K = 3 machinable banding, whole-arm, links
included, spec checked by the union/ownership audit. The along-pass
(Eikonal) route inherits the residual ~28 % of the prize (~7 % of
length) plus whatever F2 shows decomposition losing to links; its own
pre-registration remains with the operator.

---

## M4 — F2: the banded raster vs the shipped derate, COSTED (pre-registration, written BEFORE the run)

F1 passed B-capture on spacing alone. F2 prices what F1 excluded:
fragments and links — the mechanism that killed the V1 tracing arm.

**Arms (whole Shallow band, all regions in one toolpath each):**

- **arm S (control):** per-region raster at the production derate
  (θ_max over covered in-polygon cells, clamped 45°).
- **arm B (banded):** the same raster over the same polygons, split by
  the K = 3 slope bands (edges 20°/40°/45°) from F1, sub-machinable
  components (< 16 mm²) merged into the steeper band before splitting.
  A point with no band assignment (uncovered edge cells) takes the band
  of its own clamped θ, so arm B's territory is arm S's territory
  exactly.

Both arms use the instrument's 0° lattice — the shipped C2 per-region
rotation is orthogonal to the spacing question and cancels between
arms. Spec per cell holds by construction: a cell is only ever cut at
its own band's spacing or tighter (merges go steeper, never flatter).
Band-seam quantisation is sub-cell (marching of the split is at grid
resolution); reported, not gated.

**Costing (identical for both arms):** the H1 harness restated —
production relink (`hookup 25.0, sampling 0.5, reorder, flush_ride,
airborne exemption`) under the machined-stock link ceiling, F-034 cycle
time on the project kinematics. Metrology-costing scale (single-arm,
no setup/tool changes).

**Coverage precondition:** both arms ≤ 1 % unmachined fraction
(the H1 gate) on the union of Shallow-region triangles, AND arm B's
unmachined fraction ≤ arm S's + 0.5 pp. An arm that covers less
cannot win.

**Bars (pre-registered):**

- **B-time:** arm B `time_s` < 0.95 × arm S `time_s` — a ≥ 5 % time
  win, or the planner change is not worth its complexity. F1 predicts
  ~19 % less cutting DISTANCE; links must not eat more than the
  difference.
- Fragments/links: report-only — their cost is priced inside time.

### M4 RESULT — RUN 2026-09-02. B-time FAILS 1.836×. THE DECOMPOSITION ROUTE CLOSES.

Instrument: `tests/banded_raster_costed_f2.rs`, release-fast, 37 s.
A K = 2 robustness arm (30°/45°, report-only) ran beside the
pre-registered K = 3.

| arm | fragments | links | time s | cut mm |
|---|---|---|---|---|
| S (shipped per-region derate) | 1,835 | 1,818 | 3,589.7 | 41,463 |
| B (K = 3 banded) | **8,411** | 8,397 | **6,592.1 (1.836×)** | 72,869 |
| B2 (K = 2 banded) | 7,796 | 7,780 | 6,363.2 (1.772×) | 70,505 |

**F1's spacing prediction was RIGHT and it did not matter.** Arm B's
raw (pre-relink) cutting is 20,827 mm vs arm S's 26,488 mm — 0.79×,
matching F1's ~0.81×. The relink then pays 8,397 surface links (4.6×
arm S's), and the link feed swallows the refund 2.5 times over.

**The mechanism, named:** F1's machinability merge bounded island
AREA; fragments scale with boundary PERIMETER. On dendritic terrain
the slope-band boundary is long at every threshold — K = 2 halves the
band count and removes only 7 % of the fragments. This is the V1 /
synthesis-§9 failure family (ring splitting, fan overlap, band
splitting): every partition of dendritic ground pays its perimeter.

**Caveats, recorded:**

- Arm B is a point-split of a straight raster; a shipped banded
  planner (per-component serpentine, boundary smoothing) would do
  better — but to pass the bar it must shed ~95 % of the fragments
  while F1's own censuses say ~1,000+ components stand at every rung.
  The gap is not implementation polish.
- The absolute coverage-gate numbers (28–34 % "unmachined" even
  sub-clamp) are a knife-edge artifact: arm B is spec-EXACT by
  design, so audit distances sit at exactly the coverage radius on
  near-clamp slopes, and arm S "passes" partly by being over-dense.
  The achieved-spacing gate for terrain arms remains unbuilt (V1
  finding 3). This does not touch the time verdict — no coverage
  convention rescues a 1.8× time loss.

**Consequence — the avenue-F ledger after M3 + M4:**

1. Worst-point spacing overpays by 26.1 % of Shallow cutting length
   (1.354× floor) — measured, real.
2. Excision refunds nothing (G2: 1.000× on the time carriers).
3. Decomposition refunds nothing after links (M4: 1.8×).
4. The ONLY standing route is spacing that varies WITHIN a continuous
   pass — zero added fragments by construction. That is the Eikonal /
   variable-spacing candidate; its pre-registration is the operator's
   to assign, and its bar is now sharp: it must capture a useful slice
   of the 26 % while adding NO fragment bill, or the prize stays on
   the table and the shipped raster stands as the honest optimum of
   its family.

---

## M5 — E1: the graded raster (continuous variable spacing) — pre-registration, written BEFORE the run

> Operator go 2026-09-02: "in my head this was the idea that made sense
> to begin with. continuous variability. So lets give it a hoon!"

**The candidate.** The only standing route from M4: spacing that
varies WITHIN a continuous pass. Concrete form — the graded raster,
a 1D-integrated specialization of the Eikonal iso-field:

- The allowed XY pitch field is `a(x, y) = stepover · cos θ`, θ from
  the production slope map, clamped at 45° (the F1/F2 convention).
- `a` is first ERODED in y by a ±stepover/2 window (a pass pair must
  respect the worst ground BETWEEN them, not the harmonic mean — a
  thin steep ledge between two rows must pull them together).
- Per grid column, `φ(y) = ∫ dy / a_eroded` (prefix sum). `∂φ/∂y =
  1/a > 0`, so φ is strictly monotone in y and every level set
  `φ = k` is a SINGLE-VALUED continuous curve `y = f_k(x)` — a raster
  row that bends and bunches but cannot loop or branch. Fragments can
  arise ONLY from region clipping, exactly where straight rows
  already fragment. That is the zero-added-fragments mechanism, by
  construction rather than by hope.
- Out-of-territory column gaps accumulate at the clamp rate, so
  `f_k(x)` stays continuous across concavities.
- Pass z from a fine drop-cutter grid (0.3 mm, the H1 convention);
  x-sampling at the classification column spacing.

**Arms:** arm S — the F2 control, verbatim (shipped per-region
derate, 0° lattice). Arm E — the graded raster over the same
territory (the in-any-polygon covered mask), costed identically
(production relink, machined-stock ceiling, project kinematics).

**Also reported (arithmetic only, no path code): E0 row-graded
capture** — straight rows with one spacing per row (the worst cell in
each row's strip). Splits the prize between "rows may bend" and "rows
may merely bunch"; if E0 captured most of it, the simpler design
would deserve the build instead.

**Bars (pre-registered):**

- **E-spec:** per column, the vertical gap between adjacent passes
  must not exceed 1.05 × the eroded-min `a` in that gap, for ≥ 95 %
  of sampled gaps (the D1/D2 ledger exceed convention). An arm that
  fails spec cannot win, whatever its time.
- **E-frag:** arm E fragments ≤ 1.10 × arm S fragments — the
  zero-added-fragments claim, with 10 % tolerance for clipping
  differences.
- **E-time:** arm E `time_s` < 0.95 × arm S — the same shippability
  bar as F2.

All three must pass for the candidate to earn a productization
charter. F1 bounds the distance prize at ~26 %; tilt lengthening
(passes bending costs sec(tilt) per column) spends against it and is
reported.

### M7 RESULT — RUN 2026-09-02. PURE SCALLOP WINS: 0.806× WITH COVERAGE EQUALITY. THE BAND MIX DOES NOT EARN ITS OVERLAP ON THIS BOARD.

Instrument: `tests/scallop_solo_vs_unified_s1.rs`, release-fast, 802 s
(arm U 336 s, arm C 264 s, arm R 177 s generation).

| arm | time s | ×U | cut mm | rapid mm | plunges | coverage Δ |
|---|---|---|---|---|---|---|
| U unified band mix | 8,127.8 | 1.000 | 112,437 | 5,457 | 225 | — |
| **C scallop whole board** | **6,551.5** | **0.806** | 97,058 | **11** | **1** | +0.117 pp (equal) |
| R scallop on regions (1-step overlap) | 8,797.5 | 1.082 | 102,639 | 44,710 | 700 | +0.929 pp (FAILS equality) |

- **Bar C-time: PASSES for arm C.** 19.4 % faster than the production
  band mix, coverage differential inside the 0.5 pp equality margin.
- **Where the win comes from:** 15 % less cutting distance (the
  operator's overlap suspicion, confirmed — the band mix's 2 mm
  overlaps and seams), 5.4 km → 11 m of rapids, and 225 → **1** entry
  plunge: one continuous constant-cusp spiral over the whole board.
- **Arm R answers its own question:** keeping the planner's regions
  but scalloping them all LOSES (1.082×, 700 plunges, 44.7 km rapids,
  coverage equality failed) — the region STRUCTURE is the overhead,
  not the per-band strategy choice.
- Consistency check against the M2–M6 chain: no contradiction. The
  F/E chain proved the straight raster is the best RASTER-family
  policy on its band; S1 shows the whole-board scallop simply does
  not pay the band structure's fixed costs (overlap, seams, retracts)
  at all. On flat ground scallop's rings are boundary offsets at the
  same equal-cusp stepover the raster uses — neither arm wins on the
  flats; the scallop wins by never paying the joints.

**Caveats, recorded:**

- Coverage absolute numbers (~60 %) are meaningless here — the
  population includes the vertical rim walls; the DIFFERENTIAL is the
  registered measure and it is clean. Confirmation on the real
  simulated stock (union/ownership audit at sim resolution) is the
  productization gate.
- Costing is `compute_cycle_time` on as-generated toolpaths (single
  op scale; no sim, no adaptive feed).
- The known `cavalier_contours` worker-thread panic fired once
  mid-run (documented dependency class; generation survived).
- On flats the whole-board rings are BBOX-RECTANGLE offsets (corners,
  no terrain alignment) — cosmetic/kinematic quirk; `InsideOut` or
  the compact-spiral bridging are candidate polish, unmeasured.

**Consequence:** the band-mix design is REOPENED for terrain-class
work. The standalone scallop op already exists in the product — the
cheapest enactment is a project-level strategy choice, not new code.
Productization gate before any default changes: run the real project
(both tiers) with scallop ops in place of unified, simulate, and pass
the union/ownership audit plus the standard gate set.

### M7 AMENDED same day — the operator's eye found a 40 mm hole; the win is SUSPENDED; two audit defects found and fixed

**The hole.** The operator asked "why is there a huge blank space in
the middle?" — arm C has ZERO cutting moves in the central 40 mm
window (the enclosed lake basin). Ground truth confirmed by move
counts (U: 10,908 cut moves there; C: 0). Candidate mechanisms, not
yet attributed: the known `cavalier_contours` offset panic fired
during arm C's generation (a failed ring offset may have truncated
the cascade over the basin), or a cascade-topology failure at the
basin rim. Either way this is a candidate PRODUCT defect in the
standalone scallop op on basin-bearing ground — filed as
**G-SCALLOPBASIN**, needs attribution.

**Audit defect 1 — population.** The mesh-centroid audit's population
is ~57 % SUB-TOOL-RADIUS TEXTURE: real-terrain detail narrower than
the R1.5 ball, unreachable by ANY path. The audit measured the tool,
not the arms — which is why a 25-pp-of-window hole diluted to
+0.117 pp and slid under the equality margin. This also recolours
every earlier absolute from this audit family (V1's 47 %, F2/E1's
19–34 %): those absolutes were tool accessibility plus knife edge;
only the differentials meant anything.

**Audit defect 2 — knife edge, root-caused.** The equal-cusp law puts
spec-spaced midpoints at EXACTLY the coverage radius, so `d > r`
coin-flips on all ground machined at exact spec.

**The fixed ruler:** coverage is now RESIDUAL ABOVE THE BALL-REACHABLE
ENVELOPE — population = the 0.25 mm drop-cutter envelope (identical
reference for every arm), uncovered = residual > 1.25 × cusp spec.
Absolutes are finally meaningful (arm U reads 6.0 %).

**Corrected verdict:** arm C +1.221 pp vs U — **FAILS the 0.5 pp
equality precondition. The 0.806× win is SUSPENDED**, per the
pre-registered rule (an arm that covers less cannot win). Projection,
labelled as such: the hole is ~1.2 pp of envelope and ~2–3 % of work;
a basin-fixed arm C plausibly lands ~0.83× — but that is a projection,
not a measurement. Arm R (+0.578 pp) also fails equality narrowly
(it covers the basin — 0.40 % window — its excess is at region
boundaries under the 1-stepover overlap).

**Also answered (operator question): the shipped scallop is NOT
per-point variable.** `RingReducer::Min` (shipped): each ring's
stepover is the MINIMUM over ~20 samples of that ring — one scalar per
ring, worst point rules the whole loop. Spacing varies BETWEEN rings,
never ALONG one. The operator's square-spiral observation was correct.
Per-point iso-scallop (spacing varying along the ring — the operator's
stated ideal, morphing offsets→contours continuously) is unbuilt;
substrate exists (`scallop_isofield.rs`, the `RingReducer` research
seam and its oracle).

**Path forward, in order:** (1) attribute and fix G-SCALLOPBASIN;
(2) re-run S1 with the fixed arm — that settles the suspended win;
(3) if it holds, the productization gate as written; (4) per-point
iso-scallop as the measured upgrade path beyond `Min` — it inherits
arm C's baseline and the envelope-residual ruler.

### M7 RESOLVED 2026-09-03 — G-SCALLOPBASIN attributed; the win stands at 0.850× WITH SUPERIOR COVERAGE

**Attribution (measured, not inferred):** the hole is the DOCUMENTED
`max_rings` truncation (`ScallopReport::uncut_core_mm2`'s own doc):
the shipped ring budget is computed from the FLAT-ground stepover
while `RingReducer::Min` selects a smaller one on every slope, so on
terrain the cascade exhausts its budget before the centre. The
shipped-budget arm reports **uncut_core = 1,904 mm²** — the operator's
blank square, confessed by the generator itself. (The finding DOES
reach production surfaces as `truncated_core_mm2`; the defect is that
the op still emits a silently-short toolpath rather than refusing or
finishing.) The reach-policy budget truncates WORSE here
(5,357 mm²); the clamp-floor budget completes (0 mm²).

**The settled comparison (envelope-residual ruler, all arms):**

| arm | time s | ×U | unmach % | basin |
|---|---|---|---|---|
| U unified (production) | 8,127.8 | 1.000 | 6.007 | ✓ |
| C shipped budget | 6,551.5 | 0.806 | 7.229 | HOLE |
| C2 reach budget | 5,929.3 | 0.730 | 15.262 | HOLE (worse) |
| **C3 clamp-floor (completes)** | **6,906.3** | **0.850** | **2.994** | **✓** |
| R regions + scallop | 8,797.5 | 1.082 | 6.585 | ✓ |

**Bar C-time: PASSES for C3 — 0.850× with coverage 3.0 pp BETTER
than the production op** (equality precondition passed with margin to
spare; the win no longer needs it). One plunge, 7 mm of rapids. The
v3 "+92 % time, 34× over-cut" figure for the naive cap raise did NOT
reproduce on this board: completion cost +5.4 % time over the
truncated arm. That figure was another fixture; on wanaka the
clamp-floor budget is simply correct.

**The measured iso-scallop prize:** C3 pays the Min-reducer crawl to
finish (102,239 cut mm vs C2's 87,871 on the same ground minus the
hole) — the per-ring worst-point tax is ~14 % of cutting distance.
Per-point iso-scallop (the operator's stated ideal) targets exactly
that gap, now with a measured baseline and an honest ruler.

**Enactment recommendation (operator's call):** (a) productization
gate as written, using the clamp-floor budget for whole-board scallop;
(b) G-SCALLOPBASIN product fix — a completing budget or a loud
refusal when `uncut_core_mm2 > 0`, never a silently short toolpath;
(c) iso-scallop as the follow-on candidate.

### M5 amendment — E1b horizontal-erosion sweep (recorded BEFORE the sweep runs; bars unchanged)

The W = 0 (no horizontal regularity) arm ran first: E-time 2.405×,
E-frag 2.532×, E-spec exceed 15.14 %, tilt lengthening 80.28 %, and
E0 = 1.000× (every straight row crosses a clamp cell — bending is
load-bearing). Mechanism: per-column integration has no cross-column
coupling, so passes wander where the field differs between neighbour
columns.

The candidate's design knob is HORIZONTAL EROSION of the pitch field:
`a` is min-eroded in x by ±W before the y-erosion and integration.
Large W → smooth passes but the refund dies (every flat cell sees a
gully within W on dendritic ground); small W → wander. E1b sweeps
W ∈ {1, 2, 4, 8, 16} mm and costs every arm identically. The bars are
UNCHANGED (E-spec / E-frag / E-time from §M5); the sweep only asks
whether ANY W passes all three. If none does, the avenue closes with
the mechanism stated: the allowed-spacing field varies horizontally
faster than any smooth continuous-pass family can follow on this
terrain.

### M5 RESULT — RUN 2026-09-02. E-time FAILS at every W. AVENUE F CLOSES ON THIS TERRAIN.

Instrument: `tests/graded_raster_e1.rs`, release-fast, 44 s. Two harness
asymmetries were found and fixed before the ruling (each moved the
degenerate W=16 anchor toward identity, as it must): the first arm E
lacked the raster emitter's serpentine stay-down turnaround (1.83× →
1.12×), and then chained serpentines across region boundaries instead of
within regions (1.12× → 1.02×). With the anchor honest:

| W erosion | fragments | time s | vs S | E-spec exceed |
|---|---|---|---|---|
| S (control) | 1,835 | 3,589.7 | 1.000× | — |
| 0 mm | 3,134 | 5,656.0 | 1.576× | 15.14 % |
| 1 mm | 2,569 | 4,749.9 | 1.323× | 7.58 % |
| 2 mm | 2,104 | 3,963.5 | 1.104× | 4.20 % |
| 4 mm | 1,963 | 3,650.1 | 1.017× | 1.72 % |
| **8 mm (best)** | **1,962** | **3,637.6** | **1.013×** | 0.44 % |
| 16 mm | 1,962 | 3,650.2 | 1.017× | 0.14 % |

- **E-frag PASSES** at W ≥ 4 (1.069× ≤ 1.10×) — the zero-added-
  fragments mechanism works as designed.
- **E-spec PASSES** at W ≥ 4 (≤ 1.72 % ≤ 5 %).
- **E-time FAILS at every W** (best 1.013× vs the < 0.95× bar). The
  graded raster's best configuration is the straight raster.

**The mechanism, one number:** the decorrelation census — the distance
from flat (θ ≤ 20°) ground to steep (θ ≥ 40°) ground is **p50 0.90 mm,
p90 2.00 mm** across 56,467 flat cells. The spacing prize lives
interleaved with the gullies at SUB-STEPOVER scale. A machinable pass
must be smooth over at least a few millimetres; smoothing the pitch
field over even ±2 mm already clamps it nearly everywhere (field 2D
length 28,900 mm vs the control's 26,488 before any wander), while
below ±2 mm the wander lengthening (up to +80 %) exceeds the refund.
The frontier {refund preserved} ∩ {passes machinable} is EMPTY on this
terrain — measured, not argued.

**The avenue-F ledger CLOSES on wanaka tier-1:**

1. The prize is real: 26.1 % of Shallow cutting length (1.354× floor).
2. Excision refunds nothing (G2) — absorbed steep islands pin the max.
3. Decomposition refunds nothing after links (M4) — partitions pay
   their perimeter.
4. Continuous variable spacing refunds nothing (this run) — the prize
   is spatially finer than any smooth pass can follow.

The shipped per-region worst-point raster is therefore, with evidence,
the honest optimum of its family ON THIS TERRAIN CLASS. The prize is a
property of the terrain, not a defect of the planner.

**What could still reach it (recorded, none recommended now):**

- A SPEC change, not a path change: accept bounded cusp exceedance on
  gully walls (the p99-basis 1.13–1.19× per-region numbers from G2).
  Operator's domain — it trades surface finish for time.
- Smoother work: on bike-seat-class geometry the decorrelation length
  is large, and the graded raster (this instrument, W ≈ the terrain's
  own correlation length) should win there. Reopening condition: a
  census on the target part showing flat-to-steep p50 well above the
  stepover.
- The valley agent's pencil/steep-territory work is untouched by this
  closure — it changes WHO owns the gullies, not the Shallow band's
  spacing physics.

---

## M6 — SCOPE CORRECTION (operator-caught, 2026-09-02) and the tier-0 re-run

**The operator caught a territory error in the whole M2–M5 chain:** every
instrument (G2, F1, F2, E1) inherited its territory from the H1 valley
harness — the TIER-1 FINE ISLAND, i.e. the dendritic detail strips left
over after the coarse tool finishes the mountains. That territory came
from the valley workstream and is the fringe AROUND the drainage
network, not the mountain range. The 0.90 mm flat-to-steep
decorrelation is a property of that fringe. This is meta-error §2 (the
configuration could not express the candidate) applied to territory
selection — the same error class Track H's V1 was re-bounded for.

**Re-bounded rulings:**

- G2, F1, F2, E1 verdicts stand FOR TIER-1 FINE TERRITORY. Every
  "closes on this terrain class" phrasing is narrowed to "closes on the
  tier-1 fringe".
- Avenue F is NOT closed for tier-0 (mountain) territory — UNMEASURED
  there. The E1 reopening condition ("flat-to-steep p50 well above the
  stepover") may be satisfied by wanaka's own tier 0.
- Track H note: their basin CENSUS was full-territory (unaffected), but
  no arm ever built contour toolpaths on mountain ground either.

### M6 pre-registration (written BEFORE the run): E2 = the E1 chain on tier 0

Same harness, tier-0 island, the R1.5 coarse tool, its own
classification surface and planner. Reported in order:

1. **Censuses first:** band shares on tier 0 (Shallow / MidSteep /
   VerySteep, XY and 3D — how much mountain ground actually rasters vs
   already cuts contour-like), and the flat-to-steep decorrelation
   census on the tier-0 Shallow band.
2. **Arm S** (shipped per-region derate) vs the **graded-raster W
   sweep**, bars UNCHANGED from §M5 (E-spec / E-frag / E-time).
3. SVG dumps for the operator's eyeball.

The river-offset field (spacing-graded rings grown from the drainage
network — the programme's founding image) is a SEPARATE arm and gets
its own pre-registration only after these censuses land: if tier-0's
Shallow band decorrelation is as short as tier-1's, no field shape can
help, and if it is long, the graded raster and the river field compete
on measured ground.

### M6 RESULT — RUN 2026-09-02. Tier-0 is a DIFFERENT world and the answer is still no — for a measured, different reason.

Instrument: `tests/graded_raster_tier0_e2.rs` (tier-0 complement
territory, R1.5 tool, rough-only link ceiling), release-fast, ~45 s.

**Censuses (the operator's catch vindicated):**

- Band shares: Shallow 47.0 %, MidSteep 52.1 %, VerySteep 0.9 % —
  half the mountain ALREADY cuts contour-like (scallop); the raster
  question concerns 47 %.
- Decorrelation: flat-to-steep p50 **4.88 mm**, p90 16.34 mm — 8–27
  stepovers, vs 0.90 mm on the tier-1 fringe. The E1 reopening
  condition IS satisfied here; tier-0 is genuinely different ground.
- Refund potential ≈ 16 % of Shallow cutting length (65 % of the band
  is ≤ 20°).

**The W sweep (bars unchanged; 0/2/8/16/32/64 mm):** time falls
monotonically from 1.323× (W=0) toward arm S and NEVER crosses: best
**1.021× at W=64** (E-frag 1.062× passes, E-spec 1.01 % passes).
2D length dips below the control only at W=32 (−0.8 %) where erosion
has already destroyed the refund (p99 flat-to-steep = 23 mm < 32 mm).
Pass simplification to op tolerance (RDP 0.05 mm, applied to every arm
identically) changed nothing — the wiggle is the field, not sampling.
**The erosion dilemma, tier-0 form: the refund lives at 5–16 mm scale;
pass smoothness demands ~2× more; the frontier grazes 1.0 and never
crosses.**

**The contour-alignment census (the operator's topo-map intuition,
measured before building it):** |da/ds| along terrain contours vs
across them, 74,783 cells: **across/along = 1.56×**. Contour-aligned
graded fronts (the river-offset family) would see the field vary only
1.56× slower along the pass than the 0° rows do — nowhere near the
~3–5× grain coherence needed to keep the refund without the wander.
This wanaka carving is knobbly at stepover scale in EVERY direction.
The river-offset arm is therefore NOT BUILT, closed by census — the
same cheap way the catchment-zone idea closed.

**Ruling: avenue F closes on tier-0 as well — but with different
mechanism and different reopening conditions than tier-1:**

- tier-1 fringe: prize interleaved at sub-stepover scale; nothing can
  follow it (M5).
- tier-0 mountains: prize real (~16 %) and coherent at 5–16 mm, but
  every continuous family pays wander ≥ the refund because the grain
  is isotropically rough (across/along 1.56×).
- UNMEASURED residual: a full 2D-Eikonal front solve (lateral coupling
  the column family lacks). Odds stated honestly as thin — it faces
  the same erosion dilemma with at most the 1.56× alignment factor of
  headroom — and it is not recommended without operator push.

**Reopening censuses for ANY future part (now standard, ~40 s each):**
flat-to-steep p50 ≫ stepover AND across/along grain coherence ≳ 3× —
both true → the graded/contour-aligned family deserves the E2 harness
on that part; either false → the straight raster stands.

---

## M7 — S1: pure scallop vs the unified band mix (pre-registration, written BEFORE the run)

> Operator go 2026-09-02: "can we test it without the unified? There is
> a lot of overlap. I imagine just using the scallop by itself might
> win."

**The question.** The unified op pays band machinery: three strategies,
2 mm inter-band overlap, and seams. Pure scallop (constant-cusp rings
over the WHOLE surface, slope 0–90°) has one strategy and no bands. On
mountain-heavy ground the rings are the natural shape — does the band
mix actually earn its complexity?

**Arms, identical mesh / tool (R1.5) / feeds / kinematics / cusp spec
(0.03 mm) / tolerance (0.05):**

- **arm U** — the production unified op, mt2 R1.5 params verbatim (the
  E3 toolpath).
- **arm C** — the standalone shipped scallop op: slope 0–90°,
  OutsideIn, continuous spiral ON, `intra_pass_hookup_mm = 3.0` with
  kinematics (the shipped operation defaults).

**Costed identically:** `compute_cycle_time` on each generated toolpath
with the project kinematics (both generators emit their own feeds and
links — no relink; this compares the ops as shipped). Reported:
time, cutting mm, rapid mm, fragments (EntryPlunge count), retracts.

**Coverage precondition:** the E1-style lifted-centroid audit over the
whole covered board (NO slope filter — neither arm has a derate clamp;
scallop adapts spacing by slope everywhere). Knife-edge caveat (M4)
applies to absolute numbers; the DIFFERENTIAL decides: an arm whose
unmachined fraction exceeds the other's by > 0.5 pp cannot win.

- **arm R** (operator-added before the run) — KEEP the planner's
  regions but scallop every one of them: the same `decompose` regions
  the unified op plans (all three bands), extracted with
  `overlap_mm = ONE STEPOVER (0.597 mm)` instead of 2.0, each region
  fed to the shipped scallop generator as its own boundary. Tests
  whether the win (if any) is "scallop everywhere" or "drop the 2 mm
  overlap and the strategy zoo but keep the region structure".

**Bar (pre-registered):** **C-time** — an arm wins the question if its
`time_s` < arm U's with coverage equality held. No margin requirement:
this is a strategy-choice question, not a planner-change bar; any
honest win reopens the band-mix design.

RESULT: pending.

---

## M8 — S2: iso-field scallop vs the completing cascade (pre-registration, written BEFORE the run)

> Operator observation driving this: the C3 rings are "still mostly
> parallel… I thought it was going to make a more organic shape as it
> traversed mountain ranges." Correct — the cascade offsets the
> BOUNDARY's shape in plan view; terrain only modulates one scalar per
> ring. The organic form the operator expects is per-point iso-scallop.

**The candidate is already in the tree as a research seam** (M4
candidate 3): `RingSource::IsoField` — solve `|∇D| = 1/s(x,y)` from
the boundary, rings = integer level sets; per-point spacing by
construction, ring count = ⌊max D⌋ (a termination invariant, which
also retires the G-SCALLOPBASIN budget class). No shipped caller
selects it.

**Arms (same harness, tool, feeds, cusp, ruler as S1/M7):**

- **U** — production unified (reference row).
- **C3** — the completing cascade (M7's winner), the BASELINE.
- **ISO** — `scallop_toolpath_research` with the SHIPPED policy except
  `ring_source: IsoField`, clamp-floor budget (inert for iso), same
  params as C3.

**Bars (pre-registered):**

- **ISO-cov:** envelope-residual unmachined ≤ C3 + 0.5 pp.
- **ISO-time:** `time_s` < C3's — any honest win adopts iso as the
  research recommendation; < 0.95 × C3 makes it the headline (the
  measured crawl tax is ~14 % of cutting distance, so the prize is
  real if ring placement is the binding cost).
- Gouge safety is NOT covered by the envelope ruler (it only reads
  positive residual); the M4 gouge oracle exists and a production
  adoption must run it. Recorded as a gate, not measured here.

### M8 RESULT — RUN 2026-09-03. ISO-time PASSES EMPHATICALLY (0.722× C3, 0.614× U); ISO-cov FAILS vs C3 (+1.63 pp) — a trade, not a sweep.

| arm | time s | ×U | cut mm | unmach % | basin |
|---|---|---|---|---|---|
| U production unified | 8,127.8 | 1.000 | 112,437 | 6.007 | ✓ |
| C3 completing cascade | 6,906.3 | 0.850 | 102,239 | **2.994** | ✓ |
| **ISO per-point rings** | **4,987.5** | **0.614** | **72,694** | 4.624 | ✓ |

- **ISO-time: PASSES** — 0.722× the completing cascade, 0.614× the
  production op. The per-ring crawl tax realised at ~29 % of cutting
  distance (the 14 % estimate was conservative: C2's reference was
  itself Min-reduced). One plunge, 6 mm rapids, `uncut_core = 0` —
  the termination invariant retires the G-SCALLOPBASIN budget class
  by construction.
- **ISO-cov: FAILS the C3-relative bar** (4.624 % vs bar 3.494 %),
  while still beating PRODUCTION by 1.4 pp. Candidate mechanism (the
  module's own documented cost): rings are grid contours, so ring
  placement carries the field cell as an XY error floor. The obvious
  next dial is a finer field resolution; unmeasured.
- Visual: the operator's expected organic topo-morphology is
  confirmed by eyeball — rings deform around the ranges, spacing
  varies along each ring (`target/scallop_vs_unified_s1/armISO_*`).

**Standing (operator's call):** against PRODUCTION, ISO dominates on
every axis measured here. Against the completing cascade it trades
1.6 pp of envelope coverage for 28 % of time. Adoption path: (1) the
field-resolution probe to close the coverage gap; (2) the M4 gouge
oracle (mandatory gate — this ruler cannot see gouges); (3) the
real-project productization gate as in M7.

### M8b — the field-resolution probe, the gouge read, and the milled sim (2026-09-03)

- **The wiggle is the field cell, confirmed as a dial.** ISO-F
  (explicit 0.35 mm field vs the 0.75 mm envelope-quarter default):
  coverage 4.624 % → **3.740 %** (more than half the C3 gap closed;
  bar ≤ 3.494 % still not met) at time 0.680× U (vs ISO's 0.614×,
  both far under C3's 0.850×). The dial trades smoothness/coverage
  against time roughly linearly; a ~0.25 mm probe plausibly meets the
  bar. The operator's sawtooth observation = the 0.75 mm cell.
- **Gouge oracle at 0.15 mm: INCONCLUSIVE as an absolute gate** —
  all three arms, INCLUDING the shipped-family C3, read deepest
  "gouges" of −850…−981 µm ≈ cell × tan(rim slope): rim-wall
  quantization, not cutting. The COMPARATIVE read is clean: ISO
  gouges no worse than the shipped cascade on every metric. A proper
  gate needs M4-style fine-cell (0.02 mm) crop windows; queued.
- **The milled sim (operator request):** the ISO toolpath alone,
  dexel-stamped at 0.25 mm and composite-rendered
  (`target/scallop_vs_unified_s1/iso_milled.png`, 2 s) — the whole
  board carved by one continuous path, basin included, no hole.
- Coverage residue scatter shows NO corner-diagonal signature — the
  operator's corner suspicion is cosmetic (boundary shape dissolving
  inward), not a coverage defect; residue tracks terrain features and
  the rim band.

### M8c — the operator's eye finds the INVERTED SLOPE LAW; ruling; the feature ships (2026-09-03)

**Operator observation:** "spacing on the steeps looks wider than the
flats — is that wrong?" It is wrong, and it is the DOCUMENTED
inversion in `scallop_math::variable_stepover` (R/cosθ where the
geometry requires ×cosθ; 1.69× too wide at 45°), unfixable for two
months because correcting it alone slammed into the `max_rings`
budget — which the iso-field does not have. The cascade's per-ring
min-reduction had been masking the inversion; per-point ISO exposed
it to the naked eye.

**ISO-C (IsoField + `StepoverGeometry::CosineSlope`, 0.35 mm field):**
time 7,108.3 s = **0.875× U** at **3.507 %** envelope coverage (best
ISO figure; 0.013 pp from the C3 bar — noise). ~29 % of the inverted
arms' speed was spec-cheating width. The corrected arm still beats
production on BOTH axes, and it is the first arm in the product's
history to cut steeps at the true cosine law (the cascade runs the
inverted law under its min-reduction mask).

**Operator ruling:** "is one not more correct? If I want more or less
speed I want to just trade the milling bit — I don't want to trade it
across the surface." **One law ships: cosine. Speed is traded at
`scallop_height` and the tool, never across the surface.** The
inverted variant is a defect, not a mode, and is NOT exposed.

**SHIPPED (same day):** `ScallopConfig::iso_field` (serde default
FALSE — absent key loads the legacy cascade byte-identically),
production wrapper `scallop_toolpath_iso_field_with_cancel`
(IsoField + CosineSlope + cusp-quarter field resolution ≈ the
measured 0.35 mm probe + completing budget), adapter branch in
`generate_scallop`, catalog `ParamDef::optional("iso_field", "bool")`,
GUI checkbox ("Iso-Field Rings") on the scallop panel, wiring
sentries `tests/scallop_iso_field_config.rs`. Workspace gate green.
Remaining before flipping any DEFAULT: the fine-window gouge oracle
and the real-project run.

### G-ISOCHANNEL — operator-caught gouge channel in the iso spiral; FIXED (2026-09-03, `62237834`)

The operator saw a smooth CHANNEL carved through standing terrain in
the viewport and overlaid the paths: an entry/link move. Mechanism:
the continuous-spiral "helical transition" accepts hops up to
`cusp_r × 3.0` (4.5 mm on the R1.5) as CUTTING FEEDS — correct for the
cascade (that is its widest possible ring spacing), wrong for the iso
field, whose ring list is level sets: consecutive entries can be
different loops several mm apart, and the chord ploughs through
whatever stands between. Entry spans measured removing up to 92 mm³
each. THREE instrument lessons:

1. The rapid-collision checker is BLIND to this class — the chord is
   a feed move. A "0 rapid collisions" verdict says nothing about
   feed-move gouging.
2. The triage DID catch it — the `entry_load` critical (884 samples
   biting > 2× median, peak 3.61 mm) — and the agent misattributed it
   to rough-terrace handoff. The signal was on the wire; the reading
   was wrong. The operator's eye settled it.
3. The envelope-residual coverage ruler cannot see gouges (positive
   residual only) — already recorded; this is the concrete case.

Fix: the helical-link bound is ring-source-aware — cascade keeps
`cusp_r × 3.0` byte-identically; the iso field bounds at 1.5× the
flat-ground stepover (its true max ring spacing), so loop-to-loop
hops retract. Expect the iso op's retract count to rise (correctly)
and the channel to vanish. The speckled fillet observation is still
UNATTRIBUTED (candidates: arc-fit sag on curved fillets; sub-threshold
short chords) — re-examine after regeneration under the fix.

### G-RAMPTERRAIN (2026-09-03) — the channel was the RAMP DRESSUP; charter filed, handed to a future session

Post-fix regeneration still showed the channel; the operator pinned it
("ring 70 starts right with it"), and G-code analysis found **877 feed
chords, 19–21 mm, exactly 1.0 mm drop each** — the ramp-entry dressup
(3°) drawing terrain-blind zigzag legs at every ring entry. The
scallop op's DEFAULT dressups carry `entry_style = "ramp"`; the legacy
cascade has shipped with this class for months (too few entries to
see). G-ISOCHANNEL (`62237834`) was a real but separate class; the
operator's channel was the dressup. Attribution correction recorded.

Operator ruling: **"All entry moves should be stock aware"** — design
target, chartered at `planning/entry_moves_2026-09-03/CHARTER.md`.
Immediate mitigation: the iso project is saved with
`entry_style = "none"`. The iso feature's default-off status is
unchanged; its adoption gates now include the entry-chord sentry.

### G-RAMPTERRAIN attribution PROVEN by A/B G-code analysis (2026-09-03)

Frame-corrected (stock frame = model + (20, 25)), strict test: a chord
counts only if BURIED > 0.5 mm below mean terrain along its whole
length. Ramp entries: 3 chords × 19.1 mm buried 0.54–1.36 mm (plus 45
under the looser crest test, up to 21 mm). Plunge entries
(`entry_style = "none"`): the class is GONE. The wanaka ISO project is
saved with plunge entries.

**Residual class found by the same instrument:** two short buried
chords (2.0–2.5 mm, 0.61–1.17 mm deep), BOTH at model (140.5, ~139) —
too long for helical links; signature of a ring chord/arc bridging
through a small knoll. Same family as the operator's fillet-speckle
observation and Checkpoint C §3.8's chord-across-rim gouge. Next
probe: `arc_fitting` dressup off, regenerate, re-analyze + operator
eyeball. The G-code buried-chord analysis is now the working sentry
prototype the charter asks for.

### The stock-blind approach FAMILY — three members, all caught in one afternoon (2026-09-03)

The operator's speckles and channel decomposed into THREE dressup-layer
classes, isolated by A/B G-code analysis (the buried-chord sentry)
plus the triage:

1. **Ramp entries** (`entry_style = "ramp"`): 19–21 mm saw-legs, 3
   chords buried 0.5–1.4 mm (strict test). Gone with plunge entries.
2. **Arc refitting** (`arc_fitting = true`, tol 0.05): with arcs OFF,
   the `entry_load` CRITICAL (2,242 samples, peak 3.90 mm) vanishes
   from the triage entirely, and one of the two knoll chords goes with
   it — the fitter (which groups by feed rate, the known F-arc
   relabelling quirk) emits arcs that sag off the true surface. PRIME
   SUSPECT for the operator's fillet speckles (eyeball pending).
3. **Lead-in radius** (`lead_in_out = true`, 2.0 mm): the final buried
   chord — the plunge lands `lead_radius` away from the ring start and
   approaches HORIZONTALLY at ring depth, terrain-blind, through a
   knoll (G-code context pinned at model (140.5, 138.9)).

All three are the same design gap the operator's ruling names: entry
and approach moves are not stock-aware. The G-RAMPTERRAIN charter
(`planning/entry_moves_2026-09-03/CHARTER.md`) therefore covers the
FAMILY — ramp, helix, lead-in/out, and post-generation arc refits near
entries — not just ramps. The buried-chord G-code analysis is the
sentry prototype; it found all three.
