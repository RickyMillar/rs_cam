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

Test: find the source of that row. Compare the stored value against what Amana
publishes for that tool. `CREDITS.md` names the dataset origins.

This is the first test because the modulator clamps to this row. Every other
hypothesis is downstream of it.

### H2 — the derating chain compounds
The reported lookup applied a diameter scale of 1.0059, a hardness scale of
1.0992, a SemiFinish pass role in place of the requested Finish, and a DOC
derate. Each is small. The product may not be.

Test: instrument one lookup. Print the value after every stage from the raw row
to the shipped band. Name the stage with the largest single drop.

### H3 — the 0.025 mm chip-formation floor has no source
No vendor publishes a minimum chip thickness for wood. The only published
rubbing threshold found anywhere is Ingersoll's 0.004 in for carbide in METAL,
which is above the entire published wood band for a 1/8 in bit.

Test: find where 0.025 entered the code and what it cites. If it cites nothing,
the burnishing warning fires on an unsourced number, and the warning is what
drove this whole review.

### H4 — the modulator treats a band MAXIMUM as a LIMIT
A vendor band is a recommended range. The modulator clamps the feed to the top
of it, so a commanded 1050 mm/min delivered an achieved 0.0187 mm/tooth, the
same value a commanded 782 mm/min delivered. The operator has no lever.

Test: read the clamp site. Decide whether a band maximum should bind a feed at
all, or only raise a diagnostic.

### H5 — the LUT keys on diameter and material but not on tool series
Onsrud's own numbers vary by 2x at one diameter and one material: series 40-000
reads .006 - .008 in/tooth against series 52-200's .003 - .005. A single
chipload per diameter and material cannot describe the published data.

Test: check what the row key holds. If series is absent, the LUT cannot be
right for every tool at a diameter, and the error bar belongs in the band.

### H6 — the chip-thinning correction is metal practice
No wood tooling vendor recommends a feed increase for chip thinning. Every
source is metalworking. DAPRA's published tables reproduce both formulas used
in `FINDINGS.md` exactly, so the FORMULAS are right. Their application to wood
is unvalidated.

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
2. **A plunge rate cap exists for a small ball tip (300 mm/min) but no
   equivalent cap governs an untagged steep descent.** The cap guards the
   motion the planner labels a plunge and ignores the motion that behaves like
   one.

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
