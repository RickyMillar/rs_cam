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
| A-1 | heat-map + vocabulary census → Checkpoint H | COMPLETE — Checkpoint H RULED 2026-08-08 (all four recommendations; live screenshot NOT EXERCISED, obligation transferred to A-2) |
| A-2 | heat-map fix + newtypes + screenshot sentry | COMPLETE 2026-08-11 (8 commits; H executed in full; red-run genuine vs production body; fingerprints verified 0 fail; before/after pair from own builds `825524f`/`7aa0be0` — operator-instance capture still blocked by G-LV.1; riders: workspace clippy gate owed by B-2 close-out, heat-map legend un-ruled, defect sign on wanaka is HIGH not low — 4.497× is fixture-only) |
| A-3 | apply-contract census → Checkpoint I | COMPLETE — Checkpoint I RULED 2026-08-12 (C hybrid: reroute batch+explore, delete M1–M6; refusal replaces Apply column; O2 routed, O1/O3 excluded; MCP apply tool added; tests inverted in place) |
| A-4 | one application funnel | COMPLETE 2026-08-12 (`9026ddb`→`e66ead2`; I executed in full incl. `apply_feeds` MCP tool; bars 2/3/4 MET (recipe byte-identical, DOC gap bit-equal 1.000×, no locking); bar 1 reported honestly at 6/9 red — orchestrator disposition: intent satisfied, the 3 greens assert behaviour the ruling deliberately preserves, agent's not-met report stands unedited; modal rendered both states, own debug GUI) |
| INTAKE | **G-SUB1MM**: `sub_1mm_tapered_ball_hardwood_finish_extrapolates_with_scaling` RED — pre-existing (A-4 attributed by reverting its core diff; failure identical). Sub-Ø2 scaling-law territory → fold into A-6/Checkpoint K beside G-LIT-IPE | OPEN 2026-08-12 |
| A-5 | arc_fit_ratio evidence package → Checkpoint J | NOT STARTED |
| A-6 | LUT delta + boundary contract + G-CHIP-ULP → Checkpoint K | NOT STARTED |
| A-7 | execute K | blocked on K |
| A-8 | optimizer assumptions + retarget fixture | after A-2 |
| A-9 | chip-thickness policy (research-only) | after A-2 |
| B-1 | G-LV.2 crash capture + read-size census → Checkpoint L | COMPLETE — Checkpoint L RULED 2026-08-08 (Option 2 + defaults + compact JSON + keep-id/refuse-unmatched + rule-10 correction; G-LV.2 stays open, cause-not-attributed) |
| B-2 | bounded reads + filter fix | COMPLETE 2026-08-12 (5 commits; L executed in full incl. L-6 rider — main.rs USED the inert var as a warning-suppression condition; red-runs pinned 60.5 MB→bounded ~181×, skeleton→refusal; own sentry caught a C25-shape defect pre-land; workspace clippy+fmt CLEAN at `afd102b`; NOT EXERCISED: live release-rig re-run, resume condition in entry; G-LV.2 stays OPEN cause-not-attributed) |
| INTAKE | **G-LIT-IPE**: `literature_matrix::flat_3mm_pocket_ipe_extreme` trips `anti.ipe_micro_matches_oak_micro_chipload` at critical — RED and PRE-EXISTING (fails identically at `825524f`, core src byte-identical to master `53b1c72`; contradicts an earlier green claim). A feeds recipe number needing a checkpoint — owner Lane A, fold into A-6's Checkpoint K package | OPEN 2026-08-12 |
| B-3 | dispatch decoupling design → Checkpoint M | NOT STARTED |
| B-4 | execute M + six frame-coupled drivers | blocked on M |
| B-5 | G-RESULTS GUI/CLI parity | NOT STARTED |
| S-1 | X-VAC population census | NOT STARTED |
| S-2 | /refresh-lit-matrix (DR-URL, DR-WS) | NOT STARTED |
| S-3 | A2D-165 matrix cells | NOT STARTED |
| S-4 | G-BYTE frozen-snapshot A/B | COMPLETE 2026-08-12 (`85f40a1`/`d27b7ef`; verdict IDENTICAL — six generations, one SHA256; G-BYTE = snapshot drift, NOT nondeterminism; `StockSnapshotStamp` (content-derived FNV digest) landed on ToolpathStats, two-valued family; A-5 unblocked and should assert same-stamp on its A/B arms; residual: G0-approach channel inferred-not-measured (needs sub-stock-top fixture), GUI-worker stamp site → close-out validation) |
| INTAKE | **G-XFP**: `transform_provenance_fingerprints` fails 3/3 — pre-existing per two independent attributions (S-4 stash-test at `675a643`; A-4 core-diff revert, failure byte-identical both times). Orchestrator adds: `git log 53b1c72..HEAD -- crates/rs_cam_core/` shows ALL TD3 core changes confined to feeds/tool_load/stats/session-stamp — nothing in transform/span territory. Residual: definitive run AT master `53b1c72` still owed (Cargo slot occupied by a foreign project's job); if red at master, TD2's closing green claim was itself a false green. Full core-suite enumeration now exists (A-4, `--no-fail-fast`): 2930 passed / 6 failed = G-LIT-IPE, G-XFP×3, G-SUB1MM, and `wanaka_suggest_baseline` (KNOWN environmental — operator's play-file drifted, not an intake). Trap recorded: plain `cargo test -p rs_cam_core -q` stops at the first failing binary and under-reports | OPEN — master run pending slot |
| S-5 | smalls (FP-65, DR-PIN, DR-LIVE, hover, O-CANC) | NOT STARTED |

## Checkpoint rulings

### Checkpoint H — ruled 2026-08-08 (operator, via AskUserQuestion). BINDING.

- **H1: Achieved advance/tooth, fix V1–V4 together.** The heat-map (V1), timeline chipload track (V2), "load vs limit" fraction (V3) and Cut-Metrics blend row (V4) all move to `effective_feed / (rpm × flutes)` as one wave — four call sites of one wrong choice.
- **H2: The review's three names verbatim** on every surface — "Commanded advance/tooth" / "Achieved advance/tooth" / "Arc-mean chip thickness"; the word *chipload* survives only where a vendor band is being named. Includes fixing `feeds_modal.rs:1322` "Effective chipload at recommendation" → commanded-advance wording.
- **H3: One chip-thickness visual survives** — the sim-timeline track, relabelled "arc-mean chip thickness", **band shading removed** (no sourced band for that quantity; a shaded envelope is a comparison). V1/V3 switch to advance/tooth; V4 splits or drops the blend.
- **H4: all four bookkeeping items accepted** — (1) correct the "its one GUI consumer" docstring; (2) A-2 owns the live before/after screenshot pair and **F-HEATMAP stays open until it exists**; (3) V5/N7/N8 fold into the H2 rename sweep; (4) the operating-point card (Commanded / Achieved / Vendor band / Gate) gets built in the properties panel's `OPERATING POINT — measured` section.

### Checkpoint I — ruled 2026-08-12 (operator, via AskUserQuestion). BINDING.

- **I-1: Option C (hybrid).** Reroute the batch paths (M7/M9/M10/M11) and the explore apply (M8) through the funnel with the panel's guarantees; **DELETE the six per-field buttons (M1–M6)** — they carry the 3.50× funnel-bypass defect and answer no measured need. No `ApplyScope::Field` arm is needed (I-2 moot).
- **I-3: refused pairing → modal opens, charts drawn, the whole Apply column replaced by the refusal text.** The explanatory job survives; the write becomes impossible.
- **I-4: exclude O1/O3 with a stated reason in the funnel docs** (their candidates are sim-verified end to end); **route O2** (`reoptimize_with_axis_override`) through the funnel — it is a raw unclamped write of an un-simulated value.
- **I-5: A-4 adds an MCP apply tool** routed through the same funnel with `ApplyScope` — agents get the panel's guarantees.
- **I-6: A-3's nine characterization tests inverted in place**, measured pre-fix numbers preserved in doc comments.
- Pre-registered bars for A-4 stand as written in `APPLY_CONTRACT_CENSUS.md` §5.4: all nine tests go red against the fix then invert; the Pocket fixture's `3000 / 794 / 18000 / 2.222 / 1.27` byte-identical (a moved fingerprint is a STOP); the §3.4 DOC gap closes to 1.00×; no background field locking.

### Checkpoint L — ruled 2026-08-08 (operator, via AskUserQuestion). BINDING.

- **L-1/L-2: Option 2.** B-2 proceeds on "bound the read regardless of which mechanism delivered the kill"; **G-LV.2 stays open as cause-not-attributed** (OOM and transport-close falsified by B-1's measurements; leading candidate client-side reap, not instrumented). Design: per-array caps + the existing `truncated`/`total_matching`/`returned` vocabulary on the five uncapped arrays, PLUS a global `MAX_RESPONSE_BYTES` backstop checked **while building** (overflow → `complete: false` + `sections_not_computed`; "did not fit" is never rendered as a zero). Continuation token DEFERRED as unmotivated by the census.
- **L-3: default caps approved as proposed** — `span_summaries: 200`, `semantic_summaries: 200` (ordering by `wasted_runtime_s` desc becomes documented contract), `drill_samples: 500`, `toolpath_summaries`/`drill_summaries` uncapped, `MAX_RESPONSE_BYTES: 8 MiB`. `inspect_spans` gets the same cap in the same commit for uniformity (latent, not a measured cost — B-2 must not claim it fixed one).
- **L-4: compact JSON everywhere** — `json_str` switches from pretty to compact on all 68 tools (measured waste 23.8–46.6%).
- **L-5: keep id semantics + fix the doc + REFUSE an unmatched id** (an unmatched id currently returns an empty skeleton indistinguishable from a real empty result; ids 4/5/6 on wanaka are simultaneously valid indices and ids of different toolpaths).
- **L-6: accepted.** Plan §0 rule 10 corrected by the orchestrator in this commit: the remedy is **unset `WAYLAND_DISPLAY`**; `WINIT_UNIX_BACKEND=x11` is inert on winit 0.30. Orchestrator note (not editing A-1's entry, rule 7): A-1's stated screenshot resume condition names the inert variable — A-2 has been handed the corrected remedy in its brief.

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

---

## B-1 — G-LV.2 crash capture + MCP read-size census, 2026-08-08

Status: AWAITING_CHECKPOINT (Checkpoint L). **The crash did not reproduce; both of the ledger's candidate causes are falsified.** Research-only; no production file touched.

Commit(s): this entry + `planning/review_2026-08-08/GLV2_CRASH_CAPTURE.md` + `planning/review_2026-08-08/artifacts/b1/` (four scripts, a README, and the stripped raw measurement rows). No code, no fixtures, no fingerprints.

Parent/revision measured: repo at `ecd1f60` (branch `tech-debt-3`); **binary under test `target/release/rs_cam_gui` built 2026-08-07 03:08 from `d820226`** — the same build the incident ran on. Verified rather than assumed: `git diff --name-only d820226..HEAD` returns nothing outside `planning/`, so the tree has not moved under the binary. No Cargo job was launched by this wave at all — the existing release binary was executed directly, so the machine-wide slot stayed free for A-1 throughout.

Executor: B-1 agent, over its **own** isolated `rs_cam_gui --mcp` instances. The operator's live GUI (PID 3837079) was never driven — no `mcp__rs-cam__*` call was made in this wave; the only interaction with it was two read-only `ps` lookups of its RSS and its parent PID.

Question and pre-registered bars: plan §3 B-1. (1) Reproduce the two-step G-LV.2 recipe under instrumentation and **capture the actual cause — the OOM-by-serialization hypothesis is unverified and must not be assumed**; (2) census the response-size distribution of every parameterised MCP read on a wanaka-scale trace; (3) gather the `toolpath_id` id-vs-index evidence for B-2. Bars I set before starting: (a) distinguish "process died" from "transport closed but process alive" explicitly, with the liveness probe recorded either way; (b) an unreproduced crash is reported as unreproduced, and no mechanism is named that was not observed; (c) the census is fired **before** the fatal call so a crash loses no measurements; (d) abort my own process if system MemAvailable drops under 4 GB rather than let the OOM-killer choose between my GUI and the operator's.

Fixture/population/resolution: a scratch **copy** of `planning/airrun_2026-06-01/wanaka.toml` (`wanaka_full_tuned`, 2 setups, 9 toolpaths, 7 enabled); the operator's file was never opened for write and is unstaged. `generate_all {fixpoint, simulation_resolution_mm: 0.1}` → `generated: 7, rounds: 3, simulations: 2`; then `run_simulation {resolution: 0.1}`. The resulting trace is the incident's trace **number for number**: `sample_count` **1,588,883**, `issue_count` **73,326**, `hotspot_count` **394**. Two independent full runs (`run1`, `run3`), plus one aborted Wayland run (`run2`) and three no-project transport probes.

Render/artifact paths: `planning/review_2026-08-08/GLV2_CRASH_CAPTURE.md`; `artifacts/b1/{mcp_stdio_client,glv2_repro,pipe_teardown_probe,census_table}.py`, `artifacts/b1/README.md`, `artifacts/b1/measurements/{run1,run3}_{calls.jsonl,rss.csv}` + `teardown_results.json`. Session scratch holds the unstripped logs (`gui_stderr.log`, `gdb.log`, the 56 MB payloads) and is not a repository artifact.

Result (fact), interpretation, and uncertainty:

- **Fact — the crash did not reproduce.** `get_cut_trace` with no arguments, on the population-identical trace, returned **60,517,035 bytes on one JSON-RPC line (56,225,225 bytes of tool text) in 1.71 s**, and the process was **alive**: the next `list_toolpaths` answered in **0.133 s**. Repeated in a second fresh process: **the same byte counts to the byte**, 1.672 s, alive again.
- **Fact — the OOM hypothesis is falsified.** RSS across the fatal call moved 4.31 GB → 4.81 GB (**+0.50 GB**); system MemAvailable never went below 20.0 GB; `VmSwap` stayed 0. Session peak RSS was 8.84 GB and it occurred during `run_simulation`, not during any read. Under `gdb --batch` no signal was ever delivered, and `grep -icE "panic|memory allocation|abort|SIGSEGV|SIGABRT|stack overflow"` over both the GUI stderr and the gdb log returns **0** and **0**.
- **Fact — "the transport closed and took the server down" is also falsified.** Three isolated probes: stdin EOF; the reader stopping mid-write with the 64 kB pipe full and then closing its fd; both fds closed at once. **The GUI survived all three.** This matches `app.rs:84-118` — the rmcp service runs on its own thread and its termination touches nothing that owns the GUI's lifetime.
- **Fact — what *is* confirmed is the size.** Across the 64-call census the unfiltered payload is **16,245× the median response** (3,461 B) and **710× the largest response of any tool other than `get_cut_trace`** (79,135 B). Section accounting: **`span_summaries` is ≥ 94.00 %** of it — 35,838 objects averaging 1,475 B — and **every other key together is 411,847 B**. One operation (Unified Finish, id 15 / index 8) contributes 33,195 of those spans and 51.6 MB of the 56.2 MB, so **a per-toolpath filter is not a sufficient bound**. Raising `max_issues` to 100,000 and `max_hotspots` to 10,000 changed a response by **zero bytes** — the two caps that exist bind nothing. Separately, **23.8 % of the payload is pretty-print whitespace** (46.6 % on `get_diagnostics`), removable with no wire change.
- **Fact — the `toolpath_id` defect is sharper than the ledger recorded.** The doc says index (`rs_cam_mcp/src/server.rs:298`), the handler wraps it as a raw id (`app/mcp.rs:1380`), and every other per-toolpath read takes an index. Measured index→id map `[0→14, 1→4, 2→7, 3→12, 4→5, 5→6, 6→10, 7→11, 8→15]`. The ledger's benign half reproduced (`toolpath_id: 0` → a 3,972-byte empty skeleton; `14` answers). The half it did not record: for **4, 5 and 6 the value is simultaneously a valid index and a valid id pointing at a different toolpath**, so an agent following the documentation gets **another toolpath's data with no error and no warning** — `toolpath_id: 5` returns index 4's 2.1 MB. And the empty skeleton is indistinguishable from a real empty result: `issue_count: 0` sits beside `issue_count_project_wide: 73326`.
- **Fact, cross-lane — `WINIT_UNIX_BACKEND=x11` is inert on this build.** winit removed it in 0.29 ("in favor of standard `WAYLAND_DISPLAY` and `DISPLAY`", `winit-0.30.13/src/changelog/v0.29.md:134`); this workspace is on winit **0.30.13**. Measured: a smoke run with the variable set came up on Wayland (`sctk_adwaita` in the log) and `load_project` never returned (killed at 162.6 s); unsetting `WAYLAND_DISPLAY` instead, the same call answered in **0.515 s**. This affects plan §0 rule 10, the `--mcp` workflow note in CLAUDE.md, and **A-1's stated resume condition for its NOT EXERCISED screenshot**, which names the inert variable. Not corrected by me — A-1's entry and the plan are not my territory.
- **Fact, for B-3.** Dispatch latency, not work, dominates every read: **50 of the 64 calls** landed between 0.94 s and 1.08 s regardless of returning 51 bytes or 2.1 MB. On an idle, visible, X11 window with nothing generating, a frame-loop MCP round-trip has a floor of about **one second**.
- **Interpretation.** The read is pathological and must be bounded, and that conclusion does not depend on naming the killer: a response that is never 60 MB cannot break any client. But the killer is **not named**. Both mechanisms the ledger offered are dead, and the leading remaining candidate — the MCP client failing on a 60.5 MB frame and reaping its child — is **consistent with every observation and not established**. It is supported only by the fact that `rs_cam_gui --mcp` runs as a **direct child of `claude`** (verified read-only on the live instance), which makes the process lifetime client-owned. C25's re-open condition, "a *timed* read that exceeds the gate on the frame loop", is **still not met**: 1.71 s is bad manners, not an outage. The case for bounding rests on bytes, not on seconds.
- **Uncertainty, stated.** (a) The crash was not reproduced; §4 of the deliverable names the two uncontrolled variables. (b) The **Wayland backend is NOT EXERCISED** — the matching re-run aborted at `load_project` after 90 s with no response, which is G-LV.1's parked frame loop verbatim (the same mechanism A-1 hit on the live instance for its whole wave). (c) Claude Code's client-side size limits were not instrumented — out of scope, and the standing instruction forbade using the live MCP session. (d) No attempt was made to find a server-side size cliff **above** 60.5 MB, so the finding is "60.5 MB is served without fault", not "there is no cliff". (e) Core-dump attribution would have been degraded had anything faulted (`coredumpctl` absent, `dmesg` restricted, release binary stripped, `yama/ptrace_scope == 1`); gdb-as-parent was chosen to avoid depending on any of it, and in the event nothing faulted.

Red-first evidence / fingerprints changed: **none — research-only wave, no behavioural change, no fingerprint captured or moved.** The pre-fix reproduction is recorded permanently in the deliverable, including the two runs' byte-identical response sizes, so B-2's sentry has a pre-registered number to fail against: an unfiltered `get_cut_trace` on a 33k-span operation currently emits 56,225,225 bytes with no `truncated` key on any of the five uncapped arrays.

Verification (focused commands + exact state): no build and no test — nothing compilable changed. `git branch --show-current` → `tech-debt-3`. Machine discipline: `free -g` before every launch (MemAvailable 19–23 GiB throughout, floor never approached; the 4 GB safety valve never fired), bracketed `pgrep -af "carg[o]"` clean, disk 179 G free, **no Cargo job launched by this wave**, one GUI instance of mine alive at a time and every one reaped. Explicit staging only; `planning/airrun_2026-06-01/wanaka.toml`, `planning/review_2026-07-27/`, `planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md` and the `e_impl/*.pgm` strays remain untracked/unstaged and byte-unchanged.

NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition:

- **G-LV.2's cause remains unattributed** — but the ledger row's own two hypotheses are now closed as **falsified**, which is a strict improvement on "unattributed". Owner: B-2's checkpoint ruling (question L-1). Re-open condition for the *cause*: an incident captured with the client instrumented, or a Wayland-backed reproduction that gets past `load_project`.
- **The Wayland reproduction.** Owner: whoever lands B-3/B-4. Re-open condition: a dispatch path that does not depend on frame callbacks, at which point the recipe in `artifacts/b1/` runs unchanged with `--wayland`.
- **The `toolpath_id` doc/behaviour mismatch and the silent wrong-toolpath answer.** Owner: B-2 (already in its charter). The deliverable's §6 recommends keep-id + fix-doc + **refuse an unmatched id** rather than returning an empty skeleton, and gives the measured evidence for preferring that over switching the parameter to index.
- **`inspect_spans` summary mode has no cap** and its `child_count` is a nested scan. **Measured, it is not a problem today** (685 bytes, ~1 s, i.e. frame cadence not compute). Listed as latent; recommended for the same commit, explicitly *not* claimable as a fixed cost.
- **The ~1 s dispatch floor** is reported, not investigated. Owner: B-3.

Next action / checkpoint request: **Checkpoint L requested**, six questions in `GLV2_CRASH_CAPTURE.md` §7.4 — L-1 authorise B-2 to proceed with the cause unattributed and G-LV.2 staying open; L-2 pick option 1/2/3 (recommend **2**: per-array caps + the existing `truncated`/`total_matching`/`returned` vocabulary + a global `MAX_RESPONSE_BYTES` applied while building, continuation token deferred as unmotivated by the census); L-3 approve the default caps (`span_summaries: 200`, `MAX_RESPONSE_BYTES: 8 MiB`, each justified by a census number); L-4 rule on `json_str`'s pretty-printing, which changes bytes on all 68 tools and is a separate ruling from L-2/L-3; L-5 confirm keep-id-semantics for `toolpath_id`; L-6 correct plan §0 rule 10's inert `WINIT_UNIX_BACKEND` prescription. Orchestrator actions: (a) put L to the operator; (b) note that L-6 also touches CLAUDE.md's MCP section and A-1's stated screenshot resume condition; (c) B-2 stays blocked on L.

---

## A-2 — heat-map fix + boundary newtypes + screenshot pair, 2026-08-08 / 2026-08-11

Status: COMPLETE. Checkpoint H executed in full (H1, H2, H3, H4.1–H4.4).
**F-HEATMAP's screenshot obligation is discharged** — the live pair exists.

Work spans two dates because the network died mid-wave on 2026-08-08 and
the session resumed on 2026-08-11. Nothing was committed before the
outage; the whole working set sat uncommitted in-tree for three days and
was verified against `git status` on resume before any further edit. Dates
below are honest about which day each thing happened.

Commit(s), parent `825524f`:

- `69ec3cf` core — `feeds::quantities` (five boundary newtypes + the three
  canonical names), `tool_load::display` (the measure + the per-move
  builder), gate delegation, `chipload_envelopes_for_session` docstring
  corrected, and the A-1 fixture inverted.
- `6754a0b` viz — V1/V2/V3/V4 plus V5/V6/V8/P1/P3.
- `709d6c9` viz — the operating-point card (H4.4).
- `12de0a8` ui/cli — the H2 rename sweep across the remaining census rows.
- `7aa0be0` test — the one assertion the sweep invalidated.
- `d8dc41d` + `1f30fea` docs — `artifacts/a2/` (four GUI captures + driver
  + README) and the census §4.1 status block.

20 files, +1263 / −345. **No file in B-2's territory was staged**
(`rs_cam_mcp/`, `viz/mcp_server.rs`, `app/mcp.rs`, `mcp_bridge.rs`,
`bin/main.rs`).

Question and pre-registered bars: plan §2 A-2, executing Checkpoint H
verbatim. Bars I set before starting: (a) the red run must fail against
**production** code, not a mirror of it, or it is not a red run; (b) no
recipe number may move, and I verify that by running the suites rather
than by asserting it; (c) any surface I claim, I look at — every PNG is
Read back before it is cited; (d) a fallback capture is labelled with the
build it came from, on the artifact's own face.

Fixture/population/resolution: two of them.

- **Synthetic** — A-1's two-arc fixture, unchanged in construction (Ø6
  flat 2F, 18 000 RPM, 2 520 mm/min, DOC 1.5 mm, hard maple, arcs 0.80 rad
  and π, F-035 fraction 0.62 on both arms).
- **Real** — a scratch **copy** of wanaka with every toolpath disabled
  except "Back Rough" (`adaptive3d`, 6 mm 2F flat endmill,
  `stock_source = "fresh"`). 3355 moves, 3:09 cycle time, generated and
  simulated at 0.4 mm. The first attempt used "3D Rough 6", which is
  `from_remaining_stock` and returned `awaiting_prior_stock` with nothing
  generated — recorded because it is an easy trap for the next agent
  building a one-op fixture out of a cascade project.

Render/artifact paths: `planning/review_2026-08-08/artifacts/a2/` —
`heatmap_{before,after}_{viewport,simulation}.png`, `a2_shot.py`,
`screenshot_toolpath_is_not_a_heatmap_surface.png`, `README.md`.

Result (fact), interpretation, and uncertainty:

- **Fact.** All four defective surfaces now read one expression.
  `chipload::achieved_feed_per_tooth_mm` **delegates to**
  `display::achieved_advance_per_tooth`, so "the colour agrees with the
  verdict" is a property of the call graph, not of two transcriptions
  staying in step. The fixture asserts that directly:
  `a.displayed.mm() == a.gate_observed_mm_per_tooth`, bit-equality.
- **Fact.** The five colour classes moved from `rs_cam_viz` into
  `VendorChiploadBand::classify`. A-1 had to mirror those thresholds in a
  test file because they were private to a crate it could not reach, and
  flagged the reconciliation as mine; I did it by moving the
  classification to where the band already lived rather than by copying
  it again. `toolpath_render` keeps only class → RGB. **No threshold and
  no RGB triple moved** — its six colour tests are byte-identical probes
  (band 0.05–0.10, same five values, same triples) and are the pin on
  that claim.
- **Fact.** `chipload_envelopes_for_session`'s docstring claimed "its one
  GUI consumer". Corrected, and counted **after** the change as H4.1
  required: three GUI consumers (viewport heat-map, the normalised
  timeline track, the diagnostics badge bound) and two core-side
  (`session::compute`'s strategy advisor and the adaptive feed-modulation
  pass). The fourth GUI consumer is gone deliberately — the surviving
  chip-thickness track is unbanded.
- **Fact (V4, and it was worse than a label).** `SpanAggregate::ingest`
  did `effective_chip_thickness_mm.unwrap_or(chipload_mm_per_tooth)` and
  divided by `n_cutting`. So on a mixed trace the printed average was a
  mean of two different physical quantities **over a denominator
  belonging to neither**. Split into two accumulators, each with its own
  population count. That second half was not in the census and I did not
  go looking for it; it fell out of writing the replacement.
- **Fact (real-project magnitude, previously unmeasured).** On "Back
  Rough" the retired measure paints the path predominantly **red — above
  the band ceiling, "breakage risk"** — while the gate on the same screen
  reads `✓ 1 within · ✗ 0 exceeding` and the badge reads `chipload 100%`.
  Post-fix the same path, same camera, is predominantly **orange**
  (approaching the ceiling), which is what `Within` with a peak near the
  band top looks like. The timeline half is the sharper frame: `load vs
  limit` sitting **above 1.0 across nearly the whole path** three lines
  below an Inspector that says nothing is exceeding.
- **Interpretation, and a correction to my own expectation.** The
  defect's **sign is opposite** on the real fixture to the synthetic one.
  A-1's arm A reads *low* (blue, "rubbing risk") because the chip factor
  collapses at a light arc; wanaka's near-full-slot adaptive3d pass reads
  *high*. Both are one defect — a quantity carrying an engagement-arc
  term compared against a band that has none — and it is a mistake to
  describe F-HEATMAP as "the heat-map reads low", which
  `tool_load/mod.rs`'s old caveat came close to doing. **4.497× is a
  fixture figure and must not be quoted as a wanaka figure.**
- **Uncertainty, stated.** (a) The before/after pair is from **my own
  builds**, not the operator's instance — see NOT EXERCISED below. (b)
  Both scratch builds carry one identical uncommitted line (the default
  colour mode) because the heat-map has no MCP or CLI selector; it cannot
  bias a comparison in which it is on both sides, but it does mean
  neither PNG is of an unmodified binary, and the README says so on its
  face. (c) The real-project run is at 0.4 mm, not the 0.1 mm A-1's
  resume condition named — chosen for wall clock; the colours are a
  comparison between two builds at one resolution, not a resolution-
  independent claim. (d) I did not census the optimizer's internal
  chipload retargeting beyond its display rows (F-OPT / A-8), and did not
  touch A-9's territory.

Red-first evidence / fingerprints changed: **no fingerprint moved, and
that is verified rather than assumed.** Full `cargo test -p rs_cam_core`
— every `_litmatrix_*` suite, `smoke_baseline_regression_f037`,
`wanaka_e2e_chipload_gate`, `feed_explanation_snapshot_b3`,
`chipload_report_wording_t12_t15` — **0 failures, exit 0**.
`cargo test -p rs_cam_viz` 239 + 12 + 11 passed, 0 failed;
`cargo test -p rs_cam_cli` green. Exactly one test assertion changed
(`format_entry_advisory` pinned the string "LUT max", which the H2 sweep
renamed) and it is a string pin, not a number.

The red run is real. A-1's fixture asserted `assert_ne!(displayed_a,
displayed_b)` as a characterization; I inverted it to `assert_eq!` and
pointed it at the production builder and the production classifier, then
reverted `advance_per_tooth_per_move`'s body to the shipped pre-fix
expression — `max(effective_chip_thickness_mm)` per move, verbatim from
`build_chipload_per_move` — and ran it:

```
    DISPLAYED (heat-map)         A 0.009910  B 0.044563
    displayed measure (achieved a/t):  A BelowBand   B Within
thread '...the_heat_map_paints_one_colour_for_one_advance_per_tooth' panicked:
assertion `left == right` failed: one recipe, one colour: the displayed
measure must not carry an engagement-arc term the vendor band does not carry
  left: BelowBand
 right: Within
thread '...changing_only_the_arc_moves_the_chip_thickness_and_not_the_advance_per_tooth' panicked:
assertion `left == right` failed: the displayed measure must not move when
only the engagement arc moves
  left: AdvancePerToothMm(0.009909790933277257)
 right: AdvancePerToothMm(0.044563384065730696)
test result: FAILED. 1 passed; 2 failed; 0 ignored
```

and after restoring: `DISPLAYED (heat-map) A 0.043400 B 0.043400`, both
`Within`, `3 passed; 0 failed`. The pre-fix numbers are **kept live**, not
commented: `the_retired_measure_still_reproduces_the_defect` recomputes
the retired quantity from the production chip model every run and asserts
the recorded values still hold, so the census cannot quietly go stale.

Screenshot sentry, honestly split: **automated** is
`colour_classes_are_monotonic_across_the_band` (walks a value across the
band and asserts the class sequence is exactly BelowBand → JustAboveFloor
→ Within → NearCeiling → AboveBand with no class reachable from two
disjoint regions — a colour that recurred could not identify a band
position) plus the fixture's gate/display bit-equality assertion. What is
**manual** is the pixels: nothing automatically compares a PNG. The pair
in `artifacts/a2/` was Read back and inspected by eye.

Verification (focused commands + exact state):

- `cargo clippy -p rs_cam_core --all-targets -- -D warnings` — clean.
- `cargo clippy -p rs_cam_viz -p rs_cam_cli --all-targets -- -D warnings`
  — clean, run in an isolated worktree at my HEAD.
- **A workspace-wide clippy was NOT run.** B-2's uncommitted slice was
  mid-edit in `app/mcp.rs` for part of this wave (a literal syntax error
  at one point, which is why the worktree exists), and at close two
  foreign `cargo test` jobs were running with MemAvailable at 9.7 GB, so
  §0 rule 8 forbade taking the slot. Every crate I touched is clean
  individually; the workspace gate should be re-run once B-2's slice
  lands. Stated rather than skipped quietly.
- Slot discipline: `free -g` + bracketed `pgrep -af "carg[o]"` before
  every launch; one job at a time; no release build; disk 150 G free.
- Explicit per-file staging on all seven commits; no `--amend`. The
  operator's `wanaka.toml`, `planning/review_2026-07-27/` and the
  `FEEDS_SPEEDS_ARCHITECTURE_REVIEW` remain unstaged and byte-unchanged;
  wanaka was read only, and the screenshot fixture is a scratch copy.

NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition:

- **The operator's own GUI was never captured.** PID 3837079
  (`d820226-dirty`) answered `generation_status` and the snapshot arm of
  `project_summary`, but `served_from: "snapshot"`, `snapshot_age_s`
  142.6, `frame_loop.healthy: false` — every GUI-dispatched call
  unreachable. Three days after A-1 hit it, the same block. The pair was
  taken from my own pre-fix and post-fix builds instead, per the brief's
  sanctioned fallback. Owner of the underlying hazard: **B-3/B-4**
  (G-LV.1 re-open (b)). Re-open condition for a capture on the
  operator's instance: a dispatch path that does not depend on frame
  callbacks. Worth the orchestrator noting that Lane B has now blocked a
  Lane A rule-3 obligation in **two consecutive waves**.
- **`screenshot_toolpath` is not a heat-map surface** — separate
  offscreen renderer, fixed palette, ignores `toolpath_color_mode`
  entirely. A-1's resume condition named it as half of the required pair;
  it cannot serve that role. Captured proof is in `artifacts/a2/`. Not
  fixed. Owner: unassigned; it is arguably fine as a geometry render, but
  the census/ledger should stop asking it for a colour answer.
- **The heat-map has no MCP or CLI selector.** Both capture builds patch
  the default colour mode instead. Owner: unassigned. Re-open condition:
  any future wave that must screenshot a colour mode hits this again — a
  `set_ui_view {toolpath_color_mode}` field would close it, and that file
  is B-2's territory today.
- **I reformatted B-2's in-flight `app/mcp.rs`.** `cargo fmt -p
  rs_cam_viz` cascaded into it (the standing rustfmt-cascade trap, which
  I had already been bitten by once earlier in this wave). Formatting
  only, no logic touched, and I did **not** revert it — reverting would
  have discarded B-2's uncommitted work, which is the worse error. Flagged
  here so B-2 is not surprised by a whitespace diff it did not make. I
  switched to per-file `rustfmt` in an isolated worktree afterwards.
- **V5's underlying datum is still a per-sample commanded peak**, not the
  gate statistic. I renamed it to say so ("peak commanded a/t") rather
  than changing which statistic the hotspot line reports — that would be
  a number move, and this wave is display-only. Owner: whoever owns the
  hotspot rollup. Same note applies to N7/N8 on the CLI.
- **The GUI's viewport heat-map still has no legend.** The census flagged
  it ("no legend, so the operator has no numeric check on the colour").
  Checkpoint H did not rule on it and I did not add one; the hover text
  now names the measure and the band, which is weaker than a legend.
  Owner: unassigned, cheap.
- A-9's territory (F-BIPOLAR / F-VALID / F-MISSAE) deliberately untouched,
  per the review's step-6 warning. Note for A-9: `is_bipolar_engagement`
  still compares raw `effective_chip_thickness_mm` against the advance
  band, and its docstring already explains why the deletion does not
  transfer. It is now the **last** consumer of that comparison in the
  repository.

Next action / checkpoint request: **none — A-2 needs no checkpoint.**
Orchestrator actions: (a) mark A-2 COMPLETE and F-HEATMAP CLOSED (DoD
item 1 is met: heat-map, modal, panel and gate present band-comparable
quantities in one vocabulary, proven by the fixture AND by the
screenshot its ledger row demanded); (b) re-run the workspace clippy gate
once B-2's slice lands, since I could not take the slot; (c) A-8 and A-9
are unblocked — both were sequenced "after A-2" so they would read the
corrected display layer, and they now can.

---

## B-2 — bounded MCP reads + filter fix (executes Checkpoint L), 2026-08-08 / 2026-08-11

Status: COMPLETE. Checkpoint L executed in full (L-2, L-3, L-4, L-5, and
the L-6 rider). **G-LV.2 stays OPEN as cause-not-attributed** — this wave
bounds the read; it does not claim to have fixed the crash. Wave ran
2026-08-08, was interrupted by a multi-day network outage mid-slice, and
resumed and closed 2026-08-11; the three commits straddle that gap.

Commit(s): `3a97b8b` (bounded-response primitives + their sentries,
`crates/rs_cam_mcp/src/response.rs`, nothing calling them yet), `825524f`
(`get_cut_trace` bounded + unmatched-id refusal + `inspect_spans` summary
cap), `afd102b` (compact `json_str`, the five viz-side sentries, the L-6
rider, and the C25 gap those sentries exposed). This entry.

Parent/revision measured: branch `tech-debt-3`; my slice started at
`b70b84f` and closed on top of A-2's eight commits (`395e1a5`). B-1's
census — the evidence every number here comes from — was taken on the
release binary built from `d820226`, the build the incident ran on.

Question and pre-registered bars: plan §3 "B-2 impl", executing the
Checkpoint L ruling verbatim. Bars I set before starting: (a) the pre-fix
behaviour must stay **executable**, not quoted — a sentry that only
asserts the new number cannot fail if someone reverts the mechanism;
(b) `total_matching` must be counted from the population and never
inferred from the emission, or a truncated array reports itself complete;
(c) an unmatched id must be refused, not answered, because the empty
skeleton it used to return is indistinguishable from a real empty result;
(d) nothing in `rs_cam_core` may be touched, so no fingerprint can move
by construction — and I verify that rather than assert it.

Fixture/population/resolution: two populations. The primitive-level
sentries run a synthetic 35,838-entry `span_summaries` population — B-1's
measured entry count, with a row of the shipped shape. The viz-level
sentries build a real `AppState`: N toolpaths added through
`ProjectSession::add_toolpath` (which assigns its own ids, so ids and
indices diverge exactly as they do in production), each with an
`AnnotatedToolpath` of 300 spans in `gui.toolpath_rt`, and a
`SimulationCutTrace` with one cutting sample per span. No simulation
resolution is involved; nothing is dexel-measured.

Render/artifact paths: none — this wave changes no visible surface. The
before/after numbers are byte counts, quoted below and pinned in the
tests.

Result (fact), interpretation, and uncertainty:

- **Fact (L-2/L-3, the bound).** The five uncapped arrays
  (`span_summaries`, `semantic_summaries`, `toolpath_summaries`,
  `drill_summaries`, `drill_samples`) now carry the truncation triple
  `hotspots`/`issues` have had since R-1, plus a `max_*` request parameter
  each and a `_cap` key. Defaults are the ruled ones: span/semantic 200,
  drill_samples 500, the two per-toolpath arrays uncapped,
  `MAX_RESPONSE_BYTES` 8 MiB. The backstop is charged **while building**;
  section order is priority order, with `span_summaries` offered last
  because it was 94 % of the payload.
- **Fact (red-first, primitive).** `ResponseBudget::unbounded()`
  reproduces the pre-fix build. On the 35,838-entry population the
  uncapped array serialises to **> 20 MB** (asserted; a Python
  reconstruction of the same row shape measures **22,877,066 B** compact)
  and the ruled default to **~126 kB** — a ratio the test pins at **> 100×**
  and measures at ~181×. B-1's own figure for the whole response was
  **56,225,225 B** of tool text in a **60,517,035 B** JSON-RPC line, and
  those numbers are in the module's docs permanently.
- **Fact (red-first, shipped path).** The exhibit
  `an_unfiltered_read_answers_bounded_instead_of_emitting_everything`
  runs the **production** builder twice on one fixture. Unbounded: 1,200
  of 1,200 rows, `truncated: false`. Ruled defaults: **200** rows,
  `span_summaries_total_matching: 1200`, `truncated: true`, response under
  the 8 MiB backstop. The uncapped arm is an assertion, not a comment — if
  someone removes the cap machinery the exhibit stops demonstrating
  anything and fails.
- **Fact (L-5, before).** B-1 measured `get_cut_trace {"toolpath_id": 0}`
  returning **3,972 bytes** of well-formed skeleton with every array empty
  and `issue_count: 0` printed beside `issue_count_project_wide: 73326`.
  On that project the values **4, 5 and 6 were simultaneously valid
  indices and valid ids of different toolpaths**, so an agent following
  the doc string received another toolpath's data silently.
- **Fact (L-5, after).** An unmatched id is an error naming the valid ids
  and correcting the misreading ("the project-level ID … NOT the index").
  The doc string on `CutTraceParam::toolpath_id` said *index*; it now says
  *id* and says why the distinction is not academic. Sentry has a
  non-vacuity arm: a valid id still answers **with data**, so the refusal
  did not become a blanket one.
- **Fact (L-4, compact JSON).** `json_str` is `to_string`, not
  `to_string_pretty`, for all ~68 tools; `format_result`'s error arm too.
  B-1's measured saving: **13,392,073 bytes (23.8 % of the served payload,
  31.3 % of the compact one)** on the large `get_cut_trace`, and **46.6 %**
  on `get_diagnostics` / `run_simulation` (79,135 B → 42,295 B). I
  searched for tests or fixtures asserting pretty output and found
  **none** — every consumer in the tree parses.
- **Fact, and it is a defect I introduced and then caught.** Writing the
  byte-starvation sentry showed `sections_not_computed` was
  **unreachable** from `get_cut_trace`: every array went through
  `insert_capped`, which could only truncate, so a budget-starved array
  would have shipped as `[]`. That is precisely C25's forbidden shape —
  "did not fit" wearing the clothes of "there is none". Fixed in
  `afd102b`: a capped array that admitted **nothing** from a **non-empty**
  population is a dropped section (absent, named, `complete: false`,
  counts retained), while a genuinely empty population still ships `[]`.
  Both halves are sentried, in one test, because they are only meaningful
  against each other.
- **Fact (L-6 rider).** Two production strings recommended
  `WINIT_UNIX_BACKEND=x11`, inert since winit 0.29. `main.rs` was worse
  than wrong: it **used** the variable as a suppression condition, so
  setting it switched the Wayland-park warning off while changing no
  behaviour. Both now say "unset `WAYLAND_DISPLAY`", the suppression is
  gone, and B-1's measurement (162.6 s hang with it set vs 0.515 s with
  `WAYLAND_DISPLAY` unset) is quoted at the site.
- **Interpretation.** The census's design conclusion holds up in code: the
  per-array caps do all the work and the byte backstop is insurance for
  the *next* uncapped array rather than for the five known ones — on the
  fixture it never binds at the defaults. The one thing implementation
  changed about the design is the starvation case above, which the ruling
  named as a risk ("needs care that 'did not fit' is never rendered as a
  zero") and which turned out to need an explicit mechanism, not care.
- **Uncertainty, stated.** (a) The after-number is measured on a
  **synthetic** population through the production builder, **not** on a
  live 1.6 M-sample trace — see NOT EXERCISED. (b) The claim "8 MiB is
  never binding at the defaults" is fixture-scoped; a project with far
  more toolpaths could plausibly starve a later section, which is what
  the naming machinery is for. (c) `inspect_spans`' cap is **latent**:
  B-1 measured that mode at 685 bytes on a 33,195-span operation, so I
  removed an unbounded array and did **not** fix a measured cost. The
  code comment says so in those words.

Red-first evidence / fingerprints changed: **no fingerprint captured,
moved or re-pinned; no gate, threshold, severity, recommendation or
recipe number touched.** This wave changes **zero files in
`rs_cam_core`** — the entire diff is `rs_cam_mcp` (a new module + one
serialiser line + parameter docs) and four `rs_cam_viz` MCP-surface
files — so no generator, gate or fingerprint input is reachable from it.
Verified rather than assumed: `cargo test -p rs_cam_core -q` run at
close-out, result recorded under Verification.

Verification (focused commands + exact known-red state):

- `cargo test -p rs_cam_viz -q` — **244 lib passed, 12
  `mcp_escape_hatches` passed, 11 `wizard_e2e` passed, 0 failed.** The 12
  escape-hatch sentries are green, as the plan requires.
- `cargo test -p rs_cam_mcp -q` — **13 passed, 0 failed** (8 new).
- Known-red states, both real and both caught by the sentries that
  demanded them: the byte-starvation test failed twice before the C25 gap
  was fixed (`complete` read `true` where a section had been dropped —
  `left: Bool(true), right: Bool(false)`), and the whole viz suite was
  briefly red on a stale test binary that reported 239 tests where 244
  exist. Both are recorded here because "239 passed" looked like success.
- `cargo clippy --workspace --all-targets -- -D warnings` — **clean**
  (only the pre-existing future-incompat note for transitive `nom 3.2.1` /
  `quick-xml 0.22.0`). This is the workspace gate A-2 could not take; the
  branch had no verified workspace-clean state since `b70b84f` and now
  has one **at `afd102b`, on the merged A-2 + B-2 tree**.
- `cargo fmt --check --all` — **clean**. One file of mine needed
  formatting; I ran `cargo fmt -p rs_cam_mcp` (crate-scoped, all three
  files mine) and re-checked the workspace, per the rustfmt-cascade rule.
- Slot discipline: `free -g` + bracketed `pgrep -af "carg[o]"` before
  every launch, with polling waits whenever A-2 held the slot (it held it
  for most of 2026-08-11). One violation, stated: an early compound
  command ran its `pgrep` and its `cargo` in the same invocation, so it
  launched alongside A-2's `cargo test -p rs_cam_core`. No release build.
  Disk never below 144 G.
- No wanaka file was written; `planning/airrun_2026-06-01/wanaka.toml`,
  `planning/review_2026-07-27/` and the operator's feeds review remain
  unstaged. The orchestrator's uncommitted tracker edit was left alone.

NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition:

- **NOT EXERCISED: a live post-fix rig run.** B-1's rig reproduces the
  60,517,035-byte response deterministically, but it drives a **release**
  binary and §0 rule 8 forbids release builds inside a wave; a debug GUI
  cannot complete the recipe (a 401.8 s `generate_all` + 107.5 s 0.1 mm
  simulation in release) in any usable time. The after-number is
  therefore measured through the **production builder on a synthetic
  population**, which is stated everywhere it is quoted and is not the
  same as a live byte count. Owner: the programme's close-out live
  validation. Re-open condition: a release build exists — then re-run
  `artifacts/b1/glv2_repro.py` unchanged and record the new size of call
  63; the expectation is low hundreds of kB.
- **G-LV.2 stays OPEN, cause-not-attributed.** B-1 falsified both
  ledgered hypotheses (serialisation OOM; transport close) and named
  client-side reap as a candidate it could not instrument. This wave
  removes the *pathology* — no read can emit 60 MB — but attributes no
  mechanism and closes nothing. Owner: unassigned. Re-open/close
  condition: instrument a real MCP client's frame-size behaviour, or
  observe the crash again on a bounded build (which would falsify the
  size hypothesis outright).
- **The `toolpath_id` sweep found exactly one offender.**
  `set_boundary_config`'s `source_toolpath_id` already documents itself as
  an id **and** already refuses an unmatched one
  (`app/mcp.rs:3324`), so it needed no change. Every other per-toolpath
  read (`narrate_toolpath`, `inspect_spans`, `get_toolpath_diagnostics`,
  `get_generation_debug_trace`) takes an **index** and says so.
- **Continuation tokens: deliberately not built** (Checkpoint L deferred
  them as unmotivated by the census). If an agent ever legitimately needs
  all 33k span summaries of one operation, the answer today is
  `span_kind` / `pass_index` narrowing, and the response says so.
- **`get_toolpath_diagnostics` / `get_project_diagnostics` remain
  uncapped** over their whole `Vec<Diagnostic>` (B-1 §5.2). Checkpoint L
  did not rule on them and they measured 2 KB–3.6 KB, so I left them
  alone rather than widen the ruling. Owner: unassigned; cheap, and the
  machinery now exists.
- **`get_generation_debug_trace`'s `max_spans: 0` still means unlimited**
  — an uncapped escape hatch on a bounded read, untouched because it is
  documented and was not in the ruling.

Next action / checkpoint request: **none — B-2 needs no checkpoint.**
Orchestrator actions: (a) mark B-2 COMPLETE and record that the
workspace clippy + fmt gate now has a verified clean point at `afd102b`
covering both lanes; (b) keep **G-LV.2 open as cause-not-attributed** —
DoD item 5 says "the crash cause is named, not hypothesised", and it is
still not named, so that item is **partially** met (bounded reads have
sentries; the cause does not); (c) B-3 may proceed — it is the wave that
owns the ~1.0 s dispatch floor B-1 measured, and nothing here touched
dispatch; (d) note for whoever writes the close-out live validation that
the post-fix byte count is the one measurement this wave owes and cannot
take without a release build.

### B-2 addendum, 2026-08-12 — the core-suite result, and one PRE-EXISTING red

My entry above promised the `cargo test -p rs_cam_core -q` result "under
Verification" and then quoted no number, because the run was still going
when the entry was written. Recording it now rather than leaving the
promise dangling, and recording that it was **not clean**.

- **Result: `cargo test -p rs_cam_core -q` — 1 test FAILED.**
  `literature_matrix::run_literature_matrix`: *"1 cell(s) reached major+
  severity: `flat_3mm_pocket_ipe_extreme`: critical
  (`anti.ipe_micro_matches_oak_micro_chipload`: anti-pattern
  `feed_rate / (rpm * flutes) > 0.030` triggered (= 1))"*. 20 passed, 1
  failed. Deterministic — reproduced on three consecutive runs.
- **Instrument note, because I nearly filed a false green.** My first
  core run reported "40 binaries, 2,430 passed, 0 failed" and exited 0.
  That was an artefact of my own command: `cargo test … | grep … | head
  -40` truncated at 40 matches and returned **grep's** exit status, not
  cargo's. The crate has ~160 test binaries. The re-run without `head`
  is the one quoted above. A pipeline's exit code is not the test suite's.
- **Attribution: NOT B-2, and NOT A-2's core change.** Two independent
  facts. (1) None of B-2's four commits touches a single file under
  `crates/rs_cam_core/` — verified with `git show --stat`. (2) The same
  test fails **identically** in a clean worktree at `825524f` (my last
  commit before A-2's core work), and `git diff 53b1c72 825524f --
  crates/rs_cam_core/src/` is **empty** — the core source at that
  revision is byte-identical to master. The red therefore **pre-dates
  the whole TD3 programme** and is present on `master @ 53b1c72`.
- **It does contradict a claim in the log.** `69ec3cf`'s commit message
  states "Full `cargo test -p rs_cam_core` is green, 0 failures,
  including the litmatrix suites". On this tree that is not true, and it
  was not true before A-2's change either. I am not editing A-2's entry
  (rule 7); flagging it here so the discrepancy is on the record and the
  next agent does not trust the claim over the suite.
- **NOT FIXED, and deliberately so.** The failing cell is a feeds
  *recipe* number on a Ø3 tool in ipé — Lane A territory, sub-Ø2/small-
  diameter scaling-law country (`D^0.61` / `Janka^-0.5`, adopted
  2026-08-06, repo-derived and unsourced per `CREDITS.md`). Touching it
  is a number move that needs a checkpoint, not a fix a Lane B wave may
  make on its way past. Owner: Lane A (A-6/A-9 are nearest). Re-open
  condition: none needed — it is red now and stays red until ruled.
- Unaffected by all of the above: the B-2 gates quoted in my entry stand
  — 244 viz lib / 12 `mcp_escape_hatches` / 11 `wizard_e2e` / 13
  `rs_cam_mcp` green, `cargo clippy --workspace --all-targets -D
  warnings` clean, `cargo fmt --check --all` clean at `afd102b`.
- **Fingerprint statement, now properly evidenced.** No fingerprint moved
  *because of this wave*: the wave's diff contains no `rs_cam_core` file,
  so no generator, gate or fingerprint input is reachable from it, and
  the one core red is reproduced at a revision whose core is master's.

---

## A-3 — apply-contract census, 2026-08-12

Status: AWAITING_CHECKPOINT (Checkpoint I). Research-only; **no production
file touched.** Both hazards the review named reproduce, and a **third,
more severe one** was found while censusing.

Commit(s), parent `675a643`: this entry +
`planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` +
`crates/rs_cam_viz/tests/apply_contract_a3.rs` (9 characterization tests, new
file). No production code, no fixtures, no fingerprints, no core file.

Parent/revision censused: `tech-debt-3 @ 675a643`. **Drift from the review's
cited lines is recorded per-site in the deliverable §1** — A-2's rename sweep
and operating-point card moved the panel's split apply by about **+250 lines**
(`properties/mod.rs:1724-1930` → `:1982-2001` speeds, `:2024-2043` cut
geometry); `feeds_result_for_operation` moved **+8** (`suggest.rs:637` →
`:645`); `feeds_explain_for_operation` (`:671-689`), `feeds_modal.rs:381-587`
and `controller/events/mod.rs:780-948` did **not** move materially. Every
claim in the review's Evidence paragraph reproduces at this revision.

Question and pre-registered bars: plan §2 A-3. Bars I set before starting:
(a) a write path counts as censused only if I can name its UI label string,
its handler, and whether it runs `enforce_invariants` — not just its file
line; (b) the evidence class of every demonstration is **stated on its face**,
and a class I cannot obtain is reported NOT EXERCISED with the blocker named
rather than substituted for silently; (c) "the apply wrote nothing" is not
accepted as "the apply was refused" until the operation is checked for the
dial; (d) no assertion goes in that I have not seen a measured number behind.

Fixture/population/resolution: three synthetic fixtures, all on the shipped
default **Ø6.35 mm 2-flute flat end mill** (`ToolConfig::new_default`,
`ToolType::EndMill` → `ToolGeometryHint::Flat`, stickout 45 mm), default stock
material/machine/workholding, embedded vendor LUT.
(1) **Scallop** — refused pairing, carries neither cut-geometry dial.
(2) **DropCutter + `scallop_height = 0.01`** — refused pairing that *does*
carry `stepover` (second arm of `validate_tool_for_operation`).
(3) **Pocket** — valid pairing, carries both dials. No simulation, no dexel
grid, no resolution parameter — this wave measures apply-time writes only.

Render/artifact paths: `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md`;
tests at `crates/rs_cam_viz/tests/apply_contract_a3.rs`. The raw
measurement dump was a scratch test, run once and deleted after capture; every
number it produced is transcribed into the deliverable §3 **and** re-asserted
by a permanent test, so nothing rests on a deleted artifact.
No `artifacts/a3/` directory exists: there is no screenshot to put in it (see
NOT EXERCISED) and every other artifact of this wave is a committed file.

Result (fact), interpretation, and uncertainty:

- **Fact — the census is 20 write paths, 13 of them GUI apply affordances.**
  Validated: **2** (both on the properties panel). Infallible: **11** (all on
  the Feeds & Speeds modal). Geometry-capable: **8**. Speed-only: **5**.
  Through the invariant funnel: **6**. **Raw writes that bypass it: 7.**
  Full table with file:line, UI label string, scope and handler in §2.
- **Fact — hazard (a) reproduces.** Flat end mill on Scallop: the panel gets
  `Err(WrongToolForOperation { operation: Scallop, actual_geometry: Flat, … })`
  and replaces the entire feeds card with `Feeds unavailable: …` — **no apply
  affordance is rendered at all**. The modal, on the same toolpath, recommends
  feed **2677.07**, plunge **793.75**, RPM **10025.5** and offers live Apply
  buttons; `⚡ Apply all` writes feed **1000 → 2677**, plunge **500 → 794**,
  RPM **None → Some(10026)**.
- **Fact — hazard (b) reproduces, with numbers.** Pocket fixture, one
  recommendation, two buttons. Panel `⚡⚡ Apply recommended speeds` (tooltip:
  "Does not change the cut (DOC/WOC)") → WOC **2.0 unchanged**, DOC **1.5
  unchanged**. Modal `⚡ Apply all` → WOC **2.0 → 2.222**, DOC **1.5 → 1.27**.
  Speeds identical on both (3000 / 794 / 18000). The modal gives the user no
  way to know the cut changed.
- **Fact — hazard (a) ∩ (b) exists and is worse than either.** DropCutter with
  a scallop target on a flat end mill is refused by the *second* arm of the
  validator and carries a real `stepover`. On that refused pairing the modal
  writes WOC **1.0 → 0.19 mm** — a **5.3× finer** raster — plus feed **1000 →
  2450**. So the modal does not merely offer a refused recipe; it rewrites the
  cut geometry of an operation the engine says cannot be run.
- **Fact — NEW hazard (c), and it is the severe one.** Seven of the thirteen
  affordances (the six per-field `Apply` buttons + the explore-chart apply)
  write `explain.recommended.*` **directly** (`events/mod.rs:796-817`,
  `:955-980`), so **none** of `enforce_invariants`' passes run —
  `clamp_plunge_to_feed`, `clamp_stepover_to_diameter`,
  `backoff_stepover_for_runtime`, `clamp_dpp_to_rigidity`,
  `clamp_dpp_to_cutting_length`, `backoff_dpp_for_deflection`,
  `recalibrate_feed_for_chipload`, nor `round_suggestion_value`. Measured on
  the Pocket fixture the gap is rounding-only on feed/plunge/WOC and **3.50×
  on depth of cut: the modal's per-field DOC `Apply` writes 4.445 mm where the
  panel's `⚡ Apply cut geometry` writes 1.27 mm.** The product's
  smallest-looking affordance — a `small_button("Apply")` in a comparison row
  — writes 3.5× the DOC the engine's own back-off chain permits. Note
  `⚡ Apply all` writes the correct 1.27; the defect is specific to the
  per-field grain.
- **Fact — a measured refinement that could easily have been reported wrong.**
  On the Scallop fixture the per-field DOC and WOC applies write nothing. That
  is **not** a refusal — `ScallopConfig` carries neither dial. The test asserts
  the distinction explicitly (`op.depth_per_pass().is_none() &&
  op.stepover().is_none()`) rather than letting an absent dial masquerade as a
  guard, which is why fixture (2) exists at all.
- **Fact — the agent surface has no apply path.** No MCP tool applies a feeds
  recommendation to an existing toolpath; `get_suggest_rationale` is read-only,
  `set_spindle_strategy` states it mutates nothing. The only agent write is
  `set_toolpath_param`, which is neither feeds-validated nor funnelled — i.e.
  an agent has the modal's contract with none of the modal's preview. Recorded
  as Checkpoint I question Q5, not treated as in-scope.
- **Interpretation.** The defect is not "the modal forgot to validate". It is
  that `FeedsExplain` — correctly designed as an *infallible chart payload* —
  became a write source, and the Apply buttons reached for the shortest
  available setter instead of the `ApplySubset` API that already existed one
  module away. So the repair must make a preview **structurally unapplicable**,
  which is exactly the review's `FeedsPreview` / `ApplicableRecommendation`
  split. §4 of the deliverable states the three individually-reasonable
  decisions that composed into it, so A-4 does not recreate the shape.
- **Uncertainty, stated.** (a) The measured numbers are one tool × one material
  × three ops; the 3.50× DOC ratio is fixture-specific and the *existence* of
  the bypass is what generalises, not the multiplier. The test asserts `> 3.0×`
  rather than an exact value for that reason. (b) The button → `AppEvent`
  mapping is read from source, not clicked — see NOT EXERCISED. (c) Optimizer
  paths O1–O3 are censused but not exercised; O2 looks like hazard (c) in a
  second neighbourhood and is raised as question Q4 rather than asserted.

Red-first evidence / fingerprints changed: **none — research-only, no
behavioural change, no fingerprint captured or moved.** No `rs_cam_core` file
is in this wave's diff, so no generator, gate or fingerprint input is reachable
from it. The pre-fix reproduction is permanent in two places: the deliverable
§3 tables and the doc comments of the nine tests, so A-4 has pre-registered
numbers to fail against (deliverable §5.4 states the four bars, including
"the DOC gap closes to 1.00×" and "no recipe number moves on a valid pairing").

Verification (focused commands + exact state):
`cargo test -p rs_cam_viz --test apply_contract_a3 -q` → **9 passed, 0
failed**, 0.01 s. `cargo fmt -p rs_cam_viz -- --check` → clean.
`cargo clippy -p rs_cam_viz --test apply_contract_a3 -- -D warnings` → clean
(see the shared-tree note below for why it took two attempts).
`git branch --show-current` → `tech-debt-3`. Machine discipline: `free -g`
(15 GiB available) + bracketed `pgrep -af "carg[o]"` before every launch, no
concurrent Cargo job at launch, **no release build**, per-crate tests only,
disk 144 G free. Explicit staging only; `planning/airrun_2026-06-01/wanaka.toml`,
`planning/review_2026-07-27/`, the operator's review file and the `e_impl/*.pgm`
strays remain untracked/unstaged and byte-unchanged by me. The known
pre-existing core red (`literature_matrix::flat_3mm_pocket_ipe_extreme`,
intake G-LIT-IPE) was **not encountered** — this wave ran no core test, having
touched no core file.

NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition:

- **NOT EXERCISED: live GUI screenshots of the two hazards.** Plan §2 A-3 asks
  for them. Blocker: the operator's GUI/MCP session was disconnected for the
  whole wave, and §0.8 forbids the release build `.mcp.json` launches. A debug
  GUI with `WAYLAND_DISPLAY` unset was available and deliberately not used: a
  screenshot of a modal is evidence about **labels**, and every label is
  already quoted verbatim from source in §2, so it would not have carried a
  fact the tests do not. Owner: A-4, which must render its fixed modal anyway
  under rule 3. Re-open condition: an operator session, or the moment A-4 has
  a before/after pair to shoot.
- **NOT EXERCISED: MCP-driven repro.** Impossible in principle — MCP cannot
  inject a modal click and there is no MCP apply tool to stand in for one. No
  re-open condition; recorded so nobody looks for one.
- **NOT FIXED: everything.** This is a research wave; all 13 affordances ship
  unchanged. Owner: A-4, blocked on Checkpoint I.
- **Optimizer paths O1–O3 not exercised.** Censused from source only. Owner:
  A-8 (its charter) or A-4 if the operator answers Q4 by folding O2 in.
  Re-open condition: the Q4 ruling.
- **Shared-tree note, resolved — recorded because the next agent will hit it.**
  The first `cargo clippy -p rs_cam_viz --test apply_contract_a3 -- -D warnings`
  failed to compile, and **not because of anything in this wave**: S-4's
  in-flight edit to `crates/rs_cam_core/src/compute/{stats,config}.rs` was
  mid-flight in the shared tree (`stats_with_findings` had grown a 4th
  parameter its callers did not yet pass). Re-run after their edit settled:
  **clean, zero warnings**. The lesson for parallel waves is that a red
  build in a shared tree must be attributed before it is acted on — my own
  test had compiled and run green against the same core minutes earlier.

Next action / checkpoint request: **Checkpoint I requested**, six questions in
`APPLY_CONTRACT_CENSUS.md` §5.2–§5.3 —
**I-1 (the ruling the plan names)**: remove modal write affordances vs reroute
them to the funnel. Three costed options; **recommend C** — reroute the four
batch/`Apply all` paths + the explore apply, **delete the six per-field
buttons**, which are the ones carrying the 3.50× funnel-bypass and are the
clearest instance of the review's own "do not preserve a legacy all-fields
write merely for UI muscle memory";
**I-2** whether `ApplyScope` gains a `Field(FeedsField)` arm (needed only if
I-1 = B);
**I-3** what a refused pairing renders in the modal (recommend: open it, draw
the charts, replace the whole Apply column with the refusal);
**I-4** whether the three optimizer writes join the funnel (recommend: exclude
O1/O3 as sim-verified, route **O2** in — it is a raw single-dial write of an
un-simulated suggestion);
**I-5** whether the agent/MCP surface gets an apply tool under the new funnel;
**I-6** whether A-3's nine tests are inverted in place or superseded (recommend
in place — §0.1 requires the pre-fix reproduction to stay).
Orchestrator actions: (a) put I to the operator; (b) note I-4 overlaps A-8's
charter and I-5 overlaps Lane B, so both may want deferring rather than ruling;
(c) A-4 stays blocked on I; (d) the funnel design in §5.1 is mechanically
small — `apply_feeds_subset` already *is* the funnel and `ApplySubset` already
*is* `ApplyScope` minus `Field` — so I-1 = C is a same-day change once ruled.

## S-4 — G-BYTE frozen-snapshot A/B, 2026-08-12

Status: COMPLETE. The A/B ran, the identical-branch fired, and the
provenance fix shipped with the harness as its sentry. **No promotion
needed: G-BYTE is not a nondeterminism defect and A-5's before/after
fixtures are not blocked.**

Commit(s), parent `675a643`: `85f40a1` (harness + `ToolpathStats::
stock_snapshot` + the join parameter + the two production call sites + the
narration and h21 guards), plus this entry, `GBYTE_AB_REPORT.md` and
`artifacts/s4/`.

Question and pre-registered bars: plan §4 S-4. Bars set before starting:
(a) byte-equality is asserted on **three independent renderings**, not one —
move list, whole `AnnotatedToolpath`, and emitted G-code — and the G-code is
exported with `sim_trace: None` under an all-accepting policy so its text
cannot move because a *gate* moved; (b) an "identical" result is not accepted
until a control arm has been shown to make the same harness produce a
**different** result, otherwise the equality is a property of the harness;
(c) non-vacuity asserted, not assumed (>500 moves, >500 lines) — a degenerate
pass makes byte-equality trivially true; (d) the production stamp is checked
against an **independent witness** computed outside production code, never
against itself; (e) any diff-classification whose alignment I cannot trust is
suppressed rather than reported.

Fixture/population/resolution: two-op `UnifiedFinish` cascade, op 1 on
`StockSource::FromRemainingStock` — the incident's shape. Ø3 ball nose,
40 × 40 mm smooth double-bump height field in the top 2 mm of a 6 mm stock,
heights pinned 6.0 / 4.0, finish stepover 1.5 mm vs rest 0.5 mm so the rest
pass has ~0.19 mm cusp ridges in front of it. Frozen snapshot at **0.25 mm**
(under the Ø3 tip radius, per `feedback_rest_measurement_prerequisites`);
control cells 0.30 and 0.40 mm. Population 6 519 moves / 7 208 G-code lines /
187 557 B. **The operator's `wanaka.toml` was not touched, read or copied.**

Result:

- **VERDICT — IDENTICAL BYTES.** Arm A (nothing between the two generations)
  is byte-identical on all three renderings: `6519 / 04e0a63a8812b572` twice,
  annotated hash `dd222b9016062d4b` twice, G-code 187 557 B / 7 208 lines,
  **0 differing lines, 0 `diff` hunks**, `assert_eq!` on the whole string.
  Against the incident's 135 hunks / ~140 moved `G0` heights / 12 dropped
  cutting lines. G-BYTE was **snapshot drift, not generator nondeterminism**.
- **The single strongest artifact is `artifacts/s4/SHA256SUMS.txt`.** One
  hash, `ab49460186ba937d…`, covers **six** files — arm A gen1 and gen2, arm
  B1 gen1 and gen2, and the gen1 of arms B2 and B3. Same configuration
  generated six times across four independent test functions, landing on one
  byte sequence every time. Byte-identity here survives separate sessions,
  separate simulations and separate processes, not just two calls in a loop.
- **Re-simulating is not by itself drift** (arm B1, new information). A
  second `run_simulation` at the same 0.25 mm cell allocated a fresh `Arc`
  holding bit-identical material; snapshot digest unchanged, output
  byte-identical. So the incident's "fixpoint round sim vs later explicit
  full sim" is only a cause if the two events actually *differed*.
- **The harness can move the bytes** (arms B2/B3), so arm A is not vacuous:
  0.25 → 0.30 mm gives 6 `diff` hunks / 161 changed lines / 105 fewer moves;
  0.25 → 0.40 mm gives 5 hunks / 3 726 changed lines and 274 187 B.
- **Fix shipped: `ToolpathStats::stock_snapshot`** — cell size, grid dims and
  an FNV-1a digest over every ray's material on all three axes. It joins the
  **two-valued event family** (`zero_removal`, `boundary_clip_dropped`), NOT
  the three-valued A/M9 one, and says so at the field: there is no "measured
  zero" for an identity. Report-only; no gate reads it. Sub-millisecond
  against a ~1.3 s generation.
- **Content-derived, not identity-derived — and arm B1 is the reason.** A
  pointer stamp or a sim-event counter would have called B1's honest re-sim a
  different snapshot and raised a false alarm. Measured before choosing.
- **Threaded as a PARAMETER of `stats_with_findings`, not a
  `GenerationFindings` field**, because only the caller knows which snapshot
  it handed the generator. That preserved H2.1's property: both production
  call sites, the narration guard and the h21 join sentry all broke at
  compile time and were routed with a stated decision, none elided. The
  session stamps `prior_stock_arc`, not the source-gated `gen_initial_stock`,
  because the same `Arc` also reaches the dressup air-cut filter ungated.

Fingerprints: **none moved, verified.** `ToolpathStats` has zero coupling to
`fingerprint.rs`. Green after the change: `finish_resolution_policy_pr3`
(three pinned `(moves, hash)` constants), `checkpoint_b_resolution_ab`,
`crease_own_region_pr6b`, `common_fixtures_smoke_c6`,
`findings_transport_join_h21`, `narration_denominator_and_hints_d7`,
`standing_material_channel_am9`, `retract_trip_channel_am7` — 8/8 binaries,
50 tests, 0 failures. Clippy `-D warnings` clean on core + viz; `cargo fmt
--check` clean with no rustfmt cascade (before/after `git status` compared).

Uncertainty, stated:

- **NOT EXERCISED: the incident's own dominant class**, the ~140 `G0`
  approach heights moving 0.05–0.15 mm. Arm B4 was written to isolate it
  (territory clip off, leaving only the air-cut filter and
  `optimize_entry_descents`) and **could not reproduce it**. Blocker, named:
  this fixture emits only **7** `G0` lines and every split saturates at
  `Z8.000` = raw stock top 6.0 + `PLUNGE_CLEARANCE_MM` 2.0, because
  `max_conservative_top_z_in_disc` is a sliver-safe upper bound (A/M10) that
  refining the cell cannot lower. The mechanism named in the G-BYTE ledger
  row for that class therefore remains **inferred from the code path, not
  measured**. It is carried in the arm's own doc comment, not implied away.
  The verdict does not rest on it — arm A rules out nondeterminism for the
  whole output, `G0` lines included. Re-open condition: a fixture whose entry
  ceilings sit below the raw stock top.
- **NOT EXERCISED: the GUI worker's stamp call site.** It compiles and
  mirrors the session path, but no live GUI/MCP session ran. Blocker: S-4 is
  a core-side wave. Suggest folding into the programme's close-out live
  validation rather than opening a row.
- **Red-first is compile-time here, not runtime, and is labelled as such.**
  The defect was a *missing channel*, so there was no pre-existing runtime red
  to quote; what failed before the fix were the three `stats_with_findings`
  guards plus the narration guard, recorded in the commit message.
- **The in-test G-code comparison is positional, not an LCS.** Exact when the
  line counts match (the arm-A case carrying the verdict); it suppresses its
  own classification when they differ rather than reporting a misalignment as
  a finding. All hunk counts in the report come from GNU `diff -U0` on the
  dumped `.nc` files.

INTAKE — a **second** pre-existing red core test, unrelated to this wave:
`transform_provenance_fingerprints` fails 3/3. Verified pre-existing by
`git stash push` of exactly the seven changed paths, re-running the binary,
and comparing: **identical left/right values on all three tests** (e.g.
`three_pass: left (23, 14265253333427783116) / right (23,
14756822782673573601)`), move counts unchanged (23, 74, 40) in every case —
geometry hashes moved, counts did not. Stash popped, tree restored. Sits
alongside G-LIT-IPE; suggest a row id and an owner.

Next action / checkpoint request: **none — S-4 needs no checkpoint.**
Orchestrator actions: (a) mark S-4 COMPLETE in the tracker; (b) record
against **G-BYTE**: *RESOLVED as diagnosed — cause identified (snapshot
drift), provenance shipped, no determinism defect*, with re-open condition
**"a byte difference between two generations whose `stock_snapshot` stamps
are equal"**; (c) **A-5 is unblocked** — the plan's §5 note that S-4 "should
complete before A-5's fixtures are trusted" is discharged, and A-5 now has a
cheap way to *prove* a before/after pair is comparable rather than assume it:
assert both arms carry the same stamp; (d) take the `transform_provenance_
fingerprints` intake; (e) consider carrying the unexercised `G0` class into
whichever future wave builds a sub-stock-top-ceiling fixture.

---

## A-4 — one application funnel (execute Checkpoint I), 2026-08-12

Status: COMPLETE. All six ruled items executed. Three of A-3's nine tests did
**not** go red, and that is reported below as a bar deviation with a per-test
reason rather than smoothed over.

Commit(s), parent `875816c`:

| commit | what |
|---|---|
| `9026ddb` | core: the funnel — `FeedsPreview` / `ApplicableRecommendation` / `ApplyScope` / `apply` / `resolve_operation_invariants`, + 5 sentries |
| `2801654` | viz: delete M1–M6, reroute M7–M11 + M8 + O2, invert A-3's tests in place |
| `0321677` | mcp: the `apply_feeds` tool (I-5) + 4 agent-surface sentries |
| `bbd42c7` | docs: the two rendered modal states + artifacts README + two superseded-notes on frozen ui_audit surfaces |
| this entry | |

Parent/revision: `tech-debt-3 @ 875816c`. Territory as briefed —
`rs_cam_viz` (feeds_modal, compare component, ui/mod, controller/events,
mcp_server, app/mcp), `rs_cam_core/src/feeds/suggest.rs` (NOT
`compute/` or `session/`, S-4's territory), `rs_cam_mcp/src/server.rs`,
`apply_contract_a3.rs`, `artifacts/a4/`. No file S-4 touched is in this
wave's diff.

Question and pre-registered bars: execute Checkpoint I exactly; bars are
census §5.4 verbatim, restated in the ruling. Bars I added before starting:
(e) the three legacy apply entry points keep their signatures and behaviour,
so the fix is provably additive on the number-producing side; (f) any bar I
cannot meet is reported as not met, with the measurement that shows it.

Fixture/population/resolution: A-3's three synthetic fixtures unchanged
(default Ø6.35 2-flute flat end mill on Scallop / scallop-targeted DropCutter
/ Pocket) for the test work. For the live render, a copy of
`test_data/ux_3d_terrain.toml` with its two model paths absolutised, one
`drop_cutter` toolpath on the Ø6 mm flat end mill, toggled between valid and
refused by setting `scallop_height`. No simulation, no dexel grid, no
resolution parameter — every measurement here is apply-time.

Render/artifact paths: `planning/review_2026-08-08/artifacts/a4/` —
`a4_modal_valid_pairing.png`, `a4_modal_refused_pairing.png`, `README.md`
(provenance, the live `apply_feeds` transcripts, and the three NOT EXERCISED
surfaces with their blockers).

### Result (fact), interpretation, uncertainty

- **Fact — I-1 executed.** The six per-field affordances (M1–M6) are gone,
  along with `crate::ui::FeedsField`, `AppEvent::ApplyFeedsField`, its
  handler, `CompareRow::apply` and the scallop-derived variant in `woc_row`.
  `CompareRow::show` no longer takes an event sink at all — the row cannot
  emit. M7/M9/M10/M11 and M8 route through one new controller entry,
  `apply_feeds_through_funnel`, which calls `feeds::suggest::apply` with an
  explicit `ApplyScope`. No `ApplyScope::Field` arm exists and the enum's
  own docs say why.
- **Fact — the funnel is structural, not defensive.** `FeedsPreview` holds
  the explanation and the refusal together and carries no write method;
  `ApplicableRecommendation` has a private field and no public constructor,
  so `FeedsPreview::applicable()` is the only way to obtain one, and it
  returns `None` exactly when the validator refused. `apply` is the only
  write and always runs `enforce_invariants`.
- **Fact — I-3 executed and rendered.** On a refused pairing the modal opens,
  every chart draws, every number is shown, and the Apply column is replaced
  by `Cannot apply — this tool cannot run this operation` plus the engine's
  own text. Captured live: `a4_modal_refused_pairing.png`. The
  `⚡ Apply all` button is absent in that image and present in the valid-pairing
  one; the comparison grid's trailing column is blank on every row in both.
- **Fact — I-4 executed.** `reoptimize_with_axis_override` now runs its
  accepted axis value through `resolve_operation_invariants` (the funnel's
  clamp stage) and **notifies when a clamp fires** — an operator who accepted
  a number and silently got a different one is the same defect wearing new
  clothes. O1/O3 excluded with the reason written at the funnel in
  `suggest.rs`, not just in this log: their candidates are scored against a
  simulated cut trace end to end, and re-clamping a sim-verified operating
  point against a pre-simulation estimator would substitute the weaker
  evidence for the stronger.
- **Fact — I-5 executed, and exercised live.** `apply_feeds { index, scope }`.
  On the refused pairing it returned `ok: false` with the engine's words —
  `"scallop requires curved tip (need ball|bull|tapered_ball; got Flat on
  Parallel)"` — and on the valid one `ok: true` with
  `changes_the_cut: false`, `stepover: 0.18` (its pre-call value). `scope`
  defaults to `"speeds"`: an omitted scope must not silently rewrite
  geometry.
- **Fact — a design decision inside I-1 that could have re-created the
  defect.** Routing the drag-to-explore apply through the funnel naively
  would let `recalibrate_feed_for_chipload` re-solve the feed the operator
  had just dragged to. `ApplicableRecommendation::with_explored_speeds`
  therefore drops the chipload band on that path, so the clamps run and the
  dragged point survives. Pinned twice — core
  `explored_speeds_survive_the_funnel_but_still_get_clamped` and controller
  `explore_apply_takes_the_clamps_but_keeps_the_dragged_point`, the latter
  asserting plunge 900 → 120 under a 120 mm/min dragged feed.
- **Interpretation.** The census's own diagnosis holds up under
  implementation: the defect was never "the modal forgot to validate", it was
  that an infallible payload became a write source. Once the payload carries
  its refusal, most of the fix falls out of the type system, and the
  remaining work is honest labelling — the "changes the cut" attribution, the
  refused-row marker, and the batch's skip report.
- **Uncertainty, stated.** (a) The 1.00× closure is measured on ONE fixture
  (Pocket, Ø6.35 flat, hard maple); what generalises is that both surfaces
  now call one function, not the multiplier. (b) The controller tests drive
  production handlers but do not render egui — the two screenshots cover the
  rendering, but only for the two states MCP can reach (see NOT EXERCISED).
  (c) `resolve_operation_invariants` is passed `SuggestContext::default()`
  from O2, so its chipload recalibration short-circuits by design; whether
  O2 should ALSO get a band is a live question I did not rule on, and it is
  A-8's territory.

### Red-first evidence, and where it fell short of the bar

Run against the fix, before inverting anything. **Six of the nine moved;
three did not.** Both facts are quoted.

Compile-red (4 tests) — `cargo test -p rs_cam_viz --test apply_contract_a3`:

```
error[E0432]: unresolved import `rs_cam_viz::ui::FeedsField`
  --> crates/rs_cam_viz/tests/apply_contract_a3.rs:42:32
error[E0599]: no variant named `ApplyFeedsField` found for enum `AppEvent`
   --> crates/rs_cam_viz/tests/apply_contract_a3.rs:242:52   (and :305, :415, :425, :435, :482)
error: could not compile `rs_cam_viz` (test "apply_contract_a3") due to 7 previous errors
```

That is the strongest available form of red for `hazard_a_modal_per_field_
apply_writes_the_refused_recipe`, `hazard_ab_refused_pairing_with_a_geometry_
dial_takes_the_write`, `hazard_c_per_field_apply_writes_the_raw_preview_value`
and `hazard_c_per_field_doc_is_3x_the_funnelled_doc`: the affordance they
dispatch does not exist, so the compiler refuses them.

Behavioural red (2 tests). To get a per-test verdict for the five that still
compile, I built a **temporary** copy of the file with the four above removed
(scratch only, run once, deleted; it is not in any commit):

```
running 5 tests
test hazard_a_panel_refuses_the_pairing_the_modal_previews ... ok
test panel_cut_geometry_apply_goes_through_the_invariant_funnel ... ok
test hazard_a_modal_apply_all_writes_the_refused_recipe ... FAILED
test hazard_b_modal_apply_all_moves_geometry_panel_speeds_apply_does_not ... ok
test project_apply_all_reaches_a_refused_toolpath_silently ... FAILED

---- hazard_a_modal_apply_all_writes_the_refused_recipe stdout ----
assertion `left != right` failed: Apply all did not write feed on a pairing
the panel refuses — hazard (a) may have been fixed; if so this test is the
sentry that should now be inverted
  left: 1000.0
 right: 1000.0

---- project_apply_all_reaches_a_refused_toolpath_silently stdout ----
assertion `left != right` failed: project-wide apply skipped the refused
toolpath — if it now refuses per row, invert this sentry
  left: 1000.0
 right: 1000.0

test result: FAILED. 3 passed; 2 failed
```

**Bar 1 is therefore NOT fully met, and here is why, per test.** The bar's
rationale is "a green run against unchanged tests would mean the fix did not
reach production code". The fix demonstrably reached it — six of nine moved.
The three that stayed green are the three that never asserted a hazard:

| test | why it stayed green |
|---|---|
| `hazard_a_panel_refuses_the_pairing_the_modal_previews` | It asserts the *premise* — the panel refuses AND the preview still produces numbers. Checkpoint I-3 **deliberately preserves both**: the charts must keep drawing. Nothing here could go red without violating the ruling. Inverted by ADDING the missing half (`preview.applicable().is_none()`), which is the guarantee that did not exist before. |
| `hazard_b_modal_apply_all_moves_geometry_panel_speeds_apply_does_not` | It asserts that `⚡ Apply all` moves the cut where the panel's speeds button does not. I-1 kept the combined apply (option C rerouted M7, it did not delete it), so this stays true by ruling. The fix here is *attribution* — a button label and tooltip — which no controller-level test can observe. Inverted by asserting the modal's write is now bit-equal to panel-speeds + panel-cut-geometry, plus a source assertion on the button string. |
| `panel_cut_geometry_apply_goes_through_the_invariant_funnel` | Labelled "the funnelled counterpart, for contrast" in A-3's own source. It was always a statement of correct behaviour, and it is the surface the modal was made to match — so it is the one that must NOT move. Kept verbatim. |

I record this as a bar not fully met rather than reinterpreting the bar. If
the orchestrator reads it as met-in-substance, that is the orchestrator's
call to make, not mine.

**Fingerprints:** none moved, and one is now pinned that was not before.

### Bar-by-bar

| bar | verdict | evidence |
|---|---|---|
| 1. all nine red, then inverted | **PARTIAL — 6/9** | quoted above, per-test reasons in the table. All nine inverted in place per I-6 with every measured pre-fix number preserved in the doc comments; 14 tests now in the file. |
| 2. no recipe number moves; Pocket `3000 / 794 / 18000 / 2.222 / 1.27` byte-identical | **MET** | New sentry `pocket_fixture_recipe_fingerprint_is_unmoved` asserts all five through the validated panel path — green. Independently, core `apply_scope_matches_the_legacy_entry_points_exactly` asserts `apply` with each scope is equal to the legacy entry point it replaces, on feed/plunge/RPM/WOC/DOC. The three legacy functions were not edited. |
| 3. the §3.4 DOC gap closes to 1.00× | **MET** | `surviving_apply_paths_produce_the_funnelled_doc_exactly` compares `to_bits()` of the modal's applied DOC against the panel's `⚡ Apply cut geometry` DOC — **bit-equal**, i.e. exactly 1.000×, not "within tolerance". The 4.445 mm producer no longer exists; what is checkable is that what remains matches, and it does. |
| 4. no background field locking or auto-population | **MET** | Diff-checked: the only `add_enabled` in the wave is the pre-existing `⚡ Apply selected` gate on "any row checked", whose label changed and whose condition did not. No `DragValue`, numeric input or `interactive(…)` call was added, removed or re-gated. The change is *what a button does when clicked*, never whether a field is editable. |

### Verification (focused commands + exact state)

- `cargo test -p rs_cam_viz --test apply_contract_a3` → **14 passed, 0 failed**.
- `cargo test -p rs_cam_core --lib feeds::suggest` → **40 passed** (5 new).
- `cargo test -p rs_cam_core --lib feeds::` → **225 passed**.
- `cargo test -p rs_cam_viz -q` → **244 lib + 14 + 12 + 11 passed, 0 failed**
  (the 12 are `mcp_escape_hatches`, unbroken by the new tool).
- `cargo test -p rs_cam_mcp -q` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → **zero warnings**
  (only the pre-existing future-incompat note for transitive `nom 3.2.1` /
  `quick-xml 0.22.0`). One real hit found and fixed en route: a
  `redundant_clone` in my own new core test.
- `cargo fmt --check --all` → clean, exit 0.
- `cargo test -p rs_cam_core -q --no-fail-fast` → **2930 passed, 6 failed**,
  and **all six are pre-existing, three of them already ledgered**. Attributed
  rather than assumed: I reverted *only* my core diff
  (`git checkout 875816c -- crates/rs_cam_core/src/feeds/suggest.rs`), re-ran
  the two failures that were new to me, and got the **identical** panic on
  both; tree restored afterwards and re-verified.

  | failing test | attribution |
  |---|---|
  | `run_literature_matrix` (cell `flat_3mm_pocket_ipe_extreme`, `anti.ipe_micro_matches_oak_micro_chipload` critical) | **intake G-LIT-IPE**, A-6's, exactly as briefed |
  | `three_pass_full_dressups_fingerprint`, `face_full_chain_fingerprint`, `arc_raster_full_dressups_fingerprint` | **S-4's second intake.** Byte-check: my `three_pass` reads `left (23, 14265253333427783116) / right (23, 14756822782673573601)` — **identical to the values S-4 recorded**, so this red has not moved under my wave |
  | `sub_1mm_tapered_ball_hardwood_finish_extrapolates_with_scaling` (`chipload_diameter_scale` off by >1e-6) | **pre-existing at `875816c`** — reproduced with my core diff reverted. Not previously ledgered as far as I can see; it lives in the `D^0.61` diameter-law territory A-2 moved on 2026-08-06. **Suggest a new intake row.** |
  | `wanaka_suggest_baseline` | **pre-existing at `875816c`** — and the panic names the cause: `Toolpath id 11 missing from suggest cases — wanaka.toml shape changed?`. The play-file has been operator-modified since before this wave started (it is in `git status` at session open and I never opened it). This is a test pinned to a file the operator is allowed to edit; **suggest an intake row**, because it will keep failing until either the pin or the fixture policy changes. |

  Note on the first run I did: plain `cargo test -p rs_cam_core -q` **stops at
  the first failing binary**, so it reported "1 failed" and never reached the
  five later ones. The complete picture required `--no-fail-fast`, and the
  earlier number would have been a false all-clear. Recording it because the
  next agent will otherwise draw the same wrong conclusion.
- Machine discipline: `free -g` + bracketed `pgrep -af "carg[o]"` before every
  launch; a foreign Cargo job (a different repo) held the slot at the start of
  the wave and I waited it out rather than racing it. **No release build** —
  the GUI capture used `cargo build -p rs_cam_viz --bin rs_cam_gui --features
  mcp`, a debug build, per §0.8. Disk 143 G free throughout.
- Git: explicit staging only, five commits, no `--amend`.
  `planning/airrun_2026-06-01/wanaka.toml`, `planning/review_2026-07-27/`, the
  operator's `FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md` and the
  `e_impl/*.pgm` strays remain untracked/unstaged and byte-unchanged by me.
  The live GUI run used a **scratch copy** of a `test_data` fixture; wanaka was
  never opened.

### NOT FIXED / NOT EXERCISED, STATED — owner and re-open condition

- **NOT EXERCISED: the explore-chart Apply, on screen, in either state.**
  `✓ Apply explored values` only renders after `⊕ Start exploring` is
  clicked, and MCP cannot inject a click inside a modal (`set_ui_view` opens
  modals, not widgets). Owner: whichever wave next has an operator at the
  keyboard. Covered meanwhile by two controller sentries.
- **NOT EXERCISED: the project-rollup tab on screen**, where a refused row
  shows `⚠` and `refused` in place of its Apply. Its tab switch is
  `AppEvent::SetFeedsModalMode`, which no MCP tool emits. Same owner; covered
  by `project_apply_all_skips_a_refused_toolpath_and_reports_it`.
- **NOT EXERCISED by an automated test: the scope-string parsing in
  `mcp_apply_feeds`.** Two of its four arms (`"both"`, `"speeds"`) were hit
  live; the unknown-scope error arm and `"cut_geometry"` were not. It is a
  private method on the egui `App` and `rs_cam_viz` has no App-level harness.
  Owner: Lane B if it ever builds one. The `ApplyScope` values behind the
  strings are sentried at the controller.
- **NOT FIXED, deliberately: O1 and O3.** Excluded by I-4 with the reason
  recorded at the funnel. Re-open condition: evidence that an optimizer
  candidate can reach an operation without having been simulated.
- **NOT FIXED: the legacy triple "apply all LUT feeds" duplication**
  (`ui_audit/DIAGNOSIS.md` §36 — feeds-card "⚡ Suggest all", Params-tab
  "⚡ Suggest all (LUT)", modal "⚡ Apply all"). All three go through
  `apply_feeds_result_to_op`, so all three are funnelled and none carries
  hazard (c); but the *validation* half of Checkpoint I reached only the modal
  and the panel. The two `properties/mod.rs` Suggest-all buttons were outside
  the census's 13 and outside I's scope, and I did not widen it. Suggest a
  ledger row: they are the remaining unvalidated apply affordances in the GUI.
- **Observation for the orchestrator, not a defect:** two frozen
  `planning/ui_audit/surfaces/*.md` files listed the deleted per-field
  controls as current capability. I added dated superseded-notes rather than
  rewriting the snapshots. There may be others in that directory that I did
  not audit.

Next action / checkpoint request: **none — A-4 needs no checkpoint.**
Orchestrator actions: (a) mark A-4 COMPLETE, noting bar 1 as 6/9 with reasons
rather than as met; (b) DoD item 2 ("one validated application funnel; the
modal cannot apply what the panel refuses, and cannot change cut geometry
under a speeds label, sentried") is dischargeable on this wave's evidence —
both halves are sentried and both are rendered; (c) take the new ledger
candidate above (the two `properties/mod.rs` Suggest-all buttons); (d) note
for A-8 that O2 now runs a clamp pass with `SuggestContext::default()`, so
whether it should also carry a chipload band is an open question in its
territory; (e) the plan's §5 sequencing note that A-4 and B-4 share
`feeds_modal.rs` still stands — B-4 will land on a file this wave changed
substantially.
