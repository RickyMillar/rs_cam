# rs_cam_viz IA Audit — Pass 2 Summary

## How pass 2 relates to pass 1

- **Pass 1** audited the **feeds/tabs epicenter** — the toolpath property tabs and
  the feeds-and-speeds surfaces where the audit started, on the hypothesis that the
  worst IA debt clustered there.
- **Pass 2** audits **everything pass 1 skipped**: tool management, the simulation
  Inspector (diagnostic cards), the optimizer / sim-recommendation system, the
  simulation timeline + transport, and the shell / navigation chrome (status bar,
  workspace bar, menus, setup rail, toolpath card panel).

Pass 2 explicitly **stayed clear of the pass-1 exclusions** — feeds surfaces,
toolpath tabs, and badge/verdict-tooltip internals were only *placed* in redesigns,
not redesigned. Where a pass-2 finding touched those (e.g. the pre-tab toolpath
header SHE-004), it was scoped to the non-feeds portion.

## The headline

The skipped surfaces were **not** clean. **No area came back clean** — every one
warranted a redesign spec. Two of the worst defects in the entire audit live
outside the feeds epicenter: a complete tool-CRUD navigation panel that **ships
dead** (delete/duplicate unreachable in the GUI), and an optimizer that reports a
**numerically wrong** headline cycle time on a false in-code premise.

## Counts

Surviving findings after adversarial refutation: **33** (5 high · 19 med · 9 low).

| Area | Findings | High | Med | Low | Verdict |
|------|---------:|-----:|----:|----:|---------|
| Tool management | 6 | 2 | 3 | 1 | Genuinely bad |
| Inspector diagnostic cards | 9 | 2 | 4 | 2 | Genuinely bad |
| Optimizer / sim-recommendation | 6 | 1 | 3 | 2 | Genuinely bad |
| Simulation timeline / transport | 10 | 0* | 6 | 4 | Genuinely bad (partly debug-gated) |
| Shell / navigation chrome | 7 | 1 | 5 | 1 | Genuinely bad |

* Timeline's two original highs (TIM-001/002) softened to med because the divergent
third strip is behind the off-by-default debug toggle.

## Redesign specs in `pass2/redesign/`

All five areas needed a redesign; each has a spec + ASCII mockups:

| Area | Spec | Mockups |
|------|------|---------|
| Tool management | `redesign/tool-management.md` | `redesign/tool-management_mockups.md` |
| Inspector cards | `redesign/inspector-cards.md` | `redesign/inspector-cards_mockups.md` |
| Optimizer | `redesign/optimizer.md` | `redesign/optimizer_mockups.md` |
| Timeline / transport | `redesign/timeline.md` | `redesign/timeline_mockups.md` |
| Shell / navigation | `redesign/shell-nav.md` | `redesign/shell-nav_mockups.md` |

## Where the detail lives

- Full per-finding diagnosis (grouped by area, ranked severity then lens, with
  evidence file:line, user impact, and refute notes for softened findings) plus the
  executive "was this area actually a problem?" verdict: **`DIAGNOSIS_NEGLECTED.md`**.

## Cross-area themes worth fixing once

1. **Freshness is systemically unwired** — INS-005, OPT-003, TIM-009 each present
   concrete numbers without consulting the existing `is_stale` signal.
2. **Dead `pub` modules dodge the dead-code lint** — SHE-001/TOO-001
   (`project_tree.rs`), plus the dead gate-trip drill (TIM-010).
3. **Prose stands in for affordances** — TOO-006, OPT-005, TIM-005, SHE-005.
4. **No summary-first tier in the densest panels** — INS-001, TIM-008, with
   SHE-004/SHE-007 the cohesion analogue.
