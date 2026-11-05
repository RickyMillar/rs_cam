"""Print the engine answer per matrix cell. Usage: python3 -I show_cells.py [prefix] [--warn]"""
import json, sys
D = json.load(open("cells.json"))
pre = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("--") else ""
warn = "--warn" in sys.argv
for k, v in D.items():
    if k.startswith("_") or not k.startswith(pre):
        continue
    c = v["cell"]
    if "error" in v:
        print(k, "ERR", v["error"][-200:]); continue
    r = v["rec"]
    if "suggested" not in r:
        print(k, "NO-SUGGEST keys", list(r.keys()), json.dumps(r)[:400]); continue
    s = r["suggested"]; calc = r["profile"]["calculator"]; env = r["profile"]["envelope"] or {}
    prop = (env.get("proposal") or {}).get("value") or {}
    b = env.get("proposal_binding"); b = b.get("kind") if isinstance(b, dict) else b
    d = c["d"]; ap_p = prop.get("ap_mm"); ae_p = prop.get("ae_mm")
    print(f"{k:34} chip {calc['chip_load_mm'] and round(calc['chip_load_mm'],4)} rpm {s['spindle_rpm']} feed {s['feed_rate']} plunge {s.get('plunge_rate')} "
          f"| stated ap {c['ap']:.2f} ae {c['ae']:.2f} | prop ap {ap_p and round(ap_p,3)} ({ap_p and round(ap_p/d,2)}D) ae {ae_p and round(ae_p,3)} ({ae_p and round(ae_p/d,2)}D) bind {b} "
          f"| P {calc['power_kw'] and round(calc['power_kw'],3)}/{calc['available_power_kw']} lim {calc['power_limited']} | src {calc['vendor_source']}")
    if warn:
        for w in calc.get("warnings") or []:
            print("     W:", str(w)[:300])
        op = env.get("operator"); print("     operator:", json.dumps(op)[:300])
