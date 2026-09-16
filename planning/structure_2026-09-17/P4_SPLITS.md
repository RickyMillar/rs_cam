# P4 — the file-split plan

Status: **proposal, 2026-09-17.** Programme
`planning/structure_2026-09-17/`, work package P4. It follows P2, which
grouped `crates/rs_cam_core/src/` into folders. P2 solved the flat-root
problem. P4 addresses the next navigability problem: file size.

This document plans a split for each of the 19 production files over
3 000 lines. It measures every file, proposes its child modules by item
name and line range, states the visibility each moved item needs, counts
the break cost, and names the gate to run. It changes no code.

## 0. Rulings this plan follows

The operator ratified these on 2026-09-16 and 2026-09-17. They come from
`planning/structure_2026-09-17/P2_LAYOUT.md` §0 and from
`.claude` memory row "No legacy, breaking OK".

**R1 — no legacy, no shim at an old path.** A caller of a deleted or moved
module changes its path.

**R2 — a split keeps the module path of every item.** The parent module
re-exports its children's items with `pub use child::*;` or an explicit
`pub use child::Item;`. That is not a shim at an old path. It is the
split's contract: the module keeps its name, and the items keep their
place inside it. An out-of-crate caller does not change. R1 and R2 do not
conflict, because R1 governs a module that MOVES and R2 governs a module
that GROWS children.

**R3 — a split moves whole items.** It moves functions, `impl` blocks,
structs, enums, consts, statics and test modules. It never cuts a
function.

**R4 — a long function is not a split target.** `feeds/mod.rs:1193
calculate` is 1296 lines and carries step banners, but 16 `let mut` locals
cross those banners, so the banners are not a seam.
`planning/structure_2026-09-17/FEEDS_WAVE.md` row FW-22 already ruled:
"Do not schedule it in this wave." P4 keeps that ruling and extends it to
every long function in the set.

**R5 — no behaviour change.** A split commit contains moves, visibility
edits, `use` edits and re-export lines. Nothing else.

**R6 — a child holds 300 to 1200 lines and one topic.**

## 1. The measurement

Every number in this document comes from the working tree at commit
`5592b7d0`, measured on 2026-09-17.

**WARNING. The tree moves under this file. Read this before you start.**
The survey began at `2dcb2123`. Three commits landed while it ran, and
`c97837b8` deleted `NewDefaultCtx` from `compute/catalog.rs`, which moved
every line below 2613 in that file by 46 and shortened the file from 3334
to 3287. `session/compute.rs` grew by 18 lines in the same window.
`FEEDS_WAVE.md` reports the same effect. Therefore:

- **Re-locate every item by symbol name, not by line number.** The line
  ranges in this document are a reading aid. The symbol names are the
  contract.
- Re-measure each file with `wc -l` before you split it.
- An item this document names but `grep -n` cannot find is gone. Drop it.

| Quantity | Value |
|---|---:|
| Production files over 3 000 lines | 19 |
| Lines in them | 83 364 |
| Lines inside their `#[cfg(test)]` modules | 25 231 (30 %) |
| Files whose test content exceeds 500 lines | 17 |
| Lines in those 17 files' test modules | 24 708 |

**The single largest lever is the test module.** Every one of the 19 files
carries at least one `#[cfg(test)] mod`. In `adaptive3d/mod.rs` the test
module is 2 486 lines of 3 018. In `tool_load/optimize/mod.rs` five
sibling test modules hold 2 352 lines of 3 361. Move only the test
content and the 19 parents lose 24 708 lines — 30 % of the whole set —
through one mechanical move per file, with no visibility change and no
re-export.

| file | lines | test modules | test lines | after the test move |
|---|---:|---:|---:|---:|
| `session/compute.rs` | 8032 | 1 | 2046 | 5986 |
| `compute/execute.rs` | 6362 | 1 | 1786 | 4576 |
| `viz app/mcp.rs` | 6067 | 1 | 704 | 5363 |
| `viz ui/properties/mod.rs` | 5965 | 1 | 124 | stays |
| `feeds/suggest.rs` | 5769 | 1 | 2758 | 3011 |
| `finish/unified_finish.rs` | 4806 | 1 | 1566 | 3240 |
| `dressup/mod.rs` | 4726 | 1 | 1571 | 3155 |
| `feeds/mod.rs` | 4624 | 1 | 1859 | 2765 |
| `finish/conformal_spiral.rs` | 4401 | 1 | 933 | 3468 |
| `finish/pencil.rs` | 3634 | 1 | 1134 | 2500 |
| `finish/scallop.rs` | 3580 | 1 | 771 | 2809 |
| `tool_load/optimize/mod.rs` | 3361 | **5** | 2352 | **1009** |
| `viz state/simulation.rs` | 3302 | 1 | 708 | 2594 |
| `compute/catalog.rs` | 3287 | 1 | 525 | 2762 |
| `session/mutation.rs` | 3272 | 1 | 1167 | 2105 |
| `stock/simulation_cut.rs` | 3141 | 1 | 1307 | 1834 |
| `adaptive3d/mod.rs` | 3018 | 1 | 2486 | **532** |
| `viz ui/properties/operations/mod.rs` | 3013 | 1 | 399 | stays |
| `polygon.rs` | 3004 | 1 | 1035 | 1969 |

**Ten of the 19 files fall below 3 000 lines on the test move alone**:
`adaptive3d/mod.rs` (532), `tool_load/optimize/mod.rs` (1009),
`stock/simulation_cut.rs` (1834), `polygon.rs` (1969),
`session/mutation.rs` (2105), `finish/pencil.rs` (2500),
`viz state/simulation.rs` (2594), `compute/catalog.rs` (2762),
`feeds/mod.rs` (2765) and `finish/scallop.rs` (2809).

`tool_load/optimize/mod.rs` is the one file with more than one test
module. Its five siblings are `orchestration_skip_tests` (1006-1546,
541 lines), `project_rollup_tests` (1548-1871, 324),
`tests` (1873-2728, 856), `stage1_grid_tests` (2730-3182, 453) and
`candidate_eval_tests` (3184-3361, 178). They move as five sibling files
under `tool_load/optimize/`, or as one `optimize/tests/` folder. The move
edits no formula, no threshold and no control flow, which is exactly what
P2 ruling Q5 permits inside a frozen folder.

A line count inside a per-file section may differ from this table by one.
The survey tool counted a trailing newline as a line; this table uses
`wc -l`.

### The precedent already in the tree

`crates/rs_cam_core/src/diagnostics/mod.rs:60` declares:

```rust
#[cfg(test)]
mod tests;
```

and `crates/rs_cam_core/src/diagnostics/tests.rs` opens with a `//!`
header, the permitted `#![allow(...)]` block, then `use super::*;`.
`crates/rs_cam_viz/src/controller.rs:46` shows the same shape for a
`<stem>.rs` parent: the child sits at `controller/tests.rs`.

The child-folder shape is also already in the tree, twice.
`crates/rs_cam_core/src/compute/execute.rs` sits beside a
`compute/execute/` folder that holds four modules.
`crates/rs_cam_viz/src/app/mcp.rs` sits beside `app/mcp/commands.rs`. So a
`<stem>.rs` parent with a `<stem>/` child folder is an established shape
here, not a new one. Eight of the 19 files are already a `mod.rs`, so
their children become plain siblings in the folder they are already in.

So a test move needs three edits and nothing else:

1. `git mv` the module body into `<parent-dir>/tests.rs`.
2. Replace the inline module with `#[cfg(test)]\nmod tests;`.
3. Copy the module's `#![allow(...)]` set to the head of the new file.

The core `CLAUDE.md` permits only `unwrap_used`, `expect_used`, `panic`
and `indexing_slicing` there. `println!` and `eprintln!` stay denied.

**One exception to the test lever.** The move is otherwise free: no
visibility change, no re-export, no path edit. `app/mcp.rs` is the single
file where the **test-module move** is not free.
`workspace_menu_complete_g_wsmenu.rs:165` asserts a literal string that
lives INSIDE that file's `mod tests`, so moving the test module turns the
sentry red although nothing behavioural changes.

That is one of **two** verified sentry traps in this set. The second sits
on a banner comment in `ui/properties/operations/mod.rs` and fires on a
topic split, not on a test move. §7.1 gives both needles and both fixes.
Read §7.1 before the first viz commit of either kind.

## 2. How this document ranks the files

Navigability gain is the number of lines the split takes out of the
parent. Risk is a number:

| Risk | Value | Rule |
|---|---:|---|
| L | 1 | 0 exact-path sentries **and** at most 15 production-shared private helpers. |
| M | 2 | 1 or 2 exact-path sentries, **or** 0 sentries with more than 15 production-shared private helpers. |
| H | 3 | 3 or more exact-path sentries, **or** the file sits in a frozen folder (`feeds/`, `tool_load/`), **or** the split contradicts a ratified P2 ruling and needs a new operator ruling first. |

The two inputs are the sentry table in §3.2 and the coupling table in
§3.1b. Both are measured, so the letter is a lookup, not a judgement.
This document applies the rule uniformly. Where a per-file section
argues a different letter, the table here wins, and the section says so.

**Value = lines out of the parent ÷ risk value.** The ranking in §4 is
that quotient. It is reproducible from the summary table.

## 3. The two break costs

### 3.1 Path edits — zero, by construction

A `pub` item keeps its `pub` visibility in the child, and the parent adds
`pub use child::Item;`. The external path `rs_cam_core::<module>::Item`
survives. So the path-edit cost of every split in this document is **0**.

The table below records what the cost would be WITHOUT that re-export. It
measures how much the re-export line is worth, and it names the files
where a forgotten `pub use` would produce hundreds of errors.

| module | files outside the crate | occurrences | core files |
|---|---:|---:|---:|
| `compute::catalog` | 179 | 186 | 57 |
| `feeds` | 146 | 390 | 66 |
| `polygon` | 101 | 114 | 57 |
| `stock::simulation_cut` | 68 | 121 | 29 |
| `dressup` | 62 | 89 | 18 |
| `finish::unified_finish` | 31 | 34 | 11 |
| `feeds::suggest` | 26 | 86 | 7 |
| `compute::execute` | 15 | 21 | 11 |
| `tool_load::optimize` | 14 | 32 | 9 |
| `finish::pencil` | 13 | 18 | 15 |
| `finish::scallop` | 13 | 16 | 10 |
| `finish::conformal_spiral` | 7 | 12 | 1 |
| `adaptive3d` | 6 | 11 | 3 |
| `session::compute` | 0 | 0 | 1 |
| `session::mutation` | 0 | 0 | 1 |

### 3.1b `pub(super)` reaches the siblings, so a shared helper needs no home

`pub(super)` on an item inside a child module makes that item visible to
the parent module and to every sibling child of that parent. So a private
helper that two children call does **not** need a `shared.rs`, and it does
**not** have to stay in the parent. It stays in whichever child owns its
topic, it carries `pub(super)`, and the other child writes
`use super::<that_child>::<helper>;`.

This matters, because the coupling scan counts many such helpers:

| file | private helpers that two or more production items call |
|---|---:|
| `compute/execute.rs` | 30 |
| `finish/conformal_spiral.rs` | 27 |
| `feeds/suggest.rs` | 20 |
| `viz ui/properties/mod.rs` | 15 |
| `session/compute.rs` | 14 |
| `finish/pencil.rs` | 13 |
| `polygon.rs` | 13 |
| `finish/scallop.rs` | 12 |
| `viz ui/properties/operations/mod.rs` | 12 |
| `dressup/mod.rs` | 11 |
| `tool_load/optimize/mod.rs` | 11 |
| `finish/unified_finish.rs` | 8 |
| `viz state/simulation.rs` | 4 |
| `adaptive3d/mod.rs` | 3 |
| `feeds/mod.rs` | 3 |
| `viz app/mcp.rs` | 3 |
| `compute/catalog.rs` | 2 |
| `session/mutation.rs` | 1 |
| `stock/simulation_cut.rs` | 1 |

A `shared.rs` child is right only for a helper with no home topic at all.
Propose one sparingly.

**Caveat on the count.** The coupling scan matched each private name as a
whole word anywhere in an item's span, so it counted rustdoc intra-doc
links in doc comments as references. A doc link needs a path edit, not a
visibility change. The true counts are therefore lower than the table. Use
the table to rank the files, not to decide a single item.

A private item of the parent is already visible to every descendant
module. So a helper that the parent and one child share needs no
visibility edit at all. Only a helper defined in a CHILD and read from
outside that child needs `pub(super)`.

An inherent `impl` block is the cheapest case of all. `impl
ProjectSession { … }` compiles in any module of the crate, and its methods
stay reachable with no re-export at all. `session/compute.rs`,
`session/mutation.rs`, `viz app/mcp.rs` and `viz state/simulation.rs` are
mostly one inherent `impl` block each, so their true path cost is 0 even
before a `pub use` line.

### 3.2 Source-scanning sentries — the real cost

`planning/tech_debt_2026-09-16/evidence/source_scanning_sentries.txt`
lists 79 tests that read source text. They divide into two kinds.

A **directory walker** calls `std::fs::read_dir` and scans every `.rs`
file it finds. A walker stays green after a split: it finds the new child
files. `loose_executor_is_crate_private_wp12.rs`,
`stale_set_has_one_answer_wp28.rs`,
`non_egui_sites_write_through_commands_wp6b.rs`,
`egui_draw_sites_write_through_commands_wp6.rs` and
`panels_read_the_token_module_up1.rs` are walkers.

An **exact-path reader** names one file with `include_str!` or
`read_to_string`. It goes red the moment its needle moves to a child.
Every exact-path reader in the set:

| target file | exact-path readers | count |
|---|---|---:|
| `rs_cam_viz/src/app/mcp.rs` | `ui_string_hygiene:82`, `command_surface_completeness:138,648,663`, `effects_are_stamped_wp19:86,682`, `viewport_draws_selected_only_wp27:327`, `workspace_menu_complete_g_wsmenu:47`, `mcp_toasts_report_outcome_g_mcptoast:37`, `open_guard_asks_before_discarding_g_openguard:41`, `mcp_core_arm_describes_every_row:32`, `production_writes_go_through_apply_wp15a:104`, `overlays_registry:753,877` | 10 |
| `rs_cam_viz/src/ui/properties/mod.rs` | `freshness_surfaces_g_freshrender:27`, `the_gui_can_set_a_feed_g_fscontrol:169`, `the_recommendation_explains_each_row_g_whyrow:207`, `viewport_draws_selected_only_wp27:285`, `workspace_menu_complete_g_wsmenu:44`, `inspector_header_wraps_g_reachwrap:67`, `inspector_width_is_tab_independent_up4:57`, `overlays_registry:665,856` | 8 |
| `rs_cam_core/src/session/compute.rs` | `set_param_refuses_absent_field_n5:82`, `resolved_gen_inputs_has_one_producer:41` | 2 |
| `rs_cam_viz/src/ui/properties/operations/mod.rs` | `bottom_z_pin_note_g_bottompin:45` | 1 |
| the other 15 files | none | 0 |

A doc-comment mention is not a sentry.
`crates/rs_cam_core/tests/plunge_guard_ab_p3.rs:18` names
`session/compute.rs`, and
`crates/rs_cam_viz/tests/apply_contract_a3.rs:137` names
`ui/properties/mod.rs:1582`. Those lines are stale text after a split,
not a red test. Repair them with the split, but do not count them as risk.

### 3.3 The "cannot be split" comment

`crates/rs_cam_viz/tests/command_surface_completeness.rs:132-137` says
`app/mcp.rs` and `ui/properties/operations/mod.rs` "carry an INLINE `mod
tests`, so the file cannot be split". That sentence describes the
sentry's own exclusion list, `MCP_SOURCES`. The list names whole files,
and an inline `mod tests` cannot be excluded apart from its parent's
production half. A split does not break the property. It forces an edit
of `MCP_SOURCES` and a rewrite of that comment. Moving the inline tests
to a `tests.rs` child makes the list simpler, not harder.

## 4. The summary table

`out` is the number of lines the split takes out of the parent. `risk`
follows the rule in §2 exactly: this column is a lookup from §3.2 and
§3.1b, not the per-file section's own opinion. Where a section argues a
different letter, its note says so and this table wins. `value` is
`out ÷ risk value`. The rows are sorted by `value`.

Across all 19 files the plan takes **60 271** lines out of 83 364 and
leaves **23 093** in the 19 parents, spread over about 80 child files.
In rows 17 and 19 the parent figure includes a test module that stays
inline, because it is under 500 lines.

| # | file | lines | children | parent after | out | sentries | shared | risk | value |
|---:|---|---:|---:|---:|---:|---:|---:|:---:|---:|
| 12 | `session/mutation.rs` | 3272 | 4 | 120 | 3152 | 0 | 1 | L | **3152** |
| 8 | `finish/pencil.rs` | 3634 | 4 | 726 | 2908 | 0 | 13 | L | **2908** |
| 1 | `session/compute.rs` | 8032 | 6 | 2230 | 5802 | 2 | 14 | M | 2901 |
| 2 | `compute/execute.rs` | 6362 | 7 | 735 | 5627 | 0 | 30 | M | 2814 |
| 5 | `dressup/mod.rs` | 4726 | 5 | 2171 | 2555 | 0 | 11 | L | **2555** |
| 14 | `adaptive3d/mod.rs` | 3018 | 1 | 532 | 2486 | 0 | 3 | L | **2486** |
| 9 | `finish/scallop.rs` | 3580 | 3 | 1148 | 2432 | 0 | 12 | L | **2432** |
| 13 | `stock/simulation_cut.rs` | 3141 | 4 | 895 | 2246 | 0 | 1 | L | **2246** |
| 4 | `finish/unified_finish.rs` | 4806 | 2 | 2668 | 2138 | 0 | 8 | L | 2138 |
| 18 | `viz state/simulation.rs` | 3302 | 5 | 1230 | 2072 | 0 | 4 | L | 2072 |
| 11 | `compute/catalog.rs` | 3287 | 3 | 1410 | 1877 | 0 | 2 | L | 1877 |
| 16 | `viz app/mcp.rs` | 6067 | 6 | 891 | 5176 | 10 | 3 | H | 1725 |
| 3 | `feeds/suggest.rs` | 5769 | 5 | 885 | 4884 | 0 | 20 | H | 1628 |
| 7 | `finish/conformal_spiral.rs` | 4401 | 4 | 1258 | 3143 | 0 | 27 | M | 1572 |
| 17 | `viz ui/properties/mod.rs` | 5965 | 7 | 1280 | 4685 | 8 | 15 | H | 1562 |
| 19 | `viz ui/properties/operations/mod.rs` | 3013 | 3 | 711 | 2302 | 1 | 12 | M | 1151 |
| 6 | `feeds/mod.rs` | 4624 | 3 | 2089 | 2535 | 0 | 3 | H | 845 |
| 10 | `tool_load/optimize/mod.rs` | 3361 | 4 | 1009 | 2352 | 0 | 11 | H | 784 |
| 15 | `polygon.rs` | 3004 | 4 | 1105 | 1899 | 0 | 13 | H | 633 |

### Three rows the table simplifies

| row | what the row counts | what the section recommends |
|---|---|---|
| 10 `tool_load/optimize/mod.rs` | the **primary** move only: the five test modules out, parent 1009 | §10 also describes an optional second move (`strategies.rs`, `orchestrate.rs`). The summary excludes it, because §6.4 keeps the production half frozen. |
| 6 `feeds/mod.rs` | the **full** split, parent 2089 | §6.2 recommends the test move ONLY. That leaves the parent at 2765, `out` at 1859 and `value` at 620. |
| 4 `finish/unified_finish.rs` | the **full** split, parent 2668 | §6.2 recommends the test move only for the first pass. That leaves the parent at 3240, `out` at 1566 and `value` at 1566. |

### Where the table overrides a section

Six sections argued a letter this table does not use. Five were written
before the `pub(super)` correction in §3.1b landed, and each cited "a
shared private helper forces a judgement" as its reason for **M**. Under
§3.1b that judgement does not exist: the helper stays in its topic child
with `pub(super)`, and the sibling writes one `use`. The sixth,
`polygon.rs`, moves the other way. So:

| file | section says | table says | reason |
|---|:---:|:---:|---|
| `finish/unified_finish.rs` | M | **L** | 0 sentries, 8 shared helpers; the only sharer is `tests.rs`, a descendant, which needs no visibility edit at all |
| `finish/pencil.rs` | M | **L** | 0 sentries, 13 shared helpers, each crossing one child boundary |
| `finish/scallop.rs` | M | **L** | 0 sentries, 12 shared helpers |
| `session/mutation.rs` | M | **L** | 0 sentries, 1 shared helper; the "large `impl` cut across children" worry does not apply, because inherent methods resolve by type, not by module |
| `dressup/mod.rs` | M | **L** | 0 sentries, 11 shared helpers |
| `polygon.rs` | M | **H** | the split contradicts a ratified P2 ruling (see §6) |

## 5. The first wave — six files

Take the six highest-value **L** files. Together they remove **15 224**
lines from six parents (2486 + 2246 + 1877 + 3152 + 2555 + 2908), and not
one of them touches a source-scanning sentry or a frozen folder.

| order | file | parent after | out | why it is in the wave |
|---:|---|---:|---:|---|
| 1 | `adaptive3d/mod.rs` | 532 | 2486 | One move. The whole file is 82 % test module. Nothing else changes. Start here: it proves the `tests.rs` mechanic on the easiest case. |
| 2 | `stock/simulation_cut.rs` | 895 | 2246 | 1 shared helper. Three clean data clusters plus the test move. |
| 3 | `compute/catalog.rs` | 1410 | 1877 | 2 shared helpers. The registry block (24 `*_PARAMS` consts and 24 `REG_*` statics, about 990 lines) is pure data with no logic. The most mechanical split in the set. |
| 4 | `session/mutation.rs` | 120 | 3152 | The highest value in the whole document. One `impl ProjectSession` of about 70 methods splits by entity; inherent methods stay reachable with zero re-exports. |
| 5 | `dressup/mod.rs` | 2171 | 2555 | Seven banner pairs already mark the child boundaries. The parent's `pub use` block is load-bearing: 62 files outside the crate name `dressup`. |
| 6 | `finish/pencil.rs` | 726 | 2908 | Three detector arms, the chain pipeline and the pass emitter are three distinct topics. |

Do `adaptive3d/mod.rs` first. It is one `git mv` plus two lines, and it
settles the `tests.rs` pattern for the other 16 files before anyone
touches a topic seam.

### Wave 2 — the M files and the remaining L files

`finish/scallop.rs` (L, 2432), `finish/unified_finish.rs` (L, 2138, tests
only — see §6), `session/compute.rs` (M, 2901), `compute/execute.rs`
(M, 2814), `finish/conformal_spiral.rs` (M, 1572) and
`viz state/simulation.rs` (L, 2072) and
`viz ui/properties/operations/mod.rs` (M, 1151).

`session/compute.rs` carries the one precise sentry repoint in the core
crate. See §7.

### Wave 3 — the H files

They need a ruling before they need an implementer. See §6.

## 6. What must not be split, and why

### 6.1 Four functions stay whole

R3 and R4 forbid cutting a function. These four are the reason several
parents cannot reach 1200 lines:

| function | file | lines |
|---|---|---:|
| `calculate` | `feeds/mod.rs` | 1296 |
| `draw_toolpath_panel` | `viz ui/properties/mod.rs` | 1244 |
| `unified_finish_toolpath_with_cancel_and_ceiling` | `finish/unified_finish.rs` | 1173 |
| `handle_mcp_request` | `viz app/mcp.rs` | 416 |

`FEEDS_WAVE.md` row FW-22 already ruled on `calculate`: its step banners
look like a seam, but 16 `let mut` locals cross them. The same reasoning
covers the other three. A later wave may revisit them as a refactor, which
is a different job from a move.

The brief's claim that `feeds/suggest.rs:72 target_chipload` is 621 lines
is **wrong**. The measurement found an 8-line `match` expression inside a
12-line `impl SuggestAggressiveness` block. No long function exists in
that file, and R4 is not needed there.

### 6.2 Two files are "tests only", not a split

| file | do | do not |
|---|---|---|
| `feeds/mod.rs` | Move the 1859-line test module. The parent falls to 2765. | Do not split `calculate` (1296 lines, FW-22). What remains around it does not form two topics of 300 lines each. |
| `finish/unified_finish.rs` | Move the 1566-line test module. The parent falls to 3240. | The `region_routing.rs` child (579 lines) is real but buys little: the parent still floors at about 2668, because 1368 lines of public type and report definitions plus the 1173-line orchestrator both stay by R3 and R5. Schedule it after the first wave, or not at all. |

### 6.3 `polygon.rs` needs an operator ruling first

A split converts `polygon.rs` into `polygon/mod.rs`. `P2_LAYOUT.md` §0 Q1
ratified `polygon` as one of the **8 flat root files** of
`crates/rs_cam_core/src/`, and Q2 named the four files that become a
folder: `io`, `machine`, `dressup`, `material`. `polygon` is not on that
list.

The precedent points both ways. Q2's own rule says a folder's `mod.rs`
carries the folder's principal type, and `material/` already works that
way, so `polygon/mod.rs` holding `Polygon` breaks no principle. But the
operator ratified a root of 8 files, and this changes it.

**Ask before you schedule it.** The question is one line: may
`polygon.rs` become `polygon/mod.rs`, which leaves 7 `.rs` files plus a
`polygon/` folder at the root? The prize is 1899 lines out of a spine
file that 101 files outside the crate name.

### 6.4 The two frozen folders wait for their own wave

`feeds/suggest.rs`, `feeds/mod.rs` and `tool_load/optimize/mod.rs` sit in
`feeds/**` and `tool_load/**`. `FEEDS_WAVE.md` owns those folders, and P2
ruling Q5 allows path edits there but no formula, threshold or control-flow
edit. A test-module move satisfies Q5 exactly: it moves no production
line. A topic split of the production half does not, and it belongs to
the feeds wave, not to P4.

The test-only move is worth taking even inside the freeze:

| file | test lines out | parent after |
|---|---:|---:|
| `feeds/suggest.rs` | 2758 | 3011 |
| `feeds/mod.rs` | 1859 | 2765 |
| `tool_load/optimize/mod.rs` | 2352 (five modules) | **1009** |

`tool_load/optimize/mod.rs` is the strongest case in the whole document
for a test-only move. Its production half is 1009 lines. Move all five
test modules and the file stops being a size problem, with no production
edit at all.

Note for that folder only: `optimize/mod.rs:49` declares `mod policy;`
privately, so `dead_code` DOES fire inside `optimize/`. A new private
child module grows that surface. Run
`cargo clippy -p rs_cam_core --all-targets -- -D warnings` after the move.

### 6.5 The two heavy viz files are not a first-wave job

`viz app/mcp.rs` (10 exact-path sentries) and
`viz ui/properties/mod.rs` (8) carry the entire source-scanning load of
this programme. Every one of those sentries must be repointed in the same
commit that moves its needle, and
`command_surface_completeness.rs:137 MCP_SOURCES` must gain a row for each
new child. Their sections map each needle to the child that would carry
it. Read that mapping before scheduling either file.

## 7. The precise sentry facts

`crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs:82` holds

```rust
const SESSION_COMPUTE_SRC: &str = include_str!("../src/session/compute.rs");
```

Its test `every_numeric_named_arm_calls_the_shared_range_helper` slices
six ranges out of that text — from `"feed_rate" =>` through the
`// Invalidate cached result` comment — and asserts each one calls
`check_param_range(`. Those `match` arms live inside
`set_toolpath_param_impl`. If the split moves that method to a child, the
`include_str!` must move with it, in the same commit.

`crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs:41`
holds `STRUCT_FILE = "src/session/compute.rs"` and looks for the literal
line `pub struct ResolvedGenInputs {`. Its second test walks every `.rs`
file under `src/` recursively, so the producer may move freely. **The
struct declaration may not.** Keep `pub struct ResolvedGenInputs` in
`session/compute.rs` and this sentry needs no edit.

No other core file in the set has an exact-path reader.

### 7.1 Two sentries pin text that a split MOVES, in `rs_cam_viz`

These two are the traps. Each fails with a panic, not a soft assertion,
and neither is fixed by adding a child to a file list.

**`workspace_menu_complete_g_wsmenu.rs:165`** asserts

```rust
MCP_SRC.contains("for ws in Workspace::ALL {")
```

where `MCP_SRC` is `include_str!("../src/app/mcp.rs")`. That string sits
at `app/mcp.rs:6059`, inside the file's own `#[cfg(test)] mod tests`,
which starts at 5364. So the **test-module move breaks this sentry**,
although nothing about the behaviour changes. Adding
`src/app/mcp/tests.rs` to `MCP_SOURCES` does not fix it: this is a second,
independent `include_str!`. The fix is to read the child file too.

**`bottom_z_pin_note_g_bottompin.rs:143-159`** slices
`ui/properties/operations/mod.rs` from `"pub(super) fn draw_heights_params"`
to the literal `"\n// ── Stepover Pattern Diagram"` and asserts that the
slice names `bottom_z_pin_note`. Both ends are needles. The `find` for the
end marker carries `.expect(...)`, so a split that **deletes that banner
comment panics the test**. Section 19 keeps `draw_heights_params` and
`bottom_z_pin_note` in the parent, so the start needle is safe. Leave the
one-line banner comment in the parent as well, even though the code it
heads moves to `operations/shape_diagrams.rs`.

## 8. The gate for every split commit

Run these, in order, through `scripts/cargo_lane.sh`:

1. `scripts/cargo_lane.sh check --workspace --all-targets`
2. `scripts/cargo_lane.sh test -p <crate> -q --lib <module path>::`
3. the file's own sentries and integration targets, named in its section
4. `scripts/cargo_lane.sh clippy --workspace --all-targets -- -D warnings`
5. `cargo fmt --all -- --check`

Do not run the full heavy gate. The operator ruled on 2026-09-11: "dont
run the large test gates. they take forver." A split commit changes no
behaviour, so the `--lib` filter for its module plus the named sentries
is the right proof.

Known pre-existing reds to ignore: viz controller `..._ur3`,
`simulation_staleness_tracks_edits`,
`freshness_does_not_outrank_a_collision`, viz dc6
`off_workspace_run_producers_hold_their_recorded_ruling_ur3`, core f036b
`modulation_raises_cutting_chipload_toward_band`.

---

# Per-file plans

## 1. `crates/rs_cam_core/src/session/compute.rs` — 8015 lines

The file holds the compute side of `ProjectSession`: the generation types
(`ResolvedGenInputs`, `GenContext`, `GenerateToolpathHandle`, `GenObserver`),
the top-level generation entry points (`execute_job`, `execute_generation`),
the advisor/optimize handles, and two `impl ProjectSession` blocks that carry
every compute method — parameter mutation, generation-input resolution,
boundary clipping, simulation, feed modulation, diagnostics, triage, export
and report. The split follows the topic seam inside the 3191-line second
`impl` block, plus the drop of the two small helper impls out of the first.
Method names are the contract; an implementer relocates each item by name,
not by the line numbers below — they drift as soon as the first child moves.

**Measured.** 8015 lines. Top-level items: 18 `fn`, 9 `impl`, 12
`struct`/`enum`, 1 `const`, 1 `mod` (tests). One digest correction: the row
`priv type 5092 397 RapidWorstByBoundary` is not a real top-level item. It is
a local `type` alias declared inside the body of `diagnostics_with_evidence`,
which runs 5074–5488 (415 lines, not 17) and swallows both local aliases
(`RapidCountsByBoundary`, `RapidWorstByBoundary`). The corrected span still
lines up exactly with the next item, `export_gcode` at 5489, and the file's
line total is unaffected — this was a mislabel, not a miscount.
Inline `#[cfg(test)] mod tests`: lines 5976–8015 (2040 lines).
Banners: L1103 (Phase 3 plunge-rate binding), L2220 (Stage 4 engagement),
L2459 (Phase 3 operation plunge rate), L5046 (Phase 4 plunge-class backstop).

### Proposed children

All children sit under `crates/rs_cam_core/src/session/compute/` (the parent
is `compute.rs`, a `<stem>.rs` file, so rule 6 names the test child
`compute/tests.rs`).

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `compute/params.rs` | `strip_outer_quotes`, `unknown_param_error`, `check_param_range`, the first `impl ProjectSession` (`set_toolpath_param`, `set_toolpath_param_impl`, `operation_schema`, `set_tool_param`, `set_tool_param_impl`, `recommend_clearing_strategy`, `capture_recommend_clearing_strategy`, `capture_optimize_toolpath`) | 38–95, 1125–1738 | ~672 | `strip_outer_quotes` stays `pub(crate)` (cross-file use, see Break cost); the rest keep their existing `pub`/`pub(crate)`/private marks — private items (`unknown_param_error`, `check_param_range`) have no caller outside this file |
| `compute/generation.rs` | `resolve_generation_inputs`, `resolve_derived_rest_region_polys`, `resolve_containment_polygon`, `apply_boundary_clip`, `resolve_collapsed_containment`, `apply_boundary_clip_multi` (all methods, second `impl ProjectSession`) | 2665–3882 (selected methods only) | ~961 | all already `pub`/`pub(crate)`; unchanged — inherent methods, reachable via `session.method()` from anywhere in the crate with no re-export |
| `compute/simulation.rs` | `translate_mesh`; methods `build_sim_request`, `run_simulation`, `modulate_annotated_against_trace`, `modulate_simulation_trace`, `reintegrate_toolpath`, `apply_adaptive_feed_modulation` | 128–137, 2616–2664, 3921–4630 | ~769 | methods keep existing `pub`/`priv`; `translate_mesh` moves with its sole caller (`run_simulation`) and stays private — same-file, no bump |
| `compute/diagnostics.rs` | methods `collision_check` … `diagnostics_with_evidence` (corrected span), plus `AirCutScan`, `AirCutOffender`, `AirCutAbstention`, `air_cut_offenders_for_toolpaths`, `plunge_stress_offenders_for_session` | 4631–5488, 5800–5975 | ~1000 | methods keep existing `pub`/`priv`; the three `AirCut*` structs and the two free finders become `pub(super)` (tests, a sibling under `session::compute`, calls them directly — see Break cost) |
| `compute/export.rs` | `transform_bbox_world_to_local`; methods `export_gcode` … `diagnose_project_with_evidence` | 96–127, 5489–5799 | ~343 | unchanged; `transform_bbox_world_to_local` moves with its sole caller (`height_context_for_toolpath`) and stays private |
| `compute/tests.rs` | the inline `mod tests` body | 5976–8015 | 2040 | `#[cfg(test)] mod tests;` stays in the parent per rule 6 |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `ResolvedGenInputs` struct + impl, `GenContext` struct + impl, `GenerateToolpathHandle` struct + impl, `GenObserver` struct + 2 impls | 138–543 (~406) | type definitions (rule 5); `ResolvedGenInputs`'s declaration is read by `resolved_gen_inputs_has_one_producer.rs` by exact path — must stay here |
| `execute_job`, `execute_generation` | 544–1044 (~501) | the file's two top public generation entry points (rule 5) |
| `ADVISOR_CANDIDATE_STRATEGIES`, `regime_from_suggest_warnings`, `regime_from_binding` | 1045–1124 (80) | used by both the params child (first `impl` block) and the advisor cluster below — a private item two children call; kept in the parent so both reach it as an ordinary private name with no visibility change (rule: "…or it stays in the parent") |
| `AdvisorContext`, `RecommendClearingStrategyHandle` + impl, `execute_recommend_clearing_strategy`, `OptimizeToolpathHandle` + impl, `execute_optimize_toolpath`, `optimized_candidate`, `FeedContext`, `modulate_annotated_against_trace` (free fn), `SimRequestContext`, `build_sim_request` (free fn), `simulate_candidate_isolated`, `auto_resolution_for_groups` | 1739–2608, 5948–5975 (~904) | `execute_recommend_clearing_strategy` / `execute_optimize_toolpath` are public entry points (rule 5); their supporting structs (`AdvisorContext`, `FeedContext`, `SimRequestContext`) and `auto_resolution_for_groups` are each called from **this** cluster *and* from the simulation child's methods (`build_sim_request`, `run_simulation`, `modulate_annotated_against_trace`) — a genuine multi-child shared helper. Kept in the parent, not promoted, because a simulation-child method calling a parent-private item needs no visibility change at all (a child module is always a descendant of its parent). An equally valid alternative: home this whole cluster in its own `compute/advisor.rs` child instead, mark the four shared items `pub(super)`, and have `simulation.rs` reach them with `use super::advisor::FeedContext;` (etc.) — either design is correct; this report keeps the lower-churn one. |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. `session::compute` has zero
  out-of-crate references and exactly one in-core file naming it.
- Cross-file, in-crate: `crates/rs_cam_core/src/session/mutation.rs:587,597`
  calls `crate::session::compute::strip_outer_quotes` by full path. Moving
  `strip_outer_quotes` into `compute/params.rs` requires the parent to add
  `pub(crate) use params::strip_outer_quotes;` so that exact path keeps
  resolving — this is the one real path a child move can break in this file.
- `use super::` lines the children need: none beyond the `AirCut*`
  pub(super) case below — every other cross-reference is either a method
  call (`self.method()`, works regardless of file) or a child calling a
  parent-private item (works automatically).
- Shared private helpers: `ADVISOR_CANDIDATE_STRATEGIES`,
  `regime_from_suggest_warnings`, `AdvisorContext`, `FeedContext`,
  `SimRequestContext`, `auto_resolution_for_groups`, `modulate_annotated_against_trace`
  (free fn) — all kept in the parent (see table above), reachable by every
  child with no visibility edit. `AirCutScan`/`AirCutOffender`/`AirCutAbstention`,
  `air_cut_offenders_for_toolpaths`, `plunge_stress_offenders_for_session` —
  homed in `compute/diagnostics.rs`, marked `pub(super)` because
  `compute/tests.rs` (a sibling) calls them directly in ~15 test functions;
  `pub(super)` on an item in `session::compute::diagnostics` is visible
  throughout `session::compute` and all its descendants, `tests.rs` included.

### Test module

2040 lines, over the 500-line threshold → moves to `session/compute/tests.rs`
per rule 6 (parent is `compute.rs`, a `<stem>.rs` file). Leave
`#[cfg(test)] mod tests;` in the parent. The test module uses `super::*`
already; after the split it also needs `use super::diagnostics::*;` (or the
named items) for the `AirCut*`/`air_cut_offenders_for_toolpaths`/
`plunge_stress_offenders_for_session` calls, since those are no longer
directly in `session::compute`'s own namespace.

### Risk and gate

- Risk: **M** — two exact-path sentries read this file; both are satisfied
  by keeping their needles in the parent (see below), but a mistaken move
  turns either one red silently.
- `--lib` filter: `cargo test -p rs_cam_core --lib session::compute::`
- integration `--test` targets: `set_param_refuses_absent_field_n5`,
  `resolved_gen_inputs_has_one_producer`
- source-scanning sentries:
  - `set_param_refuses_absent_field_n5.rs:82` reads the whole file as
    `SESSION_COMPUTE_SRC` and, at line ~416–424, slices out six match-arm
    ranges (`"feed_rate" =>` through the `// Invalidate cached result`
    comment) and asserts each calls `check_param_range(`. Those arms live
    inside `set_toolpath_param_impl`, which this plan moves into
    `compute/params.rs`. **Repoint the sentry's `include_str!` to
    `"../src/session/compute/params.rs"` (or wherever `set_toolpath_param_impl`
    lands) in the same commit that moves it** — do not leave it reading the
    parent.
  - `resolved_gen_inputs_has_one_producer.rs:41` reads `STRUCT_FILE =
    "src/session/compute.rs"` and looks for the literal line
    `pub struct ResolvedGenInputs {`. This plan keeps that struct in the
    parent (it is a type definition, rule 5), so **this sentry needs no
    repoint**. Its second check — that exactly one function in the crate
    returns `ResolvedGenInputs` — walks every `.rs` file under `src/`
    recursively, so it is split-safe regardless of where
    `resolve_generation_inputs` ends up.


## 2. `crates/rs_cam_core/src/compute/execute.rs` — 6363 lines

The file is the single dispatch point for all 23 toolpath operations: the
error/result types, a long run of `record_*` finding recorders around
`GenerationFindings`, the drill-pick resolution block, 24
`pub(crate) fn generate_<op>` adapters (one per operation, grouped into
families: drilling, 2.5D clearing, curve/V-bit engrave, and 3D finishing),
the three `execute_operation*` entry points, the dressup-apply block, and a
handful of small validation guards used by nearly every adapter. The split
follows the operation families; the guards and recorders that no single
family owns become two small, explicitly-shared children instead of bloating
the parent. As in section 1, an implementer relocates every item by symbol
name — grep for the `fn`/`struct`/`const` name, not the line number.

**Measured.** 6363 lines. Top-level items: 73 `fn`, 3 `impl`, 3 `struct`,
2 `type`, 6 `const`, 1 `enum`, 5 `mod` (4 tiny `#[cfg(test)] mod <name>;`
pointers to already-separate files, plus the inline `tests`).
Inline `#[cfg(test)] mod tests`: lines 4584–6363 (1780 lines).
Banner: L2045 (Stage 4, planner-predicted engagement).

### A visibility fact this file leans on

A `pub(super)` item declared inside a child module is visible not only to
that child's parent, but to **every descendant of the parent** — which
includes every sibling child. So a helper used by two or three of the
proposed children below does not need to move to the parent or to a
`shared.rs`: it stays in whichever child owns its topic, marked `pub(super)`,
and the other children write `use super::<that_child>::<helper>;`. Only a
helper with no owning topic at all goes into a real `shared.rs`.

### Proposed children

All children sit under `crates/rs_cam_core/src/compute/execute/`.

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `execute/shared.rs` | `require_polygons`, `require_mesh`, `require_index`, `with_depth_run_annotation`, `generated_with_spans` + its 3 one-line wrappers (`generated_with_depth_run_spans`, `generated_with_cut_run_spans`, `generated_with_drill_spans`) | 4456–4544 | ~71 | all become `pub(super)` — every family child calls at least one of these; none has a single owning family |
| `execute/findings.rs` | the `record_*` recorder functions (`record_truncated_core` … `record_derived_stepover`), `zero_removal_engagement_floor_mm` | 188–533 | ~374 | private helpers become `pub(super)`; `record_offset_library_failures` (`pub(crate)`) and `record_boundary_clip_dropped` (`pub`) keep their marks — see Break cost |
| `execute/drilling.rs` | `build_drill_op_for_config`, `pick_to_emission_frame`, `pin_holes_in_emission_frame`, `NO_DRILL_TARGETS_MSG`, `NO_DRILL_TARGETS_SELECTED_MSG`, `drill_targets_refusal`, `DRILL_PICK_MATCH_EPS_MM`, `drill_pick_matches`, `STALE_DRILL_PICKS_PHRASE`, `stale_drill_picks_refusal`, `resolve_drill_picks`, `drill_holes_for_config`, `generate_drill`, `generate_alignment_pin_drill` | 591–700, 867–1224 | ~468 | `build_drill_op_for_config` is `pub` — parent re-exports it; `generate_drill`/`generate_alignment_pin_drill` stay `pub(crate)`; the rest are private and used only inside this cluster |
| `execute/clearing_2d.rs` | `generate_rest`, `generate_zigzag`, `generate_trace`, `generate_profile`, `generate_pocket`, `generate_face`, `generate_adaptive`, `effective_levels` | 1225–1867, 4510–4544 | ~550 | `generate_*` stay `pub(crate)`; `effective_levels` is private, used only by this family (rest/zigzag/trace/profile/pocket/adaptive) |
| `execute/curve_engrave.rs` | `generate_inlay`, `generate_vcarve`, `generate_chamfer`, `vbit_half_angle`, `generate_project_curve`, `chain_project_curve` | 1297–1425, 2052–2113, 2254–2355, 4481–4494 | ~304 | `generate_*` stay `pub(crate)`; `vbit_half_angle`/`chain_project_curve` are private, single-family use |
| `execute/finish_raster.rs` | `generate_scallop`, `generate_drop_cutter`, `generate_waterline`, `finishing_link_stage`, `relink_in_adapter` | 2173–2210 (helpers), 2468–2572, 3123–3355 | ~478 | `generate_*` stay `pub(crate)`; `finishing_link_stage`/`relink_in_adapter` are private, used only by these three ops |
| `execute/finish_3d.rs` | `adaptive3d_effective_stock_to_leave`, `generate_adaptive3d`, `generate_pencil`, `generate_unified_finish`, `generate_steep_shallow`, `generate_ramp_finish`, `generate_spiral_finish`, `generate_radial_finish`, `generate_horizontal_finish` | 1868–2051, 2356–2467, 2573–3122 (minus the raster three) | ~846 | `generate_*` stay `pub(crate)`; `adaptive3d_effective_stock_to_leave` is private, single-caller |
| `execute/dressup_apply.rs` | `attach_generic_rest_analysis`, `GENERIC_REST_STEPOVER_DEPTH_BASIS`, `SHALLOW_SLOPE_DERATE_BASIS`, `resolve_rest_reference`, `DressupTraceInfo`, `apply_dressup_traced`, `internal_link_ceiling_z`, `toolpath_is_drill_cycle`, `apply_dressups` | 3677–4455 | ~779 | `apply_dressups` is `pub` — parent re-exports it; `attach_generic_rest_analysis` becomes `pub(super)` (the parent's own `execute_operation_annotated_with_regions` calls it — `pub(super)` reaches the parent too); the rest are private, used only within this cluster |
| `execute/tests.rs` | the inline `mod tests` body | 4584–6363 | 1780 | `#[cfg(test)] mod tests;` stays in the parent per rule 6 |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `OperationError` enum + its 3 trait impls, `GeneratedToolpath`, `GenerationFindings` | 28–187, 701–721 | the file's core public result/error/findings types (rule 5) |
| `ExecutionContext`, `GenerateFn` | 722–866 | public type definitions the whole dispatch mechanism is built on (rule 5) |
| `execute_operation`, `execute_operation_annotated`, `execute_operation_annotated_with_regions` | 3356–3676 | the file's public entry points (rule 5); they dispatch into every family child by calling `pub(crate)` `generate_*` functions, which need no re-export to be callable from anywhere in the crate |
| 4 tiny `#[cfg(test)] mod <name>;` declarations (`pinned_bottom_z_reaches_motion_g_bottompin`, `project_curve_chaining`, `unified_finish_ring_collapse_g_unifiedcrash`, `unified_finish_semantic_regions`) | 4545–4583 (~39) | each is a 2–11 line pointer to a test file that already lives outside this one; nothing to move |

### Break cost

- `crate::` / `rs_cam_core::` path edits without a parent re-export: 21
  occurrences across 15 files outside `rs_cam_core` name `compute::execute`
  (plus 11 in-core files). With the parent re-exporting every `pub` item
  this plan moves (`build_drill_op_for_config`, `apply_dressups`,
  `record_boundary_clip_dropped`), that cost is 0.
- `use super::` lines the children need: `execute/shared.rs` and
  `execute/findings.rs` are consumed by every family child
  (`clearing_2d.rs`, `curve_engrave.rs`, `finish_raster.rs`, `finish_3d.rs`,
  `dressup_apply.rs`) via `use super::shared::*;` / named imports from
  `super::findings::*`; `execute/tests.rs` needs the same two, plus
  `super::drilling::drill_holes_for_config` and
  `super::finish_3d::adaptive3d_effective_stock_to_leave` for its direct
  test calls.
- Shared private helpers and where they land: `require_polygons`,
  `require_mesh`, `require_index`, `with_depth_run_annotation`,
  `generated_with_spans` family → `execute/shared.rs` (`pub(super)`, no
  single owning family). The `record_*` recorder cluster → `execute/findings.rs`
  (`pub(super)`; `record_derived_stepover` in particular is called from both
  `finish_3d.rs` (`generate_unified_finish`) and `dressup_apply.rs`
  (`attach_generic_rest_analysis`), which is exactly the case `pub(super)`
  in a home child resolves without a `shared.rs`).

### Test module

1780 lines, over the 500-line threshold → moves to `compute/execute/tests.rs`
per rule 6. Leave `#[cfg(test)] mod tests;` in the parent. Its `use super::*;`
needs supplementing with the family-child and `shared`/`findings` imports
noted above.

### Risk and gate

- Risk: **M** — no exact-path sentry reads this file, but the `record_*`
  cluster and the validation-guard trio (`require_polygons`/`require_mesh`/
  `require_index`) are genuinely shared across 3+ proposed children, which
  is the "shared private helper forces a judgement" case in the risk scale
  even without a sentry.
- `--lib` filter: `cargo test -p rs_cam_core --lib compute::execute::`
- integration `--test` targets: none (not in the exact-path-reader list)
- source-scanning sentries: none


## 3. `crates/rs_cam_core/src/feeds/suggest.rs` — 5769 lines

This file is the bridge from an `OperationConfig` into the feeds calculator
and the write-back path from a `FeedsResult` into an operation. It holds the
Suggest data model (context, warnings, policy), the read side
(`suggest_params`, `suggest_for_operation`), the write-back side (`apply`
and its field-scoped variants), and four private computation clusters:
axial-envelope picking, invariant clamping, adaptive3d entry/clearing
choice, and final-geometry rescaling. The split follows those four
clusters. Two corrections apply here and to every file below: line numbers
drift as peer sessions commit (a fresh check on 2026-09-17 shows this file
at 5769 lines, not the digest's 5770), so the implementer must locate every
item by its symbol name and re-run `grep -n 'fn <name>\|struct <name>'`
before moving it, never by the line numbers in this section. Second, a
private helper that two or more children call does not need a shared
module: it stays in the child that owns its topic, marked `pub(super)`, and
the other child writes `use super::<owning_child>::<helper>;` — the parent
gets the same access for free, because `pub(super)` reaches the defining
item's parent module too.

**Correction to the brief's own hint:** the note "`target_chipload` at line
72 is 621 lines" does not match the file. `SuggestAggressiveness::target_chipload`
at line 72 is an 8-line match expression inside a 12-line `impl` block
(69–80). There is no 621-line function anywhere in this file; the largest
functions are `rescale_feed_to_final_geometry` (237 lines) and
`apply_feeds_subset` (149 lines), both of which move whole to a child
below. Ruling 2 is not needed here — nothing in this file is a single
unsplittable giant of that size.

**Measured.** 5769 lines (digest recorded 5770; treat the file itself as
authoritative). 65 top-level items: 12 `struct`, 5 `enum`, 37 `fn`, 6
`impl`, 9 `const`, 1 `mod` (tests). Inline `#[cfg(test)] mod tests`: lines
3019–5770 (2752 lines). Banners: one incidental comment at L4597 inside the
test module ("Phase 3 (Pass 0, axial-DOC envelope)"), not a production
banner.

Because the test module forces a `<stem>/tests.rs` extraction (ruling 6)
and the parent is a bare `suggest.rs`, the split converts it into a
directory: `feeds/suggest.rs` becomes `feeds/suggest/mod.rs` plus children.
`feeds/mod.rs:30`'s existing `pub mod suggest;` needs no edit — Rust
resolves that declaration to either shape.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `feeds/suggest/apply.rs` | `ApplyScope`, `apply_feeds_subset` (priv), `apply_feeds_result_to_op`, `apply_speeds_to_op`, `apply_cut_geometry_to_op`, `FieldApplyPreview`+impl, `FieldApplyPreviews`+impl, `preview_field_applies`, `preview_field_apply`, `FeedsPreview`+impl, `ApplicableRecommendation`+impl, `ApplyContext`, `apply`, `resolve_operation_invariants`, `feeds_preview_for_operation`, `apply_drill_defaults`, `clamp_peck_to_depth` (priv), `apply_stock_defaults`, `operation_feeds_hints` | 853–1677 | ~825 | `pub` items stay `pub`; `apply_feeds_subset` becomes `pub(super)` (invariants.rs and adaptive_entry.rs call it); `clamp_peck_to_depth` stays private (used only here) |
| `feeds/suggest/axial_envelope.rs` | `apply_axial_envelope` (priv), `axial_binding_str` (priv), `axial_envelope_for_operation` (`pub(crate)`), `pick_axial_envelope` (priv), `recompute_chipload_bounds_for_dpp` (priv) | 1773–2156 | ~384 | `axial_envelope_for_operation` stays `pub(crate)`, parent adds `pub(crate) use axial_envelope::axial_envelope_for_operation;`; `pick_axial_envelope` becomes `pub(super)` (apply.rs, invariants.rs, adaptive_entry.rs, and tests.rs all call it) |
| `feeds/suggest/invariants.rs` | `enforce_invariants` (priv), `clamp_plunge_to_feed`, `clamp_stepover_to_diameter`, `backoff_stepover_for_runtime`, `clamp_dpp_to_rigidity`, `clamp_dpp_to_cutting_length`, `backoff_dpp_for_deflection` (all priv), plus the eight backoff/threshold consts `DEFLECTION_BACKOFF_TARGET_UM`, `DEFLECTION_BACKOFF_DPP_FLOOR_MM`, `DEFLECTION_BACKOFF_FACTOR`, `DEFLECTION_BACKOFF_MAX_ITERATIONS`, `STEPOVER_BACKOFF_TARGET_MOVES`, `STEPOVER_BACKOFF_DIAMETER_FRACTION`, `STEPOVER_BACKOFF_FACTOR`, `STEPOVER_BACKOFF_MAX_ITERATIONS` | 2157–2491 (fns) + 1678–1757 (consts) | ~415 | `enforce_invariants` becomes `pub(super)` (the parent's `SuggestContext`/`CalculatorOperatingPoint`/`SuggestWarning`/`ApplyScope` and apply.rs all call it); the consts become `pub(super)` (`STEPOVER_BACKOFF_*` is also read by `SuggestWarning`, which stays in the parent) |
| `feeds/suggest/adaptive_entry.rs` | `pick_adaptive3d_entry_style`, `pick_adaptive3d_clearing_strategy`, `check_plunge_entry_stability`, `geometry_feed_factor`, `rescale_feed_to_final_geometry`, `adaptive_plunge_entry_label` (all priv), plus `PLUNGE_ENTRY_UNSTABLE_DPP_OVER_D` (priv, used only here) | 2492–2999 (fns) + 1758–1772 (const) | ~523 | all stay private except `pick_adaptive3d_entry_style` and `pick_adaptive3d_clearing_strategy`, which become `pub(super)` (`enforce_invariants` in invariants.rs calls both); `rescale_feed_to_final_geometry` becomes `pub(super)` (`enforce_invariants` calls it too) |
| `feeds/suggest/tests.rs` | `mod tests` | 3019–5770 | 2752 | unchanged (`#[cfg(test)]`) |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `StockContext`+impl, `SuggestAggressiveness`+impl, `SuggestScope`, `SuggestPolicy`, `SuggestContext`, `CalculatorOperatingPoint`, `SuggestWarning`, `FeedRecalibrationCap`, `SuggestedParams`, `SuggestParamsInput`, `SuggestForOperationInput` | 18–682 | type definitions (ruling 5); this is the Suggest data model |
| `suggest_params`, `suggest_for_operation`, `feeds_input_for_operation` (priv, shared by three entry points below), `feeds_result_for_operation`, `feeds_explain_for_operation` | 683–852 | the module's headline read-side entry points, named in the module doc |
| `round_suggestion_value` | 3000–3018 | small `pub` rounding utility used by callers outside this file; not worth relocating |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: 86 occurrences across 26 out-of-crate files that name
  `feeds::suggest::…` directly, plus 7 core files.
- `use super::` lines the children need: `apply.rs` needs
  `use super::axial_envelope::pick_axial_envelope;` and
  `use super::invariants::enforce_invariants;`; `invariants.rs` needs
  `use super::apply::apply_feeds_subset;`,
  `use super::axial_envelope::pick_axial_envelope;`,
  `use super::adaptive_entry::{pick_adaptive3d_entry_style, pick_adaptive3d_clearing_strategy, rescale_feed_to_final_geometry};`;
  `adaptive_entry.rs` needs `use super::apply::apply_feeds_subset;` and
  `use super::axial_envelope::pick_axial_envelope;`; the parent needs
  `use invariants::enforce_invariants;` for `SuggestContext`,
  `CalculatorOperatingPoint`, and `SuggestWarning`.
- shared private helpers: `apply_feeds_subset` (home: apply.rs, called by
  invariants.rs and adaptive_entry.rs), `pick_axial_envelope` (home:
  axial_envelope.rs, called by apply.rs, invariants.rs, adaptive_entry.rs,
  and tests.rs), `enforce_invariants` (home: invariants.rs, called by the
  parent's type impls and by apply.rs). Full per-item call lists are in
  `coupling.txt`'s `feeds/suggest.rs` block for anything not named here.

### Test module

3019–5770, 2752 lines. Moves to `feeds/suggest/tests.rs` (ruling 6, stem
parent). Parent keeps `#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **H** — `feeds/**` is a frozen folder owned by the power-calcs
  session (`planning/structure_2026-09-17/FEEDS_WAVE.md`); any file here
  carries the extra risk step and stays out of a first wave regardless of
  its own coupling shape.
- `--lib` filter: `cargo test -p rs_cam_core --lib feeds::suggest::`
- integration `--test` targets: none read this file by exact path; the
  crate's Suggest-focused integration tests exercise it through its public
  API and stay green as-is, e.g. `wanaka_suggest_integration.rs`,
  `suggest_feed_matches_final_geometry.rs`,
  `apply_reports_a_field_it_cannot_hold_g_notheld.rs` (9 files call
  `feeds::suggest::` directly per a repo grep; none require edits).
- source-scanning sentries: none. `feeds/suggest.rs` is not in the
  exact-path reader list; only the frozen-folder step applies.


## 4. `crates/rs_cam_core/src/finish/unified_finish.rs` — 4807 lines

The file holds the P2.c/P2.d unified finishing pass: the parameter and
report type model (`UnifiedFinishParams`, `ClaimsConfig`, `RegionKind`,
`UnifiedFinishReport`, and related types), the two public entry functions,
and one orchestrator, `unified_finish_toolpath_with_cancel_and_ceiling`
(1173 lines), that decomposes a mesh into regions and routes bands between
them. Ruling 2 keeps that function whole in the parent. The seam sits
after it: 14 private items (greedy link routing, shallow raster sampling)
that only the orchestrator and the test module call, and the 1560-line
test module that ruling 6 moves out. Because ruling 5 keeps every public
type and entry point in the parent, and the orchestrator cannot split, the
parent cannot reach 1200 lines. The achievable floor is about 2668 lines:
1368 lines of type definitions and small entry functions, plus the 1173-line
orchestrator, plus a small header.

**Measured.** 4807 lines. 54 top-level items:
19 `fn`, 8 `impl`, 23 `struct`/`enum`, 3 `const`/`static`.
Inline `#[cfg(test)] mod tests`: lines 3248–4807 (1560 lines).
Banners: none.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `finish/unified_finish/region_routing.rs` | `RegionPath`, `JunctionChoice`, `SHALLOW_DERATE_MIN_SLOPE_DEG`, `build_shallow_raster_grid`, `shallow_region_max_slope_deg`, `region_sampling_window`, `band_rank`, `strippable_preamble`, `trailing_retracts`, `route_greedy`, `choose_link`, `band_z_range`, `rest_grid_index`, `cutting_length_mm` | 2669–3247 | 579 | all were `priv`; become `pub(super)` so the parent's orchestrator and the sibling `tests` module can both reach them |
| `finish/unified_finish/tests.rs` | `mod tests` body | 3248–4807 | 1560 | stays `#[cfg(test)]`; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `UnifiedFinishParams`, `ClaimsConfig`, `RegionKind`, `RegionStrategy`, `ClaimsReferenceResolution`, `UnifiedFinishReport`, and the other public param/report types, 128–1495 | 1368 | ruling 5: the parent keeps the type definitions and the small public functions built directly on them (`unified_finish_spans`, `clipped_band_finding`, `dropped_band_finding`, `unified_finish_region_annotations`, the two resolution helpers — 198 of these 1368 lines); none of these groups reaches the 300-line child minimum on its own, and splitting them off a type they read gains nothing |
| `unified_finish_toolpath_with_cancel`, `unified_finish_toolpath_with_cancel_and_ceiling` | 46 + 1173 = 1219 | the two public entry points; the second is a single 1173-line item ruling 2 forbids splitting |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. Every item that moves is
  currently `priv`; no external caller names it. The 34 occurrences across
  31 files that reach `finish::unified_finish` (brief's external-usage
  table) all use the param/report types or the two entry functions, none
  of which move.
- `use super::` lines the children need: `region_routing.rs` needs
  `use super::{RegionKind, ...}` for the report/annotation types it reads,
  which stay in the parent. `tests.rs` needs `use super::*;` plus
  `use super::region_routing::*;` for the routing helpers it exercises
  directly.
- shared private helpers: all 14 `region_routing.rs` items are used by
  both the parent's orchestrator and by `tests.rs`. `pub(super)` on each
  resolves both cases at once, because `tests.rs` is also a descendant of
  the parent module — no third "shared" file is needed.

### Test module

Lines 3248–4807 (1560 lines) → `finish/unified_finish/tests.rs`. Parent
keeps `#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **M** — no sentry reads this file, but the 14 routing helpers are
  a genuine shared-visibility judgement between the parent's orchestrator
  and the test module (coupling.txt confirms every one of them is used by
  both).
- `--lib` filter: `cargo test -p rs_cam_core --lib finish::unified_finish::`
- integration `--test` targets: none — the public API (types and the two
  entry functions) does not move, so no `--test` target needs an edit.
- source-scanning sentries: none.


## 5. `crates/rs_cam_core/src/dressup/mod.rs` — 4726 lines

Dressups are post-processing transforms on a toolpath: entry (ramp/helix),
entry-descent optimization, tab/bridge, lead-in/out, dogbone, link-vs-retract,
and air-cut filtering. The file already reads as seven sections, each
opened by a real `// ---` banner pair naming the topic, so the split follows
those seven banners rather than inventing new boundaries. Within each
section, the `pub fn apply_*`/`optimize_*` entry points and the `pub`
result types stay in the parent (ruling 5); the private step machinery
behind them moves. Two sections (Tab/Bridge, Dogbone) have no private
helpers at all and do not produce a child. As with every file in this
batch, the implementer locates every item by symbol name — `grep -n 'fn
apply_tabs\|struct Tab'`, not by the line numbers below — because the tree
moves under concurrent sessions (a fresh check today shows this file at
4726 lines, one less than the digest's 4727).

**Measured.** 4726 lines. 45 top-level items: 7 `struct`, 3 `enum`, 37
`fn`, 1 `impl`, 9 `const`, 7 `mod` (6 pre-existing submodule declarations
plus the inline test module). Inline `#[cfg(test)] mod tests`: lines
3164–4727 (1564 lines). Banners (confirmed by reading the source, all real
section markers): L50–52 "Ramp / Helix entry", L283–285 "Entry-descent
optimization (P1 W2, reworked)", L1349–1351 "Tab / Bridge dressup",
L1552–1554 "Lead-in / Lead-out dressup", L2015–2017 "Dogbone / overcut
dressup", L2182–2184 "Link-vs-Retract dressup", L2576–2578 "Air-cut filter
dressup".

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `dressup/entry_descent.rs` | `upcoming_run`, `find_next_xy_direction`, `fold_walk_budget`, `FOLD_ARC_SPACING_MM`, `collect_following_cut`, `ENTRY_CLEARANCE` (`pub(crate)`), `RampFold` (`pub(crate)`), `FOLD_EXTEND_MAX_ROUNDS`, `extend_fold_path`, `walk_fold_path`, `fold_ramp_points`, `emit_ramp` (`pub(crate)`), `ENTRY_CLIP_SPACING_MM`, `clip_polyline_to_floor`, `emit_helix` (`pub(crate)`) | 706–739, 753–1352 (excludes `is_plunge`, see below) | ~634 | `ENTRY_CLEARANCE`, `emit_ramp`, `emit_helix` stay `pub(crate)`; the rest become `pub(super)` |
| `dressup/link.rs` | `LINK_Z_MATCH_TOL`, `LINK_CORRIDOR_LOOKBACK_MOVES`, `point_segment_distance_xy`, `bridge_corridor_is_swept` (all priv) | 2202–2372 | ~171 | `bridge_corridor_is_swept` becomes `pub(super)` (parent's `apply_link_moves`/`apply_link_moves_with_provenance` call it) |
| `dressup/air_cut.rs` | `tip_depth_below_surface`, `is_in_air`, `sample_is_air_for_tool`, `material_above_cutter`, `swept_path_is_all_air` (all priv) | 2580–2610, 2666–2699, 2808–2874 | ~187 | `sample_is_air_for_tool` and `swept_path_is_all_air` become `pub(super)` (parent's `reference_engagement_of_cutting_moves`, `filter_air_cuts`, `filter_air_cuts_with_provenance` call them) |
| `dressup/tests.rs` | `mod tests` | 3164–4727 | 1564 | unchanged (`#[cfg(test)]`); parent is already `mod.rs`, so no directory conversion is needed |

`link.rs` (171 lines) and `air_cut.rs` (187 lines) fall under the 300-line
floor. They are kept separate because they are genuinely different topics
(corridor-sweep geometry vs. air-sample classification); merging them into
one `geometry_helpers.rs` file is a defensible alternative if the
implementation agent prefers fewer files.

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `EntryStyle`, `EntrySurfaceProbe`+impl, `OffMeshEntry`, `EntrySafety`, `apply_entry`, `apply_entry_with_provenance` | 54–286 | type definitions + the section's only entry points; no private helpers to extract |
| `RestEntryRamp`, `optimize_entry_descents`, `optimize_entry_descents_annotated`, `optimize_entry_descents_with_provenance`, `is_plunge` (priv) | 287–705, 740–752 | entry points + type; `is_plunge` stays because the parent itself calls it (from `apply_entry_with_provenance`) as well as two children (`entry_descent.rs`'s `collect_following_cut` and the lead-in/out code below) — shared by the parent plus two children, so it stays put rather than picking one owner |
| `Tab`, `apply_tabs` | 1353–~1550 | type + sole entry point; section has no private helpers |
| `lift_lead_points`, `lift_arc_points` (both priv), `apply_lead_in_out`, `apply_lead_in_out_with_feeds`, `apply_lead_in_out_with_provenance` | 1556–2013 | the two helpers are 27 lines total, used only by the one entry point in the same section; not worth a 27-line child |
| `apply_dogbones`, `apply_dogbones_with_provenance` | 2019–2181 | entry points only; section has no private helpers |
| `LinkMoveParams`, `apply_link_moves`, `apply_link_moves_with_provenance`, `even_tabs` | 2186–2201, 2373–2579 | type + entry points |
| `ReferenceEngagement`, `reference_engagement_of_cutting_moves`, `AirBridgePolicy`, `filter_air_cuts`, `filter_air_cuts_with_provenance` | 2700–2718, 2719–2807, 2875–3163 | types + entry points |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: 89 occurrences across 62 out-of-crate files. This file's `pub`
  API does not physically move, so most callers need no re-export at all —
  only the `pub(crate)` items do.
- `pub use` / `pub(crate) use` lines the parent needs: a repo grep confirms
  `crate::dressup::ENTRY_CLEARANCE`, `crate::dressup::emit_ramp`, and
  `crate::dressup::emit_helix` are read from outside this file today
  (`adaptive3d/path.rs:146,1428,1445,1584,1613`). The parent must add
  `pub(crate) use entry_descent::{ENTRY_CLEARANCE, emit_ramp, emit_helix};`.
  `RampFold` has no external reader (grep confirms) and needs no re-export.
- `use super::` lines the children need: none of the three children call
  back into each other; each is called only by the parent.
- shared private helpers: `is_plunge` (stays in the parent — see the Stays
  table); everything else is single-child.

### Test module

3164–4727, 1564 lines. Moves to `dressup/tests.rs` (ruling 6, `mod.rs`
parent — no directory conversion needed, the folder already exists). Its
own sub-banners (Ramp entry tests L3181, Helix entry tests L3273,
Entry-descent optimization tests L3364, Tab/bridge tests L3491,
Lead-in/out tests L3595, Dogbone tests L3638) travel with it unchanged;
they are comments inside the moved file, not a further split.

### Risk and gate

- Risk: **M** — not a frozen folder and no exact-path sentry, but the
  three children and the parent trade calls in several directions
  (`entry_descent.rs`'s `emit_ramp`/`emit_helix` called back by the parent;
  `link.rs`'s `bridge_corridor_is_swept` called back by the parent), which
  is a real judgement call to get the visibility right, and 62 out-of-crate
  files make the `pub(crate)` re-export load-bearing.
- `--lib` filter: `cargo test -p rs_cam_core --lib dressup::`
- integration `--test` targets: none read this file by exact path. A repo
  grep shows 65 integration test files call `dressup::` functions; a
  representative subset worth re-running as a sanity check:
  `capability_link_moves_safety.rs` (link-vs-retract),
  `entry_moves_stock_aware_g_rampterrain.rs` (entry-descent),
  `air_cut_family_calibration_w5bf4.rs` (air-cut filter),
  `lead_in_out_feed_rates_f040.rs` (lead-in/out). None require source
  edits — the split is a pure move.
- source-scanning sentries: none. `dressup/mod.rs` is not in the exact-path
  reader list.


## 6. `crates/rs_cam_core/src/feeds/mod.rs` — 4624 lines

This is the feeds-and-speeds calculator: the shared data model (tool
geometry, cutter kind, spindle/derate context, `FeedsInput`/`FeedsResult`/
`FeedsWarning`/`FeedsError`), the rubbing-floor policy family, and one
single public entry point, `calculate`, which is 1296 lines and is the
majority of the file's non-type, non-test content. `planning/structure_2026-09-17/FEEDS_WAVE.md`
row FW-22 measures `calculate` at 1302 lines with real step banners (`//
--- Step 1: RPM ---` and so on) and rules explicitly: low priority, **do
not schedule it in this wave.** This section honours that: `calculate`
moves nowhere and splits nowhere: its `let mut` locals cross every step
banner (ruling 2), and the wave that owns this folder has already decided
the banners are not a seam worth taking now. The split instead moves the
private step-helper functions `calculate` calls, and the self-contained
rubbing-floor policy family, out of the parent. As with the other files
here, the implementer finds every item by symbol name, not by line number
— this file grew by two lines since the digest was taken (4622 → 4624)
while this analysis was in progress.

**Measured.** 4624 lines. 57 top-level items: 7 `struct`, 10 `enum`, 13
`fn`, 6 `impl`, 5 `const`, 1 `static`, 17 `mod` (16 pre-existing submodule
declarations — `cutter_constraints`, `efficiency`, … — plus the inline test
module). Inline `#[cfg(test)] mod tests`: lines 2765–4622 (1860 lines).
Banners: 23, all inside `calculate` itself (L1216–L2611 range, `// --- Step
N: … ---`) plus one at L3800 inside the test module ("Vendor LUT
integration tests"); none of them mark a production seam this wave takes.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `feeds/tool_geometry.rs` | `impl ToolGeometryHint`, `impl CutterKind` | 79–192, 222–268 | ~161 | inherent impls; no re-export needed at all — the methods stay reachable on `ToolGeometryHint`/`CutterKind` regardless of which file defines the `impl` block (ruling's impl-anywhere allowance), so the break cost for this child is 0 |
| `feeds/rubbing_floor.rs` | `RUBBING_FLOOR_MM_TOOTH`, `effective_rubbing_floor`, `rubbing_floor_clamp_reason`, `recipe_parked_by_rubbing_floor` (all `pub`) | 913–1036 | ~124 | stay `pub`; parent adds `pub use rubbing_floor::*;` |
| `feeds/rpm_power.rs` | `drill_rpm_envelope_for_diameter`, `milling_rpm_ceiling_for_diameter`, `power_model_terms`, `POWER_LADDER_AP_FLOOR_MM`, `POWER_LADDER_AE_FLOOR_MM`, `largest_fitting` (all priv) | 1037–1192 | ~156 | all become `pub(super)` — `calculate`, which stays in the parent, calls every one of them |
| `feeds/defaults.rs` | `DefaultProfile` (priv struct), `operation_default_profile`, `default_engagement` (both priv) | 2489–2720 | ~232 | become `pub(super)` — `calculate` calls both functions |
| `feeds/tests.rs` | `mod tests` | 2765–4622 | 1860 | unchanged (`#[cfg(test)]`) |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| 16 pre-existing `pub mod` declarations, `EMBEDDED_LUT`, `embedded_vendor_lut` | 18–61 | existing submodule wiring plus a trivial 13-line data loader; not worth relocating |
| `ToolGeometryHint`, `CutterKind` (enum bodies only — impls move above) | 61–78, 193–221 | type definitions (ruling 5) |
| `OperationFamily` … `FeedsError`+impls, `validate_tool_for_operation` | 269–912 | type definitions plus the small `pub fn` that validates against them; this is the module's public data model |
| `calculate` | 1193–2488 | the module's one entry point, 1296 lines, explicitly not split this wave (FW-22) |
| `effective_diameter` | 2721–2762 | small `pub` utility; not worth relocating |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: the `feeds` row of the external-usage table (146 files, 390
  occurrences) is the closest measured proxy — that count spans the whole
  `feeds::` prefix, not this file alone, since there is no separate
  digest row for `feeds::mod`.
- `use super::` lines the children need: none — `rpm_power.rs` and
  `defaults.rs` are each called only by `calculate` in the parent (a
  descendant module's private items are visible to it without any
  visibility change; the reverse direction — parent calling into the
  child — is what needs `pub(super)` plus a `use rpm_power::*;` /
  `use defaults::*;` line in the parent).
- shared private helpers: none identified — `rpm_power.rs` and
  `defaults.rs` are each single-caller (`calculate`), and
  `rubbing_floor.rs` is fully `pub` already.

### Test module

2765–4622, 1860 lines. Moves to `feeds/tests.rs` (ruling 6, `mod.rs`
parent). Parent keeps `#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **H** — `feeds/**` is the frozen folder (see file 3 above); same
  extra risk step applies, independent of this file's own (otherwise
  shallow) coupling.
- `--lib` filter: `cargo test -p rs_cam_core --lib feeds::`
- integration `--test` targets: none read this file by exact path.
  `calculate` is exercised by a very large slice of the integration suite
  (a repo grep for `feeds::` outside `feeds::suggest` returns ~48 files);
  representative ones worth a post-move sanity run:
  `chipload_formula_calibration.rs`, `wanaka_defaults_validation.rs`,
  `test_drill_family_rpm_in_drill_band`-style fixtures live in the moved
  test module itself, so the crate's own `--lib` run covers them.
- source-scanning sentries: none. `feeds/mod.rs` is not in the exact-path
  reader list.


## 7. `crates/rs_cam_core/src/finish/conformal_spiral.rs` — 4402 lines

The file is a research-only prototype (its own header says so: "nothing
here is on a production path") for conformal-spiral finishing on a
simply-connected mesh region. The first 1258 lines hold the tuning
constants, the parameter/report type model (`SpiralParams`, `SpiralReport`,
and the smaller report-detail types), and the two-function entry point
(`plan_spiral` calling `plan_into`). After that the file runs one pipeline
in three stages, each with its own banner-marked block: mean-value
flattening of the region to a disk and distortion measurement (1259–1986),
ring sampling and greedy ring search over the flattened disk (1987–2978),
and building the best spiral plus a self-intersection check and the final
report (2979–3475). The 927-line test module follows at 3476.

**Measured.** 4402 lines. 87 top-level items:
37 `fn`, 5 `impl`, 26 `struct`/`enum`, 18 `const`/`static`.
Inline `#[cfg(test)] mod tests`: lines 3476–4402 (927 lines).
Banners: 46, mostly paired divider rules around type/const groups
(for example L256/258, L360/362, L1139/1141). The field-group labels at
L901, L915, L995, L1088, L1111, L1130 sit inside the `SpiralReport` struct
body itself, not at module scope, so they are not split points.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `finish/conformal_spiral/flatten.rs` | `Topology`, `region_topology`, `Flattening`, `MeanValueWeights`, `mean_value_weights`, `flatten_to_disk`, `flat_of`, `signed_area_2d`, `measure_flattening`, `measure_ring_anisotropy`, `corner_angle_3d`, `corner_angle_2d`, `percentile_sorted`, `triangle_dilatation`, `vertex_normals` | 1259–1986 | 728 | all `priv`; stay `priv` except where noted below |
| `finish/conformal_spiral/rings.rs` | `FlatLocator`, `Located`, `lift`, `Sample`, `sample_iso_scallop`, `audit_coverage`, `barycentric_lattice`, `SegGrid`, `dist2_point_segment`, `CentreCurve`, `LiftStats`, `lift_polyline`, `circle_disk_points`, `Ring`, `distance_to_polyline_mm`, `summarise`, `distances_to`, `BlockerCensus`, `RingLoopState`, `publish_ring_rows`, `search_rings` | 1987–2978 | 992 | `FlatLocator`, `Located`, `CentreCurve`, `SegGrid` become `pub(super)` (read by sibling children) |
| `finish/conformal_spiral/spiral_build.rs` | `blend_sigma` (pub), `roll_to_disk`, `SpiralMeta`, `SpiralDomain`, `shift_cells`, `build_spiral_domain`, `Lifted`, `lift_disk_aligned`, `build_best_spiral`, `count_disk_self_intersections`, `cross2`, `segments_properly_cross`, `finish_report` | 2979–3475 | 497 | `blend_sigma` stays `pub`; parent adds `pub use spiral_build::blend_sigma;` |
| `finish/conformal_spiral/tests.rs` | `mod tests` body | 3476–4402 | 927 | stays `#[cfg(test)]`; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| the 18 tuning `const`s, `SpiralParams`+`Default`+impl, `SpiralRefusal`, `NonManifoldDetail`, `SpiralResult`, `RadialDistortion`, `RingAnisotropy`, `StallDistanceReference`, `DistanceStats`, `StallContext`, `CoverageAudit`, `SpiralReport`, `plan_spiral`, `plan_into` | 1258 | ruling 5: type definitions and the two-function entry point |
| `median_sorted` | 15 | used by one item in each of the three children (`measure_flattening`/`measure_ring_anisotropy`, `summarise`, `finish_report`); it stays in the parent rather than picking one child to own it, per the "or stays in the parent" branch of the shared-helper rule |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0 with the re-export. Without it:
  the brief's external-usage table counts 12 occurrences across 7 files
  for `finish::conformal_spiral`, all of which reach the module for the
  param/report types or `blend_sigma`, since the module is research-only
  and has no operation/GUI/MCP caller.
- `use super::` lines the children need: `flatten.rs` needs
  `use super::rings::FlatLocator;` — `measure_ring_anisotropy` (a
  flattening-stage function) calls `FlatLocator`, which is defined in the
  ring-sampling stage. This is a real cross-stage dependency running
  "backwards" against the stage order; a later agent should not assume the
  three stages are a clean one-way pipeline.
- shared private helpers: `FlatLocator`/`Located` (defined in `rings.rs`)
  are also called from `plan_into` in the parent and from
  `lift_disk_aligned`/`build_best_spiral` in `spiral_build.rs`, so they
  need `pub(super)`. `CentreCurve`/`SegGrid` (also defined in `rings.rs`)
  are called from `build_best_spiral` and `count_disk_self_intersections`
  in `spiral_build.rs`, so they need `pub(super)` too. `median_sorted`
  stays in the parent (see above) and needs no visibility change, since a
  private parent item is already visible to every descendant module.

### Test module

Lines 3476–4402 (927 lines) → `finish/conformal_spiral/tests.rs`. Parent
keeps `#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **M** — no sentry reads this file, and it is research-only (not
  frozen, not on a production path), but `FlatLocator`/`Located`/
  `CentreCurve`/`SegGrid` crossing three module boundaries is a genuine
  judgement call, and the backwards `flatten.rs → rings.rs` dependency
  needs a deliberate `use`, not a mechanical cut.
- `--lib` filter: `cargo test -p rs_cam_core --lib finish::conformal_spiral::`
- integration `--test` targets: none — the module is research-only; the
  brief's 7-file/12-occurrence external count are same-crate callers of
  the stable param/report types and `blend_sigma`, unaffected by the split.
- source-scanning sentries: none.


## 8. `crates/rs_cam_core/src/finish/pencil.rs` — 3635 lines

The file's own header states its job: orchestration (reference-tool
resolution, detector dispatch) plus the shared pipeline every detector
feeds into — fair, lift-to-surface, rest-depth gate, offset passes,
nearest-neighbor order, emit. That maps onto three groups of items besides
the parameter/event types: the three detector arms (`curvature_arm`,
`rest_depth_arm`, `dihedral_arm`, each a valley/rest-ridge/dihedral-edge
front end) with the reference-tool resolution they share; the chain-to-path
machinery (`paths_from_sampled` and its fairing/lift/offset/gate helpers);
and the pass-emission machinery (entry-ramp planning, link-lift, and the
`emit_*` functions). The 1135-line test module follows the production
code. Several of the "mentions" coupling.txt reports from `PencilParams` to
functions like `plan_link_lift` and `rest_depth_arm` are rustdoc
intra-doc links in its field docs (confirmed by reading the source), not
function calls — they need a path edit in the doc comment, not a
visibility change.

**Measured.** 3635 lines. 65 top-level items:
34 `fn`, 8 `impl`, 11 `struct`/`enum`, 11 `const`/`static`.
Inline `#[cfg(test)] mod tests`: lines 2503–3635 (1135 lines, note: `mod
priv 2503 2501 1135 tests` — span 2501–3635; 2501 is the first attribute
line, 2503 the `mod` keyword).
Banners: 6, all inside `dihedral_arm` (2287–2404, one item) marking its
internal algorithm steps; not split points under ruling 2.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `finish/pencil/detectors.rs` | `ResolvedReference`+impl, `resolve_reference_cutter`, `SURFACE_PROBE_BALL_DIAMETER_MM`, `SURFACE_PROBE_BALL_LENGTH_MM`, `NOMINAL_REFERENCE_BALL_LENGTH_MM`, `curvature_arm`, `rest_depth_arm`, `dihedral_arm` | 1947–2404 | 458 | `curvature_arm`/`rest_depth_arm`/`dihedral_arm` were `priv`; become `pub(super)` for the parent's dispatcher. `ResolvedReference` and `resolve_reference_cutter` become `pub(super)` **together** — `resolve_reference_cutter` returns `ResolvedReference`, and rustc's private-in-public check requires the type to be at least as visible as the function once `tests.rs` needs it. `SURFACE_PROBE_*`/`NOMINAL_REFERENCE_*` stay `pub(crate)`; parent adds `pub(crate) use detectors::{...};` |
| `finish/pencil/chain_paths.rs` | `offset_polyline_variable`, `offset_polyline`, `FAIRING_STRENGTH`, `FAIRING_PASSES`, `fair_polyline_xy`, `lift_to_surface`, `reach_gap_at_point`, `reach_gap_threshold`, `paths_from_sampled`, `OffsetFan`+impl, `contact_runs` | scattered, 345–1026 | 438 | `lift_to_surface` and `fair_polyline_xy` were `priv`; become `pub(super)` (parent's `PencilPath` impl and `tests.rs` both need `lift_to_surface`; `tests.rs` needs `fair_polyline_xy`). `contact_runs` becomes `pub(super)` (needed by `emission.rs`). `paths_from_sampled`, `reach_gap_threshold` stay `pub(crate)`; parent adds `pub(crate) use chain_paths::{paths_from_sampled, reach_gap_threshold};` |
| `finish/pencil/emission.rs` | `ENTRY_RAMP_*` consts (6), `tip_contact_radius`, `entry_bite_budget_mm`, `entry_ramp_window_mm`, `EntryRampPlan`, `plan_entry_ramp`, `emit_entry_descent`, `PencilJunction`+impl, `LinkLift`, `plan_link_lift`, `emit_paths`, `emit_paths_with_entry_stock`, `PencilLinkReport`, `emit_paths_with_entry_stock_reported` | scattered, 1027–1946 | 879 | `emit_entry_descent`, `PencilJunction`, `LinkLift`, `plan_link_lift` were `priv`; `LinkLift` and `plan_link_lift` become `pub(super)` (`tests.rs` calls both directly). `tip_contact_radius`, `entry_bite_budget_mm`, `entry_ramp_window_mm`, `ENTRY_RAMP_MIN_BITE_MM`, `PencilLinkReport` stay `pub`; parent adds `pub use emission::{...};`. `EntryRampPlan`, `plan_entry_ramp`, `emit_paths`, `emit_paths_with_entry_stock`, `emit_paths_with_entry_stock_reported` stay `pub(crate)`; parent adds a matching `pub(crate) use` |
| `finish/pencil/tests.rs` | `mod tests` body | 2503–3635 | 1135 | stays `#[cfg(test)]`; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `PencilDetector`+impl, `PencilParams`+`Default`, `PencilRuntimeEvent`/`PencilRuntimeAnnotation`+impl, `PencilPath`+impl | 305 | ruling 5: public entry types |
| `bisector_strength_default`, `reference_tool_diameter_default`, `valley_saliency_default`, `curvature_smoothing_default`, `rest_cell_default`, `route_width_factor_default`, `detector_string_default` | 46 | small `pub(crate)` config-default getters tied directly to `PencilParams`; together under the 300-line child minimum |
| `gate_chains_by_depth`, `polyline_passes_depth`, `order_paths_nearest` | 172 | called from both `detectors.rs` (`dihedral_arm`, `curvature_arm`, `rest_depth_arm`) and the parent's dispatcher; simplest to leave them in the parent, which every child can already reach without a visibility change |
| `pencil_toolpath`, impl `RuntimeLabel for PencilRuntimeAnnotation`, `pencil_toolpath_structured_annotated`, `pencil_toolpath_structured_annotated_with_cancel` | 163 | the public entry points, the last of which is the dispatcher that calls into all three children |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0 with the re-exports above. The
  brief's external-usage table counts 18 occurrences across 13 files
  outside core (plus 15 in-core files) for `finish::pencil` — all reach
  `pencil_toolpath*`, `PencilParams`, `PencilDetector`, or the emission
  items listed above, none of which change path.
- `use super::` lines the children need: `detectors.rs` needs
  `use super::{gate_chains_by_depth, polyline_passes_depth};`.
  `chain_paths.rs` needs no `use super::` beyond the parameter types.
  `emission.rs` needs `use super::chain_paths::contact_runs;`.
- shared private helpers: `resolve_reference_cutter`/`ResolvedReference`
  (detectors.rs → tests.rs), `lift_to_surface` (chain_paths.rs → parent's
  `PencilPath` impl and tests.rs), `contact_runs` (chain_paths.rs →
  emission.rs), `LinkLift`/`plan_link_lift` (emission.rs → tests.rs).
  `gate_chains_by_depth`/`polyline_passes_depth` stay in the parent
  instead (see table above) because they are read by both `detectors.rs`
  and the parent's own dispatcher.

### Test module

Lines 2503–3635 (1135 lines) → `finish/pencil/tests.rs`. Parent keeps
`#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **M** — no sentry reads this file, but it has 15 in-core
  importers and coupling.txt shows four distinct private-helper pairs that
  cross the proposed child boundaries (`resolve_reference_cutter`,
  `lift_to_surface`, `contact_runs`, `plan_link_lift`); each is a real
  visibility judgement, not a mechanical move.
- `--lib` filter: `cargo test -p rs_cam_core --lib finish::pencil::`
- integration `--test` targets: none identified — the public API
  (`pencil_toolpath*`, `PencilParams`, `PencilDetector`,
  `PencilLinkReport`) is unchanged.
- source-scanning sentries: none.


## 9. `crates/rs_cam_core/src/finish/scallop.rs` — 3581 lines

The file's header describes constant-scallop-height finishing: concentric
offset contours with a stepover that widens on steep walls and tightens on
shallow convex ground. The file has three natural groups after its opening
type/policy block: the ring-generation engine (chord refinement, ring
lifting, and the 386-line `generate_scallop_rings_with_cancel`), a
separate "research" variant of the same pipeline built around the
643-line `scallop_toolpath_research_with_stage`, and the public
structured-annotated entry-point family with the report type. Both large
functions are single items ruling 2 keeps whole. **Contradicts the brief's
hint**: the hint says no test module over 500 lines was flagged for this
file, but the inline module is 772 lines (2810–3581) and so, per ruling 6,
it does move to its own child file.

**Measured.** 3581 lines. 62 top-level items:
23 `fn`, 14 `impl`, 19 `struct`/`enum`, 4 `const`/`static`, 1 `type`.
Inline `#[cfg(test)] mod tests`: lines 2810–3581 (772 lines).
Banners: none.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `finish/scallop/ring_generation.rs` | `point_is_covered`, `decimate_ring_polygon`, `decimate_closed_ring`, `RingLiftCtx`, `CHORD_REFINE_MIN_SEG_MM`, `CHORD_REFINE_MIN_SPLIT_MM`, `CHORD_REFINE_ACCEPT_FRACTION`, `CHORD_REFINE_MAX_DEPTH`, `ring_to_3d`, `refine_ring_chords`, `refine_chord`, `dropped_arc_length_mm`, `generate_scallop_rings` (pub), `generate_scallop_rings_with_cancel`, `closest_kept_point_idx`, `rotate_ring` | 759–1628 | 861 | `RingLiftCtx`, `ring_to_3d`, `refine_chord`, `closest_kept_point_idx`, `rotate_ring`, `generate_scallop_rings_with_cancel` were `priv`; become `pub(super)` (read from the parent and from `research.rs`). `generate_scallop_rings` stays `pub`; parent adds `pub use ring_generation::generate_scallop_rings;` |
| `finish/scallop/research.rs` | `ScallopRingBudget`, `scallop_toolpath_iso_field_with_cancel`, `scallop_toolpath_structured_annotated_with_resolution_and_ring_budget`, `scallop_toolpath_research`, `scallop_toolpath_research_with_stage` | 2010–2809 | 800 | all `pub` or `pub(crate)`; stay so. Parent adds `pub use research::{ScallopRingBudget, scallop_toolpath_iso_field_with_cancel, scallop_toolpath_structured_annotated_with_resolution_and_ring_budget, scallop_toolpath_research};` and `pub(crate) use research::scallop_toolpath_research_with_stage;` |
| `finish/scallop/tests.rs` | `mod tests` body | 2810–3581 | 772 | stays `#[cfg(test)]`; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `ScallopDirection`, `ScallopParams`+`Default`, `ScallopRuntimeEvent`/`Annotation`+impl, `ring_stepover`, `RingStepoverDecision`, `ring_stepover_with_policy`, `RingReducer`, `RingSampling`, `StepoverGeometry`, `CurvaturePolicy`, `PolygonReduce`, `RingSource`, `ScallopStepoverPolicy`+impl+`Default`, `RingSampleBound`, `RingCleanup`, `ScallopStepoverTrace` (lines 32–758) | 726 | ruling 5: the stepover-policy type model, all public configuration enums/structs plus the small `ring_stepover` formula they wrap |
| `RingCascadeMetrics`, `RingCascade`, `ScallopReport`+impl, `scallop_generation_resolution`, `scallop_toolpath`, impl `RuntimeLabel`, `scallop_toolpath_structured_annotated`, `..._with_cancel`, `..._with_cancel_and_stage`, `..._with_resolution` | 390 | ruling 5: the report type and the public structured-annotated entry-point family |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0 with the re-exports above. The
  brief's external-usage table counts 16 occurrences across 13 files
  outside core (plus 10 in-core files) for `finish::scallop`; all reach
  the parameter/report types or a `scallop_toolpath*` entry point, none
  of which change path.
- `use super::` lines the children need: `research.rs` needs
  `use super::ring_generation::{closest_kept_point_idx, generate_scallop_rings_with_cancel, refine_chord, ring_to_3d, rotate_ring};` plus `use super::ring_stepover;` (the last stays in the parent).
- shared private helpers: `ring_stepover` (parent) is read by
  `generate_scallop_rings_with_cancel` (ring_generation.rs) and
  `scallop_toolpath_research_with_stage` (research.rs) — it stays in the
  parent and needs no visibility change, since a private parent item is
  already visible to both children. `ring_to_3d`, `RingLiftCtx`,
  `refine_chord`, `closest_kept_point_idx`, `rotate_ring`, and
  `generate_scallop_rings_with_cancel` are defined in `ring_generation.rs`
  but also read from the parent (`RingCascade`, `ScallopReport`,
  `RingCascadeMetrics`) and from `research.rs`
  (`scallop_toolpath_research_with_stage`) — each needs `pub(super)`.

### Test module

Lines 2810–3581 (772 lines) → `finish/scallop/tests.rs`. This contradicts
the brief's hint that no test module over 500 lines was flagged for this
file; ruling 6 applies because 772 > 500.

### Risk and gate

- Risk: **M** — no sentry reads this file, but it has 10 in-core
  importers and six private items (`RingLiftCtx`, `ring_to_3d`,
  `refine_chord`, `closest_kept_point_idx`, `rotate_ring`,
  `generate_scallop_rings_with_cancel`) that coupling.txt shows are read
  from a sibling child; each needs a deliberate `pub(super)`, not a
  mechanical cut.
- `--lib` filter: `cargo test -p rs_cam_core --lib finish::scallop::`
- integration `--test` targets: none identified — the public API
  (`scallop_toolpath*`, `ScallopParams`, `ScallopReport`) is unchanged.
- source-scanning sentries: none.


## 10. `crates/rs_cam_core/src/tool_load/optimize/mod.rs` — 3361 lines

This is the tool-load optimizer's orchestration layer: it searches
feed/RPM/geometry candidates for one toolpath and ranks them by simulated
cycle time, calling out to twelve already-separate private submodules
(`candidate`, `context`, `delta`, `narrative`, `outcome`, `policy`,
`preflight`, `refusal`, `rank`, `retarget_reconciliation_a8`, and the `pub`
`axes`/`bounds`/`patches`/`progress`/`retarget`/`space`/`strategy`). Because
the module is already this modular, what is left in `mod.rs` itself is
mostly the two public entry points (`optimize_toolpath`,
`optimize_toolpath_observed`, `optimize_project`), one 274-line private
orchestration function, and five differently-named inline test modules of
very different sizes. The split moves the private step-implementation
functions into two topic children and extracts the two test modules that
exceed the 500-line threshold. As elsewhere, the implementer relocates
every item by symbol name — this file is 3361 lines today, one less than
the digest's 3362, because the tree moved under a concurrent commit while
this analysis ran.

**Note on ruling 6's scope, re-verified directly on the live file
(`grep -n '^#\[cfg(test)\]' tool_load/optimize/mod.rs`):** this file has
five sibling inline test modules, not one named `tests`, and they are not
nested inside one another. Their attribute-inclusive spans are
`orchestration_skip_tests` 1006–1547 (542 lines), `project_rollup_tests`
1548–1872 (325 lines), `tests` 1873–2729 (857 lines), `stage1_grid_tests`
2730–3183 (454 lines), `candidate_eval_tests` 3184–3361 (178 lines) — these
five total 2356 of the file's 3361 lines (70%), leaving only **1005 lines
of production code**. Ruling 6 as literally written names only `mod tests`
and sets a mandatory floor at 500 lines, so only `tests` and
`orchestration_skip_tests` are *required* extractions. But ruling 2 and
ruling 3 place no floor on how small a whole-item move may be, and for a
frozen-folder file the lowest-risk, highest-value move is to extract all
five as sibling files regardless of size: it is a pure move (no visibility
judgement calls — nothing outside its own file calls into a `#[cfg(test)]`
module), it touches no formula, threshold, or control flow (satisfying
ruling Q5's constraint on `feeds/**`/`tool_load/**` edits), and it alone
takes the parent from 3361 lines to ~1005 + glue. This is treated as the
primary recommendation below; the production-side split
(`strategies.rs`/`orchestrate.rs`) is a secondary, lower-priority move
that does touch private production call graphs inside a frozen folder and
should be scheduled separately from — and after — the test-only move.

**Measured.** 3361 lines. 40 top-level items: 2 `struct`, 1 `enum`, 11
`fn`, 1 `impl`, 1 `trait`, 1 `const`, 1 `static`, 22 `mod` (17 pre-existing
submodule declarations plus 5 inline test modules). Banners: one, at
L2822, a stray comment inside `stage1_grid_tests`, not a production seam.

### Proposed children

Primary move (test-only, do first):

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `tool_load/optimize/orchestration_skip_tests.rs` | `mod orchestration_skip_tests` | 1006–1547 | 542 | unchanged (`#[cfg(test)]`); required by ruling 6 (over 500 lines) |
| `tool_load/optimize/project_rollup_tests.rs` | `mod project_rollup_tests` | 1548–1872 | 325 | unchanged (`#[cfg(test)]`); whole-item move below the ruling-6 floor, taken anyway for the reason above |
| `tool_load/optimize/tests.rs` | `mod tests` | 1873–2729 | 857 | unchanged (`#[cfg(test)]`); required by ruling 6 |
| `tool_load/optimize/stage1_grid_tests.rs` | `mod stage1_grid_tests` | 2730–3183 | 454 | unchanged (`#[cfg(test)]`); whole-item move below the floor |
| `tool_load/optimize/candidate_eval_tests.rs` | `mod candidate_eval_tests` | 3184–3361 | 178 | unchanged (`#[cfg(test)]`); whole-item move below the floor |

Secondary move (production, optional, do second and separately):

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `tool_load/optimize/strategies.rs` | `run_headroom_strategy`, `any_load_gate_exceeds`, `run_retarget_strategy`, `RetargetStageOutput` (struct), `run_grid_strategy` (all priv) | 558–886 | ~329 | `run_headroom_strategy`, `run_retarget_strategy`, `run_grid_strategy`, `any_load_gate_exceeds` become `pub(super)` — `orchestrate.rs` (sibling, below) calls all four, and `run_grid_strategy` itself calls `run_retarget_strategy` (intra-file, no change needed) |
| `tool_load/optimize/orchestrate.rs` | `optimize_toolpath_inner`, `attach_retarget_refusals`, `NARROW_BAND_HEADLINE` (const) (all priv) | 212–557 | ~346 | `optimize_toolpath_inner` becomes `pub(super)` — the parent's `optimize_toolpath_observed` calls it; internally it needs `use super::strategies::{run_headroom_strategy, run_retarget_strategy, run_grid_strategy, any_load_gate_exceeds};` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| 17 pre-existing `mod` declarations (`axes`, `bounds`, `candidate`, … `strategy`) | 41–87 | existing submodule wiring, unrelated to this split |
| `DEFAULT_SEARCH_POLICY`, `search_policy`, `tolerance_bands_from_policy`, `SearchStage`, `optimize_toolpath`, `optimize_toolpath_observed` | 88–211 | type + the module's two headline entry points |
| `ProgressReporter` (trait), `NoProgress`+impl, `optimize_project` | 887–1012 | type definitions + the module's third entry point |

If only the primary (test-only) move runs, `optimize_toolpath_inner`,
`attach_retarget_refusals`, `NARROW_BAND_HEADLINE`, and the
`strategies.rs` cluster all stay in the parent too (production untouched):
that leaves the parent at ~1005 lines of production code plus glue, still
a large drop from 3361.

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: 32 occurrences across 14 out-of-crate files that name
  `tool_load::optimize::…`, plus 9 core files.
- `use super::` lines the children need: `orchestrate.rs` needs
  `use super::strategies::{run_headroom_strategy, run_retarget_strategy, run_grid_strategy, any_load_gate_exceeds};`
  the parent needs `use orchestrate::optimize_toolpath_inner;`.
- shared private helpers: none beyond the strategies-to-orchestrate
  direction above; `any_load_gate_exceeds` is used only by
  `optimize_toolpath_inner` (now in `orchestrate.rs`) and by the `tests`
  module (now `tests.rs`, and `pub(super)` covers that reach too).
- **`dead_code` risk specific to this folder:** `optimize/mod.rs:49`
  already declares `mod policy;` privately (not `pub mod`), and the FEEDS_WAVE
  survey records that `dead_code` genuinely fires inside that private
  subtree today. This is a risk for the *secondary* (production) move
  only: adding two more private child modules (`strategies`, `orchestrate`)
  grows that private surface. It does not apply to the primary (test-only)
  move — `#[cfg(test)]` modules sit outside the normal `dead_code` graph.
  If the secondary move runs, run
  `cargo clippy -p rs_cam_core --features rs_cam_core/heavy-tests -- -D
  warnings` scoped to this folder before treating it as done — a demoted
  or relocated item here can produce a new warning that did not exist in
  the flat file.

### Test module

Five sibling extractions, all whole-item moves, all recommended (see
"Proposed children" above for the reasoning on why the three
sub-500-line ones move too): `orchestration_skip_tests` (1006–1547, 542
lines), `project_rollup_tests` (1548–1872, 325 lines), `tests` (1873–2729,
857 lines), `stage1_grid_tests` (2730–3183, 454 lines),
`candidate_eval_tests` (3184–3361, 178 lines). Parent keeps five
`#[cfg(test)] mod <name>;` stub lines. This is the whole win for this
file: it alone drops the parent from 3361 to ~1005 lines of production
code plus glue, without editing a single formula, threshold, or control
path in the frozen folder.

### Risk and gate

- Risk: **H** for the file overall — `tool_load/**` is the second frozen
  folder (power-calcs session, FEEDS_WAVE.md); same extra risk step as
  files 3 and 6. Within that H, the primary (test-only) move is close to
  **L** in character — no production coupling, no visibility judgement
  calls, no `dead_code` exposure — while the secondary (production) move
  carries the real judgement calls (the `strategies.rs`/`orchestrate.rs`
  cross-calls and the `dead_code` check above). Sequence them separately
  rather than treating the file as one undifferentiated H.
- `--lib` filter: `cargo test -p rs_cam_core --lib tool_load::optimize::`
- integration `--test` targets: `optimizer_assumption_stamp_a8.rs`,
  `optimize_smoke.rs`, `feedopt_clamp_never_panics_wp21.rs`,
  `optimize_reports_progress_wp29.rs`,
  `optimize_toolpath_is_a_job_wp14b.rs` (all 5 files that call
  `tool_load::optimize::` directly per a repo grep). None require source
  edits.
- source-scanning sentries: none. `tool_load/optimize/mod.rs` is not in
  the exact-path reader list.


## 11. `crates/rs_cam_core/src/compute/catalog.rs` — 3287 lines

**Correction to the brief's inputs.** `digest3.txt` and the brief's header
both say 3334 lines; the live file is 3287. `git log` shows a peer session
committed `c97837b8` on top of the commit the digest was taken from, which
trimmed this file by 47 lines (and the `NewDefaultCtx` struct/impl the
digest reports no longer exists anywhere in `rs_cam_core/src`). Every line
number in this section was re-taken with `grep -n` on the live file, not
copied from the digest, for everything from line ~2600 onward; lines 1–2613
were spot-checked against the digest and match exactly. As with the other
sections, the implementer should still relocate each item by name — a
concurrent commit can move lines again before the split lands.

The file holds the operation catalogue: `OperationType`/`OperationConfig`
and their behaviour (label, schema, feeds hints, depth stepping), the schema
types that describe a parameter, and 24 pairs of `*_PARAMS` const +
`REG_*` static — one pair per operation, the literal registry data. This is
the cleanest seam of the four files: registry data, schema types, and core
type behaviour are already three non-overlapping regions in the source.

**Measured.** 3287 lines. Top-level items: 4 `fn`, 11 `impl`, 9 `enum`,
13 `struct`, 24 `const`, 24 `static`, 1 `trait`, 1 `mod` (tests).
Inline `#[cfg(test)] mod tests`: lines 2763–3287 (525 lines; `#[cfg(test)]`
at 2763, `mod tests {` at 2765).
Banners: L1648 (Stage 1 reactive-agent-vs-contour-spiral), L2822 (Phase 1
registry self-consistency), L3019 (Phase 2 X-macro sync) — note the last two
banners' line numbers, taken from the digest, sit in the drifted region; the
live text is still present, just a few lines earlier (`grep -n` to confirm
before editing near it).

### Proposed children

All children sit under `crates/rs_cam_core/src/compute/catalog/`.

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `catalog/schema.rs` | `ParamHint`, `OperationParamSchema`, `ToolConstraints`, `OperationSchema`, `ParamRange` + impl, `ParamDef` + impl, `ToolConstraintsDef` + impl, `EntryStylePolicy`, `DressupPolicy` + impl, `OpRegistryEntry` | 1214–1587 | ~379 | all already `pub` — parent adds `pub use schema::{ParamHint, OperationParamSchema, ToolConstraints, OperationSchema, ParamRange, ParamDef, ToolConstraintsDef, EntryStylePolicy, DressupPolicy, OpRegistryEntry};` (or `pub use schema::*;`) |
| `catalog/registry.rs` | the 24 `*_PARAMS` consts, the 24 `REG_*` statics | 1588–2576 | ~989 | all private in the source; the parent's `registry_entry()` method (in `impl OperationType`, kept in the parent) is their only external caller, so each becomes `pub(super)` and the parent adds `pub(super) use registry::*;` |
| `catalog/tests.rs` | the inline `mod tests` body | 2763–3287 | 525 | `#[cfg(test)] mod tests;` stays in the parent per rule 6; parent is `catalog.rs`, a `<stem>.rs` file, so the child is `catalog/tests.rs` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `OperationFamily`, `GeometryRequirement`, `UiOperationFamily`, `UiProcessRole`, `DepthSemantics`, `OperationSpec`, `OperationTransformCapabilities` + impl, `OpCategory` | 1–267 | core type definitions (rule 5) |
| `impl OperationType` (`ALL_2D`, `ALL_3D`, `registry_entry`, `label`, `supports_reach_map`, `kind_str`, `transform_capabilities`, `air_cut_high_threshold_pct`, `honors_pinned_bottom_z`, …) | 268–655 | the type's own behaviour (rule 5); `registry_entry()` is the one place that names all 24 `REG_*` statics, so this impl needs `pub(super)` visibility into `catalog/registry.rs` |
| `trait OperationParams`, `OperationConfig` enum, `OptimizationSurface` enum, `impl OperationConfig` (main: `entry_probe_leave`, `optimization_surface`, `spec`, `label`, `family`, `feeds_style`, `param_names_for_type`, `schema_for_type`, …) | 656–1208 | core type definitions and behaviour (rule 5) |
| `param_defs_for_type`, `tool_constraints_for_type` | 2577–2590 | private, called only from the main `impl OperationConfig` above (lines 1111–1204) — single consumer, already in the parent |
| `FeedsHints` struct + impl, `impl OperationConfig { feeds_hints }`, `impl OperationConfig { cutting_levels }`, `effective_spindle_rpm`, `feed_optimization_unavailable_reason` | 2591–2762 | more `OperationConfig`/`OperationType` behaviour, exhaustively matching the enum — kept with the type it dispatches on (rule 5) |

### Break cost

- `crate::`/`rs_cam_core::` path edits without a parent re-export: 186
  occurrences across 179 files outside `rs_cam_core` name `compute::catalog`
  (plus 57 in-core files) — this is the file with by far the largest
  external surface of the four in this brief. With the parent re-exporting
  the schema types (all `pub`) that cost is 0; none of the moved registry
  items (`*_PARAMS`, `REG_*`) were ever externally visible (they were
  private), so they cost nothing either way.
- `use super::` lines the children need: none — `catalog/schema.rs` and
  `catalog/registry.rs` only reference each other (`REG_*` statics read
  their own `*_PARAMS` const and, for `REG_ALIGNMENT_PIN_DRILL`, also
  `REG_DRILL` — both stay together in `registry.rs`) and the `OpRegistryEntry`
  type from `schema.rs`, reached as `super::schema::OpRegistryEntry` or via
  the parent's re-export as `super::OpRegistryEntry`.
- Shared private helpers: the 24 `REG_*` statics are the one case — their
  only caller is the parent's `registry_entry()` method. That is the
  "private item only the parent calls" rule exactly: `pub(super)` in the
  child, `pub(super) use registry::*;` in the parent.

### Test module

525 lines, over the 500-line threshold → moves to `catalog/tests.rs` per
rule 6. Leave `#[cfg(test)] mod tests;` in the parent. The test body's
`use super::*;` needs no change: it references only parent-resident types
(`OperationType`, `OperationConfig`, `ToolConstraintsDef`, …), which the
parent's own namespace still carries after the split.

### Risk and gate

- Risk: **L** — no exact-path sentry reads this file, the registry-data
  seam is a clean single-direction `pub(super)` (child feeds one caller in
  the parent), and no `impl` block is cut across children. The large
  external-usage count (179 files) is a path-preservation concern, not a
  risk-scale factor, and it is fully absorbed by the `pub use schema::*;`
  re-export.
- `--lib` filter: `cargo test -p rs_cam_core --lib compute::catalog::`
- integration `--test` targets: none (not in the exact-path-reader list)
- source-scanning sentries: none


## 12. `crates/rs_cam_core/src/session/mutation.rs` — 3273 lines

The file holds every CRUD mutation on `ProjectSession`: three small
collision/change-detection helpers, `polygons_bbox`, and one 1992-line
`impl ProjectSession` with about 70 methods, plus the inline test module.
There is no topic banner inside the `impl` block; the split groups its
methods by the entity or concern each one mutates, verified by reading the
method bodies rather than any in-file marker. As above, the implementer
relocates each method by name — the table's line numbers are a snapshot,
not the contract.

**Measured.** 3273 lines. Top-level items: 4 `fn`, 1 `impl`, 1 `mod` (tests).
Inline `#[cfg(test)] mod tests`: lines 2113–3273 (1161 lines).
Banners: none.

### Proposed children

All children sit under `crates/rs_cam_core/src/session/mutation/` (parent is
`mutation.rs`, a `<stem>.rs` file, so rule 6 names the test child
`mutation/tests.rs`).

| child file | items (methods, all on `impl ProjectSession`) | approx. lines | visibility after the move |
|---|---|---:|---|
| `mutation/toolpath.rs` | `add_toolpath`, `add_toolpath_impl`, `remove_toolpath`, `reorder_toolpath`, `invalidate_result_chain`, `invalidate_output_dependents`, `set_toolpath_enabled`, `set_toolpath_operation`, `move_toolpath_to_setup`, `set_toolpath_tool`, `set_toolpath_model`, `insert_result`, `remove_result`, `drop_result`, `bump_all_revisions`, `drop_all_results`, `drop_setup_results`, `invalidate_toolpath_inputs`, `apply_toolpath_param_snapshot_narrow`, `set_feeds_provenance`, `set_toolpath_debug_options` | ~745 | all `pub(crate)`, unchanged — inherent methods, reachable via `session.method()` with no re-export |
| `mutation/entities.rs` | `add_model`, `add_model_impl`, `remove_model`, `add_tool`, `add_tool_impl`, `remove_tool`, `add_setup`, `add_setup_impl`, `rename_setup`, `set_setup_datum`, `set_setup_pause_message`, `set_setup_models`, `set_setup_face`, `set_setup_rotation`, `set_face_selection`, `set_alignment_pin_drill_holes`, `set_drill_selected_holes`, `replace_fixture`, `replace_keep_out`, `add_fixture`, `remove_fixture`, `add_keep_out`, `remove_keep_out`, `add_alignment_pin`, `PIN_DEDUP_EPSILON_MM`, `remove_alignment_pin` | ~649 | unchanged — model/tool/setup/fixture/keep-out/pin CRUD, all `pub(crate)` |
| `mutation/config.rs` | `set_dressup_config`, `set_dressup_field`, `set_dressup_field_impl`, `set_stock_source`, `set_heights_config`, `set_boundary_config`, `auto_enable_rest_analysis_for_source`, `set_rest_analysis_config`, `invalidate_stock`, `invalidate_machine`, `invalidate_tool`, `drop_tool_results`, `invalidate_model`, `drop_results_for_model`, `adopt_model_geometry`, `set_stock_config`, `update_stock_from_bbox`, `set_post_config`, `set_machine`, `set_machine_kinematics`, `import_machine_settings`, `replace_tool`, `replace_tools` | ~588 | unchanged — dressup/boundary/rest/stock/machine config and their invalidation, all `pub(crate)` |
| `mutation/tests.rs` | the inline `mod tests` body | 1161 | `#[cfg(test)] mod tests;` stays in the parent per rule 6 |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `fixture_collision_inputs_moved`, `keep_out_collision_inputs_moved`, `post_change_reaches_motion`, `polygons_bbox` | 29–120 (~92) | `fixture_collision_inputs_moved` is called from both `keep_out_collision_inputs_moved` (line 44–53) and directly from the `impl` block; once the `impl` splits, its callers land in `mutation/entities.rs` (fixture/keep-out change detection). These four are small, free-standing, and only `polygons_bbox` is `pub` — simplest to leave all four in the parent, reachable by `entities.rs` as an ordinary parent-private call with no visibility edit. (Equally valid: move the three collision helpers into `mutation/entities.rs` directly, since that is their only real caller — either design works.) |

### Break cost

- `crate::`/`rs_cam_core::` path edits: 0. `session::mutation` has zero
  out-of-crate references and exactly one in-core file naming it.
- `use super::` lines the children need: none identified — every method
  moved is an inherent `pub(crate)` method on `ProjectSession`, callable via
  `session.method()` regardless of which file defines it.
- Shared private helpers: `fixture_collision_inputs_moved` is the only one
  (used by `keep_out_collision_inputs_moved` and by the `impl` block
  itself); resolved by keeping it in the parent (see table above).

### Test module

1161 lines, over the 500-line threshold → moves to `session/mutation/tests.rs`
per rule 6. Leave `#[cfg(test)] mod tests;` in the parent.

### Risk and gate

- Risk: **M** — no exact-path sentry reads this file, but the single
  1992-line `impl ProjectSession` block must be cut across three children
  by method-grouping judgement (no in-file topic banner to follow), which
  is the risk scale's "a large impl block must be cut across several
  children" case.
- `--lib` filter: `cargo test -p rs_cam_core --lib session::mutation::`
- integration `--test` targets: none (not in the exact-path-reader list)
- source-scanning sentries: none


## 13. `crates/rs_cam_core/src/stock/simulation_cut.rs` — 3141 lines

This file is the simulation-cut trace and summary data model: sample and
issue types, per-span and per-kinematics accumulators, the trace-building
logic (`impl SimulationCutTrace`), and reporting/export step functions
(rebasing cutting time onto wall clock, publishing cycle times, writing
and pruning the JSON artifact). Coupling here is shallow — `coupling.txt`
records only one cross-reference among production items (`tests` used
twice by `Engagement`, a digest quirk, not a real coupling) — so the split
follows the three natural computational clusters cleanly, each moving as a
self-contained child with no back-calls into the parent or into each
other. Per the usual caveat, the implementer relocates every item by
symbol name; this file sits at 3141 lines today (digest measured 3142).

**Measured.** 3141 lines. 24 top-level items: 18 `struct`, 3 `enum`, 8
`fn`, 9 `impl`, 1 `trait`, 2 `const`. Inline `#[cfg(test)] mod tests`:
lines 1842–3142 (1301 lines). Banners: 9, all inside the test module
(L2166–L2899, "--- Task E-simcut ---" and similar section markers) — none
in production code.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `stock/simulation_cut/analysis.rs` | `impl SimulationCutTrace` (240 lines: `test_fixture`, `from_samples`, `from_samples_with_semantics`, `from_samples_with_context`, `drill_summary_for`), `new_open_segment` (priv), `HotspotAccumulator`+impl (priv), `SemanticSummaryAccumulator`+impl (priv) | 329–359, 1037–1276, 1629–1760 | ~403 | inherent impl needs no re-export (impl-anywhere allowance); `new_open_segment`, `HotspotAccumulator`, `SemanticSummaryAccumulator` are used only inside this child and stay fully private |
| `stock/simulation_cut/accumulate.rs` | `finalize_per_kinematics` (`pub(crate)`), `impl KinematicsAccumulator`, `impl SummaryAccumulator`, `accumulate_by_span` (`pub`) | 1338–1628 | ~291 | `finalize_per_kinematics` stays `pub(crate)`, parent adds `pub(crate) use accumulate::finalize_per_kinematics;`; `accumulate_by_span` stays `pub`, parent adds `pub use accumulate::accumulate_by_span;`; the impls need no re-export |
| `stock/simulation_cut/reporting.rs` | `rebase_cutting_times` (`pub`), `publish_cycle_times` (`pub(crate)`), `write_simulation_cut_artifact` (`pub`), `PRUNE_GRACE_MS` (priv), `prune_simulation_cut_artifacts` (`pub`) | 708–823, 833–905, 1761–1841 | ~270 | `pub` items get `pub use reporting::{rebase_cutting_times, write_simulation_cut_artifact, prune_simulation_cut_artifacts};`; `publish_cycle_times` gets `pub(crate) use reporting::publish_cycle_times;`; `PRUNE_GRACE_MS` stays private (used only by `prune_simulation_cut_artifacts` in the same child) |
| `stock/simulation_cut/tests.rs` | `mod tests` | 1842–3142 | 1301 | unchanged (`#[cfg(test)]`) |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `SimulationMetricOptions`, `SIMULATION_CUT_TRACE_SCHEMA_VERSION`, `CutKinematics`+impl, `EngagementDirection`, `Engagement`+impl, `SimulationProvenance`, `SimulationCutIssueKind`, `SimulationCutSample`+impl, `SimulationCutIssue`, `default_sample_count` (priv), `KinematicsSummary`, `SimulationToolpathCutSummary`, `SimulationSemanticCutSummary`, `SimulationCutHotspot`, `SimulationCutSummary`, `AirCutRatios` (trait), `RebasedCuttingTimes`, `ToolpathKinematicRuntime`, `SimulationCutTrace` (struct only — its `impl` moves above), `SimulationCutArtifact`+impl, `SummaryAccumulator` (struct only), `KinematicsAccumulator` (struct only) | 12–1013, 1277–1337 | type definitions (ruling 5). `default_sample_count` stays because it is almost certainly the `#[serde(default = "default_sample_count")]` target for `SimulationCutIssue`, which stays; moving it would force a full-path serde attribute for a 4-line function. The four small inherent impls (`CutKinematics` 17 lines, `Engagement` 11, `SimulationCutSample` 34, `SimulationCutArtifact` 23) stay bundled with their types rather than forming near-empty children |

**Confirmation of the brief's hint:** `SimulationMetricOptions` and
`CutKinematics` are both type definitions that stay in the parent
unconditionally under this plan, so the viz crate's test-fixture references
to `rs_cam_core::stock::simulation_cut::{SimulationMetricOptions, CutKinematics}`
are untouched by the split regardless of the re-export — there is nothing
to break here even before counting the re-export.

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: 121 occurrences across 68 out-of-crate files.
- `use super::` lines the children need: none — each of the three children
  is self-contained; no child calls into a sibling, and none is called
  back by the parent.
- shared private helpers: none found across production items (confirmed
  against `coupling.txt`; the only recorded cross-reference is the digest
  quirk noted above). Spot-checked `accumulate_by_span`'s body directly
  (it constructs `SummaryAccumulator::default()`), confirming it belongs
  with `accumulate.rs`, not `reporting.rs`.

### Test module

1842–3142, 1301 lines. Moves to `stock/simulation_cut/tests.rs` (ruling 6,
`<stem>/tests.rs` — this requires converting `simulation_cut.rs` into a
directory, `stock/simulation_cut/mod.rs` plus children; `stock/mod.rs`'s
existing module declaration needs no edit). Parent keeps
`#[cfg(test)] mod tests;`.

### Risk and gate

- Risk: **L** — not a frozen folder, no exact-path sentry (confirmed
  against the brief's enumerated list), moved items are whole, and the
  coupling between production items is genuinely shallow — each child is
  self-contained with no cross-file calls in either direction.
- `--lib` filter: `cargo test -p rs_cam_core --lib stock::simulation_cut::`
- integration `--test` targets: none read this file by exact path. A
  repo grep shows 47 integration test files call
  `stock::simulation_cut::` items directly; representative ones worth a
  post-move sanity run: `sim_chipload_invariant.rs`,
  `air_cut_one_time_base_g_airdenom.rs` (exercises `rebase_cutting_times`,
  moved to `reporting.rs`), `engagement_denominator_m3.rs`. None require
  source edits.
- source-scanning sentries: none. `stock/simulation_cut.rs` is not in the
  exact-path reader list.


## 14. `crates/rs_cam_core/src/adaptive3d/mod.rs` — 3018 lines

This is the standout file of the five: its production content is only
about 540 lines. The module already delegates its real machinery to three
private submodules declared here (`mod clearing; mod path; mod search;`,
each its own file outside this digest), so `mod.rs` itself holds only the
parameter/event type model (`RegionOrdering`, `ClearingStrategy3d`,
`EntryStyle3d`, `Adaptive3dParams`, `ZLevelPlanMetrics`,
`Adaptive3dRuntimeEvent`+impl, `Adaptive3dRuntimeAnnotation`), two tiny
pre-existing `pub(super)` helpers, and seven thin public entry functions
that dispatch into `clearing`/`path`/`search`. There is no further seam in
that production half: it is already at the target size and is all type
definitions and entry points that ruling 5 keeps in the parent. The
2478-line inline test module (541–3018, 82% of the file) is the entire
reason this file is long, and moving it is the whole fix.

**Measured.** 3018 lines. 13 top-level items:
9 `fn`, 1 `impl`, 3 `struct`/`enum`, 4 `mod` declarations (`clearing`,
`path`, `search`, `tests`). No `const`/`static` at top level.
Inline `#[cfg(test)] mod tests`: lines 541–3018 (2478 lines).
Banners: 1 — L476, `// Stage 4 — the third tuple element carries
planner-predicted leading-arc...`, a doc comment inside
`adaptive_3d_toolpath_structured_annotated_traced_with_cancel` (one item;
not a split point).

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `adaptive3d/tests.rs` | `mod tests` body | 541–3018 | 2478 | stays `#[cfg(test)]`; parent keeps `#[cfg(test)] mod tests;` |

No second child. The production half (26–540, 515 lines plus a ~25-line
header) is already type definitions and entry points; ruling 5 keeps all
of it in the parent, and none of it clears the 300-line minimum for a
further split on its own.

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `mod clearing; mod path; mod search;` declarations, `RegionOrdering`, `ClearingStrategy3d`, `EntryStyle3d`, `Adaptive3dParams`, `stock_top_z_at`, `stock_has_material_above` (already `pub(super)`), `ZLevelPlanMetrics`, `Adaptive3dRuntimeEvent`+impl, `Adaptive3dRuntimeAnnotation`, and the seven `adaptive_3d_toolpath*` entry functions | ~540 | ruling 5: type definitions and public entry points; this is the entire production surface of the file |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. Nothing pub-facing moves.
  The brief's external-usage table counts 11 occurrences across 6 files
  outside core (plus 3 in-core files) for `adaptive3d`; all reach the
  types or entry functions listed above, unaffected by the test-module
  move.
- `use super::` lines the children need: `tests.rs` needs `use super::*;`
  and reaches `super::clearing`, `super::path`, `super::search` directly
  — these are private `mod` declarations in the parent, and
  `adaptive3d::tests` is a descendant of `adaptive3d`, so it already has
  access to them with no visibility change.
- shared private helpers: none needed. Everything the test module reads
  from the parent (the three submodules, the params/event types) is
  already reachable by a descendant module without edits.

### Test module

Lines 541–3018 (2478 lines) → `adaptive3d/tests.rs`. Parent keeps
`#[cfg(test)] mod tests;` and shrinks from 3018 to about 540 lines.

### Risk and gate

- Risk: **L** — no sentry reads this file by exact path, the moved item
  is the whole test module, and its coupling to the parent needs no
  visibility change at all (private-parent-to-descendant access already
  covers every case).
- `--lib` filter: `cargo test -p rs_cam_core --lib adaptive3d::`
- integration `--test` targets: none identified — the public API
  (`adaptive_3d_toolpath*`, `Adaptive3dParams`, `ClearingStrategy3d`) is
  unchanged.
- source-scanning sentries: none.


## 15. `crates/rs_cam_core/src/polygon.rs` — 3004 lines

This file is the crate's 2D polygon type (`Polygon2`, internally
nalgebra-based, converting to `geo-types`/`cavalier_contours` at operation
boundaries), its offset engine, ring-flattening for containment, boolean
ops, and self-intersection detection and repair. `impl Polygon2` (313
lines) stays whole in the parent as the type's own public API, alongside
the type itself — consistent with keeping every other file's headline
public surface physically in the parent in this batch. What moves is the
private machinery behind the free functions: the offset-cleanup pipeline,
the ring-flattening-for-containment pipeline, and the self-intersection /
geo-conversion primitives. As with every file above, the implementer
relocates each item by symbol name, not by line number — this file is
3004 lines today against the digest's 3005.

**Standout finding — this split conflicts with a ratified plan.**
`planning/structure_2026-09-17/P2_LAYOUT.md` §0 Q1 ratifies "the root keeps
… `polygon` … the root holds **8** files" and lists `polygon` among the
seven vocabulary files that stay flat, distinct from the four files Q2
explicitly turns into `folder/mod.rs` (`io`, `machine`, `dressup`,
`material`). `polygon` is not on that folder-conversion list — P2 records
it as staying a bare `polygon.rs`. But ruling 6 here requires the
1029-line test module to move to `<stem>/tests.rs`, which is only
possible by converting `polygon.rs` into `polygon/mod.rs` plus children —
the same conversion P2 deliberately does not apply to this file. The two
documents disagree. Functionally the conversion is free (`rs_cam_core::polygon::…`
resolves identically whether the file is `polygon.rs` or `polygon/mod.rs`,
and `lib.rs`'s `mod polygon;` declaration needs no edit), but it does
contradict the literal "root holds 8 files" count P2 ratified. This needs
an operator ruling — either accept the folder conversion as compatible
with the spirit of P2 (the root's crate-path surface is unchanged), or
hold this file out of any wave that runs before P2 lands.

**Also confirmed:** the two sentries the brief flagged
(`tests/common/offset_lab.rs:742,809,810` and
`tests/common/adversarial2d.rs:642`) name `polygon.rs` only inside doc
comments (`/// … polygon.rs feeds …`). Neither file reads `polygon.rs` by
exact path — their `read_to_string` calls target an unrelated caller-supplied
path and `/proc/self/status`. Confirmed by direct grep; these are prose
mentions, not sentries.

**Measured.** 3004 lines. 45 top-level items: 4 `struct`, 3 `enum`, 35
`fn`, 6 `impl` (one of them 313 lines: `impl Polygon2`), 2 `const`.
Inline `#[cfg(test)] mod tests`: lines 1977–3005 (1029 lines). Banners: 12,
split between one production pair (L1079–1081, an unlabeled `===`
separator ahead of `FlattenPolicy`) and eleven inside the test module
(section markers like "--- offset tests ---").

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `polygon/offset_engine.rs` | `offset_one`, `PLINE_POS_EQUAL_EPS`, `dedupe_pline`, `RedundantOutcome` (enum), `remove_redundant_contained`, `tidy_offset_ring`, `cleaned_flat_ring`, `offset_polygon_inner`, `cleanup_ring`, `simplify_closed_ring`, `rdp` (all priv) | 606–1082 (excludes the `pub` fns interleaved, which stay) | ~412 | `offset_one` becomes `pub(super)` (`offset_polygon_reported` in the parent calls it); `cleanup_ring` and `simplify_closed_ring` become `pub(super)` (`cleanup_collinear` and `simplify_bounded` in the parent call them); `PLINE_POS_EQUAL_EPS` becomes `pub(super)` (the parent's `OffsetRejection` reads it) |
| `polygon/ring_group.rs` | `RingGroup` (priv struct)+impl, `flatten_ring`, `subdivide_ring`, `flatten_for_containment` (all priv) | 1223–1426 | ~204 | `RingGroup` becomes `pub(super)` — the parent's `OffsetRingSet` struct holds a `RingGroup` field and its `impl OffsetRingSet` constructs one |
| `polygon/primitives.rs` | `ring_to_geo`, `ring_from_geo`, `ring_perimeter`, `multipolygon_to_polygons`, `normalized_clone`, `polygon_contains_polygon`, `point_in_polygon` (`pub(crate)`), `ring_aabb`, `polygon_bbox`, `ORIENT_EPS`, `orient`, `on_segment`, `segment_bboxes_overlap`, `segments_intersect`, `ring_has_self_intersection`, `point_segment_distance_sq`, `ring_near_point` (all priv except `point_in_polygon`) | 1673–1976 (excludes `detect_containment`, `largest_by_area`, `shoelace_area`, which stay) | ~275 | `point_in_polygon` stays `pub(crate)`, parent adds `pub(crate) use primitives::point_in_polygon;`; `polygon_contains_polygon` and `point_segment_distance_sq` become `pub(super)` — `impl Polygon2` (staying in the parent) calls `multipolygon_to_polygons`, `normalized_clone`, `point_segment_distance_sq`, `ring_aabb`, `ring_from_geo`, `ring_has_self_intersection`, `ring_near_point`, `ring_perimeter`, `ring_to_geo` directly, so nearly every item in this child needs `pub(super)` |
| `polygon/tests.rs` | `mod tests` | 1977–3005 | 1029 | unchanged (`#[cfg(test)]`) |

`ring_group.rs` (204) and `primitives.rs` (275) sit under the 300-line
floor; both are kept separate from `offset_engine.rs` because they serve
different pipeline stages (containment flattening; low-level geometry
predicates consumed mainly by `impl Polygon2`).

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `Polygon2`+impl | 79–440 | the type definition and its own 313-line public API (`new`, `union`, `intersection`, `contains_point`, …) — this is the file's headline surface |
| `OffsetFailure`+impl, `OffsetRejection`, `offset_polygon`, `offset_polygon_reported` | 441–605 | type definitions + entry points |
| `cleanup_collinear`, `simplify_bounded` | 919–1009 | `pub` entry points wrapping the offset-engine helpers |
| `FlattenPolicy`+impl+`impl Default` | 1083–1222 | type definition + its own small builder API |
| `OffsetRingSet`+impl | 1427–1563 | `pub` type + its own API; needs `RingGroup` from `ring_group.rs` |
| `detect_containment`, `largest_by_area`, `shoelace_area` | 1564–1672, 1760–1789 | `pub` entry points |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0, using the parent re-export.
  Without it: 114 occurrences across 101 out-of-crate files — the largest
  external-usage count of any file in this batch, second only to
  `compute::catalog`.
- `use super::` lines the children need: `primitives.rs` supplies most of
  what `impl Polygon2` (parent) needs — the parent needs
  `use primitives::{multipolygon_to_polygons, normalized_clone, point_segment_distance_sq, ring_aabb, ring_from_geo, ring_has_self_intersection, ring_near_point, ring_perimeter, ring_to_geo};`;
  the parent also needs `use offset_engine::{offset_one, cleanup_ring, simplify_closed_ring};`
  and `use ring_group::RingGroup;`.
- shared private helpers: `point_segment_distance_sq` (home:
  `primitives.rs`, called by `impl Polygon2` in the parent and by
  `cleanup_ring`/`rdp` in `offset_engine.rs` — stays in `primitives.rs`,
  `pub(super)`, `offset_engine.rs` writes `use super::primitives::point_segment_distance_sq;`).

### Test module

1977–3005, 1029 lines. Moves to `polygon/tests.rs` (ruling 6,
`<stem>/tests.rs`) — this is the conversion that conflicts with
P2_LAYOUT.md, flagged above.

### Risk and gate

- Risk: **M** — no exact-path sentry and not a frozen folder, but
  `impl Polygon2` (staying whole in the parent) reaches into all three
  proposed children for its helper functions, which is exactly the
  "shared private helper forces a judgement" trigger, compounded by the
  101-file external-usage count that makes the re-export genuinely
  load-bearing. The P2-layout conflict above is a separate, additional
  risk not captured by the numeric scale — resolve it before scheduling,
  independent of this letter grade.
- `--lib` filter: `cargo test -p rs_cam_core --lib polygon::`
- integration `--test` targets: none read this file by exact path (the two
  candidate sentries are doc mentions only, confirmed above). A repo grep
  shows 84 integration test files reference `polygon::` items; a
  representative subset: `offset_polygon_degenerate_inputs_r1.rs`,
  `adversarial_2d_campaign_r2.rs`, `property_tests.rs`,
  `tier_islands_i1.rs`. None require source edits.
- source-scanning sentries: none by exact path. Two files
  (`tests/common/offset_lab.rs`, `tests/common/adversarial2d.rs`) name
  `polygon.rs` in prose only — confirmed above, not a needle.


## 16. `crates/rs_cam_viz/src/app/mcp.rs` — 6067 lines

**Tree-move note.** `digest3.txt` measured 6068 lines; the live file now
measures 6067 (a peer commit landed one line elsewhere while this analysis
ran). Every span below is a digest line number. Relocate each item by its
symbol name (`grep -n 'fn mcp_load_project'`), not by the digest line
number, before you edit.

This file is GUI-side MCP dispatch. One `impl super::RsCamApp` block (4295
lines) holds `handle_mcp_request`, the 200-plus-line `mcp_set_ui_view`, and
about 70 `mcp_*` handler methods, one per wire request. After the impl
block, free functions build the cut-trace and span-inspection JSON the
handlers return. The seam is the MCP surface each handler serves: project
and session reads, toolpath generation, simulation and cut trace,
diagnostics and spans, feeds and tool load, and view or overlay control.
Ten tests read this file by its exact path, several of them pinning text
that sits *inside* specific handler bodies, so the split's real work is
answering, function by function, which needle follows which handler.

**Measured.** 6067 lines (digest: 6068). 14 top-level `fn`, 1 `impl`
(4295 lines, `super::RsCamApp`), 2 `mod`, 4 `struct`. Inline
`#[cfg(test)] mod tests`: lines 5371–6068 (698 lines).
Banners: L1141 `// Phase 4 — the two-sided kinematic sentence.` (inside
`kinematics_narration_sentence`); L1214 `// Phase 3 — say WHICH feeds were
read...` (inside `mcp_narrate_toolpath`); L4887 `// Step 2 D — per-kinematics
axes inline...` (inside `build_per_depth_pass_summary`).

### Proposed children

All moved methods are `impl super::super::RsCamApp { … }` blocks with no
re-export at all (the brief's inherent-impl mechanism): the methods stay
reachable as `RsCamApp::mcp_x(...)` regardless of which file compiles them.
Free helper functions keep whatever visibility they had.

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `app/mcp/project.rs` | `mcp_project_summary`, `mcp_list_toolpaths`, `mcp_list_tools`, `mcp_list_tool_library`, `mcp_list_tool_catalog`, `mcp_list_setups`, `mcp_get_toolpath_params`, `mcp_get_operation_schema`, `mcp_inspect_model`, `mcp_inspect_stock`, `mcp_inspect_machine`, `mcp_list_machine_library`, `mcp_inspect_brep_faces`, `mcp_load_project`, `unsaved_project_refusal` | 728–2378 (non-contig.) | ~730 | all `priv`, unchanged; `impl super::super::RsCamApp` |
| `app/mcp/generation.rs` | `mcp_get_suggest_rationale`, `mcp_recommend_clearing_strategy`, `mcp_get_tool_load_report`, `mcp_optimize_toolpath`, `mcp_apply_feeds`, `mcp_add_toolpath_via_gui`, `mcp_plan_multitool_finishing`, `multitool_plan_spec`, `mcp_preview_tier_map`, `validate_svg_out_path`, `mcp_generate_toolpath`, `mcp_generate_all`, `mcp_plan_error`, `mcp_preview_error`, `send_preview_error` | 1256–3462 (non-contig.) | ~1010 | all `priv`, unchanged |
| `app/mcp/diagnostics.rs` | `mcp_get_diagnostics`, `narration_tool_for`, `mcp_narrate_toolpath`, `kinematics_narration_sentence`, `mcp_get_generation_debug_trace`, `mcp_inspect_spans`, `mcp_diagnostic_snapshot`, `mcp_runtime_error_diagnostics`, `mcp_get_toolpath_diagnostics`, `mcp_get_project_diagnostics`, `viz_project_evidence`, `expand_span_kind_synonyms`, `map_debug_kind_to_span_kind`, `parse_span_kind_filter`, `span_to_json`, `build_inspect_spans_response` | 1014–5370 (non-contig.) | ~940 | `parse_span_kind_filter`, `span_to_json`, `map_debug_kind_to_span_kind` go `pub(super)` (used from `simulation.rs` sibling below); `viz_project_evidence` stays `pub(crate)` |
| `app/mcp/simulation.rs` | `mcp_runtime_status_for_toolpath_id`, `mcp_get_cut_trace`, `mcp_inspect_collisions`, `mcp_run_simulation`, `mcp_collision_check`, `mcp_screenshot_simulation`, `mcp_sim_jump_to_move`, `mcp_sim_jump_to_start`, `mcp_sim_jump_to_end`, `mcp_sim_scrub_toolpath`, `mcp_sim_playback_state`, `CutTraceRequest`, `build_cut_trace_response`, `sim_mesh_in_world_frame`, `build_span_cut_summaries`, `build_per_depth_pass_summary`, `render_per_kinematics_json` | 950–5073 (non-contig.) | ~1130 | `CutTraceRequest` stays `pub(crate)`; calls `super::diagnostics::parse_span_kind_filter` |
| `app/mcp/view.rs` | `reach_overlay_background`, `mcp_reach_map`, `mcp_screenshot_toolpath`, `mcp_screenshot_gui`, `pump_mcp_gui_screenshot`, `complete_mcp_gui_screenshot`, `mcp_set_ui_view` | 3636–4283 | ~650 | `pump_mcp_gui_screenshot`, `complete_mcp_gui_screenshot` stay `pub(crate)`; rest `priv` |
| `app/mcp/tests.rs` | the inline `mod tests` body | 5371–6068 | ~700 | unchanged; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `mod commands;`, `ScreenshotToolpathOptions`, `RestAnalysisDials`, `MultitoolDials`, `parse_tier_strategies` | ~100 | header items already in place; small, unrelated to the split |
| `drain_mcp_requests`, `begin_mcp_frame`, `end_mcp_frame`, `mcp_pump_beat`, `publish_mcp_read_snapshot`, `MCP_READ_PUBLISH_INTERVAL` | ~112 | pump/frame plumbing every surface shares |
| `handle_mcp_request` | 416 | the dispatch entry point (ruling 5); also holds the text five sentries pin: the one `McpRequestKind::Core` arm, all `push_notification`/toast literals, the `mcp_load_project` call site's toast, and the `SetToolpathDebugOptions` discard needle at its call into `mcp_generate_toolpath` |
| `mcp_show_simulation_workspace` | 22 | small dispatch-adjacent helper |
| `ui_query` | 49 | the view READ door entry point (ruling 5); its `&self` signature and its `UiQuery::{id:?}(` arms are pinned by two `command_surface_completeness.rs` tests |
| `mcp_send_progress`, `mcp_mutation_error`, `mcp_mutation_result`, `select_toolpath_for_mcp`, `notification_json` | ~136 | small plumbing helpers called from several handler groups |
| `parse_workspace`, `workspace_key` | 22 | called from `handle_mcp_request` (stays) and from `tests` (child, but a private ancestor item is visible to a descendant module without a re-export) |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. External callers use `app::mcp` (viz) at 0 files (measured), so no crate-external path changes.
- `use super::` lines the children need: `simulation.rs` needs `use super::diagnostics::{parse_span_kind_filter, span_to_json, map_debug_kind_to_span_kind};` (all three marked `pub(super)` in `diagnostics.rs`, which reaches every sibling of `app::mcp` per the corrected shared-helper rule — no `shared.rs` needed).
- shared private helpers: `render_per_kinematics_json` (used only inside `simulation.rs` by `build_span_cut_summaries` and `build_per_depth_pass_summary` — no cross-child split needed, stays private in that one child).

### Test module

Moves: 698 lines (over the 500-line bar) to `app/mcp/tests.rs`, per ruling 6.
`app/mcp.rs` keeps `#[cfg(test)] mod tests;`.

Moving it changes two things a plain `MCP_SOURCES` edit does not cover:

1. `command_surface_completeness.rs:137` (`MCP_SOURCES`) and
   `production_writes_go_through_apply_wp15a.rs:104` (its own, separate
   `MCP_SOURCES` list) both name whole files; add
   `"src/app/mcp/tests.rs"` to both lists, and update the doc comment at
   `command_surface_completeness.rs:132-136` that currently says these two
   files "cannot be split" — it is describing this exclusion list, not a
   property that breaks.
2. `workspace_menu_complete_g_wsmenu.rs:47` builds `MCP_SRC` with
   `include_str!("../src/app/mcp.rs")` and then asserts
   `MCP_SRC.contains("for ws in Workspace::ALL {")`. That text is the body
   of the `workspace_keys_round_trip` test, which lives *inside* `mod
   tests` (line 6059). Moving the test module makes this assertion false
   even though nothing behavioural changed. This is **not** a `MCP_SOURCES`
   case — it is a second, independent `include_str!` of the same file — and
   it is the one instance where splitting out `mod tests` makes a sentry
   simpler to fix wrong and harder to fix right: the fix is to also
   `include_str!("../src/app/mcp/tests.rs")` and check either string, not
   to skip the check.

### Risk and gate

- Risk: **H** — 10 exact-path readers (`ui_string_hygiene.rs`,
  `command_surface_completeness.rs` ×3, `effects_are_stamped_wp19.rs` ×2,
  `viewport_draws_selected_only_wp27.rs`, `workspace_menu_complete_g_wsmenu.rs`,
  `mcp_toasts_report_outcome_g_mcptoast.rs`,
  `open_guard_asks_before_discarding_g_openguard.rs`,
  `mcp_core_arm_describes_every_row.rs`,
  `production_writes_go_through_apply_wp15a.rs`, `overlays_registry.rs` ×2),
  well past the H bar of three.
- `--lib` filter: `cargo test -p rs_cam_viz --lib app::mcp::`
- integration `--test` targets: every file listed under Sentry facts above,
  plus `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs` for the
  directory-walk half (unaffected) and the `JUSTIFIED`-entry half (affected,
  see needle table).
- source-scanning sentries: see the per-needle table below.

### Per-needle table (every exact-path reader)

| sentry (file:line) | needle it asserts | lives in (item) | proposed child | sentry edit needed? |
|---|---|---|---|---|
| `ui_string_hygiene.rs:82` | scans the whole file for the cargo-fmt baked-indentation defect in operator-visible strings | whole file | n/a (directory-independent literal list) | none — it already names `"src/app/mcp.rs"` as one of three fixed extra paths (line ~80); add the new child paths to that same list if any hold operator-visible strings (`view.rs`, `project.rs` do) |
| `command_surface_completeness.rs:138` | `"src/app/mcp.rs"` is a member of `MCP_SOURCES` | the list itself | n/a | **yes** — add every new child path (`app/mcp/project.rs`, `generation.rs`, `diagnostics.rs`, `simulation.rs`, `view.rs`, `tests.rs`) |
| `command_surface_completeness.rs:648` | `mcp.contains("fn ui_query(&self, query: UiQuery) -> UiQueryAnswer")` | `ui_query` | stays in parent | none |
| `command_surface_completeness.rs:663` | `mcp.contains("UiQuery::{id:?}(")` for every `UiQuery`-kind `UiCommandId` | `ui_query`'s match arms | stays in parent | none |
| `effects_are_stamped_wp19.rs:86` | `JUSTIFIED` entry: path `"src/app/mcp.rs"`, needle `"Command::SetToolpathDebugOptions("` | inside `mcp_generate_toolpath` | `app/mcp/generation.rs` | **yes** — update the entry's `path` field to `"src/app/mcp/generation.rs"` |
| `effects_are_stamped_wp19.rs:682` | `!mcp.contains("mcp_stamp_stale")` / `!mcp.contains("stamp_stale")` (negative) | nowhere (already absent) | n/a | none — negative check, stays true on a smaller parent |
| `viewport_draws_selected_only_wp27.rs:327` | locates `"fn mcp_screenshot_toolpath"`, then asserts its body excludes `toolpaths_to_draw`/`isolate_toolpath`/`show_all_toolpaths` | `mcp_screenshot_toolpath` | `app/mcp/view.rs` | **yes** — read `source("src/app/mcp/view.rs")` instead (or in addition) |
| `workspace_menu_complete_g_wsmenu.rs:47` | `MCP_SRC.contains("for ws in Workspace::ALL {")` | inside `mod tests` (`workspace_keys_round_trip`) | `app/mcp/tests.rs` | **yes** — see Test module section above |
| `workspace_menu_complete_g_wsmenu.rs:47` (2nd use) | finds `"open_job_from_path"`, then `"fit_camera_to_first_model()"` within 800 chars, inside `mcp_load_project` | `mcp_load_project` | `app/mcp/project.rs` | **yes** — read the child too |
| `mcp_toasts_report_outcome_g_mcptoast.rs:37` (4 tests) | `McpRequestKind::Core(request) => {` arm ordering; `OUTCOME_TOASTS` literal + `self.mcp_load_project(` proximity; bare `push_notification(` count == 3 | all inside `handle_mcp_request` | stays in parent | none |
| `open_guard_asks_before_discarding_g_openguard.rs:41` | finds `"fn mcp_load_project"`, then checks `unsaved_project_refusal` precedes `open_job_from_path`, and `discard_unsaved` precedes it too | `mcp_load_project` | `app/mcp/project.rs` | **yes** |
| `mcp_core_arm_describes_every_row.rs:32` | count of `"McpRequestKind::Core(request) => {"` == 1, plus 5 method-name substrings in the 2000 chars after it | inside `handle_mcp_request` | stays in parent | none |
| `production_writes_go_through_apply_wp15a.rs:104` | `"src/app/mcp.rs"` is a member of its own (separate) `MCP_SOURCES` list | the list itself | n/a | **yes** — same additions as `command_surface_completeness.rs:138` |
| `overlays_registry.rs:753` | finds `"fn mcp_set_ui_view"`, checks selection-write, reach pump, and `apply_overlays` calls occur in that order inside the body | `mcp_set_ui_view` | `app/mcp/view.rs` | **yes** |
| `overlays_registry.rs:877` | every non-comment line lacks `"of MEASURED area"` (negative) | nowhere (already absent) | n/a | none |

### Risk and gate — module summary

Nine of the sixteen needle rows above need a one- or two-line sentry edit
(a `MCP_SOURCES` addition, or an `include_str!`/`source(...)` path change to
the new child). None require a behaviour change or a weakened assertion —
each needle keeps asserting exactly what it does today, against the file
the relevant code actually lives in now. This is the concrete shape of
"the split forces a sentry edit" the brief warns about, enumerated per site
rather than left as a warning.


## 17. `crates/rs_cam_viz/src/ui/properties/mod.rs` — 5965 lines

**Tree-move note.** Live count is 5965 (digest measured 5966). The test
module runs 5842–5965 (124 lines) and **stays inline** — it is well under
the 500-line bar, unlike the brief's initial size estimate suggested.
Relocate every item by symbol name, not by digest line number, before you
edit; eight tests read this file by exact path and several of them locate
their needle by searching for a function name first, so a stale line
number will not silently mislead the way it would with a fixed offset.

This file draws the whole operation inspector: panel-draft apply/flush
plumbing, the Machine tab, the Feeds & Speeds tab, the Model and
Simulation side panels, a tab-badge/diagnostic-row cluster, the 1244-line
`draw_toolpath_panel` (the tab host itself), and the Linking/Dressup
param-grid tab. The seam follows those panels. The load-bearing fact for
the split is that `draw` (684 lines, the module's one `pub fn` entry
point, called from outside this file) already contains, in its own body,
the Getting-Started wording, the empty-selection fallback text, the
`Selection::Toolpath` anchor, and one of the two `area_basis_note`/
`over_statement_note` occurrences that four of the eight sentries check
for — so keeping `draw` in the parent (ruling 5) resolves four of those
eight sentries with **no edit at all**, before any per-needle table is
needed.

**Measured.** 5965 lines. 60 `fn`, 3 `impl`, 3 `enum`, 7 `mod`
(`operations`, `pills`, `post`, `setup`, `stock`, `tool`, plus the
re-export block), 3 `struct`, 1 `type`. Inline `#[cfg(test)] mod tests`:
lines 5842–5965 (124 lines) — **stays inline**.
Banners: L5273 `// --- Parameter grid helpers ---`.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `properties/panel_apply.rs` | `BoundaryRestCandidate`'s sibling apply/flush cluster: `flush_tool_draft`, `commit_tool_draft`, `apply_stock_draft`, `first_model_bbox`, `apply_panel_command`, `apply_setup_draft`, `apply_fixture_draft`, `apply_keep_out_draft`, `machine_panel_fields_moved`, `apply_machine`, `apply_machine_kinematics`, `apply_machine_import`, `flush_post_snapshot`, `flush_machine_snapshot`, `flush_toolpath_snapshot` | 76–532 (interleaved w/ parent items) | ~460 | `commit_tool_draft`, `apply_stock_draft`, `apply_fixture_draft` were `pub(crate)` — stay `pub(crate)` (external callers unaffected, no path change); `apply_panel_command`, `apply_machine`, `apply_machine_kinematics`, `apply_machine_import` go `pub(super)` (called from `draw`, staying in parent, and from `machine_panel.rs`, a sibling) |
| `properties/machine_panel.rs` | `draw_machine_library_row`, `draw_machine_panel`, `draw_machine_kinematics`, `draw_grbl_import` | 1634–2184 | ~551 | all `priv` stay `priv` relative to this child; called only from `draw` (parent) so each goes `pub(super)`; uses `super::panel_apply::{apply_machine, apply_machine_kinematics, apply_machine_import}` |
| `properties/feeds_speeds.rs` | `calculate_and_apply_feeds`, `draw_speed_controls`, `draw_feeds_card`, `prov_from_chipload`, `draw_operating_point`, `wrapped_cell`, `draw_advance_per_tooth_card`, `advance_gate_verdict_text`, `tool_type_to_lut_family`, `evidence_grade_label`, `draw_vendor_lut_viewer`, `draw_entry_preview_diagram` | 2185–3111 | ~926 | `calculate_and_apply_feeds`, `draw_speed_controls`, `draw_vendor_lut_viewer` go `pub(super)` (called from `draw`, and from `toolpath_panel.rs`, a sibling) |
| `properties/tab_badges.rs` | `TabBadges` (+impl), `compute_tab_badges`, `wrapped_small_label`, `RowTier`, `merge_stateful_gate_rows`, `render_diagnostic_row`, `evidence_line`, `confidence_chip_label`, `draw_toolpath_tabs`, `draw_geometry_wiring`, `rest_region_pathology_caption` | 3161–3583, 3905–4030, 3835–3864 (interleaved) | ~580 | `RowTier`, `merge_stateful_gate_rows`, `draw_toolpath_tabs`, `draw_geometry_wiring` go `pub(super)` (called from `draw` and/or `toolpath_panel.rs`); `use super::ToolpathTab;` for the enum the parent keeps |
| `properties/linking_dressup.rs` | `draw_linking_params`, `draw_dressup_params` (mutually recursive — must move together), plus the "Parameter grid helpers": `dv`, `depth_caution_row`, `through_cut_row`, `record_stock_to_leave`, `dv_pill`, `tooltip_for`, `dressup_active_count` | 5275–5787, 5788–5847 | ~574 | `draw_linking_params`, `dressup_active_count` go `pub(super)` (called from `toolpath_panel.rs`, a sibling) |
| `properties/toolpath_panel.rs` | `draw_toolpath_panel` (one item, per ruling 2, never split) | 4031–5274 | ~1244 | `priv` → `pub(super)` (called from `draw`, staying in parent); stated exception to the 300–1200 cap — the function itself is 1244 lines and ruling 2 forbids cutting it |
| `properties/model_sim_panels.rs` | `draw_model_properties`, `draw_simulation_panel` | 1217–1493, 1494–1633 | ~417 | both go `pub(super)` (called from `draw`) |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `mod` declarations + `pub use`/`use` header | ~52 | untouched |
| `BoundaryRestCandidate` (type) | 21 | used by `draw` (stays) and `draw_toolpath_panel` (child) — a private parent type is visible to a descendant child without a visibility change |
| `rest_grid_footprint_area` | 9 | used by `draw` directly; small |
| `PanelEdit` (struct+impl) | 55 | `pub` type near the top of the file, undisturbed |
| `draw` | 684 | the module's one public entry point (ruling 5); its body is the anchor for the Getting-Started wording, the empty-selection fallback, `Selection::Toolpath`, and one `area_basis_note`/`over_statement_note` occurrence — four sentries resolve here with no edit |
| `ToolpathTab` (enum+impl) | 49 | used directly by `draw` (stays) as well as by the tab-badges cluster (child) — simplest to leave it where its first caller already is |
| `ToolpathPanelSnapshot`, `toolpath_panel_snapshot`, `build_entry_from_session_and_gui`, `boundary_summary_line`, `project_entry_onto`, `write_entry_config_to_session`, `write_entry_runtime_to_gui`, `ReachPanelSummary` | ~292 | `pub`/`pub(crate)` API surface this module exposes to the rest of the crate (ruling 5: parent keeps public entry points and type definitions) |
| `#[cfg(test)] mod tests` | 124 | under the 500-line bar (ruling 6) |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. `ui::properties` is named by
  8 files outside this module; every one of them reaches it through the
  existing `pub use operations::{...}` re-export or through `draw`/the
  `ToolpathPanelSnapshot` API, none of which move.
- `use super::` lines the children need: `machine_panel.rs` →
  `use super::panel_apply::{apply_machine, apply_machine_kinematics, apply_machine_import};`.
  `toolpath_panel.rs` → `use super::feeds_speeds::{calculate_and_apply_feeds, draw_speed_controls, draw_vendor_lut_viewer};`,
  `use super::tab_badges::{RowTier, merge_stateful_gate_rows, draw_toolpath_tabs, draw_geometry_wiring, rest_region_pathology_caption};`,
  `use super::linking_dressup::{draw_linking_params, dressup_active_count};`,
  `use super::ToolpathTab;`, `use super::BoundaryRestCandidate;`,
  `use super::rest_grid_footprint_area;`.
- shared private helpers, resolved per the corrected rule (`pub(super)` in
  the owning sibling, no `shared.rs`): `calculate_and_apply_feeds` (home:
  `feeds_speeds.rs`, used also by `toolpath_panel.rs`); `RowTier` (home:
  `tab_badges.rs`, used also by `toolpath_panel.rs`); `draw_linking_params`
  (home: `linking_dressup.rs`, used also by `toolpath_panel.rs`, and
  `draw_linking_params`/`draw_dressup_params` call each other so they
  cannot split across two children); `merge_stateful_gate_rows` (home:
  `tab_badges.rs`, used also by the parent's inline `tests`).

### Test module

124 lines (5842–5965): stays inline as `#[cfg(test)] mod tests { ... }`
(ruling 6 bar is 500; this file is well under it, unlike the brief's
initial estimate). It calls `ToolpathTab` (stays in parent, no change)
and `merge_stateful_gate_rows` (moves to `tab_badges.rs`; parent's
`tests` module needs `use super::tab_badges::merge_stateful_gate_rows;`).

### Risk and gate

- Risk: **H** — 8 exact-path readers, past the H bar of three.
- `--lib` filter: `cargo test -p rs_cam_viz --lib ui::properties::`
- integration `--test` targets: every file in the needle table below.
- source-scanning sentries: see the per-needle table.

### Per-needle table (every exact-path reader)

| sentry (file:line) | needle kind | lives in (item) | proposed child | sentry edit needed? |
|---|---|---|---|---|
| `freshness_surfaces_g_freshrender.rs:27` | comment-marker anchor + match-arm text (`// F2.2 — the same one state...`, `FreshnessState::EditedSince =>`, `"edited since"`) | `draw_toolpath_panel` | `toolpath_panel.rs` | **yes** — `include_str!` the child, or both |
| `the_gui_can_set_a_feed_g_fscontrol.rs:169` | function-signature marker (`fn draw_speed_controls(`) + 3 setter-call strings + a `stale_since = Some` occurrence count (≥3) + 2 negative strings | `draw_speed_controls` | `feeds_speeds.rs` | **yes** |
| `the_recommendation_explains_each_row_g_whyrow.rs:207` | negative string check (`"Why is the recommendation here?"` must be absent) across the whole file | nowhere (already absent) | n/a | none — negative, survives split trivially |
| `viewport_draws_selected_only_wp27.rs:285` | positive existence check (`"Selection::Toolpath"`) + 3 negative retired-symbol checks | `draw` (line ~821) | stays in parent | none |
| `workspace_menu_complete_g_wsmenu.rs:44` | positive existence checks: `"Select an operation, tool, setup or model"`, `"5. Generate toolpaths"`/`"6. Simulate and review"`/`"7. Export G-code"` in order, plus 2 negative strings | `draw` | stays in parent | none |
| `inspector_header_wraps_g_reachwrap.rs:67` | function-body marker `"if entry.operation.op_type().supports_reach_map() {"` sliced to its own closing brace | `draw_toolpath_panel` | `toolpath_panel.rs` | **yes** |
| `inspector_header_wraps_g_reachwrap.rs:67` (2nd test) | `function_body(src, "wrapped_small_label")` | `wrapped_small_label` | `tab_badges.rs` | **yes** |
| `inspector_header_wraps_g_reachwrap.rs:67` (3rd test) | `function_body(src, "render_diagnostic_row")` | `render_diagnostic_row` | `tab_badges.rs` | **yes** |
| `inspector_header_wraps_g_reachwrap.rs:67` (4th test) | marker `"for err in &validation_errors {"` + 500-char window contains `"wrapped_small_label("` | `draw_toolpath_panel` | `toolpath_panel.rs` | **yes** |
| `inspector_header_wraps_g_reachwrap.rs:67` (5th test) | header span between `"// ── Shared header..."` and `"// Contextual diagnostics"` markers, both inside `draw_toolpath_panel` | `draw_toolpath_panel` | `toolpath_panel.rs` | **yes** |
| `inspector_width_is_tab_independent_up4.rs:57` | function-body marker `fn draw_toolpath_panel(` (brace-depth scan) | `draw_toolpath_panel` | `toolpath_panel.rs` | **yes** |
| `overlays_registry.rs:665` | positive existence check (`"toolpath_row_controls::draw"`) + 1 negative string | `draw_model_properties` | `model_sim_panels.rs` | **yes** |
| `overlays_registry.rs:856` | positive existence checks (`"area_basis_note"`, `"over_statement_note"`) + a whole-file negative-line scan (`"of MEASURED area"`) | occurs in `draw` (line ~1058) *and* in `draw_toolpath_panel` (lines ~5194–5226) | `draw` stays in parent | none — the positive check is satisfied by the copy inside `draw`, which does not move; the negative scan is safe regardless |

### Risk and gate — module summary

Nine of the fourteen rows need a sentry path/marker edit, concentrated on
two clusters: `draw_toolpath_panel` (5 of the `inspector_header_wraps` and
`inspector_width`/`freshness` rows) and its immediate neighbours
(`wrapped_small_label`, `render_diagnostic_row`, `draw_speed_controls`,
`draw_model_properties`). Keeping `draw` in the parent — already required
by ruling 5 — is what makes the other five rows free. If a lower-risk
version of this split is wanted, the single highest-leverage change is
*not* moving `draw_toolpath_panel` and its `tab_badges.rs` neighbours out
at all, in exchange for `properties/mod.rs` staying at roughly 5965 −
(panel_apply + machine_panel + feeds_speeds + linking_dressup +
model_sim_panels) ≈ 3040 lines, still over the file-split threshold that
motivated this programme but with only two sentry edits
(`the_gui_can_set_a_feed_g_fscontrol.rs`, `overlays_registry.rs:665`)
instead of nine.


## 18. `crates/rs_cam_viz/src/state/simulation.rs` — 3302 lines

**Tree-move note.** Live count matches digest (3302/3303 — one line off;
treat 3302 as current). The test module runs 2595–3302 (708 lines) and
moves out. Relocate every item by symbol name before you edit; no test in
this crate reads this file by exact path, so the risk of a stale line
number here is about the split's own correctness, not a sentry.

This file is the simulation runtime's state: about 25 small structs and
enums (caches, checkpoints, span aggregates, playback state) followed by
one `impl SimulationState` of 1347 lines holding roughly 60 methods, then
a handful of small companion `impl` blocks and free functions, then a
708-line test module. No test names this file by exact path, and the
struct definitions are the type the rest of the crate imports, so the
split can be aggressive: move whole method groups out of the one giant
`impl` block into their own files, using the brief's inherent-impl
mechanism (an `impl SimulationState { … }` in any module still produces
`SimulationState::method(...)`, reachable with zero re-export).

**Measured.** 3302 lines. 5 top-level `fn`, 14 `impl`, 3 `enum`, 1 `mod`,
25 `struct`. Inline `#[cfg(test)] mod tests`: lines 2595–3302 (708 lines,
over the 500-line bar — moves). Banners: L579/L581 and L816/L818 (paired
divider rules around section headers), L963 `// --- Convenience accessors
---`.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `state/simulation/playback_state.rs` | `impl SimulationState` fragment: `new`, `has_results`, `total_moves`, `boundaries`, `setup_boundaries`, `checkpoints`, `prior_stock_for`, `checkpoint_for_move`, `selected_toolpaths`, `set_metric_capture_enabled`, `metric_options_are_stale`, `is_stale`, `collision_check_is_stale`, `progress`, `advance`, `current_boundary`, `current_toolpath_progress`, `focused_toolpath`, `move_to_local_toolpath_move`, `current_local_toolpath_move`, `global_move_for_local`, `holder_collision_counts_by_tp`, `boundary_for_toolpath_id` | scattered inside 897–2242 | ~320 | each method keeps its existing `pub`/`pub(crate)`; no re-export, `impl SimulationState { … }` |
| `state/simulation/issue_triage.rs` | `impl SimulationState` fragment: `cached_load_report`, `cached_chipload_envelopes`, `cached_simulation_triage`, `issues`, `issue_hotspot_count`, `ensure_issue_cache`, `issue_cache_key`, `current_issue`, `focus_issue_delta`; plus free fn `issue_kind_rank` | scattered + 2570–2601 | ~460 | same; `issue_kind_rank` moves with its only real caller (`ensure_issue_cache`/`focus_issue_delta`), stays `priv` |
| `state/simulation/semantic_trace.rs` | `impl SimulationState` fragment: `playback_semantic_item`, `semantic_item_by_id`, `active_semantic_item`, `pin_semantic_item`, `clear_pinned_semantic_item`, `active_debug_span`, `trace_target_for_item`, `trace_target_for_span`, `trace_target_for_hotspot`, `trace_target_for_annotation`, `trace_target_for_cut_issue`, `current_debug_annotation_with_index`, `current_debug_annotation`, `semantic_item_bbox_in_simulation`, `pick_semantic_item_with_ray` | scattered inside 897–2242 | ~380 | same, no re-export |
| `state/simulation/runtime_metrics.rs` | `impl SimulationState` fragment: `sync_debug_state`, `semantic_runtime_metrics`, `focused_hotspot_data`, `current_cut_sample`, `runtime_hotspots`, `project_evidence`, `evidence_fingerprint` | scattered inside 897–2242 | ~260 | same, no re-export |
| `state/simulation/tests.rs` | the inline `mod tests` body | 2595–3302 | ~710 | unchanged; parent keeps `#[cfg(test)] mod tests;` |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| All 25 struct/enum definitions (`ToolpathTraceAvailability` … `SimulationRunMeta`, `SimulationState` itself) and their small companion `impl`s (`SimulationTriageCache`, `Default for IssueListCache`, `SpanAggregate`, `SpanAggregateCache`, `SpanScope`, `SimCheckpoint`, `HolderCheckScope`, `SimulationChecks`) | ~870 (27–895) | type definitions (ruling 5); each is small (≤78 lines) and tightly bound to its own struct |
| `weak_matches` | 32 | used by 8 items, most of which (4 of the small companion `impl`s above, plus `tests`) already stay in the parent; simplest to leave it there rather than invent a home for a generic weak-pointer comparison with no single topic |
| `impl Default for SimulationState`, `impl SimulationDebugState`, `impl SimulationSemanticIndex`, `impl SimulationRuntimeProfile`, `estimate_move_runtime_seconds`, `move_length_mm`, `arc_move_length`, `impl Default for SimulationPlayback` | ~327 (2243–2569) | small companion impls/helpers for structs that stay in the parent; `estimate_move_runtime_seconds` → `move_length_mm` → `arc_move_length` is a self-contained chain with one caller (`SimulationRuntimeProfile`) |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0.
- `use super::` lines the children need: none. Every moved item is a
  method inside `impl SimulationState { … }`; the brief's inherent-impl
  rule means no child needs to import anything from a sibling to make its
  own methods compile (each method already has `&self`/`&mut self` access
  to every field, since `SimulationState`'s fields stay in the parent
  struct definition).
- shared private helpers: none. No private free function is called from
  two different proposed children — the one candidate, `weak_matches`,
  stays in the parent instead (see above), and `issue_kind_rank` has a
  single home.

### Test module

708 lines (2595–3302), over the 500-line bar: moves to
`state/simulation/tests.rs`, per ruling 6. `state/simulation.rs` keeps
`#[cfg(test)] mod tests;`. The test module calls `weak_matches` (stays in
parent, reachable from a descendant without a visibility change) and
constructs `SimulationState` and its cache types directly — none of that
changes since the type definitions do not move.

### Risk and gate

- Risk: **L** — no exact-path sentry reads this file (confirmed: no test
  under `crates/rs_cam_viz/tests/` names `src/state/simulation.rs` via
  `include_str!` or `read_to_string`). 17 viz files and 5 test files name
  `state::simulation` by module path, which a split does not disturb
  (methods stay reachable, types stay in the parent). This is the
  lowest-risk file of the four and a first-wave candidate.
- `--lib` filter: `cargo test -p rs_cam_viz --lib state::simulation::`
- integration `--test` targets: none specific to this file; run the
  general viz suite touching simulation state
  (`crates/rs_cam_viz/tests/` files that construct `SimulationState`
  indirectly through `AppController`, e.g. the controller test modules)
  as a smoke check, not because any of them reads this file's source.
- source-scanning sentries: none.


## 19. `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` — 3013 lines

**Tree-move note.** Live count is 3013 (digest measured 3014). The test
module runs 2615–3013 (399 lines) and stays inline — it is under the
500-line bar. Relocate every item by symbol name, not by digest line
number, before you edit.

This file draws the per-operation parameter panels: the Heights rows
shared by every operation type, thirteen `draw_*_diagram` minimap
renderers plus the large height-vs-stock profile diagram, and the
toolpath validation and diagnostics functions the Heights tab and the
Generate button both call. The seam is exactly the one the brief already
measured: heights rows, small shape diagrams, the one big height diagram,
and validation. Each is one topic and none crosses a function boundary.

**Measured.** 3013 lines. 33 `fn`, 3 `impl`, 2 `enum`, 6 `mod`
(sub-modules: `boundary_2d`, `drill`, `engrave`, `finishing`, `project`,
`surface_3d`), 6 `struct`, 15 `const`. Inline `#[cfg(test)] mod tests`:
lines 2615–3013 (399 lines) — stays inline (ruling 6 bar is 500).
Banners: none by the digest's regex, but the file carries plain `// ──`
section dividers at line 351 (`Stepover Pattern Diagram`) that the
`bottom_z_pin_note_g_bottompin.rs` sentry uses as an end marker.

### Proposed children

| child file | items | source lines | approx. lines | visibility after the move |
|---|---|---|---:|---|
| `operations/shape_diagrams.rs` | `StepoverPattern` (enum+impl), `draw_stepover_diagram`, `draw_dogbone_diagram`, `draw_lead_in_out_diagram`, `draw_tab_diagram`, `draw_outline_diagram`, `draw_spiral_diagram`, `draw_radial_diagram`, `draw_point_set_diagram`, `draw_pencil_diagram`, `draw_steep_shallow_diagram`, `draw_inlay_diagram`, `draw_ramp_finish_diagram` | 354–1533 | ~1180 | all were `pub(super)` (visible to `properties`, the grandparent); each becomes `pub(super) fn`/`pub(super) enum` *relative to `shape_diagrams`* (visible only to `operations`) plus `operations/mod.rs` adds `pub(super) use shape_diagrams::{StepoverPattern, draw_stepover_diagram, draw_dogbone_diagram, draw_lead_in_out_diagram, draw_tab_diagram, draw_outline_diagram, draw_spiral_diagram, draw_radial_diagram, draw_point_set_diagram, draw_pencil_diagram, draw_steep_shallow_diagram, draw_inlay_diagram, draw_ramp_finish_diagram};` so `properties/mod.rs`'s existing `use operations::{draw_dogbone_diagram, ...}` import needs no change |
| `operations/height_diagram.rs` | `DiagramLine`, `BOTTOM_LINE_INDEX`, `ModelProfile`, 8 small consts (`MODEL_FLAT_EPS_MM`, `DIAGRAM_HEIGHT`, `DIAGRAM_MAX_WIDTH`, `DIAGRAM_MIN_WIDTH`, `LABEL_COLUMN_MAX_FRACTION`, `LEGEND_SWATCH_W`, `LINE_STROKE`, `LINE_STROKE_HOVER`, `SWATCH_STROKE`, `FLAT_PROFILE_STROKE`, `STOCK_HALF_WIDTH_FRACTION`, `MODEL_HALF_WIDTH_FRACTION`), `model_profile`, `draw_height_diagram` | 1534–2063 | ~530 | `draw_height_diagram` was `pub`; `operations/mod.rs` adds `pub use height_diagram::draw_height_diagram;` (already re-exported one hop up at `properties/mod.rs`'s `pub use operations::{..., draw_height_diagram, ...}` — that line needs no change). `model_profile` was `pub(crate)`; add `pub(crate) use height_diagram::model_profile;` |
| `operations/validate.rs` | `ToolpathValidationContext` (+ `ValidationTool`, `ValidationModel`, `ValidationSetup`, impl), `validate_toolpath_config`, `drill_targets_refusal`, `validate_toolpath`, `validate_geometry_selection`, `has_prior_rest_source`, `DEPTH_BEYOND_STOCK_EPSILON_MM`, `diagnostics_heights`, `depth_beyond_stock`, `ThroughCut` (+impl), `board_thickness_text`, `through_cut_message`, `profile_through_cut_line`, `profile_through_cut`, `collect_diagnostics` | 2064–2614 | ~550 | `ToolpathValidationContext`, `ThroughCut`, `validate_toolpath_config`, `validate_toolpath`, `depth_beyond_stock`, `profile_through_cut_line`, `profile_through_cut`, `collect_diagnostics` were all `pub`; `operations/mod.rs` adds one `pub use validate::{ThroughCut, ToolpathValidationContext, collect_diagnostics, depth_beyond_stock, profile_through_cut, profile_through_cut_line, validate_toolpath, validate_toolpath_config};` line — `properties/mod.rs`'s existing re-export of these same eight names needs no change |

### Stays in the parent

| item | lines | why it stays |
|---|---:|---|
| `mod boundary_2d; mod drill; mod engrave; mod finishing; mod project; mod surface_3d;` | 6 | existing sub-module declarations, untouched |
| `height_row_display`, `commit_height_row`, `draw_height_row`, `ref_label`, `find_nearest_reference`, `bottom_z_pin_note`, `BOTTOM_Z_TOOLTIP`, `draw_heights_params` | ~306 (lines 47–353) | one topic (the Heights tab rows); `draw_heights_params` is the tab's own entry point (ruling 5) and is the function the file's one sentry reads by name |
| `#[cfg(test)] mod tests` | 399 | under the 500-line bar (ruling 6) |

### Break cost

- `crate::` / `rs_cam_core::` path edits: 0. The external-usage table shows
  0 files outside the crate name `ui::properties::operations` directly —
  every external caller goes through `properties/mod.rs`'s `pub use
  operations::{...}` re-export, which does not change.
- `use super::` lines the children need: `shape_diagrams.rs`'s
  `draw_point_set_diagram` needs `use super::drill;` (the `drill`
  sub-module, already declared in the parent — visible to any descendant
  without a visibility change). `validate.rs` needs `use super::drill;`
  and `use super::project;` for the same reason.
- shared private helpers: none of the coupling-measured private helpers
  (`commit_height_row`, `height_row_display`, `diagnostics_heights`,
  `drill_targets_refusal`, `has_prior_rest_source`, `through_cut_message`)
  are called from two *different proposed children* — each is used only
  within the cluster it already sits in (heights-rows or validate), so no
  `pub(super)` sibling-reach is needed anywhere in this file.

### Test module

399 lines (2615–3013), under the 500-line bar: stays inline as
`#[cfg(test)] mod tests { ... }`. It calls `commit_height_row`,
`height_row_display` (both stay in the parent — no visibility change,
private-to-descendant already works) and the `drill` sub-module.

### Risk and gate

- Risk: **M** — exactly one exact-path sentry reads this file
  (`bottom_z_pin_note_g_bottompin.rs`), which is the M threshold (one or
  two). The three-way seam is clean and no `impl` block needs cutting.
- `--lib` filter: `cargo test -p rs_cam_viz --lib ui::properties::operations::`
- integration `--test` targets: `bottom_z_pin_note_g_bottompin.rs`
- source-scanning sentries: `bottom_z_pin_note_g_bottompin.rs:45-46`
  (`panel_source()` returns `src/ui/properties/operations/mod.rs`, then
  arm 4 does `src.find("pub(super) fn draw_heights_params")` and asserts
  the body up to the literal `"\n// ── Stepover Pattern Diagram"` contains
  `"bottom_z_pin_note"`). Both `draw_heights_params` and
  `bottom_z_pin_note` stay in the parent in this proposal (heights-rows
  cluster), and the `"// ── Stepover Pattern Diagram"` banner marks the
  start of `shape_diagrams.rs`'s content, which is *removed* from
  `operations/mod.rs` — the end-of-body search still finds this exact
  banner text unless the banner comment itself is deleted or reworded, so
  leave that one-line comment in the parent file even though the code
  after it moves. **No sentry edit needed** under this split, because the
  needle's function stays put; if a future pass moves
  `draw_heights_params` out too, this is the sentry to update.

