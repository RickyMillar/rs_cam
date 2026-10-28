# C3, the D55 port (G-AIRPECK): pre-registration

Written before the code change and before any parity run on master.
Base: `e5614d2b8` (C2). The work runs in an isolated detached worktree at
that commit with its own `CARGO_TARGET_DIR` (the shared tree holds I5's
uncommitted core edits, `op_golden_g_opgolden.toml` included).

## The change (ported from research commit `f2724a5bd`)

`adaptive3d/path.rs::emit_peck_plunge` takes the planner's contact read
(`RapidWithFloor::contact_z`, which the helix and the ramp already take).
The rapid still stops at the sliver-safe floor. The peck ladder starts at
`min(ladder_start, max(contact, entry.z) + entry_clearance_mm)`. Above that
start, one straight feed goes through air. Master's function and call site
are the same as the research base, so no name changes.

## What can change

Only the vertical moves of a `Plunge` entry (`EntryPlunge` feeds and
`Retract` rapids) above the contact start. Every XY move stays identical.
The planner stamp does not change (it stamps the column, not the peck
shape), so the cut moves and the rapid floors do not change.

## Predictions for master's parity set

| Item | Entry | Prediction |
|---|---|---|
| `adaptive3d_emission_byte_parity` `contour_parallel` | Plunge | changes (research: 433 → 422 moves, 5 feeds and 6 retracts go) |
| `contour_parallel_flat_blend`, `contour_parallel_by_area` | Plunge | change (the same hemisphere and strategy; new cases since the research run) |
| `geodesic_adaptive`, `geodesic_adaptive_boundary` | Plunge | may change (the research arm changed there); not predicted by count |
| op golden `adaptive3d/fresh` | Plunge (default) | changes: 379 → 368 moves, 18 → 12 rapids (the research measured this row at the same "before" values) |
| op golden `adaptive3d/rest` | Plunge (default) | may change |
| op golden `adaptive3d/by_area_helix_cp` | Helix | no change |
| op golden `adaptive3d/ramp_geodesic` | Ramp | no change |
| every other op-golden row (2D and finish ops) | — | no change |
| `rs_cam_cli` tests, rest cascade parity | — | pass unchanged (no pinned 3D Rough moves) |

For every changed item the report gives old → new and the first differing
move, and checks that the XY moves are identical bit for bit.
