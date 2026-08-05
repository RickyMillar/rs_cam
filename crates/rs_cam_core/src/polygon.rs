//! 2D polygon types and offset operations.
//!
//! Internal representation uses nalgebra P2. Converts to geo-types and
//! cavalier_contours at operation boundaries per architecture rules.
//!
//! # Failure contract
//!
//! Ruled at Checkpoint C (2026-08-04), Q1 shape B. An offset that returns no
//! geometry has three possible causes and they are not interchangeable:
//!
//! | cause | reported as | what a consumer should do |
//! |---|---|---|
//! | genuine geometric collapse | `(empty, None)` | proceed — this is the expected outcome of offsetting a shape past its own width |
//! | this module's own guard refused the input | `(empty, Some(`[`OffsetFailure::RejectedInput`]`))` | do not treat as a collapse |
//! | `cavalier_contours` panicked, and was contained | `(_, Some(`[`OffsetFailure::LibraryFailure`]`))` | do not treat as a collapse |
//!
//! [`offset_polygon`] and [`OffsetRingSet::offset`] keep the old
//! `Vec<Polygon2>` / `Self` shape and drop the channel;
//! [`offset_polygon_reported`] and [`OffsetRingSet::offset_reported`] keep
//! it. That split is deliberate: fifteen call sites use the result as pure
//! geometry (containment, masking, a largest-by-area pick) and pay no churn,
//! while the sites where the difference has a cost opt in.
//!
//! The cost is not hypothetical. At the boundary layer an empty offset does
//! not shrink a containment to nothing — it removes the containment
//! **entirely** (`crate::boundary`), which is a correct pass-through when the
//! tool is larger than the stock and an unbounded over-cut when a dependency
//! assertion fired. See `planning/review_2026-08-04/CAVALIER_SHAPE_FAILURE.md`
//! and `ADVERSARIAL_2D_FINDINGS.md` F-1/F-2/F-12.
//!
//! ## Debug versus release — the contract is not build-invariant
//!
//! **Accepted and documented at Checkpoint C, Q4 option (a). Not fixed, and
//! not measured.** Three of the panic classes the containment above catches
//! are `debug_assert!`s inside `cavalier_contours` and its transitive
//! dependency `static_aabb2d_index`. In a release build they do not fire,
//! nothing unwinds, there is nothing for `catch_unwind` to contain, and the
//! library proceeds on the input the assertion was there to reject:
//!
//! | class | site | release behaviour, as documented by the source — NOT verified by running |
//! |---|---|---|
//! | slice stitching (F-3) | `pline_view.rs:507` — *"start index should be less than or equal to end index"* | proceeds with `start_index > end_index`, stitching a malformed slice into a ring |
//! | spatial index (F-11) | `static_aabb2d_index.rs:266` — `min_x <= max_x` | builds a corrupt index and offsets on it. The library's own doc at `:255-258` says an invalid box "may lead to a panic **or unexpected behaviour**" |
//! | zero-length arc (F-12) | `pline_seg.rs:33` — *"v1 must not be on top of v2"* | divides by a zero chord length, returning a NaN radius and arc centre into the offset geometry. Needs no malformed input at all — a valid 24-lobe rosette reaches it |
//!
//! Four consequences a reader has to carry:
//!
//! 1. **`ToolpathStats::offset_library_failures` is not comparable across
//!    builds.** A release run legitimately reports fewer, and a lower number
//!    there is not an improvement — it is the same input going unchecked.
//! 2. The sites that *do* panic in release are different ones and the
//!    containment still earns its keep against them:
//!    `shape_algorithms/mod.rs:786` and `pline_offset.rs:1401`
//!    (`unreachable!("loop_count exceeded max_loop_count …")`), the hard
//!    `assert!`s at `pline_view.rs:316`/`:374`/`:438`, and the `unwrap` /
//!    `expect` / raw-indexing sites in `CAVALIER_SHAPE_FAILURE.md` §3.3.
//! 3. `Cargo.toml`'s `panic = "unwind"` in `[profile.release]` is what keeps
//!    every `catch_unwind` here from being dead code. Changing it to `abort`
//!    turns a contained offset failure into a killed process.
//! 4. **No release evidence exists either way.** Every measurement behind this
//!    contract is a debug measurement (programme rule 11 forbids release
//!    builds in an implementation wave), and the failure arms of
//!    `tests/boundary_clip_escape_f1.rs`,
//!    `tests/skipped_boundary_offset_f8.rs` and
//!    `tests/cavalier_shape_failure_r2.rs` are `cfg!(debug_assertions)`-gated
//!    so they say so rather than asserting something false in release. The
//!    probe that would settle it is item 8 of §4.4 in
//!    `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md`, owned
//!    by the Checkpoint G live-validation wave.

use crate::geo::P2;
use cavalier_contours::polyline::{PlineCreation, PlineSource, PlineSourceMut, Polyline};

/// A closed 2D polygon with optional holes.
///
/// - `exterior`: outer boundary vertices in CCW order (positive area)
/// - `holes`: inner boundary vertices in CW order (negative area)
///
/// Vertices are not duplicated (last implicitly connects back to first for closed polygons).
#[derive(Debug, Clone)]
pub struct Polygon2 {
    pub exterior: Vec<P2>,
    pub holes: Vec<Vec<P2>>,
    /// If true (default), the last vertex connects back to the first.
    /// Open paths (e.g. rivers, contour lines) set this to false.
    pub closed: bool,
}

impl Polygon2 {
    /// Create a closed polygon from exterior vertices (CCW winding assumed).
    pub fn new(exterior: Vec<P2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
            closed: true,
        }
    }

    /// Create an open path (not closed — no edge from last to first vertex).
    pub fn open_path(exterior: Vec<P2>) -> Self {
        Self {
            exterior,
            holes: Vec::new(),
            closed: false,
        }
    }

    /// Create a polygon with holes.
    pub fn with_holes(exterior: Vec<P2>, holes: Vec<Vec<P2>>) -> Self {
        Self {
            exterior,
            holes,
            closed: true,
        }
    }

    /// Create a rectangle from bounds.
    pub fn rectangle(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Self {
        // CCW winding
        Self::new(vec![
            P2::new(x_min, y_min),
            P2::new(x_max, y_min),
            P2::new(x_max, y_max),
            P2::new(x_min, y_max),
        ])
    }

    /// Signed area via shoelace formula. Positive for CCW, negative for CW.
    pub fn signed_area(&self) -> f64 {
        shoelace_area(&self.exterior)
    }

    /// Absolute area (exterior minus holes).
    pub fn area(&self) -> f64 {
        let ext = self.signed_area().abs();
        let holes: f64 = self.holes.iter().map(|h| shoelace_area(h).abs()).sum();
        ext - holes
    }

    /// Ensure exterior is CCW and holes are CW.
    pub fn ensure_winding(&mut self) {
        if shoelace_area(&self.exterior) < 0.0 {
            self.exterior.reverse();
        }
        for hole in &mut self.holes {
            if shoelace_area(hole) > 0.0 {
                hole.reverse();
            }
        }
    }

    /// True if winding is correct (exterior CCW, holes CW).
    pub fn has_correct_winding(&self) -> bool {
        if shoelace_area(&self.exterior) < 0.0 {
            return false;
        }
        self.holes.iter().all(|h| shoelace_area(h) < 0.0)
    }

    /// Convert to a `geo::Polygon`. geo-types requires the closing vertex duplicated.
    pub fn to_geo_polygon(&self) -> geo::Polygon<f64> {
        let exterior = ring_to_geo(&self.exterior);
        let holes: Vec<geo::LineString<f64>> = self.holes.iter().map(|h| ring_to_geo(h)).collect();
        geo::Polygon::new(exterior, holes)
    }

    /// Create from a `geo::Polygon`. Strips the duplicated closing vertex.
    pub fn from_geo_polygon(poly: &geo::Polygon<f64>) -> Self {
        let exterior = ring_from_geo(poly.exterior());
        let holes: Vec<Vec<P2>> = poly.interiors().iter().map(ring_from_geo).collect();
        Self {
            exterior,
            holes,
            closed: true,
        }
    }

    /// Convert exterior to a cavalier_contours closed Polyline (no arcs).
    pub fn exterior_to_pline(&self) -> Polyline<f64> {
        let mut pline = Polyline::with_capacity(self.exterior.len(), true);
        for p in &self.exterior {
            pline.add(p.x, p.y, 0.0);
        }
        pline
    }

    /// Create from a cavalier_contours Polyline (arcs flattened to line segments).
    ///
    /// Arc segments (non-zero bulge) are approximated as their chord endpoints.
    /// For arc-preserving output, use the Polyline directly.
    pub fn from_pline(pline: &Polyline<f64>) -> Self {
        let exterior: Vec<P2> = pline.iter_vertexes().map(|v| P2::new(v.x, v.y)).collect();
        Self::new(exterior)
    }

    /// Perimeter length of the exterior boundary.
    pub fn perimeter(&self) -> f64 {
        ring_perimeter(&self.exterior)
    }

    /// Test if a point is inside this polygon (inside exterior, not inside any hole).
    pub fn contains_point(&self, p: &P2) -> bool {
        if !point_in_polygon(p, &self.exterior) {
            return false;
        }
        !self.holes.iter().any(|h| point_in_polygon(p, h))
    }

    /// Like `contains_point`, but also treats points within `eps` of any
    /// exterior/hole edge as inside.
    ///
    /// Marching-squares output sits exactly on grid-aligned edges, where
    /// the strict ray-cast's inside/outside call is a coin flip. This
    /// widens the boundary by `eps` so those points read as inside
    /// consistently. Does not change `contains_point` itself — that stays
    /// the hot-path check used by adaptive/material_grid.
    pub fn contains_point_eps(&self, p: &P2, eps: f64) -> bool {
        if self.contains_point(p) {
            return true;
        }
        ring_near_point(&self.exterior, p, eps)
            || self.holes.iter().any(|h| ring_near_point(h, p, eps))
    }

    /// True if the exterior or any hole ring crosses itself.
    ///
    /// A shared endpoint between two *adjacent* segments (the normal ring
    /// connectivity) does not count. A repeated vertex or crossing between
    /// two *non-adjacent* segments (a bowtie pinch) does.
    pub fn has_self_intersection(&self) -> bool {
        ring_has_self_intersection(&self.exterior)
            || self.holes.iter().any(|h| ring_has_self_intersection(h))
    }

    /// Normalize a self-intersecting polygon into simple polygons.
    ///
    /// Uses geo's boolean-overlay engine to union the polygon with itself:
    /// `boolean_op` feeds the exterior/hole rings straight into the
    /// i_overlay engine regardless of self-intersections, and the
    /// even-odd fill rule resolves pinched/bowtied rings into distinct
    /// simple polygons the same way a `buffer(0)` trick does in
    /// JTS/shapely. Non-self-intersecting input returns unchanged.
    pub fn repaired(&self) -> Vec<Polygon2> {
        if !self.has_self_intersection() {
            return vec![self.clone()];
        }
        use geo::BooleanOps;
        let geo_poly = self.to_geo_polygon();
        multipolygon_to_polygons(&geo_poly.union(&geo_poly))
    }

    /// Boolean union with `other`. May return multiple polygons if the
    /// inputs are disjoint, or one polygon per merged region otherwise.
    pub fn union(&self, other: &Polygon2) -> Vec<Polygon2> {
        let self_valid = self.exterior.len() >= 3;
        let other_valid = other.exterior.len() >= 3;
        match (self_valid, other_valid) {
            (false, false) => Vec::new(),
            (true, false) => vec![normalized_clone(self)],
            (false, true) => vec![normalized_clone(other)],
            (true, true) => {
                use geo::BooleanOps;
                let a = self.to_geo_polygon();
                let b = other.to_geo_polygon();
                multipolygon_to_polygons(&a.union(&b))
            }
        }
    }

    /// Boolean intersection with `other`. Empty if the polygons don't overlap.
    pub fn intersection(&self, other: &Polygon2) -> Vec<Polygon2> {
        if self.exterior.len() < 3 || other.exterior.len() < 3 {
            return Vec::new();
        }
        use geo::BooleanOps;
        let a = self.to_geo_polygon();
        let b = other.to_geo_polygon();
        multipolygon_to_polygons(&a.intersection(&b))
    }

    /// Boolean difference: regions of `self` not covered by `other`.
    pub fn difference(&self, other: &Polygon2) -> Vec<Polygon2> {
        if self.exterior.len() < 3 {
            return Vec::new();
        }
        if other.exterior.len() < 3 {
            return vec![normalized_clone(self)];
        }
        use geo::BooleanOps;
        let a = self.to_geo_polygon();
        let b = other.to_geo_polygon();
        multipolygon_to_polygons(&a.difference(&b))
    }

    /// Merge a whole set of polygons via a single efficient union pass
    /// (`geo::unary_union`), rather than folding pairwise unions.
    pub fn union_all(polys: &[Polygon2]) -> Vec<Polygon2> {
        let geo_polys: Vec<geo::Polygon<f64>> = polys
            .iter()
            .filter(|p| p.exterior.len() >= 3)
            .map(Polygon2::to_geo_polygon)
            .collect();
        if geo_polys.is_empty() {
            return Vec::new();
        }
        multipolygon_to_polygons(&geo::unary_union(&geo_polys))
    }
}

/// Why an offset produced nothing, when the answer is **not** "the geometry
/// ran out" (Checkpoint C, Q1, shape B — `CAVALIER_SHAPE_FAILURE.md` §6 D-1).
///
/// **A genuine geometric collapse is deliberately not a variant of this
/// enum.** It is the expected, common, correct outcome of offsetting a shape
/// inward past its own width, and it is reported as `(empty, None)` — the
/// absence of a failure, not a failure with a benign name. Three structurally
/// different events used to share one observable value (`Vec::new()`) and
/// R2's `a_contained_panic_is_indistinguishable_from_a_collapse` pinned that
/// as the defect; this type is the channel the return value never had.
///
/// The distinction has a consumer with a real cost attached. At the boundary
/// layer an empty offset does not shrink the containment to nothing, it
/// **removes the containment entirely** (`boundary.rs`, F-1), and that is a
/// legitimate pass-through when the tool is simply larger than the stock and
/// an unbounded over-cut when a dependency assertion fired. Nothing
/// downstream could tell those apart, because nothing upstream carried the
/// difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OffsetFailure {
    /// The input never reached `cavalier_contours` — one of this module's own
    /// preconditions rejected it.
    RejectedInput { reason: OffsetRejection },
    /// `cavalier_contours` (or a transitive dependency of it) panicked and
    /// the chokepoint's `catch_unwind` contained it.
    ///
    /// `assertion` is the panic payload's message, e.g. *"start index should
    /// be less than or equal to end index if polyline is open"*. There is no
    /// source location: see [`crate::panic_message`] for why a library
    /// primitive cannot recover one.
    ///
    /// **Debug/release divergence applies** (Checkpoint C, Q4 option a). Most
    /// of the assertions this arm catches are `debug_assert!`s, so in a
    /// release build they do not fire and this arm is never taken for those
    /// classes — the library proceeds on the unvalidated input instead. See
    /// this module's `## Failure contract` docs.
    LibraryFailure { assertion: String },
}

/// Which precondition of this module rejected an offset input.
///
/// These are the guards that already existed — this enum names them, it does
/// not add any. In particular **no non-finite-coordinate check is performed**:
/// `NaN` still reaches `cavalier_contours` (F-11/F-13 in
/// `ADVERSARIAL_2D_FINDINGS.md`), where in a debug build it trips
/// `static_aabb2d_index`'s `min_x <= max_x` assertion and surfaces as a
/// [`OffsetFailure::LibraryFailure`], and in a release build it does not.
/// Adding a validating constructor to [`Polygon2`] is a separate, unruled
/// item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetRejection {
    /// The exterior ring carried fewer than three vertices.
    ExteriorBelowTriangle,
    /// The exterior ring fell below three vertices once repeat positions
    /// within `PLINE_POS_EQUAL_EPS` were removed.
    ExteriorDegenerateAfterDedupe,
}

impl OffsetFailure {
    /// `true` for [`Self::LibraryFailure`] — the arm that means a dependency
    /// broke rather than that we refused the input.
    ///
    /// The two are treated the same way by every consumer ruled so far (both
    /// REFUSE at the boundary layer, both count at the report-only layer);
    /// this predicate exists so a consumer that ever needs to separate them
    /// does not re-derive the match.
    #[must_use]
    pub const fn is_library_failure(&self) -> bool {
        matches!(self, Self::LibraryFailure { .. })
    }

    /// One line naming what went wrong, for an operator-facing message.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::RejectedInput { reason } => match reason {
                OffsetRejection::ExteriorBelowTriangle => {
                    "the exterior ring has fewer than three vertices".to_owned()
                }
                OffsetRejection::ExteriorDegenerateAfterDedupe => {
                    "the exterior ring collapses below three vertices once repeat \
                     positions are removed"
                        .to_owned()
                }
            },
            Self::LibraryFailure { assertion } => {
                format!("cavalier_contours failed: {assertion}")
            }
        }
    }

    /// Keep the more serious of two failures — a library failure outranks a
    /// rejected input — so one offset over several repaired pieces reports
    /// the worst thing that happened rather than the first.
    fn merge(a: Option<Self>, b: Option<Self>) -> Option<Self> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if a.is_library_failure() { a } else { b }),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }
}

/// Offset a polygon by `distance`.
///
/// Uses cavalier_contours for arc-preserving parallel offset.
///
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Sign convention (for CCW exterior, matching cavalier_contours):
/// - `distance > 0` = **inward** (shrink) — used for pocket clearing
/// - `distance < 0` = **outward** (grow) — used for profile offset
///
/// Returns empty Vec if the polygon collapses entirely.
/// May return multiple polygons if the offset splits the shape.
///
/// R1.5 (2026-07-06): self-intersecting input (bowtie/pinched rings —
/// the shape marching-squares output can produce at saddle points) is
/// repaired via `Polygon2::repaired` *before* it ever reaches
/// cavalier_contours, since that's the actual class of degenerate input
/// most likely to hit the panic path below.
///
/// **This name says nothing about WHY the result is empty.** Callers that
/// need to tell a collapse from a failure call
/// [`offset_polygon_reported`]; this one delegates to it and drops the
/// channel, which is what keeps the fifteen geometry-only call sites at zero
/// churn (Checkpoint C, Q1, shape B).
#[must_use]
pub fn offset_polygon(polygon: &Polygon2, distance: f64) -> Vec<Polygon2> {
    offset_polygon_reported(polygon, distance).0
}

/// [`offset_polygon`] with the failure channel attached.
///
/// `None` means **no failure**: either the offset produced geometry, or it
/// collapsed the way an inward offset of a thin shape is supposed to. `Some`
/// means the emptiness (or the partial result) has a cause that is not
/// geometry — see [`OffsetFailure`].
///
/// A `Some` failure does **not** imply an empty result. A self-intersecting
/// input is repaired into several pieces before cavalier is called (R1.5),
/// and one piece can fail while its siblings succeed. That case is real, it
/// is what F-12 costs an operator (an inlay pocket that ends early and
/// reports success), and it is why the failure is reported whenever it
/// happens rather than only when nothing came back.
#[must_use]
pub fn offset_polygon_reported(
    polygon: &Polygon2,
    distance: f64,
) -> (Vec<Polygon2>, Option<OffsetFailure>) {
    let pieces: Vec<Polygon2> = if polygon.has_self_intersection() {
        polygon.repaired()
    } else {
        vec![polygon.clone()]
    };

    let mut out: Vec<Polygon2> = Vec::new();
    let mut failure: Option<OffsetFailure> = None;
    for piece in &pieces {
        let (polys, f) = offset_one(piece, distance);
        out.extend(polys);
        failure = OffsetFailure::merge(failure, f);
    }
    (out, failure)
}

/// The single chokepoint where cavalier_contours' offset is called.
fn offset_one(polygon: &Polygon2, distance: f64) -> (Vec<Polygon2>, Option<OffsetFailure>) {
    // R1 (tech-debt review 2026-06-10): cavalier_contours 0.7.0 asserts
    // on some degenerate offset inputs ("start index should be less
    // than or equal to end index if polyline is open" in
    // `Shape::parallel_offset`'s slice stitching — reproduced live by
    // WANAKA Back Rough's terrain slices: 86-vertex exterior, 13 holes,
    // inward 5.53 mm; asset at test_data/cavalier_panic_polygon_r1.json).
    // This is the single chokepoint where the dependency is called, so
    // contain the panic here: treat an offset that panics as a collapsed
    // offset (empty result). Every caller already handles empty as
    // "polygon collapsed" — pocket rings end, the adaptive machinability
    // probe reports not-machinable — all under-cut directions, never a
    // gouge.
    //
    // Checkpoint C (Q1): the emptiness is still the return value, but the
    // CAUSE now leaves with it. The `warn!` also names the assertion
    // (D-4) instead of discarding the payload, so "cavalier panicked"
    // becomes "cavalier tripped THIS invariant" in a log an operator or an
    // agent can actually act on.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        offset_polygon_inner(polygon, distance)
    })) {
        Ok(Ok(v)) => (v, None),
        Ok(Err(reason)) => (
            Vec::new(),
            Some(OffsetFailure::RejectedInput { reason }),
        ),
        Err(payload) => {
            let assertion = crate::panic_message::panic_payload_message(payload.as_ref());
            tracing::warn!(
                distance,
                exterior_verts = polygon.exterior.len(),
                holes = polygon.holes.len(),
                assertion = %assertion,
                "offset_polygon: cavalier_contours panicked on degenerate input \
                 despite self-intersection repair already having run; \
                 treating as collapsed offset (empty result)"
            );
            (Vec::new(), Some(OffsetFailure::LibraryFailure { assertion }))
        }
    }
}

/// cavalier_contours' offset contract requires inputs with no
/// repeat-position vertexes (`debug_assert!` in `pline_offset`; in
/// release the invariant is silently assumed). Repeats appear when
/// chained offsets feed cavalier output back in — `pocket_offsets` at
/// small stepovers (found by the R1 generator-extremes fuzz,
/// pocket@0.05 mm floors). Epsilon matches cavalier's
/// `OffsetOptions::default().pos_equal_eps`.
const PLINE_POS_EQUAL_EPS: f64 = 1e-5;

fn dedupe_pline(pline: Polyline<f64>) -> Polyline<f64> {
    pline
        .remove_repeat_pos(PLINE_POS_EQUAL_EPS)
        .unwrap_or(pline)
}

/// cavalier's own lossless cleanup, applied to a CHORD-FLATTENED ring.
///
/// **Reachable only from the offset CASCADE ([`OffsetRingSet`]), never from
/// the single-shot [`offset_polygon`] — wave 14, and the reason is a
/// measurement.** Checkpoint D's ruling was for this cleanup to land
/// "everywhere as the lossless companion". It cannot: `remove_redundant` is
/// lossless with respect to the SHAPE, and several consumers are not using
/// the polygon as a shape. `scallop` seeds its cascade with a rectangle
/// sampled at the flat-ground stepover and lifts every vertex with a
/// drop-cutter query; `project_curve` and `trace` project vertices onto a
/// mesh the same way. Every intermediate point on those straight runs is
/// correctly identified as carrying no geometry — and it carries all of the
/// sampling. Dropped into the shared wrapper, this turned a four-island
/// scallop pass into an EMPTY toolpath, because the four surviving corners
/// all sat off the part (`capability_link_moves_safety`).
///
/// Inside the cascade it is safe, because [`FlattenPolicy`] re-establishes
/// the sampling density explicitly on the way out — a stated contract instead
/// of an inherited accident. That pairing is the whole point.
///
/// `remove_redundant`
/// drops a vertex only when it is a repeat position within `pos_equal_eps` or
/// when the triangle it forms with its neighbours has area under
/// `pos_equal_eps` *and* the direction of travel does not reverse — so the
/// worst deviation it can introduce is `2 · eps / base`, i.e. sub-micron on
/// any segment longer than 20 µm and sub-nanometre on a 50 mm one. Nothing
/// moves; debris leaves.
///
/// It is applied here on a **bulge-free** copy on purpose. Run on a polyline
/// that still carries arcs, `remove_redundant` also merges two co-radial,
/// co-centred arc segments into one — lossless in the ARC domain (that is the
/// shrink [`OffsetRingSet`] is built on) but *not* in the flattened one,
/// because the merged arc is then replaced by one long chord instead of two
/// short ones. The domain the cleanup runs in has to match the domain the
/// result is consumed in, and `offset_polygon`'s consumers get chords.
fn cleaned_flat_ring(pline: &Polyline<f64>) -> Vec<P2> {
    let raw: Vec<P2> = pline.iter_vertexes().map(|v| P2::new(v.x, v.y)).collect();
    let mut flat = Polyline::with_capacity(pline.vertex_count(), true);
    for p in &raw {
        flat.add(p.x, p.y, 0.0);
    }
    let Some(cleaned) = flat.remove_redundant(PLINE_POS_EQUAL_EPS) else {
        // `None` means nothing was redundant — the common case, and the
        // reason single-shot consumers see byte-identical output.
        return raw;
    };
    if cleaned.vertex_count() < 3 {
        // A ring that only survives as a sliver keeps its raw form; the
        // callers' own `len() < 3` guards decide what to do with it.
        return raw;
    }
    cleaned.iter_vertexes().map(|v| P2::new(v.x, v.y)).collect()
}

/// The offset itself, minus the panic containment.
///
/// `Err` is this module's own refusal — the guards that were previously
/// bare `return Vec::new()`s indistinguishable from a collapse. `Ok(vec![])`
/// is a genuine collapse and stays that way.
fn offset_polygon_inner(
    polygon: &Polygon2,
    distance: f64,
) -> Result<Vec<Polygon2>, OffsetRejection> {
    if polygon.exterior.len() < 3 {
        return Err(OffsetRejection::ExteriorBelowTriangle);
    }

    if polygon.holes.is_empty() {
        // Simple case: just offset the exterior
        let pline = dedupe_pline(polygon.exterior_to_pline());
        if pline.vertex_count() < 3 {
            return Err(OffsetRejection::ExteriorDegenerateAfterDedupe);
        }
        let results = pline.parallel_offset(distance);
        Ok(results.iter().map(Polygon2::from_pline).collect())
    } else {
        // Polygon with holes: use Shape to handle hole interaction
        use cavalier_contours::shape_algorithms::Shape;

        let exterior = dedupe_pline(polygon.exterior_to_pline());
        if exterior.vertex_count() < 3 {
            return Err(OffsetRejection::ExteriorDegenerateAfterDedupe);
        }
        let mut plines = vec![exterior];
        for hole in &polygon.holes {
            let mut hole_pline = Polyline::with_capacity(hole.len(), true);
            for p in hole {
                hole_pline.add(p.x, p.y, 0.0);
            }
            let hole_pline = dedupe_pline(hole_pline);
            // A hole that dedupes below a triangle is pure noise.
            if hole_pline.vertex_count() >= 3 {
                plines.push(hole_pline);
            }
        }

        let shape = Shape::from_plines(plines);
        let result = shape.parallel_offset(distance, Default::default());

        // CCW plines are boundaries, CW are holes.
        // Pair each hole with its containing boundary via containment test.
        let mut polygons: Vec<Polygon2> = result
            .ccw_plines
            .iter()
            .map(|ip| Polygon2::from_pline(&ip.polyline))
            .collect();

        let holes: Vec<Vec<P2>> = result
            .cw_plines
            .iter()
            .map(|ip| {
                ip.polyline
                    .iter_vertexes()
                    .map(|v| P2::new(v.x, v.y))
                    .collect()
            })
            .collect();

        for hole in holes {
            let Some(test_pt) = hole.first() else {
                continue;
            };
            let mut assigned = false;
            for poly in &mut polygons {
                if poly.contains_point(test_pt) {
                    poly.holes.push(hole.clone());
                    assigned = true;
                    break;
                }
            }
            if !assigned && let Some(first_poly) = polygons.first_mut() {
                // Fallback: attach to first polygon (preserves old behavior)
                first_poly.holes.push(hole);
            }
        }

        Ok(polygons)
    }
}

/// **M5 research candidate (c): lossless cleanup.** Drop vertices that carry
/// no geometry — a position within `pos_eps` of the previous kept vertex, or
/// a perpendicular deviation under `deviation_tol` from the chord through its
/// neighbours.
///
/// The deviation rule is LOCAL (measured against the last kept vertex), so
/// error can in principle accumulate along a chain of removals; it is meant
/// to be called with `deviation_tol` at or near zero, where "no geometry"
/// is literal. For a bounded-error reduction use [`simplify_bounded`], whose
/// error is bounded against the whole input chain by construction.
///
/// Returns `None` when the exterior cannot keep three vertices. Holes that
/// collapse are dropped individually. **No production path calls this**; it
/// exists so M5's candidate comparison and the scallop research seam measure
/// one implementation rather than two.
#[must_use]
pub fn cleanup_collinear(polygon: &Polygon2, pos_eps: f64, deviation_tol: f64) -> Option<Polygon2> {
    let exterior = cleanup_ring(&polygon.exterior, pos_eps, deviation_tol)?;
    let holes: Vec<Vec<P2>> = polygon
        .holes
        .iter()
        .filter_map(|h| cleanup_ring(h, pos_eps, deviation_tol))
        .collect();
    let mut out = Polygon2::new(exterior);
    out.holes = holes;
    out.closed = polygon.closed;
    Some(out)
}

fn cleanup_ring(ring: &[P2], pos_eps: f64, deviation_tol: f64) -> Option<Vec<P2>> {
    let n = ring.len();
    if n < 3 {
        return None;
    }
    let mut out: Vec<P2> = Vec::with_capacity(n);
    for i in 0..n {
        // SAFETY: `i < n` and the modulo keeps `next` in range.
        #[allow(clippy::indexing_slicing)]
        let (cur, next) = (ring[i], ring[(i + 1) % n]);
        let prev = *out.last().unwrap_or_else(|| {
            // SAFETY: `n >= 3` checked above.
            #[allow(clippy::indexing_slicing)]
            &ring[n - 1]
        });
        if (cur.x - prev.x).hypot(cur.y - prev.y) <= pos_eps {
            continue;
        }
        if point_segment_distance_sq(&cur, &prev, &next) <= deviation_tol * deviation_tol {
            continue;
        }
        out.push(cur);
    }
    (out.len() >= 3).then_some(out)
}

/// **M5 research candidate (b): tolerance-bounded, topology-preserving
/// simplification.** Ramer–Douglas–Peucker on each closed ring, with the
/// deviation of every dropped vertex bounded by `tol` against the retained
/// chain — not against its immediate neighbours.
///
/// Topology guards, in order: a ring that cannot keep three vertices is kept
/// UNSIMPLIFIED rather than dropped; a result that self-intersects is
/// discarded and the input returned. Winding is preserved because RDP only
/// removes vertices and never reorders them.
///
/// **No production path calls this** — see [`cleanup_collinear`].
#[must_use]
pub fn simplify_bounded(polygon: &Polygon2, tol: f64) -> Option<Polygon2> {
    if tol <= 0.0 {
        return Some(polygon.clone());
    }
    let exterior = simplify_closed_ring(&polygon.exterior, tol);
    let holes: Vec<Vec<P2>> = polygon
        .holes
        .iter()
        .map(|h| simplify_closed_ring(h, tol))
        .collect();
    let mut out = Polygon2::new(exterior);
    out.holes = holes;
    out.closed = polygon.closed;
    if out.exterior.len() < 3 {
        return Some(polygon.clone());
    }
    if out.has_self_intersection() && !polygon.has_self_intersection() {
        // Simplification invented a crossing the input did not have. Bail
        // out rather than hand a broken ring to the next offset.
        return Some(polygon.clone());
    }
    Some(out)
}

/// RDP over a closed ring: split at vertex 0 and the vertex farthest from it
/// (two anchors that are always on the simplified hull), simplify the two
/// open chains, and stitch.
fn simplify_closed_ring(ring: &[P2], tol: f64) -> Vec<P2> {
    let n = ring.len();
    if n < 5 {
        return ring.to_vec();
    }
    // SAFETY: `n >= 5`.
    #[allow(clippy::indexing_slicing)]
    let head = ring[0];
    let far = (1..n)
        .max_by(|a, b| {
            // SAFETY: indices come from `1..n`.
            #[allow(clippy::indexing_slicing)]
            let (pa, pb) = (ring[*a], ring[*b]);
            (pa.x - head.x)
                .hypot(pa.y - head.y)
                .total_cmp(&(pb.x - head.x).hypot(pb.y - head.y))
        })
        .unwrap_or(n / 2);

    // SAFETY: `far` is in `1..n`.
    #[allow(clippy::indexing_slicing)]
    let first: Vec<P2> = ring[0..=far].to_vec();
    #[allow(clippy::indexing_slicing)]
    let mut second: Vec<P2> = ring[far..n].to_vec();
    second.push(head);

    let mut out = rdp(&first, tol);
    let mut tail = rdp(&second, tol);
    // Both chains share their endpoints; drop the duplicates.
    out.pop();
    tail.pop();
    out.extend(tail);
    if out.len() < 3 { ring.to_vec() } else { out }
}

fn rdp(chain: &[P2], tol: f64) -> Vec<P2> {
    let n = chain.len();
    if n < 3 {
        return chain.to_vec();
    }
    // SAFETY: `n >= 3`.
    #[allow(clippy::indexing_slicing)]
    let (a, b) = (chain[0], chain[n - 1]);
    let mut worst = 0.0_f64;
    let mut worst_i = 0usize;
    for (i, p) in chain.iter().enumerate().take(n - 1).skip(1) {
        let d = point_segment_distance_sq(p, &a, &b);
        if d > worst {
            worst = d;
            worst_i = i;
        }
    }
    if worst_i == 0 || worst <= tol * tol {
        return vec![a, b];
    }
    // SAFETY: `0 < worst_i < n - 1`.
    #[allow(clippy::indexing_slicing)]
    let left = rdp(&chain[0..=worst_i], tol);
    #[allow(clippy::indexing_slicing)]
    let right = rdp(&chain[worst_i..n], tol);
    let mut out = left;
    out.pop();
    out.extend(right);
    out
}

// ===========================================================================
// The arc-carrying offset cascade — M5 / Checkpoint D, 2026-08-03
// ===========================================================================

/// How an offset ring stops being arcs and starts being straight feed moves.
///
/// **There is exactly one of these in the workspace, and this is it.** The
/// defect Checkpoint D closed was not that flattening happens — a toolpath is
/// polylines in the end — but that it happened *between* every pair of
/// offsets, unbounded and un-named. [`Polygon2::from_pline`] discards each
/// arc join's bulge and keeps its two endpoints, so a join of turn angle `θ`
/// at offset radius `r` cuts the corner by its sagitta `r·(1 − cos(θ/2))`:
/// **29% of the offset distance at a 90° join**, and then the two shallower
/// corners it leaves behind each arc-join on the next pass. Measured 1:1
/// (added vertices == arc-join segments) and exactly doubling, on every
/// concave fixture (`CHECKPOINT_D_EVIDENCE.md` §3–§5).
///
/// A cascade that carries arcs and flattens ONCE, here, reads **0.0 µm** off
/// the tolerance-free erosion oracle and *shrinks* 1.4%/ring instead of
/// growing 35%/ring (§6, §7).
///
/// # Where the number comes from
///
/// The tolerance is a share of the calling operation's own chord tolerance —
/// the one the operator set — and never a second, competing dial. On a 3D
/// finishing op the emitted ring is then chord-refined against the surface
/// (`scallop::refine_chord`), which accepts at
/// `CHORD_REFINE_ACCEPT_FRACTION = 0.70` of that same tolerance and leaves
/// the remaining 30% as headroom for the probe-vs-true-worst gap M4 measured
/// at ~35% of the tolerance. Arc flattening is an error in XY, independent of
/// that Z error, so it is budgeted at [`Self::CHORD_TOLERANCE_SHARE`] = 10%:
/// small enough that the composed worst case stays inside the operator's
/// number, large enough that the emitted chords stay longer than
/// `scallop::CHORD_REFINE_MIN_SPLIT_MM` on any arc a stepover-sized offset
/// can produce (chord ≈ `2·√(2·r·tol)`, i.e. 0.13 mm at r = 0.2 mm and a
/// 10 µm budget).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlattenPolicy {
    max_deviation_mm: f64,
    /// See [`FlattenPolicy::with_max_segment`]. `None` = no sampling bound,
    /// which is right for a consumer that treats the ring purely as a shape.
    max_segment_mm: Option<f64>,
}

impl FlattenPolicy {
    /// Share of the calling operation's chord tolerance the XY flattening is
    /// allowed to consume. See the type doc for the derivation.
    pub const CHORD_TOLERANCE_SHARE: f64 = 0.10;

    /// Floor (mm). Ten times `toolpath::MIN_EMITTED_SEGMENT_MM`, the coarsest
    /// shipped post's coordinate quantum — below this the flattening is
    /// asking for precision the G-code cannot carry, and pays for it in
    /// vertices.
    pub const MIN_DEVIATION_MM: f64 = 0.001;

    /// Ceiling (mm). A very loose operation tolerance must not buy a visibly
    /// faceted wall; 50 µm is half the finest finishing tolerance in this
    /// workspace and a third of `CHORD_REFINE_MIN_SEG_MM`'s reference pitch.
    pub const MAX_DEVIATION_MM: f64 = 0.050;

    /// What a consumer with no stated chord tolerance gets — 2.5D clearing
    /// (pocket) has a stepover and a tool radius but no tolerance dial.
    ///
    /// 10 µm is 14× tighter than the corner cut that same path ships today
    /// (§8: 141 µm max, −60 µm systematic over-cut on a 12-vertex cross at a
    /// 0.5 mm stepover), and it is bounded, which the corner cut never was.
    pub const UNTOLERANCED_MM: f64 = 0.010;

    /// Derive the flatten budget from the operation's chord tolerance.
    #[must_use]
    pub fn from_chord_tolerance(chord_tolerance_mm: f64) -> Self {
        let raw = chord_tolerance_mm * Self::CHORD_TOLERANCE_SHARE;
        let clamped = if raw.is_finite() {
            raw.clamp(Self::MIN_DEVIATION_MM, Self::MAX_DEVIATION_MM)
        } else {
            Self::UNTOLERANCED_MM
        };
        Self {
            max_deviation_mm: clamped,
            max_segment_mm: None,
        }
    }

    /// The budget for a consumer that has no tolerance dial to derive from.
    #[must_use]
    pub const fn untoleranced() -> Self {
        Self {
            max_deviation_mm: Self::UNTOLERANCED_MM,
            max_segment_mm: None,
        }
    }

    /// Cap the length of any emitted segment, subdividing straight runs to
    /// suit — the SAMPLING half of the policy.
    ///
    /// **This exists because "flatten to a deviation tolerance" is only half
    /// of what a ring flattening decides, and wave 14 learned the other half
    /// the hard way.** A deviation budget puts points where the boundary
    /// CURVES and nowhere else, which is correct if the ring is only ever
    /// going to be a shape. It is wrong the moment a consumer treats the ring
    /// vertices as SAMPLE POSITIONS: `scallop` lifts every ring vertex with a
    /// drop-cutter query and asks a coverage mask whether that XY sits over
    /// real mesh, so a 50 mm straight run with two endpoints is two samples of
    /// a surface, not a straight cut. On a mesh of four disjoint islands both
    /// endpoints land off the part, the run-splitter discards the whole run,
    /// and the operation emits **nothing at all** — which is exactly what
    /// happened, and what `capability_link_moves_safety` caught.
    ///
    /// The same trap sits under the "lossless" cleanup: scallop seeds its
    /// cascade with a rectangle sampled at the flat-ground stepover, and
    /// `remove_redundant` correctly identifies every one of those intermediate
    /// points as carrying no geometry. It carries no geometry and all of the
    /// sampling. **Losslessness is a property of a MEASURE, and "the shape" is
    /// not the only measure a polygon is carrying.**
    ///
    /// So a consumer that samples through the ring states the density it needs
    /// here, in world units, and gets it — instead of inheriting it from
    /// whatever debris the offset primitive happened to leave behind.
    #[must_use]
    pub fn with_max_segment(self, max_segment_mm: f64) -> Self {
        Self {
            max_segment_mm: (max_segment_mm.is_finite() && max_segment_mm > 0.0)
                .then_some(max_segment_mm),
            ..self
        }
    }

    #[must_use]
    pub const fn max_deviation_mm(self) -> f64 {
        self.max_deviation_mm
    }

    #[must_use]
    pub const fn max_segment_mm(self) -> Option<f64> {
        self.max_segment_mm
    }
}

impl Default for FlattenPolicy {
    fn default() -> Self {
        Self::untoleranced()
    }
}

/// One offset boundary and the holes inside it, in cavalier's own
/// arc-carrying representation.
#[derive(Clone, Debug)]
struct RingGroup {
    boundary: Polyline<f64>,
    holes: Vec<Polyline<f64>>,
}

impl RingGroup {
    fn vertex_count(&self) -> usize {
        self.boundary.vertex_count()
            + self
                .holes
                .iter()
                .map(PlineSource::vertex_count)
                .sum::<usize>()
    }

    /// Offset this group once, through the SAME cavalier entry points
    /// `offset_polygon_inner` picks between, with the same panic containment
    /// and the same hole/container pairing — the only difference is that the
    /// bulges survive.
    ///
    /// The second chokepoint, and it carries the same failure channel as the
    /// single-shot one (Checkpoint C, Q1). `None` is a collapse; `Some` is a
    /// cause.
    fn offset(&self, distance: f64) -> (Vec<RingGroup>, Option<OffsetFailure>) {
        let boundary = dedupe_pline(self.boundary.clone());
        if boundary.vertex_count() < 3 {
            return (
                Vec::new(),
                Some(OffsetFailure::RejectedInput {
                    reason: OffsetRejection::ExteriorDegenerateAfterDedupe,
                }),
            );
        }
        let holes: Vec<Polyline<f64>> = self
            .holes
            .iter()
            .cloned()
            .map(dedupe_pline)
            .filter(|h| h.vertex_count() >= 3)
            .collect();

        // Same containment as `offset_one`: a cavalier panic is a collapsed
        // offset, which every caller already handles as "the ring ended".
        let out = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            if holes.is_empty() {
                (boundary.parallel_offset(distance), Vec::new())
            } else {
                use cavalier_contours::shape_algorithms::Shape;
                let mut plines = vec![boundary];
                plines.extend(holes);
                let shape = Shape::from_plines(plines);
                let result = shape.parallel_offset(distance, Default::default());
                (
                    result
                        .ccw_plines
                        .into_iter()
                        .map(|ip| ip.polyline)
                        .collect::<Vec<_>>(),
                    result
                        .cw_plines
                        .into_iter()
                        .map(|ip| ip.polyline)
                        .collect::<Vec<_>>(),
                )
            }
        })) {
            Ok(v) => v,
            Err(payload) => {
                let assertion = crate::panic_message::panic_payload_message(payload.as_ref());
                tracing::warn!(
                    distance,
                    boundary_verts = self.boundary.vertex_count(),
                    holes = self.holes.len(),
                    assertion = %assertion,
                    "offset cascade: cavalier_contours panicked; treating as a \
                     collapsed offset (empty result)"
                );
                return (
                    Vec::new(),
                    Some(OffsetFailure::LibraryFailure { assertion }),
                );
            }
        };
        let (boundaries, holes) = out;

        // The lossless companion, in the ARC domain: `remove_redundant`
        // merges two co-radial, co-centred arcs into one and drops collinear
        // line vertices. It is what turns the cascade's vertex curve from
        // flat into shrinking (−1.4%/ring, §6) and it costs nothing.
        let tidy = |pl: Polyline<f64>| -> Option<Polyline<f64>> {
            let pl = pl.remove_redundant(PLINE_POS_EQUAL_EPS).unwrap_or(pl);
            (pl.vertex_count() >= 3).then_some(pl)
        };

        let mut groups: Vec<RingGroup> = boundaries
            .into_iter()
            .filter_map(tidy)
            .map(|boundary| RingGroup {
                boundary,
                holes: Vec::new(),
            })
            .collect();
        for hole in holes.into_iter().filter_map(tidy) {
            let Some(test) = hole.iter_vertexes().next().map(|v| P2::new(v.x, v.y)) else {
                continue;
            };
            let owner = groups
                .iter_mut()
                .find(|g| flatten_for_containment(&g.boundary).contains_point(&test));
            if let Some(owner) = owner {
                owner.holes.push(hole);
            } else if let Some(first) = groups.first_mut() {
                // Same fallback `offset_polygon_inner` has always taken.
                first.holes.push(hole);
            }
        }
        (groups, None)
    }

    /// Flatten to a `Polygon2` — the ONE place the arcs leave the cascade.
    fn to_polygon(&self, policy: FlattenPolicy) -> Polygon2 {
        let mut out = Polygon2::new(flatten_ring(&self.boundary, policy));
        out.holes = self
            .holes
            .iter()
            .map(|h| flatten_ring(h, policy))
            .filter(|h| h.len() >= 3)
            .collect();
        out
    }
}

/// Tessellate one polyline's arcs at `policy`'s deviation budget, clean the
/// result losslessly, and then subdivide any run longer than the policy's
/// sampling bound.
///
/// Order matters: the lossless cleanup runs BEFORE the subdivision, so it
/// removes cavalier's debris rather than the points the sampling bound just
/// asked for.
fn flatten_ring(pline: &Polyline<f64>, policy: FlattenPolicy) -> Vec<P2> {
    let flat = pline
        .arcs_to_approx_lines(policy.max_deviation_mm())
        .unwrap_or_else(|| pline.clone());
    let ring = cleaned_flat_ring(&flat);
    match policy.max_segment_mm() {
        Some(max) => subdivide_ring(&ring, max),
        None => ring,
    }
}

/// Split every closed-ring edge longer than `max_segment_mm` into equal
/// pieces. Adds points ON the existing edges only — the ring's shape is
/// bit-for-bit the same polygon, which is what makes this safe to apply after
/// the geometry has been decided.
fn subdivide_ring(ring: &[P2], max_segment_mm: f64) -> Vec<P2> {
    let n = ring.len();
    // NaN must fall through to "no subdivision", so the finite check is
    // explicit rather than a negated comparison.
    if n < 2 || !max_segment_mm.is_finite() || max_segment_mm <= 0.0 {
        return ring.to_vec();
    }
    let mut out: Vec<P2> = Vec::with_capacity(n);
    for i in 0..n {
        // SAFETY: `i < n` and the modulo keeps the successor in range.
        #[allow(clippy::indexing_slicing)]
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        out.push(a);
        let len = (b.x - a.x).hypot(b.y - a.y);
        if !len.is_finite() || len <= max_segment_mm {
            continue;
        }
        let pieces = (len / max_segment_mm).ceil();
        if !pieces.is_finite() || pieces > 100_000.0 {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pieces = pieces as usize;
        for k in 1..pieces {
            let t = k as f64 / pieces as f64;
            out.push(P2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        }
    }
    out
}

/// A coarse flattening used ONLY to answer "is this hole inside that
/// boundary" — never emitted, never fed back into the cascade, so its
/// tolerance is a speed choice rather than a fidelity one.
fn flatten_for_containment(pline: &Polyline<f64>) -> Polygon2 {
    Polygon2::new(flatten_ring(
        pline,
        FlattenPolicy {
            max_deviation_mm: FlattenPolicy::MAX_DEVIATION_MM,
            max_segment_mm: None,
        },
    ))
}

/// A set of offset rings that keeps cavalier's arcs between offsets.
///
/// This is the cascade type Checkpoint D adopted. Use it wherever an
/// operation feeds `offset_polygon`'s own output back into it — today
/// [`crate::scallop`] and [`crate::pocket`]. A consumer that offsets **once**
/// should keep calling [`offset_polygon`]: it inherits a single arc-join
/// chord, which is a different and much smaller defect, and its topology
/// stays exactly where it is.
///
/// Groups are stable in order and one-to-one with the polygons
/// [`Self::to_polygons`] returns, so a caller can carry a per-group decision
/// (scallop's per-polygon stepover) across an offset.
#[derive(Clone, Debug, Default)]
pub struct OffsetRingSet {
    groups: Vec<RingGroup>,
}

impl OffsetRingSet {
    /// Seed a cascade from one polygon, repairing self-intersection first —
    /// exactly what [`offset_polygon`] does before it calls cavalier (R1.5).
    #[must_use]
    pub fn from_polygon(polygon: &Polygon2) -> Self {
        let pieces: Vec<Polygon2> = if polygon.has_self_intersection() {
            polygon.repaired()
        } else {
            vec![polygon.clone()]
        };
        Self::from_polygons(&pieces)
    }

    /// Seed a cascade from several polygons. Each becomes its own group, so
    /// a panic or a collapse in one cannot take the others with it.
    #[must_use]
    pub fn from_polygons(polygons: &[Polygon2]) -> Self {
        let groups = polygons
            .iter()
            .filter(|p| p.exterior.len() >= 3)
            .filter_map(|p| {
                let boundary = dedupe_pline(p.exterior_to_pline());
                if boundary.vertex_count() < 3 {
                    return None;
                }
                let holes = p
                    .holes
                    .iter()
                    .filter_map(|hole| {
                        let mut pl = Polyline::with_capacity(hole.len(), true);
                        for pt in hole {
                            pl.add(pt.x, pt.y, 0.0);
                        }
                        let pl = dedupe_pline(pl);
                        (pl.vertex_count() >= 3).then_some(pl)
                    })
                    .collect();
                Some(RingGroup { boundary, holes })
            })
            .collect();
        Self { groups }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// Number of independent boundaries. One-to-one with
    /// [`Self::to_polygons`]' output, in the same order.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// POLYLINE vertices — an arc is one vertex plus a bulge, so this is not
    /// the count a toolpath would carry. For that, flatten first.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.groups.iter().map(RingGroup::vertex_count).sum()
    }

    /// Offset every group by the same distance.
    ///
    /// Drops the failure channel; call [`Self::offset_reported`] to keep it.
    #[must_use]
    pub fn offset(&self, distance: f64) -> Self {
        self.offset_reported(distance).0
    }

    /// [`Self::offset`] with the failure channel attached — same contract as
    /// [`offset_polygon_reported`]: `None` is a collapse, `Some` is a cause,
    /// and a `Some` does not imply an empty set (one group can fail while its
    /// siblings survive).
    #[must_use]
    pub fn offset_reported(&self, distance: f64) -> (Self, Option<OffsetFailure>) {
        let mut groups = Vec::new();
        let mut failure = None;
        for g in &self.groups {
            let (out, f) = g.offset(distance);
            groups.extend(out);
            failure = OffsetFailure::merge(failure, f);
        }
        (Self { groups }, failure)
    }

    /// Offset group `i` by `distances[i]`. Groups past the end of the slice
    /// take the last distance given; an empty slice offsets nothing.
    #[must_use]
    pub fn offset_per_group(&self, distances: &[f64]) -> Self {
        self.offset_per_group_reported(distances).0
    }

    /// [`Self::offset_per_group`] with the failure channel attached.
    #[must_use]
    pub fn offset_per_group_reported(&self, distances: &[f64]) -> (Self, Option<OffsetFailure>) {
        let Some(&fallback) = distances.last() else {
            return (Self::default(), None);
        };
        let mut groups = Vec::new();
        let mut failure = None;
        for (i, g) in self.groups.iter().enumerate() {
            let (out, f) = g.offset(distances.get(i).copied().unwrap_or(fallback));
            groups.extend(out);
            failure = OffsetFailure::merge(failure, f);
        }
        (Self { groups }, failure)
    }

    /// Flatten the whole set — the boundary where the cascade's geometry
    /// becomes toolpath geometry. One polygon per group, in group order.
    #[must_use]
    pub fn to_polygons(&self, policy: FlattenPolicy) -> Vec<Polygon2> {
        self.groups.iter().map(|g| g.to_polygon(policy)).collect()
    }
}

/// Detect containment among a flat list of polygons and nest inner polygons
/// as holes of their containing polygon.
///
/// Given polygons from an SVG or DXF where separate shapes may represent
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// an outer boundary with interior islands, this function:
/// 1. Sorts polygons by area (largest first)
/// 2. For each smaller polygon, checks if it's fully inside a larger one
/// 3. If so, converts it to a hole of that polygon
///
/// Returns the nested polygons (outer boundaries with holes attached).
pub fn detect_containment(mut polygons: Vec<Polygon2>) -> Vec<Polygon2> {
    if polygons.len() <= 1 {
        return polygons;
    }

    // Ensure all have correct winding before containment test
    for poly in &mut polygons {
        poly.ensure_winding();
    }

    // Sort by area descending (largest = outer boundaries)
    polygons.sort_by(|a, b| {
        b.area()
            .partial_cmp(&a.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Even-odd nesting: for each closed polygon, count the closed polygons
    // that strictly contain it. Even depth → a top-level region (an island
    // inside a hole is solid material again); odd depth → a hole of its
    // innermost even-depth container. The previous implementation attached
    // every contained polygon as a hole of the FIRST (largest) container it
    // found, which silently deleted island-in-hole territory — surfaced by
    // the finish planner's annular slope bands (a dome cap inside a
    // mid-steep annulus vanished from the shallow band).
    // Only closed polygons participate — open paths (rivers, traces, engrave
    // curves) must stay top-level or project_curve will close their "hole"
    // rings and carve phantom lines between fragments.
    let n = polygons.len();
    let mut consumed = vec![false; n];
    // Track holes to add to each polygon
    let mut holes_for: Vec<Vec<usize>> = vec![Vec::new(); n];

    // Safety: i and j are bounded by n (polygon count); depths, consumed and
    // holes_for are sized to n. All indices are valid by loop construction.
    #[allow(clippy::indexing_slicing)]
    {
        // Pass 1: containment depth of every closed polygon. A container is
        // strictly larger, so with the area-descending sort above it always
        // has a smaller index — scanning j in 0..i suffices.
        let mut depths = vec![0usize; n];
        for i in 0..n {
            if !polygons[i].closed {
                continue;
            }
            for j in 0..i {
                if polygons[j].closed && polygon_contains_polygon(&polygons[j], &polygons[i]) {
                    depths[i] += 1;
                }
            }
        }

        // Pass 2: odd-depth polygons become holes of their innermost
        // even-depth container — the largest index (smallest area) among
        // containers that are themselves top-level.
        for i in 0..n {
            if !polygons[i].closed || depths[i].is_multiple_of(2) {
                continue;
            }
            let innermost = (0..i).rfind(|&j| {
                polygons[j].closed
                    && depths[j].is_multiple_of(2)
                    && polygon_contains_polygon(&polygons[j], &polygons[i])
            });
            if let Some(j) = innermost {
                holes_for[j].push(i);
                consumed[i] = true;
            }
        }
    }

    // Build result: outer polygons with their holes attached
    // Safety: i iterates over 0..n, hole_idx values stored in holes_for[i]
    // are valid polygon indices by construction above.
    #[allow(clippy::indexing_slicing)]
    let result = {
        let mut result = Vec::new();
        for (i, poly) in polygons.iter().enumerate() {
            if consumed[i] {
                continue;
            }
            let mut outer = poly.clone();
            for &hole_idx in &holes_for[i] {
                let mut hole_pts = polygons[hole_idx].exterior.clone();
                // Holes must be CW (reverse if CCW)
                if shoelace_area(&hole_pts) > 0.0 {
                    hole_pts.reverse();
                }
                outer.holes.push(hole_pts);
            }
            result.push(outer);
        }
        result
    };

    result
}

/// Test if all vertices of `inner` are inside `outer`'s exterior boundary.
fn polygon_contains_polygon(outer: &Polygon2, inner: &Polygon2) -> bool {
    // Quick bbox check
    let outer_bb = polygon_bbox(&outer.exterior);
    let inner_bb = polygon_bbox(&inner.exterior);
    if inner_bb.0 < outer_bb.0
        || inner_bb.1 < outer_bb.1
        || inner_bb.2 > outer_bb.2
        || inner_bb.3 > outer_bb.3
    {
        return false;
    }

    // Check that all inner vertices are inside the outer polygon (ray casting)
    inner
        .exterior
        .iter()
        .all(|p| point_in_polygon(p, &outer.exterior))
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Ray-casting point-in-polygon test. THE canonical ray cast for this
/// crate — `pub(crate)` so other modules reuse it instead of keeping their
/// own copy (R1.7, 2026-07-06: `boundary.rs`'s test-only duplicate was
/// deleted in favor of `Polygon2::contains_point`, which wraps this).
pub(crate) fn point_in_polygon(point: &P2, polygon: &[P2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    // Safety: i is bounded by 0..n, j tracks previous index (also < n).
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[(i + n - 1) % n];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
    }
    inside
}

fn polygon_bbox(pts: &[P2]) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut y_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for p in pts {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }
    (x_min, y_min, x_max, y_max)
}

// --- helpers ---

/// Pick the polygon with the largest `area()` out of a slice, NaN-safe.
///
/// Ties (equal area) resolve to the *last* maximal element — the same
/// behavior `Iterator::max_by` gives for equal keys. Returns `None` for
/// an empty slice.
pub fn largest_by_area(polys: &[Polygon2]) -> Option<&Polygon2> {
    polys.iter().max_by(|a, b| {
        a.area()
            .partial_cmp(&b.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn shoelace_area(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    // Safety: i is bounded by 0..n, j = (i+1)%n is also < n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y;
        area -= pts[j].x * pts[i].y;
    }
    area / 2.0
}

fn ring_to_geo(pts: &[P2]) -> geo::LineString<f64> {
    let mut coords: Vec<geo::Coord<f64>> =
        pts.iter().map(|p| geo::Coord { x: p.x, y: p.y }).collect();
    // geo requires closing vertex
    if let Some(&first) = pts.first() {
        coords.push(geo::Coord {
            x: first.x,
            y: first.y,
        });
    }
    geo::LineString::new(coords)
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn ring_from_geo(ring: &geo::LineString<f64>) -> Vec<P2> {
    let mut pts: Vec<P2> = ring.coords().map(|c| P2::new(c.x, c.y)).collect();
    // Strip duplicated closing vertex
    if let (Some(&first), Some(&last)) = (pts.first(), pts.last())
        && pts.len() >= 2
        && (first.x - last.x).abs() < 1e-10
        && (first.y - last.y).abs() < 1e-10
    {
        pts.pop();
    }
    pts
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
fn ring_perimeter(pts: &[P2]) -> f64 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut perim = 0.0;
    // Safety: i is bounded by 0..n, j = (i+1)%n is also < n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let j = (i + 1) % n;
        perim += (pts[j] - pts[i]).norm();
    }
    perim
}

// --- boolean-op helpers ---

/// Convert a `geo::MultiPolygon` (boolean-op output) back to `Polygon2`s,
/// with holes populated and winding normalized to this crate's convention.
fn multipolygon_to_polygons(mp: &geo::MultiPolygon<f64>) -> Vec<Polygon2> {
    mp.iter()
        .map(|p| {
            let mut poly = Polygon2::from_geo_polygon(p);
            poly.ensure_winding();
            poly
        })
        .collect()
}

/// Owned, winding-normalized copy — used when a boolean op degenerates to
/// "return one of the operands unchanged" (e.g. union/difference against
/// an empty/degenerate polygon).
fn normalized_clone(p: &Polygon2) -> Polygon2 {
    let mut c = p.clone();
    c.ensure_winding();
    c.closed = true;
    c
}

// --- self-intersection detection & repair helpers ---

/// Epsilon for the orientation-sign / collinearity tests in
/// `segments_intersect`. Loose relative to `PLINE_POS_EQUAL_EPS` since
/// this operates on raw doubles (areas of triangles), not offset positions.
const ORIENT_EPS: f64 = 1e-9;

/// Signed area of the triangle (a, b, c); sign gives orientation.
fn orient(a: &P2, b: &P2, c: &P2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// True if `p` (already known collinear with segment `a`-`b`) lies within
/// the segment's bounding box.
fn on_segment(a: &P2, b: &P2, p: &P2) -> bool {
    p.x <= a.x.max(b.x) + ORIENT_EPS
        && p.x >= a.x.min(b.x) - ORIENT_EPS
        && p.y <= a.y.max(b.y) + ORIENT_EPS
        && p.y >= a.y.min(b.y) - ORIENT_EPS
}

/// True if segment bounding boxes overlap — cheap prefilter before the
/// full intersection test.
fn segment_bboxes_overlap(a1: &P2, a2: &P2, b1: &P2, b2: &P2) -> bool {
    let (a_min_x, a_max_x) = (a1.x.min(a2.x), a1.x.max(a2.x));
    let (a_min_y, a_max_y) = (a1.y.min(a2.y), a1.y.max(a2.y));
    let (b_min_x, b_max_x) = (b1.x.min(b2.x), b1.x.max(b2.x));
    let (b_min_y, b_max_y) = (b1.y.min(b2.y), b1.y.max(b2.y));
    a_min_x <= b_max_x && a_max_x >= b_min_x && a_min_y <= b_max_y && a_max_y >= b_min_y
}

/// General segment-segment intersection test — proper crossing or mere
/// touching (shared point, collinear overlap). Adjacency filtering (two
/// segments sharing a ring vertex) is the caller's job; this only answers
/// "do these two segments meet anywhere".
fn segments_intersect(p1: &P2, p2: &P2, p3: &P2, p4: &P2) -> bool {
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);

    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return true;
    }
    // Collinear/touching cases (covers a repeated non-adjacent vertex,
    // i.e. a bowtie pinch, and partial collinear overlap).
    (d1.abs() < ORIENT_EPS && on_segment(p3, p4, p1))
        || (d2.abs() < ORIENT_EPS && on_segment(p3, p4, p2))
        || (d3.abs() < ORIENT_EPS && on_segment(p1, p2, p3))
        || (d4.abs() < ORIENT_EPS && on_segment(p1, p2, p4))
}

/// True if the ring (implicitly closed) crosses itself. Rings here are
/// marching-squares/import output, typically well under 500 vertices, so
/// the O(n^2) segment-pair scan (with a bbox prefilter) is cheap enough.
fn ring_has_self_intersection(ring: &[P2]) -> bool {
    let n = ring.len();
    if n < 4 {
        // A triangle (or fewer vertices) cannot self-intersect: every
        // segment pair is adjacent.
        return false;
    }
    // Safety: i, j, and their (+1)%n successors are all bounded by 0..n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let a1 = &ring[i];
        let a2 = &ring[(i + 1) % n];
        for j in (i + 1)..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if adjacent {
                continue;
            }
            let b1 = &ring[j];
            let b2 = &ring[(j + 1) % n];
            if segment_bboxes_overlap(a1, a2, b1, b2) && segments_intersect(a1, a2, b1, b2) {
                return true;
            }
        }
    }
    false
}

/// Squared point-to-segment distance.
fn point_segment_distance_sq(p: &P2, a: &P2, b: &P2) -> f64 {
    let ab = *b - *a;
    let len_sq = ab.norm_squared();
    if len_sq < f64::EPSILON {
        return (*p - *a).norm_squared();
    }
    let t = ((*p - *a).dot(&ab) / len_sq).clamp(0.0, 1.0);
    let closest = *a + ab * t;
    (*p - closest).norm_squared()
}

/// True if `p` lies within `eps` of any edge of the ring (implicitly closed).
fn ring_near_point(ring: &[P2], p: &P2, eps: f64) -> bool {
    let n = ring.len();
    if n < 2 {
        return false;
    }
    let eps_sq = eps * eps;
    // Safety: i and its (+1)%n successor are bounded by 0..n.
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        let a = &ring[i];
        let b = &ring[(i + 1) % n];
        if point_segment_distance_sq(p, a, b) <= eps_sq {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn square(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    #[test]
    fn test_rectangle_area() {
        let rect = Polygon2::rectangle(0.0, 0.0, 10.0, 5.0);
        assert_relative_eq!(rect.area(), 50.0, epsilon = 1e-10);
    }

    #[test]
    fn test_signed_area_ccw() {
        let sq = square(10.0);
        assert!(
            sq.signed_area() > 0.0,
            "CCW square should have positive area"
        );
        assert_relative_eq!(sq.area(), 100.0, epsilon = 1e-10);
    }

    #[test]
    fn test_signed_area_cw() {
        let pts = vec![
            P2::new(-5.0, -5.0),
            P2::new(-5.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, -5.0),
        ];
        // CW winding
        let area = shoelace_area(&pts);
        assert!(area < 0.0, "CW should have negative area");

        let mut poly = Polygon2::new(pts);
        poly.ensure_winding();
        assert!(
            poly.signed_area() > 0.0,
            "After ensure_winding, should be CCW"
        );
    }

    #[test]
    fn test_polygon_with_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        // CW hole (5x5 centered at 10,10)
        let hole = vec![
            P2::new(7.5, 7.5),
            P2::new(7.5, 12.5),
            P2::new(12.5, 12.5),
            P2::new(12.5, 7.5),
        ];
        let poly = Polygon2::with_holes(outer.exterior, vec![hole]);
        assert_relative_eq!(poly.area(), 400.0 - 25.0, epsilon = 1e-10);
    }

    #[test]
    fn test_perimeter() {
        let sq = square(10.0);
        assert_relative_eq!(sq.perimeter(), 40.0, epsilon = 1e-10);
    }

    #[test]
    fn test_ensure_winding() {
        // Create CW polygon
        let mut poly = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(0.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 0.0),
        ]);
        assert!(poly.signed_area() < 0.0);
        assert!(!poly.has_correct_winding());

        poly.ensure_winding();
        assert!(poly.signed_area() > 0.0);
        assert!(poly.has_correct_winding());
    }

    // --- geo conversion tests ---

    #[test]
    fn test_geo_roundtrip() {
        let original = Polygon2::rectangle(1.0, 2.0, 11.0, 7.0);
        let geo_poly = original.to_geo_polygon();
        let recovered = Polygon2::from_geo_polygon(&geo_poly);

        assert_eq!(recovered.exterior.len(), original.exterior.len());
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    #[test]
    fn test_geo_roundtrip_with_holes() {
        let hole = vec![
            P2::new(3.0, 3.0),
            P2::new(3.0, 5.0),
            P2::new(5.0, 5.0),
            P2::new(5.0, 3.0),
        ];
        let original = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0).exterior,
            vec![hole],
        );
        let geo_poly = original.to_geo_polygon();
        let recovered = Polygon2::from_geo_polygon(&geo_poly);

        assert_eq!(recovered.holes.len(), 1);
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    // --- pline conversion tests ---

    #[test]
    fn test_pline_roundtrip() {
        let original = square(20.0);
        let pline = original.exterior_to_pline();
        assert_eq!(pline.vertex_count(), 4);
        assert!(pline.is_closed());

        let recovered = Polygon2::from_pline(&pline);
        assert_eq!(recovered.exterior.len(), 4);
        assert_relative_eq!(recovered.area(), original.area(), epsilon = 1e-10);
    }

    // --- offset tests ---

    #[test]
    fn test_offset_inward_square() {
        let sq = square(20.0); // 20x20 centered at origin
        let results = offset_polygon(&sq, 2.0); // inward by 2

        assert_eq!(
            results.len(),
            1,
            "Single inward offset should produce one polygon"
        );
        let inner = &results[0];

        // Area should be approximately (20 - 2*2)^2 = 256
        // cavalier_contours uses round joins, so corners are rounded.
        // Area will be slightly larger than a pure rectangle but close.
        let expected_rect_area = 16.0 * 16.0; // 256
        assert!(
            inner.area() > expected_rect_area * 0.95,
            "Inner area {} should be close to {} (rounded corners make it slightly larger)",
            inner.area(),
            expected_rect_area
        );
        assert!(inner.area() < sq.area());
    }

    #[test]
    fn test_offset_collapse() {
        let sq = square(10.0); // 10x10
        let results = offset_polygon(&sq, 6.0); // inward by 6, exceeds half-width

        assert!(
            results.is_empty(),
            "Offset exceeding half-width should collapse: got {} polygons",
            results.len()
        );
    }

    #[test]
    fn test_offset_outward_square() {
        let sq = square(10.0);
        let results = offset_polygon(&sq, -2.0); // outward by 2

        assert_eq!(results.len(), 1);
        assert!(
            results[0].area() > sq.area(),
            "Outward offset should increase area"
        );
    }

    #[test]
    fn test_offset_non_convex_l_shape() {
        // L-shaped polygon (concave)
        let l_shape = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(20.0, 0.0),
            P2::new(20.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(0.0, 20.0),
        ]);
        assert_relative_eq!(l_shape.area(), 300.0, epsilon = 1e-10);

        // Small inward offset should produce one polygon
        let results = offset_polygon(&l_shape, 1.0);
        assert!(
            !results.is_empty(),
            "Small offset of L-shape should not collapse"
        );
        let total_area: f64 = results.iter().map(|p| p.area()).sum();
        assert!(total_area < l_shape.area());
        assert!(
            total_area > 200.0,
            "Area {} too small for 1mm inward offset",
            total_area
        );
    }

    #[test]
    fn test_offset_polygon_with_hole() {
        // 30x30 square with 10x10 hole in center
        let hole = vec![
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(20.0, 20.0),
            P2::new(20.0, 10.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 30.0, 30.0).exterior,
            vec![hole],
        );
        assert_relative_eq!(poly.area(), 900.0 - 100.0, epsilon = 1e-10);

        // Inward offset by 2mm should shrink exterior and grow hole
        let results = offset_polygon(&poly, 2.0);
        assert!(
            !results.is_empty(),
            "Offset of polygon-with-hole should not collapse"
        );
        let total_area: f64 = results.iter().map(|p| p.area()).sum();
        assert!(
            total_area < poly.area(),
            "Offset area {} should be less than original {}",
            total_area,
            poly.area()
        );
    }

    #[test]
    fn test_offset_preserves_center() {
        // Symmetric square centered at origin - offset should stay centered
        let sq = square(20.0);
        let results = offset_polygon(&sq, 3.0);
        assert_eq!(results.len(), 1);

        let inner = &results[0];
        // Centroid of offset result should be near origin
        let cx: f64 = inner.exterior.iter().map(|p| p.x).sum::<f64>() / inner.exterior.len() as f64;
        let cy: f64 = inner.exterior.iter().map(|p| p.y).sum::<f64>() / inner.exterior.len() as f64;
        assert!(cx.abs() < 0.5, "Centroid x={} should be near 0", cx);
        assert!(cy.abs() < 0.5, "Centroid y={} should be near 0", cy);
    }

    #[test]
    fn test_offset_zero_distance() {
        let sq = square(10.0);
        let results = offset_polygon(&sq, 0.0);
        assert_eq!(results.len(), 1);
        assert_relative_eq!(results[0].area(), sq.area(), epsilon = 0.1);
    }

    /// The cascade that replaced `pocket_offsets` (retired at Checkpoint D:
    /// no production caller, and it duplicated
    /// `pocket_contours_with_cancel`'s loop *without* its cancel hook — it is
    /// the function that ran 13 minutes and 386 MB in the M5 study).
    #[test]
    fn ring_set_cascade_shrinks_layer_by_layer() {
        let sq = square(20.0); // 20x20
        let mut rings = OffsetRingSet::from_polygon(&sq);
        let mut layers: Vec<Vec<Polygon2>> = Vec::new();
        for _ in 0..10 {
            rings = rings.offset(3.0);
            if rings.is_empty() {
                break;
            }
            layers.push(rings.to_polygons(FlattenPolicy::untoleranced()));
        }

        // Half-width 10, stepover 3 → 3 rings before collapse.
        assert!(
            (2..=4).contains(&layers.len()),
            "Expected 2-4 layers for 20x20 square with 3mm stepover, got {}",
            layers.len()
        );

        let mut prev_area: f64 = sq.area();
        for (i, layer) in layers.iter().enumerate() {
            let layer_area: f64 = layer.iter().map(|p| p.area()).sum();
            assert!(
                layer_area < prev_area,
                "Layer {i} area ({layer_area}) should be less than previous ({prev_area})"
            );
            prev_area = layer_area;
        }
    }

    /// The cascade must not inflate on the shape class that made it inflate:
    /// a boundary with reflex corners, offset inward over and over.
    #[test]
    fn ring_set_cascade_does_not_inflate_on_a_reflex_boundary() {
        // A plus/cross — four reflex corners, the §8 pocket fixture.
        let (a, b) = (20.0, 60.0);
        let cross = Polygon2::new(vec![
            P2::new(a, 0.0),
            P2::new(b, 0.0),
            P2::new(b, a),
            P2::new(b + a, a),
            P2::new(b + a, b),
            P2::new(b, b),
            P2::new(b, b + a),
            P2::new(a, b + a),
            P2::new(a, b),
            P2::new(0.0, b),
            P2::new(0.0, a),
            P2::new(a, a),
        ]);
        let mut rings = OffsetRingSet::from_polygon(&cross);
        let mut worst = 0usize;
        let mut count = 0usize;
        for _ in 0..40 {
            rings = rings.offset(0.5);
            if rings.is_empty() {
                break;
            }
            count += 1;
            worst = worst.max(rings.vertex_count());
        }
        assert!(count >= 30, "cross should survive ~40 rings, got {count}");
        // Production's chord-per-ring cascade reaches 42 148 vertices here
        // and does not finish in 20 seconds. Two hundred is generous.
        assert!(
            worst < 200,
            "arc cascade must stay bounded on a reflex boundary; peak {worst} vertices"
        );
    }

    /// The flatten policy is the only thing that decides ring density, and it
    /// answers to the operation's tolerance.
    #[test]
    fn flatten_policy_is_derived_from_the_op_tolerance_and_clamped() {
        let p = FlattenPolicy::from_chord_tolerance(0.100);
        assert_relative_eq!(p.max_deviation_mm(), 0.010, epsilon = 1e-12);
        // Absurdly tight and absurdly loose tolerances both clamp.
        assert_relative_eq!(
            FlattenPolicy::from_chord_tolerance(1e-9).max_deviation_mm(),
            FlattenPolicy::MIN_DEVIATION_MM,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            FlattenPolicy::from_chord_tolerance(10.0).max_deviation_mm(),
            FlattenPolicy::MAX_DEVIATION_MM,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            FlattenPolicy::from_chord_tolerance(f64::NAN).max_deviation_mm(),
            FlattenPolicy::UNTOLERANCED_MM,
            epsilon = 1e-12
        );
        // A tighter budget can only add points, never move the ring off the
        // arc it is approximating.
        let sq = square(40.0);
        let ring = OffsetRingSet::from_polygon(&sq).offset(-3.0);
        let coarse = ring.to_polygons(FlattenPolicy::from_chord_tolerance(0.5));
        let fine = ring.to_polygons(FlattenPolicy::from_chord_tolerance(0.01));
        let coarse_n: usize = coarse.iter().map(|p| p.exterior.len()).sum();
        let fine_n: usize = fine.iter().map(|p| p.exterior.len()).sum();
        assert!(
            fine_n > coarse_n,
            "a tighter flatten budget must produce more points: {coarse_n} -> {fine_n}"
        );
    }

    /// Holes have to survive the cascade, and stay attached to the boundary
    /// that contains them.
    #[test]
    fn ring_set_cascade_keeps_holes_with_their_container() {
        let mut sq = square(60.0);
        // A CW hole, per `Polygon2`'s contract.
        sq.holes.push(vec![
            P2::new(-10.0, -10.0),
            P2::new(-10.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, -10.0),
        ]);
        let rings = OffsetRingSet::from_polygon(&sq).offset(2.0);
        let polys = rings.to_polygons(FlattenPolicy::untoleranced());
        assert_eq!(polys.len(), 1, "one boundary in, one boundary out");
        assert_eq!(
            polys.first().map(|p| p.holes.len()),
            Some(1),
            "the hole must still be a hole of its container"
        );
    }

    // --- containment detection tests ---

    #[test]
    fn test_containment_rect_with_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let inner = Polygon2::rectangle(15.0, 15.0, 35.0, 35.0);

        let result = detect_containment(vec![outer, inner]);
        assert_eq!(
            result.len(),
            1,
            "Inner should become a hole, not a separate polygon"
        );
        assert_eq!(result[0].holes.len(), 1, "Outer should have 1 hole");
        assert_relative_eq!(result[0].area(), 50.0 * 50.0 - 20.0 * 20.0, epsilon = 1.0);
    }

    #[test]
    fn test_containment_no_nesting() {
        let a = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let b = Polygon2::rectangle(30.0, 0.0, 50.0, 20.0);

        let result = detect_containment(vec![a, b]);
        assert_eq!(result.len(), 2, "Separate polygons should stay separate");
        assert!(result[0].holes.is_empty());
        assert!(result[1].holes.is_empty());
    }

    #[test]
    fn test_containment_multiple_holes() {
        let outer = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let hole1 = Polygon2::rectangle(10.0, 10.0, 30.0, 30.0);
        let hole2 = Polygon2::rectangle(50.0, 50.0, 70.0, 70.0);

        let result = detect_containment(vec![hole1, outer, hole2]);
        assert_eq!(result.len(), 1, "Both inner rects should become holes");
        assert_eq!(result[0].holes.len(), 2);
    }

    #[test]
    fn test_containment_preserves_winding() {
        let outer = Polygon2::rectangle(0.0, 0.0, 50.0, 50.0);
        let inner = Polygon2::rectangle(10.0, 10.0, 40.0, 40.0);

        let result = detect_containment(vec![outer, inner]);
        assert!(result[0].signed_area() > 0.0, "Outer should be CCW");
        // Holes should be CW (negative area)
        let hole_area = shoelace_area(&result[0].holes[0]);
        assert!(hole_area < 0.0, "Hole should be CW, got area {}", hole_area);
    }

    #[test]
    fn test_containment_island_in_hole_stays_top_level() {
        // Even-odd depth 2: an island inside a hole is solid material again
        // and must stay a top-level polygon, not vanish as a second hole.
        let outer = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let ring_hole = Polygon2::rectangle(20.0, 20.0, 80.0, 80.0);
        let island = Polygon2::rectangle(40.0, 40.0, 60.0, 60.0);

        let result = detect_containment(vec![island, outer, ring_hole]);
        assert_eq!(result.len(), 2, "depth-2 island must stay top-level");
        assert_eq!(result[0].holes.len(), 1, "outer keeps the ring hole");
        assert!(result[1].holes.is_empty());
        assert_relative_eq!(result[1].area(), 20.0 * 20.0, epsilon = 1.0);
    }

    #[test]
    fn test_containment_depth_three_nests_hole_in_island() {
        // Even-odd depth 3: a ring inside the island is the ISLAND's hole,
        // not the outermost polygon's.
        let outer = Polygon2::rectangle(0.0, 0.0, 100.0, 100.0);
        let ring_hole = Polygon2::rectangle(20.0, 20.0, 80.0, 80.0);
        let island = Polygon2::rectangle(40.0, 40.0, 60.0, 60.0);
        let inner_hole = Polygon2::rectangle(45.0, 45.0, 55.0, 55.0);

        let result = detect_containment(vec![inner_hole, island, outer, ring_hole]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].holes.len(), 1, "outer keeps the ring hole");
        assert_eq!(
            result[1].holes.len(),
            1,
            "depth-3 ring must become the island's hole"
        );
    }

    #[test]
    fn test_containment_single_polygon() {
        let poly = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let result = detect_containment(vec![poly]);
        assert_eq!(result.len(), 1);
        assert!(result[0].holes.is_empty());
    }

    #[test]
    fn test_offset_two_separate_regions() {
        // Two separate rectangles, each with a hole. Offset inward.
        // Verify each polygon keeps its own hole after re-pairing.
        let rect1 = Polygon2::rectangle(0.0, 0.0, 30.0, 30.0);
        let hole1 = vec![
            P2::new(10.0, 10.0),
            P2::new(10.0, 20.0),
            P2::new(20.0, 20.0),
            P2::new(20.0, 10.0),
        ]; // CW
        let rect2 = Polygon2::rectangle(50.0, 0.0, 80.0, 30.0);
        let hole2 = vec![
            P2::new(60.0, 10.0),
            P2::new(60.0, 20.0),
            P2::new(70.0, 20.0),
            P2::new(70.0, 10.0),
        ]; // CW

        let poly1 = Polygon2::with_holes(rect1.exterior, vec![hole1]);
        let poly2 = Polygon2::with_holes(rect2.exterior, vec![hole2]);

        // Offset each separately — both should retain their holes
        let r1 = offset_polygon(&poly1, 1.0);
        let r2 = offset_polygon(&poly2, 1.0);
        assert!(!r1.is_empty(), "First offset should succeed");
        assert!(!r2.is_empty(), "Second offset should succeed");

        // Each result should have holes
        let r1_holes: usize = r1.iter().map(|p| p.holes.len()).sum();
        let r2_holes: usize = r2.iter().map(|p| p.holes.len()).sum();
        assert!(
            r1_holes >= 1,
            "First polygon should keep its hole, got {} holes",
            r1_holes
        );
        assert!(
            r2_holes >= 1,
            "Second polygon should keep its hole, got {} holes",
            r2_holes
        );
    }

    #[test]
    fn test_offset_single_region_two_holes() {
        // One rect with two holes — both should stay attached after offset.
        let hole1 = vec![
            P2::new(5.0, 5.0),
            P2::new(5.0, 10.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 5.0),
        ]; // CW
        let hole2 = vec![
            P2::new(20.0, 5.0),
            P2::new(20.0, 10.0),
            P2::new(25.0, 10.0),
            P2::new(25.0, 5.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(0.0, 0.0, 30.0, 15.0).exterior,
            vec![hole1, hole2],
        );

        let results = offset_polygon(&poly, 1.0);
        assert!(!results.is_empty(), "Offset should succeed");

        let total_holes: usize = results.iter().map(|p| p.holes.len()).sum();
        assert!(
            total_holes >= 2,
            "Both holes should survive offset, got {} holes",
            total_holes
        );
    }

    #[test]
    fn test_point_in_polygon_basic() {
        let square = vec![
            P2::new(0.0, 0.0),
            P2::new(10.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(&P2::new(5.0, 5.0), &square));
        assert!(!point_in_polygon(&P2::new(15.0, 5.0), &square));
        assert!(!point_in_polygon(&P2::new(-1.0, 5.0), &square));
    }

    #[test]
    fn test_largest_by_area_empty() {
        let polys: Vec<Polygon2> = Vec::new();
        assert!(largest_by_area(&polys).is_none());
    }

    #[test]
    fn test_largest_by_area_single() {
        let polys = vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)];
        let picked = largest_by_area(&polys).expect("single element is Some");
        assert_relative_eq!(picked.area(), 100.0);
    }

    #[test]
    fn test_largest_by_area_picks_largest() {
        let small = Polygon2::rectangle(0.0, 0.0, 5.0, 5.0);
        let large = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let mid = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let polys = vec![small, large, mid];
        let picked = largest_by_area(&polys).expect("non-empty slice is Some");
        assert_relative_eq!(picked.area(), 400.0);
    }

    #[test]
    fn test_largest_by_area_tie_picks_last() {
        // Documents tie behavior: equal-area candidates resolve to the
        // *last* one in the slice (matches `Iterator::max_by` semantics).
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(100.0, 100.0, 110.0, 110.0); // same area, different position
        let polys = vec![a, b];
        let picked = largest_by_area(&polys).expect("non-empty slice is Some");
        assert_relative_eq!(picked.exterior[0].x, 100.0);
    }

    // --- self-intersection detection & repair (R1.5) ---

    #[test]
    fn test_bowtie_self_intersection() {
        // Figure-eight / bowtie: edges (0,0)-(10,10) and (10,0)-(0,10) cross
        // at the center; the other two edges don't.
        let bowtie = Polygon2::new(vec![
            P2::new(0.0, 0.0),
            P2::new(10.0, 10.0),
            P2::new(10.0, 0.0),
            P2::new(0.0, 10.0),
        ]);
        assert!(bowtie.has_self_intersection());

        let repaired = bowtie.repaired();
        assert!(
            !repaired.is_empty(),
            "repair should produce at least one simple polygon"
        );
        for piece in &repaired {
            assert!(
                !piece.has_self_intersection(),
                "repaired piece should be simple"
            );
        }
    }

    #[test]
    fn test_simple_square_no_self_intersection() {
        let sq = square(10.0);
        assert!(!sq.has_self_intersection());

        let repaired = sq.repaired();
        assert_eq!(
            repaired.len(),
            1,
            "non-intersecting input passes through unchanged"
        );
        assert_relative_eq!(repaired[0].area(), sq.area(), epsilon = 1e-9);
    }

    #[test]
    fn test_offset_polygon_r1_panic_repro_does_not_panic() {
        // The captured WANAKA terrain-slice input that used to panic
        // cavalier_contours (see offset_polygon's R1 doc comment) is
        // already exercised end-to-end by the integration test
        // `tests/offset_polygon_degenerate_inputs_r1.rs` (loads the same
        // asset via `common::repo_root`). This unit-level copy just
        // confirms the asset still parses from inside the crate's own
        // test harness and the offset call — now routed through the
        // R1.5 self-intersection repair first — still returns without
        // panicking.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test_data/cavalier_panic_polygon_r1.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            // Asset not present in this checkout — nothing to assert.
            return;
        };
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse capture");
        let distance = v["distance"].as_f64().expect("distance");
        let ring = |val: &serde_json::Value| -> Vec<P2> {
            val.as_array()
                .expect("ring array")
                .iter()
                .map(|p| P2::new(p[0].as_f64().expect("x"), p[1].as_f64().expect("y")))
                .collect()
        };
        let mut poly = Polygon2::new(ring(&v["exterior"]));
        poly.closed = v["closed"].as_bool().unwrap_or(true);
        poly.holes = v["holes"]
            .as_array()
            .expect("holes array")
            .iter()
            .map(ring)
            .collect();

        let result = offset_polygon(&poly, distance);
        // The bar is "no panic" — result may legitimately be empty.
        tracing::info!(count = result.len(), "R1 panic-repro offset result");
    }

    // --- boolean ops (R1.6) ---

    #[test]
    fn test_union_overlapping_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.union(&b);
        assert_eq!(
            result.len(),
            1,
            "overlapping squares should merge into one polygon"
        );
        assert_relative_eq!(result[0].area(), 175.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_intersection_overlapping_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.intersection(&b);
        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].area(), 25.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_difference_overlapping_squares_l_shape() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = a.difference(&b);
        assert_eq!(result.len(), 1);
        assert_relative_eq!(result[0].area(), 75.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_union_disjoint_squares() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(20.0, 20.0, 30.0, 30.0);
        let result = a.union(&b);
        assert_eq!(result.len(), 2, "disjoint squares stay separate polygons");
        for poly in &result {
            assert!(poly.has_correct_winding());
        }
    }

    #[test]
    fn test_difference_produces_hole() {
        let outer = Polygon2::rectangle(0.0, 0.0, 20.0, 20.0);
        let inner = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0);
        let result = outer.difference(&inner);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].holes.len(), 1);
        assert_relative_eq!(result[0].area(), 400.0 - 100.0, epsilon = 1e-9);
        assert!(result[0].has_correct_winding());
    }

    #[test]
    fn test_union_all_merges_set() {
        let a = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let b = Polygon2::rectangle(5.0, 5.0, 15.0, 15.0); // overlaps a
        let c = Polygon2::rectangle(30.0, 30.0, 40.0, 40.0); // disjoint
        let result = Polygon2::union_all(&[a, b, c]);
        assert_eq!(result.len(), 2, "a+b merge, c stays separate");
        let total_area: f64 = result.iter().map(Polygon2::area).sum();
        assert_relative_eq!(total_area, 175.0 + 100.0, epsilon = 1e-9);
        for poly in &result {
            assert!(poly.has_correct_winding());
        }
    }

    #[test]
    fn test_union_all_empty() {
        assert!(Polygon2::union_all(&[]).is_empty());
    }

    // --- contains_point_eps (R1.7) ---

    #[test]
    fn test_contains_point_eps_on_edge() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(sq.contains_point_eps(&P2::new(10.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_just_outside() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(sq.contains_point_eps(&P2::new(10.0 + eps / 2.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_far_outside() {
        let sq = Polygon2::rectangle(0.0, 0.0, 10.0, 10.0);
        let eps = 1e-6;
        assert!(!sq.contains_point_eps(&P2::new(10.0 + eps * 10.0, 5.0), eps));
    }

    #[test]
    fn test_contains_point_eps_inside_hole_near_edge() {
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(
            Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0).exterior,
            vec![hole],
        );
        let eps = 1e-6;
        // Strictly inside the hole (excluded by strict containment) but
        // within eps of the hole's edge.
        let p = P2::new(3.0 - eps / 2.0, 0.0);
        assert!(
            !poly.contains_point(&p),
            "point should be excluded by strict containment"
        );
        assert!(poly.contains_point_eps(&p, eps));
    }
}
