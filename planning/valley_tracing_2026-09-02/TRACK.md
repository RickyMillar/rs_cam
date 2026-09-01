# Track H — valley tracing (drainage-tree finishing)

> Opened 2026-09-02. Operator's proposal: identify each "catchment
> area" on the terrain, trace a path up each valley to form a tree,
> then trace around that tree with a surface-relative offset — "out
> one way, switch and trace back the other" — instead of raster
> passes over valley areas. Operator's stated worry: triangle facets
> may make smooth valleys zig-zag.
>
> Status: **V0 MEASURED 2026-09-02 — bars pass; V1 CONDITIONALLY
> open pending V0-att attribution.** Census:
> `valley_prize_census_h0.rs` (`df3383a2`). In-mask ×floor
> 1.21–1.25 vs territory 1.155 — but the shipped-derate arithmetic
> alone predicts that band (0.486/0.344 = 1.41× on flat floors), so
> the excess may hold zero path-topology prize. V0-att (M2_spacing,
> residual bar) decides. See `FINDINGS.md` results.

## The question

Can a drainage-tree strategy beat the shipped (honest) raster inside
valley territory on wanaka, at equal achieved scallop, with fewer
fragments and links?

## Two readings of the proposal — only one is open

The literal reading (concentric offset loops around the whole tree)
is the medial-axis level-set family. The synthesis §9 falsifier
already fired on it (`planning/finishing_synthesis_2026-08-30.md`):
2.525× floor vs raster 1.310× on the 8-arm ribbon, 179 fragments vs
98. Offset rings split at every branch point. A drainage tree is
more dendritic than that fixture. **Track H does not test this
reading.**

The open reading is per-branch: trace along each valley segment,
offset passes out one side and back, capped by the local valley
half-width, with junctions handled explicitly. Pencil's offset-pass
machinery already does the per-segment part (`pencil.rs`,
width-capped since `29a6d61`).

## Standing priors (do not re-derive)

- **The worst-point rule.** One loop that hugs both valley walls
  spans different slopes; a single spacing per loop pays the worst
  point. Ledger arm D1: 99.7 % of rings hit the 45° derate clamp and
  the arm still failed spec at 11.33 % exceed.
- **Pencil does not follow drainages by design.** The `rest_depth`
  detector traces ball-reachability ridges, not topography. A broad
  valley the reference ball fully contacts has rest ≈ 0 and no line.
  The default detector is still `Dihedral`.
- **A flow-accumulation drainage detector existed and was deleted**
  (`53293c96`, 2026-07-03, operator ruling: hydrology trunks are not
  tool-relevant seams). That ruling was about pencil seam-finding.
  It does not bind a finishing-territory use, but the post-mortem
  stands: coherent lines, wrong scalar field for that purpose. The
  code was never committed; only the raster machinery survives,
  live, inside `rest_field.rs`.
- **C2 is default-ON.** The raster is already PCA-rotated to each
  region's long axis. A valley trunk is already swept along its own
  axis. The residual prize is oblique tributaries and loop overhead.
- **Literature.** The catchment + valley-tree decomposition of a
  triangulated surface is the Morse–Smale complex. Persistence
  simplification removes the spurious pits and passes a
  triangulation invents. The operator's triangle-noise worry is the
  solved part; the branch-point offset behaviour is the risky part.
- **Region shape decides the strategy** (synthesis §9 conclusion):
  compact → offsets, elongated/branched → one-direction sweeps.
  Track H's open reading conforms to this; the closed reading
  contradicts it.

## Phases — each gated on the one before it

- **V0 — prize census. MUST run first (X4).** Measure, on the
  wanaka front finish territory: (a) the fraction of finish time
  spent inside valley territory; (b) the ×floor ratio
  (`L_min = ∫dA/s_max`) of the shipped honest raster inside that
  territory; (c) the misalignment between the C2 lattice and the
  local valley axis; (d) the per-region cos θ_max derate refund
  from excising valley walls. (c) and (d) attribute the excess to a
  mechanism — tracing can only win what (c) exposes; (d) belongs to
  decomposition. A ceiling pre-check (the anisotropy census
  restricted to the valley mask) can close the track with zero new
  strategy code. Bars and decision rules are pre-registered in
  `FINDINGS.md` before the instrument runs.
- **V1 — the branch-offset falsifier.** Feed a tree into pencil's
  offset machinery on one valley region. Cost it through
  `relink_and_cost_under` under the machined-stock `link_ceiling`
  regime, at equal achieved scallop. The §9 bar applies verbatim
  and is pre-registered in `FINDINGS.md`.
- **V2 — junction handling and the raster hybrid.** Design work.
  Opens only if V1 passes its bar.

## Cross-cutting constraints

- **X1–X6** from `planning/thin_organic_2026-08-27/PROGRAMME.md`
  apply to every phase. X4 (measure before building) is the reason
  V0 exists.
- **Fixture rule.** wanaka200's median edge (~0.35 mm) fails the
  ≤ stepover/3 facet rule against the 0.486 mm stepover. A spacing
  claim needs a finer rivmap export (`max_error` dial) or must be
  labelled as not a spacing claim. `fixtures/terrain_small.stl` is
  banned for any spacing claim (status doc §6).
- **G-UNIONCOV caps every claim** (status doc §12). No whole-board
  coverage instrument exists. A valley op that replaces raster
  inside its territory creates exactly the seam-ownership gap that
  instrument would check. Until it exists, no Track H result may
  claim "faster AND complete" — per-op findings only.
- **Tree source caveat.** `rivers_aligned.dxf` (249 polylines,
  already a `project_curve` input) is map hydrography. The mesh is
  exported with `river_through_cut = false`, so the DXF network is
  not incised and may not sit in the mesh's geometric valleys. The
  DXF is acceptable for a cheap first probe; a faithful arm derives
  the tree from the mesh itself.

## Files

- Pre-registration + results: `planning/valley_tracing_2026-09-02/FINDINGS.md`
- Instruments: named per phase in `FINDINGS.md` when written.
  Convention: `#[ignore]` evidence instruments under
  `crates/rs_cam_core/tests/`, committed as soon as they lint clean.
