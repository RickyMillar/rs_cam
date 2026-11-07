# Next session: feeds and speeds correctness (brief, 2026-11-08, second edition)

HEAD at the time of writing: `0a6e78116` (P20b). The first edition of this brief (`02b832eb9`) is
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

## In flight at the end of the session (check first)

Two implementing agents ran when the session ended. Look for their records in `phases/P20.md`
sections "P20c" and "P20d" and for uncommitted files under `crates/`. Review by path, run the
gate, and commit with explicit paths. If a record is missing or partial, the work is not done.

- **P20c, one per-move machine limit.** Every commanded feed (cut, plunge, ramp, entry, link) and
  the chip gate go through `MachineKinematics::effective_max_rate_mm_min(dir)` (it exists in
  `machine/kinematics.rs`; only the cycle-time estimate used it). Covers audit row 14 (the
  dressup entry plunge, `compute/catalog/entry_contract.rs`, 0.5 x first linear feed, unsourced)
  and the modulator's lifted feeds. Also an investigation of the 2 mm 1F plunge (240 mm/min,
  likely the unsourced material base plunge; G10 covers 3.0-12.7 mm and 2-3 flutes only).
- **P20d, power from the true section.** Drill and plunge: circle (pi D^2/4 x vf); V-groove:
  triangle (h^2 tan a, edge h / cos a); ball and tapered tips from the tip shape. One path with a
  section enum that every caller must choose. Power is an Option ("not modelled", never 0.0;
  P18 item 3b).

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
collision. Before the cut:

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
