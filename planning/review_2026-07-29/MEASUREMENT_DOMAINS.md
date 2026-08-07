# M1 — Measurement domains and provenance inventory

Date: 2026-07-29
Owner: research-m1
Basis: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §M1
(line 317), §2 rule 3, §A.0 (line 852), §H4 (line 886), A/M7 (line 1015).
HEAD: `63d5e8b`.

Contract this document is written to satisfy (plan §M1 "Problem", line 321):

> Without domain and resolution provenance, future A/B tables can be
> numerically correct and conceptually false.

Everything below is cited to `file:line` at HEAD. No cargo command was run
to produce it (read-only inventory).

---

## 0. LIVE HAZARDS — cross-domain ratios in code, not just planning prose

Four found. Two are in **shipped production code**; two are in the
**gated/ignored diagnostic harnesses** that the H4 re-measurement is
supposed to be built on. All four must be closed by PR-0 before any
re-measurement is trusted.

### LH-1 (production, HIGH) — "air cut %" has two different denominators under one name

| site | expression | denominator |
|---|---|---|
| `crates/rs_cam_core/src/session/compute.rs:3077` | `summary.air_cut_time_s / summary.total_runtime_s * 100.0` → `ProjectDiagnostics::air_cut_percentage` | **total runtime** (incl. rapids) |
| `crates/rs_cam_core/src/session/compute.rs:3604` | `tp_summary.air_cut_time_s / tp_summary.total_runtime_s * 100.0`, compared against `op_type().air_cut_high_threshold_pct()` | **total runtime** |
| `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:222-234` | banner, `/ s.total_runtime_s`, threshold `> 20.0` at line 243 | **total runtime** |
| `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:629-643` | "% of runtime" chips, `/ s.total_runtime_s` | **total runtime** |
| `crates/rs_cam_core/src/narrate.rs:1078-1086` | `air_cut_time_s / cutting_time_s * 100.0`, where `cutting_time_s` is `cutting_runtime_s` (`narrate.rs:1138`, `narrate.rs:1145`) | **cutting runtime** |

`CLAUDE.md` documents the metric as *"calibrated against cutting-time, not
wall-clock"* — which matches **only** the MCP narration path. The agent-facing
number and the GUI/verdict number are therefore not the same measure, and the
per-operation thresholds in `air_cut_high_threshold_pct()` were tuned against
whichever one the tuner happened to read. Same stage, same units (seconds),
**different mask** → a ratio that is numerically correct and conceptually
false, exactly the M1 failure mode.

Note the numerator itself is single-axis: `air_cut_time_s` is defined as time
with `engagement.radial_woc_fraction < 0.02` (`simulation_cut.rs:364-371`), a
cylinder-side radial measure that is meaningless for Z-only kinematics
(`metrics_not_applicable`, `simulation_cut.rs:385-401`).

### LH-2 (production, MEDIUM) — `part_area_fraction`: polygon area over a bbox rectangle

- numerator: `regions.first().map(Polygon2::area)` — XY-projected rest-region
  polygon, post-dilation, holes subtracted (`rest_field.rs:697`,
  `polygon.rs:69-73`);
- denominator: `model_footprint_area` = **mesh XY bounding-box rectangle**,
  or stock `x*y` when no mesh (`crates/rs_cam_viz/src/ui/properties/mod.rs:499-508`);
- consumed as a ≥ 0.5 pathology trigger (`rest_field.rs:696-702`) and shown to
  the operator (`properties/mod.rs:3934`, `:4150`).

Both sides are XY-projected mm², so units agree — but the **masks** do not.
A non-rectangular part's bbox overstates its footprint, so the fraction
under-reads and the "single giant region" warning fires late. Not a domain
error; a mask error. It belongs in the same rename/provenance sweep because
the field name (`part_area_fraction`) claims a denominator the code does not
compute.

### LH-3 (diagnostic harness, HIGH) — the 313/482 pattern is *still constructible* today

`crates/rs_cam_core/tests/finish_planner_wanaka_decompose.rs` prints both
sides of the retracted ratio, in one function, ~30 lines apart, with nothing
but a comment preventing their division:

- numerator source, `:75-86` — `area += r.polygon.area()` summed per band
  (XY-projected, post-hysteresis/close/absorption, marching-squares
  extraction);
- denominator source, `:481-494` — `area_3d += a3` over mesh faces past a
  slope threshold (**true 3D face area**), with `area_xy += a3 * cos_slope`
  printed beside it;
- the guard is prose only, `:471-477` and `:501-504`.

The prose guard is the correct fix's *documentation*, not the fix. Nothing in
the type system, the field names, or a test prevents the next reader from
dividing column A of one table by column B of another. `tapered_cusp_radius_sentry.rs:184-191`
records the same lesson, again as a comment.

### LH-4 (diagnostic harness, HIGH) — "~17% of covered area reclaimed" divides mm² by a cell count

`crates/rs_cam_core/tests/v3_cascade_ab.rs`:

- `:1176` `let covered_n = hm.covered.iter().filter(|&&c| c).count();` — a
  **boolean cell count**, no cell-area multiplication anywhere;
- `:1183-1188` per-band `a + r.polygon.area()` — **mm², XY-projected**;
- `:1190-1195` both are printed on one `BAND COVERAGE:` line, `covered_n` as a
  percentage of `rows*cols` and the band areas as `mm2`;
- `:3675-3677` the docstring then states the conclusion:
  *"decompose's extraction reclaiming only ~17% of covered area at Ø1-tip
  dials"* — a ratio between two quantities that are printed in **different
  units** and were never reconciled by `cell²`.

Worse, the band polygons in this probe are extracted with
`planner.overlap_mm = 2.0` (`v3_cascade_ab.rs` via the shared
`build_band_map`, cf. `p2c_headless_ab_wanaka.rs:312-313`), so band areas are
**dilated and mutually overlapping** — summing them across bands double-counts
the overlap ring. Both the unit mismatch and the overlap double-count push the
"17%" in unknown directions.

This is the single most important LIVE hazard for H4, because
`build_band_map` is the territory oracle every A/B branch is scored against.

---

## 1. Inventory

Legend for the columns:

- **DOMAIN** — `3D-surf` (true 3D area), `XY-proj` (area projected onto XY),
  `cells` (a count of grid cells; multiply by cell² for area), `vol` (mm³),
  `len` (path length mm), `time` (s), `dimensionless` (a fraction/count).
- **STAGE** — where in the pipeline the value is captured: `raw`,
  `hysteresis`, `close`, `absorb`, `claim`, `extract` (mask→polygon,
  ± `overlap_mm` dilation), `sim`, `emit` (post-toolpath).
- **RES** — the grid whose cell the value is quantised by, and its cell area.
- **DBL** — can holes / band overlap / multi-group duplication double-count?
- **RATIO** — safe as a ratio numerator/denominator, and against what.
- **SER** — `S` = serde-serialized (compat risk), `W` = JSON-wire key looked
  up by string, `D` = diagnostic-only in-memory (rename freely).

### 1.1 `finish_planner` + `UnifiedFinishReport`

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `FinishPlannerParams::min_region_area_mm2` | `src/finish_planner.rs:136` | XY-proj (threshold) | pre-`absorb` | classification cell (`cusp_radius/4`, `finish_setup.rs:147`) | mm² | n/a | **input only** — never a ratio side | D |
| 2 | `FinishPlannerParams::close_radius_mm` | `finish_planner.rs:132` | len | `close` | ÷ cell at `:359` | mm | n/a | no | D |
| 3 | `FinishPlannerParams::overlap_mm` | `finish_planner.rs:118` | len (dilation) | `extract` | ÷ cell, `region_mask.rs:67` | mm | **creates** overlap | no — but it is the *cause* of DBL in rows 8/9 | D |
| 4 | `absorb_small_regions` local `area` | `finish_planner.rs:653` (`cells.len() as f64 * cell_area`, `:638`) | cells→XY-proj | `absorb` | classification cell² | mm² | no (disjoint components) | safe vs row 1 only | D |
| 5 | `DecomposeStats::raw_steep_islands` / `raw_very_steep_islands` | `finish_planner.rs:216` / `:218` | dimensionless | `hysteresis` (pre-`close`) | — | count | no | safe vs row 7 (same grid) | D |
| 6 | `DecomposeStats::absorbed_regions` | `finish_planner.rs:220` | dimensionless | `absorb` | — | count | **yes** — "absorbed twice across passes counts twice" (`:628-630`) | **not** a share of row 5 | D |
| 7 | `DecomposeStats::region_count` | `finish_planner.rs:222` | dimensionless | `extract` | — | count | no | **the §A.0-sanctioned invariant** | D |
| 8 | `DecomposeStats::claimed_cells` | `finish_planner.rs:229` | cells | `claim` | classification cell² | count | no (first-claim wins, `:225-229`) | needs × cell² before any mm² comparison | D |
| 9 | `PlannedRegion::polygon` → `.area()` | `finish_planner.rs:189`; area at `polygon.rs:69-73` | **XY-proj** | `extract` (± `overlap_mm`) | classification cell (0.125 mm at Ø1 tip; 0.75 mm at shaft) | mm² | **yes across bands when `overlap_mm>0`**; holes subtracted within a polygon | **NEVER vs 3D face area** — this is the invalid `313` | D |
| 10 | `RegionTableEntry::area_mm2` | `src/unified_finish.rs:402` | XY-proj | `extract` (band) / `claim` (crease) | as row 9 | mm² | yes, same as row 9; crease entries sum corridor polygons | mixed-provenance field: band area and crease-corridor area under one name | D |
| 11 | `ClaimsReport::territory_masked_cells` | `unified_finish.rs:424` | cells | pre-`decompose` mask-AND | classification cell² | count | no | safe vs total covered cells only | D |
| 12 | `ClaimsReport::territory_masked_area_mm2` | `unified_finish.rs:428`; computed `:844`, `:852` | cells→XY-proj | pre-`decompose` | classification cell² | mm² | no | **not** comparable to row 9 (different stage: pre- vs post-extraction) | D |
| 13 | `ClaimsReport::detector_coverage` | `unified_finish.rs:418`; = `rest_field.rs:180` | **len** (path length fraction) | rest detector | rest grid `cell_mm` (`rest_field.rs:106`) | dimensionless 0..1 | no | safe — both sides are skeleton mm | D |
| 14 | `ClaimsReport::crease_path_length_mm` | `unified_finish.rs:415` | len | `emit` | — | mm | no | safe vs other cutting lengths | D |
| 15 | `ClaimsReport::crease_path_count` | `unified_finish.rs:413` | dimensionless | `emit` | — | count | no | — | D |
| 16 | `ClaimsReport::post_territory_region_count` | `unified_finish.rs:432` | dimensionless | `extract` | — | count | no | safe vs row 7 | D |
| 17 | `UnifiedFinishReport::uncut_core_mm2` | `unified_finish.rs:472`; accumulated `:935`, `:1013`, `:1351` | XY-proj | generation-time residual | scallop ring polygons (not a grid) | mm² | **yes** — see row 30 (exterior-only shoelace) | not a share of anything currently computed | D |
| 18 | `BandGenStats::{region_count,move_count}` | `unified_finish.rs:188-189` | dimensionless | `emit` | — | count | no | — | D |
| 19 | `RelinkTotals::link_rate()` | `unified_finish.rs:490-493` | dimensionless | `emit` | — | 0..1 | no | safe (same junction population both sides) | D |
| 20 | `UnifiedFinishParams::intra_region_hookup_mm` / `RelinkTotals` counters | `unified_finish.rs:180`, `:477-485` | len / counts | `emit` | — | mm / count | no | — | D |
| 21 | `FinishSurface::cell_size()` — **generation** grid | `src/finish_setup.rs:30`, set `:95` `(cutter.radius()/4).max(tolerance)` | grid metadata | — | shaft-derived | mm | — | **the provenance every row 1/4/8/9/11/12 silently depends on** | D |
| 22 | classification grid cell | `finish_setup.rs:147` `(cutter.cusp_radius()/4).max(tolerance)` | grid metadata | — | tip-derived | mm | — | ≠ row 21 on tapered tools; cells do **not** align 1:1 (`finish_setup.rs:118-123`) | D |
| 23 | coverage erosion (1-cell) | `finish_planner.rs:304-331` | mask | pre-`hysteresis` | classification cell | boolean | — | shrinks every downstream area by a 1-cell rim — must be stated when comparing to an un-eroded mask | D |

### 1.2 Rest-field reports (`rest_field.rs`)

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 24 | `RestFieldReport::total_rest_volume_mm3` | `rest_field.rs:161`; computed `:486-491` (`rest × cell²` over mask) | **vol** | post-mask | `RestFieldParams::cell_mm` (default 0.5, `:132`) | mm³ | no | never vs an area; scales with cell² — **not comparable across cell sizes** | D |
| 25 | `RestFieldReport::coverage()` | `rest_field.rs:180-186` | len | post-trace, post-`min_cut_length` | rest grid | 0..1 | no | **safe** — `traced_length_mm / skeleton_length_mm`, same measure both sides | D |
| 26 | `RestFieldReport::skeleton_length_mm` | `rest_field.rs:167` | len | trace | rest grid (skeleton is cell-stepped) | mm | no | denominator of row 25 | D |
| 27 | `RestFieldReport::traced_length_mm` | `rest_field.rs:169` | len | post-`min_cut_length` | rest grid | mm | no | numerator of row 25 | D |
| 28 | `RestFieldReport::{pencil_region_count, clearing_region_count}` | `rest_field.rs:163`, `:165` | dimensionless | component labelling | — | count | no | safe vs each other | D |
| 29 | `RestFieldReport::region_peak_rest_mm` | `rest_field.rs:171` | depth (len) | post-mask | rest grid | mm | no | never an area | D |
| 30 | `RestFieldReport::{grid_nx, grid_ny}` | `rest_field.rs:173-174` | grid metadata | — | — | count | — | **the only resolution provenance this report carries** — `cell_mm` itself is not recorded | D |
| 31 | `ClearingRegion::cell_count` | `rest_field.rs:150` | cells | component labelling | rest cell² | count | no | needs × cell² for mm² | D |
| 32 | `ClearingRegion::bbox` | `rest_field.rs:147` | XY-proj extent | component labelling | rest grid | mm | — | bbox area ≠ region area (see LH-2) | D |
| 33 | `RestRegionPathology::SingleGiantRegion::part_area_fraction` | `rest_field.rs:673`; computed `:697` | XY-proj / XY-bbox | `extract` (dilated) | rest grid + dilation `pencil_radius + region_margin_mm` (`:126`) | 0..1 | numerator dilated, denominator a rectangle | **LH-2** | D |
| 34 | `RestFieldParams::cell_mm` | `rest_field.rs:106` | grid metadata | — | — | mm | — | provenance for rows 24-33 | D |
| 35 | `RestFieldParams::pencil_radius` | `rest_field.rs:116` | len | routing yardstick | — | mm | — | **H2.1 open defect** — shaft radius today; every row 24-33 is drawn on a mis-scaled mask until fixed | D |
| 36 | pencil-op trace counter `rest_volume_mm3` | `src/pencil.rs:1233`, `:1244` | vol | post-mask | rest cell² | mm³ | no | same caveat as row 24 | **W** (`scope.set_counter` key) |

### 1.3 Scallop standing material

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 37 | `ScallopReport::uncut_core_mm2` | `src/scallop.rs:626`; computed `:545-551` | XY-proj | ring cascade residual (post-`max_rings`) | **not gridded** — polygon shoelace; decimated at `heightmap.cell_size * 0.75` (`:516`) | mm² | **YES** — `shoelace_area(&p.exterior).abs()` **ignores holes** (`:549`) and sums possibly-nested polygons | not a share of region area (no region area is computed) | D |
| 38 | per-region accumulation `uncut_core_mm2 += region_uncut` | `scallop.rs:842`, returned `:851`, `:1078` | XY-proj | as row 37 | as row 37 | mm² | as row 37, across regions | — | D |
| 39 | `GenerationFindings::standing_material_mm2` | `src/compute/execute.rs:65`; filled `:1276-1277`, `:1428-1429` | XY-proj | generation | as row 37 | mm² | as row 37 | — | D |
| 40 | `ToolpathStats::standing_material_mm2` | `src/compute/config.rs:40`; zeroed in the toolpath-only path `src/compute/stats.rs:26` | XY-proj | generation | as row 37 | mm² | as row 37 | **0.0 means "not measured" here, not "nothing standing"** — a silent-zero trap for any ratio | D |
| 41 | `STANDING_MATERIAL_FLOOR_MM2` | `src/diagnostics/adapters/from_generation.rs:28` | XY-proj threshold | diagnostic gate | — | mm² | — | input only | D |
| 42 | `geom.standing_material` diagnostic message | `from_generation.rs:53-59` | XY-proj | diagnostic | — | mm² (`{area:.0} mm²`) | as row 37 | user-visible; must state domain | **W** (id string `ids.rs:61`) |
| 43 | ring-extraction min-area filter `cell*cell` | `src/region_mask.rs:82`; adaptive3d analogue `src/adaptive3d/clearing.rs:1468` | XY-proj | `extract` | source grid cell² | mm² | — | input only | D |

### 1.4 Simulation / deviation harness

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 44 | `ColumnDeviation::dev` | `src/compute/simulate.rs:263`; collected `:1024-1084` | height (len) at a **dexel-top cell** | `sim` | `SimulationRequest::resolution` (`:177`), one sample per z-grid cell (`:1080-1084`) | mm (f32) | **yes across setup groups** — same world XY sampled once per group (`:264-268`) | counts of columns are safe **only** within one `group` and one resolution | D |
| 45 | `ColumnDeviation::top_z` | `simulate.rs:274` | height, **local stock frame** | `sim` | as row 44 | mm | as row 44 | never mix with `dev` (different frames) | D |
| 46 | `ColumnDeviation::{row, col}` | `simulate.rs:279`, `:282` | grid index | `sim` | as row 44 | index | — | the canonical join key; never inverse-transform XY | D |
| 47 | `SimulationResult::deviations` (vertex path) | `simulate.rs:289` | height on a **corner-bilinear 2×2 average** of dexel columns | `sim` + mesh extraction | dexel cell, then MC averaging (`:246-255`) | mm | — | **NOT interchangeable with row 44** — documented to histogram differently for identical real texture (`:246-255`) | D |
| 48 | `SimulationRequest::resolution` | `simulate.rs:177` | grid metadata | `sim` | — | mm | — | provenance for rows 44-47 | D |
| 49 | `SimulationResult::resolution_clamped` | `simulate.rs:302`, set `:503-508` | flag | `sim` | — | bool | — | **when true the effective cell is coarser than `resolution`** and no field records the actual value | D |
| 50 | `SimulationCutSample::removed_volume_est_mm3` | `src/simulation_cut.rs:176`; produced `src/dexel_stock/stamping.rs:445`, `:451` (`× cell_area`) | **vol** | `sim` | dexel cell² | mm³ | no | never vs an area | **S** (schema v5, `simulation_cut.rs:21`) |
| 51 | `SimulationCutSample::mrr_mm3_s` | `simulation_cut.rs:177`; `src/dexel_stock/simulation.rs:525` | vol/time | `sim` | dexel cell² | mm³/s | no | safe | **S** |
| 52 | `Engagement::radial_woc_fraction` | `simulation_cut.rs:72` | dimensionless (cylinder-side width fraction) | `sim` | dexel cell; perp-extent gated on stamp coverage ≥ 0.95 | 0..1 | no | **comparative only** — a genuine full slot reads ≈0.95, not 1.0 (CLAUDE.md) | **S** |
| 53 | `Engagement::axial_doc_fraction` | `simulation_cut.rs:78` | dimensionless (of flute length) | `sim` | — | 0..1 | no | `0.0` means **unknown**, not zero (`:73-77`) — a silent-zero trap | **S** |
| 54 | sub-cell `coverage` (stamping) | `src/dexel_stock/stamping.rs:50`, `:104`, stored `:260`, `:345` | **XY area fraction of one dexel cell** | `sim` | dexel cell², 4×4 sub-samples (`:24-26`) | 0..1 | no | a *third* meaning of the word "coverage" — see §2 N-1 | D |
| 55 | `SimulationToolpathCutSummary::air_cut_time_s` | `simulation_cut.rs:371` | time (radial-WOC axis only) | `sim` | — | s | no | **LH-1** | **S** |
| 56 | `…::low_engagement_time_s` | `simulation_cut.rs:374` | time | `sim` | — | s | no | same denominator ambiguity as LH-1 | **S** |
| 57 | `…::{total_runtime_s, cutting_runtime_s, rapid_runtime_s}` | `simulation_cut.rs:361-363` | time | `sim` | — | s | no | **the two candidate denominators of LH-1** | **S** |
| 58 | `…::average_engagement` | `simulation_cut.rs:378` | dimensionless | `sim` | — | 0..1 | no | comparative only (CLAUDE.md metric caveats) | **S** |
| 59 | `…::total_removed_volume_est_mm3`, `average_mrr_mm3_s` | `simulation_cut.rs:383-384` | vol, vol/time | `sim` | dexel cell² | mm³, mm³/s | no | safe vs each other | **S** |
| 60 | `…::metrics_not_applicable` | `simulation_cut.rs:400-401` | flag | `sim` | — | bool | — | consumers **must** suppress rows 55/56/58 when set | **S** |
| 61 | `SimulationSemanticCutSummary::{air_cut_time_s, average_engagement, peak_engagement, …}` | `simulation_cut.rs:432-443` | time / dimensionless / vol | `sim` | — | mixed | no | same caveats as rows 55-59 at semantic-item scope | **S** |
| 62 | `SimulationProvenance` | `simulation_cut.rs:109-128` | provenance | `sim` | — | — | — | **the existing precedent for §5** — but records *identity* hashes, **not** resolution/domain | **S** |
| 63 | `ProjectDiagnostics::air_cut_percentage` | `src/session/mod.rs:826`; computed `session/compute.rs:3073-3080`; serialized `session/mod.rs:1438` | time ratio | `sim` | — | % | no | **LH-1** | **S** (Serialize-only, MCP wire) |
| 64 | `ProjectDiagnostics::average_engagement`, `…collision_count`, `…rapid_collision_count` | `session/mod.rs:827-829`, serialized `:1439-1441` | dimensionless / counts | `sim` | — | — | no | `rapid_collision_count` is resolution-sensitive — see §7 note on the 0/15/20 sweep | **S** (wire) |

### 1.5 MCP narration + GUI diagnostics adapters

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 65 | `ZLevelPlanMetrics::region_areas_mm2` | `src/adaptive3d/mod.rs:258`; computed `src/adaptive3d/clearing.rs:1479-1481` | **XY-proj at one Z plane** | `extract` (marching squares) | dexel `z_grid.cell_size` (`clearing.rs:346`) | mm² | no (holes subtracted, `polygon.rs:69`) | per-Z-level; **never** sum across Z levels as a surface area | D → **W** |
| 66 | same, on the wire | `src/compute/annotate.rs:367` `scope.set_param("region_areas_mm2", …)` | as row 65 | — | — | mm² | — | JSON key; renaming breaks row 67 | **W** |
| 67 | narration reader `region_areas_mm2` | `src/narrate.rs:596-602` (string lookup), `:71` field, printed `:732-739` as `"top areas [...] mm²"` | as row 65 | — | — | mm² | — | truncated to top 10 (`clearing.rs:1481`) — **a partial list presented without saying so** | **W** |
| 68 | `ZLevelPlanMetrics::residual_cleanup_cell_count` | `adaptive3d/mod.rs:262` | cells | `extract` | dexel cell² | count | no | correctly named — model for the rename convention | D → W |
| 69 | `ZLevelPlanMetrics::{dropped_micro_region_count, dropped_short_region_count}` | `adaptive3d/mod.rs:259`, `:268` | dimensionless | `extract` | — | count | no | one is an **area** filter, the other a **length** filter (`:265-267`) — do not pool | D → W |
| 70 | narration air-cut line | `narrate.rs:1086` | time ratio | `sim` | — | % | — | **LH-1** | — |
| 71 | narration stepover-as-%-of-diameter | `narrate.rs:305-317` | len ratio | config | — | % | — | safe (both mm, same object) | — |
| 72 | narration hotspot share | `narrate.rs:398-401` (`count / total * 100`) | dimensionless | `sim` | — | % | — | safe if `total` is the same sample population | — |
| 73 | GUI banner air-cut % + 20% threshold | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:222-243` | time ratio | `sim` | — | % | — | **LH-1** | — |
| 74 | GUI "% of runtime" chips | `sim_diagnostics.rs:629-682` | time ratio | `sim` | — | % | — | **LH-1** (self-consistent with row 63, inconsistent with row 70) | — |
| 75 | GUI `pct_of_cap` | `sim_diagnostics.rs:976-982`, used `:998-1028` | dimensionless | load gate | — | % | — | safe (peak vs its own cap) | — |
| 76 | GUI engagement provenance hover string | `sim_diagnostics.rs:848` | prose | — | — | — | — | **the only place a domain caveat currently reaches a user** — generalise this pattern | — |
| 77 | GUI `model_footprint_area` | `crates/rs_cam_viz/src/ui/properties/mod.rs:499-508`, `:3250` | XY **bbox** | — | — | mm² | — | **LH-2** denominator | — |
| 78 | worker `stats.standing_material_mm2` passthrough | `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:760` | XY-proj | generation | — | mm² | as row 37 | — | D |

### 1.6 Planning A/B harnesses and diagnostic tests

| # | symbol | file:line | DOMAIN | STAGE | RES / cell area | units | DBL | RATIO | SER |
|---|---|---|---|---|---|---|---|---|---|
| 79 | wanaka decompose band areas | `tests/finish_planner_wanaka_decompose.rs:75-86`, printed `:95-97` | XY-proj | `extract` | classification cell | mm² | overlap 0 here | **LH-3 numerator (`313`)** | D |
| 80 | wanaka ground-truth `area_3d` | `finish_planner_wanaka_decompose.rs:481-494`, printed `:496-500` | **3D-surf** | mesh faces, `raw` | none (exact) | mm² | no | **LH-3 denominator (`482`)** | D |
| 81 | wanaka ground-truth `area_xy` | same, `:492` (`a3 * cos_slope`) | XY-proj | mesh faces, `raw` | none (exact) | mm² | **overlapping faces are not deduplicated** (a fold projects twice) | the *only* legitimate partner for row 79, and only at the matching hysteresis-widened threshold (`:501-504`) | D |
| 82 | slope-distribution diagnostic: true `area%` vs offset `cell%` | `finish_planner_wanaka_decompose.rs:189-248` | 3D-surf **vs** cells | `raw` vs generation-grid | mesh vs `radius/4` grid | % vs % | — | two different denominators, printed side by side — **safe only because it never divides one by the other**; label them | D |
| 83 | P2.e conditioning sweep band areas | `finish_planner_wanaka_decompose.rs:338-361` | XY-proj | `extract` | classification cell **rebuilt per row** for `wanaka_band_mix_vs_cusp_radius` (`:432-444`) | mm² | — | rows vary resolution **and** dials together — explicitly not attributable (`:399-406`) | D |
| 84 | `v3_cascade_ab` band-coverage probe | `tests/v3_cascade_ab.rs:1176-1195`; claim at `:3675-3677` | cells **vs** XY-proj | mask vs `extract`(+2 mm dilation) | classification cell | count vs mm² | **yes** (overlap dilation) | **LH-4** | D |
| 85 | `op_footprint_cells` / `stamp_cells` | `v3_cascade_ab.rs:4297-4330` | **tool-CENTERLINE XY footprint**, 1 mm bins, walked at 0.5 mm | `emit` (toolpath only, no stock) | 1 mm cell = 1 mm² | count ≡ mm² | no (a `HashSet`) | **§7** — this is the `area_mm2` of the mm²/s metric and it is *not* finished area | D |
| 86 | `EFFICIENCY WITH BOUNDS` table | `v3_cascade_ab.rs:4770-4789` | row 85 ÷ time | `emit` + integrator | 1 mm | mm²/s | no | **§7** — see the domain contract there | D |
| 87 | `OP B STRATEGY MIX` `%area` | `v3_cascade_ab.rs:4693-4718` | row 85, per strategy | `emit` | 1 mm | % | **yes** — the code says so at `:4711` ("footprint sums > distinct: overlap") | shares do **not** sum to 100 | D |
| 88 | `SPARSITY` percentages | `v3_cascade_ab.rs:4723-4733` | row 85 | `emit` | 1 mm | % | no | safe — same measure both sides, different ops | D |
| 89 | `BandMap` | `tests/p2c_headless_ab_wanaka.rs:254-276`, built `:294-314` | band label per classification cell | `extract` with **`overlap_mm = 2.0`** (`:312-313`) | classification cell at Ø6 ball (`for_tool(3.0)`) | code 0..3 | steeper band wins on overlap (`:249-253`) | **the verdict band map**; over-dilation mislabels raster-owned flats as mid-steep (known, `:2012-2015`) | D |
| 90 | `fidelity_report` histogram | `p2c_headless_ab_wanaka.rs:449-474` | row 47 (**vertex** deviations) attributed by row 89 | `sim` | image grid `bm.cell * 0.5` (`:469`) | counts per bin | vertices ≠ columns | superseded by row 91 for verdicts | D |
| 91 | `band_shares` / `mid_steep_shares` (`on-size %`) | `p2c_headless_ab_wanaka.rs:3637-3663` | row 44 (**columns**) attributed by row 89 | `sim` | dexel cell, group-filtered (`:3644`) | % of columns in band | group filter present ✅ | **safe** — same population both sides; the plan's preferred quality measure | D |
| 92 | `DEV_EDGES` / `DEV_BIN_LABELS` | `p2c_headless_ab_wanaka.rs:407-415` | height bins | `sim` | — | mm | — | the ±10 µm bin is below repeatability and aliased by a 0.25 mm grid (plan `:915-917`) | D |
| 93 | ball rest-share probe | `p2c_headless_ab_wanaka.rs:3282-3325` | **cells** (leftover > 0.05 mm), banded by row 89 | two heightmaps at a pinned 0.25 mm cell | 0.25 mm | % of covered cells | no | safe — cell-count over cell-count, same grid | D |
| 94 | `ChainOutcome::{finish_removed_mm3, project_removed_mm3}` | `p2c_headless_ab_wanaka.rs:229-235` | vol | `sim` | dexel cell² | mm³ | no | never vs areas | D |
| 95 | `tapered_cusp_radius_sentry` non-shallow area | `tests/tapered_cusp_radius_sentry.rs:175-178`, `:184-196` | XY-proj | `extract` | classification cell | mm² | no | the file itself states area is **not** a safe invariant and gates on region **count** — the §A.0-correct pattern | D |
| 96 | `s1_claims_ab` gates | `p2c_headless_ab_wanaka.rs:3693`, gates `:3786-3830` | row 91 + time | `sim` | dexel cell | pp / % | — | safe; note the lesson at `:3792-3794` (skipped territory = **tail**, not an on-size shift) | D |
| 97 | `region_node_ranges_tile_the_stitched_toolpath` span coverage | referenced in CLAUDE.md memory; spans built `unified_finish.rs:496-510` | len / move ranges | `emit` | — | % of moves | — | safe after `77f2b7a`; was 12.6% before | D |

**Row count: 97.**

| class | rows | distinct fields | compat risk |
|---|---|---|---|
| `S` — serde-serialized | 14 (rows 50-64) | **25** (rows 57, 59, 61, 64 cover several fields each) | persisted sim traces (`SIMULATION_CUT_TRACE_SCHEMA_VERSION = 5`, `simulation_cut.rs:21`) + MCP wire (`ProjectDiagnostics`, `Serialize`-only) |
| `W` — JSON-wire key looked up by string | 7 (rows 36, 42, 65-69) | 7 | producer and consumer must land together |
| `D` — diagnostic-only, in-memory | 76 | — | **rename freely** |

**No project-file (TOML) area/coverage field exists** — see §3.4. PR-0
therefore carries zero project-format risk.

---

## 2. Cross-domain hazard table

Every pair below is either a ratio that **has** been formed, or two values a
reader can plausibly divide because they are adjacent in output, similarly
named, or similarly typed (`f64` labelled `_mm2`).

| ID | numerator | denominator | why it is wrong | status |
|---|---|---|---|---|
| **X-1** | row 79 `Σ PlannedRegion::polygon.area()` (XY-proj, post-conditioning) | row 80 mesh `area_3d` (3D surface) | differ by ~cos(slope); ~10× on near-vertical ribbons. **This is the retracted `313/482 = 65%`** | retracted `63d5e8b`; **prose-guarded only** (`finish_planner_wanaka_decompose.rs:471-477`) — LH-3 |
| **X-2** | row 84 band polygon area (mm², overlap-dilated) | row 84 `covered_n` (cell count, undilated) | **units differ** (mm² vs count) *and* the numerator double-counts band overlap | **LIVE** — LH-4, drives the "~17%" claim at `v3_cascade_ab.rs:3675` |
| **X-3** | row 55 `air_cut_time_s` | row 57 `cutting_runtime_s` **vs** `total_runtime_s` | same name, two denominators; the 20% threshold is applied to one and documented against the other | **LIVE** — LH-1 |
| **X-4** | row 33 rest-region polygon area (dilated) | row 77 mesh XY **bbox** rectangle | denominator is a bounding box, not a footprint; fires the ≥0.5 pathology late | **LIVE** — LH-2 |
| **X-5** | row 17/37 `uncut_core_mm2` (exterior shoelace, holes ignored) | any region area | numerator over-states when the residual polygon has holes or nests; no denominator exists in code, so any "% left standing" a reader constructs is unfounded | latent |
| **X-6** | row 9 band area at cusp-derived cell (0.125 mm) | row 9 band area at shaft-derived cell (0.75 mm) | same field, **different grid** — the whole §A.0 correction. Cell size drove the recovery; the dials bought structure | documented `plan:858-877`; no field records which grid produced a value |
| **X-7** | row 47 vertex `deviations` | row 44 `ColumnDeviation::dev` | corner-bilinear 2×2 average vs raw column top; grid-locked texture survives averaging, phase-diverse cancels — two surfaces with identical texture histogram differently (`simulate.rs:246-255`) | resolved by policy (use columns); **no type prevents mixing** |
| **X-8** | row 44 column counts across setup **groups** | total column count | the same world XY appears once per group (`simulate.rs:264-268`) | guarded in `band_shares` (`p2c:3644`); unguarded anywhere else |
| **X-9** | row 44 column counts at 0.1 mm sim cell | row 44 at 0.5 mm sim cell | population size scales with cell⁻²; also the 0/15/20 collision sweep — never clear collisions across mismatched resolutions | latent; `resolution_clamped` (row 49) can silently change the cell |
| **X-10** | row 24 `total_rest_volume_mm3` at `cell_mm=0.5` | same at another `cell_mm` | Σ rest × cell² — the value is grid-quantised; no cell size travels with the report | latent |
| **X-11** | row 13 `detector_coverage` (path-length fraction) | row 23 `covered` mask (boolean area) | three unrelated meanings of "coverage" — path-length share (row 13/25), boolean area mask (row 23), sub-cell area fraction (row 54) | **naming hazard N-1** |
| **X-12** | row 12 `territory_masked_area_mm2` (pre-decompose cells × cell²) | row 9 band polygon area (post-extract, ± dilation) | different **stage**; extraction erodes by a 1-cell rim (row 23) and may dilate by `overlap_mm` | latent; both are `_mm2` on the same report |
| **X-13** | row 65 per-Z-level `region_areas_mm2` summed over Z | any surface area | summing planar cross-sections is not a surface area; also only the top 10 survive (`clearing.rs:1481`) | latent; narration prints them as bare `mm²` (`narrate.rs:739`) |
| **X-14** | row 85 centreline footprint cells | true finished surface area | a Ø6 ball and a Ø1 tip walking the same centreline finish very different areas; the metric's claim of tool-invariance (`plan:1031`) is **not** satisfied by this implementation | **LIVE for A/M7** — see §7 |
| **X-15** | row 87 per-strategy `%area` shares | 100% | footprint sets overlap; the code says so at `v3_cascade_ab.rs:4711` but the column header does not | latent |
| **X-16** | row 6 `absorbed_regions` | row 5 `raw_*_islands` | the numerator counts an island **once per absorbing pass** (`finish_planner.rs:628-630`); can exceed the denominator | latent |
| **X-17** | row 91 on-size % under band map A | same under band map B | the band map is itself dilated by 2 mm (row 89) and mislabels raster-owned flats; verdicts must attribute by `Region` spans, not band maps | known (`p2c:2012-2015`); band map still used by the gates |
| **X-18** | row 52 `radial_woc_fraction` | 1.0 ("full slot") | a genuine full slot reads ≈0.95 by grid discretisation | documented in CLAUDE.md only |
| **X-19** | row 40 `ToolpathStats::standing_material_mm2 == 0.0` from `stats.rs:26` | "nothing standing" | 0.0 here means **not measured** (toolpath-only helper) | latent silent-zero |
| **X-20** | row 53 `axial_doc_fraction == 0.0` | "no axial engagement" | 0.0 means **unknown** (`simulation_cut.rs:73-77`) | documented in the field doc only |

---

## 3. Proposed renames

Convention: `<domain>_<what>_<unit>`. Domain prefixes:
`projected_xy_` · `surface_` (3D) · `residual_xy_cell_` (cell-count-derived) ·
`removed_volume_` · `path_` (length) · `runtime_` (time).
Counts keep `_count`; cell counts keep `_cells` and never carry `_mm2`.

### 3.1 Diagnostic-only — rename freely (no serde, no wire key)

| current | proposed | file:line | note |
|---|---|---|---|
| `PlannedRegion::polygon.area()` (call sites) | introduce `PlannedRegion::projected_xy_area_mm2()` | `finish_planner.rs:187-190` | additive accessor; leaves `Polygon2::area` alone (broad blast radius, plan rule 10) |
| `FinishPlannerParams::min_region_area_mm2` | `min_region_projected_xy_area_mm2` | `finish_planner.rs:136` | struct-literal construction in tests must follow (`:1204`, `:1536`, `:1546`, `p2c:312`, `v3:` band map) |
| `DecomposeStats::claimed_cells` | `claimed_cell_count` | `finish_planner.rs:229` | already a count; the rename only removes the plural-noun ambiguity |
| `RegionTableEntry::area_mm2` | `projected_xy_area_mm2` | `unified_finish.rs:402` | **also split provenance**: band area vs crease-corridor area (see §5) |
| `ClaimsReport::territory_masked_area_mm2` | `territory_masked_projected_xy_area_mm2` | `unified_finish.rs:428` | |
| `ClaimsReport::detector_coverage` | `detector_traced_length_fraction` | `unified_finish.rs:418` | kills the first of three "coverage" meanings |
| `RestFieldReport::coverage()` | `traced_length_fraction()` | `rest_field.rs:180` | keep `coverage()` as a `#[deprecated]` alias for one release |
| `RestFieldReport::total_rest_volume_mm3` | `removed_volume_…` is wrong here (it is *un*removed): `residual_rest_volume_mm3` | `rest_field.rs:161` | |
| `ClearingRegion::cell_count` | `residual_xy_cell_count` | `rest_field.rs:150` | |
| `RestRegionPathology::SingleGiantRegion::part_area_fraction` | `region_area_over_bbox_fraction` | `rest_field.rs:673` | name the denominator honestly (LH-2); fixing the denominator is a separate behavioural change |
| `ScallopReport::uncut_core_mm2` | `uncut_core_projected_xy_area_mm2` | `scallop.rs:626` | + fix the holes bug (§6 slice 4) |
| `GenerationFindings::standing_material_mm2` | `standing_material_projected_xy_area_mm2` | `compute/execute.rs:65` | |
| `ToolpathStats::standing_material_mm2` | same; change type to `Option<f64>` | `compute/config.rs:40` | closes X-19 (`stats.rs:26` sets `None`, not `0.0`) |
| `UnifiedFinishReport::uncut_core_mm2` | `uncut_core_projected_xy_area_mm2` | `unified_finish.rs:472` | |
| `ColumnDeviation::dev` | `dev_mm` | `simulate.rs:263` | bare `dev` invites mixing with row 47 |
| `SimulationResult::deviations` | `vertex_deviations_mm` | `simulate.rs:289` | makes X-7 visible at the call site |
| `op_footprint_cells` / `area_mm2` column | `centerline_footprint_cells` / `centerline_footprint_mm2` | `v3_cascade_ab.rs:4315`, `:4773` | **§7 depends on this** |
| `BandMap` | `DilatedBandMap` + carry `overlap_mm` | `p2c:254-261` | the 2 mm dilation is invisible at every use site |

### 3.2 Serialized schema — additive only, with serde compatibility

`SimulationCutTrace` and friends derive `Serialize + Deserialize` and carry
`SIMULATION_CUT_TRACE_SCHEMA_VERSION = 5` (`simulation_cut.rs:21`). Traces are
persisted and freshness-checked (`gcode::sim_trace_is_fresh`), so a bare
rename breaks old files.

| current | proposed | file:line | serde compatibility |
|---|---|---|---|
| `SimulationCutSample::removed_volume_est_mm3` | `removed_volume_est_mm3` (**keep**) | `simulation_cut.rs:176` | already domain-explicit; no change |
| `SimulationToolpathCutSummary::air_cut_time_s` | keep the field; **add** `air_cut_fraction_of_cutting_time` and `air_cut_fraction_of_total_time` as computed accessors, not stored fields | `simulation_cut.rs:371` | zero schema change; forces the caller to name its denominator (closes LH-1/X-3) |
| `SimulationToolpathCutSummary::average_engagement` | `#[serde(alias = "average_engagement")] pub average_radial_woc_fraction` | `simulation_cut.rs:378` | `alias` reads old traces; **writing** the new name bumps the schema → set `SIMULATION_CUT_TRACE_SCHEMA_VERSION = 6` |
| `Engagement::axial_doc_fraction` | `Option<f64>` + `#[serde(default)]` | `simulation_cut.rs:78` | closes X-20; `None` on legacy traces is exactly the intended "unknown" |
| `ProjectDiagnostics::air_cut_percentage` | **add** `air_cut_percentage_of_total_runtime` beside it; keep the old key emitting the same value for one release | `session/mod.rs:826`, `:1438` | `Serialize`-only (no `Deserialize`) ⇒ **MCP wire break only**, no file-format break. Bump the field count at `session/mod.rs:1436` (`serialize_struct("ProjectDiagnostics", 8)`) when adding |

### 3.3 JSON-wire keys (string-looked-up) — rename requires both ends in one commit

| key | producer | consumer | plan |
|---|---|---|---|
| `region_areas_mm2` | `compute/annotate.rs:367` (from `adaptive3d/mod.rs:258`) | `narrate.rs:599` (string `get`), printed `:732-739` | rename to `z_level_projected_xy_area_mm2_top10` **and** make the truncation visible; emit both keys for one release, read new-then-old at `narrate.rs:599` |
| `rest_volume_mm3` counter | `pencil.rs:1244`, `rest_field.rs:630` | trace/log readers | rename to `residual_rest_volume_mm3`; log-only, low risk |
| `geom.standing_material` | `diagnostics/ids.rs:61` | GUI diagnostics list, `ids.rs:151` registry | **do not rename the id** (it is a stable diagnostic identity). Change the *message* at `from_generation.rs:53-59` to state domain + provenance |

### 3.4 Project-file (TOML) configs — **no area fields found**

A sweep of `src/compute/operation_configs.rs`, `crates/rs_cam_cli/src/project.rs`
and the config surface found **no** `_mm2` / `_mm3` serialized *configuration*
field. Every area value in this inventory is either a report/telemetry field or
a derived dial (`FinishPlannerParams` is constructed in code from
`cusp_radius`, never deserialized). **Conclusion: PR-0 carries no
project-file compatibility risk.** The only compat surfaces are (a) persisted
simulation traces and (b) the MCP wire.

---

## 4. Provenance struct design

### 4.1 Shape

```rust
/// Where a measured area/coverage value came from. Diagnostic-only:
/// never serialized into a project file. Attached to reports, not to
/// individual scalars, so one struct covers a whole table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasurementProvenance {
    /// What the number measures.
    pub domain: MeasurementDomain,
    /// How far through the conditioning pipeline it was captured.
    pub stage: MeasurementStage,
    /// Grid cell size (mm) the value is quantised by; `None` for exact
    /// (mesh-analytic) measures.
    pub cell_mm: Option<f64>,
    /// Which tool scale derived `cell_mm` — the §14q/§A.0 axis.
    pub cell_source: CellSource,
    /// Dilation applied at polygon extraction (mm). Non-zero ⇒ regions
    /// OVERLAP and their areas MUST NOT be summed across bands.
    pub extraction_dilation_mm: f64,
    /// True when the coverage mask was eroded (rim shrink), so this value
    /// is not comparable to an un-eroded mask.
    pub coverage_eroded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementDomain {
    SurfaceArea3d,
    ProjectedXyArea,
    GridCellCount,
    DexelTopColumns,
    StockVolume,
    PathLength,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementStage {
    RawThreshold,
    Hysteresis,
    MorphologicalClose,
    MinAreaAbsorption,
    CreaseClaim,
    PolygonExtraction,
    Simulation,
    Emission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSource {
    /// `cutter.cusp_radius() / 4` — classification (finish_setup.rs:147)
    CuspRadius,
    /// `cutter.radius() / 4` — generation (finish_setup.rs:95)
    EnvelopeRadius,
    /// `SimulationRequest::resolution` (simulate.rs:177)
    SimResolution,
    /// Caller-pinned (harness fixtures)
    Explicit,
    NotGridded,
}
```

### 4.2 Where it attaches

| report | field to add | file:line |
|---|---|---|
| `DecomposeStats` | `pub provenance: MeasurementProvenance` | `finish_planner.rs:213-230` — filled in `decompose` from `slope_map.cell_size` (`:300`) and `params` |
| `UnifiedFinishReport` | `pub provenance: MeasurementProvenance` | `unified_finish.rs:437-473` |
| `ClaimsReport` | reuse the parent's; add `pub rest_cell_mm: f64` | `unified_finish.rs:408-433` |
| `RegionTableEntry` | `pub area_domain: MeasurementDomain` **or** split into `band_projected_xy_area_mm2` / `crease_corridor_projected_xy_area_mm2` | `unified_finish.rs:395-403` |
| `RestFieldReport` | `pub provenance: MeasurementProvenance` (it already carries `grid_nx/ny` but **not** `cell_mm`) | `rest_field.rs:158-175` |
| `ScallopReport` | `pub provenance: MeasurementProvenance` | `scallop.rs:615-627` |
| `ToolpathStats` | `pub standing_material_provenance: Option<MeasurementProvenance>` | `compute/config.rs:27-41` |
| `SimulationResult` | `pub column_grid_cell_mm: f64` (the **effective** cell after `resolution_clamped`) | `simulate.rs:286-307` — closes X-9 / row 49 |

The existing `SimulationProvenance` (`simulation_cut.rs:109-128`) is the
precedent and stays as-is: it answers *"is this trace fresh for these inputs"*.
`MeasurementProvenance` answers *"what does this number mean"*. Do not merge
them — the first is serialized, the second must not be.

### 4.3 Rendering rule

Any table row printed by a harness or narration that carries an area or a
percentage must print `domain/stage/cell` on the same line or in a header
immediately above it. Golden outputs (§6 slice 5) pin this.

---

## 5. The compile-time gate against reconstructing 313/482

Three layers, cheapest first. Slice 2 of §6 implements them.

**Layer A — different types.** Make the two sides non-divisible:

```rust
// finish_planner.rs
pub struct ProjectedXyAreaMm2(pub f64);
// mesh/analysis
pub struct SurfaceAreaMm2(pub f64);
```

No `Div<SurfaceAreaMm2> for ProjectedXyAreaMm2` impl exists, so
`region_area / face_area` is a **compile error**. Provide exactly one
sanctioned bridge:

```rust
impl SurfaceAreaMm2 {
    /// Project a 3D face area onto XY. Valid ONLY for a single face of
    /// known slope; NEVER for an aggregate over faces of mixed slope,
    /// and NEVER in reverse (projected → 3D is unrecoverable at
    /// vertical faces, where cos(slope) = 0).
    pub fn project_onto_xy(self, cos_slope: f64) -> ProjectedXyAreaMm2 { … }
}
```

Scope the newtypes to **exactly these two** (plan §M1: "Strong area newtypes
are optional… only introduce newtypes if the inventory finds repeated
cross-domain misuse" — the inventory finds it four times, X-1/X-2/X-4/X-14, so
they are earned here and nowhere else yet).

**Layer B — a unit test that names the crime.** In
`tests/finish_planner_wanaka_decompose.rs` (fast, synthetic, not the wanaka
fixture — plan rule 7):

```rust
/// M1 gate: the retracted §14r ratio must not be reconstructible.
/// A 76° wall has 3D area A and projected area A·cos(76°) ≈ 0.24·A.
/// Dividing the decomposition's projected output by the mesh's 3D area
/// yields ~24% "recovered" from a surface that is 100% recovered.
#[test]
fn projected_area_is_not_a_share_of_surface_area() { … }
```

It asserts (a) the two ground-truth columns differ by more than 2× on the
synthetic steep fixture, and (b) `ProjectedXyAreaMm2` and `SurfaceAreaMm2`
carry distinct `MeasurementDomain` tags. Layer A makes the division
uncompilable; Layer B makes the *reason* discoverable when someone reaches for
`.0` to escape the newtype.

**Layer C — a `#[deny]`-style lint by convention.** Any harness printing a
`%` must go through one helper:

```rust
fn ratio_pct(num: (f64, MeasurementProvenance), den: (f64, MeasurementProvenance)) -> f64
```

which `debug_assert!`s `num.1.domain == den.1.domain && num.1.stage == den.1.stage
&& num.1.cell_mm == den.1.cell_mm` and panics in tests with both provenances in
the message. This is what makes LH-4 fail loudly instead of printing "17%".

---

## 6. PR-0 implementation slices (ordered)

Each slice is independently revertible and lints clean before the next
starts. Commit each the moment it is green (durable preference: commit
instruments before gates). No slice changes generated geometry.

### Slice 1 — `MeasurementProvenance` type + attach to five reports (no renames)

- add `MeasurementProvenance`/`Domain`/`Stage`/`CellSource` (new module,
  e.g. `src/measurement.rs`);
- attach to `DecomposeStats` (`finish_planner.rs:213`), `UnifiedFinishReport`
  (`unified_finish.rs:437`), `RestFieldReport` (`rest_field.rs:158`),
  `ScallopReport` (`scallop.rs:615`), `ToolpathStats` (`compute/config.rs:27`);
- add `SimulationResult::column_grid_cell_mm` (`simulate.rs:286`).

**Gate:** workspace clippy `-D warnings`; every existing test compiles with
only `..Default::default()` additions; one new unit test asserts
`decompose` stamps `cell_mm == slope_map.cell_size` and
`extraction_dilation_mm == params.overlap_mm`.

### Slice 2 — the two newtypes + the anti-313/482 test (the plan's gate)

- `ProjectedXyAreaMm2`, `SurfaceAreaMm2`, one-way `project_onto_xy`;
- migrate **only** `finish_planner_wanaka_decompose.rs:75-86` and `:481-494`
  and `tapered_cusp_radius_sentry.rs:175-196` to the newtypes;
- add `projected_area_is_not_a_share_of_surface_area` (synthetic, fast, **not**
  `#[ignore]`).

**Gate (verbatim from plan `:362`):** compile-time proof — deleting the
newtype wrapper in the wanaka diagnostic and dividing the two must fail to
compile; the new test must be in default CI (not `--ignored`) and must run in
under a second.

### Slice 3 — close LH-1 (air-cut denominator)

- add `air_cut_fraction_of_cutting_time()` / `…_of_total_time()` accessors on
  `SimulationToolpathCutSummary` (`simulation_cut.rs:357`);
- convert `session/compute.rs:3077`, `:3604`, `sim_diagnostics.rs:222-234`,
  `:629-643`, `narrate.rs:1086` to call one of them **by name**;
- pick the canonical one for thresholds (`air_cut_high_threshold_pct`,
  `session/compute.rs:3603`) and record the choice in `CLAUDE.md`;
- add `ProjectDiagnostics::air_cut_percentage_of_total_runtime` and bump the
  `serialize_struct` arity at `session/mod.rs:1436`.

**Gate:** a unit test builds one summary with `total_runtime_s = 2 ×
cutting_runtime_s` and asserts the two accessors differ by exactly 2×;
a second test asserts every call site in `rs_cam_core` uses a named accessor
(grep-style test over the source, or make the raw division private).

### Slice 4 — close LH-4 and the scallop holes bug

- `v3_cascade_ab.rs:1176-1195`: multiply `covered_n` by `cell²`, print
  `covered_projected_xy_area_mm2` **and** the cell size; print the band areas
  with an explicit `(overlap-dilated, do not sum across bands)` annotation;
  route the printed `%` through `ratio_pct` (§5 layer C);
- correct or retract the "~17%" docstring at `v3_cascade_ab.rs:3675-3677`;
- `scallop.rs:545-551`: use `Polygon2::area()` (holes subtracted) instead of
  `shoelace_area(&p.exterior).abs()`, or document why nesting cannot occur.

**Gate:** a unit test on a residual polygon with a hole shows the reported
`uncut_core` shrinking by exactly the hole area; the band-coverage probe's
printed line carries units on both quantities.

### Slice 5 — golden diagnostic output including domain + stage

- one golden file per report (`DecomposeStats`, `UnifiedFinishReport`,
  `RestFieldReport`, `ScallopReport`) rendered from a **synthetic** fixture;
- the golden must contain the literal domain and stage words for every
  numeric row; a missing tag fails the diff.

**Gate (plan `:363`):** golden diagnostic output includes domain and stage;
the test is fast and fixture-free (plan rule 7).

### Slice 6 — renames (§3.1 diagnostic-only, then §3.2 additive, then §3.3 wire)

Split into three commits in that order; `§3.3` last because it must land
producer and consumer together (`annotate.rs:367` ↔ `narrate.rs:599`).

**Gate:** `cargo clippy --workspace --all-targets -- -D warnings`; the MCP
narration golden still parses `region_areas_mm2` from an old trace (alias
path) and the new key from a fresh one.

### Slice 7 — review checklist + docs

- add to `CLAUDE.md` under a new "Measurement contract" heading, and to
  `.claude/skills/verify/SKILL.md` as a pre-commit question:

  > **Are compared metrics in the same domain and stage?** Every ratio or
  > `%` must have numerator and denominator agreeing on domain
  > (3D surface / XY projection / grid cells / dexel columns / volume /
  > path length / time), mask, conditioning stage, and grid resolution.
  > Bare "% recovered" is banned. Render the surface before trusting an
  > aggregate.

- update the CLAUDE.md metric-caveats block for the air-cut denominator
  decision from slice 3.

**Gate (plan `:365`):** the checklist item exists and is referenced from
`/verify`; `L1` documentation sweep can cite this file instead of re-deriving.

---

## 7. The efficiency metric the addendum standardizes: mm²/s

**What the addendum says** (`plan:1027-1032`, A/M7):

> area-normalised throughput 0.476 mm²/s vs 0.938 … Use **mm²/s (area
> finished per second)** as the metric: it is invariant to tool and stepover,
> unlike cutting length or wall time.

**What the code currently computes** (`v3_cascade_ab.rs:4770-4789`):

```
area  = |op_footprint_cells(op)|                    // 4315-4330
      = distinct 1 mm XY bins touched by CUTTING moves,
        stamped from the TOOL-CENTRE polyline at 0.5 mm steps   // 4297-4312
time  = per-op integrator seconds                              // 4762-4767
mm²/s = area / time                                            // 4777-4778
```

### The domain/stage/resolution contract mm²/s needs to be valid

| requirement | current status | what PR-0 must pin |
|---|---|---|
| **Domain: finished surface area, not centreline footprint.** The cell set is stamped from the tool *centre*, so a Ø6 ball and a Ø1 tip walking identical paths score identically — the exact opposite of "invariant to tool" | ❌ **violated** — X-14 | either (a) stamp the tool's XY disc (radius-aware), or (b) rename the metric `centerline_footprint_mm2/s` and stop claiming tool invariance. **Do not do both silently.** |
| **Domain: XY-projected, and both branches must use the same projection.** Steep territory finished by two different strategies projects differently onto XY | partially — both sides use the same 1 mm XY bins ✅ | record `MeasurementDomain::ProjectedXyArea` in the table header |
| **Resolution: one pinned cell for every branch in a table.** 1 mm bins with a 0.5 mm walk step (`:4305`) under-sample any move shorter than 0.5 mm and over-count any cell a path merely clips | pinned at 1 mm ✅ but undocumented | print `cell 1.000 mm, walk 0.500 mm` in the header; forbid comparing tables built at different cell sizes (X-9 class) |
| **Stage: `emit` (toolpath-only), *not* `sim`.** The footprint never consults the stock, so a pass that travels over already-finished ground scores the same area as one that finishes fresh ground | ❌ **conceptually wrong for a rest pass** | this is the load-bearing defect for A/M7's "the rest pass is half as efficient" claim: Op B by construction re-traverses Op A's ground (measured at `:4730-4733` — the harness *knows* the overlap and still divides by the full footprint). **Either** intersect with the pass's own rest territory, **or** report `mm²/s over claimed territory` and `mm²/s over traversed footprint` as two named rows |
| **Denominator: which seconds.** `op_a_s`/`op_b_s` are integrator per-op totals including rapids and entries | consistent across rows ✅ | state it: `seconds = integrator per-op total (cutting + rapid + entry)` |
| **Both sides same measure.** Op A, Op B, cascade and D all use `op_footprint_cells` | ✅ | keep; the cascade row correctly uses `union` (`:4786`), not a sum — that is the one place overlap is handled right |

**Recommendation.** The mm²/s metric is *structurally* the right invariant
(the plan is correct that length and wall time are not), but the shipped
numerator is a **tool-centreline footprint at emission stage**, which is
neither "area finished" nor tool-invariant. Until slice 4+ fixes it, the
0.476-vs-0.938 figure in `plan:1027` should be treated the same way §H4 treats
the rest of the campaign: **void, not falsified**. It is listed here so
`SUPERSEDED_CONCLUSIONS.md` (H4 deliverable) can carry a row for it with this
file as the citation.

---

## 8. Notes for downstream work packages

- **H2/H3 acceptance gates must not be phrased as "steep area must
  increase"** (§A.0, `plan:872-877`). Use `DecomposeStats::region_count`
  (row 7) and, for quality, `band_shares` on-size (row 91). Row 95 is the
  worked example of a gate written correctly.
- **H2.1 blocks the meaning of rows 24-33.** `RestFieldParams::pencil_radius`
  is the shaft radius today (row 35), so every rest-field area/coverage in
  this inventory is drawn on a mis-scaled mask. Renaming them is safe now;
  *re-measuring* with them is not, until H2.1 lands.
- **A/M6 blocks row 85/86.** `claims_reference: self_probe` means the rest
  territory the mm²/s numerator should be restricted to does not yet exist as
  a machined-stock measurement.
- **`Region` spans, not band maps, are the correct attribution channel**
  (row 89 vs row 97). The span defect was fixed in `77f2b7a` (coverage
  12.6% → 100%); the verdict tables have not been migrated off the dilated
  band map.
