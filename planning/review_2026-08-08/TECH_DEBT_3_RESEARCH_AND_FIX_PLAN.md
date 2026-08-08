# Tech-debt programme 3 — feeds correctness + agent-facing robustness

Date: 2026-08-08
Seed briefs:
- `planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`
  (the operator's own parallel review, revision `d820226` — its four [high]
  findings and six-step sequence are the spine of Lane A and are treated as
  reviewed evidence, not hypotheses to re-derive)
- `planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4 (the deferral
  ledger; every item this plan schedules is cited by its row id)
- the W10-LV live-validation entry in
  `planning/review_2026-08-04/ORCHESTRATION_LOG.md` (G-LV.2 / G-CHIP-ULP /
  G-BYTE provenance)

Base revision: `master @ 53b1c72` (the TD1+TD2 merge). Work on a branch off
master (suggested: `tech-debt-3`); the experiment branch is merged and done.

## 0. Operating rules (carried from TD2, binding on every wave)

1. **Research first.** No behavioural change without a wave that measured the
   current behaviour and pre-registered its bars. A red-first sentry precedes
   every fix; the pre-fix reproduction stays in the file permanently.
2. **A fingerprint that moves is a STOP**, not a re-pin. Re-pins carry
   old/new, mechanism, exact build, and a consumer census.
3. **Render before verdict.** Any claim about a visible surface (heat-map,
   modal, viewport) ships with a screenshot; any claim about surface quality
   ships with a rendered surface. An aggregate without a surface is not
   evidence.
4. **Populations at source intent**; a gate handed an empty population is
   vacuous until its `sample_count`/`sample_range` is checked (X-VAC).
5. **Instrument integrity**: both sides of every ratio are the same measure;
   a changed instrument makes its own docstring a lie you then cite.
6. **Checkpoints are ruled by the operator** via AskUserQuestion. No agent
   self-approves. Rulings recorded in this directory's ORCHESTRATION_LOG.md
   are binding.
7. Agents append §3.1-format log entries; the orchestrator reconciles the
   tracker and never edits an agent's entry.
8. Machine discipline: one Cargo job machine-wide (`free -g` + bracketed
   `pgrep -af "carg[o]"` before every launch, `ps -L` for rayon liveness);
   stop compiling below ~20 GB free disk; no release builds inside waves.
9. Git discipline: explicit staging only; NEVER `git commit --amend` on the
   shared tree; NEVER stage `planning/airrun_2026-06-01/wanaka.toml` (still
   operator-modified, read-only forever) or `planning/review_2026-07-27/`;
   commit trailers: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`
   + the session URL.
10. MCP/GUI sessions: on Wayland the window must be visible for dispatch
    (or launch with `WAYLAND_DISPLAY` unset, forcing XWayland — note
    `WINIT_UNIX_BACKEND=x11` is INERT on winit 0.30, removed in 0.29;
    measured by B-1 2026-08-08, Checkpoint L-6); read `frame_loop` before
    reading an idle lane as completion; never resubmit a generate while one
    is in flight; **never call `get_cut_trace` unfiltered until B-2 lands**
    (G-LV.2 kills the GUI); record the cell beside every collision count and
    never clear a collision across mismatched resolutions.
11. Orchestration: Fable orchestrates and prompts; Opus subagents implement;
    parallel agents work disjoint file territories; one sequential verifier
    runs builds/tests; session-limit and watchdog kills recover via
    SendMessage resume with tree-state verification.

## 1. Scope — two lanes and a sweep pool

**Lane A — feeds/chipload correctness.** The operator review's sequence,
executed in order, plus the two live findings that landed in the same
territory. The review's findings are already source-proven; Lane A's research
waves therefore verify-and-fixture rather than re-discover.

**Lane B — MCP/agent-facing robustness.** The GUI is driven by an agent in
production use; a read-only diagnostic call must never take the process down
(G-LV.2), and request dispatch should not depend on compositor frame
callbacks (G-LV.1 re-open (b), operator-motivated: a repaint-coupled dispatch
"is not ideal for a program with strong AI integration").

**Sweep pool S — cheap mechanical items** that fill idle slots between
checkpoint waits. Each is bounded, none needs a checkpoint unless flagged.

Explicitly OUT of scope (ledgered, untouched): the deep parity cluster
(S-PAR / S-DZ / D161-Q / REST-24 — its own future programme), the
chip-thickness-policy investigation beyond its research wave (A-6), any
mega-rewrite of the feeds layering (the review's own instruction), B1/B2
strategy re-opening, and everything in `UNTOUCHED_TERRITORY_RISK_MAP.md`'s
intake list except where a Lane B wave touches the same file anyway.

## 2. Lane A — feeds correctness

### A-1 research: heat-map divergence + chipload vocabulary census
Ledger: **F-HEATMAP**; review [high] "viewport chipload graph compares
incompatible units" + [high] "visual vocabulary" (locations
`tool_load/mod.rs:201-223`, `gpu_upload.rs:1058-1082`,
`toolpath_render.rs:720-747`, plus the four vocabulary surfaces).
- Verify the review's cited sites at the current revision; census every
  surface that prints or colours a quantity under "chipload" language and
  classify each as commanded advance / achieved advance / arc-mean thickness.
- Build the review's synthetic two-arc fixture: same feed/rpm/flutes, two
  engagement arcs — chip-thickness display must move, advance-per-tooth
  display must not.
- Live screenshot of the current (wrong) heat-map on a real fixture, before
  any fix (rule 3; the ledger row itself demands it ship with a screenshot).
- Deliverable: `HEATMAP_VOCAB_CENSUS.md` + the fixture + screenshots.
→ **Checkpoint H** (operator): approve the display measure
  (`achieved advance/tooth = effective_feed / (rpm × flutes)` per the
  review), the labelling vocabulary for the three quantities, and whether
  arc-mean thickness keeps a separate visual.

### A-2 impl: heat-map fix + boundary newtypes + screenshot sentry
- Replace the heat-map measure per Checkpoint H; keep chip thickness as a
  distinct visual if so ruled.
- Introduce the review's small newtypes at the gate/graph/UI boundaries only
  (`AdvancePerToothMm`, `ArcMeanChipThicknessMm`, `CommandedFeedMmMin`,
  `AchievedFeedMmMin`, `VendorChiploadBand`) — not a units framework.
- Sentries: the two-arc fixture red-first against the old measure; a
  screenshot check that gate verdict, colour, and displayed units agree.
- Display-only: no recipe number may move; fingerprints are a STOP.

### A-3 research: apply-contract census
Ledger: none (the review's NEW [high] finding — modal vs main panel).
Locations from the review: `suggest.rs:637-664` (validated) vs `:671-689`
(infallible preview), `feeds_modal.rs:381-587`,
`controller/events/mod.rs:780-948`, `properties/mod.rs:1724-1930`.
- Enumerate every write path from a recommendation surface into
  `OperationConfig`: which validate, which can change cut geometry, which
  are speed-only. Table with one row per path.
- Reproduce the two hazards live: the modal applying an invalid pairing the
  panel refuses; the modal's `Apply all` changing cut geometry where the
  panel's speeds-apply does not. Screenshots.
→ **Checkpoint I** (operator): approve the funnel design
  (`FeedsPreview` read-only + `ApplicableRecommendation` post-validation +
  one application API with `ApplyScope`), and rule the short-term question:
  remove modal write affordances vs reroute them to the panel's
  speed-only/cut-only calls. (Review's recommendation: do not preserve the
  legacy all-fields write for muscle memory.)

### A-4 impl: one application funnel
Execute Checkpoint I. Red-first: the invalid-pairing apply and the
geometry-changing `Apply all` both get sentries that fail on the old paths.
NOTE (memory, standing): the Feeds tab never auto-locks numeric fields —
explicit Suggest buttons only; the funnel must not introduce background
field locking.

### A-5 evidence package: arc_fit_ratio_for_op disposition
Ledger: **F-T35** (left standing by two consecutive waves; "should not be
carried as a note a third time"). Review [high]: ratios 0.25/0.15 recalibrate
Adaptive3d/DropCutter feeds toward a metric the gate no longer reports;
retirement moves solved feeds 4×/6.7×.
- The review's four-step package, verbatim: (1) capture current Suggest →
  generate → simulate outcomes on ≥2 fixtures per affected family; (2)
  measure commanded feed, achieved feed, gate observation, and post-sim
  modulation separately; (3) present the three dispositions — retire
  automatic feed-up / demote to a clearly-marked estimated mode / move the
  action into the simulation-backed optimizer — with measured before/after
  recommendation numbers for each; (4) red-first the approved change.
- Until ruled, the UI labels the result a legacy pre-simulation estimate
  (small, separately committable wording change — allowed pre-checkpoint as
  report-tier).
→ **Checkpoint J** (operator): choose the disposition. This is the largest
  number-move in the programme (4×/6.7×) and gets its own ruling.

### A-6 research: LUT selection delta + boundary contract + G-CHIP-ULP
Ledger: **F-LUT2** (structural first, number-moving second), **G-CHIP-ULP**.
- Measure the row-selection delta between `find_best_row_for_geometry` and
  `find_best_chip_envelope_row` across the shipped LUT × the operation
  families (structural census, number-preserving).
- G-CHIP-ULP fixture (trivial per the ledger: any band whose derated max
  < 0.025): pin the floor==ceiling collision, the 1-ulp `Exceeds`, the
  missing `chipload_max` binding-constraint accounting, and the cross-gate
  boundary-semantics disagreement (drill lower bound inclusive, chipload
  upper bound exclusive-at-equality).
→ **Checkpoint K** (operator): (a) unify the LUT resolver or keep two with
  declared purposes; (b) the boundary contract — inclusive-with-epsilon at
  both ends, stated on `ChipBounds`; (c) the floor-clamped case reports
  *clamped*, not *exceeds*, on the verdict surface (the hints already say
  clamped); (d) account the floor clamp as a binding constraint in
  `modulation_summary`.

### A-7 impl: execute Checkpoint K.

### A-8 optimizer assumptions (review [medium] + ledger **F-OPT**)
- Stamp simulation assumptions (modulation state, kinematics model, cell)
  onto optimizer results; build the first real retarget/reconciliation
  fixture measuring an actual retarget outcome moving. Research-first if the
  fixture contradicts the review's reading.

### A-9 research-only: chip-thickness policy investigation
Ledger: **F-BIPOLAR**, **F-VALID**, **F-MISSAE**, plus axial-DOC floors and
the gate population predicate (review's step 6: "must not be folded into a
cosmetic graph change"). Deliverable is an evidence document + checkpoint
questions ONLY — implementation belongs to a future programme unless the
operator rules otherwise at its checkpoint.

## 3. Lane B — MCP/agent robustness

### B-1 research: G-LV.2 crash capture + read-size census
Ledger: **G-LV.2**, **C25** (this crash IS C25's re-open condition).
- Reproduce the crash under a debugger/instrumented build (two-step repro:
  full-project 0.1 mm sim on wanaka-scale project → unfiltered
  `get_cut_trace`); capture the actual cause — the OOM-by-serialization
  hypothesis is unverified and must not be assumed.
- Census the response-size distribution of every parameterised MCP read on a
  wanaka-scale trace (`get_cut_trace`, `get_toolpath_diagnostics`,
  `inspect_spans`, `get_generation_debug_trace`).
- Use a scratch COPY of wanaka (§0 rule 9) or the reference fixture — NOT
  the live play-file.
→ **Checkpoint L** (operator): approve the bounded-read design — caps +
  continuation token (C25's shape), which calls get them, and the default
  page sizes.

### B-2 impl: bounded reads
Execute Checkpoint L. Also in this territory (same files, cheap): fix the
`toolpath_id` filter doc/behaviour mismatch (matches by id, doc says index —
decide which and say so in both), and a sentry that an unfiltered call on a
large trace answers bounded instead of dying.

### B-3 research: dispatch decoupling design
Ledger: **G-LV.1** re-open (b). `Controller` is main-thread-owned and not
`Send` — the wave must produce a design, not assume one: candidate shapes
are a dedicated dispatch source integrated with the event loop
(`EventLoopProxy` user events, which winit delivers without frame
callbacks), a timer-driven drain, or partial extraction of MCP-readable
state. Must state what happens to screenshots and UI-navigation calls that
genuinely need a frame.
→ **Checkpoint M** (operator): approve the architecture. This is the
  programme's one architectural change; if the design is unconvincing, the
  documented x11 remedy stands and the row stays open — say so rather than
  forcing it.

### B-4 impl: execute Checkpoint M (if approved)
Plus, opportunistically in the same files: the six GUI-only frame-coupled
drivers `66c8f16`'s audit named (auto-regen debounce, pending_upload,
lane-idle race — the one W10-LV sighted live — RunSimulationWith re-push,
pending_toolpath_tab, scrub_drag_active), each with its own before/after.
The escape-hatch guarantees (`mcp_escape_hatches`, 12 sentries) must stay
green throughout; fingerprints must not move.

### B-5 GUI-mode results parity
Ledger: **G-RESULTS** — in GUI mode `ProjectSession.results` is never
written, so `get_toolpath_diagnostics` / `get_tool_load_report` run without
generation findings and spans while the CLI's carry both. Research-first:
census what differs on a real project through both surfaces, then wire.

## 4. Sweep pool S (idle-slot fillers, any order)

- **S-1 (X-VAC)**: census every predicate that can empty a gate population
  (`is_phantom_transit`, `is_steady_state_for_gate`, measurability
  abstention, sample-validity) + a `population == 0` marker on the verdict.
  The bar is a population bar, not a verdict bar.
- **S-2 (DR-URL/DR-WS)**: run `/refresh-lit-matrix` on the 10 dead URLs +
  the Whiteside tier question.
- **S-3 (A2D-165)**: run batches of the 165 unrun adversarial matrix cells;
  record per-cell verdicts. No claim of family cleanliness until its cells
  ran.
- **S-4 (G-BYTE)**: the frozen-snapshot A/B — freeze one machined-stock
  snapshot, generate the same op twice, byte-compare. Identical → provenance
  stamp (snapshot id into `ToolpathStats`); different → nondeterminism
  defect row, PROMOTE to a Lane-A-blocking finding and tell the operator.
- **S-5 smalls**: FP-65 doc (65 vs 75), DR-PIN (`apply_drill_defaults`
  clamps `Drill` but not `AlignmentPinDrill`), DR-LIVE (`peck_depth`
  ParamDef range), the feeds-modal negative-chipload hover readout, O-CANC
  (AlignmentPinDrill/Chamfer cancellation — pattern exists from C-impl).
  Each is one commit with its own sentry where behaviour moves.

## 5. Merge order and dependencies

A-1 → H → A-2; A-3 → I → A-4; A-5 → J; A-6 → K → A-7; A-8/A-9 after A-2
(they read the corrected display layer). B-1 → L → B-2; B-3 → M → B-4; B-5
independent. Lanes A and B are file-disjoint except
`crates/rs_cam_viz/src/ui/feeds_modal.rs` (A-4) vs B-4's controller files —
sequence those two waves, don't parallelize them. S-pool items slot anywhere
except S-4, which should complete before A-5's fixtures are trusted
(byte-stability underpins the before/after capture).

## 6. Definition of done

1. The viewport heat-map, the modal, the panel and the gate all present
   band-comparable quantities with the same vocabulary, proven by the
   two-arc fixture and a screenshot sentry (F-HEATMAP closed WITH the
   screenshot its ledger row demands).
2. One validated application funnel; the modal cannot apply what the panel
   refuses, and cannot change cut geometry under a speeds label (sentried).
3. `arc_fit_ratio_for_op` has an operator-ruled disposition executed with
   published before/after recommendation numbers — not carried as a note a
   fourth time.
4. The LUT selection policy and the chipload boundary contract are each one
   declared thing (or two with declared purposes), and G-CHIP-ULP's
   floor==ceiling case reports *clamped*, never a 1-ulp *exceeds*.
5. No MCP read can take the GUI down; the crash cause is named, not
   hypothesised; bounded reads have sentries.
6. MCP dispatch either no longer depends on window visibility, or the
   decoupling design was presented and the operator chose to keep the x11
   remedy — either way the row moves from "hazard reported" to "decided".
7. Every S-pool item is either done or re-ledgered with a named reason; no
   silent carries.
8. Programme close-out with a live validation of Lane A's visible surfaces
   (screenshots) and Lane B's crash/park behaviours on a wanaka-scale
   project, PASS / NOT EXERCISED / CONCERN / FAIL, nothing unexercised
   reported as a pass.
