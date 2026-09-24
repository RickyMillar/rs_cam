#!/usr/bin/env python3
"""G2 reconciler: write fetch/G2/verified_rows.json from candidate_rows.json.

Inputs: fetch/G2/candidate_rows.json (101 rows) and fetch/G2/verifier_verdicts.json
(the verifiers' verdicts of 2026-09-24). The script keeps a row only when a
verifier confirmed it. It drops "wrong" and "not_found" rows. A "grade_wrong"
row becomes derived / grade c with a note. The verifiers found no wrong,
not_found or grade_wrong row, so 91 rows pass. The 10 rows of the two sources
that no verifier checked (Amana PCD ball v2, Spektra 3D v6) stay out.

Two wording corrections (G8 precedent, recorded in a "correction" field):
- 37-00/37-20 notes: the tip range is 0.005-0.090 in for 37-00 (PCT-19 also
  lists 37-11 at 0.060 and 37-15 at 0.090) and 0.005-0.040 in for 37-20.
- Amana v7 ap_rule: the printed text replaces the paraphrase.

The script reads no LUT file and writes one file.
"""
import copy
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G2 = ROOT / "fetch" / "G2"

PDF_SHA = {s["source_id"]: s.get("pdf_sha256") for s in json.loads((G2 / "sources.json").read_text())}

COMMON_CAVEATS = {
    "onsrud": (
        "pdf_sha256 is the sha of the PDF, not of the stored text; the stored text adds 2 "
        "comment header lines to the pdftotext -layout output. The sheet prints the chipload "
        "cell, the Cut column and the RPM footnotes. It does not print flute_count, "
        "included_angle_deg, the part at each size or hardness_value: those come from the "
        "PCT-19 catalog excerpt or the engine's Janka proxy. operation_family trace / "
        "pass_role finish is the LUT convention for chamfer_vbit rows, not a printed value."
    ),
    "amana_ball_nose_v7": (
        "pdf_sha256 is the sha of the PDF, not of the stored text; the G2 stored text adds 2 "
        "header lines to a body identical to the LUT copy. hardness_value, operation_family "
        "pocket and pass_role roughing are programme metadata, not printed values."
    ),
}

NO_CATALOG_PART = {
    # (series, diameter key) printed on the sheet with no PCT-19 part at that size
    ("37-60", 9525): "printed at 3/8 in; PCT-19 lists no 37-60 part at 3/8 in",
    ("37-60", 14288): "printed at 9/16 in on the Laminated Chipboard table only; no part; probable layout error",
    ("37-60", 22225): "printed at 7/8 in on the Laminated Chipboard table only; no part; probable layout error",
    ("37-80", 31750): "printed at 1 1/4 in; PCT-19 lists no 37-80 part at 1 1/4 in",
    ("37-80", 50800): "printed at 2 in; two parts exist (37-92 120 deg, 37-97 140 deg); the angle is not unique",
}

TIP_OLD = "0.005-0.040 in tip"
TIP_NEW = "tip of 0.005-0.090 in on 37-00 (60 deg; PCT-19 37-01 to 37-15) and 0.005-0.040 in on 37-20 (30 deg)"

V7_AP_RULE = (
    "printed: '1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / "
    "3 x D Reduce feed rate by 50%'"
)


def series_of(row: dict) -> str | None:
    sub = row.get("tool_subfamily", "")
    for s in ("37_00", "37_50", "37_60", "37_80"):
        if s in sub:
            return s.replace("_", "-")
    return None


def main() -> None:
    cand = json.loads((G2 / "candidate_rows.json").read_text())["observations"]
    verdicts = json.loads((G2 / "verifier_verdicts.json").read_text())
    by_id = {}
    src_info = {}
    for v in verdicts["verdicts"]:
        src_info[v["source_id"]] = v
        for r in v["rows"]:
            by_id[r["observation_id"]] = r

    out, dropped, unverified = [], [], []
    for row in cand:
        v = by_id.get(row["observation_id"])
        if v is None:
            unverified.append(row["observation_id"])
            continue
        if v["verdict"] in ("wrong", "not_found"):
            dropped.append((row["observation_id"], v["verdict"]))
            continue
        r = copy.deepcopy(row)
        src = src_info[r["source_id"]]
        ver = {
            "verdict": v["verdict"],
            "verified_on": "2026-09-24",
            "url_reachable": src["url_reachable"],
            "pdf_sha256_matches": src["hash_matches"] == "yes",
            "pdf_sha256": PDF_SHA.get(r["source_id"]),
            "stored_text_identical_to_fresh_pdftotext": True,
            "note": v["note"],
        }
        if v["verdict"] == "grade_wrong":
            r["row_kind"] = "derived"
            r["evidence_grade"] = "c"
            ver["grade_note"] = "A verifier judged the grade wrong; the row is derived / c."
        if r["source_id"] == "amana_ball_nose_v7":
            ver["caveats"] = COMMON_CAVEATS["amana_ball_nose_v7"]
            ver["correction"] = {"field": "ap_rule", "was": r["ap_rule"], "now": V7_AP_RULE,
                                 "why": "verifier: the old text paraphrased the printed rule"}
            r["ap_rule"] = V7_AP_RULE
        else:
            ver["caveats"] = COMMON_CAVEATS["onsrud"]
            ser = series_of(r)
            dkey = int(round(r["diameter_mm"] * 1000)) if r.get("diameter_mm") else None
            flag = NO_CATALOG_PART.get((ser, dkey))
            if flag:
                ver["catalog_part_note"] = flag
            if ser == "37-00" and TIP_OLD in r["notes"]:
                old = r["notes"]
                r["notes"] = old.replace(TIP_OLD, TIP_NEW)
                ver["correction"] = {"field": "notes", "was": TIP_OLD, "now": TIP_NEW,
                                     "why": "lamchip verifier: PCT-19 page 20 also lists 37-11 (0.060 in) and 37-15 (0.090 in); applied to all 7 sheets"}
        r["verification"] = ver
        out.append(r)

    doc = {
        "note": (
            "G2 verified rows, 2026-09-24. Candidate rows that a verifier confirmed. "
            "Not loaded into the LUT. Built by scripts/g2_verified_rows.py from "
            "candidate_rows.json and verifier_verdicts.json."
        ),
        "dropped": dropped,
        "unverified_left_out": unverified,
        "observations": out,
    }
    (G2 / "verified_rows.json").write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"verified {len(out)}  dropped {len(dropped)}  unverified {len(unverified)}")
    print("unverified:", ", ".join(unverified))


if __name__ == "__main__":
    main()
