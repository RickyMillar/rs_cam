# G7: Material physics (no Kc for MDF, plywood, plastics)

Status: Phase 2 (trend) done 2026-09-24; ruled B6 2026-09-24; **landed 2026-09-25 (§5)**.

Inputs: the LUT at 67e98529 (`crates/rs_cam_core/data/vendor_lut/observations/*.json`),
`fetch/G7/` (Phase 1 fetch, verifier verdicts, `verified_rows.json`),
`inventory_cells.csv` (Phase 0), `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`,
and the engine source (read, not built). Scripts: `scripts/g7_verified_rows.py`
writes `fetch/G7/verified_rows.json`; `scripts/trend_g7.py` prints every table
below (read-only; run `python3 scripts/trend_g7.py`). Every number in §1 and
§3 that is not a quoted document value is **derived** by that script. No
vendor prints a Kc for wood or plastics.

The common measure. The sources print four different quantities (an affine
slope and intercept, a total kc, a per-tooth force with no chip width, a
yield stress). This file does not compare them raw. Each affine pair
(Ks N/mm², Int N/mm) is also shown as the equivalent total specific force
`kc_eq(h) = Ks + Int / h` at h = 0.05 and 0.10 mm (derived). That range is
inside every router study here. The engine line goes on the same scale:
`Ks_e = 49.95 · kc / 35.1`, `Int_e = 5.30 · kc / 35.1`.

## 0. The gap

**No cell of `inventory_cells.csv` is in G7.** The group is an overlay: it
changes the force and power numbers on cells that ship for other reasons.
It does not move a cell between ship and refuse inside the 960-cell matrix.

### 0.1 Correction to PLAN §2 and INVENTORY §1

PLAN §2 says "power on 208 of 448 cells only" and calls the cause "no Kc".
The matrix of 2026-09-23 shows otherwise (counted from the CSV):

| Quantity | Cells | Detail |
|---|---|---|
| Shipped cells | 448 | softwood 140, hardwood 134, mdf 98, plywood_hardwood 76 |
| Cells with `force_n` | 448 | every shipped cell, every material |
| Cells with `power_kw` | 196 | softwood 56, hardwood 56, mdf 48, plywood 36 |
| Operations with power | 6 | Face 34, Pocket 34, Rest 34, Zigzag 34, Adaptive 30, Adaptive3d 30 |
| Shipped cells with no power | 252 | Profile 26, ProjectCurve 26, DropCutter 22, RampFinish 22, RadialFinish 22, HorizontalFinish 22, Waterline 20, SteepShallow 20, Trace 18, Scallop 12, UnifiedFinish 12, SpiralFinish 12, Pencil 6, VCarve 4, Inlay 4, Chamfer 4 |

All four matrix materials have a Kc. The power gap is on the **operation
side**, and it is the same for every material. As read from the code (not
measured): `feeds::power_at_operating_point`
(`crates/rs_cam_core/src/feeds/operating_point.rs:193`) returns
`Err(PowerUnmodeled::NoRadialEngagement)` or `Err(NoDepthPerPass)` when the
operation has no `stepover()` or `depth_per_pass()`, and the matrix
instrument (`crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs:443`)
passes `fallback: None`. A Kc fetch does not close these 252 cells.

### 0.2 The engine rule that serves the group

| Rule | File | What it does |
|---|---|---|
| `Material::kc_n_per_mm2` | `crates/rs_cam_core/src/material/mod.rs:939` | returns one "Kc" per material, or `None` (refuse) |
| `MILLING_KC_FACTOR = 2.7` | `material/mod.rs:899` | multiplies the FPL shear strength of solid wood |
| `janka_to_kc_n_per_mm2` | `material/mod.rs:629` | `janka / 100` for `SolidWoodByJanka`, 200–4000 lbf, else `None` |
| `affine_coefficients_for_kc` | `crates/rs_cam_core/src/feeds/force.rs:118` | `Ks = 49.95 · kc / 35.1`, `F_edge = 5.30 · kc / 35.1` |
| `GRAIN_ANISOTROPY_FACTOR = 2.0` | `crates/rs_cam_core/src/tool_load/power.rs:95` | multiplies both terms for every material |
| (no helix term) | `feeds/force.rs`, `tool_load/power.rs:537` | the force line has no helix input; the power model states "no helix/grain decomposition" |
| `power_at_operating_point` | `feeds/operating_point.rs:193` | `Err(MaterialUnvalidated)` when Kc is `None` |

Materials that refuse today: every plastic except HDPE, fibreglass, foam
(by `None`), and `SolidWoodByJanka` outside 200–4000 lbf. None of them is in
the 960-cell matrix, so the matrix does not count them. The matrix
`plywood_hardwood` column uses `Plywood::BalticBirch` (Kc 13.0).

**HDPE ships a number today** (`Plastic::Hdpe => Some(40.0)`,
`material/mod.rs:1035`). The document behind it prints a yield stress, not a
cutting force (§1.1, D4 in `fetch/G7/lut_discrepancies.md`). Under the R1
rule this is a regression that needs a ruling, not a gap to fill.

## 1. The trend

### 1.1 What the LUT and the fetch hold

The vendor LUT holds chipload rows only. It has no Kc, force, power or
energy field:

#### T0. The vendor LUT (read-only scan)

| LUT files | observations | fields named kc/ks/force/power/energy |
|---|---|---|
| 22 | 389 | 0 |

#### T0b. LUT observations per material_family (chipload rows only)

| material_family | rows |
|---|---|
| hardwood | 88 |
| softwood | 84 |
| mdf | 74 |
| plywood_hardwood | 48 |
| plywood_softwood | 29 |
| aluminum | 28 |
| acrylic | 19 |
| hdpe | 7 |
| polycarbonate | 5 |
| hdf | 2 |
| particleboard | 2 |
| delrin | 2 |
| fiberglass | 1 |

So the trend comes from `fetch/G7/verified_rows.json` (24 rows, all
confirmed; §2). Every verified value, as the document prints it:

#### T3. Every verified value, as printed (figure reads are derived, grade c)

| row | material | quantity | Ks N/mm2 | Int N/mm | kc or other | h mm | mode | helix | rake | vc m/s | rho | grade/kind |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| goli2018-mdf-up-straight | mdf | affine_ks_int | 31.44 (25.81-35.58) | 3.36 (2.96-3.83) |  | 0.041-0.091 | up | 0.0 | 25.0 | 12.57 | 711.0 | b/exact |
| goli2018-ptfe-up-straight | plastic_ptfe | affine_ks_int | 20.33 (14.46-24.81) | 2.71 (2.4-3.15) |  | 0.041-0.091 | up | 0.0 | 25.0 | 12.57 | 2202.0 | b/exact |
| goli2018-beech-lvl-0deg-up-straight | lvl_beech | affine_ks_int | 32.29 (6.61-51.04) | 7.66 (3.33-7.66) |  | 0.041-0.091 | up | 0.0 | 25.0 | 12.57 | 724.0 | b/exact |
| goli2018-beech-lvl-90deg-up-straight | lvl_beech | affine_ks_int | 11.73 (6.61-51.04) | 3.7 (3.33-7.66) |  | 0.041-0.091 | up | 0.0 | 25.0 | 12.57 | 724.0 | b/exact |
| palubicki2021-pb-up-vc40 | particleboard | kc_total | - | - | 32 | ?-0.31 | up | 0.0 | 13.0 | 40.0 | 542.5 | b/exact |
| palubicki2021-pb-up-vc60 | particleboard | kc_total | - | - | 37.6 | ?-0.31 | up | 0.0 | 13.0 | 60.0 | 542.5 | b/exact |
| yang2022-hdpe-yield-rake15 | plastic_hdpe | shear_yield_stress_from_cutting | - | - | yield stress 46.89 MPa (not a Kc) | ?-? | - | - | 15.0 | - | - | b/exact |
| yang2022-hdpe-yield-rake30 | plastic_hdpe | shear_yield_stress_from_cutting | - | - | yield stress 33.85 MPa (not a Kc) | ?-? | - | - | 30.0 | - | - | b/exact |
| kopecky2019-mdf-conventional | mdf | per_tooth_force_line | - | - | Fc1z = 49.954 hm + 5.3047 N/tooth | 0.0408-0.1225 | up | 0.0 | 19.0 | 9.42 | 684.0 | b/exact |
| kopecky2019-mdf-climb | mdf | per_tooth_force_line | - | - | Fc1z = 49.123 hm + 5.936 N/tooth | 0.0408-0.1225 | down | 0.0 | 19.0 | 9.42 | 684.0 | b/exact |
| durkovic2017-oak-force-per-blade | hardwood | per_blade_force_power_law | - | - | Fb = 243.6 em^0.4761 N; kc 23.7-60.6 (derived, b 30) | 0.0216-0.1295 | up | 0.0 | - | 38.2 | 745.0 | b/exact |
| durkovic2017-krsljak-ke1-oak | hardwood | kc_textbook_base | - | - | 14 (57.99-140.63) | 0.0216-0.1295 | - | - | - | 38.2 | - | c/exact |
| curti2021-ksnorm-up-helix0 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | up | 0.0 | 25.0 | 3.14 | - | b/exact |
| curti2021-ksnorm-up-helix15 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | up | 15.0 | 25.0 | 3.14 | - | b/exact |
| curti2021-ksnorm-up-helix30 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | up | 30.0 | 25.0 | 3.14 | - | b/exact |
| curti2021-ksnorm-down-helix0 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | down | 0.0 | 25.0 | 3.14 | - | b/exact |
| curti2021-ksnorm-down-helix15 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | down | 15.0 | 25.0 | 3.14 | - | b/exact |
| curti2021-ksnorm-down-helix30 | solid_wood | density_normalised_ks_int_model | - | - | model (T4) | 0.04-0.1 | down | 30.0 | 25.0 | 3.14 | - | b/exact |
| goli2023-plywood-poplar-up-helix0-figread | plywood_poplar | affine_ks_int_figure_read | 23-43 | 3-4.9 |  | 0.04-0.1 | up | 0.0 | 25.0 | 3.14 | 430.0 | c/derived |
| goli2023-plywood-poplar-down-helix0-figread | plywood_poplar | affine_ks_int_figure_read | 24-43 | 2.6-5.3 |  | 0.04-0.1 | down | 0.0 | 25.0 | 3.14 | 430.0 | c/derived |
| goli2023-plywood-poplar-up-helix30-figread | plywood_poplar | affine_ks_int_figure_read | 20-26.5 | -0.35--0.05 |  | 0.04-0.1 | up | 30.0 | 25.0 | 3.14 | 430.0 | c/derived |
| goli2023-plywood-poplar-down-helix30-figread | plywood_poplar | affine_ks_int_figure_read | 24-29.5 | -0.35--0.25 |  | 0.04-0.1 | down | 30.0 | 25.0 | 3.14 | 430.0 | c/derived |
| goli2023-mdf-up-helix0-figread | mdf | affine_ks_int_figure_read | 28-30 | 2.9-3.1 |  | 0.04-0.1 | up | 0.0 | 25.0 | 3.14 | 720.0 | c/derived |
| goli2023-particleboard-up-helix0-figread | particleboard | affine_ks_int_figure_read | 12-23 | 2.6-3.3 |  | 0.04-0.1 | up | 0.0 | 25.0 | 3.14 | 737.8 | c/derived |

### 1.2 The engine today, on the common measure

#### T1. Engine constants (source lines)

| constant | file:line |
|---|---|
| MILLING_KC_FACTOR 2.7 | crates/rs_cam_core/src/material/mod.rs:899 |
| GenericSoftwood 6.5 x 2.7 | crates/rs_cam_core/src/material/mod.rs:965 |
| GenericHardwood 13.0 x 2.7 | crates/rs_cam_core/src/material/mod.rs:972 |
| Plywood 8.0 / 13.0 / 11.0 | crates/rs_cam_core/src/material/mod.rs:998-1002 |
| SheetGood Mdf 31.4 | crates/rs_cam_core/src/material/mod.rs:1015 |
| SheetGood Particleboard 35.0 | crates/rs_cam_core/src/material/mod.rs:1025 |
| Plastic Hdpe 40.0 | crates/rs_cam_core/src/material/mod.rs:1035 |
| janka_to_kc = janka/100 | crates/rs_cam_core/src/material/mod.rs:629-637 |
| LIT_KS 49.95 | crates/rs_cam_core/src/feeds/force.rs:62 |
| LIT_FEDGE 5.30 | crates/rs_cam_core/src/feeds/force.rs:66 |
| LIT_ANCHOR_KC 35.1 | crates/rs_cam_core/src/feeds/force.rs:77 |
| GRAIN_ANISOTROPY_FACTOR 2.0 | crates/rs_cam_core/src/tool_load/power.rs:95 |

#### T2. Engine lines on the common measure (derived from the constants)

| material | 'Kc' | Ks | F_edge | kc_eq h0.05 | kc_eq h0.10 | x2.0 grain h0.05 | x2.0 grain h0.10 |
|---|---|---|---|---|---|---|---|
| GenericSoftwood | 17.6 | 25.0 | 2.65 | 78.0 | 51.5 | 155.9 | 102.9 |
| GenericHardwood | 35.1 | 50.0 | 5.30 | 155.9 | 102.9 | 311.9 | 205.9 |
| SheetGood::Mdf | 31.4 | 44.7 | 4.74 | 139.5 | 92.1 | 279.0 | 184.2 |
| SheetGood::Particleboard | 35.0 | 49.8 | 5.28 | 155.5 | 102.7 | 311.0 | 205.3 |
| Plywood::BalticBirch (matrix plywood) | 13.0 | 18.5 | 1.96 | 57.8 | 38.1 | 115.5 | 76.3 |
| Plywood::HardwoodFaced | 11.0 | 15.7 | 1.66 | 48.9 | 32.3 | 97.7 | 64.5 |
| Plywood::Softwood | 8.0 | 11.4 | 1.21 | 35.5 | 23.5 | 71.1 | 46.9 |
| Plastic::Hdpe | 40.0 | 56.9 | 6.04 | 177.7 | 117.3 | 355.4 | 234.6 |

### 1.3 The solid-wood density law (Curti 2021, Table 5)

Curti 2021 prints one law for five species (287–1080 kg/m³):
`Fc = (Ks_norm(GA) · h + Int_norm(GA)) · Ap · ρ`, one quadratic per milling
mode and helix. Evaluated per unit density (derived):

#### T4a. Curti 2021 Table 5 per unit density (derived evaluation, GA 0 = along the grain)

| mode | helix | Ks_n(0) | Ks_n(90) | Ks_n max (GA) | Ks_n mean 0-180 | Ks_n max/min | Int_n(0) | Int_n mean |
|---|---|---|---|---|---|---|---|---|
| up | 0 | 0.0260 | 0.0755 | 0.0760 (100) | 0.0618 | 2.92 | 0.00550 | 0.00532 |
| up | 15 | 0.0320 | 0.0626 | 0.0626 (87) | 0.0517 | 2.21 | 0.00020 | -0.00073 |
| up | 30 | 0.0210 | 0.0417 | 0.0418 (83) | 0.0335 | 3.03 | -0.00006 | -0.00020 |
| down | 0 | 0.0560 | 0.0767 | 0.0768 (83) | 0.0685 | 1.57 | 0.00470 | 0.00505 |
| down | 15 | 0.0490 | 0.0697 | 0.0698 (83) | 0.0615 | 1.67 | -0.00040 | -0.00065 |
| down | 30 | 0.0300 | 0.0498 | 0.0500 (100) | 0.0443 | 1.67 | -0.00040 | -0.00071 |

At the five printed species densities (up-milling, helix 0):

#### T4b. Curti law at the five printed species densities (up-milling, helix 0; derived)

| species | rho | Ks GA0 | Ks GA90 | Ks max | Ks mean | Int GA0 | kc_eq0.05 GA0 | kc_eq0.05 max | kc_eq0.10 GA0 | kc_eq0.10 max | NRMSE % up 0 (Curti Table 6, printed) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| paulownia | 287.1 | 7.5 | 21.7 | 21.8 | 17.8 | 1.58 | 39.0 | 53.4 | 23.3 | 37.6 | 22.34 |
| lime | 585.7 | 15.2 | 44.2 | 44.5 | 36.2 | 3.22 | 79.7 | 108.9 | 47.4 | 76.7 | 14.38 |
| maple | 623.9 | 16.2 | 47.1 | 47.4 | 38.6 | 3.43 | 84.9 | 116.0 | 50.5 | 81.7 | 8.1 |
| oak | 737.8 | 19.2 | 55.7 | 56.1 | 45.6 | 4.06 | 100.3 | 137.2 | 59.8 | 96.7 | 11.52 |
| azobe | 1079.5 | 28.1 | 81.5 | 82.0 | 66.8 | 5.94 | 146.8 | 200.8 | 87.4 | 141.4 | 30.29 |

What it shows:

- Ks and Int scale linearly with density by construction. So the law's
  hardwood/softwood ratio is the density ratio and nothing else.
- The grain angle moves Ks by 1.6–3.0 times (max/min over 0–180°) inside
  one mode and helix. The lime-to-azobe density step is 1.84 times, so the grain effect is
  of the same size.
- The law fits its own five species with NRMSE 8.1–37.8 % (Table 6,
  printed). Paulownia and azobe, the two density ends, hold 9 of the 10
  values above 20 %. **No species in the law is a softwood.** Paulownia (287 kg/m³)
  is the only low-density point.

### 1.4 The density proxy against the composites

The question: does Curti's solid-wood law, evaluated at the composite's
density, predict the composite's own Ks? Goli 2023 used the same tools and
rig as Curti, so that comparison has one method on both sides.

#### T5. Density proxy test: Curti's solid-wood law at the composite's density vs the composite's own Ks (derived)

| material (source) | rho | mode helix | measured Ks | Curti Ks range 0-180 | Curti Ks mean | measured mid / Curti mean | measured vs Curti range |
|---|---|---|---|---|---|---|---|
| MDF, Goli 2018 (D80 1 blade, printed) | 711.0 | up 0 | 31.44 (25.81-35.58) | 18.5-54.0 | 44.0 | 0.70 | inside |
| MDF, Goli 2023 (same rig as Curti, read) | 720.0 | up 0 | 28-30 | 18.7-54.7 | 44.5 | 0.65 | inside |
| PB, Goli 2023 (same rig, read) | 737.8 | up 0 | 12-23 | 19.2-56.1 | 45.6 | 0.38 | below bottom |
| poplar plywood up h0 (read) | 430.0 | up 0 | 23-43 | 11.2-32.7 | 26.6 | 1.24 | above top |
| poplar plywood down h0 (read) | 430.0 | down 0 | 24-43 | 21.0-33.0 | 29.5 | 1.14 | above top |
| poplar plywood up h30 (read) | 430.0 | up 30 | 20-26.5 | 5.9-18.0 | 14.4 | 1.61 | above top |
| poplar plywood down h30 (read) | 430.0 | down 30 | 24-29.5 | 12.9-21.5 | 19.1 | 1.40 | above top |
| beech LVL across grain (printed) | 724.0 | up 0 | 32.29 | 18.8-55.0 | 44.8 | 0.72 | inside |
| beech LVL along grain (printed) | 724.0 | up 0 | 11.73 | 18.8-55.0 | 44.8 | 0.26 | below bottom |

Note: the Goli 2018 rows use a D80 single-blade cutterhead at 12.6 m/s; Curti and Goli 2023 use the same D20 2-flute tools at 3.14 m/s. Goli 2018 uses 0 deg = across the grain; Curti uses 0 deg = along.

What it shows:

- **MDF**: inside the solid-wood range, at 0.65–0.70 of the angle-mean.
  Two MDF measurements (Goli 2018 printed, Goli 2023 read) agree within
  about 10 % on Ks (31.4 against 28–30) with different tools.
- **Particleboard**: below the solid-wood range at the same density
  (Ks 12–23 against 19–56). The density proxy over-predicts it by about
  2.6 times at the angle-mean.
- **Poplar plywood** (430 kg/m³): above the top of the range for all four
  mode/helix pairs, by 1.1–1.6 times at the mid-point. The density proxy
  under-predicts it.
- **Beech LVL** (printed, other tool): across the grain it is inside;
  along the grain it is below the range.

So the density proxy does **not** predict the composites. The error has a
different sign per product (MDF and particleboard high, plywood low). A
density law is a solid-wood law only.

### 1.5 All values on one measure

#### T6. Every value on the common measure kc_eq(h) = Ks + Int/h, N/mm2 (derived; helix 0 or straight blade only)

| material, source | basis | kc_eq h 0.05 | kc_eq h 0.10 |
|---|---|---|---|
| MDF Goli 2018 | printed Ks/Int | 98.6 | 65.0 |
| PTFE Goli 2018 | printed Ks/Int | 74.5 | 47.4 |
| beech LVL across | printed Ks/Int | 185.5 | 108.9 |
| beech LVL along | printed Ks/Int | 85.7 | 48.7 |
| MDF Goli 2023 | figure read | 86-92 | 57-61 |
| PB Goli 2023 | figure read | 64-89 | 38-56 |
| poplar plywood up h0 | figure read | 83-141 | 53-92 |
| Curti @ lime 585.7 | density law, GA 0 to max | 80-110 | 47-77 |
| Curti @ oak 737.8 | density law, GA 0 to max | 100-139 | 60-97 |
| PB Palubicki 40 m/s | printed total kc, mean over h to ~0.31 | 32 (not at 0.05) | - |
| PB Palubicki 60 m/s | printed total kc, mean over h to ~0.31 | 37.6 (not at 0.05) | - |
| PB Goli 2023 at h 0.155 (outside 0.04-0.10) | figure read, consistency check only | 29-44 (at h 0.155) | - |
| PB Goli 2023 at h 0.31 (outside 0.04-0.10) | figure read, consistency check only | 20-34 (at h 0.31) | - |
| oak Durkovic 2017 | motor power, b 30 assumed, 38 m/s | 39.0 | 27.1 |
| MDF Kopecky 2019 per-mm reading (engine) | per-tooth line, b not printed | 156.0 | 103.0 |
| MDF Kopecky 2019 per-18-mm reading | per-tooth line, b not printed | 8.7 | 5.7 |

What it shows:

- At h = 0.05 mm the intercept term (Int / h) is larger than Ks for every
  helix-0 router measurement. On helix 15 and 30 the intercept is near
  zero or negative (T4a; Goli 2023 plywood helix 30), so this is not true
  there. The edge term, not the slope, sets most of the force
  at finishing chip thickness.
- The three router-rig sources (Goli 2018, Goli 2023, Curti) agree to
  within about 1.5 times on kc_eq for MDF and mid-density solid wood
  (80–140 N/mm² at h 0.05).
- **Đurković 2017 oak disagrees**: kc_eq 39 at h 0.05 against Curti's
  100–139 at the oak density, a factor of 2.6–3.5. That source is a
  4-blade cutterhead at 38 m/s, its force comes from motor power, and its
  chip width (30 mm) is an assumption. It is not a like-for-like witness.
- Pałubicki's particleboard total kc (32.0 / 37.6, averaged over h up to
  about 0.31 mm, 40 / 60 m/s) agrees with the Goli 2023 particleboard line
  only when that line is taken out of its h range to h 0.155–0.31
  (29–44 and 20–34). That is a consistency check, not a claim.
- **Kopecký 2019 has two readings and this file does not choose.** The
  per-mm reading (the engine's line) gives 156 at h 0.05; the per-18-mm
  reading gives 8.7. The first is 1.6 times the other MDF measurements; the
  second is about 10 times below them.

### 1.6 Ratios per source

#### T7. Ratios per source (derived)

| source | ratio | value | note |
|---|---|---|---|
| Goli 2023 (one rig, read) | plywood / MDF (Ks mid) | 1.14 | one lab, figure reads |
| Goli 2023 (one rig, read) | PB / MDF (Ks mid) | 0.60 | one lab, figure reads |
| Goli 2018 (printed) | MDF / beech LVL along | 2.68 | LVL is not plywood |
| Goli 2018 (printed) | MDF / beech LVL across | 0.97 |  |
| Goli 2018 (printed) | PTFE / MDF | 0.65 |  |
| Curti law (printed model) | hardwood / softwood | = rho_hw / rho_sw | linear in rho by construction |
| Curti law | oak / paulownia (printed rho) | 2.57 | both terms |
| Curti law | oak / lime (printed rho) | 1.26 |  |
| cross-source: Goli 2023 / Curti @450 mean | MDF / softwood(450) | 1.04 | rho 450 is an assumption |
| cross-source: Goli 2023 / Curti @450 mean | plywood / softwood(450) | 1.19 | rho 450 is an assumption |
| cross-source: Curti @737.8 / @450 | hardwood(oak) / softwood(450) | 1.64 | rho 450 is an assumption |
| engine | MDF / softwood | 1.79 | 31.4 / 17.55 |
| engine | plywood (BalticBirch) / softwood | 0.74 | 13.0 / 17.55 |
| engine | plywood (BalticBirch) / MDF | 0.41 |  |
| engine | PB / MDF | 1.11 |  |
| engine | hardwood / softwood | 2.00 | 13.0 / 6.5 (FPL shear) |

**There is no measured softwood row in any source.** Every ratio with
"softwood" in it uses Curti's law at an assumed density of 450 kg/m³. No
stored source prints that density. It is an illustration only.

### 1.7 The engine against the measurements

#### T8. Engine vs measured at h = 0.05 mm on the common measure (derived; helix 0 only)

| engine material | engine kc_eq | engine x2.0 grain | measured (source) | measured kc_eq | engine / measured (no grain factor) |
|---|---|---|---|---|---|
| SheetGood::Mdf | 140 | 279 | Goli 2018 MDF | 99 | 1.41 |
| GenericHardwood | 156 | 312 | Curti oak GA0..max | 100-139 | 1.13-1.55 |
| GenericSoftwood | 78 | 156 | Curti @450 GA0..max | 61-85 | 0.92-1.27 |
| Plywood::BalticBirch | 58 | 116 | Goli 2023 poplar plywood (read) | 83-141 | 0.41-0.70 |
| SheetGood::Particleboard | 156 | 311 | Goli 2023 PB (read) | 64-89 | 1.75-2.43 |

#### T8b. Helix effect: Curti kc_eq(0.05) over GA 0-180 per mode and helix vs the engine (derived; the engine has no helix input)

| rho | mode helix | Curti kc_eq h0.05 | engine kc_eq (no grain factor) | engine / Curti |
|---|---|---|---|---|
| 450 (assumed softwood) | up 0 | 56-84 | GenericSoftwood 78 | 0.9-1.4 |
| 450 (assumed softwood) | up 15 | 6-20 | GenericSoftwood 78 | 3.8-14.2 |
| 450 (assumed softwood) | up 30 | 9-15 | GenericSoftwood 78 | 5.1-8.8 |
| 450 (assumed softwood) | down 0 | 42-89 | GenericSoftwood 78 | 0.9-1.9 |
| 450 (assumed softwood) | down 15 | 12-26 | GenericSoftwood 78 | 3.1-6.7 |
| 450 (assumed softwood) | down 30 | 10-15 | GenericSoftwood 78 | 5.1-7.9 |
| 737.8 (oak, printed) | up 0 | 92-138 | GenericHardwood 156 | 1.1-1.7 |
| 737.8 (oak, printed) | up 15 | 9-33 | GenericHardwood 156 | 4.7-17.3 |
| 737.8 (oak, printed) | up 30 | 15-25 | GenericHardwood 156 | 6.2-10.7 |
| 737.8 (oak, printed) | down 0 | 68-145 | GenericHardwood 156 | 1.1-2.3 |
| 737.8 (oak, printed) | down 15 | 19-42 | GenericHardwood 156 | 3.7-8.2 |
| 737.8 (oak, printed) | down 30 | 16-25 | GenericHardwood 156 | 6.2-9.6 |

What T8 and T8b show (all at h 0.05, without the 2.0 grain factor):

- **MDF**: the engine is 1.41 times the printed Goli 2018 line (D2: the
  engine scales an affine slope a second time).
- **Hardwood**: the engine is 1.13–1.55 times Curti at oak density. The
  engine's line sits at or above the across-grain end.
- **Softwood**: the engine is inside Curti's range at 450 kg/m³ (only
  with that assumed density).
- **Plywood (Baltic birch)**: the engine is 0.41–0.70 of the only plywood
  measurement (poplar plywood, a lower density). The engine's plywood
  values are raw shear strengths without the 2.7 factor (D6).
- **Particleboard**: the engine is 1.75–2.43 times the Goli 2023 read
  (D3: a total kc used as a slope, so the edge term counts twice).
- **Helix**: T8 is helix 0 only. On Curti's helix 15 and 30 tools the
  intercept is near zero or negative, and kc_eq at h 0.05 falls to about
  6–42 N/mm². The engine has no helix input (`feeds/force.rs` has no helix
  term; `tool_load/power.rs:537` prints "no helix/grain decomposition").
  So at helix 30 the engine is about 5–11 times the Curti law for both
  softwood (assumed 450) and oak. Two limits apply. Curti's Ks is the
  cutting-direction component only; at helix 30 the back force is 25–45 %
  of it (Table 4, printed), so the resultant is larger. A negative
  intercept is a fit artefact inside h 0.04–0.10 mm, not a physical force,
  and kc_eq must not be taken below that range.
- The engine then multiplies every line by 2.0 for grain (D7). For MDF and
  particleboard, which Goli 2023 prints as isotropic, that doubles a number
  that is already high.

### 1.8 What the data does NOT show

- No value below h = 0.04 mm from any router study. Most finish chiploads
  are below that. The size effect there is unmeasured here.
- No router-speed data above about 13 m/s, except the industrial
  cutterheads (Pałubicki 40–60 m/s, Đurković 38 m/s).
- No softwood species. No Baltic birch or softwood plywood. No HDF.
- Helix 15/30 data: Curti Table 5 (four quadratics) and two Goli 2023
  figure reads, one lab. The Fig 5 intercept axis carries the label
  N/mm², and the read error (±0.2) is as large as the intercept.
- Rake 25° only for the router studies. Goli 2018, Goli 2023 and Curti are
  one laboratory (LaBoMaP / Florence). Their agreement is not independent.
- No Kc for acrylic, polycarbonate, POM or any plastic the engine lists.
  HDPE has a yield stress only.
- The simulation is not a second witness for Kc. It consumes the same
  constant.

## 2. Sources

All 24 candidate rows got the verdict "confirmed". No row was wrong, not
found or wrongly graded. The reconciler added the inferred items the
verifiers named to `derived_fields` (Curti unit, Kopecký flute count,
Đurković helix, Yang rake pairing, Pałubicki helix and approximate h max)
and corrected "18" to "19" grain positions in the Goli 2023 notes.
Grades: 17 `b`/exact, 1 `c`/exact, 6 `c`/derived.

The manifest hash is of the downloaded PDF or JATS XML, not of the stored
text. T9 gives the text hashes.

#### T9. sha256 of the stored text copies (the manifest hashes the PDF/XML)

| stored text | sha256 (first 16) |
|---|---|
| fpl_gtr190_wood_handbook_2010_excerpt.txt | d1db159dc74769af... |
| goli2023_iwms25_ewp_ks.txt | bb7e57063e2dc606... |
| marcon2021_generalized_force_model.txt | 7d9318739a1d6c8f... |
| palubicki2021_particleboard.txt | 004ec8b67e8346f9... |
| pmc6315737_round_shape_ks.txt | e65b7446d8448a99... |
| porankiewicz2007_lowdensity.txt | 14eeeeb8a536a387... |
| woodresearch_201702_11.txt | 46eca4d366f3d81c... |
| woodresearch_201905_12.txt | 951fe0bba0f9d335... |
| yang2022_hdpe.txt | 7f079fa0e097e639... |

 The two SAM ENSAM PDFs never re-hash: the
server stamps each download. Their `pdftotext -layout` output from a fresh
download is byte-identical to the stored text.

| Source id | Vendor | What it prints | Grade | URL reachable | Hash | Verdicts |
|---|---|---|---|---|---|---|
| `g7_goli2018_round_shape_ks` | peer reviewed (Materials 2018) | MDF Ks 31.44 / Int 3.36; PTFE 20.33 / 2.71; beech LVL by angle 6.61–51.04 / 3.33–7.66; per mm of edge | b | yes | matches (XML) | 4 confirmed |
| `g7_palubicki2021_particleboard` | peer reviewed (Materials 2021) | particleboard total kc 32.0 (40 m/s), 37.6 (60 m/s); no grain data | b | yes | matches (XML) | 2 confirmed |
| `g7_yang2022_hdpe` | peer reviewed (Polymers 2022) | HDPE yield stress 46.89 / 33.85 MPa, toughness 1.173 / 1.368 kJ/m²; not a Kc | b | yes | matches (XML) | 2 confirmed |
| `g7_kopecky2019_mdf_quasi_orthogonal` | peer reviewed (Wood Research 2019) | MDF Fc1z = 49.954 hm + 5.3047 (conv.), 49.123 hm + 5.936 (climb), N per tooth; chip width not printed | b | yes | matches (PDF) | 2 confirmed |
| `g7_durkovic2017_oak_peripheral` | peer reviewed (Wood Research 2017) | oak Fb = 243.6 em^0.4761 N, P = 1890.7 em^0.477 W; Krsljak Ke1 = 14, K 58–141 | b (Ke1: c) | yes | matches (PDF) | 2 confirmed |
| `g7_curti2021_generalized_wood_model` | peer reviewed (EJWWP 2021) | Table 5 density-normalised Ks/Int quadratics per mode and helix, 287–1080 kg/m³; Table 6 NRMSE 8.1–37.8 % | b | yes | no (download stamp); text identical | 6 confirmed |
| `g7_goli2023_iwms25_ewp` | conference (IWMS 2023) | particleboard, MDF, OSB, poplar plywood Ks/Int as plots only | c (figure reads) | yes | no (download stamp); text identical | 6 confirmed |
| `g7_porankiewicz2007_low_density` | peer reviewed (BioResources 2007) | multi-factor force regressions; no K value | — | yes | matches (PDF) | 0 rows |
| `g7_fpl_gtr190_wood_handbook_2010` | USDA FPL | no machining chapter; 0 hits for cutting energy/force/power | — | not verified | not verified | 0 rows (evidence for a refusal) |

Dead ends (from `fetch/G7/FETCH_NOTES.md`):

| Target | Result |
|---|---|
| Plywood Ks or kc as a printed number | only Goli 2023 plots; HAL copy behind a bot wall; no Baltic birch or softwood plywood data |
| Acrylic (PMMA) | Korkmaz 2017 HTTP 403; Yan 2020 has no force data |
| Polycarbonate | no primary study (as on 2026-05-29) |
| POM and other plastics | paywalled (Trifunović 2021, Chabbi 2017) or not published |
| USDA Wood Handbook specific cutting energy | no machining chapter (2010 edition, CARB mirror; FPL original HTTP 403) |
| Koch 1964, Kivimaa 1950 | books, not reachable |
| A printed shear-to-milling-Kc factor (the engine's 2.7) | not found |
| A printed Janka-to-Kc law (the engine's `janka/100`) | not found; Curti prints a density law instead |
| Aluminium 6061 kc1.1 / mc | Machining Doctor HTTP 403 |
| Aguilera 2020 (Maderas) | Cloudflare / HTTP 403 |
| Eyma 2004 | HTTP 403; HAL behind a bot wall |
| Goli 2023 on HAL | bot wall; the same paper came from SAM ENSAM |

## 3. For Phase 3

### 3.1 Candidate forms the trend supports

| Form | Evidence | Range it could claim | Second witness that could exist |
|---|---|---|---|
| A. Solid wood: Curti density law `Ks = Ks_norm(GA, mode, helix) · ρ`, `Int = Int_norm(…) · ρ`, replacing `2.7 × FPL shear` and `janka / 100` | Curti Table 5 (printed, grade b) | ρ 287–1080 kg/m³, h 0.04–0.10 mm, helix 0/15/30 as an input that the card names (T8b: helix moves kc_eq 5–10×), rake 25°, NRMSE 8–38 % | Đurković oak (weak: disagrees 2.6–3.5×, other method); a new literature cell for a softwood; the paper's own Table 6 residual |
| B. MDF: direct affine line Ks 31.44, Int 3.36 (SD 2.68 / 0.27) | Goli 2018 (printed) | h 0.041–0.091 mm, rake 25°, isotropic (no grain factor) | Goli 2023 read (28–30 / 2.9–3.1; same lab); Kopecký only if its chip width is resolved |
| C. Particleboard: affine line from the Goli 2023 read, or refuse | Goli 2023 read (grade c) | h 0.04–0.10 mm | Pałubicki total kc (agrees only after an h step outside the range) |
| D. Plywood: one-witness claim from the poplar plywood read, or refuse | Goli 2023 read (grade c), 430 kg/m³ | poplar plywood only, h 0.04–0.10 | none found; the density proxy fails on plywood (§1.4), so it cannot be a witness |
| E. Grain factor per family, not global | Goli 2018 ("almost two times"), Curti max/min 1.34–3.73 | solid wood and LVL only; 1.0 for MDF and particleboard | Goli 2023 prints MDF and particleboard as isotropic |

The force anchor (`LIT_KS`, `LIT_FEDGE`) must change under any of these
forms. It is an MDF fit with an unknown chip width, attached to hardwood
(D1). Forms A and B print force per mm of edge directly and need no anchor.

### 3.2 Cells that stay refused

- Every plastic except PTFE (which has no engine family). No printed Kc.
- HDPE: the shipped 40.0 is a yield stress. Refuse, or a ruling that states
  a proxy rule on the card.
- Baltic birch and softwood plywood beyond a one-witness claim. No data.
- HDF: no measurement (the engine's 36.8 is MDF × a density ratio).
- Any chip thickness below 0.04 mm on a force or power claim that the
  card states as measured.
- `SolidWoodByJanka` outside 287–1080 kg/m³ under form A (a density input
  replaces Janka; the Janka-to-density step is itself unsourced here).

### 3.3 Recommendation per sub-class

| Sub-class | Recommendation | Why |
|---|---|---|
| Solid wood | **per-family** (form A) | one printed law, one lab; the only other witness disagrees; no softwood species |
| MDF | **per-family** (form B) | two measurements agree within about 10 %; same lab |
| Particleboard | **per-family, one witness** or refuse, by ruling | figure read only on the router rig; Pałubicki is another quantity and speed |
| Plywood | **refuse** or one-witness poplar claim, by ruling | figure read only; no Baltic birch; the density proxy fails |
| Plastics | **refuse** | no printed Kc; HDPE's number is not a cutting force |
| Generic (density proxy for all materials) | **reject** | it over-predicts MDF and particleboard and under-predicts plywood (§1.4) |

The biggest open gap is not a Kc. It is the 252 finish cells with no power
(§0.1) and the chip thickness below 0.04 mm where most of those finish
cuts run. No source here measures either.

## 5. The landing (B6, 2026-09-25)

Plan: `B6_PLAN.md`. Branch `b6-force-line` (work tree `../rs_cam_b6`, from
9d547298). A separate tree keeps this FM1 diff free of the uncommitted work
of the other sessions.

### 5.1 What changed

- `Material::force_line() -> Result<ForceLine, ForceLineRefusal>`
  (`material/force_line.rs`) replaces the scalar `Kc`. Every consumer reads
  the one line: `feeds::force` (deflection), `feeds::efficiency`,
  `feeds::predict`, `tool_load::power`, `tool_load::deflection`, the
  distribution, the Suggest power ladder (step 6, step 7) and the modulator.
- Removed (breaking, no shim): `kc_n_per_mm2`, `affine_coefficients`,
  `affine_coefficients_for_kc`, the `LIT_*` anchor (49.95 / 5.30 / 35.1),
  `MILLING_KC_FACTOR` (2.7), `janka_to_kc_n_per_mm2`, `KC_FOLKLORE_*`,
  `KcProvenance`, `GRAIN_ANISOTROPY_FACTOR` (2.0),
  `DeflectionUnmodeled::MaterialCustom` and `Material::Custom.kc`.
- **Solid wood:** the Curti 2021 density law, helix 0, the upper envelope
  over grain angle 0-180° and up/down milling, per term: Ks_n 0.0768333,
  Int_n 0.0060333 (computed from the printed quadratics). `Ks = Ks_n · ρ`,
  `F_edge = Int_n · ρ`, grain factor 1.0 (the envelope holds the grain
  spread). Valid ρ 287-1080 kg/m³; outside, the line refuses.
- **Density:** `ρ = SG × 1000 × 1.12` from FPL Table 5-3a (12 % MC). The
  library rows carry `specific_gravity_12` (97 FPL rows; Honeylocust prints
  "—" and refuses). By the operator's ruling of 2026-09-25 the three
  species with no 5-3a row were fetched inside B6
  (`fetch/G7/density_rows.json`, `scripts/g7_density_check.py`): FPL
  Table 5-5a prints basic SG (ovendry weight, green volume), and FPL Ch.4
  Eq. (4-11) `Gx = Gb / [1 − 0.265 Gb (1 − x/MCfs)]`, MCfs 30 %, converts
  it to 12 % MC in code:

  | Species | Gb (5-5a) | G12 | ρ12 kg/m³ | Wood Database (grade c) | Result |
  |---|---|---|---|---|---|
  | Radiata pine | 0.42 | 0.4501 | 504.1 | 510 (0.99) | ships |
  | Jarrah | 0.67 | 0.7499 | 839.9 | 840 (1.00) | ships |
  | Ipe | 0.92 | 1.0776 | 1207.0 | 1050 (1.15) | refuses: above 1080 |

  Eq. (4-11) reproduces FPL's own white-ash example (0.55 → 0.603 against
  the printed 0.605).
- **MDF:** the Goli 2018 line direct, Ks 31.44, F_edge 3.36, grain 1.0.
- **Refused with a named reason:** plywood (all grades), particleboard,
  HDF, every plastic (HDPE included), aluminium, foam, fiberglass, Custom,
  Ipe, and the Wood Database library rows (no SG).
- **Chip range:** a mean chip outside the printed range does not refuse.
  `ForceAtPoint.chip_regime` says `below_measured_range` or
  `above_measured_range`, and the card states the extrapolation.
- **Card and MCP:** the power row paints `Force line: …` (or the refusal
  headline) and the extrapolation line; the hover gives every constant.
  The stock panel "Kc:" row is now "Force line:". MCP `basis.force_line`
  carries the form, the source, Ks, F_edge, the density and its source,
  and `evaluated_at`.
- **Diagnostics:** an Unmodeled power or deflection gate now reports
  `load.power.unmodeled` / `load.deflection.unmodeled`, not the `*.within`
  id (as the chipload and depth gates already did). B6 made every
  plywood job hit this; before, a reader of ids alone read "within".

### 5.2 FM1 moves (960 cells; HEAD CSV against the B6 run)

No cell moves between ok and refused (534 ok, 426 refused). Ratios are new
/ old, median (range).

| Column | softwood | hardwood | mdf | plywood_hardwood |
|---|---|---|---|---|
| `force_n` (534 cells) | 1.04 (0.94-1.36) | 0.85 (0.78-1.13) | 0.71 | 95 cells empty |
| `power_kw` (188 cells) | 0.53 (0.49-0.66) | 0.43 (0.41-0.56) | 0.35 (0.33-0.36) | 36 cells empty |
| `stepover_mm` | 48 cells, ±1 % | 48 cells, ±1 % | 4 cells, −0.1 % | 36 cells, 1.17 (1.11-1.28) |
| `depth_per_pass_mm` | 19 cells | 17 cells | 6 cells, ≤ 0.1 % | 36 cells, 1.21 (1.03-1.50) |
| `feed_mm_min` | none | none | none | 2 cells, −0.4 % and −1.2 % |
| `force_line` (new) | curti2021 (240) | curti2021 (240) | goli2018 (240) | refused:Plywood (240) |

- The solid-wood depth moves are small (adaptive and Adaptive3d < 1.5 %;
  a small-tool pass staircase snap such as 0.375 → 0.4286), except the
  tapered Ø3.175 Adaptive cell, 2.25 → 3.0 mm, where the axial envelope cap
  widens 5.04 → 5.32 mm. Profile and Waterline depths fall 0.3-4 %: the
  Curti Ks is above the old slope (hardwood 51.9 against 50.0), so the
  force rises at a thick chip.
- Plywood: with no force line the dial uses its section proxy, so depth
  and stepover rise (decision 7), and Suggest adds
  `DeflectionBackoffUnmodeled` (37 cells).
- No cell is power-limited before or after. The power ceiling binds on no
  shipped fixture (`power_ceiling_parity_f2`, renamed
  `the_power_ceiling_binds_on_no_shipped_fixture_b6`: peak 34.9 %, VFD /
  WhiteOak / Ø12). The card's power-bar spread is min 0.2 %, median 3.2 %,
  peak 31.5 %. The operator ruled (2026-09-25) that a peak under half
  scale is correct; the half-scale assert in
  `the_power_bar_is_informative_g_chipverdict` is gone.
- Sim CSV (38 rows): `power_peak_kw` ×0.35-0.6; `deflection_peak_mm` MDF
  ×0.71, hardwood about ×0.9, softwood about ×1.1. The 6 plywood rows go
  from `Within` to `Unmodeled` (MaterialUnvalidated) on power and
  deflection. The BullNose plywood pocket chipload goes `Unmodeled` →
  `Within` at its new 1.0 mm depth.
- **The sim budget skips one more row.** The TaperedBallNose DropCutter
  softwood row is skipped on the 150 s wall-clock budget of the sim subset
  (the hardwood row was already skipped at HEAD). The DropCutter rows ran
  about 15 % slower in all three B6 runs, on a machine at load average
  8-11. The force line adds a closed-form evaluation per sample, not a
  loop, but this record does not separate load from code: a HEAD-against-B6
  timing run on a quiet machine does that.

### 5.3 Tests

- New sentries: `the_force_line_is_printed_per_family_b6` (core),
  `the_power_row_names_its_force_line_b6` (viz), and
  `an_unmodeled_power_or_deflection_gate_is_not_reported_as_within_b6`
  (in `finish_depth_is_reported_not_capped_fm6`).
- Re-pinned with the cause in the test: the power-ceiling fixtures (T15,
  S2, powerstale, ladder, f2, f039, axialunits), the specific-energy table
  (`cut_efficiency_is_closed_form_g_specenergy`: 146.8 / 96.2 / 82.7
  J/mm³ on the B6 line; ADVICE.md §1 was measured on the retired anchor),
  the hard-maple deflection fixture in `feeds/suggest/tests.rs` (stickout
  99 → 111 mm), the A3 pocket fingerprint (stepover 1.515 → 1.525), and the
  g_visible weak-spindle case (Baltic birch → hardwood).
- Not run (ask first): `wanaka_suggest_integration`.

### 5.4 Open

- A typed tool helix that selects the printed Curti helix row (0/15/30).
  Today the engine has no helix input, and helix 0 is the largest force.
- A user-typed line for Custom; a Kienzle form for aluminium.
- Plywood, particleboard, HDF: a printed per-edge line (none fetched).
