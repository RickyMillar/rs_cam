# RESEARCH — the smoke-corpus instrument: three suspected defects

**Lane:** read/test-only research (no production code changed, nothing staged).
**Binary under test:** `tech-debt-3` @ `1d6dd855`, **debug** build of `rs_cam_cli`.
**Scratch artifacts:** `/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/fe062f6e-ba96-4d15-8a99-f12da437e66d/scratchpad/corpus_lane/`
**Follow-ups addressed:** `DELTA_w5b_f3_corpus.md` F3-1 (a), F3-3's blocking question (b), F3-2 (c).

Headline results:

| # | Claim under test | Verdict |
|---|---|---|
| a | AS015 has been `generation_failed` since `4b105dab` | **REPRODUCED**, exact refusal path root-caused, minimal repair identified and sized |
| b | 211 baseline rapid collisions are 0 today — is the detector blind? | **DETECTOR IS ALIVE** — forced fixture reads **35** collisions at HEAD against a **0** control. Attribution of the 211→0 drop is *partly* settled: **both named candidates (G-SIM-IDENTITY-FRAME, G-EXPORT-DATUM) are DISCONFIRMED for this corpus** |
| c | `run_diff`'s chipload check is vacuous; drill columns undiffed | **REPRODUCED and worse than stated** — 5 injected regressions, **1** detected |

---

## 0. How the corpus runner actually works (shared background)

`crates/rs_cam_cli/src/smoke.rs` — one `ProjectSession` per case row:

1. `run_single_case` (`smoke.rs:385`) loads `case.project_template`, overrides stock
   material from `material_family`, applies `stock_*` params via
   `apply_stock_overrides` (`smoke.rs:788`), and **disables every toolpath already in
   the template** (`smoke.rs:439-441`) so the sim only sees what the runner adds.
2. For each id in `prior_passes`, `materialize_case_toolpath(…, StockSource::Fresh, …)`
   (`smoke.rs:463-468`) — which **adds *and generates*** the toolpath (`smoke.rs:345`,
   `smoke.rs:373`).
3. The measured case is then materialised with
   `StockSource::FromRemainingStock` when `prior_passes` is non-empty
   (`smoke.rs:486-496`) — again **add *and generate* in the same call**.
4. Only then is `run_simulation` called, once (`smoke.rs:522-542`), at the CLI's
   `--resolution` (default `0.5`).

Two structural facts that matter for all three findings:

* **The runner supports row selection only through `--input`.** There is no
  `--case`/`--filter` flag (`main.rs:253-283`); the `Smoke` subcommand takes
  `--input`, `--output`, `--diff`, `--baseline`, `--resolution`. Selecting rows =
  writing a subset CSV. That is how every run below was scoped.
* **Every toolpath the runner adds uses `HeightsConfig::default()`**
  (`smoke.rs:330`) — all five heights `Auto`. So the corpus cannot express a
  user-pinned retract; heights are always derived from the stock. This is why a
  forced-collision case (b) had to be built as a *project*, not a corpus row.

---

## 1. (a) AS015 — the rest-chain row

### 1.a Reproduced

Subset CSV (`cases_subset.csv`) = header + rows `AS007, AS009, AS010, AS013, AS015,
AS017` from `planning/toolpath_acceptance/cases_agent_smoke.csv` (AS013 must be
present: it is AS015's `prior_passes`).

```
$ cargo run -p rs_cam_cli -- smoke \
    --input  <scratch>/cases_subset.csv \
    --output <scratch>/subset_today.csv \
    --resolution 0.5
```

Result row, verbatim from `subset_today.csv`:

```csv
AS015,scallop,generation_failed,,,,,,,,,,,,,,"Operation failed: 'scallop smoke' is
set to use remaining stock (rest machining) but no simulated remaining-stock
snapshot is available. Run a simulation of the preceding operations first, then
regenerate — or set the stock source to Fresh if this is the first operation.
(Refusing to fall back to fresh stock: a fine tool would clear the whole part
instead of the leftover.); param_warnings="
```

(The doc's quoted error matched exactly. AS013, the prior pass, generates fine —
`AS013,adaptive3d,ok,…`, so the failure is specific to the measured op.)

### 1.b The exact refusal path

`ProjectSession::generate_toolpath` — `crates/rs_cam_core/src/session/compute.rs:1329-1350`:

```rust
if tc.stock_source == StockSource::FromRemainingStock
    && self.simulation.as_ref()
           .and_then(|sim| sim.prior_stocks.get(&tc.id))
           .is_none()
{ return Err(SessionError::OperationFailed(…)); }
```

The predicate is `prior_stocks` **keyed by the toolpath's own id**. It is checked
*before any geometry work* (deliberately — fail fast, never fall back to fresh
stock). That block landed in `4b105dab` (2026-07-06), listed in its own commit
message as *"FromRemainingStock silent fresh-stock fallback (now fail-hard)"*.

`prior_stocks` is populated **only inside `run_simulation`**
(`compute/simulate.rs:917-923` per carved entry, `:1129-1135` for the group tail).
The runner never simulates between step 2 and step 3, so at the moment it calls
`generate_toolpath` on AS015 the map is empty for AS015's id. **The refusal is
correct; the runner is stale.** Not attributable to swept, not attributable to
anything in the perf programme.

### 1.c The repair shape — and why "just add a simulation" is not quite enough

Naively inserting `run_simulation` between the prior pass and the measured case
would **still fail**: at that moment AS015's `ToolpathConfig` does not exist, so
nothing would key a snapshot to its id.

The core already has the mechanism for this exact catch-22 — the **phantom prior
stock** slot (F.4). `PhantomPriorStockScan` (`compute/simulate.rs:116-170`) walks a
setup's configs in plan order and locks onto the **first enabled-but-ungenerated**
config; if that config is `FromRemainingStock`, its id gets a `prior_stocks`
snapshot taken at its plan position (`simulate.rs:918-923`, `:1129-1135`;
request-side at `session/compute.rs:2240-2250`; unit-pinned by
`phantom_prior_stock_populates_pending_op_snapshot`, `simulate.rs:2092`).

So the required order is **add → simulate → generate**, not simulate → add → generate:

```
1. add + generate every prior pass (StockSource::Fresh)         [unchanged]
2. ADD the measured toolpath (FromRemainingStock) — do NOT generate
3. run_simulation(&sim_opts)   → phantom slot fills prior_stocks[AS015]
4. generate_toolpath(measured)  → precondition now satisfied
5. run_simulation(&sim_opts)   → the measurement pass            [unchanged]
```

This is a strictly smaller change than an MCP-style fixpoint loop, and it is
sufficient here: `prior_passes` is a **flat list of ids resolved against the same
suite** (`smoke.rs:446-452`), never a transitive chain, so the chain depth is
always 1. A fixpoint loop would buy nothing and would need a convergence rule the
corpus does not need.

**On resolution.** The concern that "the corpus would need a pinned cell size" is
already half-satisfied: `--resolution` exists and defaults to `0.5`, and steps 3
and 5 must share **one** `SimulationOptions` value so the priming and measurement
grids agree. What is genuinely missing is that **the baseline CSV records no
resolution** — `BaselineRow` (`smoke.rs:99-117`) has no such column, so a baseline
cut at 0.5 and a run at 0.25 diff as if they were comparable. Recommend adding it
(see §4.1).

### 1.d A correction the repair must carry

AS015's `2026-06-04` baseline row (`ok`, `chipload exceeds_low 0.002008`,
`deflection within 0.1204`, `avg_engagement 0.0640`, `peak_axial 3.488`) was
produced **while `FromRemainingStock` silently fell back to fresh stock** — that
fallback is precisely what `4b105dab` removed. So the old AS015 row is a
*fresh-stock* scallop measured under a *rest-stock* label. Once the chain works,
the new numbers will differ and that is **not a regression**: AS015's baseline row
must be **re-cut, never diffed** against the old one.

---

## 2. (b) The 211 → 0 rapid collisions

### 2.a Where the number comes from

* Detector: `collision::check_rapid_collisions_against_stock`
  (`crates/rs_cam_core/src/collision.rs:450-545`). Per `MoveType::Rapid` move it
  samples ~1 mm steps and flags `pz < z_grid.top_z_at(row,col)`. Two carve-outs:
  pure-vertical retracts (`collision.rs:474`) and the F3 same-XY peck re-entry
  (`collision.rs:483-511`, added `cbd8785d`, 2026-05-10 — **before** the baseline).
* Call site: `compute/simulate.rs:925-934`, once per toolpath, against
  `group_stock.z_grid` — the **per-setup** stock *before* this toolpath carves.
* Publication: `session.diagnostics().per_toolpath[].rapid_collision_count`, read by
  `smoke.rs:564-568` and — the **same core field** — by the CLI `project` report
  (`project.rs:141-142`). The two front ends therefore measure the identical
  quantity from the identical producer.

Baseline `2026-06-04` totals: AS007 6, AS009 1, AS010 104, AS017 100 = **211**.

### 2.b Today's reading (reproduced)

From `subset_today.csv` — every `rapid_collision_count` is `0`:

| case | 2026-06-04 | today | chipload then → now | peak_axial then → now |
|---|---|---|---|---|
| AS007 trace | 6 | **0** | unmodeled → unmodeled | 0.167 → 0.167 |
| AS009 chamfer | 1 | **0** | exceeds_low → within | 0.541 → 1.100 |
| AS010 inlay | 104 | **0** | exceeds_low → within | 0.542 → 1.763 |
| AS013 adaptive3d | 0 | 0 | exceeds_low → within | 14.766 → 3.743 |
| AS017 horizontal_finish | 100 | **0** | unmodeled → unmodeled | 16.504 → 11.594 |

### 2.c THE VITAL RESULT — the detector has a population at HEAD

The corpus cannot express a diving rapid (all heights are `Auto`, §0), so the probe
was built as a project pair, identical in every respect except one height:

* `collide_control.toml` — copy of `planning/review_2026-08-08/artifacts/a8i/a8i_pocket.toml`
  (one pocket, `demo_pocket.svg` = rounded rect + circular island; stock top at world
  Z=0), all five heights `auto`.
* `collide_forced.toml` — identical, except
  `[setups.toolpaths.heights.retract_z] mode = "manual", value = -3.0`, which parks
  every link/retract rapid 3 mm **below** the stock top.

```
$ cargo run -p rs_cam_cli -- project <scratch>/collide_control.toml \
      --output-dir <scratch>/out_control --resolution 0.5 --summary \
      --no-adaptive-feed-modulation
  [ ] #1 A8i Pocket (Pocket) — 2032 moves, 0 collisions
  Verdict: OK

$ cargo run -p rs_cam_cli -- project <scratch>/collide_forced.toml \
      --output-dir <scratch>/out_forced  --resolution 0.5 --summary \
      --no-adaptive-feed-modulation
  [!] #1 A8i Pocket (Pocket) — 2008 moves, 35 collisions
  Verdict: WARNING: 35 rapid-through-stock collisions
```

Per-toolpath JSON:

| | control | forced |
|---|---|---|
| `move_count` | 2032 | 2008 |
| `cutting_distance_mm` | 2305.662 | 2209.662 |
| `rapid_distance_mm` | 1682.931 | 866.931 |
| **`rapid_collision_count`** | **0** | **35** |

**The rapid-collision detector is not blind at `1d6dd855`.** It is live, it is wired
to the same field the corpus reads, and it flags a dive the moment one exists. The
`0` on the corpus is the absence of the phenomenon, not the absence of the
instrument.

### 2.d Attribution of the drop — what is settled and what is not

**Both candidates named in the brief are disconfirmed for this corpus.**

* **G-SIM-IDENTITY-FRAME (`ff3696fd`)** changed the **global/playback** stock frame.
  Its own commit message states the collision check was *not* on the affected
  object — *"per-toolpath metrics, engagement and collision checks all read the
  correctly-framed per-group stock"* — and that reproducing it required a fixture
  that is **both mixed-setup and non-zero-origin**, which *"every existing fixture"*
  is not. Checked directly: all four corpus templates
  (`ux_2d_star`, `ux_2d_pocket`, `ux_3d_terrain`, `ux_step_stepped`) carry exactly
  **one** `[[setups]]`, `face_up = "top"`, `z_rotation = "0"` — identity, single
  setup. The fix cannot have touched them.
* **G-EXPORT-DATUM (`0bb38a2f`)** touches `gcode/mod.rs`, `eval_context.rs`,
  `viz/io/export.rs`, `mcp` — **no simulator file at all** — and its own message says
  *"Unchanged: origin==0 projects"*. Two of the four corpus templates are
  `origin_z = -12`, but the change is export-side; the simulator never reads it.

**What the evidence does support** (convergent, medium-high confidence):

1. The **grid framing for these fixtures has been stable across the whole window.**
   For identity single-setup projects `SetupEvalContext::sim_local_stock_bbox()`
   returns `None` (`session/eval_context.rs:182-188`), so the per-setup dexel grid is
   the **world** stock bbox. That is the F-024 shape, dated 2026-05-25 — *before* the
   2026-06-04 baseline. `git log -S"local_stock_bbox"` since the baseline returns
   only `7cee538c` (advisor) and `2ed9df04` (S5 memoisation), neither of which
   re-frames the grid.
2. **No move-type reclassification is masking the detector.** The detector only
   inspects `MoveType::Rapid`; a fix that turned diving rapids into feeds would zero
   the count while the dive survived. The W6 census sentry
   `retract_intent_move_type_census_w6` (`ed31d789`) measured 24 op configs × 2
   dressup profiles, 242,790 moves, 812 Retract-tagged, **zero** Retract-tagged
   Linear feeds. That escape route is closed at HEAD.
3. **The detector is alive** (§2.c).

⇒ The most probable reading is that the 211 were **real diving rapids that later
stopped being emitted** — i.e. a genuine behavioural improvement somewhere in the
2026-06-04 → HEAD window. Note the corpus's own supporting signal: AS009 and AS010
both changed *cut geometry* over the window (`peak_axial_doc_mm` 0.541→1.100 and
0.542→1.763), which is consistent with their emitters having moved, and `4b105dab`
carries a `vcarve/inlay/chamfer` frame fix touching exactly those two op families.

**Confidence: medium-high that the detector is sound and the drop is behavioural;
LOW on naming the specific commit.** A candidate that is *not* excluded: the ops'
Z frames moved for reasons unrelated to rapids and the rapids followed. The
experiment that would settle it is small and was deliberately not run here (it
needs a build at an old commit, which means touching git state while another
session holds the tree): build `rs_cam_cli` at `4b105dab^` and at `4b105dab`, run
the 4-row subset `AS007,AS009,AS010,AS017` at `--resolution 0.5`, compare
`rapid_collision_count`. ~15 min of machine time, no ambiguity in the answer.

**Recommendation for F3-3:** the blocking condition on re-cutting the baseline
("confirm the 211→0 is a fix and not a blind detector") is **discharged** by §2.c —
but discharge it *permanently* by landing the forced-dive fixture as a sentry
(§4.2), not by trusting this one-off run.

---

## 3. (c) `run_diff` coverage — the honest table

### 3.1 The predicates

`smoke.rs:239-245`:

```rust
fn is_within(kind: &str) -> bool { kind == "within" }
fn is_exceeds(kind: &str) -> bool { kind == "exceeds" }
```

Emitters:

| column | emitted spellings | site |
|---|---|---|
| `chipload_kind` | `within` · `exceeds_low` · `exceeds_high` · `unmodeled_*` · `missing` | `smoke.rs:588-606`, `598` = `format!("exceeds_{}", side_str(side))` |
| `deflection_kind` | `within` · **`exceeds`** · `unmodeled_*` · `missing` | `smoke.rs:608-620` |
| `power_kind` | `within` · **`exceeds`** · `unmodeled_*` · `missing` | `smoke.rs:622-634` |
| `drill_*_kind` | `within` · `exceeds_elevated` · `exceeds_critical` | `smoke.rs:680-703`, `686`; severities at `tool_load/drill_gates.rs:80-83` |

`is_exceeds` matches **only** the bare spelling, which only deflection and power
produce. So the chipload arm is vacuous, and — a detail the brief did not have —
even a naive `exceeds_low|exceeds_high` fix would still miss the **drill**
spellings, which are `exceeds_elevated` / `exceeds_critical`.

### 3.2 Measured, not argued

Two synthetic baselines (`diff_base.csv` / `diff_curr.csv`, 5 rows) with **five
deliberate regressions**, one of which is a positive control:

| row | injected change | should flag? |
|---|---|---|
| X01 | `chipload_kind` `within` → `exceeds_high` (observed 0.01 → 0.50) | yes |
| X02 | all three drill gates `within` → `exceeds_critical` (D/d 4.0 → 99.0) | yes |
| X03 | `deflection_kind` `within` → `exceeds` (**positive control**) | yes |
| X04 | `chipload_kind` `unmodeled_novendordata` → `exceeds_high`; `avg_engagement` 0.30 → 0.00; `peak_axial` 2.0 → 0.0 | yes |
| X05 | `op_kind` `zigzag` → `pocket`; chipload observed 0.01 → 0.90; deflection peak 0.01 → 9.90; power peak 0.01 → 9.90 | yes |

```
$ cargo run -p rs_cam_cli -- smoke --diff --baseline diff_base.csv --output diff_curr.csv
smoke-diff: 1 regression(s)
  - X03: deflection within → exceeds
```

**1 of 5.** The chipload flip, all three drill gates, the gate that went blind and
then blew up, the collapsed engagement, the 990× peak moves, and the changed
`op_kind` are all silent.

The real-data run agrees. Baseline subset (6 rows of `2026-06-04`) vs
`subset_today.csv`:

```
$ cargo run -p rs_cam_cli -- smoke --diff --baseline baseline_subset.csv --output subset_today.csv
smoke-diff: 1 regression(s)
  - AS015: status ok → generation_failed
exit=1
```

The **211→0 collision drop**, three `exceeds_low → within` chipload flips, and
`peak_axial_doc_mm` 14.766 → 3.743 on AS013 are all invisible to the net.

### 3.3 The coverage table

All 17 `BaselineRow` columns (`smoke.rs:99-117`):

| # | column | status under `run_diff` | detail |
|---|---|---|---|
| 1 | `case_id` | **partial** | used as the join key; a *removed* case reports `missing from current run` (`:182-185`); an *added* case is informational by design (`:223`) |
| 2 | `op_kind` | **silently ignored** | never read. A case can change operation family without a word |
| 3 | `status` | **partial** | only `ok → non-ok` (`:187-192`). `non-ok → different non-ok` (e.g. `generation_failed → harness_error`) is silent; `non-ok → ok` is correctly not a regression |
| 4 | `chipload_kind` | **VACUOUSLY COMPARED** | `:195` — `is_exceeds` can never match `exceeds_low`/`exceeds_high` |
| 5 | `chipload_observed_mm_tooth` | **silently ignored** | the 4.0×–14.7× shift across the corpus was invisible |
| 6 | `deflection_kind` | **works** | `:201`, bare `exceeds` |
| 7 | `deflection_peak_mm` | **silently ignored** | AS017's 1.2855 → 0.0453 (28×) was invisible |
| 8 | `power_kind` | **works** | `:207` |
| 9 | `power_peak_kw` | **silently ignored** | |
| 10 | `rapid_collision_count` | **works, one-sided** | `:214-220`, `curr > base`. A *decrease* is treated as an improvement — which is exactly how 211→0 passed unremarked. `parse().unwrap_or(0)` also silently maps an empty/garbage cell to `0` |
| 11 | `avg_engagement` | **silently ignored** | |
| 12 | `peak_axial_doc_mm` | **silently ignored** | AS013 14.766 → 3.743 invisible |
| 13 | `drill_chip_welding_kind` | **silently ignored** | never read; would *also* be vacuous under a chipload-only fix (`exceeds_elevated`/`exceeds_critical`) |
| 14 | `drill_chip_welding_observed` | **silently ignored** | AS011's 4.000 → 1.000 (R-plane rooting) invisible |
| 15 | `drill_peck_kind` | **silently ignored** | |
| 16 | `drill_plunge_kind` | **silently ignored** | |
| 17 | `notes` | **silently ignored** | acceptable — `param_warnings` churn is noise |

Score: **3 columns work** (one of them one-sided), **1 is vacuously compared**,
**11 are silently ignored**, **2 are partial**.

### 3.4 A class the table makes visible: the gate that goes blind

`is_within(base) && is_exceeds(curr)` is the only transition tested, so
`within → unmodeled_*` and `within → missing` are **not** regressions. A gate that
stops measuring — the exact "empty population passes and looks healthy" class
`CLAUDE.md` names — is invisible on all four verdict columns. X04 above demonstrates
the mirror image (`unmodeled → exceeds_high`, also silent).

---

## 4. Three fix proposals

### 4.1 — Repair the `prior_passes` chain (fixes AS015; F3-1)

**Change.** In `crates/rs_cam_cli/src/smoke.rs`:

* Split `materialize_case_toolpath` (`:260-383`) so the `generate_toolpath` tail
  (`:372-380`) is behind a `generate: bool` parameter (or extracted into a small
  `generate_case_toolpath(session, tp_idx, case, op_type, &param_warnings)` that
  returns the same `BaselineRow` failure shape).
* Hoist the `SimulationOptions` construction (`:522-534`) above the measured-case
  materialisation so one value serves both simulations.
* In `run_single_case`, for the chained branch only:

```rust
// measured case: ADD, do not generate yet
let (tp_idx, op_type, mut param_warnings) = materialize_case_toolpath(
    &mut session, case, measured_stock_source, "smoke",
    /* generate = */ prior_ids.is_empty(),
)?;

if !prior_ids.is_empty() {
    // Priming pass: the measured op is enabled-but-ungenerated and
    // FromRemainingStock, so PhantomPriorStockScan gives it a
    // prior_stocks snapshot taken at its plan position.
    if let Err(e) = session.run_simulation(&sim_opts, &cancel) {
        return BaselineRow::failure(&case.case_id, op_type.kind_str(),
            "simulation_failed", &format!("priming sim: {e}"));
    }
    if let Err(e) = session.generate_toolpath(tp_idx, &cancel) {
        return BaselineRow::failure(&case.case_id, op_type.kind_str(),
            "generation_failed", &format!("{e}; param_warnings={}",
            param_warnings.join("|")));
    }
}
```

* Separately (small, independent): add a `resolution` column to `BaselineRow`
  (`:99-117`) written from the `--resolution` argument, and have `run_diff` refuse —
  or at minimum print a loud warning — when the two files disagree.

| | |
|---|---|
| Files | 1 (`crates/rs_cam_cli/src/smoke.rs`). +1 header cell in `planning/toolpath_acceptance/baselines/*` if the resolution column lands |
| LOC | ~35 for the chain repair; ~15 more for the resolution column |
| Risk | **low** — CLI harness only, no core change. Unchained rows (17 of 18) take the identical code path they take today; the new branch is reachable only when `prior_passes` is non-empty |
| Cost | one extra `run_simulation` per chained row (currently 1 row). AS015's case took ~65 s end-to-end in the failing run; expect roughly double |
| **Moves a metric?** | **YES — user decision required.** AS015 will produce a row where it produces none today, and that row is **not** comparable to the `2026-06-04` one (§1.d: the old row was measured under the fresh-stock fallback that `4b105dab` deleted). AS015's baseline must be re-cut with a note, not diffed |

### 4.2 — Pin the collision detector's population (unblocks F3-3)

**Change.** Land the §2.c fixture pair as a sentry — a `#[test]` under
`crates/rs_cam_core/tests/` that builds a pocket session with
`heights.retract_z = HeightMode::Manual(stock_top - 3.0)`, simulates, and asserts
`rapid_collision_count > 0`; plus the `Auto` control asserting `== 0`. Both
assertions matter: the control is what makes the positive one mean something.

| | |
|---|---|
| Files | 1 new test file (~90 LOC), or ~50 LOC appended to an existing sim test |
| LOC | ~90 |
| Risk | **very low** — test-only, no production code |
| **Moves a metric?** | **No.** Nothing shipped changes |

Note this fixture belongs in `rs_cam_core/tests`, **not** in the corpus CSV: the
corpus runner cannot express a pinned retract (§0), so a corpus row could not carry
it without also changing `BaselineRow`/`SmokeCase`.

### 4.3 — De-vacuum `run_diff` (F3-2)

**Change**, all in `crates/rs_cam_cli/src/smoke.rs`:

```rust
// :239-245 — replace
fn is_within(kind: &str) -> bool { kind == "within" }
fn is_exceeds(kind: &str) -> bool { kind.starts_with("exceeds") }   // was: == "exceeds"

/// A verdict that stopped being measured. `within → unmodeled_* / missing`
/// is a regression of the INSTRUMENT even when the machining is fine.
fn went_blind(base: &str, curr: &str) -> bool {
    base == "within" && (curr.starts_with("unmodeled") || curr == "missing")
}
```

`starts_with("exceeds")` covers all four emitters in one predicate: bare `exceeds`
(deflection, power), `exceeds_low`/`exceeds_high` (chipload), and
`exceeds_elevated`/`exceeds_critical` (drill).

Then, in the per-case loop (`:195-220`), drive every verdict column through one
helper instead of three hand-copied blocks:

```rust
for (label, b, c) in [
    ("chipload",            &base_row.chipload_kind,            &curr_row.chipload_kind),
    ("deflection",          &base_row.deflection_kind,          &curr_row.deflection_kind),
    ("power",               &base_row.power_kind,               &curr_row.power_kind),
    ("drill_chip_welding",  &base_row.drill_chip_welding_kind,  &curr_row.drill_chip_welding_kind),
    ("drill_peck",          &base_row.drill_peck_kind,          &curr_row.drill_peck_kind),
    ("drill_plunge",        &base_row.drill_plunge_kind,        &curr_row.drill_plunge_kind),
] {
    if is_within(b) && is_exceeds(c) {
        regressions.push(format!("{case_id}: {label} {b} → {c}"));
    } else if went_blind(b, c) {
        regressions.push(format!("{case_id}: {label} STOPPED BEING MEASURED {b} → {c}"));
    }
}
```

Two further predicates worth landing with it (each independently defensible):

* **`status` completeness** — flag `non-ok → different non-ok` as informational, so a
  `generation_failed → harness_error` slide is visible.
* **Numeric drift as an informational channel, not a regression.** Print a
  `smoke-diff: N change(s)` block for `chipload_observed_mm_tooth`,
  `deflection_peak_mm`, `power_peak_kw`, `avg_engagement`, `peak_axial_doc_mm`,
  `drill_chip_welding_observed` (relative tolerance, e.g. >5 %) and for a
  **decrease** in `rapid_collision_count`. Exit status unchanged. This is the
  channel whose absence let 211→0 and a 28× deflection drop pass without comment —
  and it is deliberately *not* a regression, because both of those were
  improvements; the defect is that nobody was told.

| | |
|---|---|
| Files | 1 (`crates/rs_cam_cli/src/smoke.rs`) |
| LOC | ~45 for the predicate fix + drill columns + `went_blind`; ~40 more for the informational numeric channel |
| Risk | **low-to-moderate.** No production behaviour changes and no metric moves, **but the net gets stricter**: any current baseline containing a `within → exceeds_low/high` or drill flip will start failing CI the day it lands. Recommend landing it **before** re-cutting the baseline, then cutting the new baseline against the fixed net — so the first green is a real green |
| **Moves a metric?** | **No** — but it changes which runs are *called* regressions, which is a user-visible policy change and should be flagged in the commit |

**Recommended order:** 4.3 → 4.2 → 4.1 → re-cut the baseline (F3-3). Fixing the net
first means the re-cut baseline is validated by an instrument that works; landing
the AS015 repair before the re-cut means AS015 has a row to record.

---

## 5. Corrections to the record

Two claims in `DELTA_w5b_f3_corpus.md` / `BASELINES.md` that this lane did not
sustain, offered for the corrections ledger:

1. **§2.d's speculation that the 211→0 drop is "*probably* the identity-frame and
   export-datum fixes"** — both are **disconfirmed** for this corpus (§2.d). The
   corpus's four templates are single-setup identity projects, which is the exact
   shape `ff3696fd` says it *cannot* affect, and `0bb38a2f` touches no simulator
   file. The doc was appropriately hedged and explicitly declined to investigate;
   this is a refinement of an open question, not a mis-statement.
2. **F3-1's repair sentence — "needs a `run_simulation` between the prior pass and
   the measured case"** — is *necessary but not sufficient*. Simulating before the
   measured toolpath is **added** leaves `prior_stocks` with nothing keyed to its
   id. The order must be **add → simulate → generate** so the F.4 phantom slot
   fires (§1.c).

Everything else reproduced exactly: AS015's error text, the vacuous chipload
predicate, and the drill columns being undiffed (which is in fact one instance of a
much larger silent-column set — §3.3).

---

## 6. Reproduction inventory

Scratch dir
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/fe062f6e-ba96-4d15-8a99-f12da437e66d/scratchpad/corpus_lane/`:

| file | what |
|---|---|
| `cases_subset.csv` | 6-row corpus subset (AS007/09/10/13/15/17) |
| `subset_today.csv` | today's smoke output for those rows |
| `baseline_subset.csv` | the matching 6 rows of `baselines/2026-06-04.csv` |
| `collide_control.toml` / `collide_forced.toml` | the collision fixture pair (§2.c) |
| `out_control/` / `out_forced/` | their `project` diagnostics (`summary.json`, `tp_1_A8i_Pocket.json`) |
| `control.log` / `forced.log` | their `--summary` output |
| `diff_base.csv` / `diff_curr.csv` | the 5-injected-regression `run_diff` probe (§3.2) |

All cargo invocations were `flock /tmp/rs_cam_cargo.lock cargo … -j 8` behind a
≥20 GB free-memory gate, debug profile, package-scoped. No tracked file was
modified, staged, or committed by this lane.
