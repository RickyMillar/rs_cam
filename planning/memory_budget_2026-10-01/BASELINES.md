# Memory programme baselines

The baselines land before any fix is measured. Each wave adds a row with
the same protocol, so each row is a paired A/B against W0.

## Protocol (P1)

1. Release `rs_cam_gui` under `systemd-run --user --scope -p MemoryMax=16G
   -p MemorySwapMax=0`, started by the `rs-cam-capped` MCP server.
2. MCP `load_project` rivmap350.toml
   (`~/Downloads/big/rivmap_export_350_500/`; not in the repo).
3. MCP `generate_all` with `simulation_resolution_mm: 0.2`.
4. GUI Simulate at 0.2 mm, "Capture cutting metrics" OFF, the resolution
   NOT changed first (a change drops the old result and hides the effect).
5. Sample `/proc/PID/smaps_rollup` Rss and Anonymous every 2 s; mark each
   step with a UTC time.
6. Read the value at rest one minute after the simulation ends.

The OOM kill of the cgroup reports rc 0: read `journalctl --user` for
"oom-kill".

## W0: master fd06f407 (2026-10-01 UTC 22:48-22:56)

| Step | RSS (GiB) | Model (PLAN.md) |
|---|---|---|
| Load | 0.65 | — |
| generate_all end (closing simulation held) | 8.06 | load + R, R = C x 1476 B = 7.2 GB |
| Simulate, peak (22:54:52) | 14.64 | old R + new R + base = 14.8 |
| Rest after the simulation | 8.94 | base + R |

The memory did not fall when the simulation started. This agrees with
finding M1 (the old result is held for the whole run). Logs:
`w0/w0_007_rss.log` (time, Rss kB, Anonymous kB), `w0/w0_007_marks.log`.

Earlier runs at 5b34e710 (job 006 on `comms/runner`) are not comparable:
the resolution changed before some of them, and one had metrics ON.

## W1: branch c635f721 (groups A-D merged; 2026-10-02 UTC 00:13-00:21)

Same protocol P1. The `rs-cam-mem` MCP server started the branch release
GUI under the same cgroup. The operator clicked Simulate at 00:15:5x
(the mark in `w1_marks.log` is late; the RSS log shows the start).

| Step | W0 (GiB) | W1 (GiB) | Change |
|---|---|---|---|
| Load | 0.65 | 0.65 | — |
| generate_all end (closing simulation held) | 8.06 | 4.28 | -47 % |
| Simulate, peak | 14.64 | 7.52 (00:16:20) | -49 % |
| Rest after the simulation | 8.94 | 5.28 | -41 % |
| Simulation wall time | ~25 s | ~27 s | — |

Rapid collisions: 8 in both, all in "3D Rough 8". `get_diagnostics` now
publishes null plus `cut_metrics_not_measured` for the trace figures
(group D, U2), where W0 published 0.0.

CLOSED (W2 CLI A/B below): fd06f407 itself emits the 515 146-move
scallop; the branch does not change generation.

CLOSED (wave-2 group H): "5 generated" vs "7" depends only on whether the
GUI auto-regeneration (500 ms after load) made two 2.5D operations current
before generate_all arrived; the plan skips them without counting them.

## W2 (CLI, metrics ON): master fd06f407 vs branch b3a4a7ac (2026-10-02 UTC 00:59-01:07)

Protocol P2: `rs_cam_cli project rivmap350.toml --resolution 0.2
--output-dir <dir> --summary` under the same cgroup (16 GB), wrapped in
`/usr/bin/time -v`. The CLI always captures cutting metrics and full
debug traces. Runs were sequential on an otherwise idle lane.

| | A: master fd06f407 | B: branch b3a4a7ac |
|---|---|---|
| Max RSS | 10 211 048 kB (9.74 GiB) | 7 286 720 kB (6.95 GiB), -29 % |
| Wall | 3:57 | 3:55 |
| Samples / air % / time | 5 159 038 / 17.8 % / 35 684 s | identical |
| Rapid collisions | 8 (3D Rough 8) | identical |

Output neutrality: `simulation.json` (6.76 GB) is byte-identical (`cmp`).
`summary.json` differs only by the new `cut_metrics_not_measured: null`
key (U2). The `tp_*.json` files differ only in `elapsed_us` timing fields.
Logs and summaries: `w2/`. The CLI has no GUI copies, so this isolates the
core part of M2/M3/M8.

## W3: branch 51e1bccd (waves 1-3; 2026-10-02 UTC 02:05-02:15)

Protocol P1 (rs-cam-mem MCP GUI, same cgroup). After generate_all the
2026-05-26 Shapeoko `$$` (kinematics) was imported by MCP; the project
file was not saved.

| Step | W0 | W1 | W3 |
|---|---|---|---|
| generate_all end (GiB) | 8.06 | 4.28 | 4.03 |
| Simulate, metrics OFF, peak | 14.64 | 7.52 | 4.79-5.08 (three re-runs) |
| Rest after the simulation | 8.94 | 5.28 | 4.73-4.83 |
| Simulate, metrics ON, 0.2 mm, peak | OOM-killed at 16 GiB (5b34e710 and fd06f407) | not run | **6.87** (02:14:23) |

At each re-run the RSS fell to about 1.8 GiB at the start: the view
releases the old result first (M1, W2-E).

The metrics-ON run gave the operator's first full verdict on rivmap350 at
0.2 mm: cycle time 12:44:39 (wall clock; the cutting-only figure was
7:41:30), load 5/7 within, exceeds 2/7 ("3D Rough": depth of cut 29 %
above the flex ceiling), 8 rapid collisions in "3D Rough 8" (first at move
27653, a rapid from (46.0, 23.3, 31.0) to (46.0, 23.3, 21.486)).
Screenshot: `w3/w3_metrics_on.png`.

Seen once and not reproduced: after the first metrics-OFF run the GUI read
"Results may be stale (params changed)" and "cutting only: not simulated".
The MCP machine import ran next to the start of that run (a machine edit
invalidates the session's result). Two clean re-runs did not show it.
Follow-up: during a re-run after that state the panel read "Ready to
simulate", not "Running — previous result released", because the session
held no run.
