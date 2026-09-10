# Architecture consolidation plan — phases 0 to 8

> **PROVENANCE.** Produced by gpt-5.6-astra in the same pi session, 2026-09-09
> at 21:27 UTC, in answer to the operator's request for "a phased
> implementation plan to fix this all". Recovered from the transcript on
> 2026-09-10 and recorded here unedited.
>
> This plan is the WHAT. The pi command `/techdebt-orchestrate`
> (`~/.pi/agent/prompts/techdebt-orchestrate.md`) is the HOW, and it now points
> at this file.
>
> **Read `STATUS.md` beside this file first.** Work landed after 2026-09-09
> that closes part of Phase 1 and part of Phase 4A. Do not re-do it.

---


Yes—and this is manageable. **These are not twelve unrelated rewrites. Most stem from a few missing ownership boundaries.** Fix those first, and several findings disappear together.

I’d run this as **one architecture programme with independently mergeable phases**, not twelve simultaneous cleanup projects.

This is a proposed implementation plan, not implementation authorization. **No files changed or commands executed beyond read-only inspection.**

# Architecture consolidation plan

## 1. Target architecture

The intended flow is:

```text
GUI / CLI / MCP
       │
       ▼
Validated project mutations
       │
       ▼
ProjectSession
  ├─ authoritative configuration
  ├─ dependency-aware invalidation
  ├─ authoritative computed artifacts
  └─ evidence tied to those artifacts
       │
       ▼
Immutable input snapshot
       │
       ▼
Shared core resolution + execution
       │
       ├─ generation
       ├─ simulation / timing / modulation
       └─ checked export planning
```

The GUI still owns:
- asynchronous scheduling and cancellation;
- selections, visibility, and interaction;
- GPU resources and playback presentation.

It should **not independently own machining policy or authoritative machining results**.

### Programme-wide rules

1. **Extend existing abstractions before introducing replacements.** In particular: `ProjectSession`, `SetupEvalContext`, `ExecutionContext`, `ProjectEvidence`, operation registry, and provenance machinery.
2. **Separate extraction from behavior changes.** A refactor cannot quietly change feeds, geometry, thresholds, or accepted tooling.
3. **Every migration includes deleting the superseded production logic.** A shared helper with two remaining independent callers making the same decisions is not completion.
4. **Preserve project and MCP compatibility unless a change is explicitly approved.**
5. **Do not sacrifice GUI responsiveness to obtain sharing.** Expensive input resolution belongs off the frame loop too—not just generation.
6. **No blanket “all tests pass, therefore equivalent” claim.** Each phase needs assertions about the contract it changes.

---

# Phase 0 — Establish contracts and executable evidence

**Purpose:** Turn the read-only audit into a verified implementation backlog before changing architecture.

### Work

For each finding, record:
- current implementation sites and consumers;
- confirmed behavior versus suspected consequence;
- intended authoritative owner;
- behavior that must remain unchanged;
- behavior explicitly intended to change;
- evidence required to close the finding.

Build a compact representative fixture set covering:
- 2D clearing and profiling;
- 3D finishing, including disconnected regions;
- multi-operation remaining-stock chains;
- drilling and alignment-pin drilling;
- identity, flipped, and lateral setups;
- nonzero stock origins and setup datums;
- tool changes, per-operation RPM, coolant;
- modulation on/off;
- import/save/reload.

Use registry-driven coverage for operation availability rather than another handwritten operation count.

### Characterization tests to add first

- Equivalent edits through setter, undo, and optimizer application invalidate the same dependencies.
- Disabled cached operations do not enter exports.
- Core and GUI export agree on coolant, RPM, tools, and datums.
- Generation agrees across session and GUI worker entry points.
- Import and reload agree on geometry and metadata.
- Unsupported parameter writes fail rather than silently succeed.
- Disconnected finishing runs have the required retract/entry structure.
- Timing totals include drilling and remain consistent after modulation.

**Important:** Golden outputs characterize current behavior; independently stated invariants identify where current behavior is wrong. Do not freeze known bugs as desired behavior.

### Policy decisions

Recommended defaults, to approve before implementing affected behavior:

| Question | Recommendation |
|---|---|
| Export stale or missing geometry? | Refuse; display-only snapshots are not exportable. Tool-load overrides must not bypass freshness. |
| Load an invalid legacy configuration? | Preserve it as an editable draft with diagnostics; block execution until corrected. |
| Unsupported drill/tool combinations? | Explicitly refuse unsupported physical models rather than pretend they are flat cutters. |
| Broaden vendor-row substitutions? | Only with a documented physical rationale; otherwise remain unmodeled. |
| Offset failure? | Require an explicit caller decision; never implicitly treat failure as ordinary collapse. |

**Exit gate:** Every audit finding has an owner, acceptance test, and approved or explicitly blocked behavior decision.

---

# Phase 1 — Make state changes and computed artifacts authoritative

**Closes:** Findings 2 and 3.

**Primary areas:** `core/session/{mod,mutation,compute}.rs`, GUI controllers, runtime state, undo, optimizer application.

## 1A. One mutation contract

Introduce a shared mutation mechanism that:
1. validates the proposed change;
2. commits it atomically;
3. determines affected operations and evidence;
4. invalidates their outputs;
5. reports the change effects to the GUI.

Route through it:
- individual parameter edits;
- operation replacement;
- tool and stock edits;
- setup changes and operation reordering;
- undo/redo;
- optimizer and feeds application;
- bulk replacement/import.

Start conservatively where necessary. Fine-grained cache retention is secondary to correctness.

Restrict raw mutable access once callers have migrated. Bulk project loading may use a private construction path that cannot leave old results attached.

## 1B. One artifact owner

Represent these distinctions explicitly:

- **planned result**;
- **emitted result**, derived from a particular plan and feed/timing configuration;
- **stale display snapshot**;
- simulation and diagnostic evidence associated with exact inputs.

Reuse the existing result payloads and `Arc` sharing. Do not copy full toolpaths and traces into several stores.

Use stable toolpath identity at asynchronous boundaries. Results should carry sufficient identity/version information to reject completion after:
- the operation changed;
- the operation was removed or reordered;
- a different project was loaded;
- an upstream stock dependency changed.

A per-session identity plus dependency/revision stamp is safer than accepting a result by vector position.

## 1C. Migrate consumers

Move narration, export, simulation, diagnostics, and visualization onto explicit artifact accessors.

The GUI may retain stale geometry for display, but that access must be distinguishable from obtaining executable motion.

### Acceptance

- Setter, undo, and optimizer changes have equivalent dependency effects.
- Old worker completions cannot overwrite newer state.
- Narration can deliberately select planned or emitted motion.
- Repeated simulation/modulation does not progressively mutate an already-modulated plan.
- No authoritative result remains in GUI runtime.
- Existing generation status and cancellation behavior remain intact.

**Split into three integration checkpoints:** mutations → artifact storage → consumer migration. Do not attempt all three in one unreviewable patch.

---

# Phase 2 — Centralize export planning

**Closes:** Finding 4.  
**Depends on:** Phase 1 artifact access.

**Primary areas:** `core/gcode`, session export, `viz/io/export.rs`, CLI export callers.

### Work

Create a core export-plan builder accepting:
- an explicit selection: project, setup, or toolpath;
- authoritative result references;
- post-processing settings;
- export policy and applicable evidence.

It owns:
- enabled-operation filtering;
- ordering and setup grouping;
- freshness and completeness;
- tool identity and tool changes;
- per-operation RPM and coolant;
- pre/post blocks;
- compensation selection;
- export datum transforms;
- single-file versus split-setup assembly.

All frontends consume this plan. They may choose filenames and present operator prompts, but cannot independently rebuild machining phases.

Gate evaluation must describe the artifacts actually exported, while retaining applicable setup/project checks.

### Acceptance

For equivalent settings, GUI, CLI, and MCP produce equivalent programs.

Explicitly test:
- disabled results retained in cache;
- missing and stale results;
- selection of a subset;
- repeated and distinct tools;
- all supported post formats;
- coolant and RPM overrides;
- flips, lateral setups, and datums;
- split setup files.

**Delete:** duplicate phase builders, compensation selection, and result-selection policy.

---

# Phase 3 — Unify the full generation pipeline

**Closes:** Finding 1.  
**Depends on:** Phase 1; integrate Phase 4 validation before final cutover.

**Primary areas:** `core/session/compute.rs`, `core/compute`, GUI worker/controller generation code.

### Work

Evolve the existing `ResolvedGenInputs` into a core-owned request boundary rather than inventing another parallel context.

Use three stages:

```text
Capture project inputs
        ↓
Resolve geometry, stock, boundaries, tools, heights
        ↓
Execute operation + shared finishing steps
```

The captured inputs are immutable. Expensive resolution and execution can run on a worker without holding mutable session state.

One executor owns:
1. preconditions;
2. generator dispatch;
3. dressups;
4. boundary enforcement;
5. entry-descent processing;
6. empty-result classification;
7. span/trace reconciliation;
8. findings and statistics;
9. result packaging.

The existing stage order is preserved during extraction. Any later reordering is a separate behavioral change.

Resolve boundary semantics explicitly:
- absent;
- valid and nonempty;
- legitimately empty;
- collapsed;
- failed.

Do not replace these with a single optional polygon or “empty means no clipping.”

### Migration

1. Build the shared request/executor behind the session path.
2. Compare it against the existing worker path.
3. Switch the GUI worker to the shared executor.
4. Remove the worker’s machining-policy implementation.
5. Integrate strategy-advisor and optimizer callers at the appropriate shared boundary without losing their isolated-candidate semantics.

### Acceptance

- Same input produces equivalent motion, spans, findings, and stock consumption across frontends.
- Feed-optimization dressup availability is consistent.
- Remaining-stock fixpoint chains still resolve.
- Cancellation works during both resolution and execution.
- No expensive new work lands on the GUI frame loop.
- Boundary and entry safety sentries remain effective.

---

# Phase 4 — Unify importing and parameter validity

**Closes:** Findings 6 and 7.  
**Parallel opportunity:** Much of this can proceed alongside Phase 1 after Phase 0 contracts are agreed.

## 4A. Canonical model import

**Areas:** `core/io.rs`, project loading, GUI import adapters.

Create one imported-geometry bundle containing:
- mesh and BREP topology;
- polygons;
- drill targets and layers;
- units/scale provenance;
- winding/import diagnostics.

Interactive import and project reload both consume it.

Keep path resolution, missing-file recovery, persisted names, and user selections in their appropriate wrappers. Consolidate geometry interpretation, not all persistence logic.

**Acceptance:** All supported formats agree between import and reload, including unit scaling, STEP topology, DXF targets, and layer metadata.

## 4B. Canonical parameter contracts

**Areas:** operation registry, `OperationParams`, mutation APIs, GUI parameter controls.

Separate:
- **support:** does this operation possess this parameter?
- **hard validity:** is the value meaningful and representable?
- **contextual validity:** is it valid for this tool/setup?
- **recommendation:** what value should the operator use?
- **UI range:** what is convenient to manipulate?

Make unsupported mutation fallible. Eliminate successful no-op setters.

Reuse shared hard validity for GUI commits, CLI/MCP writes, and execution preparation. Deserialization may preserve invalid legacy drafts, but execution must not bypass validation.

Cover every registered parameter with either a declared rule or an explicit reviewed disposition. Do not mechanically assume every numeric field must be positive.

**Acceptance:** Equivalent inputs receive equivalent validation; invalid writes are atomic; valid manual overrides remain possible without invoking Suggest.

---

# Phase 5 — Complete shared operation-level contracts

**Closes:** Findings 8, 9, and 10.  
**Depends on:** Phase 3 executor; Phase 4 validation.

Three separable work packages:

## 5A. Resolve drilling once

Create one validated `DrillPlan` from which both motion and analytical removal are derived.

It owns:
- resolved hole coordinates;
- top/bottom depths;
- cycle and R-plane;
- tool/profile interpretation;
- feed/RPM context.

Preserve the existing shared cycle expansion. Do not replace it with another implementation.

**Acceptance:** Analytical holes and emitted motion agree for ordinary drilling, picked targets, pins, flips, and lateral setups. Unsupported tool profiles cannot silently become flat tools.

## 5B. Shared annotated run emission

Extend the existing motion emitter to support annotation hooks or returned move ranges.

Migrate manual equivalents, starting with spiral finishing. Keep path sampling and traversal strategy operation-specific.

**Acceptance:** Every disconnected run has the intended approach, plunge, body, and retract structure; annotations and spans remain attached to the correct moves.

The suspected spiral transition defect needs reproduction before its fix is characterized.

## 5C. Structured offset outcomes throughout production

Make geometry plus failure information the normal offset result.

Migrate callers by family, documenting their treatment of:
- valid output;
- legitimate collapse;
- partial output with failure;
- complete failure.

Generation findings must receive failures from composed operations as well as top-level calls.

**Acceptance:** No production caller accidentally interprets a library failure as normal geometric emptiness. Intentional geometry-only consumption must explicitly acknowledge that policy.

---

# Phase 6 — Unify timing evidence and machining-use policy

**Closes:** Findings 5 and 11.  
**Depends on:** Phase 1 artifacts; Phase 5 drilling for final integration.

## 6A. One timing view, multiple aggregations

Build an authoritative timing view associated with a particular motion artifact and machine configuration.

It must distinguish:
- commanded versus achieved timing;
- planned versus emitted feeds;
- cutting, rapid, retract, and dwell contributions;
- modelled versus unavailable timing.

Use that view to derive:
- toolpath and project totals;
- semantic and structural-span summaries;
- hotspots and per-kinematics times;
- air/low-engagement runtime;
- runtime-derived MRR.

Do not overwrite selected totals afterward while leaving their constituent summaries untouched.

Where exact sample timing is unavailable, publish the approximation used. Do not make internally consistent numbers look more physically precise than the model supports.

**Acceptance**
- Totals reconcile over the same population and time basis.
- Drill motion is never lost because it lacks milling engagement samples.
- Dwell is neither omitted from a claimed complete cycle nor double-counted.
- Modulation on/off and repeated runs remain coherent.
- Intentionally command-weighted engagement metrics remain explicitly identified.

## 6B. Explicit vendor lookup use

Extend the existing `lut_query_for` contract with a structured resolved use/rationale rather than another resolver.

Publish:
- requested operation/tool/pass use;
- resolved vendor family and role;
- substitution rationale;
- evidence or refusal.

Preserve current routing first. Broaden substitutions only in separately reviewed policy changes.

**Acceptance:** Suggest, load gates, optimizer, explanation, and overlays agree on the resolved lookup. Unsupported cases remain visibly unmodeled.

---

# Phase 7 — Consolidate cache mechanics and remove migration remnants

**Closes:** Finding 12; completes architectural cleanup.

### Work

Extract the small shared weak-identity cache mechanism used by reach and tier maps:
- liveness-checked identity;
- bounded storage and eviction;
- locking;
- counters;
- lookup/insert behavior.

Keep domain-specific keys and computations separate. Preserve content-based keying where identity-based keying is inappropriate.

Expose a suitable geometry/request identity API for the reach scheduler so unrelated project edits do not invalidate the overlay.

Then remove:
- obsolete adapters containing business logic;
- duplicate result synchronization;
- superseded parameter and import paths;
- redundant dispatch fallbacks left from completed migrations;
- stale comments claiming two implementations must mirror one another.

Public compatibility wrappers may remain if needed, but must delegate without reimplementing policy.

### Acceptance

- Cache semantics, cancellation, and bounds remain tested.
- No accidental mesh-identity or tool-shape collisions.
- No cache-growth or major cloning regression.
- Each retired implementation has no production callers.

---

# Phase 8 — Whole-system acceptance and closeout

**Purpose:** Prove this improved the architecture, not just moved code.

### Functional gate

Exercise the representative workflows end to end through:
- core/session;
- CLI;
- GUI worker/controller;
- MCP adapters.

Compare motion and physical meaning, not only display strings.

### Architectural gate

Demonstrate that:
- a new generation-wide post-step requires one implementation;
- a new export-phase property requires one phase-builder change;
- a new import format has one geometry-loading implementation;
- a supported parameter cannot silently ignore writes;
- mutation callers cannot forget dependency invalidation;
- timing aggregations cannot silently consume different clocks;
- stale display geometry cannot become executable output.

### Verification

During implementation—not this read-only planning session—run:
- smallest relevant tests per work package;
- full per-crate gates at phase completion;
- core heavy-tests;
- workspace Clippy with heavy-tests enabled;
- `cargo fmt --all -- --check`;
- relevant existing regression sentries and GUI/MCP smoke checks.

Document intentional output changes separately from refactor parity results. Performance comparisons must hold model, tools, settings, and simulation resolution constant.

Update architecture documentation, `FEATURE_CATALOG.md`, and `planning/PROGRESS.md`. Update `CREDITS.md` if any physical model or external reference changes.

---

# How I would divide this among AI agents

**Large jobs are appropriate; overlapping ownership is not.**

| Workstream | Ownership |
|---|---|
| State/lifecycle | Phase 1; authoritative mutation and artifact APIs |
| Generation | Phase 3; then operation-level integrations |
| Export | Phase 2, once artifact access is available |
| Import/validation | Phase 4; importing can start early |
| Measurement | Phase 6; timing contracts can be designed early |
| Shared utilities | Phase 7 cache extraction |
| Independent reviewer/integrator | Contract review, evidence, cross-stream integration |

Rules:
- One designated owner for heavily shared files such as `session/compute.rs`.
- Agree interfaces before dependent agents implement against them.
- Use isolated worktrees and explicit integration checkpoints.
- Run repository-wide verification on the integrated tree, not only individual branches.
- No agent marks its work complete while leaving callers on the old behavior.

### Required work-package handoff

Each agent receives:

```text
Objective
Owned files and interfaces
Required dependencies
Invariants to preserve
Approved behavior changes
Forbidden scope expansion
Acceptance tests
Old production code to remove
Required evidence and documentation
```

That final **“code to remove”** section is important: it prevents another partial consolidation.

---

## Coverage of the original audit

| Finding | Closing phase |
|---|---|
| 1. Split generation orchestration | 3 |
| 2. Inconsistent invalidation | 1 |
| 3. Competing result owners | 1 |
| 4. Duplicate export assembly | 2 |
| 5. Partial timing corrections | 6 |
| 6. Duplicate importing | 4 |
| 7. Fragmented parameter contracts | 4 |
| 8. Reconstructed drill model | 5 |
| 9. Bypassed shared emitters | 5 |
| 10. Opt-in offset failures | 5 |
| 11. Incident-specific vendor routing | 6 |
| 12. Duplicate cache infrastructure | 7 |

**My recommendation:** approve the overall architecture, then start with **Phase 0 and Phase 1**, while import consolidation proceeds independently. The biggest win is eliminating “remember to update the other path” as a development requirement—not maximizing the amount of generic code.