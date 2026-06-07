# Pass-2 Redesign — Optimizer / sim-recommendation system

Area scope: the project-level Optimize rollup (`optimize-project`), the
per-toolpath Optimize modal (`optimize-modal`), and the two entry points
into them (`sim-diagnostics-optimize-entry`, the Toolpath menu item).

Source surfaces:
- `crates/rs_cam_viz/src/ui/optimize_project.rs`
- `crates/rs_cam_viz/src/ui/optimize_modal.rs` (suggestions block only — OPT-005)
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` (entry buttons)
- `crates/rs_cam_viz/src/ui/menu_bar.rs` (menu entry)
- core: `crates/rs_cam_core/src/tool_load/optimize/outcome.rs`

OUT OF SCOPE (pass-1): per-toolpath property tabs, all feeds/speeds editors,
fixture-collision fix, Verdict-HUD reconciliation. None of the changes below
restructure tabs or consolidate feeds — they are intrinsic to the optimizer's
own correctness, affordances, grouping, and provenance.

---

## Findings addressed

| id | sev | lens | one-line |
|----|-----|------|----------|
| OPT-001 | high | S7 | headline Optimized time/savings% is wrong when any row is refused/skipped |
| OPT-002 | med | S6 | TradeOff/MarginalSafe rows tell you nothing and have no way to act |
| OPT-003 | med | S7 | optimizer can run off a stale sim with no freshness cue on any surface |
| OPT-004 | med | S3 | selection column + verdict column conflate 3 distinct row states under one blank header |
| OPT-005 | low | S5 | modal "Try this" suggestions are non-actionable prose despite the modal owning re-optimize |
| OPT-006 | low | S6 | project optimizer is reachable only via a non-obvious menu item with no disabled-reason tooltip |

---

## Target structure

### Principle map

- **One concern / one home.** The rollup answers exactly one question: "what
  will applying these recommendations do to project cycle time, and which rows
  can I act on right now?" Every cell must serve that. Today col-1 and the
  verdict column each carry three unrelated meanings.
- **Summary first, drill behind disclosure.** Header = one honest number with a
  legible baseline source. Each refused/trade-off row collapses its narrative
  to a glyph + one phrase; the full explanation and candidate list live behind
  a per-row expander, not inline truncated-to-70-chars text.
- **Distinct roles look distinct.** Actionable rows, refused rows, and
  needs-review rows get visually separate treatment — not three states crammed
  into one unlabeled checkbox column.
- **Provenance/freshness legible.** A baseline-source chip on the header is
  non-negotiable for a surface that makes authoritative cycle-time claims.
- **UI wins, not prose.** The two large muted paragraphs in Reconciling/
  Reconciled states and the modal's prose suggestions become affordances.

### 1. Honest header + baseline provenance (OPT-001, OPT-003)

The header is the contract of the whole surface, so it must be arithmetically
correct and self-describing.

**Correctness (OPT-001).** `compute_optimized_cycle()` must stop substituting
`0.0` for refused rows. The baseline cycle for every non-`Skipped` outcome is
`outcome.candidates.first().cycle_time_s` (documented at
`outcome.rs:77,82-83`; the rollup already reads `candidates` at
`optimize_project.rs:315`). The contribution rule becomes:

| kind | contributes to Optimized total |
|------|--------------------------------|
| Ranked, selected + has safe | recommended candidate cycle |
| Ranked, unselected / no safe | baseline (candidates[0]) |
| TradeOff / MarginalSafe | baseline (candidates[0]) — never auto-applied |
| NoSafeImprovement | **baseline (candidates[0])** — was wrongly 0.0 |
| Skipped | baseline from a new `Skipped`-carried field, else excluded from BOTH baseline and optimized so the delta is computed over a like-for-like denominator |

Skipped rows genuinely lack a candidate baseline. Rather than zeroing them
(which deflates Optimized) or counting them only in the denominator (which
inflates savings%), they are excluded symmetrically from both `baseline` and
`optimized` sums used for the badge, and surfaced as a separate "+N not
estimated" note on the header so the headline never silently drops time.

**Provenance (OPT-003).** A baseline-source chip sits on the header, fed by the
same staleness signal sim-diagnostics already computes (`sim.is_stale(edit_counter)`
at `sim_diagnostics.rs:819`). Two states:
- fresh: `baseline: sim @ <timestamp/run-id>` in muted text.
- stale: amber `⚠ baseline from a stale sim — params changed since this run`
  with a `Re-sim & reopen` button that triggers a fresh project sim and
  reopens the rollup against it.

The gate stays `has_results()` (don't block on stale — let the user see it and
decide) but the staleness is now *legible* on the optimizer surface itself,
which the audit found it never was. The same chip is added to the per-toolpath
modal header (OPT-003 lists `optimize-modal` as an affected surface).

### 2. Three distinct row roles, one labeled column each (OPT-004, OPT-002)

Retire the blank col-1 header and the overloaded verdict column. The grid is
re-grouped into **two sections by what the operator can do**, each with real
headers, so selectable / not-applicable / needs-review never share a cell:

**Section A — "Apply now" (Ranked rows with a safe candidate).**
Columns: `[✓] | toolpath | change | −cycle | verdict`.
- col-1 header is now `Apply` and only ever holds a live checkbox (rows without
  a safe candidate are not in this section, so the disabled-checkbox state that
  caused OPT-004 disappears entirely).
- verdict column holds only the ✓/⚠ gate glyph — consistent meaning.

**Section B — "Needs your call" (TradeOff, MarginalSafe).**
Columns: `[ role-chip ] | toolpath | candidate count | ▸`.
- role chip is a distinct, non-checkbox affordance: `trade-off` (amber) or
  `verify on scrap` (amber). No fake/blank checkbox.
- the `▸` is a real **Review** disclosure that opens the per-toolpath modal for
  that row — closing OPT-002's dead-end. This adds the missing
  `OpenOptimizeModal(ToolpathId)` emission from the rollup (the event already
  exists, `mod.rs:229`; only sim_diagnostics emits it today). The code comment
  that said "user must open the modal" becomes a button that does exactly that.

**Section C — "Not optimized" (NoSafeImprovement, Skipped), collapsed.**
A single collapsed disclosure header `Not optimized (N)`; expanding lists each
with `toolpath | reason-glyph | one-phrase reason | ▸ details`. The 70-char
inline truncation is removed — the full narrative + "tried N candidates" lives
in the per-row `▸ details` expander (dig deeper), not jammed into a striped
cell.

This collapses three confusable inline states into three *labeled, role-named*
groups, each with one consistent affordance. The unlabeled column is gone.

### 3. Reconciling / Reconciled prose → inline cues (UI wins)

The two muted explanatory paragraphs (`optimize_project.rs:449-456`,
`487-495`) are deleted. Their content becomes affordances:
- Reconciling: the existing spinner + a one-line status; the dimmed table
  already conveys "in flight".
- Reconciled: the `reconciled` column already renders a `(+Δs)` delta in amber
  on mismatch. That amber cell *is* the "cross-toolpath interaction" signal —
  add a one-word column note `xtp` on mismatched cells and drop the paragraph.

### 4. Modal suggestions become actionable (OPT-005)

`draw_suggestions` (`optimize_modal.rs:570-585`) currently prints
machine-derived ceilings/floors as prose the user must re-type. Since
`OperatorSuggestion::CapAxisAt { axis, ceiling }` /
`RaiseAxisAbove { axis, floor }` carry a concrete axis + value and the modal
already owns the re-optimize action, each suggestion renders as a row with an
**`Apply & re-optimize`** button that sets that axis bound and re-runs the
search. `DataGapHere` (no actionable value) stays as a plain note. The "we never
auto-apply a heuristic" rule is preserved — the button is an explicit operator
click, not an auto-apply; it just removes the manual re-typing step.

### 5. Discoverability + disabled affordance (OPT-006)

- The menu item `Optimize project…` (`menu_bar.rs:156-162`) keeps its
  `add_enabled(optimize_enabled, …)` but gains `.on_disabled_hover_text(
  "Run a simulation first — the optimizer needs a baseline cut trace.")` so the
  greyed state explains itself.
- The in-context entry in sim diagnostics is currently gated on `if bad > 0`
  (`sim_diagnostics.rs:658`), so it vanishes when nothing is over-budget — yet
  the optimizer can still *save cycle time* on within-gate toolpaths. Make the
  entry **always present when a sim exists**, with the label adapting:
  - `bad > 0`: `⚡ Optimize {bad} exceeding toolpath(s)` (unchanged).
  - `bad == 0`: `⚡ Optimize project for cycle time` (muted/secondary styling).
  This gives a discoverable in-context entry regardless of gate state, so the
  obscure menu item is no longer the only always-available path.

---

## What is retired

- `compute_optimized_cycle()`'s `0.0` substitution for NoSafeImprovement/Skipped
  (OPT-001) — replaced by the candidates[0] baseline + symmetric-exclusion rule.
- The blank col-1 grid header and the disabled-checkbox row state (OPT-004) —
  replaced by role-sectioned tables.
- Inline 70-char `truncate(explanation, 70)` narrative in refused rows
  (`optimize_project.rs:322`, `:613`) — moved behind per-row `▸ details`.
- The two muted explanatory paragraphs in Reconciling/Reconciled (UI-wins).
- The code comments asserting "user must open the modal" with no affordance
  (`:207-209,:347,:363-367`) — replaced by an actual Review button (OPT-002).
- The prose-only "Try this" suggestion list (OPT-005) — each becomes an
  Apply & re-optimize row.

## Cross-references

- Mockups: `optimizer_mockups.md`.
- Baseline-source / staleness signal source of truth:
  `sim_diagnostics.rs:819` (`sim.is_stale(gui.edit_counter)`).
- Outcome field semantics: `outcome.rs:72-78` (the kind/candidates/reason table).
