# Track M — one metrology home for the finishing programme

**Status: AGENT RUNNING (2026-09-02).** Operator-commissioned: "is the
measurement used universally? … let's make sure that is tidy."

## The problem, measured

The rulers are copy-pasted per instrument: `relink_and_cost` exists in
SIX test files; the achieved-surface-spacing measurement in ~7 places;
the L_min floor integrand in 2–3; the Monge/curvature estimator and the
two strategy-gate censuses live in tests only. Consequence, already
paid: the Track G ledger could not put its arms on one scale
("only large cross-scale gaps are decided"), and G-UNIONCOV exists
because no whole-board coverage ruler exists at all.

## The work

One library home (`crates/rs_cam_core/src/metrology/`), promoting, ONE
implementation each, unit-pinned:

1. `relink_and_cost_under` — the costing harness, with its realistic
   link-ceiling regime; instruments become thin consumers. Document the
   relationship to the session integrator (the two scales), so every
   future table states its scale.
2. The L_min floor integrand (`∫ dA / s_max(x)`, curvature-aware) and
   the ×floor scoring column.
3. Achieved-surface-spacing (contact-ring spacing vs `s_max`), the
   quantity Track B's acceptance gate reads.
4. The Monge-quadric curvature estimator + the two strategy censuses
   (anisotropy prize ceiling; direction-coherence w30 / coherence
   length) with their gate thresholds as named, documented constants.
5. **The union-coverage audit — the NEW capability (G-UNIONCOV's fix):**
   final simulated stock vs target surface + stock_to_leave, whole
   board, per project; reports standing-above-spec area with locations;
   loud threshold failure mode for comparison runs. First consumer: an
   instrument that re-judges the ledger arms at equal delivered finish.

## Rules

- Pure promotion first: shipped behavior byte-identical; an instrument
  re-pins only when extraction reveals a genuine divergence, disclosed.
- Every promoted function carries its unit tests into the library.
- One doc section states the measurement contract: `None` = not
  measured vs `0.0` = clean; the two air-cut denominators; the two time
  scales; resolution-conditional quantities.
- Full gates before commit.

## Evidence lands here

`planning/metrology_2026-09-02/FINDINGS.md`.
