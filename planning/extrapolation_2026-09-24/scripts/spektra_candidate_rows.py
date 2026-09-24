#!/usr/bin/env python3
"""Transcribe the Amana Spektra Spiral Plunge chart (v24) and emit the
missing observation rows.

This script does NOT read the PDF. The TABLE below is typed by hand from
`pdftotext -layout fetch/G6/pdf/amana_spektra_v24.pdf -`. A separate
verifier re-reads the PDF text and checks these numbers independently.

Scope note (2026-09-25 update): `crates/rs_cam_core/data/vendor_lut/
observations/amana_long_tail.json` already carries softwood and mdf
Spektra rows for every candidate size below (ids ending "-spektra"). This
script loads every observation id in `observations/*.json`, skips any
candidate observation whose id already exists, and only emits the
families that are still missing: hardwood, plywood_softwood and
plywood_hardwood.

Run from the repository root:
    python3 planning/extrapolation_2026-09-24/scripts/spektra_candidate_rows.py
"""

import glob
import json
import os

REPO_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..")
)
OUT_PATH = os.path.join(
    REPO_ROOT, "planning/extrapolation_2026-09-24/fetch/G6/candidate_rows_spektra_all.json"
)
OBS_GLOB = os.path.join(
    REPO_ROOT, "crates/rs_cam_core/data/vendor_lut/observations/*.json"
)
PDF_SHA256 = "5b6fef854b2cf6b422e2eb5bbc86e29b4dcab5b19195b7ff228b70f99cf86d3a"
GENERATOR = "planning/extrapolation_2026-09-24/scripts/spektra_candidate_rows.py"

# ---------------------------------------------------------------------------
# TABLE: every row the chart prints, typed by hand from the -layout text.
# Each entry is one printed diameter within one flute block. Rows that
# print identical diameters and identical values are grouped into one
# entry with a list of (Up-Cut, Down-Cut) tool number pairs, "—" for none.
#
# Fields: z (flute count), diam (as printed), tools (list of pairs),
# feed_wp/chip_wp/ramp_wp (Wood/Plywood columns, as printed strings),
# feed_mdf/chip_mdf/ramp_mdf (MDF/Laminate columns, as printed strings).
# ---------------------------------------------------------------------------

TABLE = [
    # ---- 2 Flute ----
    {
        "z": 2, "diam": '1/32"',
        "tools": [("—", "46229-K"), ("—", "46242-K")],
        "feed_wp": "35", "chip_wp": ".0010", "ramp_wp": "17.5",
        "feed_mdf": "70", "chip_mdf": ".0020", "ramp_mdf": "35",
    },
    {
        "z": 2, "diam": "1.5mm",
        "tools": [("—", "48210-K"), ("—", "48212-K")],
        "feed_wp": "70", "chip_wp": ".0020", "ramp_wp": "35",
        "feed_mdf": "105", "chip_mdf": ".0030", "ramp_mdf": "52.5",
    },
    {
        "z": 2, "diam": '1/16"',
        "tools": [
            ("—", "46237-K"), ("—", "46213-K"), ("—", "46233-K"),
            ("—", "46448-K"), ("46009-K", "46403-K"),
        ],
        "feed_wp": "70", "chip_wp": ".0020", "ramp_wp": "35",
        "feed_mdf": "105", "chip_mdf": ".0030", "ramp_mdf": "52.5",
    },
    {
        "z": 2, "diam": '3/32"',
        "tools": [("—", "46239-K"), ("—", "46244-K")],
        "feed_wp": "80", "chip_wp": ".0023", "ramp_wp": "40",
        "feed_mdf": "160", "chip_mdf": ".0046", "ramp_mdf": "80",
    },
    {
        "z": 2, "diam": "3mm",
        "tools": [("—", "48214-K"), ("48116-K", "48216-K")],
        "feed_wp": "145", "chip_wp": ".0040", "ramp_wp": "72.5",
        "feed_mdf": "180", "chip_mdf": ".0050", "ramp_mdf": "90",
    },
    {
        "z": 2, "diam": '1/8"',
        "tools": [
            ("46127-K", "46227-K"), ("46100-K", "46200-K"),
            ("46125-K", "46225-K"),
        ],
        "feed_wp": "145", "chip_wp": ".0040", "ramp_wp": "72.5",
        "feed_mdf": "180", "chip_mdf": ".0050", "ramp_mdf": "90",
    },
    {
        "z": 2, "diam": '3/16"',
        "tools": [("46101-K", "46201-K")],
        "feed_wp": "180", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "215", "chip_mdf": ".0060", "ramp_mdf": "107.5",
    },
    {
        "z": 2, "diam": "5mm",
        "tools": [("—", "46211-K")],
        "feed_wp": "180", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "215", "chip_mdf": ".0060", "ramp_mdf": "107.5",
    },
    {
        "z": 2, "diam": "6mm",
        "tools": [("48118-K", "48218-K"), ("48120-K", "48220-K")],
        "feed_wp": "180", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "215", "chip_mdf": ".0060", "ramp_mdf": "107.5",
    },
    {
        "z": 2, "diam": '1/4"',
        "tools": [
            ("46102-K", "46202-K"), ("46315-K", "46415-K"),
            ("46316-K", "46416-K"), ("46321-K", "46421-K"),
            ("46399-K", "—"),
        ],
        "feed_wp": "180", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "215", "chip_mdf": ".0060", "ramp_mdf": "107.5",
    },
    {
        "z": 2, "diam": '3/8"',
        "tools": [("—", "46203-K"), ("46320-K", "46420-K"), ("—", "46449-K")],
        "feed_wp": "230", "chip_wp": ".0064", "ramp_wp": "115",
        "feed_mdf": "390", "chip_mdf": ".0108", "ramp_mdf": "195",
    },
    {
        "z": 2, "diam": "12mm",
        "tools": [("—", "48228-K")],
        "feed_wp": "200", "chip_wp": ".0057", "ramp_wp": "100",
        "feed_mdf": "350", "chip_mdf": ".0096", "ramp_mdf": "175",
    },
    {
        "z": 2, "diam": '1/2"',
        "tools": [("46106-K", "46206-K")],
        "feed_wp": "200", "chip_wp": ".0057", "ramp_wp": "100",
        "feed_mdf": "350", "chip_mdf": ".0096", "ramp_mdf": "175",
    },
    # ---- 3 Flute ----
    {
        "z": 3, "diam": '0.023"',
        "tools": [("51629-K", "—")],
        "feed_wp": "55", "chip_wp": ".0010", "ramp_wp": "27.5",
        "feed_mdf": "110", "chip_mdf": ".0020", "ramp_mdf": "55",
    },
    {
        "z": 3, "diam": '1/8"',
        "tools": [("46001-K", "46051-K"), ("—", "46053-K")],
        "feed_wp": "215", "chip_wp": ".0040", "ramp_wp": "72",
        "feed_mdf": "270", "chip_mdf": ".0050", "ramp_mdf": "90",
    },
    {
        "z": 3, "diam": "6mm",
        "tools": [("—", "48502-K")],
        "feed_wp": "270", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "325", "chip_mdf": ".0060", "ramp_mdf": "109",
    },
    {
        "z": 3, "diam": '1/4"',
        "tools": [("46002-K", "46052-K"), ("—", "46054-K")],
        "feed_wp": "270", "chip_wp": ".0050", "ramp_wp": "90",
        "feed_mdf": "325", "chip_mdf": ".0060", "ramp_mdf": "109",
    },
    {
        "z": 3, "diam": '1/2"',
        "tools": [("46116-K", "46216-K")],
        "feed_wp": "300", "chip_wp": ".0057", "ramp_wp": "100",
        "feed_mdf": "500", "chip_mdf": ".0096", "ramp_mdf": "167",
    },
    {
        "z": 3, "diam": '3/8"',
        "tools": [("—", "46055-K")],
        "feed_wp": "345", "chip_wp": ".0064", "ramp_wp": "115",
        "feed_mdf": "580", "chip_mdf": ".0108", "ramp_mdf": "195",
    },
    {
        "z": 3, "diam": '3/4"',
        "tools": [("—", "46500-K")],
        "feed_wp": "330", "chip_wp": ".009", "ramp_wp": "110",
        "feed_mdf": "360", "chip_mdf": ".010", "ramp_mdf": "120",
    },
]

# ---------------------------------------------------------------------------
# Candidate sizes: 1/4" or larger, plus the metric sizes the chart prints
# below 1/4" (1.5mm, 3mm, 5mm, 12mm on 2F only, per the printed table
# above), excluding sizes already in the LUT under the drill range
# (3.175 mm / 1/8", 6.0 mm and 6.35 mm / 1/4", for both 2F and 3F).
#
# diam_key matches TABLE["diam"]; id_mm is round(d_mm * 1000) for the id;
# d_mm is the diameter_mm value the row publishes; row_label is the
# "X row" phrase used in source_page (no inch mark on fractions, to match
# the amana_flat_end.json -spektra rows already in the LUT).
# ---------------------------------------------------------------------------

CANDIDATES = [
    {"z": 2, "diam": "1.5mm", "d_mm": 1.5, "id_mm": 1500, "row_label": "1.5mm"},
    {"z": 2, "diam": "3mm", "d_mm": 3.0, "id_mm": 3000, "row_label": "3mm"},
    {"z": 2, "diam": "5mm", "d_mm": 5.0, "id_mm": 5000, "row_label": "5mm"},
    {"z": 2, "diam": '3/8"', "d_mm": 9.525, "id_mm": 9525, "row_label": "3/8"},
    {"z": 2, "diam": "12mm", "d_mm": 12.0, "id_mm": 12000, "row_label": "12mm"},
    {"z": 2, "diam": '1/2"', "d_mm": 12.7, "id_mm": 12700, "row_label": "1/2"},
    {"z": 3, "diam": '3/8"', "d_mm": 9.525, "id_mm": 9525, "row_label": "3/8"},
    {"z": 3, "diam": '1/2"', "d_mm": 12.7, "id_mm": 12700, "row_label": "1/2"},
    {"z": 3, "diam": '3/4"', "d_mm": 19.05, "id_mm": 19050, "row_label": "3/4"},
]

# The families each candidate size gets, in the order they are checked
# against the LUT: softwood and mdf are exact/from the two chart columns;
# hardwood, plywood_softwood and plywood_hardwood are derived reads of the
# same Wood/Plywood column, per the amana_flat_end.json -spektra precedent.
FAMILIES = [
    ("softwood", "wood", "derived", "b"),
    ("hardwood", "wood", "derived", "b"),
    ("plywood_softwood", "wood", "derived", "b"),
    ("plywood_hardwood", "wood", "derived", "b"),
    ("mdf", "mdf", "exact", "a"),
]

HARDNESS = {
    "softwood": 600.0,
    "hardwood": 1450.0,
    "plywood_softwood": 600.0,
    "plywood_hardwood": 1000.0,
    "mdf": 1100.0,
}

SOURCE_ID = "amana_spektra_spiral_plunge_v24"
SOURCE_VENDOR = "amana"
SOURCE_TITLE = "Amana Solid Carbide Spektra Spiral Plunge 2/3 Flute Chart v24"
SOURCE_URL = (
    "https://www.amanatool.com/pub/media/productattachments/"
    "Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf"
)
ACCESSED_ON = "2026-09-25"
TRANSCRIBED_LINE = (
    "Transcribed 2026-09-25 from the PDF text (G6 Spektra sizes, B5 clean-up)."
)


def load_lut_ids():
    """Return {observation_id: file_basename} for every observations/*.json file."""
    ids = {}
    for path in sorted(glob.glob(OBS_GLOB)):
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        for obs in data.get("observations", []):
            oid = obs.get("observation_id") or obs.get("id")
            if oid:
                ids[oid] = os.path.basename(path)
    return ids


def tool_group_str(pairs):
    """Format a list of (up, down) tool pairs the way amana_flat_end.json does:
    a pair with both tools joined by '/', a pair missing one tool shown bare,
    groups joined by ', ', in printed order."""
    parts = []
    for up, down in pairs:
        if up == "—" and down == "—":
            continue
        if up == "—":
            parts.append(down)
        elif down == "—":
            parts.append(up)
        else:
            parts.append(f"{up}/{down}")
    return ", ".join(parts)


def round4(x):
    return round(x, 4)


def build_id(family, id_mm, z):
    family_slug = family.replace("_", "-")
    return f"amana-flat-{family_slug}-pocket-{id_mm}-{z}f-spektra"


def make_observation(cand, family, column, row_kind, evidence_grade, row, already_ids):
    oid = build_id(family, cand["id_mm"], cand["z"])
    if oid in already_ids:
        return None, oid

    if column == "wood":
        feed = row["feed_wp"]
        chip = row["chip_wp"]
        material_label = "Wood/Plywood"
    else:
        feed = row["feed_mdf"]
        chip = row["chip_mdf"]
        material_label = "MDF/Laminate"

    chipload_max = round4(float(chip) * 25.4)
    chip_disp = chip if not chip.startswith(".") else "0" + chip
    tools_str = tool_group_str(row["tools"])

    source_page = (
        f'page 1, {cand["z"]} Flute, {cand["row_label"]} row ({tools_str}), '
        f'{material_label} column: {feed}" IPM, {chip}" chip load per tooth'
    )

    notes = (
        f"Chart prints one {material_label} column; this row applies it to "
        f"{family}. {TRANSCRIBED_LINE} The chart prints one value, not a "
        "range, so the row publishes only chipload_max_mm_tooth. The chart "
        "does not name an operation; the row sits in the pocket family with "
        "pass_role roughing at the chart's 1 x D condition. hardness_value "
        "is the engine's own Janka proxy for this material family "
        "(GenericSoftwood 600, GenericHardwood 1450, softwood plywood 600, "
        "hardwood-faced plywood 1000, MDF 1100), so the printed value "
        "reaches the generic material unscaled; the chart prints no "
        "hardness."
    )

    obs = {
        "observation_id": oid,
        "source_id": SOURCE_ID,
        "source_vendor": SOURCE_VENDOR,
        "source_title": SOURCE_TITLE,
        "source_url": SOURCE_URL,
        "accessed_on": ACCESSED_ON,
        "source_page": source_page,
        "evidence_grade": evidence_grade,
        "row_kind": row_kind,
        "tool_family": "flat_end",
        "tool_subfamily": "spektra_spiral_plunge",
        "operation_family": "pocket",
        "pass_role": "roughing",
        "material_family": family,
        "material_label": material_label,
        "hardness_kind": "janka",
        "hardness_value": HARDNESS[family],
        "diameter_mm": cand["d_mm"],
        "flute_count": cand["z"],
        "rpm_nominal": 18000.0,
        "chipload_max_mm_tooth": chipload_max,
        "ap_rule": "1xD use recommended feed rate; 2xD reduce 25%; 3xD reduce 50%",
        "ap_max_factor": 1.0,
        "machine_assumption": (
            f"chart at 18000 rpm; single printed CPT {chip_disp} in = "
            f"{chipload_max} mm, encoded as chipload_max only"
        ),
        "notes": notes,
    }
    return obs, oid


def find_row(diam, z):
    for row in TABLE:
        if row["diam"] == diam and row["z"] == z:
            return row
    raise KeyError((diam, z))


def build_ramp_down_check(cand, row):
    out = []
    for label, feed_key, ramp_key in (
        ("Wood/Plywood", "feed_wp", "ramp_wp"),
        ("MDF/Laminate", "feed_mdf", "ramp_mdf"),
    ):
        feed = float(row[feed_key])
        z = cand["z"]
        out.append(
            {
                "size": cand["row_label"] + ("" if "mm" in cand["diam"] else '"'),
                "diam_mm": cand["d_mm"],
                "flute_count": z,
                "column": label,
                "printed_feed_ipm": row[feed_key],
                "printed_ramp_down": row[ramp_key],
                "feed_over_z": round(feed / z, 2),
            }
        )
    return out


def diam_label_for_table(row):
    return row["diam"]


def in_lut_status(row):
    candidate_pairs = {(c["diam"], c["z"]) for c in CANDIDATES}
    already_pairs = {
        ('1/8"', 2), ("6mm", 2), ('1/4"', 2),
        ('1/8"', 3), ("6mm", 3), ('1/4"', 3),
    }
    key = (row["diam"], row["z"])
    if key in already_pairs:
        return "yes"
    if key in candidate_pairs:
        return "candidate"
    return "no (out of scope)"


def print_markdown_table():
    header = (
        "| Z | Diameter | d_mm | Tools | W/P IPM | W/P chip | W/P Ramp | "
        "MDF IPM | MDF chip | MDF Ramp | in LUT |"
    )
    sep = "|---|---|---|---|---|---|---|---|---|---|---|"
    print(header)
    print(sep)
    for row in TABLE:
        tools_str = tool_group_str(row["tools"])
        d_mm = diam_to_mm(row["diam"])
        print(
            f'| {row["z"]} | {row["diam"]} | {d_mm} | {tools_str} | '
            f'{row["feed_wp"]}" | {row["chip_wp"]}" | {row["ramp_wp"]}" | '
            f'{row["feed_mdf"]}" | {row["chip_mdf"]}" | {row["ramp_mdf"]}" | '
            f"{in_lut_status(row)} |"
        )


INCH_MM = {
    '1/32"': 0.79375,
    '1/16"': 1.5875,
    '3/32"': 2.38125,
    '1/8"': 3.175,
    '3/16"': 4.7625,
    '1/4"': 6.35,
    '3/8"': 9.525,
    '1/2"': 12.7,
    '3/4"': 19.05,
    '0.023"': 0.5842,
}


def diam_to_mm(diam):
    if diam in INCH_MM:
        return round(INCH_MM[diam], 4)
    if diam.endswith("mm"):
        return float(diam[:-2])
    raise ValueError(diam)


def main():
    lut_ids = load_lut_ids()

    already_in_lut = []
    observations = []
    ramp_down_check = []
    candidate_report = []

    for cand in CANDIDATES:
        row = find_row(cand["diam"], cand["z"])
        candidate_report.append(f'{cand["z"]}F {cand["diam"]}')
        ramp_down_check.extend(build_ramp_down_check(cand, row))

        for family, column, row_kind, evidence_grade in FAMILIES:
            oid = build_id(family, cand["id_mm"], cand["z"])
            if oid in lut_ids:
                already_in_lut.append({"observation_id": oid, "file": lut_ids[oid]})
                continue
            obs, oid = make_observation(
                cand, family, column, row_kind, evidence_grade, row, lut_ids
            )
            if obs is not None:
                observations.append(obs)

    printed_table = []
    for row in TABLE:
        printed_table.append(
            {
                "z": row["z"],
                "diam": row["diam"],
                "d_mm": diam_to_mm(row["diam"]),
                "tools": [list(p) for p in row["tools"]],
                "feed_wp_ipm": row["feed_wp"],
                "chip_wp_in": row["chip_wp"],
                "ramp_wp_in": row["ramp_wp"],
                "feed_mdf_ipm": row["feed_mdf"],
                "chip_mdf_in": row["chip_mdf"],
                "ramp_mdf_in": row["ramp_mdf"],
                "in_lut": in_lut_status(row),
            }
        )

    out = {
        "group": "G6",
        "written_on": "2026-09-25",
        "generator": GENERATOR,
        "source_id": SOURCE_ID,
        "pdf_sha256": PDF_SHA256,
        "rule": (
            "The candidates are the chart sizes from 1/4 in / 6.35 mm up and "
            "the metric sizes (1.5, 3, 5 and 12 mm, 2 Flute only), less the "
            "sizes where amana_flat_end.json already holds all five "
            "families (1/8 in, 6 mm and 1/4 in, 2F and 3F). "
            "amana_long_tail.json already holds softwood and mdf at every "
            "candidate size, so the file holds hardwood, plywood_softwood "
            "and plywood_hardwood only. The Ramp Down values are a check "
            "(ramp_down_check), not a row field."
        ),
        "counts": {
            "candidate_sizes": len(CANDIDATES),
            "families_per_size_emitted": 3,
            "observations_emitted": len(observations),
            "already_in_lut": len(already_in_lut),
            "printed_rows_total": len(TABLE),
        },
        "already_in_lut": already_in_lut,
        "observations": observations,
        "ramp_down_check": ramp_down_check,
        "printed_table": printed_table,
    }

    os.makedirs(os.path.dirname(OUT_PATH), exist_ok=True)
    with open(OUT_PATH, "w", encoding="utf-8") as fh:
        json.dump(out, fh, indent=2)
        fh.write("\n")

    print(f"Candidate sizes used ({len(CANDIDATES)}): " + ", ".join(candidate_report))
    print(f"Observations emitted: {len(observations)}")
    print(f"Already in LUT (skipped): {len(already_in_lut)}")
    print()
    print_markdown_table()
    print()
    print(f"Wrote {OUT_PATH}")


if __name__ == "__main__":
    main()
