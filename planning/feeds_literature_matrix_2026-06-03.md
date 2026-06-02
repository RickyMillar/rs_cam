# Feeds & Speeds — Literature Matrix Validation Suite

**Date:** 2026-06-03
**Predecessors:**
- 2026-06-02 audit workflow `w39ma2j1y` — 7 confirmed pipeline bugs
- 7 audit fixes landed (commits `3fab54b`, `cfe98ce`, `917e2fd`, `7cac9bd`, `6841df6`, `b1f52e7`)
- Research/plan workflow `wcyw1sb2b` (this doc consolidates its output + locked decisions)

**Problem:** the existing test suite (1686 unit + 62 integration + 18 smoke + F-024…F-037 sentries)
caught zero of the 7 audit bugs. Internal consistency tests can't detect "this number is wrong
but plausible" — only an external ground truth can. The literature matrix is that ground truth,
codified.

## Goal

Build a literature-derived validation matrix that catches numerically-plausible-but-machinist-wrong
recommendations. ~56 cells across (tool × operation × material) covering normal-use, extreme-size,
wrong-tool, and material-edge scenarios. Cells encode independent machinist ranges + physics
invariants, not values the engine happens to produce.

## Decisions locked (2026-06-03)

| # | Decision |
|---|---|
| Suite location | Top-level `tests/literature_matrix/` — independent, skip-friendly |
| Source set | Onsrud, Amana, GWizard, Shapeoko wiki, DAPRA/Kennametal, FPL Wood Handbook (6 gold sources) |
| Engine output surface | Adapter shim emits whatever machinist convention names (fpt, peck depth, SFM at engaged D); new engine APIs only if a real cell can't run without one |
| Hobby derate | 0.7× chipload, 0.5× DOC, applied at band-construction with both raw + derated stored |
| Rubbing-floor anti-pattern | 0.035 mm/tooth (midpoint of Shaw's 0.025 and community 0.05) |
| Wrong-tool policy | Warn, with `unusable` vs `unadvised` discriminator (see schema below) |
| Envelope vertices | Hand-pick in Phase 2; switch to derived in Phase 5 if maintenance hurts |

## Cell schema (TOML)

Each cell is one `[[cell]]` table. Conceptually:

- **Identity** — id, category, rationale (which bug class it catches)
- **Inputs** — tool, operation, material in machinist terms (tool_class, diameter_mm, material key)
- **Fixed inputs** (optional) — operator-pinned values for derived-only validation
- **Expected bands** — per output param: min/max/sources/unit/mode (`band` / `ceiling` / `floor` / `na`)
- **Coupling invariants** — closed-form physics inequalities that must hold regardless of trade-offs
- **Anti-patterns** — bright lines that are critical regardless of source agreement
- **Expected behaviour** — `values` / `unadvised` / `unusable` / `refuse` for the wrong-tool category

### Wrong-tool discrimination (key refinement from 2026-06-03 conversation)

```toml
[cell.expected_behaviour]
mode = "values" | "unadvised" | "unusable" | "refuse"
```

- **`values`** — normal-use; engine produces values in band.
- **`unadvised`** — works but not idiomatic. V-bit doing inlay paths. Ball nose for adaptive
  rough. Engine should derate conservatively OR emit a warning. **CI: warn-only, never blocks.**
  Trend log tracks if engine produces aggressive values in this regime.
- **`unusable`** — physics says no. Drill for chamfer. Flat tool for scallop. Engine MUST refuse
  or fail critical. **CI: block on engine producing values.**
- **`refuse`** — strictly the engine should reject (rare; mostly subsumed by `unusable`).

### Worked example — normal-use cell

```toml
[[cell]]
id = "flat_6mm_pocket_oak"
category = "normal-use"
description = "6mm 2F flat in red oak, conventional pocket"
rationale = "Anchors hardwood pocket band; catches LUT/unit errors"
profile_tag = "either"

[cell.inputs]
tool_class = "flat"
diameter_mm = 6.0
flute_count = 2
flute_length_mm = 22.0
stickout_mm = 25.0
operation = "pocket"
material = "oak_red"
material_class = "hardwood"
janka_lbf = 1290
machine_class = "shapeoko_xxl"

[cell.fixed_inputs]
woc_mm = "free"
doc_mm = "free"

[cell.expected.rpm]
min = 16000
max = 20000
mode = "band"
unit = "rpm"
sources = ["onsrud_hwood", "amana_spektra", "gwizard_hwood", "shopbot"]

[cell.expected.feed_per_tooth]
min = 0.028   # 0.04 raw × 0.7 hobby derate
max = 0.070   # 0.10 raw × 0.7 hobby derate
mode = "band"
unit = "mm/tooth"
hobby_derate = 0.7
sources = ["onsrud_hwood", "cutter_shop", "toolgrit"]

[cell.expected.axial_doc]
min = 0.75   # 1.5 raw × 0.5 hobby derate
max = 2.25   # 4.5 raw × 0.5 hobby derate
mode = "band"
unit = "mm"
hobby_derate = 0.5
sources = ["onsrud_doc_rule", "shapeoko_wiki"]

[cell.expected.plunge_feed]
min_fraction_of_feed = 0.30
max_fraction_of_feed = 0.50
mode = "fraction"
sources = ["shapeoko_wiki", "vectric_default"]

[[cell.invariants]]
name = "chipload_above_rubbing_floor"
expr = "feed_rate / (rpm * flutes)"
floor = 0.035
unit = "mm/tooth"
severity_on_fail = "critical"

[[cell.invariants]]
name = "mrr_under_power"
expr = "doc * woc * feed_rate / 60000"
ceiling = 0.20   # hardwood 1kW × 0.6 eff / 30 J/mm^3
unit = "cm^3/s"
severity_on_fail = "moderate"

[[cell.invariants]]
name = "envelope_pocket"
type = "convex_hull"
vars = ["woc_over_d", "doc_over_d"]
vertices = [[0.40, 0.25], [0.75, 0.25], [0.75, 0.75], [0.40, 0.75]]
severity_on_fail = "minor"

[[cell.anti_patterns]]
name = "chipload_below_rubbing_floor"
expr = "feed_rate / (rpm * flutes) < 0.025"
severity = "critical"
```

### Worked example — `unadvised` cell

```toml
[[cell]]
id = "ball_6mm_adaptive2d_oak_unadvised"
category = "wrong-tool"
description = "Ball-nose used for adaptive roughing — works but not idiomatic"
rationale = "Catches engines that treat ball-nose identically to flat for roughing"

[cell.inputs]
tool_class = "ball"
diameter_mm = 6.0
flute_count = 2
stickout_mm = 25.0
operation = "adaptive2d"
material = "oak_red"
material_class = "hardwood"
janka_lbf = 1290
machine_class = "shapeoko_xxl"

[cell.expected_behaviour]
mode = "unadvised"
expected_derate = 0.5   # engine should produce values ≤0.5× of flat-tool equivalent
expected_warning_pattern = "ball.*adaptive|flat preferred"   # optional warning diagnostic

[cell.expected.axial_doc]
min = 0.0
max = 1.8
mode = "ceiling"
unit = "mm"
note = "limited to ~0.3×D for ball roughing"

[[cell.invariants]]
name = "ball_roughing_doc_capped"
expr = "doc / D <= 0.4"
severity_on_fail = "moderate"   # warn-only on unadvised cells
```

### Worked example — `unusable` cell

```toml
[[cell]]
id = "flat_6mm_scallop_oak_unusable"
category = "wrong-tool"
description = "Flat end mill for scallop op — scallop math requires curved tip"
rationale = "Catches engines that silently accept flat tools for cusp-driven finish"

[cell.inputs]
tool_class = "flat"
diameter_mm = 6.0
flute_count = 2
operation = "scallop"
material = "oak_red"
material_class = "hardwood"

[cell.expected_behaviour]
mode = "unusable"
expected_refuse_pattern = "scallop.*(requires|expects).*(ball|tapered|bull)"

[[cell.anti_patterns]]
name = "non_ball_tool_for_scallop"
expr = "operation == 'scallop' AND tool_class not in ['ball', 'tapered_ball', 'bull']"
severity = "critical"
```

## Verdict shape

Per-cell, per-param sub-verdicts roll up to a cell verdict:

```
cell: flat_endmill_6mm × pocket × hardwood
  rpm:        18000      [16000-20000]   Within
  fpt:        0.05 mm    [0.028-0.070]   Within
  doc:        3.0 mm     [0.75-2.25]     Outside (+33%)
  woc:        2.4 mm     envelope        Within
  invariants:
    chipload_above_rubbing_floor:  Within
    mrr_under_power:               Edge (94% of envelope)
    envelope_pocket:               Within
  verdict: moderate (doc out 33% — likely real)
```

Sub-verdict tiers: `Within` / `Edge` (within 10% of band edge) / `Outside` / `NA`.
Cell verdict = worst sub-verdict, with severity adjustment from anti-pattern hits.

CI policy:
- `cosmetic` / `minor` — log only
- `moderate` — log; block at ≥10% of cells per run (aggregate guard)
- `major` / `critical` — block
- `unadvised` cells: never block, always log to trend file
- `unusable` cells: block if engine produced values; pass if engine refused

## Coverage — 56 cells

| Category | Count | What it tests |
|---|---:|---|
| Normal-use | 22 | Conventional tool/op/material — anchors bands, catches LUT errors |
| Extreme-size | 14 | Tiny tools (1.5 mm flat, 1 mm ball) + oversized (12 mm in 1kW spindle) — clamps + power caps |
| Wrong-tool | 14 | Mix of `unusable` (drill for chamfer, flat for scallop) and `unadvised` (V-bit inlay, ball adaptive rough) |
| Material-edge | 6 | Ipe (Janka 3680), HDPE vs acrylic, fiberglass abrasive |

Diameter grids per tool class:
- **Flat**: [1.5, 3.0, 6.0, 12.0] mm
- **Ball**: [1.0, 3.0, 6.0] mm
- **Bull**: [6.0] mm with R1.0 corner
- **V-bit**: [60°, 90°, 20°] included angles, 0.1 mm tip
- **Tapered ball**: [1 mm tip / 7°, 2 mm tip / 5°]

Material grid (canonical instances):
- Softwood: `pine_eastern_white` (Janka 380)
- Hardwood: `oak_red` (Janka 1290), `maple_sugar` (1450)
- Material-edge hardwood: `ipe` (3680)
- Plywood: `baltic_birch`
- MDF: `mdf`
- Plastic ductile: `hdpe`
- Plastic brittle: `acrylic`
- Aluminum: `al6061` (LUT-only — formula path forbidden)
- Composite: `fiberglass_gp`

## Starter 12 cells (Phase 1 set)

Already drafted with full TOML in the workflow output. Categories:
1. `flat_6mm_pocket_oak` — normal-use anchor (hardwood)
2. `flat_6mm_adaptive2d_birchply` — normal-use, exercises chip-thinning RCTF
3. `ball_6mm_scallop_maple` — normal-use, scallop-stepover geometric coupling
4. `vbit_60deg_vcarve_mdf` — normal-use, V-bit engaged-D SFM
5. `flat_3mm_drill_oak` — normal-use, Z-only kinematics + peck cycle
6. `flat_1p5mm_pocket_oak_clamp` — extreme-size, sub-2mm RPM clamp + deflection
7. `flat_12mm_adaptive2d_oak_power` — extreme-size, 1kW spindle power cap
8. `vbit_60deg_pocket_oak_unadvised` — unadvised (works but not idiomatic)
9. `flat_6mm_scallop_oak_unusable` — unusable (scallop needs curved tip)
10. `ball_6mm_adaptive2d_oak_unadvised` — unadvised (ball for rough — derate expected)
11. `flat_6mm_pocket_ipe_hardness` — material-edge, Janka scaling on extreme hardwood
12. `flat_6mm_pocket_hdpe_thermal` — material-edge, HDPE vs acrylic distinction

## Source registry

All citations live in `tests/literature_matrix/sources.toml`. Each cell's
`sources = [...]` references entries by key. Schema:

```toml
[onsrud_hwood]
name = "Onsrud Cutter Hardwood Feed Chart"
citation_url = "https://www.onsrud.com/files/pdf/series_70_85.pdf"
authority_tier = "gold"
last_verified = "2026-06-03"
covers = ["flat", "ball", "vbit"]
ops = ["pocket", "profile", "adaptive2d", "vcarve"]
materials = ["hardwood", "softwood", "plywood"]
notes = "De-facto wood-router standard. Industrial spindle assumed; apply hobby derate."

[amana_spektra]
# ...

[gwizard_hwood]
# ...

[shapeoko_wiki]
# ...

[dapra_rctf]
name = "DAPRA Chip Thinning Formula (Radial Chip Thinning Factor)"
authority_tier = "gold"
# ...

[fpl_wood_handbook]
name = "USDA FPL Wood Handbook (FPL-GTR-190)"
citation_url = "https://www.fpl.fs.fed.us/products/publications/several_pubs.php?grouping_id=100"
authority_tier = "gold"
notes = "Material constants — Janka, density, Kc proxies. Not feed values directly."
```

Anti-circularity rule: cells must have ≥3 sources per band with at least one from a non-overlapping
lineage. Reviewed during Phase 2 cell population.

## Phase plan

| Phase | Goal | Deliverable | Effort |
|---|---|---|---:|
| 0 — Scaffold | Schema + harness + 1 cell end-to-end | `tests/literature_matrix/` dir, runner, 1 cell, passing harness tests | 1-2d |
| 1 — Starter 12 | Validate schema against diversity | 12 cells running, first findings observed, schema frozen | 2-3d |
| 2 — Bulk to 56 | Populate the matrix per coverage strategy | ~56 cells, citation index, first full report | 3-5d |
| 3 — Triage | Classify findings, fix real bugs | Findings backlog with verdicts; first N bugs fixed | 2-4d |
| 4 — CI gate | Block on major+, PR comments on trends | Gated `cargo test --test literature_matrix`, PR comment renderer, docs | 2-3d |
| 5 — Maintenance | Keep matrix useful past year 1 | `last_verified` decay warnings, refresh skill | 1-2d |

**Total estimate: ~11-19 days** across phases, with Phase 0-2 the path to value (~6-10 days)
and Phases 3-5 the path to durability (~5-9 days).

### Phase 0 detail

Tasks:
1. Create `tests/literature_matrix/` directory.
2. Define `LiteratureCell` Rust deserializer (serde) matching the TOML schema above.
3. Implement the four invariant primitives:
   - `band-check` (min/max with mode = band/ceiling/floor)
   - `convex_hull` (point-in-polygon for 2D envelopes)
   - `closed-form expr evaluator` (use `evalexpr` crate or small AST; vars = `rpm, fpt, feed_rate, doc, woc, D, flutes, stickout, Kc, E, tip_radius, included_angle, ...`)
   - `anti-pattern check` (boolean expr → critical on true)
4. Build the engine adapter shim (`tests/literature_matrix/shim.rs`): translates cell inputs into
   `FeedsInput` + `OperationConfig`, calls the suggest pipeline, extracts outputs into machinist-
   convention values. Lives in tests — does NOT couple to engine internals.
5. Per-cell evaluator: load cell → build shim input → query engine → compute sub-verdicts → roll up
   to cell verdict → emit structured report row.
6. Text + JSON report renderers.
7. Add one cell (`flat_6mm_pocket_oak`) to `tests/literature_matrix/cells.toml` and the
   `sources.toml` entries it references.
8. Add `tests/literature_matrix/harness_tests.rs` — unit tests for the evaluator itself (golden
   inputs → golden verdicts), independent of cell content.
9. Wire: `cargo test --test literature_matrix` runs the matrix.

Exit gate: harness tests pass; the one cell loads, runs, produces a structured report.

### Phases 1-5

Specified in the workflow output verbatim; this doc lists them in the table above. Each phase
has tasks + deliverable + estimate documented in `wcyw1sb2b.output` and is reproduced here
as needed during execution.

## Risk register

| Risk | Mitigation |
|---|---|
| Literature bands drift / age out | `last_verified` on every source; CI warns >18 months stale; union-of-3-sources widens window |
| Bands too wide to catch bugs | Band-width sanity floor (>3× → `flags.low_signal = true`, can't escalate above minor); invariant + anti-pattern layers are independent of band width |
| Circular dependency on vendor LUT data the engine uses | ≥3 sources per band with non-overlapping lineage (audited in Phase 2); physics invariants (Kc, deflection beam) are derived independently |
| Trade-off envelopes false-alarm | Start envelopes generous; convert any false-alarming cell to `invariant_only` mode in Phase 3 triage |
| Engine doesn't expose needed outputs | Adapter shim computes missing outputs from emitted values where possible; `flags.ci_gate = "skip"` on cells that genuinely can't run |
| Maintenance burden exceeds value | Cap at ~60 cells; centralised citations; Phase 5 adds derived-bands automation |
| Anti-pattern thresholds defensibly wrong | Anti-patterns are physics inequalities (chipload > 0; plunge ≤ feed; DOC ≤ flute length; engaged-D SFM); numeric thresholds (e.g. 0.035 rubbing floor) cite the most-conservative defensible source |

## Success criteria

1. **Bug-finding (primary):** ≥1 confirmed engine bug surfaced in Phase 3 triage that the
   2026-06-02 audit missed. Predicted high-confidence candidates: chip-thinning RCTF in adaptive
   ops, plastic-class collapse (HDPE/acrylic), Janka scaling on Ipe.
2. **Confirmation (also valid):** if zero bugs surface, the suite produces a written confirmation
   report — "engine recommendations align with literature across 56 cells covering 5 tool classes
   × 10 ops × 7 material classes". A trust / marketing artifact, not vanity.
3. **CI gate stable:** after Phase 4, suite runs on every PR for ≥4 weeks with <1 false-positive
   escalation per week. Failures are actionable.
4. **Trend signal works:** intentional tuning shifts a cell's verdict from one source's band to
   another → informational PR comment. Accidental refactor breaks 5 cells → blocks.
5. **Citations auditable:** every passing cell can show its work via the structured report.
6. **No indefinite growth:** cell count stays at 50-70 through year 1. Old cells removed when
   their bug class is permanently fixed and the cell's verdict has been stable for 6+ months.
7. **Anti-vanity gate:** measured by bugs found + false-positive rate + presence in PR review
   conversations. NOT by cell count, pass percentage, or LOC.

## First-run predictions (from workflow `wcyw1sb2b`)

**4-8 findings expected; 2-4 likely real bugs the audit missed.**

High-confidence (60-80%):
1. **Chip-thinning RCTF compensation missing or incomplete in adaptive ops**
2. **V-bit RPM uses nominal D somewhere other than the path we just fixed**
3. **Plastic-class collapse** — HDPE and acrylic get identical recommendations

Medium-confidence (30-50%):
4. Janka scaling absent on extreme hardwoods (Ipe)
5. Spindle-power cap missing on 12 mm tools
6. Drill cycles using mill chipload bands somewhere

If zero findings, criterion #2 (confirmation) still delivers value.

## Operational guardrails (for the executing session)

These apply to whichever fresh session runs Phases 0-5. Per session memory:

- **Never stage**: `crates/rs_cam_core/src/feeds/explain.rs`, `crates/rs_cam_viz/src/ui/feeds_modal.rs`,
  `.mcp.json`. They stay unstaged working-tree edits.
- **Never touch**: `MachineKinematics::shapeoko_xxl_ricky_tuned()`.
- **Test commands**:
  - `cargo test -p rs_cam_core --lib`
  - `cargo test -p rs_cam_core --tests`
  - **Never** `cargo test --workspace` (infinite loop).
  - **Never** `cargo test` during a release build of `rs_cam_viz` (PC crash).
- **No `cargo fmt` workspace-wide** (cascades).
- **Zero-warning clippy** (16 deny lints in `Cargo.toml`).
- **Sentries must stay green**: F-024, F-026, F-027, F-028, F-031, F-036b, F-037 baseline diff.

The literature matrix itself does NOT modify the engine in Phases 0-2. If Phase 3 triage surfaces
engine bugs, fixes follow the same per-bug-commit pattern as the 2026-06-02 audit closeout —
each fix is its own commit with a regression test.

## Status

**Plan closed; ready to execute.** Next action: a fresh session (after compact) runs Phase 0
scaffold + 1 cell to validate the schema, then proceeds through Phase 1 (starter 12) and Phase 2
(bulk to 56). Phases 3-5 are gated on Phase 2 completion.
