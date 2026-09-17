//! Toolpath CRUD, plan order, per-toolpath config edits and the cached
//! result slots.
//!
//! Split out of `session/mutation.rs` by P4. Every method here is an
//! inherent method on [`ProjectSession`], so its path does not change.

use std::collections::{BTreeSet, HashMap};

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::DressupConfig;
use crate::compute::tool_config::ToolId;
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::session::dependencies::EdgeKind;
use crate::session::{Effects, ProjectSession, SessionError, ToolpathConfig};

impl ProjectSession {
    // ── Toolpath CRUD ─────────────────────────────────────────────

    /// Add a toolpath to the specified setup.
    ///
    /// [`Effects::created`] carries the new toolpath's index in
    /// `toolpath_configs`, the value this method returned before WP4.
    /// [`Effects::revision`] is `None`: the caller names no existing
    /// index, so no revision read answers about it.
    #[instrument(skip(self, config))]
    pub(crate) fn add_toolpath(
        &mut self,
        setup_index: usize,
        config: ToolpathConfig,
    ) -> Result<Effects, SessionError> {
        let mut created = None;
        // The error type is named because this call is not in tail
        // position: the `?` below leaves `E` constrained by two `From`
        // bounds alone, which the compiler cannot solve. `with_effects`
        // names `Infallible` for the same reason.
        let mut effects = self.try_with_effects(None, |session| {
            created = Some(session.add_toolpath_impl(setup_index, config)?);
            Ok::<(), SessionError>(())
        })?;
        effects.created = created;
        Ok(effects)
    }

    /// Append the toolpath and report its index. The raw half of
    /// [`Self::add_toolpath`], which wraps it in the one [`Effects`]
    /// construction site.
    pub(crate) fn add_toolpath_impl(
        &mut self,
        setup_index: usize,
        mut config: ToolpathConfig,
    ) -> Result<usize, SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;

        // Assign a fresh ID
        config.id = crate::ids::ToolpathId(self.next_toolpath_id);
        self.next_toolpath_id += 1;

        let tp_index = self.toolpath_configs.len();
        self.toolpath_configs.push(config);
        setup.toolpath_indices.push(tp_index);

        // Adding a toolpath invalidates simulation
        self.simulation = None;

        Ok(tp_index)
    }

    /// Remove a toolpath by its index in `toolpath_configs`.
    ///
    /// Updates all setup `toolpath_indices` so that indices above the
    /// removed one are shifted down by one.
    ///
    /// [`Effects::revision`] is `None`: the index names a different
    /// toolpath after the removal, so no revision read at it answers
    /// about the toolpath the caller passed.
    #[instrument(skip(self))]
    pub(crate) fn remove_toolpath(&mut self, index: usize) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            if index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(index));
            }

            // Propagate BEFORE the removal reshuffles indices: downstream
            // rest-machining results were generated against the stock this
            // toolpath left (see `invalidate_result_chain`).
            session.invalidate_output_dependents(index, true);

            session.toolpath_configs.remove(index);
            session.drop_result(index);

            // Rebuild every setup's toolpath_indices: remove the index,
            // then decrement any index above it.
            for setup in &mut session.setups {
                setup.toolpath_indices.retain(|&i| i != index);
                for idx in &mut setup.toolpath_indices {
                    if *idx > index {
                        *idx -= 1;
                    }
                }
            }

            // Re-key cached results whose index shifted
            let mut new_results = std::collections::HashMap::new();
            for (k, v) in session.results.drain() {
                if k > index {
                    new_results.insert(k - 1, v);
                } else {
                    new_results.insert(k, v);
                }
            }
            session.results = new_results;

            // Every index above the removed one now names a different
            // toolpath, so no revision a reader recorded still applies.
            session.bump_all_revisions();

            session.simulation = None;
            Ok(())
        })
    }

    /// Move a toolpath from `from_index` to `to_index` within the same setup.
    ///
    /// Both indices refer to positions in the session-level `toolpath_configs`
    /// vec. The toolpath must belong to the same setup as determined by the
    /// current `toolpath_indices`.
    ///
    /// This is an INSERT, not a swap (G-DROPINDEX, 2026-09-10): the moved op
    /// takes the target op's place in plan order and everything between the
    /// two shifts by one. It used to swap the two positions, which the
    /// operator sees only when the two are not adjacent — dragging the last
    /// op to the top also sent the top op to the bottom.
    ///
    /// The two affordances that reach this method stay consistent under the
    /// change. Move Up / Move Down pass the ADJACENT op's index, and for
    /// adjacent positions an insert and a swap are the same permutation, so
    /// those two menu items are byte-identical before and after. Drag-and-drop
    /// carries the position the operator pointed at, which is an insertion
    /// gap, and only an insert can honour it. They are two gestures with one
    /// meaning — "put this op here" — not two policies that had to be
    /// reconciled.
    ///
    /// [`Effects::revision`] is `None`: the call names two toolpaths, so
    /// there is no one index to report a revision for.
    #[instrument(skip(self))]
    pub(crate) fn reorder_toolpath(
        &mut self,
        from_index: usize,
        to_index: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            if from_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(from_index));
            }
            if to_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(to_index));
            }
            if from_index == to_index {
                return Ok(());
            }

            // Re-order only the display order within the setup that owns
            // both toolpaths. `toolpath_configs` and the results cache are
            // keyed by stable global indices and must not move.
            //
            // Remove at `pf`, insert at `pt`, both read off the ORIGINAL
            // list. Downward (pf < pt) that lands the op just after the
            // target, upward (pf > pt) just before it; either way the
            // target keeps its neighbours and only the moved op changes
            // rank.
            for setup in &mut session.setups {
                let pos_from = setup.toolpath_indices.iter().position(|&i| i == from_index);
                let pos_to = setup.toolpath_indices.iter().position(|&i| i == to_index);
                if let (Some(pf), Some(pt)) = (pos_from, pos_to) {
                    let moved = setup.toolpath_indices.remove(pf);
                    setup.toolpath_indices.insert(pt, moved);
                    break;
                }
            }

            // Plan order changed for both toolpaths: each moved op's own
            // FromRemainingStock result (if any) and everything downstream
            // of either position may have been generated against a stock
            // sequence that no longer exists (see
            // `invalidate_result_chain`). Each moved op is covered as
            // "downstream" of the other's seed.
            session.invalidate_output_dependents(from_index, true);
            session.invalidate_output_dependents(to_index, true);

            session.simulation = None;
            Ok(())
        })
    }

    /// Invalidate a toolpath's cached result plus every cached result that
    /// consumed its outputs, then drop the simulation.
    ///
    /// Dependency closure (2026-07-09, the live-v2 staleness collision
    /// class): a `FromRemainingStock` toolpath's geometry — including the
    /// `optimize_entry_descents` rapids lowered to `prior stock clearance
    /// ceiling + 2 mm` (that ceiling was the flat-disc material top until
    /// B2 made it profile-aware, so a descent may now sit legitimately
    /// BELOW the prior stock top; the invalidation below keys on the stock
    /// CHAIN, never on a height, and is unaffected)
    /// — is generated against the stock its UPSTREAM ops leave. When an
    /// upstream op's material-removal contribution changes (param edit,
    /// enable/disable, reorder, removal) and the downstream result is kept,
    /// the next simulation runs those baked descents against a stock that
    /// no longer matches: the GUI incident showed 20 rapid collisions at
    /// z = 19.996 (old finished ceiling 17.996 + 2 mm clearance) grazing
    /// the raw stock top at 20 — a real gouge hazard at rapid feed, not an
    /// instrument artifact. Cached results that can no longer be trusted
    /// must die with the edit:
    ///
    /// - every LATER toolpath in the same setup (plan order) with
    ///   `StockSource::FromRemainingStock`, downstream of any op whose
    ///   stock contribution changed;
    /// - every toolpath whose enabled `DerivedRestRegions` boundary source
    ///   is an invalidated toolpath (its clip regions came from that cached
    ///   result);
    /// - transitively, to fixpoint (an invalidated rest op's own output
    ///   feeds further rest ops).
    ///
    /// `stock_chain_changed`: whether THIS toolpath's contribution to the
    /// setup's material-removal sequence changed. Pass `true` from content
    /// edits on an enabled toolpath and from enable/disable flips (both
    /// directions change the chain); pass `false` for edits to a toolpath
    /// that is and stays disabled (only its `DerivedRestRegions` consumers
    /// are invalidated).
    ///
    /// Returns every index whose cached result this call dropped. The set
    /// always holds `index` itself, because `drop_result` runs on it
    /// whether or not it held a cached result.
    pub(crate) fn invalidate_result_chain(
        &mut self,
        index: usize,
        stock_chain_changed: bool,
    ) -> BTreeSet<usize> {
        self.drop_result(index);
        let (_dirty, mut dropped) = self.invalidate_output_dependents(index, stock_chain_changed);
        dropped.insert(index);
        dropped
    }

    /// Propagation half of [`Self::invalidate_result_chain`]: invalidate
    /// consumers of `index`'s outputs WITHOUT touching `index`'s own cached
    /// result. Used by the enable toggle — the toggled op's own result stays
    /// valid for a future re-enable; only what was built on top of its
    /// stock/regions is stale.
    ///
    /// Returns `(dirty, dropped)`. `dirty` is every index whose cached
    /// result is now invalid, INCLUDING `index` itself. `dropped` is the
    /// subset this call actually called `drop_result` on, which EXCLUDES
    /// `index`: the enable toggle depends on exactly that asymmetry, so
    /// the two sets are reported apart rather than conflated.
    pub(crate) fn invalidate_output_dependents(
        &mut self,
        index: usize,
        stock_chain_changed: bool,
    ) -> (BTreeSet<usize>, BTreeSet<usize>) {
        let seeds = BTreeSet::from([index]);
        let chain_seeds = if stock_chain_changed {
            seeds.clone()
        } else {
            BTreeSet::new()
        };
        self.invalidate_output_dependents_of_set(seeds, chain_seeds)
    }

    /// Seed-set half of [`Self::invalidate_output_dependents`]: run the
    /// same fixpoint over MANY seeds at once.
    ///
    /// A tool edit or a model refresh invalidates a SET of toolpaths in one
    /// mutation, not one toolpath (see [`Self::drop_tool_results`] and
    /// `drop_results_for_model`). Those two doors used to call
    /// [`Self::drop_result`] in a flat loop, so a same-setup
    /// `StockSource::FromRemainingStock` op, or a
    /// `BoundarySource::DerivedRestRegions` consumer, kept a cached result
    /// built on stock that the edit had moved (SES-07). They now seed this
    /// walker instead, and the whole seed set shares ONE fixpoint: a
    /// per-seed loop would walk the chain once per seed and reach the same
    /// answer more slowly.
    ///
    /// `seeds` names every index whose cached result is already invalid.
    /// `chain_seeds` names the subset whose contribution to the setup's
    /// material-removal sequence changed. A DISABLED seed belongs in that
    /// subset: `set_toolpath_enabled` writes `enabled = false` and THEN
    /// seeds this walker, and switching an op off moves the material above
    /// its successors exactly as switching it on does. Four callers pass
    /// the flag unfiltered, and two sentries pin that drop.
    ///
    /// This door is the EDIT side: it clears the simulation, which an edit
    /// owes and a completion does not. A caller that records an answer
    /// calls [`Self::walk_output_dependents`] instead.
    ///
    /// Returns `(dirty, dropped)` with the contract
    /// [`Self::invalidate_output_dependents`] states: `dropped` excludes
    /// every seed, because this call drops no seed's own result.
    pub(crate) fn invalidate_output_dependents_of_set(
        &mut self,
        seeds: BTreeSet<usize>,
        chain_seeds: BTreeSet<usize>,
    ) -> (BTreeSet<usize>, BTreeSet<usize>) {
        self.simulation = None;
        self.walk_output_dependents(seeds, chain_seeds)
    }

    /// Propagate an invalidation along the dependency edges, and touch no
    /// simulation.
    ///
    /// The pure half of [`Self::invalidate_output_dependents_of_set`].
    /// `Command::AdoptResult` calls THIS door: an adopt that cleared the
    /// simulation would destroy the `prior_stocks` snapshot the next
    /// operation in a Generate All ladder reads, and would report
    /// `simulation_cleared` on every completion.
    ///
    /// The rules are [`crate::session::dependencies::edges`], and nothing
    /// else. There are three:
    ///
    /// - (a) a Stock edge hits when its source is in `chain_dirty`: the
    ///   material above the consumer moved.
    /// - (b) a Regions edge hits when its source is in `dirty`: the
    ///   source's rest regions may have appeared, moved or vanished.
    /// - (c) a PrevTool edge hits when its source is in `dirty`, so a Rest
    ///   consumer drops when its predecessor's result drops.
    ///
    /// The edges are derived ONCE, before the loop. They come from configs,
    /// and the loop changes only results and revisions, so no edge appears
    /// or vanishes inside it.
    pub(crate) fn walk_output_dependents(
        &mut self,
        seeds: BTreeSet<usize>,
        chain_seeds: BTreeSet<usize>,
    ) -> (BTreeSet<usize>, BTreeSet<usize>) {
        debug_assert!(
            chain_seeds.is_subset(&seeds),
            "a chain seed names a stock contribution that moved, so it is \
             also a dirty seed"
        );

        // Indices whose cached result is now invalid.
        let mut dirty: BTreeSet<usize> = seeds;
        // The subset `drop_result` ran on. No seed joins it here.
        let mut dropped: BTreeSet<usize> = BTreeSet::new();
        // Subset of `dirty` whose stock contribution changed (drives the
        // same-setup downstream rule). A dirtied enabled op joins this set:
        // its regenerated output may cut differently.
        let mut chain_dirty: BTreeSet<usize> = chain_seeds;

        // The rules, derived once. `index_of` and `edges` are owned, so the
        // `&mut self` drops below do not conflict with them.
        let edges = crate::session::dependencies::edges(&*self);
        let index_of: HashMap<crate::ids::ToolpathId, usize> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .map(|(i, tc)| (tc.id, i))
            .collect();

        loop {
            let dirty_ids: BTreeSet<crate::ids::ToolpathId> = dirty
                .iter()
                .filter_map(|&i| self.toolpath_configs.get(i).map(|tc| tc.id))
                .collect();
            let chain_ids: BTreeSet<crate::ids::ToolpathId> = chain_dirty
                .iter()
                .filter_map(|&i| self.toolpath_configs.get(i).map(|tc| tc.id))
                .collect();

            let mut newly: Vec<usize> = Vec::new();
            for edge in &edges {
                let Some(src_id) = edge.on else { continue };
                let hit = match edge.kind {
                    // (a) the material above the consumer moved.
                    EdgeKind::Stock => chain_ids.contains(&src_id),
                    // (b) and (c) the source's OUTPUT moved.
                    EdgeKind::Regions | EdgeKind::PrevTool => dirty_ids.contains(&src_id),
                };
                let Some(&idx) = index_of.get(&edge.from) else {
                    continue;
                };
                if !hit || dirty.contains(&idx) || newly.contains(&idx) {
                    continue;
                }
                newly.push(idx);
            }

            if newly.is_empty() {
                break;
            }
            for tp_idx in newly {
                self.drop_result(tp_idx);
                dirty.insert(tp_idx);
                dropped.insert(tp_idx);
                if self
                    .toolpath_configs
                    .get(tp_idx)
                    .is_some_and(|tc| tc.enabled)
                {
                    chain_dirty.insert(tp_idx);
                }
            }
        }

        (dirty, dropped)
    }

    /// Enable or disable a toolpath. Flipping the flag in EITHER direction
    /// changes the setup's material-removal chain, so downstream
    /// rest-machining results are invalidated (see
    /// [`Self::invalidate_result_chain`]).
    ///
    /// The toggled op KEEPS its own cached result (the N1 design): it
    /// stays valid for a re-enable. Its revision therefore does not move,
    /// and [`Effects::stale`] — the revision-moved set — excludes
    /// `index`. It reports the downstream set only.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_enabled(
        &mut self,
        index: usize,
        enabled: bool,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            let changed = tc.enabled != enabled;
            tc.enabled = enabled;
            if changed {
                session.invalidate_output_dependents(index, true);
            }
            session.simulation = None;
            Ok(())
        })
    }

    /// Replace a toolpath's operation config wholesale — including switching
    /// the operation KIND — while keeping its tool, heights, boundary,
    /// dressups, and position in the machining order. This is the supported
    /// way to A/B one operation against another in an existing chain
    /// (chain order matters: rest-referencing ops downstream see the stock
    /// this toolpath leaves). Re-normalizes the dressups for the new op kind
    /// and drops the cached result + simulation.
    #[instrument(skip(self, operation))]
    pub(crate) fn set_toolpath_operation(
        &mut self,
        index: usize,
        operation: OperationConfig,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.operation = operation;
            // Same invariant set_dressup_config enforces: the surviving
            // dressups must be legal for the NEW operation kind.
            let mut dressups = tc.dressups.clone();
            dressups.normalize_for_op(tc.operation.op_type());
            tc.dressups = dressups;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    // ── Cross-setup moves ─────────────────────────────────────────

    /// Move a toolpath from its current setup to a different setup.
    ///
    /// `target_position` is the gap the toolpath lands in, counted in the
    /// TARGET setup's own plan order: `Some(0)` puts it first, `Some(len)`
    /// or anything larger puts it last, and `None` appends. It is an
    /// `Option` rather than a plain index because the two callers ask
    /// different questions — a drag carries the drop position the operator
    /// pointed at, while MCP `move_toolpath_to_setup` has no position
    /// argument at all and must keep appending (G-DROPINDEX, 2026-09-10:
    /// the drop index used to be computed by the panel, passed through the
    /// event and then dropped on the floor here, so every cross-setup drag
    /// landed at the bottom of the target setup whatever the operator did).
    ///
    /// The cached toolpath result is invalidated because the setup transform
    /// may have changed (e.g. top → bottom orientation).
    ///
    /// [`Effects::revision`] is `None`: the toolpath lands in another
    /// setup's plan order, so the index the caller passed no longer
    /// describes where the toolpath sits.
    #[instrument(skip(self))]
    pub(crate) fn move_toolpath_to_setup(
        &mut self,
        tp_index: usize,
        target_setup_index: usize,
        target_position: Option<usize>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, move |session| {
            if tp_index >= session.toolpath_configs.len() {
                return Err(SessionError::ToolpathNotFound(tp_index));
            }
            if target_setup_index >= session.setups.len() {
                return Err(SessionError::SetupNotFound(target_setup_index));
            }

            // Propagate while the toolpath is still in its ORIGINAL setup
            // so that setup's downstream rest ops are invalidated (see
            // `invalidate_result_chain`); its own result dies below.
            session.invalidate_output_dependents(tp_index, true);

            // Remove from whichever setup currently owns this toolpath
            for setup in &mut session.setups {
                setup.toolpath_indices.retain(|&i| i != tp_index);
            }

            // SAFETY: target_setup_index bounds-checked above
            #[allow(clippy::indexing_slicing)]
            let target = &mut session.setups[target_setup_index].toolpath_indices;
            let at = target_position.unwrap_or(target.len()).min(target.len());
            target.insert(at, tp_index);

            session.drop_result(tp_index);
            session.simulation = None;
            Ok(())
        })
    }

    // ── Toolpath config updates ──────────────────────────────────

    /// Rebind a toolpath to a different **tool**, invalidating its cached
    /// result and everything downstream of the stock it leaves.
    ///
    /// `tool_id` is the project-assigned [`ToolConfig::id`] (the `id` field
    /// of a `list_tools` row), NOT a positional index — it is the same
    /// number [`crate::session::ToolpathConfig::tool_id`] stores.
    ///
    /// This is a BINDING change, not an operation parameter: no operation
    /// config carries a `tool_id` field, so
    /// [`ProjectSession::set_toolpath_param`](Self::set_toolpath_param)
    /// cannot reach it and refuses the key. Keep the two routes disjoint —
    /// `set_toolpath_param` writes the operation's own params through a
    /// serde round-trip, and Rest's `prev_tool_id` IS one of those params
    /// (the rest-analysis reference tool), which is a different thing from
    /// the cutter this toolpath runs.
    ///
    /// The tool must exist. Its SHAPE is deliberately not checked here: an
    /// operation with a tool its registry entry rejects is a *blocked*
    /// operation the operator can fix by rebinding, and refusing the
    /// rebind would remove the only fix. The shape refusal stays where it
    /// is — `ToolConstraintsDef::allows`, read by the generators.
    ///
    /// A rebind to the tool already bound is a no-op and invalidates
    /// nothing.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_tool(
        &mut self,
        index: usize,
        tool_id: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            if !session.tools.iter().any(|t| t.id.0 == tool_id) {
                return Err(SessionError::ToolNotFound(ToolId(tool_id)));
            }
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.tool_id == tool_id {
                return Ok(());
            }
            tc.tool_id = tool_id;
            let enabled = tc.enabled;
            // A different cutter removes different material, so this is a
            // stock-chain change: same invalidation as
            // `set_face_selection` and `set_heights_config`, not the
            // narrower `remove_result`.
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Rebind a toolpath to a different **input model**, invalidating its
    /// cached result and everything downstream of the stock it leaves.
    ///
    /// `model_id` is the project-assigned [`super::LoadedModel::id`] — the
    /// `id` field of an `inspect_model` row, NOT a positional index. It is
    /// the same number [`crate::session::ToolpathConfig::model_id`] stores.
    ///
    /// The model must exist. Unlike `add_toolpath`, which accepts an
    /// unresolvable `model_id` and leaves the operation carrying a dangling
    /// reference, this setter refuses one: the whole point of the route is
    /// to REPAIR a binding.
    ///
    /// The geometry KIND is deliberately not checked (a 2D-only operation
    /// may be pointed at a mesh). That precondition belongs to the refusal
    /// contract, not to this setter.
    ///
    /// **Caveat this does not handle:** `face_selection` holds
    /// [`FaceGroupId`]s of the model that was bound when the faces were
    /// picked. A rebind leaves them in place, pointing into a different
    /// mesh. Clearing them is a separate decision; the GUI's Input combo
    /// does not clear them either.
    ///
    /// A rebind to the model already bound is a no-op and invalidates
    /// nothing.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_model(
        &mut self,
        index: usize,
        model_id: usize,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            if !session.models.iter().any(|m| m.id == model_id) {
                let known: Vec<String> = session.models.iter().map(|m| m.id.to_string()).collect();
                return Err(SessionError::MissingGeometry(format!(
                    "Model id {model_id} not found; project model ids: [{}]",
                    known.join(", ")
                )));
            }
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.model_id == model_id {
                return Ok(());
            }
            tc.model_id = model_id;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })
    }

    /// Install an operation, dressup and face-selection triple on the
    /// toolpath at `index`, and drop THAT INDEX ALONE.
    ///
    /// **The narrowness is the point, and no surface may call this.** The
    /// tool-load optimizer's candidate search
    /// (`tool_load/optimize/candidate.rs`) restores a candidate, then
    /// regenerates that one index, then runs a project simulation against
    /// the neighbours' cached results. `candidate.rs` says so at the
    /// simulation call: "Other toolpaths' cached results from baseline
    /// still apply because `generate_toolpath` only touched index
    /// `toolpath_index`." A wide invalidation would drop every downstream
    /// `StockSource::FromRemainingStock` result once PER CANDIDATE, so the
    /// search would regenerate the whole chain at every point of the
    /// search instead of once. `optimize/context.rs`'s
    /// `BaselineRestoreGuard` restores the baseline on drop through the
    /// same path, and a `Drop` cannot report an error, so a wide restore
    /// there would silently leave a dead chain behind every
    /// `optimize_toolpath` call.
    ///
    /// Every OTHER caller takes
    /// [`Command::RestoreToolpathSnapshot`](super::Command), which walks
    /// the chain (WP8, N14). `the_narrow_path_leaves_the_neighbour_result_cached`
    /// in this file's test module pins the difference.
    #[instrument(skip(self, operation, dressups, face_selection))]
    pub(crate) fn apply_toolpath_param_snapshot_narrow(
        &mut self,
        index: usize,
        operation: OperationConfig,
        dressups: DressupConfig,
        face_selection: Option<Vec<FaceGroupId>>,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.operation = operation;
            tc.dressups = dressups;
            tc.face_selection = face_selection;
            session.drop_result(index);
            session.simulation = None;
            Ok(())
        })
    }

    /// Overwrite a toolpath's feeds provenance (W2.1), stamping
    /// [`crate::feeds::ProvenanceSource::Optimizer`] on the dimensions a
    /// caller changed. Does not touch the result or simulation caches.
    ///
    /// **This has no production caller since WP8.** The three optimizer
    /// apply paths called it right after the snapshot, which is how an
    /// undo came to restore the earlier values under the later stamp.
    /// They now carry the provenance in
    /// [`RestoreToolpathSnapshotArgs`](super::RestoreToolpathSnapshotArgs),
    /// so one command writes the values and the stamp together.
    pub(crate) fn set_feeds_provenance(
        &mut self,
        index: usize,
        feeds_provenance: crate::feeds::FeedsProvenance,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.feeds_provenance = feeds_provenance;
            Ok(())
        })
    }

    /// Replace a toolpath's debug options.
    ///
    /// The door of the `SetToolpathDebugOptions` command row (§16 ruling
    /// 7). The generate door writes this flag immediately before it
    /// runs, so the row moves NO revision and drops no result: a debug
    /// trace is an output of a generation, never an input to one.
    ///
    /// This is not the §9 Q4 divergence. Q4 preserves today's behaviour
    /// for `set_toolpath_param`'s arm alone; a dedicated row for the
    /// generate-time write bumps nothing by design.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_debug_options(
        &mut self,
        index: usize,
        debug_options: crate::trace::debug_trace::ToolpathDebugOptions,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            tc.debug_options = debug_options;
            Ok(())
        })
    }

    /// Write a freshly-computed `ToolpathComputeResult` into `session.results`.
    ///
    /// Symmetric counterpart to the [`Self::drop_result`] calls
    /// performed by every mutating method on `ProjectSession`. The
    /// invariant is: `session.results[idx]` should always be either
    /// absent (toolpath is stale / never generated) or a fresh result
    /// produced from the current toolpath config. The viz-side threaded
    /// compute backend writes through this method so the core cache
    /// stays in sync with the viz-side `gui.toolpath_rt[id].result`
    /// cache; before this method existed, the two caches diverged after
    /// the undo snapshot path cleared `results[idx]` and the
    /// GUI regen only repopulated `gui.toolpath_rt`. See
    /// `planning/F1_RCA.md` (Roadmap F.1) for the divergence write-up.
    ///
    /// WP3 closed the public door. A completion now arrives as
    /// [`Command::AdoptResult`](crate::session::Command), which carries
    /// the revision the compute lane started from and refuses a
    /// completion that answers a superseded parameter set.
    /// [`ProjectSession::apply`] is the only caller.
    pub(crate) fn insert_result(
        &mut self,
        index: usize,
        result: super::ToolpathComputeResult,
    ) -> Result<(), SessionError> {
        if index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(index));
        }
        self.results.insert(index, result);
        Ok(())
    }

    /// Drop a toolpath's cached `ToolpathComputeResult`.
    ///
    /// The other half of [`Self::insert_result`]'s contract, for the case
    /// that used to have no half at all: a generation that **failed**. The
    /// viz-side worker path sets `gui.toolpath_rt[id].result = None` on a
    /// compute error, but before this existed nothing cleared the core-side
    /// `session.results[idx]`, so a previous parameter set's toolpath stayed
    /// cached and readable as if it were this configuration's answer —
    /// exported, simulated, and (via
    /// [`crate::compute::simulate::PhantomPriorStockScan`], which asks only
    /// "has this been generated?") counted as generated, which withholds a
    /// pending rest op's phantom prior-stock snapshot.
    ///
    /// Never an error: removing the result of an index that has none is
    /// the no-op the callers want.
    ///
    /// The method reported `true` when a cached result was actually
    /// removed. It reports [`Effects`] instead, and the two are not the
    /// same reading: `drop_result` bumps the revision whether or not a
    /// result was there, so `Effects::stale` always holds `index`. A
    /// caller that needs "was a result present" reads
    /// [`ProjectSession::get_result`] before the call.
    pub(crate) fn remove_result(&mut self, index: usize) -> Effects {
        self.with_effects(Some(index), move |session| {
            session.drop_result(index);
        })
    }

    /// Drop a toolpath's cached result and bump its revision.
    ///
    /// **The single site that does either.** Every mutating method on
    /// `ProjectSession` that used to call `self.results.remove(&index)`
    /// calls this instead, so "the cached answer is gone" and "the inputs
    /// moved" are one event with one record. `insert_result` deliberately
    /// does NOT bump: recording an answer is not a change of inputs.
    ///
    /// Returns `true` when a cached result was actually removed. The
    /// revision bumps either way — an edit to a never-generated toolpath
    /// still moves its inputs, and a reader comparing revisions across a
    /// generation must see that.
    pub(crate) fn drop_result(&mut self, index: usize) -> bool {
        self.next_revision += 1;
        self.toolpath_revision.insert(index, self.next_revision);
        self.results.remove(&index).is_some()
    }

    /// Bump every toolpath's revision without touching the result cache.
    ///
    /// Used where toolpath INDICES shift (a removal, a bulk replace): after
    /// such a move an index no longer names the toolpath a reader recorded
    /// a revision for, so every outstanding comparison must fail.
    pub(crate) fn bump_all_revisions(&mut self) {
        for index in 0..self.toolpath_configs.len() {
            self.next_revision += 1;
            self.toolpath_revision.insert(index, self.next_revision);
        }
    }

    /// Drop every toolpath's cached result and the simulation.
    ///
    /// The stock rule (operator ruling, 2026-09-10, R0.1 §7 Q1): ANY stock
    /// edit — dimensions, alignment pins, or the material alone — makes
    /// every toolpath edited-since. Heights reference the stock top and an
    /// inherited boundary follows the stock outline, so geometry can move;
    /// the material moves the feeds. Both the GUI route and the MCP route
    /// reach this one function, so the two cannot disagree.
    pub(crate) fn drop_all_results(&mut self) {
        for index in 0..self.toolpath_configs.len() {
            self.drop_result(index);
        }
        self.simulation = None;
    }

    /// Drop the cached result of every named toolpath, then invalidate
    /// everything downstream of them, plus the simulation.
    ///
    /// The set-shaped sibling of [`Self::invalidate_result_chain`]. A tool
    /// edit and a model refresh each name a SET of directly affected
    /// toolpaths, so they collect that set and hand it here. The seeds lose
    /// their own cached result; the fixpoint in
    /// [`Self::invalidate_output_dependents_of_set`] then reaches the
    /// same-setup `StockSource::FromRemainingStock` ops and the
    /// `BoundarySource::DerivedRestRegions` consumers that the seeds feed.
    ///
    /// A disabled seed removes no material, so it seeds `dirty` but not
    /// `chain_dirty` — the same rule every single-index caller applies.
    pub(crate) fn drop_results_and_their_dependents(&mut self, indices: &[usize]) {
        let mut seeds: BTreeSet<usize> = BTreeSet::new();
        let mut chain_seeds: BTreeSet<usize> = BTreeSet::new();
        for &index in indices {
            self.drop_result(index);
            seeds.insert(index);
            if self
                .toolpath_configs
                .get(index)
                .is_some_and(|tc| tc.enabled)
            {
                chain_seeds.insert(index);
            }
        }
        let _ = self.invalidate_output_dependents_of_set(seeds, chain_seeds);
    }

    /// Drop the cached results of every toolpath in one setup, plus the
    /// simulation. The setup transform decides the frame every one of them
    /// is generated in.
    pub(crate) fn drop_setup_results(&mut self, setup_index: usize) {
        let Some(setup) = self.setups.get(setup_index) else {
            return;
        };
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for idx in indices {
            self.drop_result(idx);
        }
        self.simulation = None;
    }

    /// Public door onto the chain invalidation for callers that write
    /// `ToolpathConfig` fields directly rather than through a setter.
    ///
    /// The feeds Apply funnel is the one such caller
    /// (`controller/events/mod.rs::apply_feeds_through_funnel`, N13). It
    /// resolves the tool, the machine, the material and the
    /// recommendation from GUI state, then writes `operation` and
    /// `feeds_provenance` through `toolpath_configs_mut`, so no core
    /// setter runs. It reads
    /// [`ToolpathConfig::generation_inputs_signature`](super::ToolpathConfig::generation_inputs_signature)
    /// either side of that write and calls this door when the signature
    /// moved. Before this door existed the core kept the PREVIOUS result
    /// and export emitted it (R0.1 §2.2 item 1), and the downstream
    /// `FromRemainingStock` chain was never invalidated either.
    ///
    /// The GUI inspector was the other such caller until WP5. It takes
    /// [`Command::ReplaceToolpathConfig`](super::Command) now, which
    /// carries the same gate inside core. The funnel keeps this door
    /// until §15 ruling 4 folds `ApplyFeeds` into a row of its own.
    pub(crate) fn invalidate_toolpath_inputs(&mut self, index: usize) -> Effects {
        self.with_effects(Some(index), move |session| {
            let enabled = session
                .toolpath_configs
                .get(index)
                .is_some_and(|tc| tc.enabled);
            session.invalidate_result_chain(index, enabled);
        })
    }
}
