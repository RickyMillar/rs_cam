# Brief: 3D viewport and selection parity research (2026-10-28)

You are a research agent for rs_cam, a Rust CAM workspace for 3-axis wood
routers. The operator wants the 3D view and its selection to feel as rich as
the web CAD tools they use. Your task is research only. Write no product
code. Commit nothing.

## The question

What does rs_cam need for parity with the web tools in 3D interaction, and
what is the best way to get it?

1. List the requirements for parity, with selection first. The operator
   gives selection the highest value.
2. Compare our implementation with each requirement.
3. Find frameworks, crates and example code that we can use or learn from.
   Include an answer on Bevy and the other Rust engines: a good fit, or too
   much for our need?
4. Recommend a path, with a cost and a risk for each step.

## What the lead found (a starting point; check it, do not trust it)

The lead read the viewport code for 10 minutes. Treat each item as a claim
to verify:

| Web tools | rs_cam now | Where |
|---|---|---|
| Orbit around the point under the cursor | Orbit around a fixed target | `crates/rs_cam_viz/src/render/camera.rs:103` |
| Zoom toward the cursor | Zoom toward the screen centre | `camera.rs:134` |
| Smooth camera moves for presets and Fit | The camera jumps | `camera.rs:140`, `:217` |
| Instant hover highlight | The hover colour is baked into the vertex colours, so a hover change rebuilds the mesh upload | `render/mesh_render.rs:268` |
| Fast picking (GPU ID buffer or a BVH) | One CPU ray tests every triangle | `crates/rs_cam_core/src/mesh.rs:373` |
| Face edges and silhouettes | Flat face colours only | no edge pass |
| Smooth edges | No MSAA on the offscreen target | `render/mod.rs:811` |
| Box select, chain/tangent select, selection filters | One click per face, plus pick modes | `crates/rs_cam_viz/src/interaction/picking.rs` |

The lead's opinion: wgpu is not the limit, because it has the same GPU
power as WebGL/WebGPU. The gap is interaction features that we did not build.
egui is immediate-mode, so polished overlays take more work, but the 3D
view is our own wgpu renderer. Test this opinion. Do not repeat it.

## Read first

1. `CLAUDE.md`, then `crates/rs_cam_viz/CLAUDE.md`, `crates/rs_cam_viz/src/render/CLAUDE.md`
   and `crates/rs_cam_viz/src/state/CLAUDE.md`.
2. The earlier selection programme: `planning/geometry_selection_2026-10-16/`
   (`REQUIREMENTS.md`, `GAP_ANALYSIS.md`, `INDUSTRY_DEFAULTS.md`,
   `DESIGN.md`, `UX_REVIEW.md`). It covers WHAT a user selects (items, loops,
   faces, pick modes). Your scope is HOW the selection feels in the 3D
   view. Do not repeat that work. Cite it and build on it.
3. The viewport code: `render/` (camera, mesh, toolpath, sim, stock),
   `interaction/picking.rs`, `state/pick.rs`, `state/selection.rs`,
   `state/selection_highlight.rs`, `app/viewport.rs`, `app/gpu_upload.rs`.
4. The import side: how STEP faces and edges reach the viz (`EnrichedMesh`,
   `FaceGroupId`, the truck BREP import). Find out if the BREP edges are
   available for an edge pass.

Use SocratiCode `codebase_search` first. Measure callers with `rg`.

## Scope of the research

### A. Requirements (selection first)

Write a requirement list with an ID per item, in these groups:

- **Selection:** hover pre-highlight, click, add/remove (Shift/Ctrl),
  box/lasso select, select-through vs visible-only, chain/tangent/loop
  select, "select similar" (same radius, same plane, same depth), selection
  filters (face/edge/vertex/loop), a pick list for stacked candidates,
  edge and vertex picks, selection in the model tree synced with the view,
  undo of a selection, keyboard use.
- **Camera:** orbit on cursor pivot, zoom to cursor, pan, eased presets and
  Fit, a view cube, fit-to-selection, perspective/ortho, touchpad and
  3-button mouse schemes (match a known CAD scheme, and say which).
- **Rendering for readability:** face edges, silhouettes, smooth shading
  with sharp feature edges, MSAA or FXAA, transparency/x-ray, section
  planes, selection outline.
- **Feedback:** cursor changes, tooltips on hover (face type, size), and the
  frame rate on large STEP parts and terrains (state the target numbers and
  where they come from).

For each requirement, give what one or more named reference tools do.
Observe the reference tools from public documentation and videos. Copy no
code from a closed product.

### B. Gap matrix

For each requirement, give: our state (with file:line), the gap, and a cost
(S/M/L with a reason). Mark what the operator gets the most from per unit
of cost.

### C. Frameworks and example code

Evaluate each one on these points: licence, maintenance (last release, open
issues), fit with egui 0.36 + egui-wgpu 0.36 + our own wgpu renderer, what
we would replace, and the migration cost.

- Engines: Bevy (with bevy_egui and bevy_picking), Fyrox, three-d, rend3,
  kiss3d. Answer: is a full engine a good fit for a CAD/CAM viewport inside
  egui, or too much? Think about the ECS, the build time, the binary size,
  the control of the frame loop, and how our MCP screenshot tools capture
  frames.
- Building blocks that keep our renderer: BVH/ray crates (parry3d, bvh,
  obvhs), GPU ID-buffer picking examples for wgpu, edge and silhouette
  rendering examples, an egui-wgpu paint callback example with picking,
  camera controllers (for example the orbit-on-cursor code in open
  viewers).
- Open-source CAD viewers to learn from: Online3DViewer, three.js examples
  (OrbitControls, outline pass, GPU picking), Chili3D, CQ-editor,
  truck's own viewer, Fornjot. Check each licence first.

Licence rule: we copy no GPL, LGPL or AGPL code. We can read such code for
ideas only, and the report says so where it does. MIT, Apache-2.0, BSD and
zlib code can be a source, with a CREDITS.md entry.

### D. Recommendation

Give one recommended path, not a survey. Order the steps by value per cost.
For each step: what it changes, the files, the cost, the risk, and how a
test or the MCP screenshot tools prove it. Say what we must not do (for
example, a rewrite that stops the GUI for weeks).

## rs_cam rules that apply

- Every product change goes through `ProjectSession::apply(Command)`. A
  selection that changes an operation must keep that path. A hover or a
  camera move is UI state, not a command.
- MCP parity: each selection action must be possible through MCP, so agents
  can drive it. Say how each recommendation keeps that.
- No text clutter in the UI: one line per message, the detail on hover.
  Prefer colour, outline and cursor state to text.
- No numbers from memory. A target (frame rate, pick latency) comes from a
  source that you cite, or from a measurement that you describe.
- Do not commit. Do not edit product code. A short probe (for example, a
  timing of `ray_pick_triangle` on a large STEP part) is allowed in a
  scratch directory, if you say what you measured and how.
- Cargo: use `scripts/cargo_lane.sh` for every cargo command. Never run a
  workspace-wide `cargo test`. Ask the operator before any run longer than
  about 3 minutes. Watch the disk: stop builds when less than 20 GB is free.

## Deliverables

Write them in `planning/viewport_parity_2026-10-28/`:

1. `REPORT.md`: the requirements (A), the gap matrix (B), the framework
   evaluation (C) and the recommendation (D). Put a one-screen summary at
   the top.
2. `SOURCES.md`: each source with its URL, licence and date read.
3. Optional: an interactive HTML page that shows the camera and selection
   behaviours side by side (the operator likes models explained as HTML),
   with a script that checks each claim that the page makes.

Write all prose in ASD-STE100 Simplified Technical English: short
sentences, the active voice, one word for one meaning.

When you finish, report to the operator in at most 15 lines: the top 5
gaps, the framework verdict, and the first 3 steps.
