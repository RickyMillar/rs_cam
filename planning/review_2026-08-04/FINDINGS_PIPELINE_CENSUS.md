# FINDINGS_PIPELINE_CENSUS — H2.1 / R7 (W1)

Date: 2026-08-04. Branch: `experiment/adaptive-spiral`.
Scope: **H2.1 only** — the generation-findings transport from core to the
GUI worker and every downstream boundary that hand-copies it. Arcfit
(H2.2), the screenshot exporter (H2.3), finishing geometry (H2.4/H2.5) and
agent-read performance (H2.6) belong to other waves and are not touched
here.

> **ERRATUM, 2026-08-06 (W10 close-out, plan §L1 stale-rationale sweep).**
> Every "twelve report-only findings" statement in this document was
> correct when written and is now **fourteen**: Checkpoint C landed two
> `ToolpathStats` slots, not one — `offset_library_failures`
> (`crates/rs_cam_core/src/compute/config.rs:420`) **and**
> `boundary_clip_dropped` (`:438`). The contract split moves with it:
> **eleven** share the `None` = not-measured / `Some(0.0)` = measured-clean
> contract and **three** are deliberately outside it (`derived_stepovers`,
> `zero_removal`, `boundary_clip_dropped`), not ten and two. The record
> below stands verbatim per errata discipline; `CLAUDE.md` and
> `FINISHING_OPEN_DEFECTS_EVIDENCE.md` §0 carry the current counts.

Research question answered (plan §H2 RQ1): *can the CLI's exhaustive
destructuring serde-view pattern be used for the GUI worker mapping without
dragging GUI-only state into core, and what is core-owned / GUI-owned /
deprecated-wire-only / computed-after-generation?*

**Answer: yes, and the join belongs in core.** The GUI worker contributes
**zero** GUI-only state to this transport — both of its inputs
(`AnnotatedToolpath` and `GenerationFindings`) are core types, and its
output (`ToolpathStats`) is a core type. See §4.

---

## 1. The payload

`GenerationFindings` — `crates/rs_cam_core/src/compute/execute.rs:66`
(`#[derive(Debug, Clone, Default, PartialEq)]`, not `#[non_exhaustive]`),
carried on `ExecutionContext::findings` as a `RefCell` (`execute.rs:514`)
and returned beside the toolpath by
`execute_operation_annotated_with_regions` (`execute.rs:2350`, return type
at `execute.rs:2374`).

Eleven fields, each written by exactly one `record_*` helper:

| # | field | producer | contract |
|---|---|---|---|
| 1 | `truncated_core_mm2` | `record_truncated_core` `execute.rs:154` | X-19 three-valued (`None` = not measured) |
| 2 | `untouched_material_mm2` | same call | X-19 three-valued |
| 3 | `reached_uncut_estimate_mm2` | same call | X-19 three-valued |
| 4 | `dropped_band` | `record_dropped_band` `execute.rs:172` | X-19 three-valued |
| 5 | `clipped_band` | `record_clipped_band` `execute.rs:183` | X-19 three-valued |
| 6 | `tip_float` | `record_tip_float` `execute.rs:195` (merging) | X-19 three-valued |
| 7 | `deprecated_dial` | `record_deprecated_dial` `execute.rs:210` | X-19 three-valued |
| 8 | `ramp_reach_clamp` | `record_ramp_reach_clamp` `execute.rs:227` | X-19 three-valued |
| 9 | `claims_reference` | `record_claims_reference` `execute.rs:240` | X-19 three-valued |
| 10 | `zero_removal` | `record_zero_removal` `execute.rs:291` | **documented exception** — `None` conflates not-measured with nothing-to-report |
| 11 | `derived_stepovers` | `record_derived_stepover` `execute.rs:336` | **documented exception** — `Vec`, empty = derived none |

`ToolpathStats` (`crates/rs_cam_core/src/compute/config.rs:135`) is the
carrier those eleven land on, plus four move-derived fields. That is the
"twelve report-only findings" of `CLAUDE.md`: the eleven above plus
`retract_trips`.

## 2. Field classification

| field | class | where the value is decided |
|---|---|---|
| `move_count` | **computed-after-generation** | `compute/stats.rs:40` (post-dressup move list) |
| `cutting_distance` | **computed-after-generation** | `compute/stats.rs:41` |
| `rapid_distance` | **computed-after-generation** | `compute/stats.rs:42` |
| `retract_trips` | **computed-after-generation** | `compute/stats.rs:72` → `compute_retract_trips` `stats.rs:93`; needs trusted spans for the in/out split |
| the eleven of §1 | **core-owned** | generation adapters, via `record_*`; not derivable from moves |
| `standing_material_mm2` | **deprecated-wire-only** | a JSON key, not a Rust field. Emitted as a duplicate of `truncated_core_mm2` at `session/mod.rs:1454` and `cli/project.rs:146`. No Rust identifier in the repo carries it |
| — | **GUI-owned** | **none.** No field of `ToolpathStats` or `GenerationFindings` originates in viz. The GUI is a pure reader (§3, B8) |

The absence of any GUI-owned field is the load-bearing fact for §4: the
worker's mapping has nothing GUI-shaped in it, so moving the join into core
imports nothing.

## 3. Boundary census

Every site that copies findings/stats across a module or crate boundary.
"Guard" = what the compiler does when a **new** field appears upstream.

| id | boundary | site | shape | guard before this wave |
|---|---|---|---|---|
| B1 | generation → `GenerationFindings` | `compute/execute.rs:2374` (tuple return) | whole value | n/a (source) |
| B2a | `GenerationFindings` → `ToolpathStats` (**core session path**) | `session/compute.rs:1549` | struct literal, no `..` | new `ToolpathStats` field: **E0063 error**. New `GenerationFindings` field: **SILENTLY DROPPED** |
| B2b | `GenerationFindings` → `ToolpathStats` (**GUI worker path**) | `rs_cam_viz/src/compute/worker/execute/mod.rs:770-808` | `compute_stats_with_spans(..)` then 11 field assignments | **none in either direction.** A new field on either struct is silently dropped |
| B3 | `ToolpathStats` → `session::ToolpathDiagnostic` | `session/compute.rs:3086` | struct literal, no `..` | new diagnostic field errors; new `ToolpathStats` field silently unpublished (deliberate lossy projection, §5) |
| B4 | `session::ToolpathDiagnostic` → JSON | `session/mod.rs:1429` hand-written `Serialize` (17 fields) | manual `serialize_field` calls | **none** — a new field is silently unpublished. Emits the dual key at `:1444`/`:1454` |
| B5 | `session::ToolpathDiagnostic` → CLI wire | `rs_cam_cli/src/project.rs:103` `ToolpathDiagnostic::from_core` | **exhaustive destructure, no `..`** | **E0027 error** — the precedent this wave ports |
| B6 | `ToolpathStats` → `Vec<Diagnostic>` | `diagnostics/adapters/from_generation.rs:33` | one `out.extend(fn(..))` per finding | none (additive fan-out; a missing producer is a silent gap, not a wrong value) |
| B7 | `ToolpathStats` → `ToolpathNarrationContext` | `session/compute.rs:2848` **and** `rs_cam_viz/src/app/mcp.rs:1098` | two parallel struct literals, no `..` | new context field errors at both; new `ToolpathStats` field is silently un-narrated |
| B8 | `ToolpathStats` → GUI | `state/toolpath/entry.rs:166` (`pub stats: ToolpathStats`) | whole value, moved through `ComputeMessage` | n/a — no copy. Readers: `ui/toolpath_panel.rs:130`, `ui/properties/mod.rs:3664` (`claims_reference`), `ui/sim_diagnostics.rs:1171`, `ui/export_wizard.rs:934`, `ui/readiness.rs:141`, `ui/sim_timeline.rs:1943` |
| B9 | `ToolpathStats` → diagnose pipeline | `diagnostics/diagnose.rs:58` (`stats: Option<&ToolpathStats>`) | whole borrow | n/a — no copy |
| B10 | move list → `ToolpathStats` | `compute/stats.rs:39` | struct literal, no `..` | new `ToolpathStats` field: **E0063 error** |

Non-production `ToolpathStats` construction sites (placeholders/tests, no
transport role): `state/toolpath/entry.rs:303,338`, `session/mod.rs:1924`,
`session/mutation.rs:1186`, `session/compute.rs:4075,4286,4478,5221`,
`compute/execute.rs:3238`, `diagnostics/adapters/from_generation.rs:589`.

### The five patches this replaces

B2b has been amended once per finding channel added — the code says so
itself at `worker/execute/mod.rs:803-805` ("this is the fifth field to need
these lines") — and the worst instance was `generate_via_core` narrowing
the return to `(toolpath, spans)`, which dropped `rest_grid`,
`rest_regions`, `planner_engagement` **and** the findings; the repair
comment survives at `worker/execute/mod.rs:220-226`.

## 4. Does the CLI pattern port to the worker?

Yes — and the honest form of it puts the join in **core**, not in viz.

* The CLI's `from_core` (B5) works because a struct pattern without `..`
  is a **hard error** (E0027) when the struct gains a field, and because
  the CLI genuinely has three CLI-only fields to add, so it must own a
  view struct.
* The worker has **no** analogous local contribution. Its two inputs are
  `AnnotatedToolpath` (core) and `GenerationFindings` (core); its output
  is `ToolpathStats` (core). Its mapping is byte-identical in intent to
  B2a's. Duplicating an exhaustive destructure into viz would preserve two
  copies of the same join and keep the divergence risk that produced the
  five patches.
* Numeric equivalence check (required before merging the two paths): B2a
  computed `cutting_distance` via `Toolpath::total_cutting_distance`
  (`toolpath.rs:197`, counts `Linear | ArcCW | ArcCCW`), B10 via
  `_ => cutting` on `MoveType`. `MoveType` has exactly four variants
  (`toolpath.rs:30-39`), so the two are **identical by construction**.
  `rapid_distance` and the `retract_trips` call were already literally the
  same expressions.

**Structural join shipped:** `compute::stats::stats_with_findings(tp,
spans, findings)` in `crates/rs_cam_core/src/compute/stats.rs` — the single
core-owned adapter, with three compile-time guards:

1. exhaustive destructure of `GenerationFindings` (no `..`) → a new
   finding is **E0027** until consciously handled;
2. exhaustive destructure of the move-derived `ToolpathStats`, naming each
   field as move-derived or generation-owned → a new stats field is
   **E0027** here;
3. a full `ToolpathStats` struct literal (no `..`) → a new stats field is
   also **E0063** here.

Both B2a and B2b become a single call to it. The worker's eleven
assignments and the session's eleven literal fields are deleted, so neither
path has a per-field surface left to drop a field from.

## 5. Surface coverage — what each finding actually reaches

Not a defect list; a map of deliberate projections. **No field is dropped
at B2a/B2b today** — both paths carried all eleven before this wave,
verified field by field.

| finding | `ToolpathStats` | diagnostics (B6) | MCP/CLI per-toolpath wire (B3/B4/B5) | narration (B7) | GUI |
|---|---|---|---|---|---|
| `truncated_core_mm2` | yes | yes `GEOM_STANDING_MATERIAL` | yes + legacy dual key | yes | via diagnostics |
| `untouched_material_mm2` | yes | yes (clause in `truncated_core`) | yes | yes | via diagnostics |
| `reached_uncut_estimate_mm2` | yes | yes (same clause) | yes | yes | via diagnostics |
| `dropped_band` | yes | yes | **area only** (`unmachined_band_area_mm2`; band name/height stay on the diagnostic) | yes | via diagnostics |
| `clipped_band` | yes | yes | no | yes | via diagnostics |
| `tip_float` | yes | yes | yes (two scalars) | yes | via diagnostics |
| `deprecated_dial` | yes | yes | no | **no** | via diagnostics |
| `derived_stepovers` | yes | yes (fans out per derivation) | no | **no** | via diagnostics |
| `ramp_reach_clamp` | yes | yes | no | yes | via diagnostics |
| `claims_reference` | yes | yes | no | **no** | yes `ui/properties/mod.rs:3664`; MCP `get_toolpath_params` `app/mcp.rs:1037` |
| `zero_removal` | yes | yes | no | yes | via diagnostics |
| `retract_trips` | yes | **no** (no adapter) | no | yes | — |

**Documentation defect found (report, not a code drop):** `CLAUDE.md` says
the twelve findings are "surfaced through `narrate_toolpath` and the
diagnostics list". Three are absent from narration (`deprecated_dial`,
`derived_stepovers`, `claims_reference` — the last has its own GUI and MCP
surface) and `retract_trips` has no diagnostic adapter. Both narration
literals (B7) agree with each other, so this is a projection gap, not a
core/GUI divergence. **NOT FIXED, STATED** — owner: W1 hand-off /
documentation lane; re-open condition: any wave that claims narration
coverage of a generation finding must first widen B7 or correct the
sentence.

## 6. Residual unguarded boundaries (deliberately out of H2.1 scope)

* **B4** — the hand-written `Serialize` for `session::ToolpathDiagnostic`
  cannot be a `derive` while the wire emits the dual `truncated_core_mm2` /
  `standing_material_mm2` key from one field, and the A6 sentry
  (`tests/standing_material_channel_am9.rs:374`) pins both that dual-key
  emission and the rule that a reader must not combine `serde(alias)` with
  it. Left as is; this wave's join is upstream of it.
* **B7** — two parallel narration-context literals. Both are exhaustive on
  their own struct; the un-narrated `ToolpathStats` fields above are the
  live consequence. A `ToolpathNarrationContext::absorb_stats(&ToolpathStats)`
  helper with the same exhaustive-destructure shape would close it in one
  core-owned site. **NOT FIXED, STATED** — owner: W8 (agent reads) or a
  follow-up in this lane; condition: a finding needs to reach narration.
* **B3** — deliberate lossy projection onto the MCP/CLI per-toolpath
  summary; widening it is a wire change, not report wiring.

## 7. Acceptance-gate evidence — the synthetic-field check

Gate: *adding a synthetic `GenerationFindings` field must fail compilation
in every required adapter until consciously handled.*

Procedure (manual):

1. Add `pub synthetic_probe_mm2: Option<f64>,` to `GenerationFindings`
   (`crates/rs_cam_core/src/compute/execute.rs`).
2. `cargo check -p rs_cam_core`, then `cargo check -p rs_cam_viz`.
3. Remove the field; re-check clean.

### Result (executed 2026-08-04)

**Parent control — revision `d40768a`, checked out into a throwaway
worktree so the working tree was never disturbed:**

```
$ cargo check -p rs_cam_core     # with synthetic_probe_mm2 present
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.88s
$ cargo check -p rs_cam_viz      # with synthetic_probe_mm2 present
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 38.57s
```

**Both PASS.** On the parent, a new `GenerationFindings` field compiles
clean and is silently dropped at B2a *and* B2b. The defect this wave exists
for, reproduced rather than asserted.

**Post-fix — same probe, this working tree:**

```
$ cargo check -p rs_cam_core
error[E0027]: pattern does not mention field `synthetic_probe_mm2`
   --> crates/rs_cam_core/src/compute/stats.rs:141:9
    |
141 |       let GenerationFindings {
    |  _________^
...
153 | |     } = findings;
    | |_____^ missing field `synthetic_probe_mm2`
    |
help: include the missing field in the pattern
help: if you don't care about this missing field, you can explicitly ignore it
help: or always ignore missing fields here

$ cargo check -p rs_cam_viz
error[E0027]: pattern does not mention field `synthetic_probe_mm2`
error: could not compile `rs_cam_core` (lib) due to 1 previous error
```

**Both FAIL**, and the GUI worker path fails through its own crate: viz
cannot build past `rs_cam_core`, so no GUI generation can ship a dropped
finding. There is exactly one required adapter after this wave and it
errors, with rustc itself offering the three conscious choices (route it,
`_`-ignore it, or `..` it — the last being the only one this design forbids
in review).

Probe removed; `git diff --stat crates/rs_cam_core/src/compute/execute.rs`
empty, and both checks clean again.

A machine-checked companion to the manual procedure ships as
`crates/rs_cam_core/tests/findings_transport_join_h21.rs`: it populates
every one of the eleven findings with a distinguishable value, runs them
through the single join, and asserts each one arrived — through an
exhaustive destructure of `ToolpathStats`, so a new stats field fails the
sentry as well as the join. It also pins that the join leaves the
move-derived half byte-identical to `compute_stats_with_spans`, and that
default findings still read as X-19 "not measured" rather than a
fabricated `Some(0.0)`.

## 8. Verification

Commands and known-red state are recorded in
`planning/review_2026-08-04/ORCHESTRATION_LOG.md` under
`## W1 — findings transport, 2026-08-04`.
