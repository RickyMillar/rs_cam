# Fix 5 — Existing-project default re-derivation policy

**Date:** 2026-05-19
**Status:** Decision recommended; no code change in this batch.

## The problem

Roadmaps B.1 / B.2 / B.3 / B.4 / B.5 / B.6 / B.7 / F.5 have each added some form of "smarter default" to fresh-toolpath creation:

- B.1: `drop_cutter.min_z` defaults to stock bottom (was −50.0)
- B.2: `face.depth` defaults to `max(stock_padding, 1.0)` (was 0)
- B.3: pocket / profile / drill / adaptive depths default off `stock_z`
- B.4: `mcp_add_toolpath` now runs the LUT calculator on creation
- B.5: dressup `entry_style` defaults vary by op + role
- B.6: dressup `link_moves` / `feed_optimization` / `optimize_rapid_order` default to `true`
- B.7: 3D ops on mesh models auto-enable model-silhouette boundary
- F.5: LUT calculator runs at toolpath creation, seeding feed/plunge/stepover/DOC
- Fix 1 (this batch): wood + flat adaptive WOC tracks `machine.adaptive_woc_factor`
- Fix 2 (this batch): plunge derate by tool tip diameter

**Projects saved before each of these changes carry the pre-change defaults forever.** The Wanaka100 project surfaced four such staleness symptoms in `planning/WANAKA_ASSESSMENT_2026-05-19.md`:

1. `TP7.min_z = -50.0` (pre-B.1)
2. `TP1/TP6 stepover = 0.7 / 0.8` (pre-LUT-on-create AND pre-Fix-1)
3. `TP4/TP5/TP7 plunge = 400/750` (pre-Fix-2)
4. `TP7 stepover = 0.30` (pre-LUT-on-create)

None were catastrophic; one (Fix 2's plunge) was a real safety concern only caught by FSWizard cross-check.

## Option A — "Freshen Defaults" command

A new operation invoked from a GUI menu or via MCP that walks every toolpath in the loaded project and re-derives each default-applicable field as if the toolpath had been freshly created with today's code.

**Pros:**
- Existing projects pick up safety improvements (Fix 2 plunge cap) without manual intervention
- Discoverable — a user opening an older project sees a "defaults are stale" banner with a one-click freshen action
- Auditable — output is a diff log of what changed

**Cons:**
- **Destroys user customisation.** A user who deliberately set `stepover = 0.7` for some material-specific reason loses that without warning unless the diff is reviewed.
- Requires per-field "is this the static default?" detection — comparing current value to default-as-saved is unreliable because user may have hand-typed the default value.
- Doesn't compose with future B-roadmap entries cleanly — each new default needs a "freshen" hook.

## Option B — Accept-as-saved (status quo)

Loaded projects keep their saved values. New projects get current defaults.

**Pros:**
- Predictable — saved files are authoritative.
- No ambiguity about user intent. If TP7 has `stepover = 0.30`, it stays 0.30 forever unless the user changes it.
- Doesn't require ongoing maintenance per B-roadmap entry.

**Cons:**
- Safety improvements (e.g. Fix 2 plunge cap) don't auto-apply to existing projects.
- Users may not realise their saved project is missing a default-improvement that ships in the new release.

## Option C — Per-field validation warnings (recommended)

Don't re-derive on load. Don't ship a "freshen" command. **Instead, when a project loads, run each toolpath through a `validate_defaults()` check that flags specific known-stale patterns as warnings** — the same surface as the existing load-warning panel that B.1's stock-bottom defaults already use.

For each B-roadmap entry, the validator gets one rule:

```rust
// Example rule format
ValidationRule {
    id: "drop_cutter_min_z_pre_b1",
    title: "Drop-cutter min_z is at the pre-2026-04 default (-50.0)",
    detail: "Roadmap B.1 makes min_z default to stock_bottom_z. \
             This project predates that change. Harmless if the model \
             doesn't extend below your current min_z, but worth tightening \
             to the stock bottom for clarity.",
    detect: |tp, ctx| matches!(&tp.operation, OperationConfig::DropCutter(c) if c.min_z <= -49.999),
    auto_fix: |tp, ctx| if let OperationConfig::DropCutter(c) = &mut tp.operation { c.min_z = ctx.stock_bottom_z; },
}
```

The user sees a load-time warning panel:
> ⚠️ 3 settings on this project may be stale: TP7 `min_z` (B.1), TP4/TP5/TP7 `plunge_rate` (Fix 2). [Review] [Apply all] [Ignore]

**Pros:**
- Visibility without surprise — user sees what would change before any change happens
- Per-rule auto-fix means the user can accept some, leave others
- Composes cleanly with future B-roadmap entries: one rule per entry
- Mirrors the existing B.1 stock-bottom warning surface
- Doesn't require "is this still the static default?" guessing — rules can be specific (e.g. `min_z <= -49.999` rather than `min_z == default()`)

**Cons:**
- More code to write than option A or B
- Rule library has to be maintained as B-roadmap entries land

## Recommendation

**Adopt Option C.** Build a `compute::validate::stale_defaults(&ProjectSession) -> Vec<StaleDefault>` returning rule matches, with `auto_fix` closures. Surface in the load-warning panel.

Start with three rules covering the Wanaka findings:
1. `drop_cutter_min_z_pre_b1` — `min_z <= -49.999` (very specific value, low false-positive rate)
2. `tapered_ball_plunge_pre_fix2` — for `tool_geometry ∈ {Ball, TaperedBall}` with `tip_d < 2`, flag `plunge_rate > 150 * tip_d`
3. `wood_adaptive_stepover_pre_fix1` — for wood-class material on flat tool, flag adaptive stepover < `0.15 × tool.diameter` (below the post-Fix-1 floor)

Each rule has a one-line auto-fix that re-derives the post-improvement default.

## Out of scope for this batch

Implementation of Option C is a separate piece of work — roughly 2-4 hours of code + tests for the validator + UI surface. The decision is recorded here; the implementation should be tracked as its own task.

## Sequence

1. ✅ Fix 1 + Fix 2 land (code-default improvements for new toolpaths) — **this batch**
2. ✅ Fix 4 helix-on-agent_search fix lands (code correctness, not a default) — **this batch**
3. ⏳ Option C validator: 3 starter rules for Wanaka findings — **next batch**
4. ⏳ Add one rule per future B-roadmap entry as part of the entry's PR — **ongoing convention**
