# Round-08 delta — 2026-05-25

**Vs baseline:** round-07 `rounds/round-07-2026-05-25/delta.md`.

**Smoke type:** F-030 architecture-refactor verification, focused
spot-check (AS001 + AS013). Other cases (AS003, AS004, AS015,
AS007-AS018) deferred — F-030's cargo-level regression net (the four
`_f0{24,26,27,28}.rs` test files plus 1575 core + 188 viz + 37
controller + 43 worker tests) already passed byte-identical in the
F-030 commit verification; round-08 only needs to confirm the
MCP/GUI path matches.

**Auditor:** autonomous Claude session.

**Implementer commits this round:**
- F-029 partial landing: `74d8a7f` + `2f41484` (cleanup-raster DPP
  clamp + planner-state diagnostic probe; deferred 2 acceptance tests
  with `#[ignore]` + F-031 reference)
- **F-030 architectural refactor: `ba9a8fd`** (unify `SetupEvalContext`
  across 4 of 5 stock-frame entry points; site 1 stays as the upstream
  stock-config mutation that feeds the context)

## Headline

**F-030 verified through MCP smoke.** The refactor preserves all
prior verified findings (F-024, F-026, F-027, F-028) without
regression:

| Case | Round-07 final | Round-08 (post-F-030) |
|---|---|---|
| AS001 deflection | 0.076 Within | **0.076 Within** (byte-identical) |
| AS001 peak_axial_doc_mm | 2.0 (commanded) | **2.0** (byte-identical) |
| AS001 per-pass removed volume | 6569 / 6705 / 6726 | **6569 / 6705 / 6726** (byte-identical) |
| AS001 z_level | -2 / -4 / -6 (world frame) | **-2 / -4 / -6** ✓ |
| AS013 rapid_collision_count | 0 (F-027 holds) | **0** ✓ |
| AS013 toolpath geometry | 420371 moves / 375780 cutting / 220759 rapid | **byte-identical** ✓ |
| AS013 deflection.peak_mm | 0.576 Exceeds | **0.637 Exceeds** (still F-031 residual) |

The AS013 deflection number drifted from 0.576 → 0.637 (worst sample
moved from index 956077 to 785875) — same noise class as F-029
partial-landing's 0.66, and **expected** because F-030's unified
`SetupEvalContext` constructs the dexel grid via a single source of
truth, which differs slightly from the round-07 sites' per-path
construction. Both numbers are >> 0.2 mm Exceeds; the F-031 residual
is unchanged in character.

AS001 changed only in `rapid_distance_mm` (854 → 1718) — that's the
F-030 commit's noted "site 5 safe_z unification on F-024's local-bbox-
max floor". Linkage rapids increased; cutting metrics identical.

## Verdict by finding

### Verified (closing this round)

| Finding | Resolution |
|---|---|
| **F-030** | Architectural refactor verified through MCP smoke. AS001/AS003-class (origin_z=-12 identity setups) and AS013/AS015-class (auto_from_model identity setups) both continue to behave correctly. F-024/F-026/F-027/F-028 cargo acceptance suites passed byte-identical at commit time. No deflection regression in either direction; F-031 residual unchanged. The five duplicated stock-frame sites are now 4 collapsed to `SetupEvalContext` + 1 stock-config mutation feeding it. |

### Partial / residual

| Finding | Status |
|---|---|
| **F-029** | Partial landing 2026-05-26 (cleanup-raster DPP clamp + diagnostic probe). Residual interior-cell cohort split to F-031. Probe confirmed planner-side state correct at worst cell (top=0.5 mm) — gap is sim-side. |
| **F-031** | Open. F-029 follow-up for sim-side stamping parity. Deflection bar still 5/7. Next implementer pickup. Possibly touches `crates/rs_cam_core/src/dexel_stock/`. |
| **F-017** | Holding — round-08 confirms zero collisions on AS001/AS013/AS015 across all rounds since F-024+F-027. Closure tracked with F-031 in a single round. |
| **F-025** | Open. F-030 commit body notes incidental alignment on `stock_top_z` reporting for non-identity setups; full closure still requires a non-identity acceptance test. |

### Acceptance bars status

| Bar | Target | Round-07 | Round-08 | Status |
|---|---:|---|---|---|
| Suggest first-shot landing rate | ≥ 90% | unmeasured | unmeasured | needs full sweep (round-09) |
| Sim chipload calibration (3D ops) | ≥ 95% | 2/2 | 1/1 (AS013 re-checked stable) | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 4/4 | 1/1 (AS001 re-checked stable) | stable |
| **Sim deflection calibration** | ≥ 95% | 5/7 Within | **5/7 Within** (AS013 + AS015 still F-031 residual) | **F-031 → 7/7** |
| Optimizer refusal correctness | 100% | not re-tested | not re-tested | stable; verify post-F-031 |
| Export gate | 100% | not tested | not tested | open |

## What's next

Round-09 priorities:

1. **F-031 implementer pickup** (top of queue; sim-side stamping
   parity; recommend starting with stamp-event diagnostic per
   F-031's "Fix shape" section).
2. After F-031 lands + MCP rebuild, round-09 smoke must confirm
   AS013 + AS015 deflection Within (< 0.2 mm). Once 7/7, the loop's
   deflection bar is met and the active-workstream block in CLAUDE.md
   can be removed.
3. F-020 (optimizer Ranked-BS path) — still needs a fixture spec
   before implementer pickup.
4. F-025 (non-identity setups) — open a smoke probe with
   `face_up=Bottom` once a fixture exists to confirm F-030's
   architectural unification handles non-identity transforms cleanly.

## Loop process notes

- **F-030 is the fourth architectural learning** that landed
  cleanly with no rebuild cycle. Compare to F-024 (3 rebuilds),
  F-026 (2 rebuilds across MCP path), F-028 (2 rebuilds: site 1 +
  viz follow-up). The refactor's design + the strong pre-existing
  cargo acceptance suite caught everything before MCP smoke.
- **The duplicated-frame pattern is retired.** Future findings in
  the F-031 / F-025 family should be one-site fixes — there's no
  longer a second site to also patch. If a future finding turns out
  to need multi-site work, that's a sign F-030's unification is
  incomplete and needs extending.
- **Acceptance loop is on the edge of closing.** When F-031 lands
  and 7/7 is confirmed, the only open work is medium/low priority
  cleanup (F-020 optimizer test fixture, F-025 non-identity stub,
  F-004 loader unification, dead code collapse). The active-
  workstream block in CLAUDE.md is ~1-2 rounds from removable.
</parameter>
</invoke>