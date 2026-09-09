# Island clip — what shreds a scallop tier, and the one-dial test

Date: 2026-09-09. Author: study session (deep-DOC modulation, §2.6/§2.7 of
`planning/deep_doc_modulation_2026-09-08.md`). Status: SPEC, code reads
only. No crate edit. The experiments below wait for the GUI.

## 0. The correction this spec starts from

§2.7 of the study doc says the scallop tier is "computed over the whole
board, then every move is tested against the island polygons". That
sentence is wrong. The code has two boundary doors, and the scallop
already uses both:

| door | where | what it does | who uses it |
|---|---|---|---|
| pre-generation regions | `session/compute.rs:1280-1340` resolves `BoundarySource::PlannedTierRegions` into `pre_boundary_regions`; `execute_operation_annotated_with_regions` (`:1561`) threads it into `ExecutionContext::boundary_regions` | the generator sees the island set BEFORE it emits a move | every arm in `execute.rs` that reads `ctx.boundary_regions` (lines 1992–2890): unified_finish, scallop (`scallop.rs:2238-2251` loops `region_boundaries`, one ring set per island), iso scallop (`execute.rs:2182/2193`), waterline, AND drop_cutter (`execute.rs:2842/2851` → `raster_toolpath_from_grid[_with_slope_filter]`, `toolpath.rs:607/773`: a row is cut into engaging runs, one run per island crossing) |
| post-generation clip | `session/compute.rs:1673` re-resolves the same map and calls `clip_toolpath_to_boundary_set_with_provenance` (`boundary.rs:332`) | a move-by-move inside/outside walk; every crossing becomes retract + rapid + `EntryPlunge` | every op with `boundary.enabled` |

Consequences:

1. The scallop is generated per island. T3's narration ("regions 10")
   agrees. The "pre-decompose door for the scallop" direction sent to
   rs-cam-73 is withdrawn.
2. G-RASTERLADDER is NOT a core gap. The drop_cutter grid raster honours
   `boundary_regions`. The gap is in two surfaces only:
   - `plan_multitool_finishing.tier_strategies` has no raster value
     (`multitool.rs:762-777`: `Scallop | IsoScallop | UnifiedFinish`).
   - `set_boundary_config` (`mcp_server.rs:1109`) accepts `stock`,
     `model_silhouette`, `derived_rest_regions` only.
   A project file carries the boundary as data
   (`[setups.toolpaths.boundary.source.planned_tier_regions]`), and the
   loader applies no op-family gate, so a hand-edited copy of T3 gives
   raster-per-island today. That copy is T5 (§3).

## 1. Why T3 has two retracts per ring

`plan_tier_operation` (`multitool.rs:755-777`) sets `continuous: true` on
every scallop tier, with the comment "one continuous spiral is the
measured win (S1)". S1 measured one region. On the island map:

- `scallop.rs:2331`: under `continuous`, a ring-to-ring connector is a
  cutting feed only when the hop is shorter than the widest ring spacing.
  A concave island, a kept-set shift, or the gap between two regions
  makes a longer hop, and the connector falls back to retract / rapid /
  replunge.
- `scallop.rs:2543`: `if params.intra_pass_hookup_mm > 0.0 &&
  !params.continuous` — the drop-cutter-sampled relink that would turn
  those airborne junctions back into surface links is SKIPPED under
  `continuous`.

So on an island map the `continuous` flag disables the only mechanism
that can join rings, and the fallback fires on most junctions. T3: 484
rings, 989 retract trips, 2.04 per ring. T2 (iso field, whole-region
level sets): 1 624 rings, 2 343 trips, 1.44 per ring.

The planner sets a whole-board default on a per-island tier. This is a
planner default, not a clip defect. Hypothesis H1: with
`continuous = false` the relink at 3.0 mm (the planner's own hookup
value) joins the rings and the retract count falls toward the region
count. H1 is falsifiable with one dial and no code.

## 2. What the post-generation clip still does on a per-island scallop

The generator already keeps every ring inside its island, so the
post-clip walk should find no crossing. G-ISOCLIPENTRY says re-entries
fired on T2 and T3, so some emitted moves left their region. Candidates,
all unverified:

- the `continuous` connectors of §1 (rapid at safe Z is inside by
  definition, but the replunge lands on a ring end that may sit on the
  island edge);
- arc fitting (`arc_tolerance 0.05`) and decimation moving ring points
  across the polygon edge by less than the tolerance;
- the 1.25 mm overlap band: the machining set the generator saw is the
  owned set dilated by `overlap_mm`; the post-clip re-resolves the same
  map (memoised), so the two should agree — confirm by counting
  `boundary_clip_dropped` and the clip's retract count on T3b.

Open. Do not solve here. Instrument: read `retract_trips` and
`boundary_clip_dropped` from `get_toolpath_diagnostics` on each run.

## 3. Experiments (all wait for the GUI; plywood copy; sim 0.2; `free -g` ≥ 15 Gi first)

Files (scratch copies of T3, tier 1 block only edited, all parse):

| run | file | change from T3 | reads |
|---|---|---|---|
| T3b | `planning/deep_doc_modulation_2026-09-08/T3b_r10_scallop_islands_relink3.toml` | `continuous = false`, hookup 3.0 | rings, retract trips, fed time, total, entry_load severity/peak, rapid collisions, air |
| T3c | `..._T3c_r10_scallop_islands_relink6.toml` | T3b + hookup 6.0 | same |
| T5 | `..._T5_r10_raster_on_planner_islands.toml` | tier 1 = `drop_cutter` s1.0 F625 P300, entry none, no lead, no link, same `planned_tier_regions` boundary | same + compare with T1 (whole-board raster on rest, 6 478 s pair) |
| T4 (rerun) | not saved; recipe in the study notes | unified tier 1 with steep 85 / waterline 89 → raster band over the whole island | tier-1 generation time (was ~15 min), same reads |

Procedure per file: `load_project` → `cancel_generation` until idle →
`generate_all(fixpoint, simulation_resolution_mm 0.2)` →
`run_simulation(resolution 0.2)` → `get_diagnostics` triage →
`narrate_toolpath(tier index)` → `get_tool_load_report` entry_load →
`screenshot_simulation`. Save nothing over the fixture.

Verdicts:

- H1 holds if T3b retracts < 200 (region count 10, ring count 484) and
  the pair total falls under T1's 6 478 s. Then the fix is one line in
  `plan_tier_operation`: `continuous: false` when the tier carries a
  `PlannedTierRegions` boundary (tier ≥ 1, or tier 0 with
  `coarse_skips_fine_islands`).
- H1 fails if retracts stay near 989. Then the fallback is in the relink
  itself (refused links that leave the region) and §2 moves first.
- T5 answers G-RASTERLADDER's measurement half directly: raster-per-
  island time and retract count against T1. If T5 beats T1, the product
  change is a `raster` value in `tier_strategies` (one arm in
  `plan_tier_operation` that emits `OperationConfig::DropCutter` with
  the equal-cusp stepover) plus `planned_tier_regions` in
  `set_boundary_config`. No core change.

## 4. Commits to make (peer rs-cam-73, by path)

- this file (new, untracked);
- the three scratch TOMLs above (new);
- `planning/deep_doc_modulation_2026-09-08.md` §2.6/§2.7/§2.8 wording
  fixes (this session edits, listed in the hand-off message);
- ledger G-RASTERLADDER: reword to "surface gap (planner strategy value +
  MCP boundary source), core raster already region-aware";
- new ledger row G-TIERCONTINUOUS (planner sets `continuous: true` on a
  per-island tier; relink disabled) — OPEN until T3b reads.

## 5. Results (2026-09-09, entry-fixed binary 09:10, all at 0.2 mm)

Full table and readings: study doc §2.7a. Short form:

| run | change | retracts | pair s | verdict |
|---|---|---:|---:|---|
| T3 rerun | planner default | 929 | 9 535 | baseline on the fixed binary |
| T3b | `continuous: false`, hookup 3 | 613 | 8 065 | H1 PARTIAL: 2.04 → 1.02 retracts per ring, −15 % pair; not the collapse to the region count |
| T3c | hookup 6 | 567 | 7 820 | +46 links only; ring hops exceed the relink reach |
| T3d | T3b at tolerance 0.146 | 1 266 | 10 146 | thinner islands, 2.2× the rings, worse |
| T5 | raster on islands, tolerance 0.05 | 954 | 7 985 | confinement CONFIRMED on the live path; loses to T1 (6 478) on retracts |
| T5b | raster on islands, tolerance 0.146 | 2 172 | 10 497 | halving the area doubles the fragments |

Findings that change the spec:

1. **G-TIERCONTINUOUS**: real, worth −15 %. Fix `plan_tier_operation`
   to `continuous: false` when the tier carries a `PlannedTierRegions`
   boundary. Do not expect more from it.
2. **G-OVERLAPFILL (new, a measured consequence, not a geometry
   defect)**: the overlap dilation in `tier_islands.rs`
   (`region_polygons_from_mask_reported(..., overlap_mm, ...)`) is a
   correct dilation, and a hole narrower than 2 × overlap collapses
   under it by definition; reaching into the coarser territory is the
   band's stated purpose (`tier_islands.rs:81`). On a dendritic map that
   purpose swallows the coarse tool's slivers: at tolerance 0.05 the
   owned set is 12 224 mm² with 1 329 holes (median 3.6 mm²), the
   machining set 29 954 mm² with 404 holes. The fine tier cuts 75 % of
   the board. The levers are dials, not code: an overlap below half the
   sliver width, or a tier tolerance at or above the coarse tool's own
   cusp (0.146 for R2.0 at s1.5). Areas at three tolerances: 0.05 →
   12 224 / 29 954, 0.146 → 4 482 / 17 169, 0.30 → 641 / 3 005 (owned /
   machining, mm²). The dial that shrinks the territory also thins the
   islands, which raises the fragment count (item 3).
3. **Retract-count law**: with the holes kept, every island pass
   fragments further (T3d, T5b). For the contour scallop and the raster
   the pair time follows the retract count; the iso field is the
   exception (T2: 2 207 retracts but 73.6 km of rings against T3's
   38 km on the same islands, 19 804 s against T5b's 10 497 at 2 172).
   The product lever is a surface link between fragments inside one
   region (raster row ends, ring ends), not a tighter boundary. The
   island clip itself is not defective. T4 (unified tier forced to its
   raster band) is redundant after T5 and was not rerun.
4. **Entry load** on the planner-default island scallops stays critical
   on the fixed binary (T2/T3 peak 1.39 mm at (115.8, 182.4, −1.59);
   T3d peak 1.05). Open, to the fix agent.

Measurement script: `planning/deep_doc_modulation_2026-09-08/svg_island_area.py`
(exterior, holes and net area per island from a `preview_tier_map` SVG).

Delivered finish (study doc §2.7b, `cusp_measure.py`): T1 26.7 % of
columns > 0.3 mm (p50 0.133) at 6 478 s; T3c 27.7 % (p50 0.128) at
7 820 s. Same surface, T1 17 % faster. The single-pass R1.5 iso-scallop
(Q3, §2.8) reaches the same bar, 25.5 %, at 4 372 s: the pairs buy no
finish over one pass at 0.3 mm, and their tails (p99, max) are the same.
