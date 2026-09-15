# Tech debt register

**Living document. Append, do not date-stamp and archive.**

`TECH_DEBT_AUDIT.md` (2026-04-11) and `TECH_DEBT_REVIEW_2026-06-10.md` are
point-in-time audits — evidence of what was measured then, per
`planning/CLAUDE.md`. This file is different: an open register of debt found
while doing other work, added to as it is found.

**The common thread: every entry here is something no gate can fail on.**
Clippy is green, the tests pass, and the defect persists. That is what makes
them worth writing down — anything the compiler or a sentry catches does not
need a register.

| ID | What | Status |
|---|---|---|
| T-1 | `enumerate_matching_rows` is dead; `pub` hides it | open |
| T-2 | LH-1's guard is syntactic and a closure defeats it | open |
| T-3 | Cross-crate sentries never run in a per-crate gate | open |
| T-4 | `predict_peak_deflection_um` returns `0.0` for every refusal | open |
| T-5 | `feeds/mod.rs` 4 144 lines, `suggest.rs` 5 667 | open |
| T-6 | Two implementations of one physical model | partly closed |

---

## T-1 — a public function with no callers, and no warning

`crates/rs_cam_core/src/feeds/vendor_lookup.rs:315`
`enumerate_matching_rows` has **zero callers** across `crates/` since D-1
(commit `b68c484d`) deleted the sibling-row scan. The only remaining mention
in the repository is a historical note in a comment.

**Why nothing catches it:** it is `pub` in a library crate, so it is part of
the public API and `dead_code` does not fire. The compiler cannot know a
library's callers.

**Cost if left:** this is exactly how the feeds area reached 28 800 lines. A
`pub` function that nothing calls reads as load-bearing to the next person,
who works around it rather than deleting it.

**Fix:** delete it, or demote it to `pub(crate)` and let the compiler decide.
Deliberately left in place during D-1 to keep that change behaviour-neutral.

---

## T-2 — a sentry that checks for a string, not for the thing

`crates/rs_cam_core/tests/air_cut_denominators_lh1.rs` asserts

```rust
!src.contains("air_cut_time_s / ")
```

to stop a surface dividing air-cut time by hand instead of calling
`AirCutRatios::air_cut_pct_of_total_runtime()`.

**It does not work.** Moving the division into a closure applied to the field
— `pct_of_total(s.air_cut_time_s)`, where the closure divides — contains no
such text. I introduced exactly the defect the sentry exists to prevent, in
commit `790b033c`, and it went unnoticed for a day. Fixed in `41d0ee5c`.

There is a second edge, found while fixing it: the sentry scans raw source
including comments, so a comment that *explains* the trap by quoting the
pattern also fails the test.

**Cost if left:** the guard reads as protection and is not. A negative
source-scan that a one-line refactor evades is worse than no guard, because
nobody investigates a green test.

**Fix:** check semantically — that the call to the named ratio is present —
rather than that one spelling of its absence is missing. Or lint on the AST.

---

## T-3 — core tests scan viz sources, and a per-crate gate never runs them

Ten tests under `crates/rs_cam_core/tests/` read files under
`crates/rs_cam_viz/src/`, asserting the GUI names its denominators, routes
through the apply funnel, and so on. They are correct and valuable.

**`cargo test -p rs_cam_viz` does not run them.** A GUI change can break a
core test, and the obvious gate for a GUI change will not say so. That is how
T-2 survived a day: the declutter work ran the viz suite repeatedly and never
the core one.

**Cost if left:** a whole class of cross-crate invariant is only enforced by
a gate nobody runs after a UI change.

**Fix:** no clean one — the test must live where it can `include_str!` both
crates. Mitigation: list the scanned viz paths somewhere a viz developer will
see, or move these to a workspace-level test target that both gates run.

---

## T-4 — a sentinel where a `Result` belongs

`crates/rs_cam_core/src/feeds/predict.rs:129`
`predict_peak_deflection_um` returns `DeflectionPrediction { predicted_um }`
and, by its documented contract, reports **every refusal as `0.0`** — drill
operations, missing Kc, no stickout, no DPP. A genuine zero deflection and
"this model does not apply" are indistinguishable to every caller.

**Why nothing catches it:** the type is honest about its shape and silent
about its meaning. Callers compile fine either way.

**Cost if left:** each caller invents its own interpretation. `feeds::
efficiency` maps `0.0` to `None` and abstains, which is conservative and
correct — but that is a decision made at the wrong layer, and the next
caller may just as reasonably publish "100 % headroom".

**Fix:** `Result<DeflectionPrediction, UnmodelledReason>`, matching the
refusal vocabulary `tool_load` already uses.

---

## T-5 — two files where a reader looks first

`crates/rs_cam_core/src/feeds/mod.rs` is 4 144 lines with 29 top-level
items: the `calculate` pipeline, the rubbing-floor policy and the domain's
shared types. `crates/rs_cam_core/src/feeds/suggest.rs` is 5 667.

**Cost if left:** they are the least navigable files in the area a reader
enters first. Nothing is broken; everything is slower.

**Fix:** its own package. Explicitly out of scope for the cut-efficiency
programme (`load_model_2026-09-16/REVIEW.md` D-4) so it does not ride along
with unrelated work.

---

## T-6 — the same physics implemented twice

The recurring structural defect in this area, and the one that motivated the
2026-09-16 programme.

- **`force.rs` vs `power.rs`** — an affine, chip-thickness-aware force model
  and a constant-specific-energy power model, disagreeing by 2–4× in the
  regime wood routing actually runs in. R1 addresses this.
- **`feed_modulation` held its own copy** of the deflection inversion that
  `force.rs` should own. Closed by N-2 (`ad89f8be`): one implementation,
  two views.

**Why nothing catches it:** both copies compile, both are tested, and each
test is consistent with its own copy. Divergence is only visible to someone
comparing them deliberately.

**Fix pattern that worked:** extract to the module that owns the model and
have the second caller consume it, rather than writing a third copy. Keep a
test that pins the extraction as behaviour-neutral — for N-2, an existing
test that had to pass **byte-identical and unmodified**.

---

## How to add an entry

State what no gate can fail on, why the compiler or the suite cannot see it,
and what it costs if left. If a gate *could* catch it, fix the gate instead
and record that here rather than the symptom.
