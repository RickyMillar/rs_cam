# SIM wave 5b — landing S1: `Auto` resolves to `Swept`

Companion to `DELTA_sim_w5_s1_DECISION.md`, which is the **evidence**; this is
the **landing**. The decision package recommended splitting the landing and not
taking full `Swept` yet. The user overrode that explicitly and in full
knowledge of the cost — land `SweptPlungeOnly` **and** full `Swept` with
re-baselines, accepting the classified metric movements and the rest-chain
geometry change. That decision is recorded here so the next reader does not
mistake this wave for the package's own recommendation.

Branch `tech-debt-3`. Merge is `d671b049` (the lane's seven commits kept, not
squashed).

| Commit | What |
|---|---|
| `d671b049` | merge `perf/s1-swept-volume` — default NOT flipped |
| `a4ff2a8c` | **the flip**: `StampDispatch::resolved()`, `Auto` → `Swept` |
| `9b4505be` | re-baseline both perf goldens |
| `a4452a59` | re-baseline the three F-XXX axial sentries |
| `b5b6db3c` | planner/sim parity — skew bar restated over the interior population |
| `dbb9c8fa` | S5 prefix key: why the dispatch shape is excluded, + precondition sentry |
| `217be7a2` | clippy clean (9 pre-existing findings on the lane's files, 1 mine) |

---

## 0. What reproduced, and the one thing that did not

The decision package's numbers were measured 2026-08-19/20 in a different
session. Paired re-measurement rules apply, so everything load-bearing was
re-measured today (2026-08-21) before being acted on.

**Reproduced exactly:**

| Claim | Package | Today |
|---|---|---|
| Golden fields moved, 2.5D / 3D | 39 / 29 | **39 / 29** |
| Exact-compared fields moved | 1 (`triage_action_count` 3 → 1) | **1, same field, same values** |
| F-027 band max axial | 3.694 mm, 3 of 54 477 | **3.694 mm, 3 of 54 477** |
| F-027 outlier count | 3 | **3** |
| F-031 steady max axial | 3.890 mm, 10 of 630 865 | **3.890 mm, 10 of 630 865** |
| Parity: AgentSearch interior | 704 → 21 of 5184 | **704 → 21** |
| Parity: ContourParallel interior | 647 → 8 of 5184 | **647 → 8** |
| Parity: `sim_higher` unchanged | 1360 / 568 both modes | **1360 / 568, bit-for-bit** |
| wanaka: every metric row in §4 of the package | see there | **every one, to the printed digit** |

**Did NOT reproduce:** `cargo clippy --workspace --all-targets -- -D warnings`
was **not** clean on the merged lane. The package reports "zero warnings" for
the side branch; run today at the merge commit it produced 4
`needless_range_loop` warnings in `swept.rs` and 6 `print_stdout` errors in
`tests/swept_stamping_s1.rs`, all on the lane's own files and none introduced
by the flip. Fixed in `217be7a2` (allow-with-reason in both cases — the index
loops address `weight(b)` / `job.bin_segment(b)` as well as `out[b]`, and the
test binary prints its measurements deliberately). Reported rather than
smoothed over, because it is the only claim in the package that did not hold.

---

## 1. The flip itself

`StampDispatch::resolved()` is the single site where `Auto` becomes a concrete
shape. `simulation.rs` resolves once and asks both schedulers the same
question; `BandDispatch::for_grid` resolves at entry so it cannot disagree.
Two independent resolutions of `self.stamp_dispatch` is how a grid ends up with
neither scheduler or with both.

**`RS_CAM_STAMP_DISPATCH` stays.** It is the A/B instrument every number in the
decision package and in this document was measured with, and `per_stamp` /
`whole_path` / `swept_plunge` remain selectable because the bit-identity and
determinism sentries are written against them.

### One correctness change came with the flip: `MIN_BANDS_FOR_SWEPT` 2 → 1

While swept was opt-in, "too small to bother batching" cost nothing — the
caller asked for a schedule and got a slightly different one. At the default it
means something else: a stock of fewer than `2 · BAND_ROWS = 16` rows would
fall through to the per-stamp kernel and **silently publish the old measure**
(sample-density-dependent removal, resolution-dependent air-cut, under-read
axial DOC) while every larger stock published the new one. A measure that
changes with the stock's row count is the defect class this review keeps
finding. Swept now runs on any grid with rows at all; `bands == 0` (an empty
grid) is the only decline. Pinned by `swept_applies_to_a_single_band_grid`,
which asserts the band count is really 1 *before* asserting anything else.

### The sentry that guarded the old default is inverted, not deleted

`auto_never_selects_swept_dispatch` → `auto_selects_swept_dispatch`, with two
halves: `Auto` must equal `Swept` bit-for-bit **and** must differ from
`WholeToolpath` on air fraction. Without the second half the test would pass if
the flip were quietly reverted and the two shapes happened to agree on this
fixture.

---

## 2. Re-baseline 1 — the two perf goldens

Regenerated with `UPDATE_PERF_GOLDENS=1`, the test's own documented mechanism.
The old→new value of every moved field was recorded **before** regeneration.

### The exact-compared column: one field moved, and it is downstream

`triage_action_count: 3 → 1`. Two air-cut actions dropped below their
threshold — a consequence of class (ii-b) below, not an independent movement.

Unchanged, with no tolerance, in both goldens: `project_collision_count`,
`project_rapid_collision_count`, `holder_collision_total`,
`triage_safety_count`, `total_sample_count`, `resolution_mm`, `toolpath_count`,
and per toolpath `name`, `op_kind`, `sample_count`, `move_count`,
`collision_count`, `rapid_collision_count`, `metrics_not_applicable`, plus
every per-kinematics `sample_count`. **The sample stream and every collision
channel are bit-stable across the flip.**

### 2.5D golden — all 39 moved fields

| Field | old | new | class |
|---|---:|---:|---|
| `triage_action_count` | 3 | 1 | (ii-b) downstream |
| `project_air_cut_pct_of_total_runtime` | 74.8057 | 50.0309 | (ii-b) |
| `project_air_cut_pct_of_cutting_time` | 77.3849 | 51.7558 | (ii-b) |
| `project_average_engagement` | 0.1038 | 0.1966 | (ii-b) |
| `project_total_removed_volume_est_mm3` | 31 010.108 | 30 380.696 | (i) |
| `[Pocket].air_cut_time_s` | 205.991 | 36.072 | (ii-b) |
| `[Pocket].total_removed_volume_est_mm3` | 21 557.759 | 21 191.965 | (i) |
| `[Pocket].average_engagement` | 0.1585 | 0.3126 | (ii-b) |
| `[Pocket].average_mrr_mm3_s` | 64.982 | 63.879 | (i) |
| `[Pocket].air_cut_pct_of_total_runtime` | 60.258 | 10.552 | (ii-b) |
| `[Pocket].air_cut_pct_of_cutting_time` | 62.092 | 10.873 | (ii-b) |
| **`[Pocket].peak_axial_doc_mm`** | **2.0625** | **3.0000** | **(ii-a)** |
| `[Pocket].kin[Linear].average_radial_woc_fraction` | 0.3434 | 0.3731 | (ii-b) |
| `[Pocket].kin[Linear].average_arc_radians` | 1.1803 | 1.2692 | (ii-b) |
| `[Pocket].kin[Linear].peak_axial_doc_mm` | 2.0625 | 3.0000 | (ii-a) |
| `[Pocket].kin[Linear].peak_chip_thickness_mm` | 0.027778 | 0.026189 | (i) |
| `[Pocket].kin[Helix].average_radial_woc_fraction` | 0.0221 | 0.2804 | (ii-b) |
| `[Pocket].kin[Helix].average_arc_radians` | 0.0733 | 1.0712 | (ii-b) |
| `[Zigzag].air_cut_time_s` | 370.560 | 357.147 | (ii-b) |
| `[Zigzag].total_removed_volume_est_mm3` | 582.689 | 541.633 | (i) |
| `[Zigzag].average_engagement` | 0.00633 | 0.02936 | (ii-b) |
| `[Zigzag].average_mrr_mm3_s` | 1.5424 | 1.4337 | (i) |
| `[Zigzag].air_cut_pct_of_total_runtime` | 94.290 | 90.877 | (ii-b) |
| `[Zigzag].air_cut_pct_of_cutting_time` | 98.088 | 94.538 | (ii-b) |
| `[Zigzag].kin[Helix].average_radial_woc_fraction` | 0.00802 | 0.03721 | (ii-b) |
| `[Zigzag].kin[Helix].average_arc_radians` | 0.02927 | 0.11360 | (ii-b) |
| `[Profile].air_cut_time_s` | 16.215 | 3.230 | (ii-b) |
| `[Profile].total_removed_volume_est_mm3` | 8869.660 | 8647.099 | (i) |
| `[Profile].average_engagement` | 0.4350 | 0.6343 | (ii-b) |
| `[Profile].average_mrr_mm3_s` | 157.082 | 153.140 | (i) |
| `[Profile].air_cut_pct_of_total_runtime` | 28.171 | 5.611 | (ii-b) |
| `[Profile].air_cut_pct_of_cutting_time` | 28.717 | 5.720 | (ii-b) |
| **`[Profile].peak_axial_doc_mm`** | **2.6367** | **3.0000** | **(ii-a)** |
| `[Profile].kin[Linear].average_radial_woc_fraction` | 0.6405 | 0.6499 | (ii-b) |
| `[Profile].kin[Linear].average_arc_radians` | 1.8438 | 1.8611 | (ii-b) |
| `[Profile].kin[Linear].peak_axial_doc_mm` | 2.6367 | 3.0000 | (ii-a) |
| `[Profile].kin[Plunge].average_radial_woc_fraction` | 0.2822 | 0.2956 | (ii-b) |
| `[Profile].kin[Helix].average_radial_woc_fraction` | 0.1767 | 0.6533 | (ii-b) |
| `[Profile].kin[Helix].average_arc_radians` | 0.5863 | 1.8724 | (ii-b) |

The fixture's `depth_per_pass` is **exactly 3.0**. Pocket read 2.0625 and
Profile 2.6367 — the shipped kernel under-reads a commanded 3 mm cut by up to
31% on this fixture and by up to 70% on the dedicated one, and the under-read
grows with cell size. Reaching the commanded depth is F-024's own stated
property.

### 3D golden — all 29 moved fields

| Field | old | new | class |
|---|---:|---:|---|
| `project_air_cut_pct_of_total_runtime` | 52.2679 | 51.3834 | (ii-b) |
| `project_air_cut_pct_of_cutting_time` | 53.1505 | 52.2511 | (ii-b) |
| `project_average_engagement` | 0.26480 | 0.26609 | (ii-b) |
| `project_total_removed_volume_est_mm3` | 3423.662 | 3418.363 | (i) |
| `[DropCutter].air_cut_time_s` | 7.4018 | 5.8693 | (ii-b) |
| `[DropCutter].low_engagement_time_s` | 1.5570 | 0.3578 | (ii-b) |
| `[DropCutter].total_removed_volume_est_mm3` | 1390.950 | 1365.259 | (i) |
| `[DropCutter].average_engagement` | 0.2888 | 0.3538 | (ii-b) |
| `[DropCutter].average_mrr_mm3_s` | 84.463 | 82.903 | (i) |
| `[DropCutter].air_cut_pct_of_total_runtime` | 43.235 | 34.283 | (ii-b) |
| `[DropCutter].air_cut_pct_of_cutting_time` | 44.946 | 35.640 | (ii-b) |
| **`[DropCutter].peak_axial_doc_mm`** | **2.1222** | **4.3354** | **(ii-a)** |
| `[DropCutter].kin[Plunge].average_radial_woc_fraction` | 0.6720 | 0.6800 | (ii-b) |
| `[DropCutter].kin[Helix].average_radial_woc_fraction` | 0.2032 | 0.2810 | (ii-b) |
| `[DropCutter].kin[Helix].average_arc_radians` | 0.6854 | 0.9186 | (ii-b) |
| `[DropCutter].kin[Helix].peak_axial_doc_mm` | 2.1222 | 4.3354 | (ii-a) |
| `[Waterline].air_cut_time_s` | 54.730 | 55.211 | (ii-b) |
| `[Waterline].low_engagement_time_s` | 1.5774 | 3.0467 | (ii-b) |
| `[Waterline].total_removed_volume_est_mm3` | 2032.712 | 2053.103 | (i) |
| `[Waterline].average_engagement` | 0.2609 | 0.2517 | (ii-b) |
| `[Waterline].average_mrr_mm3_s` | 20.240 | 20.443 | (i) |
| `[Waterline].air_cut_pct_of_total_runtime` | 53.788 | 54.261 | (ii-b) |
| `[Waterline].air_cut_pct_of_cutting_time` | 54.496 | 54.975 | (ii-b) |
| `[Waterline].kin[Plunge].average_radial_woc_fraction` | 0.09143 | 0.10159 | (ii-b) |
| `[Waterline].kin[Helix].average_radial_woc_fraction` | 0.2141 | 0.1958 | (ii-b) |
| `[Waterline].kin[Helix].average_arc_radians` | 0.6667 | 0.6182 | (ii-b) |
| `[Waterline].kin[Helix].peak_radial_woc_fraction` | 0.87002076 | 0.87002133 | (i) |
| `[Waterline].kin[Arc].average_radial_woc_fraction` | 0.4823 | 0.4935 | (ii-b) |
| `[Waterline].kin[Arc].average_arc_radians` | 1.3936 | 1.4271 | (ii-b) |

Note `[Waterline].kin[Helix].peak_radial_woc_fraction`: it moved by
**6.5 × 10⁻⁷ relative**, tripping a `1e-9` bar. It is in the table because
every moved field is in the table, not because it means anything.

Note also that `[Waterline]` moves in the *opposite* direction from
`[DropCutter]` on air-cut, removal and engagement. (ii-b) is not "air goes
down": it is "air stops being a function of the cell size", and on a fixture
where the old reading happened to be low the correction goes the other way.

### Non-vacuity

`golden_fixture_is_not_vacuous`, `golden_3d_fixture_is_not_vacuous` and
`three_d_arm_covers_arc_and_helix_kinematics` pass **unchanged**. No tolerance
was widened.

---

## 3. Re-baseline 2 — the three F-XXX axial sentries

All three assert `axial_engagement_mm ≤ commanded dpp + 0.5`.

| Sentry | reading | population |
|---|---:|---|
| `as013_terrain_model_edge_axial_within_commanded_dpp_f027` | 3.694 mm | 3 of 54 477 band samples |
| `as013_terrain_model_edge_band_outlier_count_*_f027` | 3 outliers | (asserted `== 0`) |
| `as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031` | 3.890 mm | 10 of 630 865 steady-state |

The toolpaths are unchanged in these fixtures; only the measurement moved. The
old kernel's per-sample maximum sat *below* the column's real removal, so
`dpp + 0.5` was being cleared **by an under-read, not by the geometry**.

**These are safety-adjacent nets, so the bar was split rather than raised:**

| | old | new |
|---|---|---|
| absolute per-sample ceiling | `dpp + 0.5` = 3.500 | `dpp + 1.0` = **4.000** |
| the `dpp + 0.5` bar | (the ceiling) | **kept**, now bounding the *fraction* of samples above it, capped at **0.05%** |

**Which half carries which defect is NOT symmetric across the two sentries,
and the obvious arithmetic gets F-031 wrong:**

| | defect read | today | ceiling margin | defect count | today | vs the 0.05% cap |
|---|---:|---:|---:|---:|---:|---|
| F-027 | 30–47 mm | 3.694 mm | 7–12× | ~300 / ~54 477 (**0.55%**) | 3 / 54 477 (0.0055%) | **cap catches the defect by 11×**; 10× headroom over today |
| F-031 | 44.8 mm | 3.890 mm | **11×** | 282, and they were **transit** samples | 10 / 630 865 (0.0016%) | **cap does NOT catch that defect** — 282/630 865 is 0.045%, *under* the cap, and the test's `in_transit_span` filter excludes them anyway. 31× headroom over today. |

So for F-027 both halves catch the original defect; for F-031 the **ceiling**
does, by 11×, and the population bar's job is the different one of stopping the
residue the swept measure legitimately produces from growing by an order of
magnitude unnoticed. Stated because a table that read "both bars catch both
defects" would have been tidier and false.

The ceiling gives 8% headroom on F-027 and **2.8%** on F-031, deliberately
tight: these simulations are deterministic (swept dispatch is bit-identical
across thread counts, sentried), so a bar does not need slack it has not
earned.

`as013_terrain_model_edge_band_outlier_count_zero_f027` is renamed
`..._bounded_f027` — a test whose name says "zero" and whose assertion says
"under 0.05%" is a trap. It also now computes its band population, which it did
not before: a count bar with no denominator cannot say whether it is measuring
anything.

**Not done, and filed as follow-up W5B-F2** (below): re-expressing these against
the tool's *commanded Z travel per pass*, which is still bounded by `dpp` under
both kernels. That is the better sentry and a bigger change than a landing lane
should absorb.

---

## 4. Re-baseline 3 — the planner/sim parity pair. **This one is a tightening.**

Both tests failed under the swept default because swept made the agreement
**better** and the test asserts balance, not magnitude.

### Finding W5B-F1 — the planner over-claims removal on the boundary ring

Measured today, paired, same binary, both dispatches:

| fixture / mode | interior `planner_higher` | interior `sim_higher` | boundary `sim_higher` | whole-grid skew |
|---|---:|---:|---:|---:|
| AgentSearch, `whole_path` | 704 | **0** | 1360 | 1.55× — *passed* |
| AgentSearch, `swept` | 21 | **0** | 1360 | 34.87× — failed |
| ContourParallel, `whole_path` | 647 | **0** | 568 | 1.30× — *passed* |
| ContourParallel, `swept` | 8 | **0** | 568 | 71.00× — failed |

Interior `sim_higher` is **exactly zero in all four rows**. So the whole-grid
skew bar was never reading one population with two directions. It was reading
**two separate, each perfectly one-sided populations**:

* **Interior — the simulator over-removing.** This is the old kernel's
  per-subsegment `f`-blend artifact, class (i), and it is what swept fixed:
  704 → 21 and 647 → 8. Interior disagreement falls **33×** and **81×**.
* **Boundary ring** (outside `mesh bbox ± 1 mm`) **— the planner
  over-claiming**: the planner's bookkeeping claims removal its own emitted
  path does not deliver. 1360 and 568 cells (49.7% and 20.8% of the 2737-cell
  boundary population), and **bit-for-bit identical under `whole_path` and
  under `swept`**. 98.7% / 100% of the boundary divergence is in this one
  direction.

Summed over the whole grid the two nearly cancelled, and **1.55× / 1.30× is
what that cancellation looked like from outside.** The bar was passing on an
accident.

W5B-F1 is therefore: *adaptive3d's planner claims removal on the boundary ring
that its emitted toolpath does not deliver, on both clearing strategies,
independent of the stamp kernel, pre-dating this campaign.* It is the direction
`ParityResult::sim_higher`'s own docstring predicts for "an emitter-side
transform missing from the planner's mirror", and the residual the drape-mirror
commit documented as NOT FIXED (`Cut` segments whose first emitted feed sweeps
from the emitter's true tool position rather than from the planner's raw
`last_pos`) is the standing candidate. **Not investigated here.** Owner: a
follow-up item, not this lane.

### What the tests do now

1. **The skew bar reads the interior population.** Bar unchanged at 2.5×. Under
   `whole_path` that interior skew is **704× and 647×**, so this is a
   *tightening*: `RS_CAM_STAMP_DISPATCH=whole_path` no longer runs this pair
   green, and that is correct — the old kernel really does disagree with the
   planner on 704 interior cells, all one way.
2. **The skew bar abstains below a 50-cell interior floor**, loudly (it prints),
   and the abstention is guarded by a 1%-of-interior count bar. A ratio over 8
   cells is not a direction: `directional_skew`'s `.max(1)` would turn "8 cells,
   all one way" into "8× lopsided". This is the mirror of the empty-population
   trap in `CLAUDE.md` — there a vacuous gate *passes* and looks healthy; here
   it would *fail* and look meaningful.
3. **W5B-F1 is pinned, not buried**: a cap at 55% of the boundary population,
   against measured 49.7% / 20.8%, and the assertion message says in words that
   it is a known open finding that must not be raised away.

`ParityResult` gained `interior_planner_higher` / `interior_sim_higher`, and
the `PARITY:` line prints both, so the next reader sees the split without
re-deriving it.

---

## 5. S5 interaction — verdict: **the dispatch mode does NOT belong in the prefix key**

The question the brief raised is the right one, and since the flip the risk is
real: `Swept` produces different removal, air-cut and axial DOC than
`WholeToolpath` and leaves a slightly different grid, so a prefix carved under
one and resumed under the other would be exactly the key-closure defect class
S5's ledger warns about.

**It cannot happen, for two independent reasons:**

1. **The mode is a process constant.** `TriDexelStock::from_bounds` is the only
   constructor, every simulation path goes through it, and it stamps
   `StampDispatch::default()` — a `OnceLock`-backed read of
   `RS_CAM_STAMP_DISPATCH`. No production code assigns `stamp_dispatch`; only
   the benches and the S1/S3 sentries do, and those build their own stocks.
   Within one process, every carve used the same kernel.
2. **The cache is in-process and single-slot.** No on-disk form, no
   cross-process form, and `take_match` removes on lookup either way. A
   snapshot cannot reach a run with a different environment.

**Empirical check:** all 15 `sim_prefix_memo_s5` sentries are green at the new
default, including the four whole-result bit-pattern fingerprints that compare
memo-on against memo-off. They were expected to stay green without re-baseline
— they compare the two *within one mode* — and they did.

**The edge, recorded:** `TriDexelStock::Clone` **preserves** `stamp_dispatch`,
so a restored snapshot carries the mode it was carved under rather than
re-deriving it. Harmless under (1); the moment (1) fails, that is the line a
wrong metric crosses.
`prefix_key_may_omit_stamp_dispatch_only_while_it_is_a_process_constant` pins
(1). It does not prove the key is closed and says so — it exists so that adding
a `SimulationRequest`- or per-toolpath-level dispatch setting lands as a red
test rather than as a plausible-looking resumed metric. If it goes red the fix
is to put the shape in `SimPrefixKey`, not to relax the test. The key table in
`sim_prefix.rs`'s module docs now carries the row and the argument.

---

## 6. 0D — the new wanaka200 reference

**Supersedes 0C** (`BASELINES.md`, 2026-08-20). Nine metrics that were
`not_measurable` are now measurable, the verdict changes kind, and tp8's
geometry changes; the 0C result-consistency checks are no longer the right
comparison.

Harness: `crates/rs_cam_core/tests/swept_wanaka_ab_s1.rs`, headless, one
process, `RS_CAM_STAMP_DISPATCH` **unset** — i.e. the shipped default, which
now resolves to `Swept`. Resolution 0.4 mm, F.4 ladder driven by hand.

Machine state: load average **2.12 at start, 17.43 at end** (the run's own rayon
pool), 21 GiB available, `pgrep -x cargo` zero at launch, no other cargo job.

### Safety — the row that decides everything

| Metric | 0C | **0D** |
|---|---:|---:|
| `rapid_collision_count`, project | 0 | **0** |
| `rapid_collision_count`, every one of 8 toolpaths | 0 | **0** |
| `collision_count`, project and every toolpath | 0 | **0** |
| `resolution_clamped` | 0 | **0** |
| `effective_cell_mm` | 0.400 | **0.400** |
| ladder rounds / pending after | 2 / 0 | **2 / 0** |

**Zero collisions everywhere, unchanged.** This is structural, not luck:
`check_rapid_collisions_against_stock` runs once per toolpath *before* that
toolpath carves, against a frozen snapshot, and never reads `stamp_dispatch`,
`BandDispatch` or `SweptDispatch`.

### Verdict and measurability

| | 0C | **0D** |
|---|---|---|
| Verdict | `NOT MEASURED: air-cut % withheld for 3 toolpaths` | `WARNING: high air cutting` — tp5 51%, tp8 36%, tp9 56% (severity `Polish`, kind `air_cut`) |
| `not_measurable` rows | **9** (tp5, tp8, tp9 × 3) | **0** |
| `degraded` rows | 3 (tp1) | **12** (tp1, tp5, tp8, tp9 × 3) |
| tp1 blind fraction | 37% | **15%** |
| tp5 blind fraction | 71% (not measurable) | **27%** (degraded) |
| tp8 blind fraction | 93% (not measurable) | **31%** (degraded) |
| tp9 blind fraction | 73% (not measurable) | **46%** (degraded) |

The project stops abstaining. Read that as the instrument becoming able to
answer, not as the project getting worse: the underlying toolpaths for tp5/tp9
did not change.

### Project metrics

| Metric | 0C | **0D** | Δ |
|---|---:|---:|---:|
| `air_cut_pct_of_total_runtime` | 55.06 | **44.094** | −19.9% |
| `air_cut_pct_of_cutting_time` | 64.60 | **50.458** | −21.9% |
| `average_engagement` | 0.0515 | **0.08138** | +58.2% |
| `total_removed_volume_est_mm3` | 819 223.8 | **815 307.3** | −0.48% |
| `peak_axial_doc_mm` (= tp1) | 11.335 | **16.253** | +43.4% |
| `sample_count` | 5 696 467 | **5 646 202** | −0.88% |
| `total_moves` | 507 620 | **509 652** | +0.40% |
| `total_runtime_s` | — | **55 066.35** | — |

### Per toolpath

| # | name | op_kind | removal mm³ | air % | avg eng | peak axial | moves | rapid mm |
|---|---|---|---:|---:|---:|---:|---:|---:|
| tp0 | 1 Pin Drill | `alignment_pin_drill` | — | — | — | — | 66 | 733.5 |
| tp1 | 2 Back Rough | `adaptive3d` | 586 084.0 | 16.73 | 0.1592 | **16.253** | 11 761 | 19 944.6 |
| tp2 | 3 Holes (6mm pilot) | `drill` | — | — | — | — | 234 | 1 103.7 |
| tp3 | 4 Rivers (back, V-bit) | `project_curve` | 8 659.6 | 15.97 | 0.3464 | 3.429 | 4 365 | 13 045.1 |
| tp4 | 5 Lakes (back) | `project_curve` | 7 558.2 | 10.90 | 0.4789 | 5.143 | 1 184 | 1 535.9 |
| tp5 | 6 3D Rough (front) | `adaptive3d` | 156 356.5 | 50.77 | 0.1108 | 4.200 | 16 791 | 42 667.2 |
| tp8 | 7 3D Finish (R1.5) | `drop_cutter` | 47 475.0 | 35.71 | 0.0819 | 2.805 | 443 902 | **35 698.3** |
| tp9 | 8 Pencil detail (R0.5) | `pencil` | 9 173.9 | 55.83 | 0.0461 | 1.666 | 31 349 | 127 008.3 |

Against 0C: tp1 removal −0.73%, tp5 −2.36%, tp9 −1.16% (class (i)); tp8
**+8.47%** and its rapid distance **halves**, 73 749.0 → 35 698.3 mm (−51.6%),
with `move_count` 442 913 → 443 902.

### The geometry change, stated plainly

Four of wanaka200's eight toolpaths are `StockSource::FromRemainingStock`
(tp3, tp4, tp8, tp9). Their generators read the **simulated stock grid**, and
swept changes that grid, so the rest/finish ops **generate different motion**.
tp8's rapid distance halving is emitted G-code, not a metric. It is not
obviously an improvement or a regression — it is a different decomposition of
the remaining-stock region.

**This is the accepted risk in the user's decision.** The acceptance corpus and
the 56 param sweeps have **not** been run under the new default — the sweeps are
`#[ignore]`d by default, so a green `cargo test -p rs_cam_core` says nothing
about them. That is follow-up W5B-F3, and it is the one gate this landing did
not clear.

### Wall clock — reported, not load-bearing

| Phase | 0D (today) | package `swept` (2026-08-20) | package `auto` (2026-08-20) |
|---|---:|---:|---:|
| load | 0.576 s | 0.676 s | 0.699 s |
| generate | 26.90 s | 25.59 s | 26.19 s |
| ladder 1 sim | 51.08 s | 46.88 s | 48.17 s |
| ladder 1 regen | 104.73 s | 103.31 s | 106.92 s |
| ladder 2 sim | 57.58 s | 53.39 s | 63.52 s |
| ladder 2 regen | 8.72 s | 8.76 s | 9.38 s |
| final sim | 57.69 s | 55.70 s | 65.34 s |
| diagnostics | 2.28 s | 2.24 s | 2.18 s |
| **total** | **309.9 s** | 296.9 s | 322.8 s |

**Do not read a speed-up out of this table.** 0D is a single unpaired
cross-day absolute, and `BASELINES.md`'s cross-day rule (a 22% swing observed
with no code change) forbids comparing it to either 2026-08-20 column. The
paired figure from that session stands: **−8.0% end-to-end, −14.8/−16.0% on the
simulate phases**, and it is a lower bound because both of those runs predate
`9e65d1a7`. The stamp kernel is 3–5× faster and the end-to-end run is 8%
faster; S4 and S6 are still in front of it.

---

## 7. Verification at the new default

| Gate | Result |
|---|---|
| `cargo test -p rs_cam_core --release --no-fail-fast` | **185 targets ok, 1 failed** — and it is the known pre-existing wall-clock flake (below) |
| `cargo test -p rs_cam_viz --release --no-fail-fast` | **0 failures** |
| `cargo test -p rs_cam_cli --release` | **ok** |
| `cargo test -p rs_cam_mcp --release` | **ok** (13 tests) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **zero warnings** (after `217be7a2`) |
| `rustfmt --check` on every file this lane touches | **clean** |
| `sim_prefix_memo_s5` | **15/15**, no re-baseline |
| `swept_stamping_s1` | **9/9** + 1 measurement |
| `band_stamping_determinism_s3` | green (`per_stamp` / `whole_path` arms intact) |
| `_litmatrix_*` | 7 binaries, **no failure** |
| param-sweep fingerprints | **NOT RUN** — all 56 are `#[ignore]`d by default and need `-- --ignored`. Follow-up W5B-F3. |

**The one failure**, `wanaka_scale_indexed_path_beats_linear_scan_and_matches_output`
(`tests/remap_interval_index_c9.rs`), asserts the C9 remap interval index is
≥ 5× faster than a linear scan and read **4.4×** while 180 other test binaries
ran alongside it. Re-run **in isolation on an idle lane it passes.** It touches
no dexel code and the decision package recorded the same flake in the default
mode. Not attributed to this wave.

`RS_CAM_STAMP_DISPATCH=whole_path` is **no longer suite-green** — the two
planner/sim parity tests fail there by design (§4). That is a deliberate
consequence of stating the skew bar over the interior population, and it is
recorded here so nobody reads it as a regression.

---

## 8. Corrections to the S1 prescription

The decision package refuted four (numbered 1–5 with S1b as (5)); this landing
adds one more and confirms all of them. Consolidated list for the ledger:

| # | What the review said | What is true |
|---|---|---|
| **S1-1** | "Kill by_z analytically: the min of `z(t)+h(d(t))` is at `t_center` or the descending endpoint" | **False for any round-tipped cutter** — `f` is linear + convex and the minimum is interior. On a Ø6 ball on a 30° ramp the prescribed rule is **20× worse** than the 0.02 mm subdivision it replaces. The redundancy is **loop order, not subdivision**; inverting the loops is bit-identical and worth 3.4–5.0×. |
| **S1-2** | `floor(t_center · bins)` as the binning rule | **Aliases badly.** Whenever `sample_step < cell_size` — every coarse simulation, wanaka at 0.4 mm included — whole bins contain no cell centre: **35% of cutting samples reported zero removal** at `cs = 0.5`. The landed form smears a cell over the interval its own footprint occupies, which tiles; artifact drops to 0.13%. |
| **S1-3** | "Stamp a whole move in one stadium pass" | **A loss on diagonals.** A swept pass costs its *bounding box*, and a 100 mm move at 45° with a Ø6 cutter has an ~11 300 mm² bbox around a ~630 mm² stadium. Chunks exist for this, capped by `SWEPT_MAX_BBOX_WASTE`. |
| **S1-4** | "≈ 26×, scales as `2R/s`" | A **cell-visit** ratio quoted as a **time** ratio. Measured **2.9–5.1×**. Same class of error as S3's "6–12× desktop". |
| **S1-5** | S1b: "moves tagged `MoveIntent::Retract` may skip the removal math" | **Already done** since Step 1 (2026-05-19). Nothing left to skip. |
| **S1-6** | (this landing) "metric-changing … F-XXX / litmatrix sentries need deliberate re-baselining" | **The re-baseline list is not the risk, and litmatrix is not on it.** No `_litmatrix_*` sentry moved. What actually needed touching was two goldens, three F-XXX axial bars — and one item that is **not a re-baseline at all**: the planner/sim parity bar failed because S1 *fixed* one side of a two-sided disagreement, un-masking W5B-F1. The review also did not anticipate that S1 **changes generated geometry** on every `FromRemainingStock` op (tp8's rapid distance halves), which is emitted G-code, not a metric. |

Running count of refuted prescriptions in this review: **eleven.**

---

## 9. Open follow-ups this landing created or exposed

| id | What | Why it is not done here |
|---|---|---|
| **W5B-F1** | adaptive3d's planner claims removal on the boundary ring its emitted path does not deliver — 1360/2737 and 568/2737 cells, invariant under the stamp kernel, 98.7–100% one-sided. Standing candidate: `Cut` segments whose first emitted feed sweeps from the emitter's true tool position rather than the planner's raw `last_pos`. | A planner defect, pre-existing, unrelated to stamping. Pinned so it cannot grow; investigating it is its own item. |
| **W5B-F2** | Re-express F-027 / F-031 against the tool's **commanded Z travel per pass** — a quantity still bounded by `dpp` under both kernels — instead of against measured column removal. | Better sentry, bigger change than a landing lane should absorb. The split bar is the interim. |
| **W5B-F3** | Run the **acceptance corpus and the 56 param sweeps** under the new default and read the *geometry* diff on rest chains. | The metric re-baseline is the small part; the geometry change on `FromRemainingStock` ops is the part with no net under it. |
| **W5B-F4** | Re-tune the air-cut thresholds against the new measure — the CLI's 40% verdict, the GUI's 20% banner, and every per-operation band in `OperationType::air_cut_high_threshold_pct`. | Those bands were fitted to a reading that was 3–90% resolution artifact. They are now calibrated for a retired instrument. |
| **W5B-F5** | The **non-metric** replay `simulate_toolpath_with_lut_cancel` stamps `MoveType::Linear` regardless of intent (`simulation.rs:84`), so the metric grid and the playback grid disagree about retracts. | Pre-existing, not a swept-stamping question, and changing it moves `global_stock`. Logged by the decision package; still logged. |
| **W5B-F6** | The GUI viewport's chipload heat-map still colours by the retired per-move quantity. | Pre-existing (`CLAUDE.md`), unrelated, unchanged by this wave. |
