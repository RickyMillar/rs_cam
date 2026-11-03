# Root causes of the nz-wanaka-21 feeds review findings (2026-11-03)

Six read-only research agents traced each finding of `../compare.md` into the engine. Each report
gives the code path (file:line), the evidence, fix options, the blast radius and the sentries.
The lead checked these citations against the code: registry.rs Project Curve `strip_all`,
3D Rough `FeedsHints::NONE`, `suggest/apply.rs:160-183`, the `feeds/mod.rs` rigidity floor and
flute guard, and `rough_score.rs:448-463`. Paths below are under `crates/rs_cam_core/src/` unless
they say otherwise.

| Report | Ops | Finding |
|---|---|---|
| `01_op1_op14_bulk_18mm.md` | 1, 14, 0, 12 | Suggest depth 14.05, 16219 rpm, power, 0.185 chip, 2 plunges, face stepover |
| `02_op5_op18_slot_profile.md` | 5, 18 | slot chip 0.382, rpm 7843, plunge 2999, no power, 168 straight plunges |
| `03_op2_rest_entry.md` | 2 | 14.5 mm full-width entry, 13 mm links, entry-load gate only reports |
| `04_op3_op4_drill.md` | 3, 4 | hole deeper than the flute, peck, dwell |
| `05_ops6_8_dmm_plunge.md` | 6, 7, 8 | straight plunges, centre cutting, Suggest small-tool values |
| `06_vbits_r1_risky.md` | 9, 10, 13, 15, 16, 17 | V-bit depth and tip, R1 depth and chip, long-tool factor |

## Root causes by layer

### 1. Suggest does not receive the real cut (the largest cause)

The registry `feeds_hints` of several ops is `FeedsHints::NONE`, and some ops have no total depth.
The calculator then uses family defaults, and the apply step writes its own values over the
operator's.

- 3D Rough (`compute/catalog/registry.rs:824`): no depth hint. The rigidity floor gives 2 x D
  (`feeds/mod.rs:3433`), the flute guard cuts it to 0.8 x flute = 16 mm (`feeds/mod.rs:2110`), the
  aggressiveness dial gives 14.047 (`feeds/suggest/aggressiveness.rs:220-250`). Ops 1, 14 (01).
- Face (`registry.rs:86`): no stepover hint, so a pocket default 0.35 D replaces 14.4 mm. Ops 0, 12 (01).
- Project Curve and Profile: no radial engagement and no depth. A full slot is never seen; the
  calculator assumes a light cut (`feeds/mod.rs:2076, 3007, 3054`). Ops 5, 18, 9 (02, 06).
- Trace: no depth (`registry.rs:522`); `suggest/apply.rs:160-181` writes 0.06 x D over the
  operator's depth, so the V60 slit gets 14 passes. Ops 10, 13 (06).
- R1 rest: the default depth is 2 x the tip diameter scaled by the dial, not the operator's
  value (`feeds/mod.rs:3433`, `aggressiveness.rs:146`). Op 16 (06).

### 2. Suggest rules that ship an unsafe number

- Vendor row choice: the pass role is only a score term (`feeds/vendor_lookup.rs:1036`); a score
  tie is broken by row id (`vendor_lookup.rs:741`), which picks the Onsrud compression finish row
  with the high chip. Ops 5, 18 (02).
- Feed cap and rpm: when the chip x the chart rpm passes the 6000 mm/min cap, step 7 lowers the
  rpm and holds the chip (`feeds/mod.rs:2603-2672`, ruling R4 Q10). On a constant-torque spindle
  this lowers the available power (ops 1, 14: 16219 rpm, 1.01 kW) and on a slot it doubles nothing
  but keeps a chip that was already wrong (op 5: 7843 rpm). (01, 02)
- Chip thinning is measured but not applied (ruling 2026-08-19, `feeds/mod.rs:2164-2209`). (01)
- Plunge: the plunge is the side feed / flutes (`feeds/plunge.rs:308`, `extrapolation/plunge.rs:631`,
  `suggest/apply.rs:265`); the sourced axial chip sets only the ramp feed. Op 5: 2999 mm/min. No
  rule computes the V flank chip of a plunge (`plunge.rs:307`). (02, 06)
- Drill: Suggest writes a 15 mm peck (`suggest/apply.rs:900, 1079-1104`) and has no hole-depth
  input; the feed is the Amana ramp-down chip at the drill rpm cap. (04)
- The long-tool factor never lowers the feed (`feeds/mod.rs:2236`) and has no effect on finish
  ops (`aggressiveness.rs:173`). (06)
- Data: the Amana ZrN tapered v8 cell prints a chip about 2 x the chip its own feeds imply
  (`amana_zrn_tapered_v8.json:159-163`). Op 17 (06). The DMM chip comes from a 2-flute row. (05)

### 3. Power before and after the simulation are not connected

- The pre-sim power uses the nominal stepover only (`feeds/operating_point.rs:205`); Profile and
  Project Curve give `NoRadialEngagement` (`operating_point.rs:294-301`). (01, 02)
- The sim peak (1.18 kW on op 1) does not return to Suggest (`feeds/profile.rs:17`). It reaches
  only an export refusal and the per-move power cap of the feed modulator. (01)

### 4. Generation

- Project Curve strips every entry dressup (`registry.rs:1354`, reason: phantom diagonal cuts on
  multi-ring DXFs), so each chain start is a straight plunge at the plunge rate
  (`ops/project_curve.rs:379, 393` at HEAD). Op 5: 168 straight plunges up to 10 mm. (02)
- 3D Rough rest, op 2: the per-level work map compares stock with the 6 mm drop-cutter height
  (`adaptive3d/clearing.rs:276-287, 433-470`); the fillet sliver the 18 mm leaves in a pocket
  corner is never planned, and the ramp legs run a fixed +x direction for up to 19 mm, clipped to
  the model only (`rest_stock: None`, `adaptive3d/path.rs:2160, 2290`). Needs sim to say which
  move takes the 14.5 mm. The 13 mm links: low-confidence candidate, a stale proof start
  (`clearing.rs:3296, 3811, 2305`). (03)
- 3D Rough does not plan its cut and link descents to the non-centre-cutting angle limit; the
  rule allows a straight plunge up to the protrusion (`compute/tool_config.rs:374`). (01)
- No op compares a hole depth with `cutting_length` or `stickout`; the holder check skips a DXF
  model and meets the stock top at 0 mm, under the 0.01 mm strike limit
  (`stock/collision.rs:331, 441`). The 30 mm depth comes from `derive_wanaka21.py:594`. (04)
- The peck cycle emits no G4 dwell (`gcode/mod.rs:515`); the op 4 dwell has no effect. (04)
- A tool with `non_centre_cutting = None` counts as centre cutting (`compute/tool_config.rs:478-480`);
  nothing warns on a plunge with a tool of unknown centre cutting. No entry moves to an opening an
  earlier op made. (05)
- V-bits are sharp: `ToolConfig` has no tip field (`compute/tool_config.rs:589-592`,
  `compute/cutter.rs:32-37`); the V20 cone tip sits 0.85 mm off in Z. (06)

### 5. Gates that only report

- The entry-load gate reports by design (`stock/sim_triage.rs:1258, 1504`); export refuses only on
  height order, `cuts_into_model` and G-CENTRECUT (`session/compute/export.rs:84-115`). (03)
- Before the simulation, Suggest does not read acceleration; only the post-sim gate does
  (`tool_load/mod.rs:72`). (06)

### 6. Tooling errors found on the way

- `rs_cam_cli rough-score` names an entry run by its first move (`crates/rs_cam_cli/src/rough_score.rs:448-463`),
  so a ramp that starts with a short feed through air counts as a plunge. This made op 8 look like
  9 plunges and 1 ramp; all 10 levels ramp. (05)
- `rs_cam_cli suggest` passes no fallback to `power_at_operating_point`, so Profile and Project
  Curve show `unmodeled`; the GUI uses the light default cut and shows a wrong figure. The CLI
  also does not print the drill peck. (02, 04)

## Corrections made to the report

`compare_calc.py`, `make_report.py` and `report.html` were corrected on 2026-11-03 and
`assert_report.py` passes:

1. Op 3 and op 4: Suggest writes a 15 mm peck (not "keeps the operator's peck").
2. Op 4: the 0.5 s dwell has no effect (no G4 in a peck cycle); risk safe.
3. Op 8: all 10 levels ramp; risk risky (centre cutting unknown), removed from the dangerous card.

## Open: needs a simulation (operator go-ahead)

- Op 1: which sample gives the 1.18 kW peak (about ae 14 mm at 9 mm deep), and where the 2 straight
  plunges land.
- Op 2: whether the ramp leg or the waterline cleanup takes the 14.5 mm, and the 13 mm links.
