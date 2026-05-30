# Feeds/Speeds Data Ingest Consolidation — 2026-05-30 (Phase 1 + 2B + 2C)

This document records Phase 1 of the
`planning/feeds_data_ingest_2026-05-30_phased_plan.md` execution
plus the architectural-correctness Phase 2 work pulled forward into
the same autonomous session.

Phase 1 + 2B + 2C ran in a single-owner sequential pass (per the
plan's "Why sequential"). All steps completed; lib + integration
tests + clippy stay green. Phase 2B's MCP smoke validation
(AS001–AS015) is a separate operator action item — see the
`kc_retune_log.md` "Smoke-test coverage NOT executed this round"
section.

## Step-by-step record

### Step 1A.0 — `Material::kc_n_per_mm2() -> Option<f64>`

Refactored the cutting-force accessor so the type system encodes
"this material may not have a validated Kc". All call sites updated:

- `crates/rs_cam_core/src/tool_load/power.rs` — `Some(kc) else refuse`
- `crates/rs_cam_core/src/tool_load/deflection.rs` — propagates via `?`
  in `sample_tip_deflection_mm`; `evaluate()` refuses on `None`
- `crates/rs_cam_core/src/session/compute.rs` — `DeflectionLimitInputs`
  / `PowerLimitInputs` construction is gated on `Some(kc)`; outer-`Option`
  pattern preserved (no inner-field Option churn)
- `crates/rs_cam_core/src/feeds/mod.rs` — power-check ramp short-circuits
  when Kc is None; `actual_power` reports 0 in that arm
- `crates/rs_cam_viz/src/ui/properties/stock.rs` — displays "—" for None

Custom semantics preserved: a Custom material with positive-finite Kc
returns `Some(kc)`; bad inputs (NaN, ≤0) return `None`. Behaviour at
the gates is identical to the old `kc.is_finite() && kc > 0.0` path —
no regression for projects relying on Custom.

### Step 1A — Per-family plastics Kc + Shore-D hardness

- HDPE → `Some(40.0)` — Yang 2022 midpoint of measured cutting-yield
  stress range 33.85–46.89 N/mm² (DOI 10.3390/polym14010189).
- Polycarbonate / Acrylic / Delrin / Generic → `None` — no fetched
  primary force study exists. The gate refuses cleanly; the historical
  generic `4.0` constant is gone.
- New `PlasticHardness::{ShoreD, RockwellM}` enum preserves the
  original measurement scale rather than fabricating a cross-scale
  equivalence (PMMA stays in Rockwell M).

Regression tests added: `plastic_kc_only_some_when_validated`,
`plastic_hardness_preserves_scale`, and a power-gate
`plastic_without_validated_kc_refuses_material_unvalidated` smoke.

### Step 1B — `Material::Aluminum { alloy }` variant

New `AluminumAlloy::{Alloy6061T6, Alloy7075T6}` with ASM/MatWeb
Brinell values (95 / 150). `kc_n_per_mm2()` returns `None` —
power/deflection gates refuse with `MaterialUnvalidated` until Phase 3
beat F lands a fetched-primary `kc1.1 + mc` Kienzle pair.

Exhaustive match sites extended:
- `material.rs` (hardness_index, kc, base_cutting_speed, plunge_rate,
  label, catalog, to_key, from_key)
- `feeds/vendor_normalize.rs::material_to_lut` (→ MaterialFamily::Aluminum,
  HardnessKind::Hb, brinell_hb)
- `drill_metrics.rs` (chip_welding_threshold 3.0, per_peck 1.0)
- `tool_load/drill_gates.rs::plunge_feed_envelope` (40..250 1/min)
- `rs_cam_cli/src/smoke.rs::material_for_family` ("aluminum"/"6061"/"7075")

### Step 1C — Promote 40 staged non-wood LUT rows

Bundled live (`crates/rs_cam_core/data/vendor_lut/observations/`):

| File | Rows | Source |
|------|------|--------|
| `onsrud_plastic.json` | 9 | Onsrud chipload sheets (HDPE/acrylic/PC/Delrin, 6.35 & 12.7 mm) |
| `whiteside_rpm_assorted.json` | 4 | Whiteside 1540/1550 V-groove + RU4000H/RD5218H spirals (RPM-only) |
| `amana_plastic_oflute.json` | 5 | Amana Plastic O-Flute chart |
| `amana_zrn_aluminum.json` | 2 | Amana ZrN Aluminum O-Flute chart |
| `amana_vgroove_aluminum_acrylic.json` | 3 | AMS-159 acrylic 30°/aluminum 45° + Spektra acrylic 45°, diameters set 6.35 mm from verifier report |
| `helical_aluminum.json` | 2 | Helical H45AL 6061 (12.7 mm 3-flute, pass_role normalised "rough"→"roughing") |
| `amana_compression.json` (+1) | 1 added | acrylic compression spiral 6.35 mm |

Total newly live: **26 rows** (1A.0 raised the embedded count from 85
to 111).

**Deliberately deferred to a later round:**
- **Garr aluminum (10 rows)** — staged file lumps the 242M (2-flute) /
  842M (2-flute) / A3 (3-flute) series under one row per chart entry,
  but the chart shares CPT across the series while flute count differs
  per series. Promoting as-is would store an unknown flute count.
  Action item for Phase 3 beat A: split per-series, fetch true flute
  count per Garr PDF.
- **`onsrud-article-polycarbonate-optimum-chipload-window`** — narrative
  datapoint, no diameter; stays as a provenance-only note.

### Step 1D — `Vendor::Helical`

Single enum variant added with serde `snake_case` default ("helical").
Enables `helical_aluminum.json` to deserialize.

### Step 1E — DOC-derating in the chipload gate

Implemented `doc_derating_scale(ratio)` in `tool_load/chipload.rs`:
piecewise linear scale per the verbatim Amana + Onsrud rule. Applied
once at bounds-extraction using the toolpath peak axial DOC over the
cutter's `lookup_diameter_at(peak_doc)`. The reported `ChipBounds`
reflect the actual trip — no hidden per-sample fudge.

Four unit tests pin the scale: below 1×D (1.0), at 2×D (0.75), at 3×D
(0.5), and clamp beyond 3×D (still 0.5).

## Phase 1 exit gate (held)

- `cargo test -p rs_cam_core --lib` → 0 failed
- `cargo test -p rs_cam_core --tests` → 0 failed (recorded under
  `/tmp/itest3.log`)
- `cargo clippy -p rs_cam_core --all-targets -- -D warnings` → clean
- `validate_lut.py` on `planning/data_ingest_2026-05-29/` → only
  remaining PROBLEMS are the Garr deferral set (flute_count + pass_role)
  + the narrative polycarbonate window row (documented in this
  consolidation)

## Step 2C — Janka data fixes + species/trade-group rename (bonus)

Pulled forward into this round because the rename is data-model
correctness, not Kc physics — completely isolated from the Phase 2B
Kc re-tune.

- `WoodSpecies::SouthernYellowPine` → `WoodSpecies::LongleafPine`
  (Janka 870 lbf, Wood Database verbatim quote). SYP is a trade
  group covering Longleaf/Loblolly/Shortleaf/Slash pines, each with
  different Janka; carrying a trade-group name with a guess at which
  species was the wrong data model.
- `RadiataPine` Janka updated 500 → 710 lbf (Wood Database).
- `Material::from_key` adds a legacy alias arm:
  `"southern_yellow_pine"` → `LongleafPine`. Write path emits only
  `"longleaf_pine"`. Standard schema-evolution discipline.

Regression tests added:
`legacy_southern_yellow_pine_key_aliases_to_longleaf`,
`longleaf_pine_janka_matches_wood_database`,
`radiata_pine_janka_matches_wood_database`.

`Material::kc_n_per_mm2()` for `LongleafPine` keeps its previous
value (7.0 N/mm²) until Phase 2B revisits all wood Kc values
systematically. The rename does not change any Kc-derived prediction.

## Step 2B — Calibrated Kc / grain-anisotropy re-tune (applied)

The Phase 2 plan's main physics fix, applied in this same autonomous
session. Detailed before/after in
`planning/data_ingest_2026-05-30/kc_retune_log.md`.

- **Rename + value:** `tool_load/power.rs::ANISOTROPY_MULTIPLIER` →
  `GRAIN_ANISOTROPY_FACTOR`, value 2.5 → 2.0. Citation: Pałubicki
  2021, *Materials* 14(9):2208, DOI 10.3390/ma14092208 — measured
  directional spread for particleboard peripheral up-milling.
  Mirror constants in `session/compute.rs` and `feed_modulation.rs`
  comment updated to match.
- **Sheet-good Kc:** moved to measured literature.
  `SheetGood::Particleboard` 9.0 → 35.0 (Pałubicki midpoint);
  `SheetGood::Mdf` 10.0 → 31.4 (PMC6315737 round-shape Ks);
  `SheetGood::Hdf` 12.0 → 36.8 (derived MDF × density ratio, TODO
  Phase 3 for direct measurement). Combined `Kc × factor` product
  for sheet goods now 2.5–3.1× the pre-Phase-2B value — closer to
  the measured peripheral-milling cutting forces and a major safety
  win on plastic-laminate / particleboard jobs.
- **Solid wood + plywood Kc unchanged with TODO Phase 3:** no
  fetched per-species direct Kc measurement exists; current values
  track shear-parallel-to-grain. Phase 3 beat C (FPL Ch.5 systematic
  extract) will inform the per-species derivation. The 2.0 × factor
  alone drops the combined product 20 % on these materials — a
  small step toward physical correctness.
- **Deflection bounds unchanged.** `WITHIN_BOUND_MM = 0.050` /
  `EXCEEDS_BOUND_MM = 0.200` are documented physical safety
  thresholds, NOT calibration knobs. If MCP smoke shows
  operator-known-good cuts flipping past 200 µm, that's a
  deflection-model finding to file (force arm, near-tip
  integration), NOT a reason to widen the bound.

### Test fixtures updated to match the new physics

- `test_sheet_good_kc_progression` → renamed
  `test_sheet_good_kc_in_measured_literature_band`. Old `mdf >
  particle` assertion doesn't survive the measured ordering
  (Pałubicki PB > PMC MDF). New: band check + HDF on top.
- `light_cut_is_within_with_available_kw` hand-compute comment:
  `Kc_eff` 37.5 → 30.0; predicted `P` 0.00198 → 0.00159 kW. Still
  Within.
- `vbit_triangular_cross_section_halves_power_vs_flat` expected
  literal: `37.5 * 0.5 * ...` → `30.0 * 0.5 * ...`. Test passes.
- `heavy_cut_exceeds_machine_with_available_kw` — no change.
  Verified by hand: Ipe `Kc_eff` 56, P = 0.711 kW vs 0.568 kW
  available → still Exceeds.
- `wanaka_*` deflection tests — deflection uses RAW Kc with no
  grain factor, so unchanged.

### MCP smoke (AS001–AS015) — operator action item

The MCP smoke suite needs the live GUI + MCP server and was NOT run
in this autonomous session. Run it manually with the same
`load_project` + `generate_all` + `run_simulation` recipe used in
the closed acceptance loop (`planning/acceptance_loop/STATE.md`
round-10). Record per-case before/after peak µm in
`kc_retune_log.md`. **Verdict-flip rule** (from the Phase 2 abort
criteria): flips that match real-world operator experience are
kept; flips that don't are filed as deflection-model investigations
— do NOT silently widen `WITHIN_BOUND_MM` / `EXCEEDS_BOUND_MM` or
back off the Phase 2B values.

## Carry-forward to Phase 3 + onward

Phase 1 + 2B + 2C is the full Phase 2 lift this session can
execute autonomously. Remaining work (per the plan):

- **MCP smoke validation** (operator action above).
- **Phase 3 — Parallel data-collection fleet** (8 agents: Amana long
  tail, Wood Database species sweep, FPL Ch.5 systematic extract,
  Onsrud OCR specialist, plastics Kc archival, aluminum Kienzle
  hunt, vendor breadth, hardness fan-out). Not started in this
  session; the plan's per-beat agent prompt skeleton is at
  `feeds_data_ingest_2026-05-30_phased_plan.md` Appendix C.
- **Phase 4 — Verification + promotion gate** for whatever Phase 3
  produces.

## Files changed

Production:
- `crates/rs_cam_core/src/material.rs` (new AluminumAlloy, new
  PlasticHardness, kc_n_per_mm2 → Option, exhaustive matches)
- `crates/rs_cam_core/src/tool_load/{power,deflection,chipload,drill_gates}.rs`
- `crates/rs_cam_core/src/drill_metrics.rs`
- `crates/rs_cam_core/src/feeds/{vendor_lut,vendor_normalize,mod}.rs`
- `crates/rs_cam_core/src/session/compute.rs`
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs` (test bumped 100 →
  200 mm — see note)
- `crates/rs_cam_core/tests/vendor_lut_sub_1mm.rs` (count assertion)
- `crates/rs_cam_viz/src/ui/properties/stock.rs`
- `crates/rs_cam_cli/src/smoke.rs`

Data (new files):
- `crates/rs_cam_core/data/vendor_lut/observations/{onsrud_plastic,
  whiteside_rpm_assorted, amana_plastic_oflute, amana_zrn_aluminum,
  amana_vgroove_aluminum_acrylic, helical_aluminum}.json`

Tooling:
- `planning/data_ingest_2026-05-30/validate_lut.py`

## Sensitivity finding (Step 1C)

The `test_no_match_returns_none_when_outside_sanity_floor` test
shadowed by the new Whiteside RD5218H 12.7 mm hardwood row: the prior
100 mm probe (100/12.7 = 7.87×) now sits inside the 10× sanity floor
because the largest hardwood flat-end adaptive-roughing row grew. Probe
bumped to 200 mm to preserve the test's intent (>10× ratio).

This is exactly the "more data shadows old test probes" sensitivity
behaviour the plan flagged. No code change to the sanity floor itself.
