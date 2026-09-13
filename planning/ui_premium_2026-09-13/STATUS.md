# UI premium programme — status ledger

Append-only. One row per event, dated. Do not rewrite an earlier row.

Read `AUDIT.md` for evidence, `DESIGN_SPEC.md` for the rules, `PLAN.md` for
the work packages. This file is the only place that records what has been
DECIDED and what is still OPEN.

## Where things stand — 2026-09-13

**No package has started.** UP0 is unblocked and is next.

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
| Q2 | **`egui_plot` pairing.** The compatibility scout reports 0.37.0 pairs with egui 0.36. Confirm at resolve time rather than assume; its numbering does not track egui's. | UP0 |
| Q3 | **Simulation has no status bar.** Three of four workspaces have one (`AUDIT.md` D-42). The bar carries an actionable collisions chip, so adding it is a behaviour decision, not a visual one. | UP3 |
| Q4 | **The IA boundary.** Several findings are half layout, half information architecture (D-14, D-22, D-26, D-27, D-28). UP4 and UP6 will reach the line. Decide per screen what it is FOR before those packages start. | UP4, UP6 |

## Blocking dependencies — CLEARED 2026-09-13

- ~~The 14 core clippy findings must land before UP0~~ — **done by the core
  lane as WP30** (`f092cd67`). `cargo clippy -p rs_cam_core --all-targets
  -- -D warnings` exits 0 on 1.98.1.
- ~~The toolchain is unpinned and drifts per machine~~ — **done**
  (`89268d49`): `rust-toolchain.toml` pins `1.98.1`. This was recommended
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
