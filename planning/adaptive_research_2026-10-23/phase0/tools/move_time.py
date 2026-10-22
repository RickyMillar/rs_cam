"""Estimate the cycle time at each move of one op from the exported G-code.

Time per fed move = length / F (the last F word); time per G0 move =
length / rapid (argument, mm/min). The estimate ignores acceleration, so the
script scales it with the cut-trace times it is given (move_index,
cumulative_time_s pairs from get_cut_trace issues) by a least-squares
ratio through the origin, after the time offset of the op start.

Usage: python3 move_time.py NC OP MOVES TRACE.json T1 [T2 ...] [--rapid 5000]
       TRACE '-' : no trace; use --scale (default 1.016, the Corne AgentSearch fit) and --t0.
Prints the move index at each target time (seconds from the program start).
"""
import argparse
import json
import math
import re

from links_entries import section

WORD = re.compile(r"([GXYZIJF])(-?\d+(?:\.\d+)?)")


def times(cmds, rapid):
    pos = [0.0, 0.0, 0.0]
    g = 0
    f = 1000.0
    t = 0.0
    out = []
    for s in cmds:
        w = dict((a, float(b)) for a, b in WORD.findall(s))
        g = int(w.get("G", g))
        f = w.get("F", f)
        end = [w.get("X", pos[0]), w.get("Y", pos[1]), w.get("Z", pos[2])]
        if g in (2, 3):
            cx, cy = pos[0] + w.get("I", 0.0), pos[1] + w.get("J", 0.0)
            r = math.hypot(pos[0] - cx, pos[1] - cy)
            a0 = math.atan2(pos[1] - cy, pos[0] - cx)
            a1 = math.atan2(end[1] - cy, end[0] - cx)
            da = (a1 - a0) % (2 * math.pi) if g == 3 else (a0 - a1) % (2 * math.pi)
            if da < 1e-9:
                da = 2 * math.pi
            length = math.hypot(r * da, end[2] - pos[2])
        else:
            length = math.dist(pos, end)
        t += length / (rapid if g == 0 else f) * 60.0
        out.append(t)
        pos = end
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("nc")
    ap.add_argument("op")
    ap.add_argument("moves", type=int)
    ap.add_argument("trace")
    ap.add_argument("targets", type=float, nargs="+")
    ap.add_argument("--rapid", type=float, default=5000.0)
    ap.add_argument("--scale", type=float, default=1.016, help="used when TRACE is '-' (Corne AgentSearch fit)")
    ap.add_argument("--t0", type=float, default=0.0)
    a = ap.parse_args()
    cmds = section(open(a.nc).read().splitlines(), a.op)
    est = times(cmds, a.rapid)[1:1 + a.moves]
    if a.trace == "-":
        k, t0 = a.scale, a.t0
        for tt in a.targets:
            m = min(range(len(est)), key=lambda i: abs(t0 + k * est[i] - tt))
            print(f"t={tt:.0f} s -> move {m} (scale {k}, no trace)")
        print(f"op end estimate {t0 + k * est[-1]:.0f} s")
        return
    d = json.load(open(a.trace))
    if isinstance(d, list):
        d = json.loads(d[0]["text"])
    pairs = [(i["move_index"], i["cumulative_time_s"]) for i in d["issues"] if i["move_index"] < len(est)]
    t0 = min(t - est[m] for m, t in pairs[:5])
    num = sum(est[m] * (t - t0) for m, t in pairs)
    den = sum(est[m] ** 2 for m, _ in pairs)
    k = num / den
    err = max(abs(t0 + k * est[m] - t) for m, t in pairs)
    print(f"op start t0 {t0:.1f} s, scale {k:.3f}, max abs error on {len(pairs)} trace points {err:.1f} s, "
          f"op end estimate {t0 + k * est[-1]:.0f} s")
    for tt in a.targets:
        m = min(range(len(est)), key=lambda i: abs(t0 + k * est[i] - tt))
        print(f"t={tt:.0f} s -> move {m}")


if __name__ == "__main__":
    main()
