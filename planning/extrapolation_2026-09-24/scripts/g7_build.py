#!/usr/bin/env python3
"""G7 (material physics) fetch: build sources.json and candidate_rows.json.

The script writes the two files under fetch/G7/. It checks that every
"verbatim" string is a substring of its stored text, and that every
pdf_sha256 matches the downloaded file in fetch/G7/pdf/. It stops on a
mismatch. The numbers it computes are listed in each row's
"derived_fields"; every other number is copied from the stored text.

Usage: python3 scripts/g7_build.py   (from planning/extrapolation_2026-09-24)
"""
import hashlib
import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
G7 = os.path.join(HERE, "..", "fetch", "G7")
ACCESSED = "2026-09-24"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def norm_ws(s):
    return " ".join(s.split())


SOURCES = [
    {
        "source_id": "g7_goli2018_round_shape_ks",
        "source_vendor": "peer_reviewed",
        "source_title": "Goli G., Curti R., Marcon B., Scippa A., Campatelli G., Furferi R., Denaud L. (2018). Specific Cutting Forces of Isotropic and Orthotropic Engineered Wood Products by Round Shape Machining. Materials 11(12):2575. doi:10.3390/ma11122575",
        "source_url": "https://www.ebi.ac.uk/europepmc/webservices/rest/PMC6315737/fullTextXML",
        "pdf_file": "pdf/PMC6315737.xml",
        "stored_text": "sources/pmc6315737_round_shape_ks.txt",
        "coverage_notes": "Europe PMC JATS XML (hash is of the XML), converted by scripts/jats2txt.py. Tables 2-4 print the affine slope Ks (N/mm2) and intercept Int (N/mm) of the cutting force per mm of edge engaged, for PTFE, MDF, beech LVL and poplar LVL. Up-milling, one straight blade, D 80 mm, rake 25 deg, 3000 rpm, 2 m/min, ae 0.3-1.5 mm, hm 0.041-0.091 mm. Angle convention: 0 deg = across the grain, 90 deg = along the grain. The engine's SheetGood::Mdf Kc 31.4 is this paper's Ks. The authors call negative poplar Ks values possible measurement error (about +-5 N/mm2). No plywood, no solid softwood.",
    },
    {
        "source_id": "g7_palubicki2021_particleboard",
        "source_vendor": "peer_reviewed",
        "source_title": "Palubicki B. (2021). Cutting Forces in Peripheral Up-Milling of Particleboard. Materials 14(9):2208. doi:10.3390/ma14092208",
        "source_url": "https://www.ebi.ac.uk/europepmc/webservices/rest/PMC8123317/fullTextXML",
        "pdf_file": "pdf/PMC8123317.xml",
        "stored_text": "sources/palubicki2021_particleboard.txt",
        "coverage_notes": "Europe PMC JATS XML. Prints the average principal specific cutting force kc = Fc/(b h) for particleboard (542.5 kg/m3, b 17 mm): 32.0 N/mm2 at 40 m/s and 37.6 N/mm2 at 60 m/s; one carbide knife, R 82.5 mm, rake 13 deg, fz 1.5 mm, H 2 mm, IUCT up to about 0.31 mm. It is a total kc, not an affine slope. It prints no grain-orientation spread (particleboard is isotropic); the engine's GRAIN_ANISOTROPY_FACTOR comment cites it for one.",
    },
    {
        "source_id": "g7_yang2022_hdpe",
        "source_vendor": "peer_reviewed",
        "source_title": "Yang B., Wang H., Fu K., Wang C. (2022). Prediction of Cutting Force and Chip Formation from the True Stress-Strain Relation Using an Explicit FEM for Polymer Machining. Polymers 14(1):189. doi:10.3390/polym14010189",
        "source_url": "https://www.ebi.ac.uk/europepmc/webservices/rest/PMC8747417/fullTextXML",
        "pdf_file": "pdf/PMC8747417.xml",
        "stored_text": "sources/yang2022_hdpe.txt",
        "coverage_notes": "Europe PMC JATS XML. HDPE orthogonal cutting at very low speed (the text prints both 10 mm/min and 10 mm/s), rake 15 and 30 deg. Prints a yield stress derived from the cutting analysis (46.89 and 33.85 MPa) and a fracture toughness (1.173 and 1.368 kJ/m2). These are Atkins-model material parameters, not a milling specific cutting force. The engine's Plastic::Hdpe Kc 40.0 is the midpoint of the two yield stresses.",
    },
    {
        "source_id": "g7_kopecky2019_mdf_quasi_orthogonal",
        "source_vendor": "peer_reviewed",
        "source_title": "Kopecky Z., Hlaskova L., Solar A., Nesazal P. (2019). Cutting forces in quasi-orthogonal CNC milling. Wood Research 64(5):879-890",
        "source_url": "https://www.woodresearch.sk/wr/201905/12.pdf",
        "pdf_file": "pdf/woodresearch_201905_12.pdf",
        "stored_text": "sources/woodresearch_201905_12.txt",
        "coverage_notes": "pdftotext -layout. MDF only (684 kg/m3, 18 mm board, MC 3 %). One-insert S12L cutter, D 12 mm, rake 19 deg, edge radius 11 um, 15000 rpm, fz 0.1-0.3 mm, e 2 mm. Prints the per-tooth cutting force lines Fc1z = 49.954 hm + 5.3047 (conventional) and 49.123 hm + 5.936 (climb), in N, and Table 1 (tau_gamma, R, Q, gamma, Phi). The text does not state the chip width that Fc1z covers. This is the fit that crates/rs_cam_core/src/feeds/force.rs uses as LIT_KS 49.95 and LIT_FEDGE 5.30 and attaches to GenericHardwood. A plain curl works; a 'Mozilla/5.0' user agent receives an 'Access Forbidden' HTML page with HTTP 200.",
    },
    {
        "source_id": "g7_durkovic2017_oak_peripheral",
        "source_vendor": "peer_reviewed",
        "source_title": "Durkovic M., Danon G. (2017). Comparison of measured and calculated values of cutting forces in oak wood peripheral milling. Wood Research 62(2):293-306",
        "source_url": "https://www.woodresearch.sk/wr/201702/11.pdf",
        "pdf_file": "pdf/woodresearch_201702_11.pdf",
        "stored_text": "sources/woodresearch_201702_11.txt",
        "coverage_notes": "pdftotext -layout. Oak (Quercus robur), 745 kg/m3 measured, MC 7.3 %. Table-mounted spindle moulder, 4-blade cutterhead D 125 mm, rake 16/20/25 deg, 5860 rpm, vc 38.2 m/s, up milling along the grain, em 0.0216-0.1295 mm. Prints power-law fits of measured motor power P = 1890.7 em^0.477 W and of force per blade Fb = 243.6 em^0.4761 N (from motor power, so drive losses are inside). Prints the Krsljak textbook coefficient Ke1 = 14 N/mm2 (oak, longitudinal, e = 1 mm) and a table of Krsljak-model K = 57.99-140.63 N/mm2. Sample thickness 30 mm; the chip width is not printed as such.",
    },
    {
        "source_id": "g7_curti2021_generalized_wood_model",
        "source_vendor": "peer_reviewed",
        "source_title": "Curti R., Marcon B., Denaud L., Togni M., Furferi R., Goli G. (2021). Generalized cutting force model for peripheral milling of wood, based on the effect of density, uncut chip cross section, grain orientation and tool helix angle. European Journal of Wood and Wood Products (2021). doi:10.1007/s00107-021-01667-5",
        "source_url": "https://sam.ensam.eu/bitstream/handle/10985/19997/LABOMAP_EJWWP_2021_MARCON.pdf?sequence=1&isAllowed=y",
        "pdf_file": "pdf/marcon2021_generalized_force_model.pdf",
        "stored_text": "sources/marcon2021_generalized_force_model.txt",
        "coverage_notes": "Author-deposited version (SAM ENSAM handle 10985/19997), CC BY 4.0. pdftotext -layout. Five solid woods 287.1-1079.5 kg/m3 (paulownia, lime, maple, oak, azobe), 30 mm disks, 20 mm 2-flute carbide, rake 25 deg, helix 0/15/30 deg, 3000 rpm, 2000 mm/min, ae 0.5-2.5 mm, h about 0.040-0.100 mm. Table 5 prints a density-normalised quadratic model of Ks and Int against grain angle GA (deg) per milling mode and helix; Table 6 prints its NRMSE (8-38 %). Grain angle convention: angle between the edge velocity and the grain; 0 and 180 = along the grain. The per-species Ks values are in figures only.",
    },
    {
        "source_id": "g7_goli2023_iwms25_ewp",
        "source_vendor": "peer_reviewed",
        "source_title": "Goli G., Curti R., Todaro L., Marcon B. (2023). Specific cutting coefficients for the most common engineered wood products. 25th International Wood Machining Seminar, Nagoya, 2023-10-04 (HAL hal-04274766; SAM ENSAM handle 10985/24362)",
        "source_url": "https://sam.ensam.eu/bitstream/handle/10985/24362/LABOMAP_IWMS25th_2023_GOLI.pdf?sequence=1&isAllowed=y",
        "pdf_file": "pdf/goli2023_iwms25_ewp_ks.pdf",
        "stored_text": "sources/goli2023_iwms25_ewp_ks.txt",
        "coverage_notes": "Author-deposited conference paper. The HAL copy is behind the Anubis bot wall; the SAM ENSAM copy downloads with curl. pdftotext -layout. Particleboard 737.8, MDF 720.0, OSB 585.7, poplar plywood 430.0 kg/m3. 20 mm 2-flute carbide, rake 25 deg, helix 0 and 30 deg, 3000 rpm, 2000 mm/min, ae 0.5-2.5 mm, h 40-100 um; model Fc = (Ks h + Int) ap. The Ks and Int values are printed ONLY as scatter plots (Figures 3-6); the text prints no number. The only plywood force data this fetch found. Figure 5's y axis says N/mm^2 for an intercept (N/mm), and its caption says straight blade while panel (c) says 30 deg helix.",
    },
    {
        "source_id": "g7_porankiewicz2007_low_density",
        "source_vendor": "peer_reviewed",
        "source_title": "Porankiewicz B., Bermudez J.C., Tanaka C. (2007). Cutting forces by peripheral cutting of low density wood species. BioResources 2(4):671-681",
        "source_url": "https://bioresources.cnr.ncsu.edu/BioRes_02/BioRes_02_4_671_681_Pornkiewicz_BT_CuttingForces_Peripheral_LowDenWood.pdf",
        "pdf_file": "pdf/porankiewicz2007_lowdensity.pdf",
        "stored_text": "sources/porankiewicz2007_lowdensity.txt",
        "coverage_notes": "pdftotext -layout. Yellow poplar (400 kg/m3) routing, r 5 mm, fz 0.2 mm, gs 2 mm, ws 10 mm, vc 30 m/s; Cordia alliodora grooving. Prints only multi-factor non-linear regressions of Fc against edge wear, grain angle, vc, fz and MC, and the classical Fc = ap ws K C... form without a K value. No single specific cutting force at a stated chip thickness. Read; no candidate row.",
    },
    {
        "source_id": "g7_fpl_gtr190_wood_handbook_2010",
        "source_vendor": "usda_fpl",
        "source_title": "Forest Products Laboratory (2010). Wood Handbook: Wood as an Engineering Material. General Technical Report FPL-GTR-190 (mirror copy)",
        "source_url": "https://ww2.arb.ca.gov/sites/default/files/cap-and-trade/protocols/usforest/2011/usfs_wood_handbook_2010.pdf",
        "pdf_file": "pdf/fpl_gtr190_wood_handbook_2010.pdf",
        "stored_text": "sources/fpl_gtr190_wood_handbook_2010_excerpt.txt",
        "coverage_notes": "The FPL original (fpl.fs.usda.gov/documnts/fplgtr/fpl_gtr190.pdf) returns HTTP 403 to curl; this is the California Air Resources Board mirror, 11 MB. The stored text is an EXCERPT (table of contents and grep hits), because the full text is 3.2 MB. Chapters 1-20 contain no machining chapter. A search for 'specific cutting', 'cutting energy', 'cutting power' and 'cutting force' returns no line. 'Machinability' appears only as qualitative species ratings in Chapter 2 and the index. Result: the Wood Handbook prints no specific cutting energy or Kc; it is a refusal reason, not a source of a number. Its Table 5-3a shear strengths are the base of the engine's solid-wood Kc (already in the manifest as fpl_ch5_2010).",
    },
]


def goli18(v):
    return v


# Derived helpers --------------------------------------------------------

def kopecky_hm():
    r, e = 6.0, 2.0
    psi = math.acos(1.0 - e / r)
    s = math.sin(psi / 2.0)
    return 0.1 * s, 0.3 * s, math.degrees(psi / 2.0)


def kopecky_b():
    k, q = 49.954, 5.3047
    tau, gamma, Q, R = 1.1757, 1.569, 0.656, 294.706e-3  # R in N/mm
    b_from_slope = k * Q / (tau * gamma)
    b_from_intercept = q * Q / R
    return b_from_slope, b_from_intercept


def vc(d_mm, rpm):
    return math.pi * d_mm * rpm / 60000.0


def row(**kw):
    base = {
        "observation_id": None,
        "source_id": None,
        "source_vendor": "peer_reviewed",
        "source_title": None,
        "source_url": None,
        "accessed_on": ACCESSED,
        "source_page": None,
        "evidence_grade": "b",
        "row_kind": "exact",
        "tool_family": None,
        "tool_subfamily": None,
        "operation_family": "peripheral_milling",
        "pass_role": None,
        "material_family": None,
        "material_label": None,
        "hardness_kind": None,
        "hardness_value": None,
        "diameter_mm": None,
        "flute_count": None,
        "chipload_min_mm_tooth": None,
        "chipload_max_mm_tooth": None,
        "ap_rule": None,
        "ap_max_factor": None,
        "notes": None,
        "extrapolation_group": "G7",
        "verbatim": None,
        # G7 physics fields (not in the LUT schema; the reconciler decides
        # the storage shape).
        "quantity": None,
        "force_model": None,
        "ks_n_per_mm2": None,
        "ks_min_n_per_mm2": None,
        "ks_max_n_per_mm2": None,
        "int_n_per_mm": None,
        "int_min_n_per_mm": None,
        "int_max_n_per_mm": None,
        "kc_n_per_mm2": None,
        "kc_min_n_per_mm2": None,
        "kc_max_n_per_mm2": None,
        "chip_thickness_mm_min": None,
        "chip_thickness_mm_max": None,
        "cutting_speed_m_s": None,
        "rake_deg": None,
        "helix_deg": None,
        "milling_mode": None,
        "grain_angle": None,
        "density_kg_m3": None,
        "test_method": None,
        "derived_fields": [],
    }
    for k in kw:
        if k not in base:
            raise KeyError(k)
    base.update(kw)
    return base


def build_rows(src):
    rows = []
    s = src["g7_goli2018_round_shape_ks"]
    common18 = dict(
        source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
        tool_family="straight_blade_cutterhead", tool_subfamily="single_insert_80mm",
        diameter_mm=80.0, flute_count=1, rake_deg=25.0, helix_deg=0.0, milling_mode="up",
        chip_thickness_mm_min=0.041, chip_thickness_mm_max=0.091,
        cutting_speed_m_s=round(vc(80.0, 3000.0), 2),
        force_model="Fc per mm of engaged edge = Ks*h + Int (linear regression over 4 depths of cut)",
        test_method="round-shape disk, dynamometer Kistler 9255A, 3000 rpm, 2 m/min, ae 0.3/0.7/1.1/1.5 mm",
        ap_rule="force normalised by the disk thickness (ap = whole disk)",
    )
    rows.append(row(
        observation_id="x-g7-goli2018-mdf-up-straight",
        source_page="Table 3 (MDF, up-milling)",
        material_family="mdf", material_label="MDF, 711 kg/m3, MC 9.3 %",
        density_kg_m3=711.0, quantity="affine_ks_int",
        ks_n_per_mm2=31.44, ks_min_n_per_mm2=25.81, ks_max_n_per_mm2=35.58,
        int_n_per_mm=3.36, int_min_n_per_mm=2.96, int_max_n_per_mm=3.83,
        verbatim="[row] Ks [N mm−2] | 31.44 (2.68) | 25.81 | 35.58",
        notes="Int row verbatim: '[row] Int [N mm−1] | 3.36 (0.27) | 2.96 | 3.83'. Min/max are over grain-angle positions (MDF is isotropic). The engine's SheetGood::Mdf Kc 31.4 is this Ks. cutting_speed_m_s is derived (pi*80*3000/60000).",
        derived_fields=["cutting_speed_m_s"], **common18))
    rows.append(row(
        observation_id="x-g7-goli2018-ptfe-up-straight",
        source_page="Table 2 (PTFE, up-milling)",
        material_family="plastic_ptfe", material_label="PTFE, 2202 kg/m3 (reference material)",
        density_kg_m3=2202.0, quantity="affine_ks_int",
        ks_n_per_mm2=20.33, ks_min_n_per_mm2=14.46, ks_max_n_per_mm2=24.81,
        int_n_per_mm=2.71, int_min_n_per_mm=2.40, int_max_n_per_mm=3.15,
        verbatim="[row] Ks [N mm−2] | 20.33 (2.90) | 14.46 | 24.81",
        notes="Int row verbatim: '[row] Int [N mm−1] | 2.71 (0.24) | 2.40 | 3.15'. The engine has no PTFE family; recorded as the only measured router-type Ks for an unfilled thermoplastic this fetch found.",
        derived_fields=["cutting_speed_m_s"], **common18))
    for ang, ks, it, direction in ((0, 32.29, 7.66, "across the grain"), (90, 11.73, 3.70, "along the grain")):
        rows.append(row(
            observation_id=f"x-g7-goli2018-beech-lvl-{ang}deg-up-straight",
            source_page=f"Table 4 (beech LVL, {ang} deg)",
            material_family="lvl_beech", material_label="beech LVL, 724 kg/m3, 11 plies parallel + 2 plies crossed",
            density_kg_m3=724.0, quantity="affine_ks_int", grain_angle=f"{ang} deg ({direction}; Goli 2018 convention)",
            ks_n_per_mm2=ks, int_n_per_mm=it,
            ks_min_n_per_mm2=6.61, ks_max_n_per_mm2=51.04, int_min_n_per_mm=3.33, int_max_n_per_mm=7.66,
            verbatim=f"[row] {ang} | {ks:.2f} | {it:.2f} |",
            notes="Min/max are the Table 4 extremes over all angles (text: 'very large variations from 6.61 N mm−2 to 51.04 N mm−2'). LVL is a veneer product like plywood but with nearly all plies parallel; it is NOT plywood. Poplar LVL rows are not transcribed: several are negative and the authors give +-5 N/mm2 measurement error.",
            derived_fields=["cutting_speed_m_s"], **common18))

    s = src["g7_palubicki2021_particleboard"]
    for v, kc in ((40.0, 32.0), (60.0, 37.6)):
        rows.append(row(
            observation_id=f"x-g7-palubicki2021-pb-up-vc{int(v)}",
            source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
            source_page="Abstract and Conclusions",
            tool_family="straight_blade_cutterhead", tool_subfamily="single_knife_165mm",
            diameter_mm=165.0, flute_count=1, rake_deg=13.0, helix_deg=0.0, milling_mode="up",
            material_family="particleboard", material_label="particleboard, 542.5 kg/m3, b 17 mm",
            density_kg_m3=542.5, quantity="kc_total", kc_n_per_mm2=kc,
            force_model="kc = Fc/(b*h) averaged over the instantaneous uncut chip thickness range",
            chip_thickness_mm_min=None, chip_thickness_mm_max=0.31, cutting_speed_m_s=v,
            test_method="single-knife up-milling, piezo dynamometer, fz 1.5 mm, H 2 mm, 240 cuts per speed",
            verbatim="The average specific principal cutting force for PB peripheral up-milling is equal to 32.0 N/mm2 for slow and 37.6 N/mm2 for fast milling.",
            notes="The rubbing stage at the smallest IUCT is excluded by the authors; the lower bound of h is not printed. The engine's SheetGood::Particleboard 35.0 is the mean of the two (derived there, not here). This is a total kc (edge effect folded in), not an affine slope; it must not be fed into force.rs as if it were one.",
        ))

    s = src["g7_yang2022_hdpe"]
    for rake, ys, rt in ((15.0, 46.89, 1.173), (30.0, 33.85, 1.368)):
        rows.append(row(
            observation_id=f"x-g7-yang2022-hdpe-yield-rake{int(rake)}",
            source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
            source_page="Section on Figure 8 (fracture toughness from cutting)",
            tool_family="orthogonal_cutting_tool", operation_family="orthogonal_cutting",
            rake_deg=rake, material_family="plastic_hdpe", material_label="HDPE",
            quantity="shear_yield_stress_from_cutting", kc_n_per_mm2=None,
            force_model=f"Atkins cutting mechanics: yield stress {ys} MPa, fracture toughness {rt} kJ/m2",
            test_method="orthogonal cutting at very low speed (text prints 10 mm/min and 10 mm/s), carbide tool, edge radius about 5 um",
            verbatim="The yielding stress derived from the analyses have the values of 46.89 and 33.85 MPa for the two cutting tools",
            notes="Pairing of 46.89 with rake 15 deg and 33.85 with rake 30 deg follows the order of the preceding sentence ('1.173 kJ/m2 and 1.368 kJ/m2 using the cutting tool with a rake angle of 15° and 30°'); the paper does not state the pairing explicitly for the yield stress (inferred). This is NOT a specific cutting force: kc = tau*gamma/Q + R/(Q*h) needs gamma and Q, which the text does not print. Chip thickness range not printed (sensor range 0-300 um).",
        ))

    s = src["g7_kopecky2019_mdf_quasi_orthogonal"]
    hmin, hmax, half = kopecky_hm()
    b1, b2 = kopecky_b()
    for mode, k, q, tau, R, vb in (("conventional", 49.954, 5.3047, 1.1757, 294.706, "Conventional milling: Fc1z = 49.954 hm + 5.3047"),
                                     ("climb", 49.123, 5.936, 1.0826, 329.778, "Climb milling: Fc1z = 49.123 hm + 5.936")):
        rows.append(row(
            observation_id=f"x-g7-kopecky2019-mdf-{mode}",
            source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
            source_page="Results, straight-line fits and Tab. 1",
            tool_family="straight_insert_shank_cutter", tool_subfamily="Habilis S12L, IGM N010 insert",
            diameter_mm=12.0, flute_count=1, rake_deg=19.0, helix_deg=0.0,
            milling_mode="up" if mode == "conventional" else "down",
            material_family="mdf", material_label="MDF, 684 kg/m3, 18 mm, MC 3 %", density_kg_m3=684.0,
            quantity="per_tooth_force_line",
            force_model=f"Fc1z [N] = {k} * hm [mm] + {q}; chip width not printed",
            chip_thickness_mm_min=round(hmin, 4), chip_thickness_mm_max=round(hmax, 4),
            cutting_speed_m_s=round(vc(12.0, 15000.0), 2),
            test_method=f"quasi-orthogonal edge milling, Kistler 9257B, 15000 rpm, fz 0.1-0.3 mm, e 2 mm; Tab. 1 tau_gamma {tau} MPa, R {R} J/m2",
            verbatim=vb,
            notes=(f"hm range derived: hm = fz*sin(psi/2), psi = acos(1-e/r) with e 2, r 6 -> psi/2 = {half:.2f} deg (the paper prints phi2 = 24.09 deg), fz 0.1-0.3. "
                   f"The paper prints no chip width b for Fc1z. Back-calculation from Eq. 8 and Tab. 1 (derived): slope k = tau*b*gamma/Q gives b = {b1:.1f} mm (close to the 18 mm board); intercept q = R*b/Q gives b = {b2:.1f} mm. "
                   "If b is about 18 mm, the per-mm-of-edge line is about 2.8*hm + 0.29 N/mm; if Fc1z is already per mm, it is 49.95*hm + 5.30 N/mm. The engine assumes the second reading and attaches the line to hardwood. Unresolved; see lut_discrepancies.md D1."),
            derived_fields=["chip_thickness_mm_min", "chip_thickness_mm_max", "cutting_speed_m_s"],
        ))

    s = src["g7_durkovic2017_oak_peripheral"]
    rows.append(row(
        observation_id="x-g7-durkovic2017-oak-force-per-blade",
        source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
        source_page="Eq. 7 and Eq. 3",
        tool_family="planing_cutterhead", tool_subfamily="4-blade carbide, D 125 mm, B 40 mm",
        diameter_mm=125.0, flute_count=4, rake_deg=None, helix_deg=0.0, milling_mode="up",
        material_family="hardwood", material_label="oak (Quercus robur), 745 kg/m3, MC 7.28 %", density_kg_m3=745.0,
        hardness_kind=None, quantity="per_blade_force_power_law",
        force_model="Fb [N] = 243.6 * em^0.4761 (mean force per blade from measured motor power); P [W] = 1890.7 * em^0.477",
        chip_thickness_mm_min=0.0216, chip_thickness_mm_max=0.1295, cutting_speed_m_s=38.2,
        kc_min_n_per_mm2=round(243.6 * 0.1295 ** 0.4761 / (30.0 * 0.1295), 1),
        kc_max_n_per_mm2=round(243.6 * 0.0216 ** 0.4761 / (30.0 * 0.0216), 1),
        test_method="spindle moulder, 5860 rpm, motor input power (Power Expert), rake 16/20/25 deg pooled, u 4-16 m/min, a 2-4.5 mm",
        verbatim="F b = 243.6 . em0,4761",
        notes="The measured forces come from electric input power, so drive losses are inside them. kc_min/kc_max are derived: kc = Fb/(b*em) with b = 30 mm (the sample thickness; the Axelsson table's Fp 1.20 N/mm to F 36 N ratio gives the same 30 mm) at em 0.1295 and 0.0216 mm. Rake angles pooled (16, 20, 25 deg; the text prints '160' for 16, a typo).",
        derived_fields=["kc_min_n_per_mm2", "kc_max_n_per_mm2"],
    ))
    rows.append(row(
        observation_id="x-g7-durkovic2017-krsljak-ke1-oak",
        source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
        source_page="Method of coefficients (Krsljak 2013), Tab. 3",
        evidence_grade="c", material_family="hardwood", material_label="oak, longitudinal cutting (textbook coefficient)",
        quantity="kc_textbook_base", kc_n_per_mm2=14.0, kc_min_n_per_mm2=57.99, kc_max_n_per_mm2=140.63,
        chip_thickness_mm_min=0.0216, chip_thickness_mm_max=0.1295, cutting_speed_m_s=38.2,
        force_model="K = Ke1 * Cvr * Cu * Cdelta * Cphi * Ce * Cv * Crho; Ke1 at e = 1 mm; kc_min/max = the paper's Tab. 3 K column for these conditions",
        verbatim="of Ke1= 14 N.mm-2 for wood specific resistance was adopted that holds for longitudinal oak",
        notes="Grade c: a textbook coefficient (Krsljak 2013) quoted by the paper, and a model output (Tab. 3), not a measurement. Recorded because the engine's GenericHardwood base (13.0 N/mm2 FPL shear, x2.7) sits near Ke1 at e = 1 mm, and the textbook chip-thickness factor Ce = 0.9814 em^-0.3338 (printed, Eq. 11) is a published size-effect law of the kind MILLING_KC_FACTOR stands in for.",
    ))

    s = src["g7_curti2021_generalized_wood_model"]
    t5 = [
        ("up", 0, "−5 ⋅ 10−6 GA2 + 1⋅10−3 GA + 26⋅10−3", "−1⋅10−7 GA2 + 1⋅10−5 GA + 55⋅10−4", (-5e-6, 1e-3, 26e-3), (-1e-7, 1e-5, 55e-4)),
        ("up", 15, "−4⋅10−6 GA2 + 7⋅10−4 GA + 32 ⋅ 10−3", "8⋅10−8 GA2 − 2⋅10−5 GA + 2⋅10−4", (-4e-6, 7e-4, 32e-3), (8e-8, -2e-5, 2e-4)),
        ("up", 30, "−3⋅10−6 GA2 + 5⋅10−4 GA + 21⋅10−3", "7⋅10−8 GA2 − 1⋅10−5 GA − 6⋅10−5", (-3e-6, 5e-4, 21e-3), (7e-8, -1e-5, -6e-5)),
        ("down", 0, "−3⋅10−6 GA2 + 5 ⋅ 10−4 GA + 56⋅10−3", "−3⋅10−7 GA2 + 4⋅10−5 A + 47⋅10−4", (-3e-6, 5e-4, 56e-3), (-3e-7, 4e-5, 47e-4)),
        ("down", 15, "−3⋅10−6 GA2 + 5⋅10−4 GA + 49⋅10−3", "1⋅10−8 GA2 − 4⋅10−6 GA − 4⋅10−4", (-3e-6, 5e-4, 49e-3), (1e-8, -4e-6, -4e-4)),
        ("down", 30, "−2⋅10−6 GA2 + 4⋅10−4 GA + 30⋅10−3", "3⋅10−8 GA2 − 7⋅10−6 GA − 4⋅10−4", (-2e-6, 4e-4, 30e-3), (3e-8, -7e-6, -4e-4)),
    ]

    def quad(c, x):
        return c[0] * x * x + c[1] * x + c[2]

    for mode, lam, kstr, istr, kc3, ic3 in t5:
        k0, k90 = quad(kc3, 0), quad(kc3, 90)
        i0, i90 = quad(ic3, 0), quad(ic3, 90)
        rows.append(row(
            observation_id=f"x-g7-curti2021-ksnorm-{mode}-helix{lam}",
            source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
            source_page="Table 5 (quadratic model), Table 6 (NRMSE)",
            tool_family="flat_end_mill", tool_subfamily=f"G3 Fantacci 2-flute carbide, helix {lam} deg",
            diameter_mm=20.0, flute_count=2, rake_deg=25.0, helix_deg=float(lam), milling_mode=mode,
            material_family="solid_wood", material_label="solid wood 287-1080 kg/m3 (paulownia, lime, maple, oak, azobe)",
            quantity="density_normalised_ks_int_model",
            force_model=f"Fc = Ks_norm(GA)*h + int_norm(GA)*Ap x rho (Eq. 4 as printed); Ks_norm = {kstr} [N/mm2 per kg/m3]; int_norm = {istr}",
            chip_thickness_mm_min=0.040, chip_thickness_mm_max=0.100,
            cutting_speed_m_s=round(vc(20.0, 3000.0), 2),
            ks_min_n_per_mm2=None, ks_max_n_per_mm2=None,
            test_method="round-shape disks 250-150 mm, 30 mm thick, Kistler 9255A, 3000 rpm, 2000 mm/min, ae 0.5-2.5 mm",
            verbatim=kstr,
            notes=(f"Evaluated (derived): Ks_norm(GA 0, along grain) = {k0:.4f}, Ks_norm(GA 90, across) = {k90:.4f} N/mm2 per kg/m3; int_norm(0) = {i0:.5f}, int_norm(90) = {i90:.5f}. "
                   f"At rho 450 kg/m3: Ks {k0*450:.1f}-{k90*450:.1f} N/mm2; at rho 700: Ks {k0*700:.1f}-{k90*700:.1f} N/mm2 (derived). "
                   "Valid 287-1080 kg/m3 and h 40-100 um only. Table 6 prints NRMSE 8.1-37.8 % by species and tool. The Int line for down-milling helix 0 prints 'A' for 'GA' (typo in the source). Eq. 4's bracket (whether rho multiplies both terms) is ambiguous in the text; the density normalisation text says both Ks and Int were divided by density."),
            derived_fields=["cutting_speed_m_s"],
        ))

    s = src["g7_goli2023_iwms25_ewp"]
    fig = [
        ("plywood_poplar", "poplar plywood, 430.0 kg/m3", 430.0, "up", 0, 23.0, 43.0, 3.0, 4.9, "Figure 4 (a), Figure 3 (a)", "Specific cutting coefficient (Ks) for a straight blade tool in up-milling condition (a)"),
        ("plywood_poplar", "poplar plywood, 430.0 kg/m3", 430.0, "down", 0, 24.0, 43.0, 2.6, 5.3, "Figure 4 (b), Figure 3 (b)", "Specific cutting coefficient (Ks) for a straight blade tool in up-milling condition (a) and"),
        ("plywood_poplar", "poplar plywood, 430.0 kg/m3", 430.0, "up", 30, 20.0, 26.5, -0.35, -0.05, "Figure 6 (a), Figure 5 (a)", "the 30° helical angle tool (λ = 30°)"),
        ("plywood_poplar", "poplar plywood, 430.0 kg/m3", 430.0, "down", 30, 24.0, 29.5, -0.35, -0.25, "Figure 6 (b), Figure 5 (b)", "the 30° helical angle tool (λ = 30°)"),
        ("mdf", "MDF, 720.0 kg/m3", 720.0, "up", 0, 28.0, 30.0, 2.9, 3.1, "Figure 4 (a), Figure 3 (a)", "Specific cutting coefficient (Ks) for a straight blade tool in up-milling condition (a)"),
        ("particleboard", "particleboard, 737.8 kg/m3", 737.8, "up", 0, 12.0, 23.0, 2.6, 3.3, "Figure 4 (a), Figure 3 (a)", "Specific cutting coefficient (Ks) for a straight blade tool in up-milling condition (a)"),
    ]
    for fam, label, rho, mode, lam, kmin, kmax, imin, imax, page, vb in fig:
        rows.append(row(
            observation_id=f"x-g7-goli2023-{fam.replace('_', '-')}-{mode}-helix{lam}-figread",
            source_id=s["source_id"], source_title=s["source_title"], source_url=s["source_url"],
            source_page=page, evidence_grade="c", row_kind="derived",
            tool_family="flat_end_mill", tool_subfamily=f"G3 Fantacci 2-flute carbide, helix {lam} deg",
            diameter_mm=20.0, flute_count=2, rake_deg=25.0, helix_deg=float(lam), milling_mode=mode,
            material_family=fam, material_label=label, density_kg_m3=rho,
            quantity="affine_ks_int_figure_read",
            force_model="Fc = (Ks*h + Int)*ap",
            ks_min_n_per_mm2=kmin, ks_max_n_per_mm2=kmax, int_min_n_per_mm=imin, int_max_n_per_mm=imax,
            chip_thickness_mm_min=0.040, chip_thickness_mm_max=0.100,
            cutting_speed_m_s=round(vc(20.0, 3000.0), 2),
            test_method="round-shape disks, Kistler 9255A, 3000 rpm, 2000 mm/min, ae 0.5-2.5 mm, ap = board thickness",
            verbatim=vb,
            notes="DERIVED BY EYE: the paper prints these values only as scatter points. Read from a 110 dpi render of the stored PDF (pages 5-7); reading error about +-2 N/mm2 on Ks and +-0.2 N/mm on Int. The range spans the 18 grain-position points (plywood is moderately directional, mirrored about 90 deg). Not a transcription; the reconciler must not treat it as a printed value.",
            derived_fields=["ks_min_n_per_mm2", "ks_max_n_per_mm2", "int_min_n_per_mm", "int_max_n_per_mm", "cutting_speed_m_s"],
        ))
    return rows


def main():
    src = {}
    out_sources = []
    for s in SOURCES:
        p = os.path.join(G7, s["pdf_file"])
        t = os.path.join(G7, s["stored_text"])
        if not os.path.exists(p) or not os.path.exists(t):
            sys.exit(f"missing file for {s['source_id']}")
        entry = {
            "source_id": s["source_id"],
            "source_vendor": s["source_vendor"],
            "source_title": s["source_title"],
            "source_url": s["source_url"],
            "accessed_on": ACCESSED,
            "stored_text": s["stored_text"],
            "pdf_sha256": sha256(p),
            "hashed_file": s["pdf_file"],
            "coverage_notes": s["coverage_notes"],
        }
        out_sources.append(entry)
        src[s["source_id"]] = dict(s, text=open(t, encoding="utf-8").read())
    rows = build_rows(src)
    ids = set()
    for r in rows:
        if r["observation_id"] in ids or not r["observation_id"].startswith("x-g7-"):
            sys.exit(f"bad id {r['observation_id']}")
        ids.add(r["observation_id"])
        text = src[r["source_id"]]["text"]
        if norm_ws(r["verbatim"]) not in norm_ws(text):
            sys.exit(f"verbatim not in stored text: {r['observation_id']}: {r['verbatim']!r}")
    with open(os.path.join(G7, "sources.json"), "w", encoding="utf-8") as f:
        json.dump(out_sources, f, indent=2, ensure_ascii=False)
        f.write("\n")
    with open(os.path.join(G7, "candidate_rows.json"), "w", encoding="utf-8") as f:
        json.dump({"observations": rows}, f, indent=2, ensure_ascii=False)
        f.write("\n")
    per = {}
    for r in rows:
        per[r["source_id"]] = per.get(r["source_id"], 0) + 1
    for s in out_sources:
        print(s["source_id"], per.get(s["source_id"], 0), s["pdf_sha256"][:12])
    print("rows", len(rows))


if __name__ == "__main__":
    main()
