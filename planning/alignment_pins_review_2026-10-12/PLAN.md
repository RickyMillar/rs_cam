# Alignment pins: correctness and set editing (plan, 2026-10-12)

Owner: the operator. Lead: the local runner session. Status: ACTIVE.
Evidence: `REVIEW.md` in this folder (C1-C9, packages P1-P10).

## The operator's request and rulings

- "the ux is a little clunky at the moment! ITs hard to see whats going on.
  id also like existing pins to be ajustable as a set, not individually."
- "run it" (2026-10-12): run all the packages.
- Blast radius of a pin edit (2026-10-12, option 2): a pin edit marks only
  the toolpaths that use the pins as stale (the pin-drill op, and any op
  whose result depends on the pins). It does not drop every result.
- Standing rulings: no legacy shims (a break is stated in the commit);
  ASD-STE100 prose; agents never commit; no large test gates.

## Waves

Each wave: one worktree per agent under `.claude/worktrees/<name>`, branch
`pins-<name>`, `CARGO_TARGET_DIR=target/<name>` (deleted with the worktree),
cargo only through `scripts/cargo_lane.sh` under `timeout`. The lead
reviews by path, commits on the branch, merges master in, checks, merges.

| Wave | Agent | Packages | Depends on |
|---|---|---|---|
| A | `keying` | P1 intrinsic keying check (rotation symmetry about the centroid, with diameters) + P2 full setup map (face_up + z_rotation) in the validator | none |
| B | `pinset` | P3 derive the flip axis, delete the cache + P9 clean-ups + P4 pattern module and `SetAlignmentPins` (with the stale ruling above) | A |
| C1 | `pinmcp` | P5 MCP parity and tools + P8 one source of hole positions | B |
| C2 | `pinplace` | P6 placer inputs, 2D walls, pin tool by type | B (parallel with C1) |
| D | `pinui` | P7 GUI pattern editor and viewport overlays | C1, C2 |
| E | `pinexport` | P10 pins in the export header and the setup sheet | B (may run with D) |

## Acceptance

1. The five C1 cases give the right keyed verdict; a 6/8 mm pair keys.
2. A `bottom` + `180` setup is judged against the X mirror.
3. GUI and MCP pin edits give the same toolpath list and the same report.
4. The mini test piece (`planning/rivmap_block_job/testpiece_mini.toml`)
   with the 3-pin preset loads with no keying warning, drills each hole
   once, and the operator can move the set as one.
5. The operator looks at the pin editor and the viewport overlays.

## Status 2026-10-12

All waves merged: A 8fe36b37, B ad834275, C1 cb0e3828, C2 e34c508e, E 51801f1f,
D 97522f09. Live: the mini test piece now has the 3-pin preset (axis pin
(59, 32.5), pair (7.511, 9) and (7.511, 56)), keyed with a 6.57 mm margin,
walls >= 3.0 mm; the DXF drill and the helix bore are deleted, so the Pin
Drill op (6 mm drill, 6.3 mm into the spoilboard) is the one hole source.
0 collisions; the G-code headers list the dowels.

Open (lead look on screen):
- Operator look at the editor and the viewport marks (acceptance 5).

## Follow-up 2026-10-12: drill by size, fix in place, plan view, wizard

Trigger: the operator's complaint about the mixed preset refusal
("Add a drill operation ..., then choose the preset again"). Review:
`REVIEW_UX.md`. Agent work, uncommitted, for the lead to review by path.

- R1: one pin-drill operation per pin diameter
  (`AlignmentPinDrillConfig::diameter`, required). The door creates,
  moves and removes one op per size; `pin_tool_bindings` replaces the one
  pin tool. The mixed preset is now machinable. Sentry:
  `pin_holes_are_drilled_by_size_r1`. Break: a project file without the
  field does not load; every project file in the repo was migrated.
- R2: the "Drills" block of the pin section: per size, the tool chooser,
  "Add a <d> mm drill", "Add a Top setup". The mixed preset never refuses
  for a missing drill.
- R3: the overhead plan view (`pins/plan_view.rs`); a drag moves the set.
- R4: the pin wizard (`ui/pin_wizard.rs`), four steps, own draft.
- R5: the stale load warning drops after a keying edit; MCP
  `set_ui_view(modal: none)` closes it. The pin section fits 240 points.

Open: the lead's look on screen at 1600 x 900; the start value of the
large mixed dowel (small + 2 x `PIN_MATCH_TOL_MM`) is a choice, not a
dowel standard.
