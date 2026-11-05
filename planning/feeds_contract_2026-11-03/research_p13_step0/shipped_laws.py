"""A Python port of the shipped force, power, deflection and feed laws at HEAD 36fd50c54.

Read-only research (feeds contract P13 step 0). Each law cites its Rust site.
The port is checked against the release CLI and the FM1 golden file in verify_port.py.
"""
import math

import numpy as np

# material/force_line.rs:210-228 (Curti 2021 Table 5 helix-0 quadratics, per kg/m3)
CURTI = {
    "up_ks": (-5e-6, 1e-3, 26e-3),
    "up_int": (-1e-7, 1e-5, 55e-4),
    "down_ks": (-3e-6, 5e-4, 56e-3),
    "down_int": (-3e-7, 4e-5, 47e-4),
}


def _qmax(q):
    a, b, c = q
    cands = [0.0, 180.0]  # CURTI_GRAIN_ANGLE_RANGE_DEG, force_line.rs:83
    if a < 0:
        v = -b / (2 * a)
        if 0 <= v <= 180:
            cands.append(v)
    return max(a * x * x + b * x + c for x in cands)


def curti_envelope():
    """force_line.rs:262-273: per term, max over grain angle, then the larger mode."""
    ks = max(_qmax(CURTI["up_ks"]), _qmax(CURTI["down_ks"]))
    it = max(_qmax(CURTI["up_int"]), _qmax(CURTI["down_int"]))
    return ks, it


FPL_MOISTURE = 1.12  # force_line.rs:98
# material/mod.rs:41-58: mean SG of the three FPL rows each generic species names
SG = {
    "GenericSoftwood": (0.40 + 0.36 + 0.32) / 3,
    "GenericHardwood": (0.64 + 0.54 + 0.63) / 3,
    "WhiteOak": 0.68,  # material/mod.rs:31
}
MDF_LINE = (31.44, 3.36)  # force_line.rs:90,94 (Goli 2018 Table 3)


def force_line(material):
    """(Ks N/mm2, F_edge N/mm). Plywood has no line (it refuses: MaterialUnvalidated)."""
    if material == "mdf":
        return MDF_LINE
    if material in SG:
        rho = SG[material] * 1000.0 * FPL_MOISTURE
        ks_n, int_n = curti_envelope()
        return ks_n * rho, int_n * rho
    return None


def psi_of(ae, d):
    """feeds/force.rs:27-33."""
    r = d / 2.0
    if ae <= 0 or r <= 0:
        return 0.0
    return math.acos(max(-1.0, min(1.0, 1.0 - ae / r)))


def lift(ae, d):
    """geometry.rs:55-62 (Garr peak-chip lift, no cap)."""
    p = psi_of(ae, d)
    if p <= 0 or p >= math.pi / 2:
        return 1.0
    return 1.0 / math.sin(p)


def edge_force(ap, ae, d, z, fz, ks, fe, helix_deg=30.0, nz=600, nt=180):
    """engagement.rs:229-257: mean (closed form) and peak resultant (numeric over the edge).

    dF = (Ks fz sin(phi) + F_edge) dz along (cos phi, sin phi), phi in [0, psi], K_r = 0.
    nt = EDGE_SAMPLES_PER_PITCH (engagement.rs:403); z is integrated with nz midpoints.
    """
    psi = psi_of(ae, d)
    r = d / 2.0
    mean = ap * z / (2 * math.pi) * (ks * fz * (1 - math.cos(psi)) + fe * psi)
    pitch = 2 * math.pi / z
    zz = (np.arange(nz) + 0.5) / nz * ap
    dz = ap / nz
    lag = zz * math.tan(math.radians(helix_deg)) / r  # tool/mod.rs:229-231
    best = 0.0
    for i in range(nt):
        th = pitch * i / nt
        fx = fy = 0.0
        for j in range(z):
            phi = np.mod(th + pitch * j - lag, 2 * math.pi)
            m = phi <= psi
            ph = phi[m]
            w = (ks * fz * np.sin(ph) + fe) * dz
            fx += float(np.sum(w * np.cos(ph)))
            fy += float(np.sum(w * np.sin(ph)))
        best = max(best, math.hypot(fx, fy))
    return mean, best


def power_kw(ap, ae, d, z, rpm, feed, ks, fe):
    """tool_load/power.rs:113-158 (two-term model, grain factor 1.0)."""
    psi = min(psi_of(ae, d), math.pi)
    shear = ks * ap * ae * feed / 60e6
    edge = fe * ap * math.pi * d * rpm * (z * psi / (2 * math.pi)) / 60e6
    return shear + edge


E_CARBIDE = 600_000.0  # compute/tool_config.rs:130
BEND_FRAC = 0.80  # feeds/predict.rs:255 ENDMILL_EQUIVALENT_DIAMETER_FRACTION


def tip_compliance_mm_per_n(d, shank, stickout, flute_len, ap):
    """tool/mod.rs:761-846: stepped cantilever, load at ap/2 above the tip, flat end mill."""
    L = stickout
    load = max(L - ap / 2.0, 0.0)
    if load <= 0:
        return 0.0
    shank_top = min(max(L - flute_len, 0.0), load)
    delta = slope = 0.0
    for (x0, x1, dia, n) in ((0.0, shank_top, shank, 32), (shank_top, load, BEND_FRAC * d, 64)):
        if x1 <= x0:
            continue
        i_mm4 = math.pi * dia ** 4 / 64.0
        dx = (x1 - x0) / n
        for k in range(n):
            x = x0 + (k + 0.5) * dx
            arm = load - x
            delta += arm * arm * dx / (E_CARBIDE * i_mm4)
            slope += arm * dx / (E_CARBIDE * i_mm4)
    return delta + slope * (L - load)


DEFLECTION_TARGET_MM = 0.200  # feeds/suggest/invariants.rs:53 DEFLECTION_BACKOFF_TARGET_UM (Unsourced)


def tip_deflection_mm(ap, ae, d, z, fz, ks, fe, shank, stickout, flute_len, helix_deg=30.0, k_frame=None):
    """feeds/predict.rs:615-631 (tool beam), plus an optional frame spring in series.

    k_frame (N/mm) is NOT in the shipped code; None reproduces the shipped beam.
    """
    _, peak = edge_force(ap, ae, d, z, fz, ks, fe, helix_deg)
    c = tip_compliance_mm_per_n(d, shank, stickout, flute_len, ap)
    if k_frame:
        c += 1.0 / k_frame
    return peak * c


def deflection_edge_mm(ae, d, z, fz, ks, fe, shank, stickout, flute_len, helix_deg=30.0, k_frame=None,
                       target=DEFLECTION_TARGET_MM):
    """feeds/cutter_constraints.rs:291-363: the shallowest ap where deflection meets the target.

    None = no root up to min(0.95 stickout, flute) ("no deflection limit below the flute").
    """
    lo, hi = stickout * 1e-4, min(stickout * 0.95, flute_len)
    f = lambda a: tip_deflection_mm(a, ae, d, z, fz, ks, fe, shank, stickout, flute_len, helix_deg, k_frame)
    if f(lo) > target:
        return 0.0
    if f(hi) <= target:
        return None
    # the shipped code scans 16 steps then bisects; the function is monotone in ap here
    for _ in range(40):
        mid = 0.5 * (lo + hi)
        if f(mid) <= target:
            lo = mid
        else:
            hi = mid
    return lo


def speeds(ae, d, z, rpm0, chip, cap, rpm_floor):
    """feeds/mod.rs Step 5 (raw feed = rpm chip Z lift, mod.rs:2669) and Step 7 arm L (mod.rs:3076-3104).

    Returns (rpm, feed). Arm L ships when both arms are inside the power curve; the power
    arm choice is not modelled here (power is checked as a separate limit).
    """
    feed = rpm0 * chip * z * lift(ae, d)
    if feed <= cap:
        return rpm0, feed
    per_rev = feed / rpm0
    rpm = max(math.floor(cap / per_rev), rpm_floor)
    return rpm, min(per_rev * rpm, cap)
