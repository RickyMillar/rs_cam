# Holistic UX review programme

Status: **planning and source reconnaissance only — no live usability review yet.**
Date: 2026-09-09. Source snapshot inspected: `8a4df241` plus the current working tree.
Owner: Astra for programme design, large investigations and synthesis. Use Codex or DeepInfra for bounded supporting tasks.

## 1. The question

Can a person turn their model and machining intent into an understood, verified,
recoverable machining job — without knowing how rs_cam is implemented?

The target is not fewer features. It is a short, understandable normal path,
with powerful controls available when needed and visible consequences when used.

Review **user decisions and transitions**, not one panel at a time:

> Understand the input → establish the physical setup → choose a cutting plan →
> configure and generate → inspect predicted motion → simulate → understand the
> evidence → fix or improve → verify again → export and set up the real machine.

Save, reopen, change a tool, replace a model, undo a mistake and recover from a
failed calculation are part of that flow, not afterthoughts.

### What changes from the existing prompt

Keep [PROMPT.md](PROMPT.md) intact. Use its viewport/rendering/overlay inventory
as input to R09, and its task ideas where relevant elsewhere. Do **not** run it
as a competing whole-product audit. This programme supersedes its scope and
execution order, not its useful visual checks.

The previous prompt centres a knowledgeable terrain operator and a mature
project. We also need blank-start, 2D, STEP, mixed-geometry and return-to-work
journeys. Loading a prepared job does not test authoring it. A successful MCP
mutation does not establish that its GUI control is discoverable or usable.

## 2. Users and scope

Use these as review lenses, not claims about a researched user population:

- **New to rs_cam, familiar with basic router CAM:** knows stock, tools and feeds,
  but not our operation names, navigation, implicit dependencies or defaults.
- **Occasional/returning user:** needs to reconstruct intent and what is current.
- **Experienced operator (Ricky):** needs fast iteration, fine control, multiple
  tools/setups, diagnostic depth and repeatable output.

A true CAM beginner is a separate validation participant if available. Do not
confuse a lack of machining knowledge with a navigation problem, or use a
training requirement to excuse a misleading default.

“Any model” means the **supported 3-axis workflows**: SVG/DXF drawings, STL
surfaces, STEP with face-aware features, multiple referenced models, and
cardinal-face setups. Unsupported geometry should lead to a clear explanation
and next step. It does not mean arbitrary CAD repair, free-form five-axis
machining, undercut access from above, or a newly invented CAD modeller.

Scope includes menus, viewport, side panels, dialogs, notifications, libraries,
keyboard interaction and exported setup instructions. CLI/MCP are supporting
interfaces and parity checks, not a replacement for the desktop journey.
No real machining, controller connection or production export in this programme.

## 3. What reconnaissance established

These are **source observations and review leads**, not ranked UX findings.
Confirm the visible behaviour in a pinned running build before declaring a defect.
Line references are starting points and may move.

| Observed structure | Why the review needs it | Evidence |
|---|---|---|
| The shell has Setup, Toolpaths, Simulation and Readiness, with different selection/inspector paths | Test continuity of context across workspaces, not just each layout | `crates/rs_cam_viz/src/ui/workspace_bar.rs`; `src/ui/properties/mod.rs:204` |
| Empty selection has a Getting started list; later it can say “project tree” | Test the actual first experience, instructional accuracy and whether verification enters the normal path | `crates/rs_cam_viz/src/ui/properties/mod.rs:218-243` |
| Model properties, import, rescale and project reload are different code paths | Test every supported format through the human import door and the reopen door. The mesh-only scale UI and STEP early return particularly need a live check | `crates/rs_cam_viz/src/ui/properties/mod.rs:755-985`; `src/controller/io.rs:17-153` |
| Adding an op checks available geometry; the controller initially uses the first tool and first model | Test mixed tool/model order and inappropriate pairings before assuming that selecting the right item is sufficient | `crates/rs_cam_viz/src/ui/toolpath_panel.rs:649-710`; `src/controller/events/toolpath.rs:12-173` |
| Feeds exposes separate speed and cut-geometry applies; tools have Apply/Revert and navigation-away commit semantics | Ask users to predict mutation scope. Trace the actual commit, not only the button text | `crates/rs_cam_viz/src/ui/properties/mod.rs:1947-2147`; `src/ui/properties/tool.rs:19-52`; `src/ui/properties/mod.rs:90-142` |
| Generate All with remaining-stock ops requires a pinned resolution, edited in Simulation | This is a cross-workspace prerequisite and a prime handoff to walk | `crates/rs_cam_viz/src/controller/events/toolpath.rs:347-413` |
| Multi-tool finishing has a menu → dialog → preview → Apply plan path, in addition to manually authored rest operations | Judge whether advanced capability can be found from a machining goal; do not assume tier map, reach and rest analysis answer the same question | `crates/rs_cam_viz/src/ui/menu_bar.rs:167-203`; `src/ui/multitool_planner.rs`; `src/controller/events/planner.rs` |
| Simulation has capture settings, resolution, stale states, multiple selection scopes and diagnostic paths | “Simulation finished” is not the outcome. The user must identify a priority, locate it and know what to do | `crates/rs_cam_viz/src/ui/sim_op_list.rs:17-200`; `src/ui/sim_diagnostics.rs`; `src/ui/sim_timeline.rs` |
| Readiness, direct export and the wizard are distinct entry paths | Walk every export route and verify what is checked, what can be overridden and which settings persist | `crates/rs_cam_viz/src/ui/readiness_panel.rs:17-232`; `src/ui/menu_bar.rs:86-125`; `src/ui/export_wizard.rs` |
| There are useful zero-operation UX fixture projects, not just mature terrain jobs | Authoring can be reviewed without repeatedly preparing a large project | `test_data/ux_*.toml`; [FIXTURES.md](FIXTURES.md) |
| Automation records a small set of widget labels, rectangles and enabled states; MCP navigates state, not arbitrary pointer/keyboard input | Screenshots and renderless tests alone cannot prove click paths, focus, drag/drop or native file dialogs | `crates/rs_cam_viz/src/ui/automation.rs`; `src/controller/tests.rs:349-433`; `src/mcp_server.rs:1570-1617` |

Earlier IA work is useful history, not a specification of today’s app:
`planning/ui_audit/{MAP,IA_TARGET,FINAL_DESIGN,DENSITY_PASS_2026-06-11}.md` and
`planning/UX_TESTING_SESSION_PLAN.md`. These contain retired surfaces and target
states. Even current overview prose can lag code. Verify claims through current
handlers and the live build. In particular, do not import old strategy rankings,
overlay defaults or diagnostic meanings into this review uncritically.

## 4. Independent review packages

Each package uses [PROTOCOL.md](PROTOCOL.md), the fixture manifest and its own
brief. It returns one report and evidence directory. It does not edit source.

| ID / brief | User question and ownership | Required output beyond findings |
|---|---|---|
| [R01 — Start and model intake](briefs/R01_START_AND_MODELS.md) | Can I get the intended geometry into the app at the right size and understand what it can machine? | Blank-start journey; format/selection/units matrix |
| [R02 — Physical setup](briefs/R02_PHYSICAL_SETUP.md) | Does the virtual job clearly match the stock, machine, workholding, orientation and zero I will use? | Setup-intent record; single/two-sided/lateral comparison |
| [R03 — Tools and toolpath authoring](briefs/R03_TOOLPATH_AUTHORING.md) | Can I choose an appropriate tool/operation and configure what will be cut? | Choice/parameter hierarchy; first-operation walkthroughs |
| [R04 — Feeds and improvement decisions](briefs/R04_FEEDS_AND_OPTIMIZATION.md) | Can I choose sensible settings, understand recommendations and improve a job without losing control? | Apply-scope/provenance map; fair comparison and optimization loop |
| [R05 — Multi-operation planning](briefs/R05_SEQUENCING_AND_REST.md) | Can I build and maintain a dependent, multi-tool/multi-setup cutting plan? | Dependency/ownership map; manual versus planner journey |
| [R06 — Simulation and diagnosis](briefs/R06_SIMULATION_AND_DIAGNOSIS.md) | Can I interpret results, find what matters, fix it and confirm the fix? | Evidence-to-action map; comprehension and recovery traces |
| [R07 — Readiness and handoff](briefs/R07_READINESS_AND_EXPORT.md) | Do I understand what is verified and exactly what I am sending to the machine? | Export-route matrix; physical setup-sheet teach-back |
| [R08 — Changes, persistence and recovery](briefs/R08_LIFECYCLE_AND_RECOVERY.md) | Can I iterate, interrupt, undo and return later without losing intent or trusting stale evidence? | Mutation/invalidation/undo/persistence matrix |
| [R09 — Interaction and information architecture](briefs/R09_INTERACTION_AND_IA.md) | Can I navigate, select, see and operate the tools efficiently at different experience levels and screen sizes? | Capability/home map; visual/keyboard interaction audit |

### Boundaries: avoid nine disconnected redesigns

- R01 owns input geometry; R02 owns its relationship to the physical setup; R03
  owns operation-level target selection. Test their handoffs on the same model.
- R03 owns the cutting intent and parameter hierarchy. R04 owns recommendation
  evidence, apply semantics and improvement decisions. R05 owns upstream stock,
  ordering and planner-generated groups, not a new feeds audit.
- R06 owns meaning and actionability of simulation results. R09 owns legibility,
  pointing, overlays, focus and navigation. A colour that miscommunicates a
  safety verdict is linked in both reports, with one primary finding owner.
- R07 owns the final go/no-go decision and output. R08 owns freshness and data
  lifetime. A stale export is not two unrelated defects.
- Every report includes local navigation friction. R09 synthesizes the shared
  interaction pattern; it must not prescribe a new shell before journey evidence.

## 5. Execute in waves

### Wave 0 — Establish review conditions

Follow [TOOLING.md](TOOLING.md). Pin the build, agree a dedicated GUI session,
validate fixtures, record known issues and prepare disposable project copies.
Choose which actions a human or a desktop-input tool will perform. Do not begin
“discoverability” scoring with MCP navigation silently filling the gaps.

Record a baseline **simple SVG → pocket/profile → simulation → scratch export**
and **small terrain → finishing → simulation → scratch export**. The baseline
is a journey trace, not an exhaustive audit. Do not give a new participant the
control-by-control route. Later packages inspect the decisions where they stall.

### Wave 1 — The normal path

R01 → R02 → R03 → R06 → R07 for the basic 2D and small-3D spines.
Run R08’s edit/reopen checks at the relevant transitions, not only at the end.
Read-only code research for different packages may run in parallel; GUI writes
must be serialized against one shared instance.

**Checkpoint:** summarize the three largest barriers to completing a normal
job and the three places where confidence might outrun the evidence. No redesign
approval yet. This makes the first wave useful without waiting for every feature.

### Wave 2 — Expert power and awkward cases

Run R04, R05 and the remainder of R08. Extend R01–R03 to STEP, DXF, mixed inputs,
unsupported selection and tools in an inconvenient order. Extend R02/R07 to a
flip and a lateral setup. Use the mature Wanaka project only after small fixtures.

### Wave 3 — Interaction consistency

R09 reviews the real decision points collected above and the original prompt’s
render/overlay inventory. Test normal tasks at 1400×900 logical points; repeat
critical crowded states and modal layouts at 2400×1300. Also inspect actual
machine DPI/UI scaling. Do not spend the budget photographing every possible
combination of 38 overlays and 23 operation types.

### Wave 4 — Synthesis and human validation

Astra runs [SYNTHESIS.md](SYNTHESIS.md). Merge root causes, resolve conflicting
recommendations, propose a coherent flow and rank changes by user impact.
Validate the highest-risk proposed changes with task walkthroughs or lightweight
mockups before writing implementation tickets.

Suggested session shape: one focused discovery pass, then targeted evidence
collection, then a written checkpoint. Stop a session if a long computation or
blocked input path prevents observation; record the block and continue on a
smaller fixture. No unlimited generation or strategy sweep for a UX review.

## 6. What success means

For a supported task, the operator can explain:

1. **Intent:** what model/region, setup, stock, tool and operation are active.
2. **Next action:** what to do next and why, including an unmet prerequisite.
3. **Consequence:** what a proposed edit/apply will change and invalidate.
4. **Evidence:** what was actually measured, for which version and scope.
5. **Limit:** what a green result does not establish.
6. **Recovery:** how to fix, undo, resume or safely stop.
7. **Handoff:** which files, tools, datums and setup changes the real job needs.

Measure assisted versus unassisted completion, wrong turns, workspace/dialog
switches, repeated data entry, unexplained changes, and interpretation mistakes.
Separate active user effort from compute wait. These are descriptive observations,
not an invented numerical usability score. Preserve useful power features in a
“keep” list. Do not reward fewer clicks if the shorter route hides a safety decision.

## 7. Deliverables and launch

Each reviewer writes `results/Rxx/REPORT.md`, `results/Rxx/trace.md` and evidence
under `results/Rxx/evidence/`. Use stable IDs `UX-Rxx-001`. Review-time generated
projects/outputs stay in that review’s scratch directory, never overwrite seeds.
Use the report structure in the shared protocol.

Launch one review with:

> Read `planning/ui_review_2026-09-09/PROTOCOL.md`, `TOOLING.md`, `FIXTURES.md`
> and `briefs/Rxx_….md`. Execute only that brief. Review read-only; change no
> production code or user files. Distinguish live-human, live-MCP, code-backed
> and untested evidence. Stop and report unavailable tools rather than silently
> bypassing the human route. Write the required Rxx report and trace. Commit nothing.

Astra owns synthesis and large/ambiguous investigations. Bounded source checks,
fixture inventories and report consistency checks may use Codex or DeepInfra.
Do not run multiple reviewer agents against the same live GUI simultaneously.

### Decisions to settle before live execution

- Is the assumed primary audience right: CAM-literate new/occasional users plus
  Ricky as the expert, rather than complete CNC beginners?
- Can we use a dedicated rs-cam GUI and scratch library/project area?
- Will Ricky perform the short unaided journeys, or should desktop-input
  automation be added? Ideally include someone unfamiliar with this app.

These choices need not block source reconnaissance. They do affect what the
later reports can honestly claim about usability.
