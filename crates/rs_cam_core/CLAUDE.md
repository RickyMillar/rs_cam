# rs_cam_core instructions

`rs_cam_core` owns the CAM engine, data model, generation, simulation, feeds,
diagnostics, session state and G-code export. Keep it GUI-free. Read the root
`CLAUDE.md` first.

## The folder map

`src/` holds 8 files and 24 folders. The 8 root files are the crate spine —
the vocabulary every layer names: `lib.rs`, `geo.rs`, `polygon.rs`, `mesh.rs`,
`toolpath.rs`, `ids.rs`, `interrupt.rs` and `measurement.rs`. Everything else
sits in a folder. Most folders carry their own `CLAUDE.md`; read that file
before you change the folder. It holds the folder's file map, invariants,
sentries and traps, which this file does not repeat.

| Folder | What it holds | Folder file |
|---|---|---|
| `io/` | Every door that reads a file: model, DXF, SVG, STEP, and the TOML tool and machine libraries | `io/CLAUDE.md` |
| `tool/` | Cutter geometry, holder and shank envelope, vendor metadata | `tool/CLAUDE.md` |
| `material/` | Material catalogue and the wood species library | — |
| `machine/` | Machine profile, kinematics, utilisation, strategy advisor | `machine/CLAUDE.md` |
| `geometry/` | Regions, grids, distance fields, contours, the machining boundary | `geometry/CLAUDE.md` |
| `surface/` | Drop and push cutter, slope, rest field, flow routing, reach | `surface/CLAUDE.md` |
| `maps/` | Tier and reach maps, islands, and their bounded caches | `maps/CLAUDE.md` |
| `ops/` | 2.5D and drilling operations, and the depth-stepping helper | `ops/CLAUDE.md` |
| `adaptive/`, `adaptive3d/` | Adaptive clearing, 2D and 3D | `adaptive/CLAUDE.md`, `adaptive3d/CLAUDE.md` |
| `finish/` | 3D finishing strategies and the unified finish planner | `finish/CLAUDE.md` |
| `dressup/` | Post-generation transforms: entry, leads, arc fitting, conditioning, feed optimisation, feed modulation, TSP | `dressup/CLAUDE.md` |
| `dexel_stock/` | The tri-dexel simulation engine | `dexel_stock/CLAUDE.md` |
| `stock/` | The stock data model, the cut record, the triage and the stock meshes | `stock/CLAUDE.md` |
| `trace/` | The records that describe a generated toolpath | `trace/CLAUDE.md` |
| `gcode/` | G-code emit and the post-processors | `gcode/CLAUDE.md` |
| `export/` | Output that is not G-code: preview, fingerprints, the G-code validator | `export/CLAUDE.md` |
| `feeds/`, `tool_load/` | Feeds and speeds, the cutting load model, the optimiser | `feeds/CLAUDE.md`, `tool_load/CLAUDE.md` |
| `compute/` | Operation dispatch, configuration catalogue, simulation orchestration | `compute/CLAUDE.md` |
| `session/` | `ProjectSession`, commands and effects | `session/CLAUDE.md` |
| `diagnostics/` | Diagnostic findings and their adapters | `diagnostics/CLAUDE.md` |
| `metrology/` | Measurement instruments | `metrology/CLAUDE.md` |
| `util/` | Panic classification and crate build identity | — |

## Core contracts

- Keep the crate GUI-free. Render and controller state are not part of the
  core model.
- The toolpath IR separates planning from dressups and export.

## Tests and evidence

- Tests live close to the code they protect. The heavy core binaries sit
  behind `heavy-tests`. Run the smallest relevant sentry; each folder file
  names the sentries for its own folder.
- Current code and sentries outrank a historical description. Do not copy a
  retired measurement into a new product claim.
- Many sentries name a `planning/…` document as their pre-registration. A
  dead path there is expected; the root `CLAUDE.md` says how to retrieve it.
