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

### 1.2 Caveat 2 — modulated rerun `[after rebuild]`

Caveats 1 and 2 are one defect: with no chipload band the modulator has no
target, so a rerun on this binary reads the same 750 mm/min. The rerun
(fresh stock, plywood and oak, `ConstrainedMax` 1.0) is scheduled for the
rebuilt binary, together with the flat-tool arms of §2 (G-fine, stepover
1.0 / 1.5 / 2.0) and the before/after reruns of every arm that showed
`plunge_class_load` here (S20, SP15, S15, RA06, WL15, A-ply, E-ply).

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
`load_project` of that copy, all fixture toolpaths disabled, ONE new
toolpath on the front setup, `apply_feeds(scope = speeds)` so the feed,
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

| Arm | Op / tool / geometry | Time s | Air s (tot % / cut %) | Coll. | Chipload | Deflection mm | Peak bite mm | Achieved / cmd mm/min | Retracts | Cusp proxy | Verdict |
|---|---|---:|---:|---|---|---:|---:|---:|---:|---|---|
| A-ply | adaptive3d rough, 6 mm flat, s1.2, DPP 4.2 (2 levels) — ROUGH ONLY | 2 475 | 1 083 (43.8 / 38.1) | 0/0 | MODELED Within (pocket row, at max) | 0.008 | 4.20 (0.70 D) | 1 538 / 1 100 (+64 %) | 471 | leaves 0.5 axial for a finish | reference; plunge 7/588 at 2.1× (pre-fix) |
| S15 | iso-scallop R1.5 taper, h 0.38 (s≈2.0) | 8 001 | 6 481 (81.0 / 63.9) | 0/0 | MODELED Within (extrap. hardwood scallop 3175 row, at max) | 0.018 | 8.54 | 405 / 925 (−22 %) | 359 | 0.38 ball cusp | FAIL air 81 % > 45; plunge 3/706 at 3.6× (pre-fix) |
| S20 | iso-scallop R2.0 taper, h 0.27 (s≈2.0) | **2 389** | 878 (36.8 / 35.3) | 0/0 | MODELED Within (hardwood scallop 6000 row, no extrap., at max) | 0.007 | 8.54 | 798 / 925 (−4 %) | 45 | 0.27 ball cusp | PASSES all bars except 1/109 plunge at 2.7× (pre-fix) — rerun |
| B15 | drop_cutter raster R1.5 taper, s 1.5 | **3 274** | 321 (9.8 / 14.1) | 0/0 | MODELED Within (MDF parallel 3175 row, at max) | 0.019 | 9.61 | 538 / 776 (−31 %) | 2 | 0.20 ball cusp | **PASSES every bar**; no triage actions at all |
| B10 | drop_cutter raster R1.5 taper, s 1.0 | 4 788 | 762 (15.9 / 22.2) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.018 | 9.05 | 558 / 776 (−28 %) | 2 | 0.086 ball cusp | PASSES every bar |
| SP15 | spiral_finish R1.5 taper, s 1.5 | 3 806 | 960 (25.2 / 30.9) | 0/0 | MODELED Within (extrap. scallop row, at max) | 0.020 | 9.64 | 564 / 925 (−27 %) | 198 | 0.20 ball cusp | plunge 108/197 at 3.6× (pre-fix) — rerun; 18.7 km of rapids |
| RA06 | radial_finish R1.5 taper, 0.6° (1.5 mm at the rim) | 8 718 | 6 039 (69.3 / 57.5) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.018 | 8.62 | 452 / 776 (−26 %) | 539 | 0.20 at rim, denser inward | FAIL air 69 %; plunge 178/536 at 3.0×; wrong pattern for a square part |
| WL15 | waterline R1.5 taper, z_step 1.5 (heights pinned 7 / −3) | 18 368 | 7 081 (38.6 / 41.9) | 0/0 | **UNMODELED** no_vendor_data | 0.019 | 8.37 | 192 / 954 | 1 327 | 1.5 mm lateral at 45°, ridges taller than the ball on shallow slopes | FAIL: unmodeled gate, plunge 498/1480 at 3.7×, 13 700 s of the fed time is link motion at ~130 mm/min |
| B20 | drop_cutter raster R1.5 taper, s 2.0 | 2 438 | 163 (6.7 / 9.8) | 0/0 | MODELED Within (MDF parallel row, at max) | 0.019 | 9.75 | 532 / 776 (−31 %) | 2 | 0.38 ball cusp | PASSES every bar; no triage actions |
| C2-ply | adaptive3d rough, by_area, else as A-ply — ROUGH ONLY | 2 277 | 976 (42.9 / 36.4) | 0/0 | MODELED Within (pocket row, at max) | 0.008 | 4.20 (0.70 D) | 1 544 / 1 100 (+64 %) | 433 | leaves 0.5 axial | reference; ZERO over-limit plunges emitted |
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

## 3. Verdict on the hypothesis, and a ranked recommendation

The hypothesis passes, on the ball tools, on every bar the orchestrator set:

- **drop_cutter raster, R1.5 tapered ball, stepover 1.5 (B15): 3 274 s as
  the WHOLE job** — chipload, deflection and power all MODELED and Within,
  0 collisions, air 9.8 % against a 45 % bar, `feeds_provenance = emitted`,
  and 4.35 × faster than the oak A rough + R1.5 finish pair (14 245 s) and
  3.9 × faster than Arm G + finish (12 739 s). Cusp 0.20 mm. No triage
  action of any kind. This is the recommendation for "rough + finish on
  plywood terrain": there is no rough; one ball raster does the job.
- **iso-scallop, R2.0 tapered ball, h 0.27 (S20): 2 389 s** — the fastest
  arm, chipload Within on an un-extrapolated row, air 36.8 %, one 2.7 ×
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

Where the modulator could not rescue a cut: nowhere on the ball arms. Where
the pattern, not the modulator, decided the result: S15 (fragmented
iso-field on the R1.5, 81 % air), RA06 (radial on a square part, 69 % air),
WL15 (contour fragments with slow fed links, 18 368 s), SP15 (square clip
of a spiral, 18.7 km of rapids).

Ranked, whole job on plywood, this fixture:

1. B15 — 3 274 s, cusp 0.20, passes every bar as written.
2. S20 — 2 389 s, cusp 0.27, passes every bar once the plunge fix is on the
   binary (rerun pending).
3. B20 — 2 438 s, cusp 0.38, passes every bar; for a sanded finish.
4. B10 — 4 788 s, cusp 0.086; when the cusp must be under 0.1 mm.
5. Arm G flat raster + R1.5 finish (oak) — 12 739 s; the flat rough saves
   10 % of the pair but the finish is 90 % of it.
6. A + finish (oak) — 14 245 s.

Against the adaptive3d family the deep-DOC single level (E-ply, 1 632 s
rough only) confirms the roughing lever from 2026-09-07 (−34 % on plywood),
but a rough at any DOC still needs the 11 400 s finish. The hypothesis is
answered by removing the finish, not by deepening the rough.

## 4. Not run / cannot run without code

- **Flat-tool arms** (G-fine s 1.0 / 1.5 / 2.0 as the only pass; DC s 2 / 3 /
  4 as roughs; the modulated Arm G rerun): NOT RUN on this binary — the
  chipload gate is unmodeled and the modulator has no target until the
  G-DCFLAT build lands (§1.1). Scheduled after the rebuild and MCP reset.
- **Before/after plunge reruns** (S20, SP15, S15, RA06, WL15, A-ply, E-ply):
  NOT RUN; the fix is `301f2cbc`, the binary is `1af25c87`.
- **Rough + finish pair on plywood** for A / C2 / E: NOT RUN (the oak pairs
  in §1.3 carry that comparison; the plywood finish would be ~11 400 s
  again).
- **A DOC cap on drop_cutter**: CANNOT RUN WITHOUT CODE (§1.4).
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
crops, `armG_finish_pair.toml` + `armG_finish_pair_sim.png`. Uncommitted
code: `crates/rs_cam_core/src/feeds/vendor_normalize.rs`,
`crates/rs_cam_core/src/feeds/INTEGRATION.md`,
`crates/rs_cam_core/src/tool_load/optimize/outcome.rs`,
`crates/rs_cam_core/tests/lut_resolver_census_a6.rs`,
`crates/rs_cam_core/tests/drop_cutter_flat_roughing_row_g_dcflat.rs`
(the orchestrator's agent runs the targeted tests and the build).
