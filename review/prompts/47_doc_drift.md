# Review: Documentation Drift

## Scope
Compare documentation claims against actual code to find mismatches.

## Files to compare
- `FEATURE_CATALOG.md` vs actual features in code
- `planning/PROGRESS.md` vs current code state
- `README.md` vs actual project state
- `architecture/high_level_design.md` vs actual architecture
- `architecture/requirements.md` vs implemented requirements
- `architecture/user_stories.md` vs implemented stories
- `CREDITS.md` vs actual algorithm sources used

## What to review

### FEATURE_CATALOG.md
- For each claimed feature: is it actually implemented?
- For each claimed limitation: is it still a limitation?
- Are there features in code not listed in the catalog?

### PROGRESS.md
- Does the "current state" section match reality?
- Are verification gates still passing?
- Is the "recent work" actually recent?

### Architecture docs
- Do diagrams/descriptions match the actual module structure?
- Are there modules not mentioned?
- Are there mentioned modules that don't exist?

### CREDITS.md
- Are all algorithm sources still used?
- Are there new algorithms not attributed?

### Method
- For each doc, list: Claim | Code Evidence | Match/Drift

## Output
Write findings to `review/results/47_doc_drift.md` with a drift table per document.
