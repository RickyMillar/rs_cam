# M1 — exposure of U3 (engagement normalised by the SHANK envelope radius)

> Phase M, task M1 of `PLAN.md`: *"On a real wanaka trace: what fraction of
> samples falls under the 0.02 filter, and how much `air_cut_pct` is
> misattributed? Analytic ratios are known; the population is not."*
>
> **Headline.** The population is large (48.1 % of all cutting samples, 66.3 %
> of cutting time) — but **99.75 % of it is a hard `radial_woc_fraction ==
> 0.0`**, and no denominator change can lift a zero. The correction moves
> `air_cut_pct_of_total_runtime` by **−0.26 pp** (60.617 % → 60.357 %). What
> U3 *does* move, hugely, is the **magnitude** of every engagement reading that
> already clears the filter: the taper ops' time-weighted mean engagement rises
> **3.69×** (op 7) and **5.74×** (op 8). M2 should plan for a magnitude shift,
> not a population shift.

---

## 1. Input provenance

| | |
|---|---|
| Trace | `…/scratchpad/falsify_s3/simulation.json`, 7,706,484,634 bytes, mtime 2026-08-28 15:56 |
| Project | `wanaka200` (`planning/airrun_2026-08-19/wanaka200.toml`) |
| Post-S3 | yes — same directory as `emitted.nc` / `setup1.nc` / `setup2.nc` from the S3 falsification run |
| Sim resolution | `resolution_mm = 0.3`, `sample_step_mm = 0.3` (top level and `trace.sample_step_mm`) |
| Trace schema | `trace.schema_version = 5`, `provenance.captured_arc_engagement = true` |
| Samples | `trace.samples`, byte range `[350856583, 7706449456)` = 7.36 GB, **5,478,011 records** — matches `trace.summary.sample_count` exactly |
| **git HEAD** | **`d25e0a74f251063db167740a8386f0496c7ebe2b`**. The task brief said `c2c623a7`; both are recorded here, neither silently adopted. The trace itself carries no git hash — `trace.provenance` is toolpath/tool/config/stock/machine hashes only. |
| Script | `m1_exposure.py` (this directory) |
| Raw counters | `m1_counters.json` (this directory) — every table below is derived from it |

**A note on `provenance.tool_hashes`.** Toolpaths 4 (R1.0 taper) and 9 (R0.5
taper) share hash `8271774307161459559`. That is not a mixup: `build_simulation_provenance`
(`compute/simulate.rs:489-505`) hashes `entry.tool.diameter()` — which for a
tapered ball is the **shaft** — plus shank/holder/stickout/flutes, and **no
cutter shape at all**. `compute/sim_prefix.rs:115-121` says so in those words.
The provenance block is therefore not usable to identify the cutter; the
toolpath→tool mapping below comes from the per-toolpath dumps' `"tool"` field
(`tp_*.json`) instead. It is the same envelope-stands-in-for-cutter substitution
U3 is about, in a second place.

---

## 2. Method

**Streaming, not sampling.** The file is serde_json *pretty* output, so every
sample object is one field per line and records are delimited by
`\n      {\n` / `\n      }`. `m1_exposure.py` splits on those, then extracts
ten fields per record by ordered `bytes.find` (a missing key raises; nothing
defaults). 8 worker processes over 8 byte ranges aligned to record boundaries;
14 s wall for the full 7.36 GB. **Every number in §4–§7 is a full-population
count — there is no stride anywhere, including the distribution sketch.**

**Parser validation.** 4,000 records drawn from five widely separated file
offsets were re-parsed with `json.loads` and compared field-by-field: **0
mismatches**. The same check asserted `axial_doc_mm == axial_engagement_mm`
and that `axial_engagement_mm` and `plunge_descent_mm` are never both positive
(the split at `dexel_stock/simulation.rs:1077`).

**Wire fields used** (`SimulationCutSample`, `simulation_cut.rs:163-241`):

| Quantity | Wire field |
|---|---|
| toolpath | `toolpath_id` |
| cutting vs rapid | `is_cutting` |
| the U3 metric | `engagement.radial_woc_fraction` |
| time | `segment_time_s` |
| material | `removed_volume_est_mm3` |
| axial penetration | `axial_engagement_mm` + `plunge_descent_mm` (exactly one is non-zero; their sum is the stamp's own `max_penetration`) |
| arc availability | `arc_engagement_radians` (null / non-null) |
| kinematics class | `cut_kinematics` |
| transit | `in_transit_span` |

**"Air" is replicated, not assumed.** `SummaryAccumulator::observe`
(`simulation_cut.rs:1234-1244`) classifies a sample as air-cut iff
`is_cutting && engagement.radial_woc_fraction < 0.02` — the *same* predicate as
the three load-gate filters (`chipload.rs:315`, `power.rs:192`,
`deflection.rs:87` and `:219`) and the issue emitter (`:919`). This script
applies that predicate directly.

**The corrected denominator** replicates `MillingCutter::engagement_radius_mm`
= `width_at_height` in Python for each shipped shape:
`FlatEndmill` (`tool/flat.rs`) → `radius()` for every depth;
`VBitEndmill` (`tool/vbit.rs`) → `min(h·tan(included/2), R)`;
`TaperedBallEndmill` (`tool/tapered_ball.rs`) → ball region
`sqrt(2·R_ball·h − h²)` below `h_contact = R_ball(1−sin α)`, cone region
`min((h − cone_offset)·tan α, R_shaft)`. Geometry from `wanaka200.toml`
`[[tools]]`, constructed the way `compute/cutter.rs::build_cutter` does
(`diameter` is the **ball** diameter for a tapered ball; `diameter()` reports
the **shaft**, so `radius() == shaft_diameter/2 == 3.0` for all three tapers).
**Anchor: R1.0 taper at 0.5 mm DOC gives 3.0 / 0.8660 = 3.464×**, reproducing
`PLAN.md`'s analytic "3.46× at 0.5 mm DOC" to three digits.

`corrected_rwoc = min(1.0, radial_woc_fraction × envelope_radius /
engagement_radius(doc))`. This is a post-hoc rescale of the *stored* metric:
U3 is purely the denominator at `stamping.rs:1119`, and the numerator
(`perp_max − perp_min`) is unaffected by it.

**Reconciliation against the trace's own aggregates — all exact:**

| Anchor | Rust summary | This pass |
|---|---|---|
| records | 5,478,011 | 5,478,011 |
| Σ `removed_volume_est_mm3` | 809,774.4168 | 809,774.4168 |
| `air_cut_time_s` | 49,740.2449 | 49,740.2449 |
| `average_engagement` | 0.055712837 | 0.055712837 |
| per-kinematics counts | linear 376,361 / plunge 675,011 / helix 3,756,074 / arc 180,288 | identical |
| per-toolpath `air_cut_time_s`, `average_engagement`, `total_removed_volume_est_mm3` | `toolpath_summaries` | identical on all six rows |

**Toolpath → cutter map** (wire id ← the `.toml`'s numbering; `PLAN.md`'s
"ops 7/8" are wire ids 8/9, "op 6" is wire id 5):

| wire id | name | cutter | envelope radius | note |
|---|---|---|---|---|
| 1 | 2 Back Rough | Ø6 flat end mill | 3.0 | control |
| 3 | 4 Rivers | Ø5.5 20° V-bit | 2.75 | |
| 4 | 5 Lakes | R1.0 tapered ball, α 5.7° | 3.0 | |
| 5 | 6 3D Rough (front) | Ø6 flat end mill | 3.0 | **control** |
| 8 | 7 3D Finish | R1.5 tapered ball, α 2.8° | 3.0 | **taper** |
| 9 | 8 Pencil detail | R0.5 tapered ball, α 7.1° | 3.0 | **taper** |

**Ids 0 and 2 (Pin Drill, Holes) carry no samples at all** — the trace's
`summary.toolpath_count` is 6, not 8. Drill ops are analytic and publish
`drill_samples` / `drill_summaries` instead. Nothing in M1 covers them.

---

## 3. What is NOT on the wire

Nothing M1 needs is missing. Specifically present and used: per-sample
`radial_woc_fraction`, per-sample axial penetration, per-sample
`removed_volume_est_mm3`, per-sample `segment_time_s`, and the arc/kinematics/
transit flags. No in-process instrument is required for the questions asked.

Two things a reader might expect and should not: the trace carries **no
per-sample `perp_max`/`perp_min`** (only their ratio, post-clamp — see
Limit L2) and **no build/git identity** in `provenance`.

---

## 4. Measurement 1 — the sub-0.02 population, per toolpath

Field: `engagement.radial_woc_fraction < 0.02` over `is_cutting == true`
samples. "Vol" is Σ `removed_volume_est_mm3` over that population.

| id | op | cutting samples | sub-0.02 n | share of cutting n | sub-0.02 time (s) | share of op cutting time | sub-0.02 vol (mm³) | share of op vol |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 2 Back Rough (flat) | 728,657 | 296,815 | 40.73 % | 1,420.43 | 17.62 % | 1,876.398 | 0.320 % |
| 3 | 4 Rivers (V-bit) | 103,724 | 44,376 | 42.78 % | 106.90 | 31.10 % | 1.590 | 0.018 % |
| 4 | 5 Lakes (R1.0) | 8,499 | 2,234 | 26.29 % | 15.05 | 20.85 % | 0.569 | 0.015 % |
| 5 | **6 3D Rough (flat)** | 435,812 | 233,270 | **53.53 %** | 905.24 | 44.64 % | 1,959.431 | 1.253 % |
| 8 | **7 3D Finish (R1.5)** | 2,954,187 | 1,165,532 | **39.45 %** | 4,315.56 | 37.68 % | 1,817.565 | 3.825 % |
| 9 | **8 Pencil (R0.5)** | 756,855 | 655,758 | **86.64 %** | 42,977.06 | 81.03 % | 200.414 | 2.685 % |
| — | **project** | **4,987,734** | **2,397,985** | **48.08 %** | **49,740.24** | **66.32 %** | **5,855.965** | **0.723 %** |

**The decisive split.** Of the 2,397,985 sub-threshold samples:

| | n | share of sub-0.02 | time (s) | vol (mm³) |
|---|---:|---:|---:|---:|
| `radial_woc_fraction == 0.0` exactly | 2,391,921 | **99.75 %** | 49,444.71 | 5,530.889 |
| `0 < radial_woc_fraction < 0.02` | **6,064** | **0.25 %** | 295.53 | 325.077 |

Per toolpath, the second row is: op 7 (R1.5) **0 samples**; op 8 (R0.5) 2,450;
op 6 (flat) 2,116; op 2 (flat) 1,496; Rivers 1; Lakes 1.

**Note that the taper ops are not the outliers here.** The two flat-endmill
roughs discard 40.7 % and 53.5 % of their cutting samples under the same
filter, on a tool where the U3 correction factor is *identically 1.0*. The
sub-0.02 population is overwhelmingly a **zero-reading** population, and zero
readings are not a U3 symptom.

Supplementary (the gates' own additional filters):

- `is_steady_state_for_gate` (`tool_load/locality.rs:156`) is applied at
  **`tool_load/mod.rs:296` only** — the engaged-diameter lookup. The three
  `< 0.02` sites named in `PLAN.md` do **not** drop transit samples. Reported
  for completeness: sub-0.02 samples that are also non-transit are 1,369,635
  project-wide (op 7: 1,165,432 of 1,165,532 — essentially all; op 8: 16,954 of
  655,758 — 97.4 % of the pencil's air population is `in_transit_span`).
- Power and deflection additionally require a non-null
  `arc_engagement_radians`. Among cutting samples that clear 0.02, arc is
  present for 2,509,093 project-wide; op 7 has arc on **all** 1,788,655 of its
  clearing samples, op 8 on 100,609 of 101,097.

---

## 5. Measurement 2 — air-cut misattribution mass

"Classified as air" = the wire predicate of §2 (`is_cutting` ∧ rwoc < 0.02) —
the exact rule that feeds `air_cut_time_s` and hence
`air_cut_pct_of_total_runtime`. "Removed real material" = `removed_volume_est_mm3 > 0`.

| id | op | air-classified n | of which removed vol > 0 | share | their time (s) | their vol (mm³) | vol as % of op vol |
|---|---|---:|---:|---:|---:|---:|---:|
| 1 | 2 Back Rough | 296,815 | 54,976 | 18.52 % | 432.99 | 1,876.398 | 0.320 % |
| 3 | 4 Rivers | 44,376 | 485 | 1.09 % | 0.94 | 1.590 | 0.018 % |
| 4 | 5 Lakes | 2,234 | 80 | 3.58 % | 0.74 | 0.569 | 0.015 % |
| 5 | 6 3D Rough | 233,270 | 68,038 | 29.17 % | 326.65 | 1,959.431 | 1.253 % |
| 8 | **7 3D Finish** | 1,165,532 | **504,344** | **43.27 %** | 2,395.31 | 1,817.565 | **3.825 %** |
| 9 | **8 Pencil** | 655,758 | 75,348 | 11.49 % | 2,413.22 | 200.414 | 2.685 % |
| — | **project** | **2,397,985** | **703,271** | **29.33 %** | **5,569.85** | **5,855.965** | **0.723 %** |

Read that as: **5,569.85 s — 11.20 % of the reported air-cut time, 6.79 % of
the 82,056.07 s total runtime — is time the metric calls air during which the
dexel says material left the stock.** No non-cutting (rapid) sample anywhere in
the trace carries removed volume (0 across all six toolpaths), so the whole
misattribution lives inside the cutting population.

**How much of that does the U3 fix actually reclassify?** Only the samples with
a strictly positive sub-threshold reading can be scaled across 0.02:

| id | flips (rwoc<0.02 → corrected ≥0.02) | time (s) | vol (mm³) | with arc | non-transit |
|---|---:|---:|---:|---:|---:|
| 3 Rivers | 1 | 0.012 | 0.060 | 1 | 1 |
| 4 Lakes | 1 | 0.006 | 0.025 | 1 | 1 |
| 8 3D Finish | **0** | 0.0 | 0.0 | 0 | 0 |
| 9 Pencil | **2,107** | 213.84 | 24.303 | 2,107 | 281 |
| 1, 5 (flats) | 0 | 0.0 | 0.0 | 0 | 0 |
| **project** | **2,109** | **213.86** | **24.389** | 2,109 | 283 |

`air_cut_pct_of_total_runtime`: **60.6174 % → 60.3568 %**, a **−0.26 pp** move
(air time 49,740.24 s → 49,526.38 s, denominator `total_runtime_s`
82,056.07 s). Per op the only material change is Pencil: 81.03 % → 80.63 % of
its own cutting time.

**So `PLAN.md`'s M-c — "air-cut % on a tapered tool has a zero set by the
shank" — is not supported at this resolution.** The zero set is set by the
dexel's *numerator* going to a hard zero, not by the shank denominator.
`PLAN.md`'s M-a example ("discards a real 0.12 mm side bite as air") does not
appear on this trace as a small positive number that the filter then rejects;
it appears as an exact `0.0`. §7 L3 explains why, and it is a resolution
finding, not an exoneration.

---

## 6. Measurement 3 — distribution of `radial_woc_fraction`

Full-population histogram over cutting samples, bin width **0.0002** (5,000
bins over [0,1]); quantiles are count-weighted and resolved to ±0.0002. **No
sampling stride.** `0.0000` in a quantile column means the bin is the first one,
i.e. that sample reads exactly zero. This table is the *shipped* distribution —
all cutting samples, the population the 0.02 filter actually sees. §7 restates
op 7 and op 8 on the narrower defined population where a corrected comparison
is legitimate.

| id | op | cutter | p50 | p90 | p99 | sub-0.02 share |
|---|---|---|---:|---:|---:|---:|
| 5 | **6 3D Rough (front)** | Ø6 flat | **0.0000** | 0.9001 | 0.9999 | 53.53 % |
| 8 | **7 3D Finish** | R1.5 taper | **0.0501** | 0.2001 | 0.3501 | 39.45 % |
| 9 | **8 Pencil detail** | R0.5 taper | **0.0000** | 0.0683 | 0.1599 | 86.64 % |
| 1 | 2 Back Rough | Ø6 flat | 0.1001 | 0.2475 | 0.9999 | 40.73 % |
| 4 | 5 Lakes | R1.0 taper | 0.4249 | ~1.0 | ~1.0 | 26.29 % |
| 3 | 4 Rivers | V-bit | 0.2121 | ~1.0 | ~1.0 | 42.78 % |

**The gap in the middle is the story.** The modal non-zero bins on op 7 are
0.0498/0.0500 (262,120 + 194,177), 0.0998/0.1000 (247,015 + 148,941),
0.1498/0.1500 (154,860), 0.2000 (148,171) — i.e. **integer multiples of 0.05**.
That is exactly one 0.3 mm grid cell over the 6.0 mm envelope denominator:
`0.3 / 6.0 = 0.05`. **The smallest non-zero radial reading the instrument can
produce on a 6 mm-denominator tool at 0.3 mm resolution is 2.5× the 0.02
filter.** The interval `(0, 0.02)` is therefore nearly unreachable — op 7 has
literally zero samples in it — and the population is bimodal: exactly-zero, or
≥ one cell.

---

## 7. Measurement 4 — the measured correction factor

`factor = envelope_radius / engagement_radius_mm(doc)`, per sample, with
`doc = axial_engagement_mm + plunge_descent_mm`. Defined only where
`engagement_radius > 0`; quantiles count-weighted over the defined population,
log-binned at 0.005 dex.

| id | op | cutter | defined n (share of cutting) | factor p50 | p90 | p99 |
|---|---|---|---:|---:|---:|---:|
| 1 | 2 Back Rough | Ø6 flat | 728,657 (100 %) | **1.000** | 1.000 | 1.000 |
| 5 | 6 3D Rough | Ø6 flat | 435,812 (100 %) | **1.000** | 1.000 | 1.000 |
| 8 | **7 3D Finish** | R1.5 taper | 2,304,733 (78.0 %) | **4.05** | 7.72 | 23.04 |
| 9 | **8 Pencil** | R0.5 taper | 414,523 (54.8 %) | **21.50** | 21.50 | 64.94 |
| 4 | 5 Lakes | R1.0 taper | 8,489 (99.9 %) | 15.05 | 15.05 | 15.05 |
| 3 | 4 Rivers | V-bit | 103,600 (99.9 %) | 781 | 781 | 781 |

The flat-endmill rows reading exactly 1.000 are the control: `width_at_height`
is constant for a cylinder, so U3 is a no-op there by construction, and the
script reproduces that rather than assuming it.

**Where the correction lands — time-weighted mean engagement** (the
`average_engagement` headline), computed over the *defined* subpopulation so
both sides are the same measure:

| id | op | mean rwoc (raw, all cutting) | mean rwoc (raw, defined) | mean rwoc (**corrected**, defined) | ratio |
|---|---|---:|---:|---:|---:|
| 1 | 2 Back Rough | 0.17325 | 0.17325 | 0.17325 | **1.00×** |
| 5 | 6 3D Rough | 0.21247 | 0.21247 | 0.21247 | **1.00×** |
| 8 | **7 3D Finish** | 0.08574 | 0.10235 | **0.37726** | **3.69×** |
| 9 | **8 Pencil** | 0.02271 | 0.09117 | **0.52374** | **5.74×** |
| 4 | 5 Lakes | 0.52678 | 0.52707 | 0.74930 | 1.42× (clamped) |
| 3 | 4 Rivers | 0.36747 | 0.36771 | 0.68913 | 1.87× (clamped) |

Quantiles for the two taper finish ops, **both sides taken over the same
defined population** (`rwoc_quantiles_defined` vs `corrected_rwoc_quantiles`
in `m1_counters.json`) — a raw quantile over *all* cutting samples would be
dragged down by the doc ≤ 0 zeros the corrected side never sees, so it is not
comparable and is not used here:

| id | op | raw p50 (defined) | corrected p50 | raw p90 | corrected p90 | raw p99 | corrected p99 |
|---|---|---:|---:|---:|---:|---:|---:|
| 8 | 7 3D Finish | 0.0999 | **0.3663** (3.67×) | 0.2501 | **0.8423** (3.37×) | 0.3501 | ~1.0 |
| 9 | 8 Pencil | 0.0000 | 0.0000 | 0.1107 | **0.7839** (7.08×) | 0.1791 | ~1.0 |

(For reference, the same op-7 median over *all* cutting samples is 0.0501 —
§6's number. The 0.0999 above is the honest matched-population figure and the
one that agrees with the 3.69× time-weighted mean.)

Samples with `engagement_radius == 0` (doc ≤ 0, correction undefined):
991,920 project-wide, 41,688.40 s — op 7 649,454 (1,859.23 s), op 8 342,332
(39,828.91 s). **Every one of them reads `radial_woc_fraction == 0.0`** —
measured, not inferred: the counter `doc_le0_rwoc_gt0_n` is **0 on all six
toolpaths**. Excluding them therefore cannot bias the flip counts, and it is
what makes the matched-population table above exact (the defined histogram is
the full histogram with bin 0 reduced by `doc_le0_n`).

---

## 8. Honest limits

**L1 — corrected values saturate.** 203,360 samples project-wide would exceed
`radial_woc_fraction = 1.0` under the corrected denominator and are clamped:
Rivers 59,199 (57 % of its cutting samples), op 7 125,110 (4.2 %), op 8 13,964,
Lakes 5,087. Every corrected p90/p99 above that reads `~1.0` is a **floor**, not
a physical fraction. The V-bit's factor of ~780 is arithmetically real and
physically meaningless — it says a cone tip at sub-cell depth has essentially no
width, which the grid cannot represent.

**L2 — the raw metric is already censored.** `stamping.rs:1119` clamps to
`[0,1]` before serialisation, so 80,656 samples on the wire read exactly 1.0
(Rivers 40,241, Lakes 4,179, op 6 23,096, op 2 12,652 — op 7: none). Their
pre-clamp value is unrecoverable, so their true correction ratio is unknown.

**L3 — the grid is coarser than the corrected denominator, on the tools that
matter.** At `resolution_mm = 0.3`, `2 · engagement_radius(doc) < 1 cell` for
378,674 samples: **op 8 262,001 (63 % of its defined population)**, Rivers
86,404 (83 %), op 7 30,261 (1.3 %). Where that holds, the corrected fraction is
being formed from a numerator quantised to 0.3 mm over a denominator below
0.3 mm — an upper bound, not a measurement. This is the repo's own standing rule
("sim cell must be well below the tool TIP radius") being violated by this
trace: a 0.3 mm cell against an R0.5 tip whose engaged radius at cut depth is
~0.14 mm. **§5's "the fix barely moves air-cut %" conclusion is
resolution-conditional**; a 0.05 mm re-sim of the two finish ops could produce
sub-cell positive readings in `(0, 0.02)` where this trace has hard zeros, and
that population *would* flip. M2 should not treat §5 as the final word without
one such run.

**L4 — this is a rescale of a stored scalar, not a re-run of the stamp.** U3 is
the denominator only, and the numerator `perp_max − perp_min` is provably
unaffected (verified by reading `StampPartial::finish`). But everything
*downstream* of radial is recomputed from it, and this pass does **not** model
that: `arc = acos(1 − 2w)` (`stamping.rs:1136-1146`), the chip-thickness
statistics derived from that arc, and `power.rs:205-206`'s
`radial_width = (arc/π)·engagement_radius·2`. M-b's ~1.9× arc/power figure is
untouched by M1 and stays analytic.

**L5 — plunge samples.** For `cut_kinematics == "plunge"` the axial reading is
carried in `plunge_descent_mm`, not `axial_engagement_mm`
(`dexel_stock/simulation.rs:1077`); the sum of the two is used as the depth
argument. Plunges are 675,011 samples (88.1 % of them sub-0.02) but only
1,720 s, and they contribute **0 flips**.

**L6 — the gates' further filters are not modelled in the headline numbers.**
§4's supplementary bullet gives the transit and arc splits; the headline
population numbers apply the `< 0.02` predicate alone, because that is what
`PLAN.md` asked for and what `air_cut_time_s` uses.

**L7 — one job, one resolution, one machine.** Six toolpaths, three cutter
shapes. Nothing here generalises to a project with a different taper mix.

---

## 9. What M2 should expect

**Populations barely move; magnitudes move a lot — and the two flat-endmill
roughs do not move at all.** A fix at `stamping.rs:1119` reclassifies 2,109
samples project-wide (0.04 % of the cutting population, 214 s, 24 mm³), all but
two of them on op 8 Pencil, so **no gate should gain or lose a meaningful
population, and no `Unmodeled{AllSamplesAirCutOrRapid}` verdict should flip to
"modelled" on this fixture.** What changes is what the surviving population
*says*: on the two taper finish ops the time-weighted engagement rises 3.69×
(op 7) and 5.74× (op 8), and the median op-7 sample goes from 0.0999 to 0.3663
(matched populations) — from "a tenth of the shank" to "a third of the engaged
diameter". The gates
whose verdicts should be captured before/after, in order of expected movement,
are (1) **power** on ops 7 and 8 — it consumes both the corrected radial *and*
the arc derived from it, and `power.rs:205-206` is separately self-inconsistent
(M-b), so it moves twice; (2) **deflection** on the same two ops, which reads
the same arc as an immersion angle and has 1.79 M and 0.10 M arc-carrying
samples respectively to re-grade; (3) **chipload**, which is no longer
engagement-aware since the 2026-08-06 unit deletion and should move *least* —
if a chipload verdict moves a lot, that is a finding about the fix, not about
the tool. Ops 2 and 6 (Ø6 flat) are the falsification control: their correction
factor is identically 1.000 and **any** verdict change there means the patch
touched something other than the denominator. Expect Rivers and Lakes to
saturate rather than grade (57 % and 60 % of their samples clamp at 1.0), which
makes them poor evidence either way. Finally, M2's before/after capture should
be taken at a finer resolution than 0.3 mm for ops 7 and 8, or it will inherit
L3 and understate the population effect exactly as this pass does.
