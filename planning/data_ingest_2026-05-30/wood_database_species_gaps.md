# Wood Database Species Sweep — Gaps (2026-05-30)

Species or slugs that failed to fetch cleanly from
[The Wood Database](https://www.wood-database.com/) during the Phase 3 / B
sweep on 2026-05-30. Logged here rather than fabricated.

## 404s — slug guesses that do not exist

| Attempted slug | HTTP | Resolution |
|----------------|------|-----------|
| `western-redcedar` | 404 | The correct slug is `western-red-cedar` (with the extra hyphen). Collected under that slug — see main file. |
| `meranti` | 404 | Wood-database.com has no unified `meranti` page; the meranti complex is split into colour-group pages (`light-red-meranti`, `dark-red-meranti`, `white-meranti`, `yellow-meranti`), all four of which were collected. |
| `red-meranti` | 404 | Not a slug. The two red-meranti groups are `light-red-meranti` and `dark-red-meranti`; both collected. |
| `western-red-cedar-aromatic` | 404 | Not a separate page. `aromatic-red-cedar` (Juniperus virginiana) is a different species; not pursued in this batch. |

## Species in the original brief that are NOT separately gapped

The brief mentioned "Hard Mahogany" and "Mahogany (Honduran)" as distinct
items. The Wood Database treats Honduran Mahogany (`honduran-mahogany`,
*Swietenia macrophylla*) as the canonical "true mahogany"; there is no
distinct "Hard Mahogany" species page. Honduran Mahogany was collected.

The brief mentioned "Cedar (Western Red)" — collected as `western-red-cedar`.

The brief mentioned "Beech (American)" — collected as `american-beech`.
European Beech (`european-beech`) was added as a bonus.

The brief mentioned "Yellow Pine (Loblolly specifically)" — collected as
`loblolly-pine` (Pinus taeda), 690 lbf.

## Species explicitly not pursued

None. The 34-species main file exceeds the ≥25 target. Additional species
from wood-database.com (e.g. Pau Ferro, Goncalo Alves, Ziricote, African
Blackwood, various Acacia species) would extend coverage further; they were
not attempted in this batch to keep the fetch within scope.

## Image-only / paywalled

No image-only or paywalled values were encountered for the 34 species
collected. Wood-database.com publishes Janka values as plain HTML text in the
species infobox; all 34 were extractable via direct text extraction.

## Ambiguous data

None of the 34 collected species had ambiguous Janka data. All resolved to a
single primary value with an explicit `lbf (N)` pair in the canonical
"Janka Hardness:" infobox row.
