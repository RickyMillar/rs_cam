# P12 hand-off: index

The feeds contract hand-off for an independent review session. Hand-off commit `f0150aeb5`.
The phase card is `planning/feeds_contract_2026-11-03/phases/P12.md`.

## Read in this order

1. `planning/feeds_contract_2026-11-03/handoff/VERIFICATION_BRIEF.md`: the brief. Paste it into
   the review session.
2. The parameter guide: https://claude.ai/artifact/78xXh5LQ2oPoPsErTZZnQy (source
   `planning/feeds_contract_2026-11-03/explainer_p13c/`).
3. `planning/feeds_contract_2026-11-03/handoff/MODELS.md`: each model, its formula, inputs,
   source, constants (Cited or Unsourced) and limits.
4. `planning/feeds_contract_2026-11-03/handoff/WHAT_IS_DONE.md`: the phases P0-P14, the rulings,
   the contracts K1-K8, and section 4, the reviewer items and the accepted reds.

## Where each artefact is

| Artefact | Path |
|---|---|
| Plan, rulings Q1-Q10, contracts | `planning/feeds_contract_2026-11-03/PLAN.md` |
| Status, rulings D1-D13, O1-O5, P14 lead rulings | `planning/feeds_contract_2026-11-03/STATUS.md` |
| Phase cards and results | `planning/feeds_contract_2026-11-03/phases/` |
| Contract index (one line per contract) | `crates/rs_cam_core/CONTRACTS.md` |
| Feeds folder map and sentries | `crates/rs_cam_core/src/feeds/CLAUDE.md` |
| F201 constant registry note | `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md` |
| FM1 golden (1000 cells, the engine's matrix) | `crates/rs_cam_core/tests/fixtures/feeds_matrix_2026-09-23/` |
| G-OPGOLDEN known rows | `planning/feeds_contract_2026-11-03/opgolden_known_46_rows.txt` |
| Vendor rows and stored chart text | `crates/rs_cam_core/data/vendor_lut/` |
| Research: stepover load | `planning/feeds_contract_2026-11-03/research_ae/` |
| Research: force model validation | `planning/feeds_contract_2026-11-03/research_p11_step0/` |
| Research: derate audit | `planning/feeds_contract_2026-11-03/research_derate/` |
| Research: inputs per output, sweep driver | `planning/feeds_contract_2026-11-03/research_p13_step0/` |
| Research: vendor-row chip law | `planning/feeds_contract_2026-11-03/research_p13_items45/` |
| The lead's runs of the brief | `planning/feeds_contract_2026-11-03/handoff/runs/` |
| Reference checker | `planning/feeds_contract_2026-11-03/handoff/check_refs.py` |

## The reference check

`python3 -I planning/feeds_contract_2026-11-03/handoff/check_refs.py` resolves every `path:line`
and every commit hash in the four files. It exits 0 when all resolve.
