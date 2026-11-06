# Recheck of the feeds review at HEAD bd72997c9 (2026-11-06)

The first review (`DISAGREEMENTS.md`, HEAD `ec5cbe4a9`) stays as it was. This file gives the
status of D1-D13 after feeds contract P16 (commit `bd72997c9`), re-checks the "agrees" rows, and
adds D14-D17. `recheck_bd72997c9/assert_recheck.py` checks every number in this file against
its stored source (85/85 at the time of writing).

## Method

The same doors as the first review, through copies of its scripts in `recheck_bd72997c9/`:

- **shipped** and **proposal**: `suggest --json` on the 67 matrix cells (`run_cells.py`,
  `cells.json`), the 25 power cells (`power_cells.py`), and the rivmap100 and nz_wanaka_21
  fixtures (`runs/`). The one change to `run_cells.py`: the rivmap100 `[job.machine]` block is
  found by its header, because P16 (D12) deleted the `slot_doc_factor` line and the block moved
  up one line.
- **gate**: MCP `get_tool_load_report` after `load_project` and `generate_all` (one simulation)
  on the 5 post-sim cells (`postsim.py`, `gate_mcp.json`). G-code from the CLI `project
  --emit-gcode` door (`postsim/<cell>_art/out.nc`).
- Binaries: debug `rs_cam_cli` built from `bd72997c9` (`runs/cli_build.txt`, 21:02:26; no
  change under `crates/` against HEAD at that time). Release `rs_cam_gui` built through
  `scripts/cargo_lane.sh` (`runs/gui_build.txt`, 326 s, linked 21:11:27); the GUI restarted at
  unix 1793952707, after the binary time. Three P17 edits under `crates/rs_cam_core` were saved
  at 21:11:20, 7 s before the link. Core had compiled before them, so the gate numbers are HEAD
  core by inference from the times, not by proof.
- `P_hw_slot` was not rerun: it now reads the same row and cut as `P_oak_slot`.
- The old `assert_claims.py` reads live code lines. At HEAD three of its 58 claims no longer
  derive (`recheck_bd72997c9/old_claims_at_head.py`): `score_hardness_points` (the line moved
  from 1094 to 1508), `trace_finish_k` (moved from 2122 to 2149), `feed_cap_default` (the
  constant is deleted, D9). The other 55 equal their frozen values, because they read the old
  stored outputs.

## Status of D1-D13

Sev as in the first review: A = a safety-side miss or a wrong row; B = a model term that moves
numbers; C = a record or consistency item.

| # | Status | Now (door) | Was | Source of the new number | What remains |
|---|---|---|---|---|---|
| D1 | CLOSED | rivmap100 Face 5, white oak: chip 0.127 (shipped); 6 mm white oak cell also 0.127 | 0.1311 | Spektra prints .005 in = 0.127 (`amana_spektra_spiral_plunge_v24.txt:26`) | none |
| D2 | PARTIAL (first half BLOCKED) | 6.35 mm 2F white oak: row `amana-flat-hardwood-pocket-6350-2f-spektra`, chip 0.127 at 18000 rpm (shipped). The band shows Amana 0.127, Amana 0.1524 and Freud 0.2413 (highest) | `freud-solid-carbide-quarter-hardwood`, 0.2492 at 10026 rpm | the vendor band ships the lowest member (P16 item 8) | The second term (no cross-vendor rule) is closed by the band. The first term (two readings of a missing hardness) is still in the code; it is BLOCKED on the 12x Onsrud/Amana data question (STATUS item 4c0). |
| D3 | PARTIAL | proposal 6.985 mm x 2.32 mm at chip 0.0889 (unchanged). Gate: depth exceeds, budget 5.00 mm at 0.1096 mm/tooth, in both the one-simulation and the two-simulation run | the same gate value in the two-simulation run only; the CLI trace graded 3565 mm/min | one gate feed (P16 item 7): the depth gate reads the emitted feed | The door question is settled: Suggest, the chip gate and the depth gate read one feed. The modulator lifts the deep ring to F4397 (achieved median ratio 1.12, 24 of 24 moves touched), above the Suggest chip, and the force-budget limit does not stop it (R27: modelled width 1.93 mm against the measured 2.00 mm, P16 item 7). New: the Sienci printed pocket (`P_r37_sienci`, 5.6 mm deep) now **exceeds** (budget 5.00 mm); it was within (budget 9.02 mm at the commanded 0.0892 mm/tooth). Suggest sizes the depth at one chip, the modulator ships a larger chip on the same move. A |
| D4 | OPEN | 6.35 mm 2F generic hardwood: 3.393 mm deep (proposal) | the same | - | Sienci prints a 2.5 mm hardwood slot, Carbide 3D 2.54 mm. No P16 item (research: a species factor beyond density). B |
| D5 | PARTIAL | 6.35 mm 2F Baltic birch: no depth proposal; width 5.42 mm = 0.8536 D, binding the corner coverage. Row `amana-flat-plywood-hardwood-pocket-6350-2f-spektra` | 10.58 mm deep, Freud row | P16 item 6c (no depth without a force line) and the band | Plywood still has no force line: shipped power is Unmodeled (`MaterialUnvalidated`), so a stated depth ships with no force or power check. See D17 for the 0.0 in the calculator record. A |
| D6 | PARTIAL | 6 mm 2F hardwood, 0.3 mm wide, 6 mm deep (`P_thin_005`): gate depth exceeds, budget 5.505 mm at 0.0534 mm/tooth (the emitted F1832 of the first ring); the force budget binds a 0.025 share of the moves | budget 1.85 mm at 0.2913 mm/tooth (the commanded lifted feed) | P16 item 7 (one gate feed, the force-budget limit in the modulator) | Part (b) is closed: the gate reads the emitted feed. Part (a) stays: Suggest ships the lift 2.29 on the stated width, and the first ring is a slot. The gate still exceeds by the R27 width term (first ring measured at 5.0 mm). See D14 and D15. A |
| D7 | CLOSED | wanaka River channel: lineage axial kind `surface offset`, value -4.0; no depth proposal; power and removal rate Unmodeled (`DepthAfterSimulation`); warning `DepthJudgedAfterSimulation` | the offset read as an axial depth; calculator depth 0.36 mm | P16 items 2, 2b | none (the calculator shows D as the rpm basis) |
| D8 | CLOSED | 12.7 mm 2F full slot at 3 D: softwood 900 mm/min, hardwood 900 mm/min (shipped), with `PowerLimited` and `PowerLadderReducedCut` | softwood 62, hardwood 5212 | the rubbing-floor feed 0.025 x 18000 x 2 = 900 (P16 item 4) | The power after the rung is still over the 1.125 kW budget (softwood 1.334 kW, hardwood 2.236 kW); the warning says so. Logged follow-up: the modulator power pin. |
| D9 | CLOSED | no default cap: the ceiling is the stated cap, else the X/Y axis rate, else the travel rate (`machine/mod.rs:922-924`, `TravelRate` the last arm). The matrix machine has no kinematics, so its ceiling is the travel rate: 6.35 mm 3F softwood ships 18000 rpm and 6858 mm/min | rpm 15748 under the 6000 cap | P16 D9 | none; a consequence is D15 |
| D10 | CLOSED | wanaka Lake pocket 2 mm: chip 0.0550, row `amana-flat-hardwood-pocket-2381-2f-spektra` (form A between the printed sizes) | 0.0699 | Spektra 1/16 in .002 in = 0.0508 (`amana_spektra_spiral_plunge_v24.txt:12`) and 3/32 in .0023 in = 0.0584 (`:17`) | none |
| D11 | CLOSED | wanaka V60 slit and labels: feed 2286 mm/min at 18000 rpm, chip 0.0635 | 2743 | Amana prints 90 in/min = 2286 mm/min (`amana_ams159_vgroove_v2.txt:11`) | none |
| D12 | CLOSED | no slot cap: 0 of the 67 matrix cells warn `SlottingDetected` (2 cells refuse, as before: ball nose in plywood). The Sienci hardwood slot cell binds the corner coverage at its stated 2.5 mm | the 0.25 D cap | P16 D12 | none |
| D13 | OPEN | not addressed by P16 | the same | - | Thinning on a light finish pass (Harvey against Garr). B |

## The "agrees" rows

| Row | Now | Still agrees? |
|---|---|---|
| Drill and pin drill, 6 mm, white oak | plunge 1778 mm/min (was 1835) | Yes: inside the judged 1370-2286 mm/min |
| Face 18 mm, wanaka | 0.5 mm at 0.4 D, chip 0.1791 (was 0.185), feed 6581 mm/min | Yes on the chip (below the 3/4 in rows 0.229-0.279 mm). The sentence "the feed cap binds first" is no longer true: no cap binds (D9). |
| V rivers, 20 deg 1F 3.175 mm | chip 0.127, feed 2286 mm/min | Yes, unchanged |
| Bull nose 6 mm r 1 mm (R30) | 4572 mm/min at 0.5 mm and 4572 mm/min at 3 mm deep | Yes: no false lift. The row is now the flat Spektra 6 mm row (see D16). |
| Rigid table (Unsourced budget) | the vendor depth table binds at 1 x D (6.35 mm) | Yes, unchanged; every rigid result stays Unsourced |

## New rows

| # | Cell | Engine value (door) | Reference | Missing term | Sev |
|---|---|---|---|---|---|
| D14 | `P_thin_005` (6 mm 2F hardwood, 0.3 mm wide, 6 mm deep) | plunge 4999 mm/min (shipped; half the lifted feed). The G-code plunges 8 mm into the stock at it: `G1 X8.000 Y32.000 Z-6.000 F4999`. The axial chip is 4999 / (17161 x 2) = 0.1457 mm/tooth. The machine reaches only 2402 mm/min on that plunge (gate kinematics) | The chip-thinning lift is a radial term (`CREDITS.md:384-391`); a plunge has no radial engagement. The same tool without the lift plunges at 0.0635 mm/tooth (cell `A_flat_6.000_2f_hardwood`) | **The plunge rate reads the feed before the lift.** Today the plunge is half the lifted feed. Not caused by P16: the first review's cell had the same axial chip (2999 at 10296 rpm); D9 raised the absolute rate. | A |
| D15 | `C_width_0.05` (the D6 cut, Suggest) | feed 9999 mm/min at 17161 rpm (shipped) | was 5999 under the deleted 6000 cap; the machine travel rate is the ceiling now | **The pre-sim lifted feed meets no cap below the travel rate.** After the simulation the modulator brings the first ring to F1832, but 5 moves still emit F9999. The card before the simulation shows the travel rate. Depends on D6 (a). | B |
| D16 | 12.7 mm 2F flat end mill in MDF; 6 mm r 1 mm bull nose in hardwood | the flat tool ships `amana-bull-mdf-pocket-12700-2f-cr`, chip 0.2286 (the flat Spektra member prints 0.2438); the bull nose ships `amana-flat-hardwood-pocket-6000-2f-spektra` | P16 item 8 band rule 1 keeps the existing tool-family filter, which joins flat and bull rows | **A band member of the other cutter family.** Both picks are the lower chip, so the error is on the safe side; the card names a row of another tool family. | C |
| D17 | 6.35 mm 2F plywood; wanaka Project Curve | `profile.calculator.power_kw` prints 0.0 while `profile.shipped_power` says Unmodeled (`MaterialUnvalidated`, `DepthAfterSimulation`) | the K6 rule: an unmodelled value is not a zero | **One record for an unmodelled power.** A reader of the calculator block sees zero load. | C |

## Update (P18, lead ruling 2026-11-06)

- **D15 is CLOSED as "not a cap defect".** The deleted 6000 mm/min cap shipped the thin cut at
  5999 mm/min and 10296 rpm, the same chip as the 9999 mm/min at 17161 rpm of today. The cap
  changed the rpm and the removal rate, never the chip. The diagnosis (`phases/P18.md`, item 4)
  found the cause of the F9999 moves: the 0.5 mm simulation cell does not resolve the 0.3 mm
  stepover, so those real cuts read a width of 0, and the modulator's no-engagement arm keeps
  the commanded feed. P18 lands two changes for it: the no-engagement arm applies the predicted
  kinematic cap, and a finding names a simulation cell that is coarser than stepover / 2.
