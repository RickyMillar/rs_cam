#!/usr/bin/env python3
"""G6 reconciler: write fetch/G6/verified_rows.json from candidate_rows.json.

The script keeps the rows that the URL verifiers confirmed. It changes a
"grade_wrong" row to row_kind "derived" and evidence_grade "c". It drops a
"wrong" or "not_found" row (G6 has none). Each row gets a "verification"
field with the verifier verdicts and notes, copied from the reconciler task
of 2026-09-24. The script does not change a printed number.

Run: python3 planning/extrapolation_2026-09-24/scripts/g6_verified_rows.py
"""

import copy
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parents[1] / "fetch" / "G6"
SRC = HERE / "candidate_rows.json"
OUT = HERE / "verified_rows.json"

VERIFIED_ON = "2026-09-24"

# Verifier verdicts per observation_id: list of (verifier source, verdict, note).
LEITZ = "leitz_lexicon7_06_drilling"
AMANA = "amana_spektra_spiral_plunge_v24"
ONSRUD = "onsrud_drill_cutting_data"
PCT19 = "onsrud_pct19_catalog"
PB = "precisebits_fret_plane"
CMT = "cmt_311_71_72_hwm_dowel_drill"

SOFTWOOD_PRINTED = (
    "Verifier nit: this page prints 'Softwood' as the base material. So softwood "
    "is printed, not a placeholder, although material_label says otherwise."
)

V = {
    # Leitz Lexicon ch.6, 13 rows
    "x-g6-leitz-hs-twist-z2-softwood": [
        (LEITZ, "confirmed", "PDF p35: HS solid Z2/V2, red 1,2 m/min at 2500, Softwood, Hardwood = 0.7; table D 3-12. 0.24 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-twist-z2-heel-softwood": [
        (LEITZ, "confirmed", "PDF p37: HW solid Z2/V2 with heel, red 1,5 at 4500, Softwood, Hardwood = 0.8, LVL = 1.1; table D 6-16. 0.1667 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-marathon-z2-softwood-d-le6": [
        (LEITZ, "confirmed", "PDF p38: HW solid Z2/V2 Marathon, red 3 at 4500, D <= 6 mm, Softwood, Hardwood = 0.8, LVL = 1.2. 0.3333 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-marathon-z2-softwood-d6-12": [
        (LEITZ, "confirmed", "PDF p39: span coordinates pair red 4,5 and 4500 with 'D = 6 - 12 mm'. 0.5 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-marathon-z2-softwood-d-gt12": [
        (LEITZ, "confirmed", "PDF p39 lower diagram: red 3,5 and 4500 sit above 'D > 12 mm'. 0.3889 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-vpoint-z2-softwood": [
        (LEITZ, "confirmed", "PDF p41: HW solid Z2 V-point, red 1,2 at 4500, Softwood, Hardwood = 0.8, LVL = 1.1; table D 7-12. 0.1333 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hw-vpoint-marathon-z2-softwood-d6-12": [
        (LEITZ, "confirmed", "PDF p42: HW solid Z2 V-point Marathon, red 3,5 at 4500, D = 6 - 12 mm, Softwood, through hole. 0.3889 derived. " + SOFTWOOD_PRINTED)],
    "x-g6-leitz-hs-levin-z1-solidwood": [
        (LEITZ, "confirmed", "PDF p43: HS solid Z1, red 1,5 at 4500, 'Solid wood', 'Drilling depth > 4 x D = 0.8'; table D 5-12. 0.3333 derived. Softwood is a placeholder for the printed 'Solid wood'.")],
    "x-g6-leitz-hw-levin-z1-solidwood": [
        (LEITZ, "confirmed", "PDF p44: HW Z1/V1, red 1,5 at 4500, 'Solid wood'; table D 12-16. 0.3333 derived. Softwood is a placeholder for the printed 'Solid wood'.")],
    "x-g6-leitz-dowel-excellent-z2-chipboard-coated": [
        (LEITZ, "confirmed", "PDF p12: shank 10 mm HW solid, Z2/V2, red 2 at 4500, 'Chipboard plastic coated', 'MDF, solid wood = 0.7', 'Chipboard, uncoated = 1.3'; table D 3-10. 0.2222 derived. The red 2/4500 example is printed on pp 6, 7, 9, 10, 11, 12.")],
    "x-g6-leitz-dowel-excellent-z2-mdf-by-factor": [
        (LEITZ, "confirmed", "p12 prints 'MDF, solid wood = 0.7'. 2 x 0.7 = 1.4 m/min; 1.4*1000/9000 = 0.1556, derived twice.")],
    "x-g6-leitz-dowel-excellent-z2-softwood-by-factor": [
        (LEITZ, "confirmed", "p12 prints 'MDF, solid wood = 0.7'; 0.1556 correct. The page prints 'solid wood', not 'softwood'. The softwood mapping is a placeholder; the same factor also covers hardwood on this page.")],
    "x-g6-leitz-dowel-excellent-z2-particleboard-by-factor": [
        (LEITZ, "confirmed", "p12 prints 'Chipboard, uncoated = 1.3'; 2.6*1000/9000 = 0.2889 derived. Verifier nit: the last sentence of this row's notes (about the 'MDF, solid wood' factor) is copied from the MDF row and does not apply here.")],
    # Amana Spektra v24 Ramp Down, 8 rows
    "x-g6-amana-spektra-2f-1_8-plywood_hardwood-rampdown": [
        (AMANA, "confirmed", "2 Flute 1/8 in prints Wood/Plywood Ramp Down 72.5. 72.5/(18000x2)x25.4 = 0.0512 mm derived.")],
    "x-g6-amana-spektra-2f-1_8-mdf-rampdown": [
        (AMANA, "confirmed", "2 Flute 1/8 in prints MDF/Laminate Ramp Down 90. 0.0635 mm derived.")],
    "x-g6-amana-spektra-2f-6mm-plywood_hardwood-rampdown": [
        (AMANA, "confirmed", "2 Flute 6mm prints Wood/Plywood Ramp Down 90. 0.0635 mm derived.")],
    "x-g6-amana-spektra-2f-6mm-mdf-rampdown": [
        (AMANA, "confirmed", "2 Flute 6mm prints MDF/Laminate Ramp Down 107.5. 0.0758 mm derived.")],
    "x-g6-amana-spektra-3f-1_8-plywood_hardwood-rampdown": [
        (AMANA, "confirmed", "3 Flute 1/8 in prints Wood/Plywood Ramp Down 72. 0.0339 mm derived. The vendor rounds 215/3 = 71.67 to 72.")],
    "x-g6-amana-spektra-3f-1_8-mdf-rampdown": [
        (AMANA, "confirmed", "3 Flute 1/8 in prints MDF/Laminate Ramp Down 90. 0.0423 mm derived.")],
    "x-g6-amana-spektra-3f-6mm-plywood_hardwood-rampdown": [
        (AMANA, "confirmed", "3 Flute 6mm prints Wood/Plywood Ramp Down 90. 0.0423 mm derived.")],
    "x-g6-amana-spektra-3f-6mm-mdf-rampdown": [
        (AMANA, "confirmed", "3 Flute 6mm prints MDF/Laminate Ramp Down 109. 0.0513 mm derived. The vendor rounds 325/3 = 108.3 to 109.")],
    # Onsrud 72-000, 4 rows, two verifier entries each
    "x-g6-onsrud-72000-wood-3mm": [
        (ONSRUD, "confirmed", "'.009-.011' sits under the '3' column. x25.4 correct. Header says '(in)' but 3/5/6/8 are mm sizes."),
        (PCT19, "confirmed", "Supporting check: PCT-19 p90 prints DIA 3 at 2 flutes; p124 repeats the drill table and the gang-drill footnote.")],
    "x-g6-onsrud-72000-wood-5mm": [
        (ONSRUD, "confirmed", "'.011-.013' sits under the '5' column. 0.2794-0.3302 correct."),
        (PCT19, "confirmed", "Supporting check: p90 prints DIA 5 at 2 flutes; p124 prints .011-.013 in the 5 column.")],
    "x-g6-onsrud-72000-wood-6mm": [
        (ONSRUD, "confirmed", "'.013-.015' sits under the '6' column. 0.3302-0.381 correct."),
        (PCT19, "confirmed", "Supporting check: p90 prints DIA 6 at 2 flutes; p124 prints .013-.015 in the 6 column.")],
    "x-g6-onsrud-72000-wood-8mm": [
        (ONSRUD, "confirmed", "'.015-.017' sits under the '8' column. 0.381-0.4318 correct. Footnote '* Gang drills run at 4,500 RPM and 150 IPM' printed."),
        (PCT19, "confirmed", "Supporting check: p90 prints DIA 8 at 2 flutes; p124 prints .015-.017 in the 8 column.")],
    # PreciseBits fret plane, 4 rows
    "x-g6-precisebits-fretplane-softwood-janka-lt1500-plunge": [
        (PB, "confirmed", "Live HTML prints 'Softwood (Janka < 1,500) = 75 inches/minute'. 1905 mm/min correct. No RPM, so the chip load is null. hardness_value 600 is an engine proxy inside the printed band.")],
    "x-g6-precisebits-fretplane-hardwood-janka-lt1500-plunge": [
        (PB, "grade_wrong", "The number 75 in/min is printed, but only for the band labelled 'Softwood (Janka < 1,500)'. The page prints no hardwood row at 75 in/min. The hardwood assignment at Janka 1450 is an engine-proxy inference, so row_kind 'exact' overstates it.")],
    "x-g6-precisebits-fretplane-hardwood-janka-1500-2500-plunge": [
        (PB, "confirmed", "Live HTML prints 'Medium hardness hardwood(1,500 < Janka < 2,500) = 50 inches/minute'. 1270 mm/min correct. hardness_value 2000 is a point inside the printed band.")],
    "x-g6-precisebits-fretplane-hardwood-janka-gt2500-plunge": [
        (PB, "confirmed", "Live HTML prints 'High hardness hardwoodwood (Janka > 2,500) = 40 inches/minute' (the typo is in the source). 1016 mm/min correct. hardness_value 2600 is a point inside the open band.")],
    # CMT 311.71/72HWM, 2 rows
    "x-g6-cmt-311hwm-particleboard": [
        (CMT, "confirmed", "Page prints 'Recommended feed speed 1÷ 4m/minute – RPM 6000.', Z2, 'Ideal for chipboard, MDF, HDF and laminates.' 0.0833-0.3333 derived. Caution: tool_family brad_point_drill is an inference; the page says both 'No center-point or spurs' and '2 curved ground spurs [V2]'.")],
    "x-g6-cmt-311hwm-mdf": [
        (CMT, "confirmed", "Same printed text and arithmetic as the particleboard row. The same caution about brad_point_drill applies.")],
}

DEPTH_V = [
    (LEITZ, "confirmed", "PDF p36 prints the 4 x D clearance-stroke sentence. The diagram base material on p36 is 'Chipboard plastic coated'."),
    (LEITZ, "confirmed", "p43 prints 'Suitable for depths up to approx. 4 x D without interim clearance strokes.' and 'Softwood and hardwood.'"),
    (LEITZ, "confirmed", "'Drilling depth > 4 x D = 0.8' is printed on p43 and p44."),
    (LEITZ, "confirmed", "p44 prints 'Suitable for depths up to 75 mm without interim clearance'. Table D 12-16; 75/16 to 75/12 = 4.7 to 6.3 x D is derived."),
    (LEITZ, "confirmed", "p13 prints 'Infeed depth in hardwood and glulam maximum 2 x D.' Boring pins, D = 3 mm design, Z 1/1."),
    (LEITZ, "confirmed", "p13 prints the obligatory return stroke for boring pins in hardwood and glulam."),
    (LEITZ, "confirmed", "Text lines 2146-2147 print the HW twist Z2/V2 deep-hole sentence."),
    (LEITZ, "confirmed", "Text line 3977 prints 'Tool too long at the reversal point'. Verifier nit: the printed context is 'when drilling dowel holes', which the verbatim cuts."),
]


def main() -> None:
    cand = json.loads(SRC.read_text())
    obs_out = []
    dropped = []
    for row in cand["observations"]:
        oid = row["observation_id"]
        verdicts = V.get(oid)
        if not verdicts:
            dropped.append((oid, "no verifier verdict"))
            continue
        kinds = {v for _, v, _ in verdicts}
        if kinds & {"wrong", "not_found"}:
            dropped.append((oid, "/".join(sorted(kinds))))
            continue
        r = copy.deepcopy(row)
        if "grade_wrong" in kinds:
            r["row_kind_candidate"] = r.get("row_kind")
            r["evidence_grade_candidate"] = r.get("evidence_grade")
            r["row_kind"] = "derived"
            r["evidence_grade"] = "c"
            r["notes"] = (
                r.get("notes", "")
                + " RECONCILER 2026-09-24: verifier verdict grade_wrong. The page prints 75 in/min "
                "only for 'Softwood (Janka < 1,500)'. The hardwood assignment is an engine-proxy "
                "inference (GenericHardwood Janka 1450 < 1500), so the row is derived, grade c."
            )
        r["verification"] = {
            "verified_on": VERIFIED_ON,
            "verdicts": [
                {"verifier_source_id": s, "verdict": v, "note": n} for s, v, n in verdicts
            ],
            "reconciled_verdict": "grade_wrong" if "grade_wrong" in kinds else "confirmed",
        }
        obs_out.append(r)

    depth_out = []
    assert len(cand["depth_statements"]) == len(DEPTH_V), "depth statement count changed"
    for i, (st, (s, v, n)) in enumerate(zip(cand["depth_statements"], DEPTH_V)):
        d = copy.deepcopy(st)
        d["depth_statement_id"] = f"g6-depth-{i}"
        d["verification"] = {
            "verified_on": VERIFIED_ON,
            "verdicts": [{"verifier_source_id": s, "verdict": v, "note": n}],
            "reconciled_verdict": v,
        }
        depth_out.append(d)

    out = {
        "group": "G6",
        "written_on": VERIFIED_ON,
        "generator": "planning/extrapolation_2026-09-24/scripts/g6_verified_rows.py",
        "rule": (
            "Rows the URL verifiers confirmed. grade_wrong rows are row_kind derived, grade c. "
            "wrong / not_found rows are dropped (G6 had none). Five sources were not verified "
            "(whiteside_catalog_icstc, diablo_forstner_speed_chart, clebitco_speeds_and_feeds, "
            "toolstoday_understanding_cnc_feeds_and_speeds, adams_bits_feeds_and_speeds_guide); "
            "they contribute 0 rows, so no row here is unverified."
        ),
        "counts": {
            "observations_candidate": len(cand["observations"]),
            "observations_verified": len(obs_out),
            "observations_grade_wrong_downgraded": sum(
                1 for r in obs_out if r["verification"]["reconciled_verdict"] == "grade_wrong"
            ),
            "observations_dropped": len(dropped),
            "depth_statements_verified": len(depth_out),
        },
        "dropped": [{"observation_id": o, "reason": why} for o, why in dropped],
        "observations": obs_out,
        "depth_statements": depth_out,
    }
    OUT.write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
    print(json.dumps(out["counts"]))


if __name__ == "__main__":
    main()
