# P8 Q6 mock: one collision row on every surface

Status: MOCK, for the operator. No code until the operator agrees (rule: no text clutter).
Author: P8 agent, 2026-11-05. HEAD fb260023b.

## What is already done (P2, K6)

- The checkbox "I understand the risks" and the "Export Anyway" button are gone from
  `ui/preflight.rs`. `rg "Export Anyway|preflight_export_confirm|has_failures" crates/rs_cam_viz/src`
  finds nothing.
- Core `export_warnings` (`session/compute/export.rs`, `Gate::GuiConfirm`) files one
  `project.rapid_collision` row per toolpath. MCP and the CLI read the same rows.
- The pre-flight draws ONE chip row (`draw_danger_chips`) and ONE Export button.

## What is left: two GUI reads that are not the core rows

1. The "Rapid collisions" check card reads `sim.checks.rapid_collisions` (GUI state) through
   `readiness::rapid_collision_check` and prints "N detected". The core row counts per toolpath.
2. `asks_a_click` adds `!sim.checks.rapid_collisions.is_empty()` to the core rows. The core
   rows already hold `project.rapid_collision`, so the term is a second read.

## Export area, before (today)

```
 [x] Rapid collisions   3 detected              Simulation >     <- GUI state count
 [ok] Holder clearance  clear                   Simulation >
 ...
 ----------------------------------------------------------------
 (Machine 1) (Tool 2) (Part 1)                                   <- core rows, hover: id · toolpath
 [Cancel] [Export]  (Danger style)
```

Lines in the export area: 3 (card, chip row, button row).

## Export area, after (proposal)

```
 [x] Rapid collisions   2 toolpaths             Simulation >     <- core rows, same count as MCP/CLI
 [ok] Holder clearance  clear                   Simulation >
 ...
 ----------------------------------------------------------------
 (Machine 1) (Tool 2) (Part 1)                                   <- unchanged
 [Cancel] [Export]  (Danger style)
```

Lines in the export area: 3. No new line, no new sentence. The card detail changes from a
move count to the count of `project.rapid_collision` rows (the unit the MCP export text and
the CLI `summary.json` `warnings` give). The hover of the chip already lists `id · toolpath`.

## Reused kit

- The check card (`check_card`, `CheckStatus` colour) and `StatusChip` with `danger_role`.
- No new chip, colour or text row.

## Code sites

- `crates/rs_cam_viz/src/ui/readiness.rs` `rapid_collision_check`: read the
  `project.rapid_collision` rows of `io::export::preflight_warnings`, not `sim.checks`.
- `crates/rs_cam_viz/src/ui/preflight.rs` `draw`: the card detail from the same rows; drop the
  `sim.checks.rapid_collisions` term of `asks_a_click` (the holder term stays: the GUI holder
  check is not in the session).
- New sentry `crates/rs_cam_viz/tests/a_collision_is_the_same_warning_on_every_surface_q6.rs`:
  with a rapid collision, the export button is enabled, and the GUI row ids equal the MCP
  export text ids and the CLI summary ids.

## Question for the operator

The card counts toolpaths with a rapid collision, not moves. Is that the number you want on the
card? The move count stays in the Simulation workspace (`list_collisions`).
