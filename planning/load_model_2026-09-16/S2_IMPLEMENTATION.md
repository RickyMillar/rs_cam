# S2 — power at the operating point that ships

Implemented 2026-09-18, against `SURFACE_IMPL.md` §1 step S2. The first form
was committed as `6d13b93a`. The orchestrator then ruled that `PowerFigure`
holds the model privately and answers through two accessors (§5.1), so this
document describes the form after that follow-up. The follow-up is not
committed.

S2 fixes a class-1 defect (`RESUME_PLAN.md` §9): a value computed at one
state, consumed at another. `feeds::calculate` publishes `power_kw` at the
calculator's geometry. `clamp_dpp_to_rigidity` then lowers the depth. The
published figure describes a depth that will not be cut.

---

## 1. The contract

New module `crates/rs_cam_core/src/feeds/operating_point.rs`. `feeds/mod.rs`
exports it:

```rust
pub mod operating_point;
pub use operating_point::{PowerFigure, PowerUnmodeled, power_at_operating_point};
```

```rust
pub fn power_at_operating_point(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    fallback: Option<CalculatorOperatingPoint>,
) -> Result<PowerFigure, PowerUnmodeled>;
```

`PowerFigure` carries `required_kw`, `available_kw` and the point the figure
was evaluated at (`ap_mm`, `ae_mm`, `rpm`, `feed_mm_min`). It holds the
`PowerTerms` PRIVATELY and answers through two methods:

```rust
impl PowerFigure {
    /// Predicted spindle power (kW) at `feed_mm_min`, at THIS point's
    /// geometry and speed, on the COMMANDED axis.
    pub fn required_kw_at_feed(&self, feed_mm_min: f64) -> f64;
    /// The feed (mm/min) at which this cut draws exactly `budget_kw`, in
    /// closed form. `None` when the edge term alone meets the budget.
    pub fn feed_for_kw(&self, budget_kw: f64) -> Option<f64>;
}
```

`power_at_operating_point` is the only constructor. The model type stays
inside `tool_load`, where it lives, and every caller solves through the same
two methods. Pass 10 needs only `feed_for_kw`; the Feeds card needs only
`required_kw` and `available_kw`.

### What it reads

- `ap` / `ae` — the operation's depth per pass and stepover, with `fallback`
  for an operation that exposes no such field.
- `rpm` — the operation's spindle speed, with `fallback`.
- the feed — the operation's feed rate. A feed has no fallback.
- `effective_d` — `feeds::effective_diameter` at the FINAL depth.
- ψ — `feeds::force::immersion_angle`.
- the shank — `tool.shank_diameter`, falling back to the cutting diameter.
- the ceiling — `machine.power_at_rpm(rpm) * machine.safety_factor`.

### The axis

COMMANDED. The operation's feed carries `safety_factor` from calculator
Step 9, and the ceiling carries it on the other side. A caller must not apply
the factor a second time.

### The refusals

`PowerUnmodeled` has seven variants, each with one `clause()`:

| Variant | Clause |
|---|---|
| `MaterialUnvalidated` | this material has no measured cutting coefficient |
| `NoFeed` | the operation has no feed rate |
| `NoRadialEngagement` | the operation has no radial width of cut |
| `NoDepthPerPass` | the operation has no depth per pass |
| `NoSpindleSpeed` | the operation has no spindle speed |
| `NoAvailablePower` | the machine publishes no spindle power at this speed |
| `NoEngagementDiameter` | the tool has no cutting diameter at this depth |

The brief named five. The last two are the two further guards pass 10 already
held: an unusable ceiling and an unusable effective diameter. Each was an
abstention in pass 10, so each must be an `Err` here. Without them the `Ok`
branch could not promise a finite, positive `available_kw`.

### What it does not do

It does not clamp, warn or mutate. This step adds no number.

### The doc change on the published pair

`FeedsResult::power_kw` and `FeedsResult::available_power_kw` keep their
fields and their values. Their docs now state that both sit at the
CALCULATOR's geometry, that `enforce_invariants` may lower the depth and the
stepover afterwards, and that a display of the shipped figure calls
`power_at_operating_point` on the final operation.

---

## 2. What moved out of pass 10, and the proof that nothing changed

`recheck_power_after_rescale` lost 74 lines and gained 21. The lines it lost
are the evaluation: the `Kc` read, the feed read, the `ap` / `ae` / RPM reads
with the calculator fallback, the ceiling, the effective diameter and the
`PowerTerms::of` call. They moved to `power_at_operating_point` unchanged —
the same reads, the same order, the same expressions.

Pass 10 keeps every decision it owned:

- the gate on pass 9 (`enforce_invariants` calls it only when
  `FeedRescaledToFinalGeometry` fired);
- the `required <= ceiling` early return;
- the `feed_for_kw` solve (now `figure.feed_for_kw(ceiling)`) and the
  `rescaled.min(cap)`;
- the fallback to the pre-rescale feed with `fits_at_any_feed: false`;
- the `PowerRecheckedAfterRescale` warning;
- the `clamp_plunge_to_feed` re-establishment;
- the `no_kc` debug trace, which now fires on
  `Err(PowerUnmodeled::MaterialUnvalidated)`.

Every other abstention maps to `Err(_) => return warnings`, which is what the
old early returns did.

### Proof

No test needed a re-bless. The two sentries that pin pass 10 are green,
unchanged:

- `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` — 5 passed.
- `suggest_power_ceiling_after_pass9_g_suggest_powerstale` — 5 passed.

A third arm proves the two doors are ONE evaluation rather than two that
agree today. On the T-15 fixture, `power_at_operating_point` called on the
shipped operation reproduces what `PowerRecheckedAfterRescale` reported, bit
for bit:

```
G-S2 parity | pass 10 ceiling 1.200000 kW, required at the rescaled feed
1.328055 kW; the door reproduces both exactly and reports 1.200000 kW at the
shipped 659.646 mm/min
```

Both comparisons use `f64::to_bits`, not a tolerance. The required figure
comes from `figure.required_kw_at_feed(rescaled_feed)`.

---

## 3. The measured ratio in sentry arm (a)

Sentry: `crates/rs_cam_core/tests/a_published_power_is_at_the_depth_that_cuts_g_s2.rs`.

Fixture: a Ø12 two-flute flat end mill, a roughing pocket in generic
hardwood, on the shipped `Generic Wood Router` preset. Its
`doc_roughing_factor` is 0.20, so the rigidity cap is 2.400 mm. The operator
asks for 36.000 mm (3 × D). Both `ap` and `ae` are pinned in the
`FeedsInput`, so the Step 6 ladder's geometry rungs stand down.

Measured:

```
G-S2 headline | calculator ap 36.0000 mm, feed 1069.329 mm/min, 8000.0 rpm -> 1.0080 kW
                shipped    ap  2.4000 mm, feed 2138.658 mm/min, 8000.0 rpm -> 0.0843 kW
                depth ratio 0.066667 x (shear share 0.2544 x feed ratio 2.000000
                + edge share 0.7456 x rpm ratio 1.000000) = 0.083624; measured 0.083624
```

The published figure is **11.96× the power at the depth that cuts**.

### The arithmetic

Power is affine in the feed: `P = shear·feed + edge`. On a flat end mill the
effective diameter is the nominal diameter at every depth, so between the two
points:

- the shear slope carries the cross-section `ap · ae`, and `ae` is held, so it
  scales with `ap`;
- the edge term carries `ap` and the cutting velocity `π·D·n`, so it scales
  with `ap` and the RPM;
- neither term carries the depth tier, which is what moves the feed.

```text
P_ship / P_calc = (ap_f / ap_c) · ( share_c · (f_f / f_c)
                                    + (1 − share_c) · (n_f / n_c) )
                = 0.066667 · ( 0.2544 · 2.000000 + 0.7456 · 1.000000 )
                = 0.066667 · 1.254365
                = 0.083624
```

The arm recovers the two terms at the shipped point through the door's own
public surface. Power is affine in the feed, so two evaluations determine the
whole line: `required_kw_at_feed(0.0)` IS the edge floor, and the slope is
the shear term. The probe feed is the calculator's, which keeps the two
magnitudes comparable so the subtraction stays well conditioned. The arm then
scales both terms back to the calculator's depth and RPM, and checks that
reconstruction against the published `power_kw` BEFORE it uses it — so the
arm is a cross-check of two doors rather than an identity on one. The file
restates none of the model's coefficients and reads no term directly.

The feed rises exactly 2.000000× because the depth tier moves from
`ap/D = 3` (multiplier 0.45) to `ap/D = 0.2` (multiplier 1.00), and pass 9
re-multiplies by the tier at the final depth. The machine cutting-feed
ceiling does not truncate it here.

### The tolerance

`MODEL_IDENTITY_TOLERANCE = 1.0e-9`, relative. Both sides are the same affine
model through different expression orderings — a few dozen double-precision
operations, so the expected disagreement is a small multiple of
`f64::EPSILON` (2.2e-16), about 1e-13 at worst. 1e-9 leaves four orders of
margin over float noise and stays eight orders BELOW the smallest difference
a model change could make here: one depth-tier step is a factor of 1.5. It is
a justified bound, not a slack.

### The other arms

- `the_rigidity_clamp_moves_the_depth_on_this_fixture` — non-vacuity: the
  calculator sizes its power at 36.000 mm, `RoughingDepthClampedToRigidity`
  fires, the shipped depth is the 2.400 mm cap, and the stepover is held.
- `every_refusal_names_itself` — a drill refuses `NoRadialEngagement`,
  acrylic refuses `MaterialUnvalidated`, a zero feed refuses `NoFeed`; all
  seven clauses are non-empty and pairwise distinct.
- `the_anchor_cut_is_modelled_not_refused` — non-vacuity for the refusals:
  the anchor draws 0.0843 kW of a 0.6000 kW ceiling (14.0 %), so
  `available_kw > required_kw > 0`.

### Red-first

The sentry was injected twice and went red both times.

1. The door evaluated at a depth 15× the one it reported
   (`mrr_cross_section_mm2(15.0 * ap_mm, ae_mm)` and
   `axial_doc_mm: 15.0 * ap_mm`): **3 of 5 arms failed** — the headline, the
   parity arm and the anchor. This injection was re-run after the §5.1
   follow-up, because the headline arm recovers the term split differently
   now. The same three arms failed.
2. The door used ψ × 0.90, a model divergence from Step 6 and pass 10: the
   headline arm failed on the reconstruction check, relative error 7.456e-2.

Injection 2 leaves the parity arm green, and that is correct: pass 10 now
routes through the same door, so both sides move together. The parity arm
proves the two doors are one evaluation; the headline arm is what catches a
divergence from `feeds::calculate`. The two arms are complementary.

---

## 4. Verification

Every command ran through `scripts/cargo_lane.sh`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | exit 0, no diff |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 23.64s` |
| `test -p rs_cam_core --lib -q` | `test result: ok. 2521 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 37.33s` |

The whole list below was re-run after the §5.1 follow-up. Every figure it
quotes is from that second run.

Integration targets, each `cargo test -p rs_cam_core -q --test <name>`:

| Target | Result |
|---|---|
| `a_published_power_is_at_the_depth_that_cuts_g_s2` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `suggest_power_ceiling_after_pass9_g_suggest_powerstale` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `suggest_feed_matches_final_geometry` | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.95s` |
| `a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `a_feed_lift_caps_at_the_cutting_ceiling_g_t18` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `arc_fit_disposition_a5` | `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.78s` |
| `literature_matrix` | `test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s` |
| `literature_parity` | `test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `power_ceiling_parity_f2` | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |

The brief asked for `rg -l "recheck_power_after_rescale|PowerTerms|power_kw"
crates/rs_cam_core/tests/`. It named four targets the list above did not:

| Target | Result |
|---|---|
| `the_power_ladder_pulls_the_right_lever_g_ladder` | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| `efficiency_abstains_without_kc_g_specenergy` | `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `cut_efficiency_is_closed_form_g_specenergy` | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `step_project_load` | `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |

`step_project_load` is a feature-gated target and reports no tests without
its feature. It matched the search only because a fixture `.toml` under
`tests/fixtures/` contains the string `power_kw`.

No test moved. No golden was re-blessed.

---

## 5. What the plan did not anticipate

### 5.1 `pub terms: PowerTerms` did not hold — the figure answers instead

Decision 2 fixed `PowerFigure { pub terms: PowerTerms, .. }`. `PowerTerms`
was `pub(crate)` (`tool_load/power.rs:145`), and so were `kw_at_feed` and
`feed_for_kw`. A `pub` field of a `pub(crate)` type trips rustc's
`private_interfaces`, which the core clippy run fails under `-D warnings`.

The first form widened those three items to `pub` and shipped as `6d13b93a`.
The orchestrator then ruled that decision 2 fixed the CONTRACT — one door,
the terms reusable without rebuilding — not the field. A `pub` `PowerTerms`
whose only constructor stays `pub(crate)` is a wider public surface than the
plan needs.

The follow-up therefore:

- reverts `tool_load/power.rs` to its committed `pub(crate)` state — three
  visibility tokens, no logic;
- makes `PowerFigure::terms` private;
- adds `required_kw_at_feed` and `feed_for_kw` as methods that delegate to it.

Pass 10 calls `figure.feed_for_kw(ceiling)`. The sentry calls
`required_kw_at_feed`. No caller reads a term apart.

The widening was part of `6d13b93a`, so the follow-up carries a real reverse
diff on `tool_load/power.rs` against HEAD. Verified: the file is now
byte-identical to `6d13b93a^`, its state before S2. S2 leaves `tool_load/`
untouched once both commits land.

### 5.2 Two stale doc lines this agent did not own

1. `crates/rs_cam_core/tests/a_rescaled_feed_stays_inside_the_power_ceiling_g_t15.rs:110`
   states "`tool_load::power::PowerTerms` is `pub(crate)`" as the reason the
   file restates the power model. The visibility statement is now wrong. The
   REASON still holds — a test that imports the expression it checks can only
   prove the expression equals itself — so the file's behaviour needs no
   change, only that clause.
2. `crates/rs_cam_core/src/feeds/CLAUDE.md` is exactly 40 lines, its stated
   ceiling, and its file map does not list `operating_point.rs`. The folder
   file also names the folder's sentries and does not name
   `a_published_power_is_at_the_depth_that_cuts_g_s2`.

Both are outside the brief's ownership list, so neither was edited.

### 5.3 The `--lib` run was blocked once, by a neighbour

`cargo test -p rs_cam_core --lib -q` failed to compile at the first attempt,
inside another session's untracked
`crates/rs_cam_core/src/session/generation_plan.rs` (`SetupId: Ord` not
satisfied, two sites). `cargo check -p rs_cam_core --lib` passed at the same
moment, so the break was in that file's test build only. The neighbour fixed
it, and the re-run is the 2521-passed line quoted above. Nothing in S2 was
involved.

### 5.4 The fixture the brief proposed needed one adjustment

The brief asked for a Ø12 roughing pocket at 3 D against a 0.20 D cap. That
is the `Generic Wood Router` preset, whose spindle is 0.8 kW. At 36 mm depth
the Step 6 power ladder would move the geometry, which would hide the
rigidity clamp. Both `ap` and `ae` are therefore pinned in the `FeedsInput`,
which stands the ladder's geometry rungs down — the same technique the T-15
sentry uses, and for the same reason.

---

## 6. Paths

Changed or added:

- `crates/rs_cam_core/src/feeds/operating_point.rs` (new)
- `crates/rs_cam_core/src/feeds/mod.rs` (`pub mod`, `pub use`, the doc on the
  published power pair)
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs` (pass 10 only)
- `crates/rs_cam_core/src/tool_load/power.rs` (reverts the three visibility
  tokens `6d13b93a` widened; byte-identical to `6d13b93a^` — see §5.1)
- `crates/rs_cam_core/tests/a_published_power_is_at_the_depth_that_cuts_g_s2.rs` (new)
- `planning/load_model_2026-09-16/S2_IMPLEMENTATION.md` (this file)
