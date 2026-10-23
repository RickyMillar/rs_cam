import sys, os, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stream as st, numpy as np
nm, mode, am = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
sh = st.Shape(nm); d = st.Direction(sh, st.harmonic_field(sh))
loops = st.march(sh, d, mode, air_move=am)
print(nm, mode, am, len(loops), "crossings", sum(st.self_crossings(lp["P"]) for lp in loops), "maxviol", max(lp["violation"] for lp in loops), "maxres", max(lp["residual"] for lp in loops))
flat = {}
for i, lp in enumerate(loops):
    flat[f"{nm}|{mode}|{i}|P"] = lp["P"]; flat[f"{nm}|{mode}|{i}|was"] = lp["was_stopped"]; flat[f"{nm}|{mode}|{i}|kind"] = lp["kind"]
np.savez(f"try2_{nm}_{int(am)}.npz", **flat)
