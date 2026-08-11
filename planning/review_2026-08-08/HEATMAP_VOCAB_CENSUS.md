# A-1 — heat-map divergence and the "chipload" vocabulary census

Date: 2026-08-08
Wave: TD3 Lane A wave A-1 (research only — **nothing was fixed**)
Ledger: **F-HEATMAP** (`TECH_DEBT_2_CLOSEOUT.md` §4, line 720)
Seed: `FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`, the two [high]
findings "The viewport chipload graph compares incompatible units" and
"The 'chipload' visual vocabulary does not distinguish command,
achievement, and geometry". Both are treated as **reviewed evidence**,
not hypotheses; this wave verifies their sites, censuses the surfaces
they name plus the ones they did not, and builds the fixture their
acceptance clause asks for.

Tree: `tech-debt-3 @ becf1cb` (parent `ecd1f60`).
Fixture commit: `becf1cb`.

---

## 0. Summary for the impatient

- **The review's three cited sites all exist and say what it says they
  say.** Line numbers drifted by a few lines in one file; quoted below.
- **The defect is wider than the review recorded.** The review named
  *one* defective surface (the viewport heat-map). The census finds
  **three** viz surfaces comparing an arc-mean chip thickness against an
  advance-per-tooth band, and a **fourth** that silently blends the two
  quantities into a single number under one label.
- **The core is not the problem.** Every core-side surface — the gate,
  the `FeedExplanation` stages, narration, the MCP JSON keys, the CLI —
  reports advance per tooth and, in three cases, says so in the string.
  The divergence is entirely in `rs_cam_viz`, which is where the review
  put it.
- **The fixture is decisive and its numbers are recorded** (§3). One
  recipe, two engagement arcs: the gate's observation is **bit-equal**
  across the pair and the heat-map's is **4.497×** apart, landing them in
  two different colour classes against the same band.
- **The live screenshot is NOT EXERCISED** (§4). The GUI's frame loop
  has been parked for 45 minutes (Wayland, window not visible) — the
  exact G-LV.1 hazard Lane B is chartered to fix. A derived swatch stands
  in, clearly labelled as derived. **F-HEATMAP cannot be closed on this
  evidence**; its ledger row demands a live capture and A-2 must take one.

---

## 1. The review's cited sites, verified at `becf1cb`

### 1.1 `crates/rs_cam_core/src/tool_load/mod.rs:200-223` (review said 201-223)

The docstring on `chipload_envelopes_for_session` (fn at `:224`)
documents the defect against itself:

> ```
> /// # ⚠ Unit caveat for the viewport heatmap — reported 2026-08-06, NOT fixed
> ///
> /// The band this returns is a linear **advance per tooth**
> /// (`CHIPLOAD_LITERATURE_VERDICT.md` §2, verified per source family).
> /// Its one GUI consumer colours segments by
> /// `max(effective_chip_thickness_mm)` per move
> /// (`rs_cam_viz::app::gpu_upload::build_chipload_per_move` →
> /// `render::toolpath_render::chipload_segment_color`), which is an
> /// arc-mean **chip thickness**. Those are different quantities, and the
> /// heatmap therefore paints "rubbing risk" blue over cuts that are not
> /// rubbing …
> ```

**One clause of it is now false.** It says "**its one GUI consumer**".
There are three (§2 rows V1, V2, V3), and a fourth quantity-blending
surface (V4). A-2 must correct this sentence when it fixes the consumer,
or the fix will look complete while two surfaces still carry the defect.

### 1.2 `crates/rs_cam_viz/src/app/gpu_upload.rs:1058-1086` (review said 1058-1082)

```rust
/// Build a `toolpath_id -> { move_index -> max(effective_chip_thickness_mm) }`
/// nested map from the simulation cut trace. Worst-case-per-move is the
/// right signal for "did this segment violate the envelope?". …
fn build_chipload_per_move(
    sim_trace: Option<&rs_cam_core::simulation_cut::SimulationCutTrace>,
) -> HashMap<rs_cam_core::ToolpathId, HashMap<usize, f64>> {
```

Paired with `build_chipload_envelopes` (`:1051`, a thin delegate to
`chipload_envelopes_for_session`) at `gpu_upload.rs:829-830`, where the
mismatched pair is assembled into `chipload_inputs` for the render loop.

### 1.3 `crates/rs_cam_viz/src/render/toolpath_render.rs:718-747` (review said 720-747)

```rust
/// Map a per-move effective chip thickness against an LUT envelope to a
/// segment colour. …
fn chipload_segment_color(envelope: Option<&Range<f64>>, ct: Option<f64>) -> [f32; 3] {
    let Some(env) = envelope else { return [0.40, 0.40, 0.40]; };
    let Some(ct) = ct else { return [0.25, 0.25, 0.30]; };
    let cl_min = env.start;
    let cl_max = env.end;
    if ct < cl_min {
        // Under-engaged — rubbing risk.
        [0.20, 0.40, 0.90]
    } else if ct < cl_min * 1.1 { … }        // blue → green blend
    else if ct < cl_max * 0.9 { [0.20, 0.85, 0.30] }   // green
    else if ct <= cl_max { [1.00, 0.60, 0.10] }        // orange
    else { [0.95, 0.20, 0.20] }                        // red
}
```

The function's own doc comment states the mismatch in one sentence —
"a per-move effective chip **thickness** against an LUT **envelope**" —
and the caller at `:598` supplies exactly that.

**Verdict on §1: the review is accurate at all three sites.** No
divergence found.

---

## 2. The census

One row per surface that prints or colours a quantity under "chipload"
language. Classification is one of:

- **C** — commanded advance per tooth, `feed / (rpm · flutes)`
- **A** — achieved advance per tooth, `effective_feed / (rpm · flutes)`
- **T** — arc-mean chip thickness, `ChipGeometry::mean_chip_thickness_mm`
- **MIXED** — one displayed number that is sometimes one and sometimes another

A surface is **defective** when it compares T against a band published in
C/A, or when it labels T with "chipload" language without saying so.

### 2.1 Viewport / timeline (rs_cam_viz) — the defect cluster

| id | surface | file:line | exact label shown | quantity | verdict |
|---|---|---|---|---|---|
| **V1** | Viewport toolpath heat-map (colour mode "Chipload") | `render/toolpath_render.rs:720` fed by `app/gpu_upload.rs:1062,830` | *(no text — colour only)*; mode selector reads `"Chipload"`, hover text `"Color each segment by per-sample chipload vs the matched vendor row's window"` (`ui/viewport_overlay.rs:179,199,202`) | **T** vs a **C/A** band | **DEFECTIVE** — the review's finding. No legend, so the operator has no numeric check on the colour |
| **V2** | Sim-timeline signal track `"chipload"` | `ui/sim_timeline.rs:420,432` (envelope from `:335-337`) | track label `"chipload"`; hover prints the raw value | **T** vs a **C/A** band (shaded envelope at `:817`) | **DEFECTIVE — not named by the review.** Same band, same wrong sample field |
| **V3** | Sim-timeline summary track `"load vs limit"` | `ui/sim_timeline.rs:510-523,533` | `"load vs limit"`, normalised so 1.0 = band ceiling | **T** ÷ **C/A** ceiling | **DEFECTIVE — not named by the review.** The normalisation makes it read as a fraction-of-limit, which is the strongest "trust me" framing of the three |
| **V4** | Cut-Metrics panel row `"Chipload"` | `ui/sim_diagnostics.rs:1490-1495`, value from `state/simulation.rs:225-231` | `"Chipload"` → `"avg 0.0123 · peak 0.0456 mm"` | **MIXED** — `effective_chip_thickness_mm.unwrap_or(chipload_mm_per_tooth)` | **DEFECTIVE, and the worst kind** — one displayed scalar whose *physical quantity depends on whether the sample resolved a chip model*. Also the only surface whose unit string is a bare `mm` |
| V5 | Hotspot summary line | `ui/sim_diagnostics.rs:934` | `"m123 · waste 1.20s · peak chip 0.0456 mm"` | **C** (`SimulationCutSummary::peak_chipload_mm_per_tooth`, built at `simulation_cut.rs:1179-1181` from `sample.chipload_mm_per_tooth`) | **MISLABELLED, not miscompared** — the value is a commanded advance; the word is "chip" and the unit is `mm` |
| V6 | Tool-load verdict badge `"chipload"` | `ui/sim_diagnostics.rs:994` (+ tooltip `:1810` `unit: "mm/tooth"`) | `"chipload"` badge, tooltip carries `mm/tooth` | **A** | OK — reads the gate verdict |
| V7 | Sim op-list criterion pill | `ui/sim_op_list.rs:1086` | `"chip"` | **A** | OK on quantity; the abbreviation `"chip"` reads as chip thickness |
| V8 | Timeline exceedance annotation | `ui/sim_timeline.rs:1891` | `"chipload BurnRisk peak 0.0434"` | **A** | OK on quantity; no unit printed |
| V9 | Preflight verdict line | `ui/preflight.rs:399-406` | `"chipload: EXCEEDS (ChiploadBurnRisk)"` | **A** | OK — verdict only, no number |
| V10 | Optimize modal gate rows | `ui/optimize_modal.rs:528,707-714,738-740` | `"chipload 0.0707 (+29% over LUT max 0.0550)"` | **A** | OK on quantity; no unit printed |

### 2.2 Recommendation surfaces (rs_cam_viz) — right quantity, ambiguous vocabulary

All of these compute `feed / (rpm · flutes)` from the *static
recommendation or the currently-set values*. None is defective in the
unit sense. Every one of them is the review's second [high] finding:
they apply within-band language without saying **which** advance
(commanded, not achieved) they are judging.

| id | surface | file:line | exact label shown | quantity | verdict |
|---|---|---|---|---|---|
| P1 | Properties feeds card, static row | `ui/properties/mod.rs:1824-1825` | `"Chip Load:"` → `"0.0700 mm/tooth"` | **C** (`FeedsResult::chip_load_mm`) | vocabulary |
| P2 | Properties `OPERATING POINT — measured` | `ui/properties/mod.rs:1656-1702` | `"Limited by …"`, `"Feed vs commanded: -38% median"` | feed ratio, no chipload number at all | **gap** — the one post-sim card on the panel never prints an advance/tooth, so C and A are never shown together where the review's mock-up puts them |
| P3 | Properties `ChiploadClampedToFloor` warning | `ui/properties/mod.rs:1966-1974` | `"Chipload below rubbing floor: 0.018 -> 0.025mm/tooth"` | **C** | vocabulary |
| M1 | Feeds modal compare row | `ui/feeds_modal.rs:549-556` | `"Chipload"` current vs recommended, suffix `" mm/tooth"` | **C** (`chipload_mm()` at `:381-390`) | vocabulary |
| M2 | Feeds modal "Effective chipload at recommendation" | `ui/feeds_modal.rs:1322-1360` | `"Effective chipload at recommendation: 0.0434 mm/tooth"`, then `"= feed 2520 mm/min ÷ (18000 RPM × 2 flutes)"`, then `"= 79% of vendor band max (0.0550 mm/tooth)"` | **C** | **vocabulary, and the worst offender by name** — the word "**Effective**" is the one an operator would read as *achieved*, and it is not; it is the commanded value after Suggest's derates. It prints its own arithmetic, which is the redeeming feature |
| M3 | Feeds modal nomogram hover | `ui/feeds_modal.rs:1688-1706` | `"18000 RPM · 2520 mm/min\n→ chipload 0.0700 mm/tooth · Within band"` | **C** | vocabulary — `"Within band"` on a pre-simulation quantity |
| M4 | Feeds modal explore preview | `ui/feeds_modal.rs:2068-2086` | `"→ chipload 0.0700 mm/tooth · Within band"` | **C** | vocabulary |
| M5 | Feeds modal Chart A (chipload vs diameter) | `ui/feeds_modal.rs:2198,2241,2334,2397-2403` | title `"Chipload vs Diameter"`, y-axis `"chipload mm/tooth"` | **C** | vocabulary |
| M6 | Feeds modal Chart B (chipload vs hardness) | `ui/feeds_modal.rs:2450,2495,2582-2590,2640-2646` | title `"Chipload vs Hardness"`, y-axis `"chipload mm/tooth"` | **C** | vocabulary |
| M7 | Feeds modal band readouts | `ui/feeds_modal.rs:955,1464,1476,1777-1805` | `"VENDOR BAND\n0.0320–0.0550 mm/tooth"`, `"iso-chipload min/mid/max"` | band, in **C/A** units | OK |
| M8 | Feeds modal target-chipload explainer | `ui/feeds_modal.rs:1122-1180` | `"Target chipload"`, `"Vendor LUT midpoint: … mm/tooth"`, `"Starting chipload: … mm/tooth"` | **C** | vocabulary |
| M9 | Feeds modal min-chipload warning | `ui/feeds_modal.rs:836` | `"⚠ Chipload below LUT minimum — risk of rubbing or burning…"` | **C** | vocabulary |

### 2.3 Core, MCP and CLI — clean

| id | surface | file:line | exact label shown | quantity | verdict |
|---|---|---|---|---|---|
| N1 | `narrate_toolpath` operation context | `core/narrate.rs:589-592` | `"commanded feed-per-tooth 0.0700mm/tooth (linear advance; the chipload gate reports the same quantity at the achieved feed)"` | **C** | **OK — and the model to copy.** Names the quantity, names the unit, names its relation to the gate |
| N2 | `FeedExplanation` stage labels | `core/feeds/explanation.rs:92-117,108` | `ADVANCE_PER_TOOTH = "mm of linear advance per tooth"`, `CommandedStage::unit()` | **C** and **A** as separate typed stages | **OK — the existing vocabulary spine.** A-2's newtypes should be introduced beside this, not parallel to it |
| N3 | Chipload gate verdict payload | `core/tool_load/verdict.rs:713-737` | serde keys `observed_mm_per_tooth`, `min_mm_per_tooth`, `max_mm_per_tooth` | **A** | OK |
| N4 | Gate diagnostic messages | `core/tool_load/chipload.rs` via `diagnostics::adapters::from_tool_load` | `"Observed feed-per-tooth within band …"` (pinned by `chipload_report_wording_t12_t15.rs`) | **A** | OK |
| N5 | MCP `get_tool_load_report` | `viz/app/mcp.rs:2999` (serde of N3) | as N3 | **A** | OK |
| N6 | MCP span/kinematics rollups | `viz/app/mcp.rs:4691,4813` and `:4867` | `"per_sample_peak_chipload_mm_per_tooth"` vs `"average_mean_chip_thickness_mm"` / `"peak_chip_thickness_mm"` | **C** and **T**, in *separate keys* | **OK — the only surface in the repo that publishes both quantities side by side under distinguishable names** |
| N7 | CLI project report | `cli/project.rs:539-544` | `"Peak chipload: 0.070 mm/tooth"` | **C** | mislabel-lite — correct unit, but "chipload" without "commanded"; and it is a raw per-sample peak, not the gate statistic |
| N8 | CLI per-toolpath report | `cli/main.rs:321-324` | `"Peak chipload: 0.070 mm/tooth"` | **C** | as N7 |
| N9 | CLI smoke verdicts | `cli/smoke.rs:197,593,599` | `"{case}: chipload {} → {}"` | **A** | OK |

### 2.4 Counts

| class | defective (T vs a C/A band, or MIXED) | mislabelled but correct quantity | clean |
|---|---|---|---|
| viewport / timeline (V1–V10) | **4** (V1, V2, V3, V4) | 4 (V5, V7, V8, V10 — abbreviations or missing units) | 2 (V6, V9) |
| recommendation surfaces (P1–M9) | 0 | **11** (P1, P3, M1–M6, M8, M9 + the P2 gap) | 1 (M7) |
| core / MCP / CLI (N1–N9) | 0 | 2 (N7, N8) | **7** |
| **total (30 surfaces)** | **4** | **17** | **10** |

By quantity: **C** on 19 surfaces, **A** on 8, **T** on 3, **MIXED** on 1.
Three surfaces (V1, V2, V3) compare **T** against a band published in
**C/A**; one (V4) cannot say which quantity it is showing.

### 2.5 Two things the census found that the review did not

1. **V2 and V3 are the same defect on the same screen.** Fixing only V1
   leaves the sim-timeline painting the identical wrong comparison one
   panel below, and V3's normalisation (`chip / band_ceiling`) presents
   it as a **fraction of the limit** — a stronger claim than a colour.
2. **V4 is a quantity that changes identity per sample.**
   `SpanAggregate::ingest` (`state/simulation.rs:225-227`) does
   `effective_chip_thickness_mm.unwrap_or(chipload_mm_per_tooth)`, then
   both `avg` and `peak` are printed under `"Chipload"` with unit `mm`.
   On a trace where some samples resolve a chip model and some do not,
   that single average is a **mean of two different physical
   quantities**. No gate reads it; it is display-only. It is also the
   only place in the repo where the two are summed together.

---

## 3. The fixture

`crates/rs_cam_core/tests/heatmap_two_arc_divergence_a1.rs` — committed
`becf1cb`, 2 tests, both green on `tech-debt-3`. It is a
**characterization** of shipped behaviour, deliberately passing; A-2
inverts one assertion red-first.

**Construction.** Two toolpaths in one trace, identical in every
commanded number — Ø6 flat endmill, 2 flutes, 18 000 RPM, 2 520 mm/min
(= 0.070 mm/tooth commanded), 1.5 mm axial DOC, hard maple,
`Pocket`/`Roughing` — differing only in engagement arc. The radial
engagement fraction on each arm is set to the value that arc implies,
`ae/D = (1 − cos arc)/2`, so neither arm is a self-contradictory sample.
The F-035 predicted-feed fraction is 0.62 on **both** arms, so it cannot
be a source of divergence.

**Measured (stderr of the test, verbatim):**

```
  two-arc fixture — Ø6 flat, 2F, 18000 RPM, feed 2520 mm/min, DOC 1.5 mm
    arm A  arc 0.80 rad (ae/D 0.1516)
    arm B  arc 3.14 rad (ae/D 1.0000)
    commanded advance/tooth      A 0.070000  B 0.070000
    gate observed advance/tooth  A 0.043400  B 0.043400
    arc-mean chip thickness      A 0.009910  B 0.044563   (ratio 4.497×)
    band (advance/tooth)         Some(0.032) .. 0.055000

  heat-map colour classes against band Some(0.032)..0.055000
    current measure (arc-mean chip):  A RubbingBlue   B WithinGreen
    review's measure (achieved a/t):  A WithinGreen   B WithinGreen
```

**What that is evidence of.**

- The gate's observation is **bit-equal** across the pair (`assert_eq!`
  on `f64`, not a tolerance) — because `effective_feed / (rpm · flutes)`
  contains no arc term.
- Both arms matched the **same vendor row**, asserted, so the colour
  split below cannot be a band difference.
- The heat-map's quantity moves **4.497×**, and the two arms land in
  **different colour classes**. Arm A — commanded and achieved
  comfortably mid-band — is painted **blue, "Under-engaged — rubbing
  risk"**.
- Under the review's proposed measure both arms are `WithinGreen`.

**Two physics corrections this fixture forced, now pinned in its source
so the next reader does not repeat them:**

1. **The chip factor is not monotonic in the arc.** For the shipped flat
   model (`tool/mod.rs:124-134`) `mean = (2·fz·sin(arc)/arc)·(1−cos(arc/2))`
   for `arc < π`, with `h_max = fz` at `arc ≥ π`. The factor **peaks near
   arc ≈ 2.0 rad (≈ 0.418)** and collapses at both ends — 0.141 at 0.8 rad,
   0.199 at 2.8 rad, 0.637 at a full slot. A first attempt at this fixture
   used arcs 1.2 and 2.8 and found the *wider* arc reading **thinner**
   (0.01221 vs 0.00894), which is correct behaviour and a trap for
   anyone assuming "more engagement ⇒ thicker chip".
2. `ae/D = (1 − cos arc)/2`, **not** `(1 − cos(arc/2))/2`. The wrong form
   gives 0.5 at a full slot.

The 0.637 full-slot factor is the reciprocal of the "**1.6× at a full
slot**" figure `tool_load/mod.rs`'s caveat quotes — so the caveat's own
number is now independently reproduced by a test.

---

## 4. Screenshots — what was captured, and what was not

### 4.1 ~~NOT EXERCISED~~ — CLOSED by A-2, 2026-08-11

> **Status update (A-2, 2026-08-11).** The live pair now exists:
> `artifacts/a2/heatmap_{before,after}_{viewport,simulation}.png`, four
> GUI captures of a running `rs_cam_gui --mcp`. **F-HEATMAP's screenshot
> obligation is discharged.**
>
> Not from the operator's instance. It stayed unreachable for
> GUI-dispatched calls (`served_from: "snapshot"`, `snapshot_age_s`
> 142.6, `frame_loop.healthy: false`) — the same G-LV.1 block recorded
> below, three days later. A-2 took the brief's sanctioned fallback and
> built two of its own instances, `825524f` (pre-fix) and `7aa0be0`
> (post-fix), same project / generation / resolution / camera. See
> `artifacts/a2/README.md` for which build each PNG came from and for
> the one identical line patched in both (the DEFAULT colour mode —
> the heat-map has no MCP or CLI selector).
>
> Two corrections to the resume condition written below:
> **`screenshot_toolpath` is not a heat-map surface** (separate
> offscreen renderer, fixed palette, ignores `toolpath_color_mode`;
> proof captured in `artifacts/a2/`), and `WINIT_UNIX_BACKEND=x11` is
> inert on winit 0.30 — unset `WAYLAND_DISPLAY` instead (Checkpoint
> L-6). The working recipe is `set_ui_view {workspace: "toolpaths"}` +
> `screenshot_gui`.
>
> The magnitude on a real project, which §6 recorded as unmeasured, is
> now measured — and its **sign is opposite** to the fixture's. On
> wanaka's "Back Rough" (adaptive3d, near-full-slot) the retired measure
> painted **red, above the ceiling**, over an op the gate rules
> `Within`; the fixture's synthetic light-arc arm painted **blue, below
> the floor**. Same defect, opposite direction. The fixture's 4.497×
> remains a fixture figure and must not be quoted as a wanaka one.

#### Original A-1 entry (2026-08-08), unedited


**The live screenshot required by rule 3 and by F-HEATMAP's own ledger
row was not taken.** The GUI (running `d820226-dirty`, the review's
revision) was reachable over MCP but its **frame loop has been parked
for 45 minutes** — `frames: 87899` unchanged, `healthy: false`,
`last_frame_age_s` 2086 → 2706 across the wave, one request of mine
stranded in the channel. On Wayland the compositor withholds the frame
callbacks winit needs, so `request_repaint()` cannot break the park and
**every** GUI-dispatched call (`load_project`, `set_ui_view`,
`screenshot_gui`, `screenshot_toolpath`) is unreachable. Only
`generation_status` and the snapshot arm of `project_summary` answered.

This is **G-LV.1 re-open (b)** — Lane B wave B-3's charter — reproducing
itself as a direct blocker on Lane A. Worth recording as an
observation: the two lanes are less independent than the plan assumed.

**Resume condition for A-2:** operator makes the window visible (or the
GUI is relaunched with `WINIT_UNIX_BACKEND=x11`), then wanaka is
generated and simulated at 0.1 mm, viewport colour mode set to
`Chipload`, and both a `screenshot_gui` and a
`screenshot_toolpath(include_rapids:false)` are taken **before** the
measure is changed. Without that pair, F-HEATMAP does not close.

### 4.2 Captured: derived swatch

`artifacts/a1/heatmap_measure_swatch.png` — the exact RGB triples
`chipload_segment_color` emits for the fixture's four values, rendered
side by side. **Regenerated from the shipped colour function, not
observed from the GUI**, and the image says so on its face. Script:
scratchpad `swatch.py` (thresholds transcribed from
`toolpath_render.rs:720-747`, band and values from §3).

| row | arm | value | class | rgb |
|---|---|---|---|---|
| shipped measure | A (arc 0.80) | 0.009910 | RubbingBlue | (51, 102, 230) |
| shipped measure | B (arc π) | 0.044563 | WithinGreen | (51, 217, 76) |
| review measure | A | 0.043400 | WithinGreen | (51, 217, 76) |
| review measure | B | 0.043400 | WithinGreen | (51, 217, 76) |

It is a communication aid for Checkpoint H. It is **not** evidence about
the live surface and must not be cited as such.

---

## 5. Checkpoint H package — questions for the operator

Three questions. Each carries concrete options and a recommendation. The
agent does not self-approve any of them.

### H1 — the display measure

The review proposes replacing the viewport's per-move quantity with
`effective_feed_for_sample(sample, predicted_feeds) / (rpm × flutes)` —
the **achieved advance per tooth**, which is exactly what the
post-simulation gate observes.

| option | effect | cost |
|---|---|---|
| **H1-a (review's, recommended)** | Colour by **achieved** advance/tooth. Viewport, timeline and gate then agree by construction — one number, one band, one verdict | Needs `predicted_feeds` threaded into `build_chipload_per_move`; the trace already carries it |
| H1-b | Colour by **commanded** advance/tooth (`sample.chipload_mm_per_tooth`) | Simplest change (one field swap, no new inputs). But it then disagrees with the gate wherever kinematics throttle the feed — on wanaka's finish ops that is routinely a large factor — and reintroduces "the graph says fine, the gate says exceeds" in a *new* direction |
| H1-c | Keep T, and convert the **band** to chip-thickness units instead | **Not available.** `CHIPLOAD_LITERATURE_VERDICT.md` §2.3 established no wood chart in the shipped LUT publishes an engagement condition for its chipload column, so there is no sourced conversion. This is the same reasoning that made the 2026-08-06 gate fix a deletion rather than an inversion (F-BIPOLAR carries the residue) |

**Recommendation: H1-a.** It is the only option under which the coloured
surface and the verdict surface can be *proved* to agree, and the fixture
in §3 is already the proof. Note that it makes the viewport agree with
the gate but **disagree with the modal**, which shows commanded — which
is what H2 is for.

**Scope rider (agent's own finding, needs a ruling):** H1 as written
fixes V1 only. **V2 and V3 carry the identical defect** and V4 shows a
quantity that changes identity per sample. Recommendation: A-2 fixes
**V1, V2, V3 and V4 together** — they are four call sites of one wrong
choice, splitting them leaves a half-fixed screen, and V3's
"fraction of limit" framing is arguably more misleading than the colour.

### H2 — labelling vocabulary for the three quantities

The repo already has a canonical spelling for two of the three, in
`feeds/explanation.rs`: `ADVANCE_PER_TOOTH = "mm of linear advance per
tooth"`, with `CommandedStage` and the gate stage as separate typed
stages. `narrate.rs:589` is the only *rendered* surface that follows it.

| option | vocabulary |
|---|---|
| **H2-a (recommended)** | Adopt the review's three names verbatim on every surface: **"Commanded advance/tooth"**, **"Achieved advance/tooth"**, **"Arc-mean chip thickness"**. The word *chipload* survives **only** where a vendor band is being named (`"Vendor band 0.032–0.055 mm advance/tooth"`), because that is the vendors' own column heading |
| H2-b | Keep "chipload" as the family word and qualify it: "commanded chipload" / "achieved chipload" / "chip thickness". Less churn on existing strings and on operator muscle memory; keeps the ambiguous root word on screen |
| H2-c | Label only the defective surfaces; leave the modal alone | Cheapest, but leaves the review's second [high] finding unaddressed — the modal is where a recommendation is accepted |

**Recommendation: H2-a**, with two specific string changes called out
because they are the ones most likely to mislead:

- `feeds_modal.rs:1322` **"Effective chipload at recommendation"** →
  "Commanded advance/tooth at recommendation". The word *effective*
  currently means "after Suggest's derates", but every other use of
  *effective* in this codebase means "achieved by the machine". This is
  the single worst string in the census.
- `sim_diagnostics.rs:1490` **"Chipload … avg/peak mm"** → split into two
  rows, or drop, per H3. It cannot be labelled correctly while it is a
  `unwrap_or` blend of two quantities.

And one addition rather than a rename: the review's operating-point card
(`Commanded / Achieved / Vendor band / Gate`, four lines) has **no home
today** — P2 shows a feed ratio and never prints an advance/tooth.
Recommendation: build it in the properties panel's existing
`OPERATING POINT — measured` section, where the modulation summary
already lands.

### H3 — does arc-mean chip thickness keep a separate visual?

The review says keep it, "available as a distinct engagement/force
visualisation; do not rename it to chipload or compare it to a vendor
band". The census finds it is currently *only* reachable through the
three defective surfaces plus the MCP `per_kinematics` JSON (N6).

| option | outcome |
|---|---|
| **H3-a (recommended)** | Keep one chip-thickness visual — the sim-timeline track (V2) — relabelled **"arc-mean chip thickness"**, with its band shading **removed** (there is no sourced band for it). V1 and V3 switch to advance/tooth. V4 splits into two rows or drops the blend |
| H3-b | Remove every chip-thickness visual; leave it in MCP JSON only | Cleanest, no unbanded graph to misread — but discards a real engagement/force signal an operator may want, and F-BIPOLAR still consumes the quantity in core |
| H3-c | Keep all three, relabelled, all unbanded | Three graphs of a quantity with no threshold is clutter and invites eyeballed thresholds |

**Recommendation: H3-a.** Rider: whatever is ruled, the band shading
must come off any chip-thickness plot. A shaded envelope *is* a
comparison, and there is no source for that comparison.

### H4 — bookkeeping asks (no design content, ruled by default unless the operator objects)

1. `tool_load/mod.rs:206` says the band has "**its one GUI consumer**".
   It has four. A-2 corrects the sentence.
2. F-HEATMAP's ledger row requires a live screenshot. This wave could
   not take one (§4.1). Confirm A-2 owns it and that **F-HEATMAP stays
   open until the live pair exists**.
3. V5/N7/N8 print a *commanded* peak under the word "chipload"/"chip"
   with a bare `mm` on V5. Fold into H2-a's sweep (cheap) or ledger
   separately?

---

## 6. What this wave did NOT do

- **Fixed nothing.** No production file was touched. The only tree
  change is one new test file and this directory.
- **Took no live screenshot** (§4.1) — NOT EXERCISED, blocker named,
  resume condition written.
- **Did not measure the defect on a real project.** Every number in §3
  is synthetic. The magnitude on wanaka's actual ops is unmeasured, and
  the fixture's 4.497× must not be quoted as a wanaka figure.
- **Did not touch the arc-mean chip thickness policy** — that is A-9
  (F-BIPOLAR / F-VALID / F-MISSAE) and is explicitly out of A-1's scope
  per the review's own step 6.
- **Did not census the optimizer's internal chipload retargeting**
  (`retarget/chipload.rs`) beyond its display rows — that is F-OPT / A-8.
