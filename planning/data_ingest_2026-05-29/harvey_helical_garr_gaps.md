# Gaps — Harvey / Helical / Garr slice (2026-05-29)

Sources that could NOT yield a citeable row because the actual tabulated numbers are
calculator-gated, download-gated, or only documentary (geometry, not feeds). Per the
overriding rule, none of these produced a row.

---

## VENDOR ENUM FLAG (action item for consolidation)

`source_vendor: "helical"` is used on 2 rows in `harvey_helical_garr.json` but **Helical is
NOT in the repo Vendor enum**. Current enum (`crates/rs_cam_core/src/feeds/vendor_lut.rs`):

```
Amana | Onsrud | Harvey | Whiteside | Sandvik | Garr | Autodesk | Carbide3d
```

Add a `Helical` variant (snake_case "helical") before ingesting the two
`helical-h45al-6061-*` rows, or they will fail JSON deserialization at load
(`vendor_lut.rs` `load_*` via `include_str!`). No other slice value needs an enum change
(Harvey and Garr are both present).

---

## HARVEY TOOL — entirely calculator/download-gated (GAP, no rows)

Harvey's speeds & feeds page (`https://www.harveytool.com/resources/speeds-feeds`) and
every product page checked are landing/index pages only. Verbatim: *"Below you will find
downloadable and printer-friendly Speeds & Feeds for each one of our products!"* and the
page promotes **Machining Advisor Pro (MAP)**. No SFM / chip-load / DOC numbers are
rendered on any HTML page. Per the rule (calculator-gated / per-tool downloadable PDF =
GAP), Harvey produced **0 rows**.

Documentary geometry facts confirmed (NOT feeds rows, recorded only to correct the
acquisition doc):
- Miniature End Mills - Ball - Stub & Standard - **Metric** product page states cutter
  diameter **"down to 0.5mm"** (NOT 0.05mm as the acquisition doc A4 claimed — the 0.05mm
  figure could not be verified on the actual page; treat A4's ".05mm" as an error).
- Miniature End Mills - Ball (inch) line: smallest cutter diameter **".002\""** per the
  product title "Ball Nose Miniature End Mills – Small as .002\"".
- Tapered Ball miniature end mills start at **0.015"** cutter diameter (e.g. .015" CD x 2°
  tapered neck x .500" taper reach), confirmed via product listings.

To close Harvey: would need to download per-tool-line Standard Speeds & Feeds PDFs (one
per product line, tied to tool numbers) and OCR/parse them, or run MAP per tool/material.
Neither yields a directly-readable table from a single fetch. Recommend a dedicated
PDF-harvest pass against `harveytool.com` product "Speeds & Feeds" tab downloads if the
small-diameter aluminum/plastics grid must be closed from Harvey specifically.

## HELICAL SOLUTIONS — per-tool charts calculator/download-gated (partial GAP)

`https://www.helicaltool.com/resources/speeds-feeds`: *"Helical supplies comprehensive
speeds and feeds charts for every product in its catalog. View charts by clicking the
Speeds & Feeds tab on each product table."* — i.e. per-tool downloadable PDFs + MAP. No
directly-readable tabulated SFM/chip-load on the HTML. The per-product aluminum/tapered
S&F charts are therefore a **GAP**.

What WAS captured from Helical (not gapped): the Machining Guidebook 2016 PDF, which is a
clean readable download — chip-thinning rule + formula (p.22), progressive/HEM RDOC rules,
inside-corner feed-reduction rules, and the two tabulated page-25 6061 worked examples
(both ingested as rows). These do not cover the per-diameter aluminum chip-load grid that
the per-tool charts would.

To close Helical's per-tool grid: harvest the per-product "Speeds & Feeds" PDF downloads
from each aluminum / tapered-ball (HTPR-4/HTPR-5) product page, or run MAP. Same gating as
Harvey.

## GARR — broad technical PDF chip-thinning FORMULA is glyph-garbled (rule text OK)

`https://www.garrtool.com/wp-content/uploads/2018/11/TECHNICAL.pdf` "Chip Thinning
Calculation:" block (around the page-289 area) extracted as unreadable replacement glyphs
(custom font, no Unicode mapping) — the actual equation could not be read verbatim from
this PDF. NOT a row (it is a rule, and the formula is unreadable here). HOWEVER the
equivalent chip-thinning formula IS captured verbatim from the Helical guidebook p.22, so
consolidation has a usable form:
`IPT = (CT x D) / (2 x sqrt(D x RDOC) - RDOC^2)`.
The Garr plunge/engagement rule on the same PDF *did* extract cleanly and is recorded in
provenance: *"When plunging into a solid, drop feed by approximately 50%. 20% of diameter
for basic engagement parameters."*

The Garr aluminum-specific guide (`TECH_MILLING_ALUMINUM.pdf`) extracted fully and is the
source of 10 of the 12 rows — no gap there.

---

## Coverage summary

| Target | Status |
|--------|--------|
| Small-diameter <3 mm (Harvey 0.05mm/0.015") | GAP — all behind PDF/MAP; A4's 0.05mm figure is wrong (actual 0.5mm metric / .002" inch). Garr 3.0 mm rows captured as the small-diameter floor. |
| Aluminum end-mill SFM + chipload (Helical, Garr) | CAPTURED — Garr 242M/142M/A3 + General Purpose; Helical guidebook 6061 worked examples. |
| Garr aluminum %-of-diameter + ap=.5x/1xD + ae=.5xD | CAPTURED — exact metric mm chiploads + ap/ae rules. |
| Chip-thinning RDOC<50% rule (Helical p.22 + Garr) | CAPTURED as rule text + formula in provenance. |
| Plastics (specific polymers: acrylic/HDPE/PC/Delrin) | PARTIAL — only Garr's coarse "Fiberglass/Plastics/G10" non-ISO bucket (1 generic plastic row). No per-polymer Harvey/Helical/Garr data was readable. |

## Phase 4 promotion deferral (2026-06-01)

**`garr-gp-plastics-6000-flat`** — the one Garr "Fiberglass/Plastics/
G10" row cannot be promoted to the live LUT because its
`material_family: plastic` does not match any variant of the
`MaterialFamily` enum
(`{Softwood, Hardwood, PlywoodSoftwood, PlywoodHardwood, Mdf, Hdf,
  Particleboard, Acrylic, Hdpe, Polycarbonate, Delrin, Aluminum}`).
Fiberglass / G10 is a fundamentally different cutting class
(abrasive, fiber-reinforced) and warrants its own enum variant
rather than collapsing to one of the existing polymer entries.

**Follow-up:** add `MaterialFamily::Fiberglass` (or
`CompositeFiberReinforced`) in a Phase 5 schema bump. Until then
the row stays in this staged JSON unpromoted.

---

### Status (2026-06-01, Phase 5 Step 5.3) — CLOSED

`MaterialFamily::Fiberglass` added to `vendor_lut.rs` and
`Material::Fiberglass { grade: FiberglassGrade }` lifecycle added
to `material.rs` (commit `6c56b95`). Fiberglass gets its own
`material_category` (3) in the matcher — never extrapolates onto
polymer / metal rows.

The deferred row promoted live as `garr-gp-fiberglass-6000-2f-flat`
in new file `garr_fiberglass.json`. Renamed from the staged
`-plastics-` ID to reflect the high-confidence interpretation
(Garr's chart collapses Fiberglass + Plastics + G10 onto one row;
we promote only the fiberglass class). Material handbook quotes
for G10 Kc (~100-150 N/mm²) are NOT seeded into
`Material::Fiberglass::kc_n_per_mm2()` — that returns `None`
(refuse-first; no workshop-fidelity peripheral-milling measurement
available). Other accessors carry conservative carbide-tool
placeholders.

Consolidation report:
`planning/feeds_phase5_consolidation_2026-06-01.md`.

## Phase 4 promotion success (2026-06-01)

Successfully promoted to `crates/rs_cam_core/data/vendor_lut/
observations/garr_aluminum.json`:
- garr-142m-alum-slot-6000-flat-2f (was -flat, +flute_count=2)
- garr-142m-alum-profile-6000-flat-2f (was -flat, +flute_count=2)
- garr-gp-alum-6000-flat-2f (was -flat, +flute_count=2)
- garr-gp-alum-3000-flat-2f (was -flat, +flute_count=2)

`pass_role` typo `"rough"` → `"roughing"` fixed in-place in the
staged JSON file (matches the Rust ROLE enum variant
`PassRole::Roughing`).
