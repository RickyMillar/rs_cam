# Phase 5 consolidation report — Schema unlock + per-species Kc wiring

**Date:** 2026-06-01
**Phase 5 plan:** `planning/phase_5_schema_unlock_2026-06-01.md`
**Predecessor (closed):** Phase 4 consolidation at
`planning/feeds_data_ingest_consolidation_2026-06-01.md`.

## Headline

**Phase 5 closes the sim-internal feeds-data workstream.** Four
independently committable steps landed in order:

| Step | Commit | What it did |
|---|---|---|
| 5.1 | `5fc6f4f` | `stock_*` baseline_params → `session.stock_mut()` routing in smoke runner. AS013 deflection 262.9 µm (Exceeds) → 109.0 µm (Within), matches round-09's ~105 µm. |
| 5.2 | `6901795` | `VendorObservation.diameter_mm: f64` → `Option<f64>`. V-bit + diameter-window rows now matchable on angle / material / op. Promoted 4 net-new rows (247 → 251). |
| 5.3 | `6c56b95` | `MaterialFamily::Fiberglass` + `Material::Fiberglass { grade }` lifecycle. Garr GP composite row promoted (251 → 252). |
| 5.4 | `353c9d2` | Per-species wood Kc pinned to FPL-GTR-190 Ch.5 Table 5-3a. Folklore TODO closed; smoke shifts ±7-8%, all Within. |

The four steps together raise the bundled vendor LUT count from
**247 → 252** and shift the canonical smoke baseline twice
(2026-06-02.csv → 2026-06-03.csv after Step 5.1, → 2026-06-04.csv
after Step 5.4). The F-037 sentry was updated at each baseline
shift; `smoke --diff` against each successive baseline reports
"no regressions (18 cases checked)".

The user's framing on opening Phase 5 was: *"schema changes are
cheap, we have no consumers yet."* Phase 5 took that license: each
of the three schema changes (`diameter_mm` Optional, `MaterialFamily::
Fiberglass`, `Material::Fiberglass`) is a breaking type change that
cascaded into a handful of pattern-match sites (15 arms touched for
Fiberglass; one Option pattern for V-bit) and was contained within
the same commit as the row promotion that demanded it.

## What landed live (per step)

### Step 5.1 — AS013 stock_top_z methodology fix

`crates/rs_cam_cli/src/smoke.rs::apply_stock_overrides()` intercepts
any `stock_*` prefixed key in a case's `baseline_params` and routes
it to `session.stock_mut()` instead of the toolpath operation
schema. `stock_top_z=N` sets `stock.z = N - origin_z` and disables
`auto_from_model` so subsequent re-derivation can't undo it. The
per-toolpath `set_toolpath_param` loop now skips `stock_*` keys
so they don't pollute param_warnings with "unknown parameter" noise.

Result: AS013 (3D adaptive3d on terrain_small.stl with stock top
at Z=30) deflection drops 262.9 µm Exceeds → 109.0 µm Within. The
delta is within 4 µm of round-09 STATE.md's operator-validated
~105 µm — residual sits within dexel-resolution noise on the
fixture.

### Step 5.2 — V-bit schema relaxation (Optional diameter_mm)

`crates/rs_cam_core/src/feeds/vendor_lut.rs::VendorObservation::
diameter_mm` is now `Option<f64>`. The matcher's `passes_must_match`,
`score_observation`, `build_result`, and `diameter_scale_factor`
paths all handle the `None` case: anchorless rows skip the diameter
ratio gate, contribute 0 to the diameter-proximity score, and
return `chipload_diameter_scale = 1.0` (no extrapolation). The GUI
matched-rows table renders `—` for diameter on anchorless rows.

4 net-new rows promoted after content-aware dedup vs the existing
`amana-vgroove-*` series (which already carries the same source
data under synthetic `diameter_mm = 6.35`):
- `amana-engrave-softwood-trace-30deg-1f`
- `amana-engrave-hardwood-trace-30deg-1f`
- `amana-engrave-softwood-trace-45deg-1f`
- `onsrud-article-polycarbonate-optimum-chipload-window`

The other 11 Amana v-bit rows staged in Phase 4 (AMS-159 series at
18°/30°/45°/60°/90° in softwood/hardwood/acrylic/aluminum) were
identified as data-equivalent to live `amana-vgroove-*` rows
(verifier-confirmed identical chiploads). They were NOT re-promoted
under the `amana-vbit-*` naming because that would duplicate the
live LUT without adding information; the live rows' synthetic
6.35 mm diameter is harmless to the matcher under the angle-aware
dispatcher.

New source_id added: `onsrud_routing_polycarbonate_article`.

Two focused matcher tests pinned the new code paths:
- `vbit_matches_angle_only_row_without_diameter`
- `diameter_window_row_matches_flat_query_no_extrapolation`

### Step 5.3 — Fiberglass MaterialFamily + Material variant

`MaterialFamily::Fiberglass` added to `vendor_lut.rs` with its own
`material_category` (3) — fiberglass queries never extrapolate
onto polymer or aluminum rows. `Material::Fiberglass { grade }`
added with `FiberglassGrade::{G10Fr4, Generic}`. `MaterialCategory::
Composite` added to the GUI picker hierarchy. 15 match-arm sites
touched in `material.rs` + 2 in `vendor_normalize.rs` + 2 in viz
labels (one in tracked `properties/mod.rs`, one in untracked WIP
`feeds_modal.rs`).

Fiberglass `kc_n_per_mm2()` returns `None` (refuse-first — no
fetched primary measurement; G10 handbook quotes 100–150 N/mm² but
no workshop-fidelity peripheral milling data). Other accessors
carry conservative carbide-tool placeholders pending bench
validation: `feed_scale_factor=1.3`, `base_cutting_speed_m_min=120`,
`drill_chip_welding_threshold_dtd=3.0`, `drill_per_peck_max_dtd=1.0`,
`drill_plunge_feed_envelope_per_mm=(40, 200)`.

1 vendor row promoted: `garr-gp-fiberglass-6000-2f-flat`. New file
`crates/rs_cam_core/data/vendor_lut/observations/garr_fiberglass.json`
added to the embedded LUT include list. Source manifest annotation
updated to reflect the schema-unlock path.

Focused matcher test:
`fiberglass_category_does_not_extrapolate_to_polymer_or_metal`.

### Step 5.4 — Per-species wood Kc from FPL Ch.5

`Material::SolidWood::kc_n_per_mm2()` per-species values now pinned
to FPL-GTR-190 Ch.5 Table 5-3a shear-parallel-to-grain (12% MC)
rows. Folklore TODO closed.

Shift table (folklore → FPL-cited):

| Species | Old | New | Δ % |
|---|---:|---:|---:|
| GenericSoftwood | 6.0 | 6.5 | +8% |
| LongleafPine | 7.0 | 10.4 | +48% |
| GenericHardwood | 14.0 | 13.0 | -7% |
| HardMaple | 15.0 | 16.0 | +7% |
| Walnut | 12.0 | 9.5 | -21% |
| Birch | 13.0 | 13.0 | 0% (FPL match) |
| WhiteOak | 14.0 | 13.8 | -1% |

Three species not in FPL Ch.5 (RadiataPine, Jarrah, Ipe) retain
folklore values with TODO markers pointing at CSIRO / EMBRAPA /
IPT as future sources.

**Scope deferral:** Values stay in the FPL shear-parallel regime
(6–28 N/mm²). A true peripheral-milling Kc would multiply by a
3–5× edge-radius size-effect factor — but applying it requires a
coordinated anisotropy-factor retune (analog to Phase 2B sheet
goods) and bench validation. That work is **Phase 6+ scope**.
Step 5.4 buys: citation chain ends at a specific FPL row with
verbatim quote.

Smoke deflection shifts (all within ±8%, no within→exceeds):
| Case | Pre (µm) | Post (µm) | Δ |
|---|---:|---:|---:|
| AS001 hardwood pocket    | 76.1  | 70.6  | -7% |
| AS002 softwood adaptive  | 50.5  | 54.7  | +8% |
| AS003 hardwood profile   | 84.9  | 78.8  | -7% |
| AS013 softwood adaptive3d| 109.0 | 118.1 | +8% |
| AS015 hardwood scallop   | 129.7 | 120.4 | -7% |

Two lib-level tests pinned to the folklore Kc were adjusted (see
the Step 5.4 commit message + 2026-06-04 baseline notes for the
specifics).

## Exit gate verification

| Criterion | Status |
|-----------|--------|
| `cargo test -p rs_cam_core --lib` | ✅ 1672 passed; 0 failed |
| `cargo test -p rs_cam_core --tests` | ✅ all green |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ clean across core / cli / viz / mcp |
| `cargo run -p rs_cam_cli -- smoke --diff --baseline 2026-06-04.csv --output <fresh>` | ✅ no regressions (18 cases) |
| F-037 sentry points at the post-5.4 baseline | ✅ `crates/rs_cam_core/tests/smoke_baseline_regression_f037.rs::baseline_path()` → `2026-06-04.csv` |

## Baselines on disk after Phase 5

| File | Trigger | Status |
|---|---|---|
| `2026-05-26.csv` | Phase F (informal-promotion close) | historical |
| `2026-05-31_postphasef.csv` | Phase F post-fix | historical |
| `2026-06-01.csv` | early Phase 4 work | historical |
| `2026-06-02.csv` | Phase 4 close (AS013/AS015 `prior_passes`) | historical |
| `2026-06-03.csv` | Phase 5 Step 5.1 (AS013 `stock_top_z`) | historical |
| `2026-06-04.csv` | **Phase 5 Step 5.4 (wood Kc retune)** — **current** | **live** |

Older baselines stay on disk as references for the per-step
deflection deltas; only the latest is wired into F-037.

## Closed `_gaps.md` items

These three deferral notes can now be marked CLOSED for their
Phase 5 entries (the underlying schema gap is gone):

- `planning/data_ingest_2026-05-29/amana_gaps.md` — V-bit
  `diameter_mm` gap. Closed by Step 5.2 (`Option<f64>` + matcher
  fallback). The 11 staged-but-not-promoted Amana v-bit rows
  remain documented as "data-equivalent to live `amana-vgroove-*`
  rows; not re-promoted to avoid duplication".
- `planning/data_ingest_2026-05-29/onsrud_whiteside_gaps.md` —
  Diameter-window row gap. Closed by Step 5.2 (the Onsrud
  polycarbonate article row promoted live).
- `planning/data_ingest_2026-05-29/harvey_helical_garr_gaps.md` —
  `MaterialFamily::Fiberglass` gap. Closed by Step 5.3 (enum
  variant + Material lifecycle + row promotion).

The `material.rs:660-680` "Phase 3 TODO" wood-Kc-folklore marker
is closed by Step 5.4; replacement TODOs in that block now point
at "Phase 6+: absolute-Kc calibration via shear × size-effect" as
the next-tier work.

## Open gaps after Phase 5

The schema-unlock + sim-internal data wiring is now substantially
complete. The remaining items are all Phase 6+ scope:

1. **Absolute-Kc calibration for solid wood.** Multiply per-species
   Kc by a 3–5× peripheral-milling size-effect factor + retune
   anisotropy. Requires bench validation. Operator-bound.
2. **`SolidWoodByJanka` parametric Kc derivation.** Currently uses
   `janka_lbf / 100.0` (same shear-magnitude folklore as the
   per-species values pre-5.4). Replace with per-species FPL-shear
   vs Janka regression analysis once the absolute-Kc effort lands.
3. **Vendor breadth round 3.** Kennametal / Sandvik / Vortex were
   Phase 3 stretch beats that nobody collected. Tracked at
   `planning/data_ingest_2026-05-30/vendor_breadth_gaps.md`.
4. **F-033 workflow advisory.** Pre-sim warning for finishing op
   without prior rough. Was explicitly OUT OF SCOPE for Phase 5
   per the user's framing.
5. **Bench validation against real cuts.** Operator-bound. Phase 5
   closed the sim-internal honest-data gap; only field cuts close
   the sim-vs-reality gap. When the user runs a real toolpath and
   reports back, deltas vs sim prediction become Phase 6 findings.
6. **Fiberglass Kc primary measurement.** `Material::Fiberglass::
   kc_n_per_mm2()` returns `None` until a workshop-fidelity G10/FR4
   measurement lands. Other Fiberglass accessors are conservative
   carbide-tool placeholders pending bench validation.
7. **Three wood species without FPL coverage.** RadiataPine,
   Jarrah, Ipe — sources documented in code as CSIRO / EMBRAPA /
   IPT TODOs.

## Phase 5 in one sentence

The four schema-blocked vendor rows + one Garr composite row that
Phase 4 left staged are now live; v-bit / window / fiberglass rows
land via honest schema instead of synthetic anchors; per-species
wood Kc carries an FPL citation chain instead of folklore; and
none of it regresses the smoke baseline — closing the sim-internal
feeds-data workstream and clearing the runway for Phase 6's
absolute-Kc + bench-validation work.
