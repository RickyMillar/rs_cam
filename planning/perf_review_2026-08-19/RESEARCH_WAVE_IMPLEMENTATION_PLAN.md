# Implementation wave plan — from the 2026-08-21 research wave

Status: EXECUTING 2026-08-21 (user approved "go, defaults"). Live status is
tracked in the EXECUTION STATUS section at the bottom of this file. Written 2026-08-21 by the orchestrating session so
post-compact execution is deterministic. Source docs (all in this directory,
unstaged like this file): `RESEARCH_corpus_instruments.md`,
`RESEARCH_drill_intent_erasure.md`, `RESEARCH_boundary_clear_parity.md`,
`RESEARCH_f2_and_aba.md`. Corrections ledger stands at SIXTEEN.

## Gate 0 — before ANY lane starts

- The concurrent G-code session must be finished (it owns the tree; nothing may
  be committed until then). Its visible artifact: untracked
  `crates/rs_cam_core/tests/safe_z_emission_frame_g_safez_local.rs`.
- User go on the wave, plus the decision table below (D1–D4). Lanes marked
  [needs Dx] cannot start without that decision; everything else can.

## Decision table (user)

| # | Decision | Default recommendation |
|---|----------|------------------------|
| D1 | Drill C1 — thread per-op `rebuild_clearance_z` through TSP. **Machine-visible motion change** (G-code differs; peck re-entry returns to R-plane). | Approve, but only AFTER sentry A is red on master so there is a before/after; sequence after/with G-SAFEZ-LOCAL, never concurrent. |
| D2 | AS015 baseline row: re-cut, never diff (old row was cut under the deleted fresh-stock fallback — it is a fresh-stock scallop under a rest label). Add a resolution column to `BaselineRow`. | Approve both. |
| D3 | F2 bar shape: adopt C1 (local-commanded-step ratio, ceiling 1.35, pass-0 abstains) + C3 (new sim-free ladder sentry under its own name); do NOT adopt W5B-F2 as filed (reads green on both pre-fix defects). C2 (band_max/interior_p99) deferred until measured. | Approve C1+C3. Implementation step 1 is PRINT THE LADDER — a short final pass would make C1 a silent tightening; if one exists, bar shape comes back to user. |
| D4 | Boundary FIX (beyond the sentry): inset the pre-clear to `containment⊖r` + give `waterline_cleanup` a boundary parameter. Changes generated geometry near containment edges. | Sentry this wave; fix as its own later decision package with before/after from the sentry's ignored B1==0 arm. |

## Clusters and sequencing

All clusters are mutually independent EXCEPT drill C1 × G-SAFEZ-LOCAL (Gate 0).
Standard rules: Opus lanes, one file-cluster each, `flock /tmp/rs_cam_cargo.lock`
+ `-j 8` + memory gate (`tr -dc 0-9`, fail closed), `pgrep -x`, package-scoped
cargo, narrow commits with the session trailers, planning/*.md left unstaged
for the consolidator, goldens + paired same-session A/B only.

### Cluster 1 — Corpus instruments (STRICT internal order)
Source: `RESEARCH_corpus_instruments.md`. One lane, sequential steps (later
steps depend on earlier nets being in place):
1. `run_diff` fix (~85 LOC, `crates/rs_cam_cli/src/smoke.rs`):
   `kind.starts_with("exceeds")` (covers exceeds_low/high AND drill
   exceeds_elevated/critical), `went_blind` predicate (within→unmodeled_*/missing),
   table-driven loop over all six verdict columns, non-failing numeric-drift
   channel. Must land BEFORE any re-cut (strictly tightens the net).
2. Forced-dive collision sentry: land the lane's control/forced fixture pair
   (retract_z manual −3.0 → 35 collisions vs control 0) as a permanent test so
   the detector's population is asserted, not assumed.
3. AS015 repair (~35 LOC): runner order add → simulate → generate so
   `PhantomPriorStockScan` fills `prior_stocks` for the enabled-but-ungenerated
   op (simulate-before-add keys nothing to its id). Pin a corpus cell size.
4. [needs D2] Baseline re-cut: add resolution column, re-cut AS015 (and any row
   the drift channel now reports), record 211→0 as a documented baseline move.
Note: 211→0 attribution stays UNNAMED (both candidates disconfirmed; optional
15-min settling bisect at `4b105dab^` allowed once the tree is free — nice to
have, not blocking).

### Cluster 2 — Drill TSP (ladder A → B → C1)
Source: `RESEARCH_drill_intent_erasure.md`. One lane for A+B; C1 separately gated.
- A. Sentry (~120 LOC, red on master, no behaviour change): assert emitted fed
  descents == `drill::fed_descents(cycle, bottom_z, retract_z_mm)` against
  MOTION (the stored toolpath), not the cycle description. Covers G73 inversion.
- B. Tag synthesized rapids in `rebuild_group` (~15 LOC, tsp.rs): zero G-code
  change; fixes intent erasure for ALL op families (Pocket loses 66/99 today);
  un-breaks the census/gate-transit half.
- C1. [needs D1, Gate 0 vs G-SAFEZ-LOCAL] per-op `rebuild_clearance_z` through
  TSP (~120–180 LOC, machine-visible). Sentry A flips red→green; A/B the G-code
  and cycle time (expected ≈ 3.4 s vs 21.0 s per hole on shipped defaults).
  NOTE (post-889b1573): G-SAFEZ-LOCAL landed 2026-08-21 and moved resolved
  heights on negative-origin fixtures — `clearance_z` resolves as
  `retract + 10.0` (`compute/config.rs:1386`) and inherited the shift
  (40 → 20 on wanaka). Any absolute clearance_z number measured before
  889b1573 is stale; re-derive against the current tree, or reason
  relative to retract_z.
- Same pass: decide `DressupConfig::retract_strategy` (dead dial — GUI/MCP
  settable, zero consumers): wire it or remove it. Flag to user in the C1
  package; do not silently delete a UI surface.

### Cluster 3 — Viz caches (Weak-pin ×4 + evidence fingerprint)
Source: `RESEARCH_f2_and_aba.md` Topic B. One lane, one file
(`crates/rs_cam_viz/src/state/simulation.rs`), ~+100/−20 LOC per cache, low risk:
- Weak-pin (geom_cache doctrine, `compute/sim_prefix.rs:240-271` generaliser):
  `cached_simulation_triage` (:845), `cached_load_report` (:777 — worst
  consequence, stale chipload/power/deflection verdicts), 
  `cached_chipload_envelopes` (:810), `SpanAggregateCache` (:370 — bare pointer,
  no counter, strictest form).
- THE BIGGER FIX in the same pass: fold `project_evidence()` inputs
  (rapid_collisions, rapid_collision_move_indices, holder counts, resolution_mm)
  into the triage cache key (sibling `issue_cache_key` shows the fingerprint
  pattern) — latent safety-display bug once anyone reads `triage.safety`.
- Doctrine cleanup: `render/upload_cache.rs:28-31` cites the weak cache as
  precedent — fix the comment (and ideally the key) so the weaker doctrine
  stops propagating.

### Cluster 4 — Boundary parity sentry
Source: `RESEARCH_boundary_clear_parity.md`. One lane, test-only (~1 lane-day,
8.2 s runtime), all hunks `#[cfg(test)]`:
- Third arm of the parity family with `boundary = left half, Inside`. Bars:
  non-vacuity preconditions (pre-clear zone > 0, border-clear zone == 0),
  B1 in-scope over-claim ≤ 30% (measured 27.7/27.1), B2 exact
  `assert_eq!(outside-containment cutting endpoints, 138)`, B3 outside-containment
  sim_higher explicitly not gated, plus `#[ignore]`d aspirational B1==0 carrying
  the D4 fix contract. Artifacts/maps preserved in the session scratchpad
  (`…/scratchpad/boundary_lane/`, incl. scratch source for restoration).
- The FIX (un-inset pre-clear + `waterline_cleanup` boundary param) is D4 —
  separate decision package, not this wave.

### Cluster 5 — F2 bars [needs D3]
Source: `RESEARCH_f2_and_aba.md` Topic A. One lane, tests only (~+255/−70 LOC,
zero production files):
1. Print the fixture's Z ladder (the one risk: a short final pass ⇒ C1 is a
   silent tightening ⇒ return to user).
2. C1: convert the three bars (f027:195, :356, f029:215) to ratios vs the LOCAL
   commanded step via `SpanPayload::DepthPass` join (ceiling 1.35, population
   1.1667/5e-4, pass 0 abstains LOUDLY; check `spans_valid` or the bar goes
   vacuous).
3. C3: new sim-free ladder sentry (adaptive3d honours its dpp) under its own
   name — never the F-027/F-031 names.

## Consolidation

After clusters land: consolidate docs into BASELINES.md (ledger, scoreboard),
fold the RESEARCH_* docs' outcomes into DELTA-style closure notes, update
CLAUDE.md only where an agent-facing surface changed (drill cycle numbers if C1
lands; run_diff coverage claim), run `/verify`, then merge/push per user
instruction. Sentry A red-on-master means CI/verify will fail between A and C1
landing — either land A with `#[ignore] + reason` until C1 is approved, or land
A and C1 in the same push window; ASK THE USER which.

## EXECUTION STATUS (2026-08-21, updated live by the orchestrator)

Peer TD3 session landed 8 commits during the wave (889b1573 G-SAFEZ-LOCAL,
d93837eb pin frame, 0748f7ec profile flip, d019a5a4 drill defects + peck
clamp, 5ad592a1 composite supersample ×9 fill cost, b0362626 MCP surface,
838908f8 G-AIRLADDER, 7c125fe9 pin FORCE_NO_ENTRY). My 691304b4 re-blessed
the perf golden after G-SAFEZ-LOCAL (verified: 17 fields, all exact 7 mm/leg).
All five are legitimate baseline-drift sources for Cluster 1 step 4.

- Cluster 1 (corpus): COMPLETE. a8e27d92 (run_diff fix, 12 tests), eaa9e879
  (forced-dive sentry: control 0 vs forced 35 — same 35 as the research via an
  independent construction; asserts corpus reads the detector's own number),
  5ca0c3a0 (AS015 add→simulate→generate + resolution column). AS015 re-cut:
  old row was `exceeds_low` chipload 0.002008 @ no recorded cell; new row
  `exceeds_high` 0.022857 @ 0.5 mm, deterministic across subset/full runs, now
  genuinely through PhantomPriorStockScan. Drift channel: 86 changes / 0
  regressions over 18 rows (old net would have printed ZERO lines); the
  211→0 printed for the first time (AS007 6→0, AS009 1→0, AS010 104→0,
  AS017 100→0), attribution left unnamed. Honest falsifications of the brief:
  889b1573 is STRUCTURALLY unobservable on this corpus (BaselineRow has no
  rapid/runtime/air column); pin-drill fixes unobservable (AS012 vacuous since
  baseline). Effective corpus population is 14 of 18 rows (AS006/012/016/018
  vacuous since baseline), 12 with chipload. OPEN: promote
  `exceeds_low→exceeds_high` (4 rows) to a failing regression? Currently
  reported-not-failed per the within→exceeds contract.
- Cluster 2 (drill): COMPLETE. f29eeb8d (sentry A, verified red first: emitted
  descent 0 was 10.000→2.000 vs schedule 5.000→2.000), 1c5a87b6 (rung B intent
  tagging: 1251 G-code lines byte-identical, 202 Unknown rapid intents
  eliminated, + attributed PR-6b re-pin — that pin hashes Debug incl. `intent`
  despite its geometry-claiming name, split-hash is a backlog row), 18a0a79e
  (C1 `internal_link_ceiling_z`: per hole 70.0→17.0 mm fed / 14.000→3.400 s,
  4.12×, EXACTLY fed_descents; 6 non-drill families md5-identical; hole
  reorder win kept; G73 vs G83 distinct again — they were byte-identical
  before, the cycle dial was inert). Full -p rs_cam_core 3005/0 with C1.
  Research-doc corrections: post-889b1573 the defect cost 70 mm/14 s per hole
  (not 105/21 — safe-Z resolves to 10 not 17); the doc's rebuild-height C1
  shape provably cannot go green (45 mm vs 17 mm). WAVE RULE from a recovered
  incident: never `git commit --amend` in the shared tree (an amend rewrote
  another lane's commit; caught immediately, byte-restored). TWO DOC
  CORRECTIONS from the lane, approved by orchestrator: (i) the doc's C1
  rebuild-height shape cannot go green — a single plane can't express per-peck
  re-entry (`to_z + 0.5`); shipped defaults would still feed 45 mm vs the
  schedule's 17 mm. Implemented instead as a SPLIT CEILING
  (`internal_link_ceiling_z`): rapids below it are cloned verbatim inside
  their cutting segment (a hole = one atomic TSP segment; R-plane + peck
  re-entries survive with intents; hole XY reorder preserved), rapids at/above
  re-synthesize at safe_z. (ii) signature threading via apply_dressups params
  is impossible without touching banned files; value derived inside
  apply_dressups from MoveIntent::Drilling, consolidated with the peer's
  entry-strip scan into one `toolpath_is_drill_cycle`. Fed-move intents
  untouched by construction (peer invariant holds). retract_strategy verdict:
  keep field, don't wire, don't delete; after C1 the only residual job is
  inter-group traverse height (G98/G99) which deserves its own dial; the
  "active dressups" badge overcounts it today (viz change, deferred).
- Cluster 3 (viz caches): COMPLETE. 8a7e71d3 (four Weak-pins + evidence
  fingerprint on the triage key — hashes boundaries, rapid collisions +
  indices, holder collision indices, resolution bits; None-report hashes a
  distinct sentinel so "no report yet"→"report with 0" invalidates) and
  77fd6f72 (upload keys: new `ArcId<T>(Weak<T>)`, both bare-pointer keys had
  the same hole; `advance_source` 0-sentinel replaced; doctrine comment
  retired). -p rs_cam_viz --lib 282/0, clippy clean. The
  evidence-invalidation test is red on the old key by construction.
  NEW LEDGER ITEM from the lane:
  `Arc::make_mut` on the cut trace (session/compute.rs:2865, driven by viz
  feed-modulation take/put-back) mutates IN PLACE when strong count == 1 —
  same allocation, same Weak, same edit_counter, different content. ALL
  identity-keyed caches are content-stale across that path (latent today:
  happens inside one handler with no intervening render). Not fixed; needs
  its own decision.
- Cluster 4 (boundary sentry): DONE, commit 31639742. Reproduced research
  exactly (B1 27.71%/27.07%, B2 exactly 138, non-vacuity green, #[ignore]d
  B1==0 carries the D4 contract). 2.19 s runtime.
- Cluster 5 (F2): C3 DONE, commit 84bbafd3 (+ tests/common/zladder.rs).
  C1 STOP CONDITION FIRED — fixture has a SHORT FINAL PASS (rung 20 of 20 =
  0.0652 mm, landing on the stock_to_leave floor; 19×3.0 before it). Measured,
  not reasoned: the ratio bar would read 8.6994 vs ceiling 1.35 on a shipped
  green (max axial 3.8904 mm, inside the 4.0 mm absolute bar) purely from the
  tiny divisor. Also: doc's "pass 0 abstains" was dead code (pass_index is
  1-based; first rung is pass 1 with 18 764 samples); spans_valid holds.
  C1 conversion REVERTED per contract. OPEN USER DECISION (D3 follow-up):
  (a) ratio bars exempt the final rung, (b) gate on Ladder::is_uniform() and
  keep absolute bars on this fixture, or (c) change fixture depth to divide
  evenly. Facts recorded permanently in tests/common/zladder.rs + C3 docs.
- Process: the prescribed memory gate (`free -g`+`tr -dc 0-9`) INVERTS below
  10 GiB (decimals: "7.6Gi"→"76"→passes). Replaced wave-wide with
  `awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo`. Root cause of
  a ~4 h three-lane stall: 2 rust-analyzer instances pinned 26.8 GB with swap
  exhausted; user approved killing them (freed to 38.4 GiB).
