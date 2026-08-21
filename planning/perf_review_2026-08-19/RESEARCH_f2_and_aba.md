# RESEARCH — W5B-F2 sentry re-expression, and the triage cache ABA hazard

Read-only research lane, 2026-08-21. Branch `tech-debt-3` @ `1d6dd855`.
Nothing here is implemented; both sections are sized for an implementation
lane. **Topic A changes what a shipped sentry asserts — that is a user
decision, flagged inline.**

No cargo was run in this lane. Every measured figure quoted below is
sourced from the sentry files' own comments and `DELTA_sim_w5b_landing.md`
§3, both dated 2026-08-21; none was re-measured here, and that is stated
where it matters.

---

# TOPIC A — W5B-F2: re-express F-027 / F-031 against commanded Z travel

## A.0 Two corrections to the premise, before the design

**(i) The sentries do not assert on `peak_axial_doc_mm`.** All three bars
read the *per-sample* field `SimulationCutSample::axial_engagement_mm` and
do their own max / count reduction inside the test. `peak_axial_doc_mm` is
the `SimulationSummary` roll-up and is **not referenced by any of the three
tests**. This matters for the re-expression: the tests already control
their own filter and denominator, so nothing has to change in core to
change the bar.

Sites:

| test | file:line | bar |
|---|---|---|
| `as013_terrain_model_edge_axial_within_commanded_dpp_f027` | `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs:195` | ceiling `dpp+1.0` = 4.000 **and** population `dpp+0.5` ≤ 5e-4 |
| `as013_terrain_model_edge_band_outlier_count_bounded_f027` | `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs:356` | population `dpp+0.5` ≤ 5e-4 |
| `as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031` | `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs:215` | ceiling `dpp+1.0` = 4.000 **and** population `dpp+0.5` ≤ 5e-4 |

`commanded_dpp = 3.0` is a **hard-coded literal** in all three
(`…f027.rs:260`, `…f027.rs:387`, `…f029.rs:251`), not read from the config
that sets it (`Adaptive3dConfig::depth_per_pass = 3.0`,
`…f027.rs:111`). That is a second, independent (and much cheaper) fix
available in the same lane.

**(ii) "Commanded Z travel per pass", read literally, is blind to both
defects this pair exists to catch.** The follow-up as filed
(`DELTA_sim_w5b_landing.md:539`) proposes asserting on *"the tool's
commanded Z travel per pass — a quantity still bounded by `dpp` under both
kernels"*. It is indeed kernel-invariant, and that is exactly the problem:
it is derived from the emitted toolpath, and **neither F-027 nor F-031
moved the emitted Z ladder.**

* **F-027's mechanism** (`…f027.rs:21-32`) is a planner *grid* that was too
  small in XY. Cells in the band `(mesh.max.y, stock.max.y]` were never
  stamped by the planner, so the simulator carried them virgin and its
  first stamp cleared the whole ray → 30–47 mm readings. The planner's Z
  ladder was a uniform 3.0 mm step before the fix and after it. A bar of
  the form `max commanded Z step ≤ dpp + ε` reads **green pre-fix**.
* **F-031's defect** was 282 *transit* samples at up to 44.8 mm from a
  planner↔dressup helix gap (`…f029.rs:237-245`). Same conclusion: the
  commanded ladder does not move.

So the re-expression as literally filed is not a re-expression of these
sentries. It is a **different, additional** sentry (see A.4 — it has real
standalone value), and adopting it *as a replacement* silently deletes
both regression nets.

## A.1 What the invariant actually is

Settled already, in-tree, by the H4 probe
(`crates/rs_cam_core/tests/axial_doc_step_multiple_h4.rs:45-62`), which
hand-builds a toolpath so coverage is known in closed form:

> `axial_engagement_mm` is `max(pre_ray_length − post_ray_length)` … the
> height of material this stamp REMOVED. **It has never been a reading of
> the commanded step and cannot be one: nothing in the stamping kernel
> knows what the step was.** An exact `n × step` reading means the column
> carried `n` steps of stock when the pass arrived — a COVERAGE fact about
> the toolpath, faithfully measured.

Therefore the F-027 / F-031 bars are, and always were, **coverage** bars,
not depth bars. Stated properly:

> **No column may carry more than one commanded Z step of standing stock at
> the moment a pass arrives over it.**

`dpp` is in the bar as the *denominator of a ratio* — the commanded step —
not as a depth limit. Today the test supplies that denominator as a global
literal `3.0`. The kernel change moved the *numerator*: `whole_path` split
a column's removal across the subsegments that blended it (under-reading
any single sample), `swept` credits the whole column to one sample. Same
physical column, more faithful read, ratio moves 1.0-ish → 1.23 / 1.30.

**Consequence for the whole topic:** the numerator is inherently
kernel-observed. There is no formulation of "did the simulator find a tall
column here" that does not read the simulator. Kernel-invariance and
defect-detection are in direct tension, and any design must pick where to
spend it. The three candidates below spend it differently.

## A.2 Candidate 1 — per-sample **local** commanded step (the honest reading of F-2)

Replace the global literal denominator with the commanded step **of the
pass the sample belongs to**, and assert the ratio.

### Exact computation

Both fixtures are adaptive3d, and adaptive3d **already publishes its Z
ladder as structured spans** — no move-walking, no Z clustering, no
tolerance heuristics:

* `compute::spans::push_adaptive3d_spans`
  (`crates/rs_cam_core/src/compute/spans.rs:484-541`) emits one
  `SpanKind::DepthPass` per `RegionZLevel` / `GlobalZLevel` runtime event,
  carrying `SpanPayload::DepthPass { z_level, pass_index }`
  (`…/spans.rs:532-538`). `WaterlineCleanup` events become
  `SpanKind::WaterlineCleanup` instead and carry **no** `z_level` — they
  must be excluded, and the existing `in_transit_span` filter already does
  that.
* Every sample carries `span_path: Vec<SpanId>`
  (`crates/rs_cam_core/src/simulation_cut.rs:204-208`), outermost-first,
  indexing `AnnotatedToolpath::spans`. So sample → its DepthPass span →
  `(z_level, pass_index)` is a direct join, no spatial search.
* `AnnotatedToolpath::spans_of_kind` is public
  (`crates/rs_cam_core/src/toolpath_spans.rs:593`), and the annotated
  toolpath is reachable from the test via
  `session.get_result(0)` (`crates/rs_cam_core/src/session/mod.rs:1282`)
  → `.annotated()` (`…/mod.rs:685`).

Ladder, then:

```
levels = spans_of_kind(DepthPass)
           .filter_map(|s| payload as DepthPass { z_level, pass_index })
           .collect, dedup by pass_index, sort by pass_index ascending
step(pass_index i) = levels[i-1].z_level − levels[i].z_level     for i ≥ 1
step(pass_index 0) = ABSTAIN                                     see below
```

Per steady-state sample (filters unchanged: `is_cutting`,
`cut_kinematics != Plunge`, `!in_transit_span`, and for F-027 also the
`mesh_max_y < y ≤ stock_max_y` band):

```
i = pass_index from the sample's DepthPass span
ratio = s.axial_engagement_mm / step(i)
ceiling bar   : max ratio ≤ CEILING_RATIO
population bar: fraction of samples with ratio > POPULATION_RATIO ≤ 5e-4
```

**Pass 0 must abstain, not fall back to `dpp`.** For adaptive3d on terrain
the first Z level sits near the *mesh* top, while `stock_top_z` is the
auto-grown world stock top (the F-026 growth to z≈57.6 on this fixture,
`…f027.rs:16-19`). `stock_top − z_levels[0]` is therefore tens of
millimetres of mostly-air and is a meaningless denominator. Pass 0 samples
should be counted into a separate reported population and excluded from
both bars, with the count printed — an unreported exclusion is how a bar
becomes vacuous (see the empty-population learning in `CLAUDE.md`).

### What the new bars would be

Today's readings against a **flat 3.0** denominator translate directly, as
long as the ladder is uniform (it is on this fixture: `depth_per_pass =
3.0`, `z_blend: false`, `detect_flat_areas: false`, `fine_stepdown: 0.0`,
`…f027.rs:107-134`):

| | today's absolute bar | today's reading | as a ratio | proposed ratio bar |
|---|---:|---:|---:|---:|
| F-027 ceiling | 4.000 mm | 3.694 mm | 1.231 | **1.35** (9.7% headroom) |
| F-031 ceiling | 4.000 mm | 3.890 mm | 1.297 | **1.35** (3.9% headroom) |
| both, population | 3.500 mm | — | 1.167 | **1.1667**, cap 5e-4 unchanged |

**These are conversions, not measurements.** They assume the ladder is
exactly 3.0 on every pass ≥ 1 of this fixture. That assumption is cheap to
falsify and **must be checked as step 1 of the implementation lane** (print
the ladder; if any step ≠ 3.0 the bars move and today's numbers do not
carry over). If a short final pass exists, the re-expression is a
**tightening** on that pass — a green→red risk, and the main reason this
is not a mechanical edit.

### What is gained, precisely

Not kernel-invariance. What is gained is that the denominator stops being
a literal that can drift from the config, and starts being the quantity the
planner actually commanded — so the bar survives a change to
`depth_per_pass`, a non-uniform ladder, `fine_stepdown`, or a short final
pass, none of which today's `3.0` literal survives.

### What is lost — and the answer to "would we still catch a kernel regression"

**Nothing is lost relative to today, because the numerator is unchanged.**
A stamp-kernel regression that inflated measured axial by, say, 2× would
push F-031's reading 3.890 → 7.78, ratio 2.59, and blow through a 1.35
ceiling exactly as it blows through today's 4.000 mm. The kernel-guard
property of these bars is intact under Candidate 1.

The companion tolerance assertion the brief asks about is therefore **not
needed for Candidate 1** — it would be the same assertion twice. It *is*
needed for Candidate 3 (§A.4), which does move the numerator off the
simulator.

## A.3 Candidate 2 — self-normalised ratio (the only design that is genuinely kernel-invariant *and* keeps detection)

Compare two numbers **produced by the same instrument in the same run**, so
a kernel change cancels in the ratio:

```
F-027: band_max_axial / interior_p99_axial          (interior = y ≤ mesh_max_y)
F-031: steady_max_axial / steady_p99_axial
```

Rationale, and it is the repo's own rule: *check both sides of a ratio are
the same MEASURE* (`feedback_instrument_integrity`). Both sides here are
`axial_engagement_mm` from the same trace.

Defect separation is large. F-027's defect was a band reading 30–47 mm
while the interior read ~3 mm — a ratio of 10–15×. Post-fix the two
populations are drawn from the same distribution, so the ratio should sit
near 1. A bar at ~2.5× (mirroring the 2.5× skew bar the parity pair already
uses, `DELTA_sim_w5b_landing.md` §4) would catch the defect by 4–6× and be
immune to any kernel change that scales both populations.

**Cost:** requires a p99 (sort or a histogram) over 630 865 samples for
F-031 — a few ms, irrelevant against the sim itself. **Requires the
denominator population to be non-vacuous and must abstain loudly below a
floor** (the parity pair's 50-cell interior floor is the precedent).

**Not measured in this lane.** The interior p99 for either fixture is not
recorded anywhere I could find, so the bar value above is reasoned, not
observed. An implementation lane must measure it before choosing the
constant.

**Risk:** a kernel regression that inflates *everything* uniformly cancels
out and reads green. That is the exact dual of Candidate 1's weakness, and
it is why Candidate 2 should be **added to**, not substituted for, an
absolute ceiling.

## A.4 Candidate 3 — the literal W5B-F2 sentry, as a *new* test

Assert the planner-side ladder directly, with no simulator in the loop:

```
levels = DepthPass z_levels from result.annotated(), sorted descending
max_step = max over i≥1 of (levels[i-1] − levels[i])
assert max_step ≤ config.depth_per_pass + 1e-6
```

* **Runs without a simulation at all** — generate only. On the AS013
  fixture that removes the whole sim cost from this bar.
* **Catches something nothing currently asserts:** that adaptive3d's
  emitted Z ladder honours its commanded `depth_per_pass`. Grep found no
  existing sentry on that. A planner regression that doubled a step would
  today only be visible *through* the simulator's axial reading, i.e.
  confounded with the kernel.
* **Catches neither F-027 nor F-031** (§A.0(ii)). It must not carry their
  names or replace their bars.

If Candidate 3 lands *and* the measured bars are kept, the pair is
complementary: C3 pins the commanded side, C1/C2 pin the observed side, and
the "companion tolerance assertion" the brief asks for is then expressible
cleanly as `measured_max ≤ k × commanded_max_step` — which is precisely
Candidate 1's ceiling. The three collapse into one coherent net:

| asserts | instrument | catches |
|---|---|---|
| ladder ≤ dpp | planner only | planner step regression |
| max measured / local commanded step ≤ 1.35 | sim ÷ planner | F-027/F-031 defect class **and** a kernel inflation |
| band max / interior p99 ≤ 2.5 | sim ÷ sim | F-027 spatial defect, kernel-immune |

## A.5 Recommendation

1. **Do not adopt W5B-F2 as filed** (replace measured with commanded). It
   deletes both regression nets. Correction to the follow-up's own wording
   is warranted in `DELTA_sim_w5b_landing.md:539`.
2. **Adopt Candidate 1** as the re-expression: same numerator, better
   denominator, plus kill the three hard-coded `3.0` literals. This is what
   "re-expressed against commanded Z travel" should have meant.
3. **Add Candidate 3** as a new, cheap, sim-free planner sentry under its
   own name (not F-027/F-031).
4. **Candidate 2 optional**, and only as an addition; needs a measurement
   first.

## A.6 Blast radius and LOC

| file | change | LOC |
|---|---|---|
| `crates/rs_cam_core/tests/common/mod.rs` (or new `common/zladder.rs`) | `pub fn commanded_z_ladder(&AnnotatedToolpath) -> Vec<(u32 pass_index, f64 z_level, Option<f64> step)>` + `pub fn pass_index_of(&SimulationCutSample, &AnnotatedToolpath) -> Option<u32>` | **+55 / −0** |
| `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs` | 2 tests: ratio denominator, pass-0 abstention + its printed count, comment rewrite | **~+70 / −45** |
| `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs` | 1 test, same treatment | **~+40 / −25** |
| new `crates/rs_cam_core/tests/adaptive3d_commanded_z_ladder.rs` (Candidate 3) | fixture reuse + one assert | **+90 / −0** |

**Total ≈ 255 added / 70 removed, tests only. Zero production files.**
`common/mod.rs` is shared by every core integration test, so a helper added
there is compiled by all of them — keep it in a new `common/zladder.rs`
module if compile-time matters.

**Risk: MEDIUM, concentrated in one place.** The whole risk is the §A.2
assumption that every pass ≥ 1 steps exactly 3.0 on this fixture. If a
short pass exists, Candidate 1 tightens the bar on it and can turn a
shipped green red. Step 1 of the lane is to print the ladder. Everything
else is mechanical.

**Second-order risk:** `AnnotatedToolpath::spans_valid`
(`crates/rs_cam_core/src/toolpath_spans.rs:436`) — transforms that cannot
remap set it false, and an invalidated span table must not be trusted. The
helper must check it and **abstain loudly** rather than silently produce an
empty ladder (an empty ladder would make every sample abstain and the bar
vacuous — the exact failure mode `CLAUDE.md` warns about). Not verified in
this lane whether adaptive3d's spans survive the AS013 dressup/TSP/arc-fit
chain with `spans_valid == true`; **the lane must assert it explicitly.**

## A.7 Decision flags (user)

* **D-A1** — Is deleting the measured bars acceptable? *Recommendation:
  no.* W5B-F2 as filed would do that.
* **D-A2** — Candidate 1 may turn a shipped green red on a short final
  pass. Accept a possible re-baseline, or gate the change on the ladder
  measurement first?
* **D-A3** — Ship Candidate 3 as a separate new sentry (yes/no)?
* **D-A4** — Spend the extra lane time on Candidate 2's p99 measurement, or
  defer?

---

# TOPIC B — `cached_simulation_triage` ABA hazard

## B.0 Verdict: the hazard is real, but the brief mislocates it twice

**Correction 1 — it is not in the session layer.**
`ProjectSession::simulation_triage`
(`crates/rs_cam_core/src/session/compute.rs:3164`) and
`simulation_triage_with_diagnostics` (`:3186`) are **uncached**; every call
rebuilds. The cache lives in the **viz GUI state layer**:
`SimulationState::cached_simulation_triage`,
`crates/rs_cam_viz/src/state/simulation.rs:845-879`.

**Correction 2 — the key is not a bare `Arc::as_ptr`.** It is a pointer
*paired with the GUI edit counter*, plus a `built` flag
(`crates/rs_cam_viz/src/state/simulation.rs:189-199`):

```rust
pub(crate) struct SimulationTriageCache {
    built: bool,
    trace_ptr: Option<usize>,
    edit_counter: u64,
    triage: rs_cam_core::sim_triage::SimulationTriage,
}
```

Lookup and insert are one function (`…/simulation.rs:850-877`, verified
directly in this lane):

```rust
let trace_ptr = self.results.as_ref()
    .and_then(|results| results.cut_trace.as_ref())
    .map(|trace| Arc::as_ptr(trace) as usize);          // :850-854
let fresh = self.debug.triage_cache.built
    && self.debug.triage_cache.trace_ptr == trace_ptr
    && self.debug.triage_cache.edit_counter == edit_counter;  // :855-857
```

**But the pairing does not close the hazard**, because `edit_counter`
(`crates/rs_cam_viz/src/state/runtime.rs:118`) is bumped **only** by
`GuiState::mark_edited` (`runtime.rs:192`) and counts *project edits*.
**Re-running a simulation does not bump it.** So the second key component
narrows the window and does not close it. The corrections ledger gains a
sixteenth entry on the location and the key shape; **the hazard itself
reproduces.**

## B.1 Plausibility — the allocation-size argument runs the *wrong way* here

`ArcInner<SimulationCutTrace>` is two atomics plus a **fixed-size** struct;
all sample data hangs off separate heap `Vec`s. Every cut trace in the
process is therefore the *identical* allocation size — the same malloc size
class. This is the opposite of the `geom_cache` mesh case (a 111 MB mesh is
its own size class), and it means the "require an identically sized
replacement" mitigation used in `gpu_upload.rs` **would buy nothing** if
transplanted here. Once a free precedes an allocation, same-address reuse
is likely, not unlikely.

What actually protects the common path is **drop ordering**, not the key:

* **Ordinary re-simulate is SAFE.** `controller/events/compute.rs:968-980`
  assigns `simulation.results = Some(SimulationResults { cut_trace, … })`.
  The new `Arc` was allocated on the worker thread *while the old one was
  still live in state*; the old value drops only after the assignment. The
  two coexist, so the addresses cannot coincide.
* **The feed-modulation take/put-back** (`compute.rs:1004-1017`) reuses the
  same `Arc`; `Arc::make_mut` (`session/compute.rs:2865`) mutates in place
  inside one handler with no intervening render.

**The reachable path is invalidate-then-resimulate.**
`invalidate_simulation` (`controller/events/simulation.rs:22-30`, read
directly) does `sim.results = None;` — the old `Arc` is freed with nothing
replacing it — **and does not bump `edit_counter`, and does not touch
`triage_cache`**. It is reached from the Reset-Simulation menu item
(`ui/menu_bar.rs:218`), the viewport overlay (`ui/viewport_overlay.rs:276`)
and every undo/redo of stock/tool/machine (`controller/events/undo.rs:12,
30, 53, 64, 82, 105`), none of which bump `edit_counter` either.

For a false hit you additionally need the panel **never to render while
`results` is `None`** — one such render writes `trace_ptr = None` and
defuses it. So the gesture is: Diagnostics section collapsed or its tab
hidden → Reset Simulation → re-simulate → expand. Working *against* the
hazard: the free happens on the main thread and the allocation on the
compute worker, and glibc's per-thread tcache does not hand a main-thread
free straight back to a worker thread.

**Net: possible, not routine. Low frequency, and today low consequence
(§B.2). Not a fire.**

## B.2 Blast radius — one consumer, and it is *not* the four in the brief

Exactly one call site:
`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:605`, inside the
`draw_project_section` collapsing header (`…:474-477`, `default_open(true)`).
It reads **only** `triage.measurability.entries` (`…:606-610`) to render
the `"NOT MEASURED: {metrics} for {n} operation(s)"` strip (`…:625-629`).

A stale hit therefore shows a *measurability* strip belonging to a previous
trace: wrong metric names, wrong operation count, or — the silent and worse
case — **no strip at all when the current trace really is unmeasurable**.
It does **not** corrupt collision counts; the collisions pill
(`sim_diagnostics.rs:558-563`) has a different producer.

The other three surfaces named in the brief are **unaffected** — all
rebuild from core on every call:

| Surface | Site | Cached? |
|---|---|---|
| MCP `get_diagnostics` | `crates/rs_cam_viz/src/controller/events/compute.rs:1944-1946` | **No** |
| CLI `project` report | `crates/rs_cam_cli/src/project.rs:462` | **No** |
| `narrate_toolpath` | `crates/rs_cam_viz/src/app/mcp.rs:1251` | Does not consume triage at all |

## B.3 A larger staleness hole in the same cache — and it is not ABA

`project_evidence()` (`crates/rs_cam_viz/src/state/simulation.rs:1640-1662`)
feeds the triage from inputs that are **not in the key at all**:
`checks.rapid_collisions`, `checks.rapid_collision_move_indices`,
`holder_collision_counts_by_tp()` (from `checks.collision_report`), and
`resolution_mm`. The holder-collision report is written by a *separate
async job* (`controller/events/compute.rs:1112-1126`) with no
`edit_counter` bump, no cache invalidation and the same `trace_ptr`. The
triage consumes those for its **safety** findings
(`crates/rs_cam_core/src/sim_triage.rs:263`, `:295`).

The neighbouring `issue_cache_key` (`state/simulation.rs:1839-1880`) folds
in a `collision_fingerprint` over exactly those fields. **The triage cache
does not.** Today this is masked only because the sole consumer reads
`measurability` and not `safety`. It becomes a **live safety-display bug**
the moment anyone renders `triage.safety` from the cached object — which is
the natural next feature for that panel, since `CLAUDE.md` tells agents to
read `triage.safety` first.

**This is more likely to bite than the ABA, requires no allocator
coincidence, and should be fixed in the same lane.**

## B.4 The `geom_cache` Weak-pin fix, and the transplant

`crates/rs_cam_core/src/geom_cache.rs:17-41` already argues the exact case,
in as many words:

> The review proposes keying on `Arc::as_ptr`. **A bare pointer is not a
> sound key**: an `Arc` can be dropped and a fresh allocation can land at
> the same address … 1. A live `Weak` keeps the `Arc` *allocation* from
> being freed even after the last strong reference drops (only the `T`
> inside it is dropped), so while an entry exists **no other `Arc` can be
> handed that address**. 2. Lookup upgrades the `Weak` and compares with
> `Arc::ptr_eq`. … This is strictly stronger than pairing the pointer with
> an element count, which narrows the collision window rather than closing
> it.

Implementation (`geom_cache.rs:141-165`):

```rust
struct Entry { mesh: Weak<TriangleMesh>, … }
fn matches(&self, mesh: &Arc<TriangleMesh>) -> bool {
    self.mesh.upgrade().is_some_and(|live| Arc::ptr_eq(&live, mesh))
}
```

The same pattern, generalised over several Arcs, is in
`crates/rs_cam_core/src/compute/sim_prefix.rs:240-271` (`EntryKey` +
`weak_matches`), with the rule stated at `sim_prefix.rs:91`: *"**Pointer
keys are `Weak`, never bare pointers.**"*

So `rs_cam_core` has adopted the correct pattern **twice**. `rs_cam_viz`
has not.

### Spec — transplant into `SimulationTriageCache`

**Key type change** (`crates/rs_cam_viz/src/state/simulation.rs:189-199`):

```rust
pub(crate) struct SimulationTriageCache {
    built: bool,
-   trace_ptr: Option<usize>,
+   /// Liveness-checked identity key. A `Weak` pins the allocation, so a
+   /// recycled address cannot false-hit. See `rs_cam_core::geom_cache`.
+   trace: Option<Weak<SimulationCutTrace>>,
    edit_counter: u64,
+   /// Fingerprint over the evidence inputs that are NOT the trace —
+   /// mirrors `issue_cache_key`'s `collision_fingerprint`. See §B.3.
+   evidence_fp: u64,
    triage: rs_cam_core::sim_triage::SimulationTriage,
}
```

**Lookup** (`…/simulation.rs:850-857`):

```rust
let live = self.results.as_ref().and_then(|r| r.cut_trace.as_ref());
let fresh = self.debug.triage_cache.built
    && weak_matches(self.debug.triage_cache.trace.as_ref(), live)
    && self.debug.triage_cache.edit_counter == edit_counter
    && self.debug.triage_cache.evidence_fp == evidence_fp;
```

with `weak_matches` copied verbatim from `sim_prefix.rs:265-271` (the
`(None, None) => true` arm is required — "no trace" is a legitimate cached
state, which is why `built` exists).

**Insert:** `trace = live.map(Arc::downgrade)`.

**Note the memory consequence, and that it is negligible here:** a live
`Weak` keeps the `ArcInner` *header* alive after the last strong ref drops
— for `SimulationCutTrace` that is a fixed-size struct whose `Vec`s have
already been freed, i.e. tens of bytes, not the sample data. Contrast
`geom_cache`, where the same trick pins a mesh header. No leak of
consequence. The `Weak` is replaced on the next miss.

### LOC and files

| file | change | LOC |
|---|---|---|
| `crates/rs_cam_viz/src/state/simulation.rs` | `SimulationTriageCache` key type + lookup/insert + a local `weak_matches` helper + `evidence_fp` (reuse the `issue_cache_key` fingerprint code at `:1839-1880`) | **~+45 / −8** |
| same, if the three siblings are fixed too (recommended, §B.5) | `ToolLoadReportCache` (`:176-180`, used `:768-807`), `ChiploadEnvelopeCache` (`:182-187`, used `:810-841`), `SpanAggregateCache` (`:355-372`) | **~+55 / −12** |

**Total ≈ 100 added / 20 removed, one file, one crate (`rs_cam_viz`).**
No core change. No wire/schema change. No test fixture change.

**Risk: LOW.** The change is strictly narrowing — a `Weak` key can only
*miss* where a pointer key hit, and a miss costs a rebuild, which is the
measured-slow-path already instrumented at `simulation.rs:867-872` (an
8 ms `tracing::debug!` threshold, so a regression in hit rate is already
observable without new instrumentation). The one way to get this wrong is
dropping the `(None, None) => true` arm, which would rebuild the triage
every frame when no trace exists.

**Test:** a viz-side unit test that (a) builds a cache entry, (b) drops the
`Arc`, (c) allocates a new trace and asserts the cache **misses**, is the
direct sentry. Forcing address reuse deterministically is not reliable —
prefer asserting `weak_matches` returns false for a dropped `Weak`, which
is the property that makes the address question moot.

## B.5 Sweep — three more sites with the same hazard, all in `rs_cam_viz`

| Site | Key | Assessment |
|---|---|---|
| `crates/rs_cam_viz/src/state/simulation.rs:777` — `cached_load_report` | `(trace_ptr, edit_counter)` | **Same hazard, worse consequence.** Consumers `ui/sim_op_list.rs:218`, `ui/sim_diagnostics.rs:29`, `ui/sim_timeline.rs:68` — a stale hit shows wrong chipload / power / deflection **verdicts** and wrong per-op badges |
| `crates/rs_cam_viz/src/state/simulation.rs:810` — `cached_chipload_envelopes` | `(trace_ptr, edit_counter)` | Same hazard; wrong chipload band envelopes at `sim_diagnostics.rs:909`, `sim_timeline.rs:329` |
| `crates/rs_cam_viz/src/state/simulation.rs:370` — `SpanAggregateCache::cached_trace_ptr` | **bare `Option<usize>`, no edit counter** (`:355-357`, `:368-372`) | **Strictest form of the hazard in the tree.** Does have an explicit `invalidate()` (`:404-408`), which the triage cache lacks |
| `crates/rs_cam_viz/src/app/gpu_upload.rs:959-960`, `:1031`, `:1192` — `ToolpathUploadKey` | bare pointer, no len | Hazard in kind; consequence is a stale GPU line buffer — visual, not a safety number |

**Partially mitigated (narrowed, not closed):**

* `state/simulation.rs:1845, 1850, 1873` — `IssueListCacheKey` hashes the
  trace pointer *with* `annotations.len()` / `hotspots.len()` /
  `items.len()` and a collision fingerprint (`:1839-1880`).
* `app/gpu_upload.rs:239, 242` — `(Arc::as_ptr, triangles.len())`, and it
  says so honestly at `:224-228`.

**Safe / unrelated** (identity checks on Arcs both ends of which are live,
or slice pointers): `ui/optimize_modal.rs:742`,
`rs_cam_core/src/region_set.rs:144`, `tests/common/bandmap.rs:341`,
`tests/geometry_cache_g8.rs:278,283`.

## B.6 The doctrine conflict worth surfacing

`crates/rs_cam_viz/src/render/upload_cache.rs:28-31` justifies its bare
pointer key by **citing `cached_simulation_triage` as the established
precedent**:

> keyed by `Arc::as_ptr` — pointer identity, which this codebase already
> uses as a staleness key for the simulation caches
> (`SimulationState::cached_simulation_triage`).

`crates/rs_cam_core/src/geom_cache.rs:19-41`, same workspace, argues at
length that a bare pointer **is not a sound key** and names the viz-side
"pointer + element count" as the weaker mitigation.

Two modules encode contradictory doctrine, and the weaker one is cited as
precedent for new code. Fixing `state/simulation.rs` removes the citation's
subject; the comment at `upload_cache.rs:28-31` should be corrected in the
same lane whether or not `upload_cache` itself is fixed.

## B.7 Decision flags (user)

* **D-B1** — Fix the triage cache alone, or all four viz caches (`triage`,
  `load_report`, `chipload_envelopes`, `span_aggregate`) in one lane?
  *Recommendation: all four* — the pattern is copy-paste, the risk is flat,
  and `load_report` has the worse consequence of the set.
* **D-B2** — Include the §B.3 `evidence_fp` fix? *Recommendation: yes.* It
  is more likely to bite than the ABA and is currently masked only by which
  field the panel happens to read.
* **D-B3** — Also fix `gpu_upload.rs` / `upload_cache.rs`, or only correct
  their comments? Visual-only consequence; defensible either way.
* **D-B4** — Priority. Given B.1 (needs an unusual gesture) and B.2 (one
  low-stakes readout), this is **not urgent on its own merits**. Its case
  rests on B.3 and B.5's `load_report`.

---

## Corrections ledger entries this lane produced

1. **W5B-F2's own wording** (`DELTA_sim_w5b_landing.md:539`) proposes a
   substitution that would delete both regression nets — the commanded Z
   ladder does not move under either F-027's or F-031's defect.
2. **The F-027/F-031 sentries do not read `peak_axial_doc_mm`**; they read
   per-sample `axial_engagement_mm` and reduce it themselves.
3. **The triage cache is in `rs_cam_viz`, not the session layer**, and its
   key is `(Arc::as_ptr, edit_counter)`, not a bare pointer. The hazard
   nonetheless reproduces, because `edit_counter` does not count
   simulation runs.

