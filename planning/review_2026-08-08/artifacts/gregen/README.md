# G-REGEN-RACE artifacts — `generate_all` × `process_auto_regen`

TD3 tail wave, 2026-08-16. The defect: `generate_all` issued while the GUI's
own auto-regen sweep still had a toolpath in flight **resubmitted that same
toolpath**, the lane's resubmit-cancels-and-requeues rule cancelled the
in-flight job, and the resulting `ComputeError::Cancelled` was read as the
toolpath's *outcome* — so `generate_all` reported
`"<name>: generation cancelled"`, `generated: 0`, `rounds: 1` for work it
had itself replaced, while the replacement it queued succeeded unobserved.

B-4b's original `nomin/result.json` lived in session scratch and is gone, so
both arms below are the repro **rebuilt from the mechanism**, not recovered.

## The pair

| file | arm | reading |
|---|---|---|
| `live_red/result.json` | **live, PRE-FIX** debug `rs_cam_gui --mcp` | `generated: 0, failed: 1, rounds: 1`, `errors: [{"message": "Back Rough: generation cancelled", "toolpath_id": 4}]` |
| `live_green/result.json` | **live, POST-FIX**, same rig, same fixture | `generated: 1, failed: 0, rounds: 1, errors: []` |
| `red_unit.txt` | deterministic, supersede guard reverted | **2 failed / 269**; reply carries `"Scallop: generation cancelled"` |
| `green_unit.txt` | deterministic, fixed | **0 failed** (269 + 14 + 15 + 11) |
| `green_clippy.txt` | `cargo clippy --workspace --all-targets -D warnings` | exit 0 |

Both live arms are on a window that was **never minimised and never
touched**, on Wayland with `present mode NEGOTIATED Mailbox`. That is what
rules the compositor out — the same conclusion B-4b reached, re-established
rather than inherited.

## Reproducing

```sh
# fixture: a scratch COPY. The source is only ever READ.
python3 make_fixture.py --source planning/airrun_2026-06-01/wanaka.toml \
    --keep "Back Rough" --out /tmp/gregen/back_rough.toml

cargo build -p rs_cam_viz --bin rs_cam_gui          # debug; no release build
python3 regen_race_check.py --binary target/debug/rs_cam_gui \
    --project /tmp/gregen/back_rough.toml --log-dir ./live_green

# the red: revert the one guard, rebuild, run the same rig
python3 unfix.py && cargo build -p rs_cam_viz --bin rs_cam_gui
python3 regen_race_check.py ... --log-dir ./live_red
```

`unfix.py` reverts exactly one hunk — the supersede guard in
`drain_compute_results` — and leaves the plumbing
(`ToolpathSubmitOutcome`, the `superseded_toolpaths` ledger) in place, so
the red and the green are the same tree and the same tests with one
behaviour toggled.

## What the rig will NOT do

`regen_race_check.py` **proves the arm rather than assuming it**: it polls
`generation_status` and refuses to record a verdict unless the GUI's own
sweep already has the job on the lane when `generate_all` is issued. A run
that did not arm reports `armed: false` and says it measured nothing —
because a `generate_all` onto an idle lane cannot reproduce this and would
look like a pass.

Note the relationship to `../b4/shipped_flip_check.py`: that rig's
`wait_for_idle` **waits this race out**, and its docstring names the same
mechanism. This one fires into it on purpose. That wait is now optional.

Never point either rig at the operator's live GUI or at
`planning/airrun_2026-06-01/wanaka.toml`.
