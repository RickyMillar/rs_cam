"""Check the Python port (shipped_laws.py) against the release CLI output and the FM1 golden file.

Run: python3 -I verify_port.py   Writes port_check.json. Exits 1 if a check misses its tolerance.
"""
import csv
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shipped_laws as L  # noqa: E402

REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
cli = json.load(open(os.path.join(HERE, "cli_points.json")))
checks = []


def check(cid, what, port, ship, tol):
    ok = abs(port - ship) <= tol * max(1.0, abs(ship))
    checks.append({"cell": cid, "what": what, "port": port, "shipped": ship, "rel_tol": tol, "ok": ok})


def rpm0_of(c):
    for w in c["warnings"]:
        m = re.search(r"RpmLoweredForFeedCeiling \{ rpm_from: ([0-9.]+)", w)
        if m:
            return float(m.group(1))
    return c["rpm"]


MAT = {"GenericHardwood": "GenericHardwood", "GenericSoftwood": "GenericSoftwood", "mdf": "mdf"}
CAP = {"generic": 4000.0, "pro_xxl_vfd": 6000.0}  # machine/mod.rs:797,847-853 min(6000, travel)
for cid, rec in cli.items():
    i, c, sp = rec["inputs"], rec["calculator"], rec["shipped_power"]
    ks, fe = L.force_line(MAT[i["material"]])
    z = 2
    rpm, feed = L.speeds(i["ae"], i["d"], z, rpm0_of(c), c["chip_load_mm"], CAP[i["machine"]], 8000.0)
    check(cid, "rpm", rpm, c["rpm"], 1e-6)
    check(cid, "feed_mm_min", feed, c["feed_rate_mm_min"], 1e-6)
    # Pocket ships PowerCase::Slot (FirstPass::Slot): the power at ae = D, at the op's feed
    p = L.power_kw(i["ap"], i["d"], i["d"], z, c["rpm"], c["feed_rate_mm_min"], ks, fe)
    check(cid, "power_kw_slot_case", p, c["power_kw"], 1e-6)
    # the envelope deflection edge, per column, at the column's grid chip (speeds.points ap index 0)
    env = rec["envelope"]
    pts = {round(q["ae_mm"], 9): q for q in env["speeds"]["points"] if abs(q["ap_mm"] - env["speeds"]["aps"][0]) < 1e-12}
    for lim in env["boundary"]:
        if lim["kind"] == "deflection" and lim["edge"]["kind"] == "ap_curve":
            for ae, ap_edge in lim["edge"]["points"]:
                if ap_edge >= env["speeds"]["aps"][-1] - 1e-9:
                    continue
                fz = pts[round(ae, 9)]["chip"]
                port = L.deflection_edge_mm(max(ae, 1e-3), i["d"], z, fz, ks, fe, i["d"], i["stickout"], i["flute"])
                check(cid, f"deflection_edge_ae{ae:.2f}", port, ap_edge, 0.01)

# FM1 golden: EndMill D 6 hardwood Pocket force_n and power_kw
fm1 = os.path.join(REPO, "crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv")
for row in csv.DictReader(open(fm1)):
    if row["tool_type"] == "EndMill" and row["operation"] == "Pocket" and row["material"] in ("hardwood", "softwood", "mdf"):
        mat = {"hardwood": "GenericHardwood", "softwood": "GenericSoftwood", "mdf": "mdf"}[row["material"]]
        ks, fe = L.force_line(mat)
        d, ap, ae = float(row["diameter_mm"]), float(row["depth_per_pass_mm"]), float(row["stepover_mm"])
        rpm, feed = float(row["rpm"]), float(row["feed_mm_min"])
        fz = feed / (rpm * 2)
        _, peak = L.edge_force(ap, ae, d, 2, fz, ks, fe)
        cid = f"FM1 D{d:g} {row['material']}"
        check(cid, "force_n_peak", peak, float(row["force_n"]), 1e-3)
        check(cid, "power_kw_slot_case", L.power_kw(ap, d, d, 2, rpm, feed, ks, fe), float(row["power_kw"]), 2e-3)

json.dump(checks, open(os.path.join(HERE, "port_check.json"), "w"), indent=1)
bad = [c for c in checks if not c["ok"]]
print(f"port checks {len(checks)}, misses {len(bad)}")
for b in bad:
    print("MISS", b)
sys.exit(1 if bad else 0)
