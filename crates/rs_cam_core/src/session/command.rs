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
//! WP3 adds the one construction site. Every mutation on
//! [`ProjectSession`] runs its body inside
//! [`ProjectSession::try_with_effects`], so no mutation builds an
//! [`Effects`] of its own.
//!
//! The registry carries two rows, `SetToolpathParam` and `AdoptResult`.
//! The later work packages add the rest.

use std::collections::BTreeSet;

use super::{ProjectSession, SessionError, ToolpathComputeResult};

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
            (Command, AdoptResult, "adopt_result", AdoptResultArgs,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "a completion is adopted by the GUI drain, not by a wire tool",
                 ),
                 cli: Reach::Skip(
                     "the CLI generates synchronously and never adopts a completion",
                 ),
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

macro_rules! define_command_registry {
    ($( ($kind:ident, $id:ident, $wire:literal, $payload:ident, $surfaces:expr) ),+ $(,)?) => {
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
