//! Geodesic Adaptive: 2D adaptive clearing on a geodesic wavefront
//! (`planning/adaptive_research_2026-10-23/integration/step7_design.md`).
//!
//! The research's candidate (a) with the one preset that ships, C35 plan A
//! (INTEGRATION_PLAN Q8, D52: no replay pick, the default arc ends). The
//! research module (`research_geodesic.rs`, `research` feature) keeps the
//! options, the readouts and the 2D harness; this module holds no options.
//!
//! The one door is [`geodesic_toolpath`]. The 3D Rough variant (integration
//! step 7.4) is its first product caller; until then only the sentry
//! `the_geodesic_preset_keeps_its_plan_s7_3` calls it.
//!
//! # The plan of one piece (`plan.rs`)
//!
//! 1. The helical starter pocket at the entry (`path.rs`,
//!    `emit_helical_starter_pocket`). Its last lap, radius `R` about the
//!    plunge point, is the seed disc.
//! 2. The field `T` (`field.rs`): the geodesic distance from the seed disc
//!    inside the tool-centre region, islands and outside unreachable, solved
//!    by source tracking.
//! 3. The levels of `T`, spaced by the local-level law (`field.rs`).
//! 4. The core spiral (`wraps.rs`): the closed, star-shaped levels round the
//!    seed are cut as one spiral and a closing lap.
//! 5. The wraps (`wraps.rs`): the other level sets, in cutting order, each
//!    trimmed at its meeting-line ends.
//! 6. The joins: each open arc that follows an open arc is joined from the
//!    cutter (zigzag) by a planned clearing turn, when the turn is no longer
//!    than `turn_bound` (`s + (1 + √2)·cell`) and stays in the region.
//! 7. The 2D adaptive cleanup (`path.rs`) replays every cut under the pass
//!    load and admits the planned turns (`LinkLoad::admits_turn`).
//!
//! # The field
//!
//! `T(x)` is the geodesic distance from the seed disc to `x`. The front goes
//! round an island and grows into each lobe from the cleared side; the EDT
//! of the contour spiral (`spiral.rs`) measures the distance to the wall
//! instead. Source tracking: each node keeps the point its straight path to
//! the seed starts from (the seed centre, or the lattice node where the path
//! bends round a wall), and reads its distance exactly from there with the
//! planner's straight-move test (`ToolCentreRegion::contains_segment`). The
//! error is zero where the seed centre sees the node; behind a wall it is an
//! over-read of at most one cell diagonal, never an under-read. `T` is held
//! in stepovers. The lattice is the planner's (`planner_cell_size`); a node
//! is reachable when it is in the machinable mask and inside the exact
//! tool-centre region.
//!
//! # The meeting line
//!
//! Behind an island the fronts that went round its two sides meet on a line
//! (a shock), and each level set has a kink there. Source tracking names the
//! front branch of each node, so the meeting line is the cells whose corners
//! lie in two branches. The rule: the front continues from one side only.
//! A level set is cut at the meeting line; the branch cut first runs to the
//! line, the other ends one stepover short of it. The trimmed piece lies
//! within `√2·s` of the first branch's last cutter centre, inside its disc
//! when `s ≤ R / √2`.
//!
//! # The spacing law
//!
//! A pass at cutter-centre radius `ρ` round a cleared disc meets more stock
//! than its spacing, because the stock wraps round the cutter. The last pass
//! at `ρ - s'` cleared the disc of radius `a = ρ - s' + R`; the swept width
//! across the heading is `w = R - (a² - R² - ρ²) / (2ρ)`. Setting `w = c·D`
//! (the pass ceiling) gives `s'(ρ) = ρ + R - √(R² + ρ² + 2ρ(R - c·D))`,
//! capped at the stepover; it reaches `s` at `ρ* = s(2R - s) / (2(c·D - s))`.
//! Only `R`, `s` and the ceiling enter. The local-level spacing keeps ONE
//! value of `T` per level, so the levels stay parallel: each step is the
//! largest for which every point of the new level lies within
//! `min(s, s'(ρ))` of the cut, `ρ` the level set's own three-point radius
//! over one pass step (`GeodesicField::use_level_spacing`).
//!
//! # Derived constants
//!
//! - `reach = s + (1 + √2)·cell`: one level spacing, plus the field error
//!   (at most one cell), plus one cell diagonal (the longest contour segment
//!   in one cell). The planned turn bound is the same sum (`turn_bound`).
//! - A chain is closed when its ends are at most one cell diagonal apart;
//!   fragment ends join within `CHAIN_EPS` (`geometry::marching_squares`).
//! - The pass step is the planner's, `3·cell` (`pass_step_len`).
//! - The core spiral's angle step keeps each chord within the op tolerance
//!   of its arc, and holds its heading tilt to the tolerance (`held_tilt`).
//! - The meeting-line trim is one stepover.
//! - The local-level bisection stops at a bracket of the op tolerance.
//!
//! # Known limits
//!
//! - A meeting of two fronts that a wall does not split (a lobe junction
//!   with no island) is one branch and is not trimmed.
//! - Source tracking stops after `rows + cols` rounds (the longest lattice
//!   path).

use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

use super::path::segments_to_toolpath;
use super::search::ToolCentreRegion;
use super::{AdaptiveParams, KeepDownLinks, PathStrategy2d};

mod field;
mod plan;
mod wraps;

use plan::{cleaned, params_with, plan_piece};

/// The Geodesic Adaptive toolpath of `outline` with `params`: each inset
/// piece (`inset_piece_outlines`, as the 2D adapter splits it) planned by
/// [`plan_piece`], cleaned by the 2D adaptive cleanup with its planned turns
/// ([`cleaned`]), and emitted. A piece where the field does not apply (no
/// starter pocket fits, or no node is seeded) is not planned.
///
/// The params' path strategy is set to `PathStrategy2d::Agent`, so the
/// contour spiral's corner blend does not apply. The planned turns keep the
/// commanded feed: the feed factor of a turn is the 3D variant's work
/// (step 7.4, design part F).
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires.
pub fn geodesic_toolpath(
    outline: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let params = params_with(params, PathStrategy2d::Agent);
    let pieces = super::inset_piece_outlines(outline, params.tool_radius);
    let pieces = if pieces.is_empty() {
        vec![outline.clone()]
    } else {
        pieces
    };
    let mut out = Toolpath::new();
    for piece in &pieces {
        let Some(plan) = plan_piece(piece, &params, cancel)? else {
            continue;
        };
        let segments = cleaned(piece, &params, plan.segments, &plan.turns);
        let region = (params.keep_down_links == KeepDownLinks::WithinPassLoad)
            .then(|| ToolCentreRegion::new(piece, params.tool_radius));
        let (tp, _) = segments_to_toolpath(&segments, &params, region.as_ref());
        out.moves.extend(tp.moves);
    }
    Ok(out)
}
