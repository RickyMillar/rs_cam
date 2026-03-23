//! Vendor LUT types and JSON loading for chipload observations.
//!
//! Loads production-tested vendor data (Amana, Onsrud, Whiteside, etc.) from JSON.
//! Embedded Amana data is compiled in via include_str!.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tool vendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    Amana,
    Onsrud,
    Harvey,
    Whiteside,
    Sandvik,
    Garr,
    Autodesk,
    #[serde(rename = "carbide3d")]
    Carbide3d,
}

/// Evidence quality grade: A = vendor chart, B = derived, C = community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGrade {
    A,
    B,
    C,
}

impl EvidenceGrade {
    pub fn score(self) -> i64 {
        match self {
            EvidenceGrade::A => 60,
            EvidenceGrade::B => 30,
            EvidenceGrade::C => 10,
        }
    }
}

/// How the observation was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Exact,
    Derived,
    Fallback,
}

impl ObservationKind {
    pub fn score(self) -> i64 {
        match self {
            ObservationKind::Exact => 120,
            ObservationKind::Derived => 70,
            ObservationKind::Fallback => 30,
        }
    }
}

/// Tool family as classified in vendor data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolFamily {
    FlatEnd,
    BallNose,
    TaperedBallNose,
    BullNose,
    ChamferVbit,
    FacingBit,
}

/// Material family as classified in vendor data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialFamily {
    Softwood,
    Hardwood,
    PlywoodSoftwood,
    PlywoodHardwood,
    Mdf,
    Hdf,
    Particleboard,
    Acrylic,
    Hdpe,
    Polycarbonate,
    Delrin,
    Aluminum,
}

/// Hardness measurement kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardnessKind {
    Janka,
    Hb,
    ShoreD,
}

/// A single vendor observation row.
#[derive(Debug, Clone, Deserialize)]
pub struct VendorObservation {
    pub observation_id: String,
    pub source_id: String,
    pub source_vendor: Vendor,
    #[allow(dead_code)]
    pub source_title: String,
    #[allow(dead_code)]
    pub source_url: String,
    #[allow(dead_code)]
    pub accessed_on: String,
    pub evidence_grade: EvidenceGrade,
    pub row_kind: ObservationKind,
    pub tool_family: ToolFamily,
    pub tool_subfamily: Option<String>,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
    pub material_family: MaterialFamily,
    #[allow(dead_code)]
    pub material_label: String,
    pub hardness_kind: Option<HardnessKind>,
    pub hardness_value: Option<f64>,
    pub diameter_mm: f64,
    pub flute_count: u32,
    pub rpm_min: Option<f64>,
    pub rpm_max: Option<f64>,
    pub rpm_nominal: Option<f64>,
    pub chipload_min_mm_tooth: Option<f64>,
    pub chipload_max_mm_tooth: Option<f64>,
    pub ap_min_mm: Option<f64>,
    pub ap_max_mm: Option<f64>,
    pub ae_min_mm: Option<f64>,
    pub ae_max_mm: Option<f64>,
    #[allow(dead_code)]
    pub ap_rule: Option<String>,
    #[allow(dead_code)]
    pub ae_rule: Option<String>,
    #[allow(dead_code)]
    pub machine_assumption: Option<String>,
}

/// Operation family as used in vendor LUT JSON (separate from feeds::OperationFamily for serde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LutOperationFamily {
    Adaptive,
    Pocket,
    Contour,
    Parallel,
    Scallop,
    Trace,
    Face,
}

/// Pass role as used in vendor LUT JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LutPassRole {
    Roughing,
    SemiFinish,
    Finish,
}

/// JSON wrapper for observation arrays.
#[derive(Deserialize)]
struct ObservationFile {
    observations: Vec<VendorObservation>,
}

/// Collection of vendor observations.
pub struct VendorLut {
    pub observations: Vec<VendorObservation>,
}

impl VendorLut {
    /// Load from embedded Amana data (compile-time).
    pub fn embedded() -> Self {
        let files: &[&str] = &[
            include_str!("../../data/vendor_lut/observations/amana_flat_end.json"),
            include_str!("../../data/vendor_lut/observations/amana_ball_nose.json"),
            include_str!("../../data/vendor_lut/observations/amana_3d_profiling.json"),
            include_str!("../../data/vendor_lut/observations/amana_vbit.json"),
            include_str!("../../data/vendor_lut/observations/amana_facing.json"),
        ];

        let mut observations = Vec::new();
        for json in files {
            if let Ok(file) = serde_json::from_str::<ObservationFile>(json) {
                observations.extend(file.observations);
            }
        }
        VendorLut { observations }
    }

    /// Load additional observations from a directory of JSON files.
    pub fn load_dir(&mut self, path: &Path) -> Result<usize, String> {
        let entries = std::fs::read_dir(path)
            .map_err(|e| format!("cannot read directory {}: {e}", path.display()))?;
        let mut count = 0;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                let contents = std::fs::read_to_string(&p)
                    .map_err(|e| format!("cannot read {}: {e}", p.display()))?;
                let file: ObservationFile = serde_json::from_str(&contents)
                    .map_err(|e| format!("parse error in {}: {e}", p.display()))?;
                count += file.observations.len();
                self.observations.extend(file.observations);
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_loads_all_observations() {
        let lut = VendorLut::embedded();
        assert_eq!(
            lut.observations.len(),
            61,
            "expected 61 embedded observations"
        );
    }

    #[test]
    fn test_embedded_has_all_tool_families() {
        let lut = VendorLut::embedded();
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::FlatEnd)
        );
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::BallNose)
        );
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::TaperedBallNose)
        );
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::BullNose)
        );
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::ChamferVbit)
        );
        assert!(
            lut.observations
                .iter()
                .any(|o| o.tool_family == ToolFamily::FacingBit)
        );
    }

    #[test]
    fn test_observation_chipload_ranges_valid() {
        let lut = VendorLut::embedded();
        for obs in &lut.observations {
            if let (Some(min), Some(max)) = (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
                assert!(
                    max >= min,
                    "{}: chipload max {max} < min {min}",
                    obs.observation_id
                );
                assert!(
                    min > 0.0,
                    "{}: chipload min must be positive",
                    obs.observation_id
                );
            }
        }
    }

    #[test]
    fn test_amana_flat_6mm_softwood_chipload() {
        let lut = VendorLut::embedded();
        let obs = lut
            .observations
            .iter()
            .find(|o| o.observation_id == "amana-flat-softwood-adaptive-6000-2f")
            .expect("should find amana 6mm flat softwood adaptive");
        assert_eq!(obs.diameter_mm, 6.0);
        assert_eq!(obs.flute_count, 2);
        assert!(
            (obs.chipload_min_mm_tooth
                .expect("observation should have min chipload")
                - 0.065)
                .abs()
                < 0.001
        );
        assert!(
            (obs.chipload_max_mm_tooth
                .expect("observation should have max chipload")
                - 0.11)
                .abs()
                < 0.001
        );
    }
}
