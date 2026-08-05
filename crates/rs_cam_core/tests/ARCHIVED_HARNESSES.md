# Archived mega harnesses

Two former integration-test files used to live in this directory and no
longer do:

| former path | archived to | why |
|---|---|---|
| `p2c_headless_ab_wanaka.rs` | `planning/archive/mega_harnesses_2026-08-05/p2c_headless_ab_wanaka.rs.archived` | reads the live, user-modified `planning/airrun_2026-06-01/wanaka.toml` at 15 sites; asserts a stale 2026-07-07 collision baseline; two probes have a filesystem test-ordering dependency |
| `v3_cascade_ab.rs` | `planning/archive/mega_harnesses_2026-08-05/v3_cascade_ab.rs.archived` | hard-codes an absolute, machine-local path (`/home/ricky/Downloads/wanaka100/rivmap_export/terrain.stl`) that does not exist on any other checkout; campaign closed 2026-07-28 "NOT PROVABLE on this fixture" |

Both moves happened together as part of the same disposition pass. Neither
file was deleted — each was renamed to `.rs.archived` (so it can no longer
compile or be picked up as a cargo test target) and got a dated banner
comment prepended recording exactly why it was archived, its stale
constants (quoted verbatim, not transcribed), and what was extracted out of
it before the move.

**Ruling**: Checkpoint E, 2026-08-05, Q3/E5 —
`planning/review_2026-08-04/MEGA_HARNESS_POLICY.md` §2/§3/§7;
`planning/review_2026-07-29/ORCHESTRATION_LOG.md:23`.

**What survived, reusable, in `tests/common/`**: the parts of both
harnesses that were genuinely byte-identical twins (verified by direct
diff, not just read-through) now live in `common/bandmap.rs` (the
`BandMap` territory instrument + deviation histogram — the stock-mesh-
vertex-deviation lineage, NOT the `column_deviations`/COLUMNS lineage
`strategy_comparison_h4.rs` uses) and `common/chain.rs` (the F.4
generate/simulate fixpoint ladder + measurement-resolution re-sim). See
those modules' own doc comments for the donor, the exact line ranges, and
the two parameters (`tool_radius`, and the output-subdirectory name via
`ensure_target_subdir`) that replace the intended per-harness deltas.

A third harness, `strategy_comparison_h4.rs`, was reviewed in the same
pass and is **not** archived — it stayed in place and was marked
`SCHEDULED` (owner: finishing/quality lane) in its own module doc. See
that file for details.
