"""Step 7.4a feed diff: R (research 607e52730) against B (the product) per move.

Inputs: step74a_feeds/{R,B,S}/<k>.txt, one line per move:
index x y z feed cap. R comes from the research tree, B from this tree
(`write_the_per_move_feeds`), S from this tree at the first freeze (the
swept width alone in the turn reading, before the lead ruling). The
geometry of R and B is equal move for move (the pin test), so the rows
line up by index.

Each fed move where B and R differ is in one class:
- departure (B slower): B caps the whole run piece at the least factor of
  the departure windows on it; R fed each window by its own match. These
  are listed as runs of consecutive moves.
- faster (B over R): listed one by one, with B's cap. A move with no B cap
  lies on no planned turn and no departure window in B's replay; R lowered
  it by its position match.
The script asserts each count and range that the report states.
"""
import math
import os

H = os.path.dirname(os.path.abspath(__file__))
NAMES = ["island pocket, one level", "island pocket, two levels", "shelf, one level", "channel, two levels"]
EPS = 1e-6
# The fixture's tool radius (TOOL_RADIUS in the pin test): a departure
# window holds the steps within R of the plunge.
TOOL_RADIUS = 3.0


def load(arm, k):
    rows = []
    for line in open(os.path.join(H, "step74a_feeds", arm, f"{k}.txt")):
        f = line.split()
        feed = None if f[4] == "None" else float(f[4][5:-1])
        cap = None if f[5] == "None" else float(f[5][5:-1])
        rows.append((float(f[1]), float(f[2]), float(f[3]), feed, cap))
    return rows


def plunge_distance(rows, i):
    """XY distance from the start of move `i` to the last plunge before it
    (a fed move straight down)."""
    j = i - 1
    while j > 0 and not (rows[j][3] and rows[j][:2] == rows[j - 1][:2] and rows[j][2] < rows[j - 1][2]):
        j -= 1
    start = rows[i - 1]
    return math.hypot(start[0] - rows[j][0], start[1] - rows[j][1])


def fed_time(rows):
    return sum(math.dist(a[:3], b[:3]) / b[3] * 60.0 for a, b in zip(rows, rows[1:]) if b[3])


out = []
stats = {}
for k, name in enumerate(NAMES):
    r, b, s = load("R", k), load("B", k), load("S", k)
    assert len(r) == len(b) == len(s)
    fed = [i for i, m in enumerate(r) if m[3] is not None]
    assert all(b[i][3] is not None for i in fed)
    equal, slower, faster = 0, [], []
    for i in fed:
        rf, bf = r[i][3], b[i][3]
        if abs(rf - bf) <= EPS * rf:
            equal += 1
        elif bf < rf:
            slower.append(i)
        else:
            faster.append(i)
    runs = []
    for i in slower:
        if runs and runs[-1][-1] == i - 1:
            runs[-1].append(i)
        else:
            runs.append([i])
    ratios = [b[i][3] / r[i][3] for i in slower]
    st = {"fed": len(fed), "equal": equal, "slower": len(slower), "slower runs": len(runs), "faster": len(faster)}
    if ratios:
        st["slower B/R"] = (round(min(ratios), 3), round(max(ratios), 3))
    tr, tb, ts = fed_time(r), fed_time(b), fed_time(s)
    if tr:
        st["fed time B/R"] = round(tb / tr, 4)
        st["fed time S/R"] = round(ts / tr, 4)
    stats[name] = st
    out.append(f"== {name}: " + ", ".join(f"{key} {v}" for key, v in st.items()))
    out.append("  faster moves (one by one): index (x, y, z) R feed, B feed, B/R, B cap")
    for i in faster:
        out.append(f"    {i:5d} ({r[i][0]:8.3f},{r[i][1]:8.3f},{r[i][2]:6.3f}) R {r[i][3]:8.2f} B {b[i][3]:8.2f} B/R {b[i][3] / r[i][3]:6.3f} B cap {b[i][4]}")
    out.append("  slower departure runs (conservative): moves, start (x, y, z), mm from the plunge, B/R min..max")
    for run in runs:
        rr = [b[i][3] / r[i][3] for i in run]
        i = run[0]
        d = plunge_distance(r, i)
        # Every slower run starts in a departure window.
        assert d < TOOL_RADIUS, (name, run, d)
        out.append(f"    {run[0]:5d}-{run[-1]:5d} ({r[i][0]:8.3f},{r[i][1]:8.3f},{r[i][2]:6.3f}) {d:5.2f} mm B/R {min(rr):6.3f}..{max(rr):6.3f}")
    # Every faster move has no cap in B: no planned turn, no departure window.
    assert all(b[i][4] is None for i in faster), name

open(os.path.join(H, "step74a_feed_diff.txt"), "w").write("\n".join(out) + "\n")
for name, st in stats.items():
    print(name, st)
assert [stats[n]["faster"] for n in NAMES] == [2, 6, 3, 0]
assert [stats[n]["slower"] for n in NAMES] == [70, 135, 70, 0]
assert [stats[n].get("fed time B/R") for n in NAMES] == [1.0374, 1.0362, 1.035, None]
