#!/usr/bin/env python3
"""Build index.html of the P29-10 explainer from data/ (one self-contained
page: the data it draws is inlined, no network load).

Run it after the data files change, then run assert_claims.py.
"""

import glob
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")


def thin_rows(rows):
    out = []
    for r in rows:
        s = r["subject"]
        out.append({
            "subject": s["kind"],
            "cut": s.get("cut"),
            "at": s.get("at"),
            "limit": r["check"]["limit"],
            "face": r["face"],
            "tone": r["tone"],
            "bound": r["check"]["bound"],
            "unit": r["check"]["unit"],
            "caveats": [c["kind"] for c in r["caveats"]],
            "exemption": r["exemption"],
        })
    return out


def main():
    fields = json.load(open(os.path.join(DATA, "field_operator.json")))["ops"]
    ops = []
    for path in sorted(glob.glob(os.path.join(DATA, "mcp_safe_envelope_operator_*.json")),
                       key=lambda p: int(re.search(r"_(\d+)\.json$", p).group(1))):
        idx = int(re.search(r"_(\d+)\.json$", path).group(1))
        reply = json.load(open(path))
        env = reply["envelope"]
        op = fields[idx]
        plane = op.get("plane")
        edges = []
        for limit in env["boundary"]:
            edge = limit["edge"]
            if edge["kind"] == "ap_curve":
                edges.append({"kind": limit["kind"], "points": edge["points"]})
            elif edge["kind"] == "ap_ceiling":
                edges.append({"kind": limit["kind"], "points": [[0, edge["mm"]],
                                                                [op["diameter_mm"], edge["mm"]]]})
            elif edge["kind"] == "ae_ceiling":
                edges.append({"kind": limit["kind"], "points": [[edge["mm"], 0],
                                                                [edge["mm"], op["top_mm"]]]})
        ops.append({
            "index": idx,
            "name": reply["toolpath_name"],
            "diameter": op["diameter_mm"],
            "top": op["top_mm"],
            "cell": op["cell_mm"],
            "role": plane and plane["role"],
            "budget": plane and plane["budget_n"],
            "levels": [] if not plane else [
                {"share": lv["share"], "lines": lv["lines"]} for lv in plane["levels"]],
            "edges": edges,
            "columns": env["force_budget_columns"],
            "proposal": env["plane_marks"]["proposal"],
            "stated": env["plane_marks"]["stated"],
            "outside_cut": env["outside_cut"],
            "rows": thin_rows(env["limit_rows"]),
        })
    with open(os.path.join(HERE, "page_template.html")) as f:
        template = f.read()
    page = template.replace("/*DATA*/null", json.dumps(ops, separators=(",", ":")))
    with open(os.path.join(HERE, "index.html"), "w") as f:
        f.write(page)
    print(f"index.html: {len(ops)} ops, {len(page)} bytes")


if __name__ == "__main__":
    main()
