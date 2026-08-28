#!/usr/bin/env python3
"""M1 — exposure of the U3 defect (engagement normalised by the SHANK envelope
radius instead of ``engagement_radius_mm(axial_doc)``) on a real wanaka trace.

READ-ONLY over a simulation trace JSON. No cargo, no Rust, no writes to the
trace. Streams the 7.7 GB file with a hand-rolled record splitter (the file is
serde_json *pretty* output, so every sample object has one field per line and
records are delimited by ``\\n      {\\n``).

Every counter names the wire field it came from; see M1_RESULTS.md.

Usage
-----
    python3 m1_exposure.py                       # full pass, 8 workers
    python3 m1_exposure.py --workers 4
    python3 m1_exposure.py --limit-records 200000  # smoke test (single worker)

Writes ``m1_counters.json`` next to itself.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys
import time
from multiprocessing import Pool

# ── input provenance ────────────────────────────────────────────────────────

TRACE = (
    "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/"
    "e0c6527c-583b-43f5-a2ca-38611f86b462/scratchpad/falsify_s3/simulation.json"
)

DELIM = b"\n      {\n"          # start of one sample object (indent 6)
REC_END = b"\n      }"          # end of one sample object (indent 6)

# ── tool geometry, replicated from the Rust ────────────────────────────────
#
# `crates/rs_cam_core/src/compute/simulate.rs:1055` passes `entry.tool.radius()`
# — the ENVELOPE radius — into the stamp kernel, and
# `dexel_stock/stamping.rs:1119` divides the measured perp extent by
# `2.0 * radius`.  That is U3.  The corrected denominator is
# `2.0 * engagement_radius_mm(axial_doc)`, i.e.
# `MillingCutter::width_at_height(depth)`  (`tool/mod.rs:372-388`).
#
# Cutter construction: `compute/cutter.rs::build_cutter`.
#   TaperedBallNose -> TaperedBallEndmill::new(diameter, taper_half_angle,
#                                              shaft_diameter, cutting_length)
#     `diameter` is the BALL diameter; `diameter()` reports the SHAFT, so
#     `radius() == shaft_diameter / 2`.
#   VBit           -> VBitEndmill::new(diameter, included_angle, cutting_length)
#   EndMill        -> FlatEndmill::new(diameter, cutting_length)
#
# Geometry values: planning/airrun_2026-08-19/wanaka200.toml [[tools]]
# Toolpath -> tool mapping: the per-toolpath JSON dumps in the trace directory
# (`tp_*.json`, key "tool").


class Flat:
    """`tool/flat.rs::width_at_height` -> `self.radius()` for every h."""

    kind = "flat"

    def __init__(self, diameter):
        self.env_r = diameter / 2.0

    def engagement_radius(self, h):
        return self.env_r


class VBit:
    """`tool/vbit.rs::width_at_height` -> min(h * tan(included/2), R)."""

    kind = "vbit"

    def __init__(self, diameter, included_angle_deg):
        self.env_r = diameter / 2.0
        self.tan_half = math.tan(math.radians(included_angle_deg / 2.0))

    def engagement_radius(self, h):
        if h <= 0.0:
            return 0.0
        return min(h * self.tan_half, self.env_r)


class TaperedBall:
    """`tool/tapered_ball.rs::width_at_height`.

    ball region (h <= h_contact):  sqrt(2*R_ball*h - h^2),  0 at h<=0,
                                   R_ball at h >= R_ball
    cone region:                   min((h - cone_offset) * tan(alpha), R_shaft)
    """

    kind = "tapered_ball"

    def __init__(self, ball_diameter, taper_half_angle_deg, shaft_diameter):
        self.r_ball = ball_diameter / 2.0
        self.r_shaft = shaft_diameter / 2.0
        self.env_r = shaft_diameter / 2.0  # diameter() reports the shaft
        a = math.radians(taper_half_angle_deg)
        self.sin_a, self.cos_a = math.sin(a), math.cos(a)
        self.tan_a = math.tan(a)
        self.h_contact = self.r_ball * (1.0 - self.sin_a)
        self.r_contact = self.r_ball * self.cos_a
        self.cone_offset = self.h_contact - self.r_contact / self.tan_a

    def engagement_radius(self, h):
        if h <= self.h_contact:
            if h <= 0.0:
                return 0.0
            if h >= self.r_ball:
                return self.r_ball
            return math.sqrt(max(2.0 * self.r_ball * h - h * h, 0.0))
        return min((h - self.cone_offset) * self.tan_a, self.r_shaft)


# wire toolpath id -> (display name, cutter)
TOOLS = {
    1: ("2 Back Rough (3D Rough)", Flat(6.0)),
    3: ("4 Rivers (Project Curve)", VBit(5.5, 20.0)),
    4: ("5 Lakes (Project Curve)", TaperedBall(2.0, 5.7, 6.0)),
    5: ("6 3D Rough front (3D Rough)", Flat(6.0)),
    8: ("7 3D Finish (3D Finish)", TaperedBall(3.0, 2.8, 6.0)),
    9: ("8 Pencil detail (Pencil Finish)", TaperedBall(1.0, 7.1, 6.0)),
}

CELL_MM = 0.3          # trace `resolution_mm` / `sample_step_mm`
AIR_THRESHOLD = 0.02   # chipload.rs:315, power.rs:192, deflection.rs:87,:219
LOW_THRESHOLD = 0.10   # simulation_cut.rs LowEngagement band

RWOC_BINS = 5000       # bin width 0.0002 over [0, 1]
FACTOR_BINS = 800      # log10 bins, 1.0 .. 1e4, 0.005 dex each
FACTOR_DEX_MAX = 4.0


def new_acc():
    return {
        # populations
        "n_all": 0,
        "n_cutting": 0,
        "n_noncut": 0,
        "t_all": 0.0,
        "t_cutting": 0.0,
        "t_noncut": 0.0,
        "vol_all": 0.0,
        "vol_cutting": 0.0,
        # (1) sub-threshold population, as the gates see it
        "sub_n": 0,
        "sub_t": 0.0,
        "sub_vol": 0.0,
        "sub_n_volgt0": 0,
        "sub_t_volgt0": 0.0,
        "sub_vol_volgt0": 0.0,
        # the ONLY sub-threshold population a denominator change can rescue:
        # a strictly positive but small radial reading. A hard 0.0 stays 0.0
        # under any scaling.
        "rwoc_zero_n": 0,
        "rwoc_zero_t": 0.0,
        "rwoc_zero_vol": 0.0,
        "sub_n_rwoc_gt0": 0,
        "sub_t_rwoc_gt0": 0.0,
        "sub_vol_rwoc_gt0": 0.0,
        # steady-state only (gates additionally drop in_transit_span samples)
        "sub_n_steady": 0,
        "sub_t_steady": 0.0,
        "sub_vol_steady": 0.0,
        "cut_n_steady": 0,
        "cut_t_steady": 0.0,
        # bands
        "low_n": 0,
        "low_t": 0.0,
        "ok_n": 0,
        "ok_t": 0.0,
        # arc availability (power / deflection additionally require an arc)
        "arc_n_ge002": 0,
        "arc_null_n_ge002": 0,
        "arc_n_sub002": 0,
        # non-cutting samples that still removed volume (sanity)
        "noncut_n_volgt0": 0,
        "noncut_vol": 0.0,
        # (4) corrected denominator
        "doc_le0_n": 0,          # corrected factor undefined (er == 0)
        "doc_le0_t": 0.0,
        "doc_le0_sub_n": 0,
        "doc_le0_rwoc_gt0_n": 0,  # er == 0 yet a positive radial reading
        "corr_sub_n": 0,         # corrected rwoc still < 0.02
        "corr_sub_t": 0.0,
        "flip_n": 0,             # rwoc < 0.02 AND corrected >= 0.02
        "flip_t": 0.0,
        "flip_vol": 0.0,
        "flip_arc_n": 0,
        "flip_n_steady": 0,
        "flip_t_steady": 0.0,
        "corr_clamped_n": 0,     # corrected would exceed 1.0
        "rwoc_at_1_n": 0,        # raw already clamped at 1.0 (irreversible)
        # resolution honesty: the corrected denominator vs the sim cell
        # (0.3 mm here). Where 2*er < one cell the corrected fraction cannot
        # be resolved by the grid at all.
        "er_lt_cell_n": 0,
        "er_lt_halfcell_n": 0,
        "er_ge_cell_n": 0,
        # time-weighted engagement means (the `average_engagement` headline):
        # raw over ALL cutting samples, and raw-vs-corrected over the subset
        # where the corrected denominator is defined (er > 0).
        "rwoc_t_sum": 0.0,
        "rwoc_t_sum_defined": 0.0,
        "corr_t_sum_defined": 0.0,
        "t_defined": 0.0,
        # histograms
        "hist_rwoc": [0] * RWOC_BINS,
        # raw rwoc restricted to the DEFINED population (er > 0), so raw and
        # corrected quantiles are taken over the same samples.
        "hist_rwoc_defined": [0] * RWOC_BINS,
        "hist_corr": [0] * RWOC_BINS,
        "hist_factor": [0] * (FACTOR_BINS + 1),
        # per-kinematics reconciliation + split
        "kin": {},
    }


def new_kin():
    return {"n": 0, "t": 0.0, "sub_n": 0, "sub_t": 0.0, "flip_n": 0}


def merge(a, b):
    for k, v in b.items():
        if k in ("hist_rwoc", "hist_rwoc_defined", "hist_corr", "hist_factor"):
            dst = a[k]
            for i, c in enumerate(v):
                if c:
                    dst[i] += c
        elif k == "kin":
            for kk, kv in v.items():
                d = a["kin"].setdefault(kk, new_kin())
                for f, fv in kv.items():
                    d[f] += fv
        else:
            a[k] += v


class Reader:
    """Buffered forward reader over a byte range of the trace."""

    def __init__(self, path, lo, hi, chunk=1 << 26):
        self.f = open(path, "rb")
        self.f.seek(lo)
        self.base = lo
        self.hi = hi
        self.chunk = chunk
        self.buf = b""
        self.cur = 0
        self.eof = False

    def fill(self):
        if self.eof:
            return False
        more = self.f.read(self.chunk)
        if not more:
            self.eof = True
            return False
        # drop the consumed prefix
        self.buf = self.buf[self.cur:] + more
        self.base += self.cur
        self.cur = 0
        return True


def parse_record(body, off=0):
    """Extract the eleven fields M1 needs from one pretty-printed sample.

    Ordered `bytes.find` with a position hint — no positional assumptions
    about line numbers, and a missing key raises rather than defaulting.
    """
    out = []
    for key, conv in FIELD_SPEC:
        i = body.find(key, off)
        if i < 0:
            raise ValueError("field %r not found in record" % key)
        j = i + len(key)
        e = body.find(b"\n", j)
        if e < 0:
            e = len(body)
        raw = body[j:e].rstrip(b",").strip()
        out.append(conv(raw))
        off = e
    return out


FIELD_SPEC = [
    (b'"toolpath_id": ', int),
    (b'"segment_time_s": ', float),
    (b'"is_cutting": ', lambda r: r == b"true"),
    (b'"cut_kinematics": ', lambda r: r.strip(b'"').decode()),
    (b'"axial_engagement_mm": ', float),
    (b'"plunge_descent_mm": ', float),
    (b'"arc_engagement_radians": ', lambda r: r != b"null"),
    (b'"radial_woc_fraction": ', float),
    (b'"removed_volume_est_mm3": ', float),
    (b'"in_transit_span": ', lambda r: r == b"true"),
]

LOG10 = math.log10


def worker(args):
    path, lo, hi, limit = args
    accs = {}
    rd = Reader(path, lo, hi)
    n = 0
    unknown_tp = {}
    while True:
        i = rd.buf.find(DELIM, rd.cur)
        if i < 0:
            if not rd.fill():
                break
            continue
        abs_d = rd.base + i
        if abs_d >= hi:
            break
        rs = i + len(DELIM)
        e = rd.buf.find(REC_END, rs)
        if e < 0:
            if not rd.fill():
                break
            continue
        body = rd.buf[rs:e]
        rd.cur = e
        n += 1
        if limit and n > limit:
            n -= 1
            break
        (
            tp,
            seg_t,
            is_cut,
            kin,
            axial_mm,
            plunge_mm,
            has_arc,
            rwoc,
            vol,
            transit,
        ) = parse_record(body)

        acc = accs.get(tp)
        if acc is None:
            acc = accs[tp] = new_acc()
        entry = TOOLS.get(tp)
        if entry is None:
            unknown_tp[tp] = unknown_tp.get(tp, 0) + 1
            cutter = None
        else:
            cutter = entry[1]

        vol_pos = vol if vol > 0.0 else 0.0
        acc["n_all"] += 1
        acc["t_all"] += seg_t
        acc["vol_all"] += vol_pos

        if not is_cut:
            acc["n_noncut"] += 1
            acc["t_noncut"] += seg_t
            if vol > 0.0:
                acc["noncut_n_volgt0"] += 1
                acc["noncut_vol"] += vol_pos
            continue

        k = acc["kin"].get(kin)
        if k is None:
            k = acc["kin"][kin] = new_kin()
        acc["n_cutting"] += 1
        acc["t_cutting"] += seg_t
        acc["vol_cutting"] += vol_pos
        acc["rwoc_t_sum"] += rwoc * seg_t
        k["n"] += 1
        k["t"] += seg_t

        b = int(rwoc * RWOC_BINS)
        if b < 0:
            b = 0
        elif b >= RWOC_BINS:
            b = RWOC_BINS - 1
        acc["hist_rwoc"][b] += 1
        if rwoc >= 1.0:
            acc["rwoc_at_1_n"] += 1
        if rwoc <= 0.0:
            acc["rwoc_zero_n"] += 1
            acc["rwoc_zero_t"] += seg_t
            acc["rwoc_zero_vol"] += vol_pos

        sub = rwoc < AIR_THRESHOLD
        if sub:
            acc["sub_n"] += 1
            acc["sub_t"] += seg_t
            acc["sub_vol"] += vol_pos
            k["sub_n"] += 1
            k["sub_t"] += seg_t
            if rwoc > 0.0:
                acc["sub_n_rwoc_gt0"] += 1
                acc["sub_t_rwoc_gt0"] += seg_t
                acc["sub_vol_rwoc_gt0"] += vol_pos
            if vol > 0.0:
                acc["sub_n_volgt0"] += 1
                acc["sub_t_volgt0"] += seg_t
                acc["sub_vol_volgt0"] += vol_pos
            if has_arc:
                acc["arc_n_sub002"] += 1
        elif rwoc < LOW_THRESHOLD:
            acc["low_n"] += 1
            acc["low_t"] += seg_t
        else:
            acc["ok_n"] += 1
            acc["ok_t"] += seg_t
        if not sub:
            if has_arc:
                acc["arc_n_ge002"] += 1
            else:
                acc["arc_null_n_ge002"] += 1

        if not transit:
            acc["cut_n_steady"] += 1
            acc["cut_t_steady"] += seg_t
            if sub:
                acc["sub_n_steady"] += 1
                acc["sub_t_steady"] += seg_t
                acc["sub_vol_steady"] += vol_pos

        # ── corrected denominator ──────────────────────────────────────
        # `measured_axial_mm` is split at dexel_stock/simulation.rs:1077 into
        # `axial_engagement_mm` (lateral/arc/helix) and `plunge_descent_mm`
        # (Z-only). Exactly one is non-zero, so the sum recovers the stamp's
        # own `max_penetration`.
        doc = axial_mm + plunge_mm
        er = cutter.engagement_radius(doc) if cutter is not None else 0.0
        if er <= 0.0:
            acc["doc_le0_n"] += 1
            acc["doc_le0_t"] += seg_t
            if sub:
                acc["doc_le0_sub_n"] += 1
            if rwoc > 0.0:
                acc["doc_le0_rwoc_gt0_n"] += 1
            continue
        if 2.0 * er < CELL_MM:
            acc["er_lt_cell_n"] += 1
            if 2.0 * er < 0.5 * CELL_MM:
                acc["er_lt_halfcell_n"] += 1
        else:
            acc["er_ge_cell_n"] += 1
        factor = cutter.env_r / er
        if factor < 1.0:
            factor = 1.0  # geometric invariant: er <= envelope
        fb = int(LOG10(factor) / FACTOR_DEX_MAX * FACTOR_BINS)
        if fb < 0:
            fb = 0
        elif fb > FACTOR_BINS:
            fb = FACTOR_BINS
        acc["hist_factor"][fb] += 1

        corr = rwoc * factor
        acc["t_defined"] += seg_t
        acc["hist_rwoc_defined"][b] += 1
        acc["rwoc_t_sum_defined"] += rwoc * seg_t
        acc["corr_t_sum_defined"] += min(corr, 1.0) * seg_t
        if corr > 1.0:
            acc["corr_clamped_n"] += 1
            corr = 1.0
        cb = int(corr * RWOC_BINS)
        if cb >= RWOC_BINS:
            cb = RWOC_BINS - 1
        acc["hist_corr"][cb] += 1
        if corr < AIR_THRESHOLD:
            acc["corr_sub_n"] += 1
            acc["corr_sub_t"] += seg_t
        elif sub:
            acc["flip_n"] += 1
            acc["flip_t"] += seg_t
            acc["flip_vol"] += vol_pos
            k["flip_n"] += 1
            if has_arc:
                acc["flip_arc_n"] += 1
            if not transit:
                acc["flip_n_steady"] += 1
                acc["flip_t_steady"] += seg_t
    rd.f.close()
    return accs, n, unknown_tp


# ── boundary discovery ─────────────────────────────────────────────────────


def find_span(path):
    """Byte range of the `samples` array's contents."""
    size = os.path.getsize(path)
    start = None
    with open(path, "rb") as f:
        off, tail = 0, b""
        while off < 3_000_000_000:
            b = f.read(1 << 26)
            if not b:
                break
            data = tail + b
            i = data.find(b'"samples"')
            if i >= 0:
                start = off - len(tail) + i
                break
            off += len(b)
            tail = data[-64:]
    if start is None:
        raise SystemExit("samples key not found")
    end = None
    pat = re.compile(rb'\n    "(provenance|drill_samples|drill_summaries)"')
    with open(path, "rb") as f:
        f.seek(max(0, size - 1_500_000_000))
        off, tail = max(0, size - 1_500_000_000), b""
        while True:
            b = f.read(1 << 26)
            if not b:
                break
            data = tail + b
            m = pat.search(data)
            if m:
                end = off - len(tail) + m.start()
                break
            off += len(b)
            tail = data[-64:]
    if end is None:
        end = size
    return start, end


# ── quantiles from the histograms ──────────────────────────────────────────


def quantiles(hist, qs):
    total = sum(hist)
    if total == 0:
        return {q: None for q in qs}
    out, run, qi = {}, 0, 0
    qs = sorted(qs)
    for i, c in enumerate(hist):
        run += c
        while qi < len(qs) and run >= qs[qi] * total:
            out[qs[qi]] = (i + 0.5) / RWOC_BINS
            qi += 1
        if qi >= len(qs):
            break
    for q in qs:
        out.setdefault(q, 1.0)
    return out


def factor_quantiles(hist, qs):
    total = sum(hist)
    if total == 0:
        return {q: None for q in qs}
    out, run, qi = {}, 0, 0
    qs = sorted(qs)
    for i, c in enumerate(hist):
        run += c
        while qi < len(qs) and run >= qs[qi] * total:
            out[qs[qi]] = 10.0 ** ((i + 0.5) / FACTOR_BINS * FACTOR_DEX_MAX)
            qi += 1
        if qi >= len(qs):
            break
    for q in qs:
        out.setdefault(q, None)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trace", default=TRACE)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--limit-records", type=int, default=0)
    ap.add_argument(
        "--out",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)),
                             "m1_counters.json"),
    )
    args = ap.parse_args()

    t0 = time.time()
    lo, hi = find_span(args.trace)
    sys.stderr.write("samples span: [%d, %d)  %.2f GB\n"
                     % (lo, hi, (hi - lo) / 1e9))

    if args.limit_records:
        accs, n, unknown = worker((args.trace, lo, hi, args.limit_records))
        total_records = n
    else:
        w = max(1, args.workers)
        step = (hi - lo) // w
        ranges = []
        for i in range(w):
            a = lo + i * step
            b = hi if i == w - 1 else lo + (i + 1) * step
            ranges.append((args.trace, a, b, 0))
        with Pool(w) as p:
            parts = p.map(worker, ranges)
        accs, total_records, unknown = {}, 0, {}
        for pa, pn, pu in parts:
            total_records += pn
            for tp, a in pa.items():
                if tp in accs:
                    merge(accs[tp], a)
                else:
                    accs[tp] = a
            for tp, c in pu.items():
                unknown[tp] = unknown.get(tp, 0) + c

    sys.stderr.write("parsed %d records in %.1f s\n"
                     % (total_records, time.time() - t0))
    if unknown:
        sys.stderr.write("UNKNOWN toolpath ids (no cutter mapping): %r\n"
                         % unknown)

    qs = [0.5, 0.9, 0.99]
    out = {
        "trace": args.trace,
        "samples_span": [lo, hi],
        "records_parsed": total_records,
        "air_threshold": AIR_THRESHOLD,
        "rwoc_bin_width": 1.0 / RWOC_BINS,
        "unknown_toolpath_ids": unknown,
        "per_toolpath": {},
    }
    for tp in sorted(accs):
        a = accs[tp]
        name, cutter = TOOLS.get(tp, ("?", None))
        row = {k: v for k, v in a.items()
               if k not in ("hist_rwoc", "hist_rwoc_defined", "hist_corr",
                            "hist_factor", "kin")}
        row["name"] = name
        row["cutter"] = cutter.kind if cutter else None
        row["envelope_radius_mm"] = cutter.env_r if cutter else None
        row["kin"] = a["kin"]
        row["rwoc_quantiles"] = quantiles(a["hist_rwoc"], qs)
        row["rwoc_quantiles_defined"] = quantiles(a["hist_rwoc_defined"], qs)
        row["corrected_rwoc_quantiles"] = quantiles(a["hist_corr"], qs)
        row["factor_quantiles"] = factor_quantiles(a["hist_factor"], qs)
        row["hist_rwoc_head"] = a["hist_rwoc"][:200]  # 0 .. 0.04
        top = sorted(
            ((c, i) for i, c in enumerate(a["hist_rwoc"]) if c),
            reverse=True,
        )[:15]
        row["hist_rwoc_top"] = [
            {"lo": i / RWOC_BINS, "n": c} for c, i in top
        ]
        out["per_toolpath"][str(tp)] = row

    with open(args.out, "w") as f:
        json.dump(out, f, indent=1)
    sys.stderr.write("wrote %s\n" % args.out)

    # short console digest
    for tp in sorted(accs):
        a = accs[tp]
        print(
            "tp %2d  cut=%9d  sub002=%9d (%5.1f%%)  subvol=%12.3f mm3  "
            "flip=%9d  air_t=%10.1f s"
            % (
                tp,
                a["n_cutting"],
                a["sub_n"],
                100.0 * a["sub_n"] / max(1, a["n_cutting"]),
                a["sub_vol"],
                a["flip_n"],
                a["sub_t"],
            )
        )


if __name__ == "__main__":
    main()
