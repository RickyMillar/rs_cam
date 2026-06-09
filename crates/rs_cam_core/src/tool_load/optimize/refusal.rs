//! User-facing prescription text builders surfaced by pre-flight
//! refusals (and by the U3 rollup's "skipped" rows).
//!
//! Each helper returns a free-form English string that the modal /
//! rollup renders verbatim. Keep these focused on the *lever* the
//! user has access to — what they can change to make the refusal go
//! away — rather than restating the diagnostic.

use crate::compute::catalog::OperationType;
use crate::feeds::OperationFamily;

use super::candidate::has_doc_knob;
use super::search_policy;

/// Structured prescription for a deflection-setup-locked refusal.
/// F2.3 — `peak_um` / `bound_um` / `target_stickout_mm` are structured
/// fields (mirrored onto
/// [`super::narrative::DeflectionSetupDetail`]) so MCP/GUI consumers
/// read values instead of parsing `text`.
pub(crate) struct DeflectionSetupPrescription {
    /// Predicted peak tip deflection at baseline load, µm.
    pub peak_um: f64,
    /// The Exceeds safety limit the peak breached, µm
    /// (`deflection::EXCEEDS_BOUND_MM`).
    pub bound_um: f64,
    /// Stickout estimated to land peak δ in the Within band, mm.
    pub target_stickout_mm: f64,
    /// Operator-facing prose rendered verbatim by the modal / rollup.
    pub text: String,
}

/// Build the prescription for a deflection-setup-locked refusal.
/// Reports the predicted peak tip deflection (µm) against the 200 µm
/// Exceeds limit and names the setup levers the optimizer's search
/// space can't reach (stickout, tool material). The target stickout is
/// derived by scaling the current cantilever length so that, for a
/// uniform-cylinder approximation, peak δ would land at the 50 µm
/// Within band — i.e. `target_L = current_L × (50 µm / peak)^(1/3)`.
/// (Pre-F2.3 the prose cited the 200 µm limit while the stickout math
/// silently targeted 50 µm; both numbers are now stated.)
pub(crate) fn deflection_setup_prescription(
    tool: &crate::tool::ToolDefinition,
    peak_delta_mm: f64,
) -> DeflectionSetupPrescription {
    let peak_um = peak_delta_mm * 1000.0;
    let bound_um = crate::tool_load::deflection::EXCEEDS_BOUND_MM * 1000.0;
    let target_um = search_policy().deflection_setup_target_um.value;
    let scale = if peak_um > target_um {
        (target_um / peak_um).cbrt()
    } else {
        1.0
    };
    let target_stickout_mm = (tool.stickout * scale).max(0.0);
    let text = format!(
        "predicted tip deflection {peak_um:.0} µm at peak load exceeds the {bound_um:.0} µm \
         safety limit even at the lightest reachable cut — feed/RPM/DOC/stepover can't fix \
         this setup; shorten stickout to ~{target_stickout_mm:.0} mm (sized for the \
         {target_um:.0} µm finish-quality band) or use a stiffer tool/material"
    );
    DeflectionSetupPrescription {
        peak_um,
        bound_um,
        target_stickout_mm,
        text,
    }
}

/// Build the user-facing prescription string for a bipolar-engagement
/// refusal. The lever depends on whether the operation has a
/// depth-per-pass knob the user can adjust to reduce engagement
/// variance: 2.5D ops with DOC/stepover can usually fix it; 3D
/// finishing ops typically can't and need a setup change.
pub(crate) fn bipolar_prescription(op_kind: OperationType, op_family: OperationFamily) -> String {
    // Family takes precedence over the DOC-knob check: Contour and
    // Trace are profile-follow ops whose engagement variance is driven
    // by part geometry (corners, curve changes), not by depth-per-pass.
    // Even though Profile (G1, 2026-05-08) now exposes a DOC knob to
    // Stage 1, raising DOC on a contour-follow doesn't reduce the
    // geometric variance that produced the bipolar verdict.
    let lever = match op_family {
        OperationFamily::Contour | OperationFamily::Trace => {
            "engagement variance is driven by the part geometry — break the operation into \
             multiple passes at fixed engagement, or use a smaller cutter"
        }
        _ if has_doc_knob(op_kind) => {
            "lower stepover or raise depth-per-pass to reduce engagement variance across the toolpath"
        }
        OperationFamily::Parallel | OperationFamily::Scallop => {
            "this is a 3D finishing op — reduce stepover for tighter passes, or shorten \
             the cutter to lower setup deflection"
        }
        OperationFamily::Face => {
            "engagement variance on a face op usually means the stock or stepover is \
             misaligned with the cutter footprint — adjust stepover or face the stock first"
        }
        // Adaptive / Pocket / Adaptive3d are all has_doc_knob — they hit the branch above.
        OperationFamily::Adaptive | OperationFamily::Pocket => {
            "lower stepover or raise depth-per-pass to reduce engagement variance"
        }
        OperationFamily::Drill => {
            "drill ops have no lateral engagement variance — bipolar verdicts here \
             indicate a tool/material mismatch (wrong tool for the hole, or material \
             not suited to drilling with this cutter)"
        }
    };
    format!(
        "steady-state chipload samples straddle the LUT chipload range \
         (some below the burn floor, some above the breakage ceiling) — \
         no single feed/RPM clears both extremes. {lever}."
    )
}
