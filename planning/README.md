# Planning

This directory contains active planning notes, status snapshots, and archived historical plans.

## Active docs

| File | Purpose |
|------|---------|
| [`PROGRESS.md`](PROGRESS.md) | Current project snapshot and verification status |
| [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) | Prioritized near-term engineering backlog |
| [`Performance_review.md`](Performance_review.md) | Active performance backlog and benchmark gaps |
| [`FUTURE_PLANS.md`](FUTURE_PLANS.md) | Deferred product and engineering follow-ons, including benchmark mode |
| [`WORKSPACE_UX_REDESIGN_PLAN.md`](WORKSPACE_UX_REDESIGN_PLAN.md) | Detailed handoff plan for the setup/toolpaths/simulation workspace redesign |
| [`SIMULATION_WORKSPACE_VISION.md`](SIMULATION_WORKSPACE_VISION.md) | UX vision for dedicated verification environment |
| [`WORKFLOW_TEST_PLAN.md`](WORKFLOW_TEST_PLAN.md) | End-to-end workflow test strategy and coverage map |
| [`VOXEL_SIM_DESIGN.md`](VOXEL_SIM_DESIGN.md) | Tri-dexel simulation algorithm design (implemented) |
| [`MULTI_SETUP_FEASIBILITY.md`](MULTI_SETUP_FEASIBILITY.md) | Multi-setup design research and feasibility |
| [`MULTI_SETUP_UX_PLAN.md`](MULTI_SETUP_UX_PLAN.md) | Multi-setup UX execution plan (Phases A-D) |
| [`ALIGNMENT_PINS_DESIGN.md`](ALIGNMENT_PINS_DESIGN.md) | Stock-level alignment pin design for two-sided machining |
| [`TOOL_LIBRARY_DESIGN.md`](TOOL_LIBRARY_DESIGN.md) | Persistent tool library architecture (deferred) |

## UX Fixes

Phased UX improvement plans in [`ux-fixes/`](ux-fixes/):

| File | Scope |
|------|-------|
| `phase1-critical-and-quick-wins.md` | Focus loss, quit protection, toast notifications, units |
| `phase2-theme-and-multi-model.md` | Theme system, multi-model support |
| `phase3-simulation-ux.md` | Timeline, staleness, export safety, deviation |
| `phase4-navigation-and-discoverability.md` | Help, shortcuts, tooltips, undo gaps |
| `phase5-polish-pass.md` | 40+ detail fixes across all panels |

## Archive

Historical planning snapshots kept for context in [`archive/`](archive/):

- `FEATURE_GAP_REPORT.md` — Feature comparison vs Fusion 360
- `GUI_WIRING_CATALOG.md` — Historical feature wiring catalog
- `TECH_DEBT_PLAN.md` — Completed tech debt remediation (6 phases)
- `REMEDIATION_TRACKER.md` — Completed 86-task remediation sprint
- `ORCHESTRATION_PLAN.md` — Agent orchestration methodology (completed sprint)
- `CONSOLIDATION_AUDIT.md` — Code consolidation session report
- `VENDOR_LUT_GUI_PLAN.md` — Feeds/speeds UI checklist (mostly complete)
- `OPEN_FIXES_TASKLIST.md` — Carryover tasks from remediation
- `brep_step_implementation.md` — BREP/STEP import plan (shipped)
- `brep_step_followup.md` — BREP/STEP post-implementation audit
- `brep_step_followup_implementation.md` — Face/edge workflow wiring plan

## Guidance

- Product-facing capability docs belong in the repo root (`README.md`, `FEATURE_CATALOG.md`, `CREDITS.md`), not here.
- AI analysis reference: `AI_MACHINIST_ANALYSIS_REFERENCE.md` (root).
- Durable design docs belong in `architecture/`.
- Research notes and source captures belong in `research/`.
