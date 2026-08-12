# B-5 census rig — GUI-mode results parity (G-RESULTS)

Drives an **isolated** `rs_cam_gui --mcp` through load → generate → simulate
→ the reads under census, and dumps everything as one JSON so the diff
against the CLI's `project` output can be done offline.

## Never point this at the operator's live GUI

It launches its own process, and it must be given a **copy** of a project
file. The operator's `planning/airrun_2026-06-01/wanaka.toml` is read-only
forever.

## Files

| file | what it is |
|---|---|
| `gui_census.py` | the driver. Reuses B-1's dependency-free stdio client (`../b1/mcp_stdio_client.py`) and its RSS sampler / MemAvailable safety valve. |

## Running

```sh
SCRATCH=/tmp/b5
mkdir -p $SCRATCH
cp planning/airrun_2026-06-01/wanaka.toml $SCRATCH/wanaka_b5.toml   # COPY

# GUI surface
python3 planning/review_2026-08-08/artifacts/b5/gui_census.py \
    --project $SCRATCH/wanaka_b5.toml --log-dir $SCRATCH/gui_run --resolution 0.35

# CLI surface, SAME cell size
target/release/rs_cam_cli project $SCRATCH/wanaka_b5.toml \
    --output-dir $SCRATCH/cli_out --resolution 0.35 --summary
```

Pin the same `--resolution` on both sides: collision counts and engagement
both move with the cell, so a census that lets them differ is comparing two
projects.

`--generate-indices 0,2` generates those toolpaths one at a time instead of
running `generate_all`. Cheap mode for checking the *shape* of a response on
a debug binary; the numbers from such a run are not a census.

Measured 2026-08-13 on wanaka at 0.35 mm: GUI 486 s generate (3 fixpoint
rounds, 2 simulations) + 21 s simulate, peak RSS 3.4 GB; CLI 38 s total —
and 3 of 7 toolpaths, because `rs_cam_cli project` has no fixpoint loop
(RESULTS_PARITY.md residual R-4).

## Output

`gui_run/gui_census.json` — `initialize`, `load_project`, `list_toolpaths`,
`generate_all`, `run_simulation`, `tool_load_report`, `project_diagnostics`,
and per toolpath index: `diagnostics`, `narration`, `spans`, `params`. Plus
`rss.csv` and `gui_stderr.log`.

The raw captures are **not** committed — a wanaka census JSON is ~258 KB of
project-specific trace. The findings are in
`../../RESULTS_PARITY.md`; the rig is here so the run reproduces.
