//! H1 / PR-2 sentries for the tool-scale semantic API.
//!
//! Oracle: `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md`. PR-2 added
//! `envelope_radius_mm()`, `cusp_radius_mm()` and `engagement_radius_mm(d)`
//! as *documented aliases* of `radius()`, `cusp_radius()` and
//! `engagement_radius(d)` — no new math, no behavior change. This file is
//! the coverage that makes "alias" enforceable rather than aspirational:
//!
//! 1. **Delegation parity** (§2.1 / §7.4). `ToolDefinition` — the wrapper the
//!    production path actually holds — now delegates every accessor
//!    EXPLICITLY. Before PR-2, `radius()` and `cusp_radius()` worked only via
//!    inherited trait defaults over delegated primitives, so the first shape
//!    to override `cusp_radius()` directly would have seen the wrapper
//!    silently revert to the default. These tests fail if any accessor stops
//!    tracking its shape, in either spelling, on any of the five shapes.
//! 2. **Profile properties** (§9.1 rows 15-16). Envelope bounds every sampled
//!    profile width; engagement radius is monotone in depth and bounded by
//!    the envelope; `height_at_radius` is the (one-sided) inverse of
//!    `width_at_height` and `None` above the envelope.
//! 3. **Large-arc threshold parity** (§5, H2.6 resolved = ENVELOPE). The
//!    narration diagnostic is a post-condition on `arcfit`'s own radius cap.
//!    Both sides must break at the same number; the sentry uses a TAPERED
//!    tool so a silent switch to the tip scale moves one side 6×.
//! 4. **`FinishSurface::cell_source` consistency** (PR-0 integration). The
//!    provenance tag must name the radius that actually sized the grid.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::finish_setup::{
    build_classification_surface_with_cancel, build_finish_surface_with_cancel,
    build_finish_surface_with_cell_size_and_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::measurement::CellSource;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::narrate::narrate_toolpath;
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition,
    VBitEndmill,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

// ── Fixtures ─────────────────────────────────────────────────────────────

/// The tool this project actually finishes with: Ø1 tip, 7° half-angle,
/// Ø6 shaft. Envelope 3.0 mm, cusp 0.5 mm — a 6× split, which is what makes
/// every assertion below discriminating.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// The five shapes, by name. Built on demand so the same shape can be held
/// bare and wrapped in a `ToolDefinition` at the same time.
const SHAPE_NAMES: [&str; 5] = ["flat", "ball", "bull", "tapered_ball", "vbit"];

fn shape(name: &str) -> Box<dyn MillingCutter> {
    match name {
        "flat" => Box::new(FlatEndmill::new(6.0, 25.0)),
        "ball" => Box::new(BallEndmill::new(6.0, 25.0)),
        "bull" => Box::new(BullNoseEndmill::new(6.0, 1.0, 25.0)),
        "tapered_ball" => Box::new(taper()),
        "vbit" => Box::new(VBitEndmill::new(6.0, 90.0, 25.0)),
        other => panic!("unknown shape {other}"),
    }
}

fn wrap(cutter: Box<dyn MillingCutter>) -> ToolDefinition {
    ToolDefinition::new(cutter, 6.0, 30.0, 25.0, 40.0, 2, ToolMaterial::Carbide)
}

/// Depths / radii sampled by the property tests. Chosen to straddle every
/// profile break: the tapered ball's ball/cone tangency (0.4391 mm), the
/// bullnose corner radius (1.0 mm), and each shape's full envelope.
fn sample_heights(length: f64) -> Vec<f64> {
    let mut hs = vec![0.0, 1e-6, 0.01, 0.05, 0.2, 0.4391, 0.5, 1.0, 2.0, 3.0, 5.0];
    for i in 0..=40 {
        hs.push(length * i as f64 / 40.0);
    }
    hs.retain(|h| *h >= 0.0 && *h <= length + 1e-9);
    hs
}

// ── 1. Delegation parity ─────────────────────────────────────────────────

/// Every accessor must return the same number in both spellings, on the bare
/// shape AND through `ToolDefinition`. Four comparisons per shape; a broken
/// delegation or a diverging alias fails exactly one of them.
#[test]
fn tool_definition_delegates_every_tool_scale_accessor() {
    for name in SHAPE_NAMES {
        let bare = shape(name);
        let bare_env = bare.radius();
        let bare_env_mm = bare.envelope_radius_mm();
        let bare_cusp = bare.cusp_radius();
        let bare_cusp_mm = bare.cusp_radius_mm();
        let length = bare.length();
        let def = wrap(shape(name));

        assert!(
            (bare_env - bare_env_mm).abs() < 1e-12,
            "{name}: envelope_radius_mm() must alias radius() ({bare_env} vs {bare_env_mm})"
        );
        assert!(
            (bare_cusp - bare_cusp_mm).abs() < 1e-12,
            "{name}: cusp_radius_mm() must alias cusp_radius() ({bare_cusp} vs {bare_cusp_mm})"
        );
        assert!(
            (def.radius() - bare_env).abs() < 1e-12,
            "{name}: ToolDefinition::radius() lost the shape's envelope ({} vs {bare_env})",
            def.radius()
        );
        assert!(
            (def.envelope_radius_mm() - bare_env).abs() < 1e-12,
            "{name}: ToolDefinition::envelope_radius_mm() lost the shape's envelope ({} vs {bare_env})",
            def.envelope_radius_mm()
        );
        assert!(
            (def.cusp_radius() - bare_cusp).abs() < 1e-12,
            "{name}: ToolDefinition::cusp_radius() lost the shape's tip ({} vs {bare_cusp})",
            def.cusp_radius()
        );
        assert!(
            (def.cusp_radius_mm() - bare_cusp).abs() < 1e-12,
            "{name}: ToolDefinition::cusp_radius_mm() lost the shape's tip ({} vs {bare_cusp})",
            def.cusp_radius_mm()
        );

        // Depth-parameterised accessors: parity at every sampled depth, both
        // spellings, bare and wrapped.
        for d in sample_heights(length) {
            let bare_e = bare.engagement_radius(d);
            assert!(
                (bare.engagement_radius_mm(d) - bare_e).abs() < 1e-12,
                "{name}: engagement_radius_mm({d}) must alias engagement_radius({d})"
            );
            assert!(
                (def.engagement_radius(d) - bare_e).abs() < 1e-12,
                "{name}: ToolDefinition::engagement_radius({d}) diverged from the shape"
            );
            assert!(
                (def.engagement_radius_mm(d) - bare_e).abs() < 1e-12,
                "{name}: ToolDefinition::engagement_radius_mm({d}) diverged from the shape"
            );
            // The two profile primitives the aliases are built on must also
            // survive the wrapper — `cusp_radius`'s default reads
            // `geometry_hint()`, and `height_at_radius`/`width_at_height` are
            // what every reach query resolves through.
            assert!(
                (def.width_at_height(d) - bare.width_at_height(d)).abs() < 1e-12,
                "{name}: ToolDefinition::width_at_height({d}) diverged from the shape"
            );
            assert_eq!(
                def.height_at_radius(d).is_some(),
                bare.height_at_radius(d).is_some(),
                "{name}: ToolDefinition::height_at_radius({d}) domain diverged"
            );
        }
    }
}

/// A shape that overrides `radius()` and `cusp_radius()` DIRECTLY rather
/// than through `diameter()` / `geometry_hint()`.
///
/// This is the trap `TOOL_SCALE_SEMANTICS.md` §2.1/§7.4 describes: before
/// PR-2, `ToolDefinition` delegated only the primitives, so both accessors
/// resolved through the inherited trait defaults and a shape like this one
/// would have had its overrides silently discarded at the only layer that
/// ships. No production shape does this today — which is exactly why the
/// delegation test needs a fixture that does, or it asserts nothing.
struct DirectlyOverridingCutter {
    inner: BallEndmill,
}

impl MillingCutter for DirectlyOverridingCutter {
    fn diameter(&self) -> f64 {
        self.inner.diameter()
    }
    // Deliberately NOT `diameter()/2` — the default would give 3.0.
    fn radius(&self) -> f64 {
        7.0
    }
    // Deliberately not reachable through `geometry_hint()` (a `Ball` hint
    // makes the default fall back to `radius()`).
    fn cusp_radius(&self) -> f64 {
        0.125
    }
    fn length(&self) -> f64 {
        self.inner.length()
    }
    fn chip_geometry(
        &self,
        axial_doc_mm: f64,
        arc_engagement_radians: f64,
        feed_per_tooth_mm: f64,
        flute_count: u32,
        mode: rs_cam_core::tool::EngagementMode,
    ) -> Result<rs_cam_core::tool::ChipGeometry, rs_cam_core::tool::EngagementError> {
        self.inner.chip_geometry(
            axial_doc_mm,
            arc_engagement_radians,
            feed_per_tooth_mm,
            flute_count,
            mode,
        )
    }
    fn height_at_radius(&self, r: f64) -> Option<f64> {
        self.inner.height_at_radius(r)
    }
    fn width_at_height(&self, h: f64) -> f64 {
        self.inner.width_at_height(h)
    }
    // Deliberately overridden away from `width_at_height` so the wrapper's
    // engagement delegation is testable too.
    fn engagement_radius(&self, depth_of_cut: f64) -> f64 {
        self.inner.width_at_height(depth_of_cut) + 0.75
    }
    fn geometry_hint(&self) -> rs_cam_core::feeds::ToolGeometryHint {
        self.inner.geometry_hint()
    }
    fn center_height(&self) -> f64 {
        self.inner.center_height()
    }
    fn normal_length(&self) -> f64 {
        self.inner.normal_length()
    }
    fn xy_normal_length(&self) -> f64 {
        self.inner.xy_normal_length()
    }
    fn edge_drop(&self, cl: &mut rs_cam_core::tool::CLPoint, p1: &P3, p2: &P3) {
        self.inner.edge_drop(cl, p1, p2);
    }
}

/// Red-first evidence for the delegation gate: with a shape whose overrides
/// are invisible to the trait defaults, `ToolDefinition` must still report
/// the SHAPE's numbers. Revert any of the five explicit delegations in
/// `impl MillingCutter for ToolDefinition` and this test fails on that
/// accessor (3.0 instead of 7.0, 3.0 instead of 0.125, or the un-offset
/// engagement radius).
#[test]
fn tool_definition_delegation_survives_direct_overrides() {
    let bare = DirectlyOverridingCutter {
        inner: BallEndmill::new(6.0, 25.0),
    };
    assert!((bare.radius() - 7.0).abs() < 1e-12);
    assert!((bare.envelope_radius_mm() - 7.0).abs() < 1e-12);
    assert!((bare.cusp_radius_mm() - 0.125).abs() < 1e-12);

    let def = wrap(Box::new(DirectlyOverridingCutter {
        inner: BallEndmill::new(6.0, 25.0),
    }));
    assert!(
        (def.radius() - 7.0).abs() < 1e-12,
        "ToolDefinition::radius() fell back to diameter()/2; got {}",
        def.radius()
    );
    assert!(
        (def.envelope_radius_mm() - 7.0).abs() < 1e-12,
        "ToolDefinition::envelope_radius_mm() fell back to the default; got {}",
        def.envelope_radius_mm()
    );
    assert!(
        (def.cusp_radius() - 0.125).abs() < 1e-12,
        "ToolDefinition::cusp_radius() fell back to the geometry-hint default; got {}",
        def.cusp_radius()
    );
    assert!(
        (def.cusp_radius_mm() - 0.125).abs() < 1e-12,
        "ToolDefinition::cusp_radius_mm() fell back to the default; got {}",
        def.cusp_radius_mm()
    );
    let expected = bare.engagement_radius(1.0);
    assert!(
        (def.engagement_radius(1.0) - expected).abs() < 1e-12,
        "ToolDefinition::engagement_radius() lost the override"
    );
    assert!(
        (def.engagement_radius_mm(1.0) - expected).abs() < 1e-12,
        "ToolDefinition::engagement_radius_mm() lost the override"
    );
}

/// The tapered tool is the only shape where the classes actually differ —
/// pin the split explicitly so a "simplification" that collapses the three
/// accessors onto one cannot pass by making everything equal.
#[test]
fn tapered_tool_keeps_the_three_classes_distinct() {
    let def = wrap(Box::new(taper()));
    assert!((def.envelope_radius_mm() - 3.0).abs() < 1e-9);
    assert!((def.cusp_radius_mm() - 0.5).abs() < 1e-9);
    // WIDTH(0.5 mm) sits between the two, near the tip sphere and nowhere
    // near the shaft (§4.3: 0.5038 mm at 0.5 mm depth).
    let w = def.engagement_radius_mm(0.5);
    assert!(
        (w - 0.5038).abs() < 5e-3,
        "engagement radius at 0.5 mm depth should be ~0.504 mm; got {w}"
    );
    assert!(
        def.envelope_radius_mm() > 5.0 * def.cusp_radius_mm(),
        "the fixture must keep a large shaft/tip split or every assertion here goes inert"
    );
}

// ── 2. Profile properties ────────────────────────────────────────────────

/// §9.1 row 15: the envelope radius is an upper bound on the profile — no
/// sampled width at any height may exceed it, and neither may the engagement
/// radius at any depth. This is the property that makes it safe to keep
/// envelope at every collision / padding / stamping site.
#[test]
fn envelope_radius_bounds_every_sampled_profile_width() {
    for name in SHAPE_NAMES {
        let cutter = shape(name);
        let env = cutter.envelope_radius_mm();
        for h in sample_heights(cutter.length()) {
            let w = cutter.width_at_height(h);
            assert!(
                w <= env + 1e-9,
                "{name}: width_at_height({h}) = {w} exceeds envelope {env}"
            );
            let e = cutter.engagement_radius_mm(h);
            assert!(
                e <= env + 1e-9,
                "{name}: engagement_radius_mm({h}) = {e} exceeds envelope {env}"
            );
            assert!(
                w >= -1e-12,
                "{name}: width_at_height({h}) = {w} must be non-negative"
            );
        }
        // The cusp radius is a tip-scale quantity and can never exceed the
        // envelope either.
        assert!(
            cutter.cusp_radius_mm() <= env + 1e-9,
            "{name}: cusp radius exceeds envelope"
        );
    }
}

/// §9.1 row 16: engagement radius is monotone non-decreasing in depth. A
/// profile that narrows with depth would break every stepover model built on
/// it (and would mean the tool cannot be withdrawn).
#[test]
fn engagement_radius_is_monotonic_in_depth() {
    for name in SHAPE_NAMES {
        let cutter = shape(name);
        let mut prev = f64::NEG_INFINITY;
        let mut depths = sample_heights(cutter.length());
        depths.sort_by(|a, b| a.partial_cmp(b).expect("finite sample depths"));
        for d in depths {
            let e = cutter.engagement_radius_mm(d);
            assert!(
                e >= prev - 1e-9,
                "{name}: engagement_radius_mm is not monotonic at depth {d} ({e} < {prev})"
            );
            prev = e;
        }
    }
}

/// §4.5: `height_at_radius(r)` is the CLEAR(r) query — the inverse of
/// `width_at_height`, `None` above the envelope.
///
/// The exact round trip holds only where the profile is strictly widening.
/// A flat endmill's whole bottom and a bullnose inside its corner radius sit
/// at height 0 for many radii, so the guaranteed relation is `>= r` (the
/// cutter is AT LEAST that wide once it is that deep). The doc's §4.5 claim
/// of an exact inverse "for w <= envelope" is therefore true only for the
/// strictly-widening shapes; both forms are pinned here.
#[test]
fn height_at_radius_inverts_width_at_height() {
    for name in SHAPE_NAMES {
        let cutter = shape(name);
        let env = cutter.envelope_radius_mm();
        // Above the envelope there is no profile: None, for every shape.
        for over in [env + 1e-6, env * 1.5, env + 100.0] {
            assert!(
                cutter.height_at_radius(over).is_none(),
                "{name}: height_at_radius({over}) must be None above the envelope {env}"
            );
        }
        for i in 0..=40 {
            let r = env * i as f64 / 40.0;
            let h = cutter
                .height_at_radius(r)
                .unwrap_or_else(|| panic!("{name}: height_at_radius({r}) must exist within {env}"));
            assert!(
                h >= -1e-12,
                "{name}: height_at_radius({r}) = {h} must be non-negative"
            );
            let back = cutter.width_at_height(h);
            assert!(
                back >= r - 1e-6,
                "{name}: width_at_height(height_at_radius({r}) = {h}) = {back} < {r} — the \
                 cutter must be at least that wide once it is that deep"
            );
        }
    }
    // Strictly-widening shapes: the round trip is exact.
    let ball = BallEndmill::new(6.0, 25.0);
    let vbit = VBitEndmill::new(6.0, 90.0, 25.0);
    let tap = taper();
    for (name, cutter) in [
        ("ball", &ball as &dyn MillingCutter),
        ("vbit", &vbit as &dyn MillingCutter),
        ("tapered_ball", &tap as &dyn MillingCutter),
    ] {
        let env = cutter.envelope_radius_mm();
        for i in 1..=40 {
            let r = env * i as f64 / 40.0;
            let h = cutter.height_at_radius(r).expect("within envelope");
            let back = cutter.width_at_height(h);
            assert!(
                (back - r).abs() < 1e-6,
                "{name}: round trip r={r} -> h={h} -> {back} must be exact on a strictly \
                 widening profile"
            );
        }
    }
}

// ── 3. Large-arc threshold parity (H2.6 = ENVELOPE) ──────────────────────

/// Sample `count` points along a circular arc of radius `r` centred at the
/// origin, starting at angle 0 and sweeping `sweep_deg`.
fn arc_points(r: f64, sweep_deg: f64, count: usize) -> Vec<P3> {
    (0..count)
        .map(|i| {
            let t = sweep_deg.to_radians() * i as f64 / (count - 1) as f64;
            P3::new(r * t.cos(), r * t.sin(), 0.0)
        })
        .collect()
}

/// The fitter's cap and narration's threshold must both be ENVELOPE × 30.
///
/// On the tapered fixture that is 3.0 × 30 = 90 mm; the tip scale would give
/// 0.5 × 30 = 15 mm. R = 80 mm straddles the two: it is BELOW the envelope
/// cap (so the fitter must accept it and narration must stay silent) and far
/// ABOVE the tip cap (so either side switching scale flips its verdict).
/// R = 120 mm is above both, and pins that the cap exists at all.
#[test]
fn large_arc_threshold_is_envelope_relative_on_both_sides() {
    let def = wrap(Box::new(taper()));
    // The bound `compute/execute.rs` hands the fitter is `tool_diameter/2`.
    let fitter_bound = def.diameter() / 2.0;
    assert!(
        (fitter_bound - def.envelope_radius_mm()).abs() < 1e-12,
        "the fitter's bound and narration's threshold must resolve to the same radius"
    );
    let envelope_cap = fitter_bound * 30.0;
    let tip_cap = def.cusp_radius_mm() * 30.0;
    assert!(
        (80.0 - envelope_cap).abs() > 5.0 && 80.0 > tip_cap * 2.0,
        "fixture must straddle the two caps: envelope {envelope_cap}, tip {tip_cap}"
    );

    // ── fitter side ──
    for (r, should_fit) in [(80.0_f64, true), (120.0_f64, false)] {
        let mut tp = Toolpath::new();
        let pts = arc_points(r, 8.0, 24);
        tp.rapid_to(pts[0]);
        for p in &pts[1..] {
            tp.feed_to(*p, 1000.0);
        }
        let out = fit_arcs(AnnotatedToolpath::new(tp), 0.01, fitter_bound);
        let arcs = out
            .toolpath
            .moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    rs_cam_core::toolpath::MoveType::ArcCW { .. }
                        | rs_cam_core::toolpath::MoveType::ArcCCW { .. }
                )
            })
            .count();
        if should_fit {
            assert!(
                arcs > 0,
                "R={r} is inside the ENVELOPE cap ({envelope_cap} mm) and must fit; if this \
                 fails, arcfit's cap moved to the tip scale ({tip_cap} mm)"
            );
        } else {
            assert_eq!(
                arcs, 0,
                "R={r} exceeds the envelope cap ({envelope_cap} mm) and must be rejected"
            );
        }
    }

    // ── narration side ──
    for (r, should_warn) in [(80.0_f64, false), (120.0_f64, true)] {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(r, 0.0, 0.0));
        // A quarter turn CCW about the origin: I,J point from the start back
        // to the centre, so the observed radius is exactly `r`.
        tp.arc_ccw_to(P3::new(0.0, r, 0.0), -r, 0.0, 1000.0);
        let text = narrate_toolpath(&AnnotatedToolpath::new(tp), None, None, None, &def);
        let warned = text.contains("perimeter sweep arc");
        assert_eq!(
            warned, should_warn,
            "R={r}: narration threshold must be ENVELOPE × 30 = {envelope_cap} mm, not the \
             tip scale ({tip_cap} mm). Narration said warned={warned}"
        );
        if should_warn {
            assert!(
                text.contains("envelope_radius"),
                "the anomaly must name the ENVELOPE radius, not an ambiguous 'tool_radius'"
            );
        }
    }
}

// ── 4. `FinishSurface::cell_source` consistency (PR-0 integration) ───────

/// A flat 20×20 plate at z=0.
fn plate_mesh() -> TriangleMesh {
    let v = vec![
        P3::new(0.0, 0.0, 0.0),
        P3::new(20.0, 0.0, 0.0),
        P3::new(20.0, 20.0, 0.0),
        P3::new(0.0, 20.0, 0.0),
    ];
    TriangleMesh::from_raw(v, vec![[0, 1, 2], [0, 2, 3]])
}

/// Each finish-grid builder must tag the surface with the radius that
/// actually sized its cells, and the tag must be *checkable* — on a tapered
/// tool the two production grids differ by the shaft/tip ratio, which is
/// exactly why the provenance field exists (PR-0 / M1).
#[test]
fn finish_surface_cell_source_names_the_radius_that_sized_the_grid() {
    let mesh = plate_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let cancel = || false;
    let tolerance = 0.01;

    let generation = build_finish_surface_with_cancel(&mesh, &index, &t, tolerance, &cancel)
        .expect("generation surface");
    assert_eq!(generation.cell_source, CellSource::EnvelopeRadius);
    assert!(
        (generation.cell_size() - (t.envelope_radius_mm() / 4.0).max(tolerance)).abs() < 1e-12,
        "EnvelopeRadius provenance must match an envelope-derived cell; got {}",
        generation.cell_size()
    );

    let classification =
        build_classification_surface_with_cancel(&mesh, &index, &t, tolerance, &cancel)
            .expect("classification surface");
    assert_eq!(classification.cell_source, CellSource::CuspRadius);
    assert!(
        (classification.cell_size() - (t.cusp_radius_mm() / 4.0).max(tolerance)).abs() < 1e-12,
        "CuspRadius provenance must match a cusp-derived cell; got {}",
        classification.cell_size()
    );

    // The caller-pinned entry point cannot claim a tool scale.
    let explicit = build_finish_surface_with_cell_size_and_cancel(&mesh, &index, &t, 0.5, &cancel)
        .expect("explicit surface");
    assert_eq!(explicit.cell_source, CellSource::Explicit);
    assert!((explicit.cell_size() - 0.5).abs() < 1e-12);

    // The two production grids really are different scales on this tool —
    // if a future change collapses them, the provenance tag becomes a lie
    // and this assertion is the one that notices.
    assert!(
        generation.cell_size() > classification.cell_size() * 3.0,
        "tapered generation cell {} vs classification cell {} — the shaft/tip split must \
         still be visible",
        generation.cell_size(),
        classification.cell_size()
    );

    // Padding is ENVELOPE on both, per rule 4.
    for (label, s) in [
        ("generation", &generation),
        ("classification", &classification),
    ] {
        let pad = mesh.bbox.min.x - s.heightmap.origin_x;
        assert!(
            (pad - t.envelope_radius_mm()).abs() < 1e-6,
            "{label}: grid padding must stay the full envelope; got {pad}"
        );
    }
}
