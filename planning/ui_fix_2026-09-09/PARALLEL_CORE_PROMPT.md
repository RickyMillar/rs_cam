# Parallel prompt — core geometry account

Written 2026-09-10 by the UI/UX programme orchestrator. This is for a
SECOND Claude account working at the same time as the UI/UX fix programme
in this directory. The two halves are disjoint: the UI programme owns
`rs_cam_viz` and the MCP surface, this prompt owns `rs_cam_core` geometry.

Paste everything below the line into a fresh Claude Code session started
in `/home/ricky/personal_repos/rs_cam`.

---

You are the core-geometry worker on the rs_cam repository. Another Claude
session is running at the same time on the UI/UX fix programme in
`planning/ui_fix_2026-09-09/`. It owns the GUI and the MCP surface. You own
core geometry. Do not cross the line.

## Set up before you do anything else

The other session holds a global build lock (`/tmp/rs_cam_gate.sh`) over the
shared `target/` directory, because cargo does not put the source path in a
workspace crate's metadata hash and two trees sharing one cache overwrite
each other's `rs_cam_core` / `rs_cam_viz` artefacts. **Do not use that
runner and do not share that cache.** Take your own:

```
cd /home/ricky/personal_repos/rs_cam
git worktree add -b core-parallel /home/ricky/personal_repos/rs_cam_wt_alt master
cd /home/ricky/personal_repos/rs_cam_wt_alt
export CARGO_TARGET_DIR=/home/ricky/personal_repos/rs_cam_target_alt
```

Every cargo command in this session runs with that variable set. With your
own cache you need no lock and you never wait for the other session.

**Watch the disk.** `df -h /home/ricky` showed 106 GiB free when this was
written. Your cache will reach roughly 25 GiB for a normal build and more if
you run the full heavy-tests gate (286 core test binaries). Check `df` before
the full gate and stop if free space falls below 30 GiB.

## Files you must not touch

These belong to the other session's three lanes. Editing one causes a
conflict neither of you can resolve cheaply:

- `crates/rs_cam_viz/src/ui/properties/**`
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
- `crates/rs_cam_viz/src/controller/**`
- `crates/rs_cam_viz/src/state/runtime.rs`
- `crates/rs_cam_viz/src/app/mcp.rs`
- `crates/rs_cam_viz/src/mcp_server.rs`
- `crates/rs_cam_viz/src/io/export.rs`
- `crates/rs_cam_core/src/session/mutation.rs`
- everything under `planning/ui_fix_2026-09-09/` except the report files
  this prompt tells you to write

If a job below genuinely cannot be done without one of them, stop and say
so rather than editing it.

## Read first

1. `CLAUDE.md` in the repo root. The lint policy is 21 denied lints; test
   modules need an explicit
   `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]`;
   `print_stdout` and `print_stderr` are denied in tests too.
2. `planning/ui_fix_2026-09-09/PLAN.md` §6 (the F4.1 row and the operator
   ruling block above it) and §11.
3. `planning/ui_fix_2026-09-09/research/R0.2.md` — the whole document,
   before job J2. §4 is the recommendation you implement.

Line numbers in every review and research document have moved. Locate the
SYMBOL, not the line.

## Your jobs, in this order

### J0 — consolidate four branches that never merged

Four branches carry finished work from other sessions and none of them is
in master. They all touch `dressup.rs`, `pencil.rs`, `scallop.rs`,
`narrate.rs` and `surface_link.rs` — the same files J1, J2 and J3 need. Land
them first or you will write your code against a tree that is about to move.

| Branch | Commits ahead | Subject |
|---|---|---|
| `linking-pencil-dial` | 4 | Pencil clearance-hop dial, link-tier measurement, G-LINKTRACE provenance |
| `isoclip-rapid` | 2 | G-TIERCONTINUOUS, G-OVERLAPFILL — tier band reporting |
| `ui-string-sentry` | 2 | G-ISOCLIPENTRY, string-hygiene sentry |
| `pipesmoke` | 2 | G-PIPESMOKE — every render pipeline builds on a headless adapter |

Each one is only a few commits ahead of a point already inside master;
master is 34 commits ahead of that point. So this is four small rebases,
not a merge project.

Do this: rebase each branch onto `master` in turn, run the gates below on
each result, and stack them onto a single branch `core-consolidation`.
Record any conflict you resolve and how.

**Do NOT advance `master` and do NOT open a pull request.** Hand the
operator `core-consolidation` with a note saying it is gated and ready.
Advancing master is the operator's decision, not yours.

If a branch fails its gate after the rebase, stop on that branch, record
what failed, and carry on with the others. Do not "fix" another session's
work to make it pass without saying so loudly.

### J1 — adopt the two red core test binaries

`arcfit_intent_key_cost_f1` and `narrate_regions_closed_c8` fail at master
`3d88406b`. They were confirmed red at master by the UI programme's
orchestrator, so they are not that programme's doing and they are nobody's
today. They pollute every gate run in the repo.

For each: find out why it fails, decide whether the CODE regressed or the
PIN went stale, and fix the right one. A stale pin gets updated with a
comment saying what moved it and when. A real regression gets a real fix.
Do not delete a test and do not loosen an assertion to make it pass —
these two files exist because the numbers they pin were argued for once
already; read the module doc comment at the top of each before you touch
it, because both explain exactly what they are protecting.

Do J1 before J2. `arcfit_intent_key_cost_f1` runs the shipped dressup chain
and pins arc counts, so J2 will very likely move its numbers, and you
cannot tell a change you caused from a failure that was already there.

### J2 — F4.1, 2.5D ramp containment

PLAN.md §6 F4.1 verbatim:

> Implement the recommended ramp containment for prism ops; add
> `entry_audit::fed_moves_outside_region` (region offset by tool radius) as
> a report-only finding and a triage safety row when nonzero.
> Acceptance: `ramp_contained_in_region_g_rampcontain.rs` on
> `fixtures/demo_pocket.svg`: zero fed entry moves outside the offset
> region; sim checkpoint 0 removes nothing outside the 70×50 outline. Keep
> the ramp option; document the degrade rule in CLAUDE.md beside
> G-RAMPTERRAIN.

The defect: `emit_ramp` in `dressup.rs` draws a straight leg of
`ENTRY_CLEARANCE ÷ tan(angle) ÷ 2` — 19.08 mm at 3°, independent of depth
per pass — from the entry point, unconstrained in XY. On
`fixtures/demo_pocket.svg` that leg runs about 11 mm past the pocket wall
and cuts the surrounding stock.

Follow R0.2 §4: **adopt option (a), fold the ramp along the following cut
polyline, with the degrade ladder R0.2 specifies**, on the branch that
emits the legacy blind legs, that is when `safety.surface` is `None`. Leave
the probe-clipped straight legs of the surface-riding ops alone — that is
G-RAMPTERRAIN's contract and it has been re-baselined once already.

Follow the research document's recommendation unless you find a hard
blocker, which you record rather than design around.

### J3 — the `cusp_radius` Bull arm

The operator ruled on 2026-09-10 that a bull-nose tool is allowed on a
Scallop operation. The UI programme's orchestrator then checked whether the
generator can honour that, and it cannot:

- `MillingCutter::cusp_radius` (`crates/rs_cam_core/src/tool/mod.rs`)
  special-cases `ToolGeometryHint::TaperedBall { tip_radius }` and falls
  through to `_ => self.radius()` for every other shape.
- `BullNoseEndmill` (`crates/rs_cam_core/src/tool/bullnose.rs`) overrides
  `corner_radius_mm` and `geometry_hint` but NOT `cusp_radius`.
- So a Ø10 bull nose with a 2 mm corner reports its cusp-forming radius as
  5.0, and `scallop.rs` drives every stepover and cusp equation from
  `cutter.cusp_radius_mm()`.

The feeds side already reads the corner radius correctly
(`feeds/cutter_constraints.rs::max_doc_scallop`,
`Bull { corner_radius } => Some(corner_radius)`). Only the geometry side
does not.

Add the `Bull` arm. Then do the part that matters: **measure what it
changes.** `cusp_radius` has consumers in at least `tier_islands.rs`,
`session/multitool.rs`, `rest_field.rs`, `tier_map.rs`, `pencil.rs`,
`finish_planner.rs`, `unified_finish.rs`, `measurement.rs`,
`finish_setup.rs`, `compute/operation_configs.rs`, `tool_shape_key.rs`,
`scallop.rs`, `reach.rs` and `compute/execute.rs`. Every one of them
changes behaviour for a bull-nose tool. Report, per consumer, whether the
new value is the right one there — `tool_shape_key.rs` in particular feeds
a cache key, so a changed value may invalidate stored work.

If any consumer wants the envelope radius rather than the cusp radius, do
not paper over it: give that call site the query it actually means and say
so in the report.

## Gates — required before every commit

With `CARGO_TARGET_DIR` exported as above:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`
3. `cargo test -p rs_cam_core -q` (and `-p rs_cam_viz -q` if you touched viz)
4. Your sentry.

Once per job, on the finished branch:
`cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`

**Never run a workspace-wide `cargo test`.** It can loop on this repo.

## Sentry requirement

Every behaviour change ships a sentry test named after the finding
(`*_g_<tag>.rs` in `crates/rs_cam_core/tests/`). **Show it FAILING on the
pre-fix code** — run it before your fix, capture the output, and quote that
output in your report and your commit body. A sentry that passes before the
fix proves nothing.

## Deliverable order — this matters

Write the code, the sentry and the report FIRST. Then run the gates. Then
commit. Then reply. Three workers on the sister programme died mid-task to
usage limits; a report on disk is recoverable, a chat message is not.

Reports go to `planning/ui_fix_2026-09-09/reports/`, named `J0.md`, `J1.md`,
`J2.md`, `J3.md`. Each contains: what changed by file and symbol, the
pre-fix failure output verbatim, the gate results with timings, every
deviation from this prompt or from R0.2 with the reason, and what the job
did NOT do.

## Rules

- Stage BY EXPLICIT PATH. **Never `git add -A`** — the checkout carries
  other developers' files.
- Commit subject: `fix(area): <TAG> — <one line>`. The body names the
  finding, the sentry, and how you showed it failing. End the body with:
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
- **Never commit to `master`. Never open a pull request.**
- **Forbidden, no exceptions:** numeric thresholds, gate bars, feeds
  constants, LUT rows; `CLAUDE.md` rules and permissions (adding one
  sentence to an existing caveat paragraph is allowed and expected when an
  operator-visible rule changes — J2 must do exactly that beside
  G-RAMPTERRAIN); `.mcp.json`; agent or auth config; `Cargo.toml` lint
  denies; launching or restarting `rs_cam_gui`; killing a process you did
  not start.
- The embedded MCP server is DOWN. There are no live GUI checks available.
  Do not start a GUI.
- **Verify before you believe.** A prior agent in this repo reported a
  false confirmed bug, and the sister programme rejected a task whose fix
  regressed a documented feature that its own worker had not noticed. Treat
  every claim in this prompt as a lead to check, including mine about
  `cusp_radius`.
- Evidence language: generated ≠ simulated ≠ Within ≠ safe. Do not write
  "verified" for something you reasoned about.

## When you finish a job

Append one line per job to `planning/ui_fix_2026-09-09/STATUS.md` — it is
append-only, never rewrite a line — with the date, the job id, the state,
the commit, the sentry, and what the job did NOT do. The other session
reads that file.

Work J0 → J1 → J2 → J3. Stop after any job and report if you run low on
context or credit; the next session picks up from your report.
