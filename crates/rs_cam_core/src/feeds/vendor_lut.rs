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
    /// Helical Solutions — added 2026-05-30 for 6061-T6 endmill chipload
    /// rows from `harvey_helical_garr.json`. Step 1D of the
    /// `feeds_data_ingest_2026-05-30_phased_plan` ingest.
    Helical,
    /// Freud — added 2026-05-31 (Phase 4 prerequisite) for the
    /// Solid Carbide router-bit chart staged in
    /// `planning/data_ingest_2026-05-30/vendor_breadth.json` (14
    /// rows: 1/8"–1/2" across hardwood / softwood / MDF / particle /
    /// plywood / acrylic / aluminum). Grade A.
    Freud,
    /// IDC Woodcraft community-aggregated CSV (Millmage database).
    /// Grade C — not a tool manufacturer, but a curated cross-vendor
    /// dataset used for sanity-checking the manufacturer rows.
    /// Added 2026-05-31 (Phase 4 prerequisite) for the 10 staged
    /// rows in `vendor_breadth.json`. JSON tag: `idcwoodcraft`.
    #[serde(rename = "idcwoodcraft")]
    Idcwoodcraft,
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
    /// Fiber-reinforced composites — G10/FR4 glass-epoxy, generic
    /// glass-resin laminates. Added Phase 5 Step 5.3 (2026-06-01) to
    /// unlock the staged Garr GP-plastics row (which collapsed
    /// "Fiberglass/Plastics/G10" under one vendor chart entry). These
    /// materials are abrasive (carbide tool wear is faster than for
    /// wood / acrylic) and fiber-reinforced (delamination risk on
    /// uncoated cutters). Vendor chiploads tend to track wood/acrylic
    /// magnitudes (0.03–0.05 mm/tooth at 6 mm) but the tool-life
    /// envelope is much tighter — treat as its own LUT category.
    Fiberglass,
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
    /// Calibrated cutter diameter (mm) for the vendor row. `None` for
    /// rows whose source publishes a diameter-independent chipload
    /// envelope — e.g. v-bit/chamfer charts (where engaged diameter is
    /// a function of depth) and material-routing articles that quote
    /// one chipload window across a polymer/composite line. None-rows
    /// are matched by `(tool_family, included_angle_deg)` for v-bits
    /// or by `(tool_family, material_family, op_family)` for window
    /// rows; the matcher's diameter-scaling pipeline is skipped and
    /// chipload bounds carry through unscaled.
    #[serde(default)]
    pub diameter_mm: Option<f64>,
    /// V-bit / chamfer-bit included (full) angle in degrees. Lets the
    /// chipload lookup match a V-groove row to the cutter's actual cone
    /// angle rather than its nominal diameter. `None` for non-cone rows
    /// and older data that predates the field.
    #[serde(default)]
    pub included_angle_deg: Option<f64>,
    /// Flat-tip diameter (mm) of a truncated-tip V-bit / engraving bit.
    /// `None` for pointed bits and rows that don't record it. Data slot
    /// for incoming flat-tip datasets; not yet consumed by matching.
    #[serde(default)]
    #[allow(dead_code)]
    pub tip_diameter_mm: Option<f64>,
    pub flute_count: u32,
    pub rpm_min: Option<f64>,
    pub rpm_max: Option<f64>,
    pub rpm_nominal: Option<f64>,
    pub chipload_min_mm_tooth: Option<f64>,
    pub chipload_max_mm_tooth: Option<f64>,
    pub ap_min_mm: Option<f64>,
    pub ap_max_mm: Option<f64>,
    /// Diameter-scaling lower bound for axial DOC: `ap_min = factor × diameter`.
    /// Optional so existing JSON parses unchanged; the cutter-axial-constraints
    /// calculator combines this with `ap_min_mm` per the
    /// `min(factor × diameter, absolute_mm)` rule. Populated by the
    /// `migrate_ap_rule` binary from the row's `ap_rule` prose.
    #[serde(default)]
    pub ap_min_factor: Option<f64>,
    /// Diameter-scaling upper bound for axial DOC: `ap_max = factor × diameter`.
    /// See `ap_min_factor`.
    #[serde(default)]
    pub ap_max_factor: Option<f64>,
    pub ae_min_mm: Option<f64>,
    pub ae_max_mm: Option<f64>,
    #[allow(dead_code)]
    pub ap_rule: Option<String>,
    #[allow(dead_code)]
    pub ae_rule: Option<String>,
    #[allow(dead_code)]
    pub machine_assumption: Option<String>,
    /// Page number or row label in the source PDF for audit traceability.
    /// Optional; older observations did not record this and may be backfilled.
    #[serde(default)]
    #[allow(dead_code)]
    pub source_page: Option<String>,
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
    /// Drill / peck cycles. Mirrors `OperationFamily::Drill`; LUT
    /// rows tagged `drill` apply to peck cycles. Pre-2026-06-02 the
    /// drill ops mapped to `Pocket` for LUT queries, producing
    /// milling-style chipload at milling RPM (audit finding —
    /// "Nominal-D leakage" and "Drill ops have no dedicated family
    /// branch").
    Drill,
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

/// Compile-time list of `(filename, json_contents)` tuples for the
/// embedded vendor LUT. Shared between the runtime loader and the
/// strict-parse test so a schema-violating file can't silently drop
/// rows from the LUT — production parsing stays best-effort (forward
/// compat for partial schema changes) and the test fails loud with
/// the offending filename.
///
/// Add new sources here; keep `industrial_only/` files OUT — they
/// document data we deliberately don't load.
const EMBEDDED_FILES: &[(&str, &str)] = &[
    ("amana_flat_end.json",
        include_str!("../../data/vendor_lut/observations/amana_flat_end.json")),
    ("amana_ball_nose.json",
        include_str!("../../data/vendor_lut/observations/amana_ball_nose.json")),
    ("amana_3d_profiling.json",
        include_str!("../../data/vendor_lut/observations/amana_3d_profiling.json")),
    ("amana_vbit.json",
        include_str!("../../data/vendor_lut/observations/amana_vbit.json")),
    ("amana_vgroove_engraving.json",
        include_str!("../../data/vendor_lut/observations/amana_vgroove_engraving.json")),
    ("amana_compression.json",
        include_str!("../../data/vendor_lut/observations/amana_compression.json")),
    ("amana_facing.json",
        include_str!("../../data/vendor_lut/observations/amana_facing.json")),
    // 2026-05-30 ingest round (Phase 1C): non-wood rows from the
    // 2026-05-29 staging — wired live now that Material variants
    // (per-family plastics + Aluminum) and Vendor::Helical exist.
    ("amana_plastic_oflute.json",
        include_str!("../../data/vendor_lut/observations/amana_plastic_oflute.json")),
    ("amana_zrn_aluminum.json",
        include_str!("../../data/vendor_lut/observations/amana_zrn_aluminum.json")),
    ("amana_vgroove_aluminum_acrylic.json",
        include_str!("../../data/vendor_lut/observations/amana_vgroove_aluminum_acrylic.json")),
    ("onsrud_plastic.json",
        include_str!("../../data/vendor_lut/observations/onsrud_plastic.json")),
    ("whiteside_rpm_assorted.json",
        include_str!("../../data/vendor_lut/observations/whiteside_rpm_assorted.json")),
    ("helical_aluminum.json",
        include_str!("../../data/vendor_lut/observations/helical_aluminum.json")),
    // 2026-05-30 ingest round (Phase 4): bulk LUT row promotion of the
    // staged data collected by the Phase 3 agent fleet. See
    // planning/feeds_data_ingest_phase4_2026-05-31.md for per-source
    // counts and the Freud industrial namespacing decision.
    ("amana_long_tail.json",
        include_str!("../../data/vendor_lut/observations/amana_long_tail.json")),
    ("onsrud_ocr.json",
        include_str!("../../data/vendor_lut/observations/onsrud_ocr.json")),
    ("whiteside_fusion360.json",
        include_str!("../../data/vendor_lut/observations/whiteside_fusion360.json")),
    ("freud_solid_carbide.json",
        include_str!("../../data/vendor_lut/observations/freud_solid_carbide.json")),
    ("idcwoodcraft_millmage.json",
        include_str!("../../data/vendor_lut/observations/idcwoodcraft_millmage.json")),
    // 2026-05-31 (Phase B of the completion plan): Garr Aluminum
    // Milling Guide rows, split per-series from the staged 2026-05-29
    // file. Low-Range page (3 chart entries × 242M-2f / 842M-2f / A3-3f
    // = 9 rows) + High-Range A3-only page (HEM + finish = 2 rows) = 11
    // rows. Mid-Range 142M/143M rows and General-Purpose rows were
    // skipped — see planning/feeds_data_ingest_phaseB_2026-05-31.md
    // for the row-count decomposition and skip rationale.
    ("garr_aluminum.json",
        include_str!("../../data/vendor_lut/observations/garr_aluminum.json")),
    // 2026-06-01 (Phase 5 Step 5.3): Garr GP composite (non-ISO) row —
    // fiberglass / G10 / plastics consolidated under
    // material_family=fiberglass. 1 row, 6 mm 2-flute, CPT 0.030–0.051
    // mm/tooth.
    ("garr_fiberglass.json",
        include_str!("../../data/vendor_lut/observations/garr_fiberglass.json")),
];

impl VendorLut {
    /// Load from embedded vendor data (compile-time). Best-effort: a file
    /// that fails to deserialize is silently skipped so a partial schema
    /// change doesn't crash the runtime. The `test_embedded_strict_parse`
    /// test exercises the strict path and fails noisily on bad files —
    /// silent dropouts are a CI failure, not a runtime one.
    pub fn embedded() -> Self {
        let mut observations = Vec::new();
        for (_name, json) in EMBEDDED_FILES {
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

    /// Strict-parse every embedded file individually so a schema
    /// violation surfaces with the offending filename — `embedded()`
    /// is intentionally best-effort and would otherwise silently drop
    /// a broken file's rows (caught only by the count assertion).
    #[test]
    fn test_embedded_strict_parse() {
        for (name, json) in EMBEDDED_FILES {
            serde_json::from_str::<ObservationFile>(json).unwrap_or_else(|e| {
                panic!("strict parse failed for {name}: {e}")
            });
        }
    }

    #[test]
    fn test_embedded_loads_all_observations() {
        let lut = VendorLut::embedded();
        assert_eq!(
            lut.observations.len(),
            252,
            "expected 252 embedded observations (251 after Phase 5 Step 5.2 + \
             1 net-new Phase 5 Step 5.3 2026-06-01: Garr GP composite \
             (fiberglass / G10) 6 mm 2-flute row, material_family=fiberglass — \
             see planning/phase_5_schema_unlock_2026-06-01.md)"
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

    /// Phase 1 LUT migration invariant
    /// (`planning/cutter_axial_constraints_2026-06-06.md`).
    ///
    /// Every row that carries an `ap_rule` prose string must also have a
    /// structured axial-DOC bound — either `ap_min_factor` / `ap_max_factor`
    /// (proportional rule) or `ap_min_mm` / `ap_max_mm` (absolute cap) —
    /// EXCEPT for the documented label-only rules that the source doesn't
    /// quantify. The migration binary (`migrate_ap_rule` example) keeps
    /// these in sync; this test fires when a new ingest adds an `ap_rule`
    /// the binary's mapping doesn't recognise.
    const LABEL_ONLY_AP_RULES: &[&str] = &[
        "tip depth dependent",
        "3d profiling finish",
        "3d finishing",
        "light finishing",
        "3d profiling semi",
        "semi-finish",
        "Finishing Axial = Max LOC",
        // upcut O-flute single-window row — chip-evacuation prose only.
        "side-entry or ramp entry required (no straight plunge); upcut \
         O-flute for chip evacuation. Phase 5 promotion (2026-06-01) — \
         diameter_mm omitted; article publishes one chipload window \
         across the upcut O-flute line.",
    ];

    #[test]
    fn test_ap_rule_rows_have_structured_bound() {
        let lut = VendorLut::embedded();
        for obs in &lut.observations {
            let Some(rule) = &obs.ap_rule else { continue };
            let has_factor = obs.ap_min_factor.is_some() || obs.ap_max_factor.is_some();
            let has_absolute = obs.ap_min_mm.is_some() || obs.ap_max_mm.is_some();
            if has_factor || has_absolute {
                continue;
            }
            assert!(
                LABEL_ONLY_AP_RULES.contains(&rule.as_str()),
                "{}: ap_rule {rule:?} has no structured ap bound (factor or \
                 absolute mm) and is not in the documented label-only set — \
                 add an entry to `examples/migrate_ap_rule.rs` and re-run \
                 the migration, or extend `LABEL_ONLY_AP_RULES` if the rule \
                 is truly label-only",
                obs.observation_id
            );
        }
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
        assert_eq!(obs.diameter_mm, Some(6.0));
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
