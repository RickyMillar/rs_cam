#!/usr/bin/env python3
"""G3 reconciler: write fetch/G3/verified_rows.json from candidate_rows.json.

Verdicts (2026-09-24 verifier pass): all 54 candidate rows are "confirmed"
(36 amana_corner_radius_spiral_plunge_2f, 18 idcwoodcraft_feeds_speeds_pdf).
No row is "wrong", "not_found" or "grade_wrong", so no row is dropped and no
grade changes.

The script also applies the metadata corrections the verifiers asked for.
No chipload, diameter, flute count or RPM changes.

- Amana corner radius (46460 / 46462): the chart does not print the corner
  radius (1/16 in, 1/8 in) or "up-cut". Those came from the Toolstoday
  product pages, which the fetch did not store. The label drops them; the
  notes mark them DERIVED (unstored product page).
- Amana corner radius: ap_rule quotes the uncoated chart ("Reduce chip
  load"). The ZrN and Spektra copies print "Reduce feed rate". The notes
  say so.
- IDC V-bit: the wood category, the Janka value and the operation family
  are assignments. The notes say so.

Usage: python3 g3_verified_rows.py
"""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
G3 = HERE.parent / "fetch/G3"

VERDICT = {
    "amana_corner_radius_spiral_plunge_2f": {
        "verdict": "confirmed",
        "verified_on": "2026-09-24",
        "url_reachable": True,
        "hash_matches": "yes",
        "note_template": (
            "Printed 1/{frac}\" row, {mat} column; value x25.4 correct; "
            "D {d} mm, 2 flute, 18,000 RPM; verbatim line is in the text. "
            "Operation {op} is a disclosed mapping."),
    },
    "idcwoodcraft_feeds_speeds_pdf": {
        "verdict": "confirmed",
        "verified_on": "2026-09-24",
        "url_reachable": True,
        "hash_matches": "yes",
        "note_template": (
            "Verbatim line is in the V-BITS table; feed / (RPM x flutes) x25.4 "
            "correct to 4 places; derived grade c correctly marked. Wood category, "
            "Janka value and operation {op} are assignments, not printed."),
    },
}

CR_LABEL = {
    6.35: "Amana 46460, 1/4 in diameter, 2 flute spiral plunge with corner radius",
    12.7: "Amana 46462, 1/2 in diameter, 2 flute spiral plunge with corner radius",
}
CHART_COLUMN = {"softwood": "Soft Wood", "hardwood": "Hard Wood", "mdf": "MDF"}
CR_NOTE_ADD = (
    " RECONCILER 2026-09-24: the chart does not print the corner radius "
    "(1/16 in for 46460, 1/8 in for 46462) or the word 'up-cut'. Those values "
    "are DERIVED from the Toolstoday product pages (not stored, no hash); "
    "'Corner radius = D/4' is computed from them. The ap_rule wording "
    "('Reduce chip load') is the uncoated chart's; the ZrN and Spektra "
    "copies print 'Reduce feed rate'.")
IDC_NOTE_ADD = (
    " RECONCILER 2026-09-24: the softwood/hardwood split, the Janka value and "
    "the operation family (trace, pocket, adaptive) are ASSIGNED, not printed. "
    "The PDF prints no material column and one feed for the clear pass and "
    "the final pass. Machine class: benchtop (G9); do not mix with industrial "
    "V-groove charts without a machine-class ruling.")


def main():
    cand = json.loads((G3 / "candidate_rows.json").read_text())["observations"]
    out = []
    for row in cand:
        v = VERDICT.get(row["source_id"])
        if v is None:
            continue  # unverified source: not carried
        row = dict(row)
        mat = row["material_family"]
        op = row["operation_family"]
        if row["source_id"] == "amana_corner_radius_spiral_plunge_2f":
            frac = "4" if row["diameter_mm"] == 6.35 else "2"
            note = v["note_template"].format(frac=frac, mat=mat, d=row["diameter_mm"], op=op)
            row["material_label"] = (
                f"{CHART_COLUMN[mat]} (Amana chart column); "
                + CR_LABEL[row["diameter_mm"]])
            row["notes"] = row["notes"] + CR_NOTE_ADD
        else:
            note = v["note_template"].format(op=op)
            row["notes"] = row["notes"] + IDC_NOTE_ADD
        row["verification"] = {
            "verdict": v["verdict"],
            "verified_on": v["verified_on"],
            "url_reachable": v["url_reachable"],
            "hash_matches": v["hash_matches"],
            "note": note,
        }
        out.append(row)
    (G3 / "verified_rows.json").write_text(
        json.dumps({"observations": out}, indent=1, ensure_ascii=False) + "\n")
    print(f"verified_rows.json: {len(out)} rows of {len(cand)} candidates")


if __name__ == "__main__":
    main()
