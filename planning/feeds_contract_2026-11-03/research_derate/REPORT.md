# Derate audit: from the vendor row to the shipped number

Author: derate-audit agent, 2026-11-05. Read-only research. HEAD `0f55efda4`. Every
`path:line` is at that hash. Each claim id in brackets, for example [A01], has one assert in
`assert_claims.py`. Run `python3 -I assert_claims.py`: 61 claims, all pass. The script reads the tracked files at `0f55efda4` (`git show`). The page
`index.html` shows the same content as diagrams.

Short paths: `mod.rs` = `crates/rs_cam_core/src/feeds/mod.rs`, `machine.rs` =
`crates/rs_cam_core/src/machine/mod.rs`, `gd.rs` =
`crates/rs_cam_core/src/compute/geometry_default.rs`.

## 0. Answer

1. The vendor-backed feed and rpm path has no light-machine factor. Feed = rpm x chip x Z x the
   printed vendor depth tier (`mod.rs:2679`). The long-tool share and chip thinning are measured
   but not applied [E01]. The machine enters only as limits: rpm range, power curve, cutting-feed
   cap. They clamp by `min`; they do not multiply.
2. 5 of the 42 Shapeoko table cells write a shipped number, all of them a stepover. 0 cells write
   a shipped depth, and 0 cells touch a shipped feed [A07].
3. The concern holds for geometry. Light-machine margins sit in four places: the Shapeoko table,
   machine-blind typed constants (slot cap, add-door axial k, stale check, predictor),
   `RigidityProfile` (itself a Shapeoko port [B08]), and the LUT data filter [F03].
4. The margins combine mostly by `min`. One product of machine x table exists (`mod.rs:4078`);
   it cancels to 1 on every reached row [C07].
5. A multiplied double derate exists on the 60 FM1 cells with no vendor chip: the Shapeoko chip
   formula times the Shapeoko surface speed [E05]-[E08].
6. Lineage: each stored number has one stamp and no tree. The in-memory `Basis` tree holds no
   numbers, and no surface prints it for the speeds [G01]-[G09].

## 1. The table and what reaches it

- `operation_default_profile` (`mod.rs:3513`): 21 rows, 42 factor cells. Source Unsourced, "DOC/WOC
  matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)" (`mod.rs:3503`) [A01].
- 25 registry rows reach 8 (family, role) pairs plus Drill. No row has the feeds family `Face`;
  the Face op uses `Pocket` [A02]. 16 cells sit on a reached pair; 26 cells (6 Face, 20 on unused
  roles) reach nothing [A03].
- The calculator reads the table only on an axis that the op does not state
  (`compute/catalog/cut_spec.rs:246`) [A06]. Every reached ap factor except Adaptive Roughing is at
  or below 1.0, so the depth tier stays 1.0 [A05].
- The only door to a shipped number is `default_radial_width` (`mod.rs:3954`), the add-door
  stepover. Five cells: Pocket R 0.35, Trace F 0.02, Parallel F 0.03, Scallop F 0.025, Contour F
  0.05 [A04], [A07]. Adaptive R ae (0.12) is replaced on flat, bull, ball and tapered tools.

## 2. The chain per family

### 2.1 Speeds steps (every milling family)

| Step | Site @0f55efda4 | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| S1 | `mod.rs:2041` | rpm = SFM x 1000 / (pi De) | wood 200 m/min (`material/mod.rs:735`) | Unsourced, Shapeoko port [E04] | `spindle` clamp |
| S2 | `mod.rs:2332` | vendor rpm replaces S1 | 18 000 (Amana) | Printed | `spindle` clamp |
| S3 | `mod.rs:2398` | MaxSpeed x <= 1.5 | `mod.rs:374` | Unsourced [E10] | `spindle` |
| S4 | `mod.rs:2439` | milling rpm tier | 22k..14k by D | Unsourced [E10] | no |
| S5 | row, or `mod.rs:2106` | chip | row (G1/G2/G3) or 0.024 D^0.61 (1/H)^1.26 | Printed / Unsourced "Shapeoko empirical data" [E03] | `chip_load` (same in all presets) |
| S6 | `mod.rs:2677` | depth tier | 1 / 0.75 / 0.5 at 1/2/3 D (`geometry.rs:155`) | Printed [C08], [D04] | no |
| S7 | `mod.rs:2679` | feed = rpm x chip x Z x S6 | | law | no |
| S8 | `mod.rs:2736` | power ladder | `power_at_rpm` | machine | `power` |
| S9 | `mod.rs:3067` | feed cap, arms L/S (Q1) | | machine | `max_cutting_feed_mm_min` |
| S10 | `mod.rs:3201` | plunge G10, ramp G6 | family fraction x feed | Printed claims | via S9 |
| S11 | `mod.rs:1393`, `:2693` | rubbing floor 0.025, long-tool share | warn only | Unsourced [E01], [E02] | no |
| S12 | `feeds/suggest/adaptive_entry.rs:228` | pass 9: feed x S6(kept)/S6(calc) | | law | no |

### 2.2 Clearing: Face, Pocket, Zigzag, Inlay plug, Bore (Pocket, Roughing)

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| stepover | `mod.rs:3529`; stamp `gd.rs:296` | ae = 0.35 D | [A04] | Unsourced | no (`woc_roughing_factor` unread [B01]) |
| depth 1 | `gd.rs:485` | 1.5/6 D = 0.25 D | [D06] | Unsourced | no |
| depth 2 | `feeds/suggest/invariants.rs:206`, `machine.rs:143` | min with `doc_roughing_factor` x D | | Unsourced | yes [B07] |
| depth 3 | `mod.rs:2567`, `feeds/envelope.rs:1262` | slot cap 0.25 D where ae > 0.85 D | [D01] | Unsourced | no |
| depth 4 | `mod.rs:1838` | flute 0.8 x L | [D07] | Unsourced | no |

FM1 6 mm 2F hardwood Pocket: stepover 2.1, depth 1.5, chip 0.1270, feed 3999 [C05], [D06], [E06].

### 2.3 Profile (Contour, Roughing; ae = D)

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| width | `cut_spec.rs:246` | ae = D | [D02] | geometry | no |
| depth 1 | `gd.rs:491` | 2/6 D | | Unsourced | no |
| depth 2 | `invariants.rs:206` | `doc_roughing_factor` x D | | Unsourced | yes |
| depth 3 | `mod.rs:2567` | slot cap 0.25 D | | Unsourced | no |

### 2.4 Adaptive: 2D Adaptive, 3D Adaptive (Adaptive, Roughing)

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| ae 1 | `mod.rs:3516` | table 0.12, replaced | | Unsourced | no |
| ae 2 | `mod.rs:3777` | `ADAPTIVE_FLAT_AE` 0.14 (ball 0.10, tapered 0.08) | [C01] | Unsourced | no |
| ae 3 | `mod.rs:3812` | x 0.85 at >= 3 flutes | | Unsourced | no |
| ae 4 | `mod.rs:3832`.. | x material scale; white oak medium 0.95 -> 0.133 | [C03] | Unsourced | no |
| ae 5 | `mod.rs:4104`, `:4106` | wood + flat/bull: = `adaptive_woc_factor`; else min | 0.20 / 0.25 [C02] | Unsourced | yes |
| ae 6 | `gd.rs:296` | stamp names the 0.12 row | [C06] | | |
| ap 1 | `gd.rs:491`, `:498` | 2/6 D (2D), 3/6 D (3D) | [C09] | Unsourced | no |
| ap 2 | `machine.rs:143` | min with `adaptive_doc_factor` x D | 1.5 / 2.0 D | Unsourced | yes |
| ap 3 | `mod.rs:4078` | floor `adaptive_doc x 1.50 / 1.50` (unstated ap only) | [C07] | Unsourced | yes |

rivmap100 3D Rough (6 mm, white oak, `adaptive_woc_factor` 0.2) ships stepover 1.2 [C04].

### 2.5 Finishing: Waterline, Steep/Shallow (Contour); Parallel, Ramp, Radial, Horizontal (Parallel); Scallop, Unified, Spiral (Scallop)

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| stepover | `gd.rs:548`, `:550`, `:557`; `mod.rs:3872` | chord on a ball tip, else table ae | 0.05 / 0.03 / 0.025 | chord: CREDITS; k: Unsourced | no |
| z_step / stepdown | `gd.rs:479`, `:472` | 1/6 D, 0.5/6 D | | Unsourced | no (no finish cap) |

Flat end mills on Ramp, Radial, Horizontal take the formula chip (S5) [E05].

### 2.6 Trace and engrave: VCarve, Inlay, Trace, Chamfer, Pencil, Project Curve (Trace, Finish)

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| stepover | `gd.rs:546`, `:564` | table ae 0.02 D | | Unsourced | no |
| depth | `gd.rs:472` | 0.5/6 D | | Unsourced | no |

FM1 6 mm 2F hardwood Trace: formula chip 0.0459, rpm 10 610, feed 973 [E06].

### 2.7 Drill, Pin Drill

| Order | Site | Multiplier or clamp | Value | Source | Machine |
|---|---|---|---|---|---|
| rpm | `mod.rs:1532`.. | drill tiers by D | | Unsourced | `spindle` |
| chip | G6 `DRILL_RULES` | side row x 1/Z | | Printed claim | no |
| peck | `gd.rs:498` | 3/6 D | | Unsourced | no |

## 3. Margins applied twice

| # | Places | Combine | Named case | Factor |
|---|---|---|---|---|
| M1 | Shapeoko chip formula (`machine.rs:47`) x Shapeoko SFM (`material/mod.rs:735`) | product | FM1 6 mm 2F hardwood Trace against the vendor roughing row `amana-flat-hardwood-pocket-6000-2f-spektra` (0.127 mm, 18 000 rpm). The vendor prints no finish or trace chip for this tool; the formula is the only source for that cell | chip 0.361; feed 973 / 4572 = 0.213 [E06]-[E08]; 60 FM1 cells [E05] |
| M2 | `SLOTTING_DOC_CAP` (`mod.rs:1895`) and `doc_roughing_factor` | min | Profile 6 mm: generic 1.2 mm; Shapeoko presets and nz-wanaka-21 1.5 mm; a machine at 0.5 D is still held at 1.5 mm | 0.20 / 0.25 of 1 x D; the constant overrides the machine [D01]-[D03], [B05] |
| M3 | vendor 1 x D, add-door k, rigidity cap | min | 6 mm hardwood Pocket, generic: vendor 6 mm; the Shapeoko vendor's own hardwood print (Carbide 3D chart, Mahogany .1" on .25") 0.4 D; add door 1.5 mm; cap 1.2 mm. The FM1 "hardwood" is Hard Maple, a different species | 0.20 of vendor; 0.5 of the Shapeoko chart [D04]-[D06] |
| M4 | `mod.rs:4078` and `machine.rs:143` | two formulas, one depth | 1.5 x 1.50 / 1.50 = 1.5 (generic); 2.0 (default) | 1.0 while the table row equals the divisor [C07] |
| M5 | adaptive width chain | product, then replace | white oak 2F: 0.133 -> 0.20 | machine raises it; product under a ceiling for ball, tapered, non-wood [C01]-[C03] |
| M6 | table 0.35 and `woc_roughing_factor` | field unread | nz-wanaka-21 and the operator file: 0.4 ("40-60 % of D", machine-local) | no effect [B01], [B05], [B06] |
| M7 | `industrial_only/` filter and the power curve | filter, then min | Freud 1/2" 0.46-0.69 mm/tooth kept out "on a Shapeoko-class router" | rows absent [F03] |
| M8 | typed width copies | duplicate | `validate.rs:277` 0.15 D, `:284` 0.20 D; `predict.rs:260` 0.35, `:267` 0.3 | machine-blind [F01], [F02] |

Checked, not a double derate: IDC Woodcraft and Whiteside Fusion 360 rows are embedded; 0 FM1
cells ship from them [F04]. The Amana adaptive ae bands are repo-authored [F05].

## 4. Proposal: one place for the machine margin

| Constant today | Becomes | P11b overlap |
|---|---|---|
| table ap column | delete; P10 axial rows and the envelope own depth; only `mod.rs:4078` reads it | none |
| Pocket R ae 0.35 | `woc_roughing_factor` (exists, unread) | L8 |
| Adaptive ae 0.12, `ADAPTIVE_*` width rows | delete; `adaptive_woc_factor` is the one width | L8 |
| finish and trace ae | scallop chord; else a machine-neutral finish k (Unsourced) | L8 |
| Face rows | delete (unreached) | none |
| `ADAPTIVE_RIGIDITY_DOC_DIVISOR` and the floor | delete; `depth_cap_mm` is the one adaptive depth | none |
| `SLOTTING_DOC_CAP` | `RigidityProfile::slot_doc_factor` (presets 0.25) | L8 |
| `SLOTTING_THRESHOLD` | machine-neutral geometry, Unsourced | L8 |
| `doc_roughing_factor` | a fraction of the vendor depth; Shapeoko source: the Carbide 3D chart | L6 |
| `doc_derating_scale` | a visible vendor limit | L6 |
| `AXIAL_K_*` | add-door depth = the envelope depth cap | P10 reviewer item |
| `ChipLoadFormula` | off the machine; a vendor-row law (G1), else warn (D7) | L8 |
| `SPEED_*`, rpm tiers, `MAX_SPINDLE_SPEEDUP` | vendor rpm / source hunt | L8 |
| `validate.rs`, `predict.rs` copies | read `RigidityProfile`, `default_radial_width` | L8 |
| `doc_finishing_factor`, `woc_roughing_max_mm`, `woc_finishing_mm` | delete or give a reader | none |
| `industrial_only/` filter | load the rows; K7 power is the per-spindle gate | operator |

## 5. Lineage

### 5.1 Depth today

`Basis::Derived` holds a law and children, no numbers (`feeds/sourced.rs:121`) [G01].

| Field | Records in memory | Leaf | No record |
|---|---|---|---|
| rpm | 0 (vendor) to 5 | Printed row / Rule SFM | clamp values; `Machine` leaves carry no value [G02], [G03] |
| chip | 0-1 | Printed / Claim | G2 scale value; Janka scale in the formula chip [G08] |
| feed | 1 + chip (+1 power, +1-2 cap) | via chip | depth tier value, Z, pass 9 [G09] |
| plunge, ramp | 1-2 | G10 / G6 | |
| stepover (add door) | 3 (white oak 2F Adaptive) | `ADAPTIVE_FLAT_AE` + `Machine(Rigidity)` | stamp names the 0.12 row [C06] |
| depth (add door) | 1 | `AXIAL_K_*` | |

The stored stamp is one (source, reference) pair (`feeds/provenance.rs:152`) [G04]. `Lineage::of`
reads stamps only (`trace/lineage.rs:336`) [G05]. The envelope grid strips each basis
(`feeds/envelope.rs:373`) [G06]. rivmap100 CLI output: only the proposal tree, depth 1, no number
[G07]. MCP `get_toolpath_lineage` and `get_suggest_rationale` exist; CLI `suggest` prints no bases
[G12]. Stored depth: 0 records for every field.

### 5.2 Design

1. Each multiplier writes one step: value in, factor (a leaf with its value), value out, law.
   `moved_basis` already has before and after.
2. Split `MachineField::Rigidity` per field, with the value.
3. Add the missing leaves: depth tier value, Z, Janka scale, G2 scale, pass 9.
4. Persist the tree beside the stamp. `Basis` has no `Deserialize` [G13]: write name and value;
   resolve rule names through `formula_constants()` on read.
5. Expose `speeds.<field>.tree` on `get_toolpath_lineage`; `bases` on `get_suggest_rationale` and
   CLI `suggest --json`.
6. Compiler rule: the step constructor needs the three numbers.

Size [G10], [G11]: 69 lines in 6 core files call `Basis::derived(` / `moved_basis(`;
`ValueProvenance` in 18 files; `Basis::` in 63 files; `MachineField::Rigidity` on 4 lines; MCP
`rs_cam_viz/src/app/mcp/generation.rs`, `rs_cam_mcp/src/server.rs`; CLI `suggest.rs`,
`project.rs`. Estimate 10-12 files, about 90 sites. Other cited lines: [H01].

## 6. Open questions for the operator

1. Should the Shapeoko depth margin be a fraction of the vendor depth in `RigidityProfile`, sourced from the Carbide 3D chart, in place of 0.20 / 0.25?
2. Should `woc_roughing_factor` drive the Pocket stepover in place of 0.35?
3. Should the slot cap move into `RigidityProfile`?
4. For the 60 formula-chip cells: keep the formula, warn (D7), or use a vendor-row law?
5. Should the `industrial_only/` rows be loaded now?
6. CORRECTED by `RIGIDITY_DEFAULTS.md` (2026-11-05): a machine table with no rigidity block does not load; `rigidity` has no serde default [B03]. `RigidityProfile::default()` (0.25 / 2.0 / 0.25) reaches the product only through the two Shapeoko presets; `MachineProfile::default()` (0.20 / 1.5 / 0.20, `generic_wood_router`) reaches every door with no machine table and FM1 [B02]. Both sets are Unsourced ports from one commit.
7. Should lineage be stored in the project file, or recomputed on read?
8. Side flag, not checked: the `Face` feeds family is unreached, so `vendor_normalize.rs:175` and the 8 `amana_facing.json` rows are probably unused.
