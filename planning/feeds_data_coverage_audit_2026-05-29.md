# Feeds/Speeds & Tool-Load Data Coverage Audit — 2026-05-29

Read-only audit of the feeds/speeds and tool-load data baked into `rs_cam`.
Scope: enumerate the data axes, inventory the vendor LUT, map matching/fallback
behavior, and catalogue the physical constants. The deliverable is a coverage
matrix, a gap list, and a "perfect-world" dataset spec to guide real-data
acquisition.

> **Companion:** `planning/feeds_data_source_acquisition_2026-05-29.md` is the
> sourcing answer to this audit's gaps — a citeable primary-source list (Amana,
> Onsrud, Harvey/Helical/Garr, Sandvik/Kennametal Kc, FPL/Wood Database/MatWeb
> hardness, plus named Kc research papers) with evidence grades and a ranked
> ingestion order. This doc says *what's missing*; that doc says *where to get it*.

---

## 1. The axes (from code)

### Tool families

Two parallel enums — they are NOT the same set, which is itself a gap:

- **`ToolFamily`** (LUT/vendor classification) —
  `crates/rs_cam_core/src/feeds/vendor_lut.rs:65-72`:
  `FlatEnd`, `BallNose`, `TaperedBallNose`, `BullNose`, `ChamferVbit`, `FacingBit` (6).
- **`ToolType`** (user-facing tool model) —
  `crates/rs_cam_core/src/compute/tool_config.rs:10-16`:
  `EndMill`, `BallNose`, `BullNose`, `VBit`, `TaperedBallNose` (5).
  Note: there is **no `FacingBit` in `ToolType`** — facing is done with an
  end mill / large bull nose, so the LUT's `FacingBit` family has no direct
  user tool to map to (it is only reachable via the `Bull→Flat` fallback chain,
  not a dedicated facing tool).

`ToolGeometryHint` (`feeds/mod.rs`) is the bridge: `Flat`, `Ball`, `Bull`,
`VBit { included_angle, tip_diameter }`, `TaperedBall { tip_radius, taper_angle }`.
Mapped to `ToolFamily` in `feeds/vendor_normalize.rs:12-18`.

### Material families

- **`MaterialFamily`** (LUT) — `vendor_lut.rs:77-90` (12):
  `Softwood`, `Hardwood`, `PlywoodSoftwood`, `PlywoodHardwood`, `Mdf`, `Hdf`,
  `Particleboard`, `Acrylic`, `Hdpe`, `Polycarbonate`, `Delrin`, `Aluminum`.
- **`HardnessKind`** — `vendor_lut.rs:95-99`: `Janka`, `Hb`, `ShoreD`.
- **`Material`** (engine model) — `material.rs:149-171`: `SolidWood{species}` (10
  species), `Plywood{grade}` (3), `SheetGood{kind}` (3), `Plastic{family}` (5),
  `Foam{density}` (3), `Custom{name,hardness_index,kc}`.
  - **Foam and Aluminum mismatch**: `Material` has Foam (3 densities) but the LUT
    has **no foam family** — foam maps to `Softwood/janka 200` and always falls
    through to formula (`vendor_normalize.rs:123-126`). Conversely the LUT has an
    `Aluminum` family but `Material` has **no aluminum variant** — there is no way
    to query the aluminum LUT rows from the engine's material model except via
    `Custom`, which the power gate refuses anyway (see §5).

---

## 2. Vendor LUT inventory

Embedded at compile time via `include_str!` (`vendor_lut.rs:184-190`) from
`crates/rs_cam_core/data/vendor_lut/observations/`. **67 total rows** (asserted in
`vendor_lut.rs:229`). A second identical copy lives at
`reference/shapeoko_feeds_and_speeds/data/vendor_lut/` (the upstream port source).

| File | Rows | Tool family | Vendors | Diameters (mm) | Flutes |
|------|------|-------------|---------|----------------|--------|
| `amana_flat_end.json` | 20 | flat_end | Amana 14, Onsrud 6 | 3.175, 6.0, 12.0 | 1,2,3 |
| `amana_ball_nose.json` | 17 | ball_nose | Amana 17 | 0.794, 1.0, 1.5, 1.5875, 3.175, 6.0 | 2,3,4 |
| `amana_3d_profiling.json` | 14 | tapered_ball_nose 10, bull_nose 4 | Amana 10, Onsrud 4 | 3.175, 6.0 | 2,3 |
| `amana_vbit.json` | 8 | chamfer_vbit | Amana 6, Whiteside 2 | 6.0, 12.0 | 2 |
| `amana_facing.json` | 8 | facing_bit | Amana 6, Whiteside 2 | 22.0, 25.0 | 2 |

Aggregate breakdown:
- **Evidence grade**: A (vendor chart) 53, B (derived) 14.
- **Row kind**: Exact 66, Derived 1.
- **Operation family**: parallel 22, adaptive 10, pocket 10, face 8, trace 7,
  contour 5, scallop 5.
- **Pass role**: finish 35, roughing 28, semi_finish 4.
- **Hardness kind on rows**: janka 53, shore_d 10, hb 4.

### A. Coverage matrix (tool family × material family)

Cell = number of direct LUT rows for that combination. `.` = no direct row.
Materials within the same broad category (wood/sheet group, plastics group,
metal) can extrapolate to neighbours via hardness scaling (§3), so a `.` in the
wood block is *formula-or-extrapolated*, while a `.` across category boundaries
is hard `none`.

```
tool\material      soft hard plyS plyH mdf  hdf  part acry hdpe poly delr alum
flat_end            5    4    1    1    1    1    1    2    1    1    1    1
ball_nose           6    3    .    .    4    .    .    2    .    .    .    2
bull_nose           1    1    .    1    .    .    .    .    .    .    .    1
chamfer_vbit        2    2    .    1    2    .    .    1    .    .    .    .
tapered_ball_nose   2    4    .    .    2    .    .    2    .    .    .    .
facing_bit          2    1    .    1    2    1    1    .    .    .    .    .
```

Reading the cells against the matching logic (§3):

- **`flat_end`** is the only family with full material breadth — every column has
  ≥1 row, and it is the fallback target for bull-nose queries.
- **Plastics** (acry/hdpe/poly/delr): only `acrylic` has rows (flat 2, ball 2,
  vbit 1, tapered 2). `hdpe`, `polycarbonate`, `delrin` have **exactly one row
  total each** — all in `flat_end`. Any plastic query for ball/vbit/tapered HDPE,
  PC, or Delrin extrapolates off the acrylic row (same category) or off the
  flat-end plastic row via tool-family fallback.
- **`bull_nose`** has 4 rows (soft, hard, plyH, alum) and otherwise relies on the
  `BullNose→FlatEnd` family fallback (`vendor_lookup.rs:321,333`).
- **`facing_bit`** has no plastic/metal rows at all, and the diameters (22/25 mm)
  are far from any other family — a 12.7 mm "facing" cut on an end mill won't
  reach these rows (16× ratio is past the sanity gate).
- **`aluminum`**: 4 rows (flat, 2× ball, bull). No vbit/tapered/facing aluminum.
  And again — no `Material::Aluminum` exists to query them (§1).

Per-cell verdict legend applied:
- `direct LUT rows: N` — cell value ≥ 1 above.
- `extrapolated-only` — cell `.` but a same-category sibling row exists (e.g.
  hdpe ball → acrylic ball, or any sheet-good → mdf); lookup succeeds with
  `is_extrapolated=true`.
- `formula-only` — cell `.` and the family has *no* same-category sibling for
  that tool family (e.g. vbit hdpe, tapered hdpe, facing acrylic) → falls to the
  empirical formula.
- `none` — cross-category (wood query can never reach an aluminum row;
  `vendor_lookup.rs:225-227,238`).

---

## 3. Matching / fallback logic

Entry: `feeds/mod.rs:263-289`. `calculate()` builds a `LookupQuery` via
`vendor_normalize::to_lookup_query` and calls `vendor_lookup::find_best_row`.
On `Some` → `ChiploadSource::VendorLut`; on `None` → `ChiploadSource::FormulaFallback`
(`feeds/mod.rs:128`). A third source `EdgeRadiusFloor` exists for tiny-tool floors.

### Must-match hard filters (`vendor_lookup.rs:229-258`)

1. `operation_family` must match exactly (line 230).
2. `material_category` must match — wood/sheet (cat 0), plastics (cat 1), metal
   (cat 2) (`vendor_lookup.rs:208-227`). **Cross-category = hard reject.**
3. `tool_family_compatible` — exact, or one of the fallback pairs:
   `BullNose↔FlatEnd`, `TaperedBallNose→BallNose` (`vendor_lookup.rs:314-325`).
   **Note: `ChamferVbit` and `FacingBit` have NO fallback partner** — a vbit or
   facing query can only ever match a vbit/facing row.
4. Diameter sanity floor: `query.d / row.d` must be within
   `SCALE_CLAMP_LO..=SCALE_CLAMP_HI` = **0.1 .. 10.0** (`vendor_lookup.rs:105-106,
   253-256`). Past that → reject.

### Scoring (`vendor_lookup.rs:260-311`)

Base 1000, plus: tool-family score (220 exact / 100-120 fallback), row-kind
(120/70/30), evidence (60/30/10), flute diff (+80/+30/-20), diameter proximity
(0-200 via `(1 - |ln ratio| / ln 2) × 200`), hardness proximity (0-80),
subfamily (+50), pass-role (+45 / -25), material-family exact (+100).

### Scaling / extrapolation (`vendor_lookup.rs:115-152`)

- **Diameter scale** = `query.d / row.d`, clamped 0.1-10× — multiplies chipload
  bounds directly.
- **Hardness scale** = `row.hardness / query.hardness` (inverse — softer query →
  more chipload), clamped 0.1-10×, **only when hardness kinds match**.
- **`is_extrapolated`** = `|ln(diameter_scale × hardness_scale)| >
  APPROX_LN_THRESHOLD` where the threshold is **ln(1.4) ≈ 0.3365** (±40 %)
  (`vendor_lookup.rs:113,152`). When true, downstream verdicts demote to
  `Confidence::Approximate`.

### The V-bit "no_vendor_data" trap (confirmed)

`vendor_normalize::lookup_diameter_for_input` (`vendor_normalize.rs:51-85`) does
**not** pass the V-bit's physical diameter to the LUT. For a V-bit it passes the
**engaged width at the cut depth**: `2 · axial_doc · tan(included_angle/2)`
(`vendor_normalize.rs:77-80`). But the V-bit LUT rows store a **fixed nominal
diameter** of 6.0 or 12.0 mm (`amana_vbit.json`). Worked examples for a 90° V-bit:

- DOC 2 mm → engaged width 4 mm → ratio to 6 mm row = 0.67 (passes sanity, but
  scaling 0.67× and within the Approximate band → flagged extrapolated).
- DOC 0.5 mm → engaged width 1 mm → ratio 0.17 (still inside 0.1-10×, but a 0.17×
  scale is deeply extrapolated; chipload bounds get scaled to ~1/6).
- DOC 0.3 mm on a 6 mm row → 0.6 mm width → ratio 0.10 — at the sanity floor; any
  shallower → **hard reject → formula fallback**.

So shallow V-carving (the common case) either trips `is_extrapolated`
(→ Approximate) or falls through entirely. On the tool-load side the chipload
gate then returns `Unmodeled(NoVendorData)`
(`tool_load/chipload.rs:31`, `tool_load/mod.rs:113`). The matching is comparing an
engagement-derived width against fixed-diameter rows that were never calibrated
that way — the prior audit finding is reproduced here.

---

## 4. Empirical formula constants

Formula: `fz = K0 · D^p · (1/H)^q` (`feeds/mod.rs:261`), evaluated when no LUT row
matches.

- **K0 / p / q** live on `MachineProfile.chip_load: ChipLoadFormula`
  (`machine.rs:29-44`). Default and **every preset use the same values**:
  `k0 = 0.024, p = 0.61, q = 1.26` (`machine.rs:39-41, 143-145, 164-166`).
  Commented "Soft wood baseline from Shapeoko empirical data" — a **single global
  curve fit**, not per-material or per-tool-family. There is no per-tool-family or
  per-vendor variation in the formula.
- **H (hardness index)** = `(Janka / 600)^0.4` for wood/sheet
  (`material.rs:184-188`); fixed `0.5` for ALL plastics (`material.rs:189`); foam
  `0.15/0.25/0.40` (`material.rs:190-194`). The `0.4` exponent and the plastic
  `0.5` are **hand-tuned constants with no cited source**; only the Janka anchors
  are real (Forest Products Lab Wood Handbook, per CREDITS.md:159).
- **RPM seed**: `base_cutting_speed_m_min()` (`material.rs:236-245`) — wood 200,
  ply 180, sheet 170, plastic 250, foam 300 m/min. Round-number defaults, no
  per-species or per-tool variation.

**Real vs guessed**: Janka values (`material.rs:25-37`) are real. K0/p/q are a
single empirical fit. The hardness-index exponent, the plastic flat `0.5`, foam
indices, and base cutting speeds are engineering guesses.

---

## 5. Tool-load physical constants

### Specific cutting force Kc

**Single source of truth**: `Material::kc_n_per_mm2()` (`material.rs:200-232`).
A per-material lookup table (N/mm²):

| Material | Kc | | Material | Kc |
|----------|----|--|----------|----|
| Softwood/Radiata | 6 | | Ply softwood | 8 |
| S. Yellow Pine | 7 | | Ply baltic birch | 13 |
| Generic hardwood | 14 | | Ply hardwood-faced | 11 |
| Hard maple | 15 | | MDF | 10 |
| Walnut | 12 | | HDF | 12 |
| Birch | 13 | | Particleboard | 9 |
| White oak | 14 | | Plastic (ALL) | 4 |
| Jarrah | 19 | | Foam low/med/high | 1/2/3 |
| Ipe | 28 | | Custom | user |

- **Plastics share one Kc = 4** regardless of acrylic/HDPE/PC/Delrin
  (`material.rs:224`). No aluminum entry.
- **Power gate** multiplies by a worst-case anisotropy factor:
  `ANISOTROPY_MULTIPLIER = 2.5` → `Kc_eff = 2.5 × Kc` (`tool_load/power.rs:36,82`).
  Module doc (lines 4-9) is explicit: "Real wood Kc varies 2-3× with grain
  direction; we don't model grain, so we use the upper bound." This is a single
  global multiplier, **not** a per-material or grain-aware table.
- **Deflection gate** uses **raw Kc, no anisotropy multiplier**
  (`tool_load/deflection.rs:34-35,92,103`).
- `Material::Custom` is **refused** by the power gate → `Unmodeled(MaterialUnvalidated)`
  (`tool_load/power.rs:66-76`).

### Young's modulus

Per **tool material**, two values (`compute/tool_config.rs:52-55,69-74`):
- Carbide: `600_000` N/mm² (600 GPa).
- HSS: `200_000` N/mm² (200 GPa).
Cited handbook ranges in the doc comments (550-650 GPa / 200-210 GPa). Used by the
deflection cantilever model. No tool-grade granularity.

### Deflection verdict bounds

`WITHIN_BOUND_MM = 0.050` (50 µm), `EXCEEDS_BOUND_MM = 0.200` (200 µm)
(`tool_load/deflection.rs:58-62`). Global, not per-material/finish.

---

## 6. Material property tables

- **Janka (wood)**: real, 10 species (`material.rs:25-37`), anchored to FPL Wood
  Handbook (CREDITS.md:159).
- **Plywood/sheet "effective Janka"**: `material.rs:65-71, 91-97` — softwood ply
  600, baltic birch 1200, hardwood-faced 1000, MDF 1100, HDF 1300, particleboard
  750. These are **engineering estimates** (sheet goods have no Janka rating;
  values picked to order the families).
- **Plastic hardness (Shore D)**: only in `vendor_normalize.rs:114-122` as LUT
  query keys — acrylic 85, HDPE 65, Delrin 85, PC 80. These are **not** stored on
  `Material` (which gives all plastics hardness_index 0.5 and Kc 4). So plastic
  hardness exists in two disconnected places with different granularity.
- **Drill chip-welding thresholds** double as a hardness proxy
  (`drill_metrics.rs:112-133`).

---

## 7. Machine data

- **Presets**: 3 only — `generic_wood_router`, `shapeoko_vfd`, `shapeoko_makita`
  (`machine.rs:102-186`). All three share the same `ChipLoadFormula`.
- **Spindle/power**: `PowerModel::VfdConstantTorque{1.5 kW @ 24000}` (Shapeoko VFD),
  `ConstantPower{0.71 kW}` (Makita), `ConstantPower{0.8 kW}` (generic)
  (`machine.rs:109,139-141,162`). The VFD model is a linear `power × rpm/rated`
  ramp (`machine.rs:205-218`) — **not a measured torque/power curve**.
- **Max feed**: 4000-5000 mm/min hard-coded per preset (`machine.rs:111,147,168`).
- **Rigidity profile**: DOC/WOC factors (`machine.rs:48-70, 113-121`) — hand-tuned.
- **Kinematics (accel/jerk)**: `MachineKinematics` (`machine_kinematics.rs:59-77`).
  **Every built-in preset ships `kinematics: None`** (`machine.rs:90-91,126`) —
  the absence IS the F-034 feature flag. Three named profiles exist
  (`shapeoko_xxl_stock` 250 mm/s², a 200 mm/s² variant, `shapeoko_xxl_ricky_tuned`
  350 mm/s² blended) — the tuned one is **real, calibrated against an 827 s
  wall-clock measurement on 2026-05-26** (`machine_kinematics.rs:104-124`). Jerk is
  `None` everywhere; junction velocity `None` everywhere.
- **Machine library**: `machine_library.rs` — a per-user TOML store
  (`$RS_CAM_MACHINE_DIR` / XDG), so users can add machines, but **no real machines
  ship** beyond the 3 code presets.

**Real vs default**: only the Shapeoko XXL `ricky_tuned` accel is empirically
calibrated. Power curves, max feeds, rigidity factors, and the generic router are
conservative round-number defaults.

---

## 8. Drill data

All material-specific, all in `drill_metrics.rs` / `drill_gates.rs`, all
first-pass heuristics (doc comment `drill_metrics.rs:110-111`: "Rough first-pass
values"):

- **Chip-welding D/d threshold** (`drill_metrics.rs:112-133`): softwood (janka
  ≤700) 8.0, medium hardwood (≤1500) 6.0, dense hardwood 5.0, ply/sheet 5.0,
  plastic 4.0, foam 12.0, custom = `(8/hardness).clamp(2,12)`.
- **Per-peck max D/d** (`drill_metrics.rs:137-145`): wood 2.0, ply/sheet 1.5,
  plastic 1.0, foam 4.0, custom 1.5.
- **Plunge feed/diameter envelope** (1/min) (`drill_gates.rs:72-80`): wood 50-400,
  ply/sheet 40-350, plastic 60-500, foam 100-1000, custom 40-500.

These are reasonable but uncited. No vendor drill charts; plastics are one bucket
(acrylic vs HDPE vs PC behave very differently when drilling).

---

## B. Gap list per data type (ranked by cut-safety impact)

### LUT (chipload/RPM/ap/ae) — the primary safety signal
1. **V-bit / chamfer**: engaged-width-vs-fixed-diameter mismatch makes shallow
   V-carving almost always extrapolate or fall to `NoVendorData`
   (`vendor_normalize.rs:77-80` vs `amana_vbit.json`). Only 8 vbit rows, 2
   diameters, no plastic vbit beyond a single acrylic row.
2. **Plastics breadth**: HDPE, PC, Delrin have 1 row each (all flat_end). Acrylic
   carries the whole plastics category by extrapolation.
3. **Facing bit**: 8 rows, diameters 22/25 mm only, no plastic/metal, and no
   `ToolType::FacingBit` to drive it. Effectively orphaned.
4. **Vendor monoculture**: 60/67 rows are Amana; Onsrud 10, Whiteside 4, others 0.
   No Whiteside/Onsrud/Harvey/Garr/Sandvik breadth despite the `Vendor` enum.
5. **Diameter sparsity**: ball nose jumps 0.794→1.0→1.5→3.175→6.0; no rows above
   12 mm for any milling family. Common 1/8" (3.175) and 1/4" (6.35) only
   partially covered (6.0 stands in for 6.35).
6. **No roughing data for vbit/facing**; operation families skew to parallel/finish.

### Kc (specific cutting force)
7. **Plastics share Kc=4** — acrylic (brittle, low Kc) vs Delrin/PC (tough) lumped
   together (`material.rs:224`). Power/deflection on plastics is coarse.
8. **No aluminum Kc** — `Material` has no aluminum variant; the aluminum LUT rows
   are unreachable from the engine model.
9. **Anisotropy is a single 2.5× scalar** (`power.rs:36`) — no grain-direction or
   climb/conventional decomposition.

### Hardness
10. **Plastic hardness split across two places** with different values/granularity
    (`material.rs:189` flat 0.5 vs `vendor_normalize.rs:114-122` Shore D 65-85).
11. **Sheet/ply "effective Janka" are estimates**, not measured.

### Machine
12. **Only 3 presets, one calibrated kinematics profile.** Power model is a linear
    ramp, not a measured torque curve. No real machine library ships.
13. **Single K0/p/q across all machines/tools** (`machine.rs:39-41`).

### Drill
14. **Uncited first-pass thresholds**; plastics one bucket; no vendor drill charts.

---

## C. Perfect-world dataset spec

The existing `VendorObservation` schema (`vendor_lut.rs:102-146`) is already a good
target — it has source provenance (`source_url`, `accessed_on`, `source_page`),
evidence grade, hardness, diameter, flutes, RPM range, chipload range, ap/ae
ranges, op family, and pass role. **Acquire data that fills this schema.** Below,
per data type, the ideal granularity and concrete sources.

### C.1 Vendor LUT (chipload/RPM)
Target grid: **{6 tool families} × {12 material families} × {diameter series} ×
{flute count} × {operation family} × {pass role}**.

Per-family diameter series to cover (the common router-bit sizes):
- Flat/ball/bull: 1.5, 3.175 (1/8"), 6.35 (1/4"), 9.525 (3/8"), 12.7 (1/2") mm.
- V-bit: by **included angle** (30/60/90/120°) AND a small-tip-diameter set —
  **schema change needed**: add `included_angle_deg` and `tip_diameter_mm` fields
  so vbit rows are matched on geometry, not a fake "diameter". Then the
  engaged-width query can interpolate chipload along the engaged-width axis instead
  of scaling off a 6 mm fixed row. This is the single highest-leverage schema fix.
- Tapered ball: by tip radius (0.25-1.5 mm) and taper half-angle (1.5-7°).
- Facing/surfacing: 25, 38, 50 mm, but only worth it after a `ToolType::FacingBit`
  exists.

Fields each row needs (already in schema): `chipload_min/max_mm_tooth`, `rpm_*`,
`ap_min/max_mm`, `ae_min/max_mm`, `flute_count`, `evidence_grade`, `source_url`,
`source_page`.

**Sources to acquire (by tool family × material):**
- **Amana Tool** — already partially mined; complete their feed/chipload PDFs for
  all bit series (flat, ball, V, tapered ZrN, surfacing). Highest-grade (A).
- **Onsrud** — their "Feeds & Speeds" master chart is the best plastics + wood
  source; covers HDPE, acrylic, polycarbonate, Delrin explicitly with O-flute and
  upcut/downcut/compression geometry. Closes the plastics gap (#2).
- **Whiteside** — router-bit category pages; good for V-groove and ply.
- **Harvey Tool / Helical (Garr)** — for tapered ball, micro end mills (<1 mm), and
  aluminum. Closes the small-diameter (#5) and aluminum (#8) gaps.
- **CNC Cookbook / GWizard, IDC Woodcraft chipload chart** (CREDITS.md:147) —
  grade-C cross-checks for derived rows.

### C.2 Specific cutting force Kc (N/mm²)
Target: **per material family**, ideally with a **grain-orientation pair**
(parallel-to-grain vs perpendicular) for wood, and **per plastic** (split the
current single bucket).
- Wood/sheet: **USDA Forest Products Laboratory Wood Handbook** (already cited) —
  has shear-parallel and mechanical data to derive Kc per species; gives the
  anisotropy ratio to replace the global 2.5× with a real per-species pair.
- Metals/plastics: **Sandvik Coromant** and **Kennametal** machining-data handbooks
  publish `kc1.1` (specific cutting force at 1 mm chip) plus the `mc` chip-thickness
  exponent — the right model is `kc = kc1.1 · h^(-mc)`, a small **schema add**
  (`kc1_1`, `mc`) replacing the flat per-material constant.
- Plastics specifically: **Plastics machining guides** (Curbell, Boedeker,
  Professional Plastics) and **MatWeb** for acrylic/HDPE/PC/Delrin to split Kc and
  give per-plastic Shore D + machinability.

### C.3 Hardness / material properties
- Unify the two plastic-hardness locations onto one `Material`-level table:
  Shore D + Janka(wood) + HB(metal), keyed by `HardnessKind` (already exists).
- Sources: **Janka hardness database** (Wood Database / FPL) for wood; **MatWeb**
  for Shore D (plastics) and Brinell (aluminum); manufacturer datasheets for
  branded sheet goods (Trupan MDF, etc.).

### C.4 Machine data
- Schema is ready (`MachineProfile` + `MachineKinematics`). Ship a library of real
  machines: Shapeoko 3/4/Pro/XXL, X-Carve, Onefinity, Avid, Maslow, plus common
  spindles (1.5/2.2 kW air-cooled VFD, Makita/DeWalt routers).
- **Power/torque curves**: replace the linear ramp with measured spindle
  power-vs-RPM curves — sources are VFD/spindle datasheets (Huanyang, Mechatron,
  Teknomotor) and community dyno threads. Add a `power_curve: Vec<(rpm, kw)>`
  variant to `PowerModel`.
- **Kinematics**: harvest `$$` (GRBL `$110-$122`) values per machine from community
  configs — already validated as the right approach (`machine_kinematics.rs:104`).
  Add per-axis accel and real jerk where controllers expose it.
- Replace the single K0/p/q with per-machine (and ideally per-tool-family) fits once
  enough LUT rows exist to regress them.

### C.5 Drill data
- Per-material peck depth, chip-welding D/d, plunge feed/dia — split plastics into
  acrylic/HDPE/PC/Delrin (drilling behavior diverges sharply).
- Sources: **Onsrud and Amana drilling/plunge charts**, **plastics drilling guides**
  (Curbell/Boedeker have plastic-specific peck and feed recommendations), twist-drill
  handbooks for the wood-vs-plastic point-geometry differences.

---

## Provenance already on record (CREDITS.md)
- Vendor feeds: Amana, Onsrud, Whiteside, Sandvik Coromant, IDC Woodcraft,
  Carbide3D community (CREDITS.md:124-148).
- Material/formula: USDA FPL Wood Handbook, Sandvik Coromant milling formulas, GARR
  references for power/chip-thinning (CREDITS.md:157-176).
The `Vendor` enum already includes Amana/Onsrud/Harvey/Whiteside/Sandvik/Garr/
Autodesk/Carbide3d — the acquisition just needs to populate the families that are
declared but empty (Harvey, Garr, Sandvik, Autodesk, Carbide3d all have 0 rows).
