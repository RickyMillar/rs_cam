# Feeds/Speeds & Tool-Load Data Ingest — Consolidation & Validation (2026-05-29)

Consolidation of the multi-agent data collection driven by
`planning/feeds_data_source_acquisition_2026-05-29.md` (the source list) against
`planning/feeds_data_coverage_audit_2026-05-29.md` (the gap analysis).

Raw collection + provenance lives in `planning/data_ingest_2026-05-29/`:
`amana.json`, `onsrud_whiteside.json`, `harvey_helical_garr.json`, `kc.md`,
`hardness.md`, each with a `_provenance.md` (verbatim source quotes) and a
`_gaps.md` (unreachable / paywalled sources). Every numeric value carries a
verbatim quote from a fetched source — nothing was invented or estimated; that
discipline is the whole point for a safety-critical LUT.

---

## 1. What was collected

| Slice | Rows / values | Grade | Notes |
|-------|---------------|-------|-------|
| Amana per-series LUT | 32 rows | A | V-groove 18/30/45/60/90°, engraving 15/30/45/120°, compression spirals, plastic O-flute, ZrN aluminum O-flute |
| Onsrud + Whiteside LUT | 14 rows | A (Onsrud) / B (Whiteside RPM-only) | plastics breadth (HDPE/PC/acrylic/Delrin by CED); Whiteside V-groove/spiral RPM |
| Harvey/Helical/Garr LUT | 12 rows | A | Garr aluminum %-of-D + engagement rules; 2 Helical 6061 points; Harvey calculator-gated (gap) |
| Specific cutting force Kc | ~9 values | A (metals) / B (wood, plastics) | see §3 |
| Hardness | 24 entries | A | Janka (14), Shore D/Rockwell M (8), Brinell (2) |

## 2. LUT validation results (`/tmp/validate_lut.py` against the real `VendorObservation` schema)

58 rows total, all JSON-parseable. Validated against the actual required
(non-`Option`) fields, enum membership, and range sanity. Findings:

- **Schema bug — missing required `diameter_mm` (19 rows):** every V-groove and
  engraving row omits `diameter_mm`, which is a required `f64` (no serde
  default) in `crates/rs_cam_core/src/feeds/vendor_lut.rs`. As-is they would
  fail to deserialize and `embedded()` would silently drop the whole file.
  Repair (real per-angle cutting diameters from the source charts) is being
  fetched by the verifier agent — see `verification_report.md` when it lands.
- **Missing required `flute_count` + invalid `pass_role` (10 Garr rows):** Garr
  aluminum rows used operation tokens (slot/profile) in `pass_role` and omitted
  `flute_count`. These are aluminum → staged anyway (see below).
- **Vendor enum (2 Helical rows):** `helical` is not a `Vendor` enum variant.
  Aluminum → staged.
- **1 Onsrud "polycarbonate optimum window" article row** is a narrative
  datapoint without a diameter — keep as provenance, not a LUT row.

### Promotion decision

The live `embedded()` LUT feeds the **calibrated** tool-load gates (the
acceptance loop is closed at 7/7 — see `CLAUDE.md`). New chipload rows change
which row a query matches, which can shift verdicts. So promotion is gated on
the full `rs_cam_core` test suite (incl. `tests/_f0*.rs` acceptance
regressions) staying green.

- **Live-promotable now (wood, schema-complete, has chipload): 5 rows** — Amana
  compression spirals (6.35 mm wood/MDF/plywood, 12.7 mm wood/MDF). *(Pending
  the verifier's CONFIRM on the sampled chiploads before wiring.)*
- **Promotable after diameter repair: ~15 rows** — V-groove/engraving **wood**
  rows (the angle-keyed `chamfer_vbit` data that `find_best_vbit_row` was built
  to consume). Blocked only on `diameter_mm`.
- **Staged, not live (inert until `Material` enum extends): 19 rows** — all
  plastics (acrylic/HDPE/PC/Delrin) and aluminum rows. The `Material` enum is
  wood-only, so the matcher can never reach these for current queries. They sit
  validated in `planning/data_ingest_2026-05-29/` until plastics/aluminum
  materials exist. The Garr/Helical schema gaps are resolved at that time.

## 3. Specific cutting force (Kc) — FLAGGED, no live change

**The single most safety-relevant finding: the repo's wood/EWP Kc constants are
3–12× LOWER than measured literature.** Detail + verbatim quotes in
`planning/data_ingest_2026-05-29/kc.md`.

| Material | Repo `kc_n_per_mm2()` | Measured (Grade B) | Ratio | Source |
|----------|----------------------|--------------------|-------|--------|
| Particleboard | 9.0 | 32.0–37.6 | 3.6–4.2× low | Pałubicki 2021, DOI 10.3390/ma14092208 |
| MDF | 10.0 | 31.44 | 3.1× low | PMC6315737 round-shape Ks |
| HDPE | 4.0 (generic plastic) | 33.85–46.89 | 8.5–11.7× low | Yang 2022, DOI 10.3390/polym14010189 |

Metals (for any future support): C15 steel kc1.1 1639.05 / mc 0.25; H13 2450 /
mc 0.23. Aluminum N-group only a 350–1350 range (Sandvik removed the numeric
table; no fetched pair → gap). Plastics beyond HDPE: PMMA force study found but
not back-calculable from a fetched figure; POM/Delrin paywalled; **PC has no
primary machining-force study at all**.

**Recommendation — do NOT blind-swap the constants.** The gap is real (the repo
tracks shear-parallel-to-grain strength ≈6–16 MPa, not milling specific cutting
force, and milling Kc is edge-radius/size-effect inflated 3–4× at low chip
thickness). But raising Kc 3–4× multiplies predicted power/deflection by the
same factor and would flip many currently-`Within` cuts to `Exceeds`, breaking
the calibrated acceptance loop. The correct path is a **separate, calibrated
re-tuning**: bump Kc toward the measured values AND re-run the acceptance smoke
suite + the deflection/power goldens together, adjusting the 2.5× anisotropy
multiplier and verdict bounds as one coordinated change. Track as its own task.

## 4. Hardness — FLAGGED, no live change

Detail in `planning/data_ingest_2026-05-29/hardness.md`. Janka verification
against The Wood Database + FPL + PreciseBits:

- **RadiataPine: repo 500 vs sources 710–750** — repo looks low.
- **SouthernYellowPine: repo 690 vs Longleaf 870** — repo below any published
  SYP member; may be tracking loblolly (no Grade-A loblolly fetched). Ambiguous.
- Jarrah (1910 vs 1860), Ipe (3510 vs 3490): minor, within source-edition
  rounding.
- Hard Maple / Walnut / Yellow Birch / White Oak: match.
- 6 additional species available to add (red oak, cherry, poplar, ash,
  mahogany, douglas-fir), all Grade A.
- Plastics Shore D / Rockwell M (HDPE 64, PC 80 / RM75, Delrin 86 / RM89,
  acrylic RM93) and aluminum Brinell (6061-T6 95, 7075-T6 150) collected — but
  the repo models plastics with a single flat `hardness_index`/`kc`, and has no
  aluminum material, so these are additive reference only.

**Recommendation:** the Janka values feed `hardness_index()` → feeds, not the
gates directly, so the risk is lower than Kc — but it still shifts feeds and
should be changed with a feeds-sweep check, not blindly. Correcting RadiataPine
500→~710 and clarifying the SYP species are the two worthwhile fixes; do them
with a feeds regression check.

## 5. Other structural findings

- **`Material` enum is wood-only** — the hard blocker on realizing any value
  from the plastics (Onsrud's decisive breadth) and aluminum data. Extending it
  (+ per-family Kc/hardness) is the prerequisite that unlocks 19 staged rows
  plus the plastics/aluminum Kc and hardness above.
- **DOC-derating rule is unmodeled.** Amana AND Onsrud independently state the
  same rule ("1×D recommended; 2×D −25%; 3×D −50%"). Two Grade-A vendors
  agreeing makes this a safe, data-independent modeling addition: derate the
  matched chipload by the sample's DOC/diameter ratio. Captured on every
  collected row's `ap_rule`.
- **`Vendor` enum** needs a `Helical` variant before the 2 Helical rows can
  load (deferred with the aluminum work).

## 6. Provenance / honesty ledger

- Every staged value has a verbatim source quote in its `_provenance.md`.
- Gaps logged honestly in the `_gaps.md` files: Harvey (calculator-gated),
  Sandvik aluminum numeric table (removed from site / 403), POM-C papers
  (paywalled), PMMA force figure (403), PC Kc (no primary source), MatWeb
  (login/403, worked around via named supplier datasheets).
- CREDITS.md / `source_manifest.json` are updated only for sources whose rows
  are actually bundled live (see commit), not for staged-but-unused data.
