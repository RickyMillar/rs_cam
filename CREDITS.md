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

### Tri-dexel volumetric simulation

The stock simulation in `rs_cam` uses a tri-dexel approach: three orthogonal grids of ray segment lists that represent material presence along the Z, X, and Y axes. This is the industry-standard technique for 3-axis CNC simulation, used by commercial engines including ModuleWorks (Mastercam, Siemens NX) and MecSoft (RhinoCAM, VisualCAD/CAM).

The implementation uses `SmallVec<[DexelSegment; 1]>` for allocation-free single-segment fast paths and supports all six cardinal cut directions for multi-setup machining.

Repo references:

- `crates/rs_cam_core/src/dexel.rs` — core segment, ray, and grid primitives
- `crates/rs_cam_core/src/dexel_stock.rs` — volumetric stock representation with tool stamping
- `crates/rs_cam_core/src/dexel_mesh.rs` — mesh extraction for viewport rendering
- `architecture/TRI_DEXEL_SIMULATION.md` — design rationale and implementation plan

Industry references:

- ModuleWorks tri-dexel simulation: <https://www.moduleworks.com/>
- MecSoft GPU tri-dexel: <https://mecsoft.com/>

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

2026-05-29 ingest added three Amana charts as bundled runtime rows
(`amana_vgroove_engraving.json`, `amana_compression.json`): the AMS-159
V-Groove chart, the Spektra 15/30/45/120° Engraving chart, and the
Solid-Carbide Compression Spiral chart. Only wood-family rows are bundled;
plastics/aluminum rows from the same charts (plus Onsrud, Harvey/Helical/Garr,
and the Kc/hardness research sources) are collected, cited, and staged under
`planning/data_ingest_2026-05-29/` but NOT yet bundled — they await a
`Material`-enum extension. See `planning/feeds_data_ingest_consolidation_2026-05-29.md`.

2026-05-30 Phase 1 ingest promoted the staged non-wood rows now that the
`Material::Plastic { family }` per-family Kc, the new `Material::Aluminum
{ alloy }` variant, and `Vendor::Helical` have landed. Six new bundled
runtime files: `amana_plastic_oflute.json`, `amana_zrn_aluminum.json`,
`amana_vgroove_aluminum_acrylic.json`, `onsrud_plastic.json`,
`whiteside_rpm_assorted.json`, `helical_aluminum.json`, plus the
acrylic compression-spiral row appended to `amana_compression.json` (26
new rows total; bundled count 85 → 111). Source manifest entries added
for all nine new `source_id`s. See `planning/feeds_data_ingest_consolidation_2026-05-30.md`.
The Garr aluminum staged rows remain deferred (per-series flute-count
split + pass_role repair still pending) and are tracked in the
Phase 3 backlog.

2026-05-31 Phase 4 bulk LUT row promotion landed 117 new bundled rows
across five files (bundled count 111 → 228), all from the Phase 3
agent-fleet staging:

- `amana_long_tail.json` (37 rows) — Spektra Spiral Plunge + ZrN 3D
  Profiling v8 extensions to the existing Amana coverage
- `onsrud_ocr.json` (47 rows) — OCR-extracted Hard Wood / Soft Wood /
  MDF cutting-data PDFs
- `whiteside_fusion360.json` (13 rows) — Whiteside Fusion 360 tool
  library (2019-10-23 community export)
- `freud_solid_carbide.json` (10 rows) — Freud Solid Carbide router-bit
  chart, 1/8"–3/8" subset (hobby spindle envelope)
- `idcwoodcraft_millmage.json` (10 rows) — Community Millmage CSV,
  Grade C cross-vendor sanity data

Seven new `source_id` entries added to `source_manifest.json`. Freud
1/2" rows (chiploads 0.46–0.69 mm/tooth, calibrated for industrial
10–15 kW CNC spindles) are split into
`crates/rs_cam_core/data/vendor_lut/industrial_only/freud_solid_carbide_industrial.json`
and are intentionally NOT loaded by `embedded()` — the sibling
directory makes the hobby/industrial boundary explicit at the path
layer. See `planning/feeds_data_ingest_phase4_2026-05-31.md`.

2026-05-31 Phase B (completion plan) closed the Garr aluminum
deferral with a per-series flute split. The Garr Aluminum Milling
Guide PDF (`TECH_MILLING_ALUMINUM.pdf`) presents one chipload triplet
shared across each page's series. Promoted as
`crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
(11 rows total, bundled count 228 → 239):

- Low-Range page (3 chart entries × 3 series): 242M (2-flute), 842M
  (2-flute), A3 (3-flute) end mills, slotting + profiling at 3 mm and
  6 mm = 9 rows
- High-Range page (A3 only, 3-flute): HEM profiling + finishing at
  6 mm = 2 rows

Two `source_id` entries added (`garr_milling_aluminum_low_range`,
`garr_milling_aluminum_high_range`) to `source_manifest.json`.
Mid-Range (142M/143M) rows and General-Purpose rows are NOT promoted
in this round — neither had pre-authorized flute counts (D4 of the
completion plan locked only the Low-Range trio and A3) and GP
plastics has no `MaterialFamily` enum mapping. See
`planning/feeds_data_ingest_phaseB_2026-05-31.md`.

2026-06-01 Phase 4 (verifier-gated promotion) added 8 net-new rows
to the vendor LUT after the Phase 4 plan's verifier agent confirmed
all candidate rows against their source URLs (14/14 CONFIRMED, 0
MISMATCH — see
`planning/data_ingest_2026-05-30/verification_report_2026-06-01.md`):

- **4 Garr aluminum** completing the Mid-Range and General-Purpose
  deferral from Phase B — 142M slot+profile at 6 mm (CPT 0.090–0.150
  mm/tooth), GP aluminum slot at 3 mm and 6 mm (CPT 0.015–0.051
  mm/tooth). Two new `source_id` entries
  (`garr_milling_aluminum_mid_range`, `garr_general_purpose_milling`)
  added to `source_manifest.json`. Appended to
  `crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
  (now 15 rows total).
- **4 Freud 1/2-inch solid carbide** — the Freud "Router Bit Feed
  and Speed for CNC" 2017-08-22 PDF row for 1/2-inch bits across
  hardwood, softwood, MDF/particle, and plywood-hardwood. The
  validator initially flagged the chiploads (0.46–0.69 mm/tooth) as
  suspicious; the verifier confirmed they are correct Freud
  publishing values at the 1xD-DOC reference condition (the 25%/50%
  derate rule for deeper DOC is captured in `ap_rule`). Appended to
  `crates/rs_cam_core/data/vendor_lut/observations/freud_solid_carbide.json`
  (now 14 rows total). No new `source_id` — reuses the existing
  `freud_router_bit_feed_and_speed_for_cnc_20170822` entry.

Bundled count: **239 → 247**. The Phase 4 plan's other deferrals
(15 Amana v-bits and 1 Onsrud polycarbonate article — both missing
`diameter_mm`; 1 Garr fiberglass/G10 — invalid `material_family`)
remain in staging with documented follow-up paths in their
respective `_gaps.md` files. The verifier audit confirmed each
deferred row's data matches its source — the blocker is purely
schema (Phase 5 work). See
`planning/feeds_data_ingest_consolidation_2026-06-01.md`.

2026-06-01 Phase 5 Step 5.2 (V-bit schema relaxation) promoted 4
net-new vendor LUT rows after relaxing `VendorObservation::
diameter_mm` from `f64` to `Option<f64>` (`crates/rs_cam_core/src/
feeds/vendor_lut.rs`). The matcher's `passes_must_match`,
`score_observation`, and `diameter_scale_factor` paths now skip
diameter scoring / scaling when the row has no anchor diameter —
v-bit charts (angle-driven) and diameter-window articles can be
matched honestly. Promoted rows:

- 3 Amana Spektra engrave rows (30°/30°/45° softwood-hardwood) from
  `amana_spektra_engraving_v4` (no new source — same PDF as the
  Spektra 15°/120° rows already bundled).
- 1 Onsrud polycarbonate article window row from
  `onsrud_routing_polycarbonate_article` (new source_id added to
  `source_manifest.json`; 0.1016–0.3048 mm/tooth across the upcut
  O-flute polycarbonate-routing line, no per-diameter anchor).

The other 11 staged v-bit rows (Amana AMS-159 v-groove series at
18°/30°/45°/60°/90° in softwood/hardwood/acrylic/aluminum) were
already present in the live LUT under the `amana-vgroove-*` naming
with a synthetic `diameter_mm = 6.35`; they were not re-promoted
under the `amana-vbit-*` naming to avoid LUT duplication. Bundled
count: **247 → 251**. See
`planning/phase_5_schema_unlock_2026-06-01.md` and the consolidation
report once Step 5.5 lands.

2026-05-31 Phase C (completion plan) added 5 new
`AluminumAlloy` variants to `crates/rs_cam_core/src/material.rs`,
extending aluminum coverage from 2 alloys (6061-T6, 7075-T6) to 7:

- `Alloy2024T3` — Brinell 120, ASM matweb verbatim
- `Alloy5052H32` — Brinell 60, ASM matweb verbatim
- `Alloy3003H14` — Brinell 42, MakeItFrom (ASM not hosted for 3003)
- `Alloy1100O` — Brinell 23, MakeItFrom (ASM not hosted for 1100)
- `Alloy7050T7651` — Brinell 147, ASM matweb (calc); Kaiser Aluminum
  mill datasheet reports 150 on the same alloy/temper (cross-cited)

All values read verbatim from a fetched datasheet — see
`planning/data_ingest_2026-05-30/hardness_extra.md` H.2 for the
per-alloy verbatim quotes. Kc remains the shared VDI 3323 group-22
Kienzle pair across all alloys (vendor sources don't differentiate
Kc by alloy at our fidelity). Five new `literature_parity` sentries
pin each Brinell value to its citation. See
`planning/feeds_data_ingest_phaseC_2026-05-31.md`.

2026-05-31 Phase D (completion plan) added 6 new `PlasticFamily`
variants and a new `PlasticHardness::RockwellR` scale variant to
`crates/rs_cam_core/src/material.rs`, extending plastic coverage
from 5 families (Generic / Acrylic / HDPE / Delrin / Polycarbonate)
to 11:

- `UhmwPe` — Shore D 66 (ASTM D2240), Mitsubishi TIVAR 1000
- `Polypropylene` — Shore D 70 (ASTM D2240 + ISO 868), SIMONA PP-H +
  Direct Plastics PP-H two-source corroborated
- `Nylon66` — Shore D 85 (ASTM D2240), Mitsubishi Nylatron GS
  (MoS2-filled cast machinable grade; also reports Rockwell M 85 /
  Rockwell R 115 on the same datasheet)
- `Abs` — Rockwell R 105 (MakeItFrom range 100-110 midpoint), ASTM
  D785 implied
- `Petg` — Rockwell R 115 (ASTM D-785), Plaskolite VIVAK Sheet
- `RigidPvc` — Shore D 74 (scale-only, ASTM D2240 implicit),
  Interstate Advanced Materials Type 1 sheet (D-1784 class 12454-B)

All values read verbatim from a fetched datasheet — see
`planning/data_ingest_2026-05-30/hardness_extra.md` H.1 for
per-grade verbatim quotes and caveats. None of the new families
have a fetched milling-regime Kc, so all six return
`kc_n_per_mm2() = None` per the refusal-first contract; the
`literature_parity::plastics_without_primary_source_refuse_kc`
sentry now exercises all 10 None families. Six new
`literature_parity` sentries pin each new hardness value to its
citation. See `planning/feeds_data_ingest_phaseD_2026-05-31.md`.

2026-05-31 Phase E (completion plan) added a parametric
`Material::SolidWoodByJanka { janka_lbf, label, source_id }` variant
plus a curated `WOOD_SPECIES_LIBRARY` const (132 entries) covering
wood species beyond the 10 first-class `WoodSpecies` enum variants:

- 98 entries from USDA Forest Service, *Wood Handbook — Wood as an
  Engineering Material* (GTR FPL-GTR-190, 2010), Chapter 5
  "Mechanical Properties of Wood", Table 5-3a. Side hardness
  converted from the published Newton values to lbf via
  `lbf = N / 4.448`.
- 34 entries from The Wood Database (https://www.wood-database.com/)
  per-species Janka pages, used as fill-in for species FPL doesn't
  cover (typically tropical / specialty woods). FPL precedence
  when both sources list the same common name.

Two `source_id` entries added to `source_manifest.json`
(`fpl_ch5_2010`, `wood_database_2026-05-30`). The library is wired
through a shared `janka_to_kc_n_per_mm2(janka_lbf)` fallback helper
calibrated on `[200, 4000]` lbf — outside the band the helper
refuses and the parametric variant's Kc gate refuses cleanly. The
first-class `WoodSpecies` enum variants keep their hand-tuned per-
species Kc constants (no behavior change). New literature_parity
sentries pin the helper's output to the LongleafPine enum anchor
within ±30 % folklore tolerance and to the calibrated-band
refusal contract. See `planning/feeds_data_ingest_phaseE_2026-05-31.md`.

### Acceptance benchmark seed sources

`planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md` and the seed matrix in
`planning/toolpath_acceptance/` add benchmark/acceptance targets from the
bundled vendor LUT plus these external reference and community sources:

- Cutter Shop chip-load chart: <https://cutter-shop.com/chip-load-chart/>
- IDC Woodcraft chipload calculator/chart: <https://idcwoodcraft.com/pages/chipload-calculator>
- Carbide3D community feeds/speeds discussion: <https://community.carbide3d.com/t/feeds-and-speeds-guide/17048>
- CutViewer ball-nose stepover/cusp calculator: <https://cutviewer.com/tools/stepover-calculator/>
- CNC Cookbook deep-hole drilling reference: <https://www.cnccookbook.com/deep-hole-drilling/>

These sources are used as acceptance and sweep-planning benchmarks, not as
new bundled runtime LUT data.

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
