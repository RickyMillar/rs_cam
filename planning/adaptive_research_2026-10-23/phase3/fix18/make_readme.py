"""C42: write README.md and quotes.json from lowtravel.json.
`facts(D)` is the one place that derives a quoted number; assert_fix18.py
calls it on a fresh run of lowtravel.py and compares.
Run: python3 -I make_readme.py"""
import json, os, statistics, sys

H = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, H)
import lowtravel as L  # noqa: E402

SP = ("corne spiral_pick", "wanaka200 spiral_pick")
CP = ("corne contour_parallel", "wanaka200 contour_parallel")
OP = {"corne": "'3D Rough 1'", "wanaka200": "'2 Back Rough'"}
MULTS = (1, 2, 4, 8)
FEEDS = (("link", "low_link_s"), ("cut", "low_cut_s"), ("rapid", "low_rapid_s"))


def groups(col, rows):
    cfg = L.FIX[col]
    out = [("all", rows)]
    if cfg["all_spans"]:
        out += [("into stock", [r for r in rows if r["into_stock"]]),
                ("not into stock", [r for r in rows if not r["into_stock"]]),
                ("with a rapid", [r for r in rows if r["retract"]]),
                ("no rapid (keep-down)", [r for r in rows if not r["retract"]])]
    for p in cfg["planes"]:
        out.append((f"Z {p:g}", [r for r in rows if r["level"] == p]))
    if not cfg["all_spans"]:
        for c in sorted({r["cause"] for r in rows}):
            out.append((f"C40 {c}", [r for r in rows if r["cause"] == c]))
    return out


def stats(col, rows):
    cfg = L.FIX[col]
    n = len(rows)
    p = [r for r in rows if r["path"]]
    s = {"n": n, "path": len(p), "path_pct": 100 * len(p) / n if n else 0.0,
         "lift_above": sum(r["lift_above"] for r in rows),
         "blocked_lift": sum((not r["path"]) and (not r["lift_above"]) for r in rows),
         "strict": sum(r["plunge_free"] and r["path"] for r in rows),
         "side_ok": sum(r["side_le_stepover"] for r in p),
         "ap_ok": sum(r["side_le_stepover"] and r["drop_mm"] - 0.5 <= cfg["dpp"] + 1e-9 for r in p),
         "straight_med": statistics.median(r["straight_mm"] for r in rows) if rows else 0.0,
         "travel_med": statistics.median(r["travel_mm"] for r in p) if p else 0.0,
         "side_med": statistics.median(r["side_mm"] for r in p) if p else 0.0,
         "drop_med": statistics.median(r["drop_mm"] for r in rows) if rows else 0.0,
         "pf_path_s": sum(r["plunge_feed_s"] for r in p),
         "span_path_s": sum(r["span_s"] for r in p)}
    for name, key in FEEDS:
        s[f"low_{name}_s"] = sum(r[key] for r in p)
        s[f"worth_{name}"] = sum(r[key] < r["span_s"] for r in p)
        s[f"saved_{name}_s"] = sum(max(0.0, r["span_s"] - r[key]) for r in p)
    # C42b: the swept disc along the side leg
    s["leg_ap_ok"] = sum(r["leg_ap_ok"] for r in p)
    s["leg_ae_ok"] = sum(r["leg_ae_ok"] for r in p)
    s["leg_both"] = sum(r["leg_ap_ok"] and r["leg_ae_ok"] for r in p)
    s["leg_both_pct"] = 100 * s["leg_both"] / n if n else 0.0
    s["leg_ap_med"] = statistics.median(r["leg_ap_mm"] for r in p) if p else 0.0
    s["leg_ae_med"] = statistics.median(r["leg_ae_mm"] for r in p) if p else 0.0
    s["leg_ae_so_med"] = statistics.median(r["leg_ae_mm"] / cfg["stepover"] for r in p) if p else 0.0
    s["leg_ap_max"] = max((r["leg_ap_mm"] for r in p), default=0.0)
    s["leg_both_span_s"] = sum(r["span_s"] for r in p if r["leg_ap_ok"] and r["leg_ae_ok"])
    s["leg_both_saved_cut_s"] = sum(max(0.0, r["span_s"] - r["low_cut_s"]) for r in p if r["leg_ap_ok"] and r["leg_ae_ok"])
    for m in MULTS:
        s[f"under_{m}D_pct"] = 100 * sum(r["travel_mm"] < m * L.D for r in p) / len(p) if p else 0.0
    s["side_ok_pct"] = 100 * s["side_ok"] / n if n else 0.0
    s["ap_ok_pct"] = 100 * s["ap_ok"] / n if n else 0.0
    return s


def facts(D):
    F = {}
    for col, d in D.items():
        F[f"{col}|selected"] = d["selected"]
        F[f"{col}|c40_pool"] = d["c40_pool"]
        F[f"{col}|runtime_s"] = d["runtime_s"]
        for name, rows in groups(col, d["rows"]):
            for k, v in stats(col, rows).items():
                F[f"{col}|{name}|{k}"] = v
    return F


def write(D):
    F = facts(D)
    quotes = {}

    def q(key, fmt):
        v = F[key]; t = format(v, fmt)
        quotes[key] = {"value": v, "fmt": fmt, "text": t}
        return t

    md = []
    A = md.append
    c, w = "corne spiral_pick|all|", "wanaka200 spiral_pick|all|"
    cc, wc = "corne contour_parallel|all|", "wanaka200 contour_parallel|all|"
    A("# fix18: can a descent be a low travel over cut ground? (C42, 2026-10-26)")
    A("")
    A("Python only, on committed run artefacts: the C35 spiral_pick runs (fix12) and, by the operator's ruling")
    A("that the keep-down link is a shared component, the contour_parallel runs (fix6). No code edit, no cargo,")
    A("no commit. `lowtravel.py` does the work (its docstring gives each rule), `make_readme.py` writes this")
    A("file and `quotes.json`, and `assert_fix18.py` runs `lowtravel.py` again from the raw files and checks")
    A("every number here, plus independent checks of the selection, the frames, the stock rebuild and the paths.")
    A("Run: `python3 -I lowtravel.py && python3 -I make_readme.py && python3 -I assert_fix18.py` (about 5 min).")
    A("")
    A("## Answer")
    A("")
    A("**spiral_pick: the descents are not travels, they are side steps.** The lift point and the plunge point")
    A(f"are almost at one XY: the median straight distance is {q(c + 'straight_med', '.2f')} mm (Corne) and {q(w + 'straight_med', '.2f')} mm")
    A(f"(wanaka200); the median drop is {q(c + 'drop_med', '.2f')} and {q(w + 'drop_med', '.2f')} mm. The plunge point is in stock by the")
    A("selection, so no path over cut ground reaches it; a path reaches the free point nearest to it, and a short")
    A("side leg at the level does the rest.")
    A("")
    A(f"- Strictly over cut ground to the plunge point: {q(c + 'strict', 'd')} of {q('corne spiral_pick|selected', 'd')} (Corne) and "
      f"{q(w + 'strict', 'd')} of {q('wanaka200 spiral_pick|selected', 'd')} (wanaka200).")
    A(f"- With a side leg at the end: {q(c + 'path', 'd')} ({q(c + 'path_pct', '.1f')} %) and {q(w + 'path', 'd')} ({q(w + 'path_pct', '.1f')} %);")
    A(f"  median side leg {q(c + 'side_med', '.2f')} and {q(w + 'side_med', '.2f')} mm. Side leg within one stepover (an ordinary cut):")
    A(f"  {q(c + 'side_ok', 'd')} ({q(c + 'side_ok_pct', '.1f')} % of all) and {q(w + 'side_ok', 'd')} ({q(w + 'side_ok_pct', '.1f')} %); and its axial depth")
    A(f"  (drop − 0.5 mm) within the depth per pass: {q(c + 'ap_ok', 'd')} ({q(c + 'ap_ok_pct', '.1f')} %) and {q(w + 'ap_ok', 'd')} ({q(w + 'ap_ok_pct', '.1f')} %).")
    A(f"- No path: the lift floor is above the plunge level + 0.5 mm in {q(c + 'lift_above', 'd')} and {q(w + 'lift_above', 'd')}; the lift")
    A(f"  point itself has stock over its disc in {q(c + 'blocked_lift', 'd')} and {q(w + 'blocked_lift', 'd')}.")
    A(f"- Time of the descents with a path: {q(c + 'span_path_s', '.1f')} s and {q(w + 'span_path_s', '.1f')} s now (their whole spans;")
    A(f"  plunge feed alone {q(c + 'pf_path_s', '.1f')} s and {q(w + 'pf_path_s', '.1f')} s). The low travels: {q(c + 'low_link_s', '.1f')} / {q(w + 'low_link_s', '.1f')} s at the")
    A(f"  link feed (500 mm/min), {q(c + 'low_cut_s', '.1f')} / {q(w + 'low_cut_s', '.1f')} s at the cut feed. Taking the low travel only where it is")
    A(f"  faster (at the cut feed) saves {q(c + 'saved_cut_s', '.1f')} s of {q('corne spiral_pick|runtime_s', '.1f')} s and "
      f"{q(w + 'saved_cut_s', '.1f')} s of {q('wanaka200 spiral_pick|runtime_s', '.1f')} s (sample clock).")
    A("")
    A("**C42b, the side leg on the swept disc** (section C42b): the disc swept along the leg meets stock within")
    A("depth per pass + 0.5 mm axially and within one stepover radially for")
    A(f"{q(c + 'leg_both', 'd')} of {q('corne spiral_pick|selected', 'd')} ({q(c + 'leg_both_pct', '.1f')} %) on Corne and {q(w + 'leg_both', 'd')} of "
      f"{q('wanaka200 spiral_pick|selected', 'd')} ({q(w + 'leg_both_pct', '.1f')} %) on wanaka200.")
    A(f"Axially every Corne leg passes ({q(c + 'leg_ap_ok', 'd')} of {q(c + 'path', 'd')}, max {q(c + 'leg_ap_max', '.2f')} mm); the radial test removes")
    A(f"the rest. Median ae {q(c + 'leg_ae_med', '.2f')} and {q(w + 'leg_ae_med', '.2f')} mm ({q(c + 'leg_ae_so_med', '.2f')} and {q(w + 'leg_ae_so_med', '.2f')} of the stepover). On those")
    A(f"entries the low travel at the cut feed saves {q(c + 'leg_both_saved_cut_s', '.1f')} s and {q(w + 'leg_both_saved_cut_s', '.1f')} s. On contour_parallel")
    A(f"the same test passes {q(cc + 'leg_both', 'd')} of {q('corne contour_parallel|selected', 'd')} and {q(wc + 'leg_both', 'd')} of {q('wanaka200 contour_parallel|selected', 'd')} entries, "
      f"with {q(cc + 'leg_both_saved_cut_s', '.1f')} s and {q(wc + 'leg_both_saved_cut_s', '.1f')} s saved at the cut feed.")
    A("")
    A("**contour_parallel: the shared link would save little, at feed.** Its entries are long hops: the median")
    A(f"straight distance is {q(cc + 'straight_med', '.1f')} mm (Corne) and {q(wc + 'straight_med', '.1f')} mm (wanaka200). A path over cut ground")
    A(f"exists for {q(cc + 'path', 'd')} of {q('corne contour_parallel|selected', 'd')} entries ({q(cc + 'path_pct', '.1f')} %) and {q(wc + 'path', 'd')} of "
      f"{q('wanaka200 contour_parallel|selected', 'd')} ({q(wc + 'path_pct', '.1f')} %),")
    A(f"median {q(cc + 'travel_med', '.1f')} and {q(wc + 'travel_med', '.1f')} mm. Those entries take {q(cc + 'span_path_s', '.1f')} s and "
      f"{q(wc + 'span_path_s', '.1f')} s now (mostly rapids). The same")
    A(f"travels take {q(cc + 'low_link_s', '.1f')} / {q(wc + 'low_link_s', '.1f')} s at the link feed, {q(cc + 'low_cut_s', '.1f')} / {q(wc + 'low_cut_s', '.1f')} s at the cut feed and "
      f"{q(cc + 'low_rapid_s', '.1f')} / {q(wc + 'low_rapid_s', '.1f')} s at the rapid rate.")
    A("Per entry, the low travel is faster than the entry now for:")
    A("")
    A("| Column | Entries with a path | Faster at the link feed (saved s) | at the cut feed (saved s) | at the rapid rate (saved s) | Op time s |")
    A("|---|---|---|---|---|---|")
    for col in SP + CP:
        k = f"{col}|all|"
        A(f"| {col} | {q(k + 'path', 'd')} | " + " | ".join(f"{q(k + f'worth_{n}', 'd')} ({q(k + f'saved_{n}_s', '.1f')})" for n, _ in FEEDS)
          + f" | {q(col + '|runtime_s', '.1f')} |")
    A("")
    A("So on contour_parallel a keep-down link at feed pays only on the short hops; a low link at the rapid")
    A("rate pays more. The rows by kind are below.")
    A("")
    A("## Method")
    A("")
    A("- Selection. spiral_pick: `fix12/entries_fix12.py`'s rule, keep-down-link spans whose entry_plunge samples")
    A(f"  descend 1 mm or more and remove more than 0.1 mm³ ({q('corne spiral_pick|selected', 'd')} and {q('wanaka200 spiral_pick|selected', 'd')}).")
    A("  contour_parallel: every entry span of op 1 (both styles); 'into stock' marks the same rule, 'with a rapid'")
    A("  a span that holds a G0 (a retract).")
    A("- Lift point: the tool position at the start of the span, from `program.nc` (move i = command i + 1 of the op")
    A("  section, as `fix11/audit.py` maps it). Frames: Corne sim = G-code + (stock box corner XY, 23); wanaka200")
    A("  sim = G-code + (0, 0, 25).")
    A("- Plunge point: the last entry_plunge sample of the span (no such sample: the span's own entry point). Level Z:")
    A("  the first later sample that is not entry_plunge or retract (no plunge sample: the span's entry Z). Drop: the")
    A("  highest entry_plunge sample Z less the level Z. Level plane: the highest plane at or below the level Z + 0.35.")
    A("- Stock top at the entry: `fix9/stockmap.py`'s rebuild (flat end mill, R 3, path densified at g/2) over the")
    A("  samples before the span, stamped in order. Grid: Corne 0.25 mm, wanaka200 0.5 mm.")
    A("- Free centre cell: the highest stock over its disc of R ≤ level Z + 0.5 mm (stay_down_clearance_mm). Dijkstra")
    A("  over free cells, 16 neighbours (knight steps need their two crossed cells free), from a virtual source joined to")
    A("  the free cells within 2 g of the lift point at their true distance, in a window of the two points + 24 mm.")
    A("  The path ends at the reachable free cell nearest the plunge point; travel = free path + side leg. 16-neighbour")
    A("  grid paths are at most 2.7 % longer than the true shortest path.")
    A("- Times on the samples' clock (no acceleration), as `fix11/audit.py` reads them: plunge feed = the span's fed,")
    A("  vertical, downward moves; span = all samples of the span. Low travel = travel over 500 mm/min (link_feed_rate),")
    A("  over the cut feed (the adaptive3d keep-down feed, path.rs:1576), or over max_feed_mm_min (4000 / 10000).")
    A("  'Saved' sums span − low travel over the entries where that is positive.")
    A("- C40 cause (spiral_pick only): the nearest unused op-1 'C40 entry' line of the D25 build (fix15) within 0.5 mm")
    A(f"  XY and 1.0 mm Z, as `fix15/c40_join.py`; pools {q('corne spiral_pick|c40_pool', 'd')} and {q('wanaka200 spiral_pick|c40_pool', 'd')}. A different build: a guide only.")
    A("")
    head = ("| Group | Entries | With a path (%) | Side leg ≤ stepover | and ap ≤ depth per pass | Lift above level | Lift not free | "
            "Straight median mm | Travel median mm | Travel < 1 / 2 / 4 / 8 D (% of paths) | Plunge feed now s | Span now s | "
            "Low travel at 500 / cut feed / rapid s | Saved at cut feed / rapid s |")
    for col in SP + CP:
        fx = L.FIX[col]["fixture"]
        A(f"## {col}, {OP[fx]}")
        A("")
        A(head)
        A("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
        for name, rows in groups(col, D[col]["rows"]):
            k = f"{col}|{name}|"
            if F[k + "n"] == 0:
                continue
            A(f"| {name} | {q(k + 'n', 'd')} | {q(k + 'path', 'd')} ({q(k + 'path_pct', '.1f')}) | {q(k + 'side_ok', 'd')} | {q(k + 'ap_ok', 'd')} | "
              f"{q(k + 'lift_above', 'd')} | {q(k + 'blocked_lift', 'd')} | {q(k + 'straight_med', '.2f')} | {q(k + 'travel_med', '.2f')} | "
              + " / ".join(q(k + f"under_{m}D_pct", ".1f") for m in MULTS)
              + f" | {q(k + 'pf_path_s', '.1f')} | {q(k + 'span_path_s', '.1f')} | {q(k + 'low_link_s', '.1f')} / {q(k + 'low_cut_s', '.1f')} / {q(k + 'low_rapid_s', '.1f')} | "
              f"{q(k + 'saved_cut_s', '.1f')} / {q(k + 'saved_rapid_s', '.1f')} |")
        A("")
    A("Plunge feed, span and low travel columns sum over the entries with a path.")
    A("")
    A("## C42b: the side leg on the swept disc")
    A("")
    A("The side-leg test above reads only the leg's length. Here the tool disc is swept along each side leg (path end")
    A("→ plunge point) and sampled as `corridor_ceiling_z` samples a corridor (`adaptive3d/clearing.rs:872-888`):")
    A("n = ceil(length / (g / 2)) steps, both ends included, the highest stock top over the disc of R at each step")
    A("(cells whose centre is within R; the Rust reads a conservative top per cell, so it can read a little higher).")
    A("")
    A("- Axial depth ap = that highest top − the level Z. Within limit: ap ≤ depth per pass + 0.5 mm (Corne 6.5,")
    A("  wanaka200 5.96).")
    A("- Radial engagement ae = R − the least distance from a step to the centre of a cell whose stock stands above")
    A("  the level Z + 0.5 mm (at least 0): how far the swept disc reaches into that stock. Within limit: ae ≤ the stepover")
    A("  (Corne 1.2, wanaka200 2.0 mm).")
    A("")
    A("| Column | Group | Entries | With a path | ap within | ae within | Both (% of entries) | ap median / max mm | ae median mm | ae / stepover median | Span now of 'both' s | Saved at cut feed on 'both' s |")
    A("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for col in SP + CP:
        for name, rows in groups(col, D[col]["rows"]):
            k = f"{col}|{name}|"
            if F[k + "path"] == 0:
                continue
            A(f"| {col} | {name} | {q(k + 'n', 'd')} | {q(k + 'path', 'd')} | {q(k + 'leg_ap_ok', 'd')} | {q(k + 'leg_ae_ok', 'd')} | "
              f"{q(k + 'leg_both', 'd')} ({q(k + 'leg_both_pct', '.1f')}) | {q(k + 'leg_ap_med', '.2f')} / {q(k + 'leg_ap_max', '.2f')} | "
              f"{q(k + 'leg_ae_med', '.2f')} | {q(k + 'leg_ae_so_med', '.2f')} | {q(k + 'leg_both_span_s', '.1f')} | {q(k + 'leg_both_saved_cut_s', '.1f')} |")
    A("")
    A("")
    A("## Reading")
    A("")
    A("- spiral_pick lifts where the next point is a short step away but its column still stands (C40 LargeZ is the")
    A("  main cause in the join). There the useful link is not a travel but a side step at the level, about one")
    A("  stepover long; where the drop is within the depth per pass it is an ordinary cut and removes the lift, the")
    A("  link and the plunge. Drops over the depth per pass (wanaka200 has drops near 11 mm) need the descent.")
    A("- On spiral_pick, Corne Z 11 and Z 5 (36 each) and wanaka200 Z 19.54 and Z 14.08 (80 each) have the same XY")
    A("  and path figures: the same descents repeat at each level; only the drops and times differ.")
    A("- contour_parallel's entries are retracts over tens of millimetres. A keep-down link at feed beats the")
    A("  rapid only on the short hops; most of the gain of a shared link there would need it to run at the rapid")
    A("  rate over cut ground, which the samples' clock (no acceleration) favours.")
    A("")
    A("## Limits")
    A("")
    A("- The stock is the sample rebuild of `fix9/stockmap.py`, not the CLI's dexel stock.")
    A("- The side leg is a straight cut; its engagement is read only as its length against the stepover.")
    A("- Times are the samples' clock (no acceleration). A rapid's lift and descent cost more with acceleration;")
    A("  a low travel has no lift, so the clock is not biased against the low travel.")
    A("- The C40 causes come from the D25 build, not fix12. contour_parallel (fix6) is a different build from fix12.")
    A("")
    open(os.path.join(H, "README.md"), "w").write("\n".join(md))
    json.dump(quotes, open(os.path.join(H, "quotes.json"), "w"), indent=1)


if __name__ == "__main__":
    write(json.load(open(os.path.join(H, "lowtravel.json"))))
