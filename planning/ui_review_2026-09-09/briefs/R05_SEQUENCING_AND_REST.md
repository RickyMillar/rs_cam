# R05 — Sequencing, remaining stock and multi-tool planning

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R05.
Write `../results/R05/{REPORT,trace}.md` and evidence. Source paths are repo-relative.

## Outcome

The operator can assemble, understand and revise a dependent cutting plan,
including what each tool will machine and what must be recomputed after changes.
Powerful planning should reduce work without hiding ownership or prerequisites.

## Task cards

1. **Manual chain:** V8 from a small F3 job. “Use a larger tool for bulk work and
   a smaller tool only where useful.” Find the intended remaining-stock/rest
   controls without being given enum or parameter names. Ask what geometry and
   material state the next op will receive.
2. **Generate the chain:** start with automatic simulation resolution. Use the
   normal Generate All control; observe the prerequisite/refusal and recover via
   the GUI. Then run with a deliberately pinned cell and follow intermediate
   generation/simulation/waiting states. A single MCP call supplying the cell
   does not test this handoff.
3. **Break and repair the order:** duplicate, drag/reorder, disable a predecessor
   and move an op to another setup. Ask which descendants are still meaningful.
   Observe scope, warnings, re-generation and what happens to prior evidence.
4. **Planner proposal:** with at least two appropriate tools, find Plan multi-tool
   finishing, select a ladder, preview regions, change coarseness/overlap and
   inspect advanced controls. Cancel once before applying. Ask what will be
   added or replaced before Apply plan; then compare proposal with resulting ops.
5. **Manual work after planning:** edit a generated tier, add a hand-authored op,
   then revise/reapply the plan. Inspect ownership, retained overrides, operation
   naming and undo. Do not assume replacement behaviour from a preview screenshot.
6. **Real-world density:** inspect a scratch F9 copy with its recorded enabled
   set, then an explicitly approved expanded set if needed. Follow pin drills,
   back/front operations and mixed-model curves. Locate the next actionable
   bottleneck without reading raw IDs or loading the whole job into an agent.

## Specific questions

- Are “rest machining,” “use remaining stock,” “rest analysis,” “derived rest
  regions,” “tier map” and “reach map” distinguishable by the problem they solve?
- Can the user see predecessor/consumer scope and why an op is waiting?
- Is simulation resolution a sensible, discoverable prerequisite at the place
  generation is attempted? Can the user recover without memorizing a message?
- Can a hidden op still be enabled/exported? Does row ordering visibly change
  material assumptions? Are grouping, selection and active setup aligned?
- Is the planner preview a comprehensible proposal with a reversible apply,
  rather than a collection of attractive regions and unexplained numbers?
- Are fine-tool time/quality benefits supported by actual evidence rather than
  a territory percentage being mistaken for achieved machining quality?

## Source anchors

- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`: grouping, drag/drop, dependencies.
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:236-413`: ordering and
  pinned-resolution Generate All path; `src/controller/generate_all.rs`.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:3944` onward: remaining-stock and
  boundary/rest consumers; `src/ui/properties/operations/` for pencil/rest forms.
- `crates/rs_cam_viz/src/ui/{menu_bar,multitool_planner}.rs` and
  `src/controller/events/planner.rs`: entry, preview, apply and reconciliation.
- `crates/rs_cam_core/src/session/multitool.rs`.
- `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs`.

## Deliverable additions and boundary

Draw the operator-visible dependency graph and a table of manual/planner-owned
fields, actions and replacement semantics. Compare manual versus planner paths
for the same goal; neither is automatically the winner. Capture a broken chain
and its full recovery. Include expert shortcuts to preserve.

R04 owns feed optimization; R06 owns numerical diagnosis; R08 owns general
background/undo behaviour. Send them dependency-specific evidence, not duplicate
redesigns. Report generated-path correctness defects separately from UX findings.
