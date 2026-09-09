# First normal-path checkpoint — SVG pocket

Date: 2026-09-09. Owner: Astra. **Partial expert walkthrough, not completed UX
review or novice test.** Evidence: MCP-VIEW + MCP-STATE + targeted CODE reads.
No HUMAN/DESKTOP interaction observed. R01/R02/R03/R06/R07/R08 have initial
coverage; profile/part release, small STL, advanced cases and human completion
remain outstanding. This checkpoint is not the programme-wide top ten.

## Five-line summary

- A scratch SVG pocket can be imported, configured, generated, simulated, saved,
  reopened and exported through MCP; this does not prove the GUI click path.
- The primary confidence issue is **actionable diagnostic priority**, not lack
  of metrics: an actual critical plunge-rate finding coexists with a green overview.
- A one-setting experiment removes that finding but turns the overview amber
  because of air time. The visual verdict does not communicate that tradeoff.
- Stale evidence is clearly signalled in several places; unsimulated backend
  export correctly refuses without an explicit unmodeled-data override.
- Native import/save dialogs, picking, corrective-control discovery and six of
  seven wizard steps remain untested because no desktop-input driver is exposed.

## Conditions and reproducible seed

See `../W00/REPORT.md` for build/agent provenance. Live build advertises
`8a4df241-dirty`, timestamp 08:55:51 +12. Checkout ef91cb03 plus concurrent changes;
no review source edits or builds. 1400×900 screenshots, no requested resize.

Seed: raw `fixtures/demo_pocket.svg`, model ID 0, Top setup ID 0; then explicitly
set stock Z=12 and origin Z=-12 (turning Auto from model off). Stock 80×60×12,
Generic Softwood, medium workholding; default Generic Wood Router, no explicitly
configured machine kinematics. Explicit review tool: flat Ø6, cutting length 20,
shaft/shank Ø6, shank length 10, stickout 30, holder Ø25, two flutes, tool number 1.
No fixtures or pins were added. This is not a physical setup recommendation.

MCP-created Pocket ID/index 0: depth 5, maximum depth/pass 1.2, stepover 2.1,
feed 1515, plunge 750 mm/min, RPM 17000. Saved full configuration:
`scratch/REVIEW_ONLY_svg_baseline.toml`; SHA-256
`8efa961bb490e1eb69ddc16364627c893833948178048b91cfa9b73a26cde920`.
It includes default ramp/links/arc fitting/feed-optimization dressups.

Simulation: 0.25 mm, cutting metrics enabled through MCP. Initial GUI had Capture
cutting metrics OFF and Auto from tool size ON. MCP changes both, so the observed
rich diagnostics are not evidence of what a first unaided click on Run produces.

The instance restarted during a pause. `evidence/06_export_wizard.png` misleadingly
names the intended capture: **it actually shows the new blank app**, NOT a wizard.
Do not use it as wizard evidence. Restored-job captures and A/B are in `../W01b/`.
The restored baseline was re-simulated and matched the earlier 725 moves,
361.068 s and 37.874% total-runtime air exactly before the parameter experiment.

## Task outcome ledger

| Goal | Observed outcome | Limitation |
|---|---|---|
| Start/import SVG | MCP-assisted completion; correct 70×50 geometry and 80×60 auto stock | No native-dialog/menu discoverability test |
| Establish board/tool | MCP-assisted stock/tool authoring; dimensions visible | No real workholding/datum teach-back |
| First pocket | Generated 725 moves; clear operation purpose/depth/tool/input in inspector | Add menu/default selection bypassed by explicit MCP IDs |
| Verify | Metrics/simulation complete at pinned cell | Capture opt-in bypassed; holder clearance still Not checked in Readiness |
| Identify priority | Critical plunge finding found through backend data, not visible primary overview | No human comprehension/hover test |
| Correct/reverify | One-setting counterfactual, stale-state capture, regenerated and re-simulated | Corrective field was chosen with implementation knowledge |
| Save/reopen | Saved/reloaded editable job; load auto-generated; simulation initially absent | Not native GUI file flow; no undo test |
| Export | Unsimulated backend refusal; simulated scratch export succeeds without flags | Not wizard Save/preflight/direct-menu parity |
| Profile/part release; STL; STEP/DXF | NOT TESTED in this pass | Required follow-on, not inferred from Pocket |

Active task time, unaided time, click counts and wrong turns were not measured.
Image timestamps span 09:32–09:39 and resumed work after 13:51; the intervening
pause is not application wait or compute time. No build/test/optimization wait
was induced. No user-usability score is assigned.

## Findings

### UX-R06-001 — Primary overview does not prioritize a current critical action

**Type:** evidence presentation/IA. **Impact:** S0 misleading-confidence *risk*;
confirmed presentation mismatch, NOT a demonstrated physical collision/damage.
Primary owner R06, handoff R07/R09. Local review of priority aggregation required;
not permission to promote a confidence instrument into a new export safety gate.

**Observed:** baseline `run_simulation` returns `verdict: OK`, zero collisions,
37.874% total-runtime air, and a current **critical** `project.plunge_class_load`
action: 70/70 vertical-dominant moves over the op's 750 mm/min plunge rate,
peak 2.533×, 28 over 2×. The load report separately says Within 1/1 on its three
criteria, with nonempty populations. These are different observations, not a
proof that the load gates themselves miscomputed.

`evidence/04_after_simulation.png` shows a green “No collisions, air cutting under
threshold” header, “1 within · 0 exceeding”, and optimization calls to action.
The critical plunge message is not visible in that primary captured overview.
`evidence/05_simulated_readiness.png` likewise shows green simulation/rapid/tool-load
rows, with REVIEW BEFORE CUTTING caused by Holder clearance: Not checked.

**Code corroboration:** `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:114-168`
constructs the status banner from collision count and a 40% air bar only.
The triage read at approximately 512 is used for the NOT MEASURED strip; merely
sharing that object does not establish that its actions are rendered as priorities.
`sim_op_list.rs:971` offers more kinematic detail through a pill/hover, which is
not the same as a visible current-action list. We have not tested all tooltips,
collapsed sections or alternate panels and do not claim universal absence.

**Controlled consequence:** after disabling only dressup `feed_optimization`,
regenerating and re-simulating at the same cell, the plunge action disappears;
725 moves and both path-distance totals remain unchanged. Runtime becomes
505.898 s and total-runtime air 58.106%. The overview becomes **amber** for air
cutting (`../W01b/evidence/04_after_feedopt_off.png`). The apparently greener arm
had the critical rate finding; colour alone does not rank this decision correctly.
The diagnostic's explanatory sentence blames “the modulator”; this review has
NOT verified that mechanism and does not use that sentence as causal proof.

**Export scope:** restored baseline refuses unsimulated export, then succeeds
after simulation with no accept flags, despite the plunge action. This confirms
the backend gate covers the three load verdicts, not every current action. It
is not evidence that the GUI wizard silently bypasses its own checks.

**Proposal:** show the highest-priority current triage action above efficiency
and identify its operation/location plus a useful next step. Keep collision,
load and efficiency checks separately named. Preserve expert overrides and the
non-blocking status of the kinematic instrument unless separately ruled otherwise.
**Acceptance:** on this seed, a user identifies the plunge-rate issue without
MCP/source/hover hunting and explains why removing it can increase estimated time
and air percentage; can return to refreshed evidence after an edit.

### UX-R01-001 — Onboarding omits the verification step

**Type:** guidance; S2 uncertainty; confirmed MCP-VIEW wording, human impact untested.
`../W00/evidence/01_initial.png` lists import, stock, tool, toolpath, then
“Generate and export G-code.” It does not include simulate/review/correct.
This is a poor introductory sequence for a product with separate Simulation and
Readiness workspaces, though it does not prove users skip them in practice.
**Proposal/acceptance:** include verification and correction in the normal path;
a CAM-literate newcomer can describe the checks needed between Generate and Export.
Keep the compact guidance and direct expert export routes.

### UX-R01-002 — Imported SVG inspector does not expose size/scale alongside input

**Type:** guidance/control availability; S2; visible absence confirmed, recovery
route incompletely tested. `../W00/evidence/02_svg_import.png` shows
`Type: Some(Svg)`, path and collapsed Details, with no dimensions or units/scale
control. Source `ui/properties/mod.rs` gates the dimensions/scale block on mesh.
The backend can report the 70×50 bbox; the person-facing initial inspector cannot
answer the same basic size question at a glance. Details contents and alternate
routes need human interaction; do not claim the entire app has no size information.
**Proposal/acceptance:** give 2D input the relevant bbox/units affordance and a
supported correction/refusal path; user verifies a known-size drawing before cuts.
Raw `Some(Svg)` is separately S3 terminology polish, not the root issue.

### UX-R06-002 — Stale status is visible, but the retained success banner competes

**Type:** evidence hierarchy; S3 provisional interaction impact. After the edit,
`../W01b/evidence/03_stale_simulation.png` has strong left/top/timeline stale cues,
load counts become unmodeled, and graph indicates last-run data. **These are
strengths**, not a silent stale-pass defect. However the inspector still leads
with the green “No collisions…” sentence above its smaller stale annotation.
**Proposal/acceptance:** label the successful checks as last-run evidence or give
freshness precedence. A user correctly identifies what is current without losing
the ability to inspect previous results.

## Strengths to preserve

- Correct SVG auto-stock sizing through the shared session path. A support agent
  incorrectly flagged this as broken; the orchestrator rejected that finding
  after reading `session/mutation.rs:586-607` and checking the live import.
- Pocket inspector has an explicit purpose, tool/input selectors, diagram and
  bounded set of primary geometry fields. Pending vs generated status is visible.
- Readiness distinguishes not run, current, holder not checked and unmodeled.
- Mutation triggers multiple stale cues; backend export refuses unmodeled load
  without an acknowledgement. No accept flags were used in this review.
- Wizard initial screen exposes its seven-step route and controller facts;
  `../W01b/evidence/01_export_wizard.png` is the valid capture.

## Handoffs / untested claims

R03/R04: the relevant **Feed rate optimization** checkbox is visibly on
**Linking → Optimization**, not Dressup or Feeds & Speeds
(`../W01b/evidence/05_linking_control.png`; `ui/properties/mod.rs:5203-5220`).
Whether a user can find it from the warning still needs a human task. Separate
this pre-generation dressup from post-simulation adaptive feed modulation.
R07: human-driven wizard/preflight/direct-export routes and setup sheet still
need observation. R08: native save/reopen/undo and long-job cancellation untested.
R09: collapsed warnings/hover discoverability and defaults need actual input.

Worker candidates about export-route bypasses, STEP rescale, first-tool/model
selection and vacuous gates remain leads, not this checkpoint's live findings.
No blanket redesign, machining approval or implementation ticket is issued yet.
