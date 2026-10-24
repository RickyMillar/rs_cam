# C21: write P2_C21_entry_cost.md and quotes_c21.json from results_c21.json.
# Run: python3 -I note_c21.py   (after run_c21.py)
# `facts(res, c19)` is the one place that derives a quoted number;
# assert_c21.py calls it on a fresh run and compares.
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
RES = os.path.dirname(os.path.dirname(HERE))
C19 = os.path.join(RES, "phase2", "c19")
NAMES = ("rivmap100", "corne", "wanaka200")
LABEL = {"rivmap100": "rivmap100", "corne": "Corne", "wanaka200": "wanaka200"}
ARC_HOPS = (2.0, 5.0, 8.0)
LOBE_HOPS = (6.0, 20.0, 50.0, 69.0)
C19_FEED = 1500.0  # the replay's cut feed (P2_C19_march_arms.md, Limits)
FEED = {"rivmap100": 2400.0, "corne": 1518.75, "wanaka200": 1000.0}


def c19_card():
    src = open(os.path.join(C19, "page_c19.py")).read()
    return re.search(r'^CARD = "([a-z_]+)"', src, re.M).group(1)


def row(curves, key, L):
    return next(x for x in curves[key] if x["L"] == L)


def facts(res, c19):
    """Every number the note quotes, by key."""
    F = {}
    for n in NAMES:
        r = res[n]
        cv = r["curves"]
        keys = list(cv)
        first_level = keys[1]  # "<first depth>|level"
        last = keys[-3:]  # deepest: open, level, fresh
        for L in sorted(set(ARC_HOPS + LOBE_HOPS + (0.0,))):
            x = row(cv, first_level, L)
            F[f"{n}|first|L{L:g}|entry"] = x["entry_s"]
            F[f"{n}|first|L{L:g}|keep"] = x["keep_s"]
            F[f"{n}|first|L{L:g}|pen"] = x["penalty_s"]
        for col, key in (("open1", keys[0]), ("open", last[0]), ("level", last[1]), ("fresh", last[2])):
            F[f"{n}|{col}|L5|pen"] = row(cv, key, 5.0)["penalty_s"]
        x = row(cv, first_level, 5.0)["parts"]
        for part in ("lift", "travel", "descent", "entry"):
            F[f"{n}|first|L5|part|{part}"] = x.get(part, 0.0)
        if "curves_generic_accel" in r:
            g = r["curves_generic_accel"]
            gk = list(g)
            for L in (5.0,):
                F[f"{n}|gen|first|L5|pen"] = row(g, gk[1], L)["penalty_s"]
                F[f"{n}|gen|level|L5|pen"] = row(g, gk[-2], L)["penalty_s"]
            F[f"{n}|gen|first|L50|pen"] = row(g, gk[1], 50.0)["penalty_s"]
        if "curves_tuned_rates" in r:
            t = r["curves_tuned_rates"]
            tk = list(t)
            F[f"{n}|tuned|first|L5|pen"] = row(t, tk[1], 5.0)["penalty_s"]
            F[f"{n}|tuned|level|L5|pen"] = row(t, tk[-2], 5.0)["penalty_s"]
            F[f"{n}|tuned|open|L5|pen"] = row(t, tk[-3], 5.0)["penalty_s"]
        F[f"{n}|first|cross_L"] = r["cross_L"]
        g = r["gcode"]
        F[f"{n}|gc|total"] = g["total_s"]
        F[f"{n}|gc|phase0"] = g["phase0_runtime_s"]
        F[f"{n}|gc|err_pct"] = 100 * (g["total_s"] - g["phase0_runtime_s"]) / g["phase0_runtime_s"]
        F[f"{n}|gc|plunge_s"] = g["plunge_feed_s"]
        F[f"{n}|gc|rapid_s"] = g["rapid_s"]
        F[f"{n}|gc|lifts"] = g["lifts"]
        c = r["cross"]
        F[f"{n}|x|entries"] = c["n_entries"]
        F[f"{n}|x|keep"] = c["n_keep"]
        F[f"{n}|x|hop"] = c["hop_mm"]
        F[f"{n}|x|d_f"] = c["fed_descent_mm"]
        F[f"{n}|x|retract_entry"] = c["retract_entry_s"]
        F[f"{n}|x|style"] = c["style_s"]
        F[f"{n}|x|level_style"] = c["level_style_s"]
        F[f"{n}|x|model"] = c["model_s"]
        F[f"{n}|x|gcode"] = c["gcode_s"]
        F[f"{n}|x|model_pct"] = 100 * c["model_s"] / c["runtime_s"]
        F[f"{n}|x|gcode_pct"] = 100 * c["gcode_s"] / c["runtime_s"]
        F[f"{n}|x|ratio"] = c["model_s"] / c["gcode_s"]
    p0 = res["rivmap100"]["phase0"]
    F["rivmap100|p0|entry_s"] = p0["entry_s"]
    F["rivmap100|p0|runtime"] = p0["runtime_s"]
    F["rivmap100|p0|pct"] = 100 * p0["entry_s"] / p0["runtime_s"]
    F["rivmap100|p0|per_entry"] = p0["entry_s"] / sum(p0["entries"].values())
    lvl = F["rivmap100|x|level_style"]
    F["rivmap100|lvl_model_pct"] = 100 * F["rivmap100|x|entries"] * (lvl + F["rivmap100|first|L5|part|lift"]
                                                                     + F["rivmap100|first|L5|part|descent"]) / p0["runtime_s"]
    F["rivmap100|gc|path_per_down"] = res["rivmap100"]["gcode"]["plunge_feed_path_mm"] / res["rivmap100"]["gcode"]["plunge_feed_down_mm"]
    F["ladder|entry_s"] = 538.0
    F["ladder|total_s"] = 1379.0
    F["ladder|pct"] = 100 * 538.0 / 1379.0
    # C19 break-even, from C19's own results for the plan its cards draw.
    card = c19["card"]
    for nm in ("star", "pocket"):
        a = c19["res"][nm]["a"][card]
        for src in ("e", "march"):
            x = c19["res"][nm][src][card]
            be = 60 * (x["time_cut_feed_min"] - a["time_cut_feed_min"]) / (a["entries"] - x["entries"])
            F[f"c19|{nm}|{src}|be"] = be
            F[f"c19|{nm}|{src}|entries"] = x["entries"]
            for n in NAMES:
                F[f"c19|{nm}|{src}|be|{n}"] = be * C19_FEED / FEED[n]
        F[f"c19|{nm}|a|entries"] = a["entries"]
    return F


def load_c19():
    return dict(card=c19_card(), res=json.load(open(os.path.join(C19, "results_c19.json"))))


def write(res, c19):
    F = facts(res, c19)
    quotes = {}

    def q(key, fmt):
        v = F[key]
        t = format(v, fmt)
        quotes[key] = dict(value=v, fmt=fmt, text=t)
        return t

    def s(key):
        return q(key, ".2f")

    def verdict(n, col_key):
        pen = F[col_key]
        out = []
        for nm in ("star", "pocket"):
            for src, lab in (("e", "clipped (e)"), ("march", "clipped march")):
                be = F[f"c19|{nm}|{src}|be|{n}"]
                out.append(f"{lab} {nm}: {'plan' if pen > be else '(a)'}")
        return "; ".join(out)

    md = []
    A = md.append
    A("# P2 C21 — the cost of one entry, from rs_cam's own cycle-time model (2026-10-24)")
    A("")
    A("Status: COMPLETE (Python only; no Rust edited, no cargo run, no commit).")
    A("Scripts: `phase2/c21/` (`model.py` the port, `gcode.py`, `run_c21.py`,")
    A("`note_c21.py`, `assert_c21.py`). Run `run_c21.py`, then `note_c21.py`, then")
    A("`assert_c21.py`. `note_c21.py` writes this note; `assert_c21.py` runs the model")
    A("again and checks every quoted number, and checks the port against closed forms.")
    A("")
    A("## 1. Answer")
    A("")
    A("An entry costs what its fed descent costs. At a 5 mm hop the lift, the rapid across and")
    A("the rapid down take less than one second on each machine. The entry style then feeds down at the")
    A("plunge feed, and on rivmap100 the helix feeds 2π·1.8 mm of path for each 1 mm of")
    A("descent. At a 5 mm hop, first level, into the level's own stock, the penalty")
    A("against a keep-down feed at the cut feed is:")
    A("")
    A(f"- rivmap100 (helix): **{s('rivmap100|first|L5|pen')} s**.")
    A(f"- Corne (plunge, profile with no kinematics, the simulator's no-acceleration basis): **{s('corne|first|L5|pen')} s**; "
      f"with the 200 mm/s² default that the linker reads: {s('corne|gen|first|L5|pen')} s.")
    A(f"- wanaka200 (plunge): **{s('wanaka200|first|L5|pen')} s**.")
    A("")
    A(f"Into a column that is already cut (0.5 mm fed descent), the penalty falls to {s('rivmap100|open1|L5|pen')} s, "
      f"{s('corne|open1|L5|pen')} s and {s('wanaka200|open1|L5|pen')} s. "
      f"Into a fresh column at the deepest level, it rises to {s('rivmap100|fresh|L5|pen')} s, "
      f"{s('corne|fresh|L5|pen')} s and {s('wanaka200|fresh|L5|pen')} s.")
    A("")
    A("Against C19's break-even (rescaled to each fixture's cut feed): on rivmap100 the")
    A("clipped (e) and the clipped march are faster than (a) on both shapes; on wanaka200 and on")
    A("Corne (its simulator's basis), (a) is faster on both shapes. With the 200 mm/s² envelope, Corne")
    A("tips to the clipped (e) on the star only. Section 5 gives the order of magnitude, not a")
    A("fixture verdict: the 2D shapes are not the fixtures.")
    A("")
    A("## 2. The model, with citations (HEAD 6b194b59)")
    A("")
    A("The port is `phase2/c21/model.py`. Each function cites the Rust lines it ports.")
    A("")
    A("- The integrator: `compute_cycle_time_breakdown` (`machine/kinematics.rs:1177-1247`) over")
    A("  `digest_moves` (`:1071-1118`) and `walk_junctions` (`:1131-1171`). Each move is a trapezoid,")
    A("  `trapezoidal_time` (`:1582-1610`): accelerate from v_in to the cruise ceiling, cruise, decelerate")
    A("  to v_out; a short move is a triangle with v_peak² = a·L + (v_in² + v_out²)/2.")
    A("- The acceleration is direction-aware, `effective_accel` (`:396-415`): min_i(a_i / |d_i|). A Z move")
    A("  runs at the Z axis value (270 mm/s² on the Shapeoko profile), an XY move at 500 mm/s².")
    A("- The cruise ceiling is the commanded feed, capped by `max_feed` (`:1098`) and by the per-axis")
    A("  rate when the profile has one (`cruise_ceiling`, `:724-740`). A rapid runs at")
    A("  max(rapid, max_feed) (`:1189`).")
    A("- A junction runs at GRBL's junction-deviation speed √(a·R), R = δ·sin(θ/2)/(1 − sin(θ/2))")
    A("  (`junction_velocity`, `:1538-1567`). **A junction next to a rapid is a full stop** (`:1548-1550`).")
    A("  A 90° corner at δ = 0.02 mm and 500 mm/s² runs at 4.9 mm/s, nearly a stop too.")
    A("- The simulator calls the integrator with the profile's kinematics, `max_feed_mm_min`")
    A("  (`session/compute.rs:2305-2311`) and the rapid rate `PostConfig::rapid_feed_mm_min`")
    A("  (`gcode/mod.rs:1430-1436`: `max_feed_mm_min` unless high-feedrate mode is on)")
    A("  (`compute/simulate.rs:2474-2479`). A profile with no kinematics gets no integrator: the")
    A("  time is each move's length over its feed, rapids at the rapid rate")
    A("  (`dexel_stock/simulation.rs:579`, `:1032`; basis `SimulatedNoAccel`, `session/cycle_time.rs:62`).")
    A("- The linker's envelope reads `effective_kinematics`, which gives a profile with no")
    A("  kinematics the 200 mm/s² `generic_wood_router` (`machine/mod.rs:679-697`, `kinematics.rs:325-330`).")
    A("")
    A("The entry, as the adaptive3d emitter writes it (`adaptive3d/path.rs:1514-1655`):")
    A("")
    A("1. Lift: a rapid straight up to `safe_z` (`:1460-1469`). `safe_z` = max(post safe Z, stock top + 5)")
    A("   (`compute/config.rs:162-171`); on all three fixtures it is the stock top + 5.")
    A("2. Travel: a rapid across at `safe_z` (`:1583-1586`).")
    A("3. Descent: a rapid down to the rapid floor + 0.5 mm (`descent_start_z`, `:1323-1337`). The rapid")
    A("   floor is the planner's conservative stock top over the entry footprint (tool radius, plus the")
    A("   helix radius for a helix, `:1201-1205`).")
    A("4. The entry style from there to the cut Z, at the plunge feed:")
    A("   - plunge: the peck ladder (`emit_peck_plunge`, `:164-195`): pecks of one depth per pass, each")
    A("     with a 0.5 mm rapid back up, then the rest;")
    A("   - helix (`dressup/entry_descent.rs:779-918`): 36 steps per turn, pitch/36 down per step, at")
    A("     the ramp feed (unset on the fixtures, so the plunge feed, `compute/catalog.rs:1636-1640`),")
    A("     then a feed back to the centre. Radius = D × `helix_radius_factor`, capped at the flat bottom")
    A("     (`compute/operation_configs.rs:1820-1830`, `compute/config.rs:986-1000`): 0.3 × 6 = 1.8 mm.")
    A("     A column whose floor is at the cut Z is air: one straight 0.5 mm feed (`entry_descent.rs:444-470`).")
    A("")
    A("A rs_cam keep-down link (`try_emit_stay_down_link`, `path.rs:1240-1317`) ends at the same rapid")
    A("floor, and the entry style still takes the material below it (`:1219-1221`). So in the 3D engine a")
    A("keep-down link saves the lift, the travel and the descent, not the helix or the pecks. The keep-down")
    A("of this note is the 2D research's: one feed at the cut feed and the cut Z over the hop L, with no")
    A("re-entry, as the brief defines it.")
    A("")
    A("**The harness.** Cut A: 20 mm along +X at the cut feed, from rest. The link: the entry above, or the")
    A("keep-down feed, over L along +Y. Cut B: 20 mm along +X. The integrator times the whole path. Entry time")
    A("and keep-down time are the totals less cuts A and B timed alone from rest to rest; the penalty is the")
    A("difference of the two totals. Both corners are 90°. The floor is flat (no surface lift of the helix).")
    A("")
    A("**The column**, which sets the fed descent d_f:")
    A("")
    A("- open: the column is cut down to the cut Z; d_f = 0.5 mm;")
    A("- level: the floor is one depth per pass up (the level's own stock); d_f = depth per pass + 0.5 mm;")
    A("- fresh: the floor is the stock top; d_f = depth + 0.5 mm. At the first level, level = fresh.")
    A("")
    A("**The port is checked against the Phase 0 runs.** `model.py` over each Phase 0 G-code gives:")
    A("")
    A("| Op (ContourParallel) | Phase 0 runtime s | Port over the .nc, s | Difference |")
    A("|---|---|---|---|")
    for n, op in (("rivmap100", "rivmap100 3D Rough"), ("corne", "Corne 3D Rough 1 (no-acceleration basis)"), ("wanaka200", "wanaka200 2 Back Rough")):
        A(f"| {op} | {q(n + '|gc|phase0', '.1f')} | {q(n + '|gc|total', '.1f')} | {q(n + '|gc|err_pct', '+.2f')} % |")
    A("")
    A("The .nc rounds coordinates to 1 µm and carries the post-processed arcs, so a small difference is")
    A("expected. The port is the model.")
    A("")
    A("## 3. Machine and op settings (read only)")
    A("")
    A("| | rivmap100 | Corne | wanaka200 |")
    A("|---|---|---|---|")
    A("| File | `planning/fixtures/rivmap100/rivmap100.toml` | `planning/corne_case_analysis_2026-09-18/corne_job_corrected.toml` | `planning/multitool_2026-08-23/wanaka200_mt1.toml` |")
    A("| Machine | Shapeoko Pro XXL (:29) | Generic Wood Router (:29) | Shapeoko Pro XXL (:38) |")
    A("| `max_feed_mm_min` = rapid rate | 10000 (:30); high-feedrate mode off (:24) | 4000 (:30); off (:24) | 10000 (:39); off (:33) |")
    A("| Kinematics | accel X/Y/Z 500/500/270 mm/s², scalar 423.3, δ 0.02 mm, no per-axis rates, no jerk (:56-63) | none (:28-53) | as rivmap100 (:64-71) |")
    A("| Stock top / safe_z | top 12 after the Face (setup frame); safe = top + 5 | top 23 (:9, :12); safe 28 | top 25 (setup frame); safe 30 |")
    A("| Rough op | 3D Rough, adaptive3d (:241-276) | 3D Rough 1, adaptive3d (:112-147) | 2 Back Rough, adaptive3d (:318-354) |")
    A("| Cut feed / plunge feed mm/min | 2400 / 500 (:258-259) | 1518.75 / 750 (:129-130) | 1000 / 541 (:336-337) |")
    A("| Entry style | helix, factor 0.3, pitch 1.0 (:262-265); tool D 6 (:69) | plunge (:133) | plunge (:340) |")
    A("| Depth per pass | 2.0 (:256) | 6.0 (:127) | 5.46 (:333) |")
    A("| Levels (Phase 0, setup Z) | 10, 8, 6, 4, 2, 0.5 | 17, 11, 5, 0.5 | 19.54, 14.08, 8.62, 4.0 |")
    A("")
    A("The fixtures' kinematics carry no per-axis rates, so the model runs a Z rapid at 10000 mm/min. The")
    A("preset `shapeoko_xxl_ricky_tuned` (`kinematics.rs:376-384`) holds the operator's `$112` = 1000 mm/min.")
    A(f"With it, the rivmap100 penalty at 5 mm becomes {s('rivmap100|tuned|first|L5|pen')} s (first level) and "
      f"{s('rivmap100|tuned|level|L5|pen')} s (deepest level, level column), against {s('rivmap100|first|L5|pen')} and "
      f"{s('rivmap100|level|L5|pen')} s.")
    A("")
    A("## 4. One entry against one keep-down, by hop length")
    A("")
    A("First level, level column (the 2D research's case: a new arc enters the level's own stock).")
    A("Entry / keep-down / penalty, seconds. The arc-to-arc hops of C19 and D12 are 2–8 mm; lobe")
    A("changes are 6–69 mm.")
    A("")
    hops = sorted(set(ARC_HOPS + LOBE_HOPS + (0.0,)))
    A("| Fixture | " + " | ".join(f"L = {L:g} mm" for L in hops) + " |")
    A("|---|" + "---|" * len(hops))
    for n in NAMES:
        cells = [f"{s(f'{n}|first|L{L:g}|entry')} / {s(f'{n}|first|L{L:g}|keep')} / **{s(f'{n}|first|L{L:g}|pen')}**" for L in hops]
        A(f"| {LABEL[n]} | " + " | ".join(cells) + " |")
    A("")
    A("Keep-down at L = 0 is slightly negative: cut B then runs on from cut A with no stop. The keep-down")
    A("grows by L over the cut feed; the entry grows only by the rapid across. On a 1 mm grid, the first")
    A(f"level penalty reaches zero at L = {q('corne|first|cross_L', '.0f')} mm (Corne) and "
      f"{q('wanaka200|first|cross_L', '.0f')} mm (wanaka200). On rivmap100 it stays positive to 70 mm "
      f"(code value {q('rivmap100|first|cross_L', '.0f')}).")
    A("")
    A("The parts of the first level entry at L = 5 mm (s; the stop at the end of cut A sits in cut A):")
    A("")
    A("| Fixture | Lift | Travel | Descent | Entry style |")
    A("|---|---|---|---|---|")
    for n in NAMES:
        A(f"| {LABEL[n]} | {s(n + '|first|L5|part|lift')} | {s(n + '|first|L5|part|travel')} | "
          f"{s(n + '|first|L5|part|descent')} | {s(n + '|first|L5|part|entry')} |")
    A("")
    A("The penalty at L = 5 mm by column (first level open; deepest level open, level, fresh):")
    A("")
    A("| Fixture | First level, open | Deepest, open | Deepest, level | Deepest, fresh |")
    A("|---|---|---|---|---|")
    for n in NAMES:
        A(f"| {LABEL[n]} | {s(n + '|open1|L5|pen')} | {s(n + '|open|L5|pen')} | {s(n + '|level|L5|pen')} | {s(n + '|fresh|L5|pen')} |")
    A(f"| Corne, 200 mm/s² linker envelope | {s('corne|gen|first|L5|pen')} (first level, level) | — | {s('corne|gen|level|L5|pen')} | — |")
    A("")
    A("## 5. C19's break-even")
    A("")
    A(f"C19 (`P2_C19_march_arms.md`, plan `{c19['card']}`, read from `phase2/c19/results_c19.json` when this note")
    A("was written) gives the time per entry above which a plan with fewer entries beats (a) with the same links:")
    A("(time of the plan − time of (a)) / (entries of (a) − entries of the plan). Its times are length over a")
    A("1500 mm/min cut feed with no acceleration, so the break-even scales by 1500 / F for a fixture's cut feed F.")
    A("")
    A("| Plan | Entries (plan / (a)) | Break-even at 1500 | at rivmap100's 2400 | at Corne's 1518.75 | at wanaka200's 1000 |")
    A("|---|---|---|---|---|---|")
    for nm in ("star", "pocket"):
        for src, lab in (("e", "clipped (e)"), ("march", "clipped march")):
            A(f"| {nm}, {lab} | {q(f'c19|{nm}|{src}|entries', 'd')} / {q(f'c19|{nm}|a|entries', 'd')} | "
              f"{q(f'c19|{nm}|{src}|be', '.1f')} | {q(f'c19|{nm}|{src}|be|rivmap100', '.1f')} | "
              f"{q(f'c19|{nm}|{src}|be|corne', '.1f')} | {q(f'c19|{nm}|{src}|be|wanaka200', '.1f')} |")
    A("")
    A("The penalty to compare is the first level, level column, at a 2–8 mm hop: C19's plunges and")
    A("refusals re-enter the level's stock a few millimetres away. Which plan is faster (feed-rescaled")
    A("break-even):")
    A("")
    A(f"- rivmap100, penalty {s('rivmap100|first|L5|pen')} s: {verdict('rivmap100', 'rivmap100|first|L5|pen')}.")
    A(f"- Corne, penalty {s('corne|first|L5|pen')} s (no-acceleration basis): {verdict('corne', 'corne|first|L5|pen')}. "
      f"With the 200 mm/s² envelope, {s('corne|gen|first|L5|pen')} s: {verdict('corne', 'corne|gen|first|L5|pen')}.")
    A(f"- wanaka200, penalty {s('wanaka200|first|L5|pen')} s: {verdict('wanaka200', 'wanaka200|first|L5|pen')}.")
    A(f"- Into an open column no plan with fewer entries wins anywhere: the penalty is {s('rivmap100|open1|L5|pen')}, "
      f"{s('corne|open1|L5|pen')} and {s('wanaka200|open1|L5|pen')} s.")
    A("")
    A("What decides it is the entry style and its fed descent, not the machine's rapids. The rivmap100 helix")
    A("feeds 2π·1.8 = 11.3 mm of path per mm of descent at 500 mm/min; a plunge feeds 1 mm per mm. On a")
    A("helix machine the plans with fewer entries pay; on a plunge machine (a) stays faster.")
    A("")
    A("## 6. Cross-check with Phase 0")
    A("")
    A(f"The 36–40 % of PLAN.md comes from `planning/adaptive3d_step_ladder_roughing_2026-09-24/RESULTS.md:288-304`")
    A(f"(rivmap100, By Area, `rough-score`): entry {q('ladder|entry_s', '.0f')} s of {q('ladder|total_s', '.0f')} s "
      f"({q('ladder|pct', '.0f')} %) for the base arm. That run carries no entry count. Phase 0 (2026-10-23,")
    A(f"ContourParallel) has both: entry {q('rivmap100|p0|entry_s', '.1f')} s of {q('rivmap100|p0|runtime', '.1f')} s "
      f"({q('rivmap100|p0|pct', '.0f')} %), {q('rivmap100|x|entries', 'd')} top-level entries, "
      f"so {q('rivmap100|p0|per_entry', '.1f')} s per entry.")
    A("")
    A(f"With the flat-floor level column (helix {s('rivmap100|x|level_style')} s, plus a lift and a descent for each entry, an upper bound), "
      f"{q('rivmap100|x|entries', 'd')} entries give {q('rivmap100|lvl_model_pct', '.0f')} % of the Phase 0 runtime: "
      "the order of the 36–40 %, not the Phase 0 72 %.")
    A("The G-code says why: its moves at the plunge feed go down "
      f"{q('rivmap100|x|d_f', '.1f')} mm per entry on average (the sum of the downward Z, an upper bound,")
    A("because the shipped helix lifts on the terrain and goes up and down), not 2.5 mm. The model with")
    A("the Phase 0 counts and that mean descent:")
    A("")
    A("| Fixture | Lifts | Keep-down entries | Mean hop mm | Mean fed descent mm | Model, one retract entry s | Model, one keep-down entry style s | Model total s | G-code rapid + plunge-feed s | Model / G-code | Model % of runtime | G-code % of runtime |")
    A("|---|---|---|---|---|---|---|---|---|---|---|---|")
    for n in NAMES:
        A(f"| {LABEL[n]} | {q(n + '|gc|lifts', 'd')} | {q(n + '|x|keep', 'd')} | {q(n + '|x|hop', '.1f')} | {q(n + '|x|d_f', '.2f')} | "
          f"{s(n + '|x|retract_entry')} | {s(n + '|x|style')} | {q(n + '|x|model', '.0f')} | {q(n + '|x|gcode', '.0f')} | "
          f"{s(n + '|x|ratio')} | {q(n + '|x|model_pct', '.0f')} | {q(n + '|x|gcode_pct', '.0f')} |")
    A("")
    A(f"The G-code has {q('rivmap100|gc|lifts', 'd')} lifts to the retract plane on rivmap100; Phase 0 counts 104 retracts; this note does not resolve the difference of one.")
    A("")
    A("The per-entry model reproduces the G-code's entry time to within about 10 % on all three, and the")
    A("rivmap100 share (about 70 %) matches Phase 0's entry_s share. The check is not independent: the mean")
    A("hop and the mean descent come from the same G-code. It shows that the per-entry cost is the right")
    A("size when the descent is right, and that the descent, not the count, sets the share. On wanaka200")
    A(f"the rapids take {q('wanaka200|gc|rapid_s', '.0f')} s: {q('wanaka200|gc|lifts', 'd')} lifts with a mean "
      f"hop of {q('wanaka200|x|hop', '.0f')} mm.")
    A(f"On rivmap100 the moves at the plunge feed run {q('rivmap100|gc|path_per_down', '.1f')} mm of path per mm of downward Z, "
      "against 11.3 for the flat helix.")
    A("")
    A("## 7. Limits")
    A("")
    A("- C19's times have no acceleration. With the model's ramps, the longer engaged path of (e) and")
    A("  of the march would cost more, so the true break-even is higher than the table's. The rivmap100")
    A("  verdict holds only where the penalty is well above the break-even.")
    A("- A flat floor: no surface lift of the helix. The shipped rivmap100 helix lifts on the terrain")
    A("  and makes more turns (section 6), so the flat value is a lower bound for a terrain entry.")
    A("- Both corners of the harness are 90°. A keep-down that runs straight on is faster by up to the")
    A("  two stops; at δ = 0.02 mm a 90° corner is nearly a stop, so the difference is small.")
    A("- The feeds are the op's nominal feeds. The feed modulation pass (the .nc shows F4721 cuts on")
    A("  rivmap100) changes the cut feed, not the plunge feed.")
    A("- Corne has no kinematics: its no-acceleration numbers are what its simulation reports, and they")
    A("  read optimistic (`session/cycle_time.rs:57-61`).")
    A("- The per-axis rate (`$112`) is not in the fixture files; section 3 gives its effect on rivmap100.")
    A("")
    open(os.path.join(RES, "P2_C21_entry_cost.md"), "w").write("\n".join(md))
    json.dump(quotes, open(os.path.join(HERE, "quotes_c21.json"), "w"), indent=1)
    return quotes


if __name__ == "__main__":
    write(json.load(open(os.path.join(HERE, "results_c21.json"))), load_c19())
