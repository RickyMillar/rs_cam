"""C13: print the note's tables from final.json (python3 -I report.py)."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import CD, CELL, w_front  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
F = json.load(open(os.path.join(HERE, "final.json")))


def rng(xs):
    return f"{min(xs):.3f}-{max(xs):.3f}"


def accepts(lat):
    """Phases (of 4) at which the lattice refuses no lead-in step."""
    return sum(1 for n in lat["lead_over"] if n == 0)


def band_table(rows, title):
    print(f"\n## {title}")
    print("| Level | ρ | Δ' | Arm | Arc b | Spare | Exact max (piece, wall) | Pass | Lattice cell: max (accepts) | Lattice step: max (accepts) |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    by = {}
    for r in rows:
        by.setdefault((r["level"], r["arm"]), []).append(r)
    for (lvl, arm), rs in sorted(by.items()):
        worst = max(rs, key=lambda r: r.get("lead", 9.9))
        plus = [r for r in rs if r["sgn"] > 0][0]
        ok = all(r["fits"] and r["lead"] <= CD + 1e-6 for r in rs)
        s = (f"| {lvl} | {plus['rho']:.2f} | {plus['delta']:.3f} | {arm} | {plus['L_b']:.2f} | "
             f"{min(r['spare'] for r in rs):.2f} | {worst['lead']:.4f} ({worst['piece']}, {worst['wall']:.2f}) | "
             f"{'pass' if ok else 'FAIL'} |")
        if "lattice" in plus:
            lc, ls = plus["lattice"]["cell"], plus["lattice"]["step"]
            s += f" {rng(lc['lead'])} ({accepts(lc)}/4) | {rng(ls['lead'])} ({accepts(ls)}/4) |"
        else:
            s += " - | - |"
        print(s)


print(f"cD {F['CD']:.6f}  C10 cD {F['CD_C10']:.4f}  cell {CELL}")
print("\n## Law check (C10 tilt, closed-form exact width, no wall)")
print("| Level | Δ' | m | max width | at bite | over cD by |")
print("|---|---|---|---|---|---|")
for lvl, (w, b) in sorted(F["law_check"].items(), key=lambda t: int(t[0])):
    d = F["deltas"][lvl]
    rho = {9: 19.06, 10: 21.05, 11: 23.05, 12: 25.05, 13: 27.05, 14: 29.05, 15: 31.05}[int(lvl)]
    print(f"| {lvl} | {d:.3f} | {CD - w_front(rho, d):.4f} | {w:.5f} | {b:.3f} | {w - CD:+.5f} |")

band_table(F["A"], "A: C10 law at C10's spacing")
band_table(F["B"], "B: C13 law at C10's spacing")
band_table(F["R"], "R: C13 law at the accepted spacing")

print("\n## Accepted spacing")
print("| Level | ρ | cap (law step) | C10 Δ' | accepted Δ | rule Δ | m at accepted | touch l_c arm 0 |")
print("|---|---|---|---|---|---|---|---|")
for lvl in sorted(F["accepted"], key=int):
    rho = [r for r in F["R"] if r["level"] == int(lvl)][0]["rho"]
    a0 = [r for r in F["R"] if r["level"] == int(lvl) and r["arm"] == 0 and r["sgn"] > 0][0]
    c10 = F["deltas"].get(lvl)
    print(f"| {lvl} | {rho:.2f} | - | {c10:.3f} | {F['accepted'][lvl]:.4f} | {F['rule'][lvl]:.4f} | "
          f"{CD - w_front(rho, F['accepted'][lvl]):.4f} | {a0['touch']:.2f} |")

print("\n## Wall arrival (plain pass, exact) and the lattice reading of it")
for name in ("A", "R"):
    pe = [max(r["plain_F"], r["plain_E"]) for r in F[name] if r["fits"]]
    lp = [x for r in F[name] if "lattice" in r for x in r["lattice"]["cell"]["plain"]]
    ls = [x for r in F[name] if "lattice" in r for x in r["lattice"]["step"]["plain"]]
    print(f"{name}: exact plain arrival {rng(pe)}; lattice plain max cell {rng(lp)}, step {rng(ls)}")

print("\n## Sequence (cost)")
seq = F["seq"]
print("radii", " ".join(f"{x:.3f}" for x in seq), "levels after 8:", len(seq) - 1)
K = F["K"]
fails = [r for r in K if not (r["fits"] and r["lead"] <= CD + 1e-6)]
print("K bands", len(K), "fails", len(fails), [(r["level"], r["arm"], r["sgn"], round(r.get("lead", -1), 4)) for r in fails])
