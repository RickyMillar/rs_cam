# comms/runner: job mailbox between the cloud lead and the local runner

This branch holds messages only. It has no code. Never merge it.

## Parties

| Role | Session | Writes |
|---|---|---|
| Lead (cloud) | session_01RkXBP32t4BA3gBwXffZ4gd | `jobs/` |
| Runner (Ricky's PC) | session_01X9bPqEnCfUkrbNprBjUs3d | `results/` |
| Ricky (operator) | — | any file; an edit by Ricky overrides both agents |

Each party writes only its own folder. Then two pushes never touch the
same file, and a rebase never has a conflict.

## Files

- `jobs/NNN-short-name.md`: one job. The lead writes it once. To change a
  job, the lead writes a new job that names the old number.
- `results/NNN-short-name.md`: the result for job `NNN`. The runner writes
  it. The runner can write it more than once (for example `state: running`,
  then `state: done`).
- `NNN` is a three-digit number. The next job takes the highest number
  plus one.

## Job file format

```
---
id: 001
state: queued            # queued | cancelled
commit: <sha>            # must be pushed to origin
needs_ricky_ok: true     # true for anything over about 3 minutes
---
## Commands
## Report back
## Notes
```

## Result file format

```
---
id: 001
state: waiting-ok        # waiting-ok | running | done | failed | refused
commit: <sha actually tested>
started: <UTC>
finished: <UTC>
---
## Summary
## Detail
```

## Transport

1. The lead commits a job file and pushes to `origin/comms/runner`.
2. The runner polls the branch (`git fetch`, about every 60 s). A new or
   changed file under `jobs/` wakes the runner.
3. The runner writes the result file, commits and pushes it.
4. The runner also sends the lead a short `send_message`:
   `COMMS: results/NNN updated (state)`. This wakes the lead.
5. The lead reads the result with `git fetch origin comms/runner` and
   `git show origin/comms/runner:results/NNN-*.md`.

Fallback: if a push fails, the sender puts the full message in its chat
reply and starts it with `COMMS NNN:`. The other side searches the
transcript for that marker.

## Push rule

Before a push, run `git pull --rebase origin comms/runner`. Retry a
failed push up to 4 times (2 s, 4 s, 8 s, 16 s).

## Runner rules

- The runner runs cargo through `scripts/cargo_lane.sh`, one heavy job at
  a time.
- The runner asks Ricky before a job with `needs_ricky_ok: true`.
- The runner reports only. It does not fix code, and it does not commit
  outside this branch.
