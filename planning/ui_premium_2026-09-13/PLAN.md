# rs_cam GUI premium pass — implementation plan

Date: 2026-09-13. Branch to start from: `master` at `78bb7b01`.
Read `AUDIT.md` for the evidence and `DESIGN_SPEC.md` for the rules.

Nine work packages, **UP1** to **UP9**. Each is one worktree and two
commits. They run in order. UP1 and UP2 block everything after them.

---

## 0. Rules for every package

1. **No behaviour change.** No package changes what a control does, where a
   control lives, what a number means, which rows appear on which tab, any
   threshold, any feeds constant, any default value, or anything in
   `rs_cam_core`. A package that finds it needs one of those **stops and
   writes a row in `STATUS.md`** instead.
2. **No new control.** UP1 to UP8 restyle controls that already exist. No
   package therefore adds a row to
   `crates/rs_cam_viz/src/ui_command.rs`. **If a package does add a
   control, it adds a registry row, that row declares `gui: Reach::Reached`,
   and a production view file constructs it** — the WP23 census in
   `crates/rs_cam_viz/tests/command_surface_completeness.rs` enforces this
   and a package must not weaken it.
3. **One cargo lane.** Another session owns it. A package runs its gates
   when the lane is free, and never in parallel with another package.
4. **Two commits, red first.** Commit 1 is the sentry alone and it FAILS on
   the tree as it stands. Commit 2 is the change and the sentry passes. The
   commit body states how the red was observed.
5. **The sentry is named after the package**, in the crate's existing
   idiom: `crates/rs_cam_viz/tests/<subject>_up<N>.rs`.
6. **Gates before commit 2:** `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets --features
   rs_cam_core/heavy-tests -- -D warnings`, and `cargo test -p rs_cam_viz`.
   The FULL core gate runs once, in UP9. Never run workspace-wide
   `cargo test`.
7. **Lint reminders that bite this work:** `print_stdout` and `print_stderr`
   are denied in tests too; a source-scan sentry that reports a count must
   use `assert!` with a message, not `println!`. `indexing_slicing` is
   denied outside test modules. `wildcard_imports` is denied, so a token
   module is imported by name.
8. **Acceptance is a screenshot pair.** Each package names the captures it
   must produce. It captures the same view before and after, at the same
   window size, with the same project. The before images already exist in
   `planning/ui_premium_2026-09-13/`. The after images go beside them with
   an `_up<N>` suffix. The operator looks at the pair.
9. **The MCP boot opens a second GUI process.** A package that captures
   screenshots must **close its GUI when it finishes.**
10. **Commit only your own files, staged by explicit path.** Never
    `.mcp.json`. Never `Cargo.toml`. Never push. Never open a pull request.

### Sentry shape

Three of the sentries below are source scans. The crate already has the
idiom and a package should copy it rather than invent one:
`crates/rs_cam_viz/tests/ui_string_hygiene.rs:78-105` and
`crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs`.
Both walk `src/`, strip line comments, stop at the test module, and carry a
non-vacuity guard so an empty scan cannot pass.

**Every source-scan sentry in this plan must carry a non-vacuity guard.** A
scan that finds no files, or an allowlist that has grown to cover
everything, must fail.

---

## UP1 — the token and theme module

**Goal.** One module owns every colour, space, radius, elevation and text
style. `configure_theme` sets a complete `Style`, not ten colours.

**Delivers.**

- `crates/rs_cam_viz/src/ui/tokens.rs` — `DESIGN_SPEC.md` §2 and §3.2 as
  constants and one `apply(ctx)` function.
- `crates/rs_cam_viz/src/ui/theme.rs` keeps every existing public name and
  re-exports from `tokens.rs`. **No call site changes in this package.**
  The 20 constants get the spec's values; `TEXT_DIM` and `TEXT_FAINT`
  collapse onto `INK_50`; `UNKNOWN` is added.
- `app.rs::configure_theme` (`:1060-1082`) sets the full `Style`, all from
  `tokens`, and switches to `ctx.all_styles_mut` so both themes are written
  — today's `set_visuals` / `set_global_style` pair writes one
  (`DESIGN_SPEC.md` §10.3). It sets: the **five built-in** text styles,
  `item_spacing` `(4, 4)`, `spacing.interact_size.y = 26.0`, `wrap_mode`,
  corner radius 4 on all five `WidgetVisuals` plus 8 for window and menu,
  `window_shadow`, `popup_shadow`, the surfaces and the selection pair.
- **`TextStyle::Small` moves from 9 to 11.** This one assignment lifts all
  516 `.small()` call sites without editing any of them, and is the largest
  single legibility gain in the programme.
- `app.rs::configure_fonts` (`:1085-1111`) loads Inter and JetBrains Mono
  from `crates/rs_cam_viz/assets/fonts/` and keeps both Noto fallbacks in
  their current order.
- The four `CentralPanel` fills (`app.rs:346-350`, `:377-381`, `:414-418`,
  and the simulation layout) read `tokens::SURFACE_SUNKEN`.

**Sentry.** `crates/rs_cam_viz/tests/panels_read_the_token_module_up1.rs`.
A source scan over `crates/rs_cam_viz/src/ui/` and `src/app.rs`:

1. No file outside `ui/tokens.rs` and `render/colors.rs` names
   `Color32::from_rgb(`. The count today is **412 call sites carrying 201
   distinct triples**, plus 41 distinct `from_rgba_*` values. The assertion
   carries the number and the file list so the red is legible.
2. `configure_theme` sets all five built-in `TextStyle` entries, and
   `TextStyle::Small` resolves to 11.0 or more. A headless `Context` asserts
   the resolved size rather than scanning for a literal, so the arm cannot
   be satisfied by a comment.
3. `Style::wrap_mode`, `spacing.interact_size.y` and both shadows are set.
4. Non-vacuity: the scan visited at least 40 files, and `theme.rs` still
   exports all 20 original names.

**Red on today's tree** at arm 1 (412 sites), arm 2 (`Small` is 9.0 and no
text style is set at all) and arm 3.

**Acceptance.** Re-capture `shot_02`, `shot_03`, `shot_09`, `shot_12`,
`shot_17` as `*_up1.png`. The layout is unchanged and only the ground, the
type and the radii move. `shot_09` is the telling one: the readiness
cycle-time caution should become readable with no code touching that file.

**Must not change.** Any layout, any string, any control.

**Note on the 412 sites.** UP1 does not migrate them. It adds the module and
the sentry, and the sentry's first arm therefore stays RED until UP8. Split
it: arm 1 asserts a **descending budget** that each later package lowers, so
every package's commit shows the number falling. UP8 sets it to zero.
Record the budget in this file's table, not in the test's own history.

Two files dominate and they belong to UP4: `ui/properties/mod.rs` holds 96
colour literals against 7 `theme::` references, and
`ui/properties/operations/mod.rs` holds 72 against **zero**.

| After | `Color32::from_rgb` call sites outside the token modules |
|---|---|
| UP1 | 412 (baseline recorded) |
| UP3 | ≤ 330 |
| UP4 | ≤ 200 |
| UP5 | ≤ 160 |
| UP6 | ≤ 70 |
| UP7 | ≤ 20 |
| UP8 | 0 |

---

## UP2 — the component set

**Goal.** Every pattern in `DESIGN_SPEC.md` §4 exists once, and the nine
components already in `ui/components/` are extended rather than duplicated.

**Delivers.** In `crates/rs_cam_viz/src/ui/components/`:

- `SectionHeader` (new). `UiExt::named_section` (`section.rs:35-42`)
  becomes a call into it, so 105-plus existing call sites gain the
  treatment without being edited.
- `Card` (new). `theme::card_frame` (`theme.rs:41-50`) becomes a call into
  it and keeps its signature.
- `StatusChip` (new). It renders `toolpath_panel::status_chip`'s seven
  states. **The pure function at `toolpath_panel.rs:679-714` does not
  change** — not its states, not its words, not its hover text. Only
  `PEND`'s colour moves, from `TEXT_DIM` to `UNKNOWN`.
- `Button` (new): `Primary`, `Default`, `Quiet`, `Danger`.
- `KeyValueRow` — the four-slot geometry added to `value_row.rs`, with the
  per-panel label width and the wrapping trailing slot.
- `DataTable` (new), a wrapper over `egui::Grid`.
- `EmptyState`, `Banner`, `NotMeasured` (new). `EmptyState` generalises the
  one good empty state the product already has, `ui/sim_op_list.rs:128-170`.
- `CountPill` extended per spec §4.4.
- `motion.rs` (new): the durations in spec §5 as named helpers over
  `ctx.animate_bool_with_time_and_easing` with `emath::easing::cubic_out`.
  The plain `_with_time` call is hardcoded to linear (`DESIGN_SPEC.md`
  §10.4).
- `text.rs` (new): the three rungs egui cannot carry as global styles —
  `Display`, `Subhead`, `Micro` — as `RichText` constructors, plus
  `numeric()` for the monospace value run. `Micro` applies its 0.8 pt
  tracking through `RichText::extra_letter_spacing`, and `Body` and
  `Caption` apply line height through `RichText::line_height`; neither is
  settable globally on 0.34 (§10.2, §10.8).
- **The focus ring is a component-layer guarantee.** egui 0.34 draws no
  focus indicator and renders a focused widget in its `active` visuals
  (§10.5), so every interactive component in this module draws its own
  2-point `ACCENT` ring outside its rect.

**Sentry.** `crates/rs_cam_viz/tests/component_contracts_up2.rs`, a unit
test with a headless `egui::Context`:

1. `StatusChip` renders all seven `FreshnessState` values and each produces
   the spec's word and role colour.
2. `Button::Primary` and `Button::Default` produce different fills, and
   every variant reports a height of at least 26.
2b. Every interactive component draws a focus ring when its `Response` has
   focus, and the ring lies outside the widget rect.
3. `EmptyState` renders at most one button.
4. `KeyValueRow`'s trailing slot wraps instead of clipping at a panel width
   of 240.
5. Non-vacuity: each assertion names the component it exercised, and the
   test fails if the component list is shorter than the spec's.

**Red on today's tree** because none of the seven new types exists.

**Acceptance.** A single capture, `shot_up2_gallery.png`, of a scratch
window that draws one of each component. It is a test fixture, not a
shipped surface.

**Must not change.** No production panel calls the new components in this
package. UP2 builds the kit. UP3 onward installs it.

---

## UP3 — chrome: menu bar, workspace bar, status bar, toasts, windows

**Goal.** The frame around every workspace is one surface.

**Delivers.**

- `ui/workspace_bar.rs`: tab colours from tokens (they are literals at
  `:93-102`), the active indicator animated over 160 ms, and the hint text
  (`:34-39`) raised from `.small()` / `TEXT_FAINT` to `Caption` /
  `TEXT_MUTED`.
- `ui/status_bar.rs`: the pipe-separated string at `:24-28` becomes discrete
  slots with one separator kind; thousands grouped with a thin space;
  every count in `Numeric`.
- The three copies of the status-bar block in `app.rs` (`:340`, `:370`,
  `:425`) collapse to one helper, and the `rgb(255, 200, 80)` literal goes
  to tokens. **UP3 does not add the bar to the Simulation workspace**, which
  has none today (`AUDIT.md` D-42). That bar carries an actionable
  collisions chip, so adding it is a behaviour decision and it belongs to
  the operator. UP3 records the question in `STATUS.md`.
- `app.rs:918-957`: toasts become the `Toast` component — shadow, left rule,
  slide and fade, and `request_repaint_after(16 ms)` while any toast lives.
- All twelve `egui::Window` sites get `SURFACE_OVERLAY`, `SHADOW_OVERLAY`,
  `RADIUS_MD` and the §6 footer order.
- **The scrim is an operator decision and UP3 does not take it.**
  `egui::Modal` exists in 0.34 and would give the backdrop for free, but it
  also blocks input, which is a behaviour change and rule 1 forbids it. UP3
  paints the scrim by hand, generalising
  `ui/optimize_project.rs:566-580`, and writes a `STATUS.md` row proposing
  `Modal` for the eight task windows and plain windows for Overlays,
  Shortcuts and Load Warnings (`DESIGN_SPEC.md` §6, §10.1).
- The Project Load Warnings window anchors to the viewport centre.

**Sentry.** `crates/rs_cam_viz/tests/chrome_is_one_surface_up3.rs`:

1. A source scan: every `egui::Window::new` site in `src/` is inside a
   helper that applies the overlay frame. The scan lists the twelve sites
   from `AUDIT.md` §6 by file and fails on any that is not. The crate uses
   `egui::Modal` zero times and UP3 keeps it that way — the scrim is
   visual, and no window's input handling changes.
2. A headless render of the status bar asserts one separator kind and no
   literal `"|"` in its format strings.
3. Non-vacuity: the window list is non-empty and matches the twelve files.

**Red on today's tree** at arm 1 (twelve bare windows) and arm 2 (the pipe).

**Acceptance.** `shot_01_up3.png` — the load-warnings window no longer
covers the workspace switcher. `shot_13_up3.png` — a toast with a shadow.
`shot_10_up3.png` — the Tool Library window over a scrim.

---

## UP4 — Toolpaths workspace

**Goal.** The product's busiest screen reads at a glance.

**Delivers.**

- `ui/toolpath_panel.rs`: the card becomes `Card`; `status_chip`'s draw site
  (`:409-418`) becomes `StatusChip`; `draw_trace_badge` (`:443`) uses the
  same chip form; the operation name's `rgb(190, 190, 200)` literal
  (`:446-451`) becomes `TEXT_STRONG`; the two empty states (`:107`, `:203`)
  become `EmptyState`.
- `ui/toolpath_row_controls.rs`: the six squares get `Micro` labels, the
  `Quiet` treatment, a 26-point hit target and two groups.
- `ui/properties/mod.rs` and `ui/properties/operations/mod.rs`: every
  labelled row becomes `KeyValueRow` with one label-column width per panel;
  every section mark becomes `SectionHeader`; the tool-load caution becomes
  a `Banner`; `Generate` becomes `Button::Primary`; the manual-generation
  line reserves its height so the panel does not jump.
- The inspector header becomes a fixed-height block.

**Sentry.** `crates/rs_cam_viz/tests/toolpath_inspector_layout_up4.rs`:

1. Headless render of the inspector for one operation, with and without the
   manual-generation line: **the y position of the tab strip is identical.**
   That is `AUDIT.md` D-17 as an assertion.
2. Render the same inspector on the Geometry tab and the Feeds tab: the
   panel's requested width is identical (D-16).
3. No value row's trailing text is clipped at a panel width of 280 (D-16).
4. Exactly one `Button::Primary` renders in the workspace (spec §6).
5. Non-vacuity: the render produced at least 20 rows.

**Red on today's tree** at arms 1, 2 and 4.

**Acceptance.** `shot_03_up4.png`, `shot_04_up4.png`, `shot_05_up4.png`,
`shot_07_up4.png`. The operator compares `shot_03` with `shot_03_up4` and
`shot_04` with `shot_04_up4` for the jump.

**Must not change.** No row moves between tabs. No row is hidden. No label
text changes.

---

## UP5 — Setup workspace

**Goal.** The three right-hand panels title themselves the same way, and the
first screen an operator meets is designed.

**Delivers.**

- `ui/properties/mod.rs:594-598`: the placeholder becomes `EmptyState`.
  **The sentence stays exactly as F1.13 set it** and becomes the second
  line.
- `ui/properties/setup.rs`, `stock.rs`, `tool.rs` and the machine panel: a
  `Heading` title with a rule, as the Machine panel already has
  (`AUDIT.md` D-12); every row a `KeyValueRow`; read-only values without a
  well and editable values in one (D-13).
- `ui/setup_panel.rs`: the setup card becomes `Card` with a `KeyValueRow`
  body; the three `.italics()` empty states (`:100`, `:227`, `:238`) become
  `EmptyState`.

**Sentry.** `crates/rs_cam_viz/tests/setup_panels_share_one_header_up5.rs`:

1. Headless render of all four right-hand panels: each emits exactly one
   `Heading` as its first laid-out element.
2. The properties panel with no selection emits an `EmptyState` and its
   text still contains the F1.13 sentence verbatim.
3. Non-vacuity: four panels rendered, not fewer.

**Red on today's tree** at arm 1 (three of four have no heading).

**Acceptance.** `shot_02_up5.png`, `shot_16_up5.png`.

---

## UP6 — Simulation workspace

**Goal.** The verification screen states one verdict and draws its data
once.

**Delivers.**

- `ui/sim_diagnostics.rs`: the collision line becomes a `Banner`; the
  Findings badges become `CountPill`s, with `collisions N` a `Verdict` pill
  at `DANGER` (`AUDIT.md` D-28); the `View` help paragraph moves to a button
  hover (D-27); the nine `.italics()` sites become `EmptyState` or
  `NotMeasured`, starting with `:41-48`, where the whole Inspector is one
  9-point italic line before a run.
- `ui/sim_op_list.rs`: the card becomes `Card`; the three-facts line splits
  into three `KeyValueRow`s with safety first (D-29).
- `ui/sim_timeline.rs` (54 colour literals): the transport keeps the
  transport and loses the duplicated badge row (D-28); the timeline gains a
  time axis, tick marks and operation boundaries (D-24); the signal chart
  gains a y scale, units on x and a legend built from the same colour
  function it draws with (D-25).
- The "No cutting metrics for this run." string is the one **copy** change
  in this plan. It becomes "No cutting metrics for this operation." and the
  `Re-run` button beside it is removed, because it would change nothing
  (D-26). Record this in `STATUS.md` as a deliberate exception to rule 1,
  for the operator to confirm.

**Sentry.** `crates/rs_cam_viz/tests/simulation_readout_up6.rs`:

1. The verdict badge set renders **once** per frame: a scan of the
   Simulation layout finds one `CountPill` call site group, not two.
2. Every colour-carrying element in the timeline reads its colour from the
   same function that builds its legend. The test compares the two.
3. The drill-operation message contains the word "operation" and not "run",
   and no `Re-run` button renders beside it.
4. Non-vacuity: the frame produced a timeline and at least three pills.

**Red on today's tree** at arms 1 and 3.

**Acceptance.** `shot_12_up6.png`, `shot_13_up6.png`, `shot_14_up6.png`.

---

## UP7 — Readiness, modals and the wizard

**Goal.** The pre-cut screen uses the window it is given, and separates
safety from procedure.

**This package carries the largest share of the type migration.** Five files
hold 322 of the 516 `.small()` sites and four of them are windows:
`feeds_modal.rs` 98, `optimize_modal.rs` 59, `sim_diagnostics.rs` 56,
`multitool_planner.rs` 56, `optimize_project.rs` 53. Size UP7 accordingly;
the first draft of this plan put that weight on UP4, which holds 44.

**Delivers.**

- `app.rs:381-386`: the column widens from 560 to 880 points.
- `ui/readiness_panel.rs`: the banner becomes `Banner`; the rows become a
  `DataTable` with a fixed action column (D-32); two `SectionHeader` groups,
  **Safety** and **Program** (D-31); a not-run row takes `UNKNOWN`, not
  `CAUTION`; the closing row has exactly one `Primary` (D-33); the
  cycle-time caution rises from 9 points to `Caption`.
- `ui/export_wizard.rs` (25 `.italics()` sites): the breadcrumb becomes the
  step rail in spec §6; the footer takes the §6 order.
- `ui/feeds_modal.rs` (48 colour literals): the `Current / Recommended / Δ`
  table becomes the reference `DataTable`; the nomogram's nine colours drop
  to the semantic five plus the neutral ramp; the colliding labels get
  leader lines or move to the legend; the legend stops being clipped (D-37).
- `ui/preflight.rs`, `ui/tool_library_modal.rs`,
  `ui/machine_library_modal.rs`, `ui/multitool_planner.rs`,
  `ui/optimize_modal.rs`: `EmptyState` in every empty pane, `Banner` for
  every message, spec §6 footers.

**Sentry.** `crates/rs_cam_viz/tests/readiness_and_modals_up7.rs`:

1. The Readiness page renders exactly one `Button::Primary`, and it is
   `Run simulation` when a safety row is not `OK` and `Export G-code…` when
   every safety row is `OK`.
2. A readiness row whose state is "not run" renders `UNKNOWN`, not
   `CAUTION`. Drive every row state.
3. Every modal footer renders cancel-left, primary-right.
4. Nothing in the Feeds modal legend is clipped at 1280 x 800.
5. Non-vacuity: at least six readiness rows and at least five modals.

**Red on today's tree** at arms 1, 2 and 3.

**Acceptance.** `shot_09_up7.png`, `shot_11_up7.png`, `shot_15_up7.png`,
`shot_10_up7.png`.

**Must not change.** No readiness row is added, removed or reworded. No
wizard step is added, removed or reordered. No feeds number moves.

---

## UP8 — the 3D layer

**Goal.** The workpiece is the subject, and a red line can only mean a
collision.

**Delivers.** In `crates/rs_cam_viz/src/render/colors.rs` and the two
buffer builders that read it:

- `TOOLPATH_PALETTE` (`colors.rs:9-18`) becomes the six-step cool ramp in
  spec §7.1. Red, amber and green leave the move palette.
- Cutting, rapid and unselected-toolpath opacities per §7.1.
- The five height-plane colours become one cool ramp, and the Heights tab
  diagram reads the same five values (§7.2). The two-letter labels expand.
- `STOCK_SOLID_FACE` (`colors.rs:47`) desaturates per §7.3.
- The last `Color32::from_rgb` sites in `src/` move to tokens, so UP1's
  budget reaches zero.

**Sentry.** `crates/rs_cam_viz/tests/viewport_palette_up8.rs`:

1. No entry in `TOOLPATH_PALETTE` is within a stated distance of
   `COLLISION_POINT`, of `theme::DANGER`, of `theme::CAUTION` or of
   `theme::OK`. The distance and the metric are in the test.
2. The Heights tab diagram and the height-plane renderer read the same five
   constants. The test compares them.
3. UP1's budget arm asserts zero.
4. Non-vacuity: the palette has at least six entries and the height ramp
   exactly five.

**Red on today's tree** at arms 1 and 3.

**Acceptance.** `shot_06_up8.png`, `shot_07_up8.png`, `shot_13_up8.png`.
`shot_13` is the decisive one: orange stock under pure green becomes a muted
oak part under a cool path.

**Must not change.** No geometry, no camera, no overlay rule, no registry
row in `ui/overlays/registry.rs`. This package changes values, not which
overlays exist.

---

## UP9 — acceptance and the full gate

**Goal.** Prove the whole pass on one build.

**Delivers.**

1. The FULL core gate, once:
   `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`.
   **The expected read is not green.** `adaptive_feed_modulation_pipeline_f036b`
   carries two arms that are red by design, `perf_golden_sim_metrics` carries
   G-PERFGOLDEN2D, and `replace_toolpath_config_gates_on_the_signature`
   carries a stale `cli` claim. All four are ledgered in
   `planning/arch_consolidation_2026-09-09/STATUS.md`. A fifth red is this
   pass's to answer for.
2. `cargo clippy --workspace --all-targets --features
   rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check`.
3. `cargo test -p rs_cam_viz`, `-p rs_cam_mcp`, `-p rs_cam_cli`.
4. A full re-capture of all sixteen views with the `_up9` suffix, at
   1600 x 1000 and again at 1280 x 800, on the wanaka project.
5. A read of every capture at 1280 x 800 against spec §9: no clipped text,
   no target under 26 points, every disabled control carries a reason.
6. `FEATURE_CATALOG.md` gains no row. **This pass ships no feature.** If a
   package changed a visible string — and exactly one did, in UP6 — the
   catalog records it.

**Sentry.** None of its own. UP9 is the census: every package id above maps
to a sentry file that exists and passes, in the shape of
`planning/ui_fix_2026-09-09/` V6.3.

---

## Order, and what blocks what

```text
UP1 tokens ──► UP2 components ──┬─► UP3 chrome
                                 ├─► UP4 Toolpaths
                                 ├─► UP5 Setup
                                 ├─► UP6 Simulation
                                 └─► UP7 Readiness + modals
UP8 3D layer  (needs UP1 only; run it after UP4 so the before/after
               captures are not confounded)
UP9 acceptance (needs all)
```

UP3 to UP7 are independent of each other and touch different files. On one
cargo lane they still run serially. Conflicts, if any, land in
`ui/properties/mod.rs`, which UP4 and UP5 share.

## Where status is recorded

`planning/ui_premium_2026-09-13/STATUS.md`, append-only, one row per
package: date, package id, state (`done` / `partial` / `blocked`), commit
hash, sentry name, the `Color32` budget after the package, and any open
question. Do not rewrite an earlier row. The file does not exist yet; UP1
creates it.

## Risks

| Risk | Handling |
|---|---|
| The font licence review rejects Inter or JetBrains Mono. | `DESIGN_SPEC.md` §3.1 names the fallback. The scale is the load-bearing part. |
| Raising the floor from 9 to 11 points overflows a dense panel. | **Measured and much smaller than first thought.** Form row labels already render at 13 points, because `ValueRow` uses a plain label; the two inspector files hold 44 of the 516 small-text sites and the modals hold 322. UP4's sentry arm 3 still measures clipping at 280 points, and UP1 sets a global `Style::wrap_mode` so overflow wraps rather than clips. |
| The font licence work or the weight mechanism stalls UP1. | Weight travels on `FontFamily::Name`, one named family per weight, each with the symbol fallbacks appended (`DESIGN_SPEC.md` §10.3). If fonts stall, ship UP1 without them: the `Small` slot moving to 11 is independent of the typeface and delivers most of the gain. |
| The crate pins egui 0.34 while upstream is 0.36.2. | 0.35's `Classes` and `AtomLayout` are both squarely aimed at this kind of work, and 0.36 adds the global line spacing §10.2 says is missing. Building UP2 on 0.34 and upgrading later means rebuilding part of it. Operator decision; it touches `Cargo.toml`. |
| A restyle silently changes behaviour. | Rule 1, plus `cargo test -p rs_cam_viz`, which already holds 48 sentries including the freshness, export-parity and command-surface families. |
| The `Color32` budget invites a package to move a literal without thinking. | The budget is a ceiling, not a target. The per-package sentries assert the *shape*; the budget only stops regression. |
| UP6 changes one operator-visible string. | Recorded in `STATUS.md` as a deliberate exception for the operator to confirm, and reflected in `FEATURE_CATALOG.md` in UP9. |
