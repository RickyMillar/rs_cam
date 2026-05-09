# Agent prompt — G13 deflection model rewrite

You're picking up **G13** from `planning/cutting-calcs-data-gaps.md`. Read
that gap entry first, then the rest of this prompt. The full doc is your
tracker; update G13's `Status` field as you move through the lifecycle.

## Why G13 matters now

The optimizer's deflection gate currently fires on a geometric `L/D > 6`
ratio, regardless of material or carbide modulus or actual cutting force.
On wanaka (verified 2026-05-08 after the operator's stickout change), every
End Mill TP now reports `Exceeds(LongToolStiffnessUnsafe)` at peak 7.5
(45 mm stickout / 6 mm flat), and TaperedBall TPs report
`Within(Approximate)` at peak 5.83 (35 mm stickout / 6 mm shank). These
are wood cuts. The geometric heuristic is steel-shop conservative and
prevents the operator from digging deeper than 6 mm of stickout per
diameter — wrong for wood + carbide.

The gap doc's design (G13 entry, around line ~330 of the file) calls out
a force-aware tip-deflection model:

```
δ = F · L³ / (3 · E · I)
```

where `F` is the peak transverse cutting force per sample, `E` is the
tool-material modulus (carbide ≈ 600 GPa, HSS ≈ 200 GPa, comes from
`ToolConfig.tool_material`), and `I = π · d⁴ / 64` from the engaged
diameter at commanded DOC. Verdict thresholds in microns of tip
deflection (Within / Approximate / Exceeds), calibrated against real
cuts. The operator wants this — it's the largest single piece of gap
work.

## What just landed (your starting context)

A recent burst of work (May 2026) closed several gaps end-to-end:

- **G5 + G6 + G7** (`d09001e`) — engaged-edge LUT lookup with
  diameter + hardness chipload scaling. Verdict carries
  `Confidence::Approximate(detail)` past ±40 % divergence with the
  scaling factors in the detail string.
- **G1** (`11e0f9f`) — Profile + Zigzag added to `has_doc_knob`;
  Stage 1 collapses the stepover dim when an op lacks the knob;
  bipolar prescription reorders so Contour/Trace pick the
  geometry-driven lever.
- **G2** (`c40795b`) — `scallop_height` axis added to Stage 1; gate
  widened from "has DOC knob" to "has any sweep knob". Live-validated
  against wanaka TP 7 (`1d26d95`).
- **G3** (`2926a15`) — Trace, RampFinish, Waterline get DOC axis;
  Pencil gets conditional stepover; RadialFinish split into G3a.
- **G14** — *(check status when you start; may be in flight or
  recently merged)*. Audit of engaged-diameter consistency across all
  gate paths. Whatever G14 lands, deflection's diameter usage will be
  consistent with the rest by the time you start.

The doc has been actively rewritten in place when audits proved the
original framing wrong (G5/G6/G7, G2, G3 all got rewritten). Don't trust
the audit blindly; re-verify before designing the fix.

## Working environment

- `/home/ricky/personal_repos/rs_cam` is the operator's working tree
  and the GUI's source of truth. Commit on master directly when fixes
  land — recent G1/G2/G3/G14 work has been merging straight to master
  rather than via worktree. The operator may have unrelated
  in-progress changes; don't touch files outside your scope.
- The MCP server (`rs-cam`) gives you live verdicts against the wanaka
  project. Tools are deferred — load with
  `ToolSearch select:mcp__rs-cam__optimize_toolpath` etc. Run
  `mcp__rs-cam__get_tool_load_report` to see every TP's deflection
  verdict; `mcp__rs-cam__optimize_toolpath` for end-to-end behaviour.
- The deflection model lives in
  `crates/rs_cam_core/src/tool_load/deflection.rs`. Pre-flight refusal
  hook is `DeflectionSetupLocked` in `tool_load/mod.rs` and
  `tool_load/optimize.rs::deflection_setup_prescription`.

## Workflow (per AGENT_PROMPT.md)

This is a **multi-day model rewrite, not a quick patch**. The earlier
agent prompt explicitly says don't start G13 with a vague plan. So:

1. **Re-verify the root cause.** Read `deflection.rs` end-to-end. The
   doc-comment says "conservative steel-shop heuristic". Look at what
   inputs the gate already has access to (it should have access to
   sim trace, tool config, material). Confirm the existing geometric
   check is the only logic.
2. **Find good references** before designing the fix:
   - The G13 gap entry lists physics references inline (formula,
     modulus values, threshold direction).
   - `research/feeds_and_speeds_integration_plan.md` may have RCTF
     and chipload calibration details that constrain the force input.
   - `crates/rs_cam_core/data/vendor_lut/observations/*.json` rows
     have `chipload_max_mm_tooth` per material — peak chip per tooth
     × engaged width gives a force estimate via material `Kc`.
   - For `Kc`: see `crates/rs_cam_core/src/material.rs` and how
     `tool_load::power::evaluate` already computes mrr-derived power.
     Force estimate = mrr × Kc / engaged_width per sample.
3. **Plan the fix in the gap entry under a `**Plan:**` sub-heading.**
   This is for the operator to review before you sink time into it.
   Flag any change that touches more than one crate, changes a public
   API surface, adds a dependency, or could affect non-deflection
   paths. *G13 specifically requires operator approval before
   implementation begins* — write the plan, then stop and ask before
   implementing.
4. **Calibration is the load-bearing part of this work.** The
   tip-deflection formula is two lines; the threshold values that
   decide Within / Approximate / Exceeds need to be defended against
   real cuts. Capture calibration data inline in the gap entry — what
   wanaka cuts produced what tip-deflection prediction at what
   threshold.
5. **Implement.** Stay inside `tool_load/`. The pre-flight refusal
   keeps using `DeflectionSetupLocked` for the new model's `Exceeds`
   outcome; the geometric L/D check disappears.
6. **Validate against the gap's gate.** The gap doc's validation
   bullets name specific scenarios:
   - Wanaka TP 1 (6 mm carbide flat, ~45 mm stickout) in hardwood:
     post-fix should produce `Within (Approximate)` — finish degraded
     but tool not at risk. Optimizer should reach Stage F retarget
     instead of `DeflectionSetupLocked`.
   - A 1 mm carbide engraver at 25 mm stickout in hardwood at low
     feed should still pass.
   - 6 mm HSS flat at 60 mm stickout in steel (we don't cut steel
     but the model shouldn't be wood-only) should fail.
   Run the live MCP gate; capture before/after in the commit.
7. **Update the doc.** Flip Status to Done with the calibration
   numbers and live verdict diff inline.
8. **Commit and report back.** Title `fix(optimizer): G13 force-aware
   tip-deflection model`. Body describes the formula inputs, the
   threshold calibration, and the validation outcome. Standard
   `Co-Authored-By` trailer.

## Validation infrastructure

- `mcp__rs-cam__get_tool_load_report` — current deflection verdict per
  TP. Pre-fix you'll see `Exceeds(LongToolStiffnessUnsafe)` peak 7.5
  on wanaka End Mill TPs.
- `mcp__rs-cam__optimize_toolpath` — full pipeline. The
  `OptimizeOutcome::DeflectionSetupLocked` refusal will move from
  every End-Mill TP to only the ones that genuinely fail the new
  force model.
- Param-sweep system can build fixtures (`cargo run -p rs_cam_cli --
  sweep`); useful for reproducing the calibration-point cases without
  needing wanaka.

## Hard rules

- **Don't start without operator approval on the plan.** Write the
  plan into the gap entry and stop.
- **Don't rename `OptimizeOutcome` variants.** UI/MCP consumers branch
  on the tag string.
- **Don't bundle G13 with other gaps.** It's its own commit (or tight
  set if calibration constants need their own commit).
- **Don't skip clippy.** Workspace enforces 16 deny lints.
- **Don't break wanaka regressions on TPs 0/2 (drills) and TPs 4/5/7
  (TaperedBall chipload Approximate verdicts must stay Approximate).**

## Soft rules

- Prefer extending existing helpers in `tool_load/` over creating new
  ones.
- For each calibration point, write a unit test pinning the new
  behaviour in `crates/rs_cam_core` *before* exercising via MCP.
- The thresholds are the part that needs to be defended. Capture
  *why* you picked each threshold inline in the gap entry — the
  operator may push back on specific values.

## Suggested first move

1. Read `planning/cutting-calcs-data-gaps.md` G13 entry end-to-end.
2. Read `crates/rs_cam_core/src/tool_load/deflection.rs` end-to-end.
3. Read how `tool_load::power::evaluate` already derives force-like
   quantities from sim samples (mrr × Kc, engaged width). The
   transverse-force input for the deflection formula is the same shape.
4. Capture wanaka's current verdict baseline:
   `mcp__rs-cam__get_tool_load_report` — record the
   `peak: 7.5 / 5.83` numbers and the corresponding tools/stickouts so
   you can compute the predicted tip deflection by hand for each TP
   and pick threshold values that classify them sensibly.
5. **Write the plan into G13's entry. Stop. Ask the operator to
   review.**
