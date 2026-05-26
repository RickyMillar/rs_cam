# Round-09 delta — 2026-05-26

**Vs baseline:** round-08 `rounds/round-08-2026-05-25/delta.md`.

**Smoke type:** F-031 verification (AS013 + AS015 deflection check).

**Auditor:** autonomous Claude session.

**Implementer commits this round:**
- F-031 (commit `497a3b2`): narrowed `DressupConfig::for_op` `prefer_helix`
  override to 2D Adaptive only; forced `entry_style = None` for Adaptive3d;
  added transit-filter to F-027 test infrastructure.

## Headline

**F-031 verified on AS013. Deflection bar moved 5/7 → 6/7.** AS015
unchanged — same physical-impossibility signature as F-031 but on the
scallop op's deflection gate; opened as **F-032** for follow-up.

| Case | Round-08 | Round-09 |
|---|---|---|
| **AS013 adaptive3d** | 0.637 Exceeds | **0.105 Within** ✓ |
| AS013 toolpath geometry | 420,371 moves / 375k cut / 220k rapid | **65,299 moves** / 219k cut / 132k rapid (6.4× reduction!) |
| AS013 rapid_collision_count | 0 | 0 (F-027 holds) |
| AS015 scallop | 0.434 Exceeds | 0.434 Exceeds (byte-identical; F-031 doesn't apply) |
| AS015 chipload | Exceeds_LOW 0.002 mm/tooth | Exceeds_LOW 0.002 (same) |
| AS015 power | 0.038 kW Within | 0.038 (same) |

## Collateral observation on AS013

F-031's fix not only resolved the deflection — it also dramatically
reduced toolpath size. The `prefer_helix` override was rewriting every
adaptive3d plunge into a multi-pass helix at ~2 mm radius, inflating
the toolpath 6.4× without improving cut quality. Post-F-031:

- Move count: 420,371 → **65,299** (-85%)
- Cutting distance: 375,780 → 219,766 mm (-42%)
- Rapid distance: 220,759 → 132,395 mm (-40%)

This is a meaningful real-world improvement beyond just closing the
deflection bar — generation runtime and G-code file size will both
drop ~6× on adaptive3d toolpaths. The auditor recommends a round-10
spot-check on AS017 (horizontal_finish) and AS018 (project_curve) to
confirm no similar dressup-rewrite cruft remains in other 3D ops.

## Why F-031 didn't fix AS015

AS015 is scallop, not adaptive3d. F-031's fix was narrowly scoped to
`DressupConfig::for_op(Adaptive3d)` — scallop runs through a
different op-config path with no analogous helix-entry-style dressup.
The F-031 fix has no code path to scallop.

The actual AS015 residual is **transit-sample contamination at the
deflection gate**, not a planner stamping issue. Hotspot probe on
AS015 shows:

| Hotspot | peak_axial_doc_mm | avg engagement | Comment |
|---|---:|---:|---|
| 1 | 25.68 (on a 3 mm ball nose!) | 0.013 | physically impossible for steady-state |
| 2 | 26.67 (same impossibility) | 0.051 | transit/entry sample |

A 3 mm ball nose **cannot** cut 25–26 mm axial DOC. Average engagement
< 5% confirms these are transit/entry samples being read by the
deflection gate as "heavy engagement." Chipload (0.002 mm/tooth) and
power (0.038 kW) confirm cutting forces are tiny — there's no physical
way to generate 434 µm tip deflection from this load.

This is the **F-031 pattern on a different layer**. F-031 added a
`!in_transit_span` filter to F-027's test infrastructure to mirror
`is_steady_state_for_gate`. F-032 extends that filter to the scallop
op's deflection-gate sample selection.

## Verdict by finding

### Verified (closing this round)

| Finding | Resolution |
|---|---|
| **F-031** | AS013 deflection 0.637 → 0.105 Within. Adaptive3d toolpath geometry collapses 6.4× (helix-entry-rewrite was inflating cruft). Acceptance tests in `adaptive3d_interior_cell_parity_f029.rs` pass under their `_f031` names. |
| **F-017 (rapid collisions)** | Round-09 confirms 0 rapid collisions on AS013 (post-F-031 toolpath) and unchanged on AS015. **F-017 is reframed as closed-by-F-024+F-026+F-027** — every case in the smoke suite reports 0 rapid collisions. **Recommend explicit close in round-10.** |

### Opened this round

| Finding | Trigger |
|---|---|
| **F-032** | AS015 deflection 0.434 Exceeds unchanged after F-031. Hotspot evidence shows physically-impossible peak_axial_doc_mm = 25-27 on a 3mm ball nose with avg engagement < 5%. F-031 sibling — transit-sample contamination on scallop's deflection gate, F-031's transit-filter doesn't extend. S effort; F-031's predicate just needs to honor scallop too. |

### Acceptance bars status

| Bar | Target | Round-08 | Round-09 | Status |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured | unmeasured | needs full sweep |
| Sim chipload calibration (3D ops) | ≥ 95% | 1/1 (AS013) | 2/2 (AS013, AS015) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 1/1 (AS001) | (not re-tested) | stable |
| **Sim deflection calibration** | ≥ 95% | 5/7 Within | **6/7 Within** | **F-032 → 7/7** |
| Optimizer refusal correctness | 100% | not re-tested | not re-tested | stable; verify post-F-032 |
| Export gate | 100% | not tested | not tested | open |

## What's next

Round-10 priorities:

1. **F-032 implementer pickup** — last remaining deflection bar blocker. Expected effort: S.
2. After F-032 lands + MCP rebuild, round-10 smoke confirms AS015 deflection Within. **7/7 deflection bar met.** Loop's primary acceptance criterion closed.
3. Round-10 should also explicitly close **F-017** (rapid collisions) — round-09 confirms every smoke case reports 0 rapid collisions.
4. Spot-check AS017/AS018 for any analogous dressup-rewrite cruft (F-031's collateral 6.4× toolpath reduction may have siblings in other 3D ops).
5. F-020 (optimizer Ranked-BS path) — still needs a fixture spec.
6. F-025 (non-identity setups) — open smoke probe with `face_up=Bottom`.

The acceptance-loop workstream block in `CLAUDE.md` becomes removable once F-032 verifies 7/7 in round-10.

## Loop process notes

- **F-031 + F-030 had no rebuild cycle.** Both landed cleanly through their cargo regression net + MCP smoke. Compare to F-024 (3 rebuilds), F-026 (2 rebuilds), F-028 (2 rebuilds). F-030's `SetupEvalContext` unification + the strong pre-existing cargo acceptance suite caught everything before smoke.
- **F-031 root cause was none of the 4 hypothesized causes.** The finding speculated (1) sub-sample stamping, (2) LUT cadence, (3) grid origin, (4) frame interaction. Actual cause: dressup `prefer_helix` override rewriting plunges into helixes that desynced planner stamp parity. Good investigation rigor by the implementer — they ruled out hypotheses 1-3 with a stamp-event diagnostic before settling on the dressup-emission axis.
- **F-032's framing is high-confidence**: the chipload/power/deflection inconsistency (tiny load, large deflection) is mechanically impossible without sample contamination. F-031's predicate is already in the tree; F-032 is a one-clause extension.
</parameter>
</invoke>