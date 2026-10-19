//! G-code generation and post-processors.
//!
//! Note: `let _ = write!(...)` is used throughout this module for writing to `String`.
//! Writing to `String` is infallible (only fails on OOM, which panics regardless),
//! so discarding the `Result` with `let _ =` is safe.

pub mod emitter;
pub mod ir;
mod modal;
pub mod post;
pub mod program_builder;
pub mod wizard_overlay;

pub use ir::{CannedDrillKind, Program, ProgramMetadata, Statement};
pub use post::{
    ArcLinearize, CommentStyle, Decimals, Feedrate, LoadError as PostLoadError, PostDefinition,
    PostLimits, Rpm, SafeZ, Units, WcsCode,
};
pub use wizard_overlay::{ToolChangeMode, WizardOverlay};

use crate::session::ProjectSession;
use crate::stock::simulation_cut::SimulationCutTrace;
use crate::toolpath::Toolpath;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Write};

/// The operator overrides of the tool-load export gate. Each flag waives
/// a distinct class of refusal; the default refuses both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolLoadExportPolicy {
    pub accept_unmodeled: bool,
    pub accept_exceeded: bool,
}

/// F061: the override that waives a load refusal, and how each surface
/// spells it.
///
/// A refusal names the override the way the reader types it. The core
/// field name (`accept_exceeded`) is accepted by no surface, so the text
/// never prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOverride {
    /// Waives `Exceeds` rows whose bound may gate an export.
    AcceptExceeded,
    /// Waives `Unmodeled` rows, stale simulation included.
    AcceptUnmodeled,
}

impl LoadOverride {
    /// The MCP `export_gcode` key (`rs_cam_mcp::server::ExportGcodeParams`).
    #[must_use]
    pub const fn mcp_key(self) -> &'static str {
        match self {
            Self::AcceptExceeded => "accept_exceeded_tool_load",
            Self::AcceptUnmodeled => "accept_unmodeled_tool_load",
        }
    }

    /// The CLI flag, without the leading `--` (clap `long`). The `project`
    /// and `run` subcommands name their flags from this.
    #[must_use]
    pub const fn cli_flag(self) -> &'static str {
        match self {
            Self::AcceptExceeded => "accept-exceeded-tool-load",
            Self::AcceptUnmodeled => "accept-unmodeled-tool-load",
        }
    }

    /// The start of the GUI pre-flight checkbox label (`ui/preflight.rs`).
    #[must_use]
    pub const fn gui_label(self) -> &'static str {
        match self {
            Self::AcceptExceeded => "Accept EXCEEDED criteria",
            Self::AcceptUnmodeled => "Accept unmodeled criteria",
        }
    }

    /// The sentence that names this override on every surface.
    #[must_use]
    pub fn spellings(self) -> String {
        format!(
            "MCP `{}: true`, CLI `--{}`, or the GUI pre-flight checkbox \"{}\"",
            self.mcp_key(),
            self.cli_flag(),
            self.gui_label()
        )
    }
}

/// F061: why a checked G-code export refused. One variant per kind of
/// refusal, so a surface can tell a load refusal it may override from a
/// refusal no override waives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRefusal {
    /// A1: the effective units are inch, which the emitter does not
    /// convert.
    InchUnits,
    /// G1: an operation asks for controller cutter compensation on a post
    /// that has none.
    ControllerCompensation {
        /// The effective post name.
        post: String,
        /// The operations that set compensation "In Control".
        operations: Vec<String>,
    },
    /// S4: a criterion exceeded a bound that may refuse an export.
    LoadExceeded {
        /// One line per toolpath that refused.
        refusing: Vec<String>,
        /// The rows that exceeded a bound that may not refuse, printed
        /// beside the refusal so the operator reads the whole picture.
        advisories: Vec<String>,
    },
    /// T-19: a criterion could not be modelled.
    LoadUnmodeled {
        /// One line per refusing row: toolpath, kind and clause.
        rows: Vec<String>,
    },
    /// PR-4: the simulation trace is stale, so the gates could not be
    /// honoured against the current toolpaths.
    StaleSimulation {
        /// One line per refusing row.
        rows: Vec<String>,
    },
}

impl ExportRefusal {
    /// The override that waives this refusal, or `None` when no override
    /// does (fix the project instead).
    #[must_use]
    pub fn override_needed(&self) -> Option<LoadOverride> {
        match self {
            Self::InchUnits | Self::ControllerCompensation { .. } => None,
            Self::LoadExceeded { .. } => Some(LoadOverride::AcceptExceeded),
            Self::LoadUnmodeled { .. } | Self::StaleSimulation { .. } => {
                Some(LoadOverride::AcceptUnmodeled)
            }
        }
    }

    /// The refusal text without the override sentence. A record of an
    /// override that was used prints this.
    #[must_use]
    pub fn body(&self) -> String {
        match self {
            Self::InchUnits => "inch output not yet supported — coordinates are millimeters; \
                 set units back to mm (G21) to export"
                .to_owned(),
            Self::ControllerCompensation { post, operations } => format!(
                "{post} has no cutter compensation (G41/G42), but {} set compensation to \
                 \"In Control\": {}. The cut would be off by the tool radius. \
                 Set the compensation to \"In Computer\" and regenerate",
                if operations.len() == 1 {
                    "this operation has"
                } else {
                    "these operations have"
                },
                operations.join(", ")
            ),
            Self::LoadExceeded {
                refusing,
                advisories,
            } => {
                let mut msg =
                    String::from("G-code export refused: tool load exceeded on toolpath(s):\n");
                for line in refusing {
                    let _ = writeln!(msg, "{line}");
                }
                msg.push_str(&advisory_block(advisories));
                msg.trim_end().to_owned()
            }
            Self::LoadUnmodeled { rows } => {
                let mut msg = String::from(
                    "G-code export refused: tool load not fully modeled for toolpath(s):\n",
                );
                for line in rows {
                    let _ = writeln!(msg, "{line}");
                }
                msg.trim_end().to_owned()
            }
            Self::StaleSimulation { rows } => {
                let mut msg = String::from(
                    "G-code export refused: cached simulation is stale — re-run simulation \
                     to verify load gates against current toolpath state.\n",
                );
                for line in rows {
                    let _ = writeln!(msg, "{line}");
                }
                msg.trim_end().to_owned()
            }
        }
    }
}

impl Display for ExportRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.body())?;
        match self {
            Self::InchUnits | Self::ControllerCompensation { .. } => Ok(()),
            Self::LoadExceeded { .. } => write!(
                f,
                "\nTo override (a known-dangerous override): {}.",
                LoadOverride::AcceptExceeded.spellings()
            ),
            Self::LoadUnmodeled { .. } => write!(
                f,
                "\nTo acknowledge unmodeled criteria: {}.",
                LoadOverride::AcceptUnmodeled.spellings()
            ),
            Self::StaleSimulation { .. } => write!(
                f,
                "\nRun simulation, then export again. To export against the stale evidence \
                 anyway: {}.",
                LoadOverride::AcceptUnmodeled.spellings()
            ),
        }
    }
}

impl std::error::Error for ExportRefusal {}

/// The advisory block: the header line and one line per advisory, or an
/// empty string when there is none.
fn advisory_block(advisories: &[String]) -> String {
    if advisories.is_empty() {
        return String::new();
    }
    let mut block = String::from(
        "Exceeded without refusing the export, because the bound does not gate one:\n",
    );
    for line in advisories {
        let _ = writeln!(block, "{line}");
    }
    block
}

/// F051: what the tool-load gate let through, and why.
///
/// Every export result carries one, and every door destructures it, so an
/// advisory or a used override cannot be dropped the way `let _note`
/// dropped it.
#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadPolicyOutcome {
    /// One line per criterion that exceeded a bound that may not refuse
    /// an export. The operator must still read them.
    pub advisories: Vec<String>,
    /// The refusals an operator override waived for this export.
    pub overridden: Vec<ExportRefusal>,
}

impl LoadPolicyOutcome {
    /// True when the gate has nothing to report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.advisories.is_empty() && self.overridden.is_empty()
    }

    /// The advisory block (header and lines), or `None` with no advisory.
    #[must_use]
    pub fn advisory_note(&self) -> Option<String> {
        let block = advisory_block(&self.advisories);
        (!block.is_empty()).then(|| block.trim_end().to_owned())
    }

    /// The text a surface shows for this outcome: the advisory block, then
    /// one block per override used. `None` when there is nothing to say.
    #[must_use]
    pub fn report(&self) -> Option<String> {
        let mut parts: Vec<String> = self.advisory_note().into_iter().collect();
        for refusal in &self.overridden {
            parts.push(format!(
                "OVERRIDE USED — without it this export is refused:\n{}",
                refusal.body()
            ));
        }
        (!parts.is_empty()).then(|| parts.join("\n"))
    }

    /// Add `other` to this outcome (a multi-file export reports one).
    pub fn merge(&mut self, other: Self) {
        let Self {
            advisories,
            overridden,
        } = other;
        self.advisories.extend(advisories);
        self.overridden.extend(overridden);
    }
}

/// A checked emission with no session: the program and what the load gate
/// let through. The CLI job pipeline and the session door read it.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgram {
    pub gcode: String,
    pub load_outcome: LoadPolicyOutcome,
}

/// F067: one finished export from the session door
/// ([`crate::session::ProjectSession::export_program`]) and every check
/// it ran. The GUI, MCP and the CLI read this one type.
#[must_use]
#[derive(Debug, Clone)]
pub struct ExportedGcode {
    /// The emitted program, after the high-feedrate rapid→feed rewrite
    /// when the post config asks for it.
    pub gcode: String,
    /// EDG-06: what the machine-safety pass found, in emission order. The
    /// pass ran BEFORE the rewrite, while the rapids were still `G0`. Do
    /// not re-run it on `gcode`. Empty is a clean pass.
    pub machine_safety: Vec<crate::export::gcode_validator::Finding>,
    /// F051: the load gate's advisories and the overrides it used.
    pub load_outcome: LoadPolicyOutcome,
    /// W2.5 (D10): one line per emitted toolpath whose feeds came from a
    /// simulation the session no longer holds. A caution, not a refusal.
    pub feeds_cautions: Vec<String>,
}

/// F067: what the session door emits — the phases, which toolpaths they
/// are, the load report the gate reads, the policy and the overlay.
pub struct ExportRequest<'a> {
    /// The phases, arranged by setup.
    pub phases: ExportPhases<'a>,
    /// The toolpath index of each emitted phase. The machine-safety pass
    /// reads the retract plane of these toolpaths.
    pub emitted: Vec<usize>,
    /// The report the load gate enforces `policy` against.
    pub report: &'a crate::tool_load::ToolLoadReport,
    pub policy: ToolLoadExportPolicy,
    pub overlay: &'a WizardOverlay,
}

/// Direction for G41/G42 controller cutter compensation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerCompensation {
    /// G41 — cutter compensation left.
    Left,
    /// G42 — cutter compensation right.
    Right,
}

/// Coolant mode for G-code output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoolantMode {
    /// No coolant (default).
    #[default]
    Off,
    /// Mist coolant (M7).
    Mist,
    /// Flood coolant (M8).
    Flood,
    /// Both mist and flood (M7 + M8).
    Both,
}

impl CoolantMode {
    /// Emit the G-code command(s) to activate this coolant mode.
    /// Returns an empty string for `Off`.
    pub fn start_gcode(self) -> &'static str {
        match self {
            CoolantMode::Off => "",
            CoolantMode::Mist => "M7\n",
            CoolantMode::Flood => "M8\n",
            CoolantMode::Both => "M7\nM8\n",
        }
    }

    /// Returns true if coolant is active (not Off).
    pub fn is_active(self) -> bool {
        self != CoolantMode::Off
    }
}

/// Emit G-code from a toolpath using the given post definition.
pub fn emit_gcode(toolpath: &Toolpath, post: &PostDefinition, spindle_rpm: u32) -> String {
    emitter::emit_program(&program_builder::build_single(toolpath, spindle_rpm), post)
}

/// The tool a phase cuts with, as seen by the g-code program builder.
///
/// `id` is the *identity* — the tool config id (`ToolConfig::id.0`).
/// Tool-change detection keys on it, NOT on the user-curated display
/// `number`: real projects carry T-number collisions across distinct
/// tools (WANAKA: "End Mill" and "Tapered Ball 2mm" both T1), which
/// silently suppressed the change when detection keyed on the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseTool<'a> {
    /// Tool config id — the identity used for change detection.
    pub id: usize,
    /// Display T-number, substituted into the post's `tool_change`
    /// template (`M6 T{n}` / operator message).
    pub number: u32,
    /// Tool name for the operator message
    /// (`TOOL CHANGE: {label} [T{number}]`).
    pub label: &'a str,
}

/// A phase in a multi-operation job: toolpath + spindle speed + label.
#[derive(Clone)]
pub struct GcodePhase<'a> {
    pub toolpath: &'a Toolpath,
    pub spindle_rpm: u32,
    pub label: &'a str,
    /// Raw G-code to emit before this toolpath's moves (user-defined).
    pub pre_gcode: Option<&'a str>,
    /// Raw G-code to emit after this toolpath's moves (user-defined).
    pub post_gcode: Option<&'a str>,
    /// The tool this phase cuts with. If `Some` and its `id` differs
    /// from the previous phase's, the post's tool-change template is
    /// emitted before this phase.
    pub tool: Option<PhaseTool<'a>>,
    /// Coolant mode for this phase. Coolant on/off commands are emitted
    /// when the mode changes between phases.
    pub coolant: CoolantMode,
    /// Controller cutter compensation (G41/G42). When `Some`, a G41/G42
    /// command is emitted before the first cutting move and G40 after the last.
    pub controller_compensation: Option<ControllerCompensation>,
    /// G7: the drill cycle of a drill phase. `None` for every other
    /// operation. The builder writes the G82 dwell from it, and the native
    /// G81/G82/G83 cycles when the export asks for them.
    pub drill: Option<PhaseDrill>,
    /// W2.5 (D10): the feed-stage line the builder writes as a comment
    /// after the label, from
    /// [`FeedStage::statement`](crate::session::FeedStage::statement).
    /// Every session door fills it; `None` writes no line.
    pub feeds: Option<String>,
}

/// G7: what the emitter needs to know about a drill phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseDrill {
    /// The cycle the operation drills, with its parameters.
    pub cycle: crate::ops::drill::DrillCycle,
}

impl PhaseDrill {
    /// The dwell at the hole bottom, in seconds, when the cycle has one
    /// (G82). `None` for the other cycles and for a dwell of 0.
    pub fn dwell_s(&self) -> Option<f64> {
        match self.cycle {
            crate::ops::drill::DrillCycle::Dwell(s) if s.is_finite() && s > 0.0 => Some(s),
            _ => None,
        }
    }
}

/// G7: the drill cycle a toolpath config asks for, or `None` for an
/// operation that is not a drill. The one mapping every export door uses.
pub fn phase_drill_for(tc: &crate::session::ToolpathConfig) -> Option<PhaseDrill> {
    use crate::compute::catalog::OperationConfig;
    match tc.operation {
        OperationConfig::Drill(ref cfg) => Some(PhaseDrill {
            cycle: cfg.cycle.to_core(cfg),
        }),
        // The pin config clamps the peck to the hole depth it derives at
        // generation time. The emitter needs the cycle kind and the dwell;
        // an unclamped peck only matters to a native G83 Q word, where a Q
        // above the hole depth is one peck.
        OperationConfig::AlignmentPinDrill(ref cfg) => Some(PhaseDrill {
            cycle: cfg.drill_cycle(f64::INFINITY),
        }),
        _ => None,
    }
}

/// Emit G-code from multiple phases, inserting tool changes, spindle speed
/// changes, and coolant commands between operations as needed.
///
/// Test-only shim around `emit_gcode_phased_with_overlay(_, _, &default)`.
/// Production callers go through [`emit_checked`].
#[cfg(test)]
fn emit_gcode_phased(phases: &[GcodePhase<'_>], post: &PostDefinition) -> String {
    emit_gcode_phased_with_overlay(phases, post, &WizardOverlay::default())
}

/// Same as `emit_gcode_phased`, but applies a `WizardOverlay` (WCS, units,
/// spindle warmup) on top of the post's defaults. `WizardOverlay::default()`
/// is byte-identical to the no-overlay path.
pub(crate) fn emit_gcode_phased_with_overlay(
    phases: &[GcodePhase<'_>],
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> String {
    let mut program = program_builder::build_phased_with(phases, build_options(post, overlay));
    prepend_t_collision_warnings(&mut program, phases.iter(), post, overlay);
    emitter::emit_program_with_overlay(&program, post, overlay)
}

/// The builder choices one export takes from its post and overlay.
///
/// G7: native drill cycles need the option, a post that runs them, and no
/// dry run. A native cycle cuts by itself, so the dry-run Z clamp could
/// not lift it.
fn build_options(post: &PostDefinition, overlay: &WizardOverlay) -> program_builder::BuildOptions {
    program_builder::BuildOptions {
        native_drill_cycles: overlay.native_drill_cycles
            && post.canned_drill_cycles
            && overlay.dry_run_safe_z.is_none(),
    }
}

/// A7 — distinct tools sharing a display T-number on an M6 post: the
/// controller keys the physical swap on the T word, so `M6 T1` → `M6 T1`
/// silently skips the change. Detection keys on the *effective* post
/// (overlay tool-change override applied); pause-style posts name the
/// incoming tool in the operator message, so they are not affected.
fn prepend_t_collision_warnings<'a>(
    program: &mut Program,
    phases: impl Iterator<Item = &'a GcodePhase<'a>>,
    post: &PostDefinition,
    overlay: &WizardOverlay,
) {
    let effective_post = overlay.applied_post(post);
    // grblHAL skips an M6 whose T is the loaded tool (gcode.c:2613-2615),
    // as LinuxCNC and Mach3 do.
    if !effective_post.tool_change_commands_m6() {
        return;
    }
    // number → distinct tool config ids seen under that number.
    let mut by_number: std::collections::BTreeMap<u32, Vec<usize>> =
        std::collections::BTreeMap::new();
    for phase in phases {
        if let Some(tool) = phase.tool {
            let ids = by_number.entry(tool.number).or_default();
            if !ids.contains(&tool.id) {
                ids.push(tool.id);
            }
        }
    }
    let mut warnings: Vec<Statement> = Vec::new();
    // G10: `$341=4` makes grblHAL ignore M6 (gcode.c:1838) and set the new
    // tool at once (gcode.c:4144-4147). The program then cuts on with the
    // old tool, and nothing stops it.
    if overlay.controller_ignores_m6
        && program
            .statements
            .iter()
            .any(|s| matches!(s, Statement::ToolChange { .. }))
    {
        warnings.push(Statement::Comment(
            "WARNING: the controller has $341=4 (ignore M6). The tool changes in this \
             file do not happen. Set $341=3, or export with tool change = pause"
                .to_owned(),
        ));
    }
    for (number, ids) in &by_number {
        if ids.len() > 1 {
            warnings.push(Statement::Comment(format!(
                "WARNING: distinct tools share T{number} — M6 will not trigger a physical change"
            )));
        }
    }
    for (offset, w) in warnings.into_iter().enumerate() {
        program.statements.insert(offset, w);
    }
}

// ── Export datum (G-EXPORT-DATUM, 2026-08-19) ────────────────────────
//
// Every setup's G-code must express XY in ONE frame, or a two-sided job
// hands the operator two datums under a single `G54` and the second side
// machines off by the stock origin. The chosen frame is **stock-relative**:
// program X0 Y0 at the stock's min corner, which is what non-identity
// setups already emit and the frame `StockConfig::alignment_pins` — the
// feature that physically registers the flip — is dimensioned in.
//
// The stored/generated toolpath is NOT touched: this is a translation
// applied at emit time only, so the simulator, the viewport, screenshots
// and every metric keep reading the frame they were built against. (The
// separate G-SIM-IDENTITY-FRAME fix shifts the *simulation* stock by the
// same vector inside `compute::simulate`; the two never compose because
// neither writes back to the toolpath.)
//
// ── The Z datum (2026-09-07) ────────────────────────────────────────
//
// Z follows the setup's own `datum.z_method` (`SetupEvalContext::z_datum`),
// where XY is fixed stock-relative. The two axes are unlike:
//
//  * XY is never re-zeroed between setups — the operator flips the part
//    against the same pins and keeps the same XY zero. A disagreement in
//    XY is therefore silent and fatal, which is why XY is forced to one
//    stock-relative frame for every setup.
//  * Z *is* re-zeroed between setups, so a per-setup Z datum is safe. The
//    operator ruled that the datum belongs to the setup, not the export:
//    the default `ZDatum::StockTop` now shifts program Z0 to the emitted
//    stock top. On a 2D stock that already follows the `origin_z`
//    convention (top at world Z0, set by `StockConfig::update_from_bbox`)
//    the shift is zero, so those projects stay byte-identical and keep
//    "zero to the top of the stock". A 3D job whose stock top sits above
//    world Z0 (terrain at +7) previously emitted Z0 at the MODEL origin —
//    7 mm below the stock top — and only a header comment said so; it now
//    emits Z0 AT the stock top. A flipped setup, which used to emit an
//    implicit spoilboard (table) Z datum, now zeroes to its presented
//    up-facing surface.
//  * The shift changes no motion. The retract plane is derived from the
//    emitted-frame stock top (F-024 / G-SAFEZ-LOCAL, `SetupEvalContext::
//    safe_z`) and rides in the toolpath, so a uniform Z translation
//    re-expresses every height against the new zero and preserves every
//    clearance. `StockTop` always LOWERS the frame, so the fixed positive
//    Z literals the emitter writes but does not shift (post `safe_z`,
//    postamble Z) end up farther above the stock — never inside it.
//
// `MachineTable` / `FixedOffset` would RAISE the frame and are therefore
// NOT auto-applied yet (they would drive those fixed literals into the
// stock); the header names the declared datum instead.

/// Translation from the frame `toolpath_index`'s toolpath was generated
/// in to the shared export datum. See the module note above: XY is
/// stock-relative (zero for non-identity setups, `-stock_bbox.min` for
/// identity ones); Z follows the setup's `datum.z_method` (`StockTop`
/// puts the emitted stock top at Z0). A toolpath that belongs to no setup
/// is treated as identity with the `StockTop` default (the frame the
/// generator used for it).
pub fn export_datum_shift_for_toolpath(
    session: &ProjectSession,
    toolpath_index: usize,
) -> crate::geo::P3 {
    let setup = session.find_setup_for_toolpath_index(toolpath_index);
    crate::session::SetupEvalContext::build_for_setup(session, setup).export_datum_shift()
}

/// Apply an export-datum shift to a toolpath, borrowing unchanged when
/// the shift is zero (the non-identity case, and every zero-origin
/// project). Arc `i`/`j` are start→centre offsets, so a pure translation
/// leaves them alone.
pub fn toolpath_in_export_datum(toolpath: &Toolpath, shift: crate::geo::P3) -> Cow<'_, Toolpath> {
    if shift.x == 0.0 && shift.y == 0.0 && shift.z == 0.0 {
        return Cow::Borrowed(toolpath);
    }
    let mut shifted = toolpath.clone();
    for m in &mut shifted.moves {
        m.target = crate::geo::P3::new(
            m.target.x + shift.x,
            m.target.y + shift.y,
            m.target.z + shift.z,
        );
    }
    Cow::Owned(shifted)
}

/// The phases of one export, arranged for the program builder.
pub enum ExportPhases<'a> {
    /// At most one setup holds a phase: one phased program, in the order
    /// the caller gave.
    OneSetup(Vec<GcodePhase<'a>>),
    /// Two or more setups hold a phase: one group per setup, in the
    /// setup's machining order, with an M0 pause between the groups.
    Setups(Vec<GcodeSetupPhase<'a>>),
}

impl<'a> ExportPhases<'a> {
    /// Every phase, in emission order.
    pub fn phases(&self) -> Vec<&GcodePhase<'a>> {
        match self {
            Self::OneSetup(phases) => phases.iter().collect(),
            Self::Setups(setups) => setups.iter().flat_map(|s| s.phases.iter()).collect(),
        }
    }
}

/// Arrange the phases of a single-file export by setup (G3).
///
/// `take_phase(idx)` hands over the phase for toolpath index `idx`, or
/// `None` when the export emits nothing for it (disabled, not in scope).
/// Each index is asked for at most once.
///
/// When two or more setups hold a phase, the result is one
/// [`GcodeSetupPhase`] per setup, in `setup.toolpath_indices` order (the
/// machining order). A toolpath in no setup goes into a last group named
/// "Unassigned", after its own pause. Otherwise the result is one list in
/// toolpath index order, which is the order the single-file doors always
/// used, so a one-setup program keeps its bytes.
pub fn arrange_phases_by_setup<'a>(
    session: &'a ProjectSession,
    mut take_phase: impl FnMut(usize) -> Option<GcodePhase<'a>>,
) -> ExportPhases<'a> {
    let count = session.toolpath_configs().len();
    let mut taken: Vec<Option<GcodePhase<'a>>> = (0..count).map(&mut take_phase).collect();
    let setups_with_phases = session
        .list_setups()
        .iter()
        .filter(|s| {
            s.toolpath_indices
                .iter()
                .any(|i| taken.get(*i).is_some_and(Option::is_some))
        })
        .count();
    if setups_with_phases <= 1 {
        return ExportPhases::OneSetup(taken.into_iter().flatten().collect());
    }
    let mut groups: Vec<GcodeSetupPhase<'a>> = Vec::new();
    for setup in session.list_setups() {
        let phases: Vec<GcodePhase<'a>> = setup
            .toolpath_indices
            .iter()
            .filter_map(|i| taken.get_mut(*i).and_then(Option::take))
            .collect();
        if !phases.is_empty() {
            groups.push(GcodeSetupPhase {
                setup_label: &setup.name,
                phases,
                pause_message: setup.pause_message.as_deref(),
            });
        }
    }
    let orphans: Vec<GcodePhase<'a>> = taken.into_iter().flatten().collect();
    if !orphans.is_empty() {
        groups.push(GcodeSetupPhase {
            setup_label: "Unassigned",
            phases: orphans,
            pause_message: None,
        });
    }
    ExportPhases::Setups(groups)
}

/// Check whether the cached sim trace's provenance still matches the
/// project's current state. PR-4: this is the staleness signal that
/// drives [`UnmodeledReason::StaleSimulation`] emissions out of the
/// load gates — when current toolpaths or tools differ from the
/// trace's recorded hashes, the gates can no longer be honoured
/// against the current configs.
///
/// Returns `true` when the trace is safe to evaluate against. A trace
/// without a `provenance` block carries no evidence of what it was
/// simulated against, so L7 (operator ruling, 2026-09-16) reads it as
/// STALE. The arm used to return `true` for backward compatibility
/// with a pre-provenance schema, which let a gate be honoured against
/// a configuration nobody had checked.
///
/// Cheap: only iterates enabled toolpaths and hashes the cached
/// annotated toolpath. Doesn't recompute anything heavy.
pub fn sim_trace_is_fresh(project: &ProjectSession, trace: &SimulationCutTrace) -> bool {
    // L7: no provenance, no evidence. The simulator stamps the block on
    // every trace it produces (`compute/simulate.rs`), so a trace without
    // one came from somewhere that cannot say what it simulated.
    if trace.provenance.is_none() {
        return false;
    }
    (0..project.toolpath_count()).all(|index| {
        project
            .get_toolpath_config(index)
            .is_none_or(|tc| !tc.enabled)
            || sim_trace_is_fresh_for(project, trace, index)
    })
}

/// [`sim_trace_is_fresh`] for ONE operation: its own provenance only.
///
/// G-STALECARDS (2026-09-24). The project-wide answer let one edited
/// operation stale every other operation's verdicts, and a re-run could
/// never clear it while the edited operation had no core result. The load
/// report and the cut-metric cards now judge each operation by its own
/// row: it holds a core result, the trace carved that result, and the
/// configuration has not moved since. A config-hash entry that is missing
/// (a pre-PR-4 trace) reads as a match, as before.
pub fn sim_trace_is_fresh_for(
    project: &ProjectSession,
    trace: &SimulationCutTrace,
    index: usize,
) -> bool {
    let Some(tc) = project.get_toolpath_config(index) else {
        return false;
    };
    let Some(provenance) = trace.provenance.as_ref() else {
        return false;
    };
    // A toolpath with no core result was not simulated: the trace covers
    // fewer toolpaths than the project now holds.
    let Some(result) = project.get_result(index) else {
        return false;
    };
    let Some(expected) = provenance.toolpath_hashes.get(&tc.id) else {
        return false;
    };
    if *expected != crate::compute::simulate::hash_toolpath(&result.annotated().toolpath) {
        return false;
    }
    provenance
        .operation_config_hashes
        .get(&tc.id)
        .is_none_or(|expected_cfg| {
            *expected_cfg == crate::compute::simulate::hash_operation_config(&tc.operation)
        })
}

/// Build a `ToolLoadReport` from a `ProjectSession`. Delegates each
/// toolpath to `tool_load::evaluate_toolpath` (the single verdict
/// assembly site), which runs all three criteria: `chipload` and
/// `power` (per-sample, require `sim_trace`) and `deflection`
/// (per-sample tip deflection from cutting force).
///
/// PR-4: when `sim_trace` is `Some` but a row's own provenance fails
/// (`sim_trace_is_fresh_for`, per operation since G-STALECARDS), that
/// row's evaluators run with `sim_trace = None` and the resulting
/// `Unmodeled::SimulationRequired` verdicts are post-processed into
/// `Unmodeled::StaleSimulation`. The diagnostics adapter renders
/// `StaleSimulation` as `DiagnosticState::StaleEvidence`, which the
/// UI displays as a neutral "re-run simulation" hint rather than a
/// scary warning.
pub fn project_load_report(
    project: &ProjectSession,
    sim_trace: Option<&SimulationCutTrace>,
) -> crate::tool_load::ToolLoadReport {
    // Freshness gate — see [`sim_trace_is_fresh`] for the contract.
    // G-STALECARDS: freshness is judged PER OPERATION, below. A trace
    // exists or not for the whole report; each row then uses it only when
    // its own provenance holds.
    let offered_trace = sim_trace;

    let mut per_toolpath = Vec::new();
    for (idx, tc) in project.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        // F070: the one context door. `None` for a removed tool or a tool
        // outside its domain (F029): skip rather than fabricate a verdict.
        let Some(parts) = project.load_context_parts(idx) else {
            continue;
        };
        let machine = project.machine();
        // The export uses the strict bands, `ToleranceBands::default()`.
        let strict_tolerance = crate::tool_load::ToleranceBands::default();
        let row_is_fresh =
            offered_trace.is_some_and(|trace| sim_trace_is_fresh_for(project, trace, idx));
        let sim_trace = offered_trace.filter(|_| row_is_fresh);
        // Phase 6 task 2: `tool_load::evaluate_toolpath` is the single
        // verdict assembly site.
        let mut verdict = crate::tool_load::evaluate_toolpath(
            &parts.ctx(),
            sim_trace,
            Some(machine),
            &strict_tolerance,
        );
        // Phase 4 — the kinematic reading. `evaluate_toolpath` cannot build
        // it: `ToolpathLoadContext` carries neither the emitted `Toolpath`
        // nor the operation's `plunge_rate`. This site holds the whole
        // `ProjectSession`, so it fills the slot from the session's single
        // producer, `kinematic_utilization_of`. This caller holds no result
        // of its own, so it takes the by-index convenience over the
        // session's stored result; the MCP narration, which DOES hold one,
        // passes its own move list to `_of` instead.
        // Phase 3: `sim_trace` is the trace this report is judging (already
        // `None` when it was thrown away as stale), so it is what decides
        // whether the reading describes planned or emitted feeds.
        verdict.kinematic_utilization = project.kinematic_utilization_for(idx, sim_trace);
        // PR-4: a row whose trace was thrown away as stale rewrites its
        // `SimulationRequired` verdicts to `StaleSimulation`, so the
        // diagnostics adapter surfaces `StaleEvidence` instead of
        // `NeedsSimulation`. G-STALECARDS: only THIS row; the others keep
        // their verdicts. F197: one method names every field.
        if offered_trace.is_some() && !row_is_fresh {
            verdict.mark_simulation_stale();
        }
        per_toolpath.push(verdict);
    }

    crate::tool_load::ToolLoadReport { per_toolpath }
}

/// **The exceeded criteria that may refuse a g-code export.** S4
/// (2026-09-18).
///
/// Pure, and public, so the rule is testable on hand-built rows. The
/// decision keys on the bound's PROVENANCE
/// ([`crate::tool_load::verdict::BoundSource::gates_export`]), never on
/// the quantity: the reason `0.20 × D` may not refuse a job is that no
/// publication states it, not that it measures a depth.
///
/// A row with NO source still refuses. Every shipped gate carries one,
/// so an absent source is a gate that forgot, and an absence must fail
/// safe.
#[must_use]
pub fn refusing_exceedances<'a>(
    criteria: &'a [crate::tool_load::verdict::CriterionStatus<'a>],
) -> Vec<&'a crate::tool_load::verdict::CriterionStatus<'a>> {
    criteria
        .iter()
        .filter(|status| status.refuses_export())
        .collect()
}

/// **The whole exceedance half of the export policy, pure.** S4
/// (2026-09-18).
///
/// `Err` refuses the export and names what refused, and also what
/// exceeded without refusing. `Ok` exports; its
/// [`LoadPolicyOutcome::advisories`] holds one line per criterion that
/// exceeded a bound that may not gate, which the operator must still
/// see. F051: under `accept_exceeded` the refusal the override waived is
/// kept in [`LoadPolicyOutcome::overridden`], never discarded.
///
/// [`enforce_load_policy`] delegates to this. It is separate so the rule
/// is testable on hand-built rows.
pub fn enforce_exceedance_policy(
    per_toolpath: &[(
        crate::ids::ToolpathId,
        Vec<crate::tool_load::verdict::CriterionStatus<'_>>,
    )],
    accept_exceeded: bool,
) -> Result<LoadPolicyOutcome, ExportRefusal> {
    use crate::tool_load::verdict::LoadState;

    let mut refusing: Vec<String> = Vec::new();
    let mut advisories: Vec<String> = Vec::new();
    for (id, criteria) in per_toolpath {
        let refusing_rows = refusing_exceedances(criteria);
        if !refusing_rows.is_empty() {
            let reason_list: Vec<String> = refusing_rows
                .iter()
                .map(|status| match &status.exceeded {
                    Some(ec) => format!("{}={}", ec.label, ec.reason_label),
                    None => status.kind.label().to_owned(),
                })
                .collect();
            refusing.push(format!("  toolpath {id}: {}", reason_list.join(", ")));
        }
        for status in criteria
            .iter()
            .filter(|s| s.state == LoadState::Exceeds && !s.refuses_export())
        {
            // Every number here is formatted from the value it
            // describes. Nothing is typed into the string.
            let peak = status
                .display_peak
                .map_or_else(|| "not stated".to_owned(), |p| format!("{p:.4}"));
            advisories.push(format!(
                "  toolpath {id}: {}, peak {peak} {}, {}",
                status.kind.label(),
                status.unit,
                status.bound_clause()
            ));
        }
    }

    if refusing.is_empty() {
        return Ok(LoadPolicyOutcome {
            advisories,
            overridden: Vec::new(),
        });
    }
    if !accept_exceeded {
        return Err(ExportRefusal::LoadExceeded {
            refusing,
            advisories,
        });
    }
    Ok(LoadPolicyOutcome {
        advisories,
        // The advisories are already in the outcome; the record of the
        // override holds the refusing lines only.
        overridden: vec![ExportRefusal::LoadExceeded {
            refusing,
            advisories: Vec::new(),
        }],
    })
}

/// True for the drill-native gates, false for the milling gates. T-19
/// (2026-09-18).
///
/// [`refusing_unmodeled`] reads it to reproduce the drill partition
/// [`crate::tool_load::ToolpathLoadVerdict::any_unmodeled`] applies:
/// drill rows exist in `criteria()` exactly when `drill_gates` is
/// `Some`, so "the list holds a drill row" is the same test as
/// "`drill_gates.is_some()`".
///
/// The list names the DRILL kinds, never the milling ones, and the
/// direction matters. A new milling gate takes the default arm, joins
/// the milling partition and refuses like the four beside it, with no
/// edit here. A new drill gate that nobody adds here reads as a milling
/// gate, so the partition does not fire and the export refuses — the
/// safe direction.
fn is_drill_gate(kind: crate::tool_load::verdict::CriterionKind) -> bool {
    use crate::tool_load::verdict::CriterionKind;
    matches!(
        kind,
        CriterionKind::DrillChipWelding
            | CriterionKind::DrillPeckAdequacy
            | CriterionKind::DrillPlungeFeed
    )
}

/// **The unmodelled criteria that refuse a g-code export.** T-19
/// (2026-09-18).
///
/// Pure, and public, so the rule is testable on hand-built rows. It is
/// the unmodelled half's answer to [`refusing_exceedances`], and it
/// derives from the same list: a gate added to
/// [`crate::tool_load::ToolpathLoadVerdict::criteria`] participates
/// here without an edit.
///
/// Two rows do not count, and both exclusions are the ones
/// [`crate::tool_load::ToolpathLoadVerdict::any_unmodeled`] already
/// applies, so the decision this helper makes is the decision the gate
/// made before it existed:
///
/// - a row [`crate::tool_load::verdict::CriterionStatus::is_known_absence`]
///   accepts — the gantry-push row, which no operator action can fill
///   (register T-10);
/// - every row, when the toolpath ran the drill gates and every milling
///   gate reported `NotApplicableForOp`. A drill cycle is not an
///   unmeasured milling cut.
///
/// Before T-19 the decision read this list and the MESSAGE enumerated
/// chipload, power and deflection by hand. A toolpath whose only
/// unmodelled row was the depth row refused and named no criterion.
#[must_use]
pub fn refusing_unmodeled<'a>(
    criteria: &'a [crate::tool_load::verdict::CriterionStatus<'a>],
) -> Vec<&'a crate::tool_load::verdict::CriterionStatus<'a>> {
    use crate::tool_load::UnmodeledReason;
    use crate::tool_load::verdict::LoadState;

    let ran_drill_gates = criteria.iter().any(|s| is_drill_gate(s.kind));
    let milling_all_not_applicable = criteria.iter().filter(|s| !is_drill_gate(s.kind)).all(|s| {
        matches!(
            s.unmodeled_reason,
            Some(UnmodeledReason::NotApplicableForOp(_))
        )
    });
    if ran_drill_gates && milling_all_not_applicable {
        return Vec::new();
    }
    criteria
        .iter()
        .filter(|s| s.state == LoadState::Unmodeled && !s.is_known_absence())
        .collect()
}

/// **The one operator-facing clause for an unmodelled row.** T-19
/// (2026-09-18).
///
/// [`crate::tool_load::UnmodeledReason`] carries no `Display`, and the
/// refusal used to print `{:?}` — a Rust variant name in an operator's
/// message. A vacuous row states its own clause; every other row words
/// its reason, and a reason that carries a detail string prints that
/// detail, because the gate that refused wrote it for this reader.
fn unmodeled_clause(status: &crate::tool_load::verdict::CriterionStatus<'_>) -> String {
    use crate::tool_load::UnmodeledReason;

    let vacuity = status.vacuity_clause();
    if !vacuity.is_empty() {
        return vacuity;
    }
    match status.unmodeled_reason {
        Some(UnmodeledReason::SimulationRequired) => "simulation has not been run".to_owned(),
        Some(UnmodeledReason::StaleSimulation) => {
            "simulation trace is stale — re-run simulation".to_owned()
        }
        Some(UnmodeledReason::ArcEngagementNotCaptured) => {
            "the cut trace holds no arc engagement — re-run the simulation".to_owned()
        }
        Some(UnmodeledReason::NoVendorData) => {
            "no vendor LUT row for this tool/material combination".to_owned()
        }
        Some(UnmodeledReason::SteadyStateSamplesNotPresent) => {
            "no steady-state cutting samples — the toolpath runs entirely on transient \
             (plunge/ramp) feeds"
                .to_owned()
        }
        Some(UnmodeledReason::MaterialUnvalidated) => {
            "the material has no measured force line (ruling B6)".to_owned()
        }
        Some(UnmodeledReason::CutterModeUnsupported(why)) => {
            format!("cutter mode unsupported — {why}")
        }
        Some(UnmodeledReason::NotImplemented(detail)) => {
            format!("not implemented yet — {detail}")
        }
        Some(UnmodeledReason::NotApplicableForOp(detail)) => {
            format!("not applicable to this operation — {detail}")
        }
        Some(UnmodeledReason::AllSamplesAirCutOrRapid) => {
            "the toolpath made no contact with material — every sample was a rapid or an \
             air cut"
                .to_owned()
        }
        None => "no reason stated".to_owned(),
    }
}

/// **The whole unmodelled half of the export policy, pure.** T-19
/// (2026-09-18).
///
/// `Err` refuses the export and names EVERY row it refused on, kind and
/// clause. `Ok` means nothing unmodelled counts, or the operator accepted
/// what does; F051: the accepted refusal is kept in
/// [`LoadPolicyOutcome::overridden`].
///
/// [`enforce_load_policy`] delegates to this, the way it delegates the
/// exceeded half to [`enforce_exceedance_policy`]. Both halves read the
/// criterion tier from the same place, which is the whole of register
/// item T-19.
///
/// The refusal kind distinguishes a stale simulation
/// ([`ExportRefusal::StaleSimulation`]) from a missing one
/// ([`ExportRefusal::LoadUnmodeled`]): they are different operator
/// actions even though both refuse. A row reads stale only through
/// [`crate::tool_load::UnmodeledReason::StaleSimulation`], which
/// [`project_load_report`] writes onto the rows it invalidated.
pub fn enforce_unmodeled_policy(
    per_toolpath: &[(
        crate::ids::ToolpathId,
        Vec<crate::tool_load::verdict::CriterionStatus<'_>>,
    )],
    accept_unmodeled: bool,
) -> Result<LoadPolicyOutcome, ExportRefusal> {
    use crate::tool_load::UnmodeledReason;

    let mut rows: Vec<String> = Vec::new();
    for (id, criteria) in per_toolpath {
        for status in refusing_unmodeled(criteria) {
            rows.push(format!(
                "  toolpath {id}: {}: {}",
                status.kind.label(),
                unmodeled_clause(status)
            ));
        }
    }
    if rows.is_empty() {
        return Ok(LoadPolicyOutcome::default());
    }
    let any_stale = per_toolpath
        .iter()
        .flat_map(|(_, criteria)| criteria.iter())
        .any(|s| matches!(s.unmodeled_reason, Some(UnmodeledReason::StaleSimulation)));
    let refusal = if any_stale {
        ExportRefusal::StaleSimulation { rows }
    } else {
        ExportRefusal::LoadUnmodeled { rows }
    };
    if !accept_unmodeled {
        return Err(refusal);
    }
    Ok(LoadPolicyOutcome {
        advisories: Vec::new(),
        overridden: vec![refusal],
    })
}

/// Enforce a `ToolLoadExportPolicy` against a report. The refusal names
/// each offending toolpath and criterion and the override as each surface
/// spells it.
///
/// S4/F051: the `Ok` arm is the [`LoadPolicyOutcome`]: the advisories of
/// the exceeded rows that may not refuse, and the refusals an override
/// waived. The export goes ahead and the caller reports both.
///
/// T-19 (2026-09-18): this function decides nothing of its own. It
/// builds the criterion tier once and hands it to both pure halves —
/// [`enforce_exceedance_policy`] and [`enforce_unmodeled_policy`] — so
/// the decision and the message cannot read a criterion from two
/// different places.
pub fn enforce_load_policy(
    report: &crate::tool_load::ToolLoadReport,
    policy: &ToolLoadExportPolicy,
) -> Result<LoadPolicyOutcome, ExportRefusal> {
    let per_toolpath: Vec<(
        crate::ids::ToolpathId,
        Vec<crate::tool_load::verdict::CriterionStatus<'_>>,
    )> = report
        .per_toolpath
        .iter()
        .map(|v| (v.toolpath_id, v.criteria()))
        .collect();
    let mut outcome = enforce_exceedance_policy(&per_toolpath, policy.accept_exceeded)?;
    outcome.merge(enforce_unmodeled_policy(
        &per_toolpath,
        policy.accept_unmodeled,
    )?);
    Ok(outcome)
}

/// The checked emit with no session: the A1/G1 refusals, the load gate,
/// then the program.
///
/// `report` is the tool-load report the gate enforces `policy` against
/// (C1, 2026-06-11). It is deliberately **non-optional** so no caller
/// can silently skip the gate. The session door
/// ([`crate::session::ProjectSession::export_program`]) builds it from
/// the session; the CLI job-file pipeline, which has no session, passes
/// an explicitly empty report (`ToolLoadReport { per_toolpath: vec![] }`),
/// which states at the call site that no evaluation was performed.
///
/// `safe_z` is the inter-setup retract of a [`ExportPhases::Setups`]
/// program (the overlay's `safe_z_override` wins). A
/// [`WizardOverlay::default`] is byte-identical to the no-overlay path.
///
/// Enforces, in order:
/// 1. effective units must be mm (A1 — inch output is a cosmetic G20
///    today; refusing beats a silent 25.4× error),
/// 2. no controller compensation on a post without it (G1),
/// 3. `policy` against `report` (C1 — the tool-load gate).
///
/// This runs no machine-safety pass and no rapid rewrite: both need the
/// session (the retract plane, the machine feed limit, the post config).
/// Every door that holds a session takes `export_program` instead.
pub fn emit_checked(
    phases: &ExportPhases<'_>,
    post: &PostDefinition,
    safe_z: f64,
    report: &crate::tool_load::ToolLoadReport,
    policy: ToolLoadExportPolicy,
    overlay: &WizardOverlay,
) -> Result<CheckedProgram, ExportRefusal> {
    refuse_inch_units(post, overlay)?;
    refuse_controller_compensation(phases.phases().into_iter(), post, overlay)?;
    let load_outcome = enforce_load_policy(report, &policy)?;
    let gcode = match phases {
        ExportPhases::OneSetup(phases) => emit_gcode_phased_with_overlay(phases, post, overlay),
        ExportPhases::Setups(setups) => {
            emit_gcode_multi_setup_with_overlay(setups, post, safe_z, overlay)
        }
    };
    Ok(CheckedProgram {
        gcode,
        load_outcome,
    })
}

/// A1 — inch output is not implemented: a units override to `Inch` only
/// swaps the modal word (G21 → G20) while every coordinate stays in
/// millimeters, a silent 25.4× scale error. Hard-refuse until a real
/// conversion lands (backlogged).
fn refuse_inch_units(post: &PostDefinition, overlay: &WizardOverlay) -> Result<(), ExportRefusal> {
    let effective_units = overlay.units_override.unwrap_or(post.units);
    if effective_units == Units::Inch {
        return Err(ExportRefusal::InchUnits);
    }
    Ok(())
}

/// G1 (SAFETY, 2026-10-08): refuse a phase that asks the controller for
/// cutter compensation (G41/G42) when the effective post has none.
///
/// A Profile with compensation "In Control" follows the part boundary
/// exactly and leaves the offset to the controller. The emitter drops the
/// G41/G42 line on a post with `supports_cutter_comp = false` (the GRBL
/// family). The cut then runs ON the boundary, off by the tool radius.
/// Before G1 the only sign was a WARNING comment in the file.
fn refuse_controller_compensation<'a, 'b: 'a>(
    phases: impl Iterator<Item = &'a GcodePhase<'b>>,
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> Result<(), ExportRefusal> {
    let effective = overlay.applied_post(post);
    if effective.supports_cutter_comp {
        return Ok(());
    }
    let operations: Vec<String> = phases
        .filter(|p| p.controller_compensation.is_some())
        .map(|p| p.label.to_owned())
        .collect();
    if operations.is_empty() {
        return Ok(());
    }
    Err(ExportRefusal::ControllerCompensation {
        post: effective.name.clone(),
        operations,
    })
}

/// The controller-side cutter compensation a toolpath asks for.
///
/// A Profile operation that compensates `InControl` emits G41 or G42.
/// The side and the cut direction together decide which one. Every
/// other operation, and a Profile that compensates in the computer,
/// returns `None`.
///
/// **This is the one mapping.** The viz export door called its own copy
/// until 2026-09-16, so two doors could emit different words for one
/// toolpath.
pub fn controller_compensation_for(
    tc: &crate::session::ToolpathConfig,
) -> Option<ControllerCompensation> {
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::CompensationType;
    use crate::ops::profile::ProfileSide;

    if let OperationConfig::Profile(ref cfg) = tc.operation
        && cfg.compensation == CompensationType::InControl
    {
        return Some(match (cfg.side, cfg.climb) {
            (ProfileSide::Outside, true) => ControllerCompensation::Right,
            (ProfileSide::Outside, false) => ControllerCompensation::Left,
            (ProfileSide::Inside, true) => ControllerCompensation::Left,
            (ProfileSide::Inside, false) => ControllerCompensation::Right,
        });
    }
    None
}

/// A group of toolpath phases belonging to one setup.
#[derive(Clone)]
pub struct GcodeSetupPhase<'a> {
    pub setup_label: &'a str,
    pub phases: Vec<GcodePhase<'a>>,
    /// Optional override for the M0 pause message emitted between this setup
    /// and the previous one (i.e. shown by g-Sender / UGS / CNCjs as the
    /// pause prompt). `None` falls back to `Setup change: <setup_label>`.
    pub pause_message: Option<&'a str>,
}

/// Emit G-code for multiple setups with M0 pauses between them.
///
/// Test-only shim around `emit_gcode_multi_setup_with_overlay(_, _, _, &default)`.
/// Production callers go through [`emit_checked`].
#[cfg(test)]
fn emit_gcode_multi_setup(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
) -> String {
    emit_gcode_multi_setup_with_overlay(setups, post, safe_z, &WizardOverlay::default())
}

/// Same as `emit_gcode_multi_setup`, but applies a `WizardOverlay`. The
/// overlay's `safe_z_override` (when set) replaces the `safe_z` argument
/// for the inter-setup retract; WCS/units/warmup apply as in
/// `emit_gcode_phased_with_overlay`.
pub(crate) fn emit_gcode_multi_setup_with_overlay(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
    overlay: &WizardOverlay,
) -> String {
    let effective_safe_z = overlay.safe_z_override.unwrap_or(safe_z);
    let mut program = program_builder::build_multi_setup_with(
        setups,
        effective_safe_z,
        build_options(post, overlay),
    );
    prepend_t_collision_warnings(
        &mut program,
        setups.iter().flat_map(|s| s.phases.iter()),
        post,
        overlay,
    );
    emitter::emit_program_with_overlay(&program, post, overlay)
}

/// Replace G0 rapid moves with G1 at a high feedrate.
/// Used for machines with unpredictable rapid behavior (e.g., GRBL "dogleg" rapids).
///
/// The inserted feed is clamped to `post.limits.max_feed` when set, and
/// the rewrite is a no-op when `high_feedrate <= 0` (a `F0.0` word would
/// stall the machine).
///
/// Known limits (sound for emitter output, documented for user snippets):
/// - lines after the final `M5` are rewritten too — since the postamble
///   safe-Z retract moved *before* `M5` (C2, 2026-06-11) the shipped
///   posts have no bare `G0` after `M5`, but custom postambles might;
/// - `G00` / lowercase `g0` spellings in user pre/post snippets are not
///   recognized and pass through unchanged.
pub fn replace_rapids_with_feed(gcode: &str, high_feedrate: f64, post: &PostDefinition) -> String {
    if high_feedrate <= 0.0 {
        return gcode.to_owned();
    }
    let feed = post
        .limits
        .max_feed
        .map_or(high_feedrate, |max| high_feedrate.min(max.get()));
    let mut output = String::with_capacity(gcode.len());
    for line in gcode.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("G0 ") || trimmed.starts_with("G0X") {
            // Replace G0 with G1 and append feedrate
            let rest = trimmed.get(2..).unwrap_or("");
            let _ = writeln!(output, "G1{rest} F{feed:.1}");
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Post-processor format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostFormat {
    /// The default dialect. A project file written before the `format`
    /// field existed reads back as GRBL, which is the historic behaviour.
    #[default]
    Grbl,
    /// UI-07: serde WRITES what [`PostFormat::to_token`] writes.
    ///
    /// `rename_all = "snake_case"` used to serialise this as `"grbl_hal"`
    /// while the project-file writer spelled it `"grblhal"`. The project
    /// file now holds the typed enum, so the two spellings must be one; the
    /// canonical form is the one already on disk. F090: the underscore
    /// spelling is no longer read (no alias).
    #[serde(rename = "grblhal")]
    GrblHal,
    #[serde(rename = "linuxcnc")]
    LinuxCnc,
    Mach3,
}

impl PostFormat {
    /// All available post-processor formats.
    pub const ALL: &[PostFormat] = &[
        PostFormat::Grbl,
        PostFormat::GrblHal,
        PostFormat::LinuxCnc,
        PostFormat::Mach3,
    ];

    /// Human-readable display label for UI.
    pub fn label(self) -> &'static str {
        match self {
            PostFormat::Grbl => "GRBL",
            PostFormat::GrblHal => "grblHAL",
            PostFormat::LinuxCnc => "LinuxCNC",
            PostFormat::Mach3 => "Mach3",
        }
    }

    /// Returns the shipped `PostDefinition` for this dialect.
    pub fn definition(self) -> &'static PostDefinition {
        match self {
            PostFormat::Grbl => post::grbl(),
            PostFormat::GrblHal => post::grblhal(),
            PostFormat::LinuxCnc => post::linuxcnc(),
            PostFormat::Mach3 => post::mach3(),
        }
    }

    /// The canonical token this format is written as in a project /
    /// job file. Every writer must go through here — `to_token` and
    /// [`Self::from_token`] are the only two halves of the on-disk
    /// spelling, so they cannot drift apart the way four hand-written
    /// `match` arms did (W9 / P-1).
    pub fn to_token(self) -> &'static str {
        match self {
            PostFormat::Grbl => "grbl",
            PostFormat::GrblHal => "grblhal",
            PostFormat::LinuxCnc => "linuxcnc",
            PostFormat::Mach3 => "mach3",
        }
    }

    /// **The** resolver for a post token read off a CLI flag or a job
    /// file. Case- and whitespace-insensitive; it reads only the
    /// canonical [`Self::to_token`] spellings (F090: the `grbl_hal` and
    /// `linux_cnc` aliases are gone).
    ///
    /// W9 / P-1: three production readers used to open-code this match
    /// and only one of them knew `"grblhal"` — the other two fell
    /// through to `Grbl`, so selecting grblHAL in the GUI, saving and
    /// reloading silently downgraded both the dropdown and the emitted
    /// G-code dialect. Anything that turns a string into a
    /// `PostFormat` must call this rather than re-derive it. Returning
    /// `None` (rather than defaulting) is deliberate: the caller
    /// chooses whether an unknown token is a fallback or an error.
    pub fn from_token(name: &str) -> Option<PostFormat> {
        match name.trim().to_lowercase().as_str() {
            "grbl" => Some(PostFormat::Grbl),
            "grblhal" => Some(PostFormat::GrblHal),
            "linuxcnc" => Some(PostFormat::LinuxCnc),
            "mach3" => Some(PostFormat::Mach3),
            _ => None,
        }
    }
}

/// Post-processor configuration.
///
/// UI-07: this is the ONE post config. `session::ProjectPostConfig` was a
/// second copy with `format` as a `String`, and `GuiState` translated
/// between the two in both directions on every load, save and undo. The
/// project file now holds this type, so the translation pair and the
/// `to_token` / `from_token` round trip through a project file are gone.
///
/// Every field states a default, because a project file written before a
/// field existed does not carry it. F034: a key the config does not hold
/// refuses. F015/F090: an unknown `format` token refuses (it used to read
/// as GRBL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostConfig {
    #[serde(default)]
    pub format: PostFormat,
    #[serde(default = "default_spindle_speed")]
    pub spindle_speed: u32,
    #[serde(default = "default_safe_z")]
    pub safe_z: f64,
    /// Convert G0 rapids to G1 at high feedrate (for machines with unpredictable rapid behavior).
    #[serde(default)]
    pub high_feedrate_mode: bool,
    #[serde(default = "default_high_feedrate")]
    pub high_feedrate: f64,
    /// Project-level spindle policy used by Suggest. `MatchChart`
    /// (default) preserves chart RPM. `MaxSpeed` walks the constant-
    /// chipload line up to the spindle ceiling, scaling feed
    /// proportionally. See [`crate::feeds::SpindleStrategy`].
    #[serde(default)]
    pub spindle_strategy: crate::feeds::SpindleStrategy,
    /// G5: the controller waits for the spindle after M3, so the export
    /// writes no warm-up dwell. `None` reads the machine profile
    /// (`$394` or `$340` above zero).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_waits_for_spindle: Option<bool>,
    /// G6: the board has a mist output, so `M7` is written. `None` reads
    /// the machine profile (`$I` `[OPT:` lists `M`); with no profile fact
    /// the post's own list decides (grblHAL drops M7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mist_output: Option<bool>,
    /// G7: write drill operations as native G81/G82/G83 cycles on a post
    /// that runs them (grblHAL). `None` is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_drill_cycles: Option<bool>,
}

impl PostConfig {
    /// Put the controller options of this config, with the machine
    /// profile's controller facts as the defaults, on `overlay` (G5, G6,
    /// G7, G10). Every export door calls it, so the GUI, MCP and the CLI
    /// emit the same bytes for one project.
    ///
    /// - The spindle wait: this config's value, else the profile's.
    /// - Mist: this config's value, else the profile's `$I` fact, else
    ///   unset (the post's own list decides).
    /// - Native drill cycles: this config's value, else off.
    /// - The S range: the profile's `$31`/`$30`, when both are known.
    /// - `$341=4`: the profile says the board ignores M6.
    pub fn apply_controller_options(
        &self,
        machine: &crate::machine::MachineProfile,
        overlay: &mut WizardOverlay,
    ) {
        let controller = machine.controller.as_ref();
        overlay.controller_waits_for_spindle = self
            .controller_waits_for_spindle
            .unwrap_or_else(|| controller.is_some_and(|c| c.waits_for_spindle()));
        overlay.mist_output = self
            .mist_output
            .or_else(|| controller.and_then(|c| c.mist_output));
        overlay.native_drill_cycles = self.native_drill_cycles.unwrap_or(false);
        overlay.rpm_range = controller.and_then(|c| c.rpm_range());
        overlay.controller_ignores_m6 = controller.is_some_and(|c| c.ignores_m6());
    }
}

fn default_spindle_speed() -> u32 {
    18000
}

fn default_safe_z() -> f64 {
    10.0
}

fn default_high_feedrate() -> f64 {
    5000.0
}

impl Default for PostConfig {
    fn default() -> Self {
        Self {
            format: PostFormat::Grbl,
            spindle_speed: default_spindle_speed(),
            safe_z: default_safe_z(),
            high_feedrate_mode: false,
            high_feedrate: default_high_feedrate(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            controller_waits_for_spindle: None,
            mist_output: None,
            native_drill_cycles: None,
        }
    }
}

/// Get a `PostDefinition` by name (CLI / config-string lookup).
pub fn get_post_definition(name: &str) -> Option<&'static PostDefinition> {
    PostFormat::from_token(name).map(PostFormat::definition)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::ids::ToolpathId;
    use crate::toolpath::Toolpath;

    /// The checked emit of one phase list, as text.
    fn checked_one(
        phases: &[GcodePhase<'_>],
        post: &PostDefinition,
        report: &crate::tool_load::ToolLoadReport,
        policy: ToolLoadExportPolicy,
        overlay: &WizardOverlay,
    ) -> Result<String, ExportRefusal> {
        emit_checked(
            &ExportPhases::OneSetup(phases.to_vec()),
            post,
            0.0,
            report,
            policy,
            overlay,
        )
        .map(|checked| checked.gcode)
    }

    /// The checked emit of setup groups, as text.
    fn checked_setups(
        setups: &[GcodeSetupPhase<'_>],
        post: &PostDefinition,
        safe_z: f64,
        report: &crate::tool_load::ToolLoadReport,
        policy: ToolLoadExportPolicy,
        overlay: &WizardOverlay,
    ) -> Result<String, ExportRefusal> {
        emit_checked(
            &ExportPhases::Setups(setups.to_vec()),
            post,
            safe_z,
            report,
            policy,
            overlay,
        )
        .map(|checked| checked.gcode)
    }

    /// L7 (operator ruling, 2026-09-16): a trace with no `provenance`
    /// block is stale, not fresh.
    ///
    /// The simulator stamps the block on every trace it builds
    /// (`compute/simulate.rs`), so an unstamped trace cannot say what
    /// it simulated. `sim_trace_is_fresh_for` must therefore drop it,
    /// and the gates read `SimulationRequired` rather than being
    /// honoured against a configuration nobody checked.
    ///
    /// The arm returned `true` until L7. `SimulationCutTrace::from_samples`
    /// is the constructor that leaves `provenance` at `None`.
    #[test]
    fn a_trace_without_provenance_is_stale() {
        let project = crate::session::ProjectSession::new_empty();
        let trace = crate::stock::simulation_cut::SimulationCutTrace::from_samples(0.5, Vec::new());
        assert!(trace.provenance.is_none(), "the fixture must be unstamped");
        assert!(
            !sim_trace_is_fresh(&project, &trace),
            "an unstamped trace carries no evidence, so it is stale"
        );
        assert!(
            !sim_trace_is_fresh_for(&project, &trace, 0),
            "a stale trace must not reach any row's gate evaluators"
        );
    }

    /// A toolpath that runs `operation`, with every other field neutral.
    fn toolpath_running(
        operation: crate::compute::OperationConfig,
    ) -> crate::session::ToolpathConfig {
        crate::session::ToolpathConfig {
            shoe: Default::default(),
            id: ToolpathId(0),
            name: "Test Op".to_owned(),
            enabled: true,
            operation,
            dressups: crate::compute::config::DressupConfig::default(),
            heights: crate::compute::config::HeightsConfig::default(),
            tool_id: 0,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            stock_source: crate::compute::config::StockSource::Fresh,
            coolant: CoolantMode::Off,
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            planner_origin: None,
        }
    }

    /// The side and the cut direction pick G41 or G42. Cutter comp in the
    /// computer, and every non-Profile operation, ask the controller for
    /// nothing.
    #[test]
    fn controller_compensation_follows_the_side_and_the_direction() {
        use crate::compute::catalog::OperationConfig;
        use crate::compute::operation_configs::{CompensationType, PocketConfig, ProfileConfig};
        use crate::ops::profile::ProfileSide;

        let profile = |side: ProfileSide, climb: bool, compensation: CompensationType| {
            toolpath_running(OperationConfig::Profile(ProfileConfig {
                side,
                climb,
                compensation,
                ..ProfileConfig::default()
            }))
        };
        let in_control = CompensationType::InControl;

        assert_eq!(
            controller_compensation_for(&profile(ProfileSide::Outside, true, in_control)),
            Some(ControllerCompensation::Right),
        );
        assert_eq!(
            controller_compensation_for(&profile(ProfileSide::Outside, false, in_control)),
            Some(ControllerCompensation::Left),
        );
        assert_eq!(
            controller_compensation_for(&profile(ProfileSide::Inside, true, in_control)),
            Some(ControllerCompensation::Left),
        );
        assert_eq!(
            controller_compensation_for(&profile(ProfileSide::Inside, false, in_control)),
            Some(ControllerCompensation::Right),
        );
        assert!(
            controller_compensation_for(&profile(
                ProfileSide::Outside,
                true,
                CompensationType::InComputer
            ))
            .is_none()
        );
        assert!(
            controller_compensation_for(&toolpath_running(OperationConfig::Pocket(
                PocketConfig::default()
            )))
            .is_none()
        );
    }

    /// "No load evaluation performed" report — what callers without a
    /// `ProjectSession` (CLI job path, fixture captures) pass.
    fn empty_report() -> crate::tool_load::ToolLoadReport {
        crate::tool_load::ToolLoadReport {
            per_toolpath: vec![],
        }
    }

    #[test]
    fn checked_phased_export_is_byte_identical_to_legacy_emitter() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Op 0 — pocket",
                pre_gcode: Some("M8"),
                post_gcode: Some("M9"),
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Mist,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 16_000,
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
                drill: None,
                feeds: None,
            },
        ];

        let legacy = emit_gcode_phased(&phases, post::grbl());
        let checked = checked_one(
            &phases,
            post::grbl(),
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect("checked export should succeed");

        assert_eq!(checked, legacy);
    }

    #[test]
    fn checked_multi_setup_export_is_byte_identical_to_legacy_emitter() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let setups = vec![
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
                    drill: None,
                    feeds: None,
                }],
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: vec![GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 16_000,
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
                    drill: None,
                    feeds: None,
                }],
                pause_message: None,
            },
        ];

        let legacy = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);
        let checked = checked_setups(
            &setups,
            post::grbl(),
            15.0,
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect("checked export should succeed");

        assert_eq!(checked, legacy);
    }

    #[test]
    fn test_grbl_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);

        assert!(gcode.contains("G17 G21 G90"));
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("G0 X0.000 Y0.000 Z10.000"));
        assert!(gcode.contains("G1 X10.000 Y0.000 Z0.000 F1000"));
        // Second G1 should omit F (modal)
        assert!(gcode.contains("G1 X20.000 Y0.000 Z0.000\n"));
        assert!(gcode.contains("M5"));
        assert!(gcode.contains("M30"));
    }

    #[test]
    fn test_mach3_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let gcode = emit_gcode(&tp, post::mach3(), 18000);

        assert!(gcode.contains("G17 G21 G90"));
        assert!(gcode.contains("G4 P2"), "Mach3 should have spindle dwell");
        assert!(gcode.contains("G28 G91 Z0"), "Mach3 should have G28 return");
        assert!(gcode.contains("M30"));
    }

    #[test]
    fn test_grbl_arc_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_cw_to(P3::new(0.0, 10.0, -3.0), -10.0, 0.0, 1000.0);
        tp.arc_ccw_to(P3::new(-10.0, 0.0, -3.0), 0.0, 10.0, 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);

        assert!(gcode.contains("G2 X0.000 Y10.000 Z-3.000 I-10.000 J0.000 F1000"));
        assert!(gcode.contains("G3 X-10.000 Y0.000 Z-3.000 I0.000 J10.000 F1000"));
    }

    #[test]
    fn test_get_post_definition() {
        assert!(get_post_definition("grbl").is_some());
        assert!(get_post_definition("grblhal").is_some());
        assert!(get_post_definition("grbl_hal").is_none());
        assert!(get_post_definition("linuxcnc").is_some());
        assert!(get_post_definition("linux_cnc").is_none());
        assert!(get_post_definition("mach3").is_some());
        assert!(get_post_definition("unknown").is_none());
    }

    /// W9 / P-1. Every token this repo writes must resolve back to the
    /// format that wrote it. `PostFormat::ALL` drives the loop so a
    /// fifth dialect cannot be added with a `to_token` arm and no
    /// `from_token` arm — the exact asymmetry that lost grblHAL.
    #[test]
    fn every_post_token_round_trips_through_the_resolver() {
        for &format in PostFormat::ALL {
            let token = format.to_token();
            assert_eq!(
                PostFormat::from_token(token),
                Some(format),
                "{token:?} did not resolve back to {format:?}"
            );
            assert_eq!(
                get_post_definition(token).map(|d| d.name.as_str()),
                Some(format.definition().name.as_str()),
                "{token:?} resolved to the wrong post definition"
            );
        }
    }

    /// The resolver is case- and whitespace-insensitive. F090: the
    /// underscore spellings are refused (no alias).
    #[test]
    fn the_post_resolver_refuses_the_old_alias_spellings() {
        assert_eq!(
            PostFormat::from_token("  GRBLHAL "),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            PostFormat::from_token("LinuxCNC"),
            Some(PostFormat::LinuxCnc)
        );
        assert_eq!(PostFormat::from_token("grbl_hal"), None);
        assert_eq!(PostFormat::from_token("linux_cnc"), None);
        assert_eq!(PostFormat::from_token("cobalt"), None);
    }

    /// The typed serde wire reads the project-file spelling only. F090:
    /// the `snake_case` alias is refused, and so is an unknown token in a
    /// project post table (it used to read as GRBL, F015).
    #[test]
    fn the_typed_post_wire_reads_one_spelling() {
        assert_eq!(
            serde_json::from_str::<PostFormat>("\"grblhal\"").ok(),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            serde_json::from_str::<PostFormat>("\"linuxcnc\"").ok(),
            Some(PostFormat::LinuxCnc)
        );
        assert!(serde_json::from_str::<PostFormat>("\"grbl_hal\"").is_err());
        assert!(serde_json::from_str::<PostFormat>("\"linux_cnc\"").is_err());
        assert!(toml::from_str::<PostConfig>("format = \"cobalt\"").is_err());
        assert!(toml::from_str::<PostConfig>("formatt = \"grbl\"").is_err());
        let config: PostConfig = toml::from_str("format = \"grblhal\"").unwrap();
        assert_eq!(config.format, PostFormat::GrblHal);
    }

    #[test]
    fn test_emit_gcode_phased_same_spindle() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — pocket",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18000,
                label: "Op 1 — profile",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should have one preamble with M3 S18000
        assert!(gcode.contains("M3 S18000"));
        // Should have both operations' comments
        assert!(gcode.contains("(Op 0"));
        assert!(gcode.contains("(Op 1"));
        // No extra spindle speed change since both are 18000
        let m3_count = gcode.matches("M3 S").count();
        assert_eq!(m3_count, 1, "Same spindle speed should not emit extra M3");
    }

    #[test]
    fn test_emit_gcode_phased_different_spindle() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — rough",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1 — finish",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should have preamble M3 S18000 + later M3 S24000
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("M3 S24000"));
        let m3_count = gcode.matches("M3 S").count();
        assert_eq!(
            m3_count, 2,
            "Different spindle speeds should emit M3 for each"
        );
    }

    #[test]
    fn per_op_spindle_rpm_drives_phase_emission() {
        // End-to-end: building each `GcodePhase`'s `spindle_rpm` via
        // `effective_spindle_rpm` should emit per-toolpath M3 S<rpm>
        // commands in the resulting G-code. TP0 carries an override of
        // 12000; TP1 falls back to the project default of 18000.
        use crate::compute::catalog::{OperationConfig, effective_spindle_rpm};
        use crate::compute::operation_configs::PocketConfig;

        let mut op_override = OperationConfig::Pocket(PocketConfig::default());
        op_override.set_spindle_rpm(Some(12_000));
        let op_default = OperationConfig::Pocket(PocketConfig::default());
        let project_default_rpm: u32 = 18_000;

        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: effective_spindle_rpm(&op_override, project_default_rpm),
                label: "TP0 — override",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: effective_spindle_rpm(&op_default, project_default_rpm),
                label: "TP1 — default",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(
            gcode.contains("M3 S12000"),
            "TP0's override should produce M3 S12000:\n{gcode}"
        );
        assert!(
            gcode.contains("M3 S18000"),
            "TP1 should fall back to project default 18000:\n{gcode}"
        );
    }

    #[test]
    fn test_program_pause_stops_and_pauses() {
        let pause = post::grbl().render_program_pause("Rotate stock");
        assert!(pause.contains("M5"));
        assert!(pause.contains("(Rotate stock)"));
        assert!(pause.contains("M0"));
    }

    #[test]
    fn test_emit_gcode_multi_setup_inserts_pause_between_setups() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let setups = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: vec![GcodePhase {
                    toolpath: &tp1,
                    spindle_rpm: 18_000,
                    label: "Pocket",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: None,
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                    drill: None,
                    feeds: None,
                }],
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: vec![GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 24_000,
                    label: "Profile",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: None,
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                    drill: None,
                    feeds: None,
                }],
                pause_message: None,
            },
        ];

        let gcode = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);

        assert!(gcode.contains("(=== Top ===)"));
        assert!(gcode.contains("(=== Bottom ===)"));
        assert!(gcode.contains("G0 Z15.000"));
        assert!(gcode.contains("(Setup change: Bottom)"));
        assert!(gcode.contains("M0"));
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("M3 S24000"));
    }

    #[test]
    fn test_pre_post_gcode_emitted_in_phased() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Test Op",
            pre_gcode: Some("G55\nG10 L20 P2 X0 Y0 Z0"),
            post_gcode: Some("M9"),
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // pre_gcode should appear after the label comment, before moves
        let label_pos = gcode.find("(Test Op)").expect("label comment");
        let pre_pos = gcode.find("G55").expect("pre_gcode G55");
        let move_pos = gcode.find("G0 X0.000").expect("first rapid move");
        let post_pos = gcode.find("M9").expect("post_gcode M9");
        let postamble_pos = gcode.find("M30").expect("postamble");

        assert!(label_pos < pre_pos, "pre_gcode should follow label comment");
        assert!(pre_pos < move_pos, "pre_gcode should precede moves");
        assert!(move_pos < post_pos, "post_gcode should follow moves");
        assert!(
            post_pos < postamble_pos,
            "post_gcode should precede postamble"
        );

        // Verify multi-line pre_gcode is emitted correctly
        assert!(gcode.contains("G10 L20 P2 X0 Y0 Z0"));
    }

    #[test]
    fn test_pre_post_gcode_emitted_in_multi_setup() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp,
                spindle_rpm: 18_000,
                label: "Pocket",
                pre_gcode: Some("G55"),
                post_gcode: Some("M9"),
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            }],
            pause_message: None,
        }];

        let gcode = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);

        let label_pos = gcode.find("(Pocket)").expect("label comment");
        let pre_pos = gcode.find("G55").expect("pre_gcode G55");
        let move_pos = gcode.find("G0 X0.000").expect("first rapid move");
        let post_pos = gcode.find("M9").expect("post_gcode M9");

        assert!(label_pos < pre_pos);
        assert!(pre_pos < move_pos);
        assert!(move_pos < post_pos);
    }

    #[test]
    fn test_empty_pre_post_gcode_omitted() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Empty",
            pre_gcode: Some(""),
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should not have extra blank lines from empty pre/post
        let lines: Vec<&str> = gcode.lines().collect();
        assert!(
            !lines.iter().any(|l| l.is_empty()),
            "No blank lines from empty pre/post gcode"
        );
    }

    #[test]
    fn test_m6_tool_change_between_phases() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — 6mm endmill",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1 — 3mm ballnose",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tool Two",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        // LinuxCNC's post implements M6 natively, so its tool_change
        // template is the classic M5 + M6 pair.
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        // M6 T2 should appear between the two phases
        assert!(
            gcode.contains("M6 T2"),
            "Should emit M6 T2 for tool change to tool 2"
        );
        // M5 should appear before the tool change (spindle stop)
        let m5_pos = gcode.find("M5\nM6").expect("M5 before M6");
        let m6_pos = gcode.find("M6 T2").expect("M6 T2");
        assert!(m5_pos < m6_pos, "M5 should precede M6");

        // M3 S24000 should appear after the tool change (spindle restart)
        let m3_pos = gcode[m6_pos..].find("M3 S24000").expect("M3 after M6");
        assert!(m3_pos > 0, "M3 should follow M6");

        // No M6 T1 — first tool is assumed already loaded
        assert!(
            !gcode.contains("M6 T1"),
            "First tool should not emit M6 (already loaded)"
        );
    }

    #[test]
    fn test_grbl_tool_change_is_pause_not_m6() {
        // Vanilla Grbl rejects M6 (error:20) — its tool_change template
        // is spindle-off + operator message + M0 pause; the SpindleSet
        // right after doubles as the resume spin-up.
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10610,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(
            !gcode.contains("M6"),
            "Grbl post must not emit M6 (error:20 on vanilla GRBL):\n{gcode}"
        );
        let msg = "(TOOL CHANGE: Tapered Ball 2mm [T2])";
        let msg_pos = gcode.find(msg).expect("operator message");
        let pause_pos = gcode[msg_pos..].find("M0\n").expect("M0 after message") + msg_pos;
        let spinup_pos = gcode[pause_pos..]
            .find("M3 S10610")
            .expect("resume spin-up after pause")
            + pause_pos;
        assert!(msg_pos < pause_pos && pause_pos < spinup_pos);
        // Spindle off precedes the message.
        let m5_pos = gcode[..msg_pos].rfind("M5\n").expect("M5 before message");
        assert!(m5_pos < msg_pos);
    }

    #[test]
    fn test_no_m6_when_same_tool() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18000,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        assert!(!gcode.contains("M6"), "Same tool id should not emit M6");
    }

    #[test]
    fn test_tool_change_fires_on_id_despite_colliding_numbers() {
        // WANAKA regression: two DISTINCT tools both curated as T1.
        // Detection keyed on the display number suppressed the change;
        // keyed on the config id it must fire.
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 12194,
                label: "Rough",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10610,
                label: "Finish",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 1, // SAME display number, different tool
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];

        // M6-style post: change fires with the (colliding) display T1.
        let lcnc = emit_gcode_phased(&phases, post::linuxcnc());
        assert!(
            lcnc.contains("M6 T1"),
            "tool change must fire on id change even when T-numbers collide:\n{lcnc}"
        );

        // Pause-style post: operator message names the incoming tool.
        let grbl = emit_gcode_phased(&phases, post::grbl());
        assert!(
            grbl.contains("(TOOL CHANGE: Tapered Ball 2mm [T1])"),
            "pause-style change must fire on id change:\n{grbl}"
        );
    }

    #[test]
    fn test_coolant_mist_m7() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Mist coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Mist,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M7"), "Mist coolant should emit M7");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");

        // M7 should appear before moves, M9 before postamble
        let m7_pos = gcode.find("M7").expect("M7");
        let move_pos = gcode.find("G0 X0.000").expect("move");
        let m9_pos = gcode.find("M9").expect("M9");
        let m5_pos = gcode.rfind("M5").expect("M5 in postamble");
        assert!(m7_pos < move_pos, "M7 should precede moves");
        assert!(m9_pos < m5_pos, "M9 should precede postamble M5");
    }

    #[test]
    fn test_coolant_flood_m8() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Flood coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Flood,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M8"), "Flood coolant should emit M8");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");
    }

    #[test]
    fn test_coolant_both_m7_m8() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Both coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Both,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M7"), "Both mode should emit M7");
        assert!(gcode.contains("M8"), "Both mode should emit M8");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");
    }

    #[test]
    fn test_coolant_off_no_commands() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "No coolant",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(!gcode.contains("M7"), "Off should not emit M7");
        assert!(!gcode.contains("M8"), "Off should not emit M8");
        assert!(!gcode.contains("M9"), "Off should not emit M9");
    }

    #[test]
    fn test_tool_change_with_coolant() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Flood,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tool Two",
                }),
                coolant: CoolantMode::Mist,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];
        // LinuxCNC: M6-style tool change AND native M7 mist (Grbl's post
        // filters M7 via unsupported_mcodes).
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        // Should have M8 for first phase, M9 before tool change, M6 T2, M3, M7 for second
        assert!(gcode.contains("M8"), "First phase should have M8 flood");
        assert!(gcode.contains("M6 T2"), "Should have tool change");
        assert!(gcode.contains("M7"), "Second phase should have M7 mist");
        // M9 should appear before M6 (coolant off before tool change)
        let m9_pos = gcode.find("M9").expect("M9");
        let m6_pos = gcode.find("M6 T2").expect("M6 T2");
        assert!(
            m9_pos < m6_pos,
            "M9 should precede M6 (coolant off before tool change)"
        );
    }

    // ----- enforce_load_policy negative tests -----

    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, DeflectionBounds,
        SampleEvidence,
    };
    use crate::tool_load::{
        ChiploadVerdict, Confidence, DeflectionVerdict, PowerVerdict, ToolLoadReport,
        ToolpathLoadVerdict, UnmodeledReason,
    };

    fn report_with(
        chipload: ChiploadVerdict,
        power: PowerVerdict,
        deflection: DeflectionVerdict,
    ) -> ToolLoadReport {
        ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(4),
                chipload,
                power,
                deflection,
                // S3: a hand-built export-gate fixture states no
                // measured depth; the row keeps the fixture's posture.
                depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
                material_split: None,
            }],
        }
    }

    fn chip_bounds() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn chipload_within(peak: f64) -> ChiploadVerdict {
        ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
        }
    }

    fn chipload_unmodeled(reason: UnmodeledReason) -> ChiploadVerdict {
        ChiploadVerdict::Unmodeled { reason }
    }

    fn power_not_implemented() -> PowerVerdict {
        PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
        }
    }

    fn deflection_bounds_default() -> DeflectionBounds {
        DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        }
    }

    fn deflection_within(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: deflection_bounds_default(),
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    fn deflection_exceeds(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds: deflection_bounds_default(),
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    #[test]
    fn enforce_blocks_exceeded_by_default() {
        let report = report_with(
            chipload_within(0.05),
            power_not_implemented(),
            deflection_exceeds(10.0),
        );
        let policy = ToolLoadExportPolicy::default();
        let err =
            enforce_load_policy(&report, &policy).expect_err("default policy must block Exceeds");
        let msg = err.to_string();
        assert!(
            msg.contains("toolpath 4"),
            "error names the toolpath: {msg}"
        );
        assert!(
            msg.contains("deflection"),
            "error names the criterion: {msg}"
        );
        assert!(msg.contains("stiffness"), "error names the reason: {msg}");
    }

    #[test]
    fn enforce_lets_exceeded_through_only_with_explicit_flag() {
        // Same scenario, but with accept_exceeded=true. Note unmodeled is
        // also true — if it weren't, the report's unmodeled chipload/power
        // criteria would still block. Verify the two flags are distinct.
        let report = report_with(
            chipload_within(0.05),
            power_not_implemented(),
            deflection_exceeds(10.0),
        );

        // accept_unmodeled alone is NOT enough — Exceeds still blocks.
        let policy_u = ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: false,
        };
        assert!(
            enforce_load_policy(&report, &policy_u).is_err(),
            "accept_unmodeled alone must not bypass Exceeds"
        );

        // accept_exceeded alone bypasses Exceeds, but unmodeled still blocks
        // (chipload+power are NotImplemented).
        let policy_e = ToolLoadExportPolicy {
            accept_unmodeled: false,
            accept_exceeded: true,
        };
        assert!(
            enforce_load_policy(&report, &policy_e).is_err(),
            "Exceeds bypassed but Unmodeled still blocks"
        );

        // Both flags accept everything.
        let policy_both = ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        };
        assert!(enforce_load_policy(&report, &policy_both).is_ok());
    }

    #[test]
    fn enforce_blocks_unmodeled_by_default() {
        let report = report_with(
            chipload_unmodeled(UnmodeledReason::NotImplemented("phase 5".to_owned())),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let err = enforce_load_policy(&report, &ToolLoadExportPolicy::default())
            .expect_err("Unmodeled must block by default");
        let msg = err.to_string();
        assert!(msg.contains("toolpath 4"), "names toolpath: {msg}");
        assert!(msg.contains("chipload"), "names unmodeled criterion: {msg}");
    }

    #[test]
    fn enforce_passes_when_all_within() {
        let report = report_with(
            chipload_within(0.05),
            PowerVerdict::Within {
                peak_kw: 0.5,
                available_kw: 0.71,
                evidence: SampleEvidence::empty(),
                confidence: Confidence::Approximate("isotropic Kc only".into()),
                entry_spike: None,
                bound_source: None,
            },
            deflection_within(3.5),
        );
        assert!(
            enforce_load_policy(&report, &ToolLoadExportPolicy::default()).is_ok(),
            "all-Within report must pass"
        );
    }

    #[test]
    fn enforce_distinguishes_unmodeled_reasons() {
        // Different `Unmodeled` reasons produce different error messages so
        // the user knows whether to "run sim" vs "this material has no LUT".
        let r1 = report_with(
            chipload_unmodeled(UnmodeledReason::SimulationRequired),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let r2 = report_with(
            chipload_unmodeled(UnmodeledReason::NoVendorData),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let m1 = enforce_load_policy(&r1, &ToolLoadExportPolicy::default())
            .expect_err("blocks")
            .to_string();
        let m2 = enforce_load_policy(&r2, &ToolLoadExportPolicy::default())
            .expect_err("blocks")
            .to_string();
        // T-19 (2026-09-18): the claim is unchanged and the words are
        // not. These two lines read `{:?}` — `SimulationRequired` and
        // `NoVendorData`, Rust variant names in an operator's refusal —
        // which is the half of T-19 that printed a blank line for every
        // criterion it did not enumerate. The message now words each
        // reason, so the assertions read the words.
        assert!(m1.contains("simulation has not been run"), "{m1}");
        assert!(
            m2.contains("no vendor LUT row for this tool/material combination"),
            "{m2}"
        );
        assert_ne!(m1, m2, "two reasons, two messages");
    }

    // ── C1: gate enforcement on the phase-level checked exports ──────

    fn power_within() -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw: 0.5,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
            bound_source: None,
        }
    }

    /// Report whose only failure is a modeled `Exceeds` — so only the
    /// `accept_exceeded` flag is needed to override.
    fn exceeds_only_report() -> ToolLoadReport {
        report_with(
            chipload_within(0.05),
            power_within(),
            deflection_exceeds(10.0),
        )
    }

    fn one_phase_fixture() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp
    }

    #[test]
    fn phases_checked_blocks_exceeds_and_honours_accept_exceeded() {
        let tp = one_phase_fixture();
        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18_000,
            label: "Op 0",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let report = exceeds_only_report();

        let err = checked_one(
            &phases,
            post::grbl(),
            &report,
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect_err("default policy must refuse an Exceeds report on the phases path");
        assert!(err.to_string().contains("tool load exceeded"), "{err}");

        let ok = checked_one(
            &phases,
            post::grbl(),
            &report,
            ToolLoadExportPolicy {
                accept_unmodeled: false,
                accept_exceeded: true,
            },
            &WizardOverlay::default(),
        );
        assert!(ok.is_ok(), "accept_exceeded=true must export: {ok:?}");
    }

    #[test]
    fn multi_setup_checked_blocks_exceeds_and_honours_accept_exceeded() {
        let tp = one_phase_fixture();
        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp,
                spindle_rpm: 18_000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            }],
            pause_message: None,
        }];
        let report = exceeds_only_report();

        let err = checked_setups(
            &setups,
            post::grbl(),
            15.0,
            &report,
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect_err("default policy must refuse an Exceeds report on the multi-setup path");
        assert!(err.to_string().contains("tool load exceeded"), "{err}");

        let ok = checked_setups(
            &setups,
            post::grbl(),
            15.0,
            &report,
            ToolLoadExportPolicy {
                accept_unmodeled: false,
                accept_exceeded: true,
            },
            &WizardOverlay::default(),
        );
        assert!(ok.is_ok(), "accept_exceeded=true must export: {ok:?}");
    }

    // ── A1: inch units hard-refuse ────────────────────────────────────

    #[test]
    fn inch_units_override_refuses_export() {
        let tp = one_phase_fixture();
        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18_000,
            label: "Op 0",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
            drill: None,
            feeds: None,
        }];
        let overlay = WizardOverlay {
            units_override: Some(Units::Inch),
            ..Default::default()
        };
        let err = checked_one(
            &phases,
            post::grbl(),
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &overlay,
        )
        .expect_err("inch override must refuse export");
        assert!(
            err.to_string().contains("inch output not yet supported"),
            "{err}"
        );

        // Multi-setup path refuses too.
        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases,
            pause_message: None,
        }];
        let err = checked_setups(
            &setups,
            post::grbl(),
            15.0,
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &overlay,
        )
        .expect_err("inch override must refuse multi-setup export");
        assert!(
            err.to_string().contains("inch output not yet supported"),
            "{err}"
        );
    }

    // ── A7: colliding display T-numbers on an M6 post ────────────────

    #[test]
    fn t_number_collision_warns_on_m6_post_only() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Rough",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10_610,
                label: "Finish",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 1, // SAME display number, different tool
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
                drill: None,
                feeds: None,
            },
        ];

        // M6 post (LinuxCNC): warning comment at program start.
        let lcnc = emit_gcode_phased(&phases, post::linuxcnc());
        assert!(
            lcnc.contains("WARNING: distinct tools share T1"),
            "M6 post must warn on T-number collision:\n{lcnc}"
        );
        assert!(
            lcnc.starts_with("(WARNING"),
            "warning must be prepended at program start:\n{lcnc}"
        );

        // Pause-style post (GRBL): operator message names the tool, no warning.
        let grbl = emit_gcode_phased(&phases, post::grbl());
        assert!(
            !grbl.contains("WARNING: distinct tools share"),
            "pause-style post must not warn:\n{grbl}"
        );

        // Distinct numbers on an M6 post: no warning.
        let mut distinct = phases;
        if let Some(p) = distinct.get_mut(1)
            && let Some(t) = p.tool.as_mut()
        {
            t.number = 2;
        }
        let lcnc2 = emit_gcode_phased(&distinct, post::linuxcnc());
        assert!(!lcnc2.contains("WARNING: distinct tools share"));
    }

    /// G1 (SAFETY): a phase with controller compensation is refused on a
    /// post without cutter compensation, on both checked doors. A post
    /// with compensation still emits G41/G42.
    #[test]
    fn controller_compensation_is_refused_without_post_support_g1() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 5.0));
        tp.feed_to(P3::new(0.0, 0.0, -2.0), 300.0);
        tp.feed_to(P3::new(20.0, 0.0, -2.0), 600.0);
        let phase = || GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18_000,
            label: "Outer profile",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: Some(ControllerCompensation::Right),
            drill: None,
            feeds: None,
        };
        let policy = ToolLoadExportPolicy::default();
        for post_def in [post::grbl(), post::grblhal()] {
            let err = checked_one(
                &[phase()],
                post_def,
                &empty_report(),
                policy,
                &WizardOverlay::default(),
            )
            .expect_err("a post without comp must refuse In Control");
            let text = err.to_string();
            assert!(text.contains("Outer profile"), "names the op: {text}");
            assert!(text.contains("tool radius"), "says why: {text}");

            let setups = [GcodeSetupPhase {
                setup_label: "Top",
                phases: vec![phase()],
                pause_message: None,
            }];
            assert!(
                checked_setups(
                    &setups,
                    post_def,
                    10.0,
                    &empty_report(),
                    policy,
                    &WizardOverlay::default()
                )
                .is_err(),
                "{}: the multi-setup door must refuse too",
                post_def.name
            );
        }
        for post_def in [post::linuxcnc(), post::mach3()] {
            let gcode = checked_one(
                &[phase()],
                post_def,
                &empty_report(),
                policy,
                &WizardOverlay::default(),
            )
            .expect("a post with comp emits");
            assert!(gcode.contains("G42 D1"), "{}: {gcode}", post_def.name);
        }
        // A phase without controller compensation is never refused.
        let mut plain = phase();
        plain.controller_compensation = None;
        assert!(
            checked_one(
                &[plain],
                post::grbl(),
                &empty_report(),
                policy,
                &WizardOverlay::default()
            )
            .is_ok()
        );
    }
}
