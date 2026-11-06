"""Old (ec5cbe4a9, ../matrix/cells.json) against new (bd72997c9, cells.json), per cell."""
import json, sys
o = json.load(open("../matrix/cells.json")); n = json.load(open("cells.json"))
def row(x):
    if "rec" not in x: return "ERR " + x.get("error", "")[-80:]
    p = x["rec"]["profile"]; c = p["calculator"]; e = p.get("envelope") or {}
    pv = (e.get("proposal") or {}).get("value") or {}
    pr = {"ap_mm": pv.get("ap_mm"), "ae_mm": pv.get("ae_mm"), "binding": e.get("proposal_binding")}
    pr = {k: (round(v, 3) if isinstance(v, float) else v) for k, v in pr.items()}
    b = p.get("vendor_band") or {}
    mem = [(m.get("vendor"), round(m.get("chip_mid_mm", 0), 4)) for m in b.get("members", [])]
    return (f"rpm {c['rpm']:.0f} feed {c['feed_rate_mm_min']:.0f} chip {c['chip_load_mm']:.4f} ap {c['axial_depth_mm']:.3f} "
            f"ae {c['radial_width_mm']:.3f} P {c['power_kw']:.3f} row {c['vendor_source']} | prop ap {pr.get('ap_mm')} ae {pr.get('ae_mm')} "
            f"bind {pr.get('binding')} | band {mem} | warn {[w.split(' ')[0] for w in c['warnings']]}")
pre = sys.argv[1] if len(sys.argv) > 1 else ""
for k in n:
    if k.startswith("_") or not k.startswith(pre): continue
    print(k); print("  old", row(o[k]) if k in o else "-"); print("  new", row(n[k]))
