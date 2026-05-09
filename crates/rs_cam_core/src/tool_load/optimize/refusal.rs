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

/// Build the user-facing prescription string for a deflection-setup-
/// locked refusal. Reports the predicted peak tip deflection (µm) and
/// names the setup levers the optimizer's search space can't reach
/// (stickout, tool material). The target stickout is derived by
/// scaling the current cantilever length so that, for a uniform-cylinder
/// approximation, peak δ would land at the Within bound (50 µm) — i.e.
/// `target_L = current_L × (50 µm / peak)^(1/3)`.
pub(crate) fn deflection_setup_prescription(
    tool: &crate::tool::ToolDefinition,
    peak_delta_mm: f64,
) -> String {
    let peak_um = peak_delta_mm * 1000.0;
    let target_um = search_policy().deflection_setup_target_um.value;
    let scale = if peak_um > target_um {
        (target_um / peak_um).cbrt()
    } else {
        1.0
    };
    let target_stickout_mm = (tool.stickout * scale).max(0.0);
    format!(
        "predicted tip deflection {peak_um:.0} µm at peak load (above 200 µm limit) — \
         feed/RPM/DOC/stepover alone can't bring this under threshold for this setup; \
         shorten stickout below ~{target_stickout_mm:.0} mm or use a stiffer tool/material"
    )
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
    };
    format!(
        "steady-state chipload samples straddle the LUT chipload range \
         (some below the burn floor, some above the breakage ceiling) — \
         no single feed/RPM clears both extremes. {lever}."
    )
}
