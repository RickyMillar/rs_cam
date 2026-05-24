//! Evidence payloads attached to [`super::Diagnostic`] values.
//!
//! Each variant carries the citation the operator needs to chase a
//! finding back to its source: a sim sample range, a vendor LUT row,
//! a geometric inequality, or a specific move/position. The variants
//! are intentionally distinct rather than a single bag of optional
//! fields — adapters pick the one that matches the upstream source,
//! and consumers branch on the variant to render.

use serde::{Deserialize, Serialize};

/// Locality hint for sim-derived findings — passed through from the
/// load-report evidence so the operator can interpret peak events
/// ("arc-fit overshoot", "heavy engagement").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLocality(pub String);

impl EvidenceLocality {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Where the finding came from. Tagged so consumers can branch on
/// `kind` without exhausting all variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DiagnosticEvidence {
    /// A sim-trace sample range that triggered or motivated the
    /// finding. `observed`/`threshold` are in `unit` (e.g.
    /// "mm/tooth", "kW", "mm").
    SampleRange {
        toolpath_id: usize,
        /// Project-global sample indices (matching
        /// [`crate::simulation_cut::SimulationCutSample`]'s ordering).
        sample_start: usize,
        sample_end: usize,
        observed: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        threshold: Option<f64>,
        unit: String,
        #[serde(default, skip_serializing_if = "EvidenceLocality::is_empty")]
        locality: EvidenceLocality,
    },
    /// Geometric inequality from config values alone (no sim
    /// required). E.g. `stepover (3.5 mm) > tool diameter (3.0 mm)`.
    GeometryCompare {
        lhs_label: String,
        lhs_value: f64,
        rhs_label: String,
        rhs_value: f64,
        unit: String,
    },
    /// Vendor LUT row citation — bounds from the row and the
    /// observed peak. `extrapolated` flags when the bounds were
    /// scaled from a calibrated diameter via the LUT extrapolator.
    LutCitation {
        row_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        observed: f64,
        unit: String,
        #[serde(default)]
        extrapolated: bool,
    },
    /// A single move pointer (collision, generated-empty, plunge
    /// stress). `position` is the global-frame XYZ when available.
    Move {
        toolpath_id: usize,
        move_index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<[f64; 3]>,
    },
    /// Aggregate count signal — used by project-wide verdicts that
    /// span multiple toolpaths.
    Counts {
        count: usize,
        offender_toolpath_ids: Vec<usize>,
    },
}
