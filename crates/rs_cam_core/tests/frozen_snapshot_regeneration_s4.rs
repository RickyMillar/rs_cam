//! S-4 (G-BYTE) — the frozen-snapshot A/B.
//!
//! # The incident
//!
//! `TECH_DEBT_2_CLOSEOUT.md` §4.2 row **G-BYTE**: on the W10-LV live
//! validation, re-exporting a rest-fed `UnifiedFinish` op at the SAME
//! parameter value it had at baseline produced **135 change-hunks** —
//! roughly 140 `G0` approach heights moved 0.05–0.15 mm, and 12 of 180 118
//! cutting lines were dropped (0.007 %). Cutting Zs were identical wherever
//! present, and the dial itself round-tripped exactly.
//!
//! Two causes fit that signature and the incident could not tell them apart,
//! because **nothing records which simulation event a generation consumed**:
//!
//! 1. **generator nondeterminism** — the same inputs producing different
//!    output; or
//! 2. **snapshot drift** — the two generations consumed *different*
//!    machined-stock snapshots, because a simulation ran between them. The
//!    comparison, not the generator, would then be at fault (a §6.1 rule-6
//!    violation).
//!
//! # What this harness does
//!
//! It removes the ambiguity by controlling the one thing the incident left
//! uncontrolled: **which snapshot each generation consumed**.
//!
//! | arm | between the two generations of the rest op | measured 2026-08-12 |
//! |---|---|---|
//! | **A — frozen** | nothing at all | **byte-identical** (187 557 B / 7 208 lines, 0 hunks) |
//! | **B1 — re-sim, same cell** | one `run_simulation` at 0.25 mm again | snapshot reproduced exactly; **byte-identical** |
//! | **B2 — re-sim, coarse cell** | one `run_simulation` at 0.40 mm | snapshot moved; 3 726 lines differ (5 `diff` hunks) |
//! | **B3 — re-sim, near cell** | one `run_simulation` at 0.30 mm | snapshot moved; 161 lines differ (6 `diff` hunks) |
//! | **B4 — no territory clip** | frozen, then 0.30 mm | frozen: byte-identical. Drifted: **also** byte-identical — see below |
//!
//! Arm A is the verdict. The B arms exist so that "identical" is not a
//! vacuous result: a harness that cannot make the bytes move has proven
//! something about itself, not about the generator.
//!
//! # The verdict, and what it does and does not settle
//!
//! **Settled:** the generator is deterministic. Against one frozen snapshot,
//! at identical parameters, two generations agree to the byte on all three
//! renderings — on two different dial configurations (arms A and B4-frozen).
//! G-BYTE is therefore **not** generator nondeterminism, and the fix is
//! provenance: `ToolpathStats::stock_snapshot`.
//!
//! **Also settled, and worth knowing:** re-simulating is not by itself
//! drift. Arm B1's second simulation allocated a fresh `Arc` holding
//! bit-identical material, and the output did not move. Drift requires the
//! simulation to actually *differ* — in this fixture, in its cell.
//!
//! **NOT settled — the incident's specific class.** The incident's dominant
//! signature was ~140 `G0` approach heights moving 0.05–0.15 mm with cutting
//! Zs identical. `dressup::optimize_entry_descents_with_provenance` computes
//! each split rapid's Z as
//! `stock.max_conservative_top_z_in_disc(x, y, tool_radius) + PLUNGE_CLEARANCE_MM`
//! off the consumed snapshot, which is the obvious mechanism — but arm B4,
//! written to isolate exactly that channel, could **not** reproduce it here:
//! this fixture emits only 7 `G0` lines and every split saturates at the raw
//! stock top (`Z8.000 = 6.0 + 2.0`), because the accessor is a *sliver-safe
//! upper bound* and refining the cell cannot lower it. What arms B2/B3 move
//! instead is the rest TERRITORY. See `arm_b4_…`'s own doc for the blocker.
//!
//! # Comparison method
//!
//! Byte-level, on three independent renderings of the same generation:
//!
//! * the move list (`FNV-1a` over its `Debug`, which round-trips every `f64`
//!   bit — `common::fingerprint`);
//! * the whole `AnnotatedToolpath` (moves **plus** spans, planner
//!   engagement, rest grid and rest regions);
//! * the emitted **G-code**, exported with `sim_trace: None` and an
//!   all-accepting policy so the text is a pure function of the toolpaths
//!   and cannot move because a load verdict moved.
//!
//! G-code for every arm is written to `target/gbyte_s4/` so a failure can be
//! diffed with the same tool the incident was diffed with. The in-test line
//! comparison is **positional, not an LCS**: it is exact when the line counts
//! match (which is the arm-A case that carries the verdict) and it suppresses
//! its own classification when they do not, rather than reporting a
//! misalignment as a finding.
//!
//! # This file is also the S-4 sentry for the provenance fix
//!
//! Every arm asserts `ToolpathStats::stock_snapshot` against an independent
//! witness computed from `prior_stocks` before generation — so the stamp is
//! checked against something other than itself. Same snapshot ⇒ same stamp
//! (arms A, B1, B4-frozen); different snapshot ⇒ different stamp (B2, B3,
//! B4-drifted); no machined stock ⇒ `None`
//! (`a_fresh_stock_generation_records_no_snapshot_stamp`).
//!
//! B4-drifted is the sharpest case: the output is byte-identical while the
//! stamp MOVED. That is the correct direction for a provenance channel — it
//! never claims a comparability that does not exist.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::atomic::AtomicBool;

use common::fingerprint::fnv1a_debug;
use common::meshes::height_field;
use common::session::{generate, mesh_model, pinned_heights, stock_over, toolpath_config};
use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{ClaimsReference, UnifiedFinishConfig};
use rs_cam_core::gcode::ToolLoadExportPolicy;
use rs_cam_core::session::{ProjectSession, SimulationOptions};

/// Half-extent (mm) of the fixture surface.
const HALF: f64 = 20.0;

/// Stock height (mm).
const STOCK_Z: f64 = 6.0;

/// Peak-to-trough of the fixture surface (mm).
const RELIEF: f64 = 2.0;

/// The rest pass's raster stepover (mm).
const REST_STEPOVER_MM: f64 = 0.5;

/// The finish pass's raster stepover (mm) — 3x coarser, so it leaves
/// ~0.19 mm cusp ridges and the rest pass has real material in front of it.
/// A rest pass with nothing to do would emit a degenerate path whose
/// byte-stability proves nothing.
const FINISH_STEPOVER_MM: f64 = 1.5;

/// The rest-territory gate (mm).
const REST_GATE_MM: f64 = 0.05;

/// The frozen snapshot's simulation cell (mm). Well under the Ø3 ball's tip
/// radius, per `feedback_rest_measurement_prerequisites`.
const SIM_CELL_MM: f64 = 0.25;

/// Arm B2's SECOND simulation cell (mm). Different grid, therefore a
/// different sampling of the same physical stock. Chosen coarse enough that
/// the rest TERRITORY moves too — the maximal perturbation, proving the
/// harness can move the bytes at all.
const SIM_CELL_ALT_MM: f64 = 0.4;

/// Arm B3's SECOND simulation cell (mm). A *small* grid change, modelling
/// the incident's own suspected delta (a fixpoint round's sim cell vs a
/// later explicit sim's). The prediction: a small snapshot delta produces
/// the incident's shape — `G0` approach heights moving by fractions of a
/// millimetre — rather than B2's wholesale territory change.
const SIM_CELL_NEAR_MM: f64 = 0.3;

/// The same smooth double bump the A/M6 cascade and A4 zero-removal sentries
/// use, in the same frame (occupying the TOP of the stock, so the finish
/// pass leaves a real machined surface rather than an untouched block).
fn bumpy_surface() -> rs_cam_core::mesh::TriangleMesh {
    height_field(HALF, 0.5, |x, y| {
        let sx = (x / HALF * std::f64::consts::PI).cos();
        let sy = (y / HALF * std::f64::consts::PI).cos();
        STOCK_Z - RELIEF * 0.5 * (1.0 - sx * sy * 0.9)
    })
}

fn unified_cfg(raster_stepover: f64, rest_pass: bool) -> UnifiedFinishConfig {
    UnifiedFinishConfig {
        scallop_height: 0.02,
        overlap_mm: 0.5,
        tolerance: 0.05,
        raster_stepover,
        z_step: 0.5,
        sampling: 0.4,
        feed_rate: 2000.0,
        plunge_rate: 500.0,
        pencil_claims: rest_pass,
        territory_clip: rest_pass,
        claims_reference: ClaimsReference::Auto,
        min_rest_depth_mm: REST_GATE_MM,
        ..UnifiedFinishConfig::default()
    }
}

/// Op 0: an all-over finish. Op 1: a rest-fed finish reading the stock op 0
/// left (`StockSource::FromRemainingStock`) — the incident's shape.
///
/// `rest_claims` selects which snapshot consumers op 1 exercises:
///
/// * `true` — the full rest pass: crease/pencil claims + territory clip
///   read the snapshot, **plus** the air-cut filter and the entry-descent
///   optimizer. The maximal surface.
/// * `false` — territory clip off, so the ONLY snapshot consumers left are
///   the air-cut filter and `optimize_entry_descents`. This isolates the
///   incident's dominant class: `G0` approach heights, whose Z comes
///   straight from `max_conservative_top_z_in_disc` on the snapshot.
fn cascade_session_with(rest_claims: bool) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_over(HALF, STOCK_Z));
    let tool_idx = session.add_tool(ball_tool_config(3.0));
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session.add_model(mesh_model(bumpy_surface(), "bumps"));
    let heights = pinned_heights(STOCK_Z, STOCK_Z - RELIEF);

    let mut finish = toolpath_config(
        "Finish (all-over)",
        OperationConfig::UnifiedFinish(unified_cfg(FINISH_STEPOVER_MM, false)),
        tool_id,
        model_id,
    );
    finish.heights = heights.clone();
    session.add_toolpath(0, finish).expect("add finish op");

    let mut rest = toolpath_config(
        "Rest (same tool)",
        OperationConfig::UnifiedFinish(unified_cfg(REST_STEPOVER_MM, rest_claims)),
        tool_id,
        model_id,
    );
    rest.heights = heights;
    rest.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, rest).expect("add rest op");
    session
}

/// The default cascade: a full rest pass (claims + territory clip).
fn cascade_session() -> ProjectSession {
    cascade_session_with(true)
}

fn simulate(session: &mut ProjectSession, cell_mm: f64) {
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: cell_mm,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("cascade simulation");
}

/// Everything one generation of the rest op produced, in three independent
/// byte-level renderings.
struct Capture {
    move_count: usize,
    move_hash: u64,
    annotated_hash: u64,
    gcode: String,
    /// Identity of the machined-stock snapshot this generation consumed, as
    /// the harness can see it from outside: the dexel grid the snapshot was
    /// sampled on, plus a content digest over a coarse XY lattice of its
    /// top-Z. Computed BEFORE generation, from the session's `prior_stocks`
    /// directly — an independent witness, so the production stamp is checked
    /// against something other than itself.
    snapshot_id: SnapshotId,
    /// The production provenance stamp this generation recorded
    /// (`ToolpathStats::stock_snapshot`) — the S-4 deliverable.
    stamp: Option<rs_cam_core::compute::config::StockSnapshotStamp>,
}

/// A test-side stand-in for the provenance the incident lacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SnapshotId {
    /// FNV-1a over the snapshot's grid parameters + a lattice of top-Z
    /// samples. Equal digests mean the two generations saw the same surface
    /// sampled the same way.
    digest: u64,
}

/// Sample the snapshot the given toolpath index would be seeded with.
fn snapshot_id(session: &ProjectSession, index: usize) -> SnapshotId {
    let tc = session
        .toolpath_configs()
        .get(index)
        .expect("toolpath config");
    let stock = session
        .simulation_result()
        .and_then(|sim| sim.prior_stocks.get(&tc.id).cloned())
        .expect("a FromRemainingStock op must have a prior-stock snapshot");

    // A coarse lattice is enough to separate two differently-resolved
    // samplings of the same surface — and it is what the incident's own
    // signature is made of, since `optimize_entry_descents` queries exactly
    // this surface, through exactly this accessor.
    const N: usize = 64;
    let b = stock.stock_bbox;
    let mut acc: Vec<String> = Vec::with_capacity(N * N + 1);
    acc.push(format!(
        "{b:?}|{}x{}@{}",
        stock.z_grid.rows, stock.z_grid.cols, stock.z_grid.cell_size,
    ));
    for iy in 0..N {
        for ix in 0..N {
            let fx = (ix as f64 + 0.5) / N as f64;
            let fy = (iy as f64 + 0.5) / N as f64;
            let x = b.min.x + fx * (b.max.x - b.min.x);
            let y = b.min.y + fy * (b.max.y - b.min.y);
            acc.push(format!(
                "{:?}",
                stock.max_conservative_top_z_in_disc(x, y, 1.5)
            ));
        }
    }
    SnapshotId {
        digest: fnv1a_debug(&acc),
    }
}

/// Emit G-code that is a pure function of the generated toolpaths: no sim
/// trace, and a policy that accepts every load verdict, so nothing about the
/// text can move because a *gate* moved.
fn gcode_of(session: &ProjectSession) -> String {
    rs_cam_core::gcode::export_gcode_checked(
        session,
        None,
        ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        },
    )
    .expect("export must succeed under an all-accepting policy")
}

fn capture(session: &mut ProjectSession, index: usize) -> Capture {
    let snapshot_id = snapshot_id(session, index);
    generate(session, index);
    let result = session.get_result(index).expect("rest result");
    let annotated = result.annotated();
    let stamp = result.stats.stock_snapshot;
    Capture {
        move_count: annotated.toolpath.moves.len(),
        move_hash: fnv1a_debug(&annotated.toolpath.moves),
        annotated_hash: fnv1a_debug(annotated),
        gcode: gcode_of(session),
        snapshot_id,
        stamp,
    }
}

/// Line-level difference report between two G-code renderings, in the
/// vocabulary the incident used.
struct GcodeDiff {
    bytes_a: usize,
    bytes_b: usize,
    lines_a: usize,
    lines_b: usize,
    /// Positionally differing lines (only meaningful when the line counts
    /// match; otherwise reported alongside the count mismatch).
    differing_lines: usize,
    /// Contiguous runs of differing lines — the incident's "hunks".
    hunks: usize,
    /// Of `differing_lines`, how many are a `G0` on BOTH sides at the same
    /// XY — i.e. an approach height that moved. The incident's dominant
    /// class (~140 of its hunks).
    ///
    /// **Only meaningful when `lines_a == lines_b`.** The comparison is
    /// positional, not an LCS: a single inserted or deleted line misaligns
    /// everything after it and the classification becomes noise. The
    /// reporter therefore suppresses this whole class line when the line
    /// counts differ, and the dumped `.nc` files under `target/gbyte_s4/`
    /// are there to be run through real `diff` in that case — the same tool
    /// the incident's 135 hunks were counted with.
    g0_height_moves: usize,
    /// Largest `|ΔZ|` (mm) over `g0_height_moves`. The incident reported
    /// 0.05–0.15 mm.
    max_g0_dz_mm: f64,
    /// Of `differing_lines`, how many are a `G1` on both sides. The incident
    /// reported cutting Zs IDENTICAL where present, so this class being
    /// small (or zero) is what makes a diff "the incident's shape".
    g1_moves: usize,
    /// Up to eight `(line number, a, b)` samples.
    samples: Vec<(usize, String, String)>,
}

/// Pull one axis word out of a G-code line (`X-1.500`, `Z5.556`).
fn word(line: &str, axis: char) -> Option<f64> {
    line.split_whitespace()
        .find_map(|t| t.strip_prefix(axis))
        .and_then(|v| v.parse::<f64>().ok())
}

fn diff_gcode(a: &str, b: &str) -> GcodeDiff {
    let la: Vec<&str> = a.lines().collect();
    let lb: Vec<&str> = b.lines().collect();
    let n = la.len().min(lb.len());
    let mut differing = 0usize;
    let mut hunks = 0usize;
    let mut in_hunk = false;
    let mut samples = Vec::new();
    let mut g0_height_moves = 0usize;
    let mut max_g0_dz_mm = 0.0f64;
    let mut g1_moves = 0usize;
    for i in 0..n {
        if la[i] == lb[i] {
            in_hunk = false;
        } else {
            differing += 1;
            if !in_hunk {
                hunks += 1;
                in_hunk = true;
            }
            let (x, y) = (la[i].starts_with("G0 "), lb[i].starts_with("G0 "));
            let same_xy =
                word(la[i], 'X') == word(lb[i], 'X') && word(la[i], 'Y') == word(lb[i], 'Y');
            if x && y && same_xy {
                g0_height_moves += 1;
                if let (Some(za), Some(zb)) = (word(la[i], 'Z'), word(lb[i], 'Z')) {
                    max_g0_dz_mm = max_g0_dz_mm.max((za - zb).abs());
                }
            }
            if la[i].starts_with("G1 ") && lb[i].starts_with("G1 ") {
                g1_moves += 1;
            }
            if samples.len() < 8 {
                samples.push((i + 1, la[i].to_owned(), lb[i].to_owned()));
            }
        }
    }
    GcodeDiff {
        bytes_a: a.len(),
        bytes_b: b.len(),
        lines_a: la.len(),
        lines_b: lb.len(),
        differing_lines: differing,
        hunks,
        g0_height_moves,
        max_g0_dz_mm,
        g1_moves,
        samples,
    }
}

fn dump(name: &str, gcode: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gbyte_s4");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{name}.nc")), gcode);
}

fn report(arm: &str, first: &Capture, second: &Capture) -> GcodeDiff {
    let d = diff_gcode(&first.gcode, &second.gcode);
    eprintln!(
        "[S-4 {arm}] snapshot {:016x} -> {:016x} ({})",
        first.snapshot_id.digest,
        second.snapshot_id.digest,
        if first.snapshot_id == second.snapshot_id {
            "SAME"
        } else {
            "MOVED"
        },
    );
    // The S-4 deliverable, on every arm: the production stamp must agree
    // with the independent witness above. This is the whole point — the
    // incident could not see this line.
    let show = |s: Option<rs_cam_core::compute::config::StockSnapshotStamp>| match s {
        Some(s) => format!(
            "{:016x} @{:.3}mm {}x{}",
            s.digest,
            s.cell_size_mm(),
            s.rows,
            s.cols
        ),
        None => "NONE (no machined stock consumed)".to_owned(),
    };
    eprintln!(
        "[S-4 {arm}] stamp    {} -> {} ({})",
        show(first.stamp),
        show(second.stamp),
        if first.stamp == second.stamp {
            "SAME"
        } else {
            "MOVED"
        },
    );
    assert!(
        first.stamp.is_some() && second.stamp.is_some(),
        "a FromRemainingStock generation must record a stamp",
    );
    assert_eq!(
        first.stamp == second.stamp,
        first.snapshot_id == second.snapshot_id,
        "the production stamp must agree with the independent witness: \
         stamps {:?}/{:?} vs witness {:?}/{:?}",
        first.stamp,
        second.stamp,
        first.snapshot_id,
        second.snapshot_id,
    );
    eprintln!(
        "[S-4 {arm}] moves {} -> {} | move hash {:016x} -> {:016x} | annotated hash {:016x} -> {:016x}",
        first.move_count,
        second.move_count,
        first.move_hash,
        second.move_hash,
        first.annotated_hash,
        second.annotated_hash,
    );
    eprintln!(
        "[S-4 {arm}] gcode {} B / {} lines -> {} B / {} lines | differing lines {} in {} hunks",
        d.bytes_a, d.lines_a, d.bytes_b, d.lines_b, d.differing_lines, d.hunks,
    );
    if d.lines_a == d.lines_b {
        eprintln!(
            "[S-4 {arm}] class: G0 approach-height moves {} (max |dZ| {:.4} mm) | G1-on-both {}",
            d.g0_height_moves, d.max_g0_dz_mm, d.g1_moves,
        );
    } else {
        eprintln!(
            "[S-4 {arm}] class: SUPPRESSED — line counts differ ({} vs {}), so a \
             positional compare cannot classify. Run `diff` on target/gbyte_s4/{arm}_gen*.nc",
            d.lines_a, d.lines_b,
        );
    }
    for (line, a, b) in &d.samples {
        eprintln!("[S-4 {arm}]   L{line}: {a}  ||  {b}");
    }
    dump(&format!("{arm}_gen1"), &first.gcode);
    dump(&format!("{arm}_gen2"), &second.gcode);
    d
}

/// **ARM A — THE VERDICT.** One machined-stock snapshot, frozen; the same op
/// generated twice against it with identical parameters. Nothing whatsoever
/// happens between the two generations.
///
/// If this fails, G-BYTE is generator nondeterminism and blocks every
/// before/after fixture in the programme. If it passes, the incident's 135
/// hunks were snapshot drift and the fix is provenance, not determinism.
#[test]
fn arm_a_two_generations_against_one_frozen_snapshot_are_byte_identical() {
    let mut session = cascade_session();
    generate(&mut session, 0);
    simulate(&mut session, SIM_CELL_MM);

    let first = capture(&mut session, 1);
    // NOTHING between the two generations: no simulation, no mutation, no
    // parameter touch. `generate_toolpath` writes only `self.results`, so
    // `simulation.prior_stocks` is bit-for-bit the same `Arc`.
    let second = capture(&mut session, 1);

    // Non-vacuity: a degenerate or empty pass would make byte-equality
    // trivially true and prove nothing.
    assert!(
        first.move_count > 500,
        "the rest pass must emit a real toolpath — got {} moves",
        first.move_count,
    );
    assert!(
        first.gcode.lines().count() > 500,
        "the export must be a real program — got {} lines",
        first.gcode.lines().count(),
    );

    let d = report("armA_frozen", &first, &second);

    assert_eq!(
        first.snapshot_id, second.snapshot_id,
        "the arm's premise: both generations consumed the SAME snapshot",
    );
    assert_eq!(
        (first.move_count, first.move_hash),
        (second.move_count, second.move_hash),
        "same params + same snapshot must emit the same move list",
    );
    assert_eq!(
        first.annotated_hash, second.annotated_hash,
        "spans, planner engagement, rest grid and rest regions must also match",
    );
    assert_eq!(
        (d.differing_lines, d.hunks, d.lines_a == d.lines_b),
        (0, 0, true),
        "the emitted G-code must be byte-identical",
    );
    assert_eq!(first.gcode, second.gcode, "byte-identical G-code");

    // The S-4 provenance fix, half one: same snapshot => same stamp.
    assert_eq!(
        first.stamp, second.stamp,
        "two generations against one frozen snapshot must carry the same stamp",
    );
    let stamp = first.stamp.expect("a rest-fed generation records a stamp");
    assert!(
        (stamp.cell_size_mm() - SIM_CELL_MM).abs() < 1e-12,
        "the stamp must carry the cell the snapshot was sampled on: {} vs {SIM_CELL_MM}",
        stamp.cell_size_mm(),
    );
    assert!(
        stamp.rows > 0 && stamp.cols > 0,
        "a stamp with no grid behind it is not an identity",
    );
}

/// The other half of the S-4 provenance fix: a `StockSource::Fresh`
/// generation consumed no machined stock, and must say so with `None` rather
/// than a stamp of the raw block.
///
/// This is what makes the channel readable: `Some` means "this output
/// depended on a prior op's simulated stock, and here is which one".
#[test]
fn a_fresh_stock_generation_records_no_snapshot_stamp() {
    let mut session = cascade_session();
    generate(&mut session, 0);
    let stats = &session.get_result(0).expect("finish result").stats;
    assert_eq!(
        stats.stock_snapshot, None,
        "op 0 is StockSource::Fresh with no prior simulation — no machined \
         stock was consumed, so the channel must read None",
    );
}

/// **ARM B1 — the control.** A simulation runs between the two generations,
/// at the SAME cell. Whatever this shows is a statement about the snapshot,
/// not about the generator: arm A already fixed the generator's behaviour.
#[test]
fn arm_b1_resimulating_at_the_same_cell_between_generations() {
    let mut session = cascade_session();
    generate(&mut session, 0);
    simulate(&mut session, SIM_CELL_MM);

    let first = capture(&mut session, 1);
    simulate(&mut session, SIM_CELL_MM);
    let second = capture(&mut session, 1);

    let d = report("armB1_resim_same_cell", &first, &second);
    assert!(first.move_count > 500, "arm B1 must emit a real toolpath");
    let _ = d;

    // The stamp's design claim, stated where it is exercised: the second
    // simulation allocated a NEW `Arc` holding the SAME material, and the
    // stamp must read that as the same snapshot. A pointer or an
    // incrementing sim-event counter would report a difference here and
    // raise a false alarm on every honest re-simulation.
    assert_eq!(
        first.stamp, second.stamp,
        "the stamp is content-derived: a fresh Arc with identical material \
         is the same snapshot",
    );
}

/// **ARM B2 — the mechanism probe.** A simulation runs between the two
/// generations at a DIFFERENT cell, so the same physical stock is handed to
/// the second generation sampled on a different grid. This is the harness's
/// proof that it *can* move the bytes — and the prediction is that it moves
/// them in the incident's shape: `G0` approach heights, not cutting Zs.
#[test]
fn arm_b2_resimulating_at_a_different_cell_moves_the_bytes() {
    let mut session = cascade_session();
    generate(&mut session, 0);
    simulate(&mut session, SIM_CELL_MM);

    let first = capture(&mut session, 1);
    simulate(&mut session, SIM_CELL_ALT_MM);
    let second = capture(&mut session, 1);

    let d = report("armB2_resim_alt_cell", &first, &second);
    assert!(first.move_count > 500, "arm B2 must emit a real toolpath");
    assert_ne!(
        first.snapshot_id, second.snapshot_id,
        "the arm's premise: the second generation consumed a DIFFERENT snapshot",
    );
    assert!(
        d.differing_lines > 0 || d.lines_a != d.lines_b || first.move_hash != second.move_hash,
        "a differently-resolved snapshot must be able to move the output — \
         otherwise arm A's byte-equality is a property of the harness, not of \
         the generator",
    );

    // The S-4 provenance fix, half two: a DIFFERENT snapshot must produce a
    // DIFFERENT stamp. Without this the channel could be a constant and
    // still pass arm A.
    assert_ne!(
        first.stamp, second.stamp,
        "two generations against different snapshots must carry different stamps",
    );
    let (a, b) = (first.stamp.expect("stamp"), second.stamp.expect("stamp"));
    assert!(
        (a.cell_size_mm() - SIM_CELL_MM).abs() < 1e-12
            && (b.cell_size_mm() - SIM_CELL_ALT_MM).abs() < 1e-12,
        "the stamp must name the cell that changed: {} then {}",
        a.cell_size_mm(),
        b.cell_size_mm(),
    );
}

/// **ARM B3 — the incident's shape.** B2 proves snapshot drift *can* move
/// the bytes, but it moves them wholesale: at 0.25 → 0.40 mm the rest
/// TERRITORY itself changes and 3 726 lines differ. The incident was far
/// milder — 135 hunks, cutting Zs identical, `G0` approach heights moved
/// 0.05–0.15 mm. This arm applies a *small* snapshot delta (0.25 → 0.30 mm)
/// and reports which class the resulting hunks fall in.
///
/// A measurement, not a gate: the arm exists to characterise, and both a
/// small-diff and a large-diff answer are recorded honestly.
#[test]
fn arm_b3_a_small_snapshot_delta_and_the_class_of_hunks_it_produces() {
    let mut session = cascade_session();
    generate(&mut session, 0);
    simulate(&mut session, SIM_CELL_MM);

    let first = capture(&mut session, 1);
    simulate(&mut session, SIM_CELL_NEAR_MM);
    let second = capture(&mut session, 1);

    let d = report("armB3_resim_near_cell", &first, &second);
    assert!(first.move_count > 500, "arm B3 must emit a real toolpath");
    assert_ne!(
        first.snapshot_id, second.snapshot_id,
        "the arm's premise: the second generation consumed a DIFFERENT snapshot",
    );
    let _ = d;
}

/// **ARM B4 — the entry-descent channel alone, and what it does NOT show.**
///
/// This arm was written to isolate the incident's dominant class. With the
/// territory clip off, the only remaining snapshot consumers are the air-cut
/// filter and `optimize_entry_descents`, whose split-rapid Z is
/// `max_conservative_top_z_in_disc(...) + PLUNGE_CLEARANCE_MM`.
///
/// **It did not reproduce that class, and the harness says so rather than
/// implying it.** Measured 2026-08-12: the drifted pair is byte-identical
/// (180 147 B, 6 932 lines, 0 hunks) even though the snapshot digest moved.
/// The reason is in the fixture, not the mechanism — this toolpath emits
/// only **7** `G0` lines and every split lands at `Z8.000`, i.e.
/// `6.0 (raw stock top) + 2.0`. `max_conservative_top_z_in_disc` is a
/// *sliver-safe upper bound*: over a 1.5 mm-radius disc on this fixture it
/// saturates at the unmachined stock top, so refining or coarsening the cell
/// cannot move it. A fixture whose entry ceilings sit below the raw stock
/// top is needed to exercise the class, and building one is out of S-4's
/// scope.
///
/// What the arm DOES establish is the half it can: **byte-stability under a
/// frozen snapshot on a SECOND configuration**, so arm A's verdict is not a
/// property of one dial setting.
#[test]
fn arm_b4_entry_descent_channel_alone_does_not_move_on_this_fixture() {
    // First: the frozen A/B on THIS configuration.
    let mut frozen = cascade_session_with(false);
    generate(&mut frozen, 0);
    simulate(&mut frozen, SIM_CELL_MM);
    let f1 = capture(&mut frozen, 1);
    let f2 = capture(&mut frozen, 1);
    let fd = report("armB4_frozen_noclip", &f1, &f2);
    assert_eq!(
        f1.gcode, f2.gcode,
        "byte-identical under a frozen snapshot on this configuration too",
    );
    assert_eq!((fd.differing_lines, fd.hunks), (0, 0));

    // Then: the same op, with a small snapshot delta between generations.
    let mut drifted = cascade_session_with(false);
    generate(&mut drifted, 0);
    simulate(&mut drifted, SIM_CELL_MM);
    let d1 = capture(&mut drifted, 1);
    simulate(&mut drifted, SIM_CELL_NEAR_MM);
    let d2 = capture(&mut drifted, 1);
    let dd = report("armB4_drifted_noclip", &d1, &d2);

    assert_ne!(
        d1.snapshot_id, d2.snapshot_id,
        "the arm's premise: a different snapshot",
    );
    assert!(d1.move_count > 500, "arm B4 must emit a real toolpath");
    let _ = dd;
}
