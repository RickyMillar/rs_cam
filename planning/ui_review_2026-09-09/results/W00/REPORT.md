# Wave 0 — execution conditions

Date: 2026-09-09. Owner: Astra. Status: preparation complete; human interaction
coverage remains unavailable. This is an expert MCP-assisted review, NOT a novice
usability test. No application/configuration/library changes, builds, tests,
commits or process cancellation were performed by this review.

## Live instance and evidence boundary

- Initial connection: healthy frame loop, idle generation lane, no loaded project.
  `evidence/01_initial.png` is the actual 1400×900 capture, read by the reviewer.
- Initial workspace: Toolpaths. Right inspector gives five Getting started steps,
  ending **“Generate and export G-code”**. Simulation is absent from that list.
- Raw `fixtures/demo_pocket.svg` imported through MCP, assigning model ID 0.
  Bbox (5,5,0)..(75,55,0), dimensions 70×50×0 mm. Auto stock became 80×60×25,
  origin (0,0,-25), Generic Softwood, medium workholding, one Top setup.
  `02_svg_import.png` and `03_stock.png` capture these visible states.
- `project_summary` cannot return build metadata while no project is loaded.
  After import it reports **8a4df241-dirty**, timestamp
  **2026-09-09T08:55:51+12:00**, versions 0.1.0, git_sha null.
- Checkout HEAD was **ef91cb03**, not the embedded build description. Initial
  commit delta from 8a4df241 was render visibility/test/docs changes. Later the
  working tree acquired other teams' engine/planner/MCP changes. None were edited.
- The GUI restarted during the interruption between the 09:39 and 13:51 captures.
  The connected instance was blank again. No conclusion is drawn about why it
  restarted. Earlier saved scratch job was restored; build metadata remained the
  same. W01b isolates subsequent evidence and re-runs the baseline before A/B.
- Binary file SHA-256 at 13:52 and 13:53:
  `9f1cb52277bc79087142d19e14e5bc885ea3663c08d2b03b499fccb825591311`.
  This fingerprints the on-disk executable at those times, not the earlier
  09:30 process. Multiple GUI processes existed; this review touched only the
  connected MCP instance and did not kill/resize other windows.

## Automation and agents

The connected tool inventory had 74 tools. Full-window capture works at 1400×900;
no resize was requested. Native window DPI/physical-to-logical mapping was not
independently measured. No desktop click/type/drag driver is exposed. Native
file dialogs, menu discovery, focus, scrolling and wizard step navigation remain
NOT TESTED. `set_ui_view` is state navigation, not an actual click.

Provider smoke tests used ephemeral Pi child processes with extensions, skills,
context discovery and tools disabled. Neither global definitions nor credentials
were edited. `openai-codex/gpt-5.4-mini` and `/gpt-5.4` were rejected by the account;
`deepinfra/zai-org/GLM-5.2` returned SUPPORT_OK.

Two bounded DeepInfra workers ran with ONLY read/grep/find/ls, no MCP or shell,
and a 420-second process timeout. Both finished. Prompts, output and process
metadata are in `support/`. Their reports are source leads, not accepted findings.

### Orchestrator verification corrections

**Reject intake worker Risk A.** It inferred that SVG/DXF stock cannot auto-size
because the GUI wrapper lacks an explicit `update_from_bbox`. Both wrappers call
`ProjectSession::add_model`, and `crates/rs_cam_core/src/session/mutation.rs:586-607`
auto-sizes from mesh OR polygon bbox. The live import also showed the correct
80×60 auto stock. The worker stopped one layer too early. Do not carry its
incorrect “SVG stock stays 100×100” conclusion into R01/R02/R04.

The same worker mentions a hypothetical MCP `rescale_model`; no such tool exists
in the inspected inventory. Its `.job` extension claim is not accepted either.
Actual fixture/load/save here uses TOML. Other source leads require their own
verification; do not equate a worker's “confirmed CODE” label with validation.

The verification worker explicitly leaves parts of export helper bodies unread.
Its bypass candidates and empty-population trigger are **not confirmed live
findings**. W01b only establishes the backend export route's observed behaviour.

## Shared fixtures and safety

Raw SVG SHA-256:
`11b95717c0071f75e24a3b2a9f8e1d44f245d4f541a8cb0dd717cf44bfa89ab0`.
The original fixture and all existing user jobs were left unchanged.

All authored projects, captures and export probes are under this programme's
`results/`. NC files are scratch evidence named REVIEW_ONLY_NOT_FOR_MACHINING;
they have not been sent to a controller and must not be used for cutting.
No library writes or machine connection. Loading the one-op saved job auto-generated
it; this was recorded, not mistaken for persisted simulation evidence.

## Next boundary

Finish the small-job checkpoint, then extend to a small STL. R01–R09 are not
complete. Arrange a short human task session before scoring discoverability or
claiming normal-path completion. Concurrent builds mean every resumed live pass
must recheck project/build state before changing it.
