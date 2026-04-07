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
    }
}
