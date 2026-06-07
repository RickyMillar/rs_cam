# UI / IA Audit Workflow

> **Goal:** catalogue what the GUI actually does, map how its surfaces interact,
> diagnose where the information architecture (IA) fails, and produce a concrete
> redesign **spec** (no code changes). Breadth-first inventory → targeted deep-dive
> → spec. Evidence is code-led, confirmed against the live GUI.
>
> **Parameters locked for this run:**
> - Sweep: **breadth-first inventory, then deep-dive the worst offenders**
> - Endpoint: **audit + redesign spec** (stop before touching code)
> - Evidence: **both, code-led** — egui source is ground truth, MCP screenshots confirm lived experience

---

## 0. The rubric (what "bad IA" means here — generalized from the brief)

The complaints are treated as *symptoms*, not a checklist. The audit judges every
surface against these principles. Each becomes a diagnostic lens in Phase 3.

| # | Principle | Failure it catches |
|---|-----------|--------------------|
| P1 | **One concern, one home** | A capability surfaced in N places (feeds/speeds in 4 panels) |
| P2 | **Group by concern, not by accident** | Toolpath options scattered instead of grouped: *geometry / feeds&toolpath / linking / dressup / heights* |
| P3 | **Dig deeper, don't dump** | Flat walls of fields with no summary→detail progression |
| P4 | **Distinct roles must look distinct** | Confusables: things that look alike but do different jobs (the 3 suggest systems) |
| P5 | **UI wins, not prose** | Explanatory paragraphs doing a job an affordance should do |
| P6 | **Every control earns its place** | Orphans: present in UI but not wired end-to-end; dead duplicates |
| P7 | **Provenance is legible** | When a value is *recommended*, it's unclear which system suggested it (suggest icon / vendor LUT / sim feedback) |

These map 1:1 to the diagnosis taxonomy in Phase 3. Keep them stable — they're the
contract every agent reports against.

---

## 1. The deliverable is itself "dig deeper" (artifact schema)

The audit output mirrors the UX philosophy we're advocating: a tiered catalogue you
read top-down and drill into. Three layers, all in `planning/ui_audit/`.

**Layer A — `MAP.md` (one screen).** One line per *surface*: name, where it lives,
its single-sentence job, and a health flag (🟢/🟡/🔴). This is the summary you skim.

**Layer B — surface records (`surfaces/<name>.md`).** One file per surface. Drill
target from Layer A. Fixed schema:

```
surface: feeds_modal
file: crates/rs_cam_viz/src/ui/feeds_modal.rs
kind: modal | panel | tab | inline-widget | popup | menu
job: <one sentence — the ONE thing this surface is for>
opens-from: <what launches it>
controls: [ list of control IDs -> Layer C ]
reads-state: [ ProjectSession fields / configs read ]
writes-state: [ fields written ]
confusable-with: [ other surfaces a user could mistake this for ]  # P4
recommendation-sources-touched: [ suggest-icon | vendor-lut | sim-feedback ]  # P7
health: 🟢|🟡|🔴  + one-line why
```

**Layer C — the capability matrix (`CAPABILITY_MATRIX.csv` + `.md`).** The core
diagnostic. Rows = abstract *capabilities* (e.g. "set spindle RPM", "set radial
stepover", "request a feeds suggestion"). Columns = surfaces. A cell is filled when
that surface exposes that capability, and records the *role* it plays there
(authoritative edit / mirror / suggestion / read-only display). This is what makes
duplication and fragmentation fall out mechanically:
- a row with many filled cells → **duplication** (P1)
- a capability cluster spread across many surfaces → **fragmentation** (P2)
- two columns with near-identical rows but different roles → **confusable** (P4)

Capabilities are abstract and surface-independent — that's the whole point. Two
panels editing the same `feed_rate` field are *one* capability in two cells, not two
capabilities.

---

## 2. Phases

### Phase 1 — Breadth-first inventory (code-led)
Sweep **every** file under `crates/rs_cam_viz/src/ui/` plus the state it binds to
(`state/`, `controller/`, the `rs_cam_mcp` param structs). Produce Layer A + Layer B
for *all* surfaces, shallow but complete. No diagnosis yet — just "what exists, what
it touches." Fan out by surface cluster so no agent holds the whole UI:

- **Cluster 1 — Toolpath & operations:** `toolpath_panel`, `toolpath_row_controls`,
  `properties/operations/*`, `properties/mod` (operation portions)
- **Cluster 2 — Feeds/speeds & recommendation:** `feeds_modal`, `optimize_modal`,
  `optimize_project`, every inline suggest icon, vendor-LUT entry points, the
  sim→feeds feedback path
- **Cluster 3 — Setup/stock/tool/post:** `setup_panel`, `properties/{setup,stock,tool,post}`,
  `tool_library_modal`
- **Cluster 4 — Simulation & diagnostics:** `sim_*`, `viewport_overlay`, `preflight`
- **Cluster 5 — Shell & cross-cutting:** `menu_bar`, `workspace_bar`, `status_bar`,
  `project_tree`, `automation`, `export_wizard`, `shortcuts_window`

Each cluster agent: reads the source as ground truth, fills Layer B records, lists
the capabilities it found (feeding Layer C), and flags state reads/writes. **Gate G1:**
review Layer A/B for completeness before diagnosis — did we miss a surface? (P6 starts here.)

### Phase 2 — Live confirmation (MCP, thin)
Drive the running GUI (`--mcp`), screenshot each surface from Layer A, and annotate
the Layer B records with what's *actually visible* vs. what the code defines. This is
deliberately thin — it exists to (a) catch confusables that only show up visually
(P4), (b) catch text-crutches (P5), and (c) catch orphans where code exists but the
control never renders / isn't reachable (P6). Screenshots stored alongside records.

### Phase 3 — Diagnosis (matrix-driven)
Build `CAPABILITY_MATRIX` from all Layer B capability lists, then run the seven
lenses (P1–P7) against the matrix + records. Each finding is a record:

```
finding: F-IA-NNN
lens: P1..P7
severity: high|med|low
surfaces: [...]
capability: <row, if applicable>
evidence: <file:line / screenshot>
user-impact: <one sentence — what goes wrong for the user>
```

Adversarial pass: a second agent tries to *refute* each finding (is the "duplicate"
actually two legitimately-different roles? is the "orphan" reachable by a path we
missed?). Only survivors ship. Output: `DIAGNOSIS.md` ranked by severity × frequency.

### Phase 4 — Redesign spec
For the worst offenders surfaced in Phase 3 (deep-dive targets), produce the spec.
Three artifacts, all UI-first:

1. **`IA_TARGET.md` — the new structure.** Capabilities regrouped by concern (P2),
   with the depth tiers made explicit (P3: what's on the summary, what's behind a
   drill). For each consolidated home, the *single job* it now owns (P1).
2. **ASCII mockups** of the redesigned surfaces — layout, grouping, what collapses /
   expands. This is where "UI wins" is demonstrated, not described: confusables
   given distinct visual roles (P4), prose replaced by affordances (P5), the three
   recommendation systems given one legible provenance treatment (P7).
3. **`MIGRATION.md` — old→new map.** Every capability from the matrix mapped to its
   new home. This is the "miss nothing" guarantee: a capability that doesn't appear
   in the new structure is either explicitly retired (with reason) or it's a bug in
   the spec. No silent drops.

**Gate G2:** you review the spec. Implementation is a separate decision.

---

## 3. Execution shape

Maps onto an agent fan-out (runnable as a `Workflow` on explicit opt-in):

- **Phase 1:** 5 cluster agents in parallel → Layer A/B. Barrier at **G1**.
- **Phase 2:** per-surface MCP screenshot pass (pipeline; thin).
- **Phase 3:** 1 matrix-builder → 7 lens agents (parallel) → adversarial refuter per
  finding (pipeline). Barrier before ranking.
- **Phase 4:** deep-dive agents, one per worst-offender domain, each emitting target +
  mockup + migration rows. Synthesis agent reconciles into one spec. Barrier at **G2**.

Two human gates (G1 completeness, G2 spec sign-off); everything between is automated
fan-out. Token cost scales with UI size — Phase 1 is the bulk.

---

## 4. Why this shape

- **Breadth-first first** so the capability matrix is *complete* before we judge
  anything — fragmentation and duplication are only visible globally, never from one panel.
- **Capabilities as the unit** (not fields, not files) so "same thing in 4 places"
  becomes one row with 4 cells instead of an argument.
- **The deliverable is layered** (map → record → matrix) so it practices the
  dig-deeper principle it recommends — and you can stop reading at the depth you need.
- **Adversarial refutation** so we don't "consolidate" two controls that were
  legitimately distinct (the inverse of P4 — over-merging is also bad IA).
- **Stops at spec** per the locked endpoint; migration map keeps implementation honest later.
