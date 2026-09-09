# Handoff prompt — rs_cam UX review programme

Paste this into the new session (or just reference this file):

> You are continuing an in-progress, **review-only** UX review of the rs_cam
> desktop CAM app. Start by reading, in order:
> 1. `planning/ui_review_2026-09-09/results/STATUS.md` (current state — entry point)
> 2. `planning/ui_review_2026-09-09/PROTOCOL.md` (evidence rules — binding)
> 3. `planning/ui_review_2026-09-09/PLAN.md` and `TOOLING.md` (programme + tool limits)
> 4. `planning/ui_review_2026-09-09/results/W00/REPORT.md`, `W01/CHECKPOINT.md`,
>    `W02/REPORT.md` (work done so far)
>
> Your assignment: **R03 (tools/toolpath authoring) live on your OWN GUI
> instance**, plus the read-only source tracks for R04/R05/R08 in parallel.
> Another session owns R06/R07 on its own live instance; do not duplicate them.
>
> Hard rules:
> - Review-only. NEVER edit application source, tests, `.mcp.json`, agent/auth
>   config, or user projects. Other developers are actively working in this
>   repo — do not build, run cargo test, kill processes, or restart any GUI.
> - All writes go under `planning/ui_review_2026-09-09/results/` only — yours
>   under `results/R03/` and `results/W03/` (your scratch). Scratch
>   projects/exports are named REVIEW_ONLY and never for machining.
> - **GUI ownership:** you may launch your OWN dedicated instance
>   (`target/release/rs_cam_gui --mcp` from a spare terminal — no cargo
>   build) and drive only that. Before mutating anything, verify WHICH instance
>   you are attached to via `project_summary`: the other session's instance
>   holds "REVIEW ONLY - 20mm terrain". If your MCP sees that project, STOP and
>   re-check your instance/port wiring; never mutate the other session's live
>   job. Record the `build` block — it was `8a4df241-dirty` while git HEAD was
>   `ef91cb03` and moving.
> - **No shared scratch state:** copy seeds into your own `results/R03/scratch/`;
>   never edit a file another session created. Coordinate by APPENDING to
>   `results/STATUS.md` (dated, session-named), never by rewriting another
>   session's findings.
> - Evidence labels (HUMAN/DESKTOP/MCP-VIEW/MCP-STATE/CODE) are mandatory;
>   MCP navigation is not click-path evidence. Untested means untested.
> - Supporting agents: read-only only, verified `deepinfra`/`zai-org/GLM-5.2`
>   via `pi --provider deepinfra --model zai-org/GLM-5.2 --thinking low
>   --no-extensions --no-skills --no-prompt-templates --no-context-files
>   --no-session --tools read,grep,find,ls -p "<prompt>"`. Codex models were
>   rejected by the account. Verify agent claims against source before
>   accepting them (a prior agent's "confirmed" SVG stock bug was false).
> - Read `CLAUDE.md` diagnostic caveats before interpreting any metric.
>   Generated ≠ simulated ≠ Within ≠ physically safe.
> - Findings format, severity scale and report structure are in PROTOCOL.md.

## Parallel tracks — two GUI instances, disjoint packages

Two sessions with separate live instances work fine; assign disjoint packages
so screenshots/scratch/reports never collide:

| Session | Packages | Notes |
|---|---|---|
| Owning session (resuming) | R06 drill-down (Annotation 15, plunge finding), R07 wizard/preflight/export routes | Its instance already holds the authored 20mm terrain job; scratch in `results/W01b/`, `results/W02/` |
| New session (this prompt) | R03 live on its OWN instance (fresh SVG/STEP import → tools → first ops, incl. the two-tool wrong-order and ball-required refusal cases) + B-source-tracks for R04/R05/R08 | Fresh seeds into `results/R03/scratch/`; do NOT load the terrain job |
| Either, read-only | D — consolidation, cross-references, Rxx report skeletons from briefs | Reads only |
| Human (Ricky) | C — unaided journeys | Highest-value missing evidence; do not coach; record in PROTOCOL.md trace format |

Track B (source investigations: R04 apply/provenance semantics, R05
rest-chain/planner ownership, R08 persistence matrix) needs no GUI and can run
as parallel read-only agents from either session at any time.

End-of-session duties: append your findings and live-state notes to
`results/STATUS.md`; leave the next session a dated pointer to your scratch
directory and any open questions for the GUI-owning track. Do not attempt
Wave-3 interaction sweep or synthesis until human journeys (C) exist.
