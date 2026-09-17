# UI premium programme — status ledger

Append-only. One row per event, dated. Do not rewrite an earlier row.

Read `AUDIT.md` for evidence, `DESIGN_SPEC.md` for the rules, `PLAN.md` for
the work packages. This file is the only place that records what has been
DECIDED and what is still OPEN.

## Where things stand — 2026-09-13

**The programme is COMPLETE.** UP0 to UP9 are done. The next phase is SUBTRACTION, and it is a different programme — see the last section.

| Artefact | State |
|---|---|
| `AUDIT.md` | 43 findings, 18 screenshots, two projects. Done. |
| `DESIGN_SPEC.md` | Tokens, type, 12 components, motion, per-screen hierarchy, toolkit constraints. Done. |
| `PLAN.md` | UP0 to UP9. Done. |
| Drawn specimen | Published as a private artifact. Find it with `/artifacts` in Claude Code, or the gallery at claude.ai/code/artifacts. Title: **rs_cam Instrument Specimen**. |

## Operator rulings

| Date | Ruling |
|---|---|
| 2026-09-13 | **Upgrade egui.** "I give the go ahead to upgrade egui to use its new features." Supersedes the original brief's ban on touching `Cargo.toml`, for that file and that purpose only. Became UP0. |
| 2026-09-13 | **Newest everything, if it works together.** Target is egui / eframe / egui-wgpu 0.36.2 plus egui_plot 0.37.0, in one hop. |
| 2026-09-13 | **One toolchain, not two.** The repo moves to Rust 1.98.1 and the core lane fixes the 14 clippy findings the bump surfaces, rather than pinning UP0 to an explicit `+1.98.1`. |
| 2026-09-13 | **Row rhythm is 22 points.** From looking at the drawn specimen: the parameter rows "look a bit too spaced apart" and should "retain some density". Row height became a token of its own, deliberately off the 4-point spacing scale. |
| 2026-09-13 | **N of a thing needs a rule.** Produced `NoticeStack` (`DESIGN_SPEC.md` §4.11), with severity outranking the cap. |

## Open — needs an answer before the package that consumes it

| Id | Question | Blocks |
|---|---|---|
| Q1 | **The toast cap.** Capping the visible toast stack at four is a rendering rule on a surface that renders everything today. TTLs, severities and the `get_notifications` wire are untouched. Needs an explicit nod. | UP3 |
| Q3 | **Simulation has no status bar.** Three of four workspaces have one (`AUDIT.md` D-42). The bar carries an actionable collisions chip, so adding it is a behaviour decision, not a visual one. | UP3 |
| Q4 | **The IA boundary.** Several findings are half layout, half information architecture (D-14, D-22, D-26, D-27, D-28). UP4 and UP6 will reach the line. Decide per screen what it is FOR before those packages start. | UP4, UP6 |

## Blocking dependencies — CLEARED 2026-09-13

- ~~The 14 core clippy findings must land before UP0~~ — **done by the core
  lane as WP30** (`57841b05`). `cargo clippy -p rs_cam_core --all-targets
  -- -D warnings` exits 0 on 1.98.1.
- ~~The toolchain is unpinned and drifts per machine~~ — **done**
  (`7cad232a`): `rust-toolchain.toml` pins `1.98.1`. This was recommended
  here and adopted by the core lane.

**UP0 is therefore unblocked.** Core is green, the toolchain is pinned, the
cargo lane is free.

## What UP0 does, in one paragraph

Bump four version lines in `crates/rs_cam_viz/Cargo.toml`. Fix the one
structural break at `src/lib.rs:48-54`, where `WgpuConfiguration::present_mode`
moves into a new `surface: SurfaceConfig`. Wrap five vertex buffer layouts in
`Some` at `render/mod.rs:305,401,436,471,582`. Rename 22 deprecated sizing
calls across 11 files under `ui/`. `winit` does not move. The compile is the
proof; no test may be edited to make it pass. Full detail in `PLAN.md` UP0.

## Machine state at handover

- Default toolchain: **1.98.1**, pinned by `rust-toolchain.toml`.
- `target/` is roughly 79 GB and holds pre-1.98 artifacts that are now stale.
- `.mcp.json` is modified in the tree and belongs to nobody in this
  programme. Never stage it.


---

## UP0 — DONE, 2026-09-13, commit `473fbe17`

One commit, not two. `PLAN.md` rules that UP0 carries no sentry of its own
and that the compile is its proof, as WP25 ruled for the `mcp` feature gate.

**Shipped.** egui, eframe and egui-wgpu at **0.36.2**, egui_plot at
**0.37.0**, the wgpu family at **30.0.1**. `winit` did not move.

**Gates.** `cargo test -p rs_cam_viz` passes **700 tests across 49
binaries**. `cargo fmt --all -- --check` passes. The workspace clippy gate
with `-D warnings` and `heavy-tests` passes. No test was edited to make the
upgrade pass and no assertion was weakened.

**The renderer is proved, not assumed.**
`every_render_pipeline_builds_on_a_headless_adapter_g_pipesmoke` runs and
passes, so all six render pipelines build on a real headless adapter under
wgpu 30. The 359 `wgpu::` references needed five one-word changes.

### Q2 is answered

`egui_plot` **0.37.0** declares `egui ^0.36.0`. Read from the crate's own
manifest, not matched by number. Q2 is removed from the open list.

### Corrections to PLAN.md UP0

| The plan said | The compiler said |
|---|---|
| 22 deprecated sizing calls to rename (`.default_width` 10, `.default_height` 3, `.max_height` 9) | Those 22 call sites exist and **none is deprecated in 0.36.2**. No sizing call changed. |
| Nothing about `show_inside` | **24 `show_inside` calls** are deprecated and were renamed to `show`, across six files. A pure rename: the deprecated body is a direct call to `show`. |
| Nothing about the wgpu adapter | wgpu 30 adds `RequestAdapterOptions::apply_limit_buckets`. Set to `false`, which is wgpu's own default and the wgpu 29 behaviour. |
| Nothing about epaint | epaint 0.36 asserts in `Drop for TexturesDelta`. See below. |

### The one real regression

Three controller tests panicked **in teardown, after their bodies passed**.
epaint 0.36 asserts in `Drop for TexturesDelta` that the deltas were applied
or cleared deliberately. The headless snapshot harness at
`src/controller/tests.rs:398` discarded the `FullOutput` with `let _ =`. It
now clears the delta, which is what the assert's own message prescribes.
eframe applies the deltas in the real app; the harness has no GPU.

### Behaviour held, deliberately

`present_mode` moved into the new runtime-mutable `SurfaceConfig`. The frame
latency did **not** move with it: 0.34 defaulted to `None`, which its own doc
reads as "let wgpu pick a default (currently 2)", and
`SurfaceConfig::HIGH_THROUGHPUT` states 2 outright. The surface behaves as it
did.

### Acceptance — OUTSTANDING

`shot_03`, `shot_12` and `shot_17` are not yet re-captured as `*_up0.png`.
They need a GUI booted with `--mcp`. They should look **identical** to the
originals; a visible difference is a regression, not a win. This is the only
part of UP0 not finished.

### New machine constraint, found the hard way

54 GB of RAM, **no swap**, and the desktop normally holds about 45 GB.
`cargo test -p rs_cam_viz` at the default 24 jobs links 46 test binaries at
once, exhausts memory, and the OOM killer stops the build **and the terminal
that started it**. There is no readable `dmesg` line. **Run it as
`nice -n 10 cargo test -p rs_cam_viz -j 2`, in the background, with the
output going to a file.** Every later package inherits this.

---

## UP1 — DONE, 2026-09-13, commits `3c3c4512` (red) and `94afd33a` (green)

Two commits, red first, as the plan requires. The red was observed twice:
the sentry did not compile, because `ui::tokens` did not exist, and the
per-arm red was then measured against a headless `Context` driven by the
pre-UP1 `configure_theme`.

| Field | Pre-UP1 | UP1 |
|---|---|---|
| `TextStyle::Small` | 9.0 | **11.0** |
| `Style::wrap_mode` | unset | `Wrap` |
| `spacing.interact_size.y` | 18.0 | 26.0 |
| `spacing.item_spacing` | (6, 4) | (4, 4) |
| `spacing.extra_text_line_spacing` | 0.0 | 4.0 |
| `window_shadow` | offset (10, 20), blur 15 | offset (0, 8), blur 24 |
| widget `corner_radius` | 2 | 4 |
| colour literals outside the token homes | 382 | 378 |

**Gates.** 707 tests pass in `rs_cam_viz`, fmt passes, and the workspace
clippy gate with `-D warnings` and `heavy-tests` passes. No call site
changed: `theme.rs` keeps all 20 public names and re-exports from
`tokens.rs`.

### The intent count is 41, not 40

The specification's "40 distinct intents" is a census of the EXISTING
crate's literals, not a declared size for the new token set. Counted twice
independently — once by hand, once by a scout reading only the spec — the
set §2 actually names is **41** single-valued colour intents, plus one
shadow and three scale families. The sentry asserts 41 and says so.

### Three scales the specification named but did not value

§2.9 asks for `SPAN_SCALE` (6), `CHART_SERIES` (4) and `LANE_SCALE` (4) and
gives no hex for any of them. UP1 derived all three from the stated rule —
one hue by lightness, never a category wheel — and verified every step
against the four surfaces. `SPAN_SCALE`'s first draft put its darkest step
at **2.88**, under the 3:1 WCAG floor for a graphical object; the scale was
lifted and the darkest now reads 3.44, with adjacent steps separating by
1.15 to 1.29. **Fold these values back into `DESIGN_SPEC.md` §2.9.**

### `wrap_mode` is `Wrap`, and the choice matters

egui's own `None` default already resolves to `Wrap` in a vertical layout
and `Extend` in a horizontal one. `Wrap` therefore changes ONLY the
horizontal rows that carry the D-16 defect. `Truncate` would have cut every
banner and every sentence to one line, which UP1 must not do. The component
rule handles the rows that want a fixed line box.

### The fonts are in the repo

Inter (Regular, Medium, SemiBold) and JetBrains Mono (Regular, Medium),
about 1.75 MB, in `crates/rs_cam_viz/assets/fonts/`. Both SIL OFL 1.1, both
licence texts beside them, attribution in `CREDITS.md`. They were not in
the tree; the specification assumed they were.

### A crash class that no other gate caught

`Style::text_styles` asks for `FontFamily::Name("inter_semibold")`, and the
font loader is what registers it. When those two lists disagree epaint
panics — `FontFamily::Name("inter_semibold") is not bound to any fonts` —
at the first galley. That is a **crash on launch**, not a wrong pixel.

The font loader therefore moved out of `app.rs` and into `tokens.rs` as
`apply_fonts`, beside the constants the `Style` reads, and arm 5 of the
sentry draws a label in every style and every named family. **The arm was
verified by deliberately deleting one registration**, which reproduced the
panic exactly; the registration was then restored.

Note what does NOT work as a guard: renaming the shared constant renames
both sides at once, so the first attempt at this arm passed a broken tree.
The drift the arm catches is a MISSING registration, not a renamed one.

### Still outstanding

Acceptance screenshots for UP0 and UP1 together. UP1 changes the ground,
the type and the radii on every surface, and **nothing has been looked at
yet.** This is the largest visual change in the programme and it is
unverified by eye.

---

## UP2 — DONE, 2026-09-13, commits `20cd03c7` (red) and `76faed3d` (green)

Twelve patterns, each existing once, in `ui/components/`. Fifteen sentry
arms. 731 tests pass; fmt and the workspace clippy gate are green.

**No production panel calls a new component.** UP2 builds the kit; UP3
onward installs it. Two existing helpers were rerouted so their call sites
gain the treatment unedited: `UiExt::named_section` calls `SectionHeader`,
and `theme::card_frame` calls `Card` and keeps its signature.

### The 19 rulings

Reading §4 against §2 and §3 found **six contradictions and thirteen
underspecified points**. All are ruled in `DESIGN_SPEC.md` §4.13, committed
at `4920c8ed`. The ones that changed a shipped value:

- **R1** the overflow row reads `Showing 4 of 293 · Show all` everywhere.
  §4.10's `+3 more · View` is retired.
- **R2** a chip's stroke is `BORDER`, not the role text at 40 % alpha.
- **R6** `BodyStrong` is defined: 13 points, Inter Medium.
- **R8** `Button::Primary` contrast is computed and published: `INK_05` on
  `ACCENT` is **6.22**, on `ACCENT_PRESSED` **4.59**. The obvious
  alternative was tested and rejected — `INK_95` on `ACCENT` reads **2.32**.
- **R10** `NoticeStack` orders five roles; `OK` was missing and sorts last.

### A defect UP1 introduced, found and fixed here

**R19.** Under the new palette the seven freshness states collapse onto
three colours: `GEN`, `STALE` and `WAIT` are all `CAUTION`; `PEND` and `OFF`
are both `INK_50`. Colour alone therefore separated **two** of the seven,
not seven. §2.6 rule 3 already required a glyph beside every verdict, and
nothing consumed the four glyphs the tokens shipped. `StatusChip` now does.
The pure function that maps a `FreshnessState` to its word is untouched, and
so are the words.

### The focus ring took the global route, and the risk is measured

egui draws **no** focus ring and renders a focused widget in its `active`
visuals, so a keyboard user cannot tell focus from pressed. One
`egui::Plugin` registration covers every widget, including the ones UP2 does
not wrap.

The plan recorded the clipping risk as NOT MEASURED: `StrokeKind::Outside`
paints beyond the widget rect and clips wherever a parent `Ui` has no
margin. **Measured and avoided.** The ring draws `Inside` a rect expanded by
one point — visually identical, and it cannot be clipped by a parent that
leaves even a single point of margin. No component needed a private ring.

### A PLAN.md correction

UP2's entry claims `SectionHeader` gives "105-plus existing call sites" the
treatment for free. **Measured: `named_section` has 10 call sites.** The
other **41** headers are hand-rolled `.small().strong()` and gain nothing
automatically; §3.3 already schedules them as hand work.

### Four egui API assumptions were wrong

The compiler corrected each, and they are worth knowing for UP3:
`emath` is reached as `egui::emath`; `Context::screen_rect` is now
`viewport_rect`; `egui::Plugin` is a **trait**, not a struct taking a
closure; and `Context::pass_state` is crate-private, so a focused widget's
rect comes from the public `read_response`.

### A hazard UP3 inherits

`tests/freshness_surfaces_g_freshrender.rs:75` asserts the **source text** of
`toolpath_panel.rs` contains the literal line
`let (status_text, status_color, hover) = status_chip(freshness);`.
UP2 stayed green because it touches no production panel. **UP3 installs the
chip and will break that sentry** — a source-text pin, not a behaviour pin.
Plan for it rather than discovering it.

## Acceptance screenshots — BLOCKED, needs the operator

The MCP server IS `target/release/rs_cam_gui`, spawned by Claude Code at
session start. The running process holds a **deleted inode**: it is the
binary from before UP0. Any screenshot it takes shows the pre-UP0 interface,
which is worse than no screenshot.

The binary on disk is current and correct. **The MCP connection needs a
restart** (`/mcp` in Claude Code) before `shot_03`, `shot_12` and `shot_17`
can be re-captured. Nothing in UP0 to UP2 has been looked at by eye.

---

## First look by eye, 2026-09-14 — and it found two things

The operator restarted the MCP and the first captures were taken against a
binary carrying UP0 and UP1. **Looking at it was worth more than any test
so far.**

### The win the specification predicted

`shot_09_readiness_up1.png`. The cycle-time caution (`AUDIT.md` D-01) is now
plainly readable, and **no code in that file changed** — it came entirely
from `TextStyle::Small` moving 9 → 11. The banner, the chips and the amber
all read. §3.2 called this "the single highest-leverage line in the whole
specification" and it was right.

### A regression UP1 introduced, caught only by looking

`shot_03_toolpaths_geometry_up1.png` showed the inspector reading
**"Spoilb / oard:"**, **"Retrac / t (R):"** and **"Dressu / p"**.

The cause was UP1's `Style::wrap_mode = Some(Wrap)`. I chose `Wrap` over
`Truncate` reasoning it would change only the horizontal rows carrying D-16.
It does — and it breaks a label in a narrow grid column **mid-word**.

**The global default is unset again.** egui's own `None` resolves per layout,
`Wrap` in a vertical one and `Extend` in a horizontal one, which is right for
a label whose column sizes to its content. `Truncate` is no better: it hides
the end of a word the operator must read. **D-16 is closed per component**,
where the component knows whether its text is a label or a sentence. The UP1
sentry arm now asserts the opposite of what it asserted, and says why.

No test would have caught this. It needed a picture.

---

## UP3 — DONE, 2026-09-14, commits `b21e3294` (red) and `3733ddb8` (green)

**Operator, on seeing the first capture:** *"ohh, the tabs, the buttons. all
of that looks way more out of place now"*. Correct, and it is the predictable
middle of a migration: UP1 raised the ground, type and radii everywhere while
the chrome kept its own inline styling, so it stopped reading as *plain* and
started reading as *unfinished*. The fix is to finish the chrome, never to
soften UP1.

| Thing | Was | Now |
|---|---|---|
| Active tab fill | `from_rgb(65, 72, 95)`, a private violet-blue matching nothing | `ACCENT_QUIET` — an active tab is a SELECTED thing |
| Tab height | 28, on a 26-point scale | `ROW_ACTION` |
| Tab radius | bare `4` | `RADIUS_SM`, top corners only (§2.2: a tab joins the panel) |
| Tab badge | a bare `ui.label` floating beside the tab | `StatusChip`, so a count on a tab and a count in a panel are one thing |
| Readiness actions | "Export G-code…" and "Run simulation" at the SAME weight | one `Primary` (§4.5), and the sentry holds it to one |
| Readiness banner tints | three hand-mixed literals | `TINT_OK` / `TINT_CAUTION` / `TINT_DANGER` |
| Status bar "Modified" | `from_rgb(140, 140, 100)`, an olive on no scale | `CAUTION` |

**One deliberate shim.** The three badge producers still hand the bar a
`Color32`, so `role_for` reads the colour back to the role it meant rather
than changing three signatures inside UP3. **UP4 gives them roles directly
and the function goes away.**

738 tests pass; fmt and clippy are green.

## The screenshot loop, and its one friction

The MCP server IS `target/release/rs_cam_gui`. Rebuilding it leaves the
running process on a **deleted inode**, so every rebuild needs an operator
`/mcp` restart before the next capture. Check
`readlink /proc/<pid>/exe` for `(deleted)` before trusting a screenshot —
this is the hazard `feedback_check_gui_binary_age` records, and it bit once
already in this programme.

---

## The screenshot loop, 2026-09-14 — what looking actually bought

Four rounds of capture-look-fix. **Every one found something no test had.**

| Round | Found by looking | Cause |
|---|---|---|
| 1 | Inspector labels breaking mid-word: "Spoilb / oard:" | UP1's global `wrap_mode = Wrap` |
| 1 | Operator: *"the tabs, the buttons ... way more out of place"* | UP1 raised everything around un-migrated chrome |
| 2 | The **viewport ground was still the old violet** | The 3D clear kept `(26, 26, 38)` when the panels left it |
| 3 | A **second** tab strip with its own invented blue | `properties/mod.rs` used `from_rgb(55, 60, 80)` |
| 3 | "Dressu / p" had a **second** cause | Its tab was `min_size` 24 on a 26-point floor |

**Measured, not reasoned about.** The viewport clear's colour space was
settled by sampling `shot_18_stock_panel_up3.png`: a clear of
`(0.102, 0.102, 0.149)` rendered as exactly `(26, 26, 38)`, so the target
consumes the value directly and a plain divide by 255 is right. The two
conventions differ by roughly 3× and a wrong guess would have been visible.
Verified after: the ground is now exactly `(21, 23, 26)` = `SURFACE_SUNKEN`
against panels at `(27, 30, 34)` = `SURFACE_BASE`.

**Two tab strips, two invented palettes.** The workspace bar used
`from_rgb(65, 72, 95)` and the inspector `from_rgb(55, 60, 80)` — one step
apart, neither matching the product. Both are `ACCENT_QUIET` now, the same
fill a selected row takes.

**The most destructive control wore hand-mixed colour.** The pre-flight
"Export Anyway" — which exports G-code past a failed safety check — carried
`(180, 50, 40)` and `(80, 40, 40)`, the fourth and fifth reds in a palette
that has one. It is `Button::danger` now, and its armed and unarmed states
differ by ENABLEMENT rather than a private shade. The enablement condition
is unchanged.

### The lesson worth keeping

**738 tests, clippy and fmt were all green while the wrap bug shipped.** The
sentries verified the style was SET, not that it was RIGHT. A screenshot is
not a nice-to-have on this programme; it is the only instrument that reads
the thing the operator reads.

### Still open

- The status chips in the toolpath list carry no glyph yet. That is the
  `status_chip` migration, and it trips the source-text sentry at
  `tests/freshness_surfaces_g_freshrender.rs:75`. **UP4.**
- The workspace badges sit AFTER their tab, so "6 PENDING" renders between
  Toolpaths and Simulation and reads as belonging to neither. Layout, not
  colour. **UP4 or an IA call (Q4).**
- `role_for` in `workspace_bar.rs` still reads a colour back to a role. UP4
  gives the three badge producers roles directly and it goes away.

---

## UP4 to UP7 — DONE. The colour budget is 368 → 4

| Package | Commit | Budget after |
|---|---|---|
| UP4 inspector + toolpath list | `efe94a8e` | 209 |
| UP5–UP7 simulation, setup, modals (parallel) | `aa7dadcf` | 4 |
| R22 / R23 rulings and fixes | `ba0467d6` | 4 |

### Parallel agents: what worked and what it cost

Three agents on **disjoint file sets**, none permitted to run cargo. The lead
gated once on the merged tree and **it compiled on the first attempt.**

The constraint that made it safe is the machine, not the plan: a parallel
cargo build OOMs this box (no swap, ~9 GB free) and has already killed the
terminal once. Agents write; the lead compiles, lints, tests and commits.

**The agents found more than they migrated.** Ranked by what mattered:

1. **Two agents independently hit the same wall and both STOPPED** rather
   than force a mapping: no scale in §2.9 carries three separable hues, so
   the X/Y/Z gizmo had nowhere to go. That produced ruling **R22** and closed
   the first of §2.10's four load-bearing collisions.
2. **A bug in the lead's own work.** The bulk migration of
   `properties/operations/mod.rs` mapped one teal literal onto `DIAGRAM_INK`
   across nine sites, and in three diagrams — steep/shallow, female/male
   inlay, contour/ramp — that gave BOTH arms of a category the same colour,
   leaving a raw literal as the only separator. **A table-driven sweep cannot
   see that two sites are the two halves of one contrast.** Restored onto
   span-scale steps.
3. **Categories wearing verdict colours, everywhere.** 26 semantic span kinds
   with greens on `Pass` and reds on `SlotClearing`; the feeds vendor band in
   pass-green (the audit's known example); an amber/green/red traffic light
   on an ORDERED scale, where a band's own maximum is not an exceedance; the
   setup "Keep Out" count in red and "XY" datum in green.
4. **Three gates that returned NO verdict were drawn in caution amber** and
   are `UNKNOWN` now. That is the distinction §2.6 added the role for.

### Rulings R22 and R23

**R22 — the axis triple is the one exception to the one-hue rule.** The
defect was never "an axis is red"; it was that the axis and the error text
were ONE CONSTANT. `AXIS_X` / `AXIS_Y` / `AXIS_Z` are separate constants at
different values, each clearing 3:1 on the viewport ground. The residual is
stated: `AXIS_X` sits near `DANGER` in hue because the CAD convention puts it
there, and only CONTEXT separates them. **An axis colour must never appear in
a panel.**

**R23 — "Current" is not a verdict.** The feeds charts drew the operator's
own configured value as a `DANGER` dot in four places. Red on it said "your
setting is wrong" where the chart meant "you are here". It is `TEXT_STRONG`.

### A correction to the spec, measured

§2.9 named the simulation signal strip as `CHART_SERIES`' second consumer.
**The strip has six tracks and the scale has four steps.** The strip uses
`SPAN_SCALE`. Extending `CHART_SERIES` to six was rejected: four is right for
a chart, where more series than that is a legibility problem rather than a
palette problem.

### Verified rather than assumed

The sim agent added fourteen defensive `#[allow(clippy::indexing_slicing)]`
without being able to run clippy. The lead **deleted all fourteen and
re-linted**: nine real complaints returned, so the instinct was right and
they are restored with their SAFETY comments.

## Open for the operator

| Id | Question |
|---|---|
| Q1 | The toast cap. Still needs a nod. |
| Q3 | Whether the Simulation workspace gains a status bar. Behaviour, not visual. |
| Q4 | The IA boundary per screen. The workspace badges sit AFTER their tab, so "6 pending" renders between Toolpaths and Simulation and reads as belonging to neither. |
| Q5 | **Three "Generate All" buttons** — panel, viewport overlay, menu. The menu one is conventional; the viewport one competes with the panel's Primary. Removing a control is behaviour, so it waits for a ruling. |

## Needs looking at

- **The feeds charts are now almost entirely blue** after four traffic-lights
  left the verdict palette. That may have gone too far toward monotone.
- **Four alpha washes moved premultiplied → unmultiplied** in the setup
  panels. Their old values were mathematically invalid (rgb exceeding alpha),
  so they will read fainter.

---

## UP8 and UP9 — DONE. The programme is complete

### UP8 — the budget becomes a ban

**368 colour literals → 1**, and the one survivor is named in the test:
`sim_timeline`'s `desaturate` rebuilds a colour from channels it has just
computed. There is no colour there to tokenise; the constructor is arithmetic,
not a choice.

### `NoticeStack` had ZERO production consumers, and that is on me

The operator asked for "a way to handle n number of x". UP2 built the
component, wrote fifteen sentry arms against it, and **never wired it into the
product.** The toast stack and the load-warnings window both still iterated
their whole collection. The thing that was asked for was in the codebase and
not in the app.

Both consume it now. **Q1 is ruled: the visible toast stack caps at four.**
TTLs, severities and the `get_notifications` wire are untouched — it is a
RENDERING bound — and because it goes through `NoticeStack`, §4.11's rules
come with it: an `ERROR` toast renders however many notices are queued ahead
of it.

**The lesson: a component with no call site is not a deliverable.** A sentry
that drives a component directly will pass forever while the product never
uses it.

### UP9 — the full core gate

`cargo test -p rs_cam_core --features heavy-tests --no-fail-fast` ran for the
first time in this programme. **3909 tests pass, one fails.**

The failure is **`modulation_raises_cutting_chipload_toward_band`**
(`adaptive_feed_modulation_pipeline_f036b.rs:469`), and it is **NOT this
programme's**:

- `git log 473fbe17..HEAD -- crates/rs_cam_core` is **EMPTY**. No commit in
  this programme touched that crate.
- `rs_cam_core` does not depend on `rs_cam_viz`, so a viz change cannot reach
  it.
- The file's last commit is `a21a4a24` (WP26, 2026-09-13), the other lane
  fixing a SIBLING assertion in the same test. This is their work in flight.

**Not fixed, deliberately.** `project_ui_fix_2026-09` records the rule: a
pre-existing core red must not be "fixed" inside an unrelated task.

## The final state

| Measure | Before | After |
|---|---|---|
| Colour literals outside the token module | 368 | **1**, documented |
| Distinct grid spacings | **9** | 1 |
| `TextStyle::Small` | 9 pt | 11 pt |
| Buttons that say which action a screen is for | 3 of 132 | every screen has one Primary |
| Bounded notice renderers | 0 | 1, with 2 consumers |
| `rs_cam_viz` tests | 700 | **741** |

## The next phase is SUBTRACTION, and this programme could not do it

**Operator, 2026-09-14:** *"I wanted this clean up to really unclutter, but
instead it has just polished the clutter."*

That is correct, and the cause is structural rather than a matter of taste.
**Rule 1 of `PLAN.md`, applied to every package, was "No behaviour change. No
control moves, appears or disappears."** Every package was therefore
forbidden from SUBTRACTING. The programme could only make existing elements
better, which is exactly what happened — and several changes made the clutter
denser, because they ADDED channels: glyphs on chips, borders on badges,
hairlines under headers.

Measured on the 2026-09-14 capture: **one toolpath card carries 13 elements**
(drag handle, swatch, status chip, MAN badge, name, tool name, play button,
six icon buttons). A resting card carries 8. Nine cards is about **80
elements** in one panel. The inspector shows **twelve things above the first
parameter**, three of them prose paragraphs.

The operator's brief for the next phase: one eye icon (double-click to
isolate), a shorter card, setup cards all one height, no hover-to-expand,
everything else behind a `…` menu, drop the "inspect sim" link, one
stale/fresh/uncomputed indicator, "Generate All" at full section width with a
spinner on anything generating, the Operations header deleted, the tool
library as its own tab, and warning prose collapsed to "11 warnings" that
opens.

**One item flagged for a ruling before it is built.** The `⏻` toggle is not a
view control like the eye — it changes what gets CUT and EXPORTED. Burying a
"this operation is off" affordance inside a `…` menu on a CAM application is
how an operator gets a surprise at the machine. The recommendation is that the
disabled state stays legible on the card even when the control moves.

## Deferred rather than done, deliberately

**Q3 (a status bar for the Simulation workspace) and Q4 (tab badge placement)
were approved and are NOT built.** Both are ADDITIVE. Adding a fourth status
bar for consistency immediately before a phase whose purpose is subtraction is
work that would likely be deleted. They should be re-decided in that phase,
with everything else on the table.
