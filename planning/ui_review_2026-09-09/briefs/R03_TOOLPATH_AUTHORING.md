# R03 — Choosing tools and authoring the intended cut

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R03.
Write `../results/R03/{REPORT,trace}.md` and evidence. Source paths are repo-relative.

## Outcome

A user can choose an appropriate operation/tool, target the right geometry,
configure a cut and understand the generated result without knowing enum names
or relying on an agent-authored TOML.

## Task cards

1. **First 2D cut:** F1 zero-op seed. “Clear this pocket, then cut the outline
   while keeping the part held.” Choose a tool and operation via the GUI. Trace
   boundary/inside-outside selection, depth, cut direction, heights and tabs.
   Ask the operator to predict removed and retained material before generating.
2. **First 3D cut:** F3. “Remove excess stock, then obtain this level of detail.”
   Do not prescribe adaptive3d/scallop/drop_cutter. Record how alternatives and
   tool requirements are discovered, what evidence is available before a costly
   generation and which parameters the user thinks they must change.
3. **Tool order and compatibility:** V3, with a flat tool first and a ball tool
   later, then reverse order. Try adding a ball-required operation. Compare the
   selected tool/model with the assigned defaults and the refusal path. A
   backend add with explicit IDs is not a substitute for this test.
4. **Detailed input:** F2, F4/F5 and V2. Exercise one detail/engraving pattern,
   one face-selective cut and drilling selected targets. Include mixed curves
   projected onto a surface where available. Confirm target scope visibly.
5. **Control and recovery:** adjust one geometry parameter, one height and one
   entry/linking option. Predict its effect using the UI, generate, inspect,
   and undo. Include a validation-blocked Generate and a zero/empty result.
6. **Tool reuse:** create/edit a project tool and add one from a scratch library.
   Explain tip/cutting/holder geometry and project copy versus catalog entry.
   Check Apply/Revert and navigation-away behaviour; do not mutate Ricky’s real
   catalog without agreement.

## Specific questions

- Is operation choice expressed in machining goals or only strategy names?
  Are tradeoffs and geometry/tool preconditions shown before committing?
- Can important first-use fields be distinguished from refinements? Which
  advanced features are invisible without already knowing their terminology?
- Is the scope of Tool/Input/face/boundary selection obvious? Are inherited
  boundary and automatic height values understandable and controllable?
- Are geometrical meanings clear: stepover, depth/pass, total depth, leave,
  scallop/cusp, containment, ramp, retract and clearance?
- Can the user tell “ready to generate,” “done,” “stale,” “empty” and “waiting”
  apart? Does the viewport make it possible to check the intended cut?

## Source anchors

- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`: grouped queue, tool controls and
  `add_toolpath_menu` / `add_op_menu_item` at 649–710.
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:12-173`: target setup,
  first-tool/model defaults, Suggest refusal and boundary initialization.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:3571` onward: header, validation,
  target, remaining stock, five tabs, diagnostic fixes and guidance.
- `crates/rs_cam_viz/src/ui/properties/operations/`: operation-specific forms.
- `crates/rs_cam_viz/src/ui/properties/tool.rs`; `src/ui/tool_library_modal.rs`.
- `crates/rs_cam_core/src/compute/catalog.rs`: actual operation inventory.

## Deliverable additions and boundary

Provide a decision tree of what the user needs to choose, not a proposed new
widget layout. Include a shallow all-23-op availability/guidance census, then
identify which distinct interaction patterns were deeply exercised. Keep a
list of expert controls and shortcuts that must survive simplification.

R04 owns detailed recommendation/optimization semantics; R05 owns dependent
chains and planner ownership; R06 owns simulation interpretation. Pass them
concrete authored scratch jobs and unanswered questions, not assumed success.
