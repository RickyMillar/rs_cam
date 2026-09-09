# Deep depth-of-cut with feed modulation on plywood terrain — 2026-09-08

Status: MEASURED on the pre-rebuild binary (`1af25c87`). Sections marked
`[after rebuild]` wait for the G-DCFLAT + G-AIRDENOM release build and the
operator's MCP reset.

Fixture: `planning/airrun_2026-08-19/wanaka200.toml` (unmodified). Front
setup, terrain STL, 6 mm two-flute flat end mill (tool 0), R1.5 tapered
ball (tool 3), R2.0 tapered ball (tool 4). Every simulation at 0.2 mm.
Artifacts: `planning/deep_doc_modulation_2026-09-08/`.

Hypothesis under test (operator): on plywood, where precision matters
less than in metal, a DEEP depth of cut in ONE pass with FEED MODULATION
as the load regulator may be the most efficient way to machine terrain.

Pass rule for an arm (orchestrator): all three load gates MODELED and
Within, deflection Within, 0 collisions, air under the op's threshold,
`feeds_provenance = emitted`, and time materially below the adaptive3d
baseline. An unmodeled gate is never a pass.

## 1. Arm G — making it a verdict

Arm G (2026-09-07 addendum): raw `drop_cutter` as the rough, 6 mm flat,
stepover 3.0, fresh stock, White Oak. 1117.8 s (−57 % vs baseline A),
air 30.9 %, 0/0 collisions, deflection 0.036 mm, peak bite 9.27 mm.
Three caveats stopped it from being a verdict. Their status:

### 1.1 Caveat 1 — chipload gate unmodeled (`no_vendor_data`)

Root cause, read from the code:

- `DropCutter` declares `feeds_family: Parallel`, `feeds_pass_role:
  Finish` (`compute/catalog.rs`, `feeds/INTEGRATION.md`).
- `feeds::vendor_lookup::passes_must_match` hard-filters on
  `operation_family`. Material category, tool family and diameter are
  soft or relaxed; the operation family is not.
- The embedded LUT publishes **no `flat_end` row in the `Parallel`
  family**. Every flat-end row is `pocket`, `adaptive` or `contour`
  (census of `crates/rs_cam_core/data/vendor_lut/observations/*.json`).
  The Parallel family holds ball and tapered-ball rows only.

So a flat tool on a drop-cutter raster cannot match any row, the gate
abstains, and the feed modulator (which needs a `ChiploadBand`) is a
no-op. That is also why Arm G ran at exactly the commanded 750 mm/min.

Fix written (uncommitted, uncompiled until the cargo lane is granted):

- `feeds/vendor_normalize.rs::lut_query_for` — one new arm at the single
  routing site both Suggest and the gate call: `DropCutter` + `FlatEnd`
  + declared `Parallel` → `(Pocket, Roughing)`. A flat tool on a mesh
  raster is treated as a ROUGHING use, so it resolves the SAME row the
  `Adaptive3d` rough resolves on that tool and material. The docstring
  says in so many words that the band is a roughing band, not a finish
  recommendation. Ball and tapered-ball tools on `DropCutter` keep
  `(Parallel, Finish)`. No vendor data was invented.
- `tests/lut_resolver_census_a6.rs` — `REROUTES` const gains the third
  entry.
- `feeds/INTEGRATION.md` and `tool_load/optimize/outcome.rs`
  (`LutQueryStamp` doc) — the operation-mapping text names the route.
- New sentry `tests/drop_cutter_flat_roughing_row_g_dcflat.rs`: the
  routed flat drop-cutter query on plywood_hardwood Ø6 2F resolves the
  same `observation_id` as the Adaptive3d route
  (`amana-flat-plywood-hardwood-pocket-6000-2f`, raw band 0.035–0.060
  mm/tooth, `ap_max` 4.2 mm = 0.7 × D), the unrouted query still finds
  no row (pre-fix reproduction), ball tools are untouched, and the other
  Parallel ops are NOT rerouted.

Targeted tests to run when the lane is granted:
`--test drop_cutter_flat_roughing_row_g_dcflat`, `--test
lut_resolver_census_a6`, `--test lut_resolver_purposes_a7`, `--test
wanaka_suggest_integration`, `--test _litmatrix_scallop_refuses_flat`,
`--lib feeds`, `--lib tool_load::chipload`; then `cargo fmt --all --
--check` (the three edited Rust files already pass a standalone
`rustfmt --check`).

Same hole, NOT fixed here (follow-up): `Waterline`, `RadialFinish`,
`HorizontalFinish`, `SteepShallow` and `RampFinish` also declare
`Parallel`, so a flat tool on any of them reads `no_vendor_data` today.
The sentry pins that they are not rerouted, so widening is a deliberate
change.

Expected honest outcome after the fix: the pocket row's `ap_max` is
4.2 mm and the DOC derate runs from there. At a 9.27 mm bite the derated
band sits under the 0.025 mm/tooth rubbing floor (the GUI already prints
`chipload clamped to the matched band ceiling` for this tuple on
plywood). A MODELED `Exceeds` or a modulator pinned at the floor is the
expected reading, and it is the answer, not something to tune around.

### 1.2 Caveat 2 — modulated rerun — MEASURED on the rebuilt binary

Caveats 1 and 2 were one defect: with no chipload band the modulator had
no target. On the rebuilt binary (G-DCFLAT in, `armG_dc6mm_s3.toml`
reloaded with no parameter change, oak, fresh stock, 0.2 mm, `cell_mm`
0.2 verified) the gate reads and the modulator runs:

| | Arm G pre-fix (1af25c87) | Arm G post-fix |
|---|---:|---:|
| total runtime (s) | 1 117.8 | **615.6** (−44.9 %) |
| commanded feed / plunge / rpm | 750 / 541 / 15 000 | same |
| chipload gate | UNMODELED `no_vendor_data` | **MODELED Within, validated**, row `amana-flat-hardwood-pocket-6000-2f` (hardness ×1.033, diameter ×1.0, not extrapolated), band 0.0285–0.0491, median 0.0491 at max, population 99 261 / 99 261 |
| modulator | did not run | touched 4 158 / 4 488, median **+96.2 %**, binding chipload_max 92.6 % / machine_max 7.4 % |
| achieved feed, time-weighted | 750 | **1 377** |
| deflection (validated bar 0.05) | 0.036 | **0.0425** |
| power peak / avail kW | 0.043 / 0.703 | 0.085 / 0.703 |
| peak bite / median (mm) | 9.27 / 2.72 | 9.27 / 2.72 |
| plunge over 1× / peak | 0 / 1, 1.0 | 0 / 1, 1.0 |
| air % of total (blind 0.23, degraded) | 30.9 (mixed-base) | 35.4 (single base) — 218 s |
| collisions / provenance / utilization | 0 / emitted / 1.000 | 0 / emitted / 1.000 |

Reading: Arm G is now a full verdict — all three gates MODELED, all
Within, 0 collisions, emitted — and it is a 616 s rough, 3.1× faster than
the best adaptive3d arm (BE, 1 916 s) and 4.2× faster than the oak A
rough (2 578 s) on that binary. Two honest notes. First, the §1.1
expectation ("Exceeds or floor-pinned at a 9 mm bite") did NOT come true,
and the reason is measurable on the plywood row where the raw bounds are
known: the same `amana-flat-plywood-hardwood-pocket-6000-2f` row with the
same hardness scale (0.9129) and diameter scale (1.0) reads 0.0320–0.0548
on the A-ply rough at its 4.2 mm DOC (§2.4, = 0.035/0.06 × 0.9129, no
derate at `ap_max`) and 0.0275–0.0471 on the G-fine arms at a 9.3 mm
bite (§1.2a) — both bounds × 0.86. So the DOC derate IS applied; the
stamp does not label it, and it is far milder than §1.1's reading of the
derate law (0.86×, not "under the floor"). The modulator's +88 % (plywood)
and +96 % (oak) is the gate running that derated ceiling against a
Suggest recipe that was clamped to the 0.025 rubbing floor. Second,
deflection is now the closest gate on this arm at 85 % of its validated
bar, up from 72 % un-modulated. The `feed_explanation` prints
`queried_pass_role: finish` next to `row_pass_role: roughing`; the stamp
names the routed row, so the display shows the declared role, not the
routed one.

#### 1.2a G-fine — the flat raster as the ONLY pass on plywood

Same op, plywood copy, fresh stock, `apply_feeds` speeds (F825 / P512 /
16 500 rpm, Suggest clamped 0.0244 → 0.025 floor), `min_z −18`, slope
0–90, modulation ON, 0.2 mm (`cell_mm` 0.2 verified on each). The
terrace proxy is stepover × tan(slope) at the terrain's area-weighted
median (45.3°) and p90 (63.6°).

| Arm | stepover | total s | fed s | air % total / cutting | air s | air measurability | chipload (plywood pocket row, validated) | modulator median Δ / achieved | deflection (validated) | power kW | peak / median bite | plunge | terrace p50 / p90 (mm) |
|---|---:|---:|---:|---|---:|---|---|---|---:|---:|---|---|---|
| GF10 | 1.0 | **1 807.9** | 1 734.7 | 64.3 / 65.0 | 1 163 | NOT MEASURABLE (blind 0.51) | Within, 0.0275–0.0471, median at max | +88.4 % / 1 460 | 0.0147 | 0.032 | 9.36 / 0.39 | 0/1 | 1.01 / 2.02 |
| GF15 | 1.5 | **1 177.7** | 1 148.4 | 56.7 / 56.7 | 667 | degraded (0.42) | Within, same row | +88.3 % / 1 470 | 0.0148 | 0.033 | 9.38 / 1.66 | 0/1 | 1.52 / 3.02 |
| GF20 | 2.0 | **868.7** | 850.3 | 48.2 / 48.2 | 419 | degraded (0.37) | Within, same row | +88.8 % / 1 471 | 0.0147 | 0.031 | 9.32 / 2.16 | 0/1 | 2.02 / 4.03 |
| Arm G (oak) | 3.0 | 615.6 | 607.5 | 35.4 | 218 | degraded (0.23) | Within, hardwood pocket row | +96.2 % / 1 377 | 0.0425 | 0.085 | 9.27 / 2.72 | 0/1 | 3.03 / 6.05 |

All four: 0 collisions, provenance emitted, utilization 1.000, 100 %
feed-bound, 2 retract trips, one continuous serpentine.

Reading: on plywood the flat raster with the modulator is a 15–30 min
job at any stepover, every gate modeled and Within, deflection at 30 % of
its bar. It is NOT a finish: the terrace it leaves on the median slope is
1–2 mm at these stepovers and 2–4 mm at p90, against 0.09–0.38 mm cusps
for the ball arms, and every valley narrower than 6 mm is untouched
(reach not measured on drop_cutter — `untouched_material_mm2` reads
null). As a ROUGH it beats every adaptive3d arm on this fixture (A-ply
2 475 s, E-ply 1 632 s) at stepover ≥ 1.5, with a 9.3 mm full-slot first
row that the deflection model calls 0.015 mm on plywood. Air is over the
45 % bar on GF10/GF15/GF20 and the metric is degraded-to-unmeasurable at
0.2 mm on a flat tool at these stepovers, so the air column is not a
pass/fail input here.

Artifacts: `armG_modulated_oak.toml` + `_sim.png`, `GF10_flat_s10.toml`
+ `_sim.png`, `GF15_flat_s15.toml` + `_sim.png`, `GF20_flat_s20.toml` +
`_sim.png`.

### 1.3 Finish pair on Arm G stock — MEASURED

Same fixture finish (R1.5 tapered ball drop_cutter, stepover 0.3, F1260 /
P270 / 19 000 RPM, `from_remaining_stock`) after the Arm G rough, White
Oak, 0.2 mm, so the numbers compare with the A / BE / C2E pairs of
2026-09-07. Modulation rewrites feeds, not geometry, so the stock the
rough hands over is the same whether or not the rough was modulated; this
measure did not need the rebuild.

One trap first, because it cost a run: **the simulation runs toolpaths in
INDEX order, not dependency order.** The fixture finish sits at index 6
and the Arm G rough was appended at index 8, so the first pair ran the
finish on FRESH stock and then the rough on the finished surface (the
rough's deflection population was 14 samples; the finish read a 4.20 mm
peak bite). That reading was discarded. The finish was added again at
index 9 (`7b Finish R1.5 after DC rough`, identical params) and index 6
disabled. A `from_remaining_stock` op must have a HIGHER index than the
op it follows.

| Measure | Arm G + finish | A + finish | BE + finish | C2E + finish |
|---|---:|---:|---:|---:|
| project total_runtime_s | **12 739.2** | 14 245.4 | 13 544.5 | 13 782.5 |
| finish `crosses_standing_material` | 14.6 % > 0.47 mm (3 × 0.16 median bite) | 12.2 % > 0.56 (3 × 0.19) | 15.9 % > 0.62 | 12.6 % > 0.58 |
| finish peak bite mm (at) | **2.59** (142.2, 194.1, z 0.81) | 2.93 | 3.14 | 2.93 |
| finish `peak_removed_mm` | 1.18 | 0.97 | 1.57 | 1.17 |
| finish fed_time_s | 11 415.2 | 11 486.8 | 11 451.9 | 11 491.2 |
| finish deflection mm / chipload | 0.019 / Within | 0.02 / Within | — | — |
| collisions / rapid | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |

The flat raster hands the finish a slightly deeper median bite (0.16 vs
0.19 mm after A — the raster's 3 mm terraces) but a LOWER peak (2.59 vs
2.93 mm) and the pair is the fastest of the four (−10.6 % vs A + finish).
The finish itself ran modulated (421 207 of 444 221 moves touched, median
−38.5 %, binding constraint chipload_max on 94.8 %), chipload Within on
`amana-tapered-hardwood-parallel-3175-2f`, `feeds_provenance = emitted`.

`entry_load`: absent from `get_diagnostics.triage.actions`. On a
drop_cutter with no entry intent the orchestrator's reading is that it may
be NOT MEASURED rather than clean, and this build's wire carries no
`is_measured` field for it, so it is reported as "not fired / not
distinguishable from not measured". Logged by the orchestrator as a
surfacing gap.

What this pair also shows: the finish is 11 415 of the 12 739 s. Roughing
strategy can move the pair by about 10 %; the finish is where the time
is. That is what §2 measures.

Artifacts: `armG_finish_pair_sim.png`, `armG_finish_pair.toml`.

### 1.4 Caveat 3 — a DOC cap for drop_cutter

`get_operation_schema("drop_cutter")` lists exactly: `stepover`,
`feed_rate`, `plunge_rate`, `min_z`, `slope_from`, `slope_to`,
`spindle_rpm`. There is **no depth-per-pass, no Z clamp ladder and no
multi-pass field**. `min_z` cannot substitute: `execute.rs`
`generate_drop_cutter` clamps non-contacted points to `min_z` and passes
the same value to `raster_toolpath_from_grid` as `min_z_filter`, which
treats every clamped point as a HOLE and skips it (`toolpath.rs:607-650`).
A `min_z` ladder therefore DROPS path below the clamp instead of stepping
down to it. A DOC cap on drop_cutter is a code follow-up. It was not
faked with stepover.

## 2. Plywood matrix

Fixture copy: `planning/deep_doc_modulation_2026-09-08/wanaka200_plywood.toml`
(material Baltic Birch, everything else as the original). Every arm: fresh
`load_project` of that copy, all fixture toolpaths disabled, ONE toolpath
on the front setup (a new one for the ball and flat arms; the fixture's
index 5 re-enabled for A-ply, C2-ply and E-ply), `apply_feeds(scope = speeds)` so the feed,
plunge and RPM come from the plywood band, `generate_all` (0.2 mm),
`run_simulation(0.2)`. Modulation ON (session default: `ConstrainedMax`,
aggressiveness 1.0). Each arm is the WHOLE job on fresh stock unless the row
says otherwise.

Binary note: every arm in this table ran on the GUI built at `1af25c87`,
which predates the G-BOUNDARYPLUNGE fix (`301f2cbc`). The
`plunge_class_load` readings are therefore the PRE-FIX evidence, on five
operations that had never shown it before (scallop, spiral, radial,
waterline, adaptive3d). The arms marked "rerun" are to be re-measured on
the rebuilt binary.

Terrain slope distribution (area-weighted over the STL's 661 k triangles,
55 893 mm² of surface): p10 5.3°, p25 26.1°, p50 45.3°, p75 53.6°, p90
63.6°, p95 70.0°. A flat end mill's terrace on a slope θ is
stepover × tan θ: at the median slope it equals the stepover, at p90 it is
2.0 × the stepover. A ball's cusp is stepover-only.

### 2.1 Headline table

Time = `total_runtime_s` (modulated kinematic wall clock). Air seconds =
`air_cut_pct_of_total_runtime × total_runtime_s / 100` (the orchestrator's
G-AIRDENOM ruling: compare arms on absolute air seconds, because the two
percentages have different time bases). "Achieved" is the time-weighted
achieved feed after modulation; "cmd" is the operation's commanded feed.
The arms did not share one commanded base feed (776 / 925 / 954 / 1 100
mm/min after `apply_feeds`), so the absolute air seconds are approximate
in the sense of the G-AIRDENOM ruling; the cutting distance and achieved
feed are in the table for the cross-check. "Peak bite" is the axial depth
removed at one column; the ×D figures use the ENGAGED diameter — 6.0 mm on
the flat arms, and on the tapered balls the 3.0 mm tip (a 9 mm bite on a
tapered ball engages the 6 mm shank, which is why deflection stays low).
The BASELINE for the pass rule's "time materially below the adaptive
baseline" is the WHOLE job on the front setup — an adaptive3d rough PLUS
the R1.5 finish — never the rough alone; the rough-only rows (A-ply,
C2-ply, E-ply) are references for the roughing lever, and the measured
pairs are in §2.4.

Two instrument notes on the winners: the `air_cut` and
`radial_engagement` metrics carry a `degraded` measurability flag on the
ball arms (`cell_too_coarse_for_tip_contact`; blind fraction B15 0.14, S20
0.10, B10 0.20, B20 / SP15 / WL15 measurable), so the air bar is judged on
a degraded metric for B15 and S20; and the deflection gate's confidence is
`validated` on the flat arms but `approximate` on every ball arm ("slot
engagement; climb/conventional split not modeled").

| Arm | Op / tool / geometry | Time s | Air s (tot % / cut %) | Coll. | Chipload | Deflection mm | Peak bite mm | Achieved / cmd mm/min | Retracts | Cusp proxy | Verdict |
|---|---|---:|---:|---|---|---:|---:|---:|---:|---|---|
| A-ply | adaptive3d rough, 6 mm flat, s1.2, DPP 4.2 (2 levels) — ROUGH ONLY | 2 475 | 1 083 (43.8 / 38.1) | 0/0 | MODELED Within (pocket row, at max) | 0.008 | 4.20 (0.70 D) | 1 538 / 1 100 (+64 %) | 471 | leaves 0.5 axial for a finish | reference; air 43.8 % is over the 40 % roughing bar; plunge 7/588 at 2.1× (pre-fix) |
| S15 | iso-scallop R1.5 taper, h 0.38 (s≈2.0) | 8 001 | 6 481 (81.0 / 63.9) | 0/0 | MODELED Within (extrap. hardwood scallop 3175 row, at max) | 0.018 | 8.54 | 405 / 925 (−22 %) | 359 | 0.38 ball cusp | FAIL air 81 % > 45; plunge 3/706 at 3.6× (pre-fix) |
| S20 | iso-scallop R2.0 taper, h 0.27 (s≈2.0) | **2 389** | 878 (36.8 / 35.3) | 0/0 | MODELED Within (hardwood scallop 6000 row, no extrap., at max) | 0.007 | 8.54 | 798 / 925 (−4 %) | 45 | 0.27 ball cusp | PASSES all bars except 1/109 plunge at 2.7× (pre-fix) — rerun |
| B15 | drop_cutter raster R1.5 taper, s 1.5 | **3 274** | 321 (9.8 / 14.1) | 0/0 | MODELED Within (MDF parallel 3175 row, at max) | 0.019 | 9.61 | 538 / 776 (−31 %) | 2 | 0.20 ball cusp | **PASSES every bar**; no triage actions at all |
| B10 | drop_cutter raster R1.5 taper, s 1.0 | 4 788 | 762 (15.9 / 22.2) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.018 | 9.05 | 558 / 776 (−28 %) | 2 | 0.086 ball cusp | PASSES every bar |
| SP15 | spiral_finish R1.5 taper, s 1.5 | 3 806 | 960 (25.2 / 30.9) | 0/0 | MODELED Within (extrap. scallop row, at max) | 0.020 | 9.64 | 564 / 925 (−27 %) | 198 | 0.20 ball cusp | plunge 108/197 at 3.6× (pre-fix) — rerun; 18.7 km of rapids |
| RA06 | radial_finish R1.5 taper, 0.6° (1.5 mm at the rim) | 8 718 | 6 039 (69.3 / 57.5) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.018 | 8.62 | 452 / 776 (−26 %) | 539 | 0.20 at rim, denser inward | FAIL air 69 %; plunge 178/536 at 3.0×; wrong pattern for a square part |
| WL15 | waterline R1.5 taper, z_step 1.5 (heights pinned 7 / −3) | 18 368 | 7 081 (38.6 / 41.9) | 0/0 | **UNMODELED** no_vendor_data | 0.019 | 8.37 | 192 / 954 | 1 327 | 1.5 mm lateral at 45°, ridges taller than the ball on shallow slopes | FAIL: unmodeled gate, plunge 498/1480 at 3.7×, 13 700 s of the fed time is link motion at ~130 mm/min |
| B20 | drop_cutter raster R1.5 taper, s 2.0 | 2 438 | 163 (6.7 / 9.8) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.019 | 9.75 | 532 / 776 (−31 %) | 2 | 0.38 ball cusp | PASSES every bar; no triage actions |
| C2-ply | adaptive3d rough, by_area, else as A-ply — ROUGH ONLY | 2 277 | 976 (42.9 / 36.4) | 0/0 | MODELED Within (pocket row, at max) | 0.008 | 4.20 (0.70 D) | 1 544 / 1 100 (+64 %) | 433 | leaves 0.5 axial | reference; air 42.9 % is over the 40 % roughing bar; ZERO over-limit plunges emitted |
| E-ply | adaptive3d rough, DPP 5.46 = ONE Z level, else as A-ply — ROUGH ONLY | 1 632 | 729 (44.7 / 35.7) | 0/0 | MODELED Within (pocket row, at max) | 0.010 | 5.46 (0.91 D) | 1 604 / 1 100 (+64 %) | 223 | leaves 0.5 axial | the deep-DOC adaptive control: −34 % vs A-ply; air 44.7 % is over the 40 % roughing bar; plunge 1/270 (pre-fix) |

Reference pairs from the oak fixture (2026-09-07 and §1.3): A rough + R1.5
finish 14 245 s; Arm G flat raster + R1.5 finish 12 739 s.

Cusp arithmetic beside each ball arm (`h = r − sqrt(r² − (s/2)²)`, with r
the ball radius and s the stepover; the scallop arms set h directly and the
op derives s):

| Arm | r mm | s mm | h mm | note |
|---|---:|---:|---:|---|
| B10 | 1.5 | 1.0 | 0.086 | 1.5 − sqrt(2.25 − 0.25) |
| B15 | 1.5 | 1.5 | 0.201 | 1.5 − sqrt(2.25 − 0.5625) |
| SP15 | 1.5 | 1.5 | 0.201 | same stepover, spiral pattern |
| B20 | 1.5 | 2.0 | 0.382 | 1.5 − sqrt(2.25 − 1.0) |
| S15 | 1.5 | ≈2.0 | 0.38 | set by h; s = 2·sqrt(2rh − h²) = 1.996 |
| S20 | 2.0 | ≈2.0 | 0.27 | set by h; s = 2.007 |
| RA06 | 1.5 | 1.5 at the rim | 0.20 at the rim | spacing shrinks toward the centre |
| WL15 | 1.5 | z_step / tan θ | 0.20 at 45°; > r on slopes under 27° | contour spacing depends on slope |

For the flat-tool arms (after the rebuild) the proxy is the terrace
`s · tan θ` at the terrain's median (45.3°) and p90 (63.6°) slope: s 1.0 →
1.0 / 2.0 mm, s 1.5 → 1.5 / 3.0 mm, s 2.0 → 2.0 / 4.0 mm, s 3.0 → 3.0 /
6.0 mm.

### 2.2 Kinematic readout per arm

| Arm | utilization | feed_bound | machine_bound | headroom_at_1_30 | plunge.peak_ratio (over 1×) | feeds_provenance | modulator: touched / median Δ / binding |
|---|---:|---:|---:|---:|---|---|---|
| A-ply | 0.9995 | 0.990 | 0.0098 | 0.212 | 2.15 (7/588) | emitted | 10 647/11 109, +64.3 %, chipload_max 70 % / kin_reach 25 % |
| S15 | 0.9991 | 0.995 | 0.0049 | 0.227 | 3.61 (3/706) | emitted | 61 039/72 718, −22.3 %, chipload_max 80 % / machine_max 16 % |
| S20 | 0.9997 | 0.9945 | 0.0055 | 0.222 | 2.71 (1/109) | emitted | 42 727/43 847, −4.3 %, chipload_max 92 % / kin_reach 5 % |
| B15 | 1.000 | 1.000 | 0.000 | 0.231 | 1.00 (0/1) | emitted | 17 756/17 955, −30.9 %, chipload_max 99 % |
| B10 | 1.000 | 1.000 | 0.000 | 0.231 | 1.00 (0/1) | emitted | 39 600/40 199, −28.3 %, chipload_max 98.5 % |
| SP15 | 1.000 | 1.000 | 0.000 | 0.231 | 3.61 (108/197) | emitted | 17 994/20 254, −27.5 %, chipload_max 89 % / machine_max 11 % |
| RA06 | 1.000 | 1.000 | 0.000 | 0.231 | 3.03 (178/536) | emitted | 86 004/93 141, −26.4 %, chipload_max 92 % / machine_max 8 % |
| WL15 | 0.999 | 0.9987 | 0.0013 | 0.230 | 3.73 (498/1480) | emitted | no modulation (no band) |
| B20 | 1.000 | 1.000 | 0.000 | 0.231 | 1.00 (0/1) | emitted | 9 999/9 999, −31.5 %, chipload_max 100 % |
| C2-ply | 0.9995 | 0.990 | 0.0100 | 0.212 | 1.00 (0/538) | emitted | 9 919/10 337, +64.3 %, chipload_max 69 % / kin_reach 26 % |
| E-ply | 0.9997 | 0.994 | 0.0060 | 0.215 | 2.15 (1/270) | emitted | 7 606/8 020, +64.3 %, chipload_max 74 % / kin_reach 20 % |

Every arm is ≥ 99 % feed-bound with about 0.22–0.23 headroom at a 1.30×
feed rise; the machine never binds. The commanded feed is the constraint on
every arm, and on every ball arm the modulator's binding constraint is the
chipload band MAXIMUM — the gate's median reads exactly at the band max.
That is the modulator working as designed: it runs the band ceiling in
shallow bites and backs off in the deep ones (median −22 to −31 % on the
R1.5 arms), and the deflection and power gates never came near their bars
(≤ 0.020 mm against 0.05; ≤ 0.015 kW against 0.87).

### 2.3 What the renders show

- `B15_dc_ball_r15_s15_path.png` / `B10_…`: one continuous serpentine, two
  retract trips, no fragmentation. The simulated stock shows the full
  terrain with no missed region.
- `S20_scallop_r20_h027_path.png`: concentric offset rings with a few
  fed links; 45 retracts.
- `S15_scallop_r15_h038_path.png`: the same op with the R1.5 tool is
  visibly fragmented (436 rings, 359 retracts) and its cutting distance is
  1.7 × S20's at the same nominal stepover. Not investigated further; the
  iso-field solver on the smaller tip is the suspect, and the time
  difference (8 001 vs 2 389 s) is mostly that fragmentation plus the
  narrower extrapolated band (max 0.0194 vs 0.0239 mm/tooth).
- `SP15_spiral_r15_s15_path.png`: a clean spiral clipped to the square
  footprint; the clipping is what costs 198 retracts and 18.7 km of rapids.
- `RA06_radial_r15_path.png`: rays converge on a dense plunge cluster at the
  centre (the air advisories all sit there); the rim is under-covered.
- `WL15_waterline_r15_path.png`: contour fragments joined by long fed
  links; the links dominate the picture and the time.

Close-ups of the two winners (orchestrator ask: never rank on an aggregate
without the surface). The offscreen composite is a height colour map with
no lighting, so the 4800 × 3200 render was cropped to a 45 mm window of the
TOP panel and contrast-stretched (`ImageOps.autocontrast`); the path render
was cropped at the same window without processing:

- `B15_closeup_surface_top_ac.png` / `B15_closeup_surface_frontleft_ac.png`:
  the 1.5 mm raster striation runs across the terrain as a beaded texture
  on every contour; no missed strip.
- `B15_closeup_path_top.png`: the raster rows at 1.5 mm pitch with their
  Z-follow steps.
- `S20_closeup_surface_top_ac.png` / `S20_closeup_surface_frontleft_ac.png`:
  the offset-ring texture as beads along each ring; the rings follow the
  contours, which is why the pattern reads as terrain rather than as a
  grid.
- `S20_closeup_path_top.png`: the rings at ~2 mm spacing with two of the 45
  fed links crossing the window.

A lit close-up of the cusps themselves is not obtainable through the MCP
surface: `screenshot_simulation` is a flat colour map and `set_ui_view` has
no camera control. The engagement histograms are the quantitative
complement: B15 spends 31.7 % of its in-cut samples at heavy engagement
and 17.9 % in air; S20 42.0 % heavy and 25.2 % air.

Reproducibility: B15 and S20 were reloaded from their saved TOMLs for the
close-ups, regenerated and re-simulated, and read the same
`total_runtime_s` to the millisecond (3274.427 and 2389.176).

### 2.4 The same-material pair — MEASURED

The cross-material ratio in §3 (plywood B15 against the OAK A + finish
pair) needed a same-material partner. Run on the plywood copy: fixture
index 5 (adaptive3d rough, C2 default, DOC 4.2) and index 6 (R1.5
drop_cutter finish, stepover 0.3, from_remaining_stock), both through
`apply_feeds` (speeds), modulation ON, 0.2 mm, generate_all fixpoint (2
rounds, 1 simulation) and one final simulation. Binary `1af25c87`.

| | A-ply rough (idx 5) | R1.5 finish (idx 6) | pair |
|---|---:|---:|---:|
| commanded feed / plunge / rpm | 1 100 / 512 / 16 500 | 776 / 256 / 18 500 | |
| fed time (s) | 1 921 | 11 630 | |
| total runtime (s) | | | **14 263.8** |
| air, % of total / % of cutting | 43.8 / 38.1 | 45.1 / 45.8 | 44.8 / 44.3 |
| air, absolute (s, approx.) | ≈ 1 150 | ≈ 5 250 | ≈ 6 400 |
| collisions / rapid collisions | 0 / 0 | 0 / 0 | 0 / 0 |
| chipload gate | MODELED Within, validated, row `amana-flat-plywood-hardwood-pocket-6000-2f`, band 0.032–0.055, median 0.0548 (at max) | MODELED Within, validated, row `amana-tapered-mdf-parallel-3175-2f`, band 0.0114–0.0209, median 0.0209 (at max) | |
| deflection gate | Within 0.008 mm, validated | Within 0.007 mm, approximate | |
| power gate | Within 0.017 / 0.773 kW | Within 0.002 / 0.867 kW | |
| peak bite (mm) | 4.20 (= one DOC step) | 2.93 | |
| crosses_standing | 10.4 % > 4.11 mm | 12.2 % > 0.56 mm, peak 2.93 | |
| plunge over 1× | 7 / 588 at 2.1× (pre-fix binary) | 0 / 45 | |
| modulator touched / median Δ / binding | 10 647 / 11 109, +64.3 %, chipload_max 70 % / kin_reach 25 % | 437 136 / 444 221, −0.2 %, chipload_max 98 % | |
| achieved vs commanded (time-weighted) | 1 538 / 1 538 | 774 / 774 | |
| utilization / feed_bound / machine_bound | 0.9995 / 0.990 / 0.010 | 1.000 / 1.000 / 0.000 | |
| feeds_provenance | emitted | emitted | |
| air measurability | degraded, blind 0.22 | degraded, blind 0.21 | |

Reading: the plywood pair takes 14 264 s against 14 245 s for the oak
pair, so the material changes almost nothing about the pair, because both
finishes are feed-bound on the same R1.5 raster at stepover 0.3. The
finish is 81.5 % of the plywood pair. The same-material ratio for B15 is
**14 264 / 3 274 = 4.36×**; the cross-material ratio was 4.35×. S20 reads
5.97× on the same base. The pair's finish peak bite (2.93 mm) equals the
oak finish's on the same rough, and its 0/45 plunge count shows the
G-BOUNDARYPLUNGE class lives in the rough, not the finish.

One reading here bears on the Suggest-vs-gate item in §3: on THIS finish
(median bite 0.19 mm) the gate's band max read 0.0209 and Suggest's 0.0210
sat exactly on it, so the modulator moved the feed by −0.2 %. On B15 (the
same tool, material, row and Suggest recipe, but a 9 mm peak bite as the
only pass) the gate's band max read 0.0145. The band the gate quotes
moves with the measured bite; Suggest's recipe does not. That is
consistent with the DOC-derate mechanism and still not verified in code.

Artifacts: `PLY_pair_A_finish.toml`, `PLY_pair_A_finish_sim.png`.

### 2.5 Instrument before/after across the rebuild — MEASURED

The GUI was reset onto the binary that carries G-DCFLAT, G-AIRDENOM (one
time base for the air percentages) and the G-BOUNDARYPLUNGE generator fix.
S20 and SP15 were reloaded from their saved TOMLs with no parameter
change and run again at 0.2 mm. Everything that is a property of the CUT
reproduces; the two things the fixes touch move, and only those.

| | S20 pre (1af25c87) | S20 post | SP15 pre | SP15 post |
|---|---:|---:|---:|---:|
| total runtime (s) | 2 389.18 | 2 389.02 | 3 806.3 | 3 799.96 |
| fed time (s) | 2 256.5 | 2 256.40 | 3 330.0 | 3 326.97 |
| moves / cutting mm / rapid mm | 44 190 / 30 024 / 1 542 | same | 24 593 / 31 309 / 18 738 | same |
| plunge over 1× (emitted) / peak | 1 / 109, 2.71× | **0 / 109, 1.00×** | 108 / 197, 3.61× | **0 / 197, 1.00×** |
| plunge over 1× in the PLANNED IR | 66 / 109 | 65 / 109 | 108 / 197 | **0 / 197** |
| `plunge_class_load` triage action | CRITICAL | absent | CRITICAL | absent |
| air % of total / % of cutting | 36.76 / 35.28 | **31.13 / 31.81** | 25.21 / 30.88 | **25.61 / 29.18** |
| air, absolute (s) | 878 (mixed-base, see below) | 744 | 960 (mixed-base) | 973 |
| crosses_standing | 24.0 % > 2.52, peak 8.54 | identical | — | — |
| chipload gate | Within, band 0.0135–0.0239, median at max | identical | Within (extrapolated), 0.0091–0.0181, median at max | identical |
| deflection / power peak | 0.007 mm / 0.015 kW | 0.007 / 0.015 | 0.020 mm / 0.008 kW | 0.020 / 0.008 |
| modulator touched / median Δ | 42 727 / −4.3 % | 41 389 / −4.3 % | 17 994 / −27.5 % | 17 994 / −27.5 % |
| achieved feed, time-weighted | 798.3 | 798.4 | 564.1 | 564.6 |
| collisions / provenance | 0 / emitted | 0 / emitted | 0 / emitted | 0 / emitted |

Three readings, in order of weight:

1. **The plunge fix is a generator fix and the two arms show its two
   faces.** On SP15 the planned (pre-modulation) IR went from 108 over-1×
   descents to zero: every one of the spiral's fast descents was a
   boundary re-entry that had kept its cut feed, and the generator now
   emits them at the plunge rate. On S20 the planned IR still carries 65
   untagged descents above the plunge rate and the modulator's geometric
   guard caps all of them; the fix removed exactly the one tagged re-entry
   the guard could not reach. Runtime moved by −0.16 s and −6.3 s.
2. **G-AIRDENOM moved the percentage, not the air.** S20's air reading
   fell from 36.8 % to 31.1 % of total with the cut byte-identical in
   every other column. The pre-fix "absolute air" figures in §2.1 were
   derived as percentage × total runtime, and that product was itself
   mixed-base (the percentage's denominator was the naive commanded-feed
   clock, the total was the modulated wall clock), so they OVER-state the
   air on every arm the modulator slowed. Post-fix, the air seconds are on
   one clock. **Air percentages and air seconds from the pre-fix arms in
   §2.1 are not comparable to the post-fix arms in §2.5 onward**; where
   the same arm exists on both sides, this table is the bridge.
3. The G-AIRDENOM effect is small where the modulator barely moved the
   feed (SP15: 25.2 % → 25.6 % of total) and large where it slowed the
   cut most (S20 −5.6 points). The 40 %/45 % bars were tuned against the
   mixed-base quantity and were not moved; on the post-fix binary a ball
   finish reads a few points lower against the same bar.

Instrument note for the record: the MCP `run_simulation` parameter is
`resolution`; a call with the key `resolution_mm` is accepted and IGNORED,
and the sim runs at whatever the GUI held (0.4 mm after a fresh load,
the previous value otherwise). One S20 rerun went out at 0.4 mm that way
(2 397.7 s, air 30.2 %) and was discarded; every figure in this document
was checked to be at `cell_mm = 0.2`.

Artifacts: `S20_postfix_sim.png`, `SP15_postfix_sim.png`.

### 2.6 Quality arms for the real-wood matrix — MEASURED (rebuilt binary)

Three more single-pass arms on the plywood copy, fresh stock each, one
enabled toolpath, `apply_feeds` speeds, modulation ON, 0.2 mm (`cell_mm`
0.2 verified on each). The operator's question is whether a smaller ball
buys reach into the valleys and what it costs in load and time.

| Arm | tool / op | cmd F / P / rpm | total s | fed s | air % total / cutting | air s | air measurability | chipload (row, band, median) | modulator median Δ / achieved | deflection (approx) | power kW | peak / median bite (×tip D) | plunge | entry_load | cusp (mm) | REACH untouched / reached_uncut (mm²) |
|---|---|---|---:|---:|---|---:|---|---|---|---:|---:|---|---|---|---:|---|
| Q1a | R1.0 taper (2 mm tip), raster s1.0, entry `none` (straight plunge) | 625 / **300** / 18 500 | **5 077.7** | 5 052.0 | 9.9 / 9.9 | 502 | measurable (cell < tip r) | Within validated, MDF parallel 3175 row, 0.0078–0.0144, at max | −14.8 % / 533 | 0.0075 | 0.005 | 8.60 / 1.24 (4.3×) | 0/1 at 300 | not fired | 0.134 | null / null (not measured on drop_cutter) |
| Q1b | same, entry `ramp` | — | CANNOT RUN | | | | | | | | | | | | | |
| Q2 | R2.0 taper (4 mm tip), raster s1.5 | 925 / 341 / 18 500 | **2 088.9** | 2 078.6 | 17.5 / 17.5 | 365 | degraded (0.17) | Within validated, MDF parallel 6000 row, 0.0126–0.0226, at max | −9.4 % / 839 | 0.0073 | 0.012 | 9.35 / 1.67 (2.3×) | 0/1 | not fired | 0.146 | null / null |
| Q3 | R1.5 taper (3 mm tip), iso-scallop h0.20 | 925 / 256 / 18 500 | **4 372.3** | 4 173.0 | 32.5 / 33.2 | 1 422 | degraded (0.10) | Within validated, hardwood scallop 3175 row, 0.0099–0.0198, at max | −20.7 % / 628 | 0.0178 | 0.010 | 8.22 / 1.27 (2.7×) | 0/298 (planned 210 over, guard-capped) | not fired | 0.20 | **0.0 / 0.0 measured** (scallop ring cascade) |
| B15 (§2.1, pre-fix) | R1.5 taper, raster s1.5 | 776 / 256 / 18 500 | 3 274.4 | 3 262 | 9.8 (mixed-base) | — | degraded (0.14) | Within validated, MDF parallel 3175 row | −30.9 % / 538 | 0.019 | 0.006 | 8.5 / — | 0/1 | not fired | 0.20 | null / null |

All four: 0 collisions, `feeds_provenance = emitted`, ≥ 99.6 % feed-bound,
every gate MODELED. Q3 has 121 rings and 88 retract trips (S15 at h0.38
had 436 and 359) and 2.4 km of rapids.

**Q1b cannot run.** `DropCutter`'s registry `dressup_policy` carries a
`strip_all_reason`, and `DressupConfig::normalize_for_op` forces
`entry_style = None` (plus no lead-in/out and no link moves) on every
drop_cutter — the phantom-diagonal rule. `set_dressup_field` and
`set_dressup_config` accept `"ramp"` and read back `"none"`. The only
entry on this op class is the straight plunge at the op's plunge rate,
which is what Q1a measured (one plunge, 1.0× of 300 mm/min).

**The 2 mm bit at full depth (the operator's question).** Q1a's peak
bite is 8.60 mm, 4.3× the tip diameter, and every gate says Within: the
chipload band was queried at 3.53 mm (`queried_diameter_mm`, the
taper's width about 7.7 mm up the 5.7° flank, near the peak bite), not
the 2 mm tip, and the deflection model
(approximate, slot engagement) divides by the engaged diameter at each
sample's DOC (the 2026-08-28 M3 denominator), so its 7 µm is a taper
figure, not a 2 mm-tip figure. Read that as: the sim trusts the shank,
and the 5.7° taper puts the shank in the cut from about 1.5 mm below
the tip. Nothing here models the tip's own bending or the band's
transfer to a 2 mm tip beyond the `D^0.61` law. Un-modulated, Suggest's
recipe (625 mm/min) was already at the gate's ceiling (0.0169 clamped to
the band, then −15 % by the modulator).

**Reach (the tier map).** `preview_tier_map` on the plywood project,
ladder R2.0 / R1.5 / R1.0, cell 0.4, tolerance 0.05, margin 0.5:

| tier | tool | map cells | owned islands | owned area mm² |
|---|---|---:|---:|---:|
| 0 | R2.0 | 190 139 | (sweeps the rest) | ≈ 30 400 |
| 1 | R1.5 | 20 801 | 3 (of 7 890 raw) | 190 |
| 2 | R1.0 | 40 061 | 17 (of 4 479 raw) | 7 027 |
| unassigned | grid margin outside the 200 × 200 model (521² − 500² = 21 441 cells) | 20 440 | | — |

Reading: the grid is 521 × 521 cells over a 208.4 mm span (viewBox
−4.2 … 204.2) and the model is 200 × 200 (`inspect_model`), so the model
footprint is 250 000 cells and the margin ring is 21 441; the 20 440
"unassigned" cells are that margin, not unreachable valleys — no
"no-tool-reaches" area exists in this ladder at 0.05 mm tolerance. On the
model the R2.0 ball owns 190 139 / 250 000 = **76 %** of the cells; the
R1.5 adds only 190 mm² that survives island filtering (its 7 890 raw
islands are slivers); the R1.0 owns 7 027 mm² in 17 islands, and that
owned area includes the 2 mm overlap band grown into R2.0 territory (raw
40 061 cells = 6 410 mm²). The measured REACH column agrees where it
exists: Q3's ring cascade reports 0.0 mm² untouched at its own tolerance
because a scallop's rings are the reach test, while the raster arms
report `null` (not measured). SVG: `tier_map_r10_r15_r20.svg`.

**Reach map (P5), live on Q2, and what it says now.** The per-tool reach
map landed the same day (`reach_map(index)`, GUI overlay, and
`screenshot_toolpath(reach_overlay)`); its first live reading on Q2 was
68.2 % unreachable at a 0.05 mm bar, which disagreed 3× with the tier
map's 24 %. That was two things, neither of them slope compensation: the
grid (cell = tip radius / 2, 0.645 mm, against a 0.05 mm bar) over-read
by about 13 points, and the two instruments answer different questions
(the tier map is a tool-vs-tool residual — what R2.0 misses that a finer
tool would catch; the reach map is absolute). Fixed as P5.1
(`be96933c`): unresolved cells are reported separately from unreachable,
the default bar is the operation's own cusp (0.146 for Q2), and the
reply leads with a `grid_note` (cell, floor, bar) to read before the
percentage. An independent rasterisation of the terrain closed with the
R2.0 ball (`reach_truth_rasteriser.py`, in this directory) gives the
truth to quote:

| tolerance (mm) | truth, planar whole-board (rasteriser) | truth on the map's own base (3D surface area, rim-eroded) | reach_map, 0.75 grid, P5.1 |
|---:|---:|---:|---|
| 0.05 | 55.0 % | 58.6 % | 59.05 % (+ 11.0 % unresolved) |
| 0.146 (Q2's cusp, the default bar) | 39.5 % | 42.1 % | 51.1 % (+ 2.8 % unresolved) |
| 0.30 | 25.2 % | 26.8 % | 36.4 % |

The second look on the P5.1 build read the map 12 points above the
planar truth at every bar, which is not what a grid floor does. It was
measured down on one mask: 3.5 points are the AREA BASE (the map weights
by true 3D surface area over the rim-eroded population, mean sec θ 1.34,
and the steep cells are the unreachable ones — the middle column is the
truth on that base), the tapered shank is +0.01 points (refuted: a 3°
cone needs a neighbour 3.9 mm higher at 2.1 mm away), and the remaining
~9 points are the documented discretisation — an additive gap inflation
of about +0.054 mm at the 0.75 mm cell, which on a terrain whose gap
density is nearly flat over 0.05–0.30 mm shows up as a near-constant
PERCENTAGE offset. Consequence, applied in the follow-up commit: the
map's unreachable figure is an UPPER estimate (truth at or below it),
the "lower bound" wording is gone, and the area basis is printed in the
legend; at the tight bar the map is within 0.5 points of the same-base
truth. The reach reply's first line is the grid note (cell, floor,
bar) — read it before the percentage.

So the R2.0 raster leaves about 42 % of this terrain's surface (39.5 %
of its plan area) more than its own cusp away from the true surface —
the valley floors and steep flanks the 4 mm ball cannot enter — and
about a quarter of it more than 0.3 mm away. The gap histogram says how
deep: 83 % of the measured cells sit under 0.56 mm, 13 % between 0.56
and 1.1 mm, 3 % between 1.1 and 1.7 mm, under 1 % deeper, worst 4.46 mm.
That is the price of the 35-minute single pass, stated as a measured
area rather than a picture. Screens: `Q2_reach_overlay_gui.png`,
`Q2_reach_overlay_path.png` (P5 build), `Q2_reach_overlay_gui_p51.png`,
`Q2_reach_overlay_gui_p51_wide.png`, `Q2_reach_overlay_path_p51.png`
(P5.1 build, moves dimmed under the shading).

Ranked on reach, time and cusp together:

1. **Q2 — R2.0 raster s1.5: 2 089 s, cusp 0.146, reaches ~76 %.** The
   time-at-quality winner as expected: 36 % faster than B15 at a smaller
   cusp, every gate at or under B15's, air 17.5 %.
2. **B15 — R1.5 raster s1.5: 3 274 s, cusp 0.20**; the R1.5 buys almost
   no reach over the R2.0 (190 mm²) on this terrain.
3. **Q3 — R1.5 iso-scallop h0.20: 4 372 s, cusp 0.20**, contour-following
   texture (close-up), the only arm with a measured reach figure. 34 %
   slower than the same tool on a raster.
4. **Q1a — R1.0 raster s1.0: 5 078 s, cusp 0.134, reaches the 17 R1.0
   islands (7 027 mm² with the overlap band, 6 410 mm² raw)** that no
   bigger ball enters. It is the reach tool, not the finish tool: 2.4×
   Q2's time for about 16 % more area.

**The pairing the tier map points at was RUN, and it is not a win.** The
operator asked to see it, so the ladder was built with
`plan_multitool_finishing` (tools [4, 2], cusp 0.14 → stepovers 1.470 /
1.021, cell 0.4, tolerance 0.05, `apply_feeds` speeds, tier 0 switched to
fresh stock because the planner emits it as `from_remaining_stock`),
`generate_all` fixpoint 0.2 (2 rounds, 1 simulation) and a final 0.2 mm
simulation (`cell_mm` 0.2 verified):

| | tier 0, R2.0 unified_finish | tier 1, R1.0 unified_finish | ladder |
|---|---:|---:|---:|
| fed time (s) | 3 239 | 6 204 | |
| total runtime (s) | | | **10 710** |
| moves / cutting mm / rapid mm | 56 329 / 46 015 / 4 203 | 108 983 / 66 129 / 24 141 | |
| band mix (moves) | scallop 40 876, raster 14 415, waterline 1 030 | scallop 79 218, raster 24 608, waterline 5 149 | |
| retract trips | 123 | 1 104 | |
| chipload | Within validated, hardwood scallop 6000 row, at max | Within validated, hardwood scallop 3175 row, at max | |
| deflection / power | 0.007 mm / 0.012 kW | 0.006 mm / 0.004 kW | |
| plunge over 1× | 0 / 147 | 0 / 1 316 (planned 1 266 over, guard-capped) | |
| air % of total | 31.0 (degraded 0.16) | NOT MEASURABLE (blind 0.50) | 56.1 |
| collisions / provenance | 0 / emitted | 0 / emitted | 0 |

10 710 s is 5.1× Q2 alone and 3.3× B15. The cause is the operation, not
the tools: the planner emits `unified_finish`, whose mid-steep band is a
scallop at the ladder's cusp, and on this terrain that band dominates
both tiers (tier 0 spends 40 876 of 56 329 moves in it). The R1.0 tier,
with its islands grown by the 2 mm overlap band, cuts 66 km — more than
the whole-surface Q1a raster's 45 km — with 1 104 retract trips. The
RASTER pairing the ranking implies (drop_cutter R2.0 whole-surface plus
drop_cutter R1.0 confined to the planner's island boundary) is NOT
expressible through the planner or the MCP today: `tier_strategies`
has no raster value and `set_boundary_config` does not accept
`planned_tier_regions`. The core raster does honour the island set
(`toolpath.rs:607`, corrected 2026-09-09; a hand-edited project file
carries it — see `planning/island_clip_2026-09-09/SPEC.md`, T5). So
the honest recommendation as of this section is the single R2.0 raster
(Q2), accepting the valley floors it does not reach, or B15. Artifacts: `LADDER_r20_r10islands.toml`, `_sim.png`,
`_gui.png` (the live simulation view at the end of the ladder).

Two trims the operator asked for were then measured on the same ladder
(all gates Within on both tiers, 0/0 collisions, emitted, `cell_mm` 0.2):

| ladder variant | tier 0 fed s | tier 1 fed s | total s | tier 1 retracts | tier 1 bands (moves) |
|---|---:|---:|---:|---:|---|
| base: threshold 75°, overlap 2.0 both dials | 3 239 | 6 204 | 10 710 | 1 104 | scallop 79 218 / raster 24 608 / waterline 5 149 |
| v1: tier 1 waterline threshold → 90° (stored as 89°) | 3 239 | 5 690 | 9 861 | 717 | scallop 84 855 / raster 24 543 / waterline none |
| v2: v1 + island overlap 1.25, op overlap 1.5 (tier 0) / 1.25 (tier 1), both above their stepovers | 3 080 | 4 706 | 9 158 | 1 194 | scallop 59 785 / raster 17 859 / waterline none |

Raising the tier-1 threshold removes the waterline band and its 1.02 mm
terraces on the steepest walls, hands that strip to the scallop band, and
saves 514 s of tier-1 time plus 7.4 km of rapids. Trimming both overlaps
to just above the stepover saves a further 703 s (159 on tier 0, 544 on
tier 1). The two trims together take 14.5 % off the ladder; it is still
4.4× Q2 alone and 2.8× B15, because the mid-steep scallop band is
untouched by either dial. Tier 1's retract count went UP with the
narrower overlap (717 → 1 194, 21 regions against 16): smaller islands
fragment more. Artifacts: `LADDER_v1_wl90.toml`,
`LADDER_v2_wl90_ov125.toml` + `_sim.png`.

Instrument note: tier 0's `modulation_summary` reads `moves_touched: 0`
alongside `median_feed_delta_pct −5.3 %` and an achieved feed of 852
against 925 commanded; the two fields cannot both be right and the
achieved feed says the modulator ran. Reproduced twice: the FIRST
`run_simulation` after a `generate_all` fixpoint loop reports 0 touched
on tier 0; a second simulation of the same unchanged toolpath reports
53 877. Ledger item for the orchestrator.

Close-ups (TOP panel of the 4800-px render, autocontrast; height map,
not lit): `Q1a_closeup_*`, `Q2_closeup_*`, `Q3_closeup_*` and
`B15v2_closeup_*` (`_surface_top_ac`, `_valley_top_ac`,
`_surface_frontleft_ac`). The `_valley_top_ac` pair Q3 vs B15v2 shows
the scallop's contour-following rings against the raster's 1.5 mm
blocks in the same valley window.

Artifacts: `Q1a_r10_s10_plunge.toml` + `_sim.png`, `Q2_r20_s15.toml` +
`_sim.png`, `Q3_scallop_r15_h020.toml` + `_sim.png` + `_path.png`,
`tier_map_r10_r15_r20.svg`, the close-up crops.

### 2.7 Removing the rest — trials after the R2.0 pass (2026-09-09)

The operator judged the R2.0 single pass unfinished (§2.6: ~42 % of the
surface beyond its cusp) and asked for trial and error toward removing
the rest, with the planned regions cut by a raster or a scallop rather
than the unified finish. Three pairs, each Q2 (R2.0 raster s1.5, fresh)
followed by a second R1.0 pass on the remaining stock, plywood copy,
0.2 mm (`cell_mm` 0.2 verified), all three load gates MODELED on every
pass:

| trial | second pass | pair total s | 2nd pass fed s | 2nd pass retracts / rapids km | achieved feed (cmd 782 / 625) | air % 2nd | safety / entry |
|---|---|---:|---:|---|---:|---:|---|
| T1 | R1.0 raster s1.0, whole board, plunge 300, entry `none` | **6 478** | 4 317 | 76 / 1.0 | 623 / 625 | 56.3 (degraded) | 0/0, entry_load not fired |
| T2 | planner tier 1 ISO-scallop R1.0 on its islands (cusp 0.14, overlap 1.25) | 25 580 | 18 519 | 2 343 / 164.9 | 298 / 782 | 71.6 (degraded) | **1 rapid collision** at move 68538 (109.1, 13.1, 3.31); **entry_load CRITICAL** 17 538 samples > 0.22 mm, peak 1.44 |
| T3 | planner tier 1 contour SCALLOP R1.0 on its islands | 11 105 | 7 013 | 989 / 61.6 | 374 / 782 | 65.3 (degraded) | 0/0; **entry_load CRITICAL** 7 756 samples > 0.23 mm, peak 1.79 |

Readings. T1 is the only runnable two-tool result that passes every
gate with no safety or entry finding: 1 h 48 min, 3.1× Q2 alone, 2.0×
B15, and 29 % under the trimmed unified ladder (§2.6), with a 0.134 mm
cusp everywhere the 2 mm ball fits. On cut stock the R1.0 raster runs
at its commanded feed (the modulator's median move is −0.3 %, against
−15 % on fresh stock), which is why its 4 317 s is under the 5 052 s of
the same pass on fresh stock. T2 answers the operator's iso-scallop
question in the negative as run: 1 624 ring fragments with 2 343
retracts and 165 km of rapids, a rapid through stock, and buried
re-entries. T3 shows the entry finding is not the iso field's: the
contour scallop on the same islands halves the fragmentation and drops
the rapid collision but buries its re-entries the same way (peak
1.79 mm). Both T2 and T3 are with a fix agent (`isoclip-entry-safety`);
their TOMLs are the reproductions. The first reading of this table
blamed the post-generation island clip for the fragmentation. That was
wrong (corrected 2026-09-09): the scallop generates one ring set per
island (`scallop.rs:2238`), so the rings never leave their islands. The
fragmentation comes from the planner's own `continuous: true` on every
scallop tier: under that flag the spiral connector falls back to
retract / rapid / replunge on any hop longer than the ring spacing
(`scallop.rs:2331`), and the intra-pass relink that would join the
rings on the surface is skipped (`scallop.rs:2543`). T3 reads 2.04
retracts per ring, T2 1.44. The one-dial test (`continuous = false`,
hookup 3.0 then 6.0) and a raster-per-island run on the same islands
(T5) are specified in `planning/island_clip_2026-09-09/SPEC.md` and
wait for the GUI. Recommendation as of this section, until those read:
Q2 then the R1.0 raster over the whole board on remaining stock (T1),
and accept that the second pass air-cuts the ground the R2.0 already
finished.

Artifacts: `T1_r20_then_r10_rest.toml` + `_sim.png`,
`T2_r20_raster_then_r10_iso_islands.toml` + `_sim.png`,
`T3_r20_raster_then_r10_scallop_islands.toml` + `_sim.png`.

### 2.8 The cusp the passes actually leave — raster vs iso-scallop (2026-09-09)

Every raster cusp quoted above is the flat-ground law
h = r − sqrt(r² − (s/2)²). On a flank the raster's passes are spaced in
plan, so the spacing measured on the surface grows with the slope, while
an iso-scallop holds its cusp on the surface by construction. The
operator pushed for scallop on exactly that ground, and the time ranking
in §2.1 never measured it. Measured now: the simulated stock top (0.2 mm
dexel, one column per 0.2 mm cell, 980 100 columns inside 1–199 mm) minus
the terrain height at the same XY, binned by the terrain's slope from the
STL gradient (`cusp_measure.py`, this directory, on the `stockVerts` of
the simulation's HTML export). Same tool (R1.5 tapered ball), same
nominal cusp (0.20 mm), fresh plywood stock, both arms reproduced to the
millisecond (B15 3 274.43 s, Q3 4 372.33 s).

| slope band (share of board) | B15 raster s1.5: p50 / p90 / > 0.3 mm | Q3 iso-scallop h0.20: p50 / p90 / > 0.3 mm |
|---|---|---|
| 0–25° (34 %) | 0.130 / 0.762 / 26.5 % | 0.093 / 0.507 / 17.2 % |
| 25–45° (28 %) | 0.283 / 0.817 / 47.6 % | 0.143 / 0.545 / 25.0 % |
| 45–60° (31 %) | 0.380 / 0.868 / 60.6 % | 0.178 / 0.564 / 29.5 % |
| 60–90° (7 %) | 0.550 / 1.215 / 75.7 % | 0.303 / 0.812 / 50.3 % |
| whole board | 0.269 / 0.870 / — | 0.141 / 0.573 / — |

Column by column over the 980 100 common cells: the raster leaves a
median 0.105 mm more than the scallop, the scallop is the better surface
on 70.7 % of the board and the raster on 19.2 %. Two readings:

1. **The raster's cusp is not its flat-law number.** Its median residual
   is 0.13 mm on the flats and 0.55 mm on the steep flanks — the flat law
   said 0.20 everywhere. The iso-scallop rises too (0.09 → 0.30) but by
   less, and stays under the raster in every band. The tails (p90 and
   beyond) are reach — valley floors the R1.5 ball cannot enter — and are
   the same for both patterns, which is why the reach map, not the cusp,
   governs them.
2. **The §2.1 time ranking compared unequal surfaces.** "Q3 is 34 %
   slower than B15" holds at equal NOMINAL cusp only. Put on ONE delivered
   bar — the share of the board left more than 0.3 mm above the model,
   with the whole-board median beside it — the three R1.5 single passes
   read:

   | R1.5 single pass | runtime s | board > 0.3 mm | p50 residual mm | p90 mm |
   |---|---:|---:|---:|---:|
   | B15 raster s1.5 | 3 274 | 46.4 % | 0.269 | 0.870 |
   | B10 raster s1.0 | 4 788 | 31.5 % | 0.163 | 0.687 |
   | Q3 iso-scallop h0.20 | 4 372 | 25.5 % | 0.141 | 0.573 |

   The iso-scallop is both faster and better than the raster at half its
   stepover, on every statistic and in every slope band (B10 by band:
   p50 0.058 / 0.171 / 0.228 / 0.416 mm against Q3's 0.093 / 0.143 /
   0.178 / 0.303 — the raster wins only on the flats). On the R1.5 tool
   the iso-scallop DOMINATES the raster on this terrain: the raster's
   win in §2.1 was a win at a coarser finish. The R2.0 pair (Q2 raster
   s1.5 vs S20 iso-scallop h0.27) has not had its surfaces measured. This does not change the
   two-tool result (§2.7): the second pass there is a whole-board raster
   because the island-bounded scallops as planned fragment (planner
   `continuous` default, §2.7) and the planner has no raster tier; the
   whole-board R1.0 raster on rest stock was the only pairing that passed.

Caveats: the residual is quantised to the 0.2 mm dexel cell, so the
0.06–0.13 flat-band medians are at the instrument's floor; the scallop
arm shows a deeper overcut minimum (−0.71 mm against the raster's
−0.37 mm) that has not been traced to a move; and both arms are measured
against the model, not against each other's stock.

## 3. Verdict on the hypothesis, and a ranked recommendation

The hypothesis passes, on the ball tools, on every bar the orchestrator set:

- **drop_cutter raster, R1.5 tapered ball, stepover 1.5 (B15): 3 274 s as
  the WHOLE job** — chipload, deflection and power all MODELED and Within,
  0 collisions, air 9.8 % against a 45 % bar, `feeds_provenance = emitted`,
  and 4.36 × faster than the PLYWOOD A rough + R1.5 finish pair (14 264 s,
  §2.4, same material), 4.35 × faster than the oak pair (14 245 s) and
  3.9 × faster than Arm G + finish (12 739 s, oak). Cusp 0.20 mm. No triage
  action of any kind. This is the recommendation for "rough + finish on
  plywood terrain": there is no rough; one ball raster does the job.
- **iso-scallop, R2.0 tapered ball, h 0.27 (S20): 2 389 s** — the fastest
  arm (and see §2.8: at matched DELIVERED surface the iso-scallop beats
  the raster on this terrain; the raster rankings here are at a coarser
  finish than their flat-law cusp states), chipload Within on an
  un-extrapolated row, air 36.8 %, one 2.7 ×
  plunge that is the G-BOUNDARYPLUNGE class on a pre-fix binary (rerun
  scheduled). Cusp 0.27 mm. The alternative when the bigger ball is in the
  holder.
- B20 (s 2.0, 2 438 s, cusp 0.38) and B10 (s 1.0, 4 788 s, cusp 0.086) bracket
  B15: time scales almost linearly with 1/stepover on the raster, and the
  operator picks the cusp.

Caveat that belongs in this paragraph, not in a footnote: **every R1.5
arm's chipload band came from `amana-tapered-mdf-parallel-3175-2f`**. The
LUT has no tapered-ball row for plywood at all; the resolver matched the
MDF row through the wood-category rule (hardness scale ×0.957, diameter
×1.10–1.12, not flagged as extrapolated because both raw ratios are inside
±40 %). The gate is MODELED and it is honest about its row, but the band it
judged against is an MDF band transferred by the repo's own scaling laws,
not a plywood measurement. S20's row (`amana-tapered-hardwood-scallop-6000-2f`)
is a hardwood row on the same footing. A plywood tapered-ball row is the
single most valuable LUT addition this study points at.

What the modulator did, measured: on every ball arm its binding constraint
was the chipload band MAXIMUM (89–100 % of touched moves), it cut the
commanded feed by a median 22–31 % on the R1.5 arms and 4 % on the R2.0
arm, and the gate's median advance per tooth sits exactly at the band max.
Deflection never exceeded 0.020 mm against a 0.05 mm validated bar; power
never exceeded 0.015 kW against 0.87. So the "modulation as the safety net
for variable engagement" premise holds on these arms — but note what it
was regulating: the CHIPLOAD band, not deflection or power, which were
never close. Peak bites of 8.5–9.8 mm (up to 3.2 × the 3 mm tip diameter)
went through at 0.019 mm deflection because the engaged diameter on a
tapered ball at that depth is the shank.

One more thing the data shows, and it is the premise proving itself
against Suggest's own recipe: **Suggest and the gate quoted different
chipload ceilings on the R1.5 tapered ball.** `apply_feeds` wrote F776 at
18 500 rpm on every R1.5 drop_cutter arm, which is 0.0210 mm/tooth, and
reported it as clamped to the matched band ceiling. The gate's band on the
same tool and material read 0.0079–0.0145, and the modulator then cut the
feed by a median 31 % to land on 0.0145. The ratio is 1.45×. The likely
mechanism is the DOC derate: Suggest gets no axial hint from `drop_cutter`
(`feeds/INTEGRATION.md`, "none") and derates at a default depth, while
the gate derates at the measured 9 mm bite. The mechanism is not verified
here (no code was run); §2.4 adds one supporting reading — the same
tool, row and recipe on a 0.19 mm median bite gave a gate band max of
0.0209, equal to Suggest's number. Un-modulated, these arms would have run 45 % over
the gate's ceiling on Suggest's numbers alone, so on this fixture the
modulator is not a safety net for engagement variation only, it is the
correction for Suggest's blind DOC. Handed to the ledger as a resolver-pair
observation (the F-LUT2 class). The flat drop_cutter shows the SECOND
face of the same pair, in the opposite direction: Suggest's post-derate
chipload on the plywood pocket row read 0.0244 (the "clamped to rubbing
floor" message on every G-fine add), while the gate's ceiling on the same
row at the same 9.3 mm bite read 0.0471 — 1.93× the other way. Two
resolvers, two derate magnitudes, one row; that is the class the
"Suggest must mirror the gate's piecewise DOC derate" rule describes.

Where the modulator could not rescue a cut: nowhere on the ball arms. Where
the pattern, not the modulator, decided the result: S15 (fragmented
iso-field on the R1.5, 81 % air), RA06 (radial on a square part, 69 % air),
WL15 (contour fragments with slow fed links, 18 368 s), SP15 (square clip
of a spiral, 18.7 km of rapids).

Ranked, whole job on plywood, this fixture (the air columns of the
pre-fix arms are mixed-base, §2.5; any ranking that turns on air is to be
re-read on post-fix runs):

0. Q2 — R2.0 raster s1.5 (§2.6, post-fix binary) — 2 089 s, cusp 0.146,
   every gate modeled Within, air 17.5 %; the fastest arm at or under
   B15's cusp, and the widest reach of the balls tried. The R2.0 + R1.0
   ladder the tier map suggests was run and costs 10 710 s as the planner
   emits it (§2.6), and the raster form of that pair cannot be built
   today — so Q2 stands alone, valley floors accepted, or B15. When the
   floors must go: Q2 then the R1.0 raster on remaining stock (T1, §2.7,
   6 478 s) is the measured two-tool answer.
1. B15 — 3 274 s, cusp 0.20, passes every bar as written.
2. S20 — 2 389 s, cusp 0.27, passes every bar once the plunge fix is on the
   binary (rerun pending).
3. B20 — 2 438 s, cusp 0.38, passes every bar; for a sanded finish.
4. B10 — 4 788 s, cusp 0.086; when the cusp must be under 0.1 mm.
5. Arm G flat raster + R1.5 finish (oak) — 12 739 s; the flat rough saves
   10 % of the pair but the finish is 90 % of it.
6. A + finish (plywood, §2.4) — 14 264 s; every gate MODELED Within, 0
   collisions, but both ops over the air bar (43.8 % / 45.1 %).
7. A + finish (oak) — 14 245 s.

Against the adaptive3d family the deep-DOC single level (E-ply, 1 632 s
rough only) confirms the roughing lever from 2026-09-07 (−34 % on plywood),
but a rough at any DOC still needs the 11 630 s finish (§2.4, measured). The hypothesis is
answered by removing the finish, not by deepening the rough.

## 4. Not run / cannot run without code

- **Flat-tool arms**: the modulated Arm G rerun and G-fine s 1.0 / 1.5 /
  2.0 are MEASURED on the rebuilt binary (§1.2, §1.2a). The DC s 2 / 4
  roughs from the original brief were not run; GF20 (s 2.0) and Arm G
  (s 3.0) bracket them.
- **Before/after plunge reruns**: S20 and SP15 MEASURED on the rebuilt
  binary (§2.5). S15, RA06, WL15, A-ply and E-ply were not rerun; their
  plunge findings are the same two classes and the §2.5 pair is the
  instrument's before/after.
- **Rough + finish pair on plywood**: MEASURED for A + R1.5 finish (§2.4,
  14 264 s, finish 11 630 s fed). The C2 and E pairs were not run; their
  finish would be the same op on a slightly different remaining stock, and
  the oak §1.3 pair totals differ by under 5 % across the A, BE and C2E
  roughs.
- **A DOC cap on drop_cutter**: CANNOT RUN WITHOUT CODE (§1.4).
- **Ramp entry on drop_cutter (Q1b)**: CANNOT RUN — the op's dressup
  policy strips every entry style to a straight plunge (§2.6).
- **Flat tool on waterline / radial / horizontal / steep_shallow /
  ramp_finish**: CANNOT be gated without the same routing fix widened
  (§1.1 follow-up).
- **Waterline with Auto heights**: generates zero moves (G-WATERLINEAUTO on
  the orchestrator's ledger); pinned heights are the workaround.
- **Waterline chipload on a tapered ball**: `no_vendor_data` although
  drop_cutter matched a row with the same tool; and 13 700 of its 16 867
  fed seconds are link-class motion at ~130 mm/min emitted by the
  generator (G-WATERLINELINK). Not diagnosed further.
- **`entry_load`**: absent on every arm; not distinguishable from "not
  measured" on this wire.
- **Lit close-ups of the cusps**: not obtainable through the MCP surface
  (§2.3).
- **A plywood tapered-ball LUT row**: does not exist; every ball verdict is
  on a transferred MDF or hardwood row (§3).

## 5. Artifacts

`planning/deep_doc_modulation_2026-09-08/`: `wanaka200_plywood.toml`
(fixture copy), one `<arm>.toml` + `<arm>_sim.png` + `<arm>_path.png` per
arm (S15, S20, B10, B15, B20, SP15, RA06, WL15, A_ply_adaptive3d,
C2_ply_adaptive3d_by_area, E_ply_adaptive3d_dpp546), the six close-up
crops, `armG_finish_pair.toml` + `armG_finish_pair_sim.png`,
`PLY_pair_A_finish.toml` + `PLY_pair_A_finish_sim.png`; post-rebuild:
`S20_postfix_sim.png`, `SP15_postfix_sim.png`, `armG_modulated_oak.toml`
+ `_sim.png`, `GF10/GF15/GF20_flat_s*.toml` + `_sim.png`,
`Q1a_r10_s10_plunge.toml` + `_sim.png`, `Q2_r20_s15.toml` + `_sim.png`,
`Q3_scallop_r15_h020.toml` + `_sim.png` + `_path.png`,
`tier_map_r10_r15_r20.svg`, `Q1a/Q2/Q3/B15v2_closeup_*.png`,
`LADDER_r20_r10islands.toml` + `_sim.png` + `_gui.png`. Uncommitted
code: `crates/rs_cam_core/src/feeds/vendor_normalize.rs`,
`crates/rs_cam_core/src/feeds/INTEGRATION.md`,
`crates/rs_cam_core/src/tool_load/optimize/outcome.rs`,
`crates/rs_cam_core/tests/lut_resolver_census_a6.rs`,
`crates/rs_cam_core/tests/drop_cutter_flat_roughing_row_g_dcflat.rs`
(the orchestrator's agent runs the targeted tests and the build).
