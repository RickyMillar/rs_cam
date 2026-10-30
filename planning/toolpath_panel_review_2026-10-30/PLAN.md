# Toolpath panel UI/UX review — plan (2026-10-30)

## Scope

The operator asked for a review of the toolpath side panel (the rail and the
inspector). The review covers two things:

1. The UI: the look, the layout grid, the text blocks, the warnings.
2. The UX: the state of the panel at each step of the process, and the flow to
   set up and refine a toolpath.

This is a review. It does not change `crates/`. The output is findings and
proposals for an operator decision.

## Operator complaints (the input)

- C1: long text blocks for information, warnings and errors (example: the
  reach-map grid-floor note in `state/runtime.rs:1016`).
- C2: count pills such as "3/8 dressups active".
- C3: the large errors at the top of the inspector.
- C4: the "6 hints" block.
- C5: with no tools, the panel has no "Add tool" action.
- C6: the feeds have no defaults, but the calculator can suggest values. The
  suggestion must be the default.
- C7: "Show reach map" is in the toolpath panel and in the viewport dock. The
  main use is the viewport.
- C8: the picked-face count is hard to read.
- C9: the layout grid is not the same from row to row and from tab to tab.
- C10: the tabs differ per op, and the Geometry tab sometimes has no useful
  control.
- C11 (question): does egui limit the result? Would an HTML UI be better?

## Method

1. Owners. Find the source line of each quoted string (`rg`). Each finding
   cites its owner file.
2. Capture matrix. Use one GUI and one window size for every shot.
   - Empty project: the no-tool state.
   - STEP solid (`planning/adaptive_research_2026-10-23/play/block_with_blind_hole.step`):
     Pocket, Profile, Drill, Face. Each op first with no tool, then with a tool.
     Every tab of each op.
   - Mesh (`planning/fixtures/rivmap100/rivmap100.toml`): Adaptive 3D,
     Waterline, Scallop or Pencil, a finishing op with the reach map on.
     Every tab, before and after generation.
3. Journey track (UX). Record one toolpath from "Add" to "simulated", one shot
   per step: add → no tool → tool set → geometry picked → feeds → generate →
   stale after an edit → regenerate → error state. Note at each step: what
   the panel asks for next, and where the operator must look.
4. Inventory. One row per visible block: location, text length, severity
   colour, actionable or not, rule it breaks (IC1/IC2 in
   `ui/properties/CLAUDE.md`), owner file.
5. Sentry check. Read what `the_inspector_text_is_bounded_ic1` and
   `tabular_data_reads_one_table_ic2` assert. If they pass while the shots
   show text walls, the sentry is weak; that is a finding.
6. Prior work. Read the open items of the UI review 2026-09-14, the design
   audit 2026-09-17 and the premium-feel note (egui stays, assessed
   2026-09-13). Separate "already ruled" from "new".
7. Research. Fusion 360 CAM op dialog (Tool / Geometry / Heights / Passes /
   Linking), Kiri:Moto, FreeCAD Path: validation placement, empty states,
   defaults, derived-value disclosure.
8. Proposals. Tag each one:
   - A: possible in egui with the existing `ui/components/`.
   - B: needs a new egui component.
   - C: limited by the toolkit.
   The tag counts answer C11.
9. Deliverable. An interactive HTML review page (Artifact) with the shots, the
   inventory, the journey, and before/after mockups. A copy of the findings
   stays in this folder.

## Conditions

- The GUI binary is from 10:23. HEAD (4d2639487) has one later viz commit,
  ce32777f3 (reach legend text). A peer session has uncommitted changes in
  `crates/rs_cam_core/src/maps/reach_map.rs`, so this review does not rebuild.
  The reach-map legend in the shots is the pre-R7c text.
- `ui/CLAUDE.md` says egui 0.36. That is the current version.
