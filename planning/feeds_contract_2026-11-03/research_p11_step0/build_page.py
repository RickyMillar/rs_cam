"""Embed a compact copy of results.json into page_template.html and write index.html (no network).
Run: python3 -I build_page.py  (after python3 -I model.py)
"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
R = json.load(open(os.path.join(HERE, "results.json")))
CANDS = ["I1", "I2", "I3", "I4", "I3v", "I5", "C1", "C2"]


def rnd(x, n=5):
    return None if x is None else round(x, n)


data = dict(
    verdict=R["verdict"],
    c=R["c_values"],
    fractions={g: {k: R["fractions"][g][k] for k in CANDS} for g in ("W", "D")},
    fractions_ratio={g: {k: R["fractions_ratio"][g][k] for k in CANDS} for g in ("W", "D")},
    W=[dict(label=c["label"], fz1=c["fz1"], band=c["tgt_band"], printed_ratio=c["printed_ratio"],
            ref=c["ref_cond"], tgt=c["tgt_cond"], d=c["d"], z=c["z"],
            pred={k: rnd(v) for k, v in c["pred"].items()}, passes=c["passes"])
       for c in R["cases"] if c["group"] == "W"],
    D=[[c["id"], c["vendor"], c["material"], c["grade"], c["k"], rnd(c["fz1"]),
        rnd(c["tgt_band"][0]), rnd(c["tgt_band"][1]),
        [rnd(c["pred_ratio"][k], 4) for k in CANDS], [c["passes"][k] for k in CANDS]]
       for c in R["cases"] if c["group"] == "D"],
    hand=R["hand_check"],
    sens=R["sensitivity"],
    ph2=R["post_hoc"]["PH2_inverse_time_in_cut"],
)
tpl = open(os.path.join(HERE, "page_template.html")).read()
out = tpl.replace("/*DATA*/null", json.dumps(data, separators=(",", ":")))
open(os.path.join(HERE, "index.html"), "w").write(out)
print("index.html written,", len(out), "bytes")
