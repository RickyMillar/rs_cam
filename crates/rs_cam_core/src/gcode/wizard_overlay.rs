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

/// Per-export tool-change handling override. `None` on the overlay
/// means "use the post's `tool_change` template as-is".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolChangeMode {
    /// Manual change: spindle off, operator message, M0 pause. The
    /// `SpindleSet` emitted after the change block spins back up on
    /// resume. (The GRBL-family posts' default.)
    Pause,
    /// Native `M5` + `M6 T{n}` change. (The LinuxCNC/Mach3 default.)
    M6,
    /// Suppress the post's tool-change block. Each `ToolChange`
    /// statement is replaced by a bare `ProgramPause` (M5 + operator
    /// message + M0) so a multi-tool program still STOPS at every
    /// change point instead of flowing into the next op with the wrong
    /// tool (A10); the following `SpindleSet` is kept so per-tool RPM
    /// still applies. Single-tool programs have no `ToolChange`
    /// statements and strip clean. For operators who handle changes
    /// outside the program.
    Suppress,
}

impl ToolChangeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause for manual change (M0)",
            Self::M6 => "M6 tool change",
            Self::Suppress => "Suppress tool changes",
        }
    }

    /// The `tool_change` template implementing this mode (for the
    /// template-overriding modes; `Suppress` is handled at the program
    /// level and has no template).
    fn template(self) -> Option<&'static str> {
        match self {
            Self::Pause => Some("M5\n{message_comment}\nM0\n"),
            Self::M6 => Some("M5\nM6 T{tool_number}\n"),
            Self::Suppress => None,
        }
    }
}

/// Per-export overrides collected by the export wizard.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WizardOverlay {
    pub wcs_override: Option<WcsCode>,
    pub units_override: Option<Units>,
    pub safe_z_override: Option<f64>,
    pub spindle_warmup_secs: u32,
    /// When `Some`, dry-run mode is enabled and every cutting move's Z
    /// is replaced by this value so the spindle stays in air for the
    /// entire program. `Rapid` and `SafeZRetract` Z values are clamped
    /// up to this floor too (C3, 2026-06-11): in a dry run the material
    /// is never removed, so e.g. drill peck re-entry rapids — which
    /// target just above the previous peck bottom — would otherwise
    /// drive G0 moves into solid stock.
    ///
    /// The viz layer resolves this from `wizard.dry_run` plus the
    /// effective safe-Z (`wizard.safe_z_override.unwrap_or(gui.post.safe_z)`)
    /// before calling the emit helpers.
    pub dry_run_safe_z: Option<f64>,
    /// Tool-change handling override. `None` = post default.
    /// `Pause`/`M6` replace the post's `tool_change` template;
    /// `Suppress` replaces `ToolChange` statements with bare M0 pauses
    /// (keeping the per-tool `SpindleSet`) — see [`ToolChangeMode`].
    pub tool_change_override: Option<ToolChangeMode>,
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
            && self.tool_change_override.is_none()
    }

    /// Apply the post-affecting overrides (`wcs_override`,
    /// `units_override`, and the template-overriding tool-change modes)
    /// to `base`. Returns `Cow::Borrowed(base)` if no post-level field
    /// is set, avoiding an allocation on the default path.
    ///
    /// `safe_z_override` and `spindle_warmup_secs` are NOT applied here —
    /// they're program-level concerns handled by the export helpers and
    /// `apply_to_program` (as is `ToolChangeMode::Suppress`).
    pub fn applied_post<'a>(&self, base: &'a PostDefinition) -> Cow<'a, PostDefinition> {
        let tool_change_template = self.tool_change_override.and_then(ToolChangeMode::template);
        if self.wcs_override.is_none()
            && self.units_override.is_none()
            && tool_change_template.is_none()
        {
            return Cow::Borrowed(base);
        }
        let mut p = base.clone();
        if let Some(w) = self.wcs_override {
            p.wcs = Some(w);
        }
        if let Some(u) = self.units_override {
            p.units = u;
        }
        if let Some(t) = tool_change_template {
            p.tool_change = t.to_owned();
        }
        Cow::Owned(p)
    }

    /// Apply program-level transforms (warmup-dwell injection + dry-run
    /// Z clamp). Returns `Cow::Borrowed(program)` when neither transform
    /// is active, avoiding allocation on the default path.
    ///
    /// **Warmup**: when `spindle_warmup_secs > 0`, inserts a
    /// `G4 P{secs}` `Statement::Raw` immediately after the program's
    /// first `Preamble`, AND after every `SpindleSet` that resumes from
    /// a `ToolChange` or `ProgramPause` (A4, 2026-06-11) — a spindle
    /// restarted after a manual change / M0 needs the same spin-up time
    /// as the first start.
    ///
    /// **Dry-run**: when `dry_run_safe_z = Some(z)`, replaces the Z
    /// component of every cutting move (`Linear`, `LinearModal`,
    /// `ArcCw`, `ArcCcw`) with `z`, and clamps `Rapid` / `SafeZRetract`
    /// Z up to `max(z_original, z)` (C3) — dry runs never remove
    /// material, so rapids that re-enter previously "cut" pockets would
    /// otherwise drive into solid stock.
    pub fn apply_to_program<'a>(&self, program: &'a Program) -> Cow<'a, Program> {
        let needs_warmup = self.spindle_warmup_secs > 0;
        let needs_dry_run = self.dry_run_safe_z.is_some();
        let needs_tc_suppress = self.tool_change_override == Some(ToolChangeMode::Suppress);
        if !needs_warmup && !needs_dry_run && !needs_tc_suppress {
            return Cow::Borrowed(program);
        }
        let mut p = program.clone();
        if needs_tc_suppress {
            // A10 — replace the change block with a bare M0 pause so a
            // multi-tool program still stops at every change point; the
            // SpindleSet that the builder emits right after each
            // ToolChange stays, so the incoming tool's RPM still
            // applies. Programs with a single tool have no ToolChange
            // statements, so they strip clean.
            for s in &mut p.statements {
                if let Statement::ToolChange { tool_number, label } = s {
                    *s = Statement::ProgramPause {
                        message: format!(
                            "Tool change suppressed — verify correct tool: {label} [T{tool_number}]"
                        ),
                    };
                }
            }
        }
        if needs_warmup {
            let dwell_text = format!("G4 P{}\n", self.spindle_warmup_secs);
            let mut out: Vec<Statement> = Vec::with_capacity(p.statements.len() + 2);
            let mut pending_resume = false;
            for s in p.statements {
                let after_preamble = matches!(s, Statement::Preamble { .. });
                let starts_resume = matches!(
                    s,
                    Statement::ToolChange { .. } | Statement::ProgramPause { .. }
                );
                let is_spindle_set = matches!(s, Statement::SpindleSet { .. });
                out.push(s);
                if after_preamble {
                    out.push(Statement::Raw(dwell_text.clone()));
                }
                if starts_resume {
                    pending_resume = true;
                }
                if is_spindle_set && pending_resume {
                    out.push(Statement::Raw(dwell_text.clone()));
                    pending_resume = false;
                }
            }
            p.statements = out;
        }
        if let Some(safe_z) = self.dry_run_safe_z {
            for s in &mut p.statements {
                clamp_dry_run_z(s, safe_z);
            }
        }
        Cow::Owned(p)
    }
}

/// Dry-run Z transform: cutting moves are pinned to `safe_z`; rapids
/// and retracts are clamped *up* to it (`max`) so re-entry rapids never
/// descend below the dry-run floor (C3). Non-move statements are left
/// alone.
fn clamp_dry_run_z(s: &mut Statement, safe_z: f64) {
    match s {
        Statement::Linear { z, .. }
        | Statement::LinearModal { z, .. }
        | Statement::ArcCw { z, .. }
        | Statement::ArcCcw { z, .. } => {
            *z = safe_z;
        }
        Statement::Rapid { z, .. } | Statement::SafeZRetract { z } => {
            *z = z.max(safe_z);
        }
        Statement::Preamble { .. }
        | Statement::SpindleSet { .. }
        | Statement::Postamble
        | Statement::ProgramPause { .. }
        | Statement::Comment(_)
        | Statement::ToolChange { .. }
        | Statement::Raw(_) => {}
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
            tool_change_override: None,
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
            statements: vec![Statement::Preamble {
                spindle_rpm: 18_000,
            }],
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
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
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
    fn dry_run_clamps_cutting_z_and_rapid_floor() {
        let o = WizardOverlay {
            dry_run_safe_z: Some(7.5),
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0,
                },
                Statement::Linear {
                    x: 1.0,
                    y: 0.0,
                    z: -2.0,
                    feed: 600.0,
                },
                Statement::LinearModal {
                    x: 2.0,
                    y: 0.0,
                    z: -2.0,
                },
                Statement::ArcCw {
                    x: 3.0,
                    y: 0.0,
                    z: -2.5,
                    i: 1.0,
                    j: 0.0,
                    feed: 600.0,
                },
                Statement::ArcCcw {
                    x: 4.0,
                    y: 0.0,
                    z: -2.5,
                    i: -1.0,
                    j: 0.0,
                    feed: 600.0,
                },
                Statement::SafeZRetract { z: 10.0 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        let stmts = &cow.statements;
        // C3: a rapid BELOW the dry-run floor is clamped up to it — the
        // material is never removed in a dry run, so a sub-floor G0
        // (e.g. drill peck re-entry) would drive into solid stock.
        match stmts[1] {
            Statement::Rapid { z, .. } => assert!(
                (z - 7.5).abs() < 1e-9,
                "rapid below the dry-run floor must clamp up to it, got {z}"
            ),
            ref other => panic!("expected Rapid, got {other:?}"),
        }
        // Cutting moves all pinned to 7.5.
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
        // SafeZRetract above the floor is left alone (max-clamp no-op).
        match stmts[6] {
            Statement::SafeZRetract { z } => assert!((z - 10.0).abs() < 1e-9),
            ref other => panic!("expected SafeZRetract, got {other:?}"),
        }
    }

    /// C3 regression: a program whose rapids descend below the dry-run
    /// floor (drill peck re-entry shape) comes out with every Rapid and
    /// SafeZRetract at or above the floor.
    #[test]
    fn dry_run_clamps_sub_floor_rapids_and_retracts() {
        let o = WizardOverlay {
            dry_run_safe_z: Some(6.0),
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0,
                },
                // peck 1 bottom
                Statement::Linear {
                    x: 0.0,
                    y: 0.0,
                    z: -3.0,
                    feed: 200.0,
                },
                // retract + re-entry rapid just above the peck bottom —
                // in dry-run this is solid stock.
                Statement::SafeZRetract { z: 2.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: -2.5,
                },
                Statement::Linear {
                    x: 0.0,
                    y: 0.0,
                    z: -6.0,
                    feed: 200.0,
                },
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        for (idx, s) in cow.statements.iter().enumerate() {
            match *s {
                Statement::Rapid { z, .. } | Statement::SafeZRetract { z } => {
                    assert!(
                        z >= 6.0 - 1e-9,
                        "stmt {idx}: rapid/retract Z must be >= dry-run floor, got {z}"
                    );
                }
                Statement::Linear { z, .. } => {
                    assert!((z - 6.0).abs() < 1e-9, "stmt {idx}: cut pinned to floor");
                }
                _ => {}
            }
        }
    }

    /// A4: with warmup configured, the same G4 dwell that follows the
    /// preamble is emitted after every SpindleSet that resumes from a
    /// ToolChange or ProgramPause.
    #[test]
    fn warmup_dwell_after_tool_change_and_pause_resume() {
        let o = WizardOverlay {
            spindle_warmup_secs: 5,
            ..Default::default()
        };
        let prog = Program {
            statements: vec![
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::Linear {
                    x: 1.0,
                    y: 0.0,
                    z: -1.0,
                    feed: 600.0,
                },
                Statement::ToolChange {
                    tool_number: 2,
                    label: "Ball".to_owned(),
                },
                Statement::SpindleSet { rpm: 10_610 },
                Statement::ProgramPause {
                    message: "Setup change: Bottom".to_owned(),
                },
                Statement::SpindleSet { rpm: 18_000 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        let dwell = |s: &Statement| matches!(s, Statement::Raw(t) if t == "G4 P5\n");
        let dwell_count = cow.statements.iter().filter(|s| dwell(s)).count();
        assert_eq!(
            dwell_count, 3,
            "one dwell after preamble + one after each resume SpindleSet: {:?}",
            cow.statements
        );
        // Each dwell directly follows its trigger.
        let positions: Vec<usize> = cow
            .statements
            .iter()
            .enumerate()
            .filter_map(|(i, s)| dwell(s).then_some(i))
            .collect();
        assert!(matches!(
            cow.statements[positions[0] - 1],
            Statement::Preamble { .. }
        ));
        assert!(matches!(
            cow.statements[positions[1] - 1],
            Statement::SpindleSet { rpm: 10_610 }
        ));
        assert!(matches!(
            cow.statements[positions[2] - 1],
            Statement::SpindleSet { rpm: 18_000 }
        ));
        // A mid-program SpindleSet NOT preceded by a change/pause gets
        // no dwell: rerun with such a program.
        let prog2 = Program {
            statements: vec![
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::SpindleSet { rpm: 24_000 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow2 = o.apply_to_program(&prog2);
        assert_eq!(
            cow2.statements.iter().filter(|s| dwell(s)).count(),
            1,
            "plain RPM change must not dwell: {:?}",
            cow2.statements
        );
    }

    #[test]
    fn dry_run_none_borrows_program() {
        let o = WizardOverlay {
            dry_run_safe_z: None,
            ..Default::default()
        };
        let prog = Program {
            statements: vec![Statement::Linear {
                x: 0.0,
                y: 0.0,
                z: -1.0,
                feed: 600.0,
            }],
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
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::Linear {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                    feed: 600.0,
                },
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
    fn tool_change_pause_override_swaps_template() {
        let o = WizardOverlay {
            tool_change_override: Some(ToolChangeMode::Pause),
            ..Default::default()
        };
        // LinuxCNC ships the M6 template; the Pause override replaces it.
        let cow = o.applied_post(post::linuxcnc());
        assert!(matches!(cow, Cow::Owned(_)));
        assert_eq!(cow.tool_change, "M5\n{message_comment}\nM0\n");
        assert_eq!(
            cow.render_tool_change(2, "Ball"),
            "M5\n(TOOL CHANGE: Ball [T2])\nM0\n"
        );
    }

    #[test]
    fn tool_change_m6_override_swaps_template() {
        let o = WizardOverlay {
            tool_change_override: Some(ToolChangeMode::M6),
            ..Default::default()
        };
        // Grbl ships the pause template; the M6 override replaces it
        // (e.g. for a grblHAL ATC build using the GRBL post).
        let cow = o.applied_post(post::grbl());
        assert!(matches!(cow, Cow::Owned(_)));
        assert_eq!(cow.render_tool_change(3, "Bit"), "M5\nM6 T3\n");
    }

    #[test]
    fn tool_change_suppress_keeps_pause_and_spindle() {
        let o = WizardOverlay {
            tool_change_override: Some(ToolChangeMode::Suppress),
            ..Default::default()
        };
        // Suppress does NOT touch the post (template-level Cow stays
        // borrowed)…
        assert!(matches!(o.applied_post(post::grbl()), Cow::Borrowed(_)));
        // …but replaces ToolChange statements with a bare M0 pause
        // (A10 — a multi-tool program must still stop at every change
        // point), keeping the SpindleSet that follows.
        let prog = Program {
            statements: vec![
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::ToolChange {
                    tool_number: 2,
                    label: "Ball".to_owned(),
                },
                Statement::SpindleSet { rpm: 10_610 },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&prog);
        assert_eq!(cow.statements.len(), 4);
        assert!(
            !cow.statements
                .iter()
                .any(|s| matches!(s, Statement::ToolChange { .. })),
            "ToolChange must be replaced"
        );
        let pause = cow.statements.iter().find_map(|s| match s {
            Statement::ProgramPause { message } => Some(message.as_str()),
            _ => None,
        });
        let pause = pause.expect("suppressed change must leave an M0 pause");
        assert!(
            pause.contains("Tool change suppressed") && pause.contains("[T2]"),
            "pause names the incoming tool: {pause}"
        );
        assert!(
            cow.statements
                .iter()
                .any(|s| matches!(s, Statement::SpindleSet { rpm: 10_610 })),
            "SpindleSet must survive suppression"
        );

        // Single-tool program: no ToolChange statements → strips clean
        // (no pauses added).
        let single = Program {
            statements: vec![
                Statement::Preamble {
                    spindle_rpm: 18_000,
                },
                Statement::Postamble,
            ],
            ..Default::default()
        };
        let cow = o.apply_to_program(&single);
        assert!(
            !cow.statements
                .iter()
                .any(|s| matches!(s, Statement::ProgramPause { .. })),
            "single-tool program must not gain pauses"
        );
    }

    #[test]
    fn tool_change_override_makes_overlay_non_empty() {
        for mode in [
            ToolChangeMode::Pause,
            ToolChangeMode::M6,
            ToolChangeMode::Suppress,
        ] {
            let o = WizardOverlay {
                tool_change_override: Some(mode),
                ..Default::default()
            };
            assert!(!o.is_empty(), "{mode:?} should make overlay non-empty");
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
