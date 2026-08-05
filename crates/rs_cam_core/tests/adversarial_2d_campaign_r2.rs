//! **R2 / H3 — the adversarial 2D campaign.**
//!
//! Nine operation families (pocket, adaptive, profile, trace, zigzag, inlay,
//! v-carve, rest, 2D-driven drill) against the hostile fixture library in
//! `common::adversarial2d`, driven through the production funnel
//! `ProjectSession::generate_toolpath` — the one entry point the GUI worker,
//! the CLI and MCP all share.
//!
//! # Tiers
//!
//! | test | cost | what it is for |
//! |---|---|---|
//! | `adversarial_2d_fixtures_contain_their_mechanism` | geometry only | **non-vacuity.** Every fixture proves it contains the defect class it claims BEFORE it is allowed to gate anything |
//! | `the_reflex_cross_generator_is_bit_identical_to_its_donor` | trivial | C6 donor proof for the one generator lifted from a shipped sentry |
//! | `every_2d_operation_survives_its_worst_fixtures` | CI | the acceptance gate: no panic, no silent-empty success, bounded wall clock |
//! | `no_2d_family_ignores_a_pre_set_cancel_flag` | CI | was `exactly_two_..._ignore_...`, pinning F-4; Checkpoint C Q3 made rest and drill cancellable and the pin was restated deliberately |
//! | `adversarial_2d_full_campaign` | `#[ignore]`, minutes | the full operation × fixture matrix that produces `ADVERSARIAL_2D_FINDINGS.md`'s table and the SVG gallery |
//!
//! # What counts as a failure
//!
//! 1. **A panic.** Never acceptable, on any input, valid or not.
//! 2. **A silent empty success.** An `Ok` result with zero cutting moves on a
//!    fixture declared `Expectation::MustCut` — geometry that demonstrably
//!    contains material reachable by the stated tool. This is the R2
//!    acceptance gate, and it exists because `offset_polygon` maps a
//!    contained cavalier panic, a `< 3`-vertex guard and a genuine geometric
//!    collapse onto the same empty `Vec`, and every 2D consumer maps that
//!    onto a successful empty toolpath.
//! 3. **Blowing the wall-clock ceiling.** Pre-registered per tier, below.
//!
//! A typed `Err` is **not** a failure. On adversarial input it is the best
//! available outcome: it is the only one an operator can see.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use std::time::Duration;

use common::adversarial2d::{self as adv, CancelRecord, Fixture, Outcome, RunRecord, Validity};
use common::session::{polygon_model, toolpath_config};

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, DrillConfig, InlayConfig, PocketConfig, ProfileConfig, RestConfig, TraceConfig,
    VCarveConfig, ZigzagConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;

// ---------------------------------------------------------------------------
// Pre-registered bars (rule 7: these are written before the evidence is run)
// ---------------------------------------------------------------------------

/// Wall-clock ceiling for one generate in a **debug** build.
///
/// Sized from the defect this campaign exists to catch, not from a
/// performance target: the reflex cross ran `pocket_offsets` for 13 minutes.
/// 60 s is two orders below that and two orders above every healthy generate
/// measured here, so it separates "slow debug build" from "does not
/// terminate" without becoming a flaky perf pin.
const CEILING: Duration = Duration::from_secs(60);

/// Ceiling for the CI tier, which is deliberately curated to cheap fixtures.
const CI_CEILING: Duration = Duration::from_secs(30);

/// How long a generate is allowed to run before the campaign sets its cancel
/// flag, and how long it then has to come back.
const CANCEL_AFTER: Duration = Duration::from_millis(150);
const CANCEL_GRACE: Duration = Duration::from_secs(20);

/// Shallow, single-level cuts throughout: the campaign is testing how the
/// generators handle *plan geometry*, and a depth loop only multiplies the
/// same XY answer by the level count.
const DEPTH_MM: f64 = 1.0;

// ---------------------------------------------------------------------------
// Session wiring
// ---------------------------------------------------------------------------

fn bbox(polys: &[Polygon2]) -> (f64, f64, f64, f64) {
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
    if !lo_x.is_finite() {
        return (0.0, 0.0, 1.0, 1.0);
    }
    (lo_x, lo_y, hi_x, hi_y)
}

/// Stock that covers the fixture with 5 mm of margin and hangs BELOW z = 0.
///
/// 2D operations cut at negative Z with the stock top at z = 0, so
/// `origin_z` must be negative — the `project_2d_stock_z_frame` rule, and the
/// reason `common::session::stock_under` exists. That helper assumes a
/// centred square; these fixtures are neither centred nor square.
fn stock_for(polys: &[Polygon2]) -> StockConfig {
    let (lo_x, lo_y, hi_x, hi_y) = bbox(polys);
    let height = 12.0;
    StockConfig {
        x: (hi_x - lo_x) + 10.0,
        y: (hi_y - lo_y) + 10.0,
        z: height,
        origin_x: lo_x - 5.0,
        origin_y: lo_y - 5.0,
        origin_z: -height,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn endmill(diameter: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    t.diameter = diameter;
    t.cutting_length = 25.0;
    t.shank_diameter = diameter;
    t.shank_length = 20.0;
    t.stickout = 40.0;
    t.flute_count = 2;
    t.name = format!("End Mill {diameter}mm");
    t
}

/// A 60° V-bit. Donor: `vcarve_lift_bridge_b1::make_vbit_12_7mm_60deg`, whose
/// dimensions are what the shipped V-carve sentry uses. V-carve and Inlay
/// both refuse any other cutter kind (`vbit_half_angle`), and both read the
/// half angle from the TOOL, never from the config.
fn vbit() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(0), ToolType::VBit);
    t.diameter = 12.7;
    t.cutting_length = 11.0;
    t.included_angle = 60.0;
    t.shank_diameter = 6.35;
    t.shank_length = 20.0;
    t.stickout = 30.0;
    t.flute_count = 2;
    t.name = "V-bit 60deg".to_owned();
    t
}

/// Every 2D operation config the campaign drives, parameterised by the
/// fixture's authored tool diameter.
///
/// `Rest` is absent on purpose: it needs a *second, larger* tool in the
/// session and a `prev_tool_id` pointing at it, which changes the session
/// shape rather than just the operation. [`rest_session`] builds that case.
fn op_matrix(tool_d: f64) -> Vec<(&'static str, OperationConfig, ToolKind)> {
    let stepover = tool_d * 0.4;
    vec![
        (
            "pocket",
            OperationConfig::Pocket(PocketConfig {
                stepover,
                depth: DEPTH_MM,
                depth_per_pass: DEPTH_MM,
                ..PocketConfig::default()
            }),
            ToolKind::EndMill,
        ),
        (
            "adaptive",
            OperationConfig::Adaptive(AdaptiveConfig {
                stepover,
                depth: DEPTH_MM,
                depth_per_pass: DEPTH_MM,
                ..AdaptiveConfig::default()
            }),
            ToolKind::EndMill,
        ),
        (
            "profile",
            OperationConfig::Profile(ProfileConfig {
                depth: DEPTH_MM,
                depth_per_pass: DEPTH_MM,
                ..ProfileConfig::default()
            }),
            ToolKind::EndMill,
        ),
        (
            "trace",
            OperationConfig::Trace(TraceConfig {
                depth: DEPTH_MM,
                depth_per_pass: DEPTH_MM,
                ..TraceConfig::default()
            }),
            ToolKind::EndMill,
        ),
        (
            "zigzag",
            OperationConfig::Zigzag(ZigzagConfig {
                stepover,
                depth: DEPTH_MM,
                depth_per_pass: DEPTH_MM,
                ..ZigzagConfig::default()
            }),
            ToolKind::EndMill,
        ),
        (
            "vcarve",
            OperationConfig::VCarve(VCarveConfig {
                max_depth: DEPTH_MM,
                stepover,
                ..VCarveConfig::default()
            }),
            ToolKind::VBit,
        ),
        (
            "inlay",
            OperationConfig::Inlay(InlayConfig {
                pocket_depth: DEPTH_MM,
                stepover,
                flat_tool_radius: tool_d * 0.5,
                ..InlayConfig::default()
            }),
            ToolKind::VBit,
        ),
        (
            "drill",
            OperationConfig::Drill(DrillConfig {
                depth: DEPTH_MM,
                ..DrillConfig::default()
            }),
            ToolKind::EndMill,
        ),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ToolKind {
    EndMill,
    VBit,
}

fn build_session(fixture: &Fixture, op: OperationConfig, kind: ToolKind) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_for(&fixture.polys));
    let tool = match kind {
        ToolKind::EndMill => endmill(fixture.tool_d),
        ToolKind::VBit => vbit(),
    };
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(polygon_model(fixture.polys.clone(), fixture.name));
    let cfg = toolpath_config(fixture.name, op, tool_id, model_id);
    session
        .add_toolpath(0, cfg)
        .expect("add toolpath to a fresh session");
    session
}

/// Rest needs two tools: a larger previous one and the smaller one doing the
/// rest pass. Below `tool_radius < prev_tool_radius` it returns empty by
/// contract (`rest.rs:70`), so the sizes are not decorative.
fn rest_session(fixture: &Fixture) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_for(&fixture.polys));
    let prev_idx = session.add_tool(endmill(fixture.tool_d * 2.0));
    let prev_id = session.tools()[prev_idx].id;
    let cur_idx = session.add_tool(endmill(fixture.tool_d));
    let cur_id = session.tools()[cur_idx].id.0;
    let model_id = session.add_model(polygon_model(fixture.polys.clone(), fixture.name));
    let op = OperationConfig::Rest(RestConfig {
        prev_tool_id: Some(prev_id),
        stepover: fixture.tool_d * 0.4,
        depth: DEPTH_MM,
        depth_per_pass: DEPTH_MM,
        ..RestConfig::default()
    });
    let cfg = toolpath_config(fixture.name, op, cur_id, model_id);
    session
        .add_toolpath(0, cfg)
        .expect("add rest toolpath to a fresh session");
    session
}

/// Announce a cell BEFORE running it.
///
/// Not decoration. The wall-clock ceiling is checked *after* the generate
/// returns, so a cell that never returns — or that is killed by an external
/// memory bound — leaves no assertion, only a log. The last announcement is
/// then the only thing naming which cell it was — which is how the
/// unbounded cell in `ADVERSARIAL_2D_FINDINGS.md` was identified at all,
/// after a first run reached 22.9 GB RSS with nothing in the log.
fn announce(op: &str, fixture: &str) {
    println!("··· running {op} × {fixture}");
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
}

/// One `(operation, fixture)` cell, run and judged. `None` when the fixture
/// declares the cell non-terminating — see `Fixture::skip_ops`.
fn run_cell(
    fixture: &Fixture,
    op_name: &str,
    op: OperationConfig,
    kind: ToolKind,
) -> Option<RunRecord> {
    if let Some(why) = fixture.skip_reason(op_name) {
        println!("··· SKIPPED {op_name} × {} — {why}", fixture.name);
        return None;
    }
    announce(op_name, fixture.name);
    let mut session = build_session(fixture, op, kind);
    let rec = adv::run_op(&mut session, 0, op_name, fixture.name);
    println!("{}", rec.row());
    Some(rec)
}

fn run_rest_cell(fixture: &Fixture) -> Option<RunRecord> {
    if let Some(why) = fixture.skip_reason("rest") {
        println!("··· SKIPPED rest × {} — {why}", fixture.name);
        return None;
    }
    announce("rest", fixture.name);
    let mut session = rest_session(fixture);
    let rec = adv::run_op(&mut session, 0, "rest", fixture.name);
    println!("{}", rec.row());
    Some(rec)
}

/// Judge one record against the campaign's three failure conditions.
fn violations(rec: &RunRecord, fixture: &Fixture, ceiling: Duration) -> Vec<String> {
    let mut out = Vec::new();
    if let Outcome::Panic(msg) = &rec.outcome {
        out.push(format!("{} × {}: PANIC — {msg}", rec.op, rec.fixture));
    }
    // `rest` is exempt from the silent-empty gate, and the reason is
    // geometric rather than convenient: rest machining's entire job is to
    // cut what a LARGER previous tool could not reach, so on a fixture whose
    // features are all bigger than that previous tool there is legitimately
    // nothing to do, and `rest.rs:70` returns empty by contract when
    // `tool_radius >= prev_tool_radius`. Its empties are RECORDED in the
    // findings table instead of failing here.
    if rec.op != "rest" && rec.is_silent_empty(fixture.expectation) {
        out.push(format!(
            "{} × {}: reported SUCCESS with zero cutting moves on a MustCut \
             fixture ({} moves total). An empty toolpath that reports success \
             is indistinguishable from a contained cavalier panic",
            rec.op, rec.fixture, rec.moves
        ));
    }
    if rec.wall > ceiling {
        out.push(format!(
            "{} × {}: took {:.1} s, ceiling {:.0} s",
            rec.op,
            rec.fixture,
            rec.wall.as_secs_f64(),
            ceiling.as_secs_f64()
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// 1. Non-vacuity — the fixtures are tested before the operations are
// ---------------------------------------------------------------------------

#[test]
fn adversarial_2d_fixtures_contain_their_mechanism() {
    let fixtures = adv::fixtures();
    assert!(
        fixtures.len() >= 20,
        "the R2 brief names nine hostile classes; {} fixtures is too few to \
         cover them with a valid/invalid pair each",
        fixtures.len()
    );

    let mut table = String::from(
        "| fixture | class | verts | rings | reflex | min seg (mm) | \
         sub-eps segs | min gap (mm) | min curv r (mm) | comps | nest | \
         min area (mm²) | self-int |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for f in &fixtures {
        // The gate. Fails loudly and names the measured value.
        f.assert_contains_mechanism();
        let m = f.measure();
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.2e} | {} | {:.2e} | {:.3} | {} | {} | {:.4} | {} |\n",
            f.name,
            f.class.id(),
            m.total_vertices,
            m.rings,
            m.reflex_corners,
            m.min_segment_mm,
            m.segments_below_pos_eps,
            m.min_nonadjacent_gap_mm,
            m.min_curvature_radius_mm,
            m.components,
            m.nesting_depth,
            m.min_area_mm2,
            m.self_intersecting,
        ));
        let path = adv::write_fixture_svg(f, &format!("fixture_{}", f.name));
        assert!(
            path.is_some() || matches!(f.validity, Validity::Invalid(_)),
            "{}: a VALID fixture must render — a fixture nobody has looked at \
             is not evidence",
            f.name
        );
    }

    // Every class the brief names must be present.
    for class in [
        adv::Class::Reflex,
        adv::Class::Dendrite,
        adv::Class::HolesInHoles,
        adv::Class::DisconnectedIslands,
        adv::Class::NearCoincidentWalls,
        adv::Class::ShortEdges,
        adv::Class::NearCollinear,
        adv::Class::TinyIslands,
        adv::Class::ThinSlot,
        adv::Class::HighCurvature,
        adv::Class::InvalidContour,
    ] {
        assert!(
            fixtures.iter().any(|f| f.class == class),
            "no fixture covers the {} class",
            class.id()
        );
    }

    // The `HolesInHoles` bar is "at least one hole level"; at least one
    // fixture must reach a TRUE island-inside-a-hole, or the class name is a
    // claim the library does not back.
    assert!(
        fixtures
            .iter()
            .filter(|f| f.class == adv::Class::HolesInHoles)
            .any(|f| f.measure().nesting_depth >= 3),
        "no holes-in-holes fixture reaches nesting depth 3 — the class is \
         then only 'polygon with holes'"
    );

    let path = adv::out_dir().join("fixture_mechanisms.md");
    std::fs::write(&path, &table).expect("write mechanism table");
    println!("{table}\nmechanism table: {}", path.display());
}

// ---------------------------------------------------------------------------
// 2. Donor proof (C6) — the one generator lifted from shipped code
// ---------------------------------------------------------------------------

/// `adversarial2d::reflex_cross` is the only generator in the new module
/// copied out of production test code rather than authored fresh. C6's rule
/// is that a shared helper must be proven bit-identical to its donor before
/// anything relies on it, because a "shared" fixture that quietly differs
/// from the sentry it came from is worse than two honest copies.
///
/// Donor: `pocket::tests::pocket_cascade_terminates_on_the_reflex_cross`,
/// `crates/rs_cam_core/src/pocket.rs`, `(a, b) = (20.0, 60.0)`.
#[test]
fn the_reflex_cross_generator_is_bit_identical_to_its_donor() {
    use rs_cam_core::geo::P2;
    let (a, b) = (20.0, 60.0);
    let donor = vec![
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
    ];
    let generated = adv::reflex_cross(a, b);
    assert_eq!(
        generated.exterior.len(),
        donor.len(),
        "vertex count drifted from the pocket sentry's literal"
    );
    for (i, (g, d)) in generated.exterior.iter().zip(donor.iter()).enumerate() {
        assert!(
            g.x.to_bits() == d.x.to_bits() && g.y.to_bits() == d.y.to_bits(),
            "vertex {i} differs from the donor: generated ({}, {}), donor ({}, {})",
            g.x,
            g.y,
            d.x,
            d.y
        );
    }
    assert!(generated.holes.is_empty(), "the donor cross has no holes");
    assert!(generated.closed, "the donor cross is closed");
    // And it still contains the mechanism it was famous for.
    assert_eq!(
        adv::reflex_corner_count(&generated.exterior),
        4,
        "the cross's four inner corners are what made it a 13-minute cascade"
    );
}

// ---------------------------------------------------------------------------
// 3. The CI acceptance gate
// ---------------------------------------------------------------------------

/// Every 2D family against the fixtures that historically or structurally
/// hurt it, at CI cost.
///
/// Curated rather than exhaustive: `adversarial_2d_full_campaign` is the
/// exhaustive one and it is `#[ignore]`d for runtime. The three chosen here
/// are the cheapest representatives of the three failure MODES —
/// unbounded cascade (`reflex-cross`), the `Shape::parallel_offset` hole path
/// (`holes-in-holes`), and contract-violating input (`invalid-nan`).
#[test]
fn every_2d_operation_survives_its_worst_fixtures() {
    let all = adv::fixtures();
    let picks: Vec<&Fixture> = ["reflex-cross", "holes-in-holes", "invalid-nan"]
        .iter()
        .map(|want| {
            all.iter()
                .find(|f| f.name == *want)
                .unwrap_or_else(|| panic!("fixture {want} vanished from the registry"))
        })
        .collect();

    let mut failures = Vec::new();
    let mut rows = Vec::new();
    for f in picks.iter().copied() {
        f.assert_contains_mechanism();
        for (name, op, kind) in op_matrix(f.tool_d) {
            if let Some(rec) = run_cell(f, name, op, kind) {
                failures.extend(violations(&rec, f, CI_CEILING));
                rows.push(rec);
            }
        }
        if let Some(rec) = run_rest_cell(f) {
            failures.extend(violations(&rec, f, CI_CEILING));
            rows.push(rec);
        }
    }

    assert!(
        failures.is_empty(),
        "2D adversarial acceptance gate failed:\n{}",
        failures.join("\n")
    );
    // Coverage, asserted rather than assumed: nine families ran.
    let ops: std::collections::BTreeSet<&str> = rows.iter().map(|r| r.op.as_str()).collect();
    assert_eq!(
        ops.len(),
        9,
        "expected all nine 2D families to run, got {ops:?}"
    );
}

// ---------------------------------------------------------------------------
// 3b. The unbounded cascade, proved with a bounded probe
// ---------------------------------------------------------------------------

/// **`pocket_contours_with_cancel`'s ring cascade has no ring cap and no
/// divergence check: its only exit is `rings.is_empty()`** (`pocket.rs:144-170`).
///
/// That is fine while every offset shrinks. It is not fine as a *contract*,
/// because whether the offset shrinks depends on the input's winding, and
/// nothing between the caller and the cascade checks winding. cavalier's
/// offset sign is relative to the polyline's own direction, so on a CW
/// exterior a positive distance grows the ring. The loop then never
/// collapses, never caps, and allocates until the machine gives up — 22.9 GB
/// on the first R2 run, with no assertion, because the wall-clock ceiling is
/// only checked once the call returns.
///
/// This probe caps the iteration itself and measures the **trend**, which is
/// both safe and a better instrument than a timeout: it shows the direction.
///
/// The comment already in `pocket.rs:125-127` says the quiet part —
/// *"Only the cancel hook made that a hang instead of a lock-up, which is not
/// the same thing as being bounded."* M5 removed the vertex-growth
/// mechanism; it did not add a bound.
///
/// # This still passes after Checkpoint C's Q3 fix, and that is correct
///
/// Q3 bounded the **consumer**, not the primitive:
/// `pocket::pocket_contours_reported_with_cancel` now carries a geometric
/// ring cap plus a wall-clock net and reports which one fired
/// (`pocket::CascadeBound`), pinned by
/// `pocket::tests::the_pocket_cascade_is_bounded_on_a_diverging_cw_exterior`
/// on this same CW fixture. `OffsetRingSet::offset` is deliberately left
/// unbounded — a bound is a consumer's policy, not a geometry type's, and
/// scallop's cascade already carries its own `max_rings`.
///
/// So this probe keeps measuring exactly what it always measured: that the
/// PRIMITIVE diverges on a CW exterior. If it ever starts terminating, the
/// primitive has gained a policy and F-10 does need re-ruling.
#[test]
fn the_pocket_ring_cascade_is_bounded_only_by_collapse() {
    use rs_cam_core::polygon::{FlattenPolicy, OffsetRingSet};

    /// Enough rings to establish a trend, few enough to stay instant.
    const CAP: usize = 40;
    const STEP: f64 = 2.4;

    let area_trend = |poly: &Polygon2| -> Vec<f64> {
        let mut rings = OffsetRingSet::from_polygon(poly);
        let mut areas = Vec::new();
        for _ in 0..CAP {
            rings = rings.offset(STEP);
            if rings.is_empty() {
                break;
            }
            let a: f64 = rings
                .to_polygons(FlattenPolicy::untoleranced())
                .iter()
                .map(|p| p.area().abs())
                .sum();
            areas.push(a);
            if !a.is_finite() {
                break;
            }
        }
        areas
    };

    // Control: the SAME square, wound correctly. Must collapse well inside
    // the cap — a 60 mm square eroded 2.4 mm per side per ring is gone by
    // ring 13.
    let ccw = Polygon2::rectangle(0.0, 0.0, 60.0, 60.0);
    let ccw_areas = area_trend(&ccw);
    assert!(
        ccw_areas.len() < CAP,
        "the CCW control must collapse: it ran all {CAP} rings, so this probe \
         cannot distinguish divergence from a cap that is simply too low"
    );
    assert!(
        ccw_areas.windows(2).all(|w| w[1] <= w[0] + 1e-9),
        "the CCW control's area must be non-increasing: {ccw_areas:?}"
    );

    // The defect: the same square, CW. `Polygon2`'s contract says exterior is
    // CCW (`polygon.rs:9-12`) and nothing validates it.
    let cw = adv::reversed_winding(60.0);
    let cw_areas = area_trend(&cw);
    assert_eq!(
        cw_areas.len(),
        CAP,
        "a CW exterior must never collapse — if this now terminates, the \
         cascade has gained a bound or a winding guard, and F-10 in \
         ADVERSARIAL_2D_FINDINGS.md needs re-ruling"
    );
    let (first, last) = (
        cw_areas.first().copied().unwrap_or(0.0),
        cw_areas.last().copied().unwrap_or(0.0),
    );
    assert!(
        last > first * 4.0,
        "a CW exterior must DIVERGE, not merely fail to collapse: ring 1 = \
         {first:.0} mm², ring {CAP} = {last:.0} mm²"
    );
    println!(
        "pocket cascade on a CW 60 mm square: ring 1 = {first:.0} mm² → ring \
         {CAP} = {last:.0} mm² ({:.0}× growth, still not collapsed). CCW \
         control collapsed at ring {}.",
        last / first.max(1.0),
        ccw_areas.len()
    );

    // The reach: no importer can deliver this (`svg_input.rs:161` and
    // `dxf_input.rs` call `ensure_winding`), and `detect_containment`
    // normalises too (`polygon.rs:1079`) — so this is a LOW-reachability
    // trigger for a real unbounded loop, not a live crash. That distinction
    // is the whole severity argument and it is asserted, not assumed.
    let mut normalised = adv::reversed_winding(60.0);
    normalised.ensure_winding();
    assert!(
        normalised.has_correct_winding(),
        "ensure_winding is the guard every importer applies; if it stopped \
         normalising, this finding's reachability changes completely"
    );
}

// ---------------------------------------------------------------------------
// 4. The cancellation asymmetry, pinned
// ---------------------------------------------------------------------------

/// **All nine 2D families in this campaign now read their cancel flag.**
///
/// This test was called `exactly_two_2d_families_ignore_a_pre_set_cancel_flag`
/// and it pinned F-4: `generate_rest` built no `cancel_fn` and called the
/// non-cancellable `depth::toolpath_at_levels`, and `generate_drill` never
/// touched `ctx.cancel` at all. Its own doc said what it was for — *"it pins
/// the measured fact so the findings document cannot go stale silently and a
/// future fix has a red-first target"* — and it did exactly that: making the
/// two cancellable under Checkpoint C, Q3 turned it red, which is how this
/// restatement came to be written on purpose rather than by drift.
///
/// The fix (2026-08-05): `generate_rest` polls `ctx.cancel` as its first
/// statement and runs its levels through `toolpath_at_levels_with_cancel`;
/// `generate_drill` polls as its first statement. Both are also in
/// `compute::execute`'s in-crate `cancellable_families_honour_a_preset_cancel_flag`
/// case list, which is the coverage claim `ExecutionContext`'s doc says goes
/// stale otherwise. Registry-wide that leaves AlignmentPinDrill and Chamfer,
/// neither of which is in this campaign's nine.
///
/// **Rest's first statement is the cancel check, ahead of its prev-tool
/// precondition.** That ordering is deliberate and this test depends on it:
/// a rest op with no previous tool would otherwise report the precondition
/// error and a pre-set flag would look ignored.
///
/// The flag is set **synchronously, before the call**
/// ([`adv::run_op_precancelled`]), and that detail is the whole test.
///
/// > **An earlier version of this test used the timer-based
/// > [`adv::run_op_with_cancel`] at a zero delay and reported five ignorers on
/// > one run and three on the next**, because the timer thread's store races
/// > the generate. It very nearly became a finding — "profile, trace and
/// > zigzag are uncancellable at one depth level" — with a plausible
/// > mechanism attached (their per-level generators take no cancel
/// > parameter). The mechanism is real; the conclusion was an artefact of the
/// > instrument. Three consecutive runs of the synchronous version give
/// > `["drill", "rest"]` every time.
///
/// What remains true about `profile`/`trace`/`zigzag` is narrower and is
/// measured elsewhere, by
/// `cancellable_2d_families_return_after_the_flag_is_set`: they read the flag
/// *between* Z levels, so they cannot interrupt work already inside one. On
/// the rosette fixture all three report `NOT EXERCISED (finished first)` —
/// honest, and not the same claim.
///
/// The empty-`ignored` assertion is kept as an assertion rather than deleted:
/// a family that stops polling is the regression this file exists to catch,
/// and "nobody ignores it" is a claim that has to keep being checked.
#[test]
fn no_2d_family_ignores_a_pre_set_cancel_flag() {
    let all = adv::fixtures();
    let f = all
        .iter()
        .find(|f| f.name == "reflex-cross")
        .expect("reflex-cross fixture");

    // Pre-set the flag, then generate. A family that polls it at all must
    // come back with a cancellation error before emitting anything.
    let mut ignored = Vec::new();
    let mut honoured = Vec::new();
    for (name, op, kind) in op_matrix(f.tool_d) {
        let mut session = build_session(f, op, kind);
        let rec = adv::run_op_precancelled(&mut session, 0, name, f.name);
        match &rec.outcome {
            Outcome::Err(msg) if msg.to_ascii_lowercase().contains("cancel") => {
                honoured.push(name);
            }
            _ => ignored.push(name),
        }
    }
    {
        let mut session = rest_session(f);
        let rec = adv::run_op_precancelled(&mut session, 0, "rest", f.name);
        match &rec.outcome {
            Outcome::Err(msg) if msg.to_ascii_lowercase().contains("cancel") => {
                honoured.push("rest");
            }
            _ => ignored.push("rest"),
        }
    }

    ignored.sort_unstable();
    honoured.sort_unstable();
    println!("honoured a pre-set cancel flag: {honoured:?}");
    println!("IGNORED a pre-set cancel flag:  {ignored:?}");

    assert!(
        ignored.is_empty(),
        "these 2D families ignored a pre-set cancel flag: {ignored:?}. \
         Checkpoint C (Q3) closed F-4 by making rest and drill cancellable, \
         so any name here is a REGRESSION — an operator's cancel button does \
         nothing for it"
    );
    assert_eq!(
        honoured,
        vec![
            "adaptive", "drill", "inlay", "pocket", "profile", "rest", "trace", "vcarve", "zigzag"
        ],
        "all nine read the flag at least once before emitting"
    );
    // Reachability, so the list above cannot silently describe dead code.
    for op in [
        OperationType::Rest,
        OperationType::Drill,
        OperationType::Profile,
        OperationType::Trace,
        OperationType::Zigzag,
    ] {
        assert!(
            OperationType::ALL_2D.contains(&op),
            "{op:?} must be in the 2D menu for this finding to matter"
        );
    }
}

/// Measure how long each cancellable family takes to come back after the flag
/// is set, on a fixture big enough that it is still running when the flag
/// arrives.
///
/// Records rather than gates below the grace bound: a family that checks its
/// flag only between Z levels is *correct* on a single-level fixture and
/// still uncancellable in the way that matters. The record is what
/// `ADVERSARIAL_2D_FINDINGS.md` reports.
#[test]
fn cancellable_2d_families_return_after_the_flag_is_set() {
    let all = adv::fixtures();
    let f = all
        .iter()
        .find(|f| f.name == "rosette-24")
        .expect("rosette-24 fixture");

    let mut records: Vec<CancelRecord> = Vec::new();
    let mut failures = Vec::new();
    for (name, op, kind) in op_matrix(f.tool_d) {
        if name == "drill" {
            continue; // declared uncancellable; see the test above
        }
        let mut session = build_session(f, op, kind);
        let rec = adv::run_op_with_cancel(&mut session, 0, name, f.name, CANCEL_AFTER);
        if rec.total > CANCEL_AFTER + CANCEL_GRACE {
            failures.push(format!(
                "{} × {}: still running {:.1} s after the cancel flag was set \
                 at {:.2} s (grace {:.0} s)",
                rec.op,
                rec.fixture,
                rec.total.as_secs_f64(),
                rec.set_after.as_secs_f64(),
                CANCEL_GRACE.as_secs_f64()
            ));
        }
        records.push(rec);
    }

    for r in &records {
        println!(
            "| {} | {} | {} | total {:.3} s | {} |",
            r.op,
            r.fixture,
            r.label(),
            r.total.as_secs_f64(),
            r.outcome.label()
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------------------
// 5. The full matrix — the findings table's source
// ---------------------------------------------------------------------------

/// The exhaustive operation × fixture matrix, with per-cell wall clock, RSS
/// growth, outcome, emitted topology, and a rendered toolpath for every cell
/// that emitted anything.
///
/// `#[ignore]` because it is nine families × twenty-two fixtures of hostile
/// geometry in a debug build. Run it with:
///
/// ```text
/// cargo test -p rs_cam_core --test adversarial_2d_campaign_r2 -- \
///     --ignored --nocapture adversarial_2d_full_campaign
/// ```
///
/// Set `R2_ARTIFACT_DIR` to write the SVG gallery somewhere other than
/// `target/adversarial_2d/`.
#[test]
#[ignore = "the full 9 × 22 hostile matrix: minutes in debug, and it writes an \
            SVG gallery. This is the campaign that produces \
            ADVERSARIAL_2D_FINDINGS.md; the CI teeth are \
            every_2d_operation_survives_its_worst_fixtures."]
fn adversarial_2d_full_campaign() {
    let fixtures = adv::fixtures();
    let mut rows = String::from(
        "| op | fixture | wall | outcome | moves | cutting | cut mm | runs | RSS growth |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );
    let mut failures = Vec::new();
    let mut renders = Vec::new();

    for f in &fixtures {
        f.assert_contains_mechanism();
        let mut cells: Vec<(String, OperationConfig, ToolKind)> = op_matrix(f.tool_d)
            .into_iter()
            .map(|(n, o, k)| (n.to_owned(), o, k))
            .collect();
        // Rest last, because it is the one that needs a second tool.
        for (name, op, kind) in cells.drain(..) {
            if let Some(why) = f.skip_reason(&name) {
                rows.push_str(&format!(
                    "| {name} | {} | — | SKIPPED | | | | | {why} |\n",
                    f.name
                ));
                println!("··· SKIPPED {name} × {} — {why}", f.name);
                continue;
            }
            announce(&name, f.name);
            let mut session = build_session(f, op, kind);
            let rec = adv::run_op(&mut session, 0, &name, f.name);
            failures.extend(violations(&rec, f, CEILING));
            println!("{}", rec.row());
            rows.push_str(&rec.row());
            rows.push('\n');
            if rec.moves > 0
                && let Some(r) = session.get_result(0)
                && let Some(p) =
                    adv::write_toolpath_svg(f, r.toolpath(), &format!("{}_{}", name, f.name))
            {
                renders.push(p);
            }
        }
        if f.skip_reason("rest").is_none() {
            announce("rest", f.name);
            let mut session = rest_session(f);
            let rec = adv::run_op(&mut session, 0, "rest", f.name);
            failures.extend(violations(&rec, f, CEILING));
            println!("{}", rec.row());
            rows.push_str(&rec.row());
            rows.push('\n');
            if rec.moves > 0
                && let Some(r) = session.get_result(0)
                && let Some(p) =
                    adv::write_toolpath_svg(f, r.toolpath(), &format!("rest_{}", f.name))
            {
                renders.push(p);
            }
        }
    }

    let table_path = adv::out_dir().join("campaign_matrix.md");
    std::fs::write(&table_path, &rows).expect("write campaign matrix");
    println!("{rows}");
    println!("matrix: {}", table_path.display());
    println!(
        "{} toolpath renders in {}",
        renders.len(),
        adv::out_dir().display()
    );

    assert!(
        failures.is_empty(),
        "adversarial 2D campaign findings ({} cells failed):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
