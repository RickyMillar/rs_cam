# Next session: feeds and speeds correctness (brief, 2026-11-08, second edition)

## Morning hand-off, 2026-11-10 (lead, written during the night run)

Committed overnight, in order: `5a56654dd` P22-0c-5 (every body reader on the mounted zones;
`ToolAssembly` gone; one holder answer on every surface); `ca13c390c` fourteen stale known-red
tests re-pinned (viz suite 1601 pass / 1 red: `the_inspector_text_is_bounded_ic1`, a lead
decision); `34c83e040` `perf_golden_sim_metrics` re-blessed (the bisect named 74af0f8e1, the S11
ruling; not a simulation defect); `125bb15dd` P22-0c-6a and `129deb5c2` P22-0c-6b (one upper
diameter rule, the cutting cap = width at the flute top never above the shaft on both engagement
doors, the ring plunge section `k = 1 - (d/D)^2`, a shank of 0 is not a fact); `6fd852527` +
`36b440b17` P22a (the wide steps on wanaka idx 1 were the 3D "Perimeter sweep", 72 %, not D65;
the exact `FeedCapSource` door); `7ef7a1284` P22f-1 (the nominal-margin arm of `grown_outlines`
deleted: the sweep is gone where the outline grows; idx 1 worst sample 1.98 -> 1.45 kW; Back bulk
rough motion lines 22920 -> 19650). Designs with lead rulings: `phases/P21_DESIGN.md`,
`P22-0c-6.md`, `P22f_DESIGN.md`, `P22f-2_DESIGN.md`, `KNOWN_REDS_TRIAGE.md`. Wanaka controller
`$30` corrected to 24000 (operator). Disk: two `cargo sweep` runs freed 205 GB from target/.

In the tree at hand-off (check `git status`): P22f-2a by agent `p22a-readout` (the D65 rule on
every fed geodesic step, `FeedCapSource::StockRead3d`; the band-edge arcs 1.45 -> 0.91 kW
predicted). Report: `phases/P22f-2a.md`. Then: P22f-2b (ops 20/21), P21a/P21b (byte-identical),
the wanaka read (GUI rebuild + /mcp), P22b, P22f-3, P22-0c-7.

Operator decisions waiting (plain words in `P21_DESIGN.md` section 9 and the lead's morning
summary): (1) the wanaka project setting `spindle_strategy = "constant_chip"` with
`spindle_rpm_ceiling = 24000` (needed for the 16 rpm acceptance rows; P21c); (2) the P21
acceleration rule after the A3 share is visible on wanaka; (3) P4 (the ball feed lift, the taper).


HEAD at the time of writing: `a52e88813` (P20d). The first edition of this brief (`02b832eb9`) is
done in part: section 1 of that edition became P20a and P20b.
The operator wants every feeds and speeds number to be correct and explainable, so that the app
can be shared. GUI work waits.

## Rule for this session: the app is under test

- A phase is done only when its rows in the acceptance table pass:
  `acceptance/wanaka_21.md` (and `.toml`, `README.md`, `derive_wanaka_21.py`; commit `a25ff317d`).
  A fresh agent grades the build. The table changes only by an operator ruling, never to match
  the app.
- Gates (clippy, G-OPGOLDEN, viz suite, FM1) prove only that nothing broke. FM1 is a regression
  pin, not a truth source. Do not call a number "correct" from a gate.
- Measure the emitted G-code, not the plan (see section 1).
- Protocol: the lead decides and verifies; agents implement and never commit. Every cargo command
  goes through `scripts/cargo_lane.sh` in `timeout`. Lead gate script: per phase, clippy,
  G-OPGOLDEN (46 known rows, `opgolden_known_46_rows.txt`), the viz suite (9 known red binaries
  including `the_selection_section_writes_edits_only_p4_3`; `g_pipesmoke` can SIGSEGV as a
  flake), fmt (3 known files). FM1 re-blesses only after the predicted cells are listed.

## Done on 2026-11-08

| Commit | Content |
|---|---|
| `998ac93a3` | P20a: the unsourced drill RPM tier is removed; drill feed ceilings from the 3 mm Spektra row |
| `a25ff317d` | The wanaka acceptance table: 108 rows; build 998ac93a3 PASS 31, FAIL 12, NEEDS-RULING 55 |
| `0a6e78116` | P20b: `feeds/vertical.rs` `VerticalRateLimit` (G6 axial chip and Z axis rate) for plunge, drill, ramp |

Records: `phases/P20.md` (audit table of 20 rows, P20a and P20b sections),
`phases/P20_chip_gate_abstain.md` (the chip gate finding, section 1 below).
The release GUI runs 998ac93a3 (not rebuilt for P20b; the operator said "rebuild later").

## Also done late on 2026-11-08

| Commit | Content |
|---|---|
| `42977948d` | P20c: `rate_bound_feed_mm_min` caps every fed move at the per-axis rate for its direction (one stage after the last feed writer); gates grade the reached feed (`use_predicted_feed_in_gates` = true) |
| `a52e88813` | P20d: `feeds/section.rs` `EngagedSection` (rectangle, V groove, axial circle); V-bit power from the cone; drill power advisory; `power_kw` is an Option (item 3b) |

Records: `phases/P20.md` sections 4 (P20c survey of feed writers and graders) and 5 (P20d).
The GUI is still 998ac93a3: rebuild, then re-grade the acceptance table (P20b, P20c, P20d rows).

Open from P20c/P20d (operator rulings): drill power stays advisory, or cite the MDF twist-drill
torque text (`planning/extrapolation_2026-09-24/fetch/G8/sources/materials_2019_mdf_drill_edge_radius.txt`;
the model reads 0.6-0.9x it); the drill power hover labels (ap is advance per rev, ae the
diameter); an operator-typed plunge and the axial-chip bound; the 2 mm 1F plunge (240, unsourced;
G10 refuses below 3.0 mm and 1 flute; Spektra prints 2F Ramp Down at 1/16 and 3/32 in).
Open in code: a `ReachableFeed` type (about 450 writer sites); the drill gate and plunge stress
read the op config, not the emitted move; accel reach is graded but not written; the drill
engagement floor still writes ap 0.05, ae 0.02; deflection still reads the rectangle on a V-bit.

## 1. Top finding, not yet fixed: the feed cap of 3000 and the chip gate

- Every toolpath in `planning/rivmap_block_job/nz_wanaka_21_operator.toml` has
  `feed_max_rate = 3000.0`. The modulator applies it as a hard cap (G-FEEDMAX). An export on
  2026-11-08 (three per-setup files) has no feed above F3000. The operator's 8475 mm/min on
  "Back bulk rough 18 mm" runs at 3000 (chip 0.083 mm, not 0.235 mm).
- The app does not tell the operator: the feeds card shows 8475, and the power gate reads the
  uncapped feed (it reports 1.25 kW, which is wrong for F3000).
- The chip gate abstains on every op with `feed_max_rate < 0.95 x feed_rate`
  (`tool_load/chipload.rs:223,234` compares the samples with the uncapped `feed_rate`), so it
  says "not enough steady cutting" on all roughs. Fix in `P20_chip_gate_abstain.md`: one
  reference feed `min(feed_rate, feed_max_rate, cutting ceiling)` for every gate. This is the
  same class as the P16 "one gate feed" rule. Do it before item 4c.
- Ask the operator where the 3000 came from (an old GUI default?) and if it is wanted. Under the
  "no underived defaults" ruling a written 3000 with no derivation is a defect.

## 2. Operator rulings (2026-11-08)

- P21 rebuilds the existing "Max speed" spindle strategy as the constant-chip strategy, at the
  Suggest level (one RPM per op; a VFD cannot change speed along a path).
  - It refuses without stated machine axis rates and accelerations.
  - It starts conservative: an operator RPM ceiling field; empty = no rise above the chart RPM.
  - No unexplained multipliers: `MAX_SPINDLE_SPEEDUP` 1.5, the milling RPM diameter tiers and
    `SPINDLE_CEILING_HEADROOM` 0.95 go. Every cap is a named limit (spindle max, power at that
    RPM, XY rate, Z rate, acceleration on the modulated moves, the operator ceiling).
  - Later: one "N % of the theoretical limit" setting (the old aggressiveness dial), applied to
    the binding limit and stated on the card.
- With a constant-torque VFD, power never binds a constant-chip RPM rise (required and available
  power both scale with RPM). On wanaka the spindle max binds every milling op; the Z rate binds
  the drills at 2500 / (0.0635 x 2) = 19685 rpm.
- The 10610 rpm on the 6 mm Onsrud-row ops (wanaka 5, 17) comes from the unsourced
  `material.base_cutting_speed_m_min.solid_wood`. The Onsrud chart prints a chip only. Fix it
  inside P21 (the RPM is a policy choice), not as a separate patch. The same 10610 sits in the
  project's `[job.post] spindle_speed`.

## 3. Open rulings in the acceptance table (ask the operator)

P1 RPM policy (mostly ruled above); P2 which chart row stands for the unbranded 6 mm tool;
P3 adaptive chip lift from the nominal stepover or the 1.6x engagement cap; P4 lift the ball feed
for axial thinning at a shallow depth (`calculator/feed.rs:97-98` says "observed, not applied").

## 4. Other open defects (from the acceptance table and the critique)

- Scallop (wanaka 16): SpeTool prints 0.0762 mm for a 2 mm tip; the app ships 0.0644 mm.
- The 6 mm tool gets two chips in the same oak (0.127 and 0.149 mm).
- A false alarm: "plunges faster than the plunge rate" at exactly 1.0x on the drills.
- The envelope message says 102 % where the power ratio is 111 %.
- Unsourced limits on the card: the 2 x D depth rule, the 88 % long-tool target, the vendor axial
  bound, the peck depth default (audit row 6), the drill floors (row 5).
- Machine file: `controller.rpm_max = 1000` while the spindle is 6000-24000; the operator checks
  grblHAL `$30`.
- The earlier list still holds: item 4c, BandMid, D2 (blocked on the Onsrud 60-100MW rows),
  D3/D6, D4, D13, D5, the HighAirCut headline.

## 5. The operator's cut (not yet cut)

Safety read on 998ac93a3 (generate all, simulate, export warnings): no collision, no rapid
collision. After that read the operator fixed some lakes in the project (2026-11-08,
uncommitted edits to the TOML), so regenerate, re-simulate and re-read the per-op warnings
before any verdict. The acceptance table rows are per toolpath and stay valid unless a toolpath,
tool or feed changed. Before the cut:

- The F3000 cap (section 1): the operator must decide whether the roughs run at 3000 or at the
  card feed. At the card feed, "Back bulk rough 18 mm" at 8475 needs about 1.11-1.25 kW of
  1.125: lower it to about 7000.
- Entry load (critical) on "Back rest 6 mm" (peak 15.9 mm at 94.1, 89.1, 8.84) and "Front 3D
  rough" (7.5 mm at 343.7, 36.8, 17.5): look at both on screen.
- Wood above the flute (export warnings): "Pin drill, flip pins" 30 mm over a 25 mm flute, "V
  rivers and lakes" 9.4 mm over 8.15 mm, "Day strip slit V60" 7.4 mm over 5.5 mm. Check each.
- Three ops emit feeds modulated from an older simulation run: regenerate them before export.
- `$30` (above).

## 6. GUI (later, low priority)

P17b open items in `phases/P17.md`; the explainer page `explainer_p13c/` has stale claims.
