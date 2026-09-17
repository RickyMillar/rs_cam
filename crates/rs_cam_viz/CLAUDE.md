# rs_cam_viz instructions

`rs_cam_viz` owns the egui desktop application, controller, compute workers,
rendering and the embedded MCP server. The server lives here, not in
`rs_cam_mcp`. Read root `CLAUDE.md` first.

## The directory map

Most directories carry their own `CLAUDE.md`. Read it before you change the
directory; it holds the file map, invariants, sentries and traps.

| Directory | What it holds | Directory file |
|---|---|---|
| `app/` | The application shell, the frame loop and the embedded MCP server | `app/CLAUDE.md` |
| `compute/` | The background worker lanes | `compute/CLAUDE.md` |
| `controller/` | The bridge between a UI intent and `ProjectSession` | `controller/CLAUDE.md` |
| `render/` | The wgpu viewport pipelines | `render/CLAUDE.md` |
| `state/` | GUI state: freshness, staleness, selection, viewport, jobs | `state/CLAUDE.md` |
| `ui/` | Every egui panel, modal and workspace | `ui/CLAUDE.md` |
| `ui/components/` | The shared component set | `ui/components/CLAUDE.md` |
| `ui/feeds/` | The feeds and speeds surfaces | `ui/feeds/CLAUDE.md` |
| `ui/overlays/` | The viewport Overlays panel and its registry | `ui/overlays/CLAUDE.md` |
| `ui/properties/` | The inspector tabs | `ui/properties/CLAUDE.md` |
| `interaction/`, `io/` | Picking and mouse handling; project import, export and the setup sheet | — |

## GUI contracts

- GUI state is not an alternate data model. Mutate core through
  `apply(Command)` and use `Effects.stale`.
- When GUI state gains a field, audit the setup sheet, project IO, the
  controller fixtures and the MCP and UI initialisers.
- Generated worker IR and post-simulation emitted IR can differ. A surface
  must name which evidence it reports; a planned reading is not emitted.
- Simulation presentation reads bounded triage first; `NotMeasured` abstains.

## Tests

Use focused `rs_cam_viz` integration tests first. A source-scanning test needs
a non-vacuity anchor and must not match a comment. Prefer behavioural egui
harness coverage when a contract can be rendered.
