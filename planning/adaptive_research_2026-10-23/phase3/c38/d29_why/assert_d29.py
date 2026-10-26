"""C38 §15: assert the D29 numbers (run extract.py d29, strip_feed.py first).
Also reads the run's summary.json and the feed values per op."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
P3 = H.parent.parent
s = json.loads((H / "strip_feed.json").read_text())
assert s == {"strip_samples": 6939, "over_c": 1895, "over_c_full_feed": 1895, "over_bound": 1554, "over_bound_full_feed": 1554}, s
summ = json.loads((P3 / "fix20" / "a_only" / "corne" / "summary.json").read_text())
assert summ["adaptive_feed_modulation"] is True and summ["modulation_state"].startswith("on (ConstrainedMax")
S = np.load(H / "samples_d29.npz"); it = list(S["intents"])
vals = {}
for tp in (1, 3):
    m = (S["tp"] == tp) & S["cut"] & (S["intent"] == it.index("clearing_cut"))
    v, c = np.unique(np.round(S["feed"][m]), return_counts=True)
    vals[tp] = dict(zip(v.tolist(), c.tolist()))
print({k: sorted(v.items(), key=lambda t: -t[1])[:6] for k, v in vals.items()})
# op 1 clearing cuts carry only three feeds: no slot factor (0.17-1) of F 4000 survives
assert sorted(vals[1]) == [750.0, 1519.0, 4000.0], vals[1]
# the copy op (commanded 2429) runs clearing cuts at 3137 and 4000 too: the modulator raises feeds
assert vals[3].get(3137.0, 0) > 10000 and vals[3].get(4000.0, 0) > 5000, vals[3]
print("ASSERTS OK")
