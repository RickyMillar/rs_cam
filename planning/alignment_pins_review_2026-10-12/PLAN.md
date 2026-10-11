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
