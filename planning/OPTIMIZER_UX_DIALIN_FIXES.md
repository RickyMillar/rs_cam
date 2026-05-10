# Optimizer / Sim-UX Fixes — captured during wanaka dial-in 2026-05-10

**Trigger:** manual dial-in of `wanaka_full_tuned.toml` (8 toolpaths) via the
MCP after D6/D7 landed. Worked: cycle time 3796s → 2815s (26% faster) with
all gates `Within` on the 5 milling toolpaths. The two drill-cycle ops are
unmodeled by design. TP 3 ("Rivers (back) (copy)") needs operator review —
wrong tool assigned (6 mm flat on a project_curve detail engraving).

This doc captures the friction points hit during that session. Each item is
small enough to land on its own; not a sequenced phase plan.

---

## F1. Optimizer doesn't sweep `spindle_rpm`

**Symptom.** Optimizer returned `NoSafeImprovement` on TP 1 / TP 6 (Back
Rough, 3D Rough 6) with the envelope showing `spindle_rpm: { min: 18000,
max: 18000 }` — RPM held fixed across all candidate stages.

**Why it matters.** Burn-side trips (`Exceeds(Low)` chipload — sample
median below LUT min) have two fix levers: raise feed, or *lower* RPM.
When feed is at machine cap (4000 mm/min on the wanaka spindle), the only
remaining lever is RPM down. The manual fix that brought TP 1 + TP 6 to
`Within` was RPM 18000 → 14000. The optimizer never tried it.

**Stage 0 doesn't help here.** Stage 0's analytical scaling holds
chipload constant (raises both RPM and feed proportionally). When feed is
already at the cap, scale-up is rejected immediately. Nothing in Stage 1/2
sweeps RPM downward.

**Proposal.** When the baseline verdict is `Exceeds(Low)` chipload AND
feed is at or near `max_feed_mm_min`, add a Stage 1.5 sweep of `spindle_rpm`
downward (toward `min_rpm`) at fixed feed. Step size: target chipload at
1.1× LUT min. Cap RPM reduction at the spindle's `min_rpm`. Honour all
other gates.

**Acceptance.** Re-run the wanaka MCP smoke; TP 1 + TP 6 should report
`Ranked` with the RPM-down candidate, not `NoSafeImprovement`. The
candidate's gate_deltas should show `chipload: improved`.

---

## F2. MCP `set_toolpath_param` rejects `spindle_rpm`

**Symptom.** `set_toolpath_param(index=1, param="spindle_rpm", value=14000)`
errors: `Invalid parameter: spindle_rpm must be a u32 or null`. Also fails
for `13500.0` (float) and `"13500"` (string). Forced workaround:
`save_project` → `sed` → `load_project`.

**Cause (suspected).** The MCP value channel arrives as `serde_json::Value`,
which deserializes integers >= certain magnitude as `f64`. The
`spindle_rpm` field's deserializer is strict `Option<u32>` — no f64→u32
coercion.

**Fix.** In the MCP toolpath-param router, coerce `Number` values to
`u32` for integer-typed params (`spindle_rpm`, `flute_count`, etc.) when
the value is finite, non-negative, and integer-valued (`f.fract() == 0.0`).
Reject only on actual loss-of-precision or out-of-range.

**Acceptance.** `set_toolpath_param(1, "spindle_rpm", 14000)` succeeds
without quote/decimal gymnastics.

---

## F3. Pin Drill peck rapids count as rapid collisions

**Symptom.** Wanaka simulation reports `rapid_collision_count: 18`,
`verdict: "WARNING: rapid collisions detected"`. All 18 collisions are on
TP 0 (Pin Drill) — every 3rd move (the peck retract → next-peck rapid
pattern). The "uncleared stock" the rapid is moving into IS the hole the
drill just made.

**Why it matters.** Looks like a serious safety warning at the project
verdict level. Operator reads "rapid collisions detected" → opens
collision inspector → sees all 18 on the drill peck → realises it's noise.
Same dance every time the project loads.

**Fix.** In the rapid-collision detector, carve out moves whose owning
toolpath is `OperationType::{Drill, AlignmentPinDrill}`. Drill peck
cycles are by definition rapid moves between successive peck depths in
the just-drilled hole; the dexel still sees uncleared stock around the
hole bore but the rapid stays inside the cleared cylinder.

Alternative: tag drill cycles' rapid moves at generation time with a
`SkipCollisionCheck` flag the simulator honours. More general but a
wider change.

**Acceptance.** Wanaka project sim verdict drops to `OK` (or the next
real warning); `rapid_collision_count: 0`.

---

## F4. Drill ops report 100% air-cut

**Symptom.** TP 2 (Holes drill) narrate output: "100.0% of cutting time
is air-cut; average engagement 0.000". The drill is in fact cutting wood —
the engagement model just doesn't see Z-only chip making (radial_engagement
based on cylinder side-engagement is always ~0 on a vertical descent).

**Why it matters.** Same false-alarm pattern as F3. The narrate output
flags this as ⚠ which surfaces as actionable when it's a model
limitation.

**Fix (small).** When `OperationType::{Drill, AlignmentPinDrill}` is the
operation kind, replace the air-cut % with `Unmodeled — drill cycle
(engagement model is XY-only)` in the narrate output and the diagnostics
panel.

**Fix (correct).** Implement Z-axis engagement for drill ops in the
dexel sampler — count the cell volume swept axially per sample. Bigger
change; punt unless drill chipload becomes a real gate concern.

**Acceptance.** `narrate_toolpath` on a drill op doesn't surface "100%
air-cut" as ⚠; instead a one-liner notes the model limitation.

---

## F5. `get_tool_load_report` lacks a project header

**Symptom.** To audit which toolpaths have problems I had to scan all 8
verdict blocks manually. The first natural action on opening any project
is "what's broken?" — and the JSON forces a per-toolpath read pass.

**Fix.** Prepend a header object to the report:

```json
{
  "header": {
    "total_toolpaths": 8,
    "modeled": 6,
    "unmodeled": 2,
    "within": 5,
    "exceeds": 1,
    "exceeds_breakdown": [
      {"toolpath_id": 12, "name": "Rivers (back) (copy)",
       "gate": "chipload", "side": "low",
       "observed": 0.0011, "bound": 0.0305}
    ]
  },
  "load_report": { ... existing ... }
}
```

**Acceptance.** A single read of the header tells me how many TPs need
attention and which.

---

## F6. Hotspot dots on the sim timeline overload the warning channel

**Symptom (reported by user during dial-in).** "Everything looks like
'danger' everywhere. Right now it just looks like every toolpath is
broken all the time."

**Cause.** `sim_timeline.rs:285-309` plots one dot per entry in
`SimulationCutTrace.hotspots`. But hotspots are computed by
`SimulationCutTrace::summarize` (`simulation_cut.rs:317`) as **one
aggregator per `(toolpath_id, semantic_item_id)` bucket** — a reporting
fold over every semantic span the operation emitted, not a flag for
problem locations. On wanaka TP 1 that's ~850 dots on a single timeline
track. The single *meaningful* dot — the gate-trip marker appended at
`sim_timeline.rs:322` — is one in 851 and indistinguishable from the
rest.

**Concretely:** the `SimulationCutHotspot` struct carries `wasted_runtime_s`
and `air_cut_time_s` fields that already encode "how bad" — but the
renderer doesn't use them to filter or color-code.

**Proposal — three-tier severity, drop the noise floor.**

1. **Compute severity at trace-summary time:**
   - `Severity::GateTrip` — the explicit pins appended at line 322
     (one per `Exceeds` verdict). Always rendered, red.
   - `Severity::WastedTime` — hotspot whose `wasted_runtime_s / total_runtime_s
     > 0.20` (default tunable). Amber. These are the per-semantic-item
     entries that are actually inefficient.
   - `Severity::Info` — everything else. Default off. Optional toggle in
     a "show all hotspots" debug menu.

2. **Render only `GateTrip + WastedTime` in the timeline** by default.
   Color-code by severity (red / amber).

3. **Add a count badge** to the diagnostics panel:
   `2 gate trips · 7 wasted-time hotspots · 841 info hotspots (hidden)`.

**Why this is the right shape.** The aggregator-per-semantic-item bucket
is correct as a data structure (the diagnostics panel already uses it
for the Selected-section table). The fix is purely on the *rendering*
side: only show buckets the operator should act on. Today's "every
bucket gets a dot" violates the signal-to-noise principle from the
design doc (`adaptive_review_2026-04.md`).

**Acceptance.** Wanaka TP 1 timeline shows ~1-3 dots (the
`Exceeds(Low)` chipload trip + any actually-wasted-time region) instead
of ~850. The diagnostics panel still lists all hotspots in its table —
the timeline is just curated.

---

## F7. (Stretch) Surface the optimizer's RPM-axis gap to the operator

If F1 isn't fixed immediately, the narrative should at least call out
the constraint:

> "Tried 3 candidates at the current spindle RPM (18000). Lowering RPM
> may admit more chipload-side headroom — try 14000-15000 RPM and
> re-optimize."

Cheap user-facing band-aid until F1 lands. Could be a heuristic
suggestion in `optimize::narrative` that fires when `Exceeds(Low)
chipload` AND feed at machine cap.

---

## Index of fixes by impact

| ID  | Surface          | Impact                                              | Effort |
|-----|------------------|-----------------------------------------------------|--------|
| F1  | optimizer search | Wanaka-class fixtures get auto-fix instead of NoImp | medium |
| F2  | MCP router       | Unblocks scripted RPM tuning                        | small  |
| F3  | sim diagnostics  | Removes 18 false-positive collision warnings        | small  |
| F4  | narrate / diag   | Removes "100% air-cut" false alarm on drills        | small  |
| F5  | report shape     | First-look summary of project safety                | small  |
| F6  | sim timeline     | The signal-to-noise fix the operator asked for      | medium |
| F7  | narrative        | Cheap UX band-aid until F1 ships                    | small  |

Highest user-felt: **F6** (the orange-dots fix the user explicitly called
out) and **F1** (most impactful for the actual optimization workflow).

---

## Notes / out-of-scope here

- TP 3 ("Rivers (back) (copy)") is a project-data hygiene issue, not a
  tool-load bug. The wrong tool is assigned (6 mm flat on engraving
  detail). No safe automated fix exists; this is operator review.
- D8 (cross-fixture validation from `STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`)
  is still pending — this dial-in covered wanaka but not the other
  test fixtures. F1 should ship before D8 to avoid spurious
  `NoSafeImprovement` results in the cross-fixture report.
