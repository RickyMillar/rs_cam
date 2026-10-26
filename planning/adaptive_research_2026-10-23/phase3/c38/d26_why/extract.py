"""C38 D26 analysis, step 1: compact sample arrays (with the segment time)
from a Corne run's simulation.json. Read only; about 6 GB peak."""
import json, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
P3 = H.parent.parent
RUN = sys.argv[1]
SRC = {"d26pick": P3 / "fix16" / "corne_pick" / "simulation.json",
       "c35": P3 / "fix12" / "corne" / "simulation.json",
       "c38a": H.parent / "corne_c38a" / "simulation.json",
       "d23pick": P3 / "fix13" / "corne_pick" / "simulation.json"}[RUN]
s = json.loads(SRC.read_text())["trace"]["samples"]
intents = sorted({x.get("source_intent") or "" for x in s})
kin = sorted({x.get("cut_kinematics") or "" for x in s})
np.savez_compressed(
    H / f"samples_{RUN}.npz",
    tp=np.array([x["toolpath_id"] for x in s], np.int16),
    mv=np.array([x["move_index"] for x in s], np.int32),
    pos=np.array([x["position"] for x in s], np.float32),
    cut=np.array([x["is_cutting"] for x in s], bool),
    radial=np.array([x["engagement"]["radial_woc_fraction"] for x in s], np.float32),
    axial=np.array([x["axial_engagement_mm"] for x in s], np.float32),
    removed=np.array([x["removed_volume_est_mm3"] for x in s], np.float32),
    feed=np.array([x["feed_rate_mm_min"] for x in s], np.float32),
    seg_t=np.array([x["segment_time_s"] for x in s], np.float32),
    intent=np.array([intents.index(x.get("source_intent") or "") for x in s], np.int8),
    kin=np.array([kin.index(x.get("cut_kinematics") or "") for x in s], np.int8),
    intents=np.array(intents), kins=np.array(kin),
)
print(RUN, len(s))
