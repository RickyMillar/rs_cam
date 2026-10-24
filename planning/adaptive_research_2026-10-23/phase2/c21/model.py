# C21: a Python port of rs_cam's cycle-time integrator and of the
# adaptive3d entry emitter, for the cost of one entry.
#
# Every formula cites the Rust line it ports (HEAD 6b194b59). Read-only on
# Rust: nothing here runs cargo.
#
#   K = crates/rs_cam_core/src/machine/kinematics.rs
#   P = crates/rs_cam_core/src/adaptive3d/path.rs
#   E = crates/rs_cam_core/src/dressup/entry_descent.rs
import math

# ── machine kinematics (K) ────────────────────────────────────────────


class Kin:
    """`MachineKinematics` (K:93-143). `axes` = acceleration_xyz_mm_s2,
    `rates` = max_rate_xyz_mm_min, `jd` = junction_deviation_mm. Jerk and
    the junction clamp are None in all three fixtures."""

    def __init__(self, accel, axes=None, rates=None, jd=0.010):
        self.accel = accel
        self.axes = axes
        self.rates = rates
        self.jd = jd

    def effective_accel(self, d):  # K:396-415
        scalar = max(self.accel, 1e-3)
        if self.axes is None:
            return scalar
        lim = math.inf
        for a, di in zip(self.axes, d):
            if abs(di) > 1e-9:
                lim = min(lim, max(a, 1e-3) / abs(di))
        return max(lim, 1e-3) if math.isfinite(lim) else scalar

    def governing_max_rate(self, d):  # K:435-448
        if self.rates is None:
            return None
        best = None
        for r, di in zip(self.rates, d):
            if abs(di) > 1e-9:
                lim = max(r, 1e-3) / abs(di)
                if best is None or lim < best:
                    best = lim
        return best


def cruise_ceiling(v_cmd, d, kin):  # K:724-740
    rate = kin.governing_max_rate(d)
    if rate is not None and rate / 60.0 < v_cmd:
        return rate / 60.0
    return v_cmd


def trapezoidal_time(length, v_in, v_out, v_cmd, accel):  # K:1582-1610
    if length <= 1e-9 or accel <= 1e-9:
        return 0.0
    v_in = min(max(v_in, 0.0), v_cmd)
    v_out = min(max(v_out, 0.0), v_cmd)
    d_acc = (v_cmd * v_cmd - v_in * v_in) / (2.0 * accel)
    d_dec = (v_cmd * v_cmd - v_out * v_out) / (2.0 * accel)
    if d_acc + d_dec <= length:
        t_cruise = (length - d_acc - d_dec) / v_cmd if v_cmd > 1e-9 else 0.0
        return (v_cmd - v_in) / accel + t_cruise + (v_cmd - v_out) / accel
    v_peak = min(math.sqrt(max(accel * length + 0.5 * (v_in * v_in + v_out * v_out), 0.0)), v_cmd)
    return max(v_peak - v_in, 0.0) / accel + max(v_peak - v_out, 0.0) / accel


def junction_velocity(d_in, d_out, v_in_cmd, v_out_cmd, accel, jd, rapid_adjacent):  # K:1538-1567
    if rapid_adjacent:
        return 0.0
    cap = min(v_in_cmd, v_out_cmd)
    dot = max(-1.0, min(1.0, sum(a * b for a, b in zip(d_in, d_out))))
    sin_half = math.sqrt(max(0.5 * (1.0 + dot), 0.0))
    if sin_half <= 1e-9:
        return 0.0
    if sin_half >= 1.0 - 1e-9:
        return cap
    r = jd * sin_half / (1.0 - sin_half)
    return min(math.sqrt(accel * r), cap)


# ── moves ─────────────────────────────────────────────────────────────
# A move is (kind, target, feed, i, j, tag). kind: "rapid" | "line" |
# "cw" | "ccw". (i, j) is the arc centre offset from the START point.


def arc_span(p0, p1, kind, i, j):  # K:1389-1418
    if kind not in ("cw", "ccw"):
        return None
    cx, cy = p0[0] + i, p0[1] + j
    r0 = (p0[0] - cx, p0[1] - cy)
    r1 = (p1[0] - cx, p1[1] - cy)
    rad, rad_end = math.hypot(*r0), math.hypot(*r1)
    if rad <= 1e-9 or rad_end <= 1e-9:
        return None
    a0, a1 = math.atan2(r0[1], r0[0]), math.atan2(r1[1], r1[0])
    raw = (a1 - a0) if kind == "ccw" else (a0 - a1)
    sweep = raw % (2 * math.pi)
    if sweep <= 1e-12:
        sweep = 2 * math.pi
    return rad, rad_end, r0, r1, sweep, kind == "ccw"


def move_length(p0, p1, kind, i, j):  # K:1424-1434
    dz = p1[2] - p0[2]
    a = arc_span(p0, p1, kind, i, j)
    if a is not None:
        return math.hypot(a[0] * a[4], dz)
    return math.dist(p0, p1)


def move_directions(p0, p1, kind, i, j):  # K:1441-1501
    dx, dy, dz = p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]
    chord = math.sqrt(dx * dx + dy * dy + dz * dz)
    cd = (dx / chord, dy / chord, dz / chord) if chord > 1e-9 else (0.0, 0.0, 0.0)
    a = arc_span(p0, p1, kind, i, j)
    if a is None:
        start, end = cd, cd
    else:
        rad, rad_end, r0, r1, sweep, ccw = a
        xy_len = rad * sweep

        def tangent(rx, ry, ln):
            tx, ty = ((-ry / ln, rx / ln) if ccw else (ry / ln, -rx / ln))
            v = (tx * xy_len, ty * xy_len, dz)
            n = max(math.sqrt(v[0] ** 2 + v[1] ** 2 + v[2] ** 2), 1e-12)
            return (v[0] / n, v[1] / n, v[2] / n)

        start, end = tangent(r0[0], r0[1], rad), tangent(r1[0], r1[1], rad_end)
    return (cd if chord > 1e-9 else start), start, end


def cycle_time(moves, kin, max_feed_mm_min, rapid_feed_mm_min, seed, per_move=None):
    """`compute_cycle_time_breakdown` (K:1177-1247) over `digest_moves`
    (K:1071-1118) and `walk_junctions` (K:1131-1171). `seed` is the
    position before the first move (move 0 of the Rust toolpath, which
    adds no segment). Returns (total_s, {tag: s}). When `per_move` is a
    list, it receives one time per input move (0 for a skipped move). With `kin=None` it is
    the no-acceleration basis: each move's length over its feed, rapids at
    the rapid rate (dexel_stock/simulation.rs:579 and :1032)."""
    max_feed = max(max_feed_mm_min / 60.0, 1e-6)  # K:1188
    rapid = max(rapid_feed_mm_min / 60.0, max_feed)  # K:1189
    digests = []
    prev = seed
    if per_move is not None:
        per_move[:] = [0.0] * len(moves)
    for idx, (kind, tgt, feed, i, j, tag) in enumerate(moves):
        length = move_length(prev, tgt, kind, i, j)
        if length > 1e-9:  # K:1087
            d, ds, de = move_directions(prev, tgt, kind, i, j)
            if kind == "rapid":
                v_cmd, is_rapid = rapid, True
            else:
                v_cmd, is_rapid = min(max(feed / 60.0, 1e-6), max_feed), False  # K:1098
            if kin is None:
                digests.append((length, v_cmd, tag, idx))
            else:
                acc = kin.effective_accel(d)
                ceil = cruise_ceiling(v_cmd, d, kin)
                digests.append((length, d, ds, de, v_cmd, ceil, acc, is_rapid, tag, idx))
        prev = tgt
    buckets = {}
    total = 0.0
    if kin is None:
        for length, v_cmd, tag, idx in digests:
            dt = length / v_cmd
            if per_move is not None:
                per_move[idx] = dt
            total += dt
            buckets[tag] = buckets.get(tag, 0.0) + dt
        return total, buckets
    v_in = 0.0
    n = len(digests)
    for k in range(n):
        length, d, ds, de, v_cmd, ceil, acc, is_rapid, tag, idx = digests[k]
        if k + 1 < n:
            nx = digests[k + 1]
            v_out = junction_velocity(de, nx[2], ceil, nx[5], min(acc, nx[6]), kin.jd, is_rapid or nx[7])
        else:
            v_out = 0.0
        dt = trapezoidal_time(length, v_in, v_out, cruise_ceiling(v_cmd, d, kin), kin.effective_accel(d))  # K:745-775
        if per_move is not None:
            per_move[idx] = dt
        total += dt
        buckets[tag] = buckets.get(tag, 0.0) + dt
        v_in = v_out
    return total, buckets


# ── the adaptive3d entry, as P:1514-1655 emits it ──────────────────────


def rapid(p, tag):
    return ("rapid", p, 0.0, 0.0, 0.0, tag)


def feed(p, f, tag):
    return ("line", p, f, 0.0, 0.0, tag)


def peck_plunge(entry, start_z, dpp, stock_top, plunge):  # P:164-195
    out = []
    PECK_CLEARANCE_MM = 0.5  # P:166
    ENTRY_CLEARANCE = 2.0  # E:182
    dpp = max(dpp, 0.1)
    ladder_start = min(max(entry[2], stock_top + ENTRY_CLEARANCE), start_z)
    if start_z - ladder_start > 0.1:
        out.append(rapid((entry[0], entry[1], ladder_start), "descent"))
    z = ladder_start
    while z - entry[2] > dpp + 1e-6:
        nz = z - dpp
        out.append(feed((entry[0], entry[1], nz), plunge, "entry"))
        out.append(rapid((entry[0], entry[1], nz + PECK_CLEARANCE_MM), "entry"))
        z = nz
    out.append(feed(entry, plunge, "entry"))
    return out


def helix(entry, start_z, radius, pitch, plunge, ramp_feed):
    """`emit_helix` (E:779-918) on a flat floor with the contact read at the
    rapid floor: `rapid_to_entry_top` (E:432-471) then adds no move, so the
    helix starts at `start_z`. No surface lift (flat floor), no floor lap
    (P:1509, `helix_floor_lap: false`)."""
    out = []
    dz = start_z - entry[2]
    if dz < 0.01:
        out.append(feed(entry, plunge, "entry"))
        return out
    STEPS = 36  # E:818
    step_a = 2 * math.pi / STEPS
    step_dz = pitch / STEPS
    z = start_z
    k = 0
    while True:
        k += 1
        x = entry[0] + radius * math.cos(step_a * k)
        y = entry[1] + radius * math.sin(step_a * k)
        z = max(z - step_dz, entry[2])
        out.append(feed((x, y, z), ramp_feed, "entry"))
        if z <= entry[2] + 1e-9:
            break
    out.append(feed(entry, ramp_feed, "entry"))  # E:917 return to centre
    return out


def entry_by_retract(cut_end, entry, safe_z, floor_z, op):
    """The retract arm of P:1581-1635: lift to safe_z (P:1460-1469), rapid
    across (P:1583), rapid down to the descent floor (P:1587-1592), then
    the entry style from there (P:1601-1634)."""
    out = [rapid((cut_end[0], cut_end[1], safe_z), "lift"),
           rapid((entry[0], entry[1], safe_z), "travel")]
    descent = min(floor_z + 0.5, safe_z)  # P:1333-1337, RAPID_DESCENT_BUFFER_MM P:1323
    descent = max(descent, entry[2])
    if descent < safe_z - 1e-6:
        out.append(rapid((entry[0], entry[1], descent), "descent"))
    if descent > entry[2] + 1e-6:
        if op["style"] == "plunge":
            out += peck_plunge(entry, descent, op["dpp"], op["stock_top"], op["plunge"])
        else:
            # A column whose floor is at the entry is air: E:444-470 feeds
            # straight down at the plunge feed and the helix adds nothing.
            if floor_z <= entry[2] + 1e-6:
                out.append(feed(entry, op["plunge"], "entry"))
            else:
                out += helix(entry, descent, op["helix_r"], op["helix_pitch"], op["plunge"], op["ramp"])
    return out


def keep_down(entry, f):
    """The keep-down alternative of the brief: one feed at the cut feed and
    at the cut depth over the same XY hop."""
    return [feed(entry, f, "keepdown")]
