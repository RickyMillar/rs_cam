# Architectural review of the plan

Question asked: does `SPEC.md` land somewhere architecturally sound, is
anything unnecessary removed, and is related code in one findable place?

Short answer: **the plan's core work is sound and actively consolidating.
The viz work is neutral. But it would be built on top of four things that
should be removed or moved first**, and one of them is dead code on a
per-frame path that this morning's chart deletion created.

---

## The plan itself

### What is right

**N-2 is the best thing in the plan.** Extracting
`chipload_cap_for_deflection` out of `feed_modulation` *removes* a
duplicate rather than adding one. The closed-form inversion exists in
exactly one place afterwards, and `feed_modulation`, the deflection gate
and the new chart all read it. This repo's recurring defect is two copies
of one model drifting apart — `force.rs` versus `power.rs` is the live
example — so a plan whose first step deletes a second copy is pointed the
right way.

**N-1 is in the right crate and the right module.** Core computes the
verdict, the UI renders it. That follows `rs_cam_viz/CLAUDE.md` ("GUI state
is not an alternate data model") and the `SimulationTriage` precedent of one
bounded typed answer.

**Phase C adds no core surface at all** — `ApplyScope::Speeds` already
exists. A phase that ships product value with zero new core API is the
cheapest kind.

### What the plan does not do, and should

It edits `explain.rs`'s consumers without noticing that `explain.rs` is now
partly dead (D-1 below), and it adds to `ui/feeds/` without noticing the
directory no longer describes its contents (D-3). Neither is fatal; both are
cheaper to fix before than after.

---

## D-1. Dead compute on a per-frame path — **remove before Phase A**

`FeedsPreview::new` → `explain_feeds()` → `explain.rs:192`:

```rust
let siblings = enumerate_matching_rows(lut, &family_query);
```

That scores, builds and sorts **256 vendor observations**, every time a
`FeedsPreview` is constructed — which the Feeds tab does **every frame**.

The result is `FeedsExplain::sibling_rows`, read by:

| Reader | Count |
|---|---|
| `rows_by_diameter()` | chart A — **deleted today** |
| `rows_by_hardness()` | chart B — **deleted today** |
| anything else, anywhere in `crates/` | **zero** |

Both accessors now have no callers outside `explain.rs`'s own unit tests,
and `sibling_rows` has no direct reader at all. `enumerate_matching_rows`
has exactly one live caller — this one.

**Remove:** `sibling_rows`, `rows_by_diameter`, `rows_by_hardness`, the
`family_query` widening, the `enumerate_matching_rows` call and the two unit
tests that exercise the accessors. Then re-check whether
`enumerate_matching_rows` itself still has a caller.

This is the cleanest deletion available in the whole area: it removes a
field, two methods, a per-frame full-table scan, and a paragraph of module
documentation that describes charts that no longer exist.

## D-2. Two modules one letter apart — **rename, low churn**

| File | Type | Job |
|---|---|---|
| `feeds/explain.rs` | `FeedsExplain` | the UI data contract |
| `feeds/explanation.rs` | `FeedExplanation` | the stage-labelled feed record |

Both are alive (`FeedExplanation` has 13 source uses). `FeedsExplain` and
`FeedExplanation` differ by one letter and a plural, and sit in files whose
names differ by three letters. That is a findability trap, and the kind that
costs an afternoon rather than a minute.

**Rename the files only** — `explain.rs` → `explain_payload.rs`,
`explanation.rs` → `feed_explanation.rs` — and give each a one-line header
pointing at the other. Renaming the *types* would be correct and is much
more expensive; record it as optional and do not bundle it here.

## D-3. `ui/feeds/` is named after a window; half of it is the inspector

| File | Lines | Drawn from |
|---|---|---|
| `mod.rs` | 130 | **is** the Explore window, and the module root |
| `explore.rs` | 1039 | the window |
| `compare.rs` | 681 | `ui/properties/mod.rs` |
| `why.rs` | 533 | `ui/properties/mod.rs` |
| `shared.rs` | 388 | window **and** inspector **and** `readiness_panel.rs` |

**1 214 of 2 771 lines are inspector content** reached only from the
properties panel. A reader looking for the comparison card will not look
in a directory whose `mod.rs` opens by declaring itself a window.

**Do not split the directory.** `shared.rs` has three consumers — the third
is the Readiness rollup — so moving the inspector half into
`ui/properties/feeds/` would either duplicate `shared.rs` or leave a
cross-directory dependency that is worse than today.

**Do this instead, and it is small:** move the window's `draw()` from
`mod.rs` into `ui/feeds/window.rs`, and reduce `mod.rs` to a module root
whose doc comment is a map — window here, inspector card there, shared
helpers there, and who consumes each. The directory then honestly reads as
*the feeds domain*, which is what it is.

## D-4. `feeds/mod.rs` is 4 144 lines — **note only, out of scope**

Twenty-nine top-level items: the `calculate` pipeline, the rubbing-floor
policy, and the domain's shared types, in the file a reader opens first. It
is the least navigable file in the area. Splitting it is a package of its
own and must not ride along with this one.

---

## Verdict on the end state

With D-1 to D-3 done first, the plan lands here:

```
core/feeds/
  mod.rs               pipeline + policy + types        (D-4, still large)
  explain_payload.rs   FeedsExplain — the UI contract    (D-1 slimmer)
  feed_explanation.rs  FeedExplanation — the stage record
  force.rs             THE force model: affine coeffs, immersion,
                       chipload_cap_for_deflection        (N-2 consolidates)
  efficiency.rs        CutEfficiency — the bounded answer (N-1, new)
  predict.rs           pre-sim deflection
  suggest.rs           the apply funnel + ApplyScope

viz/ui/feeds/
  mod.rs               module root + map                 (D-3)
  window.rs            the Explore window                (D-3)
  explore.rs           the nomogram
  compare.rs           the inspector card
  why.rs               per-row sentences
  shared.rs            helpers for all three surfaces
```

One force model, in `force.rs`, with one inversion. One efficiency answer,
in `efficiency.rs`. One apply funnel. Two explain modules that can be told
apart. A UI directory that says what it holds.

That is a better resting place than the plan alone would reach, and the
delta is: one deletion, two file renames, one function move.

**What it does not fix:** `feeds/mod.rs` at 4 144 lines, `suggest.rs` at
5 667, and the power/force divergence itself — which is R1, deliberately
sequenced separately because it moves recommended numbers.

---

## Revised order

```
D-1  delete sibling_rows and the two accessors    ← do first; Phase A edits its consumers
D-2  rename the two explain files
D-3  ui/feeds/window.rs + mod.rs becomes a map
N-2  extract chipload_cap_for_deflection
N-1  feeds::efficiency
A    verdict row
B    corridor
C    Match vendor chipload
─────
R1   power on the affine model  (authorised separately)
D    aggressiveness dial        (re-evaluate)
```

D-1 to D-3 are all removals or moves. None changes behaviour, all are
covered by the existing gates, and together they take less time than
Phase A.
