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
**W3.5 DONE + W3.4 mostly done (TOO-003 left) — W3.7 header/rail is the last full Wave-2 surface.**
CL + W3.1 + W3.2 + W3.3 + W3.6 + W3.5 are merged; W3.4 merged its editor-clarity findings
(TOO-004/005/006) with **TOO-003 (draft/Apply commit model) deferred**. Remaining: the **TOO-003
sliver**, then 🔁 **W3.7 header/rail** (which builds `components::visibility` with the viewport
overlay), then W3.8 dashboard + Tier-4/5 polish. Each full surface: fresh branch off master,
consumes the component layer (`ARCHITECTURE.md` §4 usage map).

⚠ **TOO-003 (deferred, focused follow-up):** make the **properties-panel** tool editor
draft-commit — edit a `draft` clone, commit on an explicit **Apply** (or auto-commit on
navigate-away as the spec's accepted fallback), show a "● modified — Apply / Revert" affordance —
to match the modal's draft-then-Save model and retire the live-apply-vs-draft inconsistency
(TOO-003). It's a `GuiState` field + `properties/mod.rs` Tool-branch + `flush_tool_snapshot`
rework that interacts with the undo snapshot and `invalidate_tool`; do it with fresh context so
a commit/invalidation bug can't silently drop a tool edit. The modal already draft-commits — only
the panel changes.

⚠ **Visual-verify debt (MCP can't capture egui panel chrome — human `cargo run` pass):**
W3.4 tool editor (Shaft/Shank split, holder header badge, Catalog-metadata fields, "Updated"
vs "Saved" toast). [W3.5 already confirmed.]

**`compare::*` not adopted in W3.5 (deliberate):** the rollup header is two cycle-time numbers
("Current / Optimized (-26.6s, -10%)"), which `delta_tag`'s "↓ 0.90×" ratio renders *less*
clearly than the existing absolute+percent. `CompareRow` is also feeds-field-specific (ties to
`ApplyFeedsField`). So compare:: didn't cleanly fit — the deferral from CL/W3.1 ("optimizer adopts
compare:: if it adds a compare view") is closed as *not-applicable*, not carried forward.

⚠ **Component-build carry-over still open:** `components::{visibility,nav,diagram}` were NOT built
in W3.6 — deferred with rationale: the Tier-2 widget is interactive (doesn't fit diagram.rs's
paint-only `InlineDiagram`), `nav::SimMove` is ceremony over `SimJumpToMove`, and `visibility`'s
real consumer is the viewport overlay (W3.7). Build each with the surface that genuinely uses it.
Also deferred: TIM-008 metric-chip row (no per-track gate signal for 4 of the 5 spine tracks).

⚠ **Execution constraint (don't true-fan-out the builds):** [[feedback_no_concurrent_release_builds]]
— parallel viz builds thrash swap and crash the PC. So although the surfaces are file-disjoint,
run them **sequentially in the main tree** (one surface → commits → green → merge `--no-ff`,
then the next), the same proven rhythm CL/W3.1/W3.2 used. Fan-out agents are only safe if their
builds are serialized.

Carry-overs the rewrites still pick up (from CL + W3.1 deferrals): ~~W3.3 adopts `CountPill` for
the Inspector Findings grid~~ ✓ DONE (W3.3); W3.5 adopts `compare::*` if it adds a compare view;
`components::{visibility, nav, diagram}` get built with the surfaces that use them.

**W3.2 landed** (2 commits, `ia-cleanup/w3.2-tabs`, merge `caed022`):
- **1/2** (`2994b84`) five-tab recharter `[Params][Feeds][Heights][Dressups]` →
  `[Geometry][Feeds & Speeds][Linking][Heights][Dressup]`. Machining Boundary moved to Geometry;
  `draw_dressup_params` split into `draw_linking_params` (Entry & Exit + Optimization + Retract)
  + `draw_dressup_params` (Path Quality). Tab enum/labels/badges renamed; default tab Geometry.
- **2/2** (`122e82c`) feed/plunge relocated into the Feeds SPEED section as editable `ValueRow`s
  with per-field ⚡; `draw_feed_params` deleted (21 call sites); bulk "Suggest all (LUT)" button
  retired from Geometry (feeds_result still cached for the stepover/DOC pills); manual SPEED
  editor added to the Feeds engine-refusal branch. Drill keeps its Z-only feed on Geometry.

**W3.1 landed** (3 stacked commits, `ia-cleanup/w3.1-feeds`) — the feeds epicenter:
- **1/3** core SPEED/CUT split: `apply_speeds_to_op` + `apply_cut_geometry_to_op` beside
  `apply_feeds_result_to_op` (all via a scratch-clone `apply_feeds_subset`; subset applies are
  byte-identical to the combined apply for the fields they write). `apply_suggested_subset`
  mirrors it. Callers unchanged. 2 unit tests lock per-axis isolation.
- **2/3** Feeds card → SPEED / CUT / Derived `named_section`s. "⚡⚡ Apply recommended speeds"
  is SPEED-only; "⚡ Apply cut geometry" is the separate attributed cut apply. Derived uses
  `compare::power_bar`/`mrr_row`. Dropped the old un-provenanced per-field set buttons.
- **3/3** spindle `PrecedenceField` (shows override vs real project default `〈N〉`): relocated
  from every op's Params tab into the Feeds SPEED section (its end-state home), so W3.2 inherits
  it. Removed spindle from `draw_feed_params` (+21 sites); deleted `draw_spindle_rpm_row` +
  `suggest_pill`. Fixes P1-005/P2-006.

**CL landed** (4 stacked commits, `ia-cleanup/components`) — the duplication-killer substrate:
- **1/4** foundations: `ProvKind`/`ProvenanceBadge` (one glyph+RGB per source, `From<&ValueProvenance>`),
  `ValueRow` (supersedes `dv`/`dv_pill`), `SuggestButton`, `UiExt`/`SummaryCard`. `dv`/`dv_pill`/
  `suggest_pill` delegate; `pill_color_for_source`/`source_short_label` deleted.
- **2/4** collapsed the provenance source narratives (properties cyan + feeds_modal green) onto
  `ProvenanceBadge` — kills the P7-003 three-colours divergence.
- **3/4** `CountPill` (`[ ]`/`{ }`/`( … → )` grammar) + `FreshnessGate` (one stale renderer,
  `theme::stale_banner` removed). Verdict HUD + all 4 stale-banner sites migrated.
- **4/4** lifted `CompareRow`/`delta_tag`/`power_bar`/`mrr_row`/`format_optional` out of
  `feeds_modal` into `components::compare`; six private fns deleted.

**Deliberate CL deferrals (NOT bugs — pick up in the surface rewrite that owns each):**
- `ProvenanceBadge` is built + reads the stored `ValueProvenance`, but the **live per-op feeds
  pill still recomputes** a fresh lookup — its rewiring is **W3.1** (those widgets are rebuilt
  there; threading display through them now = throwaway). [decision 2026-06-08]
- The **Inspector "Findings" rollup** still renders as a grid (not `CountPill`); it already reads
  the same `summary()` producer so it can't diverge — its CountPill adoption is **W3.3**.
- The **optimizer** rollup shows a `ParamDelta` string, a different concept; it adopts
  `compare::*` if/when **W3.5** adds a compare view.
- `components::{visibility, nav, diagram}` from `ARCHITECTURE.md` §1 were **not built** — they pay
  off only inside the surfaces that consume them (W3.6/W3.7/diagrams); build them with those.

— prior context —
**Tier 2 landed**: **W2.2** [P1-004] — post-panel edits write straight to the canonical
`session.post_config()` (guarded so the sim cache only drops on a real edit); added `PartialEq`
to `ProjectPostConfig`. GUI-vs-MCP stale window closed.

— prior context —
Wave 0 + Tier 1 are merged to master; **W2.1 is DONE** (per-field `ValueProvenance`).

**W2.1 landed** (`ValueProvenance` data model, core keystone) — decisions taken:
`ValueProvenance { source, reference }` (**no `when`** — deferred), **full per-field
independence** (RPM/DOC/WOC labelled from the matched LUT row's `rpm_*`/`ap_*`/`ae_*`
independently of the chipload's `ChiploadSource`), and **full apply-site consolidation**.
- `feeds::provenance` module: `ProvenanceSource` (VendorLut/Formula/EdgeRadiusFloor/Manual/
  Optimizer/AutoCorrect), `ValueProvenance`, `FeedsProvenance` (per-dimension), `FeedsField`,
  `FeedsResult::provenance()`. `ToolpathConfig.feeds_provenance` + serde via BOTH project-IO
  DTOs (core `project_file.rs` + viz `io/project.rs`), `#[serde(default)]` → back-compatible.
- Stamped at every producer: suggest funnel (`apply_feeds_result_to_op` gained `&mut
  FeedsProvenance` + `SuggestedParams.provenance`; per-field controller apply; CLI; GUI/MCP
  add-toolpath), manual `set_param` + GUI in-place edits (diff-detected at the entry→session
  flush via `detect_manual_edits`), optimizer (`stamped_optimizer` + `set_feeds_provenance`),
  auto-correct. 5 unit tests lock per-field independence + manual detection.
- **Deferred to W4.1 / component layer (deliberate):** the *rendering* repoint — the per-op
  pill still shows the recomputed suggestion source. `ProvenanceBadge` IS the visual language
  (folded into CL), and W3.1 rewrites these exact feeds widgets, so threading display through
  them now would be throwaway. The honest data model is in place and correctly populated; CL
  reads it. **MCP-ready** backend unification done.

**W2.2** [P1-004] (S, viz): `gui.post` and `session.post_config()` can disagree until next
save (GUI-vs-MCP window). Make the session canonical; route post-panel edits through a
`SetPostConfig`-style event that writes immediately (as the wizard already does). Ride it
along on this branch, then merge `ia-cleanup/tier2` → master.

Standing instruction from user: **default to merging a completed green workstream branch to
master without asking.**

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
| W-UP | egui 0.30→0.34.3 upgrade | viz | 🟥 | 🛠→🔁 | — | ☑ (visual-verify pending) |
| W0.1 | fixture→collision (safety) | core+viz | 🟪 | 🛠 | — (viz pill: components) | ◐ |
| W0.2 | optimizer cycle-time math | core+viz | 🟥 | 🛠 | — | ☑ |
| W0.3 | one collision tally + severity order | viz | 🟥 | 🛠 | — | ☑ |
| W0.4 | reconcile load rollups (one `summary()`) | core+viz | 🟥 | 🛠 | — | ☑ |
| W0.5 | wire `is_stale` everywhere | viz | 🟥 | 🛠 | — | ☑ |
| W2.1 | `ValueProvenance` data model | core | 🟥 | 🧠 | — | ☑ |
| W2.2 | canonical post-config | viz | 🟥 | 🛠 | — | ☑ |
| W1.1 | revive tool CRUD (kill `project_tree`) | viz | 🟪 | 🛠 | W-UP | ☑ |
| W1.2 | wire/retire dead controls (×4) | viz | 🟥 | 🔁 | W-UP | ☑ |
| CL | **component layer** (+W4.1) | viz | 🟦 | 🧠 | W-UP, W2.1, W0.4, W0.5 | ☑ |
| W3.1 | feeds rewrite + core split | core+viz | 🟪 | 🧠 | CL | ☑ |
| W3.2 | five-tab scaffold | viz | 🟦 | 🔁 | CL | ☑ |
| W3.3 | inspector summary-first | viz | 🟦 | 🔁 | CL | ☑ |
| W3.4 | tool editor (commit + grouping) | viz | 🟪 | 🔁 | CL, W1.1 | ◐ (TOO-003 left) |
| W3.5 | optimizer affordances | viz | 🟦 | 🔁 | CL | ☑ |
| W3.6 | timeline disambiguation | viz | 🟦 | 🔁 | CL | ☑ |
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
- **2026-06-08** — **W0.1 ◐** (CODE half ☑, top-priority safety) — fixture→collision
  (P6-003). The collision check tested the assembly only against the workpiece mesh; a holder
  crashing a clamp was never flagged. Added `CollisionKind`/`CollisionObstacle` + analytic
  `check_obstacle_collisions_with_cancel` in core `collision.rs`, an `obstacles` field on
  `CollisionCheckRequest`, and one shared builder `ProjectSession::
  collision_obstacles_for_toolpath` (enabled fixtures → clearance-expanded boxes). Wired BOTH
  call sites through it: core `session.collision_check` (headless + GUI-embedded MCP) and the
  viz worker (`CollisionRequest.obstacles` built at `request_collision_check`). +4 core unit
  tests; clippy + fmt + core collision + 189 viz tests green. **SPEC half deferred** to the
  component layer (per-fixture clearance pill, 🛡 field markers, edit-resets-pill). Frame note:
  non-identity-setup fidelity matches the existing mesh check; tracked separately if it
  surfaces. **Suggested first cut now: W0.2 ☑ W0.3 ☑ W0.4 ☑ W0.1(code) ☑ → next W1.1 + W1.2.**
- **Git:** branch `ia-cleanup/wave0`; commits: docs · correctness-trio · W0.1. Per-workstream
  commits from here. `.mcp.json` (pre-existing) left untouched.
- **2026-06-08** — **W-UP ☑ (code; visual-verify pending)** — egui 0.30→0.34.3 + wgpu 23→29.
  169→0 errors via the spike's map: egui_plot **0.35** (not 0.34), 5 wgpu descriptor renames
  in render/mod.rs, eframe `App::ui`+`show_inside` shell (app.rs `draw_frame`), egui_plot name
  args, rect_stroke StrokeKind::Middle, Margin/CornerRadius int, and all deprecations cleared
  (close_menu, menu::bar→MenuBar, Tooltip::always_open, global_style, run_ui test harness).
  clippy -D warnings + fmt + 189 viz tests + gui bin all green. Commit `0103376`. **Barrier
  lifted: the component layer + all ♻ Wave-2 rewrites are now unblocked.** ⚠ One open item:
  **visual parity unverified** — MCP screenshots capture the 3D render, not egui chrome; needs
  a human pass over panels/feeds-modal/timeline via `cargo run -p rs_cam_viz --bin rs_cam_gui`.
- **2026-06-08** — **W0.5 ☑** — wired `is_stale` at the 3 fresh-on-stale readouts
  (INS-005 reactive-inspector cards, TIM-009 timeline spine, OPT-003 optimizer baseline) via
  one shared `theme::stale_banner`. Commit `0576d03`. **Tier 0 COMPLETE** (W0.1c/W0.2/W0.3/
  W0.4/W0.5). W-UP visual parity confirmed by user.
- **2026-06-08** — **Wave 0 merged to `master`** (`99c70c8`, --no-ff: Tier 0 + W-UP as one
  revertable unit). New work branches off master per workstream.
- **2026-06-08** — **Tier 1 COMPLETE** on branch `ia-cleanup/tier1`:
  - **W1.1 ☑** (`5742240`) — harvested the full tool CRUD (Duplicate/Delete context menu, Del
    key, Manage-library, From-library import) into the live `toolpath_panel` Tool Library
    collapsible; **deleted the dead 446-LOC `project_tree.rs`** + its `pub mod`. A user can now
    delete/duplicate a tool and reach the library from the panel (couldn't before).
  - **W1.2 ☑** (`825caff`) — P6-001 wired Edit›Delete-Selected to the selection; P6-002
    retired the `StockVizMode::ByOperation` stub (uniform-color placeholder, not real per-op
    coloring — honest cut, not a misleading selector); P2-003 greyed the adaptive3d entry combo
    on `EntryStylePolicy::ForceNone`; TIM-010 retired the dead `10_000+i` gate-dot drill →
    jump-to-move.
- **2026-06-08** — **Tier 2 merged to master** (`f9673b2`, --no-ff): W2.1 per-field
  `ValueProvenance` data model + W2.2 canonical post-config.
- **2026-06-08** — **CL (component layer) COMPLETE** on branch `ia-cleanup/components`, 4 stacked
  commits each green (clippy -D warnings + fmt + viz 189+9):
  - **1/4** (`ac28637`) foundations — `ProvKind`/`ProvenanceBadge`/`ValueRow`/`SuggestButton`/
    `UiExt`/`SummaryCard`; `dv`/`dv_pill`/`suggest_pill` delegate; local colour/label helpers deleted.
  - **2/4** (`e17189c`) provenance-narrative collapse onto `ProvenanceBadge` (P7-003).
  - **3/4** (`4aa003b`) `CountPill` + `FreshnessGate`; HUD + 4 stale sites migrated; `stale_banner` removed.
  - **4/4** (`76abb54`) `components::compare` lifted from `feeds_modal`; six private fns deleted.
  Deferrals (by design, see ▶ Next action): live per-op pill rewiring → W3.1; Inspector Findings
  grid → CountPill → W3.3; optimizer compare adoption → W3.5; `visibility`/`nav`/`diagram`
  components built with the surfaces that consume them.
- **2026-06-08** — **CL merged to master** (`583ac4b`, --no-ff).
- **2026-06-08** — **W3.1 COMPLETE** on branch `ia-cleanup/w3.1-feeds`, 3 stacked commits each
  green (clippy -D warnings + fmt + core/viz tests):
  - **1/3** (`d809fdc`) core SPEED/CUT split — `apply_speeds_to_op` + `apply_cut_geometry_to_op`
    via a scratch-clone `apply_feeds_subset`; `apply_suggested_subset` mirrors it; callers
    unchanged; 2 isolation unit tests.
  - **2/3** (`008b614`) Feeds card → SPEED / CUT / Derived sections; speed-only + cut-only
    recipe applies; Derived via `compare::*`.
  - **3/3** (`619490d`) spindle `PrecedenceField` relocated to the Feeds SPEED section with the
    real project default; removed from `draw_feed_params` (+21 sites); `draw_spindle_rpm_row` +
    `suggest_pill` deleted. Fixes P1-005/P2-006.
- **2026-06-08** — **W3.2 COMPLETE** on branch `ia-cleanup/w3.2-tabs`, 2 commits each green
  (clippy --workspace -D warnings + fmt + viz 189+9), merged to master `caed022` `--no-ff`:
  - **1/2** (`2994b84`) five-tab recharter + dressup→linking/dressup split + boundary→Geometry.
  - **2/2** (`122e82c`) feed/plunge relocated into the Feeds SPEED section (editable ValueRows +
    per-field ⚡); `draw_feed_params` deleted; "Suggest all" button retired; drill feed stays on
    Geometry; manual SPEED editor on the feeds-refusal branch.
- **2026-06-08** — **Visual parity CONFIRMED** by the user for CL + W3.1 + W3.2 (live GUI pass:
  Feeds tab SPEED/CUT + editable feed/plunge + spindle precedence + the five-tab bar all good).
  The egui-chrome verification debt from those three workstreams is cleared.
- **2026-06-08** — **W3.3 COMPLETE** on branch `ia-cleanup/w3.3-inspector`, 2 commits each green
  (clippy --workspace -D warnings + fmt + viz tests), merged to master `099c4e4` `--no-ff`:
  - **1/2** (`dffe126`) CountPill Findings rollup (CL carry-over — same `summary()` producer +
    `/T` denom as the verdict HUD, exceeds-pill actionable → project Optimize, retiring the
    separate ⚡ button); `hotspot_summary_line` helper (one format across card/Top-hotspots/Span,
    INS-004); engagement→percent everywhere + `ENGAGEMENT_PROVENANCE_HOVER` (INS-006).
  - **2/2** (`4b2182e`) scope-tiered disclosure (pass2 inspector §2): fixed verdict+freshness
    status header (INS-001/005) → Project / Toolpath / Span CollapsingHeaders, each summary-first;
    distinct card shapes (filled+◍ hotspot vs hollow+△ issue, INS-003); in-panel span lock toggle
    (INS-008); Generator-item + Generation-trace folded under Span (INS-007); Optimize buttons
    scope-labelled (INS-009). `draw_reactive_inspector`/`draw_project_overview`/
    `draw_selected_section` → `draw_status_header`+`draw_project_section`+`draw_toolpath_section`+
    `draw_span_section`/`draw_span_body`+2 disclosure fns. Nothing deleted; every datum keeps a home.
  ⚠ egui-chrome visual parity unverified — MCP screenshots capture the 3D viewport, not panel
  chrome; needs a human `cargo run -p rs_cam_viz --bin rs_cam_gui` pass over the Inspector panel.
- **2026-06-09** — **W3.6 COMPLETE** on branch `ia-cleanup/w3.6-timeline`, 3 commits each green
  (clippy --workspace -D warnings + fmt + viz 189+9), merged to master `0ea4a5f` `--no-ff`:
  - **1/3** (`f8013df`) clickable verdict pills (TIM-005 — exceeds/collisions seek to first
    marker + Safety tab); spine empty-state placeholder + Enable-&-rerun (TIM-003); whole-spine
    stale skin — desaturate + drop gate dots + Re-run affordance (TIM-009); gate-dot drill →
    Safety tab focus (TIM-010); cursor-change disclosure (TIM-004).
  - **2/3** (`2190dec`) Tier-2 single-axis merge (TIM-001/002): `draw_boundary_timeline` now
    allocates ONE rect with op band + span sub-band + (debug) semantic sub-band as Y-regions,
    one shared global-X playhead, one Y-dispatched click contract (drag=scrub everywhere).
    `draw_span_ribbon`→`span_subband_present`+`paint_span_subband`; `draw_semantic_band`→
    `semantic_subband_present`+`paint_semantic_subband` (converted local-X → global-X). Two
    divergent axes + two orphan playheads gone.
  - **3/3** (`d2a39b4`) op-list (sim_op_list.rs): name label is the explicit jump target
    (TIM-006, retires the whole-card overlapping-Id click); outline kind banner (TIM-007).
  Deferred w/ rationale: TIM-008 metric-chip row; `components::{visibility,nav,diagram}` (build
  with consuming surfaces — visibility→W3.7). ⚠ Painting reorg — needs a human `cargo run` pass.
- **2026-06-09** — **W3.5 COMPLETE** on branch `ia-cleanup/w3.5-optimizer`, 3 commits each green
  (clippy --workspace -D warnings + fmt + viz ui tests), merged to master `c2505d3` `--no-ff`:
  - **1/3** (`ddb6b60`) `optimize_project.rs` role-sectioned rollup (OPT-002/OPT-004): Apply-now
    (Ranked+safe, checkbox-only col, glyph-only verdict) / Needs-your-call (TradeOff/MarginalSafe,
    role chip + `Review ▸` → `OpenOptimizeModal`) / collapsed Not-optimized (per-row expander, no
    70-char inline truncation). Header `+N not estimated` note + `baseline:` provenance line
    (OPT-001 presentation); Reconciling/Reconciled prose → `xtp` cross-toolpath cell note + hover.
  - **2/3** (`811e761`) `optimize_modal.rs` Apply-&-re-optimize suggestions (OPT-005): `CapAxisAt`/
    `RaiseAxisAbove` render as button rows → new `AppEvent::ReoptimizeWithAxisOverride` (controller
    sets the axis via the op setter, stamps Optimizer provenance, re-opens the modal); `DataGapHere`
    stays a ⓘ note. Lifted `axis_label_units`; added `format_suggestion_action`; retired prose
    `format_suggestion`.
  - **3/3** (`72c4169`) discoverability (OPT-006): `menu_bar` `on_disabled_hover_text`; inspector
    Project section gains an always-available `⚡ Optimize project for cycle time` entry when
    `bad == 0` (the W3.3 actionable exceeds-pill remains the entry when `bad > 0` — no redundant
    button re-added for that case).
  OPT-001 (header math, W0.2) + OPT-003 (stale banner, W0.5) were already done. `compare::*` not
  adopted — didn't cleanly fit (header is absolute+percent, not a ratio; `CompareRow` is feeds-only).
- **2026-06-09** — **W3.5 visual parity CONFIRMED** by the user (live GUI pass: role-sectioned
  rollup, Review buttons, modal Apply-&-re-optimize rows, inspector cycle-time entry, disabled
  menu-hover all good). W3.5 egui-chrome verification debt cleared.
- **2026-06-09** — **W3.4 PARTIAL** on branch `ia-cleanup/w3.4-tools`, 2 commits each green,
  merged to master `8e503f8` `--no-ff`. (TOO-001/TOO-002 + `project_tree.rs` deletion were already
  done by W1.1, so W3.4's real scope was the editor findings.)
  - **1/2** (`6af118c`) `properties/tool.rs` editor clarity: TOO-005 (tapered Shaft → "Cutter
    geometry" group as "Upper shaft ⌀ (taper top)"; holder relabelled "Shank ⌀ (in collet)");
    TOO-006 (no-holder collision-skip → warning badge on the Holder/Shank CollapsingHeader, italic
    prose removed); TOO-004 metadata half (editable Vendor / Product ID "Catalog metadata" group,
    reaches the modal edit form too via the shared `draw_tool_fields`).
  - **2/2** (`be0eea0`) TOO-004 append half: core `add_or_replace_to`/`add_or_replace_tool`
    (replace by dedupe key, else append; returns `(path, replaced)`); panel "Save to library"
    routes through it ("Updated" vs "Saved"), retiring the silent-append duplicate pile. +1 core test.
  - **DEFERRED — TOO-003** (draft/Apply commit model in the properties panel): a `GuiState` +
    `properties/mod.rs` + `flush_tool_snapshot` refactor touching the undo snapshot + `invalidate_tool`.
    Done with fresh context to avoid a silent edit-drop. ⚠ W3.4 visual parity unverified.
- **▶ Next:** finish W3.4's TOO-003 sliver, then W3.7 header/rail (builds `components::visibility`
  with the viewport overlay), then W3.8 + Tier-4/5. Run **sequentially** (build-thrash), one
  branch+merge per surface, consuming the component layer.
