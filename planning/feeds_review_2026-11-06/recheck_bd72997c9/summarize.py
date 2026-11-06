"""Summarise `suggest --json` output: one row per toolpath.

Usage: python3 -I summarize.py <file.jsonl> [...]
Prints a table and writes <file>.summary.json beside each input.
The peak chip is fz x sin(psi) when ae < D/2 (Garr, CREDITS.md:384-391), with fz the
shipped feed / (rpm x Z) and psi from the shipped stepover and the nominal D.
"""
import json
import math
import sys


def peak_factor(ae, d):
    if ae is None or d is None or ae >= d / 2:
        return 1.0
    c = 1.0 - 2.0 * ae / d
    return math.sqrt(1.0 - c * c)


def row(rec):
    p = rec["profile"]
    c = p["calculator"]
    s = rec["suggested"]
    t = rec["tool"]
    env = p.get("envelope") or {}
    prop = (env.get("proposal") or {}).get("value") or {}
    z = t.get("flute_count")
    fz = s["feed_rate"] / (s["spindle_rpm"] * z) if s.get("feed_rate") and s.get("spindle_rpm") and z else None
    pf = peak_factor(s.get("stepover"), t.get("diameter"))
    return dict(
        name=rec["toolpath_name"], tool=t["tool_type"], d=t["diameter"], z=z,
        cur=rec["current"], sug=s,
        chip=c.get("chip_load_mm"), bounds=c.get("chipload_bounds_mm"), point=c.get("chipload_point_mm"),
        src=c.get("vendor_source"), chip_src=str(c.get("chipload_source"))[:120],
        rpm_calc=c.get("rpm"), ap_calc=c.get("axial_depth_mm"), ae_calc=c.get("radial_width_mm"),
        power=c.get("power_kw"), avail=c.get("available_power_kw"), power_limited=c.get("power_limited"),
        warnings=c.get("warnings"),
        fz=fz, peak_chip=(fz * pf if fz else None), peak_factor=pf,
        env_prop=prop, env_binding=env.get("proposal_binding"), env_operator=env.get("operator"),
        support=(p.get("basis") or {}).get("support", {}).get("headline"),
        shipped_power=p.get("shipped_power"),
    )


def main():
    for path in sys.argv[1:]:
        rows = [row(json.loads(l)) for l in open(path) if l.strip()]
        with open(path + ".summary.json", "w") as f:
            json.dump(rows, f, indent=1, default=str)
        print("==", path)
        for r in rows:
            s = r["sug"]
            print(f"{r['name'][:34]:34} {r['tool'][:10]:10} D{r['d']} Z{r['z']} | rpm {s.get('spindle_rpm')} feed {s.get('feed_rate')} "
                  f"ae {s.get('stepover')} ap {s.get('depth_per_pass')} plunge {s.get('plunge_rate')} | chip {r['chip']} "
                  f"fz {r['fz'] and round(r['fz'],4)} peak {r['peak_chip'] and round(r['peak_chip'],4)} | src {r['src']} | "
                  f"P {r['power'] and round(r['power'],3)}/{r['avail']} | prop {r['env_prop']} bind {r['env_binding'] and r['env_binding'].get('kind') if isinstance(r['env_binding'],dict) else r['env_binding']}")
            print("    warn:", [w if isinstance(w, str) else list(w)[0] if isinstance(w, dict) else w for w in (r['warnings'] or [])][:8],
                  "| cur:", {k: r['cur'].get(k) for k in ('feed_rate', 'spindle_rpm', 'stepover', 'depth_per_pass')})


if __name__ == "__main__":
    main()
