# R01 — Starting a job and understanding its models

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R01.
Write `../results/R01/{REPORT,trace}.md` and evidence. Paths in that instruction
are relative to this brief; source paths below are repository-relative.

## Outcome

A user can begin a job, import supported geometry, establish its intended size,
recognize what is loaded and choose the next appropriate step. When the model
is not suitable, the app explains the limitation without pretending success.

## Task cards — give goals, not menu instructions

1. **Blank start:** “Use this drawing to make a pocket and outside profile in a
   board.” Start with F0 and raw F1 SVG, not the prepared project. Observe the
   initial workspace, help, import discovery, native dialog and feedback. Ask
   what the user believes they must do next and whether verification is part
   of their expected route.
2. **Small surface:** repeat intake with raw F3 STL. Ask the user to identify
   size, up direction, origin and what surface can be reached from above.
3. **STEP:** import F4 and select the intended planar face. Try a stepped or
   unsupported face from F5. Determine whether the app communicates the actual
   usable geometry/selection, not only a highlighted face.
4. **DXF:** use F10 for curves and V2 for layered hole targets. Distinguish
   closed regions, open lines, points and layer-based selection. Ask the user
   to predict what an operation would consume before generating it.
5. **Units and multiple inputs:** use V1/V3; correct a known scale mismatch,
   import a second model, switch selection and inspect dimensions. Repeat
   with different import order. Save/reopen the scratch copy to check that
   the declared geometry meaning survives.
6. **Unhappy path:** cancel an import; attempt malformed/unsupported input in
   scratch; open F7. Assess error locality, retained work and recovery advice.

## Specific questions

- Is opening a project distinct from importing another model? Can a user start
  over without accidentally adding geometry to the current job?
- Are dimensions/units visible for drawings as well as meshes? Can the user
  distinguish rescaling input from changing stock or the view zoom?
- Does successful import mean usable geometry? How are missing geometry,
  partial import, face restrictions and unsupported entities communicated?
- Are selected model, setup model scope and operation target distinguishable?
- Can a user recognize that a 3-axis/top-down workflow cannot form an undercut,
  instead of spending time tuning an irrelevant toolpath?

## Source anchors for the diagnostic pass

- `crates/rs_cam_viz/src/ui/menu_bar.rs:35-84`: import/open routes and native dialogs.
- `crates/rs_cam_viz/src/ui/setup_panel.rs:92-122`: Models disclosure and actions.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:218-243,755-985`: getting started and
  model details/scale controls. Check mesh-only conditions rather than assuming
  all supported formats expose equivalent controls.
- `crates/rs_cam_viz/src/controller/io.rs:17-153`: separate import/rescale/reload
  handlers; STEP rescale early return is a specific source lead to verify live.
- `crates/rs_cam_viz/src/app/viewport.rs`: face and drill picking.
- Core model-units and face-selection sentries; `FEATURE_CATALOG.md` partial areas.

## Deliverable additions and boundary

Produce a format matrix: raw import → dimensions/units → selection → usable
operation input → reopen, each with evidence and limitations. Record time to a
correctly understood input, not just time to a successful file load.

Give R02 a model-intent record (size, frame, intended face); give R03 explicit
selection/target assumptions. R08 owns the full persistence/recovery audit.
Do not design physical workholding or review every operation in this package.
