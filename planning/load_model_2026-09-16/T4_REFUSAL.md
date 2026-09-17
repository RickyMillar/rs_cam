# T-4 — the refusal reads as zero

*Source-only analysis, 2026-09-17. No cargo command ran.*

## Summary

1. `predict_peak_deflection_um` has nine refusal exits and every one returns
   `predicted_um = 0.0`; four production callers read that number.
2. A modelled zero is unreachable, so today **every** `0.0` is an absence —
   the `Result` change loses no information and is strictly additive.
3. The V-bit guard at `predict.rs:219` is stale. The delegate it now calls
   models a V-bit correctly through `lookup_diameter_at`; the guard predates
   the delegation and was not removed with the bespoke formula.
4. The unsafe refusal is the drill guard's neighbour, not the drill guard: a
   V-bit or an unvalidated plastic on a roughing op skips the DPP back-off
   and says nothing.
5. Reuse `UnmodeledReason`, not `CriterionStatus` or `GatePopulation`. The
   closest precedent is `force::DeflectionCapRefusal` in the same folder.

---

## 1. The consumers

`predict_peak_deflection_um` is defined at
`crates/rs_cam_core/src/feeds/predict.rs:186` and re-exported at
`crates/rs_cam_core/src/feeds/mod.rs:40`. Four production call sites exist.

### 1a. `crates/rs_cam_core/src/feeds/suggest/invariants.rs:402` and `:414`

The DPP back-off loop. This is the only consumer that changes machine
behaviour.

```rust
let initial_prediction =
    crate::feeds::predict::predict_peak_deflection_um(operation, tool, material, machine);
let predicted_initial_um = initial_prediction.predicted_um;
let mut predicted_um = predicted_initial_um;
...
while predicted_um > DEFLECTION_BACKOFF_TARGET_UM
    && iterations < DEFLECTION_BACKOFF_MAX_ITERATIONS
    && dpp > DEFLECTION_BACKOFF_DPP_FLOOR_MM
```

**What it does with a `0.0`:** `0.0 > 200.0` is false, so the loop body never
runs. `iterations` stays `0`, so the `if iterations > 0` block at
`invariants.rs:422` never pushes `SuggestWarning::DppCappedByDeflection`. The
operation keeps the DPP the rigidity clamp wrote. Suggest emits no warning, no
rationale entry and no trace event.

The function's own doc comment states the behaviour and calls it correct
(`invariants.rs:388-390`):

> The predictor returns 0 µm for refusal cases (V-bit, drill, un-validated
> material, zero feed) — the loop's `> target` condition short-circuits
> trivially and no back-off occurs.

The comment is accurate about the mechanism. It is silent about the
consequence: a pass-through and a clearance are the same event here.

### 1b. `crates/rs_cam_core/src/feeds/efficiency.rs:368`

`force_headroom`, which feeds `CutEfficiency::force_headroom`.

```rust
let predicted_um =
    crate::feeds::predict::predict_peak_deflection_um(operation, tool, material, machine)
        .predicted_um;
if !(predicted_um.is_finite() && predicted_um > 0.0) {
    return None;
}
```

**What it does with a `0.0`:** it returns `None`. This caller is correct. It
turns the sentinel back into the abstention it is, and its doc comment at
`efficiency.rs:352-358` says exactly why. Note the cost: the decision is made
at the consumer, so the reason is lost. `None` reaches the GUI, which prints
"force headroom not modelled" (`compare.rs:535`, `:548`) or "the
pre-simulation deflection predictor declined this pairing"
(`compare.rs:640-643`). The operator cannot tell a V-bit refusal from an
unvalidated-material refusal.

This caller also over-abstains in one direction: a genuine `0.0` would also
map to `None`. See section 2e — that case is unreachable today.

### 1c. `crates/rs_cam_core/src/feeds/profile.rs:206`

```rust
let predictions = Predictions {
    deflection: predict_peak_deflection_um(
        eval_op,
        input.tool,
        input.material,
        input.machine,
    ),
    move_count: predict_move_count(eval_op, input.context.model_bbox, input.tool),
};
```

**What it does with a `0.0`:** nothing. `CutterOpProfile::predictions` has no
reader. A repository-wide search for `.predictions` returns zero hits outside
`profile.rs` itself — no production surface, no test. The field is written on
every profile build and never read.

Two consumers reach `CutterOpProfile` (`cli/src/project.rs:758` and
`viz/src/app/mcp/generation.rs:38`); both read `feasibility`,
`suggested_operation`, `feeds` and `warnings` only.

This is a separate finding from T-4 and it is not T-4's to fix. It does affect
the fix's cost: one of the four call sites is dead weight.

### 1d. Test call sites

Ten, all in-crate: `predict.rs:802`, `:830`, `:866`, `:867`, `:888`, `:905`,
`:976`, and `suggest/tests.rs:1347`, `:1359`, `:2026`. No integration test
under `crates/rs_cam_core/tests/` calls the function.

---

## 2. The harm, ranked

### The exits

The module header at `predict.rs:39-49` lists seven refusal cases. The code
has nine exits, and two of them are not in the header.

| # | Exit | Line | Reached by |
|---|---|---|---|
| 1 | Drill family | `predict.rs:207` | early guard |
| 2 | V-bit cutter | `predict.rs:219` | early guard |
| 3 | No `kc_n_per_mm2()` | `predict.rs:236` | early guard |
| 4 | Non-positive diameter | `predict.rs:250` | early guard |
| 5 | No depth per pass | `predict.rs:257` | early guard |
| 6 | Non-positive radial WOC | `predict.rs:289` | early guard |
| 7 | Zero chipload | `predict.rs:317` | early guard |
| 8 | Non-positive stickout | `predict.rs:339` | early guard |
| 9 | Delegate returns `None` | `predict.rs:396` | `.map_or(0.0, …)` |

Exit 9 is the undocumented one, and it hides two distinct causes inside
`tip_deflection_from_engagement` (`predict.rs:483-487`):

```rust
if axial_mm <= 0.0 || immersion_rad <= 0.0 || fz_mm <= 0.0 || tool.stickout <= 0.0 {
    return None;
}
if matches!(material, Material::Custom { .. }) {
    return None;
}
```

**`Material::Custom` refuses unconditionally there.** But
`Material::kc_n_per_mm2()` at `material/mod.rs:1096-1102` returns `Some(kc)`
for a `Custom` material carrying a positive `kc`. So a custom material with a
valid coefficient passes guard 3 at `predict.rs:236`, runs the whole force
calculation, and then abstains silently at `predict.rs:396`. The header's
claim — "Material has no primary-source `kc_n_per_mm2()`" — under-states the
refusal set.

**The stickout fallback does not reach the delegate.** `predict.rs:333-337`
computes a `stickout_mm` fallback of `cutting_length + 5 mm` when
`tool.stickout` is non-positive. That fallback lands in the breakdown only.
The delegate reads `tool_def.stickout`, which `build_cutter`
(`compute/cutter.rs:47`) copies straight from `tool.stickout`. So a zero
stickout still refuses, through exit 9 rather than exit 8, and the breakdown
reports a stickout the model never used.

### 2a. V-bit — **actively unsafe, and the guard is stale**

The brief's reading is correct on the physics and understates the defect. Two
findings.

**The tool is the one most likely to bend.** A V-bit's bending section is
`2 · engagement_radius(depth)` (`tool/vbit.rs:133-135`), which goes to zero at
the tip. Stiffness goes as `d⁴`, so the section near the tip is vanishingly
compliant. This is the tool the guard is silent about.

**The stated reason no longer holds.** The guard says:

```rust
// V-bit closed-form is undefined (engaged-D grows linearly with DOC,
// and the "core" of a triangular profile isn't a bending section).
// The post-sim integrator with `lookup_diameter_at` handles V-bits;
// the closed-form predictor refuses.
```
`crates/rs_cam_core/src/feeds/predict.rs:216-218`

That was true of the bespoke single-section formula. It is not true of the
code that runs today. The module header records the change at
`predict.rs:21-33`: the magnitude is now delegated to
`ToolDefinition::tip_deflection_mm`, the same two-section numeric integration
the post-simulation gate uses. That integrator reads the local diameter per
step at `tool/mod.rs:697-700`:

```rust
let d = self
    .cutter
    .lookup_diameter_at(axial_from_tip)
    .max(D_FLOOR_MM);
```

`VBitEndmill` implements `lookup_diameter_at` (`tool/vbit.rs:133`). So the
delegate models a V-bit correctly. The guard at `predict.rs:219` refuses
before reaching it.

The header even records the leftover, at `predict.rs:37-38`: *"V-bit geometry
still returns zero (the post-sim integrator handles those)."* The only V-bit
dependency left in the file is `bending_diameter_mm` returning `0.0`
(`predict.rs:465`), and that value feeds `i_eff_mm4` — a breakdown
diagnostic, explicitly not the magnitude (`predict.rs:341-352`).

**Nothing in the live code path needs this guard.** It is removable, and its
removal is a larger safety win than the `Result` change.

The post-simulation gate does evaluate V-bits — `tool_load/deflection.rs` has
no V-bit refusal arm. So the operator is told eventually, but only after
running a simulation, and only after Suggest already wrote the DPP.

### 2b. Material with no `Kc` — **actively unsafe**

The shipped catalogue (`material/mod.rs:1354`) offers ten plastics. Nine of
them return `None` from `kc_n_per_mm2()` — Polycarbonate, Acrylic, Delrin,
UHMW-PE, Polypropylene, Nylon 6/6, ABS, PETG and Rigid PVC
(`material/mod.rs:1046-1057`). Only HDPE carries a measurement. Fiberglass
also returns `None` (`material/mod.rs:1095`). So do out-of-band
`SolidWoodByJanka` (`material/mod.rs:989-991`) and every `Custom` material
(see exit 9 above).

A user who picks "Acrylic" from the stock picker and runs Suggest on a pocket
gets no deflection back-off and no message. This is the highest-frequency
unsafe case, because the picker names the materials and the user has no way to
know which nine of the thirteen plastics-and-composites entries carry a model.

The post-simulation gate refuses the same input with
`UnmodeledReason::MaterialUnvalidated` and says so. Pre-simulation says
nothing. The two halves of the product disagree about whether an abstention is
worth reporting.

### 2c. Drill — **uninformative, and the refusal is correct**

Drill is `PassRole::Roughing` (`catalog/registry.rs:750`), so a drill
operation does enter `backoff_dpp_for_deflection`. The physics genuinely does
not apply: Z-only kinematics, no continuous radial engagement. The post-sim
gate makes the same call, with `UnmodeledReason::NotApplicableForOp`
(`tool_load/deflection.rs:142-144`).

The harm is that the reader cannot tell this correct abstention from case 2b.
Both read `0.0`. A drill needs no action; an acrylic pocket needs one.

### 2d. The input-shaped exits — **uninformative**

Zero diameter (4), no DPP (5), zero radial WOC (6), zero chipload (7) and zero
stickout (8) describe an unconfigured or malformed operation, not a physical
finding. Each is correct to refuse.

Two of them carry a smaller secondary defect. Exit 7 returns a partially-filled
breakdown (`predict.rs:324-330`) while exits 4, 6 and 8 return `breakdown_zero`
— so a reader of the breakdown cannot tell "not computed" from "computed as
zero" either. That is the same class one level down.

### 2e. The control case: a modelled zero is unreachable

`tip_deflection_mm` (`tool/mod.rs:648`) returns `0.0` only when
`stickout <= 0`, `E <= 0`, `force_n == 0.0`, or `load_pos <= 0.0` — the last
requiring an axial DOC of at least twice the stickout. `lateral_cutting_force`
(`force.rs:141-158`) returns a strictly positive force for positive inputs,
and every integration term is positive. So for any operating point that clears
all nine guards, `predicted_um > 0.0`.

**Every `0.0` this function returns today is an absence.** No consumer loses a
reading when the type changes. The value `0.0` has never meant "this tool does
not bend"; it has only ever meant "I did not model this", written in a
notation that cannot say so.

---

## 3. Reachability

| Case | Reachable through the shipped GUI | Evidence |
|---|---|---|
| **V-bit on a roughing op** | **Yes, two clicks** | see below |
| **Unvalidated material** | **Yes, one click** | `material/mod.rs:1354` catalogue; nine plastics plus fiberglass refuse |
| **Drill** | **Yes, daily** | `Drill` is `PassRole::Roughing` (`registry.rs:750`), so the loop runs on every drill op |
| Custom material with a valid `kc` | Not established from the source | no GUI writer for `Material::Custom` found; reachable through a hand-edited project file |
| No DPP | Not inside the back-off loop | the loop needs `Some(dpp) > 0.5 mm` (`invariants.rs:397-400`); it reaches `efficiency` and `profile` |
| Zero chipload | Yes, by setting feed to 0 | no validation found that blocks it |
| Zero stickout | Not established | `ToolConfig` defaults to `45.0` (`tool_config.rs:219`); no GUI validator found |
| Zero diameter | Not established | no GUI validator found; likely blocked upstream, unverified |

**The V-bit path, in detail.** Nothing restricts a tool type to an operation
type. `OperationSpec` (`compute/catalog.rs:68-78`) has no tool-compatibility
field. The GUI tool picker lists every tool without filtering
(`viz/src/ui/properties/tab_badges.rs:484-490`):

```rust
egui::ComboBox::from_id_salt("tp_tool")
    .selected_text(tool_label)
    .show_ui(ui, |ui| {
        for (id, name, _) in tools {
            ui.selectable_value(&mut entry.tool_id, *id, name.as_str());
        }
    });
```

So the user picks a Pocket, Profile, Face, Adaptive, Adaptive3d, Rest or
Zigzag operation — all `PassRole::Roughing` — assigns the V-bit, and runs
Suggest. The back-off loop runs, reads `0.0`, and does nothing.

`VCarve` is `PassRole::Finish` (`registry.rs`), so the natural V-bit operation
never enters the loop anyway. The reachable case is the V-bit used off its
usual operation — a chamfer-style pocket, a tapered profile — which is exactly
where the operator is least sure of the depth and most needs the guard.

---

## 4. The fix

### Does the precedent fit?

**`GatePopulation` — no.** It counts units a gate's own predicates removed:
`contributing`, `offered`, `unit` of samples or holes
(`tool_load/verdict.rs:648-658`). The predictor is one closed-form evaluation
at one operating point. There is no population, no offered count and no
filter. Writing `GatePopulation { contributing: 0, offered: 1 }` would tell
the operator "all evidence was filtered out" when the truth is "the model does
not apply to this tool". X-VAC is about an empty evidence set; T-4 is about an
inapplicable model. Take its discipline — one shared clause, one place that
words it — not its type.

**`CriterionStatus` — its vocabulary fits, its container does not.** The field
`unmodeled_reason: Option<&'a UnmodeledReason>` (`verdict.rs:587`) is the
right vocabulary, and `UnmodeledReason` (`verdict.rs:148`) already carries
`NotApplicableForOp`, `MaterialUnvalidated`, `CutterModeUnsupported(String)`
and `NotImplemented(String)`. But `CriterionStatus` also carries
`sample_range`, `population`, `confidence` and `exceeded` — four fields a
pre-simulation closed form cannot fill — and it borrows from a typed verdict
that does not exist before simulation. `feeds/profile.rs:13-17` also states
the boundary rule: the feeds layer must not shadow the post-simulation refusal
taxonomy with a competing one.

So: **reuse `UnmodeledReason` as the shared vocabulary, do not reuse
`CriterionStatus` as the container.**

**The closest precedent sits in the same folder.**
`feeds::force::DeflectionCapRefusal` (`force.rs:167-182`) is a typed pre-simulation
refusal enum, with `chipload_cap_for_deflection_with_reason` beside the
discarding `chipload_cap_for_deflection`. Its doc says the principle in one
line:

> A solver needs the distinction: a bound that **no** feed satisfies is a
> different finding from a set of inputs that carries no model at all.

Follow that shape. It is the third time this repository has drawn it.

### The change to `predict.rs`

```rust
/// Why the closed-form predictor produced no deflection figure.
///
/// Every variant maps onto the post-simulation refusal vocabulary in
/// [`crate::tool_load::verdict::UnmodeledReason`] through
/// [`Self::as_unmodeled_reason`], so the pre-simulation abstention and
/// the post-simulation abstention print the same words to the operator.
///
/// This is a pre-simulation type and it stays one: it carries no sample
/// range, no population and no confidence tier, because a closed form
/// evaluated at one operating point has none of those. See
/// `feeds/profile.rs` §Boundaries — the feeds layer must not shadow
/// `tool_load`'s taxonomy with a competing one, so it borrows the
/// vocabulary and keeps its own shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeflectionUnmodeled {
    /// Drill family — Z-only kinematics, no continuous radial engagement.
    NotApplicableForOp(OperationType),
    /// The material carries no primary-source `Kc`.
    MaterialUnvalidated,
    /// `Material::Custom`. The delegate refuses every custom material,
    /// including one carrying a positive `kc` — a case the early `Kc`
    /// guard lets through.
    MaterialCustom,
    /// The tool diameter is zero, negative or not a number.
    NoDiameter,
    /// The operation carries no depth per pass.
    NoDepthPerPass,
    /// The resolved radial width of cut is not positive.
    NoRadialEngagement,
    /// Feed, RPM or flute count resolved to a zero chipload.
    NoChipload,
    /// The tool reports no usable stickout.
    NoStickout,
    /// The engagement is deeper than the cantilever, so the load point
    /// sits at or below the tool tip.
    DegenerateCantilever,
}

impl DeflectionUnmodeled {
    /// The post-simulation refusal this pre-simulation abstention
    /// corresponds to. The operator reads one vocabulary, not two.
    pub fn as_unmodeled_reason(&self) -> UnmodeledReason {
        match self {
            Self::NotApplicableForOp(op) => UnmodeledReason::NotApplicableForOp(format!(
                "{} — no continuous radial engagement",
                op.spec().label
            )),
            Self::MaterialUnvalidated | Self::MaterialCustom => {
                UnmodeledReason::MaterialUnvalidated
            }
            Self::NoDiameter
            | Self::NoDepthPerPass
            | Self::NoRadialEngagement
            | Self::NoChipload
            | Self::NoStickout
            | Self::DegenerateCantilever => {
                UnmodeledReason::NotImplemented(self.clause().to_owned())
            }
        }
    }

    /// One operator-facing clause, identical on every surface, so the
    /// wording cannot drift between the GUI, the CLI and the MCP bridge.
    /// The same discipline `GatePopulation::vacuity_clause` applies to
    /// the X-VAC marker.
    pub fn clause(&self) -> &'static str {
        match self {
            Self::NotApplicableForOp(_) => "the deflection model does not apply to this operation",
            Self::MaterialUnvalidated => "this material has no measured cutting coefficient",
            Self::MaterialCustom => "a custom material carries no validated force model",
            Self::NoDiameter => "the tool has no usable diameter",
            Self::NoDepthPerPass => "the operation has no depth per pass",
            Self::NoRadialEngagement => "the operation has no radial width of cut",
            Self::NoChipload => "the feed, the speed or the flute count resolves to a zero chip",
            Self::NoStickout => "the tool reports no usable stickout",
            Self::DegenerateCantilever => "the cut is deeper than the tool stands out",
        }
    }
}

/// Closed-form deflection prediction. `predicted_um` is now ALWAYS a
/// modelled figure: the refusal cases return `Err` and name themselves.
///
/// Before 2026-09-17 this type carried the refusal as `predicted_um ==
/// 0.0`. A rigid cut and an unmodelled cut read the same, and the
/// Suggest back-off treated the second as the first. See T-4.
#[derive(Debug, Clone)]
pub struct DeflectionPrediction {
    pub predicted_um: f64,
    pub breakdown: DeflectionBreakdown,
}

pub fn predict_peak_deflection_um(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> Result<DeflectionPrediction, DeflectionUnmodeled> { ... }
```

`Result` rather than a nullable field: two `Option`s that must agree is the
shape that lets the next caller read the wrong one, which is the defect one
level up. `Result` also makes every existing `.predicted_um` fail to compile,
which is the point.

**Delete the V-bit guard at `predict.rs:219-227`** in the same change, and
delete the `CutterKind::VBit => 0.0` arm's dependency in `bending_diameter_mm`
(`predict.rs:465`) or leave it with a corrected comment: it feeds a
diagnostic, not the magnitude. Section 2a gives the justification. Keep the
drill guard — that refusal is real.

**Move the two hidden exits out of `.map_or`.** Guard `Material::Custom` and a
non-positive `tool_def.stickout` explicitly before the call, so exit 9 reduces
to `DegenerateCantilever` and the header stops under-stating the refusal set.

### What each consumer must do

**`invariants.rs:402/414` — the loop.** Add a warning so the abstention is
stated:

```rust
/// The closed-form deflection predictor abstained, so the DPP back-off
/// did not run. Warning-only: Suggest writes the DPP the rigidity clamp
/// produced. The operator needs the distinction, because the same
/// silence used to mean "the tool does not bend at this depth".
DeflectionBackoffUnmodeled {
    dpp_mm: f64,
    reason: crate::feeds::predict::DeflectionUnmodeled,
},
```

```rust
let initial_prediction = match crate::feeds::predict::predict_peak_deflection_um(
    operation, tool, material, machine,
) {
    Ok(p) => p,
    Err(reason) => {
        warnings.push(SuggestWarning::DeflectionBackoffUnmodeled {
            dpp_mm: current,
            reason,
        });
        return warnings;
    }
};
```

Handle the in-loop call at `:414` the same way: an `Err` mid-loop stops the
iteration and pushes the warning beside whatever `DppCappedByDeflection` the
completed steps earned.

This touches three more files:
- `suggest.rs:242` — the new variant.
- `rationale.rs:272` — a new match arm; the match is exhaustive.
- `rationale.rs:88` — a new `RationaleReason::DeflectionUnmodeled`.

`session/compute.rs:975` uses `_ => false` and needs no change; the new
warning is not deflection-binding, because nothing bound.

**`efficiency.rs:368` — `force_headroom`.** The minimal change is
`.ok()?.predicted_um`. Recommend the wider one: carry the reason, so the four
GUI sites stop printing an unexplained "not modelled".

```rust
/// `1 − predicted_δ / EXCEEDS_BOUND`, or the reason the predictor
/// abstained. A negative headroom is a real finding, not an error.
pub force_headroom: Result<f64, DeflectionUnmodeled>,
```

**`compare.rs`** — four read sites at `:533`, `:546`, `:553` and `:629`. Each
already has a "not modelled" arm; each gains `reason.clause()`. That is the
operator-facing payoff of T-4 and it is four one-line edits.

**`profile.rs:206`** — the field type becomes the `Result`. The field has no
reader, so nothing downstream moves. Whether to delete `Predictions.deflection`
outright is a separate decision and is not T-4's to make.

### Call-site count

| Scope | Sites |
|---|---|
| Production calls of `predict_peak_deflection_um` | **4** (`profile.rs:206`, `efficiency.rs:368`, `invariants.rs:402`, `invariants.rs:414`) |
| In-crate test calls | **10** (`predict.rs` ×7, `suggest/tests.rs` ×3) |
| Integration-test calls | **0** |
| **Signature change, total** | **14** |
| Re-export to widen | 1 (`feeds/mod.rs:40`) |
| New `SuggestWarning` variant | 3 (`suggest.rs`, `rationale.rs` ×2) |
| `force_headroom` type change | 5 (`efficiency.rs` field, `compare.rs` ×4) |

The signature change is small. The V-bit guard deletion is one block. The
warning and the GUI reason are the work.

### One sentry the change must not surprise

`crates/rs_cam_core/tests/wanaka_suggest_integration.rs:223-229` asserts
`DppCappedByDeflection` must not fire, with the message "(drill / V-bit)". Its
two call sites (`:671`, `:687`) are both **drill** toolpaths. No V-bit case
calls it. So removing the V-bit guard does not flip this sentry. Its message
should lose the "/ V-bit" clause, which is now wrong. This test needs the
Wanaka project and runs for minutes — do not run it casually.

---

## 5. The sentry

**File:** `crates/rs_cam_core/tests/a_refused_deflection_is_not_a_zero_t4.rs`

The name follows the folder convention in `crates/rs_cam_core/tests/CLAUDE.md`
— `<claim>_<programme code>.rs`, the claim in words a searcher would use, the
code naming the plan that pre-registered it (`planning/TECH_DEBT_REGISTER.md`
row T-4).

### The non-vacuity anchor

**A control arm that produces a real number, asserted before any refusal
assertion.** This is the anchor and it is the load-bearing part of the design.

Every other arm asserts `Err(_)`. A predictor that refuses every input passes
all of them. So the file opens with `a_rigid_cut_still_produces_a_figure`: a
6 mm four-flute carbide end mill, 45 mm stickout, hard maple, a Pocket
operation at a configured DPP, feed and RPM — and asserts
`Ok(p)` with `p.predicted_um > 0.0` and finite. Without that arm the file is
vacuous in exactly the way X-VAC names.

A second anchor covers the source-scanning arm (7): it asserts the needle is
present before it asserts the property, and it must not match a comment line.

### The arms

1. **`a_rigid_cut_still_produces_a_figure`** — the anchor. `Ok`, positive,
   finite.

2. **`every_refusal_names_itself`** — one fixture per refusal case, each
   asserting the exact `DeflectionUnmodeled` variant. The assertion runs
   through a `match` on the variant with **no wildcard arm**, so a tenth
   refusal case fails to compile rather than passing silently. Cases: drill
   op, unvalidated material (Acrylic), custom material with a positive `kc`,
   zero diameter, no DPP, zero WOC, zero feed, zero stickout, DOC past twice
   the stickout.

3. **`a_v_bit_is_modelled_now`** — the same operating point on a V-bit and on
   a flat end mill of equal diameter and stickout. Both return `Ok`. The
   V-bit's figure is the **larger** of the two, because its bending section
   narrows toward the tip. This arm pins the section-2a finding and would go
   red today.

4. **`no_input_yields_a_modelled_zero`** — sweep a small grid of valid
   operating points and assert that no `Ok(p)` carries
   `p.predicted_um == 0.0`. This is what makes the `Result` meaningful: the
   `Ok` branch never carries the old sentinel.

5. **`the_backoff_states_its_abstention`** — run `enforce_invariants` on a
   V-bit-free refusal case that reaches the loop (an Acrylic Pocket at a
   roughing DPP above the 0.5 mm floor) and assert the warning list contains
   `DeflectionBackoffUnmodeled`. Assert as the counterpart that the same
   operation in hard maple produces either a back-off or no deflection
   warning at all — never the abstention. This is the behavioural bar.

6. **`the_two_vocabularies_agree`** — for every `DeflectionUnmodeled`, call
   `as_unmodeled_reason()` and assert it lands on the `UnmodeledReason`
   `tool_load::deflection::evaluate` would produce for the same input class.
   Exhaustive match, no wildcard. This stops the pre-simulation and
   post-simulation halves drifting into two vocabularies for one idea.

7. **`no_caller_compares_the_figure_against_zero`** — a source scan over
   `crates/rs_cam_core/src/feeds/` and `crates/rs_cam_viz/src/ui/feeds/`.
   Anchor first: assert `predict_peak_deflection_um` appears at least four
   times outside comment lines. Then assert no non-comment line pairs
   `predicted_um` with a `0.0` comparison. Without the anchor a rename makes
   this pass on an empty scan.

### Inject-and-confirm

The folder file requires it: restore the `0.0` return on one refusal path and
confirm arms 2, 4 and 5 go red before trusting the file. Arm 3 goes red today,
before any fix, which is its own confirmation.
