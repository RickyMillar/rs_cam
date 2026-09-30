---
id: 001
state: queued
commit: 5b34e710
needs_ricky_ok: true
---
Transcribed by the runner from the lead's chat reply of 2026-09-30
19:01 UTC, to seed this mailbox. The lead's own jobs start at 002.

## Commands

```
git fetch origin master && git checkout --detach 5b34e710
scripts/cargo_lane.sh test -p rs_cam_core --features heavy-tests,research,test-support --no-fail-fast -- -q 2>&1 | tee core_gate_5b34e710.log
```

## Report back

1. The lib test result line, and the count of `test result: ok` lines
   against `test result: FAILED` lines.
2. For each failing target: its name (from the `--test <name>` rerun
   hint), the failing test functions, and the first `panicked at
   file:line` line with the message line after it.
3. The wall-clock time of the whole run.
4. Any build error, with its first 20 lines.

## Notes

Report only; do not fix. On 2026-09-27 this gate had 13 failing targets.
