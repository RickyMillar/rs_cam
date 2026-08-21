# RESEARCH — the drill `MoveIntent` erasure, and what it is actually a symptom of

Read/test-only lane, 2026-08-21. Branch `tech-debt-3` @ `1d6dd855`.
Nothing in this file was fixed; no tracked file was edited.

**Do not read this as a replacement for `BASELINES.md`.** It is one lane's
research note.

---

## 0. TL;DR — the finding reproduces, and the intent erasure is the small half

The intake said: *drill.rs tags rapids `Retract`/`Linking` at four sites but
the stored toolpath reads `Unknown`; and peck re-entry rapids descend to full
safe-Z instead of the R-plane.*

Both halves reproduce. The mechanism is **one pass**, named below with a
cite. But the intake's cost model — "a per-peck wasted *rapid*" — understates
it by roughly 150×, because the pass does not merely *raise* the re-entry
rapid: it **deletes the R-plane rapid entirely**, so the tool re-enters the
hole on a **G1 feed from safe-Z**.

Shipped defaults, one hole (depth 10, `Peck(3)`, Ø6, R-plane 5.0, safe-Z 17.0,
F300):

| | fed distance | fed time | vertical rapid | Z-axis time |
|---|---:|---:|---:|---:|
| what `drill.rs` emits | 17.0 mm | **3.400 s** | 97.0 mm | 4.855 s |
| what ships after dressups | 105.0 mm | **21.000 s** | 105.0 mm | 22.575 s |

**+17.72 s per hole, of which 17.60 s (99.3 %) is feed and 0.12 s (0.7 %) is
rapid.** The rapid-only framing in the intake is the 0.7 %.

And it is machine-visible: the G1 lines in the exported program change (§3).

Two further results that matter more than the seconds:

* **No simulation surface reports it.** `drill_summaries.feed_time_s` reads
  **6.800 s in both arms** on the same two-hole fixture whose emitted feed
  distance differs by 6.2× — the drill metric is computed from
  `fed_descents(config)`, never from the emitted moves. And a drill toolpath
  gets **no `toolpath_summaries` row at all**, so there is no
  `total_runtime_s` either. Measured, §5.
* **The erasure is not drill-only.** Pocket loses the intent on **66 of 99**
  rapids under the same pass. It loses no geometry, because pocket's rapids
  were already at safe-Z. Drill is the family that gets hurt because drill is
  the family whose rapids are *deliberately below* safe-Z. Measured, §6.

Ledger: the intake's description was directionally right and quantitatively
wrong in the safe direction. It is recorded here as a correction to the cost
model, not to the finding.

---

## 1. Reproduction

Harness: a scratch integration test built on the W6 census's own fixture
(flat 100×80×12 stock, `origin_z = -12` so stock top is world Z = 0; one Ø6
end mill; `DrillConfig::default()` = depth 10, `Peck`, peck 3.0,
`retract_z` 2.0, feed 300). Generated through the production
`ProjectSession::generate_toolpath`, exactly as the census does, then the
stored `result.toolpath()` was dumped move by move. The file was created new,
run, and **deleted**; it is reproduced in §8 as a recipe.

### 1a. What `drill.rs` emits (raw generator, one hole)

```
[ 0] Rapid  -> (20,20, 17.000)  Linking     <- safe_z
[ 1] Rapid  -> (20,20,  5.000)  Linking     <- R-plane
[ 2] Linear -> (20,20,  2.000)  Drilling    F300
[ 3] Rapid  -> (20,20,  5.000)  Retract
[ 4] Rapid  -> (20,20,  2.500)  Linking     <- re-entry = to_z + 0.5
[ 5] Linear -> (20,20, -1.000)  Drilling
...
[15] Rapid  -> (20,20,  5.000)  Retract
```

16 moves. Intents present. Re-entry at `to_z + PECK_REENTRY_CLEARANCE_MM`.
This is a correct Fanuc-G83 shape.

### 1b. What the session stores (shipped defaults, same hole)

```
[ 0] RAPID  -> (20,20, 17.000)  Unknown
[ 1] LIN    -> (20,20,  2.000)  Drilling   F300   <- FEEDS from 17.0
[ 2] RAPID  -> (20,20, 17.000)  Unknown
[ 3] RAPID  -> (20,20, 17.000)  Unknown           <- zero-length
[ 4] LIN    -> (20,20, -1.000)  Drilling   F300   <- FEEDS from 17.0
...
[14] RAPID  -> (20,20, 17.000)  Unknown
```

15 moves. `TOTALS: unknown=20 retract=0 linking=0 drilling=10` on the two-hole
run — the census's "Drill emits zero Retract-tagged moves" reproduces exactly.

Three separate damages, not one:

1. **Intents on rapids are gone** (`Retract` → `Unknown`).
2. **The R-plane rapid is gone.** Move 0 → move 1 is safe-Z → cut depth at
   **F300**.
3. **Every retract goes to safe-Z**, and each is followed by a **zero-length
   duplicate rapid**.

### 1c. The controlled arm — one flag

With `optimize_rapid_order = false` and nothing else changed, the stored
toolpath is the generator's output verbatim: 32 moves for two holes,
`unknown=0 retract=10 linking=12 drilling=10`, R-plane rapids at 5.0,
re-entries at 2.5 / −0.5 / −3.5 / −6.5.

**One flag, all three damages. Causality established.**

---

## 2. Mechanism, with cites

The erasing pass is **`crate::tsp::optimize_rapid_order`**, specifically
`rebuild_group`.

| step | where |
|---|---|
| `optimize_rapid_order: true` in `DressupConfig::default()` (Roadmap B.6) | `crates/rs_cam_core/src/compute/config.rs:1703` |
| Drill / AlignmentPinDrill / DropCutter / ProjectCurve get `allows_global_rapid_reorder = true` | `crates/rs_cam_core/src/compute/catalog.rs:395-397` |
| the dressup call (unbarriered arm; drill has no barriers) | `crates/rs_cam_core/src/compute/execute.rs:3256-3276` |
| the dressup call (barriered arm — this is the one Pocket/Profile take) | `crates/rs_cam_core/src/compute/execute.rs:3028-3049` |
| **rapids are discarded** on the way in | `crates/rs_cam_core/src/tsp.rs:36` — *"Rapids between segments are discarded (they will be regenerated)"*, and `Segment::src_range` at `:17-21` excludes them |
| **rapids are regenerated at `safe_z` with no intent** | `crates/rs_cam_core/src/tsp.rs:387-428` (`rebuild_group`) |
| `Toolpath::rapid_to` is `rapid_to_with_intent(target, MoveIntent::Unknown)` | `crates/rs_cam_core/src/toolpath.rs:137-139` |

`rebuild_group` is five `result.rapid_to(P3::new(_, _, safe_z))` calls
(`tsp.rs:409, 411, 414, 415, 428`). Each is the intent-less constructor, and
each hard-codes `safe_z` as the Z. The pass has **no concept of an R-plane**
— nor could it, since it discards the only moves that expressed one.

Cutting moves are `m.clone()`d (`tsp.rs:421`), which is why `Drilling` on the
feeds, and `LeadIn`/`EntryPlunge`/`FinishingCut` elsewhere, survive. **Only
rapids lose their intent.**

### 2a. Why a peck drill always hits the slow path

`optimize_one_group` (`tsp.rs:250-292`) has two verbatim fast paths — zero
segments, and **exactly one** segment — and only calls `rebuild_group` when a
group holds ≥ 2 cutting segments. A peck cycle is 5 feeds separated by rapids
⇒ 5 segments per hole ⇒ always rebuilt. Even `Simple`/`Dwell` is rebuilt as
soon as there are ≥ 2 holes.

That single-segment fast path is also why Profile and Trace show **zero**
damage in §6: on this fixture each barrier group holds one segment.

### 2b. The frame confusion that makes it silent

`generate_drill` (`execute.rs:748-757`) sets

```rust
safe_z:    ctx.heights.retract_z,                                    // 17.0
retract_z: effective_safe_z(cfg.retract_z, ctx.stock_bbox.max.z),    // max(2.0, 0+5) = 5.0
```

`effective_safe_z` (`compute/config.rs:1240-1242`) floors the user's R-plane
at `stock_top + SAFE_Z_CLEARANCE_MM` (5.0). So on any default project the
R-plane is `stock_top + 5.0` and safe-Z is `stock_top + 17.0` — 12 mm apart,
and `drill_op.rs:118-129` (R-2) documents that the emitter roots the peck grid
at the R-plane *by design*. TSP then re-roots it at safe-Z, and the R-2
invariant ("the cycle is described **once**") is broken downstream of the two
places R-2 taught to agree.

---

## 3. It is machine-visible

Same fixture, one hole, `emit_gcode` against the shipped GRBL post.

`optimize_rapid_order = false` (what the generator means):

```
G0 X20.000 Y20.000 Z17.000
G0 X20.000 Y20.000 Z5.000
G1 X20.000 Y20.000 Z2.000 F300
G0 X20.000 Y20.000 Z5.000
G0 X20.000 Y20.000 Z2.500
G1 X20.000 Y20.000 Z-1.000 F300
...
```

Shipped default:

```
G0 X20.000 Y20.000 Z17.000
G1 X20.000 Y20.000 Z2.000 F300
G0 X20.000 Y20.000 Z17.000
G0 X20.000 Y20.000 Z17.000
G1 X20.000 Y20.000 Z-1.000 F300
...
```

No canned cycle is synthesized anywhere — `gcode/mod.rs` reads `DrillOp` only
for the load report (`gcode/mod.rs:589,604`); the program comes from the
linearized toolpath. So **the emitted G1 lines are different**, and any fix
that restores the R-plane is a machine-visible motion change. Flagged in §7.

---

## 4. The cycle-time number, with the formula

Symbols (all setup-local Z):
`R` R-plane, `S` safe-Z, `T` stock top, `D` depth, `p` peck depth,
`c = PECK_REENTRY_CLEARANCE_MM = 0.5` (`drill.rs:120`),
`f` plunge feed mm/min, `V` rapid rate mm/min, `N` holes.

Peck schedule (`drill::fed_descents`, `drill.rs:164-190`), rooted at `R`:

```
k    = ceil( (R - (T - D)) / p )
z_i  = max(R - i*p, T - D),   i = 1..k
```

Shipped defaults `R=5, T=0, D=10, p=3` ⇒ `k = 5`, `z = [2, -1, -4, -7, -10]`.
(This is the `peck_count 5 / feed_time 3.400 s` pair `CLAUDE.md` already
carries.)

**Fed distance per hole**

```
L_gen = p + (k-1)(p + c)              = 3 + 4(3.5)   = 17.0 mm
L_tsp = SUM_i (S - z_i) = k*S - SUM z_i = 5(17) + 20 = 105.0 mm
```

**Vertical rapid distance per hole** (including the climb back to `S` for the
traverse to the next hole)

```
Rap_gen = (S - R) + SUM_{i<k} [ 2(R - z_i) - c ] + (R - z_k) + (S - R)
        = 12 + (5.5+11.5+17.5+23.5) + 15 + 12   = 97.0 mm
Rap_tsp = SUM_i (S - z_i)                        = 105.0 mm
```

(the four zero-length duplicate rapids contribute 0)

**Times.** `f = 300` (`DrillConfig::default()`); `V = machine.max_feed_mm_min
= 4000` for `MachineProfile::generic_wood_router` (`machine.rs:162`), which is
what the simulator uses as `rapid_feed_mm_min` unless
`post.high_feedrate_mode` is on (`session/compute.rs:908-912`).

```
t_gen = 17.0/300*60 + 97.0/4000*60  = 3.400 + 1.455 = 4.855 s / hole
t_tsp = 105.0/300*60 + 105.0/4000*60 = 21.000 + 1.575 = 22.575 s / hole
Δ     = 17.720 s / hole   (4.65x)
        feed  17.600 s  (99.3 %)
        rapid  0.120 s  ( 0.7 %)
```

**Per job** (Δ · N):

| holes | lost |
|---:|---|
| 8 | 2 min 22 s |
| 24 (shelf-pin strip) | 7 min 05 s |
| 100 | 29 min 32 s |

### 4a. All three cycles, measured (2 holes, same fixture)

| cycle | arm | moves | fed mm | fed time |
|---|---|---:|---:|---:|
| `Peck` (G83) | TSP off | 32 | 34.0 | 6.800 s |
| `Peck` (G83) | **shipped** | 30 | **210.0** | **42.000 s** |
| `ChipBreak` (G73) | TSP off | 24 | 34.0 | 6.800 s |
| `ChipBreak` (G73) | **shipped** | 30 | **210.0** | **42.000 s** |
| `Simple` (G81) | TSP off | 8 | 30.0 | 6.000 s |
| `Simple` (G81) | **shipped** | 6 | **54.0** | **10.800 s** |

G73 is the worst case *in kind*, not just in seconds: its whole point is a
small lift between pecks (`drill.rs:257-266`, `to_z + retract_amount`). TSP
turns every one of those lifts into a full trip to safe-Z and a fed return —
G73 becomes a *worse-than-G83*, and the operator's choice of cycle is
inverted. G81 loses only the R-plane approach (feed from 17 instead of 5), a
1.8× fed-distance penalty.

---

## 5. Collateral, part 1 — nothing measures it

Ran the full `ProjectSession::run_simulation` on the two-hole peck fixture in
both arms:

```
tsp=false  DRILL SUMMARY holes=2 feed_time_s=6.800
tsp=true   DRILL SUMMARY holes=2 feed_time_s=6.800
```

and **no `toolpath_summaries` row was emitted at all** in either arm.

Two independent reasons, both structural:

* `DrillToolpathSummary.feed_time_s` is built by
  `drill_metrics::build_drill_toolpath_summary` from
  `fed_descents(drill_op.cycle, hole.bottom_z, drill_op.retract_z_mm)`
  (`drill_metrics.rs:277-279, 330-340`). Its input is the **config**, and
  `DrillOp.retract_z_mm` is the *pre-dressup* R-plane (5.0, confirmed in the
  dump). It cannot see the emitted moves, so it reports the cycle the
  generator intended forever, whatever ships.
* Drill toolpaths take the analytical branch in the simulator
  (`compute/simulate.rs:942-960`, `group_stock.apply_drill_op(...)`) which
  produces **no `SimulationCutSample`s** — hence no per-toolpath summary, hence
  no `total_runtime_s`, no `cutting_runtime_s`, no air-cut percentage.

And the one instrument that *would* have caught it is switched off by policy:
`OperationType::air_cut_high_threshold_pct` is `None` for
`Drill`/`AlignmentPinDrill` (`catalog.rs:456-491` — "dexel can't see Z-only
moves").

So: 88 mm of extra **fed air per hole** is reported by nothing, on any surface,
in CLI, GUI or MCP. This is precisely the `feedback_instrument_integrity`
failure mode — a docstring (`drill_op.rs`'s R-2 note, `CLAUDE.md`'s
`feed_time 3.400`) describing a cycle that stopped being emitted.

`rapid_collision_count` is unaffected: the rapid-collision check reads the
linearized path (`simulate.rs:925-934`), so the safe-Z rapids are checked as
emitted. That signal stays trustworthy, consistent with `CLAUDE.md`.

---

## 6. Collateral, part 2 — what else the pass normalizes

Measured on the same flat fixture, TSP off vs on, counting rapids:

| op | moves | rapids | rapid intent `Unknown` | rapids at safe-Z | zero-length rapid pairs |
|---|---:|---:|---:|---:|---:|
| Pocket, tsp=off | 330 | 99 | **0** | 66 | 0 |
| Pocket, **tsp=on** | 330 | 99 | **66** | 66 | 0 |
| Profile, off/on | 42 | 9 | 0 / 0 | 6 / 6 | 2 / 2 |
| Trace, off/on | 36 | 12 | 0 / 0 | 9 / 9 | 0 / 0 |
| Drill, tsp=off | 32 | 22 | **0** | 2 | 0 |
| Drill, **tsp=on** | 30 | 20 | **20** | 20 | **8** |

Reading:

* **Intent erasure is generic.** Pocket goes through the *barriered* arm and
  loses two thirds of its rapid intents. Any op whose group holds ≥ 2 segments
  does. This is the mechanism behind the census's zero-Retract columns, and it
  is not confined to drill.
* **Geometry damage is drill-specific — on today's generators.** Pocket's move
  count and safe-Z rapid count are *identical* across the arms: its rapids
  were already at safe-Z, so re-planting them there is a no-op. Drill is hurt
  because drill is the only shipped family that deliberately puts rapids
  *below* safe-Z. The exposure is structural, though: any future generator
  that emits an intermediate clearance height — DropCutter and ProjectCurve
  are already in the unbarriered set (`catalog.rs:395`) — is silently
  normalized to safe-Z.
* **Zero-length rapids** are pure emitted-program noise, 8 per two holes.
  Harmless to the machine, but they inflate move counts and every
  per-move-indexed structure downstream.

### 6a. Adjacent — `RetractStrategy` is a dead dial

`DressupConfig::retract_strategy` (`compute/config.rs:1604-1612, 1662, 1704`)
offers `Full` / `Minimum` ("retract just above the highest Z on nearby path
+ 2 mm (faster)"). It is settable from the GUI Linking tab
(`rs_cam_viz/src/ui/properties/mod.rs:5042-5050`), counted in the panel's
"active dressups" badge (`:4807`), and exposed by MCP `set_dressups`
(`mcp_server.rs:1130`). **No consumer exists in `rs_cam_core` outside the
definition** — `rg 'RetractStrategy::(Full|Minimum)' crates/rs_cam_core/src`
returns exactly one hit, the default. `apply_dressups` never reads it.

Noted because it is the dial a user would reach for to fix this, and it does
nothing.

---

## 7. Proposed fix shapes

Three, in increasing order of blast radius. They are not exclusive; **A is a
strict prerequisite for any of them being verifiable.**

### A. Sentry first, no behaviour change — *recommended first move*

Pin the invariant the census could not: **the emitted drill toolpath's fed
descents must equal `drill::fed_descents(cycle, bottom_z, retract_z_mm)`** —
the R-2 contract, asserted against emitted motion rather than against the
config on both sides. Today both sides of that comparison read the config, so
it is vacuous (the `gate_population_vacuity` lesson, applied to a metric).

* Files: 1 new test under `crates/rs_cam_core/tests/`.
* LOC: ~120.
* Risk: none. G-code unchanged.
* It goes **red on master**, which is the point (`feedback_commit_instruments_before_gates`).

### B. Preserve intent through `rebuild_group` — cheap, partial

Give `rebuild_group` an intent for the rapids it synthesizes:
`MoveIntent::Retract` for the lift off a segment end, `MoveIntent::Linking`
for the traverse and the descent onto the next segment start. That is what
every generator already means by those moves, and `dressup.rs:349-352` sets
the same precedent for a synthesized linking rapid.

* Files: `crates/rs_cam_core/src/tsp.rs` (also the 3 `rapid_to` calls in
  `dressup::filter_air_cuts`, `dressup.rs:2073-2089`, for consistency).
* LOC: ~15.
* Risk: low. **No geometry moves; no G-code changes** (intent is not
  serialized — `Move` has no serde impl, `toolpath.rs:118-124`).
* Fixes: the census columns, transit classification for gates
  (`toolpath_spans` falls back to the per-move intent union when a span is
  dropped — `tsp.rs:120-126`), and the `CLAUDE.md` claim §4c of
  `DELTA_sim_w6_playback.md` flagged as unsupported.
* Does **not** fix a single second of cycle time.

### C. Make the pass R-plane-aware — the actual fix, and it changes G-code

The clean shape is not to teach TSP about drills. It is to stop TSP from
*inventing* clearance heights it has no information about. Two candidate
framings:

**C1 — per-op clearance height (smaller).** Thread an
`Option<f64> rebuild_clearance_z` through `optimize_rapid_order` /
`rebuild_group` alongside `safe_z`, set from the operation
(`generate_drill` already computes it as `params.retract_z`). Intra-group
rapids use it; inter-group / first / last use `safe_z`. Restores the R-plane
for drill and leaves every other family byte-identical (their rapids are at
safe-Z already, §6).

* Files: `tsp.rs`, `compute/execute.rs` (2 call sites + `apply_dressups`
  signature), `compute/catalog.rs` or the `ExecutionContext` to carry the
  value, `rs_cam_viz/src/compute/worker` if it calls `apply_dressups` directly.
* LOC: ~120–180.
* Risk: **medium — machine-visible.** See the decision flags below.

**C2 — capability refusal (smallest, bluntest).** Flip
`allows_global_rapid_reorder` to `false` for `Drill`/`AlignmentPinDrill` and
let the fast path preserve the generator's output verbatim. Costs the TSP
hole-ordering win that `drill_capability_allows_tsp_reorder_reduces_rapid`
(`capability_link_moves_safety.rs:844-882`) exists to defend — that sentry
would have to be retired or re-scoped, which makes it a *deliberate* trade
rather than a quiet one. **Not recommended**: hole-order optimisation on a
100-hole job is worth real XY travel, and the defect is in the rebuild's Z
planning, not in the reorder.

**C3 — reorder holes, not segments (best long-term, biggest).** The real
mismatch is that `split_into_segments` treats a peck cycle's five feeds as
five independently-reorderable segments when they are one atomic hole. A
`RapidOrderBarrier` per hole in `generated_with_drill_spans` would put each
hole in its own group; each group would then hold 5 segments and still be
rebuilt, so **C3 alone does not fix it** — it needs C1's clearance height too,
or a per-group "one segment ⇒ verbatim" widening to "one *hole* ⇒ verbatim".
Noted so it is not mistaken for a cheaper C1.

### Decision flags — for the user, not for an implementer

1. **C1/C2/C3 change emitted G-code on every existing drill toolpath.** The Z
   values on `G1` lines move (feed starts at the R-plane, not safe-Z) and move
   counts change. Any project regenerated after the fix produces a different
   program from the one that was proven on the machine. This is a *restoration*
   of the documented intent, and it is strictly less aggressive motion — but it
   is still a motion change and must be the user's call.
2. **It will change reported numbers on nothing**, because nothing reports it
   (§5). Do fix A first, or there is no before/after to show.
3. **`RetractStrategy` (§6a) should be decided at the same time** — either
   wire `Minimum` up (it is the same "don't retract further than you must"
   idea C1 implements) or remove it from the GUI/MCP surface. Shipping a
   fix for the drill R-plane while leaving a dead retract dial next to it in
   the same panel is the worse of the two outcomes.
4. **Scope question for whoever takes it:** should `rebuild_group` be allowed
   to synthesize rapids at all? Every one of the three damages here comes from
   the same design choice — a reorderer that throws away motion it does not
   understand and re-invents it from one scalar. An alternative shape is for
   TSP to *reorder* the input's own framing rapids rather than replace them.
   That is a larger change than any of C1–C3 and is not costed here.

---

## 8. Reproduction recipe

The scratch test file was created, run, and deleted per the lane's
constraints. To recreate: copy the fixture helpers from
`crates/rs_cam_core/tests/retract_intent_move_type_census_w6.rs`
(`flat_stock`, `add_rect_polygon`, `add_tool`, `toolpath_config`), then for
`tsp in [false, true]`:

```rust
let mut dressups = DressupConfig::for_op(OperationType::Drill);
dressups.optimize_rapid_order = tsp;           // the only variable
// ... ToolpathConfig { operation: OperationConfig::Drill(DrillConfig {
//        depth: 10.0, selected_holes: Some(vec![[20.0,20.0],[40.0,20.0]]),
//        ..Default::default() }), dressups, .. }
session.generate_toolpath(0, &cancel).unwrap();
let tp = session.get_result(0).unwrap().toolpath();
// dump (move_type, target, intent) per move
// G-code:  gcode::emit_gcode(tp, gcode::post::grbl(), 18_000)
// sim:     session.run_simulation(&SimulationOptions { resolution: 0.5, .. }, &cancel)
//          -> cut_trace.drill_summaries[0].feed_time_s   (6.800 in BOTH arms)
```

All cargo invocations were `flock /tmp/rs_cam_cargo.lock cargo test
-p rs_cam_core --test <name> -j 8 -- --nocapture`, gated on ≥ 20 GB available;
debug profile; another session's builds were waited on, never interrupted.

---

## 9. What this lane did not establish

* **No dynamic census across the 56-family sweep.** §6 covers Pocket, Profile,
  Trace and both drill families on one flat fixture. DropCutter and
  ProjectCurve are in the same unbarriered capability set as Drill
  (`catalog.rs:395`) and were **not** measured — if either emits an
  intermediate clearance height, it has the same geometric exposure.
* **The wall-clock claim is arithmetic, not a bench measurement.** It is
  emitted-distance ÷ commanded rate, ignoring acceleration. The
  acceleration-aware integrator (`machine_kinematics::compute_cycle_time`)
  would make the *fed* number slightly worse (a 27 mm G1 at F300 spends
  proportionally less time accelerating than five short ones) and the rapid
  number slightly better. The 99.3 %-of-the-loss-is-feed split does not depend
  on that correction.
* **No `.toml` project fixture was run.** Everything here is a synthetic
  session. A wanaka-class project with real hole counts would give a headline
  number; this file gives the per-hole rate to multiply.
