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
