"""C18/fix17 (C41): builds planning/adaptive_research_2026-10-23/fixture3d_v5.html
from numbers_v5.json, entries_v5.json, stock_*.npz and img/ (render_v5.py).
The page shape follows fix10/fixture3d_v4.py. Run run_v5.sh to rebuild all."""
import html, json, os, sys
import numpy as np
H = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.abspath(os.path.join(H, "..", ".."))
sys.path.insert(0, H)
from render_v5 import COLS_C, COLS_W, FX, tag, BLUE, ORANGE, RED  # noqa: E402

N = json.load(open(os.path.join(H, "numbers_v5.json")))
E = json.load(open(os.path.join(H, "entries_v5.json")))
CR = json.load(open(os.path.join(H, "crop_centres.json")))
IMG = "phase3/fix17/img"
CHANGE = {"contour_parallel": "control (fix6)",
          "zigzag_pick D20": "geodesic zigzag_pick, D20 (fix7)",
          "spiral_pick C35": "C35: the spiral core (fix12)",
          "spiral_pick D24": "D24: C35 + C38 G-b + C22a, slot removed (fix14)",
          "spiral_pick D25": "D25: D24 + G-CLIPAIR + C40 (fix15)"}
# The slot of D20: the straight row of lift-over descents from the seed (Corne y about -50, x 57..105;
# wanaka200 y about 125, x 122..220), counted on the planes where the D20 image shows it.
SLOT = {"corne": (-50.2, 57, 105, (17.0, 5.0)), "wanaka200": (124.9, 122, 220, (14.08,))}


def slot_dots(fx, col):
    yc, x0, x1, planes = SLOT[fx]
    return sum(1 for r in E[f"{fx} {col}"]["rows"] if r["style"] == "keep-down link" and r["descent_mm"] >= 1.0
               and r["removed_mm3"] > 0.1 and abs(r["y"] - yc) < 1.5 and x0 <= r["x"] <= x1 and r["end_plane"] in planes)


def area_over(fx, col):
    m = np.load(os.path.join(H, f"stock_{fx}_{tag(col)}.npz"))
    return round(float(((m["top"] - m["target"]) > 0.8).sum() * float(m["g"]) ** 2))


def numbers_table(fx, cols):
    n = {c: N[f"{fx} {c}"] for c in cols}
    e = html.escape
    rows = []
    def row(label, f):
        rows.append(f"<tr><th>{e(label)}</th>" + "".join(f"<td>{f(n[c], c)}</td>" for c in cols) + "</tr>")
    def grp(label):
        rows.append(f'<tr class="grp"><th colspan="{len(cols) + 1}">{e(label)}</th></tr>')
    grp("Descents and entries (op 1)")
    row("descents into stock (audit)", lambda r, c: r["descents"])
    row("lift-over descents (entries.py)", lambda r, c: r["lift_over"])
    row("plunge entries (trace spans)", lambda r, c: r["plunge_entries"])
    grp("Planar turns (op 1, G-code junctions)")
    row("turns over 30°: inside / outside the silhouette", lambda r, c: f'{r["turns_in"][1]} / {r["turns_out"][1]} (all {r["turns_in"][1] + r["turns_out"][1]})')
    row("reversals (over 90°): inside / outside", lambda r, c: f'{r["turns_in"][2]} / {r["turns_out"][2]} (all {r["turns_in"][2] + r["turns_out"][2]})')
    row("turn summary (C22): planned / cut / fed / unfed", lambda r, c: " / ".join(map(str, r["turn_summary"])) if r["turn_summary"] else "no line")
    grp("Time")
    row("plunge-feed share, program clock (op 1)", lambda r, c: f'{r["plunge_pct"]:.1f} % ({r["plunge_s"]:.1f} of {r["program_s"]:.1f} s)')
    row("cycle (summary.json, all ops)", lambda r, c: f'{r["cycle_s"]} s')
    grp("Stock after the first rough op")
    row("whole stock box, area over 0.8 mm above target", lambda r, c: f'{area_over(fx, c)} mm²')
    if fx == "corne":
        row("outside the silhouette, 3 to 9 mm band", lambda r, c: f'{r["band_3_9_mm2"]} mm²')
        row("island line (op 1): islands / against a wall / wall area", lambda r, c: f'{r["islands"][0]} / {r["islands"][1]} / {r["islands"][2]:.1f} mm²' if r["islands"] else "no line")
    grp("Hard bars")
    row("collisions: fed / rapid / checks failed", lambda r, c: " / ".join(map(str, r["collisions"])))
    row("export refused", lambda r, c: "yes" if r["export_refused"] else "no")
    def fed(r, c):
        if not r["fed_links"]:
            return "0"
        return f'{len(r["fed_links"])}: ' + "; ".join(f'{e(op)} move {mv} removes {mm:.2f} mm at ({e(p)})' for op, mv, mm, p in r["fed_links"])
    row("fed-link lines (stderr)", fed)
    row("deeper-than line (stderr)", lambda r, c: "; ".join(f'{a:.1f} % of samples, peak {b:.2f} mm' for a, b in r["deeper"]) if r["deeper"] else "0")
    return ('<div class="tablewrap"><table class="cmp"><tr class="head"><td></td>' +
            "".join(f"<td>{e(c)}<br><span class=\"commit\">{e(CHANGE[c])}</span></td>" for c in cols) + "</tr>" + "".join(rows) + "</table></div>")


def c40_table():
    out = []
    for run, label in (("corne_pick", "Corne D25 (corne_pick)"), ("wanaka200_pick", "wanaka200 D25 (wanaka200_pick)")):
        d = N["c40"][run]
        ops = [("op 1 entries (3D Rough 1)" if run == "corne_pick" else "op 1 entries (2 Back Rough)", d["op0"])]
        if "op1" in d:
            ops.append(("op 2 entries (3D Rough 1 (copy))", d["op1"]))
        for name, o in ops:
            split = ", ".join(f"{html.escape(k)} {v}" for k, v in o["by_kind_cause"].items())
            out.append(f'<tr><th>{html.escape(label)}: {name}</th><td>{o["entries"]}</td><td>{o["largez_share_pct"]:.2f} %</td><td>{o["other"]} ({o["other_share_pct"]:.2f} %)</td><td>{split}</td></tr>')
        j = d["join_op1"]
        split = ", ".join(f"{html.escape(k)} {v} ({j['share_pct'][k]:.1f} %)" for k, v in j["by_cause"].items())
        out.append(f'<tr><th>{html.escape(label)}: audit descents of op 1, joined</th><td>{j["descents"]} (matched {j["matched"]}, unmatched {j["unmatched"]})</td><td>{j["largez_share_pct"]:.1f} %</td><td>{j["by_cause"].get("None", 0)} ({j["other_share_pct"]:.2f} %)</td><td>{split}</td></tr>')
    return ('<div class="tablewrap"><table class="cmp"><tr class="head"><td></td><td>count</td><td>LargeZ share (LargeZ + LargeZHeld)</td><td>Other (cause None)</td><td>split by kind and cause</td></tr>'
            + "".join(out) + "</table></div>")


def fig(src, cap, alt):
    return (f'<figure><a href="{src}"><img src="{src}" alt="{html.escape(alt)}" loading="lazy"></a>'
            f'<figcaption>{html.escape(cap)}</figcaption></figure>')


def part_section(fx, cols, title):
    cfg = FX[fx]; c = CR[fx]
    s = [f'<h2>{html.escape(title)}</h2>', numbers_table(fx, cols)]
    s.append(f'<h3>Stock after the first rough op, whole stock box</h3>' + fig(
        f"{IMG}/stockmap_{fx}.png",
        "Stock height above the target after the first rough op, over the whole stock box (no mask). Blue line: the part outline. "
        f"Scale 0 to {cfg['dpp']:.2f} mm (one depth per pass); black is more.", f"{title} stock maps, five columns"))
    s.append('<h3>Strict planes and the 10 mm crop</h3><div class="grid">')
    for col in cols:
        imgs = []
        for k, z in enumerate(cfg["show"]):
            imgs.append(fig(f"{IMG}/plane_{fx}_{tag(col)}_{k}.png", f"Z {z:.2f}", f"{col} strict plane Z {z:.2f}"))
        imgs.append(fig(f"{IMG}/crop_{fx}_{tag(col)}.png", f"10 mm crop at ({c['x']:.1f}, {c['y']:.1f}), cuts from Z {c['plane']:.2f} up one depth per pass",
                        f"{col} 10 mm crop"))
        s.append(f'<div class="col"><h3>{html.escape(col)}</h3><div class="note">{html.escape(CHANGE[col])}</div><div class="imgs">{"".join(imgs)}</div></div>')
    s.append("</div>")
    return "".join(s)


def reading():
    f = N["image_facts"]; d = f["disc"]; L = f["links_z17_box_mm"]
    hc = "; ".join(f"({x:.1f}, {y:.1f})" for x, y in f["holes"])
    it = [
        f"<li><b>Three hole floors stay uncut on Corne after C35.</b> At {hc} the median stock in a 2 mm disc is "
        f"{d['spiral_pick C35'][0]:.2f} mm above the target for C35, D24 and D25 (all three holes). contour_parallel and D20 read "
        f"{d['contour_parallel'][0]:.2f} mm. See the three solid discs on the Corne stock map.</li>",
        f"<li><b>The D20 slot is gone from C35 on.</b> D20 has {slot_dots('corne', 'zigzag_pick D20')} lift-over descents on the slot row "
        f"of Corne at Z 17 and Z 5, and {slot_dots('wanaka200', 'zigzag_pick D20')} on wanaka200 at Z 14.08. C35, D24 and D25 have "
        f"{slot_dots('corne', 'spiral_pick C35')}, {slot_dots('corne', 'spiral_pick D24')}, {slot_dots('corne', 'spiral_pick D25')} on Corne and "
        f"{slot_dots('wanaka200', 'spiral_pick D25')} on wanaka200 (D25).</li>",
        f"<li><b>D24 has a band of link arcs at Z 17 on Corne.</b> In the box x 95 to 130, y −70 to −15, the linking steps on the plane are "
        f"{L['spiral_pick D24']} mm long for D24 and {L['spiral_pick D25']} mm for D25 (C35: {L['spiral_pick C35']} mm). They are the orange arcs "
        f"in the D24 Z 17 image.</li>",
        f"<li><b>The outer band shows on the whole-box maps.</b> On Corne, contour_parallel cuts the band all round "
        f"({area_over('corne', 'contour_parallel')} mm² over 0.8 mm in the box); D24 and D25 cut short runs in it "
        f"({area_over('corne', 'spiral_pick D24')} and {area_over('corne', 'spiral_pick D25')} mm²); D20 and C35 leave most of it "
        f"({area_over('corne', 'zigzag_pick D20')} and {area_over('corne', 'spiral_pick C35')} mm²).</li>",
        "<li><b>D24 and D25 add turns outside the Corne silhouette.</b> They have more than 2000 turns over 30° outside the "
        f"silhouette ({N['corne spiral_pick D24']['turns_out'][1]} and {N['corne spiral_pick D25']['turns_out'][1]}); C35 has "
        f"{N['corne spiral_pick C35']['turns_out'][1]}. On wanaka200 no planar junction lies outside the terrain footprint, so the outside "
        "column reads 0 for all arms.</li>",
    ]
    return "<h2>What the images show (author's reading, asserted)</h2><ol>" + "".join(it) + "</ol>"


def main():
    c40note = ("The C40 instrument tags each entry with a cause. 'Other' is an entry with no cause (None). "
               "The Other share is a known instrument gap: the waterline / clip gap of D25, fixed in D26. Read it as an upper bound "
               "on unexplained entries, not as a defect count.")
    method = ("Columns: contour_parallel (fix6), D20 zigzag_pick (fix7), C35 spiral_pick (fix12), D24 spiral_pick (fix14), "
              "D25 spiral_pick (fix15); all are committed artefacts. AgentSearch is out. "
              "Descents into stock: fix11/audit.py (vertical fed descents at one XY that remove stock, pecks included). "
              "Lift-over descents: a keep-down-link span that goes down 1.0 mm or more and removes more than 0.1 mm³ (entries.py). "
              "Plunge entries: the plunge-entry spans of the trace. "
              "Planar turns: junctions of two moves that both travel in XY; a reversal turns more than 90°. Inside / outside: the "
              "junction point against the hole-filled part silhouette (Corne) or the terrain footprint x 20 to 220, y 25 to 225 "
              "(wanaka200). Plunge-feed share: fed vertical-down moves of program.nc at their F, over the op's program time "
              "(rapids at the machine max feed). Cycle: summary.json; the Corne clock has no acceleration (its machine profile has "
              "no kinematics block), the wanaka200 clock uses the machine model (C39). "
              "3 to 9 mm band: stock more than 0.8 mm above the target, outside the silhouette, 3 to 9 mm from it (bands_d25.py rule). "
              "Strict plane: both sample ends within 0.01 mm of the plane Z.")
    body = (part_section("corne", list(COLS_C), "Corne") + part_section("wanaka200", list(COLS_W), "wanaka200 (setup 0)")
            + "<h2>D25: C40 entry causes per op</h2>" + f'<p class="note">{html.escape(c40note)}</p>' + c40_table())
    known = "".join(f"<li>{html.escape(k)}</li>" for k in (
        "On wanaka200 the Z 19.54 plane is the first plane; there the C35 spiral replaces the D20 rings. Z 4.0 has no strict-plane cut, so the page does not show it.",
        "The Corne cycle clock has no acceleration; the wanaka200 clock has. Compare cycles inside one part only.",
        "The stock maps are rebuilt from the sample path with a flat end mill of radius 3 mm, not read from the simulator."))
    title = "Fixture3d v5: renders of the fixes"
    page = f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)}</title><style>
:root{{--bg:#fff;--fg:#1a1a1a;--muted:#666;--line:#ddd;--card:#fafafa;--grp:#eef1f5}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126;--grp:#23272e}}}}
:root[data-theme="dark"]{{--bg:#16181c;--fg:#e8e8e8;--muted:#a0a0a0;--line:#333;--card:#1e2126;--grp:#23272e}}
body{{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px;max-width:1800px}}
h1{{font-size:20px;margin:0 0 6px}} h2{{font-size:17px;margin:22px 0 4px}} h3{{font-size:14px;margin:12px 0 6px}}
.note,.meta{{color:var(--muted);font-size:13px}} .commit{{color:var(--muted);font-weight:normal;font-size:12px}}
.tablewrap{{overflow-x:auto}} table{{border-collapse:collapse;width:100%;font-size:12.5px;margin-top:6px}}
td,th{{border:1px solid var(--line);padding:4px 6px;text-align:left;vertical-align:top}}
table.cmp th{{width:18%;font-weight:600}} tr.grp th{{background:var(--grp)}} tr.head td{{font-weight:600}}
.grid{{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:10px;margin-top:12px}} @media (max-width:1100px){{.grid{{grid-template-columns:1fr}}}}
.col{{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:8px;min-width:0}}
.imgs{{display:grid;grid-template-columns:1fr;gap:6px}}
figure{{margin:0 0 8px}} img{{width:100%;height:auto;background:#fff;border:1px solid var(--line)}} figcaption{{font-size:12px;margin-top:2px}}
.key span{{display:inline-block;margin-right:12px}}
</style></head><body>
<h1>{html.escape(title)}</h1>
<div class="meta">C41 (fix17). Builder phase3/fix17/fixture3d_v5.py; checks phase3/fix17/assert_fixture3d_v5.py. Click an image for the full size.</div>
<p class="note">{html.escape(method)}</p>
<p class="key"><span style="color:{BLUE}">&#9632; blue: cutting moves on the plane</span><span style="color:{ORANGE}">&#9632; orange line: a link on the plane</span><span style="color:{RED}">&#9675; red ring: a plunge entry</span><span style="color:{ORANGE}">&#9679; orange dot: a lift-over link that goes down into stock</span><span style="color:#2a8a2a">&#9633; green square: the 10 mm crop</span></p>
{reading()}
{body}
<h2>Known items</h2><ul>{known}</ul>
</body></html>'''
    open(os.path.join(ROOT, "fixture3d_v5.html"), "w").write(page)


if __name__ == "__main__":
    main()
