//! The command registry, and the one door that applies a command.
//!
//! A command is a named, validated mutation of the session. One X-macro
//! list declares every command row. A callback macro turns the rows into
//! the payload enum [`Command`], the fieldless mirror [`CommandId`], and
//! the per-row columns — the wire name, the kind, and the surface table.
//! The idiom is `for_each_op!`
//! (`crates/rs_cam_core/src/compute/catalog.rs:149`).
//!
//! [`ProjectSession::apply`] runs one command and reports [`Effects`] —
//! the toolpath indices the command dropped, whether it cleared the
//! simulation, and the revision of the toolpath it names. One producer
//! answers every surface, so the MCP reply and the core drop cannot
//! disagree.
//!
//! WP1 carries one row, `SetToolpathParam`. The later work packages add
//! the rest.

use std::collections::BTreeSet;

use super::{ProjectSession, SessionError};

/// Declares every command row once.
///
/// The columns are: the command kind, the identifier, the wire name, the
/// payload type, and the surface table. Edit this list, not the blocks a
/// callback macro generates from it.
///
/// The macro carries `#[macro_export]` because a sentry in `tests/`
/// counts the rows with its own callback. `for_each_op!` needs no export:
/// its completeness sentry is an in-file unit test.
#[macro_export]
macro_rules! for_each_command {
    ($m:ident) => {
        $m! {
            //  kind      id                wire name             payload
            //  surfaces
            (Command, SetToolpathParam, "set_toolpath_param", SetToolpathParamArgs,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector writes ToolpathConfig fields directly; WP5 gives it a door",
                 ),
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

/// What a command changed.
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
    pub revision: u64,
}

macro_rules! define_command_registry {
    ($( ($kind:ident, $id:ident, $wire:literal, $payload:ident, $surfaces:expr) ),+ $(,)?) => {
        /// One command and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        #[derive(Debug, Clone, PartialEq)]
        pub enum Command {
            $(
                #[doc = concat!("The `", $wire, "` command.")]
                $id($payload),
            )+
        }

        /// The identifier of one command, without its arguments.
        ///
        /// The registry columns hang here. A payload enum cannot host a
        /// `const ALL`. GENERATED from the `for_each_command!` list.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CommandId {
            $(
                #[doc = concat!("The identifier of the `", $wire, "` command.")]
                $id,
            )+
        }

        impl CommandId {
            /// Every command, in registry order. GENERATED.
            pub const ALL: &[CommandId] = &[$(CommandId::$id,)+];

            /// The name this command carries on the wire. GENERATED.
            pub fn wire_name(self) -> &'static str {
                match self {
                    $(CommandId::$id => $wire,)+
                }
            }

            /// How the session runs this command. GENERATED.
            pub fn kind(self) -> CommandKind {
                match self {
                    $(CommandId::$id => CommandKind::$kind,)+
                }
            }

            /// Which surfaces reach this command, and why one does not.
            /// GENERATED.
            pub fn surfaces(self) -> Surfaces {
                match self {
                    $(CommandId::$id => $surfaces,)+
                }
            }
        }

        impl Command {
            /// The identifier of this command. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Command::$id(_) => CommandId::$id,)+
                }
            }
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
        let simulation_before = self.simulation.is_some();
        let revisions_before = self.revision_snapshot();
        match command {
            Command::SetToolpathParam(args) => {
                let index = args.index;
                self.set_toolpath_param_impl(index, &args.param, args.value)?;
                Ok(Effects {
                    stale: self.moved_revisions(&revisions_before),
                    simulation_cleared: simulation_before && self.simulation.is_none(),
                    revision: self.toolpath_revision(index),
                })
            }
        }
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
