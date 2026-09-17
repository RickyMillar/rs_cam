# T-9 — a feed clamped onto a ceiling ships one rounding step above it

**Read at** `af7ce5c1ca81193a8ee0d69009ca0971e59ef3a2` (`master`).
Investigation only. No cargo command ran. No `.rs` file changed.

## Summary

1. The defect is REAL and it is exactly where the register says:
   `feeds/suggest/apply.rs:79` rounds the feed to the NEAREST whole mm/min
   after every clamp in `calculate` has already bound it.
2. It is SMALL. The rounding step is 1.0 mm/min, so the worst possible
   overshoot is **+0.5 mm/min**. The measured case is +0.3007 mm/min.
3. The ceiling it breaches is the spindle POWER gate, and after that the
   machine CUTTING-feed ceiling. Both are advisory. No site breaches a
   kinematic maximum.
4. Four rounding sites exist. Two matter (`apply.rs:79`, `pills.rs:191`).
   Two are latent because the shipped posts set no post-level feed limit.
5. The `next_rpm_at_or_below` pattern applies directly. Add a round-DOWN
   twin of `round_suggestion_value` and call it for the feed and the
   plunge. The whole feeds matrix then moves down by up to 1 mm/min.

---

## 1. The trace

The feed leaves the calculator through one function and reaches the
machine through two paths. The clamps sit in the calculator. Every
rounding sits downstream of them.

### 1.1 Where the ceilings bind

`crates/rs_cam_core/src/feeds/mod.rs::calculate` applies the ceilings in
this order:

| Step | Line | Ceiling | What it protects |
|---|---|---|---|
| 6 | `feeds/mod.rs` power gate | `gate_available_power` | the spindle: stall and burn |
| 7 | `feeds/mod.rs:2225-2236` | `machine.cutting_feed_ceiling_mm_min()` | cut quality and rigidity |
| 9 | `feeds/mod.rs:2245` | `feed *= machine.safety_factor` | a derate, not a ceiling |
| 9b | `feeds/mod.rs:2349-2351` | rubbing floor, capped | the workpiece: burn |
| 9c | `feeds/mod.rs:2381-2385` | drill plunge envelope | the drill |

`calculate` returns `FeedsResult.feed_rate_mm_min` at
`feeds/mod.rs:2470`. **Every clamp above is satisfied at that value.**

### 1.2 Where the rounding happens

`crates/rs_cam_core/src/feeds/suggest.rs:871-877`:

```rust
pub fn round_suggestion_value(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}
```

`f64::round` rounds half AWAY from zero. It therefore goes up about half
the time. The doc comment on the function says the caller uses it
"before clamping". For the feed that statement is wrong: the clamp
already happened inside `calculate`.

---

## 2. The sites

### Site 1 — `crates/rs_cam_core/src/feeds/suggest/apply.rs:79` (the defect)

```rust
scratch.set_feed_rate(round_suggestion_value(result.feed_rate_mm_min, 1.0));
```

Line 80 does the same to the plunge rate.

- **Rounding step:** 1.0 mm/min, to the nearest.
- **Worst-case overshoot:** +0.5 mm/min. A feed of `x.5` rounds to
  `x + 1`.
- **Worst case as a percentage:** `0.5 / feed`. On the measured 323.7
  mm/min case that is 0.154 %. On a 3000 mm/min feed it is 0.0167 %.
- **Measured case** (`TECH_DEBT_REGISTER.md` T-9, Shapeoko 1.5 kW VFD /
  Ipe / Ø12 slot): calculator 323.6993 → shipped 324.0000. The overshoot
  is 0.3007 mm/min, which is +0.0929 % of the feed.
- **Ceiling breached:** the spindle power gate (Step 6). The shipped
  point sits at 100.013 % of it.
- **Verdict:** a defect, but a small one. It is a breach of an advisory
  physical limit, not of a kinematic maximum.

**Why the power figure is smaller than the feed figure.** The power model
is affine in the feed, not proportional:
`tool_load/power.rs:216-218` gives
`kw_at_feed = shear_kw_per_mm_min · feed + edge_kw`.
The `edge_kw` term does not move with the feed. A +0.0929 % feed change
therefore raises the power by `0.0929 % × shear_share`. The register
records +0.013 %, so the shear share at that point is about 14 %. The
edge term carries the other 86 %. That agrees with T-10's own
measurement, which reports 93 % of the gantry push at zero chipload.
The two register numbers are consistent.

**Nothing re-checks the limit.** `apply_feeds_subset` runs
`enforce_invariants` on the scratch clone after the rounding, but that
pass resolves the feed ↔ chipload ↔ DPP coupling. It does not re-apply
the Step 6 power gate or the Step 7 machine ceiling.

### Site 2 — `crates/rs_cam_viz/src/ui/properties/pills.rs:191`

```rust
pub fn feed_rate(&self) -> PillSuggestion<'_> {
    self.suggestion(FeedsField::FeedRate, self.result.feed_rate_mm_min, 1.0)
}
```

`suggestion_for` (`pills.rs:41-64`) takes one of two branches:

- With a funnel preview it uses `p.value`. That value already passed
  through Site 1, so the pill inherits Site 1's overshoot and adds none.
- Without a preview it calls `round_suggestion_value(calculator, 1.0)` on
  the raw calculator feed. That is a second, independent instance of the
  same defect.

- **Rounding step:** 1.0 mm/min, to the nearest.
- **Worst-case overshoot:** +0.5 mm/min.
- **Ceiling breached:** the same two as Site 1.
- **Verdict:** the same defect in the GUI fallback path. Fix it with the
  same change.

### Site 3 — `crates/rs_cam_core/src/gcode/emitter.rs:244, 355, 361`

The emitter clamps and then formats:

```rust
let feed = clamp_feed(output, post, feed);          // emitter.rs:241, 350
"G1 X{x:.xyz$} Y{y:.xyz$} Z{z:.xyz$} F{feed:.feed_dp$}"
```

`clamp_feed` (`emitter.rs:68-80`) caps the feed at
`post.limits.max_feed`. `feed_dp` is `post.decimals.feed`, which is `0`
in all three shipped posts (`gcode/post.rs:443`, `:467`, `:586`). The
format is therefore a display rounding to a whole mm/min, applied AFTER
the clamp. A display rounding that ships is a shipped value.

- **Rounding step:** 1.0 mm/min, to the nearest.
- **Worst-case overshoot:** +0.5 mm/min above `post.limits.max_feed`.
- **Reachability today: none.** `gcode/post.rs:62-72` states that the
  shipped TOMLs leave `max_rpm` and `max_feed` unset. `clamp_feed`
  never fires on a shipped post. A custom post with a fractional
  `max_feed` reaches it.
- **Ceiling breached:** the post-level feed limit, which is a user-set
  advisory. The machine's own ceiling is enforced upstream and is an
  integer on every preset, so `{:.0}` returns it unchanged.
- **Verdict:** a latent defect. Real in shape, unreachable in the
  shipped configuration.

### Site 4 — `crates/rs_cam_core/src/gcode/mod.rs:936-944`

`replace_rapids_with_feed` clamps the inserted high feedrate to
`post.limits.max_feed` and then writes `F{feed:.1}`.

- **Rounding step:** 0.1 mm/min, to the nearest.
- **Worst-case overshoot:** +0.05 mm/min.
- **Ceiling breached:** the post-level feed limit, on a RAPID move.
- **Verdict:** a curiosity. The magnitude is below the resolution of any
  controller this project targets.

### Sites that are CLEAN

These paths clamp a feed and do NOT round afterwards. I checked each one.

- `crates/rs_cam_core/src/dressup/feed_modulation.rs:659-661` — the
  per-move modulation takes
  `max_feed.min(band_ceiling).min(predicted_cap.max(band_floor))` and
  writes the f64 straight onto the move. No rounding.
- `crates/rs_cam_core/src/dressup/feedopt.rs:176-217` — the pass applies
  the floor first and the ceiling LAST, by design, and the file says so.
  No rounding.
- `crates/rs_cam_core/src/tool_load/optimize/patches.rs:74-77` — the
  `FeedRate` patch writes `patch.value` unrounded. The `SpindleRpm`
  patch at `:83` does call `.round()`, but that is the RPM axis, not the
  feed.
- `crates/rs_cam_core/src/gcode/program_builder.rs:84-121` — the IR
  carries the feed as an f64 and elides an unchanged F word. No
  quantisation.

**Staleness note.** Another agent is editing `src/dressup/` and
`src/finish/` in this shared tree. At the commit above, `git status`
shows `dressup/CLAUDE.md`, `arcfit.rs`, `condition.rs`, `mod.rs`,
`tests.rs` and `tsp.rs` as modified. The two dressup files I relied on,
`feed_modulation.rs` and `feedopt.rs`, are NOT modified. `src/feeds/`,
`src/gcode/`, `src/machine/` and `src/tool_load/` are clean. The dressup
findings are therefore current, but confirm them if that agent's work
lands in those two files.

---

## 3. Which ceiling, and the rank

| Rank | Ceiling | What it protects | Breached by | Size |
|---|---|---|---|---|
| 1 | Spindle power gate (Step 6) | the spindle: stall, burn | Site 1, Site 2 | +0.013 % of power, measured |
| 2 | `cutting_feed_ceiling_mm_min` (Step 7) | cut quality, rigidity | Site 1, Site 2 | ≤ 0.5 mm/min |
| 3 | Chipload band ceiling | the workpiece and the edge | Site 3 only | ≤ 0.5 mm/min, unreachable |
| 4 | Deflection back-off (`predicted_cap_mm_min`) | tool deflection | Site 3 only | ≤ 0.5 mm/min, unreachable |
| 5 | `max_feed_mm_min` (`$110`/`$111`) | the kinematic maximum | nothing found | — |

**No site breaches a kinematic maximum.** `cutting_feed_ceiling_mm_min`
never exceeds `max_feed_mm_min` (`machine/mod.rs:136-140`), so a breach
of the cutting ceiling by half a mm/min stays well inside the gantry
limit. A GRBL controller clamps an over-limit F word itself in any case.

The ceilings the defect does breach are the two the engine owns and the
controller cannot rescue: the power gate and the cutting-feed ceiling.

---

## 4. An adjacent finding — NOT a rounding

While tracing the clamps I found a second breach of the same family. It
is a wrong-variable bug, not a rounding, so it is not T-9. I report it
because it is larger in principle and it sits in the same twenty lines.

Three sites cap a feed LIFT at the gantry TRAVEL rate while the ceiling
that bound the feed forty lines earlier was the CUTTING ceiling:

- `crates/rs_cam_core/src/feeds/mod.rs:2349-2351` (Step 9b, rubbing floor)
- `crates/rs_cam_core/src/feeds/mod.rs:2381-2385` (Step 9c, drill envelope)
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs:470-471`

All three read:

```rust
let machine_max_feed_after_safety = machine.max_feed_mm_min * machine.safety_factor;
```

Step 7 clamped the feed at `machine.cutting_feed_ceiling_mm_min()`, which
is `min(max_cutting_feed, 6000).min(max_feed_mm_min)`
(`machine/mod.rs:129-140`). The two differ whenever the travel rate
exceeds 6000 mm/min or the user sets an explicit cutting cap.

**Arithmetic on the wanaka fixture**
(`tests/fixtures/wanaka_2026-08-16_f530995a.toml:39-41`):
`max_feed_mm_min = 10000`, no cutting cap, `safety_factor = 0.75`.

- cutting ceiling = `min(6000, 10000)` = 6000 mm/min
- effective cutting ceiling after the safety factor = 4500 mm/min
- the lift cap actually used = `10000 × 0.75` = 7500 mm/min
- headroom the lift has above the ceiling = 7500 − 4500 = **3000 mm/min,
  or +66.7 %**

**Is it reachable? No, not today.** `effective_rubbing_floor`
(`feeds/mod.rs:974-979`) never exceeds `RUBBING_FLOOR_MM_TOOTH = 0.025`
(`feeds/mod.rs:930`). The lift target is `floor × rpm × flutes`. At the
highest shipped spindle speed and a four-flute tool that is
`0.025 × 24000 × 4 = 2400 mm/min`, which is below the 4500 mm/min
ceiling. The drill arm lifts to `env_lo_per_mm × d`; the largest shipped
`env_lo_per_mm` is 100 (Foam, `material/mod.rs:1293`), so a breach needs
a drill over 45 mm, against a 6.35 mm maximum shank.

**Verdict:** a latent wrong-variable bug with no reachable overshoot on
any shipped preset. It becomes live the moment the rubbing floor rises,
the spindle range widens, or a high flute count appears. The one-word
fix is to read `cutting_feed_ceiling_mm_min()` in all three places. It is
cheap and it removes a trap. It is not T-9.

---

## 5. There is also no guard

`crates/rs_cam_core/src/export/gcode_validator.rs:505-519` checks every
`F` word in the emitted text against `cfg.max_feed_mm_min` with
`EPS = 1e-6` (`:463`). A +0.5 mm/min overshoot would be caught.

That check is OFF in the shipped export.
`crates/rs_cam_viz/src/io/export.rs:21` passes `max_feed_mm_min: None`,
and the comment above it says the feed-cap check "stays off until the
machine profile is plumbed through". No other production site builds a
`MachineSafety` with a feed cap set.

So nothing catches this at the calculator, nothing catches it at the
apply path, and the one check that could catch it at the file is
disabled.

---

## 6. The fix

### 6.1 The pattern applies

`MachineProfile::next_rpm_at_or_below` (`machine/mod.rs:268-301`) is the
precedent. It exists because `clamp_rpm` (`machine/mod.rs:250-263`)
snaps to the NEAREST discrete speed and therefore rounds up about half
the time. The doc comment states the reason in one line: a caller that
reduces the RPM to shed load must not be handed a higher RPM.

The feed has the same shape. Add the round-DOWN twin.

### 6.2 The new helper

In `crates/rs_cam_core/src/feeds/suggest.rs`, beside
`round_suggestion_value`:

```rust
/// Round a suggestion value DOWN to a multiple of `step`.
///
/// [`round_suggestion_value`] snaps to the NEAREST multiple, which goes
/// UP about half the time. That is right for a value the calculator left
/// free. It is wrong for a value a ceiling already fixed: the shipped
/// number then sits one part-step ABOVE the limit the clamp exists to
/// enforce. The commanded feed is such a value — the Step 6 power gate
/// and the Step 7 machine ceiling both land the recommendation exactly
/// on a limit, and nothing re-checks a limit after the quantisation.
/// See T-9 in `planning/TECH_DEBT_REGISTER.md`.
///
/// Returns `value` unchanged when `step <= 0.0`.
#[must_use]
pub fn round_suggestion_value_down(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).floor() * step
}
```

### 6.3 Site 1

`crates/rs_cam_core/src/feeds/suggest/apply.rs:79-80`:

```rust
scratch.set_feed_rate(round_suggestion_value_down(result.feed_rate_mm_min, 1.0));
scratch.set_plunge_rate(round_suggestion_value_down(result.plunge_rate_mm_min, 1.0));
```

The import at `apply.rs:19` gains `round_suggestion_value_down`. The
geometry fields at `:86-87` keep `round_suggestion_value`: a stepover and
a depth-per-pass are not ceiling-bound in the same way, and
`realised_step_down` already snaps the depth to a reachable value.

### 6.4 Site 2

`crates/rs_cam_viz/src/ui/properties/pills.rs`. Give the private helper a
rounding function and pass the down twin for the feed only:

```rust
fn suggestion(
    &self,
    field: FeedsField,
    calculator: f64,
    step: f64,
    round: fn(f64, f64) -> f64,
) -> PillSuggestion<'_> { /* … pass `round` into `suggestion_for` … */ }

/// Feed-rate pill (drill ops edit their single feed on the Geometry tab).
///
/// The feed rounds DOWN: the calculator can land it exactly on a
/// ceiling, and the fallback branch writes the raw calculator value.
/// See T-9.
pub fn feed_rate(&self) -> PillSuggestion<'_> {
    self.suggestion(
        FeedsField::FeedRate,
        self.result.feed_rate_mm_min,
        1.0,
        round_suggestion_value_down,
    )
}
```

`stepover()` and `depth_per_pass()` pass `round_suggestion_value`.

### 6.5 Why NOT "clamp after the rounding" at Site 1

Re-checking the limits after the quantisation also works, and it is the
more exact fix. It is the wrong trade here for one reason: the apply path
receives only a `FeedsResult`. It does not receive the gate's available
power or the machine ceiling in the form the clamp needs, so the re-check
means threading two more inputs through `apply_feeds_subset` and
duplicating the Step 6 arithmetic outside `calculate`. The round-down
twin needs nothing new and cannot introduce a breach of its own, because
`floor(x) ≤ x ≤ ceiling` holds unconditionally.

### 6.6 Where "clamp after the rounding" IS right — Site 3

At the emitter the rounding is a display quantum that the emitter itself
owns, so the emitter can clamp against the value it will print:

```rust
fn clamp_feed(output: &mut String, post: &PostDefinition, requested: f64) -> f64 {
    let Some(max) = post.limits.max_feed else {
        return requested;
    };
    // Clamp against the number the F word will CARRY, not the number
    // handed in. The formatter rounds to `post.decimals.feed` places and
    // rounds half away from zero, so a feed clamped to `max` can print
    // above `max`. T-9.
    let quantum = 10f64.powi(-(i32::try_from(post.decimals.feed).unwrap_or(0)));
    let ceiling = (max.get() / quantum).floor() * quantum;
    if requested > ceiling {
        let line = post.render_comment(&format!(
            "WARNING: requested F{requested:.1} clamped to F{ceiling:.1} ({} max_feed)",
            post.name
        ));
        output.push_str(&line);
        return ceiling;
    }
    requested
}
```

This is optional. The shipped posts set no `max_feed`, so the change is
dead code until the wizard surfaces the limit. Do it when the wizard
does.

### 6.7 The cost, stated plainly

The change moves the recommended feed DOWN by up to 1 mm/min across the
whole feeds matrix. Two consequences follow:

1. Any blessed output that pins an exact feed needs a re-bless with a
   measured cause. That includes `tests/fixtures/perf_golden_*.json` and
   any `literature_matrix/` cell that asserts a feed to the whole
   mm/min.
2. The rounding also acts on a feed that the RUBBING FLOOR raised, and
   there it rounds AWAY from the floor. A feed sitting exactly on the
   floor goes one mm/min under it. On the measured case that is 0.3 % of
   a 0.025 mm/tooth floor, which is below the resolution of the chipload
   model. The power gate is a physical limit and the rubbing floor is an
   advisory band, so the trade is correct — but it is a trade, and the
   sentry below pins it.

---

## 7. The sentry

**File:** `crates/rs_cam_core/tests/a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown.rs`

The name follows the claim-plus-code form in
`crates/rs_cam_core/tests/CLAUDE.md`. The model is
`a_downward_traverse_rounds_down_g_rpmdown.rs`, which pins the RPM twin
of exactly this defect and carries the non-vacuity structure this claim
needs.

### Arm 1 — the claim

Sweep `MachineProfile::presets() × ALL_SPECIES`, the same population
`suggest_power_ceiling_after_pass9_g_suggest_powerstale.rs` uses. Run the
funnel for each pair. Assert:

```
shipped_feed <= result.feed_rate_mm_min + 1e-9
```

The invariant is one sentence: **the quantisation never raises a feed.**
It holds whatever ceiling bound the calculator, so the arm does not need
to know which one did.

### Arm 2 — the value that lands exactly between two steps

Build a `FeedsResult` whose `feed_rate_mm_min` is `323.5` — a feed
exactly half way between two whole mm/min. Assert the applied feed is
`323.0`, not `324.0`.

`f64::round` sends `323.5` to `324.0`, so this arm goes RED on the
current code. It is the arm that proves the fix.

Add a second point at the measured case, `323.6993`, and assert the
applied feed is `323.0`. That pins the register's own measurement.

### Arm 3 — the non-vacuity anchor

The anchor has two parts, because the claim can go vacuous in two ways.

**3a — the population reached a ceiling.** Assert
`checked > 0`, and assert that at least one swept pair was actually
ceiling-bound. Read it off the result: either a
`FeedsWarning::FeedRateClamped` or a `FeedsWarning::PowerLimited` is
present, or `result.feed_rate_mm_min` equals
`machine.cutting_feed_ceiling_mm_min()` to within `1e-9`. Without this,
Arm 1 passes on a sweep where no limit ever bound, and it proves nothing.

**3b — the nearest-rounding original DOES round up here.** Over the same
sweep, assert that `round_suggestion_value(f, 1.0) > f` for at least one
member. This is the direct copy of
`clamp_rpm_does_round_up_so_the_helper_is_not_redundant_g_rpmdown`
(`tests/a_downward_traverse_rounds_down_g_rpmdown.rs:89-125`). Without
it, `round_suggestion_value_down` could be an alias for the original on
this population, and Arm 1 would be an identity.

**This is the non-vacuity anchor the repository requires.** The claim is
behavioural, not source-scanning, so the "assert the needle is present,
and do not match comments" rule does not apply. If a source-scan arm is
added later — for example, "`apply.rs` does not call
`round_suggestion_value` on the feed" — that arm needs its own
needle-present assertion first.

### Arm 4 — the round-down does not create a rubbing recipe

Guard the direction the fix gives up. For every swept pair, assert:

```
shipped_feed >= effective_rubbing_floor(band) * rpm * flutes - 1.0
```

One whole mm/min of slack, and no more. A future change that pushes the
feed materially under the floor still fails this arm, while the one-step
round-down does not.

### Confirming the sentry

Inject the guarded defect once: revert `round_suggestion_value_down` to
`(value / step).round() * step` and confirm Arms 1 and 2 go RED. Do not
trust the file before that.

### Running it

`cargo test -p rs_cam_core -q --test a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown`

The sweep runs the funnel over three presets × the species list. It does
not touch the wanaka project, so it is a fast sentry, not a heavy one.
Name it in `crates/rs_cam_core/src/feeds/CLAUDE.md` as that folder's
sentry for the quantisation direction.

---

## 8. What changes in the register

T-9's "Fix" line already names both options. This report picks the first
one — round the feed DOWN — and gives the reason: the apply path cannot
re-check the limits without new inputs, and the floor direction is safe
unconditionally.

T-9's "Cost if left" line says "small in magnitude and unbounded in
principle". That is accurate. The bound is 0.5 mm/min per site and it
does not grow. What grows is the number of limits that land exactly on
the recommendation: R1 made the power gate one of them, and the next
binding limit will meet the same rounding.
