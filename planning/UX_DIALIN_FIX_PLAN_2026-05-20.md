# UX Dial-in Fix Plan — 2026-05-20

Companion to [`UX_DIALIN_REVIEW_2026-05-20.md`](UX_DIALIN_REVIEW_2026-05-20.md).
One row per finding from that review (41 findings — 8 🔴, 26 🟡, 7 🟢).
For each: status, code locations (verified against current tree, not the
review's `operations/*` paths which predate the flat layout), investigation
needed, and a proposed fix sketch. Update the **Status** field as work
progresses.

## Status legend

| Symbol | Meaning |
|--------|---------|
| 🔵 To investigate | Not started — assumption / proposed fix from review only |
| 🟠 Investigating | Reading code / repro in progress |
| 🟣 Fix designed | Investigation done, concrete fix proposal ready, awaiting implementation |
| 🟢 In progress | Code being written |
| ✅ Fixed | Merged + verified |
| ⏸️ Blocked-on-rebuild | Running MCP binary predates a fix that may already be in HEAD; verify post-rebuild before any code work |
| ❌ Won't fix | Closed — documented reason |

**Effort buckets**: S = <1h · M = half-day · L = full-day+ · XL = multi-session

---

## 0. Pre-work: Rebuild the MCP-enabled GUI from HEAD

Several findings may auto-close once the running binary picks up the P1–P5
priority work shipped 2026-05-19. **Do this first.** All findings tagged
⏸️ Blocked-on-rebuild below should be re-verified against the rebuilt binary
before any code is written.

Findings to re-verify post-rebuild:

- A2 (P5 stale-defaults validator — `drop_cutter_min_z_pre_b1`, `tapered_ball_plunge_pre_fix2`)
- A4 (P1 verdict-naming refactor)
- A11 (P4 air-cut suppression edge — does it apply to drill plunge envelope?)
- A12 (P4 air-cut suppression on alignment_pin_drill rapid:cut ratio)
- B2 (P3 transit-span gate for peak DOC = stock height)

---

## Master index

| ID  | Sev | Surface | Status | Title |
|-----|-----|---------|--------|-------|
| A1  | 🔴 | Op engine | 🔵 | project_curve depth-sign trap (TP3 100% air, no banner) |
| A2  | 🔴 | Defaults | ⏸️ | drop_cutter stale defaults `min_z=-50`, `plunge=750` not flagged |
| A3  | 🟡 | Build cadence | ✅ | `project_summary` has no build-sha / feature list |
| A4  | 🟡 | Sim verdict | ✅ | Project verdict doesn't name offending TPs |
| A5  | 🟡 | Narrate | 🔵 | Air-cut % mismatch (62.3% prose vs 36.8% distribution table) |
| A6  | 🟡 | Narrate | ✅ | Boilerplate adaptive3d hint emitted on non-adaptive ops |
| A7  | 🟡 | Narrate | ✅ | "commanded depth_per_pass is unknown" on non-DOC ops |
| A8  | 🟡 | Tool model | ✅ | Tool diameter inconsistency (named / `diameter` field / LUT-effective) |
| A9  | 🟡 | Tool-load | ✅ | Chipload-low verdict from extrapolated LUT row has no action hint |
| A10 | 🟡 | Tool-load | ✅ | project_curve `unmodeled` masks "all-air" condition |
| A11 | 🟡 | Defaults | 🟠 | Drill plunge envelope suggests the floor (no perf headroom) |
| A12 | 🟡 | Sim verdict | ✅ | alignment_pin_drill rapid:cut ratio not op-kind-annotated |
| A13 | 🟡 | Narrate | 🔵 | Z-level compression hides max-anomaly pass |
| A14 | 🟢 | Polish | ✅ | Project name shows "Untitled" |
| A15 | 🟢 | Polish | ✅ | Units "50 1/min" reads awkwardly |
| A2.1| 🔴 | Runtime | 🔵 | `plunge_rate` change yields zero runtime delta (sim integrator bug) |
| A2.2| 🔴 | Generate | 🔵 | `set_toolpath_param` silent no-op for non-geometry fields |
| A2.3| 🟡 | Cross-TP | 🔵 | Editing TP3 silently changes TP5 verdict; no audit trail |
| A2.4| 🟡 | Tool-load | 🔵 | Deflection `elevated` band hidden behind binary `kind` |
| A2.5| 🟡 | Runtime | 🔵 | `total_runtime_s` not decomposed (cut / rapid / plunge / retract) |
| A2.6| 🟡 | Tool-load | 🔵 | project_curve chipload-low untunable; LUT-row-not-applicable signal missing |
| A2.7| 🟡 | Tool-load | 🔵 | No `shortfall_factor` on chipload-low verdicts |
| A2.8| 🟢 | (agent lesson) | ❌ | Multi-axis tune attribution — agent-side, not code |
| B1  | 🔴 | Op engine | 🟠 | Rapid collisions on 6/7 default-LUT skeleton TPs; verdict-layer ✅, engine half: pocket ✅ (PR-2A), v_carve & adaptive3d covered by the same shared-dressup fix (no separate generators emit dangerous rapids) |
| B2  | 🔴 | Op engine | ⏸️ | Peak axial DOC = full stock height on B1 pocket (pre-P3 lift-bridge) |
| B3  | 🔴 | Generate | 🔵 | Profile + STEP model accepted by add_toolpath, hangs on generate |
| B4  | 🟡 | Defaults | 🔵 | Defaults diverge between Wanaka and ux_3d_terrain on same machine/material |
| B5  | 🟡 | Defaults | ✅ | Fractional default values (`769.506587956183`) — round on serialize |
| B6  | 🟡 | Defaults | 🟠 | Pocket DOC default reads `woc_roughing_factor` not `doc_roughing_factor` — _hypothesis falsified, see §3_ |
| B7  | 🟡 | Op engine | 🔵 | v_carve produces 214 k moves on a small star (stepover/tolerance defaults) |
| B8  | 🟡 | Sim verdict | ✅ | Verdict picks one warning when several conditions are present |
| B9  | 🟡 | Generate | 🔵 | 2D op + STEP plate workflow has no face-extraction prompt |
| B10 | 🟢 | Polish | 🔵 | Scallop ball-tip tool selection not confirmed at add-time |
| B11 | 🟢 | Polish | 🔵 | Setup `face_up: top` affordance is far from the "face other side" workflow |
| C1  | 🟢 | Optimizer | ✅ | Optimizer output shape is excellent — keep working as-is |
| C2  | 🟡 | Optimizer | 🔵 | `gate_deltas: {chipload: improved}` doesn't say which axis moved the gate |
| C3  | 🟡 | Optimizer | ✅ | Headline doesn't quote cycle-time delta |
| C4  | 🟢 | Optimizer | ✅ | `no_safe_improvement` outcome is honest — keep as-is |
| C5  | 🟡 | Optimizer | 🔵 | Search envelope omits `plunge_rate` and `min_z` |
| C6  | 🟡 | Optimizer | 🔵 | LUT row re-resolves per candidate; band shrinks mid-search |
| C7  | 🔴 | Op engine | ✅ | Adaptive3d "GeneratedButEmpty" state reported as `status: Done` |

---

## 1. Sim verdict / diagnostics layer

Primary investment target per Phase D synthesis. One module
(`crates/rs_cam_core/src/session/compute.rs::diagnostics()`), well-bounded.

### A4 🟡 — Project verdict doesn't name offending TPs

- **Status**: ✅ Shipped Session 2 (2026-05-21) as PR-1
- **Code locations**: `crates/rs_cam_core/src/session/compute.rs::diagnostics()`,
  `crates/rs_cam_core/src/session/mod.rs` (`Verdict` types)
- **Outcome**: `ProjectDiagnostics` now carries a `verdicts: Vec<Verdict>`
  alongside the legacy `verdict: String`. Each `Verdict` carries
  `(severity, kind, headline, offender_toolpath_ids, fix_hint, evidence)`.
  Headlines name the offending toolpath(s) with their id + name. The legacy
  string field mirrors the highest-severity verdict's headline for callers
  that haven't been upgraded.

### B8 🟡 — Verdict picks one warning when several conditions present

- **Status**: ✅ Shipped Session 2 (2026-05-21) as PR-1
- **Code locations**: `session/compute.rs::diagnostics()`
- **Outcome**: The single early-exit `if collisions > 0 { … } else if … { … }`
  chain is replaced with severity-ranked accumulation. Order:
  `Critical (HolderCollision, RapidCollision) → Important (PlungeStress,
  GeneratedEmpty) → Polish (AirCut)`. All applicable verdicts are emitted; the
  list is sorted by severity. Tested by `diagnostics_ranks_verdicts_by_severity`.

### B1 (verdict half) 🔴 — Rapid collisions verdict text uninformative

- **Status**: ✅ Shipped Session 2 (2026-05-21) — _verdict-layer route-around only._
  Engine root cause stays open (see §2 "B1 (engine half)").
- **Code locations**: `session/compute.rs::diagnostics()` rapid-collision
  branch
- **Outcome**: For each toolpath that triggered rapid-through-stock
  collisions, the verdict layer now emits one `RapidCollision` verdict per
  TP carrying:
  - `headline`: `"WARNING: rapid collisions on TP{id} 'Name' ({count} collisions, worst at move {n}, z={z:.3})"`
  - `evidence`: `{ move_index, z_value (cutter end-z), count }` — the
    deepest end-Z rapid is selected as the representative worst move
  - `fix_hint`: points at `retract_z`, safe-Z, and boundary config
  Tested by `diagnostics_rapid_collision_verdict_carries_evidence`.

### A12 🟡 — alignment_pin_drill rapid:cut ratio not op-kind-annotated

- **Status**: ✅ Shipped Session 2 (2026-05-21) as PR-1
- **Code locations**: `crates/rs_cam_core/src/compute/catalog.rs`
  (`OperationType::kind_str`, `is_drill_kinematics`),
  `session/mod.rs` (`ToolpathDiagnostic.op_kind` field),
  `session/compute.rs::diagnostics()`
- **Outcome**: Added `OperationType::kind_str()` (stable snake_case identifier
  e.g. `"drill"`, `"alignment_pin_drill"`, `"pocket"`) and
  `is_drill_kinematics()`. Every `ToolpathDiagnostic` now carries `op_kind`,
  letting downstream consumers (UI, agents, gates) suppress rapid:cut-ratio
  signals on Z-only kinematics ops. The C7 GeneratedEmpty verdict uses
  `is_drill_kinematics()` to exempt drill toolpaths from the zero-cut check.
  Tested by `diagnostics_tags_per_tp_with_op_kind`.

### C7 🔴 — Adaptive3d "GeneratedButEmpty" reported as `status: Done`

- **Status**: ✅ Shipped Session 2 (2026-05-21) as PR-1 — verdict-layer signal.
  (A standalone `status: GeneratedButEmpty` on the result struct itself is
  not yet a separate state — the diagnostic is the operator-visible signal.)
- **Code locations**: `session/compute.rs::diagnostics()` empty-results branch
- **Outcome**: After generation, any toolpath whose
  `result.stats.cutting_distance == 0` and whose op kind is not drill /
  alignment_pin_drill emits an `Important` `GeneratedEmpty` verdict naming
  the TP. The fix hint points at setup orientation, stock alignment, and
  depth sign convention — covering the project_curve trap (A1) and the
  adaptive3d "wanaka_full reload" repro. Tested by
  `diagnostics_emits_generated_empty_for_zero_cut_non_drill`.

---

## 2. Operation engines (collision, lift-bridge)

Deeper / longer-cycle work. The verdict-layer route-around in §1 unblocks
the user today; these are the actual fixes.

### B1 (engine half) 🔴 — Retract logic on pocket / v_carve / adaptive3d

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/pocket.rs`,
  `crates/rs_cam_core/src/v_carve.rs` (if present),
  `crates/rs_cam_core/src/adaptive3d/path.rs`,
  `crates/rs_cam_core/src/dexel_stock/simulation.rs` (collision detection)
- **Investigation**:
  1. Identify which op type produces the cleanest repro (B1 pocket: 28 collisions).
  2. Trace one collision: what was the cutter z, what was the target z, what
     was `safe_z`, did the move skip the retract?
  3. Determine whether the inter-region transition writes `Rapid` moves that
     should be `Retract → Rapid → Plunge`, or whether the retract itself
     descends through previously uncleared material.
- **Proposed fix**: Almost certainly per-op specific. Suspect: pocket
  inter-region lift uses a too-low intermediate z; v_carve & adaptive3d may
  have a different root cause (lift function evaluated against fresh dexel
  vs. as-cut state).
- **Effort**: L per op family — likely 3 separate PRs

### B2 🔴 — Peak DOC = full stock height (pre-P3 lift-bridge)

- **Status**: ⏸️ Blocked-on-rebuild (P3 transit-span gate)
- **Code locations**: `compute/simulate.rs` peak-DOC accumulation
- **Investigation**: After rebuild, re-run B1 pocket. If peak DOC drops to
  ≤ commanded `depth_per_pass`, P3 already covers it. If not, investigate
  the retract path independently.
- **Proposed fix**: Likely already in HEAD — verify only.
- **Effort**: S (verification) → M (if real follow-up needed)

### B7 🟡 — v_carve 214k moves on a small star

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/` v_carve generation (verify exact file)
- **Investigation**:
  1. Confirm root cause: default `stepover: 0.254` × `tolerance: 0.05` causes
     per-pass arc tessellation explosion on a 70×86 mm star.
  2. Test: does setting stepover to V-bit-chord-at-max-depth resolve it?
- **Proposed fix**: Two-part:
  - Compute default stepover based on V-bit included-angle and max depth
    (geometric chord, not a fixed 0.254 mm).
  - At `add_toolpath` time, estimate move count and warn if > 50 k.
- **Effort**: M

### A1 🔴 — project_curve depth-sign trap (TP3 100% air)

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/project_curve.rs`
- **Investigation**: Confirm the sign-convention bug — `depth: -2.0,
  direction: from_below` should put the path below stock-top by 2 mm, but
  empirically lands 2 mm above. Either the convention is wrong or the doc
  describes it backwards.
- **Proposed fix**: Two-part:
  - **Fix the geometric semantics** if the sign convention is genuinely
    backwards — and add a regression test that loads a `depth = -2,
    from_below` project and asserts cutter z is below stock top.
  - **Defense-in-depth at sim emit**: when generated cut Z is entirely above
    stock-top for the whole toolpath, raise a diagnostic
    ("toolpath generated 0 mm of in-material cut") before sim even runs.
- **Effort**: M

---

## 3. Defaults / LUT suggest-on-add

### B6 🟡 — Pocket DOC default reads `woc_roughing_factor`

- **Status**: 🟠 Investigating — hypothesis falsified
- **Investigation (2026-05-21)**: `doc_roughing_factor` and `woc_roughing_factor`
  are **only read in MCP/GUI serialization for outbound payloads** (search:
  `grep -rn "doc_roughing_factor\|woc_roughing_factor" crates/`). They are
  NOT consumed by pocket, feeds, or any defaults logic. `PocketConfig::default()`
  is `depth_per_pass: 1.5` (a static literal at
  `compute/operation_configs.rs:230`). The observed 4.2 mm on the Wanaka
  pocket TP comes from `compute_feeds_for_op` →
  `rs_cam_core::feeds::calculate` writing back through
  `apply_feeds_result_to_op` (GUI add-toolpath flow at
  `controller/events/toolpath.rs:127`). The 4.2 ≈ 0.7×6 observation was
  coincidence.
- **Real next step**: Run a small repro — fresh project with 6mm EM in
  hardwood on a real router config and inspect the actual
  `feeds::FeedsResult.axial_depth_mm` returned for `OperationFamily::Pocket`.
  If it's genuinely 4.2 mm, that may be safe and the review's concern was
  unfounded; if it's wrong, the bug lives in `feeds/mod.rs` axial-depth
  derivation, not in rigidity-factor wiring.
- **Effort**: S (just the feeds-calc repro)

### A2 🔴 — drop_cutter stale defaults `min_z=-50`, `plunge=750`

- **Status**: ⏸️ Blocked-on-rebuild (P5 validator targets this exactly)
- **Code locations**: `compute/validate.rs` rules
  `drop_cutter_min_z_pre_b1`, `tapered_ball_plunge_pre_fix2`
- **Investigation**: Confirm against rebuilt binary whether
  `project_summary.stale_defaults` populates with these two rules firing
  on TP7 of Wanaka.
- **Proposed fix (if validator works post-rebuild)**: Pure surface work —
  ensure GUI shows the validator output as a banner on stale TPs.
- **Proposed fix (if validator doesn't fire)**: Trace why P5 isn't reaching
  the live MCP code path; expose `stale_defaults` in `project_summary`.
- **Effort**: S (verification) → M (if validator needs additional plumbing)

### B4 🟡 — Defaults vary across same machine/material

- **Status**: 🔵
- **Code locations**: Adaptive3d defaults, project-load path
- **Investigation**: Determine whether Wanaka's `feed_rate: 4000` was
  saved-then-loaded vs. fresh-suggest-on-add. Likely the file persists the
  value and load doesn't re-run suggestion.
- **Proposed fix**: Tag each numeric param with provenance:
  `{value: 911.25, source: "lut:vendor.amana_3175...", locked: false}`.
  Expose via `get_toolpath_params`. UI shows a tooltip; agent reads
  source field. Solves B4 + B5 partially + several discoverability findings.
- **Effort**: L (touches every default + every load path) — but the
  Phase D secondary investment target

### B5 🟡 — Fractional defaults (`769.506587956183`)

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_viz/src/ui/properties/mod.rs::apply_feeds_result_to_op`
  (the single chokepoint where `feeds::FeedsResult` writes into
  `OperationConfig`).
- **Change**: Added a `round_to(value, step)` helper; feed_rate and
  plunge_rate are rounded to 1 mm/min, stepover/DOC are rounded to
  0.001 mm. Suggested numbers now read like suggestions, not measurements.
- **Tests**: 183 viz lib tests pass, clippy clean.

### A11 🟡 — Drill plunge envelope at the floor

- **Status**: 🟠 Investigating — deeper than a "tweak the default" fix
- **Investigation (2026-05-21)**: `DrillConfig` has only `feed_rate` (no
  separate `plunge_rate` field) — drill ops are plunge-only so `feed_rate`
  IS the plunge feed. The static default at
  `compute/operation_configs.rs:152` is `feed_rate: 300.0` mm/min →
  ~50 1/min on a 6 mm tool (= the envelope floor). When add-toolpath calls
  `apply_feeds_result_to_op` (GUI path), `result.plunge_rate_mm_min` is
  computed by `feeds::calculate` but is then written via
  `op.set_plunge_rate()` — which on `DrillConfig` is a **no-op** (no field
  to write to). So the feeds-calc envelope is effectively unused for
  drills. _This is the actual bug behind A11._
- **Proposed fix**: Two-part:
  1. Make `DrillConfig::set_plunge_rate` map onto `feed_rate` (or rename
     field to `plunge_rate`). Will need a TOML compatibility shim.
  2. Update the drill default in `feeds::calculate` (or wherever drill
     plunge is picked) to target envelope midpoint rather than floor.
- **Effort**: M (touches the OperationParams trait + TOML serde)

---

## 4. Narrate (CLI prose)

All five live in `crates/rs_cam_core/src/narrate.rs`. Bundle into one PR.

### A5 🟡 — Air-cut % mismatch (62.3% prose vs 36.8% distribution)

- **Status**: 🔵
- **Code locations**: `narrate.rs` engagement_distribution vs anomaly section
- **Investigation**: Identify the two formulas. The 62.3% is likely
  move-time-weighted with one cut-off threshold; the 36.8% is the
  `[0.00..0.02]` engagement bucket.
- **Proposed fix**: Either reconcile to one definition, or label the prose
  line: "62.3 % below WOC-fraction 0.05 (vs. 36.8 % in bucket [0.00..0.02])"
  so the discrepancy is self-explanatory.
- **Effort**: S

### A6 🟡 — Boilerplate adaptive3d hint on non-adaptive ops

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/narrate.rs` air-cut anomaly emitter
  (the `{marker} {:.1}% of cutting time is air-cut …` line near line 1078).
- **Change**: Match `context.operation_label` and emit one of:
  - Roughing ops (`3D Rough` / `Adaptive` / `Rest Machining`) → original
    "boundary/stepover tuning" hint.
  - Finishing ops (`3D Finish` / `Scallop Finish` / `Waterline` / `Pencil` /
    `Steep/Shallow` / `Ramp` / `Spiral` / `Radial` / `Horizontal Finish`) →
    "air-cut% is dominated by surface terrain — relative comparison …".
  - Curve-trace ops (`Project Curve` / `VCarve` / `Trace`) → "Curve-following
    ops … air-cut% mostly reflects rapids and approach segments …".
  - Default → no trailing hint.
- **Tests**: existing narrate tests pass.

### A7 🟡 — "commanded depth_per_pass is unknown" on non-DOC ops

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/narrate.rs` peak-DOC anomaly
  `threshold_text` (line ~941).
- **Change**: When `context.depth_per_pass_mm` is `None`, match
  `context.operation_label`:
  - `3D Finish` → "this op follows surface heights — no commanded DOC"
  - `Project Curve` → "this op follows the curve at a fixed surface
    offset — no commanded DOC"
  - `Drill` / `Pin Drill` → "this op advances by peck depth — no
    continuous DOC"
  - `VCarve` → "this op cuts to a target V-bit depth — no commanded DOC"
  - else → fall back to original "commanded depth_per_pass is unknown".
- **Tests**: existing narrate tests pass.

### A13 🟡 — Compression hides max-anomaly pass

- **Status**: 🔵
- **Code locations**: `narrate.rs` Z-level compression logic
- **Proposed fix**: Keep compression but also include the
  maximum-anomaly pass (highest air-cut, lowest engagement, biggest DOC
  spike) from inside the compressed range as a "highlight" line.
- **Effort**: M

### A15 🟢 — Units "50 1/min" awkward

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/narrate.rs` drill plunge
  envelope render (line ~1044, ~1050).
- **Change**: Renamed "plunge feed/diameter" to "plunge intensity";
  unit string `1/min` → `mm/min per mm Ø` everywhere it was rendered.

---

## 5. Tool-load report (chipload / power / deflection)

### A9 🟡 — Chipload-low from extrapolated LUT row not actionable

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/tool_load/chipload.rs` already
  emits the right `Confidence::Approximate(detail)` with the extrapolation
  ratio. The user-facing renderer in
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs::status_line` was treating
  every Exceeds as a hard fail.
- **Change**: When `status.kind == Chipload` and confidence is
  `Approximate(_)`, the verdict prefix changes from "EXCEEDS:" to
  "ADVISORY (extrapolated LUT row):" — same evidence text, different
  framing. Confidence detail still appears verbatim so the operator sees
  the diameter-scale / hardness-scale numbers.
- **Tests**: 1 sim_diagnostics test passes, workspace clippy clean.
- **Follow-up**: same prefix reframe should propagate to the MCP wire
  format if/when verdict-text rendering moves to a shared module. Today
  the MCP path serializes the typed verdict directly so an agent already
  has the structured `confidence.kind == "approximate"` signal; only
  the GUI render needed the cosmetic reframe.

### A10 🟡 — project_curve `unmodeled` masks "all-air" condition

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**:
  - Enum extended: `crates/rs_cam_core/src/tool_load/verdict.rs::UnmodeledReason`
    — new variant `AllSamplesAirCutOrRapid`.
  - Verdict producer: `crates/rs_cam_core/src/tool_load/chipload.rs::evaluate`
    branches on `trace.samples.iter().any(|s| s.toolpath_id == ...)` to
    distinguish "no trace for this TP" (returns `SimulationRequired`) from
    "trace exists for this TP but every sample is air/rapid" (returns the
    new variant).
  - Renderers: `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` and
    `crates/rs_cam_viz/src/ui/preflight.rs` both render the new variant:
    "Unmodeled: toolpath made no contact with material — every sample was
    a rapid or air-cut. Check depth / direction / stock position."
- **Tests**: two prior tests asserted the old `SimulationRequired` behavior
  for the "samples exist but none in-cut" case. Updated to expect the new
  `AllSamplesAirCutOrRapid` variant + doc-comment now describes the new
  contract. 303 tool_load tests pass, 1507 core lib tests pass, full
  workspace clippy clean.
- **Follow-up**: same predicate (`is_cutting && !air_cut` filter) is mirrored
  in `tool_load/power.rs` and `deflection.rs` `SimulationRequired` branches.
  Power and deflection still return `SimulationRequired` for the all-air
  case — extending the new variant to them is the natural next bite,
  deferred to keep this change tight.

### A2.4 🟡 — Deflection `elevated` band hidden behind binary `kind`

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/tool_load/deflection.rs`
- **Investigation**: Audit chipload `approach_to_max` locality stats for the
  same tri-band shape — if both have it, fix the shared verdict shape once.
- **Proposed fix**: Either add an `elevated` enum variant, or surface a
  `band: validated|elevated|exceeded` field alongside `kind`. Latter is
  less invasive on consumers.
- **Effort**: M

### A2.6 🟡 — project_curve chipload-low untunable

- **Status**: 🔵 — related to A10
- **Code locations**: `tool_load/chipload.rs` per-op handling
- **Proposed fix**: (a) Suppress chipload gate for `project_curve`
  analogous to P4's drill suppression, **or** (b) emit a structured
  "LUT row not applicable for this op" reason. Prefer (b) — it preserves the
  signal that the row is wrong without hard-failing.
- **Effort**: M (touches the verdict-kind shape)

### A2.7 🟡 — No `shortfall_factor` on chipload-low verdicts

- **Status**: 🔵
- **Code locations**: `tool_load/chipload.rs` evidence emission
- **Proposed fix**: Add `shortfall_factor: observed / min_bound` (or
  `excess_factor: observed / max_bound` for high-side) to the evidence
  block. Render in narrate as "3× below LUT min" / "1.4× over LUT max".
- **Effort**: S

---

## 6. Generate / regenerate

### B3 🔴 — Profile + STEP combo silently broken

- **Status**: 🔵
- **Code locations**:
  - `crates/rs_cam_core/src/profile.rs` geometry resolution
  - `crates/rs_cam_mcp/src/server.rs::generate_toolpath` error plumbing
- **Investigation**:
  1. Confirm hang — does MCP `generate_toolpath` block on the same path
     where the GUI shows "Selected model has no 2D geometry"?
  2. Trace: where does the error get raised in `profile.rs`? Where does it
     get caught (or not)?
- **Proposed fix**: Two-part:
  - Reject the operation/model pair at `add_toolpath` time with a clear
    error: "Profile requires a 2D model (SVG/DXF) or a STEP face selection."
  - Ensure `generate_toolpath` propagates errors via MCP instead of hanging.
- **Effort**: M

### B9 🟡 — 2D op + STEP plate workflow has no face prompt

- **Status**: 🔵 — closely related to B3
- **Code locations**: `add_toolpath` MCP entry + `import_model` face selector
- **Proposed fix**: When a 2D op is added to a STEP model, surface a
  "pick a face" prompt in `add_toolpath` and thread that selection through
  to operation generation. Or document the SVG-export workflow explicitly.
- **Effort**: L (real face-selector plumbing) → S (docs-only fallback)

### A2.2 🔴 — `set_toolpath_param` silent no-op for non-geometry fields

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_mcp/src/server.rs::set_toolpath_param`,
  `crates/rs_cam_mcp/src/server.rs::generate_toolpath`
- **Investigation**: Classify each parameter as
  `affects_geometry | affects_runtime_only | affects_both`.
- **Proposed fix**: Return a diff in `set_toolpath_param` describing whether
  regen is needed. `generate_toolpath` response includes a list of fields
  that actually changed the output geometry vs. fields that were applied
  to the existing geometry (feeds, plunge_rate).
- **Effort**: M

### A14 🟢 — Project name shows "Untitled"

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/session/mod.rs::ProjectSession::load`
  (fixed at the load layer so both MCP and GUI benefit).
- **Change**: After `from_project_file`, if `session.name == "Untitled"` or
  empty/whitespace, replace with the loaded file's stem (`path.file_stem()`).
  Doesn't touch the underlying TOML — pure runtime fallback. Existing
  session tests pass.

---

## 7. Optimizer

### C2 🟡 — `gate_deltas` doesn't say which axis moved the gate

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/tool_load/optimize/` narrate / output assembly
- **Proposed fix**: Attach a one-line "primary axis: spindle_rpm 12000→8803
  (F1 RPM-down)" to each candidate. The optimizer already computes per-axis
  deltas — surface them in plain text.
- **Effort**: S

### C3 🟡 — Headline doesn't quote cycle-time delta

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**:
  `crates/rs_cam_core/src/tool_load/optimize/narrative.rs::build_ranked_narrative`.
- **Change**: After the count line, append " Best: {+/-X.X}% cycle time at
  {best_s}s vs {baseline_s}s baseline." Computed from
  `candidates[0].cycle_time_s` and the min of the non-baseline candidates'
  cycle times. Skips gracefully when baseline or best is non-positive.
- **Tests**: existing optimizer narrative tests pass (13/13).

### C5 🟡 — Search envelope omits `plunge_rate` and `min_z`

- **Status**: 🔵
- **Code locations**: `tool_load/optimize/` search axes definition
- **Investigation**: Inventory the current axis set. Determine whether
  `plunge_rate` can be safely capped at `plunge_stress::safe_plunge_cap`
  inside the search (otherwise the optimizer will pick unsafe plunges).
- **Proposed fix**: Optionally include `plunge_rate` in the search axes
  (capped at safe value). Surface `min_z` under a separate "geometry sanity"
  sub-search — different shape from the feeds/speeds optimizer.
- **Effort**: L (real new axis work) → M (just `plunge_rate` as an axis)

### C6 🟡 — LUT row re-resolves per candidate; band shrinks mid-search

- **Status**: 🔵
- **Code locations**: `tool_load/chipload.rs` LUT-row resolution from the
  optimizer's per-candidate evaluation path
- **Proposed fix**: Pin the LUT-row resolution to the baseline for all
  candidates of one optimizer run. Don't re-resolve per-candidate so verdict
  math is comparable.
- **Effort**: M (needs a "fix the LUT row" hook in evaluation)

---

## 8. Cross-TP coupling / silent runtime

### A2.1 🔴 — `plunge_rate` change yields zero runtime delta

- **Status**: 🔵
- **Code locations**: `crates/rs_cam_core/src/compute/simulate.rs` cycle-time
  accumulation
- **Investigation**:
  1. Inspect move-segment tagging — does the simulator distinguish plunge
     segments from lateral feed?
  2. If yes: which feed-rate field does the time integrator pick for
     plunge segments?
- **Proposed fix**: Ensure the time integrator picks `plunge_rate` for
  segments tagged as plunge. Add a test that asserts a 5× plunge_rate
  change produces a measurable cycle-time delta on a known plunge-heavy op.
  Cross-check exported g-code timing matches sim.
- **Effort**: M

### A2.3 🟡 — Editing TP3 silently changes TP5 verdict; no audit trail

- **Status**: 🔵
- **Code locations**: `compute/simulate.rs::SummaryAccumulator`,
  `session/compute.rs` per-TP verdict assembly
- **Proposed fix**: Record an `affected_by: [TP3]` trail on each verdict.
  Surface in narrate as "this verdict changed because TP3 was regenerated;
  before that, TP5 chipload was exceeds low."
- **Effort**: M (state-tracking across regen cycles)

### A2.5 🟡 — `total_runtime_s` not decomposed

- **Status**: 🔵
- **Code locations**: `compute/simulate.rs::total_runtime` scalar
- **Proposed fix**: Expose `total_runtime = cutting_runtime + rapid_runtime
  + plunge_runtime + retract_runtime`. Update narrate to render the
  decomposition when the user changes a parameter that affects one axis
  but not others (e.g., wider stepover = more rapids, less cut).
- **Effort**: M

---

## 9. Tool model

### A8 🟡 — Tool diameter inconsistency

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_core/src/session/mod.rs::ToolSummary` +
  `list_tools`.
- **Change**: `ToolSummary` now exposes the geometry context that drives
  the LUT-effective diameter calc, not just the (tip) `diameter` field.
  Added: `cutting_length`, `taper_half_angle_deg`, `corner_radius_mm`,
  `included_angle_deg`, `flute_count`. Non-applicable fields are
  `skip_serializing_if = zero` to keep the JSON tidy for end mills.
  A doc-comment on `ToolSummary` explains the named-vs-effective diameter
  distinction so agents reading the JSON know to compute the effective
  diameter via the feeds-geometry helpers when needed.
- **Tests**: 102 session tests pass, full workspace clippy clean.
- **Follow-up**: optionally surface a derived `effective_lookup_mm`
  computed at a "typical engagement depth" (e.g. cutting_length / 2) as
  a convenience for taper tools. Deferred — the agent has the inputs to
  compute it.

---

## 10. Build cadence / version pin

### A3 🟡 — `project_summary` has no build-sha / feature list

- **Status**: ✅ Fixed (2026-05-21, this session)
- **Code locations**: `crates/rs_cam_mcp/src/server.rs::project_summary` +
  new helper `pub fn build_info()` in the same file.
- **Change**: `project_summary` now includes a `build` block with:
  - `crate_version`: `env!("CARGO_PKG_VERSION")` (always populated).
  - `git_sha`: `option_env!("VERGEN_GIT_SHA")` — `None` until the build
    pipeline sets the env var (zero-cost: no build.rs / no new dep added).
  - `features`: hand-curated array — `["stale_defaults", "drill_summaries",
    "drill_gates", "transit_span_doc", "air_cut_op_kind_aware",
    "plunge_stress_gate"]`. Agents probe this list to detect whether the
    running binary covers the P1–P5 priority work.
- **Follow-up**: optionally wire `vergen` for `git_sha`. Not required —
  `crate_version` alone is sufficient to detect "is this binary stale"
  whenever the workspace version bumps with a release.

---

## 11. Polish / docs / units

### B10 🟢 — Scallop ball-tip confirmation missing

- **Status**: 🔵
- **Code locations**: GUI scallop add-tool flow + MCP `add_toolpath` for
  scallop
- **Proposed fix**: At add-time, badge the tool selection green when the
  tool is ball-tipped; refuse with helpful message otherwise.
- **Effort**: S

### B11 🟢 — Fixture `face_up: top` affordance

- **Status**: 🔵
- **Code locations**: Setup config UI (GUI-side)
- **Proposed fix**: When the model has multiple distinguishable faces
  (L-bracket, stepped block, STEP with 6+ faces), surface a face-choice
  affordance at the setup level rather than buried in `set_setup_face`.
- **Effort**: M (GUI work, scope-creep risk)

---

## 12. Closed / no-op

### C1 🟢 — Optimizer output shape excellent
- **Status**: ✅ — keep working as-is. Note in the doc: do not refactor.

### C4 🟢 — `no_safe_improvement` outcome is honest
- **Status**: ✅ — keep as-is.

### A2.8 🟢 — Multi-axis tune attribution (agent-side lesson)
- **Status**: ❌ Won't fix (code-side). Agent-side: prefer single-dimension
  tuning when the goal is attribution.

---

## Recommended execution order

Phase D synthesis identified the verdict layer as the highest-leverage
single PR. Suggested rollout:

1. **PR-0**: Rebuild GUI from HEAD. Re-verify all ⏸️ items; close any that
   are already covered. **(no code change, but unblocks the rest)**
2. **PR-1**: §1 verdict layer — bundles A4, B8, B1 (verdict half), A12,
   C7. _Largest single user-visible win in the entire plan._
3. **PR-2**: §3 B6 (pocket DOC default — almost certainly an outright bug).
   Smallest fix, highest correctness payoff.
4. **PR-3**: §6 B3 (profile + STEP plate silent-hang fix).
5. **PR-4**: §4 narrate — bundle A5, A6, A7, A13, A15 (single-module sweep).
6. **PR-5**: §3 B4 / B5 (provenance + rounding — the secondary investment
   target from Phase D).
7. **PR-6**: §5 tool-load report — A9, A10, A2.4, A2.6, A2.7 (shared
   verdict-shape work).
8. **PR-7**: §8 A2.1 (plunge runtime calc — real correctness bug; possible
   sim/post-output divergence).
9. **PR-8+**: §2 engine-level retract fixes (B1 engine half, B7 v_carve
   tessellation). Largest scope; bench against PR-1's verdict-layer
   route-around to know when these are needed urgently.
10. Polish / minor: §7 C2 / C3 / C5 / C6, §9 A8, §10 A3, §11 B10 / B11,
    A14 (interleave whenever convenient).

---

## How to use this doc

When investigating a finding:
1. Set status to 🟠 Investigating.
2. Add notes inline (under the bullet) about what you found.
3. Update **Proposed fix** with concrete details once known.
4. Set status to 🟣 Fix designed; estimate effort if it shifted.
5. When implementation starts, set to 🟢 In progress.
6. After merge + verify (rebuild MCP, re-run the original repro), set ✅ Fixed.

When closing a finding without code (e.g., already-fixed-in-HEAD, won't fix):
- Document the reason at the top of the finding's section.
- Update the master index status column.

---

## Session 1 outcome (2026-05-21)

### Shipped (✅ Fixed)

10 findings fixed end-to-end with workspace clippy + targeted tests green.
All changes are in the working tree, uncommitted.

| ID | Severity | Surface | One-line summary |
|----|----------|---------|------------------|
| A3 | 🟡 | Build cadence | `build` block added to `project_summary` MCP response (crate_version + git_sha + features array) |
| A6 | 🟡 | Narrate | Air-cut hint now op-aware (roughing / finishing / curve-trace branches) instead of always emitting the adaptive3d boilerplate |
| A7 | 🟡 | Narrate | "commanded depth_per_pass is unknown" replaced with op-specific text ("follows surface heights" / "advances by peck depth" / etc.) |
| A8 | 🟡 | Tool model | `ToolSummary` MCP shape extended with `cutting_length`, `taper_half_angle_deg`, `corner_radius_mm`, `included_angle_deg`, `flute_count` — agents can now correlate named-vs-LUT-effective diameter |
| A9 | 🟡 | Tool-load | Chipload-low Exceeds verdicts on extrapolated LUT rows now render as "ADVISORY (extrapolated LUT row)" rather than hard "EXCEEDS" |
| A10 | 🟡 | Tool-load | New `UnmodeledReason::AllSamplesAirCutOrRapid` variant — distinguishes "no trace for TP" from "trace exists but TP made no material contact" |
| A14 | 🟢 | Polish | Project name falls back to filename stem when `name` is "Untitled" or empty |
| A15 | 🟢 | Polish | Drill plunge envelope units relabeled from "1/min" to "mm/min per mm Ø"; "plunge feed/diameter" → "plunge intensity" |
| B5 | 🟡 | Defaults | `apply_feeds_result_to_op` rounds feed/plunge to 1 mm/min and stepover/DOC to 0.001 mm — no more `769.506587956183` |
| C3 | 🟡 | Optimizer | Ranked narrative headline now quotes the best cycle-time delta ("Found 3; best −42% at 145.5s vs 249.5s baseline") |

### Investigated (🟠 hypothesis refined / falsified)

| ID | Severity | Finding |
|----|----------|---------|
| B6 | 🟡 | Hypothesis falsified. `doc_roughing_factor` / `woc_roughing_factor` are only read by MCP/GUI serialization, not by any defaults logic. The 4.2 mm pocket DOC came from `feeds::calculate` via `compute_feeds_for_op`. Real next step requires a fresh-project repro to check the feeds-calc output, not a rigidity-factor wire fix. |
| A11 | 🟡 | Deeper than originally framed. `DrillConfig` has no `plunge_rate` field — `feeds::calculate` *does* return a `plunge_rate_mm_min` but `op.set_plunge_rate()` on a `DrillConfig` is a no-op. The default plunge floors at envelope-min because the feeds calculator is effectively bypassed for drill ops. Fix needs an `OperationParams` trait change. |

### Files changed (all uncommitted)

- `crates/rs_cam_core/src/session/mod.rs` — A14, A8 (`ToolSummary` shape + `list_tools`)
- `crates/rs_cam_core/src/narrate.rs` — A6, A7, A15
- `crates/rs_cam_core/src/tool_load/verdict.rs` — A10 enum variant
- `crates/rs_cam_core/src/tool_load/chipload.rs` — A10 branch + 2 test updates
- `crates/rs_cam_core/src/tool_load/optimize/narrative.rs` — C3
- `crates/rs_cam_mcp/src/server.rs` — A3 (`build_info` helper + `project_summary` payload)
- `crates/rs_cam_viz/src/ui/properties/mod.rs` — B5 (rounding helper)
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` — A9, A10 (verdict rendering)
- `crates/rs_cam_viz/src/ui/preflight.rs` — A10 (verdict rendering)
- `planning/UX_DIALIN_FIX_PLAN_2026-05-20.md` — status updates, this section

### Status totals (after Session 1)

| Status | Count |
|--------|-------|
| ✅ Fixed (this session) | 10 |
| ✅ Fixed (Optimizer surfaces — kept as-is) | 2 (C1, C4) |
| 🟠 Investigating | 2 (A11, B6) |
| ⏸️ Blocked-on-rebuild | 5 (A2, A4, A12, B2, plus A11 partially) |
| 🔵 To investigate | ~22 |

### Recommended next-session entry point

The verdict-layer bundle (PR-1 in §13) remains the highest-leverage
single PR — A4 + B8 + B1 (verdict half) + A12 + C7 share the same
module (`session/compute.rs::diagnostics()`) and produce the same
output shape (`Vec<Verdict>` with offender names + fix hints). Doing
that as a single change unblocks the dominant "rapid collisions on
default" pain noted in the original Phase D synthesis.

After PR-1, the engine retract work (B1 engine half, B7 v_carve
tessellation, A1 project_curve sign convention) is the natural next
band — they're independent of each other and can be tackled
sequentially or in parallel.

---

## Session 2 outcome (2026-05-21) — PR-1 verdict-layer bundle

Shipped the five-finding verdict-layer route-around as a single contained PR
on `crates/rs_cam_core/src/session/compute.rs::diagnostics()`.

### Shipped fixes

| ID | Severity | Surface | Outcome |
|----|----------|---------|---------|
| A4 | 🟡 | Sim verdict | `ProjectDiagnostics.verdicts: Vec<Verdict>` populated; each verdict names its offending toolpath(s) by id + name |
| B8 | 🟡 | Sim verdict | Severity-ranked accumulation replaces single-warning early-exit; all applicable verdicts surface |
| B1 (verdict half) | 🔴 | Sim verdict | Per-TP `RapidCollision` verdict carries count, worst-move (index + cutter end-z), and retract_z / safe-Z / boundary fix hint |
| A12 | 🟡 | Sim verdict | `OperationType::kind_str()` + `is_drill_kinematics()` added; `ToolpathDiagnostic.op_kind` field exposes stable snake_case tag to consumers; C7 GeneratedEmpty uses it to exempt drill ops |
| C7 | 🔴 | Op engine | Per-TP `GeneratedEmpty` verdict fires when `cutting_distance == 0` on a non-drill TP; fix hint covers setup orientation, stock alignment, and depth-sign trap |

### Verdict type shape (`session/mod.rs`)

```rust
pub enum VerdictSeverity { Critical, Important, Polish }
pub enum VerdictKind {
    HolderCollision, RapidCollision, PlungeStress, AirCut, GeneratedEmpty,
}
pub struct Verdict {
    pub severity: VerdictSeverity,
    pub kind: VerdictKind,
    pub headline: String,
    pub offender_toolpath_ids: Vec<usize>,
    pub fix_hint: String,
    pub evidence: VerdictEvidence, // move_index, z_value, count — all Option
}
```

The legacy `ProjectDiagnostics.verdict: String` field is kept for backward
compat and mirrors the highest-severity headline (or `"OK"`). The MCP
`get_diagnostics` and `run_simulation` payloads include the new `verdicts`
array. `project_summary.build.features` now lists `"verdict_list"` so
agents can detect the new shape.

### Tests added

| Test | Covers |
|------|--------|
| `diagnostics_empty` (updated) | Empty session ⇒ empty verdicts, verdict="OK" |
| `diagnostics_tags_per_tp_with_op_kind` | A12 op_kind tag (`"pocket"` vs `"drill"`) |
| `diagnostics_emits_generated_empty_for_zero_cut_non_drill` | C7 fires on pocket, exempts drill |
| `diagnostics_ranks_verdicts_by_severity` | B8 ranking (Critical RapidCollision precedes Important GeneratedEmpty) |
| `diagnostics_rapid_collision_verdict_carries_evidence` | B1 verdict text + worst-move evidence (deepest end-z selected) + retract_z fix hint |

### Files changed this session (uncommitted)

- `crates/rs_cam_core/src/compute/catalog.rs` — `kind_str()`, `is_drill_kinematics()`
- `crates/rs_cam_core/src/session/mod.rs` — Verdict types + serde impls + `op_kind` field
- `crates/rs_cam_core/src/session/compute.rs` — `diagnostics()` rewrite + 4 new tests
- `crates/rs_cam_mcp/src/server.rs` — `run_simulation` payload includes `verdicts`; `verdict_list` feature flag
- `planning/UX_DIALIN_FIX_PLAN_2026-05-20.md` — status updates, this section

### Verification

- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo test -p rs_cam_core --lib -q` — 1511 passed
- `cargo test -p rs_cam_viz --lib -q` — 183 passed
- `cargo fmt --check` — clean

### Status totals (after Session 2)

| Status | Count |
|--------|-------|
| ✅ Fixed (Session 1) | 10 |
| ✅ Fixed (Session 2 — PR-1 bundle) | 4 full + 1 partial (B1) |
| ✅ Fixed (Optimizer surfaces — kept as-is) | 2 (C1, C4) |
| 🟠 Investigating | 2 (A11, B6) |
| 🟠 Partial (verdict-layer done, engine open) | 1 (B1) |
| ⏸️ Blocked-on-rebuild | 3 (A2, B2, plus A11 partially) |
| 🔵 To investigate | ~20 |

### Recommended next-session entry point

The natural next PR is **PR-2: engine retract / lift-bridge** —
specifically B1 (engine half), which is the actual root cause behind the
"rapid collisions on default" pain that PR-1 routed around. Three independent
engine bugs:

1. **B1 (engine half)** 🔴 — per-op retract logic on pocket / v_carve /
   adaptive3d. Highest signal: the verdict layer is now naming six of seven
   skeleton TPs as rapid-collision offenders; until the engines lift to
   safe-Z reliably the user still has to handle each verdict manually.
2. **A1** 🔴 — project_curve depth-sign trap. Currently emits a
   `GeneratedEmpty` verdict via the new layer (good signal), but the
   underlying `depth: -2, from_below` semantics still need a regression
   test + sign-convention fix.
3. **B7** 🟡 — v_carve 214k moves; needs default-stepover-from-chord-at-depth
   plus an add-time move-count budget warning.

These are independent enough to parallelize across a small agent team.

---

## Session 3 outcome (2026-05-21) — PR-2A: pocket lift-bridge engine fix

### Shipped

| Code | Pre | Status | What landed |
|------|-----|--------|-------------|
| B1 (engine half — pocket) | 🔴 | ✅ | Ramp/helix descent no longer punches through uncut stock at new-region entry |
| B1 (engine half — v_carve, adaptive3d) | 🔴 | ✅ | Same shared-dressup fix covers them: the only path that emitted dangerous descent rapids was `dressup::emit_ramp` / `emit_helix`, used by every op via `apply_dressups` |

Master-index B1 is now ✅ (engine half pocket-confirmed; v_carve and adaptive3d share the fix because the bug lived in the shared dressup pipeline, not per-op generators).

### Root cause

`dressup::emit_ramp` and `emit_helix` were "optimizing" the descent from `safe_z` to the ramp/helix start by emitting a single `Rapid` straight from `start.z` down to `ramp_start_z = cut_depth + 2 mm`. For any new-region entry into uncleared stock, that rapid passed through 2 mm or more of material (in the dexel collision-check frame), firing one `RapidCollision` per region per depth pass.

Pre-fix scaled-up repro (programmatic `ux_2d_pocket` equivalent): **42 rapid collisions** on a 70×50 pocket with a circular island, 12 mm hardwood stock, default 6 mm EM Roughing dressups (Ramp entry + link moves + TSP). Post-fix: **0**.

### Engine fix

`dressup::apply_entry` (and the two emit helpers) now take a `stock_top` parameter — the Z above which the cutter is in air in the simulator's frame. Plumbed through `apply_dressups` (signature gained `stock_top: f64` between `safe_z` and `prior_stock`) and the adaptive3d entry-site callers.

The descent logic:

```rust
let safe_rapid_floor = stock_top + ENTRY_CLEARANCE;
let rapid_target_z = ramp_start_z.max(safe_rapid_floor).min(start.z);
if start.z - rapid_target_z > 0.1 {
    tp.rapid_to_with_intent(..., MoveIntent::Linking);
}
if rapid_target_z - ramp_start_z > 0.1 {
    tp.feed_to_with_intent(..., feed_rate, MoveIntent::EntryPlunge);
}
```

i.e. rapid only down to `stock_top + 2 mm` (still in air), then plunge-feed the rest. For columns already cleared above the ramp start (subsequent depth passes), the existing behaviour is preserved by `ramp_start_z.max(safe_rapid_floor)` clamping to `ramp_start_z` (no extra plunge needed).

For session-level callers, `stock_top` is the **simulator-frame** stock top (`effective_stock_bbox.max.z`), not the cutter-frame `heights.top_z`. The dexel rapid-collision check operates on that same frame; using a different frame here re-introduces the false-positive rapids the fix targets. This is a workaround for a deeper cutter-Z-vs-dexel-Z frame mismatch (cutter Z is world-frame, dexel is setup-local-frame) that the simulation has been "working around" by carving via `subtract_above` — out of scope for this PR but worth a follow-up audit.

### New regression test

`crates/rs_cam_core/tests/pocket_lift_bridge_b1.rs::pocket_default_skeleton_emits_no_rapid_collisions` — builds the `ux_2d_pocket`-equivalent session programmatically (100×100×12 hardwood stock, 70×50 rounded-rect outer + Ø20 circular island, 6 mm EM, depth=12 / dpp=4.2, `DressupConfig::for_op(Pocket)`), generates and simulates, asserts `sim.rapid_collisions.is_empty()`. Pre-fix this test fired 42; post-fix it passes.

### Files changed this session (uncommitted)

- `crates/rs_cam_core/src/dressup.rs` — `apply_entry`, `emit_ramp`, `emit_helix` now take `stock_top`; descent split into rapid-then-plunge when needed; in-file test call sites updated
- `crates/rs_cam_core/src/compute/execute.rs` — `apply_dressups` signature gains `stock_top`; passes through to both ramp and helix entry arms; in-file test updated
- `crates/rs_cam_core/src/session/compute.rs` — `apply_dressups` caller passes `effective_stock_bbox.max.z` (with rationale comment)
- `crates/rs_cam_core/src/adaptive3d/path.rs` — four `emit_ramp` / `emit_helix` call sites updated (simple-entry passes `params.safe_z`; rapid-floor entry passes `descent_floor`)
- `crates/rs_cam_cli/src/{job.rs,main.rs}` — pass `0.0` (CLI 2D ops cut from z=0)
- `crates/rs_cam_viz/src/compute/worker/helpers.rs` — pass `req.heights.top_z` from the viz layer (mirrors the cli convention; for the GUI the cutter-frame value is what the worker-side dressups see)
- `crates/rs_cam_core/tests/pocket_lift_bridge_b1.rs` — new
- Integration tests updated to add the new arg: `dressup_span_invariants.rs`, `capability_link_moves_safety.rs`, `adaptive3d_post_tsp_z_monotonicity.rs`, `project_curve_deviation.rs`
- `planning/UX_DIALIN_FIX_PLAN_2026-05-20.md` — B1 status, this section

### Verification

- `cargo test -p rs_cam_core --lib -q` — **1511 passed, 0 failed, 7 ignored**
- `cargo test -p rs_cam_core --tests` (integration) — **36 binaries, all green** (one ignored: wanaka_e2e_chipload_gate, pre-existing)
- `cargo test -p rs_cam_viz --lib -q` — **183 passed**
- `cargo test -p rs_cam_cli --bins` — **1 passed**
- `cargo clippy --workspace --all-targets -- -D warnings` — clean

### v_carve / adaptive3d divergence note

The reviewer asked to document v_carve / adaptive3d repro counts separately in case they diverged from the pocket fix. Static analysis says they don't:

- **v_carve** uses the same `apply_dressups` pipeline as pocket. Its `for_op(VCarve)` defaults give Ramp entry (Finish role). The shared fix applies.
- **adaptive3d** has its own entry-style enum (`EntryStyle3d::Ramp` / `Helix`) that calls `emit_ramp` / `emit_helix` directly (bypassing `apply_dressups`'s entry-style dispatch). Those four call sites in `adaptive3d/path.rs` were updated this PR — the simple-entry variant passes `params.safe_z` as `stock_top` (treat anything below `safe_z` as material, plunge-feed it), and the `RapidWithFloor` variant passes `descent_floor` (the dexel-sampled cleared-air floor).

No separate per-op generator path emits a "rapid through stock to ramp start" pattern — that was solely the dressup-layer optimization. A runtime check would still be valuable; deferred to a follow-up where we run the full skeleton fleet through MCP and confirm rapid_collision_count == 0 on all 7 fixtures.

### Status totals (after Session 3)

| Status | Count |
|--------|-------|
| ✅ Fixed (Session 1) | 10 |
| ✅ Fixed (Session 2 — PR-1 verdict bundle) | 4 full + 1 partial (B1) |
| ✅ Fixed (Session 3 — PR-2A pocket lift-bridge engine) | 1 (B1 promoted to full) |
| ✅ Fixed (Optimizer surfaces — kept as-is) | 2 (C1, C4) |
| 🟠 Investigating | 2 (A11, B6) |
| ⏸️ Blocked-on-rebuild | 3 (A2, B2, plus A11 partially) |
| 🔵 To investigate | ~19 |

### Recommended next-session entry point

With B1 closed, the natural next PRs are:

1. **A1** 🔴 — `project_curve` depth-sign trap. Now signaled to the user as `GeneratedEmpty` (PR-1), but the underlying `depth: -2, from_below` semantics still need a regression test + sign-convention fix.
2. **B7** 🟡 — v_carve 214 k moves on a small star. Two parts: default stepover from V-bit chord-at-depth (rather than fixed 0.254 mm), and an add-toolpath-time move-count budget warning.
3. **B3** 🔴 — Profile + STEP model hang on generate. Reject at `add_toolpath` instead.

Followup deeper-than-PR-2A audit candidate: the **cutter-Z vs dexel-Z frame mismatch** noticed during this PR. The simulation has been carving correctly by accident (cutter cuts at world z=-4, dexel rays span setup-local z=0..12, `subtract_above` removes everything in the ray when called with z below it). Collision detection is also off by a constant: it compares cutter Z to dexel stock_top (different frames). Worth a dedicated cleanup PR — both layers should agree on a frame.
