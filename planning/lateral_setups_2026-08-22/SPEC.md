# Side-face setups — what is actually broken, and the one rule that fixes it

**Status**: spec for review. Two defects already fixed in the course of writing
it (G-LATERALSIGN, G-UNITSRELOAD); the remaining work needs one product
decision, marked **DECISION** below.

**Date**: 2026-08-22. Researched by four agents plus direct verification of
every load-bearing claim.

---

## 0. The headline, and a retraction

The RUN_LOG entry that started this said lateral support was "partial across
the board — milling works (the X/Y dexel grids are allocated lazily), 2D
polygon ops collapse to a degenerate line, drilling abstains."

**The parenthesis is wrong and is retracted.** Lateral *milling* does work, but
not by that mechanism. It works because a lateral setup is simulated **entirely
in its own setup-local frame**, where Z is always the tool axis, with
`direction` hardcoded to `FromTop` at `compute/simulate.rs:893`. Every metric,
gate, collision check, checkpoint mesh and `prior_stocks` snapshot comes from
that local stock. The tri-dexel X/Y grid machinery is reached by exactly one
object — the **global playback stock** — and there it was sign-inverted on all
four lateral faces.

That correction matters because it changes what needs fixing. The honest
picture:

| layer | lateral status | why |
|---|---|---|
| G-code emission | **correct** | emitted in setup-local; local Z *is* machine Z |
| metrics / gates / engagement | **correct** | computed on `group_stock`, `FromTop` |
| rapid-collision check | **correct** | reads `group_stock.z_grid` |
| checkpoint / composite mesh | **correct** | built from local mesh, then frame-mapped |
| rest machining (`FromRemainingStock`) | **correct** | reads `prior_stocks` = local stock clones |
| global playback stock, cut sign | **was inverted** | **fixed — G-LATERALSIGN** |
| live-scrub viewport mesh | **broken** | side grids append open surfaces to a solid, no boolean |
| 2D polygon ops (pocket, profile, v-carve, trace, …) | **broken** | collapse to a degenerate line |
| analytic drill removal on the global stock | **abstains** | `DrillHole` carries no axis |

So this is **not** "three silent behaviours behind one dial". It is: lateral
setups largely work, and what is broken is **the display and the 2D ops**.

---

## 1. What is already fixed

### G-LATERALSIGN — `cut_direction()` was inverted on all four lateral faces

`SetupTransformInfo::cut_direction()` mapped `FaceUp::Front → FromFront`,
`Back → FromBack`, `Left → FromLeft`, `Right → FromRight`. Every one is the
wrong sign, because **the two names describe opposite ends of the same setup**:

* `FaceUp::Front` = *the front face is up*, pointing at the spindle.
* `StockCutDirection::FromFront` = *the tool arrives from the front side*, and
  its own doc pins that as −Y.

If the front face is up, the tool arrives from where that face now points.
`inverse_transform_point` sends local `+Z` to global `+Y` for `FaceUp::Front`,
so the tool arrives from **+Y** — which is `FromBack`. Both lateral axes negate.

The Z faces *are* an identity (`Top → FromTop`, `Bottom → FromBottom`), which
is exactly why this survived: the mapping reads as an obvious identity and the
only two cases anyone had a fixture for both pass.

**Effect**: on the global stock the kernel called `subtract_below` where it
should call `subtract_above` — deleting everything from the far face up to the
cut plane instead of the shallow layer at the near face. Confined to the
live-scrub viewport; no number an operator reads moved.

**Sentry**: `tests/cut_direction_matches_transform_g_lateralsign.rs`. It does
**not** transcribe a table of six expected answers — a hand-written table is
the same kind of artefact as the mapping it checks and would have been written
wrong by the same reasoning. It pushes points through
`inverse_transform_point`, measures which global axis local `+Z` lands on and
with what sign, and requires `cut_direction()` to agree. The two Z faces are
the control on the derivation itself.

### G-UNITSRELOAD — a 2D model's unit scale was dropped on reload

Found while tracing how 2D geometry enters the pipeline; **not lateral-specific
and more urgent than anything else here**, because it hits the ordinary Top
workflow.

A project file stores a model's *path* and its declared `ModelUnits`, not its
geometry — both load doors re-import the same file. They did not agree:

| door | STL | SVG | DXF polygons | DXF drill targets |
|---|---|---|---|---|
| `io::load_model_file` (interactive import) | scaled | scaled | scaled | scaled |
| `project_file::load_model_geometry` (project load) | scaled | **dropped** | **dropped** | **dropped** |

`load_model_geometry` computed `let scale = model.units…scale_factor()` and
passed it only to `TriangleMesh::from_stl_scaled`. `ModelUnits`' own doc still
reads *"Assumed units of the imported **STL**"* — it predates 2D import, and
when the SVG/DXF arms were added they never consumed it.

**Effect**: an inch-authored DXF or SVG reloads **25.4× smaller**, silently,
with the stock still at its saved size (`update_from_bbox` runs on import, not
on load, so nothing re-fits and nothing complains). Measured on the repo's own
fixture: import door 2538.02 mm, project door 99.92 mm, ratio exactly 25.4.
Toolpaths then regenerate cleanly around a part a fortieth of its intended size.

This is the **third** divergence found in this loader pair; the previous two
were closed 2026-06-08 with "nothing left to consolidate", which was true of
the divergences then known and is the reason the claim now has a test.

**Sentry**: `tests/model_units_survive_reload_g_unitsreload.rs`, which asserts
the two doors **agree** rather than asserting a magic size — a test pinning
"254 mm wide" would pass just as well if both doors were wrong together.

---

## 2. The remaining work

### 2a. **DECIDED 2026-08-22** — what does a 2D drawing mean on a side face?

> **Operator ruling (2026-08-22): Reading B + the no-mesh precondition, as
> recommended below.** Asked directly whether edge-authored artwork on a
> mesh-less project is a real workflow; answer: no — that case is done as a
> Top setup with the edge as the stock face. The refusal's error message
> should name that workaround.

This is the whole of G-SIDEFACE-POLYCOLLAPSE, and it is a product question, not
a code question.

**The arithmetic is not buggy.** `apply_to_polygons` projects each point
through `world_to_local(P3::new(p.x, p.y, 0.0))`. For a 2D model,
`StockConfig::update_from_bbox` places the stock so its **top face is the
drawing plane** — so `z = 0` is the drawing's genuine world Z, not a
placeholder. The transform then computes a correct orthographic projection of a
horizontal drawing onto a vertical face, and that projection **is** a line. The
code is right; the request is meaningless.

Two readings, and they lead to different products:

**Reading A — the drawing is authored in world XY.** Then a side face genuinely
has no extent for it, and the correct behaviour is to **refuse** with a typed
error, not to emit a degenerate ring. Cheap, honest, and lateral 2D never works.

**Reading B — the drawing is authored for the face it will be cut into.** Then
its coordinates *are* the setup's work-plane coordinates and the transform
should be **skipped**, exactly as it already is for a Top setup — where
`apply_to_polygons` is never called at all (`needs_transform()` is false) and
the drawing's coordinates are used verbatim as machining coordinates.

**Recommendation, after working through the use cases (2026-08-22).** Reading
B, with one precondition. The reasoning is worth writing down because it turns
on a structural fact rather than a preference.

#### A 2D drawing already has two roles in this app

1. **The drawing is the part** — SVG/DXF into pocket / profile / trace /
   v-carve. `update_from_bbox` places the stock so its *top* is the drawing
   plane; the stock is fitted around the drawing.
2. **The drawing is a curve draped onto a 3D surface** — `project_curve`, the
   wanaka rivers. `generate_project_curve` requires **both** `ctx.polygons` and
   `ctx.mesh`: the mesh supplies the Z the drawing does not have.

#### The fact that settles it

**`face_up` only means something when there is a 3D model to register
against.** On a 2D-only project the drawing *is* the design — there is no
second face to turn to, because the part is whatever the drawing cuts from a
block. `FaceUp::Front` on such a project is a category error, not an
unimplemented feature. On a project carrying a mesh, setups are real: the mesh
registers the physical part, so "now machine its side" is a genuine second
operation with a defined meaning.

#### The rule, which introduces no new concept

> A 2D drawing is consumed in the **work plane of the setup that uses it**.

This is already what ships, stopped short of the lateral cases:

| setup | work plane vs drawing plane | today |
|---|---|---|
| Top / Deg0 | same | transform **skipped entirely** — drawing coords *are* machining coords |
| Top / Deg90 | same, rotated | rotated in-plane |
| Bottom | same plane, other side | mirrored — how a feature registers to the same physical place |
| Front/Back/Left/Right | **perpendicular** | collapses to a line |

For the parallel cases a mapping exists and is applied. For the perpendicular
ones no mapping exists, so the drawing is consumed **verbatim in the new work
plane** — the same rule, carried to the cases nobody had a fixture for.

Plus one precondition: **refuse a lateral setup that has no mesh**, because
`face_up` then had nothing to register against.

#### What this serves, and what it deliberately does not

Served:
* curves draped onto the **side** of a 3D part — the rivers workflow, one face
  over, well defined because the mesh supplies Z;
* a mortise, slot or edge pocket on a 3D-modelled part, bounded by a DXF.

Not served, on purpose: artwork on the edge of a **flat sheet**. The honest
answer there is "make that a Top setup with the edge as the stock face", which
is what you would physically do at the machine.

#### Why not "the drawing is an object with its own transform"

That is the right general model and it is what CAD does. The argument against
building it now is that **the setup already is that transform** — a drawing
consumed by a setup inherits it for free. Giving the drawing an independent
placement creates a *second* placement concept, and two placements force you to
specify how they compose. "Constrain to the part, or to coordinate space?" is
that composition question. The way not to answer it is to keep one transform.

If per-drawing placement is wanted later, tagging the model with its authored
face is the smallest step and layers on top of this rule without redoing it.

#### What would change this recommendation

Importing artwork authored for an **edge** on a project with **no 3D model**.
The precondition refuses that case, and it would need the tagging option up
front instead. Operator to confirm whether that is a real workflow.

#### Effort

One branch in `apply_to_polygons`, one precondition, after 2b removes the
duplicate. Both refusal mechanisms already exist
(`OperationError::MissingGeometry`; the `precondition.*` diagnostic family in
`diagnostics/ids.rs`). No new config field, no UI control, no migration.

### 2b. Two copies of the collapsing code, with a pre-existing divergence

`apply_to_polygons` (`compute/transform.rs`) and `transform_polygons`
(`rs_cam_viz/src/state/job.rs:519-557`) are the same algorithm written twice,
both carrying the `Z=0` hardcode. **They already disagree on something else**:
core calls `ensure_winding()` only when `result.closed`; the viz copy calls it
unconditionally — so open paths (rivers, traces) can be reversed by the GUI
path and not by the core path.

That is a live defect independent of lateral setups and should be closed by
**deleting the viz copy** and routing it through core, not by patching both.
Filed as **G-POLYTRANSFORM-DUP**.

### 2c. Drilling — **CLOSED-UNREACHABLE 2026-08-22**

> **The stamping fallback below is moot and was not built.** G-LATERALSCRUB
> (`3e951540`) removed the only shipped caller that ever handed
> `TriDexelStock::apply_drill_op` a lateral direction: the global playback
> stock's drill stamp now sits inside `if !lateral_playback`, and the viz
> worker's `build_playback_data` gives a lateral group `FromTop` plus its own
> setup-local frame. Every other call site passes `FromTop` literally or is
> `cfg(test)`. So there is no live failure left to fall back *from* — a
> fallback would be new machinery guarding a path nothing reaches, and its
> only exercise would be its own test.
>
> What was done instead is a doc-pin, not a deletion: `apply_drill_op`'s
> abstention arm and `DrillRemovalReport::unrepresentable_axis` **stay** (they
> are the kernel's honesty contract — a caller handed an axis the kernel
> cannot represent must be told, not shown a fabricated Z-axis hole), with the
> reachability fact recorded at the code. The side-grid kernel stays too, for
> the same reason: it is 3+2 / 5-axis capability, and the day something stamps
> it for real the contract becomes load-bearing again. The `DrillHole`
> two-3-D-endpoint redesign remains the better end state and remains off the
> critical path.

The original analysis, kept for the blast-radius numbers:

The earlier plan was "give `DrillHole` two 3-D endpoints". Research says that is
the wrong first move, for two reasons:

1. **The emitter does not use `DrillHole` at all.** `drill::drill_toolpath`
   takes `&[[f64;2]]` plus a single shared `DrillParams` and emits motion in
   setup-local coordinates, where the axis is always Z. It is already correct
   for lateral setups, and it round-trips to the global frame correctly because
   `group_toolpath_to_global` maps every point. **`DrillHole` is only the
   simulation's analytic shortcut.**
2. So the minimum honest fix is not to restructure `DrillHole` but to **fall
   back to stamping the linearised toolpath** on the global stock when the
   analytic kernel cannot represent the axis — the same path that already
   carries lateral milling. `apply_drill_op` already reports
   `unrepresentable_axis`; the caller can act on it instead of dropping it.

`DrillHole`'s two-3-D-endpoint redesign remains the *better* end state (it
would restore analytic fidelity), but it is no longer on the critical path.
Blast radius if it is ever done: 2 production construction sites, 11 test
sites, 6 consumer modules, **no serialisation migration** — `DrillHole` carries
no serde and is rebuilt on every regenerate. What *is* persisted is XY-only
(`DrillConfig.selected_holes`, `AlignmentPinDrillConfig.holes`,
`StockConfig::alignment_pins`).

### 2d. The live-scrub viewport mesh — **DONE 2026-08-22**

> Fixed in `3e951540` by the local-stock-then-map route the checkpoint meshes
> already used. Sentried by `tests/lateral_scrub_playback_stock_g_lateralscrub.rs`,
> which asserts **Z-grid solid volume** rather than a vertex probe — the
> obvious probe passes against the broken code, because the buried side
> surface really does contain the cut.

`dexel_stock_to_mesh` builds a closed marching-cubes solid from the Z grid and
then **appends** the X/Y grids as open per-segment heightmap surfaces
(`side_grid_to_mesh`). There is no boolean, and the three grids are never
reconciled — `ensure_grid` allocates a pristine full-material X/Y grid at first
lateral stamp, knowing nothing about what the Z grid already lost (asserted as
intended by `multi_grid_simulation_preserves_z_grid`).

Consequence: **a lateral cut can never remove material from the rendered solid
on any surface fed by the global stock.** Checkpoint meshes are fine (they come
from the local stock), so the checkpoint mesh and the live-scrub mesh
*disagree* for a lateral setup.

Cheapest honest fix: make the live-scrub path use the same local-stock-then-map
route the checkpoints use, rather than teaching the side grids to boolean.
Filed as **G-LATERALSCRUB**.

### 2e. `SimGroupEntry.direction` — **NOT DEAD. Deletion blocked, 2026-08-22**

Written by three producers (`session/compute.rs:955`, `:2170`,
`viz/.../execute/mod.rs:329`) and — the claim below said — read by nobody.
**That is wrong, and the deletion was stopped on it.** There is one reader:

```
crates/rs_cam_core/src/compute/sim_prefix.rs:425
    format!("{:?}", group.direction).hash(&mut hasher);
```

inside `hash_group_scalar(group: &SimGroupEntry)`, and the S5 memo's own
soundness table names it (`sim_prefix.rs:51`, *"per-group `local_stock_bbox`,
`local_to_global`, `direction` | group scalar"*). No *simulator* reads it —
`compute/simulate.rs` still derives its own `FromTop` — so the original
observation about the simulator holds. But the field is a **prefix-memo cache
key input**, so deleting it is a cache-correctness change, not a dead-code
sweep, and it needs the S5 owner's call plus a look at
`tests/sim_prefix_memo_s5.rs`.

Two further facts the earlier note did not have:

* A **fourth** producer exists, in `crates/rs_cam_core/benches/hot_paths.rs:1203`
  (`direction: StockCutDirection::FromTop`). Removing the field breaks that
  bench's compile, and `clippy --workspace --all-targets` builds it — so the
  deletion cannot be scoped to `src/` alone, and that file is the concurrent
  perf programme's.
* The producers *do* still collapse all four lateral faces to `FromTop`, so
  the value is a constant in practice and the hash contribution is constant
  with it. That is why nothing has ever gone wrong; it is also why the field
  is worth removing eventually.

Verdict: leave it, with this note, rather than delete it half-informed. The
honest one-line alternative — read it where the simulator already derives the
same answer — is worse, because the simulator's `FromTop` is *correct* and the
field's value is not.

---

## 3. Suggested order

1. ~~G-UNITSRELOAD~~ — **done.** Hits the ordinary workflow; not optional.
2. ~~G-LATERALSIGN~~ — **done.** Small, provable, unblocks everything below.
3. **G-POLYTRANSFORM-DUP** — delete the viz copy. Independent of the decision,
   and doing it first means 2a is a one-place change instead of two.
4. **2a** — decided; implement the work-plane rule + no-mesh refusal.
5. ~~**G-LATERALSCRUB**~~ — **done** 2026-08-22 (`3e951540`); see 2d.
6. ~~**G-DRILLLATERAL**~~ — **closed unreachable** 2026-08-22, doc-pinned, no
   stamping fallback built; see 2c.
7. `SimGroupEntry.direction` — **blocked, not deleted.** It has a reader
   (the S5 prefix memo's cache key) and a fourth producer in the perf
   programme's bench; see 2e.

## 4. What must exist before any of 3–7 is called done

**There is no lateral fixture anywhere in the repo.** `FaceUp::Front/Back/
Left/Right` appear only in the transform's own unit tests, `stock_config`, and
the pin-keying test — never in a machining fixture, and no project file in the
repo has ever used one. Every claim above about lateral behaviour is derived
from reading, not from running.

So the first deliverable of the implementation is **one end-to-end lateral
fixture** — a project with a Front setup, one 2D op and one drill — asserted on
emitted motion. Without it, "make lateral setups work" would ship the fourth
silent behaviour rather than removing the first three.

## 5. Guardrail in the meantime

The GUI face combo offers all six variants with no warning, MCP `set_setup_face`
accepts all six with no advisory, and a project TOML loads a lateral face
silently. Until 2a lands, a lateral setup with a 2D op produces a degenerate
ring that reaches the post-processor unchecked.

`FEATURE_CATALOG.md:111` says *"tri-dexel stock simulation (Z/X/Y grids, all 6
cardinal face orientations)"*. That is a true claim about the dexel engine's
grid capability and reads as a claim about the workflow. Worth rewording
whichever way the decision goes.
