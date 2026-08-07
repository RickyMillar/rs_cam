# UX: make the "Use remaining stock" → simulation dependency apparent

**Status:** captured 2026-07-03, NOT started. Do AFTER the pencil reference-fidelity
R2 lands (`planning/pencil_reference_fidelity_prompt.md`) — this is an independent
GUI-only polish.

## The problem

A toolpath with stock source `FromRemainingStock` ("Use remaining stock" checkbox,
`crates/rs_cam_viz/src/ui/properties/mod.rs:3348`) can only generate meaningfully
**after** a simulation of the prior operations has produced the remaining-stock
checkpoint. Today that dependency is invisible:

- The checkbox only explains itself on *hover*.
- If you check it and Generate with no prior sim, generation **silently falls back
  to fresh stock** and pushes a transient warning notification (`controller/events/
  compute.rs` ~line 320 "run a simulation first, then regenerate"; core equivalent
  in `session/compute.rs` around the `prior_stock_arc` resolution).
- If the prior op changed since the last sim, you can silently read **stale** stock.

The data flow inverts the usual order (normal: generate→simulate; IPW/rest:
simulate prior ops → generate this op), which is inherently clunky. We are NOT
chasing a coarse always-on IPW sim (F360-style) — just making the existing
dependency legible. User ask: *"make it apparent that the previous work needs sim
for that gen to work when that button is pressed."*

## Design — a state-aware checkbox

Render an inline status row directly under the checkbox whenever it is checked, so
the dependency is apparent the instant it is pressed (not after Generate).

Precompute readiness UPSTREAM (where simulation + session state is available — the
`draw_toolpath_panel` caller at `mod.rs:523`, inside the properties panel with
`self.state`) and pass it in exactly like the existing `load_verdict` /
`stale_default_defects` / `height_ctx` precomputed params. Add one parameter to
`draw_toolpath_panel` (`mod.rs:3083`):

```rust
enum RemainingStockReadiness {
    NotUsed,                        // checkbox off → render nothing
    NoPriorOp,                      // first op in setup → nothing to inherit; hint that
    Ready    { prior: String },     // sim checkpoint exists for the predecessor
    NeedsSim { prior: String },     // checked, no prior sim → THE gap
    Stale    { prior: String },     // predecessor changed since the last sim
}
```

Readiness computation (upstream): find this toolpath's position in its setup's sim
order; the predecessor op's name = `prior`. Query simulation state the same way
generation does:
- GUI: `self.state.simulation` boundaries/checkpoints (`sim.boundaries().position(id)`
  → `pos-1` → `checkpoints().find(boundary_index==prev)` → `c.stock`), mirroring
  `controller/events/compute.rs` `prior_stock` resolution.
- `Ready` when that checkpoint exists; `NeedsSim` when it does not; `NoPriorOp`
  when there is no predecessor; `Stale` (Tier 3) when the predecessor's
  `operation_config_hash` differs from the hash captured at sim time (see
  `SetupSimToolpath.operation_config_hash` / `sim_trace_is_fresh`).

### Rendering (under the checkbox)

| State | Inline row |
|-------|-----------|
| NeedsSim | ⚠️ amber — "Reads material left by **{prior}** — run a simulation first" · **[Run simulation]** |
| Ready    | ✓ green — "Reading stock from simulation of **{prior}**" |
| Stale    | ⟳ amber — "**{prior}** changed since last sim — re-simulate" · **[Run simulation]** |
| NoPriorOp| ℹ grey — "First operation in this setup — nothing to inherit; using fresh stock" |

The `[Run simulation]` button emits the existing run-simulation `AppEvent` (the
`draw_toolpath_panel` signature already carries `events: &mut Vec<AppEvent>`), so it
is one click instead of navigating to the Simulation tab and back.

## Scope tiers

- **Tier 1 (the ask):** `NeedsSim` + `Ready` badge. Small.
- **Tier 2:** inline **[Run simulation]** button. Small, high payoff.
- **Tier 3:** `Stale` detection via op-config-hash vs sim-time hash. Medium.

Keep the transient generation-time warning notification as a backstop; the badge is
the proactive signal.

## Gates / constraints

- GUI-only change; per-crate `cargo test -p rs_cam_viz` + clippy clean.
- ONE cargo job at a time (concurrent cargo has crashed the machine repeatedly).
- Reuse the sim-checkpoint lookup already in `controller/events/compute.rs`; do NOT
  duplicate the resolution logic — factor a shared helper if the shapes diverge.
