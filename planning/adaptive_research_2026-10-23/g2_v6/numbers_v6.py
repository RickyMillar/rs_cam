"""G2 v6: collect every number the page shows from the committed run files.

Writes numbers_v6.json next to this file. The page (build_v6.py) and the
checks (assert_v6.py) read only that JSON, so a page number cannot drift
from its source file."""
import json, os

H = os.path.dirname(os.path.abspath(__file__))
P3 = os.path.join(H, "..", "phase3")


def j(*p):
    return json.load(open(os.path.join(P3, *p)))


def clock(f):
    return round(j("fix26", f)["summary_total_s"], 1)


def runtime(*p):
    return round(j(*p)["total_runtime_s"], 1)


lt34 = j("fix34", "load_table.json")
lt33 = j("fix33", "load_table.json")
lo = j("fix35", "leftover.json")


def load(row):
    return {k: row[k] for k in ("stepover", "L_max", "L_p99", "L_p95", "own_bound", "mm_over_own_bound")}


def left(name):
    a = lo[name]["after job"]
    return {"max": a["max"], "p95": a["p95"], "area_over_0p6_mm2": a["area_e>0.6_mm2"]}


N = {
    "corne": {
        "source": "P15 D37, D45, D49; clocks in phase3/fix26/three_clocks_*.json",
        "cp": {"op1": clock("three_clocks_fix26_k_cp_fix6job_corne_tp1.json"),
               "rest3": clock("three_clocks_fix26_k_cp_fix6job_corne_tp3.json"),
               "job": 2841.0},
        "geo_s2p1": {"op1": clock("three_clocks_fix34_corne_B_s2p1_tp1.json"),
                     "edge": clock("three_clocks_fix34_corne_B_s2p1_tp5.json"),
                     "rest3": clock("three_clocks_fix34_corne_B_s2p1_tp3.json"),
                     "job": runtime("fix34", "corne_B_s2p1", "summary.json")},
        "geo_s1p2_before": {"op1": clock("three_clocks_fix26_k_body_cp_corne_tp1.json"),
                            "edge": clock("three_clocks_fix26_k_body_cp_corne_tp5.json"),
                            "rest3": clock("three_clocks_fix26_k_body_cp_corne_tp3.json"),
                            "job": 4344.0},
        "load_cp": load(lt34["Corne CP (D37)"]),
        "load_geo": load(lt34["Corne geo s2.1 B D49"]),
    },
    "wanaka": {
        "source": "P15 D47, D49, D51",
        "cp": {"job": clock("three_clocks_fix25_cp_alone_0.6_wanaka200_tp1.json")},
        "geo_s2p0": {"job": runtime("fix35", "wanaka_s2p0", "summary.json")},
        "geo_s3p5": {"op1": clock("three_clocks_fix34_wanaka_s3p5_tp1.json"),
                     "edge": clock("three_clocks_fix34_wanaka_s3p5_tp7.json"),
                     "job": runtime("fix34", "wanaka_s3p5", "summary.json")},
        "load_cp": load(lt33["CP alone 0.6"]),
        "load_s2p0": load(lt33["geo s2.0"]),
        "load_s3p5": load(lt33["geo s3.5"]),
        "left_cp": left("CP alone 0.6"),
        "left_s2p0": left("geo s2.0"),
        "left_s3p5": left("geo s3.5"),
    },
}
json.dump(N, open(os.path.join(H, "numbers_v6.json"), "w"), indent=1)
print(json.dumps(N, indent=1))
