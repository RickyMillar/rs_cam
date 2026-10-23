import sys, os, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, stream as st, plan as pn
from page_c17 import c15_curves, load
nm = sys.argv[1]
sh = st.Shape(nm)
c15 = c15_curves()
res, cur = load()
for name, levels in (("a", c15[(nm, "a")]), ("e", c15[(nm, "e")]),
                     ("march", [[np.vstack([lp["P"].astype(float), lp["P"][:1].astype(float)])] for lp in cur[(nm, "h_step")]])):
    t = time.time()
    pl = pn.Planner(sh)
    for lv in levels:
        pl.add_level(lv)
    m = pn.summarise(pl, sh)
    print(name, {k: (round(v, 3) if isinstance(v, float) else v) for k, v in m.items()}, round(time.time() - t, 1), flush=True)
