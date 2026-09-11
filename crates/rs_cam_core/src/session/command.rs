//! The command registry, and the one door that applies a command or a
//! query.
//!
//! A command is a named, validated mutation of the session. A query is a
//! named, synchronous read. One X-macro list declares every row of both
//! kinds. A callback macro splits the rows by kind and builds the payload
//! enums [`Command`] and [`Query`], the fieldless mirror [`CommandId`],
//! and the per-row columns — the wire name, the kind, the answer type, and
//! the surface table. The idiom is `for_each_op!`
//! (`crates/rs_cam_core/src/compute/catalog.rs:149`).
//!
//! [`ProjectSession::apply`] runs one command and reports [`Effects`] —
//! the toolpath indices the command dropped, whether it cleared the
//! simulation, and the revision of the toolpath it names. One producer
//! answers every surface, so the MCP reply and the core drop cannot
//! disagree. [`ProjectSession::query`] runs one query and reports one
//! [`QueryAnswer`] variant, named after the row.
//!
//! WP3 adds the one construction site for [`Effects`]. Every mutation on
//! [`ProjectSession`] runs its body inside
//! [`ProjectSession::try_with_effects`], so no mutation builds an
//! [`Effects`] of its own.
//!
//! WP9 adds the `Query` kind and its first row, `ToolpathCycleTime`.
//! Later work packages add the rest.
//!
//! WP8 adds `RestoreToolpathSnapshot`, the door the GUI undo stack and
//! the GUI redo stack take. It is the first row that invalidates
//! UNCONDITIONALLY by contract: it compares nothing, so a restore of a
//! byte-identical configuration still drops the chain.
//!
//! WP5 adds `ReplaceToolpathConfig`, the door the GUI inspector takes. It
//! is the first row that GATES its invalidation: the panel applies it on
//! every frame it is open, so the row always writes the configuration and
//! drops the chain only when
//! [`ToolpathConfig::generation_inputs_signature`] moved. The two rows
//! answer two different questions, and neither can serve the other.
//!
//! WP10 adds the `Job` kind and its first row, `GenerateToolpath`. A
//! job runs three synchronous steps and never an `async fn`:
//! [`ProjectSession::start`] captures on the frame loop and answers a
//! [`JobHandle`]; a free function runs the work off the loop holding no
//! session; `apply(Command::AdoptResult { .. })` records the answer.
//! The kind cost the callback macro a third accumulator and one more
//! per-row arm, not a rewrite.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::cycle_time::{self, CycleTime};
use super::{
    GenerateToolpathHandle, ProjectSession, SessionError, ToolpathComputeResult, ToolpathConfig,
};
use crate::compute::catalog::OperationConfig;
use crate::compute::config::DressupConfig;
use crate::enriched_mesh::FaceGroupId;
use crate::feeds::FeedsProvenance;
use crate::simulation_cut::SimulationCutTrace;

/// Declares every command, query and job row once.
///
/// The columns are: the row kind (`Command`, `Query` or `Job`), the
/// identifier, the wire name, the payload type, the answer type, and the
/// surface table. A `Command` row's answer type is always [`Effects`]; a
/// `Query` row names its own answer struct; a `Job` row names the HANDLE
/// its three steps share. Edit this list, not the blocks a callback
/// macro generates from it.
///
/// The macro carries `#[macro_export]` because a sentry in `tests/`
/// counts the rows with its own callback. `for_each_op!` needs no export:
/// its completeness sentry is an in-file unit test.
#[macro_export]
macro_rules! for_each_command {
    ($m:ident) => {
        $m! {
            //  kind      id                wire name             payload
            //  answer / handle     surfaces
            (Command, SetToolpathParam, "set_toolpath_param", SetToolpathParamArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, AdoptResult, "adopt_result", AdoptResultArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "a completion is adopted by the GUI drain, not by a wire tool",
                 ),
                 cli: Reach::Skip(
                     "the CLI generates synchronously and never adopts a completion",
                 ),
             }),
            (Query, ToolpathCycleTime, "toolpath_cycle_time", ToolpathCycleTimeArgs,
             ToolpathCycleTimeAnswer,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "get_cut_trace and narrate_toolpath report other quantities; WP4 revisits",
                 ),
                 cli: Reach::Skip(
                     "the CLI project report prints the simulation total, not per-toolpath",
                 ),
             }),
            (Command, RestoreToolpathSnapshot, "restore_toolpath_snapshot",
             RestoreToolpathSnapshotArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "undo and redo are GUI actions; MCP has no history",
                 ),
                 cli: Reach::Skip("the CLI holds no undo history"),
             }),
            (Command, ReplaceToolpathConfig, "replace_toolpath_config",
             ReplaceToolpathConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the MCP door edits one named parameter through set_toolpath_param",
                 ),
                 cli: Reach::Skip(
                     "the CLI writes a whole job file, not a live config",
                 ),
             }),
            (Job, GenerateToolpath, "generate_toolpath", GenerateToolpathArgs,
             GenerateToolpathHandle,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
        }
    };
}

/// How the session runs a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// A synchronous validated mutation. It returns [`Effects`].
    Command,
    /// A synchronous read. It changes nothing.
    Query,
    /// Three synchronous steps: capture, run off the frame loop, adopt.
    Job,
    /// A view command. It never enters the core session.
    UiCommand,
}

/// Whether one surface reaches a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The surface calls the command.
    Reached,
    /// The surface does not call the command. The text says why.
    Skip(&'static str),
}

/// The surfaces of one command row.
///
/// This is a struct, not a list. A literal that omits a field does not
/// compile, so a forgotten surface is a build error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surfaces {
    /// The desktop application.
    pub gui: Reach,
    /// The MCP server the desktop application embeds.
    pub mcp: Reach,
    /// The batch command line.
    pub cli: Reach,
}

/// The arguments of the `set_toolpath_param` command.
#[derive(Debug, Clone, PartialEq)]
pub struct SetToolpathParamArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The name of the operation parameter.
    pub param: String,
    /// The value to write.
    pub value: serde_json::Value,
}

/// The arguments of the `adopt_result` command.
///
/// A generation runs off the frame loop, so the configuration can move
/// while it runs. `revision` is the generation-input revision the lane
/// started from, read with
/// [`ProjectSession::toolpath_revision`](super::ProjectSession::toolpath_revision).
/// [`ProjectSession::apply`] compares it against the revision the
/// toolpath carries now and refuses a mismatch with
/// [`SessionError::StaleCompletion`], inserting nothing.
///
/// `result` is boxed. A [`ToolpathComputeResult`] carries the whole
/// annotated toolpath, its statistics and two optional traces, which is
/// hundreds of bytes beside the other rows' arguments.
#[derive(Debug, Clone)]
pub struct AdoptResultArgs {
    /// The index of the toolpath the completion answers.
    pub index: usize,
    /// The revision the compute lane started from.
    pub revision: u64,
    /// The computed result.
    pub result: Box<ToolpathComputeResult>,
}

/// The arguments of the `restore_toolpath_snapshot` command.
///
/// The GUI undo stack, the GUI redo stack and the three optimizer apply
/// paths all install a captured `(operation, dressups, face_selection)`
/// triple on one toolpath. The optimizer paths also stamp a feeds
/// provenance on the dimensions they changed, which is why the payload
/// carries one: before WP8 they wrote it in a second call, and an undo
/// then restored the earlier values under the later stamp.
///
/// **The command invalidates UNCONDITIONALLY, by contract.** It compares
/// nothing. A restore of the byte-identical configuration the cached
/// geometry was generated from still drops the chain (F2.5). Nothing
/// records which configuration a cached result answers; the snapshot
/// restores three of the nine fields that decide the geometry; and being
/// wrong this way costs one regeneration, while being wrong the other
/// way exports a program that does not match the project. A
/// signature-gated command cannot serve undo, which is why this row is
/// not `ReplaceToolpathConfig`.
///
/// Three fields are boxed. [`Command`] is one enum, so its size is the
/// size of its largest variant, and an operation configuration, a
/// dressup configuration and six provenance slots together run to
/// several hundred bytes beside the other rows' arguments.
/// `large_enum_variant` is denied.
#[derive(Debug, Clone)]
pub struct RestoreToolpathSnapshotArgs {
    /// The index of the toolpath the command restores.
    pub index: usize,
    /// The operation configuration to install.
    pub operation: Box<OperationConfig>,
    /// The dressup configuration to install.
    pub dressups: Box<DressupConfig>,
    /// The BREP face selection to install.
    pub face_selection: Option<Vec<FaceGroupId>>,
    /// The feeds provenance to install.
    ///
    /// `None` means the caller restores no provenance. The toolpath
    /// keeps the provenance it carries, and the command clears nothing.
    pub feeds_provenance: Option<Box<FeedsProvenance>>,
}

/// The arguments of the `replace_toolpath_config` command.
///
/// The GUI inspector holds no commit event. It rebuilds an owned entry
/// from the session, lets the widgets write it, and writes the entry back
/// on every frame the panel is open. So this command runs on every such
/// frame, and the gate below — not the panel — decides whether the cached
/// geometry survives.
///
/// **The command always writes the configuration, and invalidates only
/// when [`ToolpathConfig::generation_inputs_signature`] moved.** The two
/// halves are independent. Name, coolant, the pre and post G-code and the
/// debug options sit OUTSIDE the signature: they dirty the project and
/// change no motion, so they must land on a frame that drops nothing. A
/// return before the write would stop them landing.
///
/// This row therefore differs from
/// [`RestoreToolpathSnapshotArgs`], which invalidates unconditionally by
/// contract. A restore compares nothing because nothing records which
/// configuration a cached result answers; a replacement carries the whole
/// configuration, so the comparison is available and correct.
///
/// The configuration is boxed. [`Command`] is one enum, so its size is
/// the size of its largest variant, and a [`ToolpathConfig`] carries an
/// operation configuration, a dressup configuration, a heights
/// configuration and six provenance slots. `large_enum_variant` is
/// denied.
#[derive(Debug, Clone)]
pub struct ReplaceToolpathConfigArgs {
    /// The index of the toolpath the command replaces.
    pub index: usize,
    /// The configuration to install, in full.
    pub config: Box<ToolpathConfig>,
}

/// The arguments of the `generate_toolpath` job.
///
/// The payload names the toolpath and nothing else. Every generation
/// input comes from the session at step (i), so a caller cannot hand
/// one in: that is what makes the resolver the only assembly
/// (`planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §1).
/// The payload derives no `Copy`. [`ProjectSession::start`] matches on
/// the [`Job`] it is handed, and a `Copy` payload would make that match
/// a read rather than a move — the argument would then be passed by
/// value and never consumed, which `needless_pass_by_value` denies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateToolpathArgs {
    /// The index of the toolpath to generate.
    pub index: usize,
}

/// What a command changed.
///
/// The type carries `#[must_use]`. A caller that runs a mutation and
/// drops the answer states that it drops it, with `let _ =`.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    /// Every toolpath index whose generation-input revision moved.
    ///
    /// The set comes from the revision map, not from the `drop_result`
    /// call list. `bump_all_revisions`
    /// (`crates/rs_cam_core/src/session/mutation.rs:1339`) moves every
    /// index on a removal and calls `drop_result` on none of them, so a
    /// call list reports nothing stale for a removal.
    pub stale: BTreeSet<usize>,
    /// `true` when a simulation result existed before the command and
    /// does not exist after it.
    ///
    /// The chain writes `None` unconditionally, so the write alone is not
    /// evidence that a simulation existed.
    pub simulation_cleared: bool,
    /// The generation-input revision of the toolpath the command names,
    /// read after the mutation.
    ///
    /// This is [`ProjectSession::toolpath_revision`], never
    /// `next_revision`. `next_revision` is a session-global counter that
    /// any index bumps.
    ///
    /// `Some` only when the command names ONE toolpath that still exists
    /// at the SAME index after the command. A bulk mutation, a
    /// setup-index mutation, a removal and a re-order all report `None`,
    /// which means NOT MEASURED. `0` cannot carry that meaning: every
    /// toolpath starts at revision `0`.
    pub revision: Option<u64>,
}

/// The arguments of the `toolpath_cycle_time` read.
///
/// `trace` carries the simulation the caller measured this toolpath
/// against. The session's own `simulation_result()` is a different,
/// usually-empty slot: the GUI simulates off the frame loop and keeps its
/// cut trace in its own state, so a query that read only the session's
/// slot would answer `CuttingOnly` or nothing on every real project. The
/// caller therefore supplies the trace it already holds, the same
/// argument [`cycle_time::toolpath_cycle_time`] always took.
///
/// `cutting_distance_mm` and `nominal_feed_mm_min` feed the `CuttingOnly`
/// fallback, which needs both and neither is on the trace. `None` means
/// the caller has no computed result for this toolpath yet, in which case
/// the read cannot fall back and answers [`CycleTime::NONE`].
#[derive(Debug, Clone)]
pub struct ToolpathCycleTimeArgs {
    /// The index of the toolpath to read.
    pub index: usize,
    /// The cut trace to search for this toolpath's measured runtime.
    pub trace: Option<Arc<SimulationCutTrace>>,
    /// The toolpath's cutting distance, in millimetres.
    pub cutting_distance_mm: Option<f64>,
    /// The toolpath's nominal feed rate, in millimetres per minute.
    pub nominal_feed_mm_min: Option<f64>,
}

/// The answer to the `toolpath_cycle_time` read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolpathCycleTimeAnswer {
    /// The toolpath's cycle time, and the basis it was measured on.
    pub cycle_time: CycleTime,
}

/// Splits the registry's rows by kind, and emits the [`Command`],
/// [`Query`] and [`Job`] payload enums plus [`QueryAnswer`] and
/// [`JobHandle`].
///
/// GENERATED support macro for `define_command_registry!`, called with
/// the same rows that macro receives. It walks the row list one row at a
/// time — an incremental "tt-muncher" — sorting each row's columns into
/// the `commands`, `queries` or `jobs` accumulator by matching the
/// literal identifier `Command`, `Query` or `Job` in the row's first
/// column. The base case (no rows left) is tried first on every
/// recursive call, so it fires the moment the accumulators hold every
/// row and none remains.
///
/// WP10 added the third accumulator and its per-row arm. A future
/// `UiCommand` row needs one more of each — the shape does not need a
/// rewrite to grow another kind.
macro_rules! split_command_rows {
    // Base case: no rows left. Emit the three payload enums and their
    // `id()`, from the three accumulators built by the arms below.
    (@split
        commands = [$( ($c_id:ident, $c_wire:literal, $c_payload:ident) ),* $(,)?],
        queries = [
            $( ($q_id:ident, $q_wire:literal, $q_payload:ident, $q_answer:ty) ),* $(,)?
        ],
        jobs = [
            $( ($j_id:ident, $j_wire:literal, $j_payload:ident, $j_handle:ty) ),* $(,)?
        ] $(,)?
    ) => {
        /// One command and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        ///
        /// The enum derives no `PartialEq`. `AdoptResultArgs` carries a
        /// [`ToolpathComputeResult`], whose parts publish no equality —
        /// a computed toolpath answers "is this the current answer?"
        /// through its revision, not through a comparison.
        #[derive(Debug, Clone)]
        pub enum Command {
            $(
                #[doc = concat!("The `", $c_wire, "` command.")]
                $c_id($c_payload),
            )*
        }

        impl Command {
            /// The identifier of this command. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Command::$c_id(_) => CommandId::$c_id,)*
                }
            }
        }

        /// One read and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        #[derive(Debug, Clone)]
        pub enum Query {
            $(
                #[doc = concat!("The `", $q_wire, "` read.")]
                $q_id($q_payload),
            )*
        }

        impl Query {
            /// The identifier of this read. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Query::$q_id(_) => CommandId::$q_id,)*
                }
            }
        }

        /// The answer to one [`Query`].
        ///
        /// GENERATED — one variant per `Query` row, named by that row's
        /// answer column.
        #[derive(Debug, Clone)]
        pub enum QueryAnswer {
            $(
                #[doc = concat!("The answer to the `", $q_wire, "` read.")]
                $q_id($q_answer),
            )*
        }

        /// One job and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        ///
        /// A job runs three synchronous steps, never an `async fn`:
        /// [`ProjectSession::start`] captures on the frame loop and
        /// answers a [`JobHandle`], a free function runs the work off
        /// the loop holding no session, and
        /// `apply(Command::AdoptResult { .. })` records the answer.
        #[derive(Debug, Clone)]
        pub enum Job {
            $(
                #[doc = concat!("The `", $j_wire, "` job.")]
                $j_id($j_payload),
            )*
        }

        impl Job {
            /// The identifier of this job. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Job::$j_id(_) => CommandId::$j_id,)*
                }
            }
        }

        /// What [`ProjectSession::start`] captured for one [`Job`].
        ///
        /// GENERATED — one variant per `Job` row, named by that row's
        /// answer column. The handle is the whole input of step (ii),
        /// so step (ii) needs no session.
        ///
        /// The enum derives nothing. A handle owns a
        /// [`ResolvedGenInputs`](super::ResolvedGenInputs), which
        /// publishes neither equality nor a clone: a resolution is the
        /// evidence of one submit, and a copy of it would be a second
        /// assembly of the same inputs.
        pub enum JobHandle {
            $(
                #[doc = concat!("What the `", $j_wire, "` job captured.")]
                $j_id($j_handle),
            )*
        }
    };

    // Next row is a `Command` row: file it under `commands` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Command, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)* ($id, $wire, $payload),],
            queries = [$($queries)*],
            jobs = [$($jobs)*],
            $($rest)*
        }
    };

    // Next row is a `Query` row: file it under `queries` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Query, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)*],
            queries = [$($queries)* ($id, $wire, $payload, $answer),],
            jobs = [$($jobs)*],
            $($rest)*
        }
    };

    // Next row is a `Job` row: file it under `jobs` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Job, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)*],
            queries = [$($queries)*],
            jobs = [$($jobs)* ($id, $wire, $payload, $answer),],
            $($rest)*
        }
    };
}

macro_rules! define_command_registry {
    (
        $( ($kind:ident, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr) ),+
        $(,)?
    ) => {
        /// The identifier of one command or read, without its arguments.
        ///
        /// The registry columns hang here. A payload enum cannot host a
        /// `const ALL`. GENERATED from the `for_each_command!` list, over
        /// every row regardless of kind — `ALL` and its methods answer
        /// for a `Command` row and a `Query` row alike.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CommandId {
            $(
                #[doc = concat!("The identifier of the `", $wire, "` row.")]
                $id,
            )+
        }

        impl CommandId {
            /// Every row, in registry order. GENERATED.
            pub const ALL: &[CommandId] = &[$(CommandId::$id,)+];

            /// The name this row carries on the wire. GENERATED.
            pub fn wire_name(self) -> &'static str {
                match self {
                    $(CommandId::$id => $wire,)+
                }
            }

            /// How the session runs this row. GENERATED.
            pub fn kind(self) -> CommandKind {
                match self {
                    $(CommandId::$id => CommandKind::$kind,)+
                }
            }

            /// Which surfaces reach this row, and why one does not.
            /// GENERATED.
            pub fn surfaces(self) -> Surfaces {
                match self {
                    $(CommandId::$id => $surfaces,)+
                }
            }
        }

        // The payload and answer enums need one row list per kind, so the
        // kind-agnostic block above hands the same rows to the splitter.
        split_command_rows! {
            @split
            commands = [],
            queries = [],
            jobs = [],
            $( ($kind, $id, $wire, $payload, $answer, $surfaces), )+
        }
    };
}
for_each_command!(define_command_registry);

impl ProjectSession {
    /// Apply one command, and report what it changed.
    ///
    /// This is the one door. Every surface that runs a command takes it,
    /// so every surface reads the same [`Effects`].
    ///
    /// The method measures `stale` generally. It reads every toolpath
    /// revision before the mutation, runs the mutation, and reports every
    /// index whose revision moved. It does not read the invalidation
    /// chain's own drop list.
    pub fn apply(&mut self, command: Command) -> Result<Effects, SessionError> {
        match command {
            Command::SetToolpathParam(args) => {
                let index = args.index;
                self.try_with_effects(Some(index), move |session| {
                    session.set_toolpath_param_impl(index, &args.param, args.value)
                })
            }
            Command::AdoptResult(args) => {
                let AdoptResultArgs {
                    index,
                    revision,
                    result,
                } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                let current = self.toolpath_revision(index);
                if current != revision {
                    return Err(SessionError::StaleCompletion {
                        index,
                        submitted: revision,
                        current,
                    });
                }
                self.try_with_effects(Some(index), move |session| {
                    session.insert_result(index, *result)
                })
            }
            Command::RestoreToolpathSnapshot(args) => {
                let RestoreToolpathSnapshotArgs {
                    index,
                    operation,
                    dressups,
                    face_selection,
                    feeds_provenance,
                } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                Ok(self.with_effects(Some(index), move |session| {
                    if let Some(tc) = session.toolpath_configs.get_mut(index) {
                        tc.operation = *operation;
                        tc.dressups = *dressups;
                        tc.face_selection = face_selection;
                        if let Some(provenance) = feeds_provenance {
                            tc.feeds_provenance = *provenance;
                        }
                    }
                    // Unconditional by contract. The chain call runs
                    // whether or not the restore moved a field: a
                    // byte-identical restore still drops the chain
                    // (F2.5). The index is checked above, so the read
                    // below answers `Some`.
                    let enabled = session
                        .toolpath_configs
                        .get(index)
                        .is_some_and(|tc| tc.enabled);
                    session.invalidate_result_chain(index, enabled);
                }))
            }
            Command::ReplaceToolpathConfig(args) => {
                let ReplaceToolpathConfigArgs { index, config } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                Ok(self.with_effects(Some(index), move |session| {
                    let after = config.generation_inputs_signature();
                    let Some(slot) = session.toolpath_configs.get_mut(index) else {
                        return;
                    };
                    let before = slot.generation_inputs_signature();
                    // The write is unconditional. Name, coolant, the pre
                    // and post G-code and the debug options are outside
                    // the signature, so they must land on a frame that
                    // drops nothing.
                    *slot = *config;
                    if after == before {
                        return;
                    }
                    // A generation input moved, so the cached geometry
                    // answers a configuration that is gone. `enabled` is
                    // read after the write, because the replacement can
                    // move it and the chain walk reads the new value.
                    let enabled = session
                        .toolpath_configs
                        .get(index)
                        .is_some_and(|tc| tc.enabled);
                    session.invalidate_result_chain(index, enabled);
                }))
            }
        }
    }

    /// Run one read, and report its answer.
    ///
    /// This is the read door, alongside [`Self::apply`] for mutations. A
    /// query changes nothing, so it takes `&self`.
    pub fn query(&self, query: Query) -> Result<QueryAnswer, SessionError> {
        match query {
            Query::ToolpathCycleTime(args) => {
                let ToolpathCycleTimeArgs {
                    index,
                    trace,
                    cutting_distance_mm,
                    nominal_feed_mm_min,
                } = args;
                let id = self
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| tc.id)
                    .ok_or(SessionError::ToolpathNotFound(index))?;
                // Both fields must be `Some`, or the `CuttingOnly` fallback
                // would divide a real feed by an unmeasured distance
                // coerced to zero — a clean-looking answer over a value
                // that was never measured. Zeroing both instead keeps the
                // fallback's own `nominal_feed_mm_min > 0.0` check honest:
                // it reads no evidence and answers `CycleTime::NONE`.
                let (cutting_distance_mm, nominal_feed_mm_min) =
                    match (cutting_distance_mm, nominal_feed_mm_min) {
                        (Some(distance), Some(feed)) => (distance, feed),
                        _ => (0.0, 0.0),
                    };
                let cycle_time = cycle_time::toolpath_cycle_time(
                    trace.as_deref(),
                    id,
                    cutting_distance_mm,
                    nominal_feed_mm_min,
                );
                Ok(QueryAnswer::ToolpathCycleTime(ToolpathCycleTimeAnswer {
                    cycle_time,
                }))
            }
        }
    }

    /// Start one job, and report the handle its steps share.
    ///
    /// This is step (i), and it is the third door beside [`Self::apply`]
    /// and [`Self::query`]. A job runs three synchronous steps and never
    /// an `async fn`:
    ///
    /// 1. `start` holds `&mut self` and runs on the frame loop. It drops
    ///    the cached result, checks every precondition, resolves the
    ///    inputs and captures the session reads the work needs. A refusal
    ///    therefore appears at submit time.
    /// 2. A free function runs the work. It reads the handle and holds no
    ///    session, so it runs off the frame loop.
    ///    [`execute_job`](super::execute_job) is that function for the
    ///    `generate_toolpath` row.
    /// 3. `apply(Command::AdoptResult { .. })` records the answer at the
    ///    revision the handle carries. An edit between step (1) and step
    ///    (3) moves that revision and the adopt refuses.
    ///
    /// `cancel` reaches the resolver, which does real geometric work for
    /// one boundary source: a
    /// [`BoundarySource::PlannedTierRegions`](crate::compute::config::BoundarySource::PlannedTierRegions)
    /// boundary walks a full-grid tier map. A `start` that resolved that
    /// map under a flag of its own would make Cancel a lie for the whole
    /// of it.
    pub fn start(&mut self, job: Job, cancel: &AtomicBool) -> Result<JobHandle, SessionError> {
        match job {
            Job::GenerateToolpath(args) => {
                let handle = self.start_generate_toolpath(args.index, cancel)?;
                Ok(JobHandle::GenerateToolpath(handle))
            }
        }
    }

    /// Run one mutation and report what it changed.
    ///
    /// **The one construction site of [`Effects`].** The method reads
    /// every toolpath revision and the simulation before `mutate`, runs
    /// `mutate`, and reports the difference. A mutation therefore states
    /// what it changed by changing it, and no two mutations can carry two
    /// staleness models.
    ///
    /// `index` names the ONE toolpath the mutation is about, for
    /// [`Effects::revision`]. Pass `None` from a bulk mutation, from a
    /// setup-index mutation, and from a mutation that moves or removes
    /// the toolpath the index named. An index that names no toolpath
    /// after the mutation also reports `None`.
    ///
    /// An error from `mutate` propagates, and the method reports no
    /// effects.
    pub(crate) fn try_with_effects<E>(
        &mut self,
        index: Option<usize>,
        mutate: impl FnOnce(&mut Self) -> Result<(), E>,
    ) -> Result<Effects, E> {
        let simulation_before = self.simulation.is_some();
        let revisions_before = self.revision_snapshot();
        mutate(self)?;
        Ok(Effects {
            stale: self.moved_revisions(&revisions_before),
            simulation_cleared: simulation_before && self.simulation.is_none(),
            revision: index
                .filter(|i| *i < self.toolpath_count())
                .map(|i| self.toolpath_revision(i)),
        })
    }

    /// Run one mutation that cannot fail, and report what it changed.
    ///
    /// The infallible half of [`Self::try_with_effects`], which builds
    /// the [`Effects`]. This method builds none of its own.
    pub(crate) fn with_effects(
        &mut self,
        index: Option<usize>,
        mutate: impl FnOnce(&mut Self),
    ) -> Effects {
        self.try_with_effects(index, |session| {
            mutate(session);
            Ok::<(), std::convert::Infallible>(())
        })
        // SAFETY: `Infallible` holds no value, so the closure below has
        // no case to answer. The compiler proves the error arm cannot
        // happen. This is not `unwrap`: nothing can panic here.
        .unwrap_or_else(|never| match never {})
    }

    /// Read every toolpath's generation-input revision, in index order.
    fn revision_snapshot(&self) -> Vec<u64> {
        (0..self.toolpath_count())
            .map(|index| self.toolpath_revision(index))
            .collect()
    }

    /// Every index whose revision differs from the snapshot.
    ///
    /// The walk covers both lengths, because a command that removes a
    /// toolpath shortens the list. An index the snapshot does not carry
    /// counts as moved.
    fn moved_revisions(&self, before: &[u64]) -> BTreeSet<usize> {
        let count = self.toolpath_count().max(before.len());
        let mut moved = BTreeSet::new();
        for index in 0..count {
            if before.get(index).copied() != Some(self.toolpath_revision(index)) {
                moved.insert(index);
            }
        }
        moved
    }
}
