# Mega-harness policy — P7 / R6-M2

Wave: W7 (R6/M3) · Date: 2026-08-05 · Parent revision: `ebe77de`
Scope: `v3_cascade_ab.rs`, `p2c_headless_ab_wanaka.rs`, `strategy_comparison_h4.rs`
Method: static census (source + `git log`). **No Cargo command was run. None of
these harnesses was executed** — two of them cannot be, see §1.

The plan's requirement: *"Every mega harness gets a named owner, action, and
cadence/archival pointer; no unowned ignored analysis body remains
load-bearing."* Plus the standing rule: **no fourth unowned mega-harness.**

---

## 0. Headline

| | `v3_cascade_ab.rs` | `p2c_headless_ab_wanaka.rs` | `strategy_comparison_h4.rs` |
|---|---|---|---|
| lines | 5066 | 3879 | 1420 |
| `#[test]` / `#[ignore]`d | 22 / **22** | 27 / **27** | 3 / **2** |
| prose | 855 (16.9%) | 564 (14.5%) | 346 (24.4%) |
| reads `planning/` | **no** | **YES — `wanaka.toml`, 15 sites** | no (and says why) |
| runnable on another checkout | **NO** — hard-coded `/home/ricky/Downloads/...` | yes, while the user file cooperates | **yes** |
| asserted stale baselines | **none** | **yes** — `BASELINE_RAPID_COLLISIONS = 4` | none (all pre-registered) |
| uses `tests/common/` | no | no | yes |
| last content change | campaign CLOSED 2026-07-28 | campaign closed | **born 2026-08-03** |
| **verdict** | **SPLIT then ARCHIVE** | **SPLIT then ARCHIVE** | **SCHEDULED** |

---

## 1. Two corrections to the brief's premise

The brief states the first two harnesses *"carry pinned-A constants that are
STALE (they read the forbidden wanaka.toml — they must never become CI)."*
Measured, that is half right, and the half that is wrong matters for sequencing:

1. **`v3_cascade_ab.rs` does not read `wanaka.toml` at all.** The string
   appears only in comments (`:23`, `:58`, `:212`). Its machine/post/tool
   blocks are a **snapshot copied** into a 449-line TOML literal inside
   `write_fixture_project` (`:256–706`) — which is exactly the right pattern
   and should be preserved, not deleted. It also **asserts no baseline
   constants**; its gates in `verdict()` (`:3375–3509`) are self-referential
   (`collisions == 0`, cascade time `<` D time) and its doc at `:3427–3432`
   explicitly declines to inherit p2c's collision baseline.
2. **`v3_cascade_ab.rs`'s real blocker is different and worse**: `:54`
   hard-codes the absolute path
   `/home/ricky/Downloads/wanaka100/rivmap_export/terrain.stl`, asserted at
   `:55–61`. **The harness cannot run on any other machine or after that
   directory is cleaned.** It is not a stale-baseline problem; it is an
   already-dead harness that still compiles.

A third correction: the brief describes all three as `#[ignore]`d.
**`strategy_comparison_h4.rs:1322` `h4_harness_arithmetic_sentry` is NOT
ignored** and runs in the normal `cargo test -p rs_cam_core` suite. That is a
feature — it is the cheap arithmetic guard that keeps the expensive arms
honest — and the policy below preserves it deliberately.

---

## 2. `p2c_headless_ab_wanaka.rs` — the mutable-input offender

**Verdict: SPLIT, then ARCHIVE. Owner: orchestrator to route into lane E.
Cadence: none — it is never scheduled.**

Why archive rather than schedule:

- **It reads a live user file at 15 call sites** (`:45–52` `wanaka_project_path()`
  → `planning/airrun_2026-06-01/wanaka.toml`; loads at `:829, 899, 1062, 1152,
  1259, 1366, 1694, 2184, 2390, 2770, 2933, 3021, 3298, 3378, 3513`). Every one
  is `.expect("load wanaka.toml")` — it **fails fast, never skips**, which is
  the one merciful property here: it cannot be silently wrong.
- **The file already knows.** Its doc at `:55–58` records that wanaka.toml
  *"is user-live and evolves under live sessions (2026-07-09: '3D Finish 6' was
  disabled …)"*, which is why `enabled_finish_index()` (`:59–79`) exists — and
  that helper **panics if zero or more than one enabled finish op is found**. A
  user toggling an op in the GUI breaks the harness. This is the
  `wanaka_suggest_baseline` disease: *a verdict that changes when somebody
  drags a slider is not a verdict.*
- **Its baselines are asserted and dated.** `:43`
  `const BASELINE_RAPID_COLLISIONS: usize = 4` (doc: *"P0/P1 baseline,
  2026-07-07"*) is **asserted** at `:743` and `:804–810`. `:710–711`
  `PINNED_A_PROJECT_S = 8919.5` / `PINNED_A_FINISH_S = 6883.4` (measured
  2026-07-08) are printed, not asserted; the doc asks for a re-run *"whenever
  the chain or simulator changes materially"* and nothing enforces it. Since
  then the chain has changed materially several times.
- **It has inter-test file coupling**: `p2g_stamp_probe` (`:1427`) and
  `p2g_dense_env_probe` (`:2046`) read `target/p2f_fidelity/p2g_sess_*_moves.txt`
  with `.expect("run p2g_session_op8_dump first")`. Test-ordering dependencies
  through the filesystem are not something to schedule.

**Split before archiving** — the reusable half is real and is currently
trapped:

| extract to | from | approx. lines |
|---|---|---|
| `common/bandmap.rs` | `BandMap` + `BAND_NAMES` + `code_at` (`:255–278`), `build_band_map` (`:314–359`), `DEV_EDGES`/labels (`:407–427`), `BandAcc`, `band_shares` (`:3659–3698`), `outer_region_spans` (`:3630–3651`), band-map PNG (`:373–415`) | ~450 |
| `common/chain.rs` | `ChainOutcome` (`:80–95`), `run_chain` fixpoint ladder (`:96–254`), `run_measurement_sim` (`:432–448`) | ~180 |

Archive destination for the prose: the campaign narrative blocks
(`:239–248, 1241–1250, 1351–1360, 1583–1605, 2031–2043, 2253–2286, 2756–2766,
2992–3003, 3087–3100, 3277–3288, 3477–3507, 3704–3729`) belong in
`planning/review_2026-07-29/` alongside the campaign they document —
particularly `:3712`'s *"KNOWN RED on wanaka as of 2026-07-13"*, which is a
historical status note living in a test file.

---

## 3. `v3_cascade_ab.rs` — dead on arrival, valuable fixture inside

**Verdict: SPLIT, then ARCHIVE. Owner: orchestrator to route into lane E.
Cadence: none.**

- **Unrunnable** (§1.2). Not "stale" — dead.
- **Its campaign is closed.** `MEMORY.md` and `SUPERSEDED_CONCLUSIONS.md` record
  the v3 process proof as **CLOSED 2026-07-28, "NOT PROVABLE on this fixture"**.
  13 of its 22 tests are `*_probe` RCA instruments for questions already
  answered. One docstring, `:3763–3775`, is a **formal retraction** of its own
  earlier conclusion.
- **Nobody has touched its content since.** Every commit on it since the
  campaign closed is a mechanical sweep (C4 typed label keys, C2 `GridZ`, C10
  `cargo fmt`, A/M6, M3).

**What must survive the archive** — this is the part worth the split:

| keep | where | why |
|---|---|---|
| `ensure_scaled_stl` (`:126–213`) | `common/` or the archive | the ×2 scaling procedure; reproducible |
| `write_fixture_project` (`:214–721`), incl. the 449-line TOML literal | the archive, intact | **the only committed, immutable record of the scaled-wanaka fixture.** It is exactly the pattern p2c should have used. Do not delete it to save lines. |
| `Dials` (`:1688–1764`) | the archive | records what every measurement before 2026-07-27 ran with |

Everything shared with p2c (§4) moves to `common/` once, from whichever file is
processed first.

---

## 4. The duplication, measured

`v3` ↔ `p2c`: **282 byte-identical lines** in blocks ≥ 12, and roughly
**550–650 lines** counting drifted twins. v3's own comment at `:1073–1082`
admits the copy and names the only two intended deltas (output dir;
`for_tool(0.5)` vs `for_tool(3.0)`), citing *"the design prompt's explicit
instruction not to refactor that harness"*. That instruction has expired with
the campaign.

Largest verbatim blocks: `band_shares` (40 lines, v3 `:1452–1491` / p2c
`:3659–3698`), `build_band_map` (46, v3 `:1162–1207` / p2c `:314–359`),
the fixpoint ladder (41, v3 `:799–839` / p2c `:107–147`), `outer_region_spans`
(22, verbatim). The `assert!(pending.len() < before, "ladder stalled")` idiom
appears **7 times** — once in `run_chain` and inline in six p2c tests
(`:1279, 1384, 1712, 2205, 2427, 3394`).

`h4` ↔ `classification_columns_ab_m3.rs`: **138 byte-identical lines**,
including `render()` (36 lines — h4's doc at `:885` says *"Copied from
classification_columns_ab_m3.rs::render"*) and the `terrain()` crop (27 lines,
`:281–283` says *"copied verbatim"*). h4's `quantile` (`:542–549`) is
byte-identical to `common::scallop_oracle::quantile` — **and h4 already has
`mod common;`** at `:185` and simply does not import it.

`h4` ↔ `v3` and `h4` ↔ `p2c`: **zero** identical blocks ≥ 10 lines. Different
instrument lineage — h4 uses `SimulationResult::column_deviations` (the
pointwise COLUMNS instrument), v3/p2c use stock-mesh-vertex deviation plus
`BandMap`. **The policy must respect that split**: do not merge the two
instrument families into one `common/` module. They measure different things,
and the v3 campaign's central lesson was that conflating instruments is how a
gate ranks a 28 mm uncut block first.

---

## 5. `strategy_comparison_h4.rs` — the healthy one

**Verdict: SCHEDULED. Owner: the finishing/quality lane. Cadence: on demand
before any strategy question, plus its CI sentry on every run.**

It is the successor instrument and it is already built the way the others
should have been:

- **Committed fixtures only** — `tests/fixtures/terrain.stl` (`:265`, asserted
  to exist at `:266–271`, and asserted to keep > 500 triangles after cropping
  at `:305`) and the fully synthetic `common::meshes::grooved_block` (`:347`).
- **It refuses the mutable file, in writing.** Doc `:22–27`: *"that file is a
  live, user-modified project; reading it as a dependency makes a gate whose
  verdict changes when somebody drags a slider."*
- **It does not assert a winner.** `:101–104`: *"This harness does not assert a
  winner."* Its result tables (`:130–146`) are printed narrative; nothing
  asserts them. Its three gates (`:1023` collisions = 0, `:1033` cascade
  non-blind ≤ 0.90, `:1049` ≥ 2 band/strategy combos) are pre-registered and
  documented, and its two on-size bins are `#[allow(dead_code)]` — reported,
  not gated. That is exactly the discipline the ±10 µm bin lacked.
- **It ships a cheap non-ignored sentry** (`:1322`, ~100 lines on a
  `tiny_groove` fixture) so the arithmetic stays live even though the arms do
  not run.

**Its debt, and the whole of it:** import `common::scallop_oracle::quantile`
instead of the local copy; adopt a parameterised `common::out_dir(name)` in
place of the fifth copy of that pattern; and extract the 138 lines it shares
with `classification_columns_ab_m3.rs` into `common/columns.rs` (which
`checkpoint_b_resolution_ab.rs` also wants — it carries its own `quantile`
too). Low risk, and the C6 migration policy applies: **opportunistic, never in
bulk**, assertions and pinned constants through untouched.

**Caveat on scheduling.** h4 is 2 days old and its own module doc carries
result tables dated 2026-08-04. Those are prose in a test file and will rot the
same way if nobody re-runs them. The cadence below exists to stop that.

---

## 6. The four rules this makes standing

1. **A harness may not read a mutable file outside `tests/`.** If a campaign
   needs a real project, it snapshots it into a committed literal — the pattern
   `v3_cascade_ab.rs:256–706` already demonstrates. `wanaka.toml` is the
   counter-example and it cost this programme a harness.
2. **No absolute path outside the repo.** `v3_cascade_ab.rs:54` made a 5066-line
   harness un-runnable by anyone, and nothing detected it because it still
   compiles. A `tests/common` helper that resolves fixtures should be the only
   way a path is built.
3. **Dated result prose does not live in a test file.** It rots invisibly —
   `p2c:3712`'s "KNOWN RED as of 2026-07-13" and `v3:3763–3775`'s retraction are
   both status notes stranded in `#[ignore]`d code. Narrative goes to
   `planning/`; the test keeps the mechanism and the pre-registered bar.
4. **No fourth unowned mega-harness.** A new campaign extends h4 or writes a
   T3-tier harness with a named owner and cadence at birth (see
   `REFERENCE_FIXTURE_SPEC.md` §8). The T1/T2 tiers exist so that most
   questions never need a mega-harness at all.

---

## 7. Decisions requested at Checkpoint E

W7 proposes; it does not execute. None of this is done.

- **E5.** Approve SPLIT-then-ARCHIVE for `p2c_headless_ab_wanaka.rs` and
  `v3_cascade_ab.rs`, extracting `common/bandmap.rs` + `common/chain.rs` first,
  preserving v3's fixture TOML literal intact, and moving the dated narrative to
  `planning/review_2026-07-29/`?
- **E6.** Approve SCHEDULED for `strategy_comparison_h4.rs` with the finishing
  lane as owner, its non-ignored sentry preserved, and its 138 shared lines
  extracted to `common/columns.rs` opportunistically?
- **E7.** Adopt §6's four rules as standing programme rules?
- **E8.** Note the three premise corrections in §1 (v3 does not read
  `wanaka.toml`; v3's blocker is a hard-coded user-local path; h4 has a
  non-ignored CI sentry) so no later wave acts on the original description.
