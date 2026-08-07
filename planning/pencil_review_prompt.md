# Post-compact prompt — pencil finishing: full literature gap review

Do a thorough review of rs_cam's pencil-finishing implementation against the academic
literature on pencil / valley / crease / internal-corner-cleanup milling, and produce a
**prioritized gap report**. Context: we rebuilt pencil this session — it was tracing
useless fuzz because the concavity test was ~46% wrong (coin-flip); it now traces real
valley creases and is ~4× faster, but has known rough edges. I want to know what MAJOR
gaps remain versus how pencil/valley milling is *properly* done — especially correctness
gaps (would cut wrong), not just cosmetics.

## Read first
- `crates/rs_cam_core/src/pencil.rs` — the full implementation (detection, reach-gap gate,
  chaining, lift/positioning, offset passes, ordering, the #[ignore] render harness).
- `crates/rs_cam_core/src/mesh.rs` — `build_auto` (density-aware index) + `SpatialIndex`,
  and the drop-cutter (`dropcutter.rs`, `tool/ball.rs` facet/edge/vertex drop).
- `research/multitool_finishing_optimization.md` — citations already gathered: Park &
  Chung 2006 (pencil-curve from visibility / offset self-intersection), Ren/Zhu/Lee 2005
  (material-side tracing for pencil-cut on meshes), Elber & Cohen (curvature-region tool
  selection), Suresh & Yang (iso-scallop), Lin & Koren 1996, Lasemi/Xue/Gu 2010 survey.
- `planning/finishing_speedup_project.md` §Phase 2 — what we built + Known limitations.

## What our implementation currently does (verify each against the literature)
1. **Detection:** concave mesh edges via dihedral-angle threshold (`bitangency_angle`) +
   a geometric apex concavity test `is_concave = (apexB − edgePt)·nA > 0`. Then a
   tool-radius-aware **reach-gap gate**: drop the actual tool, keep where it bridges by
   `gap = cl.z − surf_z > min_valley_depth` (gated per *chain*, sampling ~8 verts, median).
2. **Chaining:** graph walk of concave edges, break chains at junctions (degree ≠ 2),
   filter by `min_cut_length`.
3. **Positioning:** `lift_to_surface` drops the tool straight down at the seam X,Y and
   keeps that X,Y — **no bisector / CL-surface offset** (known gap for asymmetric L-corners
   like vertical lake walls).
4. **Linking:** `hookup_distance` is declared but DEAD → per-fragment retract-to-safe-Z →
   ~76 m of rapids on the test relief.
5. Offset passes; nearest-neighbour TSP ordering; output is jagged (follows 0.5 mm edges).

## Already-found issues — confirm + rate severity, don't just re-discover
- Bisector/CL-surface tool positioning missing (asymmetric corners).
- Thick concave bands fragment into messy chains (no thinning / skeletonization / ridge-
  valley line extraction).
- Jagged polylines (no smoothing).
- Dead `hookup_distance` linking.

## Questions to answer
- How is pencil/valley milling *properly* formulated (CL-surface self-intersection,
  bitangent contact, material-side tracing)? Where does our mesh-dihedral + drop-down
  approach diverge, and how much does each divergence matter?
- Is mesh-dihedral detection fundamentally sound, or should detection be curvature- /
  offset-surface-based to avoid thick noisy bands?
- **Correctness gaps** (not polish): gouging, wrong tool positioning in asymmetric corners,
  missed or false valleys, any 3-axis vs multi-axis assumptions baked in wrongly.
- Right way to collapse thick concave bands to one clean centerline (medial axis /
  skeletonization / ridge-valley extraction)?
- Determinism and scalability on dense meshes (661k-tri terrain is our test case).
- Does our reach-gap depth gate match how rest/pencil regions are defined in the
  literature, or is it an ad-hoc proxy?

## Method
Use deep-research / parallel sub-agents to pull and verify the literature, then a
code-review pass mapping each finding to our exact code (file:line). Adversarially verify
claims before asserting them.

## Output
A prioritized gap report. For each gap: **what the literature says → what we do →
severity (CORRECTNESS bug vs quality/polish) → effort → recommendation.** Lead with any
correctness bugs that would cut the wrong thing. End with a recommended sequence of fixes.
