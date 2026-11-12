# P29-8 design: an entry is a declared cut, and one entry rule set serves both emitters

Written 2026-11-12 on HEAD `9fa97dbed`. No product code changed: the prototype edit was restored
from byte copies (`sha256sum -c` OK), and the probe test was deleted (section 15). The lead reviews
this note, commits it with the two prediction files, and then starts an implementer. The result of
each commit is judged against section 10. A mismatch stops the work.

Files of this step:

- this note;
- `P29-8_predictions_fm1.csv`: every FM1 cell that moves, for each branch of the two operator
  rulings and for the rejected depth-ceiling reading of the entry cut. Every row is measured
  (prototype runs, section 10.1);
- `P29-8_predictions.csv`: tables `G` (the wanaka operator file, each op and its entry, both plunge
  branches), `W` (both wanaka files), `R` (the rivmap100 files), `S` (Suggest ramp feed of a new
  op at each candidate angle, default and operator machine), `H` (hand checks).

Rulings in force: R4 (every cut group is judged), R6 (the operator, 2026-11-11: "it should be one
bit of code shared for the different tool paths. the 2d op one sounds most dialed", defaults from
printed sources), and the lead rulings of 2026-11-12 on the stop report:

- a stated angle or pitch stands; the shape is "stated or rule"; an existing file loads as stated;
- the two hard-coded 3 degree fallback ramps read the op's own ramp angle;
- the entry cut is a slope cut on the planned side; the value-check reading is the lead's default;
- the modulator hold: the sample side does not change; `P28-30` moves to the modulation step.

Two questions go to the operator (section 7, O1 and O2). This note predicts every branch, so no
run is needed after the answer. Every finding warns; no job is refused.

The acceptance (the operator, 2026-11-12): "I want to leave the code in a place where its
blatently obvious to anyone in the future that adding a mechanism like force neds to happen acreoss
all movement". For entries this means: an entry is a cut of the cut list, with an engagement, a
feed and a presence, so `limits::scope`, the planned driver and the gates judge it with no special
path; and an entry number (angle, pitch, radius, feed) has one home that every emitter reads.

## 1. Scope

### 1.1 The sources

| Source | What it puts in P29-8 |
|---|---|
| Change list (`P29_FINDINGS.md` section 6) | "Entries as declared cuts before the simulation: the inclined engagement; one entry rule set for both emitters; the dressup plunge reads the op plunge rate." Sentry: "one entry rule, two emitters". Ruling R6. |
| Rows whose `closes_in` names P29-8 | `P28-10`, `P28-21`, `P28-27`, `P28-28`, `P28-30`, `U-TabReplunge` (section 1.2) |
| Code comments that name P29-8 | `limits/cut.rs:285-287` (`NoEngagementModel::Ramp`, `Helix`), `compute/catalog/cuts.rs:120` (`DeclaredFeed::EntryRule`), `cuts.rs:206` (an entry has no model), `feeds/planned.rs:50` (`PlannedNoNumber::EntryRule`), `tool_load/groups.rs:54`, `compute/simulate.rs:1083`, `stock/sim_triage.rs:1270` |
| R6 research | `research/entry_rules_2026-11-11.md` and its stored sources `research/sources_entry_rules/` (17 files; `check_entry_rules.py` prints "all asserts pass") |
| Operator, 2026-11-11 night | the hardwood ramp angle is "probably a bit less than soft wood" |

The sources disagree in one place. The change list says "the dressup plunge reads the op plunge
rate". The stop report measured that this doubles five ops' entry feeds on the operator file
(section 3.3). The lead sent that to the operator as O1 with two branches; this note predicts both.

### 1.2 A verdict for each row

| Row | Verdict | Why |
|---|---|---|
| `U-TabReplunge` (the re-plunge after each tab of a Profile, no limit reads it) | stays; narrows in 8d | Measured on wanaka idx 17 (style Ramp, tabs): 42 `EntryPlunge` moves at the op plunge rate (1582) stay straight plunges at X 379.5, one at each end of a tab gap on each level (Z from the retract 27.5 down to the level, 23.75 to 1.5): the ramp dressup does not turn them into ramps. Cause not traced (they are not at the dressup feed `min(move, 791)`, so the dressup did not handle them). In 8d they are declared `Entry(Plunge)` cuts (`AxialPlunge`, value check), so a limit reads them; the row stays open for the open question of why no ramp replaces them (Q4). The `dressup/tabs.rs:218`, `:257` re-plunge writes the CUT feed of the interrupted move; on idx 17 no such move appears. |
| `P28-30` (the modulator skips a plunge) | re-file: `closes_in` "the modulation design step" | lead ruling (the modulator hold); the plunge now has a planned engagement (`AxialPlunge`), but the modulator is not widened |
| `P28-21` (the envelope finding of the simulation places the body group only) | re-file: `closes_in` "the modulation design step (the sample side)" | placing an entry sample is a sample-side change; the lead ruled that the sample side does not change in P29-8 |
| `P28-10` (spindle power is no limit on a drill cycle) | stays; `closes_in` "later (a drill-cycle power limit moves Drill cells; not measured)" | the plunge entry gets the axial power check (8d) through the Entry group; the drill cycle is the Drill group, whose power ladder and RPM limit are a different reader. Making it a limit moves FM1 Drill cells (6 cells hold a power today) and was not prototyped |
| `P28-27` (the tip plunge cap reads ball and tapered ball only) | stays; `closes_in` "later (no source)" | no stored source prints a plunge cap for a flat, bull or V tip (research note, table 1.4: D5 "no source found") |
| `P28-28` (the plunge-entry warning reads the Adaptive family only) | stays; `closes_in` "later (a plunge deflection model)" | R4 says the relative rule goes when the absolute one exists. The absolute plunge check of 8d is power and the vertical rate; the 0.5 D rule is a deflection transient, and `AxialPlunge` has no deflection model. The warning fires in 0 FM1 cells today (`grep -c PlungeEntryUnstableAtDpp` gives 0) |

## 2. What exists today

### 2.1 The emitters

Two doors call the shared emitters of `dressup/entry_descent.rs` (`emit_ramp`, `emit_helix`,
`emit_stock_aware_plunge`):

1. **The dressup door.** `compute/execute/dressup_apply.rs` (`ramp_entry` `:780`, `helix_entry`
   `:815`, `plunge_entry` `:867`) calls `dressup::apply_entry` (`dressup/mod.rs:420`), which finds
   each generator plunge and replaces it. Every op with `EntryContract::Dressup` takes it.
2. **The 3D Rough door.** `adaptive3d/path.rs` (`:2564-2620`) calls the emitters for each planner
   entry; `compute/execute/rough_3d.rs:105-117` maps the op config to `EntryStyle3d`.

Where the two doors differ today (HEAD line numbers):

| Quantity | Dressup door | 3D Rough door | Kind of difference |
|---|---|---|---|
| Ramp angle default | 3 deg, `compute/config.rs:981` `DRESSUP_RAMP_ANGLE_DEG` | 10 deg, `compute/operation_configs.rs:2001` `ADAPTIVE3D_RAMP_ANGLE_DEG` | a number by door, no source for either |
| Ramp angle read | `cfg.ramp_angle`, `dressup_apply.rs:788` | `cfg.ramp_angle_deg`, `rough_3d.rs:109` | two fields |
| Helix pitch default | 1 mm, `config.rs:987` | 2 mm, `operation_configs.rs:2008` | a number by door, no source |
| Helix radius | `DressupConfig::helix_radius_for` (`config.rs:1618`): operator value or `HELIX_RADIUS_OVER_D` x D | `Adaptive3dConfig::helix_radius_for` (`operation_configs.rs:2039`): `helix_radius_factor` x D | one constant (0.3), two fields |
| Fallback ramp (a helix with no room) | 3 deg, `dressup/mod.rs:518` | 3 deg, `adaptive3d/path.rs:2560` | the op's own angle is not read |
| Plunge feed (air lead, straight plunge, fallback plunge) | `min(move feed, 0.5 x first linear feed, else 500 mm/min)`, `dressup_apply.rs:746`, `entry_contract.rs:223-254` | `params.plunge_rate` (the op plunge rate), `path.rs:237`, `:2575`, `:2591`, `:2606` | two rules |
| Ramp and helix feed through material | `ramp_feed_rate_mm_min` (`dressup_apply.rs:727`) = the op `ramp_feed_rate` | `params.ramp_feed_rate` (`path.rs:2414`) | the same field |
| Ramp fold along the following cut | yes (`dressup/mod.rs:541-556`) | no, `dir = (1, 0)` into prism stock (`path.rs:2613-2617`) | geometry of the door (stays) |
| Helix floor lap | the op capability `helix_floor_lap` (2D Adaptive) | `false` (`path.rs:2423`) | an op capability (stays) |
| Peck ladder | none | `emit_peck_plunge` for style Plunge | an entry kind of the contract (stays) |
| Suggest's view of the angle | `feeds/ramp.rs:332-375` `entry_geometry`, two arms | the same | the reader mirrors the two fields |
| Card default text | `feeds/ramp.rs:954-960`, the two door constants | the same | |

### 2.2 Other writers of an entry intent

`rg "MoveIntent::(EntryPlunge|EntryRamp|EntryHelix|EntryApproach)"` over `src/`, product code:

| Site | What it writes | Verdict |
|---|---|---|
| `dressup/entry_descent.rs` (16 sites) | the shared emitters | in scope: the feeds and the angle come from the two doors |
| `dressup/mod.rs:1077`, `:1087` (`optimize_entry_descents`, G-ISOCLIPENTRY) and `finish/pencil/emission.rs:70`, `:397-412` | the rest ramp ladder, capped at `ENTRY_RAMP_MAX_ANGLE_DEG` = 12 deg ("the operator-set cap for the R1.0 tapered ball in white oak") | out of scope: an operator-set cap of a crease ramp, not an entry default. Finding F1 |
| `ops/waterline.rs:843` `lap_count` | reads `DRESSUP_RAMP_ANGLE_DEG` as the descent slope of a helical waterline ring | out of scope: a cut path, not an entry. 8a gives it its own named constant of the same value (byte-equal). Finding F2 |
| `dressup/tabs.rs:218`, `:257` | the tab re-plunge, at the cut feed | in scope as `U-TabReplunge` (Q4) |
| `adaptive3d/path.rs:216-237` | the peck ladder at `plunge_rate` | in scope (already the op plunge rate) |
| `finish/surface_link.rs:294`, `:1255`, `:1263`; `finish/steep_shallow.rs:488`; `finish/scallop/research.rs:633`, `:712`; `finish/spiral_finish.rs:241`; `toolpath.rs:435`, `:490`, `:896`, `:915`, `:1035`; `geometry/boundary.rs:494-536` | the generator plunge (`NoneEntry::GeneratorPlunge`) at the op plunge rate | already the plunge rule; no change |

### 2.3 The entry cut today

`OperationConfig::cuts()` declares one cut per entry kind of the contract
(`compute/catalog/cuts.rs:569-571`), with no width and no depth. `DeclaredCut::engagement` gives
`NoModel(Ramp)`, `NoModel(Helix)` or `NoModel(NoStatedDepth)` (`cuts.rs:245-249`). The feed rule
is `DeclaredFeed::EntryRule`, and `PlannedCut::chip_mm` gives `Err(EntryRule)` (`planned.rs:219`).
So `limits::scope` judges the Entry group, but every planned limit answers "no number".

## 3. Measurements on the operator file (today)

### 3.1 Method

A throwaway probe test loaded `nz_wanaka_21_operator.toml` read-only through `ProjectSession::load`
and generated each fresh op through `ProjectSession::generate_toolpath` (no simulation). It wrote
every move (`Move` Debug text) to the scratchpad. A parser read the entry moves: intent, feed,
and the slope of each linear entry move. The rest ops idx 2 and 15 need their ancestors
simulated first; section 3.4 says what carries over.

Generation time, debug build: idx 1 86 s, idx 9 117 s, idx 14 102 s, idx 16 117 s; every other
op under 2 s.

### 3.2 The entries of each op today

| idx | Op | Tool | Entry style | Entry moves today |
|---|---|---|---|---|
| 0 | Face back | EndMill 18 | dressup Ramp, 3.0 | 60 `EntryApproach` at 1081 |
| 1 | Back bulk rough | EndMill 18 (core 8 mm) | 3D Helix, pitch 2.0, factor 0.3 | 3 `EntryApproach` at 2162; 561 `EntryHelix` at 6448, max slope 3.378 deg; 9944 `EntryRamp` (the fallback ramp) at 6448, 3.000 deg |
| 3, 4 | Pin drill, drill | EndMill 6 | none (descent is the cut) | none |
| 5 | River channel (Project Curve) | EndMill 6 | none | 156 `EntryPlunge` at 1347 |
| 6, 7 | Lake pockets | EndMill 2 | dressup None | 644 and 16 `EntryPlunge` at 240 |
| 8 | Antenna pocket | EndMill 2 | dressup Ramp, 3.0 | 9 `EntryApproach` at 120; 400 `EntryRamp` at 1258, 3.000 deg |
| 9 | V rivers (Project Curve) | VBit 3.175 | none | 172 `EntryPlunge` at 381 |
| 10, 13 | Trace V60 | VBit 6.35 | none | 3 and 31 `EntryPlunge` at 762 |
| 11 | Face clamp pads (Pocket) | EndMill 18 | dressup None, stock-aware plunge | 8 `EntryApproach` at 1081 |
| 12 | Face front | EndMill 18 | dressup Ramp, 3.0 | 2 `EntryPlunge` at 1081 (the fallback plunge where the run has no room, 1 mm, 26.0 to 25.0); 120 `EntryRamp` at 6447, 3.000 deg |
| 14 | Front bulk rough | EndMill 18 | 3D Ramp, 3.0 | 1 `EntryApproach` and 1 `EntryPlunge` at 2162; 8086 `EntryRamp` at 6448, 3.000 deg |
| 16 | Scallop finish | TaperedBall 2 | dressup None | 301 `EntryPlunge` at 240 |
| 17 | Outline profile | EndMill 6 | dressup Ramp, 3.0 | 19 `EntryApproach` at 791; 42 `EntryPlunge` at 1582; 511 `EntryRamp` at 3165, 3.000 deg |

The air lead and the fallback plunge of the dressup door run at half the op plunge rate on idx 0,
8, 11, 12 and 17: `1081 = 0.5 x 2162`, `120 = 0.5 x 240`, `791 = 0.5 x 1582`. The first linear
move of each of these toolpaths is the generator plunge at the op plunge rate, so
`0.5 x first linear feed` is half of it.

### 3.3 Which entry moves are in material

An `EntryApproach` move is the air lead above the material read (`rapid_to_entry_top`; the K4
invariant of `dressup/CLAUDE.md`), so it cuts nothing. The 2 `EntryPlunge` moves of idx 12 descend
1 mm to the face depth (Z 26.0 to 25.0) at X 404.0 and 401.4, at the end of a raster row. Whether
they touch material was not proved (no simulation ran): the stock top in that setup is 25.5 (stock
`z = 25.5`), so the last 0.5 mm of each plunge can cut if the XY is over the stock. This note
counts them as moves that can touch material.

### 3.4 The rest ops idx 2 and 15

The prototype cannot generate them without a simulation of their ancestors (about 3 min for the
face and the rough of setup 1, per the P22a note; more for setup 3). What carries over from idx 1
and 14: both are 3D Rough ops with stated angles (3.0), stated pitches (2.0) and the 3D door,
whose plunge feed is already the op plunge rate. None of the four P29-8 changes reads anything
else of a 3D Rough: the angle and pitch are stated (they stand), the fallback reads the stated 3.0
(the old constant was 3.0), and the plunge branch (a) changes only the dressup door. So idx 2 and
15 are byte-equal by the same reasons as idx 1 and 14 (measured byte-equal). This is reasoned, not
measured.

## 4. The design

Four commits. The compiler holds each rule where it can; a sentry holds the rest.

### 4.1 One entry rule module (8a, byte-equal)

New file `crates/rs_cam_core/src/feeds/entry_rule.rs`. It owns every entry number, and it is the
only door from an op to the emitters:

```rust
/// The material class that the ramp-angle sources print (Easel, Shaper:
/// the angle is printed by material, not by tool; research note 2.4).
pub enum EntryMaterialClass { Hardwood, SoftwoodAndComposite, Plastic, NonFerrous }

/// Where the default ramp angle comes from: ONE constant, the operator's
/// answer to O2 (section 7). Each variant's numbers are cited table rows.
pub const RAMP_ANGLE_SOURCE: RampAngleSource = RampAngleSource::DoorDefaults;

pub enum RampAngleSource {
    /// Easel: 5 deg hardwood, 20 deg softwood, plywood, MDF.
    Easel,
    /// Shaper: ~20 deg oak and walnut, ~27 deg pine, cedar, plywood, MDF.
    Shaper,
    /// Today's numbers, Unsourced: 3 deg on the dressup door, 10 deg on
    /// the 3D Rough door (one table, a row per door).
    DoorDefaults,
}

/// The ramp angle of one op: stated by the operator, or the rule.
pub enum EntryAngle { Stated(f64), Rule }

/// The dressup plunge feed: ONE constant, the operator's answer to O1.
pub const DRESSUP_PLUNGE_FEED: DressupPlungeFeed = DressupPlungeFeed::FirstLinearFeedFraction;

/// Everything an emitter needs for one op's entries, resolved once.
pub struct ResolvedEntry {
    pub kind: Option<EntryKind>,           // None: no entry (style None, descent is the cut)
    pub ramp_angle_deg: Sourced<f64>,      // the stated value, or the table row
    pub fallback_ramp_angle_deg: f64,      // = ramp_angle_deg (lead ruling)
    pub helix: Option<ResolvedHelix>,      // radius (capped at the flat), pitch
    pub plunge_feed: PlungeFeed,           // DRESSUP_PLUNGE_FEED on the dressup door; the op plunge rate on the 3D door
    pub ramp_feed: Option<f64>,            // the op ramp_feed_rate
}

impl ResolvedEntry {
    pub fn of_dressup(cfg: &DressupConfig, op: &OperationConfig, tool: &ToolConfig, material: &Material) -> Self;
    pub fn of_adaptive3d(cfg: &Adaptive3dConfig, tool: &ToolConfig, material: &Material) -> Self;
}
```

What moves where:

- `DRESSUP_RAMP_ANGLE_DEG`, `ADAPTIVE3D_RAMP_ANGLE_DEG`, `DRESSUP_HELIX_PITCH_MM`,
  `ADAPTIVE3D_HELIX_PITCH_MM`, `HELIX_RADIUS_OVER_D`, `ENTRY_PLUNGE_FIRST_FEED_FRACTION`,
  `ENTRY_PLUNGE_FALLBACK_MM_MIN` move into `entry_rule.rs` as private `FormulaConstant` rows. The
  F201 list (`feeds/mod.rs:2342`) reads them from there. They are private: no emitter can name a
  number.
- `dressup_apply.rs` builds `DressupContext::entry: ResolvedEntry` (a required field, no
  `Default`); `ramp_entry`, `helix_entry` and `plunge_entry` read it and nothing else.
  `dressup::apply_entry` takes `&ResolvedEntry`; the `EntryStyle::Helix` fallback reads
  `fallback_ramp_angle_deg` (`dressup/mod.rs:518`).
- `rough_3d.rs` builds the same type with `of_adaptive3d`; `Adaptive3dParams` holds it;
  `path.rs:2560` reads `fallback_ramp_angle_deg`.
- `feeds/ramp.rs` `entry_geometry` and `entry_notes` read `ResolvedEntry` (the card says what the
  emitter does, from the same value).
- The session passes the stock material at both doors (it holds it: `session/compute.rs:1520`
  already sets `ExecutionContext::material`).
- `ops/waterline.rs:843` reads its own constant `WATERLINE_RING_DESCENT_DEG = 3.0` (F2).

Byte-equal: in 8a `RAMP_ANGLE_SOURCE` is `DoorDefaults` and `DRESSUP_PLUNGE_FEED` is
`FirstLinearFeedFraction`, so every number is today's. The fallback reads the op's own angle
(lead ruling): on wanaka every stated angle is 3.0, the old constant, so the output is equal
(measured: idx 1 and idx 12 byte-equal with the fallback switch on; section 10.3). FM1 is byte-equal
with the fallback switch on (measured, both files). A fallback moves only where a stated angle is
not 3.0 and a helix falls back (a geodesic 3D Rough with a helix and a stated ramp angle other
than 3.0, or a 2D Adaptive helix with containment): no FM1 cell and no wanaka op.

The compiler holds: a new emitter cannot get an entry number except through `ResolvedEntry`
(the constants are private; the context field is required). The sentry
`one_entry_rule_two_emitters_p29_8` holds: (1) a source scan (with `common/source_scan.rs`) that
no file outside `feeds/entry_rule.rs` names an entry constant or builds an `EntryStyle` from a
config field; (2) for a fixture op on each door, the angle, pitch, radius and plunge feed of the
emitted entry moves equal `ResolvedEntry` (the move slopes and feeds are read back).

### 4.2 Stated or rule, and the R6 table (8b)

The fields `DressupConfig::ramp_angle` and `Adaptive3dConfig::ramp_angle_deg` become
`EntryAngle`. Serde: a number loads as `Stated(v)` (every existing file); an absent key or the
token `"rule"` loads as `Rule`. A new op (the add door, `DressupConfig::for_op`,
`Adaptive3dConfig::default`) holds `Rule`. The writer saves a `Stated` value as the number and
omits `Rule`. So the wanaka files keep every angle (measured: every op byte-equal in the angle
branches, section 10.3). The setter, MCP and the GUI write `Stated`; a "use the rule" action
writes `Rule`. The helix pitch stays a number in this step (Q2).

`RampAngleSource` gives the table. One table with the class key; the `DoorDefaults` variant
answers by door:

| Class (from `vendor_normalize::material_to_lut`) | Easel | Shaper | DoorDefaults |
|---|---|---|---|
| Hardwood | 5 | 20 | 3 (dressup) / 10 (3D) |
| Softwood, plywood (both), MDF, HDF, particleboard | 20 | 27 | 3 / 10 |
| Plastics (acrylic, HDPE, polycarbonate, Delrin, fibreglass) | 10 (Shaper; Easel prints none) | 10 | 3 / 10 |
| Non-ferrous (aluminium) | 5 | 4 | 3 / 10 |

Sources (file and line, all under `research/sources_entry_rules/`):

- Easel: `easel_toolpath_ramping.txt:13`, "We recommend using the 20° angle ramp for soft woods,
  plywood, and MDF like materials. The shallower 5° angle ramp is good for hardwoods and soft
  metals like aluminum."
- Shaper: `shaper_autopass_material_recommendations.txt:27-34` (Hardwoods (Oak, Walnut), 6 mm
  2-flute up-cut, "10ipm per min (~20°)"), `:35-42` (Softwoods, "14ipm per min (~27°)"), `:43-50`
  (Composites (Plywood, MDF), "~27°"), `:59-` (acrylic, "~10°"), `:19-26` (brass, aluminium,
  "~4°").
- **The Shaper caution.** The same rows print a plunge speed and an "Auto Speed", and the printed
  angle does not follow from them. Hardwood: plunge 250 mm/min (`:30`), auto speed 250 mm/min
  (`:32`), `atan(250 / 250)` = 45.0 deg, printed ~20 deg (`:31`). Softwood: plunge 350 mm/min
  (`:38`), auto 250 mm/min (`:40`), `atan(350 / 250)` = 54.5 deg, printed ~27 deg (`:39`). The
  page does not define the relation (research note 1.1, note on A2-A5; `check_entry_rules.py`
  item 7 asserts the 45.0 deg).

The card (`entry_notes`) prints the source of the angle: `Ramp angle 5.00°: the rule for
hardwood (Easel)`, or for a stated value `Ramp angle 3.00°: operator value (the rule for
hardwood: 5.00°, Easel)`. This wording is part of 8b in every branch, so the `DoorDefaults` branch
also moves the 126 `entry_notes` cells (section 10.1). The rows of each branch are F201 rows:
Easel and Shaper rows are `Cited` (the file and line); the `DoorDefaults` rows stay `Unsourced`
(the pin does not move in any branch: the two old constants become two rows).

The helix entry is not in O2. The two pitch defaults (1 mm, 2 mm) stay as two `Unsourced` rows by
door (Q2).

### 4.3 The dressup plunge feed (8c, operator O1)

One constant, `DRESSUP_PLUNGE_FEED`:

- **Branch (a), `OpPlungeRate`:** the dressup door feeds the air lead, the stock-aware plunge and
  the fallback plunge at the op plunge rate (the value Suggest writes through the one
  vertical-rate limit, P20b). The two doors then read one plunge rule. `PlungeFeedRule::
  FirstLinearFeedFraction`, `ENTRY_PLUNGE_FIRST_FEED_FRACTION` and `ENTRY_PLUNGE_FALLBACK_MM_MIN`
  are deleted; the F201 Unsourced pin falls by 2 (section 6). A `DressupContext` with no op plunge rate
  (a test caller) gets none: `plunge_rate_mm_min` becomes a required `f64` (`rg -c
  "DressupContext \{"`: 32 literal sites in 15 files state one).
- **Branch (b), `FirstLinearFeedFraction`:** today's rule stays, as a named row of
  `entry_rule.rs` (`Unsourced`, 0.5 and 500 mm/min). No number moves; the pin stays 141. The
  dressup door and the 3D door keep two plunge rules, and `entry_rule.rs` names the difference.

In branch (b) commit 8c is empty and is not made.

### 4.4 The entry is a declared cut (8d)

The entry cut gets an engagement, a feed and a presence, so the planned driver judges it like any
cut.

```rust
// limits/cut.rs: NoEngagementModel loses Ramp and Helix.
// compute/catalog/cuts.rs
pub enum DeclaredDepth {
    Stated(f64),
    /// The cut's own depth, not the body depth (an entry): judged as a value
    /// check, never a ceiling of the body plane.
    Own(f64),
    None(NoEngagementModel),
}
```

The entry cuts of an op, with `CutContext` (the tool) and the resolved entry:

| Kind | Width | Depth | Feed | Engagement |
|---|---|---|---|---|
| `Entry(Ramp)` | `Slot` (D) | `Own(depth per pass)`: the ramp takes the full material depth (ruling 2026-09-24), and its last lap is a slot of that depth | `EntryRule` = the op `ramp_feed_rate` | `Side`, a slot at a slope (the advance per tooth along the path; the slope is under 30 deg on every default, so the path chip is within `cos 30 = 0.87` of the horizontal chip; the model takes the path chip, the larger) |
| `Entry(Helix)` | `Slot` | `Own(pitch)`: one lap engages one pitch at the leading edge (the Bore rule of P29-5b) | `EntryRule` = `ramp_feed_rate` | `Side`, a slot of depth `pitch` |
| `Entry(Plunge)`, `Entry(PeckLadder)` | none | none | `EntryRule` = the plunge feed of the door | `AxialPlunge` (flat) or `NoModel(PlungeOfShapedTip)` (`CutEngagement::axial_plunge`) |

Presence: the kind that `ResolvedEntry::kind` names is `Always`; every other kind of the contract
is `MayOccur` with the new row `U-EntryFallback` ("a helix that has no room falls back to a ramp,
a ramp with no run to a plunge"). This is a generation fact, so the row stays open. `cuts()` then
reads the dressup config: `OperationConfig::cuts(&self, entry: Option<EntryKind>)`, the one place
where the configured kind enters (`rg "\.cuts\(\)"` gives 32 call lines; each holds the toolpath
config, or passes the registry default).

`PlannedCut` gains `depth: PlaneDepth { Body, Own(f64) }`. A depth ceiling (`lowest_always`) reads
only `Body` cuts. A cut with `Own` depth gets one check per planned limit: the force budget at its
own width and chip gives the root `r`; the check is `Within` when `r >= own depth`, else `Exceeds`
(value = own depth, bound = `r`). The same for the deflection root. Power and the chip checks read
the cut as they read any cut. So the body proposal never moves (FM1: 0 cells, reasoned: section 15 says this reading was
not prototyped), and every
entry shows on the record (`SafeEnvelope::cut_checks`).

The rejected reading (Q1): the entry as a depth ceiling of the body plane (a slot at the body depth
at the entry chip, `Presence::Always`). Measured: 97 FM1 cells in 50 rows (Face and 2D Adaptive);
it lowers Face proposals by up to 79 % (TaperedBall 6, hardwood: 14.34 to 3.09 mm), because a Face
declares no slot today and its ramp would act as one. A ramp entry is a short cut, and the entry
feed is a separate number the operator can lower; it should not set the depth of the whole pass.

## 5. Commits

Each commit passes the gates alone (fmt, the named sentries, core `--lib`, viz `--lib` when no GUI
runs, FM1, full clippy).

| Commit | What | Moves a number? |
|---|---|---|
| 8a | `feeds/entry_rule.rs`, `ResolvedEntry`, both doors and Suggest read it; the fallback reads the op's angle; waterline gets its own constant; five entry constants become F201 rows (pin 141 to 146); sentry `one_entry_rule_two_emitters_p29_8` | No (FM1 byte-equal, wanaka byte-equal; measured) |
| 8b | `EntryAngle` (stated or rule), `RAMP_ANGLE_SOURCE` set to the operator's answer, the card wording | FM1 `entry_notes` 126 cells in every branch; `ramp_feed_mm_min` and `ramp_arm` 70 + 70 (Easel) or 115 + 115 (Shaper) or 0 (DoorDefaults). Wanaka: none |
| 8c | branch (a) only: `DRESSUP_PLUNGE_FEED = OpPlungeRate`, delete the fraction rule | G-code of wanaka idx 0, 8, 11, 12, 17 (table `G`); FM1 0 cells (measured); Unsourced pin minus 2 (section 6) |
| 8d | the entry as a declared cut: `DeclaredDepth::Own`, `PlaneDepth`, entry engagements, presence by the configured kind, row `U-EntryFallback`; narrow `U-TabReplunge`; re-file `P28-21`, `P28-30` | No FM1 cell (value-check reading); new `cut_checks` rows |

## 6. Exemption rows and pins

Today: ROWS 39, MAX_NEEDS_RULING 28, ROWS_WITH_A_FILE 38.

| Commit | Rows | ROWS | MAX_NEEDS_RULING | ROWS_WITH_A_FILE |
|---|---|---|---|---|
| 8a | none; `P28-30` `where_` stays | 39 | 28 | 38 |
| 8b | none | 39 | 28 | 38 |
| 8c | none (the plunge feed is no exemption) | 39 | 28 | 38 |
| 8d | add `U-EntryFallback` (NR, `where_` `src/compute/catalog/cuts.rs`); narrow `U-TabReplunge` (text: "the tab re-plunge stays a straight plunge under a ramp or helix style; cause open", still NR, no file); re-file `P28-21` and `P28-30` (text only) | 40 | 29 | 39 |

If the implementer finds and fixes the cause of the unconverted tab re-plunges in 8d (Q4), the row
closes: ROWS 39, MAX_NEEDS_RULING 28, ROWS_WITH_A_FILE 39. That fix changes the G-code of idx 17
(the 42 plunges become ramps) and needs its own prediction run; this note does not predict it.

The F201 Unsourced pin (`PINNED_UNSOURCED_COUNT`, 141 today; the five entry constants of 8a are
not F201 rows today: `rg` finds only `ENTRY_PLUNGE_FIRST_FEED_FRACTION` and
`ENTRY_PLUNGE_FALLBACK_MM_MIN` in `formula_constants()`):

| After | DoorDefaults | Easel or Shaper |
|---|---|---|
| 8a (the 3 deg, 10 deg, 1 mm, 2 mm and 0.3 rows join the F201 list as `Unsourced`) | 146 | 146 |
| 8b (only the active source's rows are listed; Easel and Shaper rows are `Cited`; the two door ramp rows leave) | 146 | 144 |
| 8c branch (a) (the two plunge rows go) | 144 | 142 |
| 8c branch (b) | 146 | 144 |

## 7. Questions

### 7.1 For the operator (the lead takes them)

**O1. The dressup plunge feed.** On the dressup entry (Face, Pocket, Profile and the other 2.5D
ops with an entry style), the feed down through the air above the stock, and the short plunge where
a ramp has no room, run today at half the op's plunge rate. Should they run at the op's plunge rate,
as the 3D Rough entries already do?

- (a) Yes. Five ops of the wanaka file feed those moves twice as fast: idx 0, 11 (2162 instead of
  1081 mm/min), 8 (240 instead of 120), 12 (2162 instead of 1081), 17 (1582 instead of 791).
  96 of the 98 moves are in air (the lead above the stock); 2 (idx 12) can cut the last 0.5 mm.
  Against the limits after the step (table `G`): every raised feed is under the machine Z axis
  rate (2500 mm/min, `$112`). The two idx 12 plunges (2162 mm/min) are inside the vertical-rate
  limit (the 18 mm tool has no G6 axial-chip claim, so the Z axis rate is the bound) and draw
  0.99 kW of the 1.125 kW available at 18000 rpm (the axial-plunge power check). One raised feed
  is above a limit: the idx 17 air lead (1582) is above the axial-chip bound of that tool at its
  RPM (1347.5 mm/min at the stored 10610 rpm). It is in air, so no chip forms; but the stored
  plunge rate of idx 17 is above that bound, and the 42 plunges of idx 17 already run at 1582
  into the stock today (finding F7).
- (b) No. Keep half, as a named rule with no source.

My choice: (a). It makes one plunge rule for both emitters and deletes two unsourced numbers.
Every raised feed is the op's own plunge rate; all are under the Z axis rate, and all but the idx
17 air lead are inside the vertical-rate limit of their tool (that one is in air and above a
stale stored plunge rate, F7).

**O2. The default ramp angle by material** (a new op only; a stated angle stands):

| Branch | Hardwood | Softwood, plywood, MDF | What changes |
|---|---|---|---|
| Easel | 5 deg | 20 deg | new ops ramp steeper than today (3 deg). FM1: 70 ramp feeds fall (to 0.25 to 0.92 of today). Table `S` (6 and 18 mm, two machines): no value moves |
| Shaper | 20 deg | 27 deg | steeper again. FM1: 115 ramp feeds fall (to 0.18 to 0.98). Table `S`: 8 of 24 values fall, the largest on the operator machine, 18 mm softwood Pocket 6447 to 4906 mm/min |
| No change | 3 deg (2.5D) / 10 deg (3D Rough) | the same | no number moves; two unsourced numbers stay |

My choice: Shaper, by the operator's words ("a bit less than soft wood": 20 against 27 is a bit
less; 5 against 20 is a quarter) and because Shaper names the tool (6 mm 2-flute up-cut). The
caution: the Shaper page's own plunge and feed numbers do not give its angles (section 4.2). If
the operator prefers the gentler entry, Easel. The table `H` row "equal axial load" gives a third
view: from the force line, the hardwood angle that holds the softwood axial force at the same
path feed is 11.78 deg (from 20) or 15.72 deg (from 27) for generic hardwood, 10.43 or 13.91 deg
for white oak (section 10.5). Both printed hardwood values sit on either side of it: Easel's 5 is
below the derived value from its own 20, Shaper's 20 is above the derived value from its own 27.

### 7.2 For the lead (each with my choice)

- **Q1. The entry cut reading.** Value check (no proposal moves, 0 FM1 cells, 0 wanaka ops) or
  depth ceiling (97 FM1 cells, Face proposals fall up to 79 %; on the operator file the geodesic
  3D roughs idx 1, 2, 14, 15 fall 2 to 16 % and the Suggest feed of idx 1 falls from 9123 to 2623
  mm/min, section 10.3). My choice: the value check (your default).
- **Q1b. The ramp entry depth of the value check.** The depth per pass (a single straight ramp
  ends as a slot of that depth; the conservative case; three operator-file entries read `Exceeds`)
  or the lap depth `2 L tan theta` (needs the ramp run `L`, which the emitter picks from the path:
  not known before generation). My choice: the depth per pass, with the card text "the ramp's
  last lap at full depth (worst case)".
- **Q2. The helix pitch.** Keep the two door defaults (1 mm, 2 mm) as two `Unsourced` rows of
  `entry_rule.rs`, or move to one helix rule by angle (FreeCAD 5 deg, `freecad_cam_adaptive_helix
  _defaults.txt`; pitch `2 pi r tan 5 deg`, research note 3.3)? The second changes the default of
  new ops (an 18 mm tool: 2.97 mm per turn) and is a third operator default. My choice: keep in
  P29-8; log it for the R6 follow-up.
- **Q3. The helix radius field of the 3D Rough** (`helix_radius_factor`, a stored factor) against
  the dressup `helix_radius` (a stored mm value or the rule): one shape? My choice: not in P29-8
  (no number moves; the stored fields stand); a finding (F3).
- **Q4. The tab re-plunge.** On wanaka idx 17 the 42 tab re-plunges stay straight plunges at the
  op plunge rate under the Ramp style (measured; cause not traced). Trace and fix it in P29-8, or
  declare them and leave the row open? My choice: declare them in 8d (a limit then reads them) and
  give the cause its own step with a prediction run (a fix moves idx 17 G-code).
- **Q5. `cuts()` reads the configured entry kind** (a new argument). Or keep `cuts()` pure and file
  every entry kind `Always`? My choice: the argument; `Always` on every kind would judge a helix
  on an op that ramps.

## 8. Tests that go red

Found by `rg` over `crates/` for the moved names and texts, and by one run with two throwaway
edits (byte copies, restored, `sha256sum -c` OK): the dressup default angle set to 20 deg
(`compute/config.rs`, `DressupConfig::default`; a stand-in for the Easel or Shaper branch), and the
dressup plunge feed set to the feed of the first generator `EntryPlunge` move (a stand-in for the
op plunge rate, branch (a)). The run: the dressup, G10, K4, G-OPGOLDEN, F201 and three P29
sentries (23 binaries, listed below). Not run: core `--lib`, viz `--lib`.

Red in the run:

| Test | Red with | Message | Commit |
|---|---|---|---|
| `a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10::the_entry_defaults_are_the_named_repo_rules_g10` (`:497`) | the angle edit | `left: 20.0, right: 3.0`: it pins the default angle | 8b (Easel, Shaper) |
| `a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10::a_bull_ramp_ships_the_cut_feed_under_the_plunge_limit_g10` (`:174`) | the angle edit | the card text "operator value (repo rule default 20.00°)": it pins the note text | 8b (every branch: the wording) |
| `every_op_keeps_its_toolpath_g_opgolden` | both edits: 146 changes in 27 op fixtures; the plunge edit alone: 54 changes (adaptive3d/by_area_helix_cp, adaptive3d/fresh, adaptive/rest, pencil/rest, pencil/rest_depth_rest, pocket/rest, profile/rest, scallop/iso_field_rest, scallop/rest, steep_shallow/rest_regions, unified_finish/claims_rest_clip, zigzag/rest, and `produced` rows of project_curve and unified_finish) | the fixtures build their dressups from `DressupConfig::default` | 8b (Easel, Shaper), 8c (a) |

The stand-ins are not the final code: the plunge stand-in reads the first `EntryPlunge` feed of the
toolpath, which differs from the op plunge rate on an op whose first plunge is not at that rate,
and 5 of its 54 changes are `cutting_length_mm`, which a feed change alone does not move. So the
G-OPGOLDEN count of 8c is a stand-in count; the implementer diffs the 8c re-bless against
`opgolden_known_46_rows.txt` and names each op. In 8b with `DoorDefaults` no default moves, so
G-OPGOLDEN is predicted green there (reasoned).

Green in the run (with both edits): `capability_link_moves_safety`, `dressup_span_invariants`,
`entry_moves_stock_aware_g_rampterrain`, `finishing_defaults_have_no_ramp_entry_r10`,
`adaptive3d_entry_stock_aware`, `plunge_guard_p3`, `plunge_guard_ab_p3`,
`an_entry_ramp_follows_the_arc_g_rampchord`, `lead_in_out_feed_rates_f040`,
`a_plunge_is_a_named_fraction_of_the_side_feed_g10`, `suggest_leaves_the_entry_style_to_the_operator_g10`,
`a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`, `every_op_states_its_entry_contract_k4`,
`an_air_lead_is_an_approach_not_a_plunge_k4`, `the_exemption_list_only_shrinks_p29_3`,
`every_limit_judges_every_declared_cut_p29_7`, `every_op_declares_its_cuts_p29_3`,
`every_feeds_formula_constant_states_its_source_f201`, `a_turning_descent_holds_the_core_dwell_rule_p22_0c`,
`isoclip_entry_ramp_g_isoclipentry`. (The F201 and P29 sentries go red by design in 8a, 8c and 8d,
when the rows and pins move: section 6.)

Found by `rg` only (not run):

| Test | Why | Commit |
|---|---|---|
| viz `tests/the_card_names_every_entry_rule_g10.rs` | asserts the card text "repo rule (default 3.00°)" | 8b |
| `feeds/ramp.rs` lib tests (`:1408-1563`) | call `entry_geometry` and `entry_notes` with three arguments and assert the default text | 8a (signature), 8b (text) |
| `compute/config.rs:2376` lib test | the `HELIX_RADIUS_OVER_D` path | 8a |
| `session/mod.rs:2867` lib test text `"ramp_angle = 3.0\n"` | the writer omits a `Rule` angle | 8b |
| the 15 files with `DressupContext {` literals (32 sites) | the new required `entry` field (8a), and the required plunge rate in branch (a) (8c) | 8a, 8c |
| FM1 | the cells of section 10.1 | 8b |

## 9. Docs to change

- `crates/rs_cam_core/src/dressup/CLAUDE.md`: the entry invariant (the emitters read
  `ResolvedEntry`; no emitter holds a number), the sentry line.
- `crates/rs_cam_core/src/feeds/CLAUDE.md`: the file map (`entry_rule.rs`), the G6/G10 invariant
  (the angle has a source table; the plunge rule of the dressup door).
- `crates/rs_cam_core/src/limits/CLAUDE.md`: an entry has an engagement; `PlaneDepth`.
- `crates/rs_cam_core/src/compute/CLAUDE.md`: "an entry feed is `entry_feed_rate()`" becomes "an
  entry is `ResolvedEntry`".
- `crates/rs_cam_core/src/adaptive3d/CLAUDE.md`: the fallback reads the op's angle.
- `FEATURE_CATALOG.md` (entry styles), the GUI help text of the angle field ("rule" or a value).
- `NEXT_SESSION.md` hand-over and `P29_FINDINGS.md` section 6, row P29-8.

## 10. Predictions

### 10.1 FM1

Prototype method: section 15. Calibration: the prototype with every switch off gives both FM1
files byte for byte (`sha256` equal to the committed files).

| Mode | Matrix cells | Rows | Per column | Simulation file |
|---|---|---|---|---|
| fallback reads the op angle (8a) | 0 | 0 | | 0 |
| plunge branch (a) (8c) | 0 | 0 | | 0 |
| angle branch DoorDefaults (8b) | 126 | 126 | `entry_notes` 126 | 0 |
| angle branch Easel (8b) | 266 | 126 | `entry_notes` 126, `ramp_feed_mm_min` 70, `ramp_arm` 70 | 0 |
| angle branch Shaper (8b) | 356 | 126 | `entry_notes` 126, `ramp_feed_mm_min` 115, `ramp_arm` 115 | 0 |
| entry as a value check (8d, chosen) | 0 | 0 | | 0 |
| entry as a depth ceiling (8d, rejected) | 97 | 50 | `power_kw` 50, `depth_per_pass_mm` 5, `r_axial_depth_mm` 5, `force_n` 5, `proposal_ap_mm` 5, `stepover_mm` 4, `r_radial_width_mm` 4, `cap_mm` 4, `binding_limit` 4, `proposal_ae_mm` 4, `rpm` 2, `feed_mm_min` 2, `ramp_feed_mm_min` 2, `chip_regime` 1 | 0 |

The 126 rows are the ramp-entry cells: Face 32, Pocket 32, Profile 30, Zigzag 32. The 2D
Adaptive helix cells (30) and the 3D Rough cells (plunge entry, 40) do not move. Every moved ramp
feed falls; none rises (ratio after/before: Easel 0.247 to 0.915, Shaper 0.176 to 0.981). The arm
moves from `PlungeSlope/CutFeed` to `PlungeSlope/PlungeTerm` (Easel 66, Shaper 92) or from
`Sourced/CutFeed` to `Sourced/ChipTerm` (Easel 4, Shaper 23): at the steeper slope the vertical
rate binds instead of the cut feed.

`entry_notes` text after 8b: `Ramp angle 20.00°: the rule for softwood, plywood, MDF (Easel) |
Entry starts 0.50 mm above the material: no source; operator rule 0.5 mm` (the clearance note
does not change). The prototype printed this wording; the implementer uses it exactly or states
the new text in the commit and re-blesses the same cells.

The total FM1 count of P29-8 is the sum of the chosen branches: 8b (126, 266 or 356) plus 8c (0)
plus 8d (0). Each mode ran alone; the combined tree (fallback on, plus an angle branch, plus the
plunge branch) did not run on FM1, so the sum is reasoned (each mode moved no cell of another
mode's columns).

The 97 cells of the depth-ceiling reading are an upper bound: the prototype declared every entry
kind of the contract `Always`; the design would declare only the configured kind `Always`.

### 10.2 Table `G` (the wanaka operator file)

See `P29-8_predictions.csv`, table `G`. In branch (b) and in every angle branch: every op
byte-equal to HEAD (measured: idx 0, 1, 3-8, 10-14, 16, 17). In branch (a): only the feed of the
listed entry moves; every other move byte-equal (a line-by-line compare that masks the feed):

| idx | Moves that change (branch a) | In material? | Feed today | Feed after | Limit after |
|---|---|---|---|---|---|
| 0 | 60 `EntryApproach` | no (air lead) | 1081 | 2162 | Z axis rate (no load: air) |
| 8 | 9 `EntryApproach` | no | 120 | 240 | Z axis rate |
| 11 | 8 `EntryApproach` | no | 1081 | 2162 | Z axis rate |
| 12 | 2 `EntryPlunge` | can touch the last 0.5 mm | 1081 | 2162 | the vertical-rate limit and the plunge power check (table `G`) |
| 17 | 19 `EntryApproach` | no | 791 | 1582 | Z axis rate |

No angle moves; no ramp steepens; no other feed rises. idx 1, 14, 16 (heavy) byte-equal in both
branches; idx 9 byte-equal in branch (a).

### 10.3 Wanaka, both files (table `W`), and rivmap100 (table `R`)

Read-only loads of the four files through the session doors (`safe_envelope`,
`suggest_for_operation` with the op's dressups):

- The chosen reading (value check) moves no proposal and no Suggest value on any op of the four
  files, in every O1 and O2 branch: every op states its angle, and the entry is no ceiling.
- The rejected reading (depth ceiling) moves the geodesic 3D roughs, which declare their slot as
  `MayOccur` today, so the entry becomes their first `Always` slot:

| File | idx | Proposal today | Depth-ceiling reading | Suggest feed today | Depth-ceiling reading |
|---|---|---|---|---|---|
| operator | 1 | 2.0552 x 5.5 | 2.0095 x 5.5 | 9123 | 2623 (the power case becomes the slot) |
| operator | 2 | 3.4391 x 1.5 | 2.9458 x 1.5 | 6841 | 6841 |
| operator | 14 | 2.4021 x 3.6 | 2.2924 x 3.6 | 10397 | 10397 |
| operator | 15 | 3.5172 x 1.382 | 2.9443 x 1.382 | 7022 | 7022 |
| work | 1 | 3.7498 x 1.2 | 3.0005 x 1.2 | 7371 | 7371 |
| work | 2 | 3.5118 x 1.5 | 3.0005 x 1.5 | 6841 | 6841 |
| work | 15 | 3.5988 x 1.382 | 3.0005 x 1.382 | 7022 | 7022 |

  No rivmap100 op moves in that reading (its 3D roughs are ContourParallel and declare their
  slot already).
- G-code on rivmap100 (both files): the Face (idx 0) and the Project Curve (idx 3) are byte-equal
  in branch (a). The Face's first linear move is a cut at 2200, so `0.5 x 2200 = 1100` is above
  its 500 plunge, and the dressup feed is already the op plunge rate (500). The rest ops idx 1 and
  2 were not generated (they need the face simulated).

The entry checks that 8d adds (value check; the force budget 35.32 N; `CutLoad` at the declared
entry cut, measured by the model functions; the driver plumbing is reasoned):

| File | idx | Entry cut | Peak force (N) | State | Power (kW of available) |
|---|---|---|---|---|---|
| operator | 0, 12 | ramp, slot 18 mm at 0.5 mm, 6447 mm/min | 7.54 | Within | 0.096 of 1.125 |
| operator | 1 | helix, slot 18 mm at 2.0 mm (one pitch), 6448 | 30.12 | Within | 0.382 of 1.125 |
| operator | 2 | ramp, slot 6 mm at 6.0 mm, 4571 | 66.01 | Exceeds (1.87 x) | 0.316 |
| operator | 8 | ramp, slot 2 mm at 0.5 mm, 1258 | 4.32 | Within | 0.003 |
| operator | 14 | ramp, slot 18 mm at 4.166 mm, 6448 | 62.49 | Exceeds (1.77 x) | 0.796 |
| operator | 15 | ramp, slot 6 mm at 5.532 mm, 4571 | 61.67 | Exceeds (1.75 x) | 0.292 |
| operator | 17 | ramp, slot 6 mm at 1.25 mm, 3165 at 10610 rpm | 16.59 | Within | 0.042 of 0.663 |
| work | 1 | helix, slot 6 mm at 2.0 mm, 6095 | 23.81 | Within | 0.141 of 1.5 |
| work | 2, 15 | ramp, slot 6 mm at 6.0 / 5.532 mm, 6095 | 66.01 / 61.67 | Exceeds | |
| work | 14 | ramp, slot 6 mm at 3.0 mm, 6095 | 35.28 | Within (99.9 %) | |
| rivmap live | 0 | ramp, slot 6 mm at 0.812 mm, 2200 | 6.63 | Within | |
| rivmap live | 1 | helix, slot 6 mm at 2.0 mm, 2400 | 16.84 | Within | |

Not measured: the fallback ramp of idx 1. idx 1 enters mostly by the fallback ramp (9944 ramp
moves against 561 helix moves, section 3.2). In 8d it is the `MayOccur` `Entry(Ramp)` cut: by the
same model, an 18 mm slot at the 10 mm depth per pass at 6448 mm/min. Its force is above the 62.49 N
of the 4.166 mm idx 14 cut (the force rises with the depth), so it reads `Exceeds` as a warning row
(a `MayOccur` cut gives a check, never a ceiling).

So three 3D-rough ramp entries of the operator file get a new `Exceeds` row on the card (four with
the idx 1 fallback warning): the
full-depth ramp model (a slot at the depth per pass) is the worst case; the real lap ladder engages
less (at most `2 L tan theta` per lap, section 4.4). The gates already judge these entry samples
after the simulation (P29-6). No feed moves for them (the modulator is held).

### 10.4 Suggest ramp feed of a new op (table `S`)

A new Pocket (dressup ramp) and a new 3D Rough (ramp entry), 6 mm and 18 mm flat end mills (the
wanaka tools), generic softwood, generic hardwood and white oak, on the default machine and on the
operator machine. The angle is set as a stated value equal to each branch's rule (a stated angle
and the rule give the same number). Measured; mm/min (arm):

| Machine | Material | Tool | Op | Today (3 / 10 deg) | Easel (5 hard / 20 soft) | Shaper (20 hard / 27 soft) |
|---|---|---|---|---|---|---|
| default | softwood | 6 | Pocket | 3999 | 3999 | 3925 (ChipTerm) |
| default | softwood | 6 | 3D Rough | 3463 | 3463 | 3399 (ChipTerm) |
| default | softwood | 18 | Pocket, 3D Rough | 3999, 3463 | the same | the same |
| default | hardwood, white oak | 6 | Pocket, 3D Rough | 3999, 3463 | the same | the same |
| default | hardwood | 18 | Pocket, 3D Rough | 3999, 3095 | the same | the same |
| operator | softwood | 18 | Pocket | 6447 | 6447 | 4906 (PlungeTerm) |
| operator | softwood | 18 | 3D Rough | 6447 | 6447 | 4906 (PlungeTerm) |
| operator | softwood | 6 | Pocket, 3D Rough | 4572, 4571 | the same | 4486, 4486 (ChipTerm) |
| operator | hardwood | 18 | Pocket | 6448 | 6448 | 5788 (PlungeTerm) |
| operator | white oak | 18 | Pocket | 6448 | 6448 | 5940 (PlungeTerm) |
| operator | hardwood, white oak | 6 | Pocket, 3D Rough | 4572, 4571 | the same | the same |
| operator | hardwood, white oak | 18 | 3D Rough | 2867, 2144 | the same | the same |

No value rises. Easel moves no value of this table (5 deg is close to 3 deg; at 20 deg the 6 and
18 mm softwood cells still bind on the cut feed). Shaper moves 6 of the 12 operator-machine cells
and 2 of the 12 default-machine cells (3 materials x 2 tools x 2 ops per machine).

### 10.5 Hand checks (table `H`)

- The equal-axial-load angle. The Curti line scales `Ks` and `F_edge` with the density
  (`material/force_line.rs:533-549`), so at one path feed the axial force of a ramp scales with
  `rho x sin(theta)`. Equal axial force: `sin(theta_h) = sin(theta_s) x rho_s / rho_h`. The
  densities come from the force line (probe, table `H`).
- The Shaper angle check: `atan(250/250)` = 45.0 deg against the printed ~20 deg.
- The raised plunge of idx 12 against its vertical-rate bound (table `G`).

## 11. What changes for the operator's cut

- With O1 (a): on the Face, Pocket and Profile ops of the wanaka file, the feed down through the
  air above each entry doubles (to the op's plunge rate). Two short plunges on the front face can
  touch the last 0.5 mm of stock at the plunge rate (2162 mm/min), inside the Z axis rate and the
  plunge power check for this tool. The idx 17 air lead goes to 1582 mm/min, above the axial-chip
  bound of that tool at its stored RPM (1347.5), in air. No ramp, no helix and no cutting move
  changes.
- With O1 (b): nothing changes.
- O2 changes only new ops: a new ramp entry in softwood or hardwood ramps steeper (20 or 27 deg
  with Shaper; 5 or 20 deg with Easel), and its Suggest ramp feed falls where the slope binds. The
  wanaka ops keep their 3 deg ramps.
- The card of each op shows the entry as a cut with its own checks (force, power, chip) in the
  entry group. No proposal moves.

## 12. Findings outside the scope

- F1. The rest ramp ladder (pencil crease and G-ISOCLIPENTRY) uses its own 12 deg cap
  (`finish/pencil/emission.rs:70`). It is a third ramp-angle number. It should read the table, or
  become a named cap of its own with the operator's ruling.
- F2. The helical waterline reads the dressup ramp angle as its ring descent limit
  (`ops/waterline.rs:843`). A ring descent is a cut, not an entry; it needs its own source.
- F3. The two doors store the helix radius differently (`helix_radius` in mm or the rule;
  `helix_radius_factor` as a factor of D).
- F4. `SuggestContext::upstream_leftover_stock_mm` (`feeds/suggest.rs:165`) is documented "v1
  doesn't use this". P29-9 can use it (P29-9 Q1).
- F5. The tab re-plunge under style None runs at the cut feed (Q4).
- F6. The air lead of the dressup door is a fed move through air at a plunge feed; a rapid would be
  faster. Not in scope.
- F7. Wanaka idx 17 (Profile, 6 mm) stores `plunge_rate = 1582`; at its RPM (10610) the
  vertical-rate limit gives 1347.5 mm/min (the G6 axial chip 0.0635 mm x 10610 x 2 flutes,
  binding `AxialChip`). The stored value is above the bound; its 42 generator plunges run at 1582
  today. A Suggest apply with the project spindle policy rewrites the pair (24000 rpm, plunge
  2500, table `W`). Measured (probe `plunge_limits`, table `W`).
- F8. With the Shaper or Easel table, the Suggest ramp feed of a new 18 mm Pocket on the operator
  machine in hardwood falls from 6448 to 5788 (20 deg) and in softwood from 6447 to 4906 (27 deg);
  on the default machine no 18 mm value moves (table `S`). The ramp feed rule divides by `tan
  theta`; a steeper default makes the vertical rate bind.

## 13. Lead rulings

Lead, 2026-11-12.

- **Order.** 8a goes now: it is byte-equal and reads neither operator answer. 8b and 8c wait for
  O1 and O2, the `DoorDefaults` wording included, so the card text is set one time. 8d follows 8a.
- **Q1: the value check.** An entry is no ceiling of the body plane.
- **Q1b: the depth per pass.** Accepted as the worst case, and the card says "worst case". The
  three or four new `Exceeds` rows on the operator file (1.75 to 1.87 times) are warnings against
  a budget that the operator parked as probably low. They block nothing. P29-10 decides how the
  card shows them.
- **Q2: keep the two pitch rows.** The helix rule by angle is logged for the R6 follow-up.
- **Q3: not in P29-8.** Finding F3 stays.
- **Q4: declare the tab re-plunges in 8d.** The cause gets its own step after 8d, with a
  prediction run (it moves the G-code of idx 17). Finding F7 goes to the operator now: it is a
  fact of today's cut and does not depend on P29-8.
- **Q5: the argument.** `cuts()` takes the configured entry kind.
- **The 0 FM1 cells of 8d are reasoned.** The implementer checks the columns `diagnostic_ids`,
  `diagnostic_severities`, `feeds_warnings` and `suggest_warnings`: a new `Exceeds` check must
  not write a new diagnostic into a matrix cell. A moved cell stops the work.
- **An absent angle key (8b).** Section 4.2 says a number loads as `Stated` in "every existing
  file". That is measured on the wanaka files only. Tracked project files with no dressup table
  exist (for example `tests/fixtures/p_thin_005_2026-11-07_3fc31cc8/project.toml`,
  `tests/fixtures/rest_cascade_g_restres/project.toml`, `research_p11_step0/after/projects/*`).
  Under Easel or Shaper their absent key loads as `Rule`, so an existing op ramps steeper. Before
  8b the implementer lists each tracked file and each file of `~/.config/rs_cam` with an entry
  and no angle key, and names what moves. An operator project in that list stops 8b until the
  operator rules.
- **O1.** My first lean to (a) had a condition: each raised move inside its plunge limit. The idx
  17 air lead (1582 against 1347.5) does not meet it, because the stored plunge rate is stale
  (F7). The lead tells the operator this with the question.

## 14. Operator rulings

Operator, 2026-11-13.

- **O1: (a), the op plunge rate.** "yeah, lets just go for the plunge rate!" The dressup door
  feeds the air lead, the stock-aware plunge and the fallback plunge at the op plunge rate. The
  operator saw the table of the doubled moves (idx 0, 8, 11, 12, 17) and the idx 17 air lead above
  the stale 1347.5 bound.
- **O2: Easel.** "lets do 5 and 20". The default ramp angle of a new op is 5 degrees in hardwood
  and 20 degrees in softwood, plywood and MDF. A stated angle stands.

## 15. Prototype and runs

The prototype was a throwaway edit of nine files (`compute/execute/dressup_apply.rs`,
`dressup/mod.rs`, `adaptive3d/path.rs`, `compute/execute/rough_3d.rs`, `compute/config.rs`,
`feeds/ramp.rs`, `feeds/suggest/apply.rs`, `session/compute.rs`, `compute/catalog/cuts.rs`),
restored from byte copies, plus a throwaway test file, deleted. Switches by environment variable
in one build:

| Variable | Off | On |
|---|---|---|
| `P29P_FALLBACK=1` | the fallback ramps read 3 deg | they read the op's resolved angle |
| `P29P_PLUNGE=a` | the dressup plunge is today's fraction | the op plunge rate |
| `P29P_ANGLE=none\|easel\|shaper` | today | the default angle of a new op is "rule" (a marker), resolved by the material class at both doors and in Suggest, with the 8b card text |
| `P29P_ENTRY=ceiling` | today | the ramp and helix entries are slot cuts at the body depth at the unlifted chip, `Always` |

Not prototyped: the value-check reading (it gives no ceiling, so its numbers equal "off"; the
check rows of table `W` were computed by the model functions at the declared cut), the
`EntryAngle` serde shape (a stored number is untouched in the prototype, as in the design), the
helix rule (Q2).

Calibration: every switch off gives both FM1 files byte for byte and every wanaka op byte for byte
(against the HEAD dumps taken before the edit).

What is measured: every FM1 cell (both files, every mode), every wanaka move (13 light ops in
three modes; idx 1, 14, 16 in three modes; idx 9 in two), the plunge limits and the entry checks
(probe on the session doors), table `S`. What is reasoned: idx 2 and 15 (section 3.4), the final
shape (`ResolvedEntry`, `EntryAngle`, `PlaneDepth`) gives the prototype's numbers, the reds of
section 8 that no run reached.
