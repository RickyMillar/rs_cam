//! G-AIRLADDER — measure the roughing depth ladder from EMITTED MOTION.
//!
//! # What this file is for
//!
//! `planning/airrun_2026-08-19/RUN_LOG.md` records a `narrate_toolpath` Z
//! ladder on wanaka200 toolpath 5 of `25.8 / 21.6 / 17.4 / 13.2 / 9.0` with
//! first material contact at `6.648`, read at the time as "five depth levels
//! cutting entirely in air". Two things had to be separated before that was a
//! defect at all, and this file separates them by measurement rather than by
//! argument:
//!
//! 1. **Nominal vs achieved.** `narrate::summarize_z_levels` (`narrate.rs:711`)
//!    prefers the `SpanPayload::DepthPass` payload `z_level` over emitted move
//!    Z whenever the spans are valid, so its ladder can be a statement about
//!    the *plan*. CLAUDE.md flags it as nominal-not-achieved, and misreading it
//!    as achieved already produced one retracted finding on this project
//!    (G-WANAKA-FLIP). Every number in this file is read off `Toolpath::moves`
//!    — the repo's "measure emitted motion, not the plan" rule.
//! 2. **Above the model is not the same as above the stock.** Roughing *must*
//!    clear the stock standing above the model surface; that is the job, not
//!    waste. Only motion above the **world stock top** is unambiguously wasted.
//!    `HeightsConfig::resolve` takes `top_z` from `ctx.stock_top_z`, which has
//!    been in the emission frame since the heights/setup-frame audit of
//!    2026-06-12, so the world stock top is the honest bar.
//!
//! # The fixture
//!
//! Shaped like the wanaka case in miniature: 25 mm of stock at
//! `origin_z = -20` (world top `+5`), a flat mesh surface 13 mm below that
//! top at world `Z = -8`, one identity-setup `adaptive3d` rough with
//! `depth_per_pass = 4.2`. Everything else is the shipped default, including
//! `post.safe_z = 10` and `Adaptive3dEntryStyle::Plunge`.
//!
//! # What the two ladders actually are
//!
//! There are two different Z ladders in an `adaptive3d` toolpath and they do
//! not have the same root:
//!
//! - the **Z-level plan** (`adaptive3d/path.rs:448`) starts at
//!   `params.stock_top_z - depth_per_pass` and walks down. It is anchored on
//!   the emission-frame stock top, so it cannot put a level above the stock.
//!   This is the ladder that reaches narration as a `DepthPass` payload.
//! - the **peck-plunge entry ladder** (`adaptive3d/path.rs:39`,
//!   `emit_peck_plunge`) starts at `params.safe_z` — the *retract plane*, not
//!   the stock top — and steps down by `depth_per_pass` at plunge feed with a
//!   retract between rungs. Its topmost rungs sit above the stock whenever
//!   `safe_z > stock_top + depth_per_pass`, and they are emitted as real
//!   feed-rate moves.
//!
//! `identity_setup_emission_frame_audit.rs` deliberately measures **lateral**
//! cutting moves only, on the grounds that a vertical feed descent through air
//! above the stock is not evidence of a frame bug. That is right for the
//! question that file asks and wrong for this one: the wasted motion here *is*
//! the vertical descent. This file therefore counts every cutting move,
//! lateral or plunge, and reports the split so the two are never conflated.
//!
//! # Reading a run
//!
//! `wasted_fed_cutting_levels_above_the_world_stock_top` asserts the healthy
//! condition — zero. It prints nothing (`print_stdout` is denied repo-wide);
//! a green run therefore means the count is zero, and a red run names every
//! offending level, its move counts and the fed path length above the stock
//! top. Either outcome states the number.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{mesh_model, single_op_session};

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{SAFE_Z_CLEARANCE_MM, effective_safe_z};
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::{SpanKind, SpanPayload};

// ── Fixture constants ───────────────────────────────────────────────────
//
// Named rather than inlined because every assertion message quotes them:
// a red run has to be readable without opening this file.

/// Stock thickness — wanaka200's `stock.z`.
const STOCK_THICKNESS_MM: f64 = 25.0;
/// Stock origin — wanaka200's `origin_z` as recorded in the run log, so the
/// world stock top lands at `+5`.
const STOCK_ORIGIN_Z_MM: f64 = -20.0;
/// World Z of the stock's top face. The bar every "is this air?" question in
/// this file is measured against.
const WORLD_STOCK_TOP_MM: f64 = STOCK_ORIGIN_Z_MM + STOCK_THICKNESS_MM;
/// Flat model surface, 13 mm below the stock top. The band between this and
/// `WORLD_STOCK_TOP_MM` is stock the rough is *supposed* to remove.
const MODEL_SURFACE_Z_MM: f64 = -8.0;
/// wanaka200's `depth_per_pass` on both roughs, to three decimals.
const DEPTH_PER_PASS_MM: f64 = 4.2;

/// Z clustering tolerance. Two cutting moves ending within this of each other
/// are one level. Well below any spacing the planner or the peck loop can
/// produce (`depth_per_pass` is 4.2 mm) and well above f64 accumulation noise.
const Z_EPS_MM: f64 = 1e-3;

/// XY tolerance separating a lateral cutting move from a pure-Z plunge.
const XY_EPS_MM: f64 = 1e-9;

// ── Fixture ─────────────────────────────────────────────────────────────

/// Flat two-triangle surface at `z`, spanning XY [10, 50] x [10, 50] — well
/// inside the 60 x 60 stock so the boundary never clips the measurement.
fn flat_quad_mesh(z: f64) -> TriangleMesh {
    let verts = vec![
        P3::new(10.0, 10.0, z),
        P3::new(50.0, 10.0, z),
        P3::new(50.0, 50.0, z),
        P3::new(10.0, 50.0, z),
    ];
    let tris = vec![[0u32, 1, 2], [0, 2, 3]];
    TriangleMesh::from_raw(verts, tris)
}

fn build_wanaka_shaped_rough() -> ProjectSession {
    let stock = StockConfig {
        x: 60.0,
        y: 60.0,
        z: STOCK_THICKNESS_MM,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: STOCK_ORIGIN_Z_MM,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };

    // Only the two dials the wanaka rough actually differs from default on.
    // Leaving the rest alone keeps this a measurement of shipped behaviour.
    let adaptive = Adaptive3dConfig {
        depth_per_pass: DEPTH_PER_PASS_MM,
        ..Adaptive3dConfig::default()
    };

    single_op_session(
        stock,
        make_endmill_6mm(),
        mesh_model(flat_quad_mesh(MODEL_SURFACE_Z_MM), "flat_plate"),
        "Rough",
        OperationConfig::Adaptive3d(adaptive),
    )
}

// ── Emitted-motion instrument ───────────────────────────────────────────

/// One distinct cutting Z, with the move population that produced it.
///
/// `fed_moves` counts every cutting move ending at this Z; `lateral_moves`
/// counts the subset that also moved in XY. A level whose two counts are
/// equal is a real cutting pass; a level where `lateral_moves == 0` is
/// nothing but vertical entry descents.
struct Level {
    z: f64,
    fed_moves: usize,
    lateral_moves: usize,
}

/// Everything this file measures, from `Toolpath::moves` alone.
struct EmittedZProfile {
    /// Distinct cutting Z levels, descending.
    levels: Vec<Level>,
    /// Cutting-move path length lying strictly above the measurement plane.
    /// This is time on the clock at feed rate, not rapid rate.
    fed_len_above_plane_mm: f64,
    /// Total cutting moves that also moved in XY, across all levels.
    lateral_cut_moves: usize,
}

/// Length of the portion of segment `a -> b` that lies strictly above
/// `plane_z`. Linear in Z, so a segment crossing the plane contributes only
/// its upper part.
fn segment_length_above(a: P3, b: P3, plane_z: f64) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let dz = b.z - a.z;
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    if len <= 0.0 {
        return 0.0;
    }
    let above_a = a.z > plane_z;
    let above_b = b.z > plane_z;
    if above_a && above_b {
        return len;
    }
    if !above_a && !above_b {
        return 0.0;
    }
    let span = (a.z - b.z).abs();
    if span <= 0.0 {
        return 0.0;
    }
    len * ((a.z.max(b.z) - plane_z) / span).clamp(0.0, 1.0)
}

/// Walk the emitted moves once and build the whole profile.
///
/// The previous position is seeded from the first move's target, so the very
/// first move contributes zero length. A toolpath's first move is a rapid to
/// the retract plane in every shipped generator, so this discards nothing that
/// could be cutting motion — and it avoids inventing an origin the machine
/// was never at.
fn profile_emitted_motion(tp: &Toolpath, plane_z: f64) -> EmittedZProfile {
    let mut levels: Vec<Level> = Vec::new();
    let mut fed_len_above_plane_mm = 0.0;
    let mut lateral_cut_moves = 0usize;

    let Some(first) = tp.moves.first() else {
        return EmittedZProfile {
            levels,
            fed_len_above_plane_mm,
            lateral_cut_moves,
        };
    };
    let mut prev = first.target;

    for mv in &tp.moves {
        let target = mv.target;
        if !mv.move_type.is_cutting() {
            prev = target;
            continue;
        }
        let lateral =
            (target.x - prev.x).abs() > XY_EPS_MM || (target.y - prev.y).abs() > XY_EPS_MM;
        if lateral {
            lateral_cut_moves += 1;
        }
        match levels
            .iter_mut()
            .find(|l| (l.z - target.z).abs() <= Z_EPS_MM)
        {
            Some(level) => {
                level.fed_moves += 1;
                level.lateral_moves += usize::from(lateral);
            }
            None => levels.push(Level {
                z: target.z,
                fed_moves: 1,
                lateral_moves: usize::from(lateral),
            }),
        }
        fed_len_above_plane_mm += segment_length_above(prev, target, plane_z);
        prev = target;
    }

    levels.sort_by(|a, b| b.z.total_cmp(&a.z));
    EmittedZProfile {
        levels,
        fed_len_above_plane_mm,
        lateral_cut_moves,
    }
}

fn render_levels(levels: &[&Level]) -> String {
    levels
        .iter()
        .map(|l| {
            format!(
                "Z = {:+9.3}  ({} cutting moves, {} of them lateral)",
                l.z, l.fed_moves, l.lateral_moves
            )
        })
        .collect::<Vec<_>>()
        .join("\n      ")
}

/// Generate toolpath 0 and profile it against the world stock top.
fn measure(session: &mut ProjectSession) -> (EmittedZProfile, Vec<f64>, bool, usize) {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(0, &cancel)
        .expect("adaptive3d generation must succeed");
    let annotated = result.op_data.annotated();
    let profile = profile_emitted_motion(&annotated.toolpath, WORLD_STOCK_TOP_MM);

    // The NOMINAL ladder, alongside: the `DepthPass` payload `z_level`s are
    // exactly what `narrate::summarize_z_levels` reports when the spans
    // survive the dressup transforms.
    let mut depth_pass_spans = 0usize;
    let mut nominal: Vec<f64> = Vec::new();
    for span in annotated.spans_of_kind(SpanKind::DepthPass) {
        depth_pass_spans += 1;
        // `SpanPayload` is not `Copy` — match on a reference, as
        // `narrate::summarize_z_levels_from_spans` does.
        if let Some(SpanPayload::DepthPass { z_level, .. }) = &span.payload {
            nominal.push(*z_level);
        }
    }
    nominal.sort_by(|a, b| b.total_cmp(a));
    (profile, nominal, annotated.spans_valid, depth_pass_spans)
}

fn fixture_context() -> String {
    let raw_safe_z = rs_cam_core::compute::stock_config::PostConfig::default().safe_z;
    format!(
        "fixture: stock {STOCK_THICKNESS_MM} mm at origin_z {STOCK_ORIGIN_Z_MM} \
         (world stock top {WORLD_STOCK_TOP_MM:+.3}), flat model surface at \
         {MODEL_SURFACE_Z_MM:+.3}, depth_per_pass {DEPTH_PER_PASS_MM}; \
         post.safe_z {raw_safe_z} floored by effective_safe_z to {:.3} \
         (= world stock top + SAFE_Z_CLEARANCE_MM {SAFE_Z_CLEARANCE_MM})",
        effective_safe_z(raw_safe_z, WORLD_STOCK_TOP_MM)
    )
}

// ── The measurement ─────────────────────────────────────────────────────

/// **The G-AIRLADDER measurement.** Zero cutting moves may end above the
/// world stock top; anything up there is feed-rate motion through empty air.
///
/// A green run says the count is zero. A red run names every level, its move
/// population and the fed path length above the stock top — which is the
/// number the row was opened to obtain.
#[test]
fn wasted_fed_cutting_levels_above_the_world_stock_top() {
    let mut session = build_wanaka_shaped_rough();
    // Read the top back off the session rather than trusting the constant:
    // if `StockConfig::bbox` ever stops being world-frame, this fails loudly
    // instead of measuring against a stale literal.
    let session_top = session.stock_bbox().max.z;
    assert!(
        (session_top - WORLD_STOCK_TOP_MM).abs() < Z_EPS_MM,
        "fixture sanity: session stock bbox top is {session_top:+.3}, expected \
         {WORLD_STOCK_TOP_MM:+.3} — the fixture no longer describes what it says it does"
    );

    let (profile, _nominal, _spans_valid, _spans) = measure(&mut session);
    assert!(
        !profile.levels.is_empty(),
        "fixture sanity: the rough emitted no cutting moves at all, so every \
         verdict below would be vacuous. {}",
        fixture_context()
    );

    let wasted: Vec<&Level> = profile
        .levels
        .iter()
        .filter(|l| l.z > WORLD_STOCK_TOP_MM + Z_EPS_MM)
        .collect();

    // Pinned at ZERO since G-PECKROOT (2026-08-22).
    //
    // The history, because the number moved twice and each move meant a
    // different thing:
    //
    // 1. G-SAFEZ-LOCAL (889b1573) took this fixture from five wasted rungs to
    //    one, and the emitted air above the stock top from ~14,591 mm —
    //    measured off the shipped `wanaka200_2_Setup_2___front.nc`, five rungs
    //    of 231 fed moves each with ZERO lateral cutting — down to ~27 mm.
    // 2. The survivor was structural, not a regression: `emit_peck_plunge`
    //    was rooted at `params.safe_z` and knew nothing about the stock top,
    //    so while `SAFE_Z_CLEARANCE_MM` (5.0) exceeds `depth_per_pass` the
    //    first rung always landed above the stock — here 10.0 - 4.2 = 5.8
    //    against a top of 5.0. This assertion sat at `<= 1` level and
    //    `<= 40 mm` for exactly as long as that was true, and said so.
    // 3. G-PECKROOT roots the ladder at `stock_top_z + ENTRY_CLEARANCE`,
    //    reached by a RAPID, reusing the same guard `dressup::emit_ramp` and
    //    `emit_helix` already apply to the identical problem. Here that is
    //    5.0 + 2.0 = 7.0, and the first fed rung lands at 7.0 - 4.2 = 2.8 —
    //    below the stock top, i.e. in material where a peck belongs.
    //
    // **The two bars now measure different things, and only one is zero.**
    // Measured 2026-08-22 immediately after the fix:
    //
    //   wasted levels: 1 -> 0        fed mm above the plane: ~27 -> 26.282
    //
    // That is not a disappointing result, it is the two quantities coming
    // apart, and reading them as one is how this would get "fixed" again by
    // someone chasing the second number:
    //
    // * **Levels** counts rungs whose whole descent sits above the stock —
    //   pure wasted motion, removing nothing. That is now genuinely **zero**,
    //   and it is the sharp bar. A regression puts a rung back in the air.
    // * **Fed mm above the plane** also counts the *upper portion of a move
    //   that crosses the plane*. The first rung now starts at the guard
    //   (5.0 + 2.0 = 7.0) and ends at 2.8, so 2 mm of it is above the top and
    //   the rest is in material. Summed over this fixture's ~13 entries that
    //   is the 26.282 mm below — i.e. **it is the guard band, once per entry,
    //   and nothing else**.
    //
    // Driving that second number to zero means feeding from exactly the
    // nominal stock top, which deletes the over-thickness allowance
    // `dressup::emit_ramp` and `emit_helix` both keep for the same reason:
    // `stock_top_z` is nominal and real timber is proud of it. That is a
    // machine-safety trade, not a tidy-up, and it has not been made.
    //
    // Also legitimate and not yet seen: a `depth_per_pass` finer than
    // `ENTRY_CLEARANCE` (2 mm) would put a whole rung inside the guard band
    // and push `levels` back to 1. No shipped default does that (wanaka is
    // 4.2). If one ever does, this failing is the correct signal to
    // re-derive the bar, not to widen it.
    const KNOWN_RESIDUAL_LEVELS: usize = 0;
    /// The guard band, once per entry — see the note above. Sized just over
    /// the measured 26.282 mm so ordinary planner jitter in the entry count
    /// does not flap the gate, and far below the ~27 mm the pre-G-PECKROOT
    /// wasted rung cost on top of it.
    const KNOWN_RESIDUAL_FED_MM: f64 = 27.0;
    assert!(
        wasted.len() == KNOWN_RESIDUAL_LEVELS
            && profile.fed_len_above_plane_mm <= KNOWN_RESIDUAL_FED_MM,
        "G-AIRLADDER: {} distinct cutting Z level(s) lie ABOVE the world stock \
         top ({WORLD_STOCK_TOP_MM:+.3} mm). Every move at these levels is \
         commanded at feed rate through empty air.\n      {}\n    \
         fed path length above the world stock top: {:.3} mm\n    \
         total distinct cutting levels: {}; lateral cutting moves: {}\n    \
         {}\n    \
         This ladder is the peck-plunge ENTRY ladder \
         (`adaptive3d/path.rs::emit_peck_plunge`, called with \
         `start_z = params.safe_z`), not the Z-level plan (`path.rs:448`, which \
         starts at `stock_top_z - depth_per_pass` and therefore cannot put a \
         level above the stock). It steps down by depth_per_pass from the \
         stock guard `stock_top_z + ENTRY_CLEARANCE` (G-PECKROOT), reached by \
         a rapid — so a fed level above the stock top means either that guard \
         regressed or `depth_per_pass` is finer than ENTRY_CLEARANCE. Levels \
         reporting 0 lateral moves \
         are pure vertical descents and remove nothing. Expected at most {KNOWN_RESIDUAL_LEVELS} level and {KNOWN_RESIDUAL_FED_MM:.1} mm.",
        wasted.len(),
        render_levels(&wasted),
        profile.fed_len_above_plane_mm,
        profile.levels.len(),
        profile.lateral_cut_moves,
        fixture_context(),
    );
}

/// The other half of the separation: levels between the model surface and the
/// stock top are **not** waste — that band is the stock the rough exists to
/// remove. This test exists so a green run of the measurement above can never
/// be explained by "the rough did nothing", and so nobody re-reads legitimate
/// stock clearing as air.
#[test]
fn stock_clearing_levels_between_model_and_stock_top_are_legitimate() {
    let mut session = build_wanaka_shaped_rough();
    let (profile, _nominal, _spans_valid, _spans) = measure(&mut session);

    let clearing: Vec<&Level> = profile
        .levels
        .iter()
        .filter(|l| l.z <= WORLD_STOCK_TOP_MM + Z_EPS_MM && l.z > MODEL_SURFACE_Z_MM + Z_EPS_MM)
        .collect();
    let at_or_below_model: usize = profile
        .levels
        .iter()
        .filter(|l| l.z <= MODEL_SURFACE_Z_MM + Z_EPS_MM)
        .count();

    assert!(
        !clearing.is_empty() && profile.lateral_cut_moves > 0,
        "fixture sanity: the rough should clear the {:.3} mm of stock standing \
         between the model surface ({MODEL_SURFACE_Z_MM:+.3}) and the stock top \
         ({WORLD_STOCK_TOP_MM:+.3}), but emitted {} level(s) in that band and \
         {} lateral cutting moves in total ({} level(s) at or below the model \
         surface, {} distinct levels overall). Without cutting in that band the \
         air-vs-stock-clearing distinction this file exists to draw is untestable.\n    {}",
        WORLD_STOCK_TOP_MM - MODEL_SURFACE_Z_MM,
        clearing.len(),
        profile.lateral_cut_moves,
        at_or_below_model,
        profile.levels.len(),
        fixture_context(),
    );
}

/// Narration's ladder is the NOMINAL one, and the nominal one is sound.
///
/// `summarize_z_levels` reads `SpanPayload::DepthPass`'s `z_level` — the
/// planner's chosen level, not a measured move Z — whenever the spans survive
/// the dressup transforms. Those levels come from `path.rs:448`, anchored on
/// `stock_top_z`, so none of them can sit above the stock. Pinning that here
/// keeps the two ladders from being confused again: if the emitted
/// measurement above goes red while this stays green, the defect is in
/// emission, not in the plan.
///
/// The population is asserted before the bar, per CLAUDE.md's "a gate handed
/// an empty population passes and looks healthy".
#[test]
fn narration_nominal_ladder_stays_at_or_below_the_world_stock_top() {
    let mut session = build_wanaka_shaped_rough();
    let (_profile, nominal, spans_valid, depth_pass_spans) = measure(&mut session);

    let ladder = nominal
        .iter()
        .map(|z| format!("{z:+.3}"))
        .collect::<Vec<_>>()
        .join(", ");
    let highest = nominal.first().copied().unwrap_or(f64::NEG_INFINITY);

    assert!(
        !nominal.is_empty() && highest <= WORLD_STOCK_TOP_MM + Z_EPS_MM,
        "the nominal ladder narration reads is either absent or above the \
         stock: {} DepthPass span(s), {} carrying a z_level payload, \
         spans_valid = {spans_valid}. Ladder: [{ladder}]. World stock top \
         {WORLD_STOCK_TOP_MM:+.3}.\n    \
         An empty ladder means narration fell back to clustering raw move Z \
         (`narrate::summarize_z_levels_from_moves`) — in that mode its output \
         IS achieved motion and the nominal-vs-achieved caveat does not apply \
         to it.\n    {}",
        depth_pass_spans,
        nominal.len(),
        fixture_context(),
    );
}
