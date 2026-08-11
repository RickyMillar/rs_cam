# S-4 (G-BYTE) — the frozen-snapshot A/B

Date: 2026-08-12
Wave: TD3 sweep-pool **S-4**, ledger row **G-BYTE**
(`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4.2)
Branch: `tech-debt-3`, base `675a643`
Harness: `crates/rs_cam_core/tests/frozen_snapshot_regeneration_s4.rs`
Artifacts: `planning/review_2026-08-08/artifacts/s4/`
Commit: `85f40a1`

---

## 0. Verdict

**IDENTICAL BYTES.** Against one frozen machined-stock snapshot, two
generations of the same operation at identical parameters produce
byte-identical output — move list, spans, and emitted G-code alike.

G-BYTE's 135 hunks were therefore **snapshot drift, not generator
nondeterminism**. The plan's identical-branch fired: the fix shipped is
provenance (`ToolpathStats::stock_snapshot`), not a determinism repair.

**A-5's before/after fixtures are not blocked.** The byte-stability that
underpins a before/after capture holds. The condition on trusting such a
capture is now checkable rather than assumed: both arms must carry the same
`stock_snapshot` stamp.

---

## 1. Setup

### 1.1 The fixture

A two-op cascade in the incident's shape — a rest-fed `UnifiedFinish`
consuming `StockSource::FromRemainingStock`:

| | op 0 | op 1 |
|---|---|---|
| operation | `UnifiedFinish` | `UnifiedFinish` |
| role | all-over finish | rest pass |
| `raster_stepover` | 1.5 mm | 0.5 mm |
| `pencil_claims` / `territory_clip` | off | on (off in arm B4) |
| `stock_source` | `Fresh` | **`FromRemainingStock`** |

Ø3 ball nose; 40 × 40 mm smooth double-bump height field occupying the top
2 mm of a 6 mm stock; heights pinned `top_z = 6.0`, `bottom_z = 4.0`. Same
surface and frame the A/M6 cascade and the A4 zero-removal sentries use.

Op 0's stepover is 3× the rest pass's on purpose: it leaves ~0.19 mm cusp
ridges, so the rest pass has real material in front of it. **Non-vacuity is
asserted, not assumed** — the arm refuses to run on fewer than 500 moves or
500 G-code lines. Measured: 6 519 moves / 7 208 lines / 187 557 B.

The operator's `wanaka.toml` was **not touched, read, or copied**.

### 1.2 Why this fixture and not the live file

`prior_stocks` is the only route by which a machined-stock snapshot reaches
generation (`session/compute.rs:1389-1396`), and it is keyed by toolpath id
regardless of project. A synthetic cascade exercises the identical code path
at 1.3 s instead of 40 min, and can be frozen exactly.

## 2. Comparison method

### 2.1 Freezing the snapshot

`ProjectSession::generate_toolpath` writes only `self.results`; it never
touches `self.simulation`. So after one `run_simulation`, calling
`generate_toolpath(1, …)` twice in a row hands the generator the **same
`Arc<TriDexelStock>`** both times, with nothing in between — no simulation,
no mutation, no parameter touch. That is the freeze.

### 2.2 What is compared

Three independent renderings, all byte-level:

1. **Move list** — FNV-1a over its `Debug`. `Debug` on `f64` round-trips
   every bit, so this is byte identity, not a lossy summary
   (`tests/common/fingerprint.rs`).
2. **Whole `AnnotatedToolpath`** — moves *plus* spans, `spans_valid`,
   planner engagement, rest grid and rest regions.
3. **Emitted G-code** — `gcode::export_gcode_checked` with `sim_trace: None`
   and `{accept_unmodeled: true, accept_exceeded: true}`, so the text is a
   pure function of the toolpaths and **cannot move because a load verdict
   moved**. Compared with `assert_eq!` on the whole `String`, and dumped to
   `target/gbyte_s4/*.nc` for real `diff`.

Hunk counts in this report come from GNU `diff -U0` on those dumps — the
same tool the incident's 135 hunks were counted with. The in-test line
comparison is positional, not an LCS; it is exact when line counts match
(the arm-A case that carries the verdict) and **suppresses its own
classification when they do not**, rather than reporting a misalignment as a
finding.

### 2.3 The independent witness

Every arm also computes a snapshot identity from `prior_stocks` **before**
generation and outside the production code: a 64×64 lattice of
`max_conservative_top_z_in_disc` plus the grid parameters. The production
stamp is asserted to agree with it, so the stamp is never checked only
against itself.

## 3. Result

### 3.1 The arms

| arm | between the two generations | snapshot | bytes | `diff` hunks |
|---|---|---|---|---|
| **A — frozen** | nothing | SAME | **IDENTICAL** 187 557 B / 7 208 lines | **0** |
| **B1 — re-sim, same cell** | `run_simulation` @ 0.25 mm | SAME | **IDENTICAL** | **0** |
| **B2 — re-sim, coarse cell** | `run_simulation` @ 0.40 mm | MOVED | 187 557 → 274 187 B | 5 |
| **B3 — re-sim, near cell** | `run_simulation` @ 0.30 mm | MOVED | 187 557 → 184 935 B | 6 |
| **B4-frozen — no territory clip** | nothing | SAME | **IDENTICAL** 180 147 B / 6 932 lines | **0** |
| **B4-drifted — no territory clip** | `run_simulation` @ 0.30 mm | MOVED | **IDENTICAL** | **0** |

Move-list fingerprints (FNV-1a over `Debug`):

| arm | gen 1 | gen 2 |
|---|---|---|
| A | `6519 / 04e0a63a8812b572` | `6519 / 04e0a63a8812b572` |
| B1 | `6519 / 04e0a63a8812b572` | `6519 / 04e0a63a8812b572` |
| B2 | `6519 / 04e0a63a8812b572` | `9824 / 6390028df9a96872` |
| B3 | `6519 / 04e0a63a8812b572` | `6414 / 897dfcade86cc928` |
| B4 | `6243 / e1be493d0d9e90b3` | `6243 / e1be493d0d9e90b3` |

### 3.2 The strongest single piece of evidence

`artifacts/s4/SHA256SUMS.txt`. **One** hash,
`ab49460186ba937dc70170eb003a98d6a0a57817792282ea93739279ce3bf4ba`, covers
six files: arm A gen1 **and** gen2, arm B1 gen1 **and** gen2, and the gen1 of
arms B2 and B3. That is the same configuration generated **six times across
four independent test functions**, landing on one byte sequence every time.
A second hash, `cb76d4d9…`, covers all four arm-B4 files identically.

Byte-identity here is not "the two calls in one loop agreed" — it survives
separate sessions, separate simulations, and separate processes.

### 3.3 Byte counts against the incident

| | incident (W10-LV) | this A/B, arm A |
|---|---|---|
| hunks | 135 | **0** |
| `G0` approach heights moved | ~140, 0.05–0.15 mm | **0** |
| cutting lines dropped | 12 of 180 118 (0.007 %) | **0 of 7 208** |
| cutting Zs | identical where present | identical (whole file identical) |

### 3.4 What the control arms establish

- **Re-simulating is not by itself drift** (B1). The second simulation
  allocated a fresh `Arc` holding bit-identical material and the output did
  not move. Drift requires the simulation to actually *differ*. This
  narrows the incident's suspected mechanism: two sim *events* are not
  enough, the two events must have differed.
- **A differently-resolved snapshot is a sufficient cause of a byte
  difference** (B2, B3). At 0.25 → 0.30 mm: 6 hunks, 161 lines, and 105
  fewer moves. So the harness *can* move the bytes; arm A's equality is a
  property of the generator, not of the harness.

## 4. What is NOT exercised

**The incident's own dominant class — `G0` approach heights.** Arm B4 was
written to isolate it: with `territory_clip` off, the only remaining
snapshot consumers are the air-cut filter and
`dressup::optimize_entry_descents_with_provenance`, whose split-rapid Z is
`max_conservative_top_z_in_disc(x, y, r) + PLUNGE_CLEARANCE_MM`.

It did not reproduce. **Blocker, named:** this fixture emits only **7** `G0`
lines and every split lands at `Z8.000` — that is `6.0` (the raw stock top)
`+ 2.0`. `max_conservative_top_z_in_disc` is a *sliver-safe upper bound*
(A/M10), and over a 1.5 mm-radius disc on this surface it saturates at the
unmachined stock top, so refining or coarsening the cell cannot move it.
What B2/B3 move instead is the rest **territory** (B3's diff: 301 changed
`G1` lines against 19 `G0`).

Consequence for the report's claims: the mechanism named in the G-BYTE
ledger row for the `G0` class remains **inferred from the code path, not
measured**. Exercising it needs a fixture whose entry ceilings sit below the
raw stock top, which is out of S-4's scope. The verdict does not depend on
it — arm A rules out nondeterminism for the *whole* output, `G0` lines
included.

Also not exercised: the GUI worker's stamp call site
(`rs_cam_viz/src/compute/worker/execute/mod.rs`) compiles and mirrors the
session path, but no live GUI run was made in this wave. **Blocker:** S-4 is
a core-side wave and a live MCP session was not in its scope.

## 5. The fix that landed

`ToolpathStats::stock_snapshot: Option<StockSnapshotStamp>`.

### 5.1 Contract family

The **two-valued event family** — `zero_removal` and
`boundary_clip_dropped` — **not** the three-valued A/M9 / X-19 family.
Stated at the field, with the reason: there is no "measured zero" for an
identity, so the three-valued split would buy a distinction with no
consumer. `None` means **no machined-stock snapshot was consumed** (a
`Fresh` op, or a `ToolpathStats::default()` placeholder). Report-only: no
gate consumes it, nothing branches on it, generation is byte-identical
whether or not it is populated.

### 5.2 What the stamp carries

Cell size (mm), Z-grid rows/cols, and an FNV-1a digest over the stock bbox,
every grid's parameters, and every ray's material segments on all three
axes. Cost is one pass over data the simulation just wrote: sub-millisecond
on the S-4 fixture (177×177 @ 0.25 mm) against a ~1.3 s generation.

**Content-derived, not identity-derived — and arm B1 is why.** An honest
re-simulation allocates a *new* `Arc` holding the *same* material. A pointer
stamp or an incrementing sim-event counter would call that a difference and
raise a false alarm on every re-sim. The digest reads it as the same
snapshot, which is the truth.

### 5.3 Why it is a parameter, not a finding

The snapshot's identity is known only to the **caller** — the session or the
worker looked it up in `prior_stocks` before handing it to the generator —
so it cannot arrive through `GenerationFindings`, which is written inside
`execute`. Threading it as `stats_with_findings`' fourth parameter preserved
H2.1's property: **both production call sites broke at compile time**, as did
the narration guard and the h21 join sentry. All four were routed with a
stated decision; none elided. A `stats.stock_snapshot = …;` line after the
join is exactly the pattern `stats_with_findings`' own doc documents as
unsafe.

The session stamps `prior_stock_arc`, **not** the source-gated
`gen_initial_stock`: the same `Arc` also reaches the dressup air-cut filter
ungated, so it is the snapshot the generation consumed in the broadest true
sense.

### 5.4 The sentry

The A/B harness itself, as the plan specified. Six tests:

| test | asserts |
|---|---|
| `arm_a_…_byte_identical` | two gens / one snapshot ⇒ byte-equal **and** same stamp; stamp carries the right cell |
| `arm_b1_…_same_cell` | fresh `Arc`, identical material ⇒ **same** stamp |
| `arm_b2_…_moves_the_bytes` | different snapshot ⇒ **different** stamp, naming both cells |
| `arm_b3_…_class_of_hunks` | small delta: characterisation, stamp/witness agreement |
| `arm_b4_…_does_not_move_on_this_fixture` | frozen byte-equality on a 2nd configuration; the NOT-EXERCISED finding in its own doc |
| `a_fresh_stock_generation_records_no_snapshot_stamp` | `Fresh` op ⇒ `None`, not a stamp of the raw block |

Every arm additionally asserts the production stamp against the independent
witness of §2.3.

**Red-first, honestly stated:** the failing-before-fix property here is
compile-time, not runtime — before the field existed, `result.stats.
stock_snapshot` did not compile, and the three `stats_with_findings` guards
plus the narration guard all fired on the change (recorded in the commit
message). There was no pre-existing runtime red to quote, because the defect
was a *missing channel*, not wrong behaviour.

The sharpest sentry case is **B4-drifted**: output byte-identical, stamp
MOVED. That is the correct direction for a provenance channel — it never
claims a comparability that does not exist.

## 6. Fingerprint statement

**No fingerprint moved. Verified, not asserted.**

`ToolpathStats` has zero coupling to `fingerprint.rs`
(`ToolpathFingerprint::from_toolpath` takes `&Toolpath`; grep for
`ToolpathStats|stats` in that file returns nothing), and the change adds no
mutation of any toolpath or stock.

Sentries run green after the change: `finish_resolution_policy_pr3` (three
pinned `(moves, hash)` constants), `checkpoint_b_resolution_ab`,
`crease_own_region_pr6b`, `common_fixtures_smoke_c6`,
`findings_transport_join_h21`, `narration_denominator_and_hints_d7`,
`standing_material_channel_am9`, `retract_trip_channel_am7` — **8/8 binaries,
50 tests, 0 failures.**

### 6.1 Intake: a SECOND pre-existing red

`transform_provenance_fingerprints` fails 3 of 3 tests. **It was already
failing at HEAD `675a643`.** Verified by `git stash push` of exactly the
seven changed paths, re-running the binary, and comparing: **identical
left/right values on all three tests**, e.g.
`three_pass: left (23, 14265253333427783116) / right (23, 14756822782673573601)`.
Move *counts* are unchanged (23, 74, 40) in every case — geometry hashes
moved, counts did not. The stash was popped and the tree restored.

Reported for intake alongside `literature_matrix::flat_3mm_pocket_ipe_extreme`
(G-LIT-IPE). Not S-4's to fix, and it does not pollute S-4's pass/fail: the
S-4 binary is run by name.

## 7. Discipline

One Cargo job machine-wide throughout; `free -g` + bracketed `pgrep`
before each launch (one BUSY reading was the documented pgrep self-match
trap, re-checked and cleared). No release builds. Per-crate, per-binary
tests. Disk stayed at 144 GB free. `cargo fmt --check` clean with **no
rustfmt cascade** (before/after `git status` compared — only the two files
this wave wrote were reformatted). `cargo clippy -p rs_cam_core -p rs_cam_viz
--all-targets -- -D warnings` clean. Explicit staging; no `--amend`; the
wanaka play-file, `planning/review_2026-07-27/` and the operator's feeds
review were never staged.

## 8. Recommended ledger disposition

**G-BYTE: RESOLVED as diagnosed — cause identified, provenance shipped, no
determinism defect.** Re-open condition: a byte difference observed between
two generations whose `stock_snapshot` stamps are **equal**. That would be
generator nondeterminism and is what arm A rules out today.

Carried forward, not fixed: the `G0`-approach-height class is unexercised on
this fixture (§4). Suggest attaching it to whichever future wave builds a
fixture with sub-stock-top entry ceilings, rather than opening a row for it
alone.
