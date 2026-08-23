# Phase 2 run log — wanaka200 (started 2026-08-23)

Plan: `EFFICIENCY_PHASE2_2026-08-23.md`. Baseline: `wanaka200_fast2.toml`
(16,248 s, gates 8/8, 0/0). Working candidate: `wanaka200_p2.toml`.

## Session 1 — 2026-08-23

**P2-0 dispatched** — three Opus implementation agents (no-cargo editors;
this session holds the single verify lane):

1. generated-empty refusal + G-STICKYEMPTY (core generation pipeline)
2. G-HEIGHTSTAB heights-tab pure-read fix (viz properties panel)
3. G-UNIFIEDCRASH scallop 0.03 + z_step 0.6 index-OOB panic (unified_finish)

Plus a read-only Opus feasibility agent pricing **A2 project_curve curve
chaining** (easy-win vs planner-rework verdict, per plan gate).

**A1 applied (file edit, pre-generation)** — `wanaka200_p2.toml` branched
from fast2 with Lakes moved to the 20° V-bit:

- `5 Lakes (back)` → `5 Lakes (back, V-bit)`: tool_id 2 (R1.0) → 5 (V-bit)
- Feeds adopted from the proven Rivers V-bit recipe: feed 1062 → 1200,
  plunge 180 → 496, RPM 19000 → 24000. depth 0.3 / side inside /
  point_spacing 0.5 unchanged (geometry untouched).
- feeds_provenance vendor row (the R1.0's amana-tapered row) → formula,
  matching Rivers.
- Rivers (idx 3) and Lakes (idx 4) already adjacent in Setup 1 → with the
  same tool the tool change between them disappears with no reorder.
- **Operator eyeball pending**: V-bit groove at 0.3 mm depth is ~0.11 mm
  wide vs the R1.0's ~0.5 mm — render queued for approval; A1 reverts if
  the look fails.

Full ladder (`generate_all` fixpoint @0.15) launched on `wanaka200_p2.toml`;
memory watcher armed (GUI-death + <12G warn / <6G critical).

**A1 measured — ladder + sim @0.15 complete: 16,200 s (4.50 h), 0/0
collisions, verdict OK, GUI load 8/8 within.** Rivers+Lakes combined
794 s (was 833) — Lakes total 87.7 s on the V-bit (was ~126 on the R1.0)
— plus one physical tool change removed (not priced by the sim). Lakes
crosses-standing caution now reads peak 4.92 mm (10% of samples > 2.70)
— same family as Rivers' pre-existing 3.56 peak: from_below projection
through roughed back stock; not new in kind. Renders for the operator
eyeball: `p2_a1_lakes_vbit_chk4.png` (composite; groove sub-pixel at
0.53 mm/px) and `p2_a1_lakes_vbit_chk4.html` (interactive — zoom the
lake outlines; V groove ~0.11 mm wide vs R1.0's ~0.5 mm).

**A2 feasibility verdict: EASY WIN** (read-only agent report, session 1).
`surface_link::relink_fragments` (scallop's intra-pass relinker) consumes
exactly the `rapid → plunge → cut → retract` units project_curve emits;
the multi-curve loop lives in `execute.rs::generate_project_curve`, which
has mesh + spatial index + boundary regions + machine kinematics + prior
stock all in scope. ~140 production lines + ~280-line sentry. MUST ship
with the stock-aware link ceiling (`max_conservative_top_z_in_disc`) —
mesh-only links on FromRemainingStock reproduced a 151-rapid-collision
regression class in the 2026-07 finishing review. `chain_distance_mm`
defaults 0.0 = byte-identical existing projects. Also learned:
`retract_strategy` is a dead dial for EVERY op (no consumer in core).

**P2-0.1/0.2 landed in working tree (agent report, unverified until the
gate run).** Generated-empty: new single-owner classifier
`compute/generated_empty.rs` — empty = zero non-Rapid moves; typed
`SessionError::GeneratedEmpty` applied at the two persist sites only
(session generate + GUI worker), terminal so the fixpoint loop doesn't
retry it; exemptions for rest ops/drills/no-region/feature-selective
finishes (Pencil, HorizontalFinish). G-STICKYEMPTY root cause PROVEN
static: `PhantomPriorStockScan` counted an empty op as generated
(`result.is_some()`) while `run_simulation` dropped it from carving
(<2 moves) so it never got a prior-stock snapshot — downstream rest
precondition then failed forever until reload; fixed with a shared
`contributes_simulated_motion` predicate + result-cache drop on every
failure exit + viz `remove_result` on failed submits. Helix-empty
NEGATIVE: emission cannot zero a toolpath (every entry arm emits ≥1
move); live zero came from upstream planning or a raced job — now
observable via the refusal either way. 6 new sentries + 9 unit tests.
Watch items for verification: (a) a standalone Waterline/Scallop op that
legitimately finds nothing now errors — acceptable per design, but check
no existing test fixture trips it; (b) result-cache now cleared on ANY
failure (behaviour change).

**G-SIMDUMP filed + fixed (orchestrator, small).** Root cause of today's
disk-full: EVERY simulation unconditionally writes the full cut-trace
artifact JSON to `target/simulation_metrics` (multi-GB per dump at 0.15
on this board; 96 GB / 81 dumps found by the agent — the fixpoint ladder
runs several sims per call). Fix: `prune_simulation_cut_artifacts(dir,
keep)` in core `simulation_cut.rs`, called after each successful write
from the viz worker, retain 5 (`SIM_CUT_ARTIFACT_RETAIN`).

**B1 evidence**: crosses-standing on the pencil unchanged — 15.9% of
cutting samples > 0.61 mm (3× median 0.20), peak 3.72 mm at
(44.5, 92.1); worst-site scrub screenshot `p2_b1_pencil_worst_scrub.png`
(wide shot; the ~0.7 mm gouge is below render resolution — the numeric
caution IS the evidence, B2's sentry gates on it). Pencil entry_s still
2,412 of 3,332 total.

**C1 diagnosis — the hills roughness is the mid-steep scallop band.**
narrate(finish): MidSteep scallop band = 59,994 of 113,260 moves (53%),
still at scallop_height 0.1 (100 µm cusps, the G-UNIFIEDCRASH
workaround). Shallow raster band at stepover 0.6 on R1.5: cusp ≈
0.6²/(8·1.5) ≈ 30 µm. The hills (mid-steep slopes) are ~3× rougher than
the flats BY PARAMETER — matches the operator's eye. Note: 0.15 mm sim
cells cannot resolve 100 µm cusps, so no render can show this; C4's A/B
accepting gate remains the operator's eye on the physical cut / C2 vs C3
timing. Suspect #2 (finish crosses-standing, peak 3.25 mm at
(32.8, 83.8)) remains real but secondary.
