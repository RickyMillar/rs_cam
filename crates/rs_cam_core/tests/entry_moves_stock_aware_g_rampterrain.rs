//! **G-RAMPTERRAIN** — entry moves are stock aware.
//!
//! # The defect
//!
//! The ramp-entry dressup replaced each plunge with two straight legs.
//! Each leg is ~19 mm long at the default 3° angle and drops ~1.0 mm.
//! No code read the mesh along the leg, so on open terrain the legs cut
//! through ridges (877 buried feed chords measured on the wanaka ISO
//! scallop export, midpoints up to 2.6 mm below terrain). The
//! rapid-collision checker audits only rapids, so a fed gouge was
//! invisible to it. Record: `planning/entry_moves_2026-09-03/`.
//!
//! Operator ruling: "All entry moves should be stock aware."
//!
//! # What is pinned
//!
//! On a Gaussian ridge that rises inside the entry legs' reach:
//!
//! - S1: no entry-intent sample of the final dressed toolpath sits
//!   more than 0.2 mm below `drop-cutter CL + stock_to_leave`
//!   (`entry_audit::buried_fed_chords`). Pre-fix this failed with a
//!   ~4 mm burial (recorded in FINDINGS.md §S1).
//! - The entry still ramps / helixes (the fix clips the legs; it must
//!   not silently degrade every entry to a plunge).
//! - An entry whose legs never meet the surface keeps the legacy
//!   two-leg shape (no churn where the terrain does not intrude).

mod common;

use common::meshes::height_field;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::execute::apply_dressups;
use rs_cam_core::dressup::entry_audit::{buried_fed_chords, is_entry_intent};
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface::dropcutter::point_drop_cutter;
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::trace::transform_provenance::ReconcileSet;

const RIDGE_HEIGHT_MM: f64 = 6.0;
const SAFE_Z: f64 = 15.0;
const FEED: f64 = 800.0;
/// S1 tolerance: facet sag plus 0.5 mm sampling (FINDINGS.md §S1).
const TOL_MM: f64 = 0.2;

/// Gaussian ridge along Y, crest at x = 10, sigma 4. The crest rises
/// ~5.7 mm above the entry column at x = 0, inside a 19 mm ramp leg.
fn ridge_mesh() -> TriangleMesh {
    height_field(30.0, 0.5, |x, _| {
        RIDGE_HEIGHT_MM * (-((x - 10.0) * (x - 10.0)) / 32.0).exp()
    })
}

/// A plunge at `(x, y)` onto the drop-cutter surface, then a cut move
/// toward +X. `apply_entry` detects the plunge and steers the entry
/// along +X — into the ridge.
fn plunging_toolpath(
    x: f64,
    y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &FlatEndmill,
) -> Toolpath {
    let z0 = point_drop_cutter(x, y, mesh, index, cutter).z;
    let z_next = point_drop_cutter(x + 25.0, y, mesh, index, cutter).z;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(x, y, SAFE_Z));
    tp.feed_to(P3::new(x, y, z0), FEED);
    tp.feed_to(P3::new(x + 25.0, y, z_next), FEED);
    tp
}

struct Dressed {
    toolpath: Toolpath,
    mesh: TriangleMesh,
    index: SpatialIndex,
    cutter: FlatEndmill,
}

/// Run the plunging toolpath through `apply_dressups` with the given
/// entry style and the surface probe, on the ridge mesh.
fn dress_on_ridge(entry_x: f64, style: DressupEntryStyle) -> Dressed {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    // Ø1 flat endmill: the CL surface tracks the terrain closely, so
    // the fixture's burial numbers are terrain numbers, not
    // footprint-radius artifacts.
    let cutter = FlatEndmill::new(1.0, 25.0);
    let tp = plunging_toolpath(entry_x, 0.0, &mesh, &index, &cutter);
    let cfg = DressupConfig {
        entry_style: style,
        ramp_angle: 3.0,
        helix_radius: 2.0,
        helix_pitch: 1.0,
        // Finish-role default: arc fitting on. The sentry audits the
        // FINAL dressed path, so post-entry transforms must run.
        arc_fitting: true,
        ..DressupConfig::default()
    };
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        // No rest stock in this fixture: the model surface IS the material,
        // which is what the G-RAMPTERRAIN legs are measured against.
        rest_stock: None,
    };
    let dressed = apply_dressups(
        AnnotatedToolpath::new(tp),
        &cfg,
        FEED,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        cutter.diameter(),
        SAFE_Z,
        RIDGE_HEIGHT_MM,
        None,
        None,
        Some(&cutter),
        Some(probe),
        OperationType::Scallop.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    );
    Dressed {
        toolpath: dressed.toolpath,
        mesh,
        index,
        cutter,
    }
}

fn assert_no_buried_entries(d: &Dressed, label: &str) {
    let probe = EntrySurfaceProbe {
        mesh: &d.mesh,
        index: &d.index,
        cutter: &d.cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        // No rest stock in this fixture: the model surface IS the material,
        // which is what the G-RAMPTERRAIN legs are measured against.
        rest_stock: None,
    };
    let reports = buried_fed_chords(&d.toolpath, &probe, 0.5, TOL_MM, is_entry_intent);
    assert!(
        reports.is_empty(),
        "{label}: {} entry move(s) buried below the drop-cutter surface; \
         worst {:.2} mm at ({:.1}, {:.1}, {:.2}) on move {} ({:?}, chord {:.1} mm)",
        reports.len(),
        reports
            .iter()
            .map(|r| r.max_burial_mm)
            .fold(f64::NEG_INFINITY, f64::max),
        reports.first().map_or(0.0, |r| r.worst.x),
        reports.first().map_or(0.0, |r| r.worst.y),
        reports.first().map_or(0.0, |r| r.worst.z),
        reports.first().map_or(0, |r| r.move_index),
        reports.first().map(|r| r.intent),
        reports.first().map_or(0.0, |r| r.chord_len_mm),
    );
}

fn count_intent(tp: &Toolpath, intent: MoveIntent) -> usize {
    tp.moves.iter().filter(|m| m.intent == intent).count()
}

/// S1, ramp arm: a 3° ramp entry at the ridge foot, legs toward the
/// crest. Pre-fix the legs buried ~4 mm.
#[test]
fn ramp_entry_never_cuts_below_surface() {
    let d = dress_on_ridge(0.0, DressupEntryStyle::Ramp);
    assert!(
        count_intent(&d.toolpath, MoveIntent::EntryRamp) > 0,
        "fixture must still produce a ramp entry (a silent fallback to \
         plunge would fake a green)"
    );
    assert_no_buried_entries(&d, "ramp on ridge");
}

/// S1, helix arm: a 2 mm-radius helix on the ridge flank. The circle's
/// uphill side meets the surface near the bottom turns; pre-fix it
/// buried ~1.4 mm.
#[test]
fn helix_entry_never_cuts_below_surface() {
    let d = dress_on_ridge(7.0, DressupEntryStyle::Helix);
    assert!(
        count_intent(&d.toolpath, MoveIntent::EntryHelix) > 0,
        "fixture must still produce a helix entry (a silent fallback to \
         plunge would fake a green)"
    );
    assert_no_buried_entries(&d, "helix on ridge");
}

/// No-intrusion parity: an entry at x = -8 ramping toward -X never
/// meets the ridge (the legs reach x = -27, still on the mesh), so the
/// fix must keep the legacy two-leg shape.
#[test]
fn unclipped_ramp_keeps_two_legs() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = FlatEndmill::new(1.0, 25.0);
    let z0 = point_drop_cutter(-8.0, 0.0, &mesh, &index, &cutter).z;
    let z_next = point_drop_cutter(-16.0, 0.0, &mesh, &index, &cutter).z;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-8.0, 0.0, SAFE_Z));
    tp.feed_to(P3::new(-8.0, 0.0, z0), FEED);
    tp.feed_to(P3::new(-16.0, 0.0, z_next), FEED);
    let cfg = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 3.0,
        // Bare entry transform: this test pins the emitted leg count,
        // and arc fitting may legally merge or split moves.
        arc_fitting: false,
        ..DressupConfig::default()
    };
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        // No rest stock in this fixture: the model surface IS the material,
        // which is what the G-RAMPTERRAIN legs are measured against.
        rest_stock: None,
    };
    let dressed = apply_dressups(
        AnnotatedToolpath::new(tp),
        &cfg,
        FEED,
        // WP22: no operation in scope, so the plunge cap does not apply.
        None,
        cutter.diameter(),
        SAFE_Z,
        RIDGE_HEIGHT_MM,
        None,
        None,
        Some(&cutter),
        Some(probe),
        OperationType::Scallop.transform_capabilities(),
        None,
        None,
        &mut ReconcileSet::empty(),
    );
    assert_eq!(
        count_intent(&dressed.toolpath, MoveIntent::EntryRamp),
        2,
        "an entry the terrain never intrudes on keeps the legacy \
         two-leg ramp"
    );
    let d = Dressed {
        toolpath: dressed.toolpath,
        mesh,
        index,
        cutter,
    };
    assert_no_buried_entries(&d, "ramp away from ridge");
}

/// S2: lead-in/out arcs are stock-aware (FINDINGS.md §S2).
///
/// The lead plunge lands `lead_radius` away from the ring start and
/// the lead arc approaches horizontally at ring depth — terrain-blind
/// before the fix. This test measures BOTH sides on one fixture:
///
/// - the probe-less path (the legacy behaviour, kept for 2D prisms)
///   must show a buried lead sample > 0.5 mm — the permanent proof
///   that the instrument sees this class (S2-red);
/// - the probed path must show no entry-intent burial > 0.2 mm and
///   still carry `LeadIn` moves (S2-green).
#[test]
fn lead_in_arcs_never_cut_below_surface() {
    use rs_cam_core::dressup::apply_lead_in_out_with_provenance;
    use rs_cam_core::trace::transform_provenance::ReconcileSet;

    let mesh = ridge_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = FlatEndmill::new(1.0, 25.0);
    // Entry on the ridge flank: the 2 mm lead circle reaches uphill,
    // so the horizontal lead at ring z bites the slope.
    let entry = 7.0;
    let z0 = point_drop_cutter(entry, 0.0, &mesh, &index, &cutter).z;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(entry, 0.0, SAFE_Z));
    tp.feed_to(P3::new(entry, 0.0, z0), FEED);
    // The cut runs along -Y (constant z within Z_LEVEL_EPS), so the
    // lead-in pass detector fires and its perpendicular offset points
    // UPHILL (+X): the lead circle reaches the rising flank.
    tp.feed_to(P3::new(entry, -8.0, z0), FEED);

    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        // No rest stock in this fixture: the model surface IS the material,
        // which is what the G-RAMPTERRAIN legs are measured against.
        rest_stock: None,
    };

    // S2-red: the probe-less path buries a lead sample.
    let legacy = apply_lead_in_out_with_provenance(
        AnnotatedToolpath::new(tp.clone()),
        2.0,
        None,
        None,
        None,
        // No retract plane offered: this arm measures the LEG geometry, not
        // the pre-position rapid's height (G-ISOCLIPRAPID).
        None,
    )
    .reconcile(&mut ReconcileSet::empty())
    .into_inner();
    let legacy_buried = buried_fed_chords(&legacy.toolpath, &probe, 0.5, 0.5, is_entry_intent);
    assert!(
        legacy_buried.iter().any(|r| r.max_burial_mm > 0.5),
        "the probe-less lead must bury > 0.5 mm on this fixture, or the \
         instrument cannot see the class (S2-red); worst: {:?}",
        legacy_buried
            .iter()
            .map(|r| r.max_burial_mm)
            .fold(f64::NEG_INFINITY, f64::max)
    );

    // S2-green: the probed path lifts the lead to the surface.
    let probed = apply_lead_in_out_with_provenance(
        AnnotatedToolpath::new(tp),
        2.0,
        None,
        None,
        Some(&probe),
        None,
    )
    .reconcile(&mut ReconcileSet::empty())
    .into_inner();
    assert!(
        count_intent(&probed.toolpath, MoveIntent::LeadIn) > 0,
        "the probed fixture must still produce a lead-in (a silent skip \
         would fake a green)"
    );
    let d = Dressed {
        toolpath: probed.toolpath,
        mesh,
        index,
        cutter,
    };
    assert_no_buried_entries(&d, "lead-in on ridge");
}

/// S3: a refit arc must be faithful to its source polyline in Z
/// (FINDINGS.md §S3).
///
/// `try_fit_arc` skipped the per-point Z check whenever the run's
/// ENDPOINTS agreed in Z, so a ring run that climbs over a knoll and
/// returns to level collapsed into a flat arc THROUGH the knoll (the
/// wanaka signature: `entry_load` peak 3.90 mm at tolerance 0.05).
/// Pinned here: the knoll run stays linear; a genuinely planar run
/// and a genuine helix still fit (the fix must not kill real arcs).
#[test]
fn refit_arcs_reject_z_bumps_but_keep_real_arcs() {
    use rs_cam_core::dressup::arcfit::fit_arcs;
    use rs_cam_core::toolpath::MoveType;

    let arc_run = |z_of: &dyn Fn(f64) -> f64| -> Toolpath {
        let mut tp = Toolpath::new();
        let r = 10.0;
        // 19 points, 0..90 deg on a circle: an ideal XY arc candidate.
        let n = 18;
        for k in 0..=n {
            let frac = k as f64 / n as f64;
            let a = std::f64::consts::FRAC_PI_2 * frac;
            let p = P3::new(r * a.cos(), r * a.sin(), z_of(frac));
            if k == 0 {
                tp.rapid_to(p);
            } else {
                tp.feed_to_with_intent(p, FEED, MoveIntent::FinishingCut);
            }
        }
        tp
    };
    let arcs_in = |tp: &Toolpath| -> usize {
        tp.moves
            .iter()
            .filter(|m| {
                matches!(
                    m.move_type,
                    MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
                )
            })
            .count()
    };
    let tolerance = 0.05;

    // A NARROW knoll: one point at z = 0.5, level neighbours. A small
    // greedy window spans it with level endpoints, which the pre-fix
    // code accepted with no interior check — the arc then holds level
    // z THROUGH the knoll. (A wide smooth bump does not reproduce the
    // defect: the greedy extension fails the helix check before any
    // level-endpoint window forms, and the sub-arcs it accepts are
    // z-faithful. Measured while building this fixture.)
    let spike_frac = 0.5;
    let spike = fit_arcs(
        AnnotatedToolpath::new(arc_run(&|f| {
            if (f - spike_frac).abs() < 1e-9 {
                0.5
            } else {
                0.0
            }
        })),
        tolerance,
        3.0,
    );
    // Fidelity, not arc count: the knoll point must survive in the
    // output path. Linearize arcs and measure the output's maximum z
    // anywhere near the spike's XY.
    let r = 10.0;
    let spike_a = std::f64::consts::FRAC_PI_2 * spike_frac;
    let (sx, sy) = (r * spike_a.cos(), r * spike_a.sin());
    let mut max_z_near_spike = f64::NEG_INFINITY;
    let mut prev = None;
    for m in &spike.toolpath.moves {
        let pts: Vec<P3> = match (prev, m.move_type) {
            (Some(p), MoveType::ArcCW { i, j, .. }) => {
                rs_cam_core::geometry::arc_util::linearize_arc(p, m.target, i, j, true, 0.1)
            }
            (Some(p), MoveType::ArcCCW { i, j, .. }) => {
                rs_cam_core::geometry::arc_util::linearize_arc(p, m.target, i, j, false, 0.1)
            }
            _ => vec![m.target],
        };
        for q in pts {
            if ((q.x - sx).powi(2) + (q.y - sy).powi(2)).sqrt() < 0.3 {
                max_z_near_spike = max_z_near_spike.max(q.z);
            }
        }
        prev = Some(m.target);
    }
    assert!(
        max_z_near_spike > 0.5 - 2.0 * tolerance,
        "the 0.5 mm knoll point must survive the refit; the output \
         passes at z = {max_z_near_spike:.3} under it — a flat arc \
         through the knoll (the wanaka buried-arc class)"
    );

    // Parity: a genuinely planar run still fits.
    let planar = fit_arcs(AnnotatedToolpath::new(arc_run(&|_| 0.0)), tolerance, 3.0);
    assert!(
        arcs_in(&planar.toolpath) > 0,
        "a planar circular run must still collapse into an arc"
    );

    // Parity: a genuine helix (z linear with swept angle) still fits.
    let helix = fit_arcs(
        AnnotatedToolpath::new(arc_run(&|f| -2.0 * f)),
        tolerance,
        3.0,
    );
    assert!(
        arcs_in(&helix.toolpath) > 0,
        "a true helical run must still collapse into an arc"
    );
}
