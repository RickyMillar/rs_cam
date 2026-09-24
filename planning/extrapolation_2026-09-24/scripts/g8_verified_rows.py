#!/usr/bin/env python3
"""G8 reconciler: write fetch/G8/verified_rows.json from candidate_rows.json.

The verifiers confirmed all 14 candidate rows (2026-09-24). No row is
"wrong", "not_found" or "grade_wrong". This script copies each confirmed
row, adds a "verification" field, and corrects one wording defect the
Spektra verifier found in ap_rule. It reads no LUT file and writes one file.
"""
import copy
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G8 = ROOT / "fetch" / "G8"

ZRN_PDF = "5cdfb9c01ec218e9db992fee90855f005a02902f49a3897c448188cd7cd4479b"
SPK_PDF = "84feaace79e13a5bc12e652c58cf237f1a242a623e6fb8bc9d3f5883d5153497"

ZRN_NOTE = {
    6350: "Page 2, '3 Flute Extra Long / Ball Nose & Flat Bottom' block, 1/4\" column, "
    "Wood, MDF, Sign-Foam row: 0.004\" - 0.006\" at 18,000 RPM, 1 x D DOC. "
    "0.1016-0.1524 mm is correct (x25.4). The verbatim occurs once in the text.",
    9525: "The 3/8\" column prints 0.005\" - 0.007\" (IPM 270\" - 370\"). 0.127-0.1778 mm "
    "is correct. The standard 3-flute ball 3/8\" column on page 1 prints 0.006\" - 0.008\".",
    12700: "The 1/2\" column prints 0.006\" - 0.008\" (IPM 320\" - 430\"). 0.1524-0.2032 mm "
    "is correct. The standard 3-flute ball 1/2\" column on page 1 prints 0.007\" - 0.009\".",
}

COMMON_CAVEATS = (
    "The chart prints one row 'Wood, MDF, Sign-Foam'. The softwood/MDF split is the LUT "
    "convention for this chart. The chart prints no hardwood row, so no hardwood row "
    "follows from it. hardness_value, operation_family and pass_role are LUT conventions; "
    "the chart does not print them."
)


def main() -> None:
    cand = json.loads((G8 / "candidate_rows.json").read_text())["observations"]
    out = []
    for row in cand:
        r = copy.deepcopy(row)
        diam_key = int(round(r["diameter_mm"] * 1000))
        if r["source_id"] == "amana_zrn_3d_profiling_v8":
            note = ZRN_NOTE[diam_key]
            if r["tool_family"] == "flat_end":
                note += " The block heading is 'Ball Nose & Flat Bottom', so one printed cell serves both families."
            r["verification"] = {
                "verdict": "confirmed",
                "verified_on": "2026-09-24",
                "url_reachable": True,
                "pdf_sha256_matches": True,
                "pdf_sha256": ZRN_PDF,
                "stored_text_identical_to_fresh_pdftotext": True,
                "note": note,
                "caveats": COMMON_CAVEATS
                + " Verifier: the block-reference list in notes leaves out 46591, which the "
                "chart prints in the block; 46597 is on the heading line, so its place in the "
                "block is not certain.",
            }
        elif r["source_id"] == "amana_spektra_3d_profiling_v6":
            old = r["ap_rule"]
            r["ap_rule"] = (
                "1xD use recommended chip load; 2xD reduce chip load by 25%; "
                "3xD reduce chip load by 50%"
            )
            r["verification"] = {
                "verdict": "confirmed",
                "verified_on": "2026-09-24",
                "url_reachable": True,
                "pdf_sha256_matches": True,
                "pdf_sha256": SPK_PDF,
                "stored_text_identical_to_fresh_pdftotext": True,
                "note": "Page 2, block '3 Flute Extra Long Ball Nose & Flat Bottom', 1/4\" "
                "(0.250\"), 46490-K, 'Based on 18,000 RPM': 'Wood, MDF, Sign-Foam 215\" - 320\" "
                "0.004\" - 0.006\"'. 0.1016-0.1524 mm is correct. The IPM column agrees: "
                "18000 x 3 x 0.004 = 216 and 18000 x 3 x 0.006 = 324 (derived check).",
                "caveats": COMMON_CAVEATS,
                "correction": {
                    "field": "ap_rule",
                    "was": old,
                    "now": r["ap_rule"],
                    "why": "The chart prints '1 x D Use recommended chip load / 2 x D Reduce "
                    "chip load by 25% / 3 x D Reduce chip load by 50%'. The candidate said "
                    "'feed rate'. The reconciler set the text to what the chart prints.",
                },
            }
        else:
            raise SystemExit(f"unexpected source {r['source_id']}")
        out.append(r)
    doc = {
        "extrapolation_group": "G8",
        "reconciled_on": "2026-09-24",
        "rule": "Only rows a verifier confirmed. Dropped: none (0 wrong, 0 not_found, "
        "0 grade_wrong of 14).",
        "observations": out,
    }
    (G8 / "verified_rows.json").write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"wrote {len(out)} rows")


if __name__ == "__main__":
    main()
