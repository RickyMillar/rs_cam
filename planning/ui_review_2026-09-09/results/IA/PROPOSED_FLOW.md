# Proposed workflow — evolve the existing shell, do not replace it

**DESIGN PROPOSAL, not shipped behaviour or a usability-test result.** Grounding:
[CURRENT_MAP.md](CURRENT_MAP.md), earlier captures and the verified portions of
four source tracks. These are rough interaction sketches, not pixel specifications.
No implementation changes are authorized by this document.

## Design decision: retain the four workspaces

Setup, Toolpaths, Simulation and Readiness are defensible concerns. Replacing
them with a new wizard or universal tree is not supported by the evidence.
Keep expert free-order work. Improve the **connections**, the **scope of actions**
and the **order in which information is presented**.

```text
CURRENT: pieces exist, but the operator joins the handoffs
 Import → change workspace → select stock → configure → change workspace
 → select tool / Add strategy → tabs → Generate → Sim
 → counts / hotspot → remember which op → return / select / edit
 → Generate → return / re-run / find issue again → choose an export route

PROPOSED: same workspaces, connected by explicit scoped actions
 Setup [input + physical intent]
   └─ Create operation for this input/setup → Toolpaths [cut intent]
        └─ Inspect / verify this operation → Simulation [evidence]
             ├─ Locate issue → Edit responsible operation [return bookmark]
             │     └─ Recheck changes → refreshed evidence at matching location
             └─ Review export → Readiness [scope, checks, output handoff]
```

Arrows are offered routes, not mandatory stages. Menu, shortcut and context-menu
launchers stay available. No automatic library writes, exports or expensive
simulations merely because a person changes workspace.

## D1 — Keep editing context visible across the shell

**Current:** workspace changes left-panel content, while the shared inspector
continues to follow global selection. Simulation op clicks change playback focus,
not editor selection. Planner setup targeting uses yet another selection rule.

**Proposed:** a small context header, backed by explicit IDs but expressed as names:

```text
Job: Terrain board  /  Setup: Top  /  Operation: Finish valleys
Input: terrain.stl   Tool: 3mm ball              [Change target…]
Path: out of date      Checks: previous run      [Generate / recheck…]
```

No redundant selectors in every panel: summary here, authoritative editors behind
Change target. If an inspector shows a setup while Toolpaths is active, label that
context and offer **Edit in Setup**. Initially preserve cross-workspace editing
for experts rather than adding a forced navigation detour. Test whether scoped
per-workspace selection memory would be better before adopting that larger change.

For blank/partial jobs, a compact task-aware next action replaces the disappearing
Getting started list: import → confirm size/stock → choose tool/operation → verify.
Start an empty job in Setup, or present the same start card from Toolpaths; the
latter is the lower-disruption alternative. Opening a mature job preserves context.
Do not call stock “configured” just because a default value exists: show its origin
and let the operator confirm their physical intent.

**Acceptance:** after switching workspaces or jumping playback to another op,
a person correctly names the object the next Edit/Apply will affect.

## D2 — Turn simulation into a scoped correction workspace

**Current:** good playback, graphs, multiple re-run buttons and detailed evidence;
manual issue→editor→fresh-issue connection missing. Counts like Annotation15 do
not name the work to do. Preserve existing lock/follow controls and timeline.

```text
┌ Verification ─────┬ Viewport ─────────────┬ Inspect: Finish valleys ───────┐
│ Check scope:      │ selected issue       │ Current / previous-run /      │
│ Top / enabled ops │ highlighted          │ limited evidence              │
│ [Run / recheck…]  │                      │                               │
│                  │                      │ Needs attention               │
│ Rough      ✓     │                      │ Named finding + consequence   │
│ Finish     !     │                      │ [Locate] [Edit operation]     │
│                  │                      │ [Details]                     │
│ Capture / cell ▸ │                      │                               │
│                  │                      │ Checks: collisions / load /   │
│                  │                      │ unmeasured — separate results │
│                  │                      │ Efficiency & time ▸           │
├──────────────────┴──────────────────────┴───────────────────────────────┤
│ Play / scrub   Inspection follows playback [Lock this region]          │
│ Timeline + optional signals / generator detail                          │
└────────────────────────────────────────────────────────────────────────┘
```

- Freshness and measurement limits qualify the priority message, not only a
  smaller annotation below a green verdict. Old evidence stays inspectable.
- Named actions come before efficiency optimization. Not every annotation is a
  danger: severity comes from the finding, not its internal kind name.
- **Edit operation** targets the responsible op and relevant tab where known;
  if the cause is uncertain, say so instead of inventing a fix button.
- **Recheck changes** previews necessary work (affected generation, dependency
  refresh, simulation scope/cell) and starts it explicitly. Reuse in-flight
  auto-generation; don't secretly duplicate or launch costly jobs.
- A return bookmark uses operation identity plus spatial/semantic context. After
  regeneration, old move indices may mean something else. If remapping fails,
  return to the op with “location changed”, not an unrelated move.
- “Finding no longer observed” requires comparable fresh checks. If a metric
  became unmeasurable, display that; disappearance alone is not resolution.

**Acceptance:** locate a finding, change the correct operation, regenerate/recheck,
and find the corresponding fresh evidence without MCP, memorized indices or
repeating the initial search. Preserve optional Optimize as an alternative, not
the only visible correction route.

## D3 — Group operation controls by decisions, not parameter inventory

Keep the five tabs. Within them, use an operation-specific primary section and
secondary/refinement disclosures. Avoid a second novice/expert application mode.
Expert disclosure state should be remembered and every parameter remain reachable.

| Operation family | Primary decision content | Detail/refinement examples |
|---|---|---|
| Pocket | Actual input/region; total depth; pass strategy; stepdown/stepover; tool | Pattern-specific angle, finishing passes where optional |
| Profile | Inside/outside; total depth; through-cut consequence; holding summary | Tab geometry details, compensation, finish passes. Zero tabs remains valid with other workholding |
| 3D rough | Scope; clearing strategy; depth/load; entry choice; material to leave for the next op | Fine-stepdown/shallow-region tuning, ordering, min radius, algorithm recovery options |
| Surface finish | Tool access; declared finish target/spacing for this strategy; scope; remaining allowance | Sampling/tolerances/threshold refinements grouped by their effect, not hidden if they invalidate the target |
| Drill | Target count/positions FIRST; tool; depth; cycle and its conditional peck/R-plane controls | Nonessential cycle refinements. Targets are not “Advanced” |

Leave/entry/workholding and essential uncertainty never disappear just because
someone collapses Advanced. Do not add unsupported Pocket stock-to-leave engine
functionality as part of this layout task. Only display measured upstream
allowance when it exists; planned leave is not measured remaining material.

```text
Profile — Cut outline                         Path: not generated
Setup Top · drawing.svg · 6mm flat             [Change target]

CUT INTENT
Side [Outside]     Depth [12 mm]     Pass depth [1.2 mm]
Through cut of a 12mm board · Holding: no tabs configured
[Review holding]   Alternative holding method allowed

PATTERN / FINISH ▸             Effective heights summary → Heights
BOUNDARY SUMMARY → Geometry    Entry summary → Linking

[Generate]  →  [Inspect / verify when generated]
```

For finish operations, a nominal scallop value is a geometric target, not a
measured roughness/finish guarantee. Tool reach and sampled-grid limits remain
separate. Do not put a vendor badge on geometric quality controls unless a real
source supports it, or promise achieved quality from a universal Fine slider.

**Acceptance:** on Pocket/Profile/3D rough/finish/Drill, users can state the target,
cut depth or quality intent and relevant consequence without opening every section;
experts can still find and change the same detailed controls.

## D4 — Make value roles and Apply scope unmistakable

Fixing the raw recommendation/cap bug alone does not solve the mental task of
merging values from a field, a summary card and a larger analysis modal.

```text
FEEDS & SPEEDS                     Context: hardwood · router · medium hold
                           CURRENT PLAN     PROPOSED CHANGE
Feed                       750 mm/min       [candidate]       [Use]
Plunge                     527 mm/min       [candidate]       [Use]
RPM                        15000 override   [candidate]       [Use]

[Apply proposed speeds]   Changes feed/plunge/RPM; keeps cut geometry
Cut-geometry proposal ▸   Depth/pass [current → proposed], stepover [→]

Essential limitation: [readable missing guarantee, if any]
[Why these values?]  [Compare / explore…]  [Vendor evidence ▸]
Post-sim emitted feed behaviour → Simulation (separate evidence)
```

Candidates here are placeholders, not newly computed recommendations.
- Current plan = configured value; proposed = current calculation; inherited =
  default with named source; emitted = post-processing/modulation result.
- Keep quick single-field actions and speed-only apply. Their exact affected
  fields and constraints must be consistent; expose dependent changes if applying
  one field necessarily adjusts another.
- Core already has persisted feed-provenance types. Use reliable stamps, and show
  unknown origin honestly rather than inferring it from today's recommendation.
- Cross-tab readouts link to the authoritative editor. Analysis modal adds room
  for comparison/exploration; it is not a required second configuration pass.

**Acceptance:** user predicts what each Apply changes and keeps an intentional
manual stepover while updating speeds. They can tell proposal from configured
and emitted values without decoding lightning glyph variants.

## D5 — Give previews, edits and replacements a consistent contract

Similar-looking dialogs currently have different commit/Close semantics. Don't
force them all to discard their caches; make their consequences explicit.

```text
Multi-tool plan                 Target: Setup Top / terrain.stl [Change]
Tools + plan dials → [Preview]   Preview only — project unchanged

CHANGE SUMMARY (before Apply)
Add: [named tiers]   Replace: [named existing tiers]
Manual edits affected: [list, or not tracked / cannot establish]
Hand-authored ops kept: [scope]
[Apply these changes]   [Close preview]
```

- **Preview** changes no project output. Reopening may retain dials/cache with a
  freshness label; stale preview cannot pass as a newly evaluated candidate.
- **Apply** names affected objects and scope before commit. Don't offer “keep
  edits” or “Undo plan” until the implementation can honour it.
- **Cancel** should cancel uncommitted changes; use **Close** when committed
  changes remain. For draft tools, explicitly explain navigation-away semantics
  or adopt a consistent commit boundary after evaluating the interaction cost.
- **Library copy** and **catalog edit** are separate modes/owners. No library
  write merely to accept a project-only tool change.
- Label wizard settings “this session/export” versus “saved with project” from
  actual persistence. Persistence defects require engineering work, not nicer copy.

**Acceptance:** before planner Apply or library Save, a person can name what is
replaced, what is retained and what will survive closing/reopening the project.

## D6 — Make export a clear handoff, not a route-dependent surprise

Keep both configured wizard export and quick export. Offer a common compact
**Review export** summary from Readiness and File, with settings/details available:

```text
REVIEW EXPORT
Includes: Top setup · 2 enabled ops · tools 1,2       [Change scope]
Controller / units / work zero / tool change          [Configure…]
Checks: current / limited / not checked               [Resolve / details]
Manual setup notes and acknowledged limitations
Files to write: [names / directory / split]            [Preview G-code]
[Save review-only output]                             [Cancel]
```

The label in this sketch is for review fixtures; product text must reflect the
real destination. Quick export may keep detailed configuration collapsed, but
must disclose the same applicable checks and acknowledgement scope. A shortcut
is not permission to silently lower checks. The existing seven-step wizard stays
available for changing settings; no new compulsory seven-step journey is proposed.

**Acceptance:** from either entry point, user predicts which ops/tools/setups and
files are exported, understands missing checks and can explain physical datum
and setup changes. Passing software checks remains distinct from physical safety.

## Alternatives deliberately not selected

- No global tab/workspace rename or universal project tree without evidence.
- No rigid first-time wizard that locks experts out of free-order work.
- No “Advanced” bucket hiding workholding, targets or missing guarantees.
- No new single safety percentage or finish-quality promise.
- No automatic simulate-after-every-edit policy.
- No wholesale removal of expert per-field apply, direct export, hover detail,
  overlay shortcuts, span lock, graphs or planner previews.

## Validate the design cheaply before implementation

Use these sketches and existing fixtures for short goal-based walkthroughs:
1. Define a held outline cut and explain the through-cut consequence.
2. Accept speeds while keeping a manual stepover; predict the field changes.
3. Locate a finding, edit its actual op, recheck and return to matching evidence.
4. Re-plan a hand-edited tier; state what is replaced before Apply.
5. Compare quick versus configured export; explain scope and missing checks.

This is validation of **design assumptions**, not a prerequisite to doing this
expert assessment. Functional route verification can use event/input automation;
actual discoverability/comprehension still needs an observer/participant, and
hit-testing/focus/drag need real input rather than direct AppEvent dispatch.
