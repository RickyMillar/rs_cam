# R02 — Matching the virtual job to the real setup

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R02.
Write `../results/R02/{REPORT,trace}.md` and evidence. Source paths are repo-relative.

## Outcome

The operator can describe the stock, material, machine, workholding, presented
face and physical zero, and can verify that the virtual job expresses that intent.
“Fields filled in” is not completion. No machine operation is part of this review.

## Task cards

1. **One board:** using F1 or F4, “Prepare this part for the board and router on
   this setup card.” Supply intended stock dimensions/material and machine facts,
   not UI field names. Ask the operator to show the part inside the stock and
   explain stock origin versus work datum. Try auto-from-model and an oversized
   manual stock, then change padding/model size and predict what moves.
2. **Machine and material:** choose a preset/library machine, inspect editable
   limits and prepare a GRBL-settings text import preview if applicable. Verify
   units, absent kinematics, project snapshot versus library ownership and the
   consequence of changing machine/material after authoring an op.
3. **Workholding:** add a fixture and a keep-out in scratch. Set physical height,
   clearance and rigidity. Ask what each is intended to protect, which checks
   use it and whether changing it invalidates a prior clearance result.
4. **Flip:** construct V4’s asymmetric two-sided job. Find the two-sided setup
   helper without being told its name. Configure pins, flip axis and face order.
   Ask the operator to physically describe how the board re-seats and where Z
   will be re-zeroed. Check model scope per setup and generated pin-drill intent.
5. **Side face:** inspect a lateral setup with a drawing/mesh combination. Try
   the supported path, then the workholding restriction. A clear refusal can be
   a correct product outcome; measure explanation and recovery, not willingness
   to accept unsupported geometry.
6. **Mistake and correction:** deliberately choose the wrong face or origin in
   scratch, then correct it. Check selection, camera, height planes, dimensions,
   dependencies and readiness feedback after each transition.

## Specific questions

- Can the operator find machine/material/workholding setup from the normal path?
- Do stock coordinates, setup-local coordinates, datum and height references
  look like different concepts when they are different?
- Which settings are global, per setup or per operation? Are inherited values
  visible, and does an override say what wins?
- Does the viewport help confirm the setup, or merely illustrate form values?
- Can a user tell front/back/left/right by the physical face, not by a remembered
  implementation convention? Can a flip appear plausible but be wrong?
- What is not checked yet, and where is the next verification action?

## Source anchors

- `crates/rs_cam_viz/src/ui/setup_panel.rs`: stock/machine/setup navigation.
- `crates/rs_cam_viz/src/ui/properties/{stock,setup}.rs`: material, origin,
  auto-sizing, datum, model scope, fixtures, keep-outs, pin editing.
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1136-1634`: machine library,
  machine fields, kinematics and settings-import preview/apply.
- `crates/rs_cam_viz/src/controller/events/model.rs:327`: two-sided helper.
- `crates/rs_cam_core/tests/{setup_datum_round_trip_p2,export_datum_setup_frame,lateral_setup_end_to_end}.rs`.
- `planning/lateral_setups_2026-08-22/SPEC.md`; current datum/export caveats in
  `CLAUDE.md`. Check actual emitted convention with R07, not just the crosshair.

## Deliverable additions and boundary

Produce a setup-intent record with an annotated physical/virtual frame sketch:
stock dimensions/origin, intended face, datum, fixtures/pins and next setup.
Compare single-sided, flip and lateral journeys and list implicit prerequisites.

R03 receives a setup ready for authoring. R07 consumes the same record to test
export/setup-sheet comprehension. R08 owns broad edit/undo/persistence matrices;
R09 owns detailed camera/picking ergonomics. Do not certify machining safety.
