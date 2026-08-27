//! **Evidence run (Phase S1)** — does any RAPID in the two shipped wanaka
//! programs descend into material that existed when it executed?
//!
//! Spec: `planning/rapid_safety_2026-08-28/S1_SPEC.md`. Recon (every frame,
//! grammar, API and tool constant below is cited to it):
//! `planning/rapid_safety_2026-08-28/S1_RECON.md`. Programme: `PLAN.md`
//! Phase S.
//!
//! # Why this instrument exists at all
//!
//! `rapid_collision_count` cannot answer the question — S-b is precisely that
//! its producer (`collision.rs:450-543`) is a **zero-radius point probe** that
//! takes no cutter argument, i.e. it shares the emitter's blind spot. Asking
//! it whether the emitter is blind is circular. So this replays **emitted
//! motion** (the repo's own standing rule: measure the G-code, not the plan)
//! against a dexel of the same job and adjudicates with the profile-aware
//! primitive `TriDexelStock::max_clearance_tip_z_for_profile`.
//!
//! # Two tiers, and only one of them may say STRIKE
//!
//! * **Coarse (0.3 mm, whole board)** — full sequential replay of every
//!   cutting move in program order. Its half-diagonal conservatism is
//!   ±0.21 mm, so a sub-cell "strike" here is discretisation, not a strike.
//!   The coarse tier may only ever bin a rapid into *skip* or *flag*.
//! * **Fine (0.05 mm, windowed)** — for each flagged rapid, a fresh stock over
//!   the segment's own neighbourhood with **only the prior** cutting moves
//!   re-stamped, then two probes: at the envelope radius, and at a radius
//!   shaved by 1.5 cell diagonals. Envelope-negative + shaved-clear is a
//!   **kerf graze** (a peck or slot re-entry riding its own kerf — radial
//!   clearance is exactly zero by construction and the un-cut rim reads as a
//!   wall). Envelope-negative that persists at the shaved radius is a real
//!   candidate.
//!
//! Never fix a false positive by globally shrinking the probe radius: that
//! deletes the sub-tool-radius sliver sensitivity the instrument exists for.
//!
//! # Tripwires, not eyeballs
//!
//! A wrong Z anchor or a wrong export frame reads as all-air or all-buried,
//! and both look plausible in a report. Three asserts run inside the replay
//! (S1_SPEC decision 2): op 1's pin drill must land on the pin's transformed
//! XY; every op must remove material; every cutting-move endpoint must lie
//! inside the transformed stock box.
//!
//! # Running it
//!
//! ```text
//! cargo test -p rs_cam_core --release --test rapid_replay_shipped_gcode_s1 \
//!     -- --ignored --nocapture
//! ```
//!
//! `--release` is not optional in practice: the coarse pass stamps ~460k
//! segments for setup 2 and the fine tier rebuilds a windowed grid per
//! adjudication. `#[ignore]` carries its usual meaning here — evidence run,
//! invoked explicitly, never by a gate. SKIPS rather than fails when the
//! shipped `.nc` files are absent.
//!
//! `clippy::print_stderr` is opted into at file scope below because this is a
//! **reporting instrument**: its output IS its product, exactly as
//! `power_ceiling_parity_f2.rs` does. `print_stdout` stays denied.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::Path;
use std::time::Instant;

use rs_cam_core::arc_util::linearize_arc_into;
use rs_cam_core::dexel::{DexelGrid, ray_material_length};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill, VBitEndmill};

// ══════════════════════════════════════════════════════════════════════
// SECTION 0 — inputs and dials
// ══════════════════════════════════════════════════════════════════════

/// Shipped programs, in the tree since the 2026-08-19 air run.
const SETUP1_NC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../planning/airrun_2026-08-19/wanaka200_1_Setup_1.nc"
);
const SETUP2_NC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../planning/airrun_2026-08-19/wanaka200_2_Setup_2___front.nc"
);
/// The project the two programs were posted from. Never read at runtime — the
/// tool constants below were copied out of it by hand — but it is the
/// provenance of those constants, so its absence means the fixture set is not
/// the one this instrument was written against.
const WANAKA_TOML: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../planning/airrun_2026-08-19/wanaka200.toml"
);

/// Coarse tier cell. Whole-board replay; flags only (S1_SPEC decision 3).
const COARSE_CELL_MM: f64 = 0.3;
/// Fine tier cell. Windowed; the only tier allowed to say STRIKE. Sized to
/// resolve the sliver class PLAN.md §S1 names (sub-tool-radius, ≲ 0.1 mm).
const FINE_CELL_MM: f64 = 0.05;

/// A rapid is flagged for fine adjudication when its coarse margin falls
/// below this. Also the early-out bar: a rapid whose lowest tip Z clears the
/// analytic stock top by ≥ this cannot have a margin below it (required
/// clearance ≤ stock top, because `conservative_top ≤ bbox.max.z` and
/// `height_at_radius ≥ 0`), so the skip is exact, not heuristic.
const FLAG_MM: f64 = 1.0;
/// Fine-tier NEAR-MISS band: cleared the envelope, but by less than this.
const NEAR_MISS_MM: f64 = 0.5;

/// Fine window = rapid segment bbox ⊕ (tool envelope radius + this), in XY,
/// intersected with the stock box (S1_SPEC decision 3).
const WINDOW_PAD_MM: f64 = 2.0;
/// Kerf-graze probe: the second radius is the envelope shaved by this many
/// fine-cell diagonals. S1_SPEC decision 1 writes "− cell_diagonal"; 1.5 is
/// the orchestrator's tightening — see "Instrument notes" in the spec.
const SHAVE_CELL_DIAGONALS: f64 = 1.5;
/// Floor for the shaved radius, so a tiny tool can never produce a negative
/// or degenerate probe disc.
const MIN_SHAVED_RADIUS_MM: f64 = 0.05;

/// Fine-tier budget. Flagged rapids are sorted worst-coarse-margin-first, so
/// the cap keeps the strongest candidates; the number dropped and the best
/// dropped margin are both reported (no silent caps).
const MAX_FINE_ADJUDICATIONS: usize = 400;
/// Refuse to build a fine window bigger than this many cells. `DexelGrid`
/// silently coarsens past 16 M (`dexel.rs:318`), and a silently-coarsened
/// fine tier would adjudicate at the wrong conservatism.
const MAX_FINE_WINDOW_CELLS: usize = 8_000_000;

/// Upper bound on along-segment samples for one rapid. Effectively inert at
/// the step sizes below (a 240 mm traverse at 0.025 mm is 9,600); reported if
/// it ever binds, because binding it would silently coarsen the sweep.
const MAX_RAPID_SAMPLES: usize = 20_000;
/// Below this XY displacement a rapid is a pure vertical drop: one disc query
/// at the lowest tip Z, not a swept corridor. The census in S1_SPEC found
/// **zero** G0 blocks combining XY and Z motion in either file, so every
/// descent takes this branch.
const XY_STATIONARY_MM: f64 = 1e-9;

/// Tripwire (c): cutting-move endpoints must sit inside the stock box
/// inflated by the tool envelope radius plus this.
const ENDPOINT_TOLERANCE_MM: f64 = 1.0;
/// Tripwire (a): the pin-drill XY match tolerance.
const PIN_DRILL_XY_TOL_MM: f64 = 1e-3;

// ══════════════════════════════════════════════════════════════════════
// SECTION 1 — tools (RECON Q5; values copied from wanaka200.toml)
// ══════════════════════════════════════════════════════════════════════

struct ToolEntry {
    /// The `[Tn]` tag the LOAD / TOOL CHANGE comment carries.
    tag: &'static str,
    label: &'static str,
    cutter: Box<dyn MillingCutter>,
    lut: RadialProfileLUT,
    /// `envelope_radius_mm()` — the conservative radius for stamping and for
    /// the primary clearance probe (RECON Q3: a smaller radius under-scans
    /// off-axis material).
    envelope_r: f64,
}

fn tool_entry(tag: &'static str, label: &'static str, cutter: Box<dyn MillingCutter>) -> ToolEntry {
    // LUT_SAMPLES = 4096, not 256: small-tip tapers need it (RECON Q4).
    let lut = RadialProfileLUT::from_cutter(&*cutter, LUT_SAMPLES);
    let envelope_r = cutter.envelope_radius_mm();
    ToolEntry {
        tag,
        label,
        cutter,
        lut,
        envelope_r,
    }
}

/// The five tools the two programs actually load, built exactly as
/// `compute::cutter::build_cutter` would (RECON Q5). Tool id 4 (R2.0, T10) is
/// defined in the toml but used by no toolpath, so it is not here.
fn wanaka_tools() -> Vec<ToolEntry> {
    vec![
        // toml id 0 (`wanaka200.toml:76-97`): end_mill, Ø6.0, cutting_length 25.0.
        tool_entry(
            "T1",
            "6mm 2F Carbide End Mill",
            Box::new(FlatEndmill::new(6.0, 25.0)),
        ),
        // toml id 5 (`:191-212`): v_bit, Ø5.5, included_angle 20.0, len 15.0.
        tool_entry(
            "T20",
            "20 deg V-bit 5.5mm 2F",
            Box::new(VBitEndmill::new(5.5, 20.0, 15.0)),
        ),
        // toml id 2 (`:122-143`): tapered_ball_nose, Ø2.0 ball, taper 5.7°,
        // shaft Ø6.0, cutting_length 20.0.
        tool_entry(
            "T8",
            "R1.0mm x 6mm x 20mm 2F Tapered Ball",
            Box::new(TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)),
        ),
        // toml id 3 (`:145-166`): Ø3.0 ball, taper 2.8°, shaft Ø6.0, len 30.5.
        tool_entry(
            "T15",
            "R1.5mm x 6mm x 30.5mm 2F Tapered Ball",
            Box::new(TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5)),
        ),
        // toml id 1 (`:99-120`): Ø1.0 ball, taper 7.1°, shaft Ø6.0, len 20.0.
        tool_entry(
            "T6",
            "R0.5mm x 6mm x 20mm 2F Tapered Ball",
            Box::new(TaperedBallEndmill::new(1.0, 7.1, 6.0, 20.0)),
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 2 — the modal G-code parser
// ══════════════════════════════════════════════════════════════════════
//
// Scope, exactly (RECON Q2): `G0/G1/G2/G3` with `X Y Z I J F`; `G17 G21 G90
// G40 G49 G80 G54` accepted and ignored; `M` words ignored; `(comment)` lines
// carry structure. No canned cycles — drill pecks are pre-expanded G0/G1.
// I/J are INCREMENTAL, start → centre (`emitter.rs:358-364,:382`).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MoveKind {
    Rapid,
    Feed,
    ArcCw,
    ArcCcw,
}

impl MoveKind {
    fn is_cutting(self) -> bool {
        self != MoveKind::Rapid
    }
}

#[derive(Clone, Copy, Debug)]
struct Move {
    kind: MoveKind,
    from: [f64; 3],
    to: [f64; 3],
    /// Arc centre offset, incremental from `from`. `None` on linear moves.
    ij: Option<[f64; 2]>,
    /// Modal feed in force for this block (mm/min). Not used geometrically;
    /// parsed so the modal-carry contract is testable.
    feed: f64,
    /// Index into the tool table. Per-MOVE, not per-op: the shipped files put
    /// the op-boundary comment BEFORE the tool change (`wanaka200_1_Setup_1.nc`
    /// line 12158 `(4 Rivers [back, V-bit])`, line 12160 `(TOOL CHANGE: … [T20])`).
    tool: usize,
    op: usize,
    /// 1-based line in the .nc file.
    line: usize,
    /// Was the machine position established before this block? The first
    /// block of each program is a Z-only safe-Z retract with no XY, so the
    /// first one or two moves have a fictitious start point. Those are
    /// recorded but never probed and never stamped.
    from_known: bool,
}

#[derive(Clone, Debug)]
struct OpSection {
    /// Leading integer of the `(N <name>)` comment.
    index: usize,
    name: String,
    /// Tool active at the section's FIRST MOVE (see `Move::tool`).
    tool: usize,
    start: usize,
    end: usize,
}

struct Program {
    ops: Vec<OpSection>,
    moves: Vec<Move>,
    /// `[Tn]` tags seen in LOAD / TOOL CHANGE comments with no table entry.
    unknown_tool_tags: Vec<String>,
}

/// `(LOAD: <name> [Tn])` / `(TOOL CHANGE: <name> [Tn])` → `"Tn"`.
///
/// Keyed on the bracket tag, not the name: `sanitize_comment_text` maps
/// parentheses to brackets inside comments (`post.rs:313-320`), so names are
/// not reliable keys.
fn tool_tag_in_comment(body: &str) -> Option<String> {
    if !(body.starts_with("LOAD:") || body.starts_with("TOOL CHANGE:")) {
        return None;
    }
    let open = body.rfind("[T")?;
    let rest = &body[open + 1..];
    let close = rest.find(']')?;
    Some(rest[..close].to_owned())
}

/// `(N <op name>)` → `(N, "<op name>")`. Anything not starting with digits
/// followed by a space is not an op header.
fn op_header(body: &str) -> Option<(usize, String)> {
    let (head, rest) = body.split_once(' ')?;
    if head.is_empty() || !head.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let index = head.parse::<usize>().ok()?;
    Some((index, rest.trim().to_owned()))
}

fn parse_program(src: &str, tools: &[ToolEntry]) -> Program {
    let mut moves: Vec<Move> = Vec::new();
    let mut ops: Vec<OpSection> = Vec::new();
    let mut unknown_tool_tags: Vec<String> = Vec::new();

    let mut pos = [0.0_f64; 3];
    let mut pos_established = false;
    let mut feed = 0.0_f64;
    let mut tool = 0_usize;
    let mut cur_op: Option<usize> = None;

    for (zero_based, raw) in src.lines().enumerate() {
        let line = zero_based + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(after) = trimmed.strip_prefix('(') {
            let body = after.strip_suffix(')').unwrap_or(after);
            if let Some(tag) = tool_tag_in_comment(body) {
                let found = tools.iter().position(|t| t.tag == tag);
                match found {
                    Some(i) => tool = i,
                    None => unknown_tool_tags.push(tag),
                }
            } else if let Some((index, name)) = op_header(body) {
                if let Some(prev) = cur_op {
                    ops[prev].end = moves.len();
                }
                ops.push(OpSection {
                    index,
                    name,
                    tool,
                    start: moves.len(),
                    end: moves.len(),
                });
                cur_op = Some(ops.len() - 1);
            }
            continue;
        }

        // Defensive: the emitter never puts a comment after code on one line,
        // but a trailing `(...)` would otherwise tokenize as words.
        let code = match trimmed.find('(') {
            Some(i) => &trimmed[..i],
            None => trimmed,
        };

        let mut motion: Option<MoveKind> = None;
        let (mut wx, mut wy, mut wz) = (None, None, None);
        let (mut wi, mut wj) = (None, None);
        for token in code.split_whitespace() {
            let mut chars = token.chars();
            let Some(letter) = chars.next() else {
                continue;
            };
            let value = chars.as_str();
            match letter.to_ascii_uppercase() {
                'G' => {
                    if let Ok(g) = value.parse::<u32>() {
                        motion = match g {
                            0 => Some(MoveKind::Rapid),
                            1 => Some(MoveKind::Feed),
                            2 => Some(MoveKind::ArcCw),
                            3 => Some(MoveKind::ArcCcw),
                            // G17 / G21 / G90 / G40 / G49 / G80 / G54 …
                            _ => motion,
                        };
                    }
                }
                'X' => wx = value.parse::<f64>().ok(),
                'Y' => wy = value.parse::<f64>().ok(),
                'Z' => wz = value.parse::<f64>().ok(),
                'I' => wi = value.parse::<f64>().ok(),
                'J' => wj = value.parse::<f64>().ok(),
                'F' => {
                    if let Ok(f) = value.parse::<f64>() {
                        feed = f;
                    }
                }
                _ => {}
            }
        }

        let Some(kind) = motion else {
            continue;
        };

        let op = match cur_op {
            Some(i) => i,
            None => {
                ops.push(OpSection {
                    index: 0,
                    name: "(preamble)".to_owned(),
                    tool,
                    start: moves.len(),
                    end: moves.len(),
                });
                cur_op = Some(ops.len() - 1);
                ops.len() - 1
            }
        };

        let from = pos;
        let to = [
            wx.unwrap_or(pos[0]),
            wy.unwrap_or(pos[1]),
            wz.unwrap_or(pos[2]),
        ];
        let ij = match (kind, wi, wj) {
            (MoveKind::ArcCw | MoveKind::ArcCcw, Some(i), Some(j)) => Some([i, j]),
            _ => None,
        };
        moves.push(Move {
            kind,
            from,
            to,
            ij,
            feed,
            tool,
            op,
            line,
            from_known: pos_established,
        });
        pos = to;
        if wx.is_some() && wy.is_some() && wz.is_some() {
            pos_established = true;
        }
    }

    if let Some(prev) = cur_op {
        ops[prev].end = moves.len();
    }
    // An op's reported tool is the one its first move actually ran with.
    for op in ops.iter_mut() {
        if op.start < moves.len() && op.start < op.end {
            op.tool = moves[op.start].tool;
        }
    }

    Program {
        ops,
        moves,
        unknown_tool_tags,
    }
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 3 — geometry helpers
// ══════════════════════════════════════════════════════════════════════

fn p3(v: [f64; 3]) -> P3 {
    P3::new(v[0], v[1], v[2])
}

fn stock_volume_mm3(stock: &TriDexelStock) -> f64 {
    let grid = &stock.z_grid;
    let cell_area = grid.cell_size * grid.cell_size;
    let total: f64 = grid
        .rays
        .iter()
        .map(|r| f64::from(ray_material_length(r)))
        .sum();
    total * cell_area
}

/// Conservative XY bbox `[x_min, y_min, x_max, y_max]` of one move. Arcs use
/// the full circle bbox rather than the true swept arc — cheap, and erring
/// LARGE only ever re-stamps a move that would not have mattered.
fn move_xy_bbox(mv: &Move) -> [f64; 4] {
    if let Some(ij) = mv.ij {
        let cx = mv.from[0] + ij[0];
        let cy = mv.from[1] + ij[1];
        let r = (ij[0] * ij[0] + ij[1] * ij[1]).sqrt();
        return [cx - r, cy - r, cx + r, cy + r];
    }
    [
        mv.from[0].min(mv.to[0]),
        mv.from[1].min(mv.to[1]),
        mv.from[0].max(mv.to[0]),
        mv.from[1].max(mv.to[1]),
    ]
}

/// The along-segment sample plan for one rapid: `(sample_count, spacing_mm)`.
///
/// A pure vertical drop needs ONE disc query — the XY is constant, so the
/// required clearance is constant and the margin is minimised at the lowest
/// tip Z. Anything with XY travel is swept at `step_mm` or finer.
fn rapid_sample_plan(mv: &Move, step_mm: f64) -> (usize, f64) {
    let dx = mv.to[0] - mv.from[0];
    let dy = mv.to[1] - mv.from[1];
    let xy_len = (dx * dx + dy * dy).sqrt();
    if xy_len <= XY_STATIONARY_MM {
        return (1, 0.0);
    }
    let segments = ((xy_len / step_mm).ceil() as usize).clamp(1, MAX_RAPID_SAMPLES);
    (segments + 1, xy_len / segments as f64)
}

struct ProbeReading {
    /// `min over samples of (tip_z − required_clearance)`. `INFINITY` when no
    /// sample produced a constraint.
    min_margin: f64,
    /// The sample position that produced `min_margin`.
    at: [f64; 3],
    samples: usize,
    /// Samples where the primitive returned `None`. RECON Q3: that is "disc
    /// entirely off-grid" or "every visited cell past the envelope" — it is
    /// NOT "no material", so it is counted, never read as a clearance.
    none_samples: usize,
    /// Set when the sample cap bound and the spacing contract was coarsened.
    spacing_exceeded: bool,
}

fn probe_rapid(
    stock: &TriDexelStock,
    mv: &Move,
    cutter: &dyn MillingCutter,
    radius: f64,
    step_mm: f64,
) -> ProbeReading {
    let (count, spacing) = rapid_sample_plan(mv, step_mm);
    let mut reading = ProbeReading {
        min_margin: f64::INFINITY,
        at: mv.from,
        samples: 0,
        none_samples: 0,
        spacing_exceeded: count > 1 && spacing > step_mm * 1.000_001,
    };
    for k in 0..count {
        let (x, y, z) = if count == 1 {
            (mv.from[0], mv.from[1], mv.from[2].min(mv.to[2]))
        } else {
            let t = k as f64 / (count - 1) as f64;
            (
                mv.from[0] + t * (mv.to[0] - mv.from[0]),
                mv.from[1] + t * (mv.to[1] - mv.from[1]),
                mv.from[2] + t * (mv.to[2] - mv.from[2]),
            )
        };
        reading.samples += 1;
        let Some(required) = stock.max_clearance_tip_z_for_profile(x, y, radius, cutter) else {
            reading.none_samples += 1;
            continue;
        };
        let margin = z - required;
        if margin < reading.min_margin {
            reading.min_margin = margin;
            reading.at = [x, y, z];
        }
    }
    reading
}

fn stamp_cut_move(
    stock: &mut TriDexelStock,
    mv: &Move,
    tools: &[ToolEntry],
    arc_buf: &mut Vec<P3>,
    arc_seg_mm: f64,
) {
    let tool = &tools[mv.tool];
    let start = p3(mv.from);
    let end = p3(mv.to);
    match mv.ij {
        Some(ij) => {
            let cw = mv.kind == MoveKind::ArcCw;
            linearize_arc_into(arc_buf, start, end, ij[0], ij[1], cw, arc_seg_mm);
            for pair in arc_buf.windows(2) {
                stock.stamp_linear_segment(
                    &tool.lut,
                    tool.envelope_r,
                    pair[0],
                    pair[1],
                    StockCutDirection::FromTop,
                );
            }
        }
        None => {
            stock.stamp_linear_segment(
                &tool.lut,
                tool.envelope_r,
                start,
                end,
                StockCutDirection::FromTop,
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 4 — verdicts and bookkeeping
// ══════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    /// Fine tier, envelope-negative, and it persists at the shaved radius.
    Strike,
    /// Fine tier, envelope-negative but shaved-clear: the tool is riding its
    /// own kerf. Benign by construction.
    KerfGraze,
    /// Cleared the envelope by less than `NEAR_MISS_MM`.
    NearMiss,
    Clean,
    /// Flagged coarse, dropped by the fine-tier budget.
    Budgeted,
    /// Fine window would have exceeded the cell cap, so it was not built.
    WindowTooLarge,
    /// The rapid's neighbourhood does not intersect the stock box at all.
    WindowOffStock,
}

impl Verdict {
    fn tag(self) -> &'static str {
        match self {
            Verdict::Strike => "STRIKE",
            Verdict::KerfGraze => "KERF-GRAZE",
            Verdict::NearMiss => "NEAR-MISS",
            Verdict::Clean => "clean",
            Verdict::Budgeted => "budgeted-out",
            Verdict::WindowTooLarge => "window-too-large",
            Verdict::WindowOffStock => "off-stock",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RapidFlag {
    move_idx: usize,
    op: usize,
    tool: usize,
    line: usize,
    coarse_margin: f64,
    at: [f64; 3],
}

#[derive(Clone, Copy, Debug)]
struct FineRow {
    flag: RapidFlag,
    margin_env: f64,
    margin_shaved: f64,
    verdict: Verdict,
    /// Prior cutting moves re-stamped into the window.
    restamped: usize,
    /// `None` readings from the envelope probe (RECON Q3 census).
    none_samples: usize,
}

#[derive(Clone, Copy, Default)]
struct OpCounters {
    rapids: usize,
    early_out: usize,
    origin_unknown: usize,
    probed: usize,
    flagged: usize,
    fine: usize,
    strikes: usize,
    grazes: usize,
    near_misses: usize,
    clean: usize,
    coarse_none_samples: usize,
    worst_coarse: Option<RapidFlag>,
    worst_fine: Option<FineRow>,
}

#[derive(Default)]
struct Totals {
    flagged: usize,
    adjudicated: usize,
    dropped: usize,
    strikes: usize,
    grazes: usize,
    near_misses: usize,
    clean: usize,
    window_too_large: usize,
    window_off_stock: usize,
    coarse_none_samples: usize,
    fine_none_samples: usize,
    probe_samples: usize,
    unstamped_unknown_origin: usize,
    spacing_exceeded: usize,
    /// Smallest (i.e. most alarming) coarse margin among the flagged rapids
    /// the fine-tier budget dropped. Reported so the cap is never silent.
    worst_dropped_margin: f64,
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 5 — setups (RECON Q1: exported-frame stock boxes)
// ══════════════════════════════════════════════════════════════════════

struct SetupSpec {
    label: &'static str,
    path: &'static str,
    /// Stock box **in exported coordinates**. The frame comes from the
    /// exporter's own contract (RECON Q1), not from a run-log table.
    stock: BoundingBox3,
    /// Op indices the parser must find, from RECON Q6's table.
    expected_ops: &'static [usize],
    /// Tripwire (a): op index + every exported XY its pin drill must hit.
    pin_probe: Option<(usize, &'static [[f64; 2]])>,
}

fn setups() -> Vec<SetupSpec> {
    vec![
        SetupSpec {
            label: "Setup 1 (back, face_up=bottom)",
            path: SETUP1_NC,
            // RECON Q1: non-identity setup, shift is zero, toolpath is
            // generated AND exported in the zero-rooted setup-local frame:
            // exported = (x_w + 20, 225 − y_w, 7 − z_w). Stock top at Z=25.
            stock: BoundingBox3 {
                min: P3::new(0.0, 0.0, 0.0),
                max: P3::new(240.0, 250.0, 25.0),
            },
            expected_ops: &[1, 2, 3, 4, 5],
            // Op 1 is `alignment_pin_drill` with
            // `holes = [[2.5, 2.5], [237.5, 247.5]]` (`wanaka200.toml:274-283`),
            // authored in this setup's own frame. Both must appear verbatim as
            // fed positions in the exported program — a wrong export frame
            // (a stray datum shift, a mirrored Y) moves them and this trips.
            pin_probe: Some((1, &[[2.5, 2.5], [237.5, 247.5]])),
        },
        SetupSpec {
            label: "Setup 2 (front, face_up=top, identity)",
            path: SETUP2_NC,
            // RECON Q1: identity setup, exported = world + (+20, +25, 0);
            // Z is never shifted, so the box is [-18, 7] and the top is Z=7.
            stock: BoundingBox3 {
                min: P3::new(0.0, 0.0, -18.0),
                max: P3::new(240.0, 250.0, 7.0),
            },
            expected_ops: &[6, 7, 8],
            pin_probe: None,
        },
    ]
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 6 — the fine tier
// ══════════════════════════════════════════════════════════════════════

struct ReplayContext<'a> {
    spec: &'a SetupSpec,
    prog: &'a Program,
    tools: &'a [ToolEntry],
    move_bboxes: &'a [[f64; 4]],
}

/// Fine window: rapid segment bbox ⊕ (envelope radius + pad) in XY,
/// **intersected with the stock box**, full stock Z.
///
/// The intersection is load-bearing and not in the spec: `from_bounds` fills
/// every cell with material, so an unclamped window would invent phantom
/// stock outside the board and manufacture strikes at the edges.
fn fine_window(mv: &Move, envelope_r: f64, stock: &BoundingBox3) -> Option<BoundingBox3> {
    let bb = move_xy_bbox(mv);
    let pad = envelope_r + WINDOW_PAD_MM;
    let x_min = (bb[0] - pad).max(stock.min.x);
    let y_min = (bb[1] - pad).max(stock.min.y);
    let x_max = (bb[2] + pad).min(stock.max.x);
    let y_max = (bb[3] + pad).min(stock.max.y);
    if x_max <= x_min || y_max <= y_min {
        return None;
    }
    Some(BoundingBox3 {
        min: P3::new(x_min, y_min, stock.min.z),
        max: P3::new(x_max, y_max, stock.max.z),
    })
}

fn adjudicate_fine(ctx: &ReplayContext<'_>, flag: &RapidFlag, totals: &mut Totals) -> FineRow {
    let mv = &ctx.prog.moves[flag.move_idx];
    let tool = &ctx.tools[flag.tool];

    let mut row = FineRow {
        flag: *flag,
        margin_env: f64::INFINITY,
        margin_shaved: f64::INFINITY,
        verdict: Verdict::Clean,
        restamped: 0,
        none_samples: 0,
    };

    let Some(window) = fine_window(mv, tool.envelope_r, &ctx.spec.stock) else {
        row.verdict = Verdict::WindowOffStock;
        totals.window_off_stock += 1;
        return row;
    };

    let ext_u = window.max.x - window.min.x;
    let ext_v = window.max.y - window.min.y;
    let cells = ((ext_u / FINE_CELL_MM).ceil() as usize + 1)
        .saturating_mul((ext_v / FINE_CELL_MM).ceil() as usize + 1);
    if cells > MAX_FINE_WINDOW_CELLS
        || DexelGrid::would_exceed_grid(FINE_CELL_MM, ext_u, ext_v).is_some()
    {
        row.verdict = Verdict::WindowTooLarge;
        totals.window_too_large += 1;
        return row;
    }

    let mut fine = TriDexelStock::from_bounds(&window, FINE_CELL_MM);
    // `DexelGrid::clamp_cell_size` coarsens SILENTLY past its cap; a
    // silently-coarsened fine tier would adjudicate at the wrong conservatism.
    assert!(
        (fine.z_grid.cell_size - FINE_CELL_MM).abs() < 1e-12,
        "fine window was silently coarsened to {} mm (requested {FINE_CELL_MM})",
        fine.z_grid.cell_size
    );

    // Re-stamp ONLY the prior cutting moves. Stamping anything later would
    // remove material that did not exist when this rapid executed, which
    // converts real strikes into clean readings — the one direction a safety
    // instrument must not err in.
    let mut arc_buf: Vec<P3> = Vec::new();
    for (i, prior) in ctx.prog.moves.iter().enumerate().take(flag.move_idx) {
        if !prior.kind.is_cutting() || !prior.from_known {
            continue;
        }
        let bb = ctx.move_bboxes[i];
        let r = ctx.tools[prior.tool].envelope_r;
        if bb[2] + r < window.min.x
            || bb[0] - r > window.max.x
            || bb[3] + r < window.min.y
            || bb[1] - r > window.max.y
        {
            continue;
        }
        stamp_cut_move(&mut fine, prior, ctx.tools, &mut arc_buf, FINE_CELL_MM);
        row.restamped += 1;
    }

    let step = FINE_CELL_MM * 0.5;
    let env = probe_rapid(&fine, mv, &*tool.cutter, tool.envelope_r, step);
    let shave = FINE_CELL_MM * std::f64::consts::SQRT_2 * SHAVE_CELL_DIAGONALS;
    let shaved_r = (tool.envelope_r - shave).max(MIN_SHAVED_RADIUS_MM);
    let shaved = probe_rapid(&fine, mv, &*tool.cutter, shaved_r, step);

    row.margin_env = env.min_margin;
    row.margin_shaved = shaved.min_margin;
    row.none_samples = env.none_samples;
    totals.fine_none_samples += env.none_samples;
    totals.probe_samples += env.samples + shaved.samples;
    if env.spacing_exceeded || shaved.spacing_exceeded {
        totals.spacing_exceeded += 1;
    }

    row.verdict = if env.min_margin < 0.0 {
        if shaved.min_margin >= 0.0 {
            Verdict::KerfGraze
        } else {
            Verdict::Strike
        }
    } else if env.min_margin < NEAR_MISS_MM {
        Verdict::NearMiss
    } else {
        Verdict::Clean
    };
    row
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 7 — one setup, end to end
// ══════════════════════════════════════════════════════════════════════

#[allow(clippy::too_many_lines)] // one instrument, one linear narrative
fn run_setup(spec: &SetupSpec, tools: &[ToolEntry], totals: &mut Totals) {
    let started = Instant::now();
    let src = std::fs::read_to_string(spec.path).expect("read shipped .nc");
    let prog = parse_program(&src, tools);
    assert!(
        prog.unknown_tool_tags.is_empty(),
        "{}: unmapped tool tags {:?} — the tool table is incomplete",
        spec.label,
        prog.unknown_tool_tags
    );

    let found_ops: Vec<usize> = prog.ops.iter().map(|o| o.index).collect();
    assert_eq!(
        found_ops, spec.expected_ops,
        "{}: parsed op indices do not match RECON Q6",
        spec.label
    );

    eprintln!(
        "\n══ {} ══\n  {} lines, {} moves, {} ops, stock [{:.1},{:.1}]x[{:.1},{:.1}]x[{:.1},{:.1}]",
        spec.label,
        src.lines().count(),
        prog.moves.len(),
        prog.ops.len(),
        spec.stock.min.x,
        spec.stock.max.x,
        spec.stock.min.y,
        spec.stock.max.y,
        spec.stock.min.z,
        spec.stock.max.z
    );

    // ── tripwire (a): the pin drill lands where the frame says it should ──
    if let Some((op_index, holes)) = spec.pin_probe {
        let op = prog
            .ops
            .iter()
            .find(|o| o.index == op_index)
            .expect("pin-drill op present");
        for xy in holes {
            let hit = prog.moves[op.start..op.end].iter().any(|m| {
                m.kind == MoveKind::Feed
                    && (m.to[0] - xy[0]).abs() < PIN_DRILL_XY_TOL_MM
                    && (m.to[1] - xy[1]).abs() < PIN_DRILL_XY_TOL_MM
            });
            assert!(
                hit,
                "{}: op {op_index} ({}) has no fed move at exported XY ({:.3}, {:.3}) — the \
                 export frame assumption is wrong",
                spec.label, op.name, xy[0], xy[1]
            );
        }
    }

    // ── tripwire (c): cutting geometry lives inside the transformed box ──
    //
    // ENDPOINTS only, deliberately. A fed move's START may legitimately sit
    // well above the stock: every drilled hole's first fed descent is rooted
    // at the R-plane (RECON Q2d — `effective_safe_z` puts it at Z30 in
    // setup 1, 5 mm above a 25 mm stock top), so the first descent is entirely
    // in air by construction. Checking `from` would abort the run on healthy
    // motion; checking `to` still catches a wrong frame anchor, because a bad
    // anchor moves every endpoint.
    for mv in prog.moves.iter().filter(|m| m.kind.is_cutting()) {
        let pad = tools[mv.tool].envelope_r + ENDPOINT_TOLERANCE_MM;
        let p = mv.to;
        let inside = p[0] >= spec.stock.min.x - pad
            && p[0] <= spec.stock.max.x + pad
            && p[1] >= spec.stock.min.y - pad
            && p[1] <= spec.stock.max.y + pad
            && p[2] >= spec.stock.min.z - pad
            && p[2] <= spec.stock.max.z + pad;
        assert!(
            inside,
            "{}: cutting move at line {} ends outside the stock box (+{pad:.1} mm): {:?} -> {:?}",
            spec.label, mv.line, mv.from, mv.to
        );
    }

    let move_bboxes: Vec<[f64; 4]> = prog.moves.iter().map(move_xy_bbox).collect();

    // ── COARSE TIER ──
    let stock_top = spec.stock.max.z;
    let coarse_step = COARSE_CELL_MM * 0.5;
    let mut stock = TriDexelStock::from_bounds(&spec.stock, COARSE_CELL_MM);
    assert!(
        (stock.z_grid.cell_size - COARSE_CELL_MM).abs() < 1e-12,
        "coarse grid was silently coarsened to {} mm",
        stock.z_grid.cell_size
    );
    let mut arc_buf: Vec<P3> = Vec::new();
    let mut counters = vec![OpCounters::default(); prog.ops.len()];
    let mut removed = vec![0.0_f64; prog.ops.len()];
    let mut flags: Vec<RapidFlag> = Vec::new();

    for (oi, op) in prog.ops.iter().enumerate() {
        let before = stock_volume_mm3(&stock);
        for (mi, mv) in prog.moves.iter().enumerate().take(op.end).skip(op.start) {
            let c = &mut counters[oi];
            if mv.kind.is_cutting() {
                if mv.from_known {
                    stamp_cut_move(&mut stock, mv, tools, &mut arc_buf, COARSE_CELL_MM);
                } else {
                    totals.unstamped_unknown_origin += 1;
                }
                continue;
            }
            c.rapids += 1;
            if !mv.from_known {
                c.origin_unknown += 1;
                continue;
            }
            // Exact early-out: required clearance can never exceed the
            // analytic stock top, so a tip this high cannot be flagged.
            if mv.from[2].min(mv.to[2]) - stock_top >= FLAG_MM {
                c.early_out += 1;
                continue;
            }
            let tool = &tools[mv.tool];
            let reading = probe_rapid(&stock, mv, &*tool.cutter, tool.envelope_r, coarse_step);
            c.probed += 1;
            c.coarse_none_samples += reading.none_samples;
            totals.coarse_none_samples += reading.none_samples;
            totals.probe_samples += reading.samples;
            if reading.spacing_exceeded {
                totals.spacing_exceeded += 1;
            }
            if reading.min_margin >= FLAG_MM {
                continue;
            }
            let flag = RapidFlag {
                move_idx: mi,
                op: mv.op,
                tool: mv.tool,
                line: mv.line,
                coarse_margin: reading.min_margin,
                at: reading.at,
            };
            c.flagged += 1;
            if c.worst_coarse
                .is_none_or(|w| flag.coarse_margin < w.coarse_margin)
            {
                c.worst_coarse = Some(flag);
            }
            flags.push(flag);
        }
        removed[oi] = before - stock_volume_mm3(&stock);
    }

    // ── tripwire (b): every op removed material ──
    for (oi, op) in prog.ops.iter().enumerate() {
        assert!(
            removed[oi] > 0.0,
            "{}: op {} ({}) removed {:.6} mm³ at the {COARSE_CELL_MM} mm coarse tier — the \
             replay is not cutting the stock it thinks it is",
            spec.label,
            op.index,
            op.name,
            removed[oi]
        );
    }

    eprintln!(
        "  coarse replay: {:.1} s, {} rapids flagged of {} probed",
        started.elapsed().as_secs_f64(),
        flags.len(),
        counters.iter().map(|c| c.probed).sum::<usize>()
    );
    totals.flagged += flags.len();

    // ── FINE TIER ──
    flags.sort_by(|a, b| a.coarse_margin.total_cmp(&b.coarse_margin));
    let budget = flags.len().min(MAX_FINE_ADJUDICATIONS);
    if flags.len() > budget {
        let worst_dropped = flags[budget].coarse_margin;
        totals.dropped += flags.len() - budget;
        totals.worst_dropped_margin = totals.worst_dropped_margin.min(worst_dropped);
        eprintln!(
            "  NOTE: fine-tier budget {MAX_FINE_ADJUDICATIONS} — {} flagged rapids DROPPED \
             (worst dropped coarse margin {worst_dropped:.4} mm; the list is sorted worst-first, \
             so every dropped candidate was weaker than every adjudicated one)",
            flags.len() - budget
        );
    }

    let ctx = ReplayContext {
        spec,
        prog: &prog,
        tools,
        move_bboxes: &move_bboxes,
    };
    let fine_started = Instant::now();
    let mut rows: Vec<FineRow> = Vec::with_capacity(budget);
    for (n, flag) in flags.iter().take(budget).enumerate() {
        if n > 0 && n % 50 == 0 {
            eprintln!(
                "    … fine tier {n}/{budget} ({:.1} s)",
                fine_started.elapsed().as_secs_f64()
            );
        }
        rows.push(adjudicate_fine(&ctx, flag, totals));
    }
    for flag in flags.iter().skip(budget) {
        rows.push(FineRow {
            flag: *flag,
            margin_env: f64::NAN,
            margin_shaved: f64::NAN,
            verdict: Verdict::Budgeted,
            restamped: 0,
            none_samples: 0,
        });
    }
    totals.adjudicated += budget;

    for row in &rows {
        let oi = row.flag.op;
        let c = &mut counters[oi];
        match row.verdict {
            Verdict::Strike => {
                c.strikes += 1;
                totals.strikes += 1;
            }
            Verdict::KerfGraze => {
                c.grazes += 1;
                totals.grazes += 1;
            }
            Verdict::NearMiss => {
                c.near_misses += 1;
                totals.near_misses += 1;
            }
            Verdict::Clean => {
                c.clean += 1;
                totals.clean += 1;
            }
            Verdict::Budgeted | Verdict::WindowTooLarge | Verdict::WindowOffStock => {}
        }
        // `fine` counts adjudications that produced a real verdict — a
        // budgeted-out, off-stock or oversized-window row is not one.
        if !matches!(
            row.verdict,
            Verdict::Strike | Verdict::KerfGraze | Verdict::NearMiss | Verdict::Clean
        ) {
            continue;
        }
        c.fine += 1;
        if c.worst_fine.is_none_or(|w| row.margin_env < w.margin_env) {
            c.worst_fine = Some(*row);
        }
    }

    report_setup(spec, &prog, tools, &counters, &removed);
    report_worst_rows(&rows);
    eprintln!(
        "  setup wall time {:.1} s (fine tier {:.1} s)",
        started.elapsed().as_secs_f64(),
        fine_started.elapsed().as_secs_f64()
    );
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 8 — the report
// ══════════════════════════════════════════════════════════════════════

fn short(name: &str, n: usize) -> String {
    if name.len() <= n {
        return name.to_owned();
    }
    let mut s: String = name.chars().take(n.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn report_setup(
    spec: &SetupSpec,
    prog: &Program,
    tools: &[ToolEntry],
    counters: &[OpCounters],
    removed: &[f64],
) {
    eprintln!("\n  per-op replay — {}", spec.label);
    eprintln!(
        "    {:>3} {:<22} {:<5} {:>11} {:>7} {:>6} {:>6} {:>5} {:>5} | {:>4} {:>5} {:>4} {:>5}",
        "op",
        "name",
        "tool",
        "removed mm³",
        "rapids",
        "early",
        "probed",
        "flag",
        "fine",
        "STK",
        "graze",
        "near",
        "clean"
    );
    for (oi, op) in prog.ops.iter().enumerate() {
        let c = counters[oi];
        eprintln!(
            "    {:>3} {:<22} {:<5} {:>11.1} {:>7} {:>6} {:>6} {:>5} {:>5} | {:>4} {:>5} {:>4} \
             {:>5}",
            op.index,
            short(&op.name, 22),
            tools[op.tool].tag,
            removed[oi],
            c.rapids,
            c.early_out,
            c.probed,
            c.flagged,
            c.fine,
            c.strikes,
            c.grazes,
            c.near_misses,
            c.clean
        );
    }
    eprintln!("\n    worst rapid per op (fine tier where adjudicated, else coarse):");
    for (oi, op) in prog.ops.iter().enumerate() {
        let c = counters[oi];
        match c.worst_fine {
            Some(row) => eprintln!(
                "      op {:>2} {:<22} nc:{:<7} {:<5} margin@env {:>9.4}  margin@shaved {:>9.4} \
                 at ({:.3}, {:.3}, {:.3})  [{}] {} prior moves re-stamped",
                op.index,
                short(&op.name, 22),
                row.flag.line,
                tools[row.flag.tool].tag,
                row.margin_env,
                row.margin_shaved,
                row.flag.at[0],
                row.flag.at[1],
                row.flag.at[2],
                row.verdict.tag(),
                row.restamped
            ),
            None => match c.worst_coarse {
                Some(flag) => eprintln!(
                    "      op {:>2} {:<22} nc:{:<7} {:<5} coarse margin {:>9.4} (not \
                     adjudicated)",
                    op.index,
                    short(&op.name, 22),
                    flag.line,
                    tools[flag.tool].tag,
                    flag.coarse_margin
                ),
                None => eprintln!(
                    "      op {:>2} {:<22} no rapid came within {FLAG_MM} mm of material",
                    op.index,
                    short(&op.name, 22)
                ),
            },
        }
        if c.origin_unknown > 0 {
            eprintln!(
                "        ({} rapid(s) skipped: machine position not yet established)",
                c.origin_unknown
            );
        }
        if c.coarse_none_samples > 0 {
            eprintln!(
                "        ({} coarse sample(s) returned None — off-grid or all-past-envelope, \
                 NOT 'no material')",
                c.coarse_none_samples
            );
        }
    }
}

fn report_worst_rows(rows: &[FineRow]) {
    let mut interesting: Vec<&FineRow> = rows
        .iter()
        .filter(|r| matches!(r.verdict, Verdict::Strike | Verdict::NearMiss))
        .collect();
    if interesting.is_empty() {
        return;
    }
    interesting.sort_by(|a, b| a.margin_env.total_cmp(&b.margin_env));
    eprintln!("\n    fine-tier STRIKE / NEAR-MISS detail (worst first, up to 20):");
    for row in interesting.iter().take(20) {
        eprintln!(
            "      nc:{:<7} margin@env {:>9.4}  margin@shaved {:>9.4}  at ({:.3}, {:.3}, {:.3}) \
             [{}]  {} prior moves re-stamped, {} None sample(s)",
            row.flag.line,
            row.margin_env,
            row.margin_shaved,
            row.flag.at[0],
            row.flag.at[1],
            row.flag.at[2],
            row.verdict.tag(),
            row.restamped,
            row.none_samples
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 9 — the evidence run
// ══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "evidence run — replays two shipped .nc programs against fine dexels; build --release"]
fn replay_shipped_wanaka_rapids_s1() {
    for path in [SETUP1_NC, SETUP2_NC, WANAKA_TOML] {
        if !Path::new(path).exists() {
            eprintln!("SKIP: {path} not present on this machine.");
            return;
        }
    }

    let tools = wanaka_tools();
    eprintln!("tool table (RECON Q5; geometry copied from wanaka200.toml):");
    for t in &tools {
        eprintln!(
            "  {:<4} envelope r {:.3} mm  {}",
            t.tag, t.envelope_r, t.label
        );
    }

    let mut totals = Totals {
        worst_dropped_margin: f64::INFINITY,
        ..Totals::default()
    };
    let started = Instant::now();
    for spec in setups() {
        run_setup(&spec, &tools, &mut totals);
    }

    eprintln!("\n══════════ S1 HEADLINE ══════════");
    eprintln!(
        "  flagged (coarse)  {:>6}\n  adjudicated (fine){:>6}\n  budgeted out      {:>6}",
        totals.flagged, totals.adjudicated, totals.dropped
    );
    if totals.dropped > 0 {
        eprintln!(
            "    worst dropped coarse margin {:.4} mm",
            totals.worst_dropped_margin
        );
    }
    eprintln!(
        "  STRIKES           {:>6}\n  kerf-grazes       {:>6}  (benign by construction)\n  \
         near-misses       {:>6}  (< {NEAR_MISS_MM} mm envelope clearance)\n  clean             \
         {:>6}",
        totals.strikes, totals.grazes, totals.near_misses, totals.clean
    );
    eprintln!(
        "  window off-stock  {:>6}\n  window too large  {:>6}\n  disc probes       {:>6}\n  \
         None samples      {:>6} coarse / {:>6} fine  (RECON Q3: off-grid or past-envelope, NOT \
         'no material')",
        totals.window_off_stock,
        totals.window_too_large,
        totals.probe_samples,
        totals.coarse_none_samples,
        totals.fine_none_samples
    );
    if totals.unstamped_unknown_origin > 0 {
        eprintln!(
            "  WARNING: {} cutting move(s) skipped for unestablished start position",
            totals.unstamped_unknown_origin
        );
    }
    if totals.spacing_exceeded > 0 {
        eprintln!(
            "  WARNING: {} rapid(s) hit the {MAX_RAPID_SAMPLES}-sample cap, so their sweep was \
             coarser than the spec's half-cell spacing",
            totals.spacing_exceeded
        );
    }

    let headline = if totals.strikes > 0 {
        "STRIKES FOUND — live safety defect; the shipped .nc files need review"
    } else if totals.near_misses > 0 {
        "LATENT NEAR-MISSES — no strike, but material sits inside the blind radius; fix on merit"
    } else {
        "NOTHING CLOSE — the geometry has been protecting us; the fix is still correct, at \
         ordinary priority"
    };
    eprintln!("\n  VERDICT: {headline}");
    eprintln!(
        "  total wall time {:.1} s\n",
        started.elapsed().as_secs_f64()
    );
}

// ══════════════════════════════════════════════════════════════════════
// SECTION 10 — parser unit tests (fast, no file I/O, not #[ignore]d)
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod parser_tests {
    use super::{
        Move, MoveKind, linearize_arc_into, op_header, p3, parse_program, rapid_sample_plan,
        tool_tag_in_comment, wanaka_tools,
    };
    use rs_cam_core::geo::P3;

    /// Mirrors the shipped grammar: F elided on continuation blocks, XYZ
    /// always spelled, the op header BEFORE the tool change.
    const SNIPPET: &str = "\
(Generated by rs_cam)
G17 G21 G90 G40 G49 G80
G54
M3 S8000
(LOAD: 6mm 2F Carbide End Mill [T1])
(1 Pin Drill)
G0 Z30.000
G0 X10.000 Y0.000 Z30.000
G1 X10.000 Y0.000 Z5.000 F2400
G1 X11.000 Y0.000 Z5.000
G1 X12.000 Y0.000 Z5.000 F1200
G1 X13.000 Y0.000 Z5.000
G0 X13.000 Y0.000 Z30.000
(2 Rivers [back, V-bit])
M5
(TOOL CHANGE: 20 deg V-bit 5.5mm 2F [T20])
M0
M3 S24000
G0 X20.000 Y20.000 Z30.000
G1 X20.000 Y20.000 Z4.000 F900
";

    #[test]
    fn modal_feed_carries_across_blocks_that_omit_f() {
        let tools = wanaka_tools();
        let prog = parse_program(SNIPPET, &tools);
        let feeds: Vec<f64> = prog
            .moves
            .iter()
            .filter(|m| m.kind == MoveKind::Feed)
            .map(|m| m.feed)
            .collect();
        assert_eq!(feeds, vec![2400.0, 2400.0, 1200.0, 1200.0, 900.0]);
    }

    #[test]
    fn tool_switch_follows_the_bracket_tag_not_the_name() {
        assert_eq!(
            tool_tag_in_comment("LOAD: 6mm 2F Carbide End Mill [T1]").as_deref(),
            Some("T1")
        );
        assert_eq!(
            tool_tag_in_comment("TOOL CHANGE: R1.5mm x 6mm x 30.5mm 2F Tapered Ball [T15]")
                .as_deref(),
            Some("T15")
        );
        assert_eq!(tool_tag_in_comment("4 Rivers [back, V-bit]"), None);
        assert_eq!(tool_tag_in_comment("Generated by rs_cam"), None);

        let tools = wanaka_tools();
        let t1 = tools.iter().position(|t| t.tag == "T1").unwrap();
        let t20 = tools.iter().position(|t| t.tag == "T20").unwrap();
        let prog = parse_program(SNIPPET, &tools);
        assert!(prog.unknown_tool_tags.is_empty());
        // Op 2's header precedes its TOOL CHANGE in the shipped grammar, so
        // the op's tool must come from its first MOVE, not from the header.
        assert_eq!(prog.ops[0].tool, t1);
        assert_eq!(prog.ops[1].tool, t20);
        assert!(
            prog.moves[prog.ops[1].start..prog.ops[1].end]
                .iter()
                .all(|m| m.tool == t20)
        );
    }

    #[test]
    fn op_boundary_comments_partition_the_move_list() {
        assert_eq!(op_header("1 Pin Drill"), Some((1, "Pin Drill".to_owned())));
        assert_eq!(
            op_header("3 Holes [6mm pilot]"),
            Some((3, "Holes [6mm pilot]".to_owned()))
        );
        assert_eq!(op_header("Generated by rs_cam"), None);
        assert_eq!(op_header("LOAD: 6mm 2F Carbide End Mill [T1]"), None);

        let tools = wanaka_tools();
        let prog = parse_program(SNIPPET, &tools);
        assert_eq!(prog.ops.len(), 2);
        assert_eq!(prog.ops[0].index, 1);
        assert_eq!(prog.ops[0].name, "Pin Drill");
        assert_eq!(prog.ops[1].index, 2);
        assert_eq!(prog.ops[1].name, "Rivers [back, V-bit]");
        // Contiguous, in order, and covering every move.
        assert_eq!(prog.ops[0].start, 0);
        assert_eq!(prog.ops[0].end, prog.ops[1].start);
        assert_eq!(prog.ops[1].end, prog.moves.len());
        assert!(prog.moves.iter().enumerate().all(|(i, m)| {
            let op = &prog.ops[m.op];
            i >= op.start && i < op.end
        }));
    }

    #[test]
    fn unestablished_start_position_is_marked_not_invented() {
        let tools = wanaka_tools();
        let prog = parse_program(SNIPPET, &tools);
        // `G0 Z30.000` (no XY) then the first full XYZ block: neither has a
        // real start point, everything after does.
        assert!(!prog.moves[0].from_known);
        assert!(!prog.moves[1].from_known);
        assert!(prog.moves[2..].iter().all(|m| m.from_known));
    }

    #[test]
    fn incremental_ij_reconstructs_a_quarter_arc_midpoint() {
        // G3 from (10,0,0) to (0,10,2) about the origin: I/J are start→centre
        // (RECON Q2a), so I=-10 J=0.
        const ARC: &str = "\
(LOAD: 6mm 2F Carbide End Mill [T1])
(1 Arc Op)
G0 X10.000 Y0.000 Z5.000
G1 X10.000 Y0.000 Z0.000 F1000
G3 X0.000 Y10.000 Z2.000 I-10.000 J0.000 F1000
";
        let tools = wanaka_tools();
        let prog = parse_program(ARC, &tools);
        let arc = prog
            .moves
            .iter()
            .find(|m| m.kind == MoveKind::ArcCcw)
            .expect("arc parsed");
        assert_eq!(arc.ij, Some([-10.0, 0.0]));

        let mut buf: Vec<P3> = Vec::new();
        let ij = arc.ij.unwrap();
        linearize_arc_into(
            &mut buf,
            p3(arc.from),
            p3(arc.to),
            ij[0],
            ij[1],
            false,
            0.05,
        );
        assert!(buf.len() > 100, "arc barely tessellated: {}", buf.len());
        // Centre reconstructed from the START, not the end.
        for pt in &buf {
            let r = (pt.x * pt.x + pt.y * pt.y).sqrt();
            assert!((r - 10.0).abs() < 1e-6, "off-radius point r={r}");
        }
        // 45° point of a 0→90° CCW sweep, with Z half-interpolated.
        let mid = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
        let best = buf
            .iter()
            .min_by(|a, b| {
                let da = (a.x - mid).hypot(a.y - mid);
                let db = (b.x - mid).hypot(b.y - mid);
                da.total_cmp(&db)
            })
            .unwrap();
        assert!((best.x - mid).abs() < 0.02, "mid x {}", best.x);
        assert!((best.y - mid).abs() < 0.02, "mid y {}", best.y);
        assert!((best.z - 1.0).abs() < 0.02, "mid z {}", best.z);
    }

    #[test]
    fn a_z_only_rapid_is_one_disc_query_and_an_xy_rapid_sweeps() {
        let base = Move {
            kind: MoveKind::Rapid,
            from: [10.0, 20.0, 30.0],
            to: [10.0, 20.0, 3.0],
            ij: None,
            feed: 0.0,
            tool: 0,
            op: 0,
            line: 1,
            from_known: true,
        };
        // Pure descent: one point probe at the lowest tip Z.
        let (count, spacing) = rapid_sample_plan(&base, 0.15);
        assert_eq!(count, 1);
        assert_eq!(spacing, 0.0);

        // Pure XY traverse at height: swept, never coarser than the step.
        let traverse = Move {
            to: [40.0, 20.0, 30.0],
            ..base
        };
        let (count, spacing) = rapid_sample_plan(&traverse, 0.15);
        assert_eq!(count, 201);
        assert!(spacing <= 0.15 + 1e-12, "spacing {spacing}");

        // The shipped files contain zero combined XY+Z rapids, but the plan
        // must still be correct if one ever ships.
        let diagonal = Move {
            to: [10.3, 20.4, 3.0],
            ..base
        };
        let (count, spacing) = rapid_sample_plan(&diagonal, 0.15);
        assert!(count > 2);
        assert!(spacing <= 0.15 + 1e-12);
    }
}
