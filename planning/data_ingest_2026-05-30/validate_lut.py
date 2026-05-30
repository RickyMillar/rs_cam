#!/usr/bin/env python3
"""Validate staged LUT JSON against the live VendorObservation schema.

Usage: python3 validate_lut.py [staging_dir]
Default staging_dir is the script's own directory.

Canonical source of this script is Appendix A of
planning/feeds_data_ingest_2026-05-30_phased_plan.md.
Update the REQUIRED/VENDORS/MFAM sets if VendorObservation in
crates/rs_cam_core/src/feeds/vendor_lut.rs changes.

2026-05-30: Vendor::Helical added (Phase 1D); aluminum/plastic
material_family entries already valid in the live enum.
"""
import json
import glob
import sys
import os
from collections import Counter

REQUIRED = ["observation_id", "source_id", "source_vendor", "source_title",
            "source_url", "accessed_on", "evidence_grade", "row_kind",
            "tool_family", "operation_family", "pass_role", "material_family",
            "material_label", "diameter_mm", "flute_count"]
VENDORS = {"amana", "onsrud", "harvey", "whiteside", "sandvik", "garr",
           "autodesk", "carbide3d", "helical"}  # post-Phase 1D
GRADES = {"a", "b", "c"}
KINDS = {"exact", "derived", "fallback"}
TFAM = {"flat_end", "ball_nose", "tapered_ball_nose", "bull_nose",
        "chamfer_vbit", "facing_bit"}
OFAM = {"adaptive", "pocket", "contour", "parallel", "scallop", "trace", "face"}
ROLE = {"roughing", "semi_finish", "finish"}
MFAM = {"softwood", "hardwood", "plywood_softwood", "plywood_hardwood",
        "mdf", "hdf", "particleboard",
        "acrylic", "hdpe", "polycarbonate", "delrin", "aluminum"}
WOOD = {"softwood", "hardwood", "plywood_softwood", "plywood_hardwood",
        "mdf", "hdf", "particleboard"}


def main(staging_dir):
    ids = set()
    problems = []
    usable_now = []
    staged_reasons = []
    for f in sorted(glob.glob(os.path.join(staging_dir, "*.json"))):
        try:
            d = json.load(open(f))
        except Exception as e:
            problems.append((os.path.basename(f), "<file>", [f"PARSE: {e}"]))
            continue
        for o in d.get("observations", []):
            oid = o.get("observation_id", "<noid>")
            miss = [k for k in REQUIRED if k not in o]
            errs = []
            if miss:
                errs.append("MISSING:" + ",".join(miss))
            if o.get("source_vendor") not in VENDORS:
                errs.append(f"vendor={o.get('source_vendor')!r} not in enum")
            if o.get("evidence_grade") not in GRADES:
                errs.append("grade invalid")
            if o.get("row_kind") not in KINDS:
                errs.append("kind invalid")
            if o.get("tool_family") not in TFAM:
                errs.append("tool_family invalid")
            if o.get("operation_family") not in OFAM:
                errs.append("operation_family invalid")
            if o.get("pass_role") not in ROLE:
                errs.append("pass_role invalid")
            if o.get("material_family") not in MFAM:
                errs.append("material_family invalid")
            if oid in ids:
                errs.append("DUPLICATE_ID")
            ids.add(oid)
            cmin = o.get("chipload_min_mm_tooth")
            cmax = o.get("chipload_max_mm_tooth")
            if cmin is not None and cmax is not None and cmax < cmin:
                errs.append("chipload max<min")
            if cmin is not None and cmin <= 0:
                errs.append("chipload min<=0")
            if cmax is not None and cmax > 0.5:
                errs.append(f"chipload max suspicious ({cmax})")
            if errs:
                problems.append((os.path.basename(f), oid, errs))
            if not miss and o.get("material_family") in WOOD and cmax is not None:
                usable_now.append(oid)
            elif miss:
                staged_reasons.append("missing-field")
            elif cmax is None:
                staged_reasons.append("no-chipload")
            else:
                staged_reasons.append("non-wood")
    print(f"TOTAL rows: {len(ids)}")
    print(f"\n=== PROBLEMS ({len(problems)}) ===")
    for f, oid, e in problems:
        print(f"  [{f}] {oid}: {'; '.join(e)}")
    print(f"\n=== USABLE NOW (wood + schema-complete + chipload): {len(usable_now)} ===")
    for x in usable_now:
        print("  +", x)
    print(f"\n=== NOT-LIVE (staged) reason counts ===")
    print(" ", Counter(staged_reasons))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.dirname(os.path.abspath(__file__)))
