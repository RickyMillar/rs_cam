"""C38 D22 analysis, step 1: load the C38a run's simulation.json once and keep
every sample as compact arrays in samples.npz (read only; about 6 GB peak)."""
import json
from pathlib import Path
import numpy as np
HERE = Path(__file__).resolve().parent
import sys
RUN = sys.argv[1] if len(sys.argv) > 1 else "c38a"
SRC = {"c38a": HERE.parent / "corne_c38a" / "simulation.json", "d20": HERE.parent.parent / "fix7" / "corne" / "geodesic_local_zigzag_pick" / "simulation.json"}[RUN]
d = json.loads(SRC.read_text())
s = d["trace"]["samples"]
intents = sorted({x.get("source_intent") or "" for x in s})
kin = sorted({x.get("cut_kinematics") or "" for x in s})
np.savez_compressed(
    HERE / f"samples_{RUN}.npz",
    tp=np.array([x["toolpath_id"] for x in s], np.int16),
    mv=np.array([x["move_index"] for x in s], np.int32),
    pos=np.array([x["position"] for x in s], np.float32),
    cut=np.array([x["is_cutting"] for x in s], bool),
    radial=np.array([x["engagement"]["radial_woc_fraction"] for x in s], np.float32),
    axial=np.array([x["axial_engagement_mm"] for x in s], np.float32),
    removed=np.array([x["removed_volume_est_mm3"] for x in s], np.float32),
    feed=np.array([x["feed_rate_mm_min"] for x in s], np.float32),
    intent=np.array([intents.index(x.get("source_intent") or "") for x in s], np.int8),
    kin=np.array([kin.index(x.get("cut_kinematics") or "") for x in s], np.int8),
    intents=np.array(intents), kins=np.array(kin),
)
print(len(s), intents, kin)
