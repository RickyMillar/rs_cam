# Assisted SVG trace (including W01b resumed segment)

All actions below are MCP-STATE; PNGs add MCP-VIEW. No pointer/keyboard actions,
unaided user predictions or active task timings were recorded. Screenshots were
read. This log reconstructs the action sequence from tool replies, not a raw
JSON transcript. Build/fixture details: CHECKPOINT.md and ../W00/REPORT.md.

| Step | Goal / action | Actual state / visible feedback | Evidence |
|---|---|---|---|
| 01 | Inspect connected session | No project, idle healthy loop, blank Toolpaths workspace | W00 01_initial.png |
| 02 | Import raw demo_pocket.svg | Model ID0; 70×50 bbox; 80×60×25 auto stock; selected model | W00 02_svg_import.png |
| 03 | Inspect physical stock | Setup stock inspector shows material, size, origin, Auto and padding | W00 03_stock.png |
| 04 | Set stock Z12/origin Z−12 | Auto cleared explicitly in response; Generic Softwood retained | saved baseline TOML |
| 05 | Add explicit review flat Ø6 tool | ID/index0, tool number1, all requested geometry acknowledged | saved baseline TOML |
| 06 | Add Pocket via explicit IDs | ID0 pending; depth5, dpp1.2, feed1515, plunge750, rpm17000 | 01_pocket_geometry.png |
| 07 | Generate (20s wait budget) | 725 moves; 7485.643 mm cutting, 2290.386 rapid; no timeout | MCP reply |
| 08 | Inspect Readiness before sim | REVIEW BEFORE CUTTING, 1/1 computed, Not run, holder Not checked, load unmodeled | 02_generated_readiness.png |
| 09 | Inspect Simulation before sim | Capture metrics OFF, Auto tool-size cell0.25 ON; Ready to simulate | 03_before_simulation.png |
| 10 | run_simulation(resolution=0.25) | MCP turns capture ON, Auto OFF. 361.068s, air37.874% total; critical plunge70/70; top-level OK | W01b metrics-excerpts.json baseline |
| 11 | Read visible results | Green collision/air header, Within1/1; plunge finding not in captured overview | 04_after_simulation.png |
| 12 | Save baseline scratch TOML | Editable state saved; source SVG path absolute | scratch/REVIEW_ONLY_svg_baseline.toml |
| 13 | Inspect Readiness after sim | Up to date, None detected, Within1/1, holder Not checked | 05_simulated_readiness.png |
| 14 | Request export wizard | View mutation acknowledged, but next capture was delayed by interruption | no valid immediate wizard image |
| 15 | Resume at ~13:51 | Newly blank connected instance; previous in-memory job gone; no cause inferred | 06_export_wizard.png actually BLANK |
| 16 | Reload saved scratch | One op restored and auto-generated; no sim; same build metadata | W01b segment starts |
| 17 | Open wizard and capture | Step1 visible with seven-step navigator, GRBL, G21, G54, supported limits; close through MCP | W01b 01_export_wizard.png |
| 18 | Export without simulation/accept flags | REFUSED: chipload/power/deflection SimulationRequired | MCP reply; no NC file |
| 19 | Re-simulate restored baseline at0.25 | Matches step10 metrics and critical plunge exactly | W01b metrics-excerpts.json |
| 20 | Export baseline, no accept flags | Scratch NC written; not a wizard/native-dialog test | W01b scratch/REVIEW_ONLY_NOT_FOR_MACHINING_baseline.nc |
| 21 | Inspect Dressup tab | Path quality controls, 5/8 dressups active; no visible feed toggle in this tab | W01b 02_dressups_before.png |
| 22 | Change only feed_optimization=false | Accepted; toolpath stale and three load diagnostics say re-run | MCP mutation reply |
| 23 | Inspect stale Simulation | Strong stale card/tabs/timeline; old green banner retained; load becomes unmodeled | W01b 03_stale_simulation.png |
| 24 | Regenerate, then simulate at0.25 | Same725 moves and distances; plunge action absent, air58.106%, 505.898s | W01b metrics-excerpts.json counterfactual |
| 25 | Inspect refreshed view | Amber high-air banner, Within1/1, live; renders new time8:26 | W01b 04_after_feedopt_off.png |
| 26 | Save independent variant | Baseline not overwritten | W01b scratch/REVIEW_ONLY_svg_feedopt_off.toml |

## Interpretation boundaries

- Successful import/add via MCP does not test real menu availability/default IDs.
- Metrics-capture defaults were changed by the MCP simulation path.
- No collision clearance conclusion follows from zero counts at a fixed cell;
  radial/air/chip measurability was degraded with blind_fraction0.130307.
- The before/after counterfactual is not a complete safe recipe and the plunge
  finding's explanatory causal sentence was not independently verified.
- The physical time estimate is not tool-call latency. The four-hour gap between
  capture groups is an interruption, not a stuck simulation/generation.
- No export override, production destination, library mutation or machine action.
