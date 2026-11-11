# `limits/` — one limit name, one check record

Feeds contract P29-2 (`planning/feeds_contract_2026-11-03/phases/P29-2_DESIGN.md`, `P29_FINDINGS.md` section 4.4). In P29-2 the old types stay; no surface reads this folder yet.

## Files

- `mod.rs` — `LimitId` (29 members, the one list of limits) and the mappings from each old list: `of_limit_kind` (`feeds::envelope::LimitKind`), `of_criterion` (`tool_load::verdict::CriterionKind`), `of_binding` (`BindingConstraint`), `of_rpm_limit` (`feeds::RpmLimit`), `of_axial_binding` (`AxialBindingConstraint`). `DepthRule` tells the two limits of one old name (`Rigidity`, `DepthOfCut`).
- `check.rs` — `Check` (limit, value, bound, unit, side, margin, state, basis, binding, population), `CheckState` (`Within`, `Exceeds`, `NoNumber(NoNumberReason)`, `NotApplicable(Geometry)`), `BoundSide`, `basis_of_bound` (the join of `BoundSource` into `feeds::sourced::Basis`).
- `project.rs` — the projections of the old records: `of_criterion` (`CriterionStatus`), `of_envelope` (`SafeEnvelope` at a planned point), `of_judged_value`, `of_rpm_choice`.

## Invariants

1. No signature and no code of this folder names an operation type, an operation family or a pass role, and the folder imports nothing from `compute/` or `session/`. A limit sees the cut, the tool, the stock and the machine (P29 design 4.5). The sentry scans the folder.
2. Each mapping from an old list is a match that names every variant. A second argument that tells two limits of one old name is a typed value (`DepthRule`, `ChipSide`), never a bool.
3. `Geometry` is a closed list of geometry reasons. A limit that an operation skips by a scope decision is `NoNumber(NoBound)` or `NoNumber(NotJudgedHere)`, never `NotApplicable`.
4. A projection takes the state of the old record. It does not judge a second time: `of_criterion` reads `LoadState`, `of_envelope` reads the margins of `SafeEnvelope::placing_margins` (the fold of `place`).
5. `Basis::setting` and `Basis::gates_export` (`feeds/sourced.rs`) equal the methods of the `BoundSource` that `basis_of_bound` read.

## Open items

- `Check::bound_detail` (the numbers of a `BoundSource` that `Basis` does not hold) is transitional. It goes at P29-10.
- `NoNumberReason::NoBound` (the depth of a finish pass) and `NotJudgedHere` (a limit on a fixed axis, a per-point limit) are scope facts of today. P29-3 lists them for a ruling.
- `Basis::setting` gives `"tool"` for every `Rule` and for a `Derived` basis that is not the drill envelope. It is exact only for a basis from `basis_of_bound`.
- `DepthRule::of_limit` tells the Factor rule from the force budget by the `law` text of the limit (`feeds::envelope::RIGIDITY_FACTOR_LAW`). P29-3 replaces that with a typed fact on the cut.
- No old type is built from `Check` yet, and no surface reads `Check` (P29-2b: MCP; P29-10: every surface).

## Sentry

`cargo test -p rs_cam_core -q --test every_limit_is_a_limit_id_p29_2`
