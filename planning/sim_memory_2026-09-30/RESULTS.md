# G-SIMMEM — the simulation of a large project took more than 20 GB (2026-09-30)

Operator report 2026-09-29: `rivmap350.toml` (350 x 500 mm terrain, stock
380 x 510 x 26 mm, 8 toolpaths, an R1.0 tapered-ball scallop of about 8 h)
went 2.3 GB -> 19.4 GB in 30 s at 0.5 mm cells and to 22.8 GB three minutes
later; the OOM killer took the desktop twice. Uncommitted working-tree fix;
the lead reviews.

## Repro

Both files are in `planning/fixtures/rivmap100/` (see its README):

- `rivmap100_memory_small.toml` — the live copy with the Scallop on (fresh
  stock, so 0.5 mm cells are allowed), Face heights auto. 110 x 110 mm stock.
- `rivmap100_memory_repro.toml` — terrain scaled x3.5 (`units = custom 3.5`,
  350 x 350 x 42 mm), stock 380 x 510 x 46 mm: 760 x 1020 = 775 k columns at
  0.5 mm, the operator's grid. Scallop 3.07 M moves, 1159 m fed.

Every run under a hard cap. This container has no systemd user bus and no
`/usr/bin/time`, so `memrun.py` (here) stands in for `time -v`: it reports
the child's peak RSS (`ru_maxrss`) and samples VmRSS every 2 s.

```
prlimit --as=8589934592 -- planning/sim_memory_2026-09-30/memrun.py \
  target/release/rs_cam_cli project <toml> --resolution 0.5 --output-dir <dir>
```

## What grew — the numbers

A temporary env-gated probe (removed) printed the loop's structures and
VmRSS at each phase.

| structure (rivmap100_memory_small) | size |
|---|---:|
| cut samples | 2,012,349 x 280 B + 24 MB span heap = 559 MB |
| prior-stock snapshots | 3, 3 MB |
| checkpoints | 3: meshes 21 MB, stocks 3 MB |
| global stock / composite mesh | 1 MB / 7 MB |

The per-sample trace is the only term that is not grid-sized. Its count is
not the path length at the declared `sample_step_mm` either:

| entry (small) | fed mm | Σ\|Δz\| mm | samples by length | samples pushed |
|---|---:|---:|---:|---:|
| Scallop | 70,245 | 30,082 | 292,258 | 1,655,659 |
| 3D Rough | 20,733 | 4,756 | 50,788 | 261,136 |

| entry (repro) | fed mm | Σ\|Δz\| mm | samples by length | samples pushed |
|---|---:|---:|---:|---:|
| Face | 137,138 | 718 | 562,805 | 597,269 |
| 3D Rough | 390,932 | 56,178 | 1,167,720 | 3,686,971 |
| Scallop | 1,158,806 | 640,589 | 4,229,752 | 34,072,681 |

**Cause.** The metric walk pushed one `SimulationCutSample` per STAMPING
subsegment, and a move is stamped in `max(⌈len/step⌉, ⌈|Δz|/0.02⌉)` of
them. The 0.02 mm Z-drop cap is a stamp-accuracy device; on a terrain finish
it made the trace 5-8x denser than its own step. The repro's scallop asked
for ONE `Vec::with_capacity` of 34 M x 280 B = 9.54 GB before its first
stamp, and the whole trace would have been about 10.7 GB. The operator's
board is 1.4x the repro's scallop area: about 15 GB of trace, which is the
2.3 -> 19.4 GB he saw.

Three transients multiplied it:

1. **The S5 memo** cloned the whole running state (every sample) and only
   then checked its 1.5 GB ceiling, so a snapshot too big to keep was still
   built (small: loop RSS 808 -> 1406 MB at the snapshot).
2. **`Vec::append`** copied the finishing entry's samples into the run's
   buffer: one more copy of the largest entry.
3. **The cut-trace artifact** (GUI worker and `cli project`) deep-cloned the
   trace and serialised it with `to_vec_pretty` / `to_string_pretty`: one
   document of ~1.24 kB per sample, 4.4x the trace, in memory
   (small: 2.5 GB `simulation.json`; RSS 891 MB -> 3.88 GB peak). In the GUI
   this is `compute/worker/execute/mod.rs`, which runs after every
   simulation; a document of ~4x a 15 GB trace is consistent with the fast
   step the operator saw (not measured in a GUI here).

## The fix

- `dexel_stock/sample_coalesce.rs` (new): after a Z-subdivided move's
  metrics are final, its samples are coalesced in groups of
  `g = ⌈subsegments / by_length⌉`, so a move keeps at most `⌈len/step⌉`
  samples. Stamps, stamp order and the stock are untouched; `g = 1` moves
  are untouched bit for bit. Per-field rules: sums for time and removed
  volume (mrr = ratio), the last clock; time-weighted MEANS for the three
  readings the feed modulator averages per move (radial WOC, `axial_doc_mm`,
  axial DOC fraction); MAXIMA for `axial_engagement_mm` ("maximum material
  height engaged at this sample"), plunge descent, arc and chip thicknesses
  (the peaks the depth, deflection, chipload and power gates read).
  Coalescing runs when the samples it would drop reach `staging_budget` —
  one dexel grid's bytes, derived from the grid — after draining the stamp
  queues (a flush only splits a batch earlier; the dispatchers document it
  as result-neutral), and once at the end of the walk. The capacity
  estimator reserves the coalesced count.
- `compute/sim_prefix.rs` + `simulate.rs`: the memo is asked `admits(est)`
  BEFORE the snapshot is cloned.
- `simulate.rs::append_samples`: move the larger buffer, copy the smaller.
- `stock/simulation_cut.rs`: `SimulationCutArtifact.trace` is an
  `Arc<SimulationCutTrace>` (same JSON); the GUI worker, `cli project` and
  `cli job` pass `Arc::clone`/owned, never a deep copy.
- `export/artifact_io.rs`: artifacts stream through a `BufWriter`
  (`to_writer_pretty`, byte-identical to `to_vec_pretty`);
  `write_simulation_cut_artifact_to` is the CLI's door.

## Before / after (peak RSS, wall time, 8 GiB cap)

| project | before | after |
|---|---|---|
| `rivmap100_live_0925.toml` | 744 MB, 14 s | 211 MB, 8 s |
| `rivmap100_memory_small.toml` | 3.88 GB, 162 s | 0.51 GB, 130 s |
| `rivmap100_memory_repro.toml` | ABORT at 408 s: `memory allocation of 9540350680 bytes failed` (RSS 3.7 GB) | 4.59 GB, 514 s, exit 0 |

Repro after: 6,489,338 samples (38.4 M before), trace 1.80 GB. The
remaining peak is the retained trace plus a transient of about 1 GB in the
feed-modulation post-pass (it clones the 3 M-move scallop IR) — the
trace stays O(path length / step) x 280 B; a compact per-sample record
would be the next step if 1.8 GB is still too much for the GUI.

## Outputs

| | live_0925 before | after | small before | after |
|---|---|---|---|---|
| total_runtime_s | 1249.6177822015607 | same bits | 7077.95… | same |
| removed volume mm³ | 72959.4420799361 | 72959.4420799368 | 87095.3646834138 | 87095.3646833984 |
| collisions / rapid collisions | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| peak axial DOC, peak chipload | 8.0, 0.13113… | same | same | same |
| average_engagement | 0.1493394361295542 | 0.1493394361295127 | 0.117991852884 | same to 3e-14 |
| air_cut % of runtime | 45.43 | 43.56 | 82.75 | 82.32 |
| samples | 326,729 | 103,819 | 2,012,349 | 434,143 |

Removed volume and runtimes agree to 1e-13 (summation order). The one
reading that moves is the per-sample classification: a coalesced sample is
air only when its step's mean radial engagement is under 2 %. The repro's
"before" cannot finish under any cap this container allows, so its outputs
have no before column; after: 0 collisions, 0 rapid collisions, runtime
139,788.8 s.

## Pins

- `tests/sim_trace_sample_rate_g_simmem.rs` — on 45° ramps, under Swept,
  WholeToolpath and PerStamp, samples ≤ Σ max(1, ⌈len/step⌉), the clock is
  the path's feed time and the trace's removed volume is the stock's loss.
  Red before: 20,205 samples against a bound of 2,109.
- `tests/sim_peak_memory_g_simmem.rs` (Linux; `/proc/self/clear_refs` +
  `VmHWM`) — a memoised simulation whose snapshot is refused peaks under
  1.5x its retained trace, and the artifact write under 0.25x. Each
  transient re-injected alone turns it red: memo clone 116 MB, `append`
  109 MB, `to_vec_pretty` 210 MB, all against a 53 MB trace.
- `sample_coalesce::tests` — the merge rules, the renumbering, the budget.
- `perf_golden_sim_metrics` re-baselined; the moved fields and why are in
  its doc (counts, air cut, plunge peak radial 1.0 -> 0.136, helix arc).
- `swept_stamping_s1` non-vacuity anchor now counts step samples (288).

## GUI path

Covered: the GUI worker runs the same `run_simulation_memoized` (coalescing
and the memo pre-check are in core), and its artifact write
(`rs_cam_viz/src/compute/worker/execute/mod.rs`) now shares the trace and
streams. Not measured in a running GUI (no display here). The modulation
post-pass `Arc::make_mut` copies the trace if another `Arc` is alive at that
moment; the controller already takes it out first (`controller/events/compute.rs`).
