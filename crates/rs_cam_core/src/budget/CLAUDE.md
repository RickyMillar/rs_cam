# `budget/` — the memory budget, the job guard, the estimators, the grid caps

Plan: `planning/memory_budget_2026-10-01/PLAN.md` (B1, B2, B3, B5).

## Files

- `mod.rs` — `MemoryBudget` (`limit_bytes: None` = no limit), `MemoryLimit`
  (the configured value), `StopReason`, `parse_byte_size`, `format_bytes`,
  `format_exact_size`, `system_memory_bytes`, `DEFAULT_SYSTEM_FRACTION`.
- `guard.rs` — `BudgetGuard`: one per job, shared by `Arc`. The cancel flag,
  the first stop reason, a rate-limited RSS probe. Implements `CancelCheck`.
- `estimate.rs` — the held-result formula R after waves 1-2, term by term
  (`SimulationEstimate`), the preflight and `largest_cell_that_fits`.
- `grid.rs` — the ONE source of every dexel grid cap (`GridCapRole`) and the
  dexel rounding (`grid_cells`).
- `settings.rs` — the ONE `settings.toml` loader and writer for the GUI and
  the CLI. The flag overrides the file. Tests never `set_var`.

## Invariants

- `DEFAULT_SYSTEM_FRACTION` is `Some(0.5)`: operator ruling 2026-10-02,
  half of the system RAM. No limit is a valid state, never an error.
- Every per-element size is `size_of` of the real type. Do not type a byte
  count. Cite the source file of each term in its doc.
- A budget never coarsens a simulation grid silently. It acts through the
  preflight refusal and the guard; the grid caps stay budget-free.
- The three caps keep their legacy values (16 M, 8 M, 4 M) as fractions of
  `GRID_CELL_CEILING`. A change moves all three; the sentry pins them.
- The guard stops a job only when the process crosses the limit during the
  job; the first writer of the flag owns the reason. A set flag with no
  reason reads as `StopReason::User`.
- An `&AtomicBool` algorithm reads `guard.flag()`; `guard.watch()` polls the
  probe for it. `interrupt::FlagCancel` adapts a flag to `CancelCheck`.
- No `unsafe`, no libc: platform reads use `memory-stats` and `sysinfo`.

## Sentries

- `cargo test -p rs_cam_core -q --test memory_budget_core_b1_b2_b3`
- `cargo test -p rs_cam_core -q --lib budget::` and
  `--lib session::rest_stock::tests` (the one `Auto` rule, bit for bit).
