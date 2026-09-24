#!/usr/bin/env python3
"""G7 reconciler: write fetch/G7/verified_rows.json from candidate_rows.json.

All 24 candidate rows got the verdict "confirmed". This script keeps every
row, adds a "verification" field per row (verdict, verifier note, hash
status), and adds to "derived_fields" the items that the verifiers found
inferred but not listed. It does not change a printed value.
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
G7 = ROOT / "fetch" / "G7"
cand = json.loads((G7 / "candidate_rows.json").read_text())["observations"]

# Hash status per source, from the verifier reports (2026-09-24).
HASH = {
    "g7_goli2018_round_shape_ks": "matches (sha256 of pdf/PMC6315737.xml; the .txt is a derived conversion)",
    "g7_palubicki2021_particleboard": "matches (sha256 of pdf/PMC8123317.xml)",
    "g7_yang2022_hdpe": "matches (sha256 of pdf/PMC8747417.xml)",
    "g7_kopecky2019_mdf_quasi_orthogonal": "matches (PDF); pdftotext of a fresh download is byte-identical to the stored text",
    "g7_durkovic2017_oak_peripheral": "matches (PDF); pdftotext of a fresh download is byte-identical to the stored text",
    "g7_curti2021_generalized_wood_model": "no: the SAM server stamps each PDF download (71 bytes change), so no fresh download re-hashes; the claimed hash matches the stored PDF, and pdftotext of a fresh download is byte-identical to the stored text",
    "g7_goli2023_iwms25_ewp": "no: the SAM server stamps each PDF download (trailer ModDate and /ID), so no fresh download re-hashes; the claimed hash matches the stored PDF, and pdftotext of a fresh download is byte-identical to the stored text",
}

# Verifier note per row (short form of the verdict note) and caveats.
NOTE = {
    "x-g7-goli2018-mdf-up-straight": "XML Table 3 prints Ks 31.44 (2.68), 25.81-35.58 and Int 3.36 (0.27), 2.96-3.83. Only cutting_speed is derived.",
    "x-g7-goli2018-ptfe-up-straight": "XML Table 2 prints Ks 20.33 (2.90), 14.46-24.81 and Int 2.71 (0.24), 2.40-3.15.",
    "x-g7-goli2018-beech-lvl-0deg-up-straight": "XML Table 4 prints Ks 32.29, Int 7.66 at 0 deg (across the grain).",
    "x-g7-goli2018-beech-lvl-90deg-up-straight": "XML Table 4 prints Ks 11.73, Int 3.70 at 90 deg (along the grain). The Discussion prints a +-5 N/mm2 measurement error.",
    "x-g7-palubicki2021-pb-up-vc40": "The Conclusions print the sentence exactly. h max is printed as 'approximately equal 0.31 mm': an approximate limit. helix 0 is implied by the straight knife.",
    "x-g7-palubicki2021-pb-up-vc60": "Same sentence, 37.6 N/mm2 at 60 m/s. h max 0.31 mm is approximate.",
    "x-g7-yang2022-hdpe-yield-rake15": "46.89 MPa and 1.173 kJ/m2 are printed. The rake pairing of the yield stress is inferred from sentence order. 'shear' in the quantity name is not printed. Not a milling Kc.",
    "x-g7-yang2022-hdpe-yield-rake30": "33.85 MPa and 1.368 kJ/m2 are printed. The rake pairing is inferred from order. Not a milling Kc.",
    "x-g7-kopecky2019-mdf-conventional": "Line 354 prints the fit verbatim. No chip width b is printed. The paper gives k in 'N m-1', which does not agree with hm in mm. flute_count 1 is inferred ('single-handed' cutter, fz = vf/n).",
    "x-g7-kopecky2019-mdf-climb": "Line 355 prints the fit verbatim. Same caveats as the conventional row.",
    "x-g7-durkovic2017-oak-force-per-blade": "Eq. 7 and Eq. 3 print the fits. kc 23.7-60.6 recomputes with b = 30 mm (derived). helix 0 is inferred. At em 0.0216 the fit gives Fb about 39 N, below the paper's printed 44-102 N, so kc_max comes from the fitted curve.",
    "x-g7-durkovic2017-krsljak-ke1-oak": "Ke1 = 14 N/mm2 and the Tab. 3 K column 57.99-140.63 are printed. A textbook coefficient and model output, grade c.",
    "x-g7-curti2021-ksnorm-up-helix0": "Table 5 polynomials match character for character. Table 5 prints no unit: 'N/mm2 per kg/m3' is inferred. The 'Ks 11.7-34.0' note uses GA 0 and 90 only, not the extremes of the quadratic.",
    "x-g7-curti2021-ksnorm-up-helix15": "Table 5 polynomials match. The unit is inferred.",
    "x-g7-curti2021-ksnorm-up-helix30": "Table 5 polynomials match. The unit is inferred.",
    "x-g7-curti2021-ksnorm-down-helix0": "Table 5 polynomials match, with the printed 'A' for 'GA' typo kept. The unit is inferred.",
    "x-g7-curti2021-ksnorm-down-helix15": "Table 5 polynomials match. The unit is inferred.",
    "x-g7-curti2021-ksnorm-down-helix30": "Table 5 polynomials match. The unit is inferred.",
    "x-g7-goli2023-plywood-poplar-up-helix0-figread": "Independent re-read of Fig 4(a): Ks about 24-43; Fig 3(a): Int about 3.15-4.65 (claim 3.0-4.9, inside the stated reading error).",
    "x-g7-goli2023-plywood-poplar-down-helix0-figread": "Independent re-read of Fig 4(b): Ks about 24-43; Fig 3(b): Int about 2.6-5.25.",
    "x-g7-goli2023-plywood-poplar-up-helix30-figread": "Re-read of Fig 6(a): Ks about 20-26.5; Fig 5(a): Int about -0.32 to -0.04. The captions say 'straight blade' for the 30 deg panels, and the Fig 5 axis reads N/mm^2 for an intercept.",
    "x-g7-goli2023-plywood-poplar-down-helix30-figread": "Re-read of Fig 6(b): Ks about 24.5-29.5; Fig 5(b): Int about -0.36 to -0.22. The reading error (+-0.2) is as large as the intercept.",
    "x-g7-goli2023-mdf-up-helix0-figread": "Re-read: Ks about 27.5-30, Int about 3.0-3.1. The notes text about plywood direction does not apply to MDF.",
    "x-g7-goli2023-particleboard-up-helix0-figread": "Re-read: Ks about 12-23, Int about 2.6-3.4. A 10 deg span over 0-180 deg gives 19 positions, not 18.",
}

# Inferred items the verifiers found missing from derived_fields.
ADD_DERIVED = {
    "x-g7-kopecky2019-mdf-conventional": ["flute_count"],
    "x-g7-kopecky2019-mdf-climb": ["flute_count"],
    "x-g7-durkovic2017-oak-force-per-blade": ["helix_deg"],
    "x-g7-yang2022-hdpe-yield-rake15": ["rake_deg", "quantity"],
    "x-g7-yang2022-hdpe-yield-rake30": ["rake_deg", "quantity"],
    "x-g7-palubicki2021-pb-up-vc40": ["helix_deg", "chip_thickness_mm_max"],
    "x-g7-palubicki2021-pb-up-vc60": ["helix_deg", "chip_thickness_mm_max"],
}
for rid in NOTE:
    if rid.startswith("x-g7-curti2021"):
        ADD_DERIVED[rid] = ["force_model"]

out = []
for r in cand:
    rid = r["observation_id"]
    assert rid in NOTE, rid
    row = dict(r)
    row["derived_fields"] = list(r.get("derived_fields") or []) + [
        d for d in ADD_DERIVED.get(rid, []) if d not in (r.get("derived_fields") or [])
    ]
    if rid == "x-g7-goli2023-particleboard-up-helix0-figread" or rid.startswith("x-g7-goli2023"):
        row["notes"] = row["notes"].replace("18 grain-position points", "19 grain-position points (10 deg steps over 0-180 deg)")
    row["verification"] = {
        "verdict": "confirmed",
        "verified_on": "2026-09-24",
        "url_reachable": True,
        "hash_status": HASH[r["source_id"]],
        "note": NOTE[rid],
        "reconciler_changes": "derived_fields extended with the inferred items the verifier named"
        if rid in ADD_DERIVED
        else "none",
    }
    out.append(row)

(G7 / "verified_rows.json").write_text(
    json.dumps({"observations": out}, indent=2, ensure_ascii=False) + "\n"
)
print(f"verified rows: {len(out)} of {len(cand)} candidates")
