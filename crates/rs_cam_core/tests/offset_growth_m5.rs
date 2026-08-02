//! **M5 research, step 1+2: reproduce the vertex inflation, then attribute it.**
//!
//! The claim under test is the one scallop's cascade comment makes and no
//! other consumer has ever checked:
//!
//! > `offset_polygon` ADDS vertices on every call (concave corners sprout
//! > arc-approximation points; none are ever removed), so an iterated cascade
//! > compounds ~15–25% vertices per ring on concave boundaries.
//!
//! Scallop pays for that with drop-only decimation after every ring. Nothing
//! else in the workspace does — and `offset_polygon` is the shared primitive
//! behind pocket, adaptive, adaptive3d, rest, boundary, profile, trace,
//! zigzag, inlay, project-curve and `region_set`.
//!
//! This file measures growth (with and without that compensation) and then
//! classifies every vertex the primitive hands back, so Checkpoint D decides
//! against numbers rather than against the comment.
//!
//! ```text
//! cargo test --release -p rs_cam_core --test offset_growth_m5 \
//!     -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The non-ignored tests are the reproduction pins and run in seconds.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::indexing_slicing, clippy::print_stdout, clippy::print_stderr)]

mod common;

use common::offset_lab::{
    Cascade, CascadeBudget, Cleanup, OffsetFixture, RingAttribution, attribute_offset, cascade,
    fixtures, out_dir, ring_vertex_count, rosette, square, write_rings_svg,
};
use rs_cam_core::polygon::Polygon2;

/// The compensation scallop applies, expressed for an arbitrary fixture:
/// `0.75 × the density the boundary was authored at`.
fn shipped_cleanup(fx: &OffsetFixture) -> Cleanup {
    Cleanup::DropOnly {
        min_spacing: fx.authored_spacing * 0.75,
    }
}

fn budget() -> CascadeBudget {
    CascadeBudget::default()
}

// ---------------------------------------------------------------------------
// Reproduction pins — fast, always run
// ---------------------------------------------------------------------------

/// The control. A convex boundary has no reflex corner for the offset to
/// arc-join, so 20 inward offsets must not inflate it at all. If this ever
/// fails, the growth measured below is not about concavity.
#[test]
fn a_convex_boundary_does_not_inflate() {
    let fx = OffsetFixture {
        name: "square",
        what: "control",
        poly: square(100.0),
        step: 0.5,
        authored_spacing: 0.5,
    };
    let run = cascade(
        &fx,
        Cleanup::Raw,
        &CascadeBudget {
            max_rings: 20,
            ..budget()
        },
        false,
    );
    let start = fx.vertices();
    for s in &run.stats {
        assert!(
            s.verts <= start,
            "convex control grew {} -> {} vertices by ring {}",
            start,
            s.verts,
            s.ring
        );
    }
}

/// **The reproduction.** A concave boundary inflates, badly, and the run ends
/// on the vertex cap rather than on collapse.
#[test]
fn a_concave_boundary_inflates_geometrically() {
    let fx = OffsetFixture {
        name: "rosette-24",
        what: "stress",
        poly: rosette(60.0, 8.0, 24, 720),
        step: 0.1,
        authored_spacing: 0.5,
    };
    let run = cascade(
        &fx,
        Cleanup::Raw,
        &CascadeBudget {
            max_rings: 25,
            vertex_cap: 60_000,
            seconds: 60.0,
        },
        false,
    );
    let growth = run.geometric_growth_pct();
    assert!(
        growth > 5.0,
        "expected geometric vertex growth on a concave cascade; measured {growth:.2}%/ring \
         over {} rings ({} -> {} vertices)",
        run.stats.len(),
        fx.vertices(),
        run.last().map_or(0, |s| s.verts)
    );
}

/// And the shipped compensation removes it — which is the reason scallop is
/// the only consumer that has never seen this defect.
#[test]
fn drop_only_decimation_keeps_the_same_cascade_bounded() {
    let fx = OffsetFixture {
        name: "rosette-24",
        what: "stress",
        poly: rosette(60.0, 8.0, 24, 720),
        step: 0.1,
        authored_spacing: 0.5,
    };
    let run = cascade(
        &fx,
        shipped_cleanup(&fx),
        &CascadeBudget {
            max_rings: 25,
            vertex_cap: 60_000,
            seconds: 60.0,
        },
        false,
    );
    let worst = run.worst_ring_growth_pct();
    let last = run.last().map_or(0, |s| s.verts);
    assert!(
        last < fx.vertices() * 2,
        "decimated cascade still inflated: {} -> {last} vertices (worst ring +{worst:.1}%)",
        fx.vertices()
    );
}

/// The property both the production decimator and the lab copy depend on:
/// it only ever DROPS points, so ring placement is never moved by it.
#[test]
fn drop_only_decimation_never_adds_a_point() {
    for fx in fixtures() {
        let before = ring_vertex_count(&fx.poly);
        let after = shipped_cleanup(&fx)
            .apply(&fx.poly)
            .map_or(0, |p| ring_vertex_count(&p));
        assert!(
            after <= before,
            "{}: decimation added points ({before} -> {after})",
            fx.name
        );
    }
}

/// Non-vacuity for the attribution instrument: it must talk to the same
/// cavalier entry points `offset_polygon` does and see the same vertex count
/// come back.
#[test]
fn the_attribution_instrument_agrees_with_the_shipped_primitive() {
    for fx in fixtures() {
        let a = attribute_offset(&fx.poly, fx.step);
        let shipped: usize = rs_cam_core::polygon::offset_polygon(&fx.poly, fx.step)
            .iter()
            .map(ring_vertex_count)
            .sum();
        // `offset_polygon` repairs self-intersections first and re-pairs
        // holes to containers; on clean fixtures the vertex totals must match
        // exactly, and on the captured ones they must not diverge wildly.
        let diff = (a.out_verts as f64 - shipped as f64).abs();
        let tol = (shipped as f64 * 0.05).max(4.0);
        assert!(
            diff <= tol,
            "{}: attribution saw {} vertices, offset_polygon returned {shipped}",
            fx.name,
            a.out_verts
        );
    }
}

// ---------------------------------------------------------------------------
// Evidence: growth curves
// ---------------------------------------------------------------------------

fn curve_row(fx: &OffsetFixture, cleanup: Cleanup, run: &Cascade) -> String {
    let last = run.last();
    format!(
        "| {:<16} | {:<26} | {:>6} | {:>7} | {:>8} | {:>7.2} | {:>7.2} | {:>7.1} | {:<11} |",
        fx.name,
        cleanup.label(),
        fx.vertices(),
        run.stats.len(),
        last.map_or(0, |s| s.verts),
        run.geometric_growth_pct(),
        run.worst_ring_growth_pct(),
        run.total_secs,
        run.stopped.label(),
    )
}

#[test]
#[ignore = "M5 evidence: repeated offset cascades on 8 fixtures; --ignored --nocapture"]
fn growth_curves_with_and_without_todays_compensation() {
    let mut csv = String::from("fixture,cleanup,ring,polys,verts,area_mm2,seconds\n");
    println!(
        "\n| fixture          | cleanup                    | verts0 | rings   | verts_end | %/ring  | worst%  | secs    | stop        |"
    );
    println!(
        "|------------------|----------------------------|--------|---------|-----------|---------|---------|---------|-------------|"
    );
    for fx in fixtures() {
        for cleanup in [Cleanup::Raw, shipped_cleanup(&fx)] {
            let keep = matches!(cleanup, Cleanup::Raw) && fx.name == "rosette-24";
            let run = cascade(&fx, cleanup, &budget(), keep);
            println!("{}", curve_row(&fx, cleanup, &run));
            for s in &run.stats {
                csv.push_str(&format!(
                    "{},{},{},{},{},{:.4},{:.6}\n",
                    fx.name,
                    cleanup.label(),
                    s.ring,
                    s.polys,
                    s.verts,
                    s.area,
                    s.secs
                ));
            }
            if keep {
                write_rings_svg(&fx.poly, &run.rings, 40, "rosette_raw_rings");
            }
        }
    }
    // The picture, not just the curve: the same fixture under the shipped
    // compensation, so the two SVGs can be laid side by side.
    let fx = fixtures()
        .into_iter()
        .find(|f| f.name == "rosette-24")
        .expect("stress fixture present");
    let run = cascade(&fx, shipped_cleanup(&fx), &budget(), true);
    write_rings_svg(&fx.poly, &run.rings, 40, "rosette_decimated_rings");

    let path = out_dir().join("growth_curves.csv");
    std::fs::write(&path, csv).expect("write growth CSV");
    println!("\nper-ring CSV: {}", path.display());
    println!("ring SVGs:    {}", out_dir().display());
}

// ---------------------------------------------------------------------------
// Evidence: attribution
// ---------------------------------------------------------------------------

fn attribution_row(name: &str, ring: usize, a: &RingAttribution) -> String {
    let pct = |n: usize| {
        if a.out_verts == 0 {
            0.0
        } else {
            100.0 * n as f64 / a.out_verts as f64
        }
    };
    format!(
        "| {:<16} | {:>4} | {:>7} | {:>8} | {:>+6} | {:>6.1}% | {:>7} | {:>7} | {:>6.1}% | {:>6.1}% | {:>6.1}% |",
        name,
        ring,
        a.in_verts,
        a.out_verts,
        a.added(),
        if a.in_verts == 0 {
            0.0
        } else {
            100.0 * a.added() as f64 / a.in_verts as f64
        },
        a.out_plines,
        a.arc_segments,
        pct(a.arc_vertices),
        pct(a.duplicates + a.collinear_1nm),
        pct(a.collinear_1um),
    )
}

#[test]
#[ignore = "M5 evidence: per-ring vertex attribution; --ignored --nocapture"]
fn where_the_new_vertices_come_from() {
    println!(
        "\nClasses are measured on the FLATTENED output — exactly the vertices `Polygon2::from_pline` keeps."
    );
    println!(
        "arc% = vertices touching a cavalier ARC join. lossless% = duplicates + collinear<1nm. <1um% = collinear within 1 um.\n"
    );
    println!(
        "| fixture          | ring |  in    |   out    | added  | growth |  plines | arc segs |   arc% | lossless |  <1um% |"
    );
    println!(
        "|------------------|------|--------|----------|--------|--------|---------|----------|--------|----------|--------|"
    );
    let mut csv = String::from(
        "fixture,ring,in_verts,out_verts,out_plines,arc_segments,arc_vertices,duplicates,collinear_1nm,collinear_1um\n",
    );
    for fx in fixtures() {
        // Attribution walks the RAW cascade — the compensated one is a
        // different question (it is measured in the growth table above).
        let mut current: Vec<Polygon2> = vec![fx.poly.clone()];
        for ring in 1..=20usize {
            let mut agg = RingAttribution::default();
            let mut next = Vec::new();
            for poly in &current {
                let a = attribute_offset(poly, fx.step);
                agg.in_verts += a.in_verts;
                agg.out_verts += a.out_verts;
                agg.out_plines += a.out_plines;
                agg.arc_segments += a.arc_segments;
                agg.arc_vertices += a.arc_vertices;
                agg.duplicates += a.duplicates;
                agg.collinear_1nm += a.collinear_1nm;
                agg.collinear_1um += a.collinear_1um;
                next.extend(rs_cam_core::polygon::offset_polygon(poly, fx.step));
            }
            if next.is_empty() {
                break;
            }
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                fx.name,
                ring,
                agg.in_verts,
                agg.out_verts,
                agg.out_plines,
                agg.arc_segments,
                agg.arc_vertices,
                agg.duplicates,
                agg.collinear_1nm,
                agg.collinear_1um
            ));
            if ring <= 3 || ring % 5 == 0 {
                println!("{}", attribution_row(fx.name, ring, &agg));
            }
            if agg.out_verts > 150_000 {
                println!("| {:<16} | stopped at the vertex cap", fx.name);
                break;
            }
            current = next;
        }
    }
    let path = out_dir().join("attribution.csv");
    std::fs::write(&path, csv).expect("write attribution CSV");
    println!("\nper-ring CSV: {}", path.display());
}

// ---------------------------------------------------------------------------
// Fixture capture
// ---------------------------------------------------------------------------

/// Extract a REAL mid-steep band polygon from `tests/fixtures/terrain.stl` and
/// write it to `test_data/m5_terrain_mid_steep_polygon.json`, so the bench
/// above has a captured real fixture that is not wanaka's play-file.
///
/// Run once; the asset is committed.
#[test]
#[ignore = "capture: full-mesh classification sampling; --ignored --nocapture"]
fn capture_terrain_mid_steep_polygon() {
    use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose_surface};
    use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
    use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
    use rs_cam_core::tool::BallEndmill;

    let path = common::repo_root().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    assert!(
        path.exists(),
        "terrain fixture missing at {}",
        path.display()
    );
    let mesh = TriangleMesh::from_stl_scaled(&path, 1.0).expect("terrain.stl loads");
    let index = SpatialIndex::build(&mesh, 10.0);

    let cutter = BallEndmill::new(6.0, 25.0);
    let cancel = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &cutter, 0.05, &cancel)
        .expect("classification surface");
    let planned = decompose_surface(&surface, &[], &FinishPlannerParams::for_tool(3.0));

    let region = planned
        .regions
        .iter()
        .filter(|r| r.band == FinishBand::MidSteep)
        .max_by_key(|r| ring_vertex_count(&r.polygon))
        .expect("terrain must decompose to at least one mid-steep region");

    println!(
        "captured mid-steep region: {} exterior vertices, {} holes, {:.0} mm^2 XY-projected",
        region.polygon.exterior.len(),
        region.polygon.holes.len(),
        region.polygon.area()
    );
    let json = common::offset_lab::capture_json(&region.polygon, 0.05);
    let out = common::offset_lab::terrain_capture_path();
    std::fs::write(&out, json).expect("write capture");
    println!("wrote {}", out.display());
}
