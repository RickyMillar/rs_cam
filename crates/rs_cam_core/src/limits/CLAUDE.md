# `limits/` — one limit name, one check record

Feeds contract P29-2 and P29-3 (`planning/feeds_contract_2026-11-03/phases/P29-2_DESIGN.md`, `P29-3_DESIGN.md`). The old types stay; no surface reads this folder yet.

## Files

- `mod.rs` — `LimitId` (28 members, the one list of limits) and the mappings from each old list: `of_limit_kind` (`feeds::envelope::LimitKind`), `of_criterion` (`tool_load::verdict::CriterionKind`), `of_binding` (`BindingConstraint`), `of_rpm_limit` (`feeds::RpmLimit`), `of_axial_binding` (`AxialBindingConstraint`). `Rigidity` and `DepthOfCut` name the force budget (P29-4 deleted `DepthRule` and `DepthFactor`).
- `check.rs` — `Check` (limit, value, bound, unit, side, margin, state, basis, binding, population), `CheckState` (`Within`, `Exceeds`, `NoNumber(NoNumberReason)`, `NotApplicable(Geometry)`), `BoundSide`, `basis_of_bound` (the join of `BoundSource` into `feeds::sourced::Basis`).
- `project.rs` — the projections of the old records: `of_criterion` (`CriterionStatus`), `of_envelope` (`SafeEnvelope` at a planned point), `of_judged_value`, `of_rpm_choice`.
- `cut.rs` (P29-3 A) — `Cut` (kind, engagement, point, source, facts), `CutKind`, `EntryKind` (moved here; `compute::catalog` re-exports it), `CutEngagement` (`Side`, `AxialPlunge`, `NoModel`), `CutSource`, `SampleDepth` (M4), `CutFacts` (`Leave`, `DepthLimit`). The facts hold label values until P29-4 to P29-7. `Cut::planned_drill` feeds the plunge doors (`feeds::cut_load::CutLoad::of_cut`).
- `sample.rs` (P29-3 C) — `Cut::of_sample` (`None` for air), `CutKind::of_sample`, `CutKind::is_judged_today` (the kind form of the span filter; a dogbone is `DressupRelief` by its `DressupArtifact` span, which wins over the cap table), `SampleContext` (tool, spans, `CapTable`, body facts; built once per toolpath, the caller passes the facts), `CapTable` / `MoveCap` (`Move::feed_cap` + `FeedCap` items). The sample does not grow (G-SIMMEM); no allocation per sample. No product reader yet.
- `exemptions.rs` (P29-3 A) — `EXEMPTIONS`, the one list of every exemption from a limit. A scope read in product code carries a marker comment `EXEMPT` with its row ids; a row names the files of its markers. Ids `C-*`: rows found while commit C was built.

## Invariants

1. No signature and no code of this folder names an operation type, an operation family or a pass role, and the folder imports nothing from `compute/` or `session/`. A limit sees the cut, the tool, the stock and the machine (P29 design 4.5). The sentry scans the folder.
2. Each mapping from an old list is a match that names every variant. A second argument that tells two limits of one old name is a typed value (`ChipSide`), never a bool.
3. `Geometry` is a closed list of geometry reasons. An exemption is `Physical(Geometry)`, `NeedsRuling` or `Ruled` (a quoted operator ruling); no fourth class. A limit that an operation skips by a scope decision is `NoNumber(NoBound)` or `NoNumber(NotJudgedHere)`, never `NotApplicable`.
4. A projection takes the state of the old record. It does not judge a second time: `of_criterion` reads `LoadState`, `of_envelope` reads the margins of `SafeEnvelope::placing_margins` (the fold of `place`).
5. `Basis::setting` and `Basis::gates_export` (`feeds/sourced.rs`) equal the methods of the `BoundSource` that `basis_of_bound` read.

## Open items

- `Check::bound_detail` (the numbers of a `BoundSource` that `Basis` does not hold) is transitional. It goes at P29-10.
- `NoNumberReason::NoBound` (the depth of a finish pass) and `NotJudgedHere` (a limit on a fixed axis, a per-point limit) are scope facts of today: the rows `S-NoBound` (ruled, = P28-02) and `S-NotJudgedHere` (needs a ruling) of `EXEMPTIONS`.
- `Basis::setting` gives `"tool"` for every `Rule` and for a `Derived` basis that is not the drill envelope. It is exact only for a basis from `basis_of_bound`.
- Commit B: `compute/catalog/cuts.rs` (`OperationConfig::cuts`) fills the facts; `PowerCases`, the depth rule of the envelope, Suggest and the modulator, the offset ring and the session thinning read them. The gate readers wait (the gate context holds no config). Commit C: `Cut::of_sample`; the gates keep the span filter until P29-6, and a sentry holds the two equal. The marker link is weak (a read with no marker is not found); P29-last replaces it with a search guard.
- No old type is built from `Check` yet, and no surface reads `Check` (P29-2b: MCP; P29-10: every surface).

## Sentry

`cargo test -p rs_cam_core -q --test every_limit_is_a_limit_id_p29_2 --test the_exemption_list_only_shrinks_p29_3 --test a_sample_states_its_cut_kind_p29_3` (the need-a-ruling count is pinned; a closed row lowers the pin; the kind equals the span filter on every sample).
