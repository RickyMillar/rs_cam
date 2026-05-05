# 3D rough engagement investigation plan

Three independent investigations to understand why wanaka's Back Rough produces:
- 4% average engagement on a "rough" operation (should be 25–40%)
- "Random hole drilling" — circular bores visible in sim that the user remembers as not-being-there in older runs
- Bipolar verdict that no feed/RPM tuple can clear

The optimizer work shipped today fixes the breakage half. The remaining problem is structural — geometry / generator / gate calibration. These three investigations identify the root cause.

**Order them by cheapest first.** Investigation 3 is git archaeology that may answer the whole question in 30 minutes. Run it first; if it points cleanly at one commit, the other two might collapse to "audit that commit."

---

## Investigation 3 — what changed in the generator history? (do first)

**Hypothesis:** the user remembers wanaka's Back Rough cutting cleanly. There's a regression from a recent commit — likely either the AgentSearch introduction, the boundary-walk patch (memory note 2026-04-15), or a recent change to `find_entry_3d` / `clearing.rs`.

**Why it matters:** if a single commit caused this, the cheapest fix is to revert or surgically patch that commit. The other two investigations become "verify the fix worked," not standalone work.

**Method:**

1. Read current `clearing_strategy = "agent_search"` in wanaka_full_tuned.toml's git history:
   ```
   git log --all --follow -p /home/ricky/Downloads/wanaka100/wanaka_full_tuned.toml | grep -B2 -A1 "clearing_strategy"
   ```
   Identify which commit set this value and what it was before.

2. Check the commits introducing or modifying AgentSearch:
   ```
   git log --oneline --follow crates/rs_cam_core/src/adaptive3d/clearing.rs
   git log --oneline --follow crates/rs_cam_core/src/adaptive3d/search.rs
   git log --oneline -- crates/rs_cam_core/src/adaptive3d/path.rs
   ```
   The 2026-04-15 boundary-walk patch should appear here. Also look for changes to `find_entry_3d` (the function that picks plunge entries — likely culprit for "random hole drilling").

3. Cross-reference with the `peck_plunge` logic added in `path.rs:554-580` (the "punched hole" guard mentioned in the source comment). When was that added, and was it added in response to a complaint? Read its commit message.

4. Reproduce on an older commit:
   - Pick the parent of the 2026-04-15 boundary-walk patch (or whichever commit looks like the regression)
   - Check out that commit (worktree, don't disturb master)
   - Generate Back Rough on the same wanaka_full_tuned.toml
   - Run sim, screenshot, compare to current sim screenshot
   - If the holes are gone in the older sim, we've found the regression

5. If no single commit looks like the trigger, expand the search to AgentSearch's predecessor (`ContourParallel` or `Adaptive`) — try generating with each and visually compare. The user might have been on a different `clearing_strategy` originally.

**Files to read:**
- `crates/rs_cam_core/src/adaptive3d/clearing.rs` (especially `find_entry_3d`)
- `crates/rs_cam_core/src/adaptive3d/path.rs` (peck-plunge logic, line 554)
- `crates/rs_cam_core/src/adaptive3d/search.rs`
- `crates/rs_cam_core/src/adaptive3d/mod.rs:75-110` (entry style + clearing strategy enums)
- The `.toml` project file's git history

**Expected outputs:**
- A specific commit hash + diff that introduced the regression, **OR**
- A clear statement that AgentSearch never produced clean output on this geometry and the user is misremembering an older run with a different strategy

**Decision criteria:**
- If single commit identified → triage as bug, plan revert or surgical fix
- If gradual drift across many commits → a redesign of the entry / clearing logic is needed (Investigation 1 informs that)
- If user is misremembering → reset expectations; this geometry needs a different op type

**Time estimate:** 1–2 hours. Faster if the commit history is clean.

**Dependencies:** none. Run first.

---

## Investigation 1 — engagement histogram on Back Rough

**Hypothesis:** the AgentSearch (or its predecessor) is producing toolpaths where most cutting moves graze already-cleared regions or sit at near-threshold engagement. The 4% average isn't a fluke — it's a per-pass distribution skewed heavily to low engagement, with rare slot peaks pulling the gate verdict.

**Why it matters:** the bipolar verdict can't be cleared by feeds. Either the generator stops producing the low-engagement moves, or the gate filters them. This investigation tells us *which* moves are pathological so we can fix the right thing.

**Method:**

1. **Build a histogram of per-sample `radial_engagement`** for Back Rough's trace. Buckets:
   - 0–0.02 (filtered as air by current gate threshold)
   - 0.02–0.05 (just above threshold — burn-verdict territory)
   - 0.05–0.15 (light cut)
   - 0.15–0.30 (typical adaptive)
   - 0.30–0.60 (aggressive adaptive)
   - 0.60–1.0 (slot territory)

   Easiest path: a one-off integration test under `crates/rs_cam_core/tests/` that loads wanaka, generates Back Rough, runs sim, and prints the histogram. Output to stdout or write a markdown table.

2. **Cross-reference with `semantic_item_id`** on each sample. Each sample carries a link to a `SemanticItem` from generator instrumentation (Pass / Entry / Cleanup / SlotClearing / Boundary etc., 26 kinds total per `AI_MACHINIST_ANALYSIS_REFERENCE.md` line 196). For each engagement bucket, count which semantic kinds are over- or under-represented.

   **What this tells us:** if 80% of "0.02–0.05" samples are `Entry` or `BoundaryClip` semantic items, the regression is in the entry/boundary code. If they're `Pass` items, the main clearing logic is wandering.

3. **Spatial heatmap (optional, nice-to-have):** plot move samples colored by engagement on top of the toolpath SVG. The "drilled holes" should appear as point clusters with high engagement surrounded by low-engagement air. The terrain regions where engagement is consistent will appear uniform. This visually confirms what the histogram already says quantitatively.

4. **Repeat with `clearing_strategy = "contour_parallel"` and `clearing_strategy = "adaptive"`** (the other two enum variants). Compare histograms — if ContourParallel has a notably better distribution, the user should switch. If all three are bad, the geometry is the issue (Investigation 3 confirms or denies this).

**Files to read:**
- `crates/rs_cam_core/src/simulation_cut.rs` — `SimulationCutSample.radial_engagement`, `semantic_item_id`
- `crates/rs_cam_core/src/semantic_trace.rs` — semantic kinds enum
- `crates/rs_cam_core/src/adaptive3d/mod.rs` — what each generator pass calls itself in semantic terms

**Files to potentially add:**
- `crates/rs_cam_core/tests/wanaka_engagement_histogram.rs` — the integration test
- A small CLI utility under `crates/rs_cam_cli/src/` for ad-hoc inspection on other projects

**Expected outputs:**
- A histogram showing where the engagement distribution actually sits
- A correlation table: bucket × semantic kind
- A clear narrative: "X% of cutting time is in semantic kind Y, which the generator emits during phase Z"

**Decision criteria:**
- If a single semantic kind dominates the low-engagement buckets → fix that generator phase
- If the distribution is genuinely smooth (no spike at low engagement) → the 4% average is wrong; bug in the avg calculation
- If the distribution is bimodal (concentrated at very-low + very-high) → bipolar is geometric; gate calibration (Investigation 2) is the right fix

**Time estimate:** 3–5 hours. Mostly setup time; the histogram itself is a few lines.

**Dependencies:** Investigation 3 first (might short-circuit this).

---

## Investigation 2 — gate engagement-threshold calibration

**Hypothesis:** the `STEADY_STATE_FEED_FRACTION = 0.95` and `radial_engagement < 0.02` filters in `chipload.rs` let through transient samples right at the threshold. These produce a deterministic minimum effective chipload that always lands below `cl_min`, generating false-positive burn verdicts on otherwise-fine operations.

**Why it matters:** the wanaka burn-risk peak `0.007235340736122094` was bit-identical across DOC/stepover/strategy variants — a clear signal that it's a calibration artifact, not real cutting. If raising the threshold suppresses this without missing real burn cases, every 3D rough op gets a more honest verdict.

**Why this isn't just "raise the threshold to 5% and ship":** raising the threshold also suppresses real burn cases on light-cut operations like project_curve where steady-state engagement legitimately sits at 5–10%. Need calibration, not a one-line change.

**Method:**

1. **Quantify the artifact.** From Investigation 1's histogram, count what fraction of Back Rough's cutting time sits in each engagement bucket. The 0.02–0.05 bucket is the suspect zone — samples filtered "just barely" by the current 2% threshold.

2. **Find the formula behind the deterministic `0.007235340736122094` value.** It should fall out of the simulator's `effective_chip_thickness_mm` calculation at `radial_engagement = 0.02`-ish, `feed = 1500`, `rpm = 18000`, `flutes = 2`. Read `simulation_cut.rs` and back out the math. Verify the value matches a closed-form prediction. If yes, the artifact is fully explained.

3. **Sweep the threshold experimentally:**
   - Add an env-var override on `STEADY_STATE_FEED_FRACTION` and the engagement filter (or just wire two new constants for testing)
   - Re-run wanaka Back Rough at threshold values: 2% (current), 3%, 5%, 8%, 10%, 15%
   - Record verdict + peak chipload at each threshold
   - Find the threshold where Back Rough's burn verdict disappears

4. **Sweep against real burn cases.** Pick 1-2 known-good "burn" cases — e.g., a project_curve at deliberately-too-low feed, or a deliberately-too-high RPM toolpath. At each threshold value, do they still surface as Burn? If yes, the threshold raise is safe.

5. **Compare to vendor-published guides.** Read 1–2 vendor F&S charts (Amana, Onsrud — `research/feeds_and_speeds_integration_plan.md` § 9 mentions the LUT data sources). What engagement floor do the vendors implicitly assume in their chip-load tables? Many vendor tables explicitly state "for ≥30% engagement" or similar. That's the calibration anchor.

**Files to read:**
- `crates/rs_cam_core/src/tool_load/chipload.rs:63` (the constant) and 119 (the filter)
- `crates/rs_cam_core/src/simulation_cut.rs` — `effective_chip_thickness_mm` calculation
- `research/feeds_and_speeds_integration_plan.md` § 9 — vendor data references

**Expected outputs:**
- A table showing verdict vs threshold for ≥3 toolpaths
- A formula for the threshold value — ideally derived from the LUT row's calibrated `ae_min_mm` field rather than a fixed 2%/5% number
- A go/no-go decision on whether to raise the constant

**Decision criteria:**
- If raising threshold to 5% suppresses Back Rough's noise without losing real burn cases on the test set → ship the change as a single constant edit
- If real burn cases get suppressed at 5% → the right threshold is per-LUT-row, derived from `matched_row.ae_min_mm` if present
- If neither approach cleans the verdict → calibration isn't the issue; Investigation 1's findings dominate

**Time estimate:** 4–6 hours. Most of it is finding good test cases.

**Dependencies:** Investigation 1's histogram (lets you target the threshold sweep where it matters).

---

## How to use this plan

Run **Investigation 3 first.** If git archaeology surfaces a regression commit, the next steps depend on what that commit did:

| Commit type | Next step |
|---|---|
| Boundary-walk patch broke entry placement | Surgical fix to `find_entry_3d` |
| AgentSearch always produced this; user was on ContourParallel previously | Switch wanaka to `clearing_strategy = "contour_parallel"`, validate, document |
| AgentSearch slowly drifted across many commits | Investigation 1 informs the redesign |

**If Investigation 3 doesn't surface a clean cause**, run Investigation 1 next. The histogram + semantic correlation will tell you which generator phase is responsible for the low-engagement moves. Investigation 2 closes the loop on whether the gate verdict can be made trustworthy regardless.

**If you're just looking for "make wanaka work today",** the practical answer is the experimental config from earlier — feed 1800 on TPs 1 and 6, baseline geometry, accept the burn-risk verdict as gate noise. The cut will execute safely on the machine even though the verdict is red. But that's not a fix to the underlying issue, just a workaround.

## Out of scope for this plan

- Fixing the generator pathology directly. That's downstream of whichever investigation result wins.
- Re-calibrating the LUT bounds against user observations (long-term work, separate plan).
- The 3D Finish 6 toolpath's behavior — same tool, different op type, different LUT routing. May or may not have the same issue. Test as a control once Back Rough is understood.
- AgentSearch's dead-code path in CLI (separate memory entry).
