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
use rs_cam_core::dressup::EntrySurfaceProbe;
use rs_cam_core::dropcutter::point_drop_cutter;
use rs_cam_core::entry_audit::{buried_fed_chords, is_entry_intent};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_core::transform_provenance::ReconcileSet;

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
    };
    let dressed = apply_dressups(
        AnnotatedToolpath::new(tp),
        &cfg,
        FEED,
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
#[ignore = "red until the G-RAMPTERRAIN clip lands; S1-red recorded in FINDINGS.md"]
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
#[ignore = "red until the G-RAMPTERRAIN clip lands; S1-red recorded in FINDINGS.md"]
fn helix_entry_never_cuts_below_surface() {
    let d = dress_on_ridge(7.0, DressupEntryStyle::Helix);
    assert!(
        count_intent(&d.toolpath, MoveIntent::EntryHelix) > 0,
        "fixture must still produce a helix entry (a silent fallback to \
         plunge would fake a green)"
    );
    assert_no_buried_entries(&d, "helix on ridge");
}

/// No-intrusion parity: an entry at x = -20 ramping toward -X never
/// meets the ridge, so the fix must keep the legacy two-leg shape.
#[test]
fn unclipped_ramp_keeps_two_legs() {
    let mesh = ridge_mesh();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = FlatEndmill::new(1.0, 25.0);
    let z0 = point_drop_cutter(-20.0, 0.0, &mesh, &index, &cutter).z;
    let z_next = point_drop_cutter(-28.0, 0.0, &mesh, &index, &cutter).z;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-20.0, 0.0, SAFE_Z));
    tp.feed_to(P3::new(-20.0, 0.0, z0), FEED);
    tp.feed_to(P3::new(-28.0, 0.0, z_next), FEED);
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
    };
    let dressed = apply_dressups(
        AnnotatedToolpath::new(tp),
        &cfg,
        FEED,
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
