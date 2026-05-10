# Optimizer / Sim-UX Fixes — captured during wanaka dial-in 2026-05-10

**Trigger:** manual dial-in of `wanaka_full_tuned.toml` (8 toolpaths) via the
MCP after D6/D7 landed. Worked: cycle time 3796s → 2815s (26% faster) with
all gates `Within` on the 5 milling toolpaths. The two drill-cycle ops are
unmodeled by design. TP 3 ("Rivers (back) (copy)") needs operator review —
wrong tool assigned (6 mm flat on a project_curve detail engraving).

This doc captures the friction points hit during that session. Each item is
small enough to land on its own; not a sequenced phase plan.

---

## Design principle (set 2026-05-10)

The app currently treats every per-sample anomaly as a discrete event —
long lists of "issues", clouds of dots on the timeline, top-level
`WARNING: 40% air cutting` strings — and presents them with the same
visual chrome as actual safety risks. The operator sees noise everywhere
and can't tell what's actually dangerous.

**Reframe:** safety and efficiency are different axes. Continuous
time-series data should be rendered as continuous time-series data
(graphs, bands, colored segments) — not as point-issue lists.

**Two-tier point markers, only on the timeline:**

| Tier | What | Render |
|---|---|---|
| **Critical** | Tool / machine / part destruction risk | Red point marker + viewport pin + top-level `ERROR:` |
| **Risky** | Damage-trending: chipload **way** out of bounds, etc | Amber point marker + top-level `WARN:` |

Everything else is a graph: chipload curve with a green band (in-bounds)
and red shading where the curve leaves the band. Power curve, deflection
curve, engagement curve, all the same treatment. Multiple stacked panels
(or toggleable layers).

**Helper text next to each graph:** `chipload 87% time in bounds`,
`power 99.4% time in bounds`, `deflection 100% time in bounds`. The
verdict is derived from those time-in-bounds percentages, not from any
per-sample point flag.

**No timeline dots for:** efficiency hotspots, air-cut segments,
low-engagement segments, in-bounds-but-near-the-edge readings. All of
those become graph color or panel text.

**Critical bar set high:** the chipload graph has bands; only when the
graph spends a meaningful fraction of time outside the band does a
point marker appear. A single 1.05 % overshoot does not warrant a dot —
the colored line segment in the chipload panel is enough.

---

## What's emitted today vs how it renders (current state)

| # | Source | What it actually means | Today's verdict string | Today's timeline | Today's diagnostics panel | Today's color |
|---|---|---|---|---|---|---|
| 1 | `HolderCollision` (cutting move into uncut stock with shank/holder) | **Critical** — tool body hitting wood | `ERROR: N collisions` | none | list | red |
| 2 | `RapidCollision` (G0 traverse clipping uncleared stock) | **Critical** in milling; **false positive** for drill peck cycles (see F3) | `WARNING: N rapid-through-stock` | none | count | yellow |
| 3 | `ChiploadVerdict::Exceeds(High)` | **Critical** — tooth breakage | not in verdict | single dot | verdict card | red |
| 4 | `ChiploadVerdict::Exceeds(Low)` | **Risky if sustained** — burn / edge wear | not in verdict | single dot | verdict card | red |
| 5 | `PowerVerdict::Exceeds` | **Critical** — spindle stall | not in verdict | single dot | verdict card | red |
| 6 | `DeflectionVerdict::Exceeds` (>200 µm) | **Critical** — accuracy loss / breakage on thin tools | not in verdict | single dot | verdict card | red |
| 7 | `DeflectionVerdict` Approximate band (50–200 µm) | **Quality** — surface degradation, not safety | not in verdict | none | "expected degradation" text | yellow text |
| 8 | `SimulationCutIssue::AirCut` (per-segment) | **Efficiency** — time wasted in air | `WARNING: N% air cutting` if >40% | none | count | yellow |
| 9 | `SimulationCutIssue::LowEngagement` (per-segment) | **Efficiency** — under-utilising tool | not in verdict | none | count | yellow |
| 10 | `SimulationCutHotspot` (one per `(toolpath_id, semantic_item_id)`) | **Reporting bucket** — not a flag at all | not in verdict | one dot per bucket (~850 on TP1!) | table | same red as #3-#6 |
| 11 | `narrate` ⚠ on >50 % air-cut | **Efficiency** | not in narrative verdict | n/a | n/a | ⚠ unicode |

Three collision points between safety and efficiency:

- **Timeline:** rows 3–6 (real safety) and row 10 (semantic-item buckets)
  share the same dots. One real signal in 850 reporting buckets.
- **Top-level verdict string:** row 8 ("40 % air cutting") uses
  `WARNING:` — the same prefix as row 2 (rapid collisions).
- **`theme::WARNING` colour (yellow 220/180/60):** stale-result text,
  in-progress dots, rapid collisions, deflection-Approximate bands, and
  air-cut warnings — five very different concerns sharing one colour.

---

## Target categorisation (after these fixes land)

| # | Source | Tier | Where it goes |
|---|---|---|---|
| 1 | HolderCollision | **Critical** | Top-level `ERROR:`, timeline marker, viewport pin |
| 2 | RapidCollision (real, milling-side) | **Critical** | Top-level `ERROR:`, timeline marker, viewport pin |
| 2 (drill peck) | — | — | **Doesn't exist after F3** (root-cause fix) |
| 3 | Chipload Exceeds(High), > 5 % time out | **Critical** | Top-level, timeline marker; chipload graph red-shaded segments |
| 3 | Chipload Exceeds(High), < 5 % time out | **Graph-only** | Chipload graph red-shaded segments + helper "X % time in bounds" |
| 4 | Chipload Exceeds(Low), > 10 % time out | **Risky** | Top-level `WARN:`, timeline marker; chipload graph red-shaded segments |
| 4 | Chipload Exceeds(Low), small | **Graph-only** | Chipload graph red-shaded segments + helper |
| 5 | Power Exceeds | **Critical** | Top-level, timeline marker; power graph red-shaded segments |
| 6 | Deflection > 200 µm | **Critical** | Top-level, timeline marker; deflection graph red-shaded segments |
| 7 | Deflection 50–200 µm | **Graph-only** | Deflection graph yellow band; helper "peak 158 µm" |
| 8 | AirCut % | **Graph helper** | Engagement graph + helper "X % air-cut" |
| 9 | LowEngagement | **Graph helper** | Engagement graph + helper "X % low-engagement" |
| 10 | Hotspot (semantic bucket) | **Diagnostics-panel only** | Sortable table; never a timeline dot |
| 11 | narrate air-cut % | **Graph helper** | Becomes a number in narrative, no ⚠ |

Verdicts derive from **time-in-bounds percentages**, not per-sample flags.
A graph that spends 99.4 % of time inside the green band gets a `Within`
header with `99.4 % time in bounds`. A graph that spends 12 % outside
gets a `Risky` header with `88 % in bounds, 12 % below LUT min`.

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

## F3. Pin Drill peck rapids count as rapid collisions — *root-cause fix*

**Symptom.** Wanaka simulation reports `rapid_collision_count: 18`,
`verdict: "WARNING: rapid collisions detected"`. All 18 collisions are on
TP 0 (Pin Drill) — every 3rd move (the peck retract → next-peck rapid
pattern).

**Root cause (confirmed by code read).**
`compute::simulate.rs:343-352` runs `check_rapid_collisions_against_stock`
once per toolpath, against the dexel grid as it stands **before** the
toolpath's first cut. Within that toolpath the dexel snapshot is *not*
updated as feed/cut moves complete — the collision check walks every
rapid against a static pre-toolpath snapshot.

For the drill peck pattern in `drill::drill_peck_full_retract`:

1. G1 feed down to `target_z` — dexel update would clear the column,
   but the snapshot is frozen.
2. G0 retract up to `retract_z` — same XY, dz>0; carve-out at
   `collision.rs:322` skips it.
3. G0 rapid back down to `target_z + 0.5 mm` (re-entry above the
   freshly-cut floor) — same XY, **dz<0**, NOT skipped. The snapshot
   still shows full stock in that column, so `pz < stock_top` fires.

Every peck after the first emits one false-positive rapid collision.

**This is a real analysis bug, not a UI carve-out.** Two options to
fix it for real:

**Option A — incremental dexel update during collision check.** Walk
the toolpath move-by-move in `check_rapid_collisions_against_stock`,
updating the local dexel snapshot for feed/cut moves before checking
each rapid against the latest state. Cost: an order of magnitude more
dexel mutations per collision check; needs the same dexel-mutate
helpers the simulator already uses.

**Option B — same-XY-descent carve-out keyed off prior-move depth.**
Extend the existing carve-out at `collision.rs:322`. Skip a rapid
when (a) it's pure-Z (xy_dist_sq < 0.01), (b) the previous move
ended at the same XY with a Z below the rapid's end Z, and (c) the
previous move was a feed (= cut, = cleared the column). Catches the
drill peck case AND any milling op with a feed-down → rapid-up →
rapid-back-down-to-just-above-floor pattern. Cheap, surgical, no
collision-check restructuring.

**Recommended:** Option B for the immediate fix. Option A as a
follow-up if other false-positive patterns surface (e.g. lift functions
that bridge across multiple just-cut columns).

**Acceptance.** Wanaka project sim verdict drops to `OK`;
`rapid_collision_count: 0`. The existing collision tests in
`collision.rs:580+` continue to pass; new test added covering the
drill peck pattern.

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
       "observed": 0.0011, "bound": 0.0305,
       "time_in_bounds_pct": 8.4}
    ]
  },
  "load_report": { ... existing ... }
}
```

`time_in_bounds_pct` ties into the F6 reframe — verdicts come from
time-in-bounds percentages, so the header surfaces the same number.

**Acceptance.** A single read of the header tells me how many TPs need
attention and which.

---

## F6. Move continuous data off the dot channel onto graphs (the big one)

**Symptom (reported by user during dial-in).** "Everything looks like
'danger' everywhere. Right now it just looks like every toolpath is
broken all the time."

**Root cause.** `sim_timeline.rs:285-309` plots one dot per entry in
`SimulationCutTrace.hotspots`. But hotspots are per-`(toolpath_id,
semantic_item_id)` aggregator buckets, not problem flags. ~850 dots on
wanaka TP 1.

Compounded: even the legitimate gate-trip dots (one per `Exceeds`
verdict) sit in the same dot channel as the buckets and the same colour
as the unrelated rapid-collision warnings, and the same chrome as
deflection-Approximate bands and air-cut warnings.

**Reframe (per design principle above).** Drop most dots. Show
continuous data as continuous data.

### F6.1 — Remove all `SimulationCutHotspot` dots from the timeline

The hotspot bucket structure is correct as a data model — the
diagnostics panel uses it for the per-semantic-item table. The renderer
should not turn it into a marker channel. Render zero hotspot dots on
the timeline. The diagnostics panel keeps the sortable table.

### F6.2 — Per-criterion graph panels with bands and out-of-bounds shading

Replace the single timeline track with a stack of stacked panels (or
toggleable overlays) — one per criterion. For each panel:

- **Time-series line** of the criterion's value (chipload, power, tip
  deflection, engagement, etc.).
- **Green band** for the in-bounds region (chipload: between LUT min
  and max; power: 0 to available; deflection: 0 to 50 µm).
- **Yellow band** for the soft warning region (deflection 50–200 µm;
  chipload between LUT min and 1.1× LUT min).
- **Red shading on the line itself** wherever the value leaves the
  green/yellow band. The shading replaces today's "single dot at
  worst sample" — operator sees *every* out-of-bounds segment, not
  just the peak one.
- **Helper text** in the panel header: `chipload — 87.4 % time in
  bounds (0.020 mm/tooth median, LUT min 0.032)`. Verdict semantic
  comes from the helper text, not from a separate verdict card.

### F6.3 — Critical-only point markers

Top-level timeline gets point markers ONLY for:

- `HolderCollision` (always Critical)
- `RapidCollision` (always Critical, after F3 false-positives are gone)
- `Chipload Exceeds(High)` AND time-out-of-bounds > 5 %
- `Power Exceeds` AND time-out-of-bounds > 1 % (power exceedance is
  always serious)
- `Deflection Exceeds` (>200 µm) AND time-out-of-bounds > 1 %

Point markers are red. No amber dots — risky readings show up as
red-shaded line segments in their respective graph panels, not as
dots.

### F6.4 — Top-level verdict string follows the same tier

- `ERROR:` only for Critical (rows 1, 2-real, 3-many-out, 5, 6).
- `WARN:` only for Risky (rows 4, 3-few-out, 7).
- Drop air-cut % from the top-level verdict string entirely. It moves
  to the engagement-graph panel header.

**Acceptance.** Wanaka TP 1 timeline shows 0 dots (all three rough TPs
are `Within` after the dial-in; the chipload graph shows the curve
nestled in the green band with a helper "100 % time in bounds"). For a
genuinely-failing toolpath, the operator sees a red line segment in the
chipload graph at the failing region, plus a single Critical dot at the
worst spot, plus a helper "73 % time in bounds, peak 0.071 over 0.055".

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

| ID | Surface | Impact | Effort |
|----|---------|--------|--------|
| F1 | optimizer search | Wanaka-class fixtures get auto-fix instead of NoImp | medium |
| F2 | MCP router | Unblocks scripted RPM tuning | small |
| F3 | rapid-collision detector | **Root-cause fix** for drill peck false positives | medium |
| F4 | narrate / diag | Drill ops stop reporting 100 % air-cut as ⚠ | small |
| F5 | report shape | First-look summary of project safety | small |
| F6 | sim timeline + graphs | The signal-to-noise reframe — graphs not dots | large |
| F7 | narrative | Cheap UX band-aid until F1 ships | small |

Highest user-felt: **F6** (the orange-dots fix the user explicitly called
out — biggest visual change in the app) and **F1** (most impactful for
the optimization workflow itself). **F3** is also load-bearing because
it removes the project-level `WARNING:` that distracts from real signal
on every drill-bearing project.

---

## Notes / out-of-scope here

- TP 3 ("Rivers (back) (copy)") is a project-data hygiene issue, not a
  tool-load bug. The wrong tool is assigned (6 mm flat on engraving
  detail). No safe automated fix exists; this is operator review.
- D8 (cross-fixture validation from `STRUCTURAL_ENTRY_SPANS_AND_LOCALITY.md`)
  is still pending — this dial-in covered wanaka but not the other
  test fixtures. F1 should ship before D8 to avoid spurious
  `NoSafeImprovement` results in the cross-fixture report.
- F6 is a wide UI change. Suggest splitting into F6.1 (remove hotspot
  dots — quick win, ~1h) and F6.2-F6.4 (graph panels + helpers + tier
  reframe — proper design pass, several days). F6.1 can ship
  immediately as a noise-floor fix; the rest is the permanent answer.
