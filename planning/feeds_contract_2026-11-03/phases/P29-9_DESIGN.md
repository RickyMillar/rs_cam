# P29-9 design: a depth where the op states none

Written 2026-11-12 on HEAD `9fa97dbed`, after the P29-8 design note. No product code changed: the
prototype edit was restored from byte copies and the probe test was deleted (section 13). The lead
reviews this note, commits it with the two prediction files, and then starts an implementer. The
result of each commit is judged against section 9. A mismatch stops the work.

Files of this step:

- this note;
- `P29-9_predictions_fm1.csv`: every FM1 cell that moves, per commit and per branch of Q2
  (measured);
- `P29-9_predictions.csv`: tables `W` (both wanaka files), `R` (the rivmap100 files), `S` (Suggest
  on the default machine), `H` (hand checks).

Order assumed: P29-8, then this step. Section 1.4 says why the two steps are independent in their
numbers.

The acceptance of the change list: "no op has a silent missing value". The operator's aim (2026-11-12)
holds here too: a limit is a function of the cut; when the cut has no depth, the record says why
(`NoNumber` with a reason), and a depth that geometry or an earlier op gives is not left out.

## 1. Scope

### 1.1 The sources

| Source | What it puts in P29-9 |
|---|---|
| Change list (`P29_FINDINGS.md` section 6) | "A depth or width where the op states none (finish: the stock the earlier op leaves; Project Curve: the offset as a bound). Else `NoNumber` on the record." Sentry: "no op has a silent missing value". |
| Rows whose `closes_in` names P29-9 | `P28-11` (every pre-sim limit that reads a depth: the Project Curve depth is known after the simulation), `P28-32` (a chamfer width and a peck give no depth), `F-OffsetRing` (the offset-ring fact is set from the Pocket family and the Roughing role) |
| Code comments that name P29-9 | `limits/cut.rs:294-296` (`NoEngagementModel::NoStatedDepth`, `NoStatedWidth`), `compute/catalog/cuts.rs:278` (the scallop width is not stated on the cut) |
| Existing parts | `SuggestContext::upstream_leftover_stock_mm` (`feeds/suggest.rs:165`, "v1 doesn't use this; v2 finish-DPP selection will"); `ExecutionContext::rest_axial_step_mm` (`compute/execute.rs:520`: "the smallest axial step of the earlier enabled ops of the setup", a precedent of "the earlier op"); `ops/chamfer.rs:114-119` (`chamfer_depth`, the generator's depth: `(chamfer_width + tip_offset) / tan(half angle)`) |

The sources disagree in one place: the change list says "Else `NoNumber` on the record". Today a
finish op has a default depth from the family table (row `P28-31`, "the default cut when the
operator states none", closes in P27). This note keeps `P28-31` for the speeds and puts `NoNumber`
on the record of each limit that would read an earlier op's leave (Q3).

### 1.2 A verdict for each row

| Row | Verdict | Why |
|---|---|---|
| `P28-11` | close, 9b | the Project Curve offset below the surface is a lower bound of the depth into the stock (the stock above the surface adds to it); the limits judge the cut at the bound; a check at a lower bound can prove `Exceeds` and never `Within` (section 2.2) |
| `P28-32` | close, 9a | the chamfer depth is the generator's own formula on the V-bit; the peck half is a drill cycle, which the physical rows `P28-04`, `P28-17`, `P28-20`, `P28-24` already exempt (no radial engagement) |
| `F-OffsetRing` | re-file: `closes_in` "later (a raster width rule)" (Q2 (b), my choice); close only with Q2 (a) | a contour Pocket lays offset rings; a raster (Pocket zigzag, Zigzag, Face) does not. But the corner rule is the gate of the removal-rate pick of these ops: the geometry fact drops the raster ops out of the pick, and their proposals fall to the family default depth (412 FM1 cells, section 9.1) |

### 1.3 What has no depth today

`OperationConfig::declared_depth` (`compute/catalog/cuts.rs:672-682`):

| Axial rule | Ops | Declared depth today | After P29-9 |
|---|---|---|---|
| `Surface { max: None }` on a finish op | DropCutter, Pencil, Scallop, SpiralFinish, RadialFinish, HorizontalFinish, and RampFinish with no cap | `None(NoStatedDepth)` | `EarlierLeave`: the axial stock that the earlier op leaves (Q1), else `None(NoEarlierLeave)` |
| `Surface { max: None }` on VCarve | VCarve with no max depth | `None(NoStatedDepth)` | unchanged: the V depth follows the outline (`TotalDepth::None`), and no earlier op's leave is its depth. The rule keys on the op's pass role (finish), not on the axial rule |
| `ChamferWidth` | Chamfer | `None(NoStatedDepth)` | `Stated(chamfer_depth)` on a V-bit |
| `SurfaceOffset(d)` | Project Curve | `None(DepthAfterSimulation)` | `AtLeast(-d)` for an offset below the surface (`d < 0`), else `None(DepthAfterSimulation)` |
| `Peck` | Drill, Pin Drill | `None(NoStatedDepth)` | unchanged: a drill cycle, `None(DrillCycle)` (a new reason, named) |

### 1.4 Independence from P29-8

P29-8 changes entry numbers (8b: the angle default; 8c: the dressup plunge feed) and adds entry
checks (8d). P29-9 changes the depth of the body cut of finish, Chamfer and Project Curve ops and
the offset-ring fact. No P29-9 reader reads an entry angle or an entry feed, and no P29-8 reader
reads a body depth of these ops (the 8d entry depth is the depth per pass or the pitch, which these
ops do not state; a Chamfer and a Project Curve take no dressup entry: `EntryContract::
StraightPlunge`). So with the chosen answers (Q2 (b)) the FM1 cells of the two steps are disjoint,
and this note predicts on HEAD plus the P29-9 prototype (measured, section 9.1: 0 shared cells).
Under Q2 (a) 12 or 15 `ramp_feed_mm_min` cells are shared with the Easel or Shaper branch of
P29-8 8b; section 9.1 says how they are handled.

## 2. The design

### 2.1 One depth door

Today the calculator reads its depth from `CutSpec::calc_inputs(diameter)` (`cut_spec.rs:357`),
the envelope from `axes_plan` (`feeds/envelope.rs:3010`), the planned driver from
`DeclaredCut::depth`, and the force-budget cap from `cap_cut_of_operation`. Each has its own arm
for `ChamferWidth` and `SurfaceOffset` (the `EXEMPT(P28-11)` and `EXEMPT(P28-32)` markers). A
chamfer's width is even read as a depth in `axes_plan` (`Axial::HelixPitch(s) | Axial::ChamferWidth
(s) => (true, Some(s.mm()), ..)`, `envelope.rs:3043`): the envelope axis holds the chamfer width
as the stated depth today.

The design gives the body depth one door: `DeclaredCuts::body_depth()` (`feeds/planned.rs`), which
resolves the declared depth with the tool and the context:

```rust
// compute/catalog/cuts.rs
pub enum DeclaredDepth {
    Stated(f64),
    /// P29-8: the cut's own depth (an entry).
    Own(f64),
    /// P29-9: a lower bound of the depth into the stock (Project Curve: the
    /// offset below the surface; the stock above the surface adds to it).
    AtLeast(f64),
    /// P29-9: the axial stock that the earlier op leaves (a finish pass).
    EarlierLeave,
    /// P29-9: a depth of the tool (Chamfer: `ops::chamfer::chamfer_depth`).
    OfTool(ToolDepth),
    None(NoEngagementModel),
}

// feeds/input.rs: FeedsInput gains one fact, filled by the session door.
pub enum EarlierLeave {
    Mm(f64),                 // the earlier op states an axial leave > 0
    None(NoEarlierLeave),    // no earlier op, or it leaves 0
}
```

`FeedsInput::of_operation` takes `EarlierLeave` (the session fills it from Q1; FM1 and the add
door pass `None(NoEarlierOp)`). `calc_inputs` no longer decides a depth: the calculator reads
`DeclaredCuts::body_depth()`. Every `match` on `DeclaredDepth` names every variant, so a new depth
source does not compile until each reader answers it.

### 2.2 A lower bound on the record

`CheckState` gains no variant: a check at a lower bound is `Exceeds` when the limit fails at the
bound (the real cut is at least as deep), else `NoNumber(NoNumberReason::OnlyALowerBound)`. The
envelope keeps "no depth proposal" for a Project Curve (P16, review D5: it proposes no depth it
cannot model), and the power figure keeps `DepthAfterSimulation`. Only the checks of the planned
driver change.

### 2.3 The finish depth

A finish op (axial rule `Surface { max: None }`) declares `EarlierLeave`. With `EarlierLeave::Mm(v)`
the planned limits judge the finish cut at depth `v`; the calculator keeps its depth from the
default table (`P28-31`) for the speeds (Q3). With `EarlierLeave::None`, the planned limits give
`NoNumber(NoEarlierLeave)`.

### 2.4 The offset ring (Q2)

`CutFacts::offset_ring` (`cuts.rs:692`) is true for the whole Pocket family in the Roughing role
(Pocket of both patterns, Zigzag, Face). As a geometry fact it would be true for a contour Pocket
only. The measured result of that change (prototype, Q2 (a)): the raster ops lose the corner
ceiling AND the removal-rate pick, because `envelope.rs:1517` runs the pick only when
`plan.computed_width` (the offset-ring fact) holds. Their proposal falls to the family default
depth: `0.7 D` (3.175 mm tools: 2.2225 mm; 6 mm tools: 4.2 mm), from up to 6.985 mm. So the fact
cannot move alone: a raster needs its own width rule and its own entry to the pick. That is a
design of its own (Q2 (b)): this step re-files the row.

## 3. Commits

| Commit | What | Moves a number? |
|---|---|---|
| 9a | one depth door (`DeclaredCuts::body_depth`), `DeclaredDepth::OfTool` for the Chamfer, `None(DrillCycle)` for a peck; close `P28-32` | FM1 Chamfer cells (section 9.1) |
| 9b | `DeclaredDepth::AtLeast` for the Project Curve, `OnlyALowerBound`; close `P28-11` | no FM1 cell (checks only); new `cut_checks` rows on wanaka idx 5, 9 and rivmap idx 3 |
| 9c | `EarlierLeave` and the session fill (Q1), `DeclaredDepth::EarlierLeave` | no FM1 cell (no earlier op in a matrix cell); new checks on wanaka idx 16 and rivmap idx 2 |
| 9d | Q2 (b), my choice: re-file `F-OffsetRing` (text only). With Q2 (a): the offset ring as geometry, close the row | Q2 (b): none. Q2 (a): 412 FM1 cells in 64 Face and Zigzag rows (section 9.1) |

## 4. Exemption rows and pins

From the P29-8 end state (ROWS 39, MAX_NEEDS_RULING 28, ROWS_WITH_A_FILE 39, with P29-8 Q4 "yes"):

| Commit | Rows | ROWS | MAX_NEEDS_RULING | ROWS_WITH_A_FILE |
|---|---|---|---|---|
| 9a | close `P28-32` | 38 | 27 | 38 |
| 9b | close `P28-11` | 37 | 26 | 37 |
| 9c | none | 37 | 26 | 37 |
| 9d | Q2 (b): re-file `F-OffsetRing` | 37 | 26 | 37 |
| 9d with Q2 (a) | close `F-OffsetRing` | 36 | 25 | 36 |

## 5. Tests that go red

Found by `rg` over `crates/` only. No sentry, G-OPGOLDEN, core `--lib` or viz `--lib` run was made
with the P29-9 prototype; the FM1 runs are the only measured reds (section 9.1). G-OPGOLDEN is
predicted green (no generator reads a declared depth; reasoned).

| Test | Why | Commit |
|---|---|---|
| `tests/a_slot_reads_the_force_budget_under_every_op_p29_5.rs:302-312` | asserts the Chamfer `Rigidity` entry is `Unmodelled` with `CHAMFER_STATES_NO_DEPTH` | 9a |
| `tests/no_depth_is_proposed_that_cannot_be_modelled_p16.rs` | the Project Curve keeps no proposal (no change expected); it names `DepthAfterSimulation`, which stays for an offset above the surface | 9b (check) |
| `tests/every_op_declares_its_cuts_p29_3.rs:228`, `tests/every_limit_judges_every_declared_cut_p29_7.rs`, `tests/every_op_states_its_cut_k1.rs` | the declared depth of Chamfer, Project Curve, the finishes and the peck change variant | 9a, 9b, 9c |
| `tests/the_exemption_list_only_shrinks_p29_3.rs` | the pins (section 4) | 9a, 9b, 9d |
| `tests/a_published_power_is_at_the_depth_that_cuts_g_s2.rs`, `tests/a_refused_deflection_is_not_a_zero_g_t4.rs` | name `DepthAfterSimulation` or the chamfer depth | 9a, 9b (check) |
| `feeds/envelope.rs`, `feeds/force_budget.rs`, `feeds/operating_point.rs`, `compute/catalog/cut_spec.rs` lib tests | the marker arms `EXEMPT(P28-11)`, `EXEMPT(P28-32)` go | 9a, 9b |
| FM1 | 76 cells (9a); 412 more with Q2 (a) | 9a, (9d) |
| viz `src/ui/feeds/compare.rs`, `src/ui/properties/operations/engrave.rs` | name the chamfer depth or `ChamferWidth` (display) | 9a (check) |

## 6. Docs to change

- `crates/rs_cam_core/src/limits/CLAUDE.md`: the depth sources of a cut, `OnlyALowerBound`.
- `crates/rs_cam_core/src/feeds/CLAUDE.md`: P16 D7 line (the Project Curve keeps no proposal but
  gets lower-bound checks), the chamfer depth, `EarlierLeave`.
- `crates/rs_cam_core/src/compute/CLAUDE.md`: `cuts.rs` file map entry.
- `NEXT_SESSION.md` hand-over and `P29_FINDINGS.md` section 6, row P29-9.

## 7. Questions for the lead (each with my choice)

- **Q1. Which op is "the earlier op"** whose axial leave a finish pass cuts?
  - (a) The nearest earlier enabled op of the same setup that cuts the same model and states an
    axial stock to leave. On wanaka idx 16 (scallop, model 4): idx 15 (3D rough, model 4, leaves
    0.5 mm).
  - (b) The op whose stock the finish reads by its stock source (`from_remaining_stock`: the
    simulation chain before it). Wanaka idx 16 is `fresh`, so (b) gives no earlier op there,
    although the simulation cuts it after idx 15.
  - (c) The largest axial leave over every earlier enabled op of the setup (the worst case).
  - (d) The op that the rest analysis names as the source.
  My choice: (a). It names one op, it reads a field the operator stated, and it follows the order
  of the simulation (S0: an op cuts what the earlier ops of the setup left). `rest_axial_step_mm`
  (the helical Waterline) reads the earlier ops of the setup the same way.
- **Q2. The offset ring.** (a) The geometry fact: only a contour Pocket holds the corner rule; the
  raster ops (Face, Zigzag, a zigzag Pocket) leave the removal-rate pick, and their proposals fall
  to `0.7 D` (412 FM1 cells in 64 rows; 0 wanaka and rivmap ops, whose rasters state their depth).
  (b) Keep the label and re-file the row "later (a raster width rule)". My choice: (b). The label
  is wrong for a raster, but under (a) a proposal falls for a reason that is not a limit (a default
  table row, `P28-31`), which is the opposite of the P29 aim.
- **Q3. The calculator depth of a finish pass** with an earlier leave: keep the default table
  (`P28-31`) for the speeds, or read the earlier leave? My choice: keep (the speeds of a finish do
  not change in P29-9; the leave reaches the checks only). Section 9.2 gives what Suggest would ship
  if it read the leave.
- **Q4. A peck** gets `None(DrillCycle)`, a new named reason, instead of `NoStatedDepth`. My
  choice: yes.
- **Q5. The chamfer feed raise.** With the true chamfer depth, Suggest for a new 12.7 mm V-bit
  Chamfer in MDF and in plywood ships 24000 rpm and 3994 mm/min (today 8000 rpm and 1331 mm/min;
  plunge 443 to 1331). The moved FM1 columns show no change of the vendor row, the chip or the band
  (only `rpm`, `feed_mm_min`, `plunge_mm_min`, the depth, the force and the cap move), so the cause
  is not traced. It raises a feed of a new op (the wanaka file has no Chamfer). Ask: trace it before
  9a lands (my choice), or accept the raise as a consequence of the true depth.

## 8. What changes for the operator's cut

Nothing that moves the machine: no G-code changes in P29-9 (no generator reads a declared depth),
and no Suggest value of the wanaka or rivmap ops moves (measured, every op of four files).

On the card:

- wanaka idx 5 (Project Curve, 6 mm, offset 4.0 below the surface): a new force-budget row,
  `Exceeds` at the lower bound (51.2 N against 35.3 N at 4.0 mm, the real cut is deeper: P29-0
  measured up to 14 mm). Before the simulation, today, the card says nothing.
- wanaka idx 9 (Project Curve, V-bit 3.175, offset 2.42): `NoNumber (only a lower bound)` (20.0 N
  at 2.42 mm is inside, but the real cut may be deeper).
- wanaka idx 16 (scallop finish): the finish cut is judged at the 0.5 mm that idx 15 leaves: 4.1 N,
  3.2 um of deflection against 200 um, 0.003 kW.
- A new Chamfer on a V-bit: the envelope proposes the chamfer depth (1.905 mm for 1.0 mm + 0.1 mm
  tip offset on a 60 deg V-bit), not the chamfer width (1.0 mm) as today; on a 12.7 mm V-bit in MDF
  or plywood Suggest then ships 24000 rpm and 3994 mm/min (today 8000 rpm, 1331 mm/min); the
  cause is not traced (Q5). That raise is on a new op only; the wanaka file has no Chamfer.

## 9. Predictions

### 9.1 FM1

Calibration: no FM1 run was made with every switch off on the build that held the P29-9 edits (the
off run was on the P29-8 build, before these edits). What was measured instead: in each P29-9 mode
the cells outside its own op rows are equal to the committed files (the chamfer mode moved only
Chamfer rows, the ring mode only Face and Zigzag rows, and the simulation file stayed byte-equal in
both), and every P29-9 switch is an `if` on its environment variable.

| Mode | Matrix cells | Rows | Per column | Simulation file |
|---|---|---|---|---|
| 9a chamfer depth | 76 | 28 | `cap_factor` 27, `cap_mm` 27, `r_axial_depth_mm` 6, `force_n` 5, `proposal_ap_mm` 5, `rpm` 2, `feed_mm_min` 2, `plunge_mm_min` 2 | 0 |
| 9b Project Curve lower bound | 0 | 0 | no FM1 column reads a cut check; the proposal and the power figure keep `DepthAfterSimulation` | 0 |
| 9c finish at the earlier leave | 0 | 0 | a matrix cell has no earlier op, so every finish gives `NoNumber(NoEarlierOp)` | 0 |
| 9d with Q2 (a) | 412 | 64 | `suggest_warnings` 64, `binding_limit` 49, `depth_per_pass_mm` 43, `r_axial_depth_mm` 43, `force_n` 43, `power_kw` 43, `proposal_ap_mm` 43, `cap_mm` 15, `feed_mm_min` 15, `ramp_feed_mm_min` 15, `rpm` 13, `proposal_ae_mm` 9, `stepover_mm` 4, `r_radial_width_mm` 4, `plunge_mm_min` 4, `r_chip_load_mm` 4, `chip_regime` 1 | 0 |

9a: the `cap_*` cells are the force-budget cap of `cap_cut_of_operation`, now solved for every
Chamfer cell (also the cells whose Suggest refuses: an end mill on a Chamfer). The V-bit cells:
`r_axial_depth_mm` 0.381 / 0.762 to 1.9053, `proposal_ap_mm` 1.0 to 1.9053, `force_n` up (6.35 mm
hardwood 2.18 to 4.45 N). 9b and 9c are 0 by construction (they write check rows only); the FM1
CSV holds no row for them.

Overlap with P29-8 (the same cell moved by both steps, counted by key): 9a and any P29-8 branch:
0. 9d under Q2 (a) and 8b: 0 (DoorDefaults), 12 (Easel), 15 (Shaper) cells of `ramp_feed_mm_min`
in Face and Zigzag rows; those cells are then not independent, and the implementer re-measures
them on the P29-8 tree. With Q2 (b), my choice: no overlap, so the two steps are independent.

### 9.2 Wanaka and rivmap100 (tables `W`, `R`)

Read-only loads, session doors, the four files:

- No proposal and no Suggest value moves in the chosen design (Q2 (b), Q3 keep).
- Q2 (a) moves no op of the four files (their rasters state the depth).
- Q3 alternative (the calculator reads the earlier leave): the scallop proposal depth moves 0.2
  to 0.5 mm on all four files; rpm, feed and plunge do not move.
- New check rows (table `W`, `R`): wanaka idx 5 `Exceeds` at the bound (51.17 N); idx 9 `NoNumber
  (OnlyALowerBound)` (20.01 N); idx 16 finish at 0.5 mm: 4.13 N (operator file) and 4.46 N (work
  file), deflection 3.2 / 3.4 um; rivmap idx 2 finish at 0.5 mm: 2.65 N; rivmap idx 3 (offset
  +1.0, above the surface): `NoNumber(DepthAfterSimulation)`.

### 9.3 What is measured and what is reasoned

- Measured: every FM1 cell of 9a and 9d (Q2 (a)); every proposal and Suggest value of the four
  files in three modes; the check values (the model functions at the declared cut).
- Reasoned: 9b and 9c add no FM1 cell (no FM1 column reads a check); the final shape
  (`DeclaredDepth::{AtLeast, EarlierLeave, OfTool}`, `EarlierLeave`) gives the prototype's numbers;
  the session fill of Q1 (a) picks idx 15 for wanaka idx 16 (read from the file, not run).

## 10. Findings outside the scope

- G1. Today the envelope reads the chamfer WIDTH as the stated depth (`envelope.rs:3043`): every
  Chamfer proposal is the width (1.0 mm in FM1). 9a fixes it; the defect is older than P29.
- G2. The removal-rate pick of the raster ops depends on the offset-ring label (`envelope.rs:1517`,
  `plan.computed_width`). A raster has no width rule of its own (Q2).
- G3. `SuggestContext::upstream_leftover_stock_mm` has no reader; 9c gives it one (the session
  fill).
- G4. The finish depth of the calculator is the family default table (`P28-31`, 0.2 mm on the 2 mm
  tapered ball); the earlier op's leave is 0.5 mm on wanaka idx 16. Q3 keeps the default for the
  speeds; a later step (P27) can retire the table.

## 11. Lead rulings

Lead, 2026-11-12.

- **Order.** P29-9 starts after P29-8 8d. Both steps edit `compute/catalog/cuts.rs`.
- **Q1: (a).** The nearest earlier enabled op of the same setup that cuts the same model and
  states an axial leave.
- **Q2: (b).** The label stays and the row is re-filed "later (a raster width rule)". No proposal
  falls for a reason that is not a limit.
- **Q3: keep the default table for the speeds.** `P28-31` closes in P27.
- **Q4: yes.** A peck gets `None(DrillCycle)`.
- **Q5: trace first.** A feed that rises three times (1331 to 3994 mm/min, 8000 to 24000 rpm)
  with no known cause does not land. The first task of the 9a implementer is the trace, by the
  accused path itself (`suggest_for_operation` on the FM1 cell). The lead rules on 9a after the
  trace.
- **Calibration.** Section 9.1 has no "off" run on the P29-9 build. The implementer proves FM1
  byte-equal before the first number-moving edit of each commit.

## 12. Prototype and runs

One build with the P29-8 prototype (P29-8 note, section 15), plus a throwaway edit of
`feeds/input.rs`, `feeds/envelope.rs`, `feeds/force_budget.rs` and `compute/catalog/cuts.rs`,
restored from byte copies (`sha256sum -c` OK, then touched so cargo rebuilds). Switches:

| Variable | On |
|---|---|
| `P29P_P299=chamfer` | the calculator, the envelope axis and the force-budget cap read `(chamfer_width + tip_offset) / tan(half angle)` on a V-bit (the formula of `ops/chamfer.rs:114-119`); the `CHAMFER_STATES_NO_DEPTH` arm is skipped |
| `P29P_P299=ring` | `offset_ring` is true for a contour Pocket only (Roughing role and a stated stepover kept) |
| `P29P_FINISH_DEPTH=0.5` | the calculator reads 0.5 mm as the depth of a finish with `Surface { max: None }` (the Q3 alternative) |

The check values of 9b and 9c were computed by the model functions (`CutLoad::of` on
`Engagement::from_cut` at the declared width, depth and chip, with the tool helix) in a probe test,
deleted. Calibration: see section 9.1 (no off run on this build; each mode left every other row
equal).

## 14. The chamfer feed raise (trace)

Traced 2026-11-13 on HEAD `d7e6527b8` (P29-8 8d), by the accused path itself: the FM1 door
`suggest_for_operation` with `DressupConfig::for_op(Chamfer)`, the FM1 tool (V-bit 12.7 mm, 60
deg, 2 flutes), `MachineProfile::default()` (generic wood router: 8000 to 24000 rpm, 0.8 kW,
cutting ceiling 4000 mm/min, no Z axis rate) and `SpindleStrategy::default()` (`MatchChart`). A
throwaway switch in `FeedsInput::of_operation` gave the calculator the depth `(chamfer_width +
tip_offset) / tan(half angle)` = (1.0 + 0.1) / tan 30 deg = 1.9053 mm. Only the calculator was
switched: that alone gives 24000 rpm and 3994 mm/min, so the envelope and the force-budget cap
are not the cause. The edit was restored from byte copies (`sha256sum -c` OK) and the probe was
deleted.

### 14.1 The steps (MDF; plywood is the same with 180 m/min)

| Step | Off (today) | On (true depth) |
|---|---|---|
| vendor row | `onsrud-mdf-37-80-1-trace`, band 0.06657 to 0.09985 mm, no printed RPM | same |
| `step1_rpm` | 8000 | 24000 |
| `step3_engagement` depth | 0.762 (family default) | 1.9053 |
| chip (band) | 0.08321 | 0.08321 |
| `step5_feed` | 1331.36 | 3994.08 |
| `step8_plunge` (`g10_plunge_vbit60`, 1/3) | 443 | 1331 |
| Step 5c (`rpm_choice`) | not run (`MatchChart`) | not run |
| power | 0.00039 kW | 0.00406 kW |

The first step that differs is `step1_rpm`. Nothing else moves: the row, the band and the chip
are equal; the feed is the chip x Z x rpm; the plunge is 1/3 of the feed.

### 14.2 The mechanism

The row prints no RPM, so Step 2 does not replace the Step 1 RPM. Step 1 is the surface-speed
rule: `rpm = V x 1000 / (pi x d_eng)`, clamped to the spindle range (`calculator/speed.rs`).
`d_eng` is the cone width at the depth that `FeedsInput::axial_depth_mm` states, else at the
tool diameter (`input.axial_depth_mm.unwrap_or(d)`, `engaged_diameter`).

- Today `CutSpec::calc_inputs` gives no depth for `Axial::ChamferWidth`, so Step 1 reads a depth
  of 12.7 mm. The cone width is `2 x 12.7 x tan 30 deg` = 14.66 mm, clamped to 12.7 mm. Then
  `170000 / (pi x 12.7)` = 4261 rpm (plywood 4511), clamped UP to the spindle minimum 8000.
- With the true depth the cone width is `2 x 1.9053 x tan 30 deg` = 2.200 mm (= 2 x (width +
  tip offset)). Then `170000 / (pi x 2.2)` = 24597 rpm (plywood 26043), clamped DOWN to the
  spindle maximum 24000.

Today the calculator reads two different depths in one call: Step 1 reads 12.7 mm, Step 3 reads
the family default 0.762 mm.

### 14.3 The answers

1. Mechanism: Step 1, the surface-speed RPM at the engaged cone width. The width moves from
   12.7 mm (no depth: the tool diameter) to 2.2 mm (the chamfer depth). The RPM moves from the
   spindle minimum to the spindle maximum; the feed follows at the held chip.
2. Today's 8000 rpm is the defect. It is the spindle minimum, reached because Step 1 reads the
   full tool diameter: the cut engages 2.2 mm, so 8000 rpm gives `pi x 2.2 x 8000 / 1000` =
   55.3 m/min against the 170 m/min of the rule (MDF). 24000 rpm is the same rule at the true
   width, under its ideal 24597 rpm. It is not a new defect. Both values stand on the surface
   speeds `material.base_cutting_speed_m_min.sheet_good` (170) and `.plywood` (180), which are
   `Unsourced` F201 rows.
3. The new point at the true depth (MDF): chip 0.0832 mm inside the band 0.0666 to 0.0999 mm and
   above the rubbing floor 0.025 mm; feed 3994 mm/min under the cutting ceiling 4000 mm/min (6
   mm/min of room); power 0.0041 kW of 0.8 kW; force 5.48 N with a force-budget cap of 6.27 mm
   over the 1.905 mm depth (section 9.1, `P29-9_predictions_fm1.csv`). Plywood has no power or
   force number (power `None`; `force_n` is empty in FM1 today too). The vertical rate has no
   number on this machine: the plunge limit holds no axial chip (`axial: None`) and the machine
   states no Z axis rate (`NotStated`), so the plunge 1331 mm/min is not judged. The ramp feed
   is none (`EntryOff`).
4. Recommendation for 9a: land the true depth. The raise is the correct reading of the existing
   rule; today's 8000 rpm is an artefact of the missing depth (row `P28-32`). Name Step 1 as the
   cause in the commit and list the four moved `rpm`, `feed_mm_min` and `plunge_mm_min` cells.
   Two follow-ups outside 9a: (a) Step 1 reads the full tool diameter for every cone tool with
   no stated depth while Step 3 reads the family default; one depth door (section 2.1) closes
   this for every op, not only the Chamfer; (b) the feed sits 6 mm/min under the machine
   ceiling, so a wider chamfer reaches the Step 7 feed cap.

## 15. Lead rulings after the trace (2026-11-13)

- **Q5: 9a lands with the true chamfer depth.** The trace (section 14) shows that today's 8000 rpm
  is the defect: Step 1 reads the cone at the tool diameter when the op states no depth. The
  commit names Step 1 as the cause. The predicted cells of 9a stand.
- **The entry chip rule (`StoredFeedChip`).** The margin of a check follows the judged value, so
  a row never reads `Within` with a margin below zero.
- **The double rounding is fixed at its source, as its own commit before 9a.** Today Suggest
  rounds the lifted cut feed down to a whole mm/min, and the ramp and the plunge then read that
  stored feed over the lift and round down again. So a ramp can sit `1 + 1/lift` mm/min under
  the band feed. The ramp and the plunge read the unlifted feed before the first rounding.

  Prediction of that commit (reasoned, written before the code):
  - FM1: only `ramp_feed_mm_min` and `plunge_mm_min` cells move. Each moved cell rises by
    exactly 1 mm/min. No cell falls. No other column moves, and the simulation file is
    byte-equal.
  - No shipped ramp or plunge is above the shipped cut feed of its cell, and none is above its
    vertical-rate bound.
  - The project files: a Suggest ramp or plunge value rises by 0 or 1 mm/min (wanaka idx 0 and
    12: ramp 6447 to 6448). No G-code moves: a stored value changes only on an apply.
  - After it, the 8 Face `ChipMin` entry rows read `Within`. The 4 rivmap rows stay `Exceeds`
    (a real chip under the band).
  A cell that moves in another way stops the work.
