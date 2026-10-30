# Toolpath panel UI/UX review — findings (2026-10-30)

Evidence: `shots/` (full window, 1920x1165 px at scale 1.2), `crops/` (the
inspector at 2x), `CODE_INVENTORY.md` (file:line owners). GUI binary from
10:23; it does not have ce32777f3 (reach legend text, R7c).

Projects used:
- STEP solid `planning/adaptive_research_2026-10-23/play/block_with_blind_hole.step`,
  new project, Pocket / Profile / Drill / Face (saved copy: scratchpad only).
- `planning/fixtures/rivmap100/rivmap100.toml` (mesh + DXF): Face, 3D Rough,
  Scallop Finish, Project Curve. The fixture file is not changed.

Tag: **A** = possible with the existing egui `ui/components/`; **B** = needs a
new egui component; **C** = limited by egui.

## Journey (one Pocket from Add to Done)

| Step | Shot | What the panel shows | Problem |
|---|---|---|---|
| 0 Empty project | `00_empty_toolpaths` | Rail: "No toolpaths · Add toolpath". Right: a 7-step "Getting started" list in plain text. | The list is not live: no step shows done, no step is a link. |
| 1 Add Pocket, no tool in project | `j01_*` | Status **"Ready"**. Header: "No tool fits: needs a tool. Set a tool on the Cut t…" (truncated). Cut tab repeats it in red and says "Set a tool on the Cut tab" while on the Cut tab. Toast repeats it. Feeds: three "set a value" boxes. | Status says Ready while two blocking errors stand. "No tool fits" is wrong when no tool exists. No "Add tool" in the inspector. Pick mode opens by itself (pick bar over the viewport). |
| 2 Add a tool to the project | — | The Pocket keeps "No tool set". | The new tool is not bound to the op that waits for one. |
| 3 Bind the tool | `j02_*` | Status **"Refused"**, header "Operation failed: Pocket: no feeds data for this…". Feeds tab shows a full **Recommendation** (RPM 18000→9842, DOC, WOC) with "Apply all", while Feed/Plunge/Spindle stay "set a value". Footer: "Geometry: Pocket: no feeds data…". Tab strip wraps to 2 rows when the dots appear. | The op refuses for "no feeds data" next to the feeds data. Wrong category ("Geometry"). Layout jumps. |
| 4 Apply speeds | `j03_*` | Header still "Refused · Operation failed: no feeds data". | The error stays after its cause is fixed, until a regenerate. |
| 5 Generate | `j04_*` | "Done · 415 moves modulated". New amber row: "Tool load: Depth of cut exceeds the rigidity rule…". "6 hints". | "Speeds only" kept DOC 1.5 against the calculator's 0.83; the panel then warns about the value it let stand. "modulated" is jargon. |
| 6 Edit stepover | `j05_*` | "Done · 371 moves planned" (auto regenerate). Hints 6 → 3. | "planned" vs "modulated" changes with no explanation; the footer moves while background jobs land. |

## Findings

| # | Finding | Evidence | Owner | Tag |
|---|---|---|---|---|
| F1 | The status word does not follow readiness: "Ready" with two blocking errors; "Manual" for a never-generated op. | j01, m04 | `properties/toolpath_panel.rs:1714-1822` | A |
| F2 | A last-run refusal stays in the header after its cause is fixed. | j03 | `toolpath_panel.rs:149-165` | A |
| F3 | The no-tool state says the same thing 3x plus a toast; it points to the Cut tab from the Cut tab; the inspector has no "Add tool"; a new project tool does not bind the waiting op; Generate greys with no reason. | j01, j02 | `tab_badges.rs:741-748`, `operations/validate.rs:66-74`, `toolpath_panel.rs:127` | A |
| F4 | Feeds: the calculator has a recommendation but the fields stay empty and the op refuses "no feeds data for this tool". Operator asks: the suggestion is the default. This revisits three rulings: (a) F.5 "the calculator never writes" (`feeds_speeds.rs:15-18`); (b) no underived defaults (operator 2026-10-23: a default with no derivation is removed, the field stays empty, the op refuses) — this is WHY the fields are empty; (c) no background field locking (explicit Suggest buttons). **Needs a ruling.** A Suggest value is derived, so (b) allows it. The narrow carve-out: on op add and on tool change, write Suggest into EMPTY fields only, show provenance "suggested" with the ⚡ marker, never overwrite a typed value, never lock. When Suggest has no basis, the field stays empty and the refusal names the missing basis. | j01, j02 | `feeds_speeds.rs:69-118`, `ui/feeds/compare.rs` | A |
| F5 | Duplicate controls with different values: Entry Style Helix (Cut) vs None (Linking) on 3D Rough; Heights grid 15/5/3/0 (offsets) vs diagram 40/30/28/25/20 vs Linking diagram "Feed Z 28 / Top height 25"; reach toggle in panel and dock. The panel writes `show_reach_map` directly (`mod.rs:1747,1794`); `settle_exclusive_surfaces` then keeps the LAST row in registry order, which may not be the toggle the operator just ticked (to verify). | m03, j01 | `operations/surface_3d.rs`, `linking_dressup.rs:423`, `operations/mod.rs:145`, `mod.rs:1747,1794` | A |
| F6 | Tabs are not op-aware: all six for every op. 3D ops: Geometry tab = one "Pick faces" button. Dressup offers dogbone on 3D (its own hover says not needed). "Machining Boundary" sits at the foot of the Cut tab, not on Geometry. "6/8 dressups active" counts 6 controls that live on Linking. | m02, s01-s03, j01 | `tab_badges.rs:614`, `geometry_tab.rs:236-266`, `toolpath_panel.rs:1331-1341` | A |
| F7 | The footer repeats on every tab: the reach block (5 rows), "Gates: run simulation to evaluate", findings, "n hints". On Scallop it is the same 5 rows on Geometry, Cut, Feeds and Dressup. | m02 | `toolpath_panel.rs:345-497` | A |
| F8 | Message content is developer prose: "(repo rule, unsourced)", "common scale 0.688, before the depth snaps…", "AT OR BELOW", "discretisation, not geometry", "the band the grid cannot classify either way". Category prefixes are wrong ("Geometry:" for feeds, "Tool load:" for a tool kind). "1 hints". | j02, j04, m02, m05 | core diagnostics adapters, `state/runtime.rs:996-1016`, `rs_cam_core/src/maps/reach_map.rs:576-583` | A |
| F9 | An error has no fix action: wrong tool kind gives no "choose a ball nose" button; the reach map still computes for the refused tool (79.1 %). | m05 | `tab_badges.rs:383-498` | A |
| F10 | Fit at the 280 px default: truncation in the header error, status ("Done · edite…"), Cuts summary ("2 i…"), recommendation deltas ("↓ 0.5…"), chips ("! within ban…"), "Each picked fa…"; a hint clipped mid-word ("Through cut of a 25 mm bc"); the Strategy combo overflows the right edge; the tab strip wraps to 2 rows when a dot appears. | all | `app.rs:633-638` (280), `components/tab_strip.rs` | A (width) + B (tab overflow) |
| F11 | Grid: 3 label widths (116 token, DataTable 0.42xW, natural width), 4 unit placements, two misaligned grids on Heights, raw `label + combo` rows (Tool, Input, Source, Containment, Offset, Entry Style). | all | see `CODE_INVENTORY.md` §7 | B (one row component, type-enforced) |
| F12 | Diagrams mislead: Drill Points draws a fixed 7-point "representative pattern" (`shape_diagrams.rs:861`) for a 1-circle selection; Scallop and Pocket share a generic contour picture; the heights codes CZ/RZ/FZ/TZ/F1 have no legend. | s02, m02, j01 | `operations/shape_diagrams.rs`, `height_diagram.rs` | A |
| F13 | Hints: closed by default, one truncated line each, hover has no evidence, open state shared across toolpaths. | code only (the hints were not opened on screen) | `toolpath_panel.rs:229-268,484-497` | A |
| F14 | Sentries: IC1 renders a fixture with a tool, a clean selection and no reach map, so it never sees the error and footer states; a truncated 250-char note counts as one row; caps are after-state + 3. IC2 scans the Feeds tab of one Pocket. | code | `tests/the_inspector_text_is_bounded_ic1.rs`, `tests/tabular_data_reads_one_table_ic2.rs` | A |

Aside (not UI, to verify): binding a new tool to Scallop Finish (index 2)
reported `stale_toolpaths: [1, 2]`, so the upstream 3D Rough went stale.

## New, repeat or already ruled

| Finding | Status | Prior record |
|---|---|---|
| F10 fit / truncation | **Repeat** | UR1 (2026-09-14): feeds tab clipped; its up4 sentry measured a synthetic string. F14 is the same lesson again. |
| F11 grid / components | **Built, then regrown** | Premium programme UP1–UP8 (2026-09-13/14, `planning/ui_premium_2026-09-13/`) built `ui/tokens.rs`, the type scale and the component set; its close-out (367413d54) says it "polished the clutter" because its rule 1 forbade moving a control. Declutter phase (236be6826), UI-02 grid kit (1c2822501) and text audit (`planning/ui_text_audit_2026-10-02/`) followed. 110 commits touched `ui/properties/` since 2026-10-13; new features used raw rows beside the kit. The missing part is enforcement, not the parts. |
| egui vs HTML | **Ruled** | 2026-09-13: egui stays; an iced/Slint port is a view rewrite, "not now". This review agrees. |
| Declutter rule | **Ruled** | UR series: each package must DELETE a control, not hide a duplicate route. The proposals below follow it. |
| F4 feeds default | **Ruled, reopened by the operator** | F.5; no underived defaults (2026-10-23); no background locking. |
| F1-F3, F5-F9, F12, F13 | New | — |
| F14 sentries | New instance of an old class | UR1 up4 sentry. |

## What other CAM tools do

V = verified in a source this session; R = recalled, not checked.

| Practice | Tool | V/R | Bears on |
|---|---|---|---|
| The op dialog has five tabs: **Tool** (tool choice, coolant, speeds and feeds), Geometry, Heights, Passes, Linking. Tool and feeds are on ONE tab. | Fusion ([operation parameters overview](https://help.autodesk.com/cloudhelp/ENU/Fusion-CAM/files/MFG-OVERVIEW-PARAMETERS-OVERVIEW.htm)) | V | F6, F3 |
| Feeds and speeds fill from the tool's **cutting preset** when you choose the tool; the op can switch presets. Typed values are the exception. | Fusion (Autodesk blog / forum) | V | F4 |
| Linking holds lead-in, lead-out, transitions. Entry is not on a second tab. | Fusion | V | F5 |
| Base Geometry with nothing selected defaults to the whole model, and says so. Depths and Heights are separate; the Tool Controller (tool + feeds) is chosen on the Operation tab. | FreeCAD CAM ([CAM Profile](https://wiki.freecad.org/CAM_Profile), [CAM Slot](https://wiki.freecad.org/CAM_Slot)) | V | F6 |
| A field error shows a warning glyph on the field and one line at the dialog top; the detail is in the tooltip. | Fusion | R | F3, F8, F9 |
| The tool picker inside the op opens the library, so "add a tool" is in the op. | Fusion | R | F3 |
| One card per op, few fields, no tabs. | Kiri:Moto | R (the search found no op reference) | F6 |

## Proposals

Each proposal deletes something. Tag as above.

| # | Proposal | Discharges | Tag |
|---|---|---|---|
| P1 | **Header: name, tool, run.** Name; the Tool dropdown (the ONLY tool control); one status word from one function (`Not ready` / `Ready` / `Generating` / `Done` / `Out of date` / `Failed`) with one button. A refusal clears when its cause clears. "planned/modulated" goes to hover. | F1, F2, F3, F8 | A |
| P2 | **A problem sits on its field.** A missing tool turns the Tool dropdown red; a missing feed turns its field red; Generate stays off. No checklist, no sentence that repeats the field (operator, 2026-10-30: the first mock showed the tool 4 times). | F3, F9 | A |
| P3 | **The tool menu is the library door.** The Tool dropdown lists project tools (refused ones grey, reason on hover), then "Add from library…" and "New tool…". The first fitting project tool binds an op that has none. | F3 | A |
| P4 | **Feeds seed on empty** (needs the F4 ruling). On add and on tool change, empty Feed/Plunge/Spindle take the Suggest value with a ⚡ "suggested" marker; a typed value stays. "Apply all" stays for the DOC/WOC half. | F4 | A |
| P5 | **Op-aware tabs.** The tool is in the header (P1). `Geometry` (selection, floor, machining boundary), `Passes` (the op params), `Feeds`, `Heights`, `Linking` (entry/exit, links, arc fit, dogbone where it applies). A tab with no control for this op is not drawn. Entry Style lives on Linking only. Delete the "n/8 dressups" label. | F5, F6 | A |
| P6 | **One owner, one place.** Core `Diagnostic` gets an `owner` (field id / operation / note / project / event / view) and two texts, `face` (≤ 60 chars) and `detail`. One viz render function places it: field → outline + hover; operation → issue row (max 3, one Fix); note → "i n" counter; project → Readiness; event → toast (never a state); view → viewport legend. Sentry: each diagnostic id paints at most once per screen in the bad-state fixtures. Counted from the shots: no tool 3 → 1, no feeds 3 → 1 or 0, wrong tool 3 → 1, reach notes 5 → 1, "Gates: run simulation" 6 per op → 0. This is the 2026-10-02 text-audit placement contract (face headline, hover detail, developer text to MCP), made a type. | F3, F7, F8, F9, F13 | A |
| P7 | **Reach map lives in the viewport.** The dock toggle and the legend own it. The panel keeps one line on the Geometry tab of a finishing op: `Reach 37 % unreachable · worst 5.9 mm  [Show]`. The grid-floor and over-statement notes move to the legend hover. Delete the panel checkbox and its 4 rows. | C7, F5, F7 | A |
| P8 | **Message rewrite pass.** Every operator-visible diagnostic message: ≤ 60 chars on the face, plain words, correct category, the derivation in the hover. Core keeps the full text for MCP. | F8 | A |
| P9 | **One property row component, type-enforced.** `PropRow { label, control, unit, hint }` with ONE label width token; units always as a suffix in the control. A raw `ui.label` + combo in a properties tab fails a sentry (or does not compile, if the row is the only door). | F11 | B |
| P10 | **Width and tab strip.** Default inspector 340 px (max 420 stays). The tab strip never wraps: it shrinks labels to icons + tooltip, or moves the tail into a `»` menu. A dot never changes the row count. | F10 | A + B |
| P11 | **Diagrams from the real data or none.** Drill draws the picked holes; ops with no faithful diagram draw none. The heights diagram labels its lines in words on hover and uses the same numbers as the grid. | F12, F5 | A |
| P12 | **Sentries with teeth.** IC1 fixtures for the no-tool, refused, wrong-tool, reach-on and boundary-on states; count the full galley length of a truncated row, not 1; fixed caps, no "+3". | F14 | A |
| P13 | **Empty state is live.** The "Getting started" list ticks each done step and each step is a link. | journey 0 | A |

## The rail (left)

Seen in the shots, not reviewed in depth: the "Generate All" button label
turns into a status ("Generate All · simulating All setups"); row tool names
truncate ("Tapered Ball…", "V-Bit Ø5.50 …"). P1's rule (a button is an action,
a status is a word) applies here too.

## Tag count and the egui question

14 findings: 12 A, 2 need a B component (row component, tab overflow), 0 C.
13 proposals: 11 A, 2 B.

egui still costs more than CSS in two places: text reflow and responsive
layout. Evidence: the mid-word clip "Through cut of a 25 mm bc" and the tab
strip wrap. `app.rs:170-176` records that `wrap_mode = Wrap` was reverted
because it breaks grid labels mid-word. These are B items with a real cost,
not toolkit blockers. The rest is information architecture and message
content, which an HTML UI would carry over unchanged. Conclusion: keep egui
(the 2026-09-13 ruling stands). The tokens and components exist; build P9 and
P6 so the property row and the one diagnostic render path are the only doors.

Review page: https://claude.ai/artifact/Gk47TzZUk4FnLZkRxk35e2 (source in `page/`).

## Design rules (placement rules on top of the existing tokens)

1. One fact, one home. A second mention is deleted, not made smaller.
2. The header answers two questions: what is this (name, tool), can it run (one status word, one button).
3. A problem sits on its field. Only a problem with no field gets an issue row.
4. One row shape: `LABEL_COL_WIDTH` label, `ROW_ACTION` control height, unit inside the control. One label width.
5. Colour means state: `DANGER` blocks, `CAUTION` check, `OK` done, `ACCENT` action/selected.
6. Three text sizes from the DESIGN_SPEC scale. Sentence case. No category prefixes. ≤ 60 chars on the face.
7. Space groups sections; the only rule line is under the tabs.
8. A suggestion is a value with a ⚡ mark, or a small ⚡ button beside a typed value.
