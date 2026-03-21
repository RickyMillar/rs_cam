# Credits

This file records the primary algorithmic inspirations, bundled data sources, direct dependencies, and runtime assets used by `rs_cam`.

It is not a substitute for `Cargo.lock`, crate licenses, or third-party NOTICE files. It is the human-readable provenance map for the repo.

## Algorithm lineage

### OpenCAMLib

`rs_cam` explicitly follows OpenCAMLib for a large part of its cutter/surface-contact model:

- trait-style cutter abstraction and drop-cutter contact structure
- flat, ball, bull, V-bit, and tapered-ball cutter geometry
- push-cutter and waterline concepts
- several edge-contact formulas referenced in code comments and research notes

Repo references:

- `crates/rs_cam_core/src/tool/mod.rs`
- `crates/rs_cam_core/src/tool/vbit.rs`
- `crates/rs_cam_core/src/tool/bullnose.rs`
- `research/04_open_source_reference.md`
- `research/raw_opencamlib_math.md`

Primary upstream:

- OpenCAMLib: <https://github.com/aewallin/opencamlib>
- Anders Wallin CAM notes: <https://www.anderswallin.net/cam/>

### Freesteel / libactp / Adaptive2d / FreeCAD CAM

Adaptive clearing in `rs_cam` is explicitly documented as Freesteel/Adaptive2d-inspired. The repo also draws workflow and dressup ideas from FreeCAD CAM.

Repo references:

- `crates/rs_cam_core/src/adaptive.rs`
- `research/02_algorithms.md`
- `research/04_open_source_reference.md`

Primary upstream:

- libactp / Adaptive2d: <https://github.com/Heeks/libactp-old>
- FreeCAD CAM: <https://github.com/FreeCAD/FreeCAD>

### Clipper2, `geo` / `i_overlay`, and CavalierContours

The 2D pocket/profile layer depends on robust polygon booleans and offsets. The repo research and code reference:

- Clipper/Clipper2-style offset and boolean workflows
- `geo` with `i_overlay` under the hood for polygon operations
- `cavalier_contours` for arc-preserving offsets

Repo references:

- `research/04_open_source_reference.md`
- `research/05_rust_ecosystem.md`
- `Cargo.toml`

Primary upstream:

- Clipper2: <https://github.com/AngusJohnson/Clipper2>
- CavalierContours: <https://github.com/jbuckmccready/CavalierContours>

### Kiri:Moto

`rs_cam` research cites Kiri:Moto as a strong reference for heightmap-based CAM and rasterized tool footprints, particularly for simulation and heightmap-oriented roughing ideas.

Repo references:

- `research/04_open_source_reference.md`
- `research/07_blue_sky.md`

Primary upstream:

- Kiri:Moto: <https://github.com/GridSpace/grid-apps>

### Other named algorithm references in the repo

The repo text or code explicitly references these algorithm families or techniques:

- drop-cutter
- push-cutter
- waterline fiber/weave contour extraction
- Douglas-Peucker simplification
- nearest-neighbor plus 2-opt TSP ordering
- radial chip thinning
- constant-scallop-height formulas
- marching-squares-style contour extraction
- Kasa least-squares circle fitting for arc fitting

Key repo references:

- `crates/rs_cam_core/src/dropcutter.rs`
- `crates/rs_cam_core/src/waterline.rs`
- `crates/rs_cam_core/src/toolpath.rs`
- `crates/rs_cam_core/src/tsp.rs`
- `crates/rs_cam_core/src/scallop_math.rs`
- `crates/rs_cam_core/src/arcfit.rs`
- `crates/rs_cam_core/src/contour_extract.rs`

## Data sources and formulas

### Vendor LUT source manifest

Vendor-seeded feeds/speeds observations are tracked in:

- `crates/rs_cam_core/data/vendor_lut/source_manifest.json`

Visible sources recorded there include:

- Amana feed and chipload charts
- Onsrud cutting-data recommendations
- Harvey speed/feed references and MAP
- Whiteside router-bit product/category pages
- Sandvik Coromant milling formulas
- GARR chip-thinning reference
- Autodesk Fusion adaptive reference help

The manifest includes URLs, titles, coverage notes, and access dates.

### Material-property and force-model references

The integrated feeds/material stack also depends on direct material and formula sources captured during development. The source set currently includes:

- USDA Forest Products Laboratory Wood Handbook
- USDA FPL maple species technical sheet
- Riga Wood plywood handbook
- Roseburg Medite MDF technical data
- Timber Products Ampine particleboard technical data
- DPI hardboard specifications
- Composite Panel Association standards index
- Sandvik Coromant milling-formula guidance

Those sources underpin material hardness anchors, sheet-good ordering, and conservative cutting-force assumptions used by the current integrated model.

### Formula provenance

The feeds/speeds implementation in `rs_cam_core` uses:

- vendor-published chipload and RPM guidance when a LUT match exists
- an integrated fallback chipload model based on diameter and material hardness
- Sandvik and GARR references for power and chip-thinning validation

The relevant implementation and provenance entry points are:

- `crates/rs_cam_core/src/feeds/mod.rs`
- `crates/rs_cam_core/src/feeds/vendor_lut.rs`
- `crates/rs_cam_core/data/vendor_lut/source_manifest.json`

## Research and terminology sources

The repo also preserves longer-form research and terminology mapping here:

- `research/02_algorithms.md`
- `research/03_tool_geometry.md`
- `research/04_open_source_reference.md`
- `research/05_rust_ecosystem.md`
- `research/08_ux_terminology.md`
- `research/raw_algorithms.md`
- `research/raw_open_source.md`
- `research/raw_opencamlib_math.md`
- `research/raw_rust_ecosystem.md`

Autodesk Fusion documentation and terminology are used as comparative references in the research material and vendor-LUT manifest; they are not presented as original `rs_cam` documentation.

## Direct Rust dependencies

These are the direct workspace dependencies visible in the current manifests.

### Core and shared

- `nalgebra`
- `stl_io`
- `kiddo`
- `rayon`
- `thiserror`
- `tracing`
- `clap`
- `anyhow`
- `serde`
- `toml`
- `geo`
- `cavalier_contours`
- `usvg`
- `dxf`
- `criterion`

Manifest references:

- `Cargo.toml`
- `crates/rs_cam_core/Cargo.toml`
- `crates/rs_cam_cli/Cargo.toml`

### GUI and desktop runtime

- `eframe`
- `egui`
- `egui-wgpu`
- `rfd`
- `bytemuck`

Manifest reference:

- `crates/rs_cam_viz/Cargo.toml`

For the full transitive dependency graph, see `Cargo.lock`.

## Runtime assets and external services

The HTML visualization path loads `three.js` from jsDelivr:

- `crates/rs_cam_core/src/viz.rs`

Those viewer templates should be considered part of the third-party runtime surface when packaging or redistributing exported HTML.

## Internal docs that should stay aligned with these credits

- `README.md`
- `FEATURE_CATALOG.md`
- `architecture/high_level_design.md`
- `crates/rs_cam_core/src/feeds/INTEGRATION.md`

When new external datasets, formulas, or reference implementations are added, update this file and the relevant source manifests in the same change.
