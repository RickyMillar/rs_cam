# B-1 repro rig — G-LV.2

Reproduces the two-step G-LV.2 recipe against an **isolated** `rs_cam_gui --mcp`
instance and censuses MCP response sizes. Written for TD3 wave B-1; kept so
B-2's sentry and any future crash capture can reuse it.

## Never point this at the operator's live GUI

It launches its own process. Use a **copy** of a project file; never the
operator's `planning/airrun_2026-06-01/wanaka.toml`.

## Files

| file | what it is |
|---|---|
| `mcp_stdio_client.py` | dependency-free MCP-over-stdio client + `/proc` RSS sampler with a MemAvailable safety valve. Two launch modes: `direct` and `gdb` (gdb is the parent, because `yama/ptrace_scope == 1` forbids attach-after-launch). |
| `glv2_repro.py` | the driver: load → `generate_all` fixpoint → `run_simulation` → bounded-read census → the unfiltered `get_cut_trace` last. Writes `calls.jsonl` (one row per call, with per-section byte accounting on large payloads), `rss.csv`, `gui_stderr.log`, `gdb.log`, `tools_list.json`. |
| `pipe_teardown_probe.py` | three isolated transport-teardown probes (stdin EOF / reader closes mid-write / both fds closed). No project needed. |
| `census_table.py` | turns a `calls.jsonl` into the markdown census table. |
| `measurements/` | the raw rows behind `../../GLV2_CRASH_CAPTURE.md` (`full_text` payloads stripped). |

## Running

```sh
SCRATCH=/tmp/b1
cp planning/airrun_2026-06-01/wanaka.toml $SCRATCH/wanaka_copy.toml   # COPY

python3 glv2_repro.py --log-dir $SCRATCH/run --project $SCRATCH/wanaka_copy.toml \
    --mode gdb --resolution 0.1 --gen-timeout 2700 --sim-timeout 2400

python3 pipe_teardown_probe.py $SCRATCH/teardown
python3 census_table.py $SCRATCH/run/calls.jsonl
```

Flags: `--short-census` skips the per-index sweeps; `--no-fatal` stops before
the unfiltered call; `--wayland` keeps `WAYLAND_DISPLAY` set (the run then
usually aborts at `load_project` — see GLV2_CRASH_CAPTURE.md §4b).

`WINIT_UNIX_BACKEND` is **inert** on winit 0.30; the client forces X11 by
unsetting `WAYLAND_DISPLAY`. One full run is ~10 min of wall clock and peaks
around 8.8 GB RSS during the 0.1 mm simulation — check `free -g` first.
