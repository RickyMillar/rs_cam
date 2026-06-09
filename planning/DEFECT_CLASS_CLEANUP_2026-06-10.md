# Defect-class cleanup — tool_load / feeds / optimize pipelines

**Started:** 2026-06-10
**Branch:** `defect-class/cleanup`
**Origin:** Live optimization run on WANAKA (2026-06-09) surfaced four issues; root-cause
investigation found they are instances of five systemic defect classes, not isolated bugs.
Full investigation reports are summarized per-class below.

**Method:** fix the four confirmed instances now (Tracks F1–F4), while audit agents sweep
each defect class across all op families / data files to find further instances (Tracks A1–A4).
Every fix lands with a regression test; every audit finding gets triaged into this doc.
Validation bar: industry norms already in the repo's literature net (FPL Wood Handbook,
Onsrud drill bulletins, Vectric defaults, Amana published chiploads — see
`crates/rs_cam_core/tests/literature_matrix/`).

---

## The five defect classes

| # | Class | Confirmed instance | Generalizing audit |
|---|-------|--------------------|--------------------|
| C1 | Bad data wearing a "validated" badge — LUT rows pass the loader with degenerate/incomplete content | `whiteside_fusion360.json` tapered-ball rows with `chipload_min == chipload_max == 0.1016` (a Fusion360 nominal preset encoded as a range) win lookups and hard-block the optimizer with `Confidence::Validated` | A1 |
| C2 | Write-path clobbers — per-op setter aliases + ordered field application silently overwrite earlier writes | `DrillConfig`/`AlignmentPinDrillConfig` alias `set_feed_rate` and `set_plunge_rate` onto one field; `apply_feeds_subset` writes plunge last, clobbering the drill-tuned feed with the milling plunge baseline | A2 |
| C3 | Untagged samples polluting gates — entry/link/transit moves without span tags count as steady state | adaptive3d entry/re-entry sample with ~3× commanded axial DOC at slot arc drives a 622 µm deflection verdict (honest reading ≈ 98 µm) → spurious `DeflectionSetupLocked` | A3 |
| C4 | Signals plumbed but dead — fields computed and serialized but never consulted at the decision point | `Confidence`/`ChipBoundsSource` never read by `candidate_is_safe`; `pass_role` ignored by deflection gate despite rough/finish limits existing in `cutter_constraints`; `AxisPatch.clamped` not serialized onto candidates | A4 |
| C5 | Duplicated evaluation that drifts — same judgement implemented ≥2× with different inputs | 3 LUT row resolvers (gate / optimizer context / retargeter); 2 bending sections (solid-D gate vs 0.7·D-core Suggest, 4.16× apart); narrate re-derives drill envelope check instead of consuming `DrillGatesVerdict` | A4 |

Cross-cutting (tracked but not a class of its own):
- One `MachineProfile::max_feed_mm_min` field conflates GRBL travel rate ($110) with cutting-feed ceiling → optimizer can propose cutting at travel speed (Track F4).
- `mem::replace` placeholder session during optimize runs makes MCP inspectors report default state (observability hole; deferred — see Backlog).
- `RefuseReason` conflation: cancellation surfaces as `NoSafeImprovement`, `SteadyStateSamplesNotPresent` covers 3 unrelated situations (Backlog).

---

## Fix tracks

### F1 — Drill feed clobber + floor-grazing defaults  — STATUS: ✅ DONE 2026-06-10

Confirmed behaviour: both WANAKA drill TPs run at exactly 50.0 feed/Ø — the SolidWood
rubbing floor. Literature band for hardwood drills is 0.08–0.18 mm/tooth ≈ 213–427 feed/Ø
(`literature_matrix/cells.toml:415-440`, sources: onsrud_drill / vectric_drill_default /
fpl_wood_handbook).

Sub-fixes:
1. Stop the clobber: `feeds::calculate` sets `plunge_rate_mm_min = feed_rate_mm_min` for
   `OperationFamily::Drill` so the aliased setters are write-order-safe
   (`crates/rs_cam_core/src/feeds/mod.rs`, `suggest.rs:727-728`).
2. Envelope sanity clamp on suggested drill feed: pass the chipload-derived feed through
   `material.drill_plunge_feed_envelope_per_mm() × diameter`; emit `FeedsWarning` when it
   binds (same convention as `RUBBING_FLOOR_MM_TOOTH`).
3. Kill hardcoded `feed_rate: 300.0` defaults (`operation_configs.rs:152,199`): route the
   pin-drill auto-create (`rs_cam_viz/controller/events/model.rs:529-552`) through the
   suggest funnel like GUI/MCP `add_toolpath` already does.
4. Chip-welding gate credits pecking: classify Peck/ChipBreak cycles on per-peck D/d
   (already computed for `peck_pattern_adequate`), not total-hole D/d
   (`drill_metrics.rs:261-270`). Add edge test at exactly 0.75×; document band as `[0.75t, t)`.
5. Reporting hygiene: split `DrillGateOutcome::Within.threshold` into `envelope_lo`/`envelope_hi`
   (`drill_gates.rs:24-38`); narrate consumes `DrillGatesVerdict` instead of re-deriving
   (`narrate.rs:1043-1065`); unify unit string to "mm/min per mm Ø"; add source citations
   to `material.rs:1105-1122`.
6. Lit-matrix invariant on **final op feed** implied chipload (`feed_rate / (rpm × flutes)`)
   at Major severity, so a clobber regression fails `flat_3mm/6mm_drill_*` cells
   (today the clobber is invisible to the matrix — plunge-fraction row reads 1.0 by construction).

Validation: drill suggest feeds land in the literature band for the cells.toml drill cells;
WANAKA pin drill + holes re-suggest off the floor.

**Landed** (commits 648b8db, 089c209 on `defect-class/f1-drill-feed`):
- Sub-fixes 1-6 as specified, plus **F1.7** (from A4): drill gates join
  `ToolpathLoadVerdict::criteria()` — Critical drill exceedances block g-code export;
  Elevated stays a warning band (mapping documented on
  `DrillGateOutcome::as_criterion_status`). `milling_criteria()` split preserves the
  not-applicable partition and the `any_unmodeled` drill special case.
- Step 9c addition discovered during F1.6: when the envelope CEILING binds, RPM
  follows the feed down (bounded by the drill band floor) so the shipped recipe
  holds its commanded chipload instead of thinning toward rubbing (Ø3 oak: implied
  0.043 @ 14k pre-fix → 0.075 @ 8k). Spindle speedup rolled back proportionally.
- Lit-matrix: 7 drill cells × 2 critical final-feed invariants (envelope band +
  rubbing floor); drill plunge/feed fraction expectation corrected to 1.0 (feed IS
  plunge — the 0.40-0.70 milling band failed every drill cell as advisory noise).
- `DrillToolpathSummary.chip_welding_dtd` added (evacuation-credited ratio the risk
  classifies from); cut-trace schema v5.
- Deferred to backlog from A2: `set_toolpath_param("plunge_rate")` provenance lie on
  drill ops; CLI run/job raw defaults; SteepShallow z_step hint/setter asymmetry.

### F2 — phantom/untagged samples polluting gates — STATUS: ✅ DONE 2026-06-10

A3 audit reframes sub-fix 1: do the `MoveIntent`→span bridge (one pass in
`compute/spans.rs`, appended in the four `generated_with_*` helpers) which fixes all
22 ops × all 3 gates, AND honor `spans_valid` at the 4 consumer sites that currently
stamp TSP-corrupted ancestry (`compute/simulate.rs:471`, `gcode/mod.rs:399`,
`optimize/mod.rs:171`, `optimize/candidate.rs:359`) — the TSP corruption is the likely
actual mechanism behind the 622 µm sample. Original framing below kept for context.

**Landed** (commits 25f5325, 84fcf6f, 102c480 on `defect-class/f2-phantom-samples`):
- F2.1: `spans_from_move_intents()` bridge (Linking→LinkBridge, Entry*/LeadIn→Entry,
  LeadOut→LeadOut), appended in all four `generated_with_*` helpers; the
  span-coverage matrix now asserts per-move intent↔span agreement for every family.
- F2.2 **(scope widened during implementation)**: honoring `spans_valid` by
  wholesale degradation broke the F-031 sentries — TSP's all-or-nothing
  `spans_valid=false` poisoning was itself the defect (one split span discarded the
  still-correct Entry/WaterlineCleanup spans → tagged transients became untagged →
  the 622 µm family). TSP now DROPS exactly the spans it split (survivors stay
  valid); the metrics stamper unions intent-derived transit classification (intents
  survive reordering); `is_phantom_transit` honors `in_transit_span` for
  ancestry-less samples even with a span lookup present. The 4 consumer sites still
  honor `spans_valid` as defense against legacy invalidators.
- F2.3: preflight evaluates the canonical closed-form deflection at the
  minimum-force corner (hard-floor DOC × hard-floor stepover) before refusing
  `DeflectionSetupLocked`; Stage F dispatches per-gate retargeters on ANY Exceeds
  gate (`any_load_gate_exceeds`) — `DeflectionDocRetargeter` no longer dead,
  `PowerFeedRetargeter` no longer starved on power-only-Exceeds, and such baselines
  no longer route to the headroom scale-up. Prescription states both the 200 µm
  Exceeds limit and the 50 µm stickout-sizing target; structured
  `peak_um`/`bound_um`/`target_stickout_mm` land on
  `OutcomeNarrative.deflection_setup`; stale L/D>6 `RefuseReason` prose replaced.
- F2.4: detector test `axial_engagement_vs_dpp_detector_f2.rs` — peak steady-state
  axial vs 1.5× commanded DPP across pocket/profile/adaptive/zigzag/trace through
  the production generate→simulate funnel (project fixtures rather than the raw
  param-sweep harness, which never produces metric samples). Dexel-bridge teeth
  remain F-031's whole-toolpath bar, which demonstrably trips on filter regressions.
- Deferred: live WANAKA re-validation via MCP (TP1 expected ~98 µm Within +
  optimizer searches instead of refusing) — pending a GUI session.

Confirmed behaviour: TP1 Back Rough peak sample carries ~9 mm axial engagement at slot arc
(commanded DOC 3.0) → 622 µm verdict → preflight `DeflectionSetupLocked`. Honest reading
≈ 98 µm (Within). Same family as F-024.

Sub-fixes:
1. Clamp/filter: in the deflection gate, exclude or clamp per-sample `axial_engagement_mm`
   to commanded depth-per-pass + tolerance, OR extend span tagging so adaptive3d
   entry/re-entry moves carry `Entry` ancestry and hit the existing advisory reroute
   (`deflection.rs:218-227`, `locality.rs:180-225`). Prefer tagging at emission —
   it fixes every gate at once, not just deflection.
2. Preflight computes instead of asserts: evaluate the closed-form model at the
   minimum-force corner of the search space before refusing (`preflight.rs:37-60`);
   dispatch per-gate retargeters on ANY Exceeds gate (today only chipload-Exceeds
   dispatches; `DeflectionDocRetargeter` is dead code — `optimize/mod.rs:297-308`).
3. Reconcile prescription: refusal targets 50 µm while citing the 200 µm limit
   (`refusal.rs:27,35`); fix stale L/D>6 prose (`tool_load/mod.rs:166-168`); promote
   peak_um / bound_um / target_stickout_mm to structured fields.

Out of scope (deliberately): recalibrating the deflection model itself. The 50/200 µm
bounds + un-multiplied shear-parallel Kc + solid-D section are co-tuned into a working
stiffness index. Touching one constant alone breaks discrimination. Needs its own
calibration effort with real measurements — see Backlog.

### F3 — degenerate Whiteside LUT rows hard-blocking the optimizer — STATUS: pending

Sub-fixes:
1. Demote the four `min==max` rows in `whiteside_fusion360.json`: drop their
   `chipload_min_mm_tooth` (gate already handles min-less rows honestly: "missing lower
   bound means burn cannot be modeled, not that we invent one" — `chipload.rs:376-391`).
   Keep them as RPM/nominal references.
2. Loader validation in `feeds/vendor_lut.rs`: reject `min == max` (or width < threshold
   fraction of midpoint) as a *range*; test that all embedded observation files pass.
3. Provenance-aware gating: extend `ChipBoundsSource` (point-preset / scaled / missing-ae);
   when the LOW side would trip on weak-provenance bounds, return `Within` + structured
   burn advisory instead of `Exceeds(Low)`. High side stays hard (breakage). Existing
   `MarginalSafe` tier is the natural landing zone for such candidates.
4. Unify LUT resolution: one shared `matched_chip_envelope(...)` helper consumed by the
   gate (`chipload.rs:334-360`), optimizer context (`optimize/context.rs:109-142`),
   preflight bipolar check (`preflight.rs:62-80`), and retargeter (`optimize/mod.rs:474-485`).
   Kills the "advice computed against a different envelope than the verdict" class
   (the 38198 mm/min narrative).

Validation: TP7-style drop_cutter finish candidate (structurally safe, 4.5× faster) is no
longer refused on burn-side chipload from preset-point bounds; Amana tapered rows
(real ranges 0.010–0.032 + ae calibration) win tapered-ball lookups.

### F4 — travel rate vs cutting-feed ceiling conflation — STATUS: pending

Sub-fixes:
1. Split `MachineProfile`: `max_feed_mm_min` (travel, $110-class — keep name for serde
   compat) + new `max_cutting_feed_mm_min: Option<f64>` (optimizer/suggest/modulation
   ceiling; None → derive conservative default, e.g. min(travel, preset cap)).
2. Plumb through `optimize/bounds.rs:227`, `strategy/headroom.rs:76`, retargeter clamps,
   and suggest.
3. `OptimizeOutcome` carries a `machine_snapshot` (name + caps used) so narratives are
   reconcilable with what the run actually consumed.
4. Surface `machine_library::resolve` silent library-override with a warning
   (`session/project_file.rs:992`).

Validation: ricky-XXL profile (travel 10000) can't be told to cut hardwood at 10000;
machine_kinematics.md guidance updated.

---

## Audit tracks (parallel agents, read-only)

| Track | Scope | Status |
|-------|-------|--------|
| A1 | All `data/vendor_lut/observations/*.json` | **done 2026-06-10** |
| A2 | All 23 op configs: setter aliases, apply order, round-trips | **done 2026-06-10** |
| A3 | Sample tagging vs gate steady-state filters, all 22 ops | **done 2026-06-10** |
| A4 | Dead signals + duplicated evaluation in tool_load/feeds/optimize | **done 2026-06-10** |

Findings get appended below as they come in, triaged into: fix-now (joins F-tracks),
backlog, or working-as-intended.

### A1 findings — vendor LUT data (252 rows / 20 files audited)

Confirmed worse than the original instance. Triage:

**BLOCKER (joins F3):**
- `whiteside_fusion360.json` — ALL 13 rows `min==max` 0.1016 (Fusion360 generic
  0.004"/tooth preset), grade-a/exact, no ae/hardness. Score-replay confirms
  `whiteside-sc64` WINS tapered-ball/hardwood/parallel/finish (1813 vs 1705 for the
  calibrated Amana row) returning Validated chipload ~15× the Amana-scaled value;
  `whiteside-ud2102` (compression) wins Profile-on-plywood 4.5× hot. Disposition:
  drop or demote to grade-c + `row_kind: fallback`.
- **NEW blocker class — chipload-less rows that win and hard-refuse the gate**:
  `whiteside_rpm_assorted.json` (4 RPM-only rows, `row_kind: exact`) — three win real
  queries and force `Unmodeled(NoVendorData)` even though usable rows exist underneath
  (60° V-bit hardwood trace; plywood adaptive 9.525 3F; hardwood adaptive 12.7 3F via
  EMBEDDED_FILES iteration-order tie-break). RPM-only rows must not be selectable by
  the chipload gate.

**MAJOR:** 62/252 rows (25%) degenerate `min==max`; 194 lack ae; 176 lack hardness
(worst: `amana_long_tail.json` 26 spektra single-points, `amana_compression.json`,
`helical_aluminum.json`). `onsrud_ocr.json` (47 rows): INDUSTRIAL chiploads
0.254–0.483 mm/tooth, no machine-class derate, no RPM — and they are the ONLY wood
contour finish/semi-finish rows (every Waterline/SteepShallow flat-end wood query lands
on them as Validated). `freud_solid_carbide.json` half-inch rows to 0.69 mm/tooth —
suspect OCR derivation, re-verify. The 9 `facing_bit` rows are UNREACHABLE dead data
(Face op declares `feeds_family: Pocket`). `lookup_best` tie-break depends on
EMBEDDED_FILES order.

**Coverage gaps forcing `Unmodeled(NoVendorData)`:** drill family (empty by design but
the slot exists); **ball_nose × contour** (the canonical Waterline 3D combo!); flat_end
× parallel; flat_end × trace (wood); bull_nose finishing; plastics scallop; aluminum
parallel; fiberglass beyond pocket.

**8 proposed loader validation rules** (degenerate-range rejection, chipload-required-
for-exact-kind, grade-A provenance denylist for CAM presets, calibration completeness,
per-diameter plausibility band, conflict detector, reachability check, deterministic
tie-break) — implement with F3.2.

### A2 findings — config write paths (23 op configs audited)

**BLOCKER (= F1.1, numerically confirmed):** drill suggested feed 4000 → stored 595
(plunge clobber) on ALL funnels: GUI add, MCP add, Apply-all, Apply-speeds, CLI
--apply-suggest. UI shows 4000 while apply stores 595; provenance stamps "suggested" on
what is actually the plunge baseline. The lit-matrix binds the calculator result, never
the applied config — clobber invisible to the regression net (= F1.6).

**MAJOR:** (= F1.2) calculator never clamped drill feed to the envelope — the
un-clobbered 4000 exceeds 2400 (Ø6 wood hi); a naive clobber fix ships gate-failing
configs → both fixed together in Step 9c. `set_toolpath_param("plunge_rate")` on drill
ops rewrites feed through the alias but stamps only PlungeRate provenance (backlog).
(= F1.3) pin-drill auto-create bypasses suggest.

**MINOR:** SteepShallow `z_step` is a calculator input hint with NO write-back setter
(suggested axial DOC can never land; self-referential hint loop); RadialFinish suggested
WOC silently dropped (no-op trait setter); Pencil conditional-getter asymmetry; CLI
run/job + legacy import create from magic defaults; DropCutter `set_scallop_height`
mode-flip via optimizer axis; `TaperedBallPlungePreFix2` would alias-clobber a drill
holding a tapered-ball tool; `apply_axis_patch_to_op` silently no-ops undeclared axes.

**6 proposed invariant tests** (setter-independence matrix w/ alias allow-list,
family-appropriate apply, hint/setter symmetry, apply-order commutation, no-bypass
creation sentry, provenance honesty) — adopt #2/#5 with F1, rest with backlog.

### A3 findings — sample tagging vs gate filters (22 ops audited)

**Reframes F2.** Two BLOCKER mechanisms, both broader than adaptive3d:

1. **`MoveIntent` is the missing bridge.** Every pollution site already carries correct
   per-move intent (`Linking`, `EntryPlunge/Helix/Ramp`, `LeadIn/Out`) — but gates read
   only `SpanKind` ancestry, and only adaptive3d tags any transient at emission. Fix:
   one `spans_from_move_intents()` pass in `compute/spans.rs` appended in the four
   `generated_with_*` helpers (`execute.rs:40-60`) — fixes all 22 ops × all 3 gates.
2. **TSP corrupts spans and nobody checks.** Default-on TSP sets `spans_valid=false`
   when it splits a non-Operation span (`tsp.rs:478-489`); `compute/simulate.rs:471`,
   `gcode/mod.rs:399`, `optimize/mod.rs:171`, `optimize/candidate.rs:359` consume
   `.spans` without checking — samples stamped with WRONG ancestry (a tagged Entry can
   become effectively untagged). Likely the actual mechanism behind the 622 µm case.

Pollution-capable untagged kinds: adaptive3d at-depth `Link` re-entries (full feed,
Helix kinematics) and 2D adaptive `Link` at cut_depth (full feed — passes even
chipload's 0.95× filter). Deflection most exposed (no feed filter, feed-independent
force, peak-driven); power next; chipload partially shielded. The other 20 ops are
clean (pure-vertical plunges are kinematics-quarantined; note the 1-nm XY-drift cliff
in `classify_cut_kinematics` as residual risk). No existing harness surfaces per-sample
axial vs commanded DPP — ~50-line detector test over param-sweep fixtures proposed
(flag peak steady-state axial > 1.5× commanded DPP).

F2 fix order: (1) intent→span bridge, (2) honor `spans_valid` at the 4 call sites,
(3) only then the defensive gate clamp (landing it first would mask real overloads),
(4) detector regression test.

### A4 findings — dead signals + duplicated evaluation

**CLASS A (High first):**
- `ToolpathLoadVerdict.drill_gates` is DISPLAY-ONLY: `criteria()` excludes drill gates,
  so export gating / readiness / `any_exceeded` all ignore them. A Critical chip-welding
  or plunge-feed exceedance does NOT block g-code export while an equivalent milling
  trip does. `DrillGatesVerdict::any_exceeded()` has zero production callers. → **F1.7**.
- `OptimizeCandidate.reconciled_verdict` written by U4 reconciliation, never read — a
  candidate that flips to Exceeds in the post-Apply project sim is invisible (only
  cycle-time mismatch flagged). Safety-relevant. → F4 follow-up.
- `Confidence::Approximate` + `ChipBoundsSource::VendorLutExtrapolated` display-only /
  fully-dead — optimizer auto-recommends on extrapolated bounds with full authority
  (= F3.3).
- `pass_role` dead for the deflection gate (rough/finish limits exist in
  cutter_constraints; gate hard-codes 50/200 for every pass) — fold into F2.3.
- Fully dead: `AxisPatch.clamped`, `CandidatePatch.{strategy,rationale}` (docs claim
  "surfaced in MCP/GUI" — false), `AxisBounds.sources`, `FeedsResult.{ramp_feed_mm_min,
  vendor_source}` (ramp feed never reaches any operation).

**CLASS B:**
- LUT row resolution is FOUR-way duplicated (not 3): gate / optimizer context /
  **viewport envelope map** (`tool_load/mod.rs:198-284` — first-match, UNFILTERED max
  axial DOC incl. phantom transits, un-derated bounds → operator-facing colors can
  disagree with the export verdict) / suggest. V-bit angle gate skipped by two. (= F3.4,
  scope widened.)
- Envelope semantics drift: gate derates bounds + arc-normalizes samples; retargeter,
  preflight-bipolar, viewport use raw bounds → systematically wrong retarget multipliers.
- Deflection model FOUR implementations (stepped integrator canonical; 0.7·D-core
  closed form +36% bias; feed_modulation uniform cylinder; retarget cube-root seed).
- Preflight ↔ DeflectionDocRetargeter contradiction confirmed (= F2.2); same dispatch
  gap starves PowerFeedRetargeter on power-only-Exceeds baselines.
- Power retargeter recomputes available power instead of reading `available_kw` off the
  verdict it consumes.
- Drill plunge envelope check duplicated in narrate (= F1.5, FIXED — narrate now calls
  `classify_plunge_feed`).
- Plunge cap literals in `feeds/mod.rs:1251-1261` mirror `plunge_stress.rs` named
  constants — feeds should call `safe_plunge_cap_mm_min` (backlog).

---

## Backlog (acknowledged, deliberately deferred)

- Deflection model recalibration: force model is feed-independent (`F = Kc·ap·ae`,
  drag-cut physics, ~16–60× energy-balance force at WANAKA params), offset by co-tuned
  constants. Needs physical measurements or a literature-anchored recalibration as one
  coordinated change. Until then: document the metric as a tuned stiffness index.
- `RefuseReason` enum split: cancellation ≠ `NoSafeImprovement`; `SteadyStateSamplesNotPresent`
  covers three unrelated situations (drill-kinematics skip, missing cycle time, genuine
  no-steady-state). Engraving-aware steady-state mode for shallow ops (project_curve at
  0.1 mm DOC) so they get cycle-time-only optimization.
- Placeholder-session swap (`controller/events/mod.rs:427,984`): MCP inspectors read an
  empty default session during optimize runs. Fix: read-only snapshot or "optimize in
  flight" response.
- Baseline/candidate verdict asymmetry: baseline scored from production trace, candidates
  from optimizer-internal sims with modulation off + coarser Stage-1 resolution → phantom
  Improved/Worsened near thresholds.
- Magic numbers outside `SearchPolicy`: rank penalties (`rank.rs:77,96,113`), suggestion
  margins, `STEADY_STATE_FEED_FRACTION`, duplicated deflection headroom 0.75, hardcoded
  `project_default_rpm: 18_000`.
- Headroom strategy on discrete spindles: RPM snap breaks constant-chipload premise while
  rationale claims uniform scaling (`headroom.rs:157-177`).

## Decision log

- 2026-06-10: branch from master (ia-cleanup viz deletions stashed:
  `git stash list` → "ia-cleanup dead-code-sweep viz deletions").
- 2026-06-10 (F1.7): drill `Exceeds(Elevated)` maps to `LoadState::Within` — only
  Critical blocks export. Rationale: Elevated is the documented warning band
  (chip-welding approach zone, plunge-feed rubbing side); blocking export on it
  would refuse working-but-suboptimal programs. The dedicated drill badges still
  surface Elevated.
- 2026-06-10 (F1.6): when the plunge-feed envelope ceiling conflicts with the
  literature chipload band (small drills: Ø3 oak band wants 2240-5040 mm/min, envelope
  caps at 1200), the machine-safe envelope wins and RPM follows down to preserve chip
  thickness — bounded by the drill band floor. The cells' `fpt` band continues to bind
  the calculator's target; the new invariants bind the final applied values.
- 2026-06-10 (F1): chip-welding credit for Peck cycles = deepest single peck governs
  (full retract clears flutes; Onsrud peck guidance), ChipBreak = half total (matches
  `chip_evacuation_score`'s 0.5 factor), Simple/Dwell unchanged. No new constants
  invented.
- 2026-06-10: fix order F1 → F2 → F3 → F4 chosen by real-world payoff: drills currently
  cut at the rubbing floor (burn risk on every default drill op); F2 converts a locked
  toolpath to optimizable; F3 unblocks a 4.5× cycle-time win; F4 makes the unblocked
  win safe to apply.
- 2026-06-10 (F2.2): TSP `remap_spans` drops split spans instead of flipping
  `spans_valid=false` for the whole vector. Rationale: wholesale invalidation
  discards every still-correct span; the planned consumer-side degradation (pass
  None / empty ancestry) then un-tags the valid Entry/WaterlineCleanup transients
  and reproduces the exact pollution F2 exists to fix (F-031 sentries fail under
  it). Dropping only the spans whose remapped bounds are wrong keeps survivors
  trustworthy; the dropped spans' moves keep transit classification via the
  per-move `MoveIntent` union in the stamper (intents travel with moves through
  reordering). Consumer-site `spans_valid` checks retained for legacy invalidators.
- 2026-06-10 (F2.3): `DeflectionSetupLocked` threshold = the closed-form corner
  must clear the bare 200 µm Exceeds bound (no headroom factor). Preflight asks
  reachability, not quality — candidates near the corner still get sim-verified
  and ranked; an over-strict corner test would re-introduce the assert-style
  refusal. The 50/200 µm constants themselves untouched (out of scope per doc).
- 2026-06-10 (F2.4): detector implemented over the ux project fixtures through the
  production session funnel instead of the `param_sweep` harness — the sweep
  harness drives generators directly and its sims don't produce metric samples,
  so `axial_engagement_mm` doesn't exist there. Same coverage intent, honest
  signal path.
