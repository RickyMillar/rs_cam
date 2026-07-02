# Pencil post-mortem + rest-driven redesign (2026-07-03)

Deep-research synthesis: why three pencil detectors produced fragmented/messy
paths while Fusion 360 handles the same relief cleanly, and the design that
follows. External claims below were web-verified (3-vote adversarial pass on
primary sources); items marked *unverified* had their verification votes
rate-limited and rest on the source fetch only.

## The one-line answer

**All three of our detectors found features of the DESIGN surface. Pencil is
not a design-surface feature — it is a TOOL-INTERACTION feature**: the locus
where the ball contacts the surface at two points at once. Industry and the
canonical literature both define it that way, and both drive *where* pencil
runs from a **reference-tool rest comparison** — precisely the "only use
pencil where I need it, driven by good rest data" goal.

## What commercial CAM actually does (verified, primary sources)

- **Mastercam 3D Pencil** "detects regions where this virtual tool
  simultaneously contacts the surface at exactly two points" — bitangent
  double-contact by virtually positioning the ball, not surface feature
  detection. Its `reference tool diameter` "calculates the areas which this
  tool could not fit into and only generates offsets in these areas."
  (mastercam.dk HSM strategy reference; both claims 3/3 verified)
- **PowerMill Corner Multi-Pencil** takes "contours from the un-machinable
  areas of a previous tool called the Reference tool" — rest-material-driven
  region selection. (Autodesk PowerMill help, 3/3 verified) PowerMill's Rest
  Machining generally runs off either a reference toolpath/tool or an explicit
  3D stock model; PowerMill 2024 merged pencil + corner-finishing + rest
  boundaries into one "rest finishing" strategy (*unverified*).
- **Inventor CAM / Fusion Pencil** exposes the dual-tool input set directly:
  rest tool diameter, previous tool diameter, previous tool corner radius —
  the uncut region is the *difference of two tool offsets*, no full stock
  model needed (*unverified*). FeatureCAM's Corner Remachining likewise derives
  uncut regions purely from the previous-tool diameter (*unverified*).
- Quality knobs across vendors: bitangent/bi-contact angle threshold (the
  significance dial), cusp height, tolerance, steep/shallow split,
  separate-regions vs linked output.

## The canonical academic method (verified)

- **Pencil curve = self-intersection of the tool-radius offset surface** (the
  CL surface folds where the ball double-contacts). Choi & Jerard,
  *Sculptured Surface Machining* (Kluwer 1998), formalize the Z-map height
  field; Park/Choi (KAIST) trace pencil curves by offsetting the *triangulated*
  mesh, computing offset self-intersections, and classifying them by
  **rendering the offset surface and reading visibility off a depth buffer**
  ("virtual digitizing" — Park, Chung, Kim, Choi, Springer; "Pencil curve
  detection from visibility data", CAD 2005).
- **Material-side tracing** (Park et al., CAD 2004, 3/3 verified): trace the
  pencil curve using which side of the CL net still holds material — "smooth
  and clean pencil-cut curves can be generated even if the actual adjacent
  pencil-cut curves are very close", plus an explicit **curve refinement**
  step to fix discontinuity at sharp corners caused by grid coarseness.
- **3D bi-contact angle** (CAD Journal 1(1-4) 2004, 3/3 verified): a pencil
  point is classified by the 3D angle between the two contact normals of the
  bitangent ball — explicitly *better than 2D/point-based criteria*, which
  "have inherent limitations … in identifying sharp-concave points."
- Robustness mechanism: the tool radius is a **morphological low-pass filter**
  (ball-rolling closing, same family as ISO 25178 areal-metrology filters).
  Sub-tool-radius mesh noise physically cannot fold the offset surface, so it
  never becomes a candidate. Detection and reachability are the same
  computation.

## Honest nuance on crest lines (the verifier earned its keep)

Three claims asserting "Yoshizawa 2005 admits crest lines fragment under mesh
noise" were **killed**: refuters fetched the actual paper and found the quotes
fabricated — the paper *claims robustness* to mesh irregularity, achieved via
machinery we never implemented (adjacent-normal cubic derivative estimation,
angle-gated gap-jumping reconnection of fragments, MVS/cyclideness
thresholding). The 2008 CAGD follow-up still had to develop new robustness
fixes (*1-vote verified*). Takeaway: our curvature detector's fragmentation is
partly an incomplete implementation of that lineage — but completing it would
still leave the deeper defect: **crest lines don't know the tool radius**. No
saliency value means "what MY tool needs"; the dial both over- and
under-selects somewhere on the part.

## Post-mortem of our three detectors

| Detector | Locus it finds | Fatal mismatch | Observed |
|---|---|---|---|
| Dihedral | Mesh-edge fold angles | Edge-scale, tool-blind; seam ≠ ball rest point (hence the bisector patch) | 9,277 chains on terrain.stl, fragmented |
| Drainage | Flow-accumulation trunks | Right *machinery* (raster + skeleton → coherent lines), wrong *scalar field* (hydrology ≠ reachability) | Coherent but mislocated; needed 4 bolt-on gates |
| Curvature crest | ∂κ₂/∂t₂ zero-crossings, κ₂<0 | Faithful to its literature, but that literature is feature *visualization*; tool-blind | Saliency 0.05→6,836 lines / 0.8→19; no setting = "tool needs this" |

Every denoise hack we added (DEM box-blur, tensor smoothing, reach-gap gate,
min-length, bisector) was an attempt to recover in design-surface space the
filtering the tool radius gives for free in offset space. The reach-gap gate
was the right idea in the wrong role: it already computes offset-space
reachability (drop the real cutter, measure the float) — but as a *filter* on
noisy candidates instead of as the *detector*.

## The redesign: rest-depth-field pencil (detector #4, the aligned one)

`rest(x, y) = drop_z(reference_tool, x, y) − drop_z(pencil_tool, x, y)` on a
grid (parallel `point_drop_cutter` — the same machinery as the drainage
detector's `build_dem`). This *is* the dual-tool-offset comparison every
vendor uses, evaluated the Z-map way.

1. **Where pencil is needed** = support of `rest > tolerance`. Zero hand-tuned
   gates: the field is zero wherever the reference tool already reached, and
   sub-pencil-radius texture never registers (both drops float equally).
2. **Centerline** = ridge/skeleton of each rest region (Zhang-Suen thinning +
   node-anchored skeleton tracing — already written once in the deleted
   `valley_network.rs`; full text preserved, trivially resurrected. Drainage's
   coherent-line machinery pointed at the right scalar field this time).
3. **Significance dial** = bi-contact angle (or peak rest depth) per region —
   matches Mastercam/PowerMill semantics, replaces `valley_saliency`.
4. **Routing** ("pencil only where I need it"): rest-region width vs pencil
   diameter classifies each region → narrow → pencil centerline (+ offsets);
   wide → adaptive3d `FromRemainingStock` clearing (wired 2026-07-02).
   One field, two strategies.
5. Downstream unchanged: `paths_from_sampled` → fair → lift → offset → link.

### Agent feedback metrics (the "is it good?" question, finally answerable)

The rest field is the ground truth the previous attempts never had:
- **Residual rest volume**: Σ max(0, rest_after) vs before — THE pass/fail.
- **Coverage**: % of rest-region skeleton length traced within tolerance.
- **Fragmentation**: chains per region, rapid:cutting ratio.
- **Smoothness**: existing accel/jerk metrics.
Vendor-equivalent knobs for tuning: cusp height, tolerance, separate-regions.

### Cost note

The grid is ~160k drops at 0.5 mm on wanaka (two tools ≈ 320k) — same order
as the drainage DEM build, parallelized; well under the pencil's current lift
cost. Fusion-class performance is not required to beat detectors #1–#3.

## Sources (primary)

- Mastercam 3D Pencil strategy reference (mastercam.dk/docs/hsmpp/Strategy3DPencil.html)
- PowerMill Corner Multi-Pencil / Corner Pencil / Model Rest Area Clearance (Autodesk help, 2019/2020)
- Inventor CAM Pencil; FeatureCAM Corner Remachining (Autodesk help)
- Fusion 360 Pencil strategy help (3D-PENCIL-READ)
- Choi & Jerard, *Sculptured Surface Machining* (Kluwer 1998)
- Park, Chung, Kim, Choi, "Pencil Curve Tracing via Virtual Digitizing" (Springer)
- Park et al., "Material side tracing and curve refinement for pencil-cut machining of complex polyhedral models" (CAD, 2004)
- "A Curve-Based Approach for Clean-up Machining" (CAD Journal 1(1-4), 2004)
- S.C. Park, "Pencil curve detection from visibility data" (CAD, 2005)
- Yoshizawa, Belyaev, Seidel SPM 2005; Yoshizawa et al. CAGD 2008 (crest lines)
