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
use super::{ControllerCompensation, CoolantMode, GcodePhase, GcodeSetupPhase};
use crate::toolpath::{MoveType, Toolpath};

/// Build a `Program` for a single toolpath.
pub fn build_single(toolpath: &Toolpath, spindle_rpm: u32) -> Program {
    let mut program = Program::new();
    program.push(Statement::Preamble { spindle_rpm });

    let mut last_feed: Option<f64> = None;
    for m in &toolpath.moves {
        match m.move_type {
            MoveType::Rapid => {
                program.push(Statement::Rapid {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                });
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

    let mut state = ModalState::new(first_rpm, first_phase.tool_number, first_coolant);

    for (idx, phase) in phases.iter().enumerate() {
        program.push(Statement::Comment(phase.label.to_owned()));

        // Tool change: emit M6 T{n} if tool number changed (skip first phase).
        if idx > 0
            && let Some(tool_num) = phase.tool_number
            && state.current_tool != Some(tool_num)
        {
            push_tool_change(&mut program, &mut state, tool_num, phase);
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
            && !(phase.tool_number.is_some() && state.current_tool == phase.tool_number)
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

    let initial_tool: Option<u32> = setups
        .iter()
        .flat_map(|setup| setup.phases.iter())
        .find_map(|phase| phase.tool_number);
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
            program.push(Statement::ProgramPause { message: pause_text });

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
            // first phase's tool to avoid spuriously emitting M6).
            if let Some(tool_num) = phase.tool_number
                && state.current_tool != Some(tool_num)
            {
                push_tool_change(&mut program, &mut state, tool_num, phase);
            }

            if phase.spindle_rpm != state.current_rpm {
                program.push(Statement::SpindleSet {
                rpm: phase.spindle_rpm,
            });
                state.current_rpm = phase.spindle_rpm;
            }

            if phase.coolant != state.current_coolant
                && !(phase.tool_number.is_some() && state.current_tool == phase.tool_number)
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
    tool_num: u32,
    phase: &GcodePhase<'_>,
) {
    if state.current_coolant.is_active() {
        program.push(Statement::Raw("M9\n".to_owned()));
    }
    program.push(Statement::Raw("M5\n".to_owned()));
    program.push(Statement::Raw(format!("M6 T{tool_num}\n")));
    program.push(Statement::SpindleSet {
                rpm: phase.spindle_rpm,
            });
    state.current_rpm = phase.spindle_rpm;
    state.current_tool = Some(tool_num);
    if phase.coolant.is_active() {
        program.push(Statement::Raw(phase.coolant.start_gcode().to_owned()));
    }
    state.current_coolant = phase.coolant;
    state.reset_feed();
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
    let tool_num_for_comp = phase.tool_number.unwrap_or(1);

    for m in &phase.toolpath.moves {
        match m.move_type {
            MoveType::Rapid => {
                program.push(Statement::Rapid {
                    x: m.target.x,
                    y: m.target.y,
                    z: m.target.z,
                });
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
                    tool_number: Some(1),
                    coolant: CoolantMode::Mist,
                    controller_compensation: Some(ControllerCompensation::Left),
                },
                GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 24_000,
                    label: "Op 1 — profile",
                    pre_gcode: None,
                    post_gcode: None,
                    tool_number: Some(2),
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
                        tool_number: Some(1),
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
                        tool_number: Some(2),
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
                tool_number: Some(1),
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
}
