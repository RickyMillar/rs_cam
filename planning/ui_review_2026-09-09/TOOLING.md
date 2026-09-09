# Review tooling and preparation

## Observed in this planning session

- SocratiCode is indexed/green and available. Source search and dependency tools
  are suitable for supporting investigations.
- Pi's MCP gateway reports **rs-cam configured but disconnected**. We did not
  connect, launch, replace a project or run a simulation for this plan. A process
  check at reconnaissance time found no running `rs_cam_gui`; recheck later.
- File reads support PNGs. Shell/file tools are available.
- No general desktop pointer/keyboard-control tool is exposed in this session.
- The existing `scout`/`worker` profiles select Anthropic models; attempted scout
  delegation failed on missing Anthropic credentials. Ricky's preference is
  **Codex or DeepInfra for support, Astra for big tasks**. Do not solve this by
  asking for Anthropic credentials.
- A bounded attempt with the existing `glm-worker` also failed: its bare
  `zai-org/GLM-5.2` model selection resolved to **huggingface**, not the intended
  DeepInfra provider, and requested unavailable credentials. That does not prove
  the DeepInfra account is unavailable. Verify provider-qualified selection with
  a tiny read-only task before relying on those profiles. No agent/provider/auth
  configuration was changed during planning.

## Minimum useful setup — do this first

1. Ask Ricky to dedicate a GUI session to the review; preserve any current
   unsaved work. Confirm whether connecting Pi launches a GUI or attaches through
   a configured launcher. Do not assume the project `.mcp.json` is Pi's config.
2. Reconnect the already configured server with Pi's MCP gateway
   `mcp({ connect: "rs-cam" })`, or its MCP reconnect UI. Inspect the actual tool
   inventory after connection. Never edit MCP/auth configuration as a side effect.
3. Query `project_summary` and record its `build` block. In current source it
   embeds `rs_cam_mcp::server::build_info()` (`app/mcp.rs:934`); do not assume a
   standalone `build_info` tool exists. Match the running binary to the review
   commit and record dirty-source differences.
4. Use a small scratch fixture, capture the **full window** with `screenshot_gui`,
   and read the image. Confirm selection/workspace changes appear and that the
   captured window is the actual review instance.
5. Arrange a short human-driven session for menu discovery, typing, selection,
   drag/drop, native dialogs and undo. The useful minimum is MCP + a human, not a
   new automation framework.

If connection needs configuration or credential changes, stop and ask. The user's
`.mcp.json` was already modified when planning began; leave it alone.

### Build only if needed, on an agreed lane

The current viz manifest enables `mcp` by default. The existing prompt's build
command is `cargo build --release -p rs_cam_viz --bin rs_cam_gui`. Before any cargo
command, check `pgrep -af "carg[o]"` and `free -g`: no competing cargo process and
at least 10 GiB available under the review lane rule. Coordinate and obtain
approval for an expensive build; use the existing matching binary where possible.
No workspace-wide tests or heavy-tests during a visual review. Source was not
changed by this planning task, so no Rust build/test was necessary.

## What each tool can and cannot do

| Tool / infrastructure | Good use | Important limit |
|---|---|---|
| `project_summary`, inspect/list/get tools | Pin state, geometry, machine, current params and evidence | Does not establish which facts the GUI tells a person |
| `set_ui_view` + `screenshot_gui` | Reach supported workspace/selection/tab/modal states and capture the whole application | State navigation is not menu discovery or arbitrary UI input; resize persists |
| `screenshot_toolpath`, `screenshot_simulation`, `reach_map` | Isolate scene and geometric evidence | Offscreen scene is not the visible panel composition; an overlay percentage is not a usability result |
| `get_diagnostics`, `get_tool_load_report`, `narrate_toolpath` | Explain a visible discrepancy and record scope/provenance | Narration can read planned IR where another path reads emitted results; read current caveats |
| `generate_all`, `run_simulation`, generation status/cancel | Prepare controlled result states | Side effects and compute cost; automation can skip human prerequisites |
| SocratiCode + source/test reads | Trace suspicious controls and identify coverage | Tests may use scripted backends and bypass the GUI |
| `ui/automation.rs` | Existing seed for stable widget IDs and rectangles | Sparse opt-in recording, not a full accessibility tree or input driver |

Current `set_ui_view` modal choices are feeds, per-op optimize, export wizard,
tool library and none. Do not invent arguments for planner, project optimize,
machine library, menus, wizard steps or arbitrary fields. Discover the running
schema; use real UI input for unsupported navigation. The source registration
has no generic click/type/drag tool.

`wizard_e2e.rs` explicitly tests the backend save/emission path, not wizard UI.
`mcp_authoring_surface.rs` tests schema/descriptions. Controller workflow tests
use `ScriptedBackend`, without GPU/UI. These are valuable regression anchors,
not evidence that a human can complete the corresponding task.

## Optional improvement — only if repeated manual work justifies it

Add an **approved desktop-input route** for this egui/wgpu app. Requirements:
window-targeted pointer move/click/drag/scroll, keyboard/text, focus inspection,
full-screen capture including native file dialogs, and timestamped action logs.
Confirm Linux display backend and permissions before choosing a tool. Browser
DOM automation alone is not a driver for this native app. Avoid unrestricted
whole-desktop input when a window-scoped route is possible.

A later, bounded rs-cam automation extension could expose the existing widget
snapshot with more IDs and accept actual egui input events. Prefer the current
GUI/controller path; do not add a parallel state-mutation API and call it UI
coverage. This is a tooling proposal, **not part of this task**.

Parallel supporting agents should have explicit Codex/DeepInfra provider/model
selection. Verify authentication and image input capability separately; a
text-only worker can inspect code or a report but must not claim to see a PNG.
Astra owns high-level reasoning, safety interpretation and synthesis. Do not
create or modify global agent definitions without agreement.

## Shared-state and compute discipline

- One GUI driver at a time. A source-research agent may run alongside it, but
  must not change selection, load projects, generate or resize that instance.
- Pin a fixture hash, build, machine/material, enabled operations, sim cell and
  capture settings. Preserve a record of initial window size and restore it.
- Copy project seeds and preserve referenced assets. Changing a library may
  write per-user catalog files even when the project is disposable: use an
  isolated library location or obtain permission before exercising those writes.
- `load_project` may auto-generate; do not treat load as side-effect-free. The
  current ledger includes disabled-op generation behaviour. Inspect the project
  before loading; a “one enabled op” file can contain many disabled operations.
- Generation calls support a timeout that returns “running” without cancelling.
  Poll `generation_status`; use `cancel_generation` only on the review-owned job.
- Remaining-stock Generate All needs an explicit simulation resolution. Pin it
  per scenario and record intermediate simulations. Do not guess it for a large
  job or claim coarse visual evidence clears fine-scale safety/finish questions.
- Start with small fixtures. Approve a bounded resource budget before mature
  terrain generation, optimizer searches or fine simulation. No parameter sweep
  is needed just to assess the UX of the comparison controls.
- Scratch exports must be clearly labelled **REVIEW ONLY — NOT FOR MACHINING**.
  Do not connect to a sender, replace production NC files or clear user libraries.
