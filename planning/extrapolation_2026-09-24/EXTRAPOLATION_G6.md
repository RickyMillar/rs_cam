# G6: Drill (any wood drill chipload or plunge rate)

Status: LANDED under ruling B5 (2026-09-24): steps 1-2 in `7eef9ffa`, step 3 after it; see §5. Phase 2 (trend) done 2026-09-24.

Inputs: the LUT in the working tree on 67e98529 (`crates/rs_cam_core/data/vendor_lut/observations/*.json`,
433 rows; 389 committed plus an uncommitted peer file `carbide3d_shapeoko.json`
with 44 rows, which no G6 table uses), `fetch/G6/verified_rows.json` (this reconciler's output),
`inventory_cells.csv` (Phase 0) and the engine constants in the Rust source.

Scripts (read-only on the LUT, no cargo):

- `scripts/g6_verified_rows.py` writes `fetch/G6/verified_rows.json` from
  `fetch/G6/candidate_rows.json` and the verifier verdicts.
- `scripts/trend_g6.py` prints every table in §1. It reads the engine
  constants (`k0`, `p`, `q`, `DRILL_CHIPLOAD_MULTIPLIER`, the RPM tiers,
  the plunge envelope, the peck bands) from the source with regular
  expressions, so a change in the code shows in the output.

Every number in §1 and §3 that is not a quoted chart value is **derived**
by that script. No vendor prints it.

## 0. The gap

### The cells

The Phase 0 inventory puts **80 refused cells** in G6. All 80 refuse.

| Axis | Counts |
|---|---|
| Tool | EndMill 16, BallNose 16, BullNose 16, TaperedBallNose 16, VBit 16 |
| Diameter | 3.175 and 6.0 mm (EndMill, BallNose, BullNose, TaperedBallNose); 6.35 and 12.7 mm (VBit) |
| Operation | Drill 40, AlignmentPinDrill 40 |
| Material | softwood 20, hardwood 20, mdf 20, plywood_hardwood 20 |

The cells are 5 tool kinds × 2 sizes × 2 operations × 4 materials.

**Correction to the fetch report.** The G6 fetch report and `fetch/G6/FETCH_NOTES.md` §2
say that the 80 cells are "EndMill 3.175 and 6.0 mm". That is wrong. Only 16
of the 80 cells are EndMill. The matrix has no drill tool kind. Every G6
cell is a router cutter on a drill operation.

### The current engine rule

The code was read at 67e98529.

| Rule | Function (file) | What it does |
|---|---|---|
| Refusal | `feeds::support::formula_backing` (`crates/rs_cam_core/src/feeds/support.rs:184`, arms at :197-204) | Returns `Clueless` for every `(ToolFamily, OperationFamily::Drill, _)`. The five reasons are `DRILL_FLAT`, `DRILL_BALL`, `DRILL_BULL`, `DRILL_TAPER` ("... the drill multiplier 2.5 is unsourced") and `DRILL_VBIT` ("... a V-bit cuts a cone, not a bore"). |
| No LUT row | `vendor_lookup` via `op_family_to_lut` | Maps Drill and AlignmentPinDrill to the LUT family `drill`. The LUT has 0 rows in that family, so every drill query misses (sentry `queryable_families_without_rows_are_a_stated_fact`, `feeds/vendor_lut.rs:869`). |
| Formula behind the refusal | `feeds::calculate` step 2 (`crates/rs_cam_core/src/feeds/mod.rs:1543`) | Drill chip load = `0.024 · D^0.61 · (600/J)^(0.4·1.26)` × `DRILL_CHIPLOAD_MULTIPLIER` 2.5. CREDITS.md "Drill-subsystem provenance" declares 2.5 unsourced. |
| RPM | `drill_rpm_envelope_for_diameter` (`feeds/mod.rs:1249`) | D ≤ 6 mm: 8000-14000; D ≤ 10: 6000-10000; D > 10: 4000-8000. Repo-authored. |
| Plunge feed | `Material::drill_plunge_feed_envelope_per_mm` (`crates/rs_cam_core/src/material/mod.rs:1284`) | Solid wood 50-400, plywood and sheet goods 40-350 mm/min per mm of D. Its own comment says "repo-authored ... not sourced". |
| Peck | `janka_to_drill_per_peck_max_dtd` (`material/mod.rs:710`), `Material::drill_per_peck_max_dtd` (:1211), `drill_default_peck_depth_mm` (:1248), `apply_drill_defaults` (`feeds/suggest/apply.rs:799`) | Per-peck maximum 6 × D (Janka ≤ 700), 5 × D (≤ 1500), 4 × D (> 1500); plywood and sheet 1.5 × D. Suggest uses half. `apply_drill_defaults` clamps the peck below the hole depth. Repo-authored. |

Drill and AlignmentPinDrill use the same `OperationFamily::Drill` in
`formula_backing`. The evidence below therefore applies to both operations
equally. The only difference is the peck clamp in `apply_drill_defaults`
(the pin drill clamps to stock thickness plus spoilboard penetration).

## 1. The trend

### 1.1 What the verified rows cover

31 rows passed verification (30 confirmed, 1 grade_wrong downgraded to
derived / c). 8 Leitz depth statements passed verification. No LUT row is a
drill row.

| Matrix tool kind | Verified rows | Diameters | Materials | Quantity |
|---|---|---|---|---|
| EndMill (flat end mill used as a drill) | 8 (Amana Spektra "Ramp Down") | 3.175, 6.0 | mdf, plywood_hardwood (the Wood/Plywood column) | axial chip load per tooth (derived) and plunge feed |
| BullNose | 4 (PreciseBits fret plane) | 3.175 only | softwood, hardwood by Janka band | plunge feed only, no RPM, no chip load |
| BallNose | 0 | - | - | - |
| TaperedBallNose | 0 | - | - | - |
| VBit | 0 | - | - | - |
| A real wood drill (no matrix tool kind) | 19 (Onsrud 4, Leitz 13, CMT 2) | 3-16 | softwood, hardwood, mdf, particleboard | chip load per lip and feed per revolution (derived) |

### 1.2 Wood drills: chip load per lip and feed per revolution against diameter

```
vendor  family            subfamily                material       D mm    Z   RPM  fz min  fz max f/rev min f/rev max
onsrud  brad_point_drill  72_000_series            hardwood       3       2  4500  0.2286  0.2794    0.4572    0.5588
onsrud  brad_point_drill  72_000_series            hardwood       5       2  4500  0.2794  0.3302    0.5588    0.6604
onsrud  brad_point_drill  72_000_series            hardwood       6       2  4500  0.3302  0.3810    0.6604    0.7620
onsrud  brad_point_drill  72_000_series            hardwood       8       2  4500  0.3810  0.4318    0.7620    0.8636
leitz   twist_drill       leitz_hs_twist           softwood       3-12    2  2500  0.2400  0.2400    0.4800    0.4800
leitz   twist_drill       leitz_hw_twist           softwood       6-16    2  4500  0.1667  0.1667    0.3334    0.3334
leitz   twist_drill       leitz_hw_marathon        softwood       -6      2  4500  0.3333  0.3333    0.6666    0.6666
leitz   twist_drill       leitz_hw_marathon        softwood       6-12    2  4500  0.5000  0.5000    1.0000    1.0000
leitz   twist_drill       leitz_hw_marathon        softwood       12-     2  4500  0.3889  0.3889    0.7778    0.7778
leitz   twist_drill       leitz_hw_vpoint          softwood       7-12    2  4500  0.1333  0.1333    0.2666    0.2666
leitz   twist_drill       leitz_hw_vpoint          softwood       6-12    2  4500  0.3889  0.3889    0.7778    0.7778
leitz   levin_drill       leitz_hs_levin           softwood       5-12    1  4500  0.3333  0.3333    0.3333    0.3333
leitz   levin_drill       leitz_hw_levin           softwood       12-16   1  4500  0.3333  0.3333    0.3333    0.3333
leitz   brad_point_drill  leitz_dowel_excellent    particleboard  3-10    2  4500  0.2222  0.2222    0.4444    0.4444
leitz   brad_point_drill  leitz_dowel_excellent    mdf            3-10    2  4500  0.1556  0.1556    0.3112    0.3112
leitz   brad_point_drill  leitz_dowel_excellent    softwood       3-10    2  4500  0.1556  0.1556    0.3112    0.3112
leitz   brad_point_drill  leitz_dowel_excellent    particleboard  3-10    2  4500  0.2889  0.2889    0.5778    0.5778
cmt     brad_point_drill  cmt_311_71_72_hwm        particleboard  5-10    2  6000  0.0833  0.3333    0.1666    0.6666
cmt     brad_point_drill  cmt_311_71_72_hwm        mdf            5-10    2  6000  0.0833  0.3333    0.1666    0.6666
```

| Material | Midpoint fz (mm/lip) | Feed per rev (mm/rev) |
|---|---|---|
| hardwood (Onsrud "Wood" only) | n=4, median 0.330, range 0.254-0.406 | n=4, median 0.660, range 0.508-0.813 |
| softwood (Leitz) | n=10, median 0.333, range 0.133-0.500 | n=10, median 0.407, range 0.267-1.000 |
| particleboard (Leitz, CMT) | n=3, median 0.222, range 0.208-0.289 | n=3, median 0.444, range 0.417-0.578 |
| mdf (Leitz, CMT) | n=2, median 0.182, range 0.156-0.208 | n=2, median 0.364, range 0.311-0.417 |
| all | n=19, median 0.289, range 0.133-0.500 | n=19, median 0.480, range 0.267-1.000 |

Size trend inside one vendor (derived):

| Vendor, tool line | Diameters | Trend |
|---|---|---|
| Onsrud 72-000 "Wood" | 3, 5, 6, 8 mm | fz ∝ D^0.485 (midpoints); D^0.527 (min), D^0.449 (max). This agrees with the 2026-08-04 audit. |
| Leitz HW Marathon | three printed D bands | ≤ 6: 0.333; 6-12: 0.500; > 12: 0.389. It rises, then falls. |
| Leitz dowel "Excellent" | one vf for D 3-10 | fz constant over D (slope 0 by construction). |
| CMT 311 HWM | one range for D 5-10 | no size split. |

Printed Leitz material factors on vf (same RPM, so they scale fz): dowel
"MDF, solid wood = 0.7", "Chipboard, uncoated = 1.3" on chipboard plastic
coated; HS twist "Hardwood = 0.7"; HW twist "Hardwood = 0.8"; laminated
veneer lumber 1.1-1.2; Levin "Drilling depth > 4 x D = 0.8".

### 1.3 The ratio the 2.5 multiplier claims

The engine says a drill chip load is 2.5 × the milling chip load. Three
comparisons test that claim. They must not be merged.

**(a) Wood drill against the same vendor's end mill.** Onsrud 72-000 "Wood"
against the Onsrud flat-end LUT rows at the nearest printed diameter
(within 1.25×). Chip loads are per tooth at each sheet's own RPM.
Caution: the 72-000 row is footnoted "Gang drills run at 4,500 RPM and
150 IPM". That is a rigid multi-spindle borer, not a router collet.

```
  drill D drill fz sheet     end-mill series              role        EM D   size diff EM fz  ratio
  3       0.2540   hardwood  60_100mw_compression_spiral  finish      3.175      +5.8% 0.2794  0.91
  3       0.2540   softwood  60_100mw_compression_spiral  finish      3.175      +5.8% 0.3048  0.83
  5       0.3048   hardwood  60_100mw_compression_spiral  finish      4.763      -4.7% 0.3302  0.92
  5       0.3048   softwood  60_100mw_compression_spiral  finish      4.763      -4.7% 0.3556  0.86
  6       0.3556   hardwood  60_100mw_compression_spiral  finish      6.350      +5.8% 0.3810  0.93
  6       0.3556   hardwood  60_200_downcut               finish      6.350      +5.8% 0.1524  2.33
  6       0.3556   softwood  60_200_downcut               finish      6.350      +5.8% 0.1524  2.33
  8       0.4064   hardwood  60_000hh_series              roughing    9.525     +19.1% 0.4064  1.00
  8       0.4064   hardwood  60_000lh_series              roughing    9.525     +19.1% 0.3556  1.14
  8       0.4064   hardwood  60_100mw_compression_spiral  finish      9.525     +19.1% 0.4318  0.94
  8       0.4064   hardwood  60_200_downcut               finish      9.525     +19.1% 0.1778  2.29
  8       0.4064   hardwood  60_800_series                roughing    9.525     +19.1% 0.4572  0.89
  8       0.4064   softwood  60_000hh_series              roughing    9.525     +19.1% 0.4318  0.94
  8       0.4064   softwood  60_000lh_series              roughing    9.525     +19.1% 0.3556  1.14
  8       0.4064   softwood  60_200_downcut               finish      9.525     +19.1% 0.1778  2.29
  8       0.4064   softwood  60_350_series                semi_finish 9.525     +19.1% 0.4572  0.89
  8       0.4064   softwood  60_800_series                roughing    9.525     +19.1% 0.4572  0.89
  ratio, all: n=17 median 0.941 range 0.833-2.333
```

The median is 0.94. The three values above 2 all come from the 60-200
downcut finish series, which prints a low chip load. Against every other
Onsrud series the ratio is 0.83-1.14.

**(b) End-mill plunge against the same tool's side chip load.** Amana
Spektra v24: same diameter, same Z, same 18,000 RPM, same material column.

| Z | D | Column | Ramp Down IPM (printed) | Axial fz (derived) | Side fz (LUT) | Ratio | 1/Z |
|---|---|---|---|---|---|---|---|
| 2 | 3.175 | MDF/Laminate | 90 | 0.0635 | 0.1270 | 0.500 | 0.500 |
| 2 | 3.175 | Wood/Plywood | 72.5 | 0.0512 | 0.1016 | 0.504 | 0.500 |
| 2 | 6 | MDF/Laminate | 107.5 | 0.0758 | 0.1524 | 0.497 | 0.500 |
| 2 | 6 | Wood/Plywood | 90 | 0.0635 | 0.1270 | 0.500 | 0.500 |
| 3 | 3.175 | MDF/Laminate | 90 | 0.0423 | 0.1270 | 0.333 | 0.333 |
| 3 | 3.175 | Wood/Plywood | 72 | 0.0339 | 0.1016 | 0.334 | 0.333 |
| 3 | 6 | MDF/Laminate | 109 | 0.0513 | 0.1524 | 0.337 | 0.333 |
| 3 | 6 | Wood/Plywood | 90 | 0.0423 | 0.1270 | 0.333 | 0.333 |

2-flute: 0.497-0.504 (n=4). 3-flute: 0.333-0.337 (n=4). The printed rule
"To find Ramp Down: Feed Rate IPM / # of flutes" makes the ratio exactly
1/Z. The small deviations are the vendor's rounding of the printed IPM.

**(c) Vendor figure against the engine's milling formula.** This is the
multiplier each figure implies: vendor fz / `0.024 · D^0.61 · (600/J)^0.504`
at the engine's Janka proxy (softwood 600, hardwood 1450, mdf 1100,
plywood_hardwood 1200, particleboard 750). Onsrud prints one "Wood" row, so
the script evaluates it against both softwood and hardwood. Rows with a
printed D range are evaluated at both ends.

| Group | Implied multiplier (band ends) |
|---|---|
| Wood drill, Onsrud 72-000 | n=16, median 6.38, range 4.36-9.29 (softwood 4.76-5.41 at midpoint, the 2026-08-04 audit figure; hardwood 7.4-8.5) |
| Wood drill, Leitz | n=24, median 3.31, range 1.22-6.98 |
| Wood drill, CMT | n=8, median 2.79, range 0.95-7.06 |
| Wood drill, all | n=48, median 4.59, range 0.95-9.29 |
| End-mill plunge, Amana Ramp Down | n=8, median 1.22, range 0.84-1.78 |

The engine's drill chip load today (formula × 2.5; the cells refuse, so no
card shows it):

| D | softwood | hardwood | mdf | plywood_hardwood |
|---|---|---|---|---|
| 3.175 | 0.1214 | 0.0778 | 0.0894 | 0.0856 |
| 6.0 | 0.1790 | 0.1147 | 0.1319 | 0.1262 |

The engine's drill fz against Amana's derived axial fz at the same D, Z and
column (softwood and hardwood read through the Wood/Plywood column):

```
    Z D      matrix material   column        engine fz Amana fz engine/Amana
    2 3.175  mdf               MDF/Laminate  0.0894    0.0635           1.41
    2 3.175  plywood_hardwood  Wood/Plywood  0.0856    0.0512           1.67
    2 3.175  softwood          Wood/Plywood  0.1214    0.0512           2.37
    2 3.175  hardwood          Wood/Plywood  0.0778    0.0512           1.52
    2 6      mdf               MDF/Laminate  0.1319    0.0758           1.74
    2 6      plywood_hardwood  Wood/Plywood  0.1262    0.0635           1.99
    2 6      softwood          Wood/Plywood  0.1790    0.0635           2.82
    2 6      hardwood          Wood/Plywood  0.1147    0.0635           1.81
    3 3.175  mdf               MDF/Laminate  0.0894    0.0423           2.11
    3 3.175  plywood_hardwood  Wood/Plywood  0.0856    0.0339           2.53
    3 3.175  softwood          Wood/Plywood  0.1214    0.0339           3.58
    3 3.175  hardwood          Wood/Plywood  0.0778    0.0339           2.30
    3 6      mdf               MDF/Laminate  0.1319    0.0513           2.57
    3 6      plywood_hardwood  Wood/Plywood  0.1262    0.0423           2.98
    3 6      softwood          Wood/Plywood  0.1790    0.0423           4.23
    3 6      hardwood          Wood/Plywood  0.1147    0.0423           2.71
    engine/Amana, printed column (mdf, plywood_hardwood): n=8 median 2.051 range 1.409-2.984
    engine/Amana, shared column (softwood, hardwood): n=8 median 2.542 range 1.520-4.231
```

For a flat end mill, the engine's drill chip load is 1.4-4.2× the Amana
Ramp Down figure (median 2.3). Every cell is above it.

### 1.4 Plunge rate: fractions and absolute rates

| Source | Tool | D | Z | Material | RPM | Plunge mm/min | Per mm D | Engine max (mm/min) | × engine max | Fraction of side feed |
|---|---|---|---|---|---|---|---|---|---|---|
| Amana | flat end, Spektra 2F | 3.175 | 2 | Wood/Plywood | 18000 | 1842 | 580 | 1111 | 1.66 | 1/2 (printed rule) |
| Amana | flat end, Spektra 2F | 3.175 | 2 | MDF/Laminate | 18000 | 2286 | 720 | 1111 | 2.06 | 1/2 |
| Amana | flat end, Spektra 2F | 6 | 2 | Wood/Plywood | 18000 | 2286 | 381 | 2100 | 1.09 | 1/2 |
| Amana | flat end, Spektra 2F | 6 | 2 | MDF/Laminate | 18000 | 2730 | 455 | 2100 | 1.30 | 1/2 |
| Amana | flat end, Spektra 3F | 3.175 | 3 | Wood/Plywood | 18000 | 1829 | 576 | 1111 | 1.65 | 1/3 |
| Amana | flat end, Spektra 3F | 3.175 | 3 | MDF/Laminate | 18000 | 2286 | 720 | 1111 | 2.06 | 1/3 |
| Amana | flat end, Spektra 3F | 6 | 3 | Wood/Plywood | 18000 | 2286 | 381 | 2100 | 1.09 | 1/3 |
| Amana | flat end, Spektra 3F | 6 | 3 | MDF/Laminate | 18000 | 2769 | 461 | 2100 | 1.32 | 1/3 |
| PreciseBits | bull nose 3F, R 0.64 | 3.175 | 3 | softwood, Janka < 1500 | - | 1905 | 600 | 1270 | 1.50 | not computable |
| PreciseBits | bull nose 3F, R 0.64 | 3.175 | 3 | hardwood J1450 (grade_wrong: printed band is "Softwood") | - | 1905 | 600 | 1270 | 1.50 | not computable |
| PreciseBits | bull nose 3F, R 0.64 | 3.175 | 3 | hardwood, Janka 1500-2500 | - | 1270 | 400 | 1270 | 1.00 | not computable |
| PreciseBits | bull nose 3F, R 0.64 | 3.175 | 3 | hardwood, Janka > 2500 | - | 1016 | 320 | 1270 | 0.80 | not computable |

The engine max column uses 400 mm/min per mm for solid wood and 350 for
plywood and sheet goods. PreciseBits prints no side feed and no RPM, so
its fraction is not computable. The printed plunge per mm of D is n=12,
median 519, range 320-720 mm/min per mm. The engine ceiling is 400 (wood) and
350 (sheet goods).

Prose fractions (grade c, and the verifiers did **not** check these sources):
CLE Bit Co. 1/3 (for a 45° ramp, not a plunge); ToolsToday "about half";
Adam's Bits 800 mm/min absolute, with no size. Onsrud 34-100 prints
plunge 40 / feed 80 IPM = 1/2, but for honeycomb composite, not wood.

Wood-drill feeds (derived from the printed vf): Leitz 1200-4500 mm/min at
2500-4500 RPM; Onsrud footnote 3810 mm/min at 4500 RPM (gang drill); CMT
1000-4000 mm/min at 6000 RPM.

### 1.5 RPM and depth

| Quantity | Engine | Printed |
|---|---|---|
| RPM, D ≤ 6 mm | 8000-14000 (`drill_rpm_envelope_for_diameter`) | wood drills 2500-6000 (Leitz, Onsrud, CMT); Leitz dowel diagram range 3000-12000; Amana end mill 18000; PreciseBits "use your maximum RPM" |
| Per-peck maximum | softwood 6 × D, medium 5 × D, dense 4 × D, sheet 1.5 × D | none for any tool |
| Total hole before a clearance stroke | none | Leitz twist HW: "greater than 4 x D interim clearance stroke is recommended"; Levin HS: up to about 4 × D without one; Levin HW D 12-16: up to 75 mm (4.7-6.3 × D, derived) |
| Maximum infeed | none | Leitz boring pins in hardwood and glulam: "maximum 2 x D", and a return stroke "is obligatory" |
| Feed above 4 × D | none | Leitz Levin: "Drilling depth > 4 x D = 0.8" |

### 1.6 What the data shows

1. **A wood drill and a router end mill need different answers.** The wood
   drills print 0.13-0.50 mm/lip (median 0.29). The only printed
   end-mill plunge figure (Amana) gives 0.034-0.076 mm/tooth axial. The two
   populations do not overlap.
2. **The 2.5 multiplier has no support in either direction.**
   - Against the same vendor's end mill, a wood drill runs at about the
     same chip load (Onsrud, median 0.94, 0.83-1.14 without the downcut
     series). The ratio is not 2.5. The Onsrud drill row is for a gang
     drill at 4,500 RPM and 150 IPM, a rigid multi-spindle borer, not a
     router collet.
   - Against the engine's own formula, wood drills imply 0.95-9.3
     (median 4.6). This number mostly measures the formula, not the drill.
   - For an end-mill plunge, Amana prints 1/Z of the side chip load
     (0.5 or 0.33). Against the formula this is 0.84-1.78 (median 1.22).
3. **Amana's Ramp Down rule is exact and simple:** axial advance per
   tooth = side chip load / Z at the same RPM. This is equivalent to Ramp
   Down feed = side feed / Z. It holds on all 8 verified rows to within the
   vendor's rounding. The column is named "Ramp Down", not plunge. This
   record reads it as the plunge figure; that reading is an inference.
4. **10 of the 12 printed plunge feeds exceed the engine's plunge
   ceiling.** Amana and PreciseBits print 320-720 mm/min per mm of D. The engine ceiling is 400
   (wood) and 350 (sheet). At 3.175 mm the Amana figure is 1.65-2.06× the
   engine ceiling.
5. **PreciseBits orders plunge feed by hardness:** 75 → 50 → 40 in/min as
   Janka rises through 1500 and 2500. The ratios are 1.00 : 0.67 : 0.53
   for one tool (derived, `trend_g6.py` T4).
6. **Wood-drill chip load has no single size law.** Onsrud rises as
   D^0.485, the Leitz dowel is flat, and Leitz Marathon rises then falls.
7. **Depth:** a vendor prints a total-hole regime (about 4 × D before a
   clearance stroke) and one maximum infeed (2 × D for boring pins in
   hardwood). Neither is a per-peck depth.

### 1.7 What the data does NOT show

- No printed figure for a straight vertical plunge with a router end mill.
  Amana's column is "Ramp Down" (the chart title prints "Spiral Plunge
  Router Bits"). This record reads the column as the plunge figure. That
  reading is an inference. The Phase 4 ruling must accept or refuse it.
- No row for a ball nose, a tapered ball nose or a V-bit used as a drill.
- No printed plunge figure for a flat end mill in solid softwood or hardwood
  that names the category. Amana prints one "Wood/Plywood" column. The LUT
  already reads that column into softwood, hardwood and both plywoods for
  the side rows (R5 shared-column rule, derived grade b).
- No end-mill plunge figure at an RPM other than 18,000 (Amana), and none
  with an RPM at all from PreciseBits.
- No Drill versus AlignmentPinDrill difference. No row names the pin-drill
  case.
- No per-peck depth for any tool in wood. The repo's 6 / 5 / 4 × D and
  1.5 × D stay unsourced.
- No bull-nose plunge at 6.0 mm. PreciseBits prints one 3.175 mm tool.
- No source for `DRILL_CHIPLOAD_MULTIPLIER = 2.5`.
- No independent second vendor for the 1/Z rule. The Amana ball nose v7
  chart prints the same rule, but it is the same vendor.

## 2. Sources

Hash: for a PDF, the sha256 of the PDF; for an HTML page, the sha256 of the
stored text file (the raw HTML hash is in the text header). All stored
texts are in `fetch/G6/sources/`.

| Source id | Vendor | What it prints | Grade | URL reachable | sha256 (first 16) | Verifier verdicts |
|---|---|---|---|---|---|---|
| `onsrud_drill_cutting_data` | Onsrud | 72-000 "Wood" chip load per tooth at 3/5/6/8 mm; footnote "Gang drills run at 4,500 RPM and 150 IPM" | b (derived: one "Wood" row) | yes (200) | `b1e3821f79bc474e` (PDF) | 4 confirmed |
| `onsrud_pct19_catalog` | Onsrud | p90: 72-000 bits have 2 flutes at 3/5/6/8 mm; p124 repeats the drill table; p32 composite plunge 40 / feed 80 IPM | supporting only | yes (200) | `83dbf74242ca1f57` (PDF) | 0 own rows; 4 supporting checks confirmed |
| `leitz_lexicon7_06_drilling` | Leitz | vf at n per drill type (red worked examples), Z, D ranges, material factors, depth statements | b (derived) | yes | `e8026f2573edf22f` (PDF) | 13 confirmed; 8 depth statements confirmed; 3 nits (softwood is printed on the twist pages; a stray sentence in one note; the reversal-point context "when drilling dowel holes") |
| `cmt_311_71_72_hwm_dowel_drill` | CMT | "Recommended feed speed 1÷ 4m/minute – RPM 6000.", Z2, chipboard/MDF/HDF/laminates, D 5-10 | b (derived) | yes (200) | `255a06347dd2fd8a` (text) | 2 confirmed; caution: the page contradicts itself on spurs, so `brad_point_drill` is an inference |
| `amana_spektra_spiral_plunge_v24` | Amana | "Ramp Down" IPM column and the rule "Feed Rate IPM / # of flutes" at 18,000 RPM | b (derived) | yes (200) | `5b6fef854b2cf6b4` (PDF; matches the LUT manifest) | 8 confirmed |
| `amana_ball_nose_v7` | Amana | the same Ramp Down rule, no column | - | yes (200) | `e851a270a9958a9a` (PDF; matches the LUT manifest) | 0 rows; claim confirmed |
| `precisebits_fret_plane` | PreciseBits | plunge 75 / 50 / 40 in/min by Janka for one 3-flute 3.18 mm bull-nose bit; no RPM | b | yes (200) | `9777923942c67479` (text) | 3 confirmed, 1 grade_wrong (hardwood at Janka < 1500, now derived / c) |
| `freud_router_bit_feed_speed_cnc_20170822` | Freud | "Carbide tipped bits should not be used to drill directly into the work piece." | refusal | yes (200) | `ff1a29ea5a669e24` (PDF) | 0 rows; claim confirmed |
| `whiteside_catalog_icstc` | Whiteside | dowel drills and boring bits, no feeds; 6100/6140 "NOT for use in routers." | refusal | not verified | `9ac0bd1c3158d922` | not verified |
| `diablo_forstner_speed_chart` | Diablo | Forstner RPM only | none | not verified | `b25ee8cb70adc3e3` | not verified |
| `clebitco_speeds_and_feeds` | CLE Bit Co. | "... at ⅓ of the calculated feed rate" (a ramp) | c | not verified | `e678480f10d38f1e` | not verified |
| `toolstoday_understanding_cnc_feeds_and_speeds` | ToolsToday | "reduce ramp or plunge feed to about half the main feed rate" | c | not verified | `ec0845b0f51c23de` | not verified |
| `adams_bits_feeds_and_speeds_guide` | Adam's Bits | "Plunge rates are not calculated"; "You can plunge at 800 mm/min for timber and plastic" | c | not verified | `6664e95d14f95af6` | not verified |

The five unverified sources give 0 rows. Only the prose ratios in §1.4 cite
them, marked grade c.

### Dead ends (from `fetch/G6/FETCH_NOTES.md` §6)

| Vendor / URL | Result |
|---|---|
| Amana boring-bit and countersink product pages | HTTP 403. Search snippets say "max RPM 8,000" and "3,000 RPM with a 30 IPM plunge rate". Not stored, so not cited. |
| Guhdo catalogue PDF | HTTP 404. The catalogue page takes requests only. |
| Vortex catalogue (ctsaw.com) | HTTP 403. `vortextool.com/feeds-speeds` has no drill or plunge data; `/chip-load-chart` and `/technical-information` are 404. |
| CMT 2014 leaflet (scosarg.com) | HTTP 403. |
| CMT 311.41/42 HW, 314.21/22 HWM, 308 HW pages | 200, no feed printed. |
| Fisch dowel-drill page | product list only. |
| Whiteside catalogue | no feeds. |
| Onsrud `Series/72-000.asp` | product index, no data. |
| Onsrud PCT-19 technical pages 109-112 | no wood plunge or drill guidance. |
| PreciseBits `reference/drillfeedspeed.htm` | PCB drills in FR-4, not wood. |
| Diablo Forstner chart | RPM only. |
| Famag, Colt, Fisch | no printed feed or chip load reached. |
| ToolsToday drill-geometry page | qualitative only. |
| ShopBot feeds PDF | no plunge or drill content. |

Prior evidence, not re-verified here: the feeds matrix row 3.2-18
(`planning/feeds_matrix_2026-09-23/EVIDENCE.md`) cites an IDC ball-nose plunge
of 15 / 30 in/min (1/8 in and 1/4 in). No G6 source stores that text, so this
record does not use it.

## 3. For Phase 3

### 3.1 Candidate forms the trend supports

| Sub-class | Cells | Candidate form | Anchor | Range it could claim | Second witness that could exist | Recommendation |
|---|---|---|---|---|---|---|
| Flat end mill (EndMill) on Drill / AlignmentPinDrill | 16 | axial fz = side fz / Z at the side row's RPM; plunge feed = side feed / Z. Anchor on the tool's own side row (the vendor lookup already finds it). One inference to rule on: the form reads Amana's "Ramp Down" column as a straight plunge. | Amana Spektra v24, 8 rows, printed rule + printed column | 3.175-6.0 mm (the verified rows), 2F and 3F, 18,000 RPM, MDF column and Wood/Plywood column. The chart prints the rule for 1/32 in to 3/4 in; those rows are not yet verified. Softwood and hardwood reach the Wood/Plywood column by the R5 shared-column rule (derived b), as the side rows already do. | (1) Simulation: the axial chip per tooth on the wanaka or terrain fixture during a plunge, against the side chip on the same tool. (2) PreciseBits absolute plunge at 3.175 mm (75 in/min against Amana 72-72.5 in/min) is a different vendor and tool at an unknown RPM; it can check the order of magnitude only. | **per-family** (flat end mill only, one witness), if the operator accepts Ramp Down as plunge. Refuse outside 3.175-6.0 mm until more Spektra sizes are verified. |
| Bull nose (BullNose) on Drill / AlignmentPinDrill | 16 | plunge feed only (mm/min) by Janka band: 1905 / 1270 / 1016 mm/min | PreciseBits, 1 tool, 3.175 mm, 3F, R 0.64 | 3.175 mm, Janka bands < 1500 / 1500-2500 / > 2500. No RPM, so no chip load. 6.0 mm has no anchor. Plywood and MDF have no anchor. | Treat the bull nose as a flat end mill (the 1/Z form above) and compare with PreciseBits. A match would be two vendors for one number at 3.175 mm. | **per-family, narrow** (3.175 mm, solid wood, feed only), or refuse. The operator decides whether a feed-only claim with no RPM may ship. The 6.0 mm cells and the plywood / MDF cells stay refused. |
| Ball nose (BallNose) on Drill / AlignmentPinDrill | 16 | none | none | - | none found | **refuse** (R1: no anchor on the family). |
| Tapered ball nose on Drill / AlignmentPinDrill | 16 | none | none | - | none found | **refuse**. |
| V-bit on Drill / AlignmentPinDrill | 16 | none; `DRILL_VBIT` already says a V-bit cuts a cone | none | - | none | **refuse**. |
| Wood drill (a future drill tool kind; 0 matrix cells) | 0 | chip load per lip by drill type and material, with the printed Leitz material factors; no single size law | Onsrud 72-000, Leitz ch.6, CMT: 19 rows, 3 vendors | 3-16 mm, 2500-6000 RPM. Three vendors overlap at 0.13-0.50 mm/lip. | the three vendors witness each other in the band. The size trend disagrees across vendors. | **per-family, later.** It needs a `ToolFamily` drill arm first. These rows **must not** serve an end-mill plunge. |
| `DRILL_CHIPLOAD_MULTIPLIER` 2.5 | all 80 | - | none | - | - | **delete, do not refit.** No figure supports 2.5. The end-mill figure is 1/Z of the side chip; the wood-drill figure belongs to a different tool. |

### 3.2 Engine rules the trend puts in question

These do not move any cell by themselves. Phase 3 must state them before
the rulings.

- `drill_plunge_feed_envelope_per_mm` ceiling (400 wood / 350 sheet) is
  below every Amana plunge at 3.175 mm (580-720 mm/min per mm). A 1/Z claim
  that ships would hit this clamp. The envelope and a 1/Z claim cannot both
  stand.
- `drill_rpm_envelope_for_diameter` gives 8000-14000 at D ≤ 6 mm. The only
  end-mill plunge figure is at 18,000 RPM. A per-tooth claim that uses the
  engine RPM changes the feed by the RPM ratio. The card must say which
  quantity it holds (per tooth or per minute).
- The per-peck bands 6 / 5 / 4 × D and 1.5 × D stay unsourced. The only
  printed depth rules are Leitz's 4 × D total-hole regime and the 2 × D
  boring-pin infeed, both for wood drills.

### 3.3 Cells that stay refused under the best case

- 48 cells: every BallNose, TaperedBallNose and VBit cell on Drill and
  AlignmentPinDrill.
- 12 BullNose cells if the operator accepts the narrow PreciseBits claim
  (6.0 mm in every material, and 3.175 mm in mdf and plywood_hardwood);
  16 if not.
- 0 EndMill cells, if the 1/Z claim ships at 3.175-6.0 mm with softwood and
  hardwood read through the Wood/Plywood column. If the operator refuses
  the shared-column transfer for a drill, the 8 EndMill softwood and
  hardwood cells stay refused as well.

Best case: 60 of 80 cells stay refused and 20 ship (16 EndMill, 4 BullNose).

## 5. The landing (B5, 2026-09-24)

Commit `7eef9ffa` (steps 1-2 of `B5_PLAN.md`) lands the flat end mill
plunge claim. Step 3 (this section, the literature matrix, `narrate` and
the `apply_drill_defaults` doc) follows it.

### 5.1 What changed

- `DRILL_CHIPLOAD_MULTIPLIER` (2.5) is deleted. No figure supported it.
- `feeds::extrapolation::drill` holds the G6 claim (`DrillRule`,
  `DrillClaim`, `Gap::Drill`). The claim serves a flat end mill, 3.175-6.0
  mm, 2 or 3 flutes, from the tool's own Amana Spektra side row (the pocket
  home row). Amana prints "Ramp Down = Feed Rate IPM / # of flutes" at one
  RPM, so the axial chip per tooth = side chip / Z. The support arm is
  `FeedsSupport::DrillTransferred`.
- The chip is held per tooth. The engine drill RPM cap (a repo rule, 14 000
  at D <= 6 mm) replaces the chart's 18 000 RPM, so the feed is about 0.78x
  the printed Ramp Down. The card says so.
- The plunge envelope ceiling moves to the largest printed plunge per mm:
  solid wood and plywood 580 mm/min per mm (72.5 in/min / 3.175 mm), sheet
  goods 720 (90 in/min / 3.175 mm). It was 400 / 350. The floors (50 / 40)
  stay repo rules.
- Every drill cell that no G6 claim serves refuses, in every material
  (plastics and aluminium too), with a text that names G6 and ruling B5.

### 5.2 The 16 cells that ship

Flat end mill (EndMill), 2 flutes, Drill and AlignmentPinDrill (identical
numbers), 14 000 RPM in every cell. The FM1 CSV
(`planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`) is committed in
`7eef9ffa`.

| D (mm) | Material | Side row chip (mm) | Axial chip = side / Z (mm) | Feed = plunge (mm/min) | Printed Ramp Down (mm/min) |
|---|---|---|---|---|---|
| 3.175 | softwood, hardwood, plywood_hardwood | 0.1016 | 0.0508 | 1422 | 1841.5 |
| 3.175 | mdf | 0.127 | 0.0635 | 1778 | 2286 |
| 6.0 | softwood, hardwood, plywood_hardwood | 0.127 | 0.0635 | 1778 | 2286 |
| 6.0 | mdf | 0.1524 | 0.0762 | 2133 | 2730.5 |

- The feed rounds down. Float rounding decides 1777 or 1778.
- The 6.0 mm MDF chip is 0.0762 against the derived 0.0758 (107.5 / (18 000
  x 2) in), because Amana rounded its printed in/min.
- Refusals in the matrix: 426 -> 410.

### 5.3 Wanaka

Toolpaths 7 (Holes) and 14 (Pin Drill) now ship. Both run a 6 mm 2-flute
flat end mill in GenericHardwood (Janka 1450, the row's own Janka, so no
hardness scale). The row is `amana-flat-hardwood-pocket-6000-2f-spektra`:
chip 0.127 / 2 = 0.0635 mm/tooth, 14 000 RPM, feed 1777-1778 mm/min. No
Wanaka toolpath refuses. `wanaka_suggest_integration` pins these numbers
and the restored drill no-lift assertions.

### 5.4 The literature matrix (step 3)

- `flat_6mm_drill_oak` and `flat_6mm_drill_oak_using_endmill_unadvised`
  ship. Their `feed_per_tooth` bands (0.10-0.22 and 0.04-0.15) were fitted
  to the formula x 2.5. They are re-banded from the printed chart
  (decision 7), not from the LUT: 90 / (18 000 x 2) in = 0.0635 mm, x
  (1450 / 1360)^0.5 = x1.0326 for white oak = 0.0656 mm/tooth. The band is
  0.0643-0.0669 (+/-2 %, a repo tolerance for the chart's in/min
  rounding). The source key is `amana_spektra_plunge_v24`.
- `drill_final_feed_in_plunge_envelope` moves from 400 to 580 on every
  drill cell. All seven drill cells are solid wood.
- The cells at 3.0 mm (two), 2.0 mm, 12 mm and the 3 mm ball nose refuse,
  and the runner reads them as NA. Their comments say so.

### 5.5 What stays refused, and why

- 48 cells: every BallNose, TaperedBallNose and VBit drill cell. No
  published plunge figure exists for these families (section 3.1).
- 16 cells: every BullNose drill cell. The one printed bull plunge
  (PreciseBits) is a feed with no RPM, for one 3-flute tool (decision 4).
- A flat end mill outside 3.175-6.0 mm, or with 1 or 4 flutes: no verified
  Ramp Down row covers it (decision 6).
- A drill cell in plastic or aluminium: without the 2.5 the formula is an
  unjudged milling number on a plunge (decision 3).

### 5.6 Follow-ups

- Real wood drills (Onsrud 72-000, Leitz, CMT; 0.13-0.50 mm per lip) need
  a drill tool kind (a `ToolFamily` drill arm) first. Those rows must never
  serve an end-mill plunge.
- The other Spektra sizes (1/4 in and up) are a transcription job. The
  chart prints the same rule for every size. Transcribe and verify the
  rows, then widen `range_mm`. No new law is necessary.
- The registry rows in `compute/catalog/registry.rs` must set
  `feeds_formula_source: None` for the two drill operations. The compute
  session owns that file.
- Open question 1 stays open: whether Amana's printed 18 000 RPM may win
  over the 14 000 cap on a claimed row. If it wins, the 3.175 mm MDF cell
  lands at exactly 720 mm/min per mm, the new ceiling.
