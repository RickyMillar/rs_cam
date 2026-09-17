# Kickoff prompt — pencil watershed-spine (resume after compaction)

> Paste this to start the evidence phase. It is self-contained: read the
> two docs it names and you have the full context without the prior chat.

Take up the pencil watershed-spine experiment, chartered at
`planning/pencil_watershed_spine_2026-09-03/CHARTER.md` with
pre-registered bars in that folder's `FINDINGS.md`. Read both first,
then the memory `[[project-entry-moves]]` sibling and
`[[project-valley-tracing]]`. Follow the repo's CLAUDE.md (session
workflow, single cargo lane, lint/fmt gates, evidence-commit style,
render-before-ruling).

**The question (do not re-open the framing):** pencil's CRITERION is
the dual-tool rest comparison — settled by
`planning/pencil_postmortem_and_rest_driven_design.md`; watershed is
NOT a new detector. The experiment is one level down: does D8
flow-accumulation on the REST FIELD extract better pencil centerlines
than the current NMS+hysteresis+Zhang-Suen skeleton inside
`rest_depth_arm` (`crates/rs_cam_core/src/rest_field.rs` §"Ridge
extraction", called from `crates/rs_cam_core/src/pencil.rs:1955`)?
"Better" = the six pre-registered metrics; the bars are fixed, and a
TIE is an adopt-nothing outcome (baseline coverage is already 0.80 from
`0db602e`, not the old hairball 0.137).

**Start at P0, in order:**

1. **P0 — promote flow-accumulation to a library module.** The
   hydrology is inline in three test files
   (`priority_flood_epsilon`, `d8_receivers`, `d8_accumulation` in
   `crates/rs_cam_core/tests/catchment_basin_census_w0.rs`,
   `valley_prize_census_h0.rs`, `valley_branch_falsifier_h1.rs`).
   Pull the shared primitives into `crates/rs_cam_core/src/flow_accum.rs`,
   switch the three test copies to consume it, keep every Track H test
   green byte-for-byte. Lint/fmt clean. Commit (own files only). This
   pays down the copy-paste debt regardless of the result.

2. **P1 — the A/B instrument** (`#[ignore]`, committed on lint-clean).
   Compute the rest field ONCE via `detect_rest_valleys`'s machinery,
   then run BOTH spine extractors on the SAME field — A = the current
   ridge pipeline, B = flow-accumulation trunks — on TWO fixtures: (a)
   wanaka200 (organic terrain — NOT a spacing claim, so the facet rule
   does not bar it) and (b) a synthetic `height_field` with a KNOWN
   Y-valley network + a broad shallow basin that MUST stay untraced.
   Report M1–M6 from `FINDINGS.md`. Render both spine sets
   (hillshade overlay) before any ruling — a number the picture
   contradicts is void.

3. **P2 — ruling.** Apply the decision rule in `FINDINGS.md` verbatim.
   Adopt only if B1–B6 all pass; then add an OPT-IN
   `PencilSpineExtractor` on the rest arm (default unchanged).
   Promote-to-default is a separate later A/B.

**Key priors (do not re-derive):**

- The shared pencil pipeline already rest-gates every arm, so
  flow-accumulation is rest-gated by construction — this is what keeps
  the idea clear of the `c73b38c3` drainage-deletion ("hydrology
  trunks, needed gates").
- Track H refuted watershed as a FULL-SURFACE strategy vs raster; that
  does NOT bind a sparse rest-gated pencil pass. The one carried
  lesson: keep pencil's offset-pass count width-gated (it already is).
- The rest field is a 2.5D drop-cutter DEM — flow accumulation applies
  natively, no new projection; it inherits (does not add) the "no
  undercut / no non-drainage seam" limit.
- The synthetic fixture's Y-junction is the crux: A is EXPECTED to
  shred at the degree-3 node (its `0db602e` failure), B to stay
  continuous. If A does not shred there, record that the premise is
  weaker than claimed and re-weigh before P2.
- Cost with `relink_and_cost_under` under the machined-stock
  `link_ceiling` regime (the finishing rig), not raw length.

**Stop-and-report** if P1 cannot tap the rest field before extraction
without a refactor larger than the experiment. Do not touch the closed
Track H files beyond the P0 switch, and do not resurrect the refuted V1
falsifier's inline centerline code.
