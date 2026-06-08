# Optimizer redesign — ASCII mockups

These demonstrate the target structure in `optimizer.md`. UI wins, not prose:
the affordances carry the meaning. Finding ids in brackets.

---

## A. Project rollup — Ready state (the main redesign)

```
┌─ Optimize project ──────────────────────────────────────────────────────┐
│                                                                          │
│  Current  4:18.0      Optimized  3:51.4   (-26.6s, -10%)    [OPT-001]    │
│  baseline: sim · run #47 · 2 min ago                        [OPT-003]    │
│  + 1 toolpath not estimated (skipped)                       [OPT-001]    │
│                                                                          │
│  ⚠ Bottleneck: roughing_adaptive  (38% of runtime)                       │
│  ──────────────────────────────────────────────────────────────────────│
│                                                                          │
│  APPLY NOW                                                    [OPT-004]   │
│  Apply  toolpath            change            −cycle   verdict           │
│  [✓]    roughing_adaptive   feed 3150, DOC 3  -18.2s    ✓                │
│  [✓]    pocket_lid          feed 2400         -6.1s     ✓                │
│  [ ]    profile_outer       rpm 18000         -2.3s     ⚠   (gate near)  │
│                                                                          │
│  NEEDS YOUR CALL                                          [OPT-002]      │
│         toolpath            note                          action         │
│  trade-off    finish_floor  2 faster, gate regression    [ Review ▸ ]    │
│  verify scrap chamfer_edge  1 inside tolerance band       [ Review ▸ ]    │
│                                                                          │
│  ▸ Not optimized (2)                                      [OPT-004]      │
│                                                                          │
│  ────────────────────────────────────────────────────────────────────  │
│                                  [ Apply selected ]   [ Close ]          │
└──────────────────────────────────────────────────────────────────────┘
```

Key changes vs today:
- header savings is computed over real baselines, not `0.0` for refused rows;
  a `+N not estimated` note replaces the silent understatement [OPT-001].
- a `baseline:` provenance line; if stale it flips amber (see C) [OPT-003].
- three role sections, each with a real header and ONE consistent affordance —
  `Apply` column is checkbox-only, `verdict` is glyph-only, NEEDS-YOUR-CALL
  rows have a role chip + Review button, refused rows are collapsed [OPT-004].
- `[ Review ▸ ]` opens the per-toolpath modal for that exact row, killing the
  "you must hunt for it" dead end [OPT-002].

### Expanded "Not optimized" disclosure

```
│  ▾ Not optimized (2)                                                     │
│      toolpath          reason                                            │
│      v_carve_text   ⚠  no safe gain — feed pinned by chip-load  ▸ details│
│      lettering      ·  skipped: custom material, no LUT row     ▸ details│
```

`▸ details` expands the full narrative + "tried N candidates" — no more
70-char inline truncation [OPT-004].

---

## B. Header — stale baseline state [OPT-003]

```
│  Current  4:18.0      Optimized  3:51.4   (-26.6s, -10%)                 │
│  ⚠ baseline from a stale sim — params changed   [ Re-sim & reopen ]      │
```

The staleness signal sim-diagnostics already owns (`sim.is_stale`) is now
visible on the optimizer itself, with a one-click recovery. Same chip is added
to the per-toolpath modal header.

---

## C. Per-toolpath modal — suggestions become actionable [OPT-005]

Before (prose the user re-types):

```
┌ Try this ─────────────────────────────────────────┐
│ • Cap feed at ~2961 mm/min and re-optimize.        │
│ • Raise RPM above ~16000 rpm and re-optimize.      │
└────────────────────────────────────────────────────┘
```

After (each suggestion is an affordance — explicit click, never auto-applied):

```
┌ Try this ─────────────────────────────────────────────────────┐
│  Cap feed → 2961 mm/min          [ Apply & re-optimize ]        │
│  Raise RPM → 16000 rpm           [ Apply & re-optimize ]        │
│  ⓘ Data gap: no LUT row for this material/hardness             │
└────────────────────────────────────────────────────────────────┘
```

`CapAxisAt`/`RaiseAxisAbove` carry the axis + value, so the button sets that
bound and re-runs the search. `DataGapHere` has no value to apply → stays a note.

---

## D. In-context entry — always present when a sim exists [OPT-006]

Over-budget (unchanged):

```
│  ⚡ Optimize 3 exceeding toolpaths                                       │
```

Within-gate (new — was hidden by `if bad > 0`):

```
│  ⚡ Optimize project for cycle time          (secondary styling)         │
```

Menu item, disabled, now self-explaining:

```
  Toolpath ▾
   ├ …
   └ Optimize project…        (greyed)
       └─hover─▶ "Run a simulation first — the optimizer needs a
                  baseline cut trace."
```
