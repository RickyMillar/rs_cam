#!/usr/bin/env python3
"""G5 reconciler: write fetch/G5/verified_rows.json from candidate_rows.json.

Read-only on the candidate rows. The verifier verdicts (2026-09-24) are
recorded below per source. Every one of the 171 candidate rows was
"confirmed"; no row was "wrong", "not_found" or "grade_wrong". The script
still applies the rule (drop wrong / not_found; grade_wrong -> derived c)
so a re-run with new verdicts stays correct.

The verdict is assigned per SOURCE, not per observation_id. That is correct
today only because every row of every verified source was confirmed. A
partial verdict list needs a per-row table here.

Run: python3 planning/extrapolation_2026-09-24/scripts/g5_verified_rows.py
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G5 = ROOT / "fetch" / "G5"

# Per-source verifier summary (from the verifier reports, 2026-09-24).
SOURCE_CHECK = {
    "amana_insert_v_groove_v16": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl -L re-download; pdftotext -layout and -raw byte-identical to the stored texts; "
        "script check of tool set, angle, flutes, RPM, column, value, x25.4 and verbatim for all 105 rows",
    },
    "onsrud_hard_wood_cutting_data": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl re-download; pdftotext -layout byte-identical; pdftotext -bbox x-centre of each cell "
        "against its column header",
    },
    "onsrud_soft_wood_cutting_data": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl re-download; pdftotext -layout byte-identical; pdftotext -bbox column check",
    },
    "onsrud_mdf_cutting_data": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl re-download; pdftotext -layout byte-identical; pdftotext -bbox column check",
    },
    "onsrud_hard_plywood_cutting_data": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl re-download; pdftotext -layout byte-identical; pdftotext -bbox column check",
    },
    "onsrud_soft_plywood_cutting_data": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "curl re-download; pdftotext -layout byte-identical; pdftotext -bbox column check",
    },
    "amana_15_60_90_vgroove_engraving_2f": {
        "url_reachable": True,
        "hash_matches": "yes",
        "method": "re-download; pdftotext -layout byte-identical; every cell prints 0.003-0.007 in",
    },
}

# Every row the verifiers saw is confirmed. Rows not named here would be
# treated as unverified and left out.
VERDICTS = {"confirmed"}

# Limit notes that the verifiers raised from printed text on the documents.
RC1142 = (
    "Limit (printed footnote, verifier): 'Attention: Item #RC-1142, when using on small, hobby type "
    "CNC machines, set RPM to 12,000 RPM with 30% slower feed rate.' This row's tool group includes RC-1142."
)
PER_REV = (
    "Limit (verifier): the chart heading says 'Per Tooth IPR**' and the footnote says 'IPR** Inches per "
    "revolution'. The printed feed 50-125 IPM at 18,000 RPM matches per revolution. On 2 flutes the "
    "per-tooth reading is half this value (0.0381-0.0889 mm). Do not use this row for a slope or a ratio."
)
SECOND_CHART = (
    "Secondary check (verifier of amana_15_60_vgroove_engraving_2f, sha256 8e4281d6...): the 15/60 chart "
    "prints the same cell, 0.003-0.007 in at 18,000 RPM, depth 1 x D."
)
SHANK = (
    "Limit (verifier): the 1/4 column is the shank and body size of the 37-00/37-20 tools, not a V-top "
    "diameter. The 37-00 and 37-20 rows are two copies of one printed cell."
)
NO_CAT_3_8 = "Limit (verifier): the catalogue PCT-19 lists no 3/8 in 37-60 tool; the sheet prints the cell."
NO_CAT_1_1_4 = "Limit (verifier): the catalogue PCT-19 lists no 1 1/4 in 37-80 tool (37-87 is 1-1/2 in); the sheet prints the cell."
JOIN = "Flute count and angle come from the catalogue PCT-19 text (a join across two documents), not from the sheet."


def limit_notes(row):
    oid = row["observation_id"]
    notes = []
    if oid.startswith("x-g5-amana-insert-") and "-90deg-1f-18000rpm" in oid:
        notes.append(RC1142)
    if oid.startswith("x-g5-amana-vgroove-engrave-2f-"):
        notes.append(PER_REV)
        if "-90deg" not in oid:
            notes.append(SECOND_CHART)
    if oid.startswith("x-g5-onsrud-"):
        notes.append(JOIN)
        if "37-00" in oid or "37-20" in oid:
            notes.append(SHANK)
        if "37-60" in oid and oid.endswith("-3_8in"):
            notes.append(NO_CAT_3_8)
        if "37-80" in oid and oid.endswith("-1-1_4in"):
            notes.append(NO_CAT_1_1_4)
    return notes


def main():
    cand = json.loads((G5 / "candidate_rows.json").read_text())["observations"]
    out, dropped = [], []
    for row in cand:
        src = row["source_id"]
        check = SOURCE_CHECK.get(src)
        verdict = "confirmed" if check else "unverified"
        if verdict in ("wrong", "not_found", "unverified"):
            dropped.append((row["observation_id"], verdict))
            continue
        r = dict(row)
        if verdict == "grade_wrong":
            r["row_kind"] = "derived"
            r["evidence_grade"] = "c"
            r["notes"] = r.get("notes", "") + " Verifier: grade_wrong; set to derived c."
        r["verification"] = {
            "verdict": verdict,
            "verified_on": "2026-09-24",
            "url_reachable": check["url_reachable"],
            "hash_matches": check["hash_matches"],
            "method": check["method"],
            "limit_notes": limit_notes(row),
        }
        out.append(r)
    doc = {
        "generated_by": "planning/extrapolation_2026-09-24/scripts/g5_verified_rows.py",
        "generated_on": "2026-09-24",
        "rule": "candidate rows the verifiers confirmed; wrong and not_found dropped; grade_wrong set to derived c",
        "counts": {"candidates": len(cand), "verified": len(out), "dropped": len(dropped)},
        "observations": out,
    }
    (G5 / "verified_rows.json").write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"candidates {len(cand)}  verified {len(out)}  dropped {len(dropped)}")
    for oid, v in dropped:
        print("  dropped", oid, v)


if __name__ == "__main__":
    main()
