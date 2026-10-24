# C21: the cost of one entry on each fixture's machine, from rs_cam's own
# cycle-time model (ported in model.py). Writes results_c21.json.
# Run: python3 -I run_c21.py
import json
import math
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gcode as G  # noqa: E402
import model as M  # noqa: E402

RES = os.path.dirname(os.path.dirname(HERE))  # adaptive_research_2026-10-23
GC = os.path.join(RES, "phase0", "gcode")
C19 = os.path.join(RES, "phase2", "c19")

# Machine and op settings, each from the fixture file (line numbers in the
# note). Kinematics None = the profile has no [job.machine.kinematics].
SHAPEOKO = dict(accel=423.3333333333333, axes=[500.0, 500.0, 270.0], rates=None, jd=0.02)
FIX = {
    "rivmap100": dict(
        kin=SHAPEOKO, max_feed=10000.0, rapid=10000.0,
        feed=2400.0, plunge=500.0, ramp=None, style="helix", dpp=2.0,
        helix_r=0.3 * 6.0, helix_pitch=1.0,
        # Phase 0 levels, setup frame: Z 10, 8, 6, 4, 2, 0.5 under a top at 12
        # (the Face took the stock to Z 12; step-ladder RESULTS.md).
        depths=[2.0, 4.0, 6.0, 8.0, 10.0, 11.5],
        gcode=("rivmap100_contour_parallel", "3D Rough"), phase0_runtime=3262.8,
    ),
    "corne": dict(
        kin=None, max_feed=4000.0, rapid=4000.0,
        feed=1518.75, plunge=750.0, ramp=None, style="plunge", dpp=6.0,
        helix_r=None, helix_pitch=None,
        depths=[6.0, 12.0, 18.0, 22.5],  # Z 17, 11, 5, 0.5 under a top at 23
        gcode=("corne_contour_parallel", "3D Rough 1"), phase0_runtime=973.2,
    ),
    "wanaka200": dict(
        kin=SHAPEOKO, max_feed=10000.0, rapid=10000.0,
        feed=1000.0, plunge=541.0, ramp=None, style="plunge", dpp=5.46,
        helix_r=None, helix_pitch=None,
        depths=[5.46, 10.92, 16.38, 21.0],  # Z 19.54, 14.08, 8.62, 4.0 under 25
        gcode=("wanaka200_contour_parallel", "2 Back Rough"), phase0_runtime=5776.8,
    ),
}
# The generation-time link envelope of a profile with no kinematics:
# `effective_kinematics` falls back to `generic_wood_router` (200 mm/s²,
# machine/mod.rs:679-682, kinematics.rs:325-330).
GENERIC = dict(accel=200.0, axes=None, rates=None, jd=0.010)
# The preset with the per-axis rates of the operator's own `$$`
# (kinematics.rs:376-384). The fixture files do not carry the rates.
TUNED = dict(accel=(500.0 + 500.0 + 270.0) / 3.0, axes=[500.0, 500.0, 270.0],
             rates=[10000.0, 10000.0, 1000.0], jd=0.020)

SAFE_Z_CLEARANCE_MM = 5.0  # compute/config.rs:162
HOPS = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0, 60.0, 69.0]
CUT_MM = 20.0


def kin_of(d):
    return None if d is None else M.Kin(d["accel"], d["axes"], d["rates"], d["jd"])


def op_of(f):
    return dict(style=f["style"], dpp=f["dpp"], stock_top=0.0, plunge=f["plunge"],
                ramp=f["ramp"] if f["ramp"] is not None else f["plunge"],
                helix_r=f["helix_r"], helix_pitch=f["helix_pitch"])


def seq_time(f, kin, L, depth, column, mode):
    """Total time of cut A, the link, cut B. The stock top is Z 0, the cut
    is at -depth, safe_z = top + 5 (effective_safe_z, compute/config.rs:169).
    column: 'open' (floor at the cut: the column was cut), 'level' (floor
    one depth per pass up: the level's own stock), 'fresh' (floor at the
    stock top). mode: 'entry' | 'keep'."""
    zc = -depth
    safe = SAFE_Z_CLEARANCE_MM
    if isinstance(column, float):  # a fed descent of `column` mm
        floor = min(zc + column - 0.5, 0.0)
    else:
        floor = {"open": zc, "level": min(zc + f["dpp"], 0.0), "fresh": 0.0}[column]
    a0, a1 = (-CUT_MM, 0.0, zc), (0.0, 0.0, zc)
    e = (0.0, L, zc)
    b1 = (CUT_MM, L, zc)
    mv = [M.feed(a1, f["feed"], "cut")]
    if mode == "entry":
        mv += M.entry_by_retract(a1, e, safe, floor, op_of(f))
    elif L > 0:
        mv += M.keep_down(e, f["feed"])
    mv.append(M.feed(b1, f["feed"], "cut"))
    tot, bk = M.cycle_time(mv, kin, f["max_feed"], f["rapid"], a0)
    return tot, bk


def cut_rr(f, kin):
    t, _ = M.cycle_time([M.feed((CUT_MM, 0.0, 0.0), f["feed"], "cut")], kin, f["max_feed"], f["rapid"], (0.0, 0.0, 0.0))
    return t


def table(f, kin, depths, columns):
    base = 2 * cut_rr(f, kin)
    out = {}
    for depth in depths:
        for col in columns:
            rows = []
            for L in HOPS:
                te, bk = seq_time(f, kin, L, depth, col, "entry")
                tk, _ = seq_time(f, kin, L, depth, col, "keep")
                rows.append(dict(L=L, entry_s=te - base, keep_s=tk - base, penalty_s=te - tk,
                                 parts={k: v for k, v in bk.items() if k != "cut"}))
            out[f"{depth:g}|{col}"] = rows
    return out


def gcode_check(name, f):
    """Run the port over the Phase 0 G-code of the op and sum the time of
    the moves at the plunge feed."""
    fn, title = f["gcode"]
    seed, mv = G.section(os.path.join(GC, fn + ".nc"), title)
    pm = []
    tot, _ = M.cycle_time(mv, kin_of(f["kin"]), f["max_feed"], f["rapid"], seed, pm)
    plunge_t = plunge_path = plunge_down = 0.0
    prev = seed
    ups = []
    for k, (kind, tgt, fd, i, j, _) in enumerate(mv):
        if kind != "rapid" and abs(fd - f["plunge"]) < 1e-6:
            plunge_t += pm[k]
            plunge_path += M.move_length(prev, tgt, kind, i, j)
            plunge_down += max(prev[2] - tgt[2], 0.0)
        if kind == "rapid" and tgt[2] > prev[2] + 1e-6:
            ups.append(round(tgt[2], 3))
        prev = tgt
    # The retract plane: the Z most upward rapids end at.
    safe = statistics.mode(ups)
    lifts = sum(1 for z in ups if z == safe)
    travel = 0.0
    prev = seed
    for kind, tgt, *_ in mv:
        if kind == "rapid" and abs(prev[2] - safe) < 1e-6 and abs(tgt[2] - safe) < 1e-6:
            travel += math.hypot(tgt[0] - prev[0], tgt[1] - prev[1])
        prev = tgt
    rapid_t = sum(pm[k] for k, m in enumerate(mv) if m[0] == "rapid")
    return dict(moves=len(mv), total_s=tot, phase0_runtime_s=f["phase0_runtime"],
                plunge_feed_s=plunge_t, plunge_feed_path_mm=plunge_path,
                plunge_feed_down_mm=plunge_down, rapid_s=rapid_t, lifts=lifts, retract_z=safe, travel_mm=travel)


def crosscheck(f, g, p0):
    """Model the op's entry time from the Phase 0 counts: each lift is a
    retract entry over the mean rapid hop of the G-code; each keep-down
    entry pays its entry style only. Both into a column whose fed descent
    is the G-code's mean descent at the plunge feed per entry."""
    kin = kin_of(f["kin"])
    n_retract = g["lifts"]
    n_entries = sum(p0["entries"].values())
    n_keep = n_entries - p0["entries"].get("helix entry", 0) - p0["entries"].get("plunge entry", 0)
    hop = g["travel_mm"] / n_retract
    d_f = g["plunge_feed_down_mm"] / n_entries
    depth = f["depths"][-1]
    base = 2 * cut_rr(f, kin)
    te, bk = seq_time(f, kin, hop, depth, d_f, "entry")
    tl, bl = seq_time(f, kin, 0.0, depth, "level", "entry")
    style = bk.get("entry", 0.0)
    model = n_retract * (te - base) + n_keep * style
    return dict(n_retract=n_retract, n_keep=n_keep, n_entries=n_entries, hop_mm=hop,
                fed_descent_mm=d_f, retract_entry_s=te - base, style_s=style,
                level_style_s=bl.get("entry", 0.0), model_s=model,
                gcode_s=g["plunge_feed_s"] + g["rapid_s"], runtime_s=g["total_s"])


def phase0_counts(name):
    d = json.load(open(os.path.join(RES, "phase0", "data", f"{name}_contour_parallel.json")))
    o = d["ops"][0]
    return dict(op=o["name"], runtime_s=o["op_runtime_s"], entry_s=o["entry_s"],
                entries=o["entries"], entries_nested=o["entries_nested"], retracts=o["retracts"])


def compute():
    res = {"hops": HOPS}
    for name, f in FIX.items():
        kin = kin_of(f["kin"])
        r = dict(curves=table(f, kin, [f["depths"][0], f["depths"][-1]], ["open", "level", "fresh"]))
        if name == "corne":
            r["curves_generic_accel"] = table(f, kin_of(GENERIC), [f["depths"][0], f["depths"][-1]], ["open", "level", "fresh"])
        if name == "rivmap100":
            r["curves_tuned_rates"] = table(f, kin_of(TUNED), [f["depths"][0], f["depths"][-1]], ["open", "level", "fresh"])
        # The hop (1 mm grid) at which the first level, level column
        # penalty first reaches zero; -1 when it stays positive to 70 mm.
        cross = -1.0
        for L in range(0, 71):
            te, _ = seq_time(f, kin, float(L), f["depths"][0], "level", "entry")
            tk, _ = seq_time(f, kin, float(L), f["depths"][0], "level", "keep")
            if te - tk <= 0.0:
                cross = float(L)
                break
        r["cross_L"] = cross
        r["gcode"] = gcode_check(name, f)
        r["phase0"] = phase0_counts(name)
        r["cross"] = crosscheck(f, r["gcode"], r["phase0"])
        res[name] = r
    return res


def main():
    json.dump(compute(), open(os.path.join(HERE, "results_c21.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
