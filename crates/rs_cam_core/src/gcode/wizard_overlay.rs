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
    /// When `Some`, dry-run mode is enabled and every cutting move's Z
    /// is clamped to this value so the spindle stays in air for the
    /// entire program. Rapids and `SafeZRetract` statements are left
    /// alone (they already operate above material).
    ///
    /// The viz layer resolves this from `wizard.dry_run` plus the
    /// effective safe-Z (`wizard.safe_z_override.unwrap_or(gui.post.safe_z)`)
    /// before calling the emit helpers.
    pub dry_run_safe_z: Option<f64>,
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
            && self.dry_run_safe_z.is_none()
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

    /// Apply program-level transforms (warmup-dwell injection + dry-run
    /// Z clamp). Returns `Cow::Borrowed(program)` when neither transform
    /// is active, avoiding allocation on the default path.
    ///
    /// **Warmup**: when `spindle_warmup_secs > 0`, inserts a
    /// `G4 P{secs}` `Statement::Raw` immediately after the program's
    /// first `Preamble` so the spindle has time to come up to speed
    /// before the first cut.
    ///
    /// **Dry-run**: when `dry_run_safe_z = Some(z)`, replaces the Z
    /// component of every cutting move (`Linear`, `LinearModal`,
    /// `ArcCw`, `ArcCcw`) with `z`. `Rapid` and `SafeZRetract` are
    /// left untouched — those already operate above material and
    /// changing them would break the entry/exit kinematics.
    pub fn apply_to_program<'a>(&self, program: &'a Program) -> Cow<'a, Program> {
        let needs_warmup = self.spindle_warmup_secs > 0;
        let needs_dry_run = self.dry_run_safe_z.is_some();
        if !needs_warmup && !needs_dry_run {
            return Cow::Borrowed(program);
        }
        let mut p = program.clone();
        if needs_warmup
            && let Some(idx) = p
                .statements
                .iter()
                .position(|s| matches!(s, Statement::Preamble { .. }))
        {
            let dwell = Statement::Raw(format!("G4 P{}\n", self.spindle_warmup_secs));
            p.statements.insert(idx + 1, dwell);
        }
        if let Some(safe_z) = self.dry_run_safe_z {
            for s in &mut p.statements {
                clamp_cutting_z(s, safe_z);
            }
        }
        Cow::Owned(p)
    }
}

/// Clamp the Z component of cutting-move statements to `safe_z`.
/// Rapids, retracts, and non-move statements are left alone.
fn clamp_cutting_z(s: &mut Statement, safe_z: f64) {
    match s {
        Statement::Linear { z, .. }
        | Statement::LinearModal { z, .. }
        | Statement::ArcCw { z, .. }
        | Statement::ArcCcw { z, .. } => {
            *z = safe_z;
        }
        Statement::Preamble { .. }
        | Statement::SpindleSet { .. }
        | Statement::Postamble
        | Statement::ProgramPause { .. }
        | Statement::Comment(_)
        | Statement::Raw(_)
        | Statement::Rapid { .. }
        | Statement::SafeZRetract { .. } => {}
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
            dry_run_safe_z: None,
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
    fn dry_run_clamps_cutting_z_only() {
        let o = WizardOverlay {
            dry_run_safe_z: Some(7.5),
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Preamble { spindle_rpm: 18_000 },
                Statement::Rapid { x: 0.0, y: 0.0, z: 5.0 },
                Statement::Linear { x: 1.0, y: 0.0, z: -2.0, feed: 600.0 },
                Statement::LinearModal { x: 2.0, y: 0.0, z: -2.0 },
                Statement::ArcCw { x: 3.0, y: 0.0, z: -2.5, i: 1.0, j: 0.0, feed: 600.0 },
                Statement::ArcCcw { x: 4.0, y: 0.0, z: -2.5, i: -1.0, j: 0.0, feed: 600.0 },
                Statement::SafeZRetract { z: 10.0 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        let stmts = &cow.statements;
        // Rapid kept at original Z (entry/exit kinematics intact).
        match stmts[1] {
            Statement::Rapid { z, .. } => assert!((z - 5.0).abs() < 1e-9),
            ref other => panic!("expected Rapid, got {other:?}"),
        }
        // Cutting moves all clamped to 7.5.
        for idx in [2, 3, 4, 5] {
            let z = match stmts[idx] {
                Statement::Linear { z, .. }
                | Statement::LinearModal { z, .. }
                | Statement::ArcCw { z, .. }
                | Statement::ArcCcw { z, .. } => z,
                ref other => panic!("stmt {idx}: expected cutting move, got {other:?}"),
            };
            assert!(
                (z - 7.5).abs() < 1e-9,
                "stmt {idx}: dry-run should clamp Z to 7.5, got {z}"
            );
        }
        // SafeZRetract left alone (already above material).
        match stmts[6] {
            Statement::SafeZRetract { z } => assert!((z - 10.0).abs() < 1e-9),
            ref other => panic!("expected SafeZRetract, got {other:?}"),
        }
    }

    #[test]
    fn dry_run_none_borrows_program() {
        let o = WizardOverlay {
            dry_run_safe_z: None,
            ..Default::default()
        };
        let prog = Program {
            statements: vec![Statement::Linear { x: 0.0, y: 0.0, z: -1.0, feed: 600.0 }],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        assert!(matches!(cow, Cow::Borrowed(_)));
    }

    #[test]
    fn dry_run_and_warmup_compose() {
        // Both transforms active: warmup dwell after preamble + Z clamp.
        let o = WizardOverlay {
            dry_run_safe_z: Some(3.0),
            spindle_warmup_secs: 4,
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Preamble { spindle_rpm: 18_000 },
                Statement::Linear { x: 0.0, y: 0.0, z: -1.0, feed: 600.0 },
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        assert_eq!(cow.statements.len(), 3);
        match &cow.statements[1] {
            Statement::Raw(s) => assert_eq!(s, "G4 P4\n"),
            other => panic!("expected dwell, got {other:?}"),
        }
        match cow.statements[2] {
            Statement::Linear { z, .. } => assert!((z - 3.0).abs() < 1e-9),
            ref other => panic!("expected clamped Linear, got {other:?}"),
        }
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
