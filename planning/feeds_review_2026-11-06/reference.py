"""Printed reference values for the feeds review (judgement before engine).

Run: python3 -I planning/feeds_review_2026-11-06/reference.py
Each FACT names a file, a 1-based line and the strings that must be on that line.
The script fails if a string is not on its line. It then derives the reviewer's
judgement from the facts only and writes judgement.json beside this file.

Unit rule: 1 in = 25.4 mm (the definition of the inch). No other constant is typed.
"""
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
IN = 25.4  # mm per inch, by definition

SIENCI = "planning/extrapolation_2026-09-24/fetch/G10/sources/sienci_feeds_speeds_metric.txt"
C3D = "planning/extrapolation_2026-09-24/fetch/G10/sources/carbide3d_s3_feeds_250.txt"
C3D_POST = "planning/extrapolation_2026-09-24/fetch/G9/sources/community_2402_post1_announcement.txt"
HARVEY = "planning/extrapolation_2026-09-24/fetch/G10/sources/metal_harvey_sf_809500.txt"
FREUD = "planning/extrapolation_2026-09-24/fetch/G2/sources/freud_router_bit_feed_and_speed_for_cnc_20170822.txt"
FUSION = "planning/extrapolation_2026-09-24/fetch/G10/sources/fusion_adaptive_roughing_reference.txt"
GARR = "planning/extrapolation_2026-09-24/fetch/G10/sources/metal_garr_technical.txt"
LUT = "crates/rs_cam_core/data/vendor_lut/sources/"
SPEKTRA = LUT + "amana_spektra_spiral_plunge_v24.txt"
BALL = LUT + "amana_ball_nose_v7.txt"
VGROOVE = LUT + "amana_ams159_vgroove_v2.txt"
SPETOOL = LUT + "spetool_2d3d_tapered.txt"
ONS_HW = LUT + "onsrud_hard_wood_cutting_data.txt"
ONS_SW = LUT + "onsrud_soft_wood_cutting_data.txt"
ONS_MDF = LUT + "onsrud_mdf_cutting_data.txt"
STIFF = "planning/feeds_contract_2026-11-03/research_sienci_stiffness/FINDINGS.md"

# id: (file, line, [strings that must be on the line], {printed values})
FACTS = {
    # Sienci metric chart. Columns: feed mm/min, plunge mm/min, stepover mm,
    # stepdown slotting mm, stepdown pocketing mm, rpm. The chart prints no flute count.
    "sienci_sw_quarter_full": (SIENCI, 79, ['1/4" Flat End Mill UC', "2740", "1370", "2.85", "5.4", "12.1", "12000"],
                               dict(feed=2740, plunge=1370, stepover=2.85, slot=5.4, pocket=12.1, rpm=12000, d_in=0.25)),
    "sienci_hw_quarter_full": (SIENCI, 170, ['1/4" Flat End Mill UC', "2740", "1370", "2.85", "2.5", "5.6", "12000"],
                               dict(feed=2740, plunge=1370, stepover=2.85, slot=2.5, pocket=5.6, rpm=12000, d_in=0.25)),
    "sienci_hw_quarter_regular": (SIENCI, 141, ['1/4" Flat End Mill UC', "2190", "1100", "2.85", "1.9", "4.2", "12000"],
                                  dict(feed=2190, plunge=1100, stepover=2.85, slot=1.9, pocket=4.2, rpm=12000, d_in=0.25)),
    "sienci_sw_eighth_full": (SIENCI, 83, ['1/8" Flat End Mill UC', "2700", "1350", "1.45", "5.5", "12.1", "17000"],
                              dict(feed=2700, plunge=1350, stepover=1.45, slot=5.5, pocket=12.1, rpm=17000, d_in=0.125)),
    "sienci_hw_eighth_full": (SIENCI, 174, ['1/8" Flat End Mill UC', "2700", "1350", "1.45", "2.6", "5.6", "17000"],
                              dict(feed=2700, plunge=1350, stepover=1.45, slot=2.6, pocket=5.6, rpm=17000, d_in=0.125)),
    "sienci_hw_sixteenth_full": (SIENCI, 177, ['1/16" Flat End Mill UC', "1350", "680", "0.70", "2.6", "5.8", "17000"],
                                 dict(feed=1350, plunge=680, stepover=0.70, slot=2.6, pocket=5.8, rpm=17000, d_in=0.0625)),
    "sienci_hw_surfacing_full": (SIENCI, 180, ["22mm Surfacing Bit", "2590", "1300", "8.80", "0.4", "1.0", "17000"],
                                 dict(feed=2590, plunge=1300, stepover=8.80, slot=0.4, pocket=1.0, rpm=17000, d_mm=22.0)),
    "sienci_sw_surfacing_full": (SIENCI, 89, ["22mm Surfacing Bit", "2590", "1300", "8.80", "0.8", "2.1", "17000"],
                                 dict(feed=2590, plunge=1300, stepover=8.80, slot=0.8, pocket=2.1, rpm=17000, d_mm=22.0)),
    "sienci_hw_ball_quarter_finish": (SIENCI, 111, ['1/4" Ball End Mill - Finishing', "2790", "1400", "0.30", "1.9", "25.4", "22000"],
                                      dict(feed=2790, plunge=1400, stepover=0.30, slot=1.9, pocket=25.4, rpm=22000, d_in=0.25)),
    "sienci_hw_tapered_coarse": (SIENCI, 106, ['1/4" - D1/16" Tapered Ball End Mill - Coarse', "3170", "1580", "0.24", "25.4", "28500"],
                                 dict(feed=3170, plunge=1580, stepover=0.24, pocket=25.4, rpm=28500)),
    "sienci_hw_vcarve": (SIENCI, 103, ["90, 60 Degree V-bit - V-Carving", "1730", "580", "1.80", "6.3", "14.1", "17000"],
                         dict(feed=1730, plunge=580, stepover=1.80, slot=6.3, pocket=14.1, rpm=17000)),
    # Carbide 3D S3 chart, .25 in 3 flute end mill at 100 % engagement (the post, line 5).
    # Columns: DOC in, RPM, dial, dial, FEED ipm, PLUNGE ipm.
    "c3d_tool": (C3D_POST, 5, ['3 flute .25" endmill, 100% engagement'], dict(flutes=3, d_in=0.25)),
    "c3d_pine": (C3D, 63, ["Pine", ".4", "21000", "75", "40"], dict(doc_in=0.4, rpm=21000, feed_ipm=75, plunge_ipm=40)),
    "c3d_mahogany": (C3D, 64, ["Mahogany", ".1", "18950", "65", "32"], dict(doc_in=0.1, rpm=18950, feed_ipm=65, plunge_ipm=32)),
    "c3d_mdf": (C3D, 66, ["MDF", ".3", "17000", "80", "30"], dict(doc_in=0.3, rpm=17000, feed_ipm=80, plunge_ipm=30)),
    # Amana Spektra v24: one Wood/Plywood column (header line 5), 18 000 rpm, 1 x D.
    "spektra_header": (SPEKTRA, 5, ["Wood/Plywood", "MDF/Laminate"], {}),
    "spektra_rpm": (SPEKTRA, 3, ["18,000 RPM", "1 x Tool Diameter"], dict(rpm=18000)),
    "spektra_2f_6mm": (SPEKTRA, 26, ["6mm", '180"', '.0050"', '90"', '215"', '.0060"'],
                       dict(wood_ipm=180, wood_chip_in=0.0050, wood_ramp_ipm=90, mdf_chip_in=0.0060, d_mm=6.0)),
    "spektra_2f_eighth": (SPEKTRA, 21, ['1/8"', '145"', '.0040"', '72.5"', '180"', '.0050"'],
                          dict(wood_ipm=145, wood_chip_in=0.0040, wood_ramp_ipm=72.5, mdf_chip_in=0.0050, d_in=0.125)),
    "spektra_2f_half": (SPEKTRA, 37, ['1/2"', '200"', '.0057"', '350"', '.0096"'],
                        dict(wood_ipm=200, wood_chip_in=0.0057, mdf_chip_in=0.0096, d_in=0.5)),
    "spektra_3f_quarter": (SPEKTRA, 47, ['1/4"', '270"', '.0050"', '325"', '.0060"'],
                           dict(wood_ipm=270, wood_chip_in=0.0050, mdf_chip_in=0.0060, d_in=0.25)),
    "spektra_2f_sixteenth": (SPEKTRA, 12, ['1/16"', '70"', '.0020"', '35"'], dict(wood_chip_in=0.0020, wood_ramp_ipm=35, d_in=0.0625)),
    "spektra_2f_3_32": (SPEKTRA, 17, ['3/32"', '80"', '.0023"', '40"'], dict(wood_chip_in=0.0023, wood_ramp_ipm=40, d_in=3 / 32)),
    "spektra_3f_three_quarter": (SPEKTRA, 54, ['3/4"', '330"', '.009"', '110"'], dict(wood_chip_in=0.009, wood_ipm=330, d_in=0.75)),
    "onsrud_hw_52200_3q": (ONS_HW, 30, [".009-.011"], dict(three_quarter_in=(0.009, 0.011))),
    "spektra_depth_2xd": (SPEKTRA, 38, ["2 x D Reduce feed rate by 25%"], dict(ratio=2, cut=0.25)),
    "spektra_depth_3xd": (SPEKTRA, 39, ["3 x D Reduce feed rate by 50%"], dict(ratio=3, cut=0.50)),
    # Onsrud 52-200/57-200 row (column mapping by character position: assert_claims checks it).
    "onsrud_hw_52200": (ONS_HW, 30, ["52-200/", ".005-.007", ".003-.005"], dict(quarter_in=(0.005, 0.007), eighth_in=(0.003, 0.005))),
    "onsrud_sw_52200": (ONS_SW, 30, ["52-200/", ".007-.009", ".006-.008"], dict(quarter_in=(0.007, 0.009), eighth_in=(0.006, 0.008))),
    "onsrud_mdf_52200": (ONS_MDF, 29, ["52-200/", ".006-.008", ".005-.007"], dict(quarter_in=(0.006, 0.008), eighth_in=(0.005, 0.007))),
    "onsrud_depth": (ONS_HW, 11, ["DEPTH OF CUT: 1 x D Use recommended chip load"], {}),
    # Freud solid carbide chart (stored text in planning, not under crates/.../sources).
    "freud_eighth": (FREUD, 63, ['1/8"', '.002"-.005"', '.004"-.006"'], dict(hw_in=(0.002, 0.005), sw_in=(0.004, 0.006))),
    "freud_quarter": (FREUD, 64, ['1/4"', '.008"-.011"', '.010"-.012"'], dict(hw_in=(0.008, 0.011), sw_in=(0.010, 0.012))),
    "freud_shallow": (FREUD, 28, ["Always start with shallow passes"], {}),
    "freud_depth_2xd": (FREUD, 47, ["If Cut Depth is 2X the bit diameter, reduce the Chip Load by at least 25%"], {}),
    # Harvey 809500, wood end mills, square upcut.
    "harvey_header": (HARVEY, 12, [".125", ".250", "Radial", "Axial"], {}),
    "harvey_soft_slot": (HARVEY, 13, ["Slot - Rough", ".0059", ".0118", "1 x Dia  1 x Dia"], dict(eighth_in=0.0059, quarter_in=0.0118)),
    "harvey_soft_finish": (HARVEY, 17, ["Finishing", ".0045", ".0090", ".1 x Dia  3 x Dia"], dict(eighth_in=0.0045, quarter_in=0.0090)),
    "harvey_hard_slot": (HARVEY, 20, ["Slot - Rough", ".0053", ".0106", "1 x Dia  1 x Dia"], dict(eighth_in=0.0053, quarter_in=0.0106)),
    "harvey_hard_finish": (HARVEY, 24, ["Finishing", ".0041", ".0081", ".1 x Dia  3 x Dia"], dict(eighth_in=0.0041, quarter_in=0.0081)),
    "harvey_rpm": (HARVEY, 36, ["Starting RPM should be between 10,000 and 20,000 RPM"], dict(lo=10000, hi=20000)),
    # Amana ball nose v7, 2 flute, 18 000 rpm.
    "ball_hw": (BALL, 14, ["Hardwood", '0.003" - 0.005"', '0.005" - 0.007"'], dict(eighth_in=(0.003, 0.005), quarter_in=(0.005, 0.007))),
    "ball_sw": (BALL, 13, ["Softwood", '0.005" - 0.007"', '0.007" - 0.009"'], dict(eighth_in=(0.005, 0.007), quarter_in=(0.007, 0.009))),
    "ball_ramp": (BALL, 37, ["To find Ramp Down: Feed Rate IPM / # of flutes"], {}),
    # SpeTool tapered, 2.0 mm tip, wood.
    "spetool_2mm": (SPETOOL, 23, ["2.0 | Wood, MDF, Sign-Foam | 18,000 | 0.003 | 108 | 2.0 | 0.8"],
                    dict(tip_mm=2.0, rpm=18000, chip_in=0.003, feed_ipm=108, stepdown_mm=2.0, stepover_mm=0.8)),
    # Amana V-groove: 60 and 90 degree 2 flute, 90 ipm, .003; 18 and 30 degree 1 flute 50-130 ipm, .003-.007.
    "vgroove_hw": (VGROOVE, 11, ["Hard Wood", '50" - 130"', '0.003" - 0.007"', '90"', '0.003"'],
                   dict(one_flute_ipm=(50, 130), one_flute_in=(0.003, 0.007), two_flute_ipm=90, two_flute_in=0.003)),
    # Fusion adaptive: no wood guideline; aluminium is the softest printed material.
    "fusion_alu": (FUSION, 29, ["1.5 to 2 times the tool diameter", "30% of the tool diameter"], dict(doc=(1.5, 2.0), woc=0.30)),
    # Garr finishing row: 2.5 % D radial at 1 % D chip; slotting 1.5 - 3 % D.
    "garr_finish": (GARR, 893, ["Radial = 30%-40%xD", "Radial = 2.5%xD"], {}),
    "garr_cpt": (GARR, 895, ["CPT (Fz) = 1.5% - 3% of diameter", "CPT (Fz) = 1% of diameter"], {}),
    # Sienci frame stiffness, derived k (research_sienci_stiffness).
    "stiff_force": (STIFF, 114, ["At 35.32 N the frame alone deflects 371 um"], {}),
}


def check_facts():
    bad = []
    for fid, (path, line, needles, _vals) in FACTS.items():
        text = open(os.path.join(REPO, path), encoding="utf-8", errors="replace").read().split("\n")
        if line > len(text):
            bad.append((fid, "line past end"))
            continue
        row = text[line - 1]
        squeezed = " ".join(row.split())
        for n in needles:
            if n not in row and " ".join(n.split()) not in squeezed:
                bad.append((fid, f"{n!r} not on {path}:{line}"))
    return bad


def v(fid):
    return FACTS[fid][3]


def mm(x):
    return x * IN


def lift(ae_over_d):
    """Garr: lift = 1/sin(psi), cos psi = 1 - ae/r, for ae < r (CREDITS.md:384-391)."""
    c = 1.0 - 2.0 * ae_over_d
    if c <= 0.0:
        return 1.0
    return 1.0 / math.sqrt(1.0 - c * c)


def judgement():
    """The reviewer's safe cut per toolpath kind, white oak, Shapeoko-class hobby table."""
    J = {}
    s = v("sienci_hw_quarter_full")
    d14 = mm(s["d_in"])
    sienci_chip_2f = s["feed"] / (s["rpm"] * 2)  # chart prints no flute count; CREDITS assumes 2
    c3d_m = v("c3d_mahogany")
    c3d_chip = mm(c3d_m["feed_ipm"]) / (c3d_m["rpm"] * 3)
    spek = v("spektra_2f_6mm")
    spek_chip = mm(spek["wood_chip_in"])
    ons = v("onsrud_hw_52200")["quarter_in"]
    fre = v("freud_quarter")["hw_in"]
    harv = v("harvey_hard_slot")["quarter_in"]
    J["flat_6mm_hardwood_chip"] = dict(
        spektra=spek_chip, onsrud=(mm(ons[0]), mm(ons[1])), freud=(mm(fre[0]), mm(fre[1])), harvey_slot=mm(harv),
        sienci_hobby=sienci_chip_2f, c3d_hobby_3f=c3d_chip,
        judgement=(sienci_chip_2f, spek_chip),
        note="Lowest printed vendor point (Spektra, one wood column) and the Sienci hobby reading bound the safe chip.")
    J["flat_6mm_hardwood_slot"] = dict(
        sienci_depth=s["slot"], c3d_depth=mm(c3d_m["doc_in"]), d=d14,
        judgement_depth=min(s["slot"], mm(c3d_m["doc_in"])), judgement_width_over_d=1.0,
        judgement_feed_at_18000=spek_chip * 2 * v("spektra_rpm")["rpm"],
        note="Two hobby charts print a hardwood slot near 0.4 D.")
    J["flat_6mm_hardwood_pocket"] = dict(
        sienci_depth=s["pocket"], sienci_width=s["stepover"], width_over_d=s["stepover"] / d14,
        judgement_depth=s["pocket"], judgement_width=s["stepover"],
        note="Sienci prints the pocket depth with its stepover 2.85 mm (0.45 D).")
    s8 = v("sienci_hw_eighth_full")
    d8 = mm(s8["d_in"])
    f8 = v("freud_eighth")["hw_in"]
    J["flat_3175_hardwood_pocket"] = dict(
        sienci_depth=s8["pocket"], sienci_slot=s8["slot"], sienci_width=s8["stepover"], d=d8,
        depth_over_d=s8["pocket"] / d8, width_over_d=s8["stepover"] / d8,
        spektra_chip=mm(v("spektra_2f_eighth")["wood_chip_in"]),
        onsrud_chip=tuple(mm(x) for x in v("onsrud_hw_52200")["eighth_in"]),
        freud_chip=(mm(f8[0]), mm(f8[1])),
        note="R37 reference: Sienci prints 5.6 mm at 1.45 mm width for a 1/8 in hardwood pocket.")
    sf = v("sienci_hw_surfacing_full")
    J["face_18mm_hardwood"] = dict(
        sienci_d=sf["d_mm"], sienci_depth=sf["pocket"], sienci_width_over_d=sf["stepover"] / sf["d_mm"],
        sienci_feed=sf["feed"], sienci_rpm=sf["rpm"],
        note="Nearest printed facing tool: Sienci 22 mm surfacing bit, hardwood.")
    J["drill_6mm_plunge"] = dict(
        spektra_ramp=mm(spek["wood_ramp_ipm"]), sienci_plunge=s["plunge"],
        rule="flat end plunge = feed / Z (Amana 'To find Ramp Down: Feed Rate IPM / # of flutes')",
        judgement=(s["plunge"], mm(spek["wood_ramp_ipm"])))
    st = v("spetool_2mm")
    J["tapered_2mm_finish"] = dict(
        chip=mm(st["chip_in"]), feed=mm(st["feed_ipm"]), rpm=st["rpm"], stepdown=st["stepdown_mm"],
        stepover=st["stepover_mm"], sienci_tapered_rpm=v("sienci_hw_tapered_coarse")["rpm"],
        note="SpeTool prints the 2.0 mm tip in wood; Sienci prints 28500 rpm, above the 24000 rpm machine top.")
    vg = v("vgroove_hw")
    J["vbit_hardwood"] = dict(
        two_flute_feed=mm(vg["two_flute_ipm"]), two_flute_chip=mm(vg["two_flute_in"]),
        two_flute_chip_from_feed=mm(vg["two_flute_ipm"]) / (18000 * 2),
        one_flute_chip=tuple(mm(x) for x in vg["one_flute_in"]),
        sienci_vcarve_feed=v("sienci_hw_vcarve")["feed"],
        note="Amana prints 90 ipm and .003 in for 2 flutes; 90/(18000 x 2) = .0025 in, so the two columns disagree.")
    b = v("ball_hw")
    sb = v("sienci_hw_ball_quarter_finish")
    J["ball_6mm_hardwood"] = dict(
        amana_quarter=tuple(mm(x) for x in b["quarter_in"]), amana_eighth=tuple(mm(x) for x in b["eighth_in"]),
        sienci_finish_chip_2f=sb["feed"] / (sb["rpm"] * 2), sienci_finish_stepover=sb["stepover"])
    hf, hs = v("harvey_hard_finish"), v("harvey_hard_slot")
    J["harvey_finish_witness"] = dict(
        ratio=hf["quarter_in"] / hs["quarter_in"], radial_over_d=0.1, axial_over_d=3,
        lift_at_0p1d=lift(0.1), depth_scale_3xd=1.0 - v("spektra_depth_3xd")["cut"],
        thinned_reading=lift(0.1) * (1.0 - v("spektra_depth_3xd")["cut"]),
        unthinned_reading=1.0 - v("spektra_depth_3xd")["cut"])
    J["material_order"] = dict(
        onsrud_quarter_hw_lt_sw=v("onsrud_hw_52200")["quarter_in"][1] < v("onsrud_sw_52200")["quarter_in"][1],
        freud_quarter_hw_lt_sw=v("freud_quarter")["hw_in"][1] < v("freud_quarter")["sw_in"][1],
        harvey_quarter_hw_lt_sw=v("harvey_hard_slot")["quarter_in"] < v("harvey_soft_slot")["quarter_in"],
        amana_ball_quarter_hw_lt_sw=b["quarter_in"][1] < v("ball_sw")["quarter_in"][1],
        spektra_one_wood_column=True,
        sienci_slot_ratio_sw_over_hw=v("sienci_sw_quarter_full")["slot"] / s["slot"],
        c3d_slot_ratio_pine_over_mahogany=v("c3d_pine")["doc_in"] / c3d_m["doc_in"])
    J["hobby_slot_points"] = {
        "sienci_sw_or_mdf_quarter": dict(depth=v("sienci_sw_quarter_full")["slot"], chip_2f=sienci_chip_2f),
        "sienci_hw_quarter": dict(depth=s["slot"], chip_2f=sienci_chip_2f),
        "c3d_pine": dict(depth=mm(v("c3d_pine")["doc_in"]), chip_3f=mm(v("c3d_pine")["feed_ipm"]) / (v("c3d_pine")["rpm"] * 3)),
        "c3d_mahogany": dict(depth=mm(c3d_m["doc_in"]), chip_3f=c3d_chip),
        "c3d_mdf": dict(depth=mm(v("c3d_mdf")["doc_in"]), chip_3f=mm(v("c3d_mdf")["feed_ipm"]) / (v("c3d_mdf")["rpm"] * 3)),
    }
    J["face_18mm_vendor_chip"] = dict(
        onsrud_3q=tuple(mm(x) for x in v("onsrud_hw_52200_3q")["three_quarter_in"]),
        spektra_3f_3q=mm(v("spektra_3f_three_quarter")["wood_chip_in"]),
        note="No printed row at 18 mm; the 3/4 in (19.05 mm) rows are the nearest.")
    J["flat_2mm_1f_hardwood"] = dict(
        spektra_sixteenth=mm(v("spektra_2f_sixteenth")["wood_chip_in"]),
        spektra_3_32=mm(v("spektra_2f_3_32")["wood_chip_in"]),
        sienci_sixteenth_slot=v("sienci_hw_sixteenth_full")["slot"],
        sienci_sixteenth_pocket=v("sienci_hw_sixteenth_full")["pocket"],
        sienci_sixteenth_stepover=v("sienci_hw_sixteenth_full")["stepover"],
        sienci_sixteenth_feed=v("sienci_hw_sixteenth_full")["feed"],
        feed_1f_at_18000=(mm(v("spektra_2f_sixteenth")["wood_chip_in"]) * 18000,
                          mm(v("spektra_2f_3_32")["wood_chip_in"]) * 18000),
        note="2 mm lies between the printed 1/16 in and 3/32 in rows; the 1 flute feed is chip x rpm x 1.")
    J["lift_table"] = {k: lift(k) for k in (0.05, 0.1, 0.2, 0.5)}
    return J


def main():
    bad = check_facts()
    if bad:
        for b in bad:
            print("FACT FAIL", b)
        sys.exit(1)
    J = judgement()
    with open(os.path.join(HERE, "judgement.json"), "w") as f:
        json.dump(J, f, indent=1)
    print("facts", len(FACTS), "ok; judgement keys", len(J))


if __name__ == "__main__":
    main()
