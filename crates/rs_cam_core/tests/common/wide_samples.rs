//! The wide-sample reads of the feeds contract P22 readouts: one
//! definition for the P22a readout (`every_wide_step_has_a_named_source_p22a`)
//! and the P22f-2a sentry (`a_geodesic_arc_end_reads_the_stock_p22f2`).
//!
//! - The width of a sample is the power gate's: `Engagement::from_sample`
//!   (`r (1 - cos psi)` at the measured arc).
//! - The pass ceiling is the planner's own `c D`
//!   (`radial_woc_fraction_from_leading_arc(pass_engagement_limit(s, R))`).
//! - The read precision is one planner cell plus one simulation cell (the
//!   2D rule admits a sliver to the ceiling plus one cell; the simulation
//!   blends partial cells).
//! - The removal check: the sample's removal rate over the rate of a full
//!   rectangle at its width, `mrr / (ap ae F / 60)`. Near 1 or above: real
//!   removal. Near 0: the arc overstates the width (a thin ring). The
//!   bound 0.5 is the midpoint of the two cases (P22a section 4).

use rs_cam_core::feeds::engagement::Engagement;
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::ToolDefinition;

/// The pass ceiling `c` as a radial fraction of D at stepover `s` and tool
/// radius `r`.
pub fn ceiling_fraction(stepover: f64, tool_radius: f64) -> f64 {
    rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc(
        rs_cam_core::adaptive::pass_engagement_limit(stepover, tool_radius),
    )
}

/// The text of a `rs_cam_core` source file (`rel` from the crate root)
/// with each `//` comment cut to the line end, so a needle never matches a
/// comment. (Not `source_scan::strip_comments`: several test binaries load
/// that file by `#[path]` beside `mod common`, and one crate may load a file
/// once. The needles read here are code lines with no `//` in a string.)
pub fn core_source(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path)
        .expect("the planner source reads")
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The number in `src` between `prefix` and `suffix` (the first match). A
/// missing needle or a value that does not parse fails the test: the
/// source moved, and the reader must follow it.
pub fn number_in(src: &str, prefix: &str, suffix: &str) -> f64 {
    let start = src
        .find(prefix)
        .expect("the source holds the prefix: update the reader")
        .saturating_add(prefix.len());
    let rest = src.get(start..).expect("the prefix ends inside the source");
    let end = rest
        .find(suffix)
        .expect("the source holds the suffix after the prefix: update the reader");
    rest.get(..end)
        .expect("the suffix ends inside the source")
        .trim()
        .parse()
        .expect("the value between the prefix and the suffix is a number")
}

/// `PLANNER_CELLS_PER_TOOL_RADIUS`, read from `adaptive/path.rs`.
pub fn planner_cells_per_radius() -> f64 {
    number_in(
        &core_source("src/adaptive/path.rs"),
        "const PLANNER_CELLS_PER_TOOL_RADIUS: f64 =",
        ";",
    )
}

/// The planner cell `max(R / PLANNER_CELLS_PER_TOOL_RADIUS, tolerance)`
/// (`adaptive::path::planner_cell_size`), the divisor read from the source.
pub fn planner_cell(tool_radius: f64, tolerance: f64) -> f64 {
    (tool_radius / planner_cells_per_radius()).max(tolerance)
}

/// The engagement of a cutting sample through the gate's builder, or
/// `None` for air.
pub fn engagement(sample: &SimulationCutSample, tool: &ToolDefinition) -> Option<Engagement> {
    Engagement::from_sample(sample, tool)
}

/// The removal ratio `mrr / (ap ae F / 60)` of a sample, or `None` when the
/// full-rectangle rate is zero.
pub fn removal_ratio(sample: &SimulationCutSample, engagement: &Engagement) -> Option<f64> {
    let full = engagement.ap_mm() * engagement.ae_mm() * sample.feed_rate_mm_min / 60.0;
    (full > 0.0).then(|| sample.mrr_mm3_s / full)
}

/// The removal ratio at or above which a wide sample is real removal.
pub const REAL_REMOVAL: f64 = 0.5;

/// Whether a sample at `at` on move `i` lies within `reach` (XY) of the end
/// of move `i` where move `i + 1` is capped or draped, or of its start where
/// move `i - 1` is (the "joint" class of the P22a readout): the simulation
/// reads the material of that step at the shared point. A move is draped
/// when it is a clearing move with a Z change.
pub fn at_cap_joint(
    moves: &[rs_cam_core::toolpath::Move],
    i: usize,
    at: [f64; 3],
    reach: f64,
) -> bool {
    use rs_cam_core::geo::P3;
    use rs_cam_core::toolpath::MoveIntent;
    let near = |p: P3| (p.x - at[0]).hypot(p.y - at[1]) <= reach;
    let marked = |k: usize| {
        k >= 1
            && k < moves.len()
            && (moves[k].feed_cap.is_some()
                || (moves[k].intent == MoveIntent::ClearingCut
                    && (moves[k].target.z - moves[k - 1].target.z).abs() > 1e-6))
    };
    (i < moves.len() && marked(i + 1) && near(moves[i].target))
        || (i >= 1 && marked(i - 1) && near(moves[i - 1].target))
}

/// The sample indices that are the first or the last sample of their move,
/// for toolpath `id` (P22f-2d). Their segment time does not match the
/// volume their stamp removes (P22f-2c: removal ratio 2-6 at a move end,
/// 0.04-0.12 inside the move), so the removal and section reads skip them.
pub fn move_end_samples(
    samples: &[SimulationCutSample],
    id: rs_cam_core::ids::ToolpathId,
) -> std::collections::HashSet<usize> {
    let mut bounds: std::collections::HashMap<usize, (usize, usize)> =
        std::collections::HashMap::new();
    for s in samples.iter().filter(|s| s.toolpath_id == id) {
        let entry = bounds
            .entry(s.move_index)
            .or_insert((s.sample_index, s.sample_index));
        entry.0 = entry.0.min(s.sample_index);
        entry.1 = entry.1.max(s.sample_index);
    }
    bounds.values().flat_map(|&(a, b)| [a, b]).collect()
}

/// The section a sample removes (mm2): its removal rate over its feed,
/// `mrr / (F / 60)`. `None` at zero feed.
pub fn section_mm2(sample: &SimulationCutSample) -> Option<f64> {
    (sample.feed_rate_mm_min > 0.0).then(|| sample.mrr_mm3_s / (sample.feed_rate_mm_min / 60.0))
}
