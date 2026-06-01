# Feeds & Speeds modal — finishing UX enhancements

**Date:** 2026-06-02
**Trigger:** Operator question on the 3D Finish 6 path of the Wanaka
project — Suggest produced stepover = 0.05 mm on a 1 mm bit, which
is sub-micron-scallop overkill for wood. Tapered tools also lack a
visible "engaged-diameter at DOC" cue, making the cone-shoulder
cutting situation invisible.

## Scope (4 sub-items, ordered by payoff)

Each is independently committable. Default behaviour preserved so
F-037 smoke baseline stays clean (no within→exceeds shifts).

### S1 — Scallop-driven stepover on 3D parallel-finish ops

**Problem:** DropCutter (the "3D Finish" op family the user is on)
uses `operation_default_profile` to set stepover at
`ae_factor=0.03 × tool_diameter`. For a 1 mm bit on wood that's
0.03 mm — equivalent to ~0.2 μm scallop. Practical wood-finish
scallop runs 5–30 μm = stepover 0.15–0.30 mm. The Suggest pipe
has no way for the operator to say "I want 10 μm scallop, give me
the stepover."

**Fix:**
- Add `scallop_height: Option<f64>` to `DropCutterConfig` in
  `crates/rs_cam_core/src/compute/operation_configs.rs`. `None` =
  legacy formula-based stepover, preserves baseline.
- When `Some(h)`, the suggest path overrides ae with
  `geometry::scallop_stepover(ball_r, h)`. For tapered ball use
  the *tip* radius (the cusp geometry is determined by the spherical
  tip only).
- Extend `operation_feeds_hints` in `feeds/suggest.rs` to plumb the
  scallop hint for DropCutter the same way it does for Scallop.
- The Feeds modal renders the stepover row as **derived** when
  scallop-mode is active: `"stepover (auto from scallop): 0.18 mm"`
  with a tooltip showing the math `h=10μm, r=0.5mm → ae=0.18mm`.
- Add a "Scallop height" input near the stepover row in the modal —
  when typed, sets scallop_height + clears any manual stepover.

**Files touched:**
- `crates/rs_cam_core/src/compute/operation_configs.rs` — add field
- `crates/rs_cam_core/src/feeds/suggest.rs` —
  `operation_feeds_hints` extension
- `crates/rs_cam_core/src/feeds/suggest.rs` —
  `apply_feeds_result_to_op` may need a guard: if scallop_height set,
  the suggested stepover IS the scallop-derived one (not re-overridden)
- `crates/rs_cam_viz/src/ui/feeds_modal.rs` — UI controls + derived-
  row rendering
- Possibly extend to other 3D finish ops in this round
  (`Waterline`, `SteepShallow`, `RampFinish`, `SpiralFinish`,
  `RadialFinish`, `HorizontalFinish`) if their cusp geometry is
  ball-tip-driven. **Quick triage in S1:** check which of these
  already accept scallop_height (Scallop does); land DropCutter
  first and document the rest as a follow-up.

**Tests:**
- `scallop_height_some_overrides_default_ae_factor` — DropCutter
  with `scallop_height=0.010` on a 1mm ball returns stepover ≈
  `2·√(2·0.5·0.010 − 0.010²) ≈ 0.198 mm`.
- `scallop_height_none_preserves_legacy_ae` — same op without
  the field returns the existing 0.03 factor result. Smoke
  baseline unchanged.
- `scallop_height_uses_tip_radius_for_tapered_ball` — tapered ball
  with tip_radius=0.5 cuts the same cusp curve as a 1mm true ball.

**ETA:** ~3 hours (data structure + suggest plumbing + UI control +
3 tests).

---

### S2 — Engaged-diameter-at-DOC annotation for tapered/V tools

**Problem:** The Feeds modal shows "tool diameter = 6.0 mm" for a
2 mm-tip 7° tapered ball, which is structurally true (shank dia
6 mm) but operationally misleading. At DOC=5 mm the engaged
diameter is `2·5·tan(7°) + 2 mm tip ≈ 3.2 mm`. The cone-shoulder
is doing most of the cutting; the published "1 mm" or "2 mm tip"
spec is irrelevant to engagement.

**Fix:**
- In the Feeds modal header (or a new "Tool geometry at DOC" row
  just below the tool/material line), for `ToolGeometryHint::
  TaperedBall { .. }` and `ToolGeometryHint::VBit { .. }`:
  - Use existing
    `vendor_normalize::lookup_diameter_for_input(input)` (it
    already computes engaged diameter at the operation's DOC for
    LUT lookup).
  - Render: `"Engaged at DOC 5.00 mm: ⌀ 3.20 mm (vs tip ⌀ 1.0 mm)"`
    with a small icon if the engaged diameter exceeds the shank
    or differs from tip by > 50 %.
- For non-tapered tools (Flat, Ball, Bull) hide the row.
- Tooltip explaining "chipload bounds in this modal apply to the
  engaged diameter, not the published tool size — they're computed
  at this DOC."

**Files touched:**
- `crates/rs_cam_viz/src/ui/feeds_modal.rs` — new annotation row
- No core changes — `lookup_diameter_for_input` is already correct;
  this is pure UI surface.

**Tests:** snapshot/visual; rendering only. Optionally a unit test
on a small helper that formats the engaged-diameter string.

**ETA:** ~2 hours.

---

### S3 — Chipload-min warning for finishing ops

**Problem:** The Exceeds-low chipload verdict already fires
correctly in the gates (`ChipSide::Low`). For finishing, this is
the COMMON failure mode (operator slows feed for surface quality
→ rubbing/burning). But the modal doesn't render the warning
prominently for finish ops.

**Fix:**
- In the modal's Chipload row, when the live verdict is
  `Exceeds{ side: Low, .. }`, show a red/amber annotation:
  *"Chipload below LUT minimum — risk of rubbing or burning. Feed
  too slow, RPM too high, or both. Raise feed or drop RPM."*
- Only render this when the operation's pass_role is Finish — for
  roughing the chipload-min warning is less common and the
  diagnostic list already surfaces it.
- Already-existing event surface; this is purely a UI line.

**Files touched:**
- `crates/rs_cam_viz/src/ui/feeds_modal.rs` — conditional warning
  text in the chipload row

**Tests:** None new — the underlying gate verdict already has full
test coverage; this is UI.

**ETA:** ~1 hour.

---

### S4 — "Engaged-diameter chipload" attestation

**Problem:** Operators don't trust the chipload number for tapered
tools because they don't know if it's computed against the tip or
the cone. We actually do the right thing (engaged diameter), but
it's not visible.

**Fix:**
- Below the chipload row in the modal, when
  `ToolGeometryHint::TaperedBall` or `VBit`, render one small
  italic line: *"Chipload computed at engaged diameter ⌀ M mm
  (DOC=N mm), not the tool tip."*
- Builds on S2 — same calculation, different surface.

**Files touched:**
- `crates/rs_cam_viz/src/ui/feeds_modal.rs`

**Tests:** None — pure UI.

**ETA:** ~30 minutes.

---

## Item ordering rationale

| Order | Item | Why |
|---|---|---|
| 1 | S1 | Highest payoff. Operator gets sensible finish stepover by typing scallop height. The thing the user is actively hitting. |
| 2 | S2 | Surfaces the engaged-diameter concept that S4 depends on, and is a frequent footgun on its own. |
| 3 | S3 | Cheap warning that catches a common failure mode. |
| 4 | S4 | Once S2 is in, this is one line — kept for completeness. |

Land them sequentially. Each commit is its own gate (lib tests +
F-037 smoke baseline diff). Default behaviour preserved
throughout — operators who don't set scallop_height get the
existing values.

## What's intentionally NOT in this round

- **Material-aware `ae_factor`** — superseded by scallop-driven
  stepover for the 3D finish case; the few other ops on the
  Parallel/Finish formula are uncommon enough to defer.
- **"Effective chipload along tool length" visualization** —
  needs a dedicated widget (a chart along the cutter geometry).
  This is part of the broader "feeds & speeds as a workspace"
  expansion tracked separately in
  `planning/feeds_workspace_breakout_investigation_2026-06-02.md`.
- **Auto-conversion in the project file format** — `scallop_height`
  field on DropCutter is the only schema addition. No migration
  needed because it's `Option<f64>` with `#[serde(default)]`.

## Exit gate

After each commit:
- `cargo test -p rs_cam_core --lib` clean
- `cargo test -p rs_cam_core --tests` clean (F-037 smoke baseline)
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo run -p rs_cam_cli -- smoke --diff --baseline
  planning/toolpath_acceptance/baselines/2026-06-04.csv` clean

S1 may shift `scallop_height_some_overrides_default_ae_factor`-test
verdicts if any smoke case sets scallop_height — but no smoke case
currently does, so the regression net stays green by construction.
