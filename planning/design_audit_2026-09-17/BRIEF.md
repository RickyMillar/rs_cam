# Design and feature-debt audit — 2026-09-17

The tree was regrouped today (24 core folders, per-folder `CLAUDE.md`). This
audit sends one read-only agent into each folder group to find design
patterns that would improve or simplify the code, and feature debt that no
gate can fail on. Output is one file per group in this directory, then
`SYNTHESIS.md` ranks across groups.

## Ground rules for every auditor

- **Read-only.** No edits, no cargo, no scripts that write outside this
  directory. You may run `rg`, `ls`, `wc`, `git log -p`, `python3` for
  counting.
- Read the crate `CLAUDE.md` and each folder `CLAUDE.md` in your group
  first. They name the invariants and the sentries. A finding that
  contradicts an invariant is wrong unless you show the invariant is stale.
- Code and a named sentry outrank prose. Any planning document dated
  before 2026-08-04 is superseded evidence; do not cite it as behaviour.
- **Write incrementally.** Append to your output file after every three
  to five findings. A session limit can end you at any moment; a partial
  file is the deliverable, an unwritten one is nothing.
- Evidence is an `rg` count or an exact `file:line` anchor you read. A
  claim without an anchor is not a finding.
- Breaking changes are fine (operator ruling 2026-09-16: no legacy
  support). Say when a proposal breaks a wire key, a file format or a
  public signature.
- `feeds/` and `tool_load/` are owned by a concurrent "power" session;
  tag every finding there `owner: power session` and do not propose
  edits to their numbers or physics.

## Do not propose (checked before, live or ruled)

- `AgentSearch`, all three `ClearingStrategy3d` variants and
  `clear_z_level` are live (2026-06-08).
- `SimGroupEntry.direction` is live (S5 cache key, bench producer).
- `crate::flow_accum` stays; the pencil NMS detector stands (2026-09-03).
- `COLLISION_POINT`, `SPACE_0`, `INK_00`, `LANE_SCALE`,
  `draw_trace_badge`, `row_hover_tint` and `planning/ui_premium_2026-09-13`
  belong to another account. Do not touch or cite as debt.
- `polygon.rs` at 3 004 lines is ruled to stay one file.
- `feeds::calculate` (1 302 lines) has no mechanical seam (T-5).
- Re-export shims for moved modules were deliberately not added.

## What counts

**Design pattern findings** (the code would be simpler or safer):

- A god function (> 200 lines) with a visible seam, or a struct that
  every layer mutates.
- A parameter struct with more than eight fields, or boolean flags that
  encode a mode (an enum would name it).
- A stringly-typed door: dispatch on a string that names a kind, an
  operation or a field.
- Enum dispatch replicated in N places: count how many files an engineer
  touches to add one operation, one command, one dressup, one overlay.
- An ad-hoc cache or memo beside the bounded ones in `maps/`.
- One concept with two representations (core vs viz, IR vs record,
  config vs params) and a hand-written translation between them.
- Unit-less numbers where a newtype exists elsewhere in the crate
  (`quantities.rs`, `measurement.rs`).
- Error handling: `String` errors, `map_err` that drops the cause,
  `Option` where a reason is known.
- Cloning where a borrow would do; `Vec` rebuilt per frame or per call.
- Duplicate helpers: `evidence_round2/dup_sweep_0.88_src.md` in
  `planning/structure_2026-09-17/` lists 118 candidate pairs. Use it as
  input, verify by reading; do not re-list it.
- A `pub` item used only from one file or only from tests
  (`evidence_round2/dead_pub_surface.md`, `test_only_pub_api.md`).

**Feature debt findings** (a capability that is half there):

- A dial or config field nothing reads, or a field read but never set
  from any surface (GUI, MCP, CLI, project file).
- A `FEATURE_CATALOG.md` claim without a live path from a product surface.
- An `#[ignore]` harness that is research, not product, still shipped as
  a test target.
- An operation, command or diagnostic missing from one of the four
  surfaces when the other three have it.
- A `TODO`, `FIXME`, `HACK` or a "for now" comment with a live consequence.
- A guard, refusal or abstention that reports the wrong thing (a hard
  zero where "not measured" is the truth).

## Finding format

One block per finding, in this exact shape so the synthesis can parse it:

```
### <GROUP>-<NN> <short title>
- kind: design | feature-debt
- pattern: <one of the names above, or your own in three words>
- where: `path/file.rs:line` (one or more)
- evidence: <rg count or the anchor you read; quote at most two lines>
- proposal: <the change in one to three sentences>
- breaks: none | <what>
- effort: S | M | L
- risk: low | medium | high — <why>
- sentry: <existing test that guards it, or the test you would write>
- owner: <blank, or "power session">
```

Aim for eight to twenty findings; quality over count. End the file with:

- `## Top three` — the three findings with the best benefit per effort.
- `## Checked and clear` — things you suspected and found fine, one line
  each, so nobody re-audits them.
- `## Add-a-thing count` — for the group's main extension point (an
  operation, a command, a panel, a post), the list of files an engineer
  edits today.

## Groups

| Group | Folders | Output |
|---|---|---|
| core-finish | `crates/rs_cam_core/src/finish/` | `core-finish.md` |
| core-compute | `crates/rs_cam_core/src/compute/` | `core-compute.md` |
| core-session | `crates/rs_cam_core/src/session/` | `core-session.md` |
| core-tool_load | `crates/rs_cam_core/src/tool_load/` | `core-tool_load.md` |
| core-feeds | `crates/rs_cam_core/src/feeds/` | `core-feeds.md` |
| core-stock | `stock/`, `dexel_stock/` | `core-stock.md` |
| core-cutting | `ops/`, `adaptive/`, `adaptive3d/`, `dressup/` | `core-cutting.md` |
| core-fields | `geometry/`, `surface/`, `maps/`, `trace/` | `core-fields.md` |
| core-edges | `gcode/`, `export/`, `io/`, `tool/`, `machine/`, `material/`, `diagnostics/`, `metrology/`, `util/` and the spine files at `src/` root | `core-edges.md` |
| viz-ui | `crates/rs_cam_viz/src/ui/` | `viz-ui.md` |
| viz-shell | `controller/`, `state/`, `app/`, `compute/`, `render/`, `io/`, `interaction/` | `viz-shell.md` |
| cli-mcp | `crates/rs_cam_cli/`, `crates/rs_cam_mcp/` | `cli-mcp.md` |
