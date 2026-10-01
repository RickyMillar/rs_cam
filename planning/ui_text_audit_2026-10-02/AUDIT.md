# UI text-density audit — 2026-10-02

- **Status:** Phase 1 complete. Read-only; no code changed.
- **Decision needed:** approve the placement contract (face headline, hover
  detail, developer text to MCP or log) and the Phase 2 order at the end.
- **Evidence:** code reading and two tracing agents. Nothing was run.
- **Next action:** Phase 2 step 1, the bug repeats (WRONG #1, #2, #4, #11).

Phase 1 (read-only). Branch `verbose-text-audit`, base master `4611ef16`.
Operator request: find the very verbose text in the UI, decide whether the user
needs it, and propose a better presentation. The pattern the operator liked is
the compact legend (branch `ui-compact-legend`, not merged): a short label on
the face and the detail on hover.

The reader is a CNC hobbyist with a 3-axis wood router. The honesty rule
stands: a caveat that changes a decision stays reachable. This audit moves
text; it does not delete truth.

## Method and limits

- A script extracted every string literal of 80 characters or more from
  `crates/rs_cam_viz/src` (test modules and test files excluded). The appendix
  lists them, with a context tag: FACE, HOVER, TOAST or `?` (not classified).
- Two read-only agents traced core-produced strings to their GUI render sites,
  and read the face text of the Simulation, Readiness, export, optimize and
  toast surfaces. Text built from many `format!` pieces does not show in the
  literal scan; the agents read the render functions for those.
- Nothing was run. Frequency comes from code reading, not from a live GUI.
- "Verified" in the tables means I read the source lines myself. "Reported"
  means an agent read them and I did not re-read them.

## Classes and treatments

| Class | Meaning |
|---|---|
| ACTION | Tells the user what to do. |
| DECISION-CAVEAT | Changes how the user must read a number. |
| EXPLANATION | Teaches how the product works. |
| DEVELOPER | Ids, formulas, grid cells, ruling names (`ruling B6`, `G10`, `Q12`), ticket codes, fn or enum names, fixture anecdotes. |
| DUPLICATE | The same information shows elsewhere, or twice in one place. |

| Treatment | Meaning |
|---|---|
| KEEP | Keep as is. |
| LABEL+HOVER | One short line on the face; the rest on hover. |
| ⓘ | Move the whole text to an ⓘ hover. |
| DETAILS | Move to a collapsed "Details" disclosure (`SummaryCard`). |
| DROP→MCP/LOG | Developer-only. Keep it in MCP, the CLI or the log; remove it from the GUI. |
| DEDUPE | Show it once. |

## The main finding: core prose goes to the face verbatim

The worst text is not written in the UI. Core builds a full paragraph, and the
GUI prints it as one label. Three sinks carry most of the volume:

1. **The inspector diagnostic ribbon.** `ui/properties/tab_badges.rs:226`
   prints `"{Category}: {d.message}"` for every `Diagnostic`, wrapped, on the
   face (Caution, Critical, Blocking and State rows; Info rows go under a
   collapsed "N hints" header). Core messages are up to about 900 characters.
   Some end in `[Report-only — no gate.]` tags, name crates, name MCP tools, or
   concatenate a headline and a detail that repeat each other.
2. **The Simulation verdict line.** `ui/sim_diagnostics.rs:155-185` takes the
   first safety, action or advisory `diagnostic.message` and prints it as the
   page headline, with no length limit.
3. **The Feeds card face lines.** `ui/feeds/why.rs` prints core `card_text()`
   headlines (plunge, size, family, drill and hardness claims, ramp records).
   These carry codes such as `G10`, `G1 size`, `G6 drill`, and long
   parentheses.

The `Diagnostic` struct has `message` and `evidence`, but no headline/detail
split. The MCP and CLI read `message` too, so a fix must keep the full text
for agents.

**Proposed approach: a placement contract, not per-string edits.**

- The face carries one headline of about 60 characters or fewer: what happened
  and, where one exists, the action.
- The hover or a "Details" disclosure carries the detail and the evidence.
  Every DECISION-CAVEAT stays here or on the face. Nothing that changes a
  decision is removed.
- Ruling codes, `G`-rule codes, fn names, enum `Debug` output, MCP tool names,
  fixture anecdotes and `[Report-only — no gate.]` tags go to MCP, the CLI or
  the log only.
- Core supplies the parts, not one string: `Diagnostic` gets a one-line
  `message` (the headline) and a `detail: Option<String>`. The MCP and CLI
  print both, so agents lose nothing. Core `card_text()` pairs already have
  this shape; only their headlines need to get shorter.
- A source-scanning sentry can hold the contract: no face string from these
  sinks over N characters, and no `ruling [A-Z]\d`, `G\d+ ` rule code or `:?}`
  in a face string.

This matches the repo preference "consolidate, don't patch". One contract fixes
the ribbon, the verdict line and the card together.

## Per-surface summary

| Surface | Long items | Worst item | Class mix | Proposal |
|---|---|---|---|---|
| Inspector diagnostic ribbon (`tab_badges.rs:226` + core adapters) | ~30 core messages | `from_generation.rs:252` offset-failure, ~900 chars, names `cavalier_contours` and `debug_assert!` | DEVELOPER, DUPLICATE, some DECISION-CAVEAT | Headline/detail split in core; ribbon shows headline, hover shows detail + evidence |
| Inspector reach footer (`properties/toolpath_panel.rs:370-398`) | 3 lines | area basis ×2 and over-statement ×2 | DECISION-CAVEAT, DUPLICATE | One line `≤ x % unreachable · worst gap y mm`; grid detail on ⓘ |
| Viewport legend (`overlays/legend_rail.rs`) | reach block ~5 lines | same reach repeat as above | DECISION-CAVEAT, DUPLICATE | Merge `ui-compact-legend`, then dedupe its hover |
| Feeds card (`feeds/why.rs`, `feeds/compare.rs`) | ~60 strings | aggressiveness face line, plunge claim headline with `G10` | DEVELOPER, EXPLANATION, DUPLICATE | Short face lines (ruling R4 keeps one line per stage); codes and formulas to hover |
| Simulation workspace (`sim_*.rs`) | ~20 | stale notice ×4 on one screen; verdict line = raw core message | DUPLICATE, DEVELOPER | One stale banner; cap the verdict line; hovers without fn names |
| Readiness / pre-flight / export wizard | ~15 | cycle-time caveat ×3 surfaces + hover; enum names on gate lines | DECISION-CAVEAT, DUPLICATE, DEVELOPER | One short inline caveat + hover; plain gate words |
| Optimize modal and project optimize | ~15 | "verify on a scrap" ×3; provenance grid with `{:?}` | DUPLICATE, DEVELOPER | Dedupe narrative; provenance grid into closed Details |
| Toasts (`controller/events/*.rs`, `compute/mod.rs`) | ~10 | failed-sim toast, 4 sentences; `{w:?}` debug | EXPLANATION, DEVELOPER | One sentence + action; numbers and paths to log |
| Op-editor hovers (`properties/operations/*.rs`, `linking_dressup.rs`) | ~40 | `surface_3d.rs:937` Monotone Cells, 836 chars | EXPLANATION, DEVELOPER | Shorten hovers to 2-3 sentences; drop measurement history |
| Multi-tool planner (`multitool_planner.rs`) | ~25 | `:322` 714-char hover; `:155` 547-char hover | EXPLANATION | Shorten hovers; the face text is mostly fine |
| Overlay registry hovers (`overlays/registry.rs`) | ~19 | `:549`, `:566`, `:623` change history ("before P6 …") | DEVELOPER | Drop history sentences |
| Preferences (`preferences.rs`) | ~7 | `:1115` MCP note | EXPLANATION | Low priority; rarely visited |

Already clean: `ui/toolpath_panel.rs`, `ui/toolpath_row_controls.rs`,
`ui/setup_panel.rs` and `ui/status_bar.rs`. They use a short word plus a
hover (for example `STALE` with a hover at `ui/toolpath_panel.rs:1058`). Use
them as the model.

## Ranking rule

Impact = how often the user sees it × how much space it takes × how much it
confuses. FACE text ranks above HOVER text. The operator approved hover as the
place for detail, so a long hover is a problem of length only. A three-line
face paragraph seen every session is the real cost.

## Top 25

Paths are relative to `crates/`. V = verified by me, R = reported by an agent.

| # | Where (file:line) | Current text (truncated) | Face? | Class | Proposal | Proposed short label | V/R |
|---|---|---|---|---|---|---|---|
| 1 | `rs_cam_core/src/diagnostics/adapters/from_feeds.rs:69-72` (+ `feeds/rationale.rs:284-296`), shown at `rs_cam_viz/src/ui/properties/tab_badges.rs:226` | "Depth per pass 6.350 -> 4.325 mm: aggressiveness 0.85 x long-tool share 0.75 = load target 63.7 % aggressiveness 0.85 x long-tool share 0.75 = load target 63.7 % of the base engagement (common scale …); force …, power …. The chipload does not change. The plunge does not take the dial…" | FACE | DUPLICATE (bug) + EXPLANATION | Fix the repeat (see WRONG #1). LABEL+HOVER. | `Depth 6.35 → 4.33 mm (load 64 %)` | V |
| 2 | Ribbon sink `tab_badges.rs:226` for all core messages; worst: `from_generation.rs:85-100`, `:252-266`, `:419-428`, `:488-506`, `:548`, `:603`, `:722` | "Rest-region extraction found {total} islands and kept only the largest {kept} (cap {cap}); {dropped} were DROPPED. Anything using these regions as a `derived_rest_regions` boundary … [Count of grouped region polygons before truncation; extraction stage. Report-only — no gate consumes this.]" | FACE | DECISION-CAVEAT + DEVELOPER | Core headline/detail split; drop bracketed tags and field names from the GUI text | `{dropped} rest islands dropped — raise Min Valley Depth` | V (`:85`), R (others) |
| 3 | `rs_cam_core/src/session/cycle_time.rs:99-113`, printed at `rs_cam_viz/src/ui/readiness_panel.rs:235`, `ui/preflight.rs:186`, `ui/export_wizard.rs:1008` (+ hover `:971`) | "Cutting distance ÷ nominal feed. Excludes rapids, acceleration and per-move feed changes. On a corner-heavy 3D finish this reads SEVERAL TIMES faster than the machine — a measured job read 25 min against 3 h." + remedy line | FACE ×3 | DECISION-CAVEAT + DUPLICATE + DEVELOPER (anecdote) | Keep it inline (the code says it must not be hover-only), but one short line + hover. Drop "25 min against 3 h". Do not print the remedy when the row title already names the cause. | `Rough estimate — the real cut can take several times longer ⓘ` | V |
| 4 | Stale notices on one workspace: `ui/sim_op_list.rs:276-285`, `ui/sim_diagnostics.rs:243`, `ui/sim_timeline.rs:58`, `ui/sim_diagnostics.rs:1625`; Readiness `readiness_panel.rs:143` + row `preflight.rs:337` | "⚠ Results are stale" / "{Reason}. Run the simulation again." / "⚠ Results stale ({reason}) — run the simulation again" / "The cut trace is from an earlier version of the project. Run the simulation again…" | FACE ×4 | DUPLICATE | DEDUPE: one `FreshnessGate` banner per workspace (op list); other panels dim and use a `STALE` chip with hover | `STALE` chip; banner: `Stale — {reason}` + `Re-run` | V (5 sites), R (`preflight.rs:337`) |
| 5 | `ui/sim_diagnostics.rs:155-185` (verdict line) + hover `:147` | "{glyph} {n} safety — {first.diagnostic.message}…"; hover "From ProjectSession::simulation_triage — the same answer the CLI report, the MCP get_diagnostics block and narration read…" | FACE (page headline) | EXPLANATION + DEVELOPER | Headline from the split `message` only; hover = detail. Drop the fn/MCP hover. | `⚠ 2 safety — Holder hits stock on "Finish"` | V |
| 6 | Reach footer `rs_cam_viz/src/ui/properties/toolpath_panel.rs:370-398`, notes from `rs_cam_core/src/maps/reach_map.rs:550-590` | "unreachable 15.2 % of 3D surface area, rim-eroded 1.6 mm · max gap …" / "cell 1.526 mm · floor 0.567 mm · tol 0.100 mm · of 3D surface area, rim-eroded 1.6 mm — the bar is UNDER the floor: the grid over-states gaps…" / the over-statement sentence again | FACE | DECISION-CAVEAT + DUPLICATE (bug) | Fix the repeat (WRONG #2). Face: one line with `≤`. Hover/ⓘ: grid line + over-statement once. Caution glyph when tol < floor. | `≤ 15.2 % unreachable · worst gap 0.84 mm ⚠` | V |
| 7 | Legend reach block `rs_cam_viz/src/ui/overlays/legend_rail.rs:563-595` (master) | same as #6 (the operator's quoted paragraph) | FACE (master) | DECISION-CAVEAT + DUPLICATE | Merge `ui-compact-legend` (chip `≤ x % unreachable`, detail on hover). That branch still pushes `grid_note` and `over_statement_note` both (its `:1166`, `:1174`): dedupe in its hover. | `Reach #3 · ≤ 15.2 % unreachable` | V |
| 8 | `rs_cam_viz/src/ui/feeds/why.rs:997-1052` (aggressiveness face line) | "Aggressiveness 0.85 (× 0.75 long tool = 64 %): depth 6.35 → 4.33 mm, stepover 3.00 → 2.10 mm; force … N, power … kW, chip section … mm² (proxy, no measured force line). Apply the cut geometry to hold the load at 64 %" | FACE | EXPLANATION | Ruling R4 needs one face line per stage; keep the line, shorten it, and add a hover (`plain_line` has none; use `detail_line`). Force/power/section go to the hover. | `Load 64 %: depth 6.35→4.33, stepover 3.00→2.10 mm` | V |
| 9 | Plunge basis face line `feeds/why.rs:586-588`, text `rs_cam_core/src/feeds/plunge.rs:161-202` | "Plunge = min(0.50 x feed, tip cap 900 mm/min) (G10 plunge claim: ball nose, Sienci, Amana, 3.175-25.4 mm, 2 flutes)" / "Plunge: no source; repo rule material base 1000 mm/min (…)" | FACE | DEVELOPER + DUPLICATE (the Plunge row hover prints it again, `why.rs:280-284`, R) | LABEL+HOVER; vendors, key, flutes, rule text and `ruling Q12` to hover | `Plunge = ½ feed (vendor rule)` / `Plunge 1000 mm/min (no vendor source)` | V (text), R (row-hover repeat) |
| 10 | Claim lines `feeds/why.rs:619-648`, text `feeds/extrapolation.rs:236`, `extrapolation/family.rs:201`, `extrapolation/drill.rs:190` | "extrapolated (G1 size): x0.68 from the 6.0 mm row" / "vendor row, family transferred (G3 family): the X row serves this Y pass" | FACE | DEVELOPER (codes) + DECISION-CAVEAT | Keep the line (it changes trust in the number); drop the `G1`/`G3`/`G6` code from the face | `Scaled ×0.68 from the 6 mm chart row` | V |
| 11 | `rs_cam_viz/src/ui/preflight.rs:608-650` | "TP 4: advance/tooth: EXCEEDS (ChiploadBurnRisk) · power: EXCEEDS (SpindlePowerExceeded) · deflection: EXCEEDS (LongToolStiffnessUnsafe)" | FACE (export) | DEVELOPER | Plain words + toolpath name; enum to hover/MCP | `"Finish": feed too low (burn risk) · over spindle power` | V |
| 12 | `rs_cam_core/src/diagnostics/adapters/from_feeds.rs:360-364` | "No vendor data for {op} on a {family} cutter — this recommendation is entirely formula-derived and carries no band, and the post-simulation chipload gate will report Unmodeled(NoVendorData) for the same reason. Missing: {rows}." | FACE (Caution) | DECISION-CAVEAT + DEVELOPER | LABEL+HOVER; drop the enum name | `No vendor data — feeds are formula-only ⓘ` | V |
| 13 | `rs_cam_core/src/tool_load/optimize/narrative.rs:918-931` + `rs_cam_viz/src/ui/optimize_modal.rs:271`, `:749-757` | heading "Verify on a scrap"; headline "Best candidate is admitted only by the layer-1 tolerance band — verify on a scrap before applying."; explanation starts with the same sentence | FACE | DUPLICATE ×3 + DEVELOPER ("layer-1") | Headline once; explanation without the restated sentence; `narrative_prose` dedupes only exact matches | `Within tolerance only — test on scrap first` | V (headline, dedupe fn), R (explanation start, heading `:271`) |
| 14 | `rs_cam_core/src/tool_load/verdict.rs:1151-1160` (vacuity clause) appended to gate messages; joined at `rs_cam_viz/src/ui/sim_diagnostics.rs:1265` | " — VACUOUS: this verdict rests on 0 of {n} {units} (all filtered out); it is not a measurement of a clean cut" → "{kind} reports {state:?} but — VACUOUS: …" | FACE + HOVER | DECISION-CAVEAT + DEVELOPER (`{:?}`) | Replace with the existing `NotMeasured` (`—`) and a reason hover | `— (no steady cut to measure)` | V (join), R (clause) |
| 15 | `rs_cam_core/src/diagnostics/adapters/from_tool_load.rs:271-350` (+ `:124-168`) | "Feed-per-tooth too low — burn / rubbing risk: 0.0312 mm [p50, mm/tooth; commanded …, ×… achieved/commanded feed] (row amana_…; scaled ×… diameter × ×… hardness from Ø… mm; filed as X/Y and carried … by the G3 family rule (…))" | FACE (Caution) + sim verdict line | DECISION-CAVEAT + DEVELOPER | Headline = side + value + band; provenance clause to hover (the LutCitation evidence hover already holds the row) | `Feed per tooth too low (0.031 < 0.05 mm) — burn risk` | R |
| 16 | `rs_cam_core/src/session/compute/diagnostics.rs:1140`, `:1180-1191`, `:1297-1301` via `from_project_diagnostics.rs:44-48` | "WARNING: rapid collisions on TP{id} … — Measured: a rapid (G0) move passes through stock … MCP inspect_collisions gives each move…" / "… The project_curve_negative_depth validator rule flags this … the project_summary.stale_defaults entry has a one-click flip-sign fix." | FACE (Safety, verdict line) | ACTION + DEVELOPER | Headline + action; MCP names out of GUI text; drop "WARNING:" after the "Safety:" prefix | `Rapid moves pass through stock on "Rough" — raise Safe Z` | V (`:44-48`), R (texts) |
| 17 | `rs_cam_core/src/diagnostics/adapters/from_model_refs.rs:70-73` | "Selected model is missing — toolpath references model_id 3 but no loaded model has that id. Call `inspect_model` and set the toolpath's model to a valid `id` from the response." | FACE (Blocking) | ACTION for agents, wrong audience | GUI headline + GUI action; keep the MCP sentence for MCP | `Model missing — pick a model in the Geometry tab` | V |
| 18 | `rs_cam_core/src/diagnostics/adapters/from_static_checks.rs:416-422` | "Depth per pass (12.0 mm) is 2.0× the tool diameter (6.0 mm). The vendors print the depth de-rate to 1× diameter only. The feed and the chipload band hold the last printed point, 50 %." | FACE (Caution) | DECISION-CAVEAT | LABEL+HOVER | `Depth 2× tool Ø — beyond vendor charts` | R |
| 19 | `rs_cam_core/src/diagnostics/adapters/from_feeds.rs:256-259` + `rs_cam_viz/src/ui/feeds/why.rs:834-839` + rationale Feed-row hover | "Advance per tooth below the rubbing floor: 0.0200 mm/tooth (floor 0.0300, repo rule, unsourced). The feed is not raised. The tool can rub and burn the work: raise the feed or lower the RPM." | FACE ×2 + HOVER | ACTION + DUPLICATE | Keep it on the Feeds card (where the fix is); the ribbon gets the headline only; one hover copy | `Feed per tooth below rubbing floor — raise feed or lower RPM` | R |
| 20 | `rs_cam_viz/src/ui/sim_diagnostics.rs:1021`, `:352-364`, `:922` | "m1234 · waste 3.21s · peak commanded a/t 0.0412 mm/tooth" / "moves a–b · n samples · peak DOC x" / "… +{n} more (open Selected for span-scoped list)" | FACE | DEVELOPER | Plain words; coordinates and sample counts to hover | `Move 1234 · 3.2 s wasted · peak 0.041 mm/tooth` | R |
| 21 | `rs_cam_viz/src/ui/optimize_modal.rs:377-600` ("How these numbers were taken", open by default) | "not stamped — this result predates the machine snapshot, or was built by a constructor that never ran a search" / "Vendor LUT query {Family:?}/{Role:?} → …" / "Boundary epsilon {:.3e} relative" | FACE | DEVELOPER | DETAILS closed by default; `{:?}` to plain labels | `How these numbers were taken ▸` (closed) | V (`{:?}` sites), R (open default) |
| 22 | `rs_cam_viz/src/controller/events/simulation.rs:62-66` | "Simulation {outcome}. The viewport released the previous simulation when this run started, so it shows no simulated stock. Rest operations still use the previous simulation. Run the simulation again to see the stock." | TOAST | EXPLANATION + DECISION-CAVEAT | One sentence + the caveat that matters | `Simulation {outcome} — stock not shown; rest ops use the last run` | R |
| 23 | `rs_cam_viz/src/ui/sim_diagnostics.rs:1387-1412`, `ui/preflight.rs:590`, `rs_cam_core/.../from_tool_load.rs:983-988` | "Unmodeled: this material has no measured force line (ruling B6)" / "… no vendor LUT row for this tool/material combination" | FACE | DECISION-CAVEAT + DEVELOPER | Drop "(ruling B6)", say "vendor chart" not "LUT" | `Not modelled: no force data for this material` | V (viz), R (core) |
| 24 | `rs_cam_viz/src/ui/optimize_project.rs:82-85`, `:648` | "Stage 0/1/2 search across every enabled toolpath. Expect 3–10 minutes on a wanaka-sized job. … click Cancel to stop and discard the partial run." / `xtp` suffix | FACE | DEVELOPER | Drop "Stage 0/1/2", "wanaka-sized"; spell out `xtp` | `Searching every toolpath — a few minutes. Cancel stops it.` | V (`:82`), R (`:648`) |
| 25 | `rs_cam_viz/src/ui/properties/operations/surface_3d.rs:937-950` (+ `multitool_planner.rs:322`, `:155`, `:307`, `:259`) | "Split each SHALLOW region into monotone cells on that region's own raster lattice, rotating the lattice to the region's PCA-minor axis where its elongation clears 3.0. … On by default since 2026-09-01 (C4 operator surface review passed) … 1.155x across the reference relief's top three shallow regions…" | HOVER (836 chars) | EXPLANATION + DEVELOPER | Shorten to 2-3 sentences: what it does, when to turn it off; history and rig figures to the planning record | `Cut shallow areas in simple cells for fewer links. Check the finish at the cell seams.` | V |

Next 10 below the cut, in short: the failed-memory toasts (`compute/mod.rs:306`,
`:340`, R); the export wizard M0 step (`export_wizard.rs:676`, 4 face lines);
"No PostLimits set" (`export_wizard.rs:1103`, type name, R); "Save is blocked"
said 3 times (`export_wizard.rs:848`, `:1029`, `app/export.rs:240`, R); the
"EXCEEDS — below band" verdict (`properties/feeds_speeds.rs:500`, V); the
power/efficiency "not modelled" hovers that explain the design decision and
print `{reason:?}` (`feeds/compare.rs:809`, `:1065`, V); the overlay registry
change-history hovers (`overlays/registry.rs:549`, `:566`, `:623`, `:1035`,
`:1051`, V); the stale-default ribbon rows that cite "pre-Roadmap-B.1" and
"Fix 1" (`compute/validate.rs:230-311`, R); the feeds-row hovers that cite
`ruling A1/B4/R4` (`feeds/why.rs:209`, `:532`, `:536`, `:598`, `:611`, V); the
multi-tool planner face lines (`multitool_planner.rs:135`, `:383`, `:515`,
`:529`, `:637`, V).

## WRONG text and text repeated by a bug

| # | Where | Defect | Status |
|---|---|---|---|
| 1 | `rs_cam_core/src/feeds/rationale.rs:284-296` + `:325-331`; joined at `diagnostics/adapters/from_feeds.rs:69-72` | The headline ends with `{target}` ("aggressiveness 0.85 x long-tool share 0.75 = load target 63.7 %"). The detail starts with the same `{target}`. `from_feeds` joins them with one space, so the ribbon prints the phrase twice in a row. This is the operator's quoted example. The Feeds row hover (`feeds/why.rs:460-462`) prints headline and detail on two lines, so it repeats there too. The depth row and the stepover row carry the same detail, so one ribbon can show the whole paragraph twice. | Verified |
| 2 | `rs_cam_core/src/maps/reach_map.rs:576-590` | `grid_note` already contains `area_basis_note`, and, when tol < floor, `over_statement_note`. Both GUI sites then print `area_basis_note` on the line above and `over_statement_note` again on the line below: `properties/toolpath_panel.rs:376-398` and `overlays/legend_rail.rs:566-595`. The `ui-compact-legend` branch keeps the repeat inside its hover. | Verified |
| 3 | `rs_cam_viz/src/controller/events/compute.rs:766-768` | "Sim resolution was coarsened to fit grid limits — consider reducing stock size or increasing resolution". "Increasing resolution" reads as "finer", which makes the grid larger. The fix is a larger cell (coarser) or a smaller stock. | Verified (wording); the intended advice needs the owner's check |
| 4 | `rs_cam_viz/src/ui/sim_diagnostics.rs:1268` | `"{} reports {:?} but{vacuity}"` with a clause that starts " — VACUOUS: …" gives "Power reports Within but — VACUOUS: …", and prints a Rust `Debug` value. | Verified |
| 5 | `rs_cam_viz/src/ui/properties/tab_badges.rs:457`, `:467`; `from_generation.rs:85-100` | The text says "raise min_valley_depth" (and `min_rest_depth_mm`). The UI label is "Min Valley Depth:" (`properties/toolpath_panel.rs:1027`). The user cannot find a field by its code name. | Verified |
| 6 | `rs_cam_core/src/diagnostics/adapters/from_model_refs.rs:70-73` | A GUI Blocking row tells the user to "Call `inspect_model`" — an MCP agent instruction. | Verified |
| 7 | `rs_cam_viz/src/ui/properties/feeds_speeds.rs:500-501` | "EXCEEDS — below band (burn/rubbing)". "Exceeds" for a value under the band reads as "too high". | Verified |
| 8 | `rs_cam_viz/src/ui/optimize_project.rs:84` vs `ui/optimize_modal.rs:94` | The project search says Cancel will "discard the partial run". The per-toolpath search says Cancel will "keep partial results". These are two flows, so both can be true. Check which one each flow does before the rewrite. | Inconsistent; not verified which is true |
| 9 | `{:?}` in user text: `ui/sim_op_list.rs:1336`, `ui/optimize_modal.rs:473`, `:584-597`, `ui/sim_diagnostics.rs:1268`, `ui/sim_timeline.rs:1001`, `ui/export_wizard.rs:863`, `ui/properties/feeds_speeds.rs:507`, `ui/properties/model_sim_panels.rs:23`, `ui/properties/operations/boundary_2d.rs:350`, `feeds/compare.rs:1065`; toasts `controller/events/mod.rs:1102-1108`, `controller/io.rs:309` | Rust `Debug` output (`NotApplicableForOp("…")`, `MaterialUnvalidated`, enum names) is shown to the user. | Verified (viz list), R (toasts) |
| 10 | `rs_cam_core/src/maps/reach_map.rs:179-197` (`ReachToleranceSource::describe`) | Reported: printed beside `grid_note`, which already has "tol x mm", so the tolerance shows twice. | Reported, not verified |
| 11 | Plunge headline vs its detail, `rs_cam_core/src/feeds/plunge.rs:192-199` | The MaterialBase headline "Plunge: no source; repo rule material base …" repeats the start of its own detail (`PLUNGE_BASE_RULE_TEXT`: "no source; repo rule: material base …"). | Verified |
| 12 | Rationale keyword routing, `rs_cam_viz/src/ui/feeds/why.rs:435-462` | An entry is appended to every row whose keyword its headline contains, so one entry ("Plunge clamped to feed") prints in both the Feed and the Plunge hovers. | Reported, not verified |

## Components: what fits and what is missing

Fits already:

- `StatusChip` (`components/chip.rs`) with `.hover()`. This IS the "caveat
  chip": `StatusChip::new("≤ 15 %", Role::Caution).hover(detail)`. No new
  component is needed.
- `NotMeasured` (`components/notice.rs:396`) with `.reason()`. Use it for
  every "VACUOUS" and "unmodeled (…)" face string.
- `EmptyState` with `.detail()` and `.action()`. Use it for the long empty
  states (`sim_op_list.rs:236`, `:249`).
- `SummaryCard` (`components/section.rs:75`), a collapsing header. This is
  the "Details" disclosure for the optimize provenance grid.
- `ValueRow` with `.tooltip()` and `.note()`.
- `FreshnessGate::banner` — the one stale cue. It is called from three panels
  of one workspace; the fix is to call it once, not a new component.

Gaps (small additions, not new components):

1. `Notice` and `Banner` have no detail slot. The `Notice` doc says "an
   optional detail", but the struct has none (`notice.rs:42-50`). Add
   `.detail(String)` shown on hover. The toast stack, the inspector cautions
   and the sim advisories then carry headline + detail without new code.
2. `KeyValueRow::trailing` is a plain `String` with no hover. Add
   `.hover(String)` so a row can carry a caveat chip.
3. `feeds/why.rs::detail_line` (a headline with a hover) is the right line
   renderer. `properties/toolpath_panel.rs` (`wrapped_small_label`) and
   `tab_badges.rs:226` hand-roll the same thing. Lift `detail_line` into
   `components/` as the one "headline + hover" line, and give
   `draw_suggest_lines` a hover (it uses `plain_line`, so a shortened face line
   has nowhere to put its detail today).
4. Core: add `detail: Option<String>` to `Diagnostic`, and keep `message` to
   one line. The MCP and the CLI print both.

Do not add more than this. The kit rule is one renderer per element.

## Constraints a Phase 2 must respect

- Ruling R4 (feeds): every stage that moves a number is one line on the Feeds
  card. Shorten the line; do not move it off the face. Sentry
  `every_stage_that_moves_a_number_is_on_the_card_g_visible`.
- The cycle-time caveat is inline on purpose (`readiness_panel.rs:232`,
  `preflight.rs:184`). Keep a short inline line; the long text may go to hover.
- The reach map is an UPPER estimate. A face label must keep `≤`.
- `ui_string_hygiene` and the feeds sentries match on some strings. A text
  change can need a sentry update in the same commit.
- MCP and CLI read core `message` text. A core split must keep the full text
  available on those surfaces (`app/mcp/*.rs`).
- The operator's surface-parity rule: one state gives the same numbers on
  every surface. Shorter words are fine; different numbers are not.

## Suggested Phase 2 order

1. Fix the bug repeats (WRONG #1, #2, #4, #11) and the wrong words (#3, #5,
   #6, #7). Small, core-local, high value.
2. Merge `ui-compact-legend`, then dedupe its reach hover.
3. `Diagnostic.detail` in core + headline-only ribbon and verdict line. Move
   the `[Report-only …]` tags, crate names and MCP instructions out of GUI
   text.
4. Stale banner once per workspace; cycle-time caveat to one line + hover.
5. Feeds card face lines: drop codes, shorten the aggressiveness and plunge
   lines, add hovers.
6. `{:?}` sweep and enum names on the pre-flight gate lines.
7. Hover length pass (op editors, planner, registry history).

## Appendix: every extracted literal of 80+ characters

The 294 rows are sorted by length. The tag comes from a 5-line context
heuristic, not from a verified render site. The per-surface table above is the
grouped view. Scope: `crates/rs_cam_viz/src`, UI, controller events and export only; MCP
server, render shaders and test files removed. Tag = context guessed from the
5 lines around the literal (FACE, HOVER, TOAST, `?` = not classified). Length
is in characters. Text is truncated at 110 characters.

| Tag | Len | file:line: text |
|---|---|---|
| HOVER | 836 | ./ui/properties/operations/surface_3d.rs:937: Split each SHALLOW region into monotone cells on that region's own raster lattice, rotating the lattice to th |
| HOVER | 714 | ./ui/multitool_planner.rs:322: Each tier's SHALLOW band splits every region into monotone cells on that region's own raster lattice, and rot |
| HOVER | 547 | ./ui/multitool_planner.rs:155: Which operation cuts this tool's tier. The tier's TERRITORY is the same whichever you pick — regions come fro |
| HOVER | 417 | ./ui/feeds/compare.rs:1040: Power {:.3} kW of {:.3} kW available \u{2014} {} of the limit.\n\n Limit from {}. Setting: {}.\n\n Evaluated a |
| HOVER | 413 | ./ui/feeds/compare.rs:1065: Power is not modelled at this operating point.\n\n {} \u{2014} so feeds::power_at_operating_point refuses with |
| HOVER | 409 | ./ui/multitool_planner.rs:307: Tier 0 leaves the fine tiers' islands uncut instead of sweeping the whole board — a finer tool re-finishes th |
| HOVER | 398 | ./ui/feeds/compare.rs:809: Efficiency is not modelled for this material.\n\n The energy-per-mm\u{00B3} model needs a force line, and this |
| ? | 391 | ./controller/events/compute.rs:149: '{toolpath_name}' is waiting on simulated stock after '{blocker_name}' (index {blocker_idx}), which has not g |
| HOVER | 350 | ./ui/feeds/explore.rs:130: Push RPM up to the spindle ceiling ({:.0} RPM × {:.0}% headroom), scaling feed proportionally to keep the co |
| HOVER | 342 | ./ui/multitool_planner.rs:259: How far each fine tier reaches past its own territory into the coarser tier's, so the seam blends two cusp pa |
| HOVER | 327 | ./ui/overlays/registry.rs:992: The regions that the selected 3D Rough detected with By Area ordering. Each region has one colour and one ord |
| HOVER | 324 | ./ui/properties/machine_panel.rs:508: The load of a Suggest recipe as a fraction of the load at full engagement. The chipload stays in the vendor b |
| HOVER | 314 | ./ui/feeds/compare.rs:719: Chipload {:.4} mm/tooth — the recommendation's commanded advance per tooth, feed \u{00F7} (RPM \u{00D7} flute |
| HOVER | 304 | ./ui/feeds/compare.rs:1166: Overwrite RPM, feed, and plunge (how fast) with the recommended values, after the safety clamps, so the chipl |
| HOVER | 300 | ./ui/multitool_planner.rs:694: What this tier's tool actually sweeps: its islands grown by the {:.2} mm overlap band, which reaches into the |
| HOVER | 284 | ./ui/properties/operations/surface_3d.rs:611: Rings from the iso-scallop field instead of the offset cascade: spacing varies point-by-point along each ring |
| HOVER | 284 | ./ui/multitool_planner.rs:447: Hands fine-tier cells within this distance of the part edge back to the coarse tool. Off by default: near the |
| HOVER | 281 | ./ui/properties/toolpath_panel.rs:308: Colour the model by what THIS toolpath's cutter can form: green where the cutter reaches the surface within t |
| HOVER | 272 | ./ui/properties/operations/surface_3d.rs:706: Derive rest from the design surface: where can this cutter not reach the model. Right for a FIRST finish pass |
| HOVER | 270 | ./ui/multitool_planner.rs:239: Region coarseness. Scales the merge radius and the minimum island area together from each tier's own tool-der |
| HOVER | 259 | ./ui/sim_diagnostics.rs:1028: Achieved advance per tooth = effective feed \u{00f7} (RPM \u{00d7} flutes), where effective feed is the machi |
| HOVER | 258 | ./ui/feeds/compare.rs:1140: Overwrite RPM, feed, and plunge (how fast) AND DOC/WOC (the cut) with the recommended values, after the safet |
| HOVER | 256 | ./ui/properties/operations/boundary_2d.rs:329: Cut full-width seeding slots before the adaptive passes. A slot line runs at full radial immersion, the one 2 |
| ? | 254 | ./controller/events/compute.rs:123: '{toolpath_name}' uses remaining stock (rest machining) but is the first enabled operation in its setup — the |
| HOVER | 252 | ./ui/properties/toolpath_panel.rs:849: The toolpath whose rest analysis supplies the rest regions. ANY operation produces them: picking it here swi |
| ? | 251 | ./ui/feeds/compare.rs:792: \nu = Ks + (F_edge \u{00B7} D \u{00B7} \u{03C8}) / (2 \u{00B7} ae \u{00B7} fz), from this material's force li |
| HOVER | 250 | ./ui/sim_diagnostics.rs:1035: Arc-mean chip thickness measured by the dexel simulator. An engagement/force signal, NOT the unit any vendor |
| FACE | 249 | ./ui/export_wizard.rs:676: Each transition emits an `M0` pause. The message below appears as the prompt in your sender (g-Sender / UGS / |
| HOVER | 248 | ./ui/properties/operations/surface_3d.rs:728: Confine generation to rest ISLANDS instead of the full surface. Runs only under the machined-stock reference |
| HOVER | 247 | ./ui/properties/operations/surface_3d.rs:674: Run the crease/rest detector inside this operation and cut its claimed valleys as an extra pass. Off by defau |
| ? | 247 | ./ui/feeds/why.rs:784: The chip was thinned as a LAST resort, after the RPM and the cut size had already been reduced. The ploughing |
| HOVER | 237 | ./ui/properties/operations/surface_3d.rs:695: Use the machined prior stock when one is in scope, and the analytic self-probe when none is. Pin the self-pro |
| HOVER | 235 | ./ui/properties/model_sim_panels.rs:59: Point this model at a different file on disk. The model keeps its name, its declared units and its identity, s |
| ? | 233 | ./ui/sim_diagnostics.rs:1327: achieved advance/tooth below the vendor band minimum — rubbing/burning risk. At low advance per tooth the too |
| ? | 233 | ./ui/feeds/why.rs:901: The {formula_chipload_mm:.4} mm/tooth shown is the empirical formula's, and this recommendation carries no v |
| ? | 228 | ./ui/feeds/why.rs:598: Operator ruling 2026-09-25: the tool follows the curve at a set depth, the same engagement a v-carve or trace |
| HOVER | 227 | ./ui/sim_diagnostics.rs:820: {what}\n{} flagged SAMPLES — a per-sample emission tally, not a defect count, and not the same population as |
| ? | 226 | ./ui/sim_diagnostics.rs:781: \nDenominator: TOTAL runtime (cutting + rapids) - the measure the banner and the per-operation thresholds use |
| ? | 224 | ./controller/events/compute.rs:142: '{toolpath_name}' is waiting on simulated stock after '{blocker_name}' (index {blocker_idx}). That operation |
| HOVER | 218 | ./ui/feeds/explore.rs:1328: Overwrite feed and RPM with the explored values, after the safety clamps (plunge is pulled down to the new fe |
| TOAST | 218 | ./controller/events/simulation.rs:64: Simulation {outcome}. The viewport released the previous simulation when this run started, so it shows no sim |
| ? | 217 | ./ui/feeds/why.rs:536: {opening} Suggest reads the vendor row, the depth ladder and the band's depth derate at the nominal Ø (ruling |
| HOVER | 215 | ./ui/overlays/registry.rs:937: How much material this operation leaves. ANY operation attaches a rest grid once something demands one. Off b |
| FACE | 214 | ./ui/properties/operations/surface_3d.rs:815: \u{26A0} This operation cuts FRESH stock, so no machined prior is in scope and the machined-stock reference c |
| HOVER | 213 | ./ui/properties/linking_dressup.rs:141: The cut bottom reaches the bottom of the {:.2} mm board, so the last pass frees the part. Tabs hold it in the |
| HOVER | 207 | ./ui/multitool_planner.rs:361: Grid resolution of the tier map, clamped to 0.2-1.0 mm. A 0.15 mm map costs about 125 s PER LADDER TOOL on a |
| ? | 205 | ./ui/feeds/why.rs:779: The spindle could not turn the cut you asked for. The engine made the cut SMALLER rather than slower: {requir |
| FACE | 203 | ./ui/multitool_planner.rs:637: No fine tier kept an island — the coarse tool holds the whole board at this tolerance. That is a planning out |
| ? | 203 | ./io/export.rs:351: '{name}' was edited after it was generated — the stored result is the previous parameter set's geometry, not |
| HOVER | 200 | ./ui/properties/linking_dressup.rs:117: The cut floor sits {:.2} mm below the bottom of a {:.2} mm board, so the tool cuts into the bed. Generate sta |
| HOVER | 200 | ./ui/multitool_planner.rs:295: How much residual a coarse tool may leave and still keep a cell. It is a RESIDUAL threshold, not a cusp heigh |
| HOVER | 198 | ./ui/properties/stock.rs:434: Two pins on the flip's mirror line, offset so the part cannot seat 180 deg out. The clear strips are the stoc |
| HOVER | 198 | ./ui/multitool_planner.rs:716: Cells LABELLED for this tool, grid-quantised. A LOWER bound on what it actually sweeps: every fine-tier islan |
| FACE | 197 | ./ui/multitool_planner.rs:383: The two dials below OVERRIDE the coarseness slider for whichever of them you set — the core takes an explicit |
| HOVER | 196 | ./ui/overlays/registry.rs:1095: Green where this cutter forms the surface inside the operation's tolerance, red where it cannot, neutral wher |
| HOVER | 195 | ./ui/properties/toolpath_panel.rs:139: This operation generated successfully, then one of its inputs changed. The path in the viewport and the figur |
| HOVER | 194 | ./ui/properties/linking_dressup.rs:361: Repo rule, no source (G10): the helix radius is 0.3 x the tool diameter. Untick to set an operator value. Eit |
| HOVER | 192 | ./ui/feeds/explore.rs:1036: \nDeflection ceiling: {ceiling_mm:.2} mm/tooth, off the top of this chart — it draws {chart_top_mm:.4} mm/too |
| HOVER | 191 | ./ui/overlays/registry.rs:549: The stock wireframe box. It is derived from the stock config, so it draws on a 2D job too \u{2014} before P6 |
| ? | 190 | ./ui/feeds/why.rs:204: The cut was over the spindle's power budget, so the RPM came DOWN ×{scale:.3} and the feed came down with it. |
| HOVER | 189 | ./ui/properties/operations/surface_3d.rs:718: Measure rest against the material the previous pass actually left. Needs this operation's stock source set to |
| HOVER | 188 | ./ui/properties/toolpath_panel.rs:713: Use the closed 2D shapes of a DXF or SVG model as the boundary. The model can be a different model from the |
| HOVER | 185 | ./ui/sim_diagnostics.rs:295: More than two fifths of the run is spent moving at cutting feed without removing material. The toolpath may b |
| HOVER | 185 | ./ui/feeds/why.rs:835: Advance per tooth below the rubbing floor: {commanded:.4} mm/tooth (floor {floor:.4}, {}). The feed is not ra |
| ? | 184 | ./ui/properties/tab_badges.rs:466: ⚠ Rest region covers {:.0}% of the part footprint — regions barely restrict the fine pass; raise min_valley_d |
| ? | 183 | ./ui/toolpath_panel.rs:1058: Inputs changed after this was generated. The path drawn in the viewport and the figures below are from the PR |
| ? | 183 | ./ui/feeds/why.rs:1070: Feed re-derived at the final depth: {requested_mm_per_min:.0} → {rescaled_mm_per_min:.0} mm/min (depth ladder |
| HOVER | 181 | ./ui/sim_op_list.rs:123: The project's simulation cell size. Every simulation and every rest operation uses it, and the project file s |
| HOVER | 181 | ./ui/properties/stock.rs:281: Derived from the setups, not set here. The CAM models one flip — Bottom, which mirrors Y about the stock cent |
| ? | 181 | ./ui/optimize_modal.rs:400: not stamped — which is not the same thing as \"the defaults\". This outcome was not passed through optimize_t |
| ? | 180 | ./ui/feeds/why.rs:408: Chip thinning ×{:.3} is OBSERVED, NOT APPLIED: the chip is thinner per pass at this stepover and DOC, but the |
| ? | 180 | ./ui/feeds/why.rs:209: The feed hit the machine's cutting-feed ceiling, so the RPM came DOWN ×{scale:.3} and the feed came down with |
| HOVER | 179 | ./ui/properties/linking_dressup.rs:470: Replace short retract-rapid-plunge sequences with slow linear feeds. Major time saver for operations with many |
| HOVER | 179 | ./ui/multitool_planner.rs:673: Connected components of this tier's raw labels, before merging, the minimum-island filter and the cap. A big |
| HOVER | 178 | ./ui/properties/operations/surface_3d.rs:291: Cutter engagement the spiral holds on every steady wrap (leading-arc fraction of the tool). Sets the stepover |
| HOVER | 178 | ./ui/properties/operations/surface_3d.rs:255: Constructive inside-out spiral per slice: one continuous stay-down pass per region with engagement bounded by |
| ? | 177 | ./ui/readiness.rs:436: No machine kinematics set \u{2014} Machine properties \u{25B8} Kinematics \u{25B8} \"Import GRBL $$\" (paste |
| FACE | 176 | ./ui/optimize_modal.rs:498: Candidates were scored with feed modulation OFF while normal simulation runs it ON. \"Safe\" and \"faster\" h |
| HOVER | 175 | ./ui/properties/linking_dressup.rs:549: Reorder disconnected toolpath segments to minimize total rapid travel distance (TSP heuristic). Only operation |
| HOVER | 175 | ./ui/multitool_planner.rs:276: The surface finish every tier is dialled to. Each tier's stepover is derived from it and that tier's own tip |
| HOVER | 172 | ./ui/properties/linking_dressup.rs:551: This operation keeps its planned cut order: its entries and keep-down links are planned against the stock that |
| HOVER | 172 | ./ui/feeds/compare.rs:1116: The numbers above show what the calculator would suggest. The line below gives the reason for the refusal. Ch |
| ? | 171 | ./ui/sim_diagnostics.rs:147: From ProjectSession::simulation_triage \u{2014} the same answer the CLI report, the MCP get_diagnostics block |
| HOVER | 170 | ./ui/properties/operations/surface_3d.rs:245: Per-step direction search with preflight skip and widen-band recovery. Slow to generate — use when Contour Pa |
| HOVER | 169 | ./ui/properties/toolpath_panel.rs:792: The model whose closed shapes bound this toolpath. The toolpath cuts only inside the shapes and not in their |
| HOVER | 169 | ./ui/properties/feeds_speeds.rs:466: Vendor chipload column from row {} (calibrated d={:.3} mm), after DOC derate. Published as a linear advance p |
| ? | 169 | ./ui/feeds/compare.rs:819: The {fz:.4} mm/tooth beside this label is the recommendation's commanded advance per tooth, feed \u{00F7} (RP |
| FACE | 168 | ./ui/optimize_project.rs:83: Stage 0/1/2 search across every enabled toolpath. Expect 3–10 minutes on a wanaka-sized job. The GUI is respo |
| HOVER | 167 | ./ui/sim_diagnostics.rs:1041: Engagement = cylinder-side radial width-of-cut fraction. Reads ~10× below the algorithmic target; use it to c |
| HOVER | 167 | ./ui/readiness_panel.rs:610: Apply to every enabled toolpath regardless of selection. CHANGES THE CUT: DOC and WOC move as well as the spe |
| HOVER | 167 | ./ui/properties/linking_dressup.rs:566: Convert sequences of linear segments into smooth G2/G3 arcs. Reduces file size, improves surface finish, and p |
| HOVER | 167 | ./ui/overlays/registry.rs:1035: The islands a rest pass would cut. Authored under Geometry \u{25B8} Machining Boundary \u{25B8} Source \u{25B |
| ? | 166 | ./ui/feeds/compare.rs:741: Vendor range: none published \u{2014} no row matched this tool \u{00D7} material, or the row publishes one li |
| HOVER | 165 | ./ui/readiness_panel.rs:600: Apply Feeds recommendations to every checked toolpath. CHANGES THE CUT: DOC and WOC move as well as the speed |
| HOVER | 164 | ./ui/overlays/registry.rs:1238: Achieved advance per tooth (effective feed \u{00F7} (RPM \u{00D7} flutes)) against the matched vendor band \u |
| ? | 164 | ./ui/feeds/why.rs:611: Suggest reads a V-bit row at its printed key (ruling B4): the printed cutting diameter, or the printed includ |
| HOVER | 163 | ./ui/sim_diagnostics.rs:658: {reason}\n\nCollision detection, material removal and axial DOC are unaffected and remain valid. Gates readin |
| HOVER | 163 | ./ui/feeds/explore.rs:1015: \nChipload corridor: not modelled. This material carries no primary-source cutting coefficient, so neither th |
| HOVER | 162 | ./ui/properties/stock.rs:114: The material's cutting-force line Fc/ap = Ks \u{00B7} h + F_edge (ruling B6). The force and power models read |
| HOVER | 162 | ./ui/properties/linking_dressup.rs:588: Add circular overcuts at inside corners so parts fit together. Essential for joints, inlays, and press-fit ass |
| ? | 162 | ./ui/feeds/why.rs:725: The tool rubs instead of cutting, which burns the work and the cutting edge. The feed is too slow, the RPM is |
| HOVER | 161 | ./ui/optimize_project.rs:661: Reconciled cycle from a project end-to-end sim. `xtp` marks a cross-toolpath interaction — the reconciled val |
| ? | 161 | ./ui/feeds/why.rs:908: The {formula_chipload_mm:.4} mm/tooth shown is the empirical formula's. This recommendation carries no vendo |
| ? | 160 | ./ui/feeds/why.rs:532: {opening} Suggest reads the vendor row at the tip Ø (ruling A1). The RPM, the depth ladder, the band's depth |
| ? | 159 | ./ui/properties/feeds_speeds.rs:447: No kinematics prediction on this trace, so the effective feed falls back to the commanded feed \u{2014} this |
| HOVER | 159 | ./ui/feeds/compare.rs:1291: Derive WOC from a target cusp (scallop) height and the tool's ball-tip radius instead of the formula default. |
| HOVER | 157 | ./ui/multitool_planner.rs:569: Coarseness, overlap and the island dials re-cut the SAME map, which is why they are quick. Tolerance, cell, m |
| ? | 156 | ./ui/menu_bar.rs:16: The simulation result holds no cut trace, and the optimizer needs that trace as a baseline. Re-run the simula |
| ? | 156 | ./controller/events/toolpath.rs:592: To generate rest for {names}, the simulation needs cells of {required_mm:.3} mm. The project resolution is {p |
| ? | 155 | ./ui/readiness.rs:387: No machine kinematics set — Machine properties ▸ Kinematics ▸ \"Import GRBL $$\" (paste your controller's set |
| ? | 155 | ./ui/properties/stock.rs:539: Cannot place registration pins: no tool is defined, so the pin diameter would be a guess. Add the drill you w |
| HOVER | 150 | ./ui/properties/toolpath_panel.rs:1010: The reference the gate measures 'deeper than'. Unset = prefer the machined stock from a prior simulation, els |
| HOVER | 149 | ./ui/properties/linking_dressup.rs:521: Dynamically adjust feed rate based on stock engagement. Higher feed in light cuts, lower in heavy cuts. Only a |
| FACE | 149 | ./ui/multitool_planner.rs:529: One drop-cutter pass per ladder tool over the whole board: seconds at 0.6 mm, tens of seconds at 0.3 mm. The |
| HOVER | 149 | ./ui/multitool_planner.rs:412: An island under this area falls back to the COARSER tool rather than being dropped from the job. Derived from |
| HOVER | 149 | ./ui/feeds/explore.rs:1041: \nDeflection ceiling: {ceiling_mm:.4} mm/tooth, from tool compliance at this stickout and depth. Above it too |
| ? | 148 | ./ui/export_wizard.rs:390: ⚠ Units override ({}) differs from post default ({}). Coordinate values are not auto-converted — verify your |
| ? | 148 | ./ui/export_wizard.rs:1029: Save is blocked: {errors} validator error(s). Go back to Preview & validate to review and tick the override c |
| HOVER | 147 | ./ui/multitool_planner.rs:709: The coarsest tier's cusp target holds everywhere it is not beaten by a finer tier, so it sweeps its territory |
| ? | 145 | ./ui/export_wizard.rs:380: ⚠ Inch output (G20) is not supported yet — coordinates are millimeters and are not converted. Export is block |
| HOVER | 142 | ./ui/overlays/registry.rs:903: Dogbone overcuts and other dressup-introduced bridge segments. Arc-fit replacements are ordinary cutting geom |
| HOVER | 142 | ./ui/feeds/explore.rs:1022: \nRubbing floor: {:.4} mm/tooth, the global floor, or the vendor maximum when that is lower. Below it the too |
| FACE | 141 | ./ui/properties/stock.rs:298: This project stores {}. The stored value is a cache with no say in whether the pins are correct; it is rewrit |
| ? | 141 | ./ui/feeds/why.rs:199: Spindle policy MaxSpeed lifted it ×{scale:.3} toward the spindle ceiling, and scaled the feed with it to hold |
| ? | 139 | ./ui/feeds/why.rs:355: Target {:.4} mm/tooth — no vendor row matched, so the empirical formula set it: fz = K₀·D^p·(1/H)^q = {:.4}·{ |
| ? | 138 | ./ui/viewport_overlay.rs:435: Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O). {count} changed fro |
| HOVER | 138 | ./ui/toolpath_panel.rs:983: Cutting moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} Cutting moves). Enable them there to u |
| HOVER | 137 | ./ui/preferences.rs:1077: Launch default: vsync for a plain launch, no vsync for --mcp. An explicit mode that the display does not supp |
| ? | 137 | ./ui/feeds/why.rs:486: The row shows what `⚡ Apply all` writes: the calculator value after the invariant funnel (the rigidity cap an |
| FACE | 136 | ./ui/properties/feeds_speeds.rs:436: effective feed \u{00f7} (RPM \u{00d7} flutes), over the {} {}. The machine reaches {:.0}% of the commanded fe |
| ? | 135 | ./ui/readiness.rs:424: The simulation does not include every enabled toolpath. Re-run the simulation (Simulation \u{25B8} Re-run Sim |
| ? | 135 | ./ui/properties/toolpath_panel.rs:751: No other toolpaths in this project yet — add one and generate it to use as the source. Picking it here switc |
| ? | 134 | ./ui/feeds/why.rs:396: The long-tool share ×{:.2} does not change the feed. It lowers the aggressiveness load target; the depth and |
| HOVER | 133 | ./ui/toolpath_row_controls.rs:75: Cutting moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} Cutting moves). Enable there to use th |
| ? | 133 | ./ui/readiness.rs:432: The machine profile has kinematics, but the simulation ran before they were set. Re-run the simulation to get |
| FACE | 133 | ./ui/multitool_planner.rs:515: Press Preview to see which tool claims which part of the surface. Nothing is generated and nothing is changed |
| FACE | 133 | ./ui/multitool_planner.rs:135: Tick the tools that take part. The chain runs coarse to fine, and the tip radius below is what decides that o |
| ? | 133 | ./ui/feeds/compare.rs:753: Deflection ceiling: {ceiling:.3} mm/tooth — the heaviest chip that keeps predicted tip deflection inside {EXC |
| HOVER | 132 | ./ui/sim_op_list.rs:112: Captures the toolpath generator's step-by-step output, used to inspect how a toolpath was built. Re-generate t |
| ? | 131 | ./ui/preferences.rs:1020: On: each simulation writes its cut trace as a JSON file. Off (the default): no file. The app keeps the trace |
| HOVER | 131 | ./ui/menu_bar.rs:239: Needs a 3D model and at least two tools in the drawer — the tier map is a drop-cutter residual between two cu |
| ? | 130 | ./ui/sim_diagnostics.rs:1412: Unmodeled: toolpath made no contact with material — every sample was a rapid or air-cut. Check depth / directi |
| HOVER | 130 | ./ui/properties/stock.rs:459: Adds one pin at the centre of the mirror line, then drag it. A centre pin is self-symmetric, so it adds redun |
| ? | 130 | ./ui/components/value_row.rs:64: Suggest {trimmed} = {:.3}{} (source: {src}) \u{2014} the same value Apply writes. Click to overwrite this fie |
| HOVER | 129 | ./ui/toolpath_panel.rs:994: Rapid moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} Rapids). Enable them there to use this p |
| ? | 129 | ./ui/readiness.rs:428: The simulation result holds no cut trace, so it measured no time. Re-run the simulation (Simulation \u{25B8} |
| HOVER | 128 | ./ui/overlays/registry.rs:566: The opaque stock block. Split from the stock box \u{2014} one checkbox used to remove the box, the block and |
| ? | 127 | ./ui/readiness.rs:420: No current simulation covers this project. Run the simulation (Simulation \u{25B8} Run Simulation) to get a m |
| HOVER | 127 | ./ui/properties/stock.rs:100: Per-material feed-rate scaling factor (softwood baseline = 1.0). Higher values reduce recommended feed rates a |
| HOVER | 127 | ./ui/properties/linking_dressup.rs:435: Add smooth arc transitions at cut start and end. Prevents tool marks at entry/exit points. Best for finishing |
| FACE | 126 | ./ui/properties/machine_panel.rs:257: Not set — showing defaults. Editing a value or importing $$ enables the acceleration-aware cycle-time model f |
| HOVER | 125 | ./ui/multitool_planner.rs:431: Over this, the largest islands are kept and the rest go back to the coarser tool. The preview says loudly whe |
| ? | 125 | ./ui/feeds/compare.rs:782: \nAgainst the middle of the vendor range: {wear:.2}\u{00D7} the energy per mm\u{00B3}, {time:.2}\u{00D7} the |
| HOVER | 124 | ./ui/toolpath_row_controls.rs:101: Rapid moves are hidden globally (Overlays \u{25B8} Toolpath \u{25B8} Rapids). Enable there to use this per-to |
| HOVER | 124 | ./ui/properties/feeds_speeds.rs:140: Override the project default spindle speed for this operation. Leave it unchecked to follow the post-config |
| ? | 124 | ./ui/feeds/why.rs:541: {opening} Suggest reads the vendor row at the engaged Ø, and every advance/tooth figure on this surface uses |
| ? | 123 | ./ui/feeds/why.rs:853: Long tool: load target ×{factor:.2} (stickout {stickout_mm:.0} mm is {ratio:.1} × Ø{diameter_mm} mm; repo rul |
| FACE | 123 | ./ui/export_wizard.rs:571: Tool-change blocks are stripped; per-tool spindle RPM is kept. You are responsible for swapping tools outside |
| HOVER | 122 | ./ui/feeds/explore.rs:325: No vendor row matched this tool, material and operation, so the recommendation is formula-derived and carries |
| HOVER | 121 | ./ui/overlays/registry.rs:806: Cyan markers where each entry / ramp / helix starts. Geometric only \u{2014} it shows WHERE an entry is, neve |
| ? | 121 | ./controller/events/compute.rs:1838: generate_all refused: {refusal} Pass `simulation_resolution_mm` to set it, or `fixpoint: false` to skip the s |
| ? | 120 | ./ui/sim_diagnostics.rs:1382: Unmodeled: the cut trace holds no arc engagement \u{2014} re-run the simulation (Simulation \u{25B8} Re-run S |
| ? | 120 | ./controller/events/compute.rs:1808: a generation plan is already running. Poll `generation_status` for its step, or call `cancel_generation` and |
| HOVER | 119 | ./ui/overlays/registry.rs:1051: The clip polygon in use. Authored under Geometry \u{25B8} Machining Boundary; the render work is ledgered, n |
| ? | 119 | ./ui/export_wizard.rs:564: ⚠ Vanilla GRBL rejects M6 (error:20). Only use this on a controller with a configured tool changer (e.g. grbl |
| HOVER | 118 | ./ui/properties/toolpath_panel.rs:801: The toolpath cuts inside the holes of the shapes, for example the area inside an edge band, and stays off the |
| HOVER | 118 | ./ui/properties/post.rs:57: Compute clamps Safe Z so rapids clear the uncut stock. Increase the value above the stock top to silence this |
| ? | 117 | ./ui/preferences.rs:871: On: the viewport starts with every generated toolpath drawn. Off (the default): it draws the selected toolpat |
| FACE | 117 | ./ui/machine_library_modal.rs:61: Snapshot library — importing COPIES a machine into this project; later library edits don't change existing pr |
| ? | 116 | ./ui/feeds/why.rs:322: This stepover is derived from your target scallop height and the tool's ball-tip radius, not from the vendor |
| HOVER | 115 | ./ui/sim_op_list.rs:165: Auto takes the finer of the smallest tool's radius / 5 and the rest tool's tip radius / 5, over the whole pro |
| ? | 115 | ./ui/preferences.rs:1115: Read-only. The MCP server starts only from the command line. Diagnostics sets the present mode for the next s |
| ? | 115 | ./ui/feeds/compare.rs:824: The advance per tooth is not shown either: the recommendation has no positive feed, RPM and flute count to di |
| HOVER | 114 | ./ui/toolpath_panel.rs:1104: Generate every enabled operation in THIS setup. The setups above it are simulated as they stand, not regenera |
| HOVER | 114 | ./ui/sim_timeline.rs:379: Playback speed multiplier. 1× = real-time playback for this project ({:.0} moves/sec, {}). [ and ] keys to adj |
| HOVER | 114 | ./ui/sim_diagnostics.rs:804: Time spent cutting at light radial engagement (2-10% of diameter). Below 2% counts as air cut, on the row ab |
| HOVER | 114 | ./ui/overlays/registry.rs:977: One colour per tool tier over the territory that tier owns, with each fine tier's overlap band in a lighter t |
| FACE | 114 | ./ui/optimize_modal.rs:565: The baseline row is scored against this trace directly — it is not re-simulated at the candidate operating po |
| ? | 114 | ./ui/feeds/compare.rs:757: Deflection ceiling: not modelled — this tool, depth or engagement leaves the deflection model nothing to solv |
| ? | 113 | ./ui/viewport_overlay.rs:183: Paths: Selected is unavailable \u{2014} no generated toolpath is selected, so no move draws. Your choice is ke |
| ? | 112 | ./ui/properties/operations/registry.rs:436: Rest machining requires an earlier enabled operation in the same setup using the previous tool on the same mod |
| ? | 112 | ./ui/preferences.rs:1046: Cut traces go to simulation_metrics and debug traces to toolpath_debug in this folder. Applies to the next jo |
| HOVER | 112 | ./ui/overlays/panel.rs:405: Run the work, then show the row when the data lands \u{2014} if the target and the chosen mode have not change |
| FACE | 111 | ./ui/preflight.rs:452: The stored result for the operations above is the geometry from before the edit. Regenerating them is the fix |
| ? | 111 | ./ui/optimize_modal.rs:390: not stamped — this result predates the machine snapshot, or was built by a constructor that never ran a searc |
| FACE | 111 | ./ui/export_wizard.rs:440: Spindle stays in air at Z={effective_safe_z:.3} mm — verify XY paths and feed rates without touching material |
| ? | 110 | ./ui/feeds/why.rs:689: \nThe hardness scale x{hardness_scale:.2}{capped} applies after the claim; the band scale is x{:.2} in total. |
| ? | 110 | ./ui/feeds/why.rs:218: The RPM was scaled ×{scale:.3} and the engine did not record why. Treat this recommendation as unexplained.\n |
| HOVER | 109 | ./ui/overlays/registry.rs:623: Fixture clearance boxes. Before P6 this one checkbox also drew keep-outs, pins, the flip axis and the datum. |
| HOVER | 109 | ./ui/overlays/panel.rs:710: Compute the rest grid, then show it if this operation is still selected and the model colour has not changed. |
| HOVER | 108 | ./ui/properties/operations/surface_3d.rs:782: \u{26A0} Territory Clip was requested and SKIPPED — this pass covered its full territory, not rest islands. |
| HOVER | 108 | ./ui/multitool_planner.rs:722: The coarse tool carries no overlap band — it holds the complement, and the fine tiers' bands reach into it. |
| ? | 107 | ./ui/properties/toolpath_panel.rs:718: No model in this project has a closed 2D shape. Import a DXF or SVG with closed shapes to use this source. |
| ? | 107 | ./ui/feeds/compare.rs:1213: Auto-derived from scallop height.\n h = {:.0} μm, tip r = {:.2} mm\n ae = 2·√(2·r·h − h²) = {derived:.3} mm |
| ? | 106 | ./ui/multitool_planner.rs:796: and {} island(s) were still DROPPED ({:.0} mm2 back to the coarser tool) — the {} largest of {} were kept |
| HOVER | 105 | ./ui/properties/toolpath_panel.rs:192: This count is from the previous generation. Regenerate the operation to measure the settings shown here. |
| HOVER | 105 | ./ui/overlays/panel.rs:747: Affects the simulated stock only \u{2014} the solid stock block and the height planes are pinned at 0.15. |
| HOVER | 105 | ./ui/feeds/why.rs:527: The published {kind} Ø is {tip_dia:.2} mm, but the cone shoulder does most of the cutting at this depth. |
| HOVER | 105 | ./ui/feeds/explore.rs:117: Use the LUT row's chart-published RPM verbatim. Tightest match to the vendor band's own test conditions. |
| HOVER | 104 | ./ui/overlays/registry.rs:581: The XYZ triad at the stock origin. Distinct from the datum crosshair, which is where the G-code zeroes. |
| FACE | 104 | ./ui/export_wizard.rs:595: Dwell after spindle-on before the first cutting move. Zero = no extra dwell beyond the post's preamble. |
| TOAST | 104 | ./controller/events/compute.rs:767: Sim resolution was coarsened to fit grid limits — consider reducing stock size or increasing resolution |
| ? | 103 | ./ui/sim_op_list.rs:1110: Plunge-class peak is {ratio:.1}× this op's own plunge rate ({} of {} vertical-dominant moves over 1×). |
| ? | 103 | ./ui/setup_panel.rs:502: \nThis setup's simulation starts from uncut stock — material removed by prior setups is not reflected. |
| HOVER | 103 | ./ui/properties/toolpath_panel.rs:747: Boundary = rest regions computed by another toolpath's rest analysis. Pick the source toolpath below. |
| HOVER | 103 | ./ui/properties/toolpath_panel.rs:646: Restrict toolpath to a boundary polygon. Moves outside the boundary are converted to rapids at safe Z. |
| ? | 103 | ./ui/feeds/why.rs:795: \nIt is STILL over budget. Take a shallower or narrower cut, or use a machine with more spindle power. |
| ? | 102 | ./ui/sim_diagnostics.rs:1626: The cut trace is from an earlier version of the project. Run the simulation again to measure the cut. |
| ? | 102 | ./ui/properties/tab_badges.rs:456: ⚠ {count} rest regions — threshold likely below the prior pass's cusp height; raise min_valley_depth. |
| ? | 102 | ./ui/feeds/compare.rs:768: Force headroom: none — the predicted deflection is {:.1} % past the {EXCEEDS_BOUND_MM:.3} mm bound.\n |
| ? | 102 | ./io/export.rs:102: MACHINE-SAFETY ERROR: {errors} of {} findings are errors. Do not run this program until you fix them. |
| HOVER | 101 | ./ui/overlays/registry.rs:1309: Deflection in \u{00B5}m, drawn as a bent cutter. 200\u{00D7} exaggerated; direction is illustrative. |
| HOVER | 101 | ./ui/overlays/registry.rs:1204: Colour cutting moves by feed rate: green \u{2192} yellow \u{2192} red for light \u{2192} heavy load. |
| ? | 100 | ./ui/sim_diagnostics.rs:1390: Unmodeled: no steady-state cutting samples — toolpath runs entirely on transient (plunge/ramp) feeds |
| FACE | 100 | ./ui/overlays/registry.rs:1568: '{id}' is one of the {} colour choices \u{2014} switch another one on instead of switching this off |
| ? | 100 | ./ui/overlays/legend_rail.rs:876: where an entry starts, not how hard it cuts \u{00B7} only an operation with an entry style draws one |
| ? | 100 | ./ui/feeds/why.rs:426: Vendor value {value:.4} (one printed value, held as a point; no band); this sits at {:.0}% of it.\n |
| TOAST | 100 | ./app/export.rs:240: Save blocked: {errors} validator error(s). Tick the override on the preview step to proceed anyway. |
| HOVER | 99 | ./ui/properties/toolpath_panel.rs:1055: The rest field this operation leaves, and the regions the operations above take as their boundary. |
| ? | 99 | ./ui/properties/operations/validate.rs:240: Selected model must provide both 2D geometry and a 3D mesh (use Surface selector for separate mesh) |
| ? | 99 | ./ui/feeds/why.rs:662: The vendor prints one row for the material in the label. The LUT derives this {} row ({}) from it. |
| ? | 98 | ./ui/overlays/live.rs:117: from a simulation that is older than the project \u{2014} run it again in the Simulation workspace |
| FACE | 98 | ./ui/optimize_modal.rs:94: This may take a few minutes. The GUI is responsive — hit Cancel to stop and keep partial results. |
| ? | 98 | ./ui/feeds/compare.rs:786: \nWear and time ratios: not modelled — the band midpoint gives the closed form no usable value.\n |
| ? | 97 | ./ui/optimize_modal.rs:584: {declared_family:?}/{declared_pass_role:?} → {queried_family:?}/{queried_pass_role:?} (rerouted) |
| HOVER | 97 | ./ui/feeds/explore.rs:797: {rpm:.0} RPM · {feed:.0} mm/min → commanded advance/tooth {cl:.4} mm/tooth · {verdict}{cap_note} |
| HOVER | 96 | ./ui/properties/toolpath_panel.rs:1035: A cell counts as REST material once the reference floats more than this above the true surface. |
| ? | 96 | ./ui/feeds/compare.rs:773: Force headroom: not modelled — the pre-simulation deflection predictor declined this pairing.\n |
| ? | 96 | ./ui/feeds/compare.rs:1219: Scallop-driven stepover needs a ball or tapered-ball tool; the formula default is used instead. |
| HOVER | 95 | ./ui/sim_diagnostics.rs:194: A metric that could not be measured returns no verdict. Collision detection is never disabled. |
| HOVER | 95 | ./ui/properties/feeds_speeds.rs:116: A drilling operation moves in Z only, so its plunge rate IS its feed rate. Set the feed above. |
| ? | 95 | ./ui/feeds/compare.rs:686: Power: calculator {calculator} of the limit; {cause}, so the cut Apply writes draws {applied}. |
| TOAST | 95 | ./controller/events/planner.rs:119: Multi-tool finishing needs a 3D model — the tier map is a drop-cutter residual over a surface. |
| HOVER | 94 | ./ui/multitool_planner.rs:837: Preview first — applying a ladder nobody has looked at is what the preview exists to prevent. |
| ? | 94 | ./ui/feeds/why.rs:922: The drilling envelope for this tool and material is {envelope_lo:.0}–{envelope_hi:.0} mm/min. |
| ? | 93 | ./ui/sim_debug.rs:162: Build Z levels from stepdown, optional shelf detection, and optional fine-stepdown expansion. |
| HOVER | 93 | ./ui/properties/toolpath_panel.rs:931: Expand (positive) or shrink (negative) the boundary. Applied before tool-radius containment. |
| HOVER | 93 | ./ui/overlays/registry.rs:1180: Per-toolpath palette colour with Z-depth blending. The only mode the span filter applies in. |
| ? | 93 | ./ui/components/value_row.rs:59: {trimmed} already matches the recommendation as applied ({:.3}{}). Source: {src}.{clamp_note} |
| ? | 93 | ./io/export.rs:359: '{name}' is still waiting on upstream stock — run Generate All / simulate the prior operation |
| ? | 93 | ./controller/events/compute.rs:2054: an earlier operation in this setup did not generate, so no simulation can unlock toolpath {} |
| ? | 92 | ./ui/sim_diagnostics.rs:1021: m{move_start} · waste {wasted_runtime_s:.2}s · peak commanded a/t {peak_advance:.4} mm/tooth |
| HOVER | 92 | ./ui/readiness_panel.rs:706: Apply the recommendation to this toolpath. CHANGES THE CUT (DOC/WOC) as well as the speeds. |
| HOVER | 92 | ./ui/multitool_planner.rs:399: Islands closer together than this merge. Derived from the tier's own tip radius when unset. |
| HOVER | 92 | ./ui/feeds/explore.rs:976: Vendor data: none matched. The range is absent and the recommendation is formula-derived.\n |
| FACE | 92 | ./ui/feeds/compare.rs:1328: ⚠ This tool has no spherical tip — scallop height is ignored; the formula stepover is used. |
| ? | 91 | ./ui/overlays/legend_rail.rs:550: green \u{2264} {:.3} mm \u{00B7} grey = unresolved \u{00B7} mid {:.2} mm \u{00B7} log scale |
| HOVER | 90 | ./ui/viewport_overlay.rs:431: Every viewport option, searchable, with the reason for any that cannot draw (shortcut: O). |
| FACE | 90 | ./ui/readiness_panel.rs:773: Every toolpath is already at or above its recommendation — no project-wide gain available. |
| HOVER | 90 | ./ui/properties/toolpath_panel.rs:112: This operation does not regenerate on its own. Press G, or click Generate, to compute it. |
| ? | 90 | ./ui/properties/operations/mod.rs:349: Approach height. Tool switches from rapid to feed rate here before plunging into material. |
| ? | 90 | ./ui/export_wizard.rs:1083: ⚠ Project spindle {} rpm exceeds post limit {} rpm — emitter will clamp at the move site. |
| TOAST | 90 | ./controller/events/mod.rs:1475: Applied Feeds recommendations to {applied} of {what}; skipped {} that Suggest refused: {} |
| FACE | 89 | ./ui/sim_op_list.rs:238: Use Run Simulation above to verify toolpaths, check collisions, and review stock removal. |
| ? | 89 | ./ui/sim_diagnostics.rs:1374: Unmodeled: no cut trace \u{2014} run the simulation (Simulation \u{25B8} Run Simulation) |
| ? | 89 | ./ui/sim_debug.rs:170: Constant-engagement stepping: direction search, local material checks, and tool stamping. |
| HOVER | 89 | ./ui/properties/post.rs:74: Replace rapids (G0) with G1 at high feedrate for machines with unpredictable rapid motion |
| ? | 89 | ./ui/overlays/panel.rs:663: The generation then runs the simulation again, so the current simulation may be replaced. |
| ? | 89 | ./ui/feeds/why.rs:887: The recommendation is formula-derived and carries no band. Missing rows: {missing_rows}. |
| ? | 89 | ./ui/feeds/compare.rs:671: Force headroom: calculator {calculator}; {cause}, so the cut Apply writes has {applied}. |
| ? | 89 | ./controller/events/compute.rs:88: '{toolpath_name}' uses remaining stock (rest machining) but is no longer in the project. |
| FACE | 88 | ./ui/sim_trace_modal.rs:127: Scroll to zoom, drag to pan, double-click to show the whole run. Click to go to a move. |
| HOVER | 88 | ./ui/readiness_panel.rs:680: Suggest refuses this toolpath, so no apply — batch or single — will write to it.\n{why} |
| FACE | 88 | ./ui/properties/operations/surface_3d.rs:796: Not generated yet — the resolved rest reference appears here after this operation runs. |
| ? | 88 | ./ui/preferences.rs:941: These are the values at start and after a project opens. They change no computed number. |
| TOAST | 88 | ./controller/events/compute.rs:775: Simulation produced an empty mesh — try increasing resolution or check stock dimensions |
| ? | 87 | ./ui/properties/operations/registry.rs:559: the unified planner runs three bands; one stepover picture would misreport two of them |
| ? | 87 | ./ui/properties/operations/height_diagram.rs:609: No model geometry reached this operation, so the diagram has no model profile to draw. |
| HOVER | 87 | ./ui/properties/linking_dressup.rs:315: Spiral descent. Best for deep pockets and hard materials. Spreads heat and load evenly. |
| ? | 87 | ./ui/preferences.rs:1013: Advanced. These settings write files for fault-finding. They change no computed number. |
| HOVER | 86 | ./ui/properties/toolpath_panel.rs:1048: Extra clearance added around detected regions beyond this toolpath's own tool radius. |
| ? | 86 | ./ui/properties/linking_dressup.rs:282: This operation sets its entry move directly — the dressup entry style isn't used here. |
| ? | 86 | ./ui/feeds/compare.rs:763: Force headroom: {:.1} % of the {EXCEEDS_BOUND_MM:.3} mm deflection bound is unused.\n |
| HOVER | 85 | ./ui/properties/toolpath_panel.rs:522: Apply the validator's auto-fix to this toolpath. Mark stale and regenerate to apply. |
| ? | 85 | ./ui/properties/operations/height_diagram.rs:599: Model: the drawing is flat. A 2D model has no Z extent, so its profile is one plane. |
| HOVER | 85 | ./ui/properties/linking_dressup.rs:313: Angled descent into material. Prevents plunge burns. Recommended for most operations. |
| ? | 85 | ./app/export.rs:329: Export summary: {} G-code lines, {} moves, {:.0} mm cutting, {} tool changes, {} ({}) |
| HOVER | 84 | ./ui/overlays/registry.rs:1020: Per-tier island polygons. The numeric table lives in the multi-tool planner dialog. |
| ? | 84 | ./ui/optimize_modal.rs:596: refused — no rows for {tool_family:?} on {declared_family:?}/{declared_pass_role:?} |
| ? | 84 | ./controller/events/compute.rs:2693: UNKNOWN: {} toolpath collision check(s) failed — the collision result is incomplete |
| ? | 83 | ./ui/properties/operations/mod.rs:339: Rapid travel height within an operation. Tool retracts here between cutting passes. |
| ? | 83 | ./ui/overlays/registry.rs:1016: drawn by Inspect \u{25B8} Tier map \u{2014} no separate island outline renderer yet |
| FACE | 83 | ./ui/optimize_modal.rs:447: Snapshot taken when the search ran — the session's machine may have changed since. |
| ? | 83 | ./ui/feeds/why.rs:250: calculator feed = advance/tooth × RPM × flutes = {advance:.4} × {:.0} × {flutes}.\n |
| ? | 83 | ./ui/feeds/why.rs:157: calculator {raw}; the dial holds the load at {pct:.0} %, so Apply writes {shown}.\n |
| HOVER | 83 | ./ui/feeds/explore.rs:1029: \nDeflection ceiling: not modelled for this tool, so the upper bound is not drawn. |
| HOVER | 82 | ./ui/toolpath_panel.rs:1097: A generation plan is already running. Stop it from the Generate All button first. |
| ? | 82 | ./ui/sim_timeline.rs:1139: {}: holder collision at local move {} (run move {move_index}) — click to navigate |
| FACE | 82 | ./ui/properties/toolpath_panel.rs:378: unreachable {unreachable_pct:.1} % {area_basis_note} · max gap {max_gap_mm:.2} mm |
| ? | 82 | ./ui/feeds/compare.rs:747: Rubbing floor: {:.3} mm/tooth — below it the edge burnishes instead of cutting.\n |
| ? | 82 | ./ui/feeds/compare.rs:736: Vendor value: {:.3} mm/tooth. The matched row publishes one value, not a range.\n |
| FACE | 80 | ./ui/sim_diagnostics.rs:492: Project — {cycle_str}{basis_tag} · \u{2713}{ok} within · \u{2715}{bad} exceeding |
| ? | 80 | ./ui/optimize_modal.rs:1075: advance/tooth dropped to {:.4} mm/tooth ({pct:+.0}% under vendor band min {:.4}) |
| ? | 80 | ./ui/feeds/why.rs:300: \nThis depth is deep enough to derate the feed ×{tier:.3}, to limit deflection. |
