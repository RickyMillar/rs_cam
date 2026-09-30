---
id: 004
state: running
commit: 5b34e710
started: 2026-09-30T20:21:00Z
finished:
---
## Summary

Release build of rs_cam_cli + rs_cam_viz into the runner worktree's own
target/ (cold release cache), then the memory repro under
systemd-run MemoryMax=10G MemorySwapMax=0.
