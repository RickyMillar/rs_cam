# Agent debugging improvements — toolpath/sim narration & queryable diagnostics

**Audience.** A future implementation agent (or a human) tasked with making rs_cam's debugging surfaces actually useful for an LLM-driven workflow.

**Premise.** rs_cam already captures rich low-level data (`cut_trace`, `semantic_trace`, `debug_trace`, per-sample chipload/engagement/MRR). The data is there. The problem is that the *interface* between that data and an agent makes debugging slower than reading the user's prose description of what they saw on screen.

This document proposes a small, focused set of **summarization tools** that sit between the raw data and the agent. The data itself stays — the tools change how it's surfaced.

## Concrete trigger that motivated this plan

Today's session: I (the agent) spent ~2 hours trying to figure out "circular arc cuts outside boundary" on wanaka Back Rough. I had access to:

- `screenshot_toolpath` (6-view PNG composites)
- `screenshot_simulation` (6-view PNGs at checkpoints)
- `get_diagnostics` (rolling totals)
- `get_cut_trace` (277 274 samples + summary)
- `get_tool_load_report` (per-toolpath verdicts)
- `get_generation_debug_trace` (often empty)

What actually broke the bug: a **prose description** from the user — "one line down +x side, then an arc that links it to itself, this cuts outside the boundary, then it travels to center and starts adaptive clearing, then digs another layer on the same area..."

That sentence let me deduce **arc-fit + RDP + circumscribing-circle bug** in ~30 seconds. None of the structured tools surfaced it.

The takeaway: I'm bad at synthesizing 3D spatial information from numerical dumps and dim, label-free thumbnails. I'm good at pattern-matching narrative text.

## Goal

Add a **summarization layer** that produces agent-friendly observations from the existing low-level data. Three tools, ordered by priority. Each is a single Rust function the MCP exposes.

---

## Tool 1 (highest priority): `narrate_toolpath(toolpath_index)`

### What it does

Returns a chronological prose report of a single toolpath's structure and behavior. Per-Z-level events plus a "notable anomalies" list at the end.

### Output shape (target ~30–60 lines of text per toolpath)

```
Back Rough — adaptive3d, 4331 moves, 28604mm cutting, 6723mm rapid

Z-level structure (Z passes from highest to lowest, all in setup-local frame):
  z=22 (1st pass, ~3mm into stock): 1 region detected (terrain peak only,
        area 12 mm², centered ~50,50). Perimeter sweep emitted as
        1 line + 1 arc (R=70mm — SUSPICIOUS, see anomalies). Adaptive
        walk inside region: 8 offset rings. Residual cleanup: 0 cells.
  z=19 (2nd pass): regions grew to 1 region of 95mm². Perimeter sweep
        is now a full circle (R=71mm). Adaptive: 24 rings.
  z=16 (3rd pass): 1 region 99mm². Perimeter R=71mm. Adaptive: 31 rings.
  ... (compressed when uneventful)
  z=4 (last pass): residual cleanup removed 12 cells in 3 paths.

Anomalies (most surprising first):
  ⚠ 5 perimeter-sweep arcs with R > tool_radius × 30 (smallest 70mm,
    largest 71mm). For a model whose silhouette is ~100×100, an arc
    radius of ~70mm = circumscribing-circle radius. If RDP simplified
    the silhouette to a few corner points, arc-fit may be merging them
    into a single circumscribing arc — that arc would bow ~21mm
    outside the original polygon. Worth checking.

  ⚠ peak axial DOC 18mm at sample 80815 (Arc move, z=7, position
    (115.7, 87.1)). Commanded depth_per_pass = 3mm; this sample bites
    18mm in one move. Likely an Arc-fit overshoot at the deepest pass
    OR the slope-aware Cut split missing an edge case at this XY.

  ⚠ 18 rapid collisions, all in [reasons / locations].

  ℹ 80.7% of cutting samples are air-cut (radial_engagement < 0.02).
    For an adaptive3d on terrain this is high — most of the toolpath's
    "in-cut" time is the cutter brushing edges. Consider boundary
    inset or stepover tuning.
```

### Implementation sketch

A single Rust function in `crates/rs_cam_core/src/narrate.rs`:

```rust
pub fn narrate_toolpath(
    toolpath: &Toolpath,
    semantic_trace: Option<&ToolpathSemanticTrace>,
    cut_trace: Option<&SimulationCutTrace>,
    debug_trace: Option<&ToolpathDebugTrace>,
    tool: &ToolDefinition,
) -> String;
```

Uses what's already captured. Walks the toolpath grouping by Z level (use `move.target.z` clustering with epsilon). For each Z group:
- Count regions (via existing semantic_trace events, or detect via XY clustering of move sequences)
- Count adaptive rings (via direction-changes in the moves)
- Find the perimeter sweep (the longest closed-loop run within each region) and report its radius (= max distance from centroid)
- Surface anomalies (see heuristics below)

### Anomaly heuristics

These run as small predicates over the moves; each produces an entry in the report:

1. **Large arcs**: any Arc move with `radius > tool_radius * 30` → "suspiciously large arc, possibly circumscribing-circle fit".
2. **Deep axial DOC**: any cut sample with `axial_doc_mm > depth_per_pass * 1.5` → peak/p99 + the sample context.
3. **Sustained low engagement**: `air_cut_pct > 50%` → "consider boundary tuning".
4. **Repeated cleanup runs**: if residual_cleanup fires > 3 times at same XY across Z-levels → "agent walk consistently misses this area".
5. **Rapid collisions**: surface count + first 3 sample positions.
6. **Steep dz transitions**: consecutive cut points with `|dz| > depth_per_pass * 1.1` → "lift function may be bridging terrain".
7. **Full-DOC perimeter cuts**: any perimeter-sweep sample at the silhouette edge with axial_doc near depth_per_pass → "cutting full DOC at boundary, no peck".

Add new heuristics as new bug classes are discovered. Each heuristic is one function; the narrator runs them all and ranks output by "deviation from expected".

### Files to add/touch

- `crates/rs_cam_core/src/narrate.rs` (new) — the narrator + heuristics.
- `crates/rs_cam_core/src/lib.rs` (add `pub mod narrate;`).
- `crates/rs_cam_viz/src/mcp_server.rs` or wherever MCP tools are registered — expose `narrate_toolpath(index: usize) -> String`.

### Success criteria

Run `narrate_toolpath(1)` on wanaka Back Rough. The output should:

- Mention the perimeter sweep arcs and flag their radius as suspicious.
- Surface the 18mm peak axial DOC.
- NOT exceed ~80 lines.
- Be readable as English prose (no JSON dumps inline).

If an agent reads the output, they should be able to ask "is the arc-fit producing circumscribing arcs?" — i.e. the right hypothesis should leap out.

### Out of scope

- Don't visualize anything. Pure text output.
- Don't aggregate across multiple toolpaths. One toolpath per call.
- Don't try to fix the bugs you surface — just describe them.

---

## Tool 2 (medium priority): `query_moves(toolpath_index, dsl_string)`

### What it does

Filter and return cutting moves matching a small predicate DSL. Lets the agent test hypotheses cheaply rather than writing one-off Rust.

### DSL example queries

```
"axial_doc > 5"
"axial_doc > 5 and z < 10"
"kinematics = arc and arc_radius > 30"
"radial_engagement < 0.02 and chipload_mm_per_tooth > 0.05"
"abs(dz_from_prev) > 5"
"toolpath_id = 4 and z = 22"
```

### Output shape

JSON: `{matched: usize, samples: Vec<{idx, position, axial_doc, kinematics, ...}>}`. Cap at 50 results by default; full count returned separately.

### Implementation sketch

A simple recursive-descent parser for `<field> <op> <value> [and/or ...]`. Operands are sample fields (axial_doc_mm, chipload_mm_per_tooth, etc.) plus computed fields (arc_radius, dz_from_prev). Operators: `<, <=, =, >=, >`.

~150 LOC. No need for full SQL — a couple of conjunctive predicates covers the diagnostic use cases.

### Files

- `crates/rs_cam_core/src/move_query.rs` (new).
- MCP registration.

### Success criteria

`query_moves(1, "kinematics = arc and arc_radius > 30")` returns the suspicious circumscribing arcs on wanaka. `query_moves(1, "axial_doc > 5")` returns the deep-bite samples.

### Out of scope

- Don't support joins, aggregation, or sorting beyond `top N` semantics.
- Don't expose this as user-facing UI — it's an agent tool.

---

## Tool 3 (lower priority): annotated single-view SVG export

### What it does

Replace the 6-view PNG composite for `screenshot_toolpath` with an option: `view: "annotated_top_svg"` that emits a single top-down SVG with:

- Stock outline (labeled with dimensions)
- Boundary polygon (labeled, in different color)
- Toolpath colored by Z-level (rainbow gradient with legend)
- Anomaly markers: red circles around suspicious arcs, peak-DOC samples, collisions
- Crosshair at user-specified coordinate (optional)
- Coordinate axes with major/minor ticks in mm

### Why SVG

Vector text labels are readable to me. PNG thumbnails of 200×200px aren't. A single 1200×1200 SVG with 12pt labels is 10× more useful than a 6-view 1200×800 PNG.

### Files

- `crates/rs_cam_core/src/svg_export.rs` (extend if exists).
- MCP option: `screenshot_toolpath(index, view: "annotated_top_svg", path)`.

### Success criteria

Saved SVG, when I read it, lets me unambiguously answer: "is the cut at (115, 87) inside or outside the boundary?" without me having to compute coordinates manually.

### Out of scope

- Don't replace the existing 6-view PNG — add as an option.
- Don't try to render 3D — top-down only. Side views can come later.

---

## Tool 4 (nice to have): `explain_sample(toolpath_index, sample_index)`

### What it does

Given one sample index, return a paragraph explaining its provenance and context. Like:

> Sample 80815 in Back Rough: Arc move at position (115.7, 87.1, 7.0).
> Cut kinematics: ArcCW with center (105, 80), radius 12mm.
> Originally generated by: perimeter-sweep at z=7 (region #1), arc-fit
> from 5 source points (RDP-simplified from 47).
> Per-cell stamp: axial DOC 18mm, removed volume 10.5 mm³, MRR 1134
> mm³/s. Before this sample, the dexel at (115.7, 87.1) had material
> from local z=0 (stock bottom) to z=23 (= world stock surface). The
> stamp removed the column from z=7 down to z=0 — 16mm of material
> in one move.

This is the "zoom in on one weird thing the narrator flagged" tool. Lets me drill into a specific anomaly without writing custom code.

### Implementation

Walks the toolpath, debug trace, and cut trace to assemble context. Needs the move-→source-point provenance, which doesn't exist yet — would require tagging arc-fit output with the source-line indices it merged from. That's the harder part.

### Out of scope

- Don't build this until provenance tagging is in place. Useful but not load-bearing.

---

## What NOT to build (explicit)

- **Bigger 6-view dashboards.** More tiles in the screenshot composite makes things worse, not better.
- **Interactive HTML reports.** I can't interact with HTML. Static text > interactive HTML for an agent.
- **A "sentiment" verdict ("toolpath is good/bad").** Surfacing observations + scores is fine; verdicting is the agent's job.
- **A separate analysis agent that calls Claude on screenshots.** Possible but slow and token-expensive. The narrator covers 80% at near-zero cost.
- **Schema migrations on cut_trace / semantic_trace / debug_trace.** Build the new tools on top of what's already captured. If a heuristic needs a new field, add it minimally; don't redesign.

## What to do FIRST

Start with `narrate_toolpath` alone. Even a v0.1 with just three heuristics (large arcs, peak axial DOC, air cut %) would have caught most of today's bugs. Land that, see how it gets used, then iterate.

The other tools are valuable but layered on top. The narrator is the load-bearing one.

## Concrete first commit

Single PR:

1. `crates/rs_cam_core/src/narrate.rs`: `narrate_toolpath` function with anomaly heuristics 1, 2, 3 from the list above.
2. MCP `narrate_toolpath(index: usize) -> String` registration.
3. Test: load wanaka_full_tuned.toml, generate Back Rough, call narrate_toolpath(1), assert output contains the strings "perimeter sweep" and "axial DOC" and "anomalies".
4. Document in `CLAUDE.md` under MCP section: "for diagnosis, prefer `narrate_toolpath` over `get_cut_trace` + `screenshot_toolpath`".

Estimated effort: half a day for v0.1, another half day for the remaining heuristics.

## Calibration: how the agent will USE this

Workflow today (slow):
> generate → run sim → screenshot → read PNG (struggle) → get_diagnostics →
> "looks like the chipload is wrong" → get_cut_trace → write rust test
> to filter samples → eprintln debug → re-run → finally see the issue

Workflow after `narrate_toolpath`:
> generate → run sim → narrate_toolpath(1) → "ah, suspicious arc R=70mm
> on a 100×100 silhouette, that's circumscribing-circle from arc-fit" →
> grep arc-fit code → fix in 10 minutes

This is the kind of workflow that makes an agent useful for CAM debugging instead of ornamental.

---

*Drafted 2026-05-06 by Claude Opus 4.7 after the wanaka arc-fit debugging session. The arc-fit chord-deviation bug, in particular, would have been a 30-second fix instead of a 2-hour investigation if this tool existed.*

---

# Addendum 2026-05-06: narrate v2 wishlist

`narrate_toolpath` v1 shipped (commit `4826783`, tag
`rough-working-needs-tuning`) and immediately paid for itself —
wanaka air-cut diagnosis went from "clusters of screenshots + custom
Rust tests" to one tool call. After using it on wanaka the
following gaps surfaced; this section is the brief for the agent
that picks up v2.

## v2 goal

Make `narrate_toolpath` answer the next class of questions without
me writing one-off diagnostics. Specifically:

1. **"Where does the air cut come from?"** — currently narrate reports
   the `air_cut_percentage` rollup but doesn't say what's driving it.
2. **"Is the bool grid behaving sensibly at each Z?"** — currently
   narrate guesses from move clustering (calls them "region/run(s)"
   ambiguously). The actual marching-squares region count + areas live
   in the engine but never reach the agent.
3. **"What did the AgentSearch walk actually do at z=N?"** — currently
   narrate has no per-pass behavioural summary; the
   `get_generation_debug_trace` tool returns empty for adaptive3d.
4. **"Why did this specific sample read what it read?"** — was the
   `explain_sample` tool from v1; deferred for now.

## Concrete additions

### A. Radial-engagement histogram in narrate

Add a histogram section to the `narrate_toolpath` output, bucketing
in-cut samples by `radial_engagement`:

```
Engagement distribution (in-cut samples only, n=12 044):
  air      [0.00 .. 0.02]  — 81.3% (9 791) ← most cuts barely scrape sidewall
  thin     [0.02 .. 0.10]  —  9.4% (1 132)
  light    [0.10 .. 0.30]  —  6.1%   (734)
  normal   [0.30 .. 0.70]  —  2.8%   (337)
  heavy    [0.70 ..    ]  —  0.4%    (50)
```

Why: on wanaka, knowing 81% of samples land in the 0–2% bucket
(rather than e.g. 80% in the 10–14% bucket as planned by stepover)
distinguishes a bad ENGAGEMENT METRIC from a bad PLANNER. The single
"air_cut_percentage" number can't make that distinction.

Implementation: walk `cut_trace.samples`, filter `is_cutting=true`,
bucket by `radial_engagement` into the 5 ranges above, emit one line
per non-empty bucket. ~30 LOC.

### B. True per-Z region counts from the engine, not move-inferred

Currently narrate calls move clusters "region/run(s)" — terminology
fix landed in v1 but the underlying numbers are still misleading.
What we actually want:

- **marching_squares_regions** at this Z: how many polygon regions
  the bool grid produced (came from `detect_containment`).
- **cut_runs** at this Z: how many separate cut sequences the
  toolpath emits (current narrate behaviour, kept).

These tell different stories. Wanaka has 1 marching-squares region
per Z but 24 cut runs (from the agent's offset spiral). That divergence
itself is diagnostic — "lots of cut runs in 1 region" means "agent
walk is fragmenting", "lots of regions" means "material is fragmented".

Implementation: extend the existing `Adaptive3dRuntimeEvent` enum
to carry per-Z region counts and areas (already computed in
`clear_z_level_agent_2d_slice`, just not surfaced). Narrate consumes
them via the semantic_trace if present. Falls back to current
move-inference if trace is absent.

Existing event types live in `crates/rs_cam_core/src/adaptive3d/mod.rs`
around `Adaptive3dRuntimeEvent::GlobalZLevel`. Add fields like:

```rust
GlobalZLevel {
    z_level: f64,
    level_index: usize,
    level_total: usize,
    // NEW:
    marching_squares_regions: usize,
    region_areas_mm2: Vec<f64>,    // cap to top 10
    perimeter_sweep_length_mm: f64,
    agent_walk_cut_length_mm: f64,
    residual_cleanup_cell_count: usize,
}
```

### C. Surface why the semantic / debug traces are empty for adaptive3d

The `get_generation_debug_trace` tool returns:

> "Toolpath 1 has no debug trace — the operation generator didn't
>  capture one."

…even though `path.rs` clearly threads `debug_ctx` through every
sub-stage. Investigate why the trace is empty after generation. Likely
either (a) the recorder isn't being given root context for adaptive3d,
or (b) the `core_scope` debug span is being dropped before its
children can attach.

Once the trace is populated, narrate can read structured per-pass
events (`z_level`, `entry_search`, `agent2d_region`, `waterline_cleanup`,
`residual_cleanup`) and emit per-Z timing + outcome:

```
z=22 — 24 regions, 3 dropped (sub-tool), 21 cleared, 1.2s elapsed.
       Agent walk: 21 entries, 21 successful, max idle_count 3.
       Perimeter sweep: 1413mm in 198 arcs.
       Residual cleanup: 0 cells.
```

This is the "what did the planner actually do" view that's missing
today.

### D. Operation-config awareness in narrate

Currently narrate doesn't know the operation's config (stepover,
depth_per_pass, feed/RPM, boundary). It infers `depth_per_pass` from
Z spacing. Pass the toolpath's `OperationConfig` into the narrator so
it can:

- Flag samples where `axial_doc > depth_per_pass × 1.5` ("DOC
  exceeds commanded depth").
- Compare `chipload_mm_per_tooth` to operation's `feed/(rpm × flutes)`
  to detect mid-toolpath feed changes.
- Mention "stepover 0.84mm = 14% of tool diameter — narrow, expect
  many low-engagement samples" up front, so the air-cut % is
  pre-explained rather than surprising.

### E. Cross-toolpath project narration

Optional — out of scope for v2 unless cheap. A `narrate_project()`
tool that surfaces project-level anomalies:

- Toolpaths in dependency order (Pin Drill → Back Rough → Holes).
- Tool changes between TPs (which way? big-to-small? unusual?).
- Total runtime breakdown by tool / operation type.
- Verdicts rolled up per criterion.

Could be `narrate_project()` returning the same prose-first format
across all toolpaths.

## Out of scope for v2

- `query_moves` DSL (still wanted, separate tool).
- Annotated SVG export.
- `explain_sample` (still wanted, separate tool).
- Re-architecting how cut_trace / semantic_trace store data — keep
  building on top.

## Suggested implementation order

1. **A (engagement histogram)** — ~30 LOC, biggest immediate diagnosis
   payoff. Ships in one PR.
2. **B (true region counts)** — needs a small enum extension + a
   producer in clearing.rs. Ship next.
3. **C (debug trace investigation)** — investigation first, fix
   second. May or may not be a quick win.
4. **D (operation-config awareness)** — straightforward, surfaces
   implicit assumptions.
5. **E (project narration)** — optional, judge after A-D shipped.

## Calibration: how an agent would use v2

Current narrate output answer: "air cut is 81%, here are some
anomalies, oh well."

After A: "air cut is 81% AND 81% of in-cut samples are in the
0–2% engagement bucket — so the planner's emitting moves at low
engagement, not just bad metric calibration. Look at stepover."

After B: "single region per Z, 1 perimeter sweep ~377mm, 24 cut
runs from agent walk — air cut isn't from fragmented material,
it's from the perimeter sweep covering 100mm of mostly-air at every
Z."

After C: "z=22 perimeter sweep took 1.2s of cutting time but only
removed 8mm³ of stock. Most of the silhouette boundary at z=22 has
no material above. Skip the perimeter sweep at this Z."

That last conclusion is the kind of thing an agent should be able
to reach in one tool call rather than two hours of debugging.
