# AgentSearch Wandering/Looping Diagnosis Plan

**Status:** ready to execute after MCP server restart
**Context:** user reports AgentSearch "wanders all over" and "gets stuck
looping around small areas for ages". This plan sets up the diagnostic
probe framework and defines what "fixed" looks like.

## What we built this session (A+B)

1. **Audited** every counter, scope, exit_reason, and hotspot in
   `clear_z_level()` (clearing.rs:788–1420). Already extensive:
   - Per-pass: `step_count`, `idle_count`, `search_evaluations`,
     `yield_ratio`, `exit_reason`, `xy_bbox`
   - Per-level: `passes`, `long_passes`, `short_passes`,
     `skipped_preflight`, `total_steps`
   - Hotspot kind: `"adaptive3d_pass"` with `low_yield_exit_count`

2. **Found the gap**: all of the above is captured on
   `ToolpathDebugTrace` during generation (stored on
   `ToolpathComputeResult.debug_trace`) — but **no MCP tool exposed
   it**. `get_cut_trace` returns simulation-time metrics only.

3. **Landed** `get_generation_debug_trace` MCP tool (commit `04d3782`)
   that surfaces:
   - Filtered spans (by `span_kind`, `exit_reason` substring,
     `max_yield_ratio` threshold)
   - A `diagnostics` summary with `passes_by_exit_reason` histogram,
     `looped_passes`, `idle_passes`, `low_yield_passes`,
     `avg_yield_ratio`, `worst_yields` top-10

4. **Audited** the direction-search scoring in
   `search_direction_3d_with_metrics` (search.rs:200–370):
   - `score = |eng - target_frac| + (angle_diff / PI) * 0.12`
   - Phase 1: 7 narrow candidates ±{0,15,30,45}°; bail if `score < 0.15`
   - Phase 2: 18 coarse 360° candidates + bracket refinement
   - Low-engagement rejection: `eng < 0.001` → skip
   - The `0.12` angle-penalty weight IS what produces arcs (correct),
     but also what drives zigzag wandering when engagement is noisy

## Probe workflow (execute after restart)

### Step 1 — Build the fixture

```
load_project sample_3d_project.toml  (or import terrain_small.stl)
add_setup "AgentSearch Diagnosis"
add_toolpath type=adaptive3d tool=0 model_id=<terrain>
set_toolpath_param stock_top_z=<stock.max.z>
set_toolpath_param clearing_strategy=agent_search
```

Use a **small mesh** (terrain_small.stl is 40K tris, ~3-4 min gen time)
or make_hemisphere_mesh via a test. The user's note: "generation takes
a while" — budget 3-5 minutes per probe on terrain_small.

### Step 2 — Generate + diagnostic snapshot

```
generate_toolpath 0
get_generation_debug_trace index=0 max_spans=0
```

This returns the full diagnostics summary. Key fields to inspect:

```json
{
  "diagnostics": {
    "pass_count": <N>,
    "passes_by_exit_reason": {
      "loop closed": <count>,     ← LOOPING
      "idle": <count>,             ← STUCK
      "no material": <count>,      ← GOOD — pass completed
      "no entry": <count>,         ← no more material found
      "preflight skip": <count>,   ← search gave up pre-entry
      "no viable direction": <count>
    },
    "low_yield_passes": <count>,   ← WANDERING
    "avg_yield_ratio": <ratio>,    ← overall efficiency
    "worst_yields": [...]          ← the bad passes
  }
}
```

### Step 3 — Visual reference check

```
screenshot_toolpath 0 path=/tmp/agentsearch_baseline.png show_stock=true
screenshot_simulation path=/tmp/agentsearch_sim.png
```

What **good** looks like (from user's reference: "classic adaptive arc
toolpaths"):
- Smooth arcs curving around concave corners
- Each pass is a spiral or trochoidal loop that progressively clears
  inward from the boundary
- No retracing: passes don't revisit already-cleared territory
- Short rapids only between passes (retract → rapid → plunge)

What **bad** looks like (the current symptom):
- Zigzag scribble across wide areas (wandering)
- Dense knot of overlapping passes in one spot (looping)
- Long rapids between distant patches (tool is jumping around)
- Many short passes interspersed with rapids

### Step 4 — Filtered probes

After the snapshot, drill into the worst passes:

```
# Show only the passes that looped
get_generation_debug_trace index=0 span_kind=adaptive_pass exit_reason=loop

# Show only low-yield passes (< 10% material yield)
get_generation_debug_trace index=0 span_kind=adaptive_pass max_yield_ratio=0.1

# Show only the preflight skips
get_generation_debug_trace index=0 exit_reason="preflight skip"
```

For each bad pass, inspect:
- `xy_bbox` — how far did the pass wander? (wide bbox + few steps = wandering)
- `idle_count` — how many steps produced no material removal?
- `step_count` vs `idle_count` ratio — above 50% idle = looping signal
- `z_level` — does the problem concentrate at specific Z levels?

### Step 5 — Parameter sensitivity

Vary one knob at a time and re-probe:

| Parameter | What it tests | Range |
|---|---|---|
| `stepover` | Wider step = more engagement per step, less wandering | {1, 2, 3} |
| `tolerance` | Finer tolerance = smaller steps, more direction-search evaluations | {0.05, 0.1, 0.5} |
| `depth_per_pass` | Deeper cut = more material per step = higher yield | {2, 3, 5} |

After each, re-run `get_generation_debug_trace` and compare `avg_yield_ratio`, `looped_passes`, `idle_passes`. Track the deltas.

## What "fixed" looks like — success criteria

| metric | current (hypothesized) | target |
|---|---|---|
| `looped_passes` | many | ≤ 2 per Z level |
| `idle_passes` | many | ≤ 1 per Z level |
| `avg_yield_ratio` across all passes | < 0.3 | > 0.5 |
| `low_yield_passes` / `pass_count` | > 30% | < 10% |
| Visual: screenshot shows arcs | zigzag / scribble | smooth arcs |
| Visual: no retrace bands | dense overlap knots | clean clearance spiral |

## Tuning hypotheses to test

### H1: Angle penalty too low (wandering, zigzag)

**Observation**: `score = |eng - target_frac| + ad * 0.12`. A 90°
turn costs only 0.06 in score. If two adjacent steps each choose
"slight engagement improvement at 45° turn", the tool zigzags.

**Proposed change**: raise the angle weight from `0.12` to `0.20`.
This costs more for turns, biasing toward smoother arcs. Test: does
`avg_yield_ratio` increase and do the "wide-bbox + low-yield" passes
reduce?

**Risk**: too-high weight → tool ignores good engagement and runs
straight into walls. The Phase 1 `score < 0.15` bail-out threshold
may also need adjustment.

### H2: Idle threshold too lenient (looping, stuck)

**Observation**: `idle_count > 20` means the tool makes 20 consecutive
steps without clearing material before bailing. At `step_len ≈ 0.5mm`,
that's 10mm of wasted motion per occurrence.

**Proposed change**: lower to `idle_count > 10`. 5mm of idle wandering
is enough to know the pass should bail.

**Risk**: might terminate passes too early on geometry where engagement
is intermittent (thin walls with alternating air/material). Trade-off:
more short passes but less total wasted motion.

### H3: No visited-cell penalty (re-tracing)

**Observation**: the direction search doesn't know where the tool has
already been. A direction with `eng = 0.001` (barely above the `0.001`
rejection threshold) still scores as a valid candidate, even if it
points back into already-cleared territory.

**Proposed change**: add a "visited cell" penalty. Track a boolean
grid of cells the tool has visited on this pass. Penalize directions
that point into mostly-visited cells. This is the most impactful
change for the "stuck looping in the same area" symptom.

**Risk**: memory overhead (boolean grid per pass). Moderate
implementation complexity. The grid can reuse the same dimensions as
`MaterialGrid` from the 2D adaptive — just mark a cell when the tool
stamps it.

### H4: Entry-point spread is defeated (re-entering same spot)

**Observation**: `find_entry_3d` uses `pass_endpoints` to avoid
re-entering near previous entry points. But after 50+ passes on a Z
level, the spread is exhausted (check: `if too_close && pass_endpoints
.len() < 50`) — then ANY nearest-material cell is valid, including one
adjacent to a previous entry.

**Proposed change**: remove the `< 50` guard, or increase it to 200,
so the spread remains active longer.

**Risk**: on simple geometry with few valid entry points, the spread
check could starve the algorithm of ANY entry point, ending the Z
level prematurely.

## Execution order

1. **Probe first, tune second.** The `get_generation_debug_trace`
   tool gives us the numbers before we change anything. The
   diagnostics summary tells us exactly which hypothesis is dominant.

2. **Visual reference first.** Screenshot the current AgentSearch
   output on terrain_small.stl and compare against the
   ContourParallel variant. The ContourParallel screenshot is the
   "reference" for what clean cutting looks like.

3. **One change at a time.** Apply H1, re-probe, compare diagnostics.
   Then revert H1 and try H2. Then combine the winners.

4. **No fingerprint grief.** Any change to AgentSearch's scoring or
   thresholds will change the param_sweep fingerprints for the 4
   adaptive3d sweep families (which use default_params → now
   ContourParallel per Package G, so only sweeps that explicitly
   select AgentSearch are affected). Accept the delta.

## Files involved

| File | What lives there |
|---|---|
| `adaptive3d/search.rs:200-370` | `search_direction_3d_with_metrics` — scoring function |
| `adaptive3d/search.rs:255` | The `ad * 0.12` angle-penalty weight (H1) |
| `adaptive3d/clearing.rs:1211` | `idle_count > 20` threshold (H2) |
| `adaptive3d/clearing.rs:1110` | `idle_count` accumulator + reset logic |
| `adaptive3d/search.rs:440-620` | `find_entry_3d` + `scan_entry_3d_bounds` — entry-point spread (H4) |
| `adaptive3d/path.rs:289` | `ClearZLevelContext.max_link_dist` — stay-down distance |
| `rs_cam_mcp/src/server.rs` | `get_generation_debug_trace` — newly added MCP tool |

## What we already fixed that helps

- **Package B**: exposed AgentSearch in the GUI/MCP so we CAN select it
- **Package F**: added `is_clear_path_3d` gate on EDT link decisions
  (also fires for AgentSearch via the shared clearing.rs code)
- **Phase 3**: lift-to-safe-z before XY traverse in `segments_to_toolpath`
  (eliminates diagonal through-material rapids for ALL strategies)
- **Package L**: issue coalescence (actionable segment-level issues
  instead of per-sample noise)
- **This commit**: `get_generation_debug_trace` MCP tool (surfaces the
  per-pass diagnostic data that was captured but unreachable)

## Restart instructions

1. Rebuild: `cargo build --release` (or just `cargo build -p rs_cam_mcp`)
2. Restart the MCP server (or rs_cam_gui with `--mcp`)
3. Open a new Claude Code session
4. Paste:
   ```
   Load planning/agent_search_diagnosis_plan.md. Run the probe workflow
   on terrain_small.stl with clearing_strategy=agent_search.
   Start with Step 1-3 (fixture + generate + diagnostic snapshot +
   screenshots), then report what the diagnostics summary says.
   ```
5. The `get_generation_debug_trace` tool will be available as a
   deferred MCP tool — load it via ToolSearch.
