//! Cantilever stiffness guardrail — purely geometric L/D ratio.
//!
//! Phase 1a deliberately does **no force prediction**. We do not compute
//! bending stress, do not compare to material yield, do not estimate
//! deflection magnitude. All of that lives behind the Phase 6 hardware-
//! validation gate.
//!
//! What we *can* honestly say from geometry alone: a cantilever with
//! length-to-diameter ratio above ~6 is too flexible to maintain accuracy
//! and tool life under any non-trivial cutting load, regardless of
//! material or operation. The thresholds here are conservative bounds
//! widely cited in machining-handbook practice; they are not derived from
//! a force model.

use crate::tool::{MillingCutter, ToolDefinition};

use super::verdict::{Confidence, ExceedsReason, Verdict};

/// L/D thresholds. `WITHIN_BOUND` and `EXCEEDS_BOUND` mark the boundaries
/// between Validated / Approximate / Exceeds.
const WITHIN_BOUND: f64 = 4.0;
const EXCEEDS_BOUND: f64 = 6.0;

/// Evaluate the cantilever stiffness criterion for a tool. Sample-independent
/// — does not require simulation.
///
/// Uses `cutter.diameter()` as the cantilever diameter, which for a
/// `TaperedBallEndmill` correctly returns `shaft_diameter` (the widest
/// cutting cross-section), not the tip. See `ToolDefinition::to_assembly`
/// for the same convention.
pub fn evaluate(tool: &ToolDefinition) -> Verdict {
    let diameter = tool.diameter();
    if diameter <= 0.0 {
        // A zero-diameter cutter is a config error, not an L/D outcome.
        // Refuse rather than emit a divide-by-zero verdict.
        return Verdict::Unmodeled {
            reason: super::verdict::UnmodeledReason::NotImplemented(
                "cutter reports zero diameter".to_owned(),
            ),
        };
    }
    let ratio = tool.stickout / diameter;
    if ratio <= WITHIN_BOUND {
        Verdict::Within {
            peak: ratio,
            confidence: Confidence::Validated,
        }
    } else if ratio <= EXCEEDS_BOUND {
        Verdict::Within {
            peak: ratio,
            confidence: Confidence::Approximate(
                "long tool — accuracy and finish degrade above L/D=4".to_owned(),
            ),
        }
    } else {
        Verdict::Exceeds {
            peak: ratio,
            sample_range: 0..0,
            reason: ExceedsReason::LongToolStiffnessUnsafe,
            confidence: Confidence::Validated,
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::tool::{FlatEndmill, ToolDefinition};

    fn tool_with_stickout(diameter: f64, stickout: f64) -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(diameter, 20.0)),
            6.0,    // shank_diameter
            30.0,   // shank_length
            20.0,   // holder_diameter
            stickout,
            2,      // flute_count
            crate::compute::tool_config::ToolMaterial::Carbide,
        )
    }

    #[test]
    fn short_tool_is_within_validated() {
        // 6mm diameter, 18mm stickout → L/D = 3.0
        let v = evaluate(&tool_with_stickout(6.0, 18.0));
        match v {
            Verdict::Within {
                peak,
                confidence: Confidence::Validated,
            } => assert!((peak - 3.0).abs() < 1e-9),
            other => panic!("expected Within(Validated), got {other:?}"),
        }
    }

    #[test]
    fn medium_tool_is_within_approximate() {
        // 6mm diameter, 30mm stickout → L/D = 5.0 (between bounds)
        let v = evaluate(&tool_with_stickout(6.0, 30.0));
        match v {
            Verdict::Within {
                peak,
                confidence: Confidence::Approximate(_),
            } => assert!((peak - 5.0).abs() < 1e-9),
            other => panic!("expected Within(Approximate), got {other:?}"),
        }
    }

    #[test]
    fn long_tool_exceeds() {
        // 6mm diameter, 60mm stickout → L/D = 10.0
        let v = evaluate(&tool_with_stickout(6.0, 60.0));
        match v {
            Verdict::Exceeds {
                peak,
                reason: ExceedsReason::LongToolStiffnessUnsafe,
                ..
            } => assert!((peak - 10.0).abs() < 1e-9),
            other => panic!("expected Exceeds(LongToolStiffnessUnsafe), got {other:?}"),
        }
    }

    #[test]
    fn boundary_at_within_bound_is_within() {
        // exactly L/D = 4
        let v = evaluate(&tool_with_stickout(5.0, 20.0));
        assert!(matches!(
            v,
            Verdict::Within {
                confidence: Confidence::Validated,
                ..
            }
        ));
    }

    #[test]
    fn boundary_at_exceeds_bound_is_within_approximate() {
        // exactly L/D = 6 — still within (approximate), exceeds is strictly greater
        let v = evaluate(&tool_with_stickout(5.0, 30.0));
        assert!(matches!(
            v,
            Verdict::Within {
                confidence: Confidence::Approximate(_),
                ..
            }
        ));
    }

    #[test]
    fn zero_diameter_refuses() {
        // Zero diameter must not cause divide-by-zero or a passing verdict.
        let v = evaluate(&tool_with_stickout(0.0, 30.0));
        assert!(matches!(v, Verdict::Unmodeled { .. }));
    }
}
