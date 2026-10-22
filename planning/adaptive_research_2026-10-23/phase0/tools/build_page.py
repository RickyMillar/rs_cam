"""Build one Phase 0 fixture page (static HTML) from the column records.

Inputs (under phase0/data/):
  <fixture>_<strategy>.json   numbers recorded from the MCP replies
  <fixture>_readings.json     the visual readings (V1-V3, new defects, three questions)
  <fixture>_meta.json         the header (fixture, HEAD, binary, notes)
  <fixture>_<strategy>_n5.json  optional: the N5 standing-spurs block

Usage: python3 build_page.py <fixture> <out.html>

The page shows pass/fail bars and plain information. It has no ranking and
no score.
"""
import html
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "..", "data")
STRATS = ["contour_parallel", "agent_search", "contour_spiral"]

CSS = """
:root{--bg:#fafaf8;--fg:#1d1d1b;--muted:#5d5d58;--line:#d9d8d2;--card:#ffffff;
--pass:#1f7a3a;--fail:#b3261e;--nm:#8a6d00;--chip:#f0efe9}
@media (prefers-color-scheme: dark){:root{--bg:#161615;--fg:#ecebe6;--muted:#a3a29b;
--line:#34332f;--card:#1f1f1d;--pass:#5cc27e;--fail:#ff8a80;--nm:#e0c050;--chip:#2a2a27}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,-apple-system,Segoe UI,Roboto,sans-serif}
header{padding:20px 16px;border-bottom:1px solid var(--line)}
h1{margin:0 0 8px;font-size:22px}
h2{font-size:17px;margin:0 0 8px}
h3{font-size:14px;margin:14px 0 6px}
.meta{display:grid;grid-template-columns:max-content 1fr;gap:2px 12px;font-size:13px;color:var(--muted)}
.meta b{color:var(--fg);font-weight:600}
code{font:12px ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--chip);padding:1px 4px;border-radius:3px;word-break:break-all}
.cols{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px;padding:16px}
@media (max-width:1100px){.cols{grid-template-columns:1fr}}
.col{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px;min-width:0}
figure{margin:0 0 10px}
figure img{width:100%;height:auto;display:block;border:1px solid var(--line);border-radius:4px;background:#222}
figcaption{font-size:12px;color:var(--muted);margin-top:3px}
table{border-collapse:collapse;width:100%;font-size:12.5px;margin:4px 0 8px}
th,td{border-top:1px solid var(--line);padding:4px 6px;text-align:left;vertical-align:top}
th{font-weight:600;width:38%}
.PASS{color:var(--pass);font-weight:700}.FAIL{color:var(--fail);font-weight:700}
.NM{color:var(--nm);font-weight:700}
.v{margin:4px 0}.v b{display:inline-block;min-width:2.2em}
.note{font-size:12.5px;color:var(--muted)}
.crops{display:grid;grid-template-columns:1fr 1fr;gap:6px}
.intro{padding:0 16px;max-width:1100px}
"""


def esc(x):
    return html.escape(str(x))


def load(name):
    with open(os.path.join(DATA, name)) as f:
        return json.load(f)


def bar_rows(col, meta):
    rows = []
    ops = col["ops"]
    if col.get("gen_error"):
        b1 = ("NM", "NOT MEASURED", "generation did not finish; nothing to check")
    else:
        rc = sum(o.get("rapid_collisions", 0) or 0 for o in ops)
        hc = sum(o.get("holder_collisions", 0) or 0 for o in ops)
        state = "PASS" if rc == 0 and hc == 0 else "FAIL"
        b1 = (state, state, f"rapid collisions {rc} (run_simulation rapid_collision_count, per op and project); "
              f"holder collisions {hc} (collision_check collision_count per rough op)")
    rows.append(("Bar 1: 0 rapid + 0 holder collisions", b1))
    rows.append(("Bar 2: coverage audit", ("NM", "NOT MEASURED", meta["bar2_note"])))
    b3 = meta["bar3_note"]
    extra = "; ".join(f"{o['name']}: crosses_standing_material {o.get('crosses_standing_material')}, "
                      f"link_cuts_stock {o.get('link_cuts_stock')}" for o in ops)
    fired = any(o.get("crosses_standing_material") != "absent" or o.get("link_cuts_stock") != "absent" for o in ops)
    rows.append(("Bar 3a: mesh gouge on emitted moves", ("NM", "NOT MEASURED", b3)))
    if fired:
        rows.append(("Bar 3b: stock-side over-cut / link plough", ("FAIL", "FAIL", extra)))
    else:
        rows.append(("Bar 3b: stock-side over-cut / link plough",
                     ("PASS", "0 reported", "neither id appears in the run_simulation triage or get_toolpath_diagnostics. " + extra)))
    if col.get("gen_error"):
        rows.append(("Bar 4: generation, no refusal", ("FAIL", "FAIL", col["gen_error"])))
    else:
        rows.append(("Bar 4: generation, no refusal", ("PASS", "PASS", "every enabled op generated; wall clock "
                     + ", ".join(f"{o['name']} {o['gen_wall_s']} s" for o in ops))))
    out = ["<table>"]
    for name, (cls, word, txt) in rows:
        out.append(f"<tr><th>{esc(name)}</th><td><span class='{cls}'>{word}</span> {esc(txt)}</td></tr>")
    out.append("</table>")
    return "\n".join(out)


def op_block(fx, op, rd_op):
    p = op["img"]
    out = [f"<h3>{esc(op['name'])} (index {op['index']})</h3>"]
    views = [("cut", "1. Cutting moves only (screenshot_toolpath include_rapids false)"),
             ("rapids", "1b. Cutting + rapids (screenshot_toolpath include_rapids true)"),
             ("links", "2. Links: G0 moves (orange) and fed keep-down links (blue), top-down XY from the exported .nc"),
             ("entries", "3. Entries: start point of each inspect_spans Entry span, by label"),
             ("stock", "4. Final stock (screenshot_simulation; colour by height, not deviation)")]
    for key, cap in views:
        f = f"img/{p}_{key}.png"
        if os.path.exists(os.path.join(HERE, "..", f)):
            out.append(f"<figure><a href='{f}'><img loading='lazy' src='{f}' alt='{esc(cap)}'></a>"
                       f"<figcaption>{esc(cap)}</figcaption></figure>")
    crops = op.get("crops", [])
    if crops:
        out.append("<div class='crops'>")
        for n in crops:
            f = f"img/{p}_crop_{n}.png"
            cap = rd_op.get("crops", {}).get(str(n), f"crop {n}")
            out.append(f"<figure><a href='{f}'><img loading='lazy' src='{f}' alt='crop {n}'></a>"
                       f"<figcaption>crop {n}: {esc(cap)}</figcaption></figure>")
        out.append("</div>")
    return "\n".join(out)


def phase3(op):
    e = op.get("entries", {})
    rows = [
        ("Retracts (G0 runs, narrate_toolpath)", op.get("retracts")),
        ("Entries by type (top-level inspect_spans Entry labels)", ", ".join(f"{k} {v}" for k, v in sorted(e.items())) or "none"),
        ("Nested Entry spans inside those (the descent parts)", ", ".join(f"{k} {v}" for k, v in sorted(op.get("entries_nested", {}).items())) or "none"),
        ("Link bridges (inspect_spans LinkBridge)", op.get("link_bridge")),
        ("Regions (inspect_spans Region)", op.get("regions")),
        ("Levels (DepthPass)", op.get("levels")),
        ("Entry time (cut trace runtime_by_intent.entry_s)", f"{op.get('entry_s')} s of {op.get('op_runtime_s')} s"),
    ]
    out = ["<table>"]
    for k, v in rows:
        out.append(f"<tr><th>{esc(k)}</th><td>{esc(v)}</td></tr>")
    out.append("</table>")
    return "\n".join(out)


def info(col, op):
    rows = [
        ("Cycle time (op, simulation)", f"{op.get('op_runtime_s')} s"),
        ("Cycle time (project incl. prerequisites)", f"{col['sim'].get('project_runtime_s')} s"),
        ("Move count", op.get("moves")),
        ("Cutting / rapid distance", f"{op.get('cut_mm')} mm / {op.get('rapid_mm')} mm"),
        ("Sim resolution", f"{col['sim']['resolution_mm']} mm ({col['sim']['mode']}, stored)"),
        ("Measurability", col["sim"].get("degraded", "")),
        ("trochoid_cap_mult (stored)", op.get("trochoid_cap_mult")),
        ("Other", op.get("extra", "")),
    ]
    out = ["<table class='note'>"]
    for k, v in rows:
        if v in ("", None):
            continue
        out.append(f"<tr><th>{esc(k)}</th><td>{esc(v)}</td></tr>")
    out.append("</table>")
    return "\n".join(out)


def readings_block(rd):
    out = []
    for key, name in (("V1", "V1 corner mop loops"), ("V2", "V2 full circles, half air"),
                      ("V3", "V3 does not grow into spaces")):
        out.append(f"<p class='v'><b>{key}</b> {esc(rd[key])}</p>")
    if rd.get("new"):
        out.append("<h3>New defects</h3><ul>")
        for n in rd["new"]:
            out.append(f"<li>{esc(n)}</li>")
        out.append("</ul>")
    out.append("<h3>The three questions (my reading)</h3>")
    out.append(f"<p class='v'><b>Links</b> {esc(rd['q_links'])}</p>")
    out.append(f"<p class='v'><b>Path</b> {esc(rd['q_path'])}</p>")
    out.append(f"<p class='v'><b>Part</b> {esc(rd['q_part'])}</p>")
    return "\n".join(out)


def main():
    fx, outp = sys.argv[1], sys.argv[2]
    meta = load(f"{fx}_meta.json")
    rds = load(f"{fx}_readings.json")
    h = ["<!doctype html><html lang='en'><head><meta charset='utf-8'>",
         "<meta name='viewport' content='width=device-width,initial-scale=1'>",
         f"<title>{esc(meta['title'])}</title><style>{CSS}</style></head><body>",
         f"<header><h1>{esc(meta['title'])}</h1><div class='meta'>"]
    for k, v in meta["header"]:
        h.append(f"<b>{esc(k)}</b><span>{v}</span>")
    h.append("</div></header>")
    if meta.get("intro"):
        h.append("<div class='intro'>" + "".join(f"<p class='note'>{esc(p)}</p>" for p in meta["intro"]) + "</div>")
    h.append("<div class='cols'>")
    for s in STRATS:
        fn = os.path.join(DATA, f"{fx}_{s}.json")
        if not os.path.exists(fn):
            continue
        col = load(f"{fx}_{s}.json")
        rd = rds[s]
        h.append(f"<section class='col'><h2>{esc(col['label'])}</h2>")
        h.append(f"<p class='note'>Project: <code>{esc(col['project'])}</code><br>G-code: <code>{esc(col['gcode'])}</code><br>"
                 f"GUI before this column: mtime {col['gui']['mtime_unix']}, pid {col['gui']['gui_pid']}</p>")
        if col.get("face"):
            h.append(f"<p class='note'>Prerequisite: {esc(col['face'])}</p>")
        for op in col["ops"]:
            h.append(op_block(fx, op, rd.get("ops", {}).get(op["name"], {})))
        h.append("<h3>Hard bars</h3>" + bar_rows(col, meta))
        h.append("<h3>V1-V3</h3>" + readings_block(rd))
        n5f = os.path.join(DATA, f"{fx}_{s}_n5.json")
        if os.path.exists(n5f):
            n5 = load(f"{fx}_{s}_n5.json")
            h.append(f"<h3>N5 standing spurs: {esc(n5['verdict'])}</h3>")
            for k in ("at_7min", "mechanism", "at_end", "gui_note"):
                if n5.get(k):
                    h.append(f"<p class='note'><b>{esc(k.replace('_', ' '))}:</b> {esc(n5[k])}</p>")
            h.append("<div class='crops'>")
            for f, cap in n5.get("imgs", []):
                h.append(f"<figure><a href='img/{f}'><img loading='lazy' src='img/{f}' alt='{esc(cap)}'></a>"
                         f"<figcaption>{esc(cap)}</figcaption></figure>")
            h.append("</div>")
        for op in col["ops"]:
            h.append(f"<h3>Phase 3: {esc(op['name'])}</h3>" + phase3(op))
            h.append(f"<h3>Information: {esc(op['name'])}</h3>" + info(col, op))
        h.append("</section>")
    h.append("</div></body></html>")
    with open(outp, "w") as f:
        f.write("\n".join(h))
    print(outp)


if __name__ == "__main__":
    main()
