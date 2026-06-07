# rs_cam_viz Capability Matrix — GUI IA Audit

## How to read this

- **Capability** = a single user-facing action or read-out, collapsed across every surface that exposes it (records merged by canonical `id`; kebab-spelling variants merged — see *Merges* below).
- **Role abbreviations** (per surface): `A` = authoritative-edit (the real editor), `M` = mirror (a duplicate trigger for an action owned elsewhere), `S` = suggestion (recommends/applies a computed value), `R` = read-only-display.
- **DUPLICATED** = a capability reachable from **3 or more surfaces** → P1 consolidation candidates (the more `M`/`S` mirrors, the higher the IA cost).
- **CONFUSABLE PAIRS** = two surfaces whose capability sets nearly coincide but whose roles differ, or that otherwise look alike while doing different jobs → P4 candidates (rename/merge/disambiguate).
- Full per-surface grid is in `CAPABILITY_MATRIX.csv` (one column per surface).

**Totals:** 368 unique capabilities across 64 surfaces; 21 duplicated (3+ surfaces); 17 confusable surface pairs.

## Merges applied (canonical id ← merged spellings)

- `generate-all` — merged generate-all-toolpaths
- `set-depth-per-pass` — merged set-doc (both write operation.depth_per_pass)
- `apply-all-feeds` — merged request-feeds-suggestion-all
- `select-keepout` — merged select-keep-out
- `delete-keepout` — merged remove-keep-out
- `toggle-toolpath-visibility` — merged toolpath-visibility (sim)
- `delete-toolpath` — merged remove-toolpath
- `delete-model` — merged remove-model

## DUPLICATED capabilities (3+ surfaces) — P1

| capability_id | name | #surfaces | surfaces (role) |
|---|---|---|---|
| `set-feed-rate` | Feed rate | 9 | Params Tab (A), Feeds Tab (M), Feeds & Speeds Modal (launcher) (M), toolpath-tab-feed-params (A), feeds-card (S), feeds-modal-toolpath (S), feeds-modal-nomogram-explore (S), optimize-modal (S), export-wizard (R) |
| `set-spindle-rpm` | Spindle RPM override | 8 | Params Tab (A), Feeds Tab (M), toolpath-tab-feed-params (A), feeds-modal-toolpath (S), feeds-modal-nomogram-explore (S), optimize-modal (S), post-processor-panel (A), export-wizard (R) |
| `set-stepover` | Stepover (radial WOC) | 6 | Params Tab (A), Feeds Tab (M), toolpath-tab-feed-params (A), feeds-card (S), feeds-modal-toolpath (S), optimize-modal (S) |
| `set-depth-per-pass` | Depth per pass (axial DOC) | 6 | Params Tab (A), Feeds Tab (M), toolpath-tab-feed-params (A), feeds-card (S), feeds-modal-toolpath (S), optimize-modal (S) |
| `apply-all-feeds` | Suggest all feeds (LUT) | 6 | Params Tab (S), Feeds Tab (S), feeds-card (S), toolpath-tab-suggest-all (S), feeds-modal-toolpath (S), feeds-modal-project (S) |
| `set-plunge-rate` | Plunge rate | 5 | Params Tab (A), Feeds Tab (M), toolpath-tab-feed-params (A), feeds-card (S), feeds-modal-toolpath (S) |
| `toggle-toolpath-visibility` | Toggle toolpath visibility | 4 | Operations Queue Panel (A), Toolpath Card Context Menu (M), Operation list (A), project-tree (A) |
| `request-feeds-suggestion` | Compute/display LUT feeds recommendation | 4 | feeds-modal-toolpath (R), feeds-card (R), toolpath-tab-feed-params (R), feeds-modal-project (R) |
| `open-optimize-modal` | Open per-toolpath sim optimizer | 4 | sim-diagnostics-optimize-entry (R), Inspector › Focused hotspot card (S), Inspector › Focused issue card (S), Inspector › Now-playing strip (S) |
| `run-simulation` | Run simulation | 4 | Setup & run (A), Verification staleness card (M), Viewport overlay toolbar (M), menu-bar (A) |
| `generate-all` | Generate all toolpaths | 3 | Operations Queue Panel (A), Viewport overlay toolbar (A), menu-bar (A) |
| `generate-toolpath` | Generate toolpath | 3 | Operations Queue Panel (A), Toolpath Card Context Menu (M), Toolpath Properties Header (M) |
| `inspect-toolpath-in-simulation` | Inspect in simulation | 3 | Operations Queue Panel (A), Toolpath Card Context Menu (M), project-tree (A) |
| `toggle-isolate-toolpath` | Isolate toolpath in viewport | 3 | Operations Queue Panel (A), Toolpath Card Context Menu (M), Viewport overlay toolbar (A) |
| `view-vendor-lut` | View vendor cutting data | 3 | Vendor LUT Viewer (R), vendor-lut-viewer (R), feeds-tab (R) |
| `set-post-format` | Set post-processor format | 3 | post-processor-panel (A), export-wizard (A), project-tree (M) |
| `read-staleness` | Results-stale indicator | 3 | Verification staleness card (R), Inspector › Project overview (M), Export Readiness modal (M) |
| `focus-hotspot` | Focus a hotspot | 3 | Inspector › Top hotspots list (A), Inspector › Selected span section (M), Signal spine (M) |
| `read-collision-count` | Collision count | 3 | Inspector › Project overview (R), Verdict HUD (M), Export Readiness modal (M) |
| `read-load-findings` | TPs within/exceeding/unmodeled | 3 | Inspector › Project overview (R), Verdict HUD (M), Export Readiness modal (M) |
| `export-setup-gcode` | Export per-setup g-code | 3 | menu-file-direct-export (A), menu-bar (M), project-tree (M) |

## CONFUSABLE surface pairs — P4

| surfaceA \| surfaceB | jaccard | shared caps | role-diffs | why |
|---|---|---|---|---|
| toolpath-tab-feed-params|feeds-card | 0.71 | 5 | 4 | near-identical-capset + role-mismatch |
| toolpath-tab-feed-params|feeds-modal-toolpath | 0.6 | 6 | 5 | near-identical-capset + role-mismatch |
| feeds-card|feeds-modal-toolpath | 0.6 | 6 | 0 | near-identical-capset |
| toolpath-tab-feed-params|optimize-modal | 0.5 | 4 | 4 | role-mismatch-on-shared |
| Feeds Tab|feeds-modal-toolpath | 0.38 | 6 | 5 | role-mismatch-on-shared |
| Feeds Tab|toolpath-tab-feed-params | 0.38 | 5 | 5 | role-mismatch-on-shared |
| Feeds Tab|feeds-card | 0.38 | 5 | 4 | role-mismatch-on-shared |
| Feeds Tab|optimize-modal | 0.29 | 4 | 4 | role-mismatch-on-shared |
| Operations Queue Panel|Toolpath Card Context Menu | 0.27 | 4 | 4 | role-mismatch-on-shared |
| Inspector › Project overview|Verdict HUD | 0.25 | 3 | 3 | role-mismatch-on-shared |
| Inspector › Project overview|Export Readiness modal | 0.19 | 4 | 3 | role-mismatch-on-shared |
| Viewport overlay toolbar|Inspector › View | 0.12 | 3 | 3 | role-mismatch-on-shared |
| Params Tab|feeds-modal-toolpath | 0.11 | 7 | 5 | role-mismatch-on-shared |
| menu-bar|project-tree | 0.11 | 6 | 5 | role-mismatch-on-shared |
| Params Tab|Feeds Tab | 0.09 | 6 | 5 | role-mismatch-on-shared |
| Params Tab|feeds-card | 0.08 | 5 | 4 | role-mismatch-on-shared |
| Params Tab|optimize-modal | 0.07 | 4 | 4 | role-mismatch-on-shared |

