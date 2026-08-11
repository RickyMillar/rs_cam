# A-3 — apply-contract census

Wave: TD3 Lane A, wave A-3 (research only; no production code changed).
Date: 2026-08-12. Branch `tech-debt-3`, censused at `675a643`.
Seed: `planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`
[high] "The modal and main panel have different safety and apply contracts".
Plan: `planning/review_2026-08-08/TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md` §2 A-3.
Feeds Checkpoint I package: §5 below.

---

## 0. One-paragraph answer

There are **twenty** write paths from a recommendation into an
`OperationConfig`, thirteen of them GUI apply affordances. **Two** of the
thirteen validate the tool × operation pairing before offering anything; the
other **eleven** are reachable on a pairing the engine has declared physically
unrunnable, and **seven** of those eleven change cut geometry. Worse than the
review's two named hazards: **seven of the thirteen bypass the invariant funnel
entirely**, writing the raw calculator output with no clamp, no back-off and no
rounding. On the shipped default fixture that gap is **3.5×** on depth of cut —
the modal's per-field DOC `Apply` writes **4.445 mm** where the panel's
`⚡ Apply cut geometry` writes **1.27 mm** (§3.4). The most innocuous-looking
affordance in the product is the one that writes the deepest unguarded cut.

---

## 1. Site verification and drift from the review

The review cited line numbers at revision `d820226`. A-2 landed a rename sweep
and the operating-point card in the same two files (`6754a0b`, `709d6c9`,
`12de0a8`), so the panel numbers moved. Verified at `675a643`:

| Review's citation | Status at `675a643` | Drift |
|---|---|---|
| `feeds/suggest.rs:637-664` — validated recipe | `feeds_result_for_operation` at **`:645-668`** | **+8** |
| `feeds/suggest.rs:671-689` — infallible preview | `feeds_explain_for_operation` at **`:671-689`** | **0** |
| `viz/ui/feeds_modal.rs:381-587` — modal controls | compare card `:482-596`, `Apply all` at **`:586-593`** | ≈ **0** (in range) |
| `viz/controller/events/mod.rs:780-948` — modal write paths | `apply_feeds_field` **`:771-822`**, `apply_feeds_all` **`:827-887`**, project fan-outs **`:925-951`**, explore **`:955-980`** | ≈ **0** (in range; range is now slightly wider than cited) |
| `viz/ui/properties/mod.rs:1724-1930` — panel split apply | **`:1982-2001`** (speeds) and **`:2024-2043`** (cut geometry) | **+~250** — A-2's operating-point card (`draw_operating_point`, `:1657-…`) was inserted above them |

Every claim in the review's Evidence paragraph reproduces. Two corrections of
emphasis, both in the direction of "worse than stated":

- The review says the modal's actions "can write the preview values directly
  **or** invoke the full legacy apply path". Both happen, on different buttons,
  and the *direct* write is the more dangerous of the two — see §3.4.
- The review names one modal `Apply all`. There are **three** buttons that
  reach `ApplyFeedsAll` plus a per-row one, two of which fan out across the
  whole project (§2 rows M7–M11).

---

## 2. The census — every write path into `OperationConfig`

Legend. **Validated** = the surface is gated on
`validate_tool_for_operation` (`feeds/mod.rs:702`) succeeding.
**Funnel** = the write goes through `apply_feeds_subset`
(`feeds/suggest.rs:727-793`) and therefore through `enforce_invariants` +
`round_suggestion_value`; `raw` = it does not. **Scope** = which dimensions it
can move: `speed` (feed/plunge/RPM) or `cut` (stepover/DOC).

### 2a. Feeds & Speeds modal — preview source is `feeds_explain_for_operation` (infallible)

| # | Entry point (file:line) | UI label | Validated | Scope | Funnel | Handler |
|---|---|---|---|---|---|---|
| M1 | `ui/feeds_modal.rs:517-525` → `ui/components/compare.rs:139-146` | `Apply` (RPM row) | **no** | speed | **raw** | `events/mod.rs:797-800` |
| M2 | `ui/feeds_modal.rs:526-534` → `compare.rs:139-146` | `Apply` (Feed row) | **no** | speed | **raw** | `events/mod.rs:801-804` |
| M3 | `ui/feeds_modal.rs:535-543` → `compare.rs:139-146` | `Apply` (Plunge row) | **no** | speed | **raw** | `events/mod.rs:805-808` |
| M4 | `ui/feeds_modal.rs:544-552` → `compare.rs:139-146` | `Apply` (DOC row) | **no** | **cut** | **raw** | `events/mod.rs:809-812` |
| M5 | `ui/feeds_modal.rs:553` → `woc_row` `:623-631` | `Apply` (WOC row) | **no** | **cut** | **raw** | `events/mod.rs:813-816` |
| M6 | `ui/feeds_modal.rs:664-673` | `Apply` (WOC row, scallop-derived variant) | **no** | **cut** | **raw** | `events/mod.rs:813-816` |
| M7 | `ui/feeds_modal.rs:586-593` | **`⚡ Apply all`** | **no** | speed + **cut** | funnel | `events/mod.rs:827-887` → `apply_feeds_result_to_op` (`ApplySubset::Both`) |
| M8 | `ui/feeds_modal.rs:2117-2126` | `✓ Apply explored values` (Chart C drag) | **no** | speed | **raw** | `events/mod.rs:955-980` |
| M9 | `ui/feeds_modal.rs:2829-2831` | `Apply` (project-tab row) | **no** | speed + **cut** | funnel | → M7's handler |
| M10 | `ui/feeds_modal.rs:2760-2765` | `⚡ Apply selected` | **no** | speed + **cut** | funnel | `events/mod.rs:925-935` → M7 × N |
| M11 | `ui/feeds_modal.rs:2766-2772` | `⚡⚡ Apply all toolpaths` | **no** | speed + **cut** | funnel | `events/mod.rs:939-951` → M7 × every **enabled** toolpath |

### 2b. Properties panel — recipe source is `feeds_result_for_operation` (validated)

| # | Entry point (file:line) | UI label | Validated | Scope | Funnel | Callee |
|---|---|---|---|---|---|---|
| P1 | `ui/properties/mod.rs:1982-2001` | **`⚡⚡ Apply recommended speeds`** ("Does not change the cut (DOC/WOC)") | **yes** | speed | funnel | `apply_speeds_to_op` (`ApplySubset::Speeds`) |
| P2 | `ui/properties/mod.rs:2024-2043` | **`⚡ Apply cut geometry`** ("Changes the cut.") | **yes** | **cut** | funnel | `apply_cut_geometry_to_op` (`ApplySubset::CutGeometry`) |

Both are drawn only inside the `Ok(result)` arm of
`calculate_and_apply_feeds` (`:1582-1602`). On the `Err` arm (`:1603-1646`) the
whole feeds card is replaced by `Feeds unavailable: {e}` plus two manual
DragValues — **no apply affordance exists**. That asymmetry is hazard (a).

### 2c. Optimizer — simulation-backed, a different recommendation engine

| # | Entry point | UI label | Validated | Scope | Funnel |
|---|---|---|---|---|---|
| O1 | `events/mod.rs:~530-637` | Optimize modal apply-candidate | n/a (gate-scored, not feeds-validated) | speed + **cut** (whole `OperationConfig` snapshot) | **n/a — bypasses feeds invariants by design** |
| O2 | `events/mod.rs:644-700` (`reoptimize_with_axis_override`, OPT-005) | accept an axis suggestion | no | speed + **cut** (`Stepover`/`DepthPerPass`/`ScallopHeight` axes) | **raw** `set_*` |
| O3 | `events/mod.rs:~1090-1130` (`optimize_project`) | project batch apply | no | speed + **cut** | **n/a** |

These are out of the review's finding and out of Checkpoint I's scope, but they
belong in the census because a funnel that claims to be "one application API"
must decide whether the optimizer joins it or is explicitly excluded (§5, Q4).

### 2d. Creation-time and batch — one-shot suggest writes

| # | Entry point | Trigger | Validated | Scope |
|---|---|---|---|---|
| C1 | `viz/controller/events/toolpath.rs:73-101` | GUI **Add toolpath** | **yes** — refusal aborts the add with a notification | whole op |
| C2 | `viz/app/mcp.rs:3109-3129` | MCP `add_toolpath` | **yes** — refusal returns `Cannot add toolpath: {e}` | whole op |
| C3 | `cli/src/project.rs:570-650` | `project --apply-suggest` | **yes** — refused pairings are skipped with a `warn!` | speed + **cut** |
| C4 | `cli/src/smoke.rs:300` | `smoke` fixture build | **yes** | whole op |

### 2e. Counts

| | count |
|---|---|
| GUI apply affordances (M + P) | **13** |
| — validated | **2** (P1, P2) |
| — infallible / unvalidated | **11** (M1–M11) |
| — geometry-capable | **8** (M4, M5, M6, M7, M9, M10, M11, P2) |
| — speed-only | **5** (M1, M2, M3, M8, P1) |
| — through the invariant funnel | **6** (M7, M9, M10, M11, P1, P2) |
| — **raw writes, no funnel** | **7** (M1–M6, M8) |
| Optimizer writes | 3 |
| Creation-time / batch suggest writes | 4 (all validated) |
| **Total write paths censused** | **20** |

### 2f. What the MCP surface does *not* have

There is **no MCP tool that applies a feeds recommendation to an existing
toolpath.** `get_suggest_rationale` (`mcp_server.rs:488-497`) is read-only and
says so; `set_spindle_strategy` (`:1159`) states explicitly that "existing
toolpath param values are NOT mutated by this call". The only agent-side write
is `set_toolpath_param`, a raw operator write that is neither feeds-validated
nor invariant-funnelled — i.e. the agent has the *modal's* contract with none
of the modal's preview. Recorded here because Checkpoint I's funnel is the
natural place to decide whether the agent surface gets an `ApplyScope`-shaped
tool or stays deliberately manual (§5, Q5).

---

## 3. Demonstrations

**Evidence class for every demonstration below: controller-level integration
test.** The tests construct a real `AppController` over a scripted
`ComputeBackend`, populate a real `ProjectSession`, and dispatch the exact
`AppEvent` each modal button pushes through the production handler in
`controller/events/mod.rs`. They exercise production code, not a mirror of it.

What that class does **not** cover: the egui render pass. The
button → `AppEvent` mapping (which label pushes which event) is read from
source and cited per test; it is not clicked. Two stronger classes were
considered and are honestly unavailable:

- **Live GUI screenshots.** The plan's §2 A-3 line asks for these. The
  operator's GUI/MCP session was disconnected for the whole wave and §0.8
  forbids the release build that `.mcp.json` launches. A debug GUI could have
  been launched with `WAYLAND_DISPLAY` unset, but a screenshot of a modal is
  evidence about *labels*, and the labels are already quoted verbatim in §2
  from source; it would not have added a fact the tests do not carry. **NOT
  EXERCISED — blocker: operator session disconnected + no release build inside
  a wave (§0.8).**
- **MCP-driven repro.** Impossible in principle: MCP cannot inject a modal
  click, and per §2f there is no MCP apply tool to stand in for one.

Test file: `crates/rs_cam_viz/tests/apply_contract_a3.rs` — 9 tests, all green
at `675a643` (`cargo test -p rs_cam_viz --test apply_contract_a3 -q`). They are
**characterization** tests: they pin today's behaviour including the wrong
parts, and A-4 is expected to invert them rather than delete them.

### 3.1 Hazard (a) — the modal applies a pairing the panel refuses

Fixture: a default Ø6.35 mm 2-flute **flat end mill** on a **Scallop** op —
`validate_tool_for_operation` refuses it because a zero tip radius makes the
scallop-stepover formula `2·√(2·R·h − h²)` undefined.

| Surface | Result |
|---|---|
| Panel (`feeds_result_for_operation`) | `Err(WrongToolForOperation { operation: Scallop, actual_geometry: Flat, required: "ball\|bull\|tapered_ball" })` → card replaced by `Feeds unavailable: …`, **no apply button** |
| Modal (`feeds_explain_for_operation`) | recommends feed **2677.07 mm/min**, plunge **793.75**, RPM **10025.5** — a full Recommendation table with live Apply buttons |

Dispatching `AppEvent::ApplyFeedsAll` on that toolpath writes it:

| field | before | after `⚡ Apply all` |
|---|---|---|
| feed | 1000 mm/min | **2677** |
| plunge | 500 mm/min | **794** |
| spindle RPM | `None` (project default) | **`Some(10026)`** |

Tests: `hazard_a_panel_refuses_the_pairing_the_modal_previews`,
`hazard_a_modal_apply_all_writes_the_refused_recipe`,
`hazard_a_modal_per_field_apply_writes_the_refused_recipe`.

A measured refinement worth carrying: on the *Scallop* fixture the per-field
DOC and WOC applies write nothing — **not because they are refused**, but
because `ScallopConfig` carries neither dial (a scallop steps by chord, not by
Z layers or a raster pitch). The test asserts the distinction explicitly,
because "the apply was refused" and "the operation had nowhere to put it" are
different facts and only the first would be a safety property.

### 3.2 Hazard (a) ∩ (b) — a refused pairing that *does* carry a cut dial

`validate_tool_for_operation` has a second arm: family `Parallel` (DropCutter)
with `target_scallop_mm.is_some()`. A flat end mill on a scallop-targeted
DropCutter is refused **and** carries a real `stepover`.

| | value |
|---|---|
| Panel | `Err(WrongToolForOperation { operation: Parallel, actual_geometry: Flat, … })` |
| Modal preview | WOC **0.1905 mm**, feed **2450.1 mm/min** |
| After `⚡ Apply all` | WOC **1.0 → 0.19 mm** (a **5.3× finer** stepover), feed **1000 → 2450** |
| After per-field WOC `Apply` | WOC **1.0 → 0.1905 mm** |

So the modal does not merely offer a refused recipe — on a refused pairing it
rewrites the cut geometry to a 5× denser raster, which is also a runtime
multiplier on an op the engine says cannot be run at all.

Test: `hazard_ab_refused_pairing_with_a_geometry_dial_takes_the_write`.

### 3.3 Hazard (b) — `Apply all` changes cut geometry where the panel's speeds-apply does not

Fixture: the same flat end mill on a **Pocket** op — a *valid* pairing, so the
panel offers both its buttons. Same recommendation feeds both surfaces
(`explain().recommended` is bit-identical to `calculate()`; verified — see the
POCKET rows in the run log).

| | feed | plunge | RPM | **WOC** | **DOC** |
|---|---|---|---|---|---|
| before | 1000 | 500 | `None` | **2.0** | **1.5** |
| panel `⚡⚡ Apply recommended speeds` | 3000 | 794 | 18000 | **2.0** (unchanged) | **1.5** (unchanged) |
| modal `⚡ Apply all` | 3000 | 794 | 18000 | **2.222** | **1.27** |

The panel's button says on its own tooltip "Does not change the cut
(DOC/WOC)", and it keeps that promise. The modal's button — the one a user
reaches from the same recommendation, one click away — moves both. The user
has no way to tell from the modal that they just changed the cut.

Test: `hazard_b_modal_apply_all_moves_geometry_panel_speeds_apply_does_not`.

### 3.4 Hazard (c) — NEW: seven affordances bypass the invariant funnel

Not in the review's finding; found while censusing, and it is the most severe
result in this document.

Every `apply_*_to_op` entry point resolves the recommendation on a scratch
clone through `enforce_invariants` (`feeds/suggest.rs:768`) before copying the
requested subset into the real operation. That is where
`clamp_plunge_to_feed`, `clamp_stepover_to_diameter`,
`backoff_stepover_for_runtime`, `clamp_dpp_to_rigidity`,
`clamp_dpp_to_cutting_length`, `backoff_dpp_for_deflection` and
`recalibrate_feed_for_chipload` live, plus `round_suggestion_value`.

`apply_feeds_field` (`events/mod.rs:796-817`) writes `explain.recommended.*`
**directly**. None of those passes run. Measured on the Pocket fixture:

| field | modal per-field `Apply` (raw) | panel funnelled apply | ratio |
|---|---|---|---|
| Feed | 3000 | 3000 | 1.00× |
| Plunge | 793.75 | 794 | rounding only |
| WOC | 2.2224999999999997 | 2.222 | rounding only |
| **DOC** | **4.445 mm** | **1.27 mm** | **3.50×** |

The DOC row is the whole finding. `4.445 mm` is the calculator's raw axial
recommendation; `1.27 mm` is what survives the rigidity clamp, the
cutting-length clamp and the deflection back-off on a Ø6.35 flat end mill at
45 mm stickout. **The modal's smallest, most incidental affordance — a
`small_button("Apply")` in a comparison row — writes 3.5× the depth of cut the
engine's own back-off chain permits, on a surface whose only stated job is to
show you the recommendation.** Note `⚡ Apply all` writes the correct 1.27; the
hazard is specific to the six per-field buttons plus the explore-chart button.

Tests: `hazard_c_per_field_apply_writes_the_raw_preview_value` (bit-exact
structural proof that no funnel intervenes),
`hazard_c_per_field_doc_is_3x_the_funnelled_doc` (the number),
`panel_cut_geometry_apply_goes_through_the_invariant_funnel` (the contrast).

### 3.5 Project-wide reach

`⚡⚡ Apply all toolpaths` fans `ApplyFeedsAll` over every **enabled** toolpath
with no per-row refusal surfaced, because the loop calls the infallible path
per id. A refused pairing sitting anywhere in the project takes the write
silently, inside a batch the user believes they understood.

Test: `project_apply_all_reaches_a_refused_toolpath_silently`.

---

## 4. Why this shape exists (so the fix does not recreate it)

Three separate, individually reasonable decisions compose into the defect:

1. `explain` was written as a *chart and comparison* payload for the redesigned
   modal (`suggest.rs:667-670` says exactly that). Being infallible is correct
   for a chart — you want to draw the nomogram even for a pairing you would
   refuse to run.
2. W3.1 split the panel's apply into speed-only and cut-only for a real UX
   reason, and did the work properly in core (`ApplySubset`).
3. The modal's Apply buttons were added to a payload that was never meant to be
   applied from, and reached for the shortest available write (a direct
   setter) rather than the subset API that already existed one module away.

So the repair is not "add validation to the modal". It is that **the type a
preview surface holds must not be applicable at all** — which is precisely the
review's `FeedsPreview` / `ApplicableRecommendation` split.

---

## 5. Checkpoint I package

### 5.1 The funnel design (the review's target, made concrete)

```rust
// core: feeds::apply

/// Infallible, read-only. What a chart / nomogram / comparison row may hold.
/// Constructible from any FeedsInput, including refused pairings — that is
/// the point. Carries no method that writes to an OperationConfig.
pub struct FeedsPreview {
    explain: FeedsExplain,
    /// Some(e) when validate_tool_for_operation refused. UI MUST render this;
    /// see 5.4 for the wording question.
    refusal: Option<FeedsError>,
}

impl FeedsPreview {
    pub fn build(input: &FeedsInput<'_>) -> Self;           // never fails
    pub fn recommended(&self) -> &FeedsResult;              // display only
    pub fn refusal(&self) -> Option<&FeedsError>;
    /// The ONLY bridge from preview to write. `None` iff refused.
    pub fn applicable(&self) -> Option<ApplicableRecommendation<'_>>;
}

/// Exists only if validate_tool_for_operation succeeded. Not constructible
/// any other way (private field, no pub ctor).
pub struct ApplicableRecommendation<'a> { result: &'a FeedsResult, /* … */ }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApplyScope {
    Speeds,          // feed / plunge / RPM        — "how fast"
    CutGeometry,     // stepover / DOC             — "changes the cut"
    Both,
    Field(FeedsField),  // one dimension — see Q2
}

/// The single write. Every caller — panel, modal, project batch, CLI — goes
/// through here, and it always runs enforce_invariants.
pub fn apply(
    rec: &ApplicableRecommendation<'_>,
    scope: ApplyScope,
    op: &mut OperationConfig,
    prov: &mut FeedsProvenance,
    ctx: ApplyContext<'_>,       // tool / machine / material / pass_role / SuggestContext
) -> Vec<SuggestWarning>;
```

Mechanically this is small: `apply_feeds_subset` already **is** the funnel;
`ApplySubset` already **is** `ApplyScope` minus `Field`. The work is (a)
promoting `ApplySubset` to `pub`, (b) adding the `Field` arm, (c) making
`FeedsExplain` unreachable as a write source, and (d) rerouting the eleven
modal affordances. The type system then does the enforcement: a surface that
holds only a `FeedsPreview` **cannot** write, and a write **cannot** skip
`enforce_invariants`, because there is one function and it always calls it.

Constraint honoured: nothing above locks, disables, or auto-populates a numeric
field. The Feeds tab's contract — **explicit Suggest buttons only, never
background field locking** — is untouched; the change is *what a button is
allowed to do when clicked*, not whether a field is editable.

### 5.2 The short-term question — three options, with a recommendation

**Q1. What happens to the modal's eleven write affordances in A-4?**

| Option | What it does | Cost | Risk |
|---|---|---|---|
| **A. Remove** all modal write affordances; the modal becomes purely explanatory and the panel is the only place to apply. | −11 buttons, −4 `AppEvent` variants, −5 handlers. Smallest possible surface. | Loses the project-wide batch apply (M10/M11), which has no panel equivalent and is genuinely useful. | Removes a working feature (project batch) to fix a different one. |
| **B. Reroute** every modal affordance to the funnel with an explicit scope; per-field buttons become `ApplyScope::Field`; the geometry-touching ones gain the panel's "changes the cut" attribution; refused pairings render the refusal in place of the Apply column. | Keeps every capability, fixes all three hazards. ~11 call sites + the `Field` arm. | More code than A. | Preserves the two-surface duplication the review dislikes — but now with *one* contract behind it. |
| **C. Hybrid** — reroute the batch paths (M7, M9, M10, M11) and the explore apply (M8); **remove** the six per-field buttons (M1–M6). | Fixes hazard (c) by deletion rather than by plumbing a `Field` scope. | Loses per-field granularity in the modal. | The per-field buttons are the ones with the 3.5× defect; deleting them is the cheapest way to be certain. |

**Recommendation: C, then B's guarantees on what remains.** Rationale:

- The review's own instruction is "do not preserve a legacy all-fields write
  merely for UI muscle memory", and the per-field buttons are exactly that —
  they exist because the compare row had a spare column, not because a
  measured need was answered. §3.4 shows they are also the *only* affordances
  with the funnel-bypass defect.
- The batch paths (M10/M11) have no panel equivalent and a real use (a
  project-wide feeds pass). Deleting them under option A would be scope
  creep dressed as safety.
- Under C the `ApplyScope::Field` arm is not needed at all in A-4, which
  removes the design's one genuinely new piece. It can be added later if a
  measured need appears.

If the operator prefers to keep per-field granularity, option B is a complete
fix too — it is strictly more work and strictly more surface, not more risk.

### 5.3 Remaining questions for the ruling

**Q2.** Does `ApplyScope` get a `Field(FeedsField)` arm? (Needed for B, not for
A or C.) A per-field apply through the funnel has an awkward semantic: the
funnel resolves the *whole* operating point on a scratch clone and copies back
one dimension, so applying Feed alone can yield a different number than
applying Feed after DOC. That is arguably more correct than today's raw write
and arguably more surprising. **Only rule this if Q1 = B.**

**Q3.** What does a refused pairing render in the modal? Three sub-options:
(i) refuse to open the modal at all (matches the panel, loses the diagnostic
value of seeing *why*); (ii) open it, draw the charts, replace the whole Apply
column with the refusal text (recommended — the modal's explanatory job is
preserved and the write is impossible); (iii) open it, keep Apply enabled, warn
(rejected — this is today's behaviour with a label).

**Q4.** Do the three optimizer writes (§2c, O1–O3) join the funnel, or are they
explicitly excluded with a stated reason? They are simulation-backed and
gate-scored, so they legitimately answer to a different validator — but O2
(`reoptimize_with_axis_override`) is a raw `set_*` on `Stepover` /
`DepthPerPass` / `ScallopHeight` with no clamp at all, which is hazard (c) in a
second neighbourhood. Recommendation: exclude O1/O3 (their candidates are
sim-verified end to end) and route **O2** through the funnel, since it is a
single-dial write of an *un*-simulated suggested value.

**Q5.** Does the agent/MCP surface (§2f) get an apply tool under the new
funnel, or stay deliberately manual via `set_toolpath_param`? Today an agent
has strictly weaker guarantees than either GUI surface. Not blocking for A-4;
worth ruling now because the funnel is the cheap moment to add it.

**Q6.** Do the A-3 characterization tests get inverted in place (same file,
assertions flipped, the measured pre-fix numbers preserved in the doc comments)
or moved to a new sentry file with A-3's kept as history? Recommendation:
inverted in place — §0.1 requires the pre-fix reproduction to stay permanently,
and it is already in the doc comments.

### 5.4 Pre-registered bars for A-4 (so the fix is checkable)

1. All nine tests in `apply_contract_a3.rs` go **red** against the fix, and are
   inverted — a green run against unchanged tests would mean the fix did not
   reach production code.
2. **No recipe number moves** on a valid pairing applied through P1/P2: the
   Pocket fixture's `3000 / 794 / 18000 / 2.222 / 1.27` must be byte-identical
   before and after. A moved fingerprint is a STOP (§0.2).
3. The DOC gap in §3.4 closes to **1.00×** — the surviving modal write paths
   produce exactly what the panel produces.
4. No numeric field acquires background locking or auto-population (standing
   constraint).

---

## 6. Artifacts and reproduction

| Artifact | Path |
|---|---|
| Characterization tests (9, green at `675a643`) | `crates/rs_cam_viz/tests/apply_contract_a3.rs` |
| This census | `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` |
| Measured operating points (§3.1–3.4) | reproduced by the tests; the raw dump was a scratch test, deleted after capture (its numbers are transcribed above and re-asserted by the permanent tests) |

```
cargo test -p rs_cam_viz --test apply_contract_a3 -q
# 9 passed
```

Machine discipline: `free -g` + bracketed `pgrep -af "carg[o]"` before every
launch; no concurrent Cargo job; no release build; per-crate tests only. The
core suite was not run by this wave (no core code touched); the known
pre-existing red cell `literature_matrix::flat_3mm_pocket_ipe_extreme`
(intake G-LIT-IPE, routed to A-6) was therefore not encountered.
