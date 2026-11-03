# Phase card template

One card per phase: `phases/P<n>.md`. A card is the full hand-off for one
session. A fresh session reads `PLAN.md` section 0, then the card, and can
start work without the research maps. Write in ASD-STE100 (root user rules).

Cite code as `path:line@40cf411a7` **plus the symbol name**. Line numbers
drift after each phase; the symbol is the stable pointer.

```markdown
# P<n>: <title>

Status: NOT STARTED | Contracts: K.. | Rulings: Q.. | Numbers move: no/yes (which)

## Goal
Two to four sentences: what is true after this phase that is not true now.

## Preconditions
- Phases that must be committed first.
- Rulings or open questions that must be answered.
- Files owned by another session (read them with `git show HEAD:<path>`).

## Changes
One numbered item per change. Each item:
- What: one sentence.
- Where: `path:line@40cf411a7` `symbol`.
- How: the type, signature or rule (a short Rust sketch where it helps).
- Callers: the `rg` command that measures the blast radius, and its hit count at HEAD.

## Out of scope
What a session must not do in this phase, and the phase that owns it.

## Tests
- New sentry files: name (house style: a sentence in snake case with the
  tag suffix, for example `..._k6`), what it asserts, red before, green after.
- `compile_fail` doctests for each refusal the compiler holds.
- Goldens that must stay byte-identical, or that re-bless (name the env var).

## Gate
The exact commands, each through `scripts/cargo_lane.sh`. Mark any run over
three minutes as "ask the operator first".

## Docs to update
Folder `CLAUDE.md` files, `crates/rs_cam_core/CONTRACTS.md`, FEATURE_CATALOG,
PROGRESS.

## Traps
What went wrong before or what is easy to miss here.

## Agent brief
A paste-ready prompt for one implementing agent (agents never commit; the
lead reviews by path).

## Done when
A checklist the lead ticks before the commit.

## Citation check
Each research-map citation this card uses that was wrong at HEAD, and the
correct one.
```
