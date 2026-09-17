# Verification lane B — audit findings 2, 6 and 7

**Method.** Read-only. I read source at the working tree of branch
`machine-kinematics-confidence` on 2026-09-10. I ran no cargo command. Every
claim below carries a `path:line` at TODAY's tree. Where I could not settle a
claim by reading I say `UNVERIFIED` and give the reason.

`core/` means `crates/rs_cam_core/src/`. `viz/` means `crates/rs_cam_viz/src/`.

---

## LEAD: the STEP scale claim is TRUE, and it is a live wrong-size defect

The audit says: *"Interactive STEP loading applies the requested scale.
Project STEP loading does not apply its computed scale."*

**That is correct at today's tree, and the dropped scale is a real one.**

- The project door computes the scale and never uses it on the STEP arm.
  `core/session/project_file.rs:617` binds `let scale = model.units…`.
  The STL arm passes it (`:625`), the DXF arm passes it (`:642-643`), the SVG
  arm passes it (`:658`). The STEP arm (`:665-678`) calls
  `crate::step_input::load_step(&full_path, 0.1)` and returns
  `Ok(LoadedGeometry::Enriched(enriched))` at `:677`. The binding `scale` is
  never read on that arm.
- The interactive door applies it. `core/io.rs:31` binds
  `let scale = units.scale_factor();`, and the STEP arm at `:104-108` calls
  `enriched.apply_uniform_scale(scale)`.
- The dropped scale is not redundant. `truck-stepio` 0.3.0's READER performs
  no unit conversion. I read
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/truck-stepio-0.3.0/src/in/`
  and found no `LENGTH_UNIT`, `SI_UNIT` or `CONVERSION_BASED_UNIT` handling.
  The only occurrences of those tokens in the whole crate are in the WRITER,
  which emits a hardcoded `SI_UNIT(.MILLI.,.METRE.)` header
  (`src/out/topology.rs:376`). `core/step_input.rs` adds none of its own —
  `load_step` (`:44`) reads the file, calls `Table::from_step` and
  tessellates. So `ModelUnits` is the ONLY unit conversion a STEP file gets
  in this product, and the project door discards it.
- `EnrichedMesh::apply_uniform_scale` (`core/enriched_mesh.rs:174`) does
  scale the BREP data as well as the mesh — face-group bboxes (`:187`),
  boundary loops (`:199-210`) and dimensional surface params (`:211+`). So
  the interactive door is the CORRECT one and the project door is the
  defective one. This is not a double-scale.

### Reachable paths, counted

The GUI cannot reach it on its own. `import_step_path` hardcodes scale 1.0
(`viz/controller/io.rs:55` → `viz/io/import.rs:45-47`, `units_for_scale(1.0)`
= `Millimeters`), and `rescale_model` refuses STEP outright
(`viz/controller/io.rs:84-86`: `if model.kind == Some(ModelKind::Step) {
return Ok(None); }`). A GUI-authored project therefore always stores
`units = millimeters` for a STEP model, and scale 1.0 is a no-op.

Three doors DO reach it:

1. **`rs_cam_cli run --units inches part.step`.** `run.rs:72` parses the
   units (`parse_units` at `:242-249` accepts `inch|in|inches`), and `:82`
   calls `LoadedModel::from_file(0, …, Some(units), …)`, which routes to
   `project_file::load_model_geometry` (`core/session/mod.rs:261`). The scale
   is dropped. The program is cut on inch-numbered geometry: **25.4× too
   small**.
2. **`rs_cam_cli` job files.** `job.rs:524` builds
   `let units = op.scale.map(ModelUnits::Custom);` and `:525` calls the same
   door. A `scale` on a `.step` input is inert.
3. **Any project TOML that declares non-millimetre units on a STEP model.**
   `ProjectSession::load` is the primary GUI open path — I read
   `viz/controller/io.rs:315` (`match ProjectSession::load(path)`), and the MCP
   `load_project` tool routes to the same function
   (`viz/app/mcp.rs:3060` → `:3074`, `self.controller.open_job_from_path`).
   The viz
   loader `viz/io/project.rs::load_project`, which DOES route through the
   interactive door (`:1056`, `import::import_model`), is only the FALLBACK
   arm for a legacy file that the core loader refused
   (`viz/controller/io.rs:442-444`). So the GUI, the CLI and the MCP all
   open such a project with the scale dropped, and the error is stable
   across save/reload because the core saver writes the record's own units
   back unchanged (`core/session/save.rs:215`, `units: m.units`, and
   `project_file.rs:858` kept the section's value on the record).

### The `units: Some(ModelUnits::Millimeters)` stamp

`core/io.rs:120` stamps `Millimeters` on the STEP record whatever the caller
asked for, while the STL/SVG/DXF arms stamp `Some(units)` (`:51`, `:75`,
`:97`). It is deliberate and it is documented at
`core/session/mod.rs:323-329`. Its meaning:

- The stamp is **self-consistent for the interactive door**: the geometry
  really is millimetres after `apply_uniform_scale`, so declaring
  millimetres is honest about the record in memory.
- The stamp is **destructive on a round trip through the GUI's legacy
  fallback loader**. That loader imports through the interactive door and
  keeps what it returns, so a project declaring `units = "inches"` on a STEP
  model loads correctly, is then saved with `units = millimeters`
  (`viz/io/project.rs:574`, `units: model.units`), and the NEXT load applies
  1.0. This arm is legacy-only, so it is a lower-priority instance.
- **The one-line fix closes doors 1, 2 and 3 on its own.** I traced the
  round trip after it. CLI `--units inches` → the project door applies 25.4,
  the record keeps `Inches` (`project_file.rs:858`), the saver writes
  `Inches` (`save.rs:215`), the next load applies 25.4 again — stable. GUI
  import → 1.0, stamps millimetres, saves millimetres, reloads at 1.0 —
  stable. **The stamp is a contract question for the consolidation, not a
  blocker for the fix**: no reachable door creates an inch-declared STEP
  record today, so the stamp bites only on the legacy viz fallback arm, and
  only on a project some other tool authored.

### The existing sentry does not cover it

`crates/rs_cam_core/tests/model_units_survive_reload_g_unitsreload.rs`
pins door equivalence for STL, SVG, DXF polygons and DXF drill targets. I
grepped it case-insensitively for `step` and found **zero** hits. Its own
table (`:14-15`) has four columns and none of them is STEP. G-UNITSRELOAD
closed the third divergence in this loader pair and left the fourth in place.

**Recommendation on Phase 4A urgency: raise the FIX, not the consolidation.**
The consolidation is still the right long-term shape and its priority is
unchanged. The scale drop is a one-line change at
`core/session/project_file.rs:665-678` plus a STEP row in
`model_units_survive_reload_g_unitsreload.rs`, and it should ship on its own
schedule rather than wait for the loader merge.

---

## Summary table

| Finding | Audit verdict | My verdict at today's tree |
|---|---|---|
| 2 [High] — invalidation is a convention, not an enforced contract | STILL TRUE | **PARTLY CLOSED.** Half of the audit's first bullet is dead: `replace_toolpath_config` now calls `invalidate_result_chain` AND has zero callers. `apply_toolpath_param_snapshot` is still narrow, with 6 production call sites. Raw mutable accessors are unchanged. One narrow path the audit did not name. |
| 6 [Medium] — model import duplicated below the convergence point | STILL TRUE, and the STEP claim is a live defect | **STILL TRUE.** The two doors are not consolidated. The STEP-scale claim is CONFIRMED and reachable on three doors. Found a THIRD dispatch (the viz legacy loader). |
| 7 [High] — parameter support and validity have multiple authorities | STILL TRUE | **STILL TRUE, and wider than the audit states.** The `set_stepover` silent success reaches ELEVEN operations, not just RadialFinish. The six named arms bypass the DR-LIVE range gate entirely. Pencil carries an undocumented `stepover` alias. G-SCHEMAENUM closed one direction of the enum contract only. |

---

## Finding 2 — invalidation is a convention rather than an enforced mutation contract

### 1. Verdict

**PARTLY CLOSED.** The audit's headline — the same logical edit has different
dependency effects depending on its entry point — is STILL TRUE. But one of
its three cited bullets is now dead code, and I found one narrow path it did
not name. The recommendation stands with a corrected scope.

### 2. Evidence at today's tree

**The audit's line numbers have all moved.** `core/session/mutation.rs:211`
is now `:236` (`invalidate_result_chain`); `:1042` is now `:1234`
(`apply_toolpath_param_snapshot`); `core/session/mod.rs:1491` is now `:1578`
(the `── Mutable accessors ──` block header).

**Bullet 1 — `replace_toolpath_config` removes only that operation's result:
NO LONGER TRUE, and the method is dead.**

`core/session/mutation.rs:1205-1223`:

```
        // R0.1 §4.3: a wholesale config replacement is an input edit like
        // any other, so it invalidates the downstream stock chain too, not
        // only this toolpath's own slot.
        let enabled = self
            .toolpath_configs
            .get(index)
            .is_some_and(|tc| tc.enabled);
        self.invalidate_result_chain(index, enabled);
```

I then grepped the whole `crates/` tree for `replace_toolpath_config` outside
`session/mod.rs` and `session/mutation.rs`. **Zero hits.** The only caller is
its own unit test (`mutation.rs:2188`). A refactor scoped to "route
`replace_toolpath_config` through the transaction" would be routing a method
nobody calls.

**Bullet 2 — `apply_toolpath_param_snapshot` does the same: STILL TRUE.**

`core/session/mutation.rs:1234-1250`. The whole invalidation is:

```
        self.drop_result(index);
        self.simulation = None;
```

No `invalidate_result_chain`, so no same-setup downstream
`FromRemainingStock` invalidation and no `DerivedRestRegions` consumer
invalidation. Compare the ordinary door: `set_toolpath_param`
(`core/session/compute.rs:283`) ends at `:569` with
`self.invalidate_result_chain(index, enabled);`, and the GUI inspector's
write-back reaches the same kernel through
`invalidate_toolpath_inputs` (`mutation.rs:1385-1391`), which is a thin
wrapper on `invalidate_result_chain`. **The two entry points do different
things to the same project.**

The unit test pins the NARROW behaviour, not the wide one.
`mutation.rs:2220-2242` asserts `assert!(!s.results.contains_key(&0));` and
`assert!(s.simulation.is_none());` and nothing about a downstream op. A fix
must extend this test, not merely satisfy it.

**Bullet 3 — undo and optimizer application use the snapshot method: STILL
TRUE. Six production call sites, three test/instrument ones.**

Production:

| Site | What it is |
|---|---|
| `core/tool_load/optimize/candidate.rs:416` | Per-candidate apply, on the LIVE session at `ctx.toolpath_index` |
| `core/tool_load/optimize/context.rs:275` | `BaselineRestoreGuard::drop` — the baseline restore |
| `viz/controller/events/mod.rs:619` | Optimizer candidate apply (GUI) |
| `viz/controller/events/mod.rs:759` | Re-optimize: applies an optimizer suggestion for one feeds axis, then stamps `Optimizer` provenance (`:749-767`) |
| `viz/controller/events/mod.rs:1247` | A second candidate-apply arm |
| `viz/controller/events/undo.rs:145` | Undo / redo of a parameter edit |

Not production: `context.rs:415` and `:465` sit after `#[cfg(test)]`
(`context.rs:335`); `retarget_reconciliation_a8.rs:238` is an in-`src`
measurement fixture, present for the reason its own header gives (`:20-27`).

**One of those production sites DEPENDS on the narrowness.**
`candidate.rs:428-430`, immediately after the apply and the regenerate:

```
    // Sim — full project sim at the requested resolution. Other
    // toolpaths' cached results from baseline still apply because
    // generate_toolpath only touched index `toolpath_index`.
```

The optimizer varies one operation's parameters and re-simulates the whole
project, relying on every other cached result surviving. Widening
`apply_toolpath_param_snapshot` unconditionally would drop the downstream
chain on every candidate and change what the optimizer measures. **This is
the single most important constraint on the fix and it is not in the audit.**
Note also the flip side: today the optimizer's simulation scores a candidate
against downstream `FromRemainingStock` results generated from the BASELINE
stock. That is a correctness question of its own and it is not a lane-B
verdict.

**Bullet 4 — public mutable accessors require callers to remember: STILL
TRUE.** The `── Mutable accessors ──` block (`core/session/mod.rs:1578`)
still carries the convention in a comment: *"**Prefer named mutation
methods** in `mutation.rs` … After using `stock_mut()` or `machine_mut()`,
call `invalidate_stock()` / `invalidate_machine()`"*. Nothing enforces it.

### 3. What changed since the audit

From `planning/ui_fix_2026-09-09/STATUS.md` and the code:

- **G-FRESHSTATE** made a GUI inspector edit drop the core result via
  `invalidate_toolpath_inputs` (`viz/ui/properties/mod.rs:3698-3754`,
  `mutation.rs:1385`), and put `tool_id` / `model_id` into
  `generation_inputs_signature`. Stock, tool, model and setup edits now
  invalidate.
- **`replace_toolpath_config`** gained the chain call (R0.1 §4.3) and lost
  its last caller.
- **F2.5 (G-UNDOFRESH)** fixed the `invalidate_tool` bypass **in viz, not in
  core** — the report says *"No core file changed."* Today
  `viz/controller/events/undo.rs:105-124` (`apply_tool_snapshot`) writes the
  tool slot through `tools_mut()` and then calls
  `self.state.session.invalidate_tool(tool_id.0)` at `:115`, in both
  directions (undo `:21`, redo `:68` both dispatch to it). **That bullet of
  F2.5 is FIXED.**

- **F2.12 / F2.13** gave the holder-clearance verdict the same edit-counter
  stamp the other readiness rows carry, and made it declare how much of the
  job it examined. I read the `STATUS.md` rows and the sentry name
  (`viz/controller/holder_clearance_staleness_g_holderstale.rs`). Both act on
  a GUI verdict's freshness presentation. **Neither touches the core mutation
  contract**, so neither moves this finding.

**Is F2.5 the same defect?** It is the same CLASS and a different INSTANCE.
Same class: a mutation route that writes state without running the
invalidation the equivalent hand edit runs. Different instance: F2.5's was
the TOOL slot; the audit's is the TOOLPATH-PARAMETER slot. The
toolpath-parameter arm of undo **still carries the class defect** —
`undo.rs:145` calls the narrow snapshot method, so undoing a parameter edit
on an upstream rough leaves every downstream `FromRemainingStock` result in
place, while making the same edit by hand drops them. The fix F2.5 applied
(remember the extra call at this one site) is exactly the per-caller patch
the audit argues against.

### 4. Is the recommendation still the right shape?

**Yes, with two corrections.**

- Drop `replace_toolpath_config` from the scope, or delete the method. It
  already does the right thing and nothing calls it.
- The transaction cannot be "always invalidate the chain". `candidate.rs`
  documents a live dependence on the narrow effect. The boundary needs an
  explicit scope on the mutation — an operator edit invalidates the chain, a
  speculative optimizer probe does not — rather than one behaviour for the
  method. That distinction is a real design decision and it should be made
  before any call site moves.

The audit's closing sentence — *"This is more valuable than merely adding
another invalidation call to each current caller"* — is now supported by
evidence rather than argument: F2.5 added exactly one such call, at one site,
and the neighbouring site in the same file still has the defect.

### 5. Size estimate — counted, not guessed

Two very different numbers. Say both.

**The narrow-method fix: 1 function, 2 crates touched, 6 call sites to
review.** `apply_toolpath_param_snapshot` is one function in `rs_cam_core`.
Its 6 production call sites are 2 in `rs_cam_core` and 4 in `rs_cam_viz`
(table above). Two of the six must NOT get the wide behaviour
(`candidate.rs:416`, `context.rs:275`), so the change is a scope parameter
plus 6 decisions, plus extending `mutation.rs:2220`.

**The audit's full recommendation — restrict raw mutable access: 73 non-test
call sites across 2 crates.** Counted with `grep -rnE "\.<name>\(" crates/`,
excluding `session/mod.rs` itself and files matching `tests`:

| Accessor | non-test sites | total sites |
|---|---|---|
| `stock_mut` | 11 | 12 |
| `machine_mut` | 11 | 11 |
| `tools_mut` | 5 | 27 |
| `models_mut` | 11 | 19 |
| `post_mut` | 5 | 5 |
| `wizard_mut` | 12 | 34 |
| `toolpath_configs_mut` | 9 | 23 |
| `setups_mut` | 3 | 7 |
| `find_toolpath_config_by_id_mut` | 2 | 3 |
| `find_setup_by_id_mut` | 4 | 4 |
| **Total** | **73** | **145** |

---

## Finding 6 — model import is duplicated below the point where it should converge

### 1. Verdict

**STILL TRUE.** The pair the audit names is not consolidated, and its
sharpest claim is a live defect. See the LEAD section for the STEP-scale
answer in full; this section covers the rest of the finding.

### 2. Evidence at today's tree

The audit's line numbers moved. `core/io.rs:25` is now `:26`
(`pub fn load_model_file`). `core/session/project_file.rs:569` is now `:592`
(`pub(crate) fn load_model_geometry`).

**Format dispatch is duplicated.** Two independent `match kind` blocks with
four arms each: `core/io.rs:37-129` and
`core/session/project_file.rs:623-687`.

**Extension-to-format recognition is duplicated FOUR ways**, not twice:

| Site | Function |
|---|---|
| `core/io.rs:127` | `infer_kind_from_path` |
| `core/session/project_file.rs` | `infer_model_kind` (called at `:600`) |
| `viz/controller/io.rs:692` | `kind_from_extension` |
| `viz/app/mcp.rs:3010` | inline `match` on `"step" \| "stp"` |

`viz/controller/io.rs:679` says so in its own words: *"Mirrors the `match` in
`app::mcp::mcp_import_model`; core's …"*.

**Metadata construction differs.** The interactive door returns a full
`LoadedModel` including `winding_report` — computed on the STL arm only
(`core/io.rs:41`, `mesh.check_winding()`). The project door returns a
`LoadedGeometry` enum and `build_session_from_project` assembles the record
around it with `winding_report: None` on every arm
(`project_file.rs:878`, `:900`, `:923`, `:944`). Under the repo's own
"`None` means NOT MEASURED" rule that is honest, but it means winding
inspection **never runs on a project load** — the audit's "winding inspection
… follow different paths" is confirmed, and the difference is presence versus
absence.

**Drill-target and layer classification IS shared now.** Both doors call
`crate::svg_input::circle_like_drill_targets` and `circle_like_layers`
(`io.rs:63-64`, `project_file.rs:661-662`), with a comment on the project
side saying the two doors must agree. That part of the divergence is closed.

### 3. What changed since the audit

`STATUS.md` records **F4.4 (`bb07d724`)**, which unified `rescale_model`,
`reload_model` and `relink_model` into `LoadedModel::adopt_geometry`
(`core/session/mod.rs:319-368`) with an exhaustive destructure as the guard.
I confirmed all three GUI doors call it: `viz/controller/io.rs:105`, `:168`
and `:263`.

**That is a different pair, exactly as `STATUS.md` warns.** `adopt_geometry`
refreshes an EXISTING record in place; the audit's pair is the two doors that
CREATE one from a file. All three F4.4 doors call `import::import_model`,
which is the interactive door — so F4.4 consolidated three consumers of door
1 and did not touch door 2.

**F4.7 (G-RESCALESTALE)** added the invalidation sweep to `rescale_model`
(`viz/controller/io.rs:124-131`) — relevant to finding 2, not to this one.

### 4. Is the recommendation still the right shape?

**Yes.** "One loader returning a geometry bundle — mesh, topology, polygons,
drill targets, layers, and import diagnostics" is the right shape, and the
tree has already grown two thirds of that bundle: `LoadedGeometry` carries
mesh / polygons+targets+layers / enriched, and `LoadedModel` carries the
diagnostics slots (`winding_report`, `load_error`).

Two additions the audit does not make:

- **Fix the STEP scale first, separately.** It is one line and it is
  currently cutting undersized parts on the CLI door. Do not let it wait for
  a consolidation.
- **The `units` stamp is a contract question, not a code-duplication
  question.** Decide what `LoadedModel::units` means — "the operator's
  declared source units" or "the units the geometry in this record is in" —
  before merging the doors. Today the STL/SVG/DXF arms mean the first and the
  STEP arm means the second (`core/io.rs:51`/`:75`/`:97` versus `:120`), and
  `adopt_geometry`'s doc already documents the resulting special case
  (`core/session/mod.rs:322-329`). A merge that does not settle this will
  reproduce the divergence inside one function.

### 5. Size estimate — counted, not guessed

- **The STEP-scale fix alone**: 1 line in 1 crate
  (`core/session/project_file.rs:665-678`), plus a STEP row in
  `crates/rs_cam_core/tests/model_units_survive_reload_g_unitsreload.rs`, plus
  a decision on the `units` stamp at `core/io.rs:120`.
- **The full consolidation**: **2 crates**, **2 loader functions**
  (`core/io.rs:26`, `core/session/project_file.rs:592`), **4 extension
  dispatchers** to collapse to one (table above), **3 record-assembly sites**
  in `build_session_from_project` (`project_file.rs:860-950`), plus the
  legacy viz loader's 2 model functions (`viz/io/project.rs:968` and
  `:1008`) which are reachable only through the fallback arm at
  `viz/controller/io.rs:444`. Direct callers of the interactive door outside
  core: 5 in `viz/io/import.rs` (`:27`, `:33`, `:39`, `:45`, `:51`). Direct
  callers of the project door outside `project_file.rs`: 2
  (`core/session/mod.rs:261`, `project_file.rs:860`), which serve 3 CLI/GUI
  entry points (`cli/run.rs:82`, `cli/job.rs:525`,
  `viz/controller/io.rs:315`).

---

## Finding 7 — parameter support and parameter validity have multiple authorities

### 1. Verdict

**STILL TRUE, and wider than the audit's example suggests.**
G-SCHEMAENUM closed one direction of the enum contract. Everything else the
finding names stands, and the `set_stepover` silent success reaches eleven
operations rather than the one the audit cites.

### 2. Evidence at today's tree

The audit's line numbers moved. `core/compute/catalog.rs:590` is now `:650`
(the `OperationParams` trait). `core/session/compute.rs:278` is now `:283`
(`pub fn set_toolpath_param`).
`viz/ui/properties/operations/finishing.rs:127` is still `:127`
(`draw_radial_finish_params`).

**Claim A — `set_stepover` defaults to doing nothing: TRUE.**
`core/compute/catalog.rs:661`:

```
    fn set_stepover(&mut self, _value: f64) {}
```

**Claim B — the common setter accepts `"stepover"` without checking support:
TRUE.** `core/session/compute.rs:342-351`:

```
            "stepover" => {
                let v = as_number(&value).ok_or_else(|| {
                    SessionError::InvalidParam("stepover must be a number".to_owned())
                })?;
                tc.operation.set_stepover(v);
                tc.feeds_provenance.set(
                    crate::feeds::FeedsField::Stepover,
                    crate::feeds::ValueProvenance::manual(),
                );
            }
```

The call returns `Ok(())`, stamps the provenance `manual`, and then
invalidates the result at `:569`. So the operation regenerates with its OLD
stepover while every provenance surface reports that the operator set it.

**Claim C — RadialFinish inherits the no-op: TRUE.**
`core/compute/operation_configs.rs:2352-2374` (`impl OperationParams for
RadialFinishConfig`) implements `feed_rate`, `plunge_rate`,
`depth_semantics` and `spindle_rpm` only. `RadialFinishConfig` has no
`stepover` field, and `RADIAL_FINISH_PARAMS` (`catalog.rs:1933-1940`) does
not advertise one.

**Found while verifying — the silent no-op reaches ELEVEN operations, not
one.** `RadialFinish` is the audit's example, but the named `"stepover"` arm
returns `Ok(())` on every operation whose config does not implement the
setter. Eleven `impl OperationParams` blocks omit `set_stepover`, so eleven
operation types accept the write and discard it:

| Operation | `impl OperationParams` at |
|---|---|
| Profile | `operation_configs.rs:1750` |
| Trace | `:1948` |
| Drill | `:1978` |
| AlignmentPinDrill | `:2003` |
| Chamfer | `:2028` |
| Waterline | `:2124` |
| Scallop | `:2196` |
| UnifiedFinish | `:2226` |
| RampFinish | `:2286` |
| RadialFinish | `:2352` |
| ProjectCurve | `:2406` |

**Correction to an earlier reading of mine.** I first reported that
SteepShallow, SpiralFinish and HorizontalFinish had a `stepover` field with no
setter. **That is wrong** — a truncated grep hid three impls. They implement
it at `operation_configs.rs:2272`, `:2338` and `:2392`. I record the error
because the corrected picture is the useful one:

- **12 configs have a `pub stepover` field** — Face `:131`, Pocket `:316`,
  Adaptive `:384`, VCarve `:433`, Rest `:457`, Inlay `:488`, Zigzag `:516`,
  DropCutter `:542`, Adaptive3d `:600`, SteepShallow `:1428`, SpiralFinish
  `:1494`, HorizontalFinish `:1543`.
- **12 `_PARAMS` tables advertise `stepover`** — the same twelve.
- **All twelve implement the setter.** The registry, the struct and the setter
  AGREE on the twelve that support the dial.

So the divergence is not "an op that has the field cannot be set". It is the
one the audit names, and it is wider than its example: **the common setter
does not consult any of the three authorities before writing.** It writes,
returns success, stamps `FeedsProvenance::Stepover = manual`
(`core/session/compute.rs:347-350`) and invalidates the cached result
(`:569`) — for eleven operations on which nothing changed.

**A twelfth case, in the other direction: an undocumented alias.**
`PencilConfig` has NO `stepover` field and its registry table does not
advertise one (`PENCIL_PARAMS`, `catalog.rs:1748-1762`, which advertises
`offset_stepover` at `:1753`). Its impl maps the trait method onto a
differently-named field (`operation_configs.rs:2182-2184`):

```
    fn set_stepover(&mut self, value: f64) {
        self.offset_stepover = value;
    }
```

So `set_toolpath_param(idx, "stepover", v)` on a pencil operation succeeds
and writes a real dial that the published schema calls something else. An
agent reading the schema cannot discover this key, and an agent that guesses
it gets a working write with no record of which field moved.

**`depth_per_pass` has the same shape.** `catalog.rs:666` is
`fn set_depth_per_pass(&mut self, _value: f64) {}` and its named arm is
`core/session/compute.rs:352-361`. Ten of the 24 `impl OperationParams`
blocks implement `set_depth_per_pass`; eight `_PARAMS` tables advertise
`depth_per_pass`. So **14 operations accept the key and discard it.** The
6-arm structural fix covers this too — do not scope the work to `stepover`
alone.

**Claim D — radial GUI controls constrain what the registry does not: TRUE.**

| Authority | angular_step | point_spacing |
|---|---|---|
| Registry | `ParamDef::required("angular_step", "f64")` — `range: None` (`catalog.rs:1934`) | `ParamDef::required("point_spacing", "f64")` — `range: None` (`catalog.rs:1935`) |
| GUI | `1.0..=90.0` (`finishing.rs:137-145`) | `0.1..=5.0` (`finishing.rs:145-153`) |
| Generator | `let num_spokes = (360.0 / params.angular_step).ceil() as usize;` (`core/radial_finish.rs:96`) | `let num_points = (max_radius / params.point_spacing).ceil() as usize + 1;` (`core/radial_finish.rs:108`) |

`core/compute/execute.rs:3039` passes `angular_step: cfg.angular_step`
straight through with no guard. A value of `0.0` through
`set_toolpath_param` reaches `360.0 / 0.0` and a saturating float-to-integer
cast. A negative value gives a negative quotient, whose `as usize` cast
saturates to `0` and produces an empty toolpath with no error.

A fourth authority disagrees with all three: the GUI's radial DIAGRAM caps
its spoke count at 72 (`viz/ui/properties/operations/mod.rs:1026`,
`let num_spokes = num_spokes.min(72); // cap for very small angular_step`).
The picture is bounded; the machine motion is not.

**Claim E — the registry, common setters, GUI ranges and generator guards are
not one contract: TRUE, and there is a fifth authority the audit missed.**
A numeric range gate DOES exist, and it is unreachable from the named arms.
`core/session/compute.rs:413` binds `range_for_param` INSIDE the `_` arm, and
`:502-518` refuses an out-of-domain value (DR-LIVE, 2026-08-14 — this
predates the audit and the audit does not mention it):

```
                if let Some(range) = range_for_param
                    && let Some(n) = value.as_f64()
                    && !range.accepts(n)
```

The six named arms above it — `feed_rate` `:322`, `plunge_rate` `:332`,
`stepover` `:342`, `depth_per_pass` `:352`, `spindle_rpm` `:362`,
`debug_enabled` `:398` — never look the range up. So a param with a declared
`ParamRange` is enforced when it goes through the generic path and
unenforced when it goes through its own named path.

### 3. What changed since the audit

**G-SCHEMAENUM (`32a1fd46`, F1.16)** added
`crates/rs_cam_core/tests/schema_enum_values_g_schemaenum.rs`. It is generic
and it pins one direction: **every value an `enum:a|b|c` string advertises
must be a value serde will accept.** Its own header states the residue
plainly (`:11-16`): *"`ProjectSession::set_toolpath_param` never parses the
string. It merges the value into the config's own JSON and hands the result
to serde … So an advertised value that serde rejects is a pure untruth"*, and
at `:31-34`: *"Note what this does NOT check: that the generator DOES
something different for each advertised value."*

**Which half is closed:**

| Property | Pinned? |
|---|---|
| advertised ⊆ accepted-by-serde | **YES** — `schema_enum_values_g_schemaenum.rs` |
| accepted-by-serde ⊆ advertised | **NO** — nothing checks it |
| the schema string is consulted at validation time | **NO** — `set_toolpath_param` still merges JSON and lets serde decide |
| each advertised value changes generator behaviour | **NO** — stated as out of scope by the sentry itself |

I spot-checked the second row by hand on four enums and found no
under-advertisement today: `FaceDirection` (`core/face.rs:17-22`, two
variants) versus `"enum:one_way|zigzag"` (`catalog.rs:1544`);
`DrillCycleType` (`operation_configs.rs:57-62`, four variants) versus
`"enum:simple|dwell|peck|chip_break"` (`catalog.rs:1660`);
`ClearingStrategy` (`operation_configs.rs:40-53`, four variants) versus
`"enum:contour_parallel|adaptive|agent_search|contour_spiral"`
(`catalog.rs:1717`); `PocketPattern` (`operation_configs.rs:18-21`) versus
`"enum:contour|zigzag"` (`catalog.rs:1555`). Clean today, unpinned tomorrow.

There are **20** `enum:` strings in `catalog.rs`, and they use two casing
conventions with no stated rule — `enum:one_way|zigzag` beside
`enum:DiskArea|LeadingArc` (`:1596`) and
`enum:Legacy|ResidueMop|ContourParallelNarrow|ContourParallelHybrid`
(`:1592`). Both are correct against their own enums, which is the point: the
string is documentation, and its shape is whatever its author typed.

### 4. Is the recommendation still the right shape?

**Yes, all four bullets.** Two refinements.

- *"Unsupported setters return an error, not silent success."* The cheapest
  correct version is for the named arms to consult
  `tc.operation.param_names()` before dispatching, which already exists and
  is already used for the unknown-param error message
  (`core/session/compute.rs:517`, `:534-539`, `:551-556`). That closes all
  eleven no-op operations at once, because the registry and the struct
  already agree — `param_names()` is a sound authority for this question
  today. It does NOT cover the Pencil alias, which needs the registry to
  publish the alias or the impl to drop it.
- *"Share hard numeric validity rules across mutation and execution."* Note
  that the mutation side already has the mechanism (`ParamRange`, DR-LIVE)
  and the gap is that (a) the named arms skip it and (b) `ParamRange` is
  opt-in per param and the divisors on RadialFinish opted out. The work is
  populating and routing, not building.

### 5. Size estimate — counted, not guessed

**Crates: 2** (`rs_cam_core`, `rs_cam_viz`). The CLI only prints the schema
(`cli/run.rs:215-222`; the G-SCHEMAENUM header cites the same table) and needs no change.

Cheap tranche, all in `rs_cam_core`:

- **6 named arms** to gate on `param_names()`
  (`core/session/compute.rs:322`, `:332`, `:342`, `:352`, `:362`, `:398`).
- **11 operations** whose `set_stepover` is the trait no-op (table above) —
  they need the support check, not new impls; all twelve configs that HAVE
  the field already implement it.
- **2 `ParamDef`s** to convert from `required` to `required_ranged`
  (`catalog.rs:1934`, `:1935`); the constructor already exists at
  `catalog.rs:1344`.
- **1 sentry** for the third table row above (advertised-but-unsettable),
  which can be generic in the same style as G-SCHEMAENUM.

Full tranche adds:

- **20 `enum:` strings** in `catalog.rs` to move from a documentation string
  to a validated declaration.
- **24 `impl OperationParams` blocks** (`operation_configs.rs:1678`…`:2406`)
  are the population any "unsupported returns an error" rule must be checked
  against.
- **1 GUI file** to reconcile against the registry once ranges exist
  (`viz/ui/properties/operations/finishing.rs`), plus the diagram cap at
  `viz/ui/properties/operations/mod.rs:1026`.

---

## Found while verifying

Items outside the three findings, or inside them but unnamed by the audit.

1. **A second narrow mutation path in the same file the audit cites.**
   `ProjectSession::set_drill_selected_holes`
   (`core/session/mutation.rs:922-947`) ends with `self.drop_result(index);
   self.simulation = None;` at `:944-945` — narrow. Its immediate neighbour
   above, which sets the pin-drill dial, ends with
   `self.invalidate_result_chain(index, enabled);` at `:911` — wide. Two
   adjacent public setters on the same struct, two different contracts. A
   drill operation removes material, so a downstream `FromRemainingStock`
   operation is left holding a result generated against a different set of
   holes. This is the audit's thesis in one screen of code and the audit did
   not find it.

2. **One dead public mutation-surface method.**
   `ProjectSession::replace_toolpath_config` (`mutation.rs:1205`) has no
   caller anywhere in `crates/` outside its own unit test
   (`mutation.rs:2188`). It widens the API surface that finding 2 asks to
   restrict, and it is the method the audit's first bullet names.

3. **A third model-load dispatch.** The audit names two doors. There is a
   third: `viz/io/project.rs` has its own complete project loader with its own
   model functions (`load_legacy_model` `:968`, `load_model_section`
   `:1008`), and it routes to the INTERACTIVE door
   (`import::import_model`, `:994` and `:1056`). It is reachable only as the
   fallback when `ProjectSession::load` errors
   (`viz/controller/io.rs:442-444`), so it is a legacy-format path — but it
   means a legacy STEP project gets the scale applied while a current one
   does not, and it is a third place `default_units_for_kind`
   (`viz/io/project.rs:1341`) decides a unit policy.

4. **`winding_report` is never measured on a project load.** The interactive
   door computes it on the STL arm (`core/io.rs:41`). Every arm of
   `build_session_from_project` sets `winding_report: None`
   (`project_file.rs:878`, `:900`, `:923`, `:944`). Under the repo's own
   rule that `None` means NOT MEASURED this is honest, but a mesh with
   inconsistent winding is silently un-inspected on every project open,
   including the GUI's.

5. **The GUI cannot rescale a STEP model at all.**
   `viz/controller/io.rs:84-86` returns early for `ModelKind::Step`, before
   the import. There is no notification and no disabled control noted at that
   site — the operator picks a unit from the dropdown
   (`viz/ui/properties/mod.rs:967-1004`) and nothing happens. This is what
   makes the STEP-scale defect unreachable from the GUI alone, so it is
   load-bearing for the LEAD section's blast-radius claim; it is also, on its
   own, a silent no-op on an operator control.

6. **`apply_toolpath_param_snapshot`'s unit test pins the narrow behaviour.**
   `mutation.rs:2220-2242`. Any widening fix fails it, which is correct — but
   whoever schedules the work should know the sentry must be rewritten, not
   merely re-run.
