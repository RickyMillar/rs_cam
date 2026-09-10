# Night 3 — one account, the merge, eight fixes, and a recovered plan

Written 2026-09-10 by the session that took over both tracks. Shape follows
`NIGHT_1.md`.

## Outcome

| | |
|---|---|
| Branch | `ui-fix-2026-09-09`, **103 commits ahead of master** |
| `master` | **UNTOUCHED at `3d88406b`.** No pull request was opened. |
| Full heavy gate | fmt 0; heavy clippy 0; core **299 binaries / 3773 passed**; viz 0; mcp 0; cli 0 |
| The one red | `adaptive_feed_modulation_pipeline_f036b`, red BY DESIGN, open for the operator |
| Fixes landed | 8, every one red-first |
| Follow-ons opened | 12, plus 6 more from the verification pass |
| Working tree | clean apart from `.mcp.json`, which was never staged |

## Tasks done

| Task | Commit | What |
|---|---|---|
| Merge | `9292287a` | The core track's 15 commits into the UI branch. One conflict, one bullet, both sides kept. |
| F4.4 | `957f051f` | A model refresh carries the file's drill targets. |
| F2.12 | `8cb8aa1d` | The holder verdict withdraws a stale clearance claim. |
| J7 + J8 GUI | `ae39e4f4` | One depth rule; a Bottom Z row that says what it does. |
| F1.24 | `cc3641cf` | A save's temp file is unique per call. |
| F2.13 | (lane f213) | The holder row says how much of the job it measured. |
| F4.8 + F4.7 | `cc85abce` | A stale drill pick refuses; a rescale drops what it invalidated. |
| N3 / G-STEPUNITS | `069a2314` | The project loader applies a STEP model's declared units. |

## The gate results, verbatim

Merge gate on `9292287a`: fmt 0, clippy 0, core 285/3717 with one target
failed, viz 30/566 with one target failed, mcp 0.

V6.1 on `c43f298b`: fmt 0, clippy 0, **core heavy 297 ok-binaries, 3764
passed, 288 ignored, 1 target failed**, viz 0, mcp 0, cli 0.

Final on `cc85abce`: fmt 0, clippy 0, **core heavy 299 ok-binaries, 3773
passed**, viz 0, mcp 0, cli 0.

N3 gate on `069a2314`: fmt 0, clippy 0, core **with `--features step`** 287
ok-binaries / 3736 passed, viz 0, cli 0.

## Every deviation from the prompt, with its reason

**The FULL heavy gate had not run since `478f0678`**, before the merge. No
gate had ever covered a tree carrying both programmes. V6.1 paid it.

**The viz and mcp suites were added to the merge gate**, which the prompt did
not ask for. They earned it: they found the duplicate Safety caution that a
core-only gate cannot see.

**Two lanes produced a `cargo fmt` diff.** Repaired with
`git rebase --exec 'cargo fmt'` across every commit, proven content-identical
by comparing the files with all whitespace stripped, and the FULL gate re-run
on the new hashes rather than arguing the old result still applied.

**F4.4's `CLAUDE.md` addition is four sentences** where the brief permitted
"a sentence". Kept: each carries a distinct fact, and it extends an existing
caveat paragraph rather than adding a bullet.

**J7's red is a COMPILE error, not a failing assertion.** Recorded as weak in
the report, the commit and the ledger. An annotation has no cut geometry to be
red about; the behavioural evidence is the core measurement.

**Three tests are not red-first and say so.** Two in F2.12 and one in F2.13
exercise API their sentry commit predates. A test cannot be red against an API
that does not exist. Disclosed rather than manufactured.

**The sentry commits still read `RED-FIRST OUTPUT: pending verifier run`.**
Deliberately not rewritten: honest when written, hashes already cited, and J5
set the precedent of not rewriting commit bodies on a shared branch.

## Findings the work itself surfaced

**Every fix this session was a duplicate or a missing share.** Two depth rules
into one predicate. Three hand-written field copies into one `adopt_geometry`.
Two detail-string builders into one derivation. A holder verdict with no
staleness stamp while every sibling row had one. The false statements were
downstream of the duplication, which is exactly the thesis of the architecture
audit recovered later the same day.

**Five findings were bigger or differently shaped than filed.** F4.4 named one
door and was three, the worst unnamed. F2.13 was sent for a label and found a
job-wide `Pass` reported from a DISABLED operation. F2.12 found `Pass` was
unreachable. F4.8 found the pin drill had the identical defect. F1.24's
mechanism was worse than filed — the winner publishes the loser's bytes and
returns `Ok(())` — and its exposure narrower.

**The orchestrator was wrong once, in the ledger, and was corrected by a
worker.** The merge row recorded the duplicate caution's evidence attribution
backwards, and concluded the core rule was "strictly better". Acting on that
would have deleted the operator's ribbon line at switchover. An append-only
correction row records it. **The live MCP check later confirmed the evidence
field is present** — so the correction is what saved it.

## The recovered plan

The operator asked where the tech-debt plan was. It was not in the repo. The
pi command `/techdebt-orchestrate` pointed at
`planning/architectural_refactor_2026-06-06_v2.md`, which is **complete** —
every work item T0..T16 landed 2026-06-07, verified by content — and is a CORE
refactor, not a GUI one.

The real audit and its nine-phase plan were produced by gpt-5.6-astra on
2026-09-09 and existed only inside a pi session transcript. Both are now at
`planning/arch_consolidation_2026-09-09/`, verbatim, with provenance headers.

**All twelve findings were then verified against a tree 98 commits younger.**
Four lanes, every claim with a file and line. Four findings partly closed, five
wider or differently shaped, one with the wrong verb (drilling derives from the
config, not from emitted motion — which re-rates that phase from High to
Small), and **six defects the audit does not contain**.

The first of those six, N3, was fixed this session. It was a live wrong-size
defect: `rs_cam_cli run --units inches part.step` cut at 25.4× the wrong scale.

## Open questions for the operator

1. **`f036b`** — re-point another programme's instrument? The measured fix is
   in `reports/J2.md`. Untouched.
2. **`master`** — 103 commits sit unmerged. The other account now works on
   this branch because the plan does not exist on master.
3. **N1** — CLI export may emit a DISABLED operation's toolpath. **A READ, NOT
   A REPRODUCTION.** Wrong-cut class if real.
4. **N2** — `CLAUDE.md`'s G-DRILLTIME entry is now HALF-TRUE: fixed on the
   integrator path, live on the retime path. That file is what every agent
   reads first.
5. **F2.14** — the holder row can no longer read `Pass` on a multi-operation
   job. Widening the check needs it made cheaper first: **measured at over 120
   seconds for ONE toolpath** on a small 2D job.
6. **The three stale branch pointers** (`isoclip-rapid`, `ui-string-sentry`,
   `pipesmoke`) — content is in master, safe to delete. Not this session's call.
7. **R0.1 §7 Q5/Q6 and R0.3 §7 Q1/Q2/Q5** — still answered by assumption.

## Still owed

**V6.2 is PARTLY paid.** J7 and J8 were confirmed live on the GUI, with
screenshots in `evidence/`. **F1.13, F1.15 and F2.2 stay OWED**, as does the
full R03 acceptance sequence.

**V6.3**, the sentry census, was not run as a task — but the verification pass
inventoried the net against the architecture plan's Phase 0 and found ten
source-level string-pin sentries, seven of which do not admit their own
weakness, and one guard that **compares MCP against a replica of MCP**.

## To resume

```
git -C /home/ricky/personal_repos/rs_cam checkout ui-fix-2026-09-09
cat planning/arch_consolidation_2026-09-09/STATUS.md      # verified tracker
cat planning/ui_fix_2026-09-09/STATUS.md                  # the append-only ledger
```

The release binary at `target/release/rs_cam_gui` is current for this head.
`.mcp.json` runs it DIRECTLY, not through `cargo run`, so a stale binary
starts happily and serves old code. **Rebuild after any code change before
starting the MCP server.**
