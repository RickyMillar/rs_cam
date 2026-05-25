# F-032 — Scallop deflection gate counts transit/entry samples (same shape as F-031, different op)

- **Stage:** sim (tool-load gate evaluation)
- **Severity:** medium (last remaining deflection-bar miss after F-031;
  closes the bar to 7/7)
- **Status:** open — **finding hypothesis refuted by round-10 implementer probe**
  (2026-05-26); root cause is NOT a transit-sample filter gap. Real
  triggering sample is a steady-state `FinishingCut` (Helix kinematics,
  radial=0.75, axial=17.77 mm on a 15 mm-flute 3 mm ball nose) inside
  fresh uncut stock above terrain. AS015 runs scallop solo without a
  prior roughing pass; the dexel grid reports the full uncut stock
  height above the cutter as engagement. See "Round-10 implementer
  reframe" section below.
- **First found in:** round-09 (2026-05-26)
- **Effort:** S (likely identical fix shape to F-031's transit-sample
  filter; just needs to extend to the scallop op's deflection sample
  selection)
- **Linked PRs:** —
- **Source audits:** round-09 MCP smoke on AS015 + hotspot cut_trace

## Evidence

AS015 scallop on `ux_3d_terrain.toml` (Ball Nose 3mm, hardwood,
scallop_height=0.03, feed=800, plunge=250):

| Metric | Round-08 | Round-09 (post-F-031) |
|---|---:|---:|
| `deflection.peak_mm` | 0.434 Exceeds | **0.434 Exceeds** (byte-identical) |
| `chipload.observed_mm_per_tooth` | 0.002 (median_low) | 0.002 (median_low) |
| `power.peak_kw` | 0.038 Within | 0.038 Within |
| `rapid_collision_count` | 0 | 0 |

F-031 did NOT move AS015 because F-031's fix was narrowly scoped to
`DressupConfig::for_op(Adaptive3d)` — the `prefer_helix` override
that desynced planner stamping. Scallop uses a different op-config
path with no analogous helix-entry-style dressup; the F-031 fix
has no path to scallop.

## Why the 0.434 mm reading is impossible for steady-state cutting

The diagnostics are **physically inconsistent**:
- `chipload` observed = 0.002 mm/tooth → cutter is barely engaging
  material (chip-thinning at narrow WOC; vendor LUT band is
  0.0125–0.020).
- `power.peak_kw` = 0.038 kW → tiny cutting force overall.
- `deflection.peak_mm` = 0.434 mm → claims ~5× larger tip deflection
  than a 6mm endmill running heavy hardwood pocket-rough at 2 mm DOC
  produces (AS001 = 0.076 mm).

A 3 mm ball nose with 2 µm chip thickness cannot generate 434 µm tip
deflection. The deflection gate is reading a sample that doesn't
represent steady-state.

## Hotspot cut_trace evidence

Top 2 hotspots from `get_cut_trace(toolpath_id=1)` on AS015 round-09:

| Hotspot | move range | sample range | peak_axial_doc_mm | avg engagement | rep_position |
|---|---|---|---:|---:|---|
| 1 | 17436-17591 | 40071-40653 | **25.68 mm** | 0.013 | (36.46, 36.46, 9.81) |
| 2 | 3875-4387 | 9581-10857 | **26.67 mm** | 0.051 | (24.24, 5.51, 8.14) |

A 3 mm ball nose **cannot** cut 25-26 mm axial DOC. Average engagement
< 5% confirms these samples are transit/entry, not steady-state.

The deflection-triggering sample for the gate (round-09 reports
`evidence.sample_range.{start,end}` = 5053-5054, near hotspot 2's
range 9581-10857) is one of these inflated-axial transit samples.

## Hypothesis

Same shape as F-031 but on a different layer. F-031's commit
`497a3b2` added a `!in_transit_span` filter to the F-027 acceptance
test (`adaptive3d_planner_stock_xy_f027.rs`) to mirror the deflection
model's `is_steady_state_for_gate`. But that filter is applied in the
**test** to assert post-fix behaviour; the deflection **gate's own
sample selection** must also honor the same predicate.

For scallop, the gate's sample selection appears to **not** filter
transit samples, so the inflated-axial entry samples are passed
through to the deflection model.

Mechanism: scallop's plunge entries (Z descent before lateral cuts)
produce samples where `axial_engagement_mm` reads the cutter's
descent distance (25+ mm) rather than the radial cut depth. The
deflection model interprets that as a heavy axial load and over-
estimates tip deflection.

## Fix shape

Mirror F-031's `is_steady_state_for_gate` semantics into the
deflection sample-selection path for the scallop op (and likely the
other 3D finishing ops: drop_cutter, waterline, scallop, pencil,
horizontal_finish, project_curve — these all use plunge entries).

Candidate fix sites:
- `crates/rs_cam_core/src/tool_load/deflection.rs` — if the gate
  applies sample filtering itself
- `crates/rs_cam_core/src/dexel_stock/` — if the `is_transit_span` tag
  is set at stamp time
- `crates/rs_cam_core/src/scallop/` (or equivalent) — if scallop
  tags its entry/exit moves differently from `Cut` segments

Read F-031's commit `497a3b2` and the test changes in
`adaptive3d_planner_stock_xy_f027.rs` to find the exact filter
predicate; extend it to the gate's sample loop.

## Acceptance test

1. Load `test_data/ux_3d_terrain.toml` via `ProjectSession`.
2. Add scallop op on model_id=1 with baseline params (scallop_height=0.03,
   feed=800, plunge=250, Ball Nose 3mm tool).
3. Generate + simulate.
4. Assert `deflection.peak_mm < 0.2` (gate Within).
5. Defensive assertion: `chipload.peak < 0.05` AND `power.peak_kw < 0.1`
   → if both small, deflection must also be small (physical consistency).
6. Run through `ProjectSession::run_simulation` per the production-
   entry-point rule.

Pre-fix this test fails at deflection=0.434. Post-fix should pass at
deflection ≈ 0.1-0.15 (similar order to AS013 post-F-031).

## Risk

S. F-031's fix shape is known; F-032 likely just extends the same
filter to a sibling op. Watch for: other 3D finishing ops may need
the same filter (drop_cutter, waterline, pencil, horizontal_finish,
project_curve) — but those aren't in the current acceptance suite,
so include them as a defensive extension if the fix site is shared.

## Notes

- **Cross-link**: F-031's `is_steady_state_for_gate` semantics are
  the canonical reference. The F-031 implementer noted in
  `74d8a7f`/`497a3b2` commit bodies that the filter was added to
  `adaptive3d_planner_stock_xy_f027.rs` to mirror the deflection
  model's predicate. F-032's job is to make sure the predicate fires
  for scallop too.
- **Independence from F-030**: F-030's `SetupEvalContext` refactor is
  about frame/bbox unification, orthogonal to sample-selection in
  the gate. F-030 stands; F-032 is a separate fix.
- **AS013 byte-identical 6.4× toolpath shrink**: round-09 AS013 went
  from 420k → 65k moves. F-031's removal of the spurious helix-entry
  rewrite is the cause. Worth confirming this doesn't surface as a
  generation correctness issue in round-10's deeper sweep.
- After F-032 lands and verifies, the deflection bar should reach
  **7/7 Within**. F-017 (rapid collisions) can also close in the
  same round if smoke confirms zero collisions on AS013/AS015.

## Round-10 implementer reframe (2026-05-26) — original hypothesis refuted

Per the implementer-contract "stop if > 2× S effort" rule, the
round-10 implementer (Claude Opus 4.7) reverted speculative changes
and flagged this finding for re-audit. Findings below from a
diagnostic probe driving the AS015 case through
`ProjectSession::run_simulation`:

**Real triggering sample (not a plunge / transit / entry):**

```
evidence.sample_range = 5052..5053
move_idx=2016, sample_idx=5052
position = (97.37, 67.61, 14.59)    ← cutter at terrain height
kinematics = Helix                    ← XY+Z lateral move, NOT plunge
move_type = Linear { feed_rate: 800 }  ← finishing-cut feed
intent = FinishingCut                  ← NOT EntryPlunge/Helix/Ramp
radial_woc_fraction = 0.750            ← 75% engagement, real cut
arc = 2.094 rad (~120°)
axial_engagement_mm = 17.77            ← > flute length (15 mm)!
chipload = 0.022 mm/tooth              ← matches vendor LUT
in_transit_span = false                ← genuine steady-state
span_path = [Operation, Region("Ring 4")]  ← no Entry ancestor
```

The deflection model's predicate `is_steady_state_for_gate` filters
on `SpanKind::Entry` / `SpanKind::WaterlineCleanup` span ancestry —
this sample has neither, and **correctly** classifies as
steady-state by every signal F-031 introduced.

**Actual root cause: dexel reports `axial_engagement_mm > flute_length`
because AS015 runs scallop solo on fresh uncut stock.**

`test_data/ux_3d_terrain.toml` has zero existing toolpaths; AS015 adds
a scallop pass as the *only* operation. Stock auto-grows to ~57.6 mm
to enclose terrain. Terrain peaks sit around Z ≈ 14-16; bulk stock
above Z=14 is never roughed away because no prior roughing op runs.
When the scallop cutter follows terrain at Z=14.59, the dexel ray at
that XY position reads the height of uncut bulk stock material above
the cutter tip — **regardless of whether the cutter can physically
engage it via its flute**. The 3 mm ball nose has 15 mm flute; the
dexel reports 17.77 mm. Material from Z=14.59+15 = 29.59 up to
~57.6 contacts only the inert shank, not the cutting flutes.

The deflection formula
`F = Kc × axial_engagement_mm × radial_width_mm` then over-estimates
the force by counting the shank-contact extent as if it were
flute-cutting material.

**A second cohort of even more inflated samples** sits at positions
like (0.0, 12.13, 0.0) / (0.0, 9.15, 0.0) — X = 0 and Z = 0,
clamped-min sentinels emitted by `scallop::ring_to_3d` for points
outside the mesh footprint. Top axial values reach ~47 mm
(near-full-stock-height) in these samples. They're not transit
samples either (in_transit_span = false, span_path = [Operation,
Region]). This is a **separate scallop generator bug**: ring points
outside the terrain mesh footprint should not be emitted at clamped
Z = 0; they should be culled or lifted to safe_z. Treating "outside
mesh footprint" as "low terrain" is unsafe.

**Speculative fixes the implementer probed and reverted:**

1. **Synthesise `SpanKind::Entry` spans from `MoveIntent::Entry*`
   moves** at `compute::execute::generated_with_spans`. Correctly
   wraps every op's EntryPlunge/EntryHelix/EntryRamp transitions in
   Entry-ancestry spans, so `is_steady_state_for_gate` filters them
   the way F-031 set up for adaptive3d via `PassEntry`. **Did not
   move AS015** because the triggering sample is `FinishingCut`,
   not an entry intent. **Verdict: a useful F-031 hardening for
   other 3D-finishing ops, but not the F-032 fix.**

2. **Clamp `axial_engagement_mm` to `tool.length()` in
   `deflection::sample_tip_deflection_mm`** — physically correct
   (force only accrues on the engaged flute, not on the shank). Moved
   AS015 from 0.434 to 0.424 mm — trivial — because the residual
   17.77→15 clamp on a 3 mm ball nose flute leaves the dominant
   force component intact, and the second cohort (axial ≈ 47 mm at
   Z=0 sentinels) becomes the new peak. **Verdict: a real model
   improvement, but does not close the 0.2 mm gate by itself.**

**Candidate fix shapes (re-audit needed before any of these land):**

- **(A) Test-data fix**: add a roughing toolpath to AS015's
  `ux_3d_terrain.toml` (or a scallop-test-only template) so the
  dexel above-terrain bulk material is cleared before scallop runs.
  Smallest blast radius but reframes AS015 as a multi-op smoke case.
- **(B) Scallop ring-emission fix**: cull or safe-Z-lift ring points
  outside the mesh footprint instead of clamping Z = 0. This kills
  the (0, *, 0) cohort.
- **(C) Engagement-model fix**: clamp `axial_engagement_mm` to
  `tool.length()` at the **dexel measurement** layer (not just the
  deflection gate), so every downstream consumer (chipload, power,
  hotspots, narration) reads the physically-engageable height.
  Architecturally cleanest; risk of fingerprint regen across many
  ops; needs a careful sweep.
- **(D) Op-aware deflection model**: detect when a 3D finishing op
  reports `axial_engagement_mm >> commanded_scallop_step` and
  surface a workflow advisory ("looks like no prior roughing on
  this finish pass") rather than a deflection-gate trip. Useful
  for narration; doesn't address the underlying model error.

**Effort estimate post-reframe**: M (not S). Likely needs (B) + (C)
together, or a test-data restructure (A) with (B) as a defensive
generator hardening. (D) is product-narration work, separate.

**Implementer artefact**: see git log; the implementer reverted all
speculative source changes and removed the speculative
`tests/scallop_deflection_transit_filter_f032.rs` acceptance test +
the `tests/probe_f032.rs` diagnostic probe (both lived only in the
implementer's working tree). The acceptance bar from the original
F-032 finding (`deflection.peak_mm < 0.2` on AS015 + the mutual-
consistency cross-check) is still the right bar; it just doesn't
fall out of an F-031 predicate extension.
</parameter>
</invoke>