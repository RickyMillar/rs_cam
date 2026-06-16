# Adaptive3d deep-channel gouge — root cause + engine-fix research

> ## ⚠️ UPDATE 2026-06-16 (later) — the radius-dilation root cause below is REFUTED
>
> The "PRECISE mechanism + fix" section (dilate the leave-surface by the
> cutter radius) is **wrong** and must not be implemented. Two findings kill it:
>
> 1. **The `SurfaceHeightmap` is ALREADY cutter-radius-dilated.** It is built
>    by `point_drop_cutter` (slope.rs:89). For a flat endmill, drop-cutter
>    returns the tool's *rest height* = the max mesh Z under the full tool
>    footprint. So the `(surf_z + leave)` floor the lift/mask already use is
>    the radius-dilated surface. There is no missing dilation to add.
>
> 2. **Synthetic experiment confirms drop-cutter protects against the gouge.**
>    `tests/adaptive3d_subtool_channel_gouge.rs` builds a deep (-12 mm),
>    sub-tool-width V-valley with tall walls and roughs it with a 6 mm flat
>    endmill, DPP 3. Result: the tool descends only to z=-9 (NOT the -12
>    floor — drop-cutter stops it where it rests), the deepest samples have
>    the *lowest* axial (0.41 mm), and peak steady-state axial is 2.46 mm
>    (< DPP). **No gouge.** A flat tool cannot be driven below its rest
>    height by the lift, because the lift reads that rest height.
>
> So the wanaka gouge is NOT "flat tool draped into a sub-tool notch via a
> point-surface lift." The surface heightmap is mesh-based and correct.
>
> **Real mechanism (now the lead hypothesis): planner↔simulator STOCK-STATE
> parity gap — the F-027 "interior cell" caveat.** The dexel simulator
> carries UNCLEARED stock standing *above* the mesh keep-surface (material
> the planner's own `material_stock` believes was cleared, or never
> scheduled to clear progressively). A later deep pass — whose cut Z is the
> correct mesh depth — sweeps its footprint through that tall uncleared
> column and removes it in ONE bite → `axial_engagement_mm`/`axial_doc_mm`
> reads the full column height (the 6–20 mm "gouge"). This is the same
> family as F-027 (model-edge cells) and F-031 (helix-entry stamping
> mismatch), both of which were planner↔sim stamping-divergence bugs. The
> F-027 sentry even documents this exact out-of-scope class:
> "interior cells inside the mesh XY footprint that the planner thinks are
> cleared but the simulator hasn't stamped before a deep-Z dive."
>
> The wanaka probe evidence (`ray_top=11.18` in footprint, cut to 5.05)
> fits this: 11.18 is uncleared STOCK above the ~5 mm mesh surface, not a
> mesh wall (a mesh wall would have raised drop-cutter's rest height and
> the tool would never have descended).
>
> **OPEN QUESTION before any fix — is the final geometry actually gouged,
> or is the cut depth correct (just taken in one big bite)?** If 5.05 mm is
> the correct mesh depth at that XY, the FINAL surface is fine and the bug
> is only a DPP-violation / deflection-gate over-fire (metric + tool-load
> safety), not a visible over-cut. But the user reports a VISIBLE channel
> milled deeper than the surrounding hills. Resolving this needs ground
> truth: drop-cutter the (setup-transformed) wanaka mesh at the peak gouge
> XY and compare to the final dexel height. Frame handling (face_up=Bottom)
> makes this non-trivial — see `wanaka_axial_doc.rs`. Do this BEFORE
> committing to a fix direction.

> ## ✅ RESOLVED 2026-06-16 — by-design ocean-wave trench in the mesh, not an engine bug (and NOT a seam/hole)
>
> **Final answer, confirmed by the rivmap/mesh-generator owner:** the deep
> coastline channel is **real, by-design geometry** — rivmap's ocean-wave
> baking carves the entire sea surface BELOW the coastal land plane
> (`features.rs` `bake_ocean_waves`: sea Z = base − wave_offset at peaks,
> base − (wave_offset + wave_depth) at troughs; land sits at ≥ base). With
> studio defaults the sea spans ~2.8 → −0.2 mm while abutting land starts at
> 3.0 mm → a continuous ~3 mm step right at the shoreline (larger on
> Wanaka's scale, matching the ~5–6 mm CAM measured). The coastline carries
> the highest mesh weight (10.0) so the greedy mesher renders that drop in
> full fidelity → "a deep narrow channel along the coastline." The mesh is
> **one watertight 2.5D surface — no seam, no hole, no two-part join.**
>
> Corrections to my earlier forensic write-up below:
> - The "mesh seam / hole" framing was **wrong**. There is no defect; the
>   trench is intentional wave geometry.
> - The `-inf` drop-cutter reading at local (26,20) was a **frame/coordinate
>   EDGE artifact (outside the XY footprint), NOT a hole** — exactly the
>   frame-sensitivity I flagged on the probe itself. Do not cite it as a hole.
>
> What DID hold up (the engine verdict is unchanged): the rough conforms to
> the mesh within 0.3 mm, the 6.125 mm peak is a transit/entry sample, and
> steady-state load is ~DPP. **The engine faithfully cuts a real trench; it
> is not over-cutting.**
>
> **Slider semantics (confirmed against rivmap source — `features.rs`
> `bake_ocean_waves`, gated ONLY on `island_mask`, applied uniformly to all
> ocean pixels — there is NO coastline-distance term / shore groove):**
> - `wave_offset` = how far the WHOLE sea sits below the land base = the
>   sea-vs-island STEP. This is the offset you want to keep.
> - `wave_depth` = peak-to-trough amplitude of the Voronoi wave TEXTURE on
>   top of the sea. Troughs reach `base − (wave_offset + wave_depth)`. This
>   is what dips deep (the ~6 mm CAM measured ≈ offset + depth at trough
>   floors). This is the "gouge".
>
> The CAM 53-feed deepest sliver = the deepest Voronoi-trough cells reaching
> the bottom roughing slab (scattered, NOT a continuous coastline outline) +
> finishing feeds concentrating on the steep land→sea wall. The bulk of the
> sea is caught by the intermediate slabs (z≈11/15/18).
>
> **Fix (rivmap params, NOT mesh repair, NOT engine change):** keep
> `wave_offset` = the step you want (e.g. 1.5–2 mm); set `wave_depth = 0`
> (flat sea, clean two-level step) or `wave_depth ≤ wave_offset` (e.g. 0.5
> mm) so troughs never reach the land plane. **Do NOT zero `wave_offset`** —
> that removes the step and makes sea tops flush with land while troughs
> still gouge (the worst case; an earlier suggestion to zero offset was
> backwards).
>
> The radius-dilation theory AND the planner↔sim-parity theory remain moot.
>
> ### Addendum 2026-06-16 (definitive final-surface measurement)
>
> Followed up on "why doesn't same-height stock get the same treatment?"
> with a SIM-BASED test (`wanaka_final_surface_vs_mesh`, `#[ignore]`):
> simulate the full Back Rough onto a dexel, read the ACTUAL final stock top
> per cell, compare to the mesh keep-surface (6 mm drop-cutter, setup-local).
> This measures real removed material — immune to the move-target/transit
> confounds of the earlier proxy.
>
> Result — **same-height stock IS cut unequally** (the user was right):
> - 642 cells at mesh height [4,6] mm: **67 over-cut** (leave < 1 mm — the
>   ~4 mm stock-to-leave eaten), incl. cells at **leave = −1.9 mm (cut BELOW
>   the keep surface, a real ~2 mm gouge)**; **573 proper** (leave ≥ 3 mm).
> - Leave left behind spans **0 → 14 mm at the same mesh height** — an
>   uneven roughed surface, not a uniform offset.
> - The over-cut cells are **scattered across the whole 93×84 mm area**
>   (centroid centred), tracking the wave-texture troughs — NOT one channel.
>
> **Mechanism:** each deep wave-trough forces a deep planned Z-level; when
> the 6 mm flat tool drops to cut the trough, its footprint laps sideways
> and eats the leave on the higher same-height stock around it (and cuts
> ~2 mm below the keep surface in places). Scattered troughs → scattered
> over-cuts; between troughs the leave is kept.
>
> **So two things are both true:**
> 1. INPUT: `wave_depth` makes the troughs. `wave_depth → 0` removes them →
>    uniform rough, no scatter (the primary fix, as the mesh owner said).
> 2. ENGINE: the rough does NOT hold a uniform stock-to-leave over a
>    troughed/textured mesh — it over-cuts (and slightly gouges, ~2 mm) the
>    leave around deep narrow features. A real secondary roughing weakness,
>    only triggered by the texture. Worth a look if robustness over messy/
>    textured meshes is wanted; not blocking this job (fix the wave param).
>
> ### ✅ ENGINE FIX LANDED 2026-06-16 (commit fa27b08) — leave now held
>
> The user's call: changing the mesh is a workaround; generic CAM must hold
> the leave regardless of mesh. Fixed in `segments_to_toolpath`
> (`adaptive3d/path.rs`). Root cause (mapped, not the earlier guesses): the
> lift reads a SINGLE grid cell (`surface_z_at_world` rounds to one cell) and
> straight cut segments are emitted between sparse points, so segment
> interiors / between-cell points dip below `surface + leave` on textured
> meshes (sub-heightmap-cell aliasing on the 220k-tri DEM). Entries plunged
> to the un-draped Z; stay-down links cleared only `terrain + clearance`.
>
> Three guards, all only ever RAISE Z (a legitimately deep cut is preserved):
> - `drape_path_to_leave`: densify each cut to `<= tool radius`, lift every
>   point to `max(cut_z, point_drop_cutter(xy) + leave)`. At step ≤ radius no
>   peak hides between samples.
> - `drape_point` on entry destinations (peck/helix/ramp) via a shadow bind.
> - stay-down link Z clears `terrain + stock_to_leave` (was `+ clearance`).
>
> Result on wanaka Back Rough (final dexel surface vs mesh keep-surface):
> over-cut same-height cells **67 → 0**; worst leave **−1.9 mm → +2.03 mm**
> (nothing cut below the keep surface). All adaptive3d sentries pass; clippy
> + fmt clean. `wanaka_final_surface_vs_mesh` (#[ignore]) now ASSERTS the
> leave is held (0 over-cut, worst leave > −0.5) as the regression gate — no
> fast synthetic reproduces the DEM aliasing, so the fixture test is the guard.
>
> Hard evidence (Back Rough, setup face_up=Bottom, setup-local frame):
> - **The 6.125 mm peak `axial_doc` is a TRANSIT/ENTRY sample**
>   (`in_transit_span = true`, move 3158, 97.8% through the toolpath).
>   Move 3155 rapids to safe-Z, 3156 plunges 31→5.053, 3157/3158 are the
>   first laterals that shear uncleared stock (`ray_top 11.18` in the
>   footprint). The deflection/load gates already EXCLUDE transit samples.
> - **The worst STEADY-STATE sample is only 3.456 mm** (~commanded DPP 3).
>   Steady-state load is fine.
> - **The cut conforms to the mesh:** at the peak XY (26.04, 29.01) the
>   mesh keep-surface (drop-cutter, setup-local) = 5.351 mm and the cut Z
>   = 5.053 mm → the tool cut only **0.297 mm** below the surface. There is
>   NO sub-surface over-cut.
> - **The mesh genuinely dips here.** Cross-channel profile of the mesh
>   surface around the peak reads ~5.0–6.7 mm — a real channel in the mesh,
>   ~6 mm below the hill levels (which the Z-histogram shows at z≈11/15/18/22,
>   ~630 feeds each; the z≈5 channel has only 53 feeds, cut dead last).
> - **There is a HOLE in the mesh next to it:** drop-cutter returns
>   `-inf` (no triangle) at local (26, 20). This is the user-reported
>   "seam where my mesh generator joins 2 mesh parts." Drop-cutter over a
>   hole clamps to `min_z`, so the rough descends into the gap.
>
> **Conclusion.** The user's FIRST instinct was correct: the artifact tracks
> a mesh seam. The deep narrow channel is (almost certainly) a fold/crack
> from the bad join, and the rough faithfully mills it. The engine did not
> over-cut — final geometry matches the (defective) mesh within 0.3 mm.
>
> **Remedies (input/robustness, not an over-cut fix):**
> 1. Repair the mesh seam (watertight join, no hole) — upstream fix.
> 2. Pin a heights `bottom_z` to stop the rough descending into the
>    hole/channel (the documented lever; see the `SurfaceHeightmap.covered`
>    note in slope.rs and the heights audit 2026-06-12 findings 2+3).
> 3. (Optional engine robustness) treat `covered == false` (hole) cells as
>    non-cutting / clamp them to neighbour surface instead of `min_z`, so a
>    mesh hole can't pull a deep dive. Tracked but not required for this job.
>
> The forensic probes live in `wanaka_axial_doc.rs` (still `#[ignore]`).

---

**Status:** ~~root-caused~~ **RESOLVED 2026-06-16 — by-design rivmap
ocean-wave geometry, not an engine over-cut (and not a seam/hole).** The sea
sits `wave_offset` below land (the step) with `wave_depth` Voronoi texture on
top; the deep cuts are the texture troughs (`offset + depth`). No engine fix.
Remedy: keep `wave_offset` as the step, set `wave_depth → 0` (or ≤ offset) —
NOT zero offset. Earlier theories (radius-dilation; planner↔sim parity; mesh
seam/hole; coastline-only groove) all refuted; the `-inf` "hole" was a
frame-edge probe artifact.
**Repro source:** wanaka100 `terrain.stl` (rivmap DEM, mountains + coastline),
`planning/airrun_2026-06-01/wanaka.toml`, Back Rough op (adaptive3d, 6mm flat
endmill, DPP 3, stepover 2.53, stock_to_leave 4mm, `face_up=Bottom` setup).

## Symptom

Every rough (agent OR spiral) carves the diagonal **coastline channel** much
deeper than the surrounding terrain — a visible gouge. Reported peak axial DOC
6–20 mm vs the 3 mm commanded DPP, always at the same channel spot
(~world XY 26,30), always at the channel floor Z.

## Root cause (CONFIRMED via `wanaka_axial_doc` diagnostic probe)

It is a **real over-cut**, not a metric artifact. The flat 6 mm endmill
surface-follows down to the **deep channel floor (setup-local z≈5.05)** while
the **adjacent terrain wall is uncleared** — the dexel probe shows real
material with `ray_top = 11.18` inside the cutter footprint right before the
peak move; the move cuts to z=5.05 → ~6 mm of real removal. The flat tool's
corner grazes/gouges the steep channel wall because **the channel is narrower
and deeper than the tool can fit**. A 6 mm flat endmill physically cannot rough
a ~5 mm-deep, sub-tool-width coastline notch without gouging its walls.

Key probe line:
`PROBE before move 3158: peak ray_top in footprint at (26.00,32.00): top=11.18`
peak move 3158: Linear z=5.053, axial DOC 6.125 mm.

## Hypotheses RULED OUT (do not re-chase)

- **Mesh defect** — STL is clean: 0 interior cracks, 0 non-manifold, 0
  overhangs/inverted faces (98.7% up-facing). Watertight DEM.
- **Path strategy** — method-independent (agent + spiral both do it). It's
  tool-vs-geometry, not path shape.
- **Stepdown size (DPP)** — halving DPP 3→1.5 made it WORSE (12.5→19.5 mm):
  finer levels → more sub-tool fragmentation.
- **Region dropping (`min_region_cut_length_mm`)** — setting 0 didn't help
  (~20 mm).
- **Stay-down bridging (`max_stay_down_distance_mm`)** — link bridges go OVER
  terrain (`link_z = max(samples).max(from).max(to)+clearance`), not through.
- **F-024 / flipped-setup frame bug (WRONG — I chased this and it's refuted)** —
  the synthetic flipped pocket test `flipped_setup_axial_doc_repro.rs` PASSES
  at 2.0 mm, so non-identity grid rooting is fine for 2.5D. And the probe shows
  real material removed, not a mis-read. The `face_up=Bottom` setup is a red
  herring.

## Repro artifacts (already in tree)

- `tests/flipped_setup_axial_doc_repro.rs` — synthetic flipped pocket, PASSES
  (2.0 mm). Keep as a sentry; it proves flipped 2.5D axial is correct and
  rules the frame theory back out if anyone re-chases it.
- `tests/wanaka_axial_doc.rs` — `#[ignore]` diagnostic that loads the real
  wanaka and probes the dexel at the peak DOC. Run with
  `cargo test --test wanaka_axial_doc -- --ignored --nocapture`. This is the
  ground-truth repro (no fast synthetic exists yet).
- `tests/dexel_stock_z_frame_f024.rs` — the IDENTITY-setup F-024 sentry
  (passes). Template for the synthetic test structure.

## Engine fix — direction

The 3D rough should **not drape the floor cut into a sub-tool-width / deep
channel** where the cutter footprint overlaps material more than ~DPP above the
cut Z. It should stop the cut above the notch and leave it for the finish
(tapered-ball) pass which can physically reach it.

Candidate fix sites:
- `adaptive3d/clearing.rs` — the `lift` closure (`z = (surf_z+stl).max(z_level)`)
  and the per-Z-level marching-squares clear. Investigate how cuts reach
  z≈5.05 BELOW the lowest planned major level (10) — surface-follow / sub-pass.
- `adaptive3d/path.rs` `segments_to_toolpath` — lift bridging + where the
  emitted cut Z is set; this is where a clamp ("don't cut more than DPP below
  the local uncleared material top") would live.
- Z-level planning (`adaptive3d/mod.rs` / `path.rs` z_levels) — does it plan
  levels down to the channel floor, or surface-follow into it on the last pass?

Acceptance test to write: a synthetic deep-narrow-channel mesh (flat terrain
with a sub-tool-width slot dropping ~6 mm), adaptive3d rough with a flat tool,
assert per-sample axial DOC ≤ ~DPP+margin (currently it will gouge the slot
walls). This is the fast repro the prior session lacked.

## PRECISE mechanism + fix (2026-06-16, traced)

- Z-level plan (`adaptive3d/path.rs:447`): `z_bottom = surface_bottom +
  stock_to_leave` (clamped by `z_floor`). `surface_bottom` = the GLOBAL min
  surface = the channel floor, so the rough plans a level right at the channel
  floor (z≈5.05). That level's cut is real.
- The `SurfaceHeightmap` IS cutter-radius-aware (drop-cutter, `slope.rs:89`
  `point_drop_cutter`), so the cutter CENTRE correctly finds the channel floor
  at a cell wide enough to drop into.
- **THE BUG:** the effective floor / lift uses the **point** surface, not the
  **cutter-radius-dilated** surface:
  - `clearing.rs:373` `effective_floor = (surf_z + stock_to_leave).max(z_level)`
  - the lift closure `z = (surf_z + stl).max(z_level)`
  Both read `surf_z` at the single cell. The cutter has radius ~3 mm, so cutting
  a channel cell to z=5.05 removes material across the footprint — including the
  adjacent wall cell whose leave-level is 11.18 → 6 mm wall gouge.

- **FIX:** dilate `(surf_z + stock_to_leave)` by the cutter (engagement) radius
  — i.e. the effective floor at cell A = `max over footprint(A) of
  (surf + stl)`, then `.max(z_level)`. A morphological max-filter of the
  leave-surface by the cutter radius. In a sub-tool channel the dilated floor =
  the wall height, so the rough leaves the notch (correct — flat tool can't fit)
  and the tapered-ball finish carves it. No wall gouge.
  - Apply in `build_material_bool_grid` (clearing.rs:334/373) AND the lift
    (clearing.rs ~1599) — both must use the dilated floor for consistency
    (mask decides WHERE to cut, lift decides the Z).
  - Cheapest impl: precompute a radius-dilated copy of the SurfaceHeightmap
    once per op (max-filter `surf_z` over a disk of the engagement radius in
    grid cells), pass it alongside the raw heightmap; clearing reads the
    dilated one for floor/mask decisions. Keep the raw heightmap for slope/
    flat-area detection where the true surface is wanted.
  - Watch: don't dilate so aggressively that legitimately-wide pockets get
    under-cut. The dilation radius = engagement radius (contact radius at DOC),
    not envelope radius. Validate against the existing F-024/F-029/F-036 sentries
    + the AS001 acceptance (pockets must still bottom out correctly).
  - Risk: this changes EVERY adaptive3d rough's floor slightly (leaves a hair
    more in concave fillets). Gate hard against the sweep + sentry suite.

## Side findings (separate bugs, worth logging)

- **Spiral generation hangs at near-slot stepover (≥~80% tool diameter)** — the
  iso-contour wrap extraction degenerates (multi-hour gen). Needs a hard cap /
  early-bail at ~80% diameter.
- **Full-job sim wedges for 45+ min** on the wanaka job (dominated by the
  112k-move tapered-ball finish op + the deep-channel state). Isolating the
  rough (disable other toolpaths) drops sim time ~13×.
- `run_simulation`'s `per_toolpath` lists disabled toolpaths (reporting noise;
  confirm they're excluded from the aggregate metrics/compute).

## Also from this session (unrelated, already committed on experiment/adaptive-spiral)

- Nibble control built then removed (commit ae6f5f3) — trochoid relief is a
  corner-relief edge case, inert on open roughing; "Optimal load" slider +
  tuned `trochoid_cap_mult=1.6` kept. See `[[project_spiral_wallclock_structural]]`.
