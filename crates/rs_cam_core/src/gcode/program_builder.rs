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
            program.push(Statement::Raw(format!("M3 S{}\n", phase.spindle_rpm)));
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
            program.push(Statement::ProgramPause {
                message: format!("Setup change: {}", setup.setup_label),
            });

            let next_rpm = setup
                .phases
                .first()
                .map(|phase| phase.spindle_rpm)
                .unwrap_or(state.current_rpm);
            program.push(Statement::Raw(format!("M3 S{next_rpm}\n")));
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
                program.push(Statement::Raw(format!("M3 S{}\n", phase.spindle_rpm)));
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
    program.push(Statement::Raw(format!("M3 S{}\n", phase.spindle_rpm)));
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
