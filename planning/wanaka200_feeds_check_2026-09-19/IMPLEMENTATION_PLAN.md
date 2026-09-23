# Implementation plan — ProjectCurve plunge escape + modulation coverage

Opened 2026-09-19. Fixes the two defects measured in FINDINGS.md and
re-verified by simulation this session. Scoped deliberately: the H1–H7
feeds-gap investigation in `INVESTIGATION_PLAN.md` stays separate and is
NOT blocked by this work.

## Measured evidence (0.2 mm sim, verified)

- **Toolpath 19 (V-bit ProjectCurve), `project.plunge_class_load`
  CRITICAL:** move 3700 descends at 1165 mm/min against the op's own
  400 mm/min plunge rate (2.9x), onto a 20° V point. 1 of 250
  vertical-dominant moves.
- **Toolpath 17 (Scallop Finish), `project.crosses_standing_material`
  CAUTION:** 27.9% of samples remove > 1.39 mm vs a 0.46 mm median
  bite, peaking at 7.44 mm — upstream `Back Rough` is disabled. Project
  sequencing, not code; no fix here beyond the diagnostic being honest.
- TP19 carries no `modulation_summary`; TP17 does. Confirms FINDINGS
  defect #1 at the current HEAD.

## Root cause (pinned in code)

`session/compute/simulation.rs::apply_adaptive_feed_modulation` walks
enabled toolpaths and **skips any without a chipload band** (the
`envelopes.get(&toolpath_id) else { continue }` arm). The V-bit
ProjectCurve has no vendor rows (investigation H7) → no band → the
modulator never runs → the **geometric plunge guard** inside
`adaptive_feed_modulate` (`dressup/feed_modulation.rs`, Phase 3, sentry
`plunge_guard_p3`) is unreachable for bandless ops. The guard is
correct; it is only reachable through a band.

## Non-negotiables

- **No new dressup stage.** `dressup/CLAUDE.md`: three passes live
  outside `DRESSUP_PIPELINE` ("add no fourth"). The fix extends the
  existing `adaptive_feed_modulate` entry in `session/compute.rs` /
  its call site in `simulation.rs`. No new pipeline stage, no parallel
  one-off pass over moves elsewhere.
- **One construction site for plunge classification.** The guard keeps
  reading `kinematic_utilization::classify_move`. No second classifier.
- **The modulator skips a plunge by geometry, not by intent tag**
  (dressup invariant) — unchanged.
- **Bandless ≠ silent.** A bandless toolpath that had moves capped must
  stamp an honest summary so the UI/diagnostics can see coverage. Do
  not invent a chipload band for a V-bit (that is H7's ruling to make).
- Lint gate: zero warnings, clippy `-D warnings`, `// SAFETY:` on any
  production `allow`.

## Work item A — bandless modulation path (fixes the CRITICAL)

`adaptive_feed_modulate` currently requires a `ChiploadBand` in
`ModulationContext`. Change the band to `Option<ChiploadBand>`:

- `Some(band)` — behaviour byte-identical to today (every existing
  sentry must pass unchanged).
- `None` (bandless) — the strategy applies ONLY:
  1. the machine cutting-feed ceiling (`MachineMaxFeed` binding),
  2. the geometric plunge guard (`PlungeRate` binding).
  No chipload targeting, no band-mid heuristics, no chip-thinning
  path. `should_skip_modulation` / engagement skips unchanged.

Call site: in `apply_adaptive_feed_modulation`, replace the
`envelopes.get(...) else { continue }` bail with a bandless call —
build the context with `plunge_rate_mm_min` from
`operation.plunge_rate()` (already the source the shared
`modulate_annotated_against_trace` uses) and `band: None`, then run
the same `modulate_annotated_against_trace` flow so summary stamping,
re-timing, and provenance refresh are the SAME code path the banded
case uses. The bandless arm must also run for ProjectCurve ops with a
banded tool — it is not V-bit-specific.

`modulate_annotated_against_trace` (`session/compute.rs`) takes the
band through to the context; widen it the same way (`Option`), one
place, shared by the strategy advisor and the sim pass so the band
the advisor modulates against and the band the sim pass applies stay
one code path (existing F-036b/F-039 comment already promises this).

## Work item B — no change needed to reach ProjectCurve?

Verify, don't assume: the candidate loop in
`apply_adaptive_feed_modulation` iterates ALL enabled toolpaths with
results — FINDINGS defect #1 ("modulation does not run on a
project_curve operation") may be fully explained by the band bail for
TP19 (V-bit, no rows) — TP id 6 ("Lakes", tapered-ball ProjectCurve)
should reach the banded arm ALREADY if its envelope resolves. Check
with `get_suggest_rationale` / envelopes for that op; if it modulates,
defect #1 is closed by A alone and B is a measurement note, not code.

## Work item C — acceptance (measurement, not code)

1. Folder sentries: `plunge_guard_p3`, `dressup_span_invariants`,
   `constrained_max_modulation_f039`, `lead_in_out_feed_rates_f040`.
2. New sentry (alongside `plunge_guard_p3`): a BANDLESS toolpath with
   a steep descent emits every vertical-dominant move at or below its
   op plunge rate; a banded path is byte-identical (regression guard
   on the `Some(band)` arm).
3. `cargo test -p rs_cam_core --lib feeds:: dressup::` + clippy
   `-p rs_cam_core --all-targets -- -D warnings`.
4. Re-sim TP17+TP19 at 0.2 mm (same cell size as the measured
   evidence): `project.plunge_class_load` must be GONE on TP19;
   TP19 must now carry a modulation summary naming the bandless
   strategy; TP17's summary must be unchanged.
5. 0.1 mm re-sim only if the operator rules it (the triage flagged
   `cell_too_coarse_for_tip_contact` on TP17, blind fraction 0.162).

## Order

A → B (verification note) → C. Everything runs through
`scripts/cargo_lane.sh`; one cargo job at a time.
