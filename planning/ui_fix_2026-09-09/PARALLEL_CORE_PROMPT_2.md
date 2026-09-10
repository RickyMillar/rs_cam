# Parallel prompt 2 — core geometry account

Written 2026-09-10 by the UI/UX programme orchestrator, after the first
four jobs (J0–J3) landed on `core-consolidation`. Paste everything below
the line into the same session, or a fresh one started in
`/home/ricky/personal_repos/rs_cam`.

---

Your first four jobs are done and the work is good. J0 consolidated four
branches, J1 correctly separated a real regression from a stale pin, J2
landed G-RAMPCONTAIN, and J3's per-consumer audit for G-BULLCUSP — sorted
into "correct", "changes the cost of a run", and "invalidates a cache key
once" — is the standard the rest of this should hold to.

Five things follow. J4 first; it is small and urgent.

## Your tree

Same as before: `/home/ricky/personal_repos/rs_cam_wt_alt`, branch
`core-consolidation`, with your OWN cache:

```
export CARGO_TARGET_DIR=/home/ricky/personal_repos/rs_cam_target_alt
```

Never use `/tmp/rs_cam_gate.sh`. That is the other session's lock over a
different cache, and three of its lanes are contending for it.

**Disk was 70 GiB free when this was written**, with the UI programme's
cache at 45 GiB and yours at 16 GiB. Check `df -h /home/ricky` before any
long build and stop below 30 GiB.

## Files you must not touch — this list has GROWN

The UI programme now has three active lanes and has merged five tasks. Stay
out of:

- `crates/rs_cam_viz/**` — all of it, now. Every lane is somewhere in it.
- `crates/rs_cam_mcp/**`
- `crates/rs_cam_core/src/session/**`
- `planning/ui_fix_2026-09-09/**` except the report files named below

Your remaining work is core geometry, core generators and the tool model.
If a job below cannot be done without a forbidden file, do the part that
can and say precisely what you left, naming the file.

## J4 — rescue your own reports (do this first)

`J0.md`, `J1.md`, `J2.md` and `J3.md` — 52 KB of work — were written into
the MAIN checkout at `/home/ricky/personal_repos/rs_cam/planning/ui_fix_2026-09-09/reports/`,
not into your worktree. They are UNTRACKED, they sit on another branch's
working tree, and they are committed nowhere. One `git clean` destroys
them. The orchestrator has copied them to a scratchpad as a hedge but has
deliberately not moved or committed them, because they are yours.

Move them into your own worktree and commit them onto `core-consolidation`.
Then check where your tooling is writing: if your working directory was the
main checkout for some operations, fix that before J5, or you will do this
again.

## J5 — pay the gate debt, or state the constraint precisely

Every one of your four commits carries the same line: the full heavy-tests
gate and the heavy-feature workspace clippy were NOT run, "on a standing
operator instruction that it must not run on this machine", and both are
OWED on this branch.

The orchestrator does not have that instruction, and it is not in
`CLAUDE.md`, which says the opposite — the FULL gate is required once per
phase or commit-gate run, and clippy must be run WITH the feature or the
twelve heavy test binaries never get linted. That matters here more than
usual: J3 changed `cusp_radius`, a core geometry primitive with fourteen
consumers, on the strength of a non-heavy clippy.

So, in order:

1. Say plainly where that instruction came from — the operator directly,
   an inference from disk pressure, or something you read. Quote it if you
   can. If it was an inference, say so; that is not a criticism, it is the
   thing that needs to be known.
2. If it is a real operator instruction, do NOT run the gate. Write the
   debt into `planning/ui_fix_2026-09-09/STATUS.md` as an explicit owed
   item naming the branch, the two commands and why they matter, and stop
   there.
3. If it was an inference, check `df` and run both:
   `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`
   and
   `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`.
   Report the result verbatim, including anything red.

Do not run the gate on the assumption it is fine. Ask the operator through
your own session if you are unsure — this is exactly the kind of thing that
should be confirmed rather than guessed.

## J6 — F1.16: the schema advertises a value the generator cannot produce

From `planning/ui_fix_2026-09-09/PLAN.md` §11, opened by the F1.7 worker:

> `ProfileSide` has only `Outside` / `Inside` and the generator has no
> on-the-line arm, yet the operation schema advertises `on`, so
> `set_toolpath_param side=on` fails at serde. Either implement the arm or
> drop the schema value. **A combo entry that generates the same as Outside
> would be a NEW untruth — do not add one.**

Decide which, implement it, and say why in the report. If you implement the
arm, an on-the-line profile centres the cutter on the contour, which is a
real and different toolpath — prove it differs from both Outside and
Inside by more than a sign. If you drop the schema value, check nothing
else advertises it and that no project file in the repo stores it.

This is squarely the programme's theme: a surface saying something that is
not true.

## J7 — F1.19: a caution that fires on a number the machine never cuts

From PLAN.md §11, opened by the F1.6 worker:

> 2.5D generators IGNORE a pinned Bottom Z, so a Heights-tab pin below the
> stock cautions on a number that is not emitted motion. Pre-existing;
> decide whether the pin should drive the cut or the field should be
> disabled for 2.5D.

Establish the fact first: confirm from the generators that a pinned Bottom
Z really is ignored, and say which operations. Then make the
recommendation. Both answers are defensible and the choice is the
operator's, so if the right answer is "the pin should drive the cut" and
that is a generator change with real blast radius, write the recommendation
and the evidence and STOP rather than implementing it unasked. If the
answer is that the field should not be offered for 2.5D, that is small and
you may implement it.

The GUI half of either answer is the UI programme's; do not edit
`crates/rs_cam_viz/**`.

## J8 — F1.18, core half only

From PLAN.md §11, opened by the F1.6 worker: the depth-beyond-stock caution
is GUI-side, so it reaches neither the Operations card row nor MCP
`get_toolpath_diagnostics`. Moving the rule into core `heights_checks`
would cover both surfaces with one predicate.

Do the CORE half: add the predicate to core so it is available to the
diagnostics path, with its own sentry. **Leave the GUI switchover alone** —
deleting the GUI-side rule touches `ui/properties/**`, where a UI lane is
working right now. Say in your report that the core predicate exists and is
unconsumed by the GUI, so the UI programme can pick it up.

Note the caveat the F1.6 worker recorded and do not lose it: the caution is
OR'd from the operation depth and a pinned Bottom Z, and per J7 the pin may
not be emitted motion at all. If J7 concludes the pin is ignored, say what
that means for this predicate rather than porting the ambiguity into core.

## Gates — required before every commit

With your own `CARGO_TARGET_DIR`:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`
3. `cargo test -p rs_cam_core -q`
4. Your sentry.

**Never run a workspace-wide `cargo test`.**

## Rules, unchanged

- Every behaviour change ships a sentry named after the finding, SHOWN
  FAILING on the pre-fix code, with the output quoted verbatim in the
  report and the commit body.
- Write the code, the sentry and the report FIRST, then gate, then commit.
- Stage BY EXPLICIT PATH. Never `git add -A`.
- **Never commit to `master`. Never open a pull request.**
- Forbidden: numeric thresholds, gate bars, feeds constants, LUT rows;
  `CLAUDE.md` rules and permissions (adding a sentence to an existing
  caveat paragraph is allowed and expected when an operator-visible rule
  changes); `.mcp.json`; agent or auth config; `Cargo.toml` lint denies;
  launching or restarting `rs_cam_gui`; killing a process you did not
  start.
- The MCP server is DOWN. No live checks. Do not start a GUI.
- Evidence language: generated ≠ simulated ≠ Within ≠ safe.
- Append one line per job to `planning/ui_fix_2026-09-09/STATUS.md`. It is
  append-only; never rewrite a line.

## The thing that has to happen eventually, and is not yours to decide

`core-consolidation` now holds seven commits the UI programme does not
have, including two fixes it would benefit from immediately: both red core
test binaries are green on your branch and still red on the UI programme's.
Merging your branch to master, or rebasing the UI branch onto it, is the
OPERATOR's decision. Do not do either. When J5 is settled, say in your
report that the branch is gated and ready, and let the operator choose.

Work J4 → J5 → J6 → J7 → J8. Stop after any job and report if you run low
on context or credit.
