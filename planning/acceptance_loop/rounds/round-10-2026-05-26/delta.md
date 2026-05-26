# Round-10 delta — 2026-05-26

**Vs baseline:** round-09 `rounds/round-09-2026-05-26/delta.md`.

**Smoke type:** F-032 reframe verification (AS015 with prior roughing).

**Auditor:** autonomous Claude session.

**Implementer commits this round:**
- `abe9b98` — F-032 reframe (docs only; round-10 implementer probe refuted the original hypothesis and reverted all speculative code changes; finding updated with real triggering-sample evidence and four candidate fix shapes).

## Headline

**🎯 Deflection bar 7/7 Within. Acceptance loop's primary acceptance criterion MET.**

Round-10 closed the bar via methodology fix on AS015 (option A from F-032's reframe). The 0.434 mm Exceeds reading was **real deflection**, not a measurement bug — AS015's smoke setup was running scallop on unroughed stock, which is malpractice. With a prior AS013-style roughing pass on the same project, scallop runs on cleared stock and deflection drops to **0.197 mm Within** (just under the 0.2 threshold, "surface finish degradation expected" note).

**System was correct. Smoke methodology was wrong.**

| Case | Round-09 | Round-10 | Resolution |
|---|---|---|---|
| AS015 (solo) | 0.434 Exceeds | (not re-run solo) | confirmed real deflection from cutting through bulk stock |
| AS015 (with prior AS013 rough) | n/a | **0.197 Within** ✓ | smoke methodology fix |

## Final deflection bar — 7/7 Within

| Case | Op | Deflection | Resolving finding |
|---|---|---|---|
| AS001 | pocket | 0.076 | F-024 + F-028 |
| AS002 | adaptive | 0.053 | F-024 + F-028 |
| AS003 | profile | 0.076 | F-024 + F-028 |
| AS004 | face | 0.005 | F-028 |
| AS005 | zigzag | 0.051 | F-024 + F-028 |
| AS013 | adaptive3d | 0.105 | F-031 |
| AS015 | scallop (with prior rough) | 0.197 | F-031 + methodology |

## F-032 reframe — what the implementer's probe found

The round-10 F-032 implementer pickup probed AS015 through `ProjectSession::run_simulation` and refuted the original "transit-sample contamination" hypothesis. The triggering sample at `move_idx=2016, sample_idx=5052` was:

- `position = (97.37, 67.61, 14.59)` — cutter at terrain height, NOT a plunge
- `kinematics = Helix` — XY+Z lateral move
- `move_type = Linear { feed_rate: 800 }` — finishing-cut feed
- `intent = FinishingCut` — NOT EntryPlunge/Helix/Ramp
- `radial_woc_fraction = 0.750` — 75% engagement, real cut
- `arc = 2.094 rad (~120°)` — real lateral sweep
- `in_transit_span = false` — F-031's predicate correctly NOT firing

So the deflection model was reading a steady-state finishing cut, not a transit artifact. The reported `axial_engagement_mm = 17.77 mm` on a 15 mm-flute 3 mm ball nose is **above the flute** — the cutter's shank is passing through ~38 mm of solid uncut stock above each cut. The deflection model correctly says this would deflect a tool 434 µm. **The system was right.**

The implementer correctly refused to ship a band-aid and reverted all speculative changes. Their reframe document on the finding (`F-032's "Round-10 implementer reframe"` section) details the real root causes and four candidate fix shapes.

## Auditor decision: option A (methodology), F-033 for option D

Option A (add prior roughing pass) was selected because:

1. AS015's smoke goal (`finish-quality-scallop-good_light_sanding`) presumes a roughed workpiece — the goal name explicitly says "finishing."
2. The 0.434 reading was real, so options B/C (generator hardening / dexel clamp) would either not move the bar (C, tried — moved to 0.424 trivially) or fix a tangential generator bug without addressing the bar (B).
3. Option D (workflow advisory) is a real UX win but doesn't move the smoke bar.

**F-033 opened** as a stub for the workflow-advisory feature (option D): a pre-sim hint that fires when the user sets up a 3D finishing op without a prior roughing toolpath in the same setup. Low severity, not loop-blocking; schedule when UX bandwidth permits.

## Verdict by finding

### Closed this round

| Finding | Resolution |
|---|---|
| **F-032** | Smoke methodology fix, not a system bug. AS015 with prior roughing reads 0.197 Within. System correctly flagged dangerous engagement on unroughed stock. F-033 opened for UX follow-up. |
| **F-017** | Rapid collisions explicitly closed. Every smoke case AS001-AS015 reports 0 rapid collisions in round-09 and round-10. F-024 + F-026 + F-027 + F-031 between them have eliminated the entire 3D-op collision surface. |

### Opened this round

| Finding | Trigger |
|---|---|
| **F-033** | UX follow-up to F-032's reframe. Pre-sim advisory when 3D finishing op has no upstream roughing toolpath in the same setup. M effort, low severity, not loop-blocking. |

### Acceptance bars status — final

| Bar | Target | Round-10 result |
|---|---:|---|
| **Sim deflection calibration** | ≥ 95% | **7/7 Within ✓ CLOSED** |
| Sim chipload calibration (3D ops) | ≥ 95% | 2/2 stable ✓ |
| Sim chipload calibration (2D ops) | ≥ 95% | 4/4 stable ✓ |
| Suggest first-shot landing rate | ≥ 90% | unmeasured (out of loop scope) |
| Optimizer honest-improvement | ≥ 95% | unmeasured (out of loop scope) |
| Optimizer refusal correctness | 100% | unmeasured (out of loop scope) |
| Export gate | 100% | unmeasured (out of loop scope) |

The deflection bar — the loop's primary acceptance criterion across rounds 04-10 — is **met**. Remaining unmeasured bars (suggest, optimizer, export) are stretch goals for follow-up workstreams; they were never the focus of this loop.

## Acceptance loop closure

**The active-workstream block in `CLAUDE.md` can be removed.** All loop-critical findings are closed or verified:

| Finding | Status | Final outcome |
|---|---|---|
| F-001 | closed round-02 | chipload 2D feedopt probe verified |
| F-002 | closed round-02 | axial_doc split into engagement + plunge |
| F-003 | closed round-02 | VENDOR_LUT collapsed |
| F-007 | closed round-02 | drill plunge_rate honored |
| F-008 | closed round-02 | compute_stale_set unified |
| F-013 | closed round-02 | feeds-result invariants |
| F-014 | closed round-02 | docs stale-marker |
| F-015 | closed round-02 | op-precondition validator |
| F-016 | closed round-02 | drill chip_welding material-aware |
| F-017 | **closed round-10** | rapid collisions retired across smoke |
| F-018 | closed round-05 | test_data regen |
| F-023 | closed round-03 | model-ref diagnostic surface |
| F-024 | closed round-04 | Z-frame three-site fix |
| F-026 | closed round-06 | auto_from_model bbox re-derivation |
| F-027 | closed round-07 | adaptive3d planner stock-XY widen |
| F-028 | closed round-07 | face op heights frame |
| F-029 | partial-landing round-08 | adaptive3d cleanup-raster DPP clamp |
| F-030 | closed round-08 | SetupEvalContext architectural refactor |
| F-031 | closed round-09 | DressupConfig::for_op(Adaptive3d) entry_style |
| F-032 | **closed round-10** | smoke methodology, not system bug |

**Stretch goals (no longer loop-blocking)**:
- F-020 — optimizer Ranked-BS path untested (needs test fixture)
- F-025 — non-identity setup Z-frame stub (F-030 likely subsumes; needs face_up=Bottom fixture)
- F-033 — workflow advisory (UX feature)
- Cleanup queue: F-004, F-005, F-006, F-009, F-010, F-011, F-019, F-021, F-022

## Loop process retrospective

Across rounds 04-10, the loop landed **20 findings** (10 round-02 originals + 10 new) and burned **2 architectural refactor cycles** (F-024's three-site fix as a "first taste of the pattern" and F-030 as the consolidation). Key learnings encoded into `implementer_contract.md` and `audit_runbook.md`:

1. **Test through the production entry point.** Cargo tests against `session.run_simulation()` aren't enough — MCP/GUI take a different code path through the viz controller. F-024's three-rebuild saga taught this; F-028's viz-path regression confirmed it.
2. **Tighten test assertions to tight bands, not upper bounds.** The F-028 regression slipped past because `peak_axial <= 0.6` is trivially satisfied by `peak_axial = 0` (cutter in the air, not cutting). F-031's `peak_axial in [1.5, 3.0]` band catches both inflation AND extinction.
3. **Refute hypotheses with probes before fixing.** F-029's interior-cell parity, F-031's transit-sample hypothesis, and F-032's transit-sample hypothesis were all wrong on first framing; each was refuted by a diagnostic probe before the actual root cause surfaced. Saved likely 2-3 rebuild cycles total.
4. **Architectural refactors retire bug classes.** F-030's `SetupEvalContext` unification eliminated the duplicated-frame-handling pattern that caused F-024, F-026, and F-028 to each need multi-site fixes. F-031 + F-032 (post-F-030) landed cleanly at single sites or as methodology corrections.
5. **Surprising root causes are common.** F-031's actual cause (`DressupConfig::prefer_helix` rewriting plunges into helixes that desynced planner stamping) was none of the four hypotheses in the F-031 finding. F-032's actual cause (smoke methodology, not system) was none of the four reframed candidates. Investigation discipline pays.

The smoke acceptance suite (`cases_agent_smoke.csv` + `_f0{24,26,27,28,31}.rs` cargo tests) is now a strong regression net for future work. Anything in the F-031/F-025/F-033 family that re-surfaces should fail one of these tests first, not pass smoke before being noticed.
</parameter>
</invoke>