# ARCFIT_INTENT_EVIDENCE.md — W2 / H2.2 evidence package

**Wave:** W2 (R7 / H2.2 arcfit intent). **PR-5 in the merge order: evidence
harness, no output change.**
**Blocking checkpoint:** F1 — arcfit semantic boundary.
**Parent/revision measured:** `894e0603a58f25da703159aa9a08d802e371f835`
(branch `experiment/adaptive-spiral`).
**Instrument committed by this wave:**
`crates/rs_cam_core/tests/arcfit_intent_boundary_f1.rs`.

Nothing in this wave changes what arcfit emits. Every fact below was
measured on the revision named above.

---

## 0. One-paragraph statement of the defect

`arcfit::fit_arcs` decides which consecutive moves may be collapsed into one
G2/G3 using a run key of **(move type is `Linear`, feed rate within
`FEED_EPS`, no span barrier strictly inside the window)**. `Move::intent` is
not in that key. When a run is collapsed, the emitted arc takes its intent
from the **first** source move (`crates/rs_cam_core/src/arcfit.rs:191-194`).
So a homogeneous `FinishingCut` run that continues, at the same feed, into
the `LeadOut` arc `apply_lead_in_out` just appended is collapsed into one arc
labelled `FinishingCut` whose target is a **lead-out** position — geometry the
finishing pass never cut. This is repository-wide relabelling: it invalidates
any population selected by a post-arcfit intent label, which is how wave 11 of
the previous programme read 21 relabelled lead-out endpoints as "21 dropped
cut positions" and blamed `relink_fragments`
(`crates/rs_cam_core/tests/scallop_intra_pass_relink_am7.rs`, header).

---

## 1. Run-key analysis

### 1.1 The key today, read from source

`fit_arcs_with_provenance` (`crates/rs_cam_core/src/arcfit.rs:57-257`) builds
a candidate window `[i, end_idx)` as follows:

| # | Term | Source | In the key? |
|---|---|---|---|
| 1 | `MoveType::Linear` | `arcfit.rs:102`, `:128-133` | **yes** — any `Rapid`/`ArcCW`/`ArcCCW` breaks the run |
| 2 | `feed_rate` equal within `FEED_EPS` (1e-6 mm/min) | `arcfit.rs:129`, `condition.rs:31` | **yes** |
| 3 | Span barrier — `RapidOrderBarrier` **or** any `DepthPass.start_move` | `arcfit.rs:66-70`, `:140-145`, `toolpath_spans.rs:637-649` | **yes, but only when `spans_valid`** |
| 4 | Z | `try_fit_arc` acceptance, `arcfit.rs:460-495` | **no** — Z is a *geometric* acceptance test (constant-Z, or a true helix linear in swept angle), not a run-key term. Deliberate (Phase 2: helical entries and spiral descents must arc-fit) |
| 5 | `Move::intent` | — | **NO — this is the defect** |
| 6 | Semantic scope (`SpanKind::Region`, `Entry`, `LeadOut`, `LinkBridge`, `WaterlineCleanup`) | — | **no** — only the two barrier kinds in row 3 participate |

The collapsed arc's own intent: `let arc_intent = moves[i].intent;`
(`arcfit.rs:194`) — first source move wins, unconditionally.

### 1.2 Why the defect is intermittent — which boundaries are protected, and by what

An intent boundary is invisible to today's key **unless something else in the
key happens to change at the same index**. That accident is the only thing
protecting the codebase now:

| Boundary | Protected today by | Verdict |
|---|---|---|
| `Retract` ↔ anything | **Move type.** Every production `Retract` is a `Rapid` (`surface_link.rs:368`, `scallop.rs:2345`, `pencil.rs:1138`, `steep_shallow.rs:486`, `adaptive3d/path.rs:1232`). Grep for a `feed_to_with_intent(.., Retract)` in `crates/rs_cam_core/src` returns test code only | **safe by construction** — but by coincidence of emission style, not by rule |
| `Drilling` ↔ anything | Geometry. Drill plunges are pure-Z; `try_fit_arc` rejects a chord of zero XY length (`arcfit.rs:319-324`) | **safe in practice** |
| `EntryPlunge`/`LeadIn` ↔ `FinishingCut` | **Feed only.** Lead-in falls back to the plunge feed — `let li_feed = lead_in_feed_rate.unwrap_or(plunge_rate);` (`dressup.rs:854`) — and cuts carry `feed_rate`. Nothing enforces that plunge and cut feeds differ | **accidental** — set plunge feed = cut feed and the boundary opens |
| **`FinishingCut` ↔ `LeadOut`** | **Nothing.** `apply_lead_in_out` falls back to the cut pass's own feed when `lead_out_feed_rate` is `None` — `let lo_feed = lead_out_feed_rate.unwrap_or(cut_feed_rate);` (`dressup.rs:940`) — and `DressupConfig::default()` ships `lead_out_feed_rate: None` (`compute/config.rs:1458`) | **UNPROTECTED ON DEFAULTS** |
| **`ClearingCut`/`FinishingCut` ↔ generator-emitted `Linking`** | **Nothing.** Stay-down links are emitted at the *cut* feed: `surface_link.rs:360,364` (`params.feed_rate`), `adaptive3d/path.rs:1091-1098` (`feed_rate`), `pencil.rs:1130-1132` (`params.feed_rate`) | **UNPROTECTED** |
| `ClearingCut` ↔ **dressup**-inserted `Linking` | Feed. `apply_link_moves` uses `link_feed_rate` (default 500.0, `config.rs:1461`) | accidental |

**Precision about "unprotected".** The table says what the *run key* does, not
what the *fitter* does. An unprotected boundary means the key does not break
the run there; whether an arc actually forms across it still depends on
`try_fit_arc` accepting the geometry. For `FinishingCut → LeadOut` it demonstrably
does — the lead-out arc is tangent to the cut by construction, and §2.2 measures
the collapse. For `ClearingCut → Linking` on adaptive3d's keep-down links
(`adaptive3d/path.rs:1086-1100`: ascend at fixed XY, traverse at `link_z`,
descend) the geometry is much less likely to be co-circular with the preceding
cut, so the *frequency* is unknown and this wave did not measure it. Both are
key defects; only the first has a measured incidence.

### 1.3 Reachability of the unprotected boundaries on shipped defaults

`DressupConfig::for_role` (`compute/config.rs:1505-1511` for the `Finish` arm):

* `Finish` → `lead_in_out: true` **and** `arc_fitting: true`;
* `Roughing` / `SemiFinish` → `arc_fitting: true`, `lead_in_out: false`.

`normalize_for_op` strips lead-in/out and link moves only for the three ops
carrying `DressupPolicy::strip_all` — DropCutter, UnifiedFinish, ProjectCurve
(`compute/catalog.rs:1748, 1870, 1988`). **It does not strip `arc_fitting`.**

Therefore, on defaults:

* **11 Finish-role operations** get the unprotected `FinishingCut → LeadOut`
  boundary: VCarve, Inlay, Trace, Chamfer, Pencil, Scallop, SteepShallow,
  RampFinish, SpiralFinish, RadialFinish, HorizontalFinish.
* **UnifiedFinish** has lead-in/out stripped but still arc-fits, and
  `surface_link` emits its stay-down `Linking` moves at the cut feed — so it
  gets the unprotected `FinishingCut ↔ Linking` boundary instead.
* **Adaptive3d** (Roughing, `arc_fitting: true`) gets the unprotected
  `ClearingCut ↔ Linking` boundary through its own keep-down links.

### 1.4 Proposed run key

Add intent as an equality term; leave everything else alone:

```
run key := (MoveType is Linear, feed_rate ±FEED_EPS, intent ==, no barrier inside)
```

Arcs that must still be allowed to form **across** a boundary, with rationale:

| Boundary | Allow crossing? | Rationale |
|---|---|---|
| Z change within one intent | **yes** | Helix acceptance is a *geometric* test and already correct. Helical entries (`EntryHelix`) and spiral descents are homogeneous runs; splitting them would undo Phase 2 and re-inflate G-code |
| Same intent, same feed, across a `Region` span boundary | **operator decision — see §6 Q2** | Not currently a barrier. If an arc straddles two `Region` spans, both regions' `move_range`s land on the same index and stop tiling — the exact invariant `region_node_ranges_tile_the_stitched_toolpath` (`unified_finish.rs:3309`) was added to protect |
| `Unknown` ↔ any tagged intent | **operator decision — see §6 Q3** | `Unknown` is the legacy-generator fallback (`toolpath.rs:78`). Several families still emit whole passes through `Toolpath::feed_to` (intent `Unknown`). A strict intent split would break runs at every `Unknown`/tagged seam and could cost arc count for no semantic gain |
| Any other intent pair | **no** | That is the fix |

### 1.5 Adjacent defect of the same class — NOT FIXED, STATED

`condition::merge_linear_runs_with_provenance`
(`crates/rs_cam_core/src/condition.rs:86-107`) uses the **same** run key —
`Linear`, `feed_rate ± FEED_EPS`, barrier — and also omits intent. It runs
immediately after arcfit (dressup step 5b, `execute.rs:2961`) and is
default-on for the Roughing role.

It does **not** relabel: it keeps each retained move verbatim, so intents on
survivors are correct. But RDP drops points across intent boundaries and folds
a dropped move's geometry into the **next kept move**, which may carry a
different intent. The failure mode is milder (geometry attribution, not label
rewriting) but it is the same missing term.

**Owner:** unassigned. **Re-open condition:** any wave that selects a
population by intent on a `segment_merge`-enabled operation. It is out of
H2.2's scope; naming it here so it is not the next surprise.

---

## 2. The Checkpoint F1 mixed-intent red fixture

**File:** `crates/rs_cam_core/tests/arcfit_intent_boundary_f1.rs` — 4 tests,
all green at `894e060`, all pinning **current-defective** behaviour.

**Form chosen:** positive assertions of the defect, *not* `#[ignore]`. An
ignored characterization rots in silence; these fail loudly the moment the
H2.2 fix lands, which is exactly the red-first evidence PR-6 needs. Each test's
doc comment states how to invert it. This is stated in the file header so the
form cannot be mistaken for a passing contract.

### 2.1 What each exhibit demonstrates

| Test | Demonstrates |
|---|---|
| `f1_exhibit_mixed_intent_run_collapses_into_one_finishing_arc` | The run key in isolation. 18 co-circular moves at one feed, tagged 6 `FinishingCut` → 6 `LeadOut` → 6 `LeadIn`, identical geometry throughout. **Result: ONE arc, intent `FinishingCut`, targeting the last `LeadIn` point; `LeadOut` and `LeadIn` populations both drop to zero.** Nothing but intent distinguished the blocks, so nothing but the run key can explain the collapse |
| `f1_control_a_feed_change_already_splits_the_run` | The same geometry with the `LeadOut` block at 1800 vs 1000 mm/min splits into two arcs and keeps its `LeadOut` label. This is the control that proves the mechanism is the key, and it must **still pass unchanged** after the fix |
| `f1_exhibit_lead_out_is_swallowed_by_the_finishing_arc` | The shipped path: `apply_lead_in_out_with_feeds(.., lead_out_feed_rate: None)` then `fit_arcs` — dressup steps 3 then 5, in order, with the shipped defaults |
| `f1_exhibit_span_and_intent_contradict_on_the_collapsed_arc` | After the collapse, at least one move sits **inside a `SpanKind::LeadOut` span** while its own `intent` reads `FinishingCut`. The two channels that both answer "what is this move" contradict each other at the same index |

### 2.2 The measured mechanism (exhibit 2), which corrects a prior assumption

The commission assumed the whole lead-out is swallowed. **It is not.** Measured
output of the production chain (Ø12 circle, 5° steps, `lead_radius` = cut
radius, cut feed 1000, plunge 300, `tolerance` 0.05):

```
 5  Linear f1000  FinishingCut  (5.977,  0.523, -2)
 6  ArcCCW f1000  FinishingCut  (-1.174, 5.936, -2)   <-- 101.2 deg; cut ended at 90 deg
 7  ArcCCW f1000  LeadOut       (-6.256, 0.267, -2)
 8  Rapid         Retract
```

The greedy fitter extends the cut's own circle into the lead-out by **as many
segments as `tolerance` allows — here exactly one** — relabels that one
`FinishingCut`, then starts a fresh run at the next move, which *is* a
`LeadOut` and so produces a correctly-labelled `LeadOut` arc. Of 8 `LeadOut`
source moves: 1 relabelled, 7 collapsed into a surviving `LeadOut` arc.

Two consequences worth the operator's attention:

1. **One lead-out position relabelled per pass.** That is precisely the "one
   lost cut position per junction, 21 across 21 junctions" arithmetic wave 11
   mis-attributed to `relink_fragments`. This fixture reproduces the arithmetic
   from first principles.
2. **The relabelled arc's target is 1.18 mm off the machined surface**
   (measured: `‖arc.target − true_cut_end‖`). The field reading recorded in
   `scallop_intra_pass_relink_am7.rs` was "up to 1.2 mm off the machined
   surface". Independent agreement.

### 2.3 Verification

```
cargo test -p rs_cam_core --test arcfit_intent_boundary_f1
# 4 passed; 0 failed  (at 894e060)
```

---

## 3. Consumer census (impact before code)

Method: `rg` over `crates/*/src`, `crates/*/tests`, `crates/*/benches`, then
each hit classified as production or test module by comparing its line number
against the file's `#[cfg(test)]` offset, then each production hit read in
context. SocratiCode impact analysis was **not** used for blast radius
(`feedback_socraticode_quality`).

Totals: **7 direct**, **5 indirect**, **11 fingerprint / pinned-geometry
targets**. A further ~40 `MoveIntent::` sites are *producers* (generation-time
tagging) or in-module tests and are unaffected.

### 3.1 Direct — reads `Move::intent` on a post-arcfit toolpath

| # | Site | What it does | Impact of the relabelling | Re-pin needed |
|---|---|---|---|---|
| D1 | `feed_modulation.rs:326-344` `should_skip_modulation`, applied post-simulation via `session/compute.rs:2160+` | Skips modulating `LeadIn`/`LeadOut`/`EntryPlunge`/`Retract`/`Drilling` | **Behavioural.** A relabelled `LeadOut` arc is modulated — the F-040 contract that lead feeds are operator-tuned is silently broken. Opt-in (`adaptive_feed_modulation` defaults false) | re-run `adaptive_feed_modulation_pipeline_f036b`, `constrained_max_modulation_f039`, `lead_in_out_feed_rates_f040` |
| D2 | `machine_kinematics.rs:577-586` cycle-time breakdown | Attributes per-move time to `cutting_s` / `entry_s` / `linking_s` / `retract_s` | **Reported number.** Relabelled lead-out time lands in `cutting_s` instead of `linking_s` | cycle-time tests / `cycle_time_rebench` reference |
| D3 | `session/compute.rs:2368-2377` planner-engagement override | Applies `max(sim_radial, planner_woc)` on `ClearingCut`\|`FinishingCut` only | **Reported metric.** A relabelled arc receives a planner WOC floor it should not | engagement tests (`engagement_vector_step2`) |
| D4 | `dexel_stock/simulation.rs:181-190` `is_retract_feed` | Reclassifies a `Linear` move tagged `Retract` out of cutting metrics | **Not reachable today** — production `Retract` is always `Rapid`. Confirmed by grep. Park, do not fix | none |
| D5 | `session/compute.rs:2017-2027` and `rs_cam_viz/src/app/mcp.rs:1113-1120` | "is this a drill toolpath" = `any(intent == Drilling)` | **Not reachable today** — drill plunges are pure-Z and `try_fit_arc` rejects a zero-length XY chord | none |
| D6 | `tsp.rs:534` `intruder_intent` | Names the intruder in a `tracing::debug!` when a span is dropped | Diagnostic string only, no behaviour | none |
| D7 | `rs_cam_viz/src/render/toolpath_render.rs`, `app/gpu_upload.rs` | Viewport move colouring by intent | **Display.** Lead-outs render as cutting moves. Adjacent to D-LV.1 (H2.3) — do not couple | viz render tests |

### 3.2 Indirect — consumes a channel derived from intent, or a population selected upstream

| # | Site | Relationship | Note |
|---|---|---|---|
| I1 | `compute/spans.rs:276` `spans_from_move_intents`, called from `compute/execute.rs:353` | Runs at **generation** time, before any dressup. Its own output is correct | Arcfit then remaps those spans onto the collapsed arc, producing the span/intent contradiction of exhibit 4 |
| I2 | `tool_load/locality.rs:149-190` `is_steady_state_for_gate` / `is_phantom_transit` | Selects by **span ancestry**, and its phantom set **includes `DressupArtifact`** | Arcfit tags *every* fitted arc `DressupArtifact` (`arcfit.rs:239`). So every arc-fitted move is **already** dropped from gate peak/LUT populations, independent of the intent bug. Changing arc counts therefore changes gate populations. This is the single largest indirect coupling and the operator should be told about it explicitly |
| I3 | `compute/execute.rs:4098-4124` F2.1 assertion | "every transit-intent move sits inside a span of the mapped kind" | Asserted on **generated** output (pre-dressup), so it is green today and stays green |
| I4 | Generation-time intent selections in `surface_link.rs:702,743`, `unified_finish.rs:2312-2334,2595,3201`, `scallop.rs`, `pencil.rs`, `steep_shallow.rs`, `spiral_finish.rs`, `ramp_finish.rs`, `radial_finish.rs`, `adaptive3d/path.rs:1674`, `boundary.rs` | All run **before** dressups, or are in-module tests | Unaffected. Listed so the next reader does not re-derive it |
| I5 | `condition.rs:86-107` `merge_linear_runs` | Same missing run-key term, milder failure mode | §1.5 — out of scope, stated |

### 3.3 Fingerprints and pinned fitted geometry

| # | Target | Runs arcfit? | Expected on fix |
|---|---|---|---|
| F1 | **`tests/transform_provenance_fingerprints.rs`** — 3 tests, 5 pinned FNV constants + 4 link-site tables, all via `full_dressups()` (`lead_in_out` + `arc_fitting` + `segment_merge`) | **yes** | **Will move.** This is the primary re-pin. Baselines in §4 |
| F2 | `src/arcfit.rs` `mod tests` — 23 tests | yes | Every fixture builds moves with `Toolpath::feed_to`, i.e. a **homogeneous `Unknown`** run. Should be unaffected — but verify, do not assume |
| F3 | `tests/dressup_span_invariants.rs` — 4 tests | yes (`arc_fitting: true` in 4 places) | Invariants only, no pinned counts. Should stay green |
| F4 | `tests/contour_spiral_gcode_validity_phase0.rs:217,386` | yes, `fit_arcs` called directly | Generated spiral geometry, homogeneous intent. Verify |
| F5 | `tests/tool_scale_semantics_pr2.rs:474` | yes, `fit_arcs` called directly | Asserts the F.10 envelope-vs-cusp cap; orthogonal to intent. Verify |
| F6 | `tests/capability_link_moves_safety.rs` | **no** — builds on `DressupConfig::default()`, `arc_fitting: false` | Named in plan §4.2 as an H2 gate. Census result: **arcfit does not run in it.** Re-run as a control, expect no change |
| F7 | **`tests/scallop_intra_pass_relink_am7.rs`** | **no** — sets `arc_fitting: false` in two places *specifically to dodge this bug* | After the fix, those two `false`s are the honest re-enable candidate. This is the test that discovered the defect; closing the loop there is the strongest possible fix evidence |
| F8 | `tests/v3_cascade_ab.rs`, `tests/p2c_headless_ab_wanaka.rs` | yes (`arc_fitting = true` in fixtures/dials) | Long A/B harnesses. Re-run selectively; they are characterization, not pins |
| F9 | `tests/vcarve_lift_bridge_b1.rs` | yes (Finish role → lead_in_out + arc_fitting) | Verify |
| F10 | `crates/rs_cam_viz/src/compute/worker/tests.rs` — `debug_trace_records_arc_fit_and_feed_optimization_phases`, `semantic_trace_records_entry_params_and_boundary_clip`, `worker_reconciles_semantic_links_with_debug_options_disabled` | yes (3 explicit `arc_fitting = true`) | Trace/phase assertions; arc counts may shift |
| F11 | `benches/perf_suite.rs:356` `fit_arcs` bench | yes | Bench only; no gate |

**Census result that shrinks the blast radius:** `tests/param_sweep.rs` (56
sweeps) calls the **generators directly** and fingerprints raw toolpaths — it
never runs `apply_dressups`. **The parameter sweeps are not in the arcfit re-pin
scope.** Likewise `crates/rs_cam_core/tests/fixtures/test_job.toml` sets
`arc_fitting` on four ops but is consumed by CLI/project tests, not by a
fingerprint pin.

---

## 4. Baseline fingerprints at `894e060` (pre-fix, evidence-backed before-side)

Captured in this wave's Cargo window. Commands are the narrowest that reach
each target.

| Target | Command | Result at `894e060` |
|---|---|---|
| arcfit unit suite | `cargo test -p rs_cam_core --lib arcfit::` | **23 passed, 0 failed** |
| transform provenance fingerprints | `cargo test -p rs_cam_core --test transform_provenance_fingerprints` | **3 passed, 0 failed** |
| dressup span invariants | `cargo test -p rs_cam_core --test dressup_span_invariants` | **4 passed, 0 failed** |
| F1 exhibit (new) | `cargo test -p rs_cam_core --test arcfit_intent_boundary_f1` | **4 passed, 0 failed** |
| F1 exhibit lint | `cargo clippy -p rs_cam_core --test arcfit_intent_boundary_f1 -- -D warnings` | **exit 0, zero warnings** |
| control — scallop relink (the test that discovered the defect) | `cargo test -p rs_cam_core --test scallop_intra_pass_relink_am7` | **5 passed, 0 failed** |
| control — capability link-move safety | `cargo test -p rs_cam_core --test capability_link_moves_safety` | **17 passed, 0 failed** |

The pinned FNV constants that PR-6 must re-pin, read from
`tests/transform_provenance_fingerprints.rs` and **confirmed green** at
`894e060`:

| Fixture | Stage | `(moves, FNV-1a)` | Link sites |
|---|---|---|---|
| `three_pass` | dressups | `(23, 14_756_822_782_673_573_601)` | head `(0,4)`, body `(5,13)`, tail `(14,22)`, whole `(0,22)` |
| `arc_raster` | dressups | `(40, 9_877_459_821_106_430_315)` | head `(0,16)`, body `(16,27)`, tail `(27,39)`, whole `(0,39)` |
| `face_full_chain` | 1 — dressups | `(74, 9_692_869_450_022_244_402)` | — |
| `face_full_chain` | 2 — boundary clip | `(97, 3_258_911_278_473_560_309)` | — |
| `face_full_chain` | 3 — descent split | `split_count 6`, `(103, 2_154_614_841_165_484_301)` | head `(0,32)`, body `(33,74)`, tail `(75,102)`, whole `(0,102)` |

All five were originally captured at HEAD `5d32150` before the C1 wave; they
are unchanged at `894e060`, so the baseline is stable and the delta PR-6
produces is attributable to PR-6.

The last three rows were run in a second Cargo window, after the slot cleared;
commit `8963b75` had shipped without them and said so. Per §3.3 F6/F7 neither
`capability_link_moves_safety` nor `scallop_intra_pass_relink_am7` runs arcfit,
so neither is a pre-fix *fingerprint* — both are post-fix **controls**, and the
green readings above are the baseline PR-6 compares them against. **No check in
this wave is left unrun.**

---

## 5. Expected deltas — predictions, not measurements

Stated as falsifiable predictions so PR-6 can be scored against them.

1. **Arc count rises wherever an unprotected boundary exists.** Every
   `FinishingCut → LeadOut` seam on the 11 Finish-role ops of §1.3 becomes an
   extra break. Predicted magnitude: **+1 break per cutting pass per lead-out**,
   plus one per generator-emitted `Linking` seam on UnifiedFinish / Adaptive3d /
   Pencil / Scallop. Move count rises by roughly the same amount (the fitter
   emits one more arc where it previously emitted one).
2. **Total G-code length rises slightly on Finish ops, negligibly on Roughing.**
   Roughing has `lead_in_out: false`; its exposure is only the generator-emitted
   `Linking` seams. Do **not** expect the arc-count regression to be uniform
   across families — an uneven result is the expected shape, not a bug.
3. **`transform_provenance_fingerprints` moves on all three fixtures.**
   `three_pass` and `face_full_chain` both run `lead_in_out: true`; `arc_raster`
   runs it too and is explicitly built so arc-fit fires. Predicted direction:
   move counts **up**, hashes change, link sites shift by the added moves.
   `arc_raster`'s fixture uses `Toolpath::feed_to` (all `Unknown`) for the cut
   body, so its delta will come from the dressup-inserted `LeadIn`/`LeadOut`
   moves, not from the raster itself.
4. **Relabelled populations shrink; correctly-labelled populations grow.**
   Specifically: `count(intent == FinishingCut)` **falls** on Finish ops and
   `count(intent == LeadOut)` **rises** by the same number of *positions*
   (though not the same number of *moves*, because a surviving lead-out already
   collapses into one arc — see §2.2). Any wave that previously measured a
   `FinishingCut`-selected population on a `lead_in_out` + `arc_fitting`
   operation should expect its count to fall.
5. **Gate populations change through `DressupArtifact`, not only through
   intent.** More arcs ⇒ more `DressupArtifact` spans ⇒ **more** samples
   dropped by `is_phantom_transit` (§3.2 I2). Predicted second-order effect on
   tool-load gate peaks. This is a consequence of the fix that has nothing to do
   with intent, and it is the one most likely to be misread as a regression.
6. **No change at all** in: `param_sweep` (does not run dressups),
   `capability_link_moves_safety` (`arc_fitting: false`), any operation whose
   run is homogeneous `Unknown`, any Retract or Drill boundary, and the
   reflex-arc / F.10-cap behaviour (orthogonal mechanisms).
7. **`scallop_intra_pass_relink_am7`'s two `arc_fitting: false` opt-outs become
   removable.** If they cannot be removed after the fix, the fix is incomplete —
   that is the strongest single acceptance signal available.

---

## 6. What Checkpoint F1 must decide

Plan §3.4 requires: mixed-intent red fixture (§2, delivered), complete
impacted-fingerprint census (§3.3, delivered), expected fitted-geometry deltas
(§5, delivered), downstream intent-consumer audit (§3.1/§3.2, delivered).

**The decision: approve the output move and the re-pin scope.** Concretely,
four rulings are needed.

**Q1 — Approve the run key of §1.4?**
`(Linear, feed ±FEED_EPS, intent ==, no barrier inside)`. This is the fix-shape
the plan already prescribes ("break arc candidates at any intent boundary, then
derive an arc's intent from a homogeneous run"). Approving it authorises a
change to emitted machining geometry on 11 Finish-role operations plus
UnifiedFinish and Adaptive3d.

**Q2 — Should a `SpanKind::Region` boundary also break an arc?**
It is not a barrier today. An arc straddling two `Region` spans puts both
regions' `move_range`s on one index and breaks the tiling invariant that
`region_node_ranges_tile_the_stitched_toolpath` exists to protect. Adding it is
cheap while the fix is open and expensive later; it also widens the delta.
Recommendation: **yes**, but it is the operator's call because it is a second
output move bundled into one PR.

**Q3 — How is `MoveIntent::Unknown` treated?**
Options: (a) strict — `Unknown` breaks against every tagged intent; (b)
permissive — `Unknown` is a wildcard that joins whatever run it lands in, and
the arc takes the run's non-`Unknown` intent. (a) is principled and may cost
arc count on legacy generators; (b) preserves today's arc counts on
`Unknown`-only paths but keeps a soft edge. Recommendation: **(a)**, because
the whole point of the wave is that a label must mean one thing — but the arc
cost is unmeasured, so this is genuinely a choice.

**Q4 — Approve the re-pin package of §3.3?**
Primary: the five FNV constants and four link-site tables in
`transform_provenance_fingerprints.rs`. Secondary re-run set: F2–F5, F8–F10.
Explicitly **excluded** by census: `param_sweep` (56 sweeps),
`capability_link_moves_safety`. Approving this bounds PR-6's re-pin surface to
one file's constants plus a re-run list.

**Constraints the fix PR inherits regardless of the ruling:**

* Do not "repair" downstream labels while the source transform is wrong (plan
  §H2 fix-shape 2).
* Never couple this to a finishing-geometry change (D-16.1 / D-16.2) — plan
  §H2 fix-shape 4 forbids it explicitly.
* Invert, do not delete, the four exhibits in
  `tests/arcfit_intent_boundary_f1.rs`; their failure is PR-6's red-first
  evidence.
* `condition::merge_linear_runs` (§1.5) is NOT in scope and stays defective;
  say so in PR-6's message rather than letting it be discovered later.

---

## 7. Open items and honest gaps

* **`merge_linear_runs` intent gap** — §1.5. Not fixed, stated, unowned.
* **Arc-count cost of the fix is predicted, not measured.** §5 items 1–3 are
  reasoned from the source, not from a run. PR-6 must measure them.
* **`DressupArtifact` already removes every fitted arc from gate populations**
  (§3.2 I2). This predates and is independent of the intent bug. It is a real
  finding about gate coverage that this wave surfaced but did not investigate;
  it deserves its own item.
* **Nothing in this wave is left unrun.** The two controls and the clippy gate
  that commit `8963b75` shipped without were completed in a second Cargo window
  and are recorded in §4.
