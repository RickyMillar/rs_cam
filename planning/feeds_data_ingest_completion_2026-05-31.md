# Feeds Data Ingest — Completion Plan to 10/10

**Date:** 2026-05-31
**Status:** Plan locked, ready to execute across multiple compact-cycle sessions.
**Predecessor commits:** Phase 1+2C (`5ccf533`), Phase 2B (`bbb164f`), audit
consolidation (`132e0a8` / `a4c6bfe` / `91625d6` / `c5f0363` / `a73f244`),
Phase 4 bulk promotion (`5a67c1d`).

## Where we are (6.5/10 as of 2026-05-31)

The plumbing is done: unified power formula, V-bit-aware dispatch, canonical
plastic hardness, tracing, literature_parity sentries, 228 embedded LUT rows.
The remaining gap is **empirical validation + staged research promotion**.

## What 10/10 means

Five streams must close:

1. **Empirical validation** — MCP smoke (AS001–AS015) + `param_sweep` baseline
   diff prove the new data behaves better than the old.
2. **Garr aluminum** — last pre-Phase-4 staged-data deferral.
3. **Plastic family extension** — 6 new `PlasticFamily` variants from
   `hardness_extra.md` (UHMW-PE, PP, Nylon 6/6, ABS, PETG, PVC).
4. **Aluminum alloy extension** — 5 new `AluminumAlloy` variants from
   `hardness_extra.md` (2024-T3, 5052-H32, 3003-H14, 1100-O, 7050-T7651).
5. **Wood species expansion** — parametric `SolidWoodByJanka` variant + species
   library lookup, with the existing enum staying tight. Surfaces the staged
   Wood Database (34) + FPL Ch.5 (113) species as selectable defaults.

**Explicitly out of scope** (per operator decision 2026-05-31):

- Per-spindle gate for Freud 1/2" industrial rows — unscoped, parked in
  `industrial_only/`. Re-open if/when 1/2" industrial coverage becomes a
  user concern.
- PMMA Kc Grade B value from `kc_extra.md` — staging-author's verbatim
  "do NOT promote" caveat stands. Acrylic Kc remains refused-by-default.
- POM/Delrin and Polycarbonate Kc — still genuine GAPs per `kc_extra.md`;
  no primary milling-scale source exists. Refusal-first stays.

---

## Design decisions locked in this plan

### D1 — Wood species: parametric variant, NOT enum explosion

**Decision:** Add `Material::SolidWoodByJanka { janka_lbf: f64, label: String,
source_id: String }` as a new variant. The existing `WoodSpecies` enum stays
tight (~10 commonly-cut species) so pattern matching coverage stays managed.

The full staged species set (34 Wood Database + 113 FPL = ~147 unique) is
exposed via a const lookup table `WOOD_SPECIES_LIBRARY: &[(name, janka,
source_id)]`. The GUI surfaces this as a searchable dropdown; selecting a
library row stores the parametric variant in the project file.

**Why this and not the alternatives:**

- *Full enum expansion (~147 variants):* enum bloat that doesn't enable
  anything the parametric variant doesn't. Every literature_parity sentry
  would be hand-written.
- *Pure parametric with no library:* loses the curated provenance — users
  type "oak" and get whatever they type, no source citation.
- *Hybrid (this):* enum keeps the 10 best-tested species first-class for
  type-safe matching; library gives broad coverage; parametric variant
  carries the citation forward into the project file.

**Promotion policy for SolidWoodByJanka:** the Kc formula already used by
`Material::SolidWood` reads through `WoodSpecies::janka_lbf()` — re-route
through a shared `janka_to_kc(janka_lbf)` helper so both variants share the
formula. Refuse Kc when the formula can't apply (e.g., extreme Janka beyond
the regression's range — add a band, refuse outside it).

### D2 — Plastic family extension

**Decision:** Add `PlasticFamily::{UhmwPe, Polypropylene, Nylon66, Abs,
Petg, RigidPvc}` (6 new variants) in one pass. Each gets:

- A `PlasticHardness` value via `PlasticFamily::hardness()` (the canonical
  accessor S1-3 wired up).
- `Material::Plastic { family }` Kc via the existing per-family switch.
- A literature_parity sentry citing `hardness_extra.md`.

**Kc treatment:** ABS, PETG, PVC have NO authoritative milling Kc in our
staging — they get `kc_n_per_mm2() -> None` per refusal-first. UHMW-PE, PP,
Nylon 6/6 — check `kc_extra.md` for sources; if none, refuse-by-default.

**`PlasticHardness` enum extension:** the existing `{ShoreD, RockwellM}`
variants cover most staged hardness scales; `RockwellR` likely needed for
ABS/PETG/Nylon. Add `RockwellR` variant if any staged data lands on R-scale.

### D3 — Aluminum alloy extension

**Decision:** Add `AluminumAlloy::{Al2024T3, Al5052H32, Al3003H14, Al1100O,
Al7050T7651}` (5 new variants). Hardness via `AluminumAlloy::brinell_hb()`
accessor (parallel to `PlasticFamily::hardness()`). Kc remains the single
Kienzle pair (kc1.1=800, mc=0.25) applied across all alloys — vendor sources
don't differentiate Kc by alloy at our fidelity level.

Each gets a literature_parity sentry citing the matweb / Kaiser PDF.

### D4 — Garr per-series flute split

**Decision:** Promote per-series with verbatim vendor CPT. Three new rows
per chart entry (242M 2-flute, 842M 2-flute, A3 3-flute). The chart's
shared-CPT-across-series claim is the vendor's published number; runtime
gates will flag if reality diverges.

**Schema change:** none — `flute_count` is already mandatory in
`VendorObservation`. Splitting is just JSON authoring.

### D5 — Audit deferrals

- **S2-8 hardness_index semantics**: defer further. The "wood-baseline
  normalizer applied to all materials" weirdness needs a Material trait
  refactor that's out of scope here. Document the caveat (already done in
  `material.rs` doc-comments per audit commit `132e0a8`).
- **S2-9 leaky LimitInputs**: pick off as a 30-min cleanup *during*
  Phase E (below), since plastic extension touches the same code path.
- **S3-13 test fixture clutter**: pick off as a 15-min cleanup with a
  `Material::test_fixture_custom()` helper. Bundle into the cleanest
  unrelated commit.

---

## Phase ordering & rationale

The order matters: empirical validation FIRST so we have a baseline against
which subsequent additions can be compared. Then promote in order of
increasing surface-area / design risk.

**Phase A — Empirical baseline** (no code changes; just data capture)
↓ proves the data we already shipped works
**Phase B — Garr aluminum split** (small, well-scoped, no design risk)
↓ closes the last carry-forward from Phase 1C
**Phase C — Aluminum alloy extension** (5 variants, established pattern)
↓ low risk, builds confidence for the plastic pass
**Phase D — Plastic family extension** (6 variants, may need PlasticHardness::RockwellR)
↓ medium risk; canonical accessor pattern already exists
**Phase E — Wood species parametric variant + library**
↓ highest design surface; do last when patterns are well-worn
**Phase F — Final verification gate**
↓ re-run smoke + sweep, compare to Phase A baseline

Each phase ends with the standard gate: `cargo test -p rs_cam_core --lib +
--tests`, `cargo clippy -p rs_cam_core --all-targets -- -D warnings`,
`cargo test -p rs_cam_core --test literature_parity`, all green.

---

## Phase A — Empirical baseline

**Goal:** capture the runtime behavior of the current LUT (post-`5a67c1d`)
against the AS001–AS015 smoke suite and the `param_sweep` battery, so
later phases can be diffed against it.

**Prereqs:**

- Live GUI: `cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp` (operator).
- `pgrep -af "cargo --release|rs_cam_gui"` must return empty before any
  parallel cargo runs.

**Steps:**

1. **MCP smoke (operator-only)** — for each of AS001–AS015:
   1. Load the case file via `load_project`.
   2. `generate_all` → `run_simulation`.
   3. Record peak deflection µm, chipload verdict, power verdict.
   4. Log to `planning/data_ingest_2026-05-30/kc_retune_log.md` under a new
      "Phase A baseline (post-`5a67c1d`)" section.
2. **param_sweep baseline (autonomous)**:
   1. `cargo test --test param_sweep 2>&1 > target/sweep_baseline_5a67c1d.log`
      (~5–10 min, runs all 54 sweeps).
   2. Snapshot `target/param_sweeps/` to
      `planning/data_ingest_2026-05-30/sweep_snapshot_5a67c1d.tar.gz`
      (read-only archive).
   3. Run `python3 toolpath_stress_test/agents/analyze_sweep.py
      target/param_sweeps/` and capture verdict counts.

**Definition of done:** the smoke log section + sweep snapshot exist in
`planning/data_ingest_2026-05-30/`. No code changes.

**Verification gate:** N/A (data capture only).

---

## Phase B — Garr aluminum per-series flute split

**Goal:** promote the 10 staged Garr aluminum rows currently blocked on
per-series flute disambiguation.

**Prereqs:**

- Read `planning/data_ingest_2026-05-29/harvey_helical_garr.json` to confirm
  source structure.
- Read `crates/rs_cam_core/data/vendor_lut/observations/helical_aluminum.json`
  as the per-series template (Helical's per-series rows are already split).

**Steps:**

1. Author `crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
   with 30 rows (10 chart entries × 3 series: 242M-2f, 842M-2f, A3-3f).
   Each row carries the vendor's published CPT verbatim with `flute_count`
   set to the per-series value.
2. Add `include_str!` entry to `EMBEDDED_FILES` const in
   `crates/rs_cam_core/src/feeds/vendor_lut.rs`.
3. Bump count assertion 228 → 258 in both
   `crates/rs_cam_core/src/feeds/vendor_lut.rs` and
   `crates/rs_cam_core/tests/vendor_lut_sub_1mm.rs`.
4. Add source manifest entry for `garr_solid_carbide_chart`.
5. Update `CREDITS.md` with the Garr block.

**Verification gate:** standard four checks. F-024/F-026/F-027/F-028 sentries
must stay green. `test_embedded_strict_parse` catches any schema issues.

**Definition of done:** 30 new rows live, source cited, commit landed.
Garr staged-deferral closed.

---

## Phase C — Aluminum alloy extension

**Goal:** add 5 new `AluminumAlloy` variants with Brinell-HB hardness from
`hardness_extra.md` H.2.

**Steps:**

1. Add `AluminumAlloy::{Al2024T3, Al5052H32, Al3003H14, Al1100O,
   Al7050T7651}` variants to `material.rs`.
2. Add `AluminumAlloy::brinell_hb() -> u32` accessor (parallel to
   `PlasticFamily::hardness()`). Wire each variant to the verbatim staged
   Brinell value.
3. Wire `Material::Aluminum { alloy }` hardness lookups through the new
   accessor (audit pattern S1-3).
4. Add 5 sentries to `crates/rs_cam_core/tests/literature_parity.rs`
   citing the matweb / Kaiser PDF rows.
5. Update GUI dropdowns (if any) — likely just a Settings combo enum case.
   Audit `crates/rs_cam_viz/` for `AluminumAlloy::` match arms (use
   `cargo check` to catch missing cases — exhaustive match is the safety
   net).
6. Update `CREDITS.md` Aluminum section.

**Verification gate:** standard four checks + literature_parity must pick
up 5 new green tests (16 sentries total).

**Definition of done:** 5 alloys callable from `Material::Aluminum { alloy }`,
each pinned to source by sentry, GUI selectable.

---

## Phase D — Plastic family extension

**Goal:** add 6 new `PlasticFamily` variants with hardness via the canonical
accessor.

**Steps:**

1. Extend `PlasticHardness` enum with `RockwellR` variant if any staged
   row uses R-scale (check `hardness_extra.md` H.1 — yes, ABS/PETG/Nylon do).
2. Add `PlasticFamily::{UhmwPe, Polypropylene, Nylon66, Abs, Petg, RigidPvc}`
   variants.
3. Wire `PlasticFamily::hardness()` for each new variant with the verbatim
   staged hardness (canonical accessor — single source of truth).
4. Update `Material::Plastic { family }` Kc switch: each new family returns
   `None` (refuse-first) unless `kc_extra.md` has an authoritative source
   for that family (POM/Delrin and PC remain GAP; PMMA Grade B caveat
   stands).
5. Add 6 sentries to `crates/rs_cam_core/tests/literature_parity.rs`
   citing `hardness_extra.md` rows.
6. Update `vendor_normalize::material_to_lut` if needed (audit S1-3 made
   it call the canonical accessor; should "just work" for new variants).
7. Update GUI plastic-family combo enum cases (same exhaustive-match
   pattern as Phase C).
8. **Bundle audit deferral S2-9** (leaky LimitInputs) — same code path,
   30-min cleanup to fold in. Adds explicit `LimitInputs` shape closures.
9. Update `CREDITS.md` Plastics section.

**Verification gate:** standard four checks + 6 new literature_parity
sentries green (22 sentries total).

**Definition of done:** 6 plastic families selectable, hardness pinned,
refusal-first preserved for Kc-gap families.

---

## Phase E — Wood species parametric variant + library

**Goal:** add `Material::SolidWoodByJanka { janka_lbf, label, source_id }`
variant + const `WOOD_SPECIES_LIBRARY` lookup. Ingest the staged 34+113
species as library defaults without enum bloat.

**Sub-phase E.1 — Shared Janka→Kc helper**

1. Extract the formula currently embedded in `Material::SolidWood` Kc switch
   to `pub(crate) fn janka_to_kc_n_per_mm2(janka_lbf: f64) -> Option<f64>`.
   Refuses outside the regression's calibrated band (define band — likely
   [200, 4000] lbf based on existing literature_parity coverage).
2. Re-wire `WoodSpecies::*` Kc lookups through the helper. Asserts the
   existing literature_parity sentries still pass.

**Sub-phase E.2 — `SolidWoodByJanka` variant**

1. Add `Material::SolidWoodByJanka { janka_lbf: f64, label: String,
   source_id: String }`.
2. Wire `hardness_index`, `kc_n_per_mm2` (via the shared helper), `label`,
   `serialize/deserialize` for project-file IO.
3. Add a literature_parity sentry asserting `janka_to_kc(870.0)` matches
   `WoodSpecies::LongleafPine::kc_n_per_mm2()` to within tolerance —
   pins the helper to the existing enum value.

**Sub-phase E.3 — `WOOD_SPECIES_LIBRARY` const**

1. Add `crates/rs_cam_core/src/material/wood_species_library.rs` containing
   `pub const WOOD_SPECIES_LIBRARY: &[WoodSpeciesEntry] = &[...]` with one
   entry per staged Wood Database + FPL species. Each entry:
   ```rust
   pub struct WoodSpeciesEntry {
       pub display_name: &'static str,
       pub scientific_name: Option<&'static str>,
       pub janka_lbf: f64,
       pub source_id: &'static str,
   }
   ```
2. Source_id values: `"wood_database_<slug>"` or
   `"fpl_ch5_<slug>"`, both registered in
   `vendor_lut/source_manifest.json` (two new top-level sources, one URL
   per source-id family).
3. Add a test asserting every library entry's `source_id` exists in
   `source_manifest.json` — catches drift.
4. Dedup if a species appears in both Wood Database and FPL — prefer FPL
   (peer-reviewed) and reference both source_ids via a `secondary_sources`
   slot.

**Sub-phase E.4 — GUI integration**

1. In `crates/rs_cam_viz/`, surface `WOOD_SPECIES_LIBRARY` as a searchable
   dropdown in the material picker. Selecting a library entry stores
   `Material::SolidWoodByJanka { ... }` in the project file.
2. Keep the existing `WoodSpecies` enum dropdown for the "well-known"
   first-class species. Order: enum first, library below a separator.

**Sub-phase E.5 — Bundle audit deferral S3-13**

Fold in the `Material::test_fixture_custom()` helper while we're touching
material.rs.

**Verification gate:** standard four checks. Literature_parity sentries
should grow by 1 (the helper-vs-enum cross-check). All existing wood-related
tests must stay green — particularly `solid_wood_jankas_match_repo_anchored_values`
and the Janka anchor sentries.

**Definition of done:** parametric variant lives in `Material`, library has
~140 species, GUI lets the user pick one, project files round-trip.

---

## Phase F — Final verification gate (re-run baseline)

**Goal:** prove the Phase A→E changes either improved or held the runtime
behavior; no regressions in smoke or sweep diff.

**Steps:**

1. Re-run MCP smoke AS001–AS015 (operator). Log under "Phase F final
   (post-completion)" section of `kc_retune_log.md`. Compare per-case
   peak µm against Phase A baseline.
2. Re-run `param_sweep` battery. Diff `target/param_sweeps/` against the
   Phase A snapshot:
   ```bash
   cargo test --test param_sweep 2>&1 > target/sweep_final_completion.log
   python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/
   # Diff verdict counts vs sweep_snapshot_5a67c1d.tar.gz
   ```
3. Write `planning/feeds_data_ingest_completion_results_2026-MM-DD.md`
   capturing the diffs, any regressions, and the final 10/10 ledger.

**Definition of done:**

- All AS001–AS015 cases ≤ baseline peak µm (or have a per-case justification).
- Sweep verdict counts: 0 regressions vs baseline (improvements OK).
- All cargo + clippy + literature_parity gates green.
- `planning/feeds_data_ingest_completion_results_2026-MM-DD.md` written.

---

## Things this plan deliberately does NOT do

- **Restructure `Material` as a trait** — tempting (Material::Custom and
  SolidWoodByJanka have overlap), but a real trait redesign would block
  every other phase. Audit deferral S2-8 records this for future
  consideration.
- **Move wood Kc formula provenance to a primary citation** — the current
  Janka→Kc regression is folklore-grade. Replacing with a Pałubicki-type
  primary source would be a separate research round. Out of scope here.
- **Per-spindle gate for industrial Freud rows** — operator decision
  2026-05-31. Parked.
- **Promote PMMA Grade B Kc** — staging author's caveat stands.
- **Touch `MachineKinematics::shapeoko_xxl_ricky_tuned()`** — operator rule.
- **Stage `crates/rs_cam_core/src/feeds/explain.rs` or
  `crates/rs_cam_viz/src/ui/feeds_modal.rs`** — operator rule.

## Constraints carried forward from prior sessions

- Don't run `cargo test --workspace` (terminal infinite loop) — use
  `-p rs_cam_core --lib` / `--tests`.
- Don't run cargo test during a release build of rs_cam_viz (swap thrash)
  — check `pgrep -af "cargo --release|rs_cam_gui"` first.
- Don't use `cargo fmt` workspace-wide (rustfmt cascades).
- Don't skip clippy/test with `--no-verify`.
- F-034 / F-036c tolerances stay widened.
- Zero-warning clippy (16 deny lints).

## Resume note for the next session

The next session should:

1. Read this doc.
2. Read `planning/tool_kinematics_chipload_audit_2026-05-31.md` for the
   audit context that feeds the deferrals.
3. Read `planning/feeds_data_ingest_phase4_2026-05-31.md` for the Phase 4
   shape (architectural patterns to mirror).
4. **Start with Phase A** — empirical baseline. The operator step (MCP
   smoke) is the bottleneck; the autonomous step (`param_sweep` baseline
   capture) can run while waiting for operator availability.
5. Proceed phase-by-phase; each phase ends with a commit and a TaskList
   reset.

After Phase F lands, the ledger is 10/10 modulo the explicitly out-of-scope
items above.
