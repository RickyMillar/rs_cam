//! Per-export overrides surfaced from the wizard.
//!
//! The wizard collects a small set of "override the post's defaults
//! for this one job" fields (WCS, units, safe-Z, spindle warmup).
//! Rather than thread four optional parameters through every emit
//! function, the wizard packs them into a `WizardOverlay` and the
//! overlay-aware emit entry points apply them in one place:
//!
//! - `wcs_override` / `units_override` mutate a per-export clone of the
//!   `PostDefinition` so the preamble templates render the chosen
//!   word (G54..G59 / G21|G20).
//! - `safe_z_override` is consumed by the viz export helpers when
//!   building the multi-setup program (the only emitter path that
//!   currently writes Z retracts between setups).
//! - `spindle_warmup_secs` injects a `G4 P{secs}` dwell immediately
//!   after the program preamble so the spindle has time to come up
//!   to speed before the first cut.
//!
//! `WizardOverlay::default()` is a zero-effect overlay: every field
//! `None` / `0`. Default-overlay export is byte-identical to the
//! pre-overlay path — the captured-fixture baseline guards this.

use std::borrow::Cow;

use super::ir::{Program, Statement};
use super::post::{PostDefinition, Units, WcsCode};

/// Per-export overrides collected by the export wizard.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WizardOverlay {
    pub wcs_override: Option<WcsCode>,
    pub units_override: Option<Units>,
    pub safe_z_override: Option<f64>,
    pub spindle_warmup_secs: u32,
}

impl WizardOverlay {
    /// True if every field is at its no-effect default. Used by the
    /// emitter to skip cloning the `PostDefinition` and `Program` when
    /// nothing is overridden.
    pub fn is_empty(&self) -> bool {
        self.wcs_override.is_none()
            && self.units_override.is_none()
            && self.safe_z_override.is_none()
            && self.spindle_warmup_secs == 0
    }

    /// Apply the post-affecting overrides (`wcs_override`, `units_override`)
    /// to `base`. Returns `Cow::Borrowed(base)` if neither field is set,
    /// avoiding an allocation on the default path.
    ///
    /// `safe_z_override` and `spindle_warmup_secs` are NOT applied here —
    /// they're program-level concerns handled by the export helpers and
    /// `apply_to_program`.
    pub fn applied_post<'a>(&self, base: &'a PostDefinition) -> Cow<'a, PostDefinition> {
        if self.wcs_override.is_none() && self.units_override.is_none() {
            return Cow::Borrowed(base);
        }
        let mut p = base.clone();
        if let Some(w) = self.wcs_override {
            p.wcs = Some(w);
        }
        if let Some(u) = self.units_override {
            p.units = u;
        }
        Cow::Owned(p)
    }

    /// Inject the warmup dwell statement into `program` after its first
    /// `Preamble`. Returns `Cow::Borrowed(program)` when no warmup is
    /// configured. The preamble is the first statement built by every
    /// `program_builder` entry point, so the dwell lands immediately
    /// after the spindle-start `M3 S<rpm>` the preamble emits.
    pub fn apply_to_program<'a>(&self, program: &'a Program) -> Cow<'a, Program> {
        if self.spindle_warmup_secs == 0 {
            return Cow::Borrowed(program);
        }
        let mut p = program.clone();
        if let Some(idx) = p
            .statements
            .iter()
            .position(|s| matches!(s, Statement::Preamble { .. }))
        {
            let dwell = Statement::Raw(format!("G4 P{}\n", self.spindle_warmup_secs));
            p.statements.insert(idx + 1, dwell);
        }
        Cow::Owned(p)
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
    use crate::gcode::post;

    #[test]
    fn default_overlay_is_empty() {
        let o = WizardOverlay::default();
        assert!(o.is_empty());
    }

    #[test]
    fn applied_post_borrows_when_no_post_overrides() {
        let o = WizardOverlay {
            wcs_override: None,
            units_override: None,
            safe_z_override: Some(20.0),
            spindle_warmup_secs: 3,
        };
        let base = post::grbl();
        let cow = o.applied_post(base);
        assert!(matches!(cow, Cow::Borrowed(_)));
    }

    #[test]
    fn applied_post_owns_when_wcs_set() {
        let o = WizardOverlay {
            wcs_override: Some(WcsCode::G55),
            ..Default::default()
        };
        let base = post::grbl();
        let cow = o.applied_post(base);
        assert!(matches!(cow, Cow::Owned(_)));
        assert_eq!(cow.wcs, Some(WcsCode::G55));
    }

    #[test]
    fn applied_post_owns_when_units_set() {
        let o = WizardOverlay {
            units_override: Some(Units::Inch),
            ..Default::default()
        };
        let base = post::linuxcnc();
        let cow = o.applied_post(base);
        assert!(matches!(cow, Cow::Owned(_)));
        assert_eq!(cow.units, Units::Inch);
    }

    #[test]
    fn warmup_zero_borrows_program() {
        let o = WizardOverlay::default();
        let prog = Program {
            statements: vec![Statement::Preamble { spindle_rpm: 18_000 }],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        assert!(matches!(cow, Cow::Borrowed(_)));
    }

    #[test]
    fn warmup_inserts_dwell_after_preamble() {
        let o = WizardOverlay {
            spindle_warmup_secs: 5,
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Preamble { spindle_rpm: 18_000 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        let stmts = &cow.statements;
        assert_eq!(stmts.len(), 3);
        assert!(matches!(stmts[0], Statement::Preamble { .. }));
        match &stmts[1] {
            Statement::Raw(s) => assert_eq!(s, "G4 P5\n"),
            other => panic!("expected G4 dwell, got {other:?}"),
        }
        assert!(matches!(stmts[2], Statement::Postamble));
    }

    #[test]
    fn warmup_no_op_when_no_preamble() {
        // Defensive: an empty program shouldn't crash; just leave it alone.
        let o = WizardOverlay {
            spindle_warmup_secs: 5,
            ..Default::default()
        };
        let prog = Program::new();
        let cow = o.apply_to_program(&prog);
        assert!(cow.statements.is_empty());
    }
}
