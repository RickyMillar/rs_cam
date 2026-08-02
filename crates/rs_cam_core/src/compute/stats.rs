use super::config::ToolpathStats;
use crate::toolpath::{MoveType, Toolpath};

/// Compute basic toolpath statistics (move count, cutting distance, rapid distance).
// SAFETY: loop from 1..len, indexing [i] and [i-1] always in bounds
#[allow(clippy::indexing_slicing)]
pub fn compute_stats(tp: &Toolpath) -> ToolpathStats {
    let mut cutting = 0.0;
    let mut rapid = 0.0;
    for i in 1..tp.moves.len() {
        let from = tp.moves[i - 1].target;
        let to = tp.moves[i].target;
        let distance =
            ((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt();
        match tp.moves[i].move_type {
            MoveType::Rapid => rapid += distance,
            _ => cutting += distance,
        }
    }
    ToolpathStats {
        move_count: tp.moves.len(),
        cutting_distance: cutting,
        rapid_distance: rapid,
        // NOT MEASURED, not zero: this helper only sees moves, and a
        // cascade residual is not derivable from them. The generation path
        // overwrites it from `GenerationFindings`; every other caller must
        // keep reading `None` so no ratio is built on a fabricated zero
        // (`MEASUREMENT_DOMAINS.md` X-19).
        standing_material_mm2: None,
        // Same rule (Wave D1): a band drop and a centreline float are
        // generation-time findings. This helper only sees moves, so it can
        // only honestly say "not measured".
        dropped_band: None,
        tip_float: None,
        deprecated_dial: None,
        derived_stepovers: Vec::new(),
        // PR-8b: same rule again — no ramp descent is visible from a move
        // list, so this helper can only honestly say "not measured".
        ramp_reach_clamp: None,
        // A/M6: same rule — no claims pipeline is visible from a move list.
        claims_reference: None,
    }
}
