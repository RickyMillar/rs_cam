# DOC survey — the operation model and toolpath generation

Scope: the operation model, the parameter schema and toolpath generation.
The simulator, feed modulation and per-move runtime behaviour are another
agent's task. Every claim below carries a `file:line` citation. The report
marks each inference as an inference.

---

## 1. Does a single scalar depth describe every operation?

**No.** Of the 24 operation configs, only **10** carry an axial
depth-per-pass knob at all (`crates/rs_cam_core/src/compute/catalog.rs:825`
to `:862`): Face, Pocket, Profile, Adaptive, Rest, Zigzag, Trace,
Adaptive3d, Waterline and RampFinish. The other 14 return `None` from
`OperationParams::depth_per_pass` and refuse the write
(`crates/rs_cam_core/src/compute/catalog.rs:673-682`). For those 14 the
apply path writes nothing at all: `apply_feeds_subset` guards the write
with `if let Some(v) = scratch.as_params().depth_per_pass()`
(`crates/rs_cam_core/src/feeds/suggest.rs:960-968`). Worse, even inside
the 10, the scalar is a **cap**, not the realised depth. Every 2.5D
operation builds its Z levels with `DepthDistribution::Even`
(`crates/rs_cam_core/src/compute/catalog.rs:2713`, `:2722`,
`crates/rs_cam_core/src/face.rs:178`, `:311`), which divides the total
depth into `ceil(depth / depth_per_pass)` equal passes
(`crates/rs_cam_core/src/depth.rs:70`, `:106-108`). The realised depth is
therefore `depth / ceil(depth / depth_per_pass)`, which is ≤
`depth_per_pass` and equals it only when the total is an exact multiple.
The lever is quantised: on a 12 mm pocket a `depth_per_pass` cut from
3.0 mm to 2.9 mm moves the realised depth from 3.0 mm to 2.4 mm, a 20%
step, not a 3% one. The plan's assumption of one scalar depth per
operation is false in three separate ways — the field is absent on 14
operations, it is a cap rather than a value on all 10, and on Adaptive3d
one operation carries three distinct stepdowns at once.

---

## 2. Operation table

`DPP` = `OperationParams::depth_per_pass()` returns `Some`.
`Hint` = the operation feeds a depth into `FeedsInput::axial_depth_mm`
(`crates/rs_cam_core/src/compute/catalog.rs:2588-2641`).

| Operation | DPP | Hint | Single scalar? | What sets the depth | Citation |
|---|---|---|---|---|---|
| Face | yes | no | **No — cap** | `depth_per_pass` caps; Even split sets the value | `operation_configs.rs:1698-1707`, `face.rs:174-179` |
| Pocket | yes | no | **No — cap** | `depth_per_pass` caps; Even split, plus `finishing_passes` spring passes at zero nominal depth | `operation_configs.rs:1736-1745`, `catalog.rs:2709-2717`, `depth.rs:120-127` |
| Profile | yes | no | **No — cap** | same as Pocket | `operation_configs.rs:1767-1776`, `catalog.rs:2718-2726` |
| Adaptive | yes | no | **No — cap** | `depth_per_pass` caps; `DepthStepping::new` = Even | `operation_configs.rs:1805-1814`, `catalog.rs:2727-2729` |
| Rest | yes | no | **No — cap** | same; plus the depth reached is bounded by what the previous tool left | `operation_configs.rs:1874-1883`, `catalog.rs:2734-2736` |
| Zigzag | yes | no | **No — cap** | same as Adaptive | `operation_configs.rs:1943-1952`, `catalog.rs:2730-2732` |
| Trace | yes | no | **No — cap** | same; steps only when `depth > depth_per_pass` | `operation_configs.rs:1974-1983`, `trace.rs:118`, `catalog.rs:2737-2739` |
| Adaptive3d | yes | no | **No — three depths** | `depth_per_pass`, `fine_stepdown`, `shallow_stepdown`; plus `detect_flat_areas` inserts shelf levels | `operation_configs.rs:2123-2132`, `:632`, `:673`, `:636`, `catalog.rs:1718-1754` |
| Waterline | yes (alias `z_step`) | yes | Per pass, but see §3 | `z_step` sets the Z-level spacing | `operation_configs.rs:2147-2156`, `catalog.rs:2605-2608` |
| RampFinish | yes (alias `max_stepdown`) | yes | **No — cap** | planner uses `min(max_stepdown, slope-derived)` | `operation_configs.rs:2312-2331` |
| SteepShallow | **no** | yes | **No — two bands** | `z_step` (steep waterline band) + `stepover` (shallow raster band); the hint goes in, no write comes back | `operation_configs.rs:2275-2305`, `catalog.rs:2609-2612`, `catalog.rs:1912-1926` |
| UnifiedFinish | **no** | no (scallop only) | **No — three bands** | `z_step`, `raster_stepover` and `scallop_height`, one per band | `catalog.rs:1822-1845`, `catalog.rs:2594-2597` |
| VCarve | **no** | yes (`max_depth`) | **No — per point** | `depth = dist / tan(half_angle)`, clamped by `max_depth` | `vcarve.rs:87-92`, `catalog.rs:2613-2616` |
| Inlay | **no** | no | **No — per point** | `((dist - gap) / tan_half + flat_depth).clamp(0, pocket_depth)` | `inlay.rs:232`, `:370` |
| Chamfer | **no** | no | **No — derived** | `depth = (chamfer_width + tip_offset) / tan(half_angle)` | `chamfer.rs:41-43` |
| Drill | **no** | no | **No — not axial DOC** | full hole depth; `peck_depth` is chip evacuation | `operation_configs.rs:1992-2016`, `catalog.rs:1677-1696` |
| AlignmentPinDrill | **no** | no | **No — not axial DOC** | `spoilboard_penetration` below the stock | `operation_configs.rs:2033-2035` |
| DropCutter | **no** | no (scallop) | **No — model-driven** | surface heightmap minus prior stock; `min_z` only floors it | `operation_configs.rs:2066-2102`, `catalog.rs:1705-1716` |
| Scallop | **no** | no (scallop) | **No — model-driven** | surface; `scallop_height` is a ridge target, not a depth | `operation_configs.rs:2215-2244` |
| Pencil | **no** | no | **No — model-driven** | valley geometry between two surfaces | `operation_configs.rs:2176-2214` |
| SpiralFinish | **no** | no | **No — model-driven** | surface | `operation_configs.rs:2343-2373` |
| RadialFinish | **no** | no | **No — model-driven** | surface; only an `angular_step` in degrees | `operation_configs.rs:2374-2397` |
| HorizontalFinish | **no** | no | **No — model-driven** | surface | `operation_configs.rs:2398-2428` |
| ProjectCurve | **no** | no | **No — surface offset** | `depth` is an offset below the mesh surface | `project_curve.rs:43-44`, `operation_configs.rs:2429-2452` |

The registry-driven test at
`crates/rs_cam_core/src/feeds/suggest.rs:5618-5666` pins the hint table
op by op, so the four hint-supplying operations are a deliberate, tested
set, not an accident.

### Names for the same physical quantity

`depth_per_pass` (8 configs), `z_step` (Waterline, SteepShallow,
UnifiedFinish), `max_stepdown` (RampFinish), `fine_stepdown` and
`shallow_stepdown` (Adaptive3d), `ap` and `axial_depth_mm` (the feeds
layer), `DOC` (the optimizer axis at
`crates/rs_cam_core/src/tool_load/optimize/axes.rs:134-141`). Three of
these are aliased behind the `depth_per_pass` setter, and the registry
publishes none of the alias names
(`crates/rs_cam_core/src/session/compute.rs:1255-1273`). The project's own
inventory of the naming spread is at
`toolpath_stress_test/GAPS_AND_ISSUES.md:93-103`.

---

## 3. What `feeds::calculate` receives and emits

**Input.** `FeedsInput::axial_depth_mm: Option<f64>`
(`crates/rs_cam_core/src/feeds/mod.rs:394-395`). It is filled from
`OperationConfig::feeds_hints()`
(`crates/rs_cam_core/src/compute/catalog.rs:2588-2641`) through
`operation_feeds_hints`
(`crates/rs_cam_core/src/feeds/suggest.rs:1565-1574`) and
`feeds_input_for_operation`
(`crates/rs_cam_core/src/feeds/suggest.rs:748`).

Only four operations supply it: Waterline `z_step`, SteepShallow
`z_step`, VCarve `max_depth`, RampFinish `max_stepdown`
(`catalog.rs:2605-2620`). **Every other operation passes `None`** —
including all eight that own a `depth_per_pass` field. The calculator
therefore never sees the configured per-pass depth of a Pocket, a Face,
an Adaptive or an Adaptive3d.

**When the input is `None`** the calculator invents a depth from the
diameter and the operation family:
`let (mut ap, mut ae) = default_engagement(d, &profile, input, machine);`
(`crates/rs_cam_core/src/feeds/mod.rs:1523-1524`). The user override, when
present, then replaces it (`mod.rs:1546-1548`).

**Per pass or total?** Mixed, and the mixing is silent. Waterline,
SteepShallow and RampFinish supply a per-pass value. **VCarve supplies
`max_depth`, a total depth, into the same per-pass slot**
(`catalog.rs:2613-2616`). The calculator has no field telling it which it
received.

**Output.** `FeedsResult::axial_depth_mm: f64`
(`crates/rs_cam_core/src/feeds/mod.rs:453`) — a plain `f64`, never `None`,
assigned at `mod.rs:2086`. Consumers:

- `crates/rs_cam_core/src/feeds/suggest.rs:877` — written into the scratch
  clone's `depth_per_pass`, rounded to 0.001 mm.
- `crates/rs_cam_core/src/feeds/suggest.rs:915` — the
  `CalculatorOperatingPoint` the final feed rescale reads.
- `crates/rs_cam_core/src/feeds/efficiency.rs:171`.
- `crates/rs_cam_viz/src/ui/properties/pills.rs:170` — the inline pill.

**The write-back is conditional.** `ApplyScope::CutGeometry` copies the
depth only when the operation owns the field:

```rust
if write_geometry {
    if let Some(v) = scratch.as_params().stepover() { operation.set_stepover(v); }
    if let Some(v) = scratch.as_params().depth_per_pass() { operation.set_depth_per_pass(v); }
}
```
(`crates/rs_cam_core/src/feeds/suggest.rs:960-968`)

So a depth recommendation reaches exactly the 10 operations in §2 and no
others. The GUI already documents this for V-carve: "also VCarve `Max
Depth`, which the funnel does not write — that one falls back to the raw
value, labelled" (`crates/rs_cam_viz/src/ui/properties/pills.rs:168-169`).

---

## 4. Where a depth recommendation does not map

These are ordered by how badly the plan breaks.

### 4.1 V-carve — the depth is the letterform (verified)

Every point's depth is computed from the medial-axis distance:

```rust
let depth = if params.max_depth > 0.0 {
    (dist / tan_half).min(params.max_depth)
} else {
    dist / tan_half
};
```
(`crates/rs_cam_core/src/vcarve.rs:87-92`)

`max_depth` is a ceiling, not a depth. Lowering it does not shave a
constant amount off the cut; it truncates the V-groove wherever the
design is wide, flat-bottoming the widest strokes and destroying the
"design outline meets exactly" property the module documents at
`vcarve.rs:128-133`. There is no per-pass depth to reduce.

Two further facts here. First, the **existing** envelope pass already
mutates `cfg.max_depth` (`crates/rs_cam_core/src/feeds/suggest.rs:1944-1954`)
— so the codebase already does the thing this section argues against, on
the deflection axis. Second, on the `ApplyScope` path that mutation is
**discarded**: `pick_axial_envelope` runs inside `enforce_invariants` on
the *scratch clone* (`suggest.rs:921`), and the copy-back at
`suggest.rs:960-968` never copies `max_depth`. The mutation survives only
through `resolve_operation_invariants`
(`crates/rs_cam_core/src/feeds/suggest.rs:1406-1414`), the optimizer
axis-override path. I found no test pinning this; it is a read of the two
call sites.

### 4.2 Chamfer — the depth is the chamfer width (verified)

```rust
fn chamfer_depth(params: &ChamferParams) -> f64 {
    (params.chamfer_width + params.tip_offset) / params.tool_half_angle.tan()
}
```
(`crates/rs_cam_core/src/chamfer.rs:41-43`)

Reducing the depth reduces the chamfer the user asked for. The config
exposes `chamfer_width`, not a depth (`catalog.rs:1697-1704`), and
`depth_semantics` reports the width itself
(`operation_configs.rs:2054-2056`). The operation has no DPP accessor, so
nothing can be written.

### 4.3 Inlay — the depth is the joint fit (verified)

`pocket_depth` sets the male plug inset (`inlay.rs:96`), the boundary
margin (`inlay.rs:150`, `:340`) and the clamp on both halves
(`inlay.rs:232`, `:370`). Male and female must agree or the inlay does
not seat. A unilateral depth reduction on one of the two toolpaths
breaks the joint.

### 4.4 Drill — the depth is the hole (verified)

The tool is fully enveloped for the whole plunge. `peck_depth` exists for
chip evacuation, not for load
(`toolpath_stress_test/PARAMETER_MATRIX.md:364-366`), and the config has
no DPP accessor (`operation_configs.rs:1992-2016`). The calculator
already treats drill as a special case elsewhere — an RPM envelope clamp
at `crates/rs_cam_core/src/feeds/mod.rs:1512-1520`, and exclusion from
DOC derating at `crates/rs_cam_core/src/feeds/suggest.rs:2185-2192`.

### 4.5 The seven surface-finishing operations — no depth exists (verified)

DropCutter, Scallop, UnifiedFinish, Pencil, SpiralFinish, RadialFinish
and HorizontalFinish carry **no axial parameter of any kind**. All seven
return `DepthSemantics::None` and `depth_per_pass() == None`
(`operation_configs.rs:2066-2102`, `:2176-2244`, `:2343-2428`). Their
axial engagement is the model surface minus whatever the previous
operation left. The only lever is `stock_to_leave`, and the existing
envelope pass deliberately declines to touch it: "automatic
`stock_to_leave` mutation is deferred until in-process stock at gen time
lands" (`crates/rs_cam_core/src/feeds/suggest.rs:1910-1914`). For these
seven a depth recommendation has nowhere to go.

### 4.6 ProjectCurve — the depth is an engraving offset (verified)

`/// Cut depth below (or above) the mesh surface (positive = into material).`
(`crates/rs_cam_core/src/project_curve.rs:43-44`). It is the engraved
groove depth the user asked for. The existing envelope pass treats it as
warning-only for exactly this reason
(`crates/rs_cam_core/src/feeds/suggest.rs:1955-1964`).

### 4.7 The 2.5D quantisation — the lever is not continuous (verified)

For Face, Pocket, Profile, Adaptive, Rest, Zigzag and Trace the realised
depth is `depth / ceil(depth / depth_per_pass)`
(`crates/rs_cam_core/src/depth.rs:65-70`, `:104-109`). Writing a
recommended `depth_per_pass` of, say, 2.4 mm onto a 12 mm pocket
currently at 3.0 mm produces 5 passes of 2.4 mm — which happens to
match. Writing 2.9 mm produces the same 5 passes of 2.4 mm. Writing
2.0 mm produces 6 passes of 2.0 mm. **A small recommended reduction
either does nothing or overshoots.** Any power-utilisation prediction
made from the recommended value, rather than from the quantised value the
planner will actually use, will be wrong. This is an inference from the
Even-distribution arithmetic, not a measured result.

`finishing_passes` compounds it: spring passes repeat the final Z level
(`crates/rs_cam_core/src/depth.rs:120-127`), so those cutting moves have
no nominal axial engagement at all.

### 4.8 Adaptive3d — one operation, three stepdowns (verified)

`depth_per_pass` (`operation_configs.rs:601`), `fine_stepdown`
(`:632`) and `shallow_stepdown` (`:673`, defaulting to
`depth_per_pass * 0.5` at
`crates/rs_cam_core/src/compute/execute.rs:1999-2005`) all set axial
engagement in the same operation. `detect_flat_areas` (`:633`) inserts
further Z levels at shelf heights. `depth_per_pass()` returns only the
first (`operation_configs.rs:2123-2125`). A recommendation written there
leaves the other two untouched, and `shallow_stepdown`'s default then
silently tracks it at half value while an explicit `shallow_stepdown`
does not.

Separately, this is 3D roughing: the first Z pass cuts into a curved
surface, so the actual engagement of each move ranges from zero up to the
level spacing. The scalar is an upper bound on a distribution. That
matches the recorded finding in memory that 3D adaptive never had true
engagement control.

### 4.9 Ramp and helical entry — depth varies within a pass (verified)

The entry dressup replaces a plunge with a descending move
(`crates/rs_cam_core/src/compute/config.rs:1884-1908`,
`DressupEntryStyle::{Ramp, Helix}`). Adaptive3d has its own copy
(`operation_configs.rs:618-633`). During the entry the axial engagement
climbs from zero to the pass depth. No parameter names this depth, and
the operation model has no field for it. The code already knows the
transient is load-relevant: `PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D`
(`crates/rs_cam_core/src/feeds/suggest.rs:1656-1664`) records a measured
362 µm entry transient against a 162 µm steady state on the same cut.

### 4.10 SteepShallow — the hint goes in, nothing comes back (verified)

`SteepShallowConfig` feeds `z_step` into `FeedsInput::axial_depth_mm`
(`catalog.rs:2609-2612`) but implements **no** `depth_per_pass` accessor
(`operation_configs.rs:2275-2305`). The calculator sizes the whole recipe
against a depth it can never revise. This asymmetry exists today; a
depth-recommendation feature would make it visible as a recommendation
the UI shows and the Apply silently drops.

---

## 5. What already clamps the depth

Five mechanisms exist. None of them responds to the power limit.

**Inside `feeds::calculate`** (`crates/rs_cam_core/src/feeds/mod.rs`):

1. **Flute guard** — `ap` capped at `0.8 × flute_length`, or `2 × D` when
   the flute length is unknown. Reports `FeedsWarning::DocExceedsFlute
   { requested, capped }` (`mod.rs:1553-1566`, warning at `:648`).
2. **Minimum engagement** — `ap = ap.max(0.05)`. **Silent, no warning**
   (`mod.rs:1568-1572`).
3. **Slotting cap** — when `ae > 0.85 × D`, `ap` is capped at
   `0.25 × D`. Reports `FeedsWarning::SlottingDetected { doc_reduced_to }`
   (`mod.rs:1574-1586`, warning at `:652`).

**Inside `enforce_invariants`** (`crates/rs_cam_core/src/feeds/suggest.rs:2055-2140`),
in this order:

4. **The axial-DOC envelope**, `pick_axial_envelope`
   (`suggest.rs:1918-2012`), built by
   `cutter_axial_constraints`
   (`crates/rs_cam_core/src/feeds/cutter_constraints.rs:181`). It bounds
   the depth by tip deflection, the matched LUT row's own axial limit, the
   axial scallop and a chipload-burn floor
   (`cutter_constraints.rs:101-117`). Routing
   (`suggest.rs:1792-1893`) — **it mutates only Adaptive3d
   `depth_per_pass` and VCarve `max_depth`**; ProjectCurve gets a warning;
   the eight finish-3D operations get an advisory; every 2.5D operation
   gets `None` and no envelope at all (`suggest.rs:1888-1892`).
   It reports through `SuggestWarning::AxialDocClampedByEnvelope`
   (`suggest.rs:460`), `AxialDocBelowBurnFloor` (`:475`),
   `AxialEnvelopeSafeBandEmpty` (`:446`), `FinishEnvelopeAdvisory` and
   `ProjectCurveDepthInfeasible`. Each carries the binding constraint as a
   string, so the UI can say *which* limit bound the cut
   (`suggest.rs:1695`).
5. **Rigidity clamp**, `clamp_dpp_to_rigidity` (`suggest.rs:2262-2296`) —
   roughing only; caps DPP at `rigidity_factor × D`, branching on the
   adaptive family. Reports `RoughingDepthClampedToRigidity { requested,
   capped }`.
6. **Cutting-length clamp**, `clamp_dpp_to_cutting_length`
   (`suggest.rs:2301-2317`) — all roles; caps DPP at `tool.cutting_length`.
   Reports `DepthClampedToCuttingLength { requested, capped }`.
7. **Deflection back-off**, `backoff_dpp_for_deflection`
   (`suggest.rs:2335+`) — roughing only; multiplies DPP by 0.8 per step,
   at most 5 steps, floored at 0.5 mm, until the predicted tip deflection
   clears 200 µm (`suggest.rs:1604-1622`).

All four of these `enforce_invariants` passes go through
`operation.depth_per_pass()` / `set_depth_per_pass()`, so they reach only
the 10 operations of §2.

**What the power limit does today.** It derates the **feed only**
(`crates/rs_cam_core/src/feeds/mod.rs:1769-1835`), reports
`FeedsWarning::PowerLimited { required_kw, available_kw }` and sets
`FeedsResult::power_limited` (`mod.rs:457`, `:2093`). The code already
states the plan's own premise in a comment at `mod.rs:1812-1819`:

> "No feed rescues that cut: thinning the chip leaves the ploughing power
> exactly where it was… The real fix is less DOC, less stepover or a lower
> RPM, none of which a feed derate can reach for."

The only consumer of `power_limited` outside the calculator is a GUI
label (`crates/rs_cam_viz/src/ui/properties/mod.rs:3252`). Nothing
currently converts a power limit into a depth change.

---

## 6. Gaps I could not resolve

1. **Whether the discarded VCarve `max_depth` clamp (§4.1) is intentional.**
   I verified the two call sites and the GUI comment that describes the
   behaviour, but I found no test pinning it and no planning note
   explaining it. It may be a deliberate safety choice or an oversight.
2. **Whether the Even-distribution quantisation (§4.7) has ever been
   measured against the load model.** I derived the arithmetic from
   `depth.rs`; I did not run anything, and the brief forbids cargo.
3. **What the realised axial engagement actually is per move.** That lives
   in the simulator and the `axial_engagement_mm` measurement, which is
   the other agent's scope. I can say only that the operation model does
   not represent it.
4. **UnifiedFinish's three bands.** I read the parameter table
   (`catalog.rs:1822-1845`) and confirmed `z_step`, `raster_stepover` and
   `scallop_height` coexist, but I did not read the planner to confirm how
   the bands divide the surface.
5. **Whether `min_z` on DropCutter ever acts as a depth limit in
   practice.** `depth_semantics` reports `DerivedStockTop(min_z.abs())`
   (`operation_configs.rs:2093-2095`), which suggests it does something,
   but there is no DPP accessor and I did not trace the planner.
