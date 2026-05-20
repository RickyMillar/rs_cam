# UX Dial-in Review — 2026-05-20

**Method**: live MCP session against the GUI. For each toolpath: accept suggested
(LUT-derived) parameters as baseline, generate, simulate, verify "did the cut do
what the op promised" via `narrate_toolpath`, pull `get_diagnostics` +
`get_tool_load_report` + drill-specific signals, then tune one dimension at a
time until a gate flips or a symptom appears.

**Severity scale**: 🔴 Critical (blocks correct result / hides safety info) ·
🟡 Important (friction or confusion) · 🟢 Polish.

**Source-code edits**: none — observation only.

---

## Phase A — Wanaka full pass

Loaded project: Wanaka (8 toolpaths across 2 setups).

| # | Name | Operation | Tool |
|---|------|-----------|------|
| 0 | Pin Drill | Pin Drill | End Mill 6 mm |
| 1 | Back Rough | 3D Rough (adaptive3d) | End Mill 6 mm |
| 2 | Holes | Drill | End Mill 6 mm |
| 3 | Rivers (back) (copy) | Project Curve | End Mill 6 mm |
| 4 | Rivers (back) | Project Curve | Tapered Ball 2 mm |
| 5 | Lakes (back, inside) | Project Curve | Tapered Ball 2 mm |
| 6 | 3D Rough 6 | 3D Rough (adaptive3d) | End Mill 6 mm |
| 7 | 3D Finish 6 | 3D Finish | Tapered Ball 2 mm |

### Session-limitation note

The MCP server is running an older binary that predates the P1–P5 priority work
shipped on 2026-05-19:

- `project_summary` does not return `stale_defaults` (P5).
- `get_tool_load_report` has no top-level summary header, only `per_toolpath`
  (the F5 "within / exceeds / fully_unmodeled" header is missing).
- `drill_gates` is `None` everywhere — Step 3 PR2 not wired into this build.
- The project-level verdict still emits `"WARNING: high air cutting"` with no
  offender names (P1 was supposed to either drop the warning or name TPs).

This is itself a finding: **the build cadence makes it easy for the user (and
me) to think a fix landed when the running GUI hasn't been rebuilt**. Logged as
🟡 [P0 surface, build/launch] below.

### Baselines (suggested LUT params, as-loaded)

| # | Name | Op | Key params | Notable |
|---|------|----|------------|---------|
| 0 | Pin Drill | alignment_pin_drill | feed 300, peck 3.0 mm, spoilboard_penetration 2 | Sane |
| 1 | Back Rough | adaptive3d (agent_search) | DOC 4.83, stepover 2.53 (42% D), feed 4000, plunge 750, RPM 12194 | Feed pinned at machine max (4000) |
| 2 | Holes | drill (peck) | depth 12, peck 3, feed 300 | Sane |
| 3 | Rivers (back) (copy) | project_curve | depth **-2.0**, from_below, 6 mm EM, feed 800 | **Cuts above stock — 100% air** |
| 4 | Rivers (back) | project_curve | depth 0.2, from_below, 1 mm TB, feed 1500, plunge 400 | Chipload-low exceeds |
| 5 | Lakes (back, inside) | project_curve | depth 0.2, from_below, 1 mm TB, feed 1500 | Chipload-low exceeds |
| 6 | 3D Rough 6 | adaptive3d | DOC 2.6, stepover 2.2 (37% D), feed 4000, plunge 500, RPM 12194 | Healthy engagement |
| 7 | 3D Finish 6 | drop_cutter | **min_z -50.0**, stepover 0.30, feed 2148, **plunge 750** on 1 mm TB | Two stale defaults + chipload-low exceeds |

### Tool-load report — verdict matrix

| TP | Chipload | Power | Deflection | Drill gates |
|----|----------|-------|------------|-------------|
| 0 Pin Drill | unmodeled | unmodeled | unmodeled | **None** (expected populated post-P3) |
| 1 Back Rough | within | within | within | n/a |
| 2 Holes | unmodeled | unmodeled | unmodeled | **None** |
| 3 Rivers copy | unmodeled | unmodeled | unmodeled | n/a |
| 4 Rivers | **exceeds (low)** | within | within | n/a |
| 5 Lakes | **exceeds (low)** | within | within | n/a |
| 6 3D Rough 6 | within | within | within | n/a |
| 7 3D Finish 6 | **exceeds (low)** | within | within | n/a |

All three "exceeds" are `side: low` (chipload too small → rubbing risk), all on
the 1 mm tapered ball, all from `vendor_lut_extrapolated` with "diameter scale
×0.35–0.45". The LUT row is calibrated at d=3.175 mm; the tool diameter at the
tip is 1 mm or 2 mm (see naming/diameter inconsistency below).

### Project-wide sim verdict

```
verdict          : "WARNING: high air cutting"
air_cut_%        : 65.75
average_engagement: 0.094
collision_count  : 0
rapid_collision  : 0
hotspots         : 1105
issue_count      : 21671      ← noise; F6 reframe hasn't fully landed in this build
total_runtime_s  : 4130.7     (≈ 69 min)
```

### Findings — Phase A

#### Critical

🔴 **[A, project_curve geometry]** TP3 "Rivers (back) (copy)" produces 100%
air-cut with no surfacing — the user set `depth: -2.0, direction: from_below`
intending a 2 mm deep cut, but the sign convention puts the path 2 mm *above*
the surface so all 1992 moves cut nothing. Sim runs to completion, no banner,
no in-graph anomaly. _Where_: `crates/rs_cam_core/src/operations/project_curve.rs`
sign of `depth` × `direction`. _Fix idea_: when generated cut Z is entirely above
stock-top for the whole toolpath, raise a span-emitter-level diagnostic
("toolpath generated 0 mm of in-material cut") before sim even runs.

🔴 **[A, drop_cutter defaults]** TP7 "3D Finish 6" has `min_z: -50.0` (pre-B1
stale) **and** `plunge_rate: 750` on a 1 mm tapered ball (pre-fix2 stale). The
P5 validator was built for exactly this — but the running binary doesn't expose
the validator output, so neither MCP nor GUI flags it. _Where_:
`crates/rs_cam_core/src/compute/validate.rs` rules `drop_cutter_min_z_pre_b1`
and `tapered_ball_plunge_pre_fix2` exist but are unreached from the live MCP.
_Fix idea_: rebuild the MCP-enabled GUI from HEAD and confirm
`project_summary.stale_defaults` populates; if it does, this collapses to a
build-cadence finding.

🟡 **[A, build cadence]** Easy to think a fix has landed when the running GUI
hasn't been rebuilt. `project_summary` JSON shape is identical pre- and post-P5,
so there's no version pin or build hash in the response. _Where_:
`crates/rs_cam_mcp/src/server.rs project_summary`. _Fix idea_: include
`build_git_sha` and a `features: ["stale_defaults", "drill_gates", ...]` list
in the project summary so the agent knows which signals to expect.

#### Important

🟡 **[A, sim verdict]** `verdict: "WARNING: high air cutting"` fires on a
project-wide 65 % air-cut, but two toolpaths (TP3 at 100 %, TP7 at 84 %) account
for almost all of it — the other six are op-kind-appropriate (drill, project
curve on small surface features). Verdict text gives no actionable next step
and doesn't name TPs. P1 was supposed to fix this; the running binary predates
P1. _Where_: `crates/rs_cam_core/src/session/compute.rs diagnostics()`. _Fix
idea_: confirm P1 verdict shape lands once the GUI is rebuilt — text should
read "WARNING: TP3 'Rivers (back) (copy)' is 100 % air-cut; TP7 '3D Finish 6'
is 84 % air-cut" rather than a project-wide scalar.

🟡 **[A, narrate]** TP6 ("3D Rough 6") narration: prose summary line says
"62.3 % of cutting time is air-cut" while the engagement-distribution
table directly above shows "air [0.00..0.02] 36.8 %". Same toolpath, same
sim run, two different air-cut numbers. The 62.3 % is move-time-weighted with
a different threshold than the engagement-bucket cut-off. _Where_:
`crates/rs_cam_core/src/cli_narrate/` (engagement_distribution vs anomaly
section). _Fix idea_: either reconcile to one definition, or label the prose
line ("62.3 % below WOC-fraction 0.05") so the user can see why the two
numbers differ.

🟡 **[A, narrate]** Project-curve and drop_cutter narrations both end with the
boilerplate "high values on 3D roughing suggest boundary/stepover tuning or
stale remaining-stock assumptions" even when the op is a finishing pass or
curve trace. The text presumes adaptive3d. _Where_: same module. _Fix idea_:
gate the boundary/stepover hint on `op_type ∈ {Adaptive3d}`.

🟡 **[A, narrate]** "commanded depth_per_pass is unknown" appears for
drop_cutter and project_curve — both are intentionally non-DOC-driven ops.
"unknown" reads as a missing field rather than "this op type doesn't drive Z
that way". _Where_: same module. _Fix idea_: replace with "drop_cutter follows
surface heights; no commanded DOC" / "project_curve follows the curve at a
fixed surface offset; no commanded DOC".

🟡 **[A, tool model]** Tool object `diameter` field reads `1.0` but the
human-readable name is "Tapered Ball 2mm tip / 7° / 6mm shank". The chipload
LUT lookup then uses `diameter scale ×0.45` (i.e. treats the tool as ~1.43 mm
equivalent). Three different numbers describe the same tool. _Where_:
`crates/rs_cam_core/src/compute/tool_config.rs ToolConfig::diameter()`. _Fix
idea_: list both `tip_diameter_mm` and `effective_lookup_mm` in `list_tools`
output so an agent (and a user) can see which one feeds the LUT.

🟡 **[A, chipload-low verdict]** TP4/5/11 all flag "Chipload exceeds (low,
median_low)" with observed ≈ 0.0027 mm/tooth vs min 0.0045 mm/tooth. The
verdict gives no actionable next step — should the user raise feed, lower RPM,
or accept this (because the LUT row is extrapolated and uncertain)? The
verdict shape doesn't say "the LUT row that triggered this is extrapolated
×0.45 from a 3.175 mm row" prominently. _Where_:
`crates/rs_cam_core/src/tool_load/chipload.rs`. _Fix idea_: when
`confidence.kind == "approximate"` and the verdict is `exceeds low`, prepend a
banner: "LUT row is extrapolated from a 3× larger calibrated tool; consider
treating this as advisory rather than hard fail".

🟡 **[A, project_curve metric suppression]** TP3 (project_curve, 6 mm EM)
reports `chipload/power/deflection: unmodeled` with no reason given.
project_curve IS a continuous engagement op when the tool actually contacts
material. The current unmodeled state is masking the fact that it's all air.
_Where_: `crates/rs_cam_core/src/tool_load/mod.rs build_per_toolpath`. _Fix
idea_: separate "unmodeled because op type has no engagement" from "unmodeled
because no in-cut samples were captured" — the second is a finding, not an
exemption.

🟡 **[A, drill plunge envelope]** Both drill ops narrate as "plunge
feed/diameter: 50 1/min (material envelope 50–400 1/min)" — i.e. the suggested
plunge is at the **floor** of the safe envelope. Adequate for safety but
arguably leaves performance on the table. _Where_:
`crates/rs_cam_core/src/operation_defaults/drill.rs`. _Fix idea_: target the
midpoint of the envelope rather than the floor, unless the material is brittle.

🟡 **[A, alignment_pin_drill]** Pin Drill's 74 mm cutting distance vs 912 mm
rapid (12× rapid) is structural to the peck cycle, but the verdict layer would
flag a generic op with this ratio. P4 was meant to suppress air-cut metrics
for drill ops; that fired correctly (engagement / air-cut% not modeled) but
the rapid ratio is still visible to anyone reading the diagnostics. _Where_:
`crates/rs_cam_core/src/compute/simulate.rs SimToolpathEntry`. _Fix idea_: add
an op-kind annotation on the per-TP diagnostic block so consumers can ignore
"high rapid:cut ratio" on drill / pin-drill ops.

🟡 **[A, narrate compression]** Long Z-level lists ("… 72 intermediate Z
levels compressed …") are good, but the compression hides drift — e.g. a
single oddly-shaped pass in the middle of the run wouldn't show up. _Where_:
same module. _Fix idea_: keep the compression but also include the
maximum-anomaly pass (highest air-cut, lowest engagement, biggest DOC spike)
from inside the compressed range.

#### Polish

🟢 **[A, project name]** Loaded project shows `name: "Untitled"` despite being
the Wanaka workspace. The project TOML doesn't expose `name` to MCP. _Where_:
`crates/rs_cam_mcp/src/server.rs project_summary`. _Fix idea_: pull
project name from filename if `name` is unset.

🟢 **[A, units consistency]** plunge feed/diameter "50 1/min" — the unit
`1/min` reads awkwardly. _Where_: drill narrate. _Fix idea_: render as
"50 mm/min per mm Ø" or rename to `plunge_intensity`.

---

## Phase A2 — Wanaka tuning (push until break)

### Tune log

| # | Toolpath | Change | Effect | Runtime (s) |
|---|----------|--------|--------|-------------|
| baseline | — | — | TP3 100% air, TP4/5/7 chipload-low | 4130.7 |
| T1 | TP3 (Rivers copy) | depth -2.0 → +2.0 | engagement 0% → 14% heavy; ✅ real material removed | 4227.1 |
| T2 | TP7 (3D Finish 6) | plunge 750 → 150, min_z -50 → -5 | **no observable change** (silent) | 4227.1 |
| T3 | TP7 (3D Finish 6) | feed 2148 → 3000 | chipload exceeds-low → within ✅ | 3947.9 |
| T4 | TP6 (3D Rough 6) | stepover 2.2 → 3.5 | still within across all gates | 3849.0 |
| T5 | TP12/TP3 (Rivers copy) | feed 800 → 2500 | chipload obs 0.001 → 0.0035 (still below LUT min 0.030 — unreachable) | 3849.0 |
| T6 | TP6 (3D Rough 6) | stepover 3.5 → 4.8, DOC 2.6 → 4.0 | all gates within; deflection 71 µm hints "surface finish degradation expected" but kind=within | 3920.1 |

Net cycle-time improvement: **4130.7s → 3920.1s = -3.5 min (5%)** with two real
verdict improvements (TP3 100%-air fixed, TP7 chipload fixed).

### Findings — Phase A2

#### Critical

🔴 **[A2, runtime calc]** Changing `plunge_rate` from 750 → 150 mm/min on a
drop_cutter toolpath (TP7) produces identical total_runtime_s. A 5× slowdown of
plunge segments must visibly extend cycle time unless the runtime calc ignores
plunge_rate. Verified independently: total_runtime didn't move 1 ms.
_Where_: `crates/rs_cam_core/src/compute/simulate.rs` cycle-time accumulation.
_Fix idea_: confirm whether plunge segments are tagged distinctly and whether
the time integrator picks `plunge_rate` for them or falls back to `feed_rate`.
If the latter, exported g-code timing differs from sim — a real-cut surprise.

🔴 **[A2, silent generation no-op]** `set_toolpath_param('plunge_rate', X)` +
`generate_toolpath` on a drop_cutter returns the **identical** geometry
(cutting_distance, move_count, rapid_distance) because plunge_rate is a feed
attribute, not a geometry parameter — but the MCP reply is "Regenerate to
apply." with no signal that nothing changed. The user can't tell whether their
edit took effect. _Where_:
`crates/rs_cam_mcp/src/server.rs set_toolpath_param`. _Fix idea_: return a diff
list of "fields that affect geometry vs. fields that only affect runtime" so
the caller knows when a regen is necessary vs. cosmetic.

#### Important

🟡 **[A2, cross-TP coupling]** Editing TP7 (depth) silently changes TP5's
chipload verdict from `exceeds` → `unmodeled (simulation_required)`, even
though TP5 wasn't touched. Cause: TP3's deeper cut now overlaps TP5's intended
cut path, so TP5 finds no material to sample. No UI indicator that an upstream
edit affected a downstream toolpath's verdict. _Where_:
`crates/rs_cam_core/src/compute/simulate.rs SummaryAccumulator`. _Fix idea_:
record an "affected_by" trail on each verdict — "this toolpath's verdict
changed because TP3 was regenerated" — so the user knows where to look.

🟡 **[A2, deflection tri-band hidden]** Deflection gate has `bounds:
{validated_within_mm: 0.05, exceeds_mm: 0.2}` (three states: validated /
elevated / exceeds) but `kind` only emits `within` / `exceeds` / `unmodeled`.
T6 produced peak 71 µm — between 50 and 200 — and the only signal is a prose
string in `confidence.detail` saying "surface finish degradation expected".
The user reading the report by `kind` alone sees a green Within. _Where_:
`crates/rs_cam_core/src/tool_load/deflection.rs`. _Fix idea_: add an
`elevated` kind, or surface a `band: validated|elevated|exceeded` field
alongside `kind`. _Adjacent_: same shape exists for chipload approach_to_max
locality stats — review for consistency.

🟡 **[A2, counterintuitive cycle time]** Increasing stepover 3.5 → 4.8 *and*
DOC 2.6 → 4.0 on TP6 reduced move_count by 40% but increased total_runtime by
71s (1.8%). Cause: wider stepover means more rapid distance per row (rapid
mm: 13295 → 15598). The "more aggressive = faster" mental model is wrong here
and there's no in-UI breakdown of cut-time vs. rapid-time to explain why.
_Where_: `crates/rs_cam_core/src/compute/simulate.rs total_runtime` is a
single scalar. _Fix idea_: expose `total_runtime = cutting_runtime +
rapid_runtime + plunge_runtime + retract_runtime` so the optimizer's "this is
faster" claim is decomposable.

🟡 **[A2, project_curve below LUT min]** TP3/TP12 chipload-low cannot be
tuned `within` regardless of feed_rate. The LUT row for a 6 mm hardwood EM
has `min_mm_per_tooth: 0.030`; project_curve's effective sample-chipload is
~5-10% of nominal, so even at machine_max feed (4000 mm/min) the observed
0.04-ish is still below 0.030 after the geometry derating. The verdict is
"exceeds low" with no actionable next step — the user can't change the LUT
row and can't change the geometry. _Where_:
`crates/rs_cam_core/src/tool_load/chipload.rs` per-op handling. _Fix idea_:
either (a) suppress the chipload gate for `project_curve` (analogous to P4's
drill suppression), or (b) emit a structured "LUT row not applicable for this
op" reason rather than a numerical exceeds.

🟡 **[A2, T5 partial win]** Bumping TP12 feed_rate 800 → 2500 (3×) moved
observed chipload from 0.0011 → 0.0035 — a proportional move, but the gap to
the LUT min (0.030) is still 8.5×. The verdict UI shows "exceeds low" with no
hint about how far below the min the observation is, or whether tuning could
ever close the gap. _Where_: same module. _Fix idea_: render a
`shortfall_factor` (observed/min) on the verdict so a user can see at a glance
"3× below" vs "9× below" vs "near miss".

🟢 **[A2, two-op test]** T2 changing both plunge_rate AND min_z in one step
made it hard to attribute the no-op to either field individually. Lesson for
the agent (not a code finding): tune one dimension at a time.

---

## Phase B — Skeleton fleet (day-0 defaults check)

Method: load each `ux_*.toml`, add **one toolpath with default LUT-suggested
params** for the op family the fixture is designed to test, generate, run sim,
record what the user would see on first run. No tuning.

### Summary

| # | Fixture | Op added | Tool | Sim verdict | Rapid collisions | Moves | Runtime |
|---|---------|----------|------|-------------|------------------|-------|---------|
| B1 | 2D pocket | pocket | 6 mm EM | WARNING: rapid collisions | **28** | 272 | 13 min |
| B2 | 2D star | v_carve | 12.7 mm V-bit | WARNING: rapid collisions | **419** | **214 261** | **111 min** |
| B3 | 3D terrain | adaptive3d + scallop | 6 mm EM / 3 mm BN | WARNING: rapid collisions | **220** | 42 530 | 78 min |
| B4 | STEP block | face | 6 mm EM | WARNING: high air cutting | 0 ✅ | 312 | 18 min |
| B5 | STEP plate | profile | 6 mm EM | — | — | — | **generate fails** "Selected model has no 2D geometry" |
| B6 | STEP L-bracket | adaptive3d | 6 mm EM | WARNING: rapid collisions | **19** | 3 556 | 16 min |
| B7 | STEP stepped | adaptive3d | 6 mm EM | WARNING: rapid collisions | **15** | 2 685 | 12 min |

**6 of 7** fixtures produce rapid collisions or generation errors on day 0
with no tuning. **Face on flat STEP block is the only clean default path.**

### Findings — Phase B

#### Critical

🔴 **[B, rapid collisions on default]** Pocket, v_carve, adaptive3d on three
different STEP fixtures, and terrain adaptive3d+scallop all report rapid
collisions on freshly-added toolpaths using LUT-suggested params with no user
edits. Range: 15–419 collisions per toolpath. Verdict text is generic
("WARNING: rapid collisions detected") with no count, no location, no fix
hint. _Where_: most likely
`crates/rs_cam_core/src/operations/{pocket,v_carve,adaptive3d}.rs` retract
logic vs. `crates/rs_cam_core/src/dexel_stock/simulation.rs` collision
detection. _Fix idea_: the rapid_collision_count itself is the right metric;
the verdict needs to (a) report the count, (b) cite the worst-offending move,
and (c) link to per-op retract config so the user can raise safe-Z or enable
proper inter-region lifts. **This is the headline UX issue of the entire
session.**

🔴 **[B, peak DOC = full stock height]** B1 pocket reports peak axial
DOC = 12.00 mm (the entire 12 mm stock) at sample 3792, vs. commanded
depth_per_pass = 4.20 mm. Almost certainly the pre-P3 lift-bridge artifact —
the rapid down to the next region starting Z is measured as a single huge
DOC because previously-uncleared material is still "above" the cutter in the
dexel grid. The narrate anomaly correctly flags it as "Large DOC spikes often
point to … lift-function bridging" but the verdict surface doesn't translate
that into action. _Where_: pre-P3 binary; P3's transit-span gate fixes the
metric, but the underlying retract issue is real and feeds the rapid-collision
count above. _Fix idea_: rebuild the MCP-enabled GUI from HEAD and re-run B1
to confirm peak DOC drops to ≤ commanded depth_per_pass. If it doesn't,
investigate retract heights independently.

🔴 **[B, profile/STEP combo silently broken]** Adding a Profile toolpath to a
STEP plate (model is a thin 3D BREP, plate_100x60x10.step) is accepted by
`add_toolpath` and returns default params via `get_toolpath_params` — but
`generate_toolpath` produces a GUI error toast "Selected model has no 2D
geometry" and the MCP call hangs rather than returning a structured error.
The user gets no signal until generate, and the agent gets no error at all.
_Where_: `crates/rs_cam_core/src/operations/profile.rs` geometry resolution
+ MCP error plumbing in `crates/rs_cam_mcp/src/server.rs generate_toolpath`.
_Fix idea_: reject the operation/model pair at `add_toolpath` time with a
clear "Profile requires a 2D model (SVG/DXF) or a STEP face selection" error,
and make sure `generate_toolpath` returns the error message via MCP instead
of hanging.

#### Important

🟡 **[B, defaults vary across same machine/material]** Wanaka's adaptive3d on
6 mm EM in hardwood defaulted to `feed_rate: 4000` and
`clearing_strategy: agent_search`. ux_3d_terrain's identical machine+material
defaulted to `feed_rate: 911.25` and `clearing_strategy: contour_parallel`.
Same tool, same material, same machine; very different suggested params. The
delta is probably project-load history (Wanaka's TOML was saved before/after a
LUT regen) but the running session offers no surface to see "this param came
from auto-suggest" vs "this param is locked from a prior save". _Where_:
`crates/rs_cam_core/src/operations/adaptive3d/defaults.rs` (or wherever
suggest-on-add lives). _Fix idea_: tag each numeric param with its provenance
(`{value: 911.25, source: "lut:vendor.amana_3175...", locked: false}`) and
expose via `get_toolpath_params`.

🟡 **[B, fractional defaults]** Suggested feed rates render as
`769.506587956183` and stepovers as `2.0999999999999996` — full-precision
floats from the LUT calc. Visually noisy and reads as "weird specific number"
rather than "auto-suggested". _Where_: same module. _Fix idea_: round defaults
to 3 sig figs (or to vendor-LUT granularity) before serialization.

🟡 **[B, pocket DOC default = WOC factor]** B1 pocket on 6 mm EM in hardwood
defaulted to `depth_per_pass: 4.20 mm` = 70% of tool diameter, which matches
`machine.rigidity.woc_roughing_factor: 0.7` exactly. But woc is WIDTH-of-cut
(radial), and depth_per_pass is depth-of-cut (axial). The machine config has
a separate `doc_roughing_factor: 0.2` (= 1.2 mm DOC on a 6 mm tool). The
default appears to be reading the wrong rigidity field. 4.2 mm DOC on a 6 mm
flat in hardwood at feed 770 is well outside any conservative envelope.
_Where_: `crates/rs_cam_core/src/operation_defaults/pocket.rs` rigidity
sourcing. _Fix idea_: confirm the default reads `doc_roughing_factor` for DOC
and `woc_roughing_factor` for stepover, not the inverse.

🟡 **[B, v_carve move count]** B2 v_carve on a star produced 214 261 moves
for a single 70 × 86 mm region — a 111-minute cycle time on a small star
shape. Suggested `stepover: 0.254`, `tolerance: 0.05` together cause a
per-pass arc tessellation explosion. _Where_:
`crates/rs_cam_core/src/operations/v_carve.rs` discretization. _Fix idea_:
suggest stepover based on the V-bit chord at max_depth rather than a fixed
0.254 mm, and pre-flight estimate move count → warn at add_toolpath time if
estimate is > 50 k.

🟡 **[B, verdict naming inconsistency]** The Wanaka project verdict said
"WARNING: high air cutting" but the skeleton-fleet runs all said "WARNING:
rapid collisions detected". When both conditions are present (as in
ux_2d_pocket: 72 % air + 28 collisions), the verdict picks one and hides the
other. _Where_: `crates/rs_cam_core/src/session/compute.rs diagnostics()`.
_Fix idea_: emit a list of verdicts rather than a single string; rank by
severity (collisions > exceeds > air-cut > engagement-low).

🟡 **[B, 2D op + STEP-plate UX gap]** Closely related to the 🔴 above —
users naturally want to do `profile` and `pocket` on the top face of a thin
STEP plate (it's the workflow the file name suggests). The system has no
auto-extraction of 2D contour from STEP top faces. Either the docs need to
make the workflow ("import as SVG / use STEP face selector then …") explicit
at the fixture-load point, or the engine needs to support 2D ops on
STEP-face-selected models. _Where_: docs + `import_model` / face-selector
plumbing. _Fix idea_: when a 2D op is added to a STEP model, surface a
"pick a face" prompt in `add_toolpath` and pass that selection through.

#### Polish

🟢 **[B, scallop tool warning]** Scallop on the terrain auto-uses the 3 mm
Ball Nose which is correct. But no UI signal confirms "scallop requires a
ball-tip tool and you picked one correctly" — confidence-building moment lost.
_Where_: scallop add UI. _Fix idea_: at add-time, badge the tool selection
green if it's ball-tipped.

🟢 **[B, fixture naming]** `ux_step_block.toml` Setup defaults `face_up: top`
which is fine for the face-the-top-of-a-block workflow, but if the user wants
to face a different side, the affordance is far away. Minor; flagged because
the same pattern would bite STEP L-bracket which has 6 distinct faces.

---

## Phase C — Optimizer dogfood

Reloaded `~/Downloads/wanaka100/wanaka_full.toml` (7 toolpaths — different
snapshot than the previous load; no Pin Drill, but otherwise same project
shape). Ran the optimizer on two reps.

### C1 — TP5 (3D Rough 6, adaptive3d, 6 mm EM)

**Outcome**: `kind: ranked`, 4 candidates, `recommended_index: 1`.

- Baseline: DOC 3.0, feed 1500, stepover 2.0, RPM (default) → cycle **249.5 s**,
  chipload exceeds-low.
- **Recommended (refined #1)**: DOC 3.9, feed **4000** (machine max),
  spindle **8803** (close to min 8000), stepover 2.875 → cycle **145.5 s**
  (−42 %), chipload within (obs 0.060 in 0.05–0.085 band), deflection 35 µm
  (validated, no degradation flag).
- gate_deltas per candidate: `{chipload: improved, deflection: same, power: same}`.
- F1 RPM-down compensation visibly engaged (default ~12 k → 8803).

🟢 **[C, optimizer headline]** Output shape is excellent: envelope, narrative
headline, recommended_index, per-candidate gate_deltas, full param snapshots.
This is the strongest surface I saw in the session.

🟡 **[C, gate_delta opacity]** `gate_deltas: {chipload: "improved"}` doesn't
say *why* — was it the feed bump? the RPM down? the stepover widen? User has
to diff the `delta` block to figure out which axis moved the gate. _Fix idea_:
attach a one-line "primary axis: spindle_rpm 12000→8803 (F1 RPM-down)" to
each candidate.

🟡 **[C, no headline cycle delta]** The headline reads "Found 3 candidates
that improve on the baseline." but doesn't quote the headline win. Should
read "Found 3; best −42 % cycle time at 145.5 s vs 249.5 s baseline." _Where_:
`crates/rs_cam_core/src/optimizer/narrate.rs`.

### C2 — TP6 (3D Finish 6, drop_cutter, 1 mm tapered ball)

**Outcome**: `kind: no_safe_improvement`. 4 candidates, all 3 refined hit
chipload (worsened: side high, +41 % over LUT extrapolated max).

- `narrative.explanation`: "no candidate was both faster than baseline and
  within the gate's safe envelope: every candidate hit a gate limit (chipload,
  power, or deflection)"
- `narrative.headline`: "Tried 3 candidates; closest-to-safe still hit:
  chipload 0.0201 mm/tooth (+41 % over LUT max 0.0143)."
- `limiting_gates`: structured array with `bound`, `observed`, `overshoot_fraction`.
- `suggestions: [{axis: "feed", ceiling: 2701.68, kind: "cap_axis_at"}]`

🟢 **[C, no_safe_improvement is honest]** The optimizer's willingness to
report "I couldn't help" rather than silently pick a slightly-worse-than-
baseline candidate is the right behavior. The `suggestions` block ("cap feed
at 2702") is actionable.

🟡 **[C, search space omits safety-critical params]** The optimizer's
envelope spans `feed_rate / spindle_rpm / depth_per_pass / stepover` —
*not* `plunge_rate` or `min_z`. So the two P5 stale-default rules
(`tapered_ball_plunge_pre_fix2`, `drop_cutter_min_z_pre_b1`) — which apply
to exactly this toolpath — cannot be auto-fixed via this surface. The user
has to run two separate workflows: optimizer for feeds/speeds, P5 validator
for plunge/min_z. _Where_:
`crates/rs_cam_core/src/optimizer/search.rs` envelope definition. _Fix idea_:
optionally include plunge_rate in the search axes (capped at the
`safe_plunge_cap_mm_min` from `tool_load::plunge_stress`) and surface min_z
under a "geometry sanity" sub-search.

🟡 **[C, extrapolated LUT row reasoning]** TP6 baseline chipload `obs 0.0042
vs min 0.0356` is from `vendor_lut`. Refined candidates' bounds drop to
`vendor_lut_extrapolated: 0.0077–0.0143` (note: max < baseline's min). The
band literally shrinks when the optimizer perturbs the tool diameter
inference. The user sees the optimizer "fail" without understanding that the
goalposts moved. _Where_: `crates/rs_cam_core/src/tool_load/chipload.rs`
LUT-row resolution per candidate. _Fix idea_: pin the LUT-row resolution to
the baseline for all candidates (don't re-resolve per-candidate) so the
verdict math is comparable.

### C3 — TP0 Back Rough degenerate state (not optimized)

Discovered during baseline sim: TP0 reports `cutting_distance: 0.0` and
`move_count: 9`. The toolpath exists, status: Done, no error — but produces
no cuts on this load of wanaka_full.toml. Earlier in the session it produced
3572 moves and 15 742 mm of cuts. Same TOML, same engine.

🔴 **[C, degenerate toolpath silent]** A toolpath with 0 cut distance and 9
total moves should be a generation-time error, not a `status: Done`. _Where_:
`crates/rs_cam_core/src/operations/adaptive3d/mod.rs` post-generate check.
_Fix idea_: when generated `cutting_distance_mm == 0` for a non-drill op,
flag the toolpath as `status: GeneratedButEmpty` and surface in the verdict
("TP0 Back Rough generated 0 mm of cuts — re-check setup orientation").

---

## Phase D — Synthesis

### What works well

- **Optimizer surface** is the strongest part of the system. `ranked` and
  `no_safe_improvement` outcomes both surface useful, structured, actionable
  output. F1 RPM-down compensation visibly engages. `gate_deltas`,
  `recommended_index`, `narrative.headline`, `suggestions[]` are all useful
  signals.
- **`narrate_toolpath` prose** is the right shape for an agent: Z-level
  structure, engagement distribution, peak DOC anomaly, drill-cycle-specific
  metrics. Compresses well for long runs. Calls out lift-bridge / arc-fit
  patterns by name.
- **Drill ops (P3/P4 lineage)** narrate cleanly: "deepest hole 27 mm,
  depth-to-diameter 4.5×, chip-welding risk low, peck pattern adequate". The
  drill-native metrics shape is exactly what an agent/user needs.
- **Tool-load report per-toolpath verdicts** are structured (kind, side,
  triggering, evidence, confidence) — far better than a single load %.

### The single dominant pain

**Rapid collisions on default-params toolpaths.** 6 of 7 skeleton fixtures
fire `rapid_collision_count > 0` on day 0 with zero user edits — pocket (28),
v_carve (419), 3D-terrain adaptive3d+scallop (220), L-bracket adaptive3d (19),
stepped-block adaptive3d (15). Only face-on-flat-STEP is clean. The verdict
text reads "WARNING: rapid collisions detected" with no count, no offender,
no fix hint. The user has no path to debug without dropping into the cut
trace.

### Common-negatives watchlist scoring

From the original `UX_TESTING_SESSION_PLAN.md`, scored against this session:

| # | Pattern | Hits | Examples |
|---|---------|------|----------|
| 1 | Silent state mutation | ✅✅✅ | TP3 100% air, TP7 plunge no-op, cross-TP coupling, TP0 0-cut-distance |
| 2 | Verdict with no next step | ✅✅✅ | "WARNING: rapid collisions", chipload-low across small-TB ops |
| 3 | Count with no severity breakdown | ✅✅ | `issue_count: 21671` (mostly noise) |
| 4 | Same color for two severities | n/a | Sim is MCP-driven; UI colors not exercised |
| 5 | List where graph would be honest | ✅ | `hotspot_count: 1105` rolls up engagement |
| 6 | Stale data shown as fresh | ✅ | After TP3 edit, TP5 chipload re-shaped without warning |
| 7 | Modal panel blocks viewport | n/a | MCP session |
| 8 | Defaults every user must change | ✅✅✅ | pocket DOC 70 % D (wrong field), v_carve stepover 0.254 (214k moves), drop_cutter min_z -50 |
| 9 | Missing model-limit framing | ✅✅ | project_curve below LUT min, drill engagement "not modeled" |
| 10 | Ambiguous affordance | ✅ | `add_toolpath` returns success for invalid op+model combos |

Eight of ten common-negatives hit, two not exercised in this session.

### Surface × severity table

| Surface | 🔴 | 🟡 | 🟢 | Total |
|---------|-----|-----|-----|-------|
| Sim verdict / diagnostics | 1 | 4 | 0 | 5 |
| Operation engines (collision, lift-bridge) | 3 | 0 | 0 | 3 |
| Defaults / LUT suggest-on-add | 1 | 4 | 0 | 5 |
| Narrate (CLI prose) | 0 | 5 | 1 | 6 |
| Tool-load report (chipload/power/deflection) | 0 | 5 | 0 | 5 |
| Generate / regenerate | 3 | 1 | 0 | 4 |
| Optimizer | 0 | 4 | 2 | 6 |
| Cross-TP coupling / silent runtime | 1 | 1 | 0 | 2 |
| Tool model | 0 | 1 | 0 | 1 |
| Build cadence / version pin | 0 | 1 | 0 | 1 |
| Polish / docs / units | 0 | 0 | 4 | 4 |
| **Total** | **9** | **26** | **7** | **42** |

### Next investment target

**Sim verdict / diagnostics layer.**

Rationale: the dominant pain (rapid collisions on default) is an engine
issue, but the engine fix is far away. In the meantime, the verdict layer is
the single place that can route around the engine bug at low cost:

- Already has the per-TP `rapid_collision_count` data.
- Already has the P1 template (name TPs in verdict text rather than
  project-wide scalar).
- Already has structured tool-load verdicts to fold in.
- One module to change (`crates/rs_cam_core/src/session/compute.rs
  diagnostics()`), well-bounded scope.

Concrete next-PR shape:

1. Verdict returns `Vec<Verdict>` not `String`, ranked by severity (collisions
   > exceeds > air-cut > engagement-low).
2. Each verdict carries (severity, headline, offender_toolpath_ids,
   fix_hint, link_to_evidence).
3. Specifically: "Rapid collisions on TP3 'Rivers (back) (copy)' (28
   collisions, worst at move 100, z=-2.500). Likely cause: inter-region
   rapid moves not lifting to safe-Z. Fix: increase `retract_z` in operation
   params or set a boundary to constrain rapids."

That single change would let the user route around the engine-level
rapid-collision bug today *and* would surface the P5 stale-defaults work
once the GUI is rebuilt from HEAD.

### Secondary investment target

**Default suggestion provenance**. The "769.506587956183" feed rate and the
pocket DOC = `woc_roughing_factor × D` bug both stem from the suggest-on-add
calc not telling the user (or agent) where each value came from. A
`{value, source, confidence}` triple on every default would solve five
findings at once:

- "weird precision" perception
- pocket DOC field mis-wiring (would have been caught at QA: "this default
  reads from woc_roughing_factor not doc_roughing_factor")
- LUT-extrapolated bands for small tools (already structured in
  tool_load — extend to defaults)
- divergent defaults between Wanaka and ux_3d_terrain (provenance reveals
  whether it's saved-state vs fresh-suggest)
- the user-confidence problem ("is this number sane or did someone type
  it?")

### Session takeaway

The engine has built deep, useful diagnostic structures (drill summaries,
tool-load gates, optimizer narratives, narrate prose). The recently-shipped
priority work (P1–P5) targets exactly the right joints. The dominant remaining
issue is that **the verdict layer doesn't yet route those structures into a
human-actionable surface** — and the safety-critical engine bug (rapid
collisions on default retract logic) is hiding behind a generic warning
string. Fix the verdict surface and the same engine becomes much safer to
operate.

---

*Session conducted 2026-05-20 via MCP against a pre-P1–P5 GUI binary. Wanaka
real-cut review found 1 critical geometric trap, 2 stale defaults, 1
meaningful tune win (+5% cycle time, two gates flipped). Skeleton fleet found
6 of 7 fixtures fire rapid-collision warnings on default params. Optimizer
returned high-quality `ranked` and `no_safe_improvement` outcomes on the two
toolpaths tested.*
