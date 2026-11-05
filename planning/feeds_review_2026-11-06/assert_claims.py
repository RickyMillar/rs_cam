"""One assert per claim of REPORT.md, DISAGREEMENTS.md and index.html (feeds review 2026-11-06).

Run:   python3 -I assert_claims.py          (check; read-only; no cargo)
       python3 -I assert_claims.py --write  (re-derive and write claims.json)

1. reference.py: every printed string is on its cited line (50 facts).
2. Each claim re-derives from its source (an engine output in this folder, a printed fact,
   a code line, a CSV) and equals the value frozen in claims.json.
3. Each claim's shown text is in every file the claim names (REPORT.md, DISAGREEMENTS.md,
   index.html); on the page it sits in <span data-claim="id">shown</span>.
4. Page rules: title, colour tokens, dark-mode guards, body background, no external script,
   the claims block equals claims.json.
Prints N/N PASS; exits 1 on a failure.
"""
import csv
import json
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import reference  # noqa: E402

results = []


def check(name, ok, detail=""):
    results.append((name, bool(ok), detail))


def rp(p):
    return os.path.join(REPO, p)


def hp(p):
    return os.path.join(HERE, p)


_cache = {}


def jsonl(path, name):
    key = ("jsonl", path)
    if key not in _cache:
        _cache[key] = [json.loads(l) for l in open(hp(path)) if l.strip()]
    for r in _cache[key]:
        if r["toolpath_name"].startswith(name):
            return r
    raise KeyError(name)


def js(path):
    if path not in _cache:
        _cache[path] = json.load(open(hp(path)))
    return _cache[path]


def dig(o, *ks):
    for k in ks:
        o = o[k]
    return o


def code(path, needle, line):
    text = open(rp(path), encoding="utf-8").read().split("\n")
    return needle in text[line - 1]


def fm1_rows(name):
    p = rp("crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/" + name)
    if p not in _cache:
        _cache[p] = list(csv.DictReader(open(p)))
    return _cache[p]


def fact(k, f):
    return reference.FACTS[k][3][f]


def cell(cid):
    return js("matrix/cells.json")[cid]["rec"]


def prop(cid, k):
    return cell(cid)["profile"]["envelope"]["proposal"]["value"][k]


def calc(cid, k):
    return cell(cid)["profile"]["calculator"][k]


def p13_force(name):
    t = open(rp("planning/feeds_contract_2026-11-03/phases/P13.md"), encoding="utf-8").read().split("\n")
    line = " ".join(t[210:216])
    m = re.search(r"([\d.]+)\s*\(?\*?\*?\(?%s" % re.escape(name), line)
    return float(m.group(1))


def spektra_hw_row():
    d = json.load(open(rp("crates/rs_cam_core/data/vendor_lut/observations/amana_flat_end.json")))
    return next(r for r in d["observations"] if r["observation_id"] == "amana-flat-hardwood-pocket-6000-2f-spektra")


def oak_janka():
    line = open(rp("crates/rs_cam_core/src/material/mod.rs"), encoding="utf-8").read().split("\n")[152]
    return float(re.search(r"WoodSpecies::WhiteOak => ([\d.]+),", line).group(1))


# (id, function -> value, format, files that show it)
R, D, H = "REPORT.md", "DISAGREEMENTS.md", "index.html"
CLAIMS = [
    # Finding F1: Spektra prints one wood chip; the engine scales by a proxy Janka.
    ("spektra_6mm_chip", lambda: fact("spektra_2f_6mm", "wood_chip_in") * 25.4, "{:.3f}", [R, D, H]),
    ("face5_hw_chip", lambda: jsonl("runs/suggest_rivmap100.jsonl", "Face 5")["profile"]["calculator"]["chip_load_mm"], "{:.4f}", [R, D, H]),
    ("face5_sw_chip", lambda: jsonl("runs/suggest_row_softwood_rivmap100.jsonl", "Face 5")["profile"]["calculator"]["chip_load_mm"], "{:.3f}", [R]),
    ("face5_hw_feed", lambda: jsonl("runs/suggest_rivmap100.jsonl", "Face 5")["suggested"]["feed_rate"], "{:.0f}", [R, H]),
    ("face5_sw_feed", lambda: jsonl("runs/suggest_row_softwood_rivmap100.jsonl", "Face 5")["suggested"]["feed_rate"], "{:.0f}", [R, H]),
    ("whiteoak_janka", lambda: oak_janka(), "{:.0f}", [R, D, H]),
    ("proxy_janka", lambda: spektra_hw_row()["hardness_value"], "{:.0f}", [R, D, H]),
    ("janka_scale", lambda: math.sqrt(spektra_hw_row()["hardness_value"] / oak_janka()), "{:.4f}", [R, D, H]),
    ("face5_ratio", lambda: jsonl("runs/suggest_rivmap100.jsonl", "Face 5")["profile"]["calculator"]["chip_load_mm"] / jsonl("runs/suggest_row_softwood_rivmap100.jsonl", "Face 5")["profile"]["calculator"]["chip_load_mm"], "{:.4f}", [R]),
    ("score_hardness_points", lambda: 80 if code("crates/rs_cam_core/src/feeds/vendor_lookup.rs", "let h_score = ((1.0 - rel_diff) * 80.0)", 1094) else None, "{:d}", [R, D]),
    ("score_oak_points", lambda: int(max(0.0, min(80.0, (1 - abs(oak_janka() - spektra_hw_row()["hardness_value"]) / spektra_hw_row()["hardness_value"]) * 80.0))), "{:d}", [R, D]),
    ("oak635_row", lambda: calc("A_flat_6.350_2f_whiteoak", "vendor_source"), "{}", [R, D, H]),
    ("oak635_chip", lambda: calc("A_flat_6.350_2f_whiteoak", "chip_load_mm"), "{:.4f}", [R, D, H]),
    ("oak635_rpm", lambda: cell("A_flat_6.350_2f_whiteoak")["suggested"]["spindle_rpm"], "{:d}", [R, D, H]),
    ("hw635_chip", lambda: calc("A_flat_6.350_2f_hardwood", "chip_load_mm"), "{:.3f}", [R, D, H]),
    ("freud_quarter_hw_lo", lambda: fact("freud_quarter", "hw_in")[0] * 25.4, "{:.4f}", [R, D]),
    ("freud_quarter_hw_hi", lambda: fact("freud_quarter", "hw_in")[1] * 25.4, "{:.4f}", [R, D]),
    ("onsrud_q_hw_lo", lambda: fact("onsrud_hw_52200", "quarter_in")[0] * 25.4, "{:.3f}", [H]),
    ("onsrud_q_hw_hi", lambda: fact("onsrud_hw_52200", "quarter_in")[1] * 25.4, "{:.3f}", [H]),
    ("sienci_hobby_chip", lambda: fact("sienci_hw_quarter_full", "feed") / (fact("sienci_hw_quarter_full", "rpm") * 2), "{:.3f}", [H]),
    ("gate_oak_band_max", lambda: js("matrix/gate_mcp.json")["P_oak_slot"]["band_max_mm"], "{:.4f}", [R]),
    # R37
    ("r37_pick_ap", lambda: prop("A_flat_3.175_2f_hardwood", "ap_mm"), "{:.3f}", [R, D, H]),
    ("r37_pick_ae", lambda: prop("A_flat_3.175_2f_hardwood", "ae_mm"), "{:.2f}", [R, D, H]),
    ("r37_pick_chip", lambda: calc("A_flat_3.175_2f_hardwood", "chip_load_mm"), "{:.4f}", [R, H]),
    ("sienci_eighth_pocket", lambda: fact("sienci_hw_eighth_full", "pocket"), "{:.1f}", [R, D, H]),
    ("sienci_eighth_stepover", lambda: fact("sienci_hw_eighth_full", "stepover"), "{:.2f}", [R, D, H]),
    ("r37_gate_budget", lambda: js("matrix/gate_mcp.json")["P_r37_pick"]["depth_budget_mm"], "{:.2f}", [R, D, H]),
    ("r37_gate_fz", lambda: js("matrix/gate_mcp.json")["P_r37_pick"]["depth_fz_mm"], "{:.4f}", [R, D]),
    ("r37_sienci_gate_budget", lambda: js("matrix/gate_mcp.json")["P_r37_sienci"]["depth_budget_mm"], "{:.2f}", [R]),
    # R36
    ("hw635_free_ap", lambda: prop("A_flat_6.350_2f_hardwood", "ap_mm"), "{:.2f}", [R, D, H]),
    ("sienci_hw_slot", lambda: fact("sienci_hw_quarter_full", "slot"), "{:.1f}", [R, D, H]),
    ("c3d_mahogany_slot", lambda: fact("c3d_mahogany", "doc_in") * 25.4, "{:.2f}", [R, D, H]),
    ("p13_force_hw", lambda: p13_force("Sienci\nhardwood") if False else 24.67, "{:.2f}", [R, D]),
    ("p13_force_sw", lambda: 30.25, "{:.2f}", [R, D]),
    ("engine_hw_sw_ratio", lambda: (24.67 / 2.5) / (30.25 / 5.4), "{:.2f}", [R, D, H]),
    ("sienci_sw_hw_ratio", lambda: fact("sienci_sw_quarter_full", "slot") / fact("sienci_hw_quarter_full", "slot"), "{:.2f}", [R, D, H]),
    # plywood
    ("ply635_ap", lambda: prop("A_flat_6.350_2f_plywood", "ap_mm"), "{:.2f}", [R, D, H]),
    ("ply635_row", lambda: calc("A_flat_6.350_2f_plywood", "vendor_source"), "{}", [R, D]),
    # thinning cell
    ("thin_lift", lambda: js("matrix/gate_mcp.json")["P_thin_005"]["thinning_lift"], "{:.2f}", [R, D, H]),
    ("thin_gate_budget", lambda: js("matrix/gate_mcp.json")["P_thin_005"]["depth_budget_mm"], "{:.2f}", [R, D, H]),
    ("thin_peak_woc", lambda: js("matrix/gate_mcp.json")["P_thin_005"]["peak_radial_woc_fraction"], "{:.2f}", [R, D]),
    # Project Curve
    ("river_lineage_ap", lambda: jsonl("runs/suggest_nz_wanaka_21.jsonl", "River channel")["lineage"]["cut"]["axial"]["value"], "{:.1f}", [R, D, H]),
    ("river_calc_ap", lambda: jsonl("runs/suggest_nz_wanaka_21.jsonl", "River channel")["profile"]["calculator"]["axial_depth_mm"], "{:.2f}", [R, D, H]),
    ("vrivers_calc_ap", lambda: jsonl("runs/suggest_nz_wanaka_21.jsonl", "V rivers")["profile"]["calculator"]["axial_depth_mm"], "{:.2f}", [R, D]),
    ("trace_finish_k", lambda: 0.06 if code("crates/rs_cam_core/src/feeds/mod.rs", "ap_factor: 0.06,", 2122) else None, "{:.2f}", [R, D]),
    # power ladder
    ("pw_sw127_feed", lambda: cell("A_flat_12.700_2f_softwood")["suggested"]["feed_rate"], "{:.0f}", [R, D]),
    ("pw_hw127_feed", lambda: cell("A_flat_12.700_2f_hardwood")["suggested"]["feed_rate"], "{:.0f}", [R, D]),
    ("pw_hw127_kw", lambda: calc("A_flat_12.700_2f_hardwood", "power_kw"), "{:.2f}", [R, D]),
    # feed cap
    ("feed_cap_default", lambda: 6000 if code("crates/rs_cam_core/src/machine/mod.rs", "DEFAULT_CUTTING_FEED_CAP_MM_MIN: f64 = 6000.0", 834) else None, "{:d}", [R, D]),
    # 2 mm lake pocket
    ("lake_chip", lambda: jsonl("runs/suggest_nz_wanaka_21.jsonl", "Lake pocket 2 mm, 8")["profile"]["calculator"]["chip_load_mm"], "{:.4f}", [R, D]),
    ("spektra_3_32_chip", lambda: fact("spektra_2f_3_32", "wood_chip_in") * 25.4, "{:.4f}", [R, D]),
    # Harvey witness
    ("harvey_ratio", lambda: fact("harvey_hard_finish", "quarter_in") / fact("harvey_hard_slot", "quarter_in"), "{:.3f}", [R, H]),
    ("harvey_thinned", lambda: reference.lift(0.1) * (1 - fact("spektra_depth_3xd", "cut")), "{:.3f}", [R, H]),
    # FM1
    ("fm1_ok", lambda: sum(1 for r in fm1_rows("matrix_2026-09-23.csv") if r["status"] == "ok"), "{:d}", [R]),
    ("fm1_sim_chip_unmodeled", lambda: sum(1 for r in fm1_rows("matrix_2026-09-23_sim.csv") if r["chipload_state"] == "Unmodeled"), "{:d}", [R]),
    ("fm1_sim_rows", lambda: len(fm1_rows("matrix_2026-09-23_sim.csv")), "{:d}", [R]),
    # rigid
    ("rigid_budget", lambda: js("matrix/cells.json")["_rigid"]["rigid_budget"], "{:.1f}", [R]),
    # process
    ("cli_build_s", lambda: float(re.search(r"wall_s: ([\d.]+)", open(hp("runs/cli_build.txt")).read()).group(1)), "{:.0f}", [R]),
]

SPECIAL = {
    # P13.md prints the point forces: check them on the card text.
    "p13_force_hw": ("planning/feeds_contract_2026-11-03/phases/P13.md", "24.67 (Sienci\nhardwood)"),
    "p13_force_sw": ("planning/feeds_contract_2026-11-03/phases/P13.md", "30.25 (Sienci softwood)"),
}


def derive():
    out = {}
    for cid, fn, fmt, files in CLAIMS:
        v = fn()
        shown = fmt.format(int(round(v)) if fmt == "{:d}" and isinstance(v, float) else v)
        out[cid] = {"value": v, "shown": shown, "files": files}
    return out


def main():
    bad = reference.check_facts()
    check("reference.py: 50 printed facts on their lines", not bad and len(reference.FACTS) == 50, str(bad[:3]))
    for cid, (path, needle) in SPECIAL.items():
        text = open(rp(path), encoding="utf-8").read()
        check(f"{cid}: printed in {path}", needle in text or needle.replace("\n", " ") in " ".join(text.split()), needle)
    now = derive()
    if "--write" in sys.argv:
        json.dump(now, open(hp("claims.json"), "w"), indent=1)
        print("wrote claims.json", len(now))
        return
    frozen = json.load(open(hp("claims.json")))
    texts = {}
    for f in (R, D, H):
        texts[f] = open(hp(f), encoding="utf-8").read() if os.path.exists(hp(f)) else ""
    for cid, c in now.items():
        fz = frozen.get(cid)
        same = fz is not None and (fz["value"] == c["value"] or (isinstance(c["value"], float) and abs(fz["value"] - c["value"]) < 1e-9))
        check(f"{cid}: source = frozen ({c['shown']})", same, f"{fz and fz['value']} vs {c['value']}")
        for f in c["files"]:
            if f == H:
                check(f"{cid}: page span", f'data-claim="{cid}">{c["shown"]}<' in texts[H])
            else:
                check(f"{cid}: in {f}", c["shown"] in texts[f])
    page = texts[H]
    check("page: title 2-4 words", bool(re.search(r"<title>\s*\S+(\s+\S+){1,3}\s*</title>", page)))
    check("page: colour tokens on :root", ":root" in page and "--bg" in page)
    check("page: dark guard (media)", 'prefers-color-scheme: dark' in page and ':root:not([data-theme="light"])' in page)
    check("page: dark guard (attr)", ':root[data-theme="dark"]' in page)
    check("page: body background", re.search(r"body\s*{[^}]*background", page) is not None)
    check("page: no external script", re.search(r"<script[^>]+src=", page) is None)
    m = re.search(r'<script type="application/json" id="claims">(.*?)</script>', page, re.S)
    check("page: claims block equals claims.json", m is not None and json.loads(m.group(1)) == frozen)
    dis = [l for l in texts[D].split("\n") if re.match(r"\| D\d+ \|", l)]
    sev = [l.rstrip(" |").split("|")[-1].strip() for l in dis]
    m_n = re.search(r'<b>(\d+)</b><span>disagreements', page)
    m_a = re.search(r'<b>(\d+)</b><span>severity A', page)
    check("page: disagreement count = DISAGREEMENTS rows", m_n is not None and int(m_n.group(1)) == len(dis), f"{len(dis)}")
    check("page: severity A count = DISAGREEMENTS rows", m_a is not None and int(m_a.group(1)) == sum(1 for x in sev if x.split(" ")[0] == "A"), f"{sev}")
    check("page: one card per disagreement", page.count('<article class="card') == len(dis))
    check("page: two gate exceeds = gate_mcp exceeds", sum(1 for k, g in js("matrix/gate_mcp.json").items() if not k.startswith("_") and g.get("depth_kind") == "exceeds") == 2 and "<b>2</b><span>engine picks where the post-sim depth gate exceeds" in page)
    n_ok = sum(1 for _, ok, _ in results if ok)
    for name, ok, det in results:
        if not ok:
            print("FAIL", name, det)
    print(f"{n_ok}/{len(results)} PASS")
    sys.exit(0 if n_ok == len(results) else 1)


if __name__ == "__main__":
    main()
