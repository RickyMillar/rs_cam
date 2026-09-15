# Planning instructions

`planning/` contains active plans, current status and historical evidence.
It is not a single current specification.

## Start here

1. Read `PROGRESS.md` for the current shipped snapshot.
2. Find the active package directory/document before implementing.
3. Read its status/results file and any explicitly linked specification.
4. Check `FEATURE_CATALOG.md` before stating that something is shipped.

## Evidence hygiene

- Dated plans, review reports and incident logs are evidence of what was
  measured then, not evergreen implementation instructions.
- A document that says superseded/retired/closed must not be cited as current
  behaviour. In particular, consult
  `review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` before citing older
  finishing-strategy comparisons.
- Prefer current code plus a named sentry over prose when they disagree.
- Preserve useful measurements and decision records under `planning/` or
  `planning/archive/`; do not keep them in root or crate instruction files.

## Maintaining plans

- Active plan/status files should state scope, owner-facing decision, current
  evidence and next action concisely.
- Move completed execution narratives out of the active entry point rather
  than deleting their evidence.
- Do not delete apparent duplicates only by filename. Compare content and
  references first; archived and root copies may be intentionally different.
- When a visible product surface changes, update the applicable plan/status
  record and the product docs named in root `CLAUDE.md`.
