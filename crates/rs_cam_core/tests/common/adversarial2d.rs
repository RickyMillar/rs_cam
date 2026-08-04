//! **R2's hostile-input bench for the 2D operation families** — procedural
//! fixture generators, per-class non-vacuity measures, a wall-clock/RSS
//! watchdog, a cancellation seam, and an SVG renderer for input *and* output.
//!
//! # Why this exists
//!
//! The 3D stack has had adversarial fixtures since M4/M5. The 2D stack —
//! pocket, adaptive, profile, trace, zigzag, inlay, v-carve, rest and the
//! 2D-driven drill — never has. A **twelve-vertex reflex cross** was enough
//! to put `pocket_offsets` at 13 minutes and 386 MB until the arc cascade
//! landed (`e3427f8`, 2026-08-03). Twelve vertices. Nothing in the suite
//! would have found it, because every 2D fixture in the tree was a square,
//! a circle, or a captured real part.
//!
//! # What a fixture has to prove about itself
//!
//! A concave fixture that has no reflex corners left after tool-radius
//! compensation cannot decide a reflex defect, and a "thin slot" wider than
//! the tool is just a pocket. So every [`Fixture`] carries a
//! [`Fixture::class`] and [`Fixture::assert_contains_mechanism`] measures
//! the geometry and fails if the named mechanism is absent. **The fixture is
//! tested before the operation is.** ([`measure`] is also the thing the
//! spec document's parameter tables are generated from, so the numbers in
//! the doc and the numbers in the gate cannot drift.)
//!
//! # What is deliberately NOT here
//!
//! Reflex comb, dendrite, rosette, hole grid and sub-epsilon short edges
//! already exist in [`super::offset_lab`], authored for M5 and pinned by
//! its measurements. They are re-exported ([`comb`], [`dendrite`], …), not
//! re-written: a second copy of a fixture is a second thing to keep in sync,
//! and M5's numbers are quoted in `CHECKPOINT_D_EVIDENCE.md` against *those*
//! generators.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::{MoveType, Toolpath};

// M5's fixtures, borrowed rather than re-authored. See the module header.
pub use super::offset_lab::{PLINE_POS_EQUAL_EPS, comb, dendrite, holed, rosette, short_edges};

// ---------------------------------------------------------------------------
// Fixture model
// ---------------------------------------------------------------------------

/// The geometric mechanism a fixture is built to contain. One variant per
/// hostile class the R2 brief names; [`assert_contains_mechanism`] has one
/// arm per variant and there is no catch-all, so a new class cannot be added
/// without also stating how to prove it is present.
///
/// [`assert_contains_mechanism`]: Fixture::assert_contains_mechanism
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Reflex (interior angle > 180°) corners that survive erosion.
    Reflex,
    /// Reflex at two scales — notches cut into the fingers of a comb.
    Dendrite,
    /// An island inside a hole inside an island: containment nesting ≥ 2.
    HolesInHoles,
    /// Several polygons with no containment relation at all.
    DisconnectedIslands,
    /// Two non-adjacent walls closer together than the tool, down to below
    /// cavalier's own position epsilon.
    NearCoincidentWalls,
    /// Segments shorter than `PLINE_POS_EQUAL_EPS`.
    ShortEdges,
    /// Vertices whose deviation from their neighbours' chord is far below
    /// any operation tolerance — removable in the shape domain, load-bearing
    /// in the sampling domain.
    NearCollinear,
    /// Islands with less area than the tool's own footprint.
    TinyIslands,
    /// A slot narrower than the tool diameter (and its just-wider control).
    ThinSlot,
    /// Curvature radius below the tool radius — the tool physically cannot
    /// reproduce the corner.
    HighCurvature,
    /// Input that violates the `Polygon2` contract: self-intersection,
    /// degenerate ring, non-finite coordinate, wrong winding, open path.
    InvalidContour,
}

impl Class {
    pub fn id(self) -> &'static str {
        match self {
            Self::Reflex => "reflex",
            Self::Dendrite => "dendrite",
            Self::HolesInHoles => "holes-in-holes",
            Self::DisconnectedIslands => "islands",
            Self::NearCoincidentWalls => "near-coincident",
            Self::ShortEdges => "short-edges",
            Self::NearCollinear => "near-collinear",
            Self::TinyIslands => "tiny-islands",
            Self::ThinSlot => "thin-slot",
            Self::HighCurvature => "high-curvature",
            Self::InvalidContour => "invalid",
        }
    }
}

/// Whether the fixture is a legal `Polygon2` set or a deliberate contract
/// violation. An invalid fixture's bar is *never a panic and never a silent
/// success*, not *produces a good toolpath*.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Validity {
    /// Satisfies the `Polygon2` contract: ≥ 3 finite vertices, CCW exterior,
    /// CW holes, no self-intersection.
    Valid,
    /// Violates it, in the stated way.
    Invalid(&'static str),
}

/// What the campaign is allowed to conclude from a run on this fixture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Expectation {
    /// There is removable material reachable by the stated tool, so a
    /// successful generate that emits **no cutting move** is a defect, not a
    /// result. This is the R2 acceptance gate "a test fails if an operation
    /// returns success with an unexpected empty path".
    MustCut,
    /// The geometry legitimately admits an empty result (the feature is
    /// smaller than the tool). Empty is fine; a panic is not.
    MayBeEmpty,
}

/// One hostile shape plus everything needed to judge what an operation did
/// with it.
pub struct Fixture {
    pub name: &'static str,
    pub class: Class,
    /// What this fixture isolates. Printed in every table and copied into
    /// `ADVERSARIAL_2D_FIXTURE_SPEC.md`.
    pub what: &'static str,
    pub polys: Vec<Polygon2>,
    /// The tool diameter (mm) the fixture is authored against — the scale
    /// that makes its feature hostile. A "thin slot" is only thin relative
    /// to a tool.
    pub tool_d: f64,
    pub validity: Validity,
    pub expectation: Expectation,
    /// Operations this fixture must NOT be run against, with the reason.
    ///
    /// **This exists for exactly one situation: the cell provably does not
    /// terminate.** A non-terminating cell cannot be "tested" by a campaign
    /// whose wall-clock ceiling is checked *after* the call returns — it just
    /// hangs the suite and takes the machine's memory with it (the first R2
    /// run reached 22.9 GB RSS). The divergence is instead proved by a
    /// *bounded* probe that caps the iteration count and measures the
    /// trend, which is a better instrument anyway: it shows the direction,
    /// not just the absence of an answer.
    ///
    /// Never use this to skip a cell that is merely slow, or one that fails.
    pub skip_ops: &'static [(&'static str, &'static str)],
}

impl Fixture {
    pub fn measure(&self) -> Mechanism {
        measure(&self.polys)
    }

    /// `Some(reason)` when this `(op, fixture)` cell must not be run.
    pub fn skip_reason(&self, op: &str) -> Option<&'static str> {
        self.skip_ops
            .iter()
            .find(|(o, _)| *o == op)
            .map(|(_, why)| *why)
    }

    /// **Non-vacuity.** Fail if the geometry does not actually contain the
    /// mechanism this fixture claims. Called by the campaign before any
    /// operation runs, and by `adversarial_2d_fixtures_contain_their_mechanism`
    /// on its own.
    #[track_caller]
    pub fn assert_contains_mechanism(&self) {
        let m = self.measure();
        let r = self.tool_d * 0.5;
        match self.class {
            Class::Reflex => assert!(
                m.reflex_corners >= 4,
                "{}: claims reflex corners, measured {}",
                self.name,
                m.reflex_corners
            ),
            Class::Dendrite => assert!(
                m.reflex_corners >= 16,
                "{}: claims two-scale reflex branching, measured {} reflex corners",
                self.name,
                m.reflex_corners
            ),
            // ≥ 2 is "at least one hole level", i.e. the `Shape::parallel_offset`
            // path rather than `Polyline::parallel_offset`. A true
            // island-inside-a-hole is depth ≥ 3, and the campaign asserts
            // separately that at least one fixture in this class reaches it —
            // `holed-9` is a polygon WITH holes (depth 2), which is a
            // different and also necessary case.
            Class::HolesInHoles => assert!(
                m.nesting_depth >= 2,
                "{}: claims at least one hole level, measured nesting depth {}",
                self.name,
                m.nesting_depth
            ),
            Class::DisconnectedIslands => assert!(
                m.components >= 3,
                "{}: claims disconnected islands, measured {} top-level components",
                self.name,
                m.components
            ),
            Class::NearCoincidentWalls => assert!(
                m.min_nonadjacent_gap_mm < self.tool_d,
                "{}: claims walls closer than the {:.3} mm tool, measured a \
                 {:.3e} mm minimum non-adjacent gap",
                self.name,
                self.tool_d,
                m.min_nonadjacent_gap_mm
            ),
            Class::ShortEdges => assert!(
                m.segments_below_pos_eps >= 100,
                "{}: claims sub-epsilon segments, measured {} below {:.0e}",
                self.name,
                m.segments_below_pos_eps,
                PLINE_POS_EQUAL_EPS
            ),
            Class::NearCollinear => assert!(
                m.near_collinear_1um >= 20,
                "{}: claims near-collinear vertices, measured {} under 1 µm \
                 chord deviation",
                self.name,
                m.near_collinear_1um
            ),
            Class::TinyIslands => assert!(
                m.min_area_mm2 < std::f64::consts::PI * r * r,
                "{}: claims islands smaller than the tool footprint \
                 ({:.4} mm²), measured a {:.4} mm² minimum",
                self.name,
                std::f64::consts::PI * r * r,
                m.min_area_mm2
            ),
            // `<=`, not `<`: the exactly-tool-width slot is the marginal case
            // this class exists to separate from the impossible one, and it
            // sits precisely on the bound.
            Class::ThinSlot => assert!(
                m.min_nonadjacent_gap_mm <= self.tool_d,
                "{}: claims a slot no wider than the {:.3} mm tool, measured a \
                 {:.4} mm minimum wall gap",
                self.name,
                self.tool_d,
                m.min_nonadjacent_gap_mm
            ),
            Class::HighCurvature => assert!(
                m.min_curvature_radius_mm < r,
                "{}: claims curvature tighter than the {:.4} mm tool radius, \
                 measured a {:.4} mm minimum",
                self.name,
                r,
                m.min_curvature_radius_mm
            ),
            Class::InvalidContour => assert!(
                matches!(self.validity, Validity::Invalid(_)),
                "{}: an InvalidContour fixture must declare Validity::Invalid",
                self.name
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Mechanism measurement — the fixtures' own oracle
// ---------------------------------------------------------------------------

/// Everything [`assert_contains_mechanism`] and the spec tables read.
///
/// All measures are taken on the fixture **as authored**, i.e. before any
/// tool-radius compensation. That is deliberate: it describes what the
/// operation is handed, which is the input whose handling is under test.
///
/// [`assert_contains_mechanism`]: Fixture::assert_contains_mechanism
#[derive(Clone, Copy, Debug, Default)]
pub struct Mechanism {
    pub total_vertices: usize,
    pub rings: usize,
    /// Corners where the boundary turns *into* the material (interior angle
    /// > 180°), counted on every ring with its own winding taken into
    /// account, so a hole's concavity is not miscounted as convexity.
    pub reflex_corners: usize,
    pub min_segment_mm: f64,
    pub segments_below_pos_eps: usize,
    /// Vertices whose perpendicular distance from the chord through their
    /// neighbours is under 1 µm.
    pub near_collinear_1um: usize,
    /// Smallest distance between two **non-adjacent** edges of the same ring
    /// or of two different rings — the width of the narrowest passage.
    pub min_nonadjacent_gap_mm: f64,
    /// Smallest circumradius over consecutive vertex triples.
    pub min_curvature_radius_mm: f64,
    /// Top-level polygons: those not contained by any other polygon.
    pub components: usize,
    /// Containment nesting levels present (1 = flat set, 2 = polygon with a
    /// hole, 3 = island inside a hole).
    pub nesting_depth: usize,
    /// Smallest absolute ring area in the set (exterior or hole).
    pub min_area_mm2: f64,
    pub self_intersecting: bool,
    pub non_finite_vertices: usize,
    pub open_paths: usize,
}

pub fn measure(polys: &[Polygon2]) -> Mechanism {
    let mut m = Mechanism {
        min_segment_mm: f64::INFINITY,
        min_nonadjacent_gap_mm: f64::INFINITY,
        min_curvature_radius_mm: f64::INFINITY,
        min_area_mm2: f64::INFINITY,
        ..Mechanism::default()
    };
    let mut all_rings: Vec<Vec<P2>> = Vec::new();
    for p in polys {
        if !p.closed {
            m.open_paths += 1;
        }
        if p.has_self_intersection() {
            m.self_intersecting = true;
        }
        all_rings.push(p.exterior.clone());
        for h in &p.holes {
            all_rings.push(h.clone());
        }
    }
    m.rings = all_rings.len();
    for ring in &all_rings {
        m.total_vertices += ring.len();
        m.non_finite_vertices += ring
            .iter()
            .filter(|p| !p.x.is_finite() || !p.y.is_finite())
            .count();
        if ring.len() < 3 {
            continue;
        }
        let area = rs_cam_core::polygon::shoelace_area(ring);
        if area.abs().is_finite() {
            m.min_area_mm2 = m.min_area_mm2.min(area.abs());
        }
        m.reflex_corners += reflex_corner_count(ring);
        let n = ring.len();
        for i in 0..n {
            let a = ring[i];
            let b = ring[(i + 1) % n];
            let len = (b.x - a.x).hypot(b.y - a.y);
            if len.is_finite() {
                m.min_segment_mm = m.min_segment_mm.min(len);
                if len < PLINE_POS_EQUAL_EPS {
                    m.segments_below_pos_eps += 1;
                }
            }
            let prev = ring[(i + n - 1) % n];
            let dev = super::offset_lab::point_segment_distance(&a, &prev, &b);
            if dev.is_finite() && dev < 1.0e-3 {
                m.near_collinear_1um += 1;
            }
            let r = circumradius(&prev, &a, &b);
            if r.is_finite() {
                m.min_curvature_radius_mm = m.min_curvature_radius_mm.min(r);
            }
        }
    }
    m.min_nonadjacent_gap_mm = min_nonadjacent_gap(&all_rings);
    let (components, depth) = containment_shape(polys);
    m.components = components;
    m.nesting_depth = depth;
    if !m.min_area_mm2.is_finite() {
        m.min_area_mm2 = 0.0;
    }
    m
}

/// Reflex corners of one ring, orientation-aware: a CCW ring turns left at a
/// convex corner, a CW ring turns right, so the sign test is taken against
/// the ring's own signed area rather than an assumed winding.
pub fn reflex_corner_count(ring: &[P2]) -> usize {
    let n = ring.len();
    if n < 3 {
        return 0;
    }
    let sign = if rs_cam_core::polygon::shoelace_area(ring) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut count = 0;
    for i in 0..n {
        let prev = ring[(i + n - 1) % n];
        let cur = ring[i];
        let next = ring[(i + 1) % n];
        let cross = (cur.x - prev.x) * (next.y - cur.y) - (cur.y - prev.y) * (next.x - cur.x);
        // A near-zero cross is a straight run, not a corner of either kind.
        if cross.is_finite() && sign * cross < -1.0e-12 {
            count += 1;
        }
    }
    count
}

/// Circumradius of three points — the local curvature radius. `INFINITY` for
/// collinear or degenerate triples, so a `min` over it ignores straight runs.
pub fn circumradius(a: &P2, b: &P2, c: &P2) -> f64 {
    let ab = (b.x - a.x).hypot(b.y - a.y);
    let bc = (c.x - b.x).hypot(c.y - b.y);
    let ca = (a.x - c.x).hypot(a.y - c.y);
    let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let area2 = cross.abs();
    if area2 <= 1.0e-18 {
        return f64::INFINITY;
    }
    ab * bc * ca / (2.0 * area2)
}

/// Minimum distance between two edges that are not neighbours in the same
/// ring. O(total_edges²); the largest fixture here is ~1600 edges, which is
/// 2.6 M segment pairs — under a second in debug and worth it, because the
/// alternative is asserting a fixture property the fixture does not have.
pub fn min_nonadjacent_gap(rings: &[Vec<P2>]) -> f64 {
    let mut edges: Vec<(usize, usize, P2, P2)> = Vec::new();
    for (ri, ring) in rings.iter().enumerate() {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        for i in 0..n {
            edges.push((ri, i, ring[i], ring[(i + 1) % n]));
        }
    }
    let mut best = f64::INFINITY;
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            let (ra, ia, a0, a1) = edges[i];
            let (rb, ib, b0, b1) = edges[j];
            if ra == rb {
                let n = rings[ra].len();
                // Skip the two neighbours and self.
                let d = (ia + n - ib) % n;
                if d <= 1 || d >= n - 1 {
                    continue;
                }
            }
            let d = segment_segment_distance(&a0, &a1, &b0, &b1);
            if d.is_finite() && d < best {
                best = d;
            }
        }
    }
    best
}

pub fn segment_segment_distance(a0: &P2, a1: &P2, b0: &P2, b1: &P2) -> f64 {
    use super::offset_lab::point_segment_distance;
    point_segment_distance(a0, b0, b1)
        .min(point_segment_distance(a1, b0, b1))
        .min(point_segment_distance(b0, a0, a1))
        .min(point_segment_distance(b1, a0, a1))
}

/// Even-odd ray cast. `polygon::point_in_polygon` is `pub(crate)`, so an
/// integration test cannot borrow it; this is the same crossing-number rule
/// and it is used only to describe the FIXTURE, never to judge an operation's
/// output, so a disagreement at a boundary point cannot leak into a verdict.
pub fn point_in_ring(p: &P2, ring: &[P2]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let dy = b.y - a.y;
            if dy.abs() > f64::MIN_POSITIVE {
                let x = a.x + (p.y - a.y) / dy * (b.x - a.x);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        j = i;
    }
    inside
}

/// `(top-level component count, containment nesting depth)`.
///
/// Depth counts *levels of alternating material*: a flat set of squares is 1,
/// a square with a hole is 2, an island sitting inside that hole is 3. Holes
/// declared on a `Polygon2` and islands declared as separate polygons both
/// count, because the operations disagree about which representation they
/// receive and the campaign has to describe the shape either way.
pub fn containment_shape(polys: &[Polygon2]) -> (usize, usize) {
    let usable: Vec<&Polygon2> = polys.iter().filter(|p| p.exterior.len() >= 3).collect();
    if usable.is_empty() {
        return (0, 0);
    }
    let mut components = 0;
    let mut max_depth = 0;
    for (i, p) in usable.iter().enumerate() {
        let Some(probe) = p.exterior.first() else {
            continue;
        };
        // How many other exteriors strictly contain this one's first vertex?
        let mut enclosing = 0;
        for (j, q) in usable.iter().enumerate() {
            if i == j {
                continue;
            }
            if point_in_ring(probe, &q.exterior) {
                enclosing += 1;
            }
        }
        if enclosing == 0 {
            components += 1;
        }
        // Each enclosing exterior contributes a hole level and a material
        // level; this polygon's own holes contribute one more level.
        let own = 1 + enclosing * 2 + usize::from(!p.holes.is_empty());
        max_depth = max_depth.max(own);
    }
    (components, max_depth)
}

// ---------------------------------------------------------------------------
// Generators — one per hostile class
// ---------------------------------------------------------------------------

/// **The twelve-vertex reflex cross.** Donor: the sentry
/// `pocket::tests::pocket_cascade_terminates_on_the_reflex_cross`
/// (`crates/rs_cam_core/src/pocket.rs`), where `(a, b) = (20.0, 60.0)`.
/// `common_fixtures_smoke_c6` proves this generator reproduces that literal
/// bit-for-bit; the sentry itself is deliberately left spelling its own
/// vertices out, per `common/mod.rs`'s migration policy.
///
/// This is the shape that ran `pocket_offsets` for 13 minutes to 386 MB.
pub fn reflex_cross(a: f64, b: f64) -> Polygon2 {
    Polygon2::new(vec![
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
    ])
}

/// An island inside a hole inside an island: `levels` alternating rings, each
/// `gap` mm inside the last.
///
/// Returned as a **flat `Vec<Polygon2>`**, which is the form an SVG/DXF
/// import hands over — `detect_containment` is what turns it into nested
/// holes, and whether it does so correctly is part of what this fixture
/// tests.
///
/// **Every ring is CCW, and that is a correction.** The first version of this
/// generator alternated winding, on the theory that a consumer trusting
/// winding rather than containment should get an honest answer. It got one:
/// `pocket × holes-in-holes` allocated past **22.9 GB without terminating**.
/// But the mechanism turned out to be the winding alone, not the nesting
/// (`ADVERSARIAL_2D_FINDINGS.md` F-10), and a CW exterior violates
/// `Polygon2`'s stated contract — so alternating winding made this a *second*
/// invalid fixture wearing a valid label, and hid the nesting case it was
/// supposed to be testing behind a crash. The winding case now lives in
/// `reversed_winding` / the `invalid-cw` fixture, where it is labelled
/// honestly, and this one tests nesting.
pub fn holes_in_holes(outer: f64, gap: f64, levels: usize) -> Vec<Polygon2> {
    let mut out = Vec::new();
    for k in 0..levels {
        let inset = gap * k as f64;
        let (lo, hi) = (inset, outer - inset);
        if hi - lo < gap {
            break;
        }
        out.push(Polygon2::rectangle(lo, lo, hi, hi));
    }
    out
}

/// `n × n` disjoint squares of side `size`, `gap` mm apart — no containment
/// relation anywhere, so every operation must produce `n²` independent
/// regions or explain which ones it dropped.
pub fn disconnected_islands(n: usize, size: f64, gap: f64) -> Vec<Polygon2> {
    let pitch = size + gap;
    let mut out = Vec::new();
    for r in 0..n {
        for c in 0..n {
            let (x, y) = (c as f64 * pitch, r as f64 * pitch);
            out.push(Polygon2::rectangle(x, y, x + size, y + size));
        }
    }
    out
}

/// A `w × h` block with a full-depth slit of width `gap` cut in from the top,
/// i.e. two walls `gap` mm apart that are **not adjacent in the ring**.
///
/// Drive it at `gap` values that straddle cavalier's `pos_equal_eps` (1e-5):
/// at 1e-2 the walls are real geometry, at 1e-6 they are below the epsilon
/// that decides whether two vertices are the same point at all.
pub fn near_coincident_walls(w: f64, h: f64, gap: f64, depth: f64) -> Polygon2 {
    let cx = w * 0.5;
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(w, 0.0),
        P2::new(w, h),
        P2::new(cx + gap * 0.5, h),
        P2::new(cx + gap * 0.5, h - depth),
        P2::new(cx - gap * 0.5, h - depth),
        P2::new(cx - gap * 0.5, h),
        P2::new(0.0, h),
    ])
}

/// A square whose edges carry `per_edge` intermediate vertices per side, each
/// pushed `dev` mm off the straight chord.
///
/// At `dev = 1e-9` these vertices are geometrically nothing and every
/// "lossless" cleanup will remove them; they are still the *only* thing
/// telling a sampling consumer where to probe. This is the fixture that
/// makes the `remove_redundant` ruling in `polygon.rs` measurable rather
/// than argued.
pub fn near_collinear(size: f64, per_edge: usize, dev: f64) -> Polygon2 {
    let corners = [
        (P2::new(0.0, 0.0), P2::new(size, 0.0)),
        (P2::new(size, 0.0), P2::new(size, size)),
        (P2::new(size, size), P2::new(0.0, size)),
        (P2::new(0.0, size), P2::new(0.0, 0.0)),
    ];
    let mut pts = Vec::new();
    for (a, b) in corners {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = dx.hypot(dy).max(1.0e-12);
        let (nx, ny) = (-dy / len, dx / len);
        pts.push(a);
        for i in 1..per_edge {
            let t = i as f64 / per_edge as f64;
            // Alternate the side so the run stays a run rather than becoming
            // a shallow bow the offset could legitimately follow.
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            pts.push(P2::new(
                a.x + dx * t + nx * dev * s,
                a.y + dy * t + ny * dev * s,
            ));
        }
    }
    Polygon2::new(pts)
}

/// One big square plus `n` islands of side `island`, each far smaller than
/// the tool footprint. Flat `Vec`, like a real import.
pub fn tiny_islands(outer: f64, n: usize, island: f64) -> Vec<Polygon2> {
    let mut out = vec![Polygon2::rectangle(0.0, 0.0, outer, outer)];
    let pitch = outer / (n as f64 + 1.0);
    for i in 1..=n {
        let (cx, cy) = (pitch * i as f64, outer * 0.5);
        out.push(Polygon2::rectangle(
            cx - island * 0.5,
            cy - island * 0.5,
            cx + island * 0.5,
            cy + island * 0.5,
        ));
    }
    out
}

/// A dumbbell: two `pad × pad` pads joined by a slot `width` mm wide and
/// `bridge` mm long. Below the tool diameter the slot is unmachinable and
/// the pads are two disconnected regions; at exactly the diameter it is a
/// single full-width plunge cut with zero offset room; above it, a pocket.
///
/// The three cases together are what separates "refuses the impossible" from
/// "collapses on the marginal".
pub fn thin_slot(pad: f64, width: f64, bridge: f64) -> Polygon2 {
    let y0 = (pad - width) * 0.5;
    let y1 = y0 + width;
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(pad, 0.0),
        P2::new(pad, y0),
        P2::new(pad + bridge, y0),
        P2::new(pad + bridge, 0.0),
        P2::new(2.0 * pad + bridge, 0.0),
        P2::new(2.0 * pad + bridge, pad),
        P2::new(pad + bridge, pad),
        P2::new(pad + bridge, y1),
        P2::new(pad, y1),
        P2::new(pad, pad),
        P2::new(0.0, pad),
    ])
}

/// A disc of radius `base` with `notches` semicircular bites of radius
/// `notch_r` taken out of its rim, each sampled at `per_notch` points.
///
/// With `notch_r` under the tool radius, every notch is a feature the tool
/// physically cannot enter, while the part as a whole stays comfortably
/// machinable — which is the combination that matters. A fixture whose
/// hostile feature is the *whole part* proves nothing about how an operation
/// handles a hostile feature inside a normal one.
///
/// The rim is sampled only between `±asin(notch_r/base)` of each notch centre
/// so the notch mouth and the rim tile exactly; without that the notch arc's
/// tangential endpoints run back over rim already emitted and the ring
/// self-intersects.
pub fn high_curvature(base: f64, notches: usize, notch_r: f64, per_notch: usize) -> Polygon2 {
    let margin = (notch_r / base).clamp(-1.0, 1.0).asin();
    let rim_steps = 12usize;
    let mut pts = Vec::new();
    for i in 0..notches {
        let a0 = std::f64::consts::TAU * i as f64 / notches as f64;
        let a1 = std::f64::consts::TAU * (i as f64 + 1.0) / notches as f64;
        let (lo, hi) = (a0 + margin, a1 - margin);
        if hi <= lo {
            continue;
        }
        for k in 0..=rim_steps {
            let t = lo + (hi - lo) * k as f64 / rim_steps as f64;
            pts.push(P2::new(base * t.cos(), base * t.sin()));
        }
        // The notch, centred on the rim at `a1`: start tangentially BEHIND
        // a1, dip through the inward direction, finish tangentially AHEAD —
        // the CCW sense the rim is already travelling in.
        let (cx, cy) = (base * a1.cos(), base * a1.sin());
        for k in 0..=per_notch {
            let phi = a1
                - std::f64::consts::FRAC_PI_2
                - std::f64::consts::PI * k as f64 / per_notch as f64;
            pts.push(P2::new(cx + notch_r * phi.cos(), cy + notch_r * phi.sin()));
        }
    }
    Polygon2::new(pts)
}

// ── Invalid contours ───────────────────────────────────────────────────────

/// A bowtie: the classic self-intersecting ring. `Polygon2::repaired` is
/// supposed to split it into two triangles before cavalier sees it (R1.5).
pub fn bowtie(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, size),
        P2::new(size, 0.0),
        P2::new(0.0, size),
    ])
}

/// Every vertex on one line: a ring with zero area and a well-formed vertex
/// list. Nothing in `Polygon2` rejects it.
pub fn zero_area_collinear(size: f64, n: usize) -> Polygon2 {
    Polygon2::new(
        (0..n)
            .map(|i| P2::new(size * i as f64 / (n as f64 - 1.0).max(1.0), 0.0))
            .collect(),
    )
}

/// A "polygon" of two vertices — below the `< 3` guard every offset path has,
/// which means it exercises the *guard*, and the guard returns the same empty
/// `Vec` a contained panic does.
pub fn two_vertex(size: f64) -> Polygon2 {
    Polygon2::new(vec![P2::new(0.0, 0.0), P2::new(size, 0.0)])
}

/// A square with a `NaN` vertex. Nothing upstream filters non-finite
/// coordinates, and `f64` comparisons against `NaN` are all false, so every
/// bounds/min/max computed from it is silently wrong rather than rejected.
pub fn non_finite(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(f64::NAN, size),
        P2::new(0.0, size),
    ])
}

/// A CW exterior — the reverse of the documented `Polygon2` contract
/// ("exterior CCW, positive area"). Offsets sign-flip on it: `distance > 0`
/// grows instead of shrinking.
pub fn reversed_winding(size: f64) -> Polygon2 {
    let mut p = Polygon2::rectangle(0.0, 0.0, size, size);
    p.exterior.reverse();
    p
}

/// An open path flagged `closed: false`, fed where a closed region is
/// expected. `offset_polygon` converts to a **closed** polyline regardless
/// (`exterior_to_pline` hardcodes `true`), so the flag is silently dropped.
pub fn open_contour(size: f64) -> Polygon2 {
    Polygon2::open_path(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(size, size),
    ])
}

/// A square at a coordinate magnitude where `f64` spacing is ~1e-10 mm, so
/// an epsilon of 1e-5 mm is 5 orders of magnitude above the representable
/// resolution of a *difference* but the absolute values still round-trip.
pub fn far_from_origin(size: f64, offset: f64) -> Polygon2 {
    Polygon2::rectangle(offset, offset, offset + size, offset + size)
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

/// Every fixture the R2 campaign runs, in one place. Tool diameters are
/// chosen so the named mechanism is hostile *to that tool*: a 6 mm cutter for
/// the shapes authored at part scale, 1 mm for the ones whose feature size is
/// sub-millimetre.
pub fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "reflex-cross",
            class: Class::Reflex,
            what: "the 12-vertex cross that ran pocket_offsets 13 min / 386 MB",
            polys: vec![reflex_cross(20.0, 60.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "comb-16",
            class: Class::Reflex,
            what: "16 slots, 6 mm fingers that collapse at ring ~30 (M5 donor)",
            polys: vec![comb(200.0, 80.0, 16, 6.0, 40.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "dendrite",
            class: Class::Dendrite,
            what: "branching concavity at two scales (M5 donor)",
            polys: vec![dendrite(200.0, 80.0, 12, 8.0, 40.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "rosette-24",
            class: Class::Reflex,
            what: "24 lobes; reflex valleys that survive erosion (M5 donor)",
            polys: vec![rosette(60.0, 8.0, 24, 720)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "holes-in-holes",
            class: Class::HolesInHoles,
            what: "5 nested CCW rings in a flat list — containment, not winding, \
                   has to decide which levels are material",
            polys: holes_in_holes(120.0, 12.0, 5),
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "islands-4x4",
            class: Class::DisconnectedIslands,
            what: "16 disjoint 20 mm squares — no containment relation at all",
            polys: disconnected_islands(4, 20.0, 10.0),
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "holed-9",
            class: Class::HolesInHoles,
            what: "square + 9 circular holes; the Shape::parallel_offset path (M5 donor)",
            polys: vec![holed(200.0, 3, 15.0, 64)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "walls-1e-2",
            class: Class::NearCoincidentWalls,
            what: "10 µm slit — real geometry, three orders under the tool",
            polys: vec![near_coincident_walls(120.0, 80.0, 1.0e-2, 60.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "walls-1e-6",
            class: Class::NearCoincidentWalls,
            what: "1 nm slit — BELOW cavalier's 1e-5 pos_equal_eps",
            polys: vec![near_coincident_walls(120.0, 80.0, 1.0e-6, 60.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "short-edges",
            class: Class::ShortEdges,
            what: "1600 sub-epsilon segments (M5 donor)",
            polys: vec![short_edges(100.0, 200)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "near-collinear-1nm",
            class: Class::NearCollinear,
            what: "100 vertices/edge at 1 nm deviation — sampling with no shape",
            polys: vec![near_collinear(100.0, 100, 1.0e-6)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "tiny-islands",
            class: Class::TinyIslands,
            what: "7 islands of 0.20 mm² inside a 120 mm square; tool footprint 28.3 mm²",
            polys: tiny_islands(120.0, 7, 0.45),
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "slot-under",
            class: Class::ThinSlot,
            what: "3 mm slot, 6 mm tool — physically unmachinable bridge",
            polys: vec![thin_slot(40.0, 3.0, 30.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "slot-exact",
            class: Class::ThinSlot,
            what: "6.000 mm slot, 6 mm tool — zero offset room, the marginal case",
            polys: vec![thin_slot(40.0, 6.0, 30.0)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "high-curvature",
            class: Class::HighCurvature,
            what: "80 mm disc with 12 notches of r = 1.0 mm — a 6 mm tool cannot \
                   enter any of them, but the part is ordinary",
            polys: vec![high_curvature(40.0, 12, 1.0, 16)],
            tool_d: 6.0,
            validity: Validity::Valid,
            expectation: Expectation::MustCut,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-bowtie",
            class: Class::InvalidContour,
            what: "self-intersecting ring; repaired() must split it before cavalier",
            polys: vec![bowtie(60.0)],
            tool_d: 6.0,
            validity: Validity::Invalid("self-intersecting exterior"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-zero-area",
            class: Class::InvalidContour,
            what: "12 collinear vertices; a well-formed ring enclosing nothing",
            polys: vec![zero_area_collinear(60.0, 12)],
            tool_d: 6.0,
            validity: Validity::Invalid("zero-area collinear ring"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-two-vertex",
            class: Class::InvalidContour,
            what: "two vertices — exercises the < 3 guard, whose empty Vec is \
                   indistinguishable from a contained panic",
            polys: vec![two_vertex(60.0)],
            tool_d: 6.0,
            validity: Validity::Invalid("fewer than 3 vertices"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-nan",
            class: Class::InvalidContour,
            what: "one NaN vertex; every bbox/min/max derived from it is silently wrong",
            polys: vec![non_finite(60.0)],
            tool_d: 6.0,
            validity: Validity::Invalid("non-finite coordinate"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-cw",
            class: Class::InvalidContour,
            what: "CW exterior — the offset sign convention inverts, so the \
                   pocket cascade GROWS and never collapses",
            polys: vec![reversed_winding(60.0)],
            tool_d: 6.0,
            validity: Validity::Invalid("clockwise exterior"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[
                (
                    "pocket",
                    "DOES NOT TERMINATE — pocket's ring cascade exits only on \
                     collapse, and a CW exterior makes every offset GROW. \
                     Proved instead, bounded, by \
                     the_pocket_ring_cascade_is_bounded_only_by_collapse. \
                     Running it here would hang the suite, not test it \
                     (22.9 GB RSS on the first attempt)",
                ),
                (
                    "inlay",
                    "same cascade, reached through inlay's female pocket \
                     (inlay.rs:108)",
                ),
            ],
        },
        Fixture {
            name: "invalid-open",
            class: Class::InvalidContour,
            what: "closed: false, silently re-closed by exterior_to_pline",
            polys: vec![open_contour(60.0)],
            tool_d: 6.0,
            validity: Validity::Invalid("open path in a closed-region slot"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
        Fixture {
            name: "invalid-far",
            class: Class::InvalidContour,
            what: "60 mm square at x = 1e9 mm; f64 spacing there is ~1e-7 mm",
            polys: vec![far_from_origin(60.0, 1.0e9)],
            tool_d: 6.0,
            validity: Validity::Invalid("coordinates far outside any machine envelope"),
            expectation: Expectation::MayBeEmpty,
            skip_ops: &[],
        },
    ]
}

// ---------------------------------------------------------------------------
// Watchdog: wall clock, memory, cancellation
// ---------------------------------------------------------------------------

/// Peak resident set size of THIS PROCESS in kB, from `/proc/self/status`'s
/// `VmHWM`.
///
/// **Method, stated because the number is only meaningful with it.** `VmHWM`
/// is a process-wide monotonic high-water mark, not a per-call figure. The
/// campaign therefore runs one operation per measurement, sequentially, in a
/// single process, and reports the *increase* in the high-water mark across
/// the call. That increase is a lower bound on the call's own peak: it is
/// exact when the call is the largest allocator so far and reads zero when an
/// earlier call already peaked higher. A zero is reported as `≤ prior peak`
/// rather than as "used no memory". Non-Linux platforms return `None` and the
/// campaign records `unmeasured`.
pub fn peak_rss_kb() -> Option<u64> {
    proc_status_field("VmHWM:")
}

/// Current RSS in kB (`VmRSS`) — used to show that memory came *back* after
/// a hostile run, which a high-water mark cannot show.
pub fn current_rss_kb() -> Option<u64> {
    proc_status_field("VmRSS:")
}

fn proc_status_field(key: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u64>().ok())
}

#[derive(Clone, Debug)]
pub enum Outcome {
    /// Generation returned `Ok`.
    Ok,
    /// Generation returned a typed error. For an adversarial input this is a
    /// *good* outcome — it is the only one the operator can see.
    Err(String),
    /// Generation panicked. Never acceptable.
    Panic(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Err(_) => "error",
            Self::Panic(_) => "PANIC",
        }
    }
    pub fn is_panic(&self) -> bool {
        matches!(self, Self::Panic(_))
    }
}

/// One operation × one fixture, measured.
#[derive(Clone, Debug)]
pub struct RunRecord {
    pub op: String,
    pub fixture: &'static str,
    pub wall: Duration,
    pub outcome: Outcome,
    /// `None` on a platform without `/proc/self/status`.
    pub rss_growth_kb: Option<u64>,
    pub peak_rss_kb: Option<u64>,
    pub moves: usize,
    pub cutting_moves: usize,
    pub rapid_moves: usize,
    pub cut_length_mm: f64,
    /// Distinct closed loops in the emitted path, counted as runs of cutting
    /// moves separated by rapids — the topology figure the findings table
    /// reports.
    pub cut_runs: usize,
    pub min_z: f64,
}

impl RunRecord {
    /// The R2 acceptance gate: an operation that reports success but emits no
    /// cutting move on a fixture declared [`Expectation::MustCut`] has
    /// silently completed nothing.
    pub fn is_silent_empty(&self, expectation: Expectation) -> bool {
        matches!(self.outcome, Outcome::Ok)
            && self.cutting_moves == 0
            && expectation == Expectation::MustCut
    }

    pub fn row(&self) -> String {
        format!(
            "| {} | {} | {:.3} s | {} | {} | {} | {:.1} | {} | {} |",
            self.op,
            self.fixture,
            self.wall.as_secs_f64(),
            self.outcome.label(),
            self.moves,
            self.cutting_moves,
            self.cut_length_mm,
            self.cut_runs,
            self.rss_growth_kb
                .map_or_else(|| "n/a".to_owned(), |kb| format!("{kb} kB")),
        )
    }
}

/// Summarise an emitted toolpath. Kept separate from [`run_op`] so the same
/// figures can be taken from a path that arrived some other way.
pub fn summarise(tp: &Toolpath) -> (usize, usize, usize, f64, usize, f64) {
    let mut cutting = 0;
    let mut rapid = 0;
    let mut length = 0.0;
    let mut runs = 0;
    let mut in_run = false;
    let mut min_z = f64::INFINITY;
    let mut prev: Option<P2> = None;
    for m in &tp.moves {
        min_z = min_z.min(m.target.z);
        let here = P2::new(m.target.x, m.target.y);
        match m.move_type {
            MoveType::Rapid => {
                rapid += 1;
                in_run = false;
            }
            _ => {
                cutting += 1;
                if !in_run {
                    runs += 1;
                    in_run = true;
                }
                if let Some(p) = prev {
                    length += (here.x - p.x).hypot(here.y - p.y);
                }
            }
        }
        prev = Some(here);
    }
    if !min_z.is_finite() {
        min_z = 0.0;
    }
    (tp.moves.len(), cutting, rapid, length, runs, min_z)
}

/// Generate toolpath `index` through the production funnel
/// (`ProjectSession::generate_toolpath`, the entry point the GUI worker and
/// the CLI share), under panic containment, with wall clock and RSS taken
/// around the call.
pub fn run_op(
    session: &mut ProjectSession,
    index: usize,
    op: &str,
    fixture: &'static str,
) -> RunRecord {
    let rss_before = peak_rss_kb();
    let cancel = AtomicBool::new(false);
    let t0 = Instant::now();
    let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.generate_toolpath(index, &cancel).map(|_| ())
    })) {
        Ok(Ok(())) => Outcome::Ok,
        Ok(Err(e)) => Outcome::Err(format!("{e:?}")),
        Err(payload) => Outcome::Panic(panic_message(&*payload)),
    };
    let wall = t0.elapsed();
    let rss_after = peak_rss_kb();
    let (moves, cutting, rapid, length, runs, min_z) = session
        .get_result(index)
        .map_or((0, 0, 0, 0.0, 0, 0.0), |r| summarise(r.toolpath()));
    RunRecord {
        op: op.to_owned(),
        fixture,
        wall,
        outcome,
        rss_growth_kb: match (rss_before, rss_after) {
            (Some(b), Some(a)) => Some(a.saturating_sub(b)),
            _ => None,
        },
        peak_rss_kb: rss_after,
        moves,
        cutting_moves: cutting,
        rapid_moves: rapid,
        cut_length_mm: length,
        cut_runs: runs,
        min_z,
    }
}

/// The cancellation seam. Sets the shared cancel flag `after`, then reports
/// how long the generator took to come back **from that moment**.
///
/// A generator that only checks its flag between Z levels can pass a
/// wall-clock ceiling and still be uncancellable inside one level, so the
/// latency after the flag is set is the figure that matters, not the total.
pub struct CancelRecord {
    pub op: String,
    pub fixture: &'static str,
    pub set_after: Duration,
    /// Time from flag-set to return. `None` when the generator finished
    /// before the flag was ever set — an honest "not exercised", never a pass.
    pub latency: Option<Duration>,
    pub total: Duration,
    pub outcome: Outcome,
}

impl CancelRecord {
    pub fn label(&self) -> String {
        match self.latency {
            Some(l) => format!("{:.3} s after flag", l.as_secs_f64()),
            None => "NOT EXERCISED (finished first)".to_owned(),
        }
    }
}

pub fn run_op_with_cancel(
    session: &mut ProjectSession,
    index: usize,
    op: &str,
    fixture: &'static str,
    set_after: Duration,
) -> CancelRecord {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let armed = Arc::new(std::sync::Mutex::new(None::<Instant>));
    let armed_w = Arc::clone(&armed);
    let ticker = std::thread::spawn(move || {
        std::thread::sleep(set_after);
        if let Ok(mut slot) = armed_w.lock() {
            *slot = Some(Instant::now());
        }
        flag.store(true, Ordering::SeqCst);
    });

    let t0 = Instant::now();
    let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.generate_toolpath(index, &cancel).map(|_| ())
    })) {
        Ok(Ok(())) => Outcome::Ok,
        Ok(Err(e)) => Outcome::Err(format!("{e:?}")),
        Err(payload) => Outcome::Panic(panic_message(&*payload)),
    };
    let returned = Instant::now();
    let total = t0.elapsed();
    let _ = ticker.join();
    let latency = armed
        .lock()
        .ok()
        .and_then(|slot| *slot)
        .filter(|set| *set <= returned)
        .map(|set| returned.duration_since(set));

    CancelRecord {
        op: op.to_owned(),
        fixture,
        set_after,
        latency,
        total,
        outcome,
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

// ---------------------------------------------------------------------------
// Renders — the campaign's "render before verdict" obligation
// ---------------------------------------------------------------------------

/// `target/adversarial_2d/` by default; override with `R2_ARTIFACT_DIR` to
/// write straight into the evidence folder.
///
/// The default is under `target/` on purpose: a test that writes into a
/// tracked directory on every run makes `git status` lie about what a wave
/// changed. The committed gallery is copied there deliberately, once, and
/// its paths are recorded in `ADVERSARIAL_2D_FINDINGS.md`.
pub fn out_dir() -> std::path::PathBuf {
    let dir = std::env::var("R2_ARTIFACT_DIR").map_or_else(
        |_| super::repo_root().join("target/adversarial_2d"),
        std::path::PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    dir
}

struct View {
    lo_x: f64,
    lo_y: f64,
    hi_y: f64,
    scale: f64,
    w: f64,
    h: f64,
}

fn view_of(polys: &[Polygon2], px: f64) -> Option<View> {
    let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in polys {
        for v in p.exterior.iter().chain(p.holes.iter().flatten()) {
            if !v.x.is_finite() || !v.y.is_finite() {
                continue;
            }
            lo_x = lo_x.min(v.x);
            lo_y = lo_y.min(v.y);
            hi_x = hi_x.max(v.x);
            hi_y = hi_y.max(v.y);
        }
    }
    if !lo_x.is_finite() || !hi_x.is_finite() {
        return None;
    }
    let (w, h) = ((hi_x - lo_x).max(1.0e-6), (hi_y - lo_y).max(1.0e-6));
    let scale = px / w.max(h);
    Some(View {
        lo_x,
        lo_y,
        hi_y,
        scale,
        w: w * scale,
        h: h * scale,
    })
}

impl View {
    fn x(&self, x: f64) -> f64 {
        (x - self.lo_x) * self.scale
    }
    fn y(&self, y: f64) -> f64 {
        (self.hi_y - y) * self.scale
    }
    fn header(&self) -> String {
        let (w, h) = (self.w + 16.0, self.h + 16.0);
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" \
             viewBox=\"-8 -8 {w:.0} {h:.0}\">\
             <rect x=\"-8\" y=\"-8\" width=\"{w:.0}\" height=\"{h:.0}\" fill=\"#111\"/>"
        )
    }
    fn ring(&self, ring: &[P2], stroke: &str, width: f64, close: bool, svg: &mut String) {
        let pts: Vec<&P2> = ring
            .iter()
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            .collect();
        let Some(first) = pts.first() else { return };
        if pts.len() < 2 {
            return;
        }
        svg.push_str(&format!(
            "<path d=\"M {:.2} {:.2}",
            self.x(first.x),
            self.y(first.y)
        ));
        for p in pts.iter().skip(1) {
            svg.push_str(&format!(" L {:.2} {:.2}", self.x(p.x), self.y(p.y)));
        }
        if close {
            svg.push_str(" Z");
        }
        svg.push_str(&format!(
            "\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{width}\"/>"
        ));
    }
}

/// Render a fixture: exterior white, holes cyan, vertices as dots so
/// sub-epsilon clusters are visible as thickening rather than invisible.
pub fn write_fixture_svg(fixture: &Fixture, name: &str) -> Option<std::path::PathBuf> {
    let view = view_of(&fixture.polys, 900.0)?;
    let mut svg = view.header();
    for p in &fixture.polys {
        view.ring(&p.exterior, "#fff", 1.4, p.closed, &mut svg);
        for h in &p.holes {
            view.ring(h, "#3cf", 1.2, true, &mut svg);
        }
        for v in p.exterior.iter().chain(p.holes.iter().flatten()) {
            if !v.x.is_finite() || !v.y.is_finite() {
                continue;
            }
            svg.push_str(&format!(
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"1.1\" fill=\"#f80\" fill-opacity=\"0.7\"/>",
                view.x(v.x),
                view.y(v.y)
            ));
        }
    }
    svg.push_str(&format!(
        "<text x=\"4\" y=\"14\" fill=\"#aaa\" font-family=\"monospace\" font-size=\"13\">\
         {} — {}</text></svg>",
        fixture.name, fixture.what
    ));
    let path = out_dir().join(format!("{name}.svg"));
    std::fs::write(&path, svg).ok().map(|()| path)
}

/// Render an emitted toolpath over its fixture: cutting moves in amber,
/// rapids in dim red. Rapids are drawn because on these fixtures the
/// interesting failure is often *where the tool travelled*, not where it cut.
pub fn write_toolpath_svg(
    fixture: &Fixture,
    tp: &Toolpath,
    name: &str,
) -> Option<std::path::PathBuf> {
    let view = view_of(&fixture.polys, 900.0)?;
    let mut svg = view.header();
    for p in &fixture.polys {
        view.ring(&p.exterior, "#444", 1.2, p.closed, &mut svg);
        for h in &p.holes {
            view.ring(h, "#335", 1.0, true, &mut svg);
        }
    }
    let mut prev: Option<P2> = None;
    for m in &tp.moves {
        let here = P2::new(m.target.x, m.target.y);
        if let Some(a) = prev
            && here.x.is_finite()
            && here.y.is_finite()
            && a.x.is_finite()
            && a.y.is_finite()
        {
            let (stroke, width, dash) = match m.move_type {
                MoveType::Rapid => ("#a33", 0.6, " stroke-dasharray=\"3 3\""),
                _ => ("#fb0", 1.0, ""),
            };
            svg.push_str(&format!(
                "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" \
                 stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>",
                view.x(a.x),
                view.y(a.y),
                view.x(here.x),
                view.y(here.y)
            ));
        }
        prev = Some(here);
    }
    let (moves, cutting, _, length, runs, _) = summarise(tp);
    svg.push_str(&format!(
        "<text x=\"4\" y=\"14\" fill=\"#aaa\" font-family=\"monospace\" font-size=\"13\">\
         {name} — {moves} moves, {cutting} cutting, {length:.0} mm, {runs} runs</text></svg>"
    ));
    let path = out_dir().join(format!("{name}.svg"));
    std::fs::write(&path, svg).ok().map(|()| path)
}
