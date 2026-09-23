# Investigation plan — why the feeds calculator reads 3x to 7x low in wood

Opened 2026-09-19. Not started. Evidence is in `FINDINGS.md` (addendum 2) and
`../load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md`.

## The question

The calculator shipped an achieved chipload of 0.0187 mm/tooth on a 3.209 mm
effective diameter in Baltic birch. Five publishers put that tool between
0.0279 and 0.1270 mm/tooth. We are 1.5x low against the most conservative
chart and 4x to 7x low against the mainstream ones. A V-bit operation is 2.3x
to 5.3x below the nearest published V-bit band.

This is not a tuning difference. It is large enough to be a defect.

## Rule for the whole investigation

Do not change a number to close the gap. Find the step that produces it,
then decide whether that step is right. A gap of this size usually comes from
one bad multiplier, not from many small ones.

## Hypotheses, cheapest test first

### H1 — the vendor LUT row is wrong or misread
Row `amana-tapered-hardwood-scallop-3175-2f`, maximum 0.0187 mm/tooth.

**Amendment (2026-09-19, verified):** 0.0187 is the POST-DERATE maximum.
The raw row in
`crates/rs_cam_core/data/vendor_lut/observations/amana_3d_profiling.json`
publishes **0.012–0.024 mm/tooth** (Ø3.175, 2F tapered ball, scallop /
semi-finish, Janka 1450, evidence grade A, source URL + access date).
So the comparison target is **0.024**, not 0.0187 — and the gap
pre-derates is only 1.16x against the most conservative chart
(0.0279). The derating chain (H2) and the band-as-limit clamp (H4)
account for the rest of the 1.5–7x. The test is still "compare the
stored row against what Amana publishes", but against the raw band.

Test: fetch the Amana ZrN 3D Profiling chart PDF and compare the
stored row against it. `CREDITS.md` names the dataset origins.

This is the first test because the modulator clamps to this row. Every other
hypothesis is downstream of it.

### H2 — the derating chain compounds
The reported lookup applied a diameter scale of 1.0059, a hardness scale of
1.0992, a SemiFinish pass role in place of the requested Finish, and a DOC
derate. Each is small. The product may not be.

Test: instrument one lookup. Print the value after every stage from the raw row
to the shipped band. Name the stage with the largest single drop.

### H3 — the 0.025 mm chip-formation floor may not hold
No vendor publishes a minimum chip thickness for wood. The only published
rubbing threshold found anywhere is Ingersoll's 0.004 in for carbide in METAL,
which is above the entire published wood band for a 1/8 in bit.

**Amendment (2026-09-19):** the constant DOES carry named citations in code —
`feeds/mod.rs:937` cites "Onsrud min-chip-thickness rule, GWizard minimum
chipload, FPL Wood Handbook chip-formation regime", and the floor was
already subordinated to band ceilings by `effective_rubbing_floor` (ruled
2026-08-06, FEEDS_CENSUS C-12). The test is therefore **verify those three
citations are real sources that say what the doc claims**, not "find where
it entered". If none holds, the burnishing warning fires on an unsourced
number, and the warning is what drove this whole review.

Test: verify each citation. If it cites nothing that holds, the warning is
unsourced.

### H4 — the modulator treats a band MAXIMUM as a LIMIT
A vendor band is a recommended range. The modulator clamps the feed to the top
of it, so a commanded 1050 mm/min delivered an achieved 0.0187 mm/tooth, the
same value a commanded 782 mm/min delivered. The operator has no lever.

Test: read the clamp site. Decide whether a band maximum should bind a feed at
all, or only raise a diagnostic.

### H5 — the LUT has no coverage for most tool series
Onsrud's own numbers vary by 2x at one diameter and one material: series 40-000
reads .006 - .008 in/tooth against series 52-200's .003 - .005. A single
chipload per diameter and material cannot describe the published data.

**Amendment (2026-09-19):** the SCHEMA supports series — `LookupQuery`
carries `tool_subfamily` and the lookup scores it (`vendor_lut.rs:531`), and
rows are per-series observations with evidence grades. The hypothesis as
originally written ("the LUT keys on diameter and material but not on tool
series") is FALSE. The real weakness is **coverage sparsity**: only 4 Onsrud
rows ship, so most tools at a diameter resolve to a row from a different
series, and the error bar belongs in the band.

Test: measure the coverage — for each shipped LUT family/diameter, how many
distinct series exist in the published charts vs in the LUT. Quantify how far
a typical query's matched row is from its own series. Coverage, not schema.

### H6 — the chip-thinning correction is metal practice
No wood tooling vendor recommends a feed increase for chip thinning. Every
source is metalworking. DAPRA's published tables reproduce both formulas used
in `FINDINGS.md` exactly, so the FORMULAS are right. Their application to wood
is unvalidated.

**Amendment (2026-09-19):** the 2026-08-19 G-CHIPTHIN-HALFFIX ruling already
deleted the chip-thinning MULTIPLICATION from Suggest and the gate
("OBSERVED, NOT APPLIED", `feeds/mod.rs:551+`). The correction survived in
ONE place: `dressup/feed_modulation.rs` still divides its BandMid target by
`sqrt(radial_woc_fraction)` (the `band_mid_feed_for_move` thinning term). There
are therefore **two chip-thinning policies alive in the repo**, and this
hypothesis should be scoped to that residue plus the literature question.

Test: literature only. Look for wood-machining work on chip thinning. Record
the absence if there is one. Do not remove the correction on no evidence.

### H7 — a V-bit curve pass has no vendor rows at all
`feeds.no_vendor_rows_for_routed_operation` fires: the LUT publishes no V-bit
contour rows, so the recommendation is entirely formula-derived and the
chipload gate reports Unmodeled. Amana publishes 15/30/45/60/90/120 degree
rows but no 20 degree row anywhere.

Test: decide whether a formula-derived V-bit recommendation should ship at all,
or whether the absence should be louder than a caution.

## Two defects found on the way, both separate from the gap

1. **Feed modulation does not run on a `project_curve` operation.** Toolpath 17
   carries a `modulation_summary`; toolpath 19 carries none. A 73 degree
   descent therefore runs at the full lateral feed and emits 1165 mm/min of
   vertical motion onto a 20 degree V point. Severity critical.
   **Status (2026-09-19): FIXED** — bandless modulation path, see
   `IMPLEMENTATION_PLAN.md` work item A and FINDINGS.md addendum 3.
   Acceptance re-sim pending the GUI rebuild/restart.
2. **A plunge rate cap exists for a small ball tip (300 mm/min) but no
   equivalent cap governs an untagged steep descent.** The cap guards the
   motion the planner labels a plunge and ignores the motion that behaves like
   one.
   **Status (2026-09-19): SAME FIX** — the geometric Phase 3 guard now
   runs for bandless ops; both defects shared the one root cause
   (the guard was unreachable without a vendor band).

## Order of work

1. H1. If the row is wrong, stop and re-measure everything against a fixed row.
2. H4. It decides whether the operator has a lever at all.
3. H2 and H3 together. They share the instrumented lookup.
4. The two defects above. They are bounded and do not wait on the gap.
5. H5, H7, then H6.

## What closes this

A worked example: one tool, one material, one operation, from the published
vendor figure to the shipped feed, with every multiplier named and every one
either cited or deleted. Put it beside a published chart row and show the
result inside the published band, or state in one sentence why it should sit
outside it.
