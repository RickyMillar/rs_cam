//! Phase 0 — a disconnected finishing run has the required
//! retract / entry structure between its islands.
//!
//! # The item
//!
//! Nothing asserted this property. The nearest neighbour,
//! `capability_link_moves_safety.rs:1180`, drives the same 4-island mesh
//! but counts `MoveIntent::Retract` only as a vacuity precondition, and it
//! FORCES the retracts by setting `intra_pass_hookup_mm: 0.0`. This test
//! runs the shipped dials instead and asserts what the run must look like
//! where the surface breaks.
//!
//! # Why the invariant is a DISJUNCTION, not "always retract"
//!
//! Two shipped mechanisms may legally carry the tool from one island to
//! the next without a full safe-Z round trip:
//!
//! - the intra-pass relink (`scallop.rs:2672` gates it on
//!   `intra_pass_hookup_mm`, `surface_link::relink_fragments` performs it)
//!   emits a drop-cutter-sampled SURFACE link when the XY gap is inside
//!   the hookup cap;
//! - G-LINKVETO (2026-08-27, `surface_link.rs:917-923`) additionally lets a
//!   LIFTED link leave the operation's territory, when the caller declares
//!   `airborne_links_may_leave_territory` — `unified_finish.rs:2345` does.
//!
//! So the contract is: EITHER the transition retracts to the operation's
//! safe Z and descends again, OR it is a fed link that stays on the
//! protected surface. A test that demanded a retract would forbid a link
//! the product ships.
//!
//! # What is CONTRACT and what is PINNED OBSERVATION
//!
//! CONTRACT (a failure is a defect):
//!
//! - every inter-island transition takes one of the two branches;
//! - a retract branch reaches the operation's safe Z, and the descent that
//!   follows it is fed and goes down;
//! - a fed-link branch buries no chord below the drop-cutter surface
//!   (`entry_audit::buried_fed_chords`);
//! - no fed move takes the tool outside an island's machining territory
//!   (`entry_audit::fed_moves_outside_region`).
//!
//! PINNED OBSERVATION (a failure is a change to read, then re-pin):
//!
//! - the descent intent is `MoveIntent::EntryPlunge`. The Phase 0 brief
//!   expected `MoveIntent::Linking`, from `entry_descent_profile_b2.rs:233`.
//!   That reading is about the RAMP DRESSUP's inserted descent, which is a
//!   different emitter. Both Scallop descents — the raw discrete branch
//!   (`scallop.rs:2626`) and the relink retract arm
//!   (`surface_link.rs:1080`) — tag `EntryPlunge`. FLIP NOTE: if this
//!   assertion fails, read the emitter and re-pin the tag. Do not relax
//!   the assertion to "any intent".
//! - the branch split. Today every inter-island transition retracts,
//!   because the island centres are 30 mm apart and the shipped scallop
//!   hookup cap is far smaller, so `surface_link`'s `too_far` arm
//!   (`surface_link.rs:877-880`) fires on every one. The split is a
//!   DIAL-DEPENDENT observation, not a contract: raise the cap and fed
//!   links become legal here.
//!
//! # Scope
//!
//! This test changes no production code. It adds one shared mesh builder,
//! `common::meshes::disconnected_hemispheres`, which reproduces
//! `capability_link_moves_safety.rs`'s local `scallop_island_mesh`
//! vertex for vertex. That file keeps its own copy: it carries no
//! `mod common;`, and `common/mod.rs`'s migration policy says a working
//! sentry moves over only when it is being edited anyway.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::meshes::disconnected_hemispheres;
use rs_cam_core::compute::operation_configs::ScallopConfig;
use rs_cam_core::dressup::entry_audit::{buried_fed_chords, fed_moves_outside_region};
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::scallop::{ScallopParams, scallop_toolpath};
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

/// The 4 island centres, 30 mm apart on both axes. Copied from
/// `capability_link_moves_safety.rs`'s `SCALLOP_ISLAND_CENTERS`.
const ISLAND_CENTRES: [(f64, f64); 4] =
    [(-15.0, -15.0), (15.0, -15.0), (-15.0, 15.0), (15.0, 15.0)];

/// Radius of each hemisphere dome (mm). Copied from the same fixture.
const ISLAND_RADIUS: f64 = 5.0;

/// Tessellation of each dome. Copied from the same fixture.
const ISLAND_DIVISIONS: usize = 10;

/// The cutter is a ball nose, which is what Scallop's tool constraint
/// accepts (`compute/execute.rs:2472-2480`).
const TOOL_DIAMETER: f64 = 3.0;

/// Cutting length of that cutter (mm).
const TOOL_CUTTING_LENGTH: f64 = 25.0;

/// The operation's safe Z.
///
/// The shipped op reads `ctx.heights.retract_z`
/// (`compute/execute.rs:2492`), and the default `HeightMode::Auto`
/// resolves that from the machine and stock context
/// (`compute/config.rs:1613`). The bare generator this test drives has no
/// such context, so this value stands in for it — the same 30.0 the
/// neighbouring fixture uses.
const SAFE_Z: f64 = 30.0;

/// Tolerance for "this rapid reached safe Z" (mm).
const SAFE_Z_EPS: f64 = 1e-6;

/// Sample spacing for both `entry_audit` checkers (mm).
const SAMPLE_SPACING_MM: f64 = 0.25;

/// Reporting tolerance for both `entry_audit` checkers (mm).
const AUDIT_TOLERANCE_MM: f64 = 0.05;

/// Segment count of each region circle.
const REGION_SEGMENTS: usize = 256;

// ── Fixture ─────────────────────────────────────────────────────────────

/// The shipped Scallop dials, as `generate_scallop` builds them.
///
/// Every field comes from `ScallopConfig::default()`, so the hookup cap is
/// the shipped one and NOT the 0.0 the neighbouring fixture forces.
fn shipped_params() -> ScallopParams {
    let cfg = ScallopConfig::default();
    assert!(
        cfg.intra_pass_hookup_mm > 0.0,
        "this test must run with the shipped relink ON; the default hookup \
         cap read 0.0, which forces a retract at every junction and makes \
         the disjunction below vacuous"
    );
    assert!(
        !cfg.iso_field,
        "the shipped default takes the offset cascade, which is the door \
         `scallop_toolpath` opens; `iso_field` now defaults ON, so this \
         fixture drives the wrong generator"
    );
    ScallopParams {
        scallop_height: cfg.scallop_height,
        tolerance: cfg.tolerance,
        direction: cfg.direction,
        continuous: cfg.continuous,
        slope_from: cfg.slope_from,
        slope_to: cfg.slope_to,
        feed_rate: cfg.feed_rate,
        plunge_rate: cfg.plunge_rate,
        safe_z: SAFE_Z,
        stock_to_leave: cfg.stock_to_leave,
        intra_pass_hookup_mm: cfg.intra_pass_hookup_mm,
        link_kinematics: None,
    }
}

/// Index of the island centre nearest to `(x, y)`.
///
/// The reach discs are disjoint — 30 mm between centres against a
/// `ISLAND_RADIUS + tool radius` reach — so the nearest centre names the
/// island a cutting move belongs to, and no threshold is needed.
fn nearest_island(x: f64, y: f64) -> usize {
    let mut best = 0usize;
    let mut best_d2 = f64::INFINITY;
    for (i, &(cx, cy)) in ISLAND_CENTRES.iter().enumerate() {
        let d2 = (x - cx).powi(2) + (y - cy).powi(2);
        if d2 < best_d2 {
            best_d2 = d2;
            best = i;
        }
    }
    best
}

/// One island's machining territory, as a circle around its centre.
///
/// The caller passes `ISLAND_RADIUS + 2 × tool_radius`, and the second
/// tool radius is not slack. `entry_audit`'s `disc_outside_polygon`
/// measures the tool DISC against the raw polygon: inside the polygon it
/// answers `tool_radius − distance_to_boundary`. A region of
/// `ISLAND_RADIUS + tool_radius` therefore demands a CENTRE LINE inside
/// `ISLAND_RADIUS`, which a dome flank breaks — the drop-cutter CL of a
/// ball of radius `r` on a sphere cap of radius `R` reaches `R + r` in XY,
/// at the rim. So the region that means "the CL is over this island" is
/// `R + r`, expanded by `r` for the disc test.
///
/// The polygon CIRCUMSCRIBES the circle, so the discretisation does not
/// eat the tolerance.
fn island_regions(radius_mm: f64) -> Vec<Polygon2> {
    let step = 2.0 * std::f64::consts::PI / REGION_SEGMENTS as f64;
    let r = radius_mm / (std::f64::consts::PI / REGION_SEGMENTS as f64).cos();
    let mut regions = Vec::new();
    for &(cx, cy) in &ISLAND_CENTRES {
        let mut ring = Vec::with_capacity(REGION_SEGMENTS);
        for i in 0..REGION_SEGMENTS {
            let a = step * i as f64;
            ring.push(P2::new(cx + r * a.cos(), cy + r * a.sin()));
        }
        regions.push(Polygon2::new(ring));
    }
    regions
}

fn is_rapid(m: &Move) -> bool {
    matches!(m.move_type, MoveType::Rapid)
}

fn is_cut(m: &Move) -> bool {
    !is_rapid(m) && m.intent == MoveIntent::FinishingCut
}

/// A maximal run of consecutive cutting moves over one island.
#[derive(Debug, Clone, Copy)]
struct IslandRun {
    island: usize,
    first_cut: usize,
    last_cut: usize,
    cuts: usize,
}

fn island_runs(tp: &Toolpath) -> Vec<IslandRun> {
    let mut runs: Vec<IslandRun> = Vec::new();
    for (i, m) in tp.moves.iter().enumerate() {
        if !is_cut(m) {
            continue;
        }
        let island = nearest_island(m.target.x, m.target.y);
        if runs.last().is_some_and(|r| r.island == island) {
            if let Some(run) = runs.last_mut() {
                run.last_cut = i;
                run.cuts += 1;
            }
        } else {
            runs.push(IslandRun {
                island,
                first_cut: i,
                last_cut: i,
                cuts: 1,
            });
        }
    }
    runs
}

// ── The test ────────────────────────────────────────────────────────────

#[test]
fn disconnected_finish_retracts_or_links_safely_between_islands() {
    let mesh = disconnected_hemispheres(&ISLAND_CENTRES, ISLAND_RADIUS, ISLAND_DIVISIONS);
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = BallEndmill::new(TOOL_DIAMETER, TOOL_CUTTING_LENGTH);
    let tool_radius = cutter.radius();
    let params = shipped_params();

    // The REAL generator — the function `compute::execute::generate_scallop`
    // calls. No dressup runs, so every move below is the generator's own.
    let tp = scallop_toolpath(&mesh, &index, &cutter, &params);
    assert!(
        !tp.moves.is_empty(),
        "the fixture must produce a non-empty scallop toolpath"
    );

    // ── Non-vacuity: the run really does fragment over the 4 islands ────
    let runs = island_runs(&tp);
    assert!(
        runs.len() >= ISLAND_CENTRES.len(),
        "the fixture must break into at least {} island runs for the \
         transition invariant to have anything to judge; got {}",
        ISLAND_CENTRES.len(),
        runs.len(),
    );
    for (i, &(cx, cy)) in ISLAND_CENTRES.iter().enumerate() {
        let mut cuts = 0usize;
        for run in &runs {
            if run.island == i {
                cuts += run.cuts;
            }
        }
        assert!(
            cuts >= 1,
            "island {i} at ({cx}, {cy}) got no cutting move; the fixture no \
             longer machines every island, so the partition below measures \
             fewer islands than it names"
        );
    }

    let mut transitions: Vec<(IslandRun, IslandRun)> = Vec::new();
    for pair in runs.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a.island != b.island {
            transitions.push((a, b));
        }
    }
    assert!(
        transitions.len() >= 3,
        "the fixture must cross between islands at least 3 times; got {}",
        transitions.len(),
    );

    // ── The surface probe the fed-link branch is judged against ─────────
    let probe = EntrySurfaceProbe {
        mesh: &mesh,
        index: &index,
        cutter: &cutter,
        stock_to_leave: params.stock_to_leave,
        // The finishing door's policy. `floor_z` does not read it, so it
        // does not move the audit; it names the caller class.
        off_mesh: OffMeshEntry::PlungeFallback,
        // Fresh stock: this pass is not rest driven.
        rest_stock: None,
    };
    let buried_chords =
        buried_fed_chords(&tp, &probe, SAMPLE_SPACING_MM, AUDIT_TOLERANCE_MM, |i| {
            i == MoveIntent::Linking
        });
    let buried: Vec<usize> = buried_chords.iter().map(|c| c.move_index).collect();

    // ── The disjunction, transition by transition ───────────────────────
    let mut retract_transitions = 0usize;
    let mut fed_link_transitions = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for (a, b) in &transitions {
        let start = a.last_cut + 1;
        let end = b.first_cut;
        if start >= end {
            failures.push(format!(
                "moves {start}..{end}: island {} cuts straight into island \
                 {} with no transition move at all",
                a.island, b.island,
            ));
            continue;
        }

        let mut last_rapid = None;
        let mut retract_at_safe_z = false;
        for (i, m) in tp.moves.iter().enumerate().take(end).skip(start) {
            if !is_rapid(m) {
                continue;
            }
            last_rapid = Some(i);
            if m.intent == MoveIntent::Retract && m.target.z >= SAFE_Z - SAFE_Z_EPS {
                retract_at_safe_z = true;
            }
        }

        let Some(rapid_at) = last_rapid else {
            // BRANCH B — a fed link that must stay on the protected surface.
            fed_link_transitions += 1;
            for i in start..end {
                if buried.contains(&i) {
                    failures.push(format!(
                        "move {i}: the fed link between island {} and island \
                         {} sinks below the drop-cutter surface",
                        a.island, b.island,
                    ));
                }
            }
            continue;
        };

        // BRANCH A — retract to safe Z, then descend again.
        retract_transitions += 1;
        if !retract_at_safe_z {
            failures.push(format!(
                "moves {start}..{end}: the transition lifts the tool but no \
                 rapid carries MoveIntent::Retract to safe Z {SAFE_Z}"
            ));
            continue;
        }
        let descent_start = rapid_at + 1;
        if descent_start >= end {
            failures.push(format!(
                "moves {start}..{end}: the retract is not followed by any \
                 descent before the next cut"
            ));
            continue;
        }
        let first = &tp.moves[descent_start];
        let before = &tp.moves[descent_start - 1];
        if first.target.z >= before.target.z {
            failures.push(format!(
                "move {descent_start}: the move after the retract does not \
                 descend ({:.4} -> {:.4})",
                before.target.z, first.target.z,
            ));
        }
        for (i, m) in tp.moves.iter().enumerate().take(end).skip(descent_start) {
            // PINNED OBSERVATION — see the module doc's flip note.
            if m.intent != MoveIntent::EntryPlunge {
                failures.push(format!(
                    "move {i}: the descent after the retract is tagged {:?}, \
                     not MoveIntent::EntryPlunge",
                    m.intent,
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} inter-island transitions break the retract-or-safe-link \
         invariant. Branch split on this run: {retract_transitions} retract, \
         {fed_link_transitions} fed link. Failures: {}",
        failures.len(),
        transitions.len(),
        failures.join("; "),
    );

    // PINNED OBSERVATION — the split, not the count. The island centres are
    // 30 mm apart and the shipped hookup cap is far smaller, so
    // `surface_link`'s `too_far` arm rejects every candidate link here. A
    // change here is a dial change to read, not a defect.
    assert_eq!(
        fed_link_transitions,
        0,
        "today every inter-island transition retracts. A fed link appeared, \
         so either the hookup cap or the island spacing moved. Read which, \
         then re-pin. The split was {retract_transitions} retract, \
         {fed_link_transitions} fed link over {} transitions.",
        transitions.len(),
    );
    assert_eq!(
        retract_transitions,
        transitions.len(),
        "every inter-island transition must take one of the two branches"
    );

    // ── No fed move leaves its island's territory ───────────────────────
    let regions = island_regions(ISLAND_RADIUS + 2.0 * tool_radius);
    assert!(
        !regions.is_empty(),
        "`fed_moves_outside_region` returns EMPTY on an empty region, which \
         is NOT MEASURED and never clean — the region must be built first"
    );
    let mut inside_fed = 0usize;
    for m in &tp.moves {
        if is_rapid(m) || m.intent == MoveIntent::Retract {
            continue;
        }
        let xy = P2::new(m.target.x, m.target.y);
        if regions.iter().any(|p| p.contains_point(&xy)) {
            inside_fed += 1;
        }
    }
    assert!(
        inside_fed > 0,
        "no fed move lands inside any island region, so the containment \
         check below would pass on an empty population"
    );
    let outside = fed_moves_outside_region(
        &tp,
        &regions,
        tool_radius,
        SAMPLE_SPACING_MM,
        AUDIT_TOLERANCE_MM,
        |i| i != MoveIntent::Retract,
    );
    assert!(
        outside.is_empty(),
        "{} fed move(s) take the cutter outside an island's territory; worst \
         {:.4} mm past the region at {:?}",
        outside.len(),
        outside.first().map_or(0.0, |c| c.max_outside_mm),
        outside.first().map(|c| c.worst),
    );
}
