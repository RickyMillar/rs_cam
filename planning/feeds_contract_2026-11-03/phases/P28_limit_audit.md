# P28: every exemption from a feeds or load limit

Read-only audit, 2026-11-11. No product code, no cargo run. Source: the code at HEAD
`e4558d936` and the phase notes. Line numbers are the lines read on that commit.

The operator, 2026-11-11: "those scope decisions did not go past me then! that's no good. I
did a whole phase to try and unify the toolpath code so things like force wouldn't be skipped
by some! I want to make sure we force things like that to be implemented across all
toolpaths if needed! How did we miss that?"

## 0. How it was missed

- Each limit decides its own scope in its own code: a `match` arm, an early `return`, or a
  flag on the op row. There is no one list of "which op reads which limit".
- P13c wrote the Adaptive exclusion in its phase note ("The Adaptive family keeps
  `adaptive_doc_factor`") and in code comments. It was not put to the operator as a question.
- The sentries then pinned the exclusion as correct behaviour (`tool_load/CLAUDE.md`: "the
  Adaptive factor stays"), and the feeds matrix was blessed with it. From then on a test run
  could not show it as a defect.
- The operator's rulings (O1, O4) name no exclusion. The code is narrower than the ruling.

## 1. The ops and how the code files them

Every limit below keys on two labels of the op row (`compute/catalog/registry.rs`): the feeds
family and the pass role. The filing itself decides which limits an op gets.

| Family, role | Ops |
|---|---|
| Pocket, Roughing | Face, Pocket, Zigzag, Bore |
| Contour, Roughing | Profile |
| Adaptive, Roughing | 2D Adaptive, 3D Adaptive |
| Drill, Roughing | Drill, Pin Drill |
| Contour, SemiFinish | Waterline |
| Trace, Finish | Trace, VCarve, Inlay pocket, Inlay plug, Chamfer, Pencil, Project Curve |
| Parallel, Finish | Parallel finish, Ramp Finish, Radial Finish, Horizontal Finish |
| Scallop, Finish | Scallop, Unified Finish, Spiral Finish |
| Contour, Finish | Steep/Shallow |

Note: Trace, VCarve, the two Inlay ops, Chamfer and Project Curve cut a full-width groove or
slot, and they are filed as Finish.

Wanaka toolpaths by op: Face idx 0, 12; 3D Adaptive idx 1, 2, 14, 15; Pin Drill idx 3; Drill
idx 4; Project Curve idx 5, 9; Pocket idx 6, 7, 8, 11; Trace idx 10, 13; Scallop idx 16;
Profile idx 17.

## 2. The exemptions

Classes: PHYSICAL (the limit has no meaning there), SCOPE (left out or deferred),
UNSOURCED RULE (a rule of thumb stands in), UNKNOWN (no reason found), SOURCED RULE (a
different rule with a printed source and an operator ruling).

| # | Limit | Who is exempt | Where | Gets instead | Stated reason | Class | Ruling | Wanaka |
|---|---|---|---|---|---|---|---|---|
| 1 | Force budget (depth cap) | Adaptive family | `machine/mod.rs:301` `RigidityProfile::depth_cap_rule` | `adaptive_doc_factor x D` (2 x D), no source | P13.md: "The Adaptive family keeps `adaptive_doc_factor`." | SCOPE | None. Operator O1 (STATUS.md:79): "derive the hobby depth from a force budget per machine class ... it replaces the typed 0.25 D depth". No exclusion in it. Lead or agent decision. | idx 1, 2, 14, 15 |
| 2 | Force budget | Finish and SemiFinish wall and surface finishes | `machine/mod.rs:303` | Nothing; the post-sim depth row is "Reported" (`tool_load/depth.rs:288`) | depth.rs: "No vendor publishes an axial finishing cap ... The deflection gate is the limit on that pass." | PHYSICAL | Operator R2, 2026-09-23 (`feeds_matrix_2026-09-23/RULINGS.md:52`): "I want the most flexibility for the user" (no depth ceiling on finishing passes; deflection decides). | idx 16 |
| 3 | Force budget | Slot and groove ops filed as Finish: Trace, VCarve, Inlay pocket, Inlay plug, Chamfer, Project Curve | The op rows (`registry.rs:570, 320, 386, 450, 778, 1502`) with rule 2 | Nothing | None. R2 was ruled for "radial-load finishes"; these ops are not wall finishes. | UNKNOWN | None found for the filing. | idx 5, 9, 10, 13 |
| 4 | Force budget | Drill family | `machine/mod.rs:300` | The drill gates | "Z-only kinematics" | PHYSICAL | - | idx 3, 4 |
| 5 | Width rule (most removal inside the limits) | Adaptive family | `feeds/envelope.rs:2461` `axes_plan` (`computed_width`), `:1287`; `feeds/suggest.rs:753` | `min(stated, cap)`; the add door writes `adaptive_woc_factor x D` | P13.md: "Adaptive and finishing ops keep `propose` (min(stated, binding cap))." | SCOPE | None. Operator O4 (STATUS.md:81): "the envelope proposal is the cut with the most removal per minute inside the limits". No exclusion in it. | idx 1, 2, 14, 15 |
| 6 | Width rule | Finish ops with a stepover (Steep/Shallow, Spiral, Horizontal, Parallel with no scallop target) | same | The stated or default width | P13.md, same sentence | SCOPE | None | - |
| 7 | Corner-coverage bound | Every op outside the Pocket family | `feeds/envelope.rs:2516` | Nothing | The rule is for offset rings | PHYSICAL | O3 names the Pocket only | - |
| 8 | A width ceiling for an adaptive cut | Adaptive family | `feeds/mod.rs:2793-2830` (default), `compute/validate.rs:283` (static floor 0.15 D) | `adaptive_woc_factor` as a default and a hint; no envelope limit but the diameter | Unsourced port of a reference calculator | UNSOURCED RULE | None | idx 1, 2, 14, 15 |
| 9 | Move budget (runtime floor of the width) | Ops with `MoveCountModel::FeatureDriven` or `NotModelled` (Unified Finish), ops with no stated stepover | `feeds/envelope.rs:3385`; `catalog/capabilities.rs:622-628` | Nothing | "No envelope model is written for this op." | SCOPE | None | - |
| 10 | Power (ladder, power edge, RPM power limit) | Drill family | `feeds/calculator/power.rs:108-112`, `spindle.rs:225`, `close.rs:356` | An advisory figure on the axial circle; nothing acts on it | "the force line is not measured on an end cut" | SCOPE (no force source for an axial cut) | Recorded as "Ruling 2026-11-08" in `acceptance/wanaka_21.md` (idx 3 note); the note does not say who ruled. | idx 3, 4 |
| 11 | Every pre-sim limit that reads a depth: power, deflection, force, removal rate, flute guard, depth proposal | Project Curve (`Axial::SurfaceOffset`) | `catalog/cut_spec.rs:165`; `feeds/calculator/cut.rs:68, 100`; `feeds/envelope.rs:1281`; `feeds/operating_point.rs:262` | Nothing before the simulation; the gates judge after | P16 (review D7): "The offset is not a depth into the stock. The simulation gives that depth" | SCOPE (a bound at the stated offset is possible) | Review item D7, lead. No operator ruling found. | idx 5, 9 |
| 12 | Power slot case (the full-width first cut) | 3D Adaptive; Face; 2D Adaptive unless `slot_clearing` | `compute/catalog.rs` `cut_spec` (`FirstPass::NotSlot` on those rows) | The planner-cap width `c D` only | None | UNKNOWN | None | idx 1, 2, 14, 15 |
| 13 | Planner width of the power case | 3D Adaptive under Contour Parallel | `compute/catalog.rs:1366` `planner_width` | `trochoid_cap_mult` (1.6) x the stepover | "The value has no source (operator ruling Q2); it stays until a contour-parallel width rule exists (P23a open item)." | UNSOURCED RULE | Operator Q2 knows that it has no source | any Contour Parallel rough |
| 14 | Pass ceiling held by the planner | 3D Adaptive under Contour Parallel | same | No bound on the cut width | "that plan holds no bound" | SCOPE (P23a open item) | None | any Contour Parallel rough |
| 15 | Deflection limit before the simulation | Finish and SemiFinish ops | `feeds/envelope.rs:2890` `deflection_limit` | Nothing pre-sim. The finish deflection value of `axial_envelope_for_operation` is computed and only its vendor bound reaches the envelope. | "The tip-deflection edge (roughing, as pass 6)" | UNKNOWN | None. It contradicts R2, where deflection is the limit of a finish. | idx 5, 9, 10, 13, 16 |
| 16 | Deflection limit value | A rough pass: the envelope, Suggest and the modulator use 200 um; the post-sim gate uses the radial stock to leave | `tool_load/deflection.rs:100-150` `exceeds_limit`; `session/compute.rs:2346` | Two different limits for one cut | "out of P17's scope, recorded in its card" | SCOPE | None | every rough |
| 17 | Deflection | Drill family | `tool_load/deflection.rs:273`, `feeds/predict.rs:313` | Nothing | "no continuous radial engagement" | PHYSICAL | - | idx 3, 4 |
| 18 | Chip-thinning lift | Finish role | `feeds/geometry.rs:62` `RadialThinning::of`; `feeds/calculator/feed.rs:88` | No lift (a lower feed) | "Garr's finish row (2.5 % D) holds a low chip on purpose" | SOURCED RULE | Operator, 2026-11-05 (STATUS.md:28): "yes, as planned" | idx 5, 9, 10, 13, 16 |
| 19 | Axial chip thinning of a ball at a shallow depth | Ball and tapered ball | `feeds/calculator/feed.rs:95` | "OBSERVED, NOT APPLIED" | "the charts print no rule for a depth below 1 x D" | SCOPE | None | idx 16 |
| 20 | Chip band gate | Drill family | `tool_load/chipload.rs:389` | The drill gates; the axial chip is the side row x 1/Z (rule G6) | "no continuous chipload" | PHYSICAL | - | idx 3, 4 |
| 21 | Every load gate (chip, power, deflection, depth) | Entry spans: plunge, ramp, helix and the cut out of the helix | `tool_load/locality.rs:156-161` `is_steady_state_for_gate` | `project.entry_load`: peak bite above 2 x the median bite of the body, with 50 or more body samples (`stock/sim_triage.rs:1293-1372`) | "a configured plunge / ramp / helix entry IS a real engagement transient the operator can dial in" | UNSOURCED RULE | None found | idx 2 (critical entry load), idx 1 |
| 22 | Every load gate | `WaterlineCleanup` spans (the cleanup sweep of a 3D Adaptive, real cutting) | `tool_load/locality.rs:124-129, 208` | Nothing | "not a steady-state condition the operator dialed in (Roadmap F.3)" | UNKNOWN | None | idx 1, 2, 14, 15 |
| 23 | Every load gate | Lead-out, link bridge and dogbone spans | `tool_load/locality.rs:194-214` | `project.link_cuts_stock` for a link | The stock reading there is of neighbouring material, not of the cut | PHYSICAL | - | - |
| 24 | Vendor depth table | Drill peck | `feeds/envelope.rs:2761` | The peck rule | A peck is not a side-cut depth | PHYSICAL | - | idx 3, 4 |
| 25 | Vendor axial bound | Every op with `AxialEnvelope::NotApplied`: Face, Pocket, Zigzag, Profile, 2D Adaptive, Trace, the Inlay ops, Bore, Chamfer, Pencil, Ramp Finish, the drills | `feeds/envelope.rs:2500-2513`; `catalog/capabilities.rs:672-675` | Nothing | "The other invariant passes cover them." Those passes (pass 0, pass 6) were deleted in P5. | UNKNOWN (stale reason) | None | idx 6, 7, 8, 11, 17 |
| 26 | Feed cap arms (Step 7) | Drill family | `feeds/calculator/feed.rs:232` | The drill feed envelope of Step 9c | A drill has one vertical feed | PHYSICAL | - | idx 3, 4 |
| 27 | Tip plunge cap (150 mm/min per mm of tip) | Flat, bull and V-bit tools | `tool_load/plunge_stress.rs:30-48` | No cap | "no flute-tip plunge risk"; "a repo rule with no stored source" | UNSOURCED RULE | None | flat tools: all roughs |
| 28 | Plunge-entry stability warning | Every family but Adaptive | `feeds/suggest/adaptive_entry.rs:117` | No warning | None | UNSOURCED RULE | None | idx 6, 7, 8, 11 |
| 29 | Modulator load caps | Lead-in and lead-out moves | `dressup/feed_modulation.rs:680-702` `should_skip_modulation` | The stated lead feed | "operator-tuned" (F-040) | SCOPE | None | ops with leads |
| 30 | Modulator load caps | Rapid, retract, drilling and straight entry plunge moves | same | The plunge rules | No lateral cut | PHYSICAL | - | - |
| 31 | The cut when the operator states none | Every family and role has its own factors | `feeds/mod.rs:2306-2412` `operation_default_profile` | A table of depth and width factors | "DOC/WOC matrix of reference/shapeoko_feeds_and_speeds calcs.rs (not in the repo)" | UNSOURCED RULE | None | ops with an unset axis |
| 32 | Depth that the calculator reads | Bore (helix pitch), Chamfer (width), drill peck | `catalog/cut_spec.rs:335-352` `calc_inputs` | The family default depth; the wall depth of a Bore is read by no pre-sim limit | "A peck, a helix pitch and a chamfer width are not a side-cut depth" | UNKNOWN | None | - |
| 33 | Deflection and chip models at the tip of a V-bit | V-bit | `feeds/predict.rs` (flute relief caveat), `tool_load/chipload.rs:885` | A caveat, or "cutter mode unsupported" | The cone tip is outside the model | UNKNOWN | None | idx 9, 10, 13 |
| 34 | Force budget, the other readers | The exclusions of rows 1 to 4 also reach: the modulator (`ForceBudgetScope::OutOfScope`, `feed_modulation.rs:462`), the force card budget line, the post-sim depth gate | one door, `depth_cap_rule` | as rows 1 to 4 | "The op's rule is not the force budget (the Adaptive factor, a finish pass, a drill): no limit." | SCOPE (follows row 1) | None | as row 1 |

Not counted as exemptions: a material with no force line gets no power, deflection or budget
figure (ruling B6, stated on the card). The zone checks (core, shank, holder, shoe, flute)
have no op, role or span exemption that I found; a zone above the flutes that is no wider
than the cutter is shadowed by it (physical).

Count: 34 rows. PHYSICAL 9. SCOPE 11. UNSOURCED RULE 6. UNKNOWN 7. SOURCED RULE 1.

## 3. Three lists

### 3.1 SCOPE and UNKNOWN, by how much they can matter to a real cut

1. Row 1: the force budget does not reach an adaptive rough. The two wanaka roughs are 1.6
   and about 4 times over it (P26).
2. Row 21: nothing judges the load of an entry or of the cut out of a helix, except a
   relative rule. Wanaka idx 2 holds a critical there.
3. Row 3: slot and groove ops filed as Finish get no force budget and no pre-sim deflection.
4. Row 12: the power of a 3D Adaptive is never sized at a full slot.
5. Row 22: the cleanup sweep of a 3D Adaptive is real cutting and no gate reads it.
6. Row 15: a finish has no deflection limit before the simulation.
7. Row 11: a Project Curve has no pre-sim load figure at all.
8. Rows 13 and 14: a Contour Parallel rough has no width bound, and its power case uses 1.6.
9. Row 5: no width proposal for an adaptive rough.
10. Row 16: two deflection limits for one rough pass.
11. Row 25: the vendor axial bound skips every 2D op.
12. Rows 32, 33, 6, 9, 19, 29: smaller.

### 3.2 Limits that a family never reads

| Family or op | Never reads |
|---|---|
| Adaptive (2D and 3D) | force budget; width rule; any width ceiling but the diameter; the slot power case (3D) |
| Finish-role ops (15 ops) and Waterline (SemiFinish) | force budget; pre-sim deflection; width rule; chip-thinning lift |
| Drill, Pin Drill | power as a limit; deflection; chip band; force budget; depth table; feed cap arms |
| Project Curve | every pre-sim limit that reads a depth |
| The 2D ops | the vendor axial bound |
| Contour Parallel 3D rough | the pass ceiling |

### 3.3 Checks on one side of the simulation only

After the simulation only: the entry load; the depth row of a finish; deflection of a finish;
every load figure of a Project Curve; a link that cuts stock; the body-zone contacts on the
live stock.

Before the simulation only: the vendor depth table; the vendor axial bound; the corner
bound; the move budget; the default cut table.

On both sides, with different values: the deflection limit of a rough pass (row 16).

## 4. Design sketch: an exemption cannot be added silently

One page, no code.

**The table.** One declared table in core: rows are (feeds family, pass role), columns are
the limits (`LimitKind` plus the gate criteria). Each cell is `Applies` or
`NotApplicable(reason)`. The reason is a closed enum of physical reasons only, for example
`NoSideEngagement`, `OffsetRingRuleOnly`, `DepthKnownAfterSimulation`. It has no member for
"deferred", "kept" or "not yet". A limit that applies with no model yet is `Applies` and
reports "not modelled" on the card; it is never hidden.

**One door.** `limit_scope(op) -> Scope` answers for every reader: the calculator, the
envelope (`axes_plan`, each `*_limit` builder), `RigidityProfile::depth_cap_rule`, the gates
(`tool_load`), the modulator (`ForceBudgetScope`) and the span filter. Today each of these
holds its own `match` or early `return` (the Where column above). They become readers.

**The compiler.** The table is an exhaustive `match` with no wildcard (the workspace already
denies `wildcard_enum_match_arm`). A new op family, pass role or limit does not compile until
its cells are stated. This is the K1 pattern of `CutSpec` and `CutRule`: no `Default`.

**The sentry.** One test pins the list of `NotApplicable` cells. The list only shrinks. A new
cell needs an operator ruling quoted in the test beside it. The same test asserts that every
`Applies` cell has a reader that ran on a fixture op of that row (non-vacuity), in the style
of `every_cell_declares_its_feeds_support_fm0`.

**The span side.** The same table has a second axis for span kinds (entry, cleanup, lead,
link): which gate reads which span, with the same closed reasons. Row 21 and row 22 become
visible cells.

**The filing.** A slot or groove op is not a Finish for load limits. The table keys on the
cut (`CutSpec`: `Radial::Slot`, `Groove`, `FirstPass`) as well as the role, so row 3 cannot
come back through a label.

**The surfaces.** Suggest's reply, the card and MCP print "Limits that do not apply to this
op" with each reason, and "Limits that apply and are not modelled".

**What it builds on.** `OpRegistryEntry` (family, role, capabilities), `CutSpec`/`CutRule`
(K1), `AxesPlan`, `LimitKind`, `RigidityProfile::depth_cap_rule`, `ForceBudgetScope::of`,
`UnmodeledReason::NotApplicableForOp` (its free text becomes the closed enum;
`NotImplemented` goes), the FM0 declaration sentry.

**Steps.**

1. P28-1: the table and the door, filled with TODAY's behaviour. Every exemption of section 2
   becomes a visible cell. Cells with no physical reason get a temporary marker that the
   sentry lists as "needs a ruling". Byte-identical output.
2. P28-2: the operator rules each marked cell: apply the limit, or name the physical reason.
3. P28-3 and after: one phase per ruled cell that changes behaviour (row 1 first), each with
   its FM1 measurement and wanaka read.
4. P28-last: delete the scattered `match` arms; a search guard keeps them out.
