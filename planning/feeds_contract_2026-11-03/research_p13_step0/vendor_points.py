"""A (light): the shipped force, power and deflection laws at printed full-slot vendor points.

Run: python3 -I vendor_points.py   Writes vendor_points.json.
Assumptions the sources do not print are named per point ("assumed").
Stickout = flute + 8 mm with flute = max(3 D, ap) (the P11c sentry fixture rule,
crates/rs_cam_core/tests/the_pocket_width_has_no_computed_limit_below_d_p11c.rs:56).
Helix 30 deg = default_helix_deg (compute/tool_config.rs:733).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shipped_laws as L  # noqa: E402

F = "planning/extrapolation_2026-09-24/fetch/"
IN = 25.4
POINTS = [
    # id, source path:line, quote, D mm, Z, material line, ap mm, feed mm/min, rpm, assumed
    ("amana_6mm_hw", "crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt:1-3",
     "18,000 RPM / Depth of Cut: 1 x Tool Diameter (row chip 0.127 mm, D 6, 2F)",
     6.0, 2, "GenericHardwood", 6.0, 18000 * 2 * 0.127, 18000, "width not printed: a full slot assumed"),
    ("harvey_quarter_hard", F + "G10/sources/metal_harvey_sf_809500.txt:20,36",
     "Harder Woods Slot - Rough .250: .0106 IPT, Radial 1 x Dia, Axial 1 x Dia; RPM 10,000-20,000",
     0.25 * IN, 2, "GenericHardwood", 0.25 * IN, 10000 * 2 * 0.0106 * IN, 10000,
     "flute count not printed (2 assumed); rpm at the low end 10,000"),
    ("harvey_quarter_hard_20k", F + "G10/sources/metal_harvey_sf_809500.txt:20,36",
     "same row at the high end 20,000 RPM",
     0.25 * IN, 2, "GenericHardwood", 0.25 * IN, 20000 * 2 * 0.0106 * IN, 20000,
     "flute count not printed (2 assumed)"),
    ("sienci_quarter_hw", F + "G10/sources/sienci_feeds_speeds_metric.txt:98,170",
     "Hardwood: 1/4\" Flat End Mill UC 2740 mm/min, Stepdown - Slotting 2.5, 12000",
     0.25 * IN, 2, "GenericHardwood", 2.5, 2740.0, 12000, "flute count not printed (2 assumed)"),
    ("sienci_quarter_sw", F + "G10/sources/sienci_feeds_speeds_metric.txt:8,79",
     "Softwood/MDF: 1/4\" Flat End Mill UC 2740, Stepdown - Slotting 5.4, 12000",
     0.25 * IN, 2, "GenericSoftwood", 5.4, 2740.0, 12000, "flute count not printed (2 assumed)"),
    ("sienci_quarter_mdf", F + "G10/sources/sienci_feeds_speeds_metric.txt:8,79",
     "same row read with the MDF line", 0.25 * IN, 2, "mdf", 5.4, 2740.0, 12000,
     "flute count not printed (2 assumed)"),
    ("c3d_s3_mdf", F + "G10/sources/carbide3d_s3_feeds_250.txt:66",
     "MDF .3\" 17000 ... 80 (3 flute .25\", 100% engagement: G9/sources/community_2402_post1_announcement.txt:5)",
     0.25 * IN, 3, "mdf", 0.3 * IN, 80 * IN, 17000, "none"),
    ("c3d_s3_pine", F + "G10/sources/carbide3d_s3_feeds_250.txt:63",
     "Pine .4\" 21000 ... 75 (3 flute, 100% engagement)",
     0.25 * IN, 3, "GenericSoftwood", 0.4 * IN, 75 * IN, 21000, "generic softwood line for pine"),
    ("c3d_s3_mahogany", F + "G10/sources/carbide3d_s3_feeds_250.txt:64",
     "Mahogany .1\" 18950 ... 65 (3 flute, 100% engagement)",
     0.25 * IN, 3, "GenericHardwood", 0.1 * IN, 65 * IN, 18950, "generic hardwood line for mahogany"),
]


def main():
    out = []
    for pid, src, quote, d, z, mat, ap, feed, rpm, assumed in POINTS:
        ks, fe = L.force_line(mat)
        fz = feed / (rpm * z)
        mean, peak = L.edge_force(ap, d, d, z, fz, ks, fe)
        pw = L.power_kw(ap, d, d, z, rpm, feed, ks, fe)
        flute = max(3 * d, ap)
        stick = flute + 8.0
        c_tool = L.tip_compliance_mm_per_n(d, d, stick, flute, ap)
        room = L.DEFLECTION_TARGET_MM - peak * c_tool
        out.append({
            "id": pid, "source": src, "quote": quote, "d": d, "z": z, "material_line": mat, "ap": ap,
            "ap_over_d": ap / d, "feed": feed, "rpm": rpm, "fz": fz, "mrr_mm3_min": d * ap * feed,
            "mean_tangential_n": mean, "peak_resultant_n": peak, "power_kw": pw,
            "tool_deflection_mm": peak * c_tool, "stickout": stick,
            "implied_k_n_per_mm_at_200um": peak / room if room > 0 else None,
            "inside_generic_0p8kw": pw <= 0.8, "inside_makita_0p71kw": pw <= 0.71,
            "inside_tool_200um": peak * c_tool <= L.DEFLECTION_TARGET_MM, "assumed": assumed,
        })
    json.dump(out, open(os.path.join(HERE, "vendor_points.json"), "w"), indent=1)
    for o in out:
        print(o["id"], f"ap/D {o['ap_over_d']:.2f} fz {o['fz']:.4f} mean {o['mean_tangential_n']:.1f} peak {o['peak_resultant_n']:.1f} "
              f"P {o['power_kw']:.3f} defl {o['tool_deflection_mm']*1000:.1f}um k {o['implied_k_n_per_mm_at_200um']:.0f} MRR {o['mrr_mm3_min']:.0f}")


if __name__ == "__main__":
    main()
