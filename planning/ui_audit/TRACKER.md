# UI/IA Cleanup — Execution Tracker

> Living status board for the IA cleanup + egui-0.34 rewrite. Companion to the other
> `planning/ui_audit/` docs: **`BACKLOG.md`** = ranked what/why · **`ARCHITECTURE.md`** =
> component layer · **`FINAL_DESIGN.md`** = target mockups · **this** = how/when/who/status.
>
> **Committed 2026-06-08.** Scope: clean up what exists (not new features); full send to
> egui 0.34.3; rewrite the worst surfaces on a shared component layer; MCP/assistant deferred
> but backend left MCP-ready.
>
> Status: ☐ todo · ◐ in progress · ☑ done · ⊘ blocked. Context: 🧠 keystone (needs full
> plan context — main context or one dedicated high-context owner, **do not fan out**) ·
> 🔁 fan-out-safe (mechanical or per-surface-disjoint — parallel agents fine) · 🛠 focused
> (one agent, single-subsystem context).

## ▶ Next action
**Wave 0.** Two concurrent tracks (crate-disjoint, so no worktree-collision risk):
Track A — egui-0.34 upgrade PR (viz). Track B — Tier-0 engine fixes (core). Track C —
W2.1 provenance data model (core, keystone, careful). Start by standing up the screenshot
baseline (see Wave 0 notes) before touching deps.

---

## Execution model (the rules we committed to)

1. **One working tree for the pervasive viz work.** No worktree isolation on the upgrade
   or component layer — they touch the same recently-modified files and worktrees would
   merge-thrash ([[feedback_worktree_extraction]]). Worktrees only for genuinely disjoint,
   read-mostly tasks (like the spike was).
2. **Parallelize across the crate boundary, not within viz files.** `rs_cam_core` work
   (provenance, apply-split, engine fixes) and `rs_cam_viz` work (upgrade) have disjoint
   file sets → safe to run concurrently. Within viz, one pass at a time until the component
   layer exists.
3. **Stacked small PRs**, each independently verifiable, each leaving the tree green
   (clippy zero-warnings, per-crate tests, `cargo fmt` whole-tree).
4. **Keystones are not delegated.** The component layer, the provenance data model, and the
   provenance visual language define contracts everything else depends on — done with full
   plan context, reviewed carefully, before the fan-out work that consumes them.
5. **After the component layer exists, per-surface rewrites fan out** — disjoint files, one
   agent/PR per surface.

---

## Parallelization map

```
WAVE 0  (concurrent — crate-disjoint)                         BARRIER: W-UP + W2.1 + W0.4/W0.5 land
 ├─ Track A (viz, serial focus) ── W-UP egui 0.34 upgrade ─────┐
 ├─ Track B (core, fan-out)     ── W0.2 W0.3 W0.4 W0.5 W2.2 ───┤
 └─ Track C (core, keystone)    ── W2.1 provenance data model ─┘
                                                               │
WAVE 1  (keystone, mostly serial)                              ▼
 ├─ 🧠 COMPONENT LAYER (incl. W4.1 provenance visual language) ── BARRIER: components stable
 └─ 🔁 alongside (disjoint viz files): W1.1 tool CRUD revive, W1.2 dead-control fixes,
        W0.1 viz half (fixture pill)
                                                               │
WAVE 2  (fan-out — one agent/PR per surface, all disjoint)     ▼
 ├─ 🧠 W3.1 feeds rewrite (epicenter — keep high-context)
 ├─ 🔁 W3.3 inspector  · W3.4 tools  · W3.5 optimizer  · W3.6 timeline
 └─ 🔁 W3.2 tab scaffold · W3.7 header/rail
                                                               │
WAVE 3  (consolidation + polish — fan-out)                     ▼
 ├─ W3.8 job-readiness dashboard (needs W0.1 + W0.4 + W0.5 + components)
 └─ 🔁 W4.2 W4.3 W4.4 W4.5 · W5.1 W5.2 W5.3 W5.4  (batch with the surface each touches)
```

**Serialization barriers (the only hard ordering):**
- Component layer **after** W-UP (Atoms substrate) + W2.1 (honest provenance) + W0.4/W0.5
  (so `CountPill`/`FreshnessGate` read the unified producers).
- All ♻ surface rewrites + W3.8 + Tier-4/5 visual work **after** the component layer.
- W4.1 (provenance visual language) is folded **into** the component layer — `ProvenanceBadge`
  *is* the visual language; designing it twice is the bug we're trying to avoid.

---

## What needs larger context vs what fans out

**🧠 Keystone — full plan context, single owner, reviewed, NOT fanned out:**
| Item | Why it needs the whole picture |
|---|---|
| Component layer (`ARCHITECTURE.md`) | Defines the contracts every surface consumes; a wrong signature propagates everywhere |
| W2.1 provenance data model | Foundational core change touching `OperationConfig` + every `set_*`/read site; design-sensitive |
| W4.1 provenance visual language | Cross-cutting; one glyph+RGB per source used on ~8 surfaces — must be decided once |
| W3.1 feeds rewrite | The epicenter; the SPEED/CUT split couples core + UI; highest blast radius of the rewrites |
| Integration & sequencing | Keeping the stacked PRs green and in order |

**🛠 Focused — one agent, single-subsystem context:**
W-UP (wgpu `render/mod.rs` + the eframe shell — understand once, then repetitive), each
Tier-0 fix (collision / optimizer math / rollup / staleness / tally are distinct subsystems),
W1.1 tool-CRUD revive.

**🔁 Fan-out-safe — parallel agents, mechanical or per-surface-disjoint:**
W-UP deprecation renames across leaf files (after the hot files compile), W1.2 dead-control
fixes, the Wave-2 per-surface rewrites (inspector/tools/optimizer/timeline/header), and all
Tier-4/5 polish.

---

## Workstream status

| ID | Title | Crate | Track | Ctx | Depends on | Status |
|----|-------|-------|-------|-----|------------|--------|
| W-UP | egui 0.30→0.34.3 upgrade | viz | 🟥 | 🛠→🔁 | — | ☐ |
| W0.1 | fixture→collision (safety) | core+viz | 🟪 | 🛠 | — (viz half: components) | ☐ |
| W0.2 | optimizer cycle-time math | core+viz | 🟥 | 🛠 | — | ☑ |
| W0.3 | one collision tally + severity order | viz | 🟥 | 🛠 | — | ☑ |
| W0.4 | reconcile load rollups (one `summary()`) | core+viz | 🟥 | 🛠 | — | ☑ |
| W0.5 | wire `is_stale` everywhere | viz | 🟥 | 🛠 | — | ☐ |
| W2.1 | `ValueProvenance` data model | core | 🟥 | 🧠 | — | ☐ |
| W2.2 | canonical post-config | viz | 🟥 | 🛠 | — | ☐ |
| W1.1 | revive tool CRUD (kill `project_tree`) | viz | 🟪 | 🛠 | W-UP | ☐ |
| W1.2 | wire/retire dead controls (×4) | viz | 🟥 | 🔁 | W-UP | ☐ |
| CL | **component layer** (+W4.1) | viz | 🟦 | 🧠 | W-UP, W2.1, W0.4, W0.5 | ☐ |
| W3.1 | feeds rewrite + core split | core+viz | 🟪 | 🧠 | CL | ☐ |
| W3.2 | five-tab scaffold | viz | 🟦 | 🔁 | CL | ☐ |
| W3.3 | inspector summary-first | viz | 🟦 | 🔁 | CL | ☐ |
| W3.4 | tool editor (commit + grouping) | viz | 🟪 | 🔁 | CL, W1.1 | ☐ |
| W3.5 | optimizer affordances | viz | 🟦 | 🔁 | CL | ☐ |
| W3.6 | timeline disambiguation | viz | 🟦 | 🔁 | CL | ☐ |
| W3.7 | header/rail de-overload | viz | 🟦 | 🔁 | CL | ☐ |
| W3.8 | job-readiness dashboard | viz | 🟦 | 🔁 | CL, W0.1, W0.4, W0.5 | ☐ |
| T4 | confusables/visual lang (W4.2-4.5) | viz | 🟦 | 🔁 | CL | ☐ |
| T5 | discoverability/prose (W5.1-5.4) | viz | 🟦 | 🔁 | CL | ☐ |

(Effort + finding-ids per workstream live in `BACKLOG.md`. 🟥 code · 🟦 spec · 🟪 both.)

---

## Wave 0 notes (the immediate work)

- **Screenshot baseline first.** Before any dep bump, capture the current 0.30 UI on a
  handful of representative surfaces (properties tabs, feeds, inspector, timeline, viewport)
  so "visual parity" for W-UP is checkable, not vibes. (MCP isn't connected this session →
  manual `cargo run -p rs_cam_viz` + screenshots.)
- **W-UP gotchas already known** (from `SPIKE_egui034.md`): `egui_plot = "0.35"` (NOT 0.34);
  wgpu reached via `use egui_wgpu::wgpu;` (no separate pin); deprecations **must** be fully
  migrated (zero-warning lint policy), not left as warnings.
- **Track B can start immediately** — Tier-0 core fixes are engine logic, unit-testable,
  and don't touch the viz files W-UP churns. Highest harm-reduction in the plan.
- **Verification discipline:** per-crate `cargo test -p …` (never workspace-wide), `cargo
  clippy --workspace --all-targets -- -D warnings`, `cargo fmt` whole-tree. Don't run
  `cargo test` while a viz release build runs (swap thrash — `pgrep` first, bracket a char).

---

## Status log
- **2026-06-08** — Plan committed. Audit (76 findings, 2 passes), BACKLOG v2, ARCHITECTURE,
  FINAL_DESIGN, and egui-0.34 spike complete. Tracker created. Nothing implemented yet;
  Wave 0 is next.
- **2026-06-08** — Wave 0 / Track B started. **W0.2 ☑** — optimizer headline cycle-time
  math (OPT-001). Moved the project-rollup math out of the viz header into a tested core
  method `ProjectOptimizeReport::optimized_cycle_time_s(&selected)` (MCP-ready); it now
  computes `baseline − Σ realized savings` so Skipped/NoSafeImprovement rows keep their real
  cost instead of folding to zero (which understated optimized time + inflated savings %).
  Deleted the buggy `compute_optimized_cycle` from `optimize_project.rs`. 2 new core
  regression tests; clippy + fmt + core tests green. (Note: rust-analyzer is surfacing stale
  wgpu-29 errors in `render/mod.rs` from the spike worktree — the real compiler is clean on
  wgpu 23; ignore until Track A.)
- **2026-06-08** — **W0.3 ☑** — collision tally + severity order (SHE-002/SHE-003). Added
  one canonical accessor `SimulationChecks::total_collision_count()` (holder + rapid) and
  routed all five readouts through it (status bar via `app.rs`, both `workspace_bar` badges,
  `sim_timeline`, `sim_diagnostics`) — the status bar previously counted holder-only and
  diverged from the workspace bar. Reordered `simulation_badge` to check collisions *before*
  staleness so a red error can't hide behind the yellow "stale" (mirrors `readiness_badge`).
  `collision_positions()` stays holder-only (viewport markers). clippy + fmt + 189 viz tests
  green.
- **2026-06-08** — **W0.4 ☑** — reconcile load rollups (P4-001/P4-002). Migrated the verdict
  HUD (`sim_timeline.rs`) off the per-`(toolpath × gate)`-criterion `verdict_counts()` onto
  the same `ToolLoadReport::summary()` the Inspector overview already uses, so the two project
  rollups can no longer print different numbers for one load concept. Pills now carry a `/T`
  denominator (`load N/T`, `exceeds N/T`, `unmodeled N/T`) proving both count the same
  universe, and the tooltips that mislabeled a per-criterion count as "Toolpaths…" are now
  honest. Dropped the `~ approx` pill (a per-criterion count that reintroduced the exact
  confusable, and absent from the Inspector) and deleted `verdict_counts` + its now-unused
  `Confidence` import. clippy + fmt + 189 viz tests green. **First-cut correctness trio
  (W0.2+W0.3+W0.4) complete.**
