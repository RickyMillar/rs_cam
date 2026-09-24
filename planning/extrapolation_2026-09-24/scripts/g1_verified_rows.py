#!/usr/bin/env python3
"""G1 reconciler: write fetch/G1/verified_rows.json from candidate_rows.json.

Read-only on the candidate rows. The verifier verdicts (2026-09-24) are
recorded below. Three sources have candidate rows, and a verifier checked
every row of each one cell by cell:

  spetool_carbide_spiral_router_bit_chart   42 rows, 42 confirmed
  amana_zrn_3d_profiling_v8                 27 rows, 27 confirmed
  spetool_2d3d_tapered_router_bit_chart     27 rows, 27 confirmed

The "grade_wrong", "wrong" and "not_found" verdicts of the verifiers are on
CLAIMS in sources.json (the 46280 geometry, the "36 of 41" count, the index
label), not on candidate rows. The script still applies the rule (drop
wrong / not_found; grade_wrong -> row_kind derived, grade c, plus a note) so
a re-run with new verdicts stays correct.

The script adds a note (it does not change a value) where a verifier finding
touches the evidence that a row cites:
  - the three Amana 0.794 mm 3-flute rows cite tool 46280;
  - the 27 SpeTool tapered rows cite the index page for the tool family.

The hardwood and softwood rows that copy the one "Wood, MDF, Sign-Foam"
printed cell stay (row_kind derived, grade b, the R5 convention). The older
manifest note refuses them. That choice moves numbers, so it waits for the
Phase 4 ruling G1-R1 (EXTRAPOLATION_G1.md section 3).

Run: python3 planning/extrapolation_2026-09-24/scripts/g1_verified_rows.py
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G1 = ROOT / "fetch" / "G1"

SOURCE_CHECK = {
    "spetool_carbide_spiral_router_bit_chart": {
        "url_reachable": True,
        "hash_matches": "yes (sha256 of the PDF, 375966a5...)",
        "method": "curl re-download to a unique directory; pdftotext -layout equal to section B apart "
        "from whitespace; page image at 110 dpi checked cell by cell against section A for all 42 rows",
    },
    "amana_zrn_3d_profiling_v8": {
        "url_reachable": True,
        "hash_matches": "yes (sha256 of the PDF, 5cdfb9c0...)",
        "method": "curl re-download (HTTP 200); pdftotext -layout line-for-line equal to the stored text "
        "(125 lines); every verbatim string found; x25.4, flutes, diameters and the IPM check recomputed",
    },
    "spetool_2d3d_tapered_router_bit_chart": {
        "url_reachable": True,
        "hash_matches": "yes (sha256 of the PDF, 0ae9e840...)",
        "method": "curl re-download (HTTP 200, application/pdf); pdftotext -layout equal to section B; "
        "page image at 150 dpi checked cell by cell against section A; feed identity 2.00 on all 9 rows",
    },
}

# Per-row verdicts. Every candidate row was confirmed; a row absent here is
# unverified and is left out.
CONFIRMED_ALL = set(SOURCE_CHECK)
DROP = {"wrong", "not_found"}

NOTE_46280 = (
    " Reconciler 2026-09-24 (verifier of amana_46xxx_tool_identity): the 46280 product page prints "
    "only 'Tapered ... 1/32\" D'; its 6.2 deg and 3 flutes are DERIVED from the chart section and the "
    "siblings 46291 and 46580. 46291, 46580 (6.2 deg, 1/32\" D, 3 flutes) and 46470 (6.2 deg, 0.8 D, "
    "3 flutes, live page) are identified as printed. The chart also lists 46471 as '0.8mm Dia.' in this "
    "section; the distributor gives 46471 as a 1 mm, 0.10 deg ball. The cell covers tapered and "
    "near-straight tools."
)
NOTE_SPETOOL_FAMILY = (
    " Reconciler 2026-09-24: the tool family comes from the SpeTool index page, which prints the tile "
    "'2D & 3D Tapered Router Bits' and links it to this chart (verified live; the label was added to the "
    "stored index text, new sha256 in sources.json). The chart page itself does not print 'tapered'."
)
NOTE_AMANA_FAMILY = (
    " Reconciler 2026-09-24: the chart heads this section 'Ball Nose' and never prints 'tapered'; the "
    "tapered identity comes from the tool numbers (sources/amana_46xxx_identity.txt). The chart names no "
    "operation: parallel / finish is an assignment."
)


def verdict_for(row):
    """The verifier verdict of one candidate row (all confirmed on 2026-09-24)."""
    if row["source_id"] in CONFIRMED_ALL:
        return "confirmed"
    return None


def main():
    cand = json.loads((G1 / "candidate_rows.json").read_text())["observations"]
    out, dropped, unverified = [], [], []
    for row in cand:
        v = verdict_for(row)
        if v is None:
            unverified.append(row["observation_id"])
            continue
        if v in DROP:
            dropped.append(row["observation_id"])
            continue
        r = dict(row)
        note = r.get("notes", "")
        if v == "grade_wrong":
            r["row_kind"] = "derived"
            r["evidence_grade"] = "c"
            note += " Reconciler: verifier verdict grade_wrong; row_kind derived, grade c."
        oid = r["observation_id"]
        if r["source_id"] == "amana_zrn_3d_profiling_v8":
            note += NOTE_AMANA_FAMILY
            if "-00794-3f" in oid:
                note += NOTE_46280
        if r["source_id"] == "spetool_2d3d_tapered_router_bit_chart":
            note += NOTE_SPETOOL_FAMILY
        r["notes"] = note
        chk = SOURCE_CHECK[r["source_id"]]
        r["verification"] = {
            "verdict": v,
            "verified_on": "2026-09-24",
            "url_reachable": chk["url_reachable"],
            "hash_matches": chk["hash_matches"],
            "method": chk["method"],
        }
        out.append(r)
    doc = {
        "generated_by": "scripts/g1_verified_rows.py",
        "generated_on": "2026-09-24",
        "candidate_rows": len(cand),
        "verified_rows": len(out),
        "dropped": dropped,
        "unverified": unverified,
        "observations": out,
    }
    (G1 / "verified_rows.json").write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n")
    print(f"candidate {len(cand)}  verified {len(out)}  dropped {len(dropped)}  unverified {len(unverified)}")


if __name__ == "__main__":
    main()
