---
name: refresh-lit-matrix
description: Walk through stale literature-matrix sources and re-verify or replace them. Use when the matrix's source-freshness report flags warn/stale rows, or roughly once a year.
allowed-tools: Bash, Read, Edit
---

# /refresh-lit-matrix — Source Freshness Refresh

The literature matrix (`crates/rs_cam_core/tests/literature_matrix/`) carries
a `last_verified` date on every entry in `sources.toml`. Sources age into
three buckets: fresh (<12 months), warn (12-18 months), stale (≥18 months).
Phase 5 of the plan added a CI report; this skill is the operator-side
counterpart for actually refreshing them.

Plan: `planning/feeds_literature_matrix_2026-06-03.md` §"Phase plan / Risk
register".
Source registry: `crates/rs_cam_core/tests/literature_matrix/sources.toml`.

## Steps

### 1. Generate today's freshness report

```bash
LIT_MATRIX_TODAY=$(date +%F) cargo test --test literature_matrix \
  -p rs_cam_core -- --nocapture 2>&1 \
  | sed -n '/=== Source freshness report ===/,/=== Source citation audit ===/p'
```

Read the warn / stale list. Fresh rows are suppressed by design.

### 2. For each warn / stale source, decide

For every flagged source key, open `sources.toml` and either:

- **Re-verify** — fetch `citation_url`, confirm the chart / table values
  the matrix relies on are unchanged. Bump `last_verified` to today's
  date. Add a one-line note if the source changed format but values are
  equivalent.
- **Replace** — if the source is gone, paywalled, or its values have
  drifted, find a replacement of equal or better `authority_tier`, add it
  as a new key, and update every `sources = [...]` array in `cells.toml`
  that referenced the old key. Delete the old entry only after the
  citation audit (run literature_matrix again) reports zero missing
  citations.

### 3. Re-run the matrix

```bash
LIT_MATRIX_TODAY=$(date +%F) cargo test --test literature_matrix \
  -p rs_cam_core -- --nocapture 2>&1 | tail -40
```

Verify both: matrix still passes AND the freshness report shows only
fresh rows (or acceptable warn rows you've intentionally deferred).

### 4. Commit

`docs(lit-matrix): refresh source freshness — N sources re-verified, M replaced`

## Knobs

| Env var | Effect |
|---------|--------|
| `LIT_MATRIX_TODAY=YYYY-MM-DD` | Override "today" for the freshness clock. |
| `LIT_MATRIX_DECAY_FAIL=1` | Hard-fail the matrix on any stale (≥18mo) source. |
