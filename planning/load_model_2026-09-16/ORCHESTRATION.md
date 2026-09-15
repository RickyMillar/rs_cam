# Running parallel agents in this repo

Written 2026-09-16, after wave 1 of the cut-efficiency programme lost about
fifteen minutes to a recoverable accident and rather more to three avoidable
ones. Each item below cost real time in that session. Phases C and R1 are
still ahead; read this before spawning agents for them.

---

## 1. Never run a tree-wide git operation

`git stash`, `git reset`, `git checkout` without a path, `git clean` — all of
them act on the whole working tree, and in a shared tree an agent owns only
its own files.

One agent ran `git stash push --include-untracked -- crates/` to get a HEAD
baseline. It captured fourteen files belonging to three agents. Nothing was
lost, because the agent that noticed restored only its own files and
deliberately did **not** pop the stash, leaving it as a read-only backup.

**The near miss is the part worth remembering.** A second agent had, at
almost the same moment, a path-scoped `git checkout HEAD -- <its two files>`
running for about three minutes. Had the stash landed inside that window it
would have captured those two files at HEAD content — and the recovery would
then have handed that agent back HEAD instead of its own work, **silently,
with every gate still green, because HEAD compiles perfectly well.** The only
reason we know it did not happen is that the agent had made byte-comparable
copies of its files beforehand and checked them afterwards.

Two path-scoped operations were harmless on their own. The tree-wide one
turned a near miss into a hazard.

**Instead:** `git show HEAD:<path>` and `git diff -- <path>` read without
writing. For a real baseline build, use `git worktree`, which is isolated by
design.

**And:** have agents keep a copy of their own files after `cargo fmt`. It is
free, and here it was the only evidence that a restore had been correct.

## 2. A recovered file is not a verified file

When work is restored from a stash or a backup, the agent that wrote it must
confirm the restored content is its **finished** state, not a mid-flight
snapshot — and any gate run that overlapped the accident is void, because it
may have measured the wiped tree. A green result against an empty tree proves
nothing.

## 3. Concurrent cargo in one target directory produces spurious failures

Cargo takes a lock on `target/`, so concurrent invocations serialise rather
than exhaust memory. That is true and it is not the whole story.

`cargo test --doc` links against a specific rlib hash. A concurrent build can
replace that rlib mid-run, and the doctests fail with:

```
error: extern location for rs_cam_core does not exist:
    target/debug/deps/librs_cam_core-<hash>.rlib
```

That is not a code failure. Re-run on a quiet cache before investigating.

**Instead:** brief each agent to run only its own named test targets
(`--test <name>`), and run the full gate once, centrally, at the end. In wave
1 two agents were each told to run the whole core gate; they queued
twenty-minute runs of the same crate and produced this race on top.

## 4. `pgrep -f "cargo ..."` matches its own shell

A waiter like

```sh
until ! pgrep -f "cargo build --release" >/dev/null; do sleep 5; done
```

never exits: the `zsh -c` wrapper running the loop has the pattern in its own
command line, so `pgrep` matches itself forever.

**Instead:** match the binary path rather than the invocation —
`pgrep -f "bin/cargo test"` — or `pgrep -x cargo`, or check for the
toolchain path. Confirm by printing what matched before trusting a waiter.

## 5. Check the claim that is expensive to get wrong, not the whole report

Agents report accurately here, but a report is only as good as the tree it
was measured against. Pick the one claim whose failure would be costly and
verify it directly:

- "the refactor is behaviour-neutral" → extract the acceptance test's body
  from `git show HEAD:<path>` and from the tree and compare bytes. It was
  identical at 1923 bytes, which no amount of prose would have established.
- "no assertion was weakened" → read the assertion-bearing diff lines. In
  that case one arm had been *strengthened*, which the report undersold.
- "the code reads correctly" → not a verification. Ask for an exit code.

## 6. Agents correct briefs, and should

Two briefs in wave 1 contained a false premise.

One claimed the peak-chip immersion form `cos ψ = 1 − 2·woc` answers a
different question from `immersion_angle`'s `cos ψ = 1 − ae/r`. It does not:
with `r = D/2`, `ae/r = 2·(ae/D)`. The agent refused the framing and
documented the truth, aiming the warning at the real hazard — a reader
"correcting" the line to `1 − woc_fraction` would halve the angle and
silently raise the deflection cap. Had it written the comment as briefed, a
careful reader would eventually have deleted it as false and taken the real
warning with it.

The other underspecified a signature. The agent split
`chipload_cap_for_deflection_with_reason` from the `Option` wrapper, because
one caller needs to know which refusal fired and the other does not.

**Write briefs that can be argued with**, and state acceptance bars that are
falsifiable — "this existing test must pass unmodified; if it needs editing,
the change is wrong, stop and report" produced better results than any
instruction about how to write the code.
