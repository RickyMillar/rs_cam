# Third technical-debt programme — close-out

Date: 2026-08-16
Basis: `TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md` (this directory) at `ecd1f60a`.
The factual record is `ORCHESTRATION_LOG.md`, which this document indexes
and does **not** replace.
Branch: `tech-debt-3`, off `master @ 53b1c72`.
Range: `ecd1f60..41c5b1fd` — **117 commits** (118 from master).
Span: **2026-08-08 → 2026-08-16**, nine days.
Waves executed: **25** (Lane A 11, Lane B 6, N-2, sweep pool 5, plus the
two intake-driven waves GXFP and QN-c). One wave — **N-3** — is agreed,
scheduled into this close-out, and **has not run**.
Checkpoints: **10**, every one ruled by the operator via AskUserQuestion —
H, I, J, K, L, M, N, O, P, Q-NARROW. No agent self-approved one.

> **What this document is.** A per-condition verdict against the plan's §6
> Definition of Done, a single place where everything still open is
> written down with an owner, and the operational lessons the next
> programme's §0 should inherit.
>
> **What it is not.** It is not a restatement of any wave's evidence.
> Every claim below cites the log entry, the evidence document or the
> sentry that carries it. It does not edit the tracker, the plan, or any
> agent's §3.1 entry (plan §0 rule 7).
>
> **This document does not close the programme.** DoD condition 8 requires
> a live validation that has not happened. Closure is the operator's call
> after N-3.

---

## 0. Headline

**The core red set is `wanaka_suggest_baseline`, alone.** It was six when
A-4 first enumerated it honestly (`2930 passed / 6 failed`) and one at
S-1's close (`--no-fail-fast`, 44 min, one red). Four of the five that
went away were **pre-existing at master** and were closed on mechanism,
not by re-baselining:

| red | disposition |
|---|---|
| `run_literature_matrix` (G-LIT-IPE) | Checkpoint K-(e2) made the anti-test relative (A-7); K-(e1) re-attributed the band and **exonerated the engine** (S-2) |
| `sub_1mm_tapered_ball_…_with_scaling` (G-SUB1MM) | stale expected value, re-pointed at `chipload_diameter_ratio_raw` per K-(f1) (A-7) |
| `transform_provenance_fingerprints` ×3 (G-XFP) | re-pinned on a **named mechanism** — `268e427`, W8/F23-impl, RULED 2026-08-06 — with the §0.2 record, boundary proved in both directions, consumer census test-only (GXFP wave) |
| `wanaka_suggest_baseline` | **still red.** See below |

**The surviving red has a two-part diagnosis and neither part is
"environmental".** G-XFP's honesty note settled it: the *working-tree*
red is file drift (`Toolpath id 11 missing from suggest cases` — the
operator's play-file is edited and read-only to every wave), but the red
**also reproduces on the COMMITTED `wanaka.toml` in a clean worktree**,
where it fails a numeric assertion instead: `Back Rough (tp 4):
envelope-clamped DPP must land near 5.4 mm (regression baseline), got
4.199999999999999` — `AxialDocClampedByEnvelope { commanded_mm: 9.0,
clamped_mm: 4.199…, binding: "vendor_ap" }`. That is a **live baseline
disagreement**, most likely a stale expectation after the Checkpoint J and
K feeds changes, and it is ledgered as **G-WANAKA-DPP** (§2.2).

Three results deserve to outlive the programme because they are about
*method*:

1. **A fixture can hold the defect it is supposed to prove absent.** Every
   one of A-8i's five P-(1a) re-pins removed a test fixture's ability to
   express the two-instrument divergence — a band stated on the
   retargeter *and* a different band stated on the verdict it was fed.
   The divergence was the defect, and it was living inside the fixtures
   written to prove the behaviour.
2. **A green result at the one place your hypothesis forbids it is the
   tell.** GXFP's shared-`CARGO_TARGET_DIR` worktree served a cached
   binary and reported 3/3 green at the mechanism commit — the exact
   opposite of the truth. Nothing else in the run looked wrong.
3. **The unrun portion is where the signal was.** S-3 ran the 165
   never-run adversarial cells in **112 s**. All **19** contained library
   panics it found are in those 165; W4's completed 33 contained zero.
   The cost that justified the twelve-day deferral was never the cost.

---

## 1. Definition of done — plan §6, condition by condition

Eight conditions. **4 MET · 3 PARTIAL · 1 UNMET.**

| # | condition (plan §6) | verdict | evidence and what is missing |
|---|---|---|---|
| 1 | heat-map, modal, panel and gate all present band-comparable quantities in one vocabulary, proven by the two-arc fixture **and a screenshot sentry** (F-HEATMAP closed WITH the screenshot its ledger row demands) | **PARTIAL** | **The measure and the vocabulary are done.** A-1 censused 30 surfaces (4 defective / 17 mislabelled / 10 clean; core, MCP and CLI clean on quantity); Checkpoint H ruled H1–H4; A-2 executed all four in `69ec3cf` `6754a0b` `709d6c9` `12de0a8` `7aa0be0`. V1–V4 now read one expression — `chipload::achieved_feed_per_tooth_mm` **delegates to** `display::achieved_advance_per_tooth`, so colour-agrees-with-verdict is a property of the call graph, asserted bit-equal. The five colour classes moved into `VendorChiploadBand::classify`; no threshold and no RGB triple moved. Red run genuine against production code, quoted verbatim in the log. **What is missing:** (a) the before/after pair is from **A-2's own builds** (`825524f` / `7aa0be0`) — the **operator-instance capture** is explicitly routed to **N-3** by Checkpoint O; (b) the "screenshot sentry" is honestly split — automated is `colour_classes_are_monotonic_across_the_band` plus the fixture's gate/display bit-equality; **nothing automatically compares a PNG**; (c) the viewport heat-map still has **no legend** (Checkpoint H did not rule on it); (d) S-1 opened **G-AXDOC-TIP** on this very surface — an emptied population silently drops `axial_doc` to 0.0 and queries the LUT at the **tip** diameter, a band-moving collapse. Also on the record: the defect's **sign is opposite** on the real fixture (wanaka reads HIGH/red, the synthetic arm reads LOW/blue) and **4.497× is a fixture figure that must never be quoted as a wanaka figure** |
| 2 | one validated application funnel; the modal cannot apply what the panel refuses, and cannot change cut geometry under a speeds label (sentried) | **MET** | A-3 censused **20 write paths, 13 GUI affordances** (validated 2, infallible 11, geometry-capable 8, raw writes bypassing the funnel **7**) and found a **third, more severe** hazard the review had not: the per-field `Apply` buttons wrote `explain.recommended.*` directly, so `enforce_invariants` never ran — measured **3.50× on depth of cut** (4.445 mm vs the funnelled 1.27 mm). Checkpoint I ruled option C; A-4 executed all six items (`9026ddb` `2801654` `0321677` `bbd42c7`). The funnel is **structural, not defensive**: `ApplicableRecommendation` has a private field and no public constructor, so `FeedsPreview::applicable()` is the only source and it returns `None` exactly when the validator refused; `CompareRow::show` no longer takes an event sink at all. Bars 2/3/4 MET — recipe fingerprint `3000 / 794 / 18000 / 2.222 / 1.27` byte-identical, the §3.4 DOC gap closes to **1.000× by `to_bits()` equality**, no background field locking (diff-checked). Bar 1 reported honestly at **6/9 red** with a per-test reason; the three greens assert behaviour Checkpoint I-3 deliberately preserves. Both states **rendered live** (`artifacts/a4/`), and `apply_feeds` (I-5) exercised live over MCP. **Residuals, named not hidden:** the explore-chart Apply and the project-rollup tab are NOT EXERCISED on screen (MCP cannot inject a click inside a modal); the two `properties/mod.rs` "Suggest all (LUT)" buttons were outside the census's 13 and remain the last unvalidated apply affordances |
| 3 | `arc_fit_ratio_for_op` has an operator-ruled disposition **executed with published before/after recommendation numbers** — not carried as a note a fourth time | **MET** | A-5's evidence package measured what the ledger only asserted: **Suggest's own shipped recommendation, simulated, trips the chipload gate 4/4 at 1.85×–5.00× band max on the default path**. The 4×/6.7× reproduces the closed form exactly, but the *realised* move is 1.88×–6.67× because the machine cutting-feed ceiling truncates the lift — **so the machine ceiling was doing the safety work**. Checkpoint J ruled (a) now, (c) as destination, plus the J-3 modulation default flip. A-5i executed both as separate commits (`25449085`, `f6533a19`) with the full published table: A3D-1 6912→**1215**, A3D-2 6000→**2243**, DC-1 4405→**1487**, DC-2 2500→**1029**, wanaka Back Rough and 3D Rough 6 both **6000→911**; `Exceeds` 4/4 → `Within` 4/4; **zero divergence from the evidence package's prediction**, to the digit. Eleven re-pins, each attributed to J-1 or J-3. The pre-fix reproduction **survived the fix** — arm A is now a pinned constant, not a value read back out of the code under test. **Residuals:** the two DropCutter DOC-derate residuals (F-3/C-2/C-5) are **ABSORBED** by default-on modulation, not gone (J-4 ruled them a separate row); `SuggestAggressiveness` is now **inert on the pre-simulation feed path** — a real loss of operator control, and A-8 confirmed the dial appears **nowhere** in `optimize/**` either; the post-J feeds-modal screenshot (rule 3) is owed and joins the N-3 queue |
| 4 | the LUT selection policy and the chipload boundary contract are each **one declared thing** (or two with declared purposes), and G-CHIP-ULP's floor==ceiling case reports *clamped*, never a 1-ulp *exceeds* | **MET** | A-6 swept **18 144 queries + 3 024 reroute pairs** and found **F-LUT2 named the wrong axis**: the entry-point pair diverges 141/18 144 (0.78 %), produces two different *bands* **zero** times, and every divergence is one class; the consequential divergence is the gate-side **family reroute** — 489/3 024 pairs to different rows plus **378** refusal asymmetries. A-5's 1.273× is re-attributed to it, proven two ways. It also **corrected the ledger against itself**: every shipped gate is `Within` at exact equality; the live `Exceeds` was a ±1-ulp **reconstruction** artefact (Suggest multiplies, the gate divides; 6–8 % of a 1 164-point grid). Checkpoint K ruled seven items; A-7 executed all seven in eight commits (`0dc66e02`→`16786d3b`). Measured: 1-ulp fixture **12 of 181 → 0 of 181** with a genuine 5 %-over feed still tripping **181/181**; reroute divergences **489 → 0**; refusal asymmetry **378 → 0**; the bare-`>` sweep grep-verified across six files with the three surviving hits named and shown not to be bound trips. The floor-clamped case now prints *"CLAMPED there by the engine's own rubbing-floor rule, not exceeded"*. A-7 also self-corrected its own commit message: the constant is 8 ulp only at the bottom of a binade — a relative epsilon is **8–16 ulp** — found by a fixture that probed "+9 ulp must still trip" and failed. **Residuals:** the clamp reaches the gate by **re-identification, not carriage** (a hand-typed feed on the same value is indistinguishable — documented at the function); **no committed fingerprint covers ProjectCurve-on-bull/V-bit Suggest**, so a4's sharper refusal half rests on the census's own in-process before/after; the three moved GUI strings are asserted from source, not seen (rule-3 debt → N-3); **G-IPE-PLUNGE** is a new open row |
| 5 | no MCP read can take the GUI down; **the crash cause is named, not hypothesised**; bounded reads have sentries | **PARTIAL** | **Clause 2 is explicitly not met, and the operator ruled that acceptable.** B-1 could not reproduce the crash: the unfiltered `get_cut_trace` returned **60 517 035 bytes in 1.71 s on a population-identical trace and the process was alive**, twice, to the byte. Both ledgered hypotheses are **falsified** — RSS moved +0.50 GB with MemAvailable never below 20 GB and no signal under `gdb --batch` (OOM dead); the GUI survived all three transport-teardown probes (transport-close dead). The leading remaining candidate — a client-side reap, `rs_cam_gui --mcp` being a direct child of `claude` — is **consistent with every observation and not established**. Checkpoint L-1 ruled: proceed on "bound the read regardless of which mechanism delivered the kill"; **G-LV.2 stays open, cause-not-attributed**. Clauses 1 and 3: B-2 executed L in full (`3a97b8b`, `825524f`, `afd102b`) — five uncapped arrays get the truncation triple, `MAX_RESPONSE_BYTES` charged **while building**, ~181× reduction pinned, unmatched-id **refusal** replacing an empty skeleton that was indistinguishable from a real empty result (values 4, 5 and 6 on wanaka were simultaneously valid indices and valid ids of *different* toolpaths), compact JSON on all ~68 tools. B-2 caught a **C25-shaped defect of its own** pre-land: `sections_not_computed` was unreachable, so a budget-starved array would have shipped as `[]` — "did not fit" wearing the clothes of "there is none". **What is missing:** the after-number is measured on a **synthetic population through the production builder**, not on a live 1.6 M-sample trace; the live release-rig re-run is the one measurement B-2 owes and could not take (§0 rule 8 forbids release builds in-wave) |
| 6 | MCP dispatch either no longer depends on window visibility, **or** the decoupling design was presented and the operator chose the x11 remedy — either way the row moves from "hazard reported" to **decided** | **PARTIAL** | **The "decided" clause is met three times over** (Checkpoints M, N and O, all operator-ruled) and the mechanism is now **measured, not modelled**. B-3's design was right about winit and wrong about what is reached: B-4 proved the main thread is blocked **below winit, inside the Wayland FIFO present** — `/proc/<pid>/syscall` reads `poll()` on **one** fd with an **infinite** timeout, the last `about_to_wait` is 488 ms after the minimise and there is never another, `wakeups 21` with `pumps` static. `wakeups` was added precisely to separate "the ping was never issued" from "the ping was issued and ignored". B-4 stopped at the step-4 gate **on the gate's own terms**; Checkpoint N kept steps 1–4 (12.0 vs 15.6 ms median, and the instrumentation is what diagnosed the block from outside the process) and authorised N-2. N-2's A/B is one variable: AutoVsync parked with both frame-door calls timing out at 15 s and `poll(1 fd, ∞)` **405/405**; **Mailbox on a minimised window has no park at all** — every call live at 1.6–1.9 ms, `epoll_wait` 355/355. Checkpoint O ruled the `--mcp`-only `AutoNoVsync` flip with the negotiated mode **observable**, and B-4b shipped it on the **shipped path**: `generate_all {fixpoint}` `rounds: 2` in 59.8 s on a minimised window, `screenshot_gui` **788 881 bytes in 0.71 s**, step 7 red **120 s hang → green 2.01 s refusal**. The observability was justified twice over — XWayland negotiates the same request to `Immediate` where Wayland gives `Mailbox`. **What is missing, and it is the half that matters:** the incident's actual states — **occluded, unfocused, screen-locked, visible-but-frozen** — are NOT EXERCISED on any wave. One compositor, one lever (minimise). The caveat travels verbatim in `present_mode.rs`, the `--mcp` startup warning, `park_refusal`'s docs and three commit messages: *"fixes the minimise reproduction; incident-state coverage pending N-3"*. **G-LV.1 stays OPEN**, owner N-3. Step 6 (six frame-coupled drivers) is deferred to N-3 and the 100 ms heartbeat therefore **stays**, per M-5's own condition. A counter-datum from A-8i: a freshly built debug `rs_cam_gui --mcp` in an agent session **created its X11 window and never reached a first frame**, three instances, `generation_status` timing out at 60 s |
| 7 | every S-pool item is either done or re-ledgered with a named reason; **no silent carries** | **MET** | **5 of 5 done**, and every one produced ledgered follow-on rather than a carry. **S-1 (X-VAC)**: the class got an *exact* definition — a predicate running AFTER the gate sets its usable-flag — **8 silent milling pairs + 3 drill gates = 11 silent paths, 0 visible before, all visible now**, via `GatePopulation{contributing, offered, unit}` on the existing evidence shape; report-tier (export byte-identical, no verdict flipped); red-first **measured** 5/9 with the pre-fix string quoted verbatim (`Power within budget (0.00/0.00 kW peak)`). Deflection was **worse than filed** — it resolves `Confidence::Validated` on nothing. And `sample_count` — the field an agent would reach for — publishes `valid_count`, which increments *before* the transit skip, so publishing it as the vacuity marker **would have reproduced the defect inside the fix**. **S-2**: 9 dead rows across 6 URLs, **2.25× the ledger's count**, 5 died since the census; DR-WS answered on evidence (zero occurrences of "RPM" in all three Whiteside documents; **not merged**); K-(e1) **exonerated the engine** — the Ipe band's real provenance is a hobby chart carried under vendor citations that contradict it, and the engine's 0.0360 is Freud's own Ø1/8″ band density-scaled, reproducing A-6's measured minimum exactly; 6/6 pre-registered predictions confirmed. **S-3**: 165/165 run in 112 s; the **first verdict was 165/165 `ok` and was not trustworthy** — a probe for contained panics and non-finite coordinates was added **before** judging; final 150 clean / 13 red / 2 declared-skip; W4's 33 rows reproduce identically. **S-4**: verdict **IDENTICAL BYTES** — one SHA256 over six generations across four test functions — so G-BYTE is snapshot drift, not nondeterminism; `StockSnapshotStamp` shipped content-derived (a pointer or event-counter stamp would have called arm B1's honest re-sim a different snapshot). **S-5**: 5/5 fixed, 0 OBE; the cancellation claim was wrong in **both** terms (22 of 24 described as 21 of 23) and is now an assertion against `OperationType::ALL` |
| 8 | programme close-out **with a live validation** of Lane A's visible surfaces (screenshots) and Lane B's crash/park behaviours on a wanaka-scale project, PASS / NOT EXERCISED / CONCERN / FAIL, nothing unexercised reported as a pass | **UNMET** | The close-out record exists (this document). **The live validation has not run.** It is N-3, agreed at Checkpoint N, folded into close-out by Checkpoint O, and never scheduled — the operator has not been at the desktop. What exists instead, stated so it is not mistaken for the condition: Lane A has A-2's before/after pair **from A-2's own builds** on a real wanaka-derived fixture, and A-4's two live modal states from its own debug GUI; five further Lane A surfaces are owed captures (§2.1). Lane B has B-1's **pre-fix** wanaka-scale crash measurement (1 588 883 samples, byte-identical across two runs) and B-4b's shipped-path park measurement on a **two-op trim** of wanaka at 1.0 mm — not a wanaka-scale post-fix run. The single most quotable gap: **B-2's bounded read has never been measured on a live 1.6 M-sample trace.** Nothing above is reported as a pass |

### The honest summary

The programme did what its plan said: it took the operator's own review as
reviewed evidence rather than re-deriving it, and it did not move a
machining number without a ruling. Every one of the four largest number
moves — the arc-fit retirement, the modulation default flip, the a4 LUT
reroute, and the retargeter's derated band — landed red-first with an
old/new table and an operator ruling cited as the mechanism.

The three PARTIAL conditions share one shape and it is not the same shape
as TD2's. TD2's partials were *divergences that became documented but
stayed divergences*. TD3's are **evidence that could not be taken from
inside an agent session**: an operator-instance screenshot (1), a live
release-build byte count (5), a compositor state only a human can produce
(6). The single UNMET condition is the session that would take all three.

That is a real distinction and it is worth stating plainly rather than
softening: **nothing in this programme is PARTIAL because a wave ran out
of effort.** They are PARTIAL because the programme's own rules — no
release builds in a wave, render before verdict — bite hardest exactly
where an agent has no eyes.

---

## 2. What is still open, in one place

### 2.1 N-3 — the agreed, unscheduled, operator-in-the-loop session

Agreed at Checkpoint N-3, folded into close-out by Checkpoint O, extended
by B-4b (step 6), A-8i, QN-c and S-1. It is the single blocker on DoD 8
and on the PARTIAL half of DoD 1 and 6.

**Agenda item 1 — park-state attribution (this is what closes G-LV.1).**
The operator drives window states on cue — occlude, unfocus, screen-lock,
change monitor arrangement, leave visible-but-frozen — while the rig
(`artifacts/n2/present_mode_ab.py` + B-4's park lever + N-2's syscall
sampler) samples callbacks and `/proc/<pid>/syscall` from outside the
process. The question it answers: **do the incident's actual states share
the FIFO present-block mechanism, or does the shipped `--mcp` flip fix a
reproduction and not the incident?** Everything measured so far is one
compositor (GNOME/mutter 46, Wayland, 60 Hz, Radeon 890M/RADV, Mesa
25.2.8) and one lever (minimise). Until this runs, "G-LV.1 is a present
block" is a strong hypothesis fitted to one reproduction — B-4 said so and
did not claim more.

**Agenda item 2 — B-4 step 6: the six frame-coupled GUI drivers.** Each
needs its own human-visible before/after in an `--mcp`-less session, which
is why it could not be a normal wave (a `--mcp`-less session has no MCP
surface, so no agent can drive or observe it). The six: auto-regen
debounce, `pending_upload`, the lane-idle drain race, the
`RunSimulationWith` re-push, `pending_toolpath_tab`, `scrub_drag_active`.
**The 100 ms heartbeat is deleted only alongside them (M-5), so it
stays.** Also unbuilt here: `visible_on_next_frame` on `set_ui_view` /
scrub (M-4's second clause — B-4b declined to ship a field nobody had
watched fail). One of the six has already shown itself and is worse than
catalogued — see **G-REGEN-RACE** below.

**Agenda item 3 — tearing by eye.** Not measurable headlessly and not
claimed: every capture path in this repo reads a completed frame buffer,
which by construction cannot contain a scanout artefact. On this surface
the non-FIFO option is Mailbox (the queued non-tearing mode; `Immediate`
is unsupported and **hard-crashes** inside `Surface::configure`), so the
expectation is no tearing — an expectation from the mode's definition, not
a measurement. Also wanted: perceived smoothness and input latency during
a viewport drag, against N-2's measured cost (idle 5.20 % → 7.03 % of a
core; continuous repaint AutoVsync 59.9 fps at 18.27 % vs Mailbox
143.8 fps at **81.40 %** — 4.46× the CPU for 2.4× the frames on a 60 Hz
panel, i.e. unthrottled, not smoother).

**Agenda item 4 — the rule-3 screenshot queue.** Seven captures are owed.
None may be marked discharged until the PNGs exist and have been Read
back:

| # | capture | owed by | rig / resume condition |
|---|---|---|---|
| 1 | the operator-instance F-HEATMAP before/after | A-2 (Checkpoint H4.2, routed to N-3 by O) | wanaka generated + simulated, viewport colour mode `Chipload`; note `screenshot_toolpath` **cannot** serve this — separate offscreen renderer, fixed palette, ignores `toolpath_color_mode` (proof in `artifacts/a2/`) |
| 2 | A-8i's P-(4) optimizer results card (both stamp blocks, incl. the `Skipped` refusal shape) | A-8i §8 | rig committed: `artifacts/a8i/p4_screenshot.py` + `a8i_pocket.toml` / `a8i_pocket_refusal.toml` (the refusal fixture costs **zero** simulations) |
| 3 | QN-c's refused optimize card — the `NoSafeImprovement` headline and the rollup explanation line | QN-c §8 | verified by test and by reading both renderers only |
| 4 | S-1's two GUI vacuity surfaces — the dimmed `∅` verdict badge + tooltip, and the `∅ <gate>` per-toolpath status flag | S-1 §6 | asserted through the shared `CriterionStatus::{is_vacuous, vacuity_clause}` the renderers call; the pixels are unseen |
| 5 | A-5i's post-J feeds modal (the two chipload-recalibration `RationaleEntry` lines no longer appear at all) | A-5i | — |
| 6 | A-7's three moved GUI strings — the advance-per-tooth card's `(clamped)` + hover, the verdict line's `CLAMPED to band ceiling — not exceeded`, and the two new warning strings | A-6 → A-7 → next (a rule-3 debt now spanning three waves) | a GUI built at or after `b7234d2f`, a sub-Ø2 finishing op, properties panel open |
| 7 | A-4's two unreachable modal states — the explore-chart `✓ Apply explored values`, and the project-rollup tab's refused row | A-4 | needs a human at the keyboard: MCP cannot inject a click inside a modal, and no MCP tool emits `AppEvent::SetFeedsModalMode` |

**Agenda item 5 — two measurements that need a release build.** §0 rule 8
forbids release builds inside a wave, so both are close-out work by
construction: (a) re-run `artifacts/b1/glv2_repro.py` **unchanged** against
a bounded release build and record the new size of call 63 — expectation
low hundreds of kB against the pre-fix 60 517 035 B (B-2's only owed
number); (b) S-4's GUI-worker `stock_snapshot` stamp call site, which
compiles and mirrors the session path but has never run live.

### 2.2 Intake rows — every one from the tracker, with proposed routing

Twelve rows are open. Three more were opened and **discharged** inside the
programme and are recorded here so the tracker's OPEN markers are not
misread: **G-SUB1MM** (closed by A-7, K-(f1)), **G-LIT-IPE** (closed by
A-7's K-(e2) and resolved by S-2's K-(e1)), **G-XFP** (closed by the GXFP
re-pin wave). The tracker rows were not edited (§0 rule 7); this is the
reconciliation.

| row | summary | proposed owner / routing |
|---|---|---|
| **G-WANAKA-DPP** | The programme's sole surviving core red, and **"environmental" is only half the story**. Working-tree red = file drift (`Toolpath id 11 missing from suggest cases`; the operator's play-file is edited and read-only to every wave). Clean-worktree red on the **committed** `wanaka.toml` = a numeric assertion — envelope-clamped DPP `4.199999999999999` against an expected ~5.4 mm, `AxialDocClampedByEnvelope { commanded_mm: 9.0, binding: "vendor_ap" }`. Likely a stale expectation after J and K | **Do this first.** Either re-baseline the expectation citing the ruled changes (§0.2 old/new, mechanism, consumer census), or attribute the number and show it is not theirs. Owner: the feeds lane. It is the one thing standing between the repo and a zero-red core suite |
| **G-REGEN-RACE** | `generate_all` and the GUI's `process_auto_regen` **cancel each other**: `generate_all` returns `"Back Rough: generation cancelled"`, `rounds: 1`, `generated: 0` for work the GUI cancelled on itself. **Reproduces on a never-minimised window** (`nomin/result.json`) — the compositor is ruled out. It is driver 1's site (`controller.rs:230`) meeting B-4's decision to call `process_auto_regen` from `off_frame_pump`, and under Mailbox `about_to_wait` runs far more often than 60 Hz | Lane B, **inside N-3's step 6**. It is not the one-frame-late UI nuisance driver 1 was catalogued as: it reports a *failure* for work nobody asked to cancel |
| **G-CLI-FIXPOINT** | B-5's R-4, and bigger than the defect B-5 fixed: `rs_cam_cli project` generated **3 of 7** enabled toolpaths where the GUI generated 7/7. Core `generate_all` is single-pass; the fixpoint ladder lives viz-side. Also R-1: `ProjectSession.simulation` is never assigned GUI-side while four core convenience methods read it, and nothing enforces the avoidance | **Its own ruled wave** — fixing it moves the output of every existing CLI invocation. Owner: the CLI/session lane |
| **G-EXPL-HIDDEN** | The optimizer per-toolpath modal's `NoSafeImprovement` branch renders `narrative.headline` and **never** `narrative.explanation`, so F2.3's `DeflectionSetupLocked` prescription has been computed and unshown since it landed. QN-c worked around it headline-only where that was honest | A small viz wave. Cheap, and it unblocks the full text of QN-c's refusal reaching the operator |
| **G-AXDOC-TIP** | S-1, recorded not fixed: `chipload_envelopes_for_session` folds `axial_doc` over `is_steady_state_for_gate(s, None)` — the hardest fallback. If that population empties, `axial_doc` silently falls to **0.0** and the LUT is queried at the tool's **tip** diameter — a population collapse that moves a **band**, on F-HEATMAP's own surface | The feeds lane, **with DoD 1's residual**. It is the one open row that can un-do condition 1 without anything looking wrong |
| **G-PECK-CAP** | S-5's DR-LIVE residual: `fed_descents` guards a non-positive or non-finite peck but **not a positive sub-epsilon one** — `depth / peck` into a `Vec`, measured linear to 15 000 descents at safe magnitudes, ~1.5e10 at 1e-9 (deliberately NOT run). The setter now refuses at the `ParamDef` boundary; **capping the emitter is a generation behaviour change**. Pinned by `a_positive_peck_still_has_no_descent_cap` | The drill lane, **ruled wave**. Re-open condition: immediately — it is live through MCP exactly as the ledgered `0` was |
| **G-IPE-PLUNGE** | Checkpoint K-(e): the cell `flat_3mm_pocket_ipe_extreme` commands **0.1167** of the cutting feed where its band is 0.30–0.50 — **−61.1 %**. Explicitly not a chipload question. S-2 added a **negative datum**: the cited `shapeoko_wiki` *does* publish "30% to 40% of the feedrate for woods" verbatim, so it is not explained by a dead citation | The feeds lane. Next step is a **measurement, not a fix**: the plunge-to-feed ratio the engine commands across the matrix's hardwood cells. One cell is not a calibration |
| **DR-SOFT404** + the DR content debt | S-2's new failure class: a dead article can serve **HTTP 200 with zero redirects** (home-page shell), so a status probe passes it — **DR-P3's "opt-in liveness check" is insufficient as specified**; liveness needs a content assertion. Standing beside it: **DR-GWIZ** (12 `feed_per_tooth` + 21 `rpm` bands cite a source whose live successor publishes no diameter table), **DR-DOCRULE** (54 citations on a band the cited PDF does not contain), **DR-WSCITE** (20 bands once justified by a chart that does not exist), **DR-403** | **A matrix-content wave, and it is bigger than a sweep-pool slot.** These are content debt, not link debt: 33 and 54 citations now rest on rows whose own notes say, in the file, that they cannot support what they are cited for |
| **A2D-F1 … A2D-F5** | S-3's five defect candidates, no fixes. **F1**: `offset_library_failures` is `None` = *not measured* on **109 of 196** cells — adaptive, drill, inlay, rest and vcarve never write it, and the slot's own docstring motivates itself with an **inlay** case. All **19** contained panics are in the previously-unrun 165. **F2**: all four opt-in families count `usize::from(failure.is_some())`, so `Collapsed` and `RejectedInput` count under a name that says *library failure* — `is_library_failure()` exists and is unused; it is also a boolean per call, not a count. **F3**: `drill × invalid-nan` emits **6 of 6 non-finite moves**, `Ok`, across the toolpath IR boundary. **F4**: inlay emits **1225 identical cutting moves on two different degenerate inputs** — `MayBeEmpty` licenses an empty result and says nothing about invented cuts. **F5** (population, not a defect): drill takes model centroids, so **17 of 19** hostile cells never reach the geometry — "drill is clean on hostile 2D" is unsupportable | The 2D/offset lane. **F1 and F2 are ONE decision, not two** — extending the channel to five more families without fixing the predicate propagates the wrong measure. F3 and F4 want checkpoint questions (a missing expectation, not a stale one). Importer reachability of F3/F4 is **unassessed, not "low"** |
| **Q-NARROW (d)** — the headroom research row | The 1.20 retarget headroom (`optimize/policy.rs:480,487`) is **repo-authored** — no source this repo has retrieved publishes a multiplicative headroom on a chipload envelope — and it is **unsatisfiable by construction on 86 of 235** two-sided shipped rows (36.6 %), of which **48** are single-point rows where any multiplicative headroom is unsatisfiable. The ratio is scale-invariant, so a raw-row census answers it exactly | The optimizer/feeds decider. **The typed refusal is its census instrument**: count `RefuseReason::ChiploadBandNarrowerThanHeadroom` on a corpus run to size the population empirically. The open question is whether a multiplicative margin is the right instrument for a nominal preset at all, not whether 1.20 is the right number |
| **The vacuous-refuse question** | S-1 §7, raised not taken: **should a gate with an empty population REFUSE rather than pass?** The shape is ready — `UnmodeledReason::PopulationEmpty(GatePopulation)` slots into the existing refusal vocabulary with no other change. Not taken because flipping a vacuous `Within` to `Unmodeled` **moves export gating and badge counts** | A checkpoint at TD4 kickoff. The counter-argument is on record: Checkpoint D ruled abstention for a *metric that could not be measured*, and an empty population is a different fact — the metric was measurable, there was nothing left to measure it on |
| **F-BASE** | Inherited from TD2 §4.3 and re-confirmed by S-1: `planning/toolpath_acceptance/baselines/2026-06-04.csv` carries no vacuity column, and `smoke_baseline_regression_f037` parses the file and tests a synthetic mutation rather than running the pipeline — so nothing goes red. Widening `smoke.rs`'s `chipload_kind` / `deflection_kind` / `power_kind` columns is a **baseline re-pin** requiring §0.2 old/new | Whoever next runs `rs_cam_cli smoke`. Named by S-1 so the gap is not mistaken for coverage |
| **TD4 intake — the chip-thickness checkpoint package** | A-9 delivered `CHIP_THICKNESS_POLICY.md` with **zero `src` diff** and six questions, **Q1/Q2 gating**, to be ruled at TD4 kickoff. Its findings stand on their own: the axial-DOC chipload floor is **`None` on 36 of 36** (row × shipped-default-stepover) combinations with the feed at each row's own band **maximum** — dead at every default, but **not** dead code (it fires at ~0.63 D radial WOC, above every default, and the probe's first run **falsified A-9's own draft claim** of structural unreachability). The floor's docstring monotonicity claim is **false** (unimodal, peak 0.4183 × fz at arc ≈ 1.97 rad — the same peak A-1's fixture independently pinned); it omits the `arc ≥ π` branch its own comment cites (0.6366 × fz canonical vs ≈1.2e-16 in the copy); and the one case it claims to report is the case it reports as unmeasured. F-VALID: **13 silent emptying paths**, of which the measurability detector covers **2**; bullnose is the worst (chip-blind for the entire finishing regime below its corner radius). F-BIPOLAR is a **refusal, not an advisory**, and returns `false` on an empty population — **every F-VALID emptying path reads as an exoneration**. F-MISSAE: 176/252 is right as a row property and wrong as a blast radius — retiring it promotes **114** rows, not 176 | **TD4 kickoff checkpoint.** Q1 (should anything gate on arc-mean chip thickness again, and under what sourcing bar) and Q2 (retire / repair / ledger the axial floor) gate Q3–Q6. Note A-9's warning that **repair is not a safe default**: it turns an inert bound into a live clamp on ball-finishing `depth_per_pass`, still comparing a chip thickness against an advance band unless Q1 is answered first |
| *(cosmetic)* | N-2 incidental, uninvestigated: a parked screenshot shows a toolpath marked `OFF` in the operations tree reported as **generating** in the status bar | Next sweep pool. It was routed to S-5 and S-5's five slots were full |
| *(flake)* | S-1 recorded, not attributed and not dismissed: `compute::worker::tests::simulation_metrics_capture_emits_cut_trace_and_artifact` failed once on a first pass, then passed in isolation and on two clean full re-runs. Cut-trace artifact emission — no contact with gate populations | Whoever next sees it. Recorded so a second sighting is a pattern rather than a surprise |

### 2.3 Deferred by ruling — decided, not forgotten

These are **not** open questions. Each was put to the operator and ruled;
they are listed so nobody re-opens one as if it had been missed.

| item | ruling | consequence |
|---|---|---|
| the 100 ms GUI heartbeat | **M-5**: deleted only alongside the six-driver fixes | Step 6 deferred to N-3 ⇒ **the heartbeat stays** |
| continuation tokens on bounded MCP reads | **L-2**: DEFERRED as unmotivated by the census | If an agent genuinely needs all 33k span summaries of one operation, the answer today is `span_kind` / `pass_index` narrowing, and the response says so |
| optimizer write paths O1 and O3 | **I-4**: excluded, with the reason written **at the funnel** | Their candidates are scored against a simulated cut trace end to end; re-clamping a sim-verified operating point against a pre-simulation estimator would substitute the weaker evidence for the stronger |
| the optimizer's `adaptive_feed_modulation` pin | **P-3**: KEEPS `false`, **disclosed** | The default flip refuted the *comment*, not the argument — modulation pre-clamps 100 % of moves onto the band max, destroying the search's headroom signal. `diverges_from_library_default_modulation()` makes it wire-visible and the sentry asserts **both** halves, so a future ruling fails loudly |
| the un-lifted feed's resting place | **J-2**: no preference expressed; the derated band **minimum** stands as the default consequence, marked **REVISITABLE** | A-5i surfaced no reason to prefer a different one; the measured 0.999× band-minimum landing held on both Adaptive3d fixtures after the retirement |
| the two DropCutter DOC-derate residuals (F-3 / C-2 / C-5) | **J-4**: a SEPARATE row, not a blocker | They are **ABSORBED** by default-on modulation at the gate, **not fixed**. `Within` 4/4 does not mean the denominator divergence is gone |
| Q-NARROW option (b), "clamp the target into the band" | **explicitly rejected** as the invent-a-number failure mode | (c) shipped: a typed refusal carrying both computed headroom targets and nothing invented. (d) is ledgered as research (§2.2) |
| the unsupported-explicit-present-mode startup crash | **O-1 rider**: LEFT, documented not guarded | The shipped path only ever requests `Auto*` modes, so it is unreachable from production; it is an `.expect`-shaped path inside wgpu that this workspace's own lint policy would not permit |
| the drill thresholds R-8 / R-9 / R-10 | Checkpoint D (TD2): correct the citation, **keep the number** | Untouched by TD3. Still declared repo-authored in code and `CREDITS.md` |
| A-9's implementation | plan §2 A-9: research wave only, implementation belongs to a future programme unless the operator rules otherwise at its checkpoint | **TD4 kickoff**, §2.2 |
| whether the Adaptive3d → Pocket family reroute is *right* | **NOT re-opened** by A-6 or A-7 | a4 applies the G16 §10 judgement to **both** sides; it does not adjudicate whether Adaptive3d should be judged against a Pocket envelope |
| whether a hobby-machine matrix cell should be judged against a hobby chart or a vendor chart | **NOT re-opened** by S-2 | Matrix policy across many cells, not a source question |

---

## 3. Operational lessons — for the next programme's §0

Nine rules, each earned. Every one has a measured incident behind it and
the citation is given so the next agent can read the incident rather than
take the rule on faith.

**L1 — A stale editor diagnostic is not a compile error. `cargo check` is
the only witness.** Six times this programme an agent was handed, or
observed, IDE-reported errors that a build disproved. The clearest is
N-2's, which spent its first action settling it: diagnostics claimed
`host::MCP_WAKE_PASS_NR` missing at `lib.rs:97` and a missing `wakeups`
field at `mcp_bridge.rs:163`; a pristine worktree build at the same
revision reported `Finished dev profile in 52.70s`, **zero errors**, and
both symbols resolve at HEAD. **Never act on a diagnostic you have not
reproduced with a compiler**, and never let one justify editing another
lane's file.

**L2 — A shared `CARGO_TARGET_DIR` across worktrees serves CACHED
binaries, and the failure is silent and inverted.** GXFP built two
detached worktrees at `23f98fc` and `268e427` and cargo hashed both test
binaries identically to HEAD's
(`transform_provenance_fingerprints-ecdd32e8a46e9528` at all three), so the
second worktree reported the first's result: **3/3 green at the mechanism
commit, the exact opposite of the truth**. It was caught for one reason —
green there was the one result the hypothesis forbade. **Isolated target
dirs per commit.** The dependency-cache saving is not worth an inverted
verdict, and nothing else in the run looks wrong when this happens.

**L3 — `pgrep -f` matches the PEER shell, not just itself.** S-3 §9: two
agents' Cargo wait-loops deadlocked on each other, because `pgrep -f
"carg[o]"` matches the *other agent's shell* whose cmdline contains the
string `cargo test`. Worse, S-1's loop was additionally **self**-matching:
`pgrep -f "carg[o] "` matched its own zsh wrapper, whose cmdline embeds
`eval 'until … cargo test …'`, so it could never exit on its own.
**Bracketing a character defeats pgrep's self-exclusion only for the pgrep
process, not for the shell that spawned it.** The repo's existing
"pgrep self-match trap" note needs the peer case added. Use `pgrep -x
cargo` (S-1's mid-wave fix), or gate on cargo's build-directory lock.

**L4 — Wait for a background suite inside the turn.** A full
`cargo test -p rs_cam_core --no-fail-fast` is **40–44 minutes** on this
machine (S-1 measured 44; QN-c waited ~40 on the slot; A-7 lost ~50
minutes to one slot violation). A wave that launches a suite and then
ends its turn holds the machine-wide slot while doing nothing, and the
next agent's `pgrep` sees a build that nobody is watching. Poll inside the
turn, and say in the entry how long you waited.

**L5 — A concurrency brief must be accurate, and the cost of a wrong one
is attribution, not throughput.** A-8i's brief stated "No other agent is
active". **S-5 was executing in the same working tree on the same
branch.** A-8i detected it at 03:54 from eight file mtimes it had not
written plus a `DR-PIN, 2026-08-14` docstring, **halted before any code
commit**, and recorded the evidence in `artifacts/a8i/HALT.md`. Its
first before-capture was already destroyed — a foreign cargo rebuilt
`rs_cam_core-919a16a4876a4f2d` at 03:50:27 **mid-run**, so binaries
13..170 no longer ran the revision under test; it is kept as
`core_suite_before_ABORTED.txt` and **used for nothing**. Cost ≈ one hour
of measurement thrown away. The halt was correct: a red could have been
S-5's and a green could have depended on S-5's. **Corollary, learned four
separate times (A-3 on S-4's mid-flight 4th parameter, N-2 on A-5i's
mid-Checkpoint-J deletions, S-5 on A-8i's transient `E0063`, GXFP on
QN-c's non-compiling `optimize/**`): a red build in a shared tree must be
ATTRIBUTED before it is acted on** — and the right response is a detached
worktree with its own target dir, never a stash or a revert of someone
else's work.

**L6 — Five distinct pipeline false-green traps, all measured here.**
Each one produced a clean-looking result that was clean because the
evidence was truncated:

- **`| grep … | head -40` returns *grep's* exit status, not cargo's.**
  B-2's first core run reported "40 binaries, 2 430 passed, 0 failed" and
  exited 0, on a crate with ~160 test binaries.
- **Plain `cargo test -q` stops at the first failing binary.** A-4's first
  run reported "1 failed" and never reached the five later ones;
  `--no-fail-fast` is mandatory for any red-set claim.
- **`| tail -N > file` silently drops the middle.** A-5i's capture came
  back exactly 400 lines, starting mid-stream, having dropped ~13 test
  binaries **including the one carrying the red it was attributing**. The
  truncated file is retained in `artifacts/a5i/` deliberately, as the
  record of the defect. Every later wave captured unbounded.
- **A bare `param_sweep` invocation reports "56 ignored" and proves
  nothing.** Fingerprint verification needs `-- --ignored` (B-5).
- **The same trap already cost the previous programme.** TD2's closing
  green over the core tests hit the first-failing-binary trap, which is
  why `transform_provenance_fingerprints` sat red for **eight days,
  unnoticed** — stale, not unexplained (GXFP §2).

  Two runtime-input cousins worth the same paragraph: **`cells.toml` and
  `sources.toml` are read at RUNTIME**, so a "before" capture taken while
  the file is already edited is contaminated (A-7 found this and
  re-derived cleanly; S-2 pre-registered around it). And **`cp` is aliased
  to `cp -i` in this shell** — S-2's first restore prompted, silently did
  not overwrite, and its first "after" run was actually a second "before".
  Caught by diffing the two captures. *A capture that looks plausible is
  not the same as a capture that happened.*

**L7 — `.mcp.json` launches the release binary directly, and the
stale-binary risk is now silent.** Two waves lost their rule-3 evidence to
it: A-2 could only reach an instance running `d820226-dirty`, and A-7
could not capture its three moved GUI strings because the live session ran
an `e66962a`-era binary that did not contain the code. **Rebuild the
release binary between waves, and never trust its mtime** — verify with
`git diff --name-only <build-rev>..HEAD`, as B-1 did before running the
existing binary. The standing tension the next §0 should resolve
explicitly: **rule 8 forbids release builds inside a wave**, which is
correct, but it means every claim needing a release binary is
undischargeable until a close-out lane that *is* permitted to build one.
Give that lane a name in the plan.

**L8 — Do not queue a Cargo job to fire when the slot frees while another
agent is active.** S-3 §9: a tier run was queued on slot-free and S-1
relaunched its suite into the same moment, so two Cargo jobs ran
concurrently for ~20 s against the one-job rule. Headroom was checked
while it happened (32 GB RAM, 173 GB disk) and nothing was harmed — but
**the queue-on-free pattern is what caused it**. Take the slot
deliberately or wait; do not arm a trigger.

**L9 — Sequence the platform lane ahead of any lane whose evidence rules
depend on a GUI.** The plan treated Lanes A and B as independent except
for one file. In practice **Lane B's dispatch hazard blocked a Lane A
rule-3 obligation in two consecutive waves** (A-1 spun ~45 min against a
parked loop, `frames: 87899` unchanged across four polls; A-2 hit the same
block three days later), and the debt has now spanned three waves on the
A-6 → A-7 → N-3 chain. A-1 flagged it as a cross-lane observation on day
one and it was right. If rule 3 is binding, the lane that owns whether a
screenshot is *possible* is a dependency of every lane that owes one.

**Two smaller rules that already exist in the repo and were re-confirmed,
so they should be restated rather than rediscovered:** scope `cargo fmt`
to your own crate or files, never workspace-wide, while another lane holds
a crate — A-2 cascaded into B-2's in-flight `app/mcp.rs` and correctly did
**not** revert it, because reverting would have discarded uncommitted work,
which is the worse error. And prefer `git show HEAD:<path>` over
`git stash` for a read of the committed state: a stash mutates a tree two
other agents are working in (S-2).

---

## 4. Wave index

Twenty-five waves. Status is the tracker's, unedited; this is a locator,
not a re-statement.

| wave | deliverable / evidence document | key commits |
|---|---|---|
| A-1 | `HEATMAP_VOCAB_CENSUS.md` → Checkpoint H | `becf1cb` |
| A-2 | `artifacts/a2/` (4 GUI captures) | `69ec3cf` `6754a0b` `709d6c9` `12de0a8` `7aa0be0` |
| A-3 | `APPLY_CONTRACT_CENSUS.md` → Checkpoint I | 9 characterization tests |
| A-4 | `artifacts/a4/` (2 rendered modal states) | `9026ddb` `2801654` `0321677` `bbd42c7` |
| A-5 | `ARC_FIT_RATIO_EVIDENCE.md` → Checkpoint J | `3e0b247` `206fd5c` |
| A-5i | `artifacts/a5i/` | `2544908` `f6533a1` |
| A-6 | `LUT_BOUNDARY_EVIDENCE.md` → Checkpoint K | `3ac55fc` |
| A-7 | `artifacts/a7/` (7 slices) | `0dc66e0` → `16786d3b` |
| A-8 | `OPTIMIZER_ASSUMPTIONS.md` → Checkpoint P | `f807628` → `edd4dd9` |
| A-8i | `artifacts/a8i/` (+ `HALT.md`, `narrow_band_census`) → Q-NARROW | `98e936ec` → `b3e0405f` |
| A-9 | `CHIP_THICKNESS_POLICY.md` → TD4 intake | `9a2e082` |
| B-1 | `GLV2_CRASH_CAPTURE.md` → Checkpoint L | research only |
| B-2 | bounded reads + filter fix | `3a97b8b` `825524f` `afd102b` |
| B-3 | `DISPATCH_DECOUPLING_DESIGN.md` → Checkpoint M | design only |
| B-4 | `artifacts/b4/` → Checkpoint N | `834c789` `9f08ac2` `43300eb` `897d9bc` |
| B-4b | shipped `--mcp` flip + steps 5, 7 | `79f6873` `5a65653` `6168e2d` |
| B-5 | `RESULTS_PARITY.md` | 5 commits |
| N-2 | `PRESENT_MODE_AB.md` → Checkpoint O | `4789446` `e761c00` |
| S-1 | `XVAC_CENSUS.md` | `91a1dde2` `e70091b5` |
| S-2 | `LIT_MATRIX_REFRESH_S2.md` | `32b0f98` `6408dd5` `43d2f0c` |
| S-3 | `A2D165_RESULTS.md` | `57fcf21` `1e87dd8` `2f1efe2` |
| S-4 | `GBYTE_AB_REPORT.md` | `85f40a1` `d27b7ef` |
| S-5 | the smalls bundle | `e1e4ea2` → `8b2dfb1` |
| GXFP | the §0.2 re-pin record | `1b3835ca` `1d05ede2` |
| QN-c | the typed refusal | `eb0f9c9b` `eadda4a7` |
| **N-3** | **NOT RUN** — §2.1 | — |
