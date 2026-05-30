# Feeds/Speeds & Tool-Load Data Ingest — Phased Implementation Plan

Authored 2026-05-30 to be executed after a context clear. This document is
deliberately self-contained: a fresh Claude session with no prior conversation
must be able to read it and execute every phase from the doc + the repo alone.

## Quick orientation (read this first)

`rs_cam` is a Rust CAM program for a 3-axis wood router (Shapeoko Pro XXL). It
has a vendor-seeded feeds/speeds LUT that drives SAFETY-CRITICAL tool-load
gates (chipload / power / deflection). The closed acceptance loop currently
verifies 7/7 Within on a smoke suite (`CLAUDE.md`).

This plan ingests additional data (mostly already collected, some still to
collect) and the code changes needed to make that data reachable, in safety
order. **The order matters.** Doing collection first piles up inert JSON
because the code blockers aren't lifted; doing Kc re-tune first without the
absorption levers breaks the calibrated acceptance loop. The phases below are
sequenced specifically so each one unlocks the next.

### Hard rules (apply to every phase, every agent, every commit)

These rules are non-negotiable and exist because the LUT/Kc feed safety gates.

1. **Honesty-or-gap on data.** Never invent, estimate, or interpolate a value.
   Record a number only if you can read it from a fetched source AND store a
   verbatim quote alongside. If a source is unreachable / image-only /
   paywalled, log it as a `_gaps.md` entry and move on.
2. **Staging-then-promote.** New data lands in `planning/data_ingest_2026-MM-DD/`
   (a new dated dir per ingest) with `_provenance.md` (quotes) and `_gaps.md`.
   Promotion to `crates/rs_cam_core/data/vendor_lut/observations/` + wiring into
   `embedded()` is a separate, test-gated step.
3. **Never blind-swap calibrated constants.** The repo's Kc values are
   calibrated for the closed acceptance loop. Raising Kc without moving the
   absorption levers (`ANISOTROPY_MULTIPLIER`, `EXCEEDS_BOUND_MM`, tolerance
   bands) together will flip Within→Exceeds and break the loop. Phase 2 is the
   coordinated re-tune; outside Phase 2, do not touch `Material::kc_n_per_mm2()`
   or its consumers.
4. **Test-gated promotion.** Before any new LUT row is in `embedded()`'s
   `include_str!` list: `cargo test -p rs_cam_core --lib` AND
   `cargo test -p rs_cam_core --tests` AND
   `cargo clippy -p rs_cam_core --all-targets -- -D warnings` must all be
   green. The `_f0*` integration tests in `crates/rs_cam_core/tests/` ARE the
   calibration regression net — they cannot regress.
5. **Don't run `cargo test --workspace`** (terminal infinite-loop crash). Use
   `-p rs_cam_core --lib` / `--tests`. Don't run cargo test during a release
   build of `rs_cam_viz` (thrashes swap, crashes the PC) — check `pgrep -af
   "cargo --release|rs_cam_gui"` first.
6. **Zero-warning clippy.** The workspace denies 16 lints. Test code is exempt
   via `#[allow(...)]` at the test module. See `CLAUDE.md` lint policy.
7. **No `cargo fmt` across the workspace.** Rustfmt cascades and rewrites
   sibling modules; rely on clippy. Format only files you authored.

### What's already in place (do NOT re-do)

This plan is the **follow-on** to the 2026-05-29 ingest, which already
delivered:

- 18 wood LUT rows live in the embedded LUT (now 85 total observations).
  Files: `crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json`
  (13 chamfer_vbit angle-keyed rows, 15°–120°) and `amana_compression.json`
  (5 flat-end compression-spiral rows).
- The `find_best_vbit_row` angle-aware matcher + `included_angle_deg` /
  `tip_diameter_mm` LUT schema fields (see
  `planning/tool_diagnostics_generic_plan.md`).
- 40 non-wood LUT rows staged (validated, with provenance) at
  `planning/data_ingest_2026-05-29/{amana,onsrud_whiteside,harvey_helical_garr}.json`,
  intentionally NOT in embedded() — inert until Phase 1 lands.
- Kc + hardness research data at
  `planning/data_ingest_2026-05-29/{kc,hardness}.md` with verbatim quotes.
- The consolidation/decision record at
  `planning/feeds_data_ingest_consolidation_2026-05-29.md`.
- CREDITS.md + `source_manifest.json` updated for the 3 bundled charts.

If you re-collect Amana V-groove / engraving / compression rows you'll be
duplicating work; the staged plastics/aluminum rows from those charts are
already collected.

### The starting-state preflight

Before doing anything, the executing session should confirm the repo is in the
expected state:

```bash
# 1. Embedded LUT count = 85 (the 67+18 baseline).
cargo test -p rs_cam_core --lib feeds::vendor_lut::tests::test_embedded_loads_all_observations 2>&1 | tail -3
# Expect: test result: ok. 1 passed; 0 failed

# 2. Acceptance regressions green.
cargo test -p rs_cam_core --tests --test dexel_stock_z_frame_f024 --test dexel_stock_z_frame_f026 --test face_stock_top_frame_f028 --test adaptive3d_planner_stock_xy_f027 2>&1 | grep "test result"
# Expect: every line "0 failed"

# 3. The staging directory exists.
ls planning/data_ingest_2026-05-29/*.json
# Expect: amana.json, onsrud_whiteside.json, harvey_helical_garr.json (plus kc.md, hardness.md, *_provenance.md, *_gaps.md)

# 4. validate_lut.py output (script in Appendix A; write it to the new ingest dir).
python3 planning/data_ingest_2026-05-29/validate_lut.py 2>&1 | tail -5
# Expect: TOTAL rows: 58 ... 32 PROBLEMS (known) ... USABLE NOW: 5
```

If any preflight check fails, STOP and investigate — the plan assumes this
baseline.

## Validation infrastructure

### The schema validator

Appendix A contains `validate_lut.py`. **Before starting Phase 3**, copy/save
it to the dated ingest dir you're using (e.g.
`planning/data_ingest_2026-06-XX/validate_lut.py`) and update the field list
if `VendorObservation` in `crates/rs_cam_core/src/feeds/vendor_lut.rs` has
changed.

The validator checks every staged JSON row for: required fields present
(non-`Option` `VendorObservation` fields), enum membership (vendor, grades,
families, etc.), uniqueness of `observation_id`, range sanity on chipload,
and prints a per-row "usable now / needs fix / staged" decision.

### The required-field schema reference

A `VendorObservation` requires (non-`Option`, no `#[serde(default)]`):
`observation_id`, `source_id`, `source_vendor`, `source_title`, `source_url`,
`accessed_on`, `evidence_grade`, `row_kind`, `tool_family`, `operation_family`,
`pass_role`, `material_family`, `material_label`, `diameter_mm`, `flute_count`.

Optional: `tool_subfamily`, `hardness_kind`, `hardness_value`,
`rpm_min/rpm_max/rpm_nominal`, `chipload_min_mm_tooth/chipload_max_mm_tooth`,
`ap_min_mm/ap_max_mm`, `ae_min_mm/ae_max_mm`, `ap_rule/ae_rule`,
`machine_assumption`, `source_page`, `included_angle_deg`, `tip_diameter_mm`.

Enum values:
- `source_vendor`: `amana|onsrud|harvey|whiteside|sandvik|garr|autodesk|carbide3d`
  (Phase 1D adds `helical`; future may add `kennametal|freud|vortex`)
- `evidence_grade`: `a|b|c`
- `row_kind`: `exact|derived|fallback`
- `tool_family`: `flat_end|ball_nose|tapered_ball_nose|bull_nose|chamfer_vbit|facing_bit`
- `operation_family`: `adaptive|pocket|contour|parallel|scallop|trace|face`
- `pass_role`: `roughing|semi_finish|finish`
- `material_family`: `softwood|hardwood|plywood_softwood|plywood_hardwood|mdf|hdf|particleboard|acrylic|hdpe|polycarbonate|delrin|aluminum`

### The acceptance regression sentries (Phase 2 + Phase 4 gate)

These integration tests pin Kc-sensitive behavior. They must stay green:
- `crates/rs_cam_core/tests/dexel_stock_z_frame_f024.rs` (axial DOC + deflection)
- `crates/rs_cam_core/tests/dexel_stock_z_frame_f026.rs` (stock bbox)
- `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs` (collisions)
- `crates/rs_cam_core/tests/face_stock_top_frame_f028.rs` (face deflection)
- (F-031 lives elsewhere; grep for it under `tests/`)

Plus the closed acceptance smoke suite (`planning/acceptance_loop/STATE.md`,
round-10) — AS001/002/003/004/005/013/015, run via MCP load+sim of
`test_data/ux_*.toml` project files. The MCP loop is heavyweight; the cargo
tests are the day-to-day gate.

---

# PHASE 1 — Unblock (sequential, single owner, NOT parallel)

**Goal:** Activate the ~40 already-collected non-wood LUT rows + the staged
plastics/aluminum Kc + hardness data + add the cross-vendor-confirmed DOC
derating. After Phase 1, Phase 3 collectors are productive (the data they
collect goes live) instead of staging-only.

**Estimated effort:** ~300 LOC across ~8 production files + test updates.
Mostly mechanical match-exhaustiveness work; the load-bearing design call is
Step 1A.0 — making `kc_n_per_mm2()` honest about which materials have
validated Kc.

**Why sequential:** Match-exhaustiveness errors cascade across the workspace;
splitting between agents loses context and causes merge friction. One careful
owner with full context is faster and safer.

## Step 1A.0 — Refactor `kc_n_per_mm2() -> Option<f64>` (prerequisite)

**Why this exists.** The current signature `Material::kc_n_per_mm2() -> f64`
lies by being total: it forces every material to have a Kc value, including
ones for which no primary source exists. The repo's response has been to
fabricate (PC/Acrylic/Delrin all return the generic plastic Kc=4.0; Aluminum
will be in the same boat). That's the wrong pattern — the gate already
handles "no validated Kc" cleanly via `UnmodeledReason::MaterialUnvalidated`
for `Material::Custom`. The type system should encode the same.

**The change.** Make Kc an honest `Option<f64>`:

```rust
// crates/rs_cam_core/src/material.rs
impl Material {
    /// Specific cutting force (N/mm²). `None` for materials whose Kc has
    /// no primary measurement — the tool-load gates refuse via
    /// `UnmodeledReason::MaterialUnvalidated` rather than predicting force
    /// from a fabricated constant.
    pub fn kc_n_per_mm2(&self) -> Option<f64> { ... }
}
```

**Call-site updates** (from scoping):
- `crates/rs_cam_core/src/tool_load/power.rs:76` — current
  `let kc = material.kc_n_per_mm2(); if !kc.is_finite() || kc <= 0.0 {
  refuse }` becomes `let Some(kc) = material.kc_n_per_mm2() else {
  return PowerVerdict::Unmodeled { reason:
  UnmodeledReason::MaterialUnvalidated }; };`. The downstream finite-check
  becomes redundant — remove it.
- `crates/rs_cam_core/src/tool_load/deflection.rs:92,140` — same pattern in
  `sample_tip_deflection_mm()` (returns `Option<f64>` already, propagate
  with `?`) and `evaluate()` (early-refuse with `MaterialUnvalidated`).
- `crates/rs_cam_core/src/feed_modulation.rs:154,174` —
  `DeflectionLimitInputs::kc_n_per_mm2` and `PowerLimitInputs::kc_eff_n_per_mm2`
  become `Option<f64>`; the constrained-max solver returns "no Kc-derived
  limit" when None, falling through to the other constraints.
- `crates/rs_cam_core/src/session/compute.rs` — wherever it populates the
  feed-modulation inputs, pass through the Option.
- `crates/rs_cam_core/src/feeds/mod.rs` — if any consumer reads Kc, same
  pattern.

**Material::Custom semantics preserved.** Custom carries a user-typed `kc:
f64`; the new signature returns `Some(kc)` if positive-finite, else `None`.
The gates' existing `MaterialUnvalidated` refusal handles None identically
to the old `kc.is_finite() && kc > 0.0` path — no semantic regression for
Custom users.

**Validation gate (1A.0):**
```bash
cargo build -p rs_cam_core 2>&1 | grep -E "error\[" | head
# Expect: no output (all callers updated for Option).

cargo test -p rs_cam_core --lib tool_load:: 2>&1 | grep "test result" | grep -v "0 failed"
# Expect: no output. All existing Custom-material refusal tests still pass.

cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:"
# Expect: no output.
```

## Step 1A — Per-family plastics Kc + Shore-D hardness

**Current state.** `Material::Plastic { family: PlasticFamily }` already
exists. After Step 1A.0, `kc_n_per_mm2()` returns `Option<f64>`. Today every
plastic family returns the same generic ~4.0 — which is not based on any
measurement. The architectural fix: only return `Some(kc)` for families with
a fetched primary measurement. The others return `None` and the gates
refuse with `MaterialUnvalidated` — honest, and unblocks future correction
without a constant edit.

Per-family decisions (cite source in code comment for each):
- `PlasticFamily::Hdpe` → `Some(40.0)`. Midpoint of Yang 2022's measured
  cutting yield stress range 33.85–46.89 N/mm² (DOI 10.3390/polym14010189).
  Grade B. Reference `planning/data_ingest_2026-05-29/kc.md` line ~111.
- `PlasticFamily::Polycarbonate` → `None`. No primary machining-force study
  exists (documented exhaustively in `kc_gaps.md`).
- `PlasticFamily::Acrylic` → `None`. PMMA Korkmaz 2017 force figure was
  fetched 403; Kc cannot be back-calculated from a fetched source.
- `PlasticFamily::Delrin` → `None`. Trifunović 2021 + Chabbi 2017 both
  paywalled; no fetched primary measurement.
- `PlasticFamily::Generic` → `None`. There is no "generic plastic Kc" in
  any literature; the historical 4.0 was a fabricated baseline.

**Behavior change to document.** This step makes the gates refuse on
plastic toolpaths that previously got a wrong-but-confident Approximate
verdict. That is the correct architectural move (silent wrongness → loud
refusal), and tests asserting plastic verdicts must be updated. Audit:

```bash
rg -l "Material::Plastic|PlasticFamily" crates/rs_cam_core/tests crates/rs_cam_core/src
```

For each test, either (a) switch to a material with Some-Kc (e.g. HDPE) if
the test's intent is to exercise the gate at all, or (b) update the
assertion to `Unmodeled(MaterialUnvalidated)` if the test's intent is plastic
specifically. `tests/drill_material_plumbing_f016.rs` will need this — the
drill gates are independent of `kc_n_per_mm2`, but if any assertion crosses
the chipload/power/deflection gates it'll see the new refusal.

**Per-family hardness.** Add a `PlasticFamily::hardness()` method returning
an enum that preserves units:
```rust
pub enum PlasticHardness {
    ShoreD(f64),
    RockwellM(f64),
}
```
Map to the existing `hardness_index()` floor via documented monotonic
transforms (do NOT fabricate Shore D ↔ Rockwell M equivalences — keep them
as separate scales internally; downstream consumers pick the conversion
appropriate for their use).

Per-family values:
- HDPE → `ShoreD(64.0)` (ISO 868, Direct Plastics).
- Polycarbonate → `ShoreD(80.0)` (ASTM D2240, Treatstock).
- Delrin → `ShoreD(86.0)` (ASTM D2240, Alro POM-H).
- Acrylic → `RockwellM(93.0)` (MakeItFrom PMMA) — NOT Shore D.
- Generic → no hardness (return `None` if the method is `Option`).

**Files to touch:**
- `crates/rs_cam_core/src/material.rs` — `Material::kc_n_per_mm2()` per-
  family inner match; new `PlasticFamily::hardness()`; update
  `hardness_index()` to read per-family.
- Tests in `material.rs` — add per-family Kc + hardness regressions.

**Validation gate (1A):**
```bash
cargo test -p rs_cam_core --lib material:: 2>&1 | grep "test result" | grep -v "0 failed"
# Expect: no output.

# Confirm the architectural intent — PC/Acrylic/Delrin return None:
cargo test -p rs_cam_core --lib material::tests::plastic_kc_only_some_when_validated 2>&1 | tail -3
# (Write this test as part of 1A: asserts HDPE returns Some(40.0±tol),
# PC/Acrylic/Delrin/Generic all return None.)

# Plastic gate now refuses cleanly (write a test or update an existing one):
cargo test -p rs_cam_core --lib tool_load::power::tests::plastic_no_kc_refuses_material_unvalidated 2>&1 | tail -3

cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:"
# Expect: no output.

# Acceptance regressions still green (none use plastics):
cargo test -p rs_cam_core --tests --test dexel_stock_z_frame_f024 \
  --test dexel_stock_z_frame_f026 --test face_stock_top_frame_f028 \
  --test adaptive3d_planner_stock_xy_f027 2>&1 | grep "test result" | grep -v "0 failed"
# Expect: no output.
```

## Step 1B — Add `Material::Aluminum { alloy: AluminumAlloy }` variant

**Goal:** New `Material` variant with two enum members
(`AluminumAlloy::Alloy6061T6`, `AluminumAlloy::Alloy7075T6`), per-alloy
Brinell hardness (95 and 150, both from ASM/MatWeb), and **`kc_n_per_mm2()`
returns `None`** until a fetched primary `kc1.1/mc` Kienzle pair lands
(Phase 3 beat F is hunting for it).

This is the same architectural pattern as Step 1A's plastics-without-primary-
source: refuse-by-default, allow-by-evidence. The power and deflection gates
will report `Unmodeled(MaterialUnvalidated)` for aluminum toolpaths until
Phase 3 lands real data — which is the correct, honest behaviour. The
chipload gate is independent of Kc and works as soon as Step 1C wires the
aluminum LUT rows live.

When Phase 3 beat F succeeds, add the kc1.1/mc pair AND change Aluminum's
`kc_n_per_mm2()` to compute `Some(kc1.1 * h^(-mc))` from a representative
chip thickness (likely 0.1 mm — document the choice). Until then: `None`.

**Files to touch** (from scoping; ~12 exhaustive matches must gain an
Aluminum arm):

Production:
- `crates/rs_cam_core/src/material.rs` — enum + 8 inner `match self {}` arms
  (`hardness_index`, `kc_n_per_mm2`, `base_cutting_speed_m_min`,
  `plunge_rate_base`, `label`, `to_key`, `from_key`, `catalog`)
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:88` — `material_to_lut`
  must route Aluminum to `MaterialFamily::Aluminum` with `HardnessKind::Hb`
  and the alloy's Brinell value.
- `crates/rs_cam_core/src/tool_load/drill_gates.rs:73` — add Aluminum arm to
  `plunge_feed_envelope()` (use a metals-applicable envelope; document the
  source in comments — drill metrics for wood-router-on-aluminum are
  application-edge).
- `crates/rs_cam_core/src/drill_metrics.rs:113,138` — Aluminum arms in
  `chip_welding_threshold` and `per_peck_max_depth_to_diameter` (the audit
  doc says aluminum threshold ~4×D — verify before using).
- `crates/rs_cam_cli/src/smoke.rs:648` — string-parse for "aluminum"/"6061"/"7075".

Tests:
- `crates/rs_cam_core/tests/lookup_parity.rs:244` — test helper replicates
  `material_to_lut`'s match; add Aluminum arm to keep parity.
- `crates/rs_cam_core/src/material.rs::test_catalog_has_all_families` (~line
  620) — add an Aluminum assertion.

**Validation gate (1B):**
```bash
# All exhaustive matches updated (no missing-arm errors):
cargo build -p rs_cam_core 2>&1 | grep -E "error\[E0004\]|non-exhaustive" | head
# Expect: no output.

# Lib + tests green:
cargo test -p rs_cam_core --lib 2>&1 | grep "test result:" | grep -v "0 failed"
# Expect: no output.
cargo test -p rs_cam_core --tests 2>&1 | grep "test result:" | grep -v "0 failed"
# Expect: no output.

# Clippy clean:
cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:" | head
# Expect: no output.

# Aluminum is reachable from the catalog (UI auto-populates):
cargo test -p rs_cam_core --lib material::tests::test_catalog_has_all_families 2>&1 | tail -3
# Expect: 0 failed.

# F-016 still green (existing plastic test):
cargo test -p rs_cam_core --tests --test drill_material_plumbing_f016 2>&1 | tail -3
# Expect: 0 failed.
```

## Step 1C — Promote the 40 staged non-wood LUT rows

**Goal:** Move the staged plastics/aluminum rows from
`planning/data_ingest_2026-05-29/` into
`crates/rs_cam_core/data/vendor_lut/observations/` and wire into `embedded()`.

The staging files are: `amana.json` (some rows already promoted to wood files
during the 2026-05-29 round; the non-wood remainder is acrylic/HDPE/PC plastic
O-flute + ZrN aluminum O-flute + acrylic/aluminum V-groove rows),
`onsrud_whiteside.json` (HDPE/PC/acrylic/Delrin), `harvey_helical_garr.json`
(all aluminum).

**Substeps:**

1. Run the validator one more time to enumerate the staged non-wood rows:
   ```bash
   python3 planning/data_ingest_2026-05-29/validate_lut.py
   ```
   Compare with the "NOT-LIVE (staged) reasons" section of
   `planning/feeds_data_ingest_consolidation_2026-05-29.md` (it should still
   list 19 plastics/aluminum + a few schema-incomplete that must be repaired
   first — see notes in the consolidation doc on Garr `flute_count` /
   `pass_role` and Helical vendor).

2. Repair the schema-incomplete non-wood rows:
   - Garr aluminum rows (`harvey_helical_garr.json`): need real `flute_count`
     (the chart specifies per-series flute counts — fetch from the Garr PDF
     using the `curl -A "Mozilla/5.0" "<url>" -o /tmp/x.pdf && pdftotext
     -layout /tmp/x.pdf -` recipe). Also fix invalid `pass_role` values
     (must be one of `roughing|semi_finish|finish`; the agent had used
     operation tokens like "slot").
   - Helical rows: `source_vendor` is `"helical"` — needs the Vendor enum
     extension in Step 1D before they load. Leave the rows as-is; Step 1D
     enables them.
   - Verify with `validate_lut.py` after each repair.

3. Split into new bundled observation files by source for clean provenance:
   - `crates/rs_cam_core/data/vendor_lut/observations/amana_plastic_oflute.json`
     (acrylic + HDPE + PC, both 3.175 and 6.35 mm)
   - `crates/rs_cam_core/data/vendor_lut/observations/amana_zrn_aluminum.json`
     (3.175 + 6.35 mm)
   - `crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_aluminum_acrylic.json`
     (the acrylic/aluminum V-groove rows the 2026-05-29 round explicitly
     deferred — diameters now defined from the verifier report:
     30°/45°=6.35 mm conical, 60°=12.7 mm, 90°=9.525 mm)
   - `crates/rs_cam_core/data/vendor_lut/observations/onsrud_plastic.json`
   - `crates/rs_cam_core/data/vendor_lut/observations/onsrud_whiteside_assorted.json`
     (the Whiteside RPM-only rows, plus any Onsrud rows that didn't fit the
     pure-plastic file)
   - `crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
   - `crates/rs_cam_core/data/vendor_lut/observations/helical_aluminum.json`
     (after Step 1D)

4. Add each file to the `include_str!` list in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs::VendorLut::embedded()`.

5. Update the count assertion in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs::tests::test_embedded_loads_all_observations`
   (currently 85; raise by the number of rows actually wired) and in
   `crates/rs_cam_core/tests/vendor_lut_sub_1mm.rs::embedded_count_matches_after_expansion`
   (same number — these two assertions live in two files and must stay in
   sync).

**Validation gate (1C):**
```bash
# Schema-clean: no missing-required-field, no bad enum:
python3 planning/data_ingest_2026-05-29/validate_lut.py 2>&1 | grep PROBLEMS
# Expect: "=== PROBLEMS (0) ===" (or a number you can explain — only the original
# "polycarbonate-optimum-window" article row should remain as a non-promotable note row).

# Tests + clippy green:
cargo test -p rs_cam_core --lib 2>&1 | grep "test result:" | grep -v "0 failed"
# Expect: no output.
cargo test -p rs_cam_core --tests 2>&1 | grep "test result:" | grep -v "0 failed"
# Expect: no output.
cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:"
# Expect: no output.

# Embedded row count grew by exactly the number of promoted rows:
cargo test -p rs_cam_core --lib feeds::vendor_lut::tests::test_embedded_loads_all_observations -- --nocapture 2>&1 | grep "85\|test result"
# Expect: the test now expects a higher count and passes.

# The new aluminum query reaches a row:
# (One-off cargo test or small repl — write a test in feeds::vendor_lookup::tests
# that queries Aluminum + flat_end + pocket + roughing and asserts find_best_row returns Some.)
```

## Step 1D — Add `Vendor::Helical` (and any other new vendors)

Trivial enum addition in
`crates/rs_cam_core/src/feeds/vendor_lut.rs::Vendor`. After landing, the 2
staged Helical 6061 rows (from `planning/data_ingest_2026-05-29/harvey_helical_garr.json`)
can serde-deserialize. Promote them as part of Step 1C's `helical_aluminum.json`.

If Phase 3 work (later) yields Kennametal / Freud / Vortex rows, add those
variants then.

**Validation gate (1D):**
```bash
cargo build -p rs_cam_core 2>&1 | grep "error\[" | head
# Expect: no output.

# A Helical row roundtrips through serde:
python3 -c "import json; d=json.load(open('planning/data_ingest_2026-05-29/harvey_helical_garr.json')); print([o['observation_id'] for o in d['observations'] if o['source_vendor']=='helical'])"
# Expect: list of helical observation_ids (currently 2).
```

## Step 1E — DOC-derating rule (cross-vendor-confirmed, data-independent)

Amana AND Onsrud independently state, verbatim:
**"1×D recommended chip load; 2×D reduce 25%; 3×D reduce 50%."**

This is on every collected row's `ap_rule` field. It is a modeling addition,
not a data change.

**Where to apply.** In `crates/rs_cam_core/src/tool_load/chipload.rs::evaluate`,
after the row is matched and bounds extracted, scale `min` and `max` by the
DOC/diameter ratio of the sample's `axial_doc_mm / lookup_diameter_at(doc)`.

A clean formulation:
```
ratio = sample.axial_doc / cutter.lookup_diameter_at(sample.axial_doc)
scale = if ratio <= 1.0 { 1.0 }
        else if ratio <= 2.0 { 1.0 - 0.25 * (ratio - 1.0) }  // linear 1.0→0.75 across 1×→2×
        else if ratio <= 3.0 { 0.75 - 0.25 * (ratio - 2.0) } // linear 0.75→0.50 across 2×→3×
        else { 0.5 }                                          // clamp at 3×+
chipload_bounds *= scale
```

Document the formula source in the function comment (Amana + Onsrud verbatim
quotes, both already in CREDITS/manifest).

**Tests to add** (in `chipload.rs::tests`):
- `doc_derating_unscaled_below_1x_d`: ratio 0.5 → scale 1.0.
- `doc_derating_25pct_at_2x_d`: ratio 2.0 → scale 0.75.
- `doc_derating_50pct_at_3x_d`: ratio 3.0 → scale 0.5.
- `doc_derating_clamps_at_3x_d`: ratio 5.0 → scale 0.5 (not negative).

**Sensitivity caveat.** This narrows the chipload band for deep cuts, which
can shift verdicts. Run the full suite after; if any acceptance test
regresses, the formula or the cutter's `lookup_diameter_at` may need
revisiting, NOT the data.

**Validation gate (1E):**
```bash
cargo test -p rs_cam_core --lib tool_load::chipload 2>&1 | grep "test result"
# Expect: "0 failed".

cargo test -p rs_cam_core --tests 2>&1 | grep "test result:" | grep -v "0 failed"
# Expect: no output.

cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:"
# Expect: no output.
```

## Phase 1 exit gate

All four of these must hold simultaneously before Phase 2 starts:

- [ ] `cargo test -p rs_cam_core --lib` → 0 failed.
- [ ] `cargo test -p rs_cam_core --tests` → 0 failed.
- [ ] `cargo clippy -p rs_cam_core --all-targets -- -D warnings` → no
  warnings/errors.
- [ ] `validate_lut.py` on the 2026-05-29 staging reports the only remaining
  PROBLEM is the `onsrud-article-polycarbonate-optimum-chipload-window` row
  (it's a narrative datapoint, not a LUT row — kept in provenance only).

Commit Phase 1 as ONE coherent commit (or a handful of step-grouped commits)
with a clear message referencing this plan.

---

# PHASE 2 — Calibrated Kc re-tune (sequential, single owner)

**Goal:** Replace the repo's accidentally-shear-strength-tracking wood/EWP
Kc constants with measured literature values, and replace the magic-number
2.5× anisotropy multiplier with a documented physical grain-anisotropy
factor. Also fix the Janka / species mislabelling surfaced in the hardness
audit (the architectural fix is renaming `SouthernYellowPine` →
`LongleafPine`, since Janka is per-species and SYP is a trade group).

**The architectural framing.** The current `Kc=9, anisotropy=2.5` combo for
particleboard yields an effective ~22 N/mm² in the power formula. Reality
per Pałubicki 2021 is 32–37 N/mm² measured AND grain anisotropy is ~2× per
the same paper's directional spread. The current numbers are the *right
ballpark for one orientation* by accident; the split between "Kc" and
"anisotropy" is physically wrong. Fix the split:
- Kc → measured literature midpoint (the physical specific cutting force).
- ANISOTROPY_MULTIPLIER → 2.0 (documented physical grain anisotropy from
  Pałubicki + isotropy notes for EWPs).
- Deflection bounds → revisit as physical tool-tip safety thresholds
  (when does a 6 mm carbide endmill at 45 mm stickout *actually* deflect
  catastrophically?), not as calibration knobs.

**Why sequential, single owner:** Kc + anisotropy + bounds + Janka must
move as one coherent physical model. Splitting across agents fragments the
physical argument and risks half-applied changes.

**Expected impact on the acceptance loop.** Today's 7/7 Within is
calibrated against physically-wrong constants. After the fix, predicted
peak loads will *roughly preserve* on shapes where the existing
Kc×anisotropy product was accidentally right (most steady-state cases),
but some shapes may flip — and those flips are signal, not failure: the
test was passing because the gate under-predicted force. Update the tests
to the new correct predictions; if a flip cannot be explained by the new
physics, that's a real finding (look at the engagement model, the force
arm, the chip geometry — not at re-fudging the constants).

**The tests get updated, the physics doesn't get bent to match the tests.**

## Step 2A — Knob inventory + baseline capture

Before changing anything, capture the current verdict numbers for the cases
that will move. The Phase 2 owner's "the loop still holds" check needs
known-before/after values.

**Knobs (locations + current values, from scoping):**
| File:Line | Constant | Current | Purpose |
|-----------|----------|---------|---------|
| `tool_load/power.rs:36` | `ANISOTROPY_MULTIPLIER` | 2.5 | Kc multiplier in power gate. |
| `tool_load/deflection.rs:59` | `WITHIN_BOUND_MM` | 0.050 | Under = Within(Validated). |
| `tool_load/deflection.rs:62` | `EXCEEDS_BOUND_MM` | 0.200 | Over = Exceeds. |
| `tool_load/mod.rs:~302-316` | `ToleranceBands` defaults | breakage=0, burn=0, power_breach=0, deflection_breach=0 | Per-verdict gate widening. |

**Tests that pin Kc-derived numbers (will need updates):**
- `tool_load/power.rs::tests::light_cut_is_within_with_available_kw` (range
  `peak_kw < 0.01` on Makita)
- `tool_load/power.rs::tests::heavy_cut_exceeds_machine_with_available_kw`
  (Ipe slot 20mm DOC → Exceeds)
- `tool_load/deflection.rs::tests::wanaka_endmill_back_rough_lands_in_approximate_band`
  (peak 100–200 µm range — EXPLICITLY Kc-tuned per its comment)
- `tool_load/deflection.rs::tests::wanaka_tapered_ball_finishing_is_within`
  (peak <50 µm)

Capture baseline by running these tests with `--nocapture` and noting actual
peak values.

**Acceptance smoke suite baseline** (per
`planning/acceptance_loop/STATE.md` round-10):

AS001 0.076mm, AS002 0.053mm, AS003 0.076mm, AS004 0.005mm, AS005 0.051mm,
AS013 0.105mm, AS015 0.197mm — all Within. These come from MCP runs
(`load_project` + `generate_all` + `run_simulation` on
`test_data/ux_*.toml`). The Phase 2 owner needs the MCP up via `cargo run -p
rs_cam_viz --bin rs_cam_gui -- --mcp` (or `.mcp.json` rs-cam server).

**Validation gate (2A):**
```bash
# Snapshot of every pinned-number test, recorded in a working scratchpad:
cargo test -p rs_cam_core --lib tool_load:: -- --nocapture 2>&1 | tee /tmp/kc_baseline.txt | grep -E "peak_kw|peak_mm|µm|kW|test result"
# Save /tmp/kc_baseline.txt content into the Phase 2 working notes under
# planning/data_ingest_2026-MM-DD/kc_retune_log.md
```

## Step 2B — Fix the Kc/anisotropy split to measured physics

**The principle.** Power gate computes `P = (kc · anisotropy) · doc · width ·
feed / 60e6`. The product `kc · anisotropy` is what physically matters.
Today: `9 · 2.5 = 22.5` for particleboard. Literature: Kc ≈ 32–37 measured,
grain anisotropy ≈ 2.0 (Pałubicki 2021 directional spread). So the *correct*
product is `~35 · 2.0 = 70` — about 3× today's product. Today's gate
under-predicts power by ~3× because the split is wrong AND the magnitude is
wrong.

**Do the full fix in one coherent change:**

1. **Update `material.rs::kc_n_per_mm2()` per-species/family to measured
   literature values.** Cite source in code comment for each.

   Wood/EWP (Grade B measured, from
   `planning/data_ingest_2026-05-29/kc.md`):
   - `SheetGoodKind::Particleboard` → `Some(35.0)` (midpoint of Pałubicki
     32.0/37.6, DOI 10.3390/ma14092208)
   - `SheetGoodKind::Mdf` → `Some(31.4)` (PMC6315737 round-shape Ks)
   - `SheetGoodKind::Hdf` → keep current or set proportional to MDF until
     direct data lands (document as derived).
   - Per-species solid wood: keep current values OR adjust per FPL Ch.5
     shear-parallel reference scaled by the wood-vs-EWP edge-radius factor
     (~3×). This is a per-species derivation; if data is incomplete for a
     species, prefer keeping its current value with a TODO over fabricating.
   - Plywood grades: derive from underlying species per the FPL Ch.5
     framework.

   The HDPE step from 1A already used the measured midpoint (Some(40.0)).
   The other plastics stay None per 1A (no primary source).

2. **Replace `ANISOTROPY_MULTIPLIER = 2.5` with a documented physical
   value.** Rename to `GRAIN_ANISOTROPY_FACTOR` to reflect what it actually
   models (the rename is part of the fix — the old name encouraged
   "tune this knob" thinking). Set to `2.0` with comment citing Pałubicki's
   directional spread.

3. **Revisit deflection bounds as physical safety thresholds.** Today's
   `WITHIN_BOUND_MM = 0.050` (50 µm) and `EXCEEDS_BOUND_MM = 0.200` (200 µm)
   are heuristic. Re-document them in terms of:
   - Below 50 µm: surface finish negligibly degraded by tool deflection.
   - 50–200 µm: surface finish visibly degraded but tool/work safe.
   - Above 200 µm: dimensional accuracy compromised AND risk of chatter /
     tool breakage on continued cuts.

   These are reasonable physical thresholds; keep them. If the new
   higher-Kc predictions push more shapes past 200 µm in the smoke suite
   AND those shapes are known to be safe in practice, that's evidence the
   *deflection model* needs work (likely the force-arm position or the
   integration's near-tip handling) — NOT that the bound should widen. File
   that as a Phase 2 finding; do not silently widen.

4. **Run the full test suite.** Update pinned-number tests in
   `power.rs::tests` and `deflection.rs::tests` to the new actuals
   (verifying each new number is physically reasonable, e.g. by sanity-
   comparing against the source's own published "should be able to cut X").
   The Kc-sensitive sites from scoping:
   - `tool_load/power.rs::tests::light_cut_is_within_with_available_kw`
     (range `peak_kw < 0.01` on Makita — re-measure)
   - `tool_load/power.rs::tests::heavy_cut_exceeds_machine_with_available_kw`
     (Ipe slot Exceeds — should still Exceed, more decisively)
   - `tool_load/deflection.rs::tests::wanaka_endmill_back_rough_lands_in_approximate_band`
     (explicitly Kc-tuned; the band 100–200 µm may shift)
   - `tool_load/deflection.rs::tests::wanaka_tapered_ball_finishing_is_within`
     (peak <50 µm — finishing pass; tapered ball; probably still <50 µm)

5. **Run the MCP smoke suite (AS001–AS015).** Record the new peak µm per
   case in `planning/data_ingest_2026-MM-DD/kc_retune_log.md`. Any case
   that flips Within→Exceeds: investigate against real-world performance
   on the same cut. If the flip matches operator experience (the cut is
   actually marginal), keep it. If the flip is spurious, the finding lives
   in the deflection model, not in the Kc constants.

6. **Do NOT widen `ToleranceBands::deflection_breach` to absorb Kc.**
   Tolerance bands are a per-toolpath safety override, not a global
   physics fudge. Using them to mask the new physics would be the exact
   opposite of the architectural intent of this phase.

## Step 2C — Janka fixes + species/trade-group rename

Two data discrepancies surfaced (see
`planning/data_ingest_2026-05-29/hardness.md`); the second forces a
data-model fix.

### 2C.1 — `RadiataPine`: 500 → 710

Repo 500 lbf vs Wood Database 710 / PreciseBits 750. Update to **710**
(Wood Database is the Grade-A primary). Cite source in code comment.

### 2C.2 — `SouthernYellowPine` → `LongleafPine` (architecturally correct rename)

`WoodSpecies` is a per-species enum; Janka is a per-species measurement.
"Southern Yellow Pine" is a *trade group* covering Longleaf, Loblolly,
Shortleaf, and Slash pines — each with different Janka values. The 690
value the repo carries fits Loblolly approximately, but no Grade-A Loblolly
Janka was fetched (only Longleaf at 870). Carrying a trade-group name with
a guess at which species' Janka is hidden behind it is the wrong model.

**Architectural fix:** rename `WoodSpecies::SouthernYellowPine` →
`WoodSpecies::LongleafPine`, set Janka to **870** (Wood Database Longleaf
Pine entry, verbatim quote in hardness.md). When/if Loblolly data lands as
a separate Grade-A primary, add `WoodSpecies::LoblollyPine` as its own
variant — distinct species, distinct Janka, distinct enum entry.

**Backward compatibility for project files.** Old `.toml` projects with
`species = "southern_yellow_pine"` must continue to load. In
`Material::from_key()` (and the equivalent `WoodSpecies::from_key` or
inline match), add an alias arm that maps the legacy string
`"southern_yellow_pine"` → `WoodSpecies::LongleafPine`. The `to_key()`
side emits only the new key `"longleaf_pine"`. Document the alias with a
comment referencing this plan.

This is standard schema-evolution discipline: rename for correctness, alias
for compat, single direction on write.

### Why this isn't power/deflection-gate-critical

Janka feeds `hardness_index()` → feeds calculation (plunge_rate_base,
vendor LUT hardness scaling), not the power or deflection gates directly.
Risk is feed-rate shift, not verdict flip. Still, run the suite + a feeds
sweep on any test project using these species.

**Validation gate (2C):**
```bash
cargo build -p rs_cam_core 2>&1 | grep -E "error\[" | head
# Expect: no output (rename ripples handled).

cargo test -p rs_cam_core --lib material:: 2>&1 | grep "test result" | grep -v "0 failed"
# Expect: no output.

# Add a regression test that the legacy SYP key still deserializes:
cargo test -p rs_cam_core --lib material::tests::legacy_southern_yellow_pine_key_aliases_to_longleaf 2>&1 | tail -3
# Expect: 0 failed. (Write this test.)

# Audit + update sites that used the old enum name:
rg -l "SouthernYellowPine|southern_yellow_pine" crates
# Expect: zero occurrences of the bare identifier in non-test code (the
# alias arm in from_key is the only allowed string-form reference).
# Test/fixture occurrences must all be updated to LongleafPine.

# Acceptance regressions still green:
cargo test -p rs_cam_core --tests --test dexel_stock_z_frame_f024 \
  --test dexel_stock_z_frame_f026 --test face_stock_top_frame_f028 \
  --test adaptive3d_planner_stock_xy_f027 2>&1 | grep "test result" | grep -v "0 failed"
# Expect: no output.
```

## Phase 2 exit gate

- [ ] `cargo test -p rs_cam_core --lib` → 0 failed (pinned-number tests
  updated to new physically-correct values).
- [ ] `cargo test -p rs_cam_core --tests` → 0 failed.
- [ ] `cargo clippy -p rs_cam_core --all-targets -- -D warnings` → clean.
- [ ] MCP smoke suite (AS001–AS015) run + per-case before/after peak µm +
  verdict recorded in `planning/data_ingest_2026-MM-DD/kc_retune_log.md`.
  Verdict CHANGES are acceptable if they reflect real physics; the log
  must justify each (preserved / flipped-correctly / flipped-suspect).
- [ ] `ANISOTROPY_MULTIPLIER` is renamed to `GRAIN_ANISOTROPY_FACTOR` with
  a citation comment for the value chosen (default 2.0).
- [ ] `Material::kc_n_per_mm2()` for each wood/EWP material carries a
  citation comment pointing at the literature source the value came from.
- [ ] `WoodSpecies::LongleafPine` replaces `SouthernYellowPine`, alias arm
  in `from_key` preserves load-compat for legacy projects.

If a smoke case flips Within→Exceeds AND the flip can't be justified by
the new physics (i.e. the cut is known to work fine in practice), STOP and
file the finding as a deflection-model investigation — do NOT widen
bounds or back off the Kc fix. The wrong fix is to bend the physics to
match yesterday's verdicts; the right fix is to find what the deflection
model is missing.

---

# PHASE 3 — Parallel collection fleet (8 agents)

**Goal:** Scale data ingest now that the schema and Material targets are
ready. Every collected row goes into a NEW dated dir
`planning/data_ingest_2026-MM-DD/` (use today's date when starting Phase 3)
to keep ingest rounds separable for review and rollback.

**Why parallel:** Each beat is independent — different vendor, different
material class, different source format. No shared write paths.

**Common agent contract** (every Phase 3 brief inherits this):

1. Honesty-or-gap (rule #1 above).
2. Output paths: `planning/data_ingest_2026-MM-DD/<beat>.json` (rows),
   `<beat>_provenance.md` (per-row verbatim quotes + unit conversions),
   `<beat>_gaps.md` (unreachable sources + reason).
3. Schema: per Appendix B field list; new vendors not in the enum → log to
   gaps with a "needs enum variant" flag.
4. Tools: `curl -sL -A "Mozilla/5.0" "<url>" -o /tmp/x.pdf && pdftotext
   -layout /tmp/x.pdf -` for vendor PDFs (Amana CDN 403s WebFetch).
   For image-only PDFs: `pdftoppm + tesseract` (the Onsrud OCR beat owns
   this).
5. Unit conversions explicit (inch×25.4=mm; show arithmetic in provenance).
6. Validation gate per agent: see per-beat section below — every agent's
   output must pass `validate_lut.py` schema-clean before its task is
   considered complete.
7. Don't modify any code or any file outside the dated ingest dir.

The full agent prompt skeleton is Appendix C.

## Collectors A–H

Run these 8 in parallel via 8 `Agent` calls in a single message. Each agent
gets a focused brief built from the skeleton + its specific beat below.

### A. Amana long tail (high-yield, low-friction)

**Targets:** ZrN 3D profiling (`ZrN-3D-Profiling-Feed-Chip-Load-Chart-v8.pdf`,
sub-1mm tapered ball rows), Spektra 3D profiling
(`Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf`), Spektra Spiral
Plunge (`Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf` — already
the source for some existing rows but check for missed diameters), spoilboard
(`Spoilboard_2_2-Speed-Chart.pdf` for facing/surfacing).

**Specific check:** Do NOT re-collect AMS-159 V-Groove, Spektra Engraving,
Compression Spirals, Plastic O-Flute, ZrN Aluminum O-Flute — those are
already in the 2026-05-29 round (live or staged).

**Validation gate (A):**
```bash
python3 planning/data_ingest_2026-MM-DD/validate_lut.py 2>&1 | grep -A1 "amana.json"
# Expect: >0 rows; 0 PROBLEMS for those rows.
```
Target: ≥10 schema-clean rows extending diameter / family coverage.

### B. Wood Database species sweep (Janka breadth)

**Targets:** Per-species Janka from wood-database.com for ~25 species not
yet in `WoodSpecies` (red oak northern, cherry, poplar, ash, mahogany,
douglas-fir, alder, beech, hickory, etc.). Output is **structured markdown**
(no JSON — this is hardness data, not LUT rows). Use the same table format
as `planning/data_ingest_2026-05-29/hardness.md` section 1b.

**Validation gate (B):**
```bash
wc -l planning/data_ingest_2026-MM-DD/wood_database_species.md
grep -c "lbf" planning/data_ingest_2026-MM-DD/wood_database_species.md
# Expect: ≥25 species rows with citation, each with verbatim quote.
```

### C. FPL Wood Handbook Ch.5 systematic extract

**Targets:** Every species' shear-parallel-to-grain (kPa or MPa) and side
hardness from FPL Wood Handbook Ch.5, fetched as PDF and extracted with
`pdftotext -layout`. Output as structured markdown table.

**Why this matters:** Per-species Kc DERIVATION reference. Phase 2 only
moved wood Kc as a class; Phase 3+ can move it per-species using
shear-parallel as the derivation backbone.

**Validation gate (C):**
```bash
grep -c "shear" planning/data_ingest_2026-MM-DD/fpl_ch5_extract.md
# Expect: ≥40 species rows.
```

### D. Onsrud OCR specialist (NEW TECHNIQUE)

**Targets:** The Onsrud `/images/*.pdf` data sheets that were image-only
(see `planning/data_ingest_2026-05-29/onsrud_whiteside_gaps.md`). Use:
```bash
curl -sL -A "Mozilla/5.0" "<pdf_url>" -o /tmp/o.pdf
pdftoppm -r 300 /tmp/o.pdf /tmp/page
for p in /tmp/page-*.ppm; do tesseract "$p" - 2>/dev/null; done
```
Tesseract output is messy — agent reviews each extracted chipload/SFM value
and only records ones where the number is unambiguous in the OCR (otherwise
GAP it). Confirm via manual cross-check of the OCR'd image (open in
viewer if needed) — this beat is unique in that it benefits from human-in-loop
spot-checking; the agent should log any "uncertain OCR" rows to the gaps
file rather than guess.

**Validation gate (D):**
```bash
python3 planning/data_ingest_2026-MM-DD/validate_lut.py 2>&1 | grep -A1 "onsrud_ocr.json"
# Expect: 0 PROBLEMS on the OCR-extracted rows; reasonable yield (≥10 rows
# from the wood sheet alone). Volume in gaps file is acceptable.
```

### E. Plastics Kc archival + back-calculation

**Targets:**
- POM/Delrin paywalled papers (Trifunović 2021 J.CleanerProd 303:127043;
  Chabbi 2017 Measurement 95): try Wayback Machine
  (`https://web.archive.org/web/*/<doi-url>`), Sci-Hub mirrors are NOT to
  be used. Library/institutional access if available. If still paywalled,
  log to gaps.
- PMMA Korkmaz 2017 (Procedia Mfg 10:683): figure-extraction for back-calc
  Kc ≈ Fc/(ap·f). Open access — re-fetch the full PDF (the ScienceDirect
  HTML returned 403 last time; try the journal landing or
  doi.org/10.1016/j.promfg.2017.07.017 with curl + browser UA).
- Polycarbonate: confirm no primary force study; document the gap
  authoritatively in `kc_gaps.md`.

**Output:** Extend `planning/data_ingest_2026-MM-DD/kc_extra.md` with the
same structure as `planning/data_ingest_2026-05-29/kc.md`.

**Validation gate (E):**
```bash
grep -c "Grade B" planning/data_ingest_2026-MM-DD/kc_extra.md
# Expect: ≥1 new Grade-B back-calculated polymer Kc (POM if Wayback works;
# PMMA from Korkmaz figure read) OR ≥1 explicit GAP entry per polymer
# attempted with the verbatim error / 403 / paywall message.
```

### F. Aluminum Kienzle kc1.1/mc archival hunt (the single most valuable Kc gap)

**Targets:** Get a fetched-primary `kc1.1 + mc` Kienzle pair for an
aluminum alloy. Sources to try (in order of likely success):
- Wayback Machine snapshots of the old Sandvik Coromant Kc page
  (`web.archive.org/web/2020*/sandvik.coromant.com/.../specific-cutting-force`)
- Kennametal "Technical Data Metric" catalog PDFs (often mirrored on
  scribd / engineering library)
- Machining Doctor glossary entry (machiningdoctor.com)
- ResearchGate Sandvik "Technical Guide – Materials ISO" PDF (the 2026-05-29
  fetch returned 403; try archive.org `web.archive.org/web/*/<url>`)

A single Grade-A `(kc1.1, mc)` for ANY aluminum alloy closes the highest-value
metals gap. If literally none can be fetched primary, document as GAP and
extract whatever range Sandvik DOES publish on the live page (e.g. "N group
350–1350" is at least a soft bound).

**Validation gate (F):**
```bash
grep -E "kc1\.1.*mc.*=" planning/data_ingest_2026-MM-DD/kc_extra.md
# Expect: at least one line with a numeric kc1.1 + mc for an aluminum alloy,
# OR a dated GAP entry explaining exhaustively what was tried and blocked.
```

### G. Whiteside / Freud / Vortex / IDC vendor breadth

**Targets:**
- Whiteside per-bit chipload (the 2026-05-29 round captured RPM only;
  Whiteside has product-page chipload info on some series — check)
- Freud `router-bit-feed-and-speed PDF` (community-mentioned as the closest
  Freud public chipload chart)
- Vortex Tool downloadable feeds & speeds (vortextool.com /resources)
- IDC Woodcraft "Millmage" database CSV downloads
  (idcwoodcraft.com/pages/database-downloads) — already structured;
  conversion to `VendorObservation` is mostly mechanical.

**New vendor variants needed in `Vendor` enum:** `freud`, `vortex`
(carbide3d already exists; idc is not a tool vendor — likely treat IDC rows
as `source_vendor: "idcwoodcraft"` with that vendor added, OR record them
as Grade-C cross-checks and not bundled).

**Validation gate (G):**
```bash
python3 planning/data_ingest_2026-MM-DD/validate_lut.py 2>&1 | grep PROBLEMS
# Expect: schema problems for new vendors NOT in enum (logged); 0 problems
# on rows from already-enum vendors.
grep -c "source_vendor" planning/data_ingest_2026-MM-DD/vendor_breadth.json
# Expect: ≥15 schema-clean rows.
```

### H. Per-material extended hardness + Shore-D fan-out

**Targets:** MatWeb / supplier datasheets for plastics already in the repo
but lacking per-grade hardness; expand to UHMW, PVC, Polypropylene
(structures the next `Material::Plastic` family additions). Aluminum: 2024,
5052, 3003 Brinell from ASM.

**Validation gate (H):**
```bash
grep -c "Shore D\|Rockwell M\|Brinell" planning/data_ingest_2026-MM-DD/hardness_extra.md
# Expect: ≥10 new entries with verbatim quote + standard.
```

## Phase 3 exit gate

- [ ] Each of A–H either produced staged outputs that pass
  `validate_lut.py` for any JSON rows, or produced a `*_gaps.md` honestly
  documenting why not. No silent failures.
- [ ] Total new staged rows ≥ 50 (a reasonable batch; if much less, some
  beats hit walls that warrant re-strategising).
- [ ] No code touched outside `crates/rs_cam_core/src/feeds/vendor_lut.rs`
  if and only if Step G needed a new `Vendor` variant.

---

# PHASE 4 — Verification + promotion gate

**Goal:** Promote what's safe; keep the rest staged with explicit notes.
Mirror the 2026-05-29 pattern (independent verifier + schema gate + test
gate).

## Step 4.1 — Schema validation pass

```bash
python3 planning/data_ingest_2026-MM-DD/validate_lut.py 2>&1 | tee /tmp/phase4_validate.txt
```
Resolve every PROBLEM line: fix the row (refetch if missing data; correct
enum if mistyped) or move to `_gaps.md`.

## Step 4.2 — Adversarial verifier agent

Dispatch one `general-purpose` agent (it does WEB work). Brief:

```
You are an independent VERIFIER. Read planning/data_ingest_2026-MM-DD/.
For each new JSON file: pick 3-5 random rows + every row that looks like an
outlier (chipload >0.3 mm, very small diameter, unfamiliar vendor).
Re-fetch each row's source_url and CONFIRM or REFUTE its chipload, RPM,
diameter, and included_angle. Report a markdown table CONFIRM/MISMATCH with
the source's actual number. ALSO answer: any signs of column-confusion,
unit errors, or material-mapping errors? Modify NOTHING. Output to
planning/data_ingest_2026-MM-DD/verification_report.md.
```

Any MISMATCH must be reconciled BEFORE Step 4.3.

**Validation gate (4.2):**
```bash
grep -E "MISMATCH|UNCONFIRMED" planning/data_ingest_2026-MM-DD/verification_report.md
# Expect: no output, OR every match has a documented disposition (correction
# applied to the row, or row moved to gaps).
```

## Step 4.3 — Test-gated promotion

For each schema-clean, verified file, the promotion script:
1. Copy/split staged JSON into one or more files under
   `crates/rs_cam_core/data/vendor_lut/observations/<source>_<scope>.json`.
2. Add the new files to the `include_str!` list in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs::VendorLut::embedded()`.
3. Update both count assertions (`tests::test_embedded_loads_all_observations`
   in `vendor_lut.rs` AND `embedded_count_matches_after_expansion` in
   `tests/vendor_lut_sub_1mm.rs`).
4. Run the full gate:
   ```bash
   cargo test -p rs_cam_core --lib 2>&1 | grep "test result:" | grep -v "0 failed"
   cargo test -p rs_cam_core --tests 2>&1 | grep "test result:" | grep -v "0 failed"
   cargo clippy -p rs_cam_core --all-targets -- -D warnings 2>&1 | grep -E "warning:|error:"
   ```
5. If green, keep. If red, identify the failing test:
   - If it's an unrelated regression, ROLLBACK that file's promotion (pull
     it out of include_str! + revert count), pin the rows to staging with a
     note explaining the regression.
   - If it's the new `_f0*` family failing on a Kc-sensitive verdict shift,
     consider whether the new row genuinely matches a test scenario it
     shouldn't (e.g. material mapping wrong) and either fix the row's
     `material_family` or pull it back.
6. Re-run MCP smoke (AS001–AS015) if any wood rows were promoted, just like
   Phase 2's exit gate.

## Step 4.4 — CREDITS + manifest update

For every NEW bundled source (one entry per source_id):
- Append to `crates/rs_cam_core/data/vendor_lut/source_manifest.json` (use
  the Python append pattern from 2026-05-29).
- Add a one-liner under CREDITS.md's "Vendor LUT source manifest" 2026-05-29
  block, dated 2026-MM-DD.

For Kc / hardness research used (even if not yet wired):
- Add a citation block under CREDITS.md's "Material-property and force-model
  references" section.

## Phase 4 exit gate

- [ ] `cargo test -p rs_cam_core --lib` and `--tests` both 0 failed.
- [ ] Clippy clean across `--all-targets`.
- [ ] MCP smoke 7/7 Within (if wood rows promoted; otherwise N/A).
- [ ] CREDITS.md + source_manifest.json reflect every bundled source.
- [ ] Phase 4 consolidation report written at
  `planning/feeds_data_ingest_consolidation_2026-MM-DD.md` listing: rows
  collected this round, rows promoted live, rows kept staged + why, Kc/hardness
  findings, open gaps.

---

# Abort / rollback criteria

- **Phase 1:** if match-exhaustiveness fixes balloon past ~400 LOC or any
  acceptance test fails after a single step, stop and surface — the scoping
  may have missed a path.
- **Phase 2:** verdict flips on the smoke suite are acceptable IF justified
  by the new physics; the abort criterion is the OPPOSITE — if a justified
  flip gets reverted by silently widening bounds or backing off the Kc fix,
  STOP. The physics is the truth; the tests get updated to match. The only
  legitimate "back off" is "the deflection model has a bug we just
  uncovered" — and that's a finding to file, not a reason to revert the
  Phase 2 changes.
- **Phase 3:** any single beat that produces zero schema-clean rows OR a
  pure-fabrication output (no verbatim quotes in provenance) — drop the
  agent's output entirely.
- **Phase 4:** any verdict regression in a `_f0*` test that can't be traced
  to a specific incorrect row → rollback the whole promotion batch, do not
  patch the test.

---

# Out-of-scope (deliberately)

These came up in scoping but are NOT in this plan:

- Material::Foam refinement (existing variants; no new Kc data found).
- Drill metrics aluminum thresholds beyond the bare minimum for compile
  (the 2026-05-29 hardness data doesn't constrain aluminum drill peck
  depths; researching aluminum drill metrics is a separate task).
- The optimizer's Kc/anisotropy paths (`feed_modulation.rs`) — they read
  via `kc_n_per_mm2()` and will auto-track Phase 2 changes; no separate
  re-tune needed unless they have their own pinned numbers (check via
  `cargo test -p rs_cam_core --lib feed_modulation::tests`).
- GUI material picker UI polish (the catalog auto-feeds the combo box;
  reorder or grouping is optional).
- New ChiploadStatistic categories for DOC-derated bounds (Step 1E uses the
  existing statistic + bounds; if the verdict messaging needs to mention
  "derated by 2×D rule" that's a UX nice-to-have, not blocking).

---

# Appendix A — `validate_lut.py` (canonical script)

Save this to your dated ingest dir
(`planning/data_ingest_2026-MM-DD/validate_lut.py`). If
`VendorObservation` in `crates/rs_cam_core/src/feeds/vendor_lut.rs` changes,
update the `REQUIRED` list / enum sets accordingly.

```python
#!/usr/bin/env python3
"""Validate staged LUT JSON against the live VendorObservation schema.

Usage: python3 validate_lut.py [staging_dir]
Default staging_dir is the script's own directory.
"""
import json, glob, sys, os
from collections import Counter

REQUIRED = ["observation_id","source_id","source_vendor","source_title","source_url",
            "accessed_on","evidence_grade","row_kind","tool_family","operation_family",
            "pass_role","material_family","material_label","diameter_mm","flute_count"]
VENDORS = {"amana","onsrud","harvey","whiteside","sandvik","garr","autodesk","carbide3d",
           "helical"}  # add new vendors here as they're enum-extended
GRADES = {"a","b","c"}
KINDS = {"exact","derived","fallback"}
TFAM = {"flat_end","ball_nose","tapered_ball_nose","bull_nose","chamfer_vbit","facing_bit"}
OFAM = {"adaptive","pocket","contour","parallel","scallop","trace","face"}
ROLE = {"roughing","semi_finish","finish"}
MFAM = {"softwood","hardwood","plywood_softwood","plywood_hardwood","mdf","hdf",
        "particleboard","acrylic","hdpe","polycarbonate","delrin","aluminum"}
WOOD = {"softwood","hardwood","plywood_softwood","plywood_hardwood","mdf","hdf","particleboard"}

def main(staging_dir):
    ids = set()
    problems = []
    usable_now = []
    staged_reasons = []
    for f in sorted(glob.glob(os.path.join(staging_dir, "*.json"))):
        try:
            d = json.load(open(f))
        except Exception as e:
            problems.append((f, "<file>", [f"PARSE: {e}"]))
            continue
        for o in d.get("observations", []):
            oid = o.get("observation_id", "<noid>")
            miss = [k for k in REQUIRED if k not in o]
            errs = []
            if miss: errs.append("MISSING:" + ",".join(miss))
            if o.get("source_vendor") not in VENDORS:
                errs.append(f"vendor={o.get('source_vendor')!r} not in enum")
            if o.get("evidence_grade") not in GRADES: errs.append("grade invalid")
            if o.get("row_kind") not in KINDS: errs.append("kind invalid")
            if o.get("tool_family") not in TFAM: errs.append("tool_family invalid")
            if o.get("operation_family") not in OFAM: errs.append("operation_family invalid")
            if o.get("pass_role") not in ROLE: errs.append("pass_role invalid")
            if o.get("material_family") not in MFAM: errs.append("material_family invalid")
            if oid in ids: errs.append("DUPLICATE_ID")
            ids.add(oid)
            cmin = o.get("chipload_min_mm_tooth")
            cmax = o.get("chipload_max_mm_tooth")
            if cmin is not None and cmax is not None and cmax < cmin:
                errs.append("chipload max<min")
            if cmin is not None and cmin <= 0: errs.append("chipload min<=0")
            if cmax is not None and cmax > 0.5:
                errs.append(f"chipload max suspicious ({cmax})")
            if errs: problems.append((os.path.basename(f), oid, errs))
            if not miss and o.get("material_family") in WOOD and cmax is not None:
                usable_now.append(oid)
            elif miss:
                staged_reasons.append("missing-field")
            elif cmax is None:
                staged_reasons.append("no-chipload")
            else:
                staged_reasons.append("non-wood")
    print(f"TOTAL rows: {len(ids)}")
    print(f"\n=== PROBLEMS ({len(problems)}) ===")
    for f, oid, e in problems:
        print(f"  [{f}] {oid}: {'; '.join(e)}")
    print(f"\n=== USABLE NOW (wood + schema-complete + chipload): {len(usable_now)} ===")
    for x in usable_now: print("  +", x)
    print(f"\n=== NOT-LIVE (staged) reason counts ===")
    print(" ", Counter(staged_reasons))

if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(__file__) or ".")
```

# Appendix B — `VendorObservation` schema (frozen reference)

Source of truth is `crates/rs_cam_core/src/feeds/vendor_lut.rs`. As of
2026-05-30:

```rust
pub struct VendorObservation {
    // REQUIRED (no Option, no serde default)
    pub observation_id: String,       // unique kebab e.g. "amana-vbit-softwood-trace-6000-2f"
    pub source_id: String,            // groups rows from same source
    pub source_vendor: Vendor,        // enum: amana|onsrud|harvey|whiteside|sandvik|garr|
                                      //   autodesk|carbide3d|helical (post-Phase1D)
    pub source_title: String,
    pub source_url: String,
    pub accessed_on: String,          // "YYYY-MM-DD"
    pub evidence_grade: EvidenceGrade,// a|b|c
    pub row_kind: ObservationKind,    // exact|derived|fallback
    pub tool_family: ToolFamily,      // flat_end|ball_nose|tapered_ball_nose|bull_nose|
                                      //   chamfer_vbit|facing_bit
    pub operation_family: LutOperationFamily,  // adaptive|pocket|contour|parallel|scallop|
                                                //   trace|face
    pub pass_role: LutPassRole,       // roughing|semi_finish|finish
    pub material_family: MaterialFamily,
    pub material_label: String,
    pub diameter_mm: f64,
    pub flute_count: u32,
    // OPTIONAL (Option<...> or #[serde(default)])
    pub tool_subfamily: Option<String>,
    pub hardness_kind: Option<HardnessKind>,    // janka|hb|shore_d
    pub hardness_value: Option<f64>,
    pub rpm_min: Option<f64>,
    pub rpm_max: Option<f64>,
    pub rpm_nominal: Option<f64>,
    pub chipload_min_mm_tooth: Option<f64>,
    pub chipload_max_mm_tooth: Option<f64>,
    pub ap_min_mm: Option<f64>,
    pub ap_max_mm: Option<f64>,
    pub ae_min_mm: Option<f64>,
    pub ae_max_mm: Option<f64>,
    pub ap_rule: Option<String>,
    pub ae_rule: Option<String>,
    pub machine_assumption: Option<String>,
    pub source_page: Option<String>,
    pub included_angle_deg: Option<f64>,   // 2026-05-29; chamfer_vbit cone angle
    pub tip_diameter_mm: Option<f64>,      // 2026-05-29; flat-tip / truncated bits
}
```

# Appendix C — Phase 3 agent prompt skeleton

Adapt the bracketed slots per beat (A–H above). Every Phase 3 brief must
include all numbered sections verbatim.

```
You are collecting real, citeable [BEAT_TOPIC] data for `rs_cam`, a Rust CAM
program for a 3-axis wood router. The data feeds SAFETY-CRITICAL tool-load
gates. Overriding rule:

**NEVER invent, estimate, or interpolate. Record a number ONLY if you can
read it from a fetched source, with a verbatim quote stored alongside.
Unreachable / image-only / paywalled / calculator-gated → log as a GAP, do
NOT produce a row.**

Read first:
- planning/feeds_data_ingest_2026-05-30_phased_plan.md (this plan; your section is [LETTER + TITLE])
- planning/feeds_data_source_acquisition_2026-05-29.md (the source list)
- crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json (schema template)
- existing files in crates/rs_cam_core/data/vendor_lut/observations/ to avoid duplicates

Your beat: [SPECIFIC URLS / SOURCES]

Conventions:
- Convert inch→mm (×25.4) and show arithmetic.
- Use the curl recipe for vendor PDFs that block WebFetch:
    curl -sL -A "Mozilla/5.0 (X11; Linux x86_64)" "<pdf_url>" -o /tmp/x.pdf && pdftotext -layout /tmp/x.pdf -
- Schema reference: Appendix B of the plan doc. Required fields are
  non-Option; missing any one of them will fail to deserialize at load time.

Output (touch NO live code or data files; only the dated staging dir):
1. planning/data_ingest_2026-MM-DD/[beat].json — {"observations":[...]}
2. planning/data_ingest_2026-MM-DD/[beat]_provenance.md — per observation_id:
   source URL, page, VERBATIM quote of the source numbers, and any
   inch→mm arithmetic shown.
3. planning/data_ingest_2026-MM-DD/[beat]_gaps.md — every URL/source you
   could not extract, with reason.

Validation target before reporting complete:
[BEAT_VALIDATION_TARGET — e.g. "≥10 schema-clean rows extending diameter
coverage; ≥3 V-groove angles not already in the 2026-05-29 set"]

Report a concise summary: rows collected, families/diameters covered, top
gaps. Verified-few beats guessed-many. Do not touch code.
```

---

# Index of artifacts referenced by this plan

- This plan: `planning/feeds_data_ingest_2026-05-30_phased_plan.md`
- Coverage audit: `planning/feeds_data_coverage_audit_2026-05-29.md`
- Source acquisition list: `planning/feeds_data_source_acquisition_2026-05-29.md`
- 2026-05-29 consolidation: `planning/feeds_data_ingest_consolidation_2026-05-29.md`
- 2026-05-29 staging: `planning/data_ingest_2026-05-29/` (json, provenance, gaps,
  kc.md, hardness.md, verification_report.md)
- Live LUT: `crates/rs_cam_core/data/vendor_lut/observations/*.json`
- LUT source manifest: `crates/rs_cam_core/data/vendor_lut/source_manifest.json`
- Material model: `crates/rs_cam_core/src/material.rs`
- Tool-load gates: `crates/rs_cam_core/src/tool_load/{chipload,power,deflection}.rs`
- LUT lookup logic: `crates/rs_cam_core/src/feeds/vendor_lookup.rs`
- Generic tool-diagnostics plan (parent): `planning/tool_diagnostics_generic_plan.md`
- Acceptance loop state: `planning/acceptance_loop/STATE.md`
- Project conventions: `CLAUDE.md`
- Credits / provenance: `CREDITS.md`
