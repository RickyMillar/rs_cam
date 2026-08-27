# Envelope-vs-profile audit, 2026-08-28

> Read-only sweep triggered by the operator's question — *"are there other areas
> that are treating this ball as 6 mm, other moves that aren't using the real
> topology of the bit?"* — after the link-ceiling defect was found and measured
> (`FINDINGS.md` §0g/§0h).
>
> **This is a CODE READ, not a runtime verification.** Magnitudes are analytic.
> §"Not determined" lists what needs a run. Nothing here is fixed.
>
> Context that raises the stakes: the radius tech-debt programme
> (`planning/review_2026-07-29/RADIUS_AUDIT.md`, ~16 waves) was declared
> **complete 2026-08-04**. Most of what follows is NEW, i.e. it survived that
> programme.

Reference tool throughout: tapered ball, ball Ø2.0 (tip radius 1.0), α = 5.7°,
Ø6 shank. `height(r) ≈ 10.02·r − 9.069` past the ball, so the envelope radius
3.0 is first reached **20.99 mm above the tip**.

---

## THE HEADLINE — the rapid-descent blind spot and its detector share it

**U1 — `adaptive3d/clearing.rs:1338-1342` (`sample_stock_top_at`)**, feeding 9
producers and emitted at `adaptive3d/path.rs:1480, :1503, :1534`.
Samples stock with a **zero-radius POINT probe** (and `ray_top`, the cell-centre
read, not `conservative_top`), then emits a **RAPID** descending to
`rapid_floor_z + 0.5`. Off-axis material is invisible; sub-cell slivers are
invisible. The only guard is `RAPID_DESCENT_BUFFER_MM = 0.5` — precisely the
"cell-scaled pad" that `dressup.rs:318-336` says "could only ever chase that
class" and **deleted**. This site kept the pad and never got the query.

**U2 — `collision.rs:450-543` `check_rapid_collisions_against_stock`** (`:526`
`top_z_at`). Also a **zero-radius point probe**, with no cutter argument at all.
And this is the **sole producer of `rapid_collision_count`** — the metric
CLAUDE.md calls *"the most reliable signal… the primary did-anything-bad-happen
indicator"*.

> **The detector shares the defect's blind spot, so the two mask each other.**

A rapid that descends into off-axis material is exactly the event
`rapid_collision_count` cannot see. On a flat Ø6.35 the blind radius is the full
3.175 mm; on the taper, material 0.5 mm off-axis needs only **0.11 mm** of
standing height to strike.

Also at that site: `collision.rs:222-228` and `:352-359` **skip rapids
entirely**, so holder/fixture strikes on a rapid are checked by nothing; and
`check_collisions*` runs against the **mesh only**, never remaining stock.

Both NEW. This is a safety finding, not an efficiency one, and it outranks
everything else in this document.

---

## U3 — the whole load lane's engagement is normalised by the SHANK

`dexel_stock/stamping.rs:1119`: `(perp_max − perp_min) / (2.0 * radius)` with
`radius` threaded from `compute/simulate.rs:1032` as `envelope_radius_mm()`.
One scalar serves two questions in the same accumulator — the stamp bbox
(envelope, correct) and the **engagement denominator** (wrong; should be
`engagement_radius_mm(axial_doc)`).

Divisor **3.46× too large** at 0.5 mm DOC; arc `acos(1−2w)` **1.94× under-read**.

Three consequences, all NEW:

- **U3a — silent population loss.** The `radial_woc_fraction < 0.02` sample
  filter (`chipload.rs:315`, `power.rs:192`, `deflection.rs:87,:219`) discards a
  real **0.12 mm side bite** as air. A finishing pass at 0.1 mm stepover empties
  every gate → `Unmodeled{AllSamplesAirCutOrRapid}`, or `Within` with
  `peak_idx: None`. This is CLAUDE.md's own *"a gate handed an empty population
  passes and looks healthy"*, reached structurally.
- **U3b — `tool_load/power.rs:205-206`** multiplies a *correct*
  `engagement_radius(axial_doc)` by the *envelope-normalised* arc. Self-
  inconsistent inside one formula; ~1.9× power under-read.
- **U3c — air-cut % is poisoned.** `simulation_cut.rs:919` → `air_cut_time_s` →
  `air_cut_pct_of_total_runtime`: the same 0.02 threshold labels genuine cutting
  `AirCut`. **The operator has been optimising air-cut % on a tapered ball with
  an instrument whose zero is set by the shank** — and it is the metric this
  whole finishing campaign has been measured against.

---

## Gates and envelopes that read LOW (silent pass)

| # | site | now → correct | effect | ledger |
|---|---|---|---|---|
| U4 | `tool_load/optimize/context.rs:107` (`:106` is right) | `tool.diameter()` → `lookup_diameter_at` | Ø6 vendor row for a Ø2 cutter; chipload gate says `Within` on an overloaded tool, and it **blocks export**, so low = silent pass. Reached by scallop, V-carve, drill | NEW |
| U5 | `feeds/cutter_constraints.rs:206` | shaft → `lookup_diameter_at` | vendor axial cap **3× too permissive** (a `1×D` row allows 6.0 mm DOC on a tool engaging ~2.3). `chipload.rs:550` solves the identical question correctly | NEW |
| U6 | `feeds/cutter_constraints.rs:279`, `tool_load/optimize/preflight.rs:191` | `immersion_angle(woc, tool.radius())` → WIDTH(doc) | under-reads ψ → under-reads force → **over-states `max_doc_deflection_mm`**; preflight fails to refuse an unmakeable op. 1.44–1.74× chip-thickness under-read | **LEDGERED WRONG** |
| U7 | `compute/execute.rs:566,:592,:627` | `radius()*2`, `ToolProfile::Flat` hardcoded | R-12 verbatim: all three drill gates divide 3× low, two block export | **LEDGERED OPEN (R-12)** |
| U8 | `dexel_stock/mod.rs:683` `apply_drill_op` | envelope diameter + `Flat` | R-12's **removal** twin: the sim carves a **Ø6 hole where a Ø2 tip cuts** (9× area). Downstream `FromRemainingStock` ops then believe material is gone that is physically there | **NEW**, extends R-12 |
| U9 | `feeds/suggest.rs:2316` → `geometry_class.rs:153` | tip → envelope (a *fit* question) | required bore 3× too small → helix entry selected into a pocket the **shank will not fit** | NEW |
| U10 | `session/compute.rs:1199`, viz `controller/events/compute.rs:418` | `prev_tool_radius = diameter/2` (tip) → engaged-at-depth | `rest.rs:9-10` insets by this; region marked cleared is too large ⇒ **rest skips material the rougher never reached**. The *same function* takes the envelope for the current tool — a 3× asymmetry in one subtraction | NEW |
| U11 | `ui/properties/mod.rs:2599-2611,:2331-2334`; `ui/feeds_modal.rs:2436-2489` | nominal/tip + cylinder → profile | three operator-trusted surfaces mis-drawn: the engagement diagram draws a **taper as a cylinder** under a "Show the math" label; the LUT viewer green-highlights a different row than the recommendation used | NEW |
| U12 | `rs_cam_cli/src/job.rs:296-297,325,327,328,475` | tip → envelope | assembly built with `shank_diameter = 2.0` on a Ø6 shank — **the shank is modelled thinner than the cutter it sits on**, so `collision.rs:97/245` filters it and a shank strike cannot be detected | NEW |

## SAFE but costly

- **S1 — `dressup.rs:337-342` `optimize_entry_descents`** — the *direct sibling*
  of the fix just landed. Flat disc at the envelope; every entry stops higher
  than needed and the surplus is spent as **fed plunge** (up to ~9 mm per entry
  on this board). **LEDGERED "DO NOT TOUCH"** (§8 item 9, A/M10) — that ruling
  predates the profile primitive and should be revisited.
- **S2** `session/compute.rs:4356` auto sim resolution `envelope/5` → 0.5 mm
  cell for a Ø2 tip (4 cells across it); correct scale is cusp. LEDGERED, open.
- **S3** `scallop.rs:1826`, `steep_shallow.rs:559` still on
  `legacy_envelope_quarter` — 2 of 4 consumers never migrated. LEDGERED.
- **S4** `feedopt.rs:139,157` samples the circle at the envelope and compares to
  the tip Z; at r = 3.0 the tool is 21 mm up. One-directional (over-reads ⇒
  slows down), opt-in. NEW.
- **S5** `pencil.rs:1419` search bound is the **TIP** — an *under*-reach: material
  at r ∈ (1.0, 1.9] that stands above `height(r)` is never sampled. Widening to
  the envelope is now **strictly better and free**. NEW.
- **S6** adaptive3d "Optimal load" slider maps on the tip; the engine uses
  `engagement_radius_mm(dpp)`. Cut is lighter than asked and the **displayed %
  is over-stated** (20% low at DPP 3, 90% at DPP 10). NEW.
- **S7** `state/multitool_planner.rs:71-82` `PlannerToolRow` carries no envelope,
  so `rim_erosion_mm` can never be seeded from the dialog despite its own
  tooltip saying it should. NEW.
- S8–S10: 2.5D containment at the envelope (LEDGERED KEEP), helix sizing
  (LEDGERED §7.7), assorted tip-diameter derates.

## Verified already CORRECT (so fixed ≠ unfixed stays legible)

Dexel stamping removal uses the true profile via `RadialProfileLUT`
(`radial_profile.rs:34-49`). `dropcutter.rs:29` / `pushcutter.rs:63` use the
envelope only as a query **bound**, with shape carried by
`edge_drop`/`facet_drop`/`vertex_drop`. `reach.rs` is a built valley-fit
primitive and `attach_generic_rest_analysis` routes through it (A1/A3/A5/A7
closed). `rest_field.rs:757,883` on cusp radius (F1). `chipload.rs:550,579-580`
and `tool_load/mod.rs:309` select the vendor row on the **engaged** diameter.
`tier_map.rs:678-708` decides tier membership from real drop-cutter residuals,
never a radius. `tool/mod.rs:665-668` integrates deflection per station via
`lookup_diameter_at`.

**Two ledger corrections:**

1. `TOOL_SCALE_SEMANTICS.md` §8 item 7 lists U6's two sites under *"must NOT
   touch — keep the envelope"* (Rule 4, feeds/force/deflection). **Rule 4 is
   wrong for those two**: they are immersion-angle computations, i.e. WIDTH(doc)
   questions wearing a force-lane badge. The rule was written to protect
   physical-extent sites and over-reached.
2. **CLAUDE.md is stale** where it says the GUI chipload heat-map "still carries
   the same mismatch on a visible surface". F-HEATMAP is **closed** —
   `render/toolpath_render.rs:590` now takes distinct newtypes so an arc-mean
   cannot reach a band.

---

## The shared primitive: adoption, not construction

It already exists — built with the link fix, and public:

```rust
// dexel_stock/mod.rs:486
pub fn max_clearance_tip_z_for_profile(
    &self, cx: f64, cy: f64, radius: f64, cutter: &dyn MillingCutter,
) -> Option<f64>
```

Sentried by `tests/profile_link_ceiling.rs`, whose
`flat_endmill_profile_ceiling_is_byte_identical` is the safety anchor.

Three changes make it general:

1. **A LUT variant for the hot path** — `RadialProfileLUT::height_at_dist_sq`
   is sqrt-free and the simulator already builds one per toolpath. **Caveat to
   document:** the LUT linearly interpolates a convex ball cap and *overshoots*
   `h`, which **lowers** the required tip Z — the unsafe direction. < 0.1 µm at
   4096 samples for the *stamp*; the clearance use has a different error budget
   and must state its own bound or floor to the lower sample.
2. **Fuse the two readings** — `material_top` and `required_tip_z` walk the
   identical cell set and `surface_link.rs:609,:633` call both per sample. One
   fused pass makes the profile version net *cheaper* than the pre-fix code.
3. **Adopt in order:** `optimize_entry_descents` (S1) → `sample_stock_top_at`
   (U1) → `check_rapid_collisions_against_stock` (U2) → `pencil.rs:1419` (S5).
   Every one already has a cutter or a LUT in scope.

> **The rule adoption must carry, or it becomes a defect:** the primitive
> relaxes a **HEIGHT**, never a **DECISION TO ACT**. `surface_link.rs:1438-1445`
> (pencil's lift trigger) and `:607-610` (flush-ride) deliberately keep the flat
> `material_top`, because a lower reading fires the guard *less* often. Lift
> that reasoning into the primitive's own doc.

**Two things it is NOT:** valley fit/routing (that is `reach.rs` — "can the tool
get *into* this", a different question), and engagement normalisation (U3 is a
*local* fix at `stamping.rs:1119` where the axial DOC is already in scope; do
not route it through a stock query).

*Cosmetic:* rename `LinkCeiling::tool_radius` → `search_radius_mm`; the doc
already says it is a bound, the name still says otherwise.

---

## Not determined without running code

1. **The cost of S1.** The disc walk is boundable analytically (~177 cells at
   0.5 mm); the per-generation entry count and plunge seconds are not.
2. **Whether U1/U2 have ever produced a real strike on a shipped project.**
   `rapid_collision_count` *cannot* answer this — U2 shares U1's blind spot.
   Needs emitted G-code replayed against a fine-grid dexel, i.e. the repo's own
   "measure emitted motion, not the plan" rule.
3. **U3's magnitude on a real trace** — what fraction of a wanaka trace falls
   under the 0.02 filter, and how much air-cut % is misattributed.
4. **Whether fixing U3 turns any green gate red.** It raises engagement, arc and
   power across the board on tapers. By the instrument-integrity rule, several
   shipped verdicts on tapered tools should be treated as **provisional** until
   answered.
5. Whether the LUT overshoot is acceptable for the clearance error budget.
6. Whether U10's rest under-cut is observable in mm² on a real cascade.
7. Whether any viz-side path calls `check_collisions` against stock.
