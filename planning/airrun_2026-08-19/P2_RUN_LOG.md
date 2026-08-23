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

## Session 1, part 2 — code landed + live validation (new binary 13:28)

All P2-0 + A2 + B2/B3 committed and pushed (ba7c6188 heights, dc2988ec
unified crash, 1015881e empty/sticky/simdump, 3ef08d32 chaining,
4346cb7b pencil ramp + entry gate). Gates: fmt/clippy/core/viz/cli/mcp
ALL GREEN. Two gate iterations worth recording: (1) the Waterline op
joined the feature-selective empty exemption (a no-steep-walls model is
a legitimate empty — in-tree fixture proved it); (2) `project.entry_load`
was scoped to REST-DRIVEN toolpaths via
`SimulationTriage::build_with_rest_context` after it fired on the perf
golden's fresh-stock drop-cutter (a fresh-stock entry plunge is planned
motion; the golden stayed untouched at 3 actions, benches/hot_paths.rs
kept compiling); plus a 10-min prune grace so concurrent sim-artifact
writers can't delete each other's fresh dumps.

**Live repro validation on the rebuilt binary (wanaka200_p2 loaded):**
- G-HEIGHTSTAB: heights tab opened on healthy Back Rough (7,672 moves)
  → still Done, not stale, move count intact. FIXED.
- G-ENTRYEMPTY/G-STICKYEMPTY: helix entry now GENERATES (7,659 moves,
  66,189 mm cutting — the campaign's 0-move result was poisoned state,
  per the agent's emission analysis) → plunge back → 7,672 exactly, no
  reload. FIXED. Side finding: helix cutting distance is 3,247 mm
  SHORTER than plunge — the blocked C5 entry-style A/B is now runnable.

`chain_distance_mm = 15.0` dialed into Rivers + Lakes; full ladder
(fixpoint @0.15) launched for the A3 + pencil-ramp measurement.

## Session 1, part 3 — measurement results + operator direction

**Post-fix ladder (chaining 15 mm + ramped entries): 15,376 s = 4.27 h**,
0/0, verdict OK. Pencil −792 s (entries 2,412 → 1,575 s; worst entry bite
3.72 → 2.25 mm — entry_load still grades Critical, but the residual is
wall-column semantics: entries now cut like the pass body, whose own peak
is 2.83). Chaining honest result: rivers entries 211 → 36 s but links cost
166 s at feed — net −32 s combined (762 s vs target ≤350; the
entries+rapids "prize" was mostly cheap rapids, not slow plunges; the
feature is correct and cost-gated, just modest HERE). Live confirms on the
new binary: heights tab pure read, sticky dead, helix generates (7,659
moves, cutting 3.2 km SHORTER than plunge — C5 A/B unblocked), scallop
0.03 + raster 0.6 generates without panic on the real project.

**C2 arm measured: 17,088 s = 4.75 h (+28.5 min)** — whole surface ~30 µm
cusps (finish 113k → 169k moves, 74 → 101 km). Saved as
`wanaka200_p2_c2.toml`; render `p2_c2_final_stock.png`. C3 (rest-only
detail) argued structurally worse for THIS defect (uniform cusps ⇒ a
rest pass re-cuts the same area plus overhead) — offered, not run.
Recommendation to operator: C2 is the keeper.

**Operator follow-ups (2026-08-23 evening):**
1. Pencil "cutting through mountains on travel moves" — REAL: surface
   links (hookup 30) follow the MESH, not stock; fix = LinkCeiling on
   pencil links (machinery landed with chaining) — agent dispatched.
2. Multi-tool island finishing idea (fine balls scallop the mountains,
   big balls the flats, island filtering to avoid 1000s of islands) —
   investigation prompt written:
   `planning/multitool_2026-08-23/INVESTIGATION_PROMPT.md`. Next session
   investigates code/UX/math + runs the config-only two-tier arm, and
   writes the orchestration plan for the session after.

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

## Session 2 — 2026-08-23 — multi-tool island finishing investigation

Ran `planning/multitool_2026-08-23/INVESTIGATION_PROMPT.md`: three
read-only Opus agents (T1 reachability math, T2 island machinery, T3 UX
— findings docs beside the prompt, every claim file:line) + the T4
config-only two-tier arm measured live. Deliverable written:
`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md`.

**T4 arm 1 (`wanaka200_mt1.toml`, branched from C2): 36,892 s = 10.25 h
vs C2's 17,088 s — LOSES by 5.5 h.** 0/0 collisions, verdict OK, gates
9/9 Within with real populations (tier-A chipload 630k contributing
samples, tier-B 334k). Config: tier-A = C2 finish retooled R1.5→R2.0 at
equal 30 µm cusp (raster_stepover 0.6→0.69, same feeds); tier-B = new
rest-driven R1.0 unified (claims_reference=machined_stock,
min_rest_depth 0.03, Lakes' proven R1.0 feeds 1062/180/19000); pencil
unchanged. Render `mt1_final_stock.png` (clean).

Per-op attribution (cut-trace runtimes):

- **Tier-A R2.0: 5,019 s.** C2's finish+pencil ≈ 9,334 s (total minus
  shared ops) → tier-A + collapsed pencil (203 s) saves ~4.1k s. **The
  big-tool-on-flats half of the operator's idea WORKS and defines the
  budget: a fine tier has ≈ 4,100 s to spend.**
- **Tier-B R1.0: 23,916 s — the killer.** 16,140 s of RAPIDS (19,137
  retract round-trips at ~0.84 s, 19,132 of them *inside* routing nodes)
  + 7,329 s cutting 89 km ≈ near-full re-coverage. narrate: MidSteep
  scallop band was ONE region node of 146,872 moves — 14 region nodes
  total, so the fragmentation is intra-region confetti, not
  1000s-of-islands.

Attribution of the eaten margin (the prompt's question): (1)
**over-selection** — min_rest_depth 0.03 sits AT tier-A's own cusp
height, and (T1's finding) the drop-cutter residual is a tool-CENTRE
surface difference biased by R·(sec θ − 1): R2-vs-fine reads ~0.6 mm at
45° on slopes both tools machine perfectly, so the whole mid-steep band
qualified; (2) **fragmentation** — the rest filter punches passes into
fragments that each pay a full retract. Same signature the July
selective-finishing ledger measured (cutting −80%, rapids 12×). Tier
overlap was NOT a factor. Measurability stated honestly: at 0.15 mm
cells 58% of tier-B's removing samples read zero engagement (R1.0 tip
below cell resolution) — time/collision verdicts stand, engagement
grades on the fine tier do not; 0.1 OOMs this board.

**T4 arm 2 (A/B sharpener, `wanaka200_mt1b.toml`: tier-B min_rest_depth
0.03→0.05, above tier-A's cusp): PENDING — ladder in flight at time of
writing, result appended below when measured.**

Session note: the CLI session crashed mid-A/B (memory pressure, watcher
logged 6G available at 14:39); the original GUI (held the arm-2
generation, unsimulated) was orphaned by the dead MCP pipe and killed;
arm 2 re-ran from `wanaka200_mt1b.toml` in a fresh GUI.

Investigation headlines feeding the plan (details in T1/T2/T3 docs):
the per-tool residual map already ships (`rest_field::detect_rest_valleys`
+ `attach_generic_rest_analysis` + `BoundarySource::DerivedRestRegions`,
the ONLY pre-decompose boundary source); unified_finish already takes an
external `machining_boundary` RegionSet ahead of decomposition; all
morphology (hysteresis/close/min-area/merge) exists in `finish_planner`;
the operator's dials (merge radius, min island area) exist un-exposed in
`FinishPlannerParams`. Blockers: B1 slope bias on the residual mask
(no sec θ compensation anywhere), B2 envelope-radius dilation welding
tapered-tool masks solid (3.5 mm on a Ø1-tip taper), B3 intra-island
routing (the 16.1k s measured above), plus MAX_REST_REGIONS=64 silent
cap. UX verdict: op-chain (one op per tier), Optimize-project-style
veto panel, tier-map preview through the rest-heatmap slot; the GUI
lacks the MCP-only rest-chain fixpoint and must gain it. Costs: per-tool
map ≈ 8 s @0.6 mm / ~31 s @0.3 per tool — planner-cheap; plan tiers at
0.3–0.6 mm, never 0.15.
