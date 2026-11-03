# Feeds contract: status

Start here in a new session:

1. Read `PLAN.md` section 0 (the operator rulings) and section 10 (the terms).
2. Find the first phase below that is not DONE.
3. Read its card in `phases/`. The card is the full hand-off.
4. Update this file in the same commit as the phase.

Citations in the cards are `path:line@40cf411a7` plus a symbol. Find the
code by the symbol when the line has moved.

## Open decisions

| # | Decision | Blocks | State |
|---|---|---|---|
| D1 | Q6: do the refusals that exist today stay? **Ruled 2026-11-03: no. The program never refuses; every finding warns.** | P2 | RULED |
| D2 | K2b envelope plot sketch (`phases/P5b.md`). | P5b | not asked |
| D3 | P0 D1: the nz-wanaka-21 pin hole (26.0 mm min) is longer than the 6 mm flute (25.0 mm). Pick a longer-flute pin tool, or accept the hole past the flute with a warning. | P0 item 3 | asked 2026-11-03 |
| D4 | P0 D2: `dwell_time` works only in G82; every tracked drill op uses peck. Delete the field (recommended), or emit G4 at each peck bottom. | P0 item 4 | asked 2026-11-03 |
| D6 | P6 D3: may the K3 migration script rewrite your tool library `~/.config/rs_cam/tools/*.toml` (6 files, outside the repo; it keeps a `.bak`)? After P6 they fail to load until migrated. | P6 | asked 2026-11-03 |
| D7 | A missing input (unset depth or floor, no chip data, no tool, inch units, a zero feed) still stops generation or emission with a reason; only findings warn. Confirm. | P2, P5 | asked 2026-11-03 |
| D5 | P0 D3: the Amana tapered 1/16" 2F rows (3 rows) print a chip band that disagrees with their own printed IPM. Use the IPM band (recommended), or refuse the cell. | P0 item 5 | asked 2026-11-03 |

## Phases

| Phase | Card | State | Commit | Next action |
|---|---|---|---|---|
| 0 Local fixes | `phases/P0.md` | NOT STARTED | | |
| 1 Wildcard lint | `phases/P1.md` | NOT STARTED | | |
| 2 K6 consequence column, no refusals | `phases/P2.md` | NOT STARTED | | |
| 3 K4 entry contract | `phases/P3.md` | BLOCKED: the perf session must commit `ops/project_curve.rs` | | |
| 4 K1 `CutSpec` | `phases/P4.md` | NOT STARTED | | |
| 5 K2 / K2b envelope | `phases/P5.md` | NOT STARTED (needs P4) | | |
| 5b GUI envelope + K8 lineage | `phases/P5b.md` | NOT STARTED (D2, needs P5) | | |
| 6 K3 `ToolFact` | `phases/P6.md` | NOT STARTED | | |
| 7 K5 feasibility rules | `phases/P7.md` | NOT STARTED (needs P6) | | |
| 8 Q1 / Q6 / Q8 | `phases/P8.md` | NOT STARTED (Q1 needs P9 changes 1-2 first) | | |
| 9 K7 one engagement | `phases/P9.md` | NOT STARTED (heavy-tests: ask first) | | |

## Not in a phase

- `crates/rs_cam_cli/src/suggest.rs` and the `Suggest` command in
  `crates/rs_cam_cli/src/main.rs`: written 2026-11-03 for the blind review;
  uncommitted until `/verify` passes.
- The op 2 rest planner (14.5 mm entry, 13 mm links) needs a simulation; it
  is separate work (`PLAN.md` section 6).
