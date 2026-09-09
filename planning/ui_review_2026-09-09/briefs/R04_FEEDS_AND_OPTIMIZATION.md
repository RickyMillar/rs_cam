# R04 — Recommendations, manual control and improvement decisions

Read `../PROTOCOL.md`, `../TOOLING.md`, `../FIXTURES.md`. Execute only R04.
Write `../results/R04/{REPORT,trace}.md` and evidence. Source paths are repo-relative.

## Outcome

The user can choose feeds/cut settings, understand the basis and uncertainty of
a recommendation, predict exactly what Apply changes and judge an improvement
without confusing a faster estimate with a better or safer finished job.

## Task cards

1. **A sensible starting recipe:** F1/F1m or F3 with a suitable tool. Ask “Choose
   settings you would be willing to verify.” Observe which machine, material,
   tool and cutting assumptions the person checks, whether inherited RPM and
   overrides are understood and whether recommendation rationale is findable.
2. **Predict before Apply:** compare speed-only apply, cut-geometry apply,
   recommended linking/dressups, a what-if exploration and optimizer apply.
   Inventory which routes actually exist first. Before each, ask which fields
   will change. Capture a full before/after diff, provenance and invalidation.
   Include cancel, revert and undo; preview must not be treated as acceptance.
3. **Weak evidence:** inspect a no-vendor-row or sub-small-diameter case and a
   tool/operation mismatch. Verify the actual reason. Ask whether the user can
   distinguish a vendor observation, repo-derived scaling, formula fallback,
   clamped recommendation, inherited default and manually typed value.
4. **Improve a verified job:** use a small simulated baseline. Find an optimizer
   from a diagnosis, inspect candidates and their scope, apply one, regenerate
   and re-simulate. What changed, what evidence is stale, and did the predicted
   benefit survive? Do one bounded search, not a tuning campaign.
5. **Compare two approaches:** V10; duplicate a small finishing operation or
   compare a feed variant. Require the user to explain time, finish/detail,
   tool changes and confidence before declaring a winner. Ask how they retain
   a baseline and return to it. If comparison requires manual bookkeeping,
   record the actual effort rather than assuming a comparison tool exists.
6. **Project-wide action:** inspect project optimizer/recommendation scope on a
   mixed job with one disabled and one unmeasured/drill operation. Predict which
   operations will be changed and which cannot be evaluated.

## Specific questions

- Can a normal user accept a useful recipe without becoming a feeds researcher,
  while an expert can still inspect evidence and override a field intentionally?
- Are values labelled by actual origin or by a newly computed recommendation
  that did not produce the stored value? Is provenance still true after editing?
- Are incompatible constraints and missing guarantees explained at point of use?
- Can the user distinguish planned versus emitted feeds and time estimates?
- Does “Optimize” imply an inappropriate safety or optimality guarantee?
- Does the app make a fair comparison possible, including delivered finish,
  operation scope, material/tool differences and simulation resolution?

## Source anchors

- `crates/rs_cam_viz/src/ui/properties/mod.rs:1947-2147`: speed/cut apply split;
  `draw_operating_point`, advance-per-tooth card and vendor evidence viewer.
- `crates/rs_cam_viz/src/ui/feeds_modal.rs`, `optimize_modal.rs`, `optimize_project.rs`.
- `crates/rs_cam_viz/src/ui/components/{provenance,precedence,suggest}.rs`.
- `crates/rs_cam_viz/src/ui/mod.rs:34-51`: removed per-field-apply contract.
- `crates/rs_cam_viz/tests/apply_contract_a3.rs`; core `feeds::suggest::apply`.
- Current `CLAUDE.md` metric/provenance caveats; prior IA targets are historical,
  not proof that their proposed per-field buttons remain shipped.

## Deliverable additions and boundary

Produce an **action → fields changed → evidence source → invalidated work → undo**
matrix and a worked comparison decision. Include the fastest expert route worth
preserving. Separate UI problems from missing/uncertain physical models.

R03 owns operation geometry hierarchy; R05 owns multi-tool territory planning;
R06 owns diagnostic meanings. Hand uncertain numerical discrepancies to an
engineering investigation rather than silently changing constants or gates.
