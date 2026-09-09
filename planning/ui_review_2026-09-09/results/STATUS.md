# UX review execution status

Updated 2026-09-09 after the small 3D baseline. Owner Astra.

## Completed in this pass

- [Wave 0 conditions and verified worker corrections](W00/REPORT.md).
- [SVG pocket checkpoint](W01/CHECKPOINT.md), including
  [action trace](W01/trace.md), save/reload, unsimulated-export refusal,
  simulated scratch export and one-setting correction/reverification.
- [Small 3D rough/finish baseline](W02/REPORT.md), with scratch export.
- Two bounded DeepInfra source investigations, complete; outputs in W00/support.
  One claimed stock-sizing bug was disproved by following the shared session
  call. Worker reports are leads, not blindly accepted findings.

## Principal observed issues

1. **Diagnostic priority:** a current critical plunge-rate finding is not visible
   in the primary green SVG overview; the no-plunge counterfactual becomes amber
   for efficiency instead. Modeled check colours do not rank the full decision.
2. **Actionability:** the terrain overview says “Must address: Annotation 15”
   without identifying the subject or a local jump action in that row.
3. **Warning readability:** long finish cautions run off the right edge at the
   normal1400×900 window width, cutting off the missing guarantee/advice.
4. **Onboarding/input context:** initial steps omit simulation; SVG inspector
   lacks at-a-glance dimensions/units. Human impact still needs observation.

Strengths: auto stock sizing works; op purpose/default context is useful; stale
state has multiple visible cues; unsimulated backend export refuses; Readiness
keeps holder clearance explicitly not checked.

## Not completed

- No unaided participant or actual desktop click/type/drag/navigation observations.
- No native file-dialog/undo/wizard Preview-and-Save walkthrough or setup sheet.
- No full profile/part-release journey, STEP/DXF/mixed-target variants or physical
  setup teach-back.
- No remaining-stock chain, planner/optimizer session or dense expert-job review.
- No full R01–R09 reports or global redesign/synthesis; these are early checkpoints.

## State left for the next pass

Connected GUI: `REVIEW ONLY - 20mm terrain, not for machining`, two generated and
simulated ops, Readiness workspace at1400×900. Saved editable state:
`W02/scratch/REVIEW_ONLY_20mm_terrain_authored.toml`.
MCP simulation cell0.25, metrics on; fine-contact measurements degraded, not a
quality/safety certification. No export accept flags were used.

Recheck session and build before mutation; GUI was restarted once during the
session and other developers are changing/building this checkout. Do not kill
or restart their processes. Embedded metadata remained8a4df241-dirty despite
checkout HEADef91cb03, so identify the actual binary, not just git HEAD.

## Next useful task

With a human or approved desktop-input driver, on the current scratch job:

1. Find what “Annotation15” means and locate one relevant move.
2. Open the finish operation and explain the complete chipload-floor caution.
3. Walk export Preview/Save to a new review-only destination and explain the
   two tools, datum, uncertainty and manual checks. Stop before any machine use.

Record routes/predictions without feeding the person implementation instructions.
Then proceed to R04/R05/R08 deeper cases with the same evidence discipline.

No application/config/library files were edited by this review; no builds,
Rust tests, commits or process cancellation. All review writes are contained in
`planning/ui_review_2026-09-09/results/`. No support agents remain running; the
review does not continue invisibly after an assistant final response.

## 2026-09-09 (late afternoon) — session `rs-cam-b9` (Claude Code, Fable 5.1): R03 live + Track B

Appended by the second session named in `HANDOFF.md`. Nothing above this line
was edited. Own GUI instance (pid 3967327, spawned by this Claude Code process
through `.mcp.json`; binary SHA-256 `9f1cb522…1311`, build `8a4df241-dirty`).
The terrain job of the other session was never loaded here; no other
`rs_cam_gui` process was running when this session started.

### Delivered

- `R03/REPORT.md` — 13 findings, ledger, decision tree, strengths, handoffs.
- `R03/trace.md` — 33 MCP steps across four seeds with hashes.
- `R03/census.md` — all 23 menu operations + Pin Drill (CODE).
- `R03/evidence/00…28_*.png` — full-window captures, all read.
- `R03/scratch/` — five REVIEW ONLY seeds, four saved authored states, one
  NOT-FOR-MACHINING NC file. Library catalogs untouched (timestamps checked).
- `W03/support/` — read-only source-track reports for R04 / R05 / R08 when
  they arrive (see below).

### Findings worth reading first

- **UX-R03-001 (S0, confirmed, generated-path defect):** pocket ramp legs
  leave the pocket region and cut the surrounding stock on the plain F1 seed
  (NC + sim checkpoint 0 + six-view). Simulation says OK / 0 collisions. This is
  the ledgered "2.5D ramp containment" follow-on in
  `planning/entry_moves_2026-09-03/FINDINGS.md:263`, now measured. Engineering
  owner; R06 evidence-gap owner.
- **UX-R03-003 (S0, confirmed):** the GUI toast says "MCP: Added toolpath …"
  for a REFUSED add (`app/mcp.rs:571-577` pushes the toast before the call).
- **UX-R03-004 (S1, confirmed):** Drill on a drawing with no circles silently
  drills the polygon centroid; the inspector promises circle positions.
- **UX-R03-002 (S1, CODE + MCP refusal text):** Add binds the FIRST project
  tool; a flat-first project cannot add Scallop by the menu even though a ball
  exists. Needs a click to confirm the toast.
- UX-R03-005 (S2 → R04): Feeds card labels recommendation values as
  "Commanded advance/tooth" and "DOC".
- UX-R03-006/007 (S2): Profile defaults to a through cut with zero tabs; depth
  beyond stock (15 on 12 mm) generates silently.

### Live state left in this instance

STEP plate scratch job loaded, one Pocket op in ERR (no face picked), plus an
imported "3mm Ball Endmill" snapshot. Nothing here is needed by another
session; the instance dies with this Claude Code session.

### Open questions for the GUI-owning / human track

1. Click Scallop Finish in the Add menu on
   `R03/scratch/REVIEW_ONLY_R03_f3_terrain_flat_first.toml`: what does the
   person see, and do they find the fix?
2. Pick a face on the STEP plate and generate the pocket (no MCP route).
3. Undo after a parameter edit and after Remove.
4. Does "Inherit from stock ✓" or the stored model-silhouette boundary win?

Supporting agents: this session used Claude Code read-only `Explore`
subagents (no MCP, no shell edits) instead of the `pi --provider deepinfra`
route named in `HANDOFF.md`, because the session already runs on Claude
credentials; nothing was configured or changed to do so. Their claims are
leads until verified against source, as before.

Addendum (same session, ~16:45): the four read-only source-track reports arrived
and are saved in `W03/support/*_v1*.md` (R04 partial; a second agent run writes
the full `*_source_track.md` files). Spot-checked against source and folded into
R03: `boundary_inherit` is a dead dial (UX-R03-009 → S1); `stale_since` is never
rendered in the GUI (UX-R03-011 → S1 for 3D ops); the per-field ⚡ pill writes the
uncapped recommendation (new UX-R03-014, S0, R04 owner); the ball-tip refusal has
three membership defects (folded into UX-R03-008). R05/R08 leads worth verifying
first are listed in `R03/REPORT.md` §8.

Final addendum (same session, ~17:00): all eight Track B files are on disk in
`W03/support/` — `r03_census_verification.md`, `r04_source_track.md`,
`r05_source_track.md`, `r08_source_track.md` (second run, full) plus the four
`*_v1*.md` first-run copies. R03 folded in one correction from them: the live
"stale row" capture (evidence 18) is not clean evidence because a Pocket
auto-regenerates within 500 ms; UX-R03-011 now rests on CODE for 3D ops.
UX-R03-003 widened: nine MCP handlers toast in the past tense before running.
Next session: R04/R05/R08 owners should start from the "verify first" lists in
`R03/REPORT.md` §8 and the live-only questions at the end of each support file.
No support agents remain running; the review does not continue after this
session's final response. Live instance: STEP scratch job, disposable.


## 2026-09-09 evening — Astra: source-led UI/IA pass complete

User-authorized four-track UI/IA review, separate from the earlier defect sweep.
Start/end source HEAD **4af7dd96**; all 115 viz source-file hashes unchanged during
this pass. No live GUI requests, application/config/library edits, builds, tests,
commits or fix agents. Review artifacts only under `results/IA/` plus this append.

**Read `IA/SUMMARY.md` first.** It contains the executive answer and six ranked
design changes. Detailed outputs: `IA/CURRENT_MAP.md`, `IA/PROPOSED_FLOW.md`
(current/proposed journeys and rough wireframes), `IA/VERIFICATION.md`, and three
read annotated screenshots under `IA/annotated/`.

Four DeepInfra read-only workers completed normally. Their `IA/tracks/*.DRAFT.md`
outputs are NOT final findings: Astra corrected claims about absent Re-run/Cancel,
toast-only errors, independent sticky focus, forced disclosure collapse, missing
provenance types and persistence. Corrections are explicit in VERIFICATION.md.

Recommendation: keep four workspaces and expert controls; improve scoped
issue-to-edit-to-recheck, current/proposed value roles, decision-first forms,
editing context/next actions, before-Apply consequences and common export handoff.
Existing engineering defects stay in a separate backlog; their current fix status
was not re-certified. Do not begin another broad sweep just to validate these IA
proposals; use bounded sketch/input tasks after design discussion.

This pass is complete; no workers remain running. No GUI instance ownership was
assumed. Recheck the current build and instance before any future live work.

## 2026-09-09 night — fix programme handed off

The review's findings are consolidated into a phased fix plan for unattended
agents at `planning/ui_fix_2026-09-09/PLAN.md` (P0 research, P1 clear-spec
defects, P2 freshness model, P3 MCP gap fill, P4 research-dependent
correctness, P5 IA changes D0–D6, P6 validation). Task status lives in
`planning/ui_fix_2026-09-09/STATUS.md`. This review directory is now read-only
input for that programme.
