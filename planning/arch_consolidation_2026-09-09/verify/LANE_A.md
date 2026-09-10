# Lane A verification — audit findings 1, 3 and 4

**Method.** I READ the tree at `HEAD` on the branch
`machine-kinematics-confidence` (98 commits after the audit commit
`627ef997`). I ran no build, no test and no application. Every line
number below is today's. Where the audit's own citation moved, I say so.
Nothing here is an execution result.

## Summary

| # | Finding | Verdict |
|---|---------|---------|
| 1 | Core and GUI assemble separate generation pipelines | **STILL TRUE** |
| 3 | Computed results have competing owners | **PARTLY CLOSED** — the export fallback is guarded; the ownership split is not |
| 4 | Export shares the emitter but duplicates program assembly | **STILL TRUE, and the audit undercounts the sites** |

One live defect follows from finding 4 and is recorded inside it.
Three smaller items are in "Found while verifying".

---

## Finding 1 — Core and GUI still assemble separate generation pipelines

### 1. Verdict

**STILL TRUE.**

### 2. Evidence at today's tree

There are exactly **two** generation assembly sites.

**Site A — core.** `ProjectSession::generate_toolpath`,
`crates/rs_cam_core/src/session/compute.rs:1412`. It calls
`ProjectSession::resolve_generation_inputs`
(`crates/rs_cam_core/src/session/compute.rs:1036`), which returns the
17-field private struct `ResolvedGenInputs`
(`crates/rs_cam_core/src/session/compute.rs:147`).

**Site B — viz.** `run_compute_with_phase_tracker`,
`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:605`. It calls
`generate_via_core`
(`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:43`) and the viz
`apply_dressups` wrapper
(`crates/rs_cam_viz/src/compute/worker/helpers.rs:24`). Its input is the
44-field `ComputeRequest`
(`crates/rs_cam_viz/src/compute/worker.rs:41`), which the controller
builds in `submit_toolpath_compute`
(`crates/rs_cam_viz/src/controller/events/compute.rs:181`, about 570
lines to the next function at `:829`).

**The audit's cited locations.** `core/session/compute.rs:1404` was
`pub fn generate_toolpath` at `627ef997`; it is now `:1412`.
`viz/compute/worker/execute/mod.rs:603` was
`run_compute_with_phase_tracker`; it is now `:605`.
`viz/compute/worker/helpers.rs:24` has not moved.

**MCP is NOT a third pipeline.** `mcp_generate_toolpath` pushes the
GUI's own event: `crates/rs_cam_viz/src/app/mcp.rs:5489` reads
`.push(crate::ui::AppEvent::GenerateToolpath(tp_id));`. MCP
`generate_all` calls `self.controller.mcp_start_generate_all(`
(`crates/rs_cam_viz/src/app/mcp.rs:5512`). Both therefore run site B.

**`ResolvedGenInputs` is private and has no viz consumer.** A workspace
grep for `ResolvedGenInputs|resolve_generation_inputs` returns hits only
in `crates/rs_cam_core/src/` and `crates/rs_cam_core/tests/`. Site A's
non-test callers are the CLI (`crates/rs_cam_cli/src/project.rs:304` and
`:342`, `crates/rs_cam_cli/src/job.rs:650`,
`crates/rs_cam_cli/src/run.rs:149`, `crates/rs_cam_cli/src/smoke.rs:593`
and `:791`), the tool-load optimizer
(`crates/rs_cam_core/src/tool_load/optimize/candidate.rs:426`,
`crates/rs_cam_core/src/tool_load/optimize/mod.rs:891`) and the bench
(`crates/rs_cam_core/benches/hot_paths.rs:1109`).

**The two sites describe themselves as mirrors.** A grep for
`Mirrors|mirror|matches the core session path|session/compute.rs|twin`
over the viz worker returns 18 comment lines, forming **14 distinct
comments** — 12 in `execute/mod.rs` and 2 in `helpers.rs`. Examples,
verbatim:

- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:139` —
  `// tool_radius, silhouette]. Mirrors session/compute.rs::resolve_containment_polygon.`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:696` —
  `//    session/compute.rs::apply_boundary_clip.`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:916` —
  `// session/compute.rs::generate_toolpath's post-clip wiring. Runs`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:936` —
  `// G-ISOCLIPENTRY — the session's twin (`session/compute.rs`). A`

Sharing is partial, not absent. The viz worker calls the core function
`ProjectSession::apply_boundary_clip_multi` for the multi-region clip
(`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:741`) and
open-codes the single-polygon clip beside it (`:760`–`:908`).

**The concrete divergence the audit names is STILL LIVE, and it is live
on the DEFAULT configuration.**

- `DressupConfig::default()` sets `feed_optimization: true`
  (`crates/rs_cam_core/src/compute/config.rs:2007`).
- Site B builds the stock:
  `crates/rs_cam_viz/src/compute/worker/helpers.rs:41`–`:52`
  (`let mut feed_opt_stock = None;` then
  `match feed_optimization_stock(req) {`).
- Site A passes a literal `None` in that argument position:
  `crates/rs_cam_core/src/session/compute.rs:1658`.
- The shared implementation needs the stock:
  `crates/rs_cam_core/src/compute/execute.rs:4344` reads
  `if cfg.feed_optimization` and the next line pattern-matches
  `(feed_opt_stock, cutter)`, so `None` skips the feature silently.

I read the code; I did not run it. On the reading, a project with the
default dressups gets the feed-optimisation dressup on the GUI and MCP
route and does not get it on the CLI, optimizer and bench route.

### 3. What changed since the audit

Nothing structural. `git log 627ef997..HEAD` over
`crates/rs_cam_core/src/session/compute.rs` and the viz worker shows no
convergence commit. The named recent work does not touch this finding:
G-FRESHSTATE (`5765a179`) and the F2 programme act on invalidation and
freshness, F4.3/F4.4/F4.7 act on model import and refresh, and
G-DEPTHSTOCKCORE (`6484b527`, `ab98b5c1`) acts on the depth caution.

### 4. Is the recommendation still the right shape?

**Yes, and it is cheaper than the audit implies.** The audit asks for
"a core-owned, immutable resolved generation request and one complete
executor". `ResolvedGenInputs`
(`crates/rs_cam_core/src/session/compute.rs:147`) is already that struct,
half-built: it is core-owned and fully owning. The work is to make it
public, give it a constructor the viz controller can call, and move the
executor tail of `generate_toolpath` beside it. Do not invent a new type.

One audit-era assumption is now stale and makes the feed-optimisation
half of the fix small. The viz doc comment at
`crates/rs_cam_viz/src/compute/worker/helpers.rs:19`–`:23` says the
availability check "must run here because core has no knowledge of
`state::toolpath`". The check now lives in core:
`feed_optimization_unavailable_reason`,
`crates/rs_cam_core/src/compute/catalog.rs:2730`. The viz path reaches it
through a re-export (`crates/rs_cam_viz/src/state/toolpath.rs:9`). Only
the `TriDexelStock::from_bounds` call is viz-side, and it is five lines
(`crates/rs_cam_viz/src/compute/worker/helpers.rs:114`–`:118`). Core can
build the same stock in about six lines.

### 5. Size estimate

Two crates: `rs_cam_core` and `rs_cam_viz`. `rs_cam_cli` needs no edit
if `generate_toolpath` keeps its signature.

Counted code:

| Unit | Path and lines | Size |
|---|---|---|
| `ResolvedGenInputs` | `rs_cam_core/src/session/compute.rs:147` | 17 fields |
| `resolve_generation_inputs` | `rs_cam_core/src/session/compute.rs:1036`–`:1408` | 372 lines |
| `generate_toolpath` | `rs_cam_core/src/session/compute.rs:1412`–`:2110` | about 698 lines |
| `ComputeRequest` | `rs_cam_viz/src/compute/worker.rs:41` | 44 fields |
| `submit_toolpath_compute` | `rs_cam_viz/src/controller/events/compute.rs:181`–`:760` | about 570 lines |
| `generate_via_core` | `rs_cam_viz/src/compute/worker/execute/mod.rs:43`–`:287` | 244 lines |
| `run_compute_with_phase_tracker` | `rs_cam_viz/src/compute/worker/execute/mod.rs:605`–`:1130` | about 525 lines |
| viz `apply_dressups` + `feed_optimization_stock` | `rs_cam_viz/src/compute/worker/helpers.rs:24`–`:122` | 98 lines |

Nine non-test call sites of `ProjectSession::generate_toolpath` and
`generate_all` would need re-checking, plus one bench.

This is the largest of the three findings by a wide margin.

---

## Finding 3 — Computed results have competing owners

### 1. Verdict

**PARTLY CLOSED.** The audit's own stated architectural problem — the
display cache as an implicit export fallback — is CLOSED. The ownership
split it rests on is OPEN, and is now deliberate.

### 2. Evidence at today's tree

**The two stores still exist.** `ToolpathRuntime::result`,
`crates/rs_cam_viz/src/state/runtime.rs:32` (the audit's `:28` is the
same struct; the field moved by four lines). `ProjectSession::results`,
written through `insert_result` from the viz controller at
`crates/rs_cam_viz/src/controller/events/compute.rs:973`.

**The export fallback still exists, and I name the line.**
`crates/rs_cam_viz/src/io/export.rs:206`:

```
    Some(gui.toolpath_rt.get(&tc.id)?.result.as_ref()?.toolpath())
```

It is now guarded three lines above, at
`crates/rs_cam_viz/src/io/export.rs:203`:

```
    if !stale.accepts_previous_geometry() {
        return None;
    }
```

`StaleResultPolicy::Refuse` carries `#[default]`
(`crates/rs_cam_viz/src/state/runtime.rs:289`–`:290`). I traced every
writer of `gui.stale_export`: there is exactly one,
`AppEvent::SetStaleExportPolicy` at
`crates/rs_cam_viz/src/app/input.rs:241`, raised by the pre-flight
modal's checkbox (`crates/rs_cam_viz/src/ui/preflight.rs:417`–`:426`).
A project load builds a fresh `GuiState`
(`crates/rs_cam_viz/src/controller/io.rs:318` and `:457`), so the policy
resets to `Refuse` on load. The MCP route passes the policy explicitly
rather than reading GUI state
(`crates/rs_cam_viz/src/app/mcp.rs:3209`).

**The refusal path.** `emitted_toolpaths`
(`crates/rs_cam_viz/src/io/export.rs:144`) collects `blocking_toolpaths`
(`:272`) and returns `VizError::Export` when any row is not waived
(`:159`–`:163`). `blocking_toolpath_message` (`:245`) is the one text
builder for the GUI, the pre-flight modal and MCP.

**The freshness view is new and is core-anchored.**
`crates/rs_cam_viz/src/state/freshness.rs:1`–`:12` states the rule:
"The core cache is now the truth about result validity; the GUI adds
only what the core cannot know". `freshness` at `:79` derives
`Current` from `core_has_result` (`:91`) and `EditedSince` from the
retained GUI result (`:92`).

**The split is now DELIBERATE, and a merge must respect that.**
`drain_compute_results` writes the GUI store unconditionally
(`crates/rs_cam_viz/src/controller/events/compute.rs:975`,
`rt.result = Some(computed);`) but withholds the core write when the
inputs moved during the job (`:934`–`:973`, the `answers_current_inputs`
guard, G-LATERESULT / F2.4). The comment at `:908`–`:918` says the
result is kept "so the viewport still draws it and the operator does not
lose a long 3D generation". The core store also deliberately omits the
debug and semantic traces (`:966`–`:971`).

**MCP narration still reads the GUI store.** The wording the audit calls
a symptom is at `crates/rs_cam_viz/src/app/mcp.rs:1553`:
"this handler narrates `state.gui.toolpath_rt`, the worker's
pre-modulation IR, which a simulation never rewrites". The narration
function is now `mcp_narrate_toolpath` at
`crates/rs_cam_viz/src/app/mcp.rs:1369` (the audit cited `:1288`, which
was this function at `627ef997`).

**Correction to the audit's wording.** The audit says export "prefers
session results but falls back to GUI results, including results
retained for stale display". That was true at `627ef997`. It is no
longer an unconditional preference-plus-fallback; it is a preference
plus an operator-gated waiver, and the waiver is refused by default.

### 3. What changed since the audit

- G-STALEXPORT, `66d2c232` — an edited operation blocks the file. This
  is the commit that closes the audit's stated problem.
- G-EXPORTSKIP, `cbe20d06` — an enabled operation with no result refuses
  instead of dropping out of the program.
- G-FRESHSTATE / F2.1, `5765a179` — one derived `FreshnessState`, read
  from the core result cache.
- G-FRESHRENDER, `6b39feaf` — every surface draws that state.
- G-LATERESULT / F2.4, `9544ca75` — a late result is stored but not made
  current. This is what turned the split from an accident into a
  contract.
- G-STICKYEMPTY — both caches are cleared on a failed generation
  (`crates/rs_cam_viz/src/controller/events/compute.rs:994`–`:1005`, and
  the core twin at `crates/rs_cam_core/src/session/compute.rs:1438`).

### 4. Is the recommendation still the right shape?

**Partly. The ground has moved under half of it.**

The audit asks for "one authoritative result store, with explicit views
such as planned, emitted, and stale display snapshot". Three notes:

1. The **view vocabulary already exists in part**.
   `FeedsProvenance::{Planned, Emitted}` is core
   (`rs_cam_core::kinematic_utilization`), and `FreshnessState`
   (`crates/rs_cam_viz/src/state/freshness.rs:26`) is the derived view
   the audit asks for, minus the store merge. Extend these; do not open
   a parallel vocabulary.
2. A **single store must still hold a result the authoritative view
   refuses**, or G-LATERESULT breaks. The "stale display snapshot" view
   is therefore mandatory, not optional.
3. The **stores hold different payloads**. The core result carries no
   debug or semantic trace by design
   (`crates/rs_cam_viz/src/controller/events/compute.rs:966`–`:971`).
   A merge must decide where the traces live before it starts.

The export half of the recommendation is already delivered. Scope the
phase against the remaining consumers, not against the export.

### 5. Size estimate

Two crates: `rs_cam_viz` (nearly all of it) and `rs_cam_core` (the store
and the view types).

Counted: `toolpath_rt` appears **180 times** in
`crates/rs_cam_viz/src`. Excluding the four test files
(`controller/tests.rs`, `controller/workflow_tests.rs`,
`controller/results_parity_tests.rs`,
`controller/holder_clearance_scope_g_holderscope.rs`) leaves **117
references across 32 files**. The heaviest are
`crates/rs_cam_viz/src/app/mcp.rs` (19),
`crates/rs_cam_viz/src/state/simulation.rs` (17) and
`crates/rs_cam_viz/src/controller/events/compute.rs` (14). Only **8** of
the 117 touch the `.result` field; the rest read status, visibility,
traces or feeds.

`get_result(` has **24** non-test call sites across the workspace.

A store merge is therefore a wide but shallow edit: about 125 sites,
most of them one-line field reads.

---

## Finding 4 — Export shares the emitter but duplicates program assembly

### 1. Verdict

**STILL TRUE.** The audit is right, and it **undercounts**: there are
three assembly sites, not two. One of its three concrete differences is a
live defect, not just a divergence.

### 2. Evidence at today's tree

**Site 1 — core.** `export_gcode_checked`,
`crates/rs_cam_core/src/gcode/mod.rs:300` (the audit's citation; it has
not moved). It selects results at `:324`–`:333` and builds `GcodePhase`
records at `:335`–`:370`.

**Site 2 — viz (GUI and MCP).** `gcode_phase_for_session_toolpath`,
`crates/rs_cam_viz/src/io/export.rs:317`, fed by `emitted_toolpaths`
at `:144`. The audit's `:118` and `:142` were these two functions at
`627ef997`; both moved, and the lines the audit cites are doc-comment
text today. Four exported entry points share them:
`export_gcode_from_session_with_policy` (`:403`),
`export_combined_gcode_from_session` (`:446`),
`export_single_toolpath_from_session` (`:515`) and
`export_setup_gcode_from_session_with_policy` (`:596`). Every MCP export
routes here (`crates/rs_cam_viz/src/app/mcp.rs:3243` and `:3317`).

**Site 3 — CLI job-file pipeline. The audit does not mention this one.**
`crates/rs_cam_cli/src/main.rs:534` (per-setup arm) and
`crates/rs_cam_cli/src/main.rs:599` (single-file arm) each build their
own `GcodePhase` list from `job_result.phases`. Both hardcode
`pre_gcode: None`, `post_gcode: None` and
`controller_compensation: None`, and both pass an empty load report
(`ToolLoadReport { per_toolpath: vec![] }`,
`crates/rs_cam_cli/src/main.rs:581` and `:623`). The CLI `project`
subcommand is different: it routes to site 1
(`crates/rs_cam_cli/src/project.rs:604`).

**The post-processor IS shared, as the audit says.**
`crates/rs_cam_core/src/gcode/program_builder.rs` takes `&[GcodePhase]`
and `&[GcodeSetupPhase]` and emits the Statement IR
(`build_single` / `build_phased` / `build_multi_setup`,
`crates/rs_cam_core/src/gcode/program_builder.rs:1`–`:13`). It is below
the plan boundary, not a plan builder. The gate chokepoint is shared too:
every site funnels into `export_gcode_phases_checked`
(`crates/rs_cam_core/src/gcode/mod.rs:745`) or its overlay twin at
`:768`.

**The three concrete differences, checked one at a time.**

- **Coolant. TRUE.** Core hardcodes it:
  `crates/rs_cam_core/src/gcode/mod.rs:364` reads
  `coolant: CoolantMode::Off,`. Viz reads the config:
  `crates/rs_cam_viz/src/io/export.rs:341` reads `coolant: tc.coolant,`.
  The CLI reads its own job phase:
  `crates/rs_cam_cli/src/main.rs:547` and `:613` read
  `coolant: phase.coolant,`.

- **Disabled operations. TRUE, and it is a LIVE DEFECT.** Viz filters:
  `crates/rs_cam_viz/src/io/export.rs:168`–`:170` reads
  `if !tc.enabled { return None; }`. Core does not:
  `crates/rs_cam_core/src/gcode/mod.rs:328` reads
  `.filter_map(|(idx, _)| {` and the next line is
  `let result = project.get_result(idx)?;` — the `ToolpathConfig` is
  discarded, so `enabled` is never consulted. I then read the
  invalidation side and confirmed the audit's premise: disabling an
  operation KEEPS its cached result.
  `ProjectSession::set_toolpath_enabled`
  (`crates/rs_cam_core/src/session/mutation.rs:327`) calls
  `self.invalidate_output_dependents(index, true);` at `:339`, not
  `invalidate_result_chain`; the doc at
  `crates/rs_cam_core/src/session/mutation.rs:242`–`:246` states the
  intent — "the toggled op's own result stays valid for a future
  re-enable".

  **Consequence, from reading only:** on the two routes that use site 1
  — `rs_cam_cli project --emit-gcode`
  (`crates/rs_cam_cli/src/project.rs:604`) and `rs_cam_cli run`
  (`crates/rs_cam_cli/src/run.rs:173` through
  `ProjectSession::export_gcode_with_policy`,
  `crates/rs_cam_core/src/session/compute.rs:4182`) — a DISABLED
  operation that was generated before it was disabled is emitted into the
  G-code file. The GUI and MCP routes are not affected. I found no test
  that pins this behaviour in either direction. **I did not run the CLI.
  This is a read, not a reproduction.** It needs a reproduction before it
  is treated as confirmed.

- **Controller compensation implemented twice. TRUE.**
  `controller_comp_for_project_toolpath`,
  `crates/rs_cam_core/src/gcode/mod.rs:795`. 
  `controller_comp_for_session_toolpath`,
  `crates/rs_cam_viz/src/io/export.rs:345`. I read both bodies. The four
  match arms are identical in both, over the same
  `(ProfileSide, climb)` pairs. They differ only in import style and in
  `return Some(match ...)` versus `let dir = match ...; return Some(dir)`.

**One difference the audit does not name.** Spindle RPM resolution is
already shared: both sites call
`rs_cam_core::compute::catalog::effective_spindle_rpm`
(`crates/rs_cam_core/src/gcode/mod.rs:358`,
`crates/rs_cam_viz/src/io/export.rs:332`). The CLI job path does not; it
carries `phase.spindle_speed` from its own job pipeline.

### 3. What changed since the audit

Only on the viz side, and only in the selection stage:

- G-EXPORTSKIP, `cbe20d06` — the viz collect stopped being a silent
  `filter_map` over missing results.
- G-STALEXPORT, `66d2c232` — the viz collect gained the freshness
  refusal.

Neither touched core site 1 or CLI site 3. The gap between the sites is
therefore **wider** than it was at the audit, not narrower: viz now
refuses three classes of operation that core still emits without
comment.

### 4. Is the recommendation still the right shape?

**Yes, and it is now more urgent than the audit judged.** The audit asks
for "a core export-plan builder owning operation selection, setup
grouping, result freshness, tool identity, RPM, coolant, and datum
handling", with the GUI and CLI supplying selection and policy only.

Two adjustments to the shape:

1. **The plan builder must take the freshness policy as an input.** The
   viz side now owns real policy (`StaleResultPolicy`,
   `ToolLoadExportPolicy`) and a real refusal vocabulary
   (`BlockingToolpath`, `crates/rs_cam_viz/src/io/export.rs:219`;
   `blocking_toolpath_message`, `:245`). Those types are the plan
   builder's selection contract and belong in core. `FreshnessState`
   itself is currently viz-side
   (`crates/rs_cam_viz/src/state/freshness.rs:26`) and would move or
   split with it. That couples this phase to finding 3.
2. **Scope it to three sites, not two.** The CLI job-file pipeline
   (`crates/rs_cam_cli/src/main.rs:534`, `:599`) feeds phases from
   `job_result.phases`, not from a `ProjectSession`, so it cannot
   consume a session-shaped plan builder unchanged. Decide early whether
   it adopts the builder or is declared out of scope. Do not discover
   this mid-phase.

### 5. Size estimate

Three crates: `rs_cam_core`, `rs_cam_viz`, `rs_cam_cli`.

Counted call sites and units:

| Unit | Path and lines | Size |
|---|---|---|
| core `export_gcode_checked` | `rs_cam_core/src/gcode/mod.rs:300`–`:376` | 77 lines |
| core `controller_comp_for_project_toolpath` | `rs_cam_core/src/gcode/mod.rs:795`–`:811` | 17 lines |
| viz `emitted_toolpaths` + `emitted_result_toolpath` | `rs_cam_viz/src/io/export.rs:144`–`:207` | 64 lines |
| viz `blocking_toolpaths` + messages | `rs_cam_viz/src/io/export.rs:209`–`:315` | 107 lines |
| viz `gcode_phase_for_session_toolpath` + comp | `rs_cam_viz/src/io/export.rs:317`–`:360` | 44 lines |
| CLI job-file phase builders | `rs_cam_cli/src/main.rs:534`–`:548`, `:599`–`:614` | 28 lines |

Callers to re-point:

- 4 viz export entry points (`rs_cam_viz/src/io/export.rs:403`, `:446`,
  `:515`, `:596`), reached from 8 non-test call sites
  (`ui/export_wizard.rs:763`, `:905`; `controller/io.rs:490`;
  `app/input.rs:244`, `:289`; `app/export.rs:27`, `:144`, `:183`) plus 2
  MCP call sites (`app/mcp.rs:3243`, `:3317`).
- 2 core session wrappers
  (`rs_cam_core/src/session/compute.rs:4174`, `:4182`).
- 2 CLI call sites (`rs_cam_cli/src/project.rs:604`,
  `rs_cam_cli/src/run.rs:173`) plus the 2 job-file arms above.
- 4 core test files call `export_gcode_checked` or
  `export_gcode_phases_checked` directly.

About 20 call sites and roughly 340 lines of assembly code. This is the
smallest of the three findings, and the best first phase.

---

## Found while verifying

These are outside the three findings. I did not fold any of them into a
verdict.

**A. A stale comment hides the feed-optimisation divergence.**
`crates/rs_cam_core/src/session/compute.rs:1662` reads:

```
                    // path — `feed_opt_stock` above stays `None` and keeps it
```

There is no `feed_opt_stock` binding anywhere in
`crates/rs_cam_core/src/session/compute.rs` — a grep for
`feed_opt|feed_optimization` in that file returns this one comment line
and nothing else. The argument at `:1658` is a bare literal `None`. A
reader who trusts the comment will look for a decision that was never
written. Any fix for finding 1 should replace this comment with a
statement of the divergence, or remove the divergence.

**B. A stale doc comment overstates the viz coupling.**
`crates/rs_cam_viz/src/compute/worker/helpers.rs:19`–`:23` reads: "The
viz layer pre-builds the feed-optimization stock (which depends on
GUI-only `state::toolpath` operation-availability checks)". The check is
core-owned at
`crates/rs_cam_core/src/compute/catalog.rs:2730`
(`pub fn feed_optimization_unavailable_reason`); viz reaches it through a
re-export at `crates/rs_cam_viz/src/state/toolpath.rs:9`. The comment
makes the fix look harder than it is.

**C. A third reader of the display store, in the post-export summary.**
`crates/rs_cam_viz/src/app/export.rs:323` reads
`gui.toolpath_rt.get(&tc.id).and_then(|r| r.result.as_ref())`
unconditionally, to total `move_count` and `cutting_distance` for the
export log line. It does not consult `session.results` and does not
consult `FreshnessState`. It therefore describes the pre-modulation
display copy, not the program that was just written. The move count and
cutting distance are unlikely to differ, so I rate this cosmetic — but it
is a third consumer choosing a store for itself, which is the pattern
finding 3 is about.

**D. `stale_export` and `tool_load_overrides` reset by construction, not
by an explicit reset.** Both doc comments say "Reset on project load"
(`crates/rs_cam_viz/src/state/runtime.rs:312`, `:315`–`:316`). I found no
assignment that resets them. They reset because
`open_job_from_path` builds a fresh `GuiState`
(`crates/rs_cam_viz/src/controller/io.rs:318`, `:457`). The behaviour is
correct today. It is load-bearing for the finding 3 guard, and it would
break silently if a load ever started reusing `GuiState`. A sentry would
be cheap.
