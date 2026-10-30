//! Toolpath CRUD, plan order, per-toolpath config edits and the cached
//! result slots.
//!
//! Split out of `session/mutation.rs` by P4. Every method here is an
//! inherent method on [`ProjectSession`], so its path does not change.

use std::collections::{BTreeSet, HashMap};

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::tool_config::ToolId;
use crate::session::dependencies::EdgeKind;
use crate::session::{Effects, ProjectSession, SessionError, SimulationDropCause, ToolpathConfig};

impl ProjectSession {
    // ── Toolpath CRUD ─────────────────────────────────────────────

    /// Add a toolpath to the specified setup.
    ///
    /// [`Effects::created`] carries the new toolpath's index in
    /// `toolpath_configs`, the value this method returned before WP4.
    /// [`Effects::revision`] is `None`: the caller names no existing
    /// index, so no revision read answers about it.
    ///
    /// UX-7b (SAFETY): THE each-face rule
    /// ([`ProjectSession::each_face_floor_due`]) runs on the new toolpath.
    /// A new area op whose selection holds a model ring or face below the
    /// stock top (the default rule on a STEP model) takes the floor "Each
    /// picked face", depth 0, in this one command, and
    /// [`Effects::floor_set`] names it. Every add door (GUI, MCP, CLI)
    /// sends this command, so one rule serves them all.
    #[instrument(skip(self, config))]
    pub(crate) fn add_toolpath(
        &mut self,
        setup_index: usize,
        config: ToolpathConfig,
    ) -> Result<Effects, SessionError> {
        // P3.2: a new toolpath states one floor field.
        config.floor_conflict()?;
        // W6b (F030): every number of the op and the shoe is inside its
        // domain.
        crate::compute::domain::validate_toolpath(&config)?;
        let mut created = None;
        let mut floor_set = None;
        // The error type is named because this call is not in tail
        // position: the `?` below leaves `E` constrained by two `From`
        // bounds alone, which the compiler cannot solve. `with_effects`
        // names `Infallible` for the same reason.
        let mut effects = self.try_with_effects(None, |session| {
            let index = session.add_toolpath_impl(setup_index, config)?;
            created = Some(index);
            let due = session.toolpath_configs.get(index).and_then(|tc| {
                session
                    .part_outline_floor_due(tc)
                    .or_else(|| session.each_face_floor_due(tc))
                    .or_else(|| session.each_hole_floor_due(tc))
            });
            if let Some(candidate) = due
                && let Some(tc) = session.toolpath_configs.get_mut(index)
            {
                floor_set = candidate.floor();
                tc.operation = candidate;
            }
            Ok::<(), SessionError>(())
        })?;
        effects.created = created;
        effects.floor_set = floor_set;
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
        config.id = crate::ids::ToolpathId::mint(self.next_toolpath_id);
        self.next_toolpath_id += 1;

        let tp_index = self.toolpath_configs.len();
        self.toolpath_configs.push(config);
        setup.toolpath_indices.push(tp_index);

        // Adding a toolpath invalidates simulation
        self.drop_simulation(SimulationDropCause::Operations);

        Ok(tp_index)
    }

    /// Add one Bore directly after the Drill at `index` (geometry selection
    /// P6.7, L-17: "Add a Bore for these"). The door of
    /// [`Command::AddBoreFromDrill`](crate::session::Command::AddBoreFromDrill).
    ///
    /// The Bore takes from the Drill:
    ///
    /// - the selection (a copy), so both ops split the holes by the fit
    ///   rule (design 4.1);
    /// - the model, because the stored selection names the toolpath model;
    /// - NOT the tool (Q9, Fusion hole recognition; it overturns the P6.7b
    ///   ruling): the Bore takes the first project tool that it accepts and
    ///   that is smaller than the smallest hole the Drill skips as `Larger`
    ///   (`bore_tool_for`). With none, the Bore has no tool (R2) and its
    ///   state names the kind and the size limit (`no_tool_state`);
    /// - the floor (R-B, lead decision 2026-10-23): the same `Floor`. Both
    ///   ops read one per-hole rule (`compute::floor::hole_floors`), so
    ///   "Each hole floor" gives each bored hole its own floor too;
    /// - the heights: both are `OpFloor` ops with the UX-1 top rule, so a
    ///   pinned Top Z gives the same depth base to both;
    /// - the spindle speed, the stock source and the coolant.
    ///
    /// The feeds, the pitch and the finish allowance are the `BoreConfig`
    /// defaults; Suggest gives feeds for the tool.
    ///
    /// [`Effects::created`] carries the new toolpath's index in
    /// `toolpath_configs`. The Bore cuts before every op after the Drill,
    /// so the rest chain after it goes stale.
    #[instrument(skip(self))]
    pub(crate) fn add_bore_from_drill(
        &mut self,
        index: usize,
        drill_id: crate::ids::ToolpathId,
    ) -> Result<Effects, SessionError> {
        let drill = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        if drill.id != drill_id {
            return Err(SessionError::ToolpathIdMismatch {
                index,
                id: drill_id,
            });
        }
        let OperationConfig::Drill(drill_op) = &drill.operation else {
            return Err(SessionError::NotADrill {
                index,
                operation: drill.operation.label(),
            });
        };
        let setup_index = self
            .setups
            .iter()
            .position(|s| s.toolpath_indices.contains(&index))
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let bore = crate::compute::operation_configs::BoreConfig {
            floor: drill_op.floor.clone(),
            spindle_rpm: drill_op.spindle_rpm,
            selection: drill_op.selection.clone(),
            ..crate::compute::operation_configs::BoreConfig::default()
        };
        let op_type = crate::compute::catalog::OperationType::Bore;
        let config = ToolpathConfig {
            id: crate::ids::ToolpathId::placeholder(),
            name: format!("{} bore", drill.name),
            enabled: drill.enabled,
            operation: OperationConfig::Bore(bore),
            dressups: crate::compute::config::DressupConfig::for_op(op_type),
            heights: drill.heights.clone(),
            tool_id: self.bore_tool_for(drill),
            model_id: drill.model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::compute::config::BoundaryConfig::default(),
            boundary_inherit: true,
            rest_analysis: crate::compute::config::RestAnalysisConfig::default(),
            stock_source: drill.stock_source,
            coolant: drill.coolant,
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: crate::feeds::FeedsProvenance::default(),
            planner_origin: None,
            shoe: drill.shoe,
            feeds_lock: drill.feeds_lock,
        };
        config.floor_conflict()?;
        crate::compute::domain::validate_toolpath(&config)?;
        let mut created = None;
        let mut effects = self.try_with_effects(None, |session| {
            let bore_index = session.add_toolpath_impl(setup_index, config)?;
            // `add_toolpath_impl` appends to the setup. Move the Bore to
            // the place after the Drill in the plan order of the setup.
            #[expect(
                clippy::indexing_slicing,
                reason = "`setup_index` is a position in `setups` (above)."
            )]
            let order = &mut session.setups[setup_index].toolpath_indices;
            order.retain(|&i| i != bore_index);
            let at = order
                .iter()
                .position(|&i| i == index)
                .map_or(order.len(), |p| p + 1);
            order.insert(at, bore_index);
            // A rest op after the Drill now sits after a new cut.
            session.invalidate_output_dependents(bore_index, true);
            created = Some(bore_index);
            Ok::<(), SessionError>(())
        })?;
        effects.created = created;
        Ok(effects)
    }

    /// Step 7.8: add the pair of the template "Geodesic body + edges". The
    /// door of
    /// [`Command::AddGeodesicBodyAndEdges`](crate::session::Command::AddGeodesicBodyAndEdges):
    /// each op goes through [`Self::add_toolpath_impl`], the function of
    /// the `AddToolpath` row, after the same checks, in one effects window.
    ///
    /// The pair rule lives here, so every surface gets it: op 1 keeps its
    /// rest analysis on exactly when the edge op exists (D44), the edge op
    /// takes op 1's new id as its `DerivedRestRegions` source, and an edge
    /// tool larger than op 1's refuses before anything is added (D53).
    pub(crate) fn add_geodesic_body_and_edges(
        &mut self,
        setup_index: usize,
        mut body: ToolpathConfig,
        edges: Option<ToolpathConfig>,
    ) -> Result<Effects, SessionError> {
        use crate::compute::config::BoundarySource;
        if self.setups.get(setup_index).is_none() {
            return Err(SessionError::SetupNotFound(setup_index));
        }
        body.rest_analysis.enabled = edges.is_some();
        for config in std::iter::once(&body).chain(edges.as_ref()) {
            config.floor_conflict()?;
            crate::compute::domain::validate_toolpath(config)?;
        }
        if let Some(edge) = edges.as_ref()
            && let (Some(own), Some(theirs)) = (
                self.built_tool_radius_mm(edge.tool_id),
                self.built_tool_radius_mm(body.tool_id),
            )
            && own > theirs
        {
            let larger = crate::session::LargerRestConsumer {
                op_name: edge.name.clone(),
                source_toolpath_id: crate::ids::ToolpathId::placeholder(),
                source_name: body.name.clone(),
                tool_radius_mm: own,
                source_tool_radius_mm: theirs,
            };
            return Err(SessionError::OperationFailed(larger.refusal("Center")));
        }
        let mut created = None;
        let mut effects = self.try_with_effects(None, |session| {
            let body_index = session.add_toolpath_impl(setup_index, body)?;
            created = Some(body_index);
            if let Some(mut edge) = edges {
                let body_id = session
                    .toolpath_configs
                    .get(body_index)
                    .map(|tc| tc.id)
                    .ok_or(SessionError::ToolpathNotFound(body_index))?;
                edge.boundary.enabled = true;
                edge.boundary.source = BoundarySource::DerivedRestRegions {
                    source_toolpath_id: body_id,
                };
                let _ = session.add_toolpath_impl(setup_index, edge)?;
            }
            Ok::<(), SessionError>(())
        })?;
        effects.created = created;
        Ok(effects)
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

            session.drop_simulation(SimulationDropCause::Operations);
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

            session.drop_simulation(SimulationDropCause::Operations);
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
        self.drop_simulation(SimulationDropCause::Operations);
        self.walk_output_dependents(seeds, chain_seeds)
    }

    /// Propagate an invalidation along the dependency edges, and touch no
    /// simulation.
    ///
    /// The half of [`Self::invalidate_output_dependents_of_set`] that touches
    /// no simulation. It still drops the results of the dirty set.
    /// `Command::AdoptResult` calls THIS door: an adopt that cleared the
    /// simulation would destroy the `prior_stocks` snapshot the next
    /// operation in a Generate All ladder reads, and would report
    /// `simulation_cleared` on every completion.
    ///
    /// The rules are [`crate::session::dependencies::edges`], and nothing
    /// else. There are two:
    ///
    /// - (a) a Stock edge hits when its source is in `chain_dirty`: the
    ///   material above the consumer moved.
    /// - (b) a Regions edge hits when its source is in `dirty`: the
    ///   source's rest regions may have appeared, moved or vanished.
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
                    // (b) the source's OUTPUT moved.
                    EdgeKind::Regions => dirty_ids.contains(&src_id),
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
            session.drop_simulation(SimulationDropCause::Operations);
            Ok(())
        })
    }

    /// Write the shoe setting of a toolpath (H1).
    ///
    /// The door of the `SetToolpathShoe` command row. The shoe moves no
    /// toolpath, so every result stays. The simulation drops: its check
    /// read the old shoe position. A stated gap must be inside its domain
    /// ([`crate::machine::head::ShoeSetting::validate`]).
    ///
    /// # Errors
    /// [`SessionError::ToolpathNotFound`], or [`SessionError::OutOfDomain`]
    /// for a bad gap.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_shoe(
        &mut self,
        index: usize,
        shoe: crate::machine::head::ShoeSetting,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(None, |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            shoe.validate()
                .map_err(|refusal| crate::compute::domain::DomainRefusal::Shoe {
                    toolpath: tc.name.clone(),
                    refusal,
                })?;
            if tc.shoe != shoe {
                tc.shoe = shoe;
                session.drop_simulation(SimulationDropCause::Machine);
            }
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
    ///
    /// Geometry selection (P2.3b, DESIGN 3.1): a change of the op KIND
    /// keeps the selection. The setter copies the old selection into the
    /// new op. When the new op holds no selection (a 3D op), the selection
    /// drops, and [`Effects::selection_dropped`] says so when it was not
    /// the default of the old op. A replace with the same kind installs the
    /// new config as it is, its selection included.
    #[instrument(skip(self, operation))]
    pub(crate) fn set_toolpath_operation(
        &mut self,
        index: usize,
        mut operation: OperationConfig,
    ) -> Result<Effects, SessionError> {
        let mut selection_dropped = false;
        let mut effects = self.try_with_effects(Some(index), |session| {
            let tc = session
                .toolpath_configs
                .get_mut(index)
                .ok_or(SessionError::ToolpathNotFound(index))?;
            if tc.operation.op_type() != operation.op_type()
                && let Some(old) = tc.operation.selection()
            {
                match operation.selection_slot() {
                    Some(slot) => *slot = old.clone(),
                    None => {
                        selection_dropped = tc.operation.default_selection().as_ref() != Some(old);
                    }
                }
            }
            // P3.2: an op change onto an `OpFloor` op with a pinned Bottom Z
            // refuses; nothing is written.
            let before = std::mem::replace(&mut tc.operation, operation);
            if let Err(refusal) = tc.floor_conflict() {
                tc.operation = before;
                return Err(refusal);
            }
            // W6b (F030): the new op passes the one operation check;
            // nothing is written on a refusal.
            if let Err(refusal) =
                crate::compute::domain::validate_toolpath_operation(&tc.name, &tc.operation)
            {
                tc.operation = before;
                return Err(refusal.into());
            }
            // Same invariant set_dressup_config enforces: the surviving
            // dressups must be legal for the NEW operation kind.
            let mut dressups = tc.dressups.clone();
            dressups.normalize_for_op(tc.operation.op_type());
            tc.dressups = dressups;
            let enabled = tc.enabled;
            session.invalidate_result_chain(index, enabled);
            Ok::<(), SessionError>(())
        })?;
        effects.selection_dropped = selection_dropped;
        Ok(effects)
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

            #[expect(
                clippy::indexing_slicing,
                reason = "target_setup_index bounds-checked above"
            )]
            let target = &mut session.setups[target_setup_index].toolpath_indices;
            let at = target_position.unwrap_or(target.len()).min(target.len());
            target.insert(at, tp_index);

            session.drop_result(tp_index);
            session.drop_simulation(SimulationDropCause::Operations);
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
    /// serde round-trip, and Pencil's `reference_tool_id` IS one of those
    /// params (the rest reference tool), which is a different thing from
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
        tool_id: ToolId,
    ) -> Result<Effects, SessionError> {
        self.try_with_effects(Some(index), move |session| {
            if !session.tools.iter().any(|t| t.id == tool_id) {
                return Err(SessionError::ToolNotFound(tool_id));
            }
            if session.toolpath_configs.get(index).is_none() {
                return Err(SessionError::ToolpathNotFound(index));
            }
            session.bind_tool_impl(index, tool_id);
            Ok(())
        })
    }

    /// The raw half of [`Self::set_toolpath_tool`], with no effects
    /// wrapper: bind `tool_id` to the toolpath at `index`. The caller
    /// checks that both exist. A bind to the tool already bound does
    /// nothing. [`Self::add_tool`] calls it too (P3 auto-bind).
    pub(crate) fn bind_tool_impl(&mut self, index: usize, tool_id: ToolId) {
        let Some(mut candidate) = self.toolpath_configs.get(index).cloned() else {
            return;
        };
        if candidate.tool_id == Some(tool_id) {
            return;
        }
        let old_tool = candidate.tool_id;
        candidate.tool_id = Some(tool_id);
        // R2: the first tool of an op with no tool applies THE boundary
        // rule of the draft (`apply_first_tool_boundary`).
        self.apply_first_tool_boundary(old_tool, &mut candidate);
        // R1: each empty or suggested feed field takes the Suggest value of
        // the new tool.
        self.seed_feeds(&mut candidate);
        let enabled = candidate.enabled;
        if let Some(tc) = self.toolpath_configs.get_mut(index) {
            *tc = candidate;
        }
        // A different cutter removes different material, so this is a
        // stock-chain change: same invalidation as
        // `edit_toolpath_selection` and `set_heights_config`, not the
        // narrower `remove_result`.
        self.invalidate_result_chain(index, enabled);
    }

    /// R1 (operator ruling 2026-10-30, lead follow-up): THE seed of the feed
    /// fields on a tool change.
    ///
    /// Each feed field of `config` (`feed_rate`, `plunge_rate`,
    /// `spindle_rpm`, and `ramp_feed_rate` when the op has one) that is
    /// EMPTY, or that holds a value Suggest wrote
    /// ([`crate::feeds::ValueProvenance::is_suggested`]), takes the value of
    /// the Apply funnel under `ApplyScope::Speeds`, the funnel that
    /// `Command::ApplyFeeds` runs, with the stamp of the funnel. A suggested
    /// value for the old tool is not a typed value, and it is wrong for the
    /// new tool.
    ///
    /// - A typed value (`Manual`) keeps its value and its stamp.
    /// - An `AutoCorrect` value keeps its value and its stamp, as a typed
    ///   value does: it is not a Suggest value.
    /// - A value with no stamp (a raw config, an old file) keeps its value:
    ///   nothing says that Suggest wrote it.
    /// - The seed locks no field: a typed value replaces a seeded value.
    /// - With no tool, nothing changes.
    /// - When Suggest refuses the new tool (`FeedsError::Unbacked`, a refused
    ///   pairing), or gives no value for a field, each field with a Suggest
    ///   stamp is CLEARED, with its stamp (operator ruling "no underived
    ///   defaults"). The value was derived for the old tool. The field stays
    ///   empty and generation refuses until it is set. An empty field stays
    ///   empty; a typed or `AutoCorrect` value stays.
    ///
    /// The tool doors call it on a tool change only: `bind_tool_impl`
    /// (`SetToolpathTool`, the P3 auto-bind) and `ReplaceToolpathConfig` (the
    /// GUI Tool combo). A new op takes the same funnel in its draft
    /// (`draft_toolpath`). This is a carve-out from F.5 ("the calculator
    /// never writes") and from "no underived defaults": a Suggest value is
    /// derived.
    pub(crate) fn seed_feeds(&self, config: &mut ToolpathConfig) {
        use crate::feeds::FeedsField;
        let Ok(tool) = self.tool_of(config) else {
            return;
        };
        let before = config.operation.as_params();
        let stamps_before = &config.feeds_provenance;
        // A field is held when it has a value that Suggest did not write.
        let held_by = |field: FeedsField, value: bool| {
            value
                && !stamps_before
                    .get(field)
                    .is_some_and(crate::feeds::ValueProvenance::is_suggested)
        };
        let held = [
            (
                FeedsField::FeedRate,
                held_by(FeedsField::FeedRate, before.feed_rate().is_some()),
            ),
            (
                FeedsField::PlungeRate,
                held_by(FeedsField::PlungeRate, before.plunge_rate().is_some()),
            ),
            (
                FeedsField::SpindleRpm,
                held_by(FeedsField::SpindleRpm, before.spindle_rpm().is_some()),
            ),
            (
                FeedsField::RampFeedRate,
                held_by(FeedsField::RampFeedRate, before.ramp_feed_rate().is_some()),
            ),
        ];
        // An op with no ramp field reads `None` there, and the funnel writes
        // no ramp feed to it, so the check is the same for every op.
        if held.iter().all(|(_, held)| *held) {
            return;
        }
        let slots = self.suggest_slots(config);
        let dressups = config.dressups.clone();
        let input = crate::feeds::suggest::OperationApplyInput {
            tool,
            machine: &self.machine,
            material: &self.stock.material,
            spindle_strategy: self.post.spindle_strategy,
            lut: crate::feeds::embedded_vendor_lut(),
            suggest: slots.context(&dressups),
        };
        let mut seeded = config.operation.clone();
        let mut stamps = config.feeds_provenance.clone();
        let refused = crate::feeds::suggest::apply_to_operation(
            input,
            crate::feeds::suggest::ApplyScope::Speeds,
            None,
            &mut seeded,
            &mut stamps,
        )
        .is_err();
        let from = seeded.as_params();
        for (field, held) in held {
            if held {
                continue;
            }
            let op = &mut config.operation;
            if refused {
                Self::clear_suggested_feed(op, &mut config.feeds_provenance, field);
                continue;
            }
            let written = match field {
                FeedsField::FeedRate => {
                    from.feed_rate().inspect(|v| op.set_feed_rate(*v)).is_some()
                }
                FeedsField::PlungeRate => from
                    .plunge_rate()
                    .inspect(|v| op.set_plunge_rate(*v))
                    .is_some(),
                FeedsField::SpindleRpm => from
                    .spindle_rpm()
                    .inspect(|v| op.set_spindle_rpm(Some(*v)))
                    .is_some(),
                FeedsField::RampFeedRate => from
                    .ramp_feed_rate()
                    .is_some_and(|v| op.as_params_mut().set_ramp_feed_rate(Some(v))),
                FeedsField::Stepover | FeedsField::DepthPerPass | FeedsField::ScallopHeight => {
                    false
                }
            };
            if written && let Some(stamp) = stamps.get(field).cloned() {
                config.feeds_provenance.set(field, stamp);
            } else if !written {
                Self::clear_suggested_feed(op, &mut config.feeds_provenance, field);
            }
        }
    }

    /// R1: clear the feed `field` of `op` when its stamp says that Suggest
    /// wrote it, and remove the stamp. A field with no Suggest stamp (empty,
    /// typed, `AutoCorrect`, no stamp) does not change.
    fn clear_suggested_feed(
        op: &mut crate::compute::catalog::OperationConfig,
        stamps: &mut crate::feeds::FeedsProvenance,
        field: crate::feeds::FeedsField,
    ) {
        use crate::feeds::FeedsField;
        if !stamps
            .get(field)
            .is_some_and(crate::feeds::ValueProvenance::is_suggested)
        {
            return;
        }
        let params = op.as_params_mut();
        match field {
            FeedsField::FeedRate => params.unset_feed_rate(),
            FeedsField::PlungeRate => params.unset_plunge_rate(),
            FeedsField::SpindleRpm => params.set_spindle_rpm(None),
            FeedsField::RampFeedRate => {
                let _ = params.set_ramp_feed_rate(None);
            }
            FeedsField::Stepover | FeedsField::DepthPerPass | FeedsField::ScallopHeight => return,
        }
        stamps.clear(field);
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
    /// A pick of the op selection names its item by geometry
    /// (`MatchKey`), not by a face id. After a rebind each pick resolves
    /// against the new model. Operator ruling R7b (2026-10-30): a pick of
    /// the op selection or the boundary selection that names no item
    /// there goes ([`Self::drop_lost_picks`]); [`Effects::picks_dropped`]
    /// names the count.
    ///
    /// A rebind to the model already bound is a no-op and invalidates
    /// nothing.
    #[instrument(skip(self))]
    pub(crate) fn set_toolpath_model(
        &mut self,
        index: usize,
        model_id: crate::ids::ModelId,
    ) -> Result<Effects, SessionError> {
        let mut dropped = None;
        let mut effects = self.try_with_effects(Some(index), |session| {
            if !session.models.iter().any(|m| m.id == model_id) {
                return Err(SessionError::ModelNotFound {
                    id: model_id,
                    known: session.models.iter().map(|m| m.id).collect(),
                });
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
            dropped = session.drop_lost_picks(index);
            session.invalidate_result_chain(index, enabled);
            Ok(())
        })?;
        effects.picks_dropped = dropped;
        Ok(effects)
    }

    /// Operator ruling R7b (2026-10-30): THE rule of a model change. Each
    /// pick of the op selection and of the boundary selection of the
    /// toolpath at `index` that names no item on its (new) model goes:
    /// THE resolution (`resolve_selection_frames`, `Peek` faces, as
    /// `check_keys_name_items` reads it) reports it lost with no match. An
    /// ambiguous pick (two or more matches) stays and refuses as before. A
    /// pick on a model whose faces are not computed yet (a cold cache)
    /// stays: the resolution reports no lost pick for it. A pick on
    /// another model than the toolpath model stays.
    ///
    /// A selection that loses every pick and holds no rule goes back to
    /// its default, the shape a fresh op gets: the op selection to
    /// `OperationConfig::default_selection`, the boundary to
    /// `draft_boundary` (the containment stays the user's choice).
    ///
    /// The caller runs it inside its effects window, after the rebind.
    /// `None` when no pick went.
    pub(crate) fn drop_lost_picks(&mut self, index: usize) -> Option<crate::session::DroppedPicks> {
        use crate::session::SelectionTarget;
        use crate::session::selection_status::FaceAccess;
        let tc = self.toolpath_configs.get(index)?.clone();
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = crate::session::SetupEvalContext::build_for_setup(self, setup);
        let model = tc.model_id;
        let mut count = 0;
        let mut reset = Vec::new();
        let mut next = tc.clone();
        for target in [SelectionTarget::Op, SelectionTarget::Boundary] {
            let Ok(Some(selection)) = tc.selection_of(target) else {
                continue;
            };
            if selection.picks().is_empty() {
                continue;
            }
            let Ok(frames) = self.resolve_selection_frames(&tc, target, &ctx, FaceAccess::Peek)
            else {
                continue;
            };
            let lost: Vec<crate::selection::MatchKey> = frames
                .model
                .lost()
                .iter()
                .filter(|lost| lost.matches == 0)
                .map(|lost| lost.key.clone().for_toolpath_model(model))
                .filter(|key| key.source == crate::selection::SourceRef::ToolpathModel)
                .collect();
            let picks: Vec<crate::selection::MatchKey> = selection
                .picks()
                .iter()
                .filter(|pick| {
                    let pick = (*pick).clone().for_toolpath_model(model);
                    !lost.iter().any(|key| key.matches(&pick))
                })
                .cloned()
                .collect();
            let gone = selection.picks().len() - picks.len();
            if gone == 0 {
                continue;
            }
            count += gone;
            let rules = selection.rules().to_vec();
            let empty = picks.is_empty() && rules.is_empty();
            let kept = crate::selection::Selection::from_parts(
                rules,
                picks,
                selection.excludes().to_vec(),
            );
            match target {
                SelectionTarget::Op => {
                    let value = if empty {
                        reset.push(target);
                        tc.operation.default_selection().unwrap_or(kept)
                    } else {
                        kept
                    };
                    if let Some(slot) = next.operation.selection_slot() {
                        *slot = value;
                    }
                }
                SelectionTarget::Boundary => {
                    if empty {
                        reset.push(target);
                        let has_mesh = self.models.iter().any(|m| m.mesh.is_some());
                        let diameter = self.toolpath_tool(&tc).map(|t| t.diameter);
                        let containment = next.boundary.containment;
                        next.boundary = crate::session::op_draft::draft_boundary(
                            tc.operation.op_type(),
                            has_mesh,
                            diameter,
                        );
                        next.boundary.containment = containment;
                    } else if let crate::compute::config::BoundarySource::Selection {
                        selection,
                        ..
                    } = &mut next.boundary.source
                    {
                        *selection = kept;
                    }
                }
            }
        }
        if count == 0 {
            return None;
        }
        if let Some(slot) = self.toolpath_configs.get_mut(index) {
            *slot = next;
        }
        Some(crate::session::DroppedPicks { count, reset })
    }

    /// Overwrite a toolpath's feeds provenance (W2.1). Does not touch the
    /// result or simulation caches.
    ///
    /// **This has no production caller.** A caller that also writes the
    /// values takes
    /// [`RestoreToolpathSnapshotArgs`](crate::session::RestoreToolpathSnapshotArgs),
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
    /// [`ProjectSession::result_at`] before the call.
    pub(crate) fn remove_result(&mut self, index: usize) -> Effects {
        self.with_effects(Some(index), move |session| {
            session.drop_result(index);
        })
    }

    /// Drop the simulation and bump the simulation epoch.
    ///
    /// **The single site that does either**, the way
    /// [`Self::drop_result`] is for a toolpath's cached answer. Nineteen
    /// mutation sites wrote `self.simulation = None` and moved nothing
    /// else, so an in-flight run that landed after the edit was stored as
    /// if it were current (defect D7). One function makes "the simulation
    /// is gone" and "the project state it answered has moved" one event
    /// with one record, which `Command::AdoptSimulation` then refuses on.
    ///
    /// The epoch bumps unconditionally, whether or not a simulation was
    /// there: two edits must move it twice, or a run submitted between
    /// them reads current.
    ///
    /// `cause` names the class of the edit. The call records the new epoch
    /// against it, so a surface can name why a run is stale
    /// ([`ProjectSession::simulation_drop_causes_since`]).
    ///
    /// The method sits here beside `drop_result` although it names no
    /// toolpath: the two are one rule, and the reader who finds either
    /// needs the other.
    pub(crate) fn drop_simulation(&mut self, cause: SimulationDropCause) {
        self.simulation = None;
        self.simulation_epoch += 1;
        if let Some(slot) = self.simulation_drop_epochs.get_mut(cause.slot()) {
            *slot = self.simulation_epoch;
        }
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
        self.drop_simulation(SimulationDropCause::Operations);
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
        self.drop_simulation(SimulationDropCause::Operations);
    }

    /// Public door onto the chain invalidation for callers that write
    /// `ToolpathConfig` fields directly rather than through a setter.
    ///
    /// The feeds Apply funnel was the one such caller
    /// (`controller/events/mod.rs::apply_feeds_through_funnel`, N13). It
    /// resolved the tool, the machine, the material and the
    /// recommendation from GUI state, then wrote `operation` and
    /// `feeds_provenance` through `toolpath_configs_mut`, so no core
    /// setter ran. F114 replaced it with `Command::ApplyFeeds`, and F126
    /// made that accessor `#[cfg(test)]`. It read
    /// [`ToolpathConfig::generation_inputs_signature`](super::ToolpathConfig::generation_inputs_signature)
    /// either side of that write and called this door when the signature
    /// moved. Before this door existed the core kept the PREVIOUS result
    /// and export emitted it (R0.1 §2.2 item 1), and the downstream
    /// `FromRemainingStock` chain was never invalidated either.
    ///
    /// The GUI inspector was the other such caller until WP5. It takes
    /// [`Command::ReplaceToolpathConfig`](crate::session::Command) now, which
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
