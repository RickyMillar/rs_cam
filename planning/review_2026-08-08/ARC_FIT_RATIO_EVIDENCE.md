# A-5 — `arc_fit_ratio_for_op` disposition: the evidence package

Date: 2026-08-12
Wave: TD3 Lane A, wave A-5
Ledger: **F-T35** (`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4.2)
Seed: `planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`,
the `[high]` finding "Stale arc-fit prediction still changes Suggest
recommendations"
Feeds: **Checkpoint J** (operator) — choose the disposition
Harness: `crates/rs_cam_core/tests/arc_fit_disposition_a5.rs`
Raw runs: `planning/review_2026-08-08/artifacts/a5/`

> **Every number below was measured by this wave.** The review's 4× / 6.7×
> is treated as a claim to reproduce, not a figure to quote. Where this
> wave's measurement agrees with it, it says so; where it does not, the
> measured figure wins and the divergence is explained.

---

## 0. The finding in one paragraph

`feeds::predict::arc_fit_ratio_for_op` carries a per-operation-family table
(Adaptive3d `0.25`, DropCutter `0.15`, tagged `Calibrated`; every other family
a conservative `Default`). Each ratio was fitted against the post-sim chipload
gate's **arc-mean chip thickness** observation. That observation was **deleted
on 2026-08-06** — the gate now reports `effective_feed / (rpm · flutes)`, a
linear advance per tooth. The table did not follow.
`suggest::recalibrate_feed_for_chipload` still solves
`feed = target / arc_fit_ratio × rpm × flutes` and is gated on
`ArcFitRatioSource::Calibrated`, so exactly two families receive an automatic
feed lift sized by `1 / ratio`, aimed at a quantity nothing reports any more.

---

## 1. The mechanism, exactly

Read at HEAD (`tech-debt-3` @ `2973200f`; A-2/A-4 have both touched
`suggest.rs` since the review was written, so the review's line numbers no
longer resolve — these do):

| what | where |
|---|---|
| the ratio table + its own ⚠ STALE block | `crates/rs_cam_core/src/feeds/predict.rs:753-836` |
| the predictor that applies it | `crates/rs_cam_core/src/feeds/predict.rs:633-700` (`predict_observed_chipload_mm`) |
| the closed-form solve (pass 8) | `crates/rs_cam_core/src/feeds/suggest.rs:2303-2400` (`recalibrate_feed_for_chipload`) |
| the `Calibrated` gate that limits it to two families | `crates/rs_cam_core/src/feeds/suggest.rs:2317-2320` |
| the report-tier render | `crates/rs_cam_core/src/feeds/rationale.rs` (`ChiploadTarget`, `ChiploadCapBound`) |

**Consumer census — the blast radius is smaller than it looks.**
`predict_observed_chipload_mm` has exactly two callers:

1. `recalibrate_feed_for_chipload` — **the only number-moving one**.
2. `feeds::profile.rs:204`, which parks the prediction on
   `CutterOpProfile::predictions.observed_chipload`. Measured this wave:
   **nothing renders that field.** `rg` over `crates/rs_cam_viz/src`,
   `crates/rs_cam_cli/src`, `crates/rs_cam_mcp/src` and `mcp_server.rs` /
   `mcp_bridge.rs` returns no hits for `observed_chipload`, `arc_fit_ratio`,
   `ArcFitRatioSource` or `predict_observed`. It is a computed-and-dropped
   field.

So the entire **visible** surface of the arc-fit prediction is the two
`RationaleEntry` strings, and the entire **behavioural** surface is pass 8.
That matters for the dispositions: retiring the table touches one decision
site, not a layer.

---

## 2. Method, and why each number is trustworthy

* **Suggest-side numbers come from the real Suggest path.**
  `ProjectSession::cutter_op_profile` — the same assembly the GUI Suggest
  button, the MCP rationale endpoint and the CLI `--apply-suggest` path use
  (`session/mod.rs:1348`). No hand-computed formulas.
* **Sim-side numbers come from `ToolpathLoadVerdict::feed_explanation`**, the
  stage-labelled record A-2 made canonical: stage 1 commanded advance/tooth,
  stage 2 vendor band, stage 3 achieved/commanded feed ratio, stage 4 the
  gate's own observation. All four carry `ADVANCE_PER_TOOTH`, so every ratio
  in this document compares like with like.
* **Stamp equality (S-4's explicit handoff).** Every arm reads
  `ToolpathStats::stock_snapshot` and the harness asserts all arms consumed
  the **same** `StockSnapshotStamp`. The arms differ in commanded feed only,
  and feed does not move geometry — but that is a claim, and the stamp is
  what makes it checkable rather than assumed. Fixtures are two-op cascades
  (op 0 fresh-stock rough, op 1 the measured op on
  `StockSource::FromRemainingStock`) precisely so the stamp is `Some` and the
  equality is not vacuous. **And the stamp channel was checked for
  responsiveness, not just equality**: on the first fixture the harness
  re-simulates at 0.8 mm and requires the stamp to move. Measured — digest
  `4807225758396607230` (cell 0.4) → `8286006674947908385` (cell 0.8). A
  channel that can only ever return "equal" would have proven nothing.
  (The digest is identical *across* fixtures too, which is expected and not a
  defect: every fixture's op 0 is the same rough on the same stock geometry.)
* **Population check (X-VAC).** The harness asserts the chipload gate's
  `sample_count > 0` before any verdict is read. A gate handed an empty
  population passes and looks healthy.
* **Simulation cell: `0.4 mm`, recorded beside every measurement.**
* **Not the operator's live file.** All fixtures are synthetic
  (`common::meshes::height_field` + `common::session`), per §0 rule 9.

### 2.1 One instrument correction, on the record

The harness's first run was **red on its own arithmetic**, and the cause is
worth carrying: an earlier Suggest pass **rewrites `spindle_rpm` on the
operation before pass 8 runs**. Deriving the reference feed from the RPM the
fixture was *authored* with put DC-1's retirement ratio at `7.04×` against a
closed form of `6.67×`. The harness now reads the RPM off
`suggested_operation` — the value pass 8 actually solved against — and the
closed form reproduces exactly. Two riders follow:

* The authored RPM and the solved-against RPM **differ on three of four
  fixtures** (14 000 → 16 000, 18 000 → 19 000, 20 000 → 19 000). Any hand
  reconstruction of a Suggest feed that uses the operation's authored RPM
  will be wrong by that ratio.
* All arms in §3.2 therefore run at the **solved-against** RPM, so the A/B/C
  delta is attributable to the commanded feed alone.

---

## 3. Steps 1–2 — the measurements

### 3.1 Suggest-side, per fixture (no simulation)

Four fixtures, two per affected family, differing in tool, species and machine
feed ceiling. `feed_B` is `FeedRaisedForChipload::requested_mm_per_min` (what
Suggest had *before* pass 8); `feed_A` is `raised_mm_per_min` (what ships);
`feed_C` is `min(target × rpm × flutes, cutting ceiling)` — the lift the
*current* gate observation would justify.

| fixture | family | ratio | source | feed_B (mm/min) | **feed_A shipped** | feed_C | A/B | **A/C** | cap |
|---|---|---|---|---|---|---|---|---|---|
| A3D-1 Ø6 2F endmill / HardMaple / ceiling lifted | Adaptive3d | 0.25 | Calibrated | 1215.0 | **6912.0** | 1728.0 | 5.69× | **4.00×** | none |
| A3D-2 Ø8 3F endmill / WhiteOak / stock ceiling | Adaptive3d | 0.25 | Calibrated | 2243.0 | **6000.0** | 3189.8 | 2.67× | **1.88×** | MaxFeed |
| DC-1 Ø3 ball / HardMaple / stock ceiling | DropCutter | 0.15 | Calibrated | 1487.0 | **4405.0** | 660.7 | 2.96× | **6.67×** | none |
| DC-2 Ø2-tip tapered ball / WhiteOak / small router | DropCutter | 0.15 | Calibrated | 1029.0 | **2500.0** | 478.4 | 2.43× | **5.23×** | MaxFeed |

Bands, targets and the commanded advance per tooth the lift produces:

| fixture | derated band (mm/tooth) | target | cmd advance/tooth **before** | **after** | after ÷ band max |
|---|---|---|---|---|---|
| A3D-1 | 0.03800 – 0.07000 | 0.05400 | 0.03797 | **0.21600** | **3.09×** |
| A3D-2 | 0.04676 – 0.08614 | 0.06645 | 0.04673 | **0.12500** | **1.45×** |
| DC-1 | 0.01159 – 0.02318 | 0.01739 | 0.03913 | **0.11592** | **5.00×** |
| DC-2 | 0.00839 – 0.01679 | 0.01259 | 0.02708 | **0.06579** | **3.92×** |

Three things this table says that the ledger row did not:

1. **The lift pushes the commanded advance per tooth well outside the vendor
   band it claims to be targeting** — 1.45× to 5.00× the band maximum, on
   every fixture. It is aiming a quantity that is `ratio ×` smaller at the
   *middle* of the band, so the quantity the gate actually reads lands
   `1/ratio ×` above it. The lift is not merely unjustifiable; it is
   directionally wrong against the current gate.
2. **On both Adaptive3d fixtures the pre-lift feed already sat at the derated
   band minimum** (0.03797 vs 0.03800; 0.04673 vs 0.04676 — 0.999× in both
   cases). The calculator had already placed the operating point inside the
   band. Pass 8 then multiplied it out of the band.
3. **On both DropCutter fixtures the pre-lift feed was already above the
   band maximum** (1.69× and 1.61×), before pass 8 touched anything. That is
   *not* attributable to arc-fit — it is the DOC-derate denominator
   divergence already ledgered (`FeedsResult::effective_diameter`'s own doc,
   census F-3 / C-2 / C-5). It is recorded here because a reader comparing
   arm B against the band will otherwise mis-attribute it. See §7 rider.

### 3.2 Sim-side arms — Suggest → generate → simulate

Simulation cell **0.4 mm**. All arms of all fixtures reported the **same**
`StockSnapshotStamp` (`cell 0.4`, digest `4807225758396607230`), so the arms
are comparable. Gate populations 448–10 343 samples — none vacuous.

Every arm runs at the RPM Suggest solved against, so the only thing that
differs between arms is the commanded feed.

| fixture | arm | feed | **verdict** | commanded adv/tooth | achieved× | **gate observed** | gate band | n |
|---|---|---|---|---|---|---|---|---|
| **A3D-1** | A shipped | 6912.0 | **Exceeds** | 0.21600 | — | 0.21600 | 0.032 – 0.055 | 756 |
| | B retire | 1215.0 | Within | 0.03797 | — | 0.03797 | " | " |
| | C re-key 1.0 | 1728.0 | Within | 0.05400 | — | 0.05400 | " | " |
| | A + modulation | 6912.0 | Within | 0.21600 | 0.2463 | **0.05321** | " | " |
| | B + modulation | 1215.0 | Within | 0.03797 | 1.4014 | **0.05321** | " | " |
| **A3D-2** | A shipped | 6000.0 | **Exceeds** | 0.12500 | — | 0.12500 | 0.03938 – 0.06768 | 448 |
| | B retire | 2243.0 | Within | 0.04673 | — | 0.04673 | " | " |
| | C re-key 1.0 | 3189.8 | Within | 0.06645 | — | 0.06645 | " | " |
| | A + modulation | 6000.0 | Within | 0.12500 | 0.3150 | **0.03938** | " | " |
| | B + modulation | 2243.0 | Within | 0.04673 | 0.8427 | **0.03938** | " | " |
| **DC-1** | A shipped | 4405.0 | **Exceeds** | 0.11592 | — | 0.11592 | 0.011592 – 0.023184 | 6729 |
| | B retire | 1487.0 | **Exceeds** | 0.03913 | — | 0.03913 | " | " |
| | C re-key 1.0 | 660.7 | Within | 0.01739 | — | 0.01739 | " | " |
| | A + modulation | 4405.0 | Within | 0.11592 | 0.2000 | **0.02318** | " | " |
| | B + modulation | 1487.0 | Within | 0.03913 | 0.5925 | **0.02318** | " | " |
| **DC-2** | A shipped | 2500.0 | **Exceeds** | 0.06579 | — | 0.06579 | 0.008344 – 0.016687 | 10343 |
| | B retire | 1029.0 | **Exceeds** | 0.02708 | — | 0.02708 | " | " |
| | C re-key 1.0 | 478.4 | Within | 0.01259 | — | 0.01259 | " | " |
| | A + modulation | 2500.0 | Within | 0.06579 | 0.2536 | **0.01669** | " | " |
| | B + modulation | 1029.0 | Within | 0.02708 | 0.6162 | **0.01669** | " | " |

Modulation extent, where it ran:

| fixture | moves touched | median Δfeed, arm A | median Δfeed, arm B |
|---|---|---|---|
| A3D-1 | 261 / 271 | **−78.9 %** | **+20.1 %** |
| A3D-2 | 184 / 212 | −68.5 % | −15.7 % |
| DC-1 | 7395 / 7395 | −80.0 % | −40.8 % |
| DC-2 | 14640 / 14640 | −74.6 % | −38.4 % |

#### The four things this table establishes

**1 — Suggest's own recommendation fails the gate Suggest claims to target,
on every fixture.** Arm A is `Exceeds` 4/4. Its gate observation runs
**1.85× to 5.00× the gate's band maximum**:

| fixture | arm A gate obs ÷ gate band max |
|---|---|
| A3D-1 | 3.93× |
| A3D-2 | 1.85× |
| DC-1 | 5.00× |
| DC-2 | 3.94× |

This is not a modelling disagreement to be argued about. The number the
operator is handed, applied unmodified and simulated, trips the breakage-side
chipload gate. `adaptive_feed_modulation` is **`false` in
`SimulationOptions::default()`** (`session/mod.rs:849`), so this is the
default path, not a corner.

**2 — Without modulation the gate observes the commanded number exactly.**
`predicted_feeds_present` is `false` on all unmodulated arms, so stage 3 is
1.0 *by absence of data* and stage 4 equals stage 1 to the digit. That makes
the A/B/C comparison maximally clean: nothing between the commanded feed and
the verdict except the band.

**3 — Adaptive feed modulation erases the lift completely.** Arms A and B are
commanded **2.4×–5.7× apart** and converge on the **same gate observation**
to within float noise on all four fixtures (0.05321, 0.03938, 0.02318,
0.01669). The lift's only effect in a modulated workflow is how far the
modulator has to travel to undo it: **−78.9 % instead of +20.1 %** on A3D-1.
Note the sign — on arm B the modulator *raises* feed, which is the
simulation-backed version of exactly the correction pass 8 is trying to make
blind.

**4 — Retiring the lift does not by itself make DropCutter correct.** Arm B
is `Exceeds` on both DropCutter fixtures at 1.69× and 1.62× the band max.
That residual is **not** arc-fit's: it is present before pass 8 runs and is
the DOC-derate denominator divergence already ledgered (census F-3 / C-2 /
C-5, named on `FeedsResult::effective_diameter`). Arc-fit was *amplifying* it
roughly 3×, not causing it. A reader must not credit disposition (a) with
fixing it, nor blame (a) for failing to.

---

## 4. Step 3 — the three dispositions, with measured after-numbers

### 4.1 Shipped feed under each disposition

| disposition | A3D-1 | A3D-2 | DC-1 | DC-2 |
|---|---|---|---|---|
| **shipped today** | 6912.0 | 6000.0 | 4405.0 | 2500.0 |
| **(a) retire the automatic feed-up** | **1215.0** | **2243.0** | **1487.0** | **1029.0** |
| **(b1) label only, still on by default** | 6912.0 | 6000.0 | 4405.0 | 2500.0 |
| **(b2) estimated mode, off by default** | 1215.0 *(6912.0 opt-in)* | 2243.0 *(6000.0)* | 1487.0 *(4405.0)* | 1029.0 *(2500.0)* |
| **(c) sim-backed** — Suggest emits (a), the lift happens post-sim | 1215.0 → modulated | 2243.0 → modulated | 1487.0 → modulated | 1029.0 → modulated |
| *(excluded reference)* re-key ratios to 1.0 | 1728.0 | 3189.8 | 660.7 | 478.4 |

### 4.2 The verdict each disposition ships

This is the column that decides the checkpoint.

| disposition | A3D-1 | A3D-2 | DC-1 | DC-2 | clean |
|---|---|---|---|---|---|
| shipped today | **Exceeds** | **Exceeds** | **Exceeds** | **Exceeds** | 0/4 |
| (a) retire | Within | Within | **Exceeds** | **Exceeds** | 2/4 |
| (b1) label only | **Exceeds** | **Exceeds** | **Exceeds** | **Exceeds** | 0/4 |
| (b2) off by default | Within | Within | **Exceeds** | **Exceeds** | 2/4 |
| **(c) sim-backed** | **Within** | **Within** | **Within** | **Within** | **4/4** |
| *(reference)* re-key to 1.0 | Within | Within | Within | Within | 4/4 |

(c)'s row is measured, not projected: it is the `B + modulation` arm of §3.2 —
Suggest emitting the un-lifted feed, with a simulation-backed pass doing the
correction from the observed number.

### 4.3 Reading each disposition honestly

**(a) Retire the automatic feed-up entirely.** Fully specifiable today, one
decision site (§1), no new machinery. Removes a defect that is `Exceeds` 4/4
and replaces it with `Exceeds` 2/4 — and both residuals belong to a different,
already-ledgered cause. Cost: the pre-simulation path loses its only automatic
rubbing remedy. **Measured mitigation:** on both Adaptive3d fixtures the
calculator *already* lands the commanded advance per tooth on the derated band
minimum without pass 8 (0.03797 vs 0.03800; 0.04673 vs 0.04676 — 0.999× both).
The rubbing case pass 8 was built to fix (Wanaka, "observed median 0.0067–0.011
vs LUT minima 0.027–0.076") was stated in the **arc-mean quantity that no
longer exists**; in the unit the gate now reports, the operating point was
already in band. That is the strongest single argument for (a).

**(b) Demote to a clearly-marked estimated mode.** Splits in two, and the split
matters:
* **(b1) label it, leave it on.** Numbers unchanged: `Exceeds` 4/4. This is the
  review's *interim* remedy, and this wave has **already shipped it** as the
  report-tier label (§6.0). As a *disposition* it is a no-op — it leaves a
  directionally-wrong number applied by default with a disclaimer attached.
* **(b2) off by default, reachable as an explicit estimated mode.** Identical
  default behaviour to (a), plus the maintenance cost of keeping a stale table
  alive and a mode that reproduces a known-bad number on request. It buys
  reversibility; it does not buy correctness.

**(c) Move the action into the simulation-backed optimizer.** The only
disposition that is clean 4/4, and the capability **already exists and already
works** — that is what `B + modulation` measures. The gap is not the
mechanism; it is that Suggest does not know the mechanism exists and lifts
blind beforehand. Two riders, both real:
* The **retargeter** path (`tool_load::optimize::retarget::chipload`) fires
  only on `ChiploadVerdict::Exceeds`, and
  `ChipBoundsSource::low_side_is_advisory()` demotes the low side to advisory
  for `VendorLutExtrapolated | VendorLutPointPreset | VendorLutMissingAe`
  (`verdict.rs:645-652`). Per **F-MISSAE**, 176 of 252 shipped rows are
  `VendorLutMissingAe`. So on most rows the *retargeter* would never see a
  burn-side trip to act on. The **modulator** path has no such gate, which is
  why the measurement used it.
* Both modulated DropCutter arms park the observation **exactly on the band
  maximum** (0.02318, 0.01669), with `ConstrainedMax` rewriting 100 % of moves.
  That is precisely the operating point **G-CHIP-ULP** shows can flip a verdict
  on 1 ulp. Not this wave's to fix; named so (c) is not adopted in ignorance
  of it.

**The excluded reference — re-keying the ratios to 1.0.** B-lit §3.3/§6.1 rules
"retire, do not re-key", and this wave's measurement supports that ruling with
a number the ledger did not have: on both Adaptive3d fixtures arm C lands at
**98.2 % of the gate's band maximum** — a 1.8 % margin — and it does so *by
luck*. Suggest and the gate matched **different band widths on 3 of 4
fixtures** (Suggest's band max is **1.273×** the gate's on both endmill
fixtures; the two ball fixtures agree to 1.006×). That is **F-LUT2** appearing
as a safety margin. Under `SuggestAggressiveness::Speed`, which targets the
band *maximum*, a re-keyed solve on A3D-1 would aim at 0.070 against a gate
maximum of 0.055 — 1.27× over, i.e. `Exceeds` — and no pre-simulation solve
can know that. See §8: the Speed-policy arm was not run.

---

## 5. The re-measured 4× / 6.7×

**The closed form reproduces exactly, and the review's figures are right as
closed-form figures.** Measured on the two fixtures where no cap binds:

| family | ratio | `1 / ratio` (predicted) | **measured A/C** | fixture |
|---|---|---|---|---|
| Adaptive3d | 0.25 | 4.000× | **4.00×** | A3D-1 (ceiling lifted) |
| DropCutter | 0.15 | 6.667× | **6.67×** | DC-1 (stock ceiling, uncapped anyway) |

**But the realised figure is not the closed-form figure, and the difference is
the machine.** On the two fixtures where the cutting-feed ceiling binds, the
lift is truncated and the retirement blast radius shrinks accordingly:

| fixture | ceiling | solved feed (uncapped) | shipped feed | **realised A/C** |
|---|---|---|---|---|
| A3D-2 | 6000 (derived `min(max_feed, DEFAULT_CUTTING_FEED_CAP_MM_MIN)`) | 12 758 | 6000 | **1.88×** |
| DC-2 | 2500 (small router `max_feed`) | 3190 | 2500 | **5.23×** |

So the honest statement of the blast radius is:

> Retiring the table moves the **solved** feed by exactly `1/ratio` — 4.00×
> for Adaptive3d and 6.67× for DropCutter — and moves the **shipped** feed by
> between **1.88× and 6.67×**, depending on whether the machine's
> cutting-feed ceiling was already truncating the lift. On a stock profile
> the derived ceiling is 6000 mm/min, and Adaptive3d roughing hits it on
> both fixtures measured here.

A corollary worth stating at the checkpoint: **the machine ceiling is
currently doing the safety work.** On A3D-1, lifting the ceiling from the
stock 6000 to 15 000 mm/min moved the shipped feed from 6000 to 6912 and the
commanded advance per tooth to 3.09× the band maximum. Operators with rigid
machines and an explicit `max_cutting_feed_mm_min` get *more* of the defect.

---

## 6. Step 4 — red-first specs, one per disposition

Step 4 of the review's package is *"red-first test the approved change"*. The
approval is Checkpoint J's, so what follows is the **spec** — which sentry,
which assertion, what colour it is today — so the implementation wave starts
without a research round.

### 6.0 Already shipped, pre-checkpoint (report-tier)

The plan's standing allowance is discharged: the two rationale entries that
present the arc-fit prediction now carry a **"Legacy pre-simulation
estimate"** label plus a sentence dating the retired observation.

* Change: `crates/rs_cam_core/src/feeds/rationale.rs` — new
  `LEGACY_ESTIMATE_LABEL` / `LEGACY_ESTIMATE_NOTE` constants, applied to the
  `detail` of `ChiploadTarget` and `ChiploadCapBound`.
* **Placed in core, not viz, on purpose.** Measured this wave: the feeds modal
  prints `entry.headline` / `entry.detail` verbatim
  (`crates/rs_cam_viz/src/ui/feeds_modal.rs:1139-1146`) and the MCP rationale
  endpoint serialises the same struct. One string, every surface, zero viz
  diff — which also keeps this wave out of B-4's territory entirely.
* Sentry: `chipload_recalibration_entries_are_labelled_a_legacy_estimate`.
* **Report-tier by construction**: `from_value` / `to_value` (the applied
  feeds) are asserted unchanged in the same test. No recipe number moves, no
  verdict moves, no fingerprint moves.
* The label went into `detail`, never `headline`, because
  `feed_raised_uncapped_round_trips` asserts the headline does *not* contain
  `"capped"`. Existing string assertions all still hold.

### 6.1 Red-first for disposition (a) — retire

**New sentry** (this file): `retired_lift_leaves_feed_at_the_calculator_value`.
For each of the four fixtures assert:

1. `profile.warnings` contains **no** `FeedRaisedForChipload`;
2. `suggested_operation.feed_rate()` equals the pinned per-fixture arm-B feed
   (1215.0 / 2243.0 / 1487.0 / 1029.0) within 0.5 mm/min;
3. `predictions.observed_chipload.source` is no longer `Calibrated` for
   Adaptive3d or DropCutter.

**Colour today: RED on all four**, and by a large margin — the warning fires
and the feed is 2.43×–5.69× higher.

**The existing pre-fix reproduction stays.** `arc_fit_arms_gate_observation`
already asserts `arm_a.verdict == "Exceeds"` on all four. That assertion is
the permanent record of what was wrong and must survive the fix (it is
asserted about *arm A*, a feed the harness applies explicitly — not about
whatever Suggest currently returns).

**Re-baselines required in the same commit** — each currently *requires* the
lift to fire, so each inverts:

| file | what inverts |
|---|---|
| `tests/wanaka_suggest_integration.rs` | Back Rough: `assert_has_feed_raised` → `assert_no_feed_raised`. 3D Finish 6: the `cap_hit == Some(MaxFeed)` and `ChiploadStillLowAfterRecalibration` assertions delete. **Caution: `wanaka_suggest_baseline` is on the known-red list at HEAD** (the project file is operator-modified) — a re-baseline here must not be read as having fixed that. |
| `src/feeds/suggest.rs` in-crate tests | four, named and located: `feed_recalibration_raises_feed_for_wanaka_back_rough_case` (`:4200`, pins 3456 mm/min), `feed_recalibration_caps_on_deflection` (`:4360`), `speed_gated_by_deflection_fires` (`:4509`), `feed_recalibration_caps_on_max_feed` (`:4626`). `feed_recalibration_skipped_when_already_in_band` (`:4709`) asserts the *absence* of the warning and **survives unchanged** — it becomes the general case. |
| `src/feeds/predict.rs` in-crate tests | the `arc_fit_ratio` pins at `:1166` (0.25), `:1211` (0.15), `:1272` (0.25) — deleted with the table |
| `src/feeds/rationale.rs` | the `ChiploadTarget` / `ChiploadCapBound` arms and their round-trip tests survive **only if** the two `SuggestWarning` variants survive; if (c) is also adopted they are re-pointed at the optimizer rather than deleted |

**Fingerprint watch (§0 rule 2).** Pass 8 writes `feed_rate` only, so generated
**geometry** cannot move — but any baseline that embeds feed values will.
Check `tests/smoke_baseline_regression_f037.rs` and the `gcode_*` baselines
before committing; a fingerprint that moves is a STOP, not a re-pin.

### 6.2 Red-first for disposition (b2) — estimated mode, off by default

* `estimated_mode_is_off_by_default` — identical assertions to §6.1 under
  `SuggestPolicy::default()`. **RED today.**
* `estimated_mode_reproduces_the_legacy_lift` — with the opt-in set, the four
  arm-A feeds reproduce to ±0.5 mm/min. **GREEN today by construction**: this
  is a *pin*, not a red, and exists only to stop the opt-in drifting into a
  third behaviour.
* §6.0's label sentry stays as-is.

(b1) needs no new red: it is §6.0, already shipped.

### 6.3 Red-first for disposition (c) — simulation-backed

Two sentries, and they are not equal in readiness.

**(c-i) Suggest must stop lifting** — §6.1's red, unchanged. RED today.

**(c-ii) the post-sim correction must be measured, not assumed** —
`sim_backed_correction_lands_the_gate_in_band`, built on this file's arms:
run arm B, simulate at the recorded cell, assert the corrected run's stage-4
observation lands inside the gate band and the verdict is `Within` on all four
fixtures. **GREEN today via the modulator path** — that is exactly the
`B + modulation` arm of §3.2, which is why (c)'s row in §4.2 is measured
rather than projected. Promoting it to a sentry costs nothing new.

**(c-iii) the *retargeter* path is NOT red-firstable today, and this is a
Checkpoint J input, not an implementation detail.** A sentry
`retarget_lifts_feed_from_measured_gate_observation` — drive
`ChiploadFeedRetargeter` from arm B's verdict, assert the multiplier is
`target / gate_observed` and that re-simulating lands stage 4 in band — would
fail for a reason that is *not* the change under test: the retargeter fires
only on `Exceeds`, and `low_side_is_advisory()` suppresses the burn side on
the three weak `ChipBoundsSource` variants, which per **F-MISSAE** covers 176
of 252 shipped rows. This is also **F-OPT**'s missing fixture. If the operator
wants (c) via the *optimizer* rather than the *modulator*, F-MISSAE has to be
ruled first.

---

## 7. Checkpoint J — the questions

**J-1 — the disposition.** Retire the automatic feed-up (a); demote it to a
clearly-marked estimated mode (b); or move the action into the
simulation-backed path (c)?

> **Recommendation: (a) now, (c) as the destination — and they are one
> commit, not two programmes.**
>
> (a) is fully specifiable today, has one decision site, and removes a defect
> that is `Exceeds` **4/4** on the default (unmodulated) path. (c) is the only
> disposition that is clean **4/4**, and its mechanism **already exists and
> already works** — `B + modulation` is a measurement, not a design. What is
> missing is not machinery: it is that Suggest lifts blind *before* the thing
> that can actually see. Adopting (a) is what lets (c) work, because (a) is
> precisely "Suggest emits the un-lifted feed".
>
> **Against (b):** (b1) is a no-op — it leaves an `Exceeds`-4/4 number applied
> by default with a disclaimer, and this wave already shipped the disclaimer.
> (b2) has identical default behaviour to (a) while keeping a stale table and
> a mode whose only function is to reproduce a known-bad number on request.
>
> **Against re-keying to 1.0** (already ruled out by B-lit, now with a
> number): it passes here with a **1.8 % margin** on both Adaptive3d fixtures,
> and only because Suggest's band happens to sit where it does. Suggest and
> the gate matched different bands on 3 of 4 fixtures (**1.273×** apart on
> both endmill fixtures). Under `SuggestAggressiveness::Speed` the same solve
> would aim **1.27× over** the gate's maximum. No pre-simulation solve can
> know this while **F-LUT2** stands.

**J-2 — does Suggest keep *any* automatic feed-up?** (a) removes the only one.
On both Adaptive3d fixtures the calculator already lands the commanded advance
per tooth on the derated band minimum unaided (0.999×), so the rubbing case
pass 8 was built for appears to have been an artifact of the deleted arc-mean
quantity. Is the operator content that the band **minimum** is the right
resting place for an un-lifted recommendation, with the lift toward the band
**median** delegated to the simulation-backed pass?

**J-3 — modulation is off by default.** `SimulationOptions::default()` has
`adaptive_feed_modulation: false` (`session/mod.rs:849`). (c)'s 4/4 result is
measured **with modulation on**. Does adopting (c) mean flipping that default,
or does it mean Suggest's recommendation must be safe unmodulated (which is
(a) + accepting `Exceeds` 2/4 from a different cause)? **This is the question
that decides whether (c) is a change of default or a change of advice.**

**J-4 — the two DropCutter residuals.** Arm B is `Exceeds` at 1.69× / 1.62×
band max from the DOC-derate denominator divergence (census F-3 / C-2 / C-5),
not from arc-fit. Does that block adopting (a) — or is it correctly a separate
row, given (a) reduces the exceedance ~3× and does not create it?

**J-5 — scope of the retirement.** Does retiring the table mean deleting
`arc_fit_ratio_for_op`, `ObservedChiploadPrediction` and the two
`SuggestWarning` variants outright, or keeping the warning variants so the
simulation-backed path can reuse them for its own (measured) feed changes?
Note the `Default` ratios currently gate *nothing* — only `Calibrated` rows
reach pass 8 — so retiring the table removes no behaviour from the other
fifteen families; it removes a computed-and-dropped field (§1).

---

## 8. What was NOT exercised, and the blocker for each

| not exercised | blocker / reason |
|---|---|
| **`SuggestAggressiveness::Speed` on any arm.** §4.3 argues a re-keyed solve under Speed would exceed the gate band by ~1.27× on A3D-1. That is **derived from the two measured band widths, not run.** | Out of the four-step package's scope; needs its own arm + sim. Cheap follow-on — one policy field on `SuggestContext`. |
| **The optimizer *retargeter* path (c-iii).** (c) was measured through the **modulator**, not through `ChiploadFeedRetargeter`. | The retargeter fires only on `Exceeds`; `low_side_is_advisory()` suppresses the burn side on 176/252 shipped rows (**F-MISSAE**). Cannot be honestly red-firsted until F-MISSAE is ruled. Also **F-OPT**. |
| **The operator's wanaka project.** Every fixture here is synthetic. | §0 rule 9 (the play-file is read-only) and `wanaka_suggest_baseline` is already red at HEAD because the file is operator-modified. Using it would have confounded the measurement. |
| **A `Default`-ratio family (Pocket, Profile, Waterline, …) under any arm.** | They receive **no lift at all** today — pass 8 is gated on `Calibrated`. There is no before-number to measure. This is also why "retire" extends nothing to them, contrary to what a reading of F-T35's "extends the lift to every other family" might suggest: that clause describes *re-keying to 1.0 while keeping the gate*, which is the excluded option. |
| **G-code / smoke fingerprint impact of (a).** | Named as a pre-commit check in §6.1; not run, because no change has been approved yet and running it now would measure nothing. |
| **A rendered GUI surface for the new label.** | §0 rule 3 asks for a screenshot for visible-surface claims. The label is a pure string substitution in a `detail` line the modal already renders verbatim, asserted by a sentry; a screenshot belongs with the *disposition* change, when the panel's numbers actually move. Recorded here as a known gap, not claimed as discharged. |

---

## 9. One-line summary for the tracker

Measured on 4 synthetic fixtures (2 per affected family, cell 0.4 mm, all arms
stamp-equal): **Suggest's shipped recommendation trips the chipload gate on
4/4 fixtures**, at 1.85×–5.00× the gate's band maximum. The retirement blast
radius reproduces the review's closed form exactly — **4.00×** (Adaptive3d,
`1/0.25`) and **6.67×** (DropCutter, `1/0.15`) — but the *realised* move is
**1.88×–6.67×** because the machine cutting-feed ceiling already truncates the
lift on stock profiles. Retiring the lift is clean 2/4; the
simulation-backed path is clean **4/4** and already works. Recommendation:
**(a) now, (c) as the destination**.
