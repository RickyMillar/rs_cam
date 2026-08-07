# TD3 orchestration log

Plan: `TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md` (this directory). Operating
rules §0 of the plan are binding on every entry. Agents append entries in
the §3.1 format inherited from TD2 (Status / Commits / Parent / Question and
pre-registered bars / Fixture-population-resolution / Render paths / Result
fact-interpretation-uncertainty / Red-first evidence / Verification / NOT
FIXED stated / Next action). The orchestrator updates the tracker after a
wave, merge, re-scope, or checkpoint ruling. No agent self-approves a
checkpoint.

## Tracker

| lane/wave | item | status |
|---|---|---|
| A-1 | heat-map + vocabulary census → Checkpoint H | IN PROGRESS 2026-08-08 |
| A-2 | heat-map fix + newtypes + screenshot sentry | blocked on H |
| A-3 | apply-contract census → Checkpoint I | NOT STARTED |
| A-4 | one application funnel | blocked on I |
| A-5 | arc_fit_ratio evidence package → Checkpoint J | NOT STARTED |
| A-6 | LUT delta + boundary contract + G-CHIP-ULP → Checkpoint K | NOT STARTED |
| A-7 | execute K | blocked on K |
| A-8 | optimizer assumptions + retarget fixture | after A-2 |
| A-9 | chip-thickness policy (research-only) | after A-2 |
| B-1 | G-LV.2 crash capture + read-size census → Checkpoint L | IN PROGRESS 2026-08-08 |
| B-2 | bounded reads + filter fix | blocked on L |
| B-3 | dispatch decoupling design → Checkpoint M | NOT STARTED |
| B-4 | execute M + six frame-coupled drivers | blocked on M |
| B-5 | G-RESULTS GUI/CLI parity | NOT STARTED |
| S-1 | X-VAC population census | NOT STARTED |
| S-2 | /refresh-lit-matrix (DR-URL, DR-WS) | NOT STARTED |
| S-3 | A2D-165 matrix cells | NOT STARTED |
| S-4 | G-BYTE frozen-snapshot A/B | NOT STARTED — should precede trusting A-5 fixtures |
| S-5 | smalls (FP-65, DR-PIN, DR-LIVE, hover, O-CANC) | NOT STARTED |

## Checkpoint rulings

(none yet)

---

## A-1 — heat-map divergence + chipload vocabulary census, 2026-08-08

Status: AWAITING_CHECKPOINT (Checkpoint H)

Commit(s): `becf1cb` (the two-arc fixture, `crates/rs_cam_core/tests/heatmap_two_arc_divergence_a1.rs`, 2 tests). The census document and its artifact are a separate docs commit in the same wave. **No production file touched.**

Parent/revision measured: `ecd1f60` (branch `tech-debt-3`). The live GUI reachable over MCP is a different, older build — `d820226-dirty`, the revision the operator's review itself examined — which matters for §4 below and is stated in the census.

Question and pre-registered bars: plan §2 A-1. Verify the review's three cited sites at the current revision; census every surface printing or colouring a quantity under "chipload" language and classify each as commanded advance / achieved advance / arc-mean thickness; build the review's synthetic two-arc fixture; capture a live before-fix screenshot. Bars I set before starting: (a) the review is reviewed evidence — I verify its sites and report divergence, I do not re-derive its findings; (b) every census row carries `file:line` **and the exact string the operator sees**, not a paraphrase; (c) the fixture must assert its own premise (identical commanded inputs) from the samples rather than from its constants; (d) any surface I could not reach live is reported NOT EXERCISED with a named resume condition, never inflated.

Fixture/population/resolution: the census population is every `rs_cam_viz`, `rs_cam_core`, `rs_cam_cli` site matching chipload/chip-load/mm-per-tooth language — **30 surfaces**, enumerated with `file:line` in `HEATMAP_VOCAB_CENSUS.md` §2. The fixture is synthetic and analytic: two toolpaths in one `SimulationCutTrace`, Ø6 flat 2F endmill, 18 000 RPM, 2 520 mm/min (0.070 mm/tooth commanded), 1.5 mm axial DOC, hard maple, `Pocket`/`Roughing`, 24 steady-state samples per arm, F-035 predicted-feed fraction 0.62 on **both** arms. The only varied input is engagement arc — 0.80 rad vs π — with `ae/D = (1 − cos arc)/2` set consistently on each arm. No simulation resolution is involved; nothing is dexel-measured.

Render/artifact paths: `planning/review_2026-08-08/HEATMAP_VOCAB_CENSUS.md`; `planning/review_2026-08-08/artifacts/a1/heatmap_measure_swatch.png` (a **derived** swatch of the shipped colour function, labelled as such on its own face — not a GUI capture). The live capture is NOT EXERCISED; see below.

Result (fact), interpretation, and uncertainty:

- **Fact.** All three of the review's cited sites exist at `ecd1f60` and say what the review says they say. Line numbers drifted slightly (`tool_load/mod.rs` 200-223 not 201-223; `gpu_upload.rs` 1058-1086 not -1082; `toolpath_render.rs` 718-747 not 720-747). **No substantive divergence from the review was found.**
- **Fact.** `chipload_envelopes_for_session`'s own docstring (`tool_load/mod.rs:206`) says the band has "**its one GUI consumer**". It has **four**: the viewport heat-map (V1), the sim-timeline `"chipload"` track (V2), the timeline `"load vs limit"` normalised summary (V3), and the Cut-Metrics `"Chipload"` row (V4). Three of the four compare an arc-mean chip thickness against an advance-per-tooth band; the fourth blends the two quantities.
- **Fact.** V4 (`state/simulation.rs:225-227`) computes `effective_chip_thickness_mm.unwrap_or(chipload_mm_per_tooth)`, then prints `avg` and `peak` of that under the label `"Chipload"` with unit `mm`. On a mixed trace that average is a **mean of two different physical quantities**. No gate reads it; display-only.
- **Fact.** Census counts across 30 surfaces: **4 defective** (T against a C/A band, or MIXED), **17 mislabelled but correct quantity**, **10 clean**. By quantity: commanded 19, achieved 8, arc-mean 3, mixed 1. Every defective surface is in `rs_cam_viz`; **core, MCP and CLI are clean on quantity**, and three core surfaces (`narrate.rs:589`, `feeds/explanation.rs:108`, MCP `per_kinematics`) already carry the vocabulary the review proposes.
- **Fact (fixture).** One recipe, two arcs: commanded advance/tooth **0.070000** on both; gate observed advance/tooth **0.043400** on both, asserted **bit-equal**; arc-mean chip thickness **0.009910** vs **0.044563**, a ratio of **4.497×**; band **0.032–0.055** advance/tooth, asserted identical across the pair. Colour classes: shipped measure **RubbingBlue / WithinGreen**, review's measure **WithinGreen / WithinGreen**.
- **Fact (physics, corrected in-wave).** The shipped flat chip model's factor is **not monotonic in the arc** — `mean = (2·fz·sin arc / arc)·(1 − cos(arc/2))` peaks near arc ≈ 2.0 rad at ≈ 0.418 and collapses at both ends (0.141 at 0.8 rad, 0.199 at 2.8 rad, 0.637 at a full slot). My first fixture attempt used arcs 1.2 and 2.8 and found the **wider** arc reading **thinner** (0.01221 vs 0.00894). That is correct behaviour and a trap; both it and the correct `ae/D = (1 − cos arc)/2` are now pinned in the fixture's source. The 0.637 full-slot factor independently reproduces the "1.6× at a full slot" number `tool_load/mod.rs`'s caveat quotes.
- **Interpretation.** The review's diagnosis is right and its scope is one surface too narrow. Fixing V1 alone leaves the identical wrong comparison on the sim-timeline one panel below, where V3's `chip ÷ band_ceiling` normalisation presents it as a **fraction of the limit** — a stronger claim than a colour. My recommendation to Checkpoint H is that A-2 fix V1–V4 together as four call sites of one wrong choice. The vocabulary half of the work is a rename sweep, not a design problem, because `feeds/explanation.rs` already holds the canonical spelling; what is genuinely **missing** rather than mislabelled is the review's operating-point card — the properties panel's `OPERATING POINT — measured` section shows a feed ratio and **never prints an advance/tooth at all**, so commanded and achieved are nowhere shown together.
- **Uncertainty, stated.** (a) Every number in the fixture is **synthetic**. The magnitude of the defect on wanaka's real operations is **unmeasured**, and 4.497× must not be quoted as a wanaka figure. (b) The census classifies surfaces by reading their source, not by observing them render — for the 17 "mislabelled" rows I am asserting what string the code formats, which is weaker than seeing it on screen, and §4 explains why I could not see it. (c) The swatch is regenerated from the colour function, so it proves the function's thresholds, not the viewport's behaviour.

Red-first evidence / fingerprints changed: **no fingerprint captured, moved or re-pinned; no gate, threshold, severity or recommendation number touched.** The wave is research-only by charter. The fixture is committed **green on purpose** — it is a characterization of shipped behaviour, and its own source says so, so that A-2 has the pre-fix numbers recorded rather than reconstructed. The red-first inversion belongs to A-2: the assertion `displayed_a != displayed_b` (the shipped measure splitting one recipe across two colour classes) is the one A-2 must make fail; `honest_a == honest_b` is the one it keeps. Repo rule honoured — the instrument was committed the moment it linted clean, ahead of the checkpoint that will rule on what it measures.

Verification (focused commands + exact known-red state):

- `cargo test -p rs_cam_core -q --test heatmap_two_arc_divergence_a1 -- --nocapture` — **2 passed, 0 failed**, in 0.01 s. Output quoted verbatim in the census §3.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean; only output is the pre-existing future-incompat note for transitive `nom 3.2.1` / `quick-xml 0.22.0`.
- `cargo fmt --check --all` — clean, exit 0. (`cargo fmt --check -- <single file>` reports a spurious import-ordering diff because it drops the workspace style edition; the workspace form is the authority.)
- Slot discipline: `free -g` (20–22 GiB available) and bracketed `pgrep -af "carg[o]"` before each of the two launches; no foreign Cargo job seen; disk 179 G free. No release build. All read-only census work was completed before either launch, so the slot was held for ~45 s total across the wave and was free for B-1 the rest of the time.
- No wanaka file was written. `planning/airrun_2026-06-01/wanaka.toml` and the operator's `FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md` were read and remain unstaged.

NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition:

- **NOT EXERCISED: the live before-fix screenshot.** The GUI's frame loop was parked for the whole wave — `frames: 87899` unchanged across four polls, `healthy: false`, `last_frame_age_s` 2087 → 2706, one request of mine stranded in the channel. Only `generation_status` and the snapshot arm of `project_summary` answered; every GUI-dispatched call (`load_project`, `set_ui_view`, `screenshot_gui`, `screenshot_toolpath`) was unreachable. I did not spin past ~45 min. **F-HEATMAP therefore cannot close on this wave's evidence** — its ledger row demands a live capture. Owner: A-2. Resume condition: window made visible (or GUI relaunched with `WINIT_UNIX_BACKEND=x11`), wanaka generated + simulated at 0.1 mm, viewport colour mode `Chipload`, then `screenshot_gui` + `screenshot_toolpath(include_rapids:false)` taken **before** the measure changes.
- **Cross-lane observation, unplanned:** that park is **G-LV.1 re-open (b)** — Lane B wave B-3's charter — reproducing itself as a hard blocker on a Lane A deliverable. The plan treats the lanes as independent except for one file; on this evidence Lane B's dispatch hazard gates Lane A's rule-3 obligations. Worth the orchestrator's attention when sequencing.
- **`tool_load/mod.rs:206`'s "its one GUI consumer" is false** (four consumers). Not corrected here — it is production text in A-2's territory. Owner: A-2. Re-open condition: none needed; it is asked as Checkpoint H item H4.1, and if A-2 fixes only V1 the sentence stays wrong in a new way.
- **The defect's magnitude on a real project is unmeasured.** Owner: A-2's live pass. Re-open condition: none — it is a gap in evidence, not a defect.
- **V5 / N7 / N8** print a *commanded* advance under the word "chipload"/"chip", with a bare `mm` unit on V5. Not fixed; asked as H4.3 (fold into the H2 sweep, or ledger separately).
- Everything in A-9's territory (F-BIPOLAR, F-VALID, F-MISSAE, the chip-thickness policy) was deliberately **not touched**, per the review's own step 6 warning that it must not be folded into a cosmetic graph change.

Next action / checkpoint request: **Checkpoint H requested**, three questions plus a bookkeeping group, each with options and a recommendation, in `HEATMAP_VOCAB_CENSUS.md` §5. H1 — the display measure (recommend the review's achieved advance/tooth), **with a scope rider asking that A-2 fix V1–V4 rather than V1 alone**. H2 — the labelling vocabulary (recommend the review's three names verbatim, with `feeds_modal.rs:1322` "Effective chipload at recommendation" called out as the single worst string in the census, and the operating-point card added to the properties panel where it currently does not exist). H3 — whether arc-mean thickness keeps a separate visual (recommend one relabelled, **unbanded** timeline track). Orchestrator action: put H to the operator via AskUserQuestion; do not let A-2 start on H1 alone without ruling the scope rider, and carry the NOT EXERCISED screenshot into A-2's definition of done.
