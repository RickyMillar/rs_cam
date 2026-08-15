# A2D-165 — the 165 unrun adversarial 2D matrix cells, run

Date: 2026-08-16
Wave: TD3 **S-3** (sweep pool), ledger row **A2D-165**
(`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4)
Revision measured: branch `tech-debt-3` @ `9c2eb01e` + this wave's
harness-only commit, **debug** build, one process per batch
Status: **RESEARCH ONLY.** No production code changed in this wave. Every
red below is a defect candidate with a proposed row id, not a fix.

Companion documents: `planning/review_2026-08-04/ADVERSARIAL_2D_FINDINGS.md`
(W4's findings F-1…F-13 and its partial matrix),
`ADVERSARIAL_2D_FIXTURE_SPEC.md` (what the fixtures are).

---

## 0. The count, checked rather than inherited

The ledger has been wrong about counts twice this programme, so the 165 is
derived here from the registry rather than quoted.

| quantity | value | how it is obtained |
|---|---|---|
| fixtures | **22** | `adversarial2d::fixtures()`, asserted `>= 20` by the non-vacuity test |
| families | **9** | `op_matrix()`'s eight + `rest` (which needs a second tool) |
| matrix cells | **198** | 22 × 9 |
| cells W4 completed | **33** | `ADVERSARIAL_2D_FINDINGS.md` §1.2 — reflex-cross, comb-16, dendrite complete (27) + rosette-24's first six (6) |
| cells W4 attempted and stopped | **1** | `inlay × rosette-24`, recorded `> 129 s, CEILING EXCEEDED, run stopped` — no data collected |
| **cells never run** | **165** | 198 − 33. Composition: fixtures 5…22 entire (18 × 9 = **162**) + `rosette-24 × {inlay, drill, rest}` (**3**) |

**The ledger's 165 is correct**, and the composition above is what was
actually run. Of those 165, **2** are `Fixture::skip_ops` cells declared
non-terminating and are reported NOT EXERCISED with the declaration quoted
(§4); **163** executed.

Every cell was then also re-run in a single **full 198-cell pass**, which is
what §2's drift check and §3's probe counts are taken from.

---

## 1. Method, and the two harness knobs this wave added

The campaign is one `#[ignore]`d test that runs the whole matrix and writes
its table after the last cell. That shape is why W4 lost its evidence twice:
a cell that does not return, or a process killed from outside, leaves
nothing. Two additions, both **test-only**, both no-ops when unset:

| knob | what it does | why |
|---|---|---|
| `R2_ONLY_FIXTURES` / `R2_ONLY_OPS` | comma-separated allow-lists | makes the matrix divisible, so a non-terminating cell costs one batch and not the campaign |
| `R2_ROW_LOG` | appends and **flushes** each row as it is measured | a killed batch keeps every row it had already taken |

No assertion, ceiling, expectation or fixture changed. Unset, the campaign
runs exactly the matrix it always ran.

Execution discipline: the test **binary** was built once and then invoked
directly, so the campaign never held the Cargo slot while running (S-1 needed
it concurrently). Each batch ran under `ulimit -v 16777216` — 16 GiB address
space, below the 22.9 GB that F-10's first run reached — and its own
`timeout`. Machine floor respected throughout: 96 GB free disk, 37 GB
available RAM, no release build.

**Total cost: 165 cells in 112 s of wall clock across seven invocations (one
trial fixture plus six batches); the full 198-cell pass in a single process
in 120 s.** The `#[ignore]` reason still says "minutes in
debug", which is now the only honest part of it — the campaign is not
expensive, and nothing about its cost justified leaving 165 cells unrun.

### 1.1 Two blind spots found in the instrument, and closed before judging

A first pass reported **165 of 165 cells `ok`**. That verdict was not
trustworthy, for a reason visible in its own stderr: `inlay × rosette-24`
printed *four* `cavalier_contours` panics and still recorded a clean `ok`.
The campaign's three failure conditions (panic escaping, silent empty on
`MustCut`, ceiling breach) cannot see:

1. **a contained library panic** — `polygon.rs:495`'s `catch_unwind` turns it
   into an empty offset, which every caller reads as a collapse;
2. **a non-finite coordinate in the emitted toolpath** — only `drill ×
   invalid-nan`'s `cut_length_mm` column printing `NaN` gave that one away,
   and a NaN in Z or in a rapid would have printed nothing at all.

Both were closed by adding a second, non-asserting **probe row** per cell
that reads `ToolpathStats::offset_library_failures`,
`ToolpathStats::boundary_clip_dropped` and a direct non-finite scan of the
emitted moves. The numbers in §3 come from that probe. Turning either into a
failure condition is a Checkpoint decision and is **not** taken here.

---

## 2. The matrix — 198 of 198 cells

Full per-cell records: `artifacts/s3/rows_full.md` (198 matrix rows + 196
probe rows), `artifacts/s3/rows_165.md` (the batched first pass over the 165),
batch logs in `artifacts/s3/logs/`.

| outcome | cells |
|---|---|
| `ok` | **196** |
| `SKIPPED` (declared non-terminating, §4) | **2** |
| typed `Err` | 0 |
| **panic escaping to the harness** | **0** |
| **silent empty on a `MustCut` fixture** | **0** |
| **wall-clock ceiling breach** (60 s) | **0** |

Slowest cell in the whole matrix: `inlay × holed-9` at **17.9 s**, 3.4× under
the ceiling. Nine of the ten slowest are `inlay` or `vcarve`, which is the
same shape W4 reported.

### 2.1 Per-family verdicts over the 165

`clean` = `ok`, no violation, nothing flagged by the probes.
`red` = carries evidence for a finding in §3.
`not exercised` = declared non-terminating.

| family | cells | clean | red | not exercised | max wall | reds |
|---|---|---|---|---|---|---|
| pocket | 18 | 16 | 1 | 1 | 0.156 s | A2D-F2 |
| adaptive | 18 | 17 | 1 | 0 | 9.399 s | A2D-F1 |
| profile | 18 | 17 | 1 | 0 | 0.117 s | A2D-F2 |
| trace | 18 | 16 | 2 | 0 | 0.029 s | A2D-F4 ×2 |
| zigzag | 18 | 18 | 0 | 0 | 0.302 s | — |
| vcarve | 18 | 17 | 1 | 0 | 15.795 s | A2D-F1 |
| inlay | 19 | 13 | 5 | 1 | 17.890 s | A2D-F1 ×3, A2D-F4 ×2 |
| rest | 19 | 18 | 1 | 0 | 1.796 s | A2D-F1 |
| drill | 19 | 18 | 1 | 0 | 0.001 s | A2D-F3 (+ A2D-F5, population) |
| **total** | **165** | **150** | **13** | **2** | — | |

**No family is declared clean on hostile geometry by this table alone.**
`zigzag` is the only one with no red cell, and even it inherits A2D-F2's
question (§3.2) on `zigzag × invalid-nan`.

### 2.2 Drift check — W4's 33 cells, re-measured

All 33 of W4's completed rows were re-run at this revision. **Every one
reproduces its emitted geometry exactly** — moves, cutting moves, cut length
to 0.1 mm, and cut runs are identical in all 33 rows across a branch change
(`experiment/adaptive-spiral @ 0e7d38b` → `tech-debt-3 @ 9c2eb01e`). Only
wall clock and RSS high-water differ, both machine-state figures.

**W4's single ceiling breach does not reproduce.** `inlay × rosette-24`,
recorded as `> 129 s, CEILING EXCEEDED, run stopped`, completes in
**6.977 s** — 8.6× under the 60 s ceiling — emitting 181 559 moves. Its four
contained library panics (§3.1) are the same ones W4 was watching when it
stopped the run. Two candidate explanations, **not** discriminated here: the
TD1+TD2 merge is not the revision W4 measured, and W4's own §1.1 records that
the machine's root filesystem was at 100% during that window. Recorded as
"the prior red does not reproduce at this revision", not as a fix.

---

## 3. Findings — thirteen red cells, five findings

Each is a **defect candidate** with the invariant it bears on quoted. None is
fixed here. Row ids are proposed for the ledger.

### 3.1 **A2D-F1** — a contained library panic is invisible on five of nine families

**6 red cells, 13 of the 19 contained panics measured in this campaign.**

| cell | contained panics | site | `offset_library_failures` reported |
|---|---|---|---|
| `inlay × walls-1e-2` | 4 | `pline_seg.rs:33` *"v1 must not be on top of v2"* | **not measured** |
| `inlay × rosette-24` | 4 | `pline_seg.rs:33` | **not measured** |
| `inlay × invalid-nan` | 3 | `static_aabb2d_index.rs:266` | **not measured** |
| `rest × invalid-nan` | 2 | `static_aabb2d_index.rs:266` | **not measured** |
| `adaptive × invalid-nan` | 2 | `static_aabb2d_index.rs:266` | **not measured** |
| `vcarve × invalid-nan` | 1 | `static_aabb2d_index.rs:266` | **not measured** |

Checkpoint C shipped the typed channel for exactly this, and its own
docstring names the motivating cost (`compute/config.rs:405-411`):

> *"Why it needs a channel at all: before this, a contained panic and a
> geometric collapse were the same `Vec::new()` and the only trace was a
> `tracing::warn!` in a process that usually installs no subscriber. The
> measured cost of that is **F-12** — an **inlay's** female pocket whose ring
> cascade stopped early on a `debug_assert!` in a transitive dependency, left
> material standing, and reported a successful generate."*

**`inlay` is one of the five families that never writes the slot.** Measured
over the full 198-cell pass: `offset_library_failures` is `None` — *not
measured*, per the slot's own contract — on **109 of 196** run cells
(adaptive, drill, inlay, rest, vcarve: 22, 22, 21, 22, 22). Only pocket,
profile, trace and zigzag opt in, which matches Checkpoint C's recorded
"four families opted in" — the gap is that the family the finding was written
about is not among the four.

Two things this campaign adds beyond restating F-12:

- **The panic site is reached from a second, fully VALID fixture.**
  `inlay × walls-1e-2` (near-coincident walls at 10⁻² mm, `Validity::Valid`,
  `Expectation::MustCut`) drives `pline_seg.rs:33` four times. W4 reached it
  only on `rosette-24`. F-12's reachability argument — *"no contract
  violation is needed to reach it"* — now has two independent witnesses.
- **All 19 contained panics are in the previously-unrun 165.** W4's 33
  completed cells contained **zero**. The unrun portion of the matrix was
  where the entire signal was.

Proposed row: **A2D-F1** — extend `record_offset_library_failures` to the
five families that do not report it, `inlay` first. Report-only channel; no
gate consumes it.

### 3.2 **A2D-F2** — the slot counts any offset failure under a name that says library

**2 red cells**, plus a correctness caveat on 3 more.

`pocket × invalid-two-vertex` and `profile × invalid-two-vertex` both report
`offset_library_failures 1` — with **zero** panics anywhere in the batch log
for either cell. The cause is in the counting expression, which is the same
in all four opted-in families:

```
crates/rs_cam_core/src/zigzag.rs:91     let failures = usize::from(failure.is_some());
crates/rs_cam_core/src/profile.rs:118   (contour, usize::from(failure.is_some()))
crates/rs_cam_core/src/trace.rs:74      let failures = usize::from(failure.is_some());
crates/rs_cam_core/src/pocket.rs:227    offset_failures: usize::from(compensation_failure.is_some()),
```

`failure` is `Option<OffsetFailure>`, and Checkpoint C's whole shipped shape
is that `OffsetFailure` distinguishes **`Collapsed` / `RejectedInput` /
`LibraryFailure`** (`polygon.rs:455`, `offset_polygon_reported`). A
two-vertex contour is a `RejectedInput` — cavalier's `< 3`-vertex guard, not
a library failure — and it increments a counter named
`offset_library_failures`. `OffsetFailure::is_library_failure` exists
(`polygon.rs:377`) and has exactly one consumer, in `session/compute.rs:2044`;
none of the four counters uses it.

Two consequences, both instrument-integrity (§0 rule 5 — *a changed
instrument makes its own docstring a lie you then cite*):

- the count **over-reports**: a collapse or a rejected input reads as a
  library failure;
- it is a **boolean per offset call**, not a count of failures:
  `OffsetFailure::merge` (`polygon.rs:406`) folds every failure of one call
  into one value, preferring the library variant. A call that panicked three
  times reports 1.

The three cells that report `1` *correctly* — `pocket`, `profile` and
`zigzag` × `invalid-nan`, each with exactly one panic on stderr — are right
by coincidence of arithmetic, not by construction.

Proposed row: **A2D-F2** — count with `is_library_failure()`, or rename the
slot to what it measures. Note this interacts with A2D-F1: fixing the
coverage gap without fixing the predicate propagates the wrong measure to
five more families.

### 3.3 **A2D-F3** — `drill × invalid-nan` emits six non-finite moves and reports success

**1 red cell.** Probe: `nan_moves 6` of 6 emitted moves. Matrix row:

```
| drill | invalid-nan | 0.000 s | ok | 6 | 2 | NaN | 2 | 0 kB |
```

This is W4's **F-13** (*"Drill emits a NaN hole position and reports
success"*, MEDIUM), now measured in the matrix and **sharper than the finding
states**: it is not one NaN hole position among finite moves — **every
emitted move is non-finite**, in X, Y or Z. The toolpath IR is the declared
boundary between planning and post-processing, so this is a NaN crossing that
boundary with an `Ok` beside it. Nothing in the campaign's failure conditions
looks at coordinates; `cut_length_mm` printed `NaN` only because the sum
happened to run through the bad move.

Seven of the nine families trip a contained panic on `invalid-nan`. The two
that do not are `trace` (§3.4) and `drill` (this row) — and both of them
emit. That is not robustness: they are the two that never reach the offset,
so they carry the NaN straight through instead of collapsing on it.

Proposed row: **A2D-F3** — a non-finite filter at import or at emission, and
a `nan_moves`-style assertion in the campaign. Cross-reference F-13; F-11
(nothing filters non-finite coordinates anywhere) is the upstream row.

### 3.4 **A2D-F4** — a contour with no interior produces cutting moves

**4 red cells.** All on `MayBeEmpty` fixtures, so the campaign excuses them —
and the excuse is the wrong shape. `Expectation::MayBeEmpty` licenses an
**empty** result on degenerate input. It says nothing about a **non-empty**
one, and there is no expectation value that means *"must not invent cuts"*.

| cell | moves | cutting | cut mm | the input |
|---|---|---|---|---|
| `inlay × invalid-zero-area` | 1228 | 1225 | 99.2 | a polygon of zero area |
| `inlay × invalid-two-vertex` | 1228 | 1225 | 99.2 | a two-vertex contour |
| `trace × invalid-zero-area` | 20 | 16 | 129.4 | a polygon of zero area |
| `trace × invalid-nan` | 9 | 5 | 9.4 | a contour with a NaN vertex |

The two `inlay` figures are **identical to the digit** on two different
degenerate inputs, which says the emission is a function of the tool and the
depth rather than of the geometry. The campaign's own charter names the
direction that matters here: the boundary layer is *"the one place an empty
offset is an **over**-cut — everywhere else it is an under-cut"*. Cuts
emitted from a shape with no interior are in the over-cut direction too.

Severity is bounded by reachability — these are `Validity::Invalid` fixtures
and both importers run `ensure_winding`/validity passes — but no importer
filter was verified for zero-area or two-vertex rings in this wave, so
reachability is **not assessed**, not "low".

Proposed row: **A2D-F4** — decide whether a third expectation
(`MustNotCut`) belongs in the fixture contract, and check whether the
importers can deliver a zero-area ring.

### 3.5 **A2D-F5** — drill's cells ran, but the hostile geometry never reaches it

**Not a defect; a population statement**, in the shape §0 rule 4 asks for.

`drill` emits **exactly 6 moves / 2 cutting / 0.0 mm on 17 of its 19 cells**
in the 165 — every fixture with one top-level component gives one hole,
because the operation takes model centroids. Only `tiny-islands` (48 moves,
16 holes) and `holes-in-holes` (30 moves, 10 holes) differ. The cells are
green, and they are green about almost nothing: a reflex cross, a 10⁻⁶ mm
wall gap and a 24-lobe rosette all produce the same six-move Z plunge.

So **"drill is clean on hostile 2D geometry" is not a claim this campaign can
support**, and the row count must not be read as if it were. The gate had a
population; the population had no hostility in it.

The same caution, weaker, applies to `rest`: it is exempt from the
silent-empty gate by design (`rest.rs:70` returns empty by contract when
`tool_radius >= prev_tool_radius`), so 4 of its 19 cells recorded zero
cutting moves with the gate unable to object.

### 3.6 Gate population, stated

Of the 165 cells, the silent-empty condition — the campaign's only
correctness gate other than "did not panic" and "returned in time" — could
fire on **90**:

| population | cells | why |
|---|---|---|
| `MustCut` fixtures (5…15 + rosette-24) | 102 | the gate has teeth |
| … minus `rest`, exempt by contract | −12 | recorded, never failed |
| **gate could fire** | **90** | |
| `MayBeEmpty` fixtures (16…22), run | 61 | gate **vacuous by construction** |
| declared non-terminating | 2 | not run |

21 of the 61 vacuous cells did return zero cutting moves. Every one is on an
`invalid-*` fixture, so none of them is a defect on this contract — but none
of them is evidence of health either.

---

## 4. NOT EXERCISED, with the blocker named

| cell | why | evidence it is not a silent skip |
|---|---|---|
| `pocket × invalid-cw` | `Fixture::skip_ops`: *"DOES NOT TERMINATE — pocket's ring cascade exits only on collapse, and a CW exterior makes every offset GROW … Running it here would hang the suite, not test it (22.9 GB RSS on the first attempt)"* | the divergence is measured instead by `the_pocket_ring_cascade_is_bounded_only_by_collapse`, a bounded probe: 40 rings, area growing, CCW control collapsing at ring 13 |
| `inlay × invalid-cw` | `Fixture::skip_ops`: *"same cascade, reached through inlay's female pocket (inlay.rs:108)"* | same probe |

Both print a `SKIPPED` row carrying their reason into the matrix file. The
other seven families **do** run against `invalid-cw` and complete: `vcarve`
emits 31 302 cutting moves in 0.353 s, `adaptive` 270, `rest` zero. The skip
list is therefore exactly as narrow as it claims to be — it names two cells,
not a fixture.

Nothing else in the 165 was skipped, and no cell was abandoned.

---

## 5. Stale expectations — none found, and why that is a real answer

Task framing anticipated cells whose expectations encode a pre-Checkpoint-J/K
feeds number, which would be a **stale-expectation** finding rather than a
behaviour defect. **There are none, structurally**: this campaign asserts on
three things only — a panic escaping, an `Ok` with zero cutting moves on a
`MustCut` fixture, and a wall-clock ceiling. It reads no feed, no speed, no
chipload, no recipe number, and constructs its tools with
`ToolConfig::new_default` rather than from the LUT. Checkpoint J and K moved
feeds numbers; **no cell in this matrix can observe that**, so nothing here
went stale when they landed.

The one expectation-shaped question the run did raise is A2D-F4's: the
fixture contract has no way to say *"must not invent cuts from nothing"*.
That is a **missing** expectation, not a stale one.

---

## 6. What this wave did not do

- **No production code changed.** Both harness knobs and the probe row are
  test-only, and the probe asserts nothing.
- **No red was fixed**, and none was promoted past "defect candidate".
- **Release behaviour is NOT EXERCISED.** Every panic in §3.1 is a
  `debug_assert!` in a dependency; in release those checks are skipped and
  the library proceeds on unvalidated input. `polygon.rs:48` says so, and it
  makes `offset_library_failures` non-comparable across builds. This wave
  measured debug only.
- **Reachability of A2D-F3 and A2D-F4 through the importers was not
  measured**, only noted as unassessed.
- **The renders were written, not read, and not committed.** The full pass
  wrote **175 toolpath SVGs, 285 MB**, to its `R2_ARTIFACT_DIR`; that is too
  large to commit and no claim in this document rests on a visual, so none is
  made (§0 rule 3 cuts the other way here — the honest move is to make no
  visual claim, not to ship 285 MB). Regenerate with
  `R2_ARTIFACT_DIR=<dir> <binary> --ignored --nocapture --exact
  adversarial_2d_full_campaign`.
