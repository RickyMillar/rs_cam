---
id: 006
state: done
commit: 5b34e710
started: 2026-09-30T20:38:46Z
finished: 2026-09-30T20:54:42Z
---
## Summary

rivmap350.toml in the release GUI at 5b34e710, under systemd-run
MemoryMax=16G MemorySwapMax=0. RSS sampled every 5 s. Ricky drove.

| Step (UTC) | Peak RSS | Notes |
|---|---|---|
| Load + generate all (20:41–20:50) | 8.73 GB | 0.8 -> 7.9 GB in 30 s at +210 s, then flat at 8.14 GB for 7 min; 8.73 GB at the end |
| Simulate 0.5 mm (20:50) | 2.53 GB | RSS dropped 8.78 -> 1.48 GB when the sim started; sim done in well under 1 min |
| Simulate 0.2 mm (20:52) | 9.17 GB | 2.5 -> 9.1 GB in 30 s, flat, then released to 2.0 GB; Ricky: "running noticeably smoother" |
| Simulate 0.1 mm (20:54, Ricky's extra test) | > 16 GB: OOM-killed | 10.09 GB at the last sample, then over 16 GB within 5 s |

0.1 mm kill, from the kernel log:
  Memory cgroup out of memory: Killed process 918703 (rs_cam_gui)
  total-vm:23870700kB, anon-rss:16736460kB
  scope: Failed with result 'oom-kill'; 16.0G memory peak, 0B swap peak.
The desktop was not affected.

GUI responsive: yes at 0.5 and 0.2 (Ricky). No panic or error text on
stderr (the GUI writes none). systemd-run --scope returned rc=0 even for
the OOM kill, so read the journal, not the exit code.

Not yet reported by Ricky: rapid collision count and cycle time shown in
the GUI. The runner adds them to this file if Ricky reads them.

## Runner notes

- Pre-fix reference: 2.3 -> 19.4 GB in 30 s. After the fix, 0.5 and 0.2
  are both bounded and release their memory at the end.
- 0.2 -> 0.1 is 4x the cells; 9.17 GB x 4 would be about 37 GB, so the
  kill fits a simple cell-count scale. The steep step (10 -> 16+ GB in
  5 s) suggests one large allocation.
- Ricky's point: 0.5 mm is too coarse for the R1.0 scallop detail; 0.2 mm
  is his working resolution.

Logs on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job006_rss.log (5 s samples),
/tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job006_marks.log (Ricky's step times)
