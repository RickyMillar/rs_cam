# Runtime survey — what depth the engine actually measures

Read-only survey. Scope: the dexel simulation, the tool-load gates and
`feed_modulation`. The operation parameter schema and toolpath generation are
another agent's scope; this document names them only where the runtime path
crosses them.

Every claim below carries a `file:line`. Claims marked **inferred** come from
reading the code, not from running it. No cargo command ran for this survey.

---

## 1. Answer to question 5 — does the loop close?

**Yes, but not directly, and not for every operation.** The runtime power
verdict never reads the number the feeds calculator would change. The gate
reads a **measured** per-sample axial depth that the dexel simulator takes off
the stock model (`tool_load/power.rs:422` consumes `s.axial_doc_mm`, which
`dexel_stock/simulation.rs:1096` writes from the stamp). The feeds calculator
reads a **per-operation scalar** `ap` (`feeds/mod.rs:1524`, `feeds/mod.rs:1782`).
The two numbers meet only through the toolpath: a smaller recommended depth
writes `depth_per_pass`, the generator emits shallower passes, the dexel then
measures a smaller `axial_doc_mm`, and the peak power falls. That chain is
real and it is pinned by a test — `axial_engagement_vs_dpp_detector_f2.rs`
asserts no steady-state sample reads more than 1.5x the commanded
`depth_per_pass` across pocket, profile, adaptive, zigzag and trace. So for
those families a depth recommendation does move the verdict. Three conditions
break it. First, the operator must apply the recommendation with
`ApplyScope::CutGeometry` or `Both`; the default scope is `Speeds`, which never
writes DOC (`feeds/suggest.rs:932-938`). Second, the toolpath must be
regenerated and re-simulated, because the verdict is a function of the trace,
not of the operation. Third, and most important, `set_depth_per_pass` has a
default implementation that returns `false` and writes nothing
(`compute/catalog.rs:679-681`); only ten configs override it
(`compute/operation_configs.rs`, 10 impls). For an operation without the field
the apply path takes `if let Some(v) = scratch.as_params().depth_per_pass()`
(`feeds/suggest.rs:936`), gets `None`, and the recommendation disappears with
no warning. For a 3D surfacing op the depth is set by the model surface, not by
a parameter, so even a successful write would not bound the measured
engagement.

---

## 2. Where the depth enters the power model

There is one power formula and three call sites. The formula is
`PowerTerms::of` (`tool_load/power.rs:170-215`), and the axial depth enters it
twice: as `cross_section_mm2` (the shear term, `power.rs:186-187`) and as
`axial_doc_mm` (the edge term, `power.rs:204`). The edge term is linear in `ap`
and carries no feed at all.

### 2.1 The simulation-side verdict — measured, per sample

`tool_load::power::evaluate` walks the trace one sample at a time.

| What | Where | Value |
|---|---|---|
| depth input | `tool_load/power.rs:422` | `s.axial_doc_mm` — measured |
| engaged radius | `tool_load/power.rs:393` | `tool.engagement_radius(s.axial_doc_mm)` |
| cross-section | `tool_load/power.rs:417` | `tool.mrr_cross_section_mm2(s.axial_doc_mm, radial_width)` |
| immersion arc | `tool_load/power.rs:383` | `s.arc_engagement_radians` — measured |
| available power | `tool_load/power.rs:439` | `machine.power_at_rpm(s.spindle_rpm) * safety_factor` |
| trip | `tool_load/power.rs:466`, `:534` | peak over contributing samples vs available |

The verdict is a **peak over samples**, not a mean. The deepest steady-state
sample decides the verdict.

Samples are filtered before they reach the peak. Non-cutting samples and
samples under 2 % radial engagement drop out (`power.rs:380`). Samples with no
arc drop out (`power.rs:383`). Phantom transit samples — WaterlineCleanup,
LinkBridge, LeadOut, DressupArtifact — drop out entirely because their dexel
depth reads `stock_top - cutter_z` over uncleared neighbouring stock
(`power.rs:447`, predicate at `tool_load/locality.rs:194-221`). Configured
Entry samples (plunge, ramp, helix) bypass the trip and surface separately as
`entry_spike` (`power.rs:450-457`).

### 2.2 The feeds calculator — a per-operation scalar

`feeds::calculate` derives one `ap` for the whole operation.

1. `default_engagement` returns `ap = ap_factor * tool_diameter`
   (`feeds/mod.rs:1524`, factors at `feeds/mod.rs:2218-2261`).
2. An operation hint overrides it when the operation carries one
   (`feeds/mod.rs:1546`). The hint comes from `operation_feeds_hints`
   (`feeds/suggest.rs:1565-1574`) and is `Option<f64>`.
3. Flute guard, minimum and slotting cap clamp it
   (`feeds/mod.rs:1556-1585`).
4. Step 6 builds the power terms from that single `ap`
   (`feeds/mod.rs:1781-1782`, assembler at `feeds/mod.rs:1057-1077`).
5. It is published as `FeedsResult::axial_depth_mm`
   (`feeds/mod.rs:2086`).

### 2.3 Do the two differ?

**Yes, and the difference is structural, not a bug.** The gate's depth is a
measurement of the worst sample. The calculator's depth is a plan for the whole
operation. They coincide only when the generator honours `depth_per_pass` and
the cut is uniform. On a 3D surfacing pass, a ramp, or any cut whose engagement
follows the model, the calculator's `ap` is a diameter-derived guess and the
gate's `peak_axial_doc_mm` is whatever the surface handed the cutter.

This matters for the derate plan. A recommendation phrased as "use a smaller
`ap`" changes the calculator's scalar. It changes the verdict only through
regeneration. The plan's assumption — that the depth is a single number
describing the whole operation — holds on the **calculator** side and does not
hold on the **runtime** side.

---

## 3. Per-move engagement — it exists, and it is genuinely measured

### 3.1 The measurement

The dexel stamp measures the axial depth per cell as the material length
removed by that stamp, and keeps the maximum over the cells
(`dexel_stock/stamping.rs:1596-1600`). `StampPartial::finish` publishes it as
element 0 of the metric tuple (`dexel_stock/stamping.rs:1222`).
`apply_subsegment_metrics` writes it onto the sample
(`dexel_stock/simulation.rs:1096-1097`), splitting it between
`axial_engagement_mm` (lateral and arc and helix cuts) and `plunge_descent_mm`
(pure-vertical plunges) so one scalar does not carry two meanings
(`dexel_stock/simulation.rs:1078-1083`).

The sample carries three related fields (`simulation_cut.rs:178-186`):

- `axial_doc_mm` — legacy wire name, identical to `axial_engagement_mm`.
- `axial_engagement_mm` — the measured lateral engagement in mm.
- `plunge_descent_mm` — Z descent on a plunge sample only.

There is one sample per **subsegment**, not per move, and a move is cut into
subsegments by length and by Z drop (`dexel_stock/simulation.rs`, reserve
arithmetic documented near `:1585`). So the resolution is finer than per-move.

### 3.2 What consumes it

| Consumer | Site | Reads |
|---|---|---|
| Power gate | `tool_load/power.rs:422` | per-sample `axial_doc_mm`, takes the peak |
| Deflection gate | `tool_load/deflection.rs:88` | per-sample `axial_engagement_mm` |
| Chipload gate | `tool_load/chipload.rs:535-538` | max over steady-state samples |
| Summary | `simulation_cut.rs:1455-1457` | `peak_axial_doc_mm`, transit samples excluded |
| Feed modulation | via `PerMoveEngagement` — see 3.3 | per-move mean **fraction** |
| Modulation nominal | `session/compute.rs:2371-2379` | per-toolpath **max** `axial_engagement_mm` |

So the answer to question 4 is yes on both halves. A ramp entry, a helical
plunge and a 3D surfacing pass all produce genuinely varying per-sample
engagement, and the simulator measures it. What consumes it is mostly an
extreme-value reduction: the power, deflection and chipload gates each collapse
the varying signal to a peak or a max before comparing against a bound. Nothing
on the power path consumes the **distribution**.

### 3.3 Feed modulation sees a varying depth, but a wrongly scaled one

`feed_modulation` does adjust the feed per move, and it does read a per-move
engagement. The per-move value is `PerMoveEngagement::axial_doc_fraction`, a
time-weighted mean over the move's samples (`session/compute.rs:2309-2311`,
struct at `feed_modulation.rs:158-168`).

The solver converts that fraction back to millimetres in one place:

```
fn effective_axial_mm(engagement, ctx) -> f64            // feed_modulation.rs:574
    ctx.nominal_axial_doc_mm * engagement.axial_doc_fraction
```
(`feed_modulation.rs:574-589`)

Both the deflection cap (`feed_modulation.rs:436`) and the power cap
(`feed_modulation.rs:484`) use it.

**The two operands are not on the same scale.** The fraction the simulator
publishes is a fraction of the **flute length**:

```
axial_doc_fraction: Some((axial_engagement_mm / flute_length).clamp(0.0, 1.0))
```
(`dexel_stock/simulation.rs:1106`; `flute_length = cutter.length()` at
`dexel_stock/simulation.rs:481`; the field's own doc at `simulation_cut.rs:76`
says "axial depth-of-cut as a fraction of flute length").

But `nominal_axial_doc_mm` is the toolpath's **peak measured depth in mm**:

```
let max_axial = ...max of s.axial_engagement_mm...;
let nominal_axial = if max_axial > 0.0 { max_axial } else { 0.0 };
```
(`session/compute.rs:2371-2379`, passed at `session/compute.rs:2431`)

The field's own doc comment says it is "per-move axial DOC (mm) **at full
engagement**" (`feed_modulation.rs:257-261`), that is, the depth when the
fraction is 1.0. The production value is not that. So the product is

```
peak_axial_mm  x  (move_axial_mm / flute_length)
```

**Worked example (inferred, by reading — not measured).** A 6 mm two-flute
endmill with a 25 mm cutting length taking a 2 mm pass. Peak axial = 2.0 mm.
A steady-state move's fraction = 2.0 / 25.0 = 0.08. `effective_axial_mm` =
2.0 x 0.08 = **0.16 mm**, against a true 2.0 mm. That is 12.5x too shallow,
and the ratio is `flute_length / peak_axial`, so it grows as the pass gets
shallower relative to the tool.

Both the modulator's deflection cap and its power cap are therefore far too
permissive on the production path. The edge term is linear in `ap`
(`tool_load/power.rs:204`) and the shear cross-section is
`axial_mm * radial_width` (`feed_modulation.rs:497`), so both terms shrink by
the same factor and `feed_for_kw` returns a much larger feed than the physics
allows.

**Why no test catches it.** Every fixture in
`tests/constrained_max_modulation_f039.rs` sets `axial_doc_fraction: 1.0`
(lines 97, 136, 172, 220, 278, 311, 316), which makes `effective_axial_mm`
equal `nominal_axial_doc_mm` exactly. The composition is only wrong when the
fraction is a real flute-length fraction, which is exactly what production
supplies and no fixture supplies.

I did not run anything, so I cannot state the measured magnitude. The scale
mismatch itself is plain from the three definitions cited above.

This is directly relevant to the derate plan. If the plan expects
`feed_modulation` to respond to a reduced depth, note that the modulator's
depth input is currently proportional to `peak_axial x move_axial`, so halving
the operation depth would move the modulator's effective depth by roughly a
factor of **four**, not two. **Inferred** from the expression at
`feed_modulation.rs:574-589`; worth a bench before relying on it.

---

## 4. Places a missing depth becomes a fabricated number

Four sites. The first three are on the paths this survey covers.

**4.1 The modulator's zero-nominal fallback substitutes a diameter for a
depth.** When no cutting sample gives a nominal depth, `effective_axial_mm`
returns `fraction x power_inputs.engagement_diameter_mm`
(`feed_modulation.rs:578-588`). A tool diameter is not an axial depth. The
comment calls it "a proxy DOC" and is honest about it, but the value then flows
into `PowerTerms::of` as `axial_doc_mm` (`feed_modulation.rs:496`) with no
marker that it was invented. The deflection cap consumes it the same way
(`feed_modulation.rs:436`).

**4.2 The feeds calculator substitutes the tool diameter for a missing DOC.**

```
let axial_doc_for_eff_d = input.axial_depth_mm.unwrap_or(d).max(0.0);
```
(`feeds/mod.rs:1110`)

`d` is the tool diameter. The result sets the engaged diameter
(`feeds/mod.rs:1111-1115`), which sets the cutting velocity `Vc` and the
immersion angle in the edge term (`feeds/mod.rs:1068-1073`). For a flat, ball
or bull tool this is harmless — `engaged_diameter_at_doc` ignores the DOC for
those shapes. For a V-bit or a tapered ball it is not: the engaged diameter
becomes the diameter at a depth equal to the whole tool diameter. That is an
absence rendered as a reading.

**4.3 `default_engagement` invents a depth from the diameter whenever the
operation supplies no hint** (`feeds/mod.rs:1524`, `:2218-2261`). This is a
documented default rather than a silent sentinel, but the consequence is the
same for the power model: on every operation whose `operation_feeds_hints`
returns `None` for the axial slot, the power prediction rests on
`ap_factor x diameter`, not on anything about the cut. Which operations those
are is the schema agent's census.

**4.4 A discarded write, not a fabricated value, but the same failure shape.**
`apply_feeds_subset` calls `scratch.set_depth_per_pass(...)`
(`feeds/suggest.rs:877`) and discards the `bool` that says whether anything was
written. The default impl returns `false` (`compute/catalog.rs:679-681`). The
later guarded write (`feeds/suggest.rs:935-937`) then finds `None` and writes
nothing. A depth recommendation for such an operation is dropped without a
warning, and the caller sees a successful apply.

**Counter-examples worth recording.** Several sites on these paths handle a
missing depth correctly and should not be confused with the above. The power
gate falls back to the tool's flute count rather than letting a zero silently
zero the edge term, and says so (`tool_load/power.rs:428-434`).
`PowerTerms::of` contributes zero edge power on unusable inputs rather than a
fabricated one (`tool_load/power.rs:190-206`). `Engagement::axial_doc_fraction`
is an `Option` precisely so an unmeasured axial reads as absent rather than as
zero (`simulation_cut.rs:78-86`). `feed_for_kw` returns `None` when no feed
answers the budget instead of serving a zero feed
(`tool_load/power.rs:232-245`), and Step 6 leaves the feed alone and warns
(`feeds/mod.rs:1819-1827`).

---

## 5. Gaps I could not resolve

1. **The magnitude of the `effective_axial_mm` scale error.** I read the three
   definitions and worked one example by hand. I did not run the modulator, so
   I cannot say what the production feeds actually come out at, nor whether
   some other cap (chipload-max, machine feed, kinematic reach) binds first and
   masks it on most moves. A bench that prints `effective_axial_mm` against the
   sample's own `axial_engagement_mm` would settle it in one run.

2. **Whether the modulated feed feeds back into the power verdict.** The gate
   reads a predicted feed when the trace carries one
   (`tool_load/power.rs:414`, `super::effective_feed_for_sample`). I did not
   trace whether the modulator's output populates `trace.predicted_feeds` on
   the production path, so I cannot say whether an over-permissive modulator
   feed would show up in the verdict or stay invisible to it.

3. **Which operations supply an axial hint.** `operation_feeds_hints` reads
   `operation.feeds_hints()` (`feeds/suggest.rs:1568`). The per-op census is
   the schema agent's scope and I did not duplicate it. The runtime consequence
   is stated at 4.3.

4. **3D surfacing depth bounding.** I did not establish whether any 3D
   finishing operation has a parameter that bounds the axial engagement at all.
   If none does, a depth-based derate has nothing to write there, and the
   verdict cannot be moved by the recommendation. This is the case I would
   check first, because it is where the constant-torque power problem is most
   likely to bite.

5. **Trace staleness after a depth change.** The trace carries
   `operation_config_hashes` (`simulation_cut.rs:150`) and the G-code exporter
   checks it (`gcode/mod.rs:428-430`). I did not check whether the tool-load
   gates or the GUI verdict surface refuse a stale trace, so I cannot say what
   an operator sees between changing the depth and re-simulating.
