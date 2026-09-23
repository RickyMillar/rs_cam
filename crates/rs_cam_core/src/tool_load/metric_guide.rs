//! **Plain facts for the (i) hover of each cut metric.**
//! Package D of `planning/sim_cut_metrics_2026-09-23/PLAN.md` (§3.2, §4 D).
//!
//! Each guide gives four short statements: what the quantity is, what a
//! value that is too low means, what a value that is too high means, and
//! the levers that move it. The operator sees, hears or changes each one.
//!
//! The copy lives in core, so the GUI, the MCP tool-load report and the
//! CLI can print the same words.
//!
//! ## The levers follow ruling R4
//!
//! The feeds ruling R4 (2026-09-23, `planning/feeds_matrix_2026-09-23/
//! RULINGS.md`): keep the chipload in the vendor band, and reduce the load
//! through engagement (depth and width), not through feed alone. The
//! chipload and power levers say this.
//!
//! ## What this module does not change
//!
//! [`super::verdict::ExceededCriterion::remedy`] stays as it is. That text
//! is the refusal line for one exceedance; a guide is the general copy for
//! the card. Where the two agree, the guide uses the same facts.
//!
//! A kind this module has no card for (gantry push, the drill trio)
//! returns `None`. It does not get invented copy.

use serde::Serialize;

use super::verdict::CriterionKind;

/// The four lines of a metric's (i) hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MetricGuide {
    /// What the quantity is.
    pub what: &'static str,
    /// What a value below the band means, and what the operator sees or
    /// hears.
    pub too_low: &'static str,
    /// What a value above the band means, and what the operator sees or
    /// hears.
    pub too_high: &'static str,
    /// What the operator changes to move the value.
    pub levers: &'static str,
}

const CHIPLOAD: MetricGuide = MetricGuide {
    what: "How far each cutting edge moves into the wood per turn: \
           feed / (RPM x flutes).",
    too_low: "The edge rubs instead of cutting. Heat builds. You see burn \
              marks, glazed walls and fine dust instead of chips, and the \
              tool dulls fast.",
    too_high: "The edge takes too big a bite. You hear chatter or a \
               labouring spindle. You see torn grain or a rough wall, or \
               the tool breaks.",
    levers: "Raise the feed or lower the RPM to raise it. Lower the feed or \
             raise the RPM to lower it. Keep it in the band: to reduce the \
             load, reduce the depth or the width of cut, not the feed alone.",
};

const DEPTH_OF_CUT: MetricGuide = MetricGuide {
    what: "How deep the tool is in the wood at this moment.",
    too_low: "Only a time cost: the job needs more passes.",
    too_high: "The tool and the gantry flex. You hear a deeper tone, and you \
               see steps or a tapered wall. This limit is a rule of thumb \
               for this machine, so it does not stop an export.",
    levers: "Reduce the step-down, or use a stiffer tool (shorter, or a \
             larger diameter).",
};

const DEFLECTION: MetricGuide = MetricGuide {
    what: "How far the tool tip bends away under the cutting force.",
    too_low: "No risk.",
    too_high: "The wall is out of size, and the finish shows ridges or \
               chatter marks. Near the limit the tool can break.",
    levers: "Reduce the stickout, reduce the step-over or the depth, or use \
             a larger diameter.",
};

const POWER: MetricGuide = MetricGuide {
    what: "The power the cut takes from the spindle.",
    too_low: "No risk.",
    too_high: "The spindle slows under load. You hear the tone drop. The \
               chipload then rises, and burns can follow.",
    levers: "Reduce the depth or the step-over first. Keep the chipload in \
             the band: do not reduce the feed alone.",
};

const ENGAGEMENT: MetricGuide = MetricGuide {
    what: "How much of the tool's circle is in the wood.",
    too_low: "Most of the time is an air cut or a light skim.",
    too_high: "Full-slot cuts: more heat, poor chip clearing and more \
               deflection.",
    levers: "Adaptive or trochoidal clearing keeps the engagement even.",
};

impl CriterionKind {
    /// The (i) hover copy for this criterion's card. `None` for a kind
    /// that has no card: gantry push and the three drill gates.
    #[must_use]
    pub fn guide(self) -> Option<MetricGuide> {
        match self {
            CriterionKind::Chipload => Some(CHIPLOAD),
            CriterionKind::Power => Some(POWER),
            CriterionKind::Deflection => Some(DEFLECTION),
            CriterionKind::DepthOfCut => Some(DEPTH_OF_CUT),
            CriterionKind::GantryPush
            | CriterionKind::DrillChipWelding
            | CriterionKind::DrillPeckAdequacy
            | CriterionKind::DrillPlungeFeed => None,
        }
    }
}

/// The (i) hover copy for the engagement card, which has no criterion.
#[must_use]
pub fn engagement_guide() -> MetricGuide {
    ENGAGEMENT
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

    #[test]
    fn every_banded_card_has_a_guide_and_no_other_kind_does() {
        for kind in [
            CriterionKind::Chipload,
            CriterionKind::Power,
            CriterionKind::Deflection,
            CriterionKind::DepthOfCut,
        ] {
            let g = kind.guide().unwrap();
            for line in [g.what, g.too_low, g.too_high, g.levers] {
                assert!(!line.trim().is_empty(), "{kind:?} has an empty line");
            }
        }
        for kind in [
            CriterionKind::GantryPush,
            CriterionKind::DrillChipWelding,
            CriterionKind::DrillPeckAdequacy,
            CriterionKind::DrillPlungeFeed,
        ] {
            assert!(kind.guide().is_none(), "{kind:?} must not get copy");
        }
        assert!(!engagement_guide().what.is_empty());
    }

    /// Ruling R4: the load levers name engagement, and do not offer the
    /// feed alone as the way to reduce load.
    #[test]
    fn load_levers_follow_ruling_r4() {
        for kind in [CriterionKind::Chipload, CriterionKind::Power] {
            let levers = kind.guide().unwrap().levers;
            assert!(levers.contains("depth"), "{kind:?}: {levers}");
            assert!(levers.contains("not"), "{kind:?}: {levers}");
            assert!(levers.contains("feed alone"), "{kind:?}: {levers}");
        }
    }
}
