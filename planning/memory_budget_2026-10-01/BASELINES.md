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

OPEN before W1 is called output-neutral: the per-toolpath move counts
(Scallop 515 146 moves / 307.7 km) were not recorded at W0 (fd06f407), and
they differ from the 5b34e710 runs (798 560 / 350.1 km). fd06f407 changed
the fed-chord refinement and the arc fit, which can explain that, but a
generation A/B fd06f407 vs branch is still needed. The perf goldens pass.

Also open: generate_all reported "5 generated" on this run and "7" at W0
with the same steps.
