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

impl std::fmt::Display for Vendor {
    /// Human-readable vendor name for UI surfaces (R6: display is the
    /// only stringly edge — identity stays on the enum).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Amana => "Amana",
            Self::Onsrud => "Onsrud",
            Self::Harvey => "Harvey",
            Self::Whiteside => "Whiteside",
            Self::Sandvik => "Sandvik",
            Self::Garr => "GARR",
            Self::Autodesk => "Autodesk",
            Self::Carbide3d => "Carbide 3D",
            Self::Helical => "Helical Solutions",
            Self::Freud => "Freud",
            Self::Idcwoodcraft => "IDC Woodcraft",
        };
        f.write_str(s)
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    (
        "amana_flat_end.json",
        include_str!("../../data/vendor_lut/observations/amana_flat_end.json"),
    ),
    (
        "amana_ball_nose.json",
        include_str!("../../data/vendor_lut/observations/amana_ball_nose.json"),
    ),
    (
        "amana_3d_profiling.json",
        include_str!("../../data/vendor_lut/observations/amana_3d_profiling.json"),
    ),
    (
        "amana_vbit.json",
        include_str!("../../data/vendor_lut/observations/amana_vbit.json"),
    ),
    (
        "amana_vgroove_engraving.json",
        include_str!("../../data/vendor_lut/observations/amana_vgroove_engraving.json"),
    ),
    (
        "amana_compression.json",
        include_str!("../../data/vendor_lut/observations/amana_compression.json"),
    ),
    (
        "amana_facing.json",
        include_str!("../../data/vendor_lut/observations/amana_facing.json"),
    ),
    // 2026-05-30 ingest round (Phase 1C): non-wood rows from the
    // 2026-05-29 staging — wired live now that Material variants
    // (per-family plastics + Aluminum) and Vendor::Helical exist.
    (
        "amana_plastic_oflute.json",
        include_str!("../../data/vendor_lut/observations/amana_plastic_oflute.json"),
    ),
    (
        "amana_zrn_aluminum.json",
        include_str!("../../data/vendor_lut/observations/amana_zrn_aluminum.json"),
    ),
    (
        "amana_vgroove_aluminum_acrylic.json",
        include_str!("../../data/vendor_lut/observations/amana_vgroove_aluminum_acrylic.json"),
    ),
    (
        "onsrud_plastic.json",
        include_str!("../../data/vendor_lut/observations/onsrud_plastic.json"),
    ),
    (
        "whiteside_rpm_assorted.json",
        include_str!("../../data/vendor_lut/observations/whiteside_rpm_assorted.json"),
    ),
    (
        "helical_aluminum.json",
        include_str!("../../data/vendor_lut/observations/helical_aluminum.json"),
    ),
    // 2026-05-30 ingest round (Phase 4): bulk LUT row promotion of the
    // staged data collected by the Phase 3 agent fleet. See
    // planning/feeds_data_ingest_phase4_2026-05-31.md for per-source
    // counts and the Freud industrial namespacing decision.
    (
        "amana_long_tail.json",
        include_str!("../../data/vendor_lut/observations/amana_long_tail.json"),
    ),
    (
        "onsrud_ocr.json",
        include_str!("../../data/vendor_lut/observations/onsrud_ocr.json"),
    ),
    (
        "whiteside_fusion360.json",
        include_str!("../../data/vendor_lut/observations/whiteside_fusion360.json"),
    ),
    (
        "freud_solid_carbide.json",
        include_str!("../../data/vendor_lut/observations/freud_solid_carbide.json"),
    ),
    (
        "idcwoodcraft_millmage.json",
        include_str!("../../data/vendor_lut/observations/idcwoodcraft_millmage.json"),
    ),
    // 2026-05-31 (Phase B of the completion plan): Garr Aluminum
    // Milling Guide rows, split per-series from the staged 2026-05-29
    // file. Low-Range page (3 chart entries × 242M-2f / 842M-2f / A3-3f
    // = 9 rows) + High-Range A3-only page (HEM + finish = 2 rows) = 11
    // rows. Mid-Range 142M/143M rows and General-Purpose rows were
    // skipped — see planning/feeds_data_ingest_phaseB_2026-05-31.md
    // for the row-count decomposition and skip rationale.
    (
        "garr_aluminum.json",
        include_str!("../../data/vendor_lut/observations/garr_aluminum.json"),
    ),
    // 2026-06-01 (Phase 5 Step 5.3): Garr GP composite (non-ISO) row —
    // fiberglass / G10 / plastics consolidated under
    // material_family=fiberglass. 1 row, 6 mm 2-flute, CPT 0.030–0.051
    // mm/tooth.
    (
        "garr_fiberglass.json",
        include_str!("../../data/vendor_lut/observations/garr_fiberglass.json"),
    ),
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
    /// F3.2 (defect class C1): rows must pass [`validate_observation`]
    /// — bad data must not enter the LUT wearing a "validated" badge.
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
                for obs in &file.observations {
                    if let Err(violation) = validate_observation(obs) {
                        return Err(format!(
                            "invalid observation in {}: {violation}",
                            p.display()
                        ));
                    }
                }
                count += file.observations.len();
                self.observations.extend(file.observations);
            }
        }
        Ok(count)
    }
}

// ── F3.2 row validation (defect class C1) ───────────────────────────────
//
// A1's audit found 62/252 embedded rows with degenerate min==max
// chipload "ranges" (CAM nominal presets encoded as ranges), grade-A
// badges on CAM tool-library presets, and RPM-only rows winning
// chipload queries. These rules reject the *classes*, not the
// instances: every newly ingested file must pass, and the
// `embedded_rows_pass_loader_validation` test holds the embedded set
// to the same bar (with a documented legacy allowlist that may only
// shrink).

/// Minimum chipload range width as a fraction of the midpoint. A
/// narrower published "range" is a single nominal value encoded as a
/// range — the burn floor and breakage ceiling it implies are
/// fabricated precision. Encode such sources with only
/// `chipload_max_mm_tooth` (nominal reference) instead.
pub const MIN_CHIPLOAD_RANGE_FRACTION: f64 = 0.05;

/// Plausibility ceiling for chipload as a fraction of cutter diameter.
/// The hottest legitimate vendor row (Onsrud industrial softwood chart,
/// 0.013 ipt = 0.3302 mm/tooth on a 1/8" compression spiral) sits at
/// 0.104×D; 0.12×D clears it while still rejecting unit mistakes —
/// inch-vs-mm (25.4×) and per-rev-vs-per-tooth (flute_count×) errors
/// land far past this band.
pub const MAX_CHIPLOAD_DIAMETER_FRACTION: f64 = 0.12;

/// Substrings identifying CAM tool-library presets. Such sources are
/// software defaults, not vendor cutting charts — they may not carry
/// `evidence_grade: a` (rule 3) because grade A means "vendor chart".
const CAM_PRESET_SOURCE_MARKERS: &[&str] = &["fusion360", ".tools", ".tool library"];

/// Validate one observation row against the F3.2 ingest rules.
/// Returns `Err(description)` naming the row and the violated rule.
pub fn validate_observation(obs: &VendorObservation) -> Result<(), String> {
    let id = &obs.observation_id;

    // Rule 1 — degenerate-range rejection.
    if let (Some(min), Some(max)) = (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
        let mid = (min + max) / 2.0;
        if mid > 0.0 && (max - min) < MIN_CHIPLOAD_RANGE_FRACTION * mid {
            return Err(format!(
                "{id}: chipload range [{min}, {max}] is degenerate (width < {:.0}% of \
                 midpoint) — a nominal preset encoded as a range; publish only \
                 chipload_max_mm_tooth instead",
                MIN_CHIPLOAD_RANGE_FRACTION * 100.0
            ));
        }
    }

    // Rule 2 — exact-kind rows must publish chipload data.
    if obs.row_kind == ObservationKind::Exact
        && obs.chipload_min_mm_tooth.is_none()
        && obs.chipload_max_mm_tooth.is_none()
    {
        return Err(format!(
            "{id}: row_kind 'exact' with no chipload bounds — RPM-only rows must be \
             'derived' or 'fallback' so they cannot outrank chipload-bearing rows"
        ));
    }

    // Rule 3 — grade-A provenance denylist for CAM presets.
    if obs.evidence_grade == EvidenceGrade::A {
        let hay = format!(
            "{} {} {}",
            obs.source_id.to_lowercase(),
            obs.source_title.to_lowercase(),
            obs.source_url.to_lowercase()
        );
        if let Some(marker) = CAM_PRESET_SOURCE_MARKERS.iter().find(|m| hay.contains(**m)) {
            return Err(format!(
                "{id}: evidence_grade 'a' on a CAM tool-library preset source \
                 (matched {marker:?}) — grade A is reserved for vendor cutting charts; \
                 use 'c' for software defaults"
            ));
        }
    }

    // Rule 4 — calibration completeness: paired/ordered numeric fields.
    if obs.hardness_kind.is_some() != obs.hardness_value.is_some() {
        return Err(format!(
            "{id}: hardness_kind and hardness_value must be present together"
        ));
    }
    for (label, lo, hi) in [
        (
            "chipload",
            obs.chipload_min_mm_tooth,
            obs.chipload_max_mm_tooth,
        ),
        ("rpm", obs.rpm_min, obs.rpm_max),
        ("ap", obs.ap_min_mm, obs.ap_max_mm),
        ("ae", obs.ae_min_mm, obs.ae_max_mm),
    ] {
        if let (Some(lo), Some(hi)) = (lo, hi)
            && lo > hi
        {
            return Err(format!("{id}: {label} min {lo} > max {hi}"));
        }
    }
    if obs
        .chipload_min_mm_tooth
        .or(obs.chipload_max_mm_tooth)
        .is_some_and(|v| v <= 0.0)
    {
        return Err(format!("{id}: chipload bounds must be positive"));
    }

    // Rule 5 — per-diameter plausibility band (anchored rows only).
    if let (Some(d), Some(max)) = (obs.diameter_mm, obs.chipload_max_mm_tooth)
        && d > 0.0
        && max > MAX_CHIPLOAD_DIAMETER_FRACTION * d
    {
        return Err(format!(
            "{id}: chipload_max {max} mm/tooth exceeds {MAX_CHIPLOAD_DIAMETER_FRACTION}×D \
             ({:.3} mm) for a {d} mm cutter — likely a unit error (inch-vs-mm or \
             per-rev vs per-tooth)",
            MAX_CHIPLOAD_DIAMETER_FRACTION * d
        ));
    }

    Ok(())
}

/// Rule 6 — conflict detector across a row set: two same-vendor rows
/// for the identical query tuple whose full chipload ranges do not
/// overlap describe contradictory physics; one of them is wrong.
pub fn detect_conflicting_rows(observations: &[VendorObservation]) -> Vec<String> {
    use std::collections::HashMap;
    let mut by_key: HashMap<String, Vec<&VendorObservation>> = HashMap::new();
    for obs in observations {
        let (Some(min), Some(max)) = (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) else {
            continue;
        };
        if min >= max {
            continue;
        }
        let key = format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{:?}",
            obs.source_vendor,
            obs.tool_family,
            obs.tool_subfamily,
            obs.material_family,
            obs.operation_family,
            obs.pass_role,
            obs.flute_count,
            obs.diameter_mm,
        );
        by_key.entry(key).or_default().push(obs);
    }
    let mut conflicts = Vec::new();
    for rows in by_key.values() {
        for (i, a) in rows.iter().enumerate() {
            for b in rows.iter().skip(i + 1) {
                // SAFETY: filtered to rows with both bounds above.
                #[allow(clippy::unwrap_used)]
                let (a_min, a_max, b_min, b_max) = (
                    a.chipload_min_mm_tooth.unwrap(),
                    a.chipload_max_mm_tooth.unwrap(),
                    b.chipload_min_mm_tooth.unwrap(),
                    b.chipload_max_mm_tooth.unwrap(),
                );
                if a_max < b_min || b_max < a_min {
                    conflicts.push(format!(
                        "{} [{a_min}, {a_max}] vs {} [{b_min}, {b_max}]: same query tuple, \
                         disjoint chipload ranges",
                        a.observation_id, b.observation_id
                    ));
                }
            }
        }
    }
    conflicts.sort();
    conflicts
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
            serde_json::from_str::<ObservationFile>(json)
                .unwrap_or_else(|e| panic!("strict parse failed for {name}: {e}"));
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

    /// F3.2 — legacy rows that predate the loader-validation rules and
    /// still violate rule 1 (degenerate single-point "range"; the A1
    /// MAJOR class: Spektra single-points, compression presets,
    /// community CSV points). This list may only SHRINK — fix a row by
    /// dropping its fabricated `chipload_min_mm_tooth` (keep the value
    /// as a nominal `max` reference) and remove it here. New ingests
    /// must pass with no additions.
    const LEGACY_DEGENERATE_RANGE_ROWS: &[&str] = &[
        "amana-compression-acrylic-pocket-6350-2f",
        "amana-compression-mdf-pocket-12700-2f",
        "amana-compression-mdf-pocket-6350-2f",
        "amana-compression-plywood-hardwood-pocket-6350-2f",
        "amana-compression-wood-pocket-12700-2f",
        "amana-compression-wood-pocket-6350-2f",
        "amana-flat-mdf-pocket-0794-2f-spektra",
        "amana-flat-mdf-pocket-12000-2f-spektra",
        "amana-flat-mdf-pocket-12700-2f-spektra",
        "amana-flat-mdf-pocket-12700-3f-spektra",
        "amana-flat-mdf-pocket-1500-2f-spektra",
        "amana-flat-mdf-pocket-1587-2f-spektra",
        "amana-flat-mdf-pocket-19050-3f-spektra",
        "amana-flat-mdf-pocket-2381-2f-spektra",
        "amana-flat-mdf-pocket-3000-2f-spektra",
        "amana-flat-mdf-pocket-4763-2f-spektra",
        "amana-flat-mdf-pocket-5000-2f-spektra",
        "amana-flat-mdf-pocket-9525-2f-spektra",
        "amana-flat-mdf-pocket-9525-3f-spektra",
        "amana-flat-softwood-pocket-0794-2f-spektra",
        "amana-flat-softwood-pocket-12000-2f-spektra",
        "amana-flat-softwood-pocket-12700-2f-spektra",
        "amana-flat-softwood-pocket-12700-3f-spektra",
        "amana-flat-softwood-pocket-1500-2f-spektra",
        "amana-flat-softwood-pocket-1587-2f-spektra",
        "amana-flat-softwood-pocket-19050-3f-spektra",
        "amana-flat-softwood-pocket-2381-2f-spektra",
        "amana-flat-softwood-pocket-3000-2f-spektra",
        "amana-flat-softwood-pocket-4763-2f-spektra",
        "amana-flat-softwood-pocket-5000-2f-spektra",
        "amana-flat-softwood-pocket-9525-2f-spektra",
        "amana-flat-softwood-pocket-9525-3f-spektra",
        "amana-vgroove-hardwood-trace-60deg-2f",
        "amana-vgroove-softwood-trace-60deg-2f",
        "amana-vgroove-softwood-trace-90deg-2f",
        "garr-a3-alum-finish-6000-flat-3f",
        "helical-h45al-6061-hem-adaptive-12700-3f",
        "helical-h45al-6061-trad-rough-12700-3f",
        "idcwoodcraft-bn-12-ball-nose",
        "idcwoodcraft-bn-14-ball-nose",
        "idcwoodcraft-bn-18-ball-nose",
        "idcwoodcraft-cm-14-compression",
        "idcwoodcraft-cm-18-compression",
        "idcwoodcraft-dc-18-downcut",
        "idcwoodcraft-of-14-acrylic-o-flute",
        "idcwoodcraft-of-18-acrylic-o-flute",
        "idcwoodcraft-su-10-surfacing",
        "idcwoodcraft-uc-18-upcut",
    ];

    /// F3.2 — every embedded row passes the loader-validation rules,
    /// modulo the shrink-only legacy allowlist above. Strict equality
    /// both ways: a NEW violation fails loud, and a FIXED row left on
    /// the allowlist fails loud too (so the list can't go stale).
    #[test]
    fn embedded_rows_pass_loader_validation() {
        let lut = VendorLut::embedded();
        let mut violating: Vec<&str> = Vec::new();
        for obs in &lut.observations {
            if let Err(violation) = validate_observation(obs) {
                if !LEGACY_DEGENERATE_RANGE_ROWS.contains(&obs.observation_id.as_str()) {
                    panic!("non-allowlisted loader-validation violation: {violation}");
                }
                violating.push(obs.observation_id.as_str());
            }
        }
        for legacy in LEGACY_DEGENERATE_RANGE_ROWS {
            assert!(
                violating.contains(legacy),
                "{legacy} no longer violates any rule — remove it from \
                 LEGACY_DEGENERATE_RANGE_ROWS (the list may only shrink)"
            );
        }
    }

    /// F3.2 rule 6 — no two same-vendor rows for the identical query
    /// tuple publish disjoint chipload ranges.
    #[test]
    fn embedded_rows_have_no_conflicting_ranges() {
        let lut = VendorLut::embedded();
        let conflicts = detect_conflicting_rows(&lut.observations);
        assert!(conflicts.is_empty(), "conflicting rows:\n{conflicts:#?}");
    }

    /// F3.2 rule 7 — reachability: every row's `operation_family` must
    /// be declared as `feeds_family` by at least one registered
    /// operation, otherwise the row is dead data no query can ever
    /// select. The facing-bit rows are the documented exception (A1):
    /// the Face op declares `feeds_family: Pocket`, so the 9
    /// `operation_family: face` rows are unreachable until that
    /// mapping decision is revisited (backlog).
    #[test]
    fn embedded_rows_reachable_by_some_operation_family() {
        use crate::compute::catalog::OperationType;
        use crate::feeds::vendor_normalize::op_family_to_lut;

        let reachable: std::collections::HashSet<LutOperationFamily> = OperationType::ALL
            .iter()
            .map(|op| op_family_to_lut(op.registry_entry().spec.feeds_family))
            .collect();
        let lut = VendorLut::embedded();
        for obs in &lut.observations {
            if obs.operation_family == LutOperationFamily::Face {
                continue; // documented dead data — see doc comment.
            }
            assert!(
                reachable.contains(&obs.operation_family),
                "{}: operation_family {:?} is not the feeds_family of any registered \
                 operation — the row can never be selected",
                obs.observation_id,
                obs.operation_family
            );
        }
    }

    /// R-17 (2026-08-04) — the INVERSE of the rule above, which was
    /// one-directional: it asserts every *row* is queryable, and
    /// nothing asserted that every *queryable family* has rows.
    ///
    /// Drill is the silent inverse case. `LutOperationFamily::Drill`
    /// exists in the schema, `op_family_to_lut` maps the Drill and
    /// AlignmentPinDrill operations onto it, and
    /// `vendor_lookup::passes_must_match` returns `false` immediately on
    /// family mismatch — so every drill query is a guaranteed miss, and
    /// has been since the family was added. Nothing anywhere said so.
    /// Every drill number in this engine is a hardcoded material
    /// constant or the `k0·D^p·(1/H)^q × DRILL_CHIPLOAD_MULTIPLIER`
    /// formula, and softening the LUT diameter/hardness exponents
    /// cannot move any of them.
    ///
    /// This test does not fail on an empty family — authoring drill LUT
    /// rows is not in scope and inventing them would be worse than
    /// having none. It fails when the *documented* set of empty
    /// families stops matching reality, in either direction: a family
    /// that quietly empties out, or drill rows arriving without this
    /// note being updated.
    #[test]
    fn queryable_families_without_rows_are_a_stated_fact() {
        use crate::compute::catalog::OperationType;
        use crate::feeds::vendor_normalize::op_family_to_lut;

        /// Queryable families with zero bundled rows, as of 2026-08-04.
        /// Every query for one of these is a guaranteed miss that falls
        /// back to the formula path.
        const KNOWN_EMPTY: &[LutOperationFamily] = &[LutOperationFamily::Drill];

        let queryable: std::collections::HashSet<LutOperationFamily> = OperationType::ALL
            .iter()
            .map(|op| op_family_to_lut(op.registry_entry().spec.feeds_family))
            .collect();
        let lut = VendorLut::embedded();
        let mut actual: Vec<String> = queryable
            .into_iter()
            .filter(|fam| {
                !lut.observations
                    .iter()
                    .any(|obs| obs.operation_family == *fam)
            })
            .map(|fam| format!("{fam:?}"))
            .collect();
        actual.sort();
        let mut expected: Vec<String> = KNOWN_EMPTY.iter().map(|f| format!("{f:?}")).collect();
        expected.sort();
        assert_eq!(
            actual, expected,
            "the set of queryable LUT families with zero rows changed. Every \
             family listed here is a guaranteed lookup miss for every query \
             that names it, which is a fact about what this engine can and \
             cannot source from vendor data — say so deliberately rather than \
             letting it drift. Update KNOWN_EMPTY and the doc comment together."
        );
    }

    fn synthetic_row() -> VendorObservation {
        // SAFETY: parse of a known-good literal.
        #[allow(clippy::expect_used)]
        serde_json::from_str(
            r#"{
                "observation_id": "synthetic-test-row",
                "source_id": "synthetic",
                "source_vendor": "amana",
                "source_title": "Synthetic Vendor Chart",
                "source_url": "https://example.com/chart.pdf",
                "accessed_on": "2026-06-10",
                "evidence_grade": "a",
                "row_kind": "exact",
                "tool_family": "flat_end",
                "operation_family": "pocket",
                "pass_role": "roughing",
                "material_family": "hardwood",
                "material_label": "synthetic",
                "diameter_mm": 6.0,
                "flute_count": 2,
                "chipload_min_mm_tooth": 0.04,
                "chipload_max_mm_tooth": 0.08
            }"#,
        )
        .expect("synthetic row parses")
    }

    #[test]
    fn validate_observation_rules_fire() {
        // Baseline passes.
        assert!(validate_observation(&synthetic_row()).is_ok());

        // Rule 1 — degenerate range.
        let mut r = synthetic_row();
        r.chipload_min_mm_tooth = Some(0.0508);
        r.chipload_max_mm_tooth = Some(0.0508);
        assert!(validate_observation(&r).unwrap_err().contains("degenerate"));

        // Rule 2 — exact kind without chipload.
        let mut r = synthetic_row();
        r.chipload_min_mm_tooth = None;
        r.chipload_max_mm_tooth = None;
        assert!(validate_observation(&r).unwrap_err().contains("RPM-only"));
        r.row_kind = ObservationKind::Derived;
        assert!(validate_observation(&r).is_ok(), "derived RPM-only is fine");

        // Rule 3 — grade-A CAM preset.
        let mut r = synthetic_row();
        r.source_title = "Vendor Fusion360 .tool Library".to_owned();
        assert!(
            validate_observation(&r)
                .unwrap_err()
                .contains("CAM tool-library preset")
        );
        r.evidence_grade = EvidenceGrade::C;
        assert!(validate_observation(&r).is_ok(), "grade-c preset is fine");

        // Rule 4 — pairing / ordering.
        let mut r = synthetic_row();
        r.hardness_kind = Some(HardnessKind::Janka);
        r.hardness_value = None;
        assert!(
            validate_observation(&r)
                .unwrap_err()
                .contains("hardness_kind and hardness_value")
        );
        let mut r = synthetic_row();
        r.rpm_min = Some(24000.0);
        r.rpm_max = Some(18000.0);
        assert!(validate_observation(&r).unwrap_err().contains("rpm min"));

        // Rule 5 — implausible chipload for the diameter.
        let mut r = synthetic_row();
        r.chipload_max_mm_tooth = Some(2.54); // 0.1" — inch-vs-mm error
        assert!(validate_observation(&r).unwrap_err().contains("unit error"));
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
