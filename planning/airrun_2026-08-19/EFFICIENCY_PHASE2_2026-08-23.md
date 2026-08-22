# wanaka200 optimisation — phase 2 plan (2026-08-23, operator-commissioned)

Operator directions: (1) put Lakes on the same 20° V-bit as Rivers and
combine them — and if curve chaining is an easy win, build it; (2) the
pencil's entries are cutting through the stock and that surprised nobody's
gate — fix the behaviour and the blind spot; (3) the hills off the R1.5 are
a little too rough — consider a rest-only detail pass (unified/scallop) to
bring detail back. Phase 2 is therefore **quality-first with code changes**,
unlike phase 1 (parameters only). Baseline: `wanaka200_fast2.toml`,
16,248 s (4.51 h), gates 8/8, 0/0.

## Working pattern

Code changes go to Opus implementation agents (one cargo lane, never-touch
list unchanged, red-first sentries, full per-crate gates); GUI validation
follows each landed fix on a rebuilt release binary. GUI-side rules carried
over: sims @0.15, full rest-chain ladders, op-inventory check after any
generation (G-STICKYEMPTY), never open the Heights tab on a healthy op
(G-HEIGHTSTAB), reload after any empty generation.

## P2-0 — Foundation fixes the whole phase leans on (CODE, first)

1. **Generated-empty must refuse, not succeed.** Root gap under
   G-HEIGHTSTAB's blast radius, G-ENTRYEMPTY, and G-STICKYEMPTY: a
   generator that emits 0 moves from a non-empty region returns success and
   nothing gates it. Turn it into a typed refusal (with the zero_removal
   report kept for genuinely-empty regions); sentry on the helix-entry
   repro.
2. **G-STICKYEMPTY**: find and fix the poisoned cache an empty generation
   leaves behind (repro sequence documented in the phase-1 ledger).
3. **G-HEIGHTSTAB**: opening the Heights properties tab must not commit
   resolved heights / mark stale / regenerate. Repro: select toolpath +
   heights tab, watch a healthy op empty itself.
4. **G-UNIFIEDCRASH**: `unified_finish` panics (index OOB) at
   scallop_height 0.03 + z_step 0.6; param isolation then fix. This is the
   direct unlock for P2-C's cheapest quality path.

## P2-A — One V-bit for Rivers + Lakes (GUI first, then CODE)

- A1 (GUI, no code): switch Lakes' tool to the 20° V-bit; reorder so the
  two V-bit ops are adjacent → one tool change removed immediately.
  **Operator eyeball required on the first render**: at 0.3 mm depth the
  V-bit groove is ~0.11 mm wide vs the R1.0's ~0.5 mm — different look for
  the lake outlines. If the look fails, A1 reverts and only chaining
  proceeds.
- A2 (CODE, feasibility-gated): **project_curve curve chaining** — link
  curve endpoints within a `chain_distance_mm` with stay-down/low-hop moves
  instead of retract → rapid → re-entry. Phase-1 measured the prize:
  Rivers spends 549 s of its 707 s on entries+rapids; Lakes similar ratio.
  First step is a scoped read of the project_curve generator to price it
  ("easy win" check); if the linking layer is genuinely per-curve-loop
  local, this is a contained feature with a distance dial + a sentry on
  retract-trip count. If it wants a planner rework, report back before
  building.
- A3 (GUI): re-measure rivers+lakes with chaining on; target ≤ 350 s
  combined (from 833 s).

## P2-B — Pencil entries: behaviour + the gate blind spot (CODE + GUI)

- B1 (GUI, evidence): render/scrub the worst pencil entries (the
  crosses-standing hits, peak 3.72 mm bite) and confirm the mechanism —
  fed entry descending through the valley wall above the target segment.
  Screenshot for the ledger; this is the operator's observed gouge.
- B2 (CODE, behaviour): pencil entries should arrive without carving: ramp
  along the valley centreline (the path already exists — the segment being
  entered), or clamp the entry XY to start where the surface is already at
  target depth. Design choice for the agent to propose; sentry = no
  entry-intent sample removes more than k× the pass median bite.
- B3 (CODE, the blind spot — G-ENTRYLOAD): the load gates' steady-state
  filter excludes entry/transit samples wholesale, so a 3.7 mm bite on an
  R0.5 at entry gets zero deflection/chipload scrutiny. Either grade entry
  samples against their own envelope or surface a typed caution when entry
  bites exceed the pass median multiple. (The crosses-standing caution
  already fires; it needs to say ENTRY and be severity-honest.)
- B4: side effect to re-measure — fixed entries may also cut the pencil's
  remaining ~3.1 k s entry time (descend less, carve less, possibly at
  cutting feed along the valley instead of the 150 mm/min vertical cap).

## P2-C — Hills surface quality (diagnose → two candidate fixes → A/B)

- C1 (GUI, diagnose): measure what "too rough" is. Suspect #1: the unified
  mid-steep band still runs **scallop_height 0.1 (100 µm)** — the
  G-UNIFIEDCRASH workaround, never re-tightened. Suspect #2: the finish's
  crosses-standing bites (peak 3.25 mm) tearing grain where the rough
  skipped shallows. Render + zoom the hill faces; narrate band mix on the
  hill region.
- C2 (after G-UNIFIEDCRASH fix): single-pass fix — scallop_height 0.03,
  z_step to match; re-price (path grows on the scallop band only; estimate
  +8–15 min). This is the simple candidate.
- C3 (operator's idea): rest-only detail pass — a second unified/scallop op
  with `claims_reference = machined_stock` + `min_rest_depth ≈ 0.03` so it
  cuts ONLY where ridges stand above the surface, R1.5, no new tool change.
  Prerequisite (from the rest-measurement rules): sim cell well below the
  R1.5 tip radius and claims_reference=machined_stock in the cascade.
- C4: A/B C2 vs C3 on time + rendered surface; **the operator's eye is the
  accepting gate** for this item, not a metric. Budget: whichever wins may
  spend up to ~+30–45 min of cycle time; the current 4.51 h has room.
- Note for C3 pricing: mill_shallow_areas on the front rough (never
  A/B'd solo) would reduce the standing material that feeds both suspects;
  worth one arm if C1 fingers the bites rather than the scallop.

## Order & wall estimate

P2-0 first (agents, ~one session incl. gates), then A1 (30 min GUI) → B1 +
C1 evidence while agents work → A2/B2/B3 land → rebuild + validate → C2/C3
A/B → consolidate to `wanaka200_v3.toml` + exports + render + wall-clock
prediction. Expect: quality up on hills and valley walls, rivers+lakes
≈ −450 s, pencil entries safer and possibly faster; total likely lands
4.2–4.6 h depending on how much C spends.

## Post-compact continuation prompt (paste after /compact)

Continue rs_cam in /home/ricky/personal_repos/rs_cam (branch master).
Read planning/airrun_2026-08-19/EFFICIENCY_PHASE2_2026-08-23.md FIRST —
it is the whole agenda. Phase-1 ledger: OVERNIGHT_TUNING_2026-08-23.md.
Candidate on disk: wanaka200_fast2.toml (16,248 s, gates 8/8). Standing
constraints: never-touch (.mcp.json, wanaka.toml, workspace Cargo.toml,
benches/hot_paths.rs, tests/perf_golden_*, planning/perf_review_2026-08-19/),
one cargo job machine-wide (pgrep -x cargo + /proc cwd, never pgrep -f), no
history rewriting, don't pipe cargo test through head, sims @0.15 (0.1 OOMs
this board), Fable orchestrates + Opus agents implement, verify op
inventory after every generation, never open the Heights tab on a healthy
op, reload after any empty generation. Start with P2-0 via implementation
agents; do A1/B1/C1 GUI evidence in parallel only if the GUI is up
(pre-built release binary required before MCP connect).
