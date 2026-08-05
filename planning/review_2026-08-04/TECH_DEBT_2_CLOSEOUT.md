# Second technical-debt programme — close-out

Date: 2026-08-06
Wave: **W10** (plan §L1)
Basis: `TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` at `7a84472`; the factual
record is `ORCHESTRATION_LOG.md`, which this document indexes and does
not replace.
Branch: `experiment/adaptive-spiral`. Range: `7a84472..HEAD`, **132
commits**.

> **What this document is.** A per-item verdict, a record of every
> checkpoint ruling, the complete deferral ledger with owners, the
> provenance index for every number the programme produced, and the
> Checkpoint G live-validation plan. It is written to be read by someone
> who was not here.
>
> **What it is not.** It is not a re-statement of any wave's evidence.
> Every claim below cites the log entry or the sentry that carries it.
> Where a wave's own conclusion was later corrected, this document
> records the correction **and** the superseded claim, per the
> programme's errata discipline (plan §L1 preferred fix shape): prior
> numbers are never rewritten.
>
> **Checkpoint G has not happened.** §6 is a *plan*, not a report. No
> line of it may be read as a result.

---

## 0. Headline

The programme's own opening premise — "restore a trustworthy gate
baseline, then treat feeds/Suggest as the first system-wide census" —
held. Both halves landed:

- **The `cargo test -p rs_cam_core --lib` accepted-red allowlist is
  empty.** It was three tests when the programme opened and it is zero
  now, with `wanaka_suggest_baseline` (integration, environmental, plan
  §2 rule 9) as the single declared exception, owned outside this
  programme. Final: **2260 passed / 0 failed / 12 ignored**.
- **The chipload comparison no longer compares two different physical
  quantities.** B3's four disagreeing numbers were reconciled
  arithmetically, the unit question was answered from primary sources,
  and the conversion shipped with a measured verdict-flip table.

Three findings deserve to outlive the programme because they are about
*method*, not about this codebase:

1. **A gate handed no samples does not fail — it passes, and looks
   healthy.** D-impl-2 measured three gates returning `Within` on a
   population of zero, with `sample_range 0..0` and `available_kw 0.0`,
   indistinguishable on every surface from a measured clean cut. W5
   pre-registered that a *verdict* bar would pass vacuously here and
   demanded a *population* bar; it was right. This class is not closed —
   see §4 row **X-VAC**.
2. **A verdict that flips twice in one day was never resting on a
   settled band.** The B3 fixture went `Within` → `Exceeds(High)` (unit
   deletion, `0a45e35`) → `Within` (diameter law, `62ffd5c`) in four
   commits. Neither move was wrong; they moved different sides of the
   comparison. Any Ø≲2 mm chipload verdict is provisional until a bench
   measurement exists.
3. **Locating a residual is cheaper than explaining it, and must come
   first.** D-16.1's filed 235 µm was attributed by superposition
   (169 + 34 + ~30). Fixing the 169 µm term moved the total by 33 µm.
   A column-index probe then showed why: the fix and the defect are in
   **different bands**. Two waves of reasoning were replaced by one
   afternoon of instrumentation.

---

## 1. Per-item verdicts — plan items

Legend: **DONE** = the plan's acceptance gates for that item are met.
**DONE (scoped)** = the approved scope is complete but the plan item as
written was narrowed by a checkpoint, and the narrowing is recorded.
**PARTIAL** = shipped work is real and gated, named work is not started.

### H0 / R3 — end the three permanent adaptive3d reds — **DONE**

| gate (plan §H0) | verdict | evidence |
|---|---|---|
| baseline has zero *unclassified* adaptive3d failures | MET | `ADAPTIVE3D_RED_BASELINE.md`; W0 log entry; `d40768a` |
| after fixes, `--lib` has no accepted-red allowlist | MET | PR-1..3 log entry: 2232/0; today 2260/0 |
| no test converted to `#[ignore]` to clean the count | MET | measured: added lines carrying `#[ignore]` across `63d5e8b..HEAD` in `src` = **0** |
| parity tests compare same stock frame, toolpath, resolution | MET | `24c9a12`; both branches share one fixed 0.5292 mm grid |
| rapid test verifies geometry **and** intent | MET | `0b6478c`; intent read at the emitter, upstream of every dressup |

**Verdict.** All three reds were **one un-mirrored transform**
(`fa27b08` added a drape to `segments_to_toolpath` and nothing to
`stamp_emitted_segment`), bisected over 343 revisions in an isolated
worktree. Two were FIX_TEST (the drape had silently moved both fixtures
out from under their own guards — one guard had been vacuous, the other
had zero live coverage of a crash-class contract); one was FIX_CODE.

**Sentries:** `adaptive3d::path::tests::peck_plunge_progresses_…` +
`…_commits_the_cut_floor_not_the_retract_height`;
`…rapid_segment_lifts_to_safe_z_before_traverse` (two fixtures,
drape-inert and drape-active); `planner_sim_dexel_parity_{agent_search,
contour_parallel}` under one shared `assert_parity_bars`.

**Commits:** `d40768a` `14b0f70` `894e060` `0b6478c` `8853fdb` `24c9a12`.

**The re-registered bar is the part to remember.** The old bar gated an
*interior* count against a tenth of the *whole-grid* count — a nominal
10 % that was really 15.3 %, and 79 % spent at grant. It was restated
against the population it counts (20 % of 5184 interior cells) **and**
paired with a directional bar (`max(planner_higher, sim_higher) ≤ 2.5 ×
min`), because the green sibling's count *rises* across its own repair.
No count bar can distinguish those two states in the right direction.
The directional bar is the detector; the commit body says so.

**Cost, recorded so a later optimisation has something to beat:**
+15–45 % on the parity fixtures (`planner_sim_dexel_parity_agent_search`
2.20/2.81 s → 3.18/3.93 s; `…contour_parallel` 9.02/9.27 → 12.17/10.26).
W0's cheaper alternative was declined on **correctness**, not cost.

### H1 / R1 — feeds/Suggest census and one shared model — **DONE (scoped)**

| gate (plan §H1) | verdict | evidence |
|---|---|---|
| B3 fixture emits four named values and explains every delta | MET | `feed_explanation_snapshot_b3` (7); `feeds::explanation::FeedExplanation`, five labelled stages, each with its own `unit()` |
| parity across Suggest projection and gate-model projection | MET *on power*; **PARTIAL** elsewhere | `power_ceiling_parity_f2` (7). The gate's radial-width divergence (P-6, ~1.23× at 30 % radial) is untouched — §4 row **F-P6** |
| chipload/power/deflection share effective-feed selection | PARTIAL | power ceiling equalised (`684fc17`); a single shared feed-selection type was **not** built |
| tapered shallow-DOC matrix catches width-model regression | MET | the divergence was *measured not to exist* except on a truncated-tip V-bit (W3 continuation item 5); the sentry hole is §4 row **F-VBIT** |
| row ID / provenance / extrapolation / advisory / verdict / wording agree in one end-to-end fixture | MET | `chipload_report_wording_t12_t15` (6); `LookupResult::row_pass_role` |
| literature-matrix green; stale citations refreshed not replaced | MET | `literature_matrix` 21; six URLs replaced **by retrieval**, two by renaming a misattributed key |

**What was actually decided.** The census's headline — "four disagreeing
chipload numbers" — resolved to **three stages of one quantity plus one
different quantity wearing the same label**. The B3 identity closes at
**+0.0000 %** through shipped code. The gate was comparing a mean chip
thickness in mm against a vendor advance per tooth; a literature wave
(`CHIPLOAD_LITERATURE_VERDICT.md`) established from primary sources
(Onsrud, Freud, Amana, Garr, IDC — verbatim formulas, one numerically
self-verifying chart) that the vendor column is **advance per tooth**,
confidence HIGH for five of seven families.

**The correction was a deletion, not an inversion.** `cl_norm = fz ·
f_lut`: the sample's own arc cancels, so dividing by `f_lut` recovers
`fz` exactly and the D9 renormalisation + chip-geometry step were
removed rather than inverted. The `ae` bands `f_lut` is built from are
**repo-authored** (72 of 76 `ae`-bearing rows carry an ingest-authored
window, not a vendor rule) — which is why deletion is strictly better
than inversion.

**The flip table, as ruled** (`0a45e35`): B3 `0.000737 mm` (20 % of band
minimum, `Within` + burn advisory) → `0.009153 mm/tooth` (127 % of band
maximum, `Exceeds(High)`). A second flip ran the other way and is a
safety finding: `chipload_formula_calibration`'s "safe" 0.18 mm/tooth
was **3.27× the band maximum** and the pre-fix gate said `Within`.
Pinned permanently as
`the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum`.

**Then the diameter law moved the other side.** `feeds-final`
(`62ffd5c`) adopted `D^0.61` and `Janka^-0.5`; on B3's Ø0.954-against-a-
Ø3.175-row query the band moves ×1.598 and the operation returns to
`Within` at 79 % of max. **These are not a revert of each other** — the
deletion moved the *observation* by 12.43×, the law moved the *band* —
and both records stand. See §0 headline 2.

**The rubbing floor.** `RUBBING_FLOOR_MM_TOOTH = 0.025` was clamping
Suggest to **3.47×** the matched row's derated band maximum: a
protection against rubbing was commanding a chipload the gate's own
envelope reads as breakage-side. Now `min(floor, derated_band_max)` as
`feeds::effective_rubbing_floor`, with the bare constant retained where
no band exists. When no feed can both clear chip formation and stay
inside the window, `FeedsWarning::ChiploadClampedToFloor` carries
`band_capped_from` and all three renderers say so in words.

**The order mattered and was ruled.** Floor first, then exponents:
`_litmatrix_rubbing_floor_clamp` pins the floor × band *interaction*,
and the two moves **cancel to the byte** on the Ipe cell. Had the laws
landed first, that cell would never have exercised the subordinated
branch and the floor fix would have shipped with no fixture proving it
fires.

**A prediction published as failed.** `LAW_MAGNITUDE_TABLES.md` §5.1
predicted `ipe_pocket_emits_chipload_clamped_warning` would go RED. It
did not: Step 9b tests the commanded feed-per-tooth *after* the safety,
LD-overhang and power derates (0.021982), not the band midpoint.
Direction right, conclusion wrong; recorded in the file's docstring and
in a banner on the document.

**The mandatory rider, and it was worth shipping.** `is_extrapolated`
was computed on the *applied* scale; softening exponents would have
silently un-flagged rows, downgrading `Approximate` → `Validated` and
converting advisory low-side reports into hard `Exceeds(Low)` trips.
Measured: **932 of 11 712 (query, row) pairs = 8.0 %**. Moved to the raw
ratios first (`6e19461`, red-first) — after which adopting the exponents
changes **no** flag at all.

**Commits:** `2823d71` `8285497` `5f7bb25` `6bc856f` `0c642e0`
`89dccbf` `684fc17` `e497ef9` `5b8667d` `09f10b0` `b299063` `6e19461`
`0a45e35` `08e7c88` `2d1bfc8` `62ffd5c`.

**Scoping, stated.** Plan §H1's "one shared pre-/post-simulation model"
was **not** built as a single type. What shipped is: one *answer* to the
unit question, one *record* (`FeedExplanation`) that names every stage
between the gate's number and the commanded one, and parity on the one
numeric ceiling that lacked it. The remaining divergences are ledgered
individually in §4 (**F-P6**, **F-T35**, **F-VBIT**, **F-BIPOLAR**,
**F-HEATMAP**), each with an owner. Calling this "one canonical physical
model" would overstate it; calling it unaddressed would understate it.

### H2 / R7 — structural hygiene — **DONE (scoped)**

Seven sub-items. Six shipped; one package was **not attempted** and says
so.

| sub-item | verdict | evidence |
|---|---|---|
| H2.1 findings transport (B7 filing) | **DONE** | `41cb426`; `findings_transport_join_h21` (6) |
| H2.2 arcfit intent boundary (B5) | **DONE** | `3dbec75` `5fad7e2`; `arcfit_intent_boundary_f1` (5), `arcfit_intent_key_cost_f1` |
| H2.3 exporter span-walk (D-LV.1 / X-1) | **DONE** | `e568ea2`; `exporter_span_classifier_x1` (8) |
| H2.4 D-16.1 band run-off | **DONE on mechanism; quality claim OPEN** | `fb22888` `0d9bfdc`; `band_run_off_reproduction_d16_1`; §4 row **D161-Q** |
| H2.5 D-16.2 `stock_to_leave` | **DONE, wider than filed** | `921c94e` `bbc443d`; `shallow_band_stock_to_leave_exhibit_d16_2` (4) |
| H2.6 agent-read paths | **PARTIAL — C1/C6/B7 done, C2–C5 not attempted** | `e147dcd` `2117b15` `7941379` `66507e4`; §4 row **C25** |
| R7-L1 clip-visibility census | **RESEARCH ONLY, nothing written** | W8 log entry; §4 row **L1-CLIP** |

| gate (plan §H2) | verdict |
|---|---|
| synthetic `GenerationFindings` field fails compilation in every required adapter | **MET, and proven on the parent first** — the probe was run at `d40768a` where both checks built clean with no warning; after the fix it is `E0027` at `compute/stats.rs:141`. The defect is reproduced, not inferred |
| mixed `FinishingCut → LeadOut → Entry` fixture produces no boundary-spanning arc; homogeneous arcs still fit | MET |
| arcfit re-pin package includes direct/indirect consumers incl. `arc_raster` | MET — exactly **one of five** transform-provenance constants moved, by **label only**, count flat at 40 |
| D-16.1 fixture has non-empty rendered run-off population, cannot pass on a convex fixture | MET |
| D-16.2 red-first, only `stock_to_leave` differs, MidSteep/Ball controls preserved | MET |
| screenshot regression demonstrates D-LV.1 red on the parent | MET **by substitution, disclosed** — the parent walk is transcribed verbatim into the sentry as `parent_revision_exporter_walk` (a shared tree made a checkout unsafe); strictly stronger, since it keeps failing if anyone reintroduces the reversed walk |
| long-generation cancel/status <1 s while narration/parameter reads are tested | MET, and the guarantee was **widened** — C6 makes the five cheap reads answer in <1 s *independent of lane activity*; the old sentry pinned the narrower behaviour as correct and was rewritten deliberately, in writing, under the F3 ruling |

**H2.2 is the one whose result was surprising.** The measured arc-count
cost of adding intent to arcfit's run key — strict `Unknown` included —
was **ZERO** on all five fixtures (23/3, 40/8, 74/13, 32/8, 32/8 before
and after). The old key was not buying arc count at the boundaries it
violated; at every one of the seven strict-`Unknown` seams, some other
term was about to break the run anyway. What it *was* buying was a wrong
label — and in one fixture a `FinishingCut` arc that overshot the
machined surface by 1.18 mm.

**And it closed its own discoverer.** `scallop_intra_pass_relink_am7`
had attributed "21 disappearing cut positions" to `relink_fragments` for
a whole wave of the *previous* programme. After the fix: **21 → 0**,
links unchanged at 21, 2427 of 2427 surviving cut positions on the
drop-cutter surface. The 1:1 arithmetic was entirely this defect.

**H2.4's honest result.** The mechanism claim is closed: `ring_to_3d`'s
coverage guard read the generation heightmap (0.75 mm cell) with
nearest-cell rounding instead of the exact point-in-triangle predicate
the Shallow band already used. Off-footprint cutting targets **131 of
3287 → 0 of 3133**; worst distance past the footprint **0.3750 mm →
0.0000 mm** (0.3750 is half a generation cell to four decimals, which is
what identified the mechanism). The **quality** claim is not closed:
arm B still overcuts by **−201.6 µm** with zero targets outside the
footprint. F23-impl declined to call a ~200 µm remainder a rounding
error, and W10's probe (§3) says why. What the fix does buy is measured:
arm B **−12.5 % runtime**, mm²/s +10.9 %, retract trips 10 → 4, arm C
−28 % with trips 58 → 20, collisions **0** on all three arms at 0.1 mm.

**A crash-class defect was found on the way and fixed first.**
Sharpening the guard moved a relinked fragment and
`no_new_collisions_at_the_finest_resolution` went 0 → 1. Instrumented
rather than re-pinned: `apply_lead_in_out` inserts the lead-out arc
between the last cut and the closing retract and copies the retract
*verbatim*, so a retract written as a pure vertical lift becomes a
diagonal one travelling backwards over the finished surface. Latent
since the relinker shipped. Fixed in its own commit **before** F2, so
the safety gate never went red (`268e427`).

**H2.5 was wider than filed.** The filing named Shallow; **VerySteep
drops the dial too** (`WaterlineParams` had no field to thread), so
fixing Shallow alone would have created a `stock_to_leave`-sized step at
the shallow↔waterline seam that does not exist today. Ruled and fixed
together. The byte-identical-at-0.0 claim was **run, not reasoned**: the
35-binary census executed both ways, `crease_own_region_pr6b`'s
`UnifiedFinish` fingerprint `(1464, 0x655861151cebaee4)` in both.

### H3 / R2 — adversarial 2D campaign — **DONE (scoped)**

| gate (plan §H3) | verdict | note |
|---|---|---|
| every 2D operation has ≥1 applicable adversarial fixture + non-vacuity assertion | MET | 22 fixtures / 11 hostile classes / 9 families; `fixture_mechanisms.md` is the measured non-vacuity table |
| a test fails if an operation returns success with an unexpected empty/early path | MET | `MustCut` contract in `adversarial_2d_campaign_r2` |
| iterative cascades have bounded termination + wall-clock ceiling + cancellation test; memory recorded | MET | F-10 stops with `stopped_by == RingCount` (asserted — a wall-clock stop would make the bound machine-dependent); 22.9 GB recorded |
| analytic offset rings within pre-registered erosion/containment tolerances | MET | `common::offset_lab` |
| concave/reflex fixtures prove they contain the mechanism before gating | MET | and one fixture was **corrected against itself** — `holes_in_holes` alternated ring winding, making it a second invalid fixture wearing a `Valid` label |
| any arc-carrying cascade migration uses one declared `FlattenPolicy` | **N/A, answered** | rollout matrix: there is **no remaining cascade to migrate** |
| broad path changes pass relevant family sweeps with fingerprint explanations | MET | `param_sweep -- --ignored` **56/56** after each of the three behavioural commits — the pre-registered STOP that never triggered |

**The central premise held and is now evidenced:** the 2D stack's
failure mode is not wrong geometry, it is **unreportable** geometry. A
library panic, a `<3`-vertex guard and a genuine collapse were one
indistinguishable `Vec::new()`.

**Shipped (Checkpoint C, shape B):** `offset_polygon_reported() ->
(Vec<Polygon2>, Option<OffsetFailure>)` distinguishing `Collapsed` /
`RejectedInput` / `LibraryFailure`; the old name delegates and is
byte-unchanged; four families opted in; a 13th `ToolpathStats` slot
`offset_library_failures` plus a 14th `boundary_clip_dropped`, both on
the `None` = *not measured* contract.

**F-1 was reproduced end-to-end, which W4 recorded as not done.** On a
boundary whose offset fails, the pre-fix code returned the toolpath with
its move count unchanged and a cut **500 mm outside** a
`ToolContainment::Inside` containment surviving as a *cut*, not a
retract. The boundary layer is the one place an empty offset is an
**over**-cut — everywhere else it is an under-cut — and it is the one
place the ruling made it a refusal. That asymmetry is the finding.

**Three panic classes, two previously unknown**, all `debug_assert!`s:
`pline_view.rs:507` (R1's known class), `static_aabb2d_index:266` (a
**transitive** dependency R1's census never covered), and
`pline_seg.rs:33` — reached from **`rosette-24`, a fully valid
fixture**: CCW, 720 finite vertices, no self-intersection. Release
behaviour is `NOT EXERCISED`, never `PASS`; Checkpoint C Q4 accepted and
documented the divergence and scheduled the measurement into §6.

**Also:** all nine families now honour a pre-set cancel flag (`ignored`
is empty). **A finding this campaign withdrew** is preserved in the
test's own docs: an early run reported five cancel-ignorers with a real
mechanism to explain it; the harness was racing itself
(`run_op_with_cancel(.., ZERO)` still arms a timer thread). The
mechanism was real; the conclusion was an instrument artefact. A
near-miss that leaves no trace teaches nothing.

**Not met, stated:** **165 of 198** matrix cells were never run. That is
recorded as `NOT RUN`, not as passing — §4 row **A2D-165**.

### M1 / R4 — simulation issue channel and measurability — **DONE**

| gate (plan §M1) | verdict | evidence |
|---|---|---|
| an all-air trace cannot create an unbounded user-facing list | MET | 6000 air samples → capped, `truncated == true`, `total_matching` = true pre-cap count |
| one rapid collision + one holder collision + one removal warning stay visible among thousands of air samples | MET | `safety`/`actions` share no list with advisories |
| dedup preserves worst evidence, never hides a distinct safety event | MET | two collisions 0.5 mm apart stay two entries |
| a fixture at cell above cut depth reports `NotMeasurable`, rapid-collision detection still live | MET | `measurability_abstention_r8` |
| GUI, MCP, CLI and narration consume **one** typed summary contract | MET | `ProjectSession::simulation_triage` is the single construction site |
| existing gates retain semantics until a checkpoint approves a change | MET | no gate threshold value moved |

**The plan's premise needed one correction, and it changed the fix.**
The channel is *not* dominated by "every out-of-material sample" — core
run-length-encoded that in April. The all-air arm is the **cheap** case
(20 328 samples → **42** segments); an ordinary pocket is the expensive
one (14 294 flagged → **759**). Transition density in ordinary cutting
is the driver. A cap plus a spatial dedup addresses that; more
contiguity coalescing does not. Implementing against the measured
decomposition rather than the written premise changed the design.

**The measurability floor is the operator-facing finding.**
`FRESH_MATERIAL_THRESHOLD_MM = 0.05` gates the radial-engagement
measurement, so a 0.02 mm-deep pass reads **air 95.9 %, peak radial
0.0000, avg engagement 0.0000** while removing **63.7 mm³** — a hard
zero dressed as a percent, clearing every shipped bar. `NotMeasurable`
gates now **abstain with a stated reason**; collision detection stays
live. The brief's "cell < cut depth" framing resolves into **two
independent conditions**, and neither is that one.

**B4 / A/L2 is closed as diagnosed.** The Rivers 6.07 mm axial-DOC spike
reproduced on a **committed** fixture at 20.4× (4.0722 mm on a commanded
0.200) — same class as wanaka's 15×. Transit/lift-bridge/source-role
**refuted** by four independent structural tags; upstream stock coverage
**confirmed** by direct reading (5.20 mm standing = the rough's
`stock_to_leave_axial = 5.0`). It is a planning-and-reporting condition,
not an engine defect, and R-12 gave the actionable half its first
channel: *this pass is cutting through what the rough left*.

**Arcfit was exonerated for the fourth time**, and the real
gate-population defect was found next door: `SpanKind::DressupArtifact`
(not `MoveIntent`) dropped **every fitted arc** from gate peak/LUT
populations. Over-broad **by construction, not by decision** — the term
landed at `7d01311` naming four kinds as one list, and the dogbone
rationale never mentions arcfit. Fixed by a new `SpanKind::GeometryRefit`
rather than by label-keying, because the defect **is** a type
conflation; adding the variant broke seven exhaustive matches, which is
the argument for it.

**Commits:** `0e1c996` `c03227f` `ebe77de` `8cbc691` `673ab8a`
`9d8b095` `197d5ef` `e6d4644` `f8cf721` `f8a60f2` `69fa1ab` `142bd17`
`f58409f` `6b6b025` `a7c3389` `d71253c`.

### M2 / R5 — drill evidence and literature re-validation — **DONE**

| gate (plan §M2) | verdict |
|---|---|
| a fixture outside each low/high gate produces `Exceeds` or a documented advisory **matching the evidence** | MET — this is the class C-1 broke and it is now sentried on the *string*, not only the severity |
| a `Within` verdict cannot display an observed value outside a hard bound without an explicit weak-provenance explanation | MET |
| literature-matrix drill sentries + freshness report pass; changed source IDs documented in `CREDITS.md` | MET |
| drill hole/peck role selection uses `RegionSpanRole`, never labels | MET — `Span::has_region_role(RegionSpanRole::DrillPeck)` |
| no drill policy/default/threshold moves before Checkpoint D | MET — and none moved after it either |

**The money finding, shipped and operator-visible:** `Chip welding (D/d)
exceeds: 7.33 vs 8.00` at `Severity::Caution` — an *observed value
strictly below the threshold it names*, because chip welding's
`Elevated` band `[0.75t, t)` mapped to `DrillGateOutcome::Exceeds`,
whose `threshold` field is documented "The violated bound". Three more
contradictions of the same shape: a `Within` reading advertising 26 %
headroom against a real 1.7 %; two gates able to fire on one hole with
contradictory remedies ("reduce peck depth" on a cycle with no peck
depth, beside "switch to a peck cycle" on an op already pecking); and
narrate printing *"peck pattern INADEQUATE"* two lines from *"mean
chip-evacuation score 1.00 (0=trapped, 1=cleared)"*.

**A model defect, corrected — and the audit's own arithmetic corrected
with it.** `peck_descents` rooted the peck grid at `hole.top_z`; the
emitter roots it at `retract_z` = `stock_top + 5.0` always. The audit
predicted a 33 % cycle-time understatement; **measured off the emitted
toolpath it is 41 %** (17.0 mm / 3.400 s, because `drill_peck_full_
retract` re-feeds the 0.5 mm re-entry clearance after every retract).
`peck_count` 4 → 5, `feed_time_s` 2.000 → 3.400. Fixed by one shared
`drill::fed_descents` — *making the knowing side publish*, not teaching
the second interpreter to re-derive.

**The paperwork was worse than the code.** `onsrud_drill`'s URL is a
404; the live chart was retrieved (756 398 B) and **contains none of the
data cited to it** — no peck, no D/d, and a wood band (0.229–0.432
mm/tooth) that does not overlap the matrix's 0.08–0.18 at any diameter.
The matrix band instead brackets *the code's own output* to three
decimals: it was fitted to the implementation and then cited. FPL Wood
Handbook was retrieved in both editions and contains **zero** drilling
content (all 26 "peck" hits are pecky cypress). **17 of 32** citation
URLs did not resolve, 9 of them gold tier; `DEFAULT_TODAY` equalled the
seed date of 30 of 32 rows, so the freshness clock could never say
`stale`.

Six URLs were repaired **by retrieval**; two misattributed keys were
renamed (`fanuc_kc_table` → `sumitomo_kc_table`, `ineos_hdpe_machining`
→ `simona_hdpe_machining`) after confirming no public Fanuc appendix and
no INEOS machining guide exist. Negative results were recorded rather
than papered over — Whiteside publishes no chipload card at all, and its
`gold` tier is stated as unsupported by any retrievable document. The
clock now runs: the same rows reach `warn` in June 2027 with no edit.

**No threshold moved**, per the ruling: R-8/R-9/R-10 keep their values
and are now **declared repo-authored** in code and in `CREDITS.md`. The
"3–8×D" figure was a total-hole regime number used as a per-peck
ceiling; moving it down onto the metal-twist-drill convention
(0.5–1.0×D, 6–12× shallower, with coolant) would be the same category
error in reverse.

**Commits:** `DRILL_GATE_EVIDENCE_AUDIT.md`, then `184b418` `fd68a36`
`c5e44a6` `0afb51b` `a45f326` `7f6d8fc` `54207cf` `0e6efde` `f99dd99`
`8e9dc6f` `9c340b6` `dcd802c` `ef81c39` `4dd8b93`.

### M3 / R6 — reference fixture, rest anomaly, mega-harness policy — **DONE (scoped)**

| gate (plan §M3) | verdict |
|---|---|
| procedural reference geometry with analytic normal/surface truth; tessellation error below the smallest adopted bin | MET — worst zone tessellation p99 **4.54 µm** at ε = 1 µm against a **50 µm** alias floor at the finest cell anyone would run |
| every fixture claims the mechanism it can exhibit, with a non-vacuity test | MET |
| repeatability bins established from **repeated runs**, not a desired dial | **NOT MET — and deliberately so.** No bin is adopted. B-2 (regeneration variance) is the arm that sets it and is `NOT RUN`; its body panics with the bar it must implement rather than passing silently |
| `terrain.stl` stays available as characterization; no fine-quality winner gate depends on it | MET |
| `rest_grid_resolution_c9` stays green until a replacement explains it | MET — unchanged and untouched |
| every mega harness has a named owner, action and cadence | MET |

**The anomaly's headline is wrong, and the prediction that falsified it
was W7's own.** *DETECTED AND REFUSED.* At the tip scale, the 0.10 mm
cell finds **95.8 %** of the shipped cell's skeleton and routes **every
millimetre** of it away (`centrelines/skeleton mm/routed away/clearing
regions`: 0.50 → 1/19.000/0.000/0; 0.25 → 0/18.500/18.500/1; 0.10 →
0/18.200/18.200/1). Refinement does not find *less feature* — it
**routes** the same feature differently. Every downstream statement of
the form "the fine grid detects nothing" must be restated. The sentry
counts `rf.centerlines`, which is **post-routing**, and reads neither of
the two fields that already carry the distinction.

The wide control's routing **never changes** while its median reach
collapses 0.5827 → 0.2417 → 0.0000, which confirms "two anomalies, not
one" and locates the second entirely in the cross-section/reach stages.

**A second W7 prediction was refuted** and published as refuted: §7 arm 3
predicted `dX/d(rim) = 1` exactly; measured, angle-preserving is
**1.000000000** and rise-preserving **0.6753/0.6674**. The study's
number survives; its derivation does not.

**ARP-1 shipped** (`reference_plate.rs`, 96×96 mm, 16 zone instances
over 9 kinds) with parity against the Python reference on **every
published number** — band areas, iso-slope circles, all 18 reach floors,
all 5 trough radii, all 6 κ, the 39× XY-vs-arc penalty (measured 38.9×)
and the alias table. **One divergence found in the reference, not the
implementation:** `ConeAnnulus.kappa_max` returns `cos θ/r`; the correct
value is `sin θ/r`. Blast radius **none** — the cone is absent from the
Python's case list. Two defects in the new generator were caught by its
own gates before commit (rotated tiles overlapped at φ = 20°; a gutter
ring hard-coded to `z = 0` painted flat ground over a neighbour's
relief).

**The useful conclusion is not that ARP-1 is better — it is that the
fixture stopped being the limit.** Tessellation error is an order of
magnitude below the alias floor, so effort belongs on the instrument's
cell. And VerySteep is **non-reportable at every cell down to 0.01 mm**
(37 µm floor at its most favourable angle vs a 20 µm interesting
difference); the only combinations clearing a 20 µm bin are shallow
ground at ≤ 0.02 mm, and nothing in this repo has ever simulated finer
than 0.1 mm.

**The dated TIN erratum.** *"terrain.stl: 1.8 % of triangles carry
40.8 % of area"* — cited in the plan, `RESEARCH_COMMISSION.md` and
`SUPERSEDED_CONCLUSIONS.md` — **does not reproduce**: 1.8 % carry
**57.0 %**, and 40.8 % sits on **99 triangles (0.05 %)**, dominated by
the base box. Corrected under a dated banner in all three, per errata
discipline; the prior numbers are preserved, not rewritten. The
**stronger** objection is stated in the same banner: the shipped scallop
stepover 0.600 mm equals the fixture's p99 relief facet edge 0.626 mm —
facets and cusps are the same size in the same places.

**P7 discharged:** `p2c_headless_ab_wanaka` and `v3_cascade_ab`
SPLIT→ARCHIVE with reusable loaders extracted (and the hard-coded
`/home/ricky/Downloads/...` path recorded — a 5066-line harness that
cannot run on any other checkout, undetected because it still compiled);
`strategy_comparison_h4` SCHEDULED with the finishing lane as owner.

**Commits:** `1c627ed` `c04724d` `3a6b7e5` `3af0d97` `2a3c6d8`
`afae0cd` `b04387a` `6f85b8e` `3404b02` `8d92056` `1be1ff9` `559f90c`.

### M4 / R8 — bounded untouched-territory scouting — **DONE**

| gate (plan §M4) | verdict |
|---|---|
| every listed risk has a concrete code location + reproduction hypothesis | MET — 20 risks with `file:line:symbol`, 12 ruled out with evidence |
| a Default/serde audit names **both** values and the load path | MET |
| **no** feature or schema change lands from scouting | MET — zero production/test/schema change, zero Cargo commands |

Disposition: **3 fix now · 17 next-programme intake · 12 ruled out**.
The three were escalated and two were later approved and executed by a
separate lane (see below); P-3 was **downgraded to `ruled out (no live
population)`** rather than fixed, which is the right outcome for a risk
whose population is empty.

**The interpretation is the deliverable.** Three of the four areas' top
risks are one pattern: *a value has two interpreters, one complete and
one incomplete, and the incomplete one is on the production path.* Same
shape as D-LV.1 and the `intra_region_hookup_mm` prior — which argues
for a class fix (single resolver / single classifier), and that is
exactly how P-1 was then fixed.

**P-1 and P-2, ruled 2026-08-06 and executed** (`a80020a`, `a092d1a`):

- **grblHAL survived on disk and nowhere else.** Two writers emit
  `"grblhal"`; only the job-file path knew how to read it back, so both
  project-path readers silently downgraded a user-selected grblHAL post
  to GRBL. Measured by reverting each in turn: a grblHAL project
  exported `(WARNING: M7 unsupported on GRBL; dropped: M7)`. **A fourth
  reader the risk map missed** was found by `rg` and routed through the
  same resolver. The typed serde wire also disagreed with the project
  writer (`"grbl_hal"` vs `"grblhal"`) and would have **rejected** the
  file; both spellings are aliased and **serialisation output is
  unchanged**, pinned by `the_written_post_tokens_are_the_shipped_
  spellings`.
- **The setup datum is operator intent and it now reaches the file.**
  Consolidated rather than mirrored: `Corner`/`XYDatum`/`ZDatum`/
  `DatumConfig` existed as two byte-identical private copies in viz and
  are now re-exports of one core definition; `SetupRuntime` is deleted
  outright (−374/+589). `model_ids` **persist, not derive** — empty
  means "all models", which is not the same statement as an explicit
  list, and neither is recoverable from the toolpaths. The join exposed
  three live defects fixed in the same commit, including a model-scope
  checkbox that pushed no event and so never set the dirty flag —
  harmless while it wrote a throwaway overlay, a way to lose an edit
  once it writes the project.

### L1 — documentation, provenance, benchmark hygiene — **DONE** (this wave)

| gate (plan §L1) | verdict |
|---|---|
| every behavioural PR contains a mechanism and before/after evidence pointer | MET — audited across all 132 commits; the two exceptions are named in §4 rows **HIST-1** and **HIST-2** and neither is a missing mechanism, both are attribution artefacts of a shared working tree |
| no planning/capability document promotes a strategy/quality winner without qualified fixture, two-fixture comparison, source population and render | MET — **and the programme published no strategy winner at all.** Checkpoint E ruled B1/B2 stay closed |
| new external source/formula work updates `CREDITS.md` | MET — drill lineage added; both scaling exponents declared **repo-derived, published by nobody**, in those words |
| no `NOT FIXED` line omits an owner and evidence condition | MET — §4 is the audit; **78 rows, every one carrying an owner and a re-open condition** |

Deliverables: this document; the stale-rationale sweep (§5); the
provenance index (§7); the Checkpoint G plan (§6).

---

## 2. Per-item verdicts — inherited ledger rows (plan §1.1)

| ledger | item | programme item | verdict | evidence |
|---|---|---|---|---|
| **B3** | four disagreeing chipload numbers | R1-H1 | **RECONCILED then CORRECTED** — identity closes at +0.0000 %; the unit was answered from primary sources and the gate-side factor deleted | `feed_explanation_snapshot_b3`; `CHIPLOAD_LITERATURE_VERDICT.md`; `0a45e35` |
| **B4 / A/L2** | Rivers 6.07 mm axial-DOC spike | R4-M1 | **CLOSED AS DIAGNOSED** — reproduced at 20.4× on a committed fixture; upstream coverage confirmed, transit/source-role refuted, arcfit exonerated (4th time). Reporting item R-12 shipped | `simulation_issue_channel_m1`; `69fa1ab` |
| **B5** | `arcfit` intent inheritance | R7-H2 | **FIXED** — cost measured ZERO; one FNV moved, label-only | `3dbec75` `5fad7e2` |
| **B7** | GUI worker hand-copies findings | R7-H1 | **FIXED, and the filing was partly refuted** — both narration literals were already exhaustive over the same 24 fields; "plus `retract_trips`" is REFUTED. But **four fields differed in their value expression**, worst of which is viz falling back to `tools().first()` and narrating the **wrong tool's geometry**. Fixed red-first | `41cb426`; `7941379` |
| **D-16.1** | UnifiedFinish band run-off | R7-H3 | **MECHANISM FIXED; QUALITY CLAIM OPEN** | `fb22888` `0d9bfdc`; §3; §4 row **D161-Q** |
| **D-16.2** | Shallow ignores `stock_to_leave` | R7-H4 | **FIXED, WIDER THAN FILED** — VerySteep too | `921c94e` `bbc443d` |
| **D-LV.1** | `screenshot_toolpath` exporter omits emission | R7-M2 | **CLASSIFICATION HALF FIXED; near-empty symptom NOT REPRODUCED and not claimed** | `e568ea2`; §4 row **DLV1-NE** |
| **`rest_grid_resolution_c9`** | refinement finds less / reach collapses | R6-M3 | **HEADLINE REFUTED** — detected and refused; two anomalies, not one; owner re-pointed from `measure_cross_section` to ridge-location/`box_smooth_rest` | `rest_routing_probe_e9`; `REST_GRID_ANOMALY_STUDY.md` |
| **VerySteep clip limitation** | MidSteep/Shallow partial clips invisible | R7-L1 | **CENSUSED, NOTHING WRITTEN — and the prior claim corrected**: MidSteep and Shallow are **not height-clipped at all**, so widening `BandHeightClip` to them would report a structural zero and be wrong | W8 log entry; §4 row **L1-CLIP** |
| **cavalier `Shape` panic** | panic → silent collapsed offset | R2-H2 | **FIXED (debug); release NOT EXERCISED and scheduled** | `693579c`; §6 step 8 |
| **Criterion baseline absence** | offset/classification regressions lack a baseline | R2-M2 | **PROTOCOL ESTABLISHED, NO NUMERIC CI THRESHOLD** — as the plan asked. `param_sweep` 56/56 is the repeatable characterization; the C6 bit-identity gate proves fixture donors unmoved | `param_sweep`; `c6_donor_bodies_are_bit_identical` |
| **P7** | three ignored mega-harnesses rot | R6-M2 | **DISCHARGED** — 2 SPLIT→ARCHIVE, 1 SCHEDULED with a named owner | `3404b02`; `MEGA_HARNESS_POLICY.md` |
| **B1/B2** | v3 closure, scaled/cascade invariance | deferred to R6 checkpoint | **STAY CLOSED** — ruled at Checkpoint E. The repeatability pass may bank evidence for a future programme; no re-measurement re-opens here | Checkpoint E Q2 |

---

## 3. The one-column probe — W10's own measurement

`FINISHING_OPEN_DEFECTS_EVIDENCE.md` §7.6 wrote D-16.1's re-open
condition word for word, and this wave ran it. Research only.

**Instrument:** `d16_1_single_column_contact_probe` in
`crates/rs_cam_core/tests/strategy_comparison_h4.rs` (`#[ignore]`d;
non-vacuity bars only, gates nothing). Commit `3d10f7b`. Transcript:
`planning/review_2026-08-04/artifacts/w10/d161_contact_probe.txt`.

```text
cargo test -p rs_cam_core --test strategy_comparison_h4 \
    -- --ignored --nocapture --test-threads=1 d16_1_single_column_contact_probe
```

**Fixture / population / resolution.** `grooved_block(2.5, 70°, 1.2)`,
the project's own Ø1-tip / 7° / Ø6-shaft taper (envelope r 3.0, cusp
r 0.5). Arm B (`UnifiedFinish`, shipped dials) is **generated once and
not simulated** — 13 429 moves. There is no resolution parameter in this
probe at all; that is the point of it. Debug build, 10.7 s.

### 3.1 Fact — the named move and column reproduce exactly

`move 4841` is at **(2.5000, −7.5000, −0.0000)**, `Linear` /
`FinishingCut`, and it is also the nearest cutting move to the named
column (0.1000 mm away). The io-fixes attribution — §7.4's "the raster
pass is cutting on the break line itself, not near it" — holds against a
fresh generation. Generation is deterministic and the pinned index still
points where it was said to point.

Its **commanded Z equals the analytic rim plane to 0.0 µm.**

### 3.2 Fact — the drop-cutter is exonerated, and §7.5's hypothesis is refuted

At the named column (2.5000, −7.6000), the query returns 1344 candidate
triangles of which 887 contact, and the answer is `cl z = −0.000000`
against an analytic surface of `0.000000`. **Two triangles tie for the
win, both by facet contact:**

| face | zone | n·z | tri x range | covers XY | facet | cl z |
|---|---|---|---|---|---|---|
| 1504 | wall (70°) | 0.3420 | 2.450 – 2.500 | yes | yes | −0.000000 |
| 1507 | rim (flat) | 1.0000 | 2.500 – 2.550 | yes | yes | −0.000000 |

§7.6 asked *"is the winning triangle the rim rather than the wall?"* The
measured answer is that the question has no single answer at this column
— both win, exactly — **and it does not matter, because both give the
same exact Z.** The dump at the named move's own XY (2.5000, −7.5000) is
the same two faces, the same tie, the same exact value.

**§7.5's stated mechanism does not happen.** It predicted that "a
drop-cutter contact evaluated on the rim triangle alone puts the tip
below the rim plane there". The rim triangle alone gives exactly 0.

Across a supplementary 41-point sweep of the break at the same y (x from
1.500 to 3.500 at 0.05 mm), the drop-cutter **never once** puts the tip
below the analytic surface: worst 0.0 µm. On the wall it correctly rides
the corner — at x = 2.400 it returns −0.010102 against a surface at
−0.274748, because the Ø1 tip cannot enter — and on the rim it returns
exactly 0.

### 3.3 Verdict, and the one lead it leaves

**The −201.6 µm residual is not a drop-cutter contact error and not a
commanded-Z error.** At the column carrying the worst overcut in the run,
the planner commands the exactly correct height, computed from a contact
evaluation that is exact. Two of §7.6's three possible outcomes are
therefore closed: the hypothesis is dead, and the `Shallow/raster`
attribution from §7.4 stands and is now corroborated on a fresh
generation.

**One arithmetic observation, recorded as a lead and explicitly NOT as a
finding.** The sweep shows the tip riding the rim corner descends
exactly **0.200000 mm** below the rim plane at x = 2.100 — 0.4 mm inboard
of the break, which is `0.5 − √(0.5² − 0.4²)` on the Ø1 tip, and the same
figure appears in the dump as the contribution of every rim triangle
0.4 mm away. The filed residual is **0.2016 mm**. Whether that is the
same 0.2 mm arriving at a rim column by some other route, or a
coincidence at two significant figures, is **not measured here** and must
not be reported as though it were.

What it does is move the next question off the planner. The remaining
candidates all have the shape *"what attributes a tool position over the
wall to a column on the rim"* — swept-volume interpolation between
successive commanded points, or the stamping footprint — and none of them
is a drop-cutter question.

**Incidental, noted not fixed.** The candidate query uses
`cutter.radius()` — the Ø6 **shaft** — so 1344 triangles are fetched and
887 contact where the 0.5 mm tip could reach a handful. The answer is
correct (a max is a max) and the cost is not this programme's: it is the
`radius()` / `cusp_radius()` split the radius programme owns.

**Owner and re-open condition** are recorded in §4 row **D161-Q**, and
they are now narrower than §7.6's: the drop-cutter path is excluded, so
the next probe should instrument the swept-volume/stamping attribution at
this same column rather than the contact evaluation.

---

## 4. Deferral ledger — NOT FIXED / NOT RE-RUN

Every row carries an **owner** and a **re-open condition**, per plan §L1's
acceptance gate and §2 rule 15. "Owner: unassigned" is used only where no
lane can honestly be named, and in those rows the re-open condition is
what makes the row actionable. **78 rows.**

Rows are grouped by subject, not by wave; the wave that filed each is in
the evidence column so the log entry can be found.

### 4.1 Scheduled into Checkpoint G

| id | item | owner | re-open condition |
|---|---|---|---|
| **G-REL** | Release behaviour of the three `debug_assert!` panic classes (F-3 / F-11 / F-12) — accepted and documented at Checkpoint C Q4, **never measured** | W10 live validation | none needed — it is §6 step 8, with a four-step read-only method and two named traps |
| **G-WFS** | `wanaka_final_surface_vs_mesh` — the `#[ignore]`d ground-truth guard `fa27b08` added; needs the wanaka project every wave was forbidden to touch | W10 live validation | none needed — §6 step 9 |
| **G-DIAG** | The rendered/operator-facing diagnostic example Checkpoint B's evidence list asked for and no wave could produce without a GUI pass | W10 live validation | none needed — §6 step 5 |

### 4.2 Open with a named next step

| id | item | owner | re-open condition |
|---|---|---|---|
| **D161-Q** | **D-16.1's quality claim.** −201.6 µm of overcut remains on arm B with **zero** cut targets outside the footprint. §3 closes the drop-cutter hypothesis: the commanded Z at the worst column is exact to 0.0 µm and the contact evaluation is exact. Attribution stands at `Shallow/raster`, mechanism unproven | the next finishing wave | instrument the **swept-volume / stamping attribution** at `row 64, col 245` (x 2.500, y −7.600), move 4841 — which tool positions contribute material removal to that column, and from what lateral offset. The drop-cutter path is now excluded by measurement and should not be re-scouted |
| **D161-L** | The `worst leftover` growth (222.9 → 339.7 µm) that came with F2. Attributed to an edge collar by geometry and by one agreeing aggregate (wall-band B p50 36.77 vs D 27.13 µm); **not measured** | same as D161-Q | same probe |
| **C25** | **C2–C5 (immutable `McpStateSnapshot`, precomputed `NarrationFacts`, bounded narration with a continuation token) — NOT ATTEMPTED.** The largest single item in W8's brief and not started. Reasons, on the record: the two read-path defects with measurable user impact are C1 and C6 and both shipped; the 720 s justification the package was sized against is **retired** (narration measured at 4 ms); and it is an L-sized change on the generation hot path with a mandatory golden-text sentry. Shipping half a snapshot layer would be worse than shipping none | a named lane | a **timed** read that exceeds the gate on the frame loop — which C6 now makes observable, since the five cheap reads answer regardless. The next candidate is a *parameterised* read (`get_toolpath_params`, `inspect_spans`, `get_toolpath_diagnostics`) measured against a stalled loop. Do not re-open it on the retired number |
| **F-T35** | **`feeds::predict::arc_fit_ratio_for_op` is stale by 4–13× and predicts a quantity that no longer exists.** Its table was fitted against the arc-mean chip observation the conversion deleted. B-lit §3.3/§6.1 rules the disposition: **retire, do not re-key**. Left standing because retiring it is number-moving *in Suggest*, not in the gate: `recalibrate_feed_for_chipload` solves `target / arc_fit_ratio` gated on `ArcFitRatioSource::Calibrated`, so setting every ratio to 1.0 moves Adaptive3d's solved feed by **4×** and DropCutter's by **6.7×**, and extends the lift to every other family for the first time. **Left standing by two consecutive waves** | census T3.5 | none needed — a ⚠ block on the function says so. It needs its own evidence package and its own before/after budget; it should not be carried as a note a third time |
| **X-VAC** | **The empty-population gate vacuity class.** Measured on arc-fit: three gates returned `Within` with `sample_range 0..0`, no locality and `available_kw 0.0` — indistinguishable on every surface from a measured clean cut, and one of them was suppressing a burn advisory. The arc-fit *instance* is fixed; **the class is not**. Any predicate that empties a gate's population produces a healthy-looking pass, and nothing on any surface distinguishes it | unassigned — a next-programme intake candidate | none needed to investigate. The cheap first step is a census of every predicate that can empty a gate population (`is_phantom_transit`, `is_steady_state_for_gate`, the measurability abstention, the sample-validity predicate) plus a `population == 0` marker on the verdict itself. Note that a *verdict* bar tests this vacuously — the bar must be a **population** bar |
| **F-HEATMAP** | The GUI viewport chipload heat-map colours by `max(effective_chip_thickness_mm)` per move against the advance-per-tooth band, i.e. the unit mismatch the gate no longer has, **on a visible surface**. It paints "rubbing risk" blue over cuts that are not rubbing, by ≥1.57× | the viz/MCP lane | none needed — named in code on `chipload_envelopes_for_session`. It must ship **with a screenshot**, per the programme's own rule that an aggregate without a surface is not evidence |
| **F-OPT** | The optimizer's retarget multiplier (`retarget/chipload.rs`, `target / gate-observed`) now divides by a different denominator on every `ae`-bearing row. Its own tests build synthetic verdicts and stayed green, so **no evidence measures a real retarget outcome moving.** B-lit §3.4 item 6 asked for this and it is **not discharged** | the optimizer lane | none needed |
| **A2D-165** | **165 of 198** adversarial matrix cells were never run — recorded as `NOT RUN`, not as passing | whoever next holds the slot | before any claim that a 2D family is clean on hostile geometry |
| **B2-BIN** | **B-2 (regeneration variance) and B-4 (cell sensitivity) are NOT RUN, and until B-2 runs no quality bin exists.** B-5's banked region is a *floor* that holds only because `max(V2, V4) ≥ V4` | a scheduled execution pass holding the Cargo slot | none needed — both are pre-registered in `reference_repeatability.rs` and their bodies **panic with the bar they must implement** rather than passing silently |
| **REST-24** | Rest-anomaly arms 2 and 4. Arm 2 is the cheapest remaining evidence on the anomaly and **must now state which rim perturbation it assumes — the answer differs by a factor of 1.5** | the ridge-extraction stages' next toucher (Checkpoint E re-pointed the owner off `measure_cross_section`) | arm 2 belongs in `rest_field.rs`'s own `#[cfg(test)] mod tests` (`box_smooth_rest` / `nms_candidates` are private); arm 4's `hillshade` is `#[cfg(test)] pub(crate)` and unreachable from `tests/` |
| **H4-SCHED** | `strategy_comparison_h4` is SCHEDULED, not run. Its dated result tables rot unless the cadence is honoured | the finishing lane (Checkpoint E, ask E6) | the cadence in the file's own header |

### 4.3 Feeds and gates — open with owners

| id | item | owner | re-open condition |
|---|---|---|---|
| **F-P6** | The power gate's radial-width divergence (~1.23× at 30 % radial). The ceiling was equalised; the **cross-section** was not | census T3.8 | a Checkpoint ruling — it is number-moving on the gate side |
| **F-FLOOR2** | The rubbing floor's second half, `max(floor, band_min)`, is unbuilt. The ruling covered the ceiling only | unassigned | an operator ruling on T4.2's other end, or a report of a burnished finish on a row whose band minimum exceeds 0.025 |
| **F-EXP** | Neither scaling exponent has a bench measurement behind it, and one of them decides a verdict that flipped twice in one day | unassigned; `CREDITS.md` names where to record a future measurement | any bench data on permissible chipload vs diameter or Janka, or an operator report of burning/breakage on a sub-Ø2 tool |
| **F-LUT2** | Suggest and the gate still use **different LUT lookup entry points** (`find_best_row_for_geometry` vs `find_best_chip_envelope_row`) | Checkpoint B item 5 | measure the row-selection delta first (structural, number-preserving); only then decide the switch, which is number-moving |
| **F-BIPOLAR** | `is_bipolar_engagement` compares raw chip thickness against the advance band. The deletion does **not** transfer: the predicate is *about* the arc that cancelled out of the gate, so re-expressing it in advance per tooth would make it near-vacuous. The samples are the right quantity and the **bounds** are the wrong yardstick | the optimizer pre-flight lane | a source for a chip-thickness envelope (no vendor in the shipped LUT publishes one), or a ruling that an engagement-variance refusal may be scaled from an advance band by an explicit disclosed factor |
| **F-MISSAE** | `ChipBoundsSource::VendorLutMissingAe`'s justification has evaporated but its behaviour is kept — it demoted the low side because "this row skipped the engagement-arc normalisation", and no row is normalised any more. Promoting 176 of 252 rows back to hard `Exceeds(Low)` trips is a behaviour change with no ruling behind it, and demotion is the conservative direction. The label now means "the weakest-annotated row class in the LUT" | unassigned | a Checkpoint ruling on whether the absence of a repo-authored `ae` window is evidence of anything |
| **F-VALID** | The chipload gate's sample-validity predicate still keys on `effective_chip_thickness_mm` and is vestigial; two `Unmodeled` reasons refuse to report an advance per tooth because a *chip model* did not resolve. Kept byte-identical so the flip table stays attributable to one cause | whoever next touches this gate | it needs its own before/after — widening it moves the population |
| **F-SPIKE** | Chipload `entry_spikes` now report kinematic feed excursions, not engagement excursions. Rare in practice (configured entries are dropped by the 95 %-of-commanded steady-state filter), but the advisory's *meaning* changed and nothing measures it | unassigned | an operator report of a missing entry advisory |
| **F-VBIT** | The hint↔trait parity sentry pins the V-bit arm at `tip_diameter: 0.0` — a pointed bit, where the two implementations agree **by construction**. It has never covered a truncated-tip V-bit, which is the one geometry measured to diverge (worst \|Δscale\| 0.4589, reachable as Adaptive3d + V-bit) | the feeds lane | none — widen it before any T3.1 re-measurement leans on it |
| **F-BASE** | `planning/toolpath_acceptance/baselines/2026-06-04.csv` is stale in its `chipload_kind` / `chipload_observed_mm_tooth` columns, **silently**: `smoke_baseline_regression_f037` parses the file and tests a synthetic mutation rather than running the pipeline, so nothing goes red | whoever next runs `rs_cam_cli smoke` | the next baseline refresh, which must be a deliberate re-pin with old/new recorded |
| **F-STR** | `band_capped_from`'s three operator-facing `format!` sites are unexercised by any test — the sentry asserts the field, not the strings | whoever next touches the feeds warning surface | a wording change, or a GUI screenshot pass over the feeds modal |
| **F-T15** | T1.5's `commanded_above_band` alarm fires at `ratio > 1.0` with no margin, and does not fire at all for rows with no chipload band | the feeds lane | an operator reports it noisy, or a band-less row needs the same report |
| **F-SHADOW** | `effective_diameter_mm`'s two same-named bindings are documented, not unified | census T2.4 / T3.6 | unify when the typed `DocRatioBasis` argument lands |
| **F-CENSUS** | `FEEDS_CENSUS.md` is stale in the resolved direction in four places (§2.6/§6.2 rate P-2 HIGH — measured false for the swept population; §2.5 and rows C-6/C-7 describe two laws each and carry the `^1.26` reading B-lit corrected to `^0.504`). It is a dated research artifact and was **not rewritten** | documentation lane, *if* the census is ever refreshed rather than superseded | none needed — the corrections are executable (`the_power_ceiling_does_not_bind_on_shipped_presets`) and are recorded in the log |
| **F-STEP69** | The Step 6 / Step 9 `safety_factor` composition is undeclared — the clamp is correct on the RAW axis only because Step 9 later applies the factor, an ordering no type enforces. The naive reading of the Checkpoint B ruling double-applies it and is a −25 % feed recalibration; it was measured, rejected, and pinned against | whoever next touches the feeds derate chain | any change to Step 9's placement or the clamp's ceiling; `a_power_limited_feed_lands_exactly_on_the_gate_ceiling` fails rather than drifts |

### 4.4 Drill

| id | item | owner | re-open condition |
|---|---|---|---|
| **DR-THR** | R-8 (chip-welding 8/6/5), R-9 (per-peck ceilings) and R-10 (plunge envelopes) **keep their values**, per the Checkpoint D ruling "correct the citation, keep the number". All three are now declared repo-authored in code and `CREDITS.md`. The 0.5–1.0×D general convention is recorded as *sizing the gap*, explicitly not as a target — moving these down on metal-twist-drill practice would be the same category error in reverse | operator, on primary evidence | a retrievable wood-drilling source or a bench trial |
| **DR-MULT** | `DRILL_CHIPLOAD_MULTIPLIER = 2.5` stays at 2.5. Unsourced, its comment was arithmetically false on its own constants, and against the one real chart it is ~2× **under**-sized (needed 4.97) on a gang-drill confound that forbids acting on it | whoever lands T3.1 | the gate-side unit conversion landing **and** the optimizer targets being re-derived |
| **DR-PIN** | `apply_drill_defaults` clamps `Drill` but not `AlignmentPinDrill`, so a softwood Ø6 pin drill gets an 18 mm Suggest peck against a ~13 mm hole — a single-shot cycle wearing a `Peck` label, which the gate then passes at 2.17 vs 6.0 | next intake | it is a Suggest default, not a report |
| **DR-LIVE** | The `peck_depth` livelock is closed only for the two functions D-impl-3 consolidated. `drill::fed_descents` guards a non-positive or non-finite peck; `ParamDef::required("peck_depth", "f64")` still carries **no range**, so MCP `set_toolpath_param` and hand-edited project TOML can still set one | W0/W8's successor, as the audit assigned | none — same class as the adaptive3d livelock PR-1 closed |
| **DR-DIV** | All three drill gates divide by the **envelope** radius with `ToolProfile::Flat` hardcoded, and `Drill` carries no tool precondition. On a tapered ball that overstates diameter by up to 14×, and two of the three block export when they trip — so the failure mode is a **silent pass** | the radius tech-debt programme (`RADIUS_AUDIT.md`, R-12) | stated there, with a cheap red-first fixture named |
| **DR-URL** | The 10 hard 404s and 1 dead host **outside** drill scope remain dead: `toolgrit`, and three gold-tier rows sharing one dead Kennametal URL (`dapra_rctf`, `kennametal_metals`, `kennametal_chipload`). Invisible to a shape check by construction | the `/refresh-lit-matrix` run | immediately |
| **DR-WS** | The two Whiteside rows describe the same absent chart and their `gold` tier is unsupported by any retrievable document. Tier is documentary — no code reads it — so it was recorded rather than silently re-graded | the refresh run | merging them is a judgement call |
| **DR-P3** | P3, opt-in URL liveness checking. Nothing in the repo performs network I/O in a test binary | `/refresh-lit-matrix` | a decision that network I/O in a test binary is acceptable at all |
| **DR-INV** | The literature-matrix citation audit does not audit **invariants or anti-patterns** (`freshness.rs`), and most drill and chipload thresholds live on invariants — so the gate that exists to check citations is not checking the ones that matter | documentation lane or M2 | any threshold recalibration |
| **DR-CELL** | `cells.toml` pins `flat_12mm_drill_oak_big` at rpm 3000–6000 / floor 2000 while the sentry and production both use 4000–8000; the same cell declares oak Janka 1360 while the Ipe sentry anchors oak at 1290 | M2/R5 | none — implementation and sentry agree with each other; the cell does not |

### 4.5 2D geometry and the offset contract

| id | item | owner | re-open condition |
|---|---|---|---|
| **O-NAN** | No non-finite pre-check on `Polygon2`. A `NaN` vertex still reaches the library, where in debug it surfaces as a `LibraryFailure` and in release it does not. `polygon::OffsetRejection`'s doc says this out loud rather than letting a reader infer a guard that is not there | whoever rules F-11/F-13 | a ruling on a validating constructor. Deliberately out of scope: Checkpoint C Q1 named the reasons, it did not add refusals |
| **O-LOC** | `OffsetFailure::LibraryFailure` carries no source location. A stated limit, not a defect: `catch_unwind`'s payload does not contain one and recovering it needs a process-global panic hook, which a primitive called from parallel worker threads must not install | none — recorded in `crate::panic_message`'s module doc | — |
| **O-ROLL** | Eleven `offset_polygon` consumers have not opted into the reported channel. Four families opted in; the fifteen geometry-only sites pay zero churn **by design** and honestly report `None` = not measured | opportunistic, per `OFFSET_CONSUMER_ROLLOUT.md` | a consumer where the collapse/failure difference has a cost |
| **O-RING** | `OffsetRingSet` remains unbounded. This is a **decision**: a bound is a consumer's policy, not a geometry type's, and scallop's cascade carries its own `max_rings` | none | W4's `the_pocket_ring_cascade_is_bounded_only_by_collapse` still measures the primitive diverging, and its doc says where the consumer-side bound went |
| **O-CANC** | `AlignmentPinDrill` and `Chamfer` are still uncancellable | unassigned | neither is in the 2D campaign's nine, so W4's evidence does not reach them; Checkpoint C Q3 ruled on rest and drill specifically. `ExecutionContext`'s doc names them so the 21-of-23 claim cannot go stale |
| **O-R1** | `offset_polygon_degenerate_inputs_r1`'s first test no longer exercises the containment it was written for — R1.5's 2026-07-06 repair splits the self-intersecting asset before cavalier is called | whoever next edits that file | it should either assert the repair path it now takes, or find an input the containment still catches |
| **F-7** | adaptive3d inherits its surface sampling density from arc-join debris (`clearing.rs:1818`): the producer guarantees **shape only**, so a straight run of any length yields two probes. Worse, site 1818 is **not** one of Checkpoint C's five opted-in sites, so the typed failure channel is blind to the site the same checkpoint flagged; on an empty offset the perimeter sweep is silently skipped, re-arming the deep-DOC failure it exists to prevent. Contract stated, fixture **specified and deliberately not built** | the adaptive3d lane | any disjoint-island or sparse-region adaptive3d fixture |

### 4.6 Structure, spans and parity

| id | item | owner | re-open condition |
|---|---|---|---|
| **S-MERGE** | **`condition::merge_linear_runs` does not break on `Region` edges.** Arc-fit does (Checkpoint F1 Q2); this pass does not, because Q4 authorised the intent term and nothing wider. A merge spanning two `Region` nodes can still fold region A's dropped geometry into region B's first kept move | unassigned | any wave that reads region-level geometry off a `segment_merge`-enabled operation — which is anything touching Roughing. Stated in `condition.rs`'s module docs |
| **S-BARR** | `rapid_order_barriers()` and arc-fit's barrier set have **diverged by design**. Arc-fit adds `Region` locally; TSP, `dressup.rs` and `execute.rs`'s barrier-count branch still see the narrower set — widening the shared accessor would have been a second, unruled output move | whoever next needs a shared barrier vocabulary | a second consumer wanting `Region` as a barrier; at that point it belongs in the shared accessor with its own ruling |
| **S-COST** | The arcfit fix's cost is unmeasured at production scale — five fixtures, no long A/B harness re-run | whoever next holds the slot for a full-scale A/B | any claim that the change is free at wanaka scale |
| **S-PAR** | Residual planner/simulator divergence adjacent to the drape mirror: the emitter feeds a `Cut` from wherever the tool actually is to the **draped** `blended[1]` while the planner stamps `blended[0] → blended[1]`, and a `Cut` of fewer than two points is emitted as nothing but still point-stamped. Both are *created* by the drape breaking the pre-`fa27b08` identity. Mirroring them needs the emitter-true position threaded alongside `last_pos`, which also drives `is_clear_path_3d` link decisions — i.e. a behavioural change | a later parity wave | either registered bar moves, or a fixture attributes standing material to Link-start stamping |
| **S-DZ** | `max dz` is still **25.000 mm — the full stock height** — on both strategies, before and after the drape mirror. Somewhere a whole column is cleared in one model and untouched in the other; that is not sub-cell blend noise. **No dz bar is registered, because a bar that cannot pass today is not a bar** | the same later parity wave | as above, or any claim that planner/simulator parity is "closed" |
| **S-DRAPE** | Planner-side drape cost is unoptimised — both sides now pay the same `point_drop_cutter` queries (+15–45 % on the parity fixtures, recorded) | whoever next profiles 3D roughing generation time | a generation-time regression report on a large DEM |
| **S-B3** | B3, the lossy `ToolpathStats` → `session::ToolpathDiagnostic` projection: deliberate, six of fourteen published | whoever needs a finding on the MCP/CLI per-toolpath summary | a consumer asks; widening it is a wire change, not report wiring |
| **S-B4** | B4, the hand-written `Serialize` for `session::ToolpathDiagnostic` — a new field is silently unpublished, with no exhaustiveness guard (its CLI sibling solves the same problem with a `..`-free destructure) | whoever next widens that wire | it cannot become a `derive` while the A6 dual key ships from one field, so any fix must keep `standing_material_channel_am9`'s dual-key and no-alias assertions green |
| **S-GAP3** | Three findings-transport boundaries **downstream** of the H2.1 join remain unguarded. A field can still be routed into `ToolpathStats` and then fail to reach narration or the MCP wire, silently | the findings lane | census §6 names them |
| **S-VALCNT** | `chipload.rs`'s `valid_count` overstatement: it increments **before** the `is_phantom_transit` skip, so the published `sample_count` overstates the population that drove the gate. Documented in place, not fixed | whoever widens that wire | `sample_count` is a shipped wire field, so correcting it is a separate gated change |
| **R7-L1** | Clip-visibility census: 20 reduction sites, 2 reach `ToolpathStats`. Report-only proposal, **nothing written**. It also corrected the prior record — MidSteep and Shallow are **not height-clipped at all**, so widening `BandHeightClip` to them would report a structural zero and be wrong | a later wave | none needed |

### 4.7 GUI, IO and read paths

| id | item | owner | re-open condition |
|---|---|---|---|
| **G-RESULTS** | In **GUI mode** `ProjectSession.results` is never written, so `get_toolpath_diagnostics` / `get_tool_load_report` run **without generation findings and without spans** while the CLI's run with both. A second, larger instance of B7's family | the viz lane | none needed — this is a live divergence between what an agent sees through the GUI and through the CLI |
| **G-DROP** | The toolpath model dropdown still lists **every** model rather than filtering by the owning setup's `model_ids`. The data now persists; filtering changes which models a toolpath can be reassigned to, which P-2 was not scoped to do. The stale "will be wired via `SetupRuntime`" comment is replaced by that statement in the source | a GUI wave | none needed |
| **G-POST** | `ProjectPostConfig.format`'s two disagreeing defaults (`""` from `serde(default)` vs `"grbl"` from `impl Default`). Still benign — `from_token("")` returns `None` and the export site falls back to GRBL, the same value — but now benign by one deliberate line rather than by accident | P-5's schema-drift item | — |
| **DLV1-NE** | D-LV.1's **near-empty** exporter symptom is NOT reproduced, and W8 does not claim it. Two hypotheses refuted on the record (classification drops no geometry; the `linearize_arc` full-circle blow-up cannot be it, since simulation calls the same function and the op simulated clean). The **classification** half is fixed and a new envelope guard covers the remaining class | the viz lane | it recurs — and the next reporter should capture `auto_ribbon_radius` and the tube mesh bbox alongside the PNG, which discriminates every remaining hypothesis in one shot |
| **P-3** | v≤2 alignment-pin migration is unreachable (core parses a v2 file successfully, so the viz fallback that owns the migration never runs). **Downgraded to `ruled out (no live population)`** by the orchestrator on 2026-08-04, not fixed | none | confirmation that `format_version <= 2` files exist for this operator |
| **INTAKE** | `I-1..I-5`, `P-4`, `P-5`, `X-2..X-5`, `G-1..G-5` from the risk map — 17 items, ranked, **none may be started inside this programme** (R8's mandate was a map, not a fix queue). Three of them explicitly need a runtime check before being called defects and say so: I-2 (SVG px factor depends on the viewBox), G-1 (`auto_resolution: true` may override the GUI seed), I-1 (only multi-shell STEP ordering varies) | the next programme's intake | `UNTOUCHED_TERRITORY_RISK_MAP.md`, with the caveat that its "ruled out" column means "inspected and found sound at `63d5e8b`", not "proven safe" |

### 4.8 Simulation channel and measurability

| id | item | owner | re-open condition |
|---|---|---|---|
| **M-DEFAULT** | `MeasurabilityReport::for_metric` returns `Measurable` for toolpaths with no row. Deliberate — absence of a measurement is not evidence that measurement failed, and abstaining by default would silently disarm every gate on the first caller that forgot to build a report — but a caller that skips report construction gets today's behaviour rather than a loud failure | the sim lane | a caller is found that should have abstained and did not |
| **M-BLIND** | `NOT_MEASURABLE_BLIND_FRACTION = 0.5` sits nowhere near the measured arms (shallow ~0.98, deep 0.0), so the classification boundary is **not load-bearing on the evidence that motivated it** | the sim lane | a fixture that lands between the two arms |
| **M-R12** | R-12's bar (2 % of samples over 3× the pass's own median, minimum 50 bites) is calibrated against **one** measured case. It has not been run against a corpus of healthy finishing passes, so its false-positive rate is unmeasured | the sim lane | a corpus run, or an operator reporting it noisy |
| **M-FRESH** | `gcode::sim_trace_is_fresh` refuses on the committed `test_job.toml` after a two-pass generate/simulate, even with the three never-generating toolpaths disabled, so **every gate on that fixture returns `Unmodeled(StaleSimulation)`** and it cannot carry verdict-level evidence. Residual cause not diagnosed | whoever owns the freshness gate | a project fixture where `sim_trace_is_fresh` holds after a two-pass generate/simulate |
| **M-VIZ** | The viz-side issue-list runtime cost (three deep clones of an unbounded `Vec` per frame) was **read, not measured** — measuring it needs an `rs_cam_viz` harness no wave built | the viz lane | none needed |

### 4.9 Fixtures, harnesses and records

| id | item | owner | re-open condition |
|---|---|---|---|
| **E-REP1** | `REFERENCE_FIXTURE_REPEATABILITY.md` §1 needs a **smooth-ground qualifier**: B-3's `cell·tan θ` bound says nothing across a step, and a COLUMNS instrument on real relief cannot make that distinction, so its realised alias will exceed the bound at every step in a part | whoever next edits that document | none — the measurement is in `reference_repeatability.rs`'s B-3 doc |
| **E-PY** | `arp1_reference.py::ConeAnnulus.kappa_max` returns `cos θ/r`; the correct value is `sin θ/r`. **Blast radius: none** — the cone is absent from the Python's case list, so no published number moves. The Python was deliberately not edited: it is W7's committed artifact | whoever next ports from that file | none — pinned by `python_reference_cone_curvature_divergence` |
| **E-COL** | `common/columns.rs` was deliberately **not** merged with the `BandMap` family, and its 138 shared lines with `classification_columns_ab_m3.rs` stay unextracted | the finishing lane, opportunistically | the C6 policy is "never in bulk", and `MEGA_HARNESS_POLICY.md` §4 explicitly forbids merging the two instrument families |
| **E-CHAIN** | `ChainOutcome` / the outer `run_chain` wrapper not extracted — the two donors' versions are structurally **diverged** rather than duplicated, and a lossy union could have changed behaviour | — | recorded in `common/chain.rs`'s module doc |
| **E-P1415** | P14 (absolute-mm NMS prominence floor compared against a one-cell difference — worst on the broad smooth rest maxima real parts have) and P15 (a refused branch whose ridge cells all fall outside the threshold mask is dropped from **both** output lists without trace) | ledgered in `ANTIPATTERNS_BACKLOG.md` with reproductions | recorded there |
| **E-WALK** | `CROSS_SECTION_WALK_CELLS = 64` is a walk budget **in cells** (32 mm at the shipped cell, 6.4 mm at 0.1 mm) with **no flag distinguishing "found a rim" from "gave up"** | the ridge-extraction owner | any study point below ≈0.05 mm, where it binds |
| **E-CELL3** | The three `0.5` rest-cell defaults are written in three separate places (`rest_field.rs`, `pencil.rs`, `config.rs`). Noted, **not** unified — out of scope and a production change | unassigned | — |
| **FP-65** | `FinishPlannerParams::waterline_threshold_deg`'s doc says "Default 65" while the code ships 75.0 | the finishing lane | none — incidental, found in passing |
| **HIST-1** | One C-impl commit carries three approved items instead of three commits. Stated rather than hidden: the channel is inert without a surface and the F-1 refusal is not expressible without the channel, so they are one mechanism; and with three lanes in a shared tree, interactive hunk staging was unavailable and a deliberately red intermediate commit would have been worse | — | recorded for whoever bisects |
| **HIST-2** | Two D-impl-1 commits are out of dependency order (`8cbc691` references a module landing in `9d8b095`), and `absorb_stats` reached the tree inside another lane's commit `0a45e35`, which swept `narrate.rs` up while it was uncommitted in the shared working tree. **The branch tip is correct in both cases; only the attribution is wrong, and it cannot be fixed without rewriting a landed commit.** Standing lesson: with two lanes in one working tree, `git commit -a` is not safe | — | do not rewrite the shared branch to fix it |

---

## 5. Stale-rationale sweep (plan §L1, practice rule P11)

Executed this wave; commit **`fdb0ae6`**. Nine claims that the programme
falsified, corrected where a reader hits them.

**Method and discipline.** Shipped docs (`CLAUDE.md`, `README.md`,
`FEATURE_CATALOG.md`, `AI_MACHINIST_ANALYSIS_REFERENCE.md`,
`architecture/`, the `sim-analysis` skill) are **live claims** and were
corrected in place with the supersession named — the repo's own house
precedent (`FEATURE_CATALOG.md`'s UnifiedFinish row,
`PROGRESS.md`'s banner). Programme and prior-programme **records** are
history and were **not rewritten**: they carry a dated blockquote
erratum in the established form, per plan §L1 and the ruling recorded
against E3.

| # | stale claim | where | correction |
|---|---|---|---|
| 1 | "`ToolpathStats` carries **twelve** report-only findings … Ten share one contract" | `CLAUDE.md` (live) | **fourteen**; eleven share the contract, three sit outside it. Checkpoint C landed **two** slots, not one |
| 2 | "…surfaced through `narrate_toolpath` and the diagnostics list" | same line | **four are not** — `deprecated_dial`, `derived_stepovers`, `claims_reference` never reach narration and `boundary_clip_dropped` has no narration adapter. The sentence now says so and says not to cite it as coverage. (W1 filed this as five including `retract_trips`; W8 **refuted** the `retract_trips` half — it is populated and rendered in both literals) |
| 3 | The chipload gate reads a **chip thickness** | `AI_MACHINIST_ANALYSIS_REFERENCE.md` §Chip Load; three in-code rationales (`dexel_stock/simulation.rs`, `dexel_stock/stamping.rs`, `sim_measurability.rs`) | **advance per tooth**, with the axis in the heading and an explicit warning that pre-2026-08-06 readings are on a different axis (per-row 2.4×–40.4×) and are not comparable. `chipload_mm_per_tooth`'s "material removed per flute per revolution" gloss replaced — it is kinematic, not a measured chip |
| 4 | `narrate_toolpath` is a "~12 min GUI-thread grind" | `RESEARCH_COMMISSION.md` (record) | dated erratum: **4 ms** measured. The structural defect survives; its 720 s justification does not |
| 5 | "shallow band ignores `stock_to_leave`" | `RESEARCH_COMMISSION.md` (record) | same erratum: landed, and **wider than filed** — VerySteep dropped it too |
| 6 | Drill thresholds presented as vendor/handbook-backed | `CLAUDE.md` drill table | **repo-authored**, with the retrieval result stated; plus `Elevated` is not an exceedance, the peck model is R-plane-rooted, and the envelope-radius divisor makes an overloaded tapered ball a silent pass |
| 7 | Three posts (GRBL, LinuxCNC, Mach3) | `README.md`, `FEATURE_CATALOG.md`, `PROGRESS.md`, `architecture/requirements.md`, `architecture/user_stories.md` | **four** — grblHAL ships, with a shipped definition, a post file and a round-trip sentry, and was listed in **zero** docs |
| 8 | "Look at `hotspots` and `rapid_collision_count` instead" of `issue_count` | `CLAUDE.md`; `AI_MACHINIST_ANALYSIS_REFERENCE.md` §11; `sim-analysis` skill | **`SimulationTriage` and measurability appeared in zero shipped docs** despite being the contract four surfaces consume. Added, ordered safety → actions → advisories, with the abstention rule and the "never disables collision detection" guarantee |
| 9 | "stock-boundary clipping with center / inside / outside containment" as an unconditional capability | `FEATURE_CATALOG.md` | **not unconditional** — a collapsed offset emits the path unclipped with a report-only finding; a failed offset now refuses |

**Also corrected, and worth naming separately** because they change what
a reader may assume rather than a number: three offset panic classes are
`debug_assert!`s, so `offset_library_failures` is **not comparable across
debug and release**; and a gate handed an empty population returns
`Within` and looks healthy, so an exoneration must be checked against
`sample_range` before it is believed.

**Incidental drift the sweep caught** (pre-existing, not this
programme's): `README.md` claimed 22 operations against 23 everywhere
else and omitted STEP import; `PROGRESS.md` described the MCP server as
"16 tools, integration ongoing" against roughly 68 against a live
session, and told the reader to run workspace-scope `cargo test`, which
`CLAUDE.md` forbids on this repo.

**Errata placed, not rewrites:** `RESEARCH_COMMISSION.md` (items 4, 5),
`FINDINGS_PIPELINE_CENSUS.md` and `UNTOUCHED_TERRITORY_RISK_MAP.md` (item
1, plus a second erratum on the map recording that two of its three "fix
now" items were ruled and executed and the third was downgraded to *ruled
out — no live population*). `FINISHING_OPEN_DEFECTS_EVIDENCE.md` §0
already carried the standing correction and needed none.

**Not swept, deliberately:** `FEEDS_CENSUS.md` — see §4 row **F-CENSUS**.
It is a dated research artifact with four stale statements, all in the
*resolved* direction, all executable, and rewriting it would convert a
record into a claim.

---

## 6. Checkpoint G — live-validation plan

> **STATUS: NOT RUN.** This section is a plan. Nothing in it is a
> result, and no line of it may be quoted as one. Checkpoint G's decision
> is *"authorize release build and read-only live characterization"* —
> until the operator gives it, step 0 does not begin.

Derived from plan §4.4 (its eight numbered steps, plus step 8's
release-behaviour probe written by C-impl) and from the twelve
behavioural batches this programme actually shipped.

### 6.0 Entry conditions — all four must hold before step 0

| # | condition | state at close-out |
|---|---|---|
| 1 | every approved behavioural wave is green | **MET** — `cargo test -p rs_cam_core --lib` 2260 / 0 / 12 ignored; `cargo fmt --check --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean |
| 2 | known-red baseline is zero, or has an explicit new operator ruling | **MET** — the accepted-red allowlist is empty. `wanaka_suggest_baseline` remains the single declared environmental exception (plan §2 rule 9), owned outside this programme, and is **not** run |
| 3 | per-wave renders/evidence are linked | **MET** — §7 |
| 4 | a live MCP plan is ready | **MET** — this section |

### 6.1 Standing rules for the whole session

These are not steps; they bind every step.

1. **`planning/airrun_2026-06-01/wanaka.toml` is read-only.** Never edit,
   stage, revert, or save it. It is currently modified in the working
   tree by the operator's own hand and **must not be committed or
   reverted by any agent**. Restore only documented session overrides;
   never save the project.
2. **Never clear a collision across mismatched resolutions.** Tip-matched
   simulation is authoritative. The GUI auto-sims at a different cell
   from the headless default, and that asymmetry has already produced a
   20-collision false alarm once. Record the cell with every collision
   count.
3. **Regenerate rest-dependent toolpaths after re-simulation.** Re-simming
   does not mark toolpaths stale. `claims_reference` must be
   `machined_stock` in a cascade — `self_probe` measures the field the
   tool cannot reach and reports a −88.7 % improvement on nothing.
4. **Render before verdict.** An aggregate without a surface is not
   evidence. This is the rule the programme broke twice and paid for
   twice.
5. **`PASS` / `NOT EXERCISED` / `CONCERN` / `FAIL`, and never turn an
   unexercised path into a pass.** `NOT EXERCISED` is the likely and
   acceptable answer for anything the live project does not reach.
6. **Compare fixed-path outputs, not a stale toolpath against fresh
   stock.**
7. One Cargo/GPU job at a time; `free -g` and bracketed
   `pgrep -af "bin/[c]argo"` before every launch.

### 6.2 The checklist — 24 items

**Preparation (3)**

| # | step | evidence to record |
|---|---|---|
| 0 | Confirm clean staging; `git status` shows no staged work and `wanaka.toml` untouched by any agent | the `git status` output |
| 1 | `cargo build --release -p rs_cam_viz --bin rs_cam_gui`, **waited to completion** before MCP connection. `.mcp.json` runs `cargo run --release`, so a cold compile blows past the 30 s connect timeout — and never trust the binary mtime | build wall-clock |
| 2 | Load Wanaka read-only; `inspect_model` / `inspect_stock` / `inspect_machine`; `list_toolpaths` | model bbox, triangle count, stock, the toolpath inventory with statuses |

**Approved behavioural batches (11).** One row per batch this programme
shipped that an operator can see.

| # | batch | what to do | what would be a `CONCERN` or `FAIL` |
|---|---|---|---|
| 3 | **adaptive3d drape mirror** (`8853fdb`) | regenerate a 3D Rough op; record generation wall-clock beside the pre-programme figure if one exists; simulate at tip-matched resolution | standing material where the planner claimed removal; a generation-time regression on a real DEM materially worse than the +15–45 % measured on the fixture (row **S-DRAPE**) |
| 4 | **arcfit intent split, live** (`3dbec75`, `5fad7e2`) | on a Finish-role op with `arc_fitting: true` and lead-in/out on: `inspect_spans` and confirm no fitted arc's `intent` disagrees with its span; confirm arc count did not blow up at production scale (row **S-COST** — this is the only place that can be measured) | an arc labelled `FinishingCut` whose target is a lead-out point; a material arc-count rise |
| 5 | **triage page-one** (`f8cf721`, `f8a60f2`) | `get_diagnostics` on the full project; read `triage` **before** anything else. Confirm: safety events present and first; advisories capped with `truncated` and a true `total_matching`; the same summary in the GUI panel and the CLI report | an unbounded advisory list; a safety event deduped away; the four surfaces disagreeing |
| 6 | **measurability abstention on a fine op** (`9d8b095`) | pick a genuinely fine finishing op (Ø1 tip) and simulate at a **deliberately coarse** cell; confirm the metric reports `NotMeasurable` with a reason, its gate **abstains** rather than passing, the GUI prints the `NOT MEASURED:` strip, and **collision detection still runs**. Then re-run tip-matched and confirm it becomes measurable | a percent printed for an abstaining metric; a gate passing on an abstention; collisions suppressed |
| 7 | **the empty-population check** (row **X-VAC**) | for every gate that returns `Within` in this session, record `sample_count` / `sample_range`. This is not a fix, it is the census the class needs | any `Within` on `sample_range 0..0` — record it, it is the next programme's first row |
| 8 | **drill wording** (`0e6efde`, `9c340b6`, `f99dd99`, `8e9dc6f`) | on the alignment-pin drill op: read the drill gate messages verbatim and the `drill_summaries` entry. Confirm no `Exceeds` string on an observed value **below** its stated bound; confirm the `Within` display shows the boundary that decided it; confirm `peck_count` and `feed_time_s` match the emitted G-code's feed-downs (R-plane rooted); confirm no two remedies contradict on one hole | any of the four C-1..C-4 contradictions surviving; a `peck_count` disagreeing with the G-code |
| 9 | **the flip-table op's live verdict** (`0a45e35` + `62ffd5c`) | find the live scallop/finish op the B3 fixture was synthesised from. Record: the chipload verdict, the observed value, the band, `is_extrapolated`, the confidence, and whether a burn advisory is raised. **Both moves are in the build** — the unit deletion and the diameter law — so the live band is neither of the two the log records | a verdict inconsistent with the observed/band arithmetic printed beside it; `is_extrapolated` absent on a sub-Ø2 down-transfer (the rider) |
| 10 | **the rendered diagnostic example** (Checkpoint B's outstanding evidence item, row **G-DIAG**) | screenshot the feeds modal and the diagnostics panel for the op in step 9. The stage-labelled explanation record, the `commanded_above_band` alarm and — if it fires — `band_capped_from`'s wording (row **F-STR**, currently untested) have never been read on a live surface | wording that does not name every term between the gate's number and the commanded one |
| 11 | **`stock_to_leave` dial** (`921c94e`) | on a UnifiedFinish op: set `stock_to_leave` to 0.5, regenerate, and confirm **all three bands** move — shallow raster, mid-steep scallop, very-steep waterline. Then set it to 0.0 and confirm byte-identical output to the pre-change baseline | any band not moving; a step at the shallow↔waterline seam |
| 12 | **grblHAL export** (`a80020a`) | select the grblHAL post, save, reload the project, export. Confirm the exported G-code uses the grblHAL definition — the observable is that `M7` **survives** (GRBL's post filters it and emits a `WARNING: M7 unsupported` line) | a GRBL preamble or a dropped `M7` on a grblHAL project |
| 13 | **exporter screenshots vs viewport** (`e568ea2`) | `screenshot_toolpath` with `include_rapids: false` on two ops with nested spans (entry, lead, link, dressup, region). Read the PNGs. Compare against the viewport for the same ops | the two disagreeing on walk direction, `GeometryRefit`/`DressupArtifact` precedence, or the `DepthPass` gradient. Also watch for D-LV.1's unreproduced **near-empty** symptom (row **DLV1-NE**) — if it recurs, capture `auto_ribbon_radius` and the tube mesh bbox alongside the PNG, which discriminates every remaining hypothesis in one shot |

**The release-behaviour probe (4).** Plan §4.4 item 8, Checkpoint C Q4
option (a). The divergence was accepted and documented, **never
measured**, and this is the first legitimate opportunity.

| class | site | debug | claimed release behaviour — **UNVERIFIED** |
|---|---|---|---|
| slice stitching | `cavalier_contours` `pline_view.rs:507` | asserts, contained | proceeds with `start_index > end_index`, stitching a malformed slice into a ring |
| spatial index | `static_aabb2d_index.rs:266` | asserts, contained | builds a corrupt index and offsets on it — the library's own docs say "a panic **or unexpected behaviour**" |
| zero-length arc | `pline_seg.rs:33` | asserts, contained | divides by a zero chord, returning a NaN radius and centre into the offset geometry |

| # | step | note |
|---|---|---|
| 14 | `cargo test --release -p rs_cam_core --test boundary_clip_escape_f1 --test skipped_boundary_offset_f8 --test cavalier_shape_failure_r2` | every failure arm is `cfg!(debug_assertions)`-gated and prints a release line instead. **A green release run is not evidence the classes are benign — only that the sentries are honest about not exercising them.** Record which arms reported their release branch |
| 15 | Generate the 2D families that reach the offset (pocket, profile, trace, zigzag, inlay) live, and record `offset_library_failures` in **release**, beside the debug number for the same project | **a lower release count is the expected divergence, not an improvement** — it means the assertions did not fire and the library proceeded on unvalidated input. Remember the count is per offset **call**, not per ring |
| 16 | **Compare the emitted geometry, not the counts.** If a release run produces a toolpath a debug run refused or truncated, that difference **is** the finding — and it must be **rendered** | the programme's own rule: an aggregate without a rendered surface is not evidence |
| 17 | Classify each of the three classes `PASS` / `NOT EXERCISED` / `CONCERN` / `FAIL` **separately** | `NOT EXERCISED` is the likely and acceptable answer for classes the live project never reaches. Recording it as `PASS` is not |

**Closing (6)**

| # | step |
|---|---|
| 18 | Run `wanaka_final_surface_vs_mesh` (row **G-WFS**) — the `#[ignore]`d ground-truth guard `fa27b08` added, which needs the project every wave was forbidden to touch. This is its first legitimate opportunity |
| 19 | Confirm the **final operator-facing wording** for feeds, drill and issues on the live surfaces — the strings, not the structs |
| 20 | Record collisions **at matched resolutions** for every op simulated, with the cell beside each count. State explicitly which resolution each number was taken at |
| 21 | Confirm `wanaka.toml` is still untouched: `git status` and `git diff --stat` on that path, quoted |
| 22 | Append a factual live report to `ORCHESTRATION_LOG.md` in the §3.1 format, classifying every path `PASS` / `NOT EXERCISED` / `CONCERN` / `FAIL` |
| 23 | Update §4's ledger for anything the session re-opens. **A live result may re-open a defect but does not retroactively alter a committed fixture gate without a new evidence wave** (Checkpoint G's own decision text) |

### 6.3 What this session is explicitly NOT for

- Fixing anything. It is read-only characterization. A finding becomes a
  ledger row and, if warranted, a new evidence wave.
- Re-running historical Wanaka campaigns (plan §"Explicitly out of
  scope").
- Re-opening B1/B2 or producing a strategy winner — Checkpoint E ruled
  they stay closed, and with B-2 unrun there is **no defensible bin** to
  gate on (row **B2-BIN**).
- Producing a quality verdict from any aggregate. Pointwise only
  (`column_deviations` / the scallop oracle), with domain, stage,
  resolution, common population and `resolution_clamped` stated.

---

## 7. Benchmark and measurement provenance index

Plan §L1 RQ2: *which context must be recorded to make a comparison
repeatable*. Every operator-relevant number this programme produced, with
the build it was measured on and the instrument that reproduces it.

**Standing conditions unless a row says otherwise:** debug profile,
branch `experiment/adaptive-spiral`, single-Cargo machine (`free -g` +
bracketed `pgrep` before every launch), rustc stable, and **no release
build in any implementation wave** (plan §2 rule 11). `wanaka.toml` was
not an input to **any** row below — every number is from a committed
fixture or a file-resident constant.

### 7.1 Gate-state timeline — `cargo test -p rs_cam_core --lib`

| revision | result | note |
|---|---|---|
| programme open | 2227 / **3 failed** / 12 | the three adaptive3d reds |
| `24c9a12` | 2232 / 0 / 12 | PR-1..3; allowlist **empty** |
| `3dbec75` | 2233 / 0 / 12 | + the `merge_linear_runs` sentry |
| `4dd8b93` | 2245 / 0 / 12 | drill package |
| `69fa1ab` | 2251 / 0 / 12 | sim-channel package |
| `c8b6615` | 2256 / 0 / 12 | 2D failure contract |
| `08e7c88` | 2256 / 0 / 12 | conversion — **identical**, which was the point |
| `66507e4` | 2257 / 0 / 12 | F2/F3 (`overlap_dilates_band_polygons` split in two) |
| `62ffd5c` | 2260 / 0 / 12 | feeds-final |
| `28fce45` / **HEAD** | **2260 / 0 / 12** | io-fixes + W10 |

One transient reading is preserved rather than tidied away: a re-run at
`c8f0f87` reported **2255 / 1 failed**, on `finish_planner::tests::
overlap_dilates_band_polygons`, caused by *another lane's uncommitted
working-tree edit*. The commit that measured 2256 touched none of the
files involved. Recorded because "which build the number came from" is
the whole point of this index.

### 7.2 Performance and cost figures

| figure | value | build / method | reproduce |
|---|---|---|---|
| planner drape mirror cost | +15–45 % (`agent_search` 2.20/2.81 → 3.18/3.93 s; `contour_parallel` 9.02/9.27 → 12.17/10.26 s) | debug, two runs per arm back-to-back in one session so machine load is common to both; whole-test wall clock (planner **plus** simulator replay), 16-segment hemisphere — **not** a 220k-triangle DEM | `--lib planner_sim_dexel_parity_*` |
| arcfit intent-key arc cost | **ZERO** — `(moves, arcs)` identical on all five fixtures | debug, same test binary run with the source change stashed out and applied | `--test arcfit_intent_key_cost_f1` |
| `get_cut_trace` speed-up (C1) | **32×**, counted in sample touches (128 000 → 4 000), asserted against a 30× floor | counted in **visits, not wall-clock**, so it reproduces on any machine | `--test span_summary_single_pass_c1` |
| narration cost | **0.004 s** on 12 580 moves / 69 808-sample cut trace | idle lane. **Retires a 720 s figure** that was one wall-clock reading taken during a >40-minute `generate_all`. Caveat: the fixture is 12.6k moves, not the 148k-move op the anecdote came from | `--test narration_cost_probe_h26 -- --ignored` |
| F2 runtime / mm²/s / trips | arm B 96.9 → 84.8 s (−12.5 %), 14.63 → 16.22 mm²/s, trips 10 → 4; arm C 140.8 → 101.2 s, trips 58 → 20 | debug, COLUMNS at **0.1 mm**, 96 641 common columns, `grooved_block(2.5, 70°, 1.2)`. Before-run executed **twice** at the parent, byte-identical, so the deltas are not run-to-run noise — but each arm is still ONE simulation | `--test strategy_comparison_h4 -- --ignored` |
| B4 axial-DOC spike | 4.0722 mm on a commanded 0.200 = **20.4×** | debug, committed `tests/fixtures/test_job.toml` at 0.5 mm, generate→sim→generate→sim, 53 976 cutting samples | `--test simulation_issue_channel_m1 -- --ignored` (907 s) |
| pocket ring cascade divergence | ring 1 = 4194 mm² → ring 40 = 55 589 mm², **13×**, still not collapsed; first observed at **22.9 GB RSS without terminating** | debug, CW-wound 60 mm square. The sentry's 5 s deadline deliberately never lets the parent approach 22.9 GB | `--test adversarial_2d_campaign_r2` |
| adaptive cancellation latency | 2.292 s vs 0.007 s (v-carve) / 0.003 s (inlay) | debug, synchronous `run_op_precancelled` harness — **not** its racing twin, which is documented as a trap | same |

### 7.3 Quality and fidelity figures

| figure | value | population / resolution |
|---|---|---|
| D-16.1 off-footprint targets | **131 of 3287 mid-steep → 0 of 3133**; worst distance past the footprint **0.3750 → 0.0000 mm** | Tier-0, **no simulation**; 0.3750 mm is half a generation cell to four decimals, which is what identified the mechanism |
| D-16.1 residual | **−201.6 µm** on arm B with zero off-footprint targets; 477 of 96 641 columns (0.4936 %) past 50 µm; **100.0 %** within 0.25 mm of a profile break; 99.8 % on the rim side | COLUMNS at 0.1 mm, one simulation per arm, cross-arm grid identity **asserted** not assumed |
| D-16.1 contact evaluation (**W10**) | commanded Z exact to **0.0 µm**; drop-cutter `cl z = −0.000000` vs analytic `0.000000`; two triangles tie by facet contact; worst tip-below-surface across a 41-point sweep **0.0 µm** | **no simulation**, generation only, 13 429 moves, debug. §3 |
| ARP-1 tessellation | last-halving p99 ratio **3.93–4.02** against a predicted 4.00 across seven zones (κ 0.125–13.16 /mm); ≤ **0.66 µm** p99 | the law was **pre-registered** as `err ~ s²` before the sweep ran |
| ARP-1 vs alias floor | worst zone p99 **4.54 µm** at ε = 1 µm against a **50 µm** alias floor at the finest cell anyone would run | this is why the fixture stopped being the limit |
| repeatability V1 | **0 differing** of 146 775 evaluator values across two independent constructions | B-1 |
| alias proportionality | per-band p50 at 0.10/0.05/0.02/0.01 mm — shallow 14.60/7.35/2.94/1.47, mid 49.47/24.72/9.92/4.97, steep 161.60/80.80/32.32/16.16 µm | B-3, **smooth-ground bound only** (row **E-REP1**) |
| VerySteep reportability | **non-reportable at every cell down to 0.01 mm** (37 µm floor vs a 20 µm interesting difference) | B-5. Only shallow ground at ≤ 0.02 mm clears a 20 µm bin, and nothing in this repo has ever simulated finer than 0.1 mm |
| rest routing at tip scale | 0.10 mm cell finds **95.8 %** of the shipped cell's skeleton and routes **100 %** of it away | E9, cell ladder [0.5, 0.25, 0.1], `RestFieldParams` byte-identical to `rest_grid_resolution_c9`'s |

### 7.4 Feeds and gate figures

| figure | value | provenance |
|---|---|---|
| B3 identity residual | **+0.0000 %** through shipped code (+0.06 % against the logged live value) | the census's first hand-arithmetic gave +0.24 %; that gap was **the author's slip**, corrected under a dated banner rather than silently |
| unit-convention gap | **2.4×–40.4×, median 10.9×**, row-dependent | swept over all 65 `ae`-bearing rows in the embedded LUT — **not** a single constant |
| B3 flip (unit deletion) | 0.000737 mm (20 % of band min, `Within` + burn) → 0.009153 mm/tooth (127 % of band max, **`Exceeds(High)`**) | committed synthetic fixture, **not** the live op; the live band is wider |
| B3 flip (diameter law) | back to **`Within` at 79 % of max**, four commits later | the observation did not move; the **band** did, ×1.598 on a Ø0.954-against-Ø3.175 query |
| the second flip | `chipload_formula_calibration`'s "safe" 0.18 mm/tooth is **3.27× the band maximum**, and the pre-fix gate said `Within` | kept permanently as `the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum` |
| `is_extrapolated` rider | **932 of 11 712 (query, row) pairs = 8.0 %** would have silently lost their flag | swept 8 query diameters × 6 query Jankas over 252 shipped observations |
| rubbing floor | **3.47×** the matched row's derated band maximum on the red fixture | measured **through `feeds::calculate`**; the census's 3.31× was computed against the *undated* band |
| Suggest before/after | B3 `925.00 → 266.80 → 426.44` mm/min; Ipe Ø6 pocket `750.00 → 681.62 → 750.00`; Ø6 oak control `1093.91 → 1093.91 → 1059.41` | middle column = the floor commit, third = the law commit. The control **did not move** under the floor change, as pre-registered |
| power ceiling parity | Generic Wood Router 0.8000 → 0.6000 kW; Shapeoko VFD 0.3750 → 0.3000; Makita 0.7100 → 0.5680. **Recommended feed unchanged on all nine representative fixtures** | 90 sweep fixtures (3 machines × 10 species × Ø3/6/12 full-width slots); the `power_limited` branch **never fires**, peaking at 23.6 % of the unfactored ceiling |
| the rejected recalibration | multiplying the Step 6 clamp double-applies `safety_factor`: feed 723.4 → 542.5 mm/min (**−25 %**), literature cell `flat_6mm_pocket_al6061_lut` goes `major`, chipload falls to 0.0269 — within 10 % of the rubbing floor | verified **both ways** by reverting one line: 19/19 green unmultiplied, 18/19 multiplied |
| law magnitudes | `D^0.61`: pivot Ø6.35, median ×2.056 at Ø1, ×0.780 at Ø12. `Janka^-0.5`: median ×0.845 into softwood, ×1.650 into Ipe | 11 712 (query, row) pairs; the largest combined multipliers are **clamp-bound on the diameter axis**, i.e. row-selection artefacts, not law effects |

### 7.5 Drill figures

| figure | value | provenance |
|---|---|---|
| peck-count / cycle-time understatement | `peck_count` 4 → 5 (**+25 %**), `feed_time_s` 2.000 → 3.400 (**+41 %**), fed distance 10.0 → 17.0 mm | measured **off the emitted toolpath**, which corrected the audit's own static prediction of 33 % |
| the shipped contradiction | `Chip welding (D/d) exceeds: 7.33 vs 8.00` at `Severity::Caution` | quoted verbatim in the red commit `0afb51b`, which was deliberately left failing |
| Onsrud 72-000 wood band | 0.229–0.432 mm/tooth vs the matrix's 0.08–0.18 — **no overlap at any diameter** | chart retrieved 2026-08-04, 756 398 B, columns recovered from the PDF's own word bounding boxes; **gang drill at 4500 rpm**, a confound recorded rather than acted on |
| citation resolution | **17 of 32** rows did not resolve (10 hard 404s, 9 gold tier; 1 dead host; 6 literal `"(pending)"`), and all 32 reported `fresh` | every URL HTTP-tested individually, 2026-08-04 |
| freshness clock | `DEFAULT_TODAY` equalled the seed `last_verified` of **30 of 32** rows, so every row computed `age_months = 0` forever | fixed; the same rows now reach `warn` in June 2027 with no edit |

### 7.6 Instrument-integrity notes that travel with the numbers

1. **Cross-arm comparisons index by `(row, col)`, never by inverse-
   transformed XY**, and the grid identity is **asserted** before any
   cross-arm number is read.
2. **Never compare across regenerations.** A prior programme's
   "envelopes equal" finding was cross-run regeneration variance.
3. **`param_sweep -- --ignored` 56/56 is the fingerprint STOP**, run
   after every behavioural commit that could move geometry. It never
   triggered.
4. **A changed instrument makes its own docstring a lie you then cite** —
   check that both sides of a ratio are the same measure.
5. **A verdict bar on a population-changing fix passes vacuously.** Use a
   population bar. This was pre-registered by W5 and vindicated by
   D-impl-2.
6. **`mm²/s`** (area finished per second) is the tool-and-stepover-
   invariant efficiency measure; runtime alone is not.

---

## 8. Definition of done — plan §5, condition by condition

Fifteen conditions. **10 MET · 4 PARTIAL · 1 UNMET.** The one UNMET is
Checkpoint G, which is not this wave's and has not happened; four are
PARTIAL and each says exactly what is missing and where it is ledgered.

| # | condition | verdict | why |
|---|---|---|---|
| 1 | the three adaptive3d permanent reds have an approved, executed and sentried disposition; no inherited red is hidden behind an exception | **MET** | Checkpoint A ruled all three, PR-1..3 executed them, and the accepted-red allowlist is **empty**. `--lib` 2260 / 0 / 12. Zero tests were converted to `#[ignore]` to clean the count — measured, not asserted. `wanaka_suggest_baseline` is the single declared environmental exception (plan §2 rule 9), owned outside this programme, and is named rather than hidden |
| 2 | `FEEDS_CENSUS.md` reconciles B3 and identifies **one canonical physical model** with explicit legitimate stage differences | **PARTIAL** | B3 reconciles at **+0.0000 %** through shipped code, and the stage differences are now explicit and *executable* — `FeedExplanation` carries five labelled stages each with its own `unit()`, on every verdict. What was **not** built is a single canonical type: the model is one *answer* (the unit question) plus one *record*, not one implementation. §4 rows **F-P6**, **F-LUT2**, **F-T35**, **F-VBIT**, **F-BIPOLAR** are the remaining divergences, each with an owner. The census document itself is stale in four resolved-direction places and was deliberately not rewritten — row **F-CENSUS** |
| 3 | Suggest, gates and optimizer cannot **silently** use divergent chipload/RPM/power/deflection physics for equivalent inputs | **PARTIAL** | The word that carries this is *silently*, and on that reading it is close to met: the chipload axis is unified and the divergence deleted; the power ceiling has parity and its publication bug is fixed; deflection was already the one fully unified criterion; and every surviving divergence is documented in code at both sites and ledgered here. What is **not** true is that they are gone — Suggest and the gate still use different LUT entry points (**F-LUT2**), the power gate's cross-section still differs by ~1.23 % of a factor (**F-P6**), `arc_fit_ratio_for_op` is stale by 4–13× **inside Suggest** (**F-T35**), the viewport heat-map carries the old unit on a visible surface (**F-HEATMAP**), and no evidence measures a real optimizer retarget outcome moving (**F-OPT**) |
| 4 | all GUI-reachable 2D operation families have adversarial fixture coverage, bounded termination/cancellation evidence, and a ranked findings disposition | **PARTIAL** | Nine families × 22 fixtures × 11 hostile classes, each fixture proving it contains its mechanism before it gates anything; the pocket cascade is bounded by a **derived** cap (`extent / stepover`, which a convergent cascade cannot reach) and asserted to stop by `RingCount` rather than by wall clock; all nine honour a pre-set cancel flag; 13 findings ranked and dispositioned at Checkpoint C. Missing: **165 of 198** matrix cells were never run (**A2D-165**), and `AlignmentPinDrill` and `Chamfer` are outside the nine and remain uncancellable (**O-CANC**) |
| 5 | a cascade/library failure cannot silently masquerade as a successful collapsed/empty operation | **PARTIAL — met in debug, unmeasured in release** | `offset_polygon_reported` separates `Collapsed` / `RejectedInput` / `LibraryFailure`; the boundary layer **refuses** on a library failure and passes through only on a genuine collapse, with a typed finding naming the dropped containment; `offset_library_failures` and `boundary_clip_dropped` publish it. But two of the three panic classes are `debug_assert!`s in transitive dependencies, so **in release the failure is not detected at all** and the library proceeds on unvalidated input. Accepted and documented at Checkpoint C Q4; **measured nowhere**. §6 steps 14–17 are the first legitimate opportunity, and until they run the honest verdict is PARTIAL |
| 6 | simulation diagnostics present bounded typed findings and say when a requested metric is not measurable at the selected resolution | **MET** | `SimulationTriage` is one contract with one construction site, consumed by GUI, MCP, CLI and narration; advisories capped 10/toolpath and 50/project with a true pre-cap `total_matching`; safety events share no list with advisories and are never deduped away. `NotMeasurable` gates abstain with a stated reason and collision detection stays live. All four of the plan's acceptance bars pass **as tests** |
| 7 | drill verdicts agree with evidence and their cited threshold sources are fresh/traceable | **MET** | Four shipped verdict/evidence contradictions fixed red-first, with the wrong string quoted verbatim in the test that failed on it. The peck model now matches the emitter. Sources: the two documents cited for every peck threshold were retrieved and **contain nothing** on the subject — so the thresholds are declared repo-authored in code and `CREDITS.md`, six URLs were repaired by retrieval, two misattributed keys renamed, and the freshness clock — which could not previously report `stale` at all — now runs. Ten dead URLs **outside** drill scope survive and are ledgered (**DR-URL**) |
| 8 | the GUI worker's findings path is structural rather than field-by-field copy debt | **MET** | One core-owned join (`compute::stats::stats_with_findings`) with three compile-time guards; both call sites collapse to one line. The gate was proven on the **parent** first — the probe builds clean at `d40768a` and is an `E0027` after — so the defect is reproduced, not inferred. Three boundaries downstream remain unguarded and are ledgered (**S-GAP3**) |
| 9 | arcfit does not cross intent boundaries, and every affected fitted-path fingerprint has an evidence-backed disposition | **MET** | Intent is a strict run-key term and `Region` edges are a local barrier. Exactly **one of five** transform-provenance constants moved, by **label only**, count flat at 40, with the mechanism and the exact build recorded. The defect's own discoverer closed: 21 phantom lost cut positions → 0. Arc-count cost measured at **ZERO**. Production-scale cost is unmeasured and ledgered (**S-COST**) |
| 10 | D-16.1, D-16.2, D-LV.1, B4, remaining clip visibility and agent-read debt are either fixed with sentries or explicitly deferred to a named decider/condition | **MET** | D-16.1 mechanism fixed and sentried, quality claim deferred with a probe named to the column (**D161-Q**, narrowed again by §3); D-16.2 fixed on both affected bands and sentried; D-LV.1's classification half fixed and sentried, the near-empty symptom explicitly **not claimed** and deferred with the discriminating measurement named (**DLV1-NE**); B4 closed as diagnosed with its actionable half given a channel; clip visibility censused and deferred (**R7-L1**); agent-read debt split — C1 and C6 shipped, B7's real defect (narrating the wrong tool's geometry) fixed red-first, C2–C5 **deferred in writing with its inherited justification retired** (**C25**) |
| 11 | a procedural reference fixture has qualified the bins and fixture classes used for any reopened strategy/quality question; B1/B2 remain closed unless Checkpoint E rules otherwise | **MET** | ARP-1 shipped with C6 bit-identity proofs, analytic truth everywhere, and tessellation error an order of magnitude below the alias floor. Checkpoint E ruled **B1/B2 stay closed**, so no strategy/quality question was reopened and none needed a bin. Stated plainly rather than glossed: **no bin was adopted**, because the arm that sets it (B-2) is unrun and B-5's region is only a floor (**B2-BIN**). The programme published **no strategy winner** |
| 12 | each ignored mega harness has a named split/archive/schedule policy and owner | **MET** | Two SPLIT→ARCHIVE with reusable loaders extracted; one SCHEDULED with the finishing lane as owner. `MEGA_HARNESS_POLICY.md` carries the four standing rules, including the one that forbids merging the two instrument families in bulk. The scheduled harness's cadence is itself ledgered (**H4-SCHED**) because a schedule nobody honours is a rot vector |
| 13 | bounded scouting has delivered a ranked next-programme map without scope creep | **MET** | 20 risks with `file:line:symbol`, 12 ruled out with evidence, dispositioned 3 / 17 / 12. Zero production, test or schema change and **zero Cargo commands** in the scouting lane. The three `fix now` items were escalated to the operator rather than fixed in-lane; two were then ruled and executed by a separate lane, and the third was **downgraded to `ruled out (no live population)`** rather than fixed — which is the right outcome for a risk with an empty population |
| 14 | every behavioural output/default/operator-number change passed its checkpoint, focused gates, documented re-pins, and **final read-only live validation** | **UNMET** | The first three clauses are met without exception: eight checkpoints ruled and executed; every behavioural change red-first with the parent failure quoted; every fingerprint move carrying old/new, mechanism, exact build and a consumer census (three moved in the whole programme, each explained). **The fourth clause has not happened.** Checkpoint G is not this wave's — §6 is its plan, and until it runs this condition is UNMET and must not be reported otherwise |
| 15 | `cargo fmt --check` and zero-warning workspace clippy pass, and `PROGRESS.md`, `FEATURE_CATALOG.md`, `AI_MACHINIST_ANALYSIS_REFERENCE.md`, `CREDITS.md`, tracker and close-out accurately describe the resulting surface | **MET** | `cargo fmt --check --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (only the pre-existing `nom` / `quick-xml` future-incompat note). All six documents updated this wave (`fdb0ae6`) — nine falsified claims corrected at source, with dated errata rather than rewrites on the three that are records. `CREDITS.md` gained the drill lineage and declares both scaling exponents repo-derived in those words |

### The honest summary

The programme did what its executive decision said it would: it restored
a trustworthy baseline first, then censused the feeds stack, and it did
not change a machining behaviour while measuring the instrument that
would judge it.

Four conditions are PARTIAL, and three of those four are the same shape —
**a divergence that was silent is now documented, owned and executable,
but is still a divergence.** That is a real improvement and it is not the
condition as written. The fourth (release behaviour) is PARTIAL because
the measurement is scheduled and has not been taken.

One condition is UNMET because the operator has not yet authorized the
live validation. Nothing in this document should be read as anticipating
its result.
