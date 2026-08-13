#!/usr/bin/env python3
"""A-8i — how many shipped LUT rows are narrower than the retarget headroom?

Checkpoint P (1b) asks the retargeter to share the gate's *comparison*, not
just its band. The obvious form of that is a hard guard: refuse (or clamp) a
target that `ChipBounds::contains` says is outside the band it came from. This
probe measures the population that guard would move, before writing it.

The question is scale-invariant, so it can be answered off the raw JSON:
`vendor_lookup` multiplies BOTH bounds by one `total_scale`
(`vendor_lookup.rs:551-552`) and `geometry::derate_chipload_bounds` multiplies
BOTH by one `doc_derating_scale` (`geometry.rs:250-253`). `max/min` is
therefore identical on the raw row, the matched row and the derated band.

With `chipload_high_headroom = chipload_low_headroom = 1.20`:

  * breakage target = max / 1.2 — below `min` when `max/min < 1.2`
  * burn     target = min * 1.2 — above `max` when `max/min < 1.2`

so a single ratio decides both sides.

    python3 narrow_band_census.py
"""

import glob
import json
import os

HEADROOM = 1.20
HERE = os.path.dirname(os.path.abspath(__file__))
OBS = os.path.normpath(
    os.path.join(HERE, "..", "..", "..", "..",
                 "crates/rs_cam_core/data/vendor_lut/observations/*.json")
)


def rows():
    for path in sorted(glob.glob(OBS)):
        doc = json.load(open(path))
        obs = doc if isinstance(doc, list) else doc.get("observations", doc)
        if isinstance(obs, dict):
            obs = list(obs.values())
        for o in obs:
            if isinstance(o, dict):
                yield os.path.basename(path), o


def main():
    both = []
    high_only = 0
    neither = 0
    for src, o in rows():
        mn = o.get("chipload_min_mm_tooth")
        mx = o.get("chipload_max_mm_tooth")
        if mn and mx:
            both.append((src, o.get("observation_id"), mn, mx, mx / mn))
        elif mx:
            high_only += 1
        else:
            neither += 1

    narrow = [r for r in both if r[4] < HEADROOM]
    point = [r for r in narrow if r[4] == 1.0]

    print(f"rows with both bounds      : {len(both)}")
    print(f"rows with max only         : {high_only}")
    print(f"rows with no chipload band : {neither}")
    print()
    print(f"max/min < {HEADROOM}            : {len(narrow)} "
          f"({100.0 * len(narrow) / len(both):.1f} % of two-sided rows)")
    print(f"  of which single-point    : {len(point)} (max == min)")
    print()
    print("On every one of those rows BOTH headroom targets fall outside the")
    print("band the gate judges by — the P-(1a) defect class, reached through")
    print("the headroom policy instead of the DOC derate.")
    print()
    by_src = {}
    for src, _oid, _mn, _mx, _r in narrow:
        by_src[src] = by_src.get(src, 0) + 1
    for src in sorted(by_src):
        print(f"  {by_src[src]:3d}  {src}")
    print()
    print("worst 12 by ratio:")
    for src, oid, mn, mx, r in sorted(narrow, key=lambda x: x[4])[:12]:
        print(f"  {r:6.4f}  {oid:<48} [{mn:.4f}, {mx:.4f}]  {src}")


if __name__ == "__main__":
    main()
