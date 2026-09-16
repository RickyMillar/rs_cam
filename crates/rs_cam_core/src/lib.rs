//! `rs_cam_core` — the CAM engine and shared data model.
//!
//! The crate is layered roughly in this order:
//!
//! 1. **Import** — STL / SVG / DXF / STEP into geometry primitives (`io`)
//! 2. **Tool model** — cutter geometry, holder/shank envelope, vendor
//!    metadata (`tool`, `material`, `machine`)
//! 3. **Operations** — 2.5D and drilling toolpath generation (`ops`),
//!    roughing (`adaptive`, `adaptive3d`) and 3D finishing (`finish`),
//!    over the derived geometry (`geometry`), the surface fields
//!    (`surface`) and the grid-walk maps (`maps`), emitting the shared
//!    `Toolpath` IR
//! 4. **Dressups** — entry strategies, leads, dogbones, arc fitting,
//!    feed optimization, TSP rapid ordering (`dressup`)
//! 5. **Simulation** — tri-dexel volumetric stock (`dexel_stock`), the
//!    stock model and cut record (`stock`), collision checks
//! 6. **Export** — G-code (`gcode`), SVG/HTML preview, fingerprints and
//!    the G-code validator (`export`)
//!
//! Two folders cut across those layers: `trace` holds the records that
//! describe a generated toolpath, and `feeds` with `tool_load` holds the
//! feeds-and-speeds model. The crate spine stays at the root: `geo`,
//! `polygon`, `mesh`, `toolpath`, `ids`, `interrupt` and `measurement`.
//!
//! The `Toolpath` IR is the boundary between planning and post / output;
//! GUI and CLI consumers depend only on the public surface of this crate.
//! See `FEATURE_CATALOG.md` and `architecture/` at the repo root for the
//! product surface and design rationale.

pub mod adaptive;
pub mod adaptive3d;
pub mod compute;
pub mod dexel_stock;
pub mod diagnostics;
pub mod dressup;
pub mod export;
pub mod feeds;
pub mod finish;
pub mod gcode;
pub mod geo;
// The walk grid `tier_map` and `reach_map` share; private to the crate.
pub mod geometry;
pub mod ids;
pub mod interrupt;
pub mod io;
pub mod machine;
pub mod maps;
pub mod material;
pub mod measurement;
pub mod mesh;
pub mod metrology;
pub mod ops;
pub mod polygon;
pub mod session;
pub mod stock;
pub mod surface;
pub mod tool;
pub mod tool_load;
pub mod toolpath;
pub mod trace;
pub mod util;

pub use ids::ToolpathId;
