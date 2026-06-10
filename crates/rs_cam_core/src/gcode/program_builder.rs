//! Build a `Program` (Statement IR) from `Toolpath` inputs.
//!
//! Three entry points mirror the legacy emitter modes:
//! - `build_single` — one toolpath (matches `emit_gcode`)
//! - `build_phased` — series of phases with shared preamble/postamble
//!   (matches `emit_gcode_phased`)
//! - `build_multi_setup` — multiple setup groups separated by M0 pauses
//!   (matches `emit_gcode_multi_setup`)
//!
//! Each function produces a `Program` whose `Statement`s, when fed to
//! `emit_program`, yield byte-identical output to the legacy direct
//! emission path. The byte-identical guarantee is verified by the
//! existing in-source tests and by the captured-fixtures baseline.

use super::ir::{Program, Statement};
use super::modal::ModalState;
use super::{ControllerCompensation, CoolantMode, GcodePhase, GcodeSetupPhase, PhaseTool};
use crate::toolpath::{MoveType, Toolpath};

/// Push a rapid move, splitting diagonals so the machine never
/// traverses unknown space on a combined `G0 X Y Z`:
///
/// - `prev_pos` is `None` (program start, or just after a tool change /
///   M0 pause where the operator may have jogged): retract Z-first to
///   the rapid's target height (safe by construction), then the full
///   move.
/// - Both XY and Z change, Z rising: lift to the target Z first, then
///   traverse.
/// - Both XY and Z change, Z falling: traverse at the current (higher)
///   Z first, then plunge straight down.
/// - Only-XY or only-Z change: a single statement, as before.
fn push_rapid(
    program: &mut Program,
    prev_pos: &mut Option<(f64, f64, f64)>,
    x: f64,
    y: f64,
    z: f64,
) {
    match *prev_pos {
        None => {
            program.push(Statement::SafeZRetract { z });
            program.push(Statement::Rapid { x, y, z });
        }
        Some((px, py, pz)) => {
            let xy_changed = x != px || y != py;
            let z_changed = z != pz;
            if xy_changed && z_changed {
                if z > pz {
                    // Rising: lift first, then traverse at the new height.
                    program.push(Statement::SafeZRetract { z });
                    program.push(Statement::Rapid { x, y, z });
                } else {
                    // Falling: traverse at the current height, then plunge.
                    program.push(Statement::Rapid { x, y, z: pz });
                    program.push(Statement::Rapid { x, y, z });
                }
            } else {
                program.push(Statement::Rapid { x, y, z });
            }
        }
    }
    *prev_pos = Some((x, y, z));
}

/// Build a `Program` for a single toolpath.
pub fn build_single(toolpath: &Toolpath, spindle_rpm: u32) -> Program {
    let mut program = Program::new();
    program.push(Statement::Preamble { spindle_rpm });

    let mut last_feed: Option<f64> = None;
    let mut prev_pos: Option<(f64, f64, f64)> = None;
    for m in &toolpath.moves {
        match m.move_type {
            MoveType::Rapid => {
                push_rapid(
                    &mut program,
                    &mut prev_pos,
                    m.target.x,
                    m.target.y,
                    m.target.z,
                );
                last_feed = None;
            }
            MoveType::Linear { feed_rate } => {
                if last_feed != Some(feed_rate) {
                    program.push(Statement::Linear {
                        x: m.target.x,
                        y: m.target.y,
                        z: m.target.z,
                        feed: feed_rate,
                    });
                    last_feed = Some(feed_rate);
                } else {
                    program.push(Statement::LinearModal {
                        x: m.target.x,
                        y: m.target.y,
                        z: m.target.z,
                    });
                }
            }
            MoveType::ArcCW { i, j, feed_rate } => {
                program.push(Statement::ArcCw {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                    i,
                    j,
                    feed: feed_rate,
                });
                last_feed = Some(feed_rate);
            }
            MoveType::ArcCCW { i, j, feed_rate } => {
                program.push(Statement::ArcCcw {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                    i,
                    j,
                    feed: feed_rate,
                });
                last_feed = Some(feed_rate);
            }
        }
        // Track the move's target so the next rapid can be sequenced
        // safely (push_rapid sets it for rapids; this covers feeds/arcs
        // and is a no-op re-assignment after a rapid).
        prev_pos = Some((m.target.x, m.target.y, m.target.z));
    }

    program.push(Statement::Postamble);
    program
}

/// Build a `Program` for a series of phases.
pub fn build_phased(phases: &[GcodePhase<'_>]) -> Program {
    let mut program = Program::new();
    if phases.is_empty() {
        return program;
    }
    // SAFETY: empty-check above guarantees first()/[0] is valid.
    let Some(first_phase) = phases.first() else {
        return program;
    };
    let first_rpm = first_phase.spindle_rpm;
    program.push(Statement::Preamble {
        spindle_rpm: first_rpm,
    });

    let first_coolant = first_phase.coolant;
    if first_coolant.is_active() {
        program.push(Statement::Raw(first_coolant.start_gcode().to_owned()));
    }

    let mut state = ModalState::new(first_rpm, first_phase.tool.map(|t| t.id), first_coolant);

    for (idx, phase) in phases.iter().enumerate() {
        program.push(Statement::Comment(phase.label.to_owned()));

        // Tool change: emit the post's tool-change block if the tool
        // *identity* (config id) changed (skip first phase). Display
        // T-numbers may collide across distinct tools — never key on them.
        if idx > 0
            && let Some(tool) = phase.tool
            && state.current_tool != Some(tool.id)
        {
            push_tool_change(&mut program, &mut state, tool, phase);
        }

        // Spindle speed change (only if we didn't already emit it in the tool change block).
        if phase.spindle_rpm != state.current_rpm {
            program.push(Statement::SpindleSet {
                rpm: phase.spindle_rpm,
            });
            state.current_rpm = phase.spindle_rpm;
        }

        // Coolant mode change (only if we didn't already handle it in tool change).
        if idx > 0
            && phase.coolant != state.current_coolant
            && !(phase.tool.is_some() && state.current_tool == phase.tool.map(|t| t.id))
        {
            push_coolant_change(&mut program, &mut state, phase.coolant);
        }

        push_pre_gcode(&mut program, phase.pre_gcode);
        push_phase_moves(&mut program, phase, &mut state);
        push_post_gcode(&mut program, phase.post_gcode);
    }

    if state.current_coolant.is_active() {
        program.push(Statement::Raw("M9\n".to_owned()));
    }
    program.push(Statement::Postamble);
    program
}

/// Build a `Program` for multiple setups separated by M0 pauses.
pub fn build_multi_setup(setups: &[GcodeSetupPhase<'_>], safe_z: f64) -> Program {
    let mut program = Program::new();
    if setups.is_empty() {
        return program;
    }

    let first_rpm = setups
        .iter()
        .flat_map(|setup| setup.phases.iter())
        .map(|phase| phase.spindle_rpm)
        .next()
        .unwrap_or(18_000);
    program.push(Statement::Preamble {
        spindle_rpm: first_rpm,
    });

    let first_coolant = setups
        .iter()
        .flat_map(|setup| setup.phases.iter())
        .map(|phase| phase.coolant)
        .next()
        .unwrap_or(CoolantMode::Off);
    if first_coolant.is_active() {
        program.push(Statement::Raw(first_coolant.start_gcode().to_owned()));
    }

    let initial_tool: Option<usize> = setups
        .iter()
        .flat_map(|setup| setup.phases.iter())
        .find_map(|phase| phase.tool.map(|t| t.id));
    let mut state = ModalState::new(first_rpm, initial_tool, first_coolant);

    for (setup_index, setup) in setups.iter().enumerate() {
        if setup_index > 0 {
            if state.current_coolant.is_active() {
                program.push(Statement::Raw("M9\n".to_owned()));
            }
            program.push(Statement::SafeZRetract { z: safe_z });
            let pause_text = setup.pause_message.map_or_else(
                || format!("Setup change: {}", setup.setup_label),
                |msg| msg.to_owned(),
            );
            program.push(Statement::ProgramPause {
                message: pause_text,
            });
            // After the M0 pause the operator may have re-fixtured or
            // jogged — forget the tracked position so the next rapid is
            // sequenced Z-first like a program start.
            state.reset_position();

            let next_rpm = setup
                .phases
                .first()
                .map(|phase| phase.spindle_rpm)
                .unwrap_or(state.current_rpm);
            program.push(Statement::SpindleSet { rpm: next_rpm });
            state.current_rpm = next_rpm;
            state.reset_feed();

            let next_coolant = setup
                .phases
                .first()
                .map(|phase| phase.coolant)
                .unwrap_or(CoolantMode::Off);
            if next_coolant.is_active() {
                program.push(Statement::Raw(next_coolant.start_gcode().to_owned()));
            }
            state.current_coolant = next_coolant;
        }

        program.push(Statement::Comment(format!("=== {} ===", setup.setup_label)));

        for phase in &setup.phases {
            program.push(Statement::Comment(phase.label.to_owned()));

            // Tool change (no idx>0 guard in multi-setup; the legacy
            // emitter relied on `current_tool` already matching the very
            // first phase's tool to avoid spuriously emitting a change).
            // Keyed on tool config id — display T-numbers may collide.
            if let Some(tool) = phase.tool
                && state.current_tool != Some(tool.id)
            {
                push_tool_change(&mut program, &mut state, tool, phase);
            }

            if phase.spindle_rpm != state.current_rpm {
                program.push(Statement::SpindleSet {
                    rpm: phase.spindle_rpm,
                });
                state.current_rpm = phase.spindle_rpm;
            }

            if phase.coolant != state.current_coolant
                && !(phase.tool.is_some() && state.current_tool == phase.tool.map(|t| t.id))
            {
                push_coolant_change(&mut program, &mut state, phase.coolant);
            }

            push_pre_gcode(&mut program, phase.pre_gcode);
            push_phase_moves(&mut program, phase, &mut state);
            push_post_gcode(&mut program, phase.post_gcode);
        }
    }

    if state.current_coolant.is_active() {
        program.push(Statement::Raw("M9\n".to_owned()));
    }
    program.push(Statement::Postamble);
    program
}

// ----- helpers shared between phased and multi-setup -----

fn push_tool_change(
    program: &mut Program,
    state: &mut ModalState,
    tool: PhaseTool<'_>,
    phase: &GcodePhase<'_>,
) {
    if state.current_coolant.is_active() {
        program.push(Statement::Raw("M9\n".to_owned()));
    }
    // Post-aware change block (M5 + M6 T{n} for controllers with M6;
    // M5 + operator message + M0 pause for vanilla GRBL). Rendered by
    // the emitter from the post's `tool_change` template.
    program.push(Statement::ToolChange {
        tool_number: tool.number,
        label: tool.label.to_owned(),
    });
    // SpindleSet doubles as the resume spin-up after a pause-style change.
    program.push(Statement::SpindleSet {
        rpm: phase.spindle_rpm,
    });
    state.current_rpm = phase.spindle_rpm;
    state.current_tool = Some(tool.id);
    if phase.coolant.is_active() {
        program.push(Statement::Raw(phase.coolant.start_gcode().to_owned()));
    }
    state.current_coolant = phase.coolant;
    state.reset_feed();
    // The operator may have jogged during a manual change — treat the
    // next rapid like a program start (Z-first).
    state.reset_position();
}

fn push_coolant_change(program: &mut Program, state: &mut ModalState, new_coolant: CoolantMode) {
    if state.current_coolant.is_active() && !new_coolant.is_active() {
        program.push(Statement::Raw("M9\n".to_owned()));
    } else if new_coolant.is_active() {
        if state.current_coolant.is_active() {
            program.push(Statement::Raw("M9\n".to_owned()));
        }
        program.push(Statement::Raw(new_coolant.start_gcode().to_owned()));
    }
    state.current_coolant = new_coolant;
}

fn push_pre_gcode(program: &mut Program, pre: Option<&str>) {
    if let Some(pre) = pre
        && !pre.is_empty()
    {
        let mut s = pre.to_owned();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        program.push(Statement::Raw(s));
    }
}

fn push_post_gcode(program: &mut Program, post_gc: Option<&str>) {
    if let Some(post_gc) = post_gc
        && !post_gc.is_empty()
    {
        let mut s = post_gc.to_owned();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        program.push(Statement::Raw(s));
    }
}

fn push_phase_moves(program: &mut Program, phase: &GcodePhase<'_>, state: &mut ModalState) {
    let comp = phase.controller_compensation;
    let mut comp_started = false;
    let tool_num_for_comp = phase.tool.map(|t| t.number).unwrap_or(1);

    for m in &phase.toolpath.moves {
        match m.move_type {
            MoveType::Rapid => {
                push_rapid(
                    program,
                    &mut state.prev_pos,
                    m.target.x,
                    m.target.y,
                    m.target.z,
                );
                state.reset_feed();
            }
            MoveType::Linear { feed_rate } => {
                push_comp_start_if_needed(program, comp, &mut comp_started, tool_num_for_comp);
                if state.last_feed != Some(feed_rate) {
                    program.push(Statement::Linear {
                        x: m.target.x,
                        y: m.target.y,
                        z: m.target.z,
                        feed: feed_rate,
                    });
                    state.last_feed = Some(feed_rate);
                } else {
                    program.push(Statement::LinearModal {
                        x: m.target.x,
                        y: m.target.y,
                        z: m.target.z,
                    });
                }
            }
            MoveType::ArcCW { i, j, feed_rate } => {
                push_comp_start_if_needed(program, comp, &mut comp_started, tool_num_for_comp);
                program.push(Statement::ArcCw {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                    i,
                    j,
                    feed: feed_rate,
                });
                state.last_feed = Some(feed_rate);
            }
            MoveType::ArcCCW { i, j, feed_rate } => {
                push_comp_start_if_needed(program, comp, &mut comp_started, tool_num_for_comp);
                program.push(Statement::ArcCcw {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                    i,
                    j,
                    feed: feed_rate,
                });
                state.last_feed = Some(feed_rate);
            }
        }
        // Track every move's target for safe rapid sequencing (no-op
        // re-assignment after a rapid; push_rapid already set it).
        state.prev_pos = Some((m.target.x, m.target.y, m.target.z));
    }

    if comp_started {
        program.push(Statement::Raw("G40\n".to_owned()));
    }
}

fn push_comp_start_if_needed(
    program: &mut Program,
    comp: Option<ControllerCompensation>,
    comp_started: &mut bool,
    tool_num_for_comp: u32,
) {
    if let Some(dir) = comp
        && !*comp_started
    {
        let code = match dir {
            ControllerCompensation::Left => "G41",
            ControllerCompensation::Right => "G42",
        };
        program.push(Statement::Raw(format!("{code} D{tool_num_for_comp}\n")));
        *comp_started = true;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    /// Same Toolpath input → same Program (Vec<Statement>) two runs in a
    /// row. Guards against accidental nondeterminism (HashMap iteration,
    /// timestamp insertion, etc.) creeping into the builder.
    #[test]
    fn program_builder_is_deterministic() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp1.feed_to(P3::new(20.0, 0.0, 0.0), 1000.0);
        tp1.arc_cw_to(P3::new(20.0, 5.0, 0.0), 0.0, 5.0, 800.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(30.0, 0.0, 10.0));
        tp2.feed_to(P3::new(40.0, 0.0, -1.0), 600.0);

        let make_phases = || {
            vec![
                GcodePhase {
                    toolpath: &tp1,
                    spindle_rpm: 18_000,
                    label: "Op 0 — pocket",
                    pre_gcode: Some("G55"),
                    post_gcode: Some("M9"),
                    tool: Some(PhaseTool {
                        id: 1,
                        number: 1,
                        label: "Tool One",
                    }),
                    coolant: CoolantMode::Mist,
                    controller_compensation: Some(ControllerCompensation::Left),
                },
                GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 24_000,
                    label: "Op 1 — profile",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: Some(PhaseTool {
                        id: 2,
                        number: 2,
                        label: "Tool Two",
                    }),
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                },
            ]
        };
        let make_setups = || {
            vec![
                GcodeSetupPhase {
                    setup_label: "Top",
                    phases: vec![GcodePhase {
                        toolpath: &tp1,
                        spindle_rpm: 18_000,
                        label: "Top pocket",
                        pre_gcode: None,
                        post_gcode: None,
                        tool: Some(PhaseTool {
                            id: 1,
                            number: 1,
                            label: "Tool One",
                        }),
                        coolant: CoolantMode::Off,
                        controller_compensation: None,
                    }],
                    pause_message: None,
                },
                GcodeSetupPhase {
                    setup_label: "Bottom",
                    phases: vec![GcodePhase {
                        toolpath: &tp2,
                        spindle_rpm: 24_000,
                        label: "Bottom profile",
                        pre_gcode: None,
                        post_gcode: None,
                        tool: Some(PhaseTool {
                            id: 2,
                            number: 2,
                            label: "Tool Two",
                        }),
                        coolant: CoolantMode::Flood,
                        controller_compensation: None,
                    }],
                    pause_message: None,
                },
            ]
        };

        let single_a = build_single(&tp1, 18_000);
        let single_b = build_single(&tp1, 18_000);
        assert_eq!(single_a, single_b, "build_single is nondeterministic");

        let phased_a = build_phased(&make_phases());
        let phased_b = build_phased(&make_phases());
        assert_eq!(phased_a, phased_b, "build_phased is nondeterministic");

        let multi_a = build_multi_setup(&make_setups(), 15.0);
        let multi_b = build_multi_setup(&make_setups(), 15.0);
        assert_eq!(multi_a, multi_b, "build_multi_setup is nondeterministic");
    }

    /// `pause_message: None` falls back to the default
    /// `Setup change: <label>` text, and `Some("…")` overrides it verbatim.
    /// Verified by inspecting the `ProgramPause` statement directly so this
    /// is independent of any post's comment-rendering quirks.
    #[test]
    fn build_multi_setup_pause_message_override() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);

        let make_phases = |label: &'static str| {
            vec![GcodePhase {
                toolpath: &tp,
                spindle_rpm: 18_000,
                label,
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }]
        };

        // Default: pause_message=None → "Setup change: Bottom"
        let setups_default = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: make_phases("top"),
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: make_phases("bottom"),
                pause_message: None,
            },
        ];
        let prog_default = build_multi_setup(&setups_default, 15.0);
        let pause_default = prog_default.statements.iter().find_map(|s| match s {
            Statement::ProgramPause { message } => Some(message.as_str()),
            _ => None,
        });
        assert_eq!(pause_default, Some("Setup change: Bottom"));

        // Override: pause_message wins.
        let setups_override = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: make_phases("top"),
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: make_phases("bottom"),
                pause_message: Some("Run Z Probe macro then Resume"),
            },
        ];
        let prog_override = build_multi_setup(&setups_override, 15.0);
        let pause_override = prog_override.statements.iter().find_map(|s| match s {
            Statement::ProgramPause { message } => Some(message.as_str()),
            _ => None,
        });
        assert_eq!(pause_override, Some("Run Z Probe macro then Resume"));
    }

    // ── Safe rapid sequencing (diagonal-rapid splitting) ──────────────

    /// Helper: collect only the motion statements (Rapid / SafeZRetract)
    /// for compact assertions.
    fn motion_statements(prog: &Program) -> Vec<Statement> {
        prog.statements
            .iter()
            .filter(|s| matches!(s, Statement::Rapid { .. } | Statement::SafeZRetract { .. }))
            .cloned()
            .collect()
    }

    fn phase_for<'a>(tp: &'a Toolpath, tool_id: usize) -> GcodePhase<'a> {
        GcodePhase {
            toolpath: tp,
            spindle_rpm: 18_000,
            label: "op",
            pre_gcode: None,
            post_gcode: None,
            tool: Some(PhaseTool {
                id: tool_id,
                number: tool_id as u32,
                label: "Tool",
            }),
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }
    }

    /// First rapid of a program has no previous position: Z-first to the
    /// rapid's (safe by construction) target height, then the full move.
    #[test]
    fn first_rapid_is_z_first() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 6.0, 10.0));

        let prog = build_single(&tp, 18_000);
        let moves = motion_statements(&prog);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 10.0 },
                Statement::Rapid {
                    x: 5.0,
                    y: 6.0,
                    z: 10.0
                },
            ]
        );

        // Same rule through the phased path.
        let prog = build_phased(&[phase_for(&tp, 1)]);
        assert_eq!(motion_statements(&prog), moves);
    }

    /// Mid-path diagonal rapid with Z rising: lift to the target Z
    /// first, then traverse at the new height.
    #[test]
    fn rising_diagonal_rapid_splits_z_first() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, -2.0), 300.0);
        tp.rapid_to(P3::new(30.0, 40.0, 5.0)); // XY + Z both change, Z rising

        let prog = build_phased(&[phase_for(&tp, 1)]);
        let moves = motion_statements(&prog);
        assert_eq!(
            moves,
            vec![
                // first rapid (program start)
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0
                },
                // rising diagonal: lift, then traverse
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0
                },
            ]
        );
    }

    /// Mid-path diagonal rapid with Z falling: traverse at the current
    /// (higher) Z first, then plunge straight down.
    #[test]
    fn falling_diagonal_rapid_splits_xy_first() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(30.0, 40.0, 2.0)); // XY + Z both change, Z falling

        let prog = build_phased(&[phase_for(&tp, 1)]);
        let moves = motion_statements(&prog);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 10.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 10.0
                },
                // falling diagonal: traverse at Z=10, then plunge
                Statement::Rapid {
                    x: 30.0,
                    y: 40.0,
                    z: 10.0
                },
                Statement::Rapid {
                    x: 30.0,
                    y: 40.0,
                    z: 2.0
                },
            ]
        );
    }

    /// Only-XY and only-Z rapids stay single statements.
    #[test]
    fn axis_aligned_rapids_do_not_split() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(20.0, 5.0, 10.0)); // XY only
        tp.rapid_to(P3::new(20.0, 5.0, 2.0)); // Z only (falling)
        tp.rapid_to(P3::new(20.0, 5.0, 12.0)); // Z only (rising)

        let prog = build_phased(&[phase_for(&tp, 1)]);
        let moves = motion_statements(&prog);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 10.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 10.0
                },
                Statement::Rapid {
                    x: 20.0,
                    y: 5.0,
                    z: 10.0
                },
                Statement::Rapid {
                    x: 20.0,
                    y: 5.0,
                    z: 2.0
                },
                Statement::Rapid {
                    x: 20.0,
                    y: 5.0,
                    z: 12.0
                },
            ]
        );
    }

    /// Position carries over BETWEEN phases of one program (no spurious
    /// Z-first on the second phase), but a tool change resets it — the
    /// operator may have jogged during a manual change.
    #[test]
    fn rapid_after_tool_change_is_z_first_but_same_tool_phase_carries_position() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp1.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 30.0, 5.0));

        // Same tool both phases: position carries over; tp2's rapid is a
        // rising diagonal from (10,0,-1) → lift-then-traverse split, NOT
        // a program-start retract-only sequence... distinguish by count:
        // rising split = SafeZRetract + Rapid (same shape as program
        // start), so instead verify the SAME-tool case by making tp2's
        // first rapid axis-aligned in Z from the carried position.
        let mut tp2_same_xy = Toolpath::new();
        tp2_same_xy.rapid_to(P3::new(10.0, 0.0, 5.0)); // Z-only from (10,0,-1)

        let same_tool = build_phased(&[phase_for(&tp1, 1), phase_for(&tp2_same_xy, 1)]);
        let moves = motion_statements(&same_tool);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0
                },
                // carried position (10,0,-1): Z-only rapid, no split
                Statement::Rapid {
                    x: 10.0,
                    y: 0.0,
                    z: 5.0
                },
            ],
            "position must carry across same-tool phase boundary"
        );

        // Different tool: change fires, position resets, next rapid is
        // Z-first even though it would have been a plain rising split.
        let diff_tool = build_phased(&[phase_for(&tp1, 1), phase_for(&tp2, 2)]);
        let has_tool_change = diff_tool
            .statements
            .iter()
            .any(|s| matches!(s, Statement::ToolChange { .. }));
        assert!(has_tool_change, "tool change must fire between tools");
        let moves = motion_statements(&diff_tool);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0
                },
                // post-tool-change: program-start sequencing again
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 20.0,
                    y: 30.0,
                    z: 5.0
                },
            ]
        );
    }

    /// Multi-setup boundary (M0 pause) also resets the tracked position.
    #[test]
    fn rapid_after_setup_pause_is_z_first() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp1.feed_to(P3::new(10.0, 0.0, -1.0), 600.0);

        let mut tp2 = Toolpath::new();
        // Z-only from tp1's last position — would NOT split if position
        // carried over; must be Z-first after the M0 pause.
        tp2.rapid_to(P3::new(10.0, 0.0, 5.0));

        let setups = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: vec![phase_for(&tp1, 1)],
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: vec![phase_for(&tp2, 1)],
                pause_message: None,
            },
        ];
        let prog = build_multi_setup(&setups, 15.0);
        let moves = motion_statements(&prog);
        assert_eq!(
            moves,
            vec![
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 0.0,
                    y: 0.0,
                    z: 5.0
                },
                // inter-setup retract to safe-Z (pre-existing behaviour)
                Statement::SafeZRetract { z: 15.0 },
                // post-pause: program-start sequencing
                Statement::SafeZRetract { z: 5.0 },
                Statement::Rapid {
                    x: 10.0,
                    y: 0.0,
                    z: 5.0
                },
            ]
        );
    }

    /// The tool-change block emits in order: coolant-off (if active),
    /// ToolChange, SpindleSet.
    #[test]
    fn tool_change_statement_ordering() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 5.0));
        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(1.0, 0.0, 5.0));

        let mut p1 = phase_for(&tp1, 1);
        p1.coolant = CoolantMode::Flood;
        let mut p2 = phase_for(&tp2, 2);
        p2.spindle_rpm = 24_000;

        let prog = build_phased(&[p1, p2]);
        let idx_of = |pred: fn(&Statement) -> bool| {
            prog.statements
                .iter()
                .position(pred)
                .expect("statement present")
        };
        let m9 = idx_of(|s| matches!(s, Statement::Raw(t) if t == "M9\n"));
        let tc = idx_of(|s| matches!(s, Statement::ToolChange { tool_number: 2, .. }));
        let spin = idx_of(|s| matches!(s, Statement::SpindleSet { rpm: 24_000 }));
        assert!(m9 < tc && tc < spin, "M9 → ToolChange → SpindleSet order");
    }
}
