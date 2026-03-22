# Review: Documentation Drift

## Summary
The most significant documentation drift is that the simulation backend has been rewritten from heightmap to tri-dexel, but architecture docs, README, and CREDITS still reference the old heightmap approach. The tri-dexel design doc exists but isn't indexed in architecture/README.md. Operation counts and feature claims in FEATURE_CATALOG.md appear accurate.

## Findings

### Critical Drift

#### 1. Simulation Backend — architecture/high_level_design.md
- **Claim (lines 112-119):** "Simulation is currently heightmap-based: stock is rasterized to a heightmap, tool motion stamps removal into the grid"
- **Reality:** TriDexelStock is the primary simulation backend (implemented in `dexel_stock.rs`). Viz layer imports and uses `TriDexelStock`, not `Heightmap`. PROGRESS.md describes 6 completed phases of tri-dexel implementation.
- **Impact:** Misleads architecture reviewers about the fundamental simulation data structure

#### 2. Product Description — README.md
- **Claim (line 11):** "Verification: heightmap stock simulation, playback, and holder/shank collision checks"
- **Reality:** Uses TriDexelStock (tri-dexel volumetric grids with Z/X/Y orthogonal dexel rays)
- **Impact:** Product overview contradicts the actual technology stack

### Significant Drift

#### 3. New Core Modules Not Documented — architecture/high_level_design.md
- **Claim:** No mention of tri-dexel modules or semantic/debug tracing
- **Reality:** Six new core modules exist:
  - `dexel_stock.rs` — volumetric stock representation
  - `dexel_mesh.rs` — tri-dexel to renderable mesh conversion
  - `dexel.rs` — core dexel ray/grid primitives
  - `semantic_trace.rs` — semantic tree and trace debugging
  - `debug_trace.rs` — performance tracing
  - `simulation_cut.rs` — simulation-specific cutting logic
- **Evidence:** All exposed in `crates/rs_cam_core/src/lib.rs`

#### 4. Design Doc Not Indexed — architecture/README.md
- **Claim:** Lists only 3 architecture documents (user_stories, requirements, high_level_design)
- **Reality:** `architecture/TRI_DEXEL_SIMULATION.md` exists and is referenced in PROGRESS.md
- **Impact:** Discovery problem — navigating architecture docs won't find the tri-dexel design

#### 5. Tri-Dexel Algorithm Not Attributed — CREDITS.md
- **Claim:** Attributes many algorithms but no mention of tri-dexel
- **Reality:** PROGRESS.md describes 6-phase tri-dexel implementation with novel multi-grid mesh extraction. No provenance or reference sources cited.
- **Impact:** Violates project's stated attribution policy ("keep CREDITS.md current when adding external datasets, formulas, or algorithm references")

### Minor / No Drift

#### 6. Operation Count — architecture/high_level_design.md
- **Claim:** "11 GUI-exposed 2.5D operations, 11 GUI-exposed 3D operations"
- **Reality:** Matches (22 total). Confirmed in FEATURE_CATALOG.md.
- **Status:** Accurate

#### 7. FEATURE_CATALOG.md
- Feature claims appear to match implemented code
- Operation list matches actual operation modules

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Simulation described as "heightmap-based" — actually tri-dexel | architecture/high_level_design.md:112-119 |
| 2 | High | README says "heightmap stock simulation" — actually tri-dexel | README.md:11 |
| 3 | Med | 6 new core modules not documented in architecture | architecture/high_level_design.md |
| 4 | Med | TRI_DEXEL_SIMULATION.md not indexed in architecture/README.md | architecture/README.md |
| 5 | Med | Tri-dexel algorithm not attributed in CREDITS.md | CREDITS.md |

## Test Gaps
- No automated check that documentation claims match code (could add a simple CI script)

## Suggestions

### High Priority
1. **Update `architecture/high_level_design.md`** — Replace heightmap description with tri-dexel architecture in "Simulation and collision" section
2. **Update `README.md`** line 11 — Reference tri-dexel instead of heightmap

### Medium Priority
3. **Update `architecture/README.md`** — Add TRI_DEXEL_SIMULATION.md to the Documents table
4. **Add tri-dexel entry to `CREDITS.md`** — Cite algorithm references and implementation module locations
5. **Add "New Modules" subsection** to high_level_design.md describing dexel, semantic_trace, and debug_trace infrastructure
