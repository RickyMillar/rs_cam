# R07 — Readiness, export and the physical handoff

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R07.
Write `../results/R07/{REPORT,trace}.md` and evidence. Source paths are repo-relative.
Only scratch output labelled REVIEW ONLY — NOT FOR MACHINING. No machine/sender.

## Outcome

The operator knows what is verified, what is still uncertain, what they are
accepting, which files will be produced and how those files map to physical tools,
stock faces and work zero. A syntactically valid NC file is not sufficient.

## Task cards

1. **Normal handoff:** from the small F1/F3 verified job, find the next step and
   export a scratch file plus setup sheet. Ask the user to explain what “ready”
   means and identify the machine/post, enabled operations, datum, tools and
   remaining manual checks. Compare that explanation to the actual output.
2. **Incomplete evidence:** try generated-but-unsimulated, stale, partial-scope,
   measured-warning and unmodeled/drill cases. Include missing holder data.
   Observe how the dashboard, preflight and wizard treat each and what can be
   overridden. Do not weaken a gate to make the review proceed.
3. **Route comparison:** inspect File → wizard, Readiness → export and the
   expert direct-export submenu/shortcut, including combined and per-setup
   routes. Follow actual handlers and scratch output. A backend `export_gcode`
   call cannot establish that the wizard route is understandable or equivalent.
4. **Wizard:** walk its current seven steps: post; output layout; coordinates
   and units; tool change/spindle; setup pauses; preview/validate; save. Change
   one inherited setting, inspect preview, navigate back, cancel, reopen and
   determine what persisted. Ask which edits changed the project versus this
   export. Exercise error/override wording without writing production output.
5. **Two-sided handoff:** use V4 and R02's setup-intent record. Choose file split,
   tool-change handling and setup pause instructions. Ask someone to describe
   the flip, pin registration, XY zero and Z re-zero from the sheet/NC comments
   alone. Inspect corresponding emitted coordinates and setup transitions.
6. **Files and recovery:** check filename previews/collisions, save-dialog
   cancellation and an unavailable scratch destination. Can the user identify
   the final saved artifacts and distinguish them from stale earlier exports?

## Specific questions

- Does readiness make a stronger promise than the actual checks support?
- Do equivalent export entry points use compatible scope and safety semantics?
  Are genuine expert shortcuts retained without silently omitting a check?
- Are required actions separated from advisories and estimates? Are overrides
  explicit, scoped and understandable rather than a generic warning dismissal?
- Can the operator explain WCS, datum, units, safe-Z, dry-run and controller
  tool-change support without confusing project defaults and export overrides?
- Are manual setup pauses/probing instructions actionable for a real handoff?
- Does save failure/cancel leave a clear project and file state?

## Source anchors

- `crates/rs_cam_viz/src/ui/{readiness_panel,readiness,preflight}.rs`.
- `crates/rs_cam_viz/src/ui/menu_bar.rs:86-125`: export route inventory.
- `crates/rs_cam_viz/src/ui/export_wizard.rs:25-69`: actual seven-step dispatch;
  later step bodies/handlers, not the stale introductory “only Step 1” comment.
- `crates/rs_cam_viz/src/io/` and `src/controller/events/`: export/save dispatch.
- `crates/rs_cam_viz/tests/wizard_e2e.rs`: backend emission/validation coverage,
  explicitly not a driven UI test; `modulated_feeds_reach_gcode_g_modexport.rs`.
- `crates/rs_cam_core/tests/export_datum_setup_frame.rs`; setup-sheet emitter.

## Deliverable additions and boundary

Provide an export-route × state × scope/override matrix and compare the user's
physical setup teach-back with actual artifacts. Keep safety-semantics defects
separate from layout/wording improvements, but cross-link their effect on trust.

R02 owns physical setup authoring; R06 owns diagnostic interpretation; R08 owns
staleness/persistence mechanics. Do not claim controller execution was verified.
