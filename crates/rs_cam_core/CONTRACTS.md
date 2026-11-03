# rs_cam_core contracts

The feeds contract (`planning/feeds_contract_2026-11-03/PLAN.md`) names the
rules that the compiler holds. One line per contract: the rule, the type or
setting, the file and the sentry. Each folder `CLAUDE.md` that a contract
touches names it in its invariants.

- Lint: no wildcard arm over two or more variants; `Cargo.toml`
  `wildcard_enum_match_arm` (deny). An exception is
  `#[expect(clippy::wildcard_enum_match_arm, reason = "..")]` on the `let`
  statement or the smallest `fn`. Sentry: the full clippy gate.
