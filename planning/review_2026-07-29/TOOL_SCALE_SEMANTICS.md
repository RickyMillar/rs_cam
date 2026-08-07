# Tool-scale and reach semantics — H1 research deliverable

Date: 2026-07-29
Basis: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` §H1, §2, §A.0;
`planning/review_2026-07-29/RADIUS_AUDIT.md`; HEAD `17861bb` (fixes `63d5e8b`,
`32c5e48`, `5732f57` already landed).
Status: **research only**. This document proposes no code change. It is the oracle
for every H2 routing decision and for human Checkpoint A.

Method: `rg -n '\.radius\(\)'` over `crates/*/src`, with each hit classified
production vs `#[cfg(test)]` by brace-matched module range; then
`cusp_radius` / `engagement_radius` / `height_at_radius` / `width_at_height`
for contrast; then per-site reading of the enclosing function.

## 0. Executive summary

| | count |
|---|---:|
| production `.radius()` call sites (all crates) | **76** |
| — of those, in `rs_cam_core/src` | 72 |
| — in `rs_cam_cli/src` | 2 |
| — in `rs_cam_viz/src` | 2 |
| test-only `.radius()` call sites | **112** (58 in `#[cfg(test)]` inside `src`, 54 in `tests/` + `benches/`) |
| production `cusp_radius()` call sites | 4 |
| production `engagement_radius(depth)` call sites | 7 |
| production sites requiring a **semantic change** (H2 scope) | **8** (7 named by the plan + 1 new) |
| production sites requiring **documentation only** | 5 |
| production sites that are **correct on the envelope and must not move** | 63 |

Three findings dominate:

1. **`height_at_radius(r)` already IS the plan's proposed `profile_height_mm(radius)`.**
   `MillingCutter::height_at_radius` (`crates/rs_cam_core/src/tool/mod.rs:206`) is the
   exact inverse of `width_at_height` / `engagement_radius`, and for a
   vertical-walled slot of half-width `w` it returns precisely the maximum descent
   below the rim. Every routing site whose input is a **measured half-width**
   (`RestCenterline::half_width_mm`) can be answered by it today with no new API.
   Adding a fourth name for it would be the `feature_radius` mistake in a new costume.
   See §3.1 and §6.
2. **The current routing yardstick is 6–14× too large, and `cusp_radius()` would be
   up to 2.1× too small in the other direction.** On the shipped wanaka tool
   (Ø1 tip, 7° half-angle, Ø6 shank) the true engaged half-width is 0.218 mm at
   0.05 mm depth, 0.504 mm at 0.5 mm, 0.688 mm at 2 mm, 1.056 mm at 5 mm — and
   only reaches the `radius()` value of 3.0 mm at **20.83 mm** of axial
   engagement. `radius()` overstates by 13.8×/6.0×/4.4×/2.8×; `cusp_radius()`
   (0.5) understates at shallow depth (conservative) but **overstates reach by
   27% at 2 mm and 2.1× at 5 mm** (dangerous). Neither is a safe global swap.
   See §4 and §5.
3. **A depth-only model is provably insufficient for valley fit.** For a
   symmetric V of half-angle φ from the bisector, the tapered ball's **cone**
   fouls the wall before the tip seats whenever `α > φ` — a failure mode with no
   dependence on depth at all. The reach answer is a two-input query
   (profile × valley geometry), which is why the census below distinguishes
   *cutter width at axial engagement* from *two-wall valley fit*. See §4.4.

**NEW FINDINGS** (sites the plan's list of seven missed) are in §7. The most
consequential is `pencil.rs:1080/1183/1307` sizing the rest **reference tool**
from `cutter.diameter()` — the shank — which silently disables the nominal
reference on every tapered tool.

---

## 1. Non-negotiables this document is written under

Restated from the plan §2 because every recommendation below is bounded by them:

- **Rule 4** — collision, bbox padding, spatial queries, swept volume and coverage
  margins keep the full envelope. §8 is the enforceable list.
- **Rule 5** — reach is profile-dependent; do not substitute `cusp_radius()` at
  routing/fit sites without proving the clearance model.
- **§A.0** — *area is not a safe invariant*; gates must be phrased on region
  count / topology / coverage / residual. Every "test oracle" column below is
  therefore written as a differential or a count, never as an area delta.
- H1's own gate: **no production behavior change in the semantic-API PR (PR-2).**

---

## 2. The six geometric questions

The plan names six classes. This document uses these short codes in every census row.

| code | question | canonical answer today | notes |
|---|---|---|---|
| **ENV** | maximum swept envelope — how far does any part of the cutter sweep? | `MillingCutter::radius()` (`tool/mod.rs:151`) = `diameter()/2` | For `TaperedBallEndmill` `diameter()` is deliberately the **shaft** (`tool/tapered_ball.rs:103-105`, "effective cutting diameter at widest point"). Conservative, correct, must not move. |
| **CUSP** | tip-sphere cusp scale — what feature size does the *cutting tip* resolve? | `MillingCutter::cusp_radius()` (`tool/mod.rs:181-186`) | Tip radius for `TaperedBall` via `geometry_hint()`, else `radius()`. Exact for scallop/cusp equations and feature-scale dials. |
| **WIDTH(d)** | cutter width at axial engagement `d` | `MillingCutter::engagement_radius(d)` (`tool/mod.rs:226-228`) → `width_at_height(d)` | Returns **0.0 at d = 0** for every ball-tipped shape (`tool/mod.rs:610`, `:619`) — callers must floor. Precedent floor: `.max(0.01)` at `compute/execute.rs:988`, `.max(1.0e-6)` at `feeds/cutter_constraints.rs:414`, `.max(0.0)` at `tool_load/power.rs:193`. |
| **CLEAR(r)** | vertical clearance available at a lateral radius `r` | `MillingCutter::height_at_radius(r)` (`tool/mod.rs:206`) → `Option<f64>`, `None` above the envelope | **Already the exact inverse of WIDTH.** For a vertical-walled slot of half-width `r`, this *is* the maximum descent below the rim. |
| **VALLEY** | two-wall/tool-profile fit in a valley | **no API today** | Needs profile × valley geometry (§4.4). Closed form exists for the symmetric-V/tapered-ball pair; general case is a monotone solve over `width_at_height`. |
| **HEURISTIC** | path scale only — no physical contract | `radius()` by convention | Legitimate, but the contract must be *written down* (see the narration decision, §5). |

### 2.1 `ToolDefinition` delegation status

`impl MillingCutter for ToolDefinition` (`tool/mod.rs:514-585`) explicitly delegates
`diameter`, `length`, `helix_deg`, `corner_radius_mm`, `chip_geometry`,
`height_at_radius`, `width_at_height`, `engagement_radius`, `lookup_diameter_at`,
`mrr_cross_section_mm2`, `flat_tip_diameter`, `center_height`, `normal_length`,
`xy_normal_length`, `profile_points`, `geometry_hint`, `edge_drop`, `facet_drop`,
`vertex_drop`.

It does **not** delegate `radius()` or `cusp_radius()`. Both currently work only
because the default bodies read `diameter()` and `geometry_hint()`, which *are*
delegated. **This is a latent trap**: the moment any shape overrides
`cusp_radius()` directly (rather than via `geometry_hint`), `ToolDefinition`
silently reverts to the trait default and the tapered fix evaporates at the only
layer that ships. The existing sentry
`tapered_cusp_radius_sentry.rs:53-62` catches this for the current shape set;
H1's acceptance gate "`ToolDefinition` delegation parity for every new accessor"
must be interpreted as **explicit delegation, not inherited default**.

---

## 3. Precedent review

Read before proposing anything, per H1.

### 3.1 `MillingCutter` (`crates/rs_cam_core/src/tool/mod.rs`)

| symbol | exact signature | semantics | verdict |
|---|---|---|---|
| `radius` | `fn radius(&self) -> f64` (`:151`, default `diameter()/2`) | ENV. Swept/collision radius. | Keep. Becomes the compat alias for `envelope_radius_mm()`. |
| `cusp_radius` | `fn cusp_radius(&self) -> f64` (`:181`, default matches `ToolGeometryHint::TaperedBall{tip_radius}` else `radius()`) | CUSP. Doc corrected by `63d5e8b` to explicitly disclaim reach. | Keep. Already answers every FEATURE-SCALE row. |
| `engagement_radius` | `fn engagement_radius(&self, depth_of_cut: f64) -> f64` (`:226`, default `width_at_height(depth_of_cut)`) | WIDTH(d). Doc at `:211-225` already says "Use this for stepover sizing, region detection, or any computation that asks *how wide is the cut at this engagement depth*". | Keep. **Already answers the derived-stepover row** (`unified_finish.rs:721`) and the offset-fit row's depth half. |
| `height_at_radius` | `fn height_at_radius(&self, r: f64) -> Option<f64>` (`:206`, required) | CLEAR(r). Z offset from tip to cutter surface at lateral radius `r`; `None` when `r` exceeds the profile. | **Already answers the plan's proposed `profile_height_mm(radius)` exactly.** Do not add a synonym. |
| `width_at_height` | `fn width_at_height(&self, h: f64) -> f64` (`:209`, required) | WIDTH(h), un-floored. | Keep. `engagement_radius` is its named alias. |
| `corner_radius_mm` | `fn corner_radius_mm(&self) -> f64` (`:191`, default `0.0`) | Bull/flat corner-blend radius. Overridden only by `flat.rs:43` and `bullnose.rs:65`. | Keep. `63d5e8b` fixed `pencil.rs:1364-1367` to fall through to `cusp_radius()` rather than `radius()`. |

Per-shape implementations for the profile pair:
`flat.rs:67-77` (constant `radius()`, height `0`), `ball.rs:67-80`,
`bullnose.rs:96-116`, `vbit.rs:113-158`, `tapered_ball.rs:147-199`.

### 3.2 `Adaptive3dParams` (`crates/rs_cam_core/src/adaptive3d/mod.rs:98-108`)

```rust
/// Engagement radius — the cutter's actual contact radius at `depth_per_pass`
/// below the tip. Used for stepover, region detection, and material clearing
/// modeling. For flat/ball cutters this equals the nominal radius; for
/// tapered cutters it's narrower than the shank radius.
pub tool_radius: f64,
/// Envelope radius — the widest extent of the cutter at any height (shank
/// radius for tapered tools). Used only for keep-out / bbox margins so the
/// tool's shank doesn't overrun the workpiece footprint.
pub envelope_radius: f64,
```

Populated at `compute/execute.rs:985-991`:

```rust
let engagement_radius = ctx.tool_def.engagement_radius(cfg.depth_per_pass).max(0.01);
let params = crate::adaptive3d::Adaptive3dParams {
    tool_radius: engagement_radius,
    envelope_radius: ctx.tool_def.radius(),
```

**This is the reference implementation of the whole H1 thesis** and it shipped a
year before the audit. Two scalars, two names, one adapter, one floor. Every
routing fix in H2 should look like this. Note the adapter still calls
`cutter.radius()` internally at `adaptive3d/path.rs:203` for the *grid* expansion
(ENV — correct) while planning on `params.tool_radius` (WIDTH — correct): the
split is honoured on both sides.

### 3.3 `ToolGeometryHint::engaged_diameter_at_doc` (`crates/rs_cam_core/src/feeds/mod.rs:123-128`)

```rust
pub fn engaged_diameter_at_doc(
    self,
    axial_doc_mm: f64,
    tool_diameter_mm: f64,
    shank_diameter_mm: f64,
) -> f64
```

The `TaperedBall` branch (`feeds/mod.rs:131-148`) is a hand-maintained twin of
`TaperedBallEndmill::lookup_diameter_at` (`tool/tapered_ball.rs:166-179`), which is
itself `2 × engagement_radius(d)` clamped to `diameter()`. Kept honest by
`feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`
(referenced at `feeds/mod.rs:118`). Relevant precedent for **why** duplication is
tolerated here — Suggest's call site holds only a `ToolGeometryHint`, not a
cutter — and for the discipline that makes it safe (a named parity sentry).
**Rule for H2: no new hand-maintained twin without its parity sentry in the same PR.**

### 3.4 `feeds/geometry.rs::tapered_ball_effective_diameter` (`:56-69`)

```rust
pub fn tapered_ball_effective_diameter(
    nominal_d: f64, tip_r: f64, taper_angle_deg: f64, axial_depth: f64,
) -> f64 {
    let local_radius = tip_r + ap * side_angle_rad.tan();
    (2.0 * local_radius).clamp(0.01, nominal_d)
}
```

**This is a third, cruder model** — a straight cone from the tip, with no
ball/cone tangency. It does not match `TaperedBallEndmill::width_at_height`
(`tool/tapered_ball.rs:181-199`), which uses the true tangent junction
(`h_contact = r_t(1 − sin α)`, `cone_offset = h_contact − r_contact/tan α`).
At the wanaka tool and 0.5 mm depth: `tapered_ball_effective_diameter` gives
`2(0.5 + 0.5·0.1228) = 1.061 mm`; `width_at_height(0.5)·2` gives `1.008 mm`.
5% apart, reached by two different formulas in the same crate.

Verdict: it is the **feeds-lane** model, used from `feeds/mod.rs:1730`; it is not
in the finishing lane and PR-2 must not touch it. But it belongs in the ADR as
evidence for why the trait should be the single source (§6) and it should be
flagged in the L1 doc sweep as a knowingly-divergent approximation.

### 3.5 `FinishPlannerParams::for_tool` (`crates/rs_cam_core/src/finish_planner.rs:163-174`)

```rust
pub fn for_tool(cusp_radius: f64) -> Self {
    Self { …
        pencil_claim_floor: cusp_radius * 0.25,
        close_radius_mm: cusp_radius * 0.5,
        min_region_area_mm2: (2.0 * cusp_radius).powi(2) * 4.0,
    }
}
```

The parameter is **named** `cusp_radius` and the doc at `:156-162` states the
contract. Landed by `5732f57`. Production call site
`compute/execute.rs:1341` passes `ctx.tool_def.cusp_radius()`. This is the
precedent for **naming the parameter after the semantic class rather than after
the tool**, and PR-2's caller-scalar deletions should follow it where the scalar
survives at all.

---

## 4. Diagrams

Reference tool throughout: `TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)` —
the tool `tapered_cusp_radius_sentry.rs:32-34` calls "the tool this project
actually finishes with". Ø1 tip (`r_t = 0.5`), taper half-angle `α = 7°`,
Ø6 shank (`R = 3.0`), 25 mm flute length.

Derived (per `tool/tapered_ball.rs:92-99`):
`h_contact = r_t(1 − sin α) = 0.4391 mm`,
`r_contact = r_t cos α = 0.4963 mm`,
`cone_offset = h_contact − r_contact/tan α = −3.6027 mm`.

### 4.1 ENV — maximum swept envelope (`radius()`)

```
            |<------------ R = 3.000 -------------->|
     ═══════╪═══════════════════════════════════════╪═══════   shank
            |                                       |
   h=20.83 -+---------------------------------------+   <- envelope first
            |  \                                 /  |      REACHED here
            |    \                             /    |
            |      \                         /      |
            |        \                     /        |
            |          \                 /          |
    h=0.44 -+            \  ball/cone  /            +
            |              \  tangent /             |
      h=0  -+----------------- \_._/ ----------------+   tool tip
            |<-0.496->|
                r_contact

  radius() = 3.000 mm  <-- what pencil/unified/generic-rest routing use TODAY
```

`radius()` is the *asymptotic* half-width. Nothing at finishing depth is 3 mm
wide. The value is correct and necessary for collision, bbox padding, spatial
queries and coverage margins — and only for those.

### 4.2 CUSP — tip sphere (`cusp_radius()`)

```
                 \                         /
                   \                     /
                     \                 /
     ................. \_ _ _ _ _ _ _/ .................
                        \    ___    /
                         \ /     \ /
             r_t = 0.5 ->  |   +   |   <- the sphere that forms the cusp
                            \_____/          between adjacent passes
                               ^
                          contact point

  cusp_radius() = 0.500 mm
  scallop stepover  s = 2·sqrt(2·r_t·h − h²)   <- exact, tip only
```

Exact for cusp height and for the feature-scale planner dials
(`close_radius_mm`, `min_region_area_mm2`, `pencil_claim_floor`,
classification cell size). **Not** an answer to "does it fit".

### 4.3 WIDTH(d) — cutter width at axial engagement (`engagement_radius(d)`)

```
   half-width w(d)                                        3.000 ┄┄┄┄┄ ENV
      (mm)                                              ,-'
   1.0 |                                          _,,-''
       |                                    _,,-''
       |                              _,,-''            cone: w = (d+3.6027)·tan7°
   0.5 |         ,--------------''''''
       |     .-''  <- ball/cone tangent at d = 0.4391
       |   ,'         ball: w = sqrt(2·r_t·d − d²)
   0.0 +-'------+--------+--------+--------+---- ... ----+-----> depth d (mm)
       0      0.5      1.0      2.0      5.0           20.83
```

| axial engagement `d` (mm) | true `engagement_radius(d)` | `radius()` = 3.0 overstates by | `cusp_radius()` = 0.5 errs by |
|---:|---:|---:|---:|
| 0.05 | 0.2179 | **13.8×** | +129% (conservative — claims *less* reach) |
| 0.20 | 0.4000 | 7.5× | +25% (conservative) |
| 0.44 (tangent) | 0.4963 | 6.0× | +0.8% |
| 0.50 | 0.5038 | 6.0× | −0.8% |
| 1.00 | 0.5651 | 5.3× | **−11%** (overstates reach) |
| 2.00 | 0.6879 | 4.4× | **−27%** (overstates reach) |
| 5.00 | 1.0563 | 2.8× | **−53%** (overstates reach) |
| 20.83 | 3.0000 | 1.0× | −83% |

Read the last two columns together: this is the audit's "same error class,
opposite direction" made numeric. `radius()` suppresses reachable detail;
`cusp_radius()` promises reach the cone will not deliver once the tool is
more than ~1 mm into the material.

### 4.4 VALLEY — two-wall fit

Symmetric V, half-angle **φ** measured from the valley bisector (so wall
inclination from horizontal is `90° − φ`; a 90°-included V has φ = 45°).

```
  case A: alpha <= phi        the CONE clears; the BALL seats
  --------------------------------------------------------------
        \                                     /
         \        _____                      /
          \     /   +   \  <- centre at r_t/sin(phi) above apex
           \    |   .   |                   /
            \   \__/ \__/                  /       contact half-width
             \    ,'  ',                  /          = r_t·cos(phi)
              \ ,'      ',               /
               X          X   <- contacts
              /'          '\
             /   float g    \                 g = r_t·(1 − sin phi)/sin phi
            /   __________   \                    <- UNREACHABLE rest,
           /   V   apex   V    \                      by any depth dial
  --------------------------------------------------------------
  case B: alpha > phi         the CONE fouls FIRST; the ball NEVER seats
  --------------------------------------------------------------
        \                                     /
         \   X======================X        /   <- cone flank jams on
          \   \                    /        /       BOTH walls up here
           \   \        __        /        /
            \   \     /    \     /        /
             \   \    | ++ |    /        /        the tip is nowhere
              \   \   \____/   /        /         near the material
               \   ',        ,'        /
                \    '.    .'         /
                 \  huge gap         /
```

Closed form for this tool/valley pair:

| φ (half-angle from bisector) | included V angle | cone clears? (`α = 7°`) | tip float `g` above apex | contact half-width `r_t·cos φ` |
|---:|---:|:--|---:|---:|
| 60° | 120° | yes | 0.077 mm | 0.250 mm |
| 45° | 90° | yes | 0.207 mm | 0.354 mm |
| 30° | 60° | yes | 0.500 mm | 0.433 mm |
| 15° | 30° | yes | 1.432 mm | 0.483 mm |
| 10° | 20° | yes | 2.379 mm | 0.492 mm |
| 7° | 14° | **marginal** | 3.601 mm | 0.496 mm |
| 5° | 10° | **NO — cone fouls** | n/a | n/a |

Two things fall out that no depth-only model can express:

1. Case B has **no depth argument at all**. `engagement_radius(d)` cannot
   represent it, because there is no `d` at which the tool is in contact.
2. The tip float `g` is the residual the pencil *cannot* remove. It is a
   property of the valley, not of the tool's dials. A routing model that
   ignores it will keep emitting centerlines into valleys the tool floats
   above — precisely the "claims are self-defeating for single-tool ops
   BY CONSTRUCTION" lesson already recorded for `pencil_claims`.

### 4.5 CLEAR(r) — vertical clearance at a lateral radius (`height_at_radius(r)`)

```
     vertical-walled slot of half-width w
     |                                   |
     |          \             /          |
     |            \         /            |
     |   ...........\_____/............   |  <- rim
     |                 |                 |
     |     D = height_at_radius(w)       |     max descent below the rim
     |                 v                 |
     |            \         /            |
     |             \  ___  /             |
     |              \/   \/              |
     |                                   |

   height_at_radius(w) is EXACTLY the inverse of width_at_height(h):
       width_at_height(height_at_radius(w)) == w        for w <= envelope
       height_at_radius(w) == None                      for w >  envelope
```

The rest-field detector already hands every routing site a measured half-width
(`RestCenterline::half_width_mm`, `rest_field.rs:224-229`: the median chamfer-DT
of the threshold mask along the ridge, in mm). `height_at_radius(half_width_mm)`
converts that directly into "how deep can this tool get here", with `None`
meaning "the valley is wider than the whole cutter — this is a clearing job,
not a pencil job". No new API, no new formula, no new hand-maintained twin.

---

## 5. Decision: the large-arc narration threshold

**Site:** `crates/rs_cam_core/src/narrate.rs:835-873`

```rust
let threshold = (tool.radius() * LARGE_ARC_RADIUS_MULTIPLIER).max(0.001);
```
with `LARGE_ARC_RADIUS_MULTIPLIER: f64 = 30.0` at `narrate.rs:25`.

**Recommendation: keep it envelope-relative (`radius()`), unchanged. Document
the contract and pin it with a sentry. Do NOT move it to `cusp_radius()`.**

Reasoning from what the diagnostic is *for*:

1. The narration string itself states the purpose (`narrate.rs:860`):
   *"Suspiciously large arcs can indicate circumscribing-circle arc-fit after
   path simplification."* It is not a machining-quality diagnostic. It is a
   **detector for a defect in our own arc fitter**.
2. The fitter enforces the *same* bound: `arcfit.rs:326`
   `if radius > tool_radius * LARGE_ARC_RADIUS_MULTIPLIER { … reject }`,
   importing the constant from narration (`arcfit.rs:16`) and documenting the
   coupling at `narrate.rs:22-24`.
3. Both sides already resolve to the **same** number today.
   `arcfit::fit_arcs` is called at `compute/execute.rs:2444` with the
   `tool_radius` bound at `compute/execute.rs:2249` (`tool_diameter / 2.0`),
   whose argument is `tool_def.diameter()` (e.g. `compute/execute.rs:3946`) —
   i.e. `ToolDefinition::radius()`. Narration reads `tool.radius()` off the same
   `ToolDefinition`.
4. Therefore narration is a **post-condition check on the fitter's own cap.**
   If narration switched to `cusp_radius()` on a tapered tool, the threshold
   would drop 6× while the fitter's cap stayed put, and the diagnostic would
   fire on every arc the fitter legitimately accepted — a guaranteed
   false-positive storm on exactly the tool class this programme is fixing.
   The same misalignment is why the arc-fit theory has now been wrongly
   accused four times on this subsystem (plan §A/L2).
5. The "feature-relative" reading would be defensible only if the message were
   about machining quality (e.g. "this arc is much larger than the detail your
   tool can cut"). It is not, and re-purposing a crash-class fitter check into
   a quality hint would lose the fitter check entirely.

**PR-2 action (doc + test only, no behavior change):**

- Rewrite the `LARGE_ARC_RADIUS_MULTIPLIER` doc at `narrate.rs:22-24` to state
  the contract explicitly: *"ENVELOPE-relative. This is a post-condition on
  `arcfit::try_fit_arc`'s own radius cap, not a feature-scale judgement. Both
  sides MUST resolve to the same radius; see the parity sentry."*
- Add a parity sentry asserting `narrate`'s threshold and `arcfit`'s cap are
  computed from the same accessor on the same `ToolDefinition`, with a
  **tapered** tool so the two would differ 6× if either drifted.
- Also correct the user-facing string at `narrate.rs:860` — it currently prints
  `"R > tool_radius × 30"`, which is now an ambiguous term. Print
  `"R > envelope_radius × 30"`.

**Consequence for H2.6:** the plan's H2.6 ("decide whether large arc means
relative to envelope or cutting feature scale") is hereby answered **envelope**,
and H2.6 collapses from a behavioral decision to a documentation + sentry task
that can ride in PR-2. It no longer needs its own routing PR.

---

## 6. Census

Legend for **Required**: ENV / CUSP / WIDTH(d) / CLEAR(r) / VALLEY / HEURISTIC
per §2. **Verdict**: `KEEP` (correct, do not touch) · `DOC` (correct, contract
unwritten) · `MIGRATE` (behavior must change — H2) · `DELETE` (caller scalar is
redundant because the callee owns `&dyn MillingCutter`).

### 6.A The eight routing/reach sites — H2 scope

These are the plan's seven plus one new (`A8`). All are **production**. None may
change in PR-2.

---

**A1 — Pencil rest-field routing yardstick**

- **Site:** `crates/rs_cam_core/src/pencil.rs:1224` — `pencil_radius: cutter.radius(),`
- **Enclosing fn:** `rest_depth_arm` (`pencil.rs:~1150-1291`)
- **Current meaning:** ENV (shank). Consumed three ways inside
  `rest_field::detect_rest_valleys`:
  (a) grid margin, `rest_field.rs:280` `margin_cells = (pencil.radius()/cell).ceil()+1`;
  (b) trust erosion, `rest_field.rs:362` `erode_cells = (pencil.radius().max(reference.erosion_radius())/cell).ceil()`;
  (c) region dilation, `rest_field.rs:482` `pencil.radius() + params.region_margin_mm`;
  (d) **the routing decision itself**, `rest_field.rs:523`
      `width_limit = params.route_width_factor * params.pencil_radius`, applied
      per traced branch against `half_width_mm`.
  Note (a)/(b)/(c) read `pencil.radius()` off the **cutter argument**, while (d)
  reads the **caller-supplied scalar** `params.pencil_radius`. They are two
  different plumbing paths that happen to carry the same number today.
- **Required meaning:** (a)(b)(c) = **ENV** — keep. (d) = **CLEAR(r)/VALLEY** —
  the question is "is this branch narrow enough that a single centerline is the
  right strategy, or is it a clearing job".
- **Available depth/profile inputs at the site:** the cutter (`&dyn MillingCutter`);
  per-branch median smoothed rest (`rest_field.rs:541-545`, computed and then
  discarded); per-branch median half-width (`rest_field.rs:546-552`, kept as
  `RestCenterline::half_width_mm`); per-component peak rest (`comp_peak`,
  `rest_field.rs:408`, kept only in `RestFieldReport::region_peak_rest_mm`).
  **Nothing depth-shaped survives onto `RestCenterline` today** — this is
  exactly what H2.1's "extend `RestCenterline` with median/peak rest depth"
  is for.
- **Intended API:** `route_width_factor × engagement_radius(branch_median_rest)`,
  floored (`.max(cusp_radius())`) so a 0-depth branch cannot collapse the limit
  to zero. Guarded by `height_at_radius(half_width_mm).is_none()` ⇒ force
  clearing (valley wider than the whole cutter).
- **Test oracle that falsifies a wrong choice:** analytic V-valley matrix, one
  branch per row, with half-widths straddling the three candidate limits
  (2×0.5 = 1.0 mm for cusp, 2×0.504 = 1.008 mm for engagement-at-0.5 mm,
  2×3.0 = 6.0 mm for envelope). Assert the **routed class** (pencil vs
  clearing) per row, not lengths. Envelope today routes a 4 mm-half-width
  valley to *pencil* (4 ≤ 6) where one centerline plus 4 offsets cannot cover
  it — the differential is unmissable. Ball control must be byte-identical.

---

**A2 — Pencil offset-pass count**

- **Site:** `crates/rs_cam_core/src/pencil.rs:1279` — 5th positional arg
  `cutter.radius()` to `crease_paths::centerline_cut_paths`
- **Enclosing fn:** `rest_depth_arm`
- **Consumer:** `crates/rs_cam_core/src/crease_paths.rs:63-66`
  ```rust
  let n = ((cl.half_width_mm - cutter_radius) / offset_stepover).round().max(0.0) as usize;
  let offset_passes = n.min(num_offset_passes_cap);
  ```
- **Current meaning:** ENV. On the wanaka tool `half_width_mm − 3.0` is negative
  for every valley narrower than 6 mm, so `n = 0` **always** and the width-aware
  pass count is dead code on tapered tools. This is the mechanism behind
  "the pencil's emitted fan is only ~tip-wide on this tool (offset passes don't
  fit under a Ø6 shank)" recorded at `unified_finish.rs:858-862`.
- **Required meaning:** **WIDTH(d)** — how much of the valley floor does the
  cutter body actually occupy at the depth it will cut at. Arguably VALLEY, but
  the offsets ride the surface (`paths_from_sampled` re-solves Z), so the
  half-width−width model is adequate once the width is honest.
- **Available inputs:** `cl.half_width_mm`; the cutter; `stock_to_leave`
  (`crease_paths.rs:41`); **no depth** — this is the gap.
- **Intended API:** `centerline_cut_paths` already takes `cutter: &dyn MillingCutter`
  (`crease_paths.rs:35`) **and** `cutter_radius: f64` (`:36`). **DELETE the
  scalar** and compute inside from the cutter + the depth statistic H2.1 adds to
  `RestCenterline`. This is the cleanest instance of H1's fix-shape item 4.
- **Test oracle:** same analytic valley matrix; assert **path count per
  centerline** (1 vs 3 vs 5) and assert that every emitted offset path's
  drop-cutter Z is within tolerance of the surface (no air-cut, no gouge).
  A wrong-scale choice moves the count by whole integers — a count oracle, per §A.0.

---

**A3 — UnifiedFinish rest routing**

- **Site:** `crates/rs_cam_core/src/unified_finish.rs:646` — `pencil_radius: cutter.radius(),`
- **Enclosing fn:** the claims pipeline in `unified_finish_toolpath_with_cancel`
  (Step 2.5, `unified_finish.rs:638-756`)
- **Current meaning / required meaning:** identical to **A1**. The doc at
  `unified_finish.rs:282-286` states the deliberate override ("the finishing tool
  IS the pencil in this op"), which is correct about *which tool* and silent
  about *which radius of it*.
- **Available inputs:** same as A1, plus `cfg.min_rest_depth_mm`
  (`unified_finish.rs:288-295`) — a genuine **depth dial already in scope at this
  site**, which A1 does not have. Also `cfg.territory_stock` when the caller
  supplies a machined prior.
- **Intended API:** must be the **same policy function** as A1 — plan H2.5
  demands one implementation, and this site and A1 differ only in who supplies
  the params struct.
- **Test oracle:** one shared policy fn + a sentry asserting standalone Pencil
  and UnifiedFinish claims produce the **same routing verdict** for the same
  (cutter, half-width, rest-depth) triple. Differential across a tapered tool
  and a ball control.

---

**A4 — UnifiedFinish offset-pass count**

- **Site:** `crates/rs_cam_core/src/unified_finish.rs:717` — 5th positional arg to
  `centerline_cut_paths`
- Identical analysis to **A2**, same consumer, same `DELETE` recommendation.
  Deleting the scalar parameter fixes A2 and A4 in one edit.

---

**A5 — UnifiedFinish derived pencil stepover**

- **Site:** `crates/rs_cam_core/src/unified_finish.rs:721` — `cutter.radius() * 0.5,`
  passed as `offset_stepover`
- **Comment at the site (`:719-721`):** *"Offset stepover mirrors `PencilParams`'s
  own default (`tool_radius * 0.5`) — no dedicated dial in S1."*
  The standalone-pencil default it claims to mirror is
  `PencilParams::default().offset_stepover = 0.5` (`pencil.rs`, Default impl) —
  a **literal 0.5 mm**, not `radius() * 0.5`. On a Ø6 ball the two coincide
  (3.0 × 0.5 = 1.5 ≠ 0.5, actually they do **not** coincide) — so the comment is
  wrong on both counts and should be corrected in PR-2's doc sweep.
- **Current meaning:** ENV × 0.5 → 1.5 mm on the wanaka tool.
- **Required meaning:** **WIDTH(d)** — a stepover is a spacing between adjacent
  engaged widths. This is the single row the existing API answers outright:
  `engagement_radius(depth)` is documented for exactly "stepover sizing"
  (`tool/mod.rs:218-220`).
- **Available inputs:** the cutter; `params.stock_to_leave`; `params.scallop_height`
  (the op's own cusp dial); `cfg.min_rest_depth_mm`.
- **Intended API:** `engagement_radius(local_rest_depth).max(cusp_radius()) * 0.5`,
  or — stronger, and consistent with the rest of the op — derive it from the
  **cusp target** via `scallop_math::stepover_from_scallop_*` on `cusp_radius()`,
  which is what the mid-steep band already does (`scallop.rs:702`). H2.3
  correctly says "only after the reach experiment"; this census's position is
  that the cusp-derived form is the one to test first, because it makes the
  pencil node's spacing commensurable with the scallop node's for the first time.
- **Test oracle:** emit into a synthetic constant-slope ribbon and measure
  **achieved cusp between adjacent offset passes** against the commanded
  `scallop_height`, on the tapered tool and the ball control. A 3× stepover
  error shows as a 9× cusp error — the same signature as the P2.f tip-radius
  bug already caught once.

---

**A6 — UnifiedFinish crease-own-region threshold (dormant)**

- **Site:** `crates/rs_cam_core/src/unified_finish.rs:868` —
  `decompose(&surface.slope_map, &covered, &[], cutter.radius(), planner)`
- **Consumer:** `finish_planner::decompose(…, tool_radius, …)` (`finish_planner.rs:275-281`)
  forwards it to `apply_crease_corridor` (`finish_planner.rs:723-729`), where its
  **only** use is `finish_planner.rs:794`:
  `let is_canyon = centerline.half_width_mm >= params.corridor_k * tool_radius;`
- **Dormancy:** the crease slice is `&[]` at this call **unconditionally** —
  the comment at `unified_finish.rs:856-867` explains why (S1 reined claims in
  to additive-only). `apply_crease_corridor` therefore never runs from
  UnifiedFinish. `rg` finds no other production caller of `decompose` with a
  non-empty crease slice.
- **Current meaning:** ENV. **Required meaning:** CLEAR(r) — "is this valley wide
  enough that it deserves its own clearing region rather than a pencil corridor"
  is the same question as A1, one level up.
- **Available inputs:** `centerline.half_width_mm`; `params.corridor_k` (default 2.0,
  `finish_planner.rs:169`); `params.pencil_claim_floor` (already cusp-derived,
  `finish_planner.rs:170`); the slope map. **No cutter** — `decompose` takes a
  bare `f64`.
- **Intended API:** per plan H2.4, *remove or rename* the scalar. Two options:
  (i) rename to `canyon_scale_mm` and require callers to pass a reach-model
  value — minimal churn, keeps `decompose` cutter-free (it is a pure grid
  function today and that is worth preserving); or
  (ii) fold it into `FinishPlannerParams` next to `corridor_k` and
  `pencil_claim_floor`, which are already tool-derived via `for_tool`.
  **Recommendation: (ii)** — `for_tool` is already the single place tool scale
  enters the planner, and adding a fourth derived dial there costs one line and
  removes a positional `f64` from a public signature.
- **Test oracle:** **this path needs a sentry that makes it live before anything
  changes** (plan H2.4 says the same). Direct unit test on `decompose` with a
  hand-built `RestCenterline` slice, asserting `PlannedCrease::own_region` is
  `Some`/`None` across the threshold, tapered vs ball. Until that exists, the
  site is untestable and must not be touched.

---

**A7 — Generic rest analysis routing**

- **Site:** `crates/rs_cam_core/src/compute/execute.rs:2122` — `pencil_radius: tool_def.radius(),`
- **Enclosing fn:** `attach_generic_rest_analysis` (`compute/execute.rs:2098-2128`)
- **Current meaning:** ENV, same three-plus-one consumption as A1.
- **Required meaning:** identical to A1/A3. This is the op-agnostic
  post-generation attachment reached from the MCP `set_rest_analysis_config`
  tool (`rs_cam_viz/src/mcp_server.rs:866`) and the GUI rest-analysis panel
  (`rs_cam_viz/src/ui/properties/mod.rs:4128`), so a wrong yardstick here
  mis-draws the **heatmap the user reads** and the
  `BoundarySource::DerivedRestRegions` polygons another op consumes.
- **Available inputs:** `tool_def: &ToolDefinition` (a full cutter);
  `cfg: &RestAnalysisConfig` with `cell_mm` / `min_valley_depth` /
  `region_margin_mm` (`compute/config.rs:332-361`). Note `RestAnalysisConfig`
  has **no** `route_width_factor` — this call takes the `Default` (2.0) via
  `..Default::default()` at `execute.rs:2123`, and also takes the **default
  `pencil_radius` of 0.5** for any field it does not set. It sets it, so the
  default is shadowed — but the struct default (`rest_field.rs:135`,
  `pencil_radius: 0.5`) being a *tip-scale literal* while every production
  caller overrides it with a *shank-scale* value is itself a smell worth
  recording.
- **Intended API:** call the same shared policy as A1/A3. Plan H2.5 is explicit:
  "no parallel formula in `compute/execute.rs`."
- **Test oracle:** a sentry asserting that, for one fixed (mesh, cutter,
  cell, min_valley_depth) tuple, `attach_generic_rest_analysis` and
  `pencil::rest_depth_arm` produce **the same centerline count and the same
  routed classes**. That single assertion makes any future divergence impossible
  to land quietly.

---

**A8 — NEW: rest *reference tool* sized from the shank**

- **Sites:** `crates/rs_cam_core/src/pencil.rs:1080` (`curvature_arm`),
  **`:1183`** (`rest_depth_arm`), `:1307` (`dihedral_arm`) —
  `resolve_reference_cutter(params, cutter.diameter())`
- **Consumer:** `pencil.rs:1051-1062`
  ```rust
  fn resolve_reference_cutter(params: &PencilParams, pencil_diameter: f64) -> ResolvedReference<'_> {
      if let Some(rc) = params.reference_cutter.as_ref() { return ResolvedReference::Real(rc); }
      if params.reference_tool_diameter > pencil_diameter + 1e-6 {
          return ResolvedReference::Nominal(BallEndmill::new(params.reference_tool_diameter, …));
      }
      ResolvedReference::SelfReferenced
  }
  ```
- **Why this is a defect:** the guard asks *"is the nominal reference genuinely
  bigger than the pencil tool?"* — a **feature-scale** question — and answers it
  with `diameter()`, the **shank**. Default `reference_tool_diameter` is 6.0
  (`pencil.rs:429-431`). On the wanaka tool `diameter() = 6.0`, so
  `6.0 > 6.0 + 1e-6` is **false** and every tapered pencil op silently falls to
  `SelfReferenced` — the analytic self-probe. That is the *same* class of
  footgun the plan flags as A/M6 (`claims_reference: self_probe`, measured
  −88.7% cutting when corrected), reached by a completely different route and
  **not** covered by A/M6's fix (which is about `ClaimsConfig`, not
  `PencilParams`).
- **Required meaning:** **CUSP** — "is the reference coarser than my cutting
  tip". With `cusp_radius()*2 = 1.0`, the default 6 mm reference correctly
  engages and the op measures rest against a genuinely bigger tool.
- **Available inputs:** the cutter; `params.reference_cutter` (a full
  `ToolDefinition` when the user picked a real reference tool);
  `params.reference_tool_diameter`.
- **Intended API:** compare on `cusp_radius() * 2.0`, or better, pass
  `&dyn MillingCutter` and compare `cusp_radius()` directly — the callee is
  already in the tool module's dependency set.
- **Test oracle:** a two-row differential — tapered Ø1/Ø6 tool with default
  `reference_tool_diameter = 6.0` must resolve to `Nominal`, not
  `SelfReferenced`; ball Ø6 with the same setting must stay `SelfReferenced`
  (unchanged). Assert the resolved discriminant, not any downstream length.
- **Severity:** this invalidates *every* tapered-tool pencil measurement in the
  §14 campaign independently of the `pencil_radius` defect, and belongs in
  H4's `SUPERSEDED_CONCLUSIONS.md` as a **fifth** stacked instrument defect.

---

### 6.B Finish-surface resolution — H3 scope, listed here for completeness

| # | site | enclosing fn | current | required | inputs available | intended API | test oracle |
|---|---|---|---|---|---|---|---|
| B1 | `finish_setup.rs:95` | `build_finish_surface_with_cancel` | `(cutter.radius()/4).max(tolerance)` — **ENV** | CUSP or an explicit tolerance-driven policy. Shared by Scallop (`scallop.rs:706`), RampFinish (`ramp_finish.rs:381`), SteepShallow (`steep_shallow.rs:589`) | cutter, `tolerance` | **H3, not H1.** `FinishSurfaceSpec`/`FinishResolutionPolicy` per plan H3 fix-sequence 1 | per-consumer A/B with only cell size varied; label-grid topology per §A.0, never area |
| B2 | `finish_setup.rs:61` | `build_finish_surface_with_cell_size_and_cancel` | ENV padding | **ENV — correct** | — | none | existing |
| B3 | `finish_setup.rs:146` | `build_classification_surface_with_cancel` | ENV padding, alongside `cusp_radius()/4` cell at `:147` | **ENV — correct**, deliberately split from cell size by `32c5e48` | — | none | `tapered_cusp_radius_sentry.rs:128-152` already pins both halves |

### 6.C Diagnostics

| # | site | enclosing fn | current | required | verdict |
|---|---|---|---|---|---|
| C1 | `narrate.rs:840` | `append_large_arc_anomalies` | ENV × 30 | **ENV** — see §5 | **DOC** + parity sentry. Fix the printed label `tool_radius` → `envelope_radius` at `narrate.rs:860` |
| C2 | `viz.rs:786` | HTML sim export, tool-profile LUT | ENV (sampling range 0..radius) | ENV | KEEP |
| C3 | `viz.rs:1451` | multi-phase HTML export, per-phase profile | ENV | ENV | KEEP |
| C4 | `rs_cam_viz/src/render/toolpath_render.rs:1098` | swept-volume profile height fallback | `length().max(radius()*2)` — ENV | ENV | KEEP |
| C5 | `rs_cam_viz/src/ui/properties/tool.rs:379` | tool-preview drawing | ENV | ENV | KEEP |
| C6 | `rs_cam_cli/src/main.rs:611`, `:705` | sim bbox margin for HTML/diagnostics | ENV (max over phases) | ENV | KEEP |

### 6.D Finishing generation — extents and scale (all ENV, all KEEP)

| # | site | enclosing fn | use | verdict |
|---|---|---|---|---|
| D1 | `scallop.rs:701` | `scallop_toolpath_structured_annotated_with_cancel` | grid extent + `max_rings` raw extent; comment at `:698-700` already states the split, and `:702` takes `cusp_radius()` for all cusp math | KEEP (exemplary — this is the pattern the routing sites should copy) |
| D2 | `waterline.rs:193` | `waterline_contours_with_cancel` | fiber-grid bbox expansion | KEEP |
| D3 | `horizontal_finish.rs:197` | flat-region raster | region inset/coverage | KEEP |
| D4 | `spiral_finish.rs:186` | spiral extent | `max_corner_distance_xy + radius` — coverage margin | KEEP (rule 4) |
| D5 | `adaptive3d/path.rs:203` | `adaptive3d_segments…` | grid expansion; planning uses `params.tool_radius` (= engagement) separately | KEEP |
| D6 | `adaptive3d/path.rs:1208` | stay-down distance default | `(radius()*2)*8` — HEURISTIC (Fusion HSM "8 × tool diameter") | **DOC** — the heuristic's lineage is stated at `:1203-1205`; add "envelope diameter, deliberately" |
| D7 | `adaptive3d/path.rs:1551` | `drape_path_to_leave` | drape probe radius — physical | KEEP |

### 6.E Rest-field internals (ENV where physical)

| # | site | use | verdict |
|---|---|---|---|
| E1 | `rest_field.rs:80` | `RestReference::erosion_radius` — the *reference* tool's overhang | KEEP (ENV of the reference; a bigger tool really does hang off the edge by its full radius) |
| E2 | `rest_field.rs:280` | grid margin so the outer ring is genuinely non-contact | KEEP (rule 4) |
| E3 | `rest_field.rs:362` | trust erosion | KEEP (rule 4) |
| E4 | `rest_field.rs:482` | region-polygon dilation | KEEP (rule 4). Note `unified_finish.rs:805-812` deliberately does **not** use `radius()` for its own territory dilation, with a measured justification — a useful precedent that "dilate by envelope" is not automatic |

### 6.F 2.5D / clearing / dressup adapters (ENV, KEEP)

`compute/execute.rs`: `:121` (drill tool diameter), `:432` (rest machining),
`:586` (pocket), `:636` (trace), `:683` (profile), `:746` (zigzag/inlay family),
`:812` (face), `:849` (2D adaptive), `:991` (adaptive3d **envelope** — the good
precedent, §3.2), `:1137` (project curve). All are 2.5D offsets or keep-out
margins where the envelope is the correct and conservative answer. None is in
H1/H2 scope.

### 6.G Simulation, physics and geometry engine (ENV, KEEP — rule 4)

| site | use |
|---|---|
| `dropcutter.rs:24` | spatial-index query radius |
| `dropcutter.rs:110` | bbox expansion for the drop grid |
| `pushcutter.rs:31` | fiber query radius |
| `radial_profile.rs:33` | LUT domain `0..radius` (sampling `height_at_radius`) |
| `feedopt.rs:131` | engagement sampling radius |
| `dexel_stock/simulation.rs:47`, `:119`, `:354` | stamping radius |
| `compute/simulate.rs:613` | per-toolpath stamp radius |
| `session/compute.rs:1529` | `optimize_entry_descents_with_provenance` ceiling radius |
| `session/compute.rs:3655` | `auto_resolution_for_groups` — min radius over the group → sim cell size |
| `feeds/cutter_constraints.rs:279` | `immersion_angle(radial_woc, radius)` |
| `tool_load/optimize/preflight.rs:165`, `:185` | min-WOC fallback + immersion |

Two of these deserve a footnote even though they stay ENV:

- `session/compute.rs:3655` picks the sim cell from `min_radius / 5`. On a
  tapered tool that is the **shank**, giving 0.6 mm → clamped to 0.5 mm — the
  exact resolution the ledger already records as "the size of the whole cutting
  tip". This is a *resolution* defect, not a radius-semantics defect, and it
  belongs to plan A/M10 (descent/resolution coupling), not H1/H2. **Flagged
  here so H2 does not accidentally claim it.**
- `session/compute.rs:1529` feeds the descent-optimiser ceiling. Also A/M10.

### 6.H Tool shape internals (definitional, KEEP)

`tool/mod.rs:184` (the `cusp_radius` fallback), `:326` (`profile_points`
domain), `:420` (`to_assembly` — collision, pinned by
`tool/mod.rs:797-808`); `tool/flat.rs:55,68,76,86,93`;
`tool/ball.rs:49,68,78,87,90,97`; `tool/bullnose.rs:82,97,116,158`;
`tool/vbit.rs:61,114,146,158,168,223`. These *define* the profile; they are the
ground truth every other row is measured against.

---

## 7. NEW FINDINGS

Beyond the plan's seven.

### 7.1 `resolve_reference_cutter` compares on `diameter()` (HIGH)

Full analysis at **A8** above. `pencil.rs:1080`, **`:1183`**, `:1307`.
Every tapered pencil op silently falls through to `SelfReferenced` because the
default 6 mm nominal reference is not "bigger than" the 6 mm **shank**.
Independent of, and additive to, the `pencil_radius` defect. Must be added to
H4's ledger and to H2.1's scope.

### 7.2 `unified_finish.rs:719-721`'s justifying comment is factually wrong (LOW, but it steers H2.3)

The comment claims the derived stepover *"mirrors `PencilParams`'s own default
(`tool_radius * 0.5`)"*. `PencilParams::default().offset_stepover` is the literal
`0.5` (`pencil.rs` Default impl), not `radius() * 0.5`. On a Ø6 ball the two are
1.5 mm and 0.5 mm — a 3× difference, not a mirror. H2.3 must not treat "keep
parity with standalone pencil" as a constraint; there is no parity today.

### 7.3 `RestFieldParams::pencil_radius`'s default is tip-scale, every production caller overrides it with shank-scale (MEDIUM)

`rest_field.rs:135` defaults to `0.5`. Production callers
(`pencil.rs:1224`, `unified_finish.rs:646`, `compute/execute.rs:2122`) all
overwrite it with `radius()`. So the struct's own default encodes the *correct*
intent and the callers all defeat it. Any test constructing
`RestFieldParams::default()` is therefore testing a configuration production
never runs — including `rest_field.rs`'s own unit tests, which build
`pencil_radius: pencil.radius()` at `:2120` with **ball** cutters, making them
inert. Worth stating in the H2.1 PR description so no one mistakes the existing
green tests for coverage.

### 7.4 `ToolDefinition` does not explicitly delegate `radius()` or `cusp_radius()` (MEDIUM)

§2.1. Both work only via inherited defaults over delegated primitives. H1's
delegation-parity gate should be read as requiring **explicit** delegation for
every new accessor, plus explicit delegation added for `cusp_radius()` in PR-2.

### 7.5 Three different tapered-ball width models coexist (MEDIUM)

- `TaperedBallEndmill::width_at_height` (`tool/tapered_ball.rs:181-199`) — true
  ball/cone tangency;
- `ToolGeometryHint::engaged_diameter_at_doc` `TaperedBall` arm
  (`feeds/mod.rs:131-148`) — same geometry, hand-maintained twin, **has** a
  parity sentry;
- `feeds/geometry.rs::tapered_ball_effective_diameter` (`:56-69`) — a straight
  cone from the tip, **no** tangency, **no** parity sentry, 5% off at 0.5 mm DOC.

The third is feeds-lane only and out of PR-2 scope, but it must be recorded so
that no H2 reach model is built on it by accident.

### 7.6 `capability_link_moves_safety.rs:1541` builds the planner from `radius()` (LOW, latent)

`FinishPlannerParams::for_tool(cutter.radius())` — the parameter is named
`cusp_radius`. Inert today because the fixture is `BallEndmill::new(3.0, 25.0)`.
It becomes a live wrong-oracle the moment anyone tapers that fixture, which
M2.4's "same shaft, different tip" control will want to do. Fix opportunistically
in PR-1.

### 7.7 `compute/execute.rs:957` sizes the helix-entry radius from `diameter()` (LOW, out of scope)

`radius: ctx.tool_def.diameter() * cfg.helix_radius_factor` — a helix ramp
diameter derived from the envelope. Defensible for entry clearance, but on a
tapered tool the helix is being sized to a width the tool does not have at ramp
depth. Not finishing, not rest, not diagnostics — **explicitly out of this
programme's scope** per plan §5 "explicitly out of scope"; recorded only so the
next audit does not re-discover it as new.

---

## 8. What PR-2 must NOT touch

Plan rule 4 made enforceable. Every site in this list keeps `radius()` (or
`envelope_radius_mm()` as its alias) and must produce **byte-identical** output:

1. **Collision** — `tool/mod.rs:418-427` (`to_assembly`), pinned by
   `tool/mod.rs:775-808`.
2. **Bbox padding / grid extent** — `finish_setup.rs:61`, `:146`;
   `scallop.rs:701`; `waterline.rs:193`; `dropcutter.rs:110`;
   `adaptive3d/path.rs:203`; `rest_field.rs:280`.
3. **Spatial queries** — `dropcutter.rs:24`; `pushcutter.rs:31`.
4. **Swept volume / stamping** — `dexel_stock/simulation.rs:47,119,354`;
   `compute/simulate.rs:613`; `radial_profile.rs:33`;
   `rs_cam_viz/src/render/toolpath_render.rs:1098`.
5. **Coverage margins / erosion / dilation** — `spiral_finish.rs:186`;
   `rest_field.rs:80,362,482`; `horizontal_finish.rs:197`;
   `adaptive3d/path.rs:1551`.
6. **2.5D offset adapters** — every `compute/execute.rs` site in §6.F.
7. **Feeds/force/deflection** — `feeds/cutter_constraints.rs:279`;
   `tool_load/optimize/preflight.rs:165,185`; and *all* of
   `feeds/geometry.rs` (§7.5).
8. **Tool shape internals** — everything in §6.H.
9. **Simulation resolution and descent planning** — `session/compute.rs:1529`,
   `:3655`. These are A/M10's, not H1's; touching them here would confound
   two programmes.
10. **The narration threshold's numeric value** — §5. Documentation and a
    parity sentry only.

Additionally PR-2 must not: change any grid cell size (that is H3/Checkpoint B),
change any routing formula (H2/Checkpoint A), or rename any serialized field
(that is M1, and needs serde aliases).

---

## 9. ADR — additive named methods vs typed millimetre newtypes

**Status:** proposed. **Decision needed at:** Checkpoint A, before PR-2 is written.

### Context

Radius-shaped quantities in this codebase answer six different questions (§2)
and all currently return bare `f64`. The audit's own summary is that the failure
mode is *semantic*, not *numeric*: `radius()` was never wrong about the envelope;
it was asked the wrong question at eight sites.

### Option 1 — additive named methods (recommended)

Add to `MillingCutter`:

```rust
fn envelope_radius_mm(&self) -> f64;          // = radius(); alias, not new math
fn cusp_radius_mm(&self) -> f64;              // = cusp_radius(); rename for symmetry
fn engagement_radius_mm(&self, depth_mm: f64) -> f64;  // = engagement_radius(depth)
```

and **do not add** `profile_height_mm(radius)` — `height_at_radius(r)` already
is it (§3.1, §4.5). Renaming it would be pure churn on a required trait method
implemented by five shapes.

`radius()` stays as the compatibility alias for `envelope_radius_mm()` — plan
fix-shape item 2.

**Pros.** Zero behavior risk; every accessor is a rename or a delegation, so
"byte-identical output" is trivially provable. The reader sees the semantic class
at the call site, which is the exact defect. Matches the existing successful
precedent (`Adaptive3dParams::{tool_radius, envelope_radius}`, §3.2) and the
existing successful naming precedent (`FinishPlannerParams::for_tool(cusp_radius)`,
§3.5). Migration is site-by-site and independently revertible, which the plan's
merge order requires. Costs one line per shape for explicit `ToolDefinition`
delegation, closing §7.4.

**Cons.** A wrong call is still only a *readability* error, not a compile error —
nothing stops a future site calling `envelope_radius_mm()` for a reach question.
The `_mm` suffix on a codebase that is millimetres-everywhere adds noise;
the honest justification for it is *symmetry with `min_region_area_mm2`,
`close_radius_mm`, `region_margin_mm`, `min_rest_depth_mm`* — the finishing lane
already suffixes its dials, so the accessors should match.

### Option 2 — typed millimetre newtypes

`struct EnvelopeRadius(f64); struct CuspRadius(f64); struct EngagedRadius(f64);`
with arithmetic impls.

**Pros.** Mixing classes becomes a compile error. Strongest possible guarantee.

**Cons.** 76 production + 112 test sites touch these values, and the values flow
into `f64`-typed config structs (`RestFieldParams`, `Adaptive3dParams`,
`FinishPlannerParams`, `TraceParams`, `FaceParams`, `AdaptiveParams`, …), serde
project IO, MCP parameter structs (`rs_cam_mcp`), and the GUI. Every boundary
needs an unwrap, and every unwrap is a place the type system stops helping. The
plan's rule 10 forbids bundling broad `MillingCutter` consumers; a newtype
migration *is* that bundle by construction. It also cannot be landed as
"no production behavior change" in one PR, which is H1's own acceptance gate.

Decisively: the eight defective sites are not cases where two *different*
radius values were mixed in one expression. They are cases where **one** value
was fetched from the wrong accessor. A newtype prevents mixing; it does not
prevent calling the wrong constructor. It would have caught **zero** of the
eight.

### Decision

**Adopt Option 1 now. Defer Option 2 to a post-migration review** — plan
fix-shape item 5, "consider strong newtypes only after the targeted migration
quantifies ergonomics and churn." Revisit if, after H2 lands, the migration
turns up a site where two classes are genuinely combined in one arithmetic
expression; that is the failure mode newtypes actually prevent, and it has not
been observed yet.

Also adopt, as binding rules:

- **No generic `feature_radius`** (plan, verbatim).
- **No fourth name for `height_at_radius`.**
- **No new hand-maintained geometry twin without a parity sentry in the same PR**
  (§3.3's precedent, §7.5's counter-example).
- **Every new accessor is explicitly delegated by `ToolDefinition`**, not
  inherited (§2.1, §7.4).

### 9.1 PR-2 migration table

PR-2 = the additive-API PR. **No behavior change.** Every row below is either a
rename, a delegation, a doc, or a test.

| # | Site | Action in PR-2 | Behavior change? | Rationale |
|---|---|---|---|---|
| 1 | `tool/mod.rs:151` `radius()` | add `envelope_radius_mm()` as the primary; `radius()` becomes `#[doc(alias)]`-style compat wrapper delegating to it | none | plan fix-shape 2 |
| 2 | `tool/mod.rs:181` `cusp_radius()` | add `cusp_radius_mm()`; keep `cusp_radius()` as compat | none | symmetry |
| 3 | `tool/mod.rs:226` `engagement_radius(d)` | add `engagement_radius_mm(depth_mm)`; keep old as compat | none | symmetry |
| 4 | `tool/mod.rs:206` `height_at_radius(r)` | **no rename.** Extend the doc to state it is the CLEAR(r) query and the inverse of `width_at_height` | none | §3.1 — it already is `profile_height_mm` |
| 5 | `tool/mod.rs` `impl MillingCutter for ToolDefinition` (`:514`) | add **explicit** delegations for `radius`, `cusp_radius`, and all three new accessors | none | closes §7.4 |
| 6 | `scallop.rs:701-702` | migrate to `envelope_radius_mm()` / `cusp_radius_mm()` | none | already correct; migrate first because it is the model call site |
| 7 | `finish_setup.rs:61,146,147` | migrate to the named accessors | none | already correct; keeps the H3 diff clean |
| 8 | `compute/execute.rs:1341` | `cusp_radius()` → `cusp_radius_mm()` | none | already correct |
| 9 | `pencil.rs:1366` | `cusp_radius()` → `cusp_radius_mm()` | none | already correct (fixed in `63d5e8b`) |
| 10 | `compute/execute.rs:988,991` | migrate to `engagement_radius_mm()` / `envelope_radius_mm()` | none | the reference implementation; make it look like one |
| 11 | `narrate.rs:840` | `tool.radius()` → `tool.envelope_radius_mm()`; rewrite the `LARGE_ARC_RADIUS_MULTIPLIER` doc; fix the printed label at `:860` | **none** (§5) | resolves H2.6 without a behavioral PR |
| 12 | `arcfit.rs:16,245,326` | doc only: state the cap is envelope-relative and must track narration | none | §5 |
| 13 | **new sentry** | `narrate`/`arcfit` threshold parity on a **tapered** tool | n/a | §5 |
| 14 | **new sentry** | `ToolDefinition` delegation parity for all five accessors, tapered + ball | n/a | H1 gate |
| 15 | **new property test** | `envelope_radius_mm() >= width_at_height(h)` for all sampled `h ∈ [0, length]`, all five shapes | n/a | H1 gate |
| 16 | **new property test** | `engagement_radius_mm(d)` monotonic non-decreasing in `d`, bounded by envelope; and `width_at_height(height_at_radius(r)) ≈ r` for `r ≤ envelope` | n/a | H1 gate + pins §4.5's inverse claim |
| 17 | `capability_link_moves_safety.rs:1541` | `for_tool(cutter.radius())` → `for_tool(cutter.cusp_radius_mm())` | none (ball fixture) | closes §7.6 before M2.4 tapers it |
| 18 | `crease_paths.rs:36` `cutter_radius: f64` | **DELETE** the parameter; `cutter: &dyn MillingCutter` at `:35` already owns it. Compute `cutter.envelope_radius_mm()` inside so the arithmetic at `:63` is bit-identical | **none** — this is the one "delete a caller scalar" that is provably inert, because both production callers (`pencil.rs:1279`, `unified_finish.rs:717`) and the unit test (`crease_paths.rs:144,172`) pass exactly `cutter.radius()` | plan fix-shape 4; A2/A4 then become a one-line change inside one function in PR-4 |

**Caller-supplied scalars that PR-2 must NOT yet delete** (the callee owns the
cutter, but deleting changes behavior or needs a live sentry first):

| scalar | site | why it waits |
|---|---|---|
| `RestFieldParams::pencil_radius` | `rest_field.rs:116`, set at `pencil.rs:1224`, `unified_finish.rs:646`, `compute/execute.rs:2122` | `detect_rest_valleys` already takes `pencil: &dyn MillingCutter` and reads `pencil.radius()` at `:280,:362,:482`, so the field is redundant **today** — but it is the single lever H2.1 needs to split routing from padding. Deleting it in PR-2 would erase the seam before the fix uses it. **Rename/split in PR-4, not before.** |
| `finish_planner::decompose(…, tool_radius, …)` | `finish_planner.rs:279`, set at `unified_finish.rs:868` | dormant (A6). Needs the live sentry first (plan H2.4). Fold into `FinishPlannerParams` in PR-6. |
| `Adaptive3dParams::{tool_radius, envelope_radius}` | `adaptive3d/mod.rs:104,108` | **keep the scalars.** The planner is a pure function over a params struct by design; passing `&dyn MillingCutter` into it would widen the blast radius for no semantic gain. This is the pattern working correctly. |

---

## 10. Answers to H1's five research questions

1. **What is each decision actually asking?** §6, one row per site. 8 sites ask
   a reach/fit question and answer with the envelope; 5 ask a heuristic/contract
   question with the contract unwritten; 63 correctly ask for the envelope.
2. **Is `engagement_radius(depth)` sufficient?** For **A5** (derived stepover),
   yes — it is documented for exactly that. For **A2/A4** (offset-pass count),
   yes once a depth statistic reaches `RestCenterline`. For **A1/A3/A6/A7**
   (routing/canyon classification), **no** — the input is a measured half-width,
   not a depth, so the correct query is `height_at_radius(half_width)`; and the
   cone-fouling case (§4.4 case B) has no depth argument at all.
3. **What depth is available at each routing point?** Today: **none** survives
   onto `RestCenterline` (`rest_field.rs:224-229` carries only `half_width_mm`).
   Branch-median rest is computed at `rest_field.rs:541-545` and discarded;
   component peak at `:408` reaches only the report. UnifiedFinish additionally
   has `cfg.min_rest_depth_mm` in scope. Commanded stock-to-leave is available
   at `crease_paths.rs:41`. **H2.1's `RestCenterline` extension is the
   prerequisite for every depth-aware model.**
4. **Conservative at the deepest, representative at the median, or local?**
   Recommendation: **median for routing, local for emission.** Routing is a
   discrete class decision over a whole branch, and the branch median is already
   the metric every other per-branch gate uses (`rest_field.rs:543`,
   `pencil::polyline_passes_depth`) — using peak there would route texture
   spurs as canyons on any branch with one deep pixel. Emission (offset-pass
   fit, per-point Z) is already local via `paths_from_sampled`'s drop-cutter
   re-solve, and should stay local. Peak belongs only in a **safety** gate:
   if peak rest exceeds `height_at_radius(half_width)`, the branch is
   *unreachable at its deepest* and should raise a finding, not silently emit.
5. **Which APIs stay backward compatible, which scalars go?** §9.1's two tables.
   Compatible: `radius()`, `cusp_radius()`, `engagement_radius()`,
   `height_at_radius()`, `width_at_height()`, all `Adaptive3dParams` fields.
   Deletable now: `crease_paths::centerline_cut_paths`'s `cutter_radius`.
   Deletable later, in the behavioral PRs: `RestFieldParams::pencil_radius`
   (PR-4), `decompose`'s `tool_radius` (PR-6).

---

## Appendix A — test-only `.radius()` sites (112)

Excluded from the census. Listed so the H2 sentry work knows what already exists
and what is inert.

**Inside `#[cfg(test)]` in `src` (58):**

| file | lines | count | note |
|---|---|---:|---|
| `dexel_stock/mod.rs` | 364, 390, 425, 449, 500, 525, 534, 605, 632, 668, 695, 722, 747, 769, 783, 812, 824, 886 | 18 | stamping fixtures, ball cutters |
| `adaptive3d/mod.rs` | 1026, 1464, 1493, 1698, 1745, 1765, 1867, 1887, 1969, 2088, 2226, 2391 | 12 | planner fixtures |
| `scallop.rs` | 1123, 1186, 1231, 1283, 1508, 1530 | 6 | `:1508`/`:1530` call `stepover_from_scallop_flat(cutter.radius(), …)` — **inert on balls, would be the P2.f bug again on a taper**; M2.2 should taper one of these |
| `steep_shallow.rs` | 814, 875, 951, 1079, 1192 | 5 | |
| `dexel_mesh.rs` | 1202, 1211, 1263 | 3 | |
| `rs_cam_cli/src/job.rs` | 863, 880, 901 | 3 | asserts `radius()` per tool type incl. tapered (`:901` expects 6.0 — the shank; correct) |
| `crease_paths.rs` | 144, 172 | 2 | both pass `tool.radius()` as the scalar PR-2 deletes (§9.1 row 18) |
| `feeds/cutter_constraints.rs` | 570, 687 | 2 | |
| `tool/flat.rs` | 165, 400 | 2 | |
| `rest_field.rs` | 2120 | 1 | `pencil_radius: pencil.radius()` — ball, inert (§7.3) |
| `tool/ball.rs` · `tool/bullnose.rs` · `tool/vbit.rs` · `tool/mod.rs` | 234 · 294 · 365 · 775 | 4 | shape assertions |
| `unified_finish.rs` | 2233 | 1 | |

**In `tests/` and `benches/` (54):**

| file | lines | count | note |
|---|---|---:|---|
| `benches/perf_suite.rs` | 196, 213, 315, 488, 505, 522, 550 | 7 | |
| `tests/adaptive3d_keep_down_link_f038b.rs` | 234, 238, 290, 379, 385, 486, 488, 493, 495 | 9 | |
| `tests/p2c_headless_ab_wanaka.rs` | 1466, 1626, 1734, 2030, 2279, 2478 | 6 | `r_max` — envelope, correct |
| `tests/sub_cell_stamping_fa.rs` | 67, 97, 136, 185, 213, 242 | 6 | stamping — envelope, correct |
| `tests/tapered_cusp_radius_sentry.rs` | 43, 45, 57, 71, 73, 75, 87, 146, 192, 193 | 10 | `:192-193` use `radius()` **deliberately** as the shaft-dial control arm |
| `tests/agent_search_coverage.rs` | 197, 221, 222, 223, 268 | 5 | |
| `tests/adaptive3d_entry_coalescing_f038.rs` | 213, 218, 226 | 3 | |
| `tests/wanaka_z_layer_render.rs` | 206, 207 | 2 | |
| `tests/end_to_end.rs` | 521, 522 | 2 | |
| `rs_cam_cli/tests/integration.rs` | 97, 99, 115 | 3 | |
| `tests/capability_link_moves_safety.rs` | 1541 | 1 | **wrong oracle, currently inert** — §7.6 |

---

## Appendix B — reproduction commands

Read-only. No cargo was run to produce this document.

```bash
# production vs test classification (the census's primary source)
rg -n '\.radius\(\)' crates/rs_cam_core/src crates/rs_cam_viz/src \
                    crates/rs_cam_cli/src crates/rs_cam_mcp/src

# the contrast set
rg -n 'cusp_radius|engagement_radius|height_at_radius|width_at_height' \
   crates/rs_cam_core/src

# the eight H2 sites, in one shot
rg -n 'pencil_radius|centerline_cut_paths|LARGE_ARC_RADIUS_MULTIPLIER|\
resolve_reference_cutter|decompose\(' crates/rs_cam_core/src
```

Numeric table in §4.3 / §4.4 derived analytically from
`crates/rs_cam_core/src/tool/tapered_ball.rs:92-99` and `:181-199` for
`TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)` — the fixture at
`crates/rs_cam_core/tests/tapered_cusp_radius_sentry.rs:32-34`.
