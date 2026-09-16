# Q — allowances, TODO markers, oversized units, pedantic lints

Read-only triage, 2026-09-16. No cargo run; `-W pedantic` counts come from
`evidence/pedantic_counts.txt` + `evidence/pedantic_raw.log`.

Headline: the allow population is in better shape than the 516 figure reads.
The axis produces **one A**, **one B**, and two duplication findings. The rest
is counted, not itemised.

| id | tier | cost | file:line | claim | OWNER |
|---|---|---|---|---|---|
| Q1 | A | M | `crates/rs_cam_viz/src/controller/events/toolpath.rs:107` | No surface populates `SuggestContext.model_bbox`, so the runtime-sanity stepover back-off never fires | mixed (viz/session + power-calcs) |
| Q2 | B | S | `crates/rs_cam_cli/src/sweep.rs:6` | The only production file-wide `#![allow(clippy::indexing_slicing)]`: 534 lines, ~18 index sites, no justification | — |
| Q3 | C | S | `crates/rs_cam_core/src/feeds/geometry_class.rs:89` | `classify_3d_terrain` and `predict_move_count`'s bbox branch are unreachable from every shipped surface, and read as load-bearing | power-calcs |
| Q5 | D | M | 26 sites / 19 files (`crates/rs_cam_core/src/`) | The `never_cancel` + `expect("… should never be cancelled")` idiom is copied 26 times, each with its own `#[allow(clippy::expect_used)]` | — |
| Q6 | D | S | `crates/rs_cam_core/src/adaptive3d/mod.rs:1150` | A test re-implements the flat-shelf histogram of `path.rs:511` verbatim, so it cannot catch drift in the production copy | — |
| Q7 | F | S | `planning/tech_debt_2026-09-16/evidence/allows_without_safety.md` | The scanner over-counts: 232 of 516 rows carry a same-line justification, 6 rows are `cfg(test)` modules, 1 row is a comment | — |
| Q4 | F | S | `crates/rs_cam_core/src/tool_load/optimize/patches.rs:11` | `#[allow(unused_imports)] use OperationParams as _;` — the allow states the compiler finds no use | power-calcs |
| Q8 | F | — | 258 `indexing_slicing` rows | 25 sampled across 14 files; 0 index is derived from unchecked external data | — |
| Q9 | F | — | 30 `expect_used`/`unwrap_used`/`panic` rows | All read; every one is a closure invariant, a fixed-size array, or a length check on the line above | — |
| Q10 | F | L | 110 `too_many_arguments` rows | Wide generator signatures; top 5 files listed below | — |
| Q11 | F | L | `crates/rs_cam_core/src/feeds/mod.rs`, `suggest.rs` | T-5, inherited by ID; 89 functions ≥ 250 lines, no defect found inside the top 10 | power-calcs |
| Q12 | F | — | `evidence/pedantic_counts.txt` | 6 correctness-adjacent lints assessed; none proposed as a deny | — |

---

## Q1 (A) — the Suggest button runs without the model it is told to read

`crates/rs_cam_viz/src/controller/events/toolpath.rs:107`:

```
// TODO(v1.2): populate model_bbox + upstream leftover so
// the GUI Suggest button benefits from runtime-sanity
// floor; needs per-tool model lookup at "add toolpath"
// time (model_id is selected later by the user).
context: rs_cam_core::feeds::suggest::SuggestContext::default(),
```

`crates/rs_cam_viz/src/app/mcp/commands.rs:858` carries the same TODO.

The TODO's excuse is add-toolpath ordering. That excuse does not cover the
other call sites. `rg model_bbox` over `crates/*/src` finds **no assignment of
`SuggestContext.model_bbox` anywhere**. Every surface passes the default:

- `crates/rs_cam_viz/src/ui/properties/mod.rs:2388` — the Suggest rationale on
  an **existing** operation, with a model already loaded.
- `crates/rs_cam_viz/src/ui/properties/pills.rs:137` — the apply-funnel preview.
- `crates/rs_cam_viz/src/controller/events/mod.rs:1023`, `:1208`
- `crates/rs_cam_viz/src/controller/events/model.rs:760`
- `crates/rs_cam_core/src/session/compute.rs:1875`

Why it can fail. `crates/rs_cam_core/src/feeds/suggest.rs:1040` documents that
`context.model_bbox` gates the runtime-sanity stepover back-off.
`crates/rs_cam_core/src/feeds/predict.rs:490` returns `0` for a `None` bbox.
The back-off therefore reads "no constraint signal" on every real Suggest.
The operator gets the un-backed-off stepover. `suggest.rs:2553` and `:2641`
classify geometry from the same `None`.

The data is in scope at the largest site: `properties/mod.rs:157` defines
`first_model_bbox(state)` in the same file, 2 200 lines above the call.

Fix shape: plumb the bbox at the sites that already hold a model. Leave the two
add-toolpath sites on the TODO until `model_id` lands earlier.

## Q2 (B) — a file-wide allow of a denied lint

`crates/rs_cam_cli/src/sweep.rs:6`:

```
#![allow(clippy::print_stdout, clippy::indexing_slicing)]
```

`Cargo.toml` denies `indexing_slicing`; `CLAUDE.md` asks for a local allow with
a `SAFETY:` comment. This suppresses the lint over 534 lines and ~18 index
expressions, so a future out-of-range index in this file lints clean.

The `print_stdout` half is legitimate (a CLI surface) and the other four
file-wide allows are the documented CLI print allowances —
`crates/rs_cam_cli/src/main.rs:2`, `:3`, `smoke.rs:14`, `nc_replay.rs:1`.
Split the attribute: keep `print_stdout` at file scope, move
`indexing_slicing` to the sites.

## Q3 (C) — two bbox consumers that nothing reaches

A consequence of Q1. It survives a decision to leave Q1 open. `crates/rs_cam_core/src/feeds/geometry_class.rs:89
classify_3d_terrain` and the bbox branch of
`crates/rs_cam_core/src/feeds/predict.rs:487 predict_move_count` take the
`None` path on every production call. Only tests exercise the other branch
(`suggest.rs:4722` is the counter-test for `None`). They read as live policy.

## Q4 (F) — an import the compiler says is unused

`crates/rs_cam_core/src/tool_load/optimize/patches.rs:11`:

```
#[allow(unused_imports)]
use OperationParams as _;
```

`use … as _` imports a trait for method resolution. The allow exists because
the compiler finds no such resolution, so either the import is dead or a
method call moved. One `cargo check` after deletion settles it.

## Q5 (D) — 26 copies of one cancellation invariant

Every uncancellable wrapper in core repeats:

```
let never_cancel = || false;
x_with_cancel(…, &never_cancel).expect("non-cancellable … should never be cancelled")
```

26 occurrences across 19 files (`ramp_finish`, `radial_finish`, `spiral_finish`,
`dropcutter`, `steep_shallow`, `horizontal_finish`, `depth`, `waterline`,
`inlay`, `vcarve`, `scallop`, `pencil`, `slope`, `pocket`, `face`, `trace`,
`project_curve`, `adaptive/mod`, `adaptive3d/mod`). Each carries its own
`#[allow(clippy::expect_used)]` — 20 of the 30 rows in Q9.

`crates/rs_cam_core/src/tier_map.rs:970` already has a `never_cancel()` helper.
A shared `NeverCancel` plus one `unwrap_or_else(|_| unreachable)` adapter
deletes 26 allows and the drift risk that one wrapper stops matching its
cancellable twin.

## Q6 (D) — a test that re-implements the code it tests

`crates/rs_cam_core/src/adaptive3d/mod.rs:1146` says so in a comment:
`// Histogram detection logic (same as in adaptive_3d_segments)`. Lines
1150–1172 duplicate `crates/rs_cam_core/src/adaptive3d/path.rs:511–535`
(bin size, `n_bins`, the 2 % threshold, the `flat_z` filter). A change to the
production flat-shelf detector leaves this test green.

---

## F material — counts

**Q7, instrument recalibration.** Of the 516 rows, 232 carry a same-line `//`
justification (`// bounded indexing in algorithmic code`, `// len >= 2 guarded
below`). 8 more have a `SAFETY:` line above the attribute, which the 3-line
window scans past. 6 rows are the allow attribute of a `cfg(test)` module
(`retarget_reconciliation_a8.rs:47`, `svg_input.rs:307`,
`gcode_validator.rs:637`, `diagnostics/tests.rs:5`, `geo.rs:365`,
`arc_util.rs:81`) and `session/compute.rs:3308` is a comment, not an attribute.
Subtract those and the 40 `dead_code` rows (some of which the 232 already
count): **≈ 230 genuinely bare allows**, not 516.

**Q8, `indexing_slicing` (258).** Sampled 25 rows across 16 files, importers
first (`mesh.rs`, `step_input.rs`, `svg_input.rs`, `dxf_input.rs`,
`enriched_mesh.rs`), then `adaptive_shared.rs`, `adaptive/path.rs`,
`edge_distance.rs`, `slope.rs`, `rest.rs`, `vcarve.rs`, `tsp.rs`,
`session/compute.rs`, `controller/io.rs`, `toolpath_render.rs` and
`sim_timeline.rs`. Every one is bounded by a fixed-size
array, a `windows(2)` slice, a loop range, a const index into a const table, or
an `is_empty` / length check within a few lines. Three that looked unsafe are
guarded: `enriched_mesh.rs:310` (`loops_2d.is_empty()` at :299),
`adaptive_shared.rs:121` (`path.len() < 3` at :117), `controller/io.rs:529`
(`toolpaths.is_empty()` at :524). No A found. Treat the group as style.

**Q9, `expect_used` / `unwrap_used` / `panic` (30 rows).** All read. 20 are Q5.
`geo.rs:365` and `arc_util.rs:81` are test modules. `slope.rs:242
from_parts` panics on a documented `# Panics` invariant.
`adaptive/path.rs:1279` follows `if path.len() < 2 { continue; }`.
`svg_input.rs:154` follows `if pts.len() >= 2`. None is keyed on parsed input,
file IO, or a user-supplied map key.

**TODO markers (all 12 read).** Ten are citation debt in the feeds physics and
are not user-visible: `material.rs:541`, `:844`, `:856`, `:874`, `:877`,
`:888`, `:913` name the missing FPL / CSIRO / EMBRAPA sources behind a `Kc`
literal, and `feeds/vendor_normalize.rs:269`, `:305` record a plastics LUT
binning and a Rockwell scale the LUT cannot hold. Each states its substitute
and its cost in place. OWNER: power-calcs; keep them. The remaining two are
Q1.

**Q10, `too_many_arguments` (110).** Top 5 files:
`dexel_stock/simulation.rs` 13, `scallop.rs` 8, `adaptive3d/clearing.rs` 7,
`feeds/suggest.rs` 6 (power-calcs), `unified_finish.rs` 5. Generator
signatures that mirror their op params. Style.

**Q11, oversized units.** Top 10 functions by length:
`feeds/mod.rs:1192 calculate` 1302, `gpu_upload.rs:158 upload_gpu_data` 1262,
`properties/mod.rs:4051 draw_toolpath_panel` 1241,
`unified_finish.rs:1510 unified_finish_toolpath_with_cancel_and_ceiling` 1198,
`session/command.rs:950 fmt` 1048,
`adaptive3d/clearing.rs:1463 clear_z_level_agent_2d_slice` 1038,
`compute/catalog.rs:1540 strip_all` 1015,
`overlays/registry.rs:393 in_simulation` 857,
`mcp/commands.rs:1823 describe_core` 826,
`adaptive3d/path.rs:237 adaptive_3d_segments` 801. No mechanical split
proposed. Only one allow sits inside any of them —
`unified_finish.rs:1841` — and its cast is guarded by a
`gc < 0.0 || gr < 0.0 || gc >= nx || gr >= ny` test on the line above.
`feeds/mod.rs` and `suggest.rs` are **T-5**, already on the register.

**Q12, pedantic lints.** Six correctness-adjacent picks:

| lint / message | count | worth denying? |
|---|---|---|
| casting may truncate (`cast_possible_truncation`) | 718 | No. The four sampled index casts all clamp or range-check first. 718 sites is a wave, not a defect. |
| may lose the sign (`cast_sign_loss`) | 264 | No. Same sites; Rust saturates a negative `f64 as usize` to 0, and the sampled ones reject negatives before the cast. |
| `cast_lossless` (154) | 154 | No. Mechanical `f64::from` rewrite, zero behaviour change. |
| `float_cmp` (16) | 16 | No, but worth 16 documented allows. Every site is deliberate bit-identity change detection (`feeds/provenance.rs:212` manual-override stamping, `gcode/program_builder.rs:45` modal move suppression) or a post-`round()` compare (`gcode/emitter.rs:298`). |
| `while_float` (32) | 32 | No. The two loops whose step could reach zero are guarded: `geo.rs:312` returns early on `spacing <= 1e-6`, `scallop.rs:2248` floors the step at `cusp_r * 0.1` and `cusp_radius()` falls back to `radius()`. |
| `manual_midpoint` (88) | 88 | No. All `f64`; the overflow the lint names cannot occur. |

Everything else in the 106-row table is style, not proposed.

---

## Summary

1. The 516 undocumented allows are mostly a measurement artifact: 232 carry a
   same-line justification and the real bare count is about 230.
2. Sampling found no A-tier allow. 25 `indexing_slicing` sites and all 30
   `expect`/`unwrap`/`panic` sites are guarded, documented, or test code.
3. The one live defect on this axis is Q1: `SuggestContext.model_bbox` is never
   populated, so the runtime-sanity stepover back-off is dead on every surface.
4. One workspace-rule violation stands: `sweep.rs` suppresses a denied lint
   file-wide over 534 lines.
5. The two duplications (Q5 `never_cancel` ×26, Q6 the copied histogram) are
   the cheapest real cleanups here; the long functions carry no defect inside.
