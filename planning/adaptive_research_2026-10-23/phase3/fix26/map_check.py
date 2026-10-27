"""D35: which G-code line is move k of an op? For each candidate offset
(line k + off = move k), compare each move's program time at its own F
(c39_lib) with the trace samples' time of that move at the sample feed
times feed ratio... simpler: compare the program move's KIND (G0 / G1
plunge / other) with the samples' cut_kinematics of the same move index.
Prints the agreement per offset. Usage: map_check.py <run> <tp> <op>"""
import glob, importlib.util, json, math, sys
from pathlib import Path
P3 = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("c39", P3 / "fix26" / "c39_lib.py"); C = importlib.util.module_from_spec(spec); spec.loader.exec_module(C)
run, tp, op = sys.argv[1], int(sys.argv[2]), sys.argv[3]
d = P3 / run
n = json.load(open(glob.glob(str(d / f"tp_{tp}_*.json"))[0]))["move_count"]
allm = C.op_moves(str(d / "program.nc"), op)
kin = {}
for s in json.load(open(d / "simulation.json"))["trace"]["samples"]:
    if s["toolpath_id"] == tp: kin.setdefault(s["move_index"], s["cut_kinematics"])
def kind(m):
    if m[0] == 0: return "rapid"
    if math.hypot(m[2][0] - m[1][0], m[2][1] - m[1][1]) < 1e-3 and m[2][2] < m[1][2]: return "plunge"
    return "other"
def skind(k): return k if k in ("rapid", "plunge") else "other"
print(run, tp, "lines", len(allm), "moves", n)
for off in (-1, 0, 1):
    ok = tot = 0
    for k, kk in kin.items():
        j = k + off
        if 0 <= j < len(allm):
            tot += 1; ok += kind(allm[j]) == skind(kk)
    print(f"  line = move {off:+d}: agree {ok}/{tot}")
