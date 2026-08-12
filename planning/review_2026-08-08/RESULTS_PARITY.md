# B-5 — GUI-mode results parity

Wave: TD3 **B-5**, ledger row **G-RESULTS**
(`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4.7).
Date: 2026-08-13. Base revision `1c0a4ac` (`tech-debt-3`).
Artifacts: `artifacts/b5/` (driver + raw census JSON kept out of the repo —
see §1.3 for the scratch paths).

## 0. The claim under test, and what it turned out to be

> **G-RESULTS** — In **GUI mode** `ProjectSession.results` is never written,
> so `get_toolpath_diagnostics` / `get_tool_load_report` run **without
> generation findings and without spans** while the CLI's run with both.

Measured at `1c0a4ac`, on wanaka, through both surfaces. **The row is half
stale and half real, and the real half is a different read from the two it
names.**

| clause | verdict | evidence |
|---|---|---|
| "`ProjectSession.results` is never written" in GUI mode | **STALE** — it has been written since `d706c036` (2026-05-11) | `AppController::drain_compute_results` (`controller/events/compute.rs`) calls `ProjectSession::insert_result` on every successful drain. Live: `get_tool_load_report` returns populated `drill_gates` for both wanaka drill ops, and `drill_gates` populate **only** from `session.results[idx].drill_op()` (`gcode/mod.rs:504`) |
| "`get_toolpath_diagnostics` runs without generation findings" | **STALE** | `diagnose_toolpath_with_trace` reads `self.results.get(&idx)` for `stats` (`session/compute.rs:3675-3679`); live GUI run returns the full diagnostic list per toolpath, incl. `drill.*` and `load.*` |
| "`get_tool_load_report` runs without spans" | **STALE** | live GUI spans present on every generated op (`inspect_spans` kind counts, §2.3); the chipload gates return `within` with real `sample_range`, not `Unmodeled(SimulationRequired)` |
| the divergence itself | **REAL, on `get_diagnostics`** | `build_mcp_diagnostics` hand-built its per-toolpath rows and dropped **ten** channels the CLI publishes (§2.4) |

The row was written from a doc comment. `build_mcp_diagnostics` carried, at
the head of the function:

> Unlike `session.diagnostics()` which reads from the session's internal
> result cache (only populated by the standalone MCP), this reads from
> `gui.toolpath_rt` …

That sentence stopped being true three months before the ledger row quoted
it, and the code under it was never revisited. Rule 5 of §0 (instrument
integrity) in its documentary form: **a stale doc comment became a ledger
row, and the ledger row became this wave's brief.**

## 1. Method

### 1.1 Fixture

A **copy** of the operator's wanaka project
(`planning/airrun_2026-06-01/wanaka.toml` → scratch; the original is never
opened for write and never staged). 9 toolpaths, 7 enabled, spanning
`alignment_pin_drill`, `adaptive3d` ×2, `drill`, `project_curve` ×2,
`unified_finish`; 219 944-triangle terrain plus three DXFs. Simulation cell
**0.35 mm on both surfaces** — collision counts and engagement move with the
cell, so the census pins one value across both rather than comparing two.

### 1.2 The two surfaces

* **CLI** — `target/release/rs_cam_cli project <copy> --resolution 0.35
  --summary`. Adaptive feed modulation ON (Checkpoint K (g1) default).
  38 s wall clock.
* **GUI** — this wave's **own** `target/release/rs_cam_gui --mcp` instance,
  driven over stdio by `artifacts/b5/gui_census.py` (reusing B-1's
  dependency-free client). `load_project` → `generate_all(fixpoint: true,
  simulation_resolution_mm: 0.35)` → `run_simulation(0.35)` → the reads.
  486 s generate (3 rounds, 2 fixpoint simulations) + 21 s simulate; peak
  RSS 3.4 GB. The operator's live GUI was never touched.

### 1.3 Reads captured

`get_toolpath_diagnostics(i)`, `get_tool_load_report()`, `get_diagnostics()`,
`narrate_toolpath(i)`, `inspect_spans(i)`, `get_toolpath_params(i)` for every
index, written to `gui_census.json`. CLI side: `tp_*.json` + `summary.json` +
the triage report on stderr.

Both raw captures live in this session's scratch
(`…/scratchpad/b5/{cli_out,gui_run}`) and are **not** committed — the GUI
capture is 258 KB of project-specific trace. The driver is committed so the
run reproduces.

### 1.4 The one asymmetry the method could not remove

The CLI's `project` command emits its own JSON; it does not expose
`get_toolpath_diagnostics` / `get_tool_load_report` on a wire. So rows 1-3
of §2 are argued **structurally** (one shared core function, and the input
that used to differ is now proven equal) plus one decisive live observable
(`drill_gates`), not by a byte diff of two JSON files. Row 4 — the one that
diverged — is proven by both a live diff and a unit sentry.

## 2. Census — one row per field/finding family

`present` = the family reaches an agent through that surface.
`n/a` = the surface has no such wire.

| # | family | CLI (`project`) | GUI MCP, pre-fix | GUI MCP, post-fix | how measured |
|---|---|---|---|---|---|
| 1 | **generation findings → `get_toolpath_diagnostics`** (`geom.*` adapters off `ToolpathStats`) | present (via `session.results`) | **present** | present | `diagnose_toolpath_with_trace` reads `results[idx].stats`; live GUI returned per-toolpath diagnostic lists on all 7 generated ops |
| 2 | **spans → `get_tool_load_report`** (gate transit classification) | present | **present** | present | `project_load_report` reads `get_result(idx).annotated().spans`; live `inspect_spans` returns non-empty kind counts on all 7 (e.g. UnifiedFinish: `GeometryRefit 32002, LinkBridge 1049, Entry 191, RapidOrderBarrier 69, Region 109, Operation 1`) |
| 3 | **`DrillOp` → `drill_gates`** | present | **present** | present | live: `per_toolpath[tp 14].drill_gates` and `[tp 7].drill_gates` populated. Sole source is `session.results[idx].drill_op()` — **the decisive proof the store is wired** |
| 4 | **`op_kind`** on the project per-toolpath row | present | **ABSENT** | present | live row key set vs `ToolpathDiagnostic` wire |
| 5 | **`collision_count`** per toolpath | present | **ABSENT** | present | ditto |
| 6 | **`rapid_collision_count`** per toolpath | present | **ABSENT** | present | ditto |
| 7 | **`truncated_core_mm2`** (+ deprecated `standing_material_mm2`) | present | **ABSENT** | present | ditto |
| 8 | **`untouched_material_mm2`** | present | **ABSENT** | present | ditto |
| 9 | **`reached_uncut_estimate_mm2`** | present | **ABSENT** | present | ditto |
| 10 | **`unmachined_band_area_mm2`** (dropped band) | present | **ABSENT** | present | ditto |
| 11 | **`tip_float_points`** | present | **ABSENT** | present | ditto |
| 12 | **`max_tip_float_mm`** | present | **ABSENT** | present | ditto |
| 13 | **project `collision_count`** | measured (per-toolpath collision sweep) | **literal `0`** | from evidence | source read: `"collision_count": 0` was a constant in the JSON literal |
| 14 | **lane columns** (`status`, `error`, `awaiting_prior_stock`, `stale`, `toolpath_index`) | n/a — a batch run has no lane | present | present | GUI-only and kept; §3 |
| 15 | **`triage`** block | present (printed) | present | present | both call `ProjectSession::simulation_triage` |
| 16 | **`verdicts`** (structured `Vec<Verdict>`) | **ABSENT** (CLI writes only the `verdict` string) | **ABSENT** | ABSENT | symmetric gap; §4 R-3 |
| 17 | **`debug_trace` / `semantic_trace` on `session.results`** | present | **ABSENT** (`None` by design; viz keeps them on `gui.toolpath_rt`) | ABSENT | `controller/events/compute.rs:772-777`; §4 R-2 |
| 18 | **`ProjectSession.simulation`** | present | **ABSENT** (never assigned in GUI mode) | ABSENT | §4 R-1 — the structural half of G-RESULTS that remains |
| 19 | **rest-chain reachability** (which toolpaths have a result at all) | **3 of 7** | **7 of 7** | 7 of 7 | §4 R-4 — the largest measured results gap in the census, and it is **CLI-side** |
| 20 | Checkpoint K: `feeds.chipload_clamped_to_floor`, `load.chipload.commanded_above_band` | present | **present** | present | live GUI diagnostics on 5 and 3 toolpaths respectively |
| 21 | Checkpoint K: `ceiling_advisory`, `FeedExplanation::clamped_to` | — | — | — | **NOT EXERCISED**: neither fired on this fixture (0 occurrences in the GUI capture). Parity is structural — both are computed inside `tool_load::chipload` / `feeds`, reached through `gcode::project_load_report` and `feeds_result_for_toolpath`, which are the *same* call for both surfaces |

**Headline: 21 families censused, 11 differed** — ten dropped keys (rows
4-12, counting the `truncated_core_mm2`/`standing_material_mm2` pair as one
family) plus one unmeasured constant (row 13) — all of them on **one** read,
`get_diagnostics`, and none of them on the two reads the ledger row named.
Three further asymmetries (rows 17-19) are structural and are carried as
residuals, one of them against the CLI.

### 2.4 The dropped set, verbatim

From the red sentry at `4bfed4c`, before any fix:

```
MCP get_diagnostics drops ["collision_count", "max_tip_float_mm", "op_kind",
"rapid_collision_count", "reached_uncut_estimate_mm2",
"standing_material_mm2", "tip_float_points", "truncated_core_mm2",
"unmachined_band_area_mm2", "untouched_material_mm2"] from the core
ToolpathDiagnostic wire
```

and the live pre-fix row from the wanaka run, complete:

```json
{"awaiting_prior_stock": null, "cutting_distance_mm": 74.0, "error": null,
 "move_count": 68, "name": "Pin Drill", "operation_type": "Pin Drill",
 "rapid_distance_mm": 912.116, "stale": false, "status": "Done",
 "tool_name": "End Mill", "toolpath_id": 14, "toolpath_index": 0}
```

**Why an absent key is worse than a `null`.** Every one of rows 7-12 carries
the X-19 contract: `null` means *not measured*, `Some(0.0)` means *measured
and clean*. A row that omits the key can say neither, and an agent reading
the GUI wire has no way to tell a clean unified-finish pass from one whose
dropped-band area was never computed. The CLI's reader could.

## 3. What was wired

Two commits on `tech-debt-3`.

**`4bfed4c` — `test(b5)`: five red-first sentries**
(`crates/rs_cam_viz/src/controller/results_parity_tests.rs`). Two green at
HEAD (the store), three red (the read). The pre-fix reproduction stays in
the file permanently, per §0 rule 1.

**`1c60f8a` — `fix(b5)`**: `build_mcp_diagnostics` now builds its
per-toolpath rows **from** `ProjectSession::diagnostics_with_evidence` — the
same `ToolpathDiagnostic` the CLI's report publishes — and layers the
GUI-only lane columns on top. Choices:

* **The lane columns are written last**, so every key the response already
  carried keeps its pre-B-5 value byte for byte. The ten core channels are
  purely additive to the wire.
* **A toolpath with no core diagnostic still gets its lane row.** The core
  builds one row per *generated* toolpath; the GUI must still answer for a
  pending, failed or `AwaitingPriorStock` op, because "cannot yet" is what
  the agent came for. Merge, not replace.
* **The hand-rolled row is deleted, not extended.** It is the mechanism that
  silently omitted each new channel once per channel added; extending it
  would have bought one wave of parity and re-armed the trap.
* **Project `collision_count`** comes off the same evidence instead of the
  literal `0` (X-VAC: a number that was never measured, printed on the
  surface an agent reads as a safety tally).
* **Core gains one seam**, `ProjectSession::simulation_triage_with_diagnostics`,
  so the response builds the project diagnostics **once** and the triage
  reuses them. `simulation_triage` delegates to it and is unchanged for
  every other caller. Without it the fix would have doubled that build per
  call.

This follows the existing core→worker→UI path rather than adding one: the
data has been in `session.results` since `d706c036`, the core record and its
`Serialize` impl already existed, and this response was the single surface
that declined to read them.

### 3.1 Sentries — red before, green after

`crates/rs_cam_viz/src/controller/results_parity_tests.rs`, 5 tests.

| sentry | at `1c0a4ac` | at `1c60f8a` |
|---|---|---|
| `gui_drain_writes_findings_and_spans_into_session_results` | **ok** (pins the store; deleting the `insert_result` call fails here) | ok |
| `gui_mode_toolpath_diagnostics_see_generation_stats` | **ok** | ok |
| `mcp_get_diagnostics_row_publishes_the_core_finding_channels` | **FAILED** — `missing the `op_kind` channel … Row was: {…}` | ok |
| `mcp_get_diagnostics_row_is_a_superset_of_the_core_diagnostic_wire` | **FAILED** — the ten-key drop list above | ok |
| `mcp_get_diagnostics_collision_count_comes_from_evidence` | **FAILED** — `left: Some(0), right: Some(1)` | ok |

The collision sentry asserts its own population first
(`core.collision_count == 1`, "or this sentry is vacuous") before asserting
the wire — §0 rule 4.

## 4. Residuals — honest, named, not fixed

**R-1 — `ProjectSession.simulation` is never assigned in GUI mode.** This is
the structural half of G-RESULTS and it is deliberate: the trace lives on
`state.simulation.results` and every GUI caller uses the `_with_evidence` /
explicit-trace variant. The hazard is that it is enforced by nothing. Any
*new* core convenience method that reads `self.simulation` — `diagnostics()`,
`triage()`, `tool_load_report()`, `export_gcode_with_policy()` all do — is
silently wrong in GUI mode with no compile-time guard, and the GUI's
work-around for the last one is a comment in `mcp_export_gcode` rather than a
type. Re-open condition: a fifth such method appears, or one of the four is
called from viz.

**R-2 — `debug_trace` / `semantic_trace` are `None` in GUI-mode
`session.results`.** Deliberate (they are `Arc`'d on `gui.toolpath_rt` and
core readers only want `annotated.spans`), and the MCP narration reads the
viz side, so no agent-facing surface loses them today. But
`ProjectSession::narrate_toolpath` — the core sibling — would narrate
without traces if ever called in GUI mode.

**R-3 — `ProjectDiagnostics::verdicts` reaches neither wire.** The CLI's
`summary.json` publishes only the legacy `verdict` string; the GUI response
now carries everything else off the same record but not this. Symmetric, so
not a parity defect; a shared gap. `triage` covers the same ground for an
agent, which is presumably why nobody noticed.

**R-4 — the CLI cannot generate a rest-machining chain at all.** Measured,
not inferred: on this fixture the CLI generated **3 of 7** enabled
toolpaths and the GUI generated **7 of 7**. `ProjectSession::generate_all`
is a single pass; the fixpoint ladder that resolves
`StockSource::FromRemainingStock` lives viz-side
(`settle_generate_all_round`), and `rs_cam_cli project` exposes no
equivalent. Four wanaka ops fail with *"is set to use remaining stock … but
no simulated remaining-stock snapshot is available"* and are simply absent
from the CLI's report. This is the **largest** results gap the census found
and it points the other way from the ledger row. **Not fixed here**: moving
the ladder into core changes what `rs_cam_cli project` emits for every
existing invocation, which is a fingerprint question and a scope this wave
was not given. Recommend a ledger row.

**R-5 — the two reads the ledger row named were never wire-diffed.** §1.4:
the CLI has no `get_toolpath_diagnostics` wire, so rows 1-3 rest on a shared
call site plus the `drill_gates` observable. A byte diff would need a
CLI-side dump of those two reads, which this wave did not add.

**R-6 — Checkpoint K's `ceiling_advisory` and `clamped_to` did not fire.**
Row 21. Parity is structural, not observed. A fixture that parks a feed on
the band ceiling would settle it.

**R-7 — the post-fix live re-run was not done.** The fix is proven by five
sentries at unit level and by the pre-fix live capture of the defect. Re-running
the wanaka census against a rebuilt binary needs a release rebuild, which
§0 rule 8 forbids inside a wave. See §5 for what *was* re-run live.

## 5. Fingerprints

**No fingerprint moved.** This is read-path plumbing:

* `crates/rs_cam_core/src/session/compute.rs` — one new method; the existing
  `simulation_triage` becomes a one-line delegation to it with identical
  inputs and identical output.
* `crates/rs_cam_viz/src/controller/events/compute.rs` — response assembly
  only. No generator, no simulator, no gate, no feed.

Verified at `60d7cab`:

| run | result |
|---|---|
| `cargo test -p rs_cam_viz --features mcp` | 256 + 14 + **15** + 11 pass, 0 fail — the 15 are `mcp_escape_hatches` |
| `cargo test -p rs_cam_core --lib` | 2272 pass, 0 fail, 12 ignored |
| `cargo test -p rs_cam_core --test param_sweep -- --ignored` | **56 pass, 0 fail** — the generation fingerprint harness, run in full (the sweeps are `#[ignore]` by default; a bare `--test param_sweep` reports `56 ignored` and proves nothing) |
| `cargo test -p rs_cam_core --test transform_provenance_fingerprints` | **3 fail — the known G-XFP three**, byte-identical messages to the pre-wave capture (`"re-pinned by PR-6 … captured at HEAD 5d32150 before C1"`, move counts 74 / 74 / 40). Red set unchanged |
| `cargo test -p rs_cam_cli` | 7 + 9 pass — includes `the_per_toolpath_json_is_byte_stable`, the CLI wire pin |
| `cargo test -p rs_cam_mcp` | 13 pass — includes the frozen capability-flag surface |
| `cargo clippy -p rs_cam_viz -p rs_cam_mcp --all-targets --all-features` | clean |
| `cargo clippy -p rs_cam_core --all-targets` | clean |
| `cargo fmt --check` | clean on every file this wave touched (the only diffs in the tree are agent A-8's in-flight `tool_load/optimize/*`) |

**NOT RUN, and why.** The full `rs_cam_core` integration suite (~2930 tests
across 171 binaries). It was started, spent a very long time inside its
first binary, and was stopped to return the cargo slot. Proportionality
argument, stated so it can be disagreed with: the only core change is
`simulation_triage` becoming a one-line delegation to a new method with the
same inputs and the same output, and **no integration test constructs a
triage** — a grep for `simulation_triage` / `SimulationTriage` / `.triage()`
outside core's own unit tests finds exactly three consumers
(`controller/events/compute.rs`, `ui/sim_diagnostics.rs`,
`cli/project.rs`), and all three crates' suites are green above. The core
lib tests, which include `sim_triage`'s own eight, passed.

## 6. Log entry (§3.1 format)

**B-5 — GUI-mode results parity.** Research-first census of wanaka through
both surfaces at one cell size; 21 field families, 11 divergent, all on one
read. G-RESULTS' first three clauses are STALE at `1c0a4ac` (the store has
been wired since `d706c036`; live `drill_gates` prove it) and its fourth is
REAL but on `get_diagnostics`, which the row does not name: ten published
channels dropped plus a literal-zero collision count. Red-first sentry
`4bfed4c` (3 of 5 red), fix `1c60f8a` routes the response through the core
`ToolpathDiagnostic` and adds one core seam so the diagnostics build once.
Four residuals carried, one of them (**R-4**, the CLI's missing fixpoint —
3 of 7 toolpaths generated vs the GUI's 7 of 7) larger than the defect
fixed and pointing the opposite way; recommend a ledger row. Fingerprints
unmoved.
